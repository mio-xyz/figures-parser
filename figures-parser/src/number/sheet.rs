//! What a reply owes the scripts it drew a worksheet's amounts from: the rows
//! a script listed beside a total, and the amounts a script took from a
//! worksheet without a check that lists the rows they count.

use super::ground::Amounts;
use super::ground::states_figure;

/// A row a script listed with the amount the reply states beside its total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFigure {
    /// The amount as the script printed it ("19,851").
    pub figure: String,
    /// The script's line for the row, which says what the amount is.
    pub name: String,
}

/// What one script run did with a worksheet, as the caller classifies it.
#[derive(Debug, Clone, Copy, Default)]
pub struct SheetRun<'a> {
    /// What the run printed.
    pub stdout: &'a str,
    /// It ran a check whose print lists the rows its figures rest on, or a
    /// reconciliation over two tables, which lists what it pairs and leaves.
    pub lists_rows: bool,
    /// It took a ledger frame from a worksheet.
    pub frames: bool,
    /// It read a worksheet's grid itself.
    pub grid_read: bool,
    /// The rows it listed with their amounts.
    pub listed: &'a [ListedFigure],
}

/// The amounts a reply took from a worksheet without the checks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UncheckedFigures {
    /// The amounts as the reply writes them.
    pub figures: Vec<String>,
    /// Whether a run took a ledger frame and summed it by hand, rather than
    /// reading the tab with no check at all.
    pub framed: bool,
}

/// What the reply owes the runs it drew an amount from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetFindings {
    /// Every row listed by a run the reply draws on, once per line.
    pub listed: Vec<ListedFigure>,
    /// The listed rows whose amount the reply leaves unstated.
    pub unstated: Vec<ListedFigure>,
    /// The amounts taken from a worksheet that no check listed.
    pub unchecked: UncheckedFigures,
}

/// What `reply` owes `runs`, the turn's script runs in order.
///
/// A run the reply shares no amount with ([`Amounts`]) holds it to nothing,
/// and one that touched no worksheet and listed no row is skipped before its
/// print is read. The rows a run listed are owed when the reply shares an
/// amount with its print. An amount shared with a run that lists its rows is
/// checked; one shared with a run that took a ledger frame (when some run
/// read a grid, since a frame over a price series is no ledger) or read a
/// grid itself is unchecked, a grid read by hand needing two shared amounts,
/// since it may be one cell looked up.
///
/// # Examples
///
/// ```
/// use figures_parser::{ListedFigure, SheetRun, sheet_findings};
///
/// let listed = [ListedFigure { figure: "19,851".into(), name: "row 6 TOTAL: 19,851 not counted".into() }];
/// let runs = [SheetRun { stdout: "total 19,850", lists_rows: true, listed: &listed, ..SheetRun::default() }];
/// let found = sheet_findings("2027 holds $19,850.", &runs);
/// assert_eq!(found.unstated, listed);
/// assert!(found.unchecked.figures.is_empty());
/// ```
#[must_use]
pub fn sheet_findings(reply: &str, runs: &[SheetRun<'_>]) -> SheetFindings {
    let amounts = Amounts::of(reply);
    let mut listed: Vec<ListedFigure> = Vec::new();
    let mut held: Vec<&str> = Vec::new();
    let mut checked: Vec<&str> = Vec::new();
    let mut framed = false;
    let ledger_read = runs.iter().any(|run| run.grid_read);
    for run in runs {
        let took = ledger_read && run.frames;
        if !run.lists_rows && !took && !run.grid_read && run.listed.is_empty() {
            continue;
        }
        let shared = amounts.shared_with(run.stdout);
        if shared.is_empty() {
            continue;
        }
        for row in run.listed {
            if !listed.iter().any(|l| l.name == row.name) {
                listed.push(row.clone());
            }
        }
        if run.lists_rows {
            for figure in shared {
                if !checked.contains(&figure) {
                    checked.push(figure);
                }
            }
            continue;
        }
        if (!took && !run.grid_read) || shared.len() < if took { 1 } else { 2 } {
            continue;
        }
        framed |= took;
        for figure in shared {
            if !held.contains(&figure) {
                held.push(figure);
            }
        }
    }
    let figures: Vec<String> = held
        .into_iter()
        .filter(|figure| !checked.contains(figure))
        .map(str::to_owned)
        .collect();
    let unstated = listed
        .iter()
        .filter(|row| !states_figure(reply, &row.figure))
        .cloned()
        .collect();
    SheetFindings {
        listed,
        unstated,
        unchecked: UncheckedFigures {
            framed: framed && !figures.is_empty(),
            figures,
        },
    }
}
