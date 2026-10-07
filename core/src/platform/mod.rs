#[cfg(unix)]
pub(crate) mod unix;

use std::ptr::NonNull;

pub(crate) struct MappedFile {
    pub ptr: NonNull<u8>,
    pub len: usize,
}

// Mappings have a stable address; access to their contents requires synchronization.
unsafe impl Send for MappedFile {}
unsafe impl Sync for MappedFile {}

impl Drop for MappedFile {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.ptr.as_ptr().cast(), self.len) };
    }
}

pub(crate) enum CreateOrOpen {
    Create(usize),
    Open,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use unix::{map, unlink};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Zinc supports Linux and macOS only (POSIX shared memory required).");
