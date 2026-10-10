//! The figure gate's comparison: what a reply states that the turn's corpus
//! does not carry.

use std::hash::BuildHasher;

use rustc_hash::FxBuildHasher;

use super::scan::tokens;
use super::{Key, Reading};

/// One figure the reply states.
struct Needle<'r> {
    /// What it is compared as.
    key: Key,
    /// As the reply wrote it.
    text: &'r str,
    /// Its place among the reply's distinct stated figures.
    index: usize,
    /// [`hash_of`] its integer.
    hash: u64,
    /// Whether the corpus has not yet carried it.
    open: bool,
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
    /// Every distinct stated figure: in the reply's order while it is read,
    /// by hash once [`Needles::seal`] has run.
    all: Vec<Needle<'r>>,
    /// A bit per integer length the reply states, so most corpus readings
    /// are turned away before any search.
    lengths: u64,
    /// How many the corpus has not yet carried, so the corpus walk stops as
    /// soon as it reaches none.
    open: usize,
}

impl<'r> Needles<'r> {
    /// Adds a stated figure unless an equal key is already there.
    fn add(&mut self, key: Key, text: &'r str) {
        if self.all.iter().any(|n| n.key == key) {
            return;
        }
        self.lengths |= length_bit(key.integer().len());
        let hash = hash_of(key.integer());
        self.all.push(Needle {
            key,
            text,
            index: self.all.len(),
            hash,
            open: true,
        });
        self.open += 1;
    }

    /// Orders the needles by hash, once the reply is read.
    fn seal(&mut self) {
        self.all.sort_by_key(|n| n.hash);
    }

    /// Closes every open needle a corpus reading grounds.
    fn ground(&mut self, reading: &Reading<'_, '_>) {
        if self.lengths & length_bit(reading.integer.len()) == 0 {
            return;
        }
        let wanted = hash_of(reading.integer);
        let from = self.all.partition_point(|n| n.hash < wanted);
        let Some(same) = self.all.get_mut(from..) else {
            return;
        };
        for needle in same.iter_mut().take_while(|n| n.hash == wanted) {
            if needle.open && needle.key.grounded_by(reading) {
                needle.open = false;
                self.open -= 1;
            }
        }
    }

    /// The open needles' texts, in the order the reply stated them.
    fn texts(mut self) -> Vec<String> {
        self.all.retain(|n| n.open);
        self.all.sort_by_key(|n| n.index);
        self.all.into_iter().map(|n| n.text.to_owned()).collect()
    }
}

/// Figures in `reply` that `corpus` does not carry, as the reply wrote them.
///
/// A decimal is matched as written: the corpus keeps its point, and a printed
/// decimal grounds the same value written with fewer places.
///
/// # Examples
///
/// ```
/// let corpus = r#"{"revenue": 1250000, "orders": 4812}"#;
/// let reply = "Revenue was 1,250,000 on 4,812 orders, up from 1,180,000.";
/// assert_eq!(figures_parser::ungrounded_figures(reply, corpus), ["1,180,000"]);
/// ```
#[must_use]
pub fn ungrounded_figures(reply: &str, corpus: &str) -> Vec<String> {
    let mut scratch = String::new();
    let mut needles = Needles::default();
    for token in tokens(reply) {
        token.for_each_reading(&mut scratch, |r| {
            if let Some(key) = r.stated() {
                needles.add(key, r.text);
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
    for token in tokens(corpus).without_currency() {
        token.for_each_reading(&mut scratch, |r| needles.ground(&r));
        if needles.open == 0 {
            return Vec::new();
        }
    }
    needles.texts()
}
