//! The duration grammar: a part is an amount and a unit ("90 minutes",
//! "deux heures et demie", "1½ hours"), a glued run of units ("1h30",
//! "1h5m30s"), or a fixed phrase ("half an hour", "un quart d'heure", "a
//! fortnight"). Arabic has a pattern of its own ("ساعتين و30 دقيقة", "ربع
//! ساعة"), run only over a text that holds Arabic letters.

use std::cmp::Reverse;
use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use super::Length;
use super::groups::{Groups, scan};
use super::words::{
    AR_ONES, AR_TENS, AR_UNITS, DAY, Form, HOUR, ITALIAN_TENS, ITALIAN_UNITS, MILLISECOND, MINUTE,
    MONTH, QUARTER, SECOND, SINGLES, TENS, WEEK, YEAR,
};
use crate::number::digits::digit_value;
use crate::text::{alternation, char_at, char_before, compile, lookup};

/// A unit of time, smallest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    /// A thousandth of a second.
    Millisecond,
    /// A second.
    Second,
    /// A minute.
    Minute,
    /// An hour.
    Hour,
    /// A day.
    Day,
    /// A week.
    Week,
    /// A month.
    Month,
    /// Three months.
    Quarter,
    /// Twelve months.
    Year,
}

impl Unit {
    /// Every unit, smallest first.
    pub const ALL: [Self; 9] = [
        Self::Millisecond,
        Self::Second,
        Self::Minute,
        Self::Hour,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Quarter,
        Self::Year,
    ];

    /// The length of one unit.
    #[must_use]
    pub const fn one(self) -> Length {
        match self {
            Self::Millisecond => Length::clock(1),
            Self::Second => Length::clock(1_000),
            Self::Minute => Length::clock(60_000),
            Self::Hour => Length::clock(3_600_000),
            Self::Day => Length::clock(86_400_000),
            Self::Week => Length::clock(604_800_000),
            Self::Month => Length::calendar(100),
            Self::Quarter => Length::calendar(300),
            Self::Year => Length::calendar(1_200),
        }
    }

    /// The words the unit is written with, matched case aside.
    #[must_use]
    pub const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Millisecond => MILLISECOND,
            Self::Second => SECOND,
            Self::Minute => MINUTE,
            Self::Hour => HOUR,
            Self::Day => DAY,
            Self::Week => WEEK,
            Self::Month => MONTH,
            Self::Quarter => QUARTER,
            Self::Year => YEAR,
        }
    }

    /// The symbols the unit is written with after a number, matched as
    /// written: lowercase only, so "3D" stays a dimension. A bare `m` is a
    /// metre or a million, so a minute has one only inside a glued run of
    /// units ("1h5m30s").
    #[must_use]
    pub const fn symbols(self) -> &'static [&'static str] {
        match self {
            Self::Millisecond => &["ms"],
            Self::Second => &["s"],
            Self::Hour => &["h"],
            Self::Day => &["d", "j"],
            Self::Week => &["w"],
            Self::Year => &["y"],
            Self::Minute | Self::Month | Self::Quarter => &[],
        }
    }

    /// The name of the unit's group in the part pattern.
    const fn group(self) -> &'static str {
        match self {
            Self::Millisecond => "ms",
            Self::Second => "s",
            Self::Minute => "min",
            Self::Hour => "h",
            Self::Day => "d",
            Self::Week => "w",
            Self::Month => "mo",
            Self::Quarter => "q",
            Self::Year => "y",
        }
    }

    /// `amount` of the unit, rounded down to the millisecond and the
    /// hundredth of a month.
    #[must_use]
    pub(crate) fn times(self, amount: Amount) -> Option<Length> {
        let one = self.one();
        let divisor = 10u64.checked_pow(amount.scale)?;
        Some(Length {
            milliseconds: one
                .milliseconds
                .checked_mul(amount.mantissa)?
                .checked_div(divisor)?,
            centimonths: one
                .centimonths
                .checked_mul(amount.mantissa)?
                .checked_div(divisor)?,
        })
    }
}

/// One part of a duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// Byte range in the scanned text.
    pub span: Range<usize>,
    /// How long it is.
    pub length: Length,
    /// Its largest unit.
    pub largest: Unit,
    /// Its smallest unit.
    pub smallest: Unit,
    /// Whether a reply is held to it: not for a number glued to a bare `s`,
    /// which may be a plural ("the 90s", "100s of users") as well as seconds.
    pub stated: bool,
}

/// A number as digits and how many of them follow the decimal point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amount {
    /// The digits, the decimal point dropped.
    pub mantissa: u64,
    /// How many of the digits are decimals.
    pub scale: u32,
}

impl Amount {
    /// A whole number.
    pub const fn whole(n: u64) -> Self {
        Self {
            mantissa: n,
            scale: 0,
        }
    }

    /// One half.
    const HALF: Self = Self {
        mantissa: 5,
        scale: 1,
    };

    /// One quarter.
    const QUARTER: Self = Self {
        mantissa: 25,
        scale: 2,
    };

    /// Three quarters.
    const THREE_QUARTERS: Self = Self {
        mantissa: 75,
        scale: 2,
    };

    /// The amount written in `text`: ASCII digits with a decimal point or
    /// comma, thousands commas when `thousands`, and a vulgar fraction after
    /// a whole number ("1½").
    pub fn of_digits(text: &str, thousands: bool) -> Option<Self> {
        let mut mantissa: u64 = 0;
        let mut scale: u32 = 0;
        let mut decimal = false;
        let mut vulgar: Option<u64> = None;
        for c in text.chars() {
            if let Some(d) = c.to_digit(10) {
                mantissa = mantissa.checked_mul(10)?.checked_add(u64::from(d))?;
                if decimal {
                    scale = scale.checked_add(1)?;
                }
            } else if c == '.' || (c == ',' && !thousands) {
                decimal = true;
            } else if c != ',' {
                vulgar = Some(match c {
                    '½' => 50,
                    '¼' => 25,
                    '¾' => 75,
                    _ => return None,
                });
            }
        }
        match vulgar {
            None => Some(Self { mantissa, scale }),
            Some(hundredths) if scale == 0 => Some(Self {
                mantissa: mantissa.checked_mul(100)?.checked_add(hundredths)?,
                scale: 2,
            }),
            Some(_) => None,
        }
    }
}

/// A case-insensitive pattern of the words in `table`, those with a value in
/// `values` only.
fn words_of(table: &'static [(&'static str, u64)], values: Range<u64>) -> String {
    alternation(
        table
            .iter()
            .filter(move |&&(_, v)| values.contains(&v))
            .map(|&(w, _)| w),
    )
}

/// The spaces a number and its unit may stand apart by.
const SPACE: &str = r"[ \x{A0}\x{202F}\x{2009}]";

/// A glued run of units, two at least: hours then minutes and seconds
/// (`1h5m30s`, `1h30m`, or `1h30`, the minutes' `m` left out), or minutes
/// and seconds (`5m30s`).
const GLUED: &str =
    "[0-9]{1,4}h(?:[0-5]?[0-9]m(?:[0-5]?[0-9]s)?|[0-5][0-9])|[0-9]{1,4}m[0-5]?[0-9]s";

/// The pattern of a part: a glued run of units, or an amount (in digits, or
/// in words followed by a space), a unit, and an optional "and a half".
fn part_pattern() -> String {
    let words = words_of(SINGLES, 0..30);
    let tens = words_of(TENS, 10..100);
    let ones = words_of(SINGLES, 1..20);
    let it_tens = words_of(ITALIAN_TENS, 20..100);
    let it_ones = words_of(ITALIAN_UNITS, 1..10);
    let units = Unit::ALL
        .iter()
        .map(|&unit| {
            let names = alternation(unit.words().iter().copied());
            let symbols = unit.symbols().join("|");
            let symbols = if symbols.is_empty() {
                symbols
            } else {
                format!("|{symbols}")
            };
            format!("(?P<{}>(?i:{names}){symbols})", unit.group())
        })
        .collect::<Vec<_>>()
        .join("|");
    let digits = r"(?P<thousands>[0-9]{1,3}(?:,[0-9]{3})+(?:\.[0-9]+)?)|(?P<decimal>[0-9]+(?:[.,][0-9]+)?[½¼¾]?)";
    let worded = format!(
        r"(?i:(?P<couple>(?:a\s+)?couple\s+of|(?:un\s+)?paio\s+di))|(?i:(?P<tens>{tens})(?:\s+(?:y|e|et)\s+|-et-|-|\s+)(?P<ones>{ones}))|(?i:(?P<ittens>{it_tens})[ia]?(?P<itones>{it_ones}))|(?i:(?P<article>an?))|(?i:(?P<word>{words}|{tens}))"
    );
    let and_half = r"\s+and\s+an?\s+half";
    let suffix = r"(?P<half>and\s+an?\s+half|et\s+demie?|y\s+media|e\s+mezz[oa]|e\s+meia)|(?P<quarter>and\s+a\s+quarter|et\s+quart|y\s+cuarto|e\s+un\s+quarto|e\s+um\s+quarto)|(?P<threequarters>e\s+tre\s+quarti)";
    format!(
        r"(?-u:\b)(?:(?P<glued>{GLUED})(?-u:\b)|(?:(?:{digits})(?i:(?P<andhalf>{and_half}))?(?P<gap>{SPACE}|-)?|(?:{worded})(?i:(?P<wordhalf>{and_half}))?(?:{SPACE}|-)|(?i:(?P<elided>un['’])|(?P<coupled>(?:un\s+)?paio\s+d['’])))(?:{units})(?-u:\b)(?:\s+(?i:{suffix})(?-u:\b))?)"
    )
}

/// The pattern of a fixed phrase.
const SPECIAL: &str = r"(?i)(?-u:\b)(?:(?P<half>half\s+an?\s+hour|an?\s+half[\s-]hour|(?:une\s+)?demie?[\s-]heure|(?:una\s+)?media\s+hora|(?:una\s+)?mezz['’]?ora|(?:uma\s+)?meia[\s-]hora|1/2\s?h(?:our|eure)?)|(?P<quarter>(?:a\s+)?quarter(?:\s+of\s+an\s+|-)hour|(?:un\s+)?quart\s+d['’]heure|(?:un\s+)?cuarto\s+de\s+hora|(?:un\s+)?quarto\s+d['’]ora|(?:um\s+)?quarto\s+de\s+hora|1/4\s?h(?:our|eure)?)|(?P<threequarters>three[\s-]quarters\s+of\s+an\s+hour|trois\s+quarts\s+d['’]heure|tres\s+cuartos\s+de\s+hora|tre\s+quarti\s+d['’]ora|tr[eê]s\s+quartos\s+de\s+hora|3/4\s?h(?:our|eure)?)|(?P<fortnight>(?:a\s+|one\s+)?fortnight))(?-u:\b)";

/// A digit as Arabic texts write them: Western, Arabic-Indic or extended
/// Arabic-Indic.
const AR_DIGIT: &str = "[0-9\u{660}-\u{669}\u{6F0}-\u{6F9}]";

/// The pattern of an Arabic part: an optional amount (digits, a number word
/// up to ninety-nine, or a fraction), a unit word, then `واحد` ("one") after
/// it, or a half, a quarter or a third added (`و`) or taken away (`إلا`).
fn arabic_pattern() -> String {
    let d = AR_DIGIT;
    let ones = words_of(AR_ONES, 1..10);
    let single = words_of(AR_ONES, 3..11);
    let tens = words_of(AR_TENS, 20..100);
    let units = alternation(AR_UNITS.iter().map(|&(w, _, _)| w));
    let fractions = "نصف|نص|ربع|ثلث";
    format!(
        r"(?:(?P<digits>{d}+(?:[.,٫]{d}+)?)\s*|(?P<cu>{ones})\s+و\s*(?P<ct>{tens})\s+|(?P<tu>{ones})\s+(?:عشرة|عشره|عشر)\s+|(?P<tens>{tens})\s+|(?P<single>{single})\s+|(?P<frac>ثلاثة\s+أرباع|ثلاث\s+أرباع|ثلاثة\s+ارباع|ثلاث\s+ارباع|{fractions})\s+)?(?P<unit>{units})(?:\s+(?P<one>واحدة|واحده|واحد))?(?:\s*(?P<plus>و\s*(?:{fractions}))|\s+(?P<minus>(?:إلا|الا)\s+(?:ربع|ثلث)))?"
    )
}

/// The part pattern, compiled once.
static PART: LazyLock<Option<Regex>> = LazyLock::new(|| compile(&part_pattern()));

/// The Arabic part pattern, compiled once.
static ARABIC: LazyLock<Option<Regex>> = LazyLock::new(|| compile(&arabic_pattern()));

/// The fixed phrases, compiled once.
static PHRASE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(SPECIAL));

/// Whether a part may start at byte `at`: not inside a word or a number,
/// nor after a fraction's slash.
fn starts_clean(text: &str, at: usize) -> bool {
    !char_before(text, at)
        .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ',' | '/'))
}

/// Whether a part may end at byte `at`: not before a letter or a digit of
/// any script, which the ASCII boundary in the pattern lets through.
fn ends_clean(text: &str, at: usize) -> bool {
    !char_at(text, at).is_some_and(char::is_alphanumeric)
}

/// The amount a part's captures write.
fn amount(caps: &Groups<'_, '_>) -> Option<Amount> {
    if let Some(m) = caps.get("thousands") {
        return Amount::of_digits(m.as_str(), true);
    }
    if let Some(m) = caps.get("decimal") {
        return Amount::of_digits(m.as_str(), false);
    }
    let value = if caps.get("couple").is_some() || caps.get("coupled").is_some() {
        2
    } else if caps.get("article").is_some() || caps.get("elided").is_some() {
        1
    } else if let (Some(tens), Some(ones)) = (caps.get("tens"), caps.get("ones")) {
        lookup(TENS, tens.as_str())?.checked_add(lookup(SINGLES, ones.as_str())?)?
    } else if let (Some(tens), Some(ones)) = (caps.get("ittens"), caps.get("itones")) {
        lookup(ITALIAN_TENS, tens.as_str())?.checked_add(lookup(ITALIAN_UNITS, ones.as_str())?)?
    } else {
        let word = caps.get("word")?.as_str();
        lookup(SINGLES, word).or_else(|| lookup(TENS, word))?
    };
    Some(Amount::whole(value))
}

/// The part a glued run of units writes ("1h5m30s"): each number takes the
/// unit after it, and a last number with none is minutes after an hour.
fn read_glued(span: Range<usize>, written: &str) -> Option<Part> {
    let mut length = Length::default();
    let mut largest: Option<Unit> = None;
    let mut smallest: Option<Unit> = None;
    let mut number: u64 = 0;
    let mut add = |unit: Unit, count: u64| -> Option<()> {
        length = length.checked_add(unit.times(Amount::whole(count))?)?;
        largest.get_or_insert(unit);
        smallest = Some(unit);
        Some(())
    };
    for c in written.chars() {
        if let Some(d) = c.to_digit(10) {
            number = number.checked_mul(10)?.checked_add(u64::from(d))?;
        } else {
            let unit = match c {
                'h' => Unit::Hour,
                'm' => Unit::Minute,
                _ => Unit::Second,
            };
            add(unit, number)?;
            number = 0;
        }
    }
    if written.ends_with(|c: char| c.is_ascii_digit()) {
        add(Unit::Minute, number)?;
    }
    Some(Part {
        span,
        length,
        largest: largest?,
        smallest: smallest?,
        stated: true,
    })
}

/// The part a match of the part pattern writes, if it is one.
fn read_part(text: &str, caps: &Groups<'_, '_>) -> Option<Part> {
    let whole = caps.whole()?;
    if !starts_clean(text, whole.start()) || !ends_clean(text, whole.end()) {
        return None;
    }
    if let Some(glued) = caps.get("glued") {
        return read_glued(whole.range(), glued.as_str());
    }
    let count = amount(caps)?;
    let (unit, written) = Unit::ALL
        .into_iter()
        .find_map(|u| caps.get(u.group()).map(|m| (u, m)))?;
    let extra = [
        ("andhalf", Amount::HALF),
        ("wordhalf", Amount::HALF),
        ("half", Amount::HALF),
        ("quarter", Amount::QUARTER),
        ("threequarters", Amount::THREE_QUARTERS),
    ]
    .into_iter()
    .find(|&(name, _)| caps.get(name).is_some());
    if caps.get("article").is_some() && extra.is_none() {
        return None;
    }
    let mut length = unit.times(count)?;
    if let Some((_, fraction)) = extra {
        length = length.checked_add(unit.times(fraction)?)?;
    }
    let glued_s =
        written.as_str() == "s" && caps.get("decimal").is_some() && caps.get("gap").is_none();
    Some(Part {
        span: whole.range(),
        length,
        largest: unit,
        smallest: unit,
        stated: !glued_s,
    })
}

/// The part a fixed phrase writes.
fn read_phrase(text: &str, caps: &Groups<'_, '_>) -> Option<Part> {
    let whole = caps.whole()?;
    if !starts_clean(text, whole.start()) || !ends_clean(text, whole.end()) {
        return None;
    }
    let (unit, count) = if caps.get("half").is_some() {
        (Unit::Minute, 30)
    } else if caps.get("quarter").is_some() {
        (Unit::Minute, 15)
    } else if caps.get("threequarters").is_some() {
        (Unit::Minute, 45)
    } else {
        (Unit::Week, 2)
    };
    Some(Part {
        span: whole.range(),
        length: unit.times(Amount::whole(count))?,
        largest: unit,
        smallest: unit,
        stated: true,
    })
}

/// What an Arabic unit word follows.
enum Before<'t> {
    /// No amount.
    Nothing,
    /// A number.
    Count(Amount),
    /// A fraction word ("نصف", "ربع", "ثلاثة أرباع").
    Fraction(&'t str),
}

/// What stands before the unit of an Arabic part's captures.
fn arabic_before<'t>(caps: &Groups<'_, 't>) -> Option<Before<'t>> {
    let count = |n: u64| Some(Before::Count(Amount::whole(n)));
    if let Some(m) = caps.get("digits") {
        return Some(Before::Count(arabic_digits(m.as_str())?));
    }
    if let (Some(ones), Some(tens)) = (caps.get("cu"), caps.get("ct")) {
        return count(
            lookup(AR_ONES, ones.as_str())?.checked_add(lookup(AR_TENS, tens.as_str())?)?,
        );
    }
    if let Some(ones) = caps.get("tu") {
        return count(lookup(AR_ONES, ones.as_str())?.checked_add(10)?);
    }
    if let Some(tens) = caps.get("tens") {
        return count(lookup(AR_TENS, tens.as_str())?);
    }
    if let Some(single) = caps.get("single") {
        return count(lookup(AR_ONES, single.as_str())?);
    }
    Some(
        caps.get("frac")
            .map_or(Before::Nothing, |f| Before::Fraction(f.as_str())),
    )
}

/// An amount in Western or Arabic-Indic digits, its decimal mark a dot, a
/// comma or the Arabic decimal separator.
fn arabic_digits(text: &str) -> Option<Amount> {
    let mut mantissa: u64 = 0;
    let mut scale: u32 = 0;
    let mut decimal = false;
    for c in text.chars() {
        if let Some(d) = digit_value(c) {
            mantissa = mantissa.checked_mul(10)?.checked_add(u64::from(d))?;
            if decimal {
                scale = scale.checked_add(1)?;
            }
        } else {
            decimal = true;
        }
    }
    Some(Amount { mantissa, scale })
}

/// `unit` times the fraction `word` names: a half, a quarter, three
/// quarters, or a third, which only a unit of whole milliseconds divides
/// into exactly.
fn arabic_fraction(unit: Unit, word: &str) -> Option<Length> {
    let word = word.trim_start_matches(|c: char| c == 'و' || c.is_whitespace());
    if word.starts_with("ثلاث") {
        return unit.times(Amount::THREE_QUARTERS);
    }
    match word {
        "نصف" | "نص" => unit.times(Amount::HALF),
        "ربع" => unit.times(Amount::QUARTER),
        "ثلث" => {
            let one = unit.one();
            (one.centimonths == 0 && one.milliseconds.is_multiple_of(3))
                .then(|| Length::clock(one.milliseconds / 3))
        }
        _ => None,
    }
}

/// Whether an Arabic part may start at byte `at`: as a part elsewhere may,
/// or after a one-letter proclitic written onto it ("و", "ب", "ل", "ف",
/// "ك": "و30 دقيقة", "بساعتين").
fn arabic_starts_clean(text: &str, at: usize) -> bool {
    match char_before(text, at) {
        Some(c @ ('و' | 'ب' | 'ل' | 'ف' | 'ك')) => {
            starts_clean(text, at.saturating_sub(c.len_utf8()))
        }
        _ => starts_clean(text, at),
    }
}

/// Whether, after byte `end`, `و` joins another amount: a digit or a number
/// word ("ساعة و30 دقيقة", "ساعة وعشر دقائق"), so the lone unit before it
/// counts one.
fn joined_after(text: &str, end: usize) -> bool {
    let Some(rest) = text
        .get(end..)
        .and_then(|t| t.trim_start().strip_prefix('و'))
        .map(str::trim_start)
    else {
        return false;
    };
    if rest.chars().next().and_then(digit_value).is_some() {
        return true;
    }
    let word_end = rest
        .char_indices()
        .find(|&(_, c)| !c.is_alphabetic())
        .map_or(rest.len(), |(i, _)| i);
    rest.get(..word_end)
        .is_some_and(|w| lookup(AR_ONES, w).is_some() || lookup(AR_TENS, w).is_some())
}

/// The part a match of the Arabic pattern writes, if it is one. A unit alone
/// counts one only with `واحد`, a fraction after it, or another amount
/// joined to it, since "يوم" and "ساعة" alone are as often "the day of" and
/// "the clock"; a dual counts two by itself.
fn read_arabic(text: &str, caps: &Groups<'_, '_>) -> Option<Part> {
    let whole = caps.whole()?;
    if !arabic_starts_clean(text, whole.start()) {
        return None;
    }
    let unit_word = caps.get("unit")?;
    let &(_, unit, form) = AR_UNITS
        .iter()
        .find(|&&(w, _, _)| w == unit_word.as_str())?;
    let one = caps.get("one");
    let mut plus = caps.get("plus");
    let mut minus = caps.get("minus");
    let mut end = whole.end();
    if !ends_clean(text, end) {
        // A fraction glued to more letters is not one: read the part without it.
        plus.take().or_else(|| minus.take())?;
        end = one.map_or_else(|| unit_word.end(), |o| o.end());
        if !ends_clean(text, end) {
            return None;
        }
    }
    let suffixed = plus.is_some() || minus.is_some();
    let base = match (arabic_before(caps)?, form) {
        (Before::Count(count), Form::Singular | Form::Plural) if one.is_none() => {
            unit.times(count)?
        }
        (Before::Fraction(word), Form::Singular | Form::Definite) if one.is_none() && !suffixed => {
            arabic_fraction(unit, word)?
        }
        (Before::Nothing, Form::Dual) if one.is_none() => unit.times(Amount::whole(2))?,
        (Before::Nothing, Form::Singular)
            if one.is_some() || suffixed || joined_after(text, end) =>
        {
            unit.one()
        }
        _ => return None,
    };
    let length = if let Some(word) = plus {
        base.checked_add(arabic_fraction(unit, word.as_str())?)?
    } else if let Some(word) = minus {
        let fraction = arabic_fraction(unit, word.as_str().split_whitespace().last()?)?;
        Length {
            milliseconds: base.milliseconds.checked_sub(fraction.milliseconds)?,
            centimonths: base.centimonths.checked_sub(fraction.centimonths)?,
        }
    } else {
        base
    };
    Some(Part {
        span: whole.start()..end,
        length,
        largest: unit,
        smallest: unit,
        stated: true,
    })
}

/// Whether `text` holds an Arabic letter or digit: a byte that leads a
/// character from U+0600 to U+067F.
fn has_arabic(text: &str) -> bool {
    memchr::memchr2(0xD8, 0xD9, text.as_bytes()).is_some()
}

/// What turns a pattern's match into a part, if it is one.
type Reader = fn(&str, &Groups<'_, '_>) -> Option<Part>;

/// Every match of `re` in `text` that `read` accepts, a rejected match
/// retried one character on.
fn collect(re: &Regex, text: &str, read: Reader, out: &mut Vec<Part>) {
    scan(re, text, |caps| {
        let Some(whole) = caps.whole() else {
            return usize::MAX;
        };
        read(text, caps).map_or_else(
            || {
                let step = char_at(text, whole.start()).map_or(1, char::len_utf8);
                whole.start().saturating_add(step)
            },
            |part| {
                out.push(part);
                whole.end()
            },
        )
    });
}

/// Every part in `text`, in order, a longer one winning an overlap.
pub fn parts(text: &str) -> Vec<Part> {
    let mut found = Vec::new();
    let readers: [(&Option<Regex>, Reader, bool); 3] = [
        (&PART, read_part, true),
        (&PHRASE, read_phrase, true),
        (&ARABIC, read_arabic, has_arabic(text)),
    ];
    for (re, read, wanted) in readers {
        if wanted && let Some(re) = re {
            collect(re, text, read, &mut found);
        }
    }
    found.sort_by_key(|p| (p.span.start, Reverse(p.span.end)));
    let mut kept: Vec<Part> = Vec::with_capacity(found.len());
    for part in found {
        if kept.last().is_none_or(|k| k.span.end <= part.span.start) {
            kept.push(part);
        }
    }
    kept
}
