use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Encapsulates the output of a CLI binary invocation with assertion helpers.
#[derive(Debug, Clone)]
pub struct CliOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    pub fn from_output(output: Output) -> Self {
        Self {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Asserts that the process exited with code 0 (success).
    pub fn assert_success(&self) -> &Self {
        assert_eq!(
            self.code,
            Some(0),
            "Expected exit code 0, got {:?}.\nStdout: {}\nStderr: {}",
            self.code,
            self.stdout,
            self.stderr
        );
        self
    }

    /// Asserts that the process exited with a specific exit code.
    pub fn assert_code(&self, expected: i32) -> &Self {
        assert_eq!(
            self.code,
            Some(expected),
            "Expected exit code {}, got {:?}.\nStdout: {}\nStderr: {}",
            expected,
            self.code,
            self.stdout,
            self.stderr
        );
        self
    }

    /// Asserts that stdout contains the specified substring.
    pub fn assert_stdout_contains(&self, needle: &str) -> &Self {
        assert!(
            self.stdout.contains(needle),
            "Expected stdout to contain '{}'.\nStdout: {}\nStderr: {}",
            needle,
            self.stdout,
            self.stderr
        );
        self
    }

    /// Asserts that stderr contains the specified substring.
    pub fn assert_stderr_contains(&self, needle: &str) -> &Self {
        assert!(
            self.stderr.contains(needle),
            "Expected stderr to contain '{}'.\nStdout: {}\nStderr: {}",
            needle,
            self.stdout,
            self.stderr
        );
        self
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn code(&self) -> Option<i32> {
        self.code
    }
}

/// Helper for discovering and executing the launcher binary in tests.
#[derive(Debug, Clone)]
pub struct CliRunner {
    bin_path: PathBuf,
}

impl CliRunner {
    /// Discovers the compiled binary path, checking PROXY_LAUNCHER_BIN, target directories,
    /// or current executable sibling paths.
    pub fn new() -> Self {
        let bin_path = Self::find_binary();
        Self { bin_path }
    }

    pub fn with_bin(bin_path: PathBuf) -> Self {
        Self { bin_path }
    }

    pub fn bin_path(&self) -> &Path {
        &self.bin_path
    }

    fn find_binary() -> PathBuf {
        // 1. Environment variable override
        if let Ok(path) = std::env::var("PROXY_LAUNCHER_BIN") {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }

        // 2. Relative to CARGO_MANIFEST_DIR (tests/e2e -> workspace root)
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(&manifest_dir);

        let candidates = [
            workspace_root.join("target").join("debug").join("launcher-cli.exe"),
            workspace_root.join("target").join("release").join("launcher-cli.exe"),
            workspace_root.join("target").join("debug").join("proxy-launcher.exe"),
            workspace_root.join("target").join("release").join("proxy-launcher.exe"),
            workspace_root.join("target").join("debug").join("launcher-cli"),
            workspace_root.join("target").join("release").join("launcher-cli"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return candidate.clone();
            }
        }

        // 3. Search near the test executable itself
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                for candidate in &[
                    parent.join("launcher-cli.exe"),
                    parent.join("../launcher-cli.exe"),
                    parent.join("../../launcher-cli.exe"),
                    parent.join("launcher-cli"),
                ] {
                    if candidate.exists() {
                        return candidate.clone();
                    }
                }
            }
        }

        panic!(
            "Could not locate launcher executable. Checked candidates relative to {}",
            workspace_root.display()
        );
    }

    /// Executes the binary with the given arguments.
    pub fn run(&self, args: &[&str]) -> CliOutput {
        let output = Command::new(&self.bin_path)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("Failed to run binary {}: {}", self.bin_path.display(), e));
        CliOutput::from_output(output)
    }

    /// Executes the binary with args plus `--config <config_path>`.
    pub fn run_with_config(&self, args: &[&str], config_path: &Path) -> CliOutput {
        let mut full_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        full_args.push("--config".to_string());
        full_args.push(config_path.to_string_lossy().into_owned());

        let output = Command::new(&self.bin_path)
            .args(&full_args)
            .output()
            .unwrap_or_else(|e| panic!("Failed to run binary {}: {}", self.bin_path.display(), e));
        CliOutput::from_output(output)
    }

    /// Executes the binary inside a specific current working directory.
    pub fn run_in_dir(&self, args: &[&str], cwd: &Path) -> CliOutput {
        let output = Command::new(&self.bin_path)
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap_or_else(|e| panic!("Failed to run binary {}: {}", self.bin_path.display(), e));
        CliOutput::from_output(output)
    }

    /// Executes the binary with specific environment variables set.
    pub fn run_with_env(&self, args: &[&str], envs: &[(&str, &str)]) -> CliOutput {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("Failed to run binary {}: {}", self.bin_path.display(), e));
        CliOutput::from_output(output)
    }
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}
