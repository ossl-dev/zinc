use std::ffi::CString;
use std::io;
use std::ptr::NonNull;

use crate::platform::{CreateOrOpen, MappedFile};
use crate::{Result, ZincError};

pub(crate) fn shm_path(name: &str) -> CString {
    CString::new(format!("/zinc_{name}")).expect("name validated before this point")
}

pub(crate) fn map(name: &str, mode: CreateOrOpen) -> Result<MappedFile> {
    let cname = shm_path(name);
    let (fd, len) = match mode {
        CreateOrOpen::Create(size) => {
            let fd = unsafe {
                libc::shm_open(
                    cname.as_ptr(),
                    libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                    (libc::S_IRUSR | libc::S_IWUSR) as libc::c_int,
                )
            };
            if fd < 0 {
                let err = io::Error::last_os_error();
                return Err(match err.kind() {
                    io::ErrorKind::AlreadyExists => ZincError::AlreadyExists(name.into()),
                    _ => ZincError::Platform(err),
                });
            }
            if unsafe { libc::ftruncate(fd, size as libc::off_t) } < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                let _ = unsafe { libc::shm_unlink(cname.as_ptr()) };
                return Err(ZincError::Platform(err));
            }
            (fd, size)
        }
        CreateOrOpen::Open => {
            let fd = unsafe { libc::shm_open(cname.as_ptr(), libc::O_RDWR, 0) };
            if fd < 0 {
                let err = io::Error::last_os_error();
                return Err(match err.kind() {
                    io::ErrorKind::NotFound => ZincError::NotFound(name.into()),
                    _ => ZincError::Platform(err),
                });
            }
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd, &mut stat) } < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(ZincError::Platform(err));
            }
            (fd, stat.st_size as usize)
        }
    };

    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    let map_err = io::Error::last_os_error();
    unsafe { libc::close(fd) };

    if ptr == libc::MAP_FAILED {
        return Err(ZincError::Platform(map_err));
    }

    Ok(MappedFile {
        ptr: NonNull::new(ptr as *mut u8).ok_or_else(|| {
            ZincError::Platform(io::Error::other("mmap returned null"))
        })?,
        len,
    })
}

pub(crate) fn unmap(f: &mut MappedFile) -> Result<()> {
    if f.len > 0 {
        let ret = unsafe { libc::munmap(f.ptr.as_ptr() as *mut libc::c_void, f.len) };
        if ret != 0 {
            return Err(ZincError::Platform(io::Error::last_os_error()));
        }
        f.len = 0;
    }
    Ok(())
}

pub(crate) fn unlink(name: &str) -> Result<()> {
    let cname = shm_path(name);
    let ret = unsafe { libc::shm_unlink(cname.as_ptr()) };
    if ret != 0 {
        return Err(ZincError::Platform(io::Error::last_os_error()));
    }
    Ok(())
}
