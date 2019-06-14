use std::sync::Mutex;

use globset::{Glob, GlobSet, GlobSetBuilder};
use lazy_static::lazy_static;
use serde_derive::{Serialize, Deserialize};

use crate::compact::Compression;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub compression: Compression,
    pub excludes: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            compression: Compression::default(),
            excludes: vec![
                "*:\\Windows\\*",
                "*.7z",
                "*.aac",
                "*.avi",
                "*.ba",
                "*.br",
                "*.bz2",
                "*.cab",
                "*.dl_",
                "*.docx",
                "*.flac",
                "*.flv",
                "*.gif",
                "*.gz",
                "*.jpeg",
                "*.jpg",
                "*.log",
                "*.lz4",
                "*.lzma",
                "*.lzx",
                "*.m[24]v",
                "*.mkv",
                "*.mp[234]",
                "*.mpeg",
                "*.mpg",
                "*.ogg",
                "*.onepkg",
                "*.png",
                "*.pptx",
                "*.rar",
                "*.upk",
                "*.vob",
                "*.vs[st]x",
                "*.wem",
                "*.webm",
                "*.wm[afv]",
                "*.xap",
                "*.xnb",
                "*.xlsx",
                "*.xz",
                "*.zst",
                "*.zstd",
                "*.{bik,bk2,bnk,pc_binkvid}",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

lazy_static! {
    static ref SETTINGS: Mutex<Settings> = Mutex::new(Settings::default());
}

impl Settings {
    pub fn get() -> Settings {
        SETTINGS.lock().expect("Settings").clone()
    }

    pub fn set(s: Settings) {
        *SETTINGS.lock().expect("Settings") = s;
    }

    pub fn globset(&self) -> Result<GlobSet, String> {
        let mut globs = GlobSetBuilder::new();
        for glob in &self.excludes {
            globs.add(Glob::new(glob).map_err(|e| e.to_string())?);
        }
        globs.build().map_err(|e| e.to_string())
    }

    // fn validate(&self) -> Result<(), String> {}
}

#[test]
fn test_settings() {
    let s = Settings::default();

    assert!(s.globset().is_ok());
    let gs = s.globset().unwrap();

    assert!(gs.is_match("C:\\foo\\bar\\hmm.rar"));
    assert!(gs.is_match("C:\\Windows\\System32\\floop\\bla.txt"));
    assert!(gs.is_match("C:\\x.lz4"));
}
