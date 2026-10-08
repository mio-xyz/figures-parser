//! Timing of the three hot paths over each corpus: `cargo bench --bench figures`.

#![allow(clippy::missing_docs_in_private_items, reason = "a bench")]

mod corpus;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use figures_parser::number::scan::tokens;
use figures_parser::{extract_numbers, ungrounded_figures};
use std::hint::black_box;

fn hot_paths(c: &mut Criterion) {
    for corpus::Case {
        name,
        text,
        grounded,
    } in corpus::all()
    {
        let mut group = c.benchmark_group(name);
        group.throughput(Throughput::Bytes(
            u64::try_from(text.len()).unwrap_or(u64::MAX),
        ));
        group.bench_function(BenchmarkId::new("tokens", corpus::SIZE), |b| {
            b.iter(|| tokens(black_box(&text)).count());
        });
        group.bench_function(BenchmarkId::new("extract_numbers", corpus::SIZE), |b| {
            b.iter(|| extract_numbers(black_box(&text)).len());
        });
        group.bench_function(BenchmarkId::new("ungrounded/grounded", corpus::SIZE), |b| {
            b.iter(|| ungrounded_figures(grounded, black_box(&text)).len());
        });
        group.bench_function(
            BenchmarkId::new("ungrounded/ungrounded", corpus::SIZE),
            |b| {
                b.iter(|| ungrounded_figures(corpus::UNGROUNDED_REPLY, black_box(&text)).len());
            },
        );
        group.finish();
    }
}

criterion_group!(benches, hot_paths);
criterion_main!(benches);
