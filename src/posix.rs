extern crate libc;

use std::{self, io, ptr};
use std::path::Path;

use ::Protection;

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

pub struct MmapInner {
    ptr: *mut libc::c_void,
    len: usize,
}

impl MmapInner {

    /// Open a file-backed memory map.
    pub fn open<P>(path: P, prot: Protection) -> io::Result<MmapInner> where P: AsRef<Path> {
        let file = try!(prot.as_open_options().open(path));
        let len = try!(file.metadata()).len();

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_prot(),
                       prot.as_flag(),
                       std::os::unix::io::AsRawFd::as_raw_fd(&file),
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

    /// Open an anonymous memory map.
    pub fn anonymous(len: usize, prot: Protection) -> io::Result<MmapInner> {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_prot(),
                       prot.as_flag() | libc::MAP_ANON,
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

    pub fn flush(&mut self) -> io::Result<()> {
        let result = unsafe { libc::msync(self.ptr, self.len as libc::size_t, libc::MS_SYNC) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn flush_async(&mut self) -> io::Result<()> {
        let result = unsafe { libc::msync(self.ptr, self.len as libc::size_t, libc::MS_ASYNC) };
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
        unsafe {
            assert!(libc::munmap(self.ptr, self.len as libc::size_t) == 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

unsafe impl Send for MmapInner { }
