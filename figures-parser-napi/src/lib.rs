//! The Node binding over `figures_parser`: `extract` and `ungrounded_figures`.

#![allow(
    clippy::needless_pass_by_value,
    reason = "napi passes a JavaScript string by value"
)]

use std::borrow::Cow;

use figures_parser::utf16::{Utf16Cursor, units};
use figures_parser::{Kind, extract_numbers};
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
    /// Written behind a currency sign.
    pub currency: bool,
    /// A neighbour reading of a thousands run: grounds, never stated.
    pub derived: bool,
}

/// One reading of one figure, with where it sits in the input.
#[napi(object)]
pub struct Entity {
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

/// What `extract` reads.
#[napi(object)]
pub struct ExtractOptions {
    /// The kinds to read, by name; every kind when absent. An unknown name is an error.
    pub kinds: Option<Vec<String>>,
}

/// Every reading of every figure in `text`.
///
/// # Errors
///
/// A kind name in `options.kinds` the library does not know.
#[napi]
pub fn extract(text: String, options: Option<ExtractOptions>) -> napi::Result<Vec<Entity>> {
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
        match kind {
            Kind::Number => {
                let mut cursor = Utf16Cursor::new(&text);
                out.extend(extract_numbers(&text).into_iter().map(|n| {
                    let start = cursor.at(n.span.start);
                    Entity {
                        kind: kind.name().to_owned(),
                        text: n.text.to_owned(),
                        start,
                        end: start.saturating_add(units(n.text)),
                        value: NumberValue {
                            integer: n.integer.into_owned(),
                            fraction: n.fraction.map(Cow::into_owned),
                            grouped: n.grouped,
                            currency: n.currency,
                            derived: n.derived,
                        },
                    }
                }));
            }
        }
    }
    Ok(out)
}

/// Figures in `reply` that `corpus` does not carry.
#[napi]
#[must_use]
pub fn ungrounded_figures(reply: String, corpus: String) -> Vec<String> {
    figures_parser::ungrounded_figures(&reply, &corpus)
}
