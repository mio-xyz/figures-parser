//! The date kind: a weekday written beside a date the calendar puts on
//! another day, the slip a model makes when it dates this year by last
//! year's calendar. A port of chrono-node's reading of such pairs.

mod bare;
mod calendar;
mod casual;
mod clock;
mod languages;
mod names;
mod window;

use std::sync::LazyLock;

use jiff::civil::Date;
use regex::Regex;

use languages::{Found, LANGUAGES, Language, Merge, Reading, YearOf, is_bidi_mark};

use crate::text::compile;

pub use window::rows_outside_window;

/// English readings chrono takes that a writer rarely means: `may` in lower
/// case is the verb ("Friday 26 may be late"), and a slash date whose two
/// numbers could each be the month is month-first to one reader and
/// day-first to another.
static ENGLISH_AMBIGUOUS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    compile(r"(?-u:\b)may(?-u:\b)|(?-u:\b)(?:0?[1-9]|1[0-2])/(?:0?[1-9]|1[0-2])(?-u:\b)")
});

/// Whether `between`, the text between two results, lets them merge: one of
/// `separators` at most, then whitespace only.
fn joins(between: &str, separators: &[char]) -> bool {
    let rest = between.strip_prefix(separators).unwrap_or(between);
    rest.chars().all(char::is_whitespace)
}

/// Whether `between`, the text between an Arabic weekday and a date, lets
/// them merge: whitespace, bidi marks and at most one comma, Latin or Arabic.
fn joins_arabic(between: &str) -> bool {
    let mut commas = 0_u8;
    between.chars().all(|c| {
        if matches!(c, ',' | '،') {
            commas = commas.saturating_add(1);
            commas == 1
        } else {
            c.is_whitespace() || is_bidi_mark(c)
        }
    })
}

/// The reading two adjacent results merge into, as the language's refiner
/// merges them.
fn merged(merge: Merge, current: Reading, next: Reading, between: &str) -> Option<Reading> {
    match (merge, current, next) {
        (Merge::WeekdayThenDate, Reading::Weekday(weekday), Reading::Date { day, month, year })
            if joins(between, &[',']) =>
        {
            Some(Reading::Dated {
                weekday,
                day,
                month,
                year,
            })
        }
        (
            Merge::DateThenWeekday,
            Reading::Date { day, month, year }
            | Reading::Dated {
                day, month, year, ..
            },
            Reading::Weekday(weekday),
        ) if joins(between, &[',', '、', 'の']) => Some(Reading::Dated {
            weekday,
            day,
            month,
            year,
        }),
        (
            Merge::ArabicWeekdayThenDate,
            Reading::Weekday(weekday),
            Reading::Date { day, month, year },
        ) if joins_arabic(between) => Some(Reading::Dated {
            weekday,
            day,
            month,
            year,
        }),
        _ => None,
    }
}

/// The language's results over `text` with its weekdays merged into their dates.
fn pairs(language: &Language, text: &str) -> Vec<Found> {
    let mut out = Vec::new();
    let mut results = language.results(text).into_iter();
    let Some(mut current) = results.next() else {
        return out;
    };
    for next in results {
        let between = text.get(current.end..next.start).unwrap_or_default();
        if let Some(reading) = merged(language.merge, current.reading, next.reading, between) {
            current = Found {
                start: current.start,
                end: next.end,
                reading,
                to: current.to.or(next.to),
            };
        } else {
            out.push(current);
            current = next;
        }
    }
    out.push(current);
    out
}

/// The weekday-and-date pairs in `reply` whose weekday is wrong for that date.
///
/// Each is `"<as written>" (<date> is a <weekday>)`, a pair named with the
/// time written beside it on its line. A date written without a year is read
/// in the year nearest `today`. A weekday beside a bare day number ("mardi
/// 4", "Tue 4th") is read against the dates from three weeks before `today`
/// to five weeks after it that carry that number, and is wrong only when
/// none of them falls on it; the finding names the nearest.
/// A language is read only when the reply names one of its weekdays.
///
/// # Examples
///
/// ```
/// let today: figures_parser::Date = "2026-10-09".parse().unwrap();
/// assert_eq!(
///     figures_parser::misdated_weekdays("Mardi 29/09, then Fri 26 Sep.", today),
///     [r#""Fri 26 Sep" (2026-09-26 is a Saturday)"#]
/// );
///
/// // No 4th from mid-October to mid-November is a Tuesday.
/// let today: figures_parser::Date = "2026-10-30".parse().unwrap();
/// assert_eq!(
///     figures_parser::misdated_weekdays("tu as un RDV mardi 4 à 17h", today),
///     [r#""mardi 4" (2026-11-04 is a Wednesday)"#]
/// );
/// ```
#[must_use]
pub fn misdated_weekdays(reply: &str, today: Date) -> Vec<String> {
    let read: Vec<(&languages::Entry, Vec<Found>)> = LANGUAGES
        .iter()
        .filter(|e| e.gate_open(reply))
        .map(|entry| (entry, pairs(entry.language(), reply)))
        .collect();
    // Where some language read a full date, a weekday is checked against it
    // and never again as a bare day by a language that missed the month.
    let dated: Vec<(usize, usize)> = read
        .iter()
        .flat_map(|(_, found)| found.iter())
        .filter(|f| matches!(f.reading, Reading::Date { .. } | Reading::Dated { .. }))
        .map(|f| (f.start, f.end))
        .collect();
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (entry, results) in &read {
        for found in results {
            match found.reading {
                Reading::Weekday(weekday) if entry.code != "ar" => {
                    if let Some((span, date)) = casual::beside(entry.code, reply, found, today) {
                        let (from, to) = with_times(entry.code, reply, span.0, span.1);
                        let written = reply
                            .get(from..to)
                            .unwrap_or_default()
                            .trim()
                            .trim_end_matches([',', '.', ';', ':']);
                        let wrong = calendar::from_sunday(date) != weekday;
                        if wrong && !seen.contains(&written) {
                            seen.push(written);
                            out.push(bare::finding(written, date));
                        }
                        continue;
                    }
                    let Some(pair) =
                        bare::bare_day_at(reply, found.start, found.end, entry.code, entry.names)
                    else {
                        continue;
                    };
                    if dated
                        .iter()
                        .any(|&(from, to)| pair.at >= from && pair.at < to)
                    {
                        continue;
                    }
                    let written = reply.get(pair.at..pair.end).unwrap_or_default();
                    if seen.contains(&written) {
                        continue;
                    }
                    seen.push(written);
                    if let Some(date) = bare::misdated(pair, today) {
                        out.push(bare::finding(written, date));
                    }
                }
                Reading::Dated {
                    weekday,
                    day,
                    month,
                    year,
                } => {
                    if let Some(finding) = check_dated(
                        reply,
                        found,
                        entry.code,
                        (weekday, day, month, year),
                        today,
                        &mut seen,
                    ) {
                        out.push(finding);
                    }
                }
                Reading::Weekday(_) | Reading::Date { .. } | Reading::Other | Reading::Void => {}
            }
        }
    }
    out
}

/// The span of a date in `reply` with the times of day chrono merges into
/// it, as a pair is named: a time the language's words join before or after
/// it ("Fri 26 Sep at 5pm", "at 5pm, Fri 26 Sep") and the zone written after
/// it, on the same line. A bare number beside a date ("Fri 26 Sep 15") is no
/// time to a reader, so it is left out.
fn with_times(code: &str, reply: &str, start: usize, end: usize) -> (usize, usize) {
    let times = clock::times_around(code, reply, start, end);
    let shown = |t: &&clock::Clocked| t.valid && !t.bare;
    // A time a signed length follows ("5:30 PM +11mins") is merged with it
    // into a time that names its own day, as English and Italian merge them.
    let after = times.iter().filter(shown).find(|t| {
        t.start >= end
            && reply
                .get(end..t.start)
                .is_some_and(|gap| clock::joins_date_and_time(code, gap))
            && !clock::signed_length_at_start(code, reply.get(t.end..).unwrap_or_default())
    });
    let before = times.iter().filter(shown).rev().find(|t| {
        t.zoned_end <= start
            && reply
                .get(t.zoned_end..start)
                .is_some_and(|gap| gap.trim() != ":" && clock::joins_date_and_time(code, gap))
    });
    let start = before.map_or(start, |t| t.start);
    let zone = after.map_or_else(
        || end.saturating_add(clock::date_zone_len(code, reply, end)),
        |t| {
            t.zoned_end
                .saturating_add(clock::zone_name_len(reply, t.zoned_end, true))
        },
    );
    (start, zone)
}

/// Whether `text` writes a date as three dotted numbers ("5.1.2025").
fn dotted_numeric(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token = token.trim_matches(|c: char| !c.is_ascii_digit());
        let parts: Vec<&str> = token.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The time of day past the clock that chrono merges after the date of
/// `found` ("Sat 13 Jul 24", "Fri 26 Sep at 24:00"), None for none: a
/// two-digit number after a space that ends its clause reads as the year.
fn hour_after(code: &str, reply: &str, found: &Found) -> Option<PastClock> {
    let time = clock::times_around(code, reply, found.start, found.end)
        .into_iter()
        .find(|t| {
            t.start >= found.end
                && reply
                    .get(found.end..t.start)
                    .is_some_and(|gap| clock::joins_date_and_time(code, gap))
        })
        .filter(|t| !t.valid)?;
    let gap = reply.get(found.end..time.start).unwrap_or_default();
    let written = reply.get(time.start..time.end).unwrap_or_default();
    let two_digits = written.len() == 2 && written.bytes().all(|b| b.is_ascii_digit());
    // A word or a number after it makes it a count ("24 personnes").
    let ends = reply
        .get(time.end..)
        .unwrap_or_default()
        .chars()
        .find(|c| *c == '\n' || !c.is_whitespace())
        .is_none_or(|c| !c.is_alphanumeric());
    let year = (time.bare
        && two_digits
        && ends
        && !gap.is_empty()
        && gap.chars().all(char::is_whitespace))
    .then(|| written.parse().ok().map(languages::likely_ad_year))
    .flatten();
    Some(year.map_or(PastClock::Dropped, |y| PastClock::Year(y, time.end)))
}

/// A time past the clock after a date, as [`hour_after`] reads it.
enum PastClock {
    /// chrono drops the date with it.
    Dropped,
    /// It is the date's two-digit year: the year, and where it ends.
    Year(i32, usize),
}

/// The finding for a weekday merged into its date, if the weekday is wrong
/// and the pair was not reported already.
fn check_dated<'r>(
    reply: &'r str,
    found: &Found,
    code: &str,
    (weekday, day, month, year): (u8, u8, u8, YearOf),
    today: Date,
    seen: &mut Vec<&'r str>,
) -> Option<String> {
    let name = |(start, end): (usize, usize)| {
        reply
            .get(start..end)
            .unwrap_or_default()
            .trim()
            .trim_end_matches([',', '.', ';', ':'])
    };
    let date_only = name((found.start, found.end));
    let ambiguous = code == "en"
        && ENGLISH_AMBIGUOUS
            .as_ref()
            .is_some_and(|re| re.is_match(date_only));
    if ambiguous {
        return None;
    }
    let mut name_end = None;
    let year = match (year, hour_after(code, reply, found)) {
        (_, None) => year,
        // chrono reads a number after a date as its hour and drops the pair
        // when that hour is past the clock; a two-digit number no hour can be
        // ("samedi 13 juillet 24") is the year to a reader.
        (YearOf::Nearest | YearOf::Current, Some(PastClock::Year(y, end))) => {
            name_end = Some(end);
            YearOf::Written(y)
        }
        (_, Some(_)) => return None,
    };
    let year = match year {
        YearOf::Written(y) => i16::try_from(y).ok(),
        YearOf::Nearest => calendar::nearest_year(today, month, day),
        YearOf::Current => Some(today.year()),
    };
    let date = Date::new(year?, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()?;
    if calendar::from_sunday(date) == weekday {
        return None;
    }
    // English reads a dotted date day first, as the rest of the world
    // writes it; one that is right month first is let be.
    let swapped = Date::new(date.year(), date.day(), date.month()).ok();
    if code == "en"
        && dotted_numeric(date_only)
        && swapped.is_some_and(|d| calendar::from_sunday(d) == weekday)
    {
        return None;
    }
    // Only a wrong pair is named, so only one reads the times beside it.
    let written = name(name_end.map_or_else(
        || with_times(code, reply, found.start, found.end),
        |end| (found.start, end),
    ));
    if seen.contains(&written) {
        return None;
    }
    seen.push(written);
    Some(bare::finding(written, date))
}
