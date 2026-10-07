use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Result, ZincError};

const DEFAULT_CAPACITY: usize = 256;

#[repr(C, align(64))]
struct RingSlot {
    seq: AtomicU64,
    value: AtomicU64,
}

/// Owns initialized ring storage. Borrow a ring with `ring()`.
pub struct RingStorage {
    slots: Box<[RingSlot]>,
    head: AtomicU64,
    tail: AtomicU64,
}

impl RingStorage {
    pub fn new(capacity: usize) -> Result<Self> {
        if capacity < 2
            || !capacity.is_power_of_two()
            || capacity
                .checked_mul(std::mem::size_of::<RingSlot>())
                .filter(|&bytes| bytes <= isize::MAX as usize)
                .is_none()
        {
            return Err(ZincError::InvalidRingCapacity);
        }
        let slots = (0..capacity)
            .map(|i| RingSlot {
                seq: AtomicU64::new(i as u64),
                value: AtomicU64::new(0),
            })
            .collect();
        Ok(Self {
            slots,
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
        })
    }

    pub fn ring(&self) -> Ring<'_> {
        Ring {
            slots: &self.slots,
            head: &self.head,
            tail: &self.tail,
        }
    }
}

impl Default for RingStorage {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY).expect("valid default capacity")
    }
}

/// A bounded queue supporting concurrent producers and consumers.
pub struct Ring<'a> {
    slots: &'a [RingSlot],
    head: &'a AtomicU64,
    tail: &'a AtomicU64,
}

impl<'a> Ring<'a> {
    /// # Safety
    /// Storage must be aligned to 64 bytes and contain 256 initialized slots.
    /// All pointers must remain valid for `'a` and be accessed only through this protocol.
    /// For an empty ring, counters are zero and each slot's sequence is its index.
    pub unsafe fn from_raw(ptr: *mut u8, head: *const AtomicU64, tail: *const AtomicU64) -> Self {
        unsafe { Self::from_raw_with_capacity(ptr, head, tail, DEFAULT_CAPACITY) }
    }

    /// # Safety
    /// The same requirements as `from_raw` apply, with `capacity` slots of 64 bytes.
    /// Capacity must be a power of two greater than one. Attaching must not reinitialize live storage.
    pub unsafe fn from_raw_with_capacity(
        ptr: *mut u8,
        head: *const AtomicU64,
        tail: *const AtomicU64,
        capacity: usize,
    ) -> Self {
        assert!(capacity >= 2 && capacity.is_power_of_two());
        Self {
            slots: unsafe { std::slice::from_raw_parts(ptr.cast::<RingSlot>(), capacity) },
            head: unsafe { &*head },
            tail: unsafe { &*tail },
        }
    }

    pub fn push(&self, value: u64) -> Result<()> {
        let mut head = self.head.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[head as usize & (self.capacity() - 1)];
            let diff = slot.seq.load(Ordering::Acquire).wrapping_sub(head) as i64;
            if diff == 0 {
                match self.head.compare_exchange_weak(
                    head,
                    head.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        slot.value.store(value, Ordering::Relaxed);
                        slot.seq.store(head.wrapping_add(1), Ordering::Release);
                        return Ok(());
                    }
                    Err(actual) => head = actual,
                }
            } else if diff < 0 {
                return Err(ZincError::RingFull);
            } else {
                head = self.head.load(Ordering::Relaxed);
            }
            std::hint::spin_loop();
        }
    }

    pub fn try_push(&self, value: u64) -> bool {
        self.push(value).is_ok()
    }

    pub fn pop(&self) -> Option<u64> {
        let mut tail = self.tail.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[tail as usize & (self.capacity() - 1)];
            let diff = slot
                .seq
                .load(Ordering::Acquire)
                .wrapping_sub(tail.wrapping_add(1)) as i64;
            if diff == 0 {
                match self.tail.compare_exchange_weak(
                    tail,
                    tail.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value = slot.value.load(Ordering::Relaxed);
                        slot.seq
                            .store(tail.wrapping_add(self.capacity() as u64), Ordering::Release);
                        return Some(value);
                    }
                    Err(actual) => tail = actual,
                }
            } else if diff < 0 {
                return None;
            } else {
                tail = self.tail.load(Ordering::Relaxed);
            }
            std::hint::spin_loop();
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Approximate occupied slots, including reservations not yet published.
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        self.head
            .load(Ordering::Relaxed)
            .wrapping_sub(tail)
            .min(self.capacity() as u64) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn remaining(&self) -> usize {
        self.capacity() - self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_and_wraparound() {
        for capacity in [2, 4, 256, 1024] {
            let storage = RingStorage::new(capacity).unwrap();
            let ring = storage.ring();
            assert_eq!(ring.capacity(), capacity);
            for cycle in 0..5 {
                assert!(ring.is_empty());
                assert_eq!(ring.pop(), None);
                for i in 0..capacity {
                    assert!(ring.try_push((cycle * capacity + i) as u64));
                }
                assert_eq!(ring.len(), capacity);
                assert_eq!(ring.remaining(), 0);
                assert!(!ring.try_push(999));
                assert!(matches!(ring.push(999), Err(ZincError::RingFull)));
                for i in 0..capacity {
                    assert_eq!(ring.pop(), Some((cycle * capacity + i) as u64));
                }
            }
        }
    }

    #[test]
    fn invalid_capacity() {
        for capacity in [0, 1, 3, 255] {
            assert!(matches!(
                RingStorage::new(capacity),
                Err(ZincError::InvalidRingCapacity)
            ));
        }
    }

    #[test]
    fn counter_overflow() {
        let storage = RingStorage::new(4).unwrap();
        let start = u64::MAX - 1;
        storage.head.store(start, Ordering::Relaxed);
        storage.tail.store(start, Ordering::Relaxed);
        for offset in 0..4 {
            let position = start.wrapping_add(offset);
            storage.slots[position as usize & 3]
                .seq
                .store(position, Ordering::Relaxed);
        }
        let ring = storage.ring();
        for value in 0..20 {
            ring.push(value).unwrap();
            assert_eq!(ring.pop(), Some(value));
        }
        assert!(ring.is_empty());
    }

    #[test]
    fn multiple_producers_and_consumers() {
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        use std::time::{Duration, Instant};

        const PRODUCERS: usize = 4;
        const PER_PRODUCER: usize = 10_000;
        const TOTAL: usize = PRODUCERS * PER_PRODUCER;
        let storage = RingStorage::new(64).unwrap();
        let ring = storage.ring();
        let received = AtomicUsize::new(0);
        let seen: Vec<_> = (0..TOTAL).map(|_| AtomicBool::new(false)).collect();
        let deadline = Instant::now() + Duration::from_secs(10);
        std::thread::scope(|scope| {
            for producer in 0..PRODUCERS {
                let ring = &ring;
                scope.spawn(move || {
                    for i in 0..PER_PRODUCER {
                        while !ring.try_push((producer * PER_PRODUCER + i) as u64) {
                            assert!(Instant::now() < deadline, "producer stalled");
                            std::thread::yield_now();
                        }
                    }
                });
            }
            for _ in 0..4 {
                scope.spawn(|| {
                    while received.load(Ordering::Relaxed) < TOTAL {
                        if let Some(value) = ring.pop() {
                            assert!(
                                !seen[value as usize].swap(true, Ordering::Relaxed),
                                "duplicate {value}"
                            );
                            received.fetch_add(1, Ordering::Relaxed);
                        } else {
                            assert!(Instant::now() < deadline, "consumer stalled");
                            std::thread::yield_now();
                        }
                    }
                });
            }
        });
        assert_eq!(received.load(Ordering::Relaxed), TOTAL);
        assert!(seen.iter().all(|item| item.load(Ordering::Relaxed)));
        assert!(ring.is_empty());
    }
}
