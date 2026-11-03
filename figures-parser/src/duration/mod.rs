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
use grammar::{Amount, Part, parts};

use crate::number::scan::approximation_at;
use crate::text::{carried, char_before, compile, report_once};
use crate::url::urls;

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

/// One duration, borrowed from the text it was found in.
///
/// It is a part, or parts in falling units joined by a space, a hyphen, a
/// comma or "and" ("1 hour and 30 minutes", "5-minute-50-second", "2 jours,
/// 4 heures", "ساعتين و30 دقيقة"), or a range a bare number and a dash open
/// ("10–30 min").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duration<'a> {
    /// As written.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
    /// How long it is; a range's upper bound.
    pub length: Length,
    /// A range's lower bound ("10" of "10–30 min"), always below `length`.
    pub from: Option<Length>,
    /// The step it is written to: its smallest unit ("6 minutes" is written to
    /// the minute), a decimal place of it ("1.5 hours", to the tenth of an
    /// hour) or the fraction added to it ("an hour and a half", to the half
    /// hour). A printed length rounds to it within half a step.
    pub place: Length,
    /// Whether an approximation mark stands before it ("~5 minutes"), which
    /// lets a printed length round to it within a whole step.
    pub approximate: bool,
    /// Whether a reply is held to it: not when it is only a number glued to a
    /// one-letter symbol, which may be a plural ("the 90s"), a time of day
    /// ("15h") or a size as well as a length.
    pub stated: bool,
}

/// What may stand between two parts of one duration: a comma, Latin or
/// Arabic, a word for "and", the Arabic `و` written onto the next part, or a
/// hyphen alone ("5-minute-50-second").
static JOIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| compile(r"^(?:\s*[,،]?\s*(?:(?i:and|et|y|e|&)\s+|و\s*)?|-)$"));

/// Whether `gap` joins two parts into one duration.
fn joins(gap: &str) -> bool {
    JOIN.as_ref().is_some_and(|re| re.is_match(gap))
}

/// Whether `c` stands between a number and a dash in a range: a space within
/// a line.
const fn is_range_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}')
}

/// Whether the digits `written` group thousands with commas ("1,500"), where
/// a comma before other than three digits is a decimal one ("1,5").
fn groups_thousands(written: &str) -> bool {
    let whole = written.split('.').next().unwrap_or_default();
    whole.contains(',') && whole.split(',').skip(1).all(|g| g.len() == 3)
}

/// The lower bound of a range that ends in `part`, and where it starts: a
/// bare number in digits, then one dash ("10–30 min", "2 - 3 hours"), below
/// the part's own amount. The part is one unit written in digits.
fn range_low(text: &str, part: &Part, floor: usize) -> Option<(usize, Length)> {
    if part.largest != part.smallest
        || !text
            .get(part.span.start..)?
            .starts_with(|c: char| c.is_ascii_digit())
    {
        return None;
    }
    let before = text
        .get(..part.span.start)?
        .trim_end_matches(is_range_space)
        .strip_suffix(['-', '\u{2013}', '\u{2014}'])?
        .trim_end_matches(is_range_space);
    let digits = before.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ',');
    let start = digits.len();
    let low = before.get(start..)?;
    if start < floor
        || !low.starts_with(|c: char| c.is_ascii_digit())
        || !low.ends_with(|c: char| c.is_ascii_digit())
        || char_before(text, start)
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ',' | '/'))
    {
        return None;
    }
    let length = part
        .smallest
        .times(Amount::of_digits(low, groups_thousands(low))?)?;
    (length < part.length).then_some((start, length))
}

/// The durations `parts` of `text` make, joining a part to the one before it
/// when its units are smaller and only a joiner stands between them, and
/// opening a range at a bare number and a dash before a part.
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
            previous.from = previous.from.and_then(|from| from.checked_add(part.length));
            previous.span.end = part.span.end;
            previous.text = joined;
            previous.place = part.place;
            previous.stated |= part.stated;
        } else {
            let floor = out.last().map_or(0, |d| d.span.end);
            let (start, from) = range_low(text, part, floor)
                .map_or((part.span.start, None), |(start, low)| (start, Some(low)));
            out.push(Duration {
                text: text.get(start..part.span.end).unwrap_or_default(),
                span: start..part.span.end,
                length: part.length,
                from,
                place: part.place,
                approximate: approximation_at(text, start).is_some(),
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

/// A reply's duration, and whether a printed length grounds it yet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Want {
    /// Its lower bound, its length when it is no range.
    low: Length,
    /// Its length, a range's upper bound.
    high: Length,
    /// The step it is written to.
    place: Length,
    /// Whether it is marked approximate.
    approximate: bool,
    /// Whether a printed length grounds it.
    found: bool,
}

impl Want {
    /// What `duration` asks of the corpus.
    const fn of(duration: &Duration<'_>) -> Self {
        Self {
            low: match duration.from {
                Some(from) => from,
                None => duration.length,
            },
            high: duration.length,
            place: duration.place,
            approximate: duration.approximate,
            found: false,
        }
    }

    /// Whether a printed length from `low` to `high` (the same for one that is
    /// no range) grounds it. The component of the step is rounded: a single
    /// length grounds when both printed bounds lie within half a step of it
    /// (a whole step when approximate), a range when the printed bounds lie
    /// inside it widened by as much; the other component matches as written.
    fn grounded_by(&self, low: Length, high: Length) -> bool {
        let calendar = self.place.centimonths > 0;
        let pick = |l: Length| {
            if calendar {
                l.centimonths
            } else {
                l.milliseconds
            }
        };
        let other = |l: Length| {
            if calendar {
                l.milliseconds
            } else {
                l.centimonths
            }
        };
        let step = pick(self.place);
        let reach = if self.approximate {
            step.saturating_mul(2)
        } else {
            step
        };
        let (want_low, want_high) = (pick(self.low), pick(self.high));
        if other(low) != other(self.low) || other(high) != other(self.high) {
            return false;
        }
        if want_low == want_high {
            let near = |p: u64| {
                let distance = p.abs_diff(want_low);
                distance == 0 || distance.saturating_mul(2) < reach
            };
            near(pick(low)) && near(pick(high))
        } else {
            pick(low).saturating_mul(2).saturating_add(reach) >= want_low.saturating_mul(2)
                && pick(high).saturating_mul(2) <= want_high.saturating_mul(2).saturating_add(reach)
        }
    }
}

/// The durations in `reply`, once each, whose length neither `corpus` (the
/// thread, the request, the run's tool results) nor `prompt` carries.
///
/// A length is carried by a duration written out, by any one part of a
/// longer one, by an ISO 8601 duration (`PT1H30M`), by a clock reading
/// (`1:30:00`, or `12:30` read both as hours and as minutes), or by a number
/// under a field named for its unit (`"duration_ms": 5400000`); a reply's
/// duration written as the corpus wrote it is carried too.
///
/// A printed length carries a reply's when it rounds to it at the
/// [`place`](Duration::place) the reply writes it to: 350 seconds is "6
/// minutes", 90 seconds neither "1 minute" nor "2 minutes". An approximate
/// one ("~5 minutes") takes a whole step either way, and a range ("10–30
/// min") any printed length inside it. A duration that is not
/// [`stated`](Duration::stated), or that starts inside a link, is never
/// reported.
///
/// # Examples
///
/// The corpus gives the call's length in seconds:
///
/// ```
/// let corpus = r#"{"duration": 2700}"#;
/// let reply = "The call lasted 45 minutes, then 2 hours of review.";
/// assert_eq!(figures_parser::ungrounded_durations(reply, corpus, ""), ["2 hours"]);
///
/// let corpus = "duration: 350 seconds";
/// let reply = "a 6-minute walkthrough, then a 2-minute recap";
/// assert_eq!(figures_parser::ungrounded_durations(reply, corpus, ""), ["2-minute"]);
/// ```
#[must_use]
pub fn ungrounded_durations(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let mut stated: Vec<Duration<'_>> = durations(reply).into_iter().filter(|d| d.stated).collect();
    if stated.is_empty() {
        return Vec::new();
    }
    let links: Vec<Range<usize>> = urls(reply).into_iter().map(|u| u.span).collect();
    stated.retain(|d| !links.iter().any(|l| l.contains(&d.span.start)));
    let mut wanted: Vec<Want> = stated.iter().map(Want::of).collect();
    corpus::mark(corpus, &mut wanted);
    corpus::mark(prompt, &mut wanted);
    let mut out: Vec<String> = Vec::new();
    for (duration, want) in stated.iter().zip(&wanted) {
        if !want.found {
            report_once(&mut out, duration.text, || {
                carried(duration.text, corpus, prompt)
            });
        }
    }
    out
}
