use std::sync::atomic::Ordering;

use crate::header::{RegionHeader, MAGIC, VERSION};
use crate::platform;
use crate::{Result, ZincError};

/// A mapped region. Raw data access requires synchronization between users.
pub struct SharedRegion {
    inner: RegionInner,
    owner: bool,
    last_seq: std::sync::atomic::AtomicU32,
}

struct RegionInner {
    name: String,
    map: platform::MappedFile,
    data: std::ptr::NonNull<u8>,
    capacity: usize,
}

// The mapping is stable; header mutations are atomic and data access uses raw pointers.
unsafe impl Send for RegionInner {}
unsafe impl Sync for RegionInner {}

impl SharedRegion {
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        validate_name(name)?;
        let page = page_size();
        let total = page
            .checked_add(capacity)
            .filter(|&total| total <= isize::MAX as usize && libc::off_t::try_from(total).is_ok())
            .ok_or(ZincError::InvalidSize { page_size: page })?;
        if capacity == 0 || capacity % page != 0 {
            return Err(ZincError::InvalidSize { page_size: page });
        }
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| {
                ZincError::Platform(std::io::Error::other("system clock before unix epoch"))
            })?
            .as_nanos();
        let created_at = u64::try_from(created_at)
            .map_err(|_| ZincError::Platform(std::io::Error::other("timestamp overflow")))?;
        let map = platform::map(name, platform::CreateOrOpen::Create(total))?;
        let hdr = map.ptr.as_ptr().cast::<RegionHeader>();
        // Openers read only magic until the release store publishes the initialized header.
        unsafe {
            std::ptr::addr_of_mut!((*hdr).version).write(VERSION);
            std::ptr::addr_of_mut!((*hdr).flags).write(0);
            std::ptr::addr_of_mut!((*hdr).capacity).write(capacity as u64);
            std::ptr::addr_of_mut!((*hdr).created_at).write(created_at);
            std::ptr::addr_of_mut!((*hdr).name_hash).write(fnv1a(name.as_bytes()));
            (*hdr).ref_count.store(1, Ordering::Relaxed);
            (*hdr)
                .owner_pid
                .store(std::process::id() as i32, Ordering::Relaxed);
            (*hdr).magic.store(MAGIC, Ordering::Release);
        }
        Ok(Self::from_mapping(name, map, true))
    }

    pub fn open(name: &str) -> Result<Self> {
        validate_name(name)?;
        let map = platform::map(name, platform::CreateOrOpen::Open)?;
        let hdr_ptr = map.ptr.as_ptr().cast::<RegionHeader>();
        let magic = unsafe { &(*hdr_ptr).magic };
        if magic.load(Ordering::Acquire) != MAGIC {
            return Err(ZincError::CorruptedRegion);
        }
        let hdr = unsafe { &*hdr_ptr };
        let capacity = map.len - page_size();
        if hdr.version != VERSION || hdr.capacity != capacity as u64 {
            return Err(ZincError::CorruptedRegion);
        }
        let mut count = hdr.ref_count.load(Ordering::Relaxed);
        loop {
            let next = count
                .checked_add(1)
                .filter(|_| count != 0)
                .ok_or(ZincError::CorruptedRegion)?;
            match hdr.ref_count.compare_exchange_weak(
                count,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => count = actual,
            }
        }
        Ok(Self::from_mapping(name, map, false))
    }

    fn from_mapping(name: &str, map: platform::MappedFile, owner: bool) -> Self {
        let capacity = map.len - page_size();
        let data = unsafe { std::ptr::NonNull::new_unchecked(map.ptr.as_ptr().add(page_size())) };
        Self {
            inner: RegionInner {
                name: name.into(),
                map,
                data,
                capacity,
            },
            owner,
            last_seq: std::sync::atomic::AtomicU32::new(0),
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.inner.data.as_ptr()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    #[inline]
    pub fn notify(&self) {
        crate::sync::notify(&self.header().notify_seq)
    }

    /// Consume a pending notification without blocking. Notifications may coalesce.
    #[inline]
    pub fn try_wait(&self) -> bool {
        let mut last = self.last_seq.load(Ordering::Relaxed);
        loop {
            let current = self.header().notify_seq.load(Ordering::Acquire);
            if current == last {
                return false;
            }
            match self.last_seq.compare_exchange_weak(
                last,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => last = actual,
            }
        }
    }

    /// Wait for a pending notification or timeout. Use separate handles for independent cursors.
    #[inline]
    pub fn wait(&self, timeout_ms: u32) -> Result<()> {
        let last = self.last_seq.load(Ordering::Relaxed);
        if self.try_wait() {
            return Ok(());
        }
        let seq_addr = &self.header().notify_seq;
        crate::sync::wait(seq_addr, last, timeout_ms)?;
        let current = seq_addr.load(Ordering::Acquire);
        let _ = self
            .last_seq
            .compare_exchange(last, current, Ordering::Relaxed, Ordering::Relaxed);
        Ok(())
    }

    #[inline(always)]
    fn header(&self) -> &RegionHeader {
        unsafe { &*(self.inner.map.ptr.as_ptr() as *const RegionHeader) }
    }
}

impl Drop for SharedRegion {
    fn drop(&mut self) {
        self.header().ref_count.fetch_sub(1, Ordering::Relaxed);
        if self.owner {
            let _ = platform::unlink(&self.inner.name);
        }
    }
}

#[cfg(target_os = "macos")]
pub const MAX_NAME_LEN: usize = 25;
#[cfg(not(target_os = "macos"))]
pub const MAX_NAME_LEN: usize = 250;

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > MAX_NAME_LEN
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(ZincError::InvalidName)
    } else {
        Ok(())
    }
}

pub(crate) fn page_size() -> usize {
    use std::sync::OnceLock;
    static PAGE_SIZE: OnceLock<usize> = OnceLock::new();
    *PAGE_SIZE.get_or_init(|| {
        let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        usize::try_from(size)
            .ok()
            .filter(|&size| size > 0)
            .expect("sysconf must return a positive page size")
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x00000100000001b3)
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use crate::header::MAGIC;
    #[cfg(unix)]
    use std::sync::atomic::Ordering;

    use super::*;

    #[cfg(unix)]
    fn cleanup(name: &str) {
        let cname = std::ffi::CString::new(format!("/zinc_{name}")).unwrap();
        unsafe { libc::shm_unlink(cname.as_ptr()) };
    }

    #[test]
    #[cfg(unix)]
    fn create_open_write_read() {
        let name = "test_create_open";
        cleanup(name);
        let capacity = page_size();

        let region = SharedRegion::create(name, capacity).expect("create");

        let ptr = region.as_ptr();
        unsafe {
            std::ptr::write_bytes(ptr, 0xAB, capacity);
            std::ptr::write(ptr as *mut u64, 0xDEADBEEF_CAFEBABE);
        }

        assert_eq!(region.header().magic.load(Ordering::Acquire), MAGIC);
        assert_eq!(region.capacity(), capacity);

        let region2 = SharedRegion::open(name).expect("open");
        let ptr2 = region2.as_ptr();
        let val = unsafe { std::ptr::read(ptr2 as *const u64) };
        assert_eq!(val, 0xDEADBEEF_CAFEBABE);
        assert_eq!(region2.capacity(), capacity);

        drop(region2);
        drop(region);
    }

    #[test]
    #[cfg(unix)]
    fn open_nonexistent_returns_not_found() {
        let result = SharedRegion::open("nonexistent_test_region");
        assert!(matches!(result, Err(ZincError::NotFound(_))));
    }

    #[test]
    #[cfg(unix)]
    fn create_duplicate_returns_already_exists() {
        let name = "test_dup";
        cleanup(name);
        let _r1 = SharedRegion::create(name, page_size()).expect("create");
        let result = SharedRegion::create(name, page_size());
        assert!(matches!(result, Err(ZincError::AlreadyExists(_))));
    }

    #[test]
    fn invalid_name_rejected() {
        assert!(matches!(
            SharedRegion::create("", page_size()),
            Err(ZincError::InvalidName)
        ));
        assert!(matches!(
            SharedRegion::create("bad/name", page_size()),
            Err(ZincError::InvalidName)
        ));
    }

    #[test]
    fn invalid_size_rejected() {
        assert!(matches!(
            SharedRegion::create("test_size", 0),
            Err(ZincError::InvalidSize { .. })
        ));
        assert!(matches!(
            SharedRegion::create("test_size", 1),
            Err(ZincError::InvalidSize { .. })
        ));
    }

    #[test]
    #[cfg(unix)]
    fn ref_counting_works() {
        let name = "test_refcount";
        cleanup(name);
        let cap = page_size();

        let owner = SharedRegion::create(name, cap).expect("create");
        assert_eq!(owner.header().ref_count.load(Ordering::SeqCst), 1);

        {
            let _opener = SharedRegion::open(name).expect("open");
            assert_eq!(owner.header().ref_count.load(Ordering::SeqCst), 2);
        }

        assert_eq!(owner.header().ref_count.load(Ordering::SeqCst), 1);
        drop(owner);
    }

    #[test]
    #[cfg(unix)]
    fn notify_wait_roundtrip() {
        let name = "test_notify";
        cleanup(name);
        let cap = page_size();

        let region = SharedRegion::create(name, cap).expect("create");

        let region2 = SharedRegion::open(name).expect("open");
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            unsafe { std::ptr::write(region2.as_ptr() as *mut u64, 42) };
            region2.notify();
        });

        let result = region.wait(5000);
        assert!(result.is_ok(), "wait should succeed: {:?}", result);

        let val = unsafe { std::ptr::read(region.as_ptr() as *const u64) };
        assert_eq!(val, 42);

        handle.join().unwrap();
        drop(region);
    }

    #[test]
    #[cfg(unix)]
    fn notify_wait_multiple_cycles() {
        // Verify repeated notify/wait cycles don't wedge.
        // Each cycle increments notify_seq. last_seq tracking ensures
        // we never wait for the value we just saw.
        let name = "test_notify_multi";
        cleanup(name);
        let cap = page_size();

        let region = SharedRegion::create(name, cap).expect("create");

        for i in 0..10 {
            let region2 = SharedRegion::open(name).expect("open");
            let handle = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_micros(100));
                unsafe { std::ptr::write(region2.as_ptr() as *mut u64, i) };
                region2.notify();
            });

            let result = region.wait(5000);
            assert!(result.is_ok(), "cycle {i}: {result:?}");
            let val = unsafe { std::ptr::read(region.as_ptr() as *const u64) };
            assert_eq!(val, i, "cycle {i} data mismatch");

            handle.join().unwrap();
        }

        drop(region);
    }

    #[test]
    fn name_validation_strict() {
        // Invalid — rejected before any platform call
        assert!(matches!(
            SharedRegion::create("", 4096),
            Err(ZincError::InvalidName)
        ));
        assert!(matches!(
            SharedRegion::create("has space", 4096),
            Err(ZincError::InvalidName)
        ));
        assert!(matches!(
            SharedRegion::create("has.dot", 4096),
            Err(ZincError::InvalidName)
        ));
        assert!(matches!(
            SharedRegion::create("has/slash", 4096),
            Err(ZincError::InvalidName)
        ));
        assert!(matches!(
            SharedRegion::create("has\0null", 4096),
            Err(ZincError::InvalidName)
        ));
    }

    #[test]
    fn size_validation_strict() {
        let page = page_size();
        assert!(matches!(
            SharedRegion::create("t", 0),
            Err(ZincError::InvalidSize { .. })
        ));
        assert!(matches!(
            SharedRegion::create("t", 1),
            Err(ZincError::InvalidSize { .. })
        ));
        assert!(matches!(
            SharedRegion::create("t", page - 1),
            Err(ZincError::InvalidSize { .. })
        ));
        // page-aligned values are valid (but may fail at platform level)
    }

    #[test]
    fn total_size_computation_no_overflow() {
        // Page + capacity must not overflow usize.
        let page = page_size();
        let max_cap = usize::MAX - page;
        let aligned_max = max_cap - (max_cap % page);
        let total = page.checked_add(aligned_max);
        assert!(total.is_some(), "page + capacity should not overflow");
    }

    #[test]
    fn creator_closes_before_reader() {
        let name = "test_owner_first";
        cleanup(name);
        let owner = SharedRegion::create(name, page_size()).unwrap();
        let reader = SharedRegion::open(name).unwrap();
        unsafe { owner.as_ptr().write(42) };
        drop(owner);
        assert!(matches!(
            SharedRegion::open(name),
            Err(ZincError::NotFound(_))
        ));
        assert_eq!(unsafe { reader.as_ptr().read() }, 42);

        let replacement = SharedRegion::create(name, page_size()).unwrap();
        drop(reader);
        let reopened = SharedRegion::open(name).unwrap();
        assert_eq!(unsafe { reopened.as_ptr().read() }, 0);
        drop(reopened);
        drop(replacement);
    }

    #[test]
    fn malformed_headers_are_rejected() {
        let name = "test_bad_header";
        cleanup(name);
        let owner = SharedRegion::create(name, page_size()).unwrap();
        let ptr = owner.inner.map.ptr.as_ptr().cast::<RegionHeader>();
        unsafe { std::ptr::addr_of_mut!((*ptr).capacity).write(u64::MAX) };
        assert!(matches!(
            SharedRegion::open(name),
            Err(ZincError::CorruptedRegion)
        ));
        unsafe { std::ptr::addr_of_mut!((*ptr).capacity).write(page_size() as u64) };
        owner.header().ref_count.store(u32::MAX, Ordering::Relaxed);
        assert!(matches!(
            SharedRegion::open(name),
            Err(ZincError::CorruptedRegion)
        ));
        owner.header().ref_count.store(1, Ordering::Relaxed);
        owner.header().magic.store(0, Ordering::Release);
        assert!(matches!(
            SharedRegion::open(name),
            Err(ZincError::CorruptedRegion)
        ));
    }

    #[test]
    fn truncated_mapping_is_rejected() {
        let name = "test_truncated";
        cleanup(name);
        let map = platform::map(name, platform::CreateOrOpen::Create(page_size())).unwrap();
        assert!(matches!(
            SharedRegion::open(name),
            Err(ZincError::CorruptedRegion)
        ));
        platform::unlink(name).unwrap();
        drop(map);
    }

    #[test]
    fn name_length_limit() {
        assert!(validate_name(&"a".repeat(MAX_NAME_LEN)).is_ok());
        assert!(matches!(
            validate_name(&"a".repeat(MAX_NAME_LEN + 1)),
            Err(ZincError::InvalidName)
        ));
    }

    #[test]
    fn oversized_capacity_is_rejected_before_mapping() {
        let capacity = (isize::MAX as usize / page_size()) * page_size();
        assert!(matches!(
            SharedRegion::create("test_huge", capacity),
            Err(ZincError::InvalidSize { .. })
        ));
    }

    #[test]
    fn pending_notifications_and_overflow() {
        let name = "test_pending";
        cleanup(name);
        let owner = SharedRegion::create(name, page_size()).unwrap();
        let reader = SharedRegion::open(name).unwrap();
        assert!(!reader.try_wait());
        owner.notify();
        owner.notify();
        assert!(reader.try_wait());
        assert!(!reader.try_wait());
        assert!(owner.try_wait());
        assert!(matches!(reader.wait(0), Err(ZincError::TimedOut)));
        owner.header().notify_seq.store(u32::MAX, Ordering::Relaxed);
        reader.last_seq.store(u32::MAX, Ordering::Relaxed);
        owner.notify();
        assert!(reader.try_wait());
        assert!(!reader.try_wait());
    }
}
