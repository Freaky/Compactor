use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use globset::{Glob, GlobSet, GlobSetBuilder};
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};

use crate::compact::Compression;

#[derive(Debug, Default)]
pub struct Settings {
    backing: Option<PathBuf>,
    config: Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub decimal: bool,
    pub compression: Compression,
    pub excludes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            decimal: false,
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

impl Settings {
    pub fn set_backing<P: AsRef<Path>>(&mut self, p: P) {
        self.backing = Some(p.as_ref().to_owned());
    }

    pub fn load(&mut self) {
        match &self.backing {
            Some(path) => {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(c) = serde_json::from_slice::<Config>(&data) {
                        self.set(c);
                    }
                }
            }
            None => (),
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        match &self.backing {
            Some(path) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }

                let data = serde_json::to_string_pretty(&self.config).expect("Serialize");
                std::fs::write(path, &data)
            }
            None => Ok(()),
        }
    }

    pub fn set(&mut self, c: Config) {
        self.config = c;
    }

    pub fn get(&self) -> Config {
        self.config.clone()
    }
}

impl Config {
    pub fn globset(&self) -> Result<GlobSet, String> {
        let mut globs = GlobSetBuilder::new();
        for glob in &self.excludes {
            globs.add(Glob::new(glob).map_err(|e| e.to_string())?);
        }
        globs.build().map_err(|e| e.to_string())
    }
}

lazy_static! {
    static ref SETTINGS: Mutex<Settings> = Mutex::new(Settings::default());
}

pub fn get() -> Config {
    SETTINGS.lock().expect("Settings").get()
}

pub fn set(s: Config) {
    SETTINGS.lock().expect("Settings").set(s);
}

pub fn load<P: AsRef<Path>>(p: P) {
    let mut s = SETTINGS.lock().unwrap();
    s.set_backing(p);
    s.load();
}

pub fn save() {
    SETTINGS.lock().unwrap().save();
}

#[test]
fn test_settings() {
    let s = Config::default();

    assert!(s.globset().is_ok());
    let gs = s.globset().unwrap();

    assert!(gs.is_match("C:\\foo\\bar\\hmm.rar"));
    assert!(gs.is_match("C:\\Windows\\System32\\floop\\bla.txt"));
    assert!(gs.is_match("C:\\x.lz4"));
}
