mod common;

use std::time::Instant;

use redis::Commands;

use zinc_core::SharedRegion;

const NAME: &str = "zr_bench";

struct BenchResult {
    gbps: f64,
    total_gb: f64,
}

fn main() {
    let mut conn =
        match redis::Client::open("redis://127.0.0.1:6379/").and_then(|c| c.get_connection()) {
            Ok(conn) => conn,
            Err(_) => {
                println!("Redis not available on localhost:6379. Start Redis and retry.");
                return;
            }
        };
    run_benchmarks(&mut conn);
}

// ── Helpers ─────────────────────────────────────────────────────

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

fn pick_iters_redis(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 5_000,
        p if p <= 65_536 => 500,
        p if p <= 1_048_576 => 50,
        _ => 10,
    }
}

fn warmup_iters_redis(payload: usize) -> usize {
    (pick_iters_redis(payload) / 5).max(5)
}

fn bench_redis_transfer(conn: &mut redis::Connection, payload: usize) -> BenchResult {
    let iters = pick_iters_redis(payload);
    let warmup = warmup_iters_redis(payload);
    let mut data = vec![0u8; payload];

    for i in 0..warmup {
        data.fill(i as u8);
        let _: () = conn
            .set("bench_key", data.as_slice())
            .expect("redis warmup set");
        let _got: Vec<u8> = conn.get("bench_key").expect("redis warmup get");
    }

    let start = Instant::now();
    for i in 0..iters {
        data.fill(i as u8);
        let _: () = conn.set("bench_key", data.as_slice()).expect("redis set");
        let got: Vec<u8> = conn.get("bench_key").expect("redis get");
        assert_eq!(got.len(), payload, "Redis returned wrong size");
        assert_eq!(got[0], data[0], "Redis returned wrong data");
        std::hint::black_box(got[0]);
    }

    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    BenchResult {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

// ── Zinc helpers ────────────────────────────────────────────────

fn cleanup(name: &str) {
    let cname = std::ffi::CString::new(format!("/zinc_{name}")).ok();
    if let Some(cn) = cname {
        unsafe {
            libc::shm_unlink(cn.as_ptr());
        }
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

fn bench_zinc_transfer(nominal: usize, aligned: usize) -> BenchResult {
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

    BenchResult {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

// ── Orchestrator ────────────────────────────────────────────────

fn run_benchmarks(conn: &mut redis::Connection) {
    let (_latency_us, _latency_iters) = common::notification_roundtrip();

    let sizes: &[usize] = &[1, 64, 1024, 10240, 102400];
    let mut rows: Vec<(usize, BenchResult, BenchResult)> = Vec::new();

    const SAMPLES: usize = 5;

    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);

        let mut zinc_best = 0.0_f64;
        let mut redis_best = 0.0_f64;
        let mut zinc_data = 0.0_f64;
        let mut redis_data = 0.0_f64;

        for s in 0..SAMPLES {
            let (z, r) = if s % 2 == 0 {
                (
                    bench_zinc_transfer(nominal, aligned),
                    bench_redis_transfer(conn, nominal),
                )
            } else {
                let r = bench_redis_transfer(conn, nominal);
                let z = bench_zinc_transfer(nominal, aligned);
                (z, r)
            };
            if z.gbps > zinc_best {
                zinc_best = z.gbps;
                zinc_data = z.total_gb;
            }
            if r.gbps > redis_best {
                redis_best = r.gbps;
                redis_data = r.total_gb;
            }
        }

        rows.push((
            payload_kb,
            BenchResult {
                gbps: zinc_best,
                total_gb: zinc_data,
            },
            BenchResult {
                gbps: redis_best,
                total_gb: redis_data,
            },
        ));
    }

    const GRN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const RST: &str = "\x1b[0m";

    println!(
        "\n\n{}══════════════════════════════════════════════════════",
        GRN
    );
    println!("           Zinc vs Redis \u{2014} Throughput");
    println!(
        "{0}══════════════════════════════════════════════════════{1}",
        GRN, RST
    );

    const H: &str = "\u{2500}";
    let c = [
        H.repeat(10),
        H.repeat(17),
        H.repeat(17),
        H.repeat(8),
        H.repeat(13),
    ];
    println!(
        "\u{250c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{2510}",
        c[0], c[1], c[2], c[3], c[4]
    );
    println!(
        "\u{2502} {:<8} \u{2502} {:>15} \u{2502} {:>15} \u{2502} {:>6} \u{2502} {:>11} \u{2502}",
        "Payload", "Zinc", "Redis", "Ratio", "Data"
    );
    println!(
        "\u{251c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{2524}",
        c[0], c[1], c[2], c[3], c[4]
    );

    for (kb, z, r) in &rows {
        let label = fmt_size(*kb);
        let ratio = z.gbps / r.gbps;
        let total = (z.total_gb + r.total_gb) / 2.0;

        let (z_color, r_color) = if (0.98..=1.02).contains(&ratio) {
            (GRN, GRN)
        } else if ratio >= 1.0 {
            (GRN, RED)
        } else {
            (RED, GRN)
        };

        println!(
            "\u{2502} {:<8} \u{2502} {}{:>10.2} GB/s{} \u{2502} {}{:>10.2} GB/s{} \u{2502} {:>5.0}x \u{2502} {:>8.2} GB \u{2502}",
            label, z_color, z.gbps, RST, r_color, r.gbps, RST, ratio, total,
        );
    }

    println!(
        "\u{2514}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2518}",
        c[0], c[1], c[2], c[3], c[4]
    );
    println!(
        "{}Zinc: memory-bandwidth-bound. Redis: network-stack-bound (~1–5 GB/s localhost).{}",
        GRN, RST
    );
    println!(
        "{}Zero kernel data copies vs. Redis full TCP stack even on loopback.{}",
        GRN, RST
    );
    println!(
        "{}Method: min-time (max GB/s) across {} samples, alternating order.{}",
        RST, SAMPLES, RST
    );
}
