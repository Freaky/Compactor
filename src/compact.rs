#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::convert::TryFrom;
use std::ffi::{CString, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::str::FromStr;

use serde_derive::{Deserialize, Serialize};

use winapi::shared::minwindef::{BOOL, DWORD, PBOOL, PULONG, ULONG};
use winapi::shared::ntdef::PVOID;
use winapi::shared::winerror::{HRESULT_CODE, SUCCEEDED};
use winapi::um::ioapiset::DeviceIoControl;
use winapi::um::winioctl::{FSCTL_DELETE_EXTERNAL_BACKING, FSCTL_SET_EXTERNAL_BACKING};
use winapi::um::winnt::{HANDLE, HRESULT, LPCWSTR};
use winapi::um::winver::{GetFileVersionInfoA, GetFileVersionInfoSizeA, VerQueryValueA};
use winapi::STRUCT;

STRUCT! {
    struct _WOF_FILE_COMPRESSION_INFO_V1 {
        Algorithm: ULONG,
        Flags: ULONG,
    }
}

STRUCT! {
    struct _WOF_EXTERNAL_INFO {
        Version: ULONG,
        Provider: ULONG,
    }
}

STRUCT! {
    struct _FILE_PROVIDER_EXTERNAL_INFO_V1 {
        Version: ULONG,
        Algorithm: ULONG,
        Flags: ULONG,
    }
}

type P_VS_FIXEDFILEINFO = *mut VS_FIXEDFILEINFO;
STRUCT! {
    struct VS_FIXEDFILEINFO {
        dwSignature: DWORD,
        dwStrucVersion: DWORD,
        dwFileVersionMS: DWORD,
        dwFileVersionLS: DWORD,
        dwProductVersionMS: DWORD,
        dwProductVersionLS: DWORD,
        dwFileFlagsMask: DWORD,
        dwFileFlags: DWORD,
        dwFileOS: DWORD,
        dwFileType: DWORD,
        dwFileSubtype: DWORD,
        dwFileDateMS: DWORD,
        dwFileDateLS: DWORD,
    }
}

const VS_FIXEDFILEINFO_SIGNATURE: DWORD = 0xFEEF_04BD;

const FILE_PROVIDER_COMPRESSION_XPRESS4K: ULONG = 0;
const FILE_PROVIDER_COMPRESSION_LZX: ULONG = 1;
const FILE_PROVIDER_COMPRESSION_XPRESS8K: ULONG = 2;
const FILE_PROVIDER_COMPRESSION_XPRESS16K: ULONG = 3;

const ERROR_SUCCESS: HRESULT = 0;
const ERROR_COMPRESSION_NOT_BENEFICIAL: HRESULT = 344;

const FILE_PROVIDER_CURRENT_VERSION: ULONG = 1;
const WOF_CURRENT_VERSION: ULONG = 1;
const WOF_PROVIDER_FILE: ULONG = 2;

impl Default for _FILE_PROVIDER_EXTERNAL_INFO_V1 {
    fn default() -> Self {
        Self {
            Version: FILE_PROVIDER_CURRENT_VERSION,
            Algorithm: FILE_PROVIDER_COMPRESSION_XPRESS4K,
            Flags: 0,
        }
    }
}

impl Default for _WOF_EXTERNAL_INFO {
    fn default() -> Self {
        Self {
            Version: WOF_CURRENT_VERSION,
            Provider: WOF_PROVIDER_FILE,
        }
    }
}

impl From<Compression> for _FILE_PROVIDER_EXTERNAL_INFO_V1 {
    fn from(compression: Compression) -> Self {
        Self {
            Version: FILE_PROVIDER_CURRENT_VERSION,
            Algorithm: compression.into(),
            Flags: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
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
            Compression::Xpress16k => write!(f, "XPRESS16K"),
            Compression::Lzx => write!(f, "LZX"),
        }
    }
}

impl FromStr for Compression {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "XPRESS4K" => Ok(Compression::Xpress4k),
            "XPRESS8K" => Ok(Compression::Xpress8k),
            "XPRESS16K" => Ok(Compression::Xpress16k),
            "LZX" => Ok(Compression::Lzx),
            _ => Err(()),
        }
    }
}

impl TryFrom<ULONG> for Compression {
    type Error = ();

    fn try_from(value: ULONG) -> Result<Self, Self::Error> {
        match value {
            FILE_PROVIDER_COMPRESSION_XPRESS4K => Ok(Compression::Xpress4k),
            FILE_PROVIDER_COMPRESSION_XPRESS8K => Ok(Compression::Xpress8k),
            FILE_PROVIDER_COMPRESSION_XPRESS16K => Ok(Compression::Xpress16k),
            FILE_PROVIDER_COMPRESSION_LZX => Ok(Compression::Lzx),
            _ => Err(()),
        }
    }
}

impl From<Compression> for ULONG {
    fn from(value: Compression) -> Self {
        match value {
            Compression::Xpress4k => FILE_PROVIDER_COMPRESSION_XPRESS4K,
            Compression::Xpress8k => FILE_PROVIDER_COMPRESSION_XPRESS8K,
            Compression::Xpress16k => FILE_PROVIDER_COMPRESSION_XPRESS16K,
            Compression::Lzx => FILE_PROVIDER_COMPRESSION_LZX,
        }
    }
}

pub fn system_supports_compression() -> std::io::Result<bool> {
    let dll = CString::new("WofUtil.dll").unwrap();
    let path = CString::new("\\").unwrap();
    let mut handle = 0;

    let len = unsafe { GetFileVersionInfoSizeA(dll.as_ptr(), &mut handle) };

    if len == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut buf = vec![0u8; len as usize];

    let ret = unsafe {
        GetFileVersionInfoA(
            dll.as_ptr(),
            handle,
            len,
            buf.as_mut_ptr() as *mut _ as PVOID,
        )
    };

    if ret == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut pinfo: PVOID = std::ptr::null_mut();
    let mut pinfo_size = 0;

    let ret = unsafe {
        VerQueryValueA(
            buf.as_mut_ptr() as *mut _ as PVOID,
            path.as_ptr(),
            &mut pinfo,
            &mut pinfo_size,
        )
    };

    if ret == 0 {
        return Err(std::io::Error::last_os_error());
    }

    assert!(pinfo_size as usize >= std::mem::size_of::<VS_FIXEDFILEINFO>());
    assert!(!pinfo.is_null());

    let pinfo: &VS_FIXEDFILEINFO = unsafe { &*(pinfo as *const VS_FIXEDFILEINFO) };
    assert!(pinfo.dwSignature == VS_FIXEDFILEINFO_SIGNATURE);

    Ok((pinfo.dwFileVersionMS >> 16) & 0xffff >= 10)
}

pub fn file_supports_compression<P: AsRef<Path>>(path: P) -> std::io::Result<bool> {
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
            Ok(Compression::try_from(file_info.Algorithm).ok())
        } else {
            Ok(None)
        }
    } else {
        Err(std::io::Error::from_raw_os_error(HRESULT_CODE(ret)))
    }
}

unsafe fn as_byte_slice<T: Sized + Copy>(p: &T) -> &[u8] {
    std::slice::from_raw_parts((p as *const T) as *const u8, std::mem::size_of::<T>())
}

pub fn compress_file<P: AsRef<Path>>(path: P, compression: Compression) -> std::io::Result<bool> {
    let file = std::fs::File::open(path)?;

    const LEN: usize = std::mem::size_of::<_WOF_EXTERNAL_INFO>()
        + std::mem::size_of::<_FILE_PROVIDER_EXTERNAL_INFO_V1>();

    let mut data = [0u8; LEN];
    let (wof, inf) = data.split_at_mut(std::mem::size_of::<_WOF_EXTERNAL_INFO>());
    unsafe {
        wof.copy_from_slice(as_byte_slice(&_WOF_EXTERNAL_INFO::default()));
        inf.copy_from_slice(as_byte_slice(&_FILE_PROVIDER_EXTERNAL_INFO_V1::from(
            compression,
        )));
    }

    let mut bytes_returned: DWORD = 0;

    let ret = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as HANDLE,
            FSCTL_SET_EXTERNAL_BACKING,
            &mut data as *mut _ as PVOID,
            data.len() as DWORD,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    // BOOL my arse
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

pub fn uncompress_file<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let file = std::fs::File::open(path)?;

    let mut bytes_returned: DWORD = 0;

    let ret = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as HANDLE,
            FSCTL_DELETE_EXTERNAL_BACKING,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if SUCCEEDED(ret) {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(HRESULT_CODE(ret)))
    }
}

#[link(name = "WofUtil")]
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

    // This is a slightly simpler way of setting file backing.
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

    let supported = system_supports_compression().expect("system_supports_compression");

    if supported && file_supports_compression(&path).expect("file_supports_compression") {
        uncompress_file(&path).expect("uncompress_file");
        assert_eq!(None, detect_compression(&path).expect("detect_compression"));
        compress_file(&path, Compression::default()).expect("compress_file");
    }
}
