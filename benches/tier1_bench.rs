use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rishe::Pipeline;

fn bench_pipeline(c: &mut Criterion) {
    let nlp = Pipeline::create(true, true, true);
    let sample = "من در سال 2026 به کتاب ها نگاه مي كنم چون سرعتش بالاست!";
    c.bench_function("end_to_end_process", |b| {
        b.iter(|| black_box(nlp.process(black_box(sample))))
    });
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
