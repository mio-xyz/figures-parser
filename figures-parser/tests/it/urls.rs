use figures_parser::{fabricated_urls, ungrounded_urls, urls};

fn texts(s: &str) -> Vec<&str> {
    urls(s).into_iter().map(|u| u.text).collect()
}

const fn none() -> Vec<String> {
    Vec::new()
}

#[test]
fn drops_trailing_punctuation_but_keeps_it_inside_the_path() {
    assert_eq!(
        texts("See https://acme.io/c. And https://acme.io/v1.2/x, ok?"),
        ["https://acme.io/c", "https://acme.io/v1.2/x"]
    );
}

#[test]
fn keeps_balanced_parens_in_the_url_and_drops_the_wrapping_ones() {
    assert_eq!(
        texts("wiki (https://en.wikipedia.org/wiki/Bracket_(disambiguation))"),
        ["https://en.wikipedia.org/wiki/Bracket_(disambiguation)"]
    );
}

#[test]
fn extracts_from_markdown_links_and_slack_mrkdwn_escaped_pipe_included() {
    assert_eq!(
        texts("[PR](https://github.com/o/r/pull/1) and <https://linear.app/x/C-1\\|C-1>!"),
        ["https://github.com/o/r/pull/1", "https://linear.app/x/C-1"]
    );
    assert_eq!(
        texts("<https://acme.io/c|label> and <https://acme.io/d>"),
        ["https://acme.io/c", "https://acme.io/d"]
    );
}

#[test]
fn ignores_bare_domains_and_non_http_schemes() {
    assert_eq!(
        texts("visit example.com or ftp://files.example.com or mailto:a@b.c"),
        Vec::<&str>::new()
    );
}

#[test]
fn a_link_slices_the_input_at_its_span_slack_wrapping_included() {
    let text = "😀 <https://acme.io/c|label> then **https://acme.io/d**.";
    for url in urls(text) {
        assert_eq!(text.get(url.span), Some(url.text));
    }
}

#[test]
fn fabricated_flags_a_url_absent_from_every_tool_result() {
    assert_eq!(
        fabricated_urls(
            "See https://github.com/org/repo/pull/12 and <https://linear.app/x/CORE-1|CORE-1>.",
            "https://github.com/org/repo/pull/12"
        ),
        ["https://linear.app/x/CORE-1"]
    );
}

#[test]
fn fabricated_ignores_trailing_punctuation_and_wrapping_markup() {
    assert_eq!(
        fabricated_urls("Look: https://acme.io/c.", "https://acme.io/c"),
        Vec::<&str>::new()
    );
    assert_eq!(
        fabricated_urls(
            "Merged **https://acme.io/pull/429**.",
            "https://acme.io/pull/429"
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        texts("Merged **https://acme.io/pull/429**."),
        ["https://acme.io/pull/429"]
    );
}

const PATH: &str = "/project/clx7k2m9p0001/traces/9f2e4c1a7b3d4e5f8a6b0c1d2e3f4a5b";

fn link() -> String {
    format!("https://cloud.tracing.tech{PATH}")
}

fn trace(url: &str) -> String {
    format!("[Trace]({url})")
}

#[test]
fn grounds_a_link_built_on_the_services_address_from_a_path_without_its_host() {
    let link = link();
    assert_eq!(
        fabricated_urls(&trace(&link), &format!("{{'htmlPath': '{PATH}'}}")),
        Vec::<&str>::new()
    );
    assert_eq!(
        fabricated_urls(
            &trace(&format!("{link}?view=preview")),
            &format!("\"htmlPath\":\"{PATH}?view=preview\"")
        ),
        Vec::<&str>::new()
    );
}

#[test]
fn grounds_the_link_without_the_query_the_path_came_with_never_with_one_nothing_gave() {
    let link = link();
    assert_eq!(
        fabricated_urls(
            &trace(&link),
            &format!("\"htmlPath\":\"{PATH}?view=preview\"")
        ),
        Vec::<&str>::new()
    );
    let tracked = format!("{link}?utm_source=ads");
    assert_eq!(
        fabricated_urls(&trace(&tracked), &format!("\"htmlPath\":\"{PATH}\"")),
        [tracked.as_str()]
    );
}

#[test]
fn does_not_ground_it_when_a_result_put_that_path_under_another_host_in_any_case() {
    let link = link();
    for other in [
        "https://eu.cloud.tracing.tech",
        "HTTPS://EU.CLOUD.TRACING.TECH",
    ] {
        let corpus = format!("htmlPath: {PATH}\nopen {other}{PATH}");
        assert_eq!(
            fabricated_urls(&trace(&link), &corpus),
            [link.as_str()],
            "{other}"
        );
    }
}

#[test]
fn grounds_the_link_without_the_fragment_the_path_came_with_never_with_one_nothing_gave() {
    let link = link();
    assert_eq!(
        fabricated_urls(
            &trace(&link),
            &format!("\"htmlPath\":\"{PATH}#observations\"")
        ),
        Vec::<&str>::new()
    );
    let anchored = format!("{link}#made-up");
    assert_eq!(
        fabricated_urls(&trace(&anchored), &format!("\"htmlPath\":\"{PATH}\"")),
        [anchored.as_str()]
    );
}

#[test]
fn grounds_the_link_that_keeps_the_query_and_drops_the_fragment_never_one_that_changes_either() {
    let link = link();
    let corpus = format!("\"htmlPath\":\"{PATH}?view=preview#observations\"");
    assert_eq!(
        fabricated_urls(&trace(&format!("{link}?view=preview")), &corpus),
        Vec::<&str>::new()
    );
    for changed in [
        format!("{link}?view=preview#made-up"),
        format!("{link}?view=prev"),
    ] {
        assert_eq!(
            fabricated_urls(&trace(&changed), &corpus),
            [changed.as_str()]
        );
    }
}

#[test]
fn does_not_ground_a_path_that_names_no_item() {
    assert_eq!(
        fabricated_urls(
            "[Billing](https://app.acme.io/settings/billing)",
            "path: /settings/billing"
        ),
        ["https://app.acme.io/settings/billing"]
    );
}

#[test]
fn does_not_ground_it_on_a_longer_path_that_only_contains_it() {
    let link = link();
    for corpus in [
        format!("path: /api/public{PATH}"),
        format!("path: {PATH}/observations"),
    ] {
        assert_eq!(
            fabricated_urls(&trace(&link), &corpus),
            [link.as_str()],
            "{corpus}"
        );
    }
}

const ISSUE_URL: &str = "https://linear.app/acme-co/issue/CORE-1710/spike-export-pdf-in-dashboard";

#[test]
fn flags_an_items_label_on_a_url_that_is_not_the_items_own() {
    let corpus = "https://github.com/acme-co/frontend https://github.com/acme-co/frontend/pull/418";
    let reply = "**[#429](https://github.com/acme-co/frontend)** and [#418](https://github.com/acme-co/frontend/pull/418)";
    assert_eq!(
        ungrounded_urls(reply, corpus, ""),
        ["[#429](https://github.com/acme-co/frontend)"]
    );
}

#[test]
fn counts_an_item_as_linked_when_the_reply_links_its_own_url_under_other_words() {
    let corpus = "https://github.com/acme-co/frontend/pull/429";
    let reply = "[github.com/acme-co/frontend/pull/429](https://github.com/acme-co/frontend/pull/429) loads fine, PR #429.";
    assert_eq!(ungrounded_urls(reply, corpus, ""), none());
}

#[test]
fn flags_an_item_named_without_its_link_when_a_tool_result_carried_its_url() {
    let corpus = "https://github.com/acme-co/frontend/issues/87 https://github.com/acme-co/frontend/pull/429";
    assert_eq!(
        ungrounded_urls(
            "Merged [#429](https://github.com/acme-co/frontend/pull/429); #429 again. Open: #87, and #12.",
            corpus,
            ""
        ),
        ["#87 (named without its link, https://github.com/acme-co/frontend/issues/87)"]
    );
}

#[test]
fn flags_an_example_domain_url_that_only_the_prompts_own_examples_carried() {
    let prompt = "Inline by identifier: [TASK-1918](https://example.com)";
    assert_eq!(
        ungrounded_urls("Fix in [PR 512](https://example.com).", "user text", prompt),
        ["https://example.com"]
    );
    assert_eq!(
        ungrounded_urls(
            "Read https://example.com for you.",
            "fetched https://example.com",
            prompt
        ),
        none()
    );
}

#[test]
fn does_not_flag_a_grounded_url_wrapped_in_bold_markers() {
    let corpus = "https://github.com/acme-co/frontend/pull/429";
    assert_eq!(
        ungrounded_urls(
            "Merged **https://github.com/acme-co/frontend/pull/429**.",
            corpus,
            ""
        ),
        none()
    );
}

#[test]
fn flags_a_link_whose_target_is_a_template_word_not_a_url() {
    assert_eq!(
        ungrounded_urls("Fix in review as [PR 512](url).", "", ""),
        ["(url)"]
    );
    assert_eq!(
        ungrounded_urls(
            "See [the doc](https://example.com/doc) and [top](#top).",
            "https://example.com/doc",
            ""
        ),
        none()
    );
}

#[test]
fn flags_a_link_derived_from_anothers_pattern_and_passes_one_a_tool_returned() {
    let corpus = format!("URL: {ISSUE_URL}");
    let reply = format!(
        "[CORE-1710]({ISSUE_URL}) and the [team board](https://linear.app/acme-co/team/CORE/issues)."
    );
    assert_eq!(
        ungrounded_urls(&reply, &corpus, ""),
        ["https://linear.app/acme-co/team/CORE/issues"]
    );
}

#[test]
fn grounds_a_link_built_from_a_hostless_path_unless_a_result_put_it_under_another_host() {
    let link = link();
    assert_eq!(
        ungrounded_urls(&trace(&link), &format!("{{'htmlPath': '{PATH}'}}"), ""),
        none()
    );
    assert_eq!(
        ungrounded_urls(
            &trace(&link),
            &format!("htmlPath: {PATH}\nopen https://eu.cloud.tracing.tech{PATH}"),
            ""
        ),
        [link]
    );
}

#[test]
fn grounds_on_what_the_tools_returned_never_on_the_models_own_drafts() {
    let rejected = "https://linear.app/acme-co/issue/CORE-1711/made-up";
    let corpus = format!("URL: {ISSUE_URL}");
    assert_eq!(
        ungrounded_urls(&format!("See [CORE-1711]({rejected})."), &corpus, ""),
        [rejected]
    );
}

#[test]
fn a_prompt_link_grounds_a_reply_link_except_on_an_example_domain() {
    assert_eq!(
        ungrounded_urls(
            "Docs: https://docs.acme.io/start",
            "",
            "See https://docs.acme.io/start"
        ),
        none()
    );
}

// linkify reads a host only under a top-level domain of two letters or more,
// which every real one has; a host without a dot or an address still counts.
#[test]
fn a_host_under_a_one_letter_top_level_domain_is_not_a_link() {
    assert_eq!(texts("https://a.b/c"), Vec::<&str>::new());
    assert_eq!(
        texts("http://localhost:3000/x and http://10.0.0.1/y"),
        ["http://localhost:3000/x", "http://10.0.0.1/y"]
    );
}

// linkify-it started a link only after whitespace, punctuation other than
// `_`, a control character, `<`, `>` or `｜`, so a link in code, in
// emphasis or after a letter of a script written without spaces went unread.
#[test]
fn reads_a_link_in_code_in_emphasis_or_glued_to_a_script_without_spaces() {
    for text in [
        "`https://acme.io/a`",
        "_https://acme.io/a_",
        "~https://acme.io/a~",
        "詳細はhttps://acme.io/a",
    ] {
        assert_eq!(texts(text), ["https://acme.io/a"], "{text}");
    }
    for glued in ["xhttps://acme.io/a", "9https://acme.io/a"] {
        assert_eq!(texts(glued), Vec::<&str>::new(), "{glued}");
    }
}

#[test]
fn unwraps_a_slack_link_whatever_the_case_of_its_scheme() {
    assert_eq!(
        texts("<HTTPS://Acme.IO/a_b\\|label>"),
        ["HTTPS://Acme.IO/a_b"]
    );
}
