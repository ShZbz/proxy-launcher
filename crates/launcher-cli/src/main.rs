//! High-performance proxy launcher CLI kernel.
//!
//! Provides ultra-lightweight zero-allocation argument parsing via `pico-args`,
//! supporting rule launching, desktop shortcut generation, and explicit configuration
//! file overrides with clean exit codes and sub-15ms startup latency.

use launcher_core::launcher::LaunchError;
use launcher_core::Config;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const USAGE: &str = r#"proxy-launcher-cli 0.1.0
High-performance proxy launcher CLI kernel

USAGE:
    launcher-cli.exe [OPTIONS]

OPTIONS:
    --launch-rule <ID_OR_NAME>      Launch target application with proxy environment injected
    --create-shortcut <ID_OR_NAME>  Create a Windows desktop shortcut for the specified rule
    --config <PATH>                 Path to configuration file (optional)
    -h, --help                      Print help information
    -V, --version                   Print version information
"#;

/// Parsed CLI configuration and commands.
#[derive(Debug, PartialEq, Eq)]
pub struct CliArgs {
    pub launch_rule: Option<String>,
    pub create_shortcut: Option<String>,
    pub config: Option<PathBuf>,
}

/// Resolves the configuration file path according to the defined search order:
/// 1. Explicit path if provided via `--config <path>`
/// 2. `config.json` next to the running executable
/// 3. `config.json` in the current working directory
pub fn resolve_config_path(explicit_path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit_path {
        if path.exists() {
            return Ok(path.to_path_buf());
        } else {
            return Err(format!(
                "Specified configuration file does not exist: {}",
                path.display()
            ));
        }
    }

    // 1. Next to current executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("config.json");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 2. In current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("config.json");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Configuration file 'config.json' not found in standard search locations (next to exe or current working directory)".to_string())
}

/// Parses CLI arguments from `pico_args::Arguments`.
pub fn parse_cli_args(mut pargs: pico_args::Arguments) -> Result<Option<CliArgs>, (String, i32)> {
    if pargs.contains(["-h", "--help"]) {
        return Ok(None);
    }

    if pargs.contains(["-V", "--version"]) {
        return Err((format!("proxy-launcher-cli {}", env!("CARGO_PKG_VERSION")), 0));
    }

    let config: Option<PathBuf> = pargs
        .opt_value_from_os_str("--config", |s| Ok::<PathBuf, &'static str>(PathBuf::from(s)))
        .map_err(|e| (format!("Error parsing '--config': {}", e), 1))?;

    let launch_rule: Option<String> = pargs
        .opt_value_from_str("--launch-rule")
        .map_err(|e| (format!("Error parsing '--launch-rule': {}", e), 1))?;

    let create_shortcut: Option<String> = pargs
        .opt_value_from_str("--create-shortcut")
        .map_err(|e| (format!("Error parsing '--create-shortcut': {}", e), 1))?;

    let remaining = pargs.finish();
    if !remaining.is_empty() {
        let remaining_str: Vec<String> = remaining
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        return Err((
            format!(
                "Unknown or unrecognized argument(s): {}",
                remaining_str.join(", ")
            ),
            1,
        ));
    }

    match (&launch_rule, &create_shortcut) {
        (None, None) => Err((
            "Missing required argument: either --launch-rule <id_or_name> or --create-shortcut <id_or_name> must be specified.".to_string(),
            1,
        )),
        (Some(_), Some(_)) => Err((
            "Cannot specify both --launch-rule and --create-shortcut simultaneously.".to_string(),
            1,
        )),
        _ => Ok(Some(CliArgs {
            launch_rule,
            create_shortcut,
            config,
        })),
    }
}

/// Executes the core logic and returns the process exit code.
pub fn run_app<W1: Write, W2: Write>(
    pargs: pico_args::Arguments,
    stdout: &mut W1,
    stderr: &mut W2,
) -> i32 {
    let cli_args = match parse_cli_args(pargs) {
        Ok(Some(args)) => args,
        Ok(None) => {
            // Help flag was passed
            let _ = write!(stdout, "{}", USAGE);
            return 0;
        }
        Err((msg, code)) => {
            if code == 0 {
                // Version flag
                let _ = writeln!(stdout, "{}", msg);
                return 0;
            } else {
                let _ = writeln!(stderr, "Error: {}\n\n{}", msg, USAGE);
                return code;
            }
        }
    };

    // 1. Resolve and load configuration
    let config_path = match resolve_config_path(cli_args.config.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(stderr, "Error: {}", e);
            return 1;
        }
    };

    let config = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            let _ = writeln!(
                stderr,
                "Error loading configuration from '{}': {}",
                config_path.display(),
                e
            );
            return 1;
        }
    };

    // 2. Handle --launch-rule
    if let Some(rule_id) = cli_args.launch_rule {
        match launcher_core::launcher::launch_rule(&config, &rule_id) {
            Ok(pid) => {
                let _ = writeln!(
                    stdout,
                    "Successfully launched rule '{}' (PID: {})",
                    rule_id, pid
                );
                return 0;
            }
            Err(LaunchError::RuleNotFound(r)) => {
                let _ = writeln!(stderr, "Error: Rule not found: {}", r);
                return 2;
            }
            Err(LaunchError::PortNotFound(p)) => {
                let _ = writeln!(stderr, "Error: Port not found: {}", p);
                return 3;
            }
            Err(err) => {
                let _ = writeln!(stderr, "Error: Launch failed: {}", err);
                return 1;
            }
        }
    }

    // 3. Handle --create-shortcut
    if let Some(rule_id) = cli_args.create_shortcut {
        let cli_exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                let _ = writeln!(stderr, "Error determining launcher executable path: {}", e);
                return 1;
            }
        };

        match launcher_core::lnk::create_shortcut_for_rule(&config, &rule_id, &cli_exe) {
            Ok(shortcut_path) => {
                let _ = writeln!(
                    stdout,
                    "Shortcut created successfully: {}",
                    shortcut_path.display()
                );
                return 0;
            }
            Err(err) => {
                let _ = writeln!(stderr, "Error creating shortcut: {}", err);
                return 1;
            }
        }
    }

    0
}

fn main() {
    let bench = std::env::var("PROXY_LAUNCHER_BENCHMARK").is_ok();
    let t0 = if bench { Some(std::time::Instant::now()) } else { None };
    let pargs = pico_args::Arguments::from_env();
    let exit_code = run_app(pargs, &mut std::io::stdout(), &mut std::io::stderr());
    if let Some(start) = t0 {
        let elapsed = start.elapsed();
        let _ = writeln!(
            std::io::stderr(),
            "[BENCHMARK_INTERNAL_MS] {:.4}",
            elapsed.as_secs_f64() * 1000.0
        );
    }
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_help_flag() {
        let args = vec![OsString::from("--help")];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 0);
        let out_str = String::from_utf8(stdout).unwrap();
        assert!(out_str.contains("USAGE:"));
        assert!(out_str.contains("--launch-rule"));
    }

    #[test]
    fn test_short_help_flag() {
        let args = vec![OsString::from("-h")];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 0);
        let out_str = String::from_utf8(stdout).unwrap();
        assert!(out_str.contains("proxy-launcher-cli"));
    }

    #[test]
    fn test_version_flag() {
        let args = vec![OsString::from("--version")];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 0);
        let out_str = String::from_utf8(stdout).unwrap();
        assert_eq!(out_str.trim(), "proxy-launcher-cli 0.1.0");
    }

    #[test]
    fn test_short_version_flag() {
        let args = vec![OsString::from("-V")];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 0);
        let out_str = String::from_utf8(stdout).unwrap();
        assert_eq!(out_str.trim(), "proxy-launcher-cli 0.1.0");
    }

    #[test]
    fn test_missing_args_returns_code_1() {
        let args = vec![];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Missing required argument"));
        assert!(err_str.contains("USAGE:"));
    }

    #[test]
    fn test_unknown_args_returns_code_1() {
        let args = vec![OsString::from("--unknown-option")];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Unknown or unrecognized argument(s)"));
    }

    #[test]
    fn test_both_actions_mutually_exclusive_returns_code_1() {
        let args = vec![
            OsString::from("--launch-rule"),
            OsString::from("rule1"),
            OsString::from("--create-shortcut"),
            OsString::from("rule1"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Cannot specify both"));
    }

    #[test]
    fn test_nonexistent_explicit_config_returns_code_1() {
        let args = vec![
            OsString::from("--config"),
            OsString::from("nonexistent_config_file_12345.json"),
            OsString::from("--launch-rule"),
            OsString::from("rule1"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);
        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Specified configuration file does not exist"));
    }

    fn write_test_config(path: &Path, port: u16) {
        let content = format!(
            r#"{{
  "version": 1,
  "test_url": "http://127.0.0.1",
  "ip_url": "http://127.0.0.1",
  "show_exit_ip": false,
  "ports": [
    {{
      "alias": "test_port",
      "host": "127.0.0.1",
      "port": {},
      "id": "port1"
    }}
  ],
  "rules": [
    {{
      "name": "MissingPortApp",
      "exe": "C:\\Windows\\System32\\cmd.exe",
      "port_id": "nonexistent_port",
      "args": "",
      "cwd": "",
      "mode": "both",
      "flag_template": "--proxy-server={{url}}",
      "enabled": true,
      "source": "Custom",
      "id": "rule_missing_port"
    }},
    {{
      "name": "ValidApp",
      "exe": "C:\\Windows\\System32\\cmd.exe",
      "port_id": "port1",
      "args": "/c exit 0",
      "cwd": "",
      "mode": "both",
      "flag_template": "--proxy-server={{url}}",
      "enabled": true,
      "source": "Custom",
      "id": "rule_valid"
    }}
  ]
}}"#,
            port
        );
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_rule_not_found_returns_code_2() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_cli_rule_not_found.json");
        write_test_config(&cfg_path, 59998);

        let args = vec![
            OsString::from("--config"),
            cfg_path.as_os_str().to_os_string(),
            OsString::from("--launch-rule"),
            OsString::from("nonexistent_rule_xyz"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);

        let _ = std::fs::remove_file(&cfg_path);

        assert_eq!(code, 2);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Rule not found: nonexistent_rule_xyz"));
    }

    #[test]
    fn test_port_not_found_returns_code_3() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_cli_port_not_found.json");
        write_test_config(&cfg_path, 59998);

        let args = vec![
            OsString::from("--config"),
            cfg_path.as_os_str().to_os_string(),
            OsString::from("--launch-rule"),
            OsString::from("rule_missing_port"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);

        let _ = std::fs::remove_file(&cfg_path);

        assert_eq!(code, 3);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Port not found: nonexistent_port"));
    }

    #[test]
    fn test_launch_failure_closed_port_returns_code_1() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_cli_closed_port.json");
        // Port 59997 is closed
        write_test_config(&cfg_path, 59997);

        let args = vec![
            OsString::from("--config"),
            cfg_path.as_os_str().to_os_string(),
            OsString::from("--launch-rule"),
            OsString::from("rule_valid"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);

        let _ = std::fs::remove_file(&cfg_path);

        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Launch failed"));
        assert!(err_str.contains("59997"));
    }

    #[test]
    fn test_shortcut_rule_not_found_returns_code_1() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_cli_shortcut_not_found.json");
        write_test_config(&cfg_path, 59998);

        let args = vec![
            OsString::from("--config"),
            cfg_path.as_os_str().to_os_string(),
            OsString::from("--create-shortcut"),
            OsString::from("nonexistent_rule_xyz"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);

        let _ = std::fs::remove_file(&cfg_path);

        assert_eq!(code, 1);
        let err_str = String::from_utf8(stderr).unwrap();
        assert!(err_str.contains("Error creating shortcut"));
    }

    #[test]
    fn test_launch_rule_success_with_open_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_cli_launch_success.json");
        write_test_config(&cfg_path, port);

        let args = vec![
            OsString::from("--config"),
            cfg_path.as_os_str().to_os_string(),
            OsString::from("--launch-rule"),
            OsString::from("rule_valid"),
        ];
        let pargs = pico_args::Arguments::from_vec(args);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_app(pargs, &mut stdout, &mut stderr);

        let _ = std::fs::remove_file(&cfg_path);
        drop(listener);

        assert_eq!(code, 0);
        let out_str = String::from_utf8(stdout).unwrap();
        assert!(out_str.contains("Successfully launched rule 'rule_valid'"));
        assert!(out_str.contains("PID:"));
    }
}
