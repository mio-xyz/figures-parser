//! The pages a message links that the thread has not opened yet.

use super::scan::urls;

/// Whether `host`, lowercased, is `slack.com` or a host under it.
fn is_slack_host(host: &str) -> bool {
    host == "slack.com" || host.ends_with(".slack.com")
}

/// The links in `text`, once each, in order, that name a page to open.
///
/// A link on `slack.com` or a host under it is none, since the chat's own
/// tools read it, nor is one of `opened`, the links the thread's earlier
/// turns already went to.
///
/// A link inside a request ("a footer with acme.io's logo") otherwise reads
/// as a label, and a reply is built from what the thread holds instead of
/// from the page. The host is read as a browser parses it, so a link that
/// does not parse is none to open.
///
/// # Examples
///
/// ```
/// let text = "Match https://acme.io/brand and https://acme.slack.com/archives/C1/p1, see https://acme.io/brand.";
/// assert_eq!(figures_parser::unopened_links(text, &[]), ["https://acme.io/brand"]);
/// assert!(figures_parser::unopened_links(text, &["https://acme.io/brand"]).is_empty());
/// ```
#[must_use]
pub fn unopened_links<'t>(text: &'t str, opened: &[&str]) -> Vec<&'t str> {
    let mut out: Vec<&str> = Vec::new();
    for url in urls(text) {
        if out.contains(&url.text) || opened.contains(&url.text) {
            continue;
        }
        let page = url::Url::parse(url.text)
            .ok()
            .and_then(|parsed| parsed.host_str().map(|host| !is_slack_host(host)))
            .unwrap_or(false);
        if page {
            out.push(url.text);
        }
    }
    out
}
