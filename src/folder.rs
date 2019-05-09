use std::path::{Path, PathBuf};

use filesize::file_real_size;
use ignore::WalkBuilder;
use serde_derive::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub logical_size: u64,
    pub physical_size: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GroupInfo {
    pub files: Vec<FileInfo>,
    pub logical_size: u64,
    pub physical_size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderInfo {
    pub path: PathBuf,
    pub logical_size: u64,
    pub physical_size: u64,
    pub compressible: GroupInfo,
    pub compressed: GroupInfo,
    pub skipped: GroupInfo,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FolderSummary {
    pub logical_size: u64,
    pub physical_size: u64,
    pub compressible: GroupSummary,
    pub compressed: GroupSummary,
    pub skipped: GroupSummary,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GroupSummary {
    pub count: usize,
    pub logical_size: u64,
    pub physical_size: u64,
}

impl FolderInfo {
    pub fn summary(&self) -> FolderSummary {
        FolderSummary {
            logical_size: self.logical_size,
            physical_size: self.physical_size,
            compressible: self.compressible.summary(),
            compressed: self.compressed.summary(),
            skipped: self.skipped.summary(),
        }
    }
}

impl GroupInfo {
    pub fn summary(&self) -> GroupSummary {
        GroupSummary {
            count: self.files.len(),
            logical_size: self.logical_size,
            physical_size: self.physical_size
        }
    }

    pub fn push(&mut self, fi: FileInfo) {
        self.logical_size += fi.logical_size;
        self.physical_size += fi.physical_size;
        self.files.push(fi);
    }
}

#[derive(Debug)]
pub struct FolderScan {
    path: PathBuf,
}

impl FolderScan {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

use crate::background::{Background, ControlToken};

impl Background for FolderScan {
    type Output = Result<FolderInfo, FolderInfo>;
    type Status = FolderSummary;

    fn run(&self, control: &ControlToken<Self::Status>) -> Self::Output {
        let mut ds = FolderInfo {
            path: self.path.clone(),
            logical_size: 0,
            physical_size: 0,
            compressible: GroupInfo::default(),
            compressed: GroupInfo::default(),
            skipped: GroupInfo::default(),
        };

        let skip_exts = vec![
            "7z", "aac", "avi", "bik", "bmp", "br", "bz2", "cab", "dl_", "docx", "flac", "flv",
            "gif", "gz", "jpeg", "jpg", "lz4", "lzma", "lzx", "m2v", "m4v", "mkv", "mp3", "mp4",
            "mpg", "ogg", "onepkg", "png", "pptx", "rar", "vob", "vssx", "vstx", "wma", "wmf",
            "wmv", "xap", "xlsx", "xz", "zip", "zst", "zstd",
        ];

        let mut last_status = Instant::now();

        let walker = WalkBuilder::new(&self.path)
            .standard_filters(false)
            .build()
            .filter_map(|e| e.map_err(|e| eprintln!("Error: {:?}", e)).ok())
            .filter_map(|e| e.metadata().map(|md| (e, md)).ok())
            .filter(|(_, md)| md.is_file())
            .filter_map(|(e, md)| file_real_size(e.path()).map(|s| (e, md, s)).ok())
            .enumerate();

        for (count, (entry, metadata, physical)) in walker {
            if count % 128 == 0 {
                if control.is_cancelled_with_pause() {
                    return Err(ds);
                }

                if last_status.elapsed() >= Duration::from_millis(100) {
                    last_status = Instant::now();
                    control.set_status(ds.summary());
                }
            }

            let logical = metadata.len();
            ds.logical_size += logical;
            ds.physical_size += physical;

            let shortname = entry
                .path()
                .strip_prefix(&self.path)
                .unwrap_or_else(|_e| entry.path())
                .to_path_buf();
            let extension = entry.path().extension().and_then(std::ffi::OsStr::to_str);

            let fi = FileInfo {
                path: shortname,
                logical_size: logical,
                physical_size: physical,
            };

            if physical < logical {
                ds.compressed.push(fi);
            } else if logical > 4096
                && !extension
                    .map(|ext| skip_exts.iter().any(|ex| ex.eq_ignore_ascii_case(ext)))
                    .unwrap_or_default()
            {
                ds.compressible.push(fi);
            } else {
                ds.skipped.push(fi);
            }
        }

        /*
        ds.compressed.sort_by(|a, b| {
            (a.physical_size as f64 / a.logical_size as f64)
                .partial_cmp(&(b.physical_size as f64 / b.logical_size as f64))
                .unwrap()
        });
        */

        Ok(ds)
    }
}

#[test]
fn it_walks() {
    use crate::background::BackgroundHandle;
    let scanner = FolderScan::new("C:\\Games");

    let task = BackgroundHandle::spawn(scanner);

    let deadline = Instant::now() + Duration::from_millis(2000);

    loop {
        let ret = task.wait_timeout(Duration::from_millis(100));

        if ret.is_some() {
            println!("Scanned: {:?}", ret);
            break;
        } else {
            println!("Status: {:?}", task.status());
        }

        if Instant::now() > deadline {
            task.cancel();
        }
    }
}
