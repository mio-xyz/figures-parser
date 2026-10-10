use figures_parser::{extract_numbers, numbers, ungrounded_figures};

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
fn names_digits_and_words_in_the_order_the_reply_wrote_them() {
    assert_eq!(
        ungrounded_figures("From 12,500 to twenty thousand, then 99,999.", ""),
        ["12,500", "twenty thousand", "99,999"]
    );
}
