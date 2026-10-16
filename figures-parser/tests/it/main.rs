//! The library's integration tests, in one binary.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_docs_in_private_items,
    reason = "a test asserts with unwrap and reads by index"
)]

mod approximate;
mod arithmetic;
mod dates;
mod digits;
mod durations;
mod emails;
mod ground;
mod ids;
mod locales;
mod panic_free;
mod ratios;
mod readings;
mod scales;
mod scan;
mod signs;
mod spelled;
mod urls;
mod utf16;
