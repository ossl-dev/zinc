use std::sync::atomic::{AtomicU64, Ordering};
use crate::{Result, ZincError};

const RING_CAPACITY: usize = 256;
const MASK: u64 = (RING_CAPACITY as u64) - 1;

#[repr(C, align(64))]
struct RingSlot {
    seq: AtomicU64,
    value: AtomicU64,
}

pub struct Ring {
    slots: *const [RingSlot; RING_CAPACITY],
    head: *const AtomicU64,
    tail: *const AtomicU64,
}

unsafe impl Send for Ring {}
unsafe impl Sync for Ring {}

impl Ring {
    /// # Safety
    /// `ptr` must point to at least `RING_CAPACITY * size_of::<RingSlot>()` bytes of
    /// shared, writable memory. `head` and `tail` must be valid pointers to shared
    /// `AtomicU64` counters that outlive this `Ring`.
    pub unsafe fn from_raw(
        ptr: *mut u8,
        head: *const AtomicU64,
        tail: *const AtomicU64,
    ) -> Self {
        Self {
            slots: ptr as *const _,
            head,
            tail,
        }
    }

    pub fn push(&self, value: u64) -> Result<()> {
        let head = unsafe { (*self.head).load(Ordering::Relaxed) };
        let slot = unsafe { &(*self.slots)[(head & MASK) as usize] };
        if slot.seq.load(Ordering::Acquire) != head {
            return Err(ZincError::RingFull);
        }
        slot.value.store(value, Ordering::Relaxed);
        slot.seq.store(head + 1, Ordering::Release);
        unsafe {
            (*self.head).fetch_add(1, Ordering::Release);
        }
        Ok(())
    }

    pub fn pop(&self) -> Option<u64> {
        let tail = unsafe { (*self.tail).load(Ordering::Relaxed) };
        let slot = unsafe { &(*self.slots)[(tail & MASK) as usize] };
        if slot.seq.load(Ordering::Acquire) != tail + 1 {
            return None;
        }
        let value = slot.value.load(Ordering::Relaxed);
        slot.seq.store(tail + RING_CAPACITY as u64, Ordering::Release);
        unsafe {
            (*self.tail).fetch_add(1, Ordering::Release);
        }
        Some(value)
    }

    /// Number of slots in the ring.
    pub const fn capacity() -> usize {
        RING_CAPACITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    struct RingFixture {
        _slots: Box<[RingSlot; RING_CAPACITY]>,
        _head: Box<AtomicU64>,
        _tail: Box<AtomicU64>,
        ring: Ring,
    }

    impl RingFixture {
        fn new() -> Self {
            let slots: Box<[RingSlot; RING_CAPACITY]> = unsafe {
                let layout = std::alloc::Layout::new::<[RingSlot; RING_CAPACITY]>();
                let ptr = std::alloc::alloc_zeroed(layout) as *mut [RingSlot; RING_CAPACITY];
                Box::from_raw(ptr)
            };
            // Box to pin addresses — Ring holds raw pointers into these.
            let head = Box::new(AtomicU64::new(0));
            let tail = Box::new(AtomicU64::new(0));

            for i in 0..RING_CAPACITY {
                slots[i].seq.store(i as u64, Ordering::Relaxed);
            }

            let ring = unsafe {
                Ring::from_raw(
                    slots.as_ptr() as *mut u8,
                    head.as_ref() as *const AtomicU64,
                    tail.as_ref() as *const AtomicU64,
                )
            };

            Self { _slots: slots, _head: head, _tail: tail, ring }
        }
    }

    #[test]
    fn push_pop_single() {
        let fix = RingFixture::new();
        fix.ring.push(42).expect("push");
        assert_eq!(fix.ring.pop(), Some(42));
    }

    #[test]
    fn push_pop_sequence() {
        let fix = RingFixture::new();
        for i in 0..100u64 {
            fix.ring.push(i).expect("push");
        }
        for i in 0..100u64 {
            assert_eq!(fix.ring.pop(), Some(i));
        }
    }

    #[test]
    fn pop_empty_returns_none() {
        let fix = RingFixture::new();
        assert_eq!(fix.ring.pop(), None);
    }

    #[test]
    fn push_full_returns_error() {
        let fix = RingFixture::new();
        for i in 0..RING_CAPACITY as u64 {
            fix.ring.push(i).expect("push");
        }
        assert!(matches!(fix.ring.push(999), Err(ZincError::RingFull)));
    }

    #[test]
    fn wrap_around_works() {
        let fix = RingFixture::new();
        // Fill and drain multiple times to test wrap-around
        for cycle in 0..5 {
            for i in 0..(RING_CAPACITY / 2) as u64 {
                fix.ring.push(i + cycle * 1000).expect("push");
            }
            for _ in 0..(RING_CAPACITY / 2) {
                assert!(fix.ring.pop().is_some());
            }
        }
    }

    #[test]
    fn mpsc_stress() {
        use std::thread;
        use std::sync::Arc;

        let expected_count = 50_000u64;
        // We need shared ownership, so wrap the fixture
        // Since Ring is Send+Sync (via raw pointers), share it in an Arc
        let slots: Box<[RingSlot; RING_CAPACITY]> = unsafe {
            let layout = std::alloc::Layout::new::<[RingSlot; RING_CAPACITY]>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut [RingSlot; RING_CAPACITY];
            Box::from_raw(ptr)
        };
        for i in 0..RING_CAPACITY {
            slots[i].seq.store(i as u64, Ordering::Relaxed);
        }

        let slots_ptr = Arc::new(unsafe {
            std::ptr::NonNull::new_unchecked(Box::into_raw(slots) as *mut [RingSlot; RING_CAPACITY])
        });

        let head = Arc::new(AtomicU64::new(0));
        let tail = Arc::new(AtomicU64::new(0));

        let ring = Arc::new(unsafe {
            Ring::from_raw(
                slots_ptr.as_ptr() as *mut u8,
                Arc::as_ptr(&head) as *const AtomicU64,
                Arc::as_ptr(&tail) as *const AtomicU64,
            )
        });

        let ring_p = Arc::clone(&ring);
        let producer = thread::spawn(move || {
            for i in 0..expected_count {
                while ring_p.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });

        let ring_c = Arc::clone(&ring);
        let consumer = thread::spawn(move || {
            let mut received = 0u64;
            let mut last = None;
            while received < expected_count {
                if let Some(val) = ring_c.pop() {
                    if let Some(prev) = last {
                        assert_eq!(val, prev + 1, "out-of-order: {prev} -> {val}");
                    }
                    last = Some(val);
                    received += 1;
                }
            }
            received
        });

        producer.join().unwrap();
        let count = consumer.join().unwrap();
        assert_eq!(count, expected_count);
    }
}
