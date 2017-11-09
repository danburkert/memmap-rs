extern crate kernel32;
extern crate winapi;

use std::{io, mem, ptr};
use std::fs::File;
use std::os::raw::c_void;
use std::os::windows::io::{AsRawHandle, RawHandle};

pub struct MmapInner {
    file: Option<File>,
    ptr: *mut c_void,
    len: usize,
    copy: bool,
}

impl MmapInner {
    /// Creates a new `MmapInner`.
    ///
    /// This is a thin wrapper around the `CreateFileMappingW` and `MapViewOfFile` system calls.
    pub fn new(
        file: &File,
        protect: winapi::DWORD,
        access: winapi::DWORD,
        offset: usize,
        len: usize,
        copy: bool,
    ) -> io::Result<MmapInner> {
        let alignment = offset % allocation_granularity();
        let aligned_offset = offset - alignment;
        let aligned_len = len + alignment;

        unsafe {
            let handle = kernel32::CreateFileMappingW(
                file.as_raw_handle(),
                ptr::null_mut(),
                protect,
                0,
                0,
                ptr::null(),
            );
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }

            let ptr = kernel32::MapViewOfFile(
                handle,
                access,
                (aligned_offset >> 16 >> 16) as winapi::DWORD,
                (aligned_offset & 0xffffffff) as winapi::DWORD,
                aligned_len as winapi::SIZE_T,
            );
            kernel32::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                Err(io::Error::last_os_error())
            } else {
                Ok(MmapInner {
                    file: Some(file.try_clone()?),
                    ptr: ptr.offset(alignment as isize),
                    len: len as usize,
                    copy: copy,
                })
            }
        }
    }

    pub fn map(len: usize, file: &File, offset: usize) -> io::Result<MmapInner> {
        let write = protection_supported(file.as_raw_handle(), winapi::PAGE_READWRITE);
        let exec = protection_supported(file.as_raw_handle(), winapi::PAGE_EXECUTE_READ);
        let mut access = winapi::FILE_MAP_READ;
        let protection = match (write, exec) {
            (true, true) => {
                access |= winapi::FILE_MAP_WRITE | winapi::FILE_MAP_EXECUTE;
                winapi::PAGE_EXECUTE_READWRITE
            }
            (true, false) => {
                access |= winapi::FILE_MAP_WRITE;
                winapi::PAGE_READWRITE
            }
            (false, true) => {
                access |= winapi::FILE_MAP_EXECUTE;
                winapi::PAGE_EXECUTE_READ
            }
            (false, false) => winapi::PAGE_READONLY,
        };

        let mut inner = MmapInner::new(file, protection, access, offset, len, false)?;
        if write || exec {
            inner.make_read_only()?;
        }
        Ok(inner)
    }

    pub fn map_exec(len: usize, file: &File, offset: usize) -> io::Result<MmapInner> {
        let write = protection_supported(file.as_raw_handle(), winapi::PAGE_READWRITE);
        let mut access = winapi::FILE_MAP_READ | winapi::FILE_MAP_EXECUTE;
        let protection = if write {
            access |= winapi::FILE_MAP_WRITE;
            winapi::PAGE_EXECUTE_READWRITE
        } else {
            winapi::PAGE_EXECUTE_READ
        };

        let mut inner = MmapInner::new(file, protection, access, offset, len, false)?;
        if write {
            inner.make_exec()?;
        }
        Ok(inner)
    }

    pub fn map_mut(len: usize, file: &File, offset: usize) -> io::Result<MmapInner> {
        let exec = protection_supported(file.as_raw_handle(), winapi::PAGE_EXECUTE_READ);
        let mut access = winapi::FILE_MAP_READ | winapi::FILE_MAP_WRITE;
        let protection = if exec {
            access |= winapi::FILE_MAP_EXECUTE;
            winapi::PAGE_EXECUTE_READWRITE
        } else {
            winapi::PAGE_READWRITE
        };

        let mut inner = MmapInner::new(file, protection, access, offset, len, false)?;
        if exec {
            inner.make_mut()?;
        }
        Ok(inner)
    }

    pub fn map_copy(len: usize, file: &File, offset: usize) -> io::Result<MmapInner> {
        let exec = protection_supported(file.as_raw_handle(), winapi::PAGE_EXECUTE_READWRITE);
        let mut access = winapi::FILE_MAP_COPY;
        let protection = if exec {
            access |= winapi::FILE_MAP_EXECUTE;
            winapi::PAGE_EXECUTE_WRITECOPY
        } else {
            winapi::PAGE_WRITECOPY
        };

        let mut inner = MmapInner::new(file, protection, access, offset, len, true)?;
        if exec {
            inner.make_mut()?;
        }
        Ok(inner)
    }

    pub fn map_anon(len: usize, _stack: bool) -> io::Result<MmapInner> {
        unsafe {
            // Create a mapping and view with maximum access permissions, then use `VirtualProtect`
            // to set the actual `Protection`. This way, we can set more permissive protection later
            // on.
            // Also see https://msdn.microsoft.com/en-us/library/windows/desktop/aa366537.aspx

            let handle = kernel32::CreateFileMappingW(
                winapi::INVALID_HANDLE_VALUE,
                ptr::null_mut(),
                winapi::PAGE_EXECUTE_READWRITE,
                (len >> 16 >> 16) as winapi::DWORD,
                (len & 0xffffffff) as winapi::DWORD,
                ptr::null(),
            );
            if handle == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }
            let access = winapi::FILE_MAP_ALL_ACCESS | winapi::FILE_MAP_EXECUTE;
            let ptr = kernel32::MapViewOfFile(handle, access, 0, 0, len as winapi::SIZE_T);
            kernel32::CloseHandle(handle);

            if ptr == ptr::null_mut() {
                return Err(io::Error::last_os_error());
            }

            let mut old = 0;
            let result = kernel32::VirtualProtect(
                ptr,
                len as winapi::SIZE_T,
                winapi::PAGE_READWRITE,
                &mut old,
            );
            if result != 0 {
                Ok(MmapInner {
                    file: None,
                    ptr: ptr,
                    len: len as usize,
                    copy: false,
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn flush(&self, offset: usize, len: usize) -> io::Result<()> {
        self.flush_async(offset, len)?;
        if let Some(ref file) = self.file {
            file.sync_data()?;
        }
        Ok(())
    }

    pub fn flush_async(&self, offset: usize, len: usize) -> io::Result<()> {
        let result = unsafe {
            kernel32::FlushViewOfFile(self.ptr.offset(offset as isize), len as winapi::SIZE_T)
        };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn virtual_protect(&mut self, protect: winapi::DWORD) -> io::Result<()> {
        unsafe {
            let alignment = self.ptr as usize % allocation_granularity();
            let ptr = self.ptr.offset(-(alignment as isize));
            let aligned_len = self.len as winapi::SIZE_T + alignment as winapi::SIZE_T;

            let mut old = 0;
            let result = kernel32::VirtualProtect(ptr, aligned_len, protect, &mut old);

            if result != 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn make_read_only(&mut self) -> io::Result<()> {
        self.virtual_protect(winapi::PAGE_READONLY)
    }

    pub fn make_exec(&mut self) -> io::Result<()> {
        if self.copy {
            self.virtual_protect(winapi::PAGE_EXECUTE_WRITECOPY)
        } else {
            self.virtual_protect(winapi::PAGE_EXECUTE_READ)
        }
    }

    pub fn make_mut(&mut self) -> io::Result<()> {
        if self.copy {
            self.virtual_protect(winapi::PAGE_WRITECOPY)
        } else {
            self.virtual_protect(winapi::PAGE_READWRITE)
        }
    }

    #[inline]
    pub fn ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    #[inline]
    pub fn mut_ptr(&mut self) -> *mut u8 {
        self.ptr as *mut u8
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
}

impl Drop for MmapInner {
    fn drop(&mut self) {
        let alignment = self.ptr as usize % allocation_granularity();
        unsafe {
            let ptr = self.ptr.offset(-(alignment as isize));
            assert!(
                kernel32::UnmapViewOfFile(ptr) != 0,
                "unable to unmap mmap: {}",
                io::Error::last_os_error()
            );
        }
    }
}

unsafe impl Sync for MmapInner {}
unsafe impl Send for MmapInner {}

fn protection_supported(handle: RawHandle, protection: winapi::DWORD) -> bool {
    unsafe {
        let handle =
            kernel32::CreateFileMappingW(handle, ptr::null_mut(), protection, 0, 0, ptr::null());
        if handle == ptr::null_mut() {
            return false;
        }
        kernel32::CloseHandle(handle);
        true
    }
}

fn allocation_granularity() -> usize {
    unsafe {
        let mut info = mem::zeroed();
        kernel32::GetSystemInfo(&mut info);
        return info.dwAllocationGranularity as usize;
    }
}
