#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod dto;
mod fs_ops;
mod prompt_pdf;
mod services;

fn main() {
    // The app re-runs itself as a short-lived helper to unsubscribe from
    // Workshop items; see `steam_workshop`.
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = sts2_mod_manager::steam_workshop::run_helper_from_args(&args) {
        std::process::exit(code);
    }
    app::run();
}
