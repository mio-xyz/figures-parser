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
fn reads_no_mark_in_a_range_or_a_wave_dash() {
    for text in [
        "10~20,000",
        "10 ~ 20,000",
        "x~12,500",
        "〜12,500",
        "～12,500",
        "~  12,500",
        "~\n12,500",
        "12,500",
    ] {
        let marked: Vec<bool> = extract_numbers(text)
            .iter()
            .map(|n| n.approximate)
            .collect();
        assert!(marked.iter().all(|&m| !m), "{text}: {marked:?}");
    }
}

#[test]
fn grounds_an_approximate_figure_by_any_value_that_rounds_to_it() {
    let reply = "Signups came to ~12,500 this quarter.";
    for corpus in ["12500", "12487", "12450", "12549.9", "12,487", "12 487"] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    for corpus in ["12449", "12550", "12431", "1.2487", "125000"] {
        assert_eq!(ungrounded_figures(reply, corpus), ["~12,500"], "{corpus}");
    }
}

#[test]
fn rounds_at_the_last_digit_written() {
    assert_eq!(ungrounded_figures("~12,000", "12487"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,000", "11500"), Vec::<String>::new());
    assert_eq!(ungrounded_figures("~12,000", "12500"), ["~12,000"]);
    assert_eq!(
        ungrounded_figures("~12,487", "12486.5"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("~12,487", "12486.4"), ["~12,487"]);
    assert_eq!(
        ungrounded_figures("~1,250,000", "1248331"),
        Vec::<String>::new()
    );
    // The exact rule takes only a printed prefix; rounding takes 1.17846 too.
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
fn takes_a_power_of_ten_from_where_a_value_rounds_up_to_it() {
    for corpus in ["9500", "9960", "14999"] {
        assert_eq!(
            ungrounded_figures("~10,000", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    for corpus in ["9499", "7213", "15000"] {
        assert_eq!(
            ungrounded_figures("~10,000", corpus),
            ["~10,000"],
            "{corpus}"
        );
    }
}

#[test]
fn holds_an_approximate_figure_to_its_sign() {
    assert_eq!(
        ungrounded_figures("~-12,500", "delta: -12487"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("~-12,500", "delta: 12487"), ["~-12,500"]);
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
