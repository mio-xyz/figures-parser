//! UTF-16 offsets for a JavaScript caller, from the byte offsets the library speaks.

/// Converts byte offsets of one text to UTF-16 offsets, walking the text once
/// when asked in ascending order.
#[derive(Debug)]
pub struct Utf16Cursor<'a> {
    /// The text the offsets index.
    text: &'a str,
    /// The byte offset the cursor stands at.
    byte: usize,
    /// The UTF-16 offset of `byte`.
    unit: u32,
}

impl<'a> Utf16Cursor<'a> {
    /// A cursor at the start of `text`.
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self {
            text,
            byte: 0,
            unit: 0,
        }
    }

    /// The UTF-16 offset of the character boundary at `byte`. Going backwards
    /// restarts from the beginning, so ascending calls are the cheap ones.
    pub fn at(&mut self, byte: usize) -> u32 {
        if byte < self.byte {
            self.byte = 0;
            self.unit = 0;
        }
        let units = self
            .text
            .get(self.byte..byte)
            .map_or(0, |s| s.encode_utf16().count());
        self.byte = byte;
        self.unit = self
            .unit
            .saturating_add(u32::try_from(units).unwrap_or(u32::MAX));
        self.unit
    }
}

/// The length of `text` in UTF-16 units.
#[must_use]
pub fn units(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}
