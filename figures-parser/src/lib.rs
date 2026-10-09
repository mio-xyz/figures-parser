//! Reads the figures and links a text states, and decides which of a reply's
//! the turn it answers does not carry.

#![forbid(unsafe_code)]

mod kind;
pub mod number;
mod text;
mod url;
pub mod utf16;

pub use kind::{Kind, UnknownKind};
pub use number::{Figure, Number, Reading, extract_numbers, numbers, ungrounded_figures};
pub use url::{Url, fabricated_urls, ungrounded_urls, urls};
