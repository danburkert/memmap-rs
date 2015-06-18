use std::{self, io, ptr, slice};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::ffi::CString;

use libc;

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

    fn as_mode(self) -> libc::mode_t {
        match self {
            Protection::Read => 0o400,
            Protection::ReadWrite => 0o600,
            Protection::ReadCopy => 0o600,
        }
    }
}

pub struct MmapInner {
    ptr: *mut libc::c_void,
    len: usize,
    shm: Option<(libc::c_int, String)>,
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
                shm: None,
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
                shm: None,
            })
        }
    }

    /// Open an named memory map.
    ///
    /// If `exclusive` is true, this method will return an error if a map with the same name
    /// already exists. The specified name must not contain any forward or backwards slashes.
    pub fn named(len: usize, prot: Protection, name: String, exclusive: bool) -> io::Result<MmapInner> {
        // Adds a forward slash to work with shm_open in POSIX systems.
        let name = format!("/{}", name);

        // Create a shared memory object. By default this will create a new object if the specified
        // name does not already exist.
        let fd = unsafe {
            libc::shm_open(CString::new(name.clone()).unwrap().as_ptr(),
                           libc::O_CREAT
                           | if let Protection::Read = prot { libc::O_RDONLY } else { libc::O_RDWR }
                           | if exclusive { libc::O_EXCL } else { 0 },
                           prot.as_mode())
        };

        if fd == -1 {
            return Err(io::Error::last_os_error());
        }

        // Truncate shared memory object to specified size.
        unsafe { libc::ftruncate(fd, len as libc::off_t) };

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_prot(),
                       prot.as_flag(),
                       fd,
                       0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(MmapInner {
                ptr: ptr,
                len: len as usize,
                shm: Some((fd, name)),
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

            // If there is an attached shared memory object,
            if let Some(ref shm) = self.shm {
                // Close the file descriptor and
                assert!(libc::close(shm.0) == 0, "unable to close shm fd: {}",
                        io::Error::last_os_error());

                // Unlink the shared memory object.
                let unlink = libc::shm_unlink(CString::new(shm.1.clone()).unwrap().as_ptr()) == 0;
                let enoent = io::Error::last_os_error().raw_os_error().unwrap() == libc::ENOENT;

                // Asserts that either the unlink was successful or there was no object with the
                // associated name, meaning a matching named Mmap was destroyed earlier.
                assert!(unlink || enoent,
                        "unable to unlink shm object: {}",
                        io::Error::last_os_error());
            }
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
