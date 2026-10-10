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
        "from 0:45",
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
        ungrounded_durations("30 minutes", "1 hour 30 minutes", ""),
        Vec::<String>::new()
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
fn a_number_glued_to_a_bare_s_grounds_but_is_never_held() {
    let found = durations("back in the 90s, 100s of users, a 30s timeout, 30 s");
    let stated: Vec<(&str, bool)> = found.iter().map(|d| (d.text, d.stated)).collect();
    assert_eq!(
        stated,
        [
            ("90s", false),
            ("100s", false),
            ("30s", false),
            ("30 s", true)
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
