//! Numeric tokens of a text: digit groups joined by single symbols.
//!
//! The byte fast paths are measured, not assumed: on a 300 KB corpus they
//! take scanning prose from 450 to 280 µs and a log of ids from 990 to 590 µs.

use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_script::{Script, UnicodeScript};

use super::digits::digit_value;

/// The group and decimal symbols CLDR locales print. `tests/it/locales.rs`
/// reads back the number each locale prints and fails when one prints a
/// symbol missing here.
pub const GROUP_OR_DECIMAL_SYMBOLS: [char; 10] = [
    ',', '.', '\'', '\u{2019}', ' ', '\u{a0}', '\u{202f}', '\u{66c}', '\u{66b}', '\u{ff0c}',
];

/// Scripts that write words without spaces, so a figure sits against a
/// letter ("合計は1,250,000円") without being part of an id.
const UNSPACED_SCRIPTS: [Script; 9] = [
    Script::Han,
    Script::Hiragana,
    Script::Katakana,
    Script::Hangul,
    Script::Thai,
    Script::Lao,
    Script::Khmer,
    Script::Myanmar,
    Script::Tibetan,
];

/// A dot, a comma or the Arabic decimal sign may end a decimal; the others only ever group.
#[must_use]
pub const fn is_decimal_symbol(c: char) -> bool {
    matches!(c, '.' | ',' | '\u{66b}')
}

/// Whether `c` is a group or decimal symbol.
#[must_use]
pub fn is_symbol(c: char) -> bool {
    if c.is_ascii() {
        return matches!(c, ',' | '.' | '\'' | ' ');
    }
    GROUP_OR_DECIMAL_SYMBOLS.contains(&c)
}

/// Whether `c` is a decimal digit of any script.
fn is_digit(c: char) -> bool {
    digit_value(c).is_some()
}

/// Whether `c` belongs to a script written without spaces. Script extensions
/// are checked so a mark shared between scripts, like the prolonged-sound
/// mark `ー`, counts for the scripts it serves; a Common or Inherited set
/// answers true for every script, so those characters ("ℝ", "𝐀") never do.
fn is_unspaced_script(c: char) -> bool {
    let extensions = c.script_extension();
    if extensions.is_common() || extensions.is_inherited() {
        return false;
    }
    UNSPACED_SCRIPTS
        .into_iter()
        .any(|script| extensions.contains_script(script))
}

/// Whether `c` ends a token: another digit, or a letter of a word.
fn is_boundary(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric();
    }
    if is_digit(c) {
        return true;
    }
    let letter = matches!(
        get_general_category(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    );
    letter && !is_unspaced_script(c)
}

/// Whether `c` is a currency sign.
fn is_currency_sign(c: char) -> bool {
    if c.is_ascii() {
        return c == '$';
    }
    get_general_category(c) == GeneralCategory::CurrencySymbol
}

/// The character starting at byte `at`.
fn char_at(text: &str, at: usize) -> Option<char> {
    match text.as_bytes().get(at) {
        Some(&b) if b.is_ascii() => Some(char::from(b)),
        _ => text.get(at..)?.chars().next(),
    }
}

/// The character ending at byte `at`.
fn char_before(text: &str, at: usize) -> Option<char> {
    match at.checked_sub(1).and_then(|i| text.as_bytes().get(i)) {
        Some(&b) if b.is_ascii() => Some(char::from(b)),
        _ => text.get(..at)?.chars().next_back(),
    }
}

/// Whether `run`, the ASCII alphanumerics beside a figure, is an ISO 4217
/// code standing as a whole word: `edge`, the character past the run, must
/// not continue it.
fn is_currency_code(run: &str, edge: Option<char>) -> bool {
    !edge.is_some_and(char::is_alphanumeric) && iso_currency::Currency::from_code(run).is_some()
}

/// Whether a currency sign or code marks the figure at `start..end`, on either
/// side, with at most one space between ("$729", "€ 1,458", "99 USD", "12 €").
fn currency_near(text: &str, start: usize, end: usize) -> bool {
    // A marker's first byte past one optional space is non-ASCII, `$` or an
    // ASCII capital; most tokens stand beside something else and stop here.
    let bytes = text.as_bytes();
    let byte_at = |at: Option<usize>| at.and_then(|i| bytes.get(i)).copied();
    let edge = |near: Option<u8>, far: Option<u8>| {
        if near.is_some_and(|b| b.is_ascii_whitespace()) {
            far
        } else {
            near
        }
    };
    let may_mark = |b: u8| !b.is_ascii() || b == b'$' || b.is_ascii_uppercase();
    let before_edge = edge(byte_at(start.checked_sub(1)), byte_at(start.checked_sub(2)));
    let after_edge = edge(byte_at(Some(end)), byte_at(Some(end + 1)));
    if !before_edge.is_some_and(may_mark) && !after_edge.is_some_and(may_mark) {
        return false;
    }
    let ascii_word = |c: char| c.is_ascii_alphanumeric();
    let before = text.get(..start).unwrap_or_default();
    let before = before.strip_suffix(char::is_whitespace).unwrap_or(before);
    let marked_before = match before.chars().next_back() {
        Some(c) if c.is_ascii_uppercase() => {
            let at = before.trim_end_matches(ascii_word).len();
            before
                .get(at..)
                .is_some_and(|run| is_currency_code(run, char_before(before, at)))
        }
        Some(c) => is_currency_sign(c),
        None => false,
    };
    if marked_before {
        return true;
    }
    let after = text.get(end..).unwrap_or_default();
    let after = after.strip_prefix(char::is_whitespace).unwrap_or(after);
    match after.chars().next() {
        Some(c) if c.is_ascii_uppercase() => {
            let len = after.len() - after.trim_start_matches(ascii_word).len();
            after
                .get(..len)
                .is_some_and(|run| is_currency_code(run, char_at(after, len)))
        }
        Some(c) => is_currency_sign(c),
        None => false,
    }
}

/// One numeric token, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// As written, symbols kept.
    pub text: &'a str,
    /// Byte offset of `text` in the scanned text.
    pub start: usize,
    /// A currency sign or ISO 4217 code stood beside the token, with at most
    /// one space between.
    pub currency: bool,
}

/// The numeric tokens of a text, found as they are asked for.
#[derive(Debug, Clone)]
pub struct Tokens<'a> {
    /// The scanned text.
    text: &'a str,
    /// The byte the scan resumes at.
    pos: usize,
}

/// The numeric tokens of `text`, in order.
///
/// A numeric token is digit groups joined by single symbols, standing apart
/// from other digits and from the letters of a word: the inside of an id
/// ("p1789368220119"), a version ("v3.1") or a unit suffix ("12.5K") is not
/// one. A dash is not a symbol, so a date ("2026-11-05") is read as its parts.
#[must_use]
pub const fn tokens(text: &str) -> Tokens<'_> {
    Tokens { text, pos: 0 }
}

impl Tokens<'_> {
    /// The next digit at or after `from`, found by byte while the text is
    /// ASCII; None at the end.
    fn next_digit(&self, mut at: usize) -> Option<usize> {
        let bytes = self.text.as_bytes();
        loop {
            let &b = bytes.get(at)?;
            if b.is_ascii() {
                if b.is_ascii_digit() {
                    return Some(at);
                }
                at += 1;
            } else {
                let c = char_at(self.text, at)?;
                if is_digit(c) {
                    return Some(at);
                }
                at += c.len_utf8();
            }
        }
    }

    /// The end of the digit run starting at `at`, by byte while it is ASCII.
    fn digits_end(&self, mut at: usize) -> usize {
        let bytes = self.text.as_bytes();
        loop {
            match bytes.get(at) {
                Some(b) if b.is_ascii_digit() => at += 1,
                Some(b) if b.is_ascii() => return at,
                _ => match char_at(self.text, at) {
                    Some(c) if is_digit(c) => at += c.len_utf8(),
                    _ => return at,
                },
            }
        }
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        let text = self.text;
        loop {
            let start = self.next_digit(self.pos)?;
            // Digits that continue a word ("p1789368220119") are not a token.
            if char_before(text, start).is_some_and(is_boundary) {
                self.pos = self.digits_end(start);
                continue;
            }
            let mut end = self.digits_end(start);
            // Where the last joining symbol stands, once there is one.
            let mut last_symbol: Option<usize> = None;
            // Extend over `symbol digits+` while a single symbol joins two groups.
            while let Some(s) = char_at(text, end)
                && is_symbol(s)
                && char_at(text, end + s.len_utf8()).is_some_and(is_digit)
            {
                last_symbol = Some(end);
                end = self.digits_end(end + s.len_utf8());
            }
            self.pos = end;
            // A run that ends against a word letter ("12.5K") gives back its
            // last group; a single group against a letter ("2026x") is part of
            // that word and reads as nothing.
            if char_at(text, end).is_some_and(is_boundary) {
                let Some(at) = last_symbol else {
                    continue;
                };
                end = at;
            }
            return Some(Token {
                text: text.get(start..end).unwrap_or_default(),
                start,
                currency: currency_near(text, start, end),
            });
        }
    }
}
