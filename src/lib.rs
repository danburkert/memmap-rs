//! A cross-platform Rust API for memory maps.

#![deny(warnings)]
#![doc(html_root_url = "https://docs.rs/memmap/0.5.2")]

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::MmapInner;

#[cfg(windows)]
pub use unix::AccessPattern;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::MmapInner;

#[cfg(unix)]
pub use unix::AccessPattern;

use std::fmt;
use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::slice;
use std::usize;
use std::ops::{Deref, DerefMut};

/// Memory map protection.
///
/// Determines how a memory map may be used. If the memory map is backed by a
/// file, then the file must have permissions corresponding to the operations
/// the protection level allows.
///
/// # Example
///
/// ```Rust
/// use std::fs::OpenOptions;
/// use memmap::Protection;
///
/// # fn try_main() -> std::io::Result<()> {
/// let file = OpenOptions::new()
///                         .read(true)
///                         .write(true)
///                         .open("README.md")?;
///
/// // Initialize a mutable memory map with `ReadCopy` protection
/// let _mmap = unsafe { memmap::file(&file)
///                             .protection(Protection::ReadCopy)
///                             .map_mut()? };
///
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap(); }
/// ```
/// __Note:__ Use [`make_read_only`] to convert a [`MmapMut`] to an [`Mmap`].
/// [`make_read_only`]: struct.MmapMut.html#method.make_read_only
/// [`Mmap`]: struct.Mmap.html
/// [`MmapMut`]: struct.MmapMut.html
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protection {

    /// A read-only memory map.
    Read,

    /// A read-write memory map. Writes to the memory map will be reflected in
    /// the file after a call to [`MmapMut::flush`](struct.MmapMut.html#method.flush)
    /// or after the `MmapMut` is dropped.
    ReadWrite,

    /// A read, copy-on-write memory map. Writes to the memory map will not be
    /// carried through to the underlying file. It is unspecified whether
    /// changes made to the file after the memory map is created will be
    /// visible.
    ReadCopy,

    /// A readable and executable mapping.
    ReadExecute,
}

// Anonymous mappings

/// Options that can be used to configure how an anonymous mapping is created.
///
/// Create this structure by calling [`memmap::anonymous()`](fn.anonymous.html),
/// then chain call methods to configure additional options, finally, call [`map()`](#method.map)
/// or [`map_mut()`](#method.map_mut).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AnonymousMmapOptions {
    protection: Option<Protection>,
    len: usize,
    stack: bool,
}

/// Configure a new anonymous mapping of `len` bytes.
///
/// # Example
///
/// ```rust
/// # use std::error::Error;
/// fn change_bytes(bytes: &mut [u8]) {
///     for i in 0..100 {
///         bytes[i] = i as u8;
///     }
/// }
///
/// fn write_to_anon() -> std::io::Result<()> {
///     let mut mmap = memmap::anonymous(4096)
///                         .protection(memmap::Protection::ReadWrite)
///                         .map_mut()?;
///     assert_eq!(mmap[51], 0);
///     change_bytes(&mut mmap);
///     assert_eq!(mmap[51], 51);
///     Ok(())
/// }
/// # fn main() { write_to_anon().unwrap(); }
/// ```
pub fn anonymous(len: usize) -> AnonymousMmapOptions {
    AnonymousMmapOptions {
        protection: None,
        len: len,
        stack: false,
    }
}

impl AnonymousMmapOptions {
    /// Make this mapping suitable to be a process or thread stack.
    ///
    /// This corresponds to `MAP_STACK` on Linux, which is currently a no-op.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// # fn try_main() -> std::io::Result<()> {
    /// let mut mmap_stack = memmap::anonymous(4096)
    ///                         .protection(memmap::Protection::ReadWrite)
    ///                         .stack()
    ///                         .map_mut()?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn stack(&mut self) -> &mut Self {
        self.stack = true;
        self
    }

    /// Set a protection to be used by this mapping.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// # fn try_main() -> std::io::Result<()> {
    /// let mut mmap_write = memmap::anonymous(4096)
    ///                         .protection(memmap::Protection::ReadWrite)
    ///                         .map_mut()?;
    ///
    /// let mut mmap_read_copy = memmap::anonymous(4096)
    ///                         .protection(memmap::Protection::ReadCopy)
    ///                         .map_mut()?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn protection(&mut self, protection: Protection) -> &mut Self {
        self.protection = Some(protection);
        self
    }

    fn map_inner(&self) -> Result<MmapInner> {
        let inner = try!(MmapInner::anonymous(self.len, self.protection.unwrap(), self.stack));
        Ok(inner)
    }

    /// Actually map this anonymous mapping into the address space.
    ///
    /// If the protection has not been [set explicitly](#method.protection), this method
    /// assumes [`ReadWrite`](enum.Protection.html#variant.ReadWrite).
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// This method *also* returns `Err` with `ErrorKind` set to `InvalidInput` if the specified
    /// protection does not allow the mapping to be mutable.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use std::io::Write;
    /// # fn try_main() -> std::io::Result<()> {
    /// let mut mmap = memmap::anonymous(4096)
    ///                     .protection(memmap::Protection::ReadWrite)
    ///                     .map_mut()?;
    /// (&mut mmap[..]).write(b"foo")?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn map_mut(&self) -> Result<MmapMut> {
        let mut this = *self;
        if this.protection.is_none() {
            this.protection = Some(Protection::ReadWrite);
        }
        match this.protection.unwrap() {
            Protection::Read | Protection::ReadExecute => Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid protection for a mutable mapping",
            )),
            Protection::ReadWrite | Protection::ReadCopy => {
                let inner = try!(this.map_inner());
                Ok( MmapMut { inner: inner } )
            }
        }
    }
}

// File-backed mappings

/// Options that can be used to configure how a file-backed mapping is created.
///
/// Create this structure by calling [`memmap::file()`](fn.file.html),
/// then chain call methods to configure additional options, finally, call [`map()`](#method.map)
/// or [`map_mut()`](#method.map_mut).
#[derive(Copy, Clone, Debug)]
pub struct FileMmapOptions<'a> {
    file: &'a File,
    protection: Option<Protection>,
    offset: usize,
    len: Option<usize>,
}

/// Configure a new file-backed mapping.
///
/// # Unsafety
///
/// This function is `unsafe`, because it's up to the caller to ensure
/// that no other process or thread is accessing the same file concurrently.
/// In particular, it is **undefined behavior** in Rust for the memory to be
/// modified by some other code while there's a reference to it.
///
/// # Example
///
/// ```rust
/// # use std::error::Error;
/// use std::fs::File;
///
/// # fn try_main() -> std::io::Result<()> {
/// let file = File::open("README.md")?;
/// let mmap = unsafe { memmap::file(&file)
///                         .offset(2)
///                         .len(6)
///                         .protection(memmap::Protection::Read)
///                         .map()? };
/// assert_eq!(b"memmap", &mmap[0..6]);
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap(); }
/// ```
pub unsafe fn file(file: &File) -> FileMmapOptions {
    FileMmapOptions {
        file: file,
        protection: None,
        offset: 0,
        len: None,
    }
}

impl<'a> FileMmapOptions<'a> {
    /// Configure this mapping to start at byte `offset` from the beginning of the file.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let (little, big) = (10, 100);
    ///
    /// let mmap_little_offset = unsafe { memmap::file(&file)
    ///                                     .offset(little)
    ///                                     .map()? };
    ///
    /// let mmap_big_offset = unsafe { memmap::file(&file)
    ///                                     .offset(big)
    ///                                     .map()? };
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn offset(&mut self, offset: usize) -> &mut Self {
        self.offset = offset;
        self
    }
    /// Configure this mapping to be `len` bytes long.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    ///
    /// let mmap = unsafe { memmap::file(&file)
    ///                         .len(25)
    ///                         .map()? };
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn len(&mut self, len: usize) -> &mut Self {
        self.len = Some(len);
        self
    }
    /// Set a protection to be used by this mapping.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    ///
    /// let mmap = unsafe { memmap::file(&file)
    ///                         .protection(memmap::Protection::Read)
    ///                         .map()? };
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn protection(&mut self, protection: Protection) -> &mut Self {
        self.protection = Some(protection);
        self
    }

    fn map_inner(&self) -> Result<MmapInner> {
        let len;
        if let Some(l) = self.len {
            len = l;
        } else {
            let l = try!(self.file.metadata()).len();
            if l > usize::MAX as u64 {
                return Err(Error::new(ErrorKind::InvalidData,
                      "file length overflows usize"));
            }
            len = l as usize - self.offset;
        }
        let inner = try!(MmapInner::open(self.file, self.protection.unwrap(), self.offset, len));
        Ok(inner)
    }

    /// Actually map this mapping into the address space.
    ///
    /// This method returns an immutable mapping, see [`map_mut()`](#method.map_mut)
    /// for a mutable version.
    ///
    /// If the protection has not been [set explicitly](#method.protection), this method
    /// assumes [`Read`](enum.Protection.html#variant.Read).
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::fs::File;
    /// # use std::error::Error;
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mmap = unsafe { memmap::file(&file)
    ///                         .protection(memmap::Protection::Read)
    ///                         .offset(20)
    ///                         .map()? };
    /// println!("{}", mmap[0]);
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn map(&self) -> Result<Mmap> {
        let mut this = *self;
        if this.protection.is_none() {
            this.protection = Some(Protection::Read);
        }
        let inner = try!(this.map_inner());
        Ok( Mmap { inner: inner } )
    }

    /// Actually map this mapping into the address space.
    ///
    /// This method returns a mutable mapping, see [`map()`](#method.map) for an immutable
    /// version.
    ///
    /// If the protection has not been [set explicitly](#method.protection), this method
    /// assumes [`ReadWrite`](enum.Protection.html#variant.ReadWrite).
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// This method *also* returns `Err` with `ErrorKind` set to `InvalidInput` if the specified
    /// protection does not allow the mapping to be mutable.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// # use std::error::Error;
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(memmap::Protection::ReadCopy)
    ///                             .map_mut()? };
    /// (&mut mmap[..]).write(b"Hello world");
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn map_mut(&self) -> Result<MmapMut> {
        let mut this = *self;
        if this.protection.is_none() {
            this.protection = Some(Protection::ReadWrite);
        }
        match this.protection.unwrap() {
            Protection::Read | Protection::ReadExecute => Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid protection for a mutable mapping",
            )),
            Protection::ReadWrite | Protection::ReadCopy => {
                let inner = try!(this.map_inner());
                Ok( MmapMut { inner: inner } )
            }
        }
    }
}

/// An immutable memory-mapped buffer.
///
/// A file-backed `Mmap` buffer may be used to read or write data to a file. Use
/// [`memmap::file(..)`](fn.file.html)`.map()` to create a file-backed memory map, or
/// [`memmap::anonymous(..)`](fn.anonymous.html)`.map()` to create an anonymous memory map.
///
/// # Example
///
/// ```
/// # use std::error::Error;
/// use std::io::Write;
/// use std::fs::File;
///
/// # fn try_main() -> std::io::Result<()> {
/// let file = File::open("README.md")?;
/// let mmap = unsafe { memmap::file(&file).map()? };
/// assert_eq!(b"# memmap", &mmap[0..8]);
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap(); }
/// ```
///
/// See [`MmapMut`](struct.MmapMut.html) for the mutable version.
pub struct Mmap {
    inner: MmapInner
}

impl Mmap {
    /// Change the `Protection` this mapping was created with.
    ///
    /// This method only changes the protection of the underlying mapping,
    /// but it doesn't make an `MmapMut` from an `Mmap`, use [`make_mut()`](#method.make_mut)
    /// method for that.
    ///
    /// If you create a read-only file-backed mapping, you can **not** use this method to make the
    /// mapping writeable. Remap the file instead.
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use memmap::Protection;
    /// use std::io::Write;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file).protection(Protection::Read).map()? };
    ///
    /// mmap.set_protection(Protection::ReadExecute);
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn set_protection(&mut self, protection: Protection) -> Result<()> {
        self.inner.set_protection(protection)
    }

    /// Change the `Protection` this mapping was created with to make it mutable.
    ///
    /// If you create a read-only file-backed mapping, you can **not** use this method to make the
    /// mapping writeable. Remap the file instead.
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// This method *also* returns `Err` with `ErrorKind` set to `InvalidInput` if the specified
    /// protection does not allow the mapping to be mutable.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::fs::File;
    /// use memmap::Protection;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mmap = unsafe { memmap::file(&file)
    ///                             .len(100)
    ///                             .protection(Protection::Read).map()? };
    ///
    /// let mut _mmap = mmap.make_mut(Protection::ReadWrite)?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn make_mut(mut self, protection: Protection) -> Result<MmapMut> {
        try!(self.inner.set_protection(protection));
        match protection {
            Protection::Read | Protection::ReadExecute => Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid protection for a mutable mapping",
            )),
            Protection::ReadWrite | Protection::ReadCopy => Ok(
                MmapMut { inner: self.inner }
            ),
        }
    }
}

impl Deref for Mmap {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.inner.ptr(), self.inner.len())
        }
    }
}

impl fmt::Debug for Mmap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Mmap {{ ptr: {:?}, len: {} }}", self.as_ptr(), self.len())
    }
}

/// A mutable memory-mapped buffer.
///
/// A file-backed `MmapMut` buffer may be used to read or write data to a file. Use
/// [`memmap::file(..)`](fn.file.html)`.map_mut()` to create a file-backed memory map. An anonymous
/// `MmapMut` buffer may be used any place that an in-memory byte buffer is needed,
/// and gives the added features of a memory map. Use
/// [`memmap::anonymous(..)`](fn.anonymous.html)`.map_mut()`
/// to create an anonymous memory map.
///
/// # Example
///
/// ```rust
/// # use std::error::Error;
/// use std::io::Write;
///
/// # fn try_main() -> std::io::Result<()> {
/// let mut mmap = memmap::anonymous(4096).map_mut()?;
/// (&mut mmap[..]).write(b"foo")?;
/// assert_eq!(b"foo\0\0", &mmap[0..5]);
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap(); }
/// ```
///
/// See [`Mmap`](struct.Mmap.html) for the immutable version.
pub struct MmapMut {
    inner: MmapInner
}

impl MmapMut {
    /// Flushes outstanding memory map modifications to disk.
    ///
    /// When this returns with a non-error result, all outstanding changes to a
    /// file-backed memory map are guaranteed to be durably stored. The file's
    /// metadata (including last modification timestamp) may not be updated.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::Write;
    /// use std::fs::File;
    /// use memmap::Protection;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    /// mmap.flush()?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn flush(&self) -> Result<()> {
        let len = self.len();
        self.inner.flush(0, len)
    }

    /// Asynchronously flushes outstanding memory map modifications to disk.
    ///
    /// This method initiates flushing modified pages to durable storage, but it
    /// will not wait for the operation to complete before returning. The file's
    /// metadata (including last modification timestamp) may not be updated.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::Write;
    /// use std::fs::File;
    /// use memmap::Protection;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    /// mmap.flush_async()?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn flush_async(&self) -> Result<()> {
        let len = self.len();
        self.inner.flush_async(0, len)
    }

    /// Hints the operating system on the expected access pattern of this section of memory.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::Write;
    /// use std::fs::File;
    /// use memmap::{Protection, AccessPattern};
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    /// mmap.advise(0usize, mmap.len(), AccessPattern::Sequential)?;
    /// for byte in &*mmap {
    ///     println!("{}", byte);
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn advise(&self, offset: usize, len: usize, advice: AccessPattern) -> Result<()> {
        self.inner.advise(offset, len, advice)
    }

    /// Flushes outstanding memory map modifications in the range to disk.
    ///
    /// The offset and length must be in the bounds of the mmap.
    ///
    /// When this returns with a non-error result, all outstanding changes to a
    /// file-backed memory in the range are guaranteed to be durable stored. The
    /// file's metadata (including last modification timestamp) may not be
    /// updated. It is not guaranteed the only the changes in the specified
    /// range are flushed; other outstanding changes to the mmap may be flushed
    /// as well.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::Write;
    /// use std::fs::File;
    /// use memmap::Protection;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .len(100)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    /// mmap.flush_range(0, 3)?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn flush_range(&self, offset: usize, len: usize) -> Result<()> {
        self.inner.flush(offset, len)
    }

    /// Asynchronously flushes outstanding memory map modifications in the range
    /// to disk.
    ///
    /// The offset and length must be in the bounds of the mmap.
    ///
    /// This method initiates flushing modified pages to durable storage, but it
    /// will not wait for the operation to complete before returning. The file's
    /// metadata (including last modification timestamp) may not be updated. It
    /// is not guaranteed that the only changes flushed are those in the
    /// specified range; other outstanding changes to the mmap may be flushed as
    /// well.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::Write;
    /// use std::fs::File;
    /// use memmap::Protection;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    /// mmap.flush_async_range(0, 3)?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn flush_async_range(&self, offset: usize, len: usize) -> Result<()> {
        self.inner.flush_async(offset, len)
    }

    /// Change the `Protection` this mapping was created with.
    ///
    /// This method only changes the protection of the underlying mapping,
    /// but it doesn't make an `Mmap` from an `MmapMut`, use
    /// [`make_read_only()`](#method.make_read_only) method for that.
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// This method *also* returns `Err` with `ErrorKind` set to `InvalidInput` if the specified
    /// protection does not allow the mapping to be mutable.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use memmap::Protection;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// mmap.set_protection(Protection::ReadCopy)?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn set_protection(&mut self, protection: Protection) -> Result<()> {
        match protection {
            Protection::Read | Protection::ReadExecute => Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid protection for a mutable mapping",
            )),
            Protection::ReadWrite | Protection::ReadCopy =>
                self.inner.set_protection(protection),
        }
    }

    /// Change the `Protection` this mapping was created with to make it immutable.
    ///
    /// # Errors
    ///
    /// This method returns `Err` when the underlying system call fails, which can happen for
    /// a variety of reasons, such as when you don't have the necessary permissions for the file.
    ///
    /// This method will **not** return `Err` if the passed `protection` is mutable.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use memmap::Protection;
    /// use std::io::Write;
    /// use std::fs::File;
    ///
    /// # fn try_main() -> std::io::Result<()> {
    /// let file = File::open("README.md")?;
    /// let mut mmap = unsafe { memmap::file(&file)
    ///                             .protection(Protection::ReadWrite)
    ///                             .map_mut()? };
    ///
    /// (&mut mmap[..]).write(b"Hi!")?;
    ///
    /// let mmap = mmap.make_read_only(Protection::Read)?;
    /// # Ok(())
    /// # }
    /// # fn main() { try_main().unwrap(); }
    /// ```
    pub fn make_read_only(mut self, protection: Protection) -> Result<Mmap> {
        try!(self.inner.set_protection(protection));
        Ok( Mmap { inner: self.inner } )
    }
}

impl Deref for MmapMut {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.inner.ptr(), self.inner.len())
        }
    }
}

impl DerefMut for MmapMut {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self.inner.mut_ptr(), self.inner.len())
        }
    }
}

impl fmt::Debug for MmapMut {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MmapMut {{ ptr: {:?}, len: {} }}", self.as_ptr(), self.len())
    }
}

#[cfg(test)]
mod test {
    mod memmap {
        pub use super::super::*;
    }
    use super::Protection;

    extern crate tempdir;

    use std::fs;
    use std::io::{Read, Write};
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn map_file() {
        let expected_len = 128;
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let file = fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path).unwrap();

        file.set_len(expected_len as u64).unwrap();

        let mut mmap = unsafe { memmap::file(&file) }
                                        .map_mut().unwrap();
        let len = mmap.len();
        assert_eq!(expected_len, len);

        let zeros = vec![0; len];
        let incr: Vec<u8> = (0..len as u8).collect();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &mmap[..]);

        // write values into the mmap
        (&mut mmap[..]).write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &mmap[..]);
    }

    /// Checks that a 0-length file will not be mapped.
    #[test]
    fn map_empty_file() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let file = fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path).unwrap();
        let mmap = unsafe { memmap::file(&file).map() };
        assert!(mmap.is_err());
    }

    #[test]
    fn map_anon() {
        let expected_len = 128;
        let mut mmap = memmap::anonymous(expected_len).map_mut().unwrap();
        let len = mmap.len();
        assert_eq!(expected_len, len);

        let zeros = vec![0; len];
        let incr: Vec<u8> = (0..len as u8).collect();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &mmap[..]);

        // write values into the mmap
        (&mut mmap[..]).write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &mmap[..]);
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

        let mut mmap = unsafe { memmap::file(&file) }
                                        .map_mut().unwrap();
        (&mut mmap[..]).write_all(write).unwrap();
        mmap.flush().unwrap();

        file.read(&mut read).unwrap();
        assert_eq!(write, &read);
    }

    #[test]
    fn flush_range() {
        let tempdir = tempdir::TempDir::new("mmap").unwrap();
        let path = tempdir.path().join("mmap");

        let file = fs::OpenOptions::new()
                                   .read(true)
                                   .write(true)
                                   .create(true)
                                   .open(&path).unwrap();
        file.set_len(128).unwrap();
        let write = b"abc123";

        let mut mmap = unsafe { memmap::file(&file) }
                                .offset(2)
                                .len(write.len())
                                .map_mut().unwrap();
        (&mut mmap[..]).write_all(write).unwrap();
        mmap.flush_range(0, write.len()).unwrap();
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

        let mut mmap = unsafe { memmap::file(&file) }
                                .protection(Protection::ReadCopy)
                                .map_mut().unwrap();

        (&mut mmap[..]).write(write).unwrap();
        mmap.flush().unwrap();

        // The mmap contains the write
        (&mmap[..]).read(&mut read).unwrap();
        assert_eq!(write, &read);

        // The file does not contain the write
        file.read(&mut read).unwrap();
        assert_eq!(nulls, &read);

        // another mmap does not contain the write
        let mmap2 = unsafe { memmap::file(&file) }
                                    .map().unwrap();
        (&mmap2[..]).read(&mut read).unwrap();
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

        let mut mmap = unsafe { memmap::file(&file) }
                                .offset(offset)
                                .len(len)
                                .map_mut().unwrap();
        assert_eq!(len, mmap.len());

        let zeros = vec![0; len];
        let incr: Vec<_> = (0..len).map(|i| i as u8).collect();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &mmap[..]);

        // write values into the mmap
        (&mut mmap[..]).write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &mmap[..]);
    }

    #[test]
    fn index() {
        let mut mmap = memmap::anonymous(128).map_mut().unwrap();
        mmap[0] = 42;
        assert_eq!(42, mmap[0]);
    }

    #[test]
    fn sync_send() {
        let mmap = Arc::new(memmap::anonymous(128).map_mut().unwrap());
        thread::spawn(move || {
            &mmap[..];
        });
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn jit_x86() {
        use std::mem;

        let mut mmap = memmap::anonymous(4096).map_mut().unwrap();

        mmap[0] = 0xB8;   // mov eax, 0xAB
        mmap[1] = 0xAB;
        mmap[2] = 0x00;
        mmap[3] = 0x00;
        mmap[4] = 0x00;
        mmap[5] = 0xC3;   // ret

        let mmap = mmap.make_read_only(Protection::ReadExecute).unwrap();

        let jitfn: extern "C" fn() -> u8 = unsafe { mem::transmute(mmap.as_ptr()) };
        assert_eq!(jitfn(), 0xab);
    }

    #[test]
    fn offset_set_protection() {
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
        let mut mmap = unsafe { memmap::file(&file) }
                                .offset(offset)
                                .len(len)
                                .map_mut().unwrap();
        assert_eq!(len, mmap.len());

        let zeros = vec![0; len];
        let incr: Vec<_> = (0..len).map(|i| i as u8).collect();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &mmap[..]);

        // write values into the mmap
        (&mut mmap[..]).write_all(&incr[..]).unwrap();

        // change to read-only protection
        let mmap = mmap.make_read_only(Protection::Read).unwrap();

        // read values back
        assert_eq!(&incr[..], &mmap[..]);
    }
}
