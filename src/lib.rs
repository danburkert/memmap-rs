//! A cross-platform Rust API for memory maps.

#![deny(warnings)]

#[macro_use]
extern crate bitflags;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::MmapInner;

#[cfg(unix)]
mod posix;
#[cfg(unix)]
use posix::MmapInner;

use std::{io, slice};
use std::cell::UnsafeCell;
use std::fs::{self, File};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

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
            Protection::ReadWrite | Protection::ReadCopy => true,
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
/// let file_mmap = Mmap::open_path("README.md", Protection::Read).unwrap();
/// let bytes: &[u8] = unsafe { file_mmap.as_slice() };
/// assert_eq!(b"# mmap", unsafe { &file_mmap.as_slice()[0..6] });
///
/// let mut anon_mmap = Mmap::anonymous(4096, Protection::ReadWrite).unwrap();
/// unsafe { anon_mmap.as_mut_slice() }.write(b"foo").unwrap();
/// assert_eq!(b"foo\0\0", unsafe { &anon_mmap.as_slice()[0..5] });
/// ```
pub struct Mmap {
    inner: MmapInner
}

impl Mmap {

    /// Opens a file-backed memory map.
    ///
    /// The file must be opened with read permissions, and write permissions if the supplied
    /// protection is `ReadWrite`. The file must not be empty.
    pub fn open(file: File, prot: Protection) -> io::Result<Mmap> {
        let len = try!(file.metadata()).len() as usize;
        MmapInner::open(file, prot, 0, len).map(|inner| Mmap { inner: inner })
    }

    /// Opens a file-backed memory map.
    ///
    /// The file must not be empty.
    pub fn open_path<P>(path: P, prot: Protection) -> io::Result<Mmap>
    where P: AsRef<Path> {
        let file = try!(prot.as_open_options().open(path));
        let len = try!(file.metadata()).len() as usize;
        MmapInner::open(file, prot, 0, len).map(|inner| Mmap { inner: inner })
    }

    /// Opens a file-backed memory map with the specified offset and length.
    ///
    /// The file must be opened with read permissions, and write permissions if the supplied
    /// protection is `ReadWrite`. The file must not be empty. The length must be greater than zero.
    pub fn open_with_offset(file: File,
                            prot: Protection,
                            offset: usize,
                            len: usize) -> io::Result<Mmap> {
        MmapInner::open(file, prot, offset, len).map(|inner| Mmap { inner: inner })
    }

    /// Opens an anonymous memory map.
    ///
    /// The length must be greater than zero.
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

    /// Returns a pointer to the mapped memory.
    ///
    /// See `Mmap::as_slice` for invariants that must hold when dereferencing the pointer.
    pub fn ptr(&self) -> *const u8 {
        self.inner.ptr()
    }

    /// Returns a pointer to the mapped memory.
    ///
    /// See `Mmap::as_mut_slice` for invariants that must hold when dereferencing the pointer.
    pub fn mut_ptr(&mut self) -> *mut u8 {
        self.inner.mut_ptr()
    }

    /// Returns the memory mapped file as an immutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently modified.
    pub unsafe fn as_slice(&self) -> &[u8] {
        slice::from_raw_parts(self.ptr(), self.len())
    }

    /// Returns the memory mapped file as a mutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently accessed.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        slice::from_raw_parts_mut(self.mut_ptr(), self.len())
    }

    /// Creates a splittable mmap view from the mmap.
    pub fn into_view(self) -> MmapView {
        let len = self.len();
        MmapView { inner: Rc::new(UnsafeCell::new(self)),
                   offset: 0,
                   len: len }
    }

    /// Creates a thread-safe splittable mmap view from the mmap.
    pub fn into_view_sync(self) -> MmapViewSync {
        let len = self.len();
        MmapViewSync { inner: Arc::new(UnsafeCell::new(self)),
                       offset: 0,
                       len: len }
    }
}

/// A view of a memory map.
///
/// The view may be split into disjoint ranges, each of which will share the
/// underlying memory map.
///
/// A mmap view is not cloneable.
pub struct MmapView {
    inner: Rc<UnsafeCell<Mmap>>,
    offset: usize,
    len: usize,
}

impl MmapView {

    /// Split the view into disjoint pieces at specified offset.
    ///
    /// The provided offset must be less than the view's length.
    pub fn split_at(self, offset: usize) -> (MmapView, MmapView) {
        assert!(offset < self.len, "MmapView split offset must be less than the view length");
        let MmapView { inner, offset: self_offset, len: self_len } = self;
        (MmapView { inner: inner.clone(),
                    offset: self_offset,
                    len: offset },
         MmapView { inner: inner,
                    offset: self_offset + offset,
                    len: self_len - offset })
    }

    /// Split the view into disjoint pieces with specified
    /// offets and lengths
    ///
    /// The provided offset and length must not exceed the view's length.
    pub fn carve(self, subviews: &[(usize,usize)]) -> Vec<MmapView> {
        let MmapView { inner, offset: self_offset, len: self_len } = self;

        subviews.iter().map(|&(offset, len)| {
            assert!( self_offset+offset < self_len, "MmapView carve offset+len must be less than the view length");
            MmapView {
                inner: inner.clone(),
                offset: self_offset + offset,
                len: len
            }
        }).collect()
    }

    /// Get a reference to the inner mmap.
    ///
    /// The caller must ensure that memory outside the `offset`/`len` range is
    /// not accessed.
    fn inner(&self) -> &Mmap {
        unsafe {
            &*self.inner.get()
        }
    }

    /// Get a mutable reference to the inner mmap.
    ///
    /// The caller must ensure that memory outside the `offset`/`len` range is
    /// not accessed.
    fn inner_mut(&self) -> &mut Mmap {
        unsafe {
            &mut *self.inner.get()
        }
    }

    /// Flushes outstanding view modifications to disk.
    ///
    /// When this returns with a non-error result, all outstanding changes to a file-backed memory
    /// map view are guaranteed to be durably stored. The file's metadata (including last
    /// modification timestamp) may not be updated.
    pub fn flush(&mut self) -> io::Result<()> {
        // TODO: this should be restricted to flushing the view.
        self.inner_mut().flush()
    }

    /// Asynchronously flushes outstanding memory map view modifications to disk.
    ///
    /// This method initiates flushing modified pages to durable storage, but it will not wait
    /// for the operation to complete before returning. The file's metadata (including last
    /// modification timestamp) may not be updated.
    pub fn flush_async(&mut self) -> io::Result<()> {
        // TODO: this should be restricted to flushing the view.
        self.inner_mut().flush_async()
    }

    /// Returns the length of the memory map view.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns a pointer to the mapped mapped.
    ///
    /// See `Mmap::as_slice` for invariants that must hold when dereferencing the pointer.
    pub fn ptr(&self) -> *const u8 {
        unsafe { self.inner().ptr().offset(self.offset as isize) }
    }

    /// Returns a pointer to the mapped memory.
    ///
    /// See `Mmap::as_mut_slice` for invariants that must hold when dereferencing the pointer.
    pub fn mut_ptr(&mut self) -> *mut u8 {
        unsafe { self.inner_mut().mut_ptr().offset(self.offset as isize) }
    }

    /// Returns the memory mapped file as an immutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently modified.
    pub unsafe fn as_slice(&self) -> &[u8] {
        &self.inner().as_slice()[self.offset..self.offset + self.len]
    }

    /// Returns the memory mapped file as a mutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently accessed.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.inner_mut().as_mut_slice()[self.offset..self.offset + self.len]
    }
}

/// A thread-safe view of a memory map.
///
/// The view may be split into disjoint ranges, each of which will share the
/// underlying memory map.
///
/// A mmap view is not cloneable.
pub struct MmapViewSync {
    inner: Arc<UnsafeCell<Mmap>>,
    offset: usize,
    len: usize,
}

impl MmapViewSync {

    /// Split the view into disjoint pieces.
    ///
    /// The provided offset must be less than the view's length.
    pub fn split_at(self, offset: usize) -> (MmapViewSync, MmapViewSync) {
        assert!(offset < self.len, "MmapView split offset must be less than the view length");
        let MmapViewSync { inner, offset: self_offset, len: self_len } = self;
        (MmapViewSync { inner: inner.clone(),
                    offset: self_offset,
                    len: offset },
         MmapViewSync { inner: inner,
                    offset: self_offset + offset,
                    len: self_len - offset })
    }

    /// Get a reference to the inner mmap.
    ///
    /// The caller must ensure that memory outside the `offset`/`len` range is
    /// not accessed.
    fn inner(&self) -> &Mmap {
        unsafe {
            &*self.inner.get()
        }
    }

    /// Get a mutable reference to the inner mmap.
    ///
    /// The caller must ensure that memory outside the `offset`/`len` range is
    /// not accessed.
    fn inner_mut(&self) -> &mut Mmap {
        unsafe {
            &mut *self.inner.get()
        }
    }

    /// Flushes outstanding view modifications to disk.
    ///
    /// When this returns with a non-error result, all outstanding changes to a file-backed memory
    /// map view are guaranteed to be durably stored. The file's metadata (including last
    /// modification timestamp) may not be updated.
    pub fn flush(&mut self) -> io::Result<()> {
        // TODO: this should be restricted to flushing the view.
        self.inner_mut().flush()
    }

    /// Asynchronously flushes outstanding memory map view modifications to disk.
    ///
    /// This method initiates flushing modified pages to durable storage, but it will not wait
    /// for the operation to complete before returning. The file's metadata (including last
    /// modification timestamp) may not be updated.
    pub fn flush_async(&mut self) -> io::Result<()> {
        // TODO: this should be restricted to flushing the view.
        self.inner_mut().flush_async()
    }

    /// Returns the length of the memory map view.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns a pointer to the mapped mapped.
    ///
    /// See `Mmap::as_slice` for invariants that must hold when dereferencing the pointer.
    pub fn ptr(&self) -> *const u8 {
        unsafe { self.inner().ptr().offset(self.offset as isize) }
    }

    /// Returns a pointer to the mapped memory.
    ///
    /// See `Mmap::as_mut_slice` for invariants that must hold when dereferencing the pointer.
    pub fn mut_ptr(&mut self) -> *mut u8 {
        unsafe { self.inner_mut().mut_ptr().offset(self.offset as isize) }
    }

    /// Returns the memory mapped file as an immutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently modified.
    pub unsafe fn as_slice(&self) -> &[u8] {
        &self.inner().as_slice()[self.offset..self.offset + self.len]
    }

    /// Returns the memory mapped file as a mutable slice.
    ///
    /// ## Unsafety
    ///
    /// The caller must ensure that the file is not concurrently accessed.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.inner_mut().as_mut_slice()[self.offset..self.offset + self.len]
    }
}

unsafe impl Sync for MmapViewSync {}
unsafe impl Send for MmapViewSync {}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::{fs, iter};
    use std::io::{Read, Write};
    use std::thread;
    use std::sync::Arc;

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

        let mut mmap = Mmap::open_path(path, Protection::ReadWrite).unwrap();
        let len = mmap.len();
        assert_eq!(expected_len, len);

        let zeros = iter::repeat(0).take(len).collect::<Vec<_>>();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();

        // check that the mmap is empty
        assert_eq!(&zeros[..], unsafe { mmap.as_slice() });

        // write values into the mmap
        unsafe { mmap.as_mut_slice() }.write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], unsafe { mmap.as_slice() });
    }

    /// Checks that a 0-length file will not be mapped.
    #[test]
    fn map_empty_file() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&path).unwrap();

        assert!(Mmap::open_path(path, Protection::ReadWrite).is_err());
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
        assert_eq!(&zeros[..], unsafe { mmap.as_slice() });

        // write values into the mmap
        unsafe { mmap.as_mut_slice() }.write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], unsafe { mmap.as_slice() });
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

        let mut mmap = Mmap::open_path(&path, Protection::ReadWrite).unwrap();
        unsafe { mmap.as_mut_slice() }.write(write).unwrap();
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

        let mut mmap = Mmap::open_path(&path, Protection::ReadCopy).unwrap();
        unsafe { mmap.as_mut_slice() }.write(write).unwrap();
        mmap.flush().unwrap();

        // The mmap contains the write
        unsafe { mmap.as_slice() }.read(&mut read).unwrap();
        assert_eq!(write, &read);

        // The file does not contain the write
        file.read(&mut read).unwrap();
        assert_eq!(nulls, &read);

        // another mmap does not contain the write
        let mmap2 = Mmap::open_path(&path, Protection::Read).unwrap();
        unsafe { mmap2.as_slice() }.read(&mut read).unwrap();
        assert_eq!(nulls, &read);
    }

    #[test]
    fn map_offset() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let file = fs::OpenOptions::new()
                                   .read(true)
                                   .write(true)
                                   .create(true)
                                   .open(&path)
                                   .unwrap();

        file.set_len(500000 as u64).unwrap();

        let offset = 5099;
        let len = 50050;

        let mut mmap = Mmap::open_with_offset(file,
                                              Protection::ReadWrite,
                                              offset,
                                              len).unwrap();
        assert_eq!(len, mmap.len());

        let zeros = iter::repeat(0).take(len).collect::<Vec<_>>();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();

        // check that the mmap is empty
        assert_eq!(&zeros[..], unsafe { mmap.as_slice() });

        // write values into the mmap
        unsafe { mmap.as_mut_slice() }.write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], unsafe { mmap.as_slice() });
    }

    #[test]
    fn index() {
        let mut mmap = Mmap::anonymous(128, Protection::ReadWrite).unwrap();
        unsafe { mmap.as_mut_slice()[0] = 42 };
        assert_eq!(42, unsafe { mmap.as_slice()[0] });
    }

    #[test]
    fn sync_send() {
        let mmap = Arc::new(Mmap::anonymous(128, Protection::ReadWrite).unwrap());
        thread::spawn(move || {
            unsafe {
                mmap.as_slice();
            }
        });
    }

    #[test]
    fn view() {
        let len = 128;
        let split = 32;
        let mut view = Mmap::anonymous(len, Protection::ReadWrite).unwrap().into_view();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();
        // write values into the view
        unsafe { view.as_mut_slice() }.write_all(&incr[..]).unwrap();

        let (view1, view2) = view.split_at(32);
        assert_eq!(view1.len(), split);
        assert_eq!(view2.len(), len - split);

        assert_eq!(&incr[0..split], unsafe { view1.as_slice() });
        assert_eq!(&incr[split..], unsafe { view2.as_slice() });

        let view1_subviews = view1.carve(&vec![ (0,10), (15,17) ]);
        assert_eq!(view1_subviews.len(), 2);
        assert_eq!(&incr[0..10], unsafe { view1_subviews[0].as_slice() });
        assert_eq!(&incr[15..32], unsafe { view1_subviews[1].as_slice() })
    }

    #[test]
    fn view_sync() {
        let len = 128;
        let split = 32;
        let mut view = Mmap::anonymous(len, Protection::ReadWrite).unwrap().into_view_sync();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();
        // write values into the view
        unsafe { view.as_mut_slice() }.write_all(&incr[..]).unwrap();

        let (view1, view2) = view.split_at(32);
        assert_eq!(view1.len(), split);
        assert_eq!(view2.len(), len - split);

        assert_eq!(&incr[0..split], unsafe { view1.as_slice() });
        assert_eq!(&incr[split..], unsafe { view2.as_slice() });
    }

    #[test]
    fn view_sync_send() {
        let view = Arc::new(Mmap::anonymous(128, Protection::ReadWrite).unwrap().into_view_sync());
        thread::spawn(move || {
            unsafe {
                view.as_slice();
            }
        });
    }
}
