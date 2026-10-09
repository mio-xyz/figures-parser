//! The links a reply writes around tracker items (`#429`, `CORE-1710`) and the
//! link targets that are not links at all.

use std::ops::Range;

use super::scheme_len;
use crate::text::{char_at, char_before, is_ascii_word};

/// The byte length of the item identifier (`#429`, `CORE-1710`) that opens
/// `text`, or None when none does.
fn identifier_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let digits_from = |at: usize| {
        let n = bytes.get(at..).map_or(0, |rest| {
            rest.iter().take_while(|b| b.is_ascii_digit()).count()
        });
        (n > 0).then_some(at + n)
    };
    match bytes.first()? {
        b'#' => digits_from(1),
        b'A'..=b'Z' => {
            let team = bytes.get(1..).map_or(0, |rest| {
                rest.iter()
                    .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                    .count()
            });
            if team == 0 || bytes.get(1 + team) != Some(&b'-') {
                return None;
            }
            digits_from(2 + team)
        }
        _ => None,
    }
}

/// A link labelled with an item identifier: `[#429](url)`, `[**CORE-1710**](url)`.
pub struct IdentifierLink<'a> {
    /// The whole link as written.
    pub whole: &'a str,
    /// The identifier.
    pub label: &'a str,
    /// The link's target.
    pub url: &'a str,
}

/// Every link in `text` labelled with an item identifier, in order.
pub fn identifier_links(text: &str) -> Vec<IdentifierLink<'_>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find('[')) {
        let open = from + found;
        from = open + 1;
        let Some(link) = identifier_link_at(text, open) else {
            continue;
        };
        from = open + link.whole.len();
        out.push(link);
    }
    out
}

/// The identifier link that opens at byte `open`, a `[`.
fn identifier_link_at(text: &str, open: usize) -> Option<IdentifierLink<'_>> {
    let skip_stars = |at: usize| {
        at + text
            .get(at..)
            .map_or(0, |rest| rest.bytes().take_while(|&b| b == b'*').count())
    };
    let label_start = skip_stars(open + 1);
    let label_end = label_start + identifier_len(text.get(label_start..)?)?;
    let close = skip_stars(label_end);
    let rest = text.get(close..)?;
    let target = rest.strip_prefix("](")?;
    let scheme = scheme_len(target, false)?;
    let url_len = scheme
        + target
            .get(scheme..)?
            .find(|c: char| c == ')' || c.is_whitespace())
            .filter(|&n| n > 0)?;
    if !target.get(url_len..)?.starts_with(')') {
        return None;
    }
    let url_start = close + 2;
    Some(IdentifierLink {
        whole: text.get(open..url_start + url_len + 1)?,
        label: text.get(label_start..label_end)?,
        url: text.get(url_start..url_start + url_len)?,
    })
}

/// The byte ranges of every markdown link (`[label](target)`) and every
/// angle-bracketed link (`<https://…>`) in `text`, in order.
pub fn markdown_links(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find(['[', '<'])) {
        let open = from + found;
        from = open + 1;
        let rest = text.get(open + 1..).unwrap_or_default();
        let len = if text.get(open..).is_some_and(|s| s.starts_with('[')) {
            rest.find(']').and_then(|close| {
                let target = rest.get(close + 1..)?.strip_prefix('(')?;
                Some(close + 2 + target.find(')')? + 1)
            })
        } else {
            scheme_len(rest, false).and_then(|_| rest.find('>').map(|close| close + 1))
        };
        if let Some(len) = len {
            out.push(open..open + 1 + len);
            from = open + 1 + len;
        }
    }
    out
}

/// Every item identifier named in `text` outside a link, once each, in order.
pub fn bare_identifiers<'a>(text: &'a str, links: &[Range<usize>]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let inside_link = links.iter().find(|link| link.contains(&at));
        if let Some(link) = inside_link {
            at = link.end;
            continue;
        }
        let standalone = !char_before(text, at)
            .is_some_and(|c| is_ascii_word(c) || matches!(c, '/' | '#' | '-'));
        let found = standalone
            .then(|| identifier_len(text.get(at..)?))
            .flatten()
            .filter(|&len| !char_at(text, at + len).is_some_and(is_ascii_word));
        match found.and_then(|len| text.get(at..at + len)) {
            Some(id) => {
                if !out.contains(&id) {
                    out.push(id);
                }
                at += id.len();
            }
            None => at += char_at(text, at).map_or(1, char::len_utf8),
        }
    }
    out
}

/// The target of every markdown link in `text` that is not a URL, a mail
/// address or an anchor: a template copied from an example ("[PR 512](url)").
pub fn placeholder_targets(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find("](")) {
        let start = from + found + 2;
        from = from + found + 1;
        let rest = text.get(start..).unwrap_or_default();
        let is_link = scheme_len(rest, false).is_some()
            || rest.starts_with("mailto:")
            || rest.starts_with('#');
        if is_link {
            continue;
        }
        let len = rest
            .find(|c: char| c == ')' || c.is_whitespace())
            .unwrap_or(rest.len());
        if len > 0 && rest.get(len..).is_some_and(|after| after.starts_with(')')) {
            out.extend(rest.get(..len));
            from = start + len + 1;
        }
    }
    out
}

/// The first URL in `text` (`http://` or `https://` up to whitespace, a quote,
/// `<`, `>`, `)` or `]`) that holds `fragment` not followed by a digit, cut
/// after the last such `fragment` in it.
pub fn url_with_fragment<'a>(text: &'a str, fragment: &str) -> Option<&'a str> {
    if !text.contains(fragment) {
        return None;
    }
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find("http")) {
        let start = from + found;
        from = start + 1;
        let Some(scheme) = text.get(start..).and_then(|rest| scheme_len(rest, false)) else {
            continue;
        };
        let body_start = start + scheme;
        let body = text.get(body_start..).unwrap_or_default();
        let body_len = body
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']'))
            .unwrap_or(body.len());
        let mut end = body_len;
        while let Some(at) = body.get(..end).and_then(|run| run.rfind(fragment)) {
            let after = body_start + at + fragment.len();
            if !char_at(text, after).is_some_and(|c| c.is_ascii_digit()) {
                return text.get(start..after);
            }
            end = at + fragment.len() - 1;
        }
    }
    None
}
