use e2e_tests::harness::{CliRunner, MockProxyServer, ProcessHelper, TestEnv};
use launcher_core::config::{load_from_path, load_or_default, save_atomic};
use launcher_core::lnk::{create_shortcut, ShortcutParams};
use launcher_core::models::{Config, Port};
use launcher_core::probe::{probe_local_port, probe_upstream_http, ProbeError};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

// ============================================================================
// Tier 3: Cross-Feature Combinations (8 tests)
// ============================================================================

#[test]
fn test_t3_01_config_save_probe_process_launch_lifecycle() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let cfg_path = env.subpath("lifecycle_config.json");

    // 1. Create and save dynamic configuration atomically
    let mut config = Config::default();
    let port = Port::new("LifecyclePort", "127.0.0.1", mock.port());
    let port_id = port.id.clone();
    let rule = ProcessHelper::create_safe_rule("rule_lifecycle", &port_id);
    let rule_id = rule.id.clone();

    config.add_port(port);
    config.add_rule(rule);
    save_atomic(&config, &cfg_path).expect("Step 1: Save atomic failed");

    // 2. Load configuration and probe port health
    let loaded = load_from_path(&cfg_path).expect("Step 2: Reload config failed");
    let active_port = loaded.find_port(&port_id).expect("Step 2: Port not found");
    let probe_res = probe_local_port(&active_port.host, active_port.port, Duration::from_millis(100));
    assert!(probe_res.is_ok(), "Step 2: Local port probe failed");

    // 3. Launch rule via CLI runner with explicit config path
    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out.assert_success();
    out.assert_stdout_contains(&format!("Successfully launched rule '{}'", rule_id));
}

#[test]
fn test_t3_02_dynamic_port_rebinding_and_launch() {
    let env = TestEnv::new();
    let mock_a = MockProxyServer::start_raw_tcp();
    let mock_b = MockProxyServer::start_raw_tcp();
    let cfg_path = env.subpath("rebind_config.json");

    let mut config = Config::default();
    let port_a = Port::new("PortA", "127.0.0.1", mock_a.port());
    let port_b = Port::new("PortB", "127.0.0.1", mock_b.port());
    let port_a_id = port_a.id.clone();
    let port_b_id = port_b.id.clone();

    let rule = ProcessHelper::create_safe_rule("rule_rebind", &port_a_id);
    let rule_id = rule.id.clone();

    config.add_port(port_a);
    config.add_port(port_b);
    config.add_rule(rule);
    save_atomic(&config, &cfg_path).expect("Initial save");

    // 1. Initial state: rule points to Port A
    let runner = CliRunner::new();
    let out1 = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out1.assert_success();

    // 2. Dynamically rebind rule to Port B and persist
    let mut modified_config = load_from_path(&cfg_path).unwrap();
    let target_rule = modified_config.find_rule_mut(&rule_id).unwrap();
    target_rule.port_id = port_b_id.clone();
    save_atomic(&modified_config, &cfg_path).expect("Update save");

    // 3. Launch rule and verify it uses Port B
    let reloaded = load_from_path(&cfg_path).unwrap();
    let active_port = reloaded.active_port_for_rule(reloaded.find_rule(&rule_id).unwrap()).unwrap();
    assert_eq!(active_port.port, mock_b.port(), "Rule must be rebound to Port B");

    let out2 = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out2.assert_success();
}

#[test]
fn test_t3_03_shortcut_creation_and_target_execution() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let (cfg_path, config) = env.create_minimal_config(mock.port());

    let runner = CliRunner::new();
    let isolated_lnk = env.subpath("RuleExecutionShortcut.lnk");
    let rule = &config.rules[0];

    // 1. Create a shortcut pointing to the launcher executable with rule args
    let params = ShortcutParams {
        target_path: runner.bin_path().to_path_buf(),
        arguments: format!("--launch-rule {} --config \"{}\"", rule.id, cfg_path.display()),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Shortcut Execution Test".to_string(),
        destination_file: isolated_lnk.clone(),
    };

    create_shortcut(&params).expect("Shortcut creation must succeed");
    assert!(isolated_lnk.exists());

    // 2. Execute target command directly as the shortcut would
    let out = runner.run_with_config(&["--launch-rule", &rule.id], &cfg_path);
    out.assert_success();
    out.assert_stdout_contains(&format!("Successfully launched rule '{}'", rule.id));
}

#[test]
fn test_t3_04_proxy_mode_matrix_verification() {
    let mock = MockProxyServer::start_raw_tcp();
    let port = Port::new("MatrixPort", "127.0.0.1", mock.port());

    let modes = ["none", "env", "flag", "both"];
    for mode in &modes {
        let mut rule = ProcessHelper::create_safe_rule(format!("rule_mode_{}", mode), &port.id);
        rule.mode = mode.to_string();
        rule.flag_template = "--proxy-server={url}".to_string();

        let cmd = launcher_core::launcher::prepare_command(&rule, &port)
            .unwrap_or_else(|e| panic!("Failed prepare_command for mode '{}': {}", mode, e));
        let env_map: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .filter_map(|(k, v)| v.map(|val| (k.to_string_lossy().into_owned(), val.to_string_lossy().into_owned())))
            .collect();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();

        let has_flag = args.iter().any(|a| a.starts_with("--proxy-server="));
        let has_env = env_map.contains_key("HTTP_PROXY");

        match *mode {
            "none" => {
                assert!(!has_env, "Mode none has env");
                assert!(!has_flag, "Mode none has flag");
            }
            "env" => {
                assert!(has_env, "Mode env missing env");
                assert!(!has_flag, "Mode env has flag");
            }
            "flag" => {
                assert!(!has_env, "Mode flag has env");
                assert!(has_flag, "Mode flag missing flag");
            }
            "both" => {
                assert!(has_env, "Mode both missing env");
                assert!(has_flag, "Mode both missing flag");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_t3_05_multi_port_concurrent_health_check() {
    // 1. Start all active mock proxy servers first so ephemeral ports do not collide
    let mock_204 = MockProxyServer::start_http_204();
    let port_204 = mock_204.port();

    let mock_502 = MockProxyServer::start_http_502();
    let port_502 = mock_502.port();

    let mock_raw = MockProxyServer::start_raw_tcp();
    let port_raw = mock_raw.port();

    // 2. Bind and immediately drop dead ports AFTER all active servers are already listening
    let l_dead1 = TcpListener::bind("127.0.0.1:0").unwrap();
    let port_dead1 = l_dead1.local_addr().unwrap().port();
    drop(l_dead1);

    let l_dead2 = TcpListener::bind("127.0.0.1:0").unwrap();
    let port_dead2 = l_dead2.local_addr().unwrap().port();
    drop(l_dead2);

    let test_url = "http://cp.cloudflare.com/generate_204";

    let t1 = thread::spawn(move || probe_upstream_http("127.0.0.1", port_204, test_url, Duration::from_millis(300)));
    let t2 = thread::spawn(move || probe_local_port("127.0.0.1", port_dead1, Duration::from_millis(50)));
    let t3 = thread::spawn(move || probe_upstream_http("127.0.0.1", port_502, test_url, Duration::from_millis(300)));
    let t4 = thread::spawn(move || probe_local_port("127.0.0.1", port_raw, Duration::from_millis(100)));
    let t5 = thread::spawn(move || probe_local_port("127.0.0.1", port_dead2, Duration::from_millis(50)));

    let res1 = t1.join().unwrap();
    let res2 = t2.join().unwrap();
    let res3 = t3.join().unwrap();
    let res4 = t4.join().unwrap();
    let res5 = t5.join().unwrap();

    assert!(res1.is_ok(), "Port 1 (HTTP 204) must succeed");
    assert!(res2.is_err(), "Port 2 (dead) must fail");
    assert!(res3.is_err(), "Port 3 (HTTP 502) must fail");
    match res3.unwrap_err() {
        ProbeError::HttpError { status, .. } => assert_eq!(status, 502),
        other => panic!("Expected HttpError(502), got {:?}", other),
    }
    assert!(res4.is_ok(), "Port 4 (raw TCP) must succeed");
    assert!(res5.is_err(), "Port 5 (dead) must fail");
}

#[test]
fn test_t3_06_corrupt_config_recovery_and_launch() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let cfg_path = env.write_file("recovering_config.json", "{ \"broken\": [true, null, ");

    // 1. Detect corruption and recover to default
    let (mut recovered, was_corrupt) = load_or_default(&cfg_path);
    assert!(was_corrupt, "Must flag corrupted config");
    assert_eq!(recovered.version, 1);

    // Verify .bad backup file was created
    let bad_file = PathBuf::from(format!("{}.bad", cfg_path.to_string_lossy()));
    assert!(bad_file.exists());

    // 2. Add valid port and rule to recovered config and persist
    let port = Port::new("RestoredPort", "127.0.0.1", mock.port());
    let rule = ProcessHelper::create_safe_rule("rule_restored", &port.id);
    let rule_id = rule.id.clone();

    recovered.add_port(port);
    recovered.add_rule(rule);
    save_atomic(&recovered, &cfg_path).expect("Saving recovered config must succeed");

    // 3. Launch rule through CLI
    let runner = CliRunner::new();
    let out = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out.assert_success();
    out.assert_stdout_contains(&format!("Successfully launched rule '{}'", rule_id));
}

#[test]
fn test_t3_07_shortcut_generation_for_nonexistent_rule() {
    let env = TestEnv::new();
    let (cfg_path, _) = env.create_minimal_config(7890);
    let runner = CliRunner::new();

    // Snapshot sandbox directory files before shortcut attempt
    let files_before: Vec<_> = std::fs::read_dir(env.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    // Call CLI to generate shortcut for a nonexistent rule ID
    let out = runner.run_with_config(&["--create-shortcut", "nonexistent_rule_99999"], &cfg_path);
    out.assert_code(1);
    out.assert_stderr_contains("Error creating shortcut");

    // Verify zero orphaned .lnk files created on disk
    let files_after: Vec<_> = std::fs::read_dir(env.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    for file in &files_after {
        if file.extension().and_then(|s| s.to_str()) == Some("lnk") {
            assert!(
                files_before.contains(file),
                "No orphaned .lnk file should be created on failure: {}",
                file.display()
            );
        }
    }
}

#[test]
fn test_t3_08_port_failover_cycle() {
    let env = TestEnv::new();
    let mock_backup = MockProxyServer::start_raw_tcp();
    let cfg_path = env.subpath("failover_config.json");

    // Primary port is dead (listener bound then dropped)
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_primary_port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut config = Config::default();
    let p_primary = Port::new("PrimaryDead", "127.0.0.1", dead_primary_port);
    let p_backup = Port::new("BackupActive", "127.0.0.1", mock_backup.port());
    let primary_id = p_primary.id.clone();
    let backup_id = p_backup.id.clone();

    let rule = ProcessHelper::create_safe_rule("rule_failover", &primary_id);
    let rule_id = rule.id.clone();

    config.add_port(p_primary);
    config.add_port(p_backup);
    config.add_rule(rule);
    save_atomic(&config, &cfg_path).unwrap();

    let runner = CliRunner::new();

    // 1. Initial attempt fails because primary port is closed (exit code 1)
    let out_fail = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out_fail.assert_code(1);
    out_fail.assert_stderr_contains("Launch failed");

    // 2. Perform failover: rebind rule to active backup port
    let mut updated_config = load_from_path(&cfg_path).unwrap();
    let r = updated_config.find_rule_mut(&rule_id).unwrap();
    r.port_id = backup_id;
    save_atomic(&updated_config, &cfg_path).unwrap();

    // 3. Retry launch: succeeds immediately with exit code 0
    let out_success = runner.run_with_config(&["--launch-rule", &rule_id], &cfg_path);
    out_success.assert_success();
    out_success.assert_stdout_contains(&format!("Successfully launched rule '{}'", rule_id));
}
