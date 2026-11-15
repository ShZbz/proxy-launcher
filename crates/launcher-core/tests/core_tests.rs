//! Comprehensive integration and unit tests for launcher-core.

use launcher_core::*;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_models_json_roundtrip() {
    let raw_json = r#"{
      "version": 1,
      "test_url": "http://cp.cloudflare.com/generate_204",
      "ip_url": "https://api.ipify.org",
      "show_exit_ip": true,
      "ports": [
        {
          "alias": "clash",
          "host": "127.0.0.1",
          "port": 49991,
          "id": "ce669f4219ed"
        }
      ],
      "rules": [
        {
          "name": "Antigravity",
          "exe": "C:\\Program Files\\app\\app.exe",
          "port_id": "ce669f4219ed",
          "args": "--proxy-bypass-list=<-loopback>",
          "cwd": "C:\\Program Files\\app",
          "mode": "both",
          "flag_template": "--proxy-server={url}",
          "enabled": true,
          "source": "GetPath",
          "id": "5032cba68f4e"
        }
      ]
    }"#;

    let config: Config = serde_json::from_str(raw_json).expect("Failed to deserialize config JSON");
    assert_eq!(config.version, 1);
    assert_eq!(config.ports.len(), 1);
    assert_eq!(config.rules.len(), 1);

    let port = &config.ports[0];
    assert_eq!(port.alias, "clash");
    assert_eq!(port.port, 49991);
    assert_eq!(port.url(), "http://127.0.0.1:49991");
    assert_eq!(port.socks_url(), "socks5://127.0.0.1:49991");

    let rule = &config.rules[0];
    assert_eq!(rule.name, "Antigravity");
    assert_eq!(rule.id, "5032cba68f4e");
    assert_eq!(rule.mode, "both");
    assert_eq!(rule.formatted_flag(port), "--proxy-server=http://127.0.0.1:49991");

    // Re-serialize and deserialize back
    let serialized = serde_json::to_string(&config).expect("Serialization failed");
    let reloaded: Config = serde_json::from_str(&serialized).expect("Re-deserialization failed");
    assert_eq!(config, reloaded);
}

#[test]
fn test_models_lookup_methods() {
    let mut config = Config::default();
    let port = Port {
        id: "port_123".to_string(),
        alias: "primary".to_string(),
        host: "127.0.0.1".to_string(),
        port: 7890,
    };
    config.add_port(port.clone());

    let rule = Rule {
        id: "rule_abc".to_string(),
        name: "TestBrowser".to_string(),
        exe: "C:\\browser\\browser.exe".to_string(),
        port_id: "port_123".to_string(),
        args: "".to_string(),
        cwd: "".to_string(),
        mode: "both".to_string(),
        flag_template: DEFAULT_FLAG_TEMPLATE.to_string(),
        enabled: true,
        source: "".to_string(),
    };
    config.add_rule(rule);

    // Exact ID lookup
    assert!(config.find_rule("rule_abc").is_some());
    // Name lookup (case-insensitive and trimmed)
    assert!(config.find_rule("  testbrowser  ").is_some());
    // Port lookup by ID
    assert!(config.find_port("port_123").is_some());
    // Port lookup by alias
    assert!(config.find_port("PRIMARY").is_some());

    // Active port for rule
    let found_rule = config.find_rule("rule_abc").unwrap();
    let active_port = config.active_port_for_rule(found_rule).unwrap();
    assert_eq!(active_port.port, 7890);
}

#[test]
fn test_split_arguments() {
    let args = r#"--flag1 --flag2="value with spaces" --flag3 'single' normal"#;
    let tokens = split_arguments(args);
    assert_eq!(
        tokens,
        vec![
            "--flag1",
            "--flag2=value with spaces",
            "--flag3",
            "'single'",
            "normal"
        ]
    );

    let empty = split_arguments("   ");
    assert!(empty.is_empty());
}

#[test]
fn test_atomic_config_save_and_load() {
    let temp_dir = std::env::temp_dir().join(format!("launcher_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let cfg_path = temp_dir.join("test_config.json");

    let mut cfg = Config::default();
    cfg.ports.push(Port::new("dev_proxy", "127.0.0.1", 1080));
    cfg.rules.push(Rule::new("DemoApp", "C:\\demo.exe", &cfg.ports[0].id));

    // Initial save
    save_atomic(&cfg, &cfg_path).expect("Atomic save failed");
    assert!(cfg_path.exists());
    // Temp file should not remain after successful atomic save
    let tmp_path = PathBuf::from(format!("{}.tmp", cfg_path.to_string_lossy()));
    assert!(!tmp_path.exists());

    // Load back
    let loaded = load_from_path(&cfg_path).expect("Load failed");
    assert_eq!(loaded.ports.len(), 1);
    assert_eq!(loaded.ports[0].alias, "dev_proxy");
    assert_eq!(loaded.rules.len(), 1);
    assert_eq!(loaded.rules[0].name, "DemoApp");

    // Overwrite existing atomically
    cfg.rules[0].name = "DemoAppUpdated".to_string();
    save_atomic(&cfg, &cfg_path).expect("Atomic overwrite failed");
    let reloaded = load_from_path(&cfg_path).expect("Reload failed");
    assert_eq!(reloaded.rules[0].name, "DemoAppUpdated");

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_load_or_default_corrupt_file() {
    let temp_dir = std::env::temp_dir().join(format!("launcher_corrupt_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let cfg_path = temp_dir.join("corrupted.json");

    // Write malformed JSON
    fs::write(&cfg_path, "{ broken json content").unwrap();

    let (cfg, was_corrupt) = load_or_default(&cfg_path);
    assert!(was_corrupt);
    assert_eq!(cfg.version, CONFIG_VERSION);

    // Verify .bad backup was created
    let bad_path = PathBuf::from(format!("{}.bad", cfg_path.to_string_lossy()));
    assert!(bad_path.exists());

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_probe_local_port_open_and_closed() {
    // 1. Test open port: bind a temporary TCP listener on loopback
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let bound_port = listener.local_addr().unwrap().port();

    let probe_res = probe_local_port("127.0.0.1", bound_port, Duration::from_millis(100));
    assert!(probe_res.is_ok(), "Open port probe should succeed");

    // Drop listener to close port
    drop(listener);

    // 2. Test closed port: connecting should fail with PortClosed
    let closed_res = probe_local_port("127.0.0.1", bound_port, Duration::from_millis(50));
    assert!(closed_res.is_err(), "Closed port probe should fail");
    match closed_res.unwrap_err() {
        ProbeError::PortClosed { port, .. } | ProbeError::Timeout { port, .. } => {
            assert_eq!(port, bound_port);
        }
        other => panic!("Expected PortClosed or Timeout error, got {:?}", other),
    }
}

#[test]
fn test_build_proxy_env() {
    let env = build_proxy_env("127.0.0.1", 8080);
    let map: std::collections::HashMap<&str, String> = env.into_iter().collect();

    assert_eq!(map.get("HTTP_PROXY").unwrap(), "http://LOCALHOST:8080");
    assert_eq!(map.get("HTTPS_PROXY").unwrap(), "http://LOCALHOST:8080");
    assert_eq!(map.get("ALL_PROXY").unwrap(), "socks5://127.0.0.1:8080");
    assert_eq!(map.get("NO_PROXY").unwrap(), "localhost,127.0.0.1,::1");

    assert_eq!(map.get("http_proxy").unwrap(), "http://LOCALHOST:8080");
    assert_eq!(map.get("https_proxy").unwrap(), "http://LOCALHOST:8080");
    assert_eq!(map.get("all_proxy").unwrap(), "socks5://127.0.0.1:8080");
    assert_eq!(map.get("no_proxy").unwrap(), "localhost,127.0.0.1,::1");

    assert_eq!(
        map.get("AICODE_ENDPOINT_URL").unwrap(),
        "https://localhost@aicode.googleapis.com"
    );

    // Non-loopback hosts must remain unchanged
    let remote_env: std::collections::HashMap<&str, String> =
        build_proxy_env("192.168.1.10", 8080).into_iter().collect();
    assert_eq!(remote_env.get("HTTP_PROXY").unwrap(), "http://192.168.1.10:8080");
    assert_eq!(remote_env.get("HTTPS_PROXY").unwrap(), "http://192.168.1.10:8080");
}

#[test]
fn test_prepare_command_validation() {
    let port = Port::new("test", "127.0.0.1", 1080);

    // 1. Non-existent target should fail with TargetNotFound
    let non_existent_rule = Rule::new("Missing", "C:\\non_existent_exe_12345.exe", &port.id);
    let err = prepare_command(&non_existent_rule, &port).unwrap_err();
    match err {
        LaunchError::TargetNotFound(_) => {}
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }

    // 2. UWP application should fail with UwpNotSupported
    let uwp_rule = Rule::new("UWPApp", "shell:AppsFolder\\Microsoft.WindowsStore_8wekyb3d8bbwe!App", &port.id);
    let err = prepare_command(&uwp_rule, &port).unwrap_err();
    match err {
        LaunchError::UwpNotSupported => {}
        other => panic!("Expected UwpNotSupported, got {:?}", other),
    }
}

#[test]
fn test_sanitize_filename() {
    assert_eq!(sanitize_filename("Antigravity: <Fast> *Mode*?"), "Antigravity_ _Fast_ _Mode__");
    assert_eq!(sanitize_filename("Normal Name"), "Normal Name");
    assert_eq!(sanitize_filename(""), "ProxyLauncher");
}

#[test]
fn test_shortcut_creation_flow() {
    let temp_dir = std::env::temp_dir().join(format!("launcher_lnk_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let lnk_path = temp_dir.join("test_app.lnk");

    let params = ShortcutParams {
        target_path: PathBuf::from("cmd.exe"),
        arguments: "--launch-rule \"test_id\"".to_string(),
        working_dir: temp_dir.clone(),
        icon_location: None,
        icon_index: 0,
        description: "Test Shortcut".to_string(),
        destination_file: lnk_path.clone(),
    };

    let result = create_shortcut(&params);
    assert!(result.is_ok(), "Shortcut creation should succeed: {:?}", result.err());
    assert!(lnk_path.exists(), "Shortcut file should exist on disk");

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_launch_rule_detached_success() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let bound_port = listener.local_addr().unwrap().port();

    let mut config = Config::default();
    let port = Port::new("live_proxy", "127.0.0.1", bound_port);
    let rule = Rule {
        id: "cmd_rule".to_string(),
        name: "CmdTest".to_string(),
        exe: "C:\\Windows\\System32\\cmd.exe".to_string(),
        port_id: port.id.clone(),
        args: "/c exit 0".to_string(),
        cwd: "C:\\Windows\\System32".to_string(),
        mode: "both".to_string(),
        flag_template: DEFAULT_FLAG_TEMPLATE.to_string(),
        enabled: true,
        source: "".to_string(),
    };
    config.add_port(port);
    config.add_rule(rule);

    let launch_res = launch_rule(&config, "cmd_rule");
    assert!(
        launch_res.is_ok(),
        "Detached process launch should succeed: {:?}",
        launch_res.err()
    );
    let pid = launch_res.unwrap();
    assert!(pid > 0, "Spawned process should return a valid PID");

    drop(listener);
}

#[test]
fn test_launch_rule_closed_port_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let bound_port = listener.local_addr().unwrap().port();
    drop(listener); // immediately close port

    let mut config = Config::default();
    let port = Port::new("closed_proxy", "127.0.0.1", bound_port);
    let rule = Rule::new("CmdTest", "C:\\Windows\\System32\\cmd.exe", &port.id);
    config.add_port(port);
    config.add_rule(rule.clone());

    let launch_res = launch_rule(&config, &rule.id);
    assert!(launch_res.is_err(), "Launch should fail when port is closed");
    match launch_res.unwrap_err() {
        LaunchError::PortUnreachable { port: p, .. } => assert_eq!(p, bound_port),
        other => panic!("Expected PortUnreachable, got {:?}", other),
    }
}

#[test]
fn test_probe_upstream_http_mock_server() {
    use std::io::Write;
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock upstream port");
    let bound_port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let response = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let status = probe_upstream_http(
        "127.0.0.1",
        bound_port,
        "http://cp.cloudflare.com/generate_204",
        Duration::from_millis(500),
    )
    .expect("Upstream probe to mock server should succeed");

    assert_eq!(status, 204);
    let _ = handle.join();
}

#[test]
fn test_effective_cwd_and_flag_formatting() {
    let rule_with_cwd = Rule {
        exe: "C:\\apps\\test.exe".to_string(),
        cwd: "C:\\custom\\cwd".to_string(),
        ..Default::default()
    };
    assert_eq!(rule_with_cwd.effective_cwd(), PathBuf::from("C:\\custom\\cwd"));

    let rule_fallback_cwd = Rule {
        exe: "C:\\apps\\test.exe".to_string(),
        cwd: "".to_string(),
        ..Default::default()
    };
    assert_eq!(rule_fallback_cwd.effective_cwd(), PathBuf::from("C:\\apps"));

    let port = Port::new("p", "127.0.0.1", 8899);
    let rule_custom_flag = Rule {
        flag_template: "--custom-proxy={url}".to_string(),
        ..Default::default()
    };
    assert_eq!(
        rule_custom_flag.formatted_flag(&port),
        "--custom-proxy=http://127.0.0.1:8899"
    );
}


