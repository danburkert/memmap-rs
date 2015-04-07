#![cfg_attr(test, feature(page_size))]

#[macro_use]
extern crate bitflags;
extern crate libc;

use std::{fs, io, ptr, slice};
use std::borrow::{Borrow, BorrowMut};
use std::ops::{
    Deref, DerefMut,
    Index, IndexMut,
    Range, RangeFrom, RangeTo, RangeFull,
};
use std::path::Path;

use Protection::*;

/// Memory protection options
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {

    /// Pages may not be accessed
    None,

    /// Pages may be read
    Read,

    /// Pages may be read or written
    ReadWrite,

    /// Pages may be executed and read
    ExecRead,

    /// Pages may be executed, read, and written
    ExecReadWrite,
}

impl Protection {

    #[cfg(any(target_os = "linux",
              target_os = "android",
              target_os = "macos",
              target_os = "ios",
              target_os = "freebsd",
              target_os = "dragonfly",
              target_os = "bitrig",
              target_os = "openbsd"))]
    fn as_flag(self) -> libc::c_int {
        match self {
            None => libc::PROT_NONE,
            Read => libc::PROT_READ,
            ReadWrite => libc::PROT_READ | libc::PROT_WRITE,
            ExecRead => libc::PROT_EXEC | libc::PROT_READ,
            ExecReadWrite => libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE,
        }
    }

    #[cfg(target_os = "windows")]
    fn as_page_flags(self) -> libc::DWORD {
        match self {
            None => 0,
            Read => libc::PAGE_READONLY,
            ReadWrite => libc::PAGE_READWRITE,
            ExecRead => libc::PAGE_EXECUTE_READ,
            ExecReadWrite => libc::PAGE_EXECUTE_READWRITE,
        }
    }

    #[cfg(target_os = "windows")]
    fn as_file_flags(self) -> libc::DWORD {
        match self {
            None => 0,
            Read => libc::FILE_MAP_READ,
            ReadWrite => libc::FILE_MAP_READ | libc::FILE_MAP_WRITE,
            ExecRead => libc::FILE_MAP_READ | libc::FILE_MAP_EXECUTE,
            ExecReadWrite => libc::FILE_MAP_READ | libc::FILE_MAP_WRITE | libc::FILE_MAP_EXECUTE,
        }
    }

    fn as_open_options(self) -> fs::OpenOptions {
        let mut options = fs::OpenOptions::new();
        options.read(self.read())
               .write(self.write());

        options
    }

    /// Returns `true` if the Protection is executable.
    pub fn execute(self) -> bool {
        match self {
            ExecRead | ExecReadWrite => true,
            _ => false,
        }
    }

    /// Returns `true` if the Protection is readable.
    pub fn read(self) -> bool {
        match self {
            Read | ReadWrite | ExecRead | ExecReadWrite => true,
            _ => false,
        }
    }

    /// Returns `true` if the Protection is writable.
    pub fn write(self) -> bool {
        match self {
            ReadWrite | ExecReadWrite => true,
            _ => false,
        }
    }
}

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
pub struct Mmap {
    ptr: *mut libc::c_void,
    len: usize,
}

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
impl Mmap {

    /// Open a file-backed memory map.
    pub fn open<P>(path: P, prot: Protection) -> io::Result<Mmap> where P: AsRef<Path> {
        let file = try!(prot.as_open_options().open(path));
        let len = try!(file.metadata()).len();

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_flag(),
                       libc::MAP_SHARED,
                       std::os::unix::io::AsRawFd::as_raw_fd(&file),
                       0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(Mmap {
                ptr: ptr,
                len: len as usize,
            })
        }
    }

    /// Open an anonymous memory map.
    pub fn anonymous(len: usize, prot: Protection) -> io::Result<Mmap> {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.as_flag(),
                       libc::MAP_SHARED | libc::MAP_ANON,
                       -1,
                       0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(Mmap {
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
}

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            assert!(libc::munmap(self.ptr, self.len as libc::size_t) == 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

#[cfg(target_os = "windows")]
pub struct Mmap {
    file: Option<fs::File>,
    ptr: *mut libc::c_void,
    len: usize,
}

#[cfg(target_os = "windows")]
impl Mmap {

    pub fn open<P>(path: P, prot: Protection) -> io::Result<Mmap>
    where P: AsRef<Path> {
        let file = try!(prot.as_open_options().open(path));
        let len = try!(file.metadata()).len();

        unsafe {
            let handle = libc::CreateFileMappingW(std::os::windows::io::AsRawHandle::as_raw_handle(&file) as *mut libc::c_void,
                                                  ptr::null_mut(),
                                                  prot.as_page_flags(),
                                                  0,
                                                  0,
                                                  ptr::null());
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }

            let ptr = libc::MapViewOfFile(handle, prot.as_file_flags(), 0, 0, len as libc::SIZE_T);
            libc::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(Mmap {
                    file: Some(file),
                    ptr: ptr,
                    len: len as usize,
                })
            }
        }
    }

    pub fn anonymous(len: usize, prot: Protection) -> io::Result<Mmap> {
        unsafe {
            let handle = libc::CreateFileMappingW(libc::INVALID_HANDLE_VALUE,
                                                  ptr::null_mut(),
                                                  prot.as_page_flags(),
                                                  (len >> 16 >> 16) as libc::DWORD,
                                                  (len & 0xffffffff) as libc::DWORD,
                                                  ptr::null());
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }
            let ptr = libc::MapViewOfFile(handle, prot.as_file_flags(), 0, 0, len as libc::SIZE_T);
            libc::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(Mmap {
                    file: Option::None,
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
        // TODO: reenable when rust-lang/rust/pull/24174 is merged
        /*
        let result = unsafe { libc::FlushViewOfFile(self.ptr, 0) };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
        */
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            assert!(libc::UnmapViewOfFile(self.ptr) != 0,
                    "unable to unmap mmap: {}", io::Error::last_os_error());
        }
    }
}

impl Mmap {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl Deref for Mmap {

    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl DerefMut for Mmap {

    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.len) }
    }
}

impl AsRef<[u8]> for Mmap {
    fn as_ref(&self) -> &[u8] {
        &*self
    }
}

impl AsMut<[u8]> for Mmap {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut *self
    }
}

impl Borrow<[u8]> for Mmap {
    fn borrow(&self) -> &[u8] {
        &*self
    }
}

impl BorrowMut<[u8]> for Mmap {
    fn borrow_mut(&mut self) -> &mut [u8] {
        &mut *self
    }
}

impl Index<usize> for Mmap {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        &(*self)[index]
    }
}

impl IndexMut<usize> for Mmap {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut (*self)[index]
    }
}

impl Index<Range<usize>> for Mmap {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &[u8] {
        Index::index(&**self, index)
    }
}

impl Index<RangeTo<usize>> for Mmap {
    type Output = [u8];

    fn index(&self, index: RangeTo<usize>) -> &[u8] {
        Index::index(&**self, index)
    }
}

impl Index<RangeFrom<usize>> for Mmap {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &[u8] {
        Index::index(&**self, index)
    }
}

impl Index<RangeFull> for Mmap {
    type Output = [u8];

    fn index(&self, _index: RangeFull) -> &[u8] {
        self
    }
}

impl IndexMut<Range<usize>> for Mmap {
    fn index_mut(&mut self, index: Range<usize>) -> &mut [u8] {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl IndexMut<RangeTo<usize>> for Mmap {
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut [u8] {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl IndexMut<RangeFrom<usize>> for Mmap {
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut [u8] {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl IndexMut<RangeFull> for Mmap {
    fn index_mut(&mut self, _index: RangeFull) -> &mut [u8] {
        self
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::{fs, env, iter};
    use std::io::{Read, Write};

    use super::*;

    #[test]
    fn map_file() {
        let expected_len = env::page_size() * 7 + 13;
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&path).unwrap()
                        .set_len(expected_len as u64).unwrap();

        let mut mmap = Mmap::open(path, Protection::ReadWrite).unwrap();
        let len = mmap.len();
        assert_eq!(expected_len, len);

        let zeros = iter::repeat(0).take(len).collect::<Vec<_>>();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &*mmap);

        // write values into the mmap
        mmap.as_mut().write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &*mmap);
    }

    // Check that a 0-length file will not be mapped
    #[test]
    fn map_empty_file() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&path).unwrap();

        assert!(Mmap::open(path, Protection::ReadWrite).is_err());
    }


    #[test]
    fn map_anon() {
        let expected_len = env::page_size() * 7 + 13;
        let mut mmap = Mmap::anonymous(expected_len, Protection::ReadWrite).unwrap();
        let len = mmap.len();
        assert_eq!(expected_len, len);

        let zeros = iter::repeat(0).take(len).collect::<Vec<_>>();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &*mmap);

        // write values into the mmap
        mmap.as_mut().write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &*mmap);
    }

    #[test]
    fn file_write() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let mut file = fs::OpenOptions::new()
                                       .read(true)
                                       .write(true)
                                       .create(true)
                                       .open(&path).unwrap();
        file.set_len(128).unwrap();

        let write = b"abc123";
        let mut read = [0u8; 6];

        let mut mmap = Mmap::open(&path, Protection::ReadWrite).unwrap();
        (&mut mmap[..]).write(write).unwrap();
        mmap.flush().unwrap();

        file.read(&mut read).unwrap();
        assert_eq!(write, &read);
    }
}
