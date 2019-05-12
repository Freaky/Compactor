use std::io;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError};

use crate::background::BackgroundHandle;
use crate::compact::Compression;
use crate::compression::BackgroundCompactor;
use crate::compresstinate::compresstinate;
use crate::filesdb::FilesDb;
use crate::folder::{FileKind, FolderInfo, FolderScan};
use crate::gui::{GuiRequest, GuiWrapper};
use crate::settings::Settings;

pub struct Backend<T> {
    gui: GuiWrapper<T>,
    msg: Receiver<GuiRequest>,
    info: Option<FolderInfo>,
}

fn format_size(size: u64) -> String {
    use humansize::{file_size_opts as options, FileSize};

    size.file_size(options::BINARY).expect("file size")
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
        let settings = Settings::get();

        let scanner = FolderScan::new(path, settings.globset().expect("globs"));
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
                    self.gui.status(
                        format!("Scan stopped after {:.2?}", start.elapsed()),
                        Some(0.5),
                    );
                    self.gui.summary(info.summary());
                    self.gui.stopped();
                    self.info = Some(info);
                    break;
                }
                None => {
                    if let Some(status) = task.status() {
                        self.gui
                            .status(format!("Scanning: {}", status.0.display()), None);
                        self.gui.summary(status.1);
                    }
                }
            }
        }
    }

    fn compress_loop(&mut self) {
        let (send_file, send_file_rx) = bounded::<Option<PathBuf>>(1);
        let (recv_result_tx, recv_result) = bounded::<(PathBuf, io::Result<bool>)>(1);

        let compression = Some(Compression::default());
        let compactor = BackgroundCompactor::new(compression, send_file_rx, recv_result_tx);
        let task = BackgroundHandle::spawn(compactor);
        let start = Instant::now();

        let mut folder = self.info.take().expect("fileinfo");
        let total = folder.len(FileKind::Compressible);
        let mut done = 0;

        let mut last_update = Instant::now();
        let mut paused = false;
        let mut stopped = false;

        let old_size = folder.physical_size;

        let mut incompressible = FilesDb::borrow();

        self.gui.compacting();

        self.gui.status("Compacting".to_string(), Some(0.0));
        loop {
            while paused && !stopped {
                self.gui
                    .status("Paused".to_string(), Some(done as f32 / total as f32));

                self.gui.summary(folder.summary());

                match self.msg.recv() {
                    Ok(GuiRequest::Pause) => {
                        paused = true;
                    }
                    Ok(GuiRequest::Resume) => {
                        self.gui
                            .status("Compacting".to_string(), Some(done as f32 / total as f32));
                        self.gui.resumed();
                        paused = false;
                        last_update = Instant::now();
                    }
                    Ok(GuiRequest::Stop) => {
                        stopped = true;
                        break;
                    }
                    Ok(_) => (),
                    Err(_) => {
                        stopped = true;
                        break;
                    }
                }
            }

            if stopped {
                break;
            }

            let mut displayed = false;

            if let Some(mut fi) = folder.pop(FileKind::Compressible) {
                let fullpath = folder.path.join(&fi.path);
                // XXX: scale ratio to size
                if compresstinate(&fullpath).unwrap() > 0.95
                {
                    if last_update.elapsed() > Duration::from_millis(50) {
                        self.gui.status(
                            format!("Skipping: {}", fi.path.display()),
                            Some(done as f32 / total as f32),
                        );
                    }
                    incompressible.insert(fullpath);
                    folder.push(FileKind::Skipped, fi);
                    done += 1;
                    continue;
                }
                send_file.send(Some(fullpath)).expect("send_file");

                if !displayed && last_update.elapsed() > Duration::from_millis(50) {
                    self.gui.status(
                        format!("Compacting: {}", fi.path.display()),
                        Some(done as f32 / total as f32),
                    );
                    last_update = Instant::now();
                    displayed = true;

                    self.gui.summary(folder.summary());
                }

                loop {
                    if let Ok((path, result)) = recv_result.recv_timeout(Duration::from_millis(25))
                    {
                        done += 1;
                        match result {
                            Ok(true) => {
                                fi.physical_size =
                                    filesize::file_real_size(&path).unwrap_or(fi.physical_size);
                                folder.push(FileKind::Compressed, fi);
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

                        break;
                    }

                    if !displayed && last_update.elapsed() > Duration::from_millis(50) {
                        self.gui.status(
                            format!("Compacting: {}", fi.path.display()),
                            Some(done as f32 / total as f32),
                        );

                        self.gui.summary(folder.summary());
                        last_update = Instant::now();
                        displayed = true;
                    }

                    match self.msg.try_recv() {
                        Ok(GuiRequest::Pause) if !paused => {
                            self.gui.status(
                                format!("Pausing after {}", fi.path.display()),
                                Some(done as f32 / total as f32),
                            );
                            self.gui.paused();
                            paused = true;
                        }
                        Ok(GuiRequest::Resume) => {
                            self.gui.resumed();
                            paused = false;
                            stopped = false;
                        }
                        Ok(GuiRequest::Stop) if !stopped => {
                            self.gui.status(
                                format!("Stopping after {}", fi.path.display()),
                                Some(done as f32 / total as f32),
                            );
                            stopped = true;
                        }
                        Ok(_) => (),
                        Err(_) => (),
                    }
                }
            } else {
                break;
            }
        }

        send_file.send(None).expect("send_file");
        task.wait();

        let new_size = folder.physical_size;

        let msg = format!(
            "Compacted {}/{} files saving {} in {:.2?}",
            done,
            total,
            format_size(old_size - new_size),
            start.elapsed()
        );

        self.gui.status(msg, Some(done as f32 / total as f32));
        self.gui.summary(folder.summary());
        self.gui.scanned();

        self.info = Some(folder);
    }

    // Oh no, not again.
    fn uncompress_loop(&mut self) {
        let (send_file, send_file_rx) = bounded::<Option<PathBuf>>(1);
        let (recv_result_tx, recv_result) = bounded::<(PathBuf, io::Result<bool>)>(1);

        let compactor = BackgroundCompactor::new(None, send_file_rx, recv_result_tx);
        let task = BackgroundHandle::spawn(compactor);
        let start = Instant::now();

        let mut folder = self.info.take().expect("fileinfo");
        let total = folder.len(FileKind::Compressed);
        let mut done = 0;

        let mut last_update = Instant::now();
        let mut paused = false;
        let mut stopped = false;

        let old_size = folder.physical_size;

        self.gui.compacting();

        self.gui.status("Expanding".to_string(), Some(0.0));
        loop {
            while paused && !stopped {
                self.gui
                    .status("Paused".to_string(), Some(done as f32 / total as f32));

                self.gui.summary(folder.summary());

                match self.msg.recv() {
                    Ok(GuiRequest::Pause) => {
                        paused = true;
                    }
                    Ok(GuiRequest::Resume) => {
                        self.gui
                            .status("Expanding".to_string(), Some(done as f32 / total as f32));
                        self.gui.resumed();
                        paused = false;
                        last_update = Instant::now();
                    }
                    Ok(GuiRequest::Stop) => {
                        stopped = true;
                        break;
                    }
                    Ok(_) => (),
                    Err(_) => {
                        stopped = true;
                        break;
                    }
                }
            }

            if stopped {
                break;
            }

            if last_update.elapsed() > Duration::from_millis(50) {
                self.gui
                    .status("Expanding".to_string(), Some(done as f32 / total as f32));
                last_update = Instant::now();

                self.gui.summary(folder.summary());
            }

            if let Some(mut fi) = folder.pop(FileKind::Compressed) {
                send_file
                    .send(Some(folder.path.join(&fi.path)))
                    .expect("send_file");

                let mut waiting = false;
                loop {
                    if let Ok((_path, result)) = recv_result.recv_timeout(Duration::from_millis(25))
                    {
                        done += 1;
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

                        break;
                    }

                    if !waiting && last_update.elapsed() > Duration::from_millis(50) {
                        self.gui.status(
                            format!("Expanding: {}", fi.path.display()),
                            Some(done as f32 / total as f32),
                        );

                        last_update = Instant::now();
                        waiting = true;
                    }

                    match self.msg.try_recv() {
                        Ok(GuiRequest::Pause) if !paused => {
                            self.gui.status(
                                format!("Pausing after {}", fi.path.display()),
                                Some(done as f32 / total as f32),
                            );
                            self.gui.paused();
                            paused = true;
                        }
                        Ok(GuiRequest::Resume) => {
                            self.gui.resumed();
                            paused = false;
                            stopped = false;
                        }
                        Ok(GuiRequest::Stop) if !stopped => {
                            self.gui.status(
                                format!("Stopping after {}", fi.path.display()),
                                Some(done as f32 / total as f32),
                            );
                            stopped = true;
                        }
                        Ok(_) => (),
                        Err(_) => (),
                    }
                }
            } else {
                break;
            }
        }

        send_file.send(None).expect("send_file");
        task.wait();

        let new_size = folder.physical_size;

        let msg = format!(
            "Expanded {}/{} files wasting {} in {:.2?}",
            done,
            total,
            format_size(new_size - old_size),
            start.elapsed()
        );

        self.gui.status(msg, Some(done as f32 / total as f32));
        self.gui.summary(folder.summary());
        self.gui.scanned();

        self.info = Some(folder);
    }
}
