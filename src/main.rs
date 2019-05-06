// #![windows_subsystem = "windows"]

use std::ffi::OsStr;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use filesize::file_real_size;
use glob::{MatchOptions, Pattern};
use ignore::WalkBuilder;

#[derive(Debug, Clone, Default)]
pub struct Compact {
    compression: Compression,
    force: bool,
    hidden_files: bool,
}

#[derive(Debug, Copy, Clone)]
pub enum Compression {
    Xpress4,
    Xpress8,
    Xpress16,
    Lzx,
}

impl Default for Compression {
    fn default() -> Self {
        Compression::Xpress8
    }
}

impl Compression {
    fn to_flag(&self) -> &str {
        match self {
            Compression::Xpress4 => "/EXE:XPRESS4K",
            Compression::Xpress8 => "/EXE:XPRESS8K",
            Compression::Xpress16 => "/EXE:XPRESS16K",
            Compression::Lzx => "/EXE:LZX",
        }
    }
}

impl Compact {
    fn compact_files<P: AsRef<OsStr>>(&self, paths: &[P]) -> Result<(), String> {
        let mut child = Command::new("compact.exe")
            .arg("/c") // compress
            .arg("/f") // force (or it'll fail on partially-compressed files)
            .arg(self.compression.to_flag())
            .args(paths)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("compact.exe failure: {:?}", e))?;

        let out = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| "compact.exe: stdio".to_string())?,
        );
        for line in out.lines() {
            println!("Compact: {}", line.unwrap_or_default());
        }

        let status = child
            .wait()
            .map_err(|e| format!("compact.exe exit: {:?}", e))?;
        dbg!(status);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub path: PathBuf,
    pub logical_size: u64,
    pub physical_size: u64,
    pub compressable: Vec<PathBuf>,
    pub compressed: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

impl FolderInfo {
    fn evaluate<P: AsRef<Path>>(path: P) -> Self {
        let mut ds = Self {
            path: path.as_ref().to_path_buf(),
            logical_size: 0,
            physical_size: 0,
            compressable: vec![],
            compressed: vec![],
            skipped: vec![]
        };

        let skip_glob = Pattern::new("*.{7z,aac,avi,bik,bmp,br,bz2,cab,dl_,docx,flac,flv,gif,gz,jpeg,jpg,lz4,lzma,lzx,m2v,m4v,mkv,mp3,mp4,mpg,ogg,onepkg,png,pptx,rar,vob,vssx,vstx,wma,wmf,wmv,xap,xlsx,xz,zip,zst,zstd}").unwrap();
        let skip_glob_opts = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let walker = WalkBuilder::new(path.as_ref())
            .standard_filters(false)
            .build()
            .filter_map(|e| e.map_err(|e| eprintln!("Error: {:?}", e)).ok())
            .filter_map(|e| e.metadata().map(|md| (e, md)).ok())
            .filter(|(_, md)| md.is_file())
            .filter_map(|(e, md)| file_real_size(e.path()).map(|s| (e, md, s)).ok());

        for (entry, metadata, physical) in walker {
            let logical = metadata.len();
            ds.logical_size += logical;
            ds.physical_size += physical;

            let shortname = entry.path().strip_prefix(&path).unwrap_or_else(|_e| entry.path()).to_path_buf();

            if physical < logical {
                ds.compressed.push(shortname);
            } else if ds.logical_size > 4096 && !skip_glob.matches_path_with(entry.path(), skip_glob_opts) {
                ds.compressable.push(shortname);
            } else {
                ds.skipped.push(shortname);
            }
        }

        ds
    }
}

mod gui;

use crossbeam_channel::{bounded, Sender, Receiver};

pub enum GuiActions {
    SetCompression(Compression),
    SelectFolder(PathBuf),
    Compress,
    Decompress,
    Pause,
    Continue,
    Cancel,
    Quit
}

pub enum GuiResponses {
    FolderStatus(FolderInfo),
    Output(String),
    Exit
}

fn spawn_worker(background_rx: Receiver<GuiActions>, gui_tx: Sender<GuiResponses>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {
        let mut compression = Compression::default();

        for action in background_rx {
            match action {
                GuiActions::SetCompression(comp) => { compression = comp; },
                GuiActions::SelectFolder(path) => {},
                GuiActions::Compress => {},
                GuiActions::Decompress => {},
                GuiActions::Pause => {},
                GuiActions::Continue => {},
                GuiActions::Cancel => {},
                GuiActions::Quit => {},
            }
        }
    })
}

fn main() {
    let (background_tx, background_rx) = bounded::<GuiActions>(1024);
    let (gui_tx, gui_rx) = bounded::<GuiResponses>(1024);

    let worker = spawn_worker(background_rx, gui_tx);
    gui::spawn_gui(background_tx, gui_rx);
}
