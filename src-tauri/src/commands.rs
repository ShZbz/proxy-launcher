//! Tauri IPC command implementations bridging to `launcher-core`.

use crate::dto::{LaunchResult, ProbeResult, ResolvedTarget};
use launcher_core::models::{Config, Port};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;

/// Persistent application state for Tauri.
pub struct AppState {
    pub custom_config_path: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            custom_config_path: Mutex::new(None),
        }
    }

    pub fn with_config_path(path: PathBuf) -> Self {
        Self {
            custom_config_path: Mutex::new(Some(path)),
        }
    }

    pub fn get_config_path(&self) -> Result<PathBuf, String> {
        let guard = self.custom_config_path.lock().map_err(|e| e.to_string())?;
        if let Some(ref p) = *guard {
            return Ok(p.clone());
        }
        drop(guard);
        crate::headless::resolve_config_path(None)
    }

    pub fn set_config_path(&self, path: PathBuf) -> Result<(), String> {
        let mut guard = self.custom_config_path.lock().map_err(|e| e.to_string())?;
        *guard = Some(path);
        Ok(())
    }
}

pub fn get_config_internal(state: &AppState) -> Result<Config, String> {
    match state.get_config_path() {
        Ok(path) => {
            if path.exists() {
                launcher_core::config::load_from_path(&path)
                    .map_err(|e| format!("加载配置文件失败: {}", e))
            } else {
                Ok(Config::default())
            }
        }
        Err(_) => Ok(Config::default()),
    }
}

pub fn save_config_internal(config: Config, state: &AppState) -> Result<(), String> {
    let path = match state.get_config_path() {
        Ok(p) => p,
        Err(_) => {
            let default_path = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("config.json");
            let _ = state.set_config_path(default_path.clone());
            default_path
        }
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    launcher_core::config::save_atomic(&config, &path)
        .map_err(|e| format!("保存配置文件失败: {}", e))
}

pub fn probe_port_sync(port: &Port, test_url: Option<&str>) -> ProbeResult {
    let local_timeout = std::time::Duration::from_millis(50);
    if let Err(e) = launcher_core::probe::probe_local_port(&port.host, port.port, local_timeout) {
        return ProbeResult {
            host: port.host.clone(),
            port: port.port,
            ok: false,
            reason: "PORT_CLOSED".to_string(),
            reason_text: "端口未监听（代理没开？）".to_string(),
            detail: format!("TCP 连接失败: {}", e),
            latency_ms: None,
            http_code: None,
            exit_ip: None,
        };
    }

    let url = test_url.unwrap_or(launcher_core::models::DEFAULT_TEST_URL);
    let http_timeout = std::time::Duration::from_millis(1500);
    let start = std::time::Instant::now();

    match launcher_core::probe::probe_upstream_http(&port.host, port.port, url, http_timeout) {
        Ok(code) => {
            let latency = start.elapsed().as_millis() as u64;
            ProbeResult {
                host: port.host.clone(),
                port: port.port,
                ok: true,
                reason: "OK".to_string(),
                reason_text: "正常".to_string(),
                detail: format!("HTTP {} 延迟 {}ms", code, latency),
                latency_ms: Some(latency),
                http_code: Some(code),
                exit_ip: None,
            }
        }
        Err(e) => ProbeResult {
            host: port.host.clone(),
            port: port.port,
            ok: false,
            reason: "HTTP_FAILED".to_string(),
            reason_text: "节点异常".to_string(),
            detail: format!("上游测试失败: {}", e),
            latency_ms: None,
            http_code: None,
            exit_ip: None,
        },
    }
}

pub fn launch_rule_internal(rule_id: &str, state: &AppState) -> Result<LaunchResult, String> {
    let config = get_config_internal(state)?;
    let exe_path = config
        .find_rule(rule_id)
        .map(|r| r.exe.clone())
        .unwrap_or_default();

    match launcher_core::launcher::launch_rule(&config, rule_id) {
        Ok(pid) => Ok(LaunchResult {
            pid,
            exe: exe_path,
            ok: true,
            message: format!("已成功启动应用 (PID: {})", pid),
        }),
        Err(e) => Err(format!("启动规则失败: {}", e)),
    }
}

pub fn create_shortcut_internal(rule_id: &str, state: &AppState) -> Result<String, String> {
    let config = get_config_internal(state)?;
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("获取当前可执行文件路径失败: {}", e))?;

    let path = launcher_core::lnk::create_shortcut_for_rule(&config, rule_id, &current_exe)
        .map_err(|e| format!("创建桌面快捷方式失败: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

pub fn launch_terminal_internal(port_id: &str, shell: &str, state: &AppState) -> Result<(), String> {
    let config = get_config_internal(state)?;
    let port = config
        .find_port(port_id)
        .ok_or_else(|| format!("未找到指定的代理端口配置: {}", port_id))?;

    let norm_shell = shell.trim().to_lowercase();
    let mut cmd = match norm_shell.as_str() {
        "powershell" | "pwsh" => {
            let bin = if norm_shell == "pwsh" { "pwsh.exe" } else { "powershell.exe" };
            let mut c = std::process::Command::new(bin);
            let command_str = format!(
                "$Host.UI.RawUI.WindowTitle = 'Proxy Terminal ({}:{})'; Write-Host '[Proxy Launcher] 已注入代理环境 -> http://{}:{}' -ForegroundColor Cyan",
                port.host, port.port, port.host, port.port
            );
            c.args(["-NoLogo", "-NoExit", "-Command", &command_str]);
            c
        }
        "cmd" => {
            let mut c = std::process::Command::new("cmd.exe");
            let banner = format!(
                "title Proxy Terminal ({}:{}) & echo [Proxy Launcher] 已注入代理端口: http://{}:{}",
                port.host, port.port, port.host, port.port
            );
            c.args(["/k", &banner]);
            c
        }
        other => return Err(format!("不支持的终端类型: {}，仅支持 powershell 或 cmd", other)),
    };

    launcher_core::launcher::inject_proxy_env(&mut cmd, &port.host, port.port);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000010); // CREATE_NEW_CONSOLE
    }

    cmd.spawn()
        .map_err(|e| format!("启动终端进程失败: {}", e))?;

    Ok(())
}

// ----------------------------------------------------------------------------
// Tauri Command Handlers
// ----------------------------------------------------------------------------

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    get_config_internal(&state)
}

#[tauri::command]
pub fn save_config(config: Config, state: State<'_, AppState>) -> Result<(), String> {
    save_config_internal(config, &state)
}

#[tauri::command]
pub async fn probe_port(port: Port) -> Result<ProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || probe_port_sync(&port, None))
        .await
        .map_err(|e| format!("Probe task failed: {}", e))
}

#[tauri::command]
pub async fn probe_all_ports(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ProbeResult>, String> {
    let config = get_config_internal(&state)?;
    let test_url = config.test_url.clone();
    let ports = config.ports.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let results = Arc::new(Mutex::new(HashMap::new()));
        let mut handles = Vec::new();

        for port in ports {
            let results_clone = Arc::clone(&results);
            let test_url_clone = test_url.clone();
            handles.push(std::thread::spawn(move || {
                let res = probe_port_sync(&port, Some(&test_url_clone));
                if let Ok(mut map) = results_clone.lock() {
                    map.insert(port.id.clone(), res);
                }
            }));
        }

        for h in handles {
            let _ = h.join();
        }

        Arc::try_unwrap(results)
            .map(|m| m.into_inner().unwrap_or_default())
            .unwrap_or_default()
    })
    .await
    .map_err(|e| format!("Probe all task failed: {}", e))
}

#[tauri::command]
pub fn launch_rule(rule_id: String, state: State<'_, AppState>) -> Result<LaunchResult, String> {
    launch_rule_internal(&rule_id, &state)
}

#[tauri::command]
pub fn create_shortcut(rule_id: String, state: State<'_, AppState>) -> Result<String, String> {
    create_shortcut_internal(&rule_id, &state)
}

#[tauri::command]
pub fn launch_terminal(
    port_id: String,
    shell: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    launch_terminal_internal(&port_id, &shell, &state)
}

#[tauri::command]
pub fn pick_executable_dialog() -> Result<Option<String>, String> {
    let file = rfd::FileDialog::new()
        .set_title("选择可执行文件或快捷方式")
        .add_filter("应用程序 / 快捷方式", &["exe", "lnk", "bat", "cmd"])
        .add_filter("所有文件", &["*"])
        .pick_file();
    Ok(file.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn resolve_dropped_path(path: String) -> Result<ResolvedTarget, String> {
    crate::lnk_parser::resolve_path(&path)
}
