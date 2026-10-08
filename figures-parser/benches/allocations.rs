//! Heap allocations per call of each hot path over each corpus, counted by a
//! wrapping allocator: `cargo bench --bench allocations`. A number here is
//! exact, so a change that adds an allocation per token shows as such.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::print_stdout,
    reason = "a bench prints its table"
)]

mod corpus;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static CALLS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded to the system allocator unchanged; the
// counters are the only addition and touch no allocator state.
#[allow(unsafe_code, reason = "a global allocator is an unsafe trait")]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: the layout is the caller's, forwarded as is.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn measure(f: &dyn Fn() -> usize) -> (usize, usize) {
    f();
    let calls = CALLS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    f();
    (
        CALLS.load(Ordering::Relaxed) - calls,
        BYTES.load(Ordering::Relaxed) - bytes,
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
