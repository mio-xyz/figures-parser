//! The figure gate's comparison: what a reply states that the turn's corpus
//! does not carry.

use std::hash::BuildHasher;
use std::ops::Range;

use rustc_hash::FxBuildHasher;

use super::scan::{Scratch, tokens};
use super::spelled::spelled;
use super::{Key, Reading, Sign, for_each_reading_in};

/// One figure the reply states, as it wrote it. The readings it fits are
/// alternatives ("1.250" is 1,250 or 1.25), and the corpus grounds it by
/// carrying any one of them.
struct Claim<'r> {
    /// As the reply wrote it.
    text: &'r str,
    /// Where the reply wrote it, which its alternative readings share.
    span: Range<usize>,
    /// Whether the corpus has not yet carried it.
    open: bool,
}

/// One key a claim may be grounded by.
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

/// The figures a reply states, their keys sorted by their integer's hash so
/// a corpus reading is compared only against the few that share its own
/// integer.
#[derive(Default)]
struct Needles<'r> {
    /// Every stated figure, in the reply's order.
    claims: Vec<Claim<'r>>,
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
    /// How many claims the corpus has not yet carried, so the corpus walk
    /// stops as soon as it reaches none.
    open: usize,
}

impl<'r> Needles<'r> {
    /// Adds a stated reading: to the claim before it when it is another
    /// reading of the same written figure, else as a claim of its own.
    fn add(&mut self, key: Key, text: &'r str, span: Range<usize>) {
        let same_figure = self.claims.last().is_some_and(|last| last.span == span);
        if !same_figure {
            self.claims.push(Claim {
                text,
                span,
                open: true,
            });
            self.open += 1;
        }
        let claim = self.claims.len().saturating_sub(1);
        let (list, lengths, hash) = if key.around.is_some() {
            (&mut self.around, &mut self.around_lengths, 0)
        } else {
            (&mut self.all, &mut self.lengths, hash_of(key.integer()))
        };
        if list.iter().any(|n| n.claim == claim && n.key == key) {
            return;
        }
        for len in key.integer_lengths() {
            *lengths |= length_bit(len);
        }
        list.push(Needle { key, claim, hash });
    }

    /// Orders the needles by hash, once the reply is read.
    fn seal(&mut self) {
        self.all.sort_by_key(|n| n.hash);
    }

    /// Closes every open claim a corpus reading grounds. `sign` is the
    /// reading's, or None while it is unread: a signed needle whose value
    /// matches then stays open, and the call returns true so the caller reads
    /// the sign, which few readings need, and calls again with it.
    #[inline]
    fn ground(&mut self, reading: &Reading<'_, '_>, sign: Option<Sign>) -> bool {
        let bit = length_bit(reading.integer.len());
        (self.lengths | self.around_lengths) & bit != 0 && self.ground_length(reading, sign, bit)
    }

    /// [`Needles::ground`] for a reading whose integer length, `bit`, some
    /// needle states.
    fn ground_length(&mut self, reading: &Reading<'_, '_>, sign: Option<Sign>, bit: u64) -> bool {
        let mut wants_sign = false;
        let mut meet = |needle: &Needle| {
            let Some(claim) = self.claims.get_mut(needle.claim) else {
                return;
            };
            match needle.meet(claim, reading, sign) {
                Met::Nothing => {}
                Met::Closed => self.open -= 1,
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
        if self.around_lengths & bit != 0 {
            self.around.iter().for_each(&mut meet);
        }
        wants_sign
    }

    /// The open claims' texts, in the order the reply stated them; a figure
    /// stated again, with the same keys, is named once.
    fn texts(&self) -> Vec<String> {
        let keys = |claim: usize| {
            self.all
                .iter()
                .chain(&self.around)
                .filter(move |n| n.claim == claim)
                .map(|n| &n.key)
        };
        let mut out = Vec::new();
        for (i, claim) in self.claims.iter().enumerate() {
            let repeated = || {
                self.claims
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

/// Figures in `reply` that `corpus` does not carry, as the reply wrote them.
///
/// A decimal is matched as written: the corpus keeps its point, and a printed
/// decimal grounds the same value written with fewer places. A signed figure
/// is held to its sign, as [`Sign::carried_by`] reads it, and an approximate
/// one ("~12,500") is grounded by any value that rounds to it; each is
/// reported with its sign and its mark.
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
/// ```
#[must_use]
pub fn ungrounded_figures(reply: &str, corpus: &str) -> Vec<String> {
    let mut scratch = Scratch::default();
    let mut needles = Needles::default();
    for_each_reading_in(reply, tokens(reply), &mut scratch, |r| {
        if let Some(key) = r.stated() {
            needles.add(key, r.full_text, r.span);
        }
    });
    // Most replies state no large figure at all, and the corpus is the whole
    // turn: it is read only once there is something to look for, and only
    // until everything is found.
    if needles.open == 0 {
        return Vec::new();
    }
    needles.seal();
    for token in tokens(corpus).without_currency().without_signs() {
        token.for_each_reading(&mut scratch, |r| {
            if needles.ground(&r, None) {
                needles.ground(&r, Some(token.sign_of(&r, corpus)));
            }
        });
        if needles.open == 0 {
            return Vec::new();
        }
    }
    // Numbers written in words are rare in a corpus, and searched for only
    // when the digits left something unfound.
    for number in spelled(corpus) {
        let reading = scratch.spelled(corpus, &number);
        let sign = reading.sign;
        needles.ground(&reading, Some(sign));
        if needles.open == 0 {
            return Vec::new();
        }
    }
    needles.texts()
}
