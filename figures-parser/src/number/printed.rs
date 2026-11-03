//! What a text prints, read as values: the figures of a printed line, the
//! numeric tokens of a result, and a text with its links taken out.

use std::ops::Range;

use super::ground::link_spans;
use super::per_unit::value_of;
use super::scan::{Scratch, tokens};

/// The values a text prints, every reading of each figure, and apart the
/// amounts: those behind a currency mark or with a decimal part.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrintedFigures {
    /// Every value, in order.
    pub values: Vec<f64>,
    /// The values printed as amounts, in order.
    pub amounts: Vec<f64>,
}

/// The figures `text`, a printed line, holds, in each reading.
///
/// "1,250" is 1250 and 1.25, and a run of thousands also its groups, since a
/// printed table may mean either. A letter may follow a figure ("7,103MW"),
/// and a scale word is not read.
///
/// # Examples
///
/// ```
/// let printed = figures_parser::printed_figures("LT: $91.65 / 2 leads / CPL 45.83");
/// assert_eq!(printed.values, [91.65, 2.0, 45.83]);
/// assert_eq!(printed.amounts, [91.65, 45.83]);
/// ```
#[must_use]
pub fn printed_figures(text: &str) -> PrintedFigures {
    let mut scratch = Scratch::default();
    let mut out = PrintedFigures::default();
    for token in tokens(text)
        .against_letters()
        .without_signs()
        .without_scales()
    {
        let marked = token.marked_before;
        token.for_each_reading(&mut scratch, |r| {
            let Some(value) = value_of(r.integer, r.fraction) else {
                return;
            };
            out.values.push(value);
            if (marked && r.span.start == token.start) || r.fraction.is_some() {
                out.amounts.push(value);
            }
        });
    }
    out
}

/// One numeric token as a result prints it, with the values it reads as.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericToken {
    /// Byte range of its digits, without the currency mark or sign before
    /// them.
    pub span: Range<usize>,
    /// Its values: "1,5" is 1.5, "1,250" is 1250 and, read loosely, 1.25
    /// too; a run that fits neither thousands nor a decimal, the figures it
    /// sets side by side.
    pub values: Vec<f64>,
}

/// The numeric tokens of `text` as a result prints them, with the values each
/// reads as.
///
/// A letter may follow one ("90s", "7,103MW"), and no scale word is read.
/// With `loose`, the decimal reading of a run that also groups is a value
/// too ("1,250" as 1.25).
///
/// # Examples
///
/// ```
/// let read = figures_parser::numeric_tokens("took 1,5 then 1,250s", false);
/// let values: Vec<_> = read.iter().map(|t| t.values.clone()).collect();
/// assert_eq!(values, [vec![1.5], vec![1250.0]]);
/// assert_eq!(figures_parser::numeric_tokens("1,250", true)[0].values, [1250.0, 1.25]);
/// ```
#[must_use]
pub fn numeric_tokens(text: &str, loose: bool) -> Vec<NumericToken> {
    let mut scratch = Scratch::default();
    let mut out = Vec::new();
    for token in tokens(text)
        .against_letters()
        .without_currency()
        .without_signs()
        .without_scales()
    {
        let mut values = Vec::new();
        token.for_each_reading(&mut scratch, |r| {
            if r.derived || (r.ambiguous && !loose) {
                return;
            }
            if let Some(value) = value_of(r.integer, r.fraction) {
                values.push(value);
            }
        });
        if !values.is_empty() {
            out.push(NumericToken {
                span: token.start..token.end(),
                values,
            });
        }
    }
    out
}

/// `text` with each link it holds replaced by a space: a link's digits (a
/// timestamp, an id) are the link check's to judge, never a figure.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     figures_parser::without_links("See https://x.io/a/206788 for $1,250."),
///     "See   for $1,250."
/// );
/// ```
#[must_use]
pub fn without_links(text: &str) -> String {
    let spans = link_spans(text);
    if spans.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for span in spans {
        if span.start < at {
            continue;
        }
        out.push_str(text.get(at..span.start).unwrap_or_default());
        out.push(' ');
        at = span.end;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}
