
use std::process::{Command, Stdio};
use std::io::BufReader;
use std::io::BufRead;
use std::ffi::OsStr;
use std::path::{PathBuf, Path};

use glob::{Pattern, MatchOptions};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

#[derive(Debug, Clone, Default)]
struct Compact {
    compression: Compression,
    force: bool,
    hidden_files: bool
}

#[derive(Debug, Copy, Clone)]
enum Compression {
    Xpress4,
    Xpress8,
    Xpress16,
    Lzx
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

        let out = BufReader::new(child.stdout.take().ok_or_else(|| "compact.exe: stdio".to_string())?);
        for line in out.lines() {
            println!("Compact: {}", line.unwrap_or_default());
        }

        let status = child.wait().map_err(|e| format!("compact.exe exit: {:?}", e))?;
        dbg!(status);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct DirectorySize {
    logical_size: u64,
    physical_size: u64,
    candidate_files: Vec<PathBuf>
}

impl DirectorySize {
    fn evaluate<P: AsRef<Path>>(path: P) -> Self {
        let walker = WalkBuilder::new(path.as_ref())
            .standard_filters(false)
            .build();

        let mut ds = Self::default();

        let skip_glob = Pattern::new("*.{7z,aac,avi,bik,bmp,br,bz2,cab,dl_,docx,flac,flv,gif,gz,jpeg,jpg,lz4,lzma,lzx,m2v,m4v,mkv,mp3,mp4,mpg,ogg,onepkg,png,pptx,rar,vob,vssx,vstx,wma,wmf,wmv,xap,xlsx,xz,zip,zst,zstd}").unwrap();
        let skip_glob_opts = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false,
            require_literal_leading_dot: false
        };

        for entry in walker {
            if entry.is_err() {
                eprintln!("Error: {:?}", entry);
                continue;
            }

            let entry = entry.unwrap();
            if !entry.file_type().map(|f| f.is_file()).unwrap_or_default() {
                continue;
            }

            if let Ok(md) = entry.metadata() {
                let logical = md.len();
                if let Ok(physical) = get_compressed_file_size(entry.path()) {
                    ds.logical_size += logical;
                    ds.physical_size += physical;

                    // TODO: evaluate this cut-off
                    if ds.logical_size < 4096 || skip_glob.matches_path_with(entry.path(), skip_glob_opts) {
                        continue;
                    }

                    ds.candidate_files.push(entry.path().to_path_buf());
                }
            }
        }

        ds
    }
}

fn get_compressed_file_size<P: AsRef<Path>>(p: P) -> std::io::Result<u64> {
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::shared::winerror::NO_ERROR;
    use winapi::um::fileapi::{GetCompressedFileSizeW, INVALID_FILE_SIZE};
    use std::os::windows::ffi::OsStrExt;

    let mut path: Vec<u16> = std::ffi::OsString::from("\\\\?\\").encode_wide().collect();
    path.extend(p.as_ref().as_os_str().encode_wide());
    path.push(0);
    let mut rest: u32 = 0;
    let ret = unsafe { GetCompressedFileSizeW(path.as_ptr(), &mut rest) };

    if ret == INVALID_FILE_SIZE && unsafe { GetLastError() != NO_ERROR } {
        Err(std::io::Error::last_os_error())
    } else {
        let size: u64 = (u64::from(rest) << 32) | u64::from(ret);
        Ok(size)
    }
}

use std::time::Instant;

fn main() {
    // evaluate_directory("D:\\test\\AIWar");
    let dir = DirectorySize::evaluate("D:\\test\\");
    println!("{:?}", dir);
    let compact = Compact::default();
    let start = Instant::now();
    dbg!(compact.compact_files(&dir.candidate_files[..]));
    println!("Exit in {:?}", start.elapsed());
}
