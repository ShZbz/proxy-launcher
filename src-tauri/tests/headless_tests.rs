use launcher_core::models::{Config, Port, Rule};
use proxy_launcher_lib::headless::{
    run_headless_create_shortcut_with_io, run_headless_launch_with_io,
};
use std::fs;
use std::net::TcpListener;

#[test]
fn test_headless_launch_missing_rule_id() {
    let args = vec!["proxy-launcher.exe".to_string(), "--launch-rule".to_string()];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_launch_with_io(&args, 1, &mut stdout, &mut stderr);
    assert_eq!(code, 1);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("requires a rule ID or name"));
}

#[test]
fn test_headless_launch_rule_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let cfg = Config::default();
    let json = serde_json::to_string(&cfg).unwrap();
    fs::write(&config_path, json).unwrap();

    let args = vec![
        "proxy-launcher.exe".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().to_string(),
        "--launch-rule".to_string(),
        "missing_rule_id".to_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_launch_with_io(&args, 3, &mut stdout, &mut stderr);
    assert_eq!(code, 2);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("Rule not found"));
}

#[test]
fn test_headless_launch_port_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut cfg = Config::default();
    cfg.rules.push(Rule {
        id: "r1".to_string(),
        name: "App".to_string(),
        exe: "C:\\Windows\\notepad.exe".to_string(),
        port_id: "non_existent_port".to_string(),
        args: "".to_string(),
        cwd: "".to_string(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "".to_string(),
    });
    let json = serde_json::to_string(&cfg).unwrap();
    fs::write(&config_path, json).unwrap();

    let args = vec![
        "proxy-launcher.exe".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().to_string(),
        "--launch-rule".to_string(),
        "r1".to_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_launch_with_io(&args, 3, &mut stdout, &mut stderr);
    assert_eq!(code, 3);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("Port not found"));
}

#[test]
fn test_headless_launch_closed_port_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut cfg = Config::default();
    cfg.ports.push(Port {
        id: "p1".to_string(),
        alias: "closed".to_string(),
        host: "127.0.0.1".to_string(),
        port: 59998,
    });
    cfg.rules.push(Rule {
        id: "r1".to_string(),
        name: "App".to_string(),
        exe: "C:\\Windows\\notepad.exe".to_string(),
        port_id: "p1".to_string(),
        args: "".to_string(),
        cwd: "".to_string(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "".to_string(),
    });
    let json = serde_json::to_string(&cfg).unwrap();
    fs::write(&config_path, json).unwrap();

    let args = vec![
        "proxy-launcher.exe".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().to_string(),
        "--launch-rule".to_string(),
        "r1".to_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_launch_with_io(&args, 3, &mut stdout, &mut stderr);
    assert_eq!(code, 1);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("Launch failed"));
}

#[test]
fn test_headless_launch_success_with_open_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port_num = listener.local_addr().unwrap().port();

    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut cfg = Config::default();
    cfg.ports.push(Port {
        id: "p_open".to_string(),
        alias: "open".to_string(),
        host: "127.0.0.1".to_string(),
        port: port_num,
    });

    let safe_cmd = if cfg!(windows) {
        "C:\\Windows\\System32\\cmd.exe"
    } else {
        "/bin/sh"
    };

    cfg.rules.push(Rule {
        id: "r_open".to_string(),
        name: "TestCmd".to_string(),
        exe: safe_cmd.to_string(),
        port_id: "p_open".to_string(),
        args: if cfg!(windows) { "/c exit 0" } else { "-c exit 0" }.to_string(),
        cwd: "".to_string(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "".to_string(),
    });

    let json = serde_json::to_string(&cfg).unwrap();
    fs::write(&config_path, json).unwrap();

    let args = vec![
        "proxy-launcher.exe".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().to_string(),
        "--launch-rule".to_string(),
        "r_open".to_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_launch_with_io(&args, 3, &mut stdout, &mut stderr);
    assert_eq!(code, 0);
    let out = String::from_utf8_lossy(&stdout);
    assert!(out.contains("Successfully launched rule 'r_open'"));
}

#[test]
fn test_headless_create_shortcut_missing_rule_id() {
    let args = vec!["proxy-launcher.exe".to_string(), "--create-shortcut".to_string()];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_headless_create_shortcut_with_io(&args, 1, &mut stdout, &mut stderr);
    assert_eq!(code, 1);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("requires a rule ID or name"));
}
