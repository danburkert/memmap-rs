#![feature(unique)]
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

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "openbsd"))]
bitflags! {
    #[doc="Memory protection options"]
    #[derive(Debug)]
    flags Protection: libc::c_int {

        #[doc="Pages may not be accessed"]
        const NONE = libc::PROT_NONE,

        #[doc="Pages may be executed"]
        const EXEC = libc::PROT_EXEC,

        #[doc="Pages may be read"]
        const READ = libc::PROT_READ,

        #[doc="Pages may be written"]
        const WRITE = libc::PROT_WRITE,

        #[doc="Pages may be read and written"]
        const READ_WRITE = libc::PROT_READ | libc::PROT_WRITE,
    }
}

pub struct Mmap {
    ptr: ptr::Unique<u8>,
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

    pub fn open<P>(path: P,
                   prot: Protection)
                   -> io::Result<Mmap>
    where P: AsRef<Path> {
        use std::os::unix::io::AsRawFd;
        let mut options = fs::OpenOptions::new();
        options.read(prot.contains(READ))
               .write(prot.contains(WRITE));

        let file = try!(options.open(path));
        let fd = file.as_raw_fd();
        let len = try!(file.metadata()).len();

        let ptr = unsafe {
            libc::mmap(ptr::null_mut(), len as libc::size_t, prot.bits(), libc::MAP_SHARED, fd, 0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(Mmap {
                ptr: unsafe { ptr::Unique::new(ptr as *mut u8) },
                len: len as usize,
            })
        }
    }

    pub fn anonymous(len: usize, prot: Protection) -> io::Result<Mmap> {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(),
                       len as libc::size_t,
                       prot.bits(),
                       libc::MAP_SHARED | libc::MAP_ANON,
                       -1,
                       0)
        };

        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(Mmap {
                ptr: unsafe { ptr::Unique::new(ptr as *mut u8) },
                len: len as usize,
            })
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
        unsafe { slice::from_raw_parts(self.ptr.get(), self.len) }
    }
}

impl DerefMut for Mmap {

    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.get_mut(), self.len) }
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
    use std::io::Write;

    use super::*;

    #[test]
    fn empty_file_page_boundary() {
        let tempdir = tempdir::TempDir::new("open-mmap").unwrap();
        let path = tempdir.path().join("open_mmap");

        fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&path).unwrap()
                        .set_len(env::page_size() as u64).unwrap();

        let mut mmap = Mmap::open(path, READ | WRITE).unwrap();
        let len = mmap.len();

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
    fn anon_page_boundary() {
        let mut mmap = Mmap::anonymous(env::page_size(), READ | WRITE).unwrap();
        let len = mmap.len();

        let zeros = iter::repeat(0).take(len).collect::<Vec<_>>();
        let incr = (0..len).map(|n| n as u8).collect::<Vec<_>>();

        // check that the mmap is empty
        assert_eq!(&zeros[..], &*mmap);

        // write values into the mmap
        mmap.as_mut().write_all(&incr[..]).unwrap();

        // read values back
        assert_eq!(&incr[..], &*mmap);
    }
}
