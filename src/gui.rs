use std::path::Path;
use crate::folder::{FolderInfo, FolderSummary};
use web_view::*;

use winapi::shared::winerror;
use winapi::um::combaseapi;
use winapi::um::knownfolders;
use winapi::um::shlobj;
use winapi::um::shtypes;
use winapi::um::winbase;
use winapi::um::winnt;

use std::path::PathBuf;

use crate::backend::Backend;

const HTML_HEAD: &str = include_str!("ui/head.html");
const HTML_CSS: &str = include_str!("ui/style.css");
const HTML_JS_DEPS: &str = include_str!("ui/cash.min.js");
const HTML_JS_APP: &str = include_str!("ui/app.js");
const HTML_REST: &str = include_str!("ui/rest.html");

use crossbeam_channel::{bounded, Receiver};

use serde_derive::{Deserialize, Serialize};
use serde_json;

// messages received from the GUI
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum GuiRequest {
    OpenUrl { url: String },
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
    Folder { path: PathBuf },
    Status { status: String, pct: Option<f32> },
    FolderSummary { info: FolderSummary },
    FolderInfo { info: FolderInfo },
    Paused,
    Resumed,
    Scanned,
    Stopped
}

pub struct GuiWrapper<T>(Handle<T>);

impl<T> GuiWrapper<T> {
    pub fn new(handle: Handle<T>) -> Self {
        Self(handle)
    }

    pub fn send(&self, msg: &GuiResponse) {
        let js = format!(
            "Response.dispatch({})",
            serde_json::to_string(msg).expect("serialize")
        );
        self.0.dispatch(move |wv| {
            println!("Eval: {}", js);
            wv.eval(&js)
        }).ok(); // let errors bubble through via messages
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
        self.send(&GuiResponse::Folder { path: path.as_ref().to_path_buf() });
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
    set_dpi_aware();

    let mut html = String::new();
    html.push_str(HTML_HEAD);
    html.push_str("<style>\n");
    html.push_str(HTML_CSS);
    html.push_str("\n</style><script>\n");
    html.push_str(HTML_JS_DEPS);
    html.push_str(HTML_JS_APP);
    html.push_str("\n</script>\n");
    html.push_str(HTML_REST);

    std::fs::write("test.html", &html).unwrap();

    let (from_gui, from_gui_rx) = bounded::<GuiRequest>(128);

    let webview = web_view::builder()
        .title("Compactor")
        .content(Content::Html(html))
        .size(1000, 700)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(move |_webview, arg| {
            match serde_json::from_str::<GuiRequest>(arg) {
                Ok(GuiRequest::OpenUrl { url }) => {
                    open_url(url);
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

    let gui = GuiWrapper::new(webview.handle());
    let mut backend = Backend::new(gui, from_gui_rx);
    std::thread::spawn(move || {
        backend.run();
    });

    webview.run().expect("webview");
    println!("Exiting");
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
