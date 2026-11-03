//! The date kind: a weekday written beside a date the calendar puts on
//! another day, the slip a model makes when it dates this year by last
//! year's calendar. A port of chrono-node's reading of such pairs.

mod bare;
mod calendar;
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
/// Each is `"<as written>" (<date> is a <weekday>)`. A date written without a
/// year is read in the year nearest `today`. A weekday beside a bare day
/// number ("mardi 4", "Tue 4th") is read against the dates from three weeks
/// before `today` to five weeks after it that carry that number, and is
/// wrong only when none of them falls on it; the finding names the nearest.
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
                Reading::Weekday(_) if entry.code != "ar" => {
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
                Reading::Weekday(_) | Reading::Date { .. } | Reading::Other => {}
            }
        }
    }
    out
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
    let written = reply
        .get(found.start..found.end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches([',', '.', ';', ':']);
    let ambiguous = code == "en"
        && ENGLISH_AMBIGUOUS
            .as_ref()
            .is_some_and(|re| re.is_match(written));
    if ambiguous || seen.contains(&written) {
        return None;
    }
    seen.push(written);
    let year = match year {
        YearOf::Written(y) => i16::try_from(y).ok(),
        YearOf::Nearest => calendar::nearest_year(today, month, day),
        YearOf::Current => Some(today.year()),
    };
    let date = Date::new(year?, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()?;
    (calendar::from_sunday(date) != weekday).then(|| bare::finding(written, date))
}
