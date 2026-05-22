#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use zinc_core::SharedRegion;

const NAME: &str = "bench_zinc";

fn cleanup(name: &str) {
    let cname = std::ffi::CString::new(format!("/zinc_{name}")).ok();
    if let Some(cn) = cname {
        unsafe { libc::shm_unlink(cn.as_ptr()); }
    }
}

fn create_region(name: &str, size: usize) -> zinc_core::SharedRegion {
    cleanup(name);
    SharedRegion::create(name, page_align(size)).expect("create")
}

fn open_region(name: &str) -> zinc_core::SharedRegion {
    SharedRegion::open(name).expect("open")
}

fn page_align(size: usize) -> usize {
    let page = page_size();
    (size + page - 1) & !(page - 1)
}

fn page_size() -> usize {
    #[cfg(unix)]
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
    #[cfg(windows)]
    {
        4096
    }
}

fn main() {
    bench_notify_latency();

    let sizes: &[usize] = &[1, 64, 1024, 10240, 1048576]; // KB
    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);
        println!("\n--- {} KB payload ---", payload_kb);
        bench_zinc_transfer(nominal, aligned);
        bench_unix_transfer(nominal);
    }
}

/// Measures notify/wait roundtrip.
/// Waiter enters wait() first (blocks on spin-loop), notifier fires 1µs
/// later. Time recorded is from wait() call to return — the true wakeup
/// latency of the spin-wait or futex mechanism.
fn bench_notify_latency() {
    let parent = create_region(NAME, 4096);
    let child = open_region(NAME);

    // Notifier runs until done flag, ensuring it never finishes before waiter.
    let done = Arc::new(AtomicBool::new(false));
    let done_signal = done.clone();
    let handle = thread::spawn(move || {
        while !done_signal.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_micros(1));
            parent.notify();
        }
    });

    // On macOS (spin-wait), each wait() call that actually blocks sees
    // the next notify within ~1µs (notifier's sleep interval). The total
    // time divided by iterations gives a ballpark wakeup latency.
    let iters = 5_000;
    let start = Instant::now();
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
/// Both parent and child point to the same mmap'd pages.
/// `nominal` is the logical payload size for display;
/// `aligned` is the page-aligned region size used for create().
fn bench_zinc_transfer(nominal: usize, aligned: usize) {
    let parent = create_region(NAME, aligned);
    let child = open_region(NAME);

    let iters = pick_iters(nominal); // use NOMINAL size for iteration count
    let payload = aligned; // region size for write_bytes

    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
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
        nominal / 1024,
        total_bytes / (1024 * 1024 * 1024) as f64,
    );

    drop(child);
    drop(parent);
}

/// Simulates cross-process data transfer via Unix domain socket.
///
/// Uses a persistent writer thread with a sync channel to avoid
/// per-iteration thread spawn overhead. The rendezvous channel
/// (sync_channel(0)) ensures write completes before main thread reads.
fn bench_unix_transfer(payload: usize) {
    let (a, mut b) = UnixStream::pair().expect("socket pair");
    let iters = pick_iters(payload);
    let mut buf = vec![0u8; payload];

    // Spawn one persistent writer thread
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let mut writer = a.try_clone().expect("clone");
    let w = thread::spawn(move || {
        while let Ok(data) = rx.recv() {
            writer.write_all(&data).expect("write");
        }
    });

    let start = Instant::now();
    for i in 0..iters {
        buf.fill(i as u8);
        // Send data to writer via rendezvous channel
        tx.send(buf.clone()).expect("send");
        b.read_exact(&mut buf).expect("read");
        std::hint::black_box(buf[0]);
    }
    drop(tx); // signal writer to stop
    w.join().expect("join");
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

/// Returns iteration count for a given payload size in bytes.
/// Uses the NOMINAL (un-aligned) payload size so both Zinc and socket
/// benchmarks use consistent iteration counts for the same logical size.
fn pick_iters(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 100_000,
        p if p <= 65_536 => 10_000,
        p if p <= 1_048_576 => 1_000,
        _ => 100,
    }
}
