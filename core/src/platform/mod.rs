#[cfg(unix)]
pub(crate) mod unix;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

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
#[cfg(target_os = "windows")]
pub(crate) fn map(name: &str, mode: CreateOrOpen) -> crate::Result<MappedFile> {
    windows::map(name, mode)
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("Zinc: unsupported platform");

#[cfg(target_os = "linux")]
pub(crate) fn unmap(_f: &mut MappedFile) -> crate::Result<()> {
    linux::unmap(_f)
}
#[cfg(target_os = "macos")]
pub(crate) fn unmap(_f: &mut MappedFile) -> crate::Result<()> {
    macos::unmap(_f)
}
#[cfg(target_os = "windows")]
pub(crate) fn unmap(_f: &mut MappedFile) -> crate::Result<()> {
    windows::unmap(_f)
}

#[cfg(target_os = "linux")]
pub(crate) fn unlink(name: &str) -> crate::Result<()> {
    linux::unlink(name)
}
#[cfg(target_os = "macos")]
pub(crate) fn unlink(name: &str) -> crate::Result<()> {
    macos::unlink(name)
}
#[cfg(target_os = "windows")]
pub(crate) fn unlink(name: &str) -> crate::Result<()> {
    windows::unlink(name)
}
