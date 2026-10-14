//! Arithmetic done in prose: a figure a reply worked out of the values a
//! message gave, where no tool did the sum.

use std::ops::Range;

use super::digits::digit_value;
use super::ground::ungrounded_by;
use super::scan::{digits_start, is_boundary, is_digit, is_symbol, joins_pieces, tokens};
use super::spelled::spelled;
use crate::text::{char_at, char_before};

/// How many values a message gives before a figure worked out of them is
/// held to a printed one; from four, the arithmetic belongs in a script.
const MIN_OPERANDS: usize = 4;

/// Whether `c` joins a group to more digits of its own run: a symbol other
/// than the plain space, which the scanner joins only into thousands, so a
/// plain space at a figure's edge sets it apart from the next.
const fn joins_groups(c: char) -> bool {
    is_symbol(c) && c != ' '
}

/// Whether the figure at `span` of `text` is a piece of something longer,
/// which is no value worked with: of a date, a time or a range, or a group
/// that a symbol joins to more digits of the same run, as in a run printed
/// side by side ("15977,10140") or one that ends against a word ("12.5kg"
/// leaves "12"). Digits that end a word before it ("Q1,12") are no part of
/// its run.
fn is_piece(text: &str, span: &Range<usize>) -> bool {
    let joins = |c: char| joins_pieces(c) || joins_groups(c);
    let before = char_before(text, span.start).is_some_and(|c| {
        if !joins(c) {
            return false;
        }
        let run_end = span.start.saturating_sub(c.len_utf8());
        let run_start = digits_start(text, run_end);
        run_start < run_end
            && (joins_pieces(c) || !char_before(text, run_start).is_some_and(is_boundary))
    });
    let after = char_at(text, span.end).is_some_and(|c| {
        joins(c) && char_at(text, span.end.saturating_add(c.len_utf8())).is_some_and(is_digit)
    });
    before || after
}

/// Whether `text` gives at least [`MIN_OPERANDS`] values, each counted where
/// it stands ("25 + 25 + 25 + 25" gives four). Digits a plain space joins
/// count once each, since a list is written so as often as a French
/// thousands ("120 340 560 780"); a number in words counts once.
fn gives_operands(text: &str) -> bool {
    let written = tokens(text)
        .without_currency()
        .without_signs()
        .filter(|t| !is_piece(text, &(t.start..t.end())))
        .map(|t| t.text.split(' ').count())
        .fold(0_usize, usize::saturating_add);
    let in_words = || {
        spelled(text)
            .filter(|n| char_at(text, n.span.start).is_none_or(|c| digit_value(c).is_none()))
            .count()
    };
    written >= MIN_OPERANDS || written.saturating_add(in_words()) >= MIN_OPERANDS
}

/// Figures `reply` states that `corpus` does not carry, when `message` gave
/// four or more values: arithmetic done in prose, as the reply wrote it.
/// Empty when the message gave fewer.
///
/// Any figure past ten is held, and any decimal or negative one; a count of
/// ten or less, a year, and the pieces of a date or a time are not. A figure
/// is grounded by a printed value equal to it, so a sum the reply rounded
/// differently is named too. The corpus is the whole turn, message included.
///
/// # Examples
///
/// ```
/// let message = "Q1 12,500, Q2 14,200, Q3 16,100, Q4 18,500: total and average?";
/// let reply = "Total 61,300, average 15,325.0 over the 4 quarters.";
/// assert_eq!(
///     figures_parser::prose_arithmetic(reply, message, message),
///     ["61,300", "15,325.0"]
/// );
///
/// // Under four values, the reply is left alone.
/// assert!(figures_parser::prose_arithmetic("35.", "10 + 20 + 5?", "").is_empty());
/// ```
#[must_use]
pub fn prose_arithmetic(reply: &str, message: &str, corpus: &str) -> Vec<String> {
    if !gives_operands(message) {
        return Vec::new();
    }
    // Only a value printed as one grounds a result: not a group of a
    // thousands run ("524" of "$1,524") nor the piece of a time ("30" of
    // "14:30"), which the figure check lets ground.
    ungrounded_by(
        reply,
        corpus,
        |r| {
            if r.derived || is_piece(reply, &r.span) {
                None
            } else {
                r.result()
            }
        },
        |r| !r.derived && !is_piece(corpus, &r.span),
    )
}
