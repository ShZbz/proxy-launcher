//! Headless CLI fast-path execution.
//!
//! Handles instant `--launch-rule` (<5ms, zero Webview) and `--create-shortcut`
//! before Tauri GUI initialization.

use launcher_core::launcher::LaunchError;
use launcher_core::Config;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Resolves configuration file path according to:
/// 1. Explicit path if provided via `--config <path>`
/// 2. `config.json` next to the running executable
/// 3. `config.json` in current working directory
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

/// Headless launch execution with custom I/O writers.
pub fn run_headless_launch_with_io<W1: Write, W2: Write>(
    args: &[String],
    pos: usize,
    stdout: &mut W1,
    stderr: &mut W2,
) -> i32 {
    let rule_id = match args.get(pos + 1) {
        Some(id) if !id.trim().is_empty() => id.trim(),
        _ => {
            let _ = writeln!(stderr, "Error: --launch-rule requires a rule ID or name argument");
            return 1;
        }
    };

    let explicit_config = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|p| args.get(p + 1))
        .map(PathBuf::from);

    let config_path = match resolve_config_path(explicit_config.as_deref()) {
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

    match launcher_core::launcher::launch_rule(&config, rule_id) {
        Ok(pid) => {
            let _ = writeln!(stdout, "Successfully launched rule '{}' (PID: {})", rule_id, pid);
            0
        }
        Err(LaunchError::RuleNotFound(r)) => {
            let _ = writeln!(stderr, "Error: Rule not found: {}", r);
            2
        }
        Err(LaunchError::PortNotFound(p)) => {
            let _ = writeln!(stderr, "Error: Port not found: {}", p);
            3
        }
        Err(err) => {
            let _ = writeln!(stderr, "Error: Launch failed: {}", err);
            1
        }
    }
}

/// Headless create-shortcut execution with custom I/O writers.
pub fn run_headless_create_shortcut_with_io<W1: Write, W2: Write>(
    args: &[String],
    pos: usize,
    stdout: &mut W1,
    stderr: &mut W2,
) -> i32 {
    let rule_id = match args.get(pos + 1) {
        Some(id) if !id.trim().is_empty() => id.trim(),
        _ => {
            let _ = writeln!(stderr, "Error: --create-shortcut requires a rule ID or name argument");
            return 1;
        }
    };

    let explicit_config = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|p| args.get(p + 1))
        .map(PathBuf::from);

    let config_path = match resolve_config_path(explicit_config.as_deref()) {
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

    let cli_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            let _ = writeln!(stderr, "Error determining launcher executable path: {}", e);
            return 1;
        }
    };

    match launcher_core::lnk::create_shortcut_for_rule(&config, rule_id, &cli_exe) {
        Ok(shortcut_path) => {
            let _ = writeln!(stdout, "Shortcut created successfully: {}", shortcut_path.display());
            0
        }
        Err(err) => {
            let _ = writeln!(stderr, "Error creating shortcut: {}", err);
            1
        }
    }
}

pub fn run_headless_launch(args: &[String], pos: usize) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    run_headless_launch_with_io(args, pos, &mut stdout, &mut stderr)
}

pub fn run_headless_create_shortcut(args: &[String], pos: usize) -> i32 {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    run_headless_create_shortcut_with_io(args, pos, &mut stdout, &mut stderr)
}
