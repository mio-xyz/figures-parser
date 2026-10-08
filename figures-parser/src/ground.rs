//! The comparison the grounding gate applies: what a reply states that the
//! turn's corpus does not carry.

use crate::number::Key;
use crate::number::scan::tokens;

/// One figure the reply states, and whether the corpus has carried it.
struct Needle<'r> {
    /// As the reply wrote it.
    text: &'r str,
    /// The integer part in ASCII digits.
    integer: String,
    /// The fractional digits a precise decimal states, which a printed
    /// decimal grounds by starting with them.
    fraction: Option<String>,
    /// Whether the corpus carried it.
    grounded: bool,
}

/// The figures a reply states, bucketed by digit count so a corpus reading
/// is compared only against the few that could equal it.
#[derive(Default)]
struct Needles<'r> {
    /// In the order the reply stated them.
    entries: Vec<Needle<'r>>,
    /// Indices into `entries` of the needles still open, by integer length.
    open: Vec<Vec<usize>>,
    /// How many needles are still open.
    open_count: usize,
}

impl<'r> Needles<'r> {
    /// Adds a stated figure unless an equal key is already there.
    fn add(&mut self, key: Key, text: &'r str) {
        let (integer, fraction) = match key {
            Key::Integer(integer) => (integer, None),
            Key::Decimal { integer, fraction } => (integer, Some(fraction)),
        };
        let len = integer.len();
        if self.open.len() <= len {
            self.open.resize_with(len + 1, Vec::new);
        }
        let Some(bucket) = self.open.get_mut(len) else {
            return;
        };
        let duplicate = bucket.iter().any(|&i| {
            self.entries
                .get(i)
                .is_some_and(|n| n.integer == integer && n.fraction == fraction)
        });
        if duplicate {
            return;
        }
        bucket.push(self.entries.len());
        self.entries.push(Needle {
            text,
            integer,
            fraction,
            grounded: false,
        });
        self.open_count += 1;
    }

    /// Closes every open needle a corpus reading grounds: the same integer,
    /// and for a precise decimal a printed fraction that starts with its own.
    fn ground(&mut self, integer: &str, fraction: Option<&str>) {
        let Self {
            entries,
            open,
            open_count,
        } = self;
        let Some(bucket) = open.get_mut(integer.len()) else {
            return;
        };
        bucket.retain(|&i| {
            let Some(needle) = entries.get_mut(i) else {
                return false;
            };
            let hit = needle.integer == integer
                && needle.fraction.as_deref().is_none_or(|stated| {
                    fraction.is_some_and(|printed| printed.starts_with(stated))
                });
            if hit {
                needle.grounded = true;
                *open_count -= 1;
            }
            !hit
        });
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
                needles.add(key, reply.get(r.span).unwrap_or_default());
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
        token.for_each_reading(&mut scratch, |r| needles.ground(r.integer, r.fraction));
        if needles.open_count == 0 {
            break;
        }
    }
    needles
        .entries
        .into_iter()
        .filter(|n| !n.grounded)
        .map(|n| n.text.to_owned())
        .collect()
}
