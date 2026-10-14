use figures_parser::ungrounded_percentages;

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
fn a_bare_number_never_grounds_a_percentage() {
    let ratio = format!("{CPLS}\nratio: 42.67");
    assert_eq!(
        ungrounded_percentages("Sign-ups costs 43% more per lead.", &ratio),
        ["43%"]
    );
    assert_eq!(
        ungrounded_percentages("30% of leads converted.", "leads: 30"),
        ["30%"]
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
    // One place is too few to stand for a ratio, and a value above one is not a fraction.
    assert_eq!(
        ungrounded_percentages("Overall 14.3%.", "rate 0.3, ratio 1.43"),
        ["14.3%"]
    );
    assert_eq!(
        ungrounded_percentages("Half of them, 50%, converted.", "share: 0.5"),
        ["50%"]
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
    assert_eq!(
        ungrounded_percentages("Down 555 %.", "-385.555%"),
        ["555 %"]
    );
    // The last of numbers a run sets side by side is the one marked.
    assert_eq!(
        ungrounded_percentages("Up 26117%.", "15977,10140,26117%"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_percentages("In 2026 25% churned.", ""), ["25%"]);
}
