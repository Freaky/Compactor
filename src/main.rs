#![windows_subsystem = "windows"]

mod backend;
mod background;
mod compact;
mod compression;
mod folder;
mod gui;

fn main() {
    gui::spawn_gui();
}
