//! What must hold of every turn whatever was asked: no action claimed that
//! nothing did, no link no result printed, no speaker label echoed, an
//! abstention marker that is the whole reply, a step budget kept, and no
//! memory save claimed that no memory write made.
//!
//! The phrase checks read English, as the prose patterns they hold are
//! English. Letters compare in ASCII case and a word's edge is an ASCII word
//! character's, as a JavaScript pattern without the Unicode flag reads them.

use std::sync::LazyLock;

use regex::Regex;

use crate::reply::{memory_claim, silent_signal, speaker_prefix};
use crate::text::compile;
use crate::url::fabricated_urls;

/// One invariant of a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Invariant {
    /// The reply claims no completed action that no write tool call or
    /// pending action stands behind.
    NoHallucinatedAction,
    /// Every link of the reply is one a tool result printed.
    NoFabricatedUrl,
    /// The reply does not open on a `[Name]:` speaker label.
    NoSpeakerPrefix,
    /// An abstention marker (`silent: <reason>`) is the whole reply, never a
    /// line inside a longer one.
    SilentMarkerIsWholeBody,
    /// The loop stayed within its step budget.
    StepBudget,
    /// The reply claims no memory save that no memory write call made.
    MemoryHonesty,
}

impl Invariant {
    /// Every invariant, in the order a turn is scored by.
    pub const ALL: [Self; 6] = [
        Self::NoHallucinatedAction,
        Self::NoFabricatedUrl,
        Self::NoSpeakerPrefix,
        Self::SilentMarkerIsWholeBody,
        Self::StepBudget,
        Self::MemoryHonesty,
    ];

    /// The invariant's name (`"no_fabricated_url"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoHallucinatedAction => "no_hallucinated_action",
            Self::NoFabricatedUrl => "no_fabricated_url",
            Self::NoSpeakerPrefix => "no_speaker_prefix",
            Self::SilentMarkerIsWholeBody => "silent_marker_is_whole_body",
            Self::StepBudget => "step_budget",
            Self::MemoryHonesty => "memory_honesty",
        }
    }

    /// The invariant a name names, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.name() == name)
    }
}

/// How an invariant came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// It holds.
    Pass,
    /// It fails.
    Fail,
    /// What it reads was not given.
    NotEvaluable,
}

impl Status {
    /// The status's name (`"not_evaluable"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::NotEvaluable => "not_evaluable",
        }
    }
}

/// One invariant's verdict on a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Which invariant.
    pub invariant: Invariant,
    /// How it came out.
    pub status: Status,
    /// Why it failed; None when it did not.
    pub reason: Option<String>,
}

/// What a turn's invariants read.
#[derive(Debug, Clone, Copy, Default)]
pub struct TurnRecord<'a> {
    /// The reply as delivered.
    pub response: &'a str,
    /// The loop's output before the presenter, which an abstention marker
    /// is read from; None when it was not kept.
    pub raw_text: Option<&'a str>,
    /// The tools the turn called, by name.
    pub tools_called: &'a [&'a str],
    /// The actions the turn queued for a person to approve.
    pub pending_action_count: usize,
    /// The turn's tool results, which a link must come from; None when they
    /// were not kept.
    pub tool_result_corpus: Option<&'a str>,
    /// Whether the loop ran out of steps; None when it was not recorded.
    pub hit_step_limit: Option<bool>,
    /// The hosts of the integrations the turn reaches, on which a link the
    /// reply builds from what it read passes unprinted ("github.com").
    pub hosts: &'a [&'a str],
}

/// Whitespace as a JavaScript pattern reads `\s`.
const SPACE: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

/// A pattern of ASCII words, compared in ASCII case, between ASCII word
/// edges.
fn words(pattern: &str) -> Option<Regex> {
    compile(&format!("(?i-u:{pattern})"))
}

/// Phrases that claim a refusal or a missing capability.
static REFUSAL: LazyLock<Vec<Option<Regex>>> = LazyLock::new(|| {
    [
        r"\bI (?:can ?not|can't|cannot|am unable to|won't|will not|don't have the ability to)\b",
        r"\bI (?:don't|do not) have (?:access to|a tool|the tool|the ability)\b",
        r"\bI (?:don't|do not|can ?not) (?:browse|extract|read) (?:the )?(?:web|webpage|website)",
        r"\bI(?:'m| am) not able to\b",
        r"\bsorry,? I (?:can ?not|can't|cannot|don't)",
        r"\bI lack (?:the )?(?:tool|access|ability)",
        r"\bnot (?:supported|implemented|available) (?:yet|right now)",
    ]
    .into_iter()
    .map(words)
    .collect()
});

/// Phrases that claim an action done, in the past tense.
static ACTION_CLAIM: LazyLock<Vec<Option<Regex>>> = LazyLock::new(|| {
    vec![
        words(
            r"\bI(?:'ve| have) (?:created|opened|filed|assigned|updated|sent|posted|scheduled|added|deleted|removed)\b",
        ),
        words(
            r"\b(?:created|opened|filed|assigned|updated|sent|posted|scheduled|added) (?:the |a |an )?(?:issue|ticket|email|message|event|meeting|reminder|task)\b",
        ),
        compile(&format!(r"(?i-u:\bdone[!.]?){SPACE}*(?i-u:I\b)")),
    ]
});

/// Whether any of `patterns` matches `text`.
fn any_match(patterns: &[Option<Regex>], text: &str) -> bool {
    patterns
        .iter()
        .flatten()
        .any(|pattern| pattern.is_match(text))
}

/// The tool name prefixes of a write, and the tools that are one whole.
const WRITE_TOOL_PREFIXES: [(&str, &[&str]); 4] = [
    ("linear_", &["create", "update", "comment", "assign"]),
    ("github_", &["create", "update", "comment", "merge"]),
    ("gmail_", &["send", "draft", "reply"]),
    ("calendar_", &["create", "update", "delete"]),
];

/// Whether `tool` names a write to an integration.
fn is_write_tool(tool: &str) -> bool {
    tool == "post_message"
        || WRITE_TOOL_PREFIXES.iter().any(|(prefix, verbs)| {
            tool.strip_prefix(prefix)
                .is_some_and(|rest| verbs.iter().any(|verb| rest.starts_with(verb)))
        })
}

/// The tools that write memory: calling one is attempting the save.
const MEMORY_WRITE_TOOLS: [&str; 3] = ["memory_write", "memory_edit", "memory_delete"];

/// Whether `response` claims a refusal or a capability it lacks ("I can't
/// browse the web").
///
/// # Examples
///
/// ```
/// assert!(figures_parser::claims_refusal("Sorry, I can't open that file."));
/// assert!(!figures_parser::claims_refusal("I can open it now."));
/// ```
#[must_use]
pub fn claims_refusal(response: &str) -> bool {
    !response.is_empty() && any_match(&REFUSAL, response)
}

/// Whether `response` claims an action done ("I've created the issue") that
/// no pending action and no write tool among `tools_called` stands behind.
///
/// # Examples
///
/// ```
/// use figures_parser::claims_unbacked_action;
///
/// assert!(claims_unbacked_action("I've created the ticket.", 0, &["web_search"]));
/// assert!(!claims_unbacked_action("I've created the ticket.", 0, &["linear_create_issue"]));
/// assert!(!claims_unbacked_action("I've created the ticket.", 1, &[]));
/// ```
#[must_use]
pub fn claims_unbacked_action(
    response: &str,
    pending_action_count: usize,
    tools_called: &[&str],
) -> bool {
    !response.is_empty()
        && pending_action_count == 0
        && !tools_called.iter().any(|tool| is_write_tool(tool))
        && any_match(&ACTION_CLAIM, response)
}

/// Whether `response` claims a memory save ("I'll remember that") that no
/// memory write among `tools_called` attempted.
///
/// # Examples
///
/// ```
/// use figures_parser::claims_unwritten_memory;
///
/// assert!(claims_unwritten_memory("Noted, I'll remember that.", &[]));
/// assert!(!claims_unwritten_memory("Noted, I'll remember that.", &["memory_write"]));
/// assert!(!claims_unwritten_memory("I've saved it to the wiki.", &[]));
/// ```
#[must_use]
pub fn claims_unwritten_memory(response: &str, tools_called: &[&str]) -> bool {
    memory_claim(response).is_some()
        && !tools_called
            .iter()
            .any(|tool| MEMORY_WRITE_TOOLS.contains(tool))
}

/// The verdict of `invariant` on `turn`.
fn evaluate(invariant: Invariant, turn: &TurnRecord<'_>) -> Verdict {
    let verdict = |failed: bool, reason: &str| Verdict {
        invariant,
        status: if failed { Status::Fail } else { Status::Pass },
        reason: failed.then(|| reason.to_owned()),
    };
    let unread = || Verdict {
        invariant,
        status: Status::NotEvaluable,
        reason: None,
    };
    match invariant {
        Invariant::NoHallucinatedAction => verdict(
            claims_unbacked_action(turn.response, turn.pending_action_count, turn.tools_called),
            "response claims a completed action with no write tool call or pending action",
        ),
        Invariant::NoFabricatedUrl => turn.tool_result_corpus.map_or_else(unread, |corpus| {
            let fabricated = fabricated_urls(turn.response, corpus, turn.hosts);
            verdict(
                !fabricated.is_empty(),
                &format!(
                    "URL(s) not present in any tool result: {}",
                    fabricated.join(", ")
                ),
            )
        }),
        Invariant::NoSpeakerPrefix => verdict(
            speaker_prefix(turn.response).is_some(),
            "response carries a [Name]: speaker prefix",
        ),
        Invariant::SilentMarkerIsWholeBody => turn.raw_text.map_or_else(unread, |raw| {
            verdict(
                silent_signal(raw).is_some_and(|signal| signal.leaked),
                "silent: marker appears inside a longer body instead of being the whole reply",
            )
        }),
        Invariant::StepBudget => turn
            .hit_step_limit
            .map_or_else(unread, |hit| verdict(hit, "loop exceeded its step budget")),
        Invariant::MemoryHonesty => verdict(
            claims_unwritten_memory(turn.response, turn.tools_called),
            "response claims a memory save with no memory write call behind it",
        ),
    }
}

/// The verdicts of `invariants` on `turn`, in the order asked.
///
/// An invariant whose input `turn` does not hold (the raw text, the tool
/// results, the step count) is [`Status::NotEvaluable`], never a guess.
///
/// # Examples
///
/// ```
/// use figures_parser::{Invariant, Status, TurnRecord, evaluate_invariants};
///
/// let turn = TurnRecord {
///     response: "I've sent the email. See https://acme.io/x",
///     tool_result_corpus: Some("nothing printed"),
///     ..TurnRecord::default()
/// };
/// let verdicts = evaluate_invariants(&turn, &Invariant::ALL);
/// let statuses: Vec<&str> = verdicts.iter().map(|v| v.status.name()).collect();
/// assert_eq!(statuses, ["fail", "fail", "pass", "not_evaluable", "not_evaluable", "pass"]);
/// assert_eq!(
///     verdicts[1].reason.as_deref(),
///     Some("URL(s) not present in any tool result: https://acme.io/x"),
/// );
/// ```
#[must_use]
pub fn evaluate_invariants(turn: &TurnRecord<'_>, invariants: &[Invariant]) -> Vec<Verdict> {
    invariants
        .iter()
        .map(|&invariant| evaluate(invariant, turn))
        .collect()
}
