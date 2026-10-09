//! The link gate's comparison: which of a reply's links, and which of the
//! items it names, the turn's corpus does not carry.

use super::items::{
    bare_identifiers, identifier_links, markdown_links, placeholder_targets, url_with_fragment,
};
use super::scan::urls;
use super::scheme_len;
use crate::text::{char_at, char_before, is_ascii_word};

/// Whether `segment` names one item rather than a page: eight or more word
/// characters or dashes, a digit among them.
fn is_id_segment(segment: &str) -> bool {
    segment.len() >= 8
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && segment.bytes().any(|b| b.is_ascii_digit())
}

/// Whether a path written in a text ends at byte `at`: no path character
/// continues it (a query's when it carries one), nor a dot before a word.
fn path_ends_at(text: &str, at: usize, query: bool) -> bool {
    match char_at(text, at) {
        Some('.') => !char_at(text, at + 1).is_some_and(is_ascii_word),
        Some(c) => {
            !(is_ascii_word(c)
                || matches!(c, '~' | '%' | '/' | '-')
                || (query && matches!(c, '?' | '&' | '=')))
        }
        None => true,
    }
}

/// Whether the corpus carried this URL's path without a host (an API's
/// `htmlPath`), so the URL is that path on the service's address. The path
/// has to name an item, and no absolute URL in the corpus may put it under
/// another host; with none at all, any host passes. A query or fragment the
/// URL carries must be the one given; one it drops still names the same item.
fn path_grounds_url(url: &str, corpus: &str) -> bool {
    let Some(rest) = scheme_len(url, false).and_then(|n| url.get(n..)) else {
        return false;
    };
    let host_len = rest
        .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace())
        .unwrap_or(rest.len());
    let Some(path) = rest
        .get(host_len..)
        .filter(|p| host_len > 0 && p.starts_with('/'))
    else {
        return false;
    };
    let path = path.split(char::is_whitespace).next().unwrap_or(path);
    let host = rest.get(..host_len).unwrap_or_default();
    let item_path = path.split(['?', '#']).next().unwrap_or(path);
    let segments = item_path.split('/').filter(|s| !s.is_empty());
    if segments.clone().count() < 2 || !segments.clone().any(is_id_segment) {
        return false;
    }
    let query = path.contains(['?', '#']);
    let given_bare = occurrences(corpus, path).any(|at| {
        !char_before(corpus, at)
            .is_some_and(|c| is_ascii_word(c) || matches!(c, '.' | '~' | '%' | '/' | '@' | '-'))
            && path_ends_at(corpus, at + path.len(), query)
    });
    given_bare && hosts_of(corpus, path, query).all(|other| other.eq_ignore_ascii_case(host))
}

/// The start of every occurrence of `needle` in `text`, overlapping ones included.
fn occurrences<'t>(text: &'t str, needle: &'t str) -> impl Iterator<Item = usize> + 't {
    let mut from = 0;
    std::iter::from_fn(move || {
        let at = from + text.get(from..)?.find(needle)?;
        from = at + text.get(at..)?.chars().next()?.len_utf8();
        Some(at)
    })
}

/// The host of every absolute URL in `text` (scheme and path compared in any
/// case) that puts `path` under it.
fn hosts_of<'t>(text: &'t str, path: &'t str, query: bool) -> impl Iterator<Item = &'t str> + 't {
    let mut from = 0;
    std::iter::from_fn(move || {
        loop {
            let rest = text.get(from..)?;
            let start = from
                + rest
                    .as_bytes()
                    .windows(4)
                    .position(|w| w.eq_ignore_ascii_case(b"http"))?;
            from = start + 1;
            let Some(scheme) = text.get(start..).and_then(|s| scheme_len(s, true)) else {
                continue;
            };
            let host_start = start + scheme;
            let after = text.get(host_start..).unwrap_or_default();
            let host_len = after
                .find(|c: char| {
                    matches!(c, '/' | '?' | '#' | '"' | '\'' | '<' | '>') || c.is_whitespace()
                })
                .unwrap_or(after.len());
            let path_start = host_start + host_len;
            let carries = host_len > 0
                && text
                    .get(path_start..path_start + path.len())
                    .is_some_and(|p| p.eq_ignore_ascii_case(path))
                && path_ends_at(text, path_start + path.len(), query);
            if carries {
                from = path_start + path.len();
                return text.get(host_start..path_start);
            }
        }
    })
}

/// `s` without `prefix`, compared in any case.
fn strip_prefix_any_case<'s>(s: &'s str, prefix: &str) -> Option<&'s str> {
    s.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .and_then(|_| s.get(prefix.len()..))
}

/// Whether `url` names a reserved example domain (`example.com`, `.org`,
/// `.net`), which the prompt's own doctrine illustrates links with.
fn is_example_host(url: &str) -> bool {
    let Some(rest) = scheme_len(url, true).and_then(|n| url.get(n..)) else {
        return false;
    };
    let rest = strip_prefix_any_case(rest, "www.").unwrap_or(rest);
    let Some(tld) = strip_prefix_any_case(rest, "example.") else {
        return false;
    };
    ["com", "org", "net"].iter().any(|t| {
        strip_prefix_any_case(tld, t)
            .is_some_and(|after| after.is_empty() || after.starts_with('/'))
    })
}

/// The links in `reply`, once each, that `corpus` does not carry.
///
/// Neither verbatim nor as a path a result gave without its host. A link is
/// read without the punctuation and markup around it, while the corpus stays
/// raw text; a link the corpus printed is always a substring of it.
#[must_use]
pub fn fabricated_urls<'r>(reply: &'r str, corpus: &str) -> Vec<&'r str> {
    let mut out: Vec<&str> = Vec::new();
    for url in urls(reply) {
        if !out.contains(&url.text)
            && !corpus.contains(url.text)
            && !path_grounds_url(url.text, corpus)
        {
            out.push(url.text);
        }
    }
    out
}

/// The parts of `reply` that neither `corpus` (the thread, the request, the
/// run's tool results) nor `prompt` grounds, as the correction round names
/// them back to the model:
///
/// - a link neither carries;
/// - a link on a reserved example domain that only the prompt carried, since
///   the prompt illustrates the format with such links;
/// - a link target that is not a URL (`(url)`), a template copied from an example;
/// - an item's label on a link that is not the item's own (`[#429](…/repo)`);
/// - an item named without a link although a result carried its URL.
#[must_use]
pub fn ungrounded_urls(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    for url in urls(reply) {
        if !seen.contains(&url.text) {
            seen.push(url.text);
        }
    }
    let mut out: Vec<String> = seen
        .iter()
        .filter(|url| {
            !corpus.contains(**url) && !path_grounds_url(url, corpus) && !prompt.contains(**url)
        })
        .map(|url| (*url).to_owned())
        .collect();
    out.extend(
        seen.iter()
            .filter(|url| is_example_host(url) && prompt.contains(**url) && !corpus.contains(**url))
            .map(|url| (*url).to_owned()),
    );
    out.extend(
        placeholder_targets(reply)
            .into_iter()
            .map(|target| format!("({target})")),
    );
    let links = identifier_links(reply);
    out.extend(
        links
            .iter()
            .filter(|link| !link.url.contains(link.label.trim_start_matches('#')))
            .map(|link| link.whole.to_owned()),
    );
    out.extend(unlinked_identifiers(reply, &links, &[corpus, prompt]));
    out
}

/// The items `reply` names without linking them anywhere although a corpus
/// carried their URL (`…/issues/N`, `…/pull/N`, `…/TEAM-N`), each with that
/// URL: a script that fetched the item may never have printed it.
fn unlinked_identifiers(
    reply: &str,
    links: &[super::items::IdentifierLink<'_>],
    corpora: &[&str],
) -> Vec<String> {
    let markdown = markdown_links(reply);
    let mut out = Vec::new();
    for id in bare_identifiers(reply, &markdown) {
        if links.iter().any(|link| link.label == id) {
            continue;
        }
        let fragments: Vec<String> = id.strip_prefix('#').map_or_else(
            || vec![format!("/{id}")],
            |n| vec![format!("/issues/{n}"), format!("/pull/{n}")],
        );
        let linked_by_url = fragments.iter().any(|f| {
            markdown
                .iter()
                .filter_map(|link| reply.get(link.clone()))
                .any(|link| url_with_fragment(link, f).is_some())
        });
        if linked_by_url {
            continue;
        }
        let carried = fragments
            .iter()
            .find_map(|f| corpora.iter().find_map(|c| url_with_fragment(c, f)));
        if let Some(url) = carried {
            out.push(format!("{id} (named without its link, {url})"));
        }
    }
    out
}
