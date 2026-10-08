use figures_parser::extract_numbers;
use figures_parser::number::scan::tokens;

fn keys(text: &str) -> Vec<(String, Option<String>, bool, bool)> {
    extract_numbers(text)
        .into_iter()
        .map(|n| {
            (
                n.integer.into_owned(),
                n.fraction.map(Into::into),
                n.grouped,
                n.derived,
            )
        })
        .collect()
}

fn k(
    i: &str,
    f: Option<&str>,
    grouped: bool,
    derived: bool,
) -> (String, Option<String>, bool, bool) {
    (i.to_owned(), f.map(str::to_owned), grouped, derived)
}

fn thousands(whole: &str, parts: [&str; 3]) -> [(String, Option<String>, bool, bool); 4] {
    let [a, b, c] = parts;
    [
        k(whole, None, true, false),
        k(a, None, false, true),
        k(b, None, false, true),
        k(c, None, false, true),
    ]
}

#[test]
fn a_bare_run_is_one_integer() {
    assert_eq!(keys("101000"), [k("101000", None, false, false)]);
    assert_eq!(keys("١٢٥٠"), [k("1250", None, false, false)]);
}

#[test]
fn a_run_in_any_script_or_width_of_symbol_reads_the_same() {
    let expected = thousands("1250000", ["1", "250", "000"]);
    assert_eq!(keys("1\u{202f}250\u{202f}000"), expected);
    assert_eq!(keys("١٬٢٥٠٬٠٠٠"), expected);
    assert_eq!(keys("𞥑,𞥒𞥕𞥐,𞥐𞥐𞥐"), expected);
    assert_eq!(keys("٣٫١٤"), [k("3", Some("14"), false, false)]);
}

#[test]
fn a_repeated_symbol_with_three_digit_groups_is_thousands_plus_derived_parts() {
    assert_eq!(keys("1,250,000"), thousands("1250000", ["1", "250", "000"]));
    assert_eq!(keys("1.250.000"), thousands("1250000", ["1", "250", "000"]));
    assert_eq!(keys("1 128 616"), thousands("1128616", ["1", "128", "616"]));
    assert_eq!(keys("12,50,000"), thousands("1250000", ["12", "50", "000"]));
}

#[test]
fn a_single_dot_or_comma_before_three_digits_reads_both_ways() {
    assert_eq!(
        keys("1.250"),
        [
            k("1250", None, true, false),
            k("1", Some("250"), false, false),
            k("1", None, false, true),
            k("250", None, false, true),
        ]
    );
    assert_eq!(
        keys("8,093"),
        [
            k("8093", None, true, false),
            k("8", Some("093"), false, false),
            k("8", None, false, true),
            k("093", None, false, true),
        ]
    );
}

#[test]
fn a_differing_last_symbol_or_a_short_last_group_ends_a_decimal() {
    assert_eq!(
        keys("1.234.567,89"),
        [k("1234567", Some("89"), true, false)]
    );
    assert_eq!(
        keys("1,234,567.89"),
        [k("1234567", Some("89"), true, false)]
    );
    assert_eq!(keys("8,22"), [k("8", Some("22"), false, false)]);
    assert_eq!(keys("1.0842"), [k("1", Some("0842"), false, false)]);
    assert_eq!(keys("8.220,5042"), [k("8220", Some("5042"), true, false)]);
    assert_eq!(keys("1.08425"), [k("1", Some("08425"), false, false)]);
    assert_eq!(
        keys("15977,10140"),
        [k("15977", Some("10140"), false, false)]
    );
}

#[test]
fn groups_that_fit_neither_are_neighbours() {
    assert_eq!(
        keys("15977,10140,26117"),
        [
            k("15977", None, false, false),
            k("10140", None, false, false),
            k("26117", None, false, false),
        ]
    );
    assert_eq!(
        keys("12 5"),
        [k("12", None, false, false), k("5", None, false, false)]
    );
    assert_eq!(
        keys("1 2345"),
        [k("1", None, false, false), k("2345", None, false, false)]
    );
}

#[test]
fn currency_rides_on_the_whole_and_on_the_first_neighbour_only() {
    let currency = |text: &str| -> Vec<bool> {
        tokens(text)
            .remove(0)
            .readings()
            .iter()
            .map(|r| r.currency)
            .collect()
    };
    assert_eq!(currency("$15977,10140,26117"), [true, false, false]);
    assert_eq!(currency("$1,250"), [true, true, true, false]);
}

#[test]
fn every_reading_slices_the_input_at_its_span() {
    let text = "Total $1,250 and 7.";
    let e = extract_numbers(text);
    let seen: Vec<(&str, &str, usize, usize)> = e
        .iter()
        .map(|n| (n.text, &*n.integer, n.span.start, n.span.end))
        .collect();
    assert_eq!(
        seen,
        [
            ("1,250", "1250", 7, 12),
            ("1,250", "1", 7, 12),
            ("1", "1", 7, 8),
            ("250", "250", 9, 12),
            ("7", "7", 17, 18),
        ]
    );
    for n in &e {
        assert_eq!(text.get(n.span.clone()), Some(n.text));
    }
    assert!(e[0].currency && e[0].grouped && !e[0].derived);
    assert!(e[2].derived);
}
