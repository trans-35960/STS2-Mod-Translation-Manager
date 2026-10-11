#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod dto;
mod fs_ops;
mod prompt_pdf;
mod services;

fn main() {
    app::run();
}
