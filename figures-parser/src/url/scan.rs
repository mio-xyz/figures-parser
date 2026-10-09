//! The http(s) links of a text, found by `linkify`.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::LazyLock;

use linkify::{LinkFinder, LinkKind};

use super::{has_http_scheme, scheme_len};

/// One http(s) link, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url<'a> {
    /// As written, without the markup or punctuation that closed around it.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
}

/// Finds links with a scheme only: a bare domain is not a link a tool returned.
static FINDER: LazyLock<LinkFinder> = LazyLock::new(|| {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]).url_must_have_scheme(true);
    finder
});

/// Whether `c` is markup or punctuation a link comes back with when it was
/// written inside `**bold**` or `_emphasis_` or ended a sentence.
const fn is_trailing_markup(c: char) -> bool {
    matches!(c, '*' | '_' | '`' | '~' | '.' | ',' | ';' | ':')
}

/// The byte ranges of the wrapping of every Slack link in `text`: the `<`, and
/// the `|label>` (or `\|label>`, or `>`) after the URL.
fn slack_wrapping(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find('<')) {
        let open = from + found;
        from = open + 1;
        let Some(rest) = text.get(from..) else {
            break;
        };
        let Some(scheme) = scheme_len(rest, true) else {
            continue;
        };
        let url_len = rest
            .get(scheme..)
            .unwrap_or_default()
            .find(['>', '|', '\\'])
            .unwrap_or(rest.len() - scheme);
        if url_len == 0 {
            continue;
        }
        let url_end = from + scheme + url_len;
        let after = text.get(url_end..).unwrap_or_default();
        let label = if after.starts_with('>') {
            Some(0)
        } else if after.starts_with('|') || after.starts_with("\\|") {
            after.find('>')
        } else {
            None
        };
        let Some(close) = label.map(|at| url_end + at) else {
            continue;
        };
        out.push(open..open + 1);
        out.push(url_end..close + 1);
        from = close + 1;
    }
    out
}

/// `text` with every Slack link reduced to its URL and spaces, at the same
/// byte offsets; borrowed when there is none.
fn unwrap_slack(text: &str) -> Cow<'_, str> {
    let wrapping = slack_wrapping(text);
    if wrapping.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for range in wrapping {
        out.push_str(text.get(at..range.start).unwrap_or_default());
        out.extend(std::iter::repeat_n(' ', range.len()));
        at = range.end;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    Cow::Owned(out)
}

/// Every http(s) link in `text`, in order, Slack's `<url|label>` included.
#[must_use]
pub fn urls(text: &str) -> Vec<Url<'_>> {
    let unwrapped = unwrap_slack(text);
    FINDER
        .links(&unwrapped)
        .filter(|link| has_http_scheme(link.as_str()))
        .filter_map(|link| {
            let found = link.as_str().trim_end_matches(is_trailing_markup);
            let span = link.start()..link.start() + found.len();
            Some(Url {
                text: text.get(span.clone())?,
                span,
            })
        })
        .collect()
}
