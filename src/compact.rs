
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
