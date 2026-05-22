use std::hint::black_box;
use std::sync::atomic::Ordering;
use std::time::Instant;

use zinc_core::SharedRegion;

fn main() {
    let name = "bench_throughput";
    let size = 100 * 1024 * 1024; // 100 MB

    let region = SharedRegion::create(name, size).expect("create region");
    let ptr = region.as_ptr();

    // Write throughput
    let start = Instant::now();
    let iterations = 10;
    for i in 0..iterations {
        unsafe {
            std::ptr::write_bytes(ptr, (i % 256) as u8, size);
        }
    }
    let elapsed = start.elapsed();
    let total_bytes = size as f64 * iterations as f64;
    let gb_per_sec = total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0;
    println!("Write throughput: {:.2} GB/s ({:.2?} for {} x {}MB)",
             gb_per_sec, elapsed, iterations, size / 1024 / 1024);

    // Read throughput
    let start = Instant::now();
    let mut sum: u64 = 0;
    for _ in 0..iterations {
        for j in 0..(size / 8) {
            unsafe {
                let val = std::ptr::read(ptr.add(j * 8) as *const u64);
                sum = sum.wrapping_add(black_box(val));
            }
        }
    }
    let elapsed = start.elapsed();
    let gb_per_sec = total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0;
    println!("Read throughput:  {:.2} GB/s ({:.2?}) — checksum: {:x}",
             gb_per_sec, elapsed, sum);

    // Notify/wait latency
    let region2 = SharedRegion::open(name).expect("open region");
    let iterations = 10_000u64;
    let start = Instant::now();
    for i in 0..iterations {
        unsafe {
            std::ptr::write(region.as_ptr() as *mut u64, i);
        }
        region.notify();
        region2.wait(1000).expect("wait");
        let val = unsafe { std::ptr::read(region2.as_ptr() as *const u64) };
        black_box(val);
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    println!("\nNotify/wait roundtrip: {:.0} ns avg ({:.2?} for {} iterations)",
             avg_ns, elapsed, iterations);

    drop(region2);
    drop(region);
}
