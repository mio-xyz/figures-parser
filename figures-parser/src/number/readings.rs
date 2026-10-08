//! What a numeric token may mean: thousands, a decimal, or neighbours printed
//! side by side.

use std::borrow::Cow;

use super::Number;
use super::digits::ascii;
use super::scan::{Token, is_decimal_symbol};

/// Whether `groups` is thousands grouping: a lead of 1–3 digits, then
/// three-digit groups; or the Indian lakh notation, two-digit groups ending
/// in one of three ("12,50,000").
fn is_thousands(groups: &[&str]) -> bool {
    let Some((lead, rest)) = groups.split_first() else {
        return false;
    };
    let Some((last, middle)) = rest.split_last() else {
        return false;
    };
    let lead_len = lead.chars().count();
    if lead_len > 3 || last.chars().count() != 3 {
        return false;
    }
    middle.iter().all(|g| g.chars().count() == 3)
        || (lead_len <= 2 && middle.iter().all(|g| g.chars().count() == 2))
}

/// The groups joined, in ASCII digits.
fn joined<'a>(groups: &[&'a str]) -> Cow<'a, str> {
    match groups {
        [one] => ascii(one),
        _ => Cow::Owned(ascii(&groups.concat()).into_owned()),
    }
}

impl<'a> Token<'a> {
    /// The readings of this token.
    ///
    /// Groups joined by one repeated symbol that fit thousands grouping are
    /// one integer; a last symbol that may end a decimal, after a single group
    /// or a whole grouped by another symbol ("1.234.567,89", "8,22"), ends a
    /// decimal part; groups that fit neither are neighbours printed side by
    /// side ("[15977,10140]"). A thousands run also yields its neighbours as
    /// derived readings, since a printed table may mean either. A single dot
    /// or comma before three digits fits both ("1.250" is 1,250 in Berlin and
    /// 1.25 in Boston) and yields both.
    #[must_use]
    pub fn readings(&self) -> Vec<Number<'a>> {
        let whole = |integer: Cow<'a, str>, fraction: Option<Cow<'a, str>>, grouped: bool| Number {
            text: self.text,
            span: self.start..self.start + self.text.len(),
            integer,
            fraction,
            grouped,
            currency: self.currency,
            derived: false,
        };
        let (Some((&last, leading)), Some((last_group, head))) =
            (self.symbols.split_last(), self.groups.split_last())
        else {
            return vec![whole(ascii(self.text), None, false)];
        };
        let thousands = leading.iter().all(|&s| s == last) && is_thousands(&self.groups);
        let decimal = is_decimal_symbol(last)
            && (head.len() == 1
                || leading.split_first().is_some_and(|(&first, _)| {
                    first != last && leading.iter().all(|&s| s == first) && is_thousands(head)
                }));

        let mut out = Vec::new();
        if thousands {
            out.push(whole(joined(&self.groups), None, true));
        }
        if decimal {
            out.push(whole(joined(head), Some(ascii(last_group)), head.len() > 1));
        }
        if thousands || !decimal {
            let mut at = self.start;
            for (i, &group) in self.groups.iter().enumerate() {
                out.push(Number {
                    text: group,
                    span: at..at + group.len(),
                    integer: ascii(group),
                    fraction: None,
                    grouped: false,
                    currency: self.currency && i == 0,
                    derived: thousands,
                });
                at += group.len() + self.symbols.get(i).map_or(0, |s| s.len_utf8());
            }
        }
        out
    }
}
