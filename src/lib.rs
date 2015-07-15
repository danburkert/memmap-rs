//! A cross-platform Rust API for memory-mapped file IO.

#[macro_use]
extern crate bitflags;

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

/// Memory map protection.
///
/// Determines how a memory map may be used. If the memory map is backed by a file, then the file
/// must have permissions corresponding to the operations the protection level allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {

    /// A read-only memory map. Writes to the memory map will result in a panic.
    Read,

    /// A read-write memory map. Writes to the memory map will be reflected in the file after a
    /// call to `Mmap::flush` or after the `Mmap` is dropped.
    ReadWrite,

    /// A read, copy-on-write memory map. Writes to the memory map will not be carried through to
    /// the underlying file. It is unspecified whether changes made to the file after the memory map
    /// is created will be visible.
    ReadCopy,
}

impl Protection {

    fn as_open_options(self) -> fs::OpenOptions {
        let mut options = fs::OpenOptions::new();
        options.read(true)
               .write(self.write());

        options
    }

    /// Returns `true` if the `Protection` is writable.
    pub fn write(self) -> bool {
        match self {
            ReadWrite | ReadCopy => true,
            _ => false,
        }
    }
}

/// A memory-mapped buffer.
///
/// A file-backed `Mmap` buffer may be used to read or write data to a file. Use `Mmap::open(..)` to
/// create a file-backed memory map. An anonymous `Mmap` buffer may be used any place that an
/// in-memory byte buffer is needed, and gives the added features of a memory map. Use
/// `Mmap::anonymous(..)` to create an anonymous memory map.
///
/// Changes written to a memory-mapped file are not guaranteed to be durable until the memory map is
/// flushed, or it is dropped.
///
/// ```
/// use std::io::Write;
/// use mmap::{Mmap, Protection};
///
/// let file_mmap = Mmap::open("README.md", Protection::Read).unwrap();
/// let bytes: &[u8] = &*file_mmap;
/// assert_eq!(b"# mmap", &file_mmap[0..6]);
///
/// let mut anon_mmap = Mmap::anonymous(4096, Protection::ReadWrite).unwrap();
/// (&mut *anon_mmap).write(b"foo").unwrap();
/// assert_eq!(b"foo\0\0", &anon_mmap[0..5]);
/// ```
pub struct Mmap {
    inner: MmapInner
}

impl Mmap {

    /// Opens a file-backed memory map.
    pub fn open<P>(path: P, prot: Protection) -> io::Result<Mmap> where P: AsRef<Path> {
        MmapInner::open(path, prot).map(|inner| Mmap { inner: inner })
    }

    /// Opens an anonymous memory map.
    pub fn anonymous(len: usize, prot: Protection) -> io::Result<Mmap> {
        MmapInner::anonymous(len, prot).map(|inner| Mmap { inner: inner })
    }

    /// Flushes outstanding memory map modifications to disk.
    ///
    /// When this returns with a non-error result, all outstanding changes to a file-backed memory
    /// map are guaranteed to be durably stored. The file's metadata (including last modification
    /// timestamp) may not be updated.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    /// Asynchronously flushes outstanding memory map modifications to disk.
    ///
    /// This method initiates flushing modified pages to durable storage, but it will not wait
    /// for the operation to complete before returning. The file's metadata (including last
    /// modification timestamp) may not be updated.
    pub fn flush_async(&mut self) -> io::Result<()> {
        self.inner.flush_async()
    }

    /// Returns the length of the memory map.
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
        &(*self.inner)[index]
    }
}

impl IndexMut<usize> for Mmap {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut (*self.inner)[index]
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

    use std::{fs, iter};
    use std::io::{Read, Write};
    use std::thread;

    use super::*;

    #[test]
    fn map_file() {
        let expected_len = 128;
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
        let expected_len = 128;
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

    #[test]
    fn map_copy() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let mut file = fs::OpenOptions::new()
                                       .read(true)
                                       .write(true)
                                       .create(true)
                                       .open(&path).unwrap();
        file.set_len(128).unwrap();

        let nulls = b"\0\0\0\0\0\0";
        let write = b"abc123";
        let mut read = [0u8; 6];

        let mut mmap = Mmap::open(&path, Protection::ReadCopy).unwrap();
        (&mut mmap[..]).write(write).unwrap();
        mmap.flush().unwrap();

        // The mmap contains the write
        (&*mmap).read(&mut read).unwrap();
        assert_eq!(write, &read);

        // The file does not contain the write
        file.read(&mut read).unwrap();
        assert_eq!(nulls, &read);

        // another mmap does not contain the write
        let mmap2 = Mmap::open(&path, Protection::Read).unwrap();
        (&*mmap2).read(&mut read).unwrap();
        assert_eq!(nulls, &read);
    }

    #[test]
    fn index() {
        let mut mmap = Mmap::anonymous(128, Protection::ReadWrite).unwrap();
        mmap[0] = 42;
        assert_eq!(42, mmap[0]);
    }

    #[test]
    fn send() {
        let mut mmap = Mmap::anonymous(128, Protection::ReadWrite).unwrap();
        (&mut mmap[..]).write(b"foobar").unwrap();
        thread::spawn(move || {
            mmap.flush().unwrap();
        });
    }
}
