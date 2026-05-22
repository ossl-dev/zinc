use std::hint::black_box;
use std::time::Instant;

use zinc_core::SharedRegion;

const SIZE: usize = 1024 * 1024 * 1024; // 1 GB

fn main() {
    let name = "bench_throughput";

    // ── Baseline: heap-allocated buffer ──
    println!("=== Baseline: heap-allocated Vec<u8> (no Zinc) ===");
    let mut heap = vec![0u8; SIZE];
    let ptr = heap.as_mut_ptr();
    bench_throughput(ptr);

    // ── Zinc: shared memory region ──
    println!("\n=== Zinc: shared memory region ===");
    let region = SharedRegion::create(name, SIZE).expect("create region");
    bench_throughput(region.as_ptr());

    // ── Notify/wait latency ──
    let region2 = SharedRegion::open(name).expect("open region");
    let iters = 10_000u64;
    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write(region.as_ptr() as *mut u64, i) }
        region.notify();
        region2.wait(1000).expect("wait");
        black_box(unsafe { std::ptr::read(region2.as_ptr() as *const u64) });
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() as f64 / iters as f64;
    println!(
        "\n=== Notify/wait latency ==="
    );
    println!(
        "roundtrip: {:.0} ns avg ({:.2?} for {} iterations)",
        avg_ns, elapsed, iters
    );

    drop(region2);
    drop(region);
}

fn bench_throughput(base: *mut u8) {
    let iters = 4;

    // Write
    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write_bytes(base, (i % 256) as u8, SIZE) }
    }
    let elapsed = start.elapsed();
    let total = SIZE as f64 * iters as f64;
    println!(
        "write: {:.2} GB/s ({:.2?} for {} x 1 GB)",
        total / elapsed.as_secs_f64() / 1_000_000_000.0,
        elapsed,
        iters,
    );

    // Read
    let mut sum: u64 = 0;
    let start = Instant::now();
    for _ in 0..iters {
        for j in 0..(SIZE / 8) {
            sum = sum.wrapping_add(black_box(unsafe {
                std::ptr::read(base.add(j * 8) as *const u64)
            }));
        }
    }
    let elapsed = start.elapsed();
    let total = SIZE as f64 * iters as f64;
    println!(
        "read:  {:.2} GB/s ({:.2?}) \u{2014} checksum: {:x}",
        total / elapsed.as_secs_f64() / 1_000_000_000.0,
        elapsed,
        sum,
    );
}
