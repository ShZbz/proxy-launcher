//! Main library entry point for Proxy Launcher desktop application.

pub mod commands;
pub mod dto;
pub mod headless;
pub mod lnk_parser;

pub use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::probe_port,
            commands::probe_all_ports,
            commands::launch_rule,
            commands::create_shortcut,
            commands::launch_terminal,
            commands::pick_executable_dialog,
            commands::resolve_dropped_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
