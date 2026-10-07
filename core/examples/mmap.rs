mod common;

use std::ffi::CString;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use zinc_core::SharedRegion;

fn main() {
    run_benchmarks();
}

const ZINC_NAME: &str = "bench_zinc_mmap";

const MMAP_NAME: &str = "bench_raw_mmap";

struct BenchResult {
    gbps: f64,
    total_gb: f64,
}

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn page_align(size: usize) -> usize {
    let page = page_size();
    (size + page - 1) & !(page - 1)
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

fn zinc_cleanup() {
    if let Ok(cn) = CString::new(format!("/zinc_{ZINC_NAME}")) {
        unsafe { libc::shm_unlink(cn.as_ptr()) };
    }
}

fn mmap_cleanup() {
    if let Ok(cn) = CString::new(format!("/mmap_{MMAP_NAME}")) {
        unsafe { libc::shm_unlink(cn.as_ptr()) };
    }
}

// ── Raw mmap wrapper ──────────────────────────────────────────────

struct MmapRegion {
    base: *mut u8,
    map_len: usize,
    data_off: usize,
    owner: bool,
    last_seq: AtomicU32,
    name: CString,
}

impl MmapRegion {
    fn create(name: &str, data_size: usize) -> Self {
        let cname = CString::new(format!("/mmap_{name}")).expect("valid name");
        let page = page_size();
        let data_off = page;
        let total = data_off + page_align(data_size);

        unsafe { libc::shm_unlink(cname.as_ptr()) };

        let fd = unsafe {
            libc::shm_open(
                cname.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                (libc::S_IRUSR | libc::S_IWUSR) as libc::c_uint,
            )
        };
        assert!(fd >= 0, "shm_open(O_CREAT) failed");

        let ret = unsafe { libc::ftruncate(fd, total as libc::off_t) };
        assert!(ret == 0, "ftruncate failed");

        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        assert!(base != libc::MAP_FAILED, "mmap failed");

        unsafe { libc::close(fd) };
        unsafe { (base as *mut AtomicU32).write(AtomicU32::new(0)) };

        MmapRegion {
            base: base as *mut u8,
            map_len: total,
            data_off,
            owner: true,
            last_seq: AtomicU32::new(0),
            name: cname,
        }
    }

    fn open(name: &str) -> Self {
        let cname = CString::new(format!("/mmap_{name}")).expect("valid name");
        let data_off = page_size();

        let fd = unsafe { libc::shm_open(cname.as_ptr(), libc::O_RDWR, 0) };
        assert!(fd >= 0, "shm_open(O_RDWR) failed");

        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::fstat(fd, &mut st) };
        assert!(ret == 0, "fstat failed");
        let total = st.st_size as usize;

        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        assert!(base != libc::MAP_FAILED, "mmap failed");

        unsafe { libc::close(fd) };

        MmapRegion {
            base: base as *mut u8,
            map_len: total,
            data_off,
            owner: false,
            last_seq: AtomicU32::new(0),
            name: cname,
        }
    }

    fn data_ptr(&self) -> *mut u8 {
        unsafe { self.base.add(self.data_off) }
    }

    fn notify_seq(&self) -> &AtomicU32 {
        unsafe { &*(self.base as *const AtomicU32) }
    }

    fn notify(&self) {
        zinc_core::notify(self.notify_seq());
    }

    fn wait(&self, timeout_ms: u32) -> zinc_core::Result<()> {
        let seq = self.notify_seq();
        let last = self.last_seq.load(Ordering::Relaxed);
        zinc_core::wait(seq, last, timeout_ms)?;
        self.last_seq
            .store(seq.load(Ordering::Acquire), Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for MmapRegion {
    fn drop(&mut self) {
        if self.map_len > 0 {
            unsafe { libc::munmap(self.base as *mut libc::c_void, self.map_len) };
        }
        if self.owner {
            unsafe { libc::shm_unlink(self.name.as_ptr()) };
        }
    }
}

// ── Notify/wait latency ────────────────────────────────────────

// ── Benchmark runners ──────────────────────────────────────────

fn bench_zinc_transfer(nominal: usize, aligned: usize) -> BenchResult {
    zinc_cleanup();
    let parent = SharedRegion::create(ZINC_NAME, aligned).expect("create parent");
    let child = SharedRegion::open(ZINC_NAME).expect("open child");

    let iters = pick_iters(nominal);
    let payload = aligned;
    let warmup = warmup_iters(nominal);

    // Warmup: fault in pages, warm caches.
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

fn bench_mmap_transfer(nominal: usize, aligned: usize) -> BenchResult {
    mmap_cleanup();
    let parent = MmapRegion::create(MMAP_NAME, aligned);
    let child = MmapRegion::open(MMAP_NAME);

    let iters = pick_iters(nominal);
    let payload = aligned;
    let warmup = warmup_iters(nominal);

    for i in 0..warmup {
        unsafe { std::ptr::write_bytes(parent.data_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("warmup");
        std::hint::black_box(unsafe { std::ptr::read(child.data_ptr()) });
    }

    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write_bytes(parent.data_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("wait");
        std::hint::black_box(unsafe { std::ptr::read(child.data_ptr()) });
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

fn run_benchmarks() {
    let (latency_us, latency_iters) = common::notification_roundtrip();

    let sizes: &[usize] = &[1, 64, 1024, 10240, 1048576];
    let mut rows: Vec<(usize, BenchResult, BenchResult)> = Vec::new();

    const SAMPLES: usize = 5;

    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);

        let mut zinc_best = 0.0_f64;
        let mut mmap_best = 0.0_f64;
        let mut zinc_data = 0.0_f64;
        let mut mmap_data = 0.0_f64;

        for s in 0..SAMPLES {
            // Alternate order to cancel first-run bias.
            let (z, m) = if s % 2 == 0 {
                (
                    bench_zinc_transfer(nominal, aligned),
                    bench_mmap_transfer(nominal, aligned),
                )
            } else {
                let m = bench_mmap_transfer(nominal, aligned);
                let z = bench_zinc_transfer(nominal, aligned);
                (z, m)
            };
            if z.gbps > zinc_best {
                zinc_best = z.gbps;
                zinc_data = z.total_gb;
            }
            if m.gbps > mmap_best {
                mmap_best = m.gbps;
                mmap_data = m.total_gb;
            }
        }

        rows.push((
            payload_kb,
            BenchResult {
                gbps: zinc_best,
                total_gb: zinc_data,
            },
            BenchResult {
                gbps: mmap_best,
                total_gb: mmap_data,
            },
        ));
    }

    const GRN: &str = "\x1b[32m";
    const YLW: &str = "\x1b[33m";
    const RST: &str = "\x1b[0m";
    const BLD: &str = "\x1b[1m";

    println!(
        "\n\n{}══════════════════════════════════════════════════════{}",
        BLD, RST
    );
    println!(
        "{}          Zinc vs Raw mmap \u{2014} Throughput{}",
        BLD, RST
    );
    println!(
        "{}  Notify/wait thread roundtrip: {:.1} \u{00b5}s avg ({} iters){}",
        BLD, latency_us, latency_iters, RST
    );
    println!(
        "{}══════════════════════════════════════════════════════{}",
        BLD, RST
    );
    const H: &str = "\u{2500}";
    let c = [
        H.repeat(10),
        H.repeat(17),
        H.repeat(17),
        H.repeat(10),
        H.repeat(13),
    ];
    println!(
        "\u{250c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{252c}{}\u{2510}",
        c[0], c[1], c[2], c[3], c[4]
    );
    println!(
        "\u{2502} {:<8} \u{2502} {:>15} \u{2502} {:>15} \u{2502} {:>8} \u{2502} {:>11} \u{2502}",
        "Payload", "Zinc", "Mmap", "Ratio", "Data"
    );
    println!(
        "\u{251c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{253c}{}\u{2524}",
        c[0], c[1], c[2], c[3], c[4]
    );

    for (kb, z, m) in &rows {
        let label = fmt_size(*kb);
        let ratio = z.gbps / m.gbps;
        let total = (z.total_gb + m.total_gb) / 2.0;

        let (z_color, m_color) = if (0.98..=1.02).contains(&ratio) {
            (GRN, GRN)
        } else {
            (YLW, YLW)
        };

        println!(
            "\u{2502} {:<8} \u{2502} {}{:>10.2} GB/s{} \u{2502} {}{:>10.2} GB/s{} \u{2502} {:>7.2}x \u{2502} {:>8.2} GB \u{2502}",
            label, z_color, z.gbps, RST, m_color, m.gbps, RST, ratio, total,
        );
    }

    println!(
        "\u{2514}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2534}{}\u{2518}",
        c[0], c[1], c[2], c[3], c[4]
    );
    println!(
        "{}Both paths use POSIX shared memory and the same notification functions.{}",
        GRN, RST
    );
    println!(
        "{}Ratios depend on scheduling, cache state, and bookkeeping costs.{}",
        RST, RST
    );
    println!(
        "{}Zinc: auto lifecycle. Mmap: manual shm_open/ftruncate/mmap/unlink.{}",
        GRN, RST
    );
    println!(
        "{}Method: min-time (max GB/s) across {} samples, alternating order.{}",
        RST, SAMPLES, RST
    );
}
