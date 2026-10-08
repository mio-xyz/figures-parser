//! Numeric tokens of a text: digit groups joined by single symbols.

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
    get_general_category(c) == GeneralCategory::CurrencySymbol
}

/// One numeric token, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// As written, symbols kept.
    pub text: &'a str,
    /// Byte offset of `text` in the scanned text.
    pub start: usize,
    /// The digit groups, as written.
    pub groups: Vec<&'a str>,
    /// The symbol after each group but the last.
    pub symbols: Vec<char>,
    /// A currency sign stood right before the token, with at most one space between.
    pub currency: bool,
}

/// The numeric tokens of `text`, in order.
///
/// A numeric token is digit groups joined by single symbols, standing apart
/// from other digits and from the letters of a word: the inside of an id
/// ("p1789368220119"), a version ("v3.1") or a unit suffix ("12.5K") is not
/// one. A dash is not a symbol, so a date ("2026-11-05") is read as its parts.
#[must_use]
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    // The character before the current one, and the one before that.
    let mut before: (Option<char>, Option<char>) = (None, None);
    while let Some((start, c)) = chars.next() {
        if !is_digit(c) || before.0.is_some_and(is_boundary) {
            before = (Some(c), before.0);
            continue;
        }
        let currency = {
            let sign = if before.0.is_some_and(char::is_whitespace) {
                before.1
            } else {
                before.0
            };
            sign.is_some_and(is_currency_sign)
        };
        let mut groups = Vec::new();
        let mut symbols = Vec::new();
        let mut group_start = start;
        let mut end = start + c.len_utf8();
        let mut last = c;
        loop {
            while let Some(&(at, d)) = chars.peek() {
                if !is_digit(d) {
                    break;
                }
                chars.next();
                end = at + d.len_utf8();
                last = d;
            }
            groups.push(text.get(group_start..end).unwrap_or_default());
            // Extend over `symbol digits+` while a single symbol joins two groups.
            let joined = chars.peek().and_then(|&(at, s)| {
                if !is_symbol(s) {
                    return None;
                }
                let next = at + s.len_utf8();
                let d = text.get(next..)?.chars().next()?;
                is_digit(d).then_some((s, next))
            });
            let Some((symbol, next)) = joined else {
                break;
            };
            chars.next();
            symbols.push(symbol);
            group_start = next;
        }
        before = (Some(last), None);
        // A run that ends against a word letter ("12.5K") gives back its
        // last group; a single group against a letter ("2026x") is part of
        // that word and reads as nothing.
        if chars.peek().is_some_and(|&(_, n)| is_boundary(n)) {
            if groups.len() == 1 {
                continue;
            }
            groups.pop();
            symbols.pop();
        }
        let len = groups.iter().map(|g| g.len()).sum::<usize>()
            + symbols.iter().map(|s| s.len_utf8()).sum::<usize>();
        out.push(Token {
            text: text.get(start..start + len).unwrap_or_default(),
            start,
            groups,
            symbols,
            currency,
        });
    }
    out
}
