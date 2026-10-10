use figures_parser::{Date, misdated_weekdays};

fn day(iso: &str) -> Date {
    iso.parse().unwrap()
}

fn check(reply: &str, today: &str) -> Vec<String> {
    misdated_weekdays(reply, day(today))
}

const TODAY: &str = "2026-10-02";

#[test]
fn flags_a_weekday_taken_from_last_years_calendar_saying_which_day_it_is() {
    assert_eq!(
        check("| Fri 26 Sep | 773 | 870 |", TODAY),
        [r#""Fri 26 Sep" (2026-09-26 is a Saturday)"#]
    );
}

#[test]
fn passes_a_correct_pair_in_every_written_form() {
    let reply = [
        "Fri 25 Sep",
        "Friday, 25 September",
        "Fri Sep 25",
        "Friday, September 25th, 2026",
        "Fri 2026-09-25",
        "Thu 1 Oct",
    ]
    .join("\n");
    assert_eq!(check(&reply, TODAY), Vec::<String>::new());
}

#[test]
fn checks_a_stated_year_as_written_not_the_nearest_one() {
    assert_eq!(
        check("Friday, 26 September 2025", TODAY),
        Vec::<String>::new()
    );
    assert_eq!(
        check("Fri 26 Sep 2026", TODAY),
        [r#""Fri 26 Sep 2026" (2026-09-26 is a Saturday)"#]
    );
    assert_eq!(check("Fri 2026-09-26T14:00", TODAY).len(), 1);
}

#[test]
fn reads_a_date_without_a_year_in_the_year_nearest_today() {
    assert_eq!(check("Thu 31 Dec", "2027-01-03"), Vec::<String>::new());
}

#[test]
fn reads_the_languages_mio_answers_in() {
    let sept = "2026-09-23";
    assert_eq!(
        check("deadline lundi 29/09", sept),
        [r#""lundi 29/09" (2026-09-29 is a Tuesday)"#]
    );
    assert_eq!(
        check("lundi 1er septembre", sept),
        [r#""lundi 1er septembre" (2026-09-01 is a Tuesday)"#]
    );
    assert_eq!(check("Freitag, 26. September", sept).len(), 1);
    assert_eq!(check("viernes 26 de septiembre", sept).len(), 1);
    assert_eq!(
        check(
            "mardi 29/09, Dienstag, 29. September, martes 29 de septiembre",
            sept
        ),
        Vec::<String>::new()
    );
}

#[test]
fn leaves_the_english_readings_a_writer_rarely_means_alone() {
    assert_eq!(
        check("The count for Friday 26 may be incomplete", TODAY),
        Vec::<String>::new()
    );
    assert_eq!(check("Mon 3/4", TODAY), Vec::<String>::new());
    assert_eq!(
        check("Mon 9/29", TODAY),
        [r#""Mon 9/29" (2026-09-29 is a Tuesday)"#]
    );
}

#[test]
fn checks_a_may_date_written_with_a_capital() {
    assert_eq!(
        check("Fri 26 May", TODAY),
        [r#""Fri 26 May" (2026-05-26 is a Tuesday)"#]
    );
}

#[test]
fn skips_a_date_the_calendar_does_not_have() {
    assert_eq!(
        check("Sun 31 Sep (impossible)", TODAY),
        Vec::<String>::new()
    );
}

#[test]
fn reports_each_written_pair_once() {
    assert_eq!(check("Fri 26 Sep, then again Fri 26 Sep", TODAY).len(), 1);
}

#[test]
fn ignores_a_weekday_with_no_date_and_what_only_looks_like_a_date() {
    assert_eq!(
        check("Backlog grew on Mon and Tue.", TODAY),
        Vec::<String>::new()
    );
    assert_eq!(
        check("Monitoring 26 Sep, Sun 31 Sep, Wed 3 Octopus", TODAY),
        Vec::<String>::new()
    );
}

#[test]
fn reads_japanese_with_the_weekday_after_the_date() {
    assert_eq!(
        check("9月26日(金)に公開", TODAY),
        [r#""9月26日(金)" (2026-09-26 is a Saturday)"#]
    );
    assert_eq!(check("9月25日 金曜日", TODAY), Vec::<String>::new());
}

#[test]
fn checks_both_ends_of_a_range() {
    assert_eq!(
        check("Mon 3 Aug to Thu 7 Aug", "2026-10-09"),
        ["\"Thu 7 Aug\" (2026-08-07 is a Friday)"]
    );
}
