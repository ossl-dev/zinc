use crate::Result;
use std::sync::atomic::{AtomicU32, Ordering};

/// Publish a notification and wake all Linux futex waiters.
pub fn notify(addr: &AtomicU32) {
    addr.fetch_add(1, Ordering::Release);
    #[cfg(target_os = "linux")]
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            addr.as_ptr(),
            libc::FUTEX_WAKE,
            i32::MAX,
            0,
            0,
            0,
        );
    }
}

/// Wait for a sequence change, retrying interruptions within the original timeout.
pub fn wait(addr: &AtomicU32, expected: u32, timeout_ms: u32) -> Result<()> {
    if addr.load(Ordering::Acquire) != expected {
        return Ok(());
    }
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(u64::from(timeout_ms));
    #[cfg(not(target_os = "linux"))]
    let mut spins = 0;
    loop {
        if addr.load(Ordering::Acquire) != expected {
            return Ok(());
        }
        let remaining = timeout
            .checked_sub(start.elapsed())
            .filter(|duration| !duration.is_zero())
            .ok_or(crate::ZincError::TimedOut)?;
        #[cfg(target_os = "linux")]
        {
            let ts = libc::timespec {
                tv_sec: remaining.as_secs() as libc::time_t,
                tv_nsec: remaining.subsec_nanos() as libc::c_long,
            };
            let ret = unsafe {
                libc::syscall(
                    libc::SYS_futex,
                    addr.as_ptr(),
                    libc::FUTEX_WAIT,
                    expected,
                    &ts,
                    0,
                    0,
                )
            };
            if ret == -1 {
                let source = std::io::Error::last_os_error();
                match source.raw_os_error() {
                    Some(libc::EAGAIN | libc::EINTR | libc::ETIMEDOUT) => {}
                    _ => {
                        return Err(crate::ZincError::Syscall {
                            syscall: "futex",
                            source,
                        })
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            if spins < 64 {
                spins += 1;
                std::hint::spin_loop();
            } else {
                std::thread::sleep(remaining.min(std::time::Duration::from_micros(50)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn notify_wait_same_thread() {
        let seq = AtomicU32::new(0);
        let expected = seq.load(Ordering::Acquire);
        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_millis(5));
                notify(&seq);
            });
            let result = wait(&seq, expected, 5000);
            assert!(result.is_ok(), "wait should succeed: {:?}", result);
        });
    }

    #[test]
    fn wait_timeout() {
        let seq = AtomicU32::new(0);
        let expected = seq.load(Ordering::Acquire);
        let result = wait(&seq, expected, 10);
        assert!(matches!(result, Err(crate::ZincError::TimedOut)));
    }

    #[test]
    fn notify_before_wait_returns_immediately() {
        let seq = AtomicU32::new(0);
        notify(&seq); // seq now 1
                      // wait with expected=0 sees seq=1, returns immediately
        let result = wait(&seq, 0, 1000);
        assert!(
            result.is_ok(),
            "wait should return immediately, got: {:?}",
            result
        );
    }

    #[test]
    fn multiple_notifies() {
        let seq = AtomicU32::new(0);
        for _ in 0..5 {
            notify(&seq);
        }
        assert_eq!(seq.load(Ordering::Acquire), 5);
    }

    #[test]
    fn zero_timeout_checks_sequence_first() {
        let seq = AtomicU32::new(1);
        assert!(wait(&seq, 0, 0).is_ok());
        assert!(matches!(wait(&seq, 1, 0), Err(crate::ZincError::TimedOut)));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn spurious_wakes_do_not_report_notification() {
        let seq = AtomicU32::new(0);
        thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..5 {
                    thread::sleep(Duration::from_millis(2));
                    unsafe {
                        libc::syscall(
                            libc::SYS_futex,
                            seq.as_ptr(),
                            libc::FUTEX_WAKE,
                            i32::MAX,
                            0,
                            0,
                            0,
                        );
                    }
                }
            });
            assert!(matches!(wait(&seq, 0, 20), Err(crate::ZincError::TimedOut)));
        });
    }
}
