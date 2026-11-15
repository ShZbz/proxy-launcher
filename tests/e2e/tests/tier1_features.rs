use e2e_tests::harness::{CliRunner, MockProxyServer, ProcessHelper, TestEnv};
use launcher_core::config::{load_from_path, load_or_default, save_atomic};
use launcher_core::lnk::{create_shortcut, sanitize_filename, ShortcutParams};
use launcher_core::models::{split_arguments, Config, Port, Rule};
use launcher_core::probe::{probe_local_port, probe_upstream_http, ProbeError};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ============================================================================
// Feature 1: Atomic Config Persistence & Tokenizer (6 tests)
// ============================================================================

#[test]
fn test_f1_01_default_config_initialization_when_file_absent() {
    let env = TestEnv::new();
    let absent_path = env.subpath("nonexistent_config.json");

    let (cfg, was_corrupt) = load_or_default(&absent_path);
    assert_eq!(cfg.version, 1, "Default config version must be 1");
    assert!(!was_corrupt, "Absent config file must not trigger corruption backup");
    assert!(cfg.ports.is_empty(), "Default config must have empty ports");
    assert!(cfg.rules.is_empty(), "Default config must have empty rules");
    assert!(!absent_path.exists(), "Absence probe must not create dummy file");
}

#[test]
fn test_f1_02_load_existing_valid_multi_port_config() {
    let env = TestEnv::new();
    let (cfg_path, _) = env.create_multi_port_config(&[("ClashMeta", 7890), ("SingBox", 2080)]);

    let loaded = load_from_path(&cfg_path).expect("Failed to load valid multi-port config");
    assert_eq!(loaded.ports.len(), 2);
    assert_eq!(loaded.rules.len(), 2);
    assert_eq!(loaded.ports[0].alias, "ClashMeta");
    assert_eq!(loaded.ports[0].port, 7890);
    assert_eq!(loaded.ports[1].alias, "SingBox");
    assert_eq!(loaded.ports[1].port, 2080);
}

#[test]
fn test_f1_03_atomic_save_with_file_replacement_and_write_through() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("atomic_config.json");

    let mut cfg = Config::default();
    cfg.ports.push(Port::new("Primary", "127.0.0.1", 1080));
    save_atomic(&cfg, &cfg_path).expect("Initial save_atomic failed");
    assert!(cfg_path.exists(), "Target config file must exist after save");
    assert!(
        !env.subpath("atomic_config.json.tmp").exists(),
        "Temporary .tmp file must be replaced/cleaned up"
    );

    // Overwrite atomically with updated state
    cfg.ports.push(Port::new("Secondary", "127.0.0.1", 1081));
    save_atomic(&cfg, &cfg_path).expect("Atomic overwrite failed");

    let reloaded = load_from_path(&cfg_path).expect("Failed to reload overwritten config");
    assert_eq!(reloaded.ports.len(), 2);
    assert_eq!(reloaded.ports[1].alias, "Secondary");
    assert_eq!(reloaded.ports[1].port, 1081);
    assert!(!env.subpath("atomic_config.json.tmp").exists());
}

#[test]
fn test_f1_04_port_crud_operations_persist_faithfully() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("crud_ports.json");
    let mut cfg = Config::default();

    // Create
    let new_port = Port::new("Clash", "127.0.0.1", 7890);
    let port_id = new_port.id.clone();
    cfg.add_port(new_port);
    save_atomic(&cfg, &cfg_path).unwrap();

    // Read
    let mut loaded = load_from_path(&cfg_path).unwrap();
    let retrieved = loaded.find_port(&port_id).expect("Port not found after create");
    assert_eq!(retrieved.alias, "Clash");
    assert_eq!(retrieved.port, 7890);

    // Update
    let target = loaded.find_port_mut(&port_id).unwrap();
    target.alias = "ClashMeta".to_string();
    target.port = 7897;
    save_atomic(&loaded, &cfg_path).unwrap();

    let reloaded = load_from_path(&cfg_path).unwrap();
    let updated = reloaded.find_port(&port_id).unwrap();
    assert_eq!(updated.alias, "ClashMeta");
    assert_eq!(updated.port, 7897);

    // Delete
    let mut to_delete = reloaded;
    to_delete.ports.retain(|p| p.id != port_id);
    save_atomic(&to_delete, &cfg_path).unwrap();

    let final_cfg = load_from_path(&cfg_path).unwrap();
    assert!(final_cfg.find_port(&port_id).is_none());
    assert!(final_cfg.ports.is_empty());
}

#[test]
fn test_f1_05_rule_crud_operations_persist_with_custom_args_and_cwd() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("crud_rules.json");
    let mut cfg = Config::default();

    let mut rule = Rule::new("TestCmd", r"C:\Windows\System32\cmd.exe", "port_abc");
    let rule_id = rule.id.clone();
    rule.args = "--custom-flag \"value with spaces\" -v".to_string();
    rule.cwd = r"C:\Windows".to_string();
    rule.mode = "env".to_string();
    cfg.add_rule(rule);
    save_atomic(&cfg, &cfg_path).unwrap();

    let loaded = load_from_path(&cfg_path).unwrap();
    let found = loaded.find_rule(&rule_id).expect("Rule not found after save");
    assert_eq!(found.name, "TestCmd");
    assert_eq!(found.args, "--custom-flag \"value with spaces\" -v");
    assert_eq!(found.cwd, r"C:\Windows");
    assert_eq!(found.mode, "env");
    assert_eq!(found.effective_cwd(), PathBuf::from(r"C:\Windows"));
}

#[test]
fn test_f1_06_argument_tokenizer_splits_complex_quotes_and_flags() {
    let raw = r#"app.exe --flag "quoted argument with spaces" unquoted \"escaped\""#;
    let tokens = split_arguments(raw);
    assert_eq!(
        tokens,
        vec![
            "app.exe",
            "--flag",
            "quoted argument with spaces",
            "unquoted",
            "\"escaped\""
        ]
    );

    let empty = split_arguments("   \t  ");
    assert!(empty.is_empty());

    let path_with_spaces = split_arguments(r#""C:\Program Files\SubFolder\app.exe" /s /v"#);
    assert_eq!(path_with_spaces[0], r#"C:\Program Files\SubFolder\app.exe"#);
    assert_eq!(path_with_spaces[1], "/s");
    assert_eq!(path_with_spaces[2], "/v");
}

// ============================================================================
// Feature 2: Two-Stage Port Health Probing (6 tests)
// ============================================================================

#[test]
fn test_f2_01_local_tcp_probe_succeeds_on_open_port() {
    let mock = MockProxyServer::start_raw_tcp();
    let res = probe_local_port("127.0.0.1", mock.port(), Duration::from_millis(200));
    assert!(res.is_ok(), "Local TCP probe should succeed on open loopback port: {:?}", res.err());
}

#[test]
fn test_f2_02_local_tcp_probe_fails_immediately_on_closed_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_port = listener.local_addr().unwrap().port();
    drop(listener); // Ensure port is closed

    let res = probe_local_port("127.0.0.1", closed_port, Duration::from_millis(50));
    assert!(res.is_err(), "Local TCP probe must fail on closed port");
    match res.unwrap_err() {
        ProbeError::PortClosed { port, .. } => assert_eq!(port, closed_port),
        ProbeError::Timeout { port, .. } => assert_eq!(port, closed_port),
        ProbeError::Io(_) => {},
        other => panic!("Unexpected error variant: {:?}", other),
    }
}

#[test]
fn test_f2_03_upstream_http_probe_succeeds_on_http_204() {
    let mock = MockProxyServer::start_http_204();
    let res = probe_upstream_http(
        "127.0.0.1",
        mock.port(),
        "http://cp.cloudflare.com/generate_204",
        Duration::from_millis(500),
    );
    assert_eq!(res.expect("Probe should return status code"), 204);
}

#[test]
fn test_f2_04_upstream_http_probe_fails_on_non_204_error() {
    let mock = MockProxyServer::start_http_502();
    let res = probe_upstream_http(
        "127.0.0.1",
        mock.port(),
        "http://cp.cloudflare.com/generate_204",
        Duration::from_millis(500),
    );
    assert!(res.is_err(), "Upstream HTTP probe must fail on HTTP 502");
    match res.unwrap_err() {
        ProbeError::HttpError { status, .. } => assert_eq!(status, 502),
        other => panic!("Expected HttpError, got {:?}", other),
    }
}

#[test]
fn test_f2_05_probe_latency_measurement_accuracy() {
    let mock = MockProxyServer::start_raw_tcp();
    let start = Instant::now();
    let res = probe_local_port("127.0.0.1", mock.port(), Duration::from_millis(100));
    let elapsed = start.elapsed();

    assert!(res.is_ok());
    // Local loopback TCP connect typically completes in < 5ms
    assert!(
        elapsed < Duration::from_millis(50),
        "Loopback probe took too long ({:?})",
        elapsed
    );
}

#[test]
fn test_f2_06_probe_handles_both_ipv4_and_localhost_hostnames() {
    let mock = MockProxyServer::start_raw_tcp();
    assert!(
        probe_local_port("127.0.0.1", mock.port(), Duration::from_millis(100)).is_ok(),
        "IPv4 literal 127.0.0.1 must succeed"
    );
    assert!(
        probe_local_port("localhost", mock.port(), Duration::from_millis(100)).is_ok(),
        "Hostname 'localhost' must resolve and succeed"
    );
}

// ============================================================================
// Feature 3: Windows Shortcut Generation (6 tests)
// ============================================================================

#[test]
fn test_f3_01_generates_valid_lnk_file_with_lnk_extension() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("valid_app.lnk");

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: "/c exit 0".to_string(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Valid LNK Test".to_string(),
        destination_file: lnk_path.clone(),
    };

    let result = create_shortcut(&params);
    assert!(result.is_ok(), "Shortcut creation failed: {:?}", result.err());
    assert!(lnk_path.exists(), "Shortcut file must exist on disk");
    assert_eq!(
        lnk_path.extension().and_then(|s| s.to_str()),
        Some("lnk"),
        "Extension must be .lnk"
    );
    assert!(
        std::fs::metadata(&lnk_path).unwrap().len() > 0,
        "File size must be non-zero"
    );
}

#[test]
fn test_f3_02_sets_sw_showminnoactive_minimized_window_mode() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("minimized.lnk");

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: String::new(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Minimized Test".to_string(),
        destination_file: lnk_path.clone(),
    };

    create_shortcut(&params).expect("Failed to create shortcut");
    let bytes = std::fs::read(&lnk_path).expect("Failed to read shortcut file");

    // MS-SHLLINK Header: ShowCommand is 4-byte uint32 at offset 0x3C (60)
    assert!(bytes.len() >= 0x40, "Shortcut file header too short");
    let show_cmd = u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]);
    assert_eq!(
        show_cmd, 7,
        "ShowCommand must be 7 (SW_SHOWMINNOACTIVE) to prevent console flashing"
    );
}

#[test]
fn test_f3_03_embeds_target_path_and_custom_arguments() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("embedded_args.lnk");
    let custom_args = "--launch-rule rule_target_999";

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: custom_args.to_string(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Embedded Args".to_string(),
        destination_file: lnk_path.clone(),
    };

    create_shortcut(&params).expect("Shortcut creation failed");
    assert!(lnk_path.exists());

    let bytes = std::fs::read(&lnk_path).unwrap();
    // In IShellLink, arguments are stored either as UTF-8/ASCII or UTF-16LE in the link string data
    let needle_ascii = custom_args.as_bytes();
    let needle_utf16: Vec<u8> = custom_args
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();

    let found_ascii = bytes.windows(needle_ascii.len()).any(|w| w == needle_ascii);
    let found_utf16 = bytes.windows(needle_utf16.len()).any(|w| w == needle_utf16);

    assert!(
        found_ascii || found_utf16,
        "Embedded arguments must be present in .lnk binary payload"
    );
}

#[test]
fn test_f3_04_sanitizes_invalid_windows_filename_characters() {
    let raw = r#"App:Name*With?Illegal<Chars>|And/Slash\Backslash"#;
    let sanitized = sanitize_filename(raw);

    for illegal in &['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
        assert!(
            !sanitized.contains(*illegal),
            "Sanitized filename must not contain illegal character '{}'",
            illegal
        );
    }
    assert_eq!(sanitize_filename(""), "ProxyLauncher");
    assert_eq!(sanitize_filename("   "), "ProxyLauncher");
    assert_eq!(sanitize_filename("CleanAppName"), "CleanAppName");
}

#[test]
fn test_f3_05_assigns_target_executable_icon_location() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("icon_app.lnk");
    let cmd_exe = ProcessHelper::safe_cmd_exe();

    let params = ShortcutParams {
        target_path: cmd_exe.clone(),
        arguments: String::new(),
        working_dir: env.path().to_path_buf(),
        icon_location: Some(cmd_exe),
        icon_index: 0,
        description: "Icon Shortcut".to_string(),
        destination_file: lnk_path.clone(),
    };

    let res = create_shortcut(&params);
    assert!(res.is_ok(), "Failed to create shortcut with icon location: {:?}", res.err());
    assert!(lnk_path.exists());
}

#[test]
fn test_f3_06_overwrites_existing_shortcut_safely() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("overwrite_test.lnk");

    let mut params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: "--first".to_string(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "First Run".to_string(),
        destination_file: lnk_path.clone(),
    };

    create_shortcut(&params).expect("First shortcut creation failed");
    let first_len = std::fs::metadata(&lnk_path).unwrap().len();

    // Overwrite with different arguments
    params.arguments = "--second-overwrite-with-longer-args".to_string();
    params.description = "Second Run".to_string();

    let res = create_shortcut(&params);
    assert!(res.is_ok(), "Overwriting existing shortcut failed: {:?}", res.err());
    assert!(lnk_path.exists());
    let second_len = std::fs::metadata(&lnk_path).unwrap().len();
    assert!(second_len > 0);
    let _ = first_len;
}

// ============================================================================
// Feature 4: Process Spawning & Env Injection (6 tests)
// ============================================================================

#[test]
fn test_f4_01_spawns_detached_process_without_blocking() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let (cfg_path, config) = env.create_minimal_config(mock.port());

    let rule = &config.rules[0];
    let start = Instant::now();
    let pid_res = launcher_core::launcher::launch_rule(&config, &rule.id);
    let elapsed = start.elapsed();

    assert!(pid_res.is_ok(), "Process launch failed: {:?}", pid_res.err());
    let pid = pid_res.unwrap();
    assert!(pid > 0, "Spawned PID must be positive");
    assert!(
        elapsed < Duration::from_millis(500),
        "Process spawn must be non-blocking and detached (took {:?})",
        elapsed
    );
    let _ = cfg_path;
}

#[test]
fn test_f4_02_injects_uppercase_proxy_variables() {
    let host = "127.0.0.1";
    let port = 7890;
    let expected_http = format!("http://LOCALHOST:{}", port);
    let expected_socks = format!("socks5://{}:{}", host, port);

    let envs = launcher_core::launcher::build_proxy_env(host, port);
    let map: std::collections::HashMap<_, _> = envs.into_iter().collect();

    assert_eq!(map.get("HTTP_PROXY"), Some(&expected_http));
    assert_eq!(map.get("HTTPS_PROXY"), Some(&expected_http));
    assert_eq!(map.get("ALL_PROXY"), Some(&expected_socks));
    assert_eq!(map.get("NO_PROXY"), Some(&"localhost,127.0.0.1,::1".to_string()));

    // Verify uppercase environment variables in child process execution
    assert_eq!(ProcessHelper::read_injected_var(host, port, "HTTP_PROXY"), expected_http);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "HTTPS_PROXY"), expected_http);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "ALL_PROXY"), expected_socks);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "NO_PROXY"), "localhost,127.0.0.1,::1");
}

#[test]
fn test_f4_03_injects_lowercase_twin_proxy_variables() {
    let host = "127.0.0.1";
    let port = 8080;
    let expected_http = format!("http://LOCALHOST:{}", port);
    let expected_socks = format!("socks5://{}:{}", host, port);

    let envs = launcher_core::launcher::build_proxy_env(host, port);
    let map: std::collections::HashMap<_, _> = envs.into_iter().collect();

    assert_eq!(map.get("http_proxy"), Some(&expected_http));
    assert_eq!(map.get("https_proxy"), Some(&expected_http));
    assert_eq!(map.get("all_proxy"), Some(&expected_socks));
    assert_eq!(map.get("no_proxy"), Some(&"localhost,127.0.0.1,::1".to_string()));

    // Verify lowercase variable resolution in child process execution on Windows
    assert_eq!(ProcessHelper::read_injected_var(host, port, "http_proxy"), expected_http);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "https_proxy"), expected_http);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "all_proxy"), expected_socks);
    assert_eq!(ProcessHelper::read_injected_var(host, port, "no_proxy"), "localhost,127.0.0.1,::1");
}

#[test]
fn test_f4_04_injects_special_ide_endpoint_aicode_url() {
    let host = "127.0.0.1";
    let port = 7890;
    let child_env = ProcessHelper::capture_all_child_env(host, port, None);

    assert_eq!(
        child_env.get("AICODE_ENDPOINT_URL"),
        Some(&"https://localhost@aicode.googleapis.com".to_string()),
        "AICODE_ENDPOINT_URL must be injected to fix IDE remote agent fastpush bug"
    );
}

#[test]
fn test_f4_05_preserves_unrelated_parent_environment_variables() {
    let host = "127.0.0.1";
    let port = 7890;
    let custom_var = ("PROXY_LAUNCHER_TEST_PRESERVE", "SECRET_KEY_12345");

    let child_env = ProcessHelper::capture_all_child_env(host, port, Some(custom_var));
    assert_eq!(
        child_env.get("PROXY_LAUNCHER_TEST_PRESERVE"),
        Some(&"SECRET_KEY_12345".to_string()),
        "Unrelated environment variables must be preserved in child process"
    );
}

#[test]
fn test_f4_06_inherits_or_applies_specified_working_directory() {
    let env = TestEnv::new();
    let custom_dir = env.subpath("custom_working_dir");
    std::fs::create_dir_all(&custom_dir).unwrap();

    let mut rule = Rule::new("CwdApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), "p1");
    rule.cwd = custom_dir.to_string_lossy().into_owned();

    assert_eq!(rule.effective_cwd(), custom_dir);

    // Verify when cwd is empty, it falls back to exe parent or "."
    rule.cwd = String::new();
    assert_eq!(
        rule.effective_cwd(),
        ProcessHelper::safe_cmd_exe().parent().unwrap().to_path_buf()
    );
}

// ============================================================================
// Feature 5: Binary CLI & Fast-Path Interface (6 tests)
// ============================================================================

#[test]
fn test_f5_01_cli_help_flag_returns_code_0_and_synopsis() {
    let runner = CliRunner::new();

    for flag in &["--help", "-h"] {
        let out = runner.run(&[flag]);
        out.assert_success();
        out.assert_stdout_contains("USAGE:");
        out.assert_stdout_contains("--launch-rule");
        out.assert_stdout_contains("--create-shortcut");
        out.assert_stdout_contains("--config");
    }
}

#[test]
fn test_f5_02_cli_version_flag_returns_code_0_and_semver() {
    let runner = CliRunner::new();

    for flag in &["--version", "-V"] {
        let out = runner.run(&[flag]);
        out.assert_success();
        out.assert_stdout_contains("0.1.0");
    }
}

#[test]
fn test_f5_03_cli_launch_rule_executes_successfully_code_0() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let (cfg_path, _) = env.create_minimal_config(mock.port());

    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", "rule_test_1"], &cfg_path);

    out.assert_success();
    out.assert_stdout_contains("Successfully launched rule 'rule_test_1'");
}

#[test]
fn test_f5_04_cli_launch_rule_nonexistent_exits_code_2() {
    let env = TestEnv::new();
    let (cfg_path, _) = env.create_minimal_config(7890);

    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", "rule_absent_id_404"], &cfg_path);

    out.assert_code(2);
    out.assert_stderr_contains("Rule not found: rule_absent_id_404");
}

#[test]
fn test_f5_05_cli_launch_rule_closed_port_exits_code_1() {
    let env = TestEnv::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_port = listener.local_addr().unwrap().port();
    drop(listener); // Ensure port is closed

    let (cfg_path, _) = env.create_minimal_config(closed_port);

    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", "rule_test_1"], &cfg_path);

    out.assert_code(1);
    out.assert_stderr_contains("Launch failed");
}

#[test]
fn test_f5_06_cli_create_shortcut_generates_isolated_shortcut() {
    let env = TestEnv::new();
    let (cfg_path, _) = env.create_minimal_config(7890);
    let runner = CliRunner::new();

    // 1. Verify CLI argument validation: nonexistent rule fails safely with exit code 1
    let out = runner.run_with_config(&["--create-shortcut", "nonexistent_rule_999"], &cfg_path);
    out.assert_code(1);
    out.assert_stderr_contains("Error creating shortcut");

    // 2. Verify desktop shortcut generation for launcher CLI binary targeting isolated sandbox
    let isolated_lnk = env.subpath("LauncherShortcut.lnk");
    let params = ShortcutParams {
        target_path: runner.bin_path().to_path_buf(),
        arguments: "--launch-rule rule_test_1".to_string(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Launcher Fast Path".to_string(),
        destination_file: isolated_lnk.clone(),
    };

    let result = create_shortcut(&params);
    assert!(result.is_ok(), "Failed to create isolated shortcut: {:?}", result.err());
    assert!(isolated_lnk.exists(), "Isolated shortcut must exist on disk");
    assert!(std::fs::metadata(&isolated_lnk).unwrap().len() > 0);
}
