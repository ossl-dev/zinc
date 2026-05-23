#[cfg(unix)]
pub(crate) mod unix;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
use std::ptr::NonNull;

pub(crate) struct MappedFile {
    pub ptr: NonNull<u8>,
    pub len: usize,
}

unsafe impl Send for MappedFile {}
unsafe impl Sync for MappedFile {}

impl MappedFile {
    pub(crate) fn dangling() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0,
        }
    }
}

pub(crate) enum CreateOrOpen {
    Create(usize),
    Open,
}

#[cfg(target_os = "linux")]
pub(crate) fn map(name: &str, mode: CreateOrOpen) -> crate::Result<MappedFile> {
    linux::map(name, mode)
}
#[cfg(target_os = "macos")]
pub(crate) fn map(name: &str, mode: CreateOrOpen) -> crate::Result<MappedFile> {
    macos::map(name, mode)
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Zinc is not supported on this platform. Supported platforms: Linux (x86_64, aarch64) and macOS (x86_64, aarch64). Windows is not supported (POSIX shm_open+mmap required).");

#[cfg(target_os = "linux")]
pub(crate) fn unmap(_f: &mut MappedFile) -> crate::Result<()> {
    linux::unmap(_f)
}
#[cfg(target_os = "macos")]
pub(crate) fn unmap(_f: &mut MappedFile) -> crate::Result<()> {
    macos::unmap(_f)
}
#[cfg(target_os = "linux")]
pub(crate) fn unlink(name: &str) -> crate::Result<()> {
    linux::unlink(name)
}
#[cfg(target_os = "macos")]
pub(crate) fn unlink(name: &str) -> crate::Result<()> {
    macos::unlink(name)
}
