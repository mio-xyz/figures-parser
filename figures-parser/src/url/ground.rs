//! The link gate's comparison: which of a reply's links, and which of the
//! items it names, the turn's corpus does not carry.

use super::items::{
    IdentifierLink, bare_identifiers, code_spans, identifier_links, is_api_link, markdown_links,
    placeholder_targets, url_with_fragment, urls_with_fragment,
};
use super::scan::urls;
use super::scheme_len;
use super::shapes::{Parts, built_from_corpus, undotted_slack_link};
use crate::text::{char_at, char_before, is_ascii_word, is_example_domain};

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

/// Whether the corpus carried this URL's path without a host (as an API
/// that gives a page's path does), so the URL is that path on the service's
/// address. The path
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
pub(super) fn occurrences<'t>(text: &'t str, needle: &'t str) -> impl Iterator<Item = usize> + 't {
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

/// Whether `url` names a reserved example domain (`example.com`, `.org`,
/// `.net`), which the prompt's own doctrine illustrates links with.
fn is_example_host(url: &str) -> bool {
    scheme_len(url, true)
        .and_then(|n| url.get(n..))
        .and_then(|rest| rest.split('/').next())
        .is_some_and(is_example_domain)
}

/// Whether `corpus` printed `url` without its scheme ("github.com/org/repo"),
/// as a note or a result may, or a longer link that starts so: the
/// occurrence stands apart from what precedes it, so it is no part of a
/// schemed link, an address or a longer host.
fn schemeless_grounds(url: &str, corpus: &str) -> bool {
    let Some(rest) = scheme_len(url, true).and_then(|n| url.get(n..)) else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.contains('.')
        && occurrences(corpus, rest).any(|at| {
            !char_before(corpus, at)
                .is_some_and(|c| is_ascii_word(c) || matches!(c, '.' | '-' | '@' | '/' | ':'))
        })
}

/// Whether `corpus` grounds `url`: verbatim, with or without its scheme, as
/// a path a result gave without its host, or as a link built from what it
/// printed on one of `hosts`.
fn grounded(url: &str, corpus: &str, hosts: &[&str]) -> bool {
    corpus.contains(url)
        || path_grounds_url(url, corpus)
        || built_from_corpus(url, corpus, hosts)
        || schemeless_grounds(url, corpus)
}

/// The links in `reply`, once each, that `corpus` does not carry.
///
/// A link is grounded when the corpus holds it verbatim, with or without its
/// scheme ("github.com/org/repo"), or holds its path under no other host (as
/// an API that gives a page's path without its host does), or when it is
/// one a reply may build: a page naming no item on one of `hosts`, the hosts
/// the caller trusts such pages on ("acme.io/new"), or a link in one of the
/// shapes `url::shapes` knows, built from ids the corpus printed (a chat
/// message from its channel and timestamp, a record from its id). A link is read
/// without the punctuation and markup around it, while the corpus stays raw
/// text; a link the corpus printed is always a substring of it.
///
/// # Examples
///
/// ```
/// let reply = "https://acme.io/a and https://acme.io/b";
/// assert_eq!(figures_parser::fabricated_urls(reply, "https://acme.io/a", &[]), ["https://acme.io/b"]);
///
/// // A page on a service's own host that names no item.
/// assert!(figures_parser::fabricated_urls("https://acme.io/new", "", &["acme.io"]).is_empty());
/// ```
#[must_use]
pub fn fabricated_urls<'r>(reply: &'r str, corpus: &str, hosts: &[&str]) -> Vec<&'r str> {
    let mut out: Vec<&str> = Vec::new();
    for url in urls(reply) {
        if !out.contains(&url.text) && !grounded(url.text, corpus, hosts) {
            out.push(url.text);
        }
    }
    out
}

/// Whether `url` stands in for a number rather than linking a thing of
/// its own: its last path segment names no item (a repo: no digit in it) or
/// another number ("/pull/418"). A link to a thing of another kind, such as
/// a Slack message, is that thing's own.
fn stands_in_for_number(url: &str) -> bool {
    let last = Parts::of(url)
        .and_then(|parts| parts.segments().last())
        .unwrap_or_default();
    let digits = last.bytes().filter(u8::is_ascii_digit).count();
    digits == 0 || digits == last.len()
}

/// What a reply's links fail on, kept apart by kind so a caller names each
/// in its own words; [`link_findings`] reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkFindings {
    /// Links no source printed.
    pub unprinted: Vec<String>,
    /// Link targets that are not URLs, each as written (`url` in
    /// `[PR 512](url)`); a file the caller attached under that name is one
    /// to name apart.
    pub placeholders: Vec<String>,
    /// An item's label on a link that is not the item's own, as written.
    pub misdirected: Vec<String>,
    /// Items named without the link a source gave them, each with its links
    /// ("#87 (url)", "#12 (url or url)").
    pub unlinked: Vec<String>,
}

impl LinkFindings {
    /// Whether no link fails.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.unprinted.is_empty()
            && self.placeholders.is_empty()
            && self.misdirected.is_empty()
            && self.unlinked.is_empty()
    }

    /// Every finding in one list, in the order of the fields, a placeholder
    /// target in parentheses (`(url)`).
    #[must_use]
    pub fn all(&self) -> Vec<String> {
        let mut out = self.unprinted.clone();
        out.extend(self.placeholders.iter().map(|target| format!("({target})")));
        out.extend(self.misdirected.iter().cloned());
        out.extend(self.unlinked.iter().cloned());
        out
    }
}

/// The parts of `reply` that neither `corpus` (the thread, the request, the
/// run's tool results) nor `prompt` grounds, by kind:
///
/// - a link neither carries, as [`fabricated_urls`] reads one, `hosts` being
///   the hosts the caller trusts such pages on, and a link on a reserved example
///   domain that only the prompt carried, since the prompt illustrates the
///   format with such links;
/// - a link target that is not a URL (`url` in `[PR 512](url)`), a template
///   copied from an example;
/// - an item's label on a link that stands in for it (`[#429](…/repo)`,
///   `[#429](…/pull/418)`); a number linked to a thing of another kind, such
///   as a Slack message, is that thing's own;
/// - an item named in prose, outside code, without a link although a source
///   carried its page's URL (`…/pull/N` before `…/issues/N`, never an API's),
///   each with that URL, or with each item's when the id names several
///   ("#12 (url or url)"); an example-domain link only the prompt carried is
///   no item's.
///
/// # Examples
///
/// ```
/// let reply = "See [#87](https://acme.io/repo), [PR 512](url) and https://acme.io/made-up.";
/// let findings = figures_parser::link_findings(reply, "https://acme.io/repo", "", &[]);
/// assert_eq!(findings.unprinted, ["https://acme.io/made-up"]);
/// assert_eq!(findings.placeholders, ["url"]);
/// assert_eq!(findings.misdirected, ["[#87](https://acme.io/repo)"]);
/// assert!(findings.unlinked.is_empty());
/// ```
#[must_use]
pub fn link_findings(reply: &str, corpus: &str, prompt: &str, hosts: &[&str]) -> LinkFindings {
    let mut seen: Vec<&str> = Vec::new();
    for url in urls(reply) {
        if !seen.contains(&url.text) {
            seen.push(url.text);
        }
    }
    let mut unprinted: Vec<String> = seen
        .iter()
        .filter(|url| !grounded(url, corpus, hosts) && !prompt.contains(**url))
        .map(|url| (*url).to_owned())
        .collect();
    unprinted.extend(
        seen.iter()
            .filter(|url| is_example_host(url) && prompt.contains(**url) && !corpus.contains(**url))
            .map(|url| (*url).to_owned()),
    );
    let links = identifier_links(reply);
    let misdirected = links
        .iter()
        .filter(|link| {
            link.label.strip_prefix('#').map_or_else(
                || !link.url.contains(link.label),
                |number| !link.url.contains(number) && stands_in_for_number(link.url),
            )
        })
        .map(|link| link.whole.to_owned())
        .collect();
    // The URL goes into the finding: a script that fetched the item may never
    // have printed it. An example-domain link the corpus never carried is the
    // prompt illustrating a format.
    let unlinked = unlinked_items(reply, &links, &[corpus, prompt])
        .into_iter()
        .filter_map(|item| {
            let own: Vec<&str> = item
                .urls
                .into_iter()
                .filter(|url| !is_example_host(url) || corpus.contains(url))
                .collect();
            (!own.is_empty()).then(|| format!("{} ({})", item.id, own.join(" or ")))
        })
        .collect();
    LinkFindings {
        unprinted,
        placeholders: placeholder_targets(reply)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        misdirected,
        unlinked,
    }
}

/// [`link_findings`] in one list, as the correction round names them back
/// to the model: the links, then each placeholder target as `(url)`, the
/// misdirected links and the unlinked items.
///
/// # Examples
///
/// The made-up link, and an item labelled on its repository's root:
///
/// ```
/// let reply = "See [#87](https://acme.io/repo) and https://acme.io/made-up.";
/// assert_eq!(
///     figures_parser::ungrounded_urls(reply, "https://acme.io/repo", "", &[]),
///     ["https://acme.io/made-up", "[#87](https://acme.io/repo)"]
/// );
/// ```
#[must_use]
pub fn ungrounded_urls(reply: &str, corpus: &str, prompt: &str, hosts: &[&str]) -> Vec<String> {
    link_findings(reply, corpus, prompt, hosts).all()
}

/// A reply with the link defects a rule can put right put right, and each
/// repair as `from → to`; [`mended_links`] makes one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mended {
    /// The reply, mended.
    pub text: String,
    /// Each repair, as `from → to`, in the order made.
    pub mends: Vec<String>,
}

/// `reply` with the link defects a rule can put right put right, so they
/// cost no rewrite.
///
/// A Slack message link that kept its ts's dot loses it
/// (`/p1791180327.741099` to `/p1791180327741099`), and an item named in
/// prose without its link is linked where the reply first names it, when
/// `corpus` gives that id one item's link. The prompt's text never supplies
/// one: an example there would be written into the reply unread.
///
/// # Examples
///
/// ```
/// let corpus = "https://github.com/acme/app/pull/429";
/// let mended = figures_parser::mended_links("Merged #429.", corpus);
/// assert_eq!(mended.text, "Merged [#429](https://github.com/acme/app/pull/429).");
/// assert_eq!(mended.mends, ["#429 → https://github.com/acme/app/pull/429"]);
/// ```
#[must_use]
pub fn mended_links(reply: &str, corpus: &str) -> Mended {
    let mut mends = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut text = reply.to_owned();
    for url in urls(reply) {
        if seen.contains(&url.text) {
            continue;
        }
        seen.push(url.text);
        if let Some(undotted) = undotted_slack_link(url.text) {
            mends.push(format!("{} → {undotted}", url.text));
            text = text.replace(url.text, &undotted);
        }
    }
    let links = identifier_links(&text);
    let mut linked: Vec<(usize, &str, &str)> = unlinked_items(&text, &links, &[corpus])
        .into_iter()
        .filter_map(|item| match item.urls.as_slice() {
            [url] => Some((item.at, item.id, *url)),
            _ => None,
        })
        .collect();
    for &(_, id, url) in &linked {
        mends.push(format!("{id} → {url}"));
    }
    // Spliced from the last back, so each earlier offset still holds.
    linked.sort_by_key(|&(at, ..)| std::cmp::Reverse(at));
    let mut spliced = text.clone();
    for (at, id, url) in linked {
        if let Some(end) = at.checked_add(id.len())
            && spliced.get(at..end) == Some(id)
        {
            spliced.replace_range(at..end, &format!("[{id}]({url})"));
        }
    }
    Mended {
        text: spliced,
        mends,
    }
}

/// An item named in a reply's prose without a link, with its links.
struct Unlinked<'r, 's> {
    /// The identifier, as written.
    id: &'r str,
    /// The byte where the prose first names it.
    at: usize,
    /// Its links in the sources, one per item: more than one when the id
    /// names several.
    urls: Vec<&'s str>,
}

/// The items `reply` names in prose without linking them anywhere although
/// one of `sources` carried their URL (`…/pull/N`, `…/issues/N`,
/// `…/TEAM-N`), each with its links, one per item the id names. An id
/// inside code is code, and an API's link is no item's page.
fn unlinked_items<'r, 's>(
    reply: &'r str,
    links: &[IdentifierLink<'_>],
    sources: &[&'s str],
) -> Vec<Unlinked<'r, 's>> {
    let markdown = markdown_links(reply);
    let mut blanked = markdown.clone();
    blanked.extend(code_spans(reply));
    let mut out = Vec::new();
    for (at, id) in bare_identifiers(reply, &blanked) {
        if links.iter().any(|link| link.label == id) {
            continue;
        }
        // The page's tail before the API's: "/pull/N" before "/issues/N",
        // which GitHub opens as the same number.
        let fragments: Vec<String> = id.strip_prefix('#').map_or_else(
            || vec![format!("/{id}")],
            |n| vec![format!("/pull/{n}"), format!("/issues/{n}")],
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
        // One link per item, in the first fragment's form.
        let mut items: Vec<(&str, &str)> = Vec::new();
        for fragment in &fragments {
            for source in sources {
                for url in urls_with_fragment(source, fragment) {
                    let item = url
                        .get(..url.len().saturating_sub(fragment.len()))
                        .unwrap_or(url);
                    if !is_api_link(url) && !items.iter().any(|&(seen, _)| seen == item) {
                        items.push((item, url));
                    }
                }
            }
        }
        if !items.is_empty() {
            out.push(Unlinked {
                id,
                at,
                urls: items.into_iter().map(|(_, url)| url).collect(),
            });
        }
    }
    out
}
