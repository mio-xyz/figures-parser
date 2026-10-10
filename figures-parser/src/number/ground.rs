//! The figure gate's comparison: what a reply states that the turn's corpus
//! does not carry.

use std::hash::BuildHasher;

use rustc_hash::FxBuildHasher;

use super::scan::tokens;
use super::{Key, Reading, Sign};

/// One figure the reply states.
struct Needle<'r> {
    /// What it is compared as.
    key: Key,
    /// As the reply wrote it.
    text: &'r str,
    /// Its place among the reply's distinct stated figures.
    index: usize,
    /// [`hash_of`] its integer; 0 for an approximate figure, which is not
    /// searched by hash.
    hash: u64,
    /// Whether the corpus has not yet carried it.
    open: bool,
}

/// What a corpus reading does to a needle.
enum Met {
    /// Nothing: the needle was closed, or the value or the sign differs.
    Nothing,
    /// It grounds the needle, which it closes.
    Closed,
    /// The value matches, and the needle waits on the reading's unread sign.
    WantsSign,
}

impl Needle<'_> {
    /// Closes this needle when `reading` grounds it, `sign` being the
    /// reading's, or None while it is unread.
    fn meet(&mut self, reading: &Reading<'_, '_>, sign: Option<Sign>) -> Met {
        if !self.open || !self.key.value_grounded_by(reading) {
            return Met::Nothing;
        }
        let carried = match sign {
            Some(read) => self.key.sign.carried_by(read),
            None if self.key.sign == Sign::Unsigned => true,
            None => return Met::WantsSign,
        };
        if carried {
            self.open = false;
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

/// The figures a reply states, sorted by their integer's hash so a corpus
/// reading is compared only against the few that share its own integer.
#[derive(Default)]
struct Needles<'r> {
    /// Every distinct exact stated figure: in the reply's order while it is
    /// read, by hash once [`Needles::seal`] has run.
    all: Vec<Needle<'r>>,
    /// Every distinct approximate one, which readings of several integers
    /// ground, so each is compared by value.
    around: Vec<Needle<'r>>,
    /// A bit per integer length the exact figures state, so most corpus
    /// readings are turned away before any search.
    lengths: u64,
    /// A bit per integer length that may ground an approximate figure.
    around_lengths: u64,
    /// How many the corpus has not yet carried, so the corpus walk stops as
    /// soon as it reaches none.
    open: usize,
}

impl<'r> Needles<'r> {
    /// Adds a stated figure unless an equal key is already there.
    fn add(&mut self, key: Key, text: &'r str) {
        let index = self.all.len() + self.around.len();
        let (list, lengths, hash) = if key.around.is_some() {
            (&mut self.around, &mut self.around_lengths, 0)
        } else {
            (&mut self.all, &mut self.lengths, hash_of(key.integer()))
        };
        if list.iter().any(|n| n.key == key) {
            return;
        }
        for len in key.integer_lengths() {
            *lengths |= length_bit(len);
        }
        list.push(Needle {
            key,
            text,
            index,
            hash,
            open: true,
        });
        self.open += 1;
    }

    /// Orders the needles by hash, once the reply is read.
    fn seal(&mut self) {
        self.all.sort_by_key(|n| n.hash);
    }

    /// Closes every open needle a corpus reading grounds. `sign` is the
    /// reading's, or None while it is unread: a signed needle whose value
    /// matches then stays open, and the call returns true so the caller reads
    /// the sign, which few readings need, and calls again with it.
    fn ground(&mut self, reading: &Reading<'_, '_>, sign: Option<Sign>) -> bool {
        let bit = length_bit(reading.integer.len());
        if (self.lengths | self.around_lengths) & bit == 0 {
            return false;
        }
        let mut wants_sign = false;
        let mut meet = |needle: &mut Needle<'r>| match needle.meet(reading, sign) {
            Met::Nothing => {}
            Met::Closed => self.open -= 1,
            Met::WantsSign => wants_sign = true,
        };
        if self.lengths & bit != 0 {
            let wanted = hash_of(reading.integer);
            let from = self.all.partition_point(|n| n.hash < wanted);
            let same = self.all.get_mut(from..).unwrap_or_default();
            same.iter_mut()
                .take_while(|n| n.hash == wanted)
                .for_each(&mut meet);
        }
        if self.around_lengths & bit != 0 {
            self.around.iter_mut().for_each(&mut meet);
        }
        wants_sign
    }

    /// The open needles' texts, in the order the reply stated them.
    fn texts(self) -> Vec<String> {
        let mut open = self.all;
        open.extend(self.around);
        open.retain(|n| n.open);
        open.sort_by_key(|n| n.index);
        open.into_iter().map(|n| n.text.to_owned()).collect()
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
    let mut scratch = String::new();
    let mut needles = Needles::default();
    for token in tokens(reply) {
        token.for_each_reading(&mut scratch, |r| {
            if let Some(key) = r.stated() {
                needles.add(key, r.full_text);
            }
        });
    }
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
    needles.texts()
}
