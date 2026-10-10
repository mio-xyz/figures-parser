//! The duration kind: the lengths of time a text writes ("90 minutes",
//! "1h30", "deux heures et demie"), and which of a reply's the turn it
//! answers never gave.

mod corpus;
mod grammar;
mod groups;
mod words;

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

pub use grammar::Unit;
use grammar::{Part, parts};

use crate::text::{carried, compile, report_once};

/// A length of time: its clock part in milliseconds, and its calendar part in
/// hundredths of a month, since a month and a year have no fixed length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Length {
    /// Weeks, days, hours, minutes, seconds and milliseconds.
    pub milliseconds: u64,
    /// Years, quarters and months, in hundredths of a month.
    pub centimonths: u64,
}

impl Length {
    /// A length on the clock.
    #[must_use]
    pub const fn clock(milliseconds: u64) -> Self {
        Self {
            milliseconds,
            centimonths: 0,
        }
    }

    /// A length on the calendar.
    #[must_use]
    pub const fn calendar(centimonths: u64) -> Self {
        Self {
            milliseconds: 0,
            centimonths,
        }
    }

    /// The two lengths end to end.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            milliseconds: self.milliseconds.checked_add(other.milliseconds)?,
            centimonths: self.centimonths.checked_add(other.centimonths)?,
        })
    }
}

/// One duration, borrowed from the text it was found in: a part, or parts
/// in falling units joined by a space, a comma or "and" ("1 hour and 30
/// minutes", "2 jours, 4 heures", "ساعتين و30 دقيقة").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duration<'a> {
    /// As written.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
    /// How long it is.
    pub length: Length,
    /// Whether a reply is held to it: not when it is only a number glued to a
    /// bare `s`, which may be a plural ("the 90s") as well as seconds.
    pub stated: bool,
}

/// What may stand between two parts of one duration: a comma, Latin or
/// Arabic, and a word for "and", the Arabic `و` written onto the next part.
static JOIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| compile(r"^\s*[,،]?\s*(?:(?i:and|et|y|e|&)\s+|و\s*)?$"));

/// Whether `gap` joins two parts into one duration.
fn joins(gap: &str) -> bool {
    JOIN.as_ref().is_some_and(|re| re.is_match(gap))
}

/// The durations `parts` of `text` make, joining a part to the one before it
/// when its units are smaller and only a joiner stands between them.
fn join<'a>(text: &'a str, parts: &[Part]) -> Vec<Duration<'a>> {
    let mut out: Vec<Duration<'a>> = Vec::with_capacity(parts.len());
    let mut smallest: Option<Unit> = None;
    for part in parts {
        if let Some(previous) = out.last_mut()
            && smallest.is_some_and(|s| part.largest < s)
            && text
                .get(previous.span.end..part.span.start)
                .is_some_and(joins)
            && let Some(length) = previous.length.checked_add(part.length)
            && let Some(joined) = text.get(previous.span.start..part.span.end)
        {
            previous.length = length;
            previous.span.end = part.span.end;
            previous.text = joined;
            previous.stated |= part.stated;
        } else {
            out.push(Duration {
                text: text.get(part.span.clone()).unwrap_or_default(),
                span: part.span.clone(),
                length: part.length,
                stated: part.stated,
            });
        }
        smallest = Some(part.smallest);
    }
    out
}

/// Every duration in `text`, in order.
///
/// # Examples
///
/// ```
/// let found = figures_parser::durations("It took 1 hour and 30 minutes, then deux heures.");
/// let read: Vec<(&str, u64)> = found.iter().map(|d| (d.text, d.length.milliseconds)).collect();
/// assert_eq!(read, [("1 hour and 30 minutes", 5_400_000), ("deux heures", 7_200_000)]);
/// ```
#[must_use]
pub fn durations(text: &str) -> Vec<Duration<'_>> {
    join(text, &parts(text))
}

/// The durations in `reply`, once each, whose length neither `corpus` (the
/// thread, the request, the run's tool results) nor `prompt` carries.
///
/// A length is carried by a duration written out, by any one part of a
/// longer one, by an ISO 8601 duration (`PT1H30M`), by a clock reading
/// (`1:30:00`, or `12:30` read both as hours and as minutes), or by a number
/// under a field named for its unit (`"duration_ms": 5400000`); a reply's
/// duration written as the corpus wrote it is carried too. A duration that is
/// not [`stated`](Duration::stated) is never reported.
///
/// # Examples
///
/// The corpus gives the call's length in seconds:
///
/// ```
/// let corpus = r#"{"duration": 2700}"#;
/// let reply = "The call lasted 45 minutes, then 2 hours of review.";
/// assert_eq!(figures_parser::ungrounded_durations(reply, corpus, ""), ["2 hours"]);
/// ```
#[must_use]
pub fn ungrounded_durations(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let stated: Vec<Duration<'_>> = durations(reply).into_iter().filter(|d| d.stated).collect();
    if stated.is_empty() {
        return Vec::new();
    }
    let mut wanted: Vec<(Length, bool)> = stated.iter().map(|d| (d.length, false)).collect();
    corpus::mark(corpus, &mut wanted);
    corpus::mark(prompt, &mut wanted);
    let mut out: Vec<String> = Vec::new();
    for (duration, &(_, found)) in stated.iter().zip(&wanted) {
        if !found {
            report_once(&mut out, duration.text, || {
                carried(duration.text, corpus, prompt)
            });
        }
    }
    out
}
