#![windows_subsystem = "windows"]

mod backend;
mod background;
mod compact;
mod compression;
mod compresstimate;
mod filesdb;
mod folder;
mod gui;
mod settings;

fn main() {
    gui::spawn_gui();
}
