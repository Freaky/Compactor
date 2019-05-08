use crate::folder::FolderInfo;
use web_view::*;

use winapi::shared::winerror;
use winapi::um::combaseapi;
use winapi::um::knownfolders;
use winapi::um::shlobj;
use winapi::um::shtypes;
use winapi::um::winbase;
use winapi::um::winnt;

use std::path::PathBuf;

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

#[derive(Debug, Clone)]
enum AppState {
    Idle,
    Scanning(PathBuf),
    Waiting(FolderInfo),
    Compressing(FolderInfo),
    Decompressing(FolderInfo),
}

// messages received from the GUI
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum GuiRequest {
    OpenUrl { url: String },
    ChooseFolder,
    Compress,
    Decompress,
    Pause,
    Resume,
    Cancel,
    Quit,
}

// messages to send to the GUI
#[derive(Serialize)]
#[serde(tag = "type")]
enum GuiResponse {
    Folder { path: PathBuf },
    Progress { status: String, pct: Option<u8> },
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

    let mut state = AppState::Idle;

    let webview = web_view::builder()
        .title("Compactor")
        .content(Content::Html(html))
        .size(1000, 900)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(move |mut webview, arg| {
            let req = match serde_json::from_str::<GuiRequest>(arg) {
                Ok(req) => { req; },
                Err(e) => {
                    eprintln!("Unhandled invoke message {:?}: {:?}", arg, e);
                }
            };

            Ok(())
        })
        .build()
        .expect("WebView");

    // coordinator_thread(rx, webview.handle());

    webview.run().expect("webview");
    println!("Exiting");
}

fn response_dispatch<T>(webview: &mut web_view::WebView<'_, T>, response: GuiResponse) -> WVResult {
    let js = &format!("Response.dispatch({})", serde_json::to_string(&response).expect("serialize"));
    println!("Eval: {}", js);
    webview.eval(&js)
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
fn select_dir<T>(webview: &mut web_view::WebView<'_, T>) -> WVResult<Option<PathBuf>> {
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
