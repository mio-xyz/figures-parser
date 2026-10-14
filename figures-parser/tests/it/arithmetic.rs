use figures_parser::prose_arithmetic;

#[test]
fn names_a_result_worked_out_from_four_operands() {
    let message = "What is 10 + 20 + 5 + 7?";
    assert_eq!(prose_arithmetic("42.", message, message), ["42"]);
}

#[test]
fn counts_a_repeated_operand_each_time_it_appears() {
    let message = "What is 25 + 25 + 25 + 25?";
    assert_eq!(prose_arithmetic("100.", message, message), ["100"]);
}

#[test]
fn names_a_negative_result() {
    let message = "Balance: 12 - 30 - 15 - 9?";
    assert_eq!(prose_arithmetic("-42.", message, message), ["-42"]);
    assert_eq!(
        prose_arithmetic("Then -0.", message, message),
        Vec::<String>::new()
    );
}

#[test]
fn lets_through_a_value_the_turn_carries() {
    let message = "Budgets: 120, 340, 560, 780. Which is largest?";
    assert_eq!(
        prose_arithmetic("The largest is 780.", message, message),
        Vec::<String>::new()
    );
    let seats = "Seats 4, 6, 9 and 12: how many in total if we add the 31 we have?";
    let corpus = format!("{seats}\nCompany profile: 31 seats");
    assert_eq!(
        prose_arithmetic("You have 31.", seats, &corpus),
        Vec::<String>::new()
    );
}

#[test]
fn ignores_counts_years_times_and_dates() {
    let message = "Runs at 9, 10, 11 and 14 on 2026-10-01: which slots are free?";
    assert_eq!(
        prose_arithmetic(
            "3 slots are free: 12:30, 15:00 and 16:00 in 2026, from 10/2026.",
            message,
            message
        ),
        Vec::<String>::new()
    );
    // A date or a time gives no operands either.
    assert_eq!(
        prose_arithmetic("42.", "Meet 2026-10-01 at 14:30?", ""),
        Vec::<String>::new()
    );
}

#[test]
fn a_value_is_matched_exactly() {
    let message = "Rates 1.25, 2.5, 3.75 and 4.0: the mean?";
    // Trailing zeros aside, "2.875" must be printed as such.
    assert_eq!(
        prose_arithmetic("Mean 2.8750.", message, "mean: 2.875"),
        Vec::<String>::new()
    );
    assert_eq!(
        prose_arithmetic("Mean 2.88.", message, "mean: 2.875"),
        ["2.88"]
    );
    assert_eq!(
        prose_arithmetic("Mean 2.", message, "mean: 2.875"),
        Vec::<String>::new()
    );
    assert_eq!(
        prose_arithmetic("Sum 11.5.", message, "sum: 11.50"),
        Vec::<String>::new()
    );
    assert_eq!(prose_arithmetic("Sum 11.", message, "sum: 11.5"), ["11"]);
}

#[test]
fn reads_operands_and_results_in_every_notation() {
    let message = "Umsatz 1.250,50 €, 2.300,25 €, 980,00 € und 1.469,25 €: Summe?";
    assert_eq!(
        prose_arithmetic("Summe: 6.000,00 €.", message, message),
        ["6.000,00"]
    );
    assert_eq!(
        prose_arithmetic("Summe: 6.000,00 €.", message, "total 6000"),
        Vec::<String>::new()
    );
    let arabic = "المبالغ ١٢٠ و٣٤٠ و٥٦٠ و٧٨٠";
    assert_eq!(prose_arithmetic("المجموع ١٨٠٠", arabic, arabic), ["١٨٠٠"]);
}

#[test]
fn a_scaled_or_approximate_result_is_grounded_by_what_rounds_to_it() {
    let message = "Deals 400,000, 350,000, 250,000 and 200,000: total?";
    assert_eq!(
        prose_arithmetic("About 1.2 million.", message, "total 1,200,000"),
        Vec::<String>::new()
    );
    assert_eq!(
        prose_arithmetic("About 1.2 million.", message, message),
        ["1.2 million"]
    );
}

#[test]
fn a_figure_cut_off_by_its_unit_is_no_result() {
    let message = "Loads of 3.5, 4.25, 2.75 and 1.5 kg: the total?";
    assert_eq!(
        prose_arithmetic("12.5kg in all.", message, message),
        Vec::<String>::new()
    );
}

#[test]
fn values_a_space_sets_side_by_side_are_operands_and_results() {
    let message = "212 12,415 31556.71 50.136";
    assert_eq!(
        prose_arithmetic("Total 0.9932.", message, message),
        ["0.9932"]
    );
    assert_eq!(
        prose_arithmetic("Then -54,916.481 1962.", "1 + 2 + 3 + 4", ""),
        ["-54,916.481"]
    );
}

#[test]
fn only_a_value_printed_as_one_grounds_a_result() {
    let message = "Costs $1,524, $2,100, $980 and $1,396 at 14:30: the share of the first?";
    // A group of a thousands run, or the piece of a time, is no printed value.
    assert_eq!(prose_arithmetic("It is 524.", message, message), ["524"]);
    assert_eq!(
        prose_arithmetic("About 30 of it.", message, message),
        ["30"]
    );
    assert_eq!(
        prose_arithmetic("It is 1,524.", message, message),
        Vec::<String>::new()
    );
}
