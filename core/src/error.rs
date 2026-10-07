use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZincError {
    #[error("region already exists: {0}")]
    AlreadyExists(String),
    #[error("region not found: {0}")]
    NotFound(String),
    #[error("size must be > 0 and a multiple of page size ({page_size})")]
    InvalidSize { page_size: usize },
    #[error("name must contain only [a-zA-Z0-9_-] and fit the platform length limit")]
    InvalidName,
    #[error("permission denied")]
    PermissionDenied,
    #[error("platform error: {0}")]
    Platform(#[from] std::io::Error),
    #[error("{syscall} failed: {source}")]
    Syscall {
        syscall: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("ring capacity must be a power of two greater than one")]
    InvalidRingCapacity,
    #[error("region is full")]
    RingFull,
    #[error("no notification is pending")]
    WouldBlock,
    #[error("wait timed out")]
    TimedOut,
    #[error("invalid region header, size, or version")]
    CorruptedRegion,
}

pub type Result<T> = std::result::Result<T, ZincError>;
