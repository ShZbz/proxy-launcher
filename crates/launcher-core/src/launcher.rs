//! Process spawning with proxy environment variable injection.
//!
//! Spawns target applications as detached processes (`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`)
//! with null stdio streams and injected proxy environment variables (`HTTP_PROXY`, `HTTPS_PROXY`,
//! `ALL_PROXY`, lowercase variants, `NO_PROXY`, and `AICODE_ENDPOINT_URL`).

use crate::models::{Config, Port, Rule, AICODE_ENDPOINT_URL_DEFAULT, NO_PROXY_VALUE};
use crate::probe::probe_local_port;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug)]
pub enum LaunchError {
    RuleNotFound(String),
    PortNotFound(String),
    TargetNotFound(String),
    UwpNotSupported,
    PortUnreachable {
        host: String,
        port: u16,
        reason: String,
    },
    ElevationRequired(String),
    SpawnFailed(String),
    Io(io::Error),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::RuleNotFound(r) => write!(f, "Rule not found: {}", r),
            LaunchError::PortNotFound(p) => write!(f, "Port not found: {}", p),
            LaunchError::TargetNotFound(t) => {
                write!(f, "Target executable does not exist: {}", t)
            }
            LaunchError::UwpNotSupported => {
                write!(
                    f,
                    "UWP/AppX applications cannot be launched with proxy environment injection"
                )
            }
            LaunchError::PortUnreachable { host, port, reason } => write!(
                f,
                "Proxy port {}:{} is not reachable: {}. Please ensure your proxy client is running.",
                host, port, reason
            ),
            LaunchError::ElevationRequired(t) => write!(
                f,
                "Target application '{}' requires administrator privileges (UAC elevation). Injected proxy environment cannot cross UAC boundaries.",
                t
            ),
            LaunchError::SpawnFailed(reason) => write!(f, "Failed to spawn process: {}", reason),
            LaunchError::Io(e) => write!(f, "I/O error during process launch: {}", e),
        }
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LaunchError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for LaunchError {
    fn from(e: io::Error) -> Self {
        LaunchError::Io(e)
    }
}

/// Builds the complete proxy environment variable suite.
///
/// For loopback hosts (`127.0.0.1`, `localhost`, `::1`, `[::1]`), `HTTP_PROXY` and
/// `HTTPS_PROXY` (and their lowercase twins) use uppercase `LOCALHOST` so that
/// Antigravity's `language_server.exe plugin-cli` (`plugincli.ignoreCommandProxy`)
/// does not mistake the user proxy for its internal loopback sandbox proxy and strip it.
pub fn build_proxy_env(host: &str, port: u16) -> Vec<(&'static str, String)> {
    let trimmed = host.trim();
    let env_http_host = if trimmed == "127.0.0.1"
        || trimmed == "::1"
        || trimmed == "[::1]"
        || trimmed.eq_ignore_ascii_case("localhost")
    {
        "LOCALHOST"
    } else {
        host
    };
    let http_url = format!("http://{}:{}", env_http_host, port);
    let socks_url = format!("socks5://{}:{}", host, port);

    vec![
        // Standard uppercase (Go, Node, Rust, generic clients)
        ("HTTP_PROXY", http_url.clone()),
        ("HTTPS_PROXY", http_url.clone()),
        ("ALL_PROXY", socks_url.clone()),
        ("NO_PROXY", NO_PROXY_VALUE.to_string()),
        // Lowercase twins (curl, git, npm, python requests)
        ("http_proxy", http_url.clone()),
        ("https_proxy", http_url),
        ("all_proxy", socks_url),
        ("no_proxy", NO_PROXY_VALUE.to_string()),
        // Antigravity IDE remote agent fastpush bug workaround
        ("AICODE_ENDPOINT_URL", AICODE_ENDPOINT_URL_DEFAULT.to_string()),
    ]
}

/// Injects proxy environment variables into a `Command`.
pub fn inject_proxy_env(cmd: &mut Command, host: &str, port: u16) {
    for (key, val) in build_proxy_env(host, port) {
        cmd.env(key, val);
    }
}

/// Prepares the Command for spawning according to the rule and port configuration.
pub fn prepare_command(rule: &Rule, port: &Port) -> Result<Command, LaunchError> {
    if rule.exe.trim().to_lowercase().starts_with("shell:appsfolder") {
        return Err(LaunchError::UwpNotSupported);
    }

    let exe_path = Path::new(&rule.exe);
    if !exe_path.exists() {
        return Err(LaunchError::TargetNotFound(rule.exe.clone()));
    }

    let mut cmd = Command::new(&rule.exe);

    // Completely decouple stdio
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Detached process flags on Windows
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    // Set working directory
    let cwd = rule.effective_cwd();
    if cwd.exists() {
        cmd.current_dir(cwd);
    }

    // Inject environment variables if mode is "env" or "both"
    if rule.mode == "env" || rule.mode == "both" {
        inject_proxy_env(&mut cmd, &port.host, port.port);
    }

    // Prepare CLI arguments
    let mut args = rule.split_args();
    if rule.mode == "flag" || rule.mode == "both" {
        args.push(rule.formatted_flag(port));
    }
    cmd.args(args);

    Ok(cmd)
}

/// Launches a configured rule detached with injected proxy environment variables.
///
/// Returns the PID of the spawned process on success.
pub fn launch_rule(config: &Config, rule_id_or_name: &str) -> Result<u32, LaunchError> {
    let rule = config
        .find_rule(rule_id_or_name)
        .ok_or_else(|| LaunchError::RuleNotFound(rule_id_or_name.to_string()))?;

    let port = config
        .active_port_for_rule(rule)
        .ok_or_else(|| LaunchError::PortNotFound(rule.port_id.clone()))?;

    // Phase 1: Fast local loopback TCP probe (50ms timeout)
    probe_local_port(&port.host, port.port, Duration::from_millis(50)).map_err(|e| {
        LaunchError::PortUnreachable {
            host: port.host.clone(),
            port: port.port,
            reason: e.to_string(),
        }
    })?;

    let mut cmd = prepare_command(rule, port)?;

    match cmd.spawn() {
        Ok(child) => Ok(child.id()),
        Err(e) => {
            // Error 740 = ERROR_ELEVATION_REQUIRED
            if e.raw_os_error() == Some(740) {
                Err(LaunchError::ElevationRequired(rule.exe.clone()))
            } else {
                Err(LaunchError::SpawnFailed(e.to_string()))
            }
        }
    }
}
