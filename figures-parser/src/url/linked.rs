//! A text's links taken out of it, so that their digits are never misread
//! as its other items.

use std::borrow::Cow;

use super::scan::urls;

/// `text` with every http(s) link it holds put out as a space, wherever the
/// link's text recurs.
///
/// A link's digits (a chat timestamp, an id) are the link check's, never a
/// figure or a duration. A link that starts another ("a.io/" and
/// "a.io/206788") takes out only itself, the longer first.
///
/// # Examples
///
/// ```
/// let text = "See https://acme.io/p/206788 and the 4,812 orders.";
/// assert_eq!(figures_parser::without_links(text), "See   and the 4,812 orders.");
/// assert_eq!(figures_parser::without_links("no link here"), "no link here");
/// ```
#[must_use]
pub fn without_links(text: &str) -> Cow<'_, str> {
    if !text.contains("://") {
        return Cow::Borrowed(text);
    }
    let mut links: Vec<&str> = Vec::new();
    for url in urls(text) {
        if !links.contains(&url.text) {
            links.push(url.text);
        }
    }
    if links.is_empty() {
        return Cow::Borrowed(text);
    }
    links.sort_by_key(|link| std::cmp::Reverse(link.len()));
    let mut out = text.to_owned();
    for link in links {
        if out.contains(link) {
            out = out.replace(link, " ");
        }
    }
    Cow::Owned(out)
}
