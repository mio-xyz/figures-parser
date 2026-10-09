//! Character lookups at a byte offset, answered by byte while the text is ASCII.

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

/// Whether `c` is an ASCII word character, as JavaScript's `\w` reads one.
pub const fn is_ascii_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
