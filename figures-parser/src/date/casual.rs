//! A weekday merged with the casual word for a day beside it ("Friday,
//! tomorrow", "vendredi demain", "今日(金)"), as chrono's weekday refiner
//! merges them: the pair says which day that word names, so its weekday is
//! checked against that day.
//!
//! Only words that name a day are read. A part of the day ("this morning",
//! "上午", "Freitag morgen") names none, though chrono's Chinese and German
//! parsers date it.

use std::sync::LazyLock;

use jiff::ToSpan;
use jiff::civil::Date;
use regex::Regex;

use super::languages::Found;

/// The days a casual word lies from today, by the word's start.
type Offsets = &'static [(&'static str, i64)];

/// A language's casual words for a day as written in source: its code, the
/// words' pattern, the right boundary and the offsets.
type Entry = (&'static str, &'static str, &'static str, Offsets);

/// A language's casual words for a day, compiled: its code, the pattern and
/// the offsets.
type Words = (&'static str, Option<Regex>, Offsets);

/// Each language's casual words for a day, after the weekday they follow
/// (before it in Japanese), each with the days it lies from today; the
/// pattern's group `w` is what the result spans, `d` the word for the day
/// and `post` its consumed right boundary.
static WORDS: LazyLock<Vec<Words>> = LazyLock::new(|| {
    let ascii = "(?P<post>[^A-Za-z0-9_]|$)";
    let unicode = r"(?P<post>[^\p{L}\p{N}_]|$)";
    let table: [Entry; 13] = [
        (
            "en",
            r"(?P<d>now|today|tonight|tomorrow|overmorrow|tmr|tmrw|yesterday|last\s*night)",
            ascii,
            &[
                ("now", 0),
                ("today", 0),
                ("tonight", 0),
                ("tomorrow", 1),
                ("tmr", 1),
                ("tmrw", 1),
                ("overmorrow", 2),
                ("yesterday", -1),
                ("last", -1),
            ],
        ),
        (
            "fr",
            r"(?P<d>maintenant|aujourd'hui|demain|hier|cette\s*nuit|la\s*veille)",
            ascii,
            &[
                ("maintenant", 0),
                ("aujourd'hui", 0),
                ("demain", 1),
                ("hier", -1),
                ("cette", 0),
                ("la", -1),
            ],
        ),
        (
            "de",
            r"(?P<d>jetzt|heute|morgen|übermorgen|uebermorgen|gestern|vorgestern|letzte\s*nacht)(?:\s*(?:morgen|vormittag|mittags?|nachmittag|abend|nacht|mitternacht))?",
            ascii,
            &[
                ("jetzt", 0),
                ("heute", 0),
                ("morgen", 1),
                ("übermorgen", 2),
                ("uebermorgen", 2),
                ("gestern", -1),
                ("vorgestern", -2),
                ("letzte", -1),
            ],
        ),
        (
            "es",
            "(?P<d>ahora|hoy|mañana|ayer)",
            ascii,
            &[("ahora", 0), ("hoy", 0), ("mañana", 1), ("ayer", -1)],
        ),
        (
            "it",
            r"(?P<d>adesso|ora|oggi|stasera|stanotte|domani|dopodomani|ieri\s*sera|ieri\s*notte|ieri)",
            ascii,
            &[
                ("adesso", 0),
                ("ora", 0),
                ("oggi", 0),
                ("stasera", 0),
                ("stanotte", 0),
                ("domani", 1),
                ("dopodomani", 2),
                ("ieri", -1),
            ],
        ),
        (
            "pt",
            "(?P<d>agora|hoje|amanha|amanhã|ontem)",
            ascii,
            &[
                ("agora", 0),
                ("hoje", 0),
                ("amanha", 1),
                ("amanhã", 1),
                ("ontem", -1),
            ],
        ),
        (
            "nl",
            "(?P<d>(?:gisteren|morgen|van)(?:ochtend|middag|namiddag|avond|nacht)|nu|vandaag|morgen|morgend|gisteren)",
            ascii,
            &[
                ("nu", 0),
                ("vandaag", 0),
                ("van", 0),
                ("morgen", 1),
                ("gisteren", -1),
            ],
        ),
        (
            "sv",
            r"(?P<d>nu|idag|imorgon|igår|förrgår|i\s*förrgår)(?:\s*(?:på\s*)?(?:morgonen?|förmiddagen?|middagen?|eftermiddagen?|kvällen?|natten?|midnatt))?",
            ascii,
            &[
                ("nu", 0),
                ("idag", 0),
                ("imorgon", 1),
                ("igår", -1),
                ("förrgår", -2),
                ("i", -2),
            ],
        ),
        (
            "fi",
            r"(?P<d>nyt|tänään|huomenna|ylihuomenna|eilen|toissapäivänä|viime\s*yönä)(?:\s*(?:aamulla|aamuna|aamupäivällä|päivällä|iltapäivällä|illalla|yöllä|keskiyöllä))?",
            ascii,
            &[
                ("nyt", 0),
                ("tänään", 0),
                ("huomenna", 1),
                ("ylihuomenna", 2),
                ("eilen", -1),
                ("toissapäivänä", -2),
                ("viime", -1),
            ],
        ),
        (
            "ru",
            r"(?:с|со)?\s*(?P<d>сегодня|вчера|завтра|послезавтра|послепослезавтра|позапозавчера|позавчера)",
            unicode,
            &[
                ("сегодня", 0),
                ("вчера", -1),
                ("завтра", 1),
                ("послезавтра", 2),
                ("послепослезавтра", 3),
                ("позавчера", -2),
                ("позапозавчера", -3),
            ],
        ),
        (
            "uk",
            r"(?:з|із|від)?\s*(?P<d>сьогодні|вчора|завтра|післязавтра|післяпіслязавтра|позапозавчора|позавчора)",
            unicode,
            &[
                ("сьогодні", 0),
                ("вчора", -1),
                ("завтра", 1),
                ("післязавтра", 2),
                ("післяпіслязавтра", 3),
                ("позавчора", -2),
                ("позапозавчора", -3),
            ],
        ),
        (
            "zh",
            r"(?P<d>而家|立刻|立即|即刻|(?:今|明|前|大前|後|大後|聽|昨|尋|琴)(?:早|朝|晚|日|天))",
            "",
            &[
                ("而家", 0),
                ("立", 0),
                ("即", 0),
                ("今", 0),
                ("明", 1),
                ("聽", 1),
                ("昨", -1),
                ("尋", -1),
                ("琴", -1),
                ("大前", -3),
                ("前", -2),
                ("大後", 3),
                ("後", 2),
            ],
        ),
        (
            "vi",
            "(?P<d>hôm nay|hôm qua|hôm kia|ngày mai|ngày kia|bây giờ|lúc này)",
            ascii,
            &[
                ("hôm nay", 0),
                ("hôm qua", -1),
                ("hôm kia", -2),
                ("ngày mai", 1),
                ("ngày kia", 2),
                ("bây giờ", 0),
                ("lúc này", 0),
            ],
        ),
    ];
    table
        .into_iter()
        .map(|(code, words, post, offsets)| {
            let re = crate::text::compile(&format!("(?i)^(?P<w>{words}){post}"));
            (code, re, offsets)
        })
        .collect()
});

/// The Japanese casual words for a day, before the weekday they merge with,
/// and the days each lies from today.
const JAPANESE: &[(&str, i64)] = &[
    ("今日", 0),
    ("きょう", 0),
    ("本日", 0),
    ("ほんじつ", 0),
    ("今夜", 0),
    ("こんや", 0),
    ("今夕", 0),
    ("こんゆう", 0),
    ("今晩", 0),
    ("こんばん", 0),
    ("今朝", 0),
    ("けさ", 0),
    ("昨日", -1),
    ("きのう", -1),
    ("明日", 1),
    ("あした", 1),
];

/// The days a casual word lies from today: that of the first entry its
/// lowercase text starts with.
fn offset(offsets: &[(&str, i64)], word: &str) -> Option<i64> {
    let lower = word.to_lowercase();
    offsets
        .iter()
        .find(|(w, _)| lower.starts_with(w))
        .map(|&(_, days)| days)
}

/// The span of the weekday `weekday` merged with the casual word for a day
/// beside it in `reply`, and the day that word names; None when no such word
/// stands beside it.
pub fn beside(
    code: &str,
    reply: &str,
    weekday: &Found,
    today: Date,
) -> Option<((usize, usize), Date)> {
    if code == "ja" {
        let before = reply.get(..weekday.start)?;
        let gap = before.trim_end_matches(char::is_whitespace);
        let gap = gap
            .strip_suffix(['、', ',', 'の'])
            .unwrap_or(gap)
            .trim_end_matches(char::is_whitespace);
        let (word, days) = JAPANESE.iter().find(|(w, _)| gap.ends_with(w))?;
        let start = gap.len().checked_sub(word.len())?;
        let date = today.checked_add(days.days()).ok()?;
        return Some(((start, weekday.end), date));
    }
    let (_, re, offsets) = WORDS.iter().find(|(c, _, _)| *c == code)?;
    let after = reply.get(weekday.end..)?;
    let rest = after.strip_prefix(',').unwrap_or(after);
    let rest = rest.trim_start_matches(char::is_whitespace);
    let at = after
        .len()
        .checked_sub(rest.len())?
        .checked_add(weekday.end)?;
    let caps = re.as_ref()?.captures(rest)?;
    let word = caps.name("w")?;
    let day = caps.name("d")?.as_str();
    // "Freitag morgen" and "vrijdag morgen" are that day's morning; only a
    // comma makes the word "tomorrow" ("Freitag, morgen").
    let comma = after.starts_with(',')
        || reply
            .get(weekday.start..weekday.end)
            .is_some_and(|w| w.trim_end().ends_with(','));
    if matches!(code, "de" | "nl") && day.eq_ignore_ascii_case("morgen") && !comma {
        return None;
    }
    let days = offset(offsets, day)?;
    let date = today.checked_add(days.days()).ok()?;
    Some(((weekday.start, at.checked_add(word.end())?), date))
}
