//! Win32 IShellLink shortcut generation.
//!
//! Creates Windows `.lnk` shortcuts using native COM interfaces `IShellLinkW` and `IPersistFile`.
//! Sets `SW_SHOWMINNOACTIVE` (7) to prevent console window flashing or stealing focus,
//! extracts application icons from the target PE executable, and targets `launcher-cli.exe`.

use crate::models::Config;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ShortcutError {
    RuleNotFound(String),
    PortNotFound(String),
    DesktopNotFound(String),
    Io(io::Error),
    ComError(String),
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShortcutError::RuleNotFound(r) => write!(f, "Rule not found for shortcut: {}", r),
            ShortcutError::PortNotFound(p) => write!(f, "Port not found for shortcut: {}", p),
            ShortcutError::DesktopNotFound(d) => write!(f, "Desktop directory not found: {}", d),
            ShortcutError::Io(e) => write!(f, "I/O error during shortcut creation: {}", e),
            ShortcutError::ComError(e) => write!(f, "COM error during shortcut creation: {}", e),
        }
    }
}

impl std::error::Error for ShortcutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShortcutError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ShortcutError {
    fn from(e: io::Error) -> Self {
        ShortcutError::Io(e)
    }
}

/// Sanitizes a string for use as a valid Windows file name.
/// Replaces illegal characters `< > : " / \ | ? *` and control characters with `_`.
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "ProxyLauncher".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolves the user's Desktop directory, supporting OneDrive Desktop redirection.
pub fn get_desktop_dir() -> Result<PathBuf, ShortcutError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        use windows::core::GUID;
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

        // FOLDERID_Desktop: {B4BFCC3A-DB2C-424C-B029-7FE99A87C641}
        const FOLDERID_DESKTOP: GUID = GUID::from_u128(0xB4BFCC3A_DB2C_424C_B029_7FE99A87C641);

        unsafe {
            if let Ok(pwstr) = SHGetKnownFolderPath(&FOLDERID_DESKTOP, KF_FLAG_DEFAULT, None) {
                let len = (0..).take_while(|&i| *pwstr.0.add(i) != 0).count();
                let slice = std::slice::from_raw_parts(pwstr.0, len);
                let os_str = std::ffi::OsString::from_wide(slice);
                CoTaskMemFree(Some(pwstr.0 as *const _));
                let path = PathBuf::from(os_str);
                if path.exists() {
                    return Ok(path);
                }
            }
        }
    }

    // Fallback: check USERPROFILE/Desktop and OneDrive/Desktop
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let onedrive_desktop = PathBuf::from(&userprofile).join("OneDrive").join("Desktop");
        if onedrive_desktop.exists() {
            return Ok(onedrive_desktop);
        }
        let std_desktop = PathBuf::from(&userprofile).join("Desktop");
        if std_desktop.exists() {
            return Ok(std_desktop);
        }
    }

    Err(ShortcutError::DesktopNotFound(
        "Could not determine Desktop path".to_string(),
    ))
}

/// Shortcut configuration parameters.
#[derive(Debug, Clone)]
pub struct ShortcutParams {
    pub target_path: PathBuf,
    pub arguments: String,
    pub working_dir: PathBuf,
    pub icon_location: Option<PathBuf>,
    pub icon_index: i32,
    pub description: String,
    pub destination_file: PathBuf,
}

/// Helper struct for COM apartment lifecycle management.
#[cfg(windows)]
struct ComGuard {
    should_uninit: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn new() -> Result<Self, ShortcutError> {
        use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
        use windows::Win32::System::Com::{
            CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
        };

        unsafe {
            let hr = CoInitializeEx(
                None,
                COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
            );
            if hr == S_OK || hr == S_FALSE {
                Ok(Self { should_uninit: true })
            } else if hr == RPC_E_CHANGED_MODE {
                Ok(Self { should_uninit: false })
            } else {
                Err(ShortcutError::ComError(format!(
                    "CoInitializeEx failed with HRESULT 0x{:08X}",
                    hr.0
                )))
            }
        }
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            use windows::Win32::System::Com::CoUninitialize;
            unsafe {
                CoUninitialize();
            }
        }
    }
}

/// Creates a Windows `.lnk` shortcut with specified parameters.
pub fn create_shortcut(params: &ShortcutParams) -> Result<PathBuf, ShortcutError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::{Interface, PCWSTR};
        use windows::Win32::System::Com::{
            CoCreateInstance, CLSCTX_INPROC_SERVER, IPersistFile,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;

        let _guard = ComGuard::new()?;

        let link: IShellLinkW = unsafe {
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| ShortcutError::ComError(format!("CoCreateInstance IShellLinkW failed: {}", e)))?
        };

        // Convert strings to null-terminated UTF-16
        let to_wide = |s: &std::ffi::OsStr| -> Vec<u16> {
            s.encode_wide().chain(Some(0)).collect()
        };

        let target_w = to_wide(params.target_path.as_os_str());
        unsafe {
            link.SetPath(PCWSTR(target_w.as_ptr()))
                .map_err(|e| ShortcutError::ComError(format!("SetPath failed: {}", e)))?;
        }

        if !params.arguments.is_empty() {
            let args_w = to_wide(std::ffi::OsStr::new(&params.arguments));
            unsafe {
                link.SetArguments(PCWSTR(args_w.as_ptr()))
                    .map_err(|e| ShortcutError::ComError(format!("SetArguments failed: {}", e)))?;
            }
        }

        if !params.working_dir.as_os_str().is_empty() {
            let cwd_w = to_wide(params.working_dir.as_os_str());
            unsafe {
                link.SetWorkingDirectory(PCWSTR(cwd_w.as_ptr()))
                    .map_err(|e| ShortcutError::ComError(format!("SetWorkingDirectory failed: {}", e)))?;
            }
        }

        if !params.description.is_empty() {
            let desc_w = to_wide(std::ffi::OsStr::new(&params.description));
            unsafe {
                link.SetDescription(PCWSTR(desc_w.as_ptr()))
                    .map_err(|e| ShortcutError::ComError(format!("SetDescription failed: {}", e)))?;
            }
        }

        // SW_SHOWMINNOACTIVE = 7: Displays the window minimized and keeps active window
        const SW_SHOWMINNOACTIVE_VAL: i32 = 7;
        unsafe {
            link.SetShowCmd(SHOW_WINDOW_CMD(SW_SHOWMINNOACTIVE_VAL))
                .map_err(|e| ShortcutError::ComError(format!("SetShowCmd failed: {}", e)))?;
        }

        // Set icon from target executable or specified icon
        if let Some(ref icon_path) = params.icon_location {
            if icon_path.exists() {
                let icon_w = to_wide(icon_path.as_os_str());
                unsafe {
                    let _ = link.SetIconLocation(PCWSTR(icon_w.as_ptr()), params.icon_index);
                }
            }
        }

        // Save shortcut via IPersistFile
        let persist: IPersistFile = link.cast().map_err(|e| {
            ShortcutError::ComError(format!("QueryInterface IPersistFile failed: {}", e))
        })?;

        let dest_w = to_wide(params.destination_file.as_os_str());
        unsafe {
            persist
                .Save(PCWSTR(dest_w.as_ptr()), true)
                .map_err(|e| ShortcutError::ComError(format!("IPersistFile::Save failed: {}", e)))?;
        }

        Ok(params.destination_file.clone())
    }

    #[cfg(not(windows))]
    {
        Err(ShortcutError::ComError("Windows shortcuts are only supported on Windows".to_string()))
    }
}

/// High-level function to create a direct desktop shortcut for a specific rule.
pub fn create_shortcut_for_rule(
    config: &Config,
    rule_id_or_name: &str,
    cli_exe_path: &Path,
) -> Result<PathBuf, ShortcutError> {
    let rule = config
        .find_rule(rule_id_or_name)
        .ok_or_else(|| ShortcutError::RuleNotFound(rule_id_or_name.to_string()))?;

    let port = config
        .active_port_for_rule(rule)
        .ok_or_else(|| ShortcutError::PortNotFound(rule.port_id.clone()))?;

    let desktop = get_desktop_dir()?;
    let rule_display_name = if rule.name.trim().is_empty() {
        "App"
    } else {
        rule.name.trim()
    };

    let filename = format!("{} (代理).lnk", sanitize_filename(rule_display_name));
    let shortcut_path = desktop.join(&filename);

    let working_dir = if let Some(parent) = cli_exe_path.parent() {
        parent.to_path_buf()
    } else {
        PathBuf::from(".")
    };

    let icon_location = if !rule.exe.is_empty() && Path::new(&rule.exe).exists() {
        Some(PathBuf::from(&rule.exe))
    } else {
        None
    };

    let description = format!(
        "通过代理 {}:{} 直接启动 {}",
        port.host, port.port, rule_display_name
    );

    let params = ShortcutParams {
        target_path: cli_exe_path.to_path_buf(),
        arguments: format!("--launch-rule \"{}\"", rule.id),
        working_dir,
        icon_location,
        icon_index: 0,
        description,
        destination_file: shortcut_path,
    };

    create_shortcut(&params)
}
