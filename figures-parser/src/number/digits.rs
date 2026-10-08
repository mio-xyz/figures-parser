//! Decimal digits of every script, from the runs the platform's ICU formats.

use std::borrow::Cow;

use super::digits_table::DIGIT_RUNS;

/// The value of a decimal digit in any script, or None for anything else.
#[must_use]
pub fn digit_value(c: char) -> Option<u8> {
    if c.is_ascii() {
        return c.to_digit(10).and_then(|d| u8::try_from(d).ok());
    }
    let cp = u32::from(c);
    let run = DIGIT_RUNS
        .partition_point(|&start| start <= cp)
        .checked_sub(1)?;
    let start = DIGIT_RUNS.get(run)?;
    cp.checked_sub(*start)
        .and_then(|d| u8::try_from(d).ok())
        .filter(|&d| d < 10)
}

/// `digits` with every decimal digit replaced by its ASCII value; any other
/// character is kept as is. Borrowed when nothing needs replacing.
#[must_use]
pub fn ascii(digits: &str) -> Cow<'_, str> {
    if digits.is_ascii() {
        return Cow::Borrowed(digits);
    }
    Cow::Owned(
        digits
            .chars()
            .map(|c| digit_value(c).map_or(c, |v| char::from(b'0' + v)))
            .collect(),
    )
}
