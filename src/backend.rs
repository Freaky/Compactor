
use std::path::PathBuf;
use crate::compact;
use crate::folder::{FolderScan, FolderSummary, FolderInfo};
use crate::background::{BackgroundHandle};
use crate::gui::{GuiWrapper, GuiRequest, GuiResponse};

use crossbeam_channel::Receiver;

use std::time::Duration;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>
}

impl<T> Backend<T> {
    pub fn new(gui: GuiWrapper<T>, msg: Receiver<GuiRequest>) -> Self {
        Self {
            gui,
            msg
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.msg.recv() {
                Ok(GuiRequest::ChooseFolder) => {
                    let path = self.gui.choose_folder().recv().ok().and_then(Result::ok);

                    if let Some(Some(path)) = path {
                        self.scan_loop(path);
                    }
                },
                Ok(msg) => {
                    eprintln!("Backend: Ignored message: {:?}", msg);
                },
                Err(_) => {
                    eprintln!("Backend: exit run loop");
                    break;
                }
            }
        }
    }

    pub fn scan_loop(&mut self, path: PathBuf) {
        self.gui.send(&GuiResponse::Folder { path: path.clone() });

        let scanner = FolderScan::new(path);
        let task = BackgroundHandle::spawn(scanner);

        self.gui.send(&GuiResponse::Status { status: "Scanning".into(), pct: None });
        loop {
            let msg = self.msg.recv_timeout(Duration::from_millis(10));

            match msg {
                Ok(GuiRequest::Pause) => {
                    task.pause();
                    self.gui.send(&GuiResponse::Status { status: "Paused".into(), pct: None });
                    // gui.paused();
                },
                Ok(GuiRequest::Resume) => {
                    task.resume();
                    self.gui.send(&GuiResponse::Status { status: "Scanning".into(), pct: None });
                    // gui.resume()/
                },
                Ok(GuiRequest::Cancel) => {
                    task.cancel();
                },
                Ok(msg) => {
                    eprintln!("Ignored message: {:?}", msg);
                },
                Err(_) => ()
            }

            match task.wait_timeout(Duration::from_millis(10)) {
                Some(Ok(info)) => {
                    self.gui.send(&GuiResponse::Status { status: "Scanned".into(), pct: Some(1.0) });
                    self.scanned_idle(info);
                    break;
                },
                Some(Err(_)) => {
                    self.gui.send(&GuiResponse::Status { status: "Stopped".into(), pct: Some(0.5) });
                    break;
                },
                None => {
                    if let Some(status) = task.status() {
                        eprintln!("Status: {:?}", status);
                        self.gui.send(&GuiResponse::FolderSummary { info: status });
                    }
                }
            }
        }
    }

    pub fn scanned_idle(&mut self, _info: FolderInfo) {

    }
}
