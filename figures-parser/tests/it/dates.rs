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
