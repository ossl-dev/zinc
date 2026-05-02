use std::sync::atomic::Ordering;

use crate::header::{RegionHeader, MAGIC, VERSION};
use crate::platform;
use crate::{Result, ZincError};

pub struct SharedRegion {
    inner: RegionInner,
    owner: bool,
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
        let total = std::mem::size_of::<RegionHeader>() + capacity;
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
        })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        unsafe { self.inner.map.ptr.as_ptr().add(std::mem::size_of::<RegionHeader>()) }
    }

    pub fn capacity(&self) -> usize {
        self.header().capacity as usize
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn notify(&self) {
        crate::sync::notify(&self.header().notify_seq)
    }

    pub fn wait(&self, timeout_ms: u32) -> Result<()> {
        let expected = self.header().notify_seq.load(Ordering::Acquire);
        crate::sync::wait(&self.header().notify_seq, expected, timeout_ms)
    }

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

fn page_size() -> usize {
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
    use std::sync::atomic::Ordering;
    use crate::header::MAGIC;

    use super::*;

    fn cleanup(name: &str) {
        let cname = std::ffi::CString::new(format!("/zinc_{name}")).unwrap();
        unsafe { libc::shm_unlink(cname.as_ptr()) };
    }

    #[test]
    fn create_open_write_read() {
        let name = "test_create_open";
        cleanup(name);
        let name = "test_create_open";
        let capacity = page_size();

        // Create
        let region = SharedRegion::create(name, capacity).expect("create");

        // Write pattern
        let ptr = region.as_ptr();
        unsafe {
            std::ptr::write_bytes(ptr, 0xAB, capacity);
            std::ptr::write(ptr as *mut u64, 0xDEADBEEF_CAFEBABE);
        }

        // Verify magic in header
        assert_eq!(region.header().magic, MAGIC);
        assert_eq!(region.capacity(), capacity);

        // Open in same process (second handle)
        let region2 = SharedRegion::open(name).expect("open");
        let ptr2 = region2.as_ptr();
        let val = unsafe { std::ptr::read(ptr2 as *const u64) };
        assert_eq!(val, 0xDEADBEEF_CAFEBABE);

        // Verify capacity matches
        assert_eq!(region2.capacity(), capacity);

        drop(region2);
        drop(region); // owner drops last, unlinks
    }

    #[test]
    fn open_nonexistent_returns_not_found() {
        let result = SharedRegion::open("nonexistent_test_region");
        assert!(matches!(result, Err(ZincError::NotFound(_))));
    }

    #[test]
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
            SharedRegion::create("test_size", 1), // Not page-aligned
            Err(ZincError::InvalidSize { .. })
        ));
    }

    #[test]
    fn ref_counting_works() {
        let name = "test_refcount";
        cleanup(name);
        let cap = page_size();

        let owner = SharedRegion::create(name, cap).expect("create");
        let ref_count_before = owner.header().ref_count.load(Ordering::SeqCst);
        assert_eq!(ref_count_before, 1);

        {
            let _opener = SharedRegion::open(name).expect("open");
            let after_open = owner.header().ref_count.load(Ordering::SeqCst);
            assert_eq!(after_open, 2);
        } // opener dropped

        let after_drop = owner.header().ref_count.load(Ordering::SeqCst);
        assert_eq!(after_drop, 1);

        drop(owner);
    }

    #[test]
    fn notify_wait_roundtrip() {
        let name = "test_notify";
        cleanup(name);
        let cap = page_size();

        let region = SharedRegion::create(name, cap).expect("create");

        // Writer thread
        let region2 = SharedRegion::open(name).expect("open");
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let ptr = region2.as_ptr();
            unsafe { std::ptr::write(ptr as *mut u64, 42) };
            region2.notify();
        });

        // Wait for notification
        let result = region.wait(5000);
        assert!(result.is_ok(), "wait should succeed: {:?}", result);

        let val = unsafe { std::ptr::read(region.as_ptr() as *const u64) };
        assert_eq!(val, 42);

        handle.join().unwrap();
        drop(region);
    }
}

