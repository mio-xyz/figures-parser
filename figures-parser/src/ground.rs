//! The comparison the grounding gate applies: what a reply states that the
//! turn's corpus does not carry.

use crate::number::scan::tokens;
use crate::number::{Key, Reading};

/// One figure the reply states.
struct Needle<'r> {
    /// What it is compared as.
    key: Key,
    /// As the reply wrote it.
    text: &'r str,
    /// Its place among the reply's stated figures.
    index: usize,
}

/// The figures a reply states that the corpus has not yet carried, bucketed
/// by digit count so a corpus reading is compared only against the few that
/// could equal it.
#[derive(Default)]
struct Needles<'r> {
    /// The open needles, by integer length.
    open: Vec<Vec<Needle<'r>>>,
    /// How many needles are open, so the corpus walk can stop without
    /// scanning the buckets after every token.
    open_count: usize,
    /// How many figures the reply stated, duplicates included.
    stated: usize,
}

impl<'r> Needles<'r> {
    /// Adds a stated figure unless an equal key is already open.
    fn add(&mut self, key: Key, text: &'r str) {
        let len = key.integer().len();
        if self.open.len() <= len {
            self.open.resize_with(len + 1, Vec::new);
        }
        let Some(bucket) = self.open.get_mut(len) else {
            return;
        };
        if bucket.iter().any(|n| n.key == key) {
            return;
        }
        bucket.push(Needle {
            key,
            text,
            index: self.stated,
        });
        self.stated += 1;
        self.open_count += 1;
    }

    /// Closes every open needle a corpus reading grounds.
    fn ground(&mut self, reading: &Reading<'_, '_>) {
        let Some(bucket) = self.open.get_mut(reading.integer.len()) else {
            return;
        };
        let before = bucket.len();
        bucket.retain(|n| !n.key.grounded_by(reading));
        self.open_count -= before - bucket.len();
    }

    /// The open needles' texts, in the order the reply stated them.
    fn texts(self) -> Vec<String> {
        let mut open: Vec<Needle<'r>> = self.open.into_iter().flatten().collect();
        open.sort_by_key(|n| n.index);
        open.into_iter().map(|n| n.text.to_owned()).collect()
    }
}

/// Figures in `reply` that `corpus` does not carry, as the reply wrote them.
///
/// A decimal is matched as written: the corpus keeps its point, and a printed
/// decimal grounds the same value written with fewer places.
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
    if needles.open_count == 0 {
        return Vec::new();
    }
    for token in tokens(corpus) {
        token.for_each_reading(&mut scratch, |r| needles.ground(&r));
        if needles.open_count == 0 {
            return Vec::new();
        }
    }
    needles.texts()
}
