use std::io;

use crate::platform::{CreateOrOpen, MappedFile};
use crate::{Result, ZincError};

pub(crate) fn map(_name: &str, _mode: CreateOrOpen) -> Result<MappedFile> {
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
