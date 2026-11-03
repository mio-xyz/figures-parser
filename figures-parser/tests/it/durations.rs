use figures_parser::{Length, durations, ungrounded_durations};

const MINUTE: u64 = 60_000;
const HOUR: u64 = 3_600_000;
const DAY: u64 = 86_400_000;

fn read(text: &str) -> Vec<(&str, Length)> {
    durations(text)
        .into_iter()
        .map(|d| (d.text, d.length))
        .collect()
}

fn one(text: &str) -> Length {
    let found = read(text);
    assert_eq!(found.len(), 1, "{text:?} read as {found:?}");
    found[0].1
}

const fn clock(milliseconds: u64) -> Length {
    Length::clock(milliseconds)
}

#[test]
fn reads_an_amount_and_a_unit_in_digits_or_words() {
    assert_eq!(
        read("The call lasted 45 minutes."),
        [("45 minutes", clock(45 * MINUTE))]
    );
    assert_eq!(one("a 45-minute call"), clock(45 * MINUTE));
    assert_eq!(one("took 250ms"), clock(250));
    assert_eq!(one("in 12s"), clock(12_000));
    assert_eq!(one("1,500 hours logged"), clock(1_500 * HOUR));
    assert_eq!(one("forty-five minutes"), clock(45 * MINUTE));
    assert_eq!(one("twenty one days"), clock(21 * DAY));
    assert_eq!(one("a couple of days"), clock(2 * DAY));
    assert_eq!(one("3 months"), Length::calendar(300));
    assert_eq!(one("2 quarters"), Length::calendar(600));
    assert_eq!(one("1.5 years"), Length::calendar(1_800));
}

#[test]
fn reads_halves_decimals_and_the_hour_with_its_minutes() {
    for text in [
        "1h30",
        "1h30m",
        "1 h 30 min",
        "1.5 hours",
        "1,5 heures",
        "1½ hours",
        "an hour and a half",
        "one and a half hours",
        "1 hour and 30 minutes",
        "1 hour, 30 minutes",
        "90 min",
    ] {
        assert_eq!(read(text), [(text, clock(90 * MINUTE))], "{text}");
    }
}

#[test]
fn reads_fixed_phrases() {
    assert_eq!(one("half an hour"), clock(30 * MINUTE));
    assert_eq!(one("a quarter of an hour"), clock(15 * MINUTE));
    assert_eq!(one("three quarters of an hour"), clock(45 * MINUTE));
    assert_eq!(one("a fortnight"), clock(14 * DAY));
    assert_eq!(one("1/2 h"), clock(30 * MINUTE));
}

#[test]
fn reads_french_spanish_italian_and_portuguese() {
    assert_eq!(one("deux heures et demie"), clock(150 * MINUTE));
    assert_eq!(one("une demi-heure"), clock(30 * MINUTE));
    assert_eq!(one("un quart d'heure"), clock(15 * MINUTE));
    assert_eq!(one("soixante-quinze minutes"), clock(75 * MINUTE));
    assert_eq!(one("vingt et un jours"), clock(21 * DAY));
    assert_eq!(one("2 jours, 4 heures"), clock(2 * DAY + 4 * HOUR));
    assert_eq!(one("dos horas y media"), clock(150 * MINUTE));
    assert_eq!(one("media hora"), clock(30 * MINUTE));
    assert_eq!(one("veinticinco minutos"), clock(25 * MINUTE));
    assert_eq!(one("due ore e mezza"), clock(150 * MINUTE));
    assert_eq!(one("un'ora"), clock(HOUR));
    assert_eq!(one("trentacinque minuti"), clock(35 * MINUTE));
    assert_eq!(one("mezz'ora"), clock(30 * MINUTE));
    assert_eq!(one("duas horas e meia"), clock(150 * MINUTE));
    assert_eq!(one("trinta e cinco minutos"), clock(35 * MINUTE));
    assert_eq!(one("3 años"), Length::calendar(3_600));
}

#[test]
fn reads_no_duration_in_rates_dimensions_or_glued_words() {
    for text in [
        "once a day",
        "a day ago",
        "a 3D model, 5m tall",
        "$50/h, 60 km/h, per hour",
        "Q1 an increase, v2 hours",
        "très bien, tres bien, dose",
        "seven",
    ] {
        assert_eq!(read(text), Vec::<(&str, Length)>::new(), "{text}");
    }
}

#[test]
fn keeps_separate_durations_apart_and_slices_the_input_at_each_span() {
    let text = "😀 2 hours, then 30 minutes later another 2 hours.";
    let found = durations(text);
    let texts: Vec<&str> = found.iter().map(|d| d.text).collect();
    assert_eq!(texts, ["2 hours", "30 minutes", "2 hours"]);
    for d in &found {
        assert_eq!(text.get(d.span.clone()), Some(d.text));
    }
}

#[test]
fn grounds_a_length_however_the_corpus_writes_it() {
    let reply = "The call lasted 45 minutes.";
    for corpus in [
        r#"{"duration": 2700}"#,
        r#"{"duration_ms": 2700000}"#,
        r#"{"durationMinutes": 45}"#,
        r#"{"length": "PT45M"}"#,
        "recording 00:45:00",
        "recording 45:00",
        "length 0:45",
        "took about 0:45:00",
        "it took three quarters of an hour",
        "45 MINUTES",
    ] {
        assert_eq!(
            ungrounded_durations(reply, corpus, ""),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
    assert_eq!(
        ungrounded_durations("90 minutes", "1 hour 30 minutes", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("1h30", "took 90 min", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("1 year", "P12M", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("15 minutes", "1 hour, 30 minutes, 15 minutes", ""),
        Vec::<String>::new()
    );
}

#[test]
fn holds_a_part_of_one_length_to_that_length() {
    for corpus in ["1 hour 30 minutes", "5-minute-30-second", "130 minutes"] {
        assert_eq!(
            ungrounded_durations("30 minutes", corpus, ""),
            ["30 minutes"],
            "{corpus}"
        );
    }
    assert_eq!(
        ungrounded_durations("took 30 minutes", "10-30 minutes", ""),
        ["30 minutes"]
    );
}

#[test]
fn flags_a_length_nothing_gave_once_and_grounds_one_the_prompt_gave() {
    let corpus = r#"{"duration": 45}"#;
    assert_eq!(
        ungrounded_durations("It took 2 hours, then 2 Hours again.", corpus, ""),
        ["2 hours"]
    );
    assert_eq!(
        ungrounded_durations("Reply within 24 hours.", "", "Answer within 24 hours."),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("No length here.", "", ""),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_clock_reading_with_its_decimals_but_not_a_timestamp() {
    assert_eq!(
        ungrounded_durations(
            "1 hour, 2 minutes and 3.5 seconds",
            r#"{"elapsed": "1:02:03.5"}"#,
            ""
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("14 hours and 30 minutes", "at 2026-10-09T14:30:00Z", ""),
        ["14 hours and 30 minutes"]
    );
    assert_eq!(
        ungrounded_durations("12 minutes", "duration:12:30", ""),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_glued_run_of_units() {
    assert_eq!(one("took 1h5m30s"), clock(HOUR + 5 * MINUTE + 30_000));
    assert_eq!(one("took 5m30s"), clock(5 * MINUTE + 30_000));
    assert_eq!(read("2 hours 30 people"), [("2 hours", clock(2 * HOUR))]);
}

#[test]
fn a_number_before_a_bare_s_grounds_but_is_never_held() {
    let found = durations("back in the 90s, 100s of users, a 30s timeout, 30 s");
    let stated: Vec<(&str, bool)> = found.iter().map(|d| (d.text, d.stated)).collect();
    assert_eq!(
        stated,
        [
            ("90s", false),
            ("100s", false),
            ("30s", false),
            ("30 s", false)
        ]
    );
    assert_eq!(
        ungrounded_durations("back in the 90s, 100s of users, a 30s timeout", "", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations("it waited 30 seconds", "timeout=30s", ""),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_field_named_by_any_word_of_its_unit_and_no_time_of_day() {
    assert_eq!(
        ungrounded_durations("It ran 2 hours.", r#"{"elapsed_hour": 2}"#, ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_durations(
            "14 hours and 30 minutes",
            "log 2026-10-09 14:30:00 started",
            ""
        ),
        ["14 hours and 30 minutes"]
    );
}

#[test]
fn reads_arabic_amounts_units_and_fractions() {
    const WEEK: u64 = 7 * DAY;
    for (text, written, length) in [
        ("استغرق الاجتماع 45 دقيقة.", "45 دقيقة", clock(45 * MINUTE)),
        ("خلال ٣ ساعات", "٣ ساعات", clock(3 * HOUR)),
        ("ساعتين", "ساعتين", clock(2 * HOUR)),
        ("بساعتين", "ساعتين", clock(2 * HOUR)),
        (
            "ساعتين و30 دقيقة",
            "ساعتين و30 دقيقة",
            clock(2 * HOUR + 30 * MINUTE),
        ),
        (
            "ساعة و20 دقيقة",
            "ساعة و20 دقيقة",
            clock(HOUR + 20 * MINUTE),
        ),
        (
            "ساعة وعشر دقائق",
            "ساعة وعشر دقائق",
            clock(HOUR + 10 * MINUTE),
        ),
        ("ساعة ونصف", "ساعة ونصف", clock(HOUR + 30 * MINUTE)),
        ("ساعتين ونص", "ساعتين ونص", clock(2 * HOUR + 30 * MINUTE)),
        ("ساعة إلا ربع", "ساعة إلا ربع", clock(45 * MINUTE)),
        ("نصف ساعة", "نصف ساعة", clock(30 * MINUTE)),
        ("ربع ساعة", "ربع ساعة", clock(15 * MINUTE)),
        ("ثلث ساعة", "ثلث ساعة", clock(20 * MINUTE)),
        ("ثلاثة أرباع الساعة", "ثلاثة أرباع الساعة", clock(45 * MINUTE)),
        ("خمس دقائق", "خمس دقائق", clock(5 * MINUTE)),
        ("خمس عشرة دقيقة", "خمس عشرة دقيقة", clock(15 * MINUTE)),
        ("خمس وأربعون دقيقة", "خمس وأربعون دقيقة", clock(45 * MINUTE)),
        ("ثلاثين يوما", "ثلاثين يوما", clock(30 * DAY)),
        ("أسبوع واحد", "أسبوع واحد", clock(WEEK)),
        ("1.5 ساعة", "1.5 ساعة", clock(90 * MINUTE)),
        ("١٫٥ ساعة", "١٫٥ ساعة", clock(90 * MINUTE)),
        ("3 أشهر", "3 أشهر", Length::calendar(300)),
        ("سنتين", "سنتين", Length::calendar(2_400)),
    ] {
        assert_eq!(read(text), [(written, length)], "{text}");
    }
}

#[test]
fn reads_no_arabic_duration_in_a_weekday_a_time_of_day_or_a_lone_word() {
    for text in [
        "يوم الجمعة",
        "الساعة 3",
        "ساعة الحائط",
        "انتظر دقيقة",
        "معلومات دقيقة",
        "عام 2026",
        "للمرة الثانية",
    ] {
        assert_eq!(read(text), [], "{text}");
    }
}

#[test]
fn grounds_an_arabic_duration_however_the_corpus_writes_it() {
    let reply = "استغرقت المهمة ساعة ونصف، ثم ساعتين للمراجعة.";
    assert_eq!(
        ungrounded_durations(reply, r#"{"duration_ms": 5400000}"#, ""),
        ["ساعتين"]
    );
    assert_eq!(
        ungrounded_durations("The job ran 90 minutes.", "استغرق ساعة ونصف", ""),
        Vec::<String>::new()
    );
}

const VIDEO_REVIEW: &str =
    "A silent 6-minute screen recording.\n350 seconds is too long: cut to 60–90 seconds max.";

fn ungrounded(reply: &str, corpus: &str) -> Vec<String> {
    ungrounded_durations(reply, corpus, "")
}

#[test]
fn names_a_length_nothing_in_the_turn_printed() {
    assert_eq!(
        ungrounded(
            "Here's a 2-minute walkthrough of how it works:",
            VIDEO_REVIEW
        ),
        ["2-minute"]
    );
}

#[test]
fn passes_a_printed_length_rounded_at_the_unit_it_is_written_in() {
    assert_eq!(
        ungrounded("Here's a 6-minute walkthrough.", "duration: 350 seconds"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("It ran 2 hours.", "it ran 100 minutes"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("It ran 2 hours.", "it ran 89 minutes"),
        ["2 hours"]
    );
}

#[test]
fn reads_a_compound_as_its_sum() {
    assert_eq!(
        ungrounded("a 5-minute-50-second walkthrough", "350 seconds"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("5 minutes 40 seconds long", "350 seconds"),
        ["5 minutes 40 seconds"]
    );
}

#[test]
fn holds_a_value_halfway_between_two_to_neither() {
    assert_eq!(ungrounded("a 2-minute clip", "90 seconds"), ["2-minute"]);
    assert_eq!(ungrounded("a 1-minute clip", "90 seconds"), ["1-minute"]);
}

#[test]
fn holds_a_decimal_to_the_place_it_is_written_to() {
    assert_eq!(ungrounded("1.5 hours", "90 minutes"), Vec::<String>::new());
    assert_eq!(ungrounded("1.5 hours", "92 minutes"), Vec::<String>::new());
    assert_eq!(ungrounded("1.5 hours", "80 minutes"), Vec::<String>::new());
    assert_eq!(ungrounded("1.5 hours", "70 minutes"), ["1.5 hours"]);
    assert_eq!(ungrounded("0.5 hours", "50 seconds"), ["0.5 hours"]);
    assert_eq!(ungrounded("1.2 hours", "80 minutes"), ["1.2 hours"]);
    assert_eq!(
        ungrounded("an hour and a half", "100 minutes"),
        Vec::<String>::new()
    );
    let found = durations("1.5 hours, 1½ hours, an hour and a half, 1.2 hours, 2.25 hours");
    let places: Vec<u64> = found.iter().map(|d| d.place.milliseconds).collect();
    assert_eq!(
        places,
        [
            30 * MINUTE,
            30 * MINUTE,
            30 * MINUTE,
            6 * MINUTE,
            15 * MINUTE
        ]
    );
}

#[test]
fn allows_one_unit_either_way_when_the_length_is_marked_approximate() {
    assert_eq!(
        ungrounded("about ~5 minutes", "350 seconds"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded("≈ 2 hours", "151 minutes"), Vec::<String>::new());
    assert_eq!(ungrounded("5 minutes", "350 seconds"), ["5 minutes"]);
    assert!(durations("~5 minutes")[0].approximate);
}

#[test]
fn passes_a_range_a_printed_length_lies_in() {
    assert_eq!(
        ungrounded("each takes 10–30 min", "the longest ran 25 minutes"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("each takes 10–30 min", "it ran 45 minutes"),
        ["10–30 min"]
    );
    assert_eq!(
        ungrounded("it took 60-90 seconds", VIDEO_REVIEW),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("it took 75 seconds", "cut to 60–90 seconds"),
        ["75 seconds"]
    );
    let range = &durations("2 - 3 hours")[0];
    assert_eq!(
        (range.text, range.from, range.length),
        ("2 - 3 hours", Some(clock(2 * HOUR)), clock(3 * HOUR))
    );
}

#[test]
fn reads_a_range_written_high_to_low_as_the_span_between_its_numbers() {
    let found = durations("step 3 - 2 minutes");
    assert_eq!(
        found
            .iter()
            .map(|d| (d.text, d.from, d.length))
            .collect::<Vec<_>>(),
        [("3 - 2 minutes", Some(clock(2 * MINUTE)), clock(3 * MINUTE))]
    );
    assert_eq!(
        ungrounded("30-10 minutes", "20 minutes"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded("30-10 minutes", "45 minutes"), ["30-10 minutes"]);
    assert_eq!(
        ungrounded("step 3 takes 2 minutes", "step 3 - 2 minutes"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_no_range_whose_low_is_part_of_a_word() {
    for (text, written) in [("Q1-3 hours", "3 hours"), ("v1.2-3 hours", "3 hours")] {
        let found = durations(text);
        assert_eq!(
            found.iter().map(|d| (d.text, d.from)).collect::<Vec<_>>(),
            [(written, None)],
            "{text}"
        );
    }
}

#[test]
fn reads_a_unit_glued_to_a_printed_figure_and_names_in_other_languages() {
    assert_eq!(
        ungrounded("Book a 30-minute call", "cal.com booking: 30min meeting"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("une vidéo de 6 minutes", "Durée : 350 secondes"),
        Vec::<String>::new()
    );
    for (text, length) in [
        ("5 Minuten", clock(5 * MINUTE)),
        ("2 Std.", clock(2 * HOUR)),
        ("3 uur", clock(3 * HOUR)),
        ("2 godziny", clock(2 * HOUR)),
        ("3 timmar", clock(3 * HOUR)),
        ("10 dakika", clock(10 * MINUTE)),
        ("5 menit", clock(5 * MINUTE)),
        ("5 phút", clock(5 * MINUTE)),
        ("2 giờ", clock(2 * HOUR)),
        ("5 минут", clock(5 * MINUTE)),
        ("2 часа", clock(2 * HOUR)),
        ("2 שעות", clock(2 * HOUR)),
        ("10 دقیقه", clock(10 * MINUTE)),
        ("5 मिनट", clock(5 * MINUTE)),
        ("3時間", clock(3 * HOUR)),
        ("2 시간", clock(2 * HOUR)),
        ("5分钟", clock(5 * MINUTE)),
        ("2 ชั่วโมง", clock(2 * HOUR)),
    ] {
        assert_eq!(read(text), [(text, length)], "{text}");
    }
    assert_eq!(read("3時間かかった"), [("3時間", clock(3 * HOUR))]);
    assert_eq!(ungrounded("5 минут", "330 seconds"), ["5 минут"]);
    assert_eq!(
        ungrounded("2 Std. 30 min", "150 minutes"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_no_unit_inside_a_word() {
    assert_eq!(read("3 minor fixes, hourly, 5 mint, 2 минуточки"), []);
}

#[test]
fn never_holds_a_number_before_a_one_letter_symbol() {
    let stated: Vec<&str> = durations("3 minor fixes, the 90s, 24h, 3d, 15h, 24 h, à 15 h")
        .into_iter()
        .filter(|d| d.stated)
        .map(|d| d.text)
        .collect();
    assert_eq!(stated, Vec::<&str>::new());
    assert_eq!(ungrounded("3 hours", "took 3h"), Vec::<String>::new());
    assert_eq!(
        ungrounded("the 90s, 24h, 1h30", "90 minutes"),
        Vec::<String>::new()
    );
}

#[test]
fn ignores_a_link_s_digits() {
    assert_eq!(
        ungrounded("see https://cal.com/someone/45min", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("see https://example.com/a?t=45 minutes", ""),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_corpus_comma_before_three_digits_as_a_decimal_too() {
    assert_eq!(
        ungrounded("1.25 seconds", "1,250 secondes"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("1,250 seconds", "1,250 secondes"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_digits_of_any_script_in_any_locale_s_grouping() {
    for (text, length) in [
        ("took ३ घंटे", clock(3 * HOUR)),
        ("took ５分钟", clock(5 * MINUTE)),
        ("took ๓ ชั่วโมง", clock(3 * HOUR)),
        ("took ٣ ساعات", clock(3 * HOUR)),
        ("took 1'250 Sekunden", clock(1_250_000)),
        ("took 1’250 Sekunden", clock(1_250_000)),
        ("took 1 250 secondes", clock(1_250_000)),
        ("took 1\u{202f}250 secondes", clock(1_250_000)),
        ("took 12,50,000 seconds", clock(1_250_000_000)),
        ("took 1.234,5 Sekunden", clock(1_234_500)),
        ("took 1.250 Sekunden", clock(1_250_000)),
    ] {
        assert_eq!(one(text), length, "{text}");
    }
}

#[test]
fn holds_a_reply_to_the_grouped_reading_and_grounds_by_either_in_a_corpus() {
    assert_eq!(
        ungrounded("took 1.250 Sekunden", "1250 seconds"),
        Vec::<String>::new()
    );
    for (reply, corpus) in [
        ("took 1.250 Sekunden", "1.25 seconds"),
        ("took 1,250 seconds", "1.25 seconds"),
        ("took 1,500 minutes", "1.5 minutes"),
        ("Es dauerte 1.500 Minuten", "1.5 minutes"),
        ("Panggilan 1,000 jam", "1 hour"),
        ("took 1.250 Sekunden", "12 seconds"),
    ] {
        assert_eq!(ungrounded(reply, corpus).len(), 1, "{reply} / {corpus}");
    }
    for corpus in ["1.250 Sekunden", "1,250 seconds"] {
        assert_eq!(
            ungrounded("took 1.25 seconds", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
}

#[test]
fn reads_a_unit_beside_a_script_written_without_spaces() {
    assert_eq!(one("合計5分钟"), clock(5 * MINUTE));
    assert_eq!(one("5分钟後に"), clock(5 * MINUTE));
    assert_eq!(one("30 minutes後"), clock(30 * MINUTE));
    assert_eq!(one("時間は36 – 50 Sekunde"), clock(50_000));
}

#[test]
fn reads_finnish_and_ukrainian_units_and_the_cases_a_count_of_one_takes() {
    for (text, length) in [
        ("kesti 5 minuuttia", clock(5 * MINUTE)),
        ("3 tunnin kokous", clock(3 * HOUR)),
        ("30 sekuntia", clock(30_000)),
        ("тривало 5 хвилин", clock(5 * MINUTE)),
        ("за 2 години", clock(2 * HOUR)),
        ("через 1 минуту", clock(MINUTE)),
        ("за 1 годину", clock(HOUR)),
    ] {
        assert_eq!(one(text), length, "{text}");
    }
}

#[test]
fn reads_a_number_before_an_arabic_dual_or_a_hyphen() {
    assert_eq!(one("استغرق 2 ساعتين"), clock(2 * HOUR));
    assert_eq!(one("5-دقائق"), clock(5 * MINUTE));
    assert_eq!(read("x97-ساعتان"), []);
}

#[test]
fn reads_a_spanish_y_as_and_not_as_a_year() {
    assert_eq!(read("entre 1 y 2 horas"), [("2 horas", clock(2 * HOUR))]);
    assert_eq!(one("took 3y"), Length::calendar(3_600));
}

#[test]
fn joins_parts_a_spaced_hyphen_or_a_bare_comma_sets_apart() {
    for text in [
        "2 hours - 30 minutes",
        "2 hours,30 minutes",
        "2 hours-30 minutes",
    ] {
        assert_eq!(read(text), [(text, clock(150 * MINUTE))], "{text}");
    }
}

#[test]
fn reads_parts_joined_by_and_as_one_length_where_the_typescript_check_reads_two() {
    assert_eq!(
        ungrounded("2 minutes and 30 seconds", "150 seconds"),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded("2 minutes and 30 seconds", "2 minutes"),
        ["2 minutes and 30 seconds"]
    );
}

#[test]
fn ends_a_length_at_a_sentence_s_full_stop() {
    assert_eq!(
        read("It ran 2 hours. 30 minutes went to review."),
        [
            ("2 hours", clock(2 * HOUR)),
            ("30 minutes", clock(30 * MINUTE))
        ]
    );
    assert_eq!(
        ungrounded("3.7 seconds", "took 72 hours. 3.7 seconds"),
        Vec::<String>::new()
    );
}

#[test]
fn joins_parts_across_an_abbreviation_s_dot() {
    for (reply, corpus) in [
        ("took 1 hr. 15 min.", "75 minutes"),
        ("it lasted 2 hrs. 30 mins.", "9000 seconds"),
        ("took 1 mins. 30 secs.", "90 seconds"),
        ("took 2 tim. 20 min.", "140 minutes"),
        ("took 2 hr. 5 min", "125 minutes"),
    ] {
        assert_eq!(ungrounded(reply, corpus), Vec::<String>::new(), "{reply}");
    }
    assert_eq!(ungrounded("it lasted 2 hrs", "2 hrs. 30 mins."), ["2 hrs"]);
    assert_eq!(
        ungrounded("3.7 seconds", "took 72 hrs. 3.7 seconds"),
        ["3.7 seconds"]
    );
}

#[test]
fn reads_no_range_across_a_line_break() {
    assert_eq!(
        read("Total: 10\n- 30 min review"),
        [("30 min", clock(30 * MINUTE))]
    );
    assert_eq!(ungrounded("10\n-\n30 min", "20 min"), ["30 min"]);
}

#[test]
fn reads_no_piece_of_a_number_as_a_length() {
    for text in [
        "1/2 hours",
        "x7,59 min",
        "v1,250 seconds",
        "1/12’665 sekundy",
    ] {
        assert!(
            durations(text).iter().all(|d| d.length != clock(2 * HOUR)
                && d.length != clock(59 * MINUTE)
                && d.length != clock(250_000)
                && d.length != clock(665_000)),
            "{text}: {:?}",
            durations(text)
        );
    }
    assert_eq!(one("1/2 hour"), clock(30 * MINUTE));
}

#[test]
fn reads_no_unit_inside_a_longer_word_of_any_script() {
    assert_eq!(read("5 วินาทีly"), []);
    assert_eq!(read("3 ساعتانly"), []);
}

#[test]
fn reads_two_numbers_a_space_sets_apart_as_two() {
    assert_eq!(
        ungrounded("1.75 seconds", "1:52:39 1.75 seconds"),
        Vec::<String>::new()
    );
    assert_eq!(
        read("phase 2 10-30 min"),
        [("10-30 min", clock(30 * MINUTE))]
    );
}

#[test]
fn reads_a_range_of_one_number_as_that_length() {
    assert_eq!(read("18-18 seconds"), [("18 seconds", clock(18_000))]);
    assert_eq!(
        ungrounded("5–73 sec", "took 18-18 seconds"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_part_inside_a_link_as_the_link_s() {
    assert_eq!(
        ungrounded("see https://cal.com/x/30min and 17 seconds", ""),
        ["17 seconds"]
    );
}

#[test]
fn reads_a_range_whose_low_is_a_link_s_as_its_part_alone() {
    assert_eq!(
        ungrounded("see https://cal.com/x/16 – 1880 seconds", "16-1880 seconds"),
        ["1880 seconds"]
    );
}

#[test]
fn holds_a_reply_to_no_time_of_day_written_with_an_hour_symbol() {
    for reply in [
        "Rendez-vous à 15h30 demain",
        "le train part à 8h15",
        "réunion de 14h à 15h30",
        "Abfahrt 15h30",
    ] {
        assert_eq!(ungrounded(reply, ""), Vec::<String>::new(), "{reply}");
    }
    assert_eq!(ungrounded("took 4h26m", ""), ["4h26m"]);
}

#[test]
fn grounds_by_no_time_of_day_written_with_an_hour_symbol() {
    for corpus in ["à 3 h du matin", "um 3h", "de 3h à 4h30", "entre 2h et 3h"] {
        assert_eq!(
            ungrounded("it took 3 hours", corpus),
            ["3 hours"],
            "{corpus}"
        );
    }
    assert_eq!(
        ungrounded("took 15 hours and 30 minutes", "à 15h30"),
        ["15 hours and 30 minutes"]
    );
    for corpus in ["took 3h", "pendant 3 h"] {
        assert_eq!(
            ungrounded("it took 3 hours", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
}

#[test]
fn grounds_by_a_clock_reading_only_when_it_is_labelled_a_length() {
    for (reply, corpus) in [
        ("the deploy took 10 hours", "at 10:05 the deploy started"),
        ("took 10 minutes", "at 10:05 the deploy started"),
        ("took 10 hours", "[10:05:12] INFO deploy started"),
        ("took 14 hours", "Meeting starts 14:00:00"),
        ("took 14 hours", "started 14:00:00 UTC"),
        ("took 9 hours", "logged 09:12:44"),
        ("took 90 minutes", "Duration: 2:00:00 at 1:30:00"),
    ] {
        assert_eq!(ungrounded(reply, corpus).len(), 1, "{reply} / {corpus}");
    }
    for corpus in [
        "Duration: 1:30:00",
        "it took 1:30:00",
        "took about 1:30:00",
        r#"{"video_duration": "1:30:00"}"#,
        "elapsedTime=1:30:00",
        "90:00",
    ] {
        assert_eq!(
            ungrounded("took 90 minutes", corpus),
            Vec::<String>::new(),
            "{corpus}"
        );
    }
}

#[test]
fn holds_each_part_of_a_compound_in_a_corpus_to_the_compound() {
    for (reply, corpus) in [
        ("the meeting is 25 minutes", "took 25 minutes, 59 seconds"),
        ("25นาที", "took 25 minutes, 59 seconds"),
        ("the call is 1 hour", "Duration: 1 hour, 30 minutes"),
        ("the call is 30 seconds", "2 hours, 15 minutes, 30 seconds"),
        ("15 segundo", "took 30 minutes, 15 seconds"),
        ("took 30 minutes", "1 hour and 30 minutes"),
        ("took 3 hours", "took 3 hours and 58 minutes"),
    ] {
        assert_eq!(ungrounded(reply, corpus).len(), 1, "{reply} / {corpus}");
    }
    assert_eq!(
        ungrounded("took 26 minutes", "took 25 minutes, 59 seconds"),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_unit_with_its_inflection_or_short_vowels() {
    for (text, written, length) in [
        ("یک جلسه ۳ ساعته داشتیم", "۳ ساعت", clock(3 * HOUR)),
        ("جلسه ۲ ساعتی طول کشید", "۲ ساعت", clock(2 * HOUR)),
        ("استغرقت 5 دقائقَ", "5 دقائق", clock(5 * MINUTE)),
        ("استغرقت ١٠ دقيقةً", "١٠ دقيقة", clock(10 * MINUTE)),
        ("वीडियो 5 मिनटों में", "5 मिनट", clock(5 * MINUTE)),
        ("10 सेकंडों में", "10 सेकंड", clock(10_000)),
    ] {
        assert_eq!(read(text), [(written, length)], "{text}");
        assert_eq!(ungrounded(text, ""), [written], "{text}");
    }
}

#[test]
fn reads_a_fraction_of_a_unit() {
    for (text, length) in [
        ("took 1/2 hours", clock(30 * MINUTE)),
        ("took 1 1/2 hours", clock(90 * MINUTE)),
        ("took 2 3/4 hours", clock(165 * MINUTE)),
        ("took ½ hour", clock(30 * MINUTE)),
        ("took ¾ hours", clock(45 * MINUTE)),
    ] {
        assert_eq!(one(text), length, "{text}");
    }
    assert_eq!(
        ungrounded("took 1 1/2 hours", "90 minutes"),
        Vec::<String>::new()
    );
    assert_eq!(ungrounded("took 1 1/2 hours", ""), ["1 1/2 hours"]);
    assert_eq!(ungrounded("took 1/2 hours", "2 hours"), ["1/2 hours"]);
}

#[test]
fn holds_a_reply_to_no_length_in_days_or_longer_units_alone() {
    for (reply, corpus) in [
        ("Over the last 30 days revenue grew", ""),
        ("We'll follow up in 2 weeks", ""),
        ("a 14-day free trial", ""),
        ("a 30-day window", "from 2026-10-01 to 2026-10-31"),
        ("took 1 year", "365 days"),
        ("took 2 months", "60 days"),
    ] {
        assert_eq!(ungrounded(reply, corpus), Vec::<String>::new(), "{reply}");
    }
    assert_eq!(ungrounded("took 48 hours", "2 days"), Vec::<String>::new());
    assert_eq!(
        ungrounded("took 2 days and 3 hours", "3 hours"),
        ["2 days and 3 hours"]
    );
}
