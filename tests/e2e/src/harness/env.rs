use launcher_core::models::{generate_id, Config, Port, Rule};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// RAII wrapper for an isolated temporary test directory.
/// Automatically cleans up all files when dropped.
#[derive(Debug)]
pub struct TestEnv {
    root: PathBuf,
}

impl TestEnv {
    /// Creates a new isolated temporary sandbox directory.
    pub fn new() -> Self {
        let count = ENV_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir_name = format!("pl_e2e_{}_{}_{}", std::process::id(), count, nanos);
        let root = std::env::temp_dir().join(dir_name);
        std::fs::create_dir_all(&root).expect("Failed to create TestEnv directory");
        Self { root }
    }

    /// Returns the root path of the sandbox directory.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Resolves a subpath relative to the sandbox directory.
    pub fn subpath(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.root.join(rel)
    }

    /// Writes content to a file inside the sandbox.
    pub fn write_file(&self, rel: impl AsRef<Path>, content: &str) -> PathBuf {
        let target = self.subpath(rel);
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&target, content).expect("Failed to write test file");
        target
    }

    /// Reads content from a file inside the sandbox.
    pub fn read_file(&self, rel: impl AsRef<Path>) -> std::io::Result<String> {
        std::fs::read_to_string(self.subpath(rel))
    }

    /// Writes a Config object to `config.json` inside the sandbox.
    pub fn create_config(&self, config: &Config) -> PathBuf {
        let json_str = serde_json::to_string_pretty(config).expect("Failed to serialize config");
        self.write_file("config.json", &json_str)
    }

    /// Creates a minimal configuration containing one port and one safe cmd rule.
    pub fn create_minimal_config(&self, port: u16) -> (PathBuf, Config) {
        let port_id = "port_test_1".to_string();
        let rule_id = "rule_test_1".to_string();

        let test_port = Port {
            id: port_id.clone(),
            alias: "TestProxy".to_string(),
            host: "127.0.0.1".to_string(),
            port,
        };

        let test_rule = Rule {
            id: rule_id.clone(),
            name: "TestCmdApp".to_string(),
            exe: r"C:\Windows\System32\cmd.exe".to_string(),
            port_id,
            args: "/c exit 0".to_string(),
            cwd: String::new(),
            mode: "both".to_string(),
            flag_template: "--proxy-server={url}".to_string(),
            enabled: true,
            source: "Test".to_string(),
        };

        let mut config = Config::default();
        config.ports.push(test_port);
        config.rules.push(test_rule);

        let path = self.create_config(&config);
        (path, config)
    }

    /// Creates a multi-port configuration with provided alias-port pairs.
    pub fn create_multi_port_config(&self, ports: &[(&str, u16)]) -> (PathBuf, Config) {
        let mut config = Config::default();

        for (alias, port_num) in ports {
            let p_id = format!("port_{}", generate_id());
            config.ports.push(Port {
                id: p_id.clone(),
                alias: alias.to_string(),
                host: "127.0.0.1".to_string(),
                port: *port_num,
            });

            config.rules.push(Rule {
                id: format!("rule_{}", generate_id()),
                name: format!("App_{}", alias),
                exe: r"C:\Windows\System32\cmd.exe".to_string(),
                port_id: p_id,
                args: "/c exit 0".to_string(),
                cwd: String::new(),
                mode: "both".to_string(),
                flag_template: "--proxy-server={url}".to_string(),
                enabled: true,
                source: "Test".to_string(),
            });
        }

        let path = self.create_config(&config);
        (path, config)
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Attempt recursive removal; ignore errors if files are temporarily locked
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
