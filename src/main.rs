#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod backend;
mod background;
mod compact;
mod compression;
mod config;
mod folder;
mod gui;
mod persistence;

use winapi::um::wincon::{AttachConsole, FreeConsole, ATTACH_PARENT_PROCESS};

fn main() {
    let mut rc = 0;

    // Enable console printing with Windows subsystem
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    if let Err(e) = std::panic::catch_unwind(gui::spawn_gui) {
        eprintln!("Unhandled panic: {:?}", e);
        rc = 1;
    }

    unsafe {
        FreeConsole();
    }

    std::process::exit(rc);
}
