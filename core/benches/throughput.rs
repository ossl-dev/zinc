use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use zinc_core::SharedRegion;

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn bench_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("write");
    let sizes = [64, 1024, 16384, 65536, 1048576];

    for size in sizes {
        let aligned = if size < page_size() {
            page_size()
        } else {
            size
        };
        group.throughput(Throughput::Bytes(size as u64));

        let name = format!("bench_throughput_{size}");
        let region = SharedRegion::create(&name, aligned).expect("create");

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &s| {
            b.iter(|| {
                unsafe {
                    std::ptr::write_bytes(region.as_ptr(), 0xAB, s);
                }
                std::hint::black_box(unsafe { std::ptr::read(region.as_ptr()) });
            })
        });
    }
    group.finish();
}

fn bench_notification_fast_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("notification_fast_path");

    let region = SharedRegion::create("bench_notify_wait", page_size()).expect("create");

    group.bench_function("same_thread", |b| {
        b.iter(|| {
            region.notify();
            region.wait(5000).expect("wait");
            std::hint::black_box(unsafe { std::ptr::read(region.as_ptr()) });
        })
    });

    group.finish();
}

criterion_group!(benches, bench_write, bench_notification_fast_path);
criterion_main!(benches);
