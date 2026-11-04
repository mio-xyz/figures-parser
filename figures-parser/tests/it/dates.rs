use figures_parser::{Date, misdated_weekdays, rows_outside_window};

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

#[test]
fn flags_an_arabic_weekday_on_the_wrong_day() {
    assert_eq!(
        check("الاجتماع يوم الجمعة 26 سبتمبر 2026", TODAY),
        [r#""يوم الجمعة 26 سبتمبر 2026" (2026-09-26 is a Saturday)"#]
    );
    assert_eq!(
        check("الخميس 25 - والجمعة 26 أيلول", TODAY),
        [r#""والجمعة 26 أيلول" (2026-09-26 is a Saturday)"#]
    );
}

#[test]
fn passes_a_correct_arabic_pair_in_every_written_form() {
    for reply in [
        "الجمعة 25 سبتمبر 2026",
        "الجمعة، 25 سبتمبر",
        "يوم الجمعة 25 أيلول 2026",
        "الجمعة ٢٥ سبتمبر ٢٠٢٦",
        "الجمعة، ٢٥ سبتمبر ٢٠٢٦",
        "الجمعة 25/09/2026",
        "الجمعة ٢٥\u{200f}/٩\u{200f}/٢٠٢٦",
        "الجمعة الموافق 25 سبتمبر 2026م",
        "الجمعة 2026/09/25",
        "الجمعه 26 جوان 2026",
        "الجمعة 2 جانفي 2026",
        "الجمعة 25 كانون الأول 2026",
        "الجمعة 7 غشت 2026",
        "الأحد 4 أكتوبر",
        "الاحد 4 تشرين الاول",
    ] {
        assert_eq!(check(reply, TODAY), Vec::<String>::new(), "{reply}");
    }
}

#[test]
fn reads_every_arabic_date_form_when_the_weekday_is_wrong() {
    for reply in [
        "الخميس ٢٥ سبتمبر ٢٠٢٦",
        "الخميس، 25/09/2026",
        "الخميس ٢٥\u{200f}/٩\u{200f}/٢٠٢٦",
        "الخميس الموافق 25 من سبتمبر عام 2026م",
        "الخميس 2026-09-25",
        "الخميس 25 أيلول/سبتمبر 2026",
        "الخميس، 25 سبتمبر",
        "يوم الخميس 25 أيلول 2026",
        "الخميس 26 جوان 2026",
        "الخميس 2 جانفي 2026",
        "الخميس 25 كانون الأول 2026",
        "الخميس 7 غشت 2026",
        "السبت 4 أكتوبر",
        "السبت 4 تشرين الاول",
        "الاتنين 25 سبتمبر",
        "الأربعا 25 سبتمبر",
    ] {
        assert_eq!(check(reply, TODAY).len(), 1, "{reply}");
    }
}

#[test]
fn leaves_an_arabic_weekday_beside_a_hijri_date_or_no_date_alone() {
    for reply in [
        "الجمعة 14 ربيع الأول 1448هـ الموافق 26 سبتمبر 2026م",
        "الجمعة 26 موظفا",
        "الجمعة القادمة",
        "أحد 26 سبتمبر",
    ] {
        assert_eq!(check(reply, TODAY), Vec::<String>::new(), "{reply}");
    }
}

const OCT_30: &str = "2026-10-30";

// 2026-11-04 is a Wednesday; 2025-11-04 was the Tuesday.
#[test]
fn reads_a_bare_day_in_the_window_around_today_and_flags_a_weekday_no_date_there_falls_on() {
    for (reply, finding) in [
        (
            "tu as un RDV mardi 4 à 17h",
            r#""mardi 4" (2026-11-04 is a Wednesday)"#,
        ),
        (
            "Call on Tue 4 at 5pm",
            r#""Tue 4" (2026-11-04 is a Wednesday)"#,
        ),
        (
            "Tuesday 4th",
            r#""Tuesday 4th" (2026-11-04 is a Wednesday)"#,
        ),
        ("el martes 4", r#""martes 4" (2026-11-04 is a Wednesday)"#),
        (
            "Dienstag 4 um 17 Uhr",
            r#""Dienstag 4" (2026-11-04 is a Wednesday)"#,
        ),
        ("mardi 4 de 9h", r#""mardi 4" (2026-11-04 is a Wednesday)"#),
    ] {
        assert_eq!(check(reply, OCT_30), [finding], "{reply}");
    }
}

#[test]
fn passes_a_weekday_some_date_in_the_window_falls_on() {
    for reply in [
        "tu as un RDV mercredi 4 à 17h",
        "lundi 2, mardi 3 et mercredi 4",
        "Wednesday 4th, el miércoles 4",
        // 2026-10-10 is a Saturday, 2026-11-10 the Tuesday.
        "Tuesday 10",
    ] {
        assert_eq!(check(reply, OCT_30), Vec::<String>::new(), "{reply}");
    }
}

#[test]
fn leans_a_month_ahead_and_three_weeks_back() {
    // 2026-10-19, 26 days ahead, is the Monday.
    assert_eq!(
        check("seulement le lun 19, à 20h", "2026-09-23"),
        Vec::<String>::new()
    );
    // 2026-09-18, 5 days back, is a Friday, and no 18th ahead is a Thursday.
    assert_eq!(
        check("Dernier message : jeudi 18.", "2026-09-23"),
        [r#""jeudi 18" (2026-09-18 is a Friday)"#]
    );
}

// A "meetings of the week" list for Oct 20-22 2026, weekdays from 2025.
#[test]
fn flags_each_bare_day_of_a_list_written_by_last_years_calendar() {
    assert_eq!(
        check("Lundi 20 / Mardi 21 / Mercredi 22", "2026-10-15"),
        [
            r#""Lundi 20" (2026-10-20 is a Tuesday)"#,
            r#""Mardi 21" (2026-10-21 is a Wednesday)"#,
            r#""Mercredi 22" (2026-10-22 is a Thursday)"#,
        ]
    );
}

#[test]
fn names_the_nearest_date_the_later_of_two_as_near() {
    // Nov 5 and Dec 5 are both 15 days from Nov 20; Nov 5 is the Thursday.
    assert_eq!(
        check("Mon 5", "2026-11-20"),
        [r#""Mon 5" (2026-12-05 is a Saturday)"#]
    );
    assert_eq!(check("Thu 5", "2026-11-20"), Vec::<String>::new());
}

#[test]
fn leaves_a_number_that_is_a_count_an_hour_a_range_or_part_of_a_month_date_alone() {
    for reply in [
        "Monday 3 meetings",
        "Tue 4 pm",
        "Mardi 4h",
        "Monday 9:30",
        "Mon 3-5",
        "has sat 2+ weeks",
        "Mar 11, 2026",
        "Mittwoch 4. November",
        "Tuesday 10 meetings",
        "Tue 4 tomorrow",
        "martes 4 a las 17h",
        // "9h à 12h" outgrows "de 9h", so no time opens the rest of the line.
        "mardi 4 de 9h à 12h",
    ] {
        assert_eq!(check(reply, OCT_30), Vec::<String>::new(), "{reply}");
    }
}

#[test]
fn checks_a_bare_day_the_rest_of_its_line_opens_on_a_time_in_any_language() {
    for reply in [
        "火曜日 4 15:00から",
        "星期二 4 下午3点",
        "Tue 4 tomorrow 15",
        "Tue 4 05.29 +02:00",
        "Tue 4 noon",
        "Tue 4 for an hour",
        "Tue 4 in 2 hours",
        "mardi 4 pendant une heure",
        "Dienstag 4 für eine Stunde",
    ] {
        assert_eq!(
            check(reply, OCT_30).len(),
            1,
            "{reply} should be read as Tuesday the 4th"
        );
    }
}

#[test]
fn leaves_a_bare_day_whose_line_opens_on_no_time_alone() {
    // A length of days sets no hour, and a slash date with a month of 0
    // still removes the time it overlaps.
    for reply in ["Tue 4 3 days ago", "ter 4 15.00-0500 15", "Tue 4 tomorrow"] {
        assert_eq!(check(reply, OCT_30), Vec::<String>::new(), "{reply}");
    }
}

#[test]
fn names_a_pair_with_the_time_written_beside_it() {
    assert_eq!(
        check("Fri 7 Nov at 5 pm CET works.", OCT_30),
        [r#""Fri 7 Nov at 5 pm CET" (2026-11-07 is a Saturday)"#]
    );
    assert_eq!(
        check("at 5pm, Fri 7 Nov works.", OCT_30),
        [r#""at 5pm, Fri 7 Nov" (2026-11-07 is a Saturday)"#]
    );
    assert_eq!(
        check("le vendredi 7 novembre à 17h", OCT_30),
        [r#""vendredi 7 novembre à 17h" (2026-11-07 is a Saturday)"#]
    );
}

// The TypeScript check names "mardi 7 nov 15" here, reading the count as an
// hour.
#[test]
fn names_a_pair_without_a_bare_number_after_it() {
    assert_eq!(
        check("mardi 7 nov 15 personnes", OCT_30),
        [r#""mardi 7 nov" (2026-11-07 is a Saturday)"#]
    );
}

// The TypeScript check loses both: chrono reads "+11" of "+11mins" as a zone
// offset whose result outgrows the pair, and joins the second pair into a
// range with the time on the line before.
#[test]
fn checks_a_pair_a_neighbouring_time_would_swallow() {
    assert_eq!(
        check("Fri 05 +11mins after", "2026-02-28"),
        [r#""Fri 05" (2026-03-05 is a Thursday)"#]
    );
    assert_eq!(
        check("2026-01-24T19:31\n- Sunday 21 Nov", "2026-12-26"),
        [r#""Sunday 21 Nov" (2026-11-21 is a Saturday)"#]
    );
}

#[test]
fn checks_a_pair_after_a_count_on_the_line_before() {
    assert_eq!(
        check("Total: 15\n- Montag 20", "2026-10-15"),
        [r#""Montag 20" (2026-10-20 is a Tuesday)"#]
    );
}

// chrono keeps the year 3 here.
#[test]
fn reads_a_two_digit_year_before_a_chinese_date_in_this_century() {
    assert_eq!(
        check("周五 03 11月5日", "2026-05-05"),
        [r#""周五 03 11月5日" (2003-11-05 is a Wednesday)"#]
    );
}

#[test]
fn checks_a_weekday_against_the_day_a_casual_word_beside_it_names() {
    let today = "2026-11-04";
    for (reply, finding) in [
        (
            "See you Friday, tomorrow.",
            r#""Friday, tomorrow" (2026-11-05 is a Thursday)"#,
        ),
        (
            "vendredi demain",
            r#""vendredi demain" (2026-11-05 is a Thursday)"#,
        ),
        (
            "пятница, завтра",
            r#""пятница, завтра" (2026-11-05 is a Thursday)"#,
        ),
        (
            "今日(金)は会議です",
            r#""今日(金)" (2026-11-04 is a Wednesday)"#,
        ),
        ("星期五今天", r#""星期五今天" (2026-11-04 is a Wednesday)"#),
        (
            "Friday, last night",
            r#""Friday, last night" (2026-11-03 is a Tuesday)"#,
        ),
    ] {
        assert_eq!(check(reply, today), [finding], "{reply}");
    }
    assert_eq!(check("Wednesday, today", today), Vec::<String>::new());
}

// The TypeScript check flags the first two, reading each part of the day as
// a date, and misses the third, whose " morgen" chrono reads as a morning.
#[test]
fn reads_a_part_of_the_day_after_a_weekday_as_no_date() {
    let today = "2026-11-04";
    assert_eq!(check("星期五上午", today), Vec::<String>::new());
    assert_eq!(check("Freitag morgen um 9", today), Vec::<String>::new());
    assert_eq!(
        check("Freitag, morgen", today),
        [r#""Freitag, morgen" (2026-11-05 is a Thursday)"#]
    );
}

// chrono merges the weekday with the hour it reads in "24" and drops both.
#[test]
fn leaves_a_weekday_beside_24_alone() {
    assert_eq!(check("Tue 24", OCT_30), Vec::<String>::new());
    assert_eq!(check("mer 24.", "2026-07-26"), Vec::<String>::new());
}

// "mer" is also Italian, which misses the French month.
#[test]
fn checks_a_date_with_its_month_once_against_that_month() {
    assert_eq!(
        check("- **Mer 15 oct** : soirée libre", "2026-09-23"),
        [r#""Mer 15 oct" (2026-10-15 is a Thursday)"#]
    );
}

fn window(reply: &str) -> Vec<String> {
    rows_outside_window(reply, day(OCT_30))
}

const WEEK_TABLE: &str =
    "Here's your LinkedIn activity from the past week (Oct 23–30), 6 original posts:

**Original posts:**

| Date | Post | Impressions |
|------|------|------------|
| Oct 29 | \"Sixteen plays, one page\" | 39 |
| Oct 27 | \"The end of the weekly ad audit?\" | 427 |
| Oct 22 | \"Sixteen agents, four pods\" | 15 |
| Oct 20 | \"Paid ads through Slack\" | 540 |";

#[test]
fn flags_the_rows_of_a_table_whose_date_falls_before_the_window_its_heading_names() {
    assert_eq!(
        window(WEEK_TABLE),
        [
            r#""Oct 22" under "Oct 23-30""#,
            r#""Oct 20" under "Oct 23-30""#
        ]
    );
}

#[test]
fn passes_a_table_whose_rows_all_fall_inside_the_window() {
    let inside: Vec<&str> = WEEK_TABLE
        .lines()
        .filter(|l| !(l.starts_with("| Oct 22") || l.starts_with("| Oct 20")))
        .collect();
    assert_eq!(window(&inside.join("\n")), Vec::<String>::new());
}

#[test]
fn reads_list_items_by_the_date_they_lead_with_not_a_date_inside_them() {
    let list = "*Weekly digest: Oct 19 to Oct 26*
• Oct 21: support added for Slack Connect, first asked for on Oct 14
• Oct 14: a release with reliability fixes";
    assert_eq!(window(list), [r#""Oct 14" under "Oct 19 to Oct 26""#]);
}

#[test]
fn lets_prose_end_what_a_heading_covers() {
    let prose = "Weekly usage digest (Oct 12-18): 1,754 turns from 121 users.
Since then the team shipped two fixes.
- Oct 20: the GTM evidence review";
    assert_eq!(window(prose), Vec::<String>::new());
}

#[test]
fn reads_the_window_a_later_heading_names_for_the_rows_under_it() {
    let two = "*This week (Oct 26-30)*
- Oct 28: launch
*Previous week (Oct 19-25)*
- Oct 22: draft";
    assert_eq!(window(two), Vec::<String>::new());
}

#[test]
fn reads_a_window_and_its_rows_in_another_language() {
    let french = "*Digest concurrents (19-26 oct.)*
- 14 oct. : nouvelle version
- 21 oct. : critique sur les crédits";
    assert_eq!(window(french), [r#""14 oct." under "19-26 oct.""#]);
    let german = "Woche vom 19. bis 26. Oktober\n- 18. Oktober: a\n- 22. Oktober: b";
    assert_eq!(
        window(german),
        [r#""18. Oktober" under "19. bis 26. Oktober""#]
    );
}

#[test]
fn reads_a_window_across_months_and_years() {
    assert_eq!(
        window("Week of Oct 28 - Nov 3:\n- Nov 4: late\n- Oct 27: early\n- Oct 30: in"),
        [
            r#""Nov 4" under "Oct 28 - Nov 3""#,
            r#""Oct 27" under "Oct 28 - Nov 3""#
        ]
    );
    assert_eq!(
        rows_outside_window(
            "## Dec 28 to Jan 3\n1. Jan 5: x\n2) Dec 27: y\n3. Dec 30: z",
            day("2026-01-02")
        ),
        [
            r#""Jan 5" under "Dec 28 to Jan 3""#,
            r#""Dec 27" under "Dec 28 to Jan 3""#
        ]
    );
}

#[test]
fn leaves_a_row_dated_by_a_range_and_a_window_running_backwards_alone() {
    assert_eq!(window("Oct 23-30\n- Oct 19-20: d"), Vec::<String>::new());
    assert_eq!(window("Oct 30-23\n- Oct 1: a"), Vec::<String>::new());
}

#[test]
fn reads_a_date_with_a_range_of_hours_as_a_one_day_window() {
    let meetings = "Your meetings on Nov 4, 9:00-17:00:\n| Date | Event |\n|---|---|\n| Nov 4 | standup |\n| Nov 6 | review |";
    assert_eq!(
        rows_outside_window(meetings, day("2026-11-05")),
        [r#""Nov 6" under "Nov 4, 9:00-17:00""#]
    );
    for (heading, row) in [
        ("Le 4 nov., 9h-17h :", "- 3 nov. : x"),
        ("Am 4. November, 9-17 Uhr:", "- 3. November: x"),
        ("11月4日 9:00-17:00", "- 11月3日: x"),
        ("ngày 4 tháng 11, 9:00 - 17:00", "- ngày 3 tháng 11: x"),
        ("2026-11-04 9:00-17:00", "- 2026-11-03: x"),
    ] {
        assert_eq!(
            rows_outside_window(&format!("{heading}\n{row}"), day("2026-11-05")).len(),
            1,
            "{heading}"
        );
    }
    // A range of hours that ends before it starts ends on the next day.
    assert_eq!(
        rows_outside_window(
            "Nov 4, 22:00-02:00:\n- Nov 5: late\n- Nov 6: later",
            day("2026-11-05")
        ),
        [r#""Nov 6" under "Nov 4, 22:00-02:00""#]
    );
}

#[test]
fn names_a_row_with_the_time_written_beside_its_date() {
    assert_eq!(
        window("Posts from Oct 23 to Oct 30:\n- Oct 20 at 5pm: launch\n- 5pm Oct 21: post"),
        [
            r#""Oct 20 at 5pm" under "Oct 23 to Oct 30""#,
            r#""5pm Oct 21" under "Oct 23 to Oct 30""#
        ]
    );
    assert_eq!(
        window("Week Oct 23-30:\n| 2026-10-20 at 5pm CET | x |"),
        [r#""2026-10-20 at 5pm CET" under "Oct 23-30""#]
    );
    assert_eq!(
        window("Posts from Oct 19 9am to Oct 26 5pm:\n- Oct 20: launch\n- Oct 28: post"),
        [r#""Oct 28" under "Oct 19 9am to Oct 26 5pm""#]
    );
}

#[test]
fn leaves_a_row_dated_by_a_range_of_hours_and_a_window_in_words_alone() {
    assert_eq!(
        window("Posts from Oct 23-30:\n- Oct 20, 9:00-17:00: workshop"),
        Vec::<String>::new()
    );
    for heading in ["Posts this week:", "Posts from the last 7 days:"] {
        assert_eq!(
            window(&format!("{heading}\n- Oct 1: launch")),
            Vec::<String>::new()
        );
    }
}

// The TypeScript check reads "-14" as a zone offset after "29 octobre", so it
// opens no window here, and names the second window with the count.
#[test]
fn reads_two_dates_a_dash_joins_as_a_window_never_as_a_date_and_a_zone() {
    assert_eq!(
        window("Week of 29 octobre–14 déc.\n- 20 oct. : lancement\n- 2 nov. : point"),
        [r#""20 oct." under "29 octobre-14 déc.""#]
    );
    assert_eq!(
        rows_outside_window(
            "Week of 1—22 nov — 6 posts\n- 2026-08-24: x",
            day("2026-11-05")
        ),
        [r#""2026-08-24" under "1-22 nov""#]
    );
}

// chrono's Chinese, Vietnamese, Finnish and German refiners join ranges
// before they merge a date with its time: the TypeScript check ends this
// window on October 2 and flags the 10th, and reads the German rows as
// ranges it leaves alone.
#[test]
fn merges_a_date_with_its_time_before_it_joins_a_range_in_every_language() {
    assert_eq!(
        rows_outside_window(
            "10月2号15点30分-10月23日\n- 10月10日: x\n- 11月3号: y",
            day("2026-11-05")
        ),
        [r#""11月3号" under "10月2号15点30分-10月23日""#]
    );
    assert_eq!(
        rows_outside_window(
            "Woche vom 2. bis 8. November:\n- 4. Nov. - 15 Uhr: Termin\n- 12. Nov. - 15 Uhr: Termin",
            day("2026-11-10")
        ),
        [r#""12. Nov. - 15 Uhr" under "2. bis 8. November""#]
    );
}

#[test]
fn checks_nothing_in_a_reply_with_no_table_or_list() {
    assert_eq!(
        window("Between Oct 23 and Oct 30 you posted on Oct 20 and Oct 29."),
        Vec::<String>::new()
    );
}

#[test]
fn checks_a_capitalised_ukrainian_weekday_beside_a_bare_day_number() {
    for reply in ["Середа 7", "СЕРЕДА 7", "середа 7"] {
        assert_eq!(check(reply, "2026-02-27").len(), 1, "{reply}");
    }
    assert_eq!(
        check("el Четвер 09", "2028-01-03"),
        [r#""Четвер 09" (2028-01-09 is a Sunday)"#]
    );
}

#[test]
fn checks_a_bare_day_number_a_signed_length_follows() {
    // chrono merges "24 +11mins" into a time of its own, so "Friday" is
    // left beside a bare 24, and no Friday the 24th falls near today.
    assert_eq!(
        check("Friday 24 +11mins", "2024-02-29"),
        [r#""Friday 24" (2024-02-24 is a Saturday)"#]
    );
    assert_eq!(check("Friday 24 +11 mins", "2024-02-29").len(), 1);
    assert_eq!(check("Friday 24", "2024-02-29"), Vec::<String>::new());
}

// chrono reads "24" after a date as 24:00 and drops the date, so the
// TypeScript check passes every pair; a reader takes 24 as the year where
// no other reading makes the weekday right, and as a count where the
// weekday is right for the date in the year nearest today.
#[test]
fn reads_a_two_digit_number_ending_a_month_name_date_as_its_year() {
    let june = "2025-06-17";
    for reply in [
        "samedi 13 juillet 24",
        "maandag 21 oktober 24",
        "lun 10 de jun de 24",
        "maanantai 21. lokakuuta 24",
        "понедельник, 21 октября 24",
    ] {
        assert_eq!(check(reply, june), Vec::<String>::new(), "{reply}");
    }
    assert_eq!(
        check("lundi 13 juillet 24", june),
        [r#""lundi 13 juillet 24" (2024-07-13 is a Saturday)"#]
    );
    for (reply, today) in [
        ("dimanche 13 juillet 24", june),
        ("Mercredi, 7 mars 24", "2029-02-28"),
        ("jeudi 20 août 24", "2026-08-26"),
        ("неділя,23 грудня 24", "2029-12-22"),
        ("Afspraak Freitag 25/02 24", "2028-02-25"),
        ("Miércoles 03/11 24", "2027-09-10"),
        ("нд,2 січня 24", "2027-12-25"),
    ] {
        assert_eq!(check(reply, today), Vec::<String>::new(), "{reply}");
    }
    // A word after it makes it a count, and the pair goes unread as chrono
    // leaves it.
    assert_eq!(
        check("dimanche 13 juillet 24 personnes", june),
        Vec::<String>::new()
    );
}

#[test]
fn names_a_pair_with_the_time_and_zone_written_after_it() {
    let today = "2025-12-25";
    assert_eq!(
        check("TUESDAY tomorrow 5:30 PM", today),
        [r#""TUESDAY tomorrow 5:30 PM" (2025-12-26 is a Friday)"#]
    );
    assert_eq!(
        check("明日 土曜日 01:50", today),
        [r#""明日 土曜日 01:50" (2025-12-26 is a Friday)"#]
    );
    assert_eq!(
        check("Mittwoch heute um 15 Uhr", today),
        [r#""Mittwoch heute um 15 Uhr" (2025-12-25 is a Thursday)"#]
    );
    assert_eq!(
        check("WEDNESDAY 15 December +02:00", today),
        [r#""WEDNESDAY 15 December +02:00" (2025-12-15 is a Monday)"#]
    );
    // A length after it is no zone: English merges it into the time.
    assert_eq!(
        check("Tuesday, tomorrow 5:30 PM +11mins", "2026-08-22"),
        [r#""Tuesday, tomorrow" (2026-08-23 is a Sunday)"#]
    );
}

// chrono's Japanese parser reads "13/05" month first, rejects the 13 and
// retries one digit in, so the TypeScript check names "3/05(木)" and checks
// March 5.
#[test]
fn reads_a_japanese_slash_date_day_first_when_its_first_number_is_no_month() {
    assert_eq!(
        check("13/05(木)", "2028-09-17"),
        [r#""13/05(木)" (2028-05-13 is a Saturday)"#]
    );
    // 29 December 2024 was a Sunday; the TypeScript check flags "9/12（日)".
    assert_eq!(check("« 29/12（日) »", "2025-01-03"), Vec::<String>::new());
    assert_eq!(
        rows_outside_window(
            "今週 (13/09 から 9月26日):\n* **9月12日**: x",
            day("2028-10-10")
        ),
        [r#""9月12日" under "13/09 から 9月26日""#]
    );
    assert_eq!(
        rows_outside_window(
            "報告 31/10-11月7日, 5 posts\n- 11月3日: in",
            day("2025-11-19")
        ),
        Vec::<String>::new()
    );
}

// chrono's English parser reads "5.1.2025" month first, as May 1.
#[test]
fn reads_a_dotted_date_day_first_in_english() {
    let today = "2025-01-10";
    assert_eq!(check("Sunday 5.1.2025", today), Vec::<String>::new());
    // Right month first, the pair is let be; wrong both ways, it is named
    // by its day-first date.
    assert_eq!(check("Thursday 5.1.2025", today), Vec::<String>::new());
    assert_eq!(
        check("Monday 5.1.2025", today),
        [r#""Monday 5.1.2025" (2025-01-05 is a Sunday)"#]
    );
    assert_eq!(
        rows_outside_window(
            "Viikko — 3.1.2025–9.1.2025\n* 2025-01-03: in",
            day("2025-01-22")
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        rows_outside_window(
            "## Past week 12.5.2027—11 Jun\n- Tue, 1 Jun 2027: in",
            day("2027-05-25")
        ),
        Vec::<String>::new()
    );
}

#[test]
fn opens_a_window_from_or_to_a_casual_day_or_a_length_from_now() {
    let today = day("2026-11-04");
    let rows = "\n- Oct 12: x\n- Nov 8: y";
    assert_eq!(
        rows_outside_window(&format!("Posts from Oct 20 to today:{rows}"), today),
        [
            r#""Oct 12" under "Oct 20 to today""#,
            r#""Nov 8" under "Oct 20 to today""#
        ]
    );
    assert_eq!(
        rows_outside_window(&format!("Posts (yesterday - Nov 10):{rows}"), today),
        [r#""Oct 12" under "yesterday - Nov 10""#]
    );
    assert_eq!(
        rows_outside_window(&format!("Posts: 3 days ago - today{rows}"), today).len(),
        2
    );
    assert_eq!(
        rows_outside_window("Отчёт (20 окт - сегодня):\n- 12 окт: x\n- 8 ноя: y", today).len(),
        2
    );
    assert_eq!(
        rows_outside_window("报告（10月20日 - 今天）：\n- 10月12日: x", today),
        [r#""10月12日" under "10月20日 - 今天""#]
    );
    // A length whose day chrono only implies opens none.
    assert_eq!(
        rows_outside_window(&format!("2 months ago - today{rows}"), today),
        Vec::<String>::new()
    );
}

#[test]
fn reads_a_two_digit_year_dotted_date_in_a_window_and_its_rows() {
    assert_eq!(
        rows_outside_window(
            "Bericht 25.11.25 bis 2025-11-27, 4 posts\n- 17. November: x",
            day("2025-12-26")
        ),
        [r#""17. November" under "25.11.25 bis 2025-11-27, 4""#]
    );
    assert_eq!(
        rows_outside_window(
            "Overzicht 13 tot 26 februari\n- 12.2.25 om 15 uur: x",
            day("2025-03-08")
        ),
        [r#""12.2.25 om 15 uur" under "13 tot 26 februari""#]
    );
    assert_eq!(
        rows_outside_window(
            "Shifts (Jul 25 to Aug 24):\n- 24.7.28 - 6 posts",
            day("2028-08-07")
        ),
        [r#""24.7.28" under "Jul 25 to Aug 24""#]
    );
}

// chrono reads the count after a range as its end's year ("Aug 5, 12" as
// 2012), so the TypeScript check flags the 5th under its own window.
#[test]
fn reads_a_count_after_a_range_as_no_year() {
    for (reply, today) in [
        ("Week 4 Aug - 5 Aug, 12 posts\n- Aug 5: in", "2025-09-01"),
        (
            "Report Mar 21 - Apr 3, 12 posts\n- 27 Mar: in",
            "2025-04-13",
        ),
        (
            "Semana 4 de agosto-5 de agosto, 4 posts\n- 5 agosto: in",
            "2025-09-01",
        ),
        (
            "Zeitraum 2025-08-01–5. Aug, 8 posts\n- 5. August: in",
            "2025-09-01",
        ),
        (
            "Bericht 21/03/2025-3. April, 6 posts\n- 27. März: in",
            "2025-04-13",
        ),
    ] {
        assert_eq!(
            rows_outside_window(reply, day(today)),
            Vec::<String>::new(),
            "{reply}"
        );
    }
    // A year with a time after it is still the year.
    assert_eq!(
        check("Thursday, Aug 5, 27 at 3pm", "2026-07-01"),
        Vec::<String>::new()
    );
}

// chrono's Spanish and Portuguese refiners join no range with "a" or
// "até", and the TypeScript check keeps the earlier heading's window over
// a heading another language reads.
#[test]
fn reads_iberian_range_words_and_lets_a_heading_any_language_reads_end_a_window() {
    let reply = "Resumo (11 de abril - 13 de abril)\n| Date | Post |\n|---|---|\n| 1º abril 2025 | x |\n*Resumo: 28 de março a 4 abr.*\n| Date | Post |\n|---|---|\n| 4 de abril | draft |";
    assert_eq!(
        rows_outside_window(reply, day("2025-05-03")),
        [r#""1º abril 2025" under "11 de abril - 13 de abril""#]
    );
    assert_eq!(
        rows_outside_window(
            "Resumen 28 de marzo a 4 de abril:\n- 27 de marzo: x\n- 2 de abril: y",
            day("2025-05-03")
        ),
        [r#""27 de marzo" under "28 de marzo a 4 de abril""#]
    );
    // A row under a later heading is held to that heading's window alone.
    assert_eq!(
        rows_outside_window(
            "**9-10 set**\n- 9 settembre 2026: x\n13-14 settembre:\n* **15/09/2026** — launch",
            day("2026-09-13")
        ),
        [r#""15/09/2026" under "13-14 settembre""#]
    );
}

// chrono keeps the longer of two overlapping results by UTF-16 width: "6
// Jan" outgrows the "Jan" and the space a month-only parser takes, whatever
// space follows.
#[test]
fn reads_a_one_digit_day_before_any_space_after_its_month() {
    for space in ['\u{202f}', '\u{2009}', '\u{3000}', '\u{2007}', '\u{2002}'] {
        for tail in ["", " ", "@ 10am", "(10am)"] {
            assert_eq!(
                check(&format!("Tue 6 Jan{space}{tail}"), "2027-06-15"),
                [r#""Tue 6 Jan" (2027-01-06 is a Wednesday)"#],
                "{space:?} {tail}"
            );
        }
        assert_eq!(
            check(&format!("пятница 7 сентября{space}в 9 утра"), "2030-01-10"),
            Vec::<String>::new(),
            "{space:?}"
        );
    }
    assert_eq!(
        check("Tue 6 Jan\u{a0}- ok", "2027-06-15"),
        [r#""Tue 6 Jan" (2027-01-06 is a Wednesday)"#]
    );
}

// A word on the next line leaves a bare day ending its own, and a line that
// opens with a weekday, a date and a time opens on that time.
#[test]
fn reads_a_bare_day_before_a_line_that_opens_on_a_dated_time() {
    assert_eq!(
        check("Call: Tue 4\nSun, 2026-11-01 15:00 kickoff", "2026-10-30"),
        [r#""Tue 4" (2026-11-04 is a Wednesday)"#]
    );
    assert_eq!(
        check("Tue 4 Sunday Nov 1, 3pm", "2026-10-30"),
        [r#""Tue 4" (2026-11-04 is a Wednesday)"#]
    );
    assert_eq!(
        check(
            "Fecha: zondag 31\nzo, 2028-04-09 15 attendees",
            "2028-05-18"
        ),
        [r#""zondag 31" (2028-05-31 is a Wednesday)"#]
    );
}

// A month number glued to its mark is no bare day; the day's own mark
// keeps one.
#[test]
fn a_japanese_month_after_a_weekday_is_no_bare_day() {
    assert_eq!(
        check("Spotkanie __土曜日 11月3日__.", "2029-12-23"),
        Vec::<String>::new()
    );
    assert_eq!(
        check("Spotkanie 土曜日 11月3日.", "2029-12-23"),
        Vec::<String>::new()
    );
    assert_eq!(
        check("Spotkanie 土曜日 11日.", "2029-12-23"),
        [r#""土曜日 11" (2029-12-11 is a Tuesday)"#]
    );
}

// A two-digit number before an hour word is the hour, not the year.
#[test]
fn a_two_digit_number_before_an_hour_word_is_no_year() {
    for row in ["16 h", "16 horas", "24 h"] {
        let reply = format!("Actividad: 17 nov 2025 al 16 dic 2025\n- 17 de noviembre {row}: x");
        assert_eq!(
            rows_outside_window(&reply, day("2026-01-03")),
            Vec::<String>::new(),
            "{row}"
        );
    }
}

// A heading's numeric dates say the order its slash dates are written in,
// and its rows' are read in that order; where none says, a date either
// order puts inside the window is let be.
#[test]
fn reads_a_window_and_its_rows_in_the_order_the_heading_writes() {
    let rows = |reply: &str, today: &str| rows_outside_window(reply, day(today));
    assert_eq!(
        rows(
            "## 2/12-15/12\n- 2027-12-02: x\n- 2027-12-20: y",
            "2028-01-02"
        ),
        [r#""2027-12-20" under "2/12-15/12""#]
    );
    for (reply, today) in [
        ("## 02/11-04/11\n- 2026-11-03: x", "2026-11-20"),
        ("Aktivität (6/12-4/1)\n- 2028-12-16: x", "2028-12-20"),
        (
            "**Résumé (7/1/2027-9/1/2027)**\n- 2027-01-07: x",
            "2027-01-20",
        ),
        ("## Week 4/2-11/2\n- 2027-02-04: x", "2027-02-20"),
        (
            "Posts: 30/1/2026-12/2/2026\n- 12/2: x\n- 1/2: y",
            "2026-02-13",
        ),
        ("Активность: 25/8-7/9\n- 07/09: x", "2028-09-10"),
        (
            "Résumé: 28 Nov-04 December\n- 03/12 9am PT: x",
            "2027-12-23",
        ),
    ] {
        assert_eq!(rows(reply, today), Vec::<String>::new(), "{reply}");
    }
    assert_eq!(
        rows(
            "## 03/02/2028 to 9.2.2028\n- 10 February: x\n- 2028-02-05: y",
            "2028-02-29"
        ),
        [r#""10 February" under "03/02/2028 to 9.2.2028""#]
    );
    assert_eq!(
        rows(
            "### Aktivitet 5.2.2027 until 07/02/2027\n- 2027-02-10: x",
            "2027-02-26"
        ),
        [r#""2027-02-10" under "5.2.2027 until 07/02/2027""#]
    );
    assert_eq!(
        rows(
            "Résumé: 28 Nov-04 December\n- 03/15 9am PT: x",
            "2027-12-23"
        ),
        [r#""03/15 9am PT" under "28 Nov-04 December""#]
    );
}

// A heading that dates what follows it, read as no window, ends the window
// before it.
#[test]
fn a_dated_heading_ends_the_window_before_it() {
    for heading in [
        "### Résumé February 13 through 19",
        "### 22 até 23 de dezembro",
        "### 5. mars 2028–11 mars 2028",
        "### 3 t/m 10 juli 2029",
        "### 14. Feb–16. Februar",
        "### 21 грудня–2028-12-23",
    ] {
        let reply = format!("## Oct 1 - Oct 3\n- Oct 2: a\n{heading}\n- 2028-02-19: x");
        assert_eq!(
            rows_outside_window(&reply, day("2028-02-28")),
            Vec::<String>::new(),
            "{heading}"
        );
    }
    assert_eq!(
        rows_outside_window(
            "## Oct 1 - Oct 3\n### Details\n- Oct 7: a",
            day("2028-02-28")
        ),
        [r#""Oct 7" under "Oct 1 - Oct 3""#]
    );
}
