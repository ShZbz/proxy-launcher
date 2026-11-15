#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(pos) = args.iter().position(|a| a == "--launch-rule") {
        let exit_code = proxy_launcher_lib::headless::run_headless_launch(&args, pos);
        std::process::exit(exit_code);
    }

    if let Some(pos) = args.iter().position(|a| a == "--create-shortcut") {
        let exit_code = proxy_launcher_lib::headless::run_headless_create_shortcut(&args, pos);
        std::process::exit(exit_code);
    }

    proxy_launcher_lib::run();
}
