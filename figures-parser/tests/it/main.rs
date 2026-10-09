//! The library's integration tests, in one binary.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_docs_in_private_items,
    reason = "a test asserts with unwrap and reads by index"
)]

mod digits;
mod ground;
mod locales;
mod readings;
mod scan;
mod urls;
mod utf16;
