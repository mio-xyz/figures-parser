//! The form of a reply, apart from what it states: a speaker prefix echoed
//! from the history's format, a claim to have saved something to memory,
//! and the marker a run writes to deliver nothing.

use std::sync::LazyLock;

use regex::Regex;

/// Whether `c` is whitespace as a JavaScript `\s` reads it: Unicode's
/// `White_Space` less U+0085, and U+FEFF.
const fn is_js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{FEFF}'
}

/// Whether `c` ends a line as a JavaScript `^` and `$` read one.
const fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The most UTF-16 units a speaker's name holds.
const MAX_SPEAKER_UNITS: usize = 80;

/// The `[Name]: ` speaker prefix that opens `reply`, as written, when it
/// opens with one: the model echoing the history's speaker format at the top
/// of its reply.
///
/// The prefix is the reply's first line only, after leading whitespace: a
/// bracketed name of 1 to 80 UTF-16 units holding no `]` and no line break,
/// then `]:` and a space. A `[ref]: url` definition or a `[Note]:` bullet
/// further down is ordinary markdown.
///
/// # Examples
///
/// ```
/// use figures_parser::speaker_prefix;
///
/// assert_eq!(speaker_prefix("[Ana]: hello"), Some("[Ana]: "));
/// assert_eq!(speaker_prefix("See the doc.\n[ref]: https://acme.io"), None);
/// assert_eq!(speaker_prefix("[link](https://acme.io)"), None);
/// ```
#[must_use]
pub fn speaker_prefix(reply: &str) -> Option<&str> {
    let start = reply
        .len()
        .saturating_sub(reply.trim_start_matches(is_js_space).len());
    let rest = reply.get(start..)?.strip_prefix('[')?;
    let close = rest
        .find([']', '\n'])
        .filter(|&at| rest.get(at..).is_some_and(|tail| tail.starts_with(']')))?;
    let name = rest.get(..close)?;
    let units: usize = name.chars().map(char::len_utf16).sum();
    if !(1..=MAX_SPEAKER_UNITS).contains(&units) {
        return None;
    }
    let after = rest.get(close..)?.strip_prefix("]:")?;
    let space = after.chars().next().filter(|&c| is_js_space(c))?;
    let end = reply
        .len()
        .saturating_sub(after.len())
        .saturating_add(space.len_utf8());
    reply.get(start..end)
}

/// A claim to have remembered something or saved it to a memory surface:
/// every alternation names remembering or memory, so a reply that reports a
/// write elsewhere ("saved it to the wiki") is no claim, nor is a
/// conversational "keep in mind". Case is read in ASCII alone, and an
/// apostrophe either straight or curly.
static MEMORY_CLAIM: LazyLock<Option<Regex>> = LazyLock::new(|| {
    crate::text::compile(concat!(
        r"(?i-u)\bI(?:(?u:['’])ll| will) remember",
        r"|\bI(?:(?u:['’])ve| have) (?:noted|remembered)",
        r"|\bcommitted (?:this|that|it) to memory",
        r"|\b(?:noted|saved|added|stored|updated) (?:this|that|it) (?:to|in) ",
        r"(?:my |your |the )?(?:memory|preferences?|notes?|dossier|profile)",
    ))
});

/// The claim `reply` makes to have saved something to memory, as written.
///
/// That is "I'll remember", "I've noted", "committed it to memory", "saved
/// that to your preferences" and their kin. A turn that makes one must have
/// called a memory write; whether it did is the caller's to know.
///
/// # Examples
///
/// ```
/// use figures_parser::memory_claim;
///
/// assert_eq!(memory_claim("Done, I'll remember that."), Some("I'll remember"));
/// assert_eq!(memory_claim("I've saved it to the wiki."), None);
/// assert_eq!(memory_claim("Keep in mind the deadline is Friday."), None);
/// ```
#[must_use]
pub fn memory_claim(reply: &str) -> Option<&str> {
    MEMORY_CLAIM
        .as_ref()?
        .find(reply)
        .map(|found| found.as_str())
}

/// A `silent: <reason>` marker, the token a run writes to deliver nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentSignal<'a> {
    /// The reason, a single run without whitespace.
    pub reason: &'a str,
    /// Whether the marker sat inside a longer body rather than being the
    /// whole of it: the run meant to abstain but broke the rule that the
    /// marker is the entire reply.
    pub leaked: bool,
}

/// The prompt framing a model may echo at the very start of its reply.
const CURRENT_MESSAGE: &str = "[CURRENT MESSAGE]";

/// `text` without the `[CURRENT MESSAGE]` prefix a model may echo before
/// its reply, leading whitespace and one space after it included.
fn without_framing(text: &str) -> &str {
    text.trim_start_matches(is_js_space)
        .strip_prefix(CURRENT_MESSAGE)
        .map_or(text, |rest| rest.strip_prefix(' ').unwrap_or(rest))
}

/// The reason of the marker that opens `text`, when one does and ends where
/// `$` holds: at the end of `text`, or with `multiline` before a line end.
fn marker_at(text: &str, multiline: bool) -> Option<&str> {
    let rest = text
        .strip_prefix("silent:")?
        .trim_start_matches(is_js_space);
    let reason_len = rest.find(is_js_space).unwrap_or(rest.len());
    let reason = rest.get(..reason_len).filter(|r| !r.is_empty())?;
    let after = rest.get(reason_len..)?;
    let space_len = after.find(|c: char| !is_js_space(c)).unwrap_or(after.len());
    let ends =
        space_len == after.len() || (multiline && after.get(..space_len)?.contains(is_line_end));
    ends.then_some(reason)
}

/// The silent marker `raw` (a run's text before presentation) carries: the
/// whole reply when it is `silent: <reason>` alone, else the first line that
/// is one, read as leaked. None when it carries none.
///
/// A `[CURRENT MESSAGE]` framing the model echoed first is read past, and
/// the marker is case-sensitive, as the run writes it.
///
/// # Examples
///
/// ```
/// use figures_parser::{SilentSignal, silent_signal};
///
/// assert_eq!(
///     silent_signal("silent: nothing_new"),
///     Some(SilentSignal { reason: "nothing_new", leaked: false })
/// );
/// assert_eq!(
///     silent_signal("Summary below.\nsilent: no_signal\nMore."),
///     Some(SilentSignal { reason: "no_signal", leaked: true })
/// );
/// assert_eq!(silent_signal("Hello"), None);
/// ```
#[must_use]
pub fn silent_signal(raw: &str) -> Option<SilentSignal<'_>> {
    let body = without_framing(raw);
    if let Some(reason) = marker_at(body.trim_matches(is_js_space), false) {
        return Some(SilentSignal {
            reason,
            leaked: false,
        });
    }
    let mut line = Some(0);
    while let Some(start) = line {
        let rest = body.get(start..)?;
        if let Some(reason) = marker_at(rest, true) {
            return Some(SilentSignal {
                reason,
                leaked: true,
            });
        }
        line = rest.find(is_line_end).and_then(|at| {
            let end = rest.get(at..)?.chars().next()?.len_utf8();
            Some(start.saturating_add(at).saturating_add(end))
        });
    }
    None
}
