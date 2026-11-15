use launcher_core::models::Rule;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// Helpers for verifying process detachment, safe binary execution,
/// and environment variable injection.
pub struct ProcessHelper;

impl ProcessHelper {
    /// Returns the absolute path to the safe, standard cmd.exe binary.
    pub fn safe_cmd_exe() -> PathBuf {
        if let Ok(comspec) = std::env::var("COMSPEC") {
            let p = PathBuf::from(comspec);
            if p.exists() {
                return p;
            }
        }
        PathBuf::from(r"C:\Windows\System32\cmd.exe")
    }

    /// Returns standard instant-terminating arguments for cmd.exe.
    pub fn safe_args() -> &'static str {
        "/c exit 0"
    }

    /// Constructs a safe Rule guaranteed to exit immediately with code 0.
    pub fn create_safe_rule(id: impl Into<String>, port_id: impl Into<String>) -> Rule {
        Rule {
            id: id.into(),
            name: "SafeExitApp".to_string(),
            exe: Self::safe_cmd_exe().to_string_lossy().into_owned(),
            port_id: port_id.into(),
            args: Self::safe_args().to_string(),
            cwd: String::new(),
            mode: "both".to_string(),
            flag_template: "--proxy-server={url}".to_string(),
            enabled: true,
            source: "TestHelper".to_string(),
        }
    }

    /// Executes a command directly with injected proxy environment variables,
    /// capturing output from `cmd /c echo %VAR%` to verify child environment.
    pub fn read_injected_var(host: &str, port: u16, var_name: &str) -> String {
        let cmd_exe = Self::safe_cmd_exe();
        let mut cmd = Command::new(&cmd_exe);
        cmd.args(["/c", &format!("echo %{}%", var_name)]);

        launcher_core::launcher::inject_proxy_env(&mut cmd, host, port);

        let output = cmd.output().expect("Failed to execute cmd to check env");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Executes `cmd /c set` to capture all environment variables passed to the child process.
    pub fn capture_all_child_env(
        host: &str,
        port: u16,
        extra_env: Option<(&str, &str)>,
    ) -> HashMap<String, String> {
        let cmd_exe = Self::safe_cmd_exe();
        let mut cmd = Command::new(&cmd_exe);
        cmd.args(["/c", "set"]);

        launcher_core::launcher::inject_proxy_env(&mut cmd, host, port);
        if let Some((k, v)) = extra_env {
            cmd.env(k, v);
        }

        let output = cmd.output().expect("Failed to run cmd set");
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut map = HashMap::new();
        for line in stdout.lines() {
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let val = line[pos + 1..].trim().to_string();
                map.insert(key, val);
            }
        }
        map
    }

    /// Checks if a process with the given PID is currently running using tasklist.
    pub fn is_pid_alive(pid: u32) -> bool {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(&pid.to_string())
        } else {
            false
        }
    }
}
