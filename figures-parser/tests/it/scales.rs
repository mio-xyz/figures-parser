use std::borrow::Cow;

use figures_parser::{Sign, extract_numbers, ungrounded_figures};

/// The first stated-side reading of `text`: as written, its integer and
/// fraction, and whether it reads as approximate.
fn first(text: &str) -> Option<(String, String, Option<String>, bool)> {
    extract_numbers(text)
        .into_iter()
        .find(|n| !n.derived)
        .map(|n| {
            (
                n.text.to_owned(),
                n.integer.into_owned(),
                n.fraction.map(Cow::into_owned),
                n.approximate,
            )
        })
}

#[test]
fn reads_a_scale_word_or_suffix_after_the_digits() {
    for (text, written, integer) in [
        ("Revenue hit 1.2 million.", "1.2 million", "1200000"),
        ("$4.5bn raised", "4.5bn", "4500000000"),
        ("about 12.5K signups", "12.5K", "12500"),
        ("12k users", "12k", "12000"),
        ("2,5 Mio. Euro", "2,5 Mio", "2500000"),
        ("2,5 Mio €", "2,5 Mio", "2500000"),
        ("1,2 Bio.", "1,2 Bio", "1200000000000"),
        ("1,5 Mrd. Umsatz", "1,5 Mrd", "1500000000"),
        ("3 milliards d'euros", "3 milliards", "3000000000"),
        ("2 Milliarden", "2 Milliarden", "2000000000"),
        ("12 mil personas", "12 mil", "12000"),
        ("1,2 mil millones", "1,2 mil millones", "1200000000"),
        ("3 mil milhões", "3 mil milhões", "3000000000"),
        ("4 milioni", "4 milioni", "4000000"),
        ("5 lakh", "5 lakh", "500000"),
        ("2 crore", "2 crore", "20000000"),
        ("₹5 Cr", "5 Cr", "50000000"),
        ("売上1.2万", "1.2万", "12000"),
        ("3億円", "3億", "300000000"),
        ("5千万元", "5千万", "50000000"),
        ("2万亿", "2万亿", "2000000000000"),
        ("0.5 million", "0.5 million", "500000"),
        ("€5m", "5m", "5000000"),
        ("$3B", "3B", "3000000000"),
        ("5 M€", "5 M", "5000000"),
        ("12 k€", "12 k", "12000"),
        ("R$ 5 mi", "5 mi", "5000000"),
        ("$5MM", "5MM", "5000000"),
        ("1,250K", "1,250K", "1250000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
}

#[test]
fn keeps_what_the_scale_leaves_a_fraction() {
    assert_eq!(
        first("1.2345678 million"),
        Some((
            "1.2345678 million".to_owned(),
            "1234567".to_owned(),
            Some("8".to_owned()),
            true
        ))
    );
}

#[test]
fn reads_a_letter_that_names_a_unit_only_beside_a_currency() {
    for text in [
        "took 5m",
        "a 7B model",
        "5mm thick",
        "12 K",
        "5 M users",
        "5Mbps",
        "12.5kg",
        "5 mi away",
        "5 cr",
        "3T drive",
        "12.5K9",
        "a thread of 2 Mio messages",
        "3 Bio products",
    ] {
        let scaled: Vec<_> = extract_numbers(text)
            .into_iter()
            .filter(|n| n.approximate)
            .map(|n| n.text)
            .collect();
        assert!(scaled.is_empty(), "{text}: {scaled:?}");
    }
}

#[test]
fn carries_the_sign_and_the_mark_through_the_word() {
    let n = extract_numbers("fell -$1.2M")
        .into_iter()
        .find(|n| !n.derived)
        .unwrap_or_else(|| panic!("no reading"));
    assert_eq!((n.sign, n.full_text), (Sign::Minus, "-$1.2M"));
    assert_eq!(
        ungrounded_figures("It fell -$1.2M.", "delta: 1234567"),
        ["-$1.2M"]
    );
    assert_eq!(
        ungrounded_figures("It fell -$1.2M.", "delta: -1234567"),
        Vec::<String>::new()
    );
}

#[test]
fn grounds_a_scaled_figure_by_a_value_that_rounds_to_it() {
    let reply = "Revenue hit $1.2 million, from 12.5K signups.";
    assert_eq!(
        ungrounded_figures(reply, "revenue 1234567, signups 12487"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(reply, "revenue 1260000, signups 12600"),
        ["1.2 million", "12.5K"]
    );
}

#[test]
fn grounds_an_exact_figure_by_a_scaled_corpus_value_of_the_same_value() {
    assert_eq!(
        ungrounded_figures("ARR is 1,200,000.", "ARR: $1.2M"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("ARR is 1,234,567.", "ARR: $1.2M"),
        ["1,234,567"]
    );
}

#[test]
fn grounds_a_scaled_figure_that_fits_two_readings_by_either() {
    let reply = "Debt of €12,500 million.";
    for corpus in ["12500000000", "12500000"] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    assert_eq!(ungrounded_figures(reply, "12600000"), ["12,500 million"]);
}

#[test]
fn holds_a_scaled_figure_to_the_floors() {
    assert_eq!(
        ungrounded_figures("5k users", "nothing"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("$5k budget", "nothing"), ["5k"]);
}
