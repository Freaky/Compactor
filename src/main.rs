// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod background;
mod compact;
mod compression;
mod compresstimate;
mod filesdb;
mod folder;
mod gui;
mod settings;
mod persistence;

fn main() {
    if let Err(e) = std::panic::catch_unwind(gui::spawn_gui) {
        eprintln!("Unhandled panic: {:?}", e);

        std::process::exit(1);
    }
}
