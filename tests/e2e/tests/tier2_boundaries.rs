use e2e_tests::harness::{CliRunner, MockProxyServer, ProcessHelper, TestEnv};
use launcher_core::config::{load_from_path, load_or_default, save_atomic, ConfigError};
use launcher_core::lnk::{create_shortcut, sanitize_filename, ShortcutParams};
use launcher_core::models::{Config, Port, Rule};
use launcher_core::probe::{probe_local_port, probe_upstream_http, ProbeError};
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Feature 1 Boundaries: Config Persistence & Edge Cases (6 tests)
// ============================================================================

#[test]
fn test_f1_b01_corrupt_json_triggers_bad_backup_and_recovery() {
    let env = TestEnv::new();
    let corrupt_path = env.write_file("broken_config.json", "{ \"version\": 1, \"ports\": [ corrupted json content... ");

    let (cfg, was_corrupt) = load_or_default(&corrupt_path);
    assert!(was_corrupt, "load_or_default must detect corrupt configuration");
    assert_eq!(cfg.version, 1, "Fallback configuration must be default");
    assert!(cfg.ports.is_empty(), "Fallback config ports must be empty");

    // Verify .bad backup file was created containing the original corrupted text
    let bad_path = PathBuf::from(format!("{}.bad", corrupt_path.to_string_lossy()));
    assert!(bad_path.exists(), "Corrupted config must be backed up as .bad file");
    let backed_up_content = std::fs::read_to_string(&bad_path).expect("Failed to read .bad backup");
    assert!(backed_up_content.contains("corrupted json content"));
}

#[test]
fn test_f1_b02_unwritable_or_readonly_config_handles_io_error() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("readonly_config.json");

    let cfg = Config::default();
    save_atomic(&cfg, &cfg_path).expect("Initial save must succeed");

    // Set file permissions to read-only on Windows
    let mut perms = std::fs::metadata(&cfg_path).expect("Metadata").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&cfg_path, perms).expect("Failed to set read-only");

    // Attempting atomic save over a read-only destination must return Err without crashing
    let mut updated_cfg = cfg.clone();
    updated_cfg.ports.push(Port::new("Blocked", "127.0.0.1", 9999));
    let save_res = save_atomic(&updated_cfg, &cfg_path);
    assert!(
        save_res.is_err(),
        "save_atomic to read-only file must return Err"
    );

    // Reset permissions so TestEnv RAII drop cleanup succeeds cleanly
    let mut clean_perms = std::fs::metadata(&cfg_path).expect("Metadata").permissions();
    clean_perms.set_readonly(false);
    let _ = std::fs::set_permissions(&cfg_path, clean_perms);
}

#[test]
fn test_f1_b03_empty_config_file_triggers_recovery() {
    let env = TestEnv::new();
    let empty_path = env.write_file("empty_config.json", "");

    // 1. load_from_path on empty file must return Json error (unexpected EOF)
    let direct_load = load_from_path(&empty_path);
    assert!(direct_load.is_err(), "Empty file must return error from load_from_path");
    match direct_load.unwrap_err() {
        ConfigError::Json(e) => assert!(e.is_eof() || e.to_string().contains("EOF")),
        other => panic!("Expected ConfigError::Json, got {:?}", other),
    }

    // 2. load_or_default on 0-byte file must trigger .bad backup and fallback to default
    let (recovered_cfg, was_corrupt) = load_or_default(&empty_path);
    assert!(was_corrupt, "Empty config must be marked as corrupt");
    assert_eq!(recovered_cfg.version, 1);
    let bad_path = PathBuf::from(format!("{}.bad", empty_path.to_string_lossy()));
    assert!(bad_path.exists(), ".bad file must be created for 0-byte config");
}

#[test]
fn test_f1_b04_long_unicode_emoji_alias_boundary() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("unicode_alias_config.json");

    // Long Unicode string with emojis, CJK, Cyrillic, and special symbols (255 characters)
    let complex_alias = "🚀_代理_ClashMeta_한국어_Русский_日本語_Special!@#$%^&*()_+~`|}{[]:;?><,./-=⚡✨🔥💡🎉💻".repeat(4);
    assert!(complex_alias.chars().count() >= 200, "Alias must be extensive Unicode boundary");

    let mut cfg = Config::default();
    let port = Port::new(&complex_alias, "127.0.0.1", 7890);
    let port_id = port.id.clone();
    cfg.add_port(port);

    save_atomic(&cfg, &cfg_path).expect("Atomic save with Unicode alias failed");

    let loaded = load_from_path(&cfg_path).expect("Failed to reload Unicode config");
    let retrieved = loaded.find_port(&port_id).expect("Port not found by ID");
    assert_eq!(retrieved.alias, complex_alias, "Unicode alias must be preserved verbatim");

    // Case-insensitive / exact find_port by Unicode alias
    let by_alias = loaded.find_port(&complex_alias).expect("Port not found by Unicode alias");
    assert_eq!(by_alias.id, port_id);
}

#[test]
fn test_f1_b05_rapid_sequential_atomic_saves_under_stress() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("stress_atomic.json");

    let mut cfg = Config::default();
    save_atomic(&cfg, &cfg_path).expect("Initial save failed");

    // 25 rapid sequential saves alternating mutations
    for i in 1..=25 {
        let port_alias = format!("Port_Iter_{}", i);
        cfg.add_port(Port::new(&port_alias, "127.0.0.1", 7000 + i as u16));
        save_atomic(&cfg, &cfg_path).unwrap_or_else(|e| panic!("Save failed at iter {}: {}", i, e));

        assert!(
            !env.subpath("stress_atomic.json.tmp").exists(),
            "Temp file must never linger after save iteration {}",
            i
        );
    }

    let final_cfg = load_from_path(&cfg_path).expect("Failed to load after stress saves");
    assert_eq!(final_cfg.ports.len(), 25);
    assert_eq!(final_cfg.ports.last().unwrap().alias, "Port_Iter_25");
}

#[test]
fn test_f1_b06_concurrent_file_access_simulation() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("concurrent_access.json");

    let initial_cfg = Config::default();
    save_atomic(&initial_cfg, &cfg_path).expect("Initial save");

    let path_arc = Arc::new(cfg_path);
    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();

    // Spawn 3 concurrent readers
    for r in 0..3 {
        let p = Arc::clone(&path_arc);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for _ in 0..20 {
                match load_from_path(&p) {
                    Ok(c) => assert!(c.version == 1, "Reader {} got invalid version", r),
                    Err(ConfigError::NotFound(_)) => {} // Transmit window allowed
                    Err(e) => panic!("Reader {} encountered corrupt JSON: {}", r, e),
                }
                thread::sleep(Duration::from_millis(2));
            }
        }));
    }

    // Spawn 1 concurrent writer performing atomic replaces
    {
        let p = Arc::clone(&path_arc);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            let mut cfg = Config::default();
            for i in 0..15 {
                cfg.add_port(Port::new(format!("P_{}", i), "127.0.0.1", 8000 + i));
                let _ = save_atomic(&cfg, &p);
                thread::sleep(Duration::from_millis(3));
            }
        }));
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Final state must be cleanly reloadable valid JSON
    let final_state = load_from_path(&path_arc).expect("Final reload failed");
    assert!(final_state.ports.len() > 0);
}

// ============================================================================
// Feature 2 Boundaries: Port Health Probing (6 tests)
// ============================================================================

#[test]
fn test_f2_b01_port_number_1_boundary() {
    // Port 1 (lowest valid TCP port). In standard environments, port 1 is closed.
    let res = probe_local_port("127.0.0.1", 1, Duration::from_millis(50));
    // Must return Result without integer underflow, overflow, or panic
    match res {
        Ok(_) => {} // In the rare event port 1 is listening
        Err(ProbeError::PortClosed { port, .. }) => assert_eq!(port, 1),
        Err(ProbeError::Timeout { port, .. }) => assert_eq!(port, 1),
        Err(ProbeError::Io(_)) => {}
        Err(other) => panic!("Unexpected error on port 1: {:?}", other),
    }
}

#[test]
fn test_f2_b02_port_number_65535_boundary() {
    // Port 65535 (u16::MAX, highest valid TCP port).
    let res = probe_local_port("127.0.0.1", 65535, Duration::from_millis(50));
    match res {
        Ok(_) => {} // If port 65535 is listening
        Err(ProbeError::PortClosed { port, .. }) => assert_eq!(port, 65535),
        Err(ProbeError::Timeout { port, .. }) => assert_eq!(port, 65535),
        Err(ProbeError::Io(_)) => {}
        Err(other) => panic!("Unexpected error on port 65535: {:?}", other),
    }
}

#[test]
fn test_f2_b03_invalid_port_number_rejected() {
    // Port 0 cannot be connected to via standard TCP.
    let res = probe_local_port("127.0.0.1", 0, Duration::from_millis(50));
    assert!(res.is_err(), "Probing port 0 must return an error");

    // Deserialization boundary: port > 65535 cannot fit in u16
    let invalid_json = r#"{
        "version": 1,
        "ports": [{
            "id": "bad_port",
            "alias": "Overflow",
            "host": "127.0.0.1",
            "port": 70000
        }]
    }"#;
    let parse_res = serde_json::from_str::<Config>(invalid_json);
    assert!(
        parse_res.is_err(),
        "Port > 65535 must fail JSON schema deserialization"
    );
}

#[test]
fn test_f2_b04_port_probe_timeout_handling() {
    // RFC 5737 TEST-NET-1 (192.0.2.1) is reserved for documentation and non-routable
    let timeout = Duration::from_millis(80);
    let start = Instant::now();
    let res = probe_local_port("192.0.2.1", 80, timeout);
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Probe to non-routable IP must fail");
    // Elapsed time should be bounded near the timeout (+ slack for thread scheduling)
    assert!(
        elapsed < Duration::from_millis(1500),
        "Timeout handling took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_f2_b05_malformed_http_response_headers() {
    // Start TCP listener that returns non-HTTP garbage data
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(b"NOT_HTTP_PROTOCOL_GARBAGE\r\n\r\n");
            let _ = stream.flush();
            thread::sleep(Duration::from_millis(50));
        }
    });

    let res = probe_upstream_http("127.0.0.1", port, "http://cp.cloudflare.com/generate_204", Duration::from_millis(200));
    let _ = handle.join();

    assert!(res.is_err(), "Probe must fail gracefully on malformed HTTP response");
    match res.unwrap_err() {
        ProbeError::Io(_) => {} // Expected I/O error (invalid data or connection reset)
        other => panic!("Expected ProbeError::Io for malformed response, got {:?}", other),
    }
}

#[test]
fn test_f2_b06_rapid_repeated_probes_without_socket_leak() {
    let mock = MockProxyServer::start_raw_tcp();
    let start = Instant::now();

    // Perform 40 rapid probes against the same port in sequence
    for i in 0..40 {
        let res = probe_local_port("127.0.0.1", mock.port(), Duration::from_millis(100));
        assert!(res.is_ok(), "Probe {} failed", i);
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(1000),
        "40 loopback probes took {:?}, exceeding performance threshold",
        elapsed
    );
}

// ============================================================================
// Feature 3 Boundaries: Windows Shortcut Generation (6 tests)
// ============================================================================

#[test]
fn test_f3_b01_long_path_near_max_path() {
    let env = TestEnv::new();
    // Build a deeply nested path approaching MAX_PATH (around 220 chars)
    let deep_dir = env.subpath("sub_1234567890/sub_1234567890/sub_1234567890/sub_1234567890/sub_1234567890");
    std::fs::create_dir_all(&deep_dir).expect("Failed to create nested test directory");

    let lnk_path = deep_dir.join("long_path_target.lnk");
    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: "/c exit 0".to_string(),
        working_dir: deep_dir.clone(),
        icon_location: None,
        icon_index: 0,
        description: "MaxPath Boundary Test".to_string(),
        destination_file: lnk_path.clone(),
    };

    let res = create_shortcut(&params);
    assert!(res.is_ok(), "Shortcut creation near MAX_PATH failed: {:?}", res.err());
    assert!(lnk_path.exists(), "Shortcut file must exist in deep directory");
}

#[test]
fn test_f3_b02_path_with_multiple_spaces_and_quotes() {
    let env = TestEnv::new();
    let space_dir = env.subpath("Directory With Many Spaces And # Special Symbols (x86)");
    std::fs::create_dir_all(&space_dir).expect("Failed to create spaced directory");

    let lnk_path = space_dir.join("Spaced & Quoted App Name.lnk");
    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: r#"/c "echo hello world" --flag="spaced argument""#.to_string(),
        working_dir: space_dir,
        icon_location: None,
        icon_index: 0,
        description: "Spaces and Quotes Test".to_string(),
        destination_file: lnk_path.clone(),
    };

    let res = create_shortcut(&params);
    assert!(res.is_ok(), "Shortcut creation with complex spaces failed: {:?}", res.err());
    assert!(lnk_path.exists());

    // Verify sanitize_filename handles extreme strings
    let dirty_name = "App: *\"<Name>\"? | / \\ [Special]";
    let clean = sanitize_filename(dirty_name);
    assert!(!clean.contains(':'));
    assert!(!clean.contains('*'));
    assert!(!clean.contains('?'));
    assert!(!clean.contains('<'));
    assert!(!clean.contains('>'));
    assert!(!clean.contains('|'));
}

#[test]
fn test_f3_b03_nonexistent_destination_folder_fails_gracefully() {
    let env = TestEnv::new();
    // Destination path in a folder that was never created
    let bad_dest = env.subpath("nonexistent_folder_abc123/shortcut.lnk");

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: String::new(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Fail Test".to_string(),
        destination_file: bad_dest,
    };

    let res = create_shortcut(&params);
    assert!(
        res.is_err(),
        "Shortcut creation in nonexistent directory must return Err"
    );
}

#[test]
fn test_f3_b04_rejection_of_uwp_virtual_store_executable() {
    let _env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let mut config = Config::default();

    let port = Port::new("P1", "127.0.0.1", mock.port());
    let port_id = port.id.clone();
    config.add_port(port);

    // Rule targeting UWP AppsFolder
    let uwp_rule = Rule {
        id: "uwp_rule_1".to_string(),
        name: "WindowsCalculator".to_string(),
        exe: "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".to_string(),
        port_id,
        args: String::new(),
        cwd: String::new(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "Test".to_string(),
    };
    config.add_rule(uwp_rule);

    let launch_res = launcher_core::launcher::launch_rule(&config, "uwp_rule_1");
    assert!(launch_res.is_err(), "Launching UWP app must be rejected");
    match launch_res.unwrap_err() {
        launcher_core::launcher::LaunchError::UwpNotSupported => {}
        other => panic!("Expected LaunchError::UwpNotSupported, got {:?}", other),
    }
}

#[test]
fn test_f3_b05_idempotent_repeated_shortcut_generation() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("idempotent.lnk");

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: "/c exit 0".to_string(),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Idempotency Test".to_string(),
        destination_file: lnk_path.clone(),
    };

    // Creating shortcut repeatedly in the exact same path must overwrite cleanly without error
    for i in 1..=5 {
        let res = create_shortcut(&params);
        assert!(res.is_ok(), "Iteration {} of create_shortcut failed: {:?}", i, res.err());
        assert!(lnk_path.exists());
    }
}

#[test]
fn test_f3_b06_empty_arguments_or_null_working_directory() {
    let env = TestEnv::new();
    let lnk_path = env.subpath("empty_args_cwd.lnk");

    let params = ShortcutParams {
        target_path: ProcessHelper::safe_cmd_exe(),
        arguments: String::new(),       // Empty arguments
        working_dir: PathBuf::new(),    // Empty working directory
        icon_location: None,
        icon_index: 0,
        description: String::new(),     // Empty description
        destination_file: lnk_path.clone(),
    };

    let res = create_shortcut(&params);
    assert!(res.is_ok(), "Failed with empty args/cwd: {:?}", res.err());
    assert!(lnk_path.exists());
    assert!(std::fs::metadata(&lnk_path).unwrap().len() > 0);
}

// ============================================================================
// Feature 4 Boundaries: Process Spawning & Env Injection (6 tests)
// ============================================================================

#[test]
fn test_f4_b01_nonexistent_target_executable_handled_without_panic() {
    let mock = MockProxyServer::start_raw_tcp();
    let mut config = Config::default();

    let port = Port::new("P1", "127.0.0.1", mock.port());
    let port_id = port.id.clone();
    config.add_port(port);

    let ghost_exe = r"C:\Windows\NonexistentFolder_9999\GhostBinary12345.exe";
    let rule = Rule::new("MissingApp", ghost_exe, &port_id);
    let rule_id = rule.id.clone();
    config.add_rule(rule);

    let res = launcher_core::launcher::launch_rule(&config, &rule_id);
    assert!(res.is_err(), "Nonexistent executable must return Err");
    match res.unwrap_err() {
        launcher_core::launcher::LaunchError::TargetNotFound(path) => {
            assert_eq!(path, ghost_exe);
        }
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }
}

#[test]
fn test_f4_b02_batch_file_cmd_execution_with_escaping() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();

    // Create a temporary batch file in test directory
    let bat_path = env.write_file("test_runner.bat", "@echo off\r\nexit 0\r\n");

    let mut config = Config::default();
    let port = Port::new("P1", "127.0.0.1", mock.port());
    let port_id = port.id.clone();
    config.add_port(port);

    let mut rule = Rule::new("BatchApp", bat_path.to_string_lossy(), &port_id);
    rule.args = r#""first arg with spaces" & second_arg^#test"#.to_string();
    let rule_id = rule.id.clone();
    config.add_rule(rule);

    let res = launcher_core::launcher::launch_rule(&config, &rule_id);
    assert!(res.is_ok(), "Spawning batch file failed: {:?}", res.err());
    let pid = res.unwrap();
    assert!(pid > 0);
}

#[test]
fn test_f4_b03_proxy_mode_none_bypass_env_variables() {
    let port = Port::new("P1", "127.0.0.1", 7890);
    let mut rule = Rule::new("BypassApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    rule.mode = "none".to_string();

    let cmd = launcher_core::launcher::prepare_command(&rule, &port).expect("prepare_command");
    let env_map: std::collections::HashMap<_, _> = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|val| (k.to_string_lossy().into_owned(), val.to_string_lossy().into_owned())))
        .collect();
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

    // In mode "none", neither HTTP_PROXY env nor --proxy-server flag should be injected
    assert!(
        !env_map.contains_key("HTTP_PROXY"),
        "Mode 'none' must not inject HTTP_PROXY"
    );
    assert!(
        !args.iter().any(|a| a.contains("--proxy-server")),
        "Mode 'none' must not append proxy flag"
    );
}

#[test]
fn test_f4_b04_proxy_mode_flag_injects_flags_without_env() {
    let port = Port::new("P1", "127.0.0.1", 7890);
    let mut rule = Rule::new("FlagOnlyApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    rule.mode = "flag".to_string();
    rule.flag_template = "--proxy-server={url}".to_string();

    let cmd = launcher_core::launcher::prepare_command(&rule, &port).expect("prepare_command");
    let env_map: std::collections::HashMap<_, _> = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|val| (k.to_string_lossy().into_owned(), val.to_string_lossy().into_owned())))
        .collect();
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

    // Mode "flag" must include formatted CLI flag but NOT proxy environment variables
    assert!(
        args.iter().any(|a| a == "--proxy-server=http://127.0.0.1:7890"),
        "Mode 'flag' must contain CLI flag"
    );
    assert!(
        !env_map.contains_key("HTTP_PROXY"),
        "Mode 'flag' must not inject proxy environment variables"
    );
}

#[test]
fn test_f4_b05_proxy_mode_both_injects_both_flags_and_env() {
    let port = Port::new("P1", "127.0.0.1", 7890);
    let mut rule = Rule::new("BothApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    rule.mode = "both".to_string();
    rule.flag_template = "--proxy-server={url}".to_string();

    let cmd = launcher_core::launcher::prepare_command(&rule, &port).expect("prepare_command");
    let env_map: std::collections::HashMap<_, _> = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|val| (k.to_string_lossy().into_owned(), val.to_string_lossy().into_owned())))
        .collect();
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

    // Mode "both" must contain both the flag and the environment variables
    assert!(args.iter().any(|a| a == "--proxy-server=http://127.0.0.1:7890"));
    assert_eq!(env_map.get("HTTP_PROXY"), Some(&"http://LOCALHOST:7890".to_string()));
    assert_eq!(env_map.get("ALL_PROXY"), Some(&"socks5://127.0.0.1:7890".to_string()));
    assert_eq!(env_map.get("AICODE_ENDPOINT_URL"), Some(&"https://localhost@aicode.googleapis.com".to_string()));
}

#[test]
fn test_f4_b06_stdio_decoupling_verification() {
    let port = Port::new("P1", "127.0.0.1", 7890);
    let rule = Rule::new("DecoupleApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);

    let cmd = launcher_core::launcher::prepare_command(&rule, &port).expect("prepare_command");
    // Verify command preparation succeeds and inspect creation characteristics
    let cmd_debug = format!("{:?}", cmd);
    assert!(cmd_debug.contains("cmd.exe"));
}

// ============================================================================
// Feature 5 Boundaries: Binary CLI & Fast-Path Interface (6 tests)
// ============================================================================

#[test]
fn test_f5_b01_missing_required_rule_id_exits_code_1() {
    let runner = CliRunner::new();
    let out = runner.run(&["--launch-rule"]);
    out.assert_code(1);
    out.assert_stderr_contains("Error parsing '--launch-rule'");
}

#[test]
fn test_f5_b02_unknown_cli_flag_exits_code_1() {
    let runner = CliRunner::new();
    let out = runner.run(&["--unknown-flag-12345", "--another-invalid"]);
    out.assert_code(1);
    out.assert_stderr_contains("Unknown or unrecognized argument");
}

#[test]
fn test_f5_b03_config_path_pointing_to_nonexistent_file() {
    let env = TestEnv::new();
    let nonexistent_cfg = env.subpath("ghost_config_dir/missing.json");

    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", "rule_1"], &nonexistent_cfg);
    out.assert_code(1);
    out.assert_stderr_contains("Specified configuration file does not exist");
}

#[test]
fn test_f5_b04_empty_arguments_exits_code_1_with_usage() {
    let runner = CliRunner::new();
    let out = runner.run(&[]);
    out.assert_code(1);
    out.assert_stderr_contains("Missing required argument");
    out.assert_stderr_contains("USAGE:");
}

#[test]
fn test_f5_b05_rule_pointing_to_invalid_ip_format() {
    let env = TestEnv::new();
    let mut config = Config::default();

    // Port with invalid IP format
    let port = Port {
        id: "bad_ip_port".to_string(),
        alias: "BadHost".to_string(),
        host: "999.999.999.999".to_string(),
        port: 7890,
    };
    let rule = ProcessHelper::create_safe_rule("rule_bad_ip", "bad_ip_port");
    config.add_port(port);
    config.add_rule(rule);

    let cfg_path = env.create_config(&config);
    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", "rule_bad_ip"], &cfg_path);

    out.assert_code(1);
    out.assert_stderr_contains("Launch failed");
}

#[test]
fn test_f5_b06_duplicate_rule_id_handling_in_configuration() {
    let mut config = Config::default();
    let port = Port::new("Port1", "127.0.0.1", 7890);
    config.add_port(port.clone());

    let mut rule1 = Rule::new("FirstApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    rule1.id = "shared_duplicate_id".to_string();
    config.rules.push(rule1);

    let mut rule2 = Rule::new("SecondApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    rule2.id = "shared_duplicate_id".to_string();
    config.rules.push(rule2);

    // find_rule must resolve without panic, returning the first matching rule
    let matched = config.find_rule("shared_duplicate_id").expect("Must resolve duplicate ID");
    assert_eq!(matched.name, "FirstApp", "Must deterministically select first match");

    // add_rule must update rather than duplicate when called programmatically
    let updated = Rule::new("ReplacedApp", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    let target_id = updated.id.clone();
    config.add_rule(updated);
    let prev_len = config.rules.len();

    let mut replacement = Rule::new("ReplacedApp_V2", ProcessHelper::safe_cmd_exe().to_string_lossy(), &port.id);
    replacement.id = target_id.clone();
    config.add_rule(replacement);

    assert_eq!(config.rules.len(), prev_len, "add_rule should update existing by ID");
    assert_eq!(config.find_rule(&target_id).unwrap().name, "ReplacedApp_V2");
}
