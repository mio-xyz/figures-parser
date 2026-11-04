//! Scale words after a figure's digits: "1.2 million", "12.5K", "3 Mrd.",
//! "1,2万". The words are CLDR's compact number formats and the long forms
//! of the languages the library reads.

use std::ops::Range;

use super::context::{Unit, currency_word_beside, names_unit};
use super::digits::digit_value;
use super::scan::{is_boundary, is_inline_space, is_symbol, marked_after, marked_before};
use crate::text::{char_at, is_letter, lookup, same_word, starts_with_lowercase};

/// A scale written after a figure's digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// The power of ten it multiplies the figure by.
    pub magnitude: u32,
    /// The other powers the name stands for in a locale that writes it so,
    /// a bit per power ("billion" is also 10¹² in French): read in a corpus,
    /// and in a reply where the figure is written as those locales write it.
    pub also: u16,
    /// The decimal mark the language of the word writes, where a run that
    /// also reads as thousands ("1,250", "8,566") is a decimal before it
    /// too: a comma before "млрд" or where a currency follows ("1,250 M€"),
    /// a dot before "million". None where the word is written alike in
    /// languages that differ ("M", "k").
    pub mark: Option<char>,
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
    /// Glued to the digits unless the words around name the unit, after a
    /// space only beside a currency: "3.2m users", but "took 5m", "5m tall".
    GluedUnless(Unit),
    /// Glued or after a space unless the words around name the unit: "5 mn
    /// people", but "took 5 mn"; "58 md", but "5 md files".
    Unless(Unit),
}

/// The longest long form, in bytes ("milliarden", "miliardów").
const LONGEST_WORD: usize = 10;

/// The power of ten a long form stands for, read after a space, its ASCII
/// letters already lowercased. "billion" is read on the short scale English
/// uses; French and German write a thousand million as "milliard" and
/// "Milliarde".
const fn ascii_word(lower: &[u8]) -> Option<u32> {
    let magnitude = match lower {
        b"hundred" => 2,
        b"thousand" | b"mille" | b"tausend" | b"mil" | b"mila" | b"duizend" | b"tusen"
        | b"ribu" => 3,
        b"lakh" | b"lakhs" => 5,
        b"million" | b"millions" | b"millionen" | b"millon" | b"millones" | b"milione"
        | b"milioni" | b"milhao" | b"milhoes" | b"miljoen" | b"miljon" | b"miljoner"
        | b"milion" | b"miliony" | b"milyon" | b"juta" => 6,
        b"crore" | b"crores" => 7,
        b"billion" | b"milliard" | b"milliards" | b"milliarde" | b"milliarden" | b"miliardo"
        | b"miliardi" | b"bilhao" | b"bilhoes" | b"miljard" | b"miljarder" | b"miliard"
        | b"miliardy" | b"milyar" | b"miliar" => 9,
        b"trillion" | b"billionen" | b"billones" | b"billions" | b"biljoen" | b"biljon"
        | b"biljoner" | b"bilion" | b"biliony" | b"trilyon" | b"triliun" => 12,
        _ => return None,
    };
    Some(magnitude)
}

/// The long forms with a letter outside ASCII, read case aside.
const ACCENTED_WORDS: &[(&str, u32)] = &[
    ("tysiąc", 3),
    ("tysiące", 3),
    ("tysięcy", 3),
    ("nghìn", 3),
    ("millón", 6),
    ("milhão", 6),
    ("milhões", 6),
    ("milionów", 6),
    ("triệu", 6),
    ("bilhão", 9),
    ("bilhões", 9),
    ("miliardów", 9),
    ("tỷ", 9),
    ("billón", 12),
    ("bilião", 12),
    ("biliões", 12),
    ("bilionów", 12),
    ("trilhão", 12),
    ("trilhões", 12),
];

/// The scale words of scripts other than Latin, read from their first
/// letter on, glued to the digits or after a space. Thai writes them glued
/// to the next word as well.
const NON_LATIN_WORDS: &[(&str, u32)] = &[
    ("тыс", 3),
    ("тысяча", 3),
    ("тысячи", 3),
    ("тысяч", 3),
    ("млн", 6),
    ("миллион", 6),
    ("миллиона", 6),
    ("миллионов", 6),
    ("млрд", 9),
    ("миллиард", 9),
    ("миллиарда", 9),
    ("миллиардов", 9),
    ("трлн", 12),
    ("триллион", 12),
    ("триллиона", 12),
    ("триллионов", 12),
    ("ألف", 3),
    ("آلاف", 3),
    ("مليون", 6),
    ("ملايين", 6),
    ("مليار", 9),
    ("ترليون", 12),
    ("هزار", 3),
    ("میلیون", 6),
    ("میلیارد", 9),
    ("تریلیون", 12),
    ("هزارمیلیارد", 12),
    ("אלף", 3),
    ("מיליון", 6),
    ("מיליארד", 9),
    ("טריליון", 12),
    ("\u{939}\u{91c}\u{93c}\u{93e}\u{930}", 3),
    ("\u{939}\u{95b}\u{93e}\u{930}", 3),
    ("लाख", 5),
    ("अरब", 9),
    ("अ\u{970}", 9),
    ("खरब", 11),
    ("ख\u{970}", 11),
    ("พัน", 3),
    ("ล้าน", 6),
    ("พันล้าน", 9),
    ("ล้านล้าน", 12),
];

/// A word after a scale that multiplies it, by the scale it follows: what
/// Spanish and Portuguese write after "mil" for a thousand million, Italian
/// after "mila" for a thousand billion, Indian English after "lakh", and
/// Vietnamese after "nghìn" for a thousand billion.
const COMPOUNDS: &[(u32, &str, u32)] = &[
    (3, "millones", 9),
    (3, "milhões", 9),
    (3, "milhoes", 9),
    (3, "miliardi", 12),
    (5, "crore", 12),
    (5, "crores", 12),
    (3, "tỷ", 12),
];

/// A bit per power of ten, for [`Scale::also`].
const fn powers(list: &[u32]) -> u16 {
    let mut bits = 0_u16;
    let mut rest = list;
    while let [power, tail @ ..] = rest {
        if *power < 16 {
            bits |= 1 << *power;
        }
        rest = tail;
    }
    bits
}

/// The power of ten an abbreviation stands for, when it reads, and the
/// other powers a locale gives it: a single letter read with its case, glued
/// to the digits or after a space ("K" and "k", but "m" is minutes as often
/// as millions), a longer one case aside.
fn abbreviation(word: &str) -> Option<(u32, Needs, u16)> {
    let found = match word {
        "K" | "k" => (3, Needs::GluedUnless(Unit::Resolution), 0),
        "M" => (6, Needs::CurrencyWhenSpaced, powers(&[9])),
        "m" => (6, Needs::GluedUnless(Unit::TimeOrLength), powers(&[9])),
        "B" | "b" => (9, Needs::CurrencyWhenSpaced, powers(&[3, 12])),
        "T" => (12, Needs::CurrencyWhenSpaced, powers(&[9])),
        "t" => (12, Needs::GluedUnless(Unit::Weight), powers(&[9])),
        "L" | "l" => (5, Needs::GluedUnless(Unit::Volume), 0),
        "N" | "n" => (3, Needs::Currency, 0),
        "Cr" => (7, Needs::Nothing, 0),
        "Mn" => (6, Needs::Nothing, 0),
        "Md" => (9, Needs::Nothing, 0),
        _ => return longer_abbreviation(word),
    };
    Some(found)
}

/// [`abbreviation`] for a name of two letters or more, case aside: "mm" of
/// millimetres and "CR" of a credit read only beside a currency, and those
/// that are also a unit, a title or a word ("mn" of French minutes, "mi" of
/// miles, "md" of a file, "tn" of tonnes, "cr" of course credits, "Mr", "5
/// bin bags", "Bio" eggs) read unless the words around say so.
fn longer_abbreviation(word: &str) -> Option<(u32, Needs, u16)> {
    let mut lower = [0_u8; 3];
    let lower = lower.get_mut(..word.len()).filter(|l| l.len() >= 2)?;
    lower.copy_from_slice(word.as_bytes());
    lower.make_ascii_lowercase();
    let found = match &*lower {
        b"tsd" | b"tys" | b"rb" => (3, Needs::Nothing, 0),
        b"bin" => (3, Needs::Unless(Unit::Bin), 0),
        b"mln" | b"jt" | b"tr" | b"mio" => (6, Needs::Nothing, 0),
        b"mm" => (6, Needs::Currency, 0),
        b"mn" => (6, Needs::Unless(Unit::Time), 0),
        b"mi" => (6, Needs::Unless(Unit::Length), 0),
        // In capitals, accounting's credit after a balance ("3,000 CR").
        b"cr" if word == "CR" => (7, Needs::Currency, 0),
        b"cr" => (7, Needs::Unless(Unit::Credit), 0),
        b"bn" => (9, Needs::Nothing, powers(&[12])),
        b"mds" | b"mrd" | b"mld" | b"bi" => (9, Needs::Nothing, 0),
        b"md" => (9, Needs::Unless(Unit::File), 0),
        b"mr" => (9, Needs::Unless(Unit::Title), 0),
        b"tn" => (12, Needs::Unless(Unit::Weight), powers(&[3])),
        b"bln" | b"lcr" | b"nt" | b"tri" => (12, Needs::Nothing, 0),
        b"bio" => (12, Needs::Unless(Unit::Word), 0),
        _ => return None,
    };
    Some(found)
}

/// The power of ten a Han or Hangul multiplier stands for: the small ones
/// (十, 百, 千, 천), which may lead a large one (万, 億, 亿, 兆, 만, 억, 조)
/// as in "5千万".
const fn han_magnitude(c: char) -> Option<(u32, bool)> {
    match c {
        '十' => Some((1, false)),
        '百' => Some((2, false)),
        '千' | '천' => Some((3, false)),
        '万' | '萬' | '만' => Some((4, true)),
        '億' | '亿' | '억' => Some((8, true)),
        '兆' | '조' => Some((12, true)),
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

/// The Han multipliers from byte `from`, glued after the digits `digits` or
/// one space past them ("1.2万", "3億", "5千万", "2万亿", "200 万"), and, after digits alone, the sections written after
/// them ("3億5000万", "1億2345万6789", "1千200万"). The sections are kept
/// only when the whole fits the integer type the readings add them in.
fn han_scale(text: &str, digits: Range<usize>, from: usize) -> Option<Scale> {
    let head = han_multipliers(text, from)?;
    let plain = Scale {
        magnitude: head.magnitude(),
        also: 0,
        mark: Some('.'),
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
            also: 0,
            mark: Some('.'),
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
        (Some(b'k' | b'm' | b'b' | b't' | b'l' | b'n'), ..) if ends(1) => true,
        (Some(b'm'), Some(b'm' | b'n' | b'i' | b'd' | b'r'), _)
        | (Some(b'b'), Some(b'n' | b'i'), _)
        | (Some(b'c'), Some(b'r'), _)
        | (Some(b't'), Some(b'n' | b'r'), _)
        | (Some(b'r'), Some(b'b'), _)
        | (Some(b'j' | b'n'), Some(b't'), _)
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
                | b"bin"
                | b"lcr"
                | b"bio"
                | b"bln"
                | b"tys"
                | b"dui"
                | b"tus"
                | b"rib"
                | b"jut"
                | b"ngh"
        ),
        _ => false,
    }
}

/// Whether `b` is a letter a Latin scale word or abbreviation begins with,
/// case aside; `| 0x20` lowercases an ASCII letter and maps no other byte
/// onto one.
const fn is_scale_initial(b: u8) -> bool {
    matches!(
        b | 0x20,
        b'b' | b'c' | b'd' | b'h' | b'j' | b'k' | b'l' | b'm' | b'n' | b'r' | b't'
    )
}

/// Whether a scale may stand at byte `end`, glued or after one space, read
/// from the bytes there: most figures are followed by nothing that may begin
/// one and are settled by one byte, inline, without reading a word. With
/// `loose`, an ASCII letter glued to the digits that may begin a name is
/// enough, since the name may run into the word after it.
#[inline]
pub(super) fn may_scale(text: &str, end: usize, loose: bool) -> bool {
    let bytes = text.as_bytes();
    let spaced = bytes.get(end) == Some(&b' ');
    let at = if spaced { end.saturating_add(1) } else { end };
    match bytes.get(at) {
        Some(&b) if b.is_ascii() => {
            is_scale_initial(b) && ((loose && !spaced) || may_start_word(bytes, at))
        }
        Some(_) => may_scale_unicode(text, end),
        None => false,
    }
}

/// [`may_scale`] where a byte outside ASCII follows: a Han or Hangul
/// multiplier, a scale word of another script, or a space within a line
/// before one or before a Latin word.
fn may_scale_unicode(text: &str, end: usize) -> bool {
    char_at(text, end).is_some_and(|c| {
        if han_magnitude(c).is_some() || non_latin_word(text, end).is_some() {
            return true;
        }
        let after = end.saturating_add(c.len_utf8());
        is_inline_space(c)
            && (may_start_word(text.as_bytes(), after)
                || non_latin_word(text, after).is_some()
                || char_at(text, after).is_some_and(|h| han_magnitude(h).is_some()))
    })
}

/// Whether `c` is the first letter of one of [`NON_LATIN_WORDS`], so most
/// words of those scripts are turned away on one character.
const fn is_non_latin_initial(c: char) -> bool {
    matches!(
        c,
        'т' | 'м'
            | 'أ'
            | 'آ'
            | 'م'
            | 'ت'
            | 'ه'
            | 'א'
            | 'מ'
            | 'ט'
            | 'ह'
            | 'ल'
            | 'अ'
            | 'ख'
            | 'พ'
            | 'ล'
    )
}

/// The scale word of another script that starts at byte `at`, the longest
/// one, with the byte it ends at.
fn non_latin_word(text: &str, at: usize) -> Option<(u32, usize)> {
    let rest = text.get(at..)?;
    if !rest.chars().next().is_some_and(is_non_latin_initial) {
        return None;
    }
    NON_LATIN_WORDS
        .iter()
        .filter(|&&(word, _)| rest.starts_with(word))
        .max_by_key(|&&(word, _)| word.len())
        .map(|&(word, magnitude)| (magnitude, at.saturating_add(word.len())))
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

/// The power of ten a long form stands for, case aside, and the other
/// powers a locale gives it.
fn word_magnitude(word: &str) -> Option<(u32, u16)> {
    if !word.is_ascii() {
        return lookup(ACCENTED_WORDS, word).map(|magnitude| (magnitude, 0));
    }
    let mut lower = [0_u8; LONGEST_WORD];
    let lower = lower.get_mut(..word.len())?;
    lower.copy_from_slice(word.as_bytes());
    lower.make_ascii_lowercase();
    let magnitude = ascii_word(lower)?;
    let also = if &*lower == b"billion" {
        powers(&[12])
    } else {
        0
    };
    Some((magnitude, also))
}

/// The power an abbreviation stands for in a corpus, case aside, and the
/// other powers a locale gives it: every one [`abbreviation`] reads.
fn loose_name(word: &str) -> Option<(u32, u16)> {
    let mut lower = [0_u8; 1];
    if let Some(single) = lower.get_mut(..word.len()) {
        single.copy_from_slice(word.as_bytes());
        single.make_ascii_lowercase();
        let found = match &*single {
            b"k" | b"n" => (3, 0),
            b"m" => (6, powers(&[9])),
            b"b" => (9, powers(&[3, 12])),
            b"t" => (12, powers(&[9])),
            b"l" => (5, 0),
            _ => return None,
        };
        return Some(found);
    }
    longer_abbreviation(word).map(|(magnitude, _, also)| (magnitude, also))
}

/// Whether the scale word ending at byte `end` is written as the locales
/// that give its name another power write it: a currency mark after it ("2
/// Md €", "1 billion EUR"), or the "of" of French, Spanish, Italian or
/// Portuguese ("1 billion d'euros", "2 bn de reais"). English puts the
/// currency first ("$2 billion").
fn continental(text: &str, end: usize) -> bool {
    if marked_after(text, end) {
        return true;
    }
    let Some(rest) = text.get(end..) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(is_inline_space) else {
        return false;
    };
    ["d'", "d\u{2019}", "de ", "di "]
        .iter()
        .any(|of| starts_with_lowercase(rest, of))
}

/// Whether the digits `run` end on a comma no grouping writes, a decimal
/// comma ("7,3", "12,50"): English never writes one, so a scale word after
/// it is read as the locales that do read it ("7,3 tn" is the Swedish 7,300).
fn decimal_comma(run: &str) -> bool {
    run.rsplit_once(',').is_some_and(|(_, tail)| {
        tail.chars().count() != 3 && tail.chars().all(|c| c.is_ascii_digit())
    })
}

/// The decimal mark the language of the scale word `word` writes: a point
/// after an English word; a comma after any other Latin word; and none after
/// a letter every locale writes alike ("M", "k", "b"), even with a currency
/// after it: "1,250 M€" is 1,250 million in an English report and 1.25
/// million in a French one, so it is read on its grouping alone.
fn word_mark(word: &str) -> Option<char> {
    const POINT: [&str; 14] = [
        "hundred", "thousand", "million", "billion", "trillion", "lakh", "lakhs", "crore",
        "crores", "bn", "tn", "cr", "lcr", "l",
    ];
    const EITHER: [&str; 6] = ["k", "m", "b", "t", "mm", "mn"];
    if POINT.iter().any(|w| w.eq_ignore_ascii_case(word)) {
        Some('.')
    } else if !EITHER.iter().any(|w| w.eq_ignore_ascii_case(word)) {
        Some(',')
    } else {
        None
    }
}

/// The scale written after the digits `start..end`, `next` being the
/// character at `end`: glued ("12.5K", "3bn", "1,2万", "12million"), or after
/// one space ("1.2 million", "3 Mrd.", "12 k€", "3 млн", "200 万"). A word
/// reads only when nothing but a letterless character follows it.
///
/// With `loose`, as a corpus is read, an abbreviation reads in any case and
/// without the currency or the dot that tells it from a unit in a reply
/// ("5 m", "38.08 Mio"), at every power a locale gives it, and a name glued
/// to the digits may run into the next word, as a scraped page prints
/// "43Kmembers": a reading taken more widely only ever grounds more.
pub(super) fn scale_after(
    text: &str,
    start: usize,
    end: usize,
    next: char,
    loose: bool,
) -> Option<Scale> {
    if han_magnitude(next).is_some() {
        return han_scale(text, start..end, end);
    }
    let spaced = is_inline_space(next);
    let word_start = if spaced {
        end.saturating_add(next.len_utf8())
    } else {
        end
    };
    if spaced && char_at(text, word_start).is_some_and(|c| han_magnitude(c).is_some()) {
        return han_scale(text, start..end, word_start);
    }
    let open_ended = loose && !spaced;
    if let Some((magnitude, stop)) = non_latin_word(text, word_start) {
        let ends = open_ended || !char_at(text, stop).is_some_and(is_boundary);
        // Russian writes a decimal comma; Arabic, Hebrew, Hindi and Thai a point.
        let cyrillic =
            char_at(text, word_start).is_some_and(|c| ('\u{400}'..='\u{4ff}').contains(&c));
        return ends.then_some(Scale {
            magnitude,
            also: 0,
            mark: Some(if cyrillic { ',' } else { '.' }),
            end: stop,
            rest: 0,
        });
    }
    let bytes = text.as_bytes();
    let initial = bytes.get(word_start).is_some_and(|&b| is_scale_initial(b));
    if !(may_start_word(bytes, word_start) || (open_ended && initial)) {
        return None;
    }
    let word_stop = word_end(text, word_start);
    let word = text.get(word_start..word_stop)?;
    if open_ended {
        return glued_name(text, word, word_start);
    }
    if char_at(text, word_stop).is_some_and(is_boundary) {
        return None;
    }
    let continental = loose || continental(text, word_stop) || decimal_comma(text.get(start..end)?);
    let others = |also: u16| if continental { also } else { 0 };
    let mark = word_mark(word);
    if let Some((magnitude, also)) = word_magnitude(word) {
        let plain = Scale {
            magnitude,
            also: others(also),
            mark,
            end: word_stop,
            rest: 0,
        };
        return Some(
            compound(text, word_stop, magnitude).map_or(plain, |scale| Scale { mark, ..scale }),
        );
    }
    let (magnitude, needs, also) = if loose {
        loose_name(word).map(|(magnitude, also)| (magnitude, Needs::Nothing, also))?
    } else {
        abbreviation(word)?
    };
    let currency = || {
        marked_before(text, start)
            || marked_after(text, word_stop)
            || currency_word_beside(text, start, word_stop)
    };
    let unit = |unit: Unit| names_unit(text, start, word_stop, unit);
    let reads = match needs {
        Needs::Nothing => true,
        Needs::CurrencyWhenSpaced => !spaced || currency(),
        Needs::Currency => currency(),
        Needs::GluedUnless(kind) => currency() || (!spaced && !unit(kind)),
        Needs::Unless(kind) => currency() || !unit(kind),
    };
    reads.then_some(Scale {
        magnitude,
        also: others(also),
        mark,
        end: word_stop,
        rest: 0,
    })
}

/// The longest name that starts `word`, a run of letters from byte `start`
/// of `text` glued to the digits: a long form or an abbreviation, case
/// aside, at every power a locale gives it. A long form that is the whole
/// word may lead a compound, as a reply reads it.
fn glued_name(text: &str, word: &str, start: usize) -> Option<Scale> {
    word.char_indices()
        .rev()
        .map(|(at, c)| at.saturating_add(c.len_utf8()))
        .filter(|&len| len <= LONGEST_WORD)
        .find_map(|len| {
            let name = word.get(..len)?;
            let end = start.saturating_add(len);
            if let Some((magnitude, also)) = word_magnitude(name) {
                let whole = (len == word.len())
                    .then(|| compound(text, end, magnitude))
                    .flatten();
                return Some(whole.unwrap_or(Scale {
                    magnitude,
                    also,
                    mark: None,
                    end,
                    rest: 0,
                }));
            }
            let (magnitude, also) = loose_name(name)?;
            Some(Scale {
                magnitude,
                also,
                mark: None,
                end,
                rest: 0,
            })
        })
}

/// The word after a scale that multiplies it ("mil millones", "5 lakh
/// crore"), when one follows the scale of power `magnitude` ending at byte
/// `end`.
fn compound(text: &str, end: usize, magnitude: u32) -> Option<Scale> {
    if !matches!(magnitude, 3 | 5) {
        return None;
    }
    let space = char_at(text, end).filter(|&c| is_inline_space(c))?;
    let start = end.saturating_add(space.len_utf8());
    let stop = word_end(text, start);
    let word = text.get(start..stop)?;
    let &(_, _, total) = COMPOUNDS
        .iter()
        .find(|&&(head, next, _)| head == magnitude && same_word(next, word))?;
    Some(Scale {
        magnitude: total,
        also: 0,
        mark: None,
        end: stop,
        rest: 0,
    })
}
