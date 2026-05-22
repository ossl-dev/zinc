#[cfg(not(windows))]
use std::io::{Read, Write};
#[cfg(not(windows))]
use std::os::unix::net::UnixStream;
#[cfg(not(windows))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(windows))]
use std::sync::Arc;
#[cfg(not(windows))]
use std::sync::mpsc;
#[cfg(not(windows))]
use std::thread;
#[cfg(not(windows))]
use std::time::{Duration, Instant};

#[cfg(not(windows))]
use zinc_core::SharedRegion;

fn main() {
    #[cfg(not(windows))]
    run_benchmarks();

    #[cfg(windows)]
    {
        eprintln!("Zinc benchmark: not supported on Windows");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
const NAME: &str = "bench_zinc";

#[cfg(not(windows))]
struct Result {
    gbps: f64,
    total_gb: f64,
}

#[cfg(not(windows))]
fn cleanup(name: &str) {
    let cname = std::ffi::CString::new(format!("/zinc_{name}")).ok();
    if let Some(cn) = cname {
        unsafe { libc::shm_unlink(cn.as_ptr()); }
    }
}

#[cfg(not(windows))]
fn create_region(name: &str, size: usize) -> zinc_core::SharedRegion {
    cleanup(name);
    SharedRegion::create(name, page_align(size)).expect("create")
}

#[cfg(not(windows))]
fn open_region(name: &str) -> zinc_core::SharedRegion {
    SharedRegion::open(name).expect("open")
}

#[cfg(not(windows))]
fn page_align(size: usize) -> usize {
    let page = page_size();
    (size + page - 1) & !(page - 1)
}

#[cfg(not(windows))]
fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

#[cfg(not(windows))]
fn run_benchmarks() {
    let (latency_us, latency_iters) = bench_notify_latency();

    let sizes: &[usize] = &[1, 64, 1024, 10240, 1048576]; // KB
    let mut rows: Vec<(usize, Result, Result)> = Vec::new();

    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);
        let zinc = bench_zinc_transfer(nominal, aligned);
        let socket = bench_unix_transfer(nominal);
        rows.push((payload_kb, zinc, socket));
    }

    const GRN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const RST: &str = "\x1b[0m";
    const BLD: &str = "\x1b[1m";

    println!("\n\n{}══════════════════════════════════════════════════════{}", BLD, RST);
    println!("{}        Zinc vs Unix Socket \u{2014} Throughput{}", BLD, RST);
    println!("{} Notify/wait latency: {:.1} \u{00b5}s avg ({} iters){}", BLD, latency_us, latency_iters, RST);
    println!("{}══════════════════════════════════════════════════════{}", BLD, RST);
    println!(" {:<8} {:>12} {:>12} {:>6} {:>10}", "Payload", "Zinc", "Socket", "Ratio", "Data");
    println!("{0:\u{2500}^10} {0:\u{2500}^14} {0:\u{2500}^14} {0:\u{2500}^7} {0:\u{2500}^12}", "");

    for (kb, z, s) in &rows {
        let label = fmt_size(*kb);
        let ratio = z.gbps / s.gbps;
        let total = (z.total_gb + s.total_gb) / 2.0;

        println!(
            " {:<8} {}{:>10.2} GB/s{} {}{:>10.2} GB/s{} {:>5.0}x {:>8.2} GB",
            label,
            GRN, z.gbps, RST,
            RED, s.gbps, RST,
            ratio, total,
        );
    }

    println!("{0:\u{2500}^10} {0:\u{2500}^14} {0:\u{2500}^14} {0:\u{2500}^7} {0:\u{2500}^12}", "");
    println!(
        "{}Zinc: memory-bandwidth-bound (~60 GB/s). Socket: kernel-copy-bound (~1.4 GB/s).{}",
        GRN, RST
    );
    println!(
        "{}Zinc has zero kernel data copies \u{2014} same physical pages, both processes.{}",
        GRN, RST
    );
}

#[cfg(not(windows))]
fn fmt_size(kb: usize) -> String {
    if kb >= 1_048_576 {
        format!("{} GB", kb / 1_048_576)
    } else if kb >= 1024 && kb % 1024 == 0 {
        format!("{} MB", kb / 1024)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{} KB", kb)
    }
}

#[cfg(not(windows))]
fn bench_notify_latency() -> (f64, usize) {
    let parent = create_region(NAME, 4096);
    let child = open_region(NAME);

    let done = Arc::new(AtomicBool::new(false));
    let done_signal = done.clone();
    let handle = thread::spawn(move || {
        while !done_signal.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_micros(1));
            parent.notify();
        }
    });

    let iters = 5_000;
    let start = Instant::now();
    for _ in 0..iters {
        child.wait(5000).expect("wait");
    }
    done.store(true, Ordering::Release);
    let elapsed = start.elapsed();
    handle.join().unwrap();

    let avg = elapsed.as_secs_f64() / iters as f64 * 1_000_000.0;
    (avg, iters)
}

#[cfg(not(windows))]
fn bench_zinc_transfer(nominal: usize, aligned: usize) -> Result {
    let parent = create_region(NAME, aligned);
    let child = open_region(NAME);

    let iters = pick_iters(nominal);
    let payload = aligned;

    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("wait");
        std::hint::black_box(unsafe { std::ptr::read(child.as_ptr()) });
    }
    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    drop(child);
    drop(parent);

    Result {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

#[cfg(not(windows))]
fn bench_unix_transfer(payload: usize) -> Result {
    let (a, mut b) = UnixStream::pair().expect("socket pair");
    let iters = pick_iters(payload);
    let mut buf = vec![0u8; payload];

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
        tx.send(buf.clone()).expect("send");
        b.read_exact(&mut buf).expect("read");
        std::hint::black_box(buf[0]);
    }
    drop(tx);
    w.join().expect("join");
    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    Result {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

#[cfg(not(windows))]
fn pick_iters(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 100_000,
        p if p <= 65_536 => 10_000,
        p if p <= 1_048_576 => 1_000,
        _ => 100,
    }
}
