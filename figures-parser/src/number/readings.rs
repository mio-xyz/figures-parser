//! What a numeric token may mean: thousands, a decimal, or neighbours printed
//! side by side.

use super::digits::{ascii_in, push_ascii};
use super::scan::{Token, is_decimal_symbol, is_symbol, look_back};
use super::{Reading, Sign};

/// The group lengths seen so far, and where they stop being thousands grouping.
#[derive(Default)]
struct Run {
    /// How many groups have ended.
    groups: usize,
    /// The first group's length in code points.
    lead: usize,
    /// The last ended group's length in code points.
    prev: usize,
    /// A middle group so far had other than three digits.
    broke_threes: bool,
    /// A middle group so far had other than two digits.
    broke_twos: bool,
}

impl Run {
    /// Ends a group of `len` digits.
    const fn push(&mut self, len: usize) {
        if self.groups == 0 {
            self.lead = len;
        } else if self.groups >= 2 {
            self.broke_threes |= self.prev != 3;
            self.broke_twos |= self.prev != 2;
        }
        self.prev = len;
        self.groups += 1;
    }

    /// Whether the groups ended so far are thousands grouping: a lead of 1–3
    /// digits, then three-digit groups; or the Indian lakh notation,
    /// two-digit groups ending in one of three ("12,50,000").
    const fn thousands(&self) -> bool {
        self.groups >= 2
            && self.lead <= 3
            && self.prev == 3
            && (!self.broke_threes || (self.lead <= 2 && !self.broke_twos))
    }
}

/// What one pass over a token's text tells about its groups and symbols.
#[derive(Default)]
struct Shape {
    /// The first symbol.
    first: Option<char>,
    /// The last symbol.
    last: Option<char>,
    /// The byte offset of the last symbol.
    last_at: usize,
    /// A symbol before the last differs from the first.
    mixed_lead: bool,
    /// The groups.
    run: Run,
    /// Whether the groups before the last one are thousands grouping.
    head_thousands: bool,
}

impl Shape {
    /// Reads `text` in one pass.
    fn of(text: &str) -> Self {
        let mut shape = Self::default();
        let mut len = 0;
        for (at, c) in text.char_indices() {
            if is_symbol(c) {
                shape.run.push(len);
                len = 0;
                match shape.first {
                    None => shape.first = Some(c),
                    Some(first) => shape.mixed_lead |= shape.last != Some(first),
                }
                shape.last = Some(c);
                shape.last_at = at;
            } else {
                len += 1;
            }
        }
        shape.head_thousands = shape.run.thousands();
        shape.run.push(len);
        shape
    }
}

/// Writes the digit groups of `text` into `scratch`, joined and in ASCII.
fn join_groups(text: &str, scratch: &mut String) {
    scratch.clear();
    for group in text.split(is_symbol) {
        push_ascii(group, scratch);
    }
}

impl<'a> Token<'a> {
    /// A reading of the whole token.
    const fn whole<'s>(
        &self,
        integer: &'s str,
        fraction: Option<&'s str>,
        grouped: bool,
    ) -> Reading<'a, 's> {
        Reading {
            text: self.text,
            span: self.start..self.start + self.text.len(),
            integer,
            fraction,
            grouped,
            currency: self.currency(),
            derived: false,
            sign: self.sign,
            approximate: self.approximate,
            full_text: self.full,
        }
    }

    /// The sign `reading`, one of this token's, would carry had the token
    /// been read with its sign: the token's for a reading that starts it,
    /// none for a later group. `text` is the text the token was found in.
    pub(super) fn sign_of(&self, reading: &Reading<'_, '_>, text: &str) -> Sign {
        if reading.span.start == self.start {
            look_back(text, self.start, self.start + self.text.len()).sign
        } else {
            Sign::Unsigned
        }
    }

    /// Calls `f` with each reading of this token. `scratch` holds the digits
    /// a reading joins or converts, so a token allocates nothing of its own.
    ///
    /// Groups joined by one repeated symbol that fit thousands grouping are
    /// one integer; a last symbol that may end a decimal, after a single group
    /// or a whole grouped by another symbol ("1.234.567,89", "8,22"), ends a
    /// decimal part; groups that fit neither are neighbours printed side by
    /// side (`[15977,10140]`). A thousands run also yields its neighbours as
    /// derived readings, since a printed table may mean either. A single dot
    /// or comma before three digits fits both ("1.250" is 1,250 in Berlin and
    /// 1.25 in Boston) and yields both.
    pub fn for_each_reading(&self, scratch: &mut String, mut f: impl FnMut(Reading<'a, '_>)) {
        if self.text.bytes().all(|b| b.is_ascii_digit()) {
            f(self.whole(self.text, None, false));
            return;
        }
        let shape = Shape::of(self.text);
        let (Some(first), Some(last)) = (shape.first, shape.last) else {
            f(self.whole(ascii_in(self.text, scratch), None, false));
            return;
        };
        let groups = shape.run.groups;
        let thousands = !shape.mixed_lead && last == first && shape.run.thousands();
        let decimal = is_decimal_symbol(last)
            && (groups == 2 || (first != last && !shape.mixed_lead && shape.head_thousands));

        if thousands {
            join_groups(self.text, scratch);
            f(self.whole(scratch.as_str(), None, true));
        }
        if decimal {
            let head = self.text.get(..shape.last_at).unwrap_or_default();
            let tail = self
                .text
                .get(shape.last_at + last.len_utf8()..)
                .unwrap_or_default();
            if groups == 2 && self.text.is_ascii() {
                f(self.whole(head, Some(tail), false));
            } else {
                join_groups(head, scratch);
                let integer_len = scratch.len();
                push_ascii(tail, scratch);
                let (integer, fraction) = scratch
                    .split_at_checked(integer_len)
                    .unwrap_or((scratch.as_str(), ""));
                f(self.whole(integer, Some(fraction), groups > 2));
            }
        }
        if thousands || !decimal {
            let last_group = shape.run.groups - 1;
            let mut at = self.start;
            let prefix_len = self.full.len().saturating_sub(self.text.len());
            for (i, piece) in self.text.split_inclusive(is_symbol).enumerate() {
                let group = piece.strip_suffix(is_symbol).unwrap_or(piece);
                let leading = i == 0;
                f(Reading {
                    text: group,
                    span: at..at + group.len(),
                    integer: ascii_in(group, scratch),
                    fraction: None,
                    grouped: false,
                    currency: (leading && self.marked_before)
                        || (i == last_group && self.marked_after),
                    derived: thousands,
                    sign: if leading { self.sign } else { Sign::Unsigned },
                    approximate: leading && self.approximate,
                    full_text: if leading {
                        self.full.get(..prefix_len + group.len()).unwrap_or(group)
                    } else {
                        group
                    },
                });
                at += piece.len();
            }
        }
    }
}
