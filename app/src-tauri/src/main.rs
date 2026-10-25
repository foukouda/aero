//! Entry point of the desktop application.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = aero_app::run() {
        eprintln!("aero failed to start: {error}");
        std::process::exit(1);
    }
}
