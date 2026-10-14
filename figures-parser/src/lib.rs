//! Check a piece of generated text against the sources it was written from.
//!
//! The crate reads the numbers, links, dated weekdays, email addresses,
//! identifiers and durations a text states. Each kind has a reader, which
//! returns what a text writes with its byte span, and a comparison, which
//! takes a reply, the corpus it was written from and, where it matters, the
//! prompt, and returns the reply's items that no source carries, once each,
//! as written.
//!
//! | Kind | Reader | Comparison |
//! |---|---|---|
//! | Numbers | [`numbers`], [`extract_numbers`] | [`ungrounded_figures`], [`ungrounded_percentages`], [`prose_arithmetic`] |
//! | Links | [`urls`] | [`ungrounded_urls`], [`fabricated_urls`] |
//! | Weekdays beside dates | | [`misdated_weekdays`] |
//! | Email addresses | [`emails`] | [`ungrounded_emails`] |
//! | Identifiers | [`identifiers`] | [`ungrounded_identifiers`], [`unseen_tokens`] |
//! | Durations | [`durations`] | [`ungrounded_durations`] |
//!
//! # Example
//!
//! ```
//! use figures_parser::{Date, misdated_weekdays, ungrounded_figures, ungrounded_identifiers};
//!
//! // What the model read, and what it wrote.
//! let corpus = r#"{"total_amount": 48120.0, "count": 214, "issue": "CORE-1710"}"#;
//! let reply = "Q3 closed at $48,200 across 214 deals; follow-up in CORE-1711.";
//!
//! assert_eq!(ungrounded_figures(reply, corpus), ["48,200"]);
//! assert_eq!(ungrounded_identifiers(reply, corpus, ""), ["CORE-1711"]);
//!
//! let today: Date = "2026-10-09".parse().unwrap();
//! assert_eq!(
//!     misdated_weekdays("The call is Fri 26 Sep.", today),
//!     [r#""Fri 26 Sep" (2026-09-26 is a Saturday)"#]
//! );
//! ```
//!
//! Offsets are bytes into the text given: `&text[span]` is the item as
//! written. [`utf16::Utf16Cursor`] converts them for a JavaScript caller.
//!
//! The crate has no `unsafe`, and its functions do not panic: every Unicode
//! code point and hundreds of thousands of generated inputs run through each
//! entry point in a build that also panics on integer overflow. A comparison
//! allocates what it keeps from the reply, never per match of the corpus.

#![forbid(unsafe_code)]

pub mod date;
pub mod duration;
mod email;
pub mod id;
mod kind;
pub mod number;
mod text;
mod url;
pub mod utf16;

pub use date::misdated_weekdays;
pub use duration::{Duration, Length, durations, ungrounded_durations};
pub use email::{Email, emails, ungrounded_emails};
pub use id::{Identifier, identifiers, ungrounded_identifiers, unseen_tokens};
pub use jiff::civil::Date;
pub use kind::{Kind, UnknownKind};
pub use number::{
    Figure, Number, Reading, Sign, extract_numbers, numbers, prose_arithmetic, ungrounded_figures,
    ungrounded_percentages,
};
pub use url::{Url, fabricated_urls, ungrounded_urls, urls};

/// The README's examples, run as doc tests so they stay true.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
