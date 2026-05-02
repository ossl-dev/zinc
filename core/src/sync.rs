use std::sync::atomic::{AtomicU32, Ordering};
use crate::Result;

/// Signal all waiters on `addr`. Zero syscalls if no one is waiting (Linux futex).
pub fn notify(addr: &AtomicU32) {
    addr.fetch_add(1, Ordering::Release);
    #[cfg(target_os = "linux")]
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            addr as *const _ as *mut u32,
            libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG,
            i32::MAX,
            0,
            0,
            0,
        );
    }
}

/// Block until `addr` changes from `expected` or `timeout_ms` elapses.
pub fn wait(addr: &AtomicU32, expected: u32, timeout_ms: u32) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let ts = libc::timespec {
            tv_sec: (timeout_ms / 1000) as i64,
            tv_nsec: ((timeout_ms % 1000) * 1_000_000) as i64,
        };
        let ret = unsafe {
            libc::syscall(
                libc::SYS_futex,
                addr as *const _ as *mut u32,
                libc::FUTEX_WAIT | libc::FUTEX_PRIVATE_FLAG,
                expected,
                &ts,
                0,
                0,
            )
        };
        if ret == -1 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ETIMEDOUT) {
                return Err(crate::ZincError::TimedOut);
            }
            return Err(crate::ZincError::Platform(err));
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Adaptive wait: spin briefly, then yield, then check timeout.
        // On macOS, ulock is private API; Windows uses events (TBD).
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms as u64);
        let mut spins: u32 = 0;
        loop {
            if addr.load(Ordering::Acquire) != expected {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(crate::ZincError::TimedOut);
            }
            spins = spins.wrapping_add(1);
            if spins & 0xFF == 0 {
                std::thread::yield_now();
            } else {
                std::hint::spin_loop();
            }
        }
    }
}
