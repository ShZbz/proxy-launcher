//! Shell link (.lnk) and executable target resolver.

use crate::dto::ResolvedTarget;
use std::path::Path;

/// Resolves a dropped or picked path (file or shortcut) to an executable target.
pub fn resolve_path(path_str: &str) -> Result<ResolvedTarget, String> {
    let trimmed = path_str.trim().trim_matches('"');
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err(format!("指定的文件不存在: {}", trimmed));
    }

    if path.is_dir() {
        return Err("拖拽目标为目录，请拖拽 .exe 或 .lnk 文件".to_string());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "url" => Err("不支持网页快捷方式 (.url)".to_string()),
        "appref-ms" => Err("不支持 ClickOnce 快捷方式 (.appref-ms)".to_string()),
        "exe" | "bat" | "cmd" => resolve_direct_executable(path, &ext),
        "lnk" => resolve_shell_link(path),
        other => Err(format!(
            "不支持的文件类型 (.{})，仅支持 .exe, .lnk, .bat, .cmd",
            other
        )),
    }
}

fn resolve_direct_executable(path: &Path, ext: &str) -> Result<ResolvedTarget, String> {
    let abs_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());

    let exe_str = strip_extended_prefix(&abs_path);

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "App".to_string());

    let cwd = abs_path
        .parent()
        .map(|p| strip_extended_prefix(p))
        .unwrap_or_default();

    let icon = if ext == "exe" {
        exe_str.clone()
    } else {
        String::new()
    };

    Ok(ResolvedTarget {
        exe: exe_str,
        name,
        args: String::new(),
        cwd,
        icon,
        needs_elevation: false,
        kind: ext.to_string(),
        parse_note: "直接可执行文件".to_string(),
        source: "EXE".to_string(),
    })
}

fn strip_extended_prefix(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

#[cfg(windows)]
fn resolve_shell_link(path: &Path) -> Result<ResolvedTarget, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{
        IShellLinkDataList, IShellLinkW, ShellLink,
    };

    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let guard = unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        if hr == S_OK || hr == S_FALSE {
            ComGuard(true)
        } else if hr == RPC_E_CHANGED_MODE {
            ComGuard(false)
        } else {
            return Err(format!("COM 初始化失败: 0x{:08X}", hr.0));
        }
    };

    let link: IShellLinkW = unsafe {
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("创建 IShellLinkW 失败: {}", e))?
    };

    let persist: IPersistFile = link
        .cast()
        .map_err(|e| format!("查询 IPersistFile 失败: {}", e))?;

    let to_wide = |s: &std::ffi::OsStr| -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    };
    let path_w = to_wide(path.as_os_str());

    unsafe {
        persist
            .Load(PCWSTR(path_w.as_ptr()), STGM_READ)
            .map_err(|e| format!("加载快捷方式失败: {}", e))?;
    }

    let mut target_buf = [0u16; 1024];
    let mut find_data = Default::default();
    let mut target_str = String::new();
    let source = "GetPath".to_string();

    let hr_path = unsafe {
        link.GetPath(
            &mut target_buf,
            &mut find_data,
            0,
        )
    };

    if hr_path.is_ok() {
        let len = target_buf.iter().position(|&c| c == 0).unwrap_or(target_buf.len());
        target_str = String::from_utf16_lossy(&target_buf[..len]);
    }

    if target_str.to_lowercase().contains("shell:appsfolder")
        || target_str.to_lowercase().contains("windowsapps")
    {
        return Err("不支持 UWP / Windows Store 应用快捷方式".to_string());
    }

    if target_str.is_empty() {
        return Err("快捷方式目标路径为空或无法解析".to_string());
    }

    let mut args_buf = [0u16; 2048];
    let args_str = unsafe {
        if link.GetArguments(&mut args_buf).is_ok() {
            let len = args_buf.iter().position(|&c| c == 0).unwrap_or(args_buf.len());
            String::from_utf16_lossy(&args_buf[..len])
        } else {
            String::new()
        }
    };

    let mut dir_buf = [0u16; 1024];
    let mut cwd_str = unsafe {
        if link.GetWorkingDirectory(&mut dir_buf).is_ok() {
            let len = dir_buf.iter().position(|&c| c == 0).unwrap_or(dir_buf.len());
            String::from_utf16_lossy(&dir_buf[..len])
        } else {
            String::new()
        }
    };

    if cwd_str.is_empty() {
        if let Some(parent) = Path::new(&target_str).parent() {
            cwd_str = parent.to_string_lossy().to_string();
        }
    }

    let mut icon_buf = [0u16; 1024];
    let mut icon_index = 0i32;
    let icon_str = unsafe {
        if link.GetIconLocation(&mut icon_buf, &mut icon_index).is_ok() {
            let len = icon_buf.iter().position(|&c| c == 0).unwrap_or(icon_buf.len());
            let loc = String::from_utf16_lossy(&icon_buf[..len]);
            if !loc.is_empty() {
                loc
            } else {
                target_str.clone()
            }
        } else {
            target_str.clone()
        }
    };

    let mut needs_elevation = false;
    if let Ok(data_list) = link.cast::<IShellLinkDataList>() {
        if let Ok(flags) = unsafe { data_list.GetFlags() } {
            if (flags & 0x00002000) != 0 {
                needs_elevation = true;
            }
        }
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "App".to_string());

    let target_ext = Path::new(&target_str)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("exe")
        .to_lowercase();

    drop(guard);

    Ok(ResolvedTarget {
        exe: target_str,
        name,
        args: args_str,
        cwd: cwd_str,
        icon: icon_str,
        needs_elevation,
        kind: target_ext,
        parse_note: format!("从快捷方式解析 ({})", source),
        source,
    })
}

#[cfg(not(windows))]
fn resolve_shell_link(_path: &Path) -> Result<ResolvedTarget, String> {
    Err("快捷方式解析仅支持 Windows 平台".to_string())
}
