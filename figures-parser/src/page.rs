//! A page rendered for a person, held to what the run read as a sent message
//! is: its figures, ratios and opaque ids, and the bookkeeping that holds a
//! page back at most a few times per file.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ungrounded_figures, ungrounded_ratios, unprinted_ids};

/// The figures, ratios and opaque ids of a page's `text` that `corpus`, what
/// the run read, does not carry, in that order.
///
/// `said`, what people wrote, grounds a ratio between their own numbers. A
/// page's links, dates and wording are its design's and are not checked.
///
/// A run whose attachment rides only as pixels has nothing to compare the
/// page with; its caller skips the check.
///
/// # Examples
///
/// ```
/// let page = "Sprint 42: activation up 12% on 1,840 sign-ups.";
/// let mut found = figures_parser::ungrounded_on_page(page, "", "");
/// found.sort();
/// assert_eq!(found, ["1,840", "12%"]);
///
/// let corpus = "signups 1840\nactivation_change 0.12";
/// assert!(figures_parser::ungrounded_on_page(page, corpus, "").is_empty());
/// ```
#[must_use]
pub fn ungrounded_on_page(text: &str, corpus: &str, said: &str) -> Vec<String> {
    let mut out = ungrounded_figures(text, corpus);
    out.extend(ungrounded_ratios(text, corpus, said));
    out.extend(unprinted_ids(text, corpus).into_iter().map(str::to_owned));
    out
}

/// How many times one file is held back before it is attached as it is: a
/// run that keeps finding new claims to make is not held in a loop.
pub const HOLDS_PER_PAGE: u32 = 3;

/// What one file was held back for.
#[derive(Debug, Default)]
struct Told {
    /// The claims the run was told about.
    claims: FxHashSet<String>,
    /// How many times it was held back.
    holds: u32,
}

/// Which of a turn's rendered pages to hold back, file by file.
///
/// A page is held back for a claim the run has not been told about for that
/// file; a render that keeps only claims it was told about is attached, so a
/// page whose figures are placeholders on purpose costs one render more,
/// never a loop. A file is held back at most [`HOLDS_PER_PAGE`] times.
///
/// # Examples
///
/// ```
/// let mut holds = figures_parser::PageHolds::default();
/// assert!(holds.hold("mockup.png", &["12%"]));
/// assert!(!holds.hold("mockup.png", &["12%"]));
/// assert!(holds.hold("mockup.png", &["12%", "40%"]));
/// assert!(holds.hold("other.png", &["12%"]));
/// let none: &[&str] = &[];
/// assert!(!holds.hold("other.png", none));
/// ```
#[derive(Debug, Default)]
pub struct PageHolds {
    /// Each file's claims and holds so far.
    files: FxHashMap<String, Told>,
}

impl PageHolds {
    /// Whether to hold back the page `filename` names, whose check found
    /// `claims`; a hold records them as told for that file.
    pub fn hold<S: AsRef<str>>(&mut self, filename: &str, claims: &[S]) -> bool {
        let told = self.files.get(filename);
        let spent = told.is_some_and(|t| t.holds >= HOLDS_PER_PAGE);
        let known = claims
            .iter()
            .all(|c| told.is_some_and(|t| t.claims.contains(c.as_ref())));
        if spent || known {
            return false;
        }
        let page = self.files.entry(filename.to_owned()).or_default();
        page.claims
            .extend(claims.iter().map(|c| c.as_ref().to_owned()));
        page.holds = page.holds.saturating_add(1);
        true
    }
}
