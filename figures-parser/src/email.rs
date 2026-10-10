//! The email kind: the addresses a text writes, and which of a reply's the
//! turn it answers never gave.

use std::ops::Range;
use std::sync::LazyLock;

use linkify::{LinkFinder, LinkKind};

use crate::text::{carried, is_example_domain, report_once};

/// One email address, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email<'a> {
    /// The address as written.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
}

/// Finds addresses whose domain has a dot, as an address a tool returned does.
static FINDER: LazyLock<LinkFinder> = LazyLock::new(|| {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Email]);
    finder
});

/// Every email address in `text`, in order, a `mailto:` link's included.
///
/// # Examples
///
/// ```
/// let text = "Write to ana@acme.io or [Ben](mailto:ben@acme.io).";
/// let found: Vec<&str> = figures_parser::emails(text).iter().map(|e| e.text).collect();
/// assert_eq!(found, ["ana@acme.io", "ben@acme.io"]);
/// ```
#[must_use]
pub fn emails(text: &str) -> Vec<Email<'_>> {
    FINDER
        .links(text)
        .map(|link| Email {
            text: link.as_str(),
            span: link.start()..link.end(),
        })
        .collect()
}

/// The addresses in `reply`, once each, that neither `corpus` (the thread,
/// the request, the run's tool results) nor `prompt` carries, case aside.
///
/// An address on a reserved example domain counts as carried only by the
/// corpus, since a prompt illustrates the format with such addresses.
///
/// # Examples
///
/// ```
/// let corpus = r#"{"owner": "Ana@Acme.io"}"#;
/// let reply = "Ask ana@acme.io or ben@acme.io.";
/// assert_eq!(figures_parser::ungrounded_emails(reply, corpus, ""), ["ben@acme.io"]);
/// ```
#[must_use]
pub fn ungrounded_emails(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for email in emails(reply) {
        let address = email.text;
        let example = address
            .rsplit_once('@')
            .is_some_and(|(_, domain)| is_example_domain(domain));
        let prompt = if example { "" } else { prompt };
        report_once(&mut out, address, || carried(address, corpus, prompt));
    }
    out
}
