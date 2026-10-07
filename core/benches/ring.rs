use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use zinc_core::ring::RingStorage;

fn ring_benchmarks(c: &mut Criterion) {
    let storage = RingStorage::default();
    let ring = storage.ring();
    c.bench_function("ring/push_pop", |b| {
        b.iter(|| {
            ring.push(std::hint::black_box(42)).unwrap();
            std::hint::black_box(ring.pop());
        });
    });

    let mut group = c.benchmark_group("ring/contention");
    group.sample_size(10);
    group.throughput(Throughput::Elements(4096));
    for producers in [1, 2, 4] {
        group.bench_with_input(
            BenchmarkId::from_parameter(producers),
            &producers,
            |b, &n| {
                b.iter(|| {
                    std::thread::scope(|scope| {
                        for _ in 0..n {
                            scope.spawn(|| {
                                for value in 0..(4096 / n) {
                                    while !ring.try_push(value) {
                                        std::hint::spin_loop();
                                    }
                                }
                            });
                        }
                        for _ in 0..4096 {
                            loop {
                                if let Some(value) = ring.pop() {
                                    std::hint::black_box(value);
                                    break;
                                }
                                std::hint::spin_loop();
                            }
                        }
                    });
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, ring_benchmarks);
criterion_main!(benches);
