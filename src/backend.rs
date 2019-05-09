use crate::background::BackgroundHandle;
use crate::compact;
use crate::folder::{FolderInfo, FolderScan, FolderSummary};
use crate::gui::{GuiRequest, GuiResponse, GuiWrapper};
use std::path::PathBuf;

use crossbeam_channel::Receiver;

use std::time::Duration;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>,
    info: Option<FolderInfo>,
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
                        self.scan_loop(path);
                    }
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

    pub fn scan_loop(&mut self, path: PathBuf) {
        self.gui.send(&GuiResponse::Folder { path: path.clone() });

        let scanner = FolderScan::new(path);
        let task = BackgroundHandle::spawn(scanner);

        self.gui.send(&GuiResponse::Status {
            status: "Scanning".into(),
            pct: None,
        });
        loop {
            let msg = self.msg.recv_timeout(Duration::from_millis(10));

            match msg {
                Ok(GuiRequest::Pause) => {
                    task.pause();
                    self.gui.send(&GuiResponse::Status {
                        status: "Paused".into(),
                        pct: None,
                    });
                }
                Ok(GuiRequest::Resume) => {
                    task.resume();
                    self.gui.send(&GuiResponse::Status {
                        status: "Scanning".into(),
                        pct: None,
                    });
                }
                Ok(GuiRequest::Stop) => {
                    task.cancel();
                }
                Ok(msg) => {
                    eprintln!("Ignored message: {:?}", msg);
                }
                Err(_) => (),
            }

            match task.wait_timeout(Duration::from_millis(10)) {
                Some(Ok(info)) => {
                    self.gui.send(&GuiResponse::Status {
                        status: "Scanned".into(),
                        pct: Some(1.0),
                    });
                    self.gui.send(&GuiResponse::FolderSummary { info: info.summary() });
                    self.info = Some(info);
                    break;
                }
                Some(Err(info)) => {
                    self.gui.send(&GuiResponse::Status {
                        status: "Stopped".into(),
                        pct: Some(0.5),
                    });
                    self.gui.send(&GuiResponse::FolderSummary { info: info.summary() });
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

    pub fn scanned_idle(&mut self, _info: FolderInfo) {}
}
