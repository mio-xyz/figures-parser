//! The comparison the grounding gate applies: what a reply states that the
//! turn's corpus does not carry.

use std::collections::{HashMap, HashSet};

use crate::number::Key;
use crate::number::scan::tokens;

/// Figures in `reply` that `corpus` does not carry, as the reply wrote them.
///
/// A decimal is matched as written: the corpus keeps its point, and a printed
/// decimal grounds the same value written with fewer places.
#[must_use]
pub fn ungrounded_figures(reply: &str, corpus: &str) -> Vec<String> {
    let mut texts: Vec<&str> = Vec::new();
    let mut seen: HashSet<Key> = HashSet::new();
    let mut integers: HashMap<String, usize> = HashMap::new();
    let mut decimals: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for token in tokens(reply) {
        for number in token.readings() {
            let Some(key) = number.stated() else {
                continue;
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            let index = texts.len();
            texts.push(number.text);
            match key {
                Key::Integer(integer) => {
                    integers.insert(integer, index);
                }
                Key::Decimal { integer, fraction } => {
                    decimals.entry(integer).or_default().push((fraction, index));
                }
            }
        }
    }
    // Most replies state no large figure at all, and the corpus is the whole
    // turn: it is read only once there is something to look for.
    if texts.is_empty() {
        return Vec::new();
    }
    let mut grounded = vec![false; texts.len()];
    let mut ground = |index: usize| {
        if let Some(g) = grounded.get_mut(index) {
            *g = true;
        }
    };
    'corpus: for token in tokens(corpus) {
        for number in token.readings() {
            if let Some(index) = integers.remove(&*number.integer) {
                ground(index);
            }
            if let Some(fraction) = &number.fraction
                && let Some(stated) = decimals.get_mut(&*number.integer)
            {
                stated.retain(|(prefix, index)| {
                    let hit = fraction.starts_with(prefix.as_str());
                    if hit {
                        ground(*index);
                    }
                    !hit
                });
                if stated.is_empty() {
                    decimals.remove(&*number.integer);
                }
            }
            if integers.is_empty() && decimals.is_empty() {
                break 'corpus;
            }
        }
    }
    texts
        .into_iter()
        .zip(grounded)
        .filter(|&(_, g)| !g)
        .map(|(text, _)| text.to_owned())
        .collect()
}
