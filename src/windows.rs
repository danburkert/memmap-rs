extern crate fs2;
extern crate kernel32;
extern crate winapi;

use std::{io, mem, ptr};
use std::fs::File;
use std::os::raw::c_void;
use std::os::windows::io::AsRawHandle;

use self::fs2::FileExt;

use ::Protection;
use ::MmapOptions;

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
    file: Option<File>,
    ptr: *mut c_void,
    len: usize,
}

impl MmapInner {

    pub fn open(file: &File, prot: Protection, offset: usize, len: usize) -> io::Result<MmapInner> {
        let alignment = offset % allocation_granularity();
        let aligned_offset = offset - alignment;
        let aligned_len = len + alignment;

        unsafe {
            let handle = kernel32::CreateFileMappingW(file.as_raw_handle(),
                                                      ptr::null_mut(),
                                                      prot.as_mapping_flag(),
                                                      0,
                                                      0,
                                                      ptr::null());
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }

            let ptr = kernel32::MapViewOfFile(handle,
                                              prot.as_view_flag(),
                                              (aligned_offset >> 16 >> 16) as winapi::DWORD,
                                              (aligned_offset & 0xffffffff) as winapi::DWORD,
                                              aligned_len as winapi::SIZE_T);
            kernel32::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(MmapInner {
                    file: Some(try!(file.duplicate())),
                    ptr: ptr.offset(alignment as isize),
                    len: len as usize,
                })
            }
        }
    }

    pub fn anonymous(len: usize, prot: Protection, _options: MmapOptions) -> io::Result<MmapInner> {
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

    pub fn flush(&mut self, offset: usize, len: usize) -> io::Result<()> {
        try!(self.flush_async(offset, len));
        if let Some(ref mut file) = self.file { file.sync_data() } else { Ok(()) }
    }

    pub fn flush_async(&mut self, offset: usize, len: usize) -> io::Result<()> {
        let result = unsafe { kernel32::FlushViewOfFile(self.ptr.offset(offset as isize),
                                                        len as u64) };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    pub fn mut_ptr(&mut self) -> *mut u8 {
        self.ptr as *mut u8
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Drop for MmapInner {
    fn drop(&mut self) {
        let alignment = self.ptr as usize % allocation_granularity();
        unsafe {
            let ptr = self.ptr.offset(0usize.wrapping_sub(alignment) as isize);
            assert!(kernel32::UnmapViewOfFile(ptr) != 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

unsafe impl Sync for MmapInner { }
unsafe impl Send for MmapInner { }

fn allocation_granularity() -> usize {
    unsafe {
        let mut info = mem::zeroed();
        kernel32::GetSystemInfo(&mut info);
        return info.dwAllocationGranularity as usize;
    }
}
