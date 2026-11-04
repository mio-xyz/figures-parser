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

#[test]
fn an_opaque_id_no_result_printed_is_unprinted_and_one_a_result_printed_is_not() {
    use figures_parser::unprinted_ids;
    let created = r#"{"id": "prod_TQ81xLmN20aB7c", "name": "Annual"}"#;
    assert_eq!(
        unprinted_ids(
            "Created prod_TQ81xLmN20aB7c and prod_VU49BpM22sWxIM.",
            created
        ),
        ["prod_VU49BpM22sWxIM"]
    );
    let failed = "Error: StripeInvalidRequestError: No such price: 'price_1Q'";
    assert_eq!(
        unprinted_ids(
            "The products were created: `prod_VU49BpM22sWxIM`, `prod_VU49Bq7sLx02Tn`.",
            failed
        ),
        ["prod_VU49BpM22sWxIM", "prod_VU49Bq7sLx02Tn"]
    );
}

#[test]
fn a_uuid_and_a_digest_are_read_and_grounded_whole_or_shortened() {
    use figures_parser::unprinted_ids;
    let read =
        "trace 7f3a9c0b2e8d4f61a5b3c9d0e1f2a3b4 sha 3080fc1335ab9e7c0d2f4a6b8c1e3f5a7b9d0c2e";
    assert_eq!(
        unprinted_ids(
            "Trace 7f3a9c0b2e8d4f61a5b3c9d0e1f2a3b4, commit 3080fc133.",
            read
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids(
            "Trace 7f3a9c0b2e8d4f61, run 0b6c2d4e-1f3a-4b5c-8d9e-0a1b2c3d4e5f.",
            read
        ),
        ["0b6c2d4e-1f3a-4b5c-8d9e-0a1b2c3d4e5f"]
    );
    assert_eq!(
        unprinted_ids("Trace A1B2C3D4E5F60718.", ""),
        ["A1B2C3D4E5F60718"]
    );
    assert_eq!(
        unprinted_ids("Trace A1B2C3D4E5F60718.", "trace a1b2c3d4e5f60718"),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids(
            "Run 0B6C2D4E-1F3A-4B5C-8D9E-0A1B2C3D4E5F",
            "run 0b6c2d4e-1f3a-4b5c-8d9e-0a1b2c3d4e5f"
        ),
        Vec::<&str>::new()
    );
}

#[test]
fn words_names_dates_numbers_links_and_mentions_are_no_opaque_ids() {
    use figures_parser::unprinted_ids;
    let reply = "Ran daily_recap_report on 2026-11-02 for order 40017731, deadbeef cafe, \
                 see https://x.com/runs/9f8e7d6c5b4a39281706.";
    assert_eq!(unprinted_ids(reply, ""), Vec::<&str>::new());
    assert_eq!(
        unprinted_ids("<@U0A1B2C3D> here is the summary, see <#C0A1B2C3D>.", ""),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids(
            "Then `df_sales2024q1 = load()`; account acct_1234567890abcdefgh.",
            ""
        ),
        ["acct_1234567890abcdefgh"]
    );
    // A tracker key and an id with a dash in its body are neither shape.
    assert_eq!(
        unprinted_ids("CORE-1711 and prod_VU49BpM22sWxIM-x", ""),
        Vec::<&str>::new()
    );
}

#[test]
fn an_opaque_id_beside_a_link_is_read_apart_from_it() {
    use figures_parser::unprinted_ids;
    // A chat link's label is read, its address is not, and a mention is the
    // mention check's.
    assert_eq!(
        unprinted_ids(
            "<https://acme.io/x|9f2e4c1a0b> and <@9f2e4c1a0c>, https://acme.io/r/0a1b2c3d4e",
            ""
        ),
        ["9f2e4c1a0b"]
    );
    // A shorter link that starts a longer one blanks only itself.
    assert_eq!(
        unprinted_ids("https://acme.io/a https://acme.io/ab 0a1b2c3d4e", ""),
        ["0a1b2c3d4e"]
    );
}

#[test]
fn an_opaque_id_is_grounded_by_a_corpus_that_lowercases_to_it() {
    use figures_parser::unprinted_ids;
    // The Kelvin sign lowercases to `k`.
    assert_eq!(
        unprinted_ids("prod_kabc12345XYZ", "prod_\u{212A}abc12345xyz"),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids("prod_kabc12345XYZ", "prod_xabc12345xyz"),
        ["prod_kabc12345XYZ"]
    );
}

// The TypeScript check reads a typed id's prefix as one word, so an id under
// a prefix that names a key's mode (a checkout session's `cs_test_`, a key's
// `sk_live_`) goes unread; such an id is as opaque as one under one word. A
// prefix of several words that names no mode is a snake-case name.
#[test]
fn holds_a_typed_id_whose_prefix_names_a_mode() {
    use figures_parser::unprinted_ids;
    assert_eq!(
        unprinted_ids(
            "Session cs_test_a1B2c3D4e5F6 and key sk_live_51HxQz9KdLmN2pQr",
            ""
        ),
        ["cs_test_a1B2c3D4e5F6", "sk_live_51HxQz9KdLmN2pQr"]
    );
    assert_eq!(
        unprinted_ids(
            "Session cs_test_a1B2c3D4e5F6.",
            r#"{"id":"cs_test_a1B2c3D4e5F6"}"#
        ),
        Vec::<&str>::new()
    );
    // Four words, or a snake-case name, are no prefix.
    assert_eq!(
        unprinted_ids("ab_cd_ef_gh_a1B2c3D4e5F6 and df_sales_2024q1", ""),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids(
            "tag it utm_campaign_BlackFriday2024 | use `fb_ads_Q4Retarget2024` | call it \
             test_user_Admin2026x | env var stripe_key_Live2026Prod | the column user_id_v2Final2024",
            ""
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        unprinted_ids(
            "acct_sandbox_Q4x9Lm2Pz8Tw and pk_test_live_9xQz2LmN4pQr",
            ""
        ),
        ["acct_sandbox_Q4x9Lm2Pz8Tw", "pk_test_live_9xQz2LmN4pQr"]
    );
}
