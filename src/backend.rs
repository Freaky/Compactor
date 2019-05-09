use crate::background::BackgroundHandle;
use crate::compact;
use crate::folder::{FolderInfo, FolderScan, FolderSummary};
use crate::gui::{GuiRequest, GuiResponse, GuiWrapper};
use std::path::PathBuf;
use std::time::Instant;

use crossbeam_channel::{Receiver, RecvTimeoutError};

use std::time::Duration;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>,
    info: Option<FolderInfo>,
}

enum Mode {
    Compress,
    Decompress
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
                    self.scan_loop(path);
                }
                Ok(GuiRequest::Compress) if self.info.is_some() => {
                    self.compress_loop(Mode::Compress);
                }
                Ok(GuiRequest::Decompress) if self.info.is_some() => {
                    self.compress_loop(Mode::Decompress);
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
            let msg = self.msg.recv_timeout(Duration::from_millis(10));

            match msg {
                Ok(GuiRequest::Pause) => {
                    task.pause();
                    self.gui.status("Paused", None);
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
                Err(RecvTimeoutError::Timeout) => ()
            }

            match task.wait_timeout(Duration::from_millis(10)) {
                Some(Ok(info)) => {
                    self.gui.status(format!("Scanned in {:.2?}", start.elapsed()), Some(1.0));
                    self.gui.summary(info.summary());
                    self.gui.scanned();
                    self.info = Some(info);
                    break;
                }
                Some(Err(info)) => {
                    self.gui.status(format!("Stopped after {:.2?}", start.elapsed()), Some(0.5));
                    self.gui.summary(info.summary());
                    self.gui.stopped();
                    self.info = Some(info);
                    break;
                }
                None => {
                    if let Some(status) = task.status() {
                        eprintln!("Status: {:?}", status);
                        self.gui.send(&GuiResponse::FolderSummary { info: status });
                    }
                }
            }
        }
    }

    fn compress_loop(&mut self, _mode: Mode) {}
}
