//! A per-unit amount stated beside a total and a count that do not give it:
//! "$74.74 spend, 2 leads, $45.83 CPL", where a printed line shows $45.83 is
//! another amount over the same count.

use super::ground::{link_spans, skipped};
use super::ratio::ratios;
use super::scan::{Scratch, Token, currency_start, tokens};

/// A per-unit amount a clause pairs with a total and a count it is not the
/// quotient of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MispairedFigure {
    /// The per-unit amount as written, its currency mark first ("$45.83").
    pub per_unit: String,
    /// The clause's total over its count, worked out ("$74.74 / 2 = $37.37").
    pub stated: String,
    /// The printed amount the per-unit one is that count's quotient of
    /// ("$91.65 / 2").
    pub printed: String,
}

/// A per-unit amount with fewer significant digits ("$20", "$4,900") is
/// round, and some printed amount over some count lands on it by chance.
const MIN_SIGNIFICANT_DIGITS: usize = 3;

/// What absorbs the error of a value parsed from decimal digits.
const EPSILON: f64 = 1e-9;

/// A figure as a reply states it.
#[derive(Debug, Clone, Copy)]
struct Stated<'r> {
    /// As written, without its currency mark ("45.83").
    text: &'r str,
    /// The currency mark written before it ("$"), or "" for none.
    mark: &'r str,
    /// Its value.
    value: f64,
    /// The decimal places written.
    places: usize,
    /// Its significant digits as written ("0.05" has one, "4,900" two).
    significant: usize,
}

impl Stated<'_> {
    /// How far an exact value may lie from it as written: half its last place.
    fn rounding(&self) -> f64 {
        0.5_f64.mul_add(place(self.places), EPSILON)
    }

    /// How far a total may lie from it as written: a whole last place, since a
    /// total is cut as often as rounded ("$448" for 448.59).
    fn cut(&self) -> f64 {
        place(self.places) + EPSILON
    }

    /// Whether it is `total / count`, `total` known to within `slack`.
    fn quotient_of(&self, total: f64, slack: f64, count: f64) -> bool {
        (total / count - self.value).abs() <= self.rounding() + slack / count
    }
}

/// The value of the last of `places` decimal places.
fn place(places: usize) -> f64 {
    i32::try_from(places).map_or(0.0, |p| 10_f64.powi(-p))
}

/// The amounts, counts and candidate per-unit amounts of one clause.
struct Clause<'r> {
    /// The figures behind a currency mark, above zero.
    amounts: Vec<Stated<'r>>,
    /// The whole figures of two or more with no currency mark.
    counts: Vec<Stated<'r>>,
    /// The amounts that may be per-unit ones, by index into `amounts`: three
    /// significant digits or more, a larger amount beside them, and no
    /// amount of the clause over one of its counts giving them.
    per_units: Vec<usize>,
}

/// A printed line that may show an amount as a quotient: the first printed
/// amount found for each question asked of the corpus.
struct Query {
    /// The clause asked about.
    clause: usize,
    /// The amount asked to be a quotient, by index into the clause's.
    amount: usize,
    /// The count it would be the quotient over, by index into the clause's.
    count: usize,
    /// The total whose own value the printed amount must not be, by index
    /// into the clause's amounts; None for any.
    total: Option<usize>,
    /// The printed amount the first line showing the quotient gives.
    found: Option<f64>,
    /// What the line being read has shown so far.
    line: LineState,
}

/// What one printed line has shown a query so far.
#[derive(Debug, Clone, Copy, Default)]
struct LineState {
    /// The count.
    count: bool,
    /// A value the amount rounds.
    rounded: bool,
    /// The first larger amount the count divides into the amount.
    source: Option<f64>,
}

/// Per-unit amounts in `reply` stated beside a total and a count they are
/// not the quotient of, when a line of `corpus` shows them as another
/// amount over that count.
///
/// A clause (a sentence or a line) holding two currency amounts and a whole
/// count reads as a total, a count and a per-unit amount once one printed
/// line carries the smaller amount, the count and a larger amount that the
/// count divides into it: the print, not a word beside the figure, shows it
/// is a quotient. One the clause's own total and count give is left alone,
/// and so is a clause's amount the print shows is a quotient itself, which
/// is a second per-unit amount, not a total. A per-unit amount of fewer than
/// three significant digits is round, and is never held. The corpus is read
/// only once a clause holds a candidate.
///
/// # Examples
///
/// ```
/// use figures_parser::mispaired_per_unit;
///
/// let printed = "LT: $91.65 / 2 leads / CPL $45.83 | Y: $74.74 / 2 leads";
/// let found = mispaired_per_unit("$74.74 spend, 2 leads, $45.83 CPL.", printed);
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].per_unit, "$45.83");
/// assert_eq!(found[0].stated, "$74.74 / 2 = $37.37");
/// assert_eq!(found[0].printed, "$91.65 / 2");
///
/// assert!(mispaired_per_unit("$74.74 spend, 2 leads, $37.37 CPL.", printed).is_empty());
/// ```
#[must_use]
pub fn mispaired_per_unit(reply: &str, corpus: &str) -> Vec<MispairedFigure> {
    let clauses = read_clauses(reply);
    if clauses.is_empty() {
        return Vec::new();
    }
    let mut queries = Vec::new();
    for (index, clause) in clauses.iter().enumerate() {
        let ask = |amount: usize, total: Option<usize>, asked: &mut Vec<Query>| {
            for count in 0..clause.counts.len() {
                asked.push(Query {
                    clause: index,
                    amount,
                    count,
                    total,
                    found: None,
                    line: LineState::default(),
                });
            }
        };
        for amount in 0..clause.amounts.len() {
            ask(amount, None, &mut queries);
        }
        for &per_unit in &clause.per_units {
            let Some(y) = clause.amounts.get(per_unit) else {
                continue;
            };
            for (total, t) in clause.amounts.iter().enumerate() {
                if t.value > y.value {
                    ask(per_unit, Some(total), &mut queries);
                }
            }
        }
    }
    answer(&clauses, &mut queries, corpus);
    let found = |clause: usize, amount: usize, count: usize, total: Option<usize>| {
        queries
            .iter()
            .find(|q| {
                q.clause == clause && q.amount == amount && q.count == count && q.total == total
            })
            .and_then(|q| q.found)
    };
    let mut out: Vec<(&str, MispairedFigure)> = Vec::new();
    for (index, clause) in clauses.iter().enumerate() {
        let is_quotient = |amount: usize| {
            (0..clause.counts.len()).any(|count| found(index, amount, count, None).is_some())
        };
        for &per_unit in &clause.per_units {
            let Some(y) = clause.amounts.get(per_unit) else {
                continue;
            };
            if out.iter().any(|(text, _)| *text == y.text) {
                continue;
            }
            let pairing = clause
                .amounts
                .iter()
                .enumerate()
                .filter(|&(total, t)| t.value > y.value && !is_quotient(total))
                .find_map(|(total, t)| {
                    clause.counts.iter().enumerate().find_map(|(count, n)| {
                        found(index, per_unit, count, Some(total)).map(|source| (t, n, source))
                    })
                });
            if let Some((total, count, source)) = pairing {
                out.push((y.text, mispaired(y, total, count, source)));
            }
        }
    }
    out.into_iter().map(|(_, m)| m).collect()
}

/// The finding for `per_unit`, stated beside `total` and `count`, while a
/// line printed it as `source` over the count.
fn mispaired(
    per_unit: &Stated<'_>,
    total: &Stated<'_>,
    count: &Stated<'_>,
    source: f64,
) -> MispairedFigure {
    let mark = per_unit.mark;
    let worked = amount(mark, total.value / count.value, per_unit.places.max(2));
    MispairedFigure {
        per_unit: format!("{mark}{}", per_unit.text),
        stated: format!("{mark}{} / {} = {worked}", total.text, count.text),
        printed: format!(
            "{} / {}",
            amount(mark, source, total.places.max(2)),
            count.text
        ),
    }
}

/// The clauses of `reply` that hold a candidate per-unit amount.
fn read_clauses(reply: &str) -> Vec<Clause<'_>> {
    let mut skip = link_spans(reply);
    skip.extend(ratios(reply).map(|ratio| ratio.span));
    let breaks = clause_breaks(reply);
    let mut clauses: Vec<Clause<'_>> = Vec::new();
    let mut current = Clause {
        amounts: Vec::new(),
        counts: Vec::new(),
        per_units: Vec::new(),
    };
    let mut at_break = 0;
    let mut scratch = Scratch::default();
    for token in tokens(reply) {
        if breaks.get(at_break).is_some_and(|&b| token.start >= b) {
            while breaks.get(at_break).is_some_and(|&b| token.start >= b) {
                at_break += 1;
            }
            close(&mut current, &mut clauses);
        }
        if token.magnitude > 0 || skipped(&skip, token.start) {
            continue;
        }
        stated_in(reply, &token, &mut scratch, |figure| {
            if !figure.mark.is_empty() && figure.value > 0.0 {
                current.amounts.push(figure);
            } else if figure.mark.is_empty() && figure.places == 0 && figure.value >= 2.0 {
                current.counts.push(figure);
            }
        });
    }
    close(&mut current, &mut clauses);
    clauses
}

/// Keeps `clause` when it holds a candidate per-unit amount, and starts the
/// next one.
fn close<'r>(clause: &mut Clause<'r>, clauses: &mut Vec<Clause<'r>>) {
    let done = std::mem::replace(
        clause,
        Clause {
            amounts: Vec::new(),
            counts: Vec::new(),
            per_units: Vec::new(),
        },
    );
    if done.amounts.len() < 2 || done.counts.is_empty() {
        return;
    }
    let per_units: Vec<usize> = done
        .amounts
        .iter()
        .enumerate()
        .filter(|(_, y)| {
            y.significant >= MIN_SIGNIFICANT_DIGITS
                && done.amounts.iter().any(|t| t.value > y.value)
                && !done.amounts.iter().any(|t| {
                    done.counts
                        .iter()
                        .any(|n| y.quotient_of(t.value, t.cut(), n.value))
                })
        })
        .map(|(i, _)| i)
        .collect();
    if !per_units.is_empty() {
        clauses.push(Clause { per_units, ..done });
    }
}

/// Calls `f` with each figure `token` of `reply` states: its readings but the
/// derived ones and the decimal reading of a token that also groups ("1,250"
/// is 1250), each with the currency mark written before it.
fn stated_in<'r>(
    reply: &'r str,
    token: &Token<'r>,
    scratch: &mut Scratch,
    mut f: impl FnMut(Stated<'r>),
) {
    let whole = token.start..token.end();
    let mark = if token.marked_before {
        currency_start(reply, token.start)
            .and_then(|at| reply.get(at..token.start))
            .map_or("", str::trim)
    } else {
        ""
    };
    let mut seen_whole = false;
    token.for_each_reading(scratch, |r| {
        if r.derived {
            return;
        }
        if r.span == whole {
            if seen_whole {
                return;
            }
            seen_whole = true;
        }
        let Some(value) = value_of(r.integer, r.fraction) else {
            return;
        };
        let places = r.fraction.map_or(0, str::len);
        f(Stated {
            text: r.text,
            mark: if r.span.start == token.start {
                mark
            } else {
                ""
            },
            value,
            places,
            significant: significant_digits(r.integer, r.fraction),
        });
    });
}

/// The value of `integer.fraction`, both ASCII digits, parsed as written so
/// it rounds as the host's `Number` does. A figure that fits a small buffer,
/// as nearly every one does, is parsed without allocating.
fn value_of(integer: &str, fraction: Option<&str>) -> Option<f64> {
    let Some(fraction) = fraction else {
        return integer.parse().ok().filter(|v: &f64| v.is_finite());
    };
    let mut buffer = [0_u8; 64];
    let len = integer
        .len()
        .saturating_add(fraction.len())
        .saturating_add(1);
    let parsed = match buffer.get_mut(..len) {
        Some(written) => {
            let (head, tail) = written.split_at_mut(integer.len());
            head.copy_from_slice(integer.as_bytes());
            let (point, rest) = tail.split_first_mut()?;
            *point = b'.';
            rest.copy_from_slice(fraction.as_bytes());
            std::str::from_utf8(written).ok()?.parse()
        }
        None => format!("{integer}.{fraction}").parse(),
    };
    parsed.ok().filter(|v: &f64| v.is_finite())
}

/// The significant digits of `integer.fraction` as written: leading zeros
/// aside, and a whole number's trailing zeros too.
fn significant_digits(integer: &str, fraction: Option<&str>) -> usize {
    let fraction = fraction.unwrap_or_default();
    let digits = integer.len().saturating_add(fraction.len());
    let leading = integer
        .bytes()
        .chain(fraction.bytes())
        .take_while(|&b| b == b'0')
        .count()
        .min(digits.saturating_sub(1));
    let trailing = if fraction.is_empty() {
        integer.len() - integer.trim_end_matches('0').len()
    } else {
        0
    };
    digits
        .saturating_sub(leading)
        .saturating_sub(trailing)
        .max(1)
}

/// Fills each query's printed amount from the first line of `corpus` that
/// shows it: the line carries the count, a value the amount rounds, and a
/// larger printed amount the count divides into it, other than the total's
/// own value, the first such amount being the one kept. A line is read once,
/// each figure met against the open queries as it is read, so nothing is
/// kept of the corpus.
fn answer(clauses: &[Clause<'_>], queries: &mut [Query], corpus: &str) {
    let mut scratch = Scratch::default();
    for line in corpus.split('\n') {
        for query in queries.iter_mut() {
            query.line = LineState::default();
        }
        for token in tokens(line)
            .against_letters()
            .without_signs()
            .without_scales()
        {
            let marked = token.marked_before;
            token.for_each_reading(&mut scratch, |r| {
                let Some(value) = value_of(r.integer, r.fraction) else {
                    return;
                };
                let printed_amount =
                    (marked && r.span.start == token.start) || r.fraction.is_some();
                for query in queries.iter_mut().filter(|q| q.found.is_none()) {
                    meet(clauses, query, value, printed_amount);
                }
            });
        }
        for query in queries.iter_mut() {
            let LineState {
                count,
                rounded,
                source,
            } = query.line;
            if query.found.is_none() && count && rounded {
                query.found = source;
            }
        }
    }
}

/// Records what a printed `value`, an amount when `printed_amount`, tells
/// `query` about the line it stands on.
fn meet(clauses: &[Clause<'_>], query: &mut Query, value: f64, printed_amount: bool) {
    let Some(clause) = clauses.get(query.clause) else {
        return;
    };
    let (Some(y), Some(count)) = (
        clause.amounts.get(query.amount),
        clause.counts.get(query.count),
    ) else {
        return;
    };
    let line = &mut query.line;
    line.count |= value.to_bits() == count.value.to_bits();
    line.rounded |= (value - y.value).abs() <= y.rounding();
    if printed_amount
        && line.source.is_none()
        && value > y.value
        && query
            .total
            .and_then(|t| clause.amounts.get(t))
            .is_none_or(|t| (value - t.value).abs() > t.cut())
        && y.quotient_of(value, 0.0, count.value)
    {
        line.source = Some(value);
    }
}

/// The breaks between the clauses of `text`, as Unicode's sentence rules
/// place them, plus one at every full stop before a space: a sentence's
/// segmenter runs on across one when the next word is lower case ("$35 CPL.
/// $984 lifetime"). A break follows a line break, and a question or
/// exclamation mark (the CJK ones too) or a full stop, with the closing
/// quotes and brackets and the spaces after it, unless a comma, a colon, a
/// semicolon, a dash or another mark comes next ("?, then"); a full stop
/// before no space ends no clause when a letter or a digit follows it
/// ("3.5", "acme.io") or, past its closers and spaces, a lower-case word.
fn clause_breaks(text: &str) -> Vec<usize> {
    let mut breaks = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let after = at.saturating_add(c.len_utf8());
        if is_line_break(c) {
            breaks.push(after);
            continue;
        }
        let full_stop = c == '.';
        if !(full_stop || is_sentence_end(c)) {
            continue;
        }
        if full_stop && chars.peek().is_some_and(|&(_, next)| next == ' ') {
            breaks.push(after);
            continue;
        }
        let glued = chars.peek().map(|&(_, next)| next);
        let closed = skip_while(&mut chars, |next| {
            matches!(
                next,
                '"' | '\'' | ')' | ']' | '\u{201d}' | '\u{2019}' | '\u{bb}'
            )
        });
        let spaced = skip_while(&mut chars, |next| {
            next.is_whitespace() && !is_line_break(next)
        });
        let Some(&(_, next)) = chars.peek() else {
            breaks.push(text.len());
            continue;
        };
        let goes_on = continues_sentence(next)
            || (full_stop
                && (next.is_lowercase()
                    || (closed.is_none()
                        && spaced.is_none()
                        && glued.is_some_and(char::is_alphanumeric))));
        if !goes_on {
            breaks.push(spaced.or(closed).unwrap_or(after));
        }
    }
    breaks
}

/// Whether `c` ends a sentence wherever it stands: a question or an
/// exclamation mark, or the CJK full stop and marks.
const fn is_sentence_end(c: char) -> bool {
    matches!(c, '?' | '!' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}')
}

/// Whether `c`, after a sentence's end, carries the sentence on: a comma, a
/// colon, a semicolon, a dash or another end mark (Unicode's `SContinue`,
/// `STerm` and `ATerm`).
const fn continues_sentence(c: char) -> bool {
    is_sentence_end(c)
        || matches!(
            c,
            '.' | ','
                | '-'
                | ':'
                | ';'
                | '\u{2013}'
                | '\u{2014}'
                | '\u{3001}'
                | '\u{ff0c}'
                | '\u{ff0d}'
                | '\u{ff1a}'
                | '\u{ff1b}'
                | '\u{ff64}'
                | '\u{60c}'
        )
}

/// Whether `c` breaks a line.
const fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Takes the characters `keep` accepts from `chars`, and returns the byte
/// the last one ends at; None when it takes none.
fn skip_while(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    keep: impl Fn(char) -> bool,
) -> Option<usize> {
    let mut end = None;
    while let Some((at, c)) = chars.next_if(|&(_, c)| keep(c)) {
        end = Some(at.saturating_add(c.len_utf8()));
    }
    end
}

/// `value` as English prints an amount, behind `mark`: thousands grouped by
/// commas, at `places` decimal places, a half rounded away from zero on the
/// shortest decimal that reads back as the value ("1.005" is "1.01").
fn amount(mark: &str, value: f64, places: usize) -> String {
    let written = format!("{}", value.abs());
    let (integer, fraction) = written.split_once('.').unwrap_or((&written, ""));
    let mut digits: Vec<u8> = integer.bytes().chain(fraction.bytes()).collect();
    let mut kept = integer.len().saturating_add(places);
    digits.resize(kept.max(digits.len()), b'0');
    let round_up = digits.get(kept).is_some_and(|&d| d >= b'5');
    digits.truncate(kept);
    if round_up {
        let mut carry = true;
        for d in digits.iter_mut().rev() {
            if *d == b'9' {
                *d = b'0';
            } else {
                *d = d.saturating_add(1);
                carry = false;
                break;
            }
        }
        if carry {
            digits.insert(0, b'1');
            kept = kept.saturating_add(1);
        }
    }
    let integer_len = kept.saturating_sub(places);
    let mut out = String::from(mark);
    for (i, &d) in digits.iter().enumerate() {
        if i == integer_len {
            out.push('.');
        } else if i > 0 && i < integer_len && (integer_len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(char::from(d));
    }
    out
}
