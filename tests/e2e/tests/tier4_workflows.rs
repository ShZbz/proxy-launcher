use e2e_tests::harness::{CliRunner, MockProxyServer, ProcessHelper, TestEnv};
use launcher_core::config::{load_from_path, load_or_default, save_atomic};
use launcher_core::lnk::{create_shortcut, sanitize_filename, ShortcutParams};
use launcher_core::models::{generate_id, Config, Port, Rule};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Tier 4: Real-World Application Scenarios & SLA (6 tests)
// ============================================================================

#[test]
fn test_t4_01_first_run_onboarding_workflow() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("onboarding_config.json");

    // 1. First run: file does not exist on disk
    assert!(!cfg_path.exists());
    let (mut cfg, was_corrupt) = load_or_default(&cfg_path);
    assert!(!was_corrupt, "Fresh install must not trigger corruption backup");
    assert_eq!(cfg.version, 1);
    assert!(cfg.ports.is_empty());
    assert!(cfg.rules.is_empty());

    // 2. User adds primary ClashMeta port
    let clash_port = Port::new("ClashMeta", "127.0.0.1", 7890);
    let port_id = clash_port.id.clone();
    cfg.add_port(clash_port);

    // 3. User registers Git rule
    let git_exe = ProcessHelper::safe_cmd_exe();
    let mut git_rule = Rule::new("Git", git_exe.to_string_lossy(), &port_id);
    git_rule.args = "clone https://github.com/example/repo.git".to_string();
    git_rule.mode = "env".to_string();
    let rule_id = git_rule.id.clone();
    cfg.add_rule(git_rule);

    // 4. User generates desktop shortcut in isolated sandbox
    let isolated_lnk = env.subpath(format!("{}.lnk", sanitize_filename("Git (代理)")));
    let params = ShortcutParams {
        target_path: git_exe,
        arguments: format!("--launch-rule {}", rule_id),
        working_dir: env.path().to_path_buf(),
        icon_location: None,
        icon_index: 0,
        description: "Onboarding Shortcut".to_string(),
        destination_file: isolated_lnk.clone(),
    };
    create_shortcut(&params).expect("Failed to create onboarding shortcut");
    assert!(isolated_lnk.exists());

    // 5. Persist configuration atomically and reload
    save_atomic(&cfg, &cfg_path).expect("Failed to save onboarding config");
    let loaded = load_from_path(&cfg_path).expect("Failed to reload saved config");
    assert_eq!(loaded.ports.len(), 1);
    assert_eq!(loaded.rules.len(), 1);
    assert_eq!(loaded.ports[0].alias, "ClashMeta");
    assert_eq!(loaded.rules[0].name, "Git");
}

#[test]
fn test_t4_02_developer_dual_proxy_switch_workflow() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("dual_proxy.json");

    let mut cfg = Config::default();
    let dev_port = Port::new("LocalClash", "127.0.0.1", 7890);
    let vpn_port = Port::new("CompanyVPN", "127.0.0.1", 1080);
    let dev_id = dev_port.id.clone();
    let vpn_id = vpn_port.id.clone();

    cfg.add_port(dev_port);
    cfg.add_port(vpn_port);

    let ide_rule = Rule::new("IDE", ProcessHelper::safe_cmd_exe().to_string_lossy(), &dev_id);
    let rule_id = ide_rule.id.clone();
    cfg.add_rule(ide_rule);
    save_atomic(&cfg, &cfg_path).expect("Save initial dual-proxy config");

    // Phase 1: IDE is bound to LocalClash (7890)
    let reloaded1 = load_from_path(&cfg_path).unwrap();
    let rule_ref1 = reloaded1.find_rule(&rule_id).unwrap();
    let port_ref1 = reloaded1.active_port_for_rule(rule_ref1).unwrap();
    assert_eq!(port_ref1.port, 7890);

    let env_dev = launcher_core::launcher::build_proxy_env(&port_ref1.host, port_ref1.port);
    let map_dev: HashMap<_, _> = env_dev.into_iter().collect();
    assert_eq!(map_dev.get("HTTP_PROXY"), Some(&"http://LOCALHOST:7890".to_string()));

    // Phase 2: Switch IDE rule to CompanyVPN (1080)
    let mut modified = reloaded1;
    let r_mut = modified.find_rule_mut(&rule_id).unwrap();
    r_mut.port_id = vpn_id;
    save_atomic(&modified, &cfg_path).expect("Save switched dual-proxy config");

    let reloaded2 = load_from_path(&cfg_path).unwrap();
    let rule_ref2 = reloaded2.find_rule(&rule_id).unwrap();
    let port_ref2 = reloaded2.active_port_for_rule(rule_ref2).unwrap();
    assert_eq!(port_ref2.port, 1080);

    let env_vpn = launcher_core::launcher::build_proxy_env(&port_ref2.host, port_ref2.port);
    let map_vpn: HashMap<_, _> = env_vpn.into_iter().collect();
    assert_eq!(map_vpn.get("HTTP_PROXY"), Some(&"http://LOCALHOST:1080".to_string()));
}

#[test]
fn test_t4_03_drag_and_drop_rule_registration_workflow() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("dnd_config.json");

    let mut cfg = Config::default();
    let default_port = Port::new("DefaultProxy", "127.0.0.1", 7890);
    let port_id = default_port.id.clone();
    cfg.add_port(default_port);

    // Simulating user dragging an executable (e.g. cmd.exe) into the launcher UI
    let dropped_exe = ProcessHelper::safe_cmd_exe();
    assert!(dropped_exe.exists(), "Dropped target must exist on disk");

    let file_stem = dropped_exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("App");
    let detected_cwd = dropped_exe.parent().unwrap_or(Path::new("."));

    let generated_rule = Rule {
        id: generate_id(),
        name: file_stem.to_string(),
        exe: dropped_exe.to_string_lossy().into_owned(),
        port_id: port_id.clone(),
        args: String::new(),
        cwd: detected_cwd.to_string_lossy().into_owned(),
        mode: "both".to_string(),
        flag_template: "--proxy-server={url}".to_string(),
        enabled: true,
        source: "DragAndDrop".to_string(),
    };
    let new_rule_id = generated_rule.id.clone();
    assert_eq!(new_rule_id.len(), 12, "Generated ID must be 12-char hex");

    cfg.add_rule(generated_rule);
    save_atomic(&cfg, &cfg_path).expect("Failed to persist dropped rule");

    // Verify persisted state
    let loaded = load_from_path(&cfg_path).expect("Failed to reload config");
    let registered = loaded.find_rule(&new_rule_id).expect("Rule must exist");
    assert_eq!(registered.name, "cmd");
    assert_eq!(registered.source, "DragAndDrop");
    assert_eq!(registered.port_id, port_id);
}

#[test]
fn test_t4_04_terminal_injection_script_generation_workflow() {
    let host = "127.0.0.1";
    let port = 7890;
    let http = format!("http://{}:{}", host, port);
    let socks = format!("socks5://{}:{}", host, port);

    // Generate terminal export scripts
    let ps_cmd = format!(
        r#"$env:HTTP_PROXY = "{}"; $env:HTTPS_PROXY = "{}"; $env:ALL_PROXY = "{}"; Write-Output $env:HTTP_PROXY"#,
        http, http, socks
    );
    let _cmd_script = format!(
        r#"set HTTP_PROXY={} && set HTTPS_PROXY={} && set ALL_PROXY={} && echo %HTTP_PROXY%"#,
        http, http, socks
    );
    let bash_script = format!(
        r#"export HTTP_PROXY="{}" && export HTTPS_PROXY="{}" && export ALL_PROXY="{}""#,
        http, http, socks
    );

    // 1. Verify PowerShell injection syntax executes cleanly
    let ps_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output()
        .expect("Failed to execute PowerShell script");
    let ps_stdout = String::from_utf8_lossy(&ps_out.stdout).trim().to_string();
    assert_eq!(ps_stdout, http, "PowerShell proxy injection script mismatch");

    // 2. Verify CMD injection syntax executes cleanly
    let cmd_out = Command::new("cmd")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .args([
            "/v:on",
            "/c",
            &format!("set HTTP_PROXY={} && set HTTPS_PROXY={} && set ALL_PROXY={} && echo !HTTP_PROXY!", http, http, socks),
        ])
        .output()
        .expect("Failed to execute CMD script");
    let cmd_stdout = String::from_utf8_lossy(&cmd_out.stdout).trim().to_string();
    assert_eq!(cmd_stdout, http, "CMD proxy injection script mismatch");

    // 3. Verify Bash script syntax formatting
    assert!(bash_script.starts_with("export HTTP_PROXY="));
    assert!(bash_script.contains("export ALL_PROXY=\"socks5://127.0.0.1:7890\""));
}

#[test]
fn test_t4_05_high_concurrency_atomic_stress_workflow() {
    let env = TestEnv::new();
    let cfg_path = env.subpath("high_concurrency_stress.json");

    let initial = Config::default();
    save_atomic(&initial, &cfg_path).expect("Initial save");

    let path_arc = Arc::new(cfg_path);
    let barrier = Arc::new(Barrier::new(5));
    let mut handles = Vec::new();

    // 5 concurrent threads each performing 10 rapid atomic updates (50 total updates)
    for worker_id in 0..5 {
        let p = Arc::clone(&path_arc);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for i in 0..10 {
                let mut cfg = load_from_path(&p).unwrap_or_default();
                let port_alias = format!("Worker_{}_{}", worker_id, i);
                cfg.add_port(Port::new(&port_alias, "127.0.0.1", 7000 + (worker_id * 10 + i) as u16));
                let _ = save_atomic(&cfg, &p);
                thread::sleep(Duration::from_millis(1));
            }
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked");
    }

    // Assert zero file corruption, clean json, and no leftover temp files
    assert!(!path_arc.with_extension("tmp").exists(), "No .tmp should linger");
    let final_config = load_from_path(&path_arc).expect("Final config must be valid JSON");
    assert!(final_config.ports.len() > 0, "At least some ports must be persisted");
}

#[test]
fn test_t4_06_microsecond_fast_path_sla_benchmark() {
    let env = TestEnv::new();
    let mock = MockProxyServer::start_raw_tcp();
    let (cfg_path, _) = env.create_minimal_config(mock.port());

    let runner = CliRunner::new();

    // 1. Warm-up invocation (loads binary, caches DLLs and disk pages)
    let _ = Command::new(runner.bin_path())
        .args(["--launch-rule", "rule_test_1", "--config", &cfg_path.to_string_lossy()])
        .output();

    // 2. Multi-sample measurement iterations taking the minimum latency
    let mut min_internal_ms: Option<f64> = None;
    let mut min_roundtrip = Duration::from_secs(10);

    for _ in 0..3 {
        let mut cmd = Command::new(runner.bin_path());
        cmd.args(["--launch-rule", "rule_test_1", "--config", &cfg_path.to_string_lossy()]);
        cmd.env("PROXY_LAUNCHER_BENCHMARK", "1");

        let start = Instant::now();
        let output = cmd.output().expect("Failed to execute benchmark launch");
        let elapsed = start.elapsed();

        assert!(output.status.success(), "Benchmark launch must succeed");
        if elapsed < min_roundtrip {
            min_roundtrip = elapsed;
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(pos) = stderr.find("[BENCHMARK_INTERNAL_MS]") {
            let line = stderr[pos..].lines().next().unwrap_or_default();
            let val_str = line.trim_start_matches("[BENCHMARK_INTERNAL_MS]").trim();
            if let Ok(internal_ms) = val_str.parse::<f64>() {
                min_internal_ms = Some(match min_internal_ms {
                    Some(prev) => prev.min(internal_ms),
                    None => internal_ms,
                });
            }
        }
    }

    if let Some(internal_ms) = min_internal_ms {
        println!("Best internal fast-path latency: {:.4} ms", internal_ms);
        // Debug builds (unoptimized, debug assertions) run around 20-50ms; Release SLA target is < 15ms.
        let max_allowed_ms = if cfg!(debug_assertions) { 100.0 } else { 15.0 };
        assert!(
            internal_ms < max_allowed_ms,
            "Fast-path internal latency ({:.2} ms) exceeded SLA threshold ({:.2} ms)!",
            internal_ms, max_allowed_ms
        );
    }

    // Also assert that the overall roundtrip execution completes well within SLA (< 500ms)
    assert!(
        min_roundtrip < Duration::from_millis(500),
        "Best roundtrip execution took {:?}, exceeding SLA",
        min_roundtrip
    );
}
