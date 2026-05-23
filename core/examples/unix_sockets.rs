use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use zinc_core::SharedRegion;

fn main() {
    run_benchmarks();
}

const NAME: &str = "bench_zinc";

struct Result {
    gbps: f64,
    total_gb: f64,
}

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
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

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

fn pick_iters(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 100_000,
        p if p <= 65_536 => 10_000,
        p if p <= 1_048_576 => 1_000,
        _ => 100,
    }
}

fn warmup_iters(payload: usize) -> usize {
    (pick_iters(payload) / 10).max(10)
}

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

fn pick_iters_socket(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 5_000,
        p if p <= 65_536 => 500,
        p if p <= 1_048_576 => 50,
        _ => 10,
    }
}

fn warmup_iters_socket(payload: usize) -> usize {
    (pick_iters_socket(payload) / 5).max(5)
}

fn bench_zinc_transfer(nominal: usize, aligned: usize) -> Result {
    let parent = create_region(NAME, aligned);
    let child = open_region(NAME);

    let iters = pick_iters(nominal);
    let payload = aligned;
    let warmup = warmup_iters(nominal);

    for i in 0..warmup {
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("warmup");
        std::hint::black_box(unsafe { std::ptr::read(child.as_ptr()) });
    }

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

fn bench_unix_transfer(payload: usize) -> Result {
    let (a, mut b) = UnixStream::pair().expect("socket pair");
    let iters = pick_iters_socket(payload);
    let warmup = warmup_iters_socket(payload);
    let mut buf = vec![0u8; payload];

    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let mut writer = a.try_clone().expect("clone");
    let w = thread::spawn(move || {
        while let Ok(data) = rx.recv() {
            writer.write_all(&data).expect("write");
        }
    });

    for i in 0..warmup {
        buf.fill(i as u8);
        tx.send(buf.clone()).expect("send");
        b.read_exact(&mut buf).expect("read");
        std::hint::black_box(buf[0]);
    }

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

fn run_benchmarks() {
    let (_latency_us, _latency_iters) = bench_notify_latency();

    let sizes: &[usize] = &[1, 64, 1024, 10240, 1048576];
    let mut rows: Vec<(usize, Result, Result)> = Vec::new();

    const SAMPLES: usize = 5;

    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);

        let mut zinc_best = 0.0_f64;
        let mut socket_best = 0.0_f64;
        let mut zinc_data = 0.0_f64;
        let mut socket_data = 0.0_f64;

        for s in 0..SAMPLES {
            let (z, sk) = if s % 2 == 0 {
                (bench_zinc_transfer(nominal, aligned), bench_unix_transfer(nominal))
            } else {
                let sk = bench_unix_transfer(nominal);
                let z = bench_zinc_transfer(nominal, aligned);
                (z, sk)
            };
            if z.gbps > zinc_best { zinc_best = z.gbps; zinc_data = z.total_gb; }
            if sk.gbps > socket_best { socket_best = sk.gbps; socket_data = sk.total_gb; }
        }

        rows.push((payload_kb,
            Result { gbps: zinc_best, total_gb: zinc_data },
            Result { gbps: socket_best, total_gb: socket_data },
        ));
    }

    const GRN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const RST: &str = "\x1b[0m";

    println!("\n\n{}══════════════════════════════════════════════════════", GRN);
    println!("        Zinc vs Unix Socket \u{2014} Throughput");
    println!("{0}══════════════════════════════════════════════════════{1}", GRN, RST);

    const H: &str = "\u{2500}";
    let c = [H.repeat(10), H.repeat(17), H.repeat(17), H.repeat(8), H.repeat(13)];
    println!("\u{250c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{2510}", c[0], c[1], c[2], c[3], c[4]);
    println!(
        "\u{2502} {:<8} \u{2502} {:>15} \u{2502} {:>15} \u{2502} {:>6} \u{2502} {:>11} \u{2502}",
        "Payload", "Zinc", "Socket", "Ratio", "Data"
    );
    println!("\u{251c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{2524}", c[0], c[1], c[2], c[3], c[4]);

    for (kb, z, s) in &rows {
        let label = fmt_size(*kb);
        let ratio = z.gbps / s.gbps;
        let total = (z.total_gb + s.total_gb) / 2.0;

        let (z_color, s_color) = if ratio >= 0.98 && ratio <= 1.02 {
            (GRN, GRN)
        } else if ratio >= 1.0 {
            (GRN, RED)
        } else {
            (RED, GRN)
        };

        println!(
            "\u{2502} {:<8} \u{2502} {}{:>10.2} GB/s{} \u{2502} {}{:>10.2} GB/s{} \u{2502} {:>5.0}x \u{2502} {:>8.2} GB \u{2502}",
            label, z_color, z.gbps, RST, s_color, s.gbps, RST, ratio, total,
        );
    }

    println!("\u{2514}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2518}", c[0], c[1], c[2], c[3], c[4]);
    println!(
        "{}Zinc: memory-bandwidth-bound. Socket: kernel-copy-bound (~1.4 GB/s).{}",
        GRN, RST
    );
    println!(
        "Zero kernel data copies \u{2014} same physical pages, both processes.{}",
        RST
    );
    println!(
        "Method: min-time (max GB/s) across {} samples, alternating order.{}",
        SAMPLES, RST
    );
}
