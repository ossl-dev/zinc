use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr::NonNull;

use crate::platform::{CreateOrOpen, MappedFile};
use crate::{Result, ZincError};

pub(crate) fn shm_path(name: &str) -> CString {
    CString::new(format!("/zinc_{name}")).expect("validated region name")
}

fn syscall_error(syscall: &'static str) -> ZincError {
    ZincError::Syscall {
        syscall,
        source: io::Error::last_os_error(),
    }
}

pub(crate) fn map(name: &str, mode: CreateOrOpen) -> Result<MappedFile> {
    let cname = shm_path(name);
    let creating = matches!(mode, CreateOrOpen::Create(_));
    let flags = if creating {
        libc::O_CREAT | libc::O_EXCL | libc::O_RDWR
    } else {
        libc::O_RDWR
    };
    let fd = unsafe { libc::shm_open(cname.as_ptr(), flags, 0o600) };
    if fd < 0 {
        let source = io::Error::last_os_error();
        return Err(match source.kind() {
            io::ErrorKind::AlreadyExists => ZincError::AlreadyExists(name.into()),
            io::ErrorKind::NotFound => ZincError::NotFound(name.into()),
            io::ErrorKind::PermissionDenied => ZincError::PermissionDenied,
            _ => ZincError::Syscall {
                syscall: "shm_open",
                source,
            },
        });
    }
    // shm_open returned a fresh descriptor owned by this call.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let result = (|| {
        let len = match mode {
            CreateOrOpen::Create(size) => {
                let size_on_disk =
                    libc::off_t::try_from(size).map_err(|_| ZincError::InvalidSize {
                        page_size: crate::region::page_size(),
                    })?;
                if unsafe { libc::ftruncate(fd.as_raw_fd(), size_on_disk) } < 0 {
                    return Err(syscall_error("ftruncate"));
                }
                size
            }
            CreateOrOpen::Open => {
                let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
                if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } < 0 {
                    return Err(syscall_error("fstat"));
                }
                let size = unsafe { stat.assume_init() }.st_size;
                let len = usize::try_from(size).map_err(|_| ZincError::CorruptedRegion)?;
                let page = crate::region::page_size();
                if len < page * 2 || len > isize::MAX as usize || len % page != 0 {
                    return Err(ZincError::CorruptedRegion);
                }
                len
            }
        };
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(syscall_error("mmap"));
        }
        let Some(ptr) = NonNull::new(ptr.cast::<u8>()) else {
            unsafe { libc::munmap(ptr, len) };
            return Err(ZincError::Platform(io::Error::other("mmap returned null")));
        };
        Ok(MappedFile { ptr, len })
    })();
    if creating && result.is_err() {
        unsafe { libc::shm_unlink(cname.as_ptr()) };
    }
    result
}

pub(crate) fn unlink(name: &str) -> Result<()> {
    let cname = shm_path(name);
    if unsafe { libc::shm_unlink(cname.as_ptr()) } < 0 {
        return Err(syscall_error("shm_unlink"));
    }
    Ok(())
}
