//! The internals check and the memory layout scrub.

use figures_parser::{
    InternalsCheck, MemoryLayout, Vocabulary, internal_names, scrub_memory_layout,
};

/// What a check that finds nothing returns.
const NONE: [&str; 0] = [];

const TOOLS: &[&str] = &[
    "web_search",
    "slack_read_channel_history",
    "run_code",
    "memory_write",
    "learn",
];
const FILES: &[&str] = &["user.md", "workspace.md", "company.md"];
const ROOTS: &[&str] = &["/memory/", "/sdk/"];
const FIXED: &[&str] = &[
    "_supervisor",
    "_child_runner",
    "MIO_SOCK",
    "MIO_NONCE",
    "MIO_TURN_DIR",
    "MIO_DEADLINE_MS",
];

const fn vocabulary(names: &'static [&'static str]) -> Vocabulary<'static> {
    Vocabulary {
        names,
        files: FILES,
        roots: ROOTS,
        fixed: FIXED,
    }
}

fn leaked(reply: &str, said: &str, vouching: &str) -> Vec<String> {
    internal_names(reply, said, &vocabulary(TOOLS), vouching)
}

const LAYOUT: MemoryLayout<'static> = MemoryLayout {
    root: "/memory/",
    tool_prefix: "memory_",
    folders: &[("integrations/", "the integration notes")],
    files: &[
        ("user.md", "the asker's personal notes"),
        ("workspace.md", "the team's shared notes"),
        ("company.md", "the team's shared notes"),
    ],
    root_phrase: "memory",
    other_phrase: "the team's notes",
    tool_phrase: "my memory",
};

fn scrub(reply: &str, said: &str) -> String {
    scrub_memory_layout(reply, said, &LAYOUT).into_owned()
}

#[test]
fn finds_tool_names_however_a_reply_wraps_them() {
    let reply = "**`web_search`**: the telescope. Runes: `query`.\n- slack_read_channel_history(channel, limit)\n- run_code: the wand";
    assert_eq!(
        leaked(reply, "", ""),
        ["web_search", "slack_read_channel_history", "run_code"]
    );
    assert_eq!(
        leaked(
            "I can search the web, read your channels, run code, and keep team notes.",
            "",
            ""
        ),
        NONE
    );
}

#[test]
fn finds_a_memory_file_and_the_memory_root() {
    let reply = "Team notes live in `/memory/workspace.md`; yours are in user.md.";
    assert_eq!(
        leaked(reply, "", ""),
        ["user.md", "workspace.md", "/memory/"]
    );
}

#[test]
fn reads_a_name_without_an_underscore_only_as_a_call() {
    assert_eq!(leaked("Happy to help you learn the API.", "", ""), NONE);
    assert_eq!(
        leaked("First I call `learn()` to get each signature.", "", ""),
        ["learn"]
    );
    assert_eq!(leaked("Relearn(ing) the API took a day.", "", ""), NONE);
    assert_eq!(leaked("We learn (slowly).", "", ""), NONE);
}

#[test]
fn lets_a_reply_repeat_what_the_person_wrote() {
    let said = "what does web_search do, and what's in workspace.md?";
    assert_eq!(
        leaked(
            "web_search finds pages; workspace.md holds team notes.",
            said,
            ""
        ),
        NONE
    );
    assert_eq!(
        leaked(
            "MIO_TURN_DIR is where a run keeps its files.",
            "what is MIO_TURN_DIR?",
            ""
        ),
        NONE
    );
    assert_eq!(
        leaked(
            "Your code runs under /sdk/.",
            "where does my code run, /sdk/?",
            ""
        ),
        NONE
    );
}

#[test]
fn finds_a_tool_name_a_tool_result_carries() {
    let vouching = "No match. Call run_code with the ids it returned.";
    assert_eq!(
        leaked("I'll use run_code for that.", "", vouching),
        ["run_code"]
    );
}

#[test]
fn reads_no_tool_name_in_another_programs_identifiers() {
    let names: &[&str] = &["web_search", "read_file"];
    let reply = "Use fs.readFile, or clone acme/web-search and call webSearch().";
    assert_eq!(internal_names(reply, "", &vocabulary(names), ""), NONE);
}

#[test]
fn lets_a_file_through_that_a_tool_result_holds_whole() {
    assert_eq!(
        leaked(
            "The setup steps are in docs/workspace.md.",
            "",
            "docs/workspace.md\nREADME.md"
        ),
        NONE
    );
    assert_eq!(
        leaked(
            "Team notes are in workspace.md.",
            "",
            "old-workspace.md.bak"
        ),
        ["workspace.md"]
    );
}

#[test]
fn finds_the_harness_names_whatever_printed_them_and_whatever_their_case() {
    let reply = "Your code runs under `/sdk/_child_runner.py` with MIO_NONCE and MIO_SOCK set.";
    let output = "MIO_NONCE=ab12\nMIO_SOCK=/tmp/mio.sock\n/sdk/_child_runner.py";
    assert_eq!(
        leaked(reply, "", output),
        ["/sdk/", "_child_runner", "MIO_SOCK", "MIO_NONCE"]
    );
    assert_eq!(
        leaked("The runner lives under /SDK/ and reads mio_nonce.", "", ""),
        ["/sdk/", "MIO_NONCE"]
    );
}

#[test]
fn reads_a_root_only_where_it_begins_a_path() {
    let reply =
        "See https://github.com/acme/app/tree/main/sdk/ and acme/app/memory/ for the client.";
    assert_eq!(leaked(reply, "", ""), NONE);
    assert_eq!(
        leaked("Installed in (/sdk/) and notes in `/memory/`.", "", ""),
        ["/memory/", "/sdk/"]
    );
    let said = "see https://github.com/acme/app/tree/main/sdk/ for the client";
    assert_eq!(
        leaked("The runner is installed in /sdk/.", said, ""),
        ["/sdk/"]
    );
}

#[test]
fn reads_one_turn_for_every_reply() {
    let check = InternalsCheck::new("what is web_search?", &vocabulary(TOOLS));
    let mut read = false;
    assert_eq!(
        check.names_in("web_search and run_code", || {
            read = true;
            ""
        }),
        ["run_code"]
    );
    assert!(!read, "no file named, so the tool results are not read");
}

// Korean writes a particle right after a name: the name is still a word.
#[test]
fn parts_a_name_from_the_hangul_after_it() {
    assert_eq!(
        leaked("workspace.md에 저장했고 web_search를 썼어요", "", ""),
        ["web_search", "workspace.md"]
    );
    assert_eq!(leaked("먼저 learn(을 불러요", "", ""), ["learn"]);
}

// Deliberately stricter than the host's segmenter, which reads Thai as
// letters of the same word as the Latin beside it: a name glued to Thai
// text is still the name, in the reply and in the person's words alike.
#[test]
fn parts_a_name_from_the_thai_beside_it() {
    assert_eq!(leaked("ใช้web_searchค้นหา", "", ""), ["web_search"]);
    assert_eq!(leaked("ภาษาrun_code(x)", "", ""), ["run_code"]);
    assert_eq!(
        leaked("Notes are in workspace.md.", "อ่านworkspace.mdให้หน่อย", ""),
        NONE
    );
}

// The Unicode word rules part Thai, Lao, Khmer and Myanmar letters one from
// the next, so a name written in them is looked for as written, whole.
#[test]
fn reads_a_name_written_in_a_dictionary_script() {
    let names = &["ไทย_tool", "ຊອກ_tool", "ค้นหา"];
    let found = |reply: &str, said: &str| internal_names(reply, said, &vocabulary(names), "");
    assert_eq!(found("ไทย_tool（", ""), ["ไทย_tool"]);
    assert_eq!(found("ใช้ ไทย_tool ค้นหา", ""), ["ไทย_tool"]);
    assert_eq!(found("ใช้ไทย_toolค้นหา", ""), ["ไทย_tool"]);
    assert_eq!(found("ຊອກ_tool", ""), ["ຊອກ_tool"]);
    assert_eq!(found("ค้นหา(x)", ""), ["ค้นหา"]);
    for reply in ["xไทย_tool", "ไทย_toolx", "ไทย_tool1", "_ไทย_tool", "ค้นหา x"]
    {
        assert_eq!(found(reply, ""), NONE, "{reply}");
    }
    assert_eq!(found("ไทย_tool", "what does ไทย_tool do?"), NONE);
}

#[test]
fn scrubs_a_path_dump_keeping_the_facts() {
    let leak = "Two memory files exist:\n`/memory/workspace.md`\n```\n- Standup is Tuesday\n```\n`/memory/company.md` and no `/memory/user.md`.";
    let out = scrub(leak, "");
    assert!(!out.contains("/memory/"));
    assert!(out.contains("- Standup is Tuesday"));
    assert!(out.contains("the team's shared notes"));
    assert!(out.contains("the asker's personal notes"));
}

#[test]
fn scrubs_the_root_tool_names_and_folders() {
    assert_eq!(
        scrub("I keep it in /memory/ and use memory_write to save.", ""),
        "I keep it in memory and use my memory to save."
    );
    assert_eq!(
        scrub("I'd call memory_append then memory_read.", ""),
        "I'd call my memory then my memory."
    );
    assert_eq!(
        scrub("Patterns live in /memory/integrations/github.md.", ""),
        "Patterns live in the integration notes."
    );
}

#[test]
fn leaves_a_repository_link_a_longer_name_and_the_persons_words() {
    let repo = "The client is at https://github.com/acme/app/tree/main/memory/store.ts.";
    assert_eq!(scrub(repo, ""), repo);
    assert!(
        scrub(
            "Yes, /memory/workspace.md holds that.",
            "what is in /memory/workspace.md?"
        )
        .contains("/memory/workspace.md")
    );
    assert!(
        scrub("memory_read reads a file.", "what does memory_read do?").contains("memory_read")
    );
    let plain = "That feature improves memory usage; nothing is stored about you.";
    assert!(matches!(
        scrub_memory_layout(plain, "", &LAYOUT),
        std::borrow::Cow::Borrowed(_)
    ));
    assert_eq!(
        scrub("Team notes live in workspace.md.", ""),
        "Team notes live in the team's shared notes."
    );
    let files = "Setup is in docs/workspace.md and old-workspace.md.bak.";
    assert_eq!(scrub(files, ""), files);
}

#[test]
fn scrubs_after_a_character_whose_lowercase_is_longer() {
    assert_eq!(
        scrub("İyi akşamlar! Kayıt: /memory/workspace.md içinde.", ""),
        "İyi akşamlar! Kayıt: the team's shared notes içinde.",
    );
    // The Kelvin sign lowercases to `k`, as the host reads it.
    assert_eq!(
        scrub("in WORKSPACE.MD now", ""),
        "in the team's shared notes now"
    );
}
