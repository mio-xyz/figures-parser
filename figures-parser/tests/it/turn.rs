//! The sheet checks, the memory a reply draws on, and the checks a turn runs
//! on a reply, a sent message and a page.

use figures_parser::{
    Date, InternalsCheck, ListedFigure, MemoryLayout, SheetRun, Turn, Vocabulary, ground_message,
    ground_page, ground_reply, grounding_tokens, memory_grounding, sheet_findings,
};

/// What a check that finds nothing returns.
const NONE: [&str; 0] = [];

/// No listed rows.
const NO_ROWS: [ListedFigure<'static>; 0] = [];

const BY_MONTH: &str =
    "By month and channel:\n2026-09 Airbnb 15977.0\n2026-09 Direct 10140.0\n2026-12 Direct 9133.0";
const REPLY: &str = "September: Direct $10,140, Airbnb $15,977. December: Direct $9,133.";

const fn hand_sum(framing: bool, lists_its_rows: bool, grid_read: bool) -> SheetRun<'static> {
    SheetRun {
        stdout: BY_MONTH,
        lists_its_rows,
        framing,
        grid_read,
        listed: &[],
    }
}

#[test]
fn lists_the_rows_a_reply_draws_on_and_leaves_unstated() {
    let total = [ListedFigure {
        figure: "19,851",
        name: "row 6 TOTAL",
    }];
    let fee = [ListedFigure {
        figure: "1,458",
        name: "row 65 fee",
    }];
    let runs = [
        SheetRun {
            stdout: "totals: Total 19,850",
            listed: &total,
            ..SheetRun::default()
        },
        SheetRun {
            stdout: "| 2026-09 | 17,435 |",
            listed: &fee,
            ..SheetRun::default()
        },
        SheetRun {
            stdout: "totals: Total 19,850",
            listed: &total,
            ..SheetRun::default()
        },
    ];
    let both = sheet_findings("September Airbnb: $17,435. 2027: $19,850.", &runs);
    assert_eq!(both.listed, [total[0], fee[0]]);
    assert_eq!(both.unstated, [total[0], fee[0]]);
    assert_eq!(sheet_findings("2027: $19,850.", &runs).listed, total);
    assert_eq!(
        sheet_findings("Which tab do you mean?", &runs).listed,
        NO_ROWS
    );
    let stated = "2027: $19,850; the TOTAL row says 19851. The $1,458 fee is counted.";
    assert_eq!(sheet_findings(stated, &runs).unstated, NO_ROWS);
}

#[test]
fn holds_amounts_from_a_frame_summed_by_hand() {
    let found = sheet_findings(REPLY, &[hand_sum(true, false, true)]);
    assert_eq!(
        (found.unchecked, found.framed),
        (vec!["10,140", "15,977", "9,133"], true)
    );
    let snapshot = SheetRun {
        stdout: "snapshot written: 4 tabs",
        grid_read: true,
        ..SheetRun::default()
    };
    let after_snapshot = sheet_findings(REPLY, &[snapshot, hand_sum(true, false, false)]);
    assert_eq!(
        (after_snapshot.unchecked, after_snapshot.framed),
        (vec!["10,140", "15,977", "9,133"], true)
    );
    let one = sheet_findings("September Direct: $10,140.", &[hand_sum(true, false, true)]);
    assert_eq!(one.unchecked, ["10,140"]);
}

#[test]
fn holds_no_frame_on_a_turn_that_read_no_sheet() {
    let prices = SheetRun {
        stdout: "Buy & Hold  $16,074,999\nSMA 50/200  $353,501",
        framing: true,
        ..SheetRun::default()
    };
    let found = sheet_findings(
        "Buy and hold ends at $16,074,999; the crossover at $353,501.",
        &[prices],
    );
    assert!(found.unchecked.is_empty() && !found.framed);
}

#[test]
fn holds_two_amounts_from_a_tab_read_with_no_helper_and_lets_one_through() {
    let found = sheet_findings(REPLY, &[hand_sum(false, false, true)]);
    assert_eq!(
        (found.unchecked, found.framed),
        (vec!["10,140", "15,977", "9,133"], false)
    );
    assert_eq!(
        sheet_findings(
            "The September Direct line is $10,140.",
            &[hand_sum(false, false, true)]
        )
        .unchecked,
        NONE
    );
    assert_eq!(
        sheet_findings(REPLY, &[hand_sum(false, false, false)]).unchecked,
        NONE
    );
}

#[test]
fn releases_an_amount_a_checking_run_printed_whichever_came_first() {
    assert_eq!(
        sheet_findings(REPLY, &[hand_sum(true, true, true)]).unchecked,
        NONE
    );
    let checked = SheetRun {
        stdout: "2026-09 Direct 10,140 | Airbnb 15,977\n2026-12 Direct 9,133",
        lists_its_rows: true,
        ..SheetRun::default()
    };
    assert_eq!(
        sheet_findings(REPLY, &[hand_sum(true, false, true), checked].clone()).unchecked,
        NONE
    );
    assert_eq!(
        sheet_findings(REPLY, &[checked, hand_sum(false, false, true)]).unchecked,
        NONE
    );
    let partly = SheetRun {
        stdout: "2026-09 Direct 10,140",
        lists_its_rows: true,
        grid_read: true,
        ..SheetRun::default()
    };
    let found = sheet_findings(REPLY, &[hand_sum(true, false, true), partly]);
    assert_eq!(
        (found.unchecked, found.framed),
        (vec!["15,977", "9,133"], true)
    );
}

#[test]
fn holds_no_ratio_from_a_sheet() {
    let backtest = SheetRun {
        stdout: "Total Return: 7789.32%\nBuy & Hold Return: 220491.24%\nStart value: 10000\nEnd value: 788932.0",
        grid_read: true,
        ..SheetRun::default()
    };
    let reply = "Total return **7,789%**; buy and hold **220,491%**, about 28.3x the strategy.";
    assert_eq!(sheet_findings(reply, &[backtest]).unchecked, NONE);
    let with_amounts = format!("$10,000 grew to $788,932. {reply}");
    assert_eq!(
        sheet_findings(&with_amounts, &[backtest]).unchecked,
        ["10,000", "788,932"]
    );
}

const WORKSPACE: &str = "- Weekly standup is every Tuesday at 10:00 CET in #standup\n- *Roster*: Priya (@priya) — GTM lead, owns the newsletter [derived 2026-09-01]\n- Deploys go out on Thursday afternoons";

#[test]
fn reads_the_distinctive_words_of_a_line() {
    assert_eq!(
        grounding_tokens("- *Roster*: Priya — GTM lead at 10:00 [derived 2026-09-01]"),
        ["roster", "priya", "gtm", "lead", "10:00"]
    );
    assert_eq!(grounding_tokens("- Owner: Priya"), ["owner", "priya"]);
    assert_eq!(
        grounding_tokens("standup at 10:00 CET"),
        ["standup", "10:00", "cet"]
    );
    // A letter outside the Basic Multilingual Plane counts twice, as the host counts it.
    assert_eq!(grounding_tokens("𝒜𝒜 ab"), ["𝒜𝒜"]);
}

#[test]
fn counts_the_lines_a_reply_draws_on() {
    let drawn = memory_grounding(
        "The standup is Tuesday at 10:00 CET, in #standup as usual.",
        &[WORKSPACE],
    );
    assert_eq!(
        (drawn.preloaded_lines, drawn.grounded_lines, drawn.grounded),
        (3, 1, true)
    );
    assert_eq!(
        memory_grounding("Priya owns the newsletter, so ask her.", &[WORKSPACE]).grounded_lines,
        1
    );
    let unrelated = memory_grounding(
        "Sure, I'll take a look at the weekly numbers tomorrow.",
        &[WORKSPACE],
    );
    assert_eq!(
        (unrelated.preloaded_lines, unrelated.grounded_lines),
        (3, 0)
    );
    assert_eq!(
        memory_grounding::<&str>("Tuesday at 10:00", &[]).preloaded_lines,
        0
    );
}

const LAYOUT: MemoryLayout<'static> = MemoryLayout {
    root: "/memory/",
    tool_prefix: "memory_",
    folders: &[("integrations/", "the integration notes")],
    files: &[("workspace.md", "the team's shared notes")],
    root_phrase: "memory",
    other_phrase: "the team's notes",
    tool_phrase: "my memory",
};

const RUNS_PRINTED: &str = "Current date and time: Tuesday, 2026-10-20 07:00 UTC.\ndeploy-prod-all.yml: run 39500776500 completed success (https://github.com/acme/app/actions/runs/39500776500)";

fn turn<'a>(corpus: &'a str, internals: Option<&'a InternalsCheck<'a>>) -> Turn<'a> {
    Turn {
        corpus,
        prompt: "",
        said: "",
        today: Date::new(2026, 10, 20).unwrap(),
        pixel_only: false,
        hosts: &[],
        attached: &|_| false,
        layout: &LAYOUT,
        internals,
        vouching: "",
        runs: &[],
    }
}

fn keys(text: &str, turn: &Turn<'_>) -> Vec<String> {
    ground_message(text, turn)
        .findings
        .iter()
        .map(figures_parser::Finding::key)
        .collect()
}

#[test]
fn passes_a_message_a_result_printed() {
    let text = "Deploy run 39500776500 succeeded: <https://github.com/acme/app/actions/runs/39500776500|run>";
    let grounding = ground_message(text, &turn(RUNS_PRINTED, None));
    assert!(grounding.is_clean());
    assert_eq!(grounding.text, text);
}

#[test]
fn keys_each_finding_by_kind() {
    let corpus = "revenue 194,718\nspend 101,834\ncos 0.523";
    assert_eq!(
        keys(
            "COS 61,0% this week on revenue of $188,027",
            &turn(corpus, None)
        ),
        ["figure:188,027", "ratio:61,0%"]
    );
    assert_eq!(
        keys("COS 52,3% on revenue of $194,718", &turn(corpus, None)),
        NONE
    );
    assert_eq!(
        keys(
            "Created prod_VU49BpM22sWxIM.",
            &turn("Error: No such price: 'price_1Q'", None)
        ),
        ["id:prod_VU49BpM22sWxIM"]
    );
    let mixed = keys(
        "Tue Oct 19: COS 52,3%, see <https://github.com/acme/app/pull/3998|#3998>",
        &turn(RUNS_PRINTED, None),
    );
    let kinds: Vec<&str> = mixed.iter().map(|k| k.split(':').next().unwrap()).collect();
    assert_eq!(kinds, ["link", "ratio", "weekday"]);
}

#[test]
fn checks_a_per_unit_amount_and_a_window() {
    let ads = "Rework | LT: $91.65 / 2 leads / CPL $45.83 | Y: $74.74 / 2 leads";
    assert_eq!(
        keys(
            "Rework: $74.74 spend, 2 leads, $45.83 CPL.",
            &turn(ads, None)
        ),
        ["per-unit:$45.83"]
    );
    assert_eq!(
        keys(
            "Rework: $74.74 spend, 2 leads, $37.37 CPL.",
            &turn(ads, None)
        ),
        NONE
    );
    let posts = "*Posts this week (Oct 14-20)*\n- Oct 19: launch post\n- Oct 12: older post";
    assert_eq!(
        keys(posts, &turn("", None)),
        [r#"window:"Oct 12" under "Oct 14-20""#]
    );
}

#[test]
fn passes_an_approximate_figure_that_rounds_a_printed_one() {
    assert_eq!(
        keys("~3,440 sessions this week", &turn("sessions 3442", None)),
        NONE
    );
}

#[test]
fn mends_and_scrubs_before_it_checks() {
    let read = "[09:00] dana: hi https://acme.slack.com/archives/C1/p1756800600000000";
    let grounding = ground_message(
        "<https://acme.slack.com/archives/C1/p1756800600.000000|Dana asked>",
        &turn(read, None),
    );
    assert!(grounding.is_clean());
    assert_eq!(
        grounding.text,
        "<https://acme.slack.com/archives/C1/p1756800600000000|Dana asked>"
    );
    assert_eq!(grounding.mends.len(), 1);
    let scrubbed = ground_message(
        "Patterns live in /memory/integrations/github.md.",
        &turn("", None),
    );
    assert!(scrubbed.is_clean());
    assert_eq!(scrubbed.text, "Patterns live in the integration notes.");
    let names: &[&str] = &["run_code"];
    let check = InternalsCheck::new(
        "",
        &Vocabulary {
            names,
            files: &["workspace.md"],
            roots: &["/memory/"],
            fixed: &[],
        },
    );
    let leak = ground_message(
        "I ran run_code over /memory/integrations/github.md.",
        &turn("", Some(&check)),
    );
    assert_eq!(
        leak.findings
            .iter()
            .map(figures_parser::Finding::key)
            .collect::<Vec<_>>(),
        ["internal:run_code"]
    );
}

#[test]
fn leaves_figures_ratios_and_ids_unchecked_on_pixels() {
    let mut pixels = turn("", None);
    pixels.pixel_only = true;
    assert!(
        ground_message(
            "COS 52,3%, revenue $188,027, order prod_VU49BpM22sWxIM",
            &pixels
        )
        .is_clean()
    );
    assert_eq!(
        ground_page("activation up 12% on 1,840 sign-ups", "", "", true),
        NONE
    );
}

#[test]
fn holds_a_reply_to_its_attached_files_and_listed_rows() {
    let attached = |target: &str| target == "chart.png";
    let mut check = turn("", None);
    check.attached = &attached;
    let reply = ground_reply("![chart](chart.png) and ![x](chart.png) [PR](url)", &check);
    let keys: Vec<String> = reply
        .findings
        .iter()
        .map(figures_parser::Finding::key)
        .collect();
    assert_eq!(keys, ["placeholder:(url)", "attached:chart.png"]);
    assert!(ground_message("![chart](chart.png)", &check).is_clean());
    let total = [ListedFigure {
        figure: "19,851",
        name: "row 6 TOTAL",
    }];
    let runs = [SheetRun {
        stdout: "Total 19,850",
        listed: &total,
        ..SheetRun::default()
    }];
    let mut sheets = turn("Total 19,850", None);
    sheets.runs = &runs;
    let unstated: Vec<String> = ground_reply("2027: $19,850.", &sheets)
        .findings
        .iter()
        .map(figures_parser::Finding::key)
        .collect();
    assert_eq!(unstated, ["unstated:19,851"]);
    assert!(ground_message("2027: $19,850.", &sheets).is_clean());
    // The reply's layout is the rewrite's to fix, not mended in place.
    assert_eq!(
        ground_reply("see /memory/", &turn("", None)).text,
        "see /memory/"
    );
}

#[test]
fn holds_a_page_to_its_figures_ratios_and_ids_only() {
    let page = "Sprint 42: onboarding shipped, activation up 12% on 1,840 sign-ups. Unsubscribe at https://quesly.example/u";
    let mut found = ground_page(page, "", "", false);
    found.sort();
    assert_eq!(found, ["1,840", "12%"]);
    assert_eq!(
        ground_page(page, "signups 1840\nactivation_change 0.12", "", false),
        NONE
    );
}
