use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64};

pub const MAGIC: u64 = 0x5A494E435F524547; // "ZINC_REG"
pub const VERSION: u16 = 2;

/// Lives at byte 0 of the mapped region. #[repr(C)] + align(64) = one cache line,
/// no false sharing with the region data that follows.
#[repr(C, align(64))]
pub struct RegionHeader {
    pub magic: u64,            // 8  — must equal MAGIC
    pub version: u16,          // 2  — breaking change guard
    pub flags: u16,            // 2  — reserved
    pub notify_seq: AtomicU32, // 4  — futex/ulock signal counter (was _pad)
    pub capacity: u64,         // 8  — usable bytes after header
    pub ref_count: AtomicU32,  // 4
    pub owner_pid: AtomicI32,  // 4  — pid of creator
    pub created_at: u64,       // 8  — unix timestamp (nanos)
    pub name_hash: u64,        // 8  — FNV-1a of the name
    pub ring_head: AtomicU64,  // 8  — notification ring write head
    pub ring_tail: AtomicU64,  // 8  — notification ring read tail
}

#[cfg(test)]
mod tests {
    use static_assertions::assert_eq_size;

    #[test]
    fn header_fits_one_cache_line() {
        assert_eq_size!(super::RegionHeader, [u8; 64]);
    }
}
