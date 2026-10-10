//! Character lookups, classes and comparisons the kinds share. A lookup at a
//! byte offset is answered by byte while the text is ASCII.

use regex::{Regex, RegexBuilder};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_script::{Script, UnicodeScript};

/// The character starting at byte `at`.
pub fn char_at(text: &str, at: usize) -> Option<char> {
    match text.as_bytes().get(at) {
        Some(&b) if b.is_ascii() => Some(char::from(b)),
        _ => text.get(at..)?.chars().next(),
    }
}

/// The character ending at byte `at`.
pub fn char_before(text: &str, at: usize) -> Option<char> {
    match at.checked_sub(1).and_then(|i| text.as_bytes().get(i)) {
        Some(&b) if b.is_ascii() => Some(char::from(b)),
        _ => text.get(..at)?.chars().next_back(),
    }
}

/// The words as a regex alternation, longest first so a longer word wins.
pub fn alternation(words: impl Iterator<Item = &'static str>) -> String {
    let mut words: Vec<&str> = words.collect();
    words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    words
        .iter()
        .map(|w| regex::escape(w))
        .collect::<Vec<_>>()
        .join("|")
}

/// Whether `haystack` holds `needle`, ASCII case aside. Candidates are
/// found by `memchr` on the needle's first byte that is not a letter (`@`, a
/// digit, `-`), or on both cases of its first byte, then compared whole.
pub fn contains_ignoring_case(haystack: &str, needle: &str) -> bool {
    let (hay, needle) = (haystack.as_bytes(), needle.as_bytes());
    let Some(&first) = needle.first() else {
        return true;
    };
    let (offset, anchor) = needle
        .iter()
        .enumerate()
        .find(|(_, b)| !b.is_ascii_alphabetic())
        .map_or((0, first), |(i, &b)| (i, b));
    memchr::memchr2_iter(
        anchor.to_ascii_lowercase(),
        anchor.to_ascii_uppercase(),
        hay,
    )
    .any(|at| {
        at.checked_sub(offset)
            .and_then(|start| hay.get(start..))
            .and_then(|rest| rest.get(..needle.len()))
            .is_some_and(|window| window.eq_ignore_ascii_case(needle))
    })
}

/// Whether `corpus` or `prompt` carries `item`, ASCII case aside.
pub fn carried(item: &str, corpus: &str, prompt: &str) -> bool {
    contains_ignoring_case(corpus, item) || contains_ignoring_case(prompt, item)
}

/// Adds `item` to a comparison's findings unless they already hold it, ASCII
/// case aside, or `carried` says a source gives it; `carried` runs only for a
/// new item.
pub fn report_once(out: &mut Vec<String>, item: &str, carried: impl FnOnce() -> bool) {
    if !out.iter().any(|seen| seen.eq_ignore_ascii_case(item)) && !carried() {
        out.push(item.to_owned());
    }
}

/// Whether `text` starts with `prefix`, a lowercase word, once lowercased;
/// what `text.to_lowercase().starts_with(prefix)` says, without the copy.
pub fn starts_with_lowercase(text: &str, prefix: &str) -> bool {
    let mut lower = text.chars().flat_map(char::to_lowercase);
    prefix.chars().all(|p| lower.next() == Some(p))
}

/// Whether two words are the same, case aside.
pub fn same_word(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// A word table's value for `word`, case aside.
pub fn lookup<T: Copy>(table: &[(&str, T)], word: &str) -> Option<T> {
    table
        .iter()
        .find(|&&(w, _)| same_word(w, word))
        .map(|&(_, v)| v)
}

/// A regex, None if it does not compile. The size limit is raised for the
/// word alternations, which outgrow the default once case-folded.
pub fn compile(pattern: &str) -> Option<Regex> {
    RegexBuilder::new(pattern).size_limit(1 << 24).build().ok()
}

/// The start of the run of bytes `keep` accepts that ends at byte `end`.
pub fn run_start(bytes: &[u8], end: usize, keep: impl Fn(u8) -> bool) -> usize {
    let mut start = end;
    while let Some(i) = start.checked_sub(1) {
        if !bytes.get(i).copied().is_some_and(&keep) {
            break;
        }
        start = i;
    }
    start
}

/// The end of the run of bytes `keep` accepts that starts at byte `start`.
pub fn run_end(bytes: &[u8], start: usize, keep: impl Fn(u8) -> bool) -> usize {
    let mut end = start;
    while bytes.get(end).copied().is_some_and(&keep) {
        end = end.saturating_add(1);
    }
    end
}

/// Whether `text` holds a letter right before byte `start` or right at
/// byte `end`, so what lies between is part of a word.
pub fn inside_word(text: &str, start: usize, end: usize) -> bool {
    char_before(text, start).is_some_and(is_letter) || char_at(text, end).is_some_and(is_letter)
}

/// Whether `bytes` is an ISO 8601 date, `YYYY-MM-DD`: the head of a
/// timestamp, which is neither an identifier nor a length.
pub fn is_iso_date(bytes: &[u8]) -> bool {
    let shape = b"dddd-dd-dd";
    bytes.len() == shape.len()
        && bytes.iter().zip(shape).all(|(&b, &s)| {
            if s == b'd' {
                b.is_ascii_digit()
            } else {
                b == s
            }
        })
}

/// Whether `host` is a reserved example domain (RFC 2606), bare or under
/// `www.`, case aside: what a prompt illustrates links and addresses with.
pub fn is_example_domain(host: &str) -> bool {
    let bytes = host.as_bytes();
    let bare = bytes
        .get(..4)
        .filter(|www| www.eq_ignore_ascii_case(b"www."))
        .and_then(|_| bytes.get(4..))
        .unwrap_or(bytes);
    [b"example.com", b"example.org", b"example.net"]
        .iter()
        .any(|domain| bare.eq_ignore_ascii_case(*domain))
}

/// The length of the tracker key `text` starts with: a capital, capitals or
/// digits, a dash and digits (`CORE-1710`).
pub fn key_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_uppercase) {
        return None;
    }
    let team = run_end(bytes, 1, |b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if team == 1 || bytes.get(team) != Some(&b'-') {
        return None;
    }
    let number = team.saturating_add(1);
    let end = run_end(bytes, number, |b| b.is_ascii_digit());
    (end > number).then_some(end)
}

/// Whether `b` is an ASCII word byte, as JavaScript's `\w` reads one.
pub const fn is_ascii_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Whether `c` is an ASCII word character, as JavaScript's `\w` reads one.
pub const fn is_ascii_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Scripts that write words without spaces, so a figure sits against a
/// letter ("合計は1,250,000円") without being part of an id.
const UNSPACED_SCRIPTS: [Script; 9] = [
    Script::Han,
    Script::Hiragana,
    Script::Katakana,
    Script::Hangul,
    Script::Thai,
    Script::Lao,
    Script::Khmer,
    Script::Myanmar,
    Script::Tibetan,
];

/// Whether `c` belongs to a script written without spaces. Script extensions
/// are checked so a mark shared between scripts, like the prolonged-sound
/// mark `ー`, counts for the scripts it serves; a Common or Inherited set
/// answers true for every script, so those characters ("ℝ", "𝐀") never do.
pub fn is_unspaced_script(c: char) -> bool {
    let extensions = c.script_extension();
    if extensions.is_common() || extensions.is_inherited() {
        return false;
    }
    UNSPACED_SCRIPTS
        .into_iter()
        .any(|script| extensions.contains_script(script))
}

/// Whether `c` is a letter, any of Unicode's `L` categories.
pub fn is_letter(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    )
}
