use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn get_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_launcher-cli"))
}

fn write_temp_config(path: &Path, port: u16) {
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
fn test_cli_binary_help() {
    let bin = get_bin_path();

    for flag in &["--help", "-h"] {
        let output = Command::new(&bin)
            .arg(flag)
            .output()
            .expect("Failed to execute launcher-cli");

        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("USAGE:"));
        assert!(stdout.contains("--launch-rule"));
        assert!(stdout.contains("--create-shortcut"));
    }
}

#[test]
fn test_cli_binary_version() {
    let bin = get_bin_path();

    for flag in &["--version", "-V"] {
        let output = Command::new(&bin)
            .arg(flag)
            .output()
            .expect("Failed to execute launcher-cli");

        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "proxy-launcher-cli 0.1.0");
    }
}

#[test]
fn test_cli_binary_missing_args() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .output()
        .expect("Failed to execute launcher-cli");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Missing required argument"));
    assert!(stderr.contains("USAGE:"));
}

#[test]
fn test_cli_binary_unknown_args() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("--unrecognized-custom-flag")
        .output()
        .expect("Failed to execute launcher-cli");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown or unrecognized argument(s)"));
}

#[test]
fn test_cli_binary_rule_not_found_exit_code_2() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_bin_rule_not_found.json");
    write_temp_config(&cfg_path, 59990);

    let output = Command::new(&bin)
        .arg("--config")
        .arg(&cfg_path)
        .arg("--launch-rule")
        .arg("non_existent_rule_999")
        .output()
        .expect("Failed to execute launcher-cli");

    let _ = std::fs::remove_file(&cfg_path);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Rule not found: non_existent_rule_999"));
}

#[test]
fn test_cli_binary_port_not_found_exit_code_3() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_bin_port_not_found.json");
    write_temp_config(&cfg_path, 59990);

    let output = Command::new(&bin)
        .arg("--config")
        .arg(&cfg_path)
        .arg("--launch-rule")
        .arg("rule_missing_port")
        .output()
        .expect("Failed to execute launcher-cli");

    let _ = std::fs::remove_file(&cfg_path);

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Port not found: nonexistent_port"));
}

#[test]
fn test_cli_binary_closed_port_exit_code_1() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_bin_closed_port.json");
    write_temp_config(&cfg_path, 59991);

    let output = Command::new(&bin)
        .arg("--config")
        .arg(&cfg_path)
        .arg("--launch-rule")
        .arg("rule_valid")
        .output()
        .expect("Failed to execute launcher-cli");

    let _ = std::fs::remove_file(&cfg_path);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Launch failed"));
    assert!(stderr.contains("59991"));
}

#[test]
fn test_cli_binary_create_shortcut_rule_not_found_exit_code_1() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_bin_shortcut_rule_not_found.json");
    write_temp_config(&cfg_path, 59990);

    let output = Command::new(&bin)
        .arg("--config")
        .arg(&cfg_path)
        .arg("--create-shortcut")
        .arg("non_existent_rule_999")
        .output()
        .expect("Failed to execute launcher-cli");

    let _ = std::fs::remove_file(&cfg_path);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error creating shortcut"));
}

#[test]
fn test_cli_binary_launch_success_exit_code_0() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_bin_launch_success.json");
    write_temp_config(&cfg_path, port);

    let output = Command::new(&bin)
        .arg("--config")
        .arg(&cfg_path)
        .arg("--launch-rule")
        .arg("rule_valid")
        .output()
        .expect("Failed to execute launcher-cli");

    let _ = std::fs::remove_file(&cfg_path);
    drop(listener);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Successfully launched rule 'rule_valid'"));
    assert!(stdout.contains("PID:"));
}
