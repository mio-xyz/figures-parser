//! The figure gate's comparison: what a reply states that the turn's corpus
//! does not carry, and the amounts a reply shares with one print.

use std::hash::BuildHasher;
use std::ops::Range;

use rustc_hash::FxBuildHasher;

use super::digits::digit_value;
use super::ratio::ratios;
use super::scan::{Scratch, Token, currency_start, tokens};
use super::spelled::spelled;
use super::{Key, Reading, Sign, Value, for_each_reading_in};
use crate::url::urls;

/// One figure the reply states, as it wrote it. The readings it fits are
/// alternatives ("1.250" is 1,250 or 1.25), and the corpus grounds it by
/// carrying any one of them.
#[derive(Clone)]
struct Claim<'r> {
    /// As the reply wrote it.
    text: &'r str,
    /// Where the reply wrote it, which its alternative readings share.
    span: Range<usize>,
    /// Whether the corpus has not yet carried it.
    open: bool,
}

/// The claims of a reply and what a corpus walk closes among them: the
/// part of a reading that a walk mutates, kept apart from the keys so a
/// reply read once is walked against many prints.
#[derive(Clone, Default)]
struct Claims<'r> {
    /// Every stated figure, in the reply's order.
    list: Vec<Claim<'r>>,
    /// How many the corpus has not yet carried, so the walk stops as soon
    /// as it reaches none.
    open: usize,
    /// The whole a printed decimal rounds up to, written here per reading
    /// so the walk allocates nothing.
    rounded: String,
}

impl Claims<'_> {
    /// Whether claim `index` is still open.
    fn is_open(&self, index: usize) -> bool {
        self.list.get(index).is_some_and(|claim| claim.open)
    }

    /// Closes claim `index` when it is open.
    fn close(&mut self, index: usize) {
        if let Some(claim) = self.list.get_mut(index)
            && claim.open
        {
            claim.open = false;
            self.open -= 1;
        }
    }
}

/// One key a claim may be grounded by.
#[derive(Clone)]
struct Needle {
    /// What it is compared as.
    key: Key,
    /// The claim it grounds.
    claim: usize,
    /// [`hash_of`] its integer; 0 for an approximate figure, which is not
    /// searched by hash.
    hash: u64,
}

/// What a corpus reading does to a needle.
enum Met {
    /// Nothing: its claim was closed, or the value or the sign differs.
    Nothing,
    /// It grounds the needle, whose claim it closes.
    Closed,
    /// The value matches, and the needle waits on the reading's unread sign.
    WantsSign,
}

impl Needle {
    /// Closes `claim`, this needle's, when `reading` grounds the needle,
    /// `sign` being the reading's, or None while it is unread.
    fn meet(&self, claim: &mut Claim<'_>, reading: &Reading<'_, '_>, sign: Option<Sign>) -> Met {
        if !claim.open || !self.key.value_grounded_by(reading) {
            return Met::Nothing;
        }
        let carried = match sign {
            Some(read) => self.key.sign.carried_by(read),
            None if self.key.sign == Sign::Unsigned => true,
            None => return Met::WantsSign,
        };
        if carried {
            claim.open = false;
            Met::Closed
        } else {
            Met::Nothing
        }
    }
}

/// The bit `lengths` sets for an integer of `len` digits; every length from
/// 63 up shares the last bit.
fn length_bit(len: usize) -> u64 {
    1 << len.min(63)
}

/// The hash a needle is sorted and searched by: its integer's, which every
/// reading that grounds it shares.
fn hash_of(integer: &str) -> u64 {
    FxBuildHasher.hash_one(integer)
}

/// Whether two sets of keys, each given as an iterator, hold the same keys.
fn same_keys<'k>(
    a: &(impl Iterator<Item = &'k Key> + Clone),
    b: &(impl Iterator<Item = &'k Key> + Clone),
) -> bool {
    a.clone().all(|k| b.clone().any(|l| l == k)) && b.clone().all(|l| a.clone().any(|k| k == l))
}

/// Whether a printed decimal is stated as the whole above it once rounded:
/// its fraction's first digit is five or more ("327.76" is stated as 328).
fn rounds_up(reading: &Reading<'_, '_>) -> bool {
    reading
        .fraction
        .is_some_and(|f| matches!(f.as_bytes().first(), Some(b'5'..=b'9')))
}

/// Writes `digits` plus one into `out`; false when `digits` is not a run of
/// ASCII digits.
fn increment(digits: &str, out: &mut String) -> bool {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    out.clear();
    let kept = digits.trim_end_matches('9');
    match kept.as_bytes().last() {
        None => out.push('1'),
        Some(&last) => {
            out.push_str(kept.get(..kept.len().saturating_sub(1)).unwrap_or_default());
            out.push(char::from(last.saturating_add(1)));
        }
    }
    for _ in kept.len()..digits.len() {
        out.push('0');
    }
    true
}

/// A range written with round bounds brackets the values a result printed
/// rather than stating one ("CPMs of $200-300"): round is at most this many
/// significant digits. A range with a precise bound ("$8,093-8,220") states
/// figures, and is checked bound by bound.
const ROUND_DIGITS: usize = 2;

/// The value of a whole number written round, with at most [`ROUND_DIGITS`]
/// significant digits ("3200", "45000"); None for any other integer.
fn round_integer(integer: &str) -> Option<u128> {
    let significant = integer.trim_end_matches('0').len().max(1);
    (significant <= ROUND_DIGITS)
        .then(|| integer.parse().ok())
        .flatten()
}

/// The value of a reading that may bound a round range: a whole number of
/// at most [`ROUND_DIGITS`] significant digits.
fn round_value(reading: &Reading<'_, '_>) -> Option<u128> {
    if reading.fraction.is_some() {
        return None;
    }
    round_integer(reading.integer)
}

/// The power of ten a round figure of `len` digits is read at, so that it
/// keeps [`ROUND_DIGITS`] significant digits: 100 for "3200" and for
/// "3000" alike. None when it does not fit.
fn round_unit(len: usize) -> Option<u128> {
    let exponent = u32::try_from(len.saturating_sub(ROUND_DIGITS)).ok()?;
    10u128.checked_pow(exponent)
}

/// A reading's value rounded to a multiple of `unit`, halves up; None when
/// the integer part does not fit.
fn rounded_to(reading: &Reading<'_, '_>, unit: u128) -> Option<u128> {
    let whole: u128 = reading.integer.parse().ok()?;
    let up = if unit == 1 {
        rounds_up(reading)
    } else {
        (whole % unit).checked_mul(2)? >= unit
    };
    let quotient = whole / unit;
    quotient.checked_add(u128::from(up))?.checked_mul(unit)
}

/// One open claim stated as a round whole number, which a printed value
/// rounding to it grounds.
struct Rounded {
    /// The stated value.
    value: u128,
    /// The power of ten it is read at.
    unit: u128,
    /// The needle that states it.
    needle: usize,
}

/// Whether `token` follows byte `prev_end` of `text` with nothing but a
/// dash between them, spaces and the token's currency mark aside ("$200-300",
/// "$200 – $300", "1,500 - 2,000").
fn dashed(text: &str, prev_end: usize, token: &Token<'_>) -> bool {
    let full_start = token.end().saturating_sub(token.full.len());
    let lead = currency_start(text, token.start).map_or(full_start, |at| at.min(full_start));
    let between = text.get(prev_end..lead).unwrap_or_default().trim();
    let mut chars = between.chars();
    chars
        .next()
        .is_some_and(|c| matches!(c, '-' | '\u{2013}' | '\u{2014}'))
        && chars.next().is_none()
}

/// The byte ranges of the links in `text`: a link's digits (a Slack ts, an
/// id) are the link check's to judge, never a figure.
pub(super) fn link_spans(text: &str) -> Vec<Range<usize>> {
    if !text.contains("://") {
        return Vec::new();
    }
    urls(text).into_iter().map(|url| url.span).collect()
}

/// Whether byte `at` falls in one of `spans`.
pub(super) fn skipped(spans: &[Range<usize>], at: usize) -> bool {
    spans.iter().any(|span| span.contains(&at))
}

/// The keys of the figures a reply states, sorted by their integer's hash
/// so a corpus reading is compared only against the few that share its own
/// integer. Read once with its [`Claims`], and never written after.
#[derive(Clone, Default)]
struct Needles {
    /// The exact keys: in the reply's order while it is read, by hash once
    /// [`Needles::seal`] has run.
    all: Vec<Needle>,
    /// The approximate keys, which readings of several integers ground, so
    /// each is compared by value.
    around: Vec<Needle>,
    /// A bit per integer length the exact keys state, so most corpus
    /// readings are turned away before any search.
    lengths: u64,
    /// A bit per integer length that may ground an approximate key.
    around_lengths: u64,
    /// The fewest integer digits any key may be grounded by, so a corpus
    /// token with fewer digits than that is passed over unread.
    fewest: usize,
    /// The same over the keys a bare digit run may ground: a whole number
    /// or an approximate one. A precise decimal needs a token with a symbol.
    fewest_whole: usize,
}

/// How many digits `text` holds, in any script, and whether a symbol stands
/// among them.
fn digits_and_symbol(text: &str) -> (usize, bool) {
    if text.is_ascii() {
        let digits = text.bytes().filter(u8::is_ascii_digit).count();
        return (digits, digits < text.len());
    }
    let mut digits = 0_usize;
    let mut symbol = false;
    for c in text.chars() {
        if digit_value(c).is_some() {
            digits = digits.saturating_add(1);
        } else {
            symbol = true;
        }
    }
    (digits, symbol)
}

impl Needles {
    /// The figures `reply` states outside `skip`, each held to the key
    /// `stated` gives its readings, sealed for the corpus walk, with their
    /// claims all open.
    fn read<'r>(
        reply: &'r str,
        skip: &[Range<usize>],
        scratch: &mut Scratch,
        stated: impl Fn(&Reading<'_, '_>) -> Option<Key>,
    ) -> (Self, Claims<'r>) {
        let mut needles = Self {
            fewest: usize::MAX,
            fewest_whole: usize::MAX,
            ..Self::default()
        };
        let mut claims = Claims::default();
        for_each_reading_in(reply, tokens(reply), scratch, |r| {
            if !skipped(skip, r.span.start)
                && let Some(key) = stated(&r)
            {
                needles.add(&mut claims, key, r.full_text, r.span);
            }
        });
        needles.seal();
        (needles, claims)
    }

    /// Adds a stated reading: to the claim before it when it is another
    /// reading of the same written figure, else as a claim of its own.
    fn add<'r>(&mut self, claims: &mut Claims<'r>, key: Key, text: &'r str, span: Range<usize>) {
        let same_figure = claims.list.last().is_some_and(|last| last.span == span);
        if !same_figure {
            claims.list.push(Claim {
                text,
                span,
                open: true,
            });
            claims.open += 1;
        }
        let claim = claims.list.len().saturating_sub(1);
        let (list, lengths, hash) = if key.around.is_some() {
            (&mut self.around, &mut self.around_lengths, 0)
        } else {
            (&mut self.all, &mut self.lengths, hash_of(key.integer()))
        };
        if list.iter().any(|n| n.claim == claim && n.key == key) {
            return;
        }
        let lengths_of_key = key.integer_lengths();
        self.fewest = self.fewest.min(*lengths_of_key.start());
        let whole = key.around.is_some()
            || matches!(&key.value, Value::Integer(_))
            || matches!(&key.value, Value::Exact { fraction, .. } if fraction.is_empty());
        if whole {
            self.fewest_whole = self.fewest_whole.min(*lengths_of_key.start());
        }
        for len in lengths_of_key {
            *lengths |= length_bit(len);
        }
        list.push(Needle { key, claim, hash });
    }

    /// Orders the needles by hash, once the reply is read.
    fn seal(&mut self) {
        self.all.sort_by_key(|n| n.hash);
    }

    /// Closes every open claim a corpus reading grounds: by its value, and
    /// by the whole it rounds up to, which grounds an exact whole figure.
    /// `sign` is the reading's, or None while it is unread: a signed needle
    /// whose value matches then stays open, and the call returns true so the
    /// caller reads the sign, which few readings need, and calls again with
    /// it.
    #[inline]
    fn ground(
        &self,
        claims: &mut Claims<'_>,
        reading: &Reading<'_, '_>,
        sign: Option<Sign>,
    ) -> bool {
        let bit = length_bit(reading.integer.len());
        let mut wants_sign = (self.lengths | self.around_lengths) & bit != 0
            && self.ground_length(claims, reading, sign, bit, false);
        if rounds_up(reading) {
            let mut rounded = std::mem::take(&mut claims.rounded);
            if increment(reading.integer, &mut rounded) {
                let rounded_bit = length_bit(rounded.len());
                if self.lengths & rounded_bit != 0 {
                    let whole = Reading {
                        integer: rounded.as_str(),
                        fraction: None,
                        ..reading.clone()
                    };
                    wants_sign |= self.ground_length(claims, &whole, sign, rounded_bit, true);
                }
            }
            claims.rounded = rounded;
        }
        wants_sign
    }

    /// Whether some needle has the integer length of `reading`, or of the
    /// whole it rounds up to: the one test that turns most corpus readings
    /// away, so a caller's own filter runs only on the rest.
    #[inline]
    fn may_ground(&self, reading: &Reading<'_, '_>) -> bool {
        let len = reading.integer.len();
        (self.lengths | self.around_lengths) & length_bit(len) != 0
            || (rounds_up(reading) && self.lengths & length_bit(len.saturating_add(1)) != 0)
    }

    /// [`Needles::ground`] for a reading whose integer length, `bit`, some
    /// needle states. With `whole_only`, the reading is a rounded whole,
    /// which grounds an exact whole figure and nothing else.
    fn ground_length(
        &self,
        claims: &mut Claims<'_>,
        reading: &Reading<'_, '_>,
        sign: Option<Sign>,
        bit: u64,
        whole_only: bool,
    ) -> bool {
        let mut wants_sign = false;
        let mut meet = |needle: &Needle| {
            if whole_only && !matches!(needle.key.value, Value::Integer(_)) {
                return;
            }
            let Some(claim) = claims.list.get_mut(needle.claim) else {
                return;
            };
            match needle.meet(claim, reading, sign) {
                Met::Nothing => {}
                Met::Closed => claims.open -= 1,
                Met::WantsSign => wants_sign = true,
            }
        };
        if self.lengths & bit != 0 {
            let wanted = hash_of(reading.integer);
            let from = self.all.partition_point(|n| n.hash < wanted);
            let same = self.all.get(from..).unwrap_or_default();
            same.iter()
                .take_while(|n| n.hash == wanted)
                .for_each(&mut meet);
        }
        if !whole_only && self.around_lengths & bit != 0 {
            self.around.iter().for_each(&mut meet);
        }
        wants_sign
    }

    /// Closes the open claims the readings of `corpus` that `grounds`
    /// accepts carry. The corpus is read only until everything is found, its
    /// numbers in words last, since they are rare in one.
    fn ground_in(
        &self,
        claims: &mut Claims<'_>,
        corpus: &str,
        scratch: &mut Scratch,
        grounds: impl Fn(&Reading<'_, '_>) -> bool,
    ) {
        if claims.open == 0 {
            return;
        }
        for token in tokens(corpus)
            .against_letters()
            .loose_scales()
            .without_currency()
            .without_signs()
        {
            // A reading's integer part has at most the token's digits, one
            // more when a decimal rounds up, and a bare run grounds no
            // precise decimal: a shorter token is left unread.
            if token.magnitude == 0 {
                let (digits, symbol) = digits_and_symbol(token.text);
                let fewest = if symbol {
                    self.fewest
                } else {
                    self.fewest_whole
                };
                if digits.saturating_add(1) < fewest {
                    continue;
                }
            }
            token.for_each_reading(scratch, |r| {
                if self.may_ground(&r) && grounds(&r) && self.ground(claims, &r, None) {
                    self.ground(claims, &r, Some(token.sign_of(&r, corpus)));
                }
            });
            if claims.open == 0 {
                return;
            }
        }
        for number in spelled(corpus) {
            let reading = scratch.spelled(corpus, &number);
            let sign = reading.sign;
            if grounds(&reading) {
                self.ground(claims, &reading, Some(sign));
            }
            if claims.open == 0 {
                return;
            }
        }
    }

    /// Closes the open claims that bound a round range of `reply`, outside
    /// `skip`, when a figure `corpus` prints falls inside it: "$200-300"
    /// brackets a printed 244.49 rather than stating 200 or 300.
    fn ground_brackets(
        &self,
        claims: &mut Claims<'_>,
        reply: &str,
        skip: &[Range<usize>],
        corpus: &str,
        scratch: &mut Scratch,
    ) {
        // Each range's bounds, and whether a printed value fell inside it.
        let mut ranges: Vec<(u128, u128, bool)> = Vec::new();
        let mut previous: Option<(Option<u128>, usize)> = None;
        for token in tokens(reply) {
            if skipped(skip, token.start) {
                previous = None;
                continue;
            }
            let mut first: Option<Option<u128>> = None;
            token.for_each_reading(scratch, |r| {
                if first.is_none() && !r.derived {
                    first = Some(round_value(&r));
                }
            });
            let Some(value) = first else {
                continue;
            };
            if let Some((Some(low), prev_end)) = previous
                && let Some(high) = value
                && low < high
                && dashed(reply, prev_end, &token)
            {
                ranges.push((low, high, false));
            }
            previous = Some((value, token.end()));
        }
        if ranges.is_empty() {
            return;
        }
        for token in tokens(corpus)
            .against_letters()
            .loose_scales()
            .without_currency()
            .without_signs()
        {
            token.for_each_reading(scratch, |r| {
                let Ok(value) = r.integer.parse::<u128>() else {
                    return;
                };
                let whole = r.fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
                for (low, high, inside) in &mut ranges {
                    *inside |= value >= *low && (value < *high || (value == *high && whole));
                }
            });
            if ranges.iter().all(|&(_, _, inside)| inside) {
                break;
            }
        }
        for &(low, high, inside) in &ranges {
            if inside {
                self.close_whole(claims, low);
                self.close_whole(claims, high);
            }
        }
    }

    /// Closes the open claims stated as a round whole number that a figure
    /// `corpus` prints rounds to at [`ROUND_DIGITS`] significant digits:
    /// "rose to $3,200" is grounded by a printed 3,225. A figure of one
    /// significant digit is read at two as well, so "$3,000" stands for
    /// 2,950 to 3,049, never for everything from 2,500 to 3,499, and a
    /// turn printing thousands of prices still grounds few round figures.
    /// A signed figure is held to its sign.
    fn ground_rounded(&self, claims: &mut Claims<'_>, corpus: &str, scratch: &mut Scratch) {
        let mut wanted: Vec<Rounded> = self
            .all
            .iter()
            .enumerate()
            .filter(|(_, n)| claims.is_open(n.claim))
            .filter_map(|(needle, n)| {
                let Value::Integer(integer) = &n.key.value else {
                    return None;
                };
                Some(Rounded {
                    value: round_integer(integer)?,
                    unit: round_unit(integer.len())?,
                    needle,
                })
            })
            .collect();
        if wanted.is_empty() {
            return;
        }
        for token in tokens(corpus)
            .against_letters()
            .loose_scales()
            .without_currency()
            .without_signs()
        {
            token.for_each_reading(scratch, |r| {
                let mut sign: Option<Sign> = None;
                wanted.retain(|target| {
                    let Some(needle) = self.all.get(target.needle) else {
                        return false;
                    };
                    if !claims.is_open(needle.claim) {
                        return false;
                    }
                    if rounded_to(&r, target.unit) != Some(target.value) {
                        return true;
                    }
                    let read = *sign.get_or_insert_with(|| token.sign_of(&r, corpus));
                    if !needle.key.sign.carried_by(read) {
                        return true;
                    }
                    claims.close(needle.claim);
                    false
                });
            });
            if wanted.is_empty() {
                return;
            }
        }
    }

    /// Closes every open claim stated as exactly the whole number `value`.
    fn close_whole(&self, claims: &mut Claims<'_>, value: u128) {
        for needle in &self.all {
            if let Value::Integer(integer) = &needle.key.value
                && integer.parse::<u128>().ok() == Some(value)
            {
                claims.close(needle.claim);
            }
        }
    }

    /// The open claims' texts, in the order the reply stated them; a figure
    /// stated again, with the same keys, is named once.
    fn texts(&self, claims: &Claims<'_>) -> Vec<String> {
        let keys = |claim: usize| {
            self.all
                .iter()
                .chain(&self.around)
                .filter(move |n| n.claim == claim)
                .map(|n| &n.key)
        };
        let mut out = Vec::new();
        for (i, claim) in claims.list.iter().enumerate() {
            let repeated = || {
                claims
                    .list
                    .iter()
                    .take(i)
                    .enumerate()
                    .any(|(j, earlier)| earlier.open && same_keys(&keys(i), &keys(j)))
            };
            if claim.open && !repeated() {
                out.push(claim.text.to_owned());
            }
        }
        out
    }
}

/// The closed claims' texts, once each, in the order the reply stated them.
fn shared<'r>(claims: &Claims<'r>) -> Vec<&'r str> {
    let mut out: Vec<&str> = Vec::new();
    for claim in claims.list.iter().filter(|claim| !claim.open) {
        if !out.contains(&claim.text) {
            out.push(claim.text);
        }
    }
    out
}

/// Figures in `reply` that `corpus` does not carry, as the reply wrote them.
///
/// A figure is held when its notation states an amount: a thousands
/// separator ("8,220"), a currency mark ("$729"), four fractional digits or
/// more, a scale word, a number in words or an approximation mark. A bare
/// digit run ("541611") is a code, a zip or an order number as often as an
/// amount, and is never held; nor are the digits inside a link, which are
/// the link check's. A decimal is matched as written: the corpus keeps its
/// point, and a printed decimal grounds the same value written with fewer
/// places, and the whole it rounds to ("327.76" grounds 328). A signed figure
/// is held to its sign, as [`Sign::carried_by`] reads it, and an approximate
/// one ("~12,500") is grounded by any value that rounds to it; each is
/// reported with its sign and its mark. A range written with round bounds
/// ("$200-300") is grounded by a printed figure inside it, and a lone round
/// figure ("$3,200") by a printed value that rounds to it at two significant
/// digits (3,225); one of a single significant digit is read at two as well,
/// so "$3,000" is grounded by 2,950 to 3,049 only. A figure of three
/// significant digits or more states a value and is matched whole.
///
/// # Examples
///
/// ```
/// let corpus = r#"{"revenue": 1250000, "orders": 4812}"#;
/// let reply = "Revenue was 1,250,000 on 4,812 orders, up from 1,180,000.";
/// assert_eq!(figures_parser::ungrounded_figures(reply, corpus), ["1,180,000"]);
///
/// // The corpus has the value, not its sign.
/// assert_eq!(figures_parser::ungrounded_figures("It moved by -12,500.", "delta: 12500"), ["-12,500"]);
///
/// // A round range brackets what a result printed, and a round figure rounds it.
/// assert!(figures_parser::ungrounded_figures("CPMs of $200-300.", "CPM=$244.49").is_empty());
/// assert!(figures_parser::ungrounded_figures("Rose to $3,200.", "max 3225.000000").is_empty());
/// assert_eq!(figures_parser::ungrounded_figures("Over $3,000.", "close 3148.02"), ["3,000"]);
/// ```
#[must_use]
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method's path fixes the reading's lifetimes, which the bound leaves free"
)]
pub fn ungrounded_figures(reply: &str, corpus: &str) -> Vec<String> {
    let skip = link_spans(reply);
    let mut scratch = Scratch::default();
    let (needles, mut claims) = Needles::read(reply, &skip, &mut scratch, |r| r.stated());
    // Most replies state no large figure at all, and the corpus is the whole
    // turn: it is read only once there is something to look for, and only
    // until everything is found.
    if claims.open == 0 {
        return Vec::new();
    }
    needles.ground_in(&mut claims, corpus, &mut scratch, |_| true);
    if claims.open > 0 {
        needles.ground_brackets(&mut claims, reply, &skip, corpus, &mut scratch);
    }
    if claims.open > 0 {
        needles.ground_rounded(&mut claims, corpus, &mut scratch);
    }
    needles.texts(&claims)
}

/// The figures in `reply` that `corpus` does not carry, a reading being held
/// to the key `stated` gives it, or to nothing when that is None, and
/// grounded only by the corpus readings `grounds` accepts.
pub(super) fn ungrounded_by(
    reply: &str,
    corpus: &str,
    stated: impl Fn(&Reading<'_, '_>) -> Option<Key>,
    grounds: impl Fn(&Reading<'_, '_>) -> bool,
) -> Vec<String> {
    let mut scratch = Scratch::default();
    let (needles, mut claims) = Needles::read(reply, &[], &mut scratch, stated);
    if claims.open == 0 {
        return Vec::new();
    }
    needles.ground_in(&mut claims, corpus, &mut scratch, grounds);
    needles.texts(&claims)
}

/// The amounts a reply states, read once, to match against one print at a
/// time.
///
/// An amount is a figure [`ungrounded_figures`] holds, or a bare digit run
/// from 10,000 up, which one print vouches for as an amount. A ratio's figure
/// ("7,789%", "1,250x") is no amount, nor is anything inside a link.
///
/// # Examples
///
/// ```
/// use figures_parser::Amounts;
///
/// let amounts = Amounts::of("2027 holds $19,850 and Airbnb $729, up 7,789%.");
/// assert_eq!(amounts.shared_with("Total 19850 | Airbnb 729"), ["19,850", "729"]);
/// assert!(amounts.shared_with("Total 175,800").is_empty());
/// ```
#[derive(Clone)]
pub struct Amounts<'r> {
    /// The reply's amounts' keys.
    needles: Needles,
    /// The reply's amounts, none yet grounded.
    claims: Claims<'r>,
}

impl<'r> Amounts<'r> {
    /// The amounts `reply` states.
    #[must_use]
    pub fn of(reply: &'r str) -> Self {
        let mut skip = link_spans(reply);
        skip.extend(ratios(reply).map(|ratio| ratio.span));
        let mut scratch = Scratch::default();
        let (needles, claims) =
            Needles::read(reply, &skip, &mut scratch, |r| r.stated_amount(true));
        Self { needles, claims }
    }

    /// Whether the reply states no amount.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.claims.open == 0
    }

    /// The reply's amounts that `text` carries, once each, as the reply
    /// wrote them.
    #[must_use]
    pub fn shared_with(&self, text: &str) -> Vec<&'r str> {
        if self.is_empty() {
            return Vec::new();
        }
        let mut claims = self.claims.clone();
        let mut scratch = Scratch::default();
        self.needles
            .ground_in(&mut claims, text, &mut scratch, |_| true);
        shared(&claims)
    }
}

/// Whether `reply` states an amount that `text` carries.
///
/// # Examples
///
/// ```
/// assert!(figures_parser::shares_amount("September 2026: $15,977", "| 2026-09 | 15,977 |"));
/// assert!(!figures_parser::shares_amount("Which tab, 2026 or 2027?", "2026 tab: 81 rows"));
/// ```
#[must_use]
pub fn shares_amount(reply: &str, text: &str) -> bool {
    !Amounts::of(reply).shared_with(text).is_empty()
}

/// Whether `text` states `figure` ("19,851", "1,234.50"), however it groups
/// it: the figure's whole part is matched, as its first reading gives it.
///
/// # Examples
///
/// ```
/// assert!(figures_parser::states_figure("the TOTAL row says 19851", "19,851"));
/// assert!(figures_parser::states_figure("a fee of $1,234", "1,234.50"));
/// assert!(!figures_parser::states_figure("2027: $19,850", "19,851"));
/// ```
#[must_use]
pub fn states_figure(text: &str, figure: &str) -> bool {
    let mut scratch = Scratch::default();
    let Some(figure_token) = tokens(figure).next() else {
        return false;
    };
    let mut wanted: Option<String> = None;
    figure_token.for_each_reading(&mut scratch, |r| {
        if wanted.is_none() && !r.derived {
            wanted = Some(r.integer.to_owned());
        }
    });
    let Some(wanted) = wanted else {
        return false;
    };
    for token in tokens(text)
        .against_letters()
        .without_currency()
        .without_signs()
    {
        let mut found = false;
        token.for_each_reading(&mut scratch, |r| {
            found |= r.integer == wanted;
        });
        if found {
            return true;
        }
    }
    false
}
