//! The http(s) links of a text, read as linkify-it reads a link with a
//! scheme, cut where markup or chat syntax closes a link that linkify-it
//! reads on through.

use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;

use unicode_general_category::{GeneralCategory, get_general_category};

use super::scheme_len;

/// One http(s) link, borrowed from the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url<'a> {
    /// As written, without the markup or punctuation that closed around it.
    pub text: &'a str,
    /// Byte range of `text` in the scanned text.
    pub span: Range<usize>,
}

/// The UTF-16 units linkify-it reads after a scheme.
const MAX_LENGTH: usize = 10_000;

/// The most items a bracket pair in a path holds.
const PAIR_ITEMS: usize = 1_000;

/// How deep bracket pairs nest in a path.
const PAIR_DEPTH: u8 = 4;

/// The most UTF-16 units a quoted part of a path holds.
const QUOTED_UNITS: usize = 100;

/// The most UTF-16 units a user name before a host holds.
const USER_UNITS: usize = 50;

/// The most characters a host label holds.
const LABEL_CHARS: usize = 63;

/// The most labels a host holds.
const HOST_LABELS: usize = 11;

/// The longest label a chat link's `|label>` carries, in bytes.
const LABEL_BYTES: usize = 1_000;

/// How a character reads in a link, as linkify-it classes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A space, a line or paragraph separator, or a control character.
    Space,
    /// Unicode punctuation.
    Punct,
    /// `<`, `>` or `｜`, which part a link from the text around it.
    Separator,
    /// Anything else: a letter, a digit, a symbol.
    Letter,
}

/// The class of `c`.
fn class_of(c: char) -> Class {
    if c.is_ascii() {
        return match c {
            '\0'..=' ' | '\u{7f}' => Class::Space,
            '<' | '>' => Class::Separator,
            '!' | '"' | '#' | '%' | '&' | '\'' | '(' | ')' | '*' | ',' | '-' | '.' | '/' | ':'
            | ';' | '?' | '@' | '[' | '\\' | ']' | '_' | '{' | '}' => Class::Punct,
            _ => Class::Letter,
        };
    }
    if c == '｜' {
        return Class::Separator;
    }
    if c.is_control() {
        return Class::Space;
    }
    let category = get_general_category(c);
    if matches!(
        category,
        GeneralCategory::SpaceSeparator
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    ) {
        Class::Space
    } else if matches!(
        category,
        GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::OtherPunctuation
    ) {
        Class::Punct
    } else {
        Class::Letter
    }
}

/// Whether `label` is an encoded host label: `xn--`, then one to 59 ASCII
/// letters, digits and dashes.
fn is_punycode(label: &str) -> bool {
    label
        .get(..4)
        .is_some_and(|head| head.eq_ignore_ascii_case("xn--"))
        && label.get(4..).is_some_and(|tail| {
            (1..=59).contains(&tail.len())
                && tail.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// One candidate link's text, as the validator sees it.
struct Reader<'t> {
    /// The text read.
    text: &'t str,
    /// The byte from which the validator sees nothing.
    limit: usize,
    /// Whether to read as linkify-it reads it, without this crate's cuts and
    /// user names.
    faithful: bool,
    /// Whether a cut linkify-it does not make was met: a reading that met
    /// none ends where linkify-it's does.
    cut: Cell<bool>,
}

impl Reader<'_> {
    /// The character at byte `at`, None at or past the limit.
    fn char_at(&self, at: usize) -> Option<char> {
        if at >= self.limit {
            return None;
        }
        crate::text::char_at(self.text, at)
    }

    /// The bytes from `at` to the limit.
    fn bytes_from(&self, at: usize) -> &[u8] {
        self.text
            .as_bytes()
            .get(at..self.limit.min(self.text.len()))
            .unwrap_or_default()
    }

    /// The character at byte `at` and how it reads. A backtick, or a `|` or
    /// `\|` that opens a chat link's label, reads as a space: the link ends
    /// there, though linkify-it reads on.
    fn read(&self, at: usize) -> Option<(char, Class)> {
        let c = self.char_at(at)?;
        let class = if !self.faithful && (c == '`' || self.opens_label(at, c)) {
            self.cut.set(true);
            Class::Space
        } else {
            class_of(c)
        };
        Some((c, class))
    }

    /// Whether the `|` or `\|` at byte `at` opens the label of a chat link
    /// whose `<` is not there: a `>` closes it on the same line, with no `<`
    /// or `|` before it.
    fn opens_label(&self, at: usize, c: char) -> bool {
        let label = match c {
            '|' => at.saturating_add(1),
            '\\' if self.text.as_bytes().get(at.saturating_add(1)) == Some(&b'|') => {
                at.saturating_add(2)
            }
            _ => return false,
        };
        self.text
            .as_bytes()
            .get(label..)
            .unwrap_or_default()
            .iter()
            .take(LABEL_BYTES)
            .find(|&&b| matches!(b, b'>' | b'<' | b'|' | b'\n'))
            == Some(&b'>')
    }

    /// Whether byte `at` is past the end, or reads as a space.
    fn space_or_end(&self, at: usize) -> bool {
        self.read(at).is_none_or(|(_, class)| class == Class::Space)
    }

    /// Whether a host may end at byte `at`: the text ends, or a separator, a
    /// space or punctuation follows that is neither `-`, `_`, a port's `:`,
    /// nor a dot before more of a host.
    fn host_ends_at(&self, at: usize) -> bool {
        let Some((c, class)) = self.read(at) else {
            return true;
        };
        if class == Class::Letter {
            return false;
        }
        let after = at.saturating_add(c.len_utf8());
        match c {
            '-' | '_' => false,
            ':' => !self.char_at(after).is_some_and(|d| d.is_ascii_digit()),
            '.' => match self.read(after) {
                None => true,
                Some(('-', _)) => false,
                Some((_, next)) => matches!(next, Class::Space | Class::Punct),
            },
            _ => true,
        }
    }

    /// The end of the run of host characters (letters and `-`) from `at`,
    /// read no further than one past the longest label.
    fn label_end(&self, at: usize) -> usize {
        let mut end = at;
        for _ in 0..=LABEL_CHARS {
            match self.read(end) {
                Some((c, class)) if class == Class::Letter || c == '-' => {
                    end = end.saturating_add(c.len_utf8());
                }
                _ => break,
            }
        }
        end
    }

    /// The label `start..end`, when it is no longer than a label may be.
    fn label(&self, start: usize, end: usize) -> Option<&str> {
        self.text
            .get(start..end)
            .filter(|label| (1..=LABEL_CHARS).contains(&label.chars().count()))
    }

    /// Whether `start..end` is a host label: encoded, or one to 63
    /// characters that neither open nor close on a dash.
    fn is_label(&self, start: usize, end: usize) -> bool {
        self.text.get(start..end).is_some_and(is_punycode)
            || self
                .label(start, end)
                .is_some_and(|label| !label.starts_with('-') && !label.ends_with('-'))
    }

    /// Whether `start..end` is a host's last label as a link without a
    /// scheme takes it: encoded, or one to 63 characters without a dash.
    fn is_root_label(&self, start: usize, end: usize) -> bool {
        self.text.get(start..end).is_some_and(is_punycode)
            || self
                .label(start, end)
                .is_some_and(|label| !label.contains('-'))
    }

    /// Whether another label follows the dot at byte `at`.
    fn label_follows(&self, at: usize) -> bool {
        self.char_at(at) == Some('.')
            && self
                .read(at.saturating_add(1))
                .is_some_and(|(c, class)| class == Class::Letter || c == '-')
    }

    /// Where the host ends at byte `at`, after the port that opens there if
    /// one does; None when it cannot end there.
    fn port_end(&self, at: usize) -> Option<usize> {
        if self.char_at(at) == Some(':') {
            let digits_start = at.saturating_add(1);
            let digits = self
                .bytes_from(digits_start)
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            let end = digits_start.saturating_add(digits);
            let valid = match digits {
                1..=4 => true,
                5 => self
                    .text
                    .get(digits_start..end)
                    .and_then(|d| d.parse::<u32>().ok())
                    .is_some_and(|port| (10_000..=65_535).contains(&port)),
                _ => false,
            };
            if valid && self.host_ends_at(end) {
                return Some(end);
            }
        }
        self.host_ends_at(at).then_some(at)
    }

    /// The end of a bracketed IPv6 address at byte `at` and its port.
    fn ipv6_end(&self, at: usize) -> Option<usize> {
        let inner = at.saturating_add(1);
        let close = inner.saturating_add(self.bytes_from(inner).iter().position(|&b| b == b']')?);
        self.text
            .get(inner..close)?
            .parse::<std::net::Ipv6Addr>()
            .ok()?;
        self.port_end(close.saturating_add(1))
    }

    /// The end of the host and port of a link with a scheme, from byte `at`.
    fn host_end(&self, at: usize) -> Option<usize> {
        if self.char_at(at) == Some('[') {
            return self.ipv6_end(at);
        }
        let mut start = at;
        for _ in 0..HOST_LABELS {
            let end = self.label_end(start);
            if !self.is_label(start, end) {
                return None;
            }
            if !self.label_follows(end) {
                return self.port_end(end);
            }
            start = end.saturating_add(1);
        }
        None
    }

    /// The end of the host and port of a link without a scheme (`//host`),
    /// from byte `at`: `localhost`, an IPv6 address, or two or more labels.
    fn relative_host_end(&self, at: usize) -> Option<usize> {
        let localhost = at.saturating_add("localhost".len());
        if localhost <= self.limit
            && self
                .text
                .get(at..localhost)
                .is_some_and(|head| head.eq_ignore_ascii_case("localhost"))
            && let Some(end) = self.port_end(localhost)
        {
            return Some(end);
        }
        if self.char_at(at) == Some('[') {
            return self.ipv6_end(at);
        }
        let mut start = at;
        for labels in 1..=HOST_LABELS {
            let end = self.label_end(start);
            if !self.label_follows(end) {
                return (labels > 1 && self.is_root_label(start, end))
                    .then(|| self.port_end(end))
                    .flatten();
            }
            if !self.is_label(start, end) {
                return None;
            }
            start = end.saturating_add(1);
        }
        None
    }

    /// The end of the bracket pair `open`…`close` at byte `at`, nesting at
    /// most `depth` deep.
    fn pair_end(&self, at: usize, open: char, close: char, depth: u8) -> Option<usize> {
        let mut end = at.saturating_add(open.len_utf8());
        let mut items = 0;
        loop {
            let (c, class) = self.read(end)?;
            if c == close {
                return Some(end.saturating_add(c.len_utf8()));
            }
            if items >= PAIR_ITEMS || class == Class::Space {
                return None;
            }
            if c == open {
                if depth <= 1 {
                    return None;
                }
                end = self.pair_end(end, open, close, depth.saturating_sub(1))?;
                items += 1;
            } else {
                end = end.saturating_add(c.len_utf8());
                items += c.len_utf16();
            }
        }
    }

    /// The end of a part quoted with `quote` at byte `at`: one to a hundred
    /// UTF-16 units without a space, then the closing quote.
    fn quoted_end(&self, at: usize, quote: char) -> Option<usize> {
        let mut end = at.saturating_add(1);
        let mut units = 0;
        loop {
            let (c, class) = self.read(end)?;
            if c == quote {
                return (units > 0).then(|| end.saturating_add(1));
            }
            if class == Class::Space || units >= QUOTED_UNITS {
                return None;
            }
            units += c.len_utf16();
            end = end.saturating_add(c.len_utf8());
        }
    }

    /// The end of the run of `c` from byte `at`, at most `most` long.
    fn run_end(&self, at: usize, c: u8, most: usize) -> usize {
        let run = self
            .bytes_from(at)
            .iter()
            .take(most)
            .take_while(|&&b| b == c)
            .count();
        at.saturating_add(run)
    }

    /// The end of the path item at byte `at`, None where the path ends.
    fn item_end(&self, at: usize) -> Option<usize> {
        let (c, class) = self.read(at)?;
        if class == Class::Space {
            return None;
        }
        let after = at.saturating_add(c.len_utf8());
        match c {
            // A bracket right before a `(` opens a markdown link's label: the
            // path ends before it.
            '[' => self.pair_end(at, '[', ']', PAIR_DEPTH).filter(|&end| {
                let markdown = !self.faithful && self.char_at(end) == Some('(');
                if markdown {
                    self.cut.set(true);
                }
                !markdown
            }),
            '(' => self.pair_end(at, '(', ')', PAIR_DEPTH),
            '{' => self.pair_end(at, '{', '}', PAIR_DEPTH),
            '"' => self.quoted_end(at, '"'),
            '\'' => self.quoted_end(at, '\'').or_else(|| {
                self.read(after)
                    .is_some_and(|(next, next_class)| next_class == Class::Letter || next == '-')
                    .then_some(after)
            }),
            '.' => {
                let dots_end = self.run_end(at, b'.', usize::MAX);
                let dots = dots_end.saturating_sub(at);
                if dots >= 2 {
                    let colon = if self.char_at(dots_end) == Some(':') {
                        dots_end.saturating_add(1)
                    } else {
                        dots_end
                    };
                    let closes = self
                        .char_at(colon)
                        .is_some_and(|d| d.is_ascii_alphanumeric() || matches!(d, '%' | '/' | '&'));
                    (dots <= 20 && closes).then(|| colon.saturating_add(1))
                } else {
                    (!self.space_or_end(after)).then_some(after)
                }
            }
            '-' => Some(self.run_end(at, b'-', 20)),
            ',' | ';' => (!self.space_or_end(after)).then_some(after),
            '!' => {
                let end = self.run_end(at, b'!', usize::MAX);
                (end.saturating_sub(at) <= 20 && !self.space_or_end(end)).then_some(end)
            }
            '?' => (!self.space_or_end(after) && self.char_at(after) != Some('?')).then_some(after),
            '\\' | '/' | ':' | '%' | '@' | '#' | '&' | '=' | '_' | '~' | '*' => Some(after),
            _ => (class == Class::Letter).then_some(after),
        }
    }

    /// The end of the path (with its query and fragment) from byte `at`.
    fn path_end(&self, at: usize) -> usize {
        let Some(lead @ ('/' | '?' | '#')) = self.char_at(at) else {
            return at;
        };
        let mut end = at.saturating_add(1);
        let mut read_any = false;
        while let Some(next) = self.item_end(end) {
            end = next;
            read_any = true;
        }
        if read_any {
            end
        } else if lead == '/' {
            at.saturating_add(1)
        } else {
            at
        }
    }

    /// The end of the user name and `@` that open an authority at byte
    /// `at`, when one does: up to fifty ASCII letters, digits and the marks a
    /// user name or password takes unescaped, opening and closing on a letter
    /// or digit.
    fn user_end(&self, at: usize) -> Option<usize> {
        let name = self
            .bytes_from(at)
            .iter()
            .take(USER_UNITS.saturating_add(1))
            .take_while(|&&b| {
                b.is_ascii_alphanumeric()
                    || matches!(
                        b,
                        b'-' | b'.'
                            | b'_'
                            | b'~'
                            | b'%'
                            | b'!'
                            | b'$'
                            | b'&'
                            | b'+'
                            | b','
                            | b';'
                            | b'='
                            | b':'
                    )
            })
            .count();
        let user = self.text.get(at..at.saturating_add(name))?;
        let closes = (1..=USER_UNITS).contains(&name)
            && self.char_at(at.saturating_add(name)) == Some('@')
            && user.starts_with(|c: char| c.is_ascii_alphanumeric())
            && user.ends_with(|c: char| c.is_ascii_alphanumeric());
        closes.then(|| at.saturating_add(name).saturating_add(1))
    }

    /// The end of the link whose scheme, `scheme_len` bytes long, opens at
    /// byte `at`, as linkify-it validates one, and whether a user name opens
    /// its authority; None when none does.
    fn link_end(&self, at: usize, scheme: Scheme, scheme_len: usize) -> Option<(usize, bool)> {
        let body = at.saturating_add(scheme_len);
        let (host_end, user) = match scheme {
            Scheme::Relative => (self.relative_host_end(body)?, false),
            Scheme::Mail => return None,
            Scheme::Web | Scheme::Ftp => {
                let hosted = self
                    .text
                    .get(body..)?
                    .starts_with("//")
                    .then(|| body.saturating_add(2))?;
                match self
                    .user_end(hosted)
                    .filter(|_| scheme == Scheme::Web && !self.faithful)
                    .and_then(|user| self.host_end(user))
                {
                    Some(end) => (end, true),
                    None => (self.host_end(hosted)?, false),
                }
            }
        };
        Some((self.path_end(host_end), user))
    }
}

/// The scheme a link candidate opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    /// `http:` or `https:`, the links read.
    Web,
    /// `ftp:`, read as linkify-it reads it so it hides what it holds.
    Ftp,
    /// `//`, a link without a scheme, likewise.
    Relative,
    /// `mailto:`, which moves where linkify-it looks for the next link.
    Mail,
}

/// The scheme that opens `text` at byte `at` and its length.
fn scheme_at(text: &str, at: usize) -> Option<(Scheme, usize)> {
    let rest = text.get(at..)?;
    let starts = |prefix: &str| {
        rest.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    };
    if starts("http:") {
        Some((Scheme::Web, 5))
    } else if starts("https:") {
        Some((Scheme::Web, 6))
    } else if starts("ftp:") {
        Some((Scheme::Ftp, 4))
    } else if rest.starts_with("//") {
        Some((Scheme::Relative, 2))
    } else if starts("mailto:") {
        Some((Scheme::Mail, 7))
    } else {
        None
    }
}

/// The scheme a candidate at byte `anchor` of `bytes` (a `:` or a `/`)
/// closes or opens: its start, the scheme and its length, `:` or `//`
/// included.
fn scheme_before(bytes: &[u8], anchor: usize) -> Option<(usize, Scheme, usize)> {
    if bytes.get(anchor) == Some(&b'/') {
        return (bytes.get(anchor.saturating_add(1)) == Some(&b'/')).then_some((
            anchor,
            Scheme::Relative,
            2,
        ));
    }
    [
        (b"https".as_slice(), Scheme::Web),
        (b"http".as_slice(), Scheme::Web),
        (b"ftp".as_slice(), Scheme::Ftp),
        (b"mailto".as_slice(), Scheme::Mail),
    ]
    .into_iter()
    .find_map(|(name, scheme)| {
        let start = anchor.checked_sub(name.len())?;
        bytes
            .get(start..anchor)
            .filter(|head| head.eq_ignore_ascii_case(name))
            .map(|_| (start, scheme, name.len().saturating_add(1)))
    })
}

/// Whether linkify-it opens a link after `before`: at the start, or after a
/// space, punctuation other than `_`, a control character or a separator.
fn opens_after(before: Option<char>) -> bool {
    before.is_none_or(|c| c != '_' && class_of(c) != Class::Letter)
}

/// How a link was read: where it ends, whether a user name opens its
/// authority, and whether a cut linkify-it does not make was met.
#[derive(Debug, Clone, Copy)]
struct Read {
    /// The byte past the link.
    end: usize,
    /// Whether a user name opens its authority.
    user: bool,
    /// Whether a cut linkify-it does not make was met.
    cut: bool,
}

/// The link whose scheme opens `text` at byte `at`, read up to
/// `MAX_LENGTH` UTF-16 units past the scheme; read as linkify-it reads it
/// when `faithful`.
fn link_end(text: &str, at: usize, scheme: Scheme, len: usize, faithful: bool) -> Option<Read> {
    let read = |limit: usize| {
        let reader = Reader {
            text,
            limit,
            faithful,
            cut: Cell::new(false),
        };
        reader.link_end(at, scheme, len).map(|(end, user)| Read {
            end,
            user,
            cut: reader.cut.get(),
        })
    };
    let found = read(text.len())?;
    let body = at.saturating_add(len);
    // A UTF-16 unit takes at least a byte, so a link this short in bytes
    // never reaches the limit.
    if found.end.saturating_sub(body) <= MAX_LENGTH {
        return Some(found);
    }
    let mut units = 0;
    let limit = text
        .get(body..)?
        .char_indices()
        .find(|&(_, c)| {
            units += c.len_utf16();
            units > MAX_LENGTH
        })
        .map_or(text.len(), |(i, _)| body.saturating_add(i));
    read(limit)
}

/// Where, between bytes `from` and `to` of `text`, a link opens that
/// linkify-it would read on its own: a link read further than linkify-it
/// reads one (glued to a letter, after `_`, or with a user name) ends before
/// it rather than take it in.
fn inner_link(text: &str, from: usize, to: usize) -> Option<usize> {
    let mut at = from;
    while let Some(found) = text
        .as_bytes()
        .get(at..to)
        .and_then(|rest| rest.iter().position(|&b| matches!(b, b'h' | b'H')))
    {
        let start = at.saturating_add(found);
        at = start.saturating_add(1);
        if let Some((Scheme::Web, len)) = scheme_at(text, start)
            && opens_after(crate::text::char_before(text, start))
            && link_end(text, start, Scheme::Web, len, false).is_some()
        {
            return Some(start);
        }
    }
    None
}

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
///
/// A link is read as linkify-it reads one with a scheme: a host of labels, a
/// port, then a path that takes balanced brackets and quotes and ends at a
/// space, at punctuation that closes a sentence or at a bracket that closes
/// nothing. It is read wherever no ASCII letter or digit is glued before it,
/// so a link in code, in emphasis or after a script written without spaces
/// counts; a backtick, a chat link's `|label>` and a markdown link's
/// `[label](` end it.
///
/// # Examples
///
/// ```
/// let text = "See <https://acme.io/a|the doc> and **https://acme.io/b**.";
/// let links: Vec<&str> = figures_parser::urls(text).iter().map(|u| u.text).collect();
/// assert_eq!(links, ["https://acme.io/a", "https://acme.io/b"]);
/// ```
#[must_use]
pub fn urls(text: &str) -> Vec<Url<'_>> {
    let unwrapped = unwrap_slack(text);
    let scanned: &str = &unwrapped;
    let bytes = scanned.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    // The byte from which linkify-it looks for the character before a
    // scheme: past the last scheme it tried, or the last character of the
    // last link it read.
    let mut seen_from = 0;
    while let Some(found) = bytes
        .get(at..)
        .and_then(|rest| memchr::memchr2(b':', b'/', rest))
    {
        let anchor = at.saturating_add(found);
        at = anchor.saturating_add(1);
        let Some((start, scheme, len)) = scheme_before(bytes, anchor) else {
            continue;
        };
        let before = crate::text::char_before(scanned, start);
        let before_at = start.saturating_sub(before.map_or(0, char::len_utf8));
        let linkify_sees =
            opens_after(before) && (before_at >= seen_from || (start == 0 && seen_from == 0));
        let opens = match scheme {
            Scheme::Web => !before.is_some_and(|c| c.is_ascii_alphanumeric()),
            Scheme::Ftp => linkify_sees,
            Scheme::Relative => linkify_sees && !matches!(before, Some(':' | '/')),
            Scheme::Mail => false,
        };
        let web = scheme == Scheme::Web;
        let link = opens
            .then(|| link_end(scanned, start, scheme, len, !web))
            .flatten();
        if linkify_sees {
            // A reading that met none of this crate's cuts ends where
            // linkify-it's does.
            let read = match link {
                Some(read) if !read.cut && !read.user => Some(read),
                _ if web => link_end(scanned, start, scheme, len, true),
                _ => link,
            };
            seen_from = read.map_or_else(
                || start.saturating_add(len),
                |read| {
                    read.end.saturating_sub(
                        crate::text::char_before(scanned, read.end).map_or(0, char::len_utf8),
                    )
                },
            );
        }
        let Some(Read { mut end, user, .. }) = link else {
            continue;
        };
        if !web {
            at = end;
            continue;
        }
        if user || !linkify_sees {
            end = inner_link(scanned, start.saturating_add(len), end).unwrap_or(end);
        }
        at = end;
        let kept = scanned
            .get(start..end)
            .unwrap_or_default()
            .trim_end_matches(is_trailing_markup);
        let span = start..start.saturating_add(kept.len());
        if let Some(text) = text.get(span.clone()) {
            out.push(Url { text, span });
        }
    }
    out
}
