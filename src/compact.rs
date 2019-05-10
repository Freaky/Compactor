use crate::background::Background;
use crate::background::ControlToken;
use std::ffi::OsStr;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Command, Stdio};

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

use lazy_static::lazy_static;
use regex::Regex;

use std::path::PathBuf;

pub struct Compacted {
    pub path: PathBuf,
    pub old_size: u64,
    pub new_size: u64,
}

impl Compact {
    fn compact_files<P: AsRef<OsStr>>(&self, paths: &[P]) -> Result<Vec<Compacted>, String> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r"\A([^:]+)\s+(\d+) :\s+(\d+) = [0-9.]+ to 1 \[OK\]").unwrap();
        }

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

        let mut compacted = Vec::with_capacity(paths.len());

        let mut folder = PathBuf::new();
        for line in out.lines() {
            let line = line.unwrap_or_default();

            // this better not be localised...
            if line.starts_with(" Compressing files in ") {
                folder = PathBuf::from(line[" Compressing files in ".len()..].to_owned());
                println!("Folder: {}", folder.display());
            } else if let Some(captures) = RE.captures(&line) {
                let path = folder.join(captures[1].to_owned());
                let old_size: u64 = captures[2].parse().unwrap();
                let new_size: u64 = captures[3].parse().unwrap();
                compacted.push(Compacted {
                    path,
                    old_size,
                    new_size,
                });
            }
        }

        let status = child
            .wait()
            .map_err(|e| format!("compact.exe exit: {:?}", e))?;

        dbg!(status);

        Ok(compacted)
    }

    fn decompact_files<P: AsRef<OsStr>>(&self, paths: &[P]) -> Result<Vec<PathBuf>, String> {
        let mut child = Command::new("compact.exe")
            .arg("/u") // uncompress
            .arg("/f") // force (or it'll fail on partially-compressed files)
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

        let mut compacted = Vec::with_capacity(paths.len());

        let mut folder = PathBuf::new();
        for line in out.lines() {
            let line = line.unwrap_or_default();

            eprintln!("Compact.exe: {}", line);

            // this better not be localised...
            if line.starts_with(" Uncompressing files in ") {
                folder = PathBuf::from(line[" Uncompressing files in ".len()..].to_owned());
                println!("Folder: {}", folder.display());
            } else if line.ends_with(" [OK]") {
                let path = folder.join(line[..(line.len() - " [OK]".len())].to_owned());
                compacted.push(path);
            }
        }

        let status = child
            .wait()
            .map_err(|e| format!("compact.exe exit: {:?}", e))?;

        dbg!(status);

        Ok(compacted)
    }
}

use crossbeam_channel::{Receiver, Sender};

enum Mode {
    Compress,
    Decompress,
}

#[derive(Debug)]
pub struct BackgroundCompactor {
    compactor: Compact,
    files_in: Receiver<Option<PathBuf>>,
    files_out: Sender<Compacted>,
}

impl BackgroundCompactor {
    pub fn new(
        files_in: Receiver<Option<PathBuf>>,
        files_out: Sender<Compacted>,
        compactor: Compact,
    ) -> Self {
        Self {
            compactor,
            files_in,
            files_out,
        }
    }
}

impl Background for BackgroundCompactor {
    type Output = Result<(), String>;
    type Status = ();

    fn run(&self, control: &ControlToken<Self::Status>) -> Self::Output {
        let mut batch = Vec::with_capacity(8);
        let mut done = false;
        for file in &self.files_in {
            if control.is_cancelled_with_pause() {
                return Err("Stopped".to_string());
            }

            if file.is_none() {
                done = true;
            }
            if let Some(file) = file {
                batch.push(file);
            }

            if batch.len() >= 8 || done && !batch.is_empty() {
                let ret = self.compactor.compact_files(&batch[..]);
                batch.clear();

                match ret {
                    Ok(compressed) => {
                        for c in compressed {
                            self.files_out.send(c).unwrap();
                        }
                    }
                    Err(s) => {
                        dbg!(s);
                        return Err("meh".to_string());
                    }
                }
            }

            if done {
                break;
            }
        }

        Ok(())
    }
}
