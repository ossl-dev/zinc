use criterion::{criterion_group, criterion_main, Criterion};
use std::thread;
use zinc_core::SharedRegion;

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn bench_create_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("create_open");

    group.bench_function("create_4k", |b| {
        b.iter(|| {
            let r = SharedRegion::create("bench_create", page_size()).expect("create");
            std::hint::black_box(&r);
        })
    });

    // Pre-create one region so open() always finds it
    let _keep = SharedRegion::create("bench_open_target", page_size()).expect("create");

    group.bench_function("open_4k", |b| {
        b.iter(|| {
            let r = SharedRegion::open("bench_open_target").expect("open");
            std::hint::black_box(&r);
        })
    });

    group.finish();
}

fn bench_notify_wait_latency(c: &mut Criterion) {
    let region = SharedRegion::create("bench_latency", page_size()).expect("create");
    let child = SharedRegion::open("bench_latency").expect("open");

    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_sig = done.clone();

    let handle = thread::spawn(move || {
        while !done_sig.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_micros(1));
            child.notify();
        }
    });

    let mut group = c.benchmark_group("latency");
    group.bench_function("notify_wait_wall_clock", |b| {
        b.iter(|| {
            region.wait(5000).expect("wait");
            std::hint::black_box(unsafe { std::ptr::read(region.as_ptr()) });
        })
    });

    done.store(true, std::sync::atomic::Ordering::Release);
    handle.join().unwrap();
    group.finish();
}

criterion_group!(benches, bench_create_open, bench_notify_wait_latency);
criterion_main!(benches);
