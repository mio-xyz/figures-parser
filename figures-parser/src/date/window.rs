//! A table or list under a heading that names a date window ("past week
//! (Oct 23-30)") whose rows lead with dates outside it. The dates are read
//! by the same chrono-node port as the weekday check, in every language
//! chrono reads, with its range rules: a month-name date's own range ("Oct
//! 23-30", "19-26 oct.") and two dates a language's range word joins ("Oct
//! 19 to Oct 26", "Oct 28 - Nov 3").

use std::sync::LazyLock;

use jiff::ToSpan;
use jiff::civil::Date;
use regex::Regex;

use super::languages::{Found, LANGUAGES, Language, Reading, YearOf};
use super::{calendar, pairs};

/// The dashes chrono reads as the first day alone, not a range ("Oct 23–30").
const RANGE_DASHES: &[char] = &['–', '—'];

/// What opens a list item.
const BULLETS: &[&str] = &["- ", "* ", "• ", "◦ ", "‣ "];

/// What emphasis wraps around a cell or a label.
const EMPHASIS: &[char] = &['*', '_', '`', '~', ' '];

/// A date with its year as chrono holds it: a value, written or implied.
#[derive(Debug, Clone, Copy)]
struct Day {
    /// The day of the month.
    of_month: u8,
    /// The month, 1 to 12.
    month: u8,
    /// The year, written or implied.
    year: i16,
    /// Whether the text writes the year.
    year_written: bool,
}

impl Day {
    /// The calendar date, None for a day the month does not have.
    fn date(self) -> Option<Date> {
        Date::new(
            self.year,
            i8::try_from(self.month).ok()?,
            i8::try_from(self.of_month).ok()?,
        )
        .ok()
    }
}

/// One result over a line: where it stands, the date it starts on if it
/// has a day and a month, and where a range ends.
#[derive(Debug, Clone, Copy)]
struct Read {
    /// Byte offset of its text.
    start: usize,
    /// One past its last byte.
    end: usize,
    /// The date it starts on, None for a weekday alone or anything else.
    from: Option<Day>,
    /// Whether it is a range; chrono reads no window from a range of other things.
    range: bool,
    /// Where a range of dates ends.
    to: Option<Day>,
}

/// The date a reading starts on, with its year resolved as chrono resolves it.
fn day_of(found: &Found, today: Date) -> Option<Day> {
    let (Reading::Date { day, month, year }
    | Reading::Dated {
        day, month, year, ..
    }) = found.reading
    else {
        return None;
    };
    let (year, year_written) = match year {
        YearOf::Written(y) => (i16::try_from(y).ok()?, true),
        YearOf::Nearest => (calendar::nearest_year(today, month, day)?, false),
        YearOf::Current => (today.year(), false),
    };
    Some(Day {
        of_month: day,
        month,
        year,
        year_written,
    })
}

/// Each language's range words between two dates, as its merge refiner
/// reads them; Swedish has none.
static RANGE_WORDS: LazyLock<Vec<(&str, Option<Regex>)>> = LazyLock::new(|| {
    [
        ("en", r"(?i)^\s*(?:to|-|–|until|through|till)\s*$"),
        ("fr", r"(?i)^\s*(?:à|a|au|-)\s*$"),
        ("de", r"(?i)^\s*(?:bis(?:\s*(?:am|zum))?|-)\s*$"),
        ("es", r"^\s*-\s*$"),
        ("it", r"(?i)^\s*(?:a|al|-|–|fino\s*a|fino\s*al)\s*$"),
        ("pt", r"^\s*-\s*$"),
        ("nl", r"(?i)^\s*(?:tot|-)\s*$"),
        ("fi", r"^\s*(?:-|–)\s*$"),
        ("ru", r"(?i)^\s*(?:и до|и по|до|по|-)\s*$"),
        ("uk", r"(?i)^\s*(?:і до|і по|до|по|-)\s*$"),
        ("ja", r"^\s*(?:から|－|ー|-|～|~)\s*$"),
        ("zh", r"^\s*(?:至|到|-|~|～|－|ー)\s*$"),
        ("vi", r"^\s*(?:–|-|đến|tới|và)\s*$"),
    ]
    .into_iter()
    .map(|(code, re)| (code, crate::text::compile(re)))
    .collect()
});

/// Whether `between` joins two results into a range in the language `code`.
fn joins_range(code: &str, between: &str) -> bool {
    RANGE_WORDS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, re)| re.as_ref())
        .is_some_and(|re| re.is_match(between))
}

/// Two dates a range word joins, ordered as chrono orders them: a later
/// start moves the end a year on, or the start a year back, when the text
/// leaves that year unwritten, and is swapped with the end otherwise.
fn merged_range(mut from: Day, mut to: Day) -> Option<(Day, Day)> {
    if !from.year_written && to.year_written {
        from.year = to.year;
    }
    if !to.year_written && from.year_written {
        to.year = from.year;
    }
    let (start, end) = (from.date()?, to.date()?);
    if start <= end {
        return Some((from, to));
    }
    if !to.year_written && end.checked_add(1.year()).ok()? > start {
        to.year = to.year.checked_add(1)?;
        return Some((from, to));
    }
    if !from.year_written && start.checked_sub(1.year()).ok()? < end {
        from.year = from.year.checked_sub(1)?;
        return Some((from, to));
    }
    Some((to, from))
}

/// The language's results over `text`, weekdays merged into their dates and
/// ranges into one result, as chrono's refiners merge them.
fn reads(code: &str, language: &Language, text: &str, today: Date) -> Vec<Read> {
    let mut out: Vec<Read> = Vec::new();
    for found in pairs(language, text) {
        let from = day_of(&found, today);
        let own = Read {
            start: found.start,
            end: found.end,
            from,
            range: found.to.is_some(),
            to: found.to.zip(from).map(|(day, from)| Day {
                of_month: day,
                ..from
            }),
        };
        let Some(last) = out.last_mut() else {
            out.push(own);
            continue;
        };
        let between = text.get(last.end..own.start).unwrap_or_default();
        if last.range || own.range || !joins_range(code, between) {
            out.push(own);
            continue;
        }
        let ends = last
            .from
            .zip(own.from)
            .and_then(|(a, b)| merged_range(a, b));
        *last = Read {
            start: last.start,
            end: own.end,
            from: ends.map(|(a, _)| a),
            range: true,
            to: ends.map(|(_, b)| b),
        };
    }
    out
}

/// A window a line names: its first and last date, and the range as written.
struct Window<'l> {
    /// Its first date.
    first: Date,
    /// Its last date.
    last: Date,
    /// The range as the line writes it.
    written: &'l str,
}

/// The first range of dates in `line` that runs forward.
fn window_in<'l>(
    code: &str,
    language: &Language,
    line: &'l str,
    today: Date,
) -> Option<Window<'l>> {
    reads(code, language, line, today)
        .into_iter()
        .find_map(|r| {
            let first = r.from?.date()?;
            let last = r.to?.date()?;
            (first <= last).then(|| Window {
                first,
                last,
                written: line.get(r.start..r.end).unwrap_or_default().trim(),
            })
        })
}

/// `text` without the emphasis marks and spaces around it.
fn trim_emphasis(text: &str) -> &str {
    text.trim_matches(EMPHASIS)
}

/// What a table row or list item leads with: its first cell, or the text
/// after its bullet or number; None for any other line.
fn row_lead(line: &str) -> Option<&str> {
    let text = line.trim_start();
    if let Some(row) = text.strip_prefix('|') {
        return Some(trim_emphasis(row.split('|').next().unwrap_or_default()));
    }
    if let Some(rest) = BULLETS.iter().find_map(|b| text.strip_prefix(b)) {
        return Some(trim_emphasis(rest));
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let rest = text.get(digits..).unwrap_or_default();
    (digits > 0)
        .then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")))
        .flatten()
        .map(trim_emphasis)
}

/// Whether a line keeps a window open over the rows after it: a blank line,
/// a heading, a label ending in a colon, or a line wholly emphasised. Any
/// other prose ends what the heading covers.
fn keeps_window(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() || text.starts_with('#') {
        return true;
    }
    let bare = trim_emphasis(text);
    bare.ends_with(':')
        || (!bare.is_empty() && text.starts_with(EMPHASIS) && text.ends_with(EMPHASIS))
}

/// One language's findings over the lines, added to `out` once each.
fn outside_for(
    code: &str,
    language: &Language,
    lines: &[&str],
    today: Date,
    out: &mut Vec<String>,
) {
    let mut window: Option<Window<'_>> = None;
    for line in lines {
        let Some(lead) = row_lead(line) else {
            match window_in(code, language, line, today) {
                Some(opened) => window = Some(opened),
                None if !keeps_window(line) => window = None,
                None => {}
            }
            continue;
        };
        let Some(open) = &window else {
            continue;
        };
        let Some(own) = reads(code, language, lead, today)
            .into_iter()
            .find(|r| r.start == 0)
        else {
            continue;
        };
        if own.range {
            continue;
        }
        let Some(date) = own.from.and_then(Day::date) else {
            continue;
        };
        if date < open.first || date > open.last {
            let written = lead.get(own.start..own.end).unwrap_or_default().trim();
            let finding = format!("\"{written}\" under \"{}\"", open.written);
            if !out.contains(&finding) {
                out.push(finding);
            }
        }
    }
}

/// The rows of a table or list, under a heading that names a date window,
/// whose own leading date falls outside that window.
///
/// Each is `"<row's date>" under "<window>"`, as written, with an en or em
/// dash in either read as a hyphen. A heading opens a window when it writes
/// a range of dates ("past week (Oct 23-30)", "Oct 19 to Oct 26", "19-26
/// oct."); a blank line, a label ending in a colon or a line wholly
/// emphasised keeps it open, and other prose closes it. A row is a table
/// row, read by its first cell, or a list item, read by what follows its
/// bullet or number; a row whose lead is a range is left alone. A date
/// without a year is read in the year nearest `today`. Every language the
/// weekday check reads is read, Arabic aside.
///
/// # Examples
///
/// ```
/// let today: figures_parser::Date = "2026-10-30".parse().unwrap();
/// let reply = "Your posts from the past week (Oct 23–30):\n\
///              | Date | Post |\n\
///              |------|------|\n\
///              | Oct 29 | Sixteen plays |\n\
///              | Oct 20 | Paid ads through Slack |";
/// assert_eq!(
///     figures_parser::rows_outside_window(reply, today),
///     [r#""Oct 20" under "Oct 23-30""#]
/// );
/// ```
#[must_use]
pub fn rows_outside_window(reply: &str, today: Date) -> Vec<String> {
    let text = reply.replace(RANGE_DASHES, "-");
    let lines: Vec<&str> = text.split('\n').collect();
    if !lines.iter().any(|l| row_lead(l).is_some()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in LANGUAGES.iter().filter(|e| e.code != "ar") {
        outside_for(entry.code, entry.language(), &lines, today, &mut out);
    }
    out
}
