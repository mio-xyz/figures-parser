use figures_parser::ungrounded_ratios;

/// The ratio check with nothing said by the person.
fn ungrounded_percentages(reply: &str, corpus: &str) -> Vec<String> {
    ungrounded_ratios(reply, corpus, "")
}

const CPLS: &str = "Sign-ups   $ 229.94   12   $19.16\nAdd2Slack  $ 335.75   25   $13.43";

#[test]
fn a_percentage_printed_with_more_places_carries_the_reply_rounding() {
    let printed = format!("{CPLS}\nSign-ups vs Add2Slack: +42.67%");
    for reply in [
        "Sign-ups costs 43% more per lead.",
        "Sign-ups costs 42.7% more per lead.",
        "Le CPL est 42,7 % plus cher.",
        "Sign-ups costs 42.67% more.",
    ] {
        assert_eq!(
            ungrounded_percentages(reply, &printed),
            Vec::<String>::new(),
            "{reply}"
        );
    }
    // Half a unit of the reply's last place either way, and no further.
    assert_eq!(
        ungrounded_percentages("Up 42.65%.", "change: 42.645%"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Up 42.6%.", "change: 42.67%"),
        ["42.6%"]
    );
    // Nor does a coarser printed value carry a finer stated one.
    assert_eq!(
        ungrounded_percentages("Up 42.67%.", "change: 42.7%"),
        ["42.67%"]
    );
}

#[test]
fn a_bare_whole_number_never_grounds_a_ratio_and_a_printed_decimal_does() {
    assert_eq!(
        ungrounded_percentages("Sign-ups costs 43% more per lead.", "ratio: 43"),
        ["43%"]
    );
    assert_eq!(
        ungrounded_percentages("30% of leads converted.", "leads: 30"),
        ["30%"]
    );
    // A decimal a script printed grounds the ratio it rounds to, as is or a
    // hundred times it, at the places the reply wrote.
    let ratio = format!("{CPLS}\nratio: 42.67");
    assert_eq!(
        ungrounded_percentages("Sign-ups costs 43% more per lead.", &ratio),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("11.3 % of it", "share 11.29"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("about 11%", "share: 0.113"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_percentages("about 11%", "share: 0.117"), ["11%"]);
    // A comma that may group thousands does; a dot is a decimal point.
    assert_eq!(
        ungrounded_percentages("about 19.9x", "Total 19,850"),
        ["19.9x"]
    );
    assert_eq!(
        ungrounded_percentages("about 85%", "share 0,850"),
        Vec::<String>::new()
    );
    // Nor a date's parts, nor a bare figure beside them.
    assert_eq!(
        ungrounded_percentages("about 11%", "booked 2026-11-05, row 11"),
        ["11%"]
    );
}

#[test]
fn a_printed_fraction_grounds_its_percentage() {
    assert_eq!(
        ungrounded_percentages(
            "Overall 14.3% (100 / 700), East best at 24%.",
            "overall_rate = 0.1429\neast = 0.24"
        ),
        Vec::<String>::new()
    );
    // A decimal grounds only what it rounds to, as is or a hundred times it.
    assert_eq!(
        ungrounded_percentages("Overall 14.3%.", "rate 0.3, ratio 1.43"),
        ["14.3%"]
    );
    assert_eq!(
        ungrounded_percentages("Half of them, 50%, converted.", "share: 0.5"),
        Vec::<String>::new()
    );
    // In any notation the reader knows.
    assert_eq!(
        ungrounded_percentages("Overall 14,3 %.", "taux = 0,1429"),
        Vec::<String>::new()
    );
}

#[test]
fn the_size_is_compared_and_the_sign_is_reported() {
    assert_eq!(
        ungrounded_percentages("Spend fell 30%.", "spend change: -29.9%"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Spend moved -3.2%.", "change: 4%"),
        ["-3.2%"]
    );
}

#[test]
fn none_and_all_are_never_held() {
    assert_eq!(
        ungrounded_percentages(
            "0% of the Brain leads came in, and 100% of the spend is on Add2Slack; 100.0% done.",
            CPLS
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_sign_or_a_word_that_does_not_end_the_figure_is_no_percentage() {
    assert_eq!(
        ungrounded_percentages("See https://example.com/report?share=50%25.", CPLS),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Use code 50%off, up 12 percentage points.", CPLS),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Build v12% and id x50%", ""),
        Vec::<String>::new()
    );
}

#[test]
fn reads_every_percent_sign_and_word() {
    let corpus = "growth: 38%";
    for reply in [
        "Up 38%.",
        "Up 38 %.",
        "Up 38\u{a0}%.",
        "Up 38\u{202f}%.",
        "زيادة ٣٨٪ هذا العام",
        "增长３８％。",
        "Up 38\u{fe6a}.",
        "Up 38 percent.",
        "Up 38 per cent.",
        "En hausse de 38 pour cent.",
        "Plus 38 Prozent.",
        "Sube 38 por ciento.",
        "Sobe 38 por cento.",
        "Sale del 38 per cento.",
    ] {
        assert_eq!(
            ungrounded_percentages(reply, corpus),
            Vec::<String>::new(),
            "{reply}"
        );
        assert_eq!(
            ungrounded_percentages(reply, "growth: 39%").len(),
            1,
            "{reply}"
        );
    }
    // The corpus is read the same way.
    assert_eq!(
        ungrounded_percentages("Up 38%.", "croissance de 38 pour cent"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Up 38%.", "النمو ٣٨٪"),
        Vec::<String>::new()
    );
}

#[test]
fn an_ambiguous_figure_is_grounded_by_either_reading() {
    // "1,250%" is 1250 in English and 1.25 in German.
    assert_eq!(
        ungrounded_percentages("Up 1,250%.", "change: 1.25%"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("Up 1,250%.", "change: 1250%"),
        Vec::<String>::new()
    );
}

#[test]
fn each_percentage_is_named_once_as_written() {
    assert_eq!(
        ungrounded_percentages("Up 12%, then 12% again, then ~15%.", ""),
        ["12%", "~15%"]
    );
}

#[test]
fn a_reply_without_a_percentage_reads_no_corpus() {
    assert_eq!(
        ungrounded_percentages("Revenue was 1,250,000.", "anything 0.5"),
        Vec::<String>::new()
    );
}

#[test]
fn a_group_of_a_printed_figure_grounds_no_percentage() {
    assert_eq!(ungrounded_percentages("Up 4%.", "rate 4,552,788%"), ["4%"]);
    assert_eq!(ungrounded_percentages("Down 555 %.", "-385.555%"), ["555%"]);
    // The last of numbers a run sets side by side is the one marked.
    assert_eq!(
        ungrounded_percentages("Up 26117%.", "15977,10140,26117%"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_percentages("In 2026 25% churned.", ""), ["25%"]);
}

const PRINTED: &str = "2026: Total 175,800 (secured by 2026-10-10)\n2027: Total 19,850, row 6 TOTAL 19,851, dated 11/12";

#[test]
fn flags_a_multiple_or_a_percentage_worked_out_in_prose_from_printed_amounts() {
    assert_eq!(
        ungrounded_percentages("2026 is running at roughly 8.9x the 2027 figure.", PRINTED),
        ["8.9x"]
    );
    assert_eq!(
        ungrounded_percentages("2027 is at roughly 11% of 2026.", PRINTED),
        ["11%"]
    );
    assert_eq!(
        ungrounded_percentages("down 88.7% year over year", PRINTED),
        ["88.7%"]
    );
    // A multiple a result printed, or a decimal it rounds to, grounds it.
    assert_eq!(
        ungrounded_percentages("roughly 8.9x", &format!("{PRINTED}\nratio: 8.857")),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("about 1,250x faster", "speedup: 1250x"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("about 1,250,000x faster", "speedup: 1250000x"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("about 1,250,000x faster", ""),
        ["1,250,000x"]
    );
    assert_eq!(
        ungrounded_percentages("roughly 8.9X the 2027 figure, 3× more", ""),
        ["8.9X", "3×"]
    );
}

#[test]
fn reads_no_ratio_in_a_product_a_size_a_hex_literal_or_a_version() {
    assert_eq!(
        ungrounded_percentages("a 3 x 4 grid at 1920x1080, mask 0x1F, v2.5x", PRINTED),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("at 1920X1080, 0X1F", ""),
        Vec::<String>::new()
    );
}

#[test]
fn grounds_a_ratio_the_persons_own_numbers_give_or_one_they_stated() {
    let said = "make me a Sankey of the funnel: 1000 trials, 420 activated, 180 paying, 90 churned";
    assert_eq!(
        ungrounded_ratios(
            "58% of trials don't activate, 57% don't pay, 50% churn",
            "",
            said
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_ratios("revenue up 12% QoQ", "", "Q3 revenue 150k, up 12 percent"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_ratios("roughly 8.9x", PRINTED, "compare 2026 to 2027"),
        ["8.9x"]
    );
    assert_eq!(
        ungrounded_ratios("roughly 8.9x", "", "2026 did 8857 and 2027 did 1000"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_percent_sign_before_the_figure_as_turkish_writes_it() {
    assert_eq!(ungrounded_percentages("(%11 of them)", ""), ["%11"]);
    assert_eq!(ungrounded_percentages("\"%11\" she said", ""), ["%11"]);
    assert_eq!(
        ungrounded_percentages("Oran %11,5 oldu.", "0.115"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("see https://x.test/a%20b and /x%41", ""),
        Vec::<String>::new()
    );
    // The corpus is read the same way.
    assert_eq!(
        ungrounded_percentages("Up 38%.", "büyüme %38"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_ratio_against_the_letters_of_a_script_written_without_spaces() {
    assert_eq!(ungrounded_percentages("伸び率は50%です。", ""), ["50%"]);
    assert_eq!(
        ungrounded_percentages("伸び率は50%です。", "比率は0.5です"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_percentages("ยอดขายโต12.5%", ""), ["12.5%"]);
    assert_eq!(ungrounded_percentages("前年の8.9x倍", ""), ["8.9x"]);
    assert_eq!(
        ungrounded_percentages("about 50%, v2.5x", "rate0.5"),
        ["50%"]
    );
}

#[test]
fn reads_the_fullwidth_forms_of_a_digit_a_decimal_point_and_a_sign() {
    assert_eq!(
        ungrounded_percentages("增长率为１２．５％", "share: 0.125"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_percentages("增长率为１２．５％", ""),
        ["１２．５％"]
    );
    assert_eq!(ungrounded_percentages("about 50﹪", ""), ["50﹪"]);
}

#[test]
fn names_a_ratio_once_however_its_mark_is_written() {
    assert_eq!(
        ungrounded_percentages(
            "8.9x by one count, 8.9× by another; 11 %, 11% and %11",
            PRINTED
        ),
        ["8.9x", "11%"]
    );
}
