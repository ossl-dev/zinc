use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64};

pub const MAGIC: u64 = 0x5A494E435F524547; // "ZINC_REG"
pub const VERSION: u16 = 2;

/// The versioned 64-byte header at the start of the first mapping page.
#[repr(C, align(64))]
pub struct RegionHeader {
    pub magic: AtomicU64,
    pub version: u16,
    pub flags: u16,
    pub notify_seq: AtomicU32,
    pub capacity: u64,
    pub ref_count: AtomicU32,
    pub owner_pid: AtomicI32,
    pub created_at: u64,
    pub name_hash: u64,
    pub ring_head: AtomicU64,
    pub ring_tail: AtomicU64,
}

#[cfg(test)]
mod tests {
    use super::RegionHeader;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn header_layout_matches_abi() {
        assert_eq!(size_of::<RegionHeader>(), 64);
        assert_eq!(align_of::<RegionHeader>(), 64);
        assert_eq!(
            [
                offset_of!(RegionHeader, magic),
                offset_of!(RegionHeader, version),
                offset_of!(RegionHeader, flags),
                offset_of!(RegionHeader, notify_seq),
                offset_of!(RegionHeader, capacity),
                offset_of!(RegionHeader, ref_count),
                offset_of!(RegionHeader, owner_pid),
                offset_of!(RegionHeader, created_at),
                offset_of!(RegionHeader, name_hash),
                offset_of!(RegionHeader, ring_head),
                offset_of!(RegionHeader, ring_tail),
            ],
            [0, 8, 10, 12, 16, 24, 28, 32, 40, 48, 56]
        );
    }
}
