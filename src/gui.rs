use crate::GuiResponses;
use crate::GuiActions;
use crossbeam_channel::Sender;
use crossbeam_channel::Receiver;
use web_view::*;

use winapi::shared::winerror;
use winapi::um::knownfolders;
use winapi::um::combaseapi;
use winapi::um::shlobj;
use winapi::um::shtypes;
use winapi::um::winbase;
use winapi::um::winnt;

use std::path::PathBuf;

const HTML_HEAD: &str = include_str!("ui/head.html");
const HTML_CSS: &str = include_str!("ui/style.css");
const HTML_JS_DEPS: &str = include_str!("ui/deps.js");
const HTML_JS_APP: &str = include_str!("ui/app.js");
const HTML_REST: &str = include_str!("ui/rest.html");

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

pub fn spawn_gui(background_tx: Sender<GuiActions>, gui_rx: Receiver<GuiResponses>) -> WVResult {
    set_dpi_aware();

    let mut html = String::new();
    html.push_str(HTML_HEAD);
    html.push_str("<style>\n");
    escape_html_into(HTML_CSS, &mut html);
    html.push_str("\n</style><script>\n");
    escape_html_into(HTML_JS_DEPS, &mut html);
    escape_html_into(HTML_JS_APP, &mut html);
    html.push_str("\n</script>\n");
    html.push_str(HTML_REST);

    let webview = web_view::builder()
        .title("Compactor")
        .content(Content::Html(html))
        .size(800, 600)
        .resizable(true)
        .debug(true)
        .user_data(())
        .invoke_handler(|mut webview, arg| {
            match arg {
                "choose" => {
                    match select_dir(&mut webview)? {
                        Some(path) => webview.dialog().info("Dir", path.to_string_lossy())?,
                        None => webview
                            .dialog()
                            .warning("Warning", "You didn't choose a file.")?,
                    };
                },
                _ => unimplemented!()
            }
            Ok(())
        })
        .build()?;

    webview.run()
}

fn set_dpi_aware() {
    use winapi::um::shellscalingapi::{PROCESS_SYSTEM_DPI_AWARE, SetProcessDpiAwareness};

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
        let result = shlobj::SHGetKnownFolderPath(folder_id, 0, std::ptr::null_mut(), &mut path_ptr);
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

