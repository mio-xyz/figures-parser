//! Lengths of time from now, as chrono-node 2.10.2's time-unit parsers read
//! them (MIT): "in 2 hours", "for an hour", "3 days ago", "next 2 weeks",
//! "il y a une heure", "vor 2 Stunden", "через час", "两小时内". A length
//! that holds hours, minutes or seconds, or a fraction of a day that rounds
//! to some, sets an hour; a length of days, weeks, months or years sets
//! only a date.
//!
//! Each reads at the very start of a text; the caller decides where a
//! parser may start.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::Time;
use crate::text::alternation;

/// A unit a length is counted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    /// Years.
    Year,
    /// Quarters.
    Quarter,
    /// Months.
    Month,
    /// Weeks.
    Week,
    /// Days.
    Day,
    /// Hours.
    Hour,
    /// Minutes.
    Minute,
    /// Seconds.
    Second,
}

impl Unit {
    /// Its slot in a [`Duration`].
    const fn slot(self) -> usize {
        match self {
            Self::Year => 0,
            Self::Quarter => 1,
            Self::Month => 2,
            Self::Week => 3,
            Self::Day => 4,
            Self::Hour => 5,
            Self::Minute => 6,
            Self::Second => 7,
        }
    }
}

/// A length as chrono holds it: a value per unit the text names.
#[derive(Debug, Clone, Copy, Default)]
struct Duration([Option<f64>; 8]);

impl Duration {
    /// The duration with `value` in `unit`, the last one written winning.
    fn set(mut self, unit: Unit, value: f64) -> Self {
        if let Some(slot) = self.0.get_mut(unit.slot()) {
            *slot = Some(value);
        }
        self
    }

    /// Every value negated, as a length into the past.
    fn reversed(self) -> Self {
        Self(self.0.map(|v| v.map(|x| -x)))
    }

    /// What the length sets, as chrono's `addDuration` carries a fraction
    /// down to the next unit and `createRelativeFromReference` then reads
    /// the units present: whether it sets an hour, whether it sets a date,
    /// and whether its values make a date at all.
    #[expect(
        clippy::suboptimal_flops,
        reason = "JavaScript rounds the product before it adds the half"
    )]
    fn reading(self) -> (bool, bool, bool) {
        let [
            year,
            quarter,
            mut month,
            mut week,
            mut day,
            mut hour,
            minute,
            second,
        ] = self.0;
        let fraction = |v: f64| v - v.floor();
        let add = |slot: Option<f64>, extra: f64| Some(slot.unwrap_or(0.0) + extra);
        if let Some(y) = year.filter(|&y| fraction(y) > 0.0) {
            month = add(month, fraction(y) * 12.0);
        }
        if let Some(q) = quarter.filter(|&q| fraction(q) > 0.0) {
            month = add(month, fraction(q) * 3.0);
        }
        if let Some(m) = month.filter(|&m| fraction(m) > 0.0) {
            week = add(week, fraction(m) * 4.0);
        }
        if let Some(w) = week.filter(|&w| fraction(w) > 0.0) {
            day = add(day, (fraction(w) * 7.0 + 0.5).floor());
        }
        if let Some(d) = day.filter(|&d| fraction(d) > 0.0) {
            hour = add(hour, (fraction(d) * 24.0 + 0.5).floor());
        }
        let values = [year, quarter, month, week, day, hour, minute, second];
        // A JavaScript date spans 8.64e15 ms either side of 1970.
        let millis = [3.156e10, 7.89e9, 2.63e9, 6.048e8, 8.64e7, 3.6e6, 6e4, 1e3];
        let span: f64 = values
            .iter()
            .zip(millis)
            .map(|(v, ms)| v.map_or(0.0, |v| (v * ms).abs()))
            .sum();
        let valid = values.iter().flatten().all(|v| v.is_finite()) && span < 8.64e15;
        let sets_hour = hour.is_some() || minute.is_some() || second.is_some();
        let sets_date =
            sets_hour || year.is_some() || month.is_some() || week.is_some() || day.is_some();
        (sets_hour, sets_date, valid)
    }
}

/// How a language's number words read beyond its integer words.
type Special = fn(&str) -> Option<f64>;

/// One language's lengths: its numbers and units, and how a run of them is
/// read back.
struct Lexicon {
    /// The integer words and their values.
    words: &'static [(&'static str, u32)],
    /// The number pattern after the integer words, in chrono's order.
    numbers: &'static str,
    /// The unit words and their units.
    units: &'static [(&'static str, Unit)],
    /// What separates a number from its unit.
    gap: &'static str,
    /// What follows a unit inside one length.
    tail: &'static str,
    /// What may open a run of lengths.
    prefix: &'static str,
    /// What joins two lengths of a run.
    connector: &'static str,
    /// Whether a length written in letters only ("am") is skipped (English, Italian).
    skip_letters: bool,
    /// Whether the rest is trimmed after each length is read.
    trim: bool,
    /// Whether a run that reads no length is no result (English, Italian).
    none_when_empty: bool,
    /// Number words other than the integer words, and how they read.
    special: Special,
}

/// What a number written in the language's number pattern is worth:
/// its integer word, then a special word, then JavaScript's `parseFloat`.
fn number_value(lexicon: &Lexicon, text: &str) -> Option<f64> {
    let lower = text.to_lowercase();
    if let Some(&(_, v)) = lexicon.words.iter().find(|&&(w, _)| w == lower) {
        return Some(f64::from(v));
    }
    (lexicon.special)(&lower).or_else(|| Some(parse_float(&lower)))
}

/// JavaScript's `parseFloat`: the longest decimal prefix after leading
/// whitespace, NaN when there is none.
fn parse_float(text: &str) -> f64 {
    let text = text.trim_start();
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let whole = digits(text);
    let fraction = text
        .get(whole..)
        .and_then(|rest| rest.strip_prefix('.'))
        .map_or(0, digits);
    let end = if fraction > 0 {
        whole.saturating_add(1).saturating_add(fraction)
    } else {
        whole
    };
    text.get(..end)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::NAN)
}

/// Whether `text` holds `word`, as chrono's `match` tests a number word.
fn has(text: &str, word: &str) -> bool {
    text.contains(word)
}

/// The special number words of each language.
fn english(n: &str) -> Option<f64> {
    if matches!(n, "a" | "an" | "the") {
        Some(1.0)
    } else if has(n, "few") {
        Some(3.0)
    } else if has(n, "half") {
        Some(0.5)
    } else if has(n, "couple") {
        Some(2.0)
    } else if has(n, "several") {
        Some(7.0)
    } else {
        None
    }
}

/// French's special number words.
fn french(n: &str) -> Option<f64> {
    if matches!(n, "une" | "un") {
        Some(1.0)
    } else if has(n, "quelque") {
        Some(3.0)
    } else if has(n, "demi") {
        Some(0.5)
    } else {
        None
    }
}

/// German's special number words.
fn german(n: &str) -> Option<f64> {
    if matches!(n, "ein" | "einer" | "einem" | "einen" | "eine") {
        Some(1.0)
    } else if has(n, "wenigen") {
        Some(2.0)
    } else if has(n, "halb") {
        Some(0.5)
    } else if has(n, "einigen") {
        Some(3.0)
    } else if has(n, "mehreren") {
        Some(7.0)
    } else {
        None
    }
}

/// Spanish's special number words.
fn spanish(n: &str) -> Option<f64> {
    if matches!(n, "un" | "una" | "uno") {
        Some(1.0)
    } else if has(n, "algun") || n.starts_with("uno") {
        Some(3.0)
    } else if n.contains("medi") {
        Some(0.5)
    } else {
        None
    }
}

/// Italian's special number words.
fn italian(n: &str) -> Option<f64> {
    if matches!(n, "un" | "un'" | "una" | "uno") {
        Some(1.0)
    } else if has(n, "qualche") {
        Some(3.0)
    } else if has(n, "mezz") {
        Some(0.5)
    } else if has(n, "paio") {
        Some(2.0)
    } else if has(n, "alcuni") {
        Some(7.0)
    } else {
        None
    }
}

/// Dutch's special number words, and its decimal comma.
#[expect(
    clippy::unnecessary_wraps,
    reason = "every language's special number reader shares one signature"
)]
fn dutch(n: &str) -> Option<f64> {
    if n == "paar" {
        Some(2.0)
    } else if n == "half" || has(n, "halv") {
        Some(0.5)
    } else {
        Some(parse_float(&n.replacen(',', ".", 1)))
    }
}

/// JavaScript's `parseInt` on a number pattern of digits (Swedish, Finnish).
#[expect(
    clippy::unnecessary_wraps,
    reason = "every language's special number reader shares one signature"
)]
fn integer(n: &str) -> Option<f64> {
    let digits = n.bytes().take_while(u8::is_ascii_digit).count();
    Some(
        n.get(..digits)
            .and_then(|d| d.parse().ok())
            .unwrap_or(f64::NAN),
    )
}

/// Russian's special number words; an empty number is one.
fn russian(n: &str) -> Option<f64> {
    if has(n, "несколько") {
        Some(3.0)
    } else if has(n, "пол") {
        Some(0.5)
    } else if has(n, "пар") {
        Some(2.0)
    } else if n.is_empty() {
        Some(1.0)
    } else {
        None
    }
}

/// Ukrainian's special number words; an empty number is one.
fn ukrainian(n: &str) -> Option<f64> {
    if has(n, "декілька") || has(n, "пар") {
        Some(2.0)
    } else if has(n, "пів") {
        Some(0.5)
    } else if n.is_empty() {
        Some(1.0)
    } else {
        None
    }
}

/// No special number words.
const fn none(_: &str) -> Option<f64> {
    None
}

/// The connector most languages join lengths with.
const CONNECTOR: &str = r"\s{0,5},?\s{0,5}";

/// English's lengths.
static EN: Lexicon = Lexicon {
    words: &[
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|half(?:\s{0,2}an?)?|an?(?-u:\b)(?:\s{0,2}few)?|few|several|the|a?\s{0,2}couple\s{0,2}(?:of)?",
    units: &[
        ("s", Unit::Second),
        ("sec", Unit::Second),
        ("second", Unit::Second),
        ("seconds", Unit::Second),
        ("m", Unit::Minute),
        ("min", Unit::Minute),
        ("mins", Unit::Minute),
        ("minute", Unit::Minute),
        ("minutes", Unit::Minute),
        ("h", Unit::Hour),
        ("hr", Unit::Hour),
        ("hrs", Unit::Hour),
        ("hour", Unit::Hour),
        ("hours", Unit::Hour),
        ("d", Unit::Day),
        ("day", Unit::Day),
        ("days", Unit::Day),
        ("w", Unit::Week),
        ("week", Unit::Week),
        ("weeks", Unit::Week),
        ("mo", Unit::Month),
        ("mon", Unit::Month),
        ("mos", Unit::Month),
        ("month", Unit::Month),
        ("months", Unit::Month),
        ("qtr", Unit::Quarter),
        ("quarter", Unit::Quarter),
        ("quarters", Unit::Quarter),
        ("y", Unit::Year),
        ("yr", Unit::Year),
        ("year", Unit::Year),
        ("years", Unit::Year),
    ],
    gap: r"\s{0,3}",
    tail: "",
    prefix: r"(?:(?:about|around)\s{0,3})?",
    connector: r"\s{0,5},?(?:\s*and)?\s{0,5}",
    skip_letters: true,
    trim: true,
    none_when_empty: true,
    special: english,
};

/// French's lengths.
static FR: Lexicon = Lexicon {
    words: &[
        ("un", 1),
        ("deux", 2),
        ("trois", 3),
        ("quatre", 4),
        ("cinq", 5),
        ("six", 6),
        ("sept", 7),
        ("huit", 8),
        ("neuf", 9),
        ("dix", 10),
        ("onze", 11),
        ("douze", 12),
        ("treize", 13),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|une?(?-u:\b)|quelques?|demi-?",
    units: &[
        ("sec", Unit::Second),
        ("seconde", Unit::Second),
        ("secondes", Unit::Second),
        ("min", Unit::Minute),
        ("mins", Unit::Minute),
        ("minute", Unit::Minute),
        ("minutes", Unit::Minute),
        ("h", Unit::Hour),
        ("hr", Unit::Hour),
        ("hrs", Unit::Hour),
        ("heure", Unit::Hour),
        ("heures", Unit::Hour),
        ("jour", Unit::Day),
        ("jours", Unit::Day),
        ("semaine", Unit::Week),
        ("semaines", Unit::Week),
        ("mois", Unit::Month),
        ("trimestre", Unit::Quarter),
        ("trimestres", Unit::Quarter),
        ("ans", Unit::Year),
        ("année", Unit::Year),
        ("années", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: french,
};

/// German's lengths.
static DE: Lexicon = Lexicon {
    words: &[
        ("eins", 1),
        ("eine", 1),
        ("einem", 1),
        ("einen", 1),
        ("einer", 1),
        ("zwei", 2),
        ("drei", 3),
        ("vier", 4),
        ("fünf", 5),
        ("fuenf", 5),
        ("sechs", 6),
        ("sieben", 7),
        ("acht", 8),
        ("neun", 9),
        ("zehn", 10),
        ("elf", 11),
        ("zwölf", 12),
        ("zwoelf", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|halb?|halbe?|einigen?|wenigen?|mehreren?",
    units: &[
        ("sek", Unit::Second),
        ("sekunde", Unit::Second),
        ("sekunden", Unit::Second),
        ("min", Unit::Minute),
        ("minute", Unit::Minute),
        ("minuten", Unit::Minute),
        ("h", Unit::Hour),
        ("std", Unit::Hour),
        ("stunde", Unit::Hour),
        ("stunden", Unit::Hour),
        ("tag", Unit::Day),
        ("tage", Unit::Day),
        ("tagen", Unit::Day),
        ("woche", Unit::Week),
        ("wochen", Unit::Week),
        ("monat", Unit::Month),
        ("monate", Unit::Month),
        ("monaten", Unit::Month),
        ("monats", Unit::Month),
        ("quartal", Unit::Quarter),
        ("quartals", Unit::Quarter),
        ("quartale", Unit::Quarter),
        ("quartalen", Unit::Quarter),
        ("a", Unit::Year),
        ("j", Unit::Year),
        ("jr", Unit::Year),
        ("jahr", Unit::Year),
        ("jahre", Unit::Year),
        ("jahren", Unit::Year),
        ("jahres", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: german,
};

/// Spanish's lengths.
static ES: Lexicon = Lexicon {
    words: &[
        ("uno", 1),
        ("dos", 2),
        ("tres", 3),
        ("cuatro", 4),
        ("cinco", 5),
        ("seis", 6),
        ("siete", 7),
        ("ocho", 8),
        ("nueve", 9),
        ("diez", 10),
        ("once", 11),
        ("doce", 12),
        ("trece", 13),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|un?|uno?|una?|algunos?|unos?|demi-?",
    units: &[
        ("sec", Unit::Second),
        ("segundo", Unit::Second),
        ("segundos", Unit::Second),
        ("min", Unit::Minute),
        ("mins", Unit::Minute),
        ("minuto", Unit::Minute),
        ("minutos", Unit::Minute),
        ("h", Unit::Hour),
        ("hr", Unit::Hour),
        ("hrs", Unit::Hour),
        ("hora", Unit::Hour),
        ("horas", Unit::Hour),
        ("día", Unit::Day),
        ("días", Unit::Day),
        ("semana", Unit::Week),
        ("semanas", Unit::Week),
        ("mes", Unit::Month),
        ("meses", Unit::Month),
        ("cuarto", Unit::Quarter),
        ("cuartos", Unit::Quarter),
        ("año", Unit::Year),
        ("años", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: spanish,
};

/// Italian's lengths.
static IT: Lexicon = Lexicon {
    words: &[
        ("uno", 1),
        ("una", 1),
        ("un", 1),
        ("due", 2),
        ("tre", 3),
        ("quattro", 4),
        ("cinque", 5),
        ("sei", 6),
        ("sette", 7),
        ("otto", 8),
        ("nove", 9),
        ("dieci", 10),
        ("undici", 11),
        ("dodici", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|mezz[oa]?(?:\s{0,2})?|un(?:'|\s{0,2})?|qualche|alcuni|paio\s{0,2}(?:di)?",
    units: &[
        ("s", Unit::Second),
        ("sec", Unit::Second),
        ("secondo", Unit::Second),
        ("secondi", Unit::Second),
        ("m", Unit::Minute),
        ("min", Unit::Minute),
        ("minuto", Unit::Minute),
        ("minuti", Unit::Minute),
        ("h", Unit::Hour),
        ("ora", Unit::Hour),
        ("ore", Unit::Hour),
        ("g", Unit::Day),
        ("gg", Unit::Day),
        ("giorno", Unit::Day),
        ("giorni", Unit::Day),
        ("sett", Unit::Week),
        ("settimana", Unit::Week),
        ("settimane", Unit::Week),
        ("mese", Unit::Month),
        ("mesi", Unit::Month),
        ("trim", Unit::Quarter),
        ("trimestre", Unit::Quarter),
        ("trimestri", Unit::Quarter),
        ("anno", Unit::Year),
        ("anni", Unit::Year),
    ],
    gap: r"\s{0,3}",
    tail: "",
    prefix: r"(?:(?:circa|approssimativamente)\s{0,3})?",
    connector: r"\s{0,5},?(?:\s*e)?\s{0,5}",
    skip_letters: true,
    trim: true,
    none_when_empty: true,
    special: italian,
};

/// Dutch's lengths.
static NL: Lexicon = Lexicon {
    words: &[
        ("een", 1),
        ("twee", 2),
        ("drie", 3),
        ("vier", 4),
        ("vijf", 5),
        ("zes", 6),
        ("zeven", 7),
        ("acht", 8),
        ("negen", 9),
        ("tien", 10),
        ("elf", 11),
        ("twaalf", 12),
    ],
    numbers: r"[0-9]+|[0-9]+[\.,][0-9]+|halve?|half|paar",
    units: &[
        ("sec", Unit::Second),
        ("second", Unit::Second),
        ("seconden", Unit::Second),
        ("min", Unit::Minute),
        ("mins", Unit::Minute),
        ("minute", Unit::Minute),
        ("minuut", Unit::Minute),
        ("minuten", Unit::Minute),
        ("minuutje", Unit::Minute),
        ("h", Unit::Hour),
        ("hr", Unit::Hour),
        ("hrs", Unit::Hour),
        ("uur", Unit::Hour),
        ("u", Unit::Hour),
        ("uren", Unit::Hour),
        ("dag", Unit::Day),
        ("dagen", Unit::Day),
        ("week", Unit::Week),
        ("weken", Unit::Week),
        ("maand", Unit::Month),
        ("maanden", Unit::Month),
        ("jaar", Unit::Year),
        ("jr", Unit::Year),
        ("jaren", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: r"(?:(?:binnen|in)\s*)?",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: dutch,
};

/// Swedish's lengths.
static SV: Lexicon = Lexicon {
    words: &[
        ("en", 1),
        ("ett", 1),
        ("två", 2),
        ("tre", 3),
        ("fyra", 4),
        ("fem", 5),
        ("sex", 6),
        ("sju", 7),
        ("åtta", 8),
        ("nio", 9),
        ("tio", 10),
        ("elva", 11),
        ("tolv", 12),
        ("tretton", 13),
        ("fjorton", 14),
        ("femton", 15),
        ("sexton", 16),
        ("sjutton", 17),
        ("arton", 18),
        ("nitton", 19),
        ("tjugo", 20),
        // chrono writes the last letter of thirty in Cyrillic.
        ("trettiо", 30),
        ("fyrtio", 40),
        ("femtio", 50),
        ("sextio", 60),
        ("sjuttio", 70),
        ("åttio", 80),
        ("nittio", 90),
        ("hundra", 100),
        ("tusen", 1000),
    ],
    numbers: r"[0-9]+",
    units: &[
        ("sek", Unit::Second),
        ("sekund", Unit::Second),
        ("sekunder", Unit::Second),
        ("min", Unit::Minute),
        ("minut", Unit::Minute),
        ("minuter", Unit::Minute),
        ("tim", Unit::Hour),
        ("timme", Unit::Hour),
        ("timmar", Unit::Hour),
        ("dag", Unit::Day),
        ("dagar", Unit::Day),
        ("vecka", Unit::Week),
        ("veckor", Unit::Week),
        ("mån", Unit::Month),
        ("månad", Unit::Month),
        ("månader", Unit::Month),
        ("år", Unit::Year),
        ("kvartаl", Unit::Quarter),
        ("kvartal", Unit::Quarter),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: integer,
};

/// Finnish's lengths.
static FI: Lexicon = Lexicon {
    words: &[
        ("yksi", 1),
        ("yhden", 1),
        ("kaksi", 2),
        ("kahden", 2),
        ("kolme", 3),
        ("kolmen", 3),
        ("neljä", 4),
        ("neljän", 4),
        ("viisi", 5),
        ("viiden", 5),
        ("kuusi", 6),
        ("kuuden", 6),
        ("seitsemän", 7),
        ("kahdeksan", 8),
        ("yhdeksän", 9),
        ("kymmenen", 10),
    ],
    numbers: r"[0-9]+",
    units: &[
        ("s", Unit::Second),
        ("sek", Unit::Second),
        ("sekunti", Unit::Second),
        ("sekuntia", Unit::Second),
        ("sekunnin", Unit::Second),
        ("min", Unit::Minute),
        ("minuutti", Unit::Minute),
        ("minuuttia", Unit::Minute),
        ("minuutin", Unit::Minute),
        ("t", Unit::Hour),
        ("tunti", Unit::Hour),
        ("tuntia", Unit::Hour),
        ("tunnin", Unit::Hour),
        ("pv", Unit::Day),
        ("päivä", Unit::Day),
        ("päivää", Unit::Day),
        ("päivän", Unit::Day),
        ("vk", Unit::Week),
        ("viikko", Unit::Week),
        ("viikkoa", Unit::Week),
        ("viikon", Unit::Week),
        ("kk", Unit::Month),
        ("kuukausi", Unit::Month),
        ("kuukautta", Unit::Month),
        ("kuukauden", Unit::Month),
        ("vuosi", Unit::Year),
        ("vuotta", Unit::Year),
        ("vuoden", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: integer,
};

/// Russian's lengths.
static RU: Lexicon = Lexicon {
    words: &[
        ("один", 1),
        ("одна", 1),
        ("одной", 1),
        ("одну", 1),
        ("две", 2),
        ("два", 2),
        ("двух", 2),
        ("три", 3),
        ("трех", 3),
        ("трёх", 3),
        ("четыре", 4),
        ("четырех", 4),
        ("четырёх", 4),
        ("пять", 5),
        ("пяти", 5),
        ("шесть", 6),
        ("шести", 6),
        ("семь", 7),
        ("семи", 7),
        ("восемь", 8),
        ("восьми", 8),
        ("девять", 9),
        ("девяти", 9),
        ("десять", 10),
        ("десяти", 10),
        ("одиннадцать", 11),
        ("одиннадцати", 11),
        ("двенадцать", 12),
        ("двенадцати", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|пол|несколько|пар(?:ы|у)|\s{0,3}",
    units: &[
        ("сек", Unit::Second),
        ("секунда", Unit::Second),
        ("секунд", Unit::Second),
        ("секунды", Unit::Second),
        ("секунду", Unit::Second),
        ("секундочка", Unit::Second),
        ("секундочки", Unit::Second),
        ("секундочек", Unit::Second),
        ("секундочку", Unit::Second),
        ("мин", Unit::Minute),
        ("минута", Unit::Minute),
        ("минут", Unit::Minute),
        ("минуты", Unit::Minute),
        ("минуту", Unit::Minute),
        ("минуток", Unit::Minute),
        ("минутки", Unit::Minute),
        ("минутку", Unit::Minute),
        ("минуточек", Unit::Minute),
        ("минуточки", Unit::Minute),
        ("минуточку", Unit::Minute),
        ("час", Unit::Hour),
        ("часов", Unit::Hour),
        ("часа", Unit::Hour),
        ("часу", Unit::Hour),
        ("часиков", Unit::Hour),
        ("часика", Unit::Hour),
        ("часике", Unit::Hour),
        ("часик", Unit::Hour),
        ("день", Unit::Day),
        ("дня", Unit::Day),
        ("дней", Unit::Day),
        ("суток", Unit::Day),
        ("сутки", Unit::Day),
        ("неделя", Unit::Week),
        ("неделе", Unit::Week),
        ("недели", Unit::Week),
        ("неделю", Unit::Week),
        ("недель", Unit::Week),
        ("недельке", Unit::Week),
        ("недельки", Unit::Week),
        ("неделек", Unit::Week),
        ("месяц", Unit::Month),
        ("месяце", Unit::Month),
        ("месяцев", Unit::Month),
        ("месяца", Unit::Month),
        ("квартал", Unit::Quarter),
        ("квартале", Unit::Quarter),
        ("кварталов", Unit::Quarter),
        ("год", Unit::Year),
        ("года", Unit::Year),
        ("году", Unit::Year),
        ("годов", Unit::Year),
        ("лет", Unit::Year),
        ("годик", Unit::Year),
        ("годика", Unit::Year),
        ("годиков", Unit::Year),
    ],
    gap: r"\s{0,3}",
    tail: "",
    prefix: r"(?:(?:около|примерно)\s{0,3})?",
    connector: CONNECTOR,
    skip_letters: false,
    trim: true,
    none_when_empty: false,
    special: russian,
};

/// Ukrainian's lengths.
static UK: Lexicon = Lexicon {
    words: &[
        ("один", 1),
        ("одна", 1),
        ("одної", 1),
        ("одну", 1),
        ("дві", 2),
        ("два", 2),
        ("двох", 2),
        ("три", 3),
        ("трьох", 3),
        ("чотири", 4),
        ("чотирьох", 4),
        ("п'ять", 5),
        ("п'яти", 5),
        ("шість", 6),
        ("шести", 6),
        ("сім", 7),
        ("семи", 7),
        ("вісім", 8),
        ("восьми", 8),
        ("дев'ять", 9),
        ("дев'яти", 9),
        ("десять", 10),
        ("десяти", 10),
        ("одинадцять", 11),
        ("одинадцяти", 11),
        ("дванадцять", 12),
        ("дванадцяти", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+|пів|декілька|пар(?:у)|\s{0,3}",
    units: &[
        ("сек", Unit::Second),
        ("секунда", Unit::Second),
        ("секунд", Unit::Second),
        ("секунди", Unit::Second),
        ("секунду", Unit::Second),
        ("секундочок", Unit::Second),
        ("секундочки", Unit::Second),
        ("секундочку", Unit::Second),
        ("хв", Unit::Minute),
        ("хвилина", Unit::Minute),
        ("хвилин", Unit::Minute),
        ("хвилини", Unit::Minute),
        ("хвилину", Unit::Minute),
        ("хвилинок", Unit::Minute),
        ("хвилинки", Unit::Minute),
        ("хвилинку", Unit::Minute),
        ("хвилиночок", Unit::Minute),
        ("хвилиночки", Unit::Minute),
        ("хвилиночку", Unit::Minute),
        ("год", Unit::Hour),
        ("година", Unit::Hour),
        ("годин", Unit::Hour),
        ("години", Unit::Hour),
        ("годину", Unit::Hour),
        ("годинка", Unit::Hour),
        ("годинок", Unit::Hour),
        ("годинки", Unit::Hour),
        ("годинку", Unit::Hour),
        ("день", Unit::Day),
        ("дня", Unit::Day),
        ("днів", Unit::Day),
        ("дні", Unit::Day),
        ("доба", Unit::Day),
        ("добу", Unit::Day),
        ("тиждень", Unit::Week),
        ("тижню", Unit::Week),
        ("тижня", Unit::Week),
        ("тижні", Unit::Week),
        ("тижнів", Unit::Week),
        ("місяць", Unit::Month),
        ("місяців", Unit::Month),
        ("місяці", Unit::Month),
        ("місяця", Unit::Month),
        ("квартал", Unit::Quarter),
        ("кварталу", Unit::Quarter),
        ("квартала", Unit::Quarter),
        ("кварталів", Unit::Quarter),
        ("кварталі", Unit::Quarter),
        ("рік", Unit::Year),
        ("року", Unit::Year),
        ("році", Unit::Year),
        ("років", Unit::Year),
        ("роки", Unit::Year),
    ],
    gap: r"\s{0,3}",
    tail: "",
    prefix: r"(?:(?:близько|приблизно)\s{0,3})?",
    connector: CONNECTOR,
    skip_letters: false,
    trim: true,
    none_when_empty: false,
    special: ukrainian,
};

/// Vietnamese's lengths.
static VI: Lexicon = Lexicon {
    words: &[
        ("một", 1),
        ("hai", 2),
        ("ba", 3),
        ("bốn", 4),
        ("năm", 5),
        ("sáu", 6),
        ("bảy", 7),
        ("tám", 8),
        ("chín", 9),
        ("mười", 10),
        ("mười một", 11),
        ("mười hai", 12),
    ],
    numbers: r"[0-9]+|[0-9]+\.[0-9]+",
    units: &[
        ("giây", Unit::Second),
        ("phút", Unit::Minute),
        ("giờ", Unit::Hour),
        ("ngày", Unit::Day),
        ("tuần", Unit::Week),
        ("tháng", Unit::Month),
        ("năm", Unit::Year),
    ],
    gap: r"\s{0,5}",
    tail: r"\s{0,5}",
    prefix: "",
    connector: CONNECTOR,
    skip_letters: false,
    trim: false,
    none_when_empty: false,
    special: none,
};

impl Lexicon {
    /// The number pattern: the integer words, then the rest, in chrono's order.
    fn number(&self) -> String {
        format!(
            "(?:(?:{})|{})",
            alternation(self.words.iter().map(|&(w, _)| w)),
            self.numbers
        )
    }

    /// The unit words as an alternation.
    fn unit(&self) -> String {
        alternation(self.units.iter().map(|&(w, _)| w))
    }

    /// One length, its number and unit captured.
    fn single(&self) -> String {
        format!(
            "(?P<n>{}){}(?P<w>{}){}",
            self.number(),
            self.gap,
            self.unit(),
            self.tail
        )
    }

    /// A run of lengths, uncaptured.
    fn run(&self) -> String {
        let one = format!(
            "(?:{}){}(?:{}){}",
            self.number(),
            self.gap,
            self.unit(),
            self.tail
        );
        format!("{}{one}(?:{}{one}){{0,10}}", self.prefix, self.connector)
    }

    /// The unit a word names.
    fn unit_of(&self, word: &str) -> Option<Unit> {
        let lower = word.to_lowercase();
        self.units
            .iter()
            .find(|&&(w, _)| w == lower)
            .map(|&(_, u)| u)
    }
}

/// One length, compiled per language.
static SINGLES: LazyLock<Vec<(&'static str, Option<Regex>)>> = LazyLock::new(|| {
    LEXICONS
        .iter()
        .map(|(code, lexicon)| (*code, compile(&lexicon.single())))
        .collect()
});

/// Every language with lengths, by code.
const LEXICONS: [(&str, &Lexicon); 11] = [
    ("en", &EN),
    ("fr", &FR),
    ("de", &DE),
    ("es", &ES),
    ("it", &IT),
    ("nl", &NL),
    ("sv", &SV),
    ("fi", &FI),
    ("ru", &RU),
    ("uk", &UK),
    ("vi", &VI),
];

/// A case-insensitive regex, None if it does not compile.
fn compile(pattern: &str) -> Option<Regex> {
    crate::text::compile(&format!("(?i){pattern}"))
}

/// The length a run of lengths reads, as the language's `parseDuration`
/// does: each length found in what is left, the rest cut by the length of
/// the match (not its end, as chrono cuts it), None for a run with none
/// where the language says so.
fn duration(code: &str, lexicon: &Lexicon, text: &str) -> Option<Duration> {
    let single = SINGLES
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, re)| re.as_ref())?;
    let mut out = Duration::default();
    let mut found = false;
    let mut rest = text;
    while let Some(caps) = single.captures(rest) {
        let whole = caps.get(0).map_or("", |m| m.as_str());
        let letters = !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_alphabetic());
        if !(lexicon.skip_letters && letters) {
            let value = caps
                .name("n")
                .and_then(|n| number_value(lexicon, n.as_str()));
            let unit = caps.name("w").and_then(|w| lexicon.unit_of(w.as_str()));
            if let (Some(value), Some(unit)) = (value, unit) {
                out = out.set(unit, value);
                found = true;
            }
        }
        let units: usize = whole.chars().map(char::len_utf16).sum();
        let mut cut = 0;
        let mut counted = 0;
        for c in rest.chars() {
            if counted >= units {
                break;
            }
            counted += c.len_utf16();
            cut += c.len_utf8();
        }
        if cut == 0 {
            break;
        }
        rest = rest.get(cut..).unwrap_or_default();
        if lexicon.trim {
            rest = rest.trim();
        }
    }
    (found || !lexicon.none_when_empty).then_some(out)
}

/// How a parser turns its match into a length.
#[derive(Clone, Copy)]
enum Kind {
    /// The run in group `u`, forward or into the past.
    Run {
        /// Whether the length points into the past.
        reverse: bool,
    },
    /// The run in group `u`, into the past when the word in group `m` is one of these.
    Modified(&'static [&'static str]),
    /// One length with an optional number `n`, its unit `w`, and a
    /// required modifier `m1` or `m2`, into the past when it holds one of
    /// these (French and German).
    Single(&'static [&'static str]),
    /// One unit `w` after a modifier `m`: one ahead after one of the first
    /// words, one back after one of the second, and no hour otherwise.
    Next(&'static [&'static str], &'static [&'static str]),
    /// Vietnamese's casual relative length: a modifier before or after an
    /// optional number and a unit.
    Vietnamese,
}

/// One time-unit parser.
struct Parser {
    /// The pattern, anchored, its right lookahead a consumed `post` group.
    re: Option<Regex>,
    /// How its match reads.
    kind: Kind,
    /// A match this pattern rejects ("for the year", "per il mese").
    exclude: Option<Regex>,
    /// Whether its left boundary is Unicode's (Russian and Ukrainian).
    unicode_boundary: bool,
}

/// chrono's `(?=\W|$)`, consumed.
const POST: &str = "(?P<post>[^A-Za-z0-9_]|$)";

/// The Russian and Ukrainian right boundary, consumed.
const POST_U: &str = r"(?P<post>[^\p{L}\p{N}_]|$)";

/// A parser from its pattern; `{u}` is the run of lengths, `{n}` a number
/// and `{w}` a unit.
fn parser(lexicon: &Lexicon, pattern: &str, kind: Kind) -> Parser {
    let pattern = pattern
        .replace("{u}", &format!("(?P<u>{})", lexicon.run()))
        .replace("{n}", &lexicon.number())
        .replace("{w}", &lexicon.unit());
    Parser {
        re: compile(&format!("^{pattern}")),
        kind,
        exclude: None,
        unicode_boundary: false,
    }
}

/// The same parser, rejecting matches of `exclude`.
fn excluding(mut parser: Parser, exclude: &str) -> Parser {
    parser.exclude = crate::text::compile(exclude);
    parser
}

/// The same parser, with Unicode's left boundary.
const fn unicode(mut parser: Parser) -> Parser {
    parser.unicode_boundary = true;
    parser
}

/// Each language's time-unit parsers.
static PARSERS: LazyLock<Vec<(&'static str, Vec<Parser>)>> = LazyLock::new(|| {
    use Kind::{Modified, Next, Run, Single, Vietnamese};
    let fwd = Run { reverse: false };
    let back = Run { reverse: true };
    vec![
        (
            "en",
            vec![
                excluding(
                    parser(
                        &EN,
                        &format!(
                            r"(?:within|in|for)\s*(?:(?:about|around|roughly|approximately|just)\s*(?:~\s*)?)?{{u}}{POST}"
                        ),
                        fwd,
                    ),
                    r"(?i)^for\s*the\s*[A-Za-z0-9_]+",
                ),
                parser(
                    &EN,
                    &format!(r"{{u}}\s{{0,5}}(?:ago|before|earlier){POST}"),
                    back,
                ),
                parser(
                    &EN,
                    &format!(
                        r"{{u}}\s{{0,5}}(?:later|after|from now|henceforth|forward|out){POST}"
                    ),
                    fwd,
                ),
                parser(
                    &EN,
                    &format!(r"(?P<m>this|last|past|next|after|\+|-)\s*{{u}}{POST}"),
                    Modified(&["last", "past", "-"]),
                ),
                parser(
                    &EN,
                    &format!(r"(?P<m>this|last|past|next|after\s*this)\s*(?P<w>{{w}}){POST}"),
                    Next(&["next", "after"], &["last", "past"]),
                ),
            ],
        ),
        (
            "fr",
            vec![
                parser(
                    &FR,
                    &format!(
                        r"(?:les?|la|l'|du|des?)\s*(?P<n>{{n}})?(?:\s*(?P<m1>prochaine?s?|derni[eè]re?s?|pass[ée]e?s?|pr[ée]c[ée]dents?|suivante?s?))?\s*(?P<w>{{w}})(?:\s*(?P<m2>prochaine?s?|derni[eè]re?s?|pass[ée]e?s?|pr[ée]c[ée]dents?|suivante?s?))?{POST}"
                    ),
                    Single(&["derni", "pass", "prec", "préc"]),
                ),
                parser(&FR, &format!(r"il y a\s*{{u}}{POST}"), back),
                parser(
                    &FR,
                    &format!(r"(?:dans|en|pour|pendant|de)\s*{{u}}{POST}"),
                    fwd,
                ),
            ],
        ),
        (
            "de",
            vec![
                parser(
                    &DE,
                    &format!(
                        r"(?:\s*(?P<m1>(?:nächste|kommende|folgende|letzte|vergangene|vorige|vor(?:her|an)gegangene)(?:s|n|m|r)?|vor|in)\s*)?(?P<n>{{n}})?(?:\s*(?P<m2>nächste|kommende|folgende|letzte|vergangene|vorige|vor(?:her|an)gegangene)(?:s|n|m|r)?)?\s*(?P<w>{{w}}){POST}"
                    ),
                    Single(&["vor", "letzte", "vergangen"]),
                ),
                parser(&DE, &format!(r"(?:in|für|während)\s*{{u}}{POST}"), fwd),
            ],
        ),
        (
            "es",
            vec![parser(
                &ES,
                &format!(r"(?:en|por|durante|de|dentro de)\s*{{u}}{POST}"),
                fwd,
            )],
        ),
        (
            "it",
            vec![
                excluding(
                    parser(
                        &IT,
                        &format!(
                            r"(?:entro|tra|fra|in|per)\s*(?:(?:circa|approssimativamente)\s*(?:~\s*)?)?{{u}}{POST}"
                        ),
                        fwd,
                    ),
                    r"^per\s*(?:il|la|l')\s*[A-Za-z0-9_]+",
                ),
                parser(&IT, &format!(r"{{u}}\s{{0,5}}(?:fa|prima){POST}"), back),
                parser(
                    &IT,
                    &format!(r"{{u}}\s{{0,5}}(?:dopo|più\s*tardi|da\s*adesso|da\s*ora){POST}"),
                    fwd,
                ),
                parser(
                    &IT,
                    &format!(
                        r"(?:(?:il\s*|la\s*|l'\s*)?(?P<m>questo|questa|quest'|scorso|scorsa|prossimo|prossima|dopo\s*questo|dopo\s*questa)\s*(?P<w>{{w}})|(?:il\s*|la\s*|l'\s*)(?P<w2>{{w}})\s*(?P<m2>scorso|scorsa|prossimo|prossima)){POST}"
                    ),
                    Next(&["prossimo", "prossima", "dopo"], &["scorso", "scorsa"]),
                ),
                parser(
                    &IT,
                    &format!(
                        r"(?:il\s*|la\s*|l'\s*)?(?P<m>questo|questa|quest'|scorso|scorsa|prossimo|prossima|dopo|\+|-)\s*{{u}}{POST}"
                    ),
                    Modified(&["scorso", "scorsa", "-"]),
                ),
            ],
        ),
        (
            "nl",
            vec![
                parser(
                    &NL,
                    &format!(
                        r"(?P<m>dit|deze|vorig|afgelopen|(?:aan)?komend|over|\+|-)e?\s*{{u}}{POST}"
                    ),
                    Modified(&["vorig", "afgelopen", "-"]),
                ),
                parser(
                    &NL,
                    &format!(
                        r"(?P<m>dit|deze|(?:aan)?komend|volgend|afgelopen|vorig)e?\s*(?P<w>{{w}}){POST}"
                    ),
                    Next(&["volgend", "komend", "aankomend"], &["afgelopen", "vorig"]),
                ),
                parser(
                    &NL,
                    &format!(r"(?:binnen|in|binnen de|voor)\s*{{u}}{POST}"),
                    fwd,
                ),
                parser(&NL, &format!(r"{{u}}(?:geleden|voor|eerder){POST}"), back),
                parser(
                    &NL,
                    &format!(r"{{u}}(?:later|na|vanaf nu|voortaan|vooruit|uit){POST}"),
                    fwd,
                ),
            ],
        ),
        (
            "sv",
            vec![parser(
                &SV,
                &format!(
                    r"(?P<m>denna|den här|förra|passerade|nästa|kommande|efter|\+|-)\s*{{u}}{POST}"
                ),
                Modified(&["förra", "passerade", "-"]),
            )],
        ),
        (
            "fi",
            vec![
                parser(
                    &FI,
                    &format!(
                        r"(?P<m>seuraava|seuraavat|seuraavien|edellinen|edelliset|edellisten|viimeiset|viimeisten|kuluneet|kuluneiden|\+|-)\s*{{u}}{POST}"
                    ),
                    Modified(&[
                        "edellinen",
                        "edelliset",
                        "edellisten",
                        "viimeiset",
                        "viimeisten",
                        "kuluneet",
                        "kuluneiden",
                        "-",
                    ]),
                ),
                parser(
                    &FI,
                    &format!(r"{{u}}\s*(?:sisällä|kuluessa|päästä){POST}"),
                    fwd,
                ),
                parser(&FI, &format!(r"{{u}}\s*sitten{POST}"), back),
            ],
        ),
        (
            "ru",
            vec![
                unicode(parser(
                    &RU,
                    &format!(
                        r"(?P<m>эти|последние|прошлые|следующие|после|спустя|через|\+|-)\s*{{u}}{POST_U}"
                    ),
                    Modified(&["последние", "прошлые", "-"]),
                )),
                unicode(parser(
                    &RU,
                    &format!(
                        r"(?P<m>в прошлом|на прошлой|на следующей|в следующем|на этой|в этом)\s*(?P<w>{{w}}){POST_U}"
                    ),
                    Next(
                        &["на следующей", "в следующем"],
                        &["в прошлом", "на прошлой"],
                    ),
                )),
                unicode(parser(
                    &RU,
                    &format!(
                        r"(?:в течение|в течении)\s*(?:(?:около|примерно)\s*(?:~\s*)?)?{{u}}{POST_U}"
                    ),
                    fwd,
                )),
                unicode(parser(&RU, &format!(r"{{u}}\s{{0,5}}назад{POST}"), back)),
            ],
        ),
        (
            "uk",
            vec![
                unicode(parser(
                    &UK,
                    &format!(
                        r"(?P<m>ці|останні|минулі|майбутні|наступні|після|через|\+|-)\s*{{u}}{POST_U}"
                    ),
                    Modified(&["останні", "минулі", "-"]),
                )),
                unicode(parser(
                    &UK,
                    &format!(
                        r"(?P<m>в минулому|у минулому|на минулому|минулого|на наступному|в наступному|у наступному|наступного|на цьому|в цьому|у цьому|цього)\s*(?P<w>{{w}}){POST_U}"
                    ),
                    Next(
                        &[
                            "на наступному",
                            "в наступному",
                            "у наступному",
                            "наступного",
                        ],
                        &["на минулому", "в минулому", "у минулому", "минулого"],
                    ),
                )),
                unicode(parser(
                    &UK,
                    &format!(
                        r"(?:протягом|на протязі|протягом|упродовж|впродовж)\s*(?:(?:приблизно|орієнтовно)\s*(?:~\s*)?)?{{u}}{POST_U}"
                    ),
                    fwd,
                )),
                unicode(parser(&UK, &format!(r"{{u}}\s{{0,5}}тому{POST}"), back)),
            ],
        ),
        (
            "vi",
            vec![
                parser(
                    &VI,
                    &format!(
                        r"(?:(?P<m>này|trước|qua|sau|tới|tiếp)\s*(?P<c>(?:{{n}}\s{{0,5}})?(?:{{w}}))|(?P<c2>(?:{{n}}\s{{0,5}})?(?:{{w}}))\s*(?P<m2>này|trước|qua|sau|tới|tiếp){POST})"
                    ),
                    Vietnamese,
                ),
                parser(&VI, &format!(r"{{u}}\s{{0,5}}(?:trước|qua){POST}"), back),
                parser(
                    &VI,
                    &format!(r"{{u}}\s{{0,5}}(?:sau|nữa|tới|tiếp){POST}"),
                    fwd,
                ),
                parser(&VI, &format!(r"(?:trong\s*(?:vòng\s*)?){{u}}{POST}"), fwd),
            ],
        ),
    ]
});

/// The match without its consumed right boundary.
fn matched_len(caps: &Captures<'_>) -> usize {
    caps.name("post")
        .map_or_else(|| caps.get(0).map_or(0, |m| m.end()), |m| m.start())
}

/// The length one parser reads from its match, None where it rejects it.
fn read(code: &str, lexicon: &Lexicon, parser: &Parser, caps: &Captures<'_>) -> Option<Duration> {
    let group = |name: &str| caps.name(name).map(|m| m.as_str());
    match parser.kind {
        Kind::Run { reverse } => {
            let d = duration(code, lexicon, group("u")?)?;
            Some(if reverse { d.reversed() } else { d })
        }
        Kind::Modified(back) => {
            let d = duration(code, lexicon, group("u")?)?;
            let modifier = group("m").map(str::to_lowercase).unwrap_or_default();
            Some(if back.contains(&modifier.as_str()) {
                d.reversed()
            } else {
                d
            })
        }
        Kind::Single(back) => {
            let modifier = group("m1").or_else(|| group("m2"))?.to_lowercase();
            let value = group("n").map_or(Some(1.0), |n| number_value(lexicon, n))?;
            let unit = lexicon.unit_of(group("w")?)?;
            let d = Duration::default().set(unit, value);
            Some(if back.iter().any(|b| modifier.contains(b)) {
                d.reversed()
            } else {
                d
            })
        }
        Kind::Next(ahead, back) => {
            let modifier = group("m").or_else(|| group("m2"))?.to_lowercase();
            let unit = lexicon.unit_of(group("w").or_else(|| group("w2"))?)?;
            if ahead.iter().any(|a| {
                modifier == *a || (*a == "after" || *a == "dopo") && modifier.starts_with(a)
            }) {
                Some(Duration::default().set(unit, 1.0))
            } else if back.contains(&modifier.as_str()) {
                Some(Duration::default().set(unit, -1.0))
            } else {
                Some(Duration::default())
            }
        }
        Kind::Vietnamese => {
            let modifier = group("m")
                .or_else(|| group("m2"))
                .map(str::to_lowercase)
                .unwrap_or_default();
            let unit_text = group("c").or_else(|| group("c2"))?.to_lowercase();
            let mut d = duration(code, lexicon, &unit_text)?;
            if d.0.iter().all(Option::is_none) {
                d = Duration::default().set(lexicon.unit_of(&unit_text)?, 1.0);
            }
            Some(if matches!(modifier.as_str(), "trước" | "qua") {
                d.reversed()
            } else {
                d
            })
        }
    }
}

/// The longest length from now the language `code` reads at the start of
/// `text`. `word_ok` says whether a parser with chrono's ASCII left
/// boundary may start there, `unicode_ok` one with Unicode's; Chinese's
/// deadline has none.
pub fn at_start(code: &str, text: &str, word_ok: bool, unicode_ok: bool) -> Option<Time> {
    let mut kept: Option<Time> = None;
    let lexicon = LEXICONS.iter().find(|(c, _)| *c == code).map(|(_, l)| *l)?;
    let (_, parsers) = PARSERS.iter().find(|(c, _)| *c == code)?;
    for parser in parsers {
        let allowed = if parser.unicode_boundary {
            unicode_ok
        } else {
            word_ok
        };
        if !allowed {
            continue;
        }
        let Some(caps) = parser.re.as_ref().and_then(|re| re.captures(text)) else {
            continue;
        };
        let len = matched_len(&caps);
        let whole = text.get(..len).unwrap_or_default();
        if parser.exclude.as_ref().is_some_and(|re| re.is_match(whole)) {
            continue;
        }
        let Some(d) = read(code, lexicon, parser, &caps) else {
            continue;
        };
        let time = from_length(len, whole, d);
        kept = Some(kept.map_or(time, |k| super::longer(text, k, time)));
    }
    kept
}

/// The result a length from now makes.
fn from_length(len: usize, text: &str, d: Duration) -> Time {
    let (hour, dated, valid) = d.reading();
    Time {
        len,
        valid,
        unlikely: super::unlikely(text),
        hour,
        dated,
        range: None,
        zoned: hour,
    }
}

/// Chinese's deadline at the start of `text`, in either script ("两小时内",
/// "3天之后"): its pattern has no left boundary.
pub fn deadline(code: &str, text: &str) -> Option<Time> {
    if code != "zh" {
        return None;
    }
    DEADLINES
        .iter()
        .filter_map(|(re, numerals)| {
            let caps = re.as_ref()?.captures(text)?;
            let len = caps.get(0)?.end();
            let number = caps.name("n")?.as_str();
            let value = match number {
                "几" | "幾" => 3.0,
                "半" => 0.5,
                _ => f64::from(super::east::number(number, numerals)?),
            };
            let unit = match caps.name("w")?.as_str().chars().next()? {
                '日' | '天' => Unit::Day,
                '星' | '礼' | '禮' => Unit::Week,
                '月' => Unit::Month,
                '年' => Unit::Year,
                '秒' => Unit::Second,
                '分' => Unit::Minute,
                _ => Unit::Hour,
            };
            Some(from_length(
                len,
                text.get(..len)?,
                Duration::default().set(unit, value),
            ))
        })
        .reduce(|kept, next| super::longer(text, kept, next))
}

/// A deadline pattern and the numerals of its script.
type Deadline = (Option<Regex>, &'static [(char, u32)]);

/// Chinese's deadline patterns, simplified then traditional, with their numerals.
static DEADLINES: LazyLock<[Deadline; 2]> = LazyLock::new(|| {
    let hans = super::east::HANS_NUMERALS;
    let hant = super::east::HANT_NUMERALS;
    let class = |n: &[(char, u32)]| n.iter().map(|&(c, _)| c).collect::<String>();
    [
        (
            compile(&format!(
                r"^(?P<n>[0-9]+|[{}]+|半|几)(?:\s*)(?:个)?(?P<w>秒(?:钟)?|分钟|小时|钟|日|天|星期|礼拜|月|年)(?:(?:之|过)?后|(?:之)?内)",
                class(hans)
            )),
            hans,
        ),
        (
            compile(&format!(
                r"^(?P<n>[0-9]+|[{}]+|半|幾)(?:\s*)(?:個)?(?P<w>秒(?:鐘)?|分鐘|小時|鐘|日|天|星期|禮拜|月|年)(?:(?:之|過)?後|(?:之)?內)",
                class(hant)
            )),
            hant,
        ),
    ]
});
