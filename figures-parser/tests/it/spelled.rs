use figures_parser::{Sign, extract_numbers, numbers, ungrounded_figures};

/// The numbers `text` writes in words: as written, and their value.
fn spelled(text: &str) -> Vec<(String, String)> {
    extract_numbers(text)
        .into_iter()
        .filter(|n| {
            n.text.chars().any(char::is_alphabetic)
                && !n.text.starts_with(|c: char| c.is_ascii_digit())
        })
        .map(|n| (n.text.to_owned(), n.integer.into_owned()))
        .collect()
}

#[test]
fn reads_numbers_written_in_words_in_six_languages() {
    for (text, written, value) in [
        (
            "We hit twelve thousand five hundred users.",
            "twelve thousand five hundred",
            "12500",
        ),
        (
            "one hundred and twenty thousand",
            "one hundred and twenty thousand",
            "120000",
        ),
        (
            "two million three hundred thousand",
            "two million three hundred thousand",
            "2300000",
        ),
        ("fifteen hundred dollars", "fifteen hundred", "1500"),
        ("twenty-five thousand", "twenty-five thousand", "25000"),
        ("two thousand and twenty", "two thousand and twenty", "2020"),
        ("five lakh", "five lakh", "500000"),
        ("vingt-cinq mille", "vingt-cinq mille", "25000"),
        (
            "un million deux cent mille",
            "un million deux cent mille",
            "1200000",
        ),
        (
            "quatre-vingt-dix-sept mille",
            "quatre-vingt-dix-sept mille",
            "97000",
        ),
        ("soixante et onze mille", "soixante et onze mille", "71000"),
        ("cent mille euros", "cent mille", "100000"),
        (
            "un millón doscientos mil",
            "un millón doscientos mil",
            "1200000",
        ),
        ("doce mil millones", "doce mil millones", "12000000000"),
        ("treinta y cinco mil", "treinta y cinco mil", "35000"),
        ("dodicimila", "dodicimila", "12000"),
        (
            "un milione duecentomila",
            "un milione duecentomila",
            "1200000",
        ),
        ("centoventimila", "centoventimila", "120000"),
        (
            "zweihundertfünfzigtausend",
            "zweihundertfünfzigtausend",
            "250000",
        ),
        ("fünfundzwanzigtausend", "fünfundzwanzigtausend", "25000"),
        (
            "zwei Millionen dreihunderttausend",
            "zwei Millionen dreihunderttausend",
            "2300000",
        ),
        ("vinte e cinco mil", "vinte e cinco mil", "25000"),
        (
            "um milhão e duzentos mil",
            "um milhão e duzentos mil",
            "1200000",
        ),
        ("一万二千", "一万二千", "12000"),
        ("十二万", "十二万", "120000"),
        ("三億五千万", "三億五千万", "350000000"),
        ("一万零五", "一万零五", "10005"),
    ] {
        assert_eq!(
            spelled(text),
            [(written.to_owned(), value.to_owned())],
            "{text}"
        );
    }
}

#[test]
fn reads_a_half_before_or_after_its_scale() {
    for (text, written, value) in [
        (
            "two and a half million",
            "two and a half million",
            "2500000",
        ),
        (
            "one and a half million",
            "one and a half million",
            "1500000",
        ),
        ("two and a half thousand", "two and a half thousand", "2500"),
        ("half a million", "half a million", "500000"),
        ("a half million", "a half million", "500000"),
        ("a million and a half", "a million and a half", "1500000"),
        (
            "two million and a half",
            "two million and a half",
            "2500000",
        ),
        ("deux millions et demi", "deux millions et demi", "2500000"),
        ("un milliard et demi", "un milliard et demi", "1500000000"),
        ("un demi-million", "un demi-million", "500000"),
        ("dos millones y medio", "dos millones y medio", "2500000"),
        ("un millón y medio", "un millón y medio", "1500000"),
        ("millón y medio", "millón y medio", "1500000"),
        ("medio millón", "medio millón", "500000"),
        ("due milioni e mezzo", "due milioni e mezzo", "2500000"),
        ("mezzo milione", "mezzo milione", "500000"),
        ("dois milhões e meio", "dois milhões e meio", "2500000"),
        ("meio milhão", "meio milhão", "500000"),
        ("zweieinhalb Millionen", "zweieinhalb Millionen", "2500000"),
        ("anderthalb Millionen", "anderthalb Millionen", "1500000"),
        ("dreieinhalbtausend", "dreieinhalbtausend", "3500"),
    ] {
        assert_eq!(
            spelled(text),
            [(written.to_owned(), value.to_owned())],
            "{text}"
        );
    }
}

#[test]
fn reads_a_decimal_point_before_a_scale() {
    for (text, written, value) in [
        ("one point two million", "one point two million", "1200000"),
        (
            "two point five billion",
            "two point five billion",
            "2500000000",
        ),
        (
            "one point two five million",
            "one point two five million",
            "1250000",
        ),
        (
            "zero point five million",
            "zero point five million",
            "500000",
        ),
        (
            "deux virgule cinq millions",
            "deux virgule cinq millions",
            "2500000",
        ),
        (
            "zwei Komma fünf Millionen",
            "zwei Komma fünf Millionen",
            "2500000",
        ),
        (
            "uno coma cinco millones",
            "uno coma cinco millones",
            "1500000",
        ),
        (
            "uno coma cinco mil millones",
            "uno coma cinco mil millones",
            "1500000000",
        ),
        (
            "due virgola cinque milioni",
            "due virgola cinque milioni",
            "2500000",
        ),
        (
            "dois vírgula cinco milhões",
            "dois vírgula cinco milhões",
            "2500000",
        ),
        ("one point five lakh", "one point five lakh", "150000"),
    ] {
        assert_eq!(
            spelled(text),
            [(written.to_owned(), value.to_owned())],
            "{text}"
        );
    }
}

#[test]
fn reads_no_half_or_decimal_without_its_scale() {
    for (text, read) in [
        ("two and a half", vec![]),
        ("one point two", vec![]),
        ("a five point plan", vec![]),
        ("two hundred and a half", vec![("two hundred", "200")]),
        (
            "one point two million three hundred",
            vec![
                ("one point two million", "1200000"),
                ("three hundred", "300"),
            ],
        ),
        (
            "deux millions et demi et trois",
            vec![("deux millions et demi", "2500000")],
        ),
        ("half twelve thousand", vec![("twelve thousand", "12000")]),
    ] {
        let read: Vec<_> = read
            .into_iter()
            .map(|(w, v)| (w.to_owned(), v.to_owned()))
            .collect();
        assert_eq!(spelled(text), read, "{text}");
    }
}

#[test]
fn reads_no_lone_scale_word_mixed_language_or_word_that_only_holds_one() {
    for text in [
        "mille grazie",
        "mil gracias",
        "the first thousand",
        "thousands of users",
        "twenty cents",
        "The percent of similar ability, in a recent century.",
        "Zweitausender",
        "万一",
        "百貨店",
    ] {
        assert_eq!(spelled(text), Vec::<(String, String)>::new(), "{text}");
    }
}

#[test]
fn starts_a_number_where_its_words_begin_to_make_one() {
    assert_eq!(
        spelled("and twelve thousand"),
        [("twelve thousand".to_owned(), "12000".to_owned())]
    );
    assert_eq!(
        spelled("one two thousand"),
        [("two thousand".to_owned(), "2000".to_owned())]
    );
}

#[test]
fn reads_digits_that_the_words_continue_as_one_number() {
    for (text, read) in [
        (
            "3 thousand five hundred",
            vec![("3 thousand five hundred", "3500")],
        ),
        (
            "2 million three hundred thousand",
            vec![("2 million three hundred thousand", "2300000")],
        ),
        (
            "2 and a half million",
            vec![("2 and a half million", "2500000")],
        ),
        ("1 millón y medio", vec![("1 millón y medio", "1500000")]),
        ("3 mil millones", vec![("3 mil millones", "3000000000")]),
        ("3 thousand", vec![("3 thousand", "3000")]),
        ("1.5 million and a half", vec![("1.5 million", "1500000")]),
        (
            "12.5 thousand five hundred",
            vec![("12.5 thousand", "12500"), ("five hundred", "500")],
        ),
    ] {
        let read: Vec<_> = read
            .into_iter()
            .map(|(w, v)| (w.to_owned(), v.to_owned()))
            .collect();
        let found: Vec<_> = extract_numbers(text)
            .into_iter()
            .filter(|n| !n.derived)
            .map(|n| (n.text.to_owned(), n.integer.into_owned()))
            .collect();
        assert_eq!(found, read, "{text}");
        let lazily: Vec<_> = numbers(text)
            .filter(|n| !n.derived)
            .map(|n| (n.text.to_owned(), n.integer.into_owned()))
            .collect();
        assert_eq!(lazily, read, "{text}");
    }
}

#[test]
fn holds_a_number_led_by_digits_to_the_sign_before_them() {
    let read = extract_numbers("-2 and a half million");
    assert_eq!(
        read.iter()
            .map(|n| (n.full_text, n.sign))
            .collect::<Vec<_>>(),
        [("-2 and a half million", Sign::Minus)]
    );
    assert_eq!(
        ungrounded_figures("It fell by -2 and a half million.", "change: 2,500,000"),
        ["-2 and a half million"]
    );
    assert_eq!(
        ungrounded_figures("It fell by -2 and a half million.", "change: -2,500,000"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_digits_and_words_in_order() {
    let read: Vec<_> = numbers("From 12,500 to twenty thousand, then 99,999.")
        .filter(|n| !n.derived && n.fraction.is_none())
        .map(|n| n.text)
        .collect();
    assert_eq!(read, ["12,500", "twenty thousand", "99,999"]);
}

#[test]
fn grounds_a_number_in_words_by_a_value_that_rounds_to_it() {
    let reply = "We signed twelve thousand five hundred users.";
    assert_eq!(
        ungrounded_figures(reply, "users: 12487"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(reply, "users: 12600"),
        ["twelve thousand five hundred"]
    );
}

#[test]
fn grounds_digits_by_a_corpus_that_writes_them_in_words() {
    let corpus = "On the call: revenue was one million two hundred thousand.";
    assert_eq!(
        ungrounded_figures("Revenue is 1,200,000.", corpus),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("Revenue is 1,300,000.", corpus),
        ["1,300,000"]
    );
}

#[test]
fn holds_no_idiom_and_no_number_under_the_floor() {
    assert_eq!(
        ungrounded_figures("Thanks a million!", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("five thousand users", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("a hundred thousand rows", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("one hundred thousand rows", ""),
        ["one hundred thousand"]
    );
    assert_eq!(
        ungrounded_figures("Revenue is 1,000,000.", "thanks a million"),
        Vec::<String>::new()
    );
}

#[test]
fn grounds_a_half_or_a_decimal_in_words_like_the_figure_in_digits() {
    assert_eq!(
        ungrounded_figures(
            "We reached two and a half million users.",
            "users: 2,512,300"
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(
            "We reached two and a half million users.",
            "users: 2,600,000"
        ),
        ["two and a half million"]
    );
    assert_eq!(
        ungrounded_figures(
            "Revenue is $1.2 million.",
            "Revenue was one point two million."
        ),
        Vec::<String>::new()
    );
}

#[test]
fn names_digits_and_words_in_the_order_the_reply_wrote_them() {
    assert_eq!(
        ungrounded_figures("From 12,500 to twenty thousand, then 99,999.", ""),
        ["12,500", "twenty thousand", "99,999"]
    );
}

/// The numbers `text` states, digits or words: as written, and their value.
fn stated(text: &str) -> Vec<(String, String)> {
    extract_numbers(text)
        .into_iter()
        .filter(|n| !n.derived)
        .map(|n| (n.text.to_owned(), n.integer.into_owned()))
        .collect()
}

#[test]
fn digits_lead_only_into_a_hundred_a_scale_a_half_or_a_point() {
    for (text, read) in [
        (
            "On June 30 two hundred thousand people marched",
            vec![("30", "30"), ("two hundred thousand", "200000")],
        ),
        (
            "Since April 10 two million users",
            vec![("10", "10"), ("two million", "2000000")],
        ),
        (
            "the 10 two-million-dollar homes",
            vec![("10", "10"), ("two-million", "2000000")],
        ),
        (
            "at 10:30 two hundred thousand",
            vec![
                ("10", "10"),
                ("30", "30"),
                ("two hundred thousand", "200000"),
            ],
        ),
        (
            "2 and a half million",
            vec![("2 and a half million", "2500000")],
        ),
    ] {
        let read: Vec<_> = read
            .into_iter()
            .map(|(w, v)| (w.to_owned(), v.to_owned()))
            .collect();
        assert_eq!(stated(text), read, "{text}");
    }
    assert_eq!(
        ungrounded_figures("the 10 two-million-dollar homes", "2,000,000"),
        Vec::<String>::new()
    );
}

#[test]
fn a_phrase_that_breaks_restarts_after_its_last_complete_prefix() {
    let reply = "costs between 5 and a hundred thousand dollars";
    assert_eq!(
        spelled(reply),
        [("a hundred thousand".to_owned(), "100000".to_owned())]
    );
    assert_eq!(
        ungrounded_figures(reply, "range 5 to 100"),
        Vec::<String>::new()
    );
    for (text, value) in [
        ("At one point two hundred thousand people", "200000"),
        ("one point five hundred thousand", "500000"),
    ] {
        assert_eq!(
            spelled(text),
            [(
                text.trim_start_matches("At one point ")
                    .trim_start_matches("one point ")
                    .trim_end_matches(" people")
                    .to_owned(),
                value.to_owned()
            )],
            "{text}"
        );
    }
}

#[test]
fn a_number_word_in_a_compound_is_no_part_of_a_number() {
    let reply = "We sold 5 million two-bedroom units";
    assert_eq!(
        stated(reply),
        [("5 million".to_owned(), "5000000".to_owned())]
    );
    assert_eq!(
        ungrounded_figures(reply, "units: 5,000,000"),
        Vec::<String>::new()
    );
    assert_eq!(
        spelled("a forty-foot thousand-seat hall and twenty-five thousand fans"),
        [("twenty-five thousand".to_owned(), "25000".to_owned())]
    );
}

#[test]
fn reads_a_fraction_written_in_two_digit_words() {
    for (text, value) in [
        ("deux virgule vingt-cinq millions", "2250000"),
        ("zwei Komma fünfundzwanzig Millionen", "2250000"),
        ("dos coma veinticinco millones", "2250000"),
        ("one point twenty five million", "1250000"),
        ("un virgule soixante-dix millions", "1700000"),
        ("one point zero five million", "1050000"),
        ("zero point five million", "500000"),
        ("null Komma fünf Millionen", "500000"),
    ] {
        assert_eq!(
            spelled(text),
            [(text.to_owned(), value.to_owned())],
            "{text}"
        );
    }
}

#[test]
fn a_half_ends_the_number_as_a_decimal_does() {
    let reply = "We hit two and a half million three years ago.";
    assert_eq!(
        spelled(reply),
        [("two and a half million".to_owned(), "2500000".to_owned())]
    );
    assert_eq!(
        ungrounded_figures(reply, "users: 2,500,000"),
        Vec::<String>::new()
    );
    for (text, written, value) in [
        ("half a million two years ago", "half a million", "500000"),
        (
            "zweieinhalb Millionen drei Jahre",
            "zweieinhalb Millionen",
            "2500000",
        ),
        ("two million three years ago", "two million", "2000000"),
        ("three thousand twenty", "three thousand twenty", "3020"),
        (
            "two million three hundred",
            "two million three hundred",
            "2000300",
        ),
    ] {
        assert_eq!(
            spelled(text),
            [(written.to_owned(), value.to_owned())],
            "{text}"
        );
    }
    assert_eq!(
        stated("2 and a half million three years ago"),
        [
            ("2 and a half million".to_owned(), "2500000".to_owned()),
            ("three".to_owned(), "3".to_owned())
        ][..1]
    );
}

#[test]
fn a_half_opened_by_an_article_is_a_count() {
    for reply in [
        "we reached a half million users",
        "we reached half a million users",
    ] {
        assert_eq!(
            ungrounded_figures(reply, "users: 480,000"),
            Vec::<String>::new(),
            "{reply}"
        );
        assert_eq!(
            ungrounded_figures(reply, "users: 380,000").len(),
            1,
            "{reply}"
        );
    }
    assert_eq!(
        ungrounded_figures("thanks a million", ""),
        Vec::<String>::new()
    );
}

#[test]
fn digits_lead_the_words_where_the_scanner_reads_them_as_a_figure() {
    for text in ["x_2 and a half million", "売上2 and a half million"] {
        assert_eq!(
            stated(text),
            [("2 and a half million".to_owned(), "2500000".to_owned())],
            "{text}"
        );
    }
    assert_eq!(
        stated("p2 and a half million"),
        [("a half million".to_owned(), "500000".to_owned())]
    );
}

#[test]
fn reads_a_fraction_written_in_digits_after_the_point() {
    for (text, value) in [
        ("2 point 5 million", "2500000"),
        ("2 point 25 million", "2250000"),
        ("1 point 05 million", "1050000"),
    ] {
        assert_eq!(
            stated(text),
            [(text.to_owned(), value.to_owned())],
            "{text}"
        );
    }
    assert_eq!(
        ungrounded_figures("Revenue hit 2 point 5 million.", "revenue: 2,500,000"),
        Vec::<String>::new()
    );
}

#[test]
fn zero_opens_only_a_decimal() {
    assert_eq!(
        spelled("zero hundred thousand"),
        [("hundred thousand".to_owned(), "100000".to_owned())]
    );
    for text in ["twenty zero thousand", "cero mil"] {
        assert_eq!(spelled(text), [], "{text}");
    }
}
