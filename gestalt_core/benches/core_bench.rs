use criterion::{black_box, criterion_group, criterion_main, Criterion};
// Import what you need from gestalt_core

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| black_box(20)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
