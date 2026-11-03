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
use grammar::{Amount, Part, is_number_symbol, parts, starts_clean};

use crate::number::scan::approximation_at;
use crate::text::{compile, report_once};
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
    /// A range written high to low ("30-10 minutes", "step 3 - 2 minutes")
    /// is read as the span between its two numbers.
    pub from: Option<Length>,
    /// The step it is written to: its smallest unit ("6 minutes" is written to
    /// the minute), a decimal place of it ("1.5 hours", to the tenth of an
    /// hour) or the fraction added to it ("an hour and a half", to the half
    /// hour). A printed length rounds to it within half a step.
    pub place: Length,
    /// The other reading of a length whose digits fit two, and the step it
    /// is written to: a lone dot or comma before three digits may group them
    /// or end the whole number ("1.250 Sekunden" is 1,250 seconds, or 1.25).
    /// A printed length that grounds either grounds the duration.
    pub loose: Option<(Length, Length)>,
    /// Whether an approximation mark stands before it ("~5 minutes"), which
    /// lets a printed length round to it within a whole step.
    pub approximate: bool,
    /// Whether a reply is held to it: not when it is only a number glued to a
    /// one-letter symbol, which may be a plural ("the 90s"), a time of day
    /// ("15h") or a size as well as a length.
    pub stated: bool,
}

/// What may stand between two parts of one duration, within a line: a
/// comma, Latin or Arabic, a word for "and", the Arabic `و` written onto the
/// next part, or a hyphen ("5-minute-50-second", "2 hours - 30 minutes").
static JOIN: LazyLock<Option<Regex>> = LazyLock::new(|| {
    compile(
        r"^(?:[ \t\x{A0}\x{202F}\x{2009}]*[,،]?[ \t\x{A0}\x{202F}\x{2009}]*(?:(?i:and|et|y|e|&)[ \t\x{A0}\x{202F}\x{2009}]+|و[ \t\x{A0}\x{202F}\x{2009}]*)?|[ \t\x{A0}\x{202F}\x{2009}]*-[ \t\x{A0}\x{202F}\x{2009}]*)$",
    )
});

/// Whether `gap` joins two parts into one duration.
fn joins(gap: &str) -> bool {
    JOIN.as_ref().is_some_and(|re| re.is_match(gap))
}

/// Whether `c` stands between a number and a dash in a range: a space within
/// a line.
const fn is_range_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}')
}

/// The lower bound of a range that ends in `part`, and where it starts: a
/// bare number, then one dash within the line ("10–30 min", "2 - 3 hours").
/// The part is one unit written in digits. The bound may be above the
/// part's own amount, a range written high to low.
fn range_low(text: &str, part: &Part, floor: usize) -> Option<(usize, Length)> {
    if part.largest != part.smallest
        || !text
            .get(part.span.start..)?
            .starts_with(|c: char| c.is_numeric())
    {
        return None;
    }
    let before = text
        .get(..part.span.start)?
        .trim_end_matches(is_range_space)
        .strip_suffix(['-', '\u{2013}', '\u{2014}'])?
        .trim_end_matches(is_range_space);
    let low = before
        .get(
            before
                .trim_end_matches(|c: char| c.is_numeric() || is_number_symbol(c))
                .len()..,
        )?
        .trim_start_matches(|c: char| !c.is_numeric());
    // Groups a plain space joins that fit no one number are side by side
    // ("phase 2 10-30 min"): the last one opens the range.
    let (low, amount) = if let Some((amount, _)) = Amount::of_printed(low) {
        (low, amount)
    } else {
        let (_, last) = low.rsplit_once(' ')?;
        (last, Amount::of_printed(last)?.0)
    };
    let start = before.len().saturating_sub(low.len());
    if start < floor || !low.ends_with(|c: char| c.is_numeric()) || !starts_clean(text, start) {
        return None;
    }
    Some((start, part.smallest.times(amount)?))
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
            previous.loose = None;
            previous.stated |= part.stated;
        } else {
            let floor = out.last().map_or(0, |d| d.span.end);
            let (start, from, length) = match range_low(text, part, floor) {
                Some((start, low)) if low < part.length => (start, Some(low), part.length),
                Some((start, high)) if high > part.length => (start, Some(part.length), high),
                _ => (part.span.start, None, part.length),
            };
            out.push(Duration {
                text: text.get(start..part.span.end).unwrap_or_default(),
                span: start..part.span.end,
                length,
                from,
                place: part.place,
                loose: part.loose.filter(|_| from.is_none()),
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
    /// Its other reading and that reading's step, when its digits fit two.
    loose: Option<(Length, Length)>,
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
            loose: duration.loose,
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
        self.reads_as(self.low, self.high, self.place, low, high)
            || self
                .loose
                .is_some_and(|(length, place)| self.reads_as(length, length, place, low, high))
    }

    /// Whether a printed length from `low` to `high` rounds to the reading
    /// from `want_low` to `want_high` written to `place`, as
    /// [`grounded_by`](Self::grounded_by) says.
    fn reads_as(
        &self,
        want_low: Length,
        want_high: Length,
        place: Length,
        low: Length,
        high: Length,
    ) -> bool {
        let calendar = place.centimonths > 0;
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
        let step = pick(place);
        let reach = if self.approximate {
            step.saturating_mul(2)
        } else {
            step
        };
        if other(low) != other(want_low) || other(high) != other(want_high) {
            return false;
        }
        let (want_low, want_high) = (pick(want_low), pick(want_high));
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
/// list ("1 hour, 30 minutes"), by an ISO 8601 duration (`PT1H30M`), by a
/// clock reading (`1:30:00`, or `12:30` read both as hours and as minutes),
/// or by a number under a field named for its unit (`"duration_ms":
/// 5400000`).
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
    let mut found = parts(reply);
    if !found.iter().any(|p| p.stated) {
        return Vec::new();
    }
    // A part inside a link is the link's ("cal.com/x/30min"), and joins no
    // part after it.
    let links: Vec<Range<usize>> = urls(reply).into_iter().map(|u| u.span).collect();
    found.retain(|p| !links.iter().any(|l| l.contains(&p.span.start)));
    let in_link = |at: usize| links.iter().any(|l| l.contains(&at));
    let stated: Vec<Duration<'_>> = join(reply, &found)
        .into_iter()
        .filter(|d| d.stated)
        .filter_map(|d| {
            if !in_link(d.span.start) {
                return Some(d);
            }
            // A range whose low is the link's ("cal.com/x/16 – 1880 seconds")
            // is its part alone.
            let part = found.iter().find(|p| p.span.end == d.span.end)?;
            Some(Duration {
                text: reply.get(part.span.clone())?,
                span: part.span.clone(),
                length: part.length,
                from: None,
                place: part.place,
                loose: part.loose,
                approximate: false,
                stated: part.stated,
            })
        })
        .collect();
    let mut wanted: Vec<Want> = stated.iter().map(Want::of).collect();
    corpus::mark(corpus, &mut wanted);
    corpus::mark(prompt, &mut wanted);
    let mut out: Vec<String> = Vec::new();
    for (duration, want) in stated.iter().zip(&wanted) {
        if !want.found {
            report_once(&mut out, duration.text, || false);
        }
    }
    out
}
