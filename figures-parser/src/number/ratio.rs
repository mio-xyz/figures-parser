//! Ratios: a percentage or a multiple written in a text, and what a reply's
//! is held to.

use std::ops::Range;

use unicode_general_category::{GeneralCategory, get_general_category};

use super::digits::digit_value;
use super::scan::{
    Scratch, Token, after_marks, before_marks, is_boundary, is_decimal_symbol, is_inline_space,
    is_symbol, is_times_sign, tokens,
};
use super::{Reading, for_each_reading_in, scaled};
use crate::text::{char_at, char_before};

/// The percent signs: ASCII's, the Arabic one, and the fullwidth and small
/// forms.
const fn is_percent_sign(c: char) -> bool {
    matches!(c, '%' | '\u{66a}' | '\u{ff05}' | '\u{fe6a}')
}

/// Percent as a word after the figure, in the languages whose number words
/// are read; matched ignoring ASCII case.
const PERCENT_WORDS: [&str; 7] = [
    "percent",
    "per cent",
    "pour cent",
    "prozent",
    "por ciento",
    "por cento",
    "per cento",
];

/// Past this many decimal places a printed value is cut, far below any
/// precision a reply states.
const MAX_PLACES: u32 = 30;

/// The person's numbers whose pairwise ratios are read: a message carries a
/// few.
const MAX_SAID_NUMBERS: usize = 30;

/// Where the percent mark after byte `end` of `text` ends: a percent sign
/// after at most one space of the line and any bidi marks ("12%", "12,5 %",
/// "٥٠٪"), or a percent word after one space ("12 percent"). None without
/// one, or when a letter or a digit continues past it ("50%off", "12
/// percentage points", the "%25" of an encoded link).
fn percent_after(text: &str, end: usize) -> Option<usize> {
    let rest = text.get(end..)?;
    let spaced = rest.strip_prefix(is_inline_space);
    let after_space = spaced.unwrap_or(rest);
    // Every sign, word and bidi mark starts with one of these bytes, which
    // turns most tokens away before any is looked for.
    if !matches!(
        after_space.bytes().next(),
        Some(b'%' | b'p' | b'P' | 0x80..)
    ) {
        return None;
    }
    let space_end = text.len().saturating_sub(after_space.len());
    let ends = |mark_end: usize| !char_at(text, mark_end).is_some_and(is_boundary);
    let at = after_marks(text, space_end);
    if let Some(sign) = char_at(text, at).filter(|&c| is_percent_sign(c)) {
        let mark_end = at.saturating_add(sign.len_utf8());
        return ends(mark_end).then_some(mark_end);
    }
    spaced?;
    PERCENT_WORDS
        .iter()
        .filter(|word| {
            after_space
                .get(..word.len())
                .is_some_and(|written| written.eq_ignore_ascii_case(word))
        })
        .map(|word| space_end.saturating_add(word.len()))
        .find(|&mark_end| ends(mark_end))
}

/// Where the multiplication sign glued to the figure that ends at byte `end`
/// of `text` ends ("8.9x", "3×", "1,250X"). None when a letter or a digit
/// continues past it, which makes it a size ("1920x1080") or a hex literal
/// ("0x1F"); a space before it makes it a product ("3 x 4").
fn times_after(text: &str, end: usize) -> Option<usize> {
    let sign = char_at(text, end).filter(|&c| is_times_sign(c))?;
    let mark_end = end.saturating_add(sign.len_utf8());
    (!char_at(text, mark_end).is_some_and(is_boundary)).then_some(mark_end)
}

/// Whether `c` may stand right before a percent sign that precedes its
/// figure: whitespace, or an opening bracket or quote.
const fn opens(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '(' | '[' | '{' | '"' | '\'' | '\u{201c}' | '\u{2018}' | '\u{ab}'
        )
}

/// Where the percent sign before the figure that starts at byte `start` of
/// `text` starts, as Turkish and Basque write one ("%11", "% 11"): bidi
/// marks and at most one space of the line between, and nothing, whitespace
/// or an opening bracket or quote before the sign, so the "%20" of an
/// encoded link is none.
fn percent_before(text: &str, start: usize) -> Option<usize> {
    let at = before_marks(text, start);
    let at = match char_before(text, at) {
        Some(c) if is_inline_space(c) => at.saturating_sub(c.len_utf8()),
        _ => at,
    };
    let sign = char_before(text, at).filter(|&c| is_percent_sign(c))?;
    let sign_at = at.saturating_sub(sign.len_utf8());
    char_before(text, sign_at)
        .is_none_or(opens)
        .then_some(sign_at)
}

/// A ratio's mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    /// A percent sign or word.
    Percent,
    /// A multiplication sign.
    Times,
}

/// One ratio written in a text.
#[derive(Debug, Clone)]
pub(super) struct Ratio<'a> {
    /// Its figure.
    pub token: Token<'a>,
    /// From the figure's sign or approximation mark, or the percent sign
    /// before it, through the mark after it ("~15%", "%11", "8.9x").
    pub span: Range<usize>,
    /// Its mark.
    pub mark: Mark,
    /// Whether the mark stands before the figure ("%11").
    pub sign_first: bool,
}

impl Ratio<'_> {
    /// Whether `r`, a reading of this ratio's token, is the figure its mark
    /// marks: the whole token, or of the numbers a run sets side by side the
    /// one against the mark. A group of a thousands run ("4" of
    /// "4,552,788%") is none.
    const fn is_figure(&self, r: &Reading<'_, '_>) -> bool {
        !r.derived
            && if self.sign_first {
                r.span.start == self.token.start
            } else {
                r.span.end == self.token.end()
            }
    }
}

/// The ratio `token`, found in `text`, is the figure of, if any: a token
/// without a scale word, with a percent mark after it, a multiplication sign
/// glued after it, or a percent sign before it.
fn ratio_of<'a>(text: &'a str, token: Token<'a>) -> Option<Ratio<'a>> {
    if token.magnitude > 0 {
        return None;
    }
    let end = token.end();
    let full_start = end.saturating_sub(token.full.len());
    if let Some(mark_end) = percent_after(text, end) {
        return Some(Ratio {
            token,
            span: full_start..mark_end,
            mark: Mark::Percent,
            sign_first: false,
        });
    }
    if let Some(mark_end) = times_after(text, end) {
        return Some(Ratio {
            token,
            span: full_start..mark_end,
            mark: Mark::Times,
            sign_first: false,
        });
    }
    let mark_start = percent_before(text, full_start)?;
    Some(Ratio {
        token,
        span: mark_start..end,
        mark: Mark::Percent,
        sign_first: true,
    })
}

/// Every ratio written in `text`, in order.
pub(super) fn ratios(text: &str) -> impl Iterator<Item = Ratio<'_>> {
    tokens(text).filter_map(move |token| ratio_of(text, token))
}

/// One reading of a ratio the reply states.
struct Stated {
    /// The value at `places` decimal places: 427 for "42.7".
    value: u128,
    /// How many decimal places the reply wrote.
    places: u32,
}

impl Stated {
    /// The reading `integer.fraction`; None past what a u128 holds.
    fn of(integer: &str, fraction: &str) -> Option<Self> {
        let places = u32::try_from(fraction.len()).ok()?;
        let value = scaled(integer, Some(fraction), places)?;
        Some(Self { value, places })
    }

    /// None or all, which a count shows without a division.
    fn is_bound(&self) -> bool {
        self.value == 0
            || 10_u128
                .checked_pow(self.places)
                .and_then(|p| p.checked_mul(100))
                == Some(self.value)
    }

    /// Whether a printed value, `integer.fraction` times `10^shift`, rounds
    /// to this one at the places it is written with: "42.67" carries "42.7"
    /// and "43", not "42.6".
    fn carried_by(&self, integer: &str, fraction: &str, shift: u32) -> bool {
        let printed_places = u32::try_from(fraction.len())
            .unwrap_or(u32::MAX)
            .saturating_sub(shift);
        let scale = self
            .places
            .saturating_add(1)
            .max(printed_places)
            .min(MAX_PLACES);
        let Some(below) = scale
            .checked_sub(self.places)
            .and_then(|d| d.checked_sub(1))
        else {
            return false;
        };
        let half = 10_u128.checked_pow(below).and_then(|p| p.checked_mul(5));
        let printed = scale
            .checked_add(shift)
            .and_then(|s| scaled(integer, Some(fraction), s));
        let stated = 10_u128
            .checked_pow(below.saturating_add(1))
            .and_then(|p| self.value.checked_mul(p));
        match (half, printed, stated) {
            (Some(half), Some(printed), Some(stated)) => printed.abs_diff(stated) <= half,
            _ => false,
        }
    }

    /// Whether a value worked out of the person's numbers rounds to this one
    /// at the places it is written with.
    fn near(&self, value: f64) -> bool {
        let places = i32::try_from(self.places).unwrap_or(i32::MAX);
        let tolerance = 0.5_f64.mul_add(10_f64.powi(-places), 1e-9);
        format!("{}e-{}", self.value, self.places)
            .parse::<f64>()
            .is_ok_and(|stated| (stated - value).abs() <= tolerance)
    }
}

/// One ratio the reply states: its readings are alternatives ("1,250%" may
/// be 1250 or 1.25), and the corpus grounds it by carrying any one.
struct Claim<'r> {
    /// Its figure as written, which names it once however its mark is
    /// written ("11 %", "11%", "%11").
    figure: &'r str,
    /// Its mark.
    mark: Mark,
    /// As the reply wrote it, spaces and format characters dropped.
    text: String,
    /// Its readings.
    readings: Vec<Stated>,
}

/// Whether `token` is a decimal a result printed ("8.857", "0,113"): one
/// decimal symbol between two digit groups. Money keeps its grouping
/// ("1,234.50") and is none.
fn is_plain_decimal(token: &Token<'_>) -> bool {
    let mut symbols = token.text.chars().filter(|&c| is_symbol(c));
    match (symbols.next(), symbols.next()) {
        (Some(symbol), None) => is_decimal_symbol(symbol),
        _ => false,
    }
}

/// Drops the claims a printed value, `integer.fraction` times `10^shift`,
/// carries.
fn retain(claims: &mut Vec<Claim<'_>>, integer: &str, fraction: &str, shift: u32) {
    claims.retain(|claim| {
        !claim
            .readings
            .iter()
            .any(|stated| stated.carried_by(integer, fraction, shift))
    });
}

/// The ratios plain arithmetic over two of the numbers `said` states gives
/// (a share, a multiple, a rise or a drop), each also as a percentage, and
/// the numbers themselves: the arithmetic over a few numbers the person
/// supplied that is left to prose.
fn said_ratios(said: &str) -> Vec<f64> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut scratch = Scratch::default();
    let mut written = String::new();
    let read = tokens(said)
        .against_letters()
        .without_currency()
        .without_signs();
    for_each_reading_in(said, read, &mut scratch, |r| {
        if numbers.len() >= MAX_SAID_NUMBERS {
            return;
        }
        written.clear();
        written.push_str(r.integer);
        if let Some(fraction) = r.fraction {
            written.push('.');
            written.push_str(fraction);
        }
        if let Ok(value) = written.parse::<f64>()
            && !numbers.iter().any(|n| n.to_bits() == value.to_bits())
        {
            numbers.push(value);
        }
    });
    let mut out = numbers.clone();
    for &a in &numbers {
        for &b in &numbers {
            if b == 0.0 {
                continue;
            }
            for ratio in [a / b, (a / b - 1.0).abs()] {
                out.push(ratio);
                out.push(ratio * 100.0);
            }
        }
    }
    out
}

/// Ratios in `reply` that neither `corpus` nor `said` supports, once each,
/// as the reply wrote them.
///
/// Such a ratio is a share, a multiple or a change in percent worked out in
/// prose from two printed amounts, where the base is the step that slips. A
/// ratio is a figure with a percent sign after it, in any script
/// ("12,5 %", "٥٠٪", "１２％"), before it as Turkish writes it ("%11"), or a
/// percent word after it ("12 percent", "12 pour cent"), or a figure with a
/// multiplication sign glued after it ("8.9x", "3×", "1,250X"); a letter or
/// a digit past the sign makes a size ("1920x1080") or a hex literal
/// ("0x1F"), and a space before it a product ("3 x 4"). The corpus carries a
/// ratio with the same ratio printed, rounding to it at the places the reply
/// wrote ("42.67%" carries "42.7%" and "43%"), or with a decimal a script
/// printed that rounds to it as is or a hundred times it ("0.1429" and
/// "14.29" both carry "14.3%", "8.857" carries "8.9x"). A bare whole number
/// never does, since it may be either operand. `said`, the person's own
/// words, carries a ratio with a number they stated ("up 12 percent") or
/// with plain arithmetic over two of their numbers: a quotient, or a change,
/// each also as a percentage. The size is compared, not the sign ("fell 30%"
/// against "-29.9%"), and none and all (0% and 100%) are never held. One
/// ratio is named once however its mark is written.
///
/// # Examples
///
/// ```
/// let corpus = "Sign-ups $19.16 per lead, Add2Slack $13.43; overall_rate = 0.1429";
/// let reply = "Overall 14.3% converted, and Sign-ups costs 30% more per lead.";
/// assert_eq!(figures_parser::ungrounded_ratios(reply, corpus, ""), ["30%"]);
///
/// // The person's own numbers give the share.
/// let said = "1000 trials, 420 activated";
/// assert!(figures_parser::ungrounded_ratios("58% never activate.", "", said).is_empty());
/// ```
#[must_use]
pub fn ungrounded_ratios(reply: &str, corpus: &str, said: &str) -> Vec<String> {
    let mut scratch = Scratch::default();
    let mut claims: Vec<Claim<'_>> = Vec::new();
    for ratio in ratios(reply) {
        let figure = ratio.token.text;
        if claims
            .iter()
            .any(|claim| claim.figure == figure && claim.mark == ratio.mark)
        {
            continue;
        }
        let mut readings = Vec::new();
        ratio.token.for_each_reading(&mut scratch, |r| {
            if ratio.is_figure(&r)
                && let Some(stated) = Stated::of(r.integer, r.fraction.unwrap_or_default())
            {
                readings.push(stated);
            }
        });
        if ratio.mark == Mark::Percent && readings.iter().all(Stated::is_bound) {
            continue;
        }
        let text = reply
            .get(ratio.span.clone())
            .unwrap_or(figure)
            .chars()
            .filter(|&c| !c.is_whitespace() && get_general_category(c) != GeneralCategory::Format)
            .collect();
        claims.push(Claim {
            figure,
            mark: ratio.mark,
            text,
            readings,
        });
    }
    if claims.is_empty() {
        return Vec::new();
    }
    for token in tokens(corpus)
        .against_letters()
        .without_currency()
        .without_signs()
    {
        if token.magnitude > 0 {
            continue;
        }
        if let Some(ratio) = ratio_of(corpus, token.clone()) {
            token.for_each_reading(&mut scratch, |r| {
                if ratio.is_figure(&r) {
                    retain(&mut claims, r.integer, r.fraction.unwrap_or_default(), 0);
                }
            });
        } else if is_plain_decimal(&token) {
            // A comma that may group thousands does ("19,850"), though no
            // grouping writes a zero lead ("0,850"); a dot is a script's
            // decimal point.
            let zero_lead = token.text.chars().next().and_then(digit_value) == Some(0);
            let mut grouped_comma = false;
            token.for_each_reading(&mut scratch, |r| {
                if r.derived {
                    return;
                }
                grouped_comma |= r.grouped && !zero_lead && token.text.contains(',');
                if let Some(fraction) = r.fraction
                    && !grouped_comma
                {
                    retain(&mut claims, r.integer, fraction, 0);
                    retain(&mut claims, r.integer, fraction, 2);
                }
            });
        }
        if claims.is_empty() {
            return Vec::new();
        }
    }
    if !said.is_empty() {
        let printed = said_ratios(said);
        claims.retain(|claim| {
            !claim
                .readings
                .iter()
                .any(|stated| printed.iter().any(|&value| stated.near(value)))
        });
    }
    claims.into_iter().map(|claim| claim.text).collect()
}
