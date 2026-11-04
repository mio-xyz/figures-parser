use figures_parser::{MemoryGrounding, grounding_tokens, memory_grounding};

const WORKSPACE: &str = "- Weekly standup is every Tuesday at 10:00 CET in #standup\n\
    - *Roster*: Priya (@priya) — GTM lead, owns the newsletter [derived 2026-09-01]\n\
    - Deploys go out on Thursday afternoons";

const fn read(preloaded_lines: usize, grounded_lines: usize) -> MemoryGrounding {
    MemoryGrounding {
        preloaded_lines,
        grounded_lines,
        grounded: grounded_lines > 0,
    }
}

#[test]
fn drops_markers_markdown_and_stopwords_and_keeps_times_and_names() {
    assert_eq!(
        grounding_tokens("- *Roster*: Priya — GTM lead at 10:00 [derived 2026-09-01]"),
        ["roster", "priya", "gtm", "lead", "10:00"]
    );
    assert_eq!(grounding_tokens("- Owner: Priya"), ["owner", "priya"]);
    assert_eq!(
        grounding_tokens("standup at 10:00 CET"),
        ["standup", "10:00", "cet"]
    );
    // Letters and numbers of any script, lowercased; a short one is dropped.
    assert_eq!(
        grounding_tokens("ÉQUIPE 日本語 ٢٠٢٦ 10"),
        ["équipe", "日本語", "٢٠٢٦"]
    );
}

#[test]
fn counts_a_line_the_reply_draws_on_by_overlap_or_three_words_in_a_row() {
    assert_eq!(
        memory_grounding(
            "The standup is Tuesday at 10:00 CET, in #standup as usual.",
            &[WORKSPACE]
        ),
        read(3, 1)
    );
    assert_eq!(
        memory_grounding("Priya owns the newsletter, so ask her.", &[WORKSPACE]).grounded_lines,
        1
    );
    assert_eq!(
        memory_grounding(
            "Ask the owner Priya about it.",
            &["- Owner: Priya\n- Billing: Dana"]
        ),
        read(2, 1)
    );
}

#[test]
fn counts_no_line_that_shares_only_a_common_word() {
    assert_eq!(
        memory_grounding(
            "Sure, I'll take a look at the weekly numbers tomorrow.",
            &[WORKSPACE]
        ),
        read(3, 0)
    );
    let none: [&str; 0] = [];
    assert_eq!(memory_grounding("Tuesday at 10:00", &none), read(0, 0));
}

#[test]
fn skips_headings_and_blank_lines() {
    let company = "# Acme\n\n## ICP\n- Seed-stage B2B SaaS teams of 10-50\n";
    assert_eq!(
        memory_grounding(
            "Our ICP is seed-stage B2B SaaS teams of 10-50 people.",
            &[company]
        ),
        read(1, 1)
    );
}
