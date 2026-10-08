//! Reads the figures a text states and decides which of a reply's figures
//! the turn it answers does not carry.

#![forbid(unsafe_code)]

pub mod ground;
pub mod kind;
pub mod number;
pub mod utf16;

pub use ground::ungrounded_figures;
pub use kind::Kind;
pub use number::{Number, extract_numbers};
