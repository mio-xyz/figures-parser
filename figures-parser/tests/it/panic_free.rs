//! Every public entry point, fed every code point and generated turns, must
//! return without panicking. Slow, so ignored by default: `make
//! check-figures-parser` runs them in the `panic-check` profile, a release
//! build that also panics on overflow.

use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};

use figures_parser::number::scan::{Scratch, tokens};
use figures_parser::utf16::{Utf16Cursor, units};
use figures_parser::{
    Amounts, Date, InternalsCheck, Invariant, ListedFigure, MemoryLayout, PageHolds, SheetRun,
    Turn, TurnRecord, Vocabulary, built_from_corpus, claims_refusal, claims_unbacked_action,
    claims_unwritten_memory, durations, emails, evaluate_invariants, extract_numbers,
    fabricated_urls, ground_message, ground_reply, grounding_tokens, identifiers, internal_names,
    link_findings, memory_claim, memory_grounding, mended_links, misdated_weekdays,
    mispaired_per_unit, mispaired_per_unit_with, numbers, numeric_tokens, placeholder_targets,
    printed_figures, prose_arithmetic, rows_outside_window, scrub_memory_layout, sheet_findings,
    silent_signal, speaker_prefix, stated_figures, states_figure, undotted_message_link,
    ungrounded_durations, ungrounded_emails, ungrounded_figures, ungrounded_identifiers,
    ungrounded_on_page, ungrounded_ratios, ungrounded_ratios_with, ungrounded_urls, unopened_links,
    unprinted_ids, unseen_tokens, url_printed_in, urls, without_links,
};

/// Calls every entry point on the three texts, and returns a sum the
/// optimizer cannot drop.
fn exercise(reply: &str, corpus: &str, prompt: &str) -> usize {
    let mut n = 0;
    let mut scratch = Scratch::default();
    for text in [reply, corpus, prompt] {
        n += tokens(text).count()
            + tokens(text).without_currency().count()
            + tokens(text).against_letters().loose_scales().count()
            + tokens(text).without_scales().count();
        for token in tokens(text) {
            token.for_each_reading(&mut scratch, |r| {
                n += r.stated().map_or(0, |k| k.integer().len()) + usize::from(r.currency);
            });
        }
        n += numbers(text).count()
            + placeholder_targets(text).len()
            + unopened_links(text, &[reply]).len()
            + undotted_message_link(text).map_or(0, |link| link.len())
            + usize::from(built_from_corpus(text, reply, &["github.com"]))
            + grounding_tokens(text).len()
            + extract_numbers(text).len()
            + stated_figures(text).len()
            + printed_figures(text).values.len()
            + numeric_tokens(text, false).len()
            + numeric_tokens(text, true).len()
            + without_links(text).len()
            + emails(text).len()
            + identifiers(text).len()
            + durations(text).len()
            + speaker_prefix(text).map_or(0, str::len)
            + memory_claim(text).map_or(0, str::len)
            + silent_signal(text).map_or(0, |signal| signal.reason.len());
        let found = urls(text);
        let mut cursor = Utf16Cursor::new(text);
        for url in &found {
            n += usize::try_from(cursor.at(url.span.start) + units(url.text)).unwrap();
        }
        // Past the end, inside a character, and going backwards.
        for at in [text.len() + 3, 1, text.len() / 2, 0, 2] {
            n += usize::try_from(Utf16Cursor::new(text).at(at) + cursor.at(at)).unwrap();
        }
    }
    n += misdated_weekdays(reply, today_for(reply)).len()
        + misdated_weekdays(corpus, today_for(corpus)).len()
        + rows_outside_window(reply, today_for(reply)).len()
        + rows_outside_window(corpus, today_for(corpus)).len();
    let amounts = Amounts::of(reply);
    n += amounts.shared_with(corpus).len()
        + amounts.shared_with(prompt).len()
        + usize::from(states_figure(corpus, reply))
        + usize::from(states_figure(reply, prompt))
        + ungrounded_ratios_with(reply, corpus, || prompt).len()
        + mispaired_per_unit_with(prompt, || reply).len();
    n + ungrounded_figures(reply, corpus).len()
        + ungrounded_figures(corpus, reply).len()
        + ungrounded_ratios(reply, corpus, "").len()
        + ungrounded_ratios(corpus, reply, "").len()
        + prose_arithmetic(reply, prompt, corpus).len()
        + prose_arithmetic(corpus, reply, prompt).len()
        + mispaired_per_unit(reply, corpus).len()
        + mispaired_per_unit(corpus, prompt).len()
        + fabricated_urls(reply, corpus, &[]).len()
        + fabricated_urls(corpus, reply, &["github.com"]).len()
        + ungrounded_urls(reply, corpus, prompt, &[]).len()
        + ungrounded_urls(prompt, reply, corpus, &["github.com"]).len()
        + link_findings(reply, corpus, prompt, &["github.com"])
            .all()
            .len()
        + mended_links(reply, corpus).mends.len()
        + mended_links(corpus, reply).text.len()
        + ungrounded_emails(reply, corpus, prompt).len()
        + ungrounded_identifiers(reply, corpus, prompt).len()
        + unprinted_ids(reply, corpus).len()
        + unprinted_ids(corpus, reply).len()
        + usize::from(url_printed_in(reply, corpus))
        + usize::from(url_printed_in(prompt, reply))
        + found_urls_printed(reply, corpus)
        + memory_grounding(reply, &[corpus, prompt]).grounded_lines
        + found_urls_built(reply, corpus)
        + unseen_tokens(reply, corpus, prompt).len()
        + ungrounded_durations(reply, corpus, prompt).len()
        + ungrounded_durations(corpus, reply, prompt).len()
        + ungrounded_on_page(reply, corpus, prompt, false).len()
        + usize::from(PageHolds::default().hold(reply, &[corpus, prompt]))
        + turn_checks(reply, corpus, prompt)
}

/// The checks that read a whole turn, each text taking every part in turn:
/// the vocabulary and the layout are cut from the texts themselves, so a
/// name or a root can be any string.
fn turn_checks(reply: &str, corpus: &str, prompt: &str) -> usize {
    let words: Vec<&str> = prompt.split_whitespace().take(8).collect();
    let (names, files) = words.split_at(words.len() / 2);
    let vocabulary = Vocabulary {
        names,
        files,
        roots: &["/memory/", "/sdk/"],
        fixed: files,
    };
    let root = words.first().copied().unwrap_or("/memory/");
    let layout = MemoryLayout {
        root,
        tool_prefix: words.get(1).copied().unwrap_or("memory_"),
        folders: &[("integrations/", "the integration notes")],
        files: &[("user.md", "your notes"), ("", "")],
        root_phrase: "memory",
        other_phrase: "notes",
        tool_phrase: "my memory",
    };
    let check = InternalsCheck::new(corpus, &vocabulary);
    let listed = [
        ListedFigure {
            figure: prompt,
            name: reply,
        },
        ListedFigure {
            figure: "19,851",
            name: corpus,
        },
    ];
    let runs = [
        SheetRun {
            stdout: corpus,
            lists_its_rows: reply.len().is_multiple_of(2),
            framing: true,
            grid_read: true,
            listed: &listed,
        },
        SheetRun {
            stdout: prompt,
            grid_read: corpus.len().is_multiple_of(2),
            ..SheetRun::default()
        },
    ];
    let attached = |target: &str| target.len().is_multiple_of(2);
    let turn = Turn {
        corpus,
        prompt,
        said: corpus,
        today: today_for(reply),
        pixel_only: false,
        hosts: &["github.com"],
        attached: &attached,
        layout: &layout,
        internals: Some(&check),
        vouching: prompt,
        runs: &runs,
    };
    internal_names(reply, corpus, &vocabulary, prompt).len()
        + check.names_in(prompt, || reply).len()
        + scrub_memory_layout(reply, corpus, &layout).len()
        + scrub_memory_layout(corpus, "", &layout).len()
        + grounding_tokens(reply).len()
        + memory_grounding(reply, &[corpus, prompt]).grounded_lines
        + sheet_findings(reply, &runs).unchecked.len()
        + ground_message(reply, &turn).findings.len()
        + ground_reply(corpus, &turn).findings.len()
        + ungrounded_on_page(reply, corpus, prompt, true).len()
        + invariant_checks(reply, corpus, prompt)
}

/// The invariants and the link text functions, on the three texts.
fn invariant_checks(reply: &str, corpus: &str, prompt: &str) -> usize {
    let tools: Vec<&str> = prompt.split_whitespace().take(4).collect();
    let record = TurnRecord {
        response: reply,
        raw_text: Some(corpus),
        tools_called: &tools,
        pending_action_count: 0,
        tool_result_corpus: Some(prompt),
        hit_step_limit: Some(false),
        hosts: &["github.com"],
    };
    evaluate_invariants(&record, &Invariant::ALL).len()
        + usize::from(claims_refusal(reply))
        + usize::from(claims_unbacked_action(corpus, 0, &tools))
        + usize::from(claims_unwritten_memory(reply, &tools))
        + usize::from(speaker_prefix(corpus).is_some())
        + silent_signal(reply).map_or(0, |m| m.reason.len())
        + without_links(reply).len()
        + unopened_links(reply, &[corpus]).len()
        + urls(reply)
            .iter()
            .map(|u| {
                usize::from(built_from_corpus(u.text, corpus, &["github.com"]))
                    + undotted_message_link(u.text).map_or(0, |l| l.len())
            })
            .sum::<usize>()
}

/// How many of `text`'s links `read` printed, so a parseable address is
/// compared too.
fn found_urls_printed(text: &str, read: &str) -> usize {
    urls(text)
        .iter()
        .filter(|url| url_printed_in(url.text, read))
        .count()
}

/// How many of `text`'s links a reply may build from `corpus`, and how many
/// of them keep a Slack message ts's dot.
fn found_urls_built(text: &str, corpus: &str) -> usize {
    urls(text)
        .iter()
        .filter(|url| {
            built_from_corpus(url.text, corpus, &["github.com"])
                || undotted_message_link(url.text).is_some()
        })
        .count()
}

/// One "today" per input, rotating through the edges of the calendar.
fn today_for(text: &str) -> Date {
    let todays = [
        Date::constant(2026, 10, 2),
        Date::constant(2027, 1, 31),
        Date::MIN,
        Date::MAX,
    ];
    todays
        .get(text.len() % todays.len())
        .copied()
        .unwrap_or(Date::MIN)
}

fn check(reply: &str, corpus: &str, prompt: &str) {
    let run = catch_unwind(AssertUnwindSafe(|| {
        black_box(exercise(reply, corpus, prompt))
    }));
    assert!(
        run.is_ok(),
        "panicked on reply={reply:?} corpus={corpus:?} prompt={prompt:?}"
    );
}

#[test]
#[ignore = "slow: run with --ignored in the panic-check profile"]
fn every_code_point_beside_a_figure_and_a_link() {
    for c in '\0'..=char::MAX {
        let figure = format!(
            "{c}1{c}2,500{c} {c}$9.99{c}USD 3{c}.{c}4 ١{c}٢ {c} 12{c}% 0{c},25 7 {c}percent و{c}٣ 1:{c}2 {c}"
        );
        let dated = format!(
            "Fri{c} 26 Sep{c} lundi {c}29/09 9月{c}26日({c}) 星期{c}五 9月26日 mar. ago.{c} 11"
        );
        let run = catch_unwind(|| black_box(misdated_weekdays(&dated, today_for(&dated)).len()));
        assert!(run.is_ok(), "panicked on {dated:?}");
        let bare = format!("mardi{c} 4 à{c} 17h Tue 4{c}th 火曜日{c} 4 Mon 24{c} at{c} 5{c}pm");
        let bare_run = catch_unwind(|| black_box(misdated_weekdays(&bare, today_for(&bare)).len()));
        assert!(bare_run.is_ok(), "panicked on {bare:?}");
        let table = format!(
            "Oct 23{c}–30\n| Oct{c} 20 | x |\n- 19-26 oct.{c}\n- Oct 28 {c}to Nov{c} 3\n{c}Nov 4, 9{c}:00-17:00\n- 5pm{c} Oct 20 CET{c}\n11月4日 15時{c}から\n- 火曜日 4 下午{c}3点"
        );
        let table_run =
            catch_unwind(|| black_box(rows_outside_window(&table, today_for(&table)).len()));
        assert!(table_run.is_ok(), "panicked on {table:?}");
        let link = format!(
            "{c}https://a.io/{c}x{c}) <https://b.io/{c}|{c}> [#1{c}](https://c.io/{c}) CORE-{c}1 [{c}]({c})"
        );
        check(&figure, &link, &figure);
        check(&link, &figure, &link);
    }
}

/// Fragments that sit on the reader's boundaries: digits of every script,
/// separators, currency, link and markdown punctuation, identifiers, marks
/// that combine or join, and letters that bound a figure or do not.
const POOL: &[&str] = &[
    "0",
    "1",
    "7",
    "9",
    "12",
    "250",
    "000",
    "1,250",
    "1.250",
    "8,22",
    "1.0842",
    "12,50,000",
    "١",
    "٢٥٠",
    "٬",
    "٫",
    "۱۲",
    "७",
    "𞥑",
    "𝟗",
    "１",
    "，",
    ",",
    ".",
    "'",
    "\u{2019}",
    " ",
    "  ",
    "\u{a0}",
    "\u{202f}",
    "\t",
    "\n",
    "$",
    "€",
    "¥",
    "₹",
    "₽",
    "円",
    "USD",
    "EUR",
    "CHF",
    "XAU",
    "ARR",
    "US",
    "USDX",
    "http",
    "https://",
    "http://",
    "HTTPS://",
    "a.io",
    "acme.io",
    "/pull/",
    "/issues/",
    "#",
    "#12",
    "CORE-1710",
    "AB-",
    "[",
    "]",
    "(",
    ")",
    "](",
    "<",
    ">",
    "|",
    "\\|",
    "\\",
    "*",
    "**",
    "_",
    "`",
    "~",
    "-",
    "−",
    "+",
    "＋",
    "-12,500",
    "($1,234.50)",
    "EUR-",
    "\u{200e}",
    "\u{61c}",
    "~",
    "≈",
    "~12,500",
    "10~20",
    " million",
    " mil millones",
    "K",
    "万",
    "千",
    "億",
    " Mrd",
    "$5m",
    "took ",
    "m ago",
    "m tall",
    " ×",
    "1h ",
    " 30s",
    " mn",
    " md files",
    "Rs ",
    " bin TL",
    " dollars",
    " d'euros",
    ",886b €",
    " thousand",
    "twelve",
    " hundred and",
    "-five",
    "quatre-vingt",
    " mille",
    "zwölf",
    "tausend",
    "dodici",
    "mila",
    " y ",
    "一",
    "零",
    "百",
    "a million",
    "الجمعة",
    "يوم ",
    "، الموافق ",
    "٢٦",
    " سبتمبر ",
    "كانون الأول",
    "\u{200f}/",
    "2026م",
    "ساعتين",
    " ونصف",
    " إلا ربع",
    " و30 ",
    "دقيقة",
    "خمس عشرة ",
    "١٫٥",
    "–",
    "—",
    " минут",
    "時間",
    " Std.",
    "-minute-",
    "分钟後",
    "合計",
    " tunnin",
    " хвилину",
    " y ",
    " 1'250",
    " วินาทีly",
    "ساعته",
    "دقائقَ",
    "मिनटों",
    " 1 1/2 hours",
    "½ hour",
    " hr. ",
    " mins.",
    "15h30",
    "à 3 h du matin",
    "de 14h à ",
    "Duration: ",
    "took 1:30:00",
    "[10:05:12] ",
    " giờ",
    "मिनट",
    " and a half",
    "half a",
    " et demi",
    "einhalb",
    "anderthalb",
    " point two",
    " Komma",
    " zero",
    "3億5000万",
    "5,000",
    "万6789",
    "/",
    "?",
    "&",
    "=",
    "%",
    "@",
    ":",
    ";",
    "!",
    "\"",
    "htmlPath: /p/clx7k2m9p0001/t",
    "example.com",
    "www.",
    "mailto:",
    "prod_VU49BpM22sWxIM",
    "0b6c2d4e-1f3a-4b5c-8d9e-0a1b2c3d4e5f",
    "<@",
    "https://www.acme.com:8080/a/../b?q=1#x",
    "file:////",
    "file:\\\\C|\\..",
    "foo://h:x/",
    "ftp://",
    "//",
    "[::1]",
    ":65536",
    "\\..",
    "%2e",
    "|x>",
    " -> ",
    "(a|b)>",
    "silent: ",
    "\n[CURRENT MESSAGE] ",
    "[Mio]: ",
    "I’ll remember",
    "saved it to my notes",
    "%31",
    "https:/",
    "cs_test_",
    "a1B2c3D4e5F6",
    "[derived ",
    "[as of",
    ".slack.com/archives/C1/p1791180327.741099",
    "?thread_ts=",
    "/ads/library/?view_all_page_id=",
    "xn--",
    "\u{212A}",
    "\u{130}",
    "合計",
    "は",
    "ー",
    "ｻ",
    "é",
    "e\u{301}",
    "\u{200d}",
    "\u{200b}",
    "\u{200f}",
    "\u{feff}",
    "\u{0}",
    "\u{7f}",
    "😀",
    "👍🏽",
    "ℝ",
    "𝐀",
    "Ⅳ",
    "①",
    "²",
    "x",
    "K",
    "p",
    "v",
    "FY",
    "2026-11-05",
    "%",
    "٪",
    "％",
    "﹪",
    " %",
    " percent",
    " per cent",
    " pour cent",
    "Prozent",
    "0.1429",
    "0,25",
    "100%",
    "و",
    "وب",
    "ل",
    "12:30",
    "10/2026",
    "4 + ",
    "Tue 4 ",
    "mardi ",
    " à 17h",
    " at 5pm",
    "火曜日 ",
    "15時30分",
    "午後3時から5時",
    "下午3点半",
    "到24点",
    "lúc 7 giờ 30 phút sáng",
    " tomorrow 15",
    " for an hour",
    " 2.5 days ago",
    " через полчаса",
    "两小时内",
    " +02:00",
    " CET",
    "T10:00Z",
    "9:00-17:00",
    "22:00-02:00",
    "15.00-0500",
    "00 19 de nov",
    "Friday, tomorrow",
    "今日(金)",
    "Freitag, morgen",
    " vanavond",
    " to today",
    "3 days ago - ",
    " - hôm nay",
    "13/05(木)",
    "31/10-",
    "5.1.2025",
    "25.11.25 bis ",
    " +11mins",
    "juillet 24",
    ", 12 posts",
    " a 4 abr.",
    "\n| Oct 20 | ",
    "Oct 23-30\n",
    "\n- ",
    "19-26 oct.",
    " to ",
    "Dec 28",
    " млн",
    "млрд",
    "ล้าน",
    "억",
    "만",
    "천",
    " miljoen",
    " mila miliardi",
    " lakh crore",
    "Kmembers",
    "bn",
    " M",
    " m",
    "B",
    "T",
    " tỷ",
    " tysiące",
    "हज़ार",
    "$74.74 spend, 2 leads, $45.83 CPL",
    " rb",
    "jt",
    " tr",
    " nt",
    " lcr",
    " tri",
    " bi",
    " bin",
    " Mr",
    " Md",
    " mn",
    "₺",
    " L",
    "l",
    " n",
    " nghìn tỷ",
    " mille miliardi",
    " d'euros",
    " de ",
    " billion",
    " Tn",
    " % ",
    "0,850",
    " 万",
    "USD ",
    " अ\u{970}",
    "57.37M",
    "LT: $91.65 / 2 leads / CPL $45.83",
    "?, ",
    ". ",
    ".)",
    "silent: ",
    "silent:",
    "[CURRENT MESSAGE]",
    "[Ana]: ",
    "\u{2028}",
    "I've created the ticket",
    "done! I",
    "I'll remember",
    "saved it to my notes",
    "sorry, I can't",
    "ไทย_tool",
    "ค้นหา(",
];

/// A xorshift generator, seeded so a failure reproduces.
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }

    /// Up to 60 pieces: a pool fragment, a random code point from any plane,
    /// or a run of up to 300 digits.
    fn text(&mut self) -> String {
        let mut s = String::new();
        for _ in 0..self.below(60) {
            match self.below(20) {
                0 => s.extend(char::from_u32(
                    u32::try_from(self.below(0x11_0000)).unwrap(),
                )),
                1 => {
                    for _ in 0..self.below(300) {
                        s.extend(char::from_digit(u32::try_from(self.below(10)).unwrap(), 10));
                    }
                }
                _ => s.push_str(POOL[self.below(POOL.len())]),
            }
        }
        s
    }
}

#[test]
#[ignore = "slow: run with --ignored in the panic-check profile"]
fn generated_turns() {
    let cases: usize = std::env::var("PANIC_CHECK_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200_000);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..cases {
        let (reply, corpus, prompt) = (rng.text(), rng.text(), rng.text());
        check(&reply, &corpus, &prompt);
    }
}
