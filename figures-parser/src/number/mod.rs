//! The number kind: digits in any script, grouped or decimal, and what a
//! reading of one is held to.

mod arithmetic;
pub mod digits;
mod ground;
mod percent;
mod readings;
mod scale;
pub mod scan;
mod spelled;

pub use arithmetic::prose_arithmetic;
pub use ground::ungrounded_figures;
pub use percent::ungrounded_percentages;

use std::borrow::Cow;
use std::collections::VecDeque;
use std::iter::Peekable;
use std::ops::{Range, RangeInclusive};

use scan::{Scratch, Token, Tokens, tokens};
use spelled::{Spelled, SpelledNumbers, spelled};

/// The sign written before a figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Sign {
    /// None.
    #[default]
    Unsigned,
    /// A plus sign ("+12%").
    Plus,
    /// A minus sign ("-12,500", "−1 234,5", "-$12").
    Minus,
    /// Parentheses around the figure, as accounting prints a negative
    /// ("($1,234.50)"); prose writes an aside the same way, so it may or may
    /// not be negative.
    Parenthesized,
}

impl Sign {
    /// The sign's name, as a caller reads it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unsigned => "none",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Parenthesized => "parenthesized",
        }
    }

    /// The sign a reply is held to when it writes this one: parentheses,
    /// which may be an aside, hold it to none.
    #[must_use]
    pub const fn stated(self) -> Self {
        match self {
            Self::Parenthesized => Self::Unsigned,
            Self::Unsigned | Self::Plus | Self::Minus => self,
        }
    }

    /// Whether a corpus figure written with `read` carries a figure stated
    /// with this sign: a minus needs one that is or may be negative, a plus
    /// one that is not negative, and none takes either, since a reply may
    /// put the sign in words ("fell by 12,500").
    #[must_use]
    pub const fn carried_by(self, read: Self) -> bool {
        match self {
            Self::Unsigned | Self::Parenthesized => true,
            Self::Minus => matches!(read, Self::Minus | Self::Parenthesized),
            Self::Plus => !matches!(read, Self::Minus),
        }
    }
}

/// One reading of a numeric token, its digits held as `D`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a fact of its own about the reading, not a state"
)]
pub struct Figure<'a, D> {
    /// As written, separators and a scale word kept ("1.2 million").
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
    /// The integer part in ASCII digits, separators dropped.
    pub integer: D,
    /// The fractional digits in ASCII, or None for a whole number.
    pub fraction: Option<D>,
    /// Written with thousands separators, so stated as a precise amount.
    pub grouped: bool,
    /// Written beside a currency sign or code.
    pub currency: bool,
    /// A neighbour reading of a run that also reads as thousands: the corpus
    /// may ground with it, the reply never states it.
    pub derived: bool,
    /// The sign written before it; of a run's neighbours, only the first has
    /// one.
    pub sign: Sign,
    /// Whether it is stated as rounded: an approximation mark stands before
    /// it ("~12,500"), or a scale word after its digits ("1.2 million"). Of
    /// a run's neighbours, only the first has the mark.
    pub approximate: bool,
    /// Its text from its sign or approximation mark on ("~-$1.2 million");
    /// the same as `text` when neither stands before it.
    pub full_text: &'a str,
}

/// A reading as [`scan::Token::for_each_reading`] yields it: its digits
/// borrow the text or the scratch buffer, for the call.
pub type Reading<'a, 's> = Figure<'a, &'s str>;

/// A reading that outlives the scratch buffer: its digits borrow the text
/// where they are a slice of it, which they are for every ASCII group, and
/// are owned where they were joined or converted.
pub type Number<'a> = Figure<'a, Cow<'a, str>>;

/// `digits` borrowed from `text` when it starts (or, from the end, ends)
/// with exactly those bytes, else owned.
fn digits_of<'a>(text: &'a str, digits: &str, from_end: bool) -> Cow<'a, str> {
    let slice = if from_end {
        text.len()
            .checked_sub(digits.len())
            .and_then(|at| text.get(at..))
    } else {
        text.get(..digits.len())
    };
    match slice {
        Some(slice) if slice == digits => Cow::Borrowed(slice),
        _ => Cow::Owned(digits.to_owned()),
    }
}

impl<'a> From<Reading<'a, '_>> for Number<'a> {
    fn from(r: Reading<'a, '_>) -> Self {
        Self {
            text: r.text,
            span: r.span,
            integer: digits_of(r.text, r.integer, false),
            fraction: r.fraction.map(|f| digits_of(r.text, f, true)),
            grouped: r.grouped,
            currency: r.currency,
            derived: r.derived,
            sign: r.sign,
            approximate: r.approximate,
            full_text: r.full_text,
        }
    }
}

/// The readings of a text's numbers, found as they are asked for.
#[derive(Debug)]
pub struct Numbers<'a> {
    /// The text read.
    text: &'a str,
    /// The numeric tokens not yet read.
    tokens: Peekable<Tokens<'a>>,
    /// The numbers written in words not yet read.
    words: Words<'a>,
    /// The digits a reading joins, converts or scales.
    scratch: Scratch,
    /// The current token's readings not yet yielded.
    pending: VecDeque<Number<'a>>,
}

/// Every reading of every number in `text`, written in digits or in words,
/// in order of appearance, one at a time: a consumer that stops early reads
/// no further.
///
/// # Examples
///
/// ```
/// let first = figures_parser::numbers("Total $1,250 and 7.").next().unwrap();
/// assert_eq!((first.text, &*first.integer, first.currency), ("1,250", "1250", true));
/// ```
#[must_use]
pub fn numbers(text: &str) -> Numbers<'_> {
    Numbers {
        text,
        tokens: tokens(text).peekable(),
        words: Words::new(text),
        scratch: Scratch::new(),
        pending: VecDeque::new(),
    }
}

impl<'a> Iterator for Numbers<'a> {
    type Item = Number<'a>;

    fn next(&mut self) -> Option<Number<'a>> {
        loop {
            if let Some(number) = self.pending.pop_front() {
                return Some(number);
            }
            let token_start = self.tokens.peek().map(|token| token.start);
            if let Some(number) = self.words.before(token_start) {
                return Some(self.scratch.spelled(self.text, &number).into());
            }
            let token = self.tokens.next()?;
            match self.words.at(&token) {
                Standing::For(number) => {
                    return Some(self.scratch.spelled(self.text, &number).into());
                }
                Standing::Covered => {}
                Standing::Alone => {
                    token.for_each_reading(&mut self.scratch, |r| self.pending.push_back(r.into()));
                }
            }
        }
    }
}

/// The numbers written in words in a text, paired with its numeric tokens.
#[derive(Debug)]
struct Words<'a> {
    /// The numbers not yet given out.
    numbers: Peekable<SpelledNumbers<'a>>,
    /// The byte the last number given out ends at.
    covered: usize,
}

/// What a numeric token stands as beside the numbers in words.
enum Standing {
    /// On its own.
    Alone,
    /// Inside a number in words already given out ("5 million" of "2 point
    /// 5 million").
    Covered,
    /// At the start of a number in words that goes on past what the token
    /// reads ("2 million three hundred thousand"), which stands for it; one
    /// that ends with the token ("3 mil millones") repeats it, and is
    /// dropped.
    For(Spelled),
}

impl<'a> Words<'a> {
    /// The numbers `text` writes in words.
    fn new(text: &'a str) -> Self {
        Self {
            numbers: spelled(text).peekable(),
            covered: 0,
        }
    }

    /// The next number, when it starts before byte `at`; any, for None.
    fn before(&mut self, at: Option<usize>) -> Option<Spelled> {
        let number = self
            .numbers
            .next_if(|n| at.is_none_or(|at| n.span.start < at))?;
        self.covered = number.span.end;
        Some(number)
    }

    /// What `token` stands as.
    fn at(&mut self, token: &Token<'_>) -> Standing {
        if token.start < self.covered {
            return Standing::Covered;
        }
        match self.numbers.next_if(|n| n.span.start == token.start) {
            Some(number) if number.span.end > token.end() => {
                self.covered = number.span.end;
                Standing::For(number)
            }
            _ => Standing::Alone,
        }
    }
}

/// Calls `f` with every reading of `text` in order of appearance: those of
/// the numeric tokens `tokens` finds, and of the numbers written in words.
pub(crate) fn for_each_reading_in<'a>(
    text: &'a str,
    tokens: Tokens<'a>,
    scratch: &mut Scratch,
    mut f: impl FnMut(Reading<'a, '_>),
) {
    let mut words = Words::new(text);
    for token in tokens {
        while let Some(number) = words.before(Some(token.start)) {
            f(scratch.spelled(text, &number));
        }
        match words.at(&token) {
            Standing::For(number) => f(scratch.spelled(text, &number)),
            Standing::Covered => {}
            Standing::Alone => token.for_each_reading(scratch, &mut f),
        }
    }
    while let Some(number) = words.before(None) {
        f(scratch.spelled(text, &number));
    }
}

/// Every reading of every number in `text`, written in digits or in words,
/// in order of appearance.
///
/// # Examples
///
/// "1.250" fits two readings, and its thousands reading also yields the
/// groups printed side by side, as derived readings that only ground:
///
/// ```
/// let readings: Vec<_> = figures_parser::extract_numbers("1.250")
///     .iter()
///     .map(|n| (n.integer.to_string(), n.fraction.as_deref().map(str::to_owned), n.derived))
///     .collect();
/// assert_eq!(readings, [
///     ("1250".into(), None, false),
///     ("1".into(), Some("250".into()), false),
///     ("1".into(), None, true),
///     ("250".into(), None, true),
/// ]);
/// ```
#[must_use]
pub fn extract_numbers(text: &str) -> Vec<Number<'_>> {
    let mut out = Vec::new();
    let mut scratch = Scratch::default();
    for_each_reading_in(text, tokens(text), &mut scratch, |r| out.push(r.into()));
    out
}

/// A bare digit run below this is usually the model's own arithmetic, which
/// no tool result carries verbatim.
const MIN_BARE_FIGURE: u128 = 10_000;
/// A figure written with a thousands separator ("8,220") is stated as a
/// precise amount, so the separator lowers the floor to the smallest figure
/// the notation can write.
const MIN_SEPARATED_FIGURE: u128 = 1_000;
/// A currency sign states an amount as surely as a separator does ("$729").
const MIN_CURRENCY_FIGURE: u128 = 100;
/// A value with this many fractional digits is read off a result, never
/// rounded arithmetic of the model's own.
const PRECISE_FRACTION_DIGITS: usize = 4;

/// What a stated reading is compared as: its value, the sign it holds the
/// corpus to, and for a figure marked approximate the values that round to
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    /// The value.
    pub value: Value,
    /// The sign, as [`Sign::stated`] reads it.
    pub sign: Sign,
    /// For a figure marked approximate ("~12,500"), the values that round to
    /// it, which ground it as its exact value does.
    pub around: Option<Around>,
}

/// The value a stated reading is compared by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    /// The integer, which a printed decimal of the same integer part also grounds.
    Integer(String),
    /// A precise decimal, grounded by a printed decimal that starts with it.
    Decimal {
        /// The integer part in ASCII digits.
        integer: String,
        /// The fractional digits in ASCII.
        fraction: String,
    },
    /// A result worked out in prose, grounded only by a printed value equal
    /// to it, a fraction's trailing zeros aside ("15,325.0" is 15325).
    Exact {
        /// The integer part in ASCII digits.
        integer: String,
        /// The fractional digits in ASCII, trailing zeros cut; empty for a
        /// whole number.
        fraction: String,
    },
}

/// The values that round to an approximate figure: those whose digits,
/// scaled by `10^scale` and cut there, fall in `low..high`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Around {
    /// The lowest scaled value that rounds to the figure.
    pub low: u128,
    /// The lowest scaled value above it that no longer does.
    pub high: u128,
    /// The power of ten the bounds are scaled by.
    pub scale: u32,
}

impl Key {
    /// The integer part of the stated value.
    #[must_use]
    pub fn integer(&self) -> &str {
        match &self.value {
            Value::Integer(integer)
            | Value::Decimal { integer, .. }
            | Value::Exact { integer, .. } => integer,
        }
    }

    /// The digit counts of the integer parts that may ground this key: its
    /// own as written, leading zeros kept, and for an approximate one those
    /// of every value that rounds to it.
    #[must_use]
    pub fn integer_lengths(&self) -> RangeInclusive<usize> {
        let own = self.integer().len();
        self.around.map_or(own..=own, |around| {
            let range = around.integer_lengths();
            own.min(*range.start())..=own.max(*range.end())
        })
    }

    /// Whether a corpus reading carries this key's value, whatever its sign:
    /// its exact value, or for an approximate one a value that rounds to it.
    #[must_use]
    pub fn value_grounded_by(&self, reading: &Reading<'_, '_>) -> bool {
        self.value.grounded_by(reading)
            || self.around.is_some_and(|around| around.contains(reading))
    }

    /// Whether a corpus reading carries this key: its value, and a sign
    /// [`Sign::carried_by`] accepts.
    #[must_use]
    pub fn grounded_by(&self, reading: &Reading<'_, '_>) -> bool {
        self.value_grounded_by(reading) && self.sign.carried_by(reading.sign)
    }
}

impl Value {
    /// Whether a corpus reading carries this value, whatever its sign: the
    /// same integer, and for a precise decimal a printed fraction that starts
    /// with the stated one.
    #[must_use]
    pub fn grounded_by(&self, reading: &Reading<'_, '_>) -> bool {
        match self {
            Self::Integer(integer) => integer == reading.integer,
            Self::Decimal { integer, fraction } => {
                integer == reading.integer
                    && reading
                        .fraction
                        .is_some_and(|printed| printed.starts_with(fraction.as_str()))
            }
            Self::Exact { integer, fraction } => {
                integer == reading.integer
                    && reading.fraction.unwrap_or_default().trim_end_matches('0') == fraction
            }
        }
    }
}

impl Around {
    /// The values that round to a figure written as `integer.fraction`, at
    /// the precision it is written with: its last digit for a decimal, its
    /// last nonzero digit for a whole number ("~12,500" takes 12,450 to
    /// 12,549). A power of ten with one significant digit takes from where a
    /// value rounds up to it ("~10,000" takes 9,500 to 14,999). None when the
    /// bounds overflow.
    fn of(integer: &str, fraction: Option<&str>) -> Option<Self> {
        let fraction = fraction.unwrap_or_default();
        let places = u32::try_from(fraction.len()).ok()?;
        let stated = scaled(integer, Some(fraction), places)?;
        let zeros = if fraction.is_empty() && stated != 0 {
            integer.len() - integer.trim_end_matches('0').len()
        } else {
            0
        };
        let unit = 10_u128.checked_pow(u32::try_from(zeros).ok()?)?;
        let half_above = unit.checked_mul(50)?;
        let half_below = if stated == unit {
            unit.checked_mul(5)?
        } else {
            half_above
        };
        let scale = places
            .checked_add(2)
            .filter(|&scale| 10_u128.checked_pow(scale).is_some())?;
        let centre = stated.checked_mul(100)?;
        Some(Self {
            low: centre.checked_sub(half_below)?,
            high: centre.checked_add(half_above)?,
            scale,
        })
    }

    /// The digit counts of the integer parts of the values in the range.
    #[must_use]
    pub fn integer_lengths(&self) -> RangeInclusive<usize> {
        let integer_of = |v: u128| {
            10_u128
                .checked_pow(self.scale)
                .and_then(|p| v.checked_div(p))
                .unwrap_or_default()
        };
        let digits = |v: u128| {
            v.checked_ilog10()
                .and_then(|l| usize::try_from(l).ok())
                .map_or(1, |l| l.saturating_add(1))
        };
        digits(integer_of(self.low))..=digits(integer_of(self.high.saturating_sub(1)))
    }

    /// Whether a corpus reading's value is in the range, whatever its sign.
    #[must_use]
    pub fn contains(&self, reading: &Reading<'_, '_>) -> bool {
        scaled(reading.integer, reading.fraction, self.scale)
            .is_some_and(|v| (self.low..self.high).contains(&v))
    }
}

/// `integer.fraction` times `10^scale`, the fraction cut or padded with zeros
/// to `scale` places; None past `u128`. Both parts are ASCII digits.
pub(super) fn scaled(integer: &str, fraction: Option<&str>, scale: u32) -> Option<u128> {
    let mut value = integer.parse::<u128>().ok()?;
    let mut places = fraction.unwrap_or_default().bytes();
    for _ in 0..scale {
        let digit = places.next().map_or(0, |b| b.saturating_sub(b'0'));
        value = value.checked_mul(10)?.checked_add(u128::from(digit))?;
    }
    Some(value)
}

impl Reading<'_, '_> {
    /// The key a reply states with this reading, or None when it is not held
    /// to it: a derived reading, or a figure under the floor its notation
    /// sets. An approximate reading is also grounded by a value that rounds
    /// to it.
    #[must_use]
    pub fn stated(&self) -> Option<Key> {
        if self.derived {
            return None;
        }
        let value = if let Some(fraction) = self.fraction
            && fraction.len() >= PRECISE_FRACTION_DIGITS
        {
            Value::Decimal {
                integer: self.integer.to_owned(),
                fraction: fraction.to_owned(),
            }
        } else {
            let floor = if self.grouped {
                MIN_SEPARATED_FIGURE
            } else if self.currency {
                MIN_CURRENCY_FIGURE
            } else {
                MIN_BARE_FIGURE
            };
            // An integer too long for u128 is far above every floor.
            if self.integer.parse::<u128>().is_ok_and(|n| n < floor) {
                return None;
            }
            Value::Integer(self.integer.to_owned())
        };
        Some(self.key(value))
    }

    /// The key a reply states with this reading as a result it worked out
    /// itself, or None when it is not one: a derived reading, a whole number
    /// of ten or less, which is a count ("3 options"), or a year ("2026"). A
    /// decimal or a negative is always one, "-0" aside. Its value is matched
    /// exactly.
    #[must_use]
    pub fn result(&self) -> Option<Key> {
        if self.derived {
            return None;
        }
        let count = || self.integer.parse::<u128>().is_ok_and(|n| n <= 10);
        let year = || {
            !self.grouped
                && self.integer.len() == 4
                && self
                    .integer
                    .parse::<u16>()
                    .is_ok_and(|n| (1900..=2100).contains(&n))
        };
        let negative = self.sign == Sign::Minus && !self.integer.bytes().all(|b| b == b'0');
        if self.fraction.is_none() && !negative && (count() || year()) {
            return None;
        }
        Some(
            self.key(Value::Exact {
                integer: self.integer.to_owned(),
                fraction: self
                    .fraction
                    .unwrap_or_default()
                    .trim_end_matches('0')
                    .to_owned(),
            }),
        )
    }

    /// The key this reading states with `value`: its sign as stated, and for
    /// an approximate reading the values that round to it.
    fn key(&self, value: Value) -> Key {
        Key {
            value,
            sign: self.sign.stated(),
            around: self
                .approximate
                .then(|| Around::of(self.integer, self.fraction))
                .flatten(),
        }
    }
}
