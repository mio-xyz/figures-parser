//! What the host's TypeScript checks read and flag, which the library reads
//! and flags too, and the inputs where it deliberately reads them otherwise.

use figures_parser::{
    Amounts, ListedFigure, SheetFindings, SheetRun, numeric_tokens, printed_figures,
    sheet_findings, stated_figures, states_figure, ungrounded_figures, ungrounded_ratios,
    ungrounded_ratios_with, without_links,
};

const fn none() -> Vec<String> {
    Vec::new()
}

#[test]
fn reads_the_scale_abbreviations_of_every_locale_the_host_reads() {
    for (reply, corpus) in [
        ("Pendapatan naik 35 rb.", "35000"),
        ("Pendapatan Rp 5 jt.", "5000000"),
        ("Doanh thu 79,7 tr.", "79700000"),
        ("Doanh thu 5 nt.", "5000000000000"),
        ("Doanh thu 2 nghìn tỷ.", "2000000000000"),
        ("Revenue ₹5 L.", "500000"),
        ("Revenue 3 lcr.", "3000000000000"),
        ("Receita R$ 5,6 tri.", "5600000000000"),
        ("Receita 52,2 bi.", "52200000000"),
        ("Gelir ₺5 bin.", "5000"),
        ("Gelir 741 Mr ₺.", "741000000000"),
        ("Gelir 79 mn ₺.", "79000000"),
        ("CA 2 Md.", "2000000000"),
        ("Revenue 2,045Mn.", "2045000000"),
        ("Fatturato 5 mille miliardi.", "5000000000000"),
        ("Revenue 4,732 Cr.", "47320000000"),
        ("Revenue 5 MLN.", "5000000"),
        ("राजस्व 5 अ\u{970}", "5000000000"),
    ] {
        assert_eq!(ungrounded_figures(reply, corpus), none(), "{reply}");
        assert_eq!(
            ungrounded_figures(reply, "1"),
            ungrounded_figures(reply, ""),
            "{reply}"
        );
        assert_eq!(ungrounded_figures(reply, "").len(), 1, "{reply}");
    }
}

#[test]
fn reads_a_name_at_every_power_where_it_is_written_as_the_locales_that_give_it_one() {
    // A currency or an "of" after the scale is continental writing.
    for (reply, corpus) in [
        ("Une dette de 1 billion d'euros.", "1000000000000"),
        ("Une dette de 1 billion d'euros.", "1000000000"),
        ("Dette 2 bn €.", "2000000000000"),
        ("Omsättning 2 tn €.", "2000"),
        ("Gelir 5 B ₺.", "5000"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            none(),
            "{reply} | {corpus}"
        );
    }
    // English puts the currency first, and reads its own scale.
    assert_eq!(
        ungrounded_figures("Debt $2 billion.", "debt 2000000000000"),
        ["$2 billion"]
    );
    assert_eq!(ungrounded_figures("Raised 2tn.", "raised 2000"), ["2tn"]);
}

#[test]
fn reads_a_corpus_name_at_every_power_a_locale_gives_it() {
    for (reply, corpus) in [
        ("Debt 2,000,000,000,000.", "dette 2 billions"),
        ("Debt 2,000,000,000,000.", "debt 2 bn"),
        ("Revenue 2,000.", "omsättning 2 tn"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            none(),
            "{reply} | {corpus}"
        );
    }
}

#[test]
fn holds_a_figure_a_space_parts_from_its_han_multiplier_to_the_multiplier() {
    assert_eq!(ungrounded_figures("売上 4,813 万", "4,813"), ["4,813 万"]);
    assert_eq!(ungrounded_figures("売上 4,813 万", "48130000"), none());
    assert_eq!(ungrounded_figures("매출 87 억 원", "8700000000"), none());
}

#[test]
fn names_a_scaled_figure_with_the_currency_after_it() {
    assert_eq!(ungrounded_figures("Reserve 500 k€.", "fee 500"), ["500 k€"]);
    assert_eq!(ungrounded_figures("CA 38.08M€.", "38.08"), ["38.08M€"]);
}

#[test]
fn reads_the_scale_words_of_a_figure_once() {
    assert_eq!(
        ungrounded_figures("Deuda de 78.78 mil millones.", ""),
        ["78.78 mil millones"]
    );
    assert_eq!(
        ungrounded_figures("Debito di 51 mille miliardi.", ""),
        ["51 mille miliardi"]
    );
}

#[test]
fn rounds_a_printed_value_to_an_approximate_figure_as_to_an_exact_one() {
    assert_eq!(ungrounded_figures("Paid ~$1,000.7.", "1001.7"), none());
    assert_eq!(
        ungrounded_figures("Paid ~$1,000.7.", "1061.7"),
        ["~$1,000.7"]
    );
    // A scaled figure keeps the place it is written to.
    assert_eq!(ungrounded_figures("CA €34,0M.", "34300000"), ["€34,0M"]);
}

#[test]
fn reads_a_zero_lead_as_a_decimal() {
    assert_eq!(ungrounded_figures("Paid $0,850.", ""), none());
    assert_eq!(ungrounded_figures("Paid $0,850.", "850"), none());
}

#[test]
fn gives_a_currency_code_between_two_figures_to_neither() {
    assert_eq!(
        ungrounded_figures("Paid 2,685.20 USD 2026 in total.", "2685.2"),
        none()
    );
    // A sign glued to the next figure is that figure's.
    assert_eq!(ungrounded_figures("Paid 12 $750.", ""), ["750"]);
}

#[test]
fn holds_none_and_all_and_reads_each_percent_sign_once() {
    assert_eq!(
        ungrounded_ratios("Churn 0%, retention 100%.", "", ""),
        ["0%", "100%"]
    );
    assert_eq!(
        ungrounded_ratios("CTR 84 % 8,322 impressions.", "", ""),
        ["84%"]
    );
    assert_eq!(ungrounded_ratios("2027 %95.24 büyüme", "", ""), ["%95.24"]);
    assert_eq!(ungrounded_ratios("Up 12 percent.", "", ""), ["12 percent"]);
    assert_eq!(ungrounded_ratios("Up % $45 today.", "", ""), none());
    assert_eq!(
        ungrounded_ratios("Total 2,343\n% of it came in.", "", ""),
        none()
    );
}

#[test]
fn reads_the_persons_words_only_when_a_ratio_is_left() {
    let mut asked = 0;
    let mut said = || {
        asked += 1;
        "1000 trials, 420 activated"
    };
    assert_eq!(
        ungrounded_ratios_with("58% never activate.", "", &mut said),
        none()
    );
    assert_eq!(
        ungrounded_ratios_with("Overall 14.3%.", "rate = 0.1429", &mut said),
        none()
    );
    assert_eq!(ungrounded_ratios_with("No ratio.", "", &mut said), none());
    assert_eq!(asked, 1);
}

#[test]
fn shares_a_scaled_amount_by_its_digits_as_printed() {
    let amounts = Amounts::of("Pendapatan USD 9.162 jt dan 12,500 users.");
    assert_eq!(amounts.shared_with("total 9.162"), ["9.162 jt"]);
    assert_eq!(amounts.shared_with("total 9162000000"), ["9.162 jt"]);
    assert_eq!(amounts.shared_with("total 9162000"), Vec::<&str>::new());
    assert!(states_figure("revenue 57.37M", "57.37"));
    assert!(states_figure("revenue 1.2 million", "1,200,000"));
}

#[test]
fn reads_the_figures_a_clause_states_and_a_line_prints() {
    let stated: Vec<_> =
        stated_figures("USD 6 951 spend, 3 leads, 12% CTR, $1.2M budget, 1,250 rows")
            .iter()
            .map(|f| (f.mark, f.value))
            .collect();
    assert_eq!(stated, [("USD", 6951.0), ("", 3.0), ("", 1250.0)]);
    let printed = printed_figures("v2.5 | $1,250 | 7.5");
    assert_eq!(printed.values, [1250.0, 1.25, 1.0, 250.0, 7.5]);
    assert_eq!(printed.amounts, [1250.0, 1.25, 1.0, 7.5]);
    let tokens: Vec<_> = numeric_tokens("v2.5 took 90s, 1 250 rows", false)
        .into_iter()
        .map(|t| t.values)
        .collect();
    assert_eq!(tokens, [vec![90.0], vec![1250.0]]);
    assert_eq!(without_links("x https://a.io/1 y"), "x   y");
}

#[test]
fn walks_the_runs_a_reply_drew_a_sheets_amounts_from() {
    let listed = [ListedFigure {
        figure: "19,851",
        name: "row 6 TOTAL: 19,851 not counted",
    }];
    let reply = "2027 holds $19,850 and $1,458 in fees.";
    let checked = SheetRun {
        stdout: "Total | 19,850",
        lists_its_rows: true,
        listed: &listed,
        ..SheetRun::default()
    };
    let by_hand = SheetRun {
        stdout: "sum | 19,850 | fees | 1,458",
        grid_read: true,
        ..SheetRun::default()
    };
    let found = sheet_findings(reply, &[checked, by_hand]);
    assert_eq!(found.unstated, listed);
    assert_eq!(found.unchecked, ["1,458"]);
    assert!(!found.framed);
    // A run that touched no worksheet holds the reply to nothing.
    let plain = SheetRun {
        stdout: "19,850 | 1,458",
        ..SheetRun::default()
    };
    assert_eq!(sheet_findings(reply, &[plain]), SheetFindings::default());
}

// The inputs below are read otherwise than the host's TypeScript reads
// them, where it misreads the text; docs/guarantees.md gives each reason.

#[test]
fn reads_a_decimal_mark_before_a_scale_word_whose_language_writes_it() {
    assert_eq!(ungrounded_figures("Долг 8,566 млрд.", "8566000000"), none());
    assert_eq!(
        ungrounded_figures("Revenue of 1.250 million.", "1250000"),
        none()
    );
    // Before a letter every locale writes alike, the run is thousands, as
    // the host reads it.
    assert_eq!(
        ungrounded_figures("Budget 1,250 M€.", "budget 1250000"),
        ["1,250 M€"]
    );
}

#[test]
fn reads_no_scale_that_moves_a_decimal_back_onto_the_printed_digits() {
    for (reply, corpus) in [
        ("Revenue 9.007 tn €", "x 9007"),
        ("CHF 9.007 tn EUR", "x 9007"),
        ("Revenue 9,007 tn €", "x 9007"),
        ("Total 46,886b €", "{amount: 46886}"),
        ("Total 46,886b€", "{amount: 46886}"),
        ("Total 1,001b$", "1,001"),
        ("Total 46,886k €", "46886"),
    ] {
        assert_eq!(ungrounded_figures(reply, corpus).len(), 1, "{reply}");
    }
}

#[test]
fn reads_a_compound_scale_a_plain_space_joins() {
    assert_eq!(ungrounded_figures("2 lakh crore", "2000000000000"), none());
    assert_eq!(
        ungrounded_figures("2 lakh crore", "200000"),
        ["2 lakh crore"]
    );
}

#[test]
fn holds_a_figure_whose_digits_a_scaled_figure_beside_it_shares() {
    assert_eq!(
        ungrounded_figures("Paid $500 of a 500 k€ budget.", "budget 500000"),
        ["500"]
    );
}

#[test]
fn reads_a_plain_space_only_into_thousands_in_a_corpus() {
    // Two values a script printed side by side are two values.
    assert_eq!(
        ungrounded_figures("Revenue $12,480.", "sessions 3442 12,480"),
        none()
    );
    assert_eq!(ungrounded_ratios("CTR 57.44x.", "", ""), ["57.44x"]);
    assert_eq!(ungrounded_ratios("95.11 57.44x", "57.44", ""), none());
}

#[test]
fn reads_an_approximation_mark_before_a_currency_written_with_letters() {
    for (reply, corpus) in [
        ("Revenue ≈ R$3,000.", "3,100"),
        ("Paid ~US$882,915.", "882,916"),
        ("Paid ≈ EUR 466,841.", "466,842"),
    ] {
        assert_eq!(ungrounded_figures(reply, corpus), none(), "{reply}");
    }
}

#[test]
fn reads_no_figure_in_the_piece_of_a_version_or_a_hex_literal() {
    assert_eq!(printed_figures("build v2.5").values, Vec::<f64>::new());
    assert!(
        numeric_tokens("0x66.32", false)
            .iter()
            .all(|t| t.values != [32.0])
    );
}

#[test]
fn reads_a_scale_abbreviation_that_is_also_a_word_or_a_unit_only_beside_a_currency() {
    // Each is held to its digits as written, which a scaled reading is not.
    for (reply, digits) in [
        ("The job took 5m.", ""),
        ("A 2481t truck.", ""),
        ("300l of water.", ""),
        ("5 bin bags.", ""),
        ("Ran 5 mi.", ""),
        ("Took 165489 mn.", ""),
        ("Earned 92 cr this term.", ""),
        ("Balance 3,000 CR.", "3000"),
        ("Wrote 5 md files.", ""),
        ("Ate 899'404 Bio eggs.", "899404"),
    ] {
        assert_eq!(ungrounded_figures(reply, digits), none(), "{reply}");
    }
    for (reply, digits) in [
        ("Paid $5m.", "5"),
        ("Rp 300l ₹.", "300"),
        ("Gelir ₺5 bin.", "5"),
        ("R$ 5 mi.", "5"),
        ("Sent 8 004 Mio. €", "8004"),
        ("Balance $3,000 CR.", "3000"),
        ("Sent 8 004 Mio messages.", "8004"),
        ("Gelir 5 bin TL.", "5"),
        ("Revenue Rs 92 cr.", "92"),
        ("Umsatz 5 Bio Euro.", "5"),
        ("Revenue 3.2m dollars.", "3.2"),
    ] {
        assert_eq!(ungrounded_figures(reply, digits).len(), 1, "{reply}");
    }
}
