//! The words around a scale abbreviation that is also a unit or a word: a
//! currency named in letters, which makes it a scale ("5 bin TL", "Rs 92
//! cr"), and the words that make it a unit ("took 5m", "5m tall", "5 md
//! files").

use crate::text::{char_before, is_letter, same_word};

/// What an abbreviation names when it is not a scale.
#[derive(Debug, Clone, Copy)]
pub(super) enum Unit {
    /// Minutes ("m", "mn"), or metres ("m").
    TimeOrLength,
    /// Minutes ("mn").
    Time,
    /// Miles ("mi").
    Length,
    /// A file type ("md").
    File,
    /// Tonnes ("t", "tn").
    Weight,
    /// Litres ("l").
    Volume,
    /// Course credits ("cr").
    Credit,
    /// A title before a name ("Mr").
    Title,
    /// A waste bin ("bin").
    Bin,
    /// A word of its own before a noun ("Bio" is organic).
    Word,
    /// A display resolution ("4K screens").
    Resolution,
}

/// Words before a figure that measure time with it: "took 5m", "every 5
/// mn", "en 5 mn".
const TIME_BEFORE: &[&str] = &[
    "took",
    "take",
    "takes",
    "taking",
    "lasted",
    "lasts",
    "lasting",
    "waited",
    "wait",
    "waits",
    "waiting",
    "ran",
    "run",
    "runs",
    "every",
    "within",
    "after",
    "elapsed",
    "en",
    "dans",
    "pendant",
    "durant",
    "dure",
    "après",
    "toutes",
    "chaque",
    "dauert",
    "dauerte",
    "nach",
    "dura",
    "duró",
    "tarda",
    "tardó",
    "durou",
    "demorou",
    "levou",
    "длилось",
    "длится",
    "заняло",
    "занимает",
    "через",
];

/// Words after a unit of time: "5m ago", "5 mn later".
const TIME_AFTER: &[&str] = &[
    "ago",
    "later",
    "left",
    "elapsed",
    "remaining",
    "late",
    "early",
];

/// Words before a figure that measure a length with it: "ran 5 mi".
const LENGTH_BEFORE: &[&str] = &[
    "ran",
    "run",
    "runs",
    "walked",
    "walk",
    "drove",
    "drive",
    "hiked",
    "swam",
    "cycled",
    "rode",
    "flew",
    "measures",
    "measured",
    "measuring",
    "height",
    "width",
    "length",
    "depth",
    "radius",
    "diameter",
    "distance",
    "x",
    "×",
];

/// Words after a unit of length: "5m tall", "3m x 4m".
const LENGTH_AFTER: &[&str] = &[
    "long", "tall", "wide", "high", "deep", "thick", "away", "far", "apart", "across", "x",
];

/// Words after a file type: "5 md files".
const FILE_AFTER: &[&str] = &[
    "file",
    "files",
    "doc",
    "docs",
    "document",
    "documents",
    "page",
    "pages",
    "note",
    "notes",
];

/// Words after a display resolution: "4K screens", "8K video".
const RESOLUTION_AFTER: &[&str] = &[
    "screen",
    "screens",
    "display",
    "displays",
    "monitor",
    "monitors",
    "tv",
    "tvs",
    "video",
    "videos",
    "resolution",
    "footage",
    "camera",
    "cameras",
    "stream",
    "streams",
    "streaming",
    "uhd",
    "hdr",
    "panel",
    "panels",
    "projector",
    "projectors",
    "television",
    "televisions",
];

/// Words before a figure that weigh with it: "weighs 5 tn".
const WEIGHT_BEFORE: &[&str] = &[
    "weighs", "weighed", "weighing", "weight", "load", "loaded", "carries", "carried", "carry",
    "capacity", "shipped", "hauled", "payload",
];

/// Words after a unit of weight: "a 2481t truck".
const WEIGHT_AFTER: &[&str] = &[
    "truck", "trucks", "lorry", "lorries", "load", "cargo", "payload", "crane", "trailer",
];

/// Words before a figure that measure a volume with it: "holds 300l".
const VOLUME_BEFORE: &[&str] = &[
    "holds", "hold", "capacity", "drank", "drink", "poured", "pour", "filled", "fill", "volume",
];

/// Words after a unit of volume: "300l of water", "a 5l bottle".
const VOLUME_AFTER: &[&str] = &[
    "of",
    "bottle",
    "bottles",
    "tank",
    "tanks",
    "jug",
    "can",
    "cans",
    "drum",
    "drums",
    "container",
    "containers",
    "engine",
];

/// Words before a figure that count course credits: "earned 92 cr".
const CREDIT_BEFORE: &[&str] = &[
    "earned",
    "earn",
    "earns",
    "earning",
    "completed",
    "complete",
    "needs",
    "need",
    "requires",
    "required",
];

/// Words after course credits: "92 cr this term".
const CREDIT_AFTER: &[&str] = &[
    "course", "courses", "hours", "this", "per", "semester", "term", "module", "modules",
];

/// Words after a waste bin: "5 bin bags".
const BIN_AFTER: &[&str] = &[
    "bag",
    "bags",
    "liner",
    "liners",
    "lid",
    "lids",
    "lorry",
    "lorries",
    "truck",
    "trucks",
    "collection",
    "collections",
];

/// Words that only soften the figure after them ("took about 5m"), passed
/// over to the word before.
const SOFTENERS: &[&str] = &[
    "about",
    "around",
    "roughly",
    "approximately",
    "approx",
    "nearly",
    "almost",
    "just",
    "only",
    "over",
    "under",
    "environ",
];

/// Currencies written in letters, case aside, and their plurals: what a
/// locale writes after an amount, or before it as a symbol ("Rs", "TL").
const CURRENCY_WORDS: &[&str] = &[
    "dollar", "dollars", "dólar", "dólares", "dolar", "dolares", "euro", "euros", "avro", "pound",
    "pounds", "sterling", "yen", "yuan", "renminbi", "rupee", "rupees", "rupiah", "lira", "liras",
    "peso", "pesos", "real", "reais", "franc", "francs", "franken", "krona", "kronor", "krone",
    "kroner", "rand", "ruble", "rubles", "rouble", "roubles", "dirham", "dirhams", "riyal",
    "riyals", "rial", "rials", "dinar", "dinars", "won", "baht", "dong", "ringgit", "zloty",
    "złoty", "złotych", "forint", "shekel", "shekels", "naira", "lei", "leu",
];

/// The local currency symbols written in letters that no ISO code spells:
/// "Rs 92 cr", "5 bin TL", "Rp 300l".
const CURRENCY_SYMBOLS: &[&str] = &[
    "Rs", "TL", "Rp", "RM", "kr", "Kr", "zł", "Kč", "Ft", "đ", "Đ",
];

/// The run of letters that ends at byte `end` of `text`, and the byte it
/// starts at.
fn letters_before(text: &str, end: usize) -> (&str, usize) {
    let head = text.get(..end).unwrap_or_default();
    let start = head.trim_end_matches(is_word_letter).len();
    (head.get(start..).unwrap_or_default(), start)
}

/// The run of letters that starts at byte `start` of `text`.
fn letters_after(text: &str, start: usize) -> &str {
    let tail = text.get(start..).unwrap_or_default();
    let len = tail.len() - tail.trim_start_matches(is_word_letter).len();
    tail.get(..len).unwrap_or_default()
}

/// Whether `c` belongs to a word.
fn is_word_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || (!c.is_ascii() && is_letter(c))
}

/// The word before the figure that starts at byte `start`, over its sign,
/// its approximation mark, a softening word ("took about ~5m") and the low
/// bound of a range it ends ("takes 40-50m").
fn word_before(text: &str, start: usize) -> &str {
    let mut at = start;
    let marks = |c: char| c.is_whitespace() || matches!(c, '~' | '≈' | '∼' | '-' | '\u{2013}');
    let lead = text.get(..at).unwrap_or_default();
    let bound = lead.trim_end_matches(marks);
    if bound.len() < lead.len()
        && lead
            .get(bound.len()..)
            .is_some_and(|between| between.contains(['-', '\u{2013}']))
    {
        let low = bound.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | ','));
        if low.len() < bound.len() && !low.ends_with(|c: char| c.is_alphanumeric()) {
            at = low.len();
        }
    }
    for _ in 0..2 {
        let head = text.get(..at).unwrap_or_default();
        let trimmed = head.trim_end_matches(marks);
        if let Some(before_sign) = trimmed.strip_suffix('×') {
            return trimmed.get(before_sign.len()..).unwrap_or_default();
        }
        let (word, word_start) = letters_before(text, trimmed.len());
        if !SOFTENERS.iter().any(|w| same_word(w, word)) {
            return word;
        }
        at = word_start;
    }
    ""
}

/// The word after byte `end`, past one space: a run of letters, or the
/// multiplication sign a dimension is written with.
fn word_after(text: &str, end: usize) -> &str {
    let tail = text.get(end..).unwrap_or_default();
    let tail = tail.strip_prefix(char::is_whitespace).unwrap_or(tail);
    if let Some(rest) = tail.strip_prefix('×') {
        return tail.get(..tail.len() - rest.len()).unwrap_or_default();
    }
    letters_after(text, text.len() - tail.len())
}

/// Whether the abbreviation that `start..end` of `text` reads, a figure's
/// digits from `start`, names the unit `unit` there: a word before or after
/// the figure measures that unit with it, or, for a time, another part of a
/// duration stands beside it ("1h 5m", "5m 30s").
pub(super) fn names_unit(text: &str, start: usize, end: usize, unit: Unit) -> bool {
    let before = word_before(text, start);
    let after = word_after(text, end);
    let is = |list: &[&str], word: &str| list.iter().any(|w| same_word(w, word));
    let time =
        || is(TIME_BEFORE, before) || is(TIME_AFTER, after) || beside_duration(text, start, end);
    let length = || {
        (is(LENGTH_BEFORE, before) && (!is_times(before) || measures_before(text, start)))
            || is(LENGTH_AFTER, after)
    };
    match unit {
        Unit::TimeOrLength => time() || length(),
        Unit::Time => time(),
        Unit::Length => length(),
        Unit::File => is(FILE_AFTER, after),
        Unit::Weight => is(WEIGHT_BEFORE, before) || is(WEIGHT_AFTER, after),
        Unit::Volume => is(VOLUME_BEFORE, before) || is(VOLUME_AFTER, after),
        Unit::Credit => is(CREDIT_BEFORE, before) || is(CREDIT_AFTER, after),
        Unit::Title => after.chars().next().is_some_and(char::is_uppercase),
        Unit::Bin => is(BIN_AFTER, after),
        Unit::Word => after.chars().next().is_some_and(char::is_alphabetic),
        Unit::Resolution => is(RESOLUTION_AFTER, after),
    }
}

/// Whether `word` is the multiplication sign a dimension is written with.
fn is_times(word: &str) -> bool {
    matches!(word, "x" | "X" | "×")
}

/// Whether the multiplication sign before the figure from byte `start`
/// stands between two measures ("3m x 5m", "3 × 5m"): a word of its own
/// after a figure. Glued to a figure ("2x"), after a word ("Option x") or
/// ending a link's path, it multiplies or names nothing to measure.
fn measures_before(text: &str, start: usize) -> bool {
    let head = text.get(..start).unwrap_or_default().trim_end();
    let Some(sign) = head.chars().next_back() else {
        return false;
    };
    let rest = head
        .get(..head.len().saturating_sub(sign.len_utf8()))
        .unwrap_or_default();
    let spaced = rest.ends_with(char::is_whitespace);
    let rest = rest.trim_end();
    let rest = rest.strip_suffix('m').unwrap_or(rest).trim_end();
    (spaced || sign == '×') && rest.ends_with(|c: char| c.is_ascii_digit())
}

/// Whether hours stand before the figure from `start` ("1h 5m", "2 h 30
/// mn"), or seconds after the unit ending at `end` ("5m 30s").
fn beside_duration(text: &str, start: usize, end: usize) -> bool {
    let head = text.get(..start).unwrap_or_default().trim_end();
    let hours = head.strip_suffix('h').is_some_and(|h| {
        char_before(h.trim_end(), h.trim_end().len()).is_some_and(|c| c.is_ascii_digit())
    });
    if hours {
        return true;
    }
    let tail = text.get(end..).unwrap_or_default().trim_start();
    let unit = tail.trim_start_matches(|c: char| c.is_ascii_digit());
    unit.len() < tail.len() && {
        let unit = unit.trim_start();
        let word = letters_after(unit, 0);
        matches!(word, "s" | "sec" | "secs" | "seconds")
    }
}

/// Whether the letters glued before byte `start` of `text` are a currency
/// written in letters: a local symbol ("Rp1.250.000", "Kč915") or an ISO
/// 4217 code ("USD12,500").
pub(super) fn currency_letters_before(text: &str, start: usize) -> bool {
    let (before, _) = letters_before(text, start);
    CURRENCY_SYMBOLS.contains(&before)
        || (before.len() == 3
            && before.bytes().all(|b| b.is_ascii_uppercase())
            && iso_currency::Currency::from_code(before).is_some())
}

/// Whether the letters glued after byte `end` of `text` are a currency
/// symbol written in letters ("12.480.000đ", "915zł").
pub(super) fn currency_letters_after(text: &str, end: usize) -> bool {
    CURRENCY_SYMBOLS.contains(&letters_after(text, end))
}

/// Whether a currency written in letters stands beside the figure from byte
/// `start` whose scale word ends at byte `end`: a local symbol before it
/// ("Rs 92 cr", "TL 5 bin"), or a currency's name or symbol after it, the
/// "of" of a Romance language allowed between ("5 bin TL", "3.2m dollars",
/// "100,000m d'euros").
pub(super) fn currency_word_beside(text: &str, start: usize, end: usize) -> bool {
    let head = text.get(..start).unwrap_or_default();
    let head = head.strip_suffix(char::is_whitespace).unwrap_or(head);
    let head = head.strip_suffix('.').unwrap_or(head);
    let (before, _) = letters_before(text, head.len());
    if CURRENCY_SYMBOLS.contains(&before) {
        return true;
    }
    let tail = text.get(end..).unwrap_or_default();
    let tail = tail.strip_prefix(char::is_whitespace).unwrap_or(tail);
    let tail = ["d'", "d\u{2019}", "de ", "di ", "of "]
        .iter()
        .find_map(|of| tail.strip_prefix(of))
        .unwrap_or(tail);
    let after = letters_after(tail, 0);
    CURRENCY_SYMBOLS.contains(&after) || CURRENCY_WORDS.iter().any(|w| same_word(w, after))
}
