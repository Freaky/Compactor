// #![windows_subsystem = "windows"]

mod backend;
mod background;
mod compact;
mod folder;
mod gui;

use crate::compact::Compression;
use crate::folder::FolderInfo;

use std::path::PathBuf;

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
    gui::spawn_gui();
}
