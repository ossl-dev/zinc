use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZincError {
    #[error("region already exists: {0}")]
    AlreadyExists(String),
    #[error("region not found: {0}")]
    NotFound(String),
    #[error("size must be > 0 and a multiple of page size ({page_size})")]
    InvalidSize { page_size: usize },
    #[error("name must be non-empty and contain only [a-zA-Z0-9_-]")]
    InvalidName,
    #[error("permission denied")]
    PermissionDenied,
    #[error("platform error: {0}")]
    Platform(#[from] std::io::Error),
    #[error("region is full")]
    RingFull,
    #[error("wait timed out")]
    TimedOut,
    #[error("magic mismatch — region corrupted or wrong version")]
    CorruptedRegion,
}

pub type Result<T> = std::result::Result<T, ZincError>;
