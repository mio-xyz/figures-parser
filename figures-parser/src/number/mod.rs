//! The number kind: digits in any script, grouped or decimal, and what a
//! reading of one is held to.

pub mod digits;
mod ground;
mod readings;
pub mod scan;

pub use ground::ungrounded_figures;

use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

use scan::{Tokens, tokens};

/// One reading of a numeric token, its digits held as `D`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure<'a, D> {
    /// As written, separators kept.
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
        }
    }
}

/// The readings of a text's numeric tokens, found as they are asked for.
#[derive(Debug)]
pub struct Numbers<'a> {
    /// The tokens not yet read.
    tokens: Tokens<'a>,
    /// The digits a reading joins or converts.
    scratch: String,
    /// The current token's readings not yet yielded.
    pending: VecDeque<Number<'a>>,
}

/// Every reading of every numeric token in `text`, in order of appearance,
/// one at a time: a consumer that stops early reads no further.
#[must_use]
pub const fn numbers(text: &str) -> Numbers<'_> {
    Numbers {
        tokens: tokens(text),
        scratch: String::new(),
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
            let token = self.tokens.next()?;
            token.for_each_reading(&mut self.scratch, |r| self.pending.push_back(r.into()));
        }
    }
}

/// Every reading of every numeric token in `text`, in order of appearance.
#[must_use]
pub fn extract_numbers(text: &str) -> Vec<Number<'_>> {
    let mut out = Vec::new();
    let mut scratch = String::new();
    for token in tokens(text) {
        token.for_each_reading(&mut scratch, |r| out.push(r.into()));
    }
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

/// What a stated reading is compared as.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// The integer, which a printed decimal of the same integer part also grounds.
    Integer(String),
    /// A precise decimal, grounded by a printed decimal that starts with it.
    Decimal {
        /// The integer part in ASCII digits.
        integer: String,
        /// The fractional digits in ASCII.
        fraction: String,
    },
}

impl Key {
    /// The integer part, which every reading that grounds this key shares.
    #[must_use]
    pub fn integer(&self) -> &str {
        match self {
            Self::Integer(integer) | Self::Decimal { integer, .. } => integer,
        }
    }

    /// Whether a corpus reading carries this key: the same integer, and for a
    /// precise decimal a printed fraction that starts with the stated one.
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
        }
    }
}

impl Reading<'_, '_> {
    /// The key a reply states with this reading, or None when it is not held
    /// to it: a derived reading, or a figure under the floor its notation sets.
    #[must_use]
    pub fn stated(&self) -> Option<Key> {
        if self.derived {
            return None;
        }
        if let Some(fraction) = self.fraction
            && fraction.len() >= PRECISE_FRACTION_DIGITS
        {
            return Some(Key::Decimal {
                integer: self.integer.to_owned(),
                fraction: fraction.to_owned(),
            });
        }
        let floor = if self.grouped {
            MIN_SEPARATED_FIGURE
        } else if self.currency {
            MIN_CURRENCY_FIGURE
        } else {
            MIN_BARE_FIGURE
        };
        // An integer too long for u128 is far above every floor.
        let reaches = self.integer.parse::<u128>().map_or(true, |n| n >= floor);
        reaches.then(|| Key::Integer(self.integer.to_owned()))
    }
}
