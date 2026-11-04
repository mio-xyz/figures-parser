//! The invariants every turn is held to.

use figures_parser::{
    Invariant, SilentSignal, Status, TurnRecord, claims_refusal, claims_unbacked_action,
    claims_unwritten_memory, evaluate_invariants, silent_signal, speaker_prefix,
};

fn statuses(turn: &TurnRecord<'_>) -> Vec<(&'static str, &'static str)> {
    evaluate_invariants(turn, &Invariant::ALL)
        .iter()
        .map(|v| (v.invariant.name(), v.status.name()))
        .collect()
}

// A link the reply builds on an integration's host passes, as the link
// check lets it; a caller that names no host holds it to the corpus.
#[test]
fn passes_a_link_built_on_a_host_the_turn_reaches() {
    for link in [
        "https://github.com/new",
        "https://linear.app/",
        "https://www.notion.so/",
        "https://github.com/settings/security",
        "https://acme.notion.site/",
    ] {
        let response = format!("See {link} to start.");
        let turn = TurnRecord {
            response: &response,
            tool_result_corpus: Some("nothing here"),
            hosts: &["github.com", "linear.app", "notion.so", "notion.site"],
            ..TurnRecord::default()
        };
        let verdicts = evaluate_invariants(&turn, &[Invariant::NoFabricatedUrl]);
        assert_eq!(verdicts[0].status, Status::Pass, "{link}");
        let unhosted = TurnRecord { hosts: &[], ..turn };
        let held = evaluate_invariants(&unhosted, &[Invariant::NoFabricatedUrl]);
        assert_eq!(held[0].status, Status::Fail, "{link}");
    }
}

#[test]
fn reads_what_it_is_given_and_leaves_the_rest_unread() {
    let turn = TurnRecord {
        response: "Here is the summary.",
        ..TurnRecord::default()
    };
    assert_eq!(
        statuses(&turn),
        [
            ("no_hallucinated_action", "pass"),
            ("no_fabricated_url", "not_evaluable"),
            ("no_speaker_prefix", "pass"),
            ("silent_marker_is_whole_body", "not_evaluable"),
            ("step_budget", "not_evaluable"),
            ("memory_honesty", "pass"),
        ]
    );
    let stopped = TurnRecord {
        hit_step_limit: Some(true),
        raw_text: Some("silent: nothing_new"),
        ..turn
    };
    let verdicts = evaluate_invariants(
        &stopped,
        &[Invariant::StepBudget, Invariant::SilentMarkerIsWholeBody],
    );
    assert_eq!(verdicts[0].status, Status::Fail);
    assert_eq!(
        verdicts[0].reason.as_deref(),
        Some("loop exceeded its step budget")
    );
    assert_eq!(verdicts[1].status, Status::Pass);
    assert_eq!(
        Invariant::from_name("memory_honesty"),
        Some(Invariant::MemoryHonesty)
    );
}

#[test]
fn holds_an_action_claim_to_a_write_or_a_pending_action() {
    for response in [
        "I've created the issue in Linear.",
        "Created the ticket for you.",
        "Done! I moved it.",
        "done.\u{a0}I added it",
        "I HAVE SENT it",
    ] {
        assert!(
            claims_unbacked_action(response, 0, &["web_search"]),
            "{response}"
        );
    }
    for response in [
        "I created it",
        "I've createdx",
        "xI've created",
        "Done -- I",
        "",
    ] {
        assert!(!claims_unbacked_action(response, 0, &[]), "{response}");
    }
    let claim = "I've sent the email.";
    assert!(!claims_unbacked_action(claim, 0, &["gmail_send_email"]));
    assert!(!claims_unbacked_action(claim, 0, &["post_message"]));
    assert!(claims_unbacked_action(
        claim,
        0,
        &["post_message_x", "gmail_read"]
    ));
    assert!(!claims_unbacked_action(claim, 2, &[]));
    // Letters compare in ASCII case: the Kelvin sign and the long s are not k and s.
    assert!(!claims_unbacked_action("I've \u{17f}ent it", 0, &[]));
}

#[test]
fn holds_a_memory_claim_to_a_memory_write() {
    for response in [
        "I'll remember that.",
        "I have noted it",
        "Committed this to memory.",
        "Saved that to your preferences",
        "updated it in the dossier",
    ] {
        assert!(claims_unwritten_memory(response, &[]), "{response}");
        assert!(
            !claims_unwritten_memory(response, &["memory_edit"]),
            "{response}"
        );
    }
    for response in [
        "I'll keep it in mind",
        "I've saved it to Notion",
        "xI'll remember",
    ] {
        assert!(!claims_unwritten_memory(response, &[]), "{response}");
    }
}

#[test]
fn reads_a_speaker_label_on_the_first_line_only() {
    assert!(speaker_prefix("  [Ana Lopez]: sure").is_some());
    assert!(speaker_prefix("[😀]:\tok").is_some());
    for response in [
        "[]: x",
        "[Ana]:x",
        "text\n[Ana]: x",
        "[A\nna]: x",
        "[ref]: ",
    ] {
        assert_eq!(
            speaker_prefix(response).is_some(),
            response == "[ref]: ",
            "{response}"
        );
    }
    let long = format!("[{}]: x", "a".repeat(81));
    assert!(speaker_prefix(&long).is_none());
    // The label is counted in UTF-16 units: forty emoji are eighty.
    assert!(speaker_prefix(&format!("[{}]: x", "😀".repeat(40))).is_some());
    assert!(speaker_prefix(&format!("[{}]: x", "😀".repeat(41))).is_none());
}

#[test]
fn tells_a_whole_abstention_from_a_leaked_one() {
    let whole = |reason| {
        Some(SilentSignal {
            reason,
            leaked: false,
        })
    };
    let leaked = |reason| {
        Some(SilentSignal {
            reason,
            leaked: true,
        })
    };
    assert_eq!(
        silent_signal("  silent:   no_change \n"),
        whole("no_change")
    );
    assert_eq!(silent_signal("[CURRENT MESSAGE] silent: x"), whole("x"));
    assert_eq!(silent_signal("silent:\nno_change"), whole("no_change"));
    assert_eq!(
        silent_signal("Draft below.\nsilent: no_change  \nmore"),
        leaked("no_change")
    );
    assert_eq!(silent_signal("a\u{2028}silent: x"), leaked("x"));
    assert_eq!(silent_signal("Draft\nsilent:\n\nx\n"), leaked("x"));
    for text in [
        "silent: two words",
        "Keep silent: x",
        "Silent: x",
        "silent:",
        "a silent: x",
    ] {
        assert_eq!(silent_signal(text), None, "{text}");
    }
}

#[test]
fn reads_a_refusal_or_a_missing_capability() {
    for response in [
        "I cannot do that",
        "I don't have access to your calendar",
        "I can not browse the web",
        "Sorry I don't know",
        "That is not supported yet",
    ] {
        assert!(claims_refusal(response), "{response}");
    }
    for response in ["I can do that", "", "I cannotx"] {
        assert!(!claims_refusal(response), "{response}");
    }
}
