use figures_parser::{Sign, extract_numbers, ungrounded_figures};

/// The sign and signed text of the first reading of `text`.
fn first(text: &str) -> (Sign, String) {
    extract_numbers(text)
        .first()
        .map_or((Sign::Unsigned, String::new()), |n| {
            (n.sign, n.signed_text.to_owned())
        })
}

#[test]
fn reads_a_minus_however_a_locale_prints_it() {
    for (text, signed) in [
        ("-1,234.5", "-1,234.5"),
        ("−1 234,5", "−1 234,5"),
        ("-1.234,50 €", "-1.234,50"),
        ("-€1,234.50", "-€1,234.50"),
        ("$-1'234.50", "-1'234.50"),
        ("€ -1.234,50", "-1.234,50"),
        ("EUR-1'234.50", "-1'234.50"),
        ("-US$ 1.234,50", "-US$ 1.234,50"),
        ("\u{61c}-١٬٢٣٤٫٥", "-١٬٢٣٤٫٥"),
        ("\u{61c}-\u{200f}١٬٢٣٤٫٥٠ €", "-\u{200f}١٬٢٣٤٫٥٠"),
        ("\u{200e}−\u{200e}€۱٬۲۳۴٫۵۰", "−\u{200e}€۱٬۲۳۴٫۵۰"),
        ("loss: -12,500", "-12,500"),
        ("| −12,500 |", "−12,500"),
        ("(−12.5%)", "−12.5"),
    ] {
        assert_eq!(first(text), (Sign::Minus, signed.to_owned()), "{text}");
    }
    assert_eq!(first("+12.5%"), (Sign::Plus, "+12.5".to_owned()));
    assert_eq!(first("＋12.5%"), (Sign::Plus, "＋12.5".to_owned()));
}

#[test]
fn reads_accounting_parentheses() {
    for text in [
        "($1,234.50)",
        "(1 234,50 $US)",
        "(US$ 1.234,50)",
        "(USD 1 234,50)",
        "(১,২৩৪.৫০ US$)",
        "(1234,50 $)",
        "(12,500)",
    ] {
        assert_eq!(first(text).0, Sign::Parenthesized, "{text}");
    }
}

#[test]
fn reads_no_sign_in_a_dash_that_joins() {
    for text in [
        "2026-11-05",
        "CORE-12500",
        "10-12,500",
        "--12,500",
        "x-12,500",
        "- 12,500",
        "C++11",
        "(12,500 users)",
        "1+2",
    ] {
        let signed: Vec<Sign> = extract_numbers(text).iter().map(|n| n.sign).collect();
        assert!(
            signed.iter().all(|&s| s == Sign::Unsigned),
            "{text}: {signed:?}"
        );
    }
}

#[test]
fn a_run_signs_its_whole_readings_and_its_first_group_only() {
    let readings: Vec<(String, Sign, String)> = extract_numbers("-12,500")
        .iter()
        .map(|n| (n.integer.to_string(), n.sign, n.signed_text.to_owned()))
        .collect();
    assert_eq!(
        readings,
        [
            ("12500".into(), Sign::Minus, "-12,500".into()),
            ("12".into(), Sign::Minus, "-12,500".into()),
            ("12".into(), Sign::Minus, "-12".into()),
            ("500".into(), Sign::Unsigned, "500".into()),
        ]
    );
}

#[test]
fn holds_a_signed_figure_to_its_sign() {
    let reply = "Revenue moved by -12,500 this week.";
    assert_eq!(ungrounded_figures(reply, "delta: 12500"), ["-12,500"]);
    assert_eq!(
        ungrounded_figures(reply, "delta: -12500"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(reply, "Net income (12,500)"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(reply, "before: 12500, after: -12500"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("A gain of +12,500.", "delta: -12500"),
        ["+12,500"]
    );
    assert_eq!(
        ungrounded_figures("A gain of +12,500.", "delta: 12500"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("It moved by −1.17853.", "rate 1.17853"),
        ["−1.17853"]
    );
}

#[test]
fn takes_an_unsigned_or_parenthesized_figure_from_either_sign() {
    assert_eq!(
        ungrounded_figures("It fell by 12,500.", "delta: -12500"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("Net (12,500).", "delta: 12500"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("From 12,500 to -12,500.", "delta: 12500"),
        ["-12,500"]
    );
}
