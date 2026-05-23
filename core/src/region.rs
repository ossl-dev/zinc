use std::sync::atomic::Ordering;

use crate::header::{RegionHeader, MAGIC, VERSION};
use crate::platform;
use crate::{Result, ZincError};

/// An owned or opened handle to a shared memory region.
/// `last_seq` tracks the last seen notification sequence number
/// per-handle to avoid the race where we load an already-incremented
/// seq as the "expected" value for futex WAIT, which would block
/// indefinitely waiting for the next notification.
pub struct SharedRegion {
    inner: RegionInner,
    owner: bool,
    last_seq: std::sync::atomic::AtomicU32,
}

struct RegionInner {
    name: String,
    map: platform::MappedFile,
}

impl SharedRegion {
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        validate_name(name)?;
        let page = page_size();
        if capacity == 0 || !capacity.is_multiple_of(page) {
            return Err(ZincError::InvalidSize { page_size: page });
        }
        // Reserve full first page for the header so data is page-aligned.
        // Page-aligned data lets CPU write-combining and L1 streaming
        // prefetch operate at full throughput for memset/write_bytes.
        let total = page
            .checked_add(capacity)
            .ok_or(ZincError::InvalidSize { page_size: page })?;
        let map = platform::map(name, platform::CreateOrOpen::Create(total))?;
        let hdr = unsafe { &mut *(map.ptr.as_ptr() as *mut RegionHeader) };
        hdr.magic = MAGIC;
        hdr.version = VERSION;
        hdr.capacity = capacity as u64;
        hdr.ref_count.store(1, Ordering::Release);
        hdr.owner_pid
            .store(std::process::id() as i32, Ordering::Release);
        hdr.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ZincError::Platform(std::io::Error::other(
                "system clock before unix epoch"
            )))?
            .as_nanos() as u64;
        hdr.name_hash = fnv1a(name.as_bytes());
        Ok(Self {
            inner: RegionInner {
                name: name.into(),
                map,
            },
            owner: true,
            last_seq: std::sync::atomic::AtomicU32::new(0),
        })
    }

    pub fn open(name: &str) -> Result<Self> {
        validate_name(name)?;
        let map = platform::map(name, platform::CreateOrOpen::Open)?;
        let hdr = unsafe { &*(map.ptr.as_ptr() as *const RegionHeader) };
        if hdr.magic != MAGIC {
            return Err(ZincError::CorruptedRegion);
        }
        if hdr.version != VERSION {
            return Err(ZincError::CorruptedRegion);
        }
        hdr.ref_count.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            inner: RegionInner {
                name: name.into(),
                map,
            },
            owner: false,
            last_seq: std::sync::atomic::AtomicU32::new(0),
        })
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        unsafe { self.inner.map.ptr.as_ptr().add(page_size()) }
    }

    pub fn capacity(&self) -> usize {
        self.header().capacity as usize
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    #[inline]
    pub fn notify(&self) {
        crate::sync::notify(&self.header().notify_seq)
    }

    /// Block until another handle notifies, or timeout.
    ///
    /// Uses a per-handle `last_seq` as the "expected" value for the
    /// underlying futex/sync wait. This avoids the race where:
    ///   writer: notify_seq.fetch_add(1)
    ///   reader: load notify_seq → reads 1 (already incremented)
    ///   reader: futex WAIT expected=1 → *addr == 1 → blocks forever
    ///
    /// With `last_seq`, the reader waits for a change from its own
    /// last-known value, which is always the pre-notification value.
    #[inline]
    pub fn wait(&self, timeout_ms: u32) -> Result<()> {
        let last = self.last_seq.load(Ordering::Acquire);
        let seq_addr = &self.header().notify_seq;
        // Fast path: seq already changed since last check
        if seq_addr.load(Ordering::Acquire) != last {
            self.last_seq
                .store(seq_addr.load(Ordering::Relaxed), Ordering::Release);
            return Ok(());
        }
        // Wait for seq to differ from our last-known value.
        // On Linux futex: if seq != last, returns EAGAIN (handled as Ok).
        // If seq == last, blocks until wake or timeout.
        let result = crate::sync::wait(seq_addr, last, timeout_ms);
        if result.is_ok() {
            self.last_seq
                .store(seq_addr.load(Ordering::Relaxed), Ordering::Release);
        }
        result
    }

    #[inline(always)]
    fn header(&self) -> &RegionHeader {
        unsafe { &*(self.inner.map.ptr.as_ptr() as *const RegionHeader) }
    }
}

impl Drop for SharedRegion {
    fn drop(&mut self) {
        let prev = self.header().ref_count.fetch_sub(1, Ordering::SeqCst);
        if self.owner && prev == 1 {
            let _ = platform::unlink(&self.inner.name);
        }
        let mut map = std::mem::replace(&mut self.inner.map, platform::MappedFile::dangling());
        let _ = platform::unmap(&mut map);
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
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
        #[cfg(unix)]
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
        #[cfg(windows)]
        { 4096 }
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x00000100000001b3))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::sync::atomic::Ordering;
    #[cfg(unix)]
    use crate::header::MAGIC;

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

        assert_eq!(region.header().magic, MAGIC);
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
        assert!(matches!(SharedRegion::create("", 4096), Err(ZincError::InvalidName)));
        assert!(matches!(SharedRegion::create("has space", 4096), Err(ZincError::InvalidName)));
        assert!(matches!(SharedRegion::create("has.dot", 4096), Err(ZincError::InvalidName)));
        assert!(matches!(SharedRegion::create("has/slash", 4096), Err(ZincError::InvalidName)));
        assert!(matches!(SharedRegion::create("has\0null", 4096), Err(ZincError::InvalidName)));
    }

    #[test]
    fn size_validation_strict() {
        let page = page_size();
        assert!(matches!(SharedRegion::create("t", 0), Err(ZincError::InvalidSize { .. })));
        assert!(matches!(SharedRegion::create("t", 1), Err(ZincError::InvalidSize { .. })));
        assert!(matches!(SharedRegion::create("t", page - 1), Err(ZincError::InvalidSize { .. })));
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
}

