//! Launcher Core library for proxy launcher.
//!
//! Provides data models, atomic configuration persistence, two-phase port probing,
//! native Win32 IShellLink COM shortcut generation, and detached process execution
//! with proxy environment injection.

pub mod config;
pub mod launcher;
pub mod lnk;
pub mod models;
pub mod probe;

// Re-exports for convenience
pub use config::{load_from_path, load_or_default, save_atomic, ConfigError};
pub use launcher::{build_proxy_env, inject_proxy_env, launch_rule, prepare_command, LaunchError};
pub use lnk::{create_shortcut, create_shortcut_for_rule, get_desktop_dir, sanitize_filename, ShortcutError, ShortcutParams};
pub use models::{
    split_arguments, Config, Port, Rule, AICODE_ENDPOINT_URL_DEFAULT, CONFIG_VERSION,
    DEFAULT_FLAG_TEMPLATE, DEFAULT_IP_URL, DEFAULT_TEST_URL, NO_PROXY_VALUE,
};
pub use probe::{probe_local_port, probe_upstream_http, ProbeError};

use std::fmt;

/// Unified error type for launcher-core operations.
#[derive(Debug)]
pub enum Error {
    Config(ConfigError),
    Probe(ProbeError),
    Shortcut(ShortcutError),
    Launch(LaunchError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Config(e) => write!(f, "{}", e),
            Error::Probe(e) => write!(f, "{}", e),
            Error::Shortcut(e) => write!(f, "{}", e),
            Error::Launch(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Config(e) => Some(e),
            Error::Probe(e) => Some(e),
            Error::Shortcut(e) => Some(e),
            Error::Launch(e) => Some(e),
        }
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Self {
        Error::Config(e)
    }
}

impl From<ProbeError> for Error {
    fn from(e: ProbeError) -> Self {
        Error::Probe(e)
    }
}

impl From<ShortcutError> for Error {
    fn from(e: ShortcutError) -> Self {
        Error::Shortcut(e)
    }
}

impl From<LaunchError> for Error {
    fn from(e: LaunchError) -> Self {
        Error::Launch(e)
    }
}
