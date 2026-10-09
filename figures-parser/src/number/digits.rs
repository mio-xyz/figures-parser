//! Decimal digits of every script.

/// The first code point of every run of ten decimal digits Unicode encodes,
/// sorted. Unicode's stability policy encodes every decimal digit in a run
/// of ten ascending from zero and never moves one, so the table only grows:
/// `tests/it/digits.rs` fails when the `unicode-general-category` tables know
/// a decimal digit missing here.
#[rustfmt::skip]
const DIGIT_RUNS: &[u32] = &[
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6,
    0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66, 0x0DE6, 0x0E50, 0x0ED0,
    0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80,
    0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900,
    0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10, 0x104A0, 0x10D30, 0x10D40,
    0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650,
    0x116C0, 0x116D0, 0x116DA, 0x11730, 0x118E0, 0x11950, 0x11BF0, 0x11C50,
    0x11D50, 0x11DA0, 0x11DE0, 0x11F50, 0x16130, 0x16A60, 0x16AC0, 0x16B50,
    0x16D70, 0x1CCF0, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140,
    0x1E2F0, 0x1E4F0, 0x1E5F1, 0x1E950, 0x1FBF0,
];

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

/// Appends `digits` to `out` with every decimal digit replaced by its ASCII
/// value; any other character is kept as is.
pub fn push_ascii(digits: &str, out: &mut String) {
    if digits.is_ascii() {
        out.push_str(digits);
    } else {
        push_converted(digits, out);
    }
}

/// Appends `digits` to `out`, every decimal digit converted to ASCII.
fn push_converted(digits: &str, out: &mut String) {
    out.extend(
        digits
            .chars()
            .map(|c| digit_value(c).map_or(c, |v| char::from(b'0' + v))),
    );
}

/// `digits` in ASCII: itself when it already is, else converted into `scratch`.
pub fn ascii_in<'s>(digits: &'s str, scratch: &'s mut String) -> &'s str {
    if digits.is_ascii() {
        return digits;
    }
    scratch.clear();
    push_converted(digits, scratch);
    scratch.as_str()
}
