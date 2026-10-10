//! Numeric tokens of a text: digit groups joined by single symbols.
//!
//! An ASCII character is decided by its byte before any Unicode table is
//! consulted; `benches/` measures the scan.

use unicode_general_category::{GeneralCategory, get_general_category};

use super::Sign;
use super::digits::digit_value;
pub use super::readings::Scratch;
use super::scale::{may_scale, scale_after};
use crate::text::{char_at, char_before, is_letter, is_unspaced_script};

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

/// Whether `c` ends a token: another digit, or a letter of a word.
pub(super) fn is_boundary(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric();
    }
    if is_digit(c) {
        return true;
    }
    is_letter(c) && !is_unspaced_script(c)
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
pub(super) fn marked_before(text: &str, start: usize) -> bool {
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

/// Whether `c` is a bidi mark a locale writes around a sign: the left-to-right
/// and right-to-left marks, and the Arabic letter mark ("؜-١٬٢٣٤").
const fn is_bidi_mark(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{61c}')
}

/// Whether `c` is a minus sign: the hyphen-minus, the minus sign Swedish,
/// Finnish or Croatian print (U+2212), or the fullwidth hyphen-minus.
const fn is_minus(c: char) -> bool {
    matches!(c, '-' | '\u{2212}' | '\u{ff0d}')
}

/// Whether `c` is a plus sign, halfwidth or fullwidth.
const fn is_plus(c: char) -> bool {
    matches!(c, '+' | '\u{ff0b}')
}

/// Whether `c` marks the figure after it as approximate: a tilde, the tilde
/// operator or the almost-equal sign ("~12,500", "≈ 1.2"). The wave dashes
/// CJK text writes (`〜`, `～`) mean a range or "up to" there, so they are
/// not marks.
const fn is_approximation_mark(c: char) -> bool {
    matches!(c, '~' | '\u{223c}' | '\u{2248}')
}

/// The byte the bidi marks right before `at` start at.
fn before_marks(text: &str, mut at: usize) -> usize {
    while let Some(c) = char_before(text, at).filter(|&c| is_bidi_mark(c)) {
        at = at.saturating_sub(c.len_utf8());
    }
    at
}

/// The byte just past the bidi marks right after `at`.
fn after_marks(text: &str, mut at: usize) -> usize {
    while let Some(c) = char_at(text, at).filter(|&c| is_bidi_mark(c)) {
        at = at.saturating_add(c.len_utf8());
    }
    at
}

/// Where a currency mark right before byte `at` begins, with at most one
/// space between: a sign with any capitals glued before it ("$", "US$",
/// "R$"), or an ISO 4217 code ("USD ").
fn currency_start(text: &str, end: usize) -> Option<usize> {
    let before = text.get(..end)?;
    let at = before_marks(
        text,
        before
            .strip_suffix(char::is_whitespace)
            .unwrap_or(before)
            .len(),
    );
    let c = char_before(text, at)?;
    if c.is_ascii_uppercase() {
        let head = text
            .get(..at)?
            .trim_end_matches(|w: char| w.is_ascii_alphanumeric());
        let run = text.get(head.len()..at)?;
        return is_currency_code(run, char_before(head, head.len())).then_some(head.len());
    }
    if !is_currency_sign(c) {
        return None;
    }
    let sign_at = at.saturating_sub(c.len_utf8());
    let head = text
        .get(..sign_at)?
        .trim_end_matches(|w: char| w.is_ascii_uppercase());
    Some(head.len())
}

/// Whether a currency mark, with at most one space before it, and then a
/// closing parenthesis follow byte `end` ("…)", "… $US)", "… USD)").
fn closes(text: &str, end: usize) -> bool {
    let Some(rest) = text.get(after_marks(text, end)..) else {
        return false;
    };
    if rest.starts_with(')') {
        return true;
    }
    let rest = rest.strip_prefix(char::is_whitespace).unwrap_or(rest);
    let capitals = |r: &str| r.len() - r.trim_start_matches(|c: char| c.is_ascii_uppercase()).len();
    let lead = capitals(rest);
    let after_lead = rest.get(lead..).unwrap_or_default();
    let after_sign = after_lead.strip_prefix(is_currency_sign);
    let tail = match after_sign {
        Some(after) => after.get(capitals(after)..).unwrap_or_default(),
        None if lead == 3 && is_currency_code(rest.get(..3).unwrap_or_default(), None) => {
            after_lead
        }
        None => return false,
    };
    tail.trim_start_matches(is_bidi_mark).starts_with(')')
}

/// The sign written right before byte `at`, and where it starts: a minus or
/// a plus with only bidi marks after it, standing after no digit or word
/// letter (after one, a dash joins: "2026-11-05", "CORE-12", "10-12"),
/// unless that word is a currency code ("EUR-1'234.50").
fn sign_at(text: &str, at: usize) -> Option<(Sign, usize)> {
    let c = char_before(text, at)?;
    let sign = if is_minus(c) {
        Sign::Minus
    } else if is_plus(c) {
        Sign::Plus
    } else {
        return None;
    };
    let start = at.saturating_sub(c.len_utf8());
    let edge = before_marks(text, start);
    let joined =
        char_before(text, edge).is_some_and(|b| is_boundary(b) || is_minus(b) || is_plus(b));
    let after_code = joined && currency_start(text, edge).is_some_and(|code| code < edge);
    (!joined || after_code).then_some((sign, start))
}

/// Whether `c` is a space within a line: the space, the no-break space or
/// the narrow no-break space French writes after a mark.
pub(super) const fn is_inline_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}')
}

/// Where an approximation mark right before byte `at` starts, with at most
/// one space of the same line between ("~12", "≈ 12"), standing after no
/// digit or word letter: after a digit, with or without a space, a tilde is
/// a range ("10~20", "10 ~ 20").
fn approximation_at(text: &str, at: usize) -> Option<usize> {
    let before = text.get(..at)?;
    let mark_end = before_marks(
        text,
        before.strip_suffix(is_inline_space).unwrap_or(before).len(),
    );
    let c = char_before(text, mark_end).filter(|&c| is_approximation_mark(c))?;
    let start = mark_end.saturating_sub(c.len_utf8());
    let edge = before_marks(text, start);
    let head = text.get(..edge)?;
    let spaced = head.strip_suffix(char::is_whitespace);
    let joined = char_before(head, edge).is_some_and(is_boundary)
        || spaced.is_some_and(|h| char_before(h, h.len()).is_some_and(is_digit));
    (!joined).then_some(start)
}

/// Whether the text before `start` may hold a sign, an approximation mark, a
/// currency mark or an opening parenthesis, read from the one or two
/// characters before it, so most figures are settled without the full
/// look-back: a dash after a digit or a lowercase letter joins
/// ("2026-11-05", "x-12"), and a space leads one only after a capital, a
/// currency sign, an approximation mark or a bidi mark ("USD 12", "€ 12",
/// "~ 12").
fn may_have_prefix(text: &str, start: usize, prev: char) -> bool {
    let before = || char_before(text, start.saturating_sub(prev.len_utf8()));
    if is_minus(prev) || is_plus(prev) {
        !before().is_some_and(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
    } else if prev.is_whitespace() {
        before().is_some_and(|b| {
            b.is_ascii_uppercase()
                || is_approximation_mark(b)
                || is_bidi_mark(b)
                || is_currency_sign(b)
        })
    } else {
        prev == '(' || is_approximation_mark(prev) || is_bidi_mark(prev) || is_currency_sign(prev)
    }
}

/// What is written before a figure: its sign, whether it is marked
/// approximate, and the byte the figure starts at with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Prefix {
    /// The sign.
    pub sign: Sign,
    /// Whether an approximation mark stands before it.
    pub approximate: bool,
    /// Where the sign or the approximation mark starts; the token's own
    /// start when neither is written.
    pub from: usize,
}

/// What is written before the token `start..end`. The sign is a minus or a
/// plus before the digits, or before a currency mark that is before them
/// ("-12", "−1 234", "-$12", "$-12", "€ -12", "EUR-1'234"), or parentheses
/// around the token and its currency mark, as accounting prints a negative
/// ("($1,234.50)", "(1 234,50 $US)"). The approximation mark stands before
/// all of them ("~12,500", "≈ -$12", "~€ 1.234,50"). [`may_have_prefix`]
/// says, from the characters right before the token, when to look.
pub(super) fn look_back(text: &str, start: usize, end: usize) -> Prefix {
    let digits = before_marks(text, start);
    let currency = currency_start(text, digits).map(|at| before_marks(text, at));
    let signed = sign_at(text, digits).or_else(|| currency.and_then(|at| sign_at(text, at)));
    let open = currency.unwrap_or(digits);
    let (sign, lead) = if let Some((sign, at)) = signed {
        (sign, at.min(open))
    } else if char_before(text, open) == Some('(') && closes(text, end) {
        (Sign::Parenthesized, open)
    } else {
        (Sign::Unsigned, open)
    };
    let mark = approximation_at(text, lead);
    Prefix {
        sign,
        approximate: mark.is_some(),
        from: mark.or_else(|| signed.map(|(_, at)| at)).unwrap_or(start),
    }
}

/// Whether a currency sign or code opens the text after byte `end`, with at
/// most one space between ("99 USD", "12 €").
pub(super) fn marked_after(text: &str, end: usize) -> bool {
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
    /// The sign written before it.
    pub sign: Sign,
    /// Whether an approximation mark stands before it ("~12,500").
    pub approximate: bool,
    /// The power of ten a scale word after the digits multiplies it by
    /// ("1.2 million", "12.5K"); 0 without one.
    pub magnitude: u32,
    /// The token through its scale word ("1.2 million"); the same as `text`
    /// without one.
    pub written: &'a str,
    /// The token from its sign or approximation mark on, through its scale
    /// word ("~-$1.2 million"), as a reply wrote it; the same as `written`
    /// when nothing stands before it.
    pub full: &'a str,
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
    /// Whether a token reads the sign and the approximation mark before it.
    signs: bool,
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
        signs: true,
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

    /// These tokens without reading the sign or the approximation mark
    /// before them, so every token is unsigned and exact: for a corpus, where
    /// a sign matters only beside a value the reply states, and is read
    /// there.
    #[must_use]
    pub const fn without_signs(mut self) -> Self {
        self.signs = false;
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
            let prev = char_before(text, start);
            // Digits that continue a word ("p1789368220119") are not a token.
            if prev.is_some_and(is_boundary) {
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
            let scale = if may_scale(text, end) {
                next.and_then(|n| scale_after(text, start, end, n))
            } else {
                None
            };
            // A run that ends against a word letter ("12.5kg") gives back its
            // last group; a single group against a letter ("2026x") is part of
            // that word and reads as nothing.
            if scale.is_none() && next.is_some_and(is_boundary) {
                let Some(at) = last_symbol else {
                    continue;
                };
                end = at;
            }
            let written_end = scale.map_or(end, |scale| scale.end);
            self.pos = self.pos.max(written_end);
            let token = text.get(start..end).unwrap_or_default();
            let written = if scale.is_some() {
                text.get(start..written_end).unwrap_or(token)
            } else {
                token
            };
            let prefix = if self.signs && prev.is_some_and(|p| may_have_prefix(text, start, p)) {
                look_back(text, start, written_end)
            } else {
                Prefix {
                    sign: Sign::Unsigned,
                    approximate: false,
                    from: start,
                }
            };
            return Some(Token {
                text: token,
                start,
                marked_before: self.marks && marked_before(text, start),
                marked_after: self.marks && marked_after(text, written_end),
                sign: prefix.sign,
                approximate: prefix.approximate,
                magnitude: scale.map_or(0, |scale| scale.magnitude),
                written,
                full: if prefix.from == start {
                    written
                } else {
                    text.get(prefix.from..written_end).unwrap_or(written)
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Prefix, Sign, char_before, look_back, may_have_prefix, tokens};

    /// The fast path skips only figures the full look-back reads as unsigned
    /// and exact, over every arrangement of up to three pieces before a
    /// figure.
    #[test]
    fn the_fast_path_skips_only_unprefixed_figures() {
        let pieces = [
            "", "-", "−", "＋", "+", "(", "$", "€", "US", "EUR", " ", "\u{a0}", "\u{200e}",
            "\u{200f}", "\u{61c}", "a", "Z", "1", "_", "は", "é", ":", "~", "≈", "∼",
        ];
        for a in pieces {
            for b in pieces {
                for c in pieces {
                    let text = format!("{a}{b}{c}12,500 $US)");
                    for token in tokens(&text) {
                        let end = token.start + token.text.len();
                        let full = look_back(&text, token.start, end);
                        let fast = char_before(&text, token.start)
                            .is_some_and(|p| may_have_prefix(&text, token.start, p));
                        let none = Prefix {
                            sign: Sign::Unsigned,
                            approximate: false,
                            from: token.start,
                        };
                        assert!(
                            fast || full == none,
                            "{text:?} at {}: {full:?}",
                            token.start
                        );
                    }
                }
            }
        }
    }
}
