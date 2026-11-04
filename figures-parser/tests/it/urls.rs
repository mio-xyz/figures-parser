use figures_parser::{unopened_links, urls, without_links};

/// The link check with no integration hosts declared.
fn fabricated_urls<'r>(reply: &'r str, corpus: &str) -> Vec<&'r str> {
    figures_parser::fabricated_urls(reply, corpus, &[])
}

/// The reply check with no integration hosts declared.
fn ungrounded_urls(reply: &str, corpus: &str, prompt: &str) -> Vec<String> {
    figures_parser::ungrounded_urls(reply, corpus, prompt, &[])
}

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
        ["#87 (https://github.com/acme-co/frontend/issues/87)"]
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

// A host is read as linkify-it reads one after a scheme: any run of labels,
// one letter long or an address included, and a port up to 65535.
#[test]
fn reads_a_host_of_any_labels_and_a_port_as_linkify_it_does() {
    assert_eq!(texts("https://a.b/c"), ["https://a.b/c"]);
    assert_eq!(
        texts("http://localhost:3000/x and http://10.0.0.1/y"),
        ["http://localhost:3000/x", "http://10.0.0.1/y"]
    );
    assert_eq!(texts("http://acme.io:65535/x"), ["http://acme.io:65535/x"]);
    // A port past 65535, or a host a dash or an underscore closes, is no link.
    for none in [
        "http://acme.io:65536/x",
        "https://acme.io-",
        "https://acme_x.io/a",
    ] {
        assert_eq!(texts(none), Vec::<&str>::new(), "{none}");
    }
}

// A bracket that closes nothing ends a link, and one the path opens and
// closes stays in it, as linkify-it reads them.
#[test]
fn ends_a_link_at_a_bracket_that_closes_nothing() {
    assert_eq!(
        texts("(see https://acme.io/a(b)/c) and https://acme.io/d(日本"),
        ["https://acme.io/a(b)/c", "https://acme.io/d"]
    );
    assert_eq!(
        texts("https://acme.io/x)https://acme.io/y"),
        ["https://acme.io/x", "https://acme.io/y"]
    );
}

// An ftp link and a link without a scheme are read as linkify-it reads
// them, so a link inside one is part of it.
#[test]
fn reads_no_link_inside_another_scheme_s() {
    assert_eq!(texts("ftp://acme.io/https://b.io/x"), Vec::<&str>::new());
    assert_eq!(texts("(//acme.io/https://b.io/x)"), Vec::<&str>::new());
}

// linkify-it reads a user name as the host ("https://user"), so the link
// ends before the `@`; the host is the one after it.
#[test]
fn reads_a_user_name_before_the_host() {
    assert_eq!(
        texts("Open https://user@acme.io/settings now."),
        ["https://user@acme.io/settings"]
    );
    // Marks no user name takes unescaped are no user name.
    assert_eq!(
        texts("https://www.github.com\"@the"),
        ["https://www.github.com"]
    );
    assert_eq!(texts("https://docs.acme.io<@x"), ["https://docs.acme.io"]);
}

// linkify-it reads the label of a chat link whose `<` is missing, a
// markdown link's label written right after a link, and a backtick, as part
// of the link: each ends it here.
#[test]
fn ends_a_link_where_chat_or_markdown_syntax_opens() {
    assert_eq!(
        texts("https://acme.io/88a|the doc> later"),
        ["https://acme.io/88a"]
    );
    assert_eq!(
        texts("https://acme.io/88a\\|the doc>"),
        ["https://acme.io/88a"]
    );
    assert_eq!(
        texts("https://acme.io/x[docs](https://b.io/y)"),
        ["https://acme.io/x", "https://b.io/y"]
    );
    assert_eq!(texts("https://acme.io/a``,issue"), ["https://acme.io/a"]);
    // A pipe no label follows stays in the link, as some links hold one.
    assert_eq!(
        texts("https://fonts.example.org/css?family=Roboto|Open+Sans"),
        ["https://fonts.example.org/css?family=Roboto|Open+Sans"]
    );
}

// A `>` later in the sentence that is an arrow, a comparison or `>=` closes
// no chat label, so the pipe before it stays in the link, as linkify-it
// reads it, and an invented link is not cut down to one the corpus holds.
#[test]
fn reads_on_through_a_pipe_when_the_later_angle_bracket_is_prose() {
    let reply = "Fonts: https://fonts.example.org/css?family=Roboto|Inter -> used on the site";
    assert_eq!(
        texts(reply),
        ["https://fonts.example.org/css?family=Roboto|Inter"]
    );
    assert_eq!(
        fabricated_urls(reply, "https://fonts.example.org/css?family=Roboto"),
        ["https://fonts.example.org/css?family=Roboto|Inter"]
    );
    assert_eq!(
        texts("https://acme.io/blog/(draft|v2) -> published"),
        ["https://acme.io/blog/(draft|v2)"]
    );
    assert_eq!(
        texts("https://grafana.acme.io/d/abc?var-env=prod|staging (filter > 5)"),
        ["https://grafana.acme.io/d/abc?var-env=prod|staging"]
    );
    assert_eq!(
        texts("https://acme.io/q?a=1|2 => b and x|y>=3"),
        ["https://acme.io/q?a=1|2"]
    );
    assert_eq!(
        texts("https://acme.io/q?a=1|b>=3"),
        ["https://acme.io/q?a=1|b"]
    );
    assert_eq!(
        texts("https://acme.io/88a|the doc>."),
        ["https://acme.io/88a"]
    );
    // A `)` the label never opened closes a group the link opened.
    assert_eq!(
        texts("https://acme.io/blog/(draft|v2)>."),
        ["https://acme.io/blog/(draft|v2)"]
    );
}

// A link read where linkify-it reads none (glued to a letter, or after a
// scheme that opens no link, which linkify-it then skips past) never takes
// in one linkify-it reads on its own.
#[test]
fn a_glued_link_ends_before_a_link_linkify_it_reads() {
    assert_eq!(texts("http:https://b.io/x"), ["https://b.io/x"]);
    assert_eq!(
        texts("😀https://acme.io/x=;https://b.io/y"),
        ["https://acme.io/x=", "https://b.io/y"]
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

#[test]
fn grounds_a_link_a_source_printed_without_its_scheme() {
    assert_eq!(
        fabricated_urls(
            "See https://github.com/org/repo.",
            "repo: github.com/org/repo"
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        fabricated_urls(
            "See https://github.com/org/repo.",
            "repo: github.com/org/repository"
        ),
        Vec::<&str>::new()
    );
    assert_eq!(
        fabricated_urls(
            "See https://github.com/org/repo.",
            "mail: x@github.com/org/repo"
        ),
        ["https://github.com/org/repo"]
    );
    assert_eq!(
        fabricated_urls(
            "See https://github.com/org/repo.",
            "foo.github.com/org/repo"
        ),
        ["https://github.com/org/repo"]
    );
}

const HOSTS: [&str; 3] = ["github.com", "linear.app", "notion.so"];

fn built(url: &str, corpus: &str) -> bool {
    figures_parser::fabricated_urls(url, corpus, &HOSTS).is_empty()
}

#[test]
fn takes_a_page_naming_no_item_on_a_services_own_host() {
    assert!(built("https://github.com/new", ""));
    assert!(built("https://app.pipedrive.com", ""));
    assert!(built("https://linear.app/", ""));
    assert!(built("https://github.com/settings/security", ""));
    assert!(built("https://github.com/settings", ""));
    assert!(built("https://www.github.com/settings/security", ""));
    assert!(!built("https://github.com/settings/tokens/12345", ""));
    assert!(!built("https://github.com/settings/keys/new", ""));
    // An owner and a repo, an id, or an unknown host hold to the turn's own print.
    assert!(!built("https://github.com/mio-xyz/invented-repo", ""));
    assert!(!built("https://github.com/mio-xyz/settings", ""));
    assert!(!built("https://github.com/pull/12", ""));
    assert!(!built("https://www.google.com", ""));
    // A query or a fragment too.
    assert!(!built("https://github.com/new?x=1", ""));
    assert!(!built("https://github.com/new#setup", ""));
    assert!(!built("https://app.pipedrive.com/?x=1", ""));
    // A shape's host gets its root and never a page under it.
    assert!(built("https://www.facebook.com/", ""));
    assert!(!built("https://www.facebook.com/someinventedpage", ""));
    assert!(!built("https://app.pipedrive.com/settings", ""));
    assert!(!built(
        "https://mioxyz.slack.com/archives",
        "mioxyz.slack.com"
    ));
    assert_eq!(
        figures_parser::fabricated_urls(
            "https://www.facebook.com/someinventedpage",
            "",
            &["facebook.com"]
        ),
        ["https://www.facebook.com/someinventedpage"]
    );
}

#[test]
fn builds_a_slack_message_link_from_its_channel_and_ts_on_the_workspace_the_turn_printed() {
    let corpus = "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791443682601779 | thread_ts: 1791180327.741099 in C09R2NX3D1R";
    let reply = "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327741099";
    assert!(built(reply, corpus));
    assert!(built(
        &format!("{reply}?thread_ts=1791180327.741099&cid=C09R2NX3D1R"),
        corpus
    ));
    assert!(!built(
        &format!("{reply}?thread_ts=1791180327.741100&cid=C09R2NX3D1R"),
        corpus
    ));
    assert!(!built(&format!("{reply}?thread_ts="), corpus));
    assert!(!built(&format!("{reply}?utm_source=mio"), corpus));
    assert!(!built(&format!("{reply}#top"), corpus));
    assert!(!built(
        "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327.741099",
        corpus
    ));
    assert!(!built(
        "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327741100",
        corpus
    ));
    assert!(!built(
        "https://mioxyz.slack.com/archives/C0OTHER0001/p1791180327741099",
        corpus
    ));
    assert!(!built(
        "https://other.slack.com/archives/C09R2NX3D1R/p1791180327741099",
        corpus
    ));
}

#[test]
fn builds_a_pipedrive_record_link_from_an_id_the_turn_printed() {
    let corpus = "ID: 3 | Title: Acme renewal | URL: None\nID: 31 | Title: Beta";
    assert!(built("https://app.pipedrive.com/deal/3", corpus));
    assert!(built("https://app.pipedrive.com/person/31", corpus));
    assert!(!built("https://app.pipedrive.com/deal/1", corpus));
    assert!(!built("https://app.pipedrive.com/lead/3", corpus));
    assert!(!built("https://acme.pipedrive.com/deal/3", corpus));
    assert!(built(
        "https://acme.pipedrive.com/deal/3",
        &format!("{corpus}\nacme.pipedrive.com")
    ));
    assert!(!built("https://app.pipedrive.com/deal/3?x=1", corpus));
    assert!(!built("https://app.pipedrive.com/deal/3#notes", corpus));
}

#[test]
fn builds_a_pages_ad_library_link_from_the_page_id_the_turn_printed() {
    let corpus = "{'id': '1219275764612649', 'name': 'Mio'}";
    let library = "https://www.facebook.com/ads/library/?active_status=active&ad_type=all&country=US&view_all_page_id=";
    assert!(built(
        &format!("{library}1219275764612649&search_type=page"),
        corpus
    ));
    assert!(!built(
        &format!("{library}1219275764612650&search_type=page"),
        corpus
    ));
}

#[test]
fn gives_an_items_page_link_never_its_api_one_and_each_item_an_id_names() {
    let api = "https://api.github.com/repos/mio-xyz/coworker/issues/3647";
    assert_eq!(ungrounded_urls("Open: #3647.", api, ""), none());
    assert_eq!(
        ungrounded_urls(
            "Open: #3647.",
            &format!("{api} https://github.com/mio-xyz/coworker/pull/3647"),
            ""
        ),
        ["#3647 (https://github.com/mio-xyz/coworker/pull/3647)"]
    );
    assert_eq!(
        ungrounded_urls(
            "Open: #12.",
            "https://github.com/a/one/issues/12 https://github.com/a/two/pull/12",
            ""
        ),
        ["#12 (https://github.com/a/two/pull/12 or https://github.com/a/one/issues/12)"]
    );
    assert_eq!(
        ungrounded_urls(
            "Open: #12.",
            "https://gitlab.com/api/v4/projects/1/issues/12",
            ""
        ),
        none()
    );
}

#[test]
fn reads_an_id_inside_code_as_code_never_as_an_item_named() {
    let corpus = "https://github.com/mio-xyz/coworker/pull/429";
    assert_eq!(
        ungrounded_urls("Run `gh pr checkout #429` first.", corpus, ""),
        none()
    );
    assert_eq!(
        ungrounded_urls("```\ngit log --grep '#429'\n```", corpus, ""),
        none()
    );
    // A fence closes at a line with at least as many backticks, or at the end.
    assert_eq!(
        ungrounded_urls("```\ngit log --grep '#429'\n````\nThen #429.", corpus, ""),
        [format!("#429 ({corpus})")]
    );
    assert_eq!(
        ungrounded_urls("```\ngit log --grep '#429'", corpus, ""),
        none()
    );
    assert_eq!(
        ungrounded_urls("Run ``#429`` then #429.", corpus, ""),
        [format!("#429 ({corpus})")]
    );
    assert_eq!(
        ungrounded_urls("Run ``#429` then nothing.", corpus, ""),
        [format!("#429 ({corpus})")]
    );
}

#[test]
fn takes_an_example_domain_link_the_prompt_illustrates_a_format_with_as_no_items_own() {
    let prompt = "Inline by identifier: [#429](https://example.com/issues/429)";
    assert_eq!(ungrounded_urls("Open: #429.", "", prompt), none());
    assert_eq!(
        ungrounded_urls("Open: #429.", "https://example.com/issues/429", prompt),
        ["#429 (https://example.com/issues/429)"]
    );
    assert_eq!(
        ungrounded_urls("Open: #429.", "", "Repo: https://github.com/o/r/pull/429"),
        ["#429 (https://github.com/o/r/pull/429)"]
    );
}

#[test]
fn takes_a_number_linked_to_a_page_of_another_kind_as_that_pages_own() {
    let sweep = "https://mfc-nyc.slack.com/archives/C0B9H0H1YJZ/p1791652457074079";
    assert_eq!(
        ungrounded_urls(&format!("Between Sweep #59 and [#60]({sweep})."), sweep, ""),
        none()
    );
    let pr = "https://github.com/mio-xyz/coworker/pull/418";
    assert_eq!(
        ungrounded_urls(&format!("[#429]({pr})"), pr, ""),
        [format!("[#429]({pr})")]
    );
    assert_eq!(
        ungrounded_urls(&format!("[#429]({pr}/files)"), &format!("{pr}/files"), ""),
        [format!("[#429]({pr}/files)")]
    );
}

const PR: &str = "https://github.com/mio-xyz/coworker/pull/3647";
const DOTTED: &str = "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327.741099";
const UNDOTTED: &str = "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327741099";

// A link no browser opens (a port past 65535, a host it rejects, an
// unclosed address) links no item, so a number's label on it is misdirected.
#[test]
fn takes_a_number_on_a_link_that_does_not_parse_as_misdirected() {
    let reply = "[#12](https://acme.io:65536/org/prod_VU49Bp) ; [#12](https://%zz.io/org/a1b) ; [#12](https://[::1/x1y)";
    assert_eq!(
        figures_parser::link_findings(reply, "", "", &[]).misdirected,
        [
            "[#12](https://acme.io:65536/org/prod_VU49Bp)",
            "[#12](https://%zz.io/org/a1b)",
            "[#12](https://[::1/x1y)"
        ]
    );
    assert_eq!(
        figures_parser::link_findings("[#12](https://acme.io:8080/org/prod_VU49Bp)", "", "", &[])
            .misdirected,
        Vec::<&str>::new()
    );
    // A host of ASCII labels whose punycode does not decode still parses.
    assert_eq!(
        figures_parser::link_findings("[#12](https://xn--zz.io/org/a1b)", "", "", &[]).misdirected,
        Vec::<&str>::new()
    );
}

#[test]
fn keeps_the_findings_apart_by_kind_and_lists_them_as_the_flat_check_does() {
    let reply =
        "See [#87](https://acme.io/repo), [PR 512](url) and https://acme.io/made-up; open #12.";
    let corpus = "https://acme.io/repo https://github.com/a/one/issues/12";
    let findings = figures_parser::link_findings(reply, corpus, "", &[]);
    assert_eq!(findings.unprinted, ["https://acme.io/made-up"]);
    assert_eq!(findings.placeholders, ["url"]);
    assert_eq!(findings.misdirected, ["[#87](https://acme.io/repo)"]);
    assert_eq!(
        findings.unlinked,
        ["#12 (https://github.com/a/one/issues/12)"]
    );
    assert!(!findings.is_empty());
    assert_eq!(findings.all(), ungrounded_urls(reply, corpus, ""));
    assert!(figures_parser::link_findings("Nothing linked.", "", "", &[]).is_empty());
}

#[test]
fn mends_the_dot_a_slack_message_link_kept_from_its_ts_wherever_the_reply_writes_it() {
    let reply = format!(
        "[Axel]({DOTTED}) and <{DOTTED}|this one>, then {DOTTED}?thread_ts=1791180327.741099."
    );
    assert_eq!(
        figures_parser::mended_links(&reply, "").text,
        format!(
            "[Axel]({UNDOTTED}) and <{UNDOTTED}|this one>, then {UNDOTTED}?thread_ts=1791180327.741099."
        )
    );
    let right = format!("[Axel]({UNDOTTED})");
    assert_eq!(figures_parser::mended_links(&right, "").text, right);
    assert_eq!(
        figures_parser::mended_links(&format!("{DOTTED}?q=$&x"), "").text,
        format!("{UNDOTTED}?q=$&x")
    );
    // A ts of another length, or a link off Slack, is left as written.
    for untouched in [
        "https://mioxyz.slack.com/archives/C09R2NX3D1R/p1791180327.7410",
        "https://example.com/archives/C09R2NX3D1R/p1791180327.741099",
    ] {
        let mended = figures_parser::mended_links(untouched, "");
        assert_eq!(mended.text, untouched);
        assert_eq!(mended.mends, none());
    }
}

#[test]
fn mends_an_item_named_without_its_link_where_the_reply_first_names_it_in_prose() {
    let corpus = format!(
        "{{\"issue_url\": \"https://api.github.com/repos/mio-xyz/coworker/issues/3647\", \"html_url\": \"{PR}\"}}\nCORE-4517 https://linear.app/mioxyz/issue/CORE-4517/fix-it"
    );
    assert_eq!(
        figures_parser::mended_links(
            "Pick up `#3647` (#3647, **CORE-4517**); #3647 is small.",
            &corpus
        )
        .text,
        format!(
            "Pick up `#3647` ([#3647]({PR}), **[CORE-4517](https://linear.app/mioxyz/issue/CORE-4517)**); #3647 is small."
        )
    );
    // An item linked once already, one the results do not link, and an id naming two items stay.
    let linked = format!("[the PR]({PR}) is #3647.");
    assert_eq!(figures_parser::mended_links(&linked, PR).text, linked);
    assert_eq!(
        figures_parser::mended_links("Open: #3648.", PR).text,
        "Open: #3648."
    );
    let two = "https://github.com/a/one/issues/12 https://github.com/a/two/issues/12";
    assert_eq!(
        figures_parser::mended_links("Open: #12.", two).text,
        "Open: #12."
    );
}

#[test]
fn a_mended_reply_passes_the_check_and_names_each_repair_made() {
    let corpus = format!("{PR}\nthread_ts: 1791180327.741099 in C09R2NX3D1R on mioxyz.slack.com");
    let reply = format!("#3647 came up in {DOTTED}.");
    assert_eq!(ungrounded_urls(&reply, &corpus, "").len(), 2);
    let mended = figures_parser::mended_links(&reply, &corpus);
    assert_eq!(ungrounded_urls(&mended.text, &corpus, ""), none());
    assert_eq!(
        mended.mends,
        [format!("{DOTTED} → {UNDOTTED}"), format!("#3647 → {PR}")]
    );
}

#[test]
fn an_address_is_printed_whole_without_its_scheme_or_by_its_path() {
    use figures_parser::url_printed_in;
    let url = "https://www.acme.com/blog/when-marking-runs-late/";
    for read in [
        "[When marking runs late](/blog/when-marking-runs-late)",
        "see acme.com/blog/when-marking-runs-late",
        "<https://www.acme.com/blog/when-marking-runs-late/>",
    ] {
        assert!(url_printed_in(url, read), "{read}");
    }
    // A site's root the asker gave without a scheme.
    assert!(url_printed_in(
        "https://acme.com",
        "the site is live on <http://acme.com|acme.com>"
    ));
}

#[test]
fn an_address_built_from_a_title_is_not_printed() {
    use figures_parser::url_printed_in;
    let read = "the site is live on acme.com\n## The Journal\n## When marking runs late\n\
                ![](/blog-images/marking.jpg)";
    assert!(!url_printed_in(
        "https://acme.com/blog/when-marking-runs-late",
        read
    ));
    assert!(!url_printed_in("https://acme.com", "nothing"));
}

#[test]
fn an_address_is_compared_as_a_browser_parses_it() {
    use figures_parser::url_printed_in;
    assert!(url_printed_in("https://ACME.com/x", "acme.com/x"));
    assert!(url_printed_in("https://acme.com:443/x", "acme.com/x"));
    assert!(url_printed_in("https://acme.com:8080/x", "acme.com:8080/x"));
    assert!(!url_printed_in("https://acme.com:8080/", "acme.com"));
    assert!(url_printed_in("https://acme.com/a b", "/a%20b"));
    assert!(url_printed_in("https://acme.com/a/../b", "acme.com/b"));
    assert!(url_printed_in("https://bücher.de/x", "xn--bcher-kva.de/x"));
    assert!(url_printed_in("https://acme.com/?q=1", "?q=1"));
    // One that does not parse is printed only whole.
    assert!(url_printed_in("acme.com/x/", "acme.com/x"));
    assert!(!url_printed_in("acme.com/x", "/x"));
}

// Node's URL parser keeps a `file:` path's empty segments, escapes a `^` and
// reads a drive letter only in a `file:` path; the `url` crate does none of
// that, so the path is read the browser's way.
#[test]
fn an_address_s_path_is_parsed_as_a_browser_parses_it() {
    use figures_parser::url_printed_in;
    assert!(url_printed_in("file:////x", "//x"));
    assert!(!url_printed_in("file:////x", "see /x."));
    assert!(url_printed_in("https://acme.com/a^b/c", "/a%5Eb/c"));
    assert!(url_printed_in("https://acme.com/C|/.%2E/x", "acme.com/x"));
    // A scheme with no special meaning and a port that is no number does not
    // parse, so it is printed only whole.
    assert!(!url_printed_in("foo://C:\\x?a", "foo://c:\\x?a"));
}

#[test]
fn names_each_link_target_that_is_no_url() {
    use figures_parser::placeholder_targets;
    let reply = "![chart](chart.png), [again](chart.png), [notes](notes.md), [top](#top), [mail](mailto:a@b.c) and [doc](https://acme.io/doc).";
    assert_eq!(
        placeholder_targets(reply),
        ["chart.png", "chart.png", "notes.md"]
    );
}

#[test]
fn drops_the_dot_a_slack_message_link_kept_and_leaves_any_other() {
    use figures_parser::undotted_message_link;
    assert_eq!(
        undotted_message_link(
            "https://acme.slack.com/archives/C1/p1791180327.741099?thread_ts=1791180327.741099"
        )
        .as_deref(),
        Some("https://acme.slack.com/archives/C1/p1791180327741099?thread_ts=1791180327.741099")
    );
    for left in [
        "https://acme.slack.com/archives/C1/p1791180327741099",
        "https://acme.slack.com/archives/C1/p179118032.741099",
        "https://acme.io/archives/C1/p1791180327.741099",
    ] {
        assert_eq!(undotted_message_link(left), None, "{left}");
    }
}

#[test]
fn lists_the_pages_a_message_links_once_each_less_slack_and_the_opened() {
    use figures_parser::unopened_links;
    assert_eq!(
        unopened_links(
            "the footer should be a png with <http://acme.example.com|acme.example.com> logo",
            &[]
        ),
        ["http://acme.example.com"]
    );
    assert_eq!(
        unopened_links(
            "compare https://a.example.org/pricing and <https://b.example.org> with https://a.example.org/pricing",
            &[]
        ),
        ["https://a.example.org/pricing", "https://b.example.org"]
    );
    assert_eq!(
        unopened_links(
            "see <https://acme.slack.com/archives/C0123ABCD/p1793264631123499> and https://acme.example.com/brand",
            &[]
        ),
        ["https://acme.example.com/brand"]
    );
    assert_eq!(
        unopened_links("make the footer nicer, acme.example.com style", &[]),
        Vec::<&str>::new()
    );
    assert_eq!(
        unopened_links(
            "again: https://a.example.org/pricing, https://a.example.org/faq and https://b.example.org",
            &["https://a.example.org/pricing", "https://b.example.org"]
        ),
        ["https://a.example.org/faq"]
    );
}

// A query's values are read decoded, as a browser reads them, and the
// slashes after an http(s) scheme as a browser reads them; another scheme's
// link on a service's host is no page the service serves.
#[test]
fn reads_a_built_link_s_query_decoded_and_its_scheme_as_a_browser_does() {
    use figures_parser::built_from_corpus;
    assert!(built_from_corpus(
        "https://www.facebook.com/ads/library/?view_all_page_id=%31%32",
        "page 12",
        &[]
    ));
    assert!(built_from_corpus(
        "https://www.facebook.com/ads/library/?view%5Fall_page_id=12",
        "page 12",
        &[]
    ));
    assert!(built_from_corpus(
        "https://a.slack.com/archives/C1/p1791180327741099?cid=C%31",
        "a.slack.com C1 1791180327.741099",
        &[]
    ));
    assert!(!built_from_corpus(
        "https://a.slack.com/archives/C1/p1791180327741099?cid=C%32",
        "a.slack.com C1 1791180327.741099",
        &[]
    ));
    for link in [
        "https:/notion.so/library",
        "https:///notion.so",
        "https:notion.so",
        "HTTPS:\\\\notion.so\\library",
    ] {
        assert!(built_from_corpus(link, "", &["notion.so"]), "{link}");
    }
    for link in [
        "ftp://notion.so/library",
        "ws://notion.so",
        "file://app.pipedrive.com",
        "hxxp://facebook.com#",
        "https:",
    ] {
        assert!(!built_from_corpus(link, "", &["notion.so"]), "{link}");
    }
}

// A browser resolves `.` and `..` and reads `\` as `/`, so a link that
// resolves to a host's root is that root.
#[test]
fn reads_a_built_link_s_path_as_a_browser_resolves_it() {
    use figures_parser::built_from_corpus;
    assert!(built_from_corpus("https://facebook.com/.", "", &[]));
    assert!(built_from_corpus("https://app.pipedrive.com/x/..", "", &[]));
    assert!(built_from_corpus("https://github.com/a/../new", "", &HOSTS));
    assert!(built_from_corpus("https://github.com\\new", "", &HOSTS));
    assert!(!built_from_corpus(
        "https://facebook.com/someinventedpage",
        "",
        &[]
    ));
}

// Node percent-encodes a page name outside ASCII, so the TypeScript check
// finds digits in `é` and holds the page to an item; the name has none.
#[test]
fn reads_a_page_name_outside_ascii_as_written() {
    use figures_parser::built_from_corpus;
    assert!(built_from_corpus("https://github.com/équipe", "", &HOSTS));
    assert_eq!(
        ungrounded_urls(
            "[#4](https://bücher.de/x/é_x)",
            "https://bücher.de/x/é_x",
            ""
        ),
        ["[#4](https://bücher.de/x/é_x)"]
    );
}

// The TypeScript check reads a workspace's host in the corpus as written,
// so an upper-cased one grounds nothing; a host has no case.
#[test]
fn a_workspace_host_printed_in_any_case_grounds_its_root() {
    use figures_parser::built_from_corpus;
    assert!(built_from_corpus(
        "https://acme.slack.com/",
        "workspace ACME.SLACK.COM",
        &[]
    ));
}

// The TypeScript check finds a link printed without its scheme only under a
// top-level domain it lists, and without a port or an address; a printed
// `acme.app/x`, `acme.io:8080/x` or `127.0.0.1/x` is the same link.
#[test]
fn grounds_a_link_printed_without_its_scheme_under_any_host() {
    for (link, printed) in [
        (
            "https://linear.app/acme/issue/CORE-1",
            "url: linear.app/acme/issue/CORE-1",
        ),
        ("https://acme.io:8080/deal/352", "at acme.io:8080/deal/352"),
        (
            "https://127.0.0.1/runs/572/logs",
            "see 127.0.0.1/runs/572/logs.",
        ),
    ] {
        assert_eq!(fabricated_urls(link, printed), Vec::<&str>::new(), "{link}");
    }
}

#[test]
fn takes_links_out_wherever_their_text_recurs() {
    assert_eq!(
        without_links("a.com/ https://a.com/206788 and https://a.com/ x"),
        "a.com/   and   x"
    );
    assert_eq!(
        without_links("<https://acme.io/p/1|label> 1,250"),
        "< |label> 1,250"
    );
    assert_eq!(
        without_links("12:30 at ftp://x.io/9"),
        "12:30 at ftp://x.io/9"
    );
}

#[test]
fn lists_the_pages_a_message_links_that_no_turn_opened() {
    let text = "Like https://acme.io/brand and https://acme.io/brand, not https://ACME.Slack.com/archives/C1/p1 or https://slack.com/x";
    assert_eq!(unopened_links(text, &[]), ["https://acme.io/brand"]);
    assert_eq!(
        unopened_links(
            "see <https://acme.io/a|a> and https://b.io",
            &["https://b.io"]
        ),
        ["https://acme.io/a"]
    );
    assert_eq!(
        unopened_links("https://notslack.com.evil.io/x", &[]),
        ["https://notslack.com.evil.io/x"]
    );
}
