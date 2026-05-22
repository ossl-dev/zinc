use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};

use zinc_core::SharedRegion;

const NAME: &str = "bench_zinc";

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
/// The waiter thread actually blocks in futex/sync wait, then gets woken
/// by the notifier — this measures the true kernel-assisted wakeup latency.
fn bench_notify_latency() {
    let parent = SharedRegion::create(NAME, 4096).expect("create");
    let child = SharedRegion::open(NAME).expect("open");

    let iters = 1_000;
    let start = Instant::now();

    let handle = thread::spawn(move || {
        for _ in 0..iters {
            parent.notify();
            thread::sleep(Duration::from_micros(1));
        }
    });

    for _ in 0..iters {
        child.wait(5000).expect("wait");
    }

    let elapsed = start.elapsed();
    handle.join().unwrap();

    println!("\n=== Notify/wait latency (cross-thread) ===");
    println!(
        "  roundtrip: {:.1} µs avg  ({:.2?} for {} iterations)",
        elapsed.as_secs_f64() / iters as f64 * 1_000_000.0,
        elapsed,
        iters,
    );

    // No drop — child was moved into thread (actually no, child is
    // in the main thread, parent was moved)
    // Actually parent was moved into the spawned thread, so it's dropped there.
    // child is in main thread, drop it here.
}

/// Simulates cross-process data transfer via Zinc shared memory.
///
/// In real usage, the child handle lives in a separate process and reads
/// the same physical pages — zero copy. This benchmark approximates that
/// by writing data, notifying, and reading in the same thread (the data
/// never moves — both handles point to the same mmap'd pages).
fn bench_zinc_transfer(payload: usize) {
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
