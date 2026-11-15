//! Data Transfer Objects (DTOs) for Tauri IPC and Dual-Mode Bridge.

use serde::{Deserialize, Serialize};

/// Result of probing a proxy port.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeResult {
    pub host: String,
    pub port: u16,
    pub ok: bool,
    pub reason: String,
    pub reason_text: String,
    pub detail: String,
    pub latency_ms: Option<u64>,
    pub http_code: Option<u16>,
    pub exit_ip: Option<String>,
}

/// Result of launching an application rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchResult {
    pub pid: u32,
    pub exe: String,
    pub ok: bool,
    pub message: String,
}

/// Information extracted when an executable or shortcut (.lnk) is dropped or picked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedTarget {
    pub exe: String,
    pub name: String,
    pub args: String,
    pub cwd: String,
    pub icon: String,
    pub needs_elevation: bool,
    pub kind: String,
    pub parse_note: String,
    pub source: String,
}
