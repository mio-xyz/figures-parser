use figures_parser::id::Shape;
use figures_parser::{identifiers, ungrounded_identifiers, unseen_tokens};

fn shapes(s: &str) -> Vec<(&str, Shape)> {
    identifiers(s)
        .into_iter()
        .map(|i| (i.text, i.shape))
        .collect()
}

#[test]
fn reads_each_shape_and_leaves_links_and_addresses_to_their_kinds() {
    let text = "Run 9f2e4c1a, trace 3fa85f64-5717-4562-b3fc-2c963f66afa6, CORE-1710, cus_9aZ3kLm2, \
                req_01J9ZK3Q7X8 and https://acme.io/x/CORE-1711 for ana_b@acme.io";
    assert_eq!(
        shapes(text),
        [
            ("9f2e4c1a", Shape::Hex),
            ("3fa85f64-5717-4562-b3fc-2c963f66afa6", Shape::Uuid),
            ("CORE-1710", Shape::Key),
            ("cus_9aZ3kLm2", Shape::Prefixed),
            ("req_01J9ZK3Q7X8", Shape::Prefixed),
        ]
    );
}

#[test]
fn reads_no_identifier_in_words_numbers_dates_or_versions() {
    assert_eq!(
        shapes("deadbeef cafe 1234567 2026-11-05 2026-06-01T10:00:00Z v3.1 GPT-4o well-known"),
        Vec::<(&str, Shape)>::new()
    );
}

#[test]
fn flags_a_hash_or_uuid_no_result_gave_and_grounds_a_short_hash_by_its_full_one() {
    let corpus = "commit 9f2e4c1a7b3d4e5f8a6b0c1d2e3f4a5b, id 3fa85f64-5717-4562-b3fc-2c963f66afa6";
    assert_eq!(
        ungrounded_identifiers("Fixed in 9f2e4c1 and 1a2b3c4d.", corpus, ""),
        ["1a2b3c4d"]
    );
    assert_eq!(
        ungrounded_identifiers("Trace 3FA85F64-5717-4562-B3FC-2C963F66AFA6.", corpus, ""),
        Vec::<String>::new()
    );
}

#[test]
fn flags_a_tracker_key_only_beside_another_of_its_prefix() {
    let corpus = "CORE-1710 Spike voice playback";
    assert_eq!(
        ungrounded_identifiers("CORE-1710 and CORE-1711 are open.", corpus, ""),
        ["CORE-1711"]
    );
    assert_eq!(
        ungrounded_identifiers("Uses UTF-8 and GPT-4, see OPS-12.", corpus, ""),
        Vec::<String>::new()
    );
}

#[test]
fn flags_a_prefixed_id_only_beside_another_of_its_prefix() {
    let corpus = r#"{"customer": "cus_9aZ3kLm2"}"#;
    assert_eq!(
        ungrounded_identifiers("Charged cus_9aZ3kLm2 and cus_7qQ1xYz8.", corpus, ""),
        ["cus_7qQ1xYz8"]
    );
    assert_eq!(
        ungrounded_identifiers("Request req_01J9ZK3Q7X8 failed.", corpus, ""),
        Vec::<String>::new()
    );
}

#[test]
fn grounds_an_identifier_the_prompt_carried_and_reports_each_once() {
    assert_eq!(
        ungrounded_identifiers("1a2b3c4d, then 1A2B3C4D again", "", "pinned 1a2b3c4d"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_identifiers("1a2b3c4d, then 1A2B3C4D again", "", ""),
        ["1a2b3c4d"]
    );
}

#[test]
fn returns_long_mixed_tokens_apart_for_measurement() {
    assert_eq!(
        unseen_tokens(
            "Model claude-opus-5-5 on node ip-10-0-3-17",
            "node ip-10-0-3-17",
            ""
        ),
        ["claude-opus-5-5"]
    );
    assert_eq!(
        ungrounded_identifiers("Model claude-opus-5-5", "", ""),
        Vec::<String>::new()
    );
}

#[test]
fn a_run_against_a_letter_of_another_script_is_no_identifier() {
    assert_eq!(shapes("é9f2e4c1a 9f2e4c1aй"), Vec::<(&str, Shape)>::new());
    assert_eq!(
        shapes("«9f2e4c1a» —9f2e4c1a"),
        [("9f2e4c1a", Shape::Hex), ("9f2e4c1a", Shape::Hex)]
    );
}

#[test]
fn a_tracker_key_is_held_by_a_key_of_its_prefix_inside_a_corpus_link() {
    let corpus = "See https://acme.atlassian.net/browse/CORE-1710 for details";
    assert_eq!(
        ungrounded_identifiers("CORE-1711 is the follow-up.", corpus, ""),
        ["CORE-1711"]
    );
}
