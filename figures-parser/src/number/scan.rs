//! Numeric tokens of a text: digit groups joined by single symbols.

use std::iter::Peekable;
use std::str::CharIndices;

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

/// One numeric token, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// As written, symbols kept.
    pub text: &'a str,
    /// Byte offset of `text` in the scanned text.
    pub start: usize,
    /// A currency sign stood right before the token, with at most one space between.
    pub currency: bool,
}

/// The numeric tokens of a text, found as they are asked for.
#[derive(Debug, Clone)]
pub struct Tokens<'a> {
    /// The scanned text.
    text: &'a str,
    /// The characters not yet scanned.
    chars: Peekable<CharIndices<'a>>,
    /// The character before the next one, and the one before that.
    before: (Option<char>, Option<char>),
}

/// The numeric tokens of `text`, in order.
///
/// A numeric token is digit groups joined by single symbols, standing apart
/// from other digits and from the letters of a word: the inside of an id
/// ("p1789368220119"), a version ("v3.1") or a unit suffix ("12.5K") is not
/// one. A dash is not a symbol, so a date ("2026-11-05") is read as its parts.
#[must_use]
pub fn tokens(text: &str) -> Tokens<'_> {
    Tokens {
        text,
        chars: text.char_indices().peekable(),
        before: (None, None),
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        loop {
            let (start, c) = self.chars.next()?;
            if !is_digit(c) || self.before.0.is_some_and(is_boundary) {
                self.before = (Some(c), self.before.0);
                continue;
            }
            let currency = {
                let sign = if self.before.0.is_some_and(char::is_whitespace) {
                    self.before.1
                } else {
                    self.before.0
                };
                sign.is_some_and(is_currency_sign)
            };
            let mut end = start + c.len_utf8();
            let mut last = c;
            // Where the last joining symbol stands, once there is one.
            let mut last_symbol: Option<usize> = None;
            loop {
                while let Some(&(at, d)) = self.chars.peek() {
                    if !is_digit(d) {
                        break;
                    }
                    self.chars.next();
                    end = at + d.len_utf8();
                    last = d;
                }
                // Extend over `symbol digits+` while a single symbol joins two groups.
                let joins = self.chars.peek().is_some_and(|&(at, s)| {
                    is_symbol(s)
                        && self
                            .text
                            .get(at + s.len_utf8()..)
                            .and_then(|rest| rest.chars().next())
                            .is_some_and(is_digit)
                });
                if !joins {
                    break;
                }
                last_symbol = self.chars.next().map(|(at, _)| at);
            }
            self.before = (Some(last), None);
            // A run that ends against a word letter ("12.5K") gives back its
            // last group; a single group against a letter ("2026x") is part of
            // that word and reads as nothing.
            if self.chars.peek().is_some_and(|&(_, n)| is_boundary(n)) {
                let Some(at) = last_symbol else {
                    continue;
                };
                end = at;
            }
            return Some(Token {
                text: self.text.get(start..end).unwrap_or_default(),
                start,
                currency,
            });
        }
    }
}
