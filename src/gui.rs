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

/*
fn escape_html_into(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c)
        };
    }
}
*/
use crossbeam_channel::{bounded, Receiver, Sender};

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
    Stop,
    Quit,
}

// messages to send to the GUI
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum GuiResponse {
    ChooseFolder,
    Folder { path: PathBuf },
    Status { status: String, pct: Option<f32> },
    FolderSummary { info: FolderSummary },
    FolderInfo { info: FolderInfo },
}

/*
fn coordinator_thread(from_gui: Receiver<GuiRequest>, to_gui: Sender<GuiResponse>) {
    std::thread::spawn(move || {
        for ev in from_gui {
            match ev {
                GuiRequest::OpenUrl { url } => {
                    open_url(url);
                }
                GuiRequest::ChooseFolder => {
                    match state {
                        AppState::Idle | AppState::Waiting(_) => {
                            if let Some(dir) = select_dir(&mut webview)? {
                                println!("Selected: {:?}", dir);
                                response_dispatch(&mut wv, GuiResponse::Folder { path: dir.clone() })?;
                                state = AppState::Scanning(dir);
                            }
                        },
                        _ => {
                            println!("Can't select folder in {:?}", state);
                        }
                    }
                }
                _ => {
                    println!("Unhandled: {:?}", req);
                }
            }
        }
    });
}
*/

/*
fn gui_rx_thread(from_gui: Receiver<GuiRequest>, to_gui: Sender<GuiResponse>) {
    std::thread::spawn(move || {
        for msg in from_gui {
            if let GuiRequest::OpenUrl { url } = msg {
                open_url(url);
            }
        }
    });
}
*/

/*
fn gui_tx_thread<T: 'static>(to_gui: Receiver<GuiResponse>, webview: Handle<T>) {
    std::thread::spawn(move || {
        for msg in to_gui {
            match msg {
                GuiResponse::ChooseFolder => {
                    webview.dispatch(|wv| {
                        select_dir(&mut webview)
                    })
                }
            }
            let json = serde_json::to_string(&msg).expect("serialize");
            let res = webview.dispatch(move |wv| {
                let js = format!("Response.dispatch({})", json);
                println!("Eval: {}", js);
                wv.eval(&js)
            });

            if let Err(Error::Dispatch) = res {
                break; // webview has gone away
            }
        }
    });
}
*/

pub struct GuiWrapper<T>(Handle<T>);

impl<T> GuiWrapper<T> {
    pub fn new(handle: Handle<T>) -> Self {
        Self(handle)
    }

    pub fn send(&self, msg: &GuiResponse) -> WVResult {
        let js = format!("Response.dispatch({})", serde_json::to_string(msg).expect("serialize"));
        self.0.dispatch(move |wv| {
            println!("Eval: {}", js);
            wv.eval(&js)
        })
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
        .size(1000, 900)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(move |_webview, arg| {
            match serde_json::from_str::<GuiRequest>(arg) {
                Ok(GuiRequest::OpenUrl { url }) => {
                    open_url(url);
                },
                Ok(msg) => {
                    from_gui.send(msg).expect("GUI message queue");
                },
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
