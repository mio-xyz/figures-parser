//! The lengths a corpus carries: its written durations and their parts, and
//! the forms an API returns a length in: ISO 8601 durations, clock readings
//! and numbers under a field named for their unit. A clock reading and a
//! field are read by hand at each `:` and `=`, since a pattern's per-match
//! cost on every timestamp of a log outweighs the scan.

use std::sync::LazyLock;

use regex::Regex;
use rustc_hash::FxHashMap;

use super::grammar::{Amount, Part, Unit, parts};
use super::{Duration, Length, Want, join};
use crate::text::{
    compile, contains_ignoring_case, is_ascii_word_byte, is_iso_date, run_end, run_start, same_word,
};

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

/// The clock reading whose first colon is at byte `colon`, and where it
/// starts: one to three digits, two more, optionally two more and up to three
/// decimals, standing apart from any word. A later colon of a reading is not
/// a first one, and a reading right after a date (`2026-10-09 14:30`) is a
/// time of day.
fn clock_at(text: &str, colon: usize) -> Option<(usize, &str)> {
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
    Some((start, text.get(start..end)?))
}

/// The words a length's label holds, matched inside the label's last word
/// case aside (`duration`, `video_duration`, `elapsedTime`, "took").
const LENGTH_CUES: &[&str] = &[
    "duration",
    "durée",
    "dauer",
    "duración",
    "duracion",
    "durata",
    "duração",
    "duracao",
    "duur",
    "length",
    "runtime",
    "uptime",
    "elapsed",
    "took",
    "lasted",
    "lasting",
    "lasts",
    "taken",
    "spent",
    "temps",
    "tempo",
    "longueur",
    "länge",
    "laufzeit",
    "recording",
    "video",
    "audio",
    "clip",
    "episode",
    "total",
];

/// How many bytes before a clock reading its label is looked for in.
const LABEL_REACH: usize = 48;

/// Words that may stand between a length's label and its reading ("took
/// about 1:30:00").
const FILLERS: &[&str] = &[
    "about",
    "approx",
    "approximately",
    "around",
    "roughly",
    "of",
    "for",
    "in",
    "is",
    "was",
    "env",
    "environ",
    "ca",
    "etwa",
];

/// Whether the clock reading that starts at byte `start` is labelled as a
/// length: its first field is past a day's hours ("75:30"), or the last word
/// before it on its line, past punctuation and a filler word, names a length
/// (`"duration": "1:30:00"`, "took 1:30:00"). A reading after "at", or with
/// no label ("`[10:05:12] INFO`", "starts 14:00:00"), is as often a time of
/// day.
fn labelled_length(text: &str, start: usize, reading: &str) -> bool {
    let first = reading
        .split(':')
        .next()
        .and_then(|f| f.parse::<u32>().ok());
    if first.is_some_and(|hours| hours >= 24) {
        return true;
    }
    // A label stands close before its reading; a window of a few words keeps
    // a long line from being walked back once per reading.
    let mut from = start.saturating_sub(LABEL_REACH);
    while !text.is_char_boundary(from) {
        from = from.saturating_add(1);
    }
    let Some(before) = text.get(from..start) else {
        return false;
    };
    let line = before.rsplit_once('\n').map_or(before, |(_, l)| l);
    let last_word = |t: &str| -> (usize, usize) {
        let t_end = t
            .trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '_'))
            .len();
        let word_start = t.get(..t_end).map_or(0, |w| {
            w.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_')
                .len()
        });
        (word_start, t_end)
    };
    let (mut word_start, mut word_end) = last_word(line);
    if line
        .get(word_start..word_end)
        .is_some_and(|w| FILLERS.iter().any(|f| w.eq_ignore_ascii_case(f)))
    {
        (word_start, word_end) = last_word(line.get(..word_start).unwrap_or_default());
    }
    line.get(word_start..word_end).is_some_and(|word| {
        LENGTH_CUES
            .iter()
            .any(|cue| contains_ignoring_case(word, cue))
    })
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
    if let Some((start, reading)) = clock_at(text, at) {
        if labelled_length(text, start, reading) {
            return clock_lengths(reading);
        }
        return [None, None];
    }
    field_at(text, at).map_or([None, None], |(key, number)| field_lengths(key, number))
}

/// Whether the part at `index` of `parts`, inside `duration`, is a length a
/// corpus printed on its own: not a range's upper bound, which bounds the
/// range, and not a part joined to the part before or after it ("5 minutes
/// 40 seconds", "1 hour, 30 minutes", "25 minutes and 59 seconds"), which is
/// one length with it.
fn stands_alone(duration: &Duration<'_>, parts: &[Part], index: usize) -> bool {
    let Some(part) = parts.get(index) else {
        return true;
    };
    let inside = |p: &Part| duration.span.start <= p.span.start && p.span.end <= duration.span.end;
    let previous = index
        .checked_sub(1)
        .and_then(|i| parts.get(i))
        .filter(|p| inside(p));
    if let Some(from) = duration.from
        && previous.is_none()
    {
        // The first part of a range is its upper bound, unless the range is
        // written high to low ("step 3 - 2 minutes"), where it is the low.
        let rest = parts
            .get(index.saturating_add(1)..)
            .unwrap_or_default()
            .iter()
            .take_while(|p| inside(p))
            .fold(Length::default(), |sum, p| {
                sum.checked_add(p.length).unwrap_or(sum)
            });
        if Some(from) != part.length.checked_add(rest) {
            return false;
        }
    }
    let next = parts.get(index.saturating_add(1)).filter(|p| inside(p));
    previous.is_none() && next.is_none()
}

/// The words before an hour that make it a time of day ("à 15 h", "um 15h",
/// "from 14h").
const CLOCK_WORDS: &[&str] = &[
    "à",
    "at",
    "um",
    "vers",
    "dès",
    "ab",
    "bis",
    "gegen",
    "hasta",
    "desde",
    "las",
    "alle",
    "ore",
    "entre",
    "from",
    "until",
    "till",
    "by",
    "às",
    "jusqu'à",
    "jusqu’à",
];

/// The words that join the two ends of a range of hours ("14h à 15h30").
const RANGE_WORDS: &[&str] = &["-", "–", "—", "à", "a", "to", "bis", "et", "and", "y"];

/// What follows an hour of the day in French ("3 h du matin").
const DAY_PARTS: &[&str] = &["du matin", "du soir", "de l'après-midi", "de l’après-midi"];

/// Whether `text` ends in an hour written with its symbol: digits, an
/// optional space, `h` and optional minutes ("14h", "15 h 30", "8h15").
fn ends_in_hour(text: &str) -> bool {
    text.trim_end_matches(|c: char| c.is_ascii_digit())
        .trim_end_matches([' ', '\u{a0}'])
        .strip_suffix('h')
        .is_some_and(|t| {
            t.trim_end_matches([' ', '\u{a0}'])
                .ends_with(|c: char| c.is_ascii_digit())
        })
}

/// Whether `text` starts with an hour written with its symbol ("15h30",
/// "16 h").
fn starts_with_hour(text: &str) -> bool {
    let t = text.trim_start_matches(|c: char| c.is_ascii_digit());
    t.len() < text.len() && t.trim_start_matches([' ', '\u{a0}']).starts_with('h')
}

/// Whether the hour written with its symbol from byte `start` to `end` of
/// `text` is a time of day: a clock word before it, a part of the day after
/// it, or another hour a range word joins to it on either side ("de 14h à
/// 15h30").
fn time_of_day(text: &str, start: usize, end: usize) -> bool {
    let is_space = |c: char| c == ' ' || c == '\u{a0}' || c == '\u{202f}';
    let before = text
        .get(..start)
        .unwrap_or_default()
        .trim_end_matches(is_space);
    let after = text
        .get(end..)
        .unwrap_or_default()
        .trim_start_matches(is_space);
    let word_start = before
        .trim_end_matches(|c: char| c.is_alphabetic() || c == '\'' || c == '’')
        .len();
    let word = before.get(word_start..).unwrap_or_default();
    if CLOCK_WORDS.iter().any(|w| same_word(w, word)) {
        return true;
    }
    if DAY_PARTS
        .iter()
        .any(|p| after.get(..p.len()).is_some_and(|a| same_word(a, p)))
    {
        return true;
    }
    RANGE_WORDS.iter().any(|w| {
        let next = after
            .get(..w.len())
            .filter(|a| same_word(a, w))
            .and_then(|_| after.get(w.len()..));
        let previous = before
            .get(before.len().saturating_sub(w.len())..)
            .filter(|b| same_word(b, w))
            .and_then(|_| before.get(..before.len().saturating_sub(w.len())));
        next.is_some_and(|n| starts_with_hour(n.trim_start_matches(is_space)))
            || previous.is_some_and(|p| ends_in_hour(p.trim_end_matches(is_space)))
    })
}

/// Marks each duration in `wanted` that a length `text` prints grounds, a
/// range printed as its two bounds and each part of a longer duration as a
/// length of its own, a part's thousands comma read as a decimal one too. The written durations are read first,
/// whole; the ISO durations, clock readings and fields after them are read
/// only until every duration is marked.
pub fn mark(text: &str, wanted: &mut [Want]) {
    if wanted.iter().all(|w| w.found) {
        return;
    }
    let found = parts(text);
    let joined = join(text, &found);
    // An hour written with its symbol is a length only when no clock word,
    // part of the day or other hour makes it a time of day.
    let clock_hour = |at: usize, start: usize, end: usize| {
        found.get(at).is_some_and(|p: &Part| {
            !p.stated && p.largest == Unit::Hour && time_of_day(text, start, end)
        })
    };
    let single = |l: Length| (l, l);
    // Parts and durations both run in order, so one cursor finds each part's.
    let mut cursor = 0;
    let mut alone = |(index, p): &(usize, &Part)| {
        while joined
            .get(cursor)
            .is_some_and(|d| d.span.end <= p.span.start)
        {
            cursor = cursor.saturating_add(1);
        }
        joined
            .get(cursor)
            .filter(|d| d.span.start <= p.span.start)
            .is_none_or(|d| stands_alone(d, &found, *index))
    };
    let readings = found
        .iter()
        .enumerate()
        .filter(|(i, p)| !clock_hour(*i, p.span.start, p.span.end))
        .filter(|p| alone(p))
        .flat_map(|(_, p)| [Some(p.length), p.loose.map(|(length, _)| length)])
        .flatten()
        .map(single)
        .chain(
            joined
                .iter()
                .filter(|d| {
                    d.stated
                        || found
                            .binary_search_by_key(&d.span.start, |p| p.span.start)
                            .map_or(true, |i| !clock_hour(i, d.span.start, d.span.end))
                })
                .map(|d| (d.from.unwrap_or(d.length), d.length)),
        )
        .chain(
            ISO.iter()
                .flat_map(|re| re.find_iter(text))
                .filter_map(|m| iso_length(m.as_str()))
                .map(single),
        )
        .chain(
            memchr::memchr2_iter(b':', b'=', text.as_bytes())
                .flat_map(|at| at_separator(text, at))
                .flatten()
                .map(single),
        );
    for (low, high) in readings {
        let mut all = true;
        for want in wanted.iter_mut() {
            want.found = want.found || want.grounded_by(low, high);
            all &= want.found;
        }
        if all {
            return;
        }
    }
}
