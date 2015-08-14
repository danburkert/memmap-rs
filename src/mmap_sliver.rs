use std::{ io, slice };
use std::rc::Rc;
use std::cell::RefCell;

extern crate libc;

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

pub unsafe fn carve(mmap: Mmap, ranges: Vec<(usize,usize)> ) -> Vec<MmapSliver> {
	let parent = Rc::new( RefCell::new(mmap) );

	let ptr = parent.borrow().ptr() as *mut libc::c_void;

    ranges.into_iter().map(|(start_offset,end_offset)| {
        MmapSliver {
            parent: parent.clone(),
            inner: MmapInner {
                ptr: ptr.offset(start_offset as isize),
                len: (end_offset-start_offset)
            }
        }
    }).collect()
}

impl MmapSliver {
    pub fn flush(&mut self) -> io::Result<()> {
        self.parent.borrow_mut().flush()
    }

    // pub fn flush_async(&mut self) -> io::Result<()> {
    //     self.parent.borrow_mut().flush_async()
    // }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn ptr(&self) -> *const u8 {
        self.inner.ptr()
    }

    // pub fn mut_ptr(&mut self) -> *mut u8 {
    //     self.inner.mut_ptr()
    // }

    pub unsafe fn as_slice(&self) -> &[u8] {
        slice::from_raw_parts(self.ptr(), self.len())
    }

    // pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
    //     slice::from_raw_parts_mut(self.mut_ptr(), self.len())
    // }
}

#[cfg(test)]
mod test {
	use super::*;
	use super::super::*;

	#[test]
	fn carve_mmap(){
        let expected_len = 128;
        let mmap = Mmap::anonymous(expected_len, Protection::ReadWrite).unwrap();


        let mut slivers = unsafe {
            carve(mmap, vec![(10,10)])
        };

        assert_eq!(slivers.len(), 2);

        let ref mut sliver = slivers[0];
        sliver.flush().unwrap();
	}
}