use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver};
use ctrlc;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use web_view::*;
use winapi::shared::winerror;
use winapi::um::combaseapi;
use winapi::um::knownfolders;
use winapi::um::shlobj;
use winapi::um::shtypes;
use winapi::um::winbase;
use winapi::um::winnt;

use crate::backend::Backend;
use crate::compact::system_supports_compression;
use crate::folder::FolderSummary;
use crate::persistence;
use crate::settings::{self, Config};

// messages received from the GUI
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum GuiRequest {
    OpenUrl {
        url: String,
    },
    SaveSettings {
        decimal: bool,
        compression: String,
        excludes: String,
    },
    ResetSettings,
    ChooseFolder,
    Compress,
    Decompress,
    Pause,
    Resume,
    Analyse,
    Stop,
    Quit,
}

// messages to send to the GUI
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum GuiResponse {
    Version {
        date: String,
        version: String,
    },
    SettingsReset {
        decimal: bool,
        compression: String,
        excludes: String,
    },
    Folder {
        path: PathBuf,
    },
    Status {
        status: String,
        pct: Option<f32>,
    },
    FolderSummary {
        info: FolderSummary,
    },
    Paused,
    Resumed,
    Scanned,
    Stopped,
    Compacting,
}

pub struct GuiWrapper<T>(Handle<T>);

impl<T> GuiWrapper<T> {
    pub fn new(handle: Handle<T>) -> Self {
        let gui = Self(handle);
        gui.version();
        gui
    }

    pub fn send(&self, msg: &GuiResponse) {
        let js = format!(
            "Response.dispatch({})",
            serde_json::to_string(msg).expect("serialize")
        );
        self.0.dispatch(move |wv| wv.eval(&js)).ok(); // let errors bubble through via messages
    }

    pub fn version(&self) {
        let version = GuiResponse::Version {
            date: env!("VERGEN_BUILD_DATE").to_string(),
            version: format!("{}-{}", env!("VERGEN_SEMVER"), env!("VERGEN_SHA_SHORT")),
        };
        self.send(&version);
    }

    pub fn summary(&self, info: FolderSummary) {
        self.send(&GuiResponse::FolderSummary { info });
    }

    pub fn status<S: AsRef<str>>(&self, msg: S, val: Option<f32>) {
        self.send(&GuiResponse::Status {
            status: msg.as_ref().to_owned(),
            pct: val,
        });
    }

    pub fn folder<P: AsRef<Path>>(&self, path: P) {
        self.send(&GuiResponse::Folder {
            path: path.as_ref().to_path_buf(),
        });
    }

    pub fn paused(&self) {
        self.send(&GuiResponse::Paused);
    }

    pub fn resumed(&self) {
        self.send(&GuiResponse::Resumed);
    }

    pub fn scanned(&self) {
        self.send(&GuiResponse::Scanned);
    }

    pub fn stopped(&self) {
        self.send(&GuiResponse::Stopped);
    }

    pub fn compacting(&self) {
        self.send(&GuiResponse::Compacting);
    }

    pub fn choose_folder(&self) -> Receiver<WVResult<Option<PathBuf>>> {
        let (tx, rx) = bounded::<WVResult<Option<PathBuf>>>(1);
        let _ = self.0.dispatch(move |wv| {
            let _ = tx.send(choose_folder(wv));
            Ok(())
        });

        rx
    }
}

pub fn spawn_gui() {
    let signalled = Arc::new(AtomicBool::new(false));
    let r = signalled.clone();
    ctrlc::set_handler(move || {
        r.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    set_dpi_aware();

    let html = format!(
        include_str!("ui/index.html"),
        style = include_str!("ui/style.css"),
        script = format!(
            "{}\n{}",
            include_str!("ui/cash.min.js"),
            include_str!("ui/app.js")
        )
    );

    let (from_gui, from_gui_rx) = bounded::<GuiRequest>(128);

    let mut webview = web_view::builder()
        .title("Compactor")
        .content(Content::Html(html))
        .size(1000, 550)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(move |mut webview, arg| {
            match serde_json::from_str::<GuiRequest>(arg) {
                Ok(GuiRequest::OpenUrl { url }) => {
                    open_url(url);
                }
                Ok(GuiRequest::SaveSettings {
                    decimal,
                    compression,
                    excludes,
                }) => {
                    let s = Config {
                        decimal,
                        compression: compression.parse().unwrap_or_default(),
                        excludes: excludes.split('\n').map(str::to_owned).collect(),
                    };

                    if let Err(msg) = s.globset() {
                        webview.dialog().error("Settings Error", msg).ok();
                    } else {
                        message_dispatch(
                            &mut webview,
                            &GuiResponse::SettingsReset {
                                decimal: s.decimal,
                                compression: s.compression.to_string(),
                                excludes: s.excludes.join("\n"),
                            },
                        );
                        webview
                            .dialog()
                            .info("Settings Saved", "Settings Updated")
                            .ok();
                        settings::set(s);
                        settings::save();
                    }
                }
                Ok(GuiRequest::ResetSettings) => {
                    let s = Config::default();

                    message_dispatch(
                        &mut webview,
                        &GuiResponse::SettingsReset {
                            decimal: s.decimal,
                            compression: s.compression.to_string(),
                            excludes: s.excludes.join("\n"),
                        },
                    );
                    settings::set(s);
                    settings::save();
                }
                Ok(msg) => {
                    from_gui.send(msg).expect("GUI message queue");
                }
                Err(err) => {
                    eprintln!("Unhandled message {:?}: {:?}", arg, err);
                }
            }

            Ok(())
        })
        .build()
        .expect("WebView");

    persistence::load();
    let s = settings::get();

    message_dispatch(
        &mut webview,
        &GuiResponse::SettingsReset {
            decimal: s.decimal,
            compression: s.compression.to_string(),
            excludes: s.excludes.join("\n"),
        },
    );
    // Config::set(s);

    if !system_supports_compression().unwrap_or_default() {
        webview
            .dialog()
            .error(
                "Unsupported OS",
                "Compactor requires Windows 10 features, \
                 and is completely untested on older systems.\n\n\
                 Proceed if you fancy being a guinea pig.\n\n\
                 Analysis will probably work, \
                 compression and decompression will not.",
            )
            .ok();
    }

    let gui = GuiWrapper::new(webview.handle());
    let mut backend = Backend::new(gui, from_gui_rx);
    let bg = std::thread::spawn(move || {
        backend.run();
    });

    loop {
        if signalled.load(Ordering::SeqCst) {
            webview.into_inner();
            break;
        }

        match webview.step() {
            Some(Ok(_)) => (),
            Some(e) => {
                eprintln!("Error: {:?}", e);
            }
            None => {
                webview.into_inner();
                break;
            }
        }
    }

    bg.join().expect("background thread");
}

fn message_dispatch<T>(wv: &mut web_view::WebView<'_, T>, msg: &GuiResponse) {
    let js = format!(
        "Response.dispatch({})",
        serde_json::to_string(msg).expect("serialize")
    );

    wv.eval(&js).ok();
}

fn open_url<U: AsRef<str>>(url: U) {
    let _ = open::that(url.as_ref());
}

fn set_dpi_aware() {
    use winapi::um::shellscalingapi::{SetProcessDpiAwareness, PROCESS_SYSTEM_DPI_AWARE};

    unsafe { SetProcessDpiAwareness(PROCESS_SYSTEM_DPI_AWARE) };
}

fn program_files() -> PathBuf {
    known_folder(&knownfolders::FOLDERID_ProgramFiles).expect("Program files path")
}

// stolen from directories crate
// Copyright (c) 2018 directories-rs contributors
// (MIT license)
fn known_folder(folder_id: shtypes::REFKNOWNFOLDERID) -> Option<PathBuf> {
    unsafe {
        let mut path_ptr: winnt::PWSTR = std::ptr::null_mut();
        let result =
            shlobj::SHGetKnownFolderPath(folder_id, 0, std::ptr::null_mut(), &mut path_ptr);
        if result == winerror::S_OK {
            let len = winbase::lstrlenW(path_ptr) as usize;
            let path = std::slice::from_raw_parts(path_ptr, len);
            let ostr: std::ffi::OsString = std::os::windows::ffi::OsStringExt::from_wide(path);
            combaseapi::CoTaskMemFree(path_ptr as *mut winapi::ctypes::c_void);
            Some(PathBuf::from(ostr))
        } else {
            None
        }
    }
}

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref LAST_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);
}

// WebView has an irritatingly stupid bug here, failing to initialize variables
// causing the dialog to fail to open and this to return None depending on the
// random junk on the stack. (#214)
//
// The message loop seems to be broken on Windows too (#220, #221).
//
// Sadly nobody seems interested in merging these.  For now, use a locally modified
// copy.
fn choose_folder<T>(webview: &mut web_view::WebView<'_, T>) -> WVResult<Option<PathBuf>> {
    let mut last = LAST_FILE.lock().unwrap();
    if let Some(path) = webview.dialog().choose_directory(
        "Select Directory",
        last.clone().unwrap_or_else(program_files),
    )? {
        last.replace(path.clone());

        Ok(Some(path))
    } else {
        Ok(None)
    }
}
