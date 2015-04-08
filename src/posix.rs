use std::{self, io, ptr, slice};
use std::ops::{Deref, DerefMut};
use std::path::Path;

use libc;

use ::MapKind;

impl MapKind {

    /// Returns the `MapKind` value as a POSIX protection flag.
    pub fn as_prot(self) -> libc::c_int {
        match self {
            MapKind::Read => libc::PROT_READ,
            MapKind::ReadWrite => libc::PROT_READ | libc::PROT_WRITE,
            MapKind::ReadCopy => libc::PROT_READ | libc::PROT_WRITE,
        }
    }

    pub fn as_flag(self) -> libc::c_int {
        match self {
            MapKind::Read => libc::MAP_SHARED,
            MapKind::ReadWrite => libc::MAP_SHARED,
            MapKind::ReadCopy => libc::MAP_PRIVATE,
        }
    }
}

pub struct MmapInner {
    ptr: *mut libc::c_void,
    len: usize,
}

impl MmapInner {

    /// Open a file-backed memory map.
    pub fn open<P>(path: P, kind: MapKind) -> io::Result<MmapInner> where P: AsRef<Path> {
        let file = try!(kind.as_open_options().open(path));
        let len = try!(file.metadata()).len();

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       kind.as_prot(),
                       kind.as_flag(),
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
    pub fn anonymous(len: usize, kind: MapKind) -> io::Result<MmapInner> {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       kind.as_prot(),
                       kind.as_flag() | libc::MAP_ANON,
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
