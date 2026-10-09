//! Numeric tokens of a text: digit groups joined by single symbols.
//!
//! An ASCII character is decided by its byte before any Unicode table is
//! consulted; `benches/` measures the scan.

use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_script::{Script, UnicodeScript};

use super::digits::digit_value;
use crate::text::{char_at, char_before};

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
pub(super) const fn is_decimal_symbol(c: char) -> bool {
    matches!(c, '.' | ',' | '\u{66b}')
}

/// Whether `c` is a group or decimal symbol a CLDR locale prints.
///
/// Comma, dot, the two apostrophes Swiss German uses, the three spaces
/// French and Russian group with, the Arabic thousands and decimal signs,
/// and the fullwidth comma. `tests/it/locales.rs` reads back the number each
/// locale prints and fails when one prints a symbol missing here.
#[must_use]
pub const fn is_symbol(c: char) -> bool {
    if c.is_ascii() {
        return matches!(c, ',' | '.' | '\'' | ' ');
    }
    matches!(
        c,
        '\u{2019}' | '\u{a0}' | '\u{202f}' | '\u{66c}' | '\u{66b}' | '\u{ff0c}'
    )
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

/// Whether `run`, the ASCII alphanumerics beside a figure, is an ISO 4217
/// code standing as a word of its own: `edge`, the character past the run,
/// must not continue it the way a letter continues a figure's word.
fn is_currency_code(run: &str, edge: Option<char>) -> bool {
    run.len() == 3
        && run.bytes().all(|b| b.is_ascii_uppercase())
        && !edge.is_some_and(is_boundary)
        && iso_currency::Currency::from_code(run).is_some()
}

/// Whether a currency sign or code ends the text before byte `start`, with
/// at most one space between ("$729", "€ 1,458", "USD 99").
fn marked_before(text: &str, start: usize) -> bool {
    let before = text.get(..start).unwrap_or_default();
    let before = before.strip_suffix(char::is_whitespace).unwrap_or(before);
    match char_before(before, before.len()) {
        Some(c) if c.is_ascii_uppercase() => {
            let head = before.trim_end_matches(|w: char| w.is_ascii_alphanumeric());
            before
                .get(head.len()..)
                .is_some_and(|run| is_currency_code(run, char_before(head, head.len())))
        }
        Some(c) => is_currency_sign(c),
        None => false,
    }
}

/// Whether a currency sign or code opens the text after byte `end`, with at
/// most one space between ("99 USD", "12 €").
fn marked_after(text: &str, end: usize) -> bool {
    let after = text.get(end..).unwrap_or_default();
    let after = after.strip_prefix(char::is_whitespace).unwrap_or(after);
    match char_at(after, 0) {
        Some(c) if c.is_ascii_uppercase() => {
            let rest = after.trim_start_matches(|w: char| w.is_ascii_alphanumeric());
            let run_len = after.len() - rest.len();
            after
                .get(..run_len)
                .is_some_and(|run| is_currency_code(run, char_at(rest, 0)))
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
    /// A currency sign or ISO 4217 code stands right before the token, with
    /// at most one space between ("$729", "USD 99").
    pub marked_before: bool,
    /// A currency sign or ISO 4217 code stands right after the token, with at
    /// most one space between ("99 USD", "12 €").
    pub marked_after: bool,
}

impl Token<'_> {
    /// Whether a currency sign or code stands beside the token on either side.
    #[must_use]
    pub const fn currency(&self) -> bool {
        self.marked_before || self.marked_after
    }
}

/// The numeric tokens of a text, found as they are asked for.
#[derive(Debug, Clone)]
pub struct Tokens<'a> {
    /// The scanned text.
    text: &'a str,
    /// The byte the scan resumes at.
    pos: usize,
    /// Whether a token reads the currency marks around it.
    marks: bool,
}

/// The numeric tokens of `text`, in order.
///
/// A numeric token is digit groups joined by single symbols, standing apart
/// from other digits and from the letters of a word: the inside of an id
/// ("p1789368220119"), a version ("v3.1") or a unit suffix ("12.5K") is not
/// one. A dash is not a symbol, so a date ("2026-11-05") is read as its parts.
#[must_use]
pub const fn tokens(text: &str) -> Tokens<'_> {
    Tokens {
        text,
        pos: 0,
        marks: true,
    }
}

impl Tokens<'_> {
    /// These tokens without reading the currency marks around them, so no
    /// token is marked: for a text whose readings only ground,
    /// such as a corpus, where no floor depends on a mark.
    #[must_use]
    pub const fn without_currency(mut self) -> Self {
        self.marks = false;
        self
    }

    /// The next digit at or after `at`, found by byte while the text is
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
            let mut next = char_at(text, end);
            // Where the last joining symbol stands, once there is one.
            let mut last_symbol: Option<usize> = None;
            // Extend over `symbol digits+` while a single symbol joins two groups.
            while let Some(s) = next
                && is_symbol(s)
                && char_at(text, end + s.len_utf8()).is_some_and(is_digit)
            {
                last_symbol = Some(end);
                end = self.digits_end(end + s.len_utf8());
                next = char_at(text, end);
            }
            self.pos = end;
            // A run that ends against a word letter ("12.5K") gives back its
            // last group; a single group against a letter ("2026x") is part of
            // that word and reads as nothing.
            if next.is_some_and(is_boundary) {
                let Some(at) = last_symbol else {
                    continue;
                };
                end = at;
            }
            return Some(Token {
                text: text.get(start..end).unwrap_or_default(),
                start,
                marked_before: self.marks && marked_before(text, start),
                marked_after: self.marks && marked_after(text, end),
            });
        }
    }
}
