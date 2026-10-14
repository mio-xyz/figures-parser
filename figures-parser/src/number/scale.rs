//! Scale words after a figure's digits: "1.2 million", "12.5K", "3 Mrd.",
//! "1,2万". The words are CLDR's compact number formats and the long forms
//! of the languages the library reads.

use std::ops::Range;

use super::digits::digit_value;
use super::scan::{is_boundary, is_inline_space, is_symbol, marked_after, marked_before};
use crate::text::{char_at, is_letter, lookup};

/// A scale written after a figure's digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// The power of ten it multiplies the figure by.
    pub magnitude: u32,
    /// The byte it ends at, past the sections written after a Han
    /// multiplier ("5000万" in "3億5000万").
    pub end: usize,
    /// What those sections add to the scaled digits; 0 without them. The
    /// sections stay below the head's multiplier, so what fits a `u128`
    /// with the head fits here, and the rest is dropped with its head.
    pub rest: u64,
}

/// When an abbreviation is read as a scale.
#[derive(Debug, Clone, Copy)]
enum Needs {
    /// Wherever it stands.
    Nothing,
    /// Glued to the digits anywhere ("12K"), after a space only beside a
    /// currency ("12 k€"), where a lone letter is otherwise a unit.
    CurrencyWhenSpaced,
    /// Only beside a currency, since the letters also name a unit or a size
    /// ("5m" of minutes or metres, "7B" parameters, "5mm"): "$5m", "€3B".
    Currency,
    /// Before the dot German abbreviates with ("2,5 Mio."), or beside a
    /// currency, since the bare letters are also a name ("2 Mio messages").
    DotOrCurrency,
}

/// The longest ASCII long form, in bytes ("milliarden").
const LONGEST_WORD: usize = 10;

/// The power of ten a long form stands for, read after a space, its ASCII
/// letters already lowercased. "billion" is read on the short scale English
/// uses; French and German write a thousand million as "milliard" and
/// "Milliarde".
const fn ascii_word(lower: &[u8]) -> Option<u32> {
    let magnitude = match lower {
        b"hundred" => 2,
        b"thousand" | b"mille" | b"tausend" | b"mil" | b"mila" => 3,
        b"lakh" | b"lakhs" => 5,
        b"million" | b"millions" | b"millionen" | b"millon" | b"millones" | b"milione"
        | b"milioni" | b"milhao" | b"milhoes" => 6,
        b"crore" | b"crores" => 7,
        b"billion" | b"milliard" | b"milliards" | b"milliarde" | b"milliarden" | b"miliardo"
        | b"miliardi" | b"bilhao" | b"bilhoes" => 9,
        b"trillion" | b"billionen" | b"billones" => 12,
        _ => return None,
    };
    Some(magnitude)
}

/// The long forms with a letter outside ASCII, read case aside.
const ACCENTED_WORDS: &[(&str, u32)] = &[
    ("millón", 6),
    ("milhão", 6),
    ("milhões", 6),
    ("bilhão", 9),
    ("bilhões", 9),
    ("billón", 12),
    ("bilião", 12),
    ("biliões", 12),
];

/// What Spanish and Portuguese write after "mil" for a thousand million.
const THOUSAND_MILLION: &[(&str, u32)] = &[("millones", 9), ("milhões", 9), ("milhoes", 9)];

/// The power of ten an abbreviation stands for, read with its case, glued
/// to the digits or after a space, and when it reads.
fn abbreviation(word: &str) -> Option<(u32, Needs)> {
    let found = match word {
        "K" | "k" => (3, Needs::CurrencyWhenSpaced),
        "Tsd" | "tsd" => (3, Needs::Nothing),
        "M" => (6, Needs::CurrencyWhenSpaced),
        "MM" | "mm" | "m" | "Mn" | "mn" | "mi" => (6, Needs::Currency),
        "Mio" => (6, Needs::DotOrCurrency),
        "Mln" | "mln" => (6, Needs::Nothing),
        "Cr" | "cr" => (7, Needs::Currency),
        "B" | "b" | "bi" | "Md" => (9, Needs::Currency),
        "bn" | "Bn" | "BN" | "Mds" | "Mrd" | "Mld" | "mld" => (9, Needs::Nothing),
        "T" => (12, Needs::Currency),
        "tn" | "Tn" => (12, Needs::CurrencyWhenSpaced),
        "Bio" => (12, Needs::DotOrCurrency),
        _ => return None,
    };
    Some(found)
}

/// The power of ten a Han multiplier stands for: the small ones (十, 百,
/// 千), which may lead a large one (万, 億, 亿, 兆) as in "5千万".
const fn han_magnitude(c: char) -> Option<(u32, bool)> {
    match c {
        '十' => Some((1, false)),
        '百' => Some((2, false)),
        '千' => Some((3, false)),
        '万' | '萬' => Some((4, true)),
        '億' | '亿' => Some((8, true)),
        '兆' => Some((12, true)),
        _ => None,
    }
}

/// The Han multipliers glued after a figure's digits: a small one (十, 百,
/// 千), if any, then the large ones (万, 億, 兆).
#[derive(Debug, Clone, Copy)]
struct Multipliers {
    /// The power of the small one; 0 without one.
    small: u32,
    /// The powers of the large ones added; 0 without one.
    large: u32,
    /// The byte they end at.
    end: usize,
}

impl Multipliers {
    /// The power of ten they multiply by together.
    const fn magnitude(&self) -> u32 {
        self.small.saturating_add(self.large)
    }
}

/// The Han multipliers glued after the digits `digits` ("1.2万", "3億",
/// "5千万", "2万亿"), and, after digits alone, the sections written after
/// them ("3億5000万", "1億2345万6789", "1千200万"). The sections are kept
/// only when the whole fits the integer type the readings add them in.
fn han_scale(text: &str, digits: Range<usize>) -> Option<Scale> {
    let head = han_multipliers(text, digits.end)?;
    let plain = Scale {
        magnitude: head.magnitude(),
        end: head.end,
        rest: 0,
    };
    let whole = text
        .get(digits.clone())
        .is_some_and(|run| run.chars().all(|c| digit_value(c).is_some()));
    let Some((rest, lift, end)) = whole.then(|| han_sections(text, head)).flatten() else {
        return Some(plain);
    };
    let magnitude = head.magnitude().saturating_add(lift);
    let fits = section_digits(text, digits.start)
        .and_then(|(value, ..)| 10_u128.checked_pow(magnitude)?.checked_mul(value))
        .and_then(|value| value.checked_add(rest))
        .is_some();
    match u64::try_from(rest) {
        Ok(rest) if fits => Some(Scale {
            magnitude,
            end,
            rest,
        }),
        _ => Some(plain),
    }
}

/// The Han multipliers glued after byte `end`: the small one, if any, then
/// the large ones.
fn han_multipliers(text: &str, end: usize) -> Option<Multipliers> {
    let mut found = Multipliers {
        small: 0,
        large: 0,
        end,
    };
    while let Some(c) = char_at(text, found.end) {
        let Some((power, large)) = han_magnitude(c) else {
            break;
        };
        if large {
            found.large = found.large.saturating_add(power);
        } else if found.magnitude() > 0 {
            break;
        } else {
            found.small = power;
        }
        found.end = found.end.saturating_add(c.len_utf8());
    }
    (found.magnitude() > 0).then_some(found)
}

/// The sections written after the multipliers `head` of whole digits: runs
/// of digits, each with multipliers below the one before it, and at the end
/// a run of digits alone of two digits or more ("2万5000", "1億2345万6789"),
/// since "3万5" is spoken shorthand for 35,000. A section after 億 or 兆
/// that stops short of the myriad is such shorthand too, and fills the
/// place a text writes through 万: "3億5千" and "3億5000" are 3億5000万. A
/// large multiplier after small ones multiplies all before it: "1千200万" is
/// 1,200万. Returns what the sections add to the scaled digits, the power
/// that multiplier adds to the head's, and the byte the sections end at;
/// None without one.
fn han_sections(text: &str, head: Multipliers) -> Option<(u128, u32, usize)> {
    let pow = |power: u32| 10_u128.checked_pow(power);
    let mut added = 0_u128;
    let mut lift = 0_u32;
    let mut at = head.end;
    let mut power = head.magnitude();
    let mut large = head.large;
    while let Some((value, digits, digits_end)) = section_digits(text, at) {
        let found = han_multipliers(text, digits_end);
        // The places below the last large multiplier that a section short
        // of the myriad leaves unwritten.
        let short = large.saturating_sub(4);
        if let Some(m) = found
            && m.large > 0
            && large == 0
        {
            let group = pow(m.small)
                .and_then(|scale| value.checked_mul(scale))
                .filter(|&group| pow(power).is_some_and(|limit| group < limit));
            let Some(lifted) = group
                .and_then(|group| added.checked_add(group))
                .and_then(|group| pow(m.large)?.checked_mul(group))
            else {
                break;
            };
            added = lifted;
            lift = m.large;
            power = m.large;
            large = m.large;
            at = m.end;
            continue;
        }
        let section = match found {
            Some(m) if m.magnitude() < power && (m.large > 0 || short == 0) => pow(m.magnitude())
                .and_then(|scale| value.checked_mul(scale))
                .map(|section| (section, m.magnitude(), m.large, m.end)),
            Some(m) if m.large == 0 && m.magnitude() < power => pow(m.small.saturating_add(short))
                .and_then(|scale| value.checked_mul(scale))
                .map(|section| (section, 0, large, m.end)),
            Some(_) => None,
            None => (digits >= 2 && !continues_figure(text, digits_end))
                .then(|| pow(short).and_then(|scale| value.checked_mul(scale)))
                .flatten()
                .map(|section| (section, 0, large, digits_end)),
        };
        let Some((section, next_power, next_large, end)) = section else {
            break;
        };
        let Some(sum) = pow(power)
            .filter(|&limit| section < limit)
            .and_then(|_| added.checked_add(section))
        else {
            break;
        };
        added = sum;
        at = end;
        power = next_power;
        large = next_large;
        if power == 0 {
            break;
        }
    }
    (at > head.end).then_some((added, lift, at))
}

/// The digits written from byte `at`, a comma allowed before each group of
/// three ("5,000"): their value, how many there are, and the byte they end
/// at; None when no digit stands there.
fn section_digits(text: &str, mut at: usize) -> Option<(u128, u32, usize)> {
    let mut value = 0_u128;
    let mut digits = 0_u32;
    let mut group = 0_u32;
    while let Some(c) = char_at(text, at) {
        if let Some(digit) = digit_value(c) {
            value = value.checked_mul(10)?.checked_add(u128::from(digit))?;
            digits = digits.saturating_add(1);
            group = group.saturating_add(1);
        } else if matches!(c, ',' | '\u{ff0c}')
            && (1..=3).contains(&group)
            && grouped_after(text, at)
        {
            group = 0;
        } else {
            break;
        }
        at = at.saturating_add(c.len_utf8());
    }
    (digits > 0).then_some((value, digits, at))
}

/// Whether the comma at byte `at` is followed by a group of exactly three
/// digits.
fn grouped_after(text: &str, at: usize) -> bool {
    let mut next = at.saturating_add(char_at(text, at).map_or(1, char::len_utf8));
    for _ in 0..3 {
        match char_at(text, next) {
            Some(c) if digit_value(c).is_some() => next = next.saturating_add(c.len_utf8()),
            _ => return false,
        }
    }
    char_at(text, next).is_none_or(|c| digit_value(c).is_none())
}

/// Whether the figure goes on at byte `end`: a word letter, or a symbol
/// before another digit ("2万5000.5", "2万5000kg").
pub(super) fn continues_figure(text: &str, end: usize) -> bool {
    char_at(text, end).is_some_and(|c| {
        is_boundary(c)
            || (is_symbol(c)
                && char_at(text, end.saturating_add(c.len_utf8()))
                    .is_some_and(|d| digit_value(d).is_some()))
    })
}

/// Whether the bytes at `at` may begin a Latin scale word or abbreviation,
/// read from their first three letters, case aside: one of the letters that
/// stand alone ("K", "m"), one of the pairs ("bn", "Cr"), or the head of a
/// longer word ("mil", "tho", "Mrd").
fn may_start_word(bytes: &[u8], at: usize) -> bool {
    if !bytes.get(at).is_some_and(|&b| is_scale_initial(b)) {
        return false;
    }
    let lower = |i: usize| bytes.get(at.saturating_add(i)).map(u8::to_ascii_lowercase);
    let ends = |i: usize| {
        !bytes
            .get(at.saturating_add(i))
            .is_some_and(u8::is_ascii_alphanumeric)
    };
    match (lower(0), lower(1), lower(2)) {
        (Some(b'k' | b'm' | b'b' | b't'), ..) if ends(1) => true,
        (Some(b'm'), Some(b'm' | b'n' | b'i' | b'd'), _)
        | (Some(b'b'), Some(b'n' | b'i'), _)
        | (Some(b'c'), Some(b'r'), _)
        | (Some(b't'), Some(b'n'), _)
            if ends(2) =>
        {
            true
        }
        (Some(a), Some(b), Some(c)) => matches!(
            &[a, b, c],
            b"hun"
                | b"tho"
                | b"tri"
                | b"tau"
                | b"tsd"
                | b"lak"
                | b"cro"
                | b"mil"
                | b"mio"
                | b"mln"
                | b"mrd"
                | b"mds"
                | b"mld"
                | b"bil"
                | b"bio"
        ),
        _ => false,
    }
}

/// Whether `b` is a letter a Latin scale word or abbreviation begins with,
/// case aside; `| 0x20` lowercases an ASCII letter and maps no other byte
/// onto one.
const fn is_scale_initial(b: u8) -> bool {
    matches!(b | 0x20, b'b' | b'c' | b'h' | b'k' | b'l' | b'm' | b't')
}

/// Whether a scale may stand at byte `end`, glued or after one space, read
/// from the bytes there: most figures are followed by nothing that may begin
/// one and are settled by one byte, inline, without reading a word.
#[inline]
pub(super) fn may_scale(text: &str, end: usize) -> bool {
    let bytes = text.as_bytes();
    let at = if bytes.get(end) == Some(&b' ') {
        end.saturating_add(1)
    } else {
        end
    };
    match bytes.get(at) {
        Some(&b) if b.is_ascii() => is_scale_initial(b) && may_start_word(bytes, at),
        Some(_) => may_scale_unicode(text, end),
        None => false,
    }
}

/// [`may_scale`] where a byte outside ASCII follows: only a Han multiplier
/// or a space within a line may begin a scale there.
fn may_scale_unicode(text: &str, end: usize) -> bool {
    char_at(text, end).is_some_and(|c| {
        han_magnitude(c).is_some()
            || (is_inline_space(c)
                && may_start_word(text.as_bytes(), end.saturating_add(c.len_utf8())))
    })
}

/// The end of the run of letters a word holds from byte `start`, by byte
/// while it is ASCII.
fn word_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut at = start;
    while let Some(&b) = bytes.get(at) {
        if b.is_ascii_alphabetic() {
            at = at.saturating_add(1);
        } else if b.is_ascii() {
            break;
        } else {
            match char_at(text, at) {
                Some(c) if is_letter(c) => at = at.saturating_add(c.len_utf8()),
                _ => break,
            }
        }
    }
    at
}

/// The power of ten a long form stands for, case aside.
fn word_magnitude(word: &str) -> Option<u32> {
    if !word.is_ascii() {
        return lookup(ACCENTED_WORDS, word);
    }
    let mut lower = [0_u8; LONGEST_WORD];
    let lower = lower.get_mut(..word.len())?;
    lower.copy_from_slice(word.as_bytes());
    lower.make_ascii_lowercase();
    ascii_word(lower)
}

/// The scale written after the digits `start..end`, `next` being the
/// character at `end`: glued ("12.5K", "3bn", "1,2万"), or after one space
/// ("1.2 million", "3 Mrd.", "12 k€"). A word reads only when nothing but a
/// letterless character follows it.
pub(super) fn scale_after(text: &str, start: usize, end: usize, next: char) -> Option<Scale> {
    if han_magnitude(next).is_some() {
        return han_scale(text, start..end);
    }
    let spaced = is_inline_space(next);
    let word_start = if spaced {
        end.saturating_add(next.len_utf8())
    } else {
        end
    };
    if !may_start_word(text.as_bytes(), word_start) {
        return None;
    }
    let word_stop = word_end(text, word_start);
    if char_at(text, word_stop).is_some_and(is_boundary) {
        return None;
    }
    let word = text.get(word_start..word_stop)?;
    if spaced && let Some(magnitude) = word_magnitude(word) {
        let millions = (magnitude == 3)
            .then(|| thousand_million(text, word_stop))
            .flatten();
        return Some(millions.unwrap_or(Scale {
            magnitude,
            end: word_stop,
            rest: 0,
        }));
    }
    let (magnitude, needs) = abbreviation(word)?;
    let currency = || marked_before(text, start) || marked_after(text, word_stop);
    let reads = match needs {
        Needs::Nothing => true,
        Needs::CurrencyWhenSpaced => !spaced || currency(),
        Needs::Currency => currency(),
        Needs::DotOrCurrency => text.as_bytes().get(word_stop) == Some(&b'.') || currency(),
    };
    reads.then_some(Scale {
        magnitude,
        end: word_stop,
        rest: 0,
    })
}

/// "mil millones", "mil milhões": a thousand million, when the millions
/// follow the thousand that ends at byte `end`.
fn thousand_million(text: &str, end: usize) -> Option<Scale> {
    let space = char_at(text, end).filter(|&c| is_inline_space(c))?;
    let start = end.saturating_add(space.len_utf8());
    let stop = word_end(text, start);
    Some(Scale {
        magnitude: lookup(THOUSAND_MILLION, text.get(start..stop)?)?,
        end: stop,
        rest: 0,
    })
}
