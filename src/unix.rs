extern crate libc;

use std::{self, io, ptr};
use std::fs::File;

use ::Protection;
use ::MmapOptions;

impl Protection {

    /// Returns the `Protection` value as a POSIX protection flag.
    fn as_prot(self) -> libc::c_int {
        match self {
            Protection::Read => libc::PROT_READ,
            Protection::ReadWrite => libc::PROT_READ | libc::PROT_WRITE,
            Protection::ReadCopy => libc::PROT_READ | libc::PROT_WRITE,
        }
    }

    fn as_flag(self) -> libc::c_int {
        match self {
            Protection::Read => libc::MAP_SHARED,
            Protection::ReadWrite => libc::MAP_SHARED,
            Protection::ReadCopy => libc::MAP_PRIVATE,
        }
    }
}

impl MmapOptions {
    fn as_flag(self) -> libc::c_int {
        let mut flag = 0;
        if self.stack { flag |= libc::MAP_STACK }
        flag
    }
}

pub struct MmapInner {
    ptr: *mut libc::c_void,
    len: usize,
}

impl MmapInner {

    pub fn open(file: File, prot: Protection, offset: usize, len: usize) -> io::Result<MmapInner> {
        let alignment = offset % page_size();
        let aligned_offset = offset - alignment;
        let aligned_len = len + alignment;

        unsafe {
            let ptr = libc::mmap(ptr::null_mut(),
                                 aligned_len as libc::size_t,
                                 prot.as_prot(),
                                 prot.as_flag(),
                                 std::os::unix::io::AsRawFd::as_raw_fd(&file),
                                 aligned_offset as libc::off_t);

            if ptr == libc::MAP_FAILED {
                Err(io::Error::last_os_error())
            } else {
                Ok(MmapInner {
                    ptr: ptr.offset(alignment as isize),
                    len: len,
                })
            }
        }
    }

    /// Open an anonymous memory map.
    pub fn anonymous(len: usize, prot: Protection, options: MmapOptions) -> io::Result<MmapInner> {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_prot(),
                       options.as_flag() | prot.as_flag() | libc::MAP_ANON,
                       -1,
                       0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(MmapInner {
                ptr: ptr,
                len: len as usize,
            })
        }
    }

    pub fn flush(&mut self, offset: usize, len: usize) -> io::Result<()> {
        let result = unsafe { libc::msync(self.ptr.offset(offset as isize),
                                          len as libc::size_t,
                                          libc::MS_SYNC) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn flush_async(&mut self, offset: usize, len: usize) -> io::Result<()> {
        let result = unsafe { libc::msync(self.ptr.offset(offset as isize),
                                          len as libc::size_t,
                                          libc::MS_ASYNC) };
        if result == 0 {
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
        let alignment = self.ptr as usize % page_size();
        unsafe {
            assert!(libc::munmap(self.ptr.offset(0usize.wrapping_sub(alignment) as isize),
                                 (self.len + alignment) as libc::size_t) == 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

unsafe impl Sync for MmapInner { }
unsafe impl Send for MmapInner { }

fn page_size() -> usize {
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
}
