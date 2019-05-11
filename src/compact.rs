#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use winapi::shared::minwindef::{BOOL, PBOOL, PULONG, ULONG};
use winapi::shared::ntdef::PVOID;
use winapi::shared::winerror::{HRESULT_CODE, SUCCEEDED};
use winapi::um::winnt::{HANDLE, HRESULT, LPCWSTR};
use winapi::STRUCT;

type P_WOF_FILE_COMPRESSION_INFO_V1 = *mut _WOF_FILE_COMPRESSION_INFO_V1;
STRUCT! {
    struct _WOF_FILE_COMPRESSION_INFO_V1 {
        Algorithm: ULONG,
        Flags: ULONG,
    }
}

const FILE_PROVIDER_COMPRESSION_XPRESS4K: ULONG = 0;
const FILE_PROVIDER_COMPRESSION_LZX: ULONG = 1;
const FILE_PROVIDER_COMPRESSION_XPRESS8K: ULONG = 2;
const FILE_PROVIDER_COMPRESSION_XPRESS16K: ULONG = 3;

const ERROR_SUCCESS: HRESULT = 0;
const ERROR_COMPRESSION_NOT_BENEFICIAL: HRESULT = 344;

const WOF_PROVIDER_FILE: ULONG = 2;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Compression {
    Xpress4k,
    Xpress8k,
    Xpress16k,
    Lzx,
}

impl Default for Compression {
    fn default() -> Self {
        Compression::Xpress8k
    }
}

impl std::fmt::Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Compression::Xpress4k => write!(f, "XPRESS4k"),
            Compression::Xpress8k => write!(f, "XPRESS8K"),
            Compression::Xpress16k => write!(f, "XPRES16K"),
            Compression::Lzx => write!(f, "LZX"),
        }
    }
}

impl Compression {
    fn to_api(self) -> ULONG {
        match self {
            Compression::Xpress4k => FILE_PROVIDER_COMPRESSION_XPRESS4K,
            Compression::Xpress8k => FILE_PROVIDER_COMPRESSION_XPRESS8K,
            Compression::Xpress16k => FILE_PROVIDER_COMPRESSION_XPRESS16K,
            Compression::Lzx => FILE_PROVIDER_COMPRESSION_LZX,
        }
    }

    fn from_api(c: ULONG) -> Option<Self> {
        match c {
            FILE_PROVIDER_COMPRESSION_XPRESS4K => Some(Compression::Xpress4k),
            FILE_PROVIDER_COMPRESSION_XPRESS8K => Some(Compression::Xpress8k),
            FILE_PROVIDER_COMPRESSION_XPRESS16K => Some(Compression::Xpress16k),
            FILE_PROVIDER_COMPRESSION_LZX => Some(Compression::Lzx),
            _ => None,
        }
    }

    pub fn from_str<S: AsRef<str>>(s: S) -> Option<Self> {
        match s.as_ref() {
            "XPRESS4K" => Some(Compression::Xpress4k),
            "XPRESS8K" => Some(Compression::Xpress8k),
            "XPRESS16K" => Some(Compression::Xpress16k),
            "LZX" => Some(Compression::Lzx),
            _ => None,
        }
    }
}

pub struct Compact;

impl Compact {
    pub fn is_compression_supported<P: AsRef<Path>>(path: P) -> std::io::Result<bool> {
        let file = std::fs::File::open(path)?;
        let mut version: ULONG = 0;

        let ret = unsafe {
            WofGetDriverVersion(
                file.as_raw_handle() as HANDLE,
                WOF_PROVIDER_FILE,
                &mut version,
            )
        };

        if SUCCEEDED(ret) && version > 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn detect_compression<P: AsRef<OsStr>>(path: P) -> std::io::Result<Option<Compression>> {
        let mut p: Vec<u16> = path.as_ref().encode_wide().collect();
        p.push(0);

        let mut is_external: BOOL = 0;
        let mut provider: ULONG = 0;
        let mut file_info: _WOF_FILE_COMPRESSION_INFO_V1 = unsafe { std::mem::zeroed() };
        let mut len: ULONG = std::mem::size_of::<_WOF_FILE_COMPRESSION_INFO_V1>() as ULONG;

        let ret = unsafe {
            WofIsExternalFile(
                p.as_ptr(),
                &mut is_external,
                &mut provider,
                &mut file_info as *mut _ as PVOID,
                &mut len,
            )
        };

        if SUCCEEDED(ret) {
            if is_external > 0 && provider == WOF_PROVIDER_FILE {
                Ok(Compression::from_api(file_info.Algorithm))
            } else {
                Ok(None)
            }
        } else {
            Err(std::io::Error::from_raw_os_error(HRESULT_CODE(ret)))
        }
    }

    pub fn compress_file<P: AsRef<Path>>(
        path: P,
        compression: Compression,
    ) -> std::io::Result<bool> {
        let file = std::fs::File::open(path)?;

        let info = _WOF_FILE_COMPRESSION_INFO_V1 {
            Algorithm: compression.to_api(),
            Flags: 0,
        };
        let len: ULONG = std::mem::size_of::<_WOF_FILE_COMPRESSION_INFO_V1>() as ULONG;

        let ret = unsafe {
            WofSetFileDataLocation(
                file.as_raw_handle() as HANDLE,
                WOF_PROVIDER_FILE,
                &info as *const _ as PVOID,
                len,
            )
        };

        if SUCCEEDED(ret) {
            Ok(true)
        } else {
            let e = HRESULT_CODE(ret);

            if e == ERROR_COMPRESSION_NOT_BENEFICIAL {
                Ok(false)
            } else {
                Err(std::io::Error::from_raw_os_error(e))
            }
        }
    }

    // compact.exe uses FltFsControlFile() with FSCTL_DELETE_EXTERNAL_BACKING
    // this is a reasonable workalike and much simpler.
    pub fn uncompress_file<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map(|_| ())
    }
}

#[link(name = "wofutil")]
extern "system" {
    pub fn WofGetDriverVersion(
        file_or_volume_handle: HANDLE,
        provider: ULONG,
        version: PULONG,
    ) -> HRESULT;

    pub fn WofIsExternalFile(
        file_path: LPCWSTR,
        is_external_file: PBOOL,
        provider: PULONG,
        external_file_info: PVOID,
        length: PULONG,
    ) -> HRESULT;

    pub fn WofSetFileDataLocation(
        file_handle: HANDLE,
        provider: ULONG,
        external_file_info: PVOID,
        length: ULONG,
    ) -> HRESULT;
}

#[test]
fn compact_works_i_guess() {
    let path = std::path::PathBuf::from("Cargo.lock");

    let supported = Compact::is_compression_supported(&path).expect("is_compression_supported");

    if supported {
        Compact::uncompress_file(&path).expect("uncompress_file");
        assert_eq!(
            None,
            Compact::detect_compression(&path).expect("detect_compression")
        );
        Compact::compress_file(&path, Compression::default()).expect("compress_file");
    }
}
