#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case)]

mod backend;
mod background;
mod compact;
mod compression;
mod folder;
mod gui;
mod persistence;
mod config;

fn main() {
    if let Err(e) = std::panic::catch_unwind(gui::spawn_gui) {
        eprintln!("Unhandled panic: {:?}", e);

        std::process::exit(1);
    }
}
