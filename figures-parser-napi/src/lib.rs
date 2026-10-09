//! The Node binding over `figures_parser`: `extract`, `ungroundedFigures`,
//! `ungroundedUrls` and `fabricatedUrls`. Every export catches a panic and
//! throws it as an `Error`, since one unwinding into Node aborts the process.

#![allow(
    clippy::needless_pass_by_value,
    reason = "napi passes a JavaScript string by value"
)]

use std::borrow::Cow;

use figures_parser::utf16::{Utf16Cursor, units};
use figures_parser::{Kind, numbers, urls};
use napi::bindgen_prelude::Either;
use napi_derive::napi;

/// A number reading's comparable value.
#[napi(object)]
pub struct NumberValue {
    /// The integer part in ASCII digits, separators dropped.
    pub integer: String,
    /// The fractional digits in ASCII; absent for a whole number.
    pub fraction: Option<String>,
    /// Written with thousands separators.
    pub grouped: bool,
    /// Written beside a currency sign or code.
    pub currency: bool,
    /// A neighbour reading of a thousands run: grounds, never stated.
    pub derived: bool,
}

/// One reading of one number, with where it sits in the input.
#[napi(object)]
pub struct NumberEntity {
    /// The kind of figure.
    #[napi(ts_type = "\"number\"")]
    pub kind: String,
    /// As written; `input.slice(start, end)` gives it back.
    pub text: String,
    /// UTF-16 offset of the first unit.
    pub start: u32,
    /// One past the last UTF-16 unit.
    pub end: u32,
    /// The comparable value.
    pub value: NumberValue,
}

/// One http(s) link, with where it sits in the input.
#[napi(object)]
pub struct UrlEntity {
    /// The kind of figure.
    #[napi(ts_type = "\"url\"")]
    pub kind: String,
    /// As written, without the markup or punctuation around it;
    /// `input.slice(start, end)` gives it back.
    pub text: String,
    /// UTF-16 offset of the first unit.
    pub start: u32,
    /// One past the last UTF-16 unit.
    pub end: u32,
}

/// What `extract` reads.
#[napi(object)]
pub struct ExtractOptions {
    /// The kinds to read, by name; every kind when absent. An unknown name is an error.
    pub kinds: Option<Vec<String>>,
}

/// The UTF-16 range of `text`, which starts at byte `start` of the input the
/// cursor walks.
fn place(cursor: &mut Utf16Cursor<'_>, start: usize, text: &str) -> (u32, u32) {
    let first = cursor.at(start);
    (first, first.saturating_add(units(text)))
}

/// Every reading of every figure in `text`, kind by kind.
///
/// # Errors
///
/// A kind name in `options.kinds` the library does not know.
#[napi(catch_unwind)]
pub fn extract(
    text: String,
    options: Option<ExtractOptions>,
) -> napi::Result<Vec<Either<NumberEntity, UrlEntity>>> {
    let kinds: Vec<Kind> = match options.and_then(|o| o.kinds) {
        None => Kind::ALL.to_vec(),
        Some(names) => names
            .iter()
            .map(|name| {
                name.parse::<Kind>()
                    .map_err(|e| napi::Error::from_reason(e.to_string()))
            })
            .collect::<napi::Result<_>>()?,
    };
    let mut out = Vec::new();
    for kind in kinds {
        let mut cursor = Utf16Cursor::new(&text);
        match kind {
            Kind::Number => out.extend(numbers(&text).map(|n| {
                let (start, end) = place(&mut cursor, n.span.start, n.text);
                Either::A(NumberEntity {
                    kind: kind.name().to_owned(),
                    text: n.text.to_owned(),
                    start,
                    end,
                    value: NumberValue {
                        integer: n.integer.into_owned(),
                        fraction: n.fraction.map(Cow::into_owned),
                        grouped: n.grouped,
                        currency: n.currency,
                        derived: n.derived,
                    },
                })
            })),
            Kind::Url => out.extend(urls(&text).into_iter().map(|u| {
                let (start, end) = place(&mut cursor, u.span.start, u.text);
                Either::B(UrlEntity {
                    kind: kind.name().to_owned(),
                    text: u.text.to_owned(),
                    start,
                    end,
                })
            })),
        }
    }
    Ok(out)
}

/// Figures in `reply` that `corpus` does not carry.
#[napi(catch_unwind)]
#[must_use]
pub fn ungrounded_figures(reply: String, corpus: String) -> Vec<String> {
    figures_parser::ungrounded_figures(&reply, &corpus)
}

/// The parts of `reply` that neither `corpus` nor `prompt` grounds.
///
/// `corpus` is the thread, the request and the run's tool results. The parts
/// are links, placeholder targets, items labelled on another item's link,
/// and items named without the link a result carried.
#[napi(catch_unwind)]
#[must_use]
pub fn ungrounded_urls(reply: String, corpus: String, prompt: Option<String>) -> Vec<String> {
    figures_parser::ungrounded_urls(&reply, &corpus, prompt.as_deref().unwrap_or_default())
}

/// The links in `reply`, once each, that `corpus` does not carry.
#[napi(catch_unwind)]
#[must_use]
pub fn fabricated_urls(reply: String, corpus: String) -> Vec<String> {
    figures_parser::fabricated_urls(&reply, &corpus)
        .into_iter()
        .map(str::to_owned)
        .collect()
}
