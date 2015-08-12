use std::{io, slice};
use std::rc::Rc;
use std::cell::RefCell;

use ::Mmap;

#[cfg(target_os = "windows")]
use super::windows::MmapInner;

#[cfg(not(target_os = "windows"))]
use super::posix::MmapInner;

pub struct MmapSliver {
	// The Rc tracks when to drop
	// The RefCell allows mutable borrows of parent
	// for flushing
	parent: Rc< RefCell<Mmap> >,
	inner: MmapInner
}

pub unsafe fn carve(mmap: Mmap, carvings: Vec<(usize,usize)> ) -> Vec<MmapSliver> {
	let parent = Rc::new( RefCell::new(mmap) );

	let ptr = parent.borrow().ptr();
	let len = parent.borrow().len();



	vec![ MmapSliver{
		parent: parent,
		inner: MmapInner {
			ptr: ptr,
			len: len
		}
	}]
}

impl MmapSliver {
    pub fn flush(&mut self) -> io::Result<()> {
        self.parent.borrow_mut().flush()
    }

    pub fn flush_async(&mut self) -> io::Result<()> {
        self.parent.borrow_mut().flush_async()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn ptr(&self) -> *const u8 {
        self.inner.ptr()
    }

    pub fn mut_ptr(&mut self) -> *mut u8 {
        self.inner.mut_ptr()
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        slice::from_raw_parts(self.ptr(), self.len())
    }

    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        slice::from_raw_parts_mut(self.mut_ptr(), self.len())
    }
}

#[cfg(test)]
mod test {
	use super::*;
	use super::super::*;

	#[test]
	fn carve_mmap(){
        let expected_len = 128;
        let mut mmap = Mmap::anonymous(expected_len, Protection::ReadWrite).unwrap();


        carve(mmap, vec![(10,10)]);

        assert_eq!(mmap.len(), 123);
	}
}