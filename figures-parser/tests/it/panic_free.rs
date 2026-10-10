//! Every public entry point, fed every code point and generated turns, must
//! return without panicking. Slow, so ignored by default: `make
//! check-figures-parser` runs them in the `panic-check` profile, a release
//! build that also panics on overflow.

use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};

use figures_parser::number::scan::{Scratch, tokens};
use figures_parser::utf16::{Utf16Cursor, units};
use figures_parser::{
    Date, durations, emails, extract_numbers, fabricated_urls, identifiers, misdated_weekdays,
    numbers, ungrounded_durations, ungrounded_emails, ungrounded_figures, ungrounded_identifiers,
    ungrounded_urls, unseen_tokens, urls,
};

/// Calls every entry point on the three texts, and returns a sum the
/// optimizer cannot drop.
fn exercise(reply: &str, corpus: &str, prompt: &str) -> usize {
    let mut n = 0;
    let mut scratch = Scratch::default();
    for text in [reply, corpus, prompt] {
        n += tokens(text).count() + tokens(text).without_currency().count();
        for token in tokens(text) {
            token.for_each_reading(&mut scratch, |r| {
                n += r.stated().map_or(0, |k| k.integer().len()) + usize::from(r.currency);
            });
        }
        n += numbers(text).count()
            + extract_numbers(text).len()
            + emails(text).len()
            + identifiers(text).len()
            + durations(text).len();
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
        + misdated_weekdays(corpus, today_for(corpus)).len();
    n + ungrounded_figures(reply, corpus).len()
        + ungrounded_figures(corpus, reply).len()
        + fabricated_urls(reply, corpus).len()
        + fabricated_urls(corpus, reply).len()
        + ungrounded_urls(reply, corpus, prompt).len()
        + ungrounded_urls(prompt, reply, corpus).len()
        + ungrounded_emails(reply, corpus, prompt).len()
        + ungrounded_identifiers(reply, corpus, prompt).len()
        + unseen_tokens(reply, corpus, prompt).len()
        + ungrounded_durations(reply, corpus, prompt).len()
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
        let figure = format!("{c}1{c}2,500{c} {c}$9.99{c}USD 3{c}.{c}4 ١{c}٢ {c}");
        let dated = format!(
            "Fri{c} 26 Sep{c} lundi {c}29/09 9月{c}26日({c}) 星期{c}五 9月26日 mar. ago.{c} 11"
        );
        let run = catch_unwind(|| black_box(misdated_weekdays(&dated, today_for(&dated)).len()));
        assert!(run.is_ok(), "panicked on {dated:?}");
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
