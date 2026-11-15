use launcher_core::models::{Config, Port, Rule};
use proxy_launcher_lib::commands::*;
use proxy_launcher_lib::lnk_parser::resolve_path;
use std::fs;
use std::net::TcpListener;

#[test]
fn test_get_and_save_config_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let state = AppState::with_config_path(config_path.clone());

    // When config file doesn't exist yet, get_config returns default
    let cfg = get_config_internal(&state).unwrap();
    assert_eq!(cfg.version, 1);
    assert!(cfg.ports.is_empty());
    assert!(cfg.rules.is_empty());

    // Add a port and rule and save
    let mut updated_cfg = cfg;
    updated_cfg.ports.push(Port {
        id: "p1".to_string(),
        alias: "test_proxy".to_string(),
        host: "127.0.0.1".to_string(),
        port: 7890,
    });
    updated_cfg.rules.push(Rule {
        id: "r1".to_string(),
        name: "TestApp".to_string(),
        exe: "C:\\Windows\\notepad.exe".to_string(),
        port_id: "p1".to_string(),
        args: "--debug".to_string(),
        cwd: "C:\\Windows".to_string(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "EXE".to_string(),
    });

    save_config_internal(updated_cfg.clone(), &state).unwrap();
    assert!(config_path.exists());

    // Reload from state and assert equality
    let reloaded = get_config_internal(&state).unwrap();
    assert_eq!(reloaded.ports.len(), 1);
    assert_eq!(reloaded.ports[0].alias, "test_proxy");
    assert_eq!(reloaded.rules.len(), 1);
    assert_eq!(reloaded.rules[0].name, "TestApp");
}

#[test]
fn test_probe_port_sync_closed_port() {
    let port = Port {
        id: "closed_port".to_string(),
        alias: "closed".to_string(),
        host: "127.0.0.1".to_string(),
        port: 59999,
    };

    let result = probe_port_sync(&port, None);
    assert!(!result.ok);
    assert_eq!(result.reason, "PORT_CLOSED");
    assert_eq!(result.reason_text, "端口未监听（代理没开？）");
}

#[test]
fn test_probe_port_sync_open_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port_num = listener.local_addr().unwrap().port();

    let port = Port {
        id: "open_port".to_string(),
        alias: "open".to_string(),
        host: "127.0.0.1".to_string(),
        port: port_num,
    };

    let result = probe_port_sync(&port, None);
    assert!(result.reason == "HTTP_FAILED" || result.reason == "OK");
    assert_ne!(result.reason, "PORT_CLOSED");
}

#[test]
fn test_resolve_dropped_path_direct_executable() {
    let temp_dir = tempfile::tempdir().unwrap();
    let exe_path = temp_dir.path().join("fake_app.exe");
    fs::write(&exe_path, b"dummy exe").unwrap();

    let resolved = resolve_path(&exe_path.to_string_lossy()).unwrap();
    assert_eq!(resolved.name, "fake_app");
    assert_eq!(resolved.kind, "exe");
    assert_eq!(resolved.source, "EXE");
    assert!(!resolved.needs_elevation);
}

#[test]
fn test_resolve_dropped_path_bat_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let bat_path = temp_dir.path().join("run_script.bat");
    fs::write(&bat_path, b"@echo off\n").unwrap();

    let resolved = resolve_path(&bat_path.to_string_lossy()).unwrap();
    assert_eq!(resolved.name, "run_script");
    assert_eq!(resolved.kind, "bat");
    assert_eq!(resolved.source, "EXE");
}

#[test]
fn test_resolve_dropped_path_rejects_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let res = resolve_path(&temp_dir.path().to_string_lossy());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("目录"));
}

#[test]
fn test_resolve_dropped_path_rejects_unsupported_extensions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let txt_path = temp_dir.path().join("test.txt");
    fs::write(&txt_path, b"hello").unwrap();

    let res = resolve_path(&txt_path.to_string_lossy());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("不支持的文件类型"));

    let url_path = temp_dir.path().join("link.url");
    fs::write(&url_path, b"[InternetShortcut]").unwrap();
    let res = resolve_path(&url_path.to_string_lossy());
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("不支持网页快捷方式"));
}

#[test]
fn test_launch_rule_rule_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let state = AppState::with_config_path(config_path);

    let res = launch_rule_internal("non_existent_rule", &state);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("RuleNotFound") || err.contains("未找到"));
}

#[test]
fn test_launch_terminal_port_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let state = AppState::with_config_path(config_path);

    let res = launch_terminal_internal("non_existent_port", "cmd", &state);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("未找到指定的代理端口配置"));
}

#[test]
fn test_launch_terminal_unsupported_shell() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let state = AppState::with_config_path(config_path);

    let mut cfg = Config::default();
    cfg.ports.push(Port {
        id: "p1".to_string(),
        alias: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 7890,
    });
    save_config_internal(cfg, &state).unwrap();

    let res = launch_terminal_internal("p1", "bash_invalid", &state);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("不支持的终端类型"));
}
