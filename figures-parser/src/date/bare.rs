//! A weekday beside a bare day number ("mardi 4", "Tue 4th"): the pair names
//! no month, so it is read against every date near today that carries that
//! number.

use jiff::ToSpan;
use jiff::civil::Date;
use unicode_segmentation::UnicodeSegmentation;

use super::{calendar, clock, languages};
use crate::text::is_unspaced_script;

/// How many days back from today a bare day number may point.
const PAST: i64 = 21;

/// How many days ahead: a reply plans a month out more often than it looks
/// back.
const AHEAD: i64 = 35;

/// The suffixes an ordinal day takes: English, French, Spanish, Portuguese
/// and Italian.
const ORDINAL_SUFFIXES: &[&str] = &["st", "nd", "rd", "th", "er", "e", "º", "ª"];

/// What after a number makes it a time, a range, a count or a slash date
/// rather than a day.
const NOT_A_DAY_AFTER: &[&str] = &[":", "/", "-", "–", "+", "%", "h"];

/// A weekday beside a bare day number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BareDay {
    /// Byte offset of the weekday word in the reply.
    pub at: usize,
    /// One past the day number's last byte.
    pub end: usize,
    /// The weekday, Monday 1 to Sunday 7.
    pub weekday: i8,
    /// The day number, 1 to 31.
    pub day: i8,
}

/// The day a token names: one or two digits, then nothing or an ordinal
/// suffix, from 1 to 31.
fn day_number(token: &str) -> Option<i8> {
    let digits = token.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 2 {
        return None;
    }
    let suffix = token.get(digits..)?.to_lowercase();
    if !suffix.is_empty() && !ORDINAL_SUFFIXES.contains(&suffix.as_str()) {
        return None;
    }
    let day: i8 = token.get(..digits)?.parse().ok()?;
    (1..=31).contains(&day).then_some(day)
}

/// Whether a word segment holds a letter, digit or ideograph, as ICU marks
/// a segment word-like.
fn word_like(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

/// The weekday word a parser read in `reply[start..end]` and the bare day
/// number right after it, if the pair is one to check.
///
/// `names` are the language's Intl weekday names, long then short, Monday
/// first. A word after the number makes it a count ("Monday 3 meetings"), an
/// hour ("Tue 4 pm") or a month date ("Mer 15 oct") unless the rest of the
/// line opens on a time of day ("mardi 4 à 17h", "Tue 4 at 5pm").
pub fn bare_day_at(
    reply: &str,
    start: usize,
    end: usize,
    code: &str,
    names: &[&str],
) -> Option<BareDay> {
    let tail = reply.get(start..)?;
    let (name_at, name, weekday) = weekday_word(tail, end.saturating_sub(start), names)?;
    let after_name = name_at.saturating_add(name.len());
    let segments: Vec<(usize, &str)> = tail
        .get(after_name..)
        .unwrap_or_default()
        .split_word_bound_indices()
        .take(4)
        .map(|(offset, s)| (offset.saturating_add(after_name), s))
        .collect();
    // "sat 2 weeks": a lowercase English short name is as often a word.
    if code == "en" && name.chars().next().is_some_and(char::is_lowercase) {
        return None;
    }
    let &(_, gap) = segments.first()?;
    if !gap.trim().is_empty() || gap.contains('\n') {
        return None;
    }
    let &(number_at, number) = segments.get(1)?;
    let day = day_number(number)?;
    // chrono merges a weekday with the time it reads in the number after it,
    // and drops both when that time is 24:00 ("Tue 24"): no pair is left.
    let from_number = tail.get(number_at..).unwrap_or_default();
    let number_line = from_number.split('\n').next().unwrap_or_default();
    let past_clock = clock::time_at_start(code, number_line).is_some_and(clock::Time::is_invalid);
    if past_clock
        && !clock::signed_length_at_start(code, number_line.get(number.len()..).unwrap_or_default())
    {
        return None;
    }
    let after = segments.get(2).map(|&(_, s)| s);
    if after.is_some_and(|a| NOT_A_DAY_AFTER.contains(&a.to_lowercase().as_str())) {
        return None;
    }
    if let (Some(a), Some(&(next_at, next))) = (after, segments.get(3))
        && a.trim().is_empty()
        && word_like(next)
    {
        if clock::hour_at_start(code, &format!("{number} {next}")) {
            return None;
        }
        let rest = tail.get(next_at..).unwrap_or_default();
        let line = rest.split('\n').next().unwrap_or_default();
        if !clock::hour_at_start(code, line) {
            return None;
        }
    }
    let at = start.saturating_add(name_at);
    let number_end = start.saturating_add(number_at).saturating_add(number.len());
    // "Mar 11" is March to an English reader whatever the language that read it.
    if english_month_date(reply.get(at..number_end).unwrap_or_default()) {
        return None;
    }
    Some(BareDay {
        at,
        end: number_end,
        weekday,
        day,
    })
}

/// The first weekday word that starts within `span` bytes of `tail`: its
/// offset, the word and its weekday, Monday 1. A word is a word segment, but
/// a name in a script written without spaces is found inside the text, as
/// ICU's dictionary keeps "火曜日" one word.
fn weekday_word<'t>(tail: &'t str, span: usize, names: &[&str]) -> Option<(usize, &'t str, i8)> {
    let weekday = |i: usize| i8::try_from(i % 7 + 1).ok();
    // A script's mark that other scripts share ("пʼятниця" writes U+02BC,
    // which Thai uses too) does not make the language one without spaces.
    if names
        .iter()
        .any(|n| n.chars().next().is_some_and(is_unspaced_script))
    {
        return tail
            .char_indices()
            .take_while(|&(offset, _)| offset < span)
            .find_map(|(offset, _)| {
                let rest = tail.get(offset..)?;
                let (i, name) = names
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| rest.starts_with(**n))
                    .max_by_key(|(_, n)| n.len())?;
                Some((offset, rest.get(..name.len())?, weekday(i)?))
            });
    }
    tail.split_word_bound_indices()
        .take_while(|&(offset, _)| offset < span)
        .find_map(|(offset, word)| {
            if !word_like(word) {
                return None;
            }
            let lower = word.to_lowercase();
            let i = names.iter().position(|n| *n == lower)?;
            Some((offset, word, weekday(i)?))
        })
}

/// Whether English reads a date with a month at the start of `text`.
fn english_month_date(text: &str) -> bool {
    languages::LANGUAGES
        .iter()
        .find(|entry| entry.code == "en")
        .is_some_and(|entry| {
            entry.language().results(text).iter().any(|f| {
                f.start == 0
                    && matches!(
                        f.reading,
                        languages::Reading::Date { .. } | languages::Reading::Dated { .. }
                    )
            })
        })
}

/// The dates from three weeks back to five weeks ahead of `today` that carry
/// `day`, nearest first, the later of two as near.
fn window_dates(day: i8, today: Date) -> Vec<Date> {
    let mut dates: Vec<(i64, Date)> = (-PAST..=AHEAD)
        .filter_map(|offset| {
            let date = today.checked_add(offset.days()).ok()?;
            (date.day() == day).then_some((offset, date))
        })
        .collect();
    dates.sort_by_key(|&(offset, _)| (offset.abs(), -offset));
    dates.into_iter().map(|(_, date)| date).collect()
}

/// The finding for a bare day, if no date near `today` with its number falls
/// on its weekday: the nearest such date and the weekday it falls on.
pub fn misdated(bare: BareDay, today: Date) -> Option<Date> {
    let dates = window_dates(bare.day, today);
    let nearest = *dates.first()?;
    (!dates
        .iter()
        .any(|d| d.weekday().to_monday_one_offset() == bare.weekday))
    .then_some(nearest)
}

/// The finding's text: `"<as written>" (<date> is a <weekday>)`.
pub fn finding(written: &str, date: Date) -> String {
    format!(
        "\"{written}\" ({date} is a {})",
        calendar::name(date.weekday())
    )
}
