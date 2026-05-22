use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use zinc_core::SharedRegion;

const NAME: &str = "bench_zinc";

fn page_align(size: usize) -> usize {
    let page = page_size();
    (size + page - 1) & !(page - 1)
}

fn page_size() -> usize {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
    #[cfg(target_os = "linux")]
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
    #[cfg(windows)]
    {
        4096
    }
}

fn main() {
    // ── Notify/wait latency (cross-thread) ──
    bench_notify_latency();

    // ── Cross-process data transfer: Zinc vs Unix socket ──
    for &payload_kb in &[1, 64, 1024, 10240] {
        let payload = payload_kb * 1024;
        println!("\n--- {} KB payload ---", payload_kb);
        bench_zinc_transfer(payload);
        bench_unix_transfer(payload);
    }
}

/// Measures roundtrip time for an empty signal via cross-thread notify/wait.
/// Notifier thread runs continuously (every 1 µs) until main thread finishes.
/// This prevents the race where the notifier finishes before the waiter catches up.
fn bench_notify_latency() {
    let parent = SharedRegion::create(NAME, page_align(4096)).expect("create");
    let child = SharedRegion::open(NAME).expect("open");
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();

    let iters = 5_000;
    let start = Instant::now();

    let handle = thread::spawn(move || {
        while !done_clone.load(Ordering::Relaxed) {
            parent.notify();
            thread::sleep(Duration::from_micros(1));
        }
    });

    for _ in 0..iters {
        child.wait(5000).expect("wait");
    }

    done.store(true, Ordering::Release);
    let elapsed = start.elapsed();
    handle.join().unwrap();

    println!("\n=== Notify/wait latency (cross-thread) ===");
    println!(
        "  roundtrip: {:.1} µs avg  ({:.2?} for {} iterations)",
        elapsed.as_secs_f64() / iters as f64 * 1_000_000.0,
        elapsed,
        iters,
    );
}

/// Simulates cross-process data transfer via Zinc shared memory.
///
/// In real usage, the child handle lives in a separate process and reads
/// the same physical pages — zero copy. This benchmark approximates that
/// by writing data, notifying, and reading in the same thread (the data
/// never moves — both handles point to the same mmap'd pages).
fn bench_zinc_transfer(payload: usize) {
    let payload = page_align(payload); // must be page-aligned on all platforms
    let parent = SharedRegion::create(NAME, payload).expect("create");
    let child = SharedRegion::open(NAME).expect("open");

    let iters = pick_iters(payload);

    let start = Instant::now();
    for i in 0..iters {
        // Writer writes payload to shared memory
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        // Reader reads from the same shared memory — zero copy
        child.wait(5000).expect("wait");
        std::hint::black_box(unsafe { std::ptr::read(child.as_ptr()) });
    }
    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    println!(
        "  zinc:    {:>8.2} GB/s  ({:.2?}, {} x {} KB, {:.1} GB total)",
        total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        elapsed,
        iters,
        payload / 1024,
        total_bytes / (1024 * 1024 * 1024) as f64,
    );

    drop(child);
    drop(parent);
}

/// Simulates cross-process data transfer via Unix domain socket.
///
/// Data must be copied through the kernel: write() copies from user to
/// kernel buffer, read() copies from kernel to user buffer. Two kernel
/// copies per transfer, plus syscall overhead.
fn bench_unix_transfer(payload: usize) {
    let (mut a, mut b) = UnixStream::pair().expect("socket pair");
    // Large buffer to avoid fragmentation
    a.set_write_timeout(Some(Duration::from_secs(30))).ok();
    b.set_read_timeout(Some(Duration::from_secs(30))).ok();

    let iters = pick_iters(payload);
    let mut buf = vec![0u8; payload];

    let start = Instant::now();
    for i in 0..iters {
        buf.fill(i as u8);
        a.write_all(&buf).expect("write");
        b.read_exact(&mut buf).expect("read");
        std::hint::black_box(buf[0]);
    }
    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    println!(
        "  socket:  {:>8.2} GB/s  ({:.2?}, {} x {} KB, {:.1} GB total)",
        total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        elapsed,
        iters,
        payload / 1024,
        total_bytes / (1024 * 1024 * 1024) as f64,
    );
}

fn pick_iters(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 100_000,     // 1 KB
        p if p <= 65_536 => 10_000,    // 64 KB
        p if p <= 1_048_576 => 1_000,  // 1 MB
        _ => 100,                       // 10 MB+
    }
}
