//! Data models matching config.json schema and domain objects.

use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_FLAG_TEMPLATE: &str = "--proxy-server={url}";
pub const DEFAULT_TEST_URL: &str = "http://cp.cloudflare.com/generate_204";
pub const DEFAULT_IP_URL: &str = "https://api.ipify.org";
pub const NO_PROXY_VALUE: &str = "localhost,127.0.0.1,::1";
pub const AICODE_ENDPOINT_URL_DEFAULT: &str = "https://localhost@aicode.googleapis.com";
pub const CDP_REMOTE_DEBUG_PORT_FLAG: &str = "--remote-debugging-port=0";
pub const CDP_REMOTE_DEBUG_ADDR_FLAG: &str = "--remote-debugging-address=127.0.0.1";

fn default_version() -> u32 {
    CONFIG_VERSION
}

fn default_test_url() -> String {
    DEFAULT_TEST_URL.to_string()
}

fn default_ip_url() -> String {
    DEFAULT_IP_URL.to_string()
}

fn default_true() -> bool {
    true
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_flag_template() -> String {
    DEFAULT_FLAG_TEMPLATE.to_string()
}

fn default_mode() -> String {
    "both".to_string()
}

/// Generates a 12-character hex ID matching Python's `uuid.uuid4().hex[:12]`.
pub fn generate_id() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::System::Com::CoCreateGuid;
        if let Ok(guid) = unsafe { CoCreateGuid() } {
            return format!("{:08x}{:04x}", guid.data1, guid.data2);
        }
    }

    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU32 = AtomicU32::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let cnt = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:08x}{:04x}", (now & 0xFFFFFFFF) as u32, cnt & 0xFFFF)
}

fn deserialize_mode<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.trim().to_lowercase().as_str() {
        "env" | "flag" | "both" => Ok(s.trim().to_lowercase()),
        _ => Ok("both".to_string()),
    }
}

/// Port configuration item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Port {
    #[serde(default = "generate_id")]
    pub id: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
}

impl Default for Port {
    fn default() -> Self {
        Self {
            id: generate_id(),
            alias: String::new(),
            host: default_host(),
            port: 0,
        }
    }
}

impl Port {
    pub fn new(alias: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: generate_id(),
            alias: alias.into(),
            host: host.into(),
            port,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn socks_url(&self) -> String {
        format!("socks5://{}:{}", self.host, self.port)
    }
}

/// Rule configuration item defining target executable and proxy mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rule {
    #[serde(default = "generate_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub exe: String,
    #[serde(default)]
    pub port_id: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default = "default_mode", deserialize_with = "deserialize_mode")]
    pub mode: String,
    #[serde(default = "default_flag_template")]
    pub flag_template: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub source: String,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: generate_id(),
            name: String::new(),
            exe: String::new(),
            port_id: String::new(),
            args: String::new(),
            cwd: String::new(),
            mode: default_mode(),
            flag_template: default_flag_template(),
            enabled: true,
            source: String::new(),
        }
    }
}

impl Rule {
    pub fn new(
        name: impl Into<String>,
        exe: impl Into<String>,
        port_id: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_id(),
            name: name.into(),
            exe: exe.into(),
            port_id: port_id.into(),
            args: String::new(),
            cwd: String::new(),
            mode: default_mode(),
            flag_template: default_flag_template(),
            enabled: true,
            source: String::new(),
        }
    }

    /// Determines the effective working directory for process execution.
    pub fn effective_cwd(&self) -> PathBuf {
        let trimmed_cwd = self.cwd.trim();
        if !trimmed_cwd.is_empty() {
            return PathBuf::from(trimmed_cwd);
        }
        let exe_path = Path::new(&self.exe);
        if let Some(parent) = exe_path.parent() {
            if !parent.as_os_str().is_empty() {
                return parent.to_path_buf();
            }
        }
        PathBuf::from(".")
    }

    /// Formats the command-line flag template with the port's HTTP URL.
    pub fn formatted_flag(&self, port: &Port) -> String {
        let url = port.url();
        if self.flag_template.contains("{url}") {
            self.flag_template.replace("{url}", &url)
        } else {
            format!("{}={}", self.flag_template, url)
        }
    }

    /// Splits the rule's argument string into individual CLI tokens, respecting quotes.
    pub fn split_args(&self) -> Vec<String> {
        split_arguments(&self.args)
    }

    /// Returns true if this rule targets Antigravity (agy) by rule name or executable filename.
    pub fn is_antigravity(&self) -> bool {
        let has_agy_token = |s: &str| {
            s.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|tok| tok.eq_ignore_ascii_case("agy"))
        };

        let name_low = self.name.trim().to_ascii_lowercase();
        if name_low.contains("antigravity") || has_agy_token(&name_low) {
            return true;
        }

        let base = self
            .exe
            .trim()
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let stem = match base.rfind('.') {
            Some(idx) if idx > 0 => &base[..idx],
            _ => &base,
        };
        stem.contains("antigravity") || has_agy_token(stem)
    }
}

/// Splits a Windows command line string into tokens respecting double quotes.
pub fn split_arguments(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            '\\' => {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '"' {
                        current.push('"');
                        chars.next();
                        continue;
                    }
                }
                current.push('\\');
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Top-level launcher configuration matching config.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_test_url")]
    pub test_url: String,
    #[serde(default = "default_ip_url")]
    pub ip_url: String,
    #[serde(default = "default_true")]
    pub show_exit_ip: bool,
    #[serde(default)]
    pub antigravity_cdp: bool,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            test_url: default_test_url(),
            ip_url: default_ip_url(),
            show_exit_ip: true,
            antigravity_cdp: false,
            ports: Vec::new(),
            rules: Vec::new(),
        }
    }
}

impl Config {
    /// Finds a rule by exact ID match first, falling back to case-insensitive name match.
    pub fn find_rule(&self, rule_id_or_name: &str) -> Option<&Rule> {
        let trimmed = rule_id_or_name.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(r) = self.rules.iter().find(|r| r.id == trimmed) {
            return Some(r);
        }
        self.rules
            .iter()
            .find(|r| r.name.trim().eq_ignore_ascii_case(trimmed))
    }

    /// Finds a mutable rule by exact ID or case-insensitive name match.
    pub fn find_rule_mut(&mut self, rule_id_or_name: &str) -> Option<&mut Rule> {
        let trimmed = rule_id_or_name.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(pos) = self.rules.iter().position(|r| r.id == trimmed) {
            return Some(&mut self.rules[pos]);
        }
        self.rules
            .iter_mut()
            .find(|r| r.name.trim().eq_ignore_ascii_case(trimmed))
    }

    /// Finds a port by exact ID match first, falling back to case-insensitive alias match.
    pub fn find_port(&self, port_id_or_alias: &str) -> Option<&Port> {
        let trimmed = port_id_or_alias.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(p) = self.ports.iter().find(|p| p.id == trimmed) {
            return Some(p);
        }
        self.ports
            .iter()
            .find(|p| p.alias.trim().eq_ignore_ascii_case(trimmed))
    }

    /// Finds a mutable port by exact ID or case-insensitive alias match.
    pub fn find_port_mut(&mut self, port_id_or_alias: &str) -> Option<&mut Port> {
        let trimmed = port_id_or_alias.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(pos) = self.ports.iter().position(|p| p.id == trimmed) {
            return Some(&mut self.ports[pos]);
        }
        self.ports
            .iter_mut()
            .find(|p| p.alias.trim().eq_ignore_ascii_case(trimmed))
    }

    /// Resolves the active port configured for the given rule.
    pub fn active_port_for_rule(&self, rule: &Rule) -> Option<&Port> {
        self.find_port(&rule.port_id)
    }

    /// Adds or updates a port by ID.
    pub fn add_port(&mut self, port: Port) {
        if let Some(existing) = self.ports.iter_mut().find(|p| p.id == port.id) {
            *existing = port;
        } else {
            self.ports.push(port);
        }
    }

    /// Adds or updates a rule by ID.
    pub fn add_rule(&mut self, rule: Rule) {
        if let Some(existing) = self.rules.iter_mut().find(|r| r.id == rule.id) {
            *existing = rule;
        } else {
            self.rules.push(rule);
        }
    }
}
