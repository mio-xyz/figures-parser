//! Each language's weekday and date parsers, translated from chrono-node
//! 2.10.2's patterns (MIT), in the order its configuration runs them.
//!
//! Arabic, which chrono does not read, has parsers of its own in the same
//! shape.
//!
//! A pattern's left word boundary is a consumed `pre` group and its right
//! lookahead a consumed `post` group, so the engine backtracks the way
//! chrono's does (an optional year is dropped when what follows it would not
//! end the date); a result's text runs from after `pre` to before `post`. A
//! negative lookahead is a check after the match, falling back to the same
//! pattern without its year where chrono would have backtracked out of it.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::names;
use crate::number::digits::digit_value;
use crate::text::{
    alternation, inside_word, is_ascii_word, is_unspaced_script, lookup, run_end, same_word,
    starts_with_lowercase,
};

/// Where a date's year comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YearOf {
    /// The text writes it.
    Written(i32),
    /// The text writes none: the year nearest today, as most of chrono's parsers read it.
    Nearest,
    /// The text writes none and the parser takes today's year (Chinese), or
    /// says "this year" (Japanese 今年).
    Current,
}

/// What one parser read at one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// A weekday alone, counted from Sunday as 0.
    Weekday(u8),
    /// A date with its day: the day and month as written, and where its year comes from.
    Date {
        /// The day of the month.
        day: u8,
        /// The month, 1 to 12.
        month: u8,
        /// Where the year comes from.
        year: YearOf,
    },
    /// A date with a weekday merged into it.
    Dated {
        /// The weekday as written.
        weekday: u8,
        /// The day of the month.
        day: u8,
        /// The month, 1 to 12.
        month: u8,
        /// Where the year comes from.
        year: YearOf,
    },
    /// Anything else a parser takes: it removes what it overlaps and merges with nothing.
    Other,
}

/// One result: its byte range in the text and what it read.
#[derive(Debug, Clone, Copy)]
pub struct Found {
    /// Byte offset of the result's text.
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
    /// What it read.
    pub reading: Reading,
}

/// How a language's refiner pairs a weekday with a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merge {
    /// A weekday, then at most a comma and whitespace, then a date.
    WeekdayThenDate,
    /// A date, then at most `,`, `、` or `の` and whitespace, then a weekday (Japanese).
    DateThenWeekday,
    /// A weekday, then whitespace, bidi marks and at most one comma, Latin
    /// or Arabic, then a date (Arabic).
    ArabicWeekdayThenDate,
}

/// The words a language's month-name dates read.
struct Words {
    /// Month words, lowercase, to their month.
    months: &'static [(&'static str, u8)],
    /// Ordinal day words, lowercase, to their day.
    ordinals: &'static [(&'static str, u8)],
}

/// What must not follow a written year, a negative lookahead in chrono.
#[derive(Clone, Copy)]
enum YearCheck {
    /// No ASCII word character.
    NotWord,
    /// English: no word character, and a two-digit year not followed by a
    /// time or a month (`26 am`, `26 Sep`).
    EnglishYear,
    /// Not a non-space character followed by a digit.
    NoGluedDigit,
}

/// How a parser turns a match into a reading.
#[derive(Clone, Copy)]
enum Kind {
    /// A weekday word.
    Weekday(&'static [(&'static str, u8)]),
    /// A weekday character (Japanese and Chinese).
    WeekdayChar(&'static [(char, u8)]),
    /// Read only so it can remove what it overlaps, never merged: a relative
    /// weekday ("下周五"), a month and a year ("9/2026", "2026 Sep").
    Other,
    /// A month with no day ("September", "Mar 2024"): read only so it can
    /// remove what it overlaps, as chrono's month parsers do.
    MonthOnly {
        /// Full month names; a match of three characters or fewer that is
        /// not one ("mar", "gen") is rejected.
        full: &'static [&'static str],
        /// Whether the result's text leaves out the `prefix` group ("in", "di").
        drop_prefix: bool,
    },
    /// A day and a month word, in either order.
    MonthName {
        /// The language's words.
        words: &'static Words,
        /// Whether the day comes first: a rejected day past 31 is then
        /// skipped whole ("96 Ago" never retries as "6 Ago").
        day_first: bool,
        /// Italian's middle-endian rule: a two-digit day with no year reads as a year.
        skip_year_like: bool,
        /// What must not follow the year.
        year_check: YearCheck,
    },
    /// A four-digit year, a month (word or number) and a day.
    YearMonthDay(&'static Words),
    /// Two numbers and an optional year joined by `/`, `.` or `-`.
    Slash {
        /// Whether the first number is the day.
        day_first: bool,
    },
    /// `YYYY-MM-DD`, with an optional time.
    Iso,
    /// Vietnamese: `ngày D tháng M năm Y`.
    Vietnamese,
    /// Japanese: `Y年M月D日`, `Y/M/D` or `M/D`, digits halfwidth or fullwidth.
    Japanese,
    /// Chinese: `Y年M月D日`, digits or Chinese numerals.
    Chinese,
    /// Arabic: a day, a month word or number and an optional year, in
    /// Western or Arabic-Indic digits.
    Arabic,
    /// Arabic: a day, a month and an optional year, joined by `/`, `.` or
    /// `-`, day first, in Western or Arabic-Indic digits.
    ArabicSlash,
}

/// One parser: its pattern, the pattern without its year, and its reading.
struct Parser {
    /// The pattern; None only if it failed to compile.
    re: Option<Regex>,
    /// The same pattern without its optional year, for the fallback.
    no_year: Option<Regex>,
    /// How a match reads.
    kind: Kind,
}

/// A language's parsers in chrono's order, and how its weekdays merge.
pub struct Language {
    /// How a weekday and a date merge.
    pub merge: Merge,
    /// The parsers, in chrono's order.
    parsers: Vec<Parser>,
}

/// ASCII non-word, JavaScript's `\W`.
const NW: &str = "[^A-Za-z0-9_]";
/// The left word boundary chrono puts before a parser.
const PRE: &str = "(?:(?P<pre>[^A-Za-z0-9_])|^)";
/// The left boundary Russian and Ukrainian parsers use.
const PRE_U: &str = r"(?:(?P<pre>[^\p{L}\p{N}_])|^)";
/// The right boundary after most parsers.
const POST: &str = "(?P<post>[^A-Za-z0-9_]|$)";
/// The right boundary after a French, Spanish or Portuguese weekday, which a digit may follow.
const POST_DIGIT: &str = "(?P<post>[^A-Za-z_]|$)";
/// The right boundary Russian and Ukrainian parsers use.
const POST_U: &str = r"(?P<post>[^\p{L}\p{N}_]|$)";
/// A weekday's trailing comma or closing parenthesis.
const CLOSE: &str = r"(?:\s*(?:,|\)|）))?";
/// A weekday's leading comma or opening parenthesis.
const OPEN: &str = r"(?:(?:,|\(|（)\s*)?";

/// The keys of a word table as an alternation.
fn keys(table: &'static [(&'static str, u8)]) -> String {
    alternation(table.iter().map(|&(w, _)| w))
}

/// A case-insensitive regex, None if it does not compile.
fn compile(pattern: &str) -> Option<Regex> {
    crate::text::compile(&format!("(?i){pattern}"))
}

/// A parser whose pattern has an optional year, written `{Y}` where the year
/// group goes and `{/Y}` where it ends.
fn with_year(pattern: &str, kind: Kind) -> Parser {
    let full = pattern.replace("{Y}", "").replace("{/Y}", "");
    let no_year = match (pattern.find("{Y}"), pattern.find("{/Y}")) {
        (Some(open), Some(close)) => {
            let head = pattern.get(..open).unwrap_or_default();
            let tail = pattern.get(close + "{/Y}".len()..).unwrap_or_default();
            compile(&format!("{head}{tail}"))
        }
        _ => None,
    };
    Parser {
        re: compile(&full),
        no_year,
        kind,
    }
}

/// A parser with no year fallback.
fn plain(pattern: &str, kind: Kind) -> Parser {
    Parser {
        re: compile(pattern),
        no_year: None,
        kind,
    }
}

/// What may follow a month with no day: chrono's
/// `(?=[^\s\w]|\s+[^0-9]|\s+$|$)`.
const MONTH_POST: &str = r"(?P<post>[^\sA-Za-z0-9_]|\s+[^0-9]|\s+$|$)";

/// A month-only parser: `prefix` before the month, then `year`: chrono's
/// separator and optional year, which take a trailing comma and whitespace
/// even with no year, so the result can outgrow a date it overlaps.
fn month_only(
    pre: &str,
    prefix: &str,
    months: &'static [(&'static str, u8)],
    year: &str,
    full: &'static [&'static str],
    drop_prefix: bool,
) -> Parser {
    plain(
        &format!(
            "{pre}(?P<prefix>{prefix})?(?P<m>{}){year}{MONTH_POST}",
            keys(months)
        ),
        Kind::MonthOnly { full, drop_prefix },
    )
}

/// A month written as a number and a four-digit year (`9/2026`): read only
/// for its overlaps.
fn slash_month() -> Parser {
    plain(
        &format!("{PRE}(?:[0-9]|0[1-9]|1[012])/[0-9]{{4}}{POST}"),
        Kind::Other,
    )
}

/// The shared `YYYY-MM-DD` parser.
fn iso() -> Parser {
    plain(
        &format!(
            r"{PRE}(?P<y>[0-9]{{4}})-(?P<mn>[0-9]{{1,2}})-(?P<d>[0-9]{{1,2}})(?:T[0-9]{{1,2}}:[0-9]{{1,2}}(?::[0-9]{{1,2}}(?:\.[0-9]{{1,4}})?)?(?:Z|[+-][0-9]{{2}}:?(?:[0-9]{{2}})?)?)?{POST}"
        ),
        Kind::Iso,
    )
}

/// A year, a month word or number and a day, joined by `separator`.
fn year_month_day(words: &'static Words, separator: &str) -> Parser {
    plain(
        &format!(
            r"{PRE}(?P<y>[0-9]{{4}}){separator}(?:(?P<m>{})|(?P<mn>[0-9]{{1,2}})){separator}(?P<d>[0-9]{{1,2}}){POST}",
            keys(words.months)
        ),
        Kind::YearMonthDay(words),
    )
}

/// The shared slash parser.
fn slash(day_first: bool) -> Parser {
    plain(
        &format!(
            r"(?:(?P<pre>[^0-9])|^)(?P<a>[0-3]?[0-9])[/.\-](?P<b>[0-3]?[0-9])(?:[/.\-](?P<y>[0-9]{{4}}|[0-9]{{2}}))?(?P<post>{NW}|$)"
        ),
        Kind::Slash { day_first },
    )
}

/// A weekday-word parser: `before` and `after` are the language's modifier
/// patterns around the word.
fn weekday(
    pre: &str,
    before: &str,
    table: &'static [(&'static str, u8)],
    extra: &str,
    after: &str,
    post: &str,
) -> Parser {
    plain(
        &format!(
            "{pre}{OPEN}{before}(?P<w>{}{extra}){after}{post}",
            keys(table)
        ),
        Kind::Weekday(table),
    )
}

impl Words {
    /// A language's month and ordinal words.
    const fn new(
        months: &'static [(&'static str, u8)],
        ordinals: &'static [(&'static str, u8)],
    ) -> Self {
        Self { months, ordinals }
    }
}

/// English month and ordinal words.
static EN_WORDS: Words = Words::new(names::EN_MONTHS, names::EN_ORDINALS);
/// French month and ordinal words.
static FR_WORDS: Words = Words::new(names::FR_MONTHS, &[]);
/// German month and ordinal words.
static DE_WORDS: Words = Words::new(names::DE_MONTHS, &[]);
/// Spanish month and ordinal words.
static ES_WORDS: Words = Words::new(names::ES_MONTHS, &[]);
/// Italian month and ordinal words.
static IT_WORDS: Words = Words::new(names::IT_MONTHS, names::IT_ORDINALS);
/// Portuguese month and ordinal words.
static PT_WORDS: Words = Words::new(names::PT_MONTHS, &[]);
/// Dutch month and ordinal words.
static NL_WORDS: Words = Words::new(names::NL_MONTHS, names::NL_ORDINALS);
/// Swedish month and ordinal words.
static SV_WORDS: Words = Words::new(names::SV_MONTHS, &[]);
/// Finnish month and ordinal words.
static FI_WORDS: Words = Words::new(names::FI_MONTHS, &[]);
/// Russian month and ordinal words.
static RU_WORDS: Words = Words::new(names::RU_MONTHS, names::RU_ORDINALS);
/// Ukrainian month and ordinal words.
static UK_WORDS: Words = Words::new(names::UK_MONTHS, names::UK_ORDINALS);

/// English.
fn english() -> Language {
    let m = keys(names::EN_MONTHS);
    let ord = format!(
        "(?:{}|[0-9]{{1,2}}(?:st|nd|rd|th)?)",
        keys(names::EN_ORDINALS)
    );
    let year = r"(?:[1-9][0-9]{0,3}\s{0,2}(?:BE|AD|BC|BCE|CE)|[1-9][0-9]{3}|[0-9]{2})";
    let month_name = |day_first: bool| Kind::MonthName {
        words: &EN_WORDS,
        day_first,
        skip_year_like: false,
        year_check: YearCheck::EnglishYear,
    };
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            year_month_day(&EN_WORDS, r"[-./\s]"),
            iso(),
            slash(false),
            with_year(
                &format!(
                    r"{PRE}(?:on\s{{0,3}})?(?P<d>{ord})(?:\s{{0,3}}(?:to|-|–|until|through|till)\s{{0,3}}(?P<d2>{ord}))?(?:-|/|\s{{0,3}}(?:of)?\s{{0,3}})(?P<m>{m}){{Y}}(?:(?:-|/|,?\s{{0,3}})(?P<y>{year})){{/Y}}?{POST}"
                ),
                month_name(true),
            ),
            with_year(
                &format!(
                    r"{PRE}(?P<m>{m})(?:-|/|\s*,?\s*)(?P<d>{ord})\s*(?:(?:to|-)\s*(?P<d2>{ord})\s*)?{{Y}}(?:(?:-|/|\s*,\s*|\s+)(?P<y>{year})){{/Y}}?{POST}"
                ),
                month_name(false),
            ),
            weekday(
                PRE,
                r"(?:on\s*?)?(?:(?:this|last|past|next)\s*)?",
                names::EN_WEEKDAYS,
                "|weekend|weekday",
                &format!(r"{CLOSE}(?:\s*(?:of\s*)?(?:this|last|past|next)\s*week)?"),
                POST,
            ),
            slash_month(),
            plain(
                &format!(
                    r"{PRE}(?:[1-9][0-9]{{0,3}}\s{{0,2}}(?:BE|AD|BC|BCE|CE)|[1-9][0-9]{{3}})(?:\s*[-./,]?\s*|\s+of\s+)(?:{m}){MONTH_POST}"
                ),
                Kind::Other,
            ),
            month_only(
                PRE,
                r"in\s*",
                names::EN_MONTHS,
                &format!(r"\s*(?:(?:,|-|of)?\s*(?:{year})?)?"),
                names::EN_FULL_MONTHS,
                true,
            ),
        ],
    }
}

/// A day-first month-name parser, chrono's shape in French, German,
/// Spanish, Portuguese, Swedish, Finnish, Russian and Ukrainian: `head` (the
/// day and its range) before the month word, `tail` (the optional year
/// between `{Y}` and `{/Y}`) after it.
fn little_endian(pre: &str, head: &str, words: &'static Words, tail: &str, post: &str) -> Parser {
    with_year(
        &format!("{pre}{head}(?P<m>{}){tail}{post}", keys(words.months)),
        Kind::MonthName {
            words,
            day_first: true,
            skip_year_like: false,
            year_check: YearCheck::NoGluedDigit,
        },
    )
}

/// French.
fn french() -> Language {
    let day = "[0-9]{1,2}(?:er)?";
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            little_endian(
                PRE,
                &format!(
                    r"(?:on\s*?)?(?P<d>{day})(?:\s*(?:au|-|–|jusqu'au?|\s)\s*(?P<d2>{day}))?(?:-|/|\s*(?:de)?\s*)"
                ),
                &FR_WORDS,
                r"{Y}(?:(?:-|/|,?\s*)(?P<y>[1-9][0-9]{0,3}\s*(?:AC|AD|p\.\s*C(?:hr?)?\.\s*n\.)|[1-2][0-9]{3}|[5-9][0-9])){/Y}?",
                POST,
            ),
            weekday(
                PRE,
                r"(?:(?:ce)\s*)?",
                names::FR_WEEKDAYS,
                "",
                &format!(r"{CLOSE}(?:\s*(?:dernier|prochain)\s*)?"),
                POST_DIGIT,
            ),
        ],
    }
}

/// German.
fn german() -> Language {
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            little_endian(
                PRE,
                r"(?:am\s*?)?(?:den\s*?)?(?P<d>[0-9]{1,2})\.(?:\s*(?:bis(?:\s*(?:am|zum))?|-|–|\s)\s*(?P<d2>[0-9]{1,2})\.?)?\s*",
                &DE_WORDS,
                r"{Y}(?:(?:-|/|,?\s*)(?P<y>[0-9]{1,4}(?:\s*[vn]\.?\s*(?:C(?:hr)?|(?:u\.?|d\.?(?:\s*g\.?)?)?\s*Z)\.?|\s*(?:u\.?|d\.?(?:\s*g\.)?)\s*Z\.?)?)){/Y}?",
                POST,
            ),
            weekday(
                PRE,
                r"(?:a[mn]\s*?)?(?:(?:diese[mn]|letzte[mn]|n(?:ä|ae)chste[mn])\s*)?",
                names::DE_WEEKDAYS,
                "",
                &format!(r"{CLOSE}(?:\s*(?:diese|letzte|n(?:ä|ae)chste)\s*woche)?"),
                POST,
            ),
        ],
    }
}

/// Spanish and Portuguese share their date shape.
fn iberian(
    weekdays: &'static [(&'static str, u8)],
    words: &'static Words,
    modifiers: &str,
) -> Language {
    let day = "[0-9]{1,2}(?:º|ª|°)?";
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            weekday(
                PRE,
                &format!(r"(?:(?:{modifiers})\s*)?"),
                weekdays,
                "",
                &format!(r"{CLOSE}(?:\s*(?:{modifiers})\s*semana)?"),
                POST_DIGIT,
            ),
            little_endian(
                PRE,
                &format!(
                    r"(?P<d>{day})(?:\s*(?:desde|de|-|–|ao?|\s)\s*(?P<d2>{day}))?\s*(?:de)?\s*(?:-|/|\s*(?:de|,)?\s*)"
                ),
                words,
                r"{Y}(?:\s*(?:de|,)?\s*(?P<y>[0-9]{1,4}(?:\s*[a|d]\.?\s*c\.?|\s*a\.?\s*d\.?)?)){/Y}?",
                POST,
            ),
        ],
    }
}

/// Italian.
fn italian() -> Language {
    let m = keys(names::IT_MONTHS);
    let ord = format!("(?:{}|[0-9]{{1,2}}(?:°|ª|º)?)", keys(names::IT_ORDINALS));
    let year = r"(?:[1-9][0-9]{0,3}\s{0,2}(?:BE|AD|BC|BCE|CE)|[1-2][0-9]{3}|[5-9][0-9]|2[0-5])";
    let month_name = |day_first: bool| Kind::MonthName {
        words: &IT_WORDS,
        day_first,
        skip_year_like: !day_first,
        year_check: YearCheck::NotWord,
    };
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            year_month_day(&IT_WORDS, r"[-./\s]"),
            iso(),
            slash(true),
            with_year(
                &format!(
                    r"{PRE}(?:il\s{{0,3}})?(?P<d>{ord})(?:\s{{0,3}}(?:al|-|–|fino\s*al?)?\s{{0,3}}(?P<d2>{ord}))?(?:-|/|\s{{0,3}}(?:di)?\s{{0,3}})(?P<m>{m}){{Y}}(?:(?:-|/|,?\s{{0,3}})(?P<y>{year})){{/Y}}?{POST}"
                ),
                month_name(true),
            ),
            with_year(
                &format!(
                    r"{PRE}(?P<m>{m})(?:-|/|\s*,?\s*)(?P<d>{ord})\s*(?:(?:al|-)\s*(?P<d2>{ord})\s*)?{{Y}}(?:(?:-|/|\s*,\s*|\s+)(?P<y>{year})){{/Y}}?{POST}"
                ),
                month_name(false),
            ),
            weekday(
                PRE,
                r"(?:(?:questo|questa|quest'|scorso|scorsa|prossimo|prossima)\s*)?",
                names::IT_WEEKDAYS,
                r"|weekend|fine\s*settimana",
                &format!(r"{CLOSE}(?:\s*(?:scorso|scorsa|prossimo|prossima)(?:\s*settimana)?)?"),
                POST,
            ),
            slash_month(),
            month_only(
                PRE,
                r"(?:a|in|di|del)\s*",
                names::IT_MONTHS,
                &format!(r"\s*(?:(?:,|-|del)?\s*(?:{year})?)?"),
                names::IT_FULL_MONTHS,
                true,
            ),
        ],
    }
}

/// Dutch.
fn dutch() -> Language {
    let m = keys(names::NL_MONTHS);
    let ord = format!("(?:{}|[0-9]{{1,2}}(?:ste|de)?)", keys(names::NL_ORDINALS));
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            month_only(
                PRE,
                "",
                names::NL_MONTHS,
                r"\s*(?:[,-]?\s*(?:[1-9][0-9]{0,3}\s*(?:voor Christus|na Christus)|[1-2][0-9]{3}|[5-9][0-9])?)?",
                &[],
                true,
            ),
            iso(),
            slash(true),
            with_year(
                &format!(
                    r"{PRE}(?:on\s*?)?(?P<d>{ord})(?:\s*(?:tot|-|–|until|through|till|\s)\s*(?P<d2>{ord}))?(?:-|/|\s*(?:of)?\s*)(?P<m>{m}){{Y}}(?:(?:-|/|,?\s*)(?P<y>[1-9][0-9]{{0,3}}\s*(?:voor Christus|na Christus)|[1-2][0-9]{{3}}|[5-9][0-9])){{/Y}}?{POST}"
                ),
                Kind::MonthName {
                    words: &NL_WORDS,
                    day_first: true,
                    skip_year_like: false,
                    year_check: YearCheck::NoGluedDigit,
                },
            ),
            weekday(
                PRE,
                r"(?:op\s*?)?(?:(?:deze|vorige|volgende)\s*(?:week\s*)?)?",
                names::NL_WEEKDAYS,
                "",
                "",
                POST,
            ),
            year_month_day(&NL_WORDS, r"[./\s]"),
            slash_month(),
        ],
    }
}

/// Swedish.
fn swedish() -> Language {
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            little_endian(
                PRE,
                r"(?:den\s*?)?(?P<d>[0-9]{1,2})(?:\s*(?:till|-|–|\s)\s*(?P<d2>[0-9]{1,2}))?\s*",
                &SV_WORDS,
                r"{Y}(?:(?:-|/|,?\s*)(?P<y>[0-9]{4})){/Y}?",
                POST,
            ),
            weekday(
                PRE,
                r"(?:på\s*?)?(?:(?:förra|senaste|nästa|kommande)\s*)?",
                names::SV_WEEKDAYS,
                "",
                &format!(r"{CLOSE}(?:\s*(?:förra|senaste|nästa|kommande)\s*vecka)?"),
                POST,
            ),
        ],
    }
}

/// Finnish.
fn finnish() -> Language {
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            little_endian(
                PRE,
                r"(?P<d>[0-9]{1,2})\.?(?:\s*(?:-|–|\s)\s*(?P<d2>[0-9]{1,2})\.?)?\s*",
                &FI_WORDS,
                r"{Y}(?:(?:-|/|,?\s*)(?P<y>[0-9]{4})){/Y}?",
                POST,
            ),
            weekday(
                PRE,
                r"(?:(?:viime|edellinen|edellisenä|ensi|seuraava|seuraavana|tämä|tänä)\s*)?",
                names::FI_WEEKDAYS,
                "",
                &format!(r"{CLOSE}(?:\s*(?:viime|ensi|seuraava)\s*viikolla)?"),
                POST,
            ),
        ],
    }
}

/// What Russian and Ukrainian differ in: their words, the preposition before
/// a date, the words and eras after a year, and the weekday modifiers.
struct EastSlavic {
    /// Weekday words.
    weekdays: &'static [(&'static str, u8)],
    /// Month and ordinal words.
    words: &'static Words,
    /// The preposition a date may open with ("с", "з").
    from: &'static str,
    /// The words that may follow a year ("году", "року").
    year_words: &'static str,
    /// The era marks a year may carry.
    era: &'static str,
    /// The modifiers before a weekday.
    before: &'static str,
    /// The modifiers after a weekday.
    after: String,
    /// The month words chrono counts as full names.
    full_months: &'static [&'static str],
    /// The preposition a month alone may follow ("в", "у").
    in_month: &'static str,
}

/// Russian and Ukrainian share their date shape.
fn east_slavic(spec: EastSlavic) -> Language {
    let EastSlavic {
        weekdays,
        words,
        from,
        year_words,
        era,
        before,
        after,
        full_months,
        in_month,
    } = spec;
    let ord = format!("(?:{}|[0-9]{{1,2}}(?:го|ого|е|ое)?)", keys(words.ordinals));
    let year = format!(
        r"[1-9][0-9]{{0,3}}(?:\s+(?:{year_words}))?\s*(?:{era})|[1-2][0-9]{{3}}(?:\s+(?:{year_words}))?|[5-9][0-9](?:\s+(?:{year_words}))?"
    );
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            month_only(
                PRE_U,
                &format!(r"(?:{in_month})\s*"),
                words.months,
                &format!(r"\s*(?:[,-]?\s*(?:{year})?)?"),
                full_months,
                false,
            ),
            iso(),
            slash(true),
            little_endian(
                PRE_U,
                &format!(
                    r"(?:{from})?\s*(?P<d>{ord})(?:\s{{0,3}}(?:по|-|–|до)?\s{{0,3}}(?P<d2>{ord}))?(?:-|/|\s{{0,3}}(?:of)?\s{{0,3}})"
                ),
                words,
                &format!(r"{{Y}}(?:(?:-|/|,?\s{{0,3}})(?P<y>{year})){{/Y}}?"),
                POST_U,
            ),
            weekday(PRE_U, before, weekdays, "", &after, POST_U),
        ],
    }
}

/// Vietnamese.
fn vietnamese() -> Language {
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            slash(true),
            plain(
                &format!(
                    r"{PRE}(?:ngày\s*)?(?P<d>[0-9]{{1,2}})\s*tháng\s*(?P<mn>[0-9]{{1,2}})(?:\s*năm\s*(?P<y>[0-9]{{1,4}}(?:\s*TCN)?))?{POST}"
                ),
                Kind::Vietnamese,
            ),
            plain(
                &format!(
                    r"{PRE}(?P<w>{})(?:\s*(?:này|tới|sau|qua))?{POST}",
                    keys(names::VI_WEEKDAYS)
                ),
                Kind::Weekday(names::VI_WEEKDAYS),
            ),
        ],
    }
}

/// Japanese.
fn japanese() -> Language {
    let digit = "[0-9０-９]";
    Language {
        merge: Merge::DateThenWeekday,
        parsers: vec![
            iso(),
            plain(
                &format!(
                    r"(?:(?:(?P<rel>[同今本])|(?P<era>昭和|平成|令和)?(?P<y>{digit}{{1,4}}|元))年\s*)?(?P<mn>{digit}{{1,2}})月\s*(?P<d>{digit}{{1,2}})日"
                ),
                Kind::Japanese,
            ),
            plain(
                r"(?:前の|次の|今週)?(?P<w>[日月火水木金土])(?:曜日|曜)",
                Kind::WeekdayChar(names::JA_WEEKDAYS),
            ),
            plain(
                r"(?:\(|（)(?P<w>[日月火水木金土])(?:\)|）)",
                Kind::WeekdayChar(names::JA_WEEKDAYS),
            ),
            plain(
                r"(?:(?P<y>[0-9０-９]{4})[/|／])?(?P<mn>[0-1０-１]?[0-9０-９])(?:[/|／](?P<d>[0-3０-３]?[0-9０-９]))",
                Kind::Japanese,
            ),
        ],
    }
}

/// Chinese, simplified and traditional together.
fn chinese() -> Language {
    let numeral = "零〇一二两兩三四五六七八九十廿卅";
    let weekday_words = "星期|礼拜|禮拜|周|週";
    Language {
        merge: Merge::WeekdayThenDate,
        parsers: vec![
            iso(),
            plain(
                &format!(
                    r"{PRE}(?P<y>\d{{2,4}}|[{numeral}]{{4}}|[{numeral}]{{2}})?(?:\s*)(?:年)?(?:[\s|,，]*)(?P<mn>\d{{1,2}}|[{numeral}]{{1,3}})(?:\s*)(?:月)(?:\s*)(?P<d>\d{{1,2}}|[{numeral}]{{1,3}})?(?:\s*)(?:日|号|號)?"
                ),
                Kind::Chinese,
            ),
            plain(
                &format!(
                    r"{PRE}(?:上|今|下|这|這|呢)(?:个|個)?(?:{weekday_words})[天日一二三四五六]"
                ),
                Kind::Other,
            ),
            plain(
                &format!(r"{PRE}(?:{weekday_words})(?P<w>[天日一二三四五六])"),
                Kind::WeekdayChar(names::ZH_WEEKDAYS),
            ),
        ],
    }
}

/// A digit as Arabic texts write them: Western, Arabic-Indic or the extended
/// Arabic-Indic of Persian and Urdu.
const AR_DIGIT: &str = "[0-9\u{660}-\u{669}\u{6F0}-\u{6F9}]";
/// Not such a digit.
const AR_NOT_DIGIT: &str = "[^0-9\u{660}-\u{669}\u{6F0}-\u{6F9}]";
/// The bidi marks a formatter puts around numbers in right-to-left text.
const AR_MARKS: &str = "[\u{200E}\u{200F}\u{61C}]*";

/// Arabic, which chrono does not read: a weekday with its article, after
/// `يوم` ("day") or the conjunction `و`, before a date with a month name or in
/// numbers. `الموافق` ("corresponding to") may stand between them, as formal
/// writing puts it. A Hijri date ("14 ربيع الأول 1448هـ") has no month this
/// reads, so a weekday beside one is left alone.
fn arabic() -> Language {
    let d = AR_DIGIT;
    let marks = AR_MARKS;
    let m = keys(names::AR_MONTHS);
    let modifiers = "القادم|القادمة|المقبل|المقبلة|الماضي|الماضية|المنصرم|المنصرمة|الفائت|الفائتة";
    Language {
        merge: Merge::ArabicWeekdayThenDate,
        parsers: vec![
            iso(),
            with_year(
                &format!(
                    r"{PRE_U}(?P<d>{d}{{1,2}}){marks}(?:\s*(?:-|–|إلى|الى|حتى)\s*(?P<d2>{d}{{1,2}}){marks})?\s*(?:من\s+)?(?:شهر\s+)?(?P<m>{m})(?:\s*[/(]\s*(?:{m})\s*\)?)?{{Y}}(?:{marks}\s*[,،]?\s*(?:(?:لعام|عام|لسنة|سنة)\s*)?(?P<y>{d}{{4}})(?:\s*م)?){{/Y}}?{POST_U}"
                ),
                Kind::Arabic,
            ),
            plain(
                &format!(
                    r"{PRE_U}(?P<y>{d}{{4}}){marks}[/.\-]{marks}(?P<mn>{d}{{1,2}}){marks}[/.\-]{marks}(?P<d>{d}{{1,2}}){POST_U}"
                ),
                Kind::Arabic,
            ),
            plain(
                &format!(
                    r"(?:(?P<pre>{AR_NOT_DIGIT})|^)(?P<a>{d}{{1,2}}){marks}[/.\-]{marks}(?P<b>{d}{{1,2}})(?:{marks}[/.\-]{marks}(?P<y>{d}{{4}}|{d}{{2}}))?{marks}(?P<post>[^\p{{L}}\p{{N}}_]|$)"
                ),
                Kind::ArabicSlash,
            ),
            weekday(
                PRE_U,
                r"(?:و)?(?:يوم\s+)?",
                names::AR_WEEKDAYS,
                "",
                &format!(
                    r"(?:\s+(?:{modifiers}))?{marks}(?:\s*[,،])?{marks}(?:\s*الموافق(?:\s*لـ?)?)?"
                ),
                POST_U,
            ),
        ],
    }
}

/// Spanish.
fn spanish() -> Language {
    iberian(names::ES_WEEKDAYS, &ES_WORDS, "este|esta|pasado|pr[oó]ximo")
}

/// Portuguese.
fn portuguese() -> Language {
    iberian(
        names::PT_WEEKDAYS,
        &PT_WORDS,
        "este|esta|passado|pr[oó]ximo",
    )
}

/// Russian.
fn russian() -> Language {
    east_slavic(EastSlavic {
        weekdays: names::RU_WEEKDAYS,
        words: &RU_WORDS,
        from: "с",
        year_words: "году|года|год|г|г.",
        era: "н.э.|до н.э.|н. э.|до н. э.",
        before: r"(?:в\s*?)?(?:(?:эту|этот|прошлый|прошлую|следующий|следующую|следующего)\s*)?",
        after: format!(r"{CLOSE}(?:\s*на\s*(?:этой|прошлой|следующей)\s*неделе)?"),
        full_months: names::RU_FULL_MONTHS,
        in_month: "в",
    })
}

/// Ukrainian.
fn ukrainian() -> Language {
    east_slavic(EastSlavic {
        weekdays: names::UK_WEEKDAYS,
        words: &UK_WORDS,
        from: "з|із",
        year_words: "року|рік|р|р.",
        era: "н.е.|до н.е.|н. е.|до н. е.",
        before: r"(?:в\s*?)?(?:у\s*?)?(?:(?:цей|минулого|минулий|попередній|попереднього|наступного|наступний|наступному)\s*)?",
        after: format!(r"{CLOSE}(?:\s*(?:на|у|в)\s*(?:цьому|минулому|наступному)\s*тижні)?"),
        full_months: names::UK_FULL_MONTHS,
        in_month: "в|у",
    })
}

/// A language the check may read: its gate, compiled up front, and its
/// parsers, compiled the first time a reply opens the gate.
pub struct Entry {
    /// The language's code, as chrono names it.
    pub code: &'static str,
    /// The gate's weekday names as one case-insensitive pattern.
    gate: Option<Regex>,
    /// The language's parsers.
    language: LazyLock<Language, fn() -> Language>,
}

/// Whether `word` is written in a script without spaces, so it is found
/// inside text rather than as a whole word.
fn unspaced(word: &str) -> bool {
    word.chars().any(is_unspaced_script)
}

impl Entry {
    /// An entry whose gate keeps, as the TypeScript gate does, a name of three
    /// letters or more or one written without spaces: a two-letter
    /// abbreviation ("to", "so", "di") is a word elsewhere.
    fn new(code: &'static str, names: &'static [&'static str], build: fn() -> Language) -> Self {
        let words = names
            .iter()
            .copied()
            .filter(|w| unspaced(w) || w.chars().count() >= 3);
        Self {
            code,
            gate: compile(&format!("(?:{})", alternation(words))),
            language: LazyLock::new(build),
        }
    }

    /// Whether `text` names one of the language's weekdays: as a whole word,
    /// or anywhere for a name written without spaces.
    #[must_use]
    pub fn gate_open(&self, text: &str) -> bool {
        let Some(re) = &self.gate else {
            return false;
        };
        re.find_iter(text)
            .any(|m| unspaced(m.as_str()) || !inside_word(text, m.start(), m.end()))
    }

    /// The language's parsers, compiled on first use.
    #[must_use]
    pub fn language(&self) -> &Language {
        &self.language
    }
}

/// Every language the check reads, in the order the TypeScript runs them.
pub static LANGUAGES: LazyLock<[Entry; 15]> = LazyLock::new(|| {
    [
        Entry::new("en", names::EN_GATE, english),
        Entry::new("fr", names::FR_GATE, french),
        Entry::new("de", names::DE_GATE, german),
        Entry::new("es", names::ES_GATE, spanish),
        Entry::new("it", names::IT_GATE, italian),
        Entry::new("pt", names::PT_GATE, portuguese),
        Entry::new("nl", names::NL_GATE, dutch),
        Entry::new("sv", names::SV_GATE, swedish),
        Entry::new("fi", names::FI_GATE, finnish),
        Entry::new("ru", names::RU_GATE, russian),
        Entry::new("uk", names::UK_GATE, ukrainian),
        Entry::new("ja", names::JA_GATE, japanese),
        Entry::new("zh", names::ZH_GATE, chinese),
        Entry::new("vi", names::VI_GATE, vietnamese),
        Entry::new("ar", names::AR_GATE, arabic),
    ]
});

/// The value of a digit written halfwidth or fullwidth.
fn wide_digit(c: char) -> Option<u32> {
    c.to_digit(10).or_else(|| {
        let offset = u32::from(c).checked_sub(u32::from('０'))?;
        (offset < 10).then_some(offset)
    })
}

/// A number written in halfwidth or fullwidth digits.
fn wide_number(text: &str) -> Option<u32> {
    text.chars()
        .try_fold(0u32, |n, c| n.checked_mul(10)?.checked_add(wide_digit(c)?))
}

/// A number written in digits or Chinese numerals, as chrono reads it.
fn chinese_number(text: &str) -> Option<u32> {
    if let Some(n) = wide_number(text) {
        return Some(n);
    }
    let value = |c: char| {
        names::ZH_NUMERALS
            .iter()
            .find(|&&(k, _)| k == c)
            .map(|&(_, v)| u32::from(v))
    };
    let mut total = 0u32;
    let mut current = 0u32;
    for c in text.chars() {
        match c {
            '十' => {
                total += current.max(1) * 10;
                current = 0;
            }
            '廿' => total += 20,
            '卅' => total += 30,
            _ => current = value(c)?,
        }
    }
    Some(total + current)
}

/// A year written as digits or Chinese numerals, digit by digit.
fn chinese_year(text: &str) -> Option<i32> {
    let digits = text.chars().try_fold(0i32, |n, c| {
        let d = wide_digit(c).or_else(|| {
            names::ZH_NUMERALS
                .iter()
                .find(|&&(k, _)| k == c)
                .map(|&(_, v)| u32::from(v))
                .filter(|&v| v < 10)
        })?;
        n.checked_mul(10)?.checked_add(i32::try_from(d).ok()?)
    })?;
    Some(likely_ad_year(digits))
}

/// chrono's reading of a year below 100: after 50 the 1900s, else the 2000s.
const fn likely_ad_year(year: i32) -> i32 {
    if year < 100 {
        if year > 50 { year + 1900 } else { year + 2000 }
    } else {
        year
    }
}

/// A written year, its era marks read: Buddhist years count from 543 BC, and
/// a year before Christ is negative.
fn parse_year(text: &str) -> Option<i32> {
    let digits = run_end(text.as_bytes(), 0, |b| b.is_ascii_digit());
    let n: i32 = text.get(..digits)?.parse().ok()?;
    let rest = text.get(digits..).unwrap_or_default().trim();
    if rest.is_empty() {
        return Some(likely_ad_year(n));
    }
    let before_christ = ["bc", "bce", "v", "a.c", "a. c", "ac", "до", "tcn", "voor"]
        .iter()
        .any(|mark| starts_with_lowercase(rest, mark));
    if starts_with_lowercase(rest, "be") {
        Some(n - 543)
    } else if before_christ && !starts_with_lowercase(rest, "ad") {
        Some(-n)
    } else {
        Some(n)
    }
}

/// The day a month-name date writes: digits with a suffix, or an ordinal word.
fn parse_day(text: &str, ordinals: &[(&str, u8)]) -> Option<u8> {
    let digits = run_end(text.as_bytes(), 0, |b| b.is_ascii_digit());
    if digits > 0 {
        return text.get(..digits)?.parse().ok();
    }
    lookup(ordinals, text)
}

/// Whether the year check holds on what follows a written year.
fn year_follows_well(check: YearCheck, year: &str, after: &str) -> bool {
    let mut next = after.chars();
    let first = next.next();
    let word = |c: Option<char>| c.is_some_and(is_ascii_word);
    match check {
        YearCheck::NotWord => !word(first),
        YearCheck::NoGluedDigit => {
            !(first.is_some_and(|c| !c.is_whitespace())
                && next.next().is_some_and(|c| c.is_ascii_digit()))
        }
        YearCheck::EnglishYear => {
            if word(first) {
                return false;
            }
            let two_digit = year.len() == 2 && year.bytes().all(|b| b.is_ascii_digit());
            if !two_digit {
                return true;
            }
            let time_or_month = after.starts_with(':')
                && after.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
                || after.starts_with(char::is_whitespace)
                    && [
                        "am", "pm", "o clock", "oclock", "jan", "feb", "mar", "apr", "may", "jun",
                        "jul", "aug", "sep", "oct", "nov", "dec",
                    ]
                    .iter()
                    .any(|w| starts_with_lowercase(after.trim_start(), w));
            !time_or_month
        }
    }
}

impl Language {
    /// Every result of every parser over `text`, sorted by position, overlaps
    /// removed as chrono removes them: of two that overlap, the longer stays.
    #[must_use]
    pub fn results(&self, text: &str) -> Vec<Found> {
        let mut all: Vec<Found> = Vec::new();
        for parser in &self.parsers {
            run(parser, text, &mut all);
        }
        all.sort_by_key(|f| f.start);
        let mut kept: Vec<Found> = Vec::with_capacity(all.len());
        let mut iter = all.into_iter();
        let Some(mut prev) = iter.next() else {
            return kept;
        };
        for found in iter {
            if found.start >= prev.end {
                kept.push(prev);
                prev = found;
            } else if found.end - found.start > prev.end - prev.start {
                prev = found;
            }
        }
        kept.push(prev);
        kept
    }
}

/// The next character boundary after byte `at`, or one past the end.
const fn next_boundary(text: &str, at: usize) -> usize {
    let next = at.saturating_add(1);
    if next > text.len() {
        next
    } else {
        text.ceil_char_boundary(next)
    }
}

/// Where, in the searched text, chrono resumes after rejecting a match, less
/// one character: where the result's text would start (after the left
/// boundary the base parser strips; the slash parser's own leading character
/// stays), and past the day group when a day-first date's day exceeded 31.
fn retry_offset(parser: &Parser, caps: &Captures<'_>) -> usize {
    let Some(whole) = caps.get(0) else {
        return 0;
    };
    let header = if matches!(parser.kind, Kind::Slash { .. } | Kind::ArabicSlash) {
        None
    } else {
        caps.name("pre")
    };
    let inner_start = header.map_or_else(|| whole.start(), |pre| pre.end());
    let skipped_day = if let Kind::MonthName {
        words,
        day_first: true,
        ..
    } = parser.kind
    {
        caps.name("d")
            .filter(|d| parse_day(d.as_str(), words.ordinals).is_some_and(|n| n > 31))
            .map_or(0, |d| d.len())
    } else {
        0
    };
    inner_start + skipped_day
}

/// Runs one parser over `text` the way chrono does: a match it rejects is
/// retried one character later, and after a result the search resumes past
/// it, where the pattern's `^` matches again.
fn run(parser: &Parser, text: &str, out: &mut Vec<Found>) {
    let Some(re) = &parser.re else {
        return;
    };
    let mut from = 0;
    while let Some(rest) = text.get(from..) {
        let Some(caps) = re.captures(rest) else {
            break;
        };
        let Some(whole) = caps.get(0) else {
            break;
        };
        let read = extract(parser, &caps, text, from).or_else(|| {
            // Back out of the year chrono would not have taken.
            let no_year = parser.no_year.as_ref()?;
            let without = no_year.captures_at(rest, whole.start())?;
            (without.get(0)?.start() == whole.start()).then_some(())?;
            extract(parser, &without, text, from)
        });
        match read {
            Some(found) => {
                out.push(found);
                from = if found.end > from {
                    found.end
                } else {
                    next_boundary(text, from)
                };
            }
            None => from = next_boundary(text, from + retry_offset(parser, &caps)),
        }
    }
}

/// One match, with the haystack it came from.
struct Match<'c, 't> {
    /// The pattern's groups, offsets from `from`.
    caps: &'c Captures<'t>,
    /// The whole text.
    text: &'t str,
    /// Where the search started in `text`.
    from: usize,
}

impl<'t> Match<'_, 't> {
    /// A group's text.
    fn group(&self, name: &str) -> Option<&'t str> {
        self.caps.name(name).map(|m| m.as_str())
    }

    /// What follows a group, to the end of the text.
    fn after(&self, name: &str) -> &'t str {
        self.caps
            .name(name)
            .and_then(|m| self.text.get(self.from.saturating_add(m.end())..))
            .unwrap_or_default()
    }
}

/// A written year, or the nearest one.
fn written_or_nearest(year: Option<i32>) -> YearOf {
    year.map_or(YearOf::Nearest, YearOf::Written)
}

/// A month-name date, or None where chrono's parser rejects it.
fn read_month_name(
    m: &Match<'_, '_>,
    words: &Words,
    day_first: bool,
    skip_year_like: bool,
    year_check: YearCheck,
) -> Option<Reading> {
    let day_text = m.group("d")?;
    let day = parse_day(day_text, words.ordinals).filter(|d| (1..=31).contains(d))?;
    // Month first, a day followed by `am` or `pm` is a time.
    let after_day = m.after("d").trim_start();
    if !day_first
        && ["am", "pm"]
            .iter()
            .any(|w| starts_with_lowercase(after_day, w))
    {
        return None;
    }
    let month = lookup(words.months, m.group("m")?)?;
    let year = match m.group("y") {
        Some(y) if !year_follows_well(year_check, y.trim(), m.after("y")) => return None,
        Some(y) => Some(parse_year(y)?),
        None => None,
    };
    // "Gennaio 21": a day from 20 to 25 with no year may be the year.
    let year_like = matches!(day_text.as_bytes(), [b'2', b'0'..=b'5']);
    if skip_year_like && m.group("d2").is_none() && year.is_none() && year_like {
        return None;
    }
    Some(Reading::Date {
        day,
        month,
        year: written_or_nearest(year),
    })
}

/// A month and a day that may have been written the other way round: a
/// month past 12 is swapped with a day that can be a month.
fn ordered(mut month: u8, mut day: u8) -> Option<(u8, u8)> {
    if month > 12 {
        if (1..=12).contains(&day) && month <= 31 {
            std::mem::swap(&mut month, &mut day);
        } else {
            return None;
        }
    }
    (month >= 1 && (1..=31).contains(&day)).then_some((month, day))
}

/// A year, a month word or number, and a day.
fn read_year_month_day(m: &Match<'_, '_>, words: &Words) -> Option<Reading> {
    let month = match (m.group("mn"), m.group("m")) {
        (Some(n), _) => n.parse().ok()?,
        (None, Some(w)) => lookup(words.months, w)?,
        (None, None) => return None,
    };
    let (month, day) = ordered(month, m.group("d")?.parse().ok()?)?;
    Some(Reading::Date {
        day,
        month,
        year: YearOf::Written(m.group("y")?.parse().ok()?),
    })
}

/// Whether `written` reads as a version number (`1.12`, `1.12.12`) rather than a date.
fn version_like(written: &str) -> bool {
    let parts: Vec<&str> = written.trim_end().split('.').collect();
    parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && match parts.as_slice() {
            [x, y] => x.len() == 1 && y.len() == 1,
            [x, y, z] => x.len() == 1 && y.len() <= 2 && z.len() <= 2,
            _ => false,
        }
}

/// A slash date, with its own range: chrono's slash parser takes its text
/// from after the character before the first number.
fn read_slash(m: &Match<'_, '_>, end: usize, day_first: bool) -> Option<Found> {
    let a = m.caps.name("a")?;
    let index = m.from + a.start();
    let mut back = m.text.get(..index).unwrap_or_default().chars().rev();
    let last = back.next();
    let glued_before = last.is_some_and(|c| c.is_ascii_digit())
        || (last == Some('/') && back.next().is_some_and(|c| c.is_ascii_digit()));
    let after = m.text.get(end..).unwrap_or_default();
    let glued_after = after
        .strip_prefix('/')
        .unwrap_or(after)
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit());
    let written = m.text.get(index..end).unwrap_or_default();
    let year = m.group("y");
    if glued_before
        || glued_after
        || version_like(written)
        || (year.is_none() && !written.contains('/'))
    {
        return None;
    }
    let (first, second): (u8, u8) = (a.as_str().parse().ok()?, m.group("b")?.parse().ok()?);
    let (month, day) = if day_first {
        (second, first)
    } else {
        (first, second)
    };
    let (month, day) = ordered(month, day)?;
    let year = match year {
        Some(y) => YearOf::Written(likely_ad_year(y.parse().ok()?)),
        None => YearOf::Nearest,
    };
    Some(Found {
        start: index,
        end,
        reading: Reading::Date { day, month, year },
    })
}

/// A number in any decimal digits, Arabic-Indic ones included.
fn any_digits(text: &str) -> Option<u32> {
    text.chars().try_fold(0u32, |n, c| {
        n.checked_mul(10)?.checked_add(u32::from(digit_value(c)?))
    })
}

/// An Arabic date: a day, a month word or number, and the year if written.
fn read_arabic(m: &Match<'_, '_>) -> Option<Reading> {
    let day = u8::try_from(any_digits(m.group("d")?)?).ok()?;
    let month = match (m.group("mn"), m.group("m")) {
        (Some(n), _) => u8::try_from(any_digits(n)?).ok()?,
        (None, Some(w)) => lookup(names::AR_MONTHS, w)?,
        (None, None) => return None,
    };
    let (month, day) = ordered(month, day)?;
    let year = match m.group("y") {
        Some(y) => Some(i32::try_from(any_digits(y)?).ok()?),
        None => None,
    };
    Some(Reading::Date {
        day,
        month,
        year: written_or_nearest(year),
    })
}

/// An Arabic date in numbers, day first, under the slash parser's rules: no
/// digit glued on either side, not a version number, and a year unless the
/// numbers are joined by `/`.
fn read_arabic_slash(m: &Match<'_, '_>, end: usize) -> Option<Found> {
    let is_digit = |c: char| digit_value(c).is_some();
    let a = m.caps.name("a")?;
    let index = m.from + a.start();
    let mut back = m.text.get(..index).unwrap_or_default().chars().rev();
    let last = back.next();
    let glued_before =
        last.is_some_and(is_digit) || (last == Some('/') && back.next().is_some_and(is_digit));
    let after = m.text.get(end..).unwrap_or_default();
    let glued_after = after
        .strip_prefix('/')
        .unwrap_or(after)
        .chars()
        .next()
        .is_some_and(is_digit);
    let written = m.text.get(index..end).unwrap_or_default();
    let ascii: String = written
        .chars()
        .filter(|&c| !is_bidi_mark(c))
        .map(|c| digit_value(c).map_or(c, |d| char::from(b'0'.saturating_add(d))))
        .collect();
    let year = m.group("y");
    if glued_before
        || glued_after
        || version_like(&ascii)
        || (year.is_none() && !written.contains('/'))
    {
        return None;
    }
    let first = u8::try_from(any_digits(a.as_str())?).ok()?;
    let second = u8::try_from(any_digits(m.group("b")?)?).ok()?;
    let (month, day) = ordered(second, first)?;
    let year = match year {
        Some(y) => YearOf::Written(likely_ad_year(i32::try_from(any_digits(y)?).ok()?)),
        None => YearOf::Nearest,
    };
    Some(Found {
        start: index,
        end,
        reading: Reading::Date { day, month, year },
    })
}

/// Whether `c` is a bidi mark a formatter writes around numbers in
/// right-to-left text: left-to-right, right-to-left or the Arabic letter
/// mark.
pub const fn is_bidi_mark(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{61C}')
}

/// A Japanese date: a day and month in halfwidth or fullwidth digits, the
/// year in an era (令和 counts from 2019, 平成 from 1989, 昭和 from 1926, 元
/// its first year), or this year (同, 今, 本).
fn read_wide(m: &Match<'_, '_>) -> Option<Reading> {
    let era_offset = match m.group("era") {
        Some("令和") => 2018,
        Some("平成") => 1988,
        Some("昭和") => 1925,
        _ => 0,
    };
    let year = if m.group("rel").is_some() {
        YearOf::Current
    } else {
        match m.group("y") {
            Some("元") => YearOf::Written(1 + era_offset),
            Some(y) => YearOf::Written(i32::try_from(wide_number(y)?).ok()? + era_offset),
            None => YearOf::Nearest,
        }
    };
    Some(Reading::Date {
        day: u8::try_from(wide_number(m.group("d")?)?).ok()?,
        month: u8::try_from(wide_number(m.group("mn")?)?).ok()?,
        year,
    })
}

/// A Chinese date; one without a day is no date to check.
fn read_chinese(m: &Match<'_, '_>) -> Option<Reading> {
    let Some(day) = m.group("d") else {
        return Some(Reading::Other);
    };
    Some(Reading::Date {
        day: u8::try_from(chinese_number(day)?).ok()?,
        month: u8::try_from(chinese_number(m.group("mn")?)?).ok()?,
        year: match m.group("y") {
            Some(y) => YearOf::Written(chinese_year(y)?),
            None => YearOf::Current,
        },
    })
}

/// The reading of one match, with the result's absolute byte range; None
/// where chrono's parser rejects it.
fn extract(parser: &Parser, caps: &Captures<'_>, text: &str, from: usize) -> Option<Found> {
    let whole = caps.get(0)?;
    let start = from + caps.name("pre").map_or_else(|| whole.start(), |m| m.end());
    let end = from + caps.name("post").map_or_else(|| whole.end(), |m| m.start());
    let m = Match { caps, text, from };
    let reading = match parser.kind {
        Kind::Weekday(table) => {
            lookup(table, m.group("w")?).map_or(Reading::Other, Reading::Weekday)
        }
        Kind::WeekdayChar(table) => {
            let c = m.group("w")?.chars().next()?;
            table
                .iter()
                .find(|&&(k, _)| k == c)
                .map_or(Reading::Other, |&(_, day)| Reading::Weekday(day))
        }
        Kind::Other => Reading::Other,
        Kind::MonthOnly { full, drop_prefix } => {
            let prefix_end = caps.name("prefix").map(|p| from + p.end());
            let text_start = if drop_prefix {
                prefix_end.unwrap_or(start)
            } else {
                start
            };
            let inner = text.get(start..end).unwrap_or_default();
            let word = m.group("m")?;
            if inner.chars().count() <= 3 && !full.iter().any(|f| same_word(f, word)) {
                return None;
            }
            return Some(Found {
                start: text_start,
                end,
                reading: Reading::Other,
            });
        }
        Kind::MonthName {
            words,
            day_first,
            skip_year_like,
            year_check,
        } => read_month_name(&m, words, day_first, skip_year_like, year_check)?,
        Kind::YearMonthDay(words) => read_year_month_day(&m, words)?,
        Kind::Slash { day_first } => return read_slash(&m, end, day_first),
        Kind::Iso => Reading::Date {
            day: m.group("d")?.parse().ok()?,
            month: m.group("mn")?.parse().ok()?,
            year: YearOf::Written(m.group("y")?.parse().ok()?),
        },
        Kind::Vietnamese => Reading::Date {
            day: m.group("d")?.parse().ok()?,
            month: m.group("mn")?.parse().ok()?,
            year: written_or_nearest(m.group("y").and_then(parse_year)),
        },
        Kind::Japanese => read_wide(&m)?,
        Kind::Chinese => read_chinese(&m)?,
        Kind::Arabic => read_arabic(&m)?,
        Kind::ArabicSlash => return read_arabic_slash(&m, end),
    };
    Some(Found {
        start,
        end,
        reading,
    })
}
