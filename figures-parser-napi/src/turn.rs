//! The checks that read a whole turn.
//!
//! An agent's own internals, its memory layout, the sheets its scripts read,
//! the memory a reply draws on, and the checks a turn runs on a reply, a sent
//! message and a page in one call.

use figures_parser::{InternalsCheck, ListedFigure, SheetRun};
use napi::bindgen_prelude::Function;
use napi_derive::napi;

use crate::parse_today;

/// The names a reply must not show unless a person wrote them.
#[napi(object, js_name = "Vocabulary")]
pub struct VocabularyInput {
    /// The tools and sandbox functions; one with an underscore counts as a
    /// word, one without only as a call.
    pub names: Vec<String>,
    /// Memory file names ("workspace.md"); a tool result in `vouching` that
    /// holds one vouches for it.
    pub files: Vec<String>,
    /// Directories ("/memory/"), counted only where they begin a path.
    pub roots: Vec<String>,
    /// Other names (programs, environment variables), counted anywhere.
    pub fixed: Vec<String>,
}

/// A name and the plain words it reads as.
#[napi(object)]
pub struct Phrase {
    /// A folder under the memory root, or a file name.
    pub name: String,
    /// What it reads as.
    pub phrase: String,
}

/// Where an agent keeps its memory, and the plain words each part reads as.
#[napi(object, js_name = "MemoryLayout")]
pub struct LayoutInput {
    /// The memory directory, with its trailing slash ("/memory/").
    pub root: String,
    /// The prefix every memory tool's name carries ("memory_").
    pub tool_prefix: String,
    /// The folders under the root, checked first.
    pub folders: Vec<Phrase>,
    /// The file names a path ends in, or written alone, in order.
    pub files: Vec<Phrase>,
    /// What the root alone reads as.
    pub root_phrase: String,
    /// What any other path under the root reads as.
    pub other_phrase: String,
    /// What a memory tool's name reads as.
    pub tool_phrase: String,
}

/// A row a checking helper listed with an amount.
#[napi(object, js_name = "ListedFigure")]
pub struct ListedInput {
    /// The amount as the helper printed it.
    pub figure: String,
    /// The helper's line for the row.
    pub name: String,
}

/// One script run of the turn, as the sheet checks read it.
#[napi(object, js_name = "SheetRun")]
pub struct RunInput {
    /// What the run printed.
    pub stdout: String,
    /// Whether it called a helper that lists the rows its figures rest on;
    /// false when absent.
    pub lists_its_rows: Option<bool>,
    /// Whether it called a helper that takes a sheet as a frame of rows;
    /// false when absent.
    pub framing: Option<bool>,
    /// Whether it read a worksheet's cells itself; false when absent.
    pub grid_read: Option<bool>,
    /// The rows a helper listed for the reply to name beside its total; none
    /// when absent.
    pub listed: Option<Vec<ListedInput>>,
}

/// The amounts a reply took from a worksheet without the helpers' checks.
#[napi(object)]
pub struct UncheckedFigures {
    /// The amounts, as the reply writes them.
    pub figures: Vec<String>,
    /// Whether a run that gave one took a frame of rows and summed it by hand.
    pub framed: bool,
}

/// What a reply owes the runs it drew an amount from.
#[napi(object)]
pub struct SheetFindings {
    /// Every row listed by a run the reply draws on, once per line.
    pub listed: Vec<ListedInput>,
    /// The listed rows whose amount the reply does not state.
    pub unstated: Vec<ListedInput>,
    /// The amounts read off a sheet with no helper that lists their rows.
    pub unchecked: UncheckedFigures,
}

/// How much of a reply rests on the preloaded memory.
#[napi(object)]
pub struct MemoryGrounding {
    /// The lines of content the notes held.
    pub preloaded_lines: u32,
    /// The lines the reply draws on.
    pub grounded_lines: u32,
    /// Whether the reply draws on any.
    pub grounded: bool,
}

/// What a text is checked against: the turn so far.
#[napi(object, js_name = "Turn")]
pub struct TurnInput {
    /// The thread, the request and the run's tool results.
    pub corpus: String,
    /// The system prompt, which grounds a link but never an example-domain
    /// one.
    pub prompt: Option<String>,
    /// What the people in the thread wrote.
    pub said: Option<String>,
    /// The turn's date, `YYYY-MM-DD`.
    pub today: String,
    /// Whether an attachment the turn read rides only as pixels.
    pub pixel_only: Option<bool>,
    /// The lowercased hosts the caller trusts a built link on.
    pub hosts: Option<Vec<String>>,
    /// Where the agent keeps its memory; a sent message's layout is put in
    /// plain words.
    pub layout: Option<LayoutInput>,
    /// The names of the agent's internals, when the caller checks them.
    pub vocabulary: Option<VocabularyInput>,
    /// The tool results other than the memory tools'.
    pub vouching: Option<String>,
    /// The turn's script runs, for the sheet checks.
    pub runs: Option<Vec<RunInput>>,
}

/// One thing a check found.
#[napi(object)]
pub struct Finding {
    /// What it is about: `link`, `placeholder`, `misdirected`, `unlinked`,
    /// `attached`, `figure`, `id`, `ratio`, `per-unit`, `unstated`,
    /// `unchecked`, `weekday`, `window` or `internal`.
    pub kind: String,
    /// What was found.
    pub value: String,
}

/// A text with its mendable defects put right, and what it still fails on.
#[napi(object)]
pub struct Grounding {
    /// Whether the text fails on nothing.
    pub ok: bool,
    /// The text, mended.
    pub text: String,
    /// Each link repair, as `from → to`.
    pub mends: Vec<String>,
    /// What the mended text fails on, in check order.
    pub findings: Vec<Finding>,
}

/// The strings of a list, borrowed.
fn strs(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

/// The pairs of a phrase list, borrowed.
fn pairs(list: &[Phrase]) -> Vec<(&str, &str)> {
    list.iter()
        .map(|p| (p.name.as_str(), p.phrase.as_str()))
        .collect()
}

/// The names of an agent's own internals `reply` shows that no person wrote
/// in `said`: code names (lowercased), memory files no tool result in
/// `vouching` holds whole, then roots and fixed names, as given.
#[napi(catch_unwind)]
#[must_use]
pub fn internal_names(
    reply: String,
    said: String,
    vocabulary: VocabularyInput,
    vouching: Option<String>,
) -> Vec<String> {
    let (names, files) = (strs(&vocabulary.names), strs(&vocabulary.files));
    let (roots, fixed) = (strs(&vocabulary.roots), strs(&vocabulary.fixed));
    let lists = figures_parser::Vocabulary {
        names: &names,
        files: &files,
        roots: &roots,
        fixed: &fixed,
    };
    figures_parser::internal_names(
        &reply,
        &said,
        &lists,
        vouching.as_deref().unwrap_or_default(),
    )
}

/// `reply` with each memory path, memory file named alone and memory tool
/// name the person did not write in `said` put in its plain words.
#[napi(catch_unwind)]
#[must_use]
pub fn scrub_memory_layout(reply: String, said: String, layout: LayoutInput) -> String {
    let (folders, files) = (pairs(&layout.folders), pairs(&layout.files));
    let memory = figures_parser::MemoryLayout {
        root: &layout.root,
        tool_prefix: &layout.tool_prefix,
        folders: &folders,
        files: &files,
        root_phrase: &layout.root_phrase,
        other_phrase: &layout.other_phrase,
        tool_phrase: &layout.tool_phrase,
    };
    figures_parser::scrub_memory_layout(&reply, &said, &memory).into_owned()
}

/// The distinctive words of `text`, lowercased: no freshness marker,
/// markdown mark, stopword or short token.
#[napi(catch_unwind)]
#[must_use]
pub fn grounding_tokens(text: String) -> Vec<String> {
    figures_parser::grounding_tokens(&text)
}

/// Which lines of the preloaded `notes` `response` visibly draws on.
#[napi(catch_unwind)]
#[must_use]
pub fn memory_grounding(response: String, notes: Vec<String>) -> MemoryGrounding {
    let found = figures_parser::memory_grounding(&response, &notes);
    MemoryGrounding {
        preloaded_lines: u32::try_from(found.preloaded_lines).unwrap_or(u32::MAX),
        grounded_lines: u32::try_from(found.grounded_lines).unwrap_or(u32::MAX),
        grounded: found.grounded,
    }
}

/// The rows of each run, borrowed.
fn listed_rows(runs: &[RunInput]) -> Vec<Vec<ListedFigure<'_>>> {
    runs.iter()
        .map(|run| {
            run.listed
                .iter()
                .flatten()
                .map(|row| ListedFigure {
                    figure: &row.figure,
                    name: &row.name,
                })
                .collect()
        })
        .collect()
}

/// The runs, borrowed, each with its rows.
fn sheet_runs<'a>(runs: &'a [RunInput], rows: &'a [Vec<ListedFigure<'a>>]) -> Vec<SheetRun<'a>> {
    runs.iter()
        .zip(rows)
        .map(|(run, listed)| SheetRun {
            stdout: &run.stdout,
            lists_its_rows: run.lists_its_rows.unwrap_or(false),
            framing: run.framing.unwrap_or(false),
            grid_read: run.grid_read.unwrap_or(false),
            listed,
        })
        .collect()
}

/// A listed row, owned.
fn listed_output(row: &ListedFigure<'_>) -> ListedInput {
    ListedInput {
        figure: row.figure.to_owned(),
        name: row.name.to_owned(),
    }
}

/// What `reply` owes the turn's script `runs`: the listed rows it leaves
/// unstated, and the amounts it took off a sheet without the helpers that
/// list their rows.
#[napi(catch_unwind)]
#[must_use]
pub fn sheet_findings(reply: String, runs: Vec<RunInput>) -> SheetFindings {
    let rows = listed_rows(&runs);
    let runs = sheet_runs(&runs, &rows);
    let found = figures_parser::sheet_findings(&reply, &runs);
    SheetFindings {
        listed: found.listed.iter().map(listed_output).collect(),
        unstated: found.unstated.iter().map(listed_output).collect(),
        unchecked: UncheckedFigures {
            figures: found.unchecked.into_iter().map(str::to_owned).collect(),
            framed: found.framed,
        },
    }
}

/// Which text a turn checks.
#[derive(Clone, Copy)]
enum Surface {
    /// The reply, before it is delivered.
    Reply,
    /// A message the run sends itself.
    Message,
}

/// `text` checked as `surface`, against `turn`.
fn ground(
    text: &str,
    turn: &TurnInput,
    attached: Option<&Function<String, bool>>,
    surface: Surface,
) -> napi::Result<Grounding> {
    let today = parse_today(&turn.today)?;
    let hosts = strs(turn.hosts.as_deref().unwrap_or_default());
    let (folders, files) = turn.layout.as_ref().map_or_else(
        || (Vec::new(), Vec::new()),
        |l| (pairs(&l.folders), pairs(&l.files)),
    );
    let layout = turn
        .layout
        .as_ref()
        .map_or_else(figures_parser::MemoryLayout::default, |l| {
            figures_parser::MemoryLayout {
                root: &l.root,
                tool_prefix: &l.tool_prefix,
                folders: &folders,
                files: &files,
                root_phrase: &l.root_phrase,
                other_phrase: &l.other_phrase,
                tool_phrase: &l.tool_phrase,
            }
        });
    let lists = turn.vocabulary.as_ref().map(|v| {
        (
            strs(&v.names),
            strs(&v.files),
            strs(&v.roots),
            strs(&v.fixed),
        )
    });
    let vocabulary =
        lists.as_ref().map(
            |(names, file_names, roots, fixed)| figures_parser::Vocabulary {
                names,
                files: file_names,
                roots,
                fixed,
            },
        );
    let said = turn.said.as_deref().unwrap_or_default();
    let internals = vocabulary.as_ref().map(|v| InternalsCheck::new(said, v));
    let runs = turn.runs.as_deref().unwrap_or_default();
    let rows = listed_rows(runs);
    let runs = sheet_runs(runs, &rows);
    let is_attached =
        |target: &str| attached.is_some_and(|f| f.call(target.to_owned()).unwrap_or(false));
    let check = figures_parser::Turn {
        corpus: &turn.corpus,
        prompt: turn.prompt.as_deref().unwrap_or_default(),
        said,
        today,
        pixel_only: turn.pixel_only.unwrap_or(false),
        hosts: &hosts,
        attached: &is_attached,
        layout: &layout,
        internals: internals.as_ref(),
        vouching: turn.vouching.as_deref().unwrap_or_default(),
        runs: &runs,
    };
    let found = match surface {
        Surface::Reply => figures_parser::ground_reply(text, &check),
        Surface::Message => figures_parser::ground_message(text, &check),
    };
    Ok(Grounding {
        ok: found.is_clean(),
        text: found.text,
        mends: found.mends,
        findings: found
            .findings
            .into_iter()
            .map(|f| Finding {
                kind: f.kind.name().to_owned(),
                value: f.value,
            })
            .collect(),
    })
}

/// A message the run sends itself, checked as the reply's round checks a
/// reply.
///
/// Its links are mended, its memory layout put in plain words, then it is
/// held to the turn so far. `attached` says whether a link target names a file
/// the run made.
///
/// # Errors
///
/// `turn.today` is not a `YYYY-MM-DD` date.
#[napi(catch_unwind)]
pub fn ground_message(
    text: String,
    turn: TurnInput,
    attached: Option<Function<String, bool>>,
) -> napi::Result<Grounding> {
    ground(&text, &turn, attached.as_ref(), Surface::Message)
}

/// A reply before it is delivered, checked as the correction round checks
/// it: its links mended, then held to the turn so far, a link to a file the
/// run made and a listed sheet row left unstated included.
///
/// # Errors
///
/// `turn.today` is not a `YYYY-MM-DD` date.
#[napi(catch_unwind)]
pub fn ground_reply(
    text: String,
    turn: TurnInput,
    attached: Option<Function<String, bool>>,
) -> napi::Result<Grounding> {
    ground(&text, &turn, attached.as_ref(), Surface::Reply)
}

/// The figures, ratios and ids of a page made for a person that `corpus`
/// does not carry; none when the turn read pixels only.
#[napi(catch_unwind)]
#[must_use]
pub fn ground_page(
    text: String,
    corpus: String,
    said: Option<String>,
    pixel_only: Option<bool>,
) -> Vec<String> {
    figures_parser::ground_page(
        &text,
        &corpus,
        said.as_deref().unwrap_or_default(),
        pixel_only.unwrap_or(false),
    )
}
