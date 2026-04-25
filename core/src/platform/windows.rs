use std::ffi::CString;
use std::io;
use std::ptr::NonNull;

use crate::platform::{CreateOrOpen, MappedFile};
use crate::{Result, ZincError};

pub(crate) fn map(_name: &str, _mode: CreateOrOpen) -> Result<MappedFile> {
    // Windows uses CreateFileMapping + MapViewOfFile
    // Full implementation requires windows-sys or windows crate.
    // For now, return an unsupported error on non-Windows compilation targets
    // that somehow reach this codepath.
    Err(ZincError::Platform(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows platform backend not yet implemented",
    )))
}

pub(crate) fn unmap(_f: &mut MappedFile) -> Result<()> {
    Ok(())
}

pub(crate) fn unlink(_name: &str) -> Result<()> {
    Ok(())
}
