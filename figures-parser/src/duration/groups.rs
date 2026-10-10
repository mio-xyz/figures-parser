//! A regex's matches read through one set of capture locations, reused from
//! match to match so a scan allocates once rather than once per attempt.

use std::ops::Range;

use regex::{CaptureLocations, Regex};

/// One group of a match.
#[derive(Debug, Clone, Copy)]
pub struct Group<'t> {
    /// Byte offset of its first byte.
    start: usize,
    /// Byte offset one past its last byte.
    end: usize,
    /// Its text.
    text: &'t str,
}

impl<'t> Group<'t> {
    /// Its text.
    pub const fn as_str(&self) -> &'t str {
        self.text
    }

    /// Byte offset of its first byte.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Byte offset one past its last byte.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Its byte range.
    pub const fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// The groups of the current match.
pub struct Groups<'a, 't> {
    /// The pattern, for its group names.
    re: &'a Regex,
    /// Where each group matched.
    locations: &'a CaptureLocations,
    /// The text scanned.
    text: &'t str,
}

impl<'t> Groups<'_, 't> {
    /// The group at `index`.
    fn at(&self, index: usize) -> Option<Group<'t>> {
        let (start, end) = self.locations.get(index)?;
        Some(Group {
            start,
            end,
            text: self.text.get(start..end)?,
        })
    }

    /// The whole match.
    pub fn whole(&self) -> Option<Group<'t>> {
        self.at(0)
    }

    /// The group named `name`, when it took part in the match.
    pub fn get(&self, name: &str) -> Option<Group<'t>> {
        let index = self.re.capture_names().position(|n| n == Some(name))?;
        self.at(index)
    }
}

/// Calls `visit` on each match of `re` in `text`, searching again from the
/// offset it returns (at least one byte on).
pub fn scan<'t>(re: &Regex, text: &'t str, mut visit: impl FnMut(&Groups<'_, 't>) -> usize) {
    let mut locations = re.capture_locations();
    let mut at = 0;
    while at <= text.len() && re.captures_read_at(&mut locations, text, at).is_some() {
        let groups = Groups {
            re,
            locations: &locations,
            text,
        };
        at = visit(&groups).max(at.saturating_add(1));
    }
}
