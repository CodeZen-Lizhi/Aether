#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod gateway;
mod instance;
mod process;
mod secrets;
mod session;
mod windows;

fn main() {
    if let Err(error) = windows::run() {
        eprintln!("Aether 无法启动：{error}");
        std::process::exit(1);
    }
}
