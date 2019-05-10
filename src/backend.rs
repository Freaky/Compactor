use crate::background::BackgroundHandle;
use crate::compact;
use crate::compact::{BackgroundCompactor, Compact, Compacted};
use crate::folder::{FolderInfo, FolderScan, FolderSummary};
use crate::gui::{GuiRequest, GuiResponse, GuiWrapper};
use std::path::PathBuf;
use std::time::Instant;

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError};

use std::time::Duration;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>,
    info: Option<FolderInfo>,
}

enum Mode {
    Compress,
    Decompress,
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
                    let path = self
                        .gui
                        .choose_folder()
                        .recv()
                        .ok()
                        .and_then(Result::ok)
                        .and_then(|x| x);

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
                    // self.compact_loop(Mode::Decompress);
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
        let scanner = FolderScan::new(path);
        let task = BackgroundHandle::spawn(scanner);
        let start = Instant::now();

        self.gui.status("Scanning", None);
        loop {
            let msg = self.msg.recv_timeout(Duration::from_millis(25));

            match msg {
                Ok(GuiRequest::Pause) => {
                    task.pause();
                    self.gui.status("Paused", Some(0.5));
                    self.gui.paused();
                }
                Ok(GuiRequest::Resume) => {
                    task.resume();
                    self.gui.status("Scanning", None);
                    self.gui.resumed();
                }
                Ok(GuiRequest::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    task.cancel();
                }
                Ok(msg) => {
                    eprintln!("Ignored message: {:?}", msg);
                }
                Err(RecvTimeoutError::Timeout) => (),
            }

            match task.wait_timeout(Duration::from_millis(25)) {
                Some(Ok(info)) => {
                    self.gui
                        .status(format!("Scanned in {:.2?}", start.elapsed()), Some(1.0));
                    self.gui.summary(info.summary());
                    self.gui.scanned();
                    self.info = Some(info);
                    break;
                }
                Some(Err(info)) => {
                    self.gui
                        .status(format!("Stopped after {:.2?}", start.elapsed()), Some(0.5));
                    self.gui.summary(info.summary());
                    self.gui.stopped();
                    self.info = Some(info);
                    break;
                }
                None => {
                    if let Some(status) = task.status() {
                        self.gui.summary(status);
                    }
                }
            }
        }
    }

    fn compress_loop(&mut self) {
        let (send_file, send_file_rx) = bounded::<Option<PathBuf>>(512);
        let (recv_result_tx, recv_result) = bounded::<Compacted>(512);

        let compact = Compact::default();
        let compactor = BackgroundCompactor::new(send_file_rx, recv_result_tx, compact);
        let task = BackgroundHandle::spawn(compactor);
        let start = Instant::now();

        let mut fi = self.info.take().expect("fileinfo");
        let mut idx = 0;
        let mut done = 0;
        let total = fi.compressible.files.len();

        self.gui.status("Compacting".to_string(), Some(0.0));
        loop {
            while idx < total
                && send_file
                    .try_send(Some(fi.path.join(&fi.compressible.files[idx].path)))
                    .is_ok()
            {
                idx += 1;

                if idx == total {
                    send_file.send(None).unwrap();
                }
            }

            let msg = self.msg.recv_timeout(Duration::from_millis(25));
            match msg {
                Ok(GuiRequest::Pause) => {
                    task.pause();
                    self.gui
                        .status("Pausing".to_string(), Some(done as f32 / total as f32));
                    self.gui.paused();
                }
                Ok(GuiRequest::Resume) => {
                    task.resume();
                    self.gui
                        .status("Compacting".to_string(), Some(done as f32 / total as f32));
                    self.gui.resumed();
                }
                Ok(GuiRequest::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    task.cancel();
                }
                Ok(msg) => {
                    eprintln!("Ignored message: {:?}", msg);
                }
                Err(RecvTimeoutError::Timeout) => (),
            }

            while let Ok(completed) = recv_result.try_recv() {
                done += 1;
                fi.compressible.physical_size -= completed.old_size;
                fi.compressible.logical_size -= completed.old_size;

                fi.compressed.physical_size += completed.old_size;
                fi.compressed.logical_size += completed.new_size;

                fi.physical_size -= completed.old_size;
                fi.physical_size += completed.new_size;
            }

            self.gui
                .status("Compacting".to_string(), Some(done as f32 / total as f32));
            self.gui.summary(fi.summary());

            match task.wait_timeout(Duration::from_millis(25)) {
                Some(Ok(())) => {
                    self.gui.status(
                        format!("Compacted in {:.2?}", start.elapsed()),
                        Some(done as f32 / total as f32),
                    );
                    self.gui.summary(fi.summary());
                    self.gui.scanned();
                    self.info = Some(fi);
                    break;
                }
                Some(Err(msg)) => {
                    eprintln!("Error: {}", msg);
                    self.gui.status(
                        format!("Stopped after {:.2?}", start.elapsed()),
                        Some(done as f32 / total as f32),
                    );
                    self.gui.summary(fi.summary());
                    self.gui.stopped();
                    self.info = Some(fi);
                    break;
                }
                None => (),
            }
        }
    }
}
