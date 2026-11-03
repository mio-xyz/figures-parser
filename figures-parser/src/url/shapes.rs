//! Links a reply may build rather than copy: a page naming no item on a
//! service's own host, or an item's link in the shape its service gives it,
//! built from ids the turn printed, for services whose results print an
//! item's id but not its link. Any other link is one the turn has to have
//! printed.

use super::ground::occurrences;
use super::{is_double_dot, is_single_dot, scheme_len};
use crate::text::contains_ignoring_case;

/// The parts of an http(s) link.
pub(super) struct Parts<'a> {
    /// The host, without userinfo or a port.
    pub host: &'a str,
    /// The path, from its first slash; empty for a host root.
    pub path: &'a str,
    /// The query after its `?`, when there is one and it is not empty.
    pub query: Option<&'a str>,
    /// The fragment after its `#`, when there is one and it is not empty.
    pub fragment: Option<&'a str>,
}

impl<'a> Parts<'a> {
    /// The parts of `url`; None when it is no http(s) link with a host.
    pub(super) fn of(url: &'a str) -> Option<Self> {
        let rest = url.get(scheme_len(url, true)?..)?;
        let host_len = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
        let authority = rest.get(..host_len)?;
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let host = host.split(':').next().unwrap_or(host);
        if host.is_empty() {
            return None;
        }
        let after = rest.get(host_len..)?;
        let (before_fragment, fragment) = after
            .split_once('#')
            .map_or((after, None), |(a, f)| (a, Some(f)));
        let (path, query) = before_fragment
            .split_once('?')
            .map_or((before_fragment, None), |(p, q)| (p, Some(q)));
        Some(Self {
            host,
            path,
            query: query.filter(|q| !q.is_empty()),
            fragment: fragment.filter(|f| !f.is_empty()),
        })
    }

    /// The path's segments as a browser resolves them, `.` and `..` read and
    /// empty ones dropped, a `\\` separating them as a `/` does.
    pub(super) fn segments(&self) -> Segments<'a> {
        let path = self.path.strip_prefix(['/', '\\']).unwrap_or(self.path);
        let split = path.split(['/', '\\']);
        if !split
            .clone()
            .any(|segment| is_single_dot(segment) || is_double_dot(segment))
        {
            return Segments::Plain(split);
        }
        let mut resolved: Vec<&'a str> = Vec::new();
        for segment in split {
            if is_double_dot(segment) {
                resolved.pop();
            } else if !is_single_dot(segment) {
                resolved.push(segment);
            }
        }
        resolved.retain(|segment| !segment.is_empty());
        Segments::Resolved(resolved.into_iter())
    }

    /// The query's pairs, each a key and a value (empty without an `=`).
    fn pairs(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.query
            .unwrap_or_default()
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
    }

    /// Whether the host is `name`, or a host under it, case aside.
    pub(super) fn host_under(&self, name: &str) -> bool {
        self.host.eq_ignore_ascii_case(name)
            || self
                .host
                .len()
                .checked_sub(name.len().saturating_add(1))
                .and_then(|at| self.host.get(at..))
                .and_then(|tail| tail.strip_prefix('.'))
                .is_some_and(|tail| tail.eq_ignore_ascii_case(name))
    }

    /// The host without a leading `www.`, which serves the same pages.
    fn host_without_www(&self) -> &'a str {
        self.host
            .get(..4)
            .filter(|www| www.eq_ignore_ascii_case("www."))
            .and_then(|_| self.host.get(4..))
            .unwrap_or(self.host)
    }
}

/// A link's path segments: as written when none is `.` or `..`, which is
/// what a path almost always is, else resolved.
pub(super) enum Segments<'a> {
    /// Split from the path, empty segments still to drop.
    Plain(std::str::Split<'a, [char; 2]>),
    /// Resolved, empty segments dropped.
    Resolved(std::vec::IntoIter<&'a str>),
}

impl<'a> Iterator for Segments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        match self {
            Self::Plain(split) => split.find(|segment| !segment.is_empty()),
            Self::Resolved(resolved) => resolved.next(),
        }
    }
}

/// Whether `s` is a run of ASCII digits.
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `corpus` prints `id` standing apart, never inside a longer id or
/// number.
fn prints_id(corpus: &str, id: &str) -> bool {
    !id.is_empty()
        && occurrences(corpus, id).any(|at| {
            let before = corpus.get(..at).and_then(|s| s.bytes().next_back());
            let after = corpus
                .get(at.saturating_add(id.len())..)
                .and_then(|s| s.bytes().next());
            !before.is_some_and(|b| b.is_ascii_alphanumeric())
                && !after.is_some_and(|b| b.is_ascii_alphanumeric())
        })
}

/// A Slack message, `/archives/<channel>/p<its ts, the dot dropped>`, on a
/// workspace the turn printed: built from its channel and ts when the turn
/// printed both. A reply in a thread links with its parent's ts and its
/// channel in the query, ids too; any other query is none of the shape. A
/// link that keeps the ts's dot is no message's.
fn slack_message(parts: &Parts<'_>, corpus: &str) -> bool {
    let mut segments = parts.segments();
    let (Some("archives"), Some(channel), Some(message), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };
    let Some((seconds, micros)) = message
        .strip_prefix('p')
        .filter(|ts| ts.len() == 16 && is_number(ts))
        .and_then(|ts| ts.split_at_checked(10))
    else {
        return false;
    };
    let query_printed = parts
        .pairs()
        .all(|(key, value)| matches!(key, "thread_ts" | "cid") && prints_id(corpus, value));
    query_printed && prints_id(corpus, channel) && prints_id(corpus, &format!("{seconds}.{micros}"))
}

/// `url` with the dot its Slack message ts kept dropped, or None for a
/// message link already right, a ts of another length or a link off Slack.
///
/// `/archives/<channel>/p1791180327.741099` becomes `…/p1791180327741099`,
/// the query and fragment as written.
///
/// # Examples
///
/// ```
/// use figures_parser::undotted_slack_link;
///
/// let dotted = "https://acme.slack.com/archives/C09R2NX3D1R/p1791180327.741099";
/// assert_eq!(
///     undotted_slack_link(dotted).as_deref(),
///     Some("https://acme.slack.com/archives/C09R2NX3D1R/p1791180327741099")
/// );
/// assert_eq!(undotted_slack_link("https://acme.io/p1791180327.741099"), None);
/// ```
#[must_use]
pub fn undotted_slack_link(url: &str) -> Option<String> {
    let parts = Parts::of(url)?;
    let host = parts.host.to_ascii_lowercase();
    if !host.ends_with(".slack.com") {
        return None;
    }
    let mut segments = parts.segments();
    let (Some("archives"), Some(_channel), Some(message), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return None;
    };
    let (seconds, micros) = message.strip_prefix('p')?.split_once('.')?;
    let dotted =
        seconds.len() == 10 && is_number(seconds) && micros.len() == 6 && is_number(micros);
    dotted.then(|| url.replacen(&format!("/{message}"), &format!("/p{seconds}{micros}"), 1))
}

/// A Pipedrive record, `/deal/<id>`, `/person/<id>` or `/organization/<id>`,
/// built from an id the turn printed; a query is none of the shape.
fn pipedrive_record(parts: &Parts<'_>, corpus: &str) -> bool {
    let mut segments = parts.segments();
    let (Some("deal" | "person" | "organization"), Some(id), None) =
        (segments.next(), segments.next(), segments.next())
    else {
        return false;
    };
    parts.query.is_none() && is_number(id) && prints_id(corpus, id)
}

/// A Facebook page's ads in Meta's Ad Library,
/// `/ads/library/?view_all_page_id=<page id>` beside the library's filters,
/// built from a page id the turn printed.
fn ad_library(parts: &Parts<'_>, corpus: &str) -> bool {
    let mut segments = parts.segments();
    let library = matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some("ads"), Some("library"), None)
    );
    library
        && parts
            .pairs()
            .find(|&(key, _)| key == "view_all_page_id")
            .is_some_and(|(_, page)| is_number(page) && prints_id(corpus, page))
}

/// Whether `url` names no item: a host root, or on a trusted host (one in
/// `hosts`) one segment without a digit (`github.com/new`, though a profile's
/// name passes with it) or one under GitHub's settings, which no account can
/// be named for. A shape's host gets its root only, whatever `hosts`
/// declares: one segment there names a page (`facebook.com/<page>`). Any
/// other two segments read as an owner and a repo, a name the model can
/// invent.
fn names_no_item(parts: &Parts<'_>, shape_host: bool, hosts: &[&str]) -> bool {
    if parts.query.is_some() || parts.fragment.is_some() {
        return false;
    }
    let trusted = || hosts.iter().any(|host| parts.host_under(host));
    let mut segments = parts.segments();
    let Some(first) = segments.next() else {
        return shape_host || trusted();
    };
    if shape_host {
        return false;
    }
    let page = match (segments.next(), segments.next()) {
        (None, _) => first,
        (Some(second), None)
            if first == "settings"
                && parts.host_without_www().eq_ignore_ascii_case("github.com") =>
        {
            second
        }
        _ => return false,
    };
    !page.bytes().any(|b| b.is_ascii_digit()) && trusted()
}

/// Whether `url` is a link a reply may build rather than copy.
///
/// That is a page naming no item on a host in `hosts` (the hosts the caller
/// trusts such pages on, lowercased; a host under one counts), or an item's
/// link in its service's shape with ids `corpus` prints: a chat message from
/// its channel and ts on a workspace the corpus names, a CRM record from its
/// id, an ad library page from its page id. No shape's link carries a
/// fragment. [`fabricated_urls`](crate::fabricated_urls) grounds such a link
/// besides one the corpus printed.
///
/// # Examples
///
/// ```
/// use figures_parser::built_from_corpus;
///
/// let corpus = "posted in C09R2NX3D1R at 1791180327.741099 on acme.slack.com";
/// let link = "https://acme.slack.com/archives/C09R2NX3D1R/p1791180327741099";
/// assert!(built_from_corpus(link, corpus, &[]));
/// assert!(built_from_corpus("https://github.com/new", "", &["github.com"]));
/// assert!(!built_from_corpus("https://github.com/acme/app", "", &["github.com"]));
/// ```
#[must_use]
pub fn built_from_corpus(url: &str, corpus: &str, hosts: &[&str]) -> bool {
    let Some(parts) = Parts::of(url) else {
        return false;
    };
    let printed_host = || contains_ignoring_case(corpus, parts.host);
    let slack = parts.host_under("slack.com")
        && !parts.host.eq_ignore_ascii_case("slack.com")
        && printed_host();
    let pipedrive = parts.host.eq_ignore_ascii_case("app.pipedrive.com")
        || (parts.host_under("pipedrive.com")
            && !parts.host.eq_ignore_ascii_case("pipedrive.com")
            && printed_host());
    let facebook = parts.host.eq_ignore_ascii_case("www.facebook.com")
        || parts.host.eq_ignore_ascii_case("facebook.com");
    if names_no_item(&parts, slack || pipedrive || facebook, hosts) {
        return true;
    }
    parts.fragment.is_none()
        && ((slack && slack_message(&parts, corpus))
            || (pipedrive && pipedrive_record(&parts, corpus))
            || (facebook && ad_library(&parts, corpus)))
}
