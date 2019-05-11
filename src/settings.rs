use std::sync::{Arc, Mutex};

use globset::{Glob, GlobSet, GlobSetBuilder};
use lazy_static::lazy_static;

use crate::compact::Compression;

#[derive(Debug, Clone)]
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
                "*.bik",
                "*.bmp",
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
                "*.lz4",
                "*.lzma",
                "*.lzx",
                "*.m2v",
                "*.m4v",
                "*.mkv",
                "*.mp[234]",
                "*.mpg",
                "*.mpeg",
                "*.ogg",
                "*.onepkg",
                "*.png",
                "*.pptx",
                "*.rar",
                "*.vob",
                "*.vs[st]x",
                "*.wm[afv]",
                "*.xap",
                "*.xlsx",
                "*.xz",
                "*.zip",
                "*.zst",
                "*.zstd",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

lazy_static! {
    static ref SETTINGS: Arc<Mutex<Settings>> = Arc::new(Mutex::new(Settings::default()));
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
