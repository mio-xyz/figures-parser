//! The date kind: a weekday written beside a date the calendar puts on
//! another day, the slip a model makes when it dates this year by last
//! year's calendar. A port of chrono-node's reading of such pairs.

mod calendar;
mod languages;
mod names;

use std::sync::LazyLock;

use jiff::civil::Date;
use regex::Regex;

use languages::{Found, LANGUAGES, Language, Merge, Reading, YearOf};

use crate::text::compile;

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
/// year is read in the year nearest `today`. A language is read only when
/// the reply names one of its weekdays.
///
/// # Examples
///
/// ```
/// let today: figures_parser::Date = "2026-10-09".parse().unwrap();
/// assert_eq!(
///     figures_parser::misdated_weekdays("Mardi 29/09, then Fri 26 Sep.", today),
///     [r#""Fri 26 Sep" (2026-09-26 is a Saturday)"#]
/// );
/// ```
#[must_use]
pub fn misdated_weekdays(reply: &str, today: Date) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for entry in LANGUAGES.iter().filter(|e| e.gate_open(reply)) {
        let language = entry.language();
        for found in pairs(language, reply) {
            let Reading::Dated {
                weekday,
                day,
                month,
                year,
            } = found.reading
            else {
                continue;
            };
            let written = reply
                .get(found.start..found.end)
                .unwrap_or_default()
                .trim()
                .trim_end_matches([',', '.', ';', ':']);
            let ambiguous = entry.code == "en"
                && ENGLISH_AMBIGUOUS
                    .as_ref()
                    .is_some_and(|re| re.is_match(written));
            if ambiguous || seen.contains(&written) {
                continue;
            }
            seen.push(written);
            let year = match year {
                YearOf::Written(y) => i16::try_from(y).ok(),
                YearOf::Nearest => calendar::nearest_year(today, month, day),
                YearOf::Current => Some(today.year()),
            };
            let date = year.and_then(|y| {
                Date::new(y, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
            });
            let Some(date) = date else {
                continue;
            };
            if calendar::from_sunday(date) != weekday {
                out.push(format!(
                    "\"{written}\" ({date} is a {})",
                    calendar::name(date.weekday())
                ));
            }
        }
    }
    out
}
