//! The lengths a corpus carries: its written durations and their parts, and
//! the forms an API returns a length in: ISO 8601 durations, clock readings
//! and numbers under a field named for their unit. A clock reading and a
//! field are read by hand at each `:` and `=`, since a pattern's per-match
//! cost on every timestamp of a log outweighs the scan.

use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::FxHashMap;

use super::grammar::{Amount, Unit, parts};
use super::{Length, join};
use crate::text::{compile, is_ascii_word_byte, is_iso_date, run_end, run_start, same_word};

/// An ISO 8601 duration (`P1Y2M`, `PT1H30M`, `P3W`).
static ISO: LazyLock<Option<Regex>> = LazyLock::new(|| {
    compile(
        r"(?-u:\b)P(?:[0-9]+(?:[.,][0-9]+)?[YMWD])*(?:T(?:[0-9]+(?:[.,][0-9]+)?[HMS])+)?(?-u:\b)",
    )
});

/// Every unit word, lowercase, to its unit.
static UNIT_WORDS: LazyLock<FxHashMap<&'static str, Unit>> = LazyLock::new(|| {
    Unit::ALL
        .into_iter()
        .flat_map(|unit| unit.words().iter().map(move |&word| (word, unit)))
        .collect()
});

/// The unit a field's last word names, looked up lowercase: an ASCII word is
/// lowercased on the stack, since a log's every `key=value` field is a lookup.
fn unit_named(word: &str) -> Option<Unit> {
    let mut lower = [0u8; 16];
    if !word.is_ascii() {
        return Unit::ALL
            .into_iter()
            .find(|unit| unit.words().iter().any(|w| same_word(w, word)));
    }
    let buffer = lower.get_mut(..word.len())?;
    buffer.copy_from_slice(word.as_bytes());
    buffer.make_ascii_lowercase();
    let key = std::str::from_utf8(buffer).ok()?;
    UNIT_WORDS.get(key).copied().or(match key {
        "ms" => Some(Unit::Millisecond),
        "s" => Some(Unit::Second),
        "h" => Some(Unit::Hour),
        _ => None,
    })
}

/// The units a field's last word names: a unit's words, `ms`, `s` or `h`
/// (`elapsed_ms`), or a bare `duration`, read in seconds and in minutes, the
/// two units APIs give it in.
fn field_units(word: &str) -> [Option<Unit>; 2] {
    if word.eq_ignore_ascii_case("duration") {
        [Some(Unit::Second), Some(Unit::Minute)]
    } else {
        [unit_named(word), None]
    }
}

/// The last word of a field name, split at `_` and at a lowercase letter
/// followed by a capital (`duration_ms`, `durationMs`).
fn last_word(key: &str) -> &str {
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (i, c) in key.char_indices() {
        if c == '_' {
            start = i.saturating_add(1);
        } else if c.is_ascii_uppercase() && previous.is_some_and(|p| p.is_ascii_lowercase()) {
            start = i;
        }
        previous = Some(c);
    }
    key.get(start..).unwrap_or_default()
}

/// The lengths `parts` add up to.
fn sum(parts: &[(Amount, Unit)]) -> Option<Length> {
    parts
        .iter()
        .try_fold(Length::default(), |total, &(amount, unit)| {
            total.checked_add(unit.times(amount)?)
        })
}

/// The length an ISO 8601 duration writes; None for a bare `P` or a
/// designator out of place.
fn iso_length(written: &str) -> Option<Length> {
    let body = written.strip_prefix('P')?;
    let mut total = Length::default();
    let mut any = false;
    let mut time = false;
    let mut start = 0;
    for (i, c) in body.char_indices() {
        if c == 'T' {
            time = true;
            start = i.saturating_add(1);
        } else if !(c.is_ascii_digit() || c == '.' || c == ',') {
            let unit = match (time, c) {
                (false, 'Y') => Unit::Year,
                (false, 'M') => Unit::Month,
                (false, 'W') => Unit::Week,
                (false, 'D') => Unit::Day,
                (true, 'H') => Unit::Hour,
                (true, 'M') => Unit::Minute,
                (true, 'S') => Unit::Second,
                _ => return None,
            };
            let amount = Amount::of_digits(body.get(start..i)?, false)?;
            total = total.checked_add(unit.times(amount)?)?;
            any = true;
            start = i.saturating_add(1);
        }
    }
    any.then_some(total)
}

/// The lengths a clock reading writes: `h:mm:ss`, and `a:bb` both as
/// minutes and seconds and as hours and minutes.
fn clock_lengths(written: &str) -> [Option<Length>; 2] {
    let (main, fraction) = written
        .split_once('.')
        .map_or((written, None), |(m, f)| (m, Some(f)));
    let mut fields = main.split(':').map(|f| Amount::of_digits(f, false));
    let (Some(Some(a)), Some(Some(b))) = (fields.next(), fields.next()) else {
        return [None, None];
    };
    match fields.next() {
        Some(Some(c)) => {
            let fraction = fraction.and_then(|f| {
                let digits = Amount::of_digits(f, false)?;
                let scale = u32::try_from(f.len()).ok()?;
                Unit::Second.times(Amount { scale, ..digits })
            });
            let whole = sum(&[(a, Unit::Hour), (b, Unit::Minute), (c, Unit::Second)]);
            [
                whole.and_then(|w| w.checked_add(fraction.unwrap_or_default())),
                None,
            ]
        }
        Some(None) => [None, None],
        None => [
            sum(&[(a, Unit::Minute), (b, Unit::Second)]),
            sum(&[(a, Unit::Hour), (b, Unit::Minute)]),
        ],
    }
}

/// The lengths `number` writes under the field `key`, when the field's last
/// word names a unit.
fn field_lengths(key: &str, number: &str) -> [Option<Length>; 2] {
    let amount = Amount::of_digits(number, false);
    field_units(last_word(key)).map(|unit| unit?.times(amount?))
}

/// The clock reading whose first colon is at byte `colon`: one to three
/// digits, two more, optionally two more and up to three decimals, standing
/// apart from any word. A later colon of a reading is not a first one, and a
/// reading right after a date (`2026-10-09 14:30`) is a time of day.
fn clock_at(text: &str, colon: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = run_start(bytes, colon, |b| b.is_ascii_digit());
    let before = start.checked_sub(1).and_then(|i| bytes.get(i)).copied();
    let dated = start
        .checked_sub(11)
        .and_then(|from| bytes.get(from..start.saturating_sub(1)))
        .is_some_and(is_iso_date);
    if !(1..=3).contains(&colon.saturating_sub(start))
        || before.is_some_and(|b| is_ascii_word_byte(b) || b == b':')
        || dated
    {
        return None;
    }
    let pair = |at: usize| {
        let tens = bytes.get(at).copied()?;
        let ones = bytes.get(at.saturating_add(1)).copied()?;
        ((b'0'..=b'5').contains(&tens) && ones.is_ascii_digit()).then_some(at.saturating_add(2))
    };
    let free = |at: usize| !bytes.get(at).copied().is_some_and(is_ascii_word_byte);
    let minutes = pair(colon.saturating_add(1))?;
    let seconds = (bytes.get(minutes) == Some(&b':'))
        .then(|| pair(minutes.saturating_add(1)))
        .flatten();
    let end = seconds
        .map(|after| {
            let digits = run_end(bytes, after.saturating_add(1), |b| b.is_ascii_digit());
            let decimals = digits.saturating_sub(after.saturating_add(1));
            let fraction =
                bytes.get(after) == Some(&b'.') && (1..=3).contains(&decimals) && free(digits);
            if fraction { digits } else { after }
        })
        .filter(|&end| free(end))
        .or_else(|| free(minutes).then_some(minutes))?;
    text.get(start..end)
}

/// The field whose separator is at byte `at`: a name, an optional quote and
/// spaces, the separator, spaces and an optional quote, then a number.
fn field_at(text: &str, at: usize) -> Option<(&str, &str)> {
    let bytes = text.as_bytes();
    let mut key_end = run_start(bytes, at, |b| b == b' ' || b == b'\t');
    if let Some(quote) = key_end
        .checked_sub(1)
        .filter(|&i| matches!(bytes.get(i), Some(b'"' | b'\'')))
    {
        key_end = quote;
    }
    let key = text.get(run_start(bytes, key_end, is_ascii_word_byte)..key_end)?;
    let mut start = run_end(bytes, at.saturating_add(1), |b| b == b' ' || b == b'\t');
    if matches!(bytes.get(start), Some(b'"' | b'\'')) {
        start = start.saturating_add(1);
    }
    let whole = run_end(bytes, start, |b| b.is_ascii_digit());
    let digits = run_end(bytes, whole.saturating_add(1), |b| b.is_ascii_digit());
    let end = if bytes.get(whole) == Some(&b'.') && digits > whole.saturating_add(1) {
        digits
    } else {
        whole
    };
    let has_letter = key.bytes().any(|b| b.is_ascii_alphabetic());
    (has_letter && end > start).then_some((key, text.get(start..end)?))
}

/// The lengths read at the `:` or `=` at byte `at`: a clock reading's or a
/// field's.
fn at_separator(text: &str, at: usize) -> [Option<Length>; 2] {
    if let Some(reading) = clock_at(text, at) {
        return clock_lengths(reading);
    }
    field_at(text, at).map_or([None, None], |(key, number)| field_lengths(key, number))
}

/// Marks each length in `wanted` that `text` carries. The written durations
/// are read first, whole; the ISO durations, clock readings and fields after
/// them are read only until every length is marked.
pub fn mark(text: &str, wanted: &mut [(Length, bool)]) {
    if wanted.iter().all(|&(_, found)| found) {
        return;
    }
    let found = parts(text);
    let readings = found
        .iter()
        .map(|p| p.length)
        .chain(join(text, &found).into_iter().map(|d| d.length))
        .chain(
            ISO.iter()
                .flat_map(|re| re.find_iter(text))
                .filter_map(|m| iso_length(m.as_str())),
        )
        .chain(
            memchr::memchr2_iter(b':', b'=', text.as_bytes())
                .flat_map(|at| at_separator(text, at))
                .flatten(),
        );
    for length in readings {
        let mut all = true;
        for (want, marked) in wanted.iter_mut() {
            *marked |= *want == length;
            all &= *marked;
        }
        if all {
            return;
        }
    }
}
