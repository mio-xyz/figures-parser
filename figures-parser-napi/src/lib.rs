//! The Node binding over `figures_parser`: `extract`, which returns the
//! figures of the kinds it reads, and the library's comparisons. Every export
//! catches a panic and throws it as an `Error`, since one unwinding into Node
//! aborts the process.

#![allow(
    clippy::needless_pass_by_value,
    reason = "napi passes a JavaScript string by value"
)]

use std::borrow::Cow;

use figures_parser::utf16::{Utf16Cursor, units};
use figures_parser::{Kind, durations, emails, identifiers, numbers, urls};
use napi::bindgen_prelude::Either5;
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

/// One email address, with where it sits in the input.
#[napi(object)]
pub struct EmailEntity {
    /// The kind of figure.
    #[napi(ts_type = "\"email\"")]
    pub kind: String,
    /// The address as written; `input.slice(start, end)` gives it back.
    pub text: String,
    /// UTF-16 offset of the first unit.
    pub start: u32,
    /// One past the last UTF-16 unit.
    pub end: u32,
}

/// One identifier, with where it sits in the input.
#[napi(object)]
pub struct IdEntity {
    /// The kind of figure.
    #[napi(ts_type = "\"id\"")]
    pub kind: String,
    /// As written; `input.slice(start, end)` gives it back.
    pub text: String,
    /// UTF-16 offset of the first unit.
    pub start: u32,
    /// One past the last UTF-16 unit.
    pub end: u32,
    /// Its shape.
    #[napi(ts_type = "\"uuid\" | \"hex\" | \"key\" | \"prefixed\" | \"token\"")]
    pub shape: String,
}

/// One duration, with where it sits in the input.
#[napi(object)]
pub struct DurationEntity {
    /// The kind of figure.
    #[napi(ts_type = "\"duration\"")]
    pub kind: String,
    /// As written; `input.slice(start, end)` gives it back.
    pub text: String,
    /// UTF-16 offset of the first unit.
    pub start: u32,
    /// One past the last UTF-16 unit.
    pub end: u32,
    /// Its weeks, days, hours, minutes and seconds, in milliseconds.
    pub milliseconds: i64,
    /// Its years, quarters and months, in hundredths of a month.
    pub centimonths: i64,
    /// Whether a reply is held to it: false for a number glued to a bare
    /// `s`, which may be a plural ("the 90s") as well as seconds.
    pub stated: bool,
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
#[allow(
    clippy::type_complexity,
    reason = "napi derives the TypeScript union from the return type as written"
)]
pub fn extract(
    text: String,
    options: Option<ExtractOptions>,
) -> napi::Result<Vec<Either5<NumberEntity, UrlEntity, EmailEntity, IdEntity, DurationEntity>>> {
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
                Either5::A(NumberEntity {
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
            Kind::Email => out.extend(emails(&text).into_iter().map(|e| {
                let (start, end) = place(&mut cursor, e.span.start, e.text);
                Either5::C(EmailEntity {
                    kind: kind.name().to_owned(),
                    text: e.text.to_owned(),
                    start,
                    end,
                })
            })),
            Kind::Id => out.extend(identifiers(&text).into_iter().map(|i| {
                let (start, end) = place(&mut cursor, i.span.start, i.text);
                Either5::D(IdEntity {
                    kind: kind.name().to_owned(),
                    text: i.text.to_owned(),
                    start,
                    end,
                    shape: i.shape.name().to_owned(),
                })
            })),
            Kind::Duration => out.extend(durations(&text).into_iter().map(|d| {
                let (start, end) = place(&mut cursor, d.span.start, d.text);
                Either5::E(DurationEntity {
                    kind: kind.name().to_owned(),
                    text: d.text.to_owned(),
                    start,
                    end,
                    milliseconds: i64::try_from(d.length.milliseconds).unwrap_or(i64::MAX),
                    centimonths: i64::try_from(d.length.centimonths).unwrap_or(i64::MAX),
                    stated: d.stated,
                })
            })),
            Kind::Url => out.extend(urls(&text).into_iter().map(|u| {
                let (start, end) = place(&mut cursor, u.span.start, u.text);
                Either5::B(UrlEntity {
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

/// The weekday-and-date pairs in `reply` whose weekday is wrong for that date.
///
/// `today` is `YYYY-MM-DD`; a date written without a year is read in the year
/// nearest it.
///
/// # Errors
///
/// `today` is not a `YYYY-MM-DD` date.
#[napi(catch_unwind)]
pub fn misdated_weekdays(reply: String, today: String) -> napi::Result<Vec<String>> {
    let today: figures_parser::Date = today
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("today is not a YYYY-MM-DD date: {e}")))?;
    Ok(figures_parser::misdated_weekdays(&reply, today))
}

/// The email addresses in `reply`, once each, that neither `corpus` nor
/// `prompt` carries; one on a reserved example domain only by `corpus`.
#[napi(catch_unwind)]
#[must_use]
pub fn ungrounded_emails(reply: String, corpus: String, prompt: Option<String>) -> Vec<String> {
    figures_parser::ungrounded_emails(&reply, &corpus, prompt.as_deref().unwrap_or_default())
}

/// The identifiers in `reply`, once each, that neither `corpus` nor `prompt`
/// carries: a UUID or a hash always, a tracker key or a prefixed id only when
/// `corpus` carries another of its prefix.
#[napi(catch_unwind)]
#[must_use]
pub fn ungrounded_identifiers(
    reply: String,
    corpus: String,
    prompt: Option<String>,
) -> Vec<String> {
    figures_parser::ungrounded_identifiers(&reply, &corpus, prompt.as_deref().unwrap_or_default())
}

/// The long mixed tokens in `reply`, once each, that neither `corpus` nor
/// `prompt` carries, for measurement only.
#[napi(catch_unwind)]
#[must_use]
pub fn unseen_tokens(reply: String, corpus: String, prompt: Option<String>) -> Vec<String> {
    figures_parser::unseen_tokens(&reply, &corpus, prompt.as_deref().unwrap_or_default())
}

/// The durations in `reply`, once each, whose length neither `corpus` nor
/// `prompt` carries, written out or as an API gives it.
#[napi(catch_unwind)]
#[must_use]
pub fn ungrounded_durations(reply: String, corpus: String, prompt: Option<String>) -> Vec<String> {
    figures_parser::ungrounded_durations(&reply, &corpus, prompt.as_deref().unwrap_or_default())
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
