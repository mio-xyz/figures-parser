//! Whether a reply rests on retained memory: which lines of the dossiers a
//! turn preloaded the reply visibly draws on.

use rustc_hash::FxHashSet;
use unicode_general_category::{GeneralCategory, get_general_category};

/// The fewest distinctive words a reply shares with a line to draw on it.
const MIN_SHARED: usize = 2;

/// The shortest token kept, in UTF-16 units: a bare `7` or `10` matches too
/// much to be distinctive, while `10:00` stands on its own.
const MIN_TOKEN_UNITS: usize = 3;

/// Words too common to tell one line from another.
const STOPWORDS: [&str; 109] = [
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

/// How much of a reply rests on the dossiers a turn preloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryGrounding {
    /// The dossier lines read: every non-empty line that is no heading.
    pub preloaded_lines: usize,
    /// The lines the reply draws on.
    pub grounded_lines: usize,
    /// Whether it draws on any.
    pub grounded: bool,
}

/// Whether `c` is part of a token: a letter, a number or `:`.
fn is_token_char(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == ':';
    }
    matches!(
        get_general_category(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
            | GeneralCategory::DecimalNumber
            | GeneralCategory::LetterNumber
            | GeneralCategory::OtherNumber
    )
}

/// The byte past the freshness marker (`[derived 2026-09-07]`, `[as of
/// 2026-09]`) that opens `text`, when one does.
fn marker_len(text: &str) -> Option<usize> {
    let rest = text.strip_prefix('[')?;
    let opens = |word: &str| {
        rest.get(..word.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(word))
    };
    if !(opens("derived") || opens("as of")) {
        return None;
    }
    rest.find(']').map(|close| close.saturating_add(2))
}

/// The distinctive words of `text`, lowercased, in order.
///
/// Its freshness markers and markdown are dropped, it is cut at every
/// character that is no letter, number or `:`, a `:` at either end of a token
/// is dropped (the one in `10:00` stays), and every token shorter than three
/// UTF-16 units or too common is left out.
///
/// # Examples
///
/// ```
/// let tokens = figures_parser::grounding_tokens("**Owner:** Ana, standup at 10:00 [as of 2026-09]");
/// assert_eq!(tokens, ["owner", "ana", "standup", "10:00"]);
/// ```
#[must_use]
pub fn grounding_tokens(text: &str) -> Vec<String> {
    let mut plain = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(c) = text.get(at..).and_then(|rest| rest.chars().next()) {
        if c == '['
            && let Some(len) = text.get(at..).and_then(marker_len)
        {
            plain.push(' ');
            at = at.saturating_add(len);
            continue;
        }
        plain.push(if matches!(c, '*' | '_' | '`' | '~' | '>' | '#') {
            ' '
        } else {
            c
        });
        at = at.saturating_add(c.len_utf8());
    }
    plain
        .to_lowercase()
        .split(|c: char| !is_token_char(c))
        .map(|token| token.trim_matches(':'))
        .filter(|token| {
            token.encode_utf16().count() >= MIN_TOKEN_UNITS && !STOPWORDS.contains(token)
        })
        .map(str::to_owned)
        .collect()
}

/// Whether `c` is whitespace as JavaScript's `trim` reads it.
const fn is_js_space(c: char) -> bool {
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

/// Every three tokens in a row of `tokens`, joined by a space.
fn trigrams(tokens: &[String]) -> FxHashSet<String> {
    tokens.windows(3).map(|three| three.join(" ")).collect()
}

/// Which lines of `dossiers` (each a preloaded dossier's text) `response`
/// visibly draws on.
///
/// A line is every non-empty line of a dossier that is no `#` heading. The
/// response draws on one when it shares at least two of the line's distinct
/// [`grounding_tokens`] and at least half of them, or repeats three of the
/// line's tokens in a row. A line of fewer than two distinct tokens is read
/// but never drawn on by overlap.
///
/// # Examples
///
/// ```
/// let dossier = "# Ana\nPrefers async standups at 10:00\nOwns the Q3 renewal";
/// let read = figures_parser::memory_grounding("Standup moves to 10:00, async as Ana prefers.", &[dossier]);
/// assert_eq!((read.preloaded_lines, read.grounded_lines, read.grounded), (2, 1, true));
/// ```
#[must_use]
pub fn memory_grounding(response: &str, dossiers: &[&str]) -> MemoryGrounding {
    let response_tokens = grounding_tokens(response);
    let response_set: FxHashSet<&str> = response_tokens.iter().map(String::as_str).collect();
    let response_trigrams = trigrams(&response_tokens);
    let mut read = MemoryGrounding::default();
    let mut joined = String::new();
    for line in dossiers
        .iter()
        .flat_map(|dossier| dossier.split('\n'))
        .map(|line| line.trim_matches(is_js_space))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        read.preloaded_lines = read.preloaded_lines.saturating_add(1);
        let tokens = grounding_tokens(line);
        let distinct: FxHashSet<&str> = tokens.iter().map(String::as_str).collect();
        if distinct.len() < MIN_SHARED {
            continue;
        }
        let shared = distinct
            .iter()
            .filter(|token| response_set.contains(*token))
            .count();
        // At least half the line's distinct tokens, compared without division.
        let half = distinct.len().div_ceil(2);
        let by_overlap = shared >= MIN_SHARED && shared >= half;
        let by_trigram = tokens.windows(3).any(|three| {
            joined.clear();
            for (i, token) in three.iter().enumerate() {
                if i > 0 {
                    joined.push(' ');
                }
                joined.push_str(token);
            }
            response_trigrams.contains(joined.as_str())
        });
        if by_overlap || by_trigram {
            read.grounded_lines = read.grounded_lines.saturating_add(1);
        }
    }
    read.grounded = read.grounded_lines > 0;
    read
}
