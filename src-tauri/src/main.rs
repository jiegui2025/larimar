//! Binary entry point that selects a headless mode before Tauri starts.
//!
//! Neither `--mcp` nor the browser-messaging host may initialize the GUI: both
//! own stdin and stdout, and a window would be a bug the caller cannot see.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--mcp") {
        if let Err(error) = larimar_lib::mcp::run_from_env() {
            eprintln!("Larimar MCP server error: {error}");
            std::process::exit(1);
        }
        return;
    }

    if larimar_lib::browser_host::is_browser_host_launch(&args) {
        if let Err(error) = larimar_lib::browser_host::run() {
            eprintln!("Larimar browser host error: {error}");
            std::process::exit(1);
        }
        return;
    }

    larimar_lib::run();
}
