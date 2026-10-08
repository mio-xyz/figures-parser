//! What a numeric token may mean: thousands, a decimal, or neighbours printed
//! side by side.

use std::borrow::Cow;

use super::digits::{ascii_in, push_ascii};
use super::scan::{Token, is_decimal_symbol, is_symbol};
use super::{Number, Reading};

/// The length of a digit group in code points.
fn len_chars(group: &str) -> usize {
    if group.is_ascii() {
        group.len()
    } else {
        group.chars().count()
    }
}

/// Whether group lengths are thousands grouping: a lead of 1–3 digits, then
/// three-digit groups; or the Indian lakh notation, two-digit groups ending
/// in one of three ("12,50,000").
fn is_thousands(mut lengths: impl Iterator<Item = usize>) -> bool {
    let Some(lead) = lengths.next() else {
        return false;
    };
    let Some(mut last) = lengths.next() else {
        return false;
    };
    let (mut threes, mut twos) = (true, true);
    for next in lengths {
        threes &= last == 3;
        twos &= last == 2;
        last = next;
    }
    lead <= 3 && last == 3 && (threes || (lead <= 2 && twos))
}

impl<'a> Token<'a> {
    /// The digit groups, as written.
    fn groups(&self) -> impl DoubleEndedIterator<Item = &'a str> + Clone {
        self.text.split(is_symbol)
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
    pub fn for_each_reading(&self, scratch: &mut String, mut f: impl FnMut(Reading<'_>)) {
        let span = self.start..self.start + self.text.len();
        let mut symbols = self.text.chars().filter(|&c| is_symbol(c));
        let Some(first) = symbols.next() else {
            f(Reading {
                text: self.text,
                span,
                integer: ascii_in(self.text, scratch),
                fraction: None,
                grouped: false,
                currency: self.currency,
                derived: false,
            });
            return;
        };
        let mut last = first;
        let mut count = 1;
        // Whether every symbol but the last is the first one.
        let mut leading_first = true;
        for s in symbols {
            leading_first &= last == first;
            last = s;
            count += 1;
        }
        let thousands =
            leading_first && last == first && is_thousands(self.groups().map(len_chars));
        let decimal = is_decimal_symbol(last)
            && (count == 1
                || (first != last
                    && leading_first
                    && is_thousands(self.groups().take(count).map(len_chars))));

        if thousands {
            scratch.clear();
            for group in self.groups() {
                push_ascii(group, scratch);
            }
            f(Reading {
                text: self.text,
                span: span.clone(),
                integer: scratch.as_str(),
                fraction: None,
                grouped: true,
                currency: self.currency,
                derived: false,
            });
        }
        if decimal {
            scratch.clear();
            let mut groups = self.groups();
            let fraction_group = groups.next_back().unwrap_or_default();
            for group in groups {
                push_ascii(group, scratch);
            }
            let integer_len = scratch.len();
            push_ascii(fraction_group, scratch);
            let (integer, fraction) = scratch
                .split_at_checked(integer_len)
                .unwrap_or((scratch.as_str(), ""));
            f(Reading {
                text: self.text,
                span,
                integer,
                fraction: Some(fraction),
                grouped: count > 1,
                currency: self.currency,
                derived: false,
            });
        }
        if thousands || !decimal {
            let mut at = 0;
            for (i, group) in self.groups().enumerate() {
                let symbol_len = self
                    .text
                    .get(at + group.len()..)
                    .and_then(|rest| rest.chars().next())
                    .map_or(0, char::len_utf8);
                f(Reading {
                    text: group,
                    span: self.start + at..self.start + at + group.len(),
                    integer: ascii_in(group, scratch),
                    fraction: None,
                    grouped: false,
                    currency: self.currency && i == 0,
                    derived: thousands,
                });
                at += group.len() + symbol_len;
            }
        }
    }

    /// The readings of this token, each owning what it could not borrow from
    /// the text. See [`Token::for_each_reading`] for what the readings are.
    #[must_use]
    pub fn readings(&self) -> Vec<Number<'a>> {
        let mut out = Vec::new();
        self.readings_into(&mut String::new(), &mut out);
        out
    }

    /// Appends the readings of this token to `out`; see [`Token::readings`].
    pub fn readings_into(&self, scratch: &mut String, out: &mut Vec<Number<'a>>) {
        self.for_each_reading(scratch, |r| {
            let text = self
                .text
                .get(r.span.start - self.start..r.span.end - self.start)
                .unwrap_or_default();
            let integer = if r.integer == text {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(r.integer.to_owned())
            };
            let fraction = r.fraction.map(|f| match text.get(text.len() - f.len()..) {
                Some(tail) if tail == f => Cow::Borrowed(tail),
                _ => Cow::Owned(f.to_owned()),
            });
            out.push(Number {
                text,
                span: r.span,
                integer,
                fraction,
                grouped: r.grouped,
                currency: r.currency,
                derived: r.derived,
            });
        });
    }
}
