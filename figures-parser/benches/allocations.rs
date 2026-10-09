//! Heap allocations per call of each hot path over each corpus, counted by
//! `stats_alloc`: `cargo bench --bench allocations`. A count here is exact, so
//! a change that adds an allocation per token shows as such. `allocs` counts
//! allocations and reallocations; `bytes` is the bytes they requested, a
//! reallocation counted by how much it grew.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::print_stdout,
    reason = "a bench prints its table"
)]

mod corpus;

use std::alloc::System;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static ALLOCATOR: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn measure(f: &dyn Fn() -> usize) -> (usize, isize) {
    f();
    let region = Region::new(ALLOCATOR);
    f();
    let stats = region.change();
    (
        stats.allocations + stats.reallocations,
        isize::try_from(stats.bytes_allocated).unwrap_or(isize::MAX) + stats.bytes_reallocated,
    )
}

fn main() {
    println!(
        "{:<12} {:<22} {:>10} {:>12}",
        "corpus", "path", "allocs", "bytes"
    );
    for case in corpus::all() {
        for (path, f) in corpus::paths(&case) {
            let (calls, bytes) = measure(&f);
            println!("{:<12} {path:<22} {calls:>10} {bytes:>12}", case.name);
        }
    }
}
