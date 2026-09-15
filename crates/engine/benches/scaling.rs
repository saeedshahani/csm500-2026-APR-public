mod support;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use engine::Execution;
use std::time::Duration;
fn scaling(c: &mut Criterion) {
    for (family, n) in [
        ("atoms", 16),
        ("atoms", 1024),
        ("rules", 8),
        ("pattern", 4),
        ("branch", 1),
        ("branch", 32),
        ("lambda", 1),
        ("reflective", 1),
    ] {
        let graph = support::fixture(family, n);
        support::check(&graph, family, n);
        let execution = Execution::new(graph.clone(), Default::default()).unwrap();
        c.bench_function(&format!("{family}-{n}/validate_decode"), |b| {
            b.iter(|| black_box(graph_core::validate(black_box(&graph)).unwrap()))
        });
        c.bench_function(&format!("{family}-{n}/enabled"), |b| {
            b.iter(|| black_box(execution.enabled().unwrap()))
        });
        if !execution.enabled().unwrap().steps().is_empty() {
            c.bench_function(&format!("{family}-{n}/commit"), |b| {
                b.iter_batched_ref(
                    || {
                        let e = Execution::new(graph.clone(), Default::default()).unwrap();
                        let step = e.enabled().unwrap().into_steps().remove(0);
                        (e, Some(step))
                    },
                    |(e, step)| black_box(e.commit(step.take().unwrap()).unwrap()),
                    BatchSize::SmallInput,
                )
            });
        }
    }
}
criterion_group! {name=benches; config=Criterion::default().warm_up_time(Duration::from_millis(50)).measurement_time(Duration::from_millis(150)).sample_size(10); targets=scaling}
criterion_main!(benches);
