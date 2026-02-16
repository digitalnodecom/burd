// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Run the Tauri app
    // Note: The HTTPS proxy is now handled by Caddy, not a built-in daemon
    burd_lib::run()
}
