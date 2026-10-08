//! Timing of the hot paths over each corpus: `cargo bench --bench figures`.

#![allow(clippy::missing_docs_in_private_items, reason = "a bench")]

mod corpus;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn hot_paths(c: &mut Criterion) {
    for case in corpus::all() {
        let mut group = c.benchmark_group(case.name);
        group.throughput(Throughput::Bytes(
            u64::try_from(case.text.len()).unwrap_or(u64::MAX),
        ));
        for (path, f) in corpus::paths(&case) {
            group.bench_function(path, |b| b.iter(&f));
        }
        group.finish();
    }
}

criterion_group!(benches, hot_paths);
criterion_main!(benches);
