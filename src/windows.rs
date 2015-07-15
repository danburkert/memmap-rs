extern crate kernel32;
extern crate winapi;

use std::{self, fs, io, ptr, slice};
use std::ops::{Deref, DerefMut};
use std::os::raw::c_void;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use ::Protection;

impl Protection {

    /// Returns the `Protection` as a flag appropriate for a call to `CreateFileMapping`.
    fn as_mapping_flag(self) -> winapi::DWORD {
        match self {
            Protection::Read => winapi::PAGE_READONLY,
            Protection::ReadWrite => winapi::PAGE_READWRITE,
            Protection::ReadCopy => winapi::PAGE_READONLY,
        }
    }

    /// Returns the `Protection` as a flag appropriate for a call to `MapViewOfFile`.
    fn as_view_flag(self) -> winapi::DWORD {
        match self {
            Protection::Read => winapi::FILE_MAP_READ,
            Protection::ReadWrite => winapi::FILE_MAP_ALL_ACCESS,
            Protection::ReadCopy => winapi::FILE_MAP_COPY,
        }
    }
}

pub struct MmapInner {
    file: Option<fs::File>,
    ptr: *mut c_void,
    len: usize,
}

impl MmapInner {

    pub fn open<P>(path: P, prot: Protection) -> io::Result<MmapInner>
    where P: AsRef<Path> {
        let file = try!(prot.as_open_options().open(path));
        let len = try!(file.metadata()).len();

        unsafe {
            let handle = kernel32::CreateFileMappingW(AsRawHandle::as_raw_handle(&file) as *mut c_void,
                                                      ptr::null_mut(),
                                                      prot.as_mapping_flag(),
                                                      0,
                                                      0,
                                                      ptr::null());
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }

            let ptr = kernel32::MapViewOfFile(handle, prot.as_view_flag(), 0, 0, len as winapi::SIZE_T);
            kernel32::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(MmapInner {
                    file: Some(file),
                    ptr: ptr,
                    len: len as usize,
                })
            }
        }
    }

    pub fn anonymous(len: usize, prot: Protection) -> io::Result<MmapInner> {
        unsafe {
            let handle = kernel32::CreateFileMappingW(winapi::INVALID_HANDLE_VALUE,
                                                      ptr::null_mut(),
                                                      prot.as_mapping_flag(),
                                                      (len >> 16 >> 16) as winapi::DWORD,
                                                      (len & 0xffffffff) as winapi::DWORD,
                                                      ptr::null());
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }
            let ptr = kernel32::MapViewOfFile(handle, prot.as_view_flag(), 0, 0, len as winapi::SIZE_T);
            kernel32::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(MmapInner {
                    file: None,
                    ptr: ptr,
                    len: len as usize,
                })
            }
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        try!(self.flush_async());
        if let Some(ref mut file) = self.file { file.sync_data() } else { Ok(()) }
    }

    pub fn flush_async(&mut self) -> io::Result<()> {
        let result = unsafe { kernel32::FlushViewOfFile(self.ptr, 0) };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Drop for MmapInner {
    fn drop(&mut self) {
        unsafe {
            assert!(kernel32::UnmapViewOfFile(self.ptr) != 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

unsafe impl Send for MmapInner { }

impl Deref for MmapInner {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl DerefMut for MmapInner {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.len) }
    }
}
