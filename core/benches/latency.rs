use criterion::{criterion_group, criterion_main, Criterion};
use std::sync::atomic::{AtomicBool, Ordering};
use zinc_core::SharedRegion;

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn bench_create_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("create_open");
    let create_name = format!("bc_{}", std::process::id());
    group.bench_function("create_one_page", |b| {
        b.iter(|| {
            let region = SharedRegion::create(&create_name, page_size()).unwrap();
            std::hint::black_box(&region);
        });
    });
    let open_name = format!("bo_{}", std::process::id());
    let _owner = SharedRegion::create(&open_name, page_size()).unwrap();
    group.bench_function("open_one_page", |b| {
        b.iter(|| std::hint::black_box(SharedRegion::open(&open_name).unwrap()));
    });
    group.finish();
}

fn bench_notify_wait_latency(c: &mut Criterion) {
    let request_name = format!("breq_{}", std::process::id());
    let reply_name = format!("brep_{}", std::process::id());
    let request = SharedRegion::create(&request_name, page_size()).unwrap();
    let reply = SharedRegion::create(&reply_name, page_size()).unwrap();
    let worker_request = SharedRegion::open(&request_name).unwrap();
    let worker_reply = SharedRegion::open(&reply_name).unwrap();
    let done = AtomicBool::new(false);

    std::thread::scope(|scope| {
        scope.spawn(|| loop {
            worker_request.wait(5000).unwrap();
            if done.load(Ordering::Relaxed) {
                break;
            }
            worker_reply.notify();
        });
        c.bench_function("latency/thread_ping_pong", |b| {
            b.iter(|| {
                request.notify();
                reply.wait(5000).unwrap();
            });
        });
        done.store(true, Ordering::Relaxed);
        request.notify();
    });
}

criterion_group!(benches, bench_create_open, bench_notify_wait_latency);
criterion_main!(benches);
