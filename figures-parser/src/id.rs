//! The id kind: identifiers a reply writes (a UUID, a hash, a tracker key, a
//! prefixed id) that the turn it answers never gave.

use std::ops::Range;

use crate::email::emails;
use crate::text::{
    carried, inside_word, is_ascii_word_byte, is_iso_date, key_len, report_once, run_end, run_start,
};
use crate::url::urls;

/// The shape of an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `8-4-4-4-12` hexadecimal digits.
    Uuid,
    /// 7 to 64 hexadecimal digits, letters and digits both: a hash or digest.
    Hex,
    /// A tracker key: capitals, a dash, digits (`CORE-1710`).
    Key,
    /// A lowercase prefix, an underscore, then letters and digits (`cus_9aZ3kLm2`).
    Prefixed,
    /// Any other run of ten or more letters, digits, `_` and `-` mixing letters
    /// and digits: possibly an identifier, read for measurement only.
    Token,
}

impl Shape {
    /// The shape's name, as a caller selects it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Hex => "hex",
            Self::Key => "key",
            Self::Prefixed => "prefixed",
            Self::Token => "token",
        }
    }
}

/// One identifier, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier<'a> {
    /// As written.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
    /// Its shape.
    pub shape: Shape,
}

/// Whether `b` belongs to an identifier run.
const fn is_run_byte(b: u8) -> bool {
    is_ascii_word_byte(b) || b == b'-'
}

/// Whether `run` mixes ASCII letters and digits.
fn mixes(run: &str) -> bool {
    run.bytes().any(|b| b.is_ascii_digit()) && run.bytes().any(|b| b.is_ascii_alphabetic())
}

/// Whether `run` is a UUID.
fn is_uuid(run: &str) -> bool {
    let mut lengths = [8, 4, 4, 4, 12].into_iter();
    run.len() == 36
        && run.split('-').all(|group| {
            lengths.next() == Some(group.len()) && group.bytes().all(|b| b.is_ascii_hexdigit())
        })
        && lengths.next().is_none()
}

/// Whether `run` is a hash: 7 to 64 hexadecimal digits mixing letters and digits.
fn is_hex(run: &str) -> bool {
    (7..=64).contains(&run.len()) && run.bytes().all(|b| b.is_ascii_hexdigit()) && mixes(run)
}

/// The prefix of a tracker key (`CORE` of `CORE-1710`), when `run` is one.
fn key_prefix(run: &str) -> Option<&str> {
    (key_len(run) == Some(run.len()))
        .then(|| run.split_once('-').map(|(prefix, _)| prefix))
        .flatten()
}

/// The prefix of a prefixed id (`cus` of `cus_9aZ3kLm2`), when `run` is one:
/// lowercase words joined by `_`, then six or more letters and digits that
/// hold a digit.
fn id_prefix(run: &str) -> Option<&str> {
    let (prefix, tail) = run.rsplit_once('_')?;
    let prefix_ok = prefix.starts_with(|c: char| c.is_ascii_lowercase())
        && prefix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let tail_ok = tail.len() >= 6
        && tail.bytes().all(|b| b.is_ascii_alphanumeric())
        && tail.bytes().any(|b| b.is_ascii_digit());
    (prefix_ok && tail_ok).then_some(prefix)
}

/// The shape of a run, None when it is no identifier.
fn shape_of(run: &str) -> Option<Shape> {
    if is_uuid(run) {
        Some(Shape::Uuid)
    } else if is_hex(run) {
        Some(Shape::Hex)
    } else if key_prefix(run).is_some() {
        Some(Shape::Key)
    } else if id_prefix(run).is_some() {
        Some(Shape::Prefixed)
    } else if run.len() >= 10 && mixes(run) && !run.as_bytes().get(..10).is_some_and(is_iso_date) {
        Some(Shape::Token)
    } else {
        None
    }
}

/// The identifier the run of bytes `start..end` of `text` writes, if any:
/// its outer `_` and `-` dropped, with no letter of another script against
/// either end.
fn identifier_at(text: &str, start: usize, end: usize) -> Option<Identifier<'_>> {
    let raw = text.get(start..end)?;
    let run = raw.trim_start_matches(['-', '_']);
    let start = start.saturating_add(raw.len().saturating_sub(run.len()));
    let run = run.trim_end_matches(['-', '_']);
    let shape = shape_of(run).filter(|_| !inside_word(text, start, end))?;
    Some(Identifier {
        text: run,
        span: start..start.saturating_add(run.len()),
        shape,
    })
}

/// Every identifier-shaped run in `text`, in order: ASCII letters, digits,
/// `_` and `-`.
fn runs(text: &str) -> impl Iterator<Item = Identifier<'_>> {
    let bytes = text.as_bytes();
    let mut at: usize = 0;
    std::iter::from_fn(move || {
        loop {
            let start = at.saturating_add(bytes.get(at..)?.iter().position(|&b| is_run_byte(b))?);
            at = run_end(bytes, start, is_run_byte);
            if let Some(id) = identifier_at(text, start, at) {
                return Some(id);
            }
        }
    })
}

/// Every identifier in `text`, in order, leaving out those inside a link or
/// an email address, which those kinds read.
///
/// # Examples
///
/// ```
/// use figures_parser::id::Shape;
///
/// let found: Vec<(&str, Shape)> = figures_parser::identifiers("Fixed in 9f2e4c1a, see CORE-1710.")
///     .iter()
///     .map(|id| (id.text, id.shape))
///     .collect();
/// assert_eq!(found, [("9f2e4c1a", Shape::Hex), ("CORE-1710", Shape::Key)]);
/// ```
#[must_use]
pub fn identifiers(text: &str) -> Vec<Identifier<'_>> {
    let taken: Vec<Range<usize>> = urls(text)
        .into_iter()
        .map(|u| u.span)
        .chain(emails(text).into_iter().map(|e| e.span))
        .collect();
    runs(text)
        .filter(|id| {
            !taken
                .iter()
                .any(|t| t.start <= id.span.start && id.span.end <= t.end)
        })
        .collect()
}

impl<'a> Identifier<'a> {
    /// The prefix a tracker key or a prefixed id is numbered under.
    fn prefix(&self) -> Option<&'a str> {
        match self.shape {
            Shape::Key => key_prefix(self.text),
            Shape::Prefixed => id_prefix(self.text),
            Shape::Uuid | Shape::Hex | Shape::Token => None,
        }
    }
}

/// Whether `corpus` numbers an identifier of `shape` under `prefix`: each
/// place the prefix and its separator occur is read as the run that holds
/// it, so a corpus without the prefix is never tokenized.
fn numbers(corpus: &str, shape: Shape, prefix: &str) -> bool {
    let separator = if shape == Shape::Key { b'-' } else { b'_' };
    let bytes = corpus.as_bytes();
    memchr::memmem::find_iter(bytes, prefix.as_bytes()).any(|hit| {
        let after = hit.saturating_add(prefix.len());
        bytes.get(after) == Some(&separator) && {
            let start = run_start(bytes, hit, is_run_byte);
            let end = run_end(bytes, after, is_run_byte);
            identifier_at(corpus, start, end)
                .is_some_and(|id| id.shape == shape && id.prefix() == Some(prefix))
        }
    })
}

/// The identifiers in `reply`, once each, that neither `corpus` (the thread,
/// the request, the run's tool results) nor `prompt` carries, case aside.
///
/// A UUID or a hash is always held to it; a short hash is carried by any
/// longer one that holds it. A tracker key or a prefixed id is held to it
/// only when the corpus carries another of the same prefix: that catches an
/// invented next number (`CORE-1711` beside `CORE-1710`) without holding
/// names that share the shape (`GPT-4`, `UTF-8`) to anything.
///
/// # Examples
///
/// ```
/// let corpus = "CORE-1710 Renewal follow-up";
/// let reply = "CORE-1710 and CORE-1711 are open; we use UTF-8 and GPT-4.";
/// assert_eq!(figures_parser::ungrounded_identifiers(reply, corpus, ""), ["CORE-1711"]);
/// ```
#[must_use]
pub fn ungrounded_identifiers(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in identifiers(reply) {
        let held = id.prefix().map_or_else(
            || matches!(id.shape, Shape::Uuid | Shape::Hex),
            |prefix| numbers(corpus, id.shape, prefix),
        );
        if held {
            report_once(&mut out, id.text, || carried(id.text, corpus, prompt));
        }
    }
    out
}

/// The long mixed tokens in `reply`, once each, that neither `corpus` nor
/// `prompt` carries.
///
/// They are possibly invented identifiers, for measurement only, since a
/// product or model name ("claude-opus-5-5") shares the shape.
///
/// # Examples
///
/// ```
/// let reply = "Model claude-opus-5-5 on node ip-10-0-3-17.";
/// assert_eq!(
///     figures_parser::unseen_tokens(reply, "node ip-10-0-3-17", ""),
///     ["claude-opus-5-5"]
/// );
/// ```
#[must_use]
pub fn unseen_tokens(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in identifiers(reply)
        .into_iter()
        .filter(|id| id.shape == Shape::Token)
    {
        report_once(&mut out, id.text, || carried(id.text, corpus, prompt));
    }
    out
}
