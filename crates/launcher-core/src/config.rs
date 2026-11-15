//! Configuration file loading and atomic persistence.

use crate::models::Config;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    Io(io::Error),
    Json(serde_json::Error),
    AtomicRenameFailed {
        source: PathBuf,
        target: PathBuf,
        details: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(p) => write!(f, "Configuration file not found: {}", p.display()),
            ConfigError::Io(e) => write!(f, "I/O error while accessing config: {}", e),
            ConfigError::Json(e) => write!(f, "JSON parse error in config: {}", e),
            ConfigError::AtomicRenameFailed {
                source,
                target,
                details,
            } => write!(
                f,
                "Failed to atomically rename {} to {}: {}",
                source.display(),
                target.display(),
                details
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Json(e)
    }
}

/// Loads and parses configuration from the given file path.
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.to_path_buf()));
    }
    let content = std::fs::read_to_string(path)?;
    let content_trimmed = content.strip_prefix('\u{feff}').unwrap_or(&content);
    let config = serde_json::from_str::<Config>(content_trimmed)?;
    Ok(config)
}

/// Loads configuration from the given path, or backs up corrupted config as `.bad` and returns default.
pub fn load_or_default(path: &Path) -> (Config, bool) {
    if !path.exists() {
        return (Config::default(), false);
    }
    match load_from_path(path) {
        Ok(cfg) => (cfg, false),
        Err(_) => {
            // Backup bad config file
            let bad_path = PathBuf::from(format!("{}.bad", path.to_string_lossy()));
            let _ = std::fs::copy(path, bad_path);
            (Config::default(), true)
        }
    }
}

/// Saves the configuration atomically using a temporary file and atomic replace.
pub fn save_atomic(config: &Config, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let tmp_path = PathBuf::from(format!("{}.tmp", path.to_string_lossy()));
    let serialized = serde_json::to_string_pretty(config)?;

    {
        use io::Write;
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };

        let tmp_w: Vec<u16> = tmp_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let target_w: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

        let res = unsafe {
            MoveFileExW(
                PCWSTR(tmp_w.as_ptr()),
                PCWSTR(target_w.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };

        if let Err(err) = res {
            // Clean up temporary file on failure
            let _ = std::fs::remove_file(&tmp_path);
            return Err(ConfigError::AtomicRenameFailed {
                source: tmp_path,
                target: path.to_path_buf(),
                details: err.to_string(),
            });
        }
    }

    #[cfg(not(windows))]
    {
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(ConfigError::AtomicRenameFailed {
                source: tmp_path,
                target: path.to_path_buf(),
                details: e.to_string(),
            });
        }
    }

    Ok(())
}

impl Config {
    /// Loads configuration from path.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        load_from_path(path)
    }

    /// Saves configuration atomically to path.
    pub fn save_atomic(&self, path: &Path) -> Result<(), ConfigError> {
        save_atomic(self, path)
    }
}
