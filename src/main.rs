// #![windows_subsystem = "windows"]

mod gui;
mod compact;
mod folder;

use crate::folder::FolderInfo;
use crate::compact::Compression;

use std::path::PathBuf;

use crossbeam_channel::{bounded, Sender, Receiver};

pub enum GuiActions {
    SetCompression(Compression),
    SelectFolder(PathBuf),
    Compress,
    Decompress,
    Pause,
    Continue,
    Cancel,
    Quit
}

pub enum GuiResponses {
    FolderStatus(FolderInfo),
    Output(String),
    Exit
}

fn spawn_worker(background_rx: Receiver<GuiActions>, gui_tx: Sender<GuiResponses>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {
        let mut compression = Compression::default();

        for action in background_rx {
            match action {
                GuiActions::SetCompression(comp) => { compression = comp; },
                GuiActions::SelectFolder(path) => {},
                GuiActions::Compress => {},
                GuiActions::Decompress => {},
                GuiActions::Pause => {},
                GuiActions::Continue => {},
                GuiActions::Cancel => {},
                GuiActions::Quit => {},
            }
        }
    })
}

fn main() {
    /*
    let fi = FolderInfo::evaluate("D:\\Games\\Steam\\steamapps\\common\\AI War 2");
    println!("{} compressible ({} bytes), {} compressed ({} -> {} bytes), {} incompressible ({} bytes)",
             fi.compressable.len(),
             fi.compressable.iter().map(|f| f.logical_size).sum::<u64>(),
             fi.compressed.len(),
             fi.compressed.iter().map(|f| f.logical_size).sum::<u64>(),
             fi.compressed.iter().map(|f| f.physical_size).sum::<u64>(),
             fi.skipped.len(),
             fi.skipped.iter().map(|f| f.logical_size).sum::<u64>()
             );

    for file in fi.compressed.iter().take(10) {
        println!("{} ({:.2}x, {} -> {})", file.path.display(), file.physical_size as f64 / file.logical_size as f64, file.logical_size, file.physical_size);
    }
    */

    let (background_tx, background_rx) = bounded::<GuiActions>(1024);
    let (gui_tx, gui_rx) = bounded::<GuiResponses>(1024);

    let worker = spawn_worker(background_rx, gui_tx);
    gui::spawn_gui(background_tx, gui_rx);
    worker.join().unwrap();
}
