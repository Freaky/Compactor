use std::io;
use std::mem;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crossbeam_channel::{bounded, Receiver, RecvError};
use filesize::PathExt;

use crate::background::BackgroundHandle;
use crate::compression::BackgroundCompactor;
use crate::folder::{FileKind, FolderInfo, FolderScan};
use crate::gui::{GuiRequest, GuiWrapper};
use crate::persistence::{config, pathdb};
use std::collections::HashMap;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>,
    info: Option<FolderInfo>,
}

fn format_size(size: u64, decimal: bool) -> String {
    use humansize::{file_size_opts as options, FileSize};

    size.file_size(if decimal {
        options::DECIMAL
    } else {
        options::BINARY
    })
    .expect("file size")
}

impl<T> Backend<T> {
    pub fn new(gui: GuiWrapper<T>, msg: Receiver<GuiRequest>) -> Self {
        Self {
            gui,
            msg,
            info: None,
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.msg.recv() {
                Ok(GuiRequest::ChooseFolder) => {
                    let path = self.gui.choose_folder().recv().ok().flatten();

                    if let Some(path) = path {
                        self.gui.folder(&path);
                        self.scan_loop(path);
                    }
                }
                Ok(GuiRequest::Analyse) if self.info.is_some() => {
                    let path = self.info.take().unwrap().path;
                    self.gui.folder(&path);
                    self.scan_loop(path);
                }
                Ok(GuiRequest::Compress) if self.info.is_some() => {
                    self.compress_loop();
                }
                Ok(GuiRequest::Decompress) if self.info.is_some() => {
                    self.uncompress_loop();
                }
                Ok(msg) => {
                    eprintln!("Backend: Ignored message: {:?}", msg);
                }
                Err(_) => {
                    eprintln!("Backend: exit run loop");
                    break;
                }
            }
        }
    }

    fn scan_loop(&mut self, path: PathBuf) {
        let excludes = config().read().unwrap().current().globset().expect("globs");

        let scanner = FolderScan::new(path, excludes);
        let task = BackgroundHandle::spawn(scanner);
        let start = Instant::now();

        let mut paused = false;
        self.gui.status("Scanning", None);
        loop {
            let display = if paused {
                crossbeam_channel::never()
            } else {
                crossbeam_channel::after(Duration::from_millis(50))
            };
            crossbeam_channel::select! {
                recv(self.msg) -> msg => match msg {
                    Ok(GuiRequest::Pause) => {
                        task.pause();
                        self.gui.status("Paused", Some(0.5));
                        self.gui.paused();
                        paused = true;
                    }
                    Ok(GuiRequest::Resume) => {
                        task.resume();
                        self.gui.status("Scanning", None);
                        self.gui.resumed();
                        paused = false;
                    }
                    Ok(GuiRequest::Stop) | Err(RecvError) => {
                        task.cancel();
                    }
                    Ok(msg) => {
                        eprintln!("Ignored message: {:?}", msg);
                    }
                },
                recv(task.result_chan()) -> msg => match msg.unwrap() {
                    Ok(Ok(info)) => {
                        self.gui
                            .status(format!("Scanned in {:.2?}", start.elapsed()), Some(1.0));
                        self.gui.summary(info.summary());
                        self.gui.scanned();
                        self.info = Some(info);
                        break;
                    }
                    Ok(Err(info)) => {
                        self.gui.status(
                            format!("Scan stopped after {:.2?}", start.elapsed()),
                            Some(0.5),
                        );
                        self.gui.summary(info.summary());
                        self.gui.stopped();
                        self.info = Some(info);
                        break;
                    }
                    Err(e) => {
                        let err_str: &str;
                        if let Some(s) = e.downcast_ref::<&str>() {
                            err_str = s;
                        } else if let Some(s) = e.downcast_ref::<String>() {
                            err_str = s;
                        } else {
                            err_str = "Unknown error";
                        }
                        self.gui.status(format!("Error occurred: {}", err_str), Some(0.5));
                        self.gui.stopped();
                        break;
                    }
                },
                recv(display) -> _ => {
                    if let Some(status) = task.status() {
                        self.gui
                            .status(format!("Scanning: {}", status.0.display()), None);
                        self.gui.summary(status.1);
                    }
                }
            }
        }
    }

    // Ph'nglui mglw'nafh Cthulhu R'lyeh wgah'nagl fhtagn.
    fn compress_loop(&mut self) {
        let (send_file, send_file_rx) = bounded::<(PathBuf, u64)>(0);
        let (recv_result_tx, recv_result) = bounded::<(PathBuf, io::Result<bool>)>(1);

        let compression = Some(config().read().unwrap().current().compression);
        let compactor = BackgroundCompactor::new(compression, send_file_rx, recv_result_tx);
        let task = BackgroundHandle::spawn(compactor);
        let start = Instant::now();

        let mut folder = self.info.take().expect("fileinfo");
        let total = folder.len(FileKind::Compressible);
        let mut done = 0;

        // Option to allow easy mapping
        let mut running = Some(());

        let old_size = folder.physical_size;
        let compressible_size = folder.summary().compressible.physical_size;

        let incompressible = pathdb();
        let mut incompressible = incompressible.write().unwrap();
        let _ = incompressible.load();

        self.gui.compacting();

        self.gui.status("Compacting".to_string(), Some(0.0));

        let mut file_infos = HashMap::new();
        let mut next_fi = folder.pop(FileKind::Compressible);
        let mut last_path = PathBuf::from("None");

        let save_incompressible = crossbeam_channel::tick(Duration::from_secs(60));
        let display = crossbeam_channel::tick(Duration::from_millis(50));

        // Use an option, so we can set it to None when there is nothing to send
        let mut send_file = Some(send_file);

        loop {
            if next_fi.is_none() {
                send_file = None;
            }

            let mut select = crossbeam_channel::Select::new();

            let gui_idx = select.recv(&self.msg);
            let result_idx = running.map(|_| select.recv(&recv_result));
            let save_idx = running.map(|_| select.recv(&save_incompressible));
            let display_idx = running.map(|_| select.recv(&display));
            let send_idx =
                running.and_then(|_| send_file.as_ref().map(|sender| select.send(sender)));

            let oper = select.select();
            let oper_idx = oper.index();

            if oper_idx == gui_idx {
                match oper.recv(&self.msg) {
                    Ok(GuiRequest::Pause) => {
                        if running.is_some() {
                            self.gui
                                .status("Paused".to_string(), Some(done as f32 / total as f32));
                            self.gui.paused();
                            running = None;
                        }
                    }
                    Ok(GuiRequest::Resume) => {
                        self.gui
                            .status("Compacting".to_string(), Some(done as f32 / total as f32));
                        self.gui.resumed();
                        running = Some(());
                    }
                    Ok(GuiRequest::Stop) | Err(crossbeam_channel::RecvError) => {
                        self.gui.status(
                            format!("Stopping after {}", last_path.display()),
                            Some(done as f32 / total as f32),
                        );
                        self.gui.stopped();
                        // Close the sender, we'll stop when we drain the remaining results
                        send_file = None;
                        next_fi = None;
                        // Resume, so we drain the remainder of the results
                        running = Some(());
                    }
                    Ok(msg) => {
                        eprintln!("Ignored message: {:?}", msg);
                    }
                }
            } else if Some(oper_idx) == send_idx {
                let send_file = send_file
                    .as_ref()
                    .expect("Shouldn't drop sender until there are no more files");
                let fi = mem::replace(&mut next_fi, folder.pop(FileKind::Compressible));
                let fi = fi.expect(
                    "Should have disabled sending if there was no current file info to send",
                );

                let full_path = folder.path.join(&fi.path);
                oper.send(send_file, (full_path.clone(), fi.logical_size))
                    .expect("Worker shouldn't quit until we send it everything");
                last_path = fi.path.clone();
                file_infos.insert(full_path, fi);
            } else if Some(oper_idx) == result_idx {
                let (path, result) = match oper.recv(&recv_result) {
                    Ok(x) => x,
                    Err(crossbeam_channel::RecvError) => break,
                };
                done += 1;
                let mut fi = file_infos
                    .remove(&path)
                    .expect("Should only get a result from a path we passed");
                match result {
                    Ok(true) => {
                        fi.physical_size = path.size_on_disk().unwrap_or(fi.physical_size);

                        // Irritatingly Windows can return success when it fails.
                        if fi.physical_size == fi.logical_size {
                            incompressible.insert(path);
                            folder.push(FileKind::Skipped, fi);
                        } else {
                            folder.push(FileKind::Compressed, fi);
                        }
                    }
                    Ok(false) => {
                        incompressible.insert(path);
                        folder.push(FileKind::Skipped, fi);
                    }
                    Err(err) => {
                        self.gui.status(
                            format!("Error: {}, {}", err, fi.path.display()),
                            Some(done as f32 / total as f32),
                        );
                        folder.push(FileKind::Skipped, fi);
                    }
                }
            } else if Some(oper_idx) == save_idx {
                let _ = oper.recv(&save_incompressible);
                let _ = incompressible.save();
            } else if Some(oper_idx) == display_idx {
                let _ = oper.recv(&display);
                self.gui.status(
                    format!("Compacting: {}", last_path.display()),
                    Some(done as f32 / total as f32),
                );
                self.gui.summary(folder.summary());
            }
        }

        drop(send_file);
        drop(recv_result);
        task.wait();

        let _ = incompressible.save();

        let new_size = folder.physical_size;
        let decimal = config().read().unwrap().current().decimal;

        let msg = format!(
            "Compacted {} in {} files, saving {} in {:.2?}",
            format_size(compressible_size, decimal),
            done,
            format_size(old_size - new_size, decimal),
            start.elapsed()
        );

        self.gui.status(msg, Some(done as f32 / total as f32));
        self.gui.summary(folder.summary());
        self.gui.scanned();

        self.info = Some(folder);
    }

    // Oh no, not again.
    fn uncompress_loop(&mut self) {
        let (send_file, send_file_rx) = bounded::<(PathBuf, u64)>(0);
        let (recv_result_tx, recv_result) = bounded::<(PathBuf, io::Result<bool>)>(1);

        let compactor = BackgroundCompactor::new(None, send_file_rx, recv_result_tx);
        let task = BackgroundHandle::spawn(compactor);
        let start = Instant::now();

        let mut folder = self.info.take().expect("fileinfo");
        let total = folder.len(FileKind::Compressed);
        let mut done = 0;

        // Option to allow easy mapping
        let mut running = Some(());

        let old_size = folder.physical_size;

        self.gui.compacting();

        self.gui.status("Expanding".to_string(), Some(0.0));

        let mut file_infos = HashMap::new();
        let mut next_fi = folder.pop(FileKind::Compressed);
        let mut last_path = PathBuf::from("None");

        let display = crossbeam_channel::tick(Duration::from_millis(50));

        // Use an option, so we can set it to None when there is nothing to send
        let mut send_file = Some(send_file);

        loop {
            if next_fi.is_none() {
                send_file = None;
            }

            let mut select = crossbeam_channel::Select::new();

            let gui_idx = select.recv(&self.msg);
            let result_idx = running.map(|_| select.recv(&recv_result));
            let display_idx = running.map(|_| select.recv(&display));
            let send_idx =
                running.and_then(|_| send_file.as_ref().map(|sender| select.send(sender)));

            let oper = select.select();
            let oper_idx = oper.index();

            if oper_idx == gui_idx {
                match oper.recv(&self.msg) {
                    Ok(GuiRequest::Pause) => {
                        if running.is_some() {
                            self.gui
                                .status("Paused".to_string(), Some(done as f32 / total as f32));
                            self.gui.paused();
                            running = None;
                        }
                    }
                    Ok(GuiRequest::Resume) => {
                        self.gui
                            .status("Expanding".to_string(), Some(done as f32 / total as f32));
                        self.gui.resumed();
                        running = Some(());
                    }
                    Ok(GuiRequest::Stop) | Err(crossbeam_channel::RecvError) => {
                        self.gui.status(
                            format!("Stopping after {}", last_path.display()),
                            Some(done as f32 / total as f32),
                        );
                        self.gui.stopped();
                        // Close the sender, we'll stop when we drain the remaining results
                        send_file = None;
                        next_fi = None;
                        // Resume, so we drain the remainder of the results
                        running = Some(());
                    }
                    Ok(msg) => {
                        eprintln!("Ignored message: {:?}", msg);
                    }
                }
            } else if Some(oper_idx) == send_idx {
                let send_file = send_file
                    .as_ref()
                    .expect("Shouldn't drop sender until there are no more files");
                let fi = mem::replace(&mut next_fi, folder.pop(FileKind::Compressed));
                let fi = fi.expect(
                    "Should have disabled sending if there was no current file info to send",
                );

                let full_path = folder.path.join(&fi.path);
                oper.send(send_file, (full_path.clone(), fi.logical_size))
                    .expect("Worker shouldn't quit until we send it everything");
                last_path = fi.path.clone();
                file_infos.insert(full_path, fi);
            } else if Some(oper_idx) == result_idx {
                let (path, result) = match oper.recv(&recv_result) {
                    Ok(x) => x,
                    Err(crossbeam_channel::RecvError) => break,
                };
                done += 1;
                let mut fi = file_infos
                    .remove(&path)
                    .expect("Should only get a result from a path we passed");
                match result {
                    Ok(_) => {
                        fi.physical_size = fi.logical_size;
                        folder.push(FileKind::Compressible, fi);
                    }
                    Err(err) => {
                        self.gui.status(
                            format!("Error: {}, {}", err, fi.path.display()),
                            Some(done as f32 / total as f32),
                        );
                        folder.push(FileKind::Skipped, fi);
                    }
                }
            } else if Some(oper_idx) == display_idx {
                let _ = oper.recv(&display);
                self.gui.status(
                    format!("Expanding: {}", last_path.display()),
                    Some(done as f32 / total as f32),
                );
                self.gui.summary(folder.summary());
            }
        }

        drop(send_file);
        drop(recv_result);
        task.wait();

        let new_size = folder.physical_size;

        let msg = format!(
            "Expanded {} files wasting {} in {:.2?}",
            done,
            format_size(
                new_size - old_size,
                config().read().unwrap().current().decimal
            ),
            start.elapsed()
        );

        self.gui.status(msg, Some(done as f32 / total as f32));
        self.gui.summary(folder.summary());
        self.gui.scanned();

        self.info = Some(folder);
    }
}
