//! The times of day chrono-node 2.10.2's parsers read (MIT): its time
//! expressions, French and German written hours ("17h30", "17 Uhr"), the
//! casual words that set an hour ("noon", "now"), Japanese, Chinese and
//! Vietnamese times (`east`) and lengths from now (`relative`), with the
//! dates and zones chrono's refiners merge into them. The weekday check asks
//! whether a text opens on one, after a bare day number; the window check
//! and the naming of a pair ask for the times beside a date.
//!
//! Each parser reads at one place, so each pattern is anchored; a lookahead
//! in chrono is a consumed `post` group, as in `languages`.

use std::sync::LazyLock;

use regex::{Captures, Regex};

mod east;
mod relative;

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
    /// Japanese's, Chinese's or Vietnamese's time expression.
    east: Option<fn(&str) -> Option<Time>>,
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

/// The hour and minute a primary time sets, as
/// `extractPrimaryTimeComponents` reads them, and whether its meridiem is
/// certain; None where chrono rejects it.
fn primary_hour(
    caps: &Captures<'_>,
    four_digits_is_year: bool,
    whole: &str,
) -> Option<(Clock24, bool)> {
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
    let second = group(caps, "s").and_then(digits);
    if second.is_some_and(|s| s >= 60) {
        return None;
    }
    let clock = Clock24::new(hour, minute, second.unwrap_or(0), certain);
    Some((clock, certain))
}

/// The second time of a range, as `extractFollowingTimeComponents` reads
/// it: its time, and the start's once its meridiem is settled.
fn following_hours(
    caps: &Captures<'_>,
    start: Clock24,
    start_certain: bool,
) -> Option<(Clock24, Clock24)> {
    if group(caps, "ms").is_some_and(|ms| leading_int(ms.get(..3).unwrap_or(ms)) >= 1000) {
        return None;
    }
    let second = group(caps, "s").and_then(digits);
    if second.is_some_and(|s| s >= 60) {
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
            if start.hour != 12 {
                start.hour += 12;
            }
        } else if start.hour == 12 {
            start.hour = 0;
        }
    }
    if !set && start_certain && start.hour > 12 && start.hour - 12 <= hour && hour <= 12 {
        hour += 12;
    }
    Some((Clock24::new(hour, minute, second.unwrap_or(0), set), start))
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

/// A time of day as a parser sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Clock24 {
    /// The hour, 0 to 24.
    hour: u32,
    /// The minute.
    minute: u32,
    /// The second.
    second: u32,
    /// Whether the text writes its meridiem.
    meridiem: bool,
}

impl Clock24 {
    /// A time from its parts.
    const fn new(hour: u32, minute: u32, second: u32, meridiem: bool) -> Self {
        Self {
            hour,
            minute,
            second,
            meridiem,
        }
    }

    /// Whether it comes before `other` on the clock.
    const fn before(self, other: Self) -> bool {
        self.hour < other.hour
            || (self.hour == other.hour
                && (self.minute < other.minute
                    || (self.minute == other.minute && self.second < other.second)))
    }
}

/// A result a time parser read at the start of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one of chrono's components, set apart from the others"
)]
pub struct Time {
    /// Its length in bytes, which decides which of two overlapping times stays.
    len: usize,
    /// Whether its hours exist on a clock: chrono drops a result at 24:00,
    /// and with it a weekday the time was merged into.
    valid: bool,
    /// Whether its text is only digits and a dot, which chrono's last filter
    /// drops when nothing was merged into it.
    unlikely: bool,
    /// Whether it sets an hour; a length of days from now sets only a date.
    hour: bool,
    /// Whether it names its day too ("明天下午3点", "in 2 hours"), so no date
    /// merges into it.
    dated: bool,
    /// For a range of times, whether its end falls before its start on the
    /// clock, which puts the end on the next day.
    range: Option<bool>,
    /// Whether it sets its zone, so no zone written after it is taken in.
    zoned: bool,
}

/// The time expression's result at the start of `text`, if one passes the
/// parser's own checks.
fn expression_at_start(expression: &Expression, text: &str) -> Option<Time> {
    let shapes = &*SHAPES;
    let caps = expression.primary.as_ref()?.captures(text)?;
    let primary = matched(&caps, text);
    let (start, certain) = primary_hour(&caps, expression.four_digits_is_year, primary)?;
    let alone = |clock: Clock24| {
        stands_alone(shapes, primary).then(|| Time::clock(primary.len(), text, clock, None))
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
    stands_as_range(shapes, read_text).then(|| Time::clock(read_text.len(), read_text, start, end))
}

/// The time French's or German's written-hour parser reads, None where it
/// rejects the match.
fn written_hour(written: &Written, caps: &Captures<'_>) -> Option<Clock24> {
    let mut hour = digits(group(caps, "h")?)?;
    let minute = group(caps, "mi").map_or(Some(0), digits)?;
    if minute >= 60 || hour > 24 {
        return None;
    }
    let ap = group(caps, "ap");
    if let Some(ap) = ap {
        if hour > 12 {
            return None;
        }
        hour = (written.meridiem)(ap, hour);
    }
    let second = group(caps, "s").map_or(Some(0), digits)?;
    if second >= 60 {
        return None;
    }
    Some(Clock24::new(hour, minute, second, ap.is_some()))
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
    match second
        .as_ref()
        .map(|c| (matched(c, rest), written_hour(written, c)))
    {
        Some((s, Some(end))) => Some(Time::clock(first.len() + s.len(), text, start, Some(end))),
        _ => Some(Time::clock(first.len(), text, start, None)),
    }
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

/// A casual-word parser: `words` at the start, then `post`.
fn casual(words: &str, post: &str) -> Option<Regex> {
    compile(&format!("^(?:{words}){post}"))
}

/// The time parsers of every language the weekday check reads, by code.
static CLOCKS: LazyLock<[(&str, Clock); 14]> = LazyLock::new(|| {
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
                east: None,
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
                east: None,
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
                east: None,
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
                east: None,
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
                east: None,
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
                east: None,
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
                east: None,
            },
        ),
        (
            "sv",
            Clock {
                expression: None,
                written: None,
                casual: casual("nu", POST_WORD),
                east: None,
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
                east: None,
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
                east: None,
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
                east: None,
            },
        ),
        (
            "ja",
            Clock {
                expression: None,
                written: None,
                casual: None,
                east: Some(east::japanese),
            },
        ),
        (
            "zh",
            Clock {
                expression: None,
                written: None,
                casual: None,
                east: Some(|text| east::chinese(text).map(|(time, dated)| Time { dated, ..time })),
            },
        ),
        (
            "vi",
            Clock {
                expression: None,
                written: None,
                casual: casual(r"bây giờ|lúc này", POST_WORD),
                east: Some(east::vietnamese),
            },
        ),
    ]
});

/// What the language `code` reads at the very start of `text`, as chrono's
/// first result there: of the times, lengths from now, casual dates and
/// dates its parsers read at the start, the longest, unless a longer one
/// starting inside it removes it (chrono's overlap removal: "de 9h à 12h"
/// holds "9h à 12h", which does not start the text). A zone written after
/// it is taken into it, and a date with no time takes the time written
/// after it ("tomorrow 15", "Nov 5 at 3pm").
pub fn time_at_start(code: &str, text: &str) -> Option<Time> {
    let (_, clock) = CLOCKS.iter().find(|(c, _)| *c == code)?;
    let dates = super::languages::LANGUAGES
        .iter()
        .find(|e| e.code == code)
        .map(|e| e.language().results(text))
        .unwrap_or_default();
    let date_at = |at: usize| {
        dates
            .iter()
            .filter(|f| f.start == at)
            .max_by_key(|f| f.end)
            .map(|f| date_time(f, text.get(at..).unwrap_or_default()))
    };
    let longest = |a: Option<Time>, b: Option<Time>| match (a, b) {
        (Some(a), Some(b)) => Some(longer(text, a, b)),
        (a, b) => a.or(b),
    };
    let first = [
        iso_at(text),
        date_at(0),
        longest_at(code, clock, text, 0, Boundary::Start),
        casual_date_at(code, text, true, true),
    ]
    .into_iter()
    .fold(None, longest)?;
    let first_width = width(text.get(..first.len).unwrap_or(text));
    let date_outgrows = dates.iter().any(|f| {
        f.start > 0
            && f.start < first.len
            && text
                .get(f.start..f.end)
                .is_some_and(|d| width(d) > first_width)
    });
    let time_outgrows = text
        .char_indices()
        .skip(1)
        .take_while(|&(at, _)| at < first.len)
        .any(|(at, _)| {
            Boundary::at(text, at).is_some_and(|boundary| {
                longest_at(code, clock, text, at, boundary).is_some_and(|t| {
                    text.get(at..at.saturating_add(t.len))
                        .is_some_and(|inner| width(inner) > first_width)
                })
            })
        });
    if date_outgrows || time_outgrows {
        return None;
    }
    let first = with_offset(code, text, first);
    let next = next_result(code, clock, text, &dates, first.len);
    Some(with_zone_name(text, refined(code, text, first, next)))
}

/// The result a date parser's reading makes, at the start of `text`: one
/// with a day or month of 0 is dropped with whatever merges into it.
fn date_time(found: &super::languages::Found, text: &str) -> Time {
    let date = Time::date(found.end.saturating_sub(found.start), text);
    Time {
        valid: !matches!(found.reading, super::languages::Reading::Void),
        ..date
    }
}

/// What a date result holds for a range's year rules: its day, its month,
/// whether it writes its year, and whether it is a weekday alone.
#[derive(Debug, Clone, Copy)]
struct Dated {
    /// The day of the month, 0 for none.
    day: u8,
    /// The month, 0 for none.
    month: u8,
    /// Whether the text writes the year.
    year_written: bool,
    /// Whether it is a weekday and nothing else.
    weekday_only: bool,
}

/// The result after `from` in `text`: the longest of what the language's
/// parsers read where the first of them starts, a dozen characters on at
/// most, with what a date result holds.
fn next_result(
    code: &str,
    clock: &Clock,
    text: &str,
    dates: &[super::languages::Found],
    from: usize,
) -> Option<(usize, Time, Option<Dated>)> {
    use super::languages::{Reading, YearOf};
    let rest = text.get(from..)?;
    rest.char_indices()
        .map(|(at, _)| from.saturating_add(at))
        .take(16)
        .find_map(|at| {
            let here = text.get(at..)?;
            let boundary = Boundary::at(text, at);
            let word_ok = matches!(
                boundary,
                Some(Boundary::Start | Boundary::Space | Boundary::NonWord)
            );
            let unicode_ok = crate::text::char_before(text, at)
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            let date = dates
                .iter()
                .filter(|f| f.start == at)
                .max_by_key(|f| f.end)
                .map(|f| {
                    let held = match f.reading {
                        Reading::Date { day, month, year }
                        | Reading::Dated {
                            day, month, year, ..
                        } => Dated {
                            day,
                            month,
                            year_written: matches!(year, YearOf::Written(_)),
                            weekday_only: false,
                        },
                        Reading::Weekday(_) => Dated {
                            day: 0,
                            month: 0,
                            year_written: false,
                            weekday_only: true,
                        },
                        Reading::Other | Reading::Void => Dated {
                            day: 0,
                            month: 0,
                            year_written: true,
                            weekday_only: false,
                        },
                    };
                    (date_time(f, here), Some(held))
                });
            let casual = casual_date_at(code, here, word_ok, unicode_ok).map(|t| (t, None));
            let time = boundary
                .and_then(|b| longest_at(code, clock, text, at, b))
                .map(|t| (with_offset(code, here, t), None));
            [date, time, casual]
                .into_iter()
                .flatten()
                .reduce(|kept, next| {
                    if longer(here, kept.0, next.0) == next.0 && kept.0 != next.0 {
                        next
                    } else {
                        kept
                    }
                })
                .map(|(t, held)| (at, t, held))
        })
}

/// The languages whose range refiner runs before their date-and-time one.
const RANGE_FIRST: &[&str] = &["de", "fi", "zh", "vi"];

/// The result at the start once chrono's refiners merge it with the one
/// after it: a date and a time the language's words join (`mergeDateTimeResult`),
/// or two results a range word joins (`AbstractMergeDateRangeRefiner`), in
/// the language's order. A range starts on its earlier end: a time before a
/// date it falls on, unless the date, read in the year nearest 1970-01-01
/// (the instant the hour check reads from), is the later one and is swapped
/// in.
fn refined(
    code: &str,
    text: &str,
    first: Time,
    next: Option<(usize, Time, Option<Dated>)>,
) -> Time {
    let Some((at, next, held)) = next else {
        return first;
    };
    let gap = text.get(first.len..at).unwrap_or_default();
    let len = at.saturating_add(next.len);
    let merged = |hour: bool| Time {
        len,
        valid: first.valid && next.valid,
        unlikely: unlikely(text.get(..len).unwrap_or(text)),
        hour,
        dated: true,
        range: if first.hour && next.hour {
            None
        } else {
            next.range.or(first.range)
        },
        zoned: first.zoned || next.zoned,
    };
    let range_gap = super::window::joins_range(code, gap);
    let range = || {
        let starts_on_time = first.hour
            && (next.hour
                || held.is_none_or(|d| {
                    d.weekday_only || d.year_written || epoch_year(d.month, d.day) == Some(1970)
                }));
        merged(starts_on_time)
    };
    let range_first = RANGE_FIRST.contains(&code);
    if range_first && range_gap {
        return range();
    }
    let time_only = |t: Time| t.hour && !t.dated;
    let date_gap = MERGE_GAPS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, re)| re.as_ref())
        .is_some_and(|re| re.is_match(gap));
    if date_gap && !first.hour && time_only(next) {
        return merged(true);
    }
    // "24: July 23" is a count before a date, not a time; chrono's
    // refiner keeps a bare colon to the date-then-time order.
    if date_gap && time_only(first) && !next.hour && gap.trim() != ":" {
        return merged(true);
    }
    if !range_first && range_gap {
        return range();
    }
    first
}

/// The year chrono gives a date with no year read from 1970-01-01.
fn epoch_year(month: u8, day: u8) -> Option<i16> {
    let epoch = jiff::civil::Date::new(1970, 1, 1).ok()?;
    super::calendar::nearest_year(epoch, month, day)
}

/// What may stand between a date and the time a language's refiner merges
/// into it, by code; Swedish merges none.
static MERGE_GAPS: LazyLock<Vec<(&str, Option<Regex>)>> = LazyLock::new(|| {
    [
        ("en", r"^\s*(?:T|at|after|before|on|of|,|-|\.|∙|:)?\s*$"),
        ("fr", r"^\s*(?:T|à|a|au|vers|de|,|-)?\s*$"),
        ("de", r"^\s*(?:T|um|am|,|-)?\s*$"),
        ("es", r"^\s*(?:,|de|aslas|a)?\s*$"),
        (
            "it",
            r"^\s*(?:T|alle?|dopo\s*le|prima\s*delle?|,|-|\.|∙|:)?\s*$",
        ),
        ("pt", r"^\s*(?:,|à)?\s*$"),
        ("nl", r"^\s*(?:om|na|voor|in de|,|-)?\s*$"),
        ("fi", r"^\s*(?:T|klo|kello|,|-)?\s*$"),
        ("ru", r"^\s*(?:T|в|,|-)?\s*$"),
        ("uk", r"^\s*(?:T|в|у|о|,|-)?\s*$"),
        ("ja", r"(?i)^\s*(?:の)?\s*$"),
        ("zh", r"^\s*$"),
        ("vi", r"^\s*(?:lúc|vào|,|T|-)?\s*$"),
    ]
    .into_iter()
    .map(|(code, re)| (code, crate::text::compile(re)))
    .collect()
});

/// chrono's zone offset after a result ("+02:00", "GMT+1", "(UTC-3)"):
/// `ExtractTimezoneOffsetRefiner`, which Chinese leaves out.
static OFFSET: LazyLock<Option<Regex>> = LazyLock::new(|| {
    crate::text::compile(
        r"(?i)^\s*(?:\(?(?:GMT|UTC)\s?)?[+-](?P<h>[0-9]{1,2})(?::?(?P<m>[0-9]{2}))?\)?",
    )
});

/// `time` with the zone offset written after it taken in, where chrono
/// takes it: an offset of at most 14 hours, after a result that sets no
/// zone of its own.
fn with_offset(code: &str, text: &str, time: Time) -> Time {
    if code == "zh" || time.zoned {
        return time;
    }
    let rest = text.get(time.len..).unwrap_or_default();
    let Some(caps) = OFFSET.as_ref().and_then(|re| re.captures(rest)) else {
        return time;
    };
    let value = |name: &str| {
        caps.name(name)
            .and_then(|m| digits(m.as_str()))
            .unwrap_or(0)
    };
    if value("h").saturating_mul(60).saturating_add(value("m")) > 14 * 60 {
        return time;
    }
    let len = time.len.saturating_add(caps.get(0).map_or(0, |m| m.end()));
    Time {
        len,
        unlikely: unlikely(text.get(..len).unwrap_or(text)),
        zoned: true,
        ..time
    }
}

/// chrono's zone name after a result: `ExtractTimezoneAbbrRefiner`.
static ZONE_NAME: LazyLock<Option<Regex>> = LazyLock::new(|| {
    crate::text::compile(r"(?i)^\s*,?\s*\(?(?P<name>[A-Z]{2,4})\)?(?P<post>[^A-Za-z0-9_]|$)")
});

/// `time` with the zone name written after it taken in, where chrono takes
/// it: a name it knows, written in capitals after a date with no time, and
/// one at UTC after a result already zoned at UTC (a length from now, "now").
fn with_zone_name(text: &str, time: Time) -> Time {
    let rest = text.get(time.len..).unwrap_or_default();
    let Some(caps) = ZONE_NAME.as_ref().and_then(|re| re.captures(rest)) else {
        return time;
    };
    let Some(name) = caps.name("name").map(|m| m.as_str()) else {
        return time;
    };
    let upper = name.to_ascii_uppercase();
    if !ZONE_NAMES.contains(&upper.as_str()) {
        return time;
    }
    let written_upper = upper == name;
    let at_utc = UTC_NAMES.contains(&upper.as_str());
    if (time.zoned && !at_utc) || (!time.hour && !written_upper) {
        return time;
    }
    let len = time.len.saturating_add(
        caps.name("post")
            .map_or_else(|| caps.get(0).map_or(0, |m| m.end()), |m| m.start()),
    );
    Time {
        len,
        unlikely: unlikely(text.get(..len).unwrap_or(text)),
        zoned: true,
        ..time
    }
}

/// The zone names chrono knows at UTC.
const UTC_NAMES: &[&str] = &["EGST", "GMT", "UTC", "WET", "WEZ", "WT"];

/// The two- to four-letter zone names chrono knows.
const ZONE_NAMES: &[&str] = &[
    "ACDT", "ACST", "ADT", "AEDT", "AEST", "AFT", "AKDT", "AKST", "ALMT", "AMST", "AMT", "ANAT",
    "AQTT", "ART", "AST", "AWDT", "AWST", "AZOT", "AZST", "AZT", "BNT", "BOT", "BRST", "BRT",
    "BST", "BTT", "CAST", "CAT", "CCT", "CDT", "CEST", "CET", "CKT", "CLST", "CLT", "COT", "CST",
    "CT", "CVT", "CXT", "DAVT", "EAST", "EAT", "ECT", "EDT", "EEST", "EET", "EGST", "EGT", "EST",
    "ET", "FJST", "FJT", "FKST", "FKT", "FNT", "GALT", "GAMT", "GET", "GFT", "GILT", "GMT", "GST",
    "GYT", "HAA", "HAC", "HADT", "HAE", "HAP", "HAR", "HAST", "HAT", "HAY", "HKT", "HLV", "HNA",
    "HNC", "HNE", "HNP", "HNR", "HNT", "HNY", "HOVT", "ICT", "IDT", "IOT", "IRDT", "IRKT", "IRST",
    "IST", "JST", "KGT", "KRAT", "KST", "KUYT", "LHDT", "LHST", "LINT", "MAGT", "MART", "MAWT",
    "MDT", "MESZ", "MEZ", "MHT", "MMT", "MSD", "MSK", "MST", "MT", "MUT", "MVT", "MYT", "NCT",
    "NDT", "NFT", "NOVT", "NPT", "NST", "NUT", "NZDT", "NZST", "OMST", "PDT", "PET", "PETT", "PGT",
    "PHOT", "PHT", "PKT", "PMDT", "PMST", "PONT", "PST", "PT", "PWT", "PYST", "PYT", "RET", "SAMT",
    "SAST", "SBT", "SCT", "SGT", "SRT", "SST", "TAHT", "TFT", "TJT", "TKT", "TLT", "TMT", "TVT",
    "ULAT", "UTC", "UYST", "UYT", "UZT", "VET", "VLAT", "VUT", "WAST", "WAT", "WEST", "WESZ",
    "WET", "WEZ", "WFT", "WGST", "WGT", "WIB", "WIT", "WITA", "WST", "WT", "YAKT", "YAPT", "YEKT",
];

/// chrono's ISO 8601 date at the start of `text`, with its time if it
/// writes one: `ISOFormatParser`.
static ISO: LazyLock<Option<Regex>> = LazyLock::new(|| {
    crate::text::compile(
        r"(?i)^(?P<y>[0-9]{4})\-(?P<mo>[0-9]{1,2})\-(?P<d>[0-9]{1,2})(?:T(?P<h>[0-9]{1,2}):(?P<mi>[0-9]{1,2})(?::(?P<s>[0-9]{1,2})(?:\.[0-9]{1,4})?)?(?P<z>Z|[+-][0-9]{2}:?(?:[0-9]{2})?)?)?(?P<post>[^A-Za-z0-9_]|$)",
    )
});

/// An ISO 8601 date and time at the start of `text`, None for one without
/// a time, which the date parsers read.
fn iso_at(text: &str) -> Option<Time> {
    let caps = ISO.as_ref()?.captures(text)?;
    let hour = digits(caps.name("h")?.as_str())?;
    let value = |name: &str| caps.name(name).and_then(|m| digits(m.as_str()));
    let (year, month, day) = (value("y")?, value("mo")?, value("d")?);
    let minute = value("mi")?;
    let second = value("s").unwrap_or(0);
    let date = i16::try_from(year)
        .ok()
        .zip(i8::try_from(month).ok())
        .zip(i8::try_from(day).ok())
        .and_then(|((y, m), d)| jiff::civil::Date::new(y, m, d).ok());
    let len = caps
        .name("post")
        .map_or_else(|| caps.get(0).map_or(0, |m| m.end()), |m| m.start());
    Some(Time {
        len,
        valid: date.is_some() && hour < 24 && minute < 60 && second < 60,
        unlikely: false,
        hour: true,
        dated: true,
        range: None,
        zoned: caps.name("z").is_some(),
    })
}

/// The casual date a language reads at the start of `text` ("tomorrow",
/// "demain", "heute Abend", "明日"), or a part of the day ("this morning")
/// that only implies its hour: a date with no time.
fn casual_date_at(code: &str, text: &str, word_ok: bool, unicode_ok: bool) -> Option<Time> {
    let (_, re, unicode) = CASUAL_DATES.iter().find(|(c, _, _)| *c == code)?;
    if !(if *unicode { unicode_ok } else { word_ok }) {
        return None;
    }
    let caps = re.as_ref()?.captures(text)?;
    let len = caps
        .name("post")
        .map_or_else(|| caps.get(0).map_or(0, |m| m.end()), |m| m.start());
    (len > 0).then(|| Time::date(len, text))
}

/// Each language's casual dates and parts of the day that set no hour, and
/// whether its boundaries are Unicode's.
static CASUAL_DATES: LazyLock<Vec<(&str, Option<Regex>, bool)>> = LazyLock::new(|| {
    [
        ("en", r"(?:today|tonight|tomorrow|overmorrow|tmr|tmrw|yesterday|last\s*night|(?:this)?\s{0,3}(?:morning|afternoon|evening|night))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("fr", r"(?:aujourd'hui|demain|hier|cette\s*nuit|la\s*veille|(?:cet?)?\s*(?:matin|soir|après-midi|aprem|a midi|à minuit))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("de", r"(?:(?:heute|morgen|übermorgen|uebermorgen|gestern|vorgestern|letzte\s*nacht)(?:\s*(?:morgen|vormittag|mittags?|nachmittag|abend|nacht|mitternacht))?|(?:diesen)?\s*(?:morgen|vormittag|mittags?|nachmittag|abend|nacht|mitternacht))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("es", r"(?:hoy|mañana|ayer|(?:esta\s*)?(?:mañana|tarde|medianoche|mediodia|mediodía|noche))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("it", r"(?:stamattina|stasera|stanotte|stamani|(?:oggi|stasera|stanotte|domani|dopodomani|ieri|ieri\s*sera|ieri\s*notte|(?:questa\s*)?(?:mattina|pomeriggio|sera|notte))(?P<post>[^A-Za-z0-9_]|$))", false),
        ("pt", r"(?:hoje|amanha|amanhã|ontem|(?:esta\s*)?(?:manha|manhã|tarde|meia-noite|meio-dia|noite))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("nl", r"(?:vandaag|morgen|morgend|gisteren|(?:gisteren|morgen|van)(?:ochtend|middag|namiddag|avond|nacht)|(?:deze)?\s*(?:namiddag|avond|middernacht|ochtend|middag|'s middags|'s avonds|'s ochtends))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("sv", r"(?:idag|imorgon|övermorgon|igår|förrgår|i\s*förrgår)(?:\s*(?:på\s*)?(?:morgonen?|förmiddagen?|middagen?|eftermiddagen?|kvällen?|natten?|midnatt))?(?P<post>[^A-Za-z0-9_]|$)", false),
        ("fi", r"(?:(?:tänään|huomenna|ylihuomenna|eilen|toissapäivänä|viime\s*yönä)(?:\s*(?:aamulla|aamuna|aamupäivällä|päivällä|iltapäivällä|illalla|yöllä|keskiyöllä))?|(?:tänä\s*)?(?:aamulla|aamuna|aamupäivällä|päivällä|iltapäivällä|illalla|yöllä|keskiyöllä))(?P<post>[^A-Za-z0-9_]|$)", false),
        ("ru", r"(?:(?:с|со)?\s*(?:сегодня|вчера|завтра|послезавтра|послепослезавтра|позапозавчера|позавчера)|прошлым\s*вечером|прошлой\s*ночью|этим утром|утром|утра|вечером|вечера)(?P<post>[^\p{L}\p{N}_]|$)", true),
        ("uk", r"(?:(?:з|із|від)?\s*(?:сьогодні|вчора|завтра|післязавтра|післяпіслязавтра|позапозавчора|позавчора)|минулого\s*вечора|минулої\s*ночі|наступної\s*ночі|цього ранку|вранці|ранку|зранку|ввечері|вечора)(?P<post>[^\p{L}\p{N}_]|$)", true),
        ("ja", r"(?:今日|きょう|本日|ほんじつ|昨日|きのう|明日|あした|今夜|こんや|今夕|こんゆう|今晩|こんばん|今朝|けさ)", false),
        ("zh", r"(?:而家|立(?:刻|即)|即刻|(?:今|明|前|大前|後|大後|聽|昨|尋|琴)(?:早|朝|晚)|(?:上(?:午|晝)|朝(?:早)|早(?:上)|下(?:午|晝)|晏(?:晝)|晚(?:上)|夜(?:晚)?|中(?:午)|凌(?:晨))|(?:今|明|前|大前|後|大後|聽|昨|尋|琴)(?:日|天)(?:[\s|,|，]*)(?:上(?:午|晝)|朝(?:早)|早(?:上)|下(?:午|晝)|晏(?:晝)|晚(?:上)|夜(?:晚)?|中(?:午)|凌(?:晨))?)", false),
        ("vi", r"(?:hôm nay|hôm qua|hôm kia|ngày mai|ngày kia|(?:buổi\s*)?(?:sáng sớm|sáng|trưa|chiều|tối|đêm|nửa đêm|bình minh))(?P<post>[^A-Za-z0-9_]|$)", false),
    ]
    .into_iter()
    .map(|(code, re, unicode)| (code, crate::text::compile(&format!("(?i)^{re}")), unicode))
    .collect()
});

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

/// The casual words for the present moment.
const NOW_WORDS: &[&str] = &[
    "now",
    "maintenant",
    "jetzt",
    "ahora",
    "adesso",
    "ora",
    "agora",
    "nu",
    "nyt",
    "сейчас",
    "зараз",
    "bây giờ",
    "lúc này",
];

/// The longest time the clock's parsers read at byte `at` of `text`, given
/// what stands before it.
fn longest_at(
    code: &str,
    clock: &Clock,
    text: &str,
    at: usize,
    boundary: Boundary,
) -> Option<Time> {
    let rest = text.get(at..)?;
    let before = crate::text::char_before(text, at);
    let unicode_ok = before.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    let worded = |re: Option<&Regex>, haystack: &str| {
        re.and_then(|re| re.captures(haystack)).map(|caps| {
            let word = matched(&caps, haystack);
            // "now" sets the date and the zone too; "noon" and "midnight"
            // only the hour.
            let now = NOW_WORDS.contains(&word.to_lowercase().as_str());
            Time {
                len: word.len(),
                valid: true,
                unlikely: false,
                hour: true,
                dated: now,
                range: None,
                zoned: now,
            }
        })
    };
    let any = matches!(
        boundary,
        Boundary::Start | Boundary::Space | Boundary::NonWord
    );
    let spaced = matches!(boundary, Boundary::Start | Boundary::Space | Boundary::Tee);
    // chrono's `(\W|^)` takes a leading non-word character first, so a
    // parser that reads after it starts its result there, not at the start.
    let first = rest.chars().next();
    let after = |shifts: bool| {
        first
            .filter(|_| at == 0 && shifts)
            .and_then(|c| rest.get(c.len_utf8()..))
    };
    let ascii_after = after(first.is_some_and(|c| !crate::text::is_ascii_word(c)));
    let unicode_after = after(first.is_some_and(|c| !(c.is_alphanumeric() || c == '_')));
    let word = |read: &dyn Fn(&str) -> Option<Time>| {
        if !any || ascii_after.is_some_and(|a| read(a).is_some()) {
            return None;
        }
        read(rest)
    };
    let expression_ok = !matches!(code, "ru" | "uk") || unicode_ok || before == Some('T');
    let relative_word =
        any && ascii_after.is_none_or(|a| relative::at_start(code, a, true, false).is_none());
    let relative_unicode = unicode_ok
        && unicode_after.is_none_or(|a| relative::at_start(code, a, false, true).is_none());
    // Every time expression holds a digit within its first bytes, past
    // its prefix ("a las ", "kello "): a place without one reads none.
    let digit_within = |n: usize| rest.bytes().take(n).any(|b| b.is_ascii_digit());
    [
        clock
            .expression
            .as_ref()
            .filter(|_| expression_ok && digit_within(16))
            .and_then(|e| expression_at_start(e, rest)),
        clock
            .written
            .as_ref()
            .filter(|_| spaced && digit_within(12))
            .and_then(|w| written_at_start(w, rest)),
        word(&|t| worded(clock.casual.as_ref(), t)),
        clock.east.and_then(|read| {
            if code == "zh" {
                read(rest)
            } else {
                word(&read)
            }
        }),
        relative::at_start(code, rest, relative_word, relative_unicode),
        relative::deadline(code, rest),
    ]
    .into_iter()
    .flatten()
    .reduce(|kept, next| longer(rest, kept, next))
}

/// The length of a text as JavaScript counts it, in UTF-16 units, which
/// decides which of two overlapping results chrono keeps.
fn width(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// Of two results at the start of `text`, the one chrono keeps: the
/// longer, the first of two as long.
fn longer(text: &str, kept: Time, next: Time) -> Time {
    let span = |t: Time| width(text.get(..t.len).unwrap_or(text));
    if span(next) > span(kept) { next } else { kept }
}

/// Whether the language `code` reads a time of day at the very start of
/// `text` that survives on its own.
#[must_use]
pub fn hour_at_start(code: &str, text: &str) -> bool {
    let survives = |t: Time| t.hour && t.valid && !t.unlikely;
    time_at_start(code, text).is_some_and(survives)
        || after_weekday(code, text)
            .and_then(|rest| time_at_start(code, rest))
            .is_some_and(|t| t.dated && survives(t))
}

/// The text after the weekday name that opens `text` in the language
/// `code`, its abbreviation's dot, a comma and the spaces after it ("Sun,
/// 2026-11-01 15:00" gives "2026-11-01 15:00"): chrono merges such a weekday
/// into the date and time after it, so the line opens on that time.
fn after_weekday<'t>(code: &str, text: &'t str) -> Option<&'t str> {
    let entry = super::languages::LANGUAGES
        .iter()
        .find(|e| e.code == code)?;
    let name_len = entry
        .names
        .iter()
        .filter(|name| {
            text.get(..name.len())
                .is_some_and(|head| crate::text::same_word(head, name))
        })
        .map(|name| name.len())
        .max()?;
    let rest = text.get(name_len..)?;
    if rest.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    let rest = rest.strip_prefix('.').unwrap_or(rest);
    let rest = rest.strip_prefix(',').unwrap_or(rest);
    let trimmed = rest.trim_start();
    (trimmed.len() < rest.len()).then_some(trimmed)
}

impl Time {
    /// A time of day `len` bytes long at the start of `text`, from `start`
    /// to `end` for a range.
    fn clock(len: usize, text: &str, start: Clock24, end: Option<Clock24>) -> Self {
        Self {
            len,
            valid: start.hour < 24 && end.is_none_or(|e| e.hour < 24),
            unlikely: unlikely(text.get(..len).unwrap_or(text)),
            hour: true,
            dated: false,
            range: end.map(|e| e.before(start)),
            zoned: false,
        }
    }

    /// A date with no time, `len` bytes long at the start of `text`.
    fn date(len: usize, text: &str) -> Self {
        Self {
            len,
            valid: true,
            unlikely: unlikely(text.get(..len).unwrap_or(text)),
            hour: false,
            dated: true,
            range: None,
            zoned: false,
        }
    }

    /// Whether chrono drops the time, and a weekday merged into it, as no
    /// time a clock shows (24:00).
    pub const fn is_invalid(self) -> bool {
        !self.valid
    }
}

/// A time of day a language reads inside a text, for the readers that merge
/// it into the date beside it.
#[derive(Debug, Clone, Copy)]
pub struct Clocked {
    /// Byte offset of its text.
    pub start: usize,
    /// One past its last byte, as its parser read it.
    pub end: usize,
    /// One past its last byte with a zone offset after it, which chrono
    /// takes in once overlapping results are removed.
    pub zoned_end: usize,
    /// For a range of times, whether its end falls on the next day.
    pub next_day: Option<bool>,
    /// Whether it is a bare number ("15"), which chrono reads as an hour
    /// beside a date but a reader does not.
    pub bare: bool,
    /// Whether its hours exist on a clock.
    pub valid: bool,
    /// Whether its text is only digits and a dot.
    pub unlikely: bool,
}

/// The times of day the language `code` reads in `text` at the byte
/// offsets `starts`, each the longest its parsers read there.
pub fn times_at(code: &str, text: &str, starts: impl Iterator<Item = usize>) -> Vec<Clocked> {
    let Some((_, clock)) = CLOCKS.iter().find(|(c, _)| *c == code) else {
        return Vec::new();
    };
    let mut out: Vec<Clocked> = Vec::new();
    for at in starts {
        if out.last().is_some_and(|t| at < t.end) {
            continue;
        }
        let boundary = if at == 0 {
            Some(Boundary::Start)
        } else {
            Boundary::at(text, at)
        };
        let Some(time) = boundary.and_then(|b| longest_at(code, clock, text, at, b)) else {
            continue;
        };
        if !time.hour || time.dated {
            continue;
        }
        let here = text.get(at..).unwrap_or_default();
        let written = here.get(..time.len).unwrap_or_default();
        let zoned = with_offset(code, here, time);
        out.push(Clocked {
            start: at,
            end: at.saturating_add(time.len),
            zoned_end: at.saturating_add(zoned.len),
            next_day: time.range,
            bare: written.trim().bytes().all(|b| b.is_ascii_digit()),
            valid: time.valid,
            unlikely: time.unlikely,
        });
    }
    out
}

/// Whether `text` opens on a signed length ("+11mins", "- 2 days") that the
/// language `code` merges into the time before it, as chrono's English and
/// Italian relative-after-date refiners do: the pair then names its own day.
pub fn signed_length_at_start(code: &str, text: &str) -> bool {
    let rest = text.trim_start();
    matches!(code, "en" | "it")
        && rest.starts_with(['+', '-'])
        && relative::at_start(code, rest, true, true).is_some()
}

/// The date a length from now written at byte `at` of `text` lands on
/// ("3 days ago", "in 2 weeks"), counted from `today`, and where it ends.
pub fn length_date_at(
    code: &str,
    text: &str,
    at: usize,
    today: jiff::civil::Date,
) -> Option<(usize, jiff::civil::Date)> {
    let rest = text.get(at..)?;
    let before = crate::text::char_before(text, at);
    let word_ok = before.is_none_or(|c| !crate::text::is_ascii_word(c));
    let unicode_ok = before.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    let (len, date) = relative::date_at_start(code, rest, word_ok, unicode_ok, today)?;
    Some((at.saturating_add(len), date))
}

/// Whether `between` lets the language `code` merge a date and a time.
pub fn joins_date_and_time(code: &str, between: &str) -> bool {
    MERGE_GAPS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, re)| re.as_ref())
        .is_some_and(|re| re.is_match(between))
}

/// The bytes of the zone name chrono takes in after a result that ends at
/// `end` of `text` ("CET", "(UTC)"); `timed` says whether the result
/// writes a time, without which the name must be in capitals.
pub fn zone_name_len(text: &str, end: usize, timed: bool) -> usize {
    let Some(rest) = text.get(end..) else {
        return 0;
    };
    let time = Time {
        len: 0,
        valid: true,
        unlikely: false,
        hour: timed,
        dated: true,
        range: None,
        zoned: false,
    };
    with_zone_name(rest, time).len
}

/// The bytes of the zone chrono takes in after a date with no time that
/// ends at `end` of `text`: a name in capitals, then an offset ("15 December
/// +02:00"), which chrono's offset refiner reads after any result.
pub fn date_zone_len(code: &str, text: &str, end: usize) -> usize {
    let name = zone_name_len(text, end, false);
    let Some(rest) = text.get(end.saturating_add(name)..) else {
        return name;
    };
    let date = Time {
        len: 0,
        valid: true,
        unlikely: false,
        hour: false,
        dated: true,
        range: None,
        zoned: false,
    };
    // English and Italian read "+11mins" as a length, the others its "+11"
    // as a zone.
    if signed_length_at_start(code, rest) {
        return name;
    }
    name.saturating_add(with_offset(code, rest, date).len)
}

/// The places a time of day that may merge into, or overlap, the date
/// written at `start..end` of `text` in the language `code` can start: inside
/// the date when `inside` asks for them, before it when a time is glued to its
/// start ("9:00-17:00 19 de nov"), and after or before it where the
/// language's words join a date and a time, on its line. Only these places
/// are read, so the prose around a date costs nothing.
pub fn time_starts_around(
    code: &str,
    text: &str,
    start: usize,
    end: usize,
    inside: bool,
) -> Vec<usize> {
    let line_start = text
        .get(..start)
        .and_then(|t| t.rfind('\n'))
        .map_or(0, |at| at + 1);
    let line_end = text
        .get(end..)
        .and_then(|t| t.find('\n'))
        .map_or(text.len(), |at| at + end);
    let boundaries = |from: usize, to: usize| {
        (from.max(line_start)..to.min(line_end)).filter(|&at| text.is_char_boundary(at))
    };
    let mut starts: Vec<usize> = Vec::new();
    if inside {
        starts.extend(boundaries(start, end));
        let glued = crate::text::char_before(text, start)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | '-' | '：'));
        if glued {
            starts.extend(boundaries(start.saturating_sub(16), start));
        }
    }
    starts.extend(boundaries(end, end.saturating_add(24)).filter(|&at| {
        text.get(end..at)
            .is_some_and(|gap| joins_date_and_time(code, gap))
    }));
    let ends_a_time = |c: char| c.is_alphanumeric() || matches!(c, '.' | ')' | '）');
    for q in boundaries(start.saturating_sub(24), start.saturating_add(1)) {
        let joined = text
            .get(q..start)
            .is_some_and(|gap| gap.trim() != ":" && joins_date_and_time(code, gap));
        if joined && crate::text::char_before(text, q).is_some_and(ends_a_time) {
            starts.extend(boundaries(q.saturating_sub(24), q));
        }
    }
    starts
}

/// The times of day that may merge into the date at `start..end` of `text`,
/// read at [`time_starts_around`]'s places.
pub fn times_around(code: &str, text: &str, start: usize, end: usize) -> Vec<Clocked> {
    let mut starts = time_starts_around(code, text, start, end, false);
    starts.sort_unstable();
    starts.dedup();
    times_at(code, text, starts.into_iter())
}
