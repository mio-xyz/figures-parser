//! The invariants every turn is held to.

use figures_parser::{Invariant, TurnRecord};
use napi_derive::napi;

/// What a turn's invariants read.
#[napi(object, js_name = "InvariantInput")]
pub struct InvariantInput {
    /// The reply as delivered.
    pub response: String,
    /// The loop's output before the presenter; absent when it was not kept.
    pub raw_text: Option<String>,
    /// The tools the turn called, by name.
    pub tools_called: Vec<String>,
    /// The actions the turn queued for a person to approve.
    pub pending_action_count: u32,
    /// The turn's tool results; absent when they were not kept.
    pub tool_result_corpus: Option<String>,
    /// Whether the loop ran out of steps; absent when not recorded.
    pub hit_step_limit: Option<bool>,
}

/// One invariant's verdict on a turn.
#[napi(object)]
pub struct InvariantVerdict {
    /// The invariant's name.
    #[napi(
        ts_type = "\"no_hallucinated_action\" | \"no_fabricated_url\" | \"no_speaker_prefix\" | \"silent_marker_is_whole_body\" | \"step_budget\" | \"memory_honesty\""
    )]
    pub name: String,
    /// How it came out.
    #[napi(ts_type = "\"pass\" | \"fail\" | \"not_evaluable\"")]
    pub status: String,
    /// Why it failed; absent when it did not.
    pub reason: Option<String>,
}

/// The strings of a list, borrowed.
fn strs(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

/// The verdicts of the invariants named in `names` (all of them when
/// absent, in order) on `input`; an unknown name is skipped.
#[napi(catch_unwind)]
#[must_use]
pub fn evaluate_invariants(
    input: InvariantInput,
    names: Option<Vec<String>>,
) -> Vec<InvariantVerdict> {
    let invariants: Vec<Invariant> = names.map_or_else(
        || Invariant::ALL.to_vec(),
        |names| {
            names
                .iter()
                .filter_map(|n| Invariant::from_name(n))
                .collect()
        },
    );
    let tools = strs(&input.tools_called);
    let turn = TurnRecord {
        response: &input.response,
        raw_text: input.raw_text.as_deref(),
        tools_called: &tools,
        pending_action_count: usize::try_from(input.pending_action_count).unwrap_or(usize::MAX),
        tool_result_corpus: input.tool_result_corpus.as_deref(),
        hit_step_limit: input.hit_step_limit,
    };
    figures_parser::evaluate_invariants(&turn, &invariants)
        .into_iter()
        .map(|v| InvariantVerdict {
            name: v.invariant.name().to_owned(),
            status: v.status.name().to_owned(),
            reason: v.reason,
        })
        .collect()
}

/// Whether `response` claims a refusal or a capability it lacks.
#[napi(catch_unwind)]
#[must_use]
pub fn claims_refusal(response: String) -> bool {
    figures_parser::claims_refusal(&response)
}

/// Whether `response` claims an action done that no pending action and no
/// write tool among `tools_called` stands behind.
#[napi(catch_unwind)]
#[must_use]
pub fn claims_unbacked_action(
    response: String,
    pending_action_count: u32,
    tools_called: Vec<String>,
) -> bool {
    figures_parser::claims_unbacked_action(
        &response,
        usize::try_from(pending_action_count).unwrap_or(usize::MAX),
        &strs(&tools_called),
    )
}

/// Whether `response` claims a memory save that no memory write among
/// `tools_called` attempted.
#[napi(catch_unwind)]
#[must_use]
pub fn claims_unwritten_memory(response: String, tools_called: Vec<String>) -> bool {
    figures_parser::claims_unwritten_memory(&response, &strs(&tools_called))
}
