//! Percentages: a figure with a percent sign or word after it, and what a
//! reply's is held to.

use super::digits::digit_value;
use super::scan::{Scratch, after_marks, is_boundary, is_inline_space, tokens};
use super::{Reading, scaled};
use crate::text::char_at;

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

/// Whether `r`, a reading of a token that ends at byte `end` before a percent
/// mark, is the percentage it marks: the whole token, or the last of the
/// numbers a run sets side by side. A group of a thousands run ("4" of
/// "4,552,788%") is none.
const fn before_sign(r: &Reading<'_, '_>, end: usize) -> bool {
    !r.derived && r.span.end == end
}

/// One reading of a percentage the reply states.
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
}

/// One percentage the reply states: its readings are alternatives ("1,250%"
/// may be 1250 or 1.25), and the corpus grounds it by carrying any one.
struct Claim<'r> {
    /// As the reply wrote it, sign and mark included.
    text: &'r str,
    /// Its readings.
    readings: Vec<Stated>,
}

/// Percentages in `reply` that `corpus` does not carry, once each, as the
/// reply wrote them: a ratio worked out in prose, where the base is the step
/// that slips.
///
/// A percentage is a figure with a percent sign after it, in any script
/// ("12,5 %", "٥٠٪", "１２％"), or a percent word ("12 percent", "12 pour
/// cent"). The corpus carries one with a percentage that rounds to it at the
/// places the reply wrote ("42.67%" carries "42.7%" and "43%"), or with a
/// fraction of two places or more that a script printed for it ("0.1429"
/// carries "14.3%"). A bare number never does, since it may be either
/// operand. The size is compared, not the sign ("fell 30%" against
/// "-29.9%"), and none and all (0% and 100%) are never held.
///
/// # Examples
///
/// ```
/// let corpus = "Sign-ups $19.16 per lead, Add2Slack $13.43; overall_rate = 0.1429";
/// let reply = "Overall 14.3% converted, and Sign-ups costs 30% more per lead.";
/// assert_eq!(figures_parser::ungrounded_percentages(reply, corpus), ["30%"]);
/// ```
#[must_use]
pub fn ungrounded_percentages(reply: &str, corpus: &str) -> Vec<String> {
    let mut scratch = Scratch::default();
    let mut claims: Vec<Claim<'_>> = Vec::new();
    for token in tokens(reply) {
        if token.magnitude > 0 {
            continue;
        }
        let end = token.end();
        let Some(mark_end) = percent_after(reply, end) else {
            continue;
        };
        let from = end.saturating_sub(token.full.len());
        let text = reply.get(from..mark_end).unwrap_or(token.full);
        if claims.iter().any(|claim| claim.text == text) {
            continue;
        }
        let mut readings = Vec::new();
        token.for_each_reading(&mut scratch, |r| {
            if before_sign(&r, end)
                && let Some(stated) = Stated::of(r.integer, r.fraction.unwrap_or_default())
            {
                readings.push(stated);
            }
        });
        if !readings.iter().all(Stated::is_bound) {
            claims.push(Claim { text, readings });
        }
    }
    if claims.is_empty() {
        return Vec::new();
    }
    for token in tokens(corpus).without_currency().without_signs() {
        if token.magnitude > 0 {
            continue;
        }
        let end = token.end();
        let percent = percent_after(corpus, end).is_some();
        // A fraction's first digit is its zero, so most tokens are passed by
        // without being read.
        if !percent && token.text.chars().next().and_then(digit_value) != Some(0) {
            continue;
        }
        token.for_each_reading(&mut scratch, |r| {
            let fraction = r.fraction.unwrap_or_default();
            let shift = if percent {
                if !before_sign(&r, end) {
                    return;
                }
                0
            } else if !r.derived && r.integer == "0" && fraction.len() >= 2 {
                2
            } else {
                return;
            };
            claims.retain(|claim| {
                !claim
                    .readings
                    .iter()
                    .any(|stated| stated.carried_by(r.integer, fraction, shift))
            });
        });
        if claims.is_empty() {
            return Vec::new();
        }
    }
    claims
        .into_iter()
        .map(|claim| claim.text.to_owned())
        .collect()
}
