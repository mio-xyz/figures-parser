use figures_parser::ungrounded_figures;

const CORPUS: &str = "run_code: exit 0\nstdout:\nTotal: 101000\nRows: 2000\nARR 1,250,000 in 2026";

const fn none() -> Vec<String> {
    Vec::new()
}

#[test]
fn flags_a_figure_the_turn_never_carried_whatever_its_formatting() {
    assert_eq!(
        ungrounded_figures("**1,010,000** across all 2,000 rows.", CORPUS),
        vec!["1,010,000"]
    );
    assert_eq!(
        ungrounded_figures("**101,000** across all 2,000 rows.", CORPUS),
        none()
    );
    assert_eq!(ungrounded_figures("ARR is 1250000.", CORPUS), none());
}

#[test]
fn flags_a_precise_decimal_no_result_carried_and_leaves_one_a_result_printed() {
    let rates = "EUR/USD — 1.0842 USD per EUR. Day range 1.0815–1.0860.";
    assert_eq!(
        ungrounded_figures("**1 EUR = 1.1785 USD** right now.", rates),
        vec!["1.1785"]
    );
    assert_eq!(
        ungrounded_figures("1 EUR = 1.0842 USD, range 1.0815–1.0860.", rates),
        none()
    );
    assert_eq!(
        ungrounded_figures("About 1.08 USD, up 0.25% on v3.1.", rates),
        none()
    );
}

#[test]
fn leaves_small_numbers_percentages_and_years_alone() {
    assert_eq!(
        ungrounded_figures("225 of 250 open, up 38% since 2026, ~900 orders.", CORPUS),
        none()
    );
}

#[test]
fn flags_an_amount_written_with_a_separator_that_the_model_subtracted_in_prose() {
    let totals = "Direct total: 9133  Airbnb total: 0\nOwen Farrow Direct 12/18-12/20 $1,040\nOwen Farrow Direct 12/17-12/20 $1,913";
    assert_eq!(
        ungrounded_figures(
            "December Direct drops to $8,093 keeping the $1,913 row, or $8,220 keeping $1,040.",
            totals
        ),
        vec!["8,093", "8,220"]
    );
    assert_eq!(
        ungrounded_figures(
            "December Direct is **$9,133**, of which $1,913 is Owen's longer stay.",
            totals
        ),
        none()
    );
}

#[test]
fn is_not_grounded_by_the_inside_of_an_id_or_a_timestamp() {
    assert_eq!(
        ungrounded_figures(
            "That is 8,220 in all.",
            "permalink p1789368220119 and 2,000 rows"
        ),
        vec!["8,220"]
    );
}

#[test]
fn flags_a_bare_figure_written_behind_a_currency_sign_from_100_up() {
    assert_eq!(
        ungrounded_figures(
            "The fee collected was a $729 fee.",
            "Flor Ferro cancelled: $1,458"
        ),
        vec!["729"]
    );
    assert_eq!(
        ungrounded_figures("The fee collected was a €729 fee.", "fee 1,458"),
        vec!["729"]
    );
    assert_eq!(
        ungrounded_figures("That is $729.", "fee 729 collected"),
        none()
    );
    assert_eq!(
        ungrounded_figures("About $12.5K, or $8k a month, $99 fee.", ""),
        vec!["12.5K", "8k"]
    );
}

#[test]
fn checks_a_separated_currency_figure_once() {
    assert_eq!(
        ungrounded_figures("A $1,458 fee.", "nothing printed"),
        vec!["1,458"]
    );
}

#[test]
fn grounds_each_of_two_numbers_printed_side_by_side_on_one_line() {
    assert_eq!(
        ungrounded_figures(
            "Direct is 26,117 and Airbnb 15,977.",
            "run_code: exit 0\nstdout:\n26117 15977\n"
        ),
        none()
    );
    assert_eq!(
        ungrounded_figures("December is 26,117.", "2026-09 15977 10140 26117"),
        none()
    );
}

#[test]
fn grounds_a_figure_printed_inside_a_json_array() {
    assert_eq!(
        ungrounded_figures("Total **$26,117**.", "[15977,10140,26117]"),
        none()
    );
    assert_eq!(
        ungrounded_figures("Total **$1,597,710,140**.", "[15977,10140,26117]"),
        vec!["1,597,710,140"]
    );
}

#[test]
fn grounds_a_figure_printed_in_a_dataframes_columns() {
    let frame = "     month  direct  airbnb   total\n0  2026-11   15977   10140   26117\n1  2026-12    9133       0    9133";
    assert_eq!(
        ungrounded_figures("November came to $26,117, December $9,133.", frame),
        none()
    );
    assert_eq!(
        ungrounded_figures("November came to $26,171.", frame),
        vec!["26,171"]
    );
}

#[test]
fn grounds_a_figure_printed_with_separators_or_a_fractional_part() {
    assert_eq!(
        ungrounded_figures("Total 101000.", "Total: 101,000.00"),
        none()
    );
}

#[test]
fn reads_a_figure_grouped_with_spaces_as_one_figure_as_french_replies_write_it() {
    assert_eq!(
        ungrounded_figures("Août : 1 128 616 € installés.", ""),
        vec!["1 128 616"]
    );
    assert_eq!(
        ungrounded_figures("Août : 1 128 616 € installés.", "ca_installe: 1128616"),
        none()
    );
    assert_eq!(
        ungrounded_figures("Total\u{a0}: 12\u{202f}500 €", ""),
        vec!["12\u{202f}500"]
    );
}

#[test]
fn grounds_a_space_grouped_figure_by_a_table_that_printed_it_either_way() {
    assert_eq!(
        ungrounded_figures("Le total est de 1 128 616 €.", "| total | 1 128 616 |"),
        none()
    );
    assert_eq!(
        ungrounded_figures("Columns 120 and 340 are grounded.", "values: 5 120 340"),
        none()
    );
}

// A script's print() sets values side by side with one space.
#[test]
fn figures_a_space_sets_side_by_side_are_read_apart() {
    assert_eq!(
        ungrounded_figures("Q2 was 980,000.", "Revenue 1,250,000 980,000"),
        none()
    );
    assert_eq!(
        ungrounded_figures("In 2026 125,000 users signed up.", "users: 120000"),
        ["125,000"]
    );
}

#[test]
fn never_joins_groups_that_are_not_exact_thousands() {
    assert_eq!(
        ungrounded_figures("Rows 12 5 and 1 2345 are separate.", ""),
        none()
    );
}

// A run welds once from its first group: "1 128 616" grounds 1128616 and its
// parts, never 128616, a number the table did not print.
#[test]
fn does_not_weld_the_interior_of_a_space_grouped_run_in_the_corpus() {
    assert_eq!(
        ungrounded_figures("Le sous-total est de 128 616 €.", "| total | 1 128 616 |"),
        vec!["128 616"]
    );
}

// Exact three-digit comma groups read as thousands in the reply; the corpus
// also keeps the parts, so a printed list grounds the values it holds.
#[test]
fn reads_a_comma_list_of_three_digit_values_as_thousands_and_grounds_its_parts_from_the_corpus() {
    assert_eq!(
        ungrounded_figures("Values 100,200,300 were read.", "[100,200,300]"),
        none()
    );
    assert_eq!(ungrounded_figures("The $200 row.", "[100,200,300]"), none());
}

#[test]
fn reads_a_figure_that_ends_a_sentence() {
    assert_eq!(
        ungrounded_figures("December Direct drops to $8,093.", "Direct total: 9133"),
        vec!["8,093"]
    );
    assert_eq!(ungrounded_figures("Rows: 2,000.", "Rows: 2000"), none());
    assert_eq!(
        ungrounded_figures("ARR is $1,250,000.50.", "ARR 1250000.5"),
        none()
    );
    assert_eq!(
        ungrounded_figures("The fee was $729.", "Flor Ferro cancelled: $1,458"),
        vec!["729"]
    );
}

#[test]
fn grounds_the_printed_figure_and_flags_an_invented_one_in_each_locale() {
    let total = "total 1250000";
    let cases: [(&str, &str, &str); 10] = [
        ("German", "Umsatz: 1.250.000 €.", "Umsatz: 1.010.000 €."),
        (
            "Spanish",
            "Ingresos: 1.250.000 €.",
            "Ingresos: 1.010.000 €.",
        ),
        (
            "Swiss German",
            "Total CHF 1'250'000.",
            "Total CHF 1'010'000.",
        ),
        ("French", "Total : 1 250 000 €", "Total : 1 010 000 €"),
        ("Russian", "Итого: 1 250 000 ₽.", "Итого: 1 010 000 ₽."),
        (
            "Arabic, Arabic-Indic digits",
            "الإجمالي ١٬٢٥٠٬٠٠٠ ريال.",
            "الإجمالي ١٬٠١٠٬٠٠٠ ريال.",
        ),
        ("Persian", "مجموع ۱٬۲۵۰٬۰۰۰ تومان", "مجموع ۱٬۰۱۰٬۰۰۰ تومان"),
        (
            "Hindi, lakh grouping",
            "कुल ₹12,50,000 है।",
            "कुल ₹10,10,000 है।",
        ),
        (
            "Japanese, no spaces",
            "合計は1,250,000円です。",
            "合計は1,010,000円です。",
        ),
        (
            "Chinese, fullwidth",
            "合计１，２５０，０００元",
            "合计１，０１０，０００元",
        ),
    ];
    for (locale, stated, invented) in cases {
        assert_eq!(
            ungrounded_figures(stated, total),
            none(),
            "{locale}: stated"
        );
        assert_eq!(
            ungrounded_figures(invented, total).len(),
            1,
            "{locale}: invented"
        );
    }
}

#[test]
fn grounds_across_locales_a_dot_grouped_result_behind_a_comma_grouped_reply() {
    assert_eq!(
        ungrounded_figures("Revenue is $1,250,000.", "Umsatz: 1.250.000 €"),
        none()
    );
}

#[test]
fn reads_a_locales_decimal_a_comma_after_a_dot_grouped_whole_or_alone() {
    assert_eq!(
        ungrounded_figures("Gebühr: 8.220,5042 €.", "fee 8220.5042"),
        none()
    );
    assert_eq!(
        ungrounded_figures("Gebühr: 8.220,5042 €.", "fee 8220.5"),
        vec!["8.220,5042"]
    );
    assert_eq!(
        ungrounded_figures("Kurs: 1,0842 USD.", "rate 1.0842"),
        none()
    );
}

// "1.250" is 1,250 in Berlin and 1.25 in Boston: both sides read it both
// ways, so a German amount is checked as the integer it states and a
// printed value grounds it under either convention.
#[test]
fn reads_a_single_dot_or_comma_before_three_digits_both_ways() {
    assert_eq!(
        ungrounded_figures("Gebühr: 8.093 €.", "total 9133"),
        vec!["8.093"]
    );
    assert_eq!(
        ungrounded_figures("Gebühr: 8.093 €.", "Gebühr 8.093"),
        none()
    );
    assert_eq!(
        ungrounded_figures("The fee is $1,250.", "Gebühr 1.250 €"),
        none()
    );
    assert_eq!(
        ungrounded_figures("The fee is $1,250.", "Gebühr 1,25 €"),
        vec!["1,250"]
    );
    assert_eq!(
        ungrounded_figures("The fee is $1.250.", "fee 1.25"),
        vec!["1.250"]
    );
}

#[test]
fn keeps_a_figure_apart_from_the_id_version_or_unit_it_sits_in() {
    assert_eq!(
        ungrounded_figures("See p1789368220119 and v3.1, about 12.5kg.", ""),
        none()
    );
    assert_eq!(
        ungrounded_figures("FY2026 revenue: $1,250,000.", "total 1250000"),
        none()
    );
}

#[test]
fn grounds_a_figure_that_fits_two_readings_by_either() {
    let reply = "Paid $123.456 in total.";
    for corpus in ["amount 123456", "amount 123.456", "amount 123"] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    assert_eq!(ungrounded_figures(reply, "amount 124"), ["123.456"]);
}

#[test]
fn names_a_figure_stated_twice_once() {
    assert_eq!(
        ungrounded_figures("Revenue 12,500, again 12500, then 12,500.", "nothing"),
        ["12,500"]
    );
}
