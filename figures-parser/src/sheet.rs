//! What a reply owes the scripts it took a worksheet's amounts from: the
//! rows a checking helper listed beside a total, and the amounts a script
//! read off a sheet without such a helper.

use crate::number::{Amounts, states_figure};

/// A row a checking helper listed with an amount, for the reply to state
/// beside its total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListedFigure<'a> {
    /// The amount as the helper printed it ("19,851").
    pub figure: &'a str,
    /// The helper's line for the row, which says what the amount is.
    pub name: &'a str,
}

/// One script run of the turn, as [`sheet_findings`] reads it.
#[derive(Debug, Clone, Copy, Default)]
pub struct SheetRun<'a> {
    /// What the run printed.
    pub stdout: &'a str,
    /// Whether it called a helper that lists the rows its figures rest on
    /// (a checking or a reconciling helper).
    pub lists_its_rows: bool,
    /// Whether it called a helper that takes a sheet as a frame of rows.
    pub framing: bool,
    /// Whether it read a worksheet's cells itself.
    pub grid_read: bool,
    /// The rows a helper listed for the reply to name beside its total.
    pub listed: &'a [ListedFigure<'a>],
}

/// What a reply owes the runs it drew an amount from; [`sheet_findings`]
/// reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetFindings<'r, 'a> {
    /// Every row listed by a run the reply draws on, once per line.
    pub listed: Vec<ListedFigure<'a>>,
    /// The listed rows whose amount the reply does not state.
    pub unstated: Vec<ListedFigure<'a>>,
    /// The reply's amounts, as it writes them, that a run read off a sheet
    /// with no helper that lists the rows they rest on.
    pub unchecked: Vec<&'r str>,
    /// Whether a run that gave an unchecked amount took a frame of rows and
    /// summed it by hand, rather than reading the cells with no helper.
    pub framed: bool,
}

/// What `reply` owes the turn's script `runs`, read in one walk.
///
/// A run the reply states no amount of holds it to nothing. The rows a run
/// listed are owed once the reply states one of its amounts; an amount a run
/// that lists its rows printed is checked. An amount is unchecked when a run
/// printed it after taking a frame of rows (when some run of the turn read
/// a sheet's cells: a frame of a price series is no ledger), or when a run
/// that read the cells itself printed it with another of the reply's
/// amounts, since one amount alone may be one cell looked up.
///
/// # Examples
///
/// ```
/// use figures_parser::{ListedFigure, SheetRun, sheet_findings};
///
/// let listed = [ListedFigure { figure: "1,040", name: "Refund on 3 Oct, left out" }];
/// let runs = [
///     SheetRun { stdout: "total 19,851", lists_its_rows: true, listed: &listed, ..SheetRun::default() },
///     SheetRun { stdout: "sum 4,200 of 12 rows, 3,100 net", grid_read: true, ..SheetRun::default() },
/// ];
/// let findings = sheet_findings("Total 19,851; Q3 4,200 and 3,100 net.", &runs);
/// assert_eq!(findings.unstated, listed);
/// assert_eq!(findings.unchecked, ["4,200", "3,100"]);
/// assert!(!findings.framed);
/// ```
#[must_use]
pub fn sheet_findings<'r, 'a>(reply: &'r str, runs: &[SheetRun<'a>]) -> SheetFindings<'r, 'a> {
    let mut listed: Vec<ListedFigure<'a>> = Vec::new();
    let mut held: Vec<&'r str> = Vec::new();
    let mut checked: Vec<&'r str> = Vec::new();
    let mut framed = false;
    let ledger_read = runs.iter().any(|run| run.grid_read);
    let mut amounts: Option<Amounts<'r>> = None;
    for run in runs {
        let took = ledger_read && run.framing;
        if !run.lists_its_rows && !took && !run.grid_read && run.listed.is_empty() {
            continue;
        }
        let shared = amounts
            .get_or_insert_with(|| Amounts::of(reply))
            .shared_with(run.stdout);
        if shared.is_empty() {
            continue;
        }
        for row in run.listed {
            if !listed.iter().any(|seen| seen.name == row.name) {
                listed.push(*row);
            }
        }
        if run.lists_its_rows {
            push_new(&mut checked, &shared);
            continue;
        }
        if (!took && !run.grid_read) || shared.len() < if took { 1 } else { 2 } {
            continue;
        }
        framed |= took;
        push_new(&mut held, &shared);
    }
    let unchecked: Vec<&str> = held
        .into_iter()
        .filter(|figure| !checked.contains(figure))
        .collect();
    let unstated = listed
        .iter()
        .copied()
        .filter(|row| !states_figure(reply, row.figure))
        .collect();
    SheetFindings {
        listed,
        unstated,
        framed: framed && !unchecked.is_empty(),
        unchecked,
    }
}

/// Adds each of `items` that `out` does not hold yet.
fn push_new<'r>(out: &mut Vec<&'r str>, items: &[&'r str]) {
    for item in items {
        if !out.contains(item) {
            out.push(item);
        }
    }
}
