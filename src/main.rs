#![windows_subsystem = "windows"]

mod filesdb;
mod backend;
mod background;
mod compact;
mod compression;
mod folder;
mod gui;
mod settings;

fn main() {
    gui::spawn_gui();
}
