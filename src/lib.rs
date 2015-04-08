#![cfg_attr(test, feature(page_size))]

#[macro_use]
extern crate bitflags;
extern crate libc;

#[cfg(target_os = "windows")]
extern crate kernel32;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::MmapInner;

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
mod posix;
#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
use posix::MmapInner;

use std::{fs, io};
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

pub struct Mmap {
    inner: MmapInner
}


impl Mmap {

    /// Open a file-backed memory map.
    pub fn open<P>(path: P, prot: Protection) -> io::Result<Mmap> where P: AsRef<Path> {
        MmapInner::open(path, prot).map(|inner| Mmap { inner: inner })
    }

    /// Open an anonymous memory map.
    pub fn anonymous(len: usize, prot: Protection) -> io::Result<Mmap> {
        MmapInner::anonymous(len, prot).map(|inner| Mmap { inner: inner })
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    pub fn flush_async(&mut self) -> io::Result<()> {
        self.inner.flush_async()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &*self.inner
    }
}

impl DerefMut for Mmap {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut *self.inner
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
