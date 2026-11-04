//! The duration grammar: a part is an amount and a unit ("90 minutes",
//! "deux heures et demie", "1½ hours"), a glued run of units ("1h30",
//! "1h5m30s"), or a fixed phrase ("half an hour", "un quart d'heure", "a
//! fortnight"). Arabic has a pattern of its own ("ساعتين و30 دقيقة", "ربع
//! ساعة"), run only over a text that holds Arabic letters.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use super::Length;
use super::groups::{Groups, scan};
use super::words::{
    AR_ONES, AR_TENS, AR_UNITS, DAY, DOTTED, Form, HOUR, ITALIAN_TENS, ITALIAN_UNITS, MILLISECOND,
    MINUTE, MONTH, NAMED, QUARTER, SECOND, SINGLES, TENS, WEEK, YEAR,
};
use crate::number::digits::digit_value;
use crate::text::{
    alternation, char_at, char_before, compile, is_letter, is_unspaced_script, lookup, same_word,
};

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
    /// hundredth of a month. The product is taken wide, so a float a tool
    /// prints to 17 digits ("237.39999999999998") reads, and a length past
    /// what a `u64` of milliseconds holds is held at its largest value
    /// rather than dropped.
    #[must_use]
    pub(crate) fn times(self, amount: Amount) -> Option<Length> {
        let one = self.one();
        let divisor = 10u128.checked_pow(amount.scale)?;
        let scaled = |of: u64| {
            u128::from(of)
                .checked_mul(u128::from(amount.mantissa))
                .and_then(|n| n.checked_div(divisor))
                .map(|n| u64::try_from(n).unwrap_or(u64::MAX))
        };
        Some(Length {
            milliseconds: scaled(one.milliseconds)?,
            centimonths: scaled(one.centimonths)?,
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
    /// The step it is written to: its smallest unit, a decimal place of it
    /// ("1.5 hours" is written to the tenth of an hour), or the fraction
    /// added to it ("an hour and a half" is written to the half hour).
    pub place: Length,
    /// The other reading of its digits, with the step it is written to, when
    /// a lone dot or comma before three digits may group them or end the
    /// whole number ("1,250 secondes" as 1.25 seconds, "1.250 Sekunden" as
    /// 1,250).
    pub loose: Option<(Length, Length)>,
    /// Whether a reply is held to it: not for a number beside a one-letter
    /// symbol, which may be a plural ("the 90s"), a time of day ("15h",
    /// "15h30") or a size as well as a length, nor for a day or a longer unit,
    /// which a window or a plan is written with as well.
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
            if let Some(d) = digit_value(c) {
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

/// Whether `c` groups digits or ends a whole number in a printed number, in
/// any of the figure reader's locales: a comma, a dot, the two apostrophes
/// Swiss German groups with, the spaces French and Russian group with, the
/// Arabic thousands and decimal signs, and the fullwidth comma.
pub const fn is_number_symbol(c: char) -> bool {
    matches!(
        c,
        ',' | '.'
            | '\''
            | '\u{2019}'
            | ' '
            | '\u{a0}'
            | '\u{202f}'
            | '\u{2009}'
            | '\u{66c}'
            | '\u{66b}'
            | '\u{ff0c}'
    )
}

/// Whether `c` may end a whole number before its decimals: a dot, a comma or
/// the Arabic decimal sign; the other symbols only ever group.
const fn is_decimal_symbol(c: char) -> bool {
    matches!(c, '.' | ',' | '\u{66b}')
}

/// Whether `groups`, a printed number's digit groups in order, group
/// thousands: a lead of one to three digits that is not zero, then groups of
/// three; or the Indian lakh notation, a lead of one or two, groups of two
/// and a last group of three ("12,50,000").
fn is_thousands<'a>(mut groups: impl Iterator<Item = &'a str>) -> bool {
    let Some(lead) = groups.next() else {
        return false;
    };
    let lead_digits = lead.chars().count();
    if !(1..=3).contains(&lead_digits) || lead.chars().all(|c| digit_value(c) == Some(0)) {
        return false;
    }
    let (mut rest, mut last) = (0_usize, 0_usize);
    let (mut threes, mut twos) = (true, true);
    for group in groups {
        if rest > 0 {
            threes &= last == 3;
            twos &= last == 2;
        }
        last = group.chars().count();
        rest = rest.saturating_add(1);
    }
    rest > 0 && last == 3 && (threes || (lead_digits <= 2 && twos))
}

impl Amount {
    /// The digits of `text` in any script, every other character skipped, a
    /// whole number.
    fn of_any_digits(text: &str) -> Option<Self> {
        text.chars()
            .filter_map(digit_value)
            .try_fold(0_u64, |n, d| n.checked_mul(10)?.checked_add(u64::from(d)))
            .map(Self::whole)
    }

    /// The readings of a printed number, digits of any script joined by
    /// [`is_number_symbol`]s: the one it is held to, and the other one when a
    /// lone dot or comma before three digits may group them or end the whole
    /// number ("1.250" is 1,250 in Berlin and 1.25 in Boston). Groups joined
    /// by one repeated symbol that fit thousands grouping are a whole number;
    /// a last decimal symbol after one group, or after a whole grouped by
    /// another symbol ("1.234.567,89"), ends the whole number. Groups that fit
    /// neither are no one number.
    pub(crate) fn of_printed(text: &str) -> Option<(Self, Option<Self>)> {
        let mut symbols = text.chars().filter(|&c| is_number_symbol(c));
        let Some(first) = symbols.next() else {
            return Some((Self::of_any_digits(text)?, None));
        };
        let count = symbols.count().saturating_add(1);
        let (whole, fraction) = text.rsplit_once(is_number_symbol)?;
        let last = text.get(whole.len()..)?.chars().next()?;
        let thousands = text
            .chars()
            .filter(|&c| is_number_symbol(c))
            .all(|c| c == last)
            && is_thousands(text.split(is_number_symbol));
        let decimal = is_decimal_symbol(last)
            && (count == 1
                || (first != last
                    && whole
                        .chars()
                        .filter(|&c| is_number_symbol(c))
                        .all(|c| c == first)
                    && is_thousands(whole.split(is_number_symbol))));
        let decimal = decimal
            .then(|| {
                let Self { mantissa, .. } = Self::of_any_digits(text)?;
                let scale = u32::try_from(fraction.chars().count()).ok()?;
                Some(Self { mantissa, scale })
            })
            .flatten();
        if thousands {
            Some((Self::of_any_digits(text)?, decimal))
        } else {
            Some((decimal?, None))
        }
    }
}

/// The step `amount` of `unit` is written to: the unit, or the decimal place
/// its digits stop at; a half or a quarter when the fraction is one, written
/// as a vulgar fraction ("1½") or in digits ("1.5", "2.25"), since "1.5
/// hours" is "an hour and a half" spelled in digits.
fn digits_place(unit: Unit, amount: Amount, written: &str) -> Option<Length> {
    let hundredths = match amount.scale {
        1 => amount.mantissa.checked_rem(10)?.checked_mul(10)?,
        2 => amount.mantissa.checked_rem(100)?,
        _ => 0,
    };
    if written.contains('½') || hundredths == 50 {
        unit.times(Amount::HALF)
    } else if written.contains(['¼', '¾']) || matches!(hundredths, 25 | 75) {
        unit.times(Amount::QUARTER)
    } else {
        unit.times(Amount {
            mantissa: 1,
            scale: amount.scale,
        })
    }
}

/// The length `count` of `unit` reads as, with the step it is written to.
fn reading(unit: Unit, count: Amount, written: &str) -> Option<(Length, Length)> {
    Some((unit.times(count)?, digits_place(unit, count, written)?))
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

/// The symbols a printed number's digit groups are joined by, as
/// [`is_number_symbol`] reads them.
const GROUP: &str = r"[,.'\x{2019} \x{A0}\x{202F}\x{2009}\x{66C}\x{66B}\x{FF0C}]";

/// The start of a number's digits: an ASCII digit at a word boundary, or a
/// digit of another script, which no ASCII boundary precedes.
const LEAD: &str = r"(?:(?-u:\b)[0-9]|[\p{Nd}--[0-9]])\p{Nd}*";

/// A printed number: digits of any script, in groups joined by one of
/// [`is_number_symbol`]'s symbols each.
const PRINTED: &str = r"(?:(?-u:\b)[0-9]|[\p{Nd}--[0-9]])\p{Nd}*(?:[,.'\x{2019} \x{A0}\x{202F}\x{2009}\x{66C}\x{66B}\x{FF0C}]\p{Nd}+)*";

/// The spaces a number and its unit may stand apart by.
const SPACE: &str = r"[ \x{A0}\x{202F}\x{2009}]";

/// A glued run of units, two at least: hours then minutes and seconds
/// (`1h5m30s`, `1h30m`, or `1h30`, the minutes' `m` left out), or minutes
/// and seconds (`5m30s`).
const GLUED: &str =
    "[0-9]{1,4}h(?:[0-5]?[0-9]m(?:[0-5]?[0-9]s)?|[0-5][0-9])|[0-9]{1,4}m[0-5]?[0-9]s";

/// The pattern of a part: a glued run of units, or an amount (in digits, a
/// fraction "1/2", "1/4" or "3/4" after an optional whole number, or in words
/// followed by a space), a unit, and an optional "and a half".
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
    let digits = format!(
        r"(?P<slash>(?:(?P<slashwhole>[0-9]{{1,4}}){SPACE}+)?(?-u:\b)(?P<slashpart>1/[24]|3/4))|(?P<vulgar>(?:{LEAD})?[½¼¾])|(?P<printed>{PRINTED})"
    );
    let worded = format!(
        r"(?i:(?P<couple>(?:a\s+)?couple\s+of|(?:un\s+)?paio\s+di))|(?i:(?P<tens>{tens})(?:\s+(?:y|e|et)\s+|-et-|-|\s+)(?P<ones>{ones}))|(?i:(?P<ittens>{it_tens})[ia]?(?P<itones>{it_ones}))|(?i:(?P<article>an?))|(?i:(?P<word>{words}|{tens}))"
    );
    let and_half = r"\s+and\s+an?\s+half";
    let suffix = r"(?P<half>and\s+an?\s+half|et\s+demie?|y\s+media|e\s+mezz[oa]|e\s+meia)|(?P<quarter>and\s+a\s+quarter|et\s+quart|y\s+cuarto|e\s+un\s+quarto|e\s+um\s+quarto)|(?P<threequarters>e\s+tre\s+quarti)";
    format!(
        r"(?:(?-u:\b)(?P<glued>{GLUED})(?-u:\b)|(?:(?:{digits})(?i:(?P<andhalf>{and_half}))?(?P<gap>{SPACE}|-)?|(?-u:\b)(?:{worded})(?i:(?P<wordhalf>{and_half}))?(?:{SPACE}|-)|(?-u:\b)(?i:(?P<elided>un['’])|(?P<coupled>(?:un\s+)?paio\s+d['’])))(?:{units})(?-u:\b)(?:\s+(?i:{suffix})(?-u:\b))?)"
    )
}

/// The pattern of a fixed phrase.
const SPECIAL: &str = r"(?i)(?-u:\b)(?:(?P<half>half\s+an?\s+hour|an?\s+half[\s-]hour|(?:une\s+)?demie?[\s-]heure|(?:una\s+)?media\s+hora|(?:una\s+)?mezz['’]?ora|(?:uma\s+)?meia[\s-]hora|1/2\s?h(?:our|eure)?)|(?P<quarter>(?:a\s+)?quarter(?:\s+of\s+an\s+|-)hour|(?:un\s+)?quart\s+d['’]heure|(?:un\s+)?cuarto\s+de\s+hora|(?:un\s+)?quarto\s+d['’]ora|(?:um\s+)?quarto\s+de\s+hora|1/4\s?h(?:our|eure)?)|(?P<threequarters>three[\s-]quarters\s+of\s+an\s+hour|trois\s+quarts\s+d['’]heure|tres\s+cuartos\s+de\s+hora|tre\s+quarti\s+d['’]ora|tr[eê]s\s+quartos\s+de\s+hora|3/4\s?h(?:our|eure)?)|(?P<fortnight>(?:a\s+|one\s+)?fortnight))(?-u:\b)";

/// A digit as Arabic texts write them: Western, Arabic-Indic, extended
/// Arabic-Indic, or any other script's.
const AR_DIGIT: &str = r"\p{Nd}";

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
        r"(?:(?P<digits>{d}+(?:{GROUP}{d}+)*)(?:\s*|-)|(?P<cu>{ones})\s+و\s*(?P<ct>{tens})\s+|(?P<tu>{ones})\s+(?:عشرة|عشره|عشر)\s+|(?P<tens>{tens})\s+|(?P<single>{single})\s+|(?P<frac>ثلاثة\s+أرباع|ثلاث\s+أرباع|ثلاثة\s+ارباع|ثلاث\s+ارباع|{fractions})\s+)?(?P<unit>{units})(?:\s+(?P<one>واحدة|واحده|واحد))?(?:\s*(?P<plus>و\s*(?:{fractions}))|\s+(?P<minus>(?:إلا|الا)\s+(?:ربع|ثلث)))?"
    )
}

/// The pattern of a part whose unit name ends outside ASCII: digits, an
/// optional space or hyphen, and a name of [`NAMED`].
fn named_pattern() -> String {
    let names = alternation(NAMED.iter().map(|&(w, _)| w));
    format!(r"(?P<printed>{PRINTED})(?:{SPACE}|-)?(?P<unit>(?i:{names}))")
}

/// The named-unit part pattern, compiled once.
static NAMED_PART: LazyLock<Option<Regex>> = LazyLock::new(|| compile(&named_pattern()));

/// The part pattern, compiled once.
static PART: LazyLock<Option<Regex>> = LazyLock::new(|| compile(&part_pattern()));

/// The Arabic part pattern, compiled once.
static ARABIC: LazyLock<Option<Regex>> = LazyLock::new(|| compile(&arabic_pattern()));

/// The fixed phrases, compiled once.
static PHRASE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(SPECIAL));

/// Whether a part may start at byte `at`: not inside a word or a number,
/// nor after a number's group or decimal symbol or a fraction slash ("1/2
/// hours"), which leaves a piece of the number before it. A
/// letter of a script written without spaces is no word boundary
/// ("合計5分钟").
pub fn starts_clean(text: &str, at: usize) -> bool {
    match char_before(text, at) {
        None => true,
        Some(c) if c == '/' || (is_number_symbol(c) && !c.is_whitespace()) => {
            !char_before(text, at.saturating_sub(c.len_utf8())).is_some_and(char::is_numeric)
        }
        Some(c) => !(c == '_' || c.is_numeric() || (c.is_alphabetic() && !is_unspaced_script(c))),
    }
}

/// Whether a part may end at byte `at`: not before a letter or a digit of
/// any script, which the ASCII boundary in the pattern lets through, except
/// a letter of a script written without spaces ("5分钟後", "30 minutes後").
fn ends_clean(text: &str, at: usize) -> bool {
    !char_at(text, at).is_some_and(|c| c.is_alphanumeric() && !is_unspaced_script(c))
}

/// The amount a part's captures write, and its other reading when its digits
/// fit two (see [`Amount::of_printed`]).
fn amount(caps: &Groups<'_, '_>) -> Option<(Amount, Option<Amount>)> {
    if let Some(m) = caps.get("printed") {
        return Amount::of_printed(m.as_str());
    }
    if let Some(m) = caps.get("vulgar") {
        return Some((Amount::of_digits(m.as_str(), false)?, None));
    }
    if let Some(part) = caps.get("slashpart") {
        let whole = caps
            .get("slashwhole")
            .map_or(Some(0), |w| w.as_str().parse::<u64>().ok())?;
        let hundredths = match part.as_str() {
            "1/2" => 50,
            "1/4" => 25,
            _ => 75,
        };
        let mantissa = whole.checked_mul(100)?.checked_add(hundredths)?;
        return Some((Amount { mantissa, scale: 2 }, None));
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
    Some((Amount::whole(value), None))
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
    let smallest = smallest?;
    Some(Part {
        span,
        length,
        largest: largest?,
        smallest,
        place: smallest.one(),
        loose: None,
        // An hour and two digits with no `m` after them ("15h30") is how
        // French and German write a time of day.
        stated: !written.ends_with(|c: char| c.is_ascii_digit()),
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
    let (count, other) = amount(caps)?;
    let (unit, written) = Unit::ALL
        .into_iter()
        .find_map(|u| caps.get(u.group()).map(|m| (u, m)))?;
    let digits = caps
        .get("printed")
        .or_else(|| caps.get("vulgar"))
        .or_else(|| caps.get("slash"));
    let symbol = written.as_str().chars().count() == 1;
    // "1/2 h" is a fixed phrase's, which holds a reply to it.
    if symbol && caps.get("slash").is_some() {
        return None;
    }
    // A spaced "y" is the Spanish "and" ("entre 1 y 2 horas"), not a year.
    if written.as_str() == "y" && (digits.is_none() || caps.get("gap").is_some()) {
        return None;
    }
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
    let mut place = match digits {
        Some(digits) => digits_place(unit, count, digits.as_str())?,
        None => unit.one(),
    };
    let loose = match (other, digits) {
        (Some(other), Some(digits)) if extra.is_none() => {
            Some(reading(unit, other, digits.as_str())?)
        }
        _ => None,
    };
    if let Some((_, fraction)) = extra {
        length = length.checked_add(unit.times(fraction)?)?;
        let step = if fraction == Amount::HALF {
            Amount::HALF
        } else {
            Amount::QUARTER
        };
        place = place.min(unit.times(step)?);
    }
    Some(Part {
        span: whole.start()..dotted_end(text, written.as_str(), whole.end()),
        length,
        largest: unit,
        smallest: unit,
        place,
        loose,
        stated: !symbol && unit < Unit::Day,
    })
}

/// The part a fixed phrase writes.
fn read_phrase(text: &str, caps: &Groups<'_, '_>) -> Option<Part> {
    let whole = caps.whole()?;
    if !starts_clean(text, whole.start()) || !ends_clean(text, whole.end()) {
        return None;
    }
    let (unit, count, step) = if caps.get("half").is_some() {
        (Unit::Minute, 30, 30)
    } else if caps.get("quarter").is_some() {
        (Unit::Minute, 15, 15)
    } else if caps.get("threequarters").is_some() {
        (Unit::Minute, 45, 15)
    } else {
        (Unit::Week, 2, 1)
    };
    Some(Part {
        span: whole.range(),
        length: unit.times(Amount::whole(count))?,
        largest: unit,
        smallest: unit,
        place: unit.times(Amount::whole(step))?,
        loose: None,
        stated: unit < Unit::Day,
    })
}

/// Whether `c` is a digit or a cased letter, which no uncased script
/// writes.
fn is_cased_or_digit(c: char) -> bool {
    c.is_numeric() || c.is_lowercase() || c.is_uppercase()
}

/// Whether a unit name ending in `last` may end before byte `at`. A name in a
/// cased script ends before anything but a letter, a mark or a digit. A name
/// in a script written without spaces ("3時間かかった", "3시간이") may end
/// before an uncased letter. A name in another uncased script may go on in
/// letters and marks of its own, an inflection ("۳ ساعته", "5 मिनटों",
/// "5 دقائقَ"), but not into a cased letter or a digit ("วินาทีly").
fn ends_named(text: &str, at: usize, last: char) -> bool {
    let Some(rest) = text.get(at..) else {
        return true;
    };
    let Some(next) = rest.chars().next() else {
        return true;
    };
    if is_unspaced_script(last) {
        !is_cased_or_digit(next)
    } else if is_cased_or_digit(last) {
        is_unspaced_script(next) || !(is_cased_or_digit(next) || is_letter(next) || is_mark(next))
    } else {
        !rest
            .chars()
            .take_while(|&c| is_cased_or_digit(c) || is_letter(c) || is_mark(c))
            .any(is_cased_or_digit)
    }
}

/// Byte `at` moved past the combining marks that start there: the short
/// vowels and tanween an Arabic word may be written with ("دقائقَ").
fn past_marks(text: &str, at: usize) -> usize {
    text.get(at..).map_or(at, |rest| {
        at.saturating_add(
            rest.char_indices()
                .find(|&(_, c)| !is_mark(c))
                .map_or(rest.len(), |(i, _)| i),
        )
    })
}

/// Whether `c` is a combining mark, which continues the letter before it.
fn is_mark(c: char) -> bool {
    use unicode_general_category::{GeneralCategory, get_general_category};
    matches!(
        get_general_category(c),
        GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark
    )
}

/// Where a unit written `name` and ending at byte `end` ends with its closing
/// dot: past the dot when CLDR abbreviates the name with one ("Std.",
/// "godz.", "min."), so a compound after it ("2 Std. 30 min") still joins.
fn dotted_end(text: &str, name: &str, end: usize) -> usize {
    let dotted = DOTTED.iter().any(|&d| same_word(d, name));
    if dotted && text.get(end..).is_some_and(|rest| rest.starts_with('.')) {
        end.saturating_add(1)
    } else {
        end
    }
}

/// The part a match of the named-unit pattern writes, if it is one.
fn read_named(text: &str, caps: &Groups<'_, '_>) -> Option<Part> {
    let whole = caps.whole()?;
    let name = caps.get("unit")?;
    let last = name.as_str().chars().next_back()?;
    if !starts_clean(text, whole.start()) || !ends_named(text, whole.end(), last) {
        return None;
    }
    let &(_, unit) = NAMED.iter().find(|&&(w, _)| same_word(w, name.as_str()))?;
    let written = caps.get("printed")?.as_str();
    let (count, other) = Amount::of_printed(written)?;
    let (length, place) = reading(unit, count, written)?;
    Some(Part {
        span: whole.start()..dotted_end(text, name.as_str(), whole.end()),
        length,
        largest: unit,
        smallest: unit,
        place,
        loose: other.and_then(|o| reading(unit, o, written)),
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
        return Some(Before::Count(Amount::of_printed(m.as_str())?.0));
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

/// The step a fraction word writes `unit` to: three quarters are written to
/// the quarter, a half and a third to themselves.
fn arabic_step(unit: Unit, word: &str) -> Option<Length> {
    let word = word.trim_start_matches(|c: char| c == 'و' || c.is_whitespace());
    if word.starts_with("ثلاث") {
        unit.times(Amount::QUARTER)
    } else {
        arabic_fraction(unit, word)
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
/// "the clock"; a dual counts two by itself, and a number before it ("2
/// ساعتين") as a plural does.
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
    if !ends_clean(text, past_marks(text, end)) {
        // A fraction glued to more letters is not one: read the part without it.
        plus.take().or_else(|| minus.take())?;
        end = one.map_or_else(|| unit_word.end(), |o| o.end());
        if !ends_clean(text, past_marks(text, end)) {
            return None;
        }
    }
    let suffixed = plus.is_some() || minus.is_some();
    let mut place = unit.one();
    let before = arabic_before(caps)?;
    // A unit with no amount right after digits is the end of a number this
    // pattern did not take ("x97-ساعتان"), not a count of its own.
    if matches!(before, Before::Nothing)
        && text
            .get(..whole.start())
            .map(|t| t.strip_suffix([' ', '-']).unwrap_or(t))
            .and_then(|t| t.chars().next_back())
            .is_some_and(char::is_numeric)
    {
        return None;
    }
    let base = match (before, form) {
        (Before::Count(count), Form::Singular | Form::Plural | Form::Dual) if one.is_none() => {
            place = unit.times(Amount {
                mantissa: 1,
                scale: count.scale,
            })?;
            unit.times(count)?
        }
        (Before::Fraction(word), Form::Singular | Form::Definite) if one.is_none() && !suffixed => {
            place = arabic_step(unit, word)?;
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
        place = place.min(arabic_step(unit, word.as_str())?);
        base.checked_add(arabic_fraction(unit, word.as_str())?)?
    } else if let Some(word) = minus {
        let word = word.as_str().split_whitespace().last()?;
        place = place.min(arabic_step(unit, word)?);
        let fraction = arabic_fraction(unit, word)?;
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
        place,
        loose: None,
        stated: unit < Unit::Day,
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

/// `text` with each underscore that marks emphasis read as a space
/// ("_2 hours_", "__94 seconds__"), so a length in italics is read; one
/// between two letters or digits joins a name (`run_2h`) and stays. The
/// text keeps its length, so a part's span is the same in both.
fn without_emphasis(text: &str) -> Cow<'_, str> {
    let marks = |at: usize| {
        !(char_before(text, at).is_some_and(char::is_alphanumeric)
            && char_at(text, at.saturating_add(1)).is_some_and(char::is_alphanumeric))
    };
    let emphasis = |at: &usize| marks(*at) && amount_beside(text, *at);
    let mut found = text
        .match_indices('_')
        .map(|(at, _)| at)
        .filter(emphasis)
        .peekable();
    if found.peek().is_none() {
        return Cow::Borrowed(text);
    }
    let mut out = text.as_bytes().to_vec();
    for at in found {
        if let Some(b) = out.get_mut(at) {
            *b = b' ';
        }
    }
    String::from_utf8(out).map_or(Cow::Borrowed(text), Cow::Owned)
}

/// Whether a digit or a letter stands right before or after the underscore
/// at byte `at`, so the mark wraps something a length may be read in.
fn amount_beside(text: &str, at: usize) -> bool {
    char_at(text, at.saturating_add(1)).is_some_and(|c| c.is_alphanumeric() || c == '~')
        || char_before(text, at).is_some_and(|c| c.is_alphanumeric() || c == '.')
}

/// Every part in `text`, in order, a longer one winning an overlap.
pub fn parts(text: &str) -> Vec<Part> {
    let unemphasised = without_emphasis(text);
    let scanned: &str = &unemphasised;
    let mut found = Vec::new();
    let readers: [(&Option<Regex>, Reader, bool); 4] = [
        (&PART, read_part, true),
        (&PHRASE, read_phrase, true),
        (&ARABIC, read_arabic, has_arabic(scanned)),
        (&NAMED_PART, read_named, !scanned.is_ascii()),
    ];
    for (re, read, wanted) in readers {
        if wanted && let Some(re) = re {
            collect(re, scanned, read, &mut found);
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
