//! Every check of the library run on one text the way a turn runs them: a
//! reply before it is delivered, a message sent mid-run, a page made for a
//! person.

use std::borrow::Cow;

use crate::internals::{InternalsCheck, MemoryLayout, scrub_memory_layout};
use crate::sheet::{SheetRun, sheet_findings};
use crate::{
    Date, link_findings, mended_links, misdated_weekdays, mispaired_per_unit, rows_outside_window,
    ungrounded_figures, ungrounded_ratios, unprinted_ids,
};

/// What a finding is about; [`Finding::kind`] holds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindingKind {
    /// A link no source printed.
    Link,
    /// A link target that is not a URL, as `(target)`.
    Placeholder,
    /// An item's label on a link that is not the item's own.
    Misdirected,
    /// An item named without the link a source gave it.
    Unlinked,
    /// A link target naming a file the run made, which reaches the thread
    /// as an attachment of its own; only a reply is checked for one.
    Attached,
    /// A figure no source carries.
    Figure,
    /// An opaque id no source printed.
    Id,
    /// A ratio no source or the person's numbers support.
    Ratio,
    /// A per-unit amount beside a total and a count that do not give it.
    PerUnit,
    /// A sheet row's amount a run listed beside a total the reply states,
    /// left unstated; only a reply is checked for one.
    Unstated,
    /// An amount read off a sheet without the helpers that list its rows.
    Unchecked,
    /// A date written with the wrong weekday.
    Weekday,
    /// A row dated outside the window its heading names.
    Window,
    /// A name of the agent's own internals.
    Internal,
}

impl FindingKind {
    /// The kind's name, the key a caller tells findings apart by
    /// (`"per-unit"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Placeholder => "placeholder",
            Self::Misdirected => "misdirected",
            Self::Unlinked => "unlinked",
            Self::Attached => "attached",
            Self::Figure => "figure",
            Self::Id => "id",
            Self::Ratio => "ratio",
            Self::PerUnit => "per-unit",
            Self::Unstated => "unstated",
            Self::Unchecked => "unchecked",
            Self::Weekday => "weekday",
            Self::Window => "window",
            Self::Internal => "internal",
        }
    }
}

/// One thing a check found, as the text wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// What it is about.
    pub kind: FindingKind,
    /// What was found, as the check names it.
    pub value: String,
}

impl Finding {
    /// The finding as `kind:value`, a key that tells a claim already
    /// refused from a new one.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind.name(), self.value)
    }
}

/// What a text is checked against: the turn so far.
#[derive(Clone, Copy)]
pub struct Turn<'a> {
    /// What a link, figure or id may rest on: the thread, the request, the
    /// run's tool results.
    pub corpus: &'a str,
    /// The system prompt, which grounds a link but never an example-domain
    /// one.
    pub prompt: &'a str,
    /// What the people in the thread wrote: it grounds a ratio of their own
    /// numbers, and may name what a text otherwise would not.
    pub said: &'a str,
    /// The turn's date, which a weekday or a window is read against.
    pub today: Date,
    /// Whether an attachment the turn read rides only as pixels: a figure
    /// read off one is in no text, so figures, ratios, per-unit amounts and
    /// ids go unchecked.
    pub pixel_only: bool,
    /// The hosts the caller trusts a built link on, as [`link_findings`]
    /// reads them.
    pub hosts: &'a [&'a str],
    /// Whether a link target names a file the run made.
    pub attached: &'a dyn Fn(&str) -> bool,
    /// Where the agent keeps its memory, whose layout a sent message never
    /// shows.
    pub layout: &'a MemoryLayout<'a>,
    /// The turn's internals check, if the caller runs one.
    pub internals: Option<&'a InternalsCheck<'a>>,
    /// The tool results that vouch for a memory file's name: all but the
    /// memory tools'.
    pub vouching: &'a str,
    /// The turn's script runs, for the sheet checks.
    pub runs: &'a [SheetRun<'a>],
}

/// A text with its mendable defects put right, and what it still fails on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grounding {
    /// The text, mended: the one to deliver or to check again.
    pub text: String,
    /// Each link repair, as `from → to`.
    pub mends: Vec<String>,
    /// What the mended text fails on, by kind, in the order of
    /// [`FindingKind`].
    pub findings: Vec<Finding>,
}

impl Grounding {
    /// Whether the text fails on nothing.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Which text a turn is checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    /// The reply, before it is delivered.
    Reply,
    /// A message the run sends itself.
    Message,
}

/// Adds each of `values` to `out` as a finding of `kind`.
fn push<S: Into<String>>(
    out: &mut Vec<Finding>,
    kind: FindingKind,
    values: impl IntoIterator<Item = S>,
) {
    out.extend(values.into_iter().map(|value| Finding {
        kind,
        value: value.into(),
    }));
}

/// The checks of `surface` on `text`, already mended.
fn findings(text: &str, turn: &Turn<'_>, surface: Surface) -> Vec<Finding> {
    let mut out = Vec::new();
    let links = link_findings(text, turn.corpus, turn.prompt, turn.hosts);
    push(&mut out, FindingKind::Link, links.unprinted);
    let (attached, placeholders): (Vec<&str>, Vec<&str>) = links
        .placeholders
        .into_iter()
        .partition(|target| (turn.attached)(target));
    push(
        &mut out,
        FindingKind::Placeholder,
        placeholders.iter().map(|target| format!("({target})")),
    );
    push(&mut out, FindingKind::Misdirected, links.misdirected);
    push(&mut out, FindingKind::Unlinked, links.unlinked);
    if surface == Surface::Reply {
        let mut once: Vec<&str> = Vec::new();
        for target in attached {
            if !once.contains(&target) {
                once.push(target);
            }
        }
        push(&mut out, FindingKind::Attached, once);
    }
    if !turn.pixel_only {
        push(
            &mut out,
            FindingKind::Figure,
            ungrounded_figures(text, turn.corpus),
        );
        push(&mut out, FindingKind::Id, unprinted_ids(text, turn.corpus));
        push(
            &mut out,
            FindingKind::Ratio,
            ungrounded_ratios(text, turn.corpus, turn.said),
        );
        push(
            &mut out,
            FindingKind::PerUnit,
            mispaired_per_unit(text, turn.corpus)
                .into_iter()
                .map(|m| m.per_unit),
        );
    }
    let sheets = sheet_findings(text, turn.runs);
    if surface == Surface::Reply {
        push(
            &mut out,
            FindingKind::Unstated,
            sheets.unstated.iter().map(|row| row.figure),
        );
    }
    push(&mut out, FindingKind::Unchecked, sheets.unchecked);
    push(
        &mut out,
        FindingKind::Weekday,
        misdated_weekdays(text, turn.today),
    );
    push(
        &mut out,
        FindingKind::Window,
        rows_outside_window(text, turn.today),
    );
    if let Some(check) = turn.internals {
        push(
            &mut out,
            FindingKind::Internal,
            check.names_in(text, || turn.vouching),
        );
    }
    out
}

/// A message the run sends itself, checked as a reply's round checks the
/// reply: its links mended, its memory layout put in plain words, then held
/// to the turn so far.
///
/// It fails on a link no source printed, a link target that is not a URL
/// (one naming a file the run made passes, since a send may attach the file
/// itself), a link labelled with another item's id or an item left without
/// the link a source gave it; a figure, a ratio or an id no source carried
/// and a per-unit amount beside a total and a count that do not give it
/// (unless the turn read pixels only); an amount read off a sheet without
/// the helpers that list its rows; a date with the wrong weekday or a row
/// outside its heading's window; a name of the agent's internals.
///
/// # Examples
///
/// ```
/// use figures_parser::{Date, MemoryLayout, Turn, ground_message};
///
/// let turn = Turn {
///     corpus: "revenue 194,718\ncos 0.523",
///     prompt: "",
///     said: "",
///     today: Date::new(2026, 10, 20).unwrap(),
///     pixel_only: false,
///     hosts: &[],
///     attached: &|_| false,
///     layout: &MemoryLayout::default(),
///     internals: None,
///     vouching: "",
///     runs: &[],
/// };
/// let keys: Vec<String> = ground_message("COS 61,0% on revenue of $188,027", &turn)
///     .findings
///     .iter()
///     .map(|f| f.key())
///     .collect();
/// assert_eq!(keys, ["figure:188,027", "ratio:61,0%"]);
/// assert!(ground_message("COS 52,3% on revenue of $194,718", &turn).is_clean());
/// ```
#[must_use]
pub fn ground_message(text: &str, turn: &Turn<'_>) -> Grounding {
    let linked = mended_links(text, turn.corpus);
    let mended = match scrub_memory_layout(&linked.text, turn.said, turn.layout) {
        Cow::Borrowed(_) => linked.text,
        Cow::Owned(scrubbed) => scrubbed,
    };
    let findings = findings(&mended, turn, Surface::Message);
    Grounding {
        text: mended,
        mends: linked.mends,
        findings,
    }
}

/// A reply before it is delivered, checked as the turn's correction round
/// checks it: its links mended, then held to the turn so far.
///
/// It fails on what [`ground_message`] fails on, and on a link to a file the
/// run made (the file reaches the thread beside the reply, so the link
/// renders dead) and a sheet row a run listed beside a total the reply
/// states, left unstated. The memory layout is not put in plain words here:
/// a reply naming it is asked for a rewrite, and the delivered text is
/// scrubbed with [`scrub_memory_layout`] after the round.
///
/// # Examples
///
/// ```
/// use figures_parser::{Date, MemoryLayout, Turn, ground_reply};
///
/// let turn = Turn {
///     corpus: "chart.png written",
///     prompt: "",
///     said: "",
///     today: Date::new(2026, 10, 20).unwrap(),
///     pixel_only: false,
///     hosts: &[],
///     attached: &|target| target == "chart.png",
///     layout: &MemoryLayout::default(),
///     internals: None,
///     vouching: "",
///     runs: &[],
/// };
/// let found = ground_reply("Here: ![chart](chart.png)", &turn);
/// assert_eq!(found.findings.iter().map(|f| f.key()).collect::<Vec<_>>(), ["attached:chart.png"]);
/// ```
#[must_use]
pub fn ground_reply(text: &str, turn: &Turn<'_>) -> Grounding {
    let linked = mended_links(text, turn.corpus);
    let findings = findings(&linked.text, turn, Surface::Reply);
    Grounding {
        text: linked.text,
        mends: linked.mends,
        findings,
    }
}

/// The figures, ratios and ids of a page made for a person that `corpus`
/// does not carry.
///
/// A page is held to what was read, as a sent message is, while its links, dates and wording are its design's. Nothing is checked
/// when the turn read pixels only (`pixel_only`).
///
/// # Examples
///
/// ```
/// let page = "<h1>Q3</h1><p>Revenue $1,240,000, 14% up, order prod_Zq81LmN07tYwPa</p>";
/// assert_eq!(
///     figures_parser::ground_page(page, "revenue: 1240000", "", false),
///     ["14%", "prod_Zq81LmN07tYwPa"],
/// );
/// ```
#[must_use]
pub fn ground_page(text: &str, corpus: &str, said: &str, pixel_only: bool) -> Vec<String> {
    if pixel_only {
        return Vec::new();
    }
    let mut out = ungrounded_figures(text, corpus);
    out.extend(ungrounded_ratios(text, corpus, said));
    out.extend(unprinted_ids(text, corpus).into_iter().map(str::to_owned));
    out
}
