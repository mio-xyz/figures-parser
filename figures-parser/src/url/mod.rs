//! The url kind: the http(s) links a text holds, and which of a reply's the
//! turn it answers does not carry.

mod ground;
mod items;
mod scan;

pub use ground::{fabricated_urls, ungrounded_urls};
pub use scan::{Url, urls};

/// Whether `url` opens with `http://` or `https://`, its scheme in any case.
fn has_http_scheme(url: &str) -> bool {
    scheme_len(url, true).is_some()
}

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
