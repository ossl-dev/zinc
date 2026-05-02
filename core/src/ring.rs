use std::sync::atomic::{AtomicU64, Ordering};
use crossbeam_utils::CachePadded;
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
    head: *const CachePadded<AtomicU64>,
    tail: *const CachePadded<AtomicU64>,
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
            head: head as *const CachePadded<AtomicU64>,
            tail: tail as *const CachePadded<AtomicU64>,
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
}
