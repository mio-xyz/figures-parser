use figures_parser::{Sign, extract_numbers, ungrounded_figures};

/// Whether the first reading of `text` is marked approximate, its sign, and
/// its full text.
fn first(text: &str) -> (bool, Sign, String) {
    extract_numbers(text)
        .first()
        .map_or((false, Sign::Unsigned, String::new()), |n| {
            (n.approximate, n.sign, n.full_text.to_owned())
        })
}

#[test]
fn reads_an_approximation_mark_before_the_figure_its_sign_and_its_currency() {
    for (text, sign, full) in [
        ("~12,500", Sign::Unsigned, "~12,500"),
        ("about ~ 12,500 users", Sign::Unsigned, "~ 12,500"),
        ("≈12,500", Sign::Unsigned, "≈12,500"),
        ("∼12,500", Sign::Unsigned, "∼12,500"),
        ("~$12,500", Sign::Unsigned, "~$12,500"),
        ("~ $12,500", Sign::Unsigned, "~ $12,500"),
        ("≈ €1.234,50", Sign::Unsigned, "≈ €1.234,50"),
        ("~-12,500", Sign::Minus, "~-12,500"),
        ("~-$12,500", Sign::Minus, "~-$12,500"),
        ("(~12,500)", Sign::Unsigned, "~12,500"),
        ("total: ~12,500", Sign::Unsigned, "~12,500"),
    ] {
        assert_eq!(first(text), (true, sign, full.to_owned()), "{text}");
    }
}

#[test]
fn reads_a_tilde_before_a_figure_as_a_mark_wherever_it_stands() {
    for text in [
        "10~20,000",
        "10 ~ 20,000",
        "Total 5 ~20,000",
        "Total 19,851\n~20,000",
        "Mon 2026-10-19 ~20,000",
        "x~20,000",
    ] {
        let marked = extract_numbers(text)
            .iter()
            .find(|n| n.text == "20,000")
            .map(|n| n.approximate);
        assert_eq!(marked, Some(true), "{text}");
    }
}

#[test]
fn grounds_a_marked_figure_after_another_figure_as_any_marked_figure() {
    for reply in [
        "Total 19,851\n~3,440 sessions",
        "Total 5 ~3,440 sessions",
        "Mon 2026-10-19 ~3,440 sessions",
    ] {
        assert_eq!(
            ungrounded_figures(reply, "total 19851, sessions 3442"),
            Vec::<String>::new(),
            "{reply}"
        );
    }
}

#[test]
fn reads_no_mark_in_a_wave_dash_or_apart_from_the_figure() {
    for text in ["〜12,500", "～12,500", "~  12,500", "~\n12,500", "12,500"] {
        let marked: Vec<bool> = extract_numbers(text)
            .iter()
            .map(|n| n.approximate)
            .collect();
        assert!(marked.iter().all(|&m| !m), "{text}: {marked:?}");
    }
}

#[test]
fn grounds_a_marked_figure_by_any_value_within_a_unit_of_its_last_digit() {
    let reply = "Signups came to ~12,500 this quarter.";
    for corpus in [
        "12500", "12487", "12450", "12431", "12400", "12600", "12,487", "12 487",
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    for corpus in ["12399", "12600.5", "12601", "1.2487", "125000"] {
        assert_eq!(ungrounded_figures(reply, corpus), ["~12,500"], "{corpus}");
    }
}

#[test]
fn takes_a_unit_of_the_last_digit_written() {
    assert_eq!(ungrounded_figures("~12,000", "12487"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,000", "11000"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,000", "13000"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,000", "10999"), ["~12,000"]);
    assert_eq!(ungrounded_figures("~12,000", "13001"), ["~12,000"]);
    assert_eq!(ungrounded_figures("~12,487", "12486"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,487", "12485.9"), ["~12,487"]);
    assert_eq!(ungrounded_figures("~3,440", "3449"), Vec::<String>::new());
    assert_eq!(
        ungrounded_figures("~1,250,000", "1248331"),
        Vec::<String>::new()
    );
    // The exact rule takes only a printed prefix; the mark takes 1.17846 too.
    assert_eq!(
        ungrounded_figures("~1.1785", "1.17846"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("1.1785", "1.17846"), ["1.1785"]);
    assert_eq!(ungrounded_figures("~1.1785", "1.17836"), ["~1.1785"]);
}

#[test]
fn grounds_whatever_grounds_the_exact_figure() {
    // A short decimal is compared by its integer, a precise one by a printed
    // prefix; the mark takes those as well as what rounds to the figure.
    for (reply, corpus) in [
        ("~12,500.7", "12500.2"),
        ("~12,500.7", "12500.66"),
        ("~1.1785", "1.17859"),
        ("~12,500", "12500.9"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{reply} | {corpus}"
        );
    }
    assert_eq!(ungrounded_figures("~12,500.7", "12499.9"), ["~12,500.7"]);
}

#[test]
fn takes_a_scaled_figure_to_the_place_its_digits_are_written_to() {
    for corpus in ["9500000", "9960000", "10499999"] {
        assert_eq!(
            ungrounded_figures("10 million users", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    for corpus in ["9499999", "7213000", "10500000", "14999999"] {
        assert_eq!(
            ungrounded_figures("10 million users", corpus),
            ["10 million"],
            "{corpus}"
        );
    }
}

#[test]
fn takes_a_marked_power_of_ten_a_unit_either_side() {
    for corpus in ["9000", "18000", "20000"] {
        assert_eq!(
            ungrounded_figures("~10,000", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    assert_eq!(ungrounded_figures("~10,000", "20001"), ["~10,000"]);
}

#[test]
fn grounds_a_marked_scaled_figure_within_a_unit_of_its_last_digit() {
    // A scale word alone states a value rounded: "3M" takes 2.5M to 3.5M.
    assert_eq!(
        ungrounded_figures("About 3M teachers.", "teachers: 2,412,000"),
        ["3M"]
    );
    for corpus in ["teachers: 2,412,000", "teachers: 3,600,000", "4000000"] {
        assert_eq!(
            ungrounded_figures("About ~3M teachers.", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    assert_eq!(
        ungrounded_figures("About ~3M teachers.", "teachers: 4,100,000"),
        ["~3M"]
    );
    assert_eq!(
        ungrounded_figures("About ~3.2M teachers.", "teachers: 3,290,000"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(
            "≈\u{a0}3,500 sessions, ~\u{202f}$12,500 in",
            "sessions 3442\nrevenue 12480"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn holds_an_approximate_figure_to_its_sign() {
    assert_eq!(
        ungrounded_figures("~-12,500", "delta: -12487"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("~-12,500", "delta: +12487"),
        ["~-12,500"]
    );
    assert_eq!(
        ungrounded_figures("~-12,500", "delta: 12487"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("~12,500", "delta: -12487"),
        Vec::<String>::new()
    );
}

#[test]
fn keeps_an_exact_figure_exact_beside_an_approximate_one() {
    assert_eq!(
        ungrounded_figures("~12,500 now, 12,500 exactly", "total 12487"),
        ["12,500"]
    );
}

#[test]
fn leaves_a_figure_under_its_floor_unchecked() {
    assert_eq!(
        ungrounded_figures("~500 users", "total 12487"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("~$500", "total 12487"), ["~$500"]);
}

#[test]
fn grounds_an_approximate_figure_written_with_a_leading_zero_by_itself() {
    assert_eq!(
        ungrounded_figures("≈0.51.250", "≈0.51.250"),
        Vec::<String>::new()
    );
}
