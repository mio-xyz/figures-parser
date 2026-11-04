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
use super::{calendar, casual, clock, merged};

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one of chrono's components, set apart from the others"
)]
struct Read {
    /// Byte offset of its text.
    start: usize,
    /// One past its last byte.
    end: usize,
    /// The date it starts on, None for a weekday alone, a time alone or anything else.
    from: Option<Day>,
    /// Whether it is a range; chrono reads no window from a range of other things.
    range: bool,
    /// Where a range of dates ends.
    to: Option<Day>,
    /// For a range of times, whether it ends on the next day.
    next_day: bool,
    /// Whether it writes a time of day.
    timed: bool,
    /// Whether it writes a date or a weekday; a time alone does not.
    dated: bool,
    /// The bytes it is named by: all of it, but a bare number chrono reads as
    /// its hour ("Oct 20 - 12 posts") left out.
    name: (usize, usize),
    /// Whether every part of it exists on the calendar and the clock.
    valid: bool,
    /// Whether it is a time written only in digits and a dot.
    unlikely: bool,
}

impl Read {
    /// A result read by a date parser.
    const fn date(
        start: usize,
        end: usize,
        from: Option<Day>,
        to: Option<Day>,
        range: bool,
    ) -> Self {
        Self {
            start,
            end,
            from,
            range,
            to,
            next_day: false,
            timed: false,
            dated: true,
            name: (start, end),
            valid: true,
            unlikely: false,
        }
    }
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
pub(super) fn joins_range(code: &str, between: &str) -> bool {
    RANGE_WORDS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, re)| re.as_ref())
        .is_some_and(|re| re.is_match(between))
}

/// The range words between two dates Spanish and Portuguese write that
/// chrono's refiners for them do not read ("28 de março a 4 abr.", "del 3
/// al 9 de mayo").
static IBERIAN_RANGE_WORDS: LazyLock<Option<Regex>> =
    LazyLock::new(|| crate::text::compile(r"(?i)^\s*(?:a|al|hasta|à|ao|até)\s*$"));

/// Whether `between` joins two dates into a range in the language `code`:
/// its refiner's words, and the Iberian ones chrono misses.
fn joins_dates(code: &str, between: &str) -> bool {
    joins_range(code, between)
        || (matches!(code, "es" | "pt")
            && IBERIAN_RANGE_WORDS
                .as_ref()
                .is_some_and(|re| re.is_match(between)))
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

/// The language's date results over `text` and the times of day written
/// beside them, the longer of two overlapping results kept as chrono's
/// overlap removal keeps it, then weekdays merged into their dates.
fn dates_and_times(code: &str, language: &Language, text: &str, today: Date) -> Vec<Read> {
    let mut dates = language.every_result(text);
    let casual = casual::days_in(code, text, today);
    if dates.is_empty() && casual.is_empty() {
        return Vec::new();
    }
    let lengths = lengths_in(code, text, today);
    if !casual.is_empty() || !lengths.is_empty() {
        dates.extend(
            casual
                .into_iter()
                .chain(lengths)
                .map(|(start, end, date)| day_found(start, end, date)),
        );
        dates.sort_by_key(|f| f.start);
    }
    let mut starts: Vec<usize> = dates
        .iter()
        .flat_map(|d| clock::time_starts_around(code, text, d.start, d.end, true))
        .filter(|&at| !dates.iter().any(|d| inside_dotted_date(text, d, at)))
        .collect();
    starts.sort_unstable();
    starts.dedup();
    let mut times = clock::times_at(code, text, starts.into_iter());
    // A time a range word joins to one of these is read too ("2/11 9:00 -
    // 17:00"), so the two make one range of times.
    let mut follow: Vec<usize> = times
        .iter()
        .flat_map(|t| {
            (t.zoned_end..t.zoned_end.saturating_add(16).min(text.len())).filter(|&at| {
                text.is_char_boundary(at)
                    && text
                        .get(t.zoned_end..at)
                        .is_some_and(|gap| !gap.is_empty() && joins_range(code, gap))
            })
        })
        .filter(|at| !times.iter().any(|t| t.start <= *at && *at < t.end))
        .collect();
    if !follow.is_empty() {
        follow.sort_unstable();
        follow.dedup();
        times.extend(clock::times_at(code, text, follow.into_iter()));
        times.sort_by_key(|t| t.start);
    }
    let mut all: Vec<Item> = dates.into_iter().map(Item::Date).collect();
    all.extend(times.into_iter().map(Item::Time));
    all.sort_by_key(Item::start);
    let width = |item: &Item| {
        text.get(item.start()..item.end())
            .map_or(0, |t| t.chars().map(char::len_utf16).sum::<usize>())
    };
    let mut kept: Vec<Item> = Vec::with_capacity(all.len());
    for item in all {
        match kept.last_mut() {
            Some(last) if item.start() < last.end() => {
                if width(&item) > width(last) {
                    *last = item;
                }
            }
            _ => kept.push(item),
        }
    }
    let mut out: Vec<Read> = Vec::with_capacity(kept.len());
    let mut pending: Option<Found> = None;
    for item in kept {
        match item {
            Item::Date(found) => {
                let joined = pending.and_then(|current| {
                    let between = text.get(current.end..found.start).unwrap_or_default();
                    merged(language.merge, current.reading, found.reading, between).map(|reading| {
                        Found {
                            start: current.start,
                            end: found.end,
                            reading,
                            to: current.to.or(found.to),
                        }
                    })
                });
                if joined.is_none()
                    && let Some(current) = pending
                {
                    out.push(date_read(&current, today));
                }
                pending = Some(joined.unwrap_or(found));
            }
            Item::Time(time) => {
                if let Some(current) = pending.take() {
                    out.push(date_read(&current, today));
                }
                out.push(time_read(&time));
            }
        }
    }
    if let Some(current) = pending {
        out.push(date_read(&current, today));
    }
    out
}

/// Whether `at` falls after the first dot of a date written as three
/// dotted numbers ("25.11.25", "12.2.2025"). chrono's time scan tries the
/// date's first numbers ("25.11") and resumes past them, so the time it
/// reads further in ("11.25 bis 2025") is one a reader sees as no time;
/// the date is kept.
fn inside_dotted_date(text: &str, date: &Found, at: usize) -> bool {
    let Some(written) = text.get(date.start..date.end) else {
        return false;
    };
    let mut parts = written.split('.');
    let shape = [(1, 2), (1, 2), (2, 4)].iter().all(|&(min, max)| {
        parts.next().is_some_and(|p| {
            (min..=max).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit())
        })
    }) && parts.next().is_none();
    let first_dot = written
        .find('.')
        .map_or(date.end, |d| date.start.saturating_add(d));
    shape && at > first_dot && at < date.end
}

/// The lengths from now in `text` that land on a day ("3 days ago"), each
/// with where it stands and that day; read where a word or number starts.
fn lengths_in(code: &str, text: &str, today: Date) -> Vec<(usize, usize, Date)> {
    let mut out = Vec::new();
    let mut from = 0;
    for (at, c) in text.char_indices() {
        if at < from || !c.is_alphanumeric() {
            continue;
        }
        let starts_word = crate::text::char_before(text, at).is_none_or(|b| !b.is_alphanumeric());
        if !starts_word {
            continue;
        }
        if let Some((end, date)) = clock::length_date_at(code, text, at, today) {
            out.push((at, end, date));
            from = end;
        }
    }
    out
}

/// A casual day or a length from now as a date result: a day chrono holds
/// as certain, year included.
fn day_found(start: usize, end: usize, date: Date) -> Found {
    let part = |n: i8| u8::try_from(n).unwrap_or_default();
    Found {
        start,
        end,
        reading: Reading::Date {
            day: part(date.day()),
            month: part(date.month()),
            year: YearOf::Written(i32::from(date.year())),
        },
        to: None,
    }
}

/// The read a time of day makes: no date, and no name for a bare number.
const fn time_read(time: &clock::Clocked) -> Read {
    Read {
        start: time.start,
        end: time.zoned_end,
        from: None,
        range: time.next_day.is_some(),
        next_day: matches!(time.next_day, Some(true)),
        to: None,
        timed: true,
        dated: false,
        name: if time.bare {
            (time.start, time.start)
        } else {
            (time.start, time.zoned_end)
        },
        valid: time.valid,
        unlikely: time.unlikely,
    }
}

/// A date parser's result or a time of day, before overlapping ones are removed.
enum Item {
    /// A date parser's result.
    Date(Found),
    /// A time of day.
    Time(clock::Clocked),
}

impl Item {
    /// Where its text starts.
    const fn start(&self) -> usize {
        match self {
            Self::Date(found) => found.start,
            Self::Time(time) => time.start,
        }
    }

    /// Where its text ends, as its parser read it.
    const fn end(&self) -> usize {
        match self {
            Self::Date(found) => found.end,
            Self::Time(time) => time.end,
        }
    }
}

/// The read a date result makes.
fn date_read(found: &Found, today: Date) -> Read {
    let from = day_of(found, today);
    let to = found.to.zip(from).map(|(day, from)| Day {
        of_month: day,
        ..from
    });
    Read {
        valid: !matches!(found.reading, Reading::Void),
        ..Read::date(found.start, found.end, from, to, found.to.is_some())
    }
}

/// The language's results over `text`, merged as chrono's refiners merge
/// them: two times a range word joins into one range of times, a date and
/// the time beside it into one ("Nov 4, 9:00-17:00" is one day from 9 to
/// 17), then two results a range word joins into a range. Every language
/// merges a date with its time before it joins ranges, so "4. Nov. - 15
/// Uhr" is one date and "10月2号15点30分-10月23日" runs to the 23rd.
fn reads(code: &str, language: &Language, text: &str, today: Date) -> Vec<Read> {
    let reads = dates_and_times(code, language, text, today);
    let merged = dated_times(code, text, time_ranges(code, text, reads));
    // English and Italian join ranges last, after the zone names and after
    // dropping a lone time in digits and a dot; the others join them first.
    let mut out = if matches!(code, "en" | "it") {
        let mut named = with_zone_names(text, merged);
        named.retain(|r| !r.unlikely || r.dated);
        // They merge dates and times again once zones are taken in
        // ("10:00 UTC Sep 14").
        ranges(code, text, dated_times(code, text, named))
    } else {
        with_zone_names(text, ranges(code, text, merged))
    };
    out.retain(|r| r.valid && (!r.unlikely || r.dated));
    out
}

/// The results with the zone name written after each taken in.
fn with_zone_names(text: &str, mut reads: Vec<Read>) -> Vec<Read> {
    for read in &mut reads {
        let zone = clock::zone_name_len(text, read.end, read.timed);
        if zone > 0 {
            if read.name.1 == read.end {
                read.name.1 = read.end.saturating_add(zone);
            }
            read.end = read.end.saturating_add(zone);
        }
    }
    reads
}

/// The results with two times a range word joins merged into one range of
/// times; chrono puts the earlier first, so the range never runs past the
/// day.
fn time_ranges(code: &str, text: &str, reads: Vec<Read>) -> Vec<Read> {
    let mut out: Vec<Read> = Vec::with_capacity(reads.len());
    for read in reads {
        let Some(last) = out.last_mut() else {
            out.push(read);
            continue;
        };
        let between = text.get(last.end..read.start).unwrap_or_default();
        let times = !last.dated && !read.dated && !last.range && !read.range;
        if !times || !joins_range(code, between) {
            out.push(read);
            continue;
        }
        let named = last.name.1 > last.name.0 || read.name.1 > read.name.0;
        *last = Read {
            end: read.end,
            range: true,
            next_day: false,
            name: if named {
                (last.start, read.end)
            } else {
                (last.start, last.start)
            },
            valid: last.valid && read.valid,
            unlikely: false,
            ..*last
        };
    }
    out
}

/// The results with each date merged with the time beside it, as chrono's
/// `AbstractMergeDateTimeRefiner` merges them.
fn dated_times(code: &str, text: &str, reads: Vec<Read>) -> Vec<Read> {
    let mut out: Vec<Read> = Vec::with_capacity(reads.len());
    for read in reads {
        let Some(last) = out.last_mut() else {
            out.push(read);
            continue;
        };
        let between = text.get(last.end..read.start).unwrap_or_default();
        let date_then_time = !last.timed && !read.dated;
        // "24: July 23" is a count before a date: a bare colon joins only a
        // date to the time after it.
        let time_then_date = !last.dated && !read.timed && between.trim() != ":";
        if !(date_then_time || time_then_date) || !clock::joins_date_and_time(code, between) {
            out.push(read);
            continue;
        }
        let (date, time) = if date_then_time {
            (*last, read)
        } else {
            (read, *last)
        };
        *last = with_time(date, time);
    }
    out
}

/// A date merged with its time: a range of times on a date that writes no
/// range of its own ends on that date, or on the next when it ends before
/// it starts ("Tuesday 9pm - 1am").
fn with_time(date: Read, time: Read) -> Read {
    let start = date.start.min(time.start);
    let end = date.end.max(time.end);
    let to = if date.range {
        date.to
    } else if time.range {
        date.from.and_then(|from| {
            if time.next_day {
                let next = from.date()?.checked_add(1.day()).ok()?;
                Some(Day {
                    of_month: u8::try_from(next.day()).ok()?,
                    month: u8::try_from(next.month()).ok()?,
                    year: next.year(),
                    year_written: from.year_written,
                })
            } else {
                Some(from)
            }
        })
    } else {
        None
    };
    let named = time.name.1 > time.name.0;
    Read {
        start,
        end,
        from: date.from,
        range: date.range || time.range,
        to,
        next_day: false,
        timed: true,
        dated: true,
        name: if named { (start, end) } else { date.name },
        valid: date.valid && time.valid,
        unlikely: false,
    }
}

/// The results with two a range word joins merged into a range, as
/// chrono's `AbstractMergeDateRangeRefiner` merges them.
fn ranges(code: &str, text: &str, reads: Vec<Read>) -> Vec<Read> {
    let mut out: Vec<Read> = Vec::with_capacity(reads.len());
    for read in reads {
        let Some(last) = out.last_mut() else {
            out.push(read);
            continue;
        };
        let between = text.get(last.end..read.start).unwrap_or_default();
        let dates = last.dated && read.dated;
        if last.range
            || read.range
            || !(joins_range(code, between) || dates && joins_dates(code, between))
        {
            out.push(read);
            continue;
        }
        let ends = last
            .from
            .zip(read.from)
            .and_then(|(a, b)| merged_range(a, b));
        *last = Read {
            start: last.start,
            end: read.end,
            from: ends.map(|(a, _)| a),
            range: true,
            to: ends.map(|(_, b)| b),
            next_day: false,
            timed: last.timed || read.timed,
            dated: last.dated || read.dated,
            name: (last.start, read.end),
            valid: last.valid && read.valid,
            unlikely: false,
        };
    }
    out
}

/// A window a line names: its first and last date, the range as written,
/// and the order its slash dates are written in.
struct Window<'l> {
    /// Its first date.
    first: Date,
    /// Its last date.
    last: Date,
    /// The range as the line writes it.
    written: &'l str,
    /// Whether the line's numeric dates are written day first (true) or
    /// month first (false); None when none of them says.
    order: Option<bool>,
    /// The first and last date read in the other order, when none says and
    /// the line writes a slash date: either may be the window.
    other: Option<(Date, Date)>,
}

/// The first range of dates in `line` that runs forward, its slash dates
/// read in the order the line's own numeric dates write them.
fn window_in<'l>(
    code: &str,
    language: &Language,
    line: &'l str,
    today: Date,
) -> Option<Window<'l>> {
    let order = numeric_order(line);
    let swapped = order
        .filter(|&day_first| swaps(code) && day_first != reads_day_first(code))
        .and_then(|_| swapped_slashes(line));
    let read = swapped.as_deref().unwrap_or(line);
    let range = |text: &str| {
        reads(code, language, text, today)
            .into_iter()
            .find_map(|r| {
                let first = r.from?.date()?;
                let last = r.to?.date()?;
                (first <= last).then_some((first, last, r.name))
            })
    };
    let (first, last, name) = range(read)?;
    let other = (order.is_none() && swaps(code))
        .then(|| swapped_slashes(line))
        .flatten()
        .and_then(|other_order| range(&other_order))
        .map(|(from, to, _)| (from, to));
    Some(Window {
        first,
        last,
        written: line.get(name.0..name.1).unwrap_or_default().trim(),
        order,
        other,
    })
}

/// Whether the language `code` reads a slash date day first; English and
/// Japanese read one month first.
fn reads_day_first(code: &str) -> bool {
    !matches!(code, "en" | "ja")
}

/// Whether `code` reads a day and a month joined by a slash, which swapping
/// the two numbers reads in the other order; Chinese writes none, Arabic its
/// own.
fn swaps(code: &str) -> bool {
    !matches!(code, "zh" | "ar")
}

/// The numeric dates of `text`: each a day and a month, in the order
/// written, the byte both start at, and whether a slash joins them. Only a
/// pair of one- or two-digit numbers no digit, slash or dot runs into; a
/// dotted pair needs a third part or a closing dot ("9.2.2028", "2.12."),
/// so a decimal is none.
fn numeric_pairs(text: &str) -> impl Iterator<Item = (u8, u8, usize, bool)> + '_ {
    let bytes = text.as_bytes();
    let digits_from = move |at: usize| {
        bytes
            .get(at..)
            .map_or(0, |b| b.iter().take_while(|d| d.is_ascii_digit()).count())
    };
    (0..bytes.len()).filter_map(move |at| {
        let prev = at.checked_sub(1).and_then(|p| bytes.get(p));
        if prev.is_some_and(|&p| p.is_ascii_digit() || matches!(p, b'/' | b'.' | b':')) {
            return None;
        }
        let a = digits_from(at);
        let sep = *bytes.get(at.saturating_add(a))?;
        let b_at = at.saturating_add(a).saturating_add(1);
        let b = digits_from(b_at);
        if !(1..=2).contains(&a) || !(1..=2).contains(&b) || !matches!(sep, b'/' | b'.') {
            return None;
        }
        let after = b_at.saturating_add(b);
        let next = bytes.get(after);
        let joined = match sep {
            b'/' => !next.is_some_and(u8::is_ascii_digit),
            _ => {
                next == Some(&b'.') && (2..=4).contains(&digits_from(after.saturating_add(1)))
                    || next == Some(&b'.')
                        && !bytes
                            .get(after.saturating_add(1))
                            .is_some_and(u8::is_ascii_digit)
            }
        };
        let number =
            |from: usize, len: usize| text.get(from..from.saturating_add(len))?.parse().ok();
        joined
            .then(|| Some((number(at, a)?, number(b_at, b)?, at, sep == b'/')))
            .flatten()
    })
}

/// The order `text`'s numeric dates are written in: day first where one
/// can only be ("15/12", "30/1/2026") or is dotted ("9.2.2028"), month first
/// where one can only be ("12/25"); None when none says, or they disagree.
fn numeric_order(text: &str) -> Option<bool> {
    let mut order = None;
    for (first, second, _, slash) in numeric_pairs(text) {
        let says = if !slash || (first > 12 && second <= 12) {
            Some(true)
        } else if second > 12 && first <= 12 {
            Some(false)
        } else {
            None
        };
        match (order, says) {
            (_, None) => {}
            (None, found) => order = found,
            (Some(known), Some(found)) if known != found => return None,
            _ => {}
        }
    }
    order
}

/// `text` with the two numbers of every slash date swapped ("2/12" reads
/// "12/2"), so a reader of the other order reads the dates as written; the
/// text keeps its length, so a reading's offsets name the original. None
/// when it writes no slash date.
fn swapped_slashes(text: &str) -> Option<String> {
    let mut out: Option<Vec<u8>> = None;
    for (first, second, at, slash) in numeric_pairs(text) {
        if !slash || first == second {
            continue;
        }
        let digits = |from: usize| {
            text.get(from..)
                .map_or(0, |t| t.bytes().take_while(u8::is_ascii_digit).count())
        };
        let a_len = digits(at);
        let b_at = at.saturating_add(a_len).saturating_add(1);
        let end = b_at.saturating_add(digits(b_at));
        let (Some(head), Some(tail)) =
            (text.get(at..at.saturating_add(a_len)), text.get(b_at..end))
        else {
            continue;
        };
        let buffer = out.get_or_insert_with(|| text.as_bytes().to_vec());
        let swapped = tail
            .bytes()
            .chain(std::iter::once(b'/'))
            .chain(head.bytes());
        for (slot, byte) in buffer
            .get_mut(at..end)
            .unwrap_or_default()
            .iter_mut()
            .zip(swapped)
        {
            *slot = byte;
        }
    }
    out.and_then(|bytes| String::from_utf8(bytes).ok())
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

/// Whether `line` is a heading: marked as one, or wholly emphasised.
fn is_heading(line: &str) -> bool {
    let text = line.trim();
    text.starts_with('#')
        || (trim_emphasis(text).len() < text.len()
            && text.starts_with(EMPHASIS)
            && text.ends_with(EMPHASIS))
}

/// One language's findings over the lines, added to `out` once each.
/// `windows` holds the window this language reads on each line that is no
/// row, and `named` whether any language reads one there, or a date on a
/// heading: a heading another language reads as a window ends the one this
/// language had open, and so does a heading that dates what follows it in
/// a way no language reads as a window.
fn outside_for(
    code: &str,
    language: &Language,
    lines: &[&str],
    (windows, named): (&[Option<Window<'_>>], &[bool]),
    today: Date,
    out: &mut Vec<String>,
) {
    let mut window: Option<&Window<'_>> = None;
    for (i, line) in lines.iter().enumerate() {
        let Some(lead) = row_lead(line) else {
            match windows.get(i).and_then(Option::as_ref) {
                Some(opened) => window = Some(opened),
                None if !keeps_window(line) || named.get(i).copied().unwrap_or(false) => {
                    window = None;
                }
                None => {}
            }
            continue;
        };
        let Some(open) = window else {
            continue;
        };
        let lead_date = |read: &str| {
            reads(code, language, read, today)
                .into_iter()
                .find(|r| r.start == 0)
                .filter(|r| !r.range)
                .and_then(|r| Some((r.from.and_then(Day::date)?, r.name)))
        };
        // A row's slash date is read in the order its heading writes them;
        // where the heading does not say, a date either order puts inside
        // the window is let be.
        let swapped = swaps(code).then(|| swapped_slashes(lead)).flatten();
        let other_order = || swapped.as_deref().and_then(lead_date).map(|(date, _)| date);
        let own = match open.order {
            Some(day_first) if day_first != reads_day_first(code) => swapped
                .as_deref()
                .map_or_else(|| lead_date(lead), lead_date),
            _ => lead_date(lead),
        };
        let Some((date, name)) = own else {
            continue;
        };
        let outside = |day: Date| {
            (day < open.first || day > open.last)
                && open
                    .other
                    .is_none_or(|(first, last)| day < first || day > last)
        };
        if outside(date) && (open.order.is_some() || other_order().is_none_or(outside)) {
            let written = lead.get(name.0..name.1).unwrap_or_default().trim();
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
/// Each is `"<row's date>" under "<window>"`, as written with the times beside
/// the dates, an en or em dash in either read as a hyphen. A heading opens a
/// window when it writes a range of dates ("past week (Oct 23-30)", "Oct 19
/// to Oct 26", "19-26 oct.") or a day with a range of hours ("Nov 4,
/// 9:00-17:00"); a blank line, a label ending in a colon or a line wholly
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
    let entries: Vec<_> = LANGUAGES.iter().filter(|e| e.code != "ar").collect();
    let windows: Vec<Vec<Option<Window<'_>>>> = entries
        .iter()
        .map(|entry| {
            lines
                .iter()
                .map(|line| {
                    row_lead(line)
                        .is_none()
                        .then(|| window_in(entry.code, entry.language(), line, today))
                        .flatten()
                })
                .collect()
        })
        .collect();
    let named: Vec<bool> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            windows
                .iter()
                .any(|w| w.get(i).is_some_and(Option::is_some))
                || (row_lead(line).is_none()
                    && is_heading(line)
                    && entries.iter().any(|entry| {
                        reads(entry.code, entry.language(), line, today)
                            .iter()
                            .any(|r| r.dated || r.range)
                    }))
        })
        .collect();
    let mut out = Vec::new();
    for (entry, own) in entries.iter().zip(&windows) {
        outside_for(
            entry.code,
            entry.language(),
            &lines,
            (own, &named),
            today,
            &mut out,
        );
    }
    out
}
