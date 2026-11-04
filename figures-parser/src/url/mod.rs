//! The url kind: the http(s) links a text holds, and which of a reply's the
//! turn it answers does not carry.

mod ground;
mod items;
mod linked;
mod opened;
mod printed;
mod scan;
mod shapes;

pub use ground::{
    LinkFindings, Mended, fabricated_urls, link_findings, mended_links, ungrounded_urls,
};
pub use items::placeholder_targets;
pub use linked::without_links;
pub use opened::unopened_links;
pub use printed::url_printed_in;
pub use scan::{Url, urls};
pub use shapes::{built_from_corpus, undotted_message_link};

/// The length of the `http://` or `https://` that opens `text`, compared in
/// any case when `any_case`, else exactly.
fn scheme_len(text: &str, any_case: bool) -> Option<usize> {
    let starts = |prefix: &str| {
        text.get(..prefix.len()).is_some_and(|head| {
            if any_case {
                head.eq_ignore_ascii_case(prefix)
            } else {
                head == prefix
            }
        })
    };
    if starts("https://") {
        Some("https://".len())
    } else if starts("http://") {
        Some("http://".len())
    } else {
        None
    }
}

/// Whether a path segment is `.`, written plain or escaped.
fn is_single_dot(segment: &str) -> bool {
    segment == "." || segment.eq_ignore_ascii_case("%2e")
}

/// Whether a path segment is `..`, written plain or escaped.
fn is_double_dot(segment: &str) -> bool {
    segment == ".."
        || [".%2e", "%2e.", "%2e%2e"]
            .iter()
            .any(|form| segment.eq_ignore_ascii_case(form))
}
