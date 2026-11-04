use std::borrow::Cow;

use figures_parser::{Sign, extract_numbers, ungrounded_figures};

/// No finding.
const fn none() -> Vec<String> {
    Vec::new()
}

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
        "5mm thick",
        "12 K",
        "5 M users",
        "5Mbps",
        "12.5kg",
        "5 mi away",
        "a 7 B model",
        "5 bin bags",
        "12.5K9",
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
        ungrounded_figures("It fell -$1.2M.", "delta: +1234567"),
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
fn reads_a_scaled_run_that_groups_as_a_decimal_where_the_word_writes_that_mark() {
    // A comma decimal before a word of a language that writes one; a point
    // before an English word.
    for (reply, corpus) in [
        ("Долг 8,566 млрд.", "8566000000"),
        ("Umsatz 1,250 Mio. Euro.", "1250000"),
        ("Budget 1,250 M€.", "1250000000"),
        ("Revenue of 1.250 million.", "1250000"),
        ("Revenue of 1.250 million.", "1250000000"),
        ("Pendapatan 12,500 rb.", "12500"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{reply} | {corpus}"
        );
    }
    // Elsewhere the run is thousands, as "$206,788" is: before a letter every
    // locale writes alike, a currency after it included, since an English
    // report writes "1,250 M€" for 1.25 billion.
    for (reply, corpus) in [
        ("Budget 1,250 M€.", "1250000"),
        ("Total 46,886b €", "46886"),
        ("Debt of €12,500 million.", "12500000"),
        ("Debt of £1,250 million.", "1250000"),
        ("Budget 1.250 M€.", "1250000"),
        ("Umsatz 1.250 Mio. Euro.", "1250000"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus).len(),
            1,
            "{reply} | {corpus}"
        );
    }
    assert_eq!(
        ungrounded_figures("Debt of €12,500 million.", "12500000000"),
        Vec::<String>::new()
    );
}

#[test]
fn holds_a_scaled_figure_whose_digits_group_to_the_grouped_reading() {
    let reply = "Debt of €12,500 million.";
    assert_eq!(
        ungrounded_figures(reply, "12500000000"),
        Vec::<String>::new()
    );
    for corpus in ["12500000", "12600000"] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            ["12,500 million"],
            "{corpus}"
        );
    }
}

#[test]
fn holds_a_scaled_figure_to_the_floors() {
    assert_eq!(ungrounded_figures("5k users", "nothing"), ["5k"]);
    assert_eq!(
        ungrounded_figures("2 hundred users", "nothing"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded_figures("$5k budget", "nothing"), ["5k"]);
}

#[test]
fn reads_the_sections_written_after_a_han_multiplier() {
    for (text, written, integer) in [
        ("売上高は3億5000万円", "3億5000万", "350000000"),
        ("3億5,000万円", "3億5,000万", "350000000"),
        ("３億５０００万円", "３億５０００万", "350000000"),
        ("1億2345万6789円", "1億2345万6789", "123456789"),
        ("1万2千", "1万2千", "12000"),
        ("2万5000円", "2万5000", "25000"),
        ("1万500円", "1万500", "10500"),
        ("3亿5000万元", "3亿5000万", "350000000"),
        ("1兆2000億", "1兆2000億", "1200000000000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
}

#[test]
fn ends_the_sections_where_they_stop_fitting_below_the_multiplier() {
    for (text, written, integer) in [
        ("3万5", "3万", "30000"),
        ("1万12000", "1万", "10000"),
        ("3万5億", "3万", "30000"),
        ("2万5000.5", "2万", "20000"),
        ("2万5000kg", "2万", "20000"),
        ("3万〜5万", "3万", "30000"),
        ("1.2万25", "1.2万", "12000"),
        ("1,250万5000", "1,250万", "12500000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
}

#[test]
fn grounds_a_han_figure_written_in_sections_as_one_value() {
    assert_eq!(
        ungrounded_figures("売上高は3億5000万円でした。", "revenue: 350,000,000"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("Revenue was 350,000,000.", "売上高 3億5,000万円"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures("売上高は3億5000万円でした。", "revenue: 300,000,000"),
        ["3億5000万"]
    );
}

#[test]
fn a_small_multiplier_before_a_large_one_scales_the_whole() {
    for (text, written, integer) in [
        ("売上1千200万円", "1千200万", "12000000"),
        ("2千500億円", "2千500億", "250000000000"),
        ("3千5百万円", "3千5百万", "35000000"),
        ("1千2百50万", "1千2百50万", "12500000"),
        ("1千200万3000", "1千200万3000", "12003000"),
        ("5千万3000", "5千万3000", "50003000"),
        ("1億2千万3000", "1億2千万3000", "120003000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
    assert_eq!(
        ungrounded_figures("Revenue 1千200万円", "revenue: 12000000"),
        Vec::<String>::new()
    );
}

#[test]
fn a_section_short_of_the_myriad_after_a_hundred_million_fills_it() {
    for (text, written, integer) in [
        ("3億5千", "3億5千", "350000000"),
        ("3億5000", "3億5000", "350000000"),
        ("1兆2千", "1兆2千", "1200000000000"),
        ("1兆2000", "1兆2000", "1200000000000"),
        ("3億5千2百", "3億5千", "350000000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
    assert_eq!(
        ungrounded_figures("3億5千円", "revenue: 350,000,000"),
        Vec::<String>::new()
    );
}

#[test]
fn a_han_figure_past_the_integer_type_keeps_its_head() {
    let text = "12345678901234567890123456789012345678万5000";
    assert_eq!(
        first(text),
        Some((
            "12345678901234567890123456789012345678万".to_owned(),
            "123456789012345678901234567890123456780000".to_owned(),
            None,
            true
        ))
    );
}

#[test]
fn reads_scale_words_glued_and_in_more_languages() {
    for (text, written, integer) in [
        ("12million users", "12million", "12000000"),
        ("3 miljoen", "3 miljoen", "3000000"),
        ("2 miljarder", "2 miljarder", "2000000000"),
        ("4 miliony", "4 miliony", "4000000"),
        ("3 tysiące", "3 tysiące", "3000"),
        ("5 milyar", "5 milyar", "5000000000"),
        ("7 juta", "7 juta", "7000000"),
        ("2 triệu", "2 triệu", "2000000"),
        ("3 tỷ", "3 tỷ", "3000000000"),
        ("1,5 bln", "1,5 bln", "1500000000000"),
        ("5 mila miliardi", "5 mila miliardi", "5000000000000"),
        ("2 lakh crore", "2 lakh crore", "2000000000000"),
        ("3 млн рублей", "3 млн", "3000000"),
        ("2 миллиарда", "2 миллиарда", "2000000000"),
        ("5 مليون", "5 مليون", "5000000"),
        ("4 میلیارد", "4 میلیارد", "4000000000"),
        ("3 מיליון", "3 מיליון", "3000000"),
        ("2 लाख", "2 लाख", "200000"),
        ("5ล้านคน", "5ล้าน", "5000000"),
        ("3억 명", "3억", "300000000"),
        ("3억5000만원", "3억5000만", "350000000"),
        ("5천만", "5천만", "50000000"),
        ("a 7B model", "7B", "7000000000"),
        ("2.8T tokens", "2.8T", "2800000000000"),
        ("1.5b users", "1.5b", "1500000000"),
    ] {
        assert_eq!(
            first(text),
            Some((written.to_owned(), integer.to_owned(), None, true)),
            "{text}"
        );
    }
}

#[test]
fn checks_a_count_written_with_a_scale_and_no_currency() {
    let reply = "The district counts 3.2M teachers.";
    assert_eq!(ungrounded_figures(reply, ""), ["3.2M"]);
    assert_eq!(ungrounded_figures(reply, "teachers: 3,312,000"), ["3.2M"]);
    for corpus in [
        "teachers: 3,213,480",
        "3.2 million teachers in the district",
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    let counts = "12k signups, 1.5bn views, 3.2 million users.";
    assert_eq!(
        ungrounded_figures(counts, "signups 12040\nviews 1500000000\nusers 3190000"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_figures(counts, ""),
        ["12k", "1.5bn", "3.2 million"]
    );
    assert_eq!(
        ungrounded_figures("A 16B model at 4K resolution with 3M users.", ""),
        ["16B", "4K", "3M"]
    );
    assert_eq!(
        ungrounded_figures("See p1789368220119 and v3.1, about 12.5K.", "visits 12480"),
        Vec::<String>::new()
    );
}

#[test]
fn is_not_grounded_by_a_bare_printed_figure_of_the_same_digits() {
    assert_eq!(
        ungrounded_figures("The district counts 3.2M teachers.", "ratio 3.2"),
        ["3.2M"]
    );
}

#[test]
fn reads_a_lone_letter_a_space_parts_from_the_digits_as_a_unit_or_a_word() {
    for reply in [
        "A 500 m mesh at 1750 N Highway 121, 3 M users.",
        "Le prélèvement du 27/10/26 n'a pas pu être effectué.",
        "The job took 5m.",
    ] {
        assert_eq!(
            ungrounded_figures(reply, ""),
            Vec::<String>::new(),
            "{reply}"
        );
    }
}

#[test]
fn reads_a_corpus_scale_loosely() {
    for (reply, corpus) in [
        (
            "r/matheducation has 43K members.",
            "r/matheducation 43Kmembers Forum",
        ),
        ("1.5bn views", "1.5bnviews"),
        ("12K users", "size 12Kb"),
        ("3.2 million teachers", "3.2millionteachers"),
        ("$38M revenue", "revenue 38.08 M"),
        ("$38M revenue", "revenue 38.08 Mio"),
        ("$38M revenue", "revenue 38.08m"),
        ("$5,000,000 raised", "took 5 m"),
        ("$12,000 spent", "12 k rows"),
        ("€2,5 Mio. Umsatz", "2,5 Mio"),
        ("$1,200,000 total", "total 1.2 M"),
        ("Revenue $3M.", "revenue 3 млн"),
        ("Revenue $3M.", "revenue 3млн"),
        ("Budget 3 Mrd. €", "budget 3 mrd"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{reply} | {corpus}"
        );
    }
    assert_eq!(
        ungrounded_figures(
            "r/matheducation has 43K members.",
            "r/matheducation 43members"
        ),
        ["43K"]
    );
}

#[test]
fn keeps_the_digits_of_a_corpus_figure_read_with_a_scale() {
    for (reply, corpus) in [
        ("Ran 1,500 laps.", "ran 1500 m"),
        ("Signups 12,500 overall.", "signups 12,500 k-factor"),
        ("Shipped 2,500 units.", "shipped 2,500 M"),
    ] {
        assert_eq!(
            ungrounded_figures(reply, corpus),
            Vec::<String>::new(),
            "{reply} | {corpus}"
        );
    }
}

#[test]
fn reads_an_abbreviation_at_the_scale_of_the_language_that_writes_it_so() {
    // "tn" is a thousand in Swedish and a trillion in English; a reply is
    // read at the one English writes.
    assert_eq!(ungrounded_figures("Raised 2tn.", "raised 2000"), ["2tn"]);
    assert_eq!(
        ungrounded_figures("Raised 2tn.", "raised 2000000000000"),
        Vec::<String>::new()
    );
}

/// What a generated figure is built from: a mark, a currency, digits, a gap,
/// a scale word, and what follows it.
const MARKS: &[&str] = &["", "", "~", "≈ "];
const CURRENCIES: &[&str] = &["", "", "$", "€ ", "-"];
const FIGURES: &[&str] = &[
    "12", "3.2", "1,250", "2,5", "0.5", "7", "38.08", "1 811", "12,500",
];
const GAPS: &[&str] = &["", " ", "\u{a0}"];
const SCALES: &[&str] = &[
    "",
    "M",
    "m",
    "K",
    "k",
    "B",
    "b",
    "T",
    "bn",
    "tn",
    "Mio",
    "Mio.",
    "Mrd",
    "mln",
    "bln",
    "million",
    "Millionen",
    "mil",
    "mil millones",
    "mila miliardi",
    "lakh",
    "crore",
    "miljoen",
    "tysiące",
    "tỷ",
    "млн",
    "مليون",
    "ล้าน",
    "억",
    "만",
    "천",
    "万",
    "億",
    "千",
    "€",
];
const TAILS: &[&str] = &["", " users", "members", ".", ",", "\n", ")", "x", "%"];

#[test]
fn every_generated_text_grounds_its_own_scaled_figures() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut pick = |from: &[&'static str]| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let at = usize::try_from(state % u64::try_from(from.len()).unwrap_or(1)).unwrap_or(0);
        from.get(at).copied().unwrap_or("")
    };
    for _ in 0..20_000 {
        let mut text = String::new();
        for _ in 0..3 {
            for part in [MARKS, CURRENCIES, FIGURES, GAPS, SCALES, TAILS] {
                text.push_str(pick(part));
            }
            text.push(' ');
        }
        assert_eq!(
            ungrounded_figures(&text, &text),
            Vec::<String>::new(),
            "{text:?}"
        );
    }
}

#[test]
fn every_scale_word_of_another_script_starts_with_a_listed_initial() {
    for word in [
        "тыс",
        "млн",
        "млрд",
        "трлн",
        "ألف",
        "آلاف",
        "مليون",
        "ملايين",
        "مليار",
        "ترليون",
        "هزار",
        "میلیون",
        "میلیارد",
        "تریلیون",
        "אלף",
        "מיליון",
        "מיליארד",
        "טריליון",
        "हज़ार",
        "लाख",
        "अरब",
        "खरब",
        "พัน",
        "ล้าน",
    ] {
        assert_eq!(
            first(&format!("3 {word}")).map(|(written, ..)| written),
            Some(format!("3 {word}")),
            "{word}"
        );
    }
}

#[test]
fn reads_an_abbreviation_that_is_also_a_unit_as_a_scale_unless_the_words_around_name_the_unit() {
    for reply in [
        "We have 3.2m users.",
        "Revenue 206,788m.",
        "Revenue 872 426m",
        "Budget 100,000m d'euros",
        "Revenue 5 mn",
        "Revenue 5mn people",
        "Gelir 8 mn",
        "Revenue 58 md",
        "Revenue 5 Mio",
        "Revenue 12 mi",
        "Revenue 5 cr",
        "Gelir 5 Mr",
        "Gelir 729 bin",
        "Revenue 25 tn",
        "Umsatz 5 Bio",
        "Revenue 3t",
        "Revenue 5L",
    ] {
        assert_eq!(ungrounded_figures(reply, "").len(), 1, "{reply}");
    }
    for reply in [
        "The job took 5m.",
        "Took about 5m.",
        "It finished 5m ago.",
        "Done in 1h 5m.",
        "Waited 5m 30s.",
        "a 3D model, 5m tall",
        "A room 3m x 4m.",
        "Took 165489 mn.",
        "Ça prend en 5 mn.",
        "Wrote 5 md files.",
        "Ran 5 mi.",
        "It is 5 mi away.",
        "Earned 92 cr this term.",
        "Table 5 Mr Smith.",
        "Bought 5 bin bags.",
        "It weighs 25 tn.",
        "A 2481t truck.",
        "300l of water.",
        "Ate 3 Bio eggs.",
    ] {
        assert_eq!(ungrounded_figures(reply, ""), none(), "{reply}");
    }
}

#[test]
fn holds_a_figure_with_a_compact_scale_whatever_its_value() {
    for reply in [
        "Total 0.95k",
        "Total 0.957k",
        "Revenue 0,81k",
        "Total 0.95 mil",
        "Total 0,94 тыс.",
        "Total 0,94 हज़ार",
        "Total 0.4661 mil",
    ] {
        assert_eq!(ungrounded_figures(reply, "").len(), 1, "{reply}");
    }
    assert_eq!(ungrounded_figures("Total 0.95k", "950"), none());
}
