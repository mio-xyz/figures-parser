//! Whether a reply draws on the memory it was given: which of the preloaded
//! notes' lines it visibly repeats.

use rustc_hash::FxHashSet;
use unicode_general_category::{GeneralCategory, get_general_category};

use crate::text::is_letter;

/// How many of a line's distinct words a reply must share.
const MIN_SHARED: usize = 2;

/// The share of a line's distinct words a reply must hold, as a fraction
/// `MIN_RATIO.0 / MIN_RATIO.1`.
const MIN_RATIO: (usize, usize) = (1, 2);

/// Shorter tokens are dropped with the short words: a bare `7` matches too
/// much to be distinctive, while `10:00` clears the bar on its own. Counted
/// in UTF-16 units, as the host's lines are.
const MIN_TOKEN_UNITS: usize = 3;

/// Words that tell no line from another.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "of", "to", "in", "on", "at", "for", "with", "by", "as",
    "is", "are", "was", "were", "be", "been", "being", "it", "its", "this", "that", "these",
    "those", "from", "not", "no", "yes", "we", "our", "ours", "you", "your", "yours", "i", "my",
    "me", "mine", "they", "them", "their", "he", "she", "his", "her", "him", "will", "would",
    "can", "could", "should", "may", "might", "do", "does", "did", "have", "has", "had", "than",
    "then", "there", "here", "when", "where", "which", "who", "whom", "what", "how", "every",
    "each", "all", "any", "some", "more", "most", "very", "just", "also", "only", "into", "over",
    "under", "about", "after", "before", "between", "during", "without", "within", "per", "via",
    "etc", "still", "yet", "now", "new", "old", "use", "used", "uses", "using", "one", "two",
];

/// How much of a reply rests on the preloaded memory; [`memory_grounding`]
/// reads it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryGrounding {
    /// The lines of content the notes held.
    pub preloaded_lines: usize,
    /// The lines the reply draws on.
    pub grounded_lines: usize,
    /// Whether the reply draws on any.
    pub grounded: bool,
}

/// The length of the freshness marker (`[derived …]`, `[as of …]`) that
/// opens at the start of `text`, its brackets included.
fn marker_len(text: &str) -> Option<usize> {
    let rest = text.strip_prefix('[')?;
    let head = ["derived", "as of"].into_iter().find(|word| {
        rest.get(..word.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(word))
    })?;
    let close = rest.get(head.len()..)?.find(']')?;
    close.checked_add(head.len())?.checked_add(2)
}

/// `text` with its freshness markers and markdown marks made spaces: they
/// mark how a line was kept, not what it says.
fn without_marks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(rest) = text.get(at..) {
        let Some(c) = rest.chars().next() else {
            break;
        };
        if let Some(len) = marker_len(rest) {
            out.push(' ');
            at = at.saturating_add(len);
            continue;
        }
        out.push(if matches!(c, '*' | '_' | '`' | '~' | '>' | '#') {
            ' '
        } else {
            c
        });
        at = at.saturating_add(c.len_utf8());
    }
    out
}

/// Whether `c` is a letter, a number of any kind, or the colon a time holds.
fn is_token_char(c: char) -> bool {
    c == ':'
        || is_letter(c)
        || matches!(
            get_general_category(c),
            GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
        )
}

/// The distinctive words of `text`, lowercased, in order: no freshness
/// marker, markdown mark, stopword or short token, and no colon at either end
/// of one (`Owner:`), while the one inside `10:00` stays.
///
/// # Examples
///
/// ```
/// let tokens = figures_parser::grounding_tokens("**Standup** is at 10:00 [as of 2026-09]. Owner: Ana");
/// assert_eq!(tokens, ["standup", "10:00", "owner", "ana"]);
/// ```
#[must_use]
pub fn grounding_tokens(text: &str) -> Vec<String> {
    tokens(&lowered(text)).map(str::to_owned).collect()
}

/// `text` without its marks, lowercased: what [`tokens`] splits.
fn lowered(text: &str) -> String {
    without_marks(text).to_lowercase()
}

/// The distinctive words of a [`lowered`] text.
fn tokens(lower: &str) -> impl Iterator<Item = &str> {
    lower
        .split(|c: char| !is_token_char(c))
        .map(|token| token.trim_matches(':'))
        .filter(|token| {
            token.encode_utf16().count() >= MIN_TOKEN_UNITS && !STOPWORDS.contains(token)
        })
}

/// Whether `c` is white space as a host's `trim` reads it.
const fn is_trim_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// The lines of a note that say something: not empty, not a heading.
fn content_lines(note: &str) -> impl Iterator<Item = &str> {
    note.split('\n')
        .map(|line| line.trim_matches(is_trim_space))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// Every run of three tokens in `words`.
fn trigrams<'t>(words: &[&'t str]) -> impl Iterator<Item = (&'t str, &'t str, &'t str)> {
    words.windows(3).filter_map(|w| match *w {
        [a, b, c] => Some((a, b, c)),
        _ => None,
    })
}

/// Which lines of the preloaded `notes` `response` visibly draws on.
///
/// A line counts when the response shares at least two of its distinctive words
/// and at least half of them, or repeats three of its words in a row; a
/// line of fewer than two distinct words never counts.
///
/// # Examples
///
/// ```
/// let notes = ["# Team\n- Standup moved to Tuesday 10:00 in the Zoom room\n- Ana owns billing"];
/// let grounding = figures_parser::memory_grounding("Standup is Tuesday at 10:00.", &notes);
/// assert_eq!((grounding.preloaded_lines, grounding.grounded_lines), (2, 1));
/// assert!(grounding.grounded);
/// ```
#[must_use]
pub fn memory_grounding<N: AsRef<str>>(response: &str, notes: &[N]) -> MemoryGrounding {
    let lower = lowered(response);
    let response_tokens: Vec<&str> = tokens(&lower).collect();
    let words: FxHashSet<&str> = response_tokens.iter().copied().collect();
    let runs: FxHashSet<(&str, &str, &str)> = trigrams(&response_tokens).collect();
    let mut preloaded_lines = 0_usize;
    let mut grounded_lines = 0_usize;
    for line in notes.iter().flat_map(|note| content_lines(note.as_ref())) {
        preloaded_lines = preloaded_lines.saturating_add(1);
        let line_lower = lowered(line);
        let line_tokens: Vec<&str> = tokens(&line_lower).collect();
        let distinct: FxHashSet<&str> = line_tokens.iter().copied().collect();
        if distinct.len() < MIN_SHARED {
            continue;
        }
        let shared = distinct.iter().filter(|t| words.contains(*t)).count();
        let by_overlap = shared >= MIN_SHARED
            && shared.saturating_mul(MIN_RATIO.1) >= distinct.len().saturating_mul(MIN_RATIO.0);
        if by_overlap || trigrams(&line_tokens).any(|run| runs.contains(&run)) {
            grounded_lines = grounded_lines.saturating_add(1);
        }
    }
    MemoryGrounding {
        preloaded_lines,
        grounded_lines,
        grounded: grounded_lines > 0,
    }
}
