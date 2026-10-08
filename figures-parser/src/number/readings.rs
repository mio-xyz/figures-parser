//! What a numeric token may mean: thousands, a decimal, or neighbours printed
//! side by side.

use super::Reading;
use super::digits::{ascii_in, push_ascii};
use super::scan::{Token, is_decimal_symbol, is_symbol};

/// The group lengths seen so far, and whether they are thousands grouping.
#[derive(Default)]
struct Run {
    /// How many groups have ended.
    groups: usize,
    /// The first group's length in code points.
    lead: usize,
    /// The last ended group's length in code points.
    prev: usize,
    /// Whether every middle group so far has three digits.
    threes: bool,
    /// Whether every middle group so far has two digits.
    twos: bool,
}

impl Run {
    /// Ends a group of `len` digits.
    const fn push(&mut self, len: usize) {
        if self.groups == 0 {
            self.lead = len;
        } else if self.groups >= 2 {
            self.threes &= self.prev == 3;
            self.twos &= self.prev == 2;
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
            && (self.threes || (self.lead <= 2 && self.twos))
    }
}

/// What one pass over a token's text tells about its groups and symbols.
#[derive(Default)]
struct Shape {
    /// How many symbols join the groups.
    symbols: usize,
    /// The first symbol.
    first: Option<char>,
    /// The last symbol.
    last: Option<char>,
    /// Whether every symbol but the last is the first one.
    leading_first: bool,
    /// The groups.
    run: Run,
    /// Whether the groups before the last one are thousands grouping.
    head_thousands: bool,
}

impl Shape {
    /// Reads `text` in one pass.
    fn of(text: &str) -> Self {
        let mut shape = Self {
            leading_first: true,
            run: Run {
                threes: true,
                twos: true,
                ..Run::default()
            },
            ..Self::default()
        };
        let mut len = 0;
        for c in text.chars() {
            if is_symbol(c) {
                shape.group(len);
                len = 0;
                shape.symbols += 1;
                match shape.first {
                    None => shape.first = Some(c),
                    Some(first) => shape.leading_first &= shape.last == Some(first),
                }
                shape.last = Some(c);
            } else {
                len += 1;
            }
        }
        shape.group(len);
        shape
    }

    /// Ends a group of `len` digits, remembering first whether the groups
    /// before it were thousands grouping.
    const fn group(&mut self, len: usize) {
        self.head_thousands = self.run.thousands();
        self.run.push(len);
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
            currency: self.currency,
            derived: false,
        }
    }

    /// Calls `f` with each reading of this token. `scratch` holds the digits
    /// a reading joins or converts, so a token allocates nothing of its own.
    ///
    /// Groups joined by one repeated symbol that fit thousands grouping are
    /// one integer; a last symbol that may end a decimal, after a single group
    /// or a whole grouped by another symbol ("1.234.567,89", "8,22"), ends a
    /// decimal part; groups that fit neither are neighbours printed side by
    /// side ("[15977,10140]"). A thousands run also yields its neighbours as
    /// derived readings, since a printed table may mean either. A single dot
    /// or comma before three digits fits both ("1.250" is 1,250 in Berlin and
    /// 1.25 in Boston) and yields both.
    pub fn for_each_reading(&self, scratch: &mut String, mut f: impl FnMut(Reading<'a, '_>)) {
        let shape = Shape::of(self.text);
        let (Some(first), Some(last)) = (shape.first, shape.last) else {
            f(self.whole(ascii_in(self.text, scratch), None, false));
            return;
        };
        let thousands = shape.leading_first && last == first && shape.run.thousands();
        let decimal = is_decimal_symbol(last)
            && (shape.symbols == 1
                || (first != last && shape.leading_first && shape.head_thousands));

        if thousands {
            scratch.clear();
            for group in self.text.split(is_symbol) {
                push_ascii(group, scratch);
            }
            f(self.whole(scratch.as_str(), None, true));
        }
        if decimal {
            scratch.clear();
            let (head, fraction_group) = self.text.rsplit_once(last).unwrap_or((self.text, ""));
            for group in head.split(is_symbol) {
                push_ascii(group, scratch);
            }
            let integer_len = scratch.len();
            push_ascii(fraction_group, scratch);
            let (integer, fraction) = scratch
                .split_at_checked(integer_len)
                .unwrap_or((scratch.as_str(), ""));
            f(self.whole(integer, Some(fraction), shape.symbols > 1));
        }
        if thousands || !decimal {
            let mut at = self.start;
            for (i, piece) in self.text.split_inclusive(is_symbol).enumerate() {
                let group = piece.strip_suffix(is_symbol).unwrap_or(piece);
                f(Reading {
                    text: group,
                    span: at..at + group.len(),
                    integer: ascii_in(group, scratch),
                    fraction: None,
                    grouped: false,
                    currency: self.currency && i == 0,
                    derived: thousands,
                });
                at += piece.len();
            }
        }
    }
}
