//! Whether a text carried an address the caller is about to read or has
//! failed to read, so a failure on one it never carried is known to be on an
//! address put together by the writer, not on a link of the site.

use super::{is_double_dot, is_single_dot};

/// `text` without one trailing `/`.
fn without_trailing_slash(text: &str) -> &str {
    text.strip_suffix('/').unwrap_or(text)
}

/// Whether `segment` is a Windows drive letter (`C:`, `C|`), or only the
/// normalized form (`C:`) when `normalized`.
fn is_drive_letter(segment: &str, normalized: bool) -> bool {
    let mut chars = segment.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic()
    ) || (!normalized
        && matches!(
            (segment.chars().next(), segment.chars().nth(1), segment.chars().nth(2)),
            (Some(letter), Some('|'), None) if letter.is_ascii_alphabetic()
        ))
}

/// Whether a character in a path is escaped: a control, a space, a
/// character outside ASCII, a backtick, or one of `"#<>?^{}`.
const fn escaped_in_path(c: char) -> bool {
    c <= ' ' || c >= '\u{7f}' || matches!(c, '"' | '#' | '<' | '>' | '?' | '^' | '`' | '{' | '}')
}

/// Whether the authority of an address whose scheme (`scheme`, lowercased)
/// has no special meaning carries a port that is no number (`foo://C:\\x`):
/// the WHATWG URL standard rejects the address, where the `url` crate takes
/// the port as empty and the rest as a path.
fn rejects_port(url: &str, scheme: &str) -> bool {
    if matches!(scheme, "http" | "https" | "ws" | "wss" | "ftp" | "file") {
        return false;
    }
    let Some(after) = url
        .get(scheme.len().saturating_add(1)..)
        .and_then(|rest| rest.strip_prefix("//"))
    else {
        return false;
    };
    let authority = after
        .get(..after.find(['/', '?', '#']).unwrap_or(after.len()))
        .unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let port = if host.starts_with('[') {
        host.split_once(']').map_or("", |(_, rest)| rest)
    } else {
        host
    };
    port.split_once(':')
        .is_some_and(|(_, port)| !port.bytes().all(|b| b.is_ascii_digit()))
}

/// The path of an address as the WHATWG URL standard parses it, for a
/// scheme whose path is a list of segments: the `url` crate leaves a `^` as
/// written, reads a drive letter outside `file:` and drops the empty
/// segments of a `file:` path (`file:////` is `//`). `url` is the address
/// with its tabs and newlines removed and its ends trimmed, and `scheme` its
/// scheme, lowercased.
fn whatwg_path(url: &str, scheme: &str) -> Option<String> {
    let file = scheme == "file";
    let special = matches!(scheme, "http" | "https" | "ws" | "wss" | "ftp" | "file");
    let rest = url.get(scheme.len().saturating_add(1)..)?;
    let is_slash = |c: char| c == '/' || (special && c == '\\');
    let authority_end = |after: &str| {
        after
            .find(|c: char| is_slash(c) || matches!(c, '?' | '#'))
            .unwrap_or(after.len())
    };
    let mut chars = rest.chars();
    let two_slashes = chars.next().is_some_and(is_slash) && chars.next().is_some_and(is_slash);
    let path_input = if file {
        if two_slashes {
            let after = rest.get(2..)?;
            let host_len = authority_end(after);
            if is_drive_letter(after.get(..host_len)?, false) {
                after
            } else {
                after.get(host_len..)?
            }
        } else {
            rest
        }
    } else if special {
        let after = rest.trim_start_matches(is_slash);
        after.get(authority_end(after)..)?
    } else if two_slashes {
        let after = rest.get(2..)?;
        after.get(authority_end(after)..)?
    } else if rest.starts_with('/') {
        rest
    } else {
        return None;
    };
    let path_input = path_input.split(['?', '#']).next().unwrap_or_default();
    if path_input.is_empty() && !special {
        return Some(String::new());
    }
    let path_input = path_input.strip_prefix(is_slash).unwrap_or(path_input);
    let mut segments: Vec<String> = Vec::new();
    let mut parts = path_input.split(is_slash).peekable();
    while let Some(part) = parts.next() {
        let last = parts.peek().is_none();
        let mut buffer = String::with_capacity(part.len());
        for c in part.chars() {
            if escaped_in_path(c) {
                let mut bytes = [0; 4];
                for b in c.encode_utf8(&mut bytes).bytes() {
                    buffer.push('%');
                    buffer.push(char::from(
                        b"0123456789ABCDEF".get(usize::from(b >> 4)).copied()?,
                    ));
                    buffer.push(char::from(
                        b"0123456789ABCDEF".get(usize::from(b & 15)).copied()?,
                    ));
                }
            } else {
                buffer.push(c);
            }
        }
        if is_double_dot(&buffer) {
            let drive_only = file
                && segments.len() == 1
                && segments
                    .first()
                    .is_some_and(|first| is_drive_letter(first, true));
            if !drive_only {
                segments.pop();
            }
            if last {
                segments.push(String::new());
            }
        } else if is_single_dot(&buffer) {
            if last {
                segments.push(String::new());
            }
        } else {
            if file && segments.is_empty() && is_drive_letter(&buffer, false) {
                buffer.replace_range(1..2, ":");
            }
            segments.push(buffer);
        }
    }
    let mut path = String::with_capacity(path_input.len().saturating_add(1));
    for segment in &segments {
        path.push('/');
        path.push_str(segment);
    }
    if segments.is_empty() {
        path.push('/');
    }
    Some(path)
}

/// Whether `read` carries `url` whole, without its scheme, or as the path a
/// page links it by.
///
/// The address is parsed as a browser parses it (the WHATWG URL standard):
/// its host lowercased and without one leading `www.`, its path normalized
/// and without a trailing `/`, its query kept, its fragment dropped. `read`
/// carries it when it holds, as a substring, the address as given (less a
/// trailing `/`), the host and path together ("acme.com/blog/post"), or the
/// path alone ("/blog/post", as a page's relative link gives it); a bare root
/// has no path form. An address that does not parse is carried only whole.
///
/// What `read` is (every page, message and tool result a turn read) is the
/// caller's to assemble: this tells an address the turn put together itself
/// from one it was given, so that a failed read of it says nothing about the
/// site.
///
/// # Examples
///
/// ```
/// use figures_parser::url_printed_in;
///
/// let url = "https://www.acme.com/blog/when-marking-runs-late/";
/// assert!(url_printed_in(url, "[When marking runs late](/blog/when-marking-runs-late)"));
/// assert!(url_printed_in(url, "see acme.com/blog/when-marking-runs-late"));
/// // A title is no link: the address was built from it.
/// assert!(!url_printed_in(url, "## The Journal\n## When marking runs late"));
/// ```
#[must_use]
pub fn url_printed_in(url: &str, read: &str) -> bool {
    let whole = without_trailing_slash(url);
    if read.contains(whole) {
        return true;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default();
    let host = host.strip_prefix("www.").unwrap_or(host);
    let query = parsed.query().filter(|q| !q.is_empty());
    let cleaned: String = url
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    if rejects_port(&cleaned, parsed.scheme()) {
        return false;
    }
    let segmented = (!parsed.cannot_be_a_base())
        .then(|| whatwg_path(&cleaned, parsed.scheme()))
        .flatten();
    let parsed_path = segmented.as_deref().unwrap_or_else(|| parsed.path());
    let mut path = String::with_capacity(
        parsed_path
            .len()
            .saturating_add(query.map_or(0, |q| q.len().saturating_add(1))),
    );
    path.push_str(without_trailing_slash(parsed_path));
    if let Some(query) = query {
        path.push('?');
        path.push_str(query);
    }
    if !path.is_empty() && read.contains(path.as_str()) {
        return true;
    }
    let port = parsed.port().map(|port| port.to_string());
    let mut hosted = String::with_capacity(host.len().saturating_add(path.len()).saturating_add(6));
    hosted.push_str(host);
    if let Some(port) = port {
        hosted.push(':');
        hosted.push_str(&port);
    }
    hosted.push_str(&path);
    read.contains(hosted.as_str())
}
