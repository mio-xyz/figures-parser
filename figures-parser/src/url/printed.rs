//! Whether a text carried an address the caller is about to read or has
//! failed to read, so a failure on one it never carried is known to be on an
//! address put together by the writer, not on a link of the site.

/// `text` without one trailing `/`.
fn without_trailing_slash(text: &str) -> &str {
    text.strip_suffix('/').unwrap_or(text)
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
    let mut path = String::with_capacity(
        parsed
            .path()
            .len()
            .saturating_add(query.map_or(0, |q| q.len().saturating_add(1))),
    );
    path.push_str(without_trailing_slash(parsed.path()));
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
