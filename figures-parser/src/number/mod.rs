//! The number kind: digits in any script, grouped or decimal, and what a
//! reading of one is held to.

pub mod digits;
mod digits_table;
mod readings;
pub mod scan;

use std::borrow::Cow;
use std::ops::Range;

use scan::{Token, tokens};

/// One reading of a numeric token, borrowed from the text it was read in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Number<'a> {
    /// As written, separators kept.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
    /// The integer part in ASCII digits, separators dropped.
    pub integer: Cow<'a, str>,
    /// The fractional digits in ASCII, or None for a whole number.
    pub fraction: Option<Cow<'a, str>>,
    /// Written with thousands separators, so stated as a precise amount.
    pub grouped: bool,
    /// Written behind a currency sign.
    pub currency: bool,
    /// A neighbour reading of a run that also reads as thousands: the corpus
    /// may ground with it, the reply never states it.
    pub derived: bool,
}

/// Every reading of every numeric token in `text`, in order of appearance.
#[must_use]
pub fn extract_numbers(text: &str) -> Vec<Number<'_>> {
    tokens(text).iter().flat_map(Token::readings).collect()
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

/// What a reading is compared as.
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

impl Number<'_> {
    /// The key a reply states with this reading, or None when it is not held
    /// to it: a derived reading, or a figure under the floor its notation sets.
    #[must_use]
    pub fn stated(&self) -> Option<Key> {
        if self.derived {
            return None;
        }
        if let Some(fraction) = &self.fraction
            && fraction.chars().count() >= PRECISE_FRACTION_DIGITS
        {
            return Some(Key::Decimal {
                integer: self.integer.to_string(),
                fraction: fraction.to_string(),
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
        reaches.then(|| Key::Integer(self.integer.to_string()))
    }
}
