//! Whether a text opens on a time of day, as chrono-node 2.10.2's time
//! parsers read one (MIT): its time expressions, French and German written
//! hours ("17h30", "17 Uhr"), lengths from now ("de 9h") and the casual
//! words that set an hour ("noon", "now"). The weekday check asks it what
//! follows a bare day number.
//!
//! Only a reading at the very start counts, so each pattern is anchored; a
//! lookahead in chrono is a consumed `post` group, as in `languages`.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// The digits, separators and meridiem of chrono's time expression, after
/// its prefix and before its suffix.
const CLOCK: &str = r"(?P<h>[0-9]{1,4})(?:(?:\.|:|：)(?P<mi>[0-9]{1,2})(?:(?::|：)(?P<s>[0-9]{2})(?:\.(?P<ms>[0-9]{1,6}))?)?)?(?:\s*(?P<ap>a\.m\.|p\.m\.|am?|pm?))?";

/// The same after a range's separator, where seconds take one digit too.
const FOLLOWING_CLOCK: &str = r"(?P<h>[0-9]{1,4})(?:(?:\.|:|：)(?P<mi>[0-9]{1,2})(?:(?:\.|:|：)(?P<s>[0-9]{1,2})(?:\.(?P<ms>[0-9]{1,6}))?)?)?(?:\s*(?P<ap>a\.m\.|p\.m\.|am?|pm?))?";

/// chrono's `(?!/)(?=\W|$)`, consumed.
const POST: &str = "(?P<post>[^A-Za-z0-9_/]|$)";

/// chrono's `(?=\W|$)`, consumed.
const POST_WORD: &str = "(?P<post>[^A-Za-z0-9_]|$)";

/// The Russian and Ukrainian right boundary after `(?!/)`, consumed.
const POST_CYRILLIC: &str = r"(?P<post>[^\p{L}\p{N}_/]|$)";

/// The Russian and Ukrainian right boundary, consumed.
const POST_CYRILLIC_WORD: &str = r"(?P<post>[^\p{L}\p{N}_]|$)";

/// A case-insensitive regex, None if it does not compile.
fn compile(pattern: &str) -> Option<Regex> {
    crate::text::compile(&format!("(?i){pattern}"))
}

/// One language's subclass of chrono's `AbstractTimeExpressionParser`.
struct Expression {
    /// The prefix, the time, the suffix and the right boundary, anchored.
    primary: Option<Regex>,
    /// A range's separator and second time, anchored.
    following: Option<Regex>,
    /// Whether a lone four-digit number is a year, not a time (French,
    /// German, Dutch, Finnish).
    four_digits_is_year: bool,
}

impl Expression {
    /// A parser from its prefix, suffix and range separator.
    fn new(prefix: &str, suffix: &str, post: &str, phase: &str, year: bool) -> Self {
        Self {
            primary: compile(&format!("^{prefix}{CLOCK}{suffix}{post}")),
            following: compile(&format!("^(?:{phase}){FOLLOWING_CLOCK}{POST}")),
            four_digits_is_year: year,
        }
    }
}

/// French's or German's parser of written hours ("17h30", "8 Uhr abends").
struct Written {
    /// The hour, minutes and seconds, anchored.
    first: Option<Regex>,
    /// A range's separator and second hour, anchored.
    second: Option<Regex>,
    /// How the meridiem group reads.
    meridiem: fn(&str, u32) -> u32,
}

/// One language's time parsers.
struct Clock {
    /// The time expression, if the language has one.
    expression: Option<Expression>,
    /// The written-hour parser, if the language has one.
    written: Option<Written>,
    /// The casual words that set an hour ("noon", "now"), anchored.
    casual: Option<Regex>,
    /// A length of time from now ("de 9h", "in 2 hours"), anchored.
    within: Option<Regex>,
}

/// A run of ASCII digits.
fn digits(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.bytes().try_fold(0_u32, |n, b| {
        n.checked_mul(10)?
            .checked_add(u32::from(b.wrapping_sub(b'0')))
    })
}

/// JavaScript's `parseInt` on a run of digits and dots: its leading digits.
fn leading_int(text: &str) -> u32 {
    let end = text.bytes().take_while(u8::is_ascii_digit).count();
    text.get(..end).and_then(digits).unwrap_or(0)
}

/// A group's text.
fn group<'t>(caps: &Captures<'t>, name: &str) -> Option<&'t str> {
    caps.name(name).map(|m| m.as_str())
}

/// The match without its consumed right boundary.
fn matched<'t>(caps: &Captures<'t>, text: &'t str) -> &'t str {
    let end = caps
        .name("post")
        .map_or_else(|| caps.get(0).map_or(0, |m| m.end()), |m| m.start());
    text.get(..end).unwrap_or_default()
}

/// The meridiem letter of an `am`/`pm` group.
fn meridiem_letter(caps: &Captures<'_>) -> Option<u8> {
    group(caps, "ap")
        .and_then(|ap| ap.bytes().next())
        .map(|b| b.to_ascii_lowercase())
}

/// The hour a primary time sets, as `extractPrimaryTimeComponents` reads
/// it, and whether its meridiem is certain; None where chrono rejects it.
fn primary_hour(
    caps: &Captures<'_>,
    four_digits_is_year: bool,
    whole: &str,
) -> Option<(u32, bool)> {
    if four_digits_is_year {
        let trimmed = whole.trim();
        if trimmed.len() == 4 && trimmed.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let hour_text = group(caps, "h")?;
    let mut hour = digits(hour_text)?;
    let minute_text = group(caps, "mi");
    let ap = meridiem_letter(caps);
    let mut minute = 0;
    if hour > 100 {
        if hour_text.len() == 4 && minute_text.is_none() && ap.is_none() {
            return None;
        }
        if minute_text.is_some() {
            return None;
        }
        minute = hour % 100;
        hour /= 100;
    }
    if hour > 24 {
        return None;
    }
    if let Some(m) = minute_text {
        if m.len() == 1 && ap.is_none() {
            return None;
        }
        minute = digits(m)?;
    }
    if minute >= 60 {
        return None;
    }
    let mut certain = hour > 12;
    match ap {
        Some(_) if hour > 12 => return None,
        Some(b'a') => {
            certain = true;
            if hour == 12 {
                hour = 0;
            }
        }
        Some(b'p') => {
            certain = true;
            if hour != 12 {
                hour += 12;
            }
        }
        _ => {}
    }
    if group(caps, "ms").is_some_and(|ms| leading_int(ms.get(..3).unwrap_or(ms)) >= 1000) {
        return None;
    }
    if group(caps, "s").and_then(digits).is_some_and(|s| s >= 60) {
        return None;
    }
    Some((hour, certain))
}

/// The second time of a range, as `extractFollowingTimeComponents` reads
/// it: its hour, and the start's hour once its meridiem is settled.
fn following_hours(caps: &Captures<'_>, start: u32, start_certain: bool) -> Option<(u32, u32)> {
    if group(caps, "ms").is_some_and(|ms| leading_int(ms.get(..3).unwrap_or(ms)) >= 1000) {
        return None;
    }
    if group(caps, "s").and_then(digits).is_some_and(|s| s >= 60) {
        return None;
    }
    let mut hour = digits(group(caps, "h")?)?;
    let mut minute = 0;
    if let Some(m) = group(caps, "mi") {
        minute = digits(m)?;
    } else if hour > 100 {
        minute = hour % 100;
        hour /= 100;
    }
    if minute >= 60 || hour > 24 {
        return None;
    }
    let mut start = start;
    let mut pm = hour >= 12;
    let mut set = pm;
    match meridiem_letter(caps) {
        Some(_) if hour > 12 => return None,
        Some(b'a') => {
            set = true;
            pm = false;
            if hour == 12 {
                hour = 0;
            }
        }
        Some(b'p') => {
            set = true;
            pm = true;
            if hour != 12 {
                hour += 12;
            }
        }
        _ => {}
    }
    if meridiem_letter(caps).is_some() && !start_certain {
        if pm {
            if start != 12 {
                start += 12;
            }
        } else if start == 12 {
            start = 0;
        }
    }
    if !set && start_certain && start > 12 && start - 12 <= hour && hour <= 12 {
        hour += 12;
    }
    Some((hour, start))
}

/// Patterns chrono matches a time's text against.
struct Shapes {
    /// `^\s*([+-])\s*\d{2,4}$`
    offset: Option<Regex>,
    /// `^\s*([+-])\s*\d{2}\W\d{2}`
    offset_clock: Option<Regex>,
    /// `^\s*([+-])\s*\d{3,4}$`
    zone: Option<Regex>,
    /// `\d[apAP]$`
    bare_meridiem: Option<Regex>,
    /// `[^\d:.](\d[\d.]+)$`
    ending: Option<Regex>,
    /// `\d(\.\d{2})+$`
    dotted: Option<Regex>,
    /// `[^\d:.](\d[\d.]+)\s*-\s*(\d[\d.]+)$`
    ending_range: Option<Regex>,
    /// `^\d+-\d+$`
    bare_range: Option<Regex>,
}

/// The patterns, compiled once.
static SHAPES: LazyLock<Shapes> = LazyLock::new(|| Shapes {
    offset: crate::text::compile(r"^\s*[+-]\s*[0-9]{2,4}$"),
    offset_clock: crate::text::compile(r"^\s*[+-]\s*[0-9]{2}[^A-Za-z0-9_][0-9]{2}"),
    zone: crate::text::compile(r"^\s*[+-]\s*[0-9]{3,4}$"),
    bare_meridiem: crate::text::compile(r"[0-9][apAP]$"),
    ending: crate::text::compile(r"[^0-9:.]([0-9][0-9.]+)$"),
    dotted: crate::text::compile(r"[0-9](?:\.[0-9]{2})+$"),
    ending_range: crate::text::compile(r"[^0-9:.]([0-9][0-9.]+)\s*-\s*([0-9][0-9.]+)$"),
    bare_range: crate::text::compile(r"^[0-9]+-[0-9]+$"),
});

/// Whether `re` matches `text`.
fn is(re: Option<&Regex>, text: &str) -> bool {
    re.is_some_and(|re| re.is_match(text))
}

/// Whether an ending number fails chrono's check: a dot not followed by two
/// digits per group, or a value past 24.
fn bad_ending(shapes: &Shapes, number: &str) -> bool {
    (number.contains('.') && !is(shapes.dotted.as_ref(), number)) || leading_int(number) > 24
}

/// `checkAndReturnWithoutFollowingPattern`: whether the text stays a time.
fn stands_alone(shapes: &Shapes, text: &str) -> bool {
    let all_digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if all_digits(text) && (text.len() == 1 || text.len() >= 3) {
        return false;
    }
    if is(shapes.bare_meridiem.as_ref(), text) {
        return false;
    }
    let ending = shapes
        .ending
        .as_ref()
        .and_then(|re| re.captures(text))
        .and_then(|c| c.get(1));
    !ending.is_some_and(|n| bad_ending(shapes, n.as_str()))
}

/// `checkAndReturnWithFollowingPattern`: whether the range stays a time.
fn stands_as_range(shapes: &Shapes, text: &str) -> bool {
    if is(shapes.bare_range.as_ref(), text) {
        return false;
    }
    let Some(caps) = shapes
        .ending_range
        .as_ref()
        .and_then(|re| re.captures(text))
    else {
        return true;
    };
    let (Some(from), Some(to)) = (caps.get(1), caps.get(2)) else {
        return true;
    };
    (!to.as_str().contains('.') || is(shapes.dotted.as_ref(), to.as_str()))
        && leading_int(to.as_str()) <= 24
        && leading_int(from.as_str()) <= 24
}

/// chrono's `UnlikelyFormatFilter`: a text that is only digits and a dot,
/// its first space aside, is no time.
fn unlikely(text: &str) -> bool {
    let squeezed = text.replacen(' ', "", 1);
    let mut parts = squeezed.splitn(2, '.');
    parts.all(|p| p.bytes().all(|b| b.is_ascii_digit()))
}

/// A time a parser read at the start of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    /// Its length in bytes, which decides which of two overlapping times stays.
    len: usize,
    /// Whether its hours exist on a clock: chrono drops a result at 24:00,
    /// and with it a weekday the time was merged into.
    valid: bool,
    /// Whether its text is only digits and a dot, which chrono's last filter
    /// drops when nothing was merged into it.
    unlikely: bool,
}

/// The time expression's result at the start of `text`, if one passes the
/// parser's own checks.
fn expression_at_start(expression: &Expression, text: &str) -> Option<Time> {
    let shapes = &*SHAPES;
    let caps = expression.primary.as_ref()?.captures(text)?;
    let primary = matched(&caps, text);
    let (start, certain) = primary_hour(&caps, expression.four_digits_is_year, primary)?;
    let alone = |hour: u32| {
        stands_alone(shapes, primary).then(|| Time {
            len: primary.len(),
            valid: hour < 24,
            unlikely: unlikely(primary),
        })
    };
    let rest = text.get(primary.len()..).unwrap_or_default();
    let following = expression
        .following
        .as_ref()
        .and_then(|re| re.captures(rest));
    let following_text = following.as_ref().map(|c| matched(c, rest));
    if let Some(f) = following_text {
        let long_lead = primary.bytes().take_while(u8::is_ascii_digit).count() >= 3;
        if long_lead && (is(shapes.offset.as_ref(), f) || is(shapes.offset_clock.as_ref(), f)) {
            return None;
        }
    }
    let (Some(second), Some(f)) = (following, following_text) else {
        return alone(start);
    };
    if is(shapes.zone.as_ref(), f) {
        return alone(start);
    }
    let read;
    let (read_text, start, end) = match following_hours(&second, start, certain) {
        Some((end, start)) => {
            read = format!("{primary}{f}");
            (read.as_str(), start, Some(end))
        }
        None => (primary, start, None),
    };
    stands_as_range(shapes, read_text).then(|| Time {
        len: read_text.len(),
        valid: start < 24 && end.is_none_or(|h| h < 24),
        unlikely: unlikely(read_text),
    })
}

/// The hour French's or German's written-hour parser reads, None where it
/// rejects the match.
fn written_hour(written: &Written, caps: &Captures<'_>) -> Option<u32> {
    let mut hour = digits(group(caps, "h")?)?;
    let minute = group(caps, "mi").map_or(Some(0), digits)?;
    if minute >= 60 || hour > 24 {
        return None;
    }
    if let Some(ap) = group(caps, "ap") {
        if hour > 12 {
            return None;
        }
        hour = (written.meridiem)(ap, hour);
    }
    if group(caps, "s").and_then(digits).is_some_and(|s| s >= 60) {
        return None;
    }
    Some(hour)
}

/// The written-hour parser's result at the start of `text`.
fn written_at_start(written: &Written, text: &str) -> Option<Time> {
    let caps = written.first.as_ref()?.captures(text)?;
    let first = matched(&caps, text);
    if first.len() == 4 && first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let start = written_hour(written, &caps)?;
    let rest = text.get(first.len()..).unwrap_or_default();
    let second = written.second.as_ref().and_then(|re| re.captures(rest));
    let (read, end) = match second
        .as_ref()
        .map(|c| (matched(c, rest), written_hour(written, c)))
    {
        Some((s, Some(end))) => (format!("{first}{s}"), Some(end)),
        _ => (first.to_owned(), None),
    };
    Some(Time {
        len: read.len(),
        valid: start < 24 && end.is_none_or(|h| h < 24),
        unlikely: unlikely(&read),
    })
}

/// French's meridiem: `a` for the morning, `p` for the afternoon.
fn french_meridiem(ap: &str, hour: u32) -> u32 {
    match ap.bytes().next().map(|b| b.to_ascii_lowercase()) {
        Some(b'a') if hour == 12 => 0,
        Some(b'p') if hour != 12 => hour + 12,
        _ => hour,
    }
}

/// German's meridiem words.
fn german_meridiem(ap: &str, hour: u32) -> u32 {
    let ap = ap.to_lowercase();
    let mut hour = hour;
    if (ap.contains("morgen") || ap.contains("vormittag")) && hour == 12 {
        hour = 0;
    }
    if (ap.contains("nachmittag") || ap.contains("abend")) && hour != 12 {
        hour += 12;
    }
    if ap.contains("nacht") {
        if hour == 12 {
            hour = 0;
        } else if hour >= 6 {
            hour += 12;
        }
    }
    hour
}

/// A written-hour parser: `prefix` before the hour, `tail` after the
/// seconds, `phase` between a range's ends.
fn written(
    prefix: &str,
    seconds: &str,
    tail: &str,
    phase: &str,
    meridiem: fn(&str, u32) -> u32,
) -> Written {
    let body = format!(
        r"(?P<h>[0-9]{{1,2}})(?:h|:)?(?:(?P<mi>[0-9]{{1,2}})(?:m|:)?)?(?:(?P<s>[0-9]{{1,2}}){seconds})?{tail}{POST_WORD}"
    );
    Written {
        first: compile(&format!("^{prefix}{body}")),
        second: compile(&format!(r"^\s*(?:{phase})\s*{body}")),
        meridiem,
    }
}

/// A within-format parser: `prefix`, then a run of lengths in hours,
/// minutes or seconds (`units`), each a number in digits, as chrono's
/// `TimeUnitWithinFormatParser` reads them; a length of days sets no hour.
fn within(prefix: &str, units: &'static [&'static str]) -> Option<Regex> {
    let one = format!(
        r"(?:[0-9]+|[0-9]+\.[0-9]+)\s{{0,5}}(?:{})\s{{0,5}}",
        crate::text::alternation(units.iter().copied())
    );
    compile(&format!(
        r"^(?:{prefix})\s*{one}(?:\s{{0,5}},?\s{{0,5}}{one}){{0,10}}{POST_WORD}"
    ))
}

/// A casual-word parser: `words` at the start, then `post`.
fn casual(words: &str, post: &str) -> Option<Regex> {
    compile(&format!("^(?:{words}){post}"))
}

/// The time parsers of every language the weekday check reads, by code.
static CLOCKS: LazyLock<[(&str, Clock); 11]> = LazyLock::new(|| {
    let english_suffix =
        r"(?:\s*(?:o[^A-Za-z0-9_]*clock|at\s*night|in\s*the\s*(?:morning|afternoon)))?";
    [
        (
            "en",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:at|from)\s*)??",
                    english_suffix,
                    POST,
                    r"\s*(?:\-|–|~|〜|to|until|through|till|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual(r"now|(?:this)?\s{0,3}(?:midnight|midday|noon)", POST_WORD),
                within: within(
                    r"(?:within|in|for)\s*(?:(?:about|around|roughly|approximately|just)\s*(?:~\s*)?)?",
                    &[
                        "s", "sec", "second", "seconds", "m", "min", "mins", "minute", "minutes",
                        "h", "hr", "hrs", "hour", "hours",
                    ],
                ),
            },
        ),
        (
            "fr",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:[àa])\s*)?",
                    "",
                    POST,
                    r"\s*(?:\-|–|~|〜|[àa]|\?)\s*",
                    true,
                )),
                written: Some(written(
                    r"(?:(?:[àa])\s*)?",
                    "(?:s|:)?",
                    r"(?:\s*(?P<ap>A\.M\.|P\.M\.|AM?|PM?))?",
                    r"\-|–|~|〜|[àa]|\?",
                    french_meridiem,
                )),
                casual: casual("maintenant", POST_WORD),
                within: within(
                    "dans|en|pour|pendant|de",
                    &[
                        "sec", "seconde", "secondes", "min", "mins", "minute", "minutes", "h",
                        "hr", "hrs", "heure", "heures",
                    ],
                ),
            },
        ),
        (
            "de",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:um|von)\s*)?",
                    "",
                    POST,
                    r"\s*(?:\-|–|~|〜|bis)\s*",
                    true,
                )),
                written: Some(written(
                    r"(?:(?:um|von)\s*)?",
                    "(?:s)?",
                    r"(?:\s*Uhr)?(?:\s*(?P<ap>morgens|vormittags|nachmittags|abends|nachts|am\s+(?:Morgen|Vormittag|Nachmittag|Abend)|in\s+der\s+Nacht))?",
                    r"\-|–|~|〜|bis(?:\s+um)?|\?",
                    german_meridiem,
                )),
                casual: casual("jetzt", POST_WORD),
                within: within(
                    "in|für|während",
                    &[
                        "sek", "sekunde", "sekunden", "min", "minute", "minuten", "h", "std",
                        "stunde", "stunden",
                    ],
                ),
            },
        ),
        (
            "es",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:aslas|deslas|las?|al?|de|del)\s*)?",
                    "",
                    POST,
                    r"\s*(?:\-|–|~|〜|a(?:l)?|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual("ahora", POST_WORD),
                within: within(
                    "en|por|durante|de|dentro de",
                    &[
                        "sec", "segundo", "segundos", "min", "mins", "minuto", "minutos", "h",
                        "hr", "hrs", "hora", "horas",
                    ],
                ),
            },
        ),
        (
            "it",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:alle?|dalle?)\s*)??",
                    r"(?:\s*(?:di\s*(?:sera|notte|mattina|pomeriggio)))?",
                    POST,
                    r"\s*(?:\-|–|~|〜|a|fino\s*a|alle?|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual(
                    r"adesso|ora|(?:questa\s*)?(?:mezzanotte|mezzogiorno)",
                    POST_WORD,
                ),
                within: within(
                    r"(?:entro|tra|fra|in|per)\s*(?:(?:circa|approssimativamente)\s*(?:~\s*)?)?",
                    &[
                        "s", "sec", "secondo", "secondi", "m", "min", "minuto", "minuti", "h",
                        "ora", "ore",
                    ],
                ),
            },
        ),
        (
            "pt",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:ao?|às?|das|da|de|do)\s*)?",
                    "",
                    POST,
                    r"\s*(?:\-|–|~|〜|a(?:o)?|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual("agora", POST_WORD),
                within: None,
            },
        ),
        (
            "nl",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:om)\s*)?",
                    r"(?:\s*(?:uur))?",
                    POST,
                    r"\s*(?:\-|–|~|〜|om|\?)\s*",
                    true,
                )),
                written: None,
                casual: casual("nu", POST_WORD),
                within: within(
                    "binnen|in|binnen de|voor",
                    &[
                        "sec", "second", "seconden", "min", "mins", "minute", "minuut", "minuten",
                        "minuutje", "h", "hr", "hrs", "uur", "u", "uren",
                    ],
                ),
            },
        ),
        (
            "sv",
            Clock {
                expression: None,
                written: None,
                casual: casual("nu", POST_WORD),
                within: None,
            },
        ),
        (
            "fi",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:klo|kello)\s*)?",
                    "",
                    POST,
                    r"\s*(?:\-|–|~|〜)\s*",
                    true,
                )),
                written: None,
                casual: casual("nyt", POST_WORD),
                within: None,
            },
        ),
        (
            "ru",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:в|с)\s*)??",
                    r"(?:\s*(?:утра|вечера|после полудня))?",
                    POST_CYRILLIC,
                    r"\s*(?:\-|–|~|〜|до|и|по|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual(
                    r"сейчас|следующей\s*ночью|сегодня\s*ночью|этой\s*ночью|ночью|в\s*полдень|в\s*полночь",
                    POST_CYRILLIC_WORD,
                ),
                within: None,
            },
        ),
        (
            "uk",
            Clock {
                expression: Some(Expression::new(
                    r"(?:(?:в|у|о|об|з|із|від)\s*)??",
                    r"(?:\s*(?:ранку|вечора|по обіді|після обіду))?",
                    POST_CYRILLIC,
                    r"\s*(?:\-|–|~|〜|до|і|по|\?)\s*",
                    false,
                )),
                written: None,
                casual: casual(
                    r"зараз|сьогодні\s*вночі|цієї\s*ночі|опівдні|опівночі|вночі",
                    POST_CYRILLIC_WORD,
                ),
                within: None,
            },
        ),
    ]
});

/// The time the language `code` reads at the very start of `text`, one
/// whose hour is written rather than implied: of two its parsers read, the
/// longer, as chrono's overlap removal keeps it. Japanese, Chinese and
/// Vietnamese time parsers are not ported, so they read none.
pub fn time_at_start(code: &str, text: &str) -> Option<Time> {
    let (_, clock) = CLOCKS.iter().find(|(c, _)| *c == code)?;
    let kept = longest_at(clock, text, 0, Boundary::Start)?;
    // chrono keeps the longer of two overlapping results, so a longer time
    // or date starting inside this one ("de 9h à 12h" holds "9h à 12h")
    // leaves nothing at the start.
    let dates = super::languages::LANGUAGES
        .iter()
        .find(|e| e.code == code)
        .map(|e| e.language().results(text))
        .unwrap_or_default();
    let date_outgrows = dates
        .iter()
        .any(|f| f.start > 0 && f.start < kept.len && f.end - f.start > kept.len);
    let time_outgrows = text
        .char_indices()
        .skip(1)
        .take_while(|&(at, _)| at < kept.len)
        .any(|(at, _)| {
            Boundary::at(text, at).is_some_and(|boundary| {
                longest_at(clock, text, at, boundary).is_some_and(|t| t.len > kept.len)
            })
        });
    (!date_outgrows && !time_outgrows).then_some(kept)
}

/// What stands before a position, as the parsers' left boundaries test it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    /// The start of the text: every parser may match.
    Start,
    /// Whitespace: every parser may match.
    Space,
    /// A `T`, as between an ISO date and its time: the time parsers only.
    Tee,
    /// Another ASCII non-word character: the word-boundary parsers (casual
    /// words, lengths) and the time expression.
    NonWord,
    /// A change between ASCII word and non-word characters the other way
    /// (a letter, then a digit is none): the time expression only.
    WordEdge,
}

impl Boundary {
    /// The boundary before byte `at` of `text`, None where no parser may start.
    fn at(text: &str, at: usize) -> Option<Self> {
        let before = crate::text::char_before(text, at)?;
        let here = crate::text::char_at(text, at)?;
        if before.is_whitespace() {
            return Some(Self::Space);
        }
        if before == 'T' {
            return Some(Self::Tee);
        }
        let word = crate::text::is_ascii_word;
        if !word(before) {
            return Some(Self::NonWord);
        }
        (!word(here)).then_some(Self::WordEdge)
    }
}

/// The longest time the clock's parsers read at byte `at` of `text`, given
/// what stands before it.
fn longest_at(clock: &Clock, text: &str, at: usize, boundary: Boundary) -> Option<Time> {
    let rest = text.get(at..)?;
    let worded = |re: Option<&Regex>| {
        re.and_then(|re| re.captures(rest)).map(|caps| Time {
            len: matched(&caps, rest).len(),
            valid: true,
            unlikely: false,
        })
    };
    let any = matches!(
        boundary,
        Boundary::Start | Boundary::Space | Boundary::NonWord
    );
    let spaced = matches!(boundary, Boundary::Start | Boundary::Space | Boundary::Tee);
    [
        clock
            .expression
            .as_ref()
            .and_then(|e| expression_at_start(e, rest)),
        clock
            .written
            .as_ref()
            .filter(|_| spaced)
            .and_then(|w| written_at_start(w, rest)),
        worded(clock.casual.as_ref().filter(|_| any)),
        worded(clock.within.as_ref().filter(|_| any)),
    ]
    .into_iter()
    .flatten()
    .reduce(|kept, next| if next.len > kept.len { next } else { kept })
}

/// Whether the language `code` reads a time of day at the very start of
/// `text` that survives on its own.
#[must_use]
pub fn hour_at_start(code: &str, text: &str) -> bool {
    time_at_start(code, text).is_some_and(|t| t.valid && !t.unlikely)
}

impl Time {
    /// Whether chrono drops the time, and a weekday merged into it, as no
    /// time a clock shows (24:00).
    pub const fn is_invalid(self) -> bool {
        !self.valid
    }
}
