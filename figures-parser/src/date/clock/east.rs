//! The time expressions of Japanese, Chinese and Vietnamese, as chrono-node
//! 2.10.2 reads them (MIT): "15時30分", "午後3時", "下午3点半", "15:30",
//! "lúc 7 giờ 30 phút sáng", each with a range's second time.
//!
//! Each reads at the very start of a text; the caller decides where a
//! parser may start.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::{Clock24, Time};

/// The Japanese numerals chrono reads.
const JA_NUMERALS: &[(char, u32)] = &[
    ('零', 0),
    ('〇', 0),
    ('一', 1),
    ('二', 2),
    ('三', 3),
    ('四', 4),
    ('五', 5),
    ('六', 6),
    ('七', 7),
    ('八', 8),
    ('九', 9),
    ('十', 10),
];

/// The simplified Chinese numerals chrono reads.
pub const HANS_NUMERALS: &[(char, u32)] = &[
    ('零', 0),
    ('〇', 0),
    ('一', 1),
    ('二', 2),
    ('两', 2),
    ('三', 3),
    ('四', 4),
    ('五', 5),
    ('六', 6),
    ('七', 7),
    ('八', 8),
    ('九', 9),
    ('十', 10),
];

/// The traditional Chinese numerals chrono reads.
pub const HANT_NUMERALS: &[(char, u32)] = &[
    ('零', 0),
    ('一', 1),
    ('二', 2),
    ('兩', 2),
    ('三', 3),
    ('四', 4),
    ('五', 5),
    ('六', 6),
    ('七', 7),
    ('八', 8),
    ('九', 9),
    ('十', 10),
    ('廿', 20),
    ('卅', 30),
];

/// The numerals of a table as a character class body.
fn class(numerals: &[(char, u32)]) -> String {
    numerals.iter().map(|&(c, _)| c).collect()
}

/// A halfwidth or fullwidth digit's value.
fn wide_digit(c: char) -> Option<u32> {
    c.to_digit(10).or_else(|| {
        let offset = u32::from(c).checked_sub(u32::from('０'))?;
        (offset < 10).then_some(offset)
    })
}

/// A number as chrono's `parseInt`, then its numeral reader, read it: a
/// run of digits by value (None past `u32`, which no clock reads), else
/// numerals summed with `十` multiplying what precedes it.
pub fn number(text: &str, numerals: &[(char, u32)]) -> Option<u32> {
    if text.chars().next().and_then(wide_digit).is_some() {
        return text
            .chars()
            .map_while(wide_digit)
            .try_fold(0_u32, |n, d| n.checked_mul(10)?.checked_add(d));
    }
    text.chars().try_fold(0_u32, |n, c| {
        let value = numerals.iter().find(|&&(k, _)| k == c).map(|&(_, v)| v)?;
        if c == '十' {
            Some(if n == 0 { 10 } else { n.checked_mul(10)? })
        } else {
            n.checked_add(value)
        }
    })
}

/// A group's text.
fn group<'t>(caps: &Captures<'t>, name: &str) -> Option<&'t str> {
    caps.name(name).map(|m| m.as_str())
}

/// The end of the whole match.
fn end_of(caps: &Captures<'_>) -> usize {
    caps.get(0).map_or(0, |m| m.end())
}

/// Japanese's time, then a range's second time, as chrono's
/// `JPTimeExpressionParser` writes it.
struct Japanese {
    /// The first time, anchored.
    first: Option<Regex>,
    /// A range's separator and second time, anchored.
    second: Option<Regex>,
}

/// The body both Japanese times share after their separator.
fn japanese_body() -> String {
    let n = class(JA_NUMERALS);
    format!(
        r"(?:(?P<ap1>午前|午後|A.M.|P.M.|AM|PM))?(?:[\s,，、]*)(?:(?P<h>[0-9０-９]+|[{n}]+)(?:\s*)(?P<sep>時|:|：)(?:\s*)(?P<mi>[0-9０-９]+|半|[{n}]+)?(?:\s*)(?:分|:|：)?(?:\s*)(?P<s>[0-9０-９]+|[{n}]+)?(?:\s*)(?:秒)?)(?:\s*(?P<ap2>A.M.|P.M.|AM?|PM?))?"
    )
}

/// The patterns, compiled once.
static JAPANESE: LazyLock<Japanese> = LazyLock::new(|| {
    let body = japanese_body();
    Japanese {
        first: crate::text::compile(&format!("(?i)^{body}")),
        second: crate::text::compile(&format!(r"(?i)^\s*(?:から|\-|–|－|~|〜)\s*{body}")),
    }
});

/// The hour and minute a Japanese time sets, None where chrono rejects it.
fn japanese_clock(caps: &Captures<'_>) -> Option<Clock24> {
    let mut hour = number(group(caps, "h")?, JA_NUMERALS)?;
    if hour > 24 {
        return None;
    }
    let minute = match group(caps, "mi") {
        Some("半") => 30,
        Some(m) => number(m, JA_NUMERALS)?,
        None => 0,
    };
    if minute >= 60 {
        return None;
    }
    let second = group(caps, "s").map_or(Some(0), |s| number(s, JA_NUMERALS))?;
    if second >= 60 {
        return None;
    }
    if let Some(ap) = group(caps, "ap1").or_else(|| group(caps, "ap2")) {
        if hour > 12 {
            return None;
        }
        let first = ap.chars().next().map(|c| c.to_ascii_lowercase());
        if ap == "午前" || first == Some('a') {
            if hour == 12 {
                hour = 0;
            }
        } else if (ap == "午後" || first == Some('p')) && hour != 12 {
            hour += 12;
        }
    }
    let written = group(caps, "ap1").or_else(|| group(caps, "ap2")).is_some();
    Some(Clock24::new(hour, minute, second, written))
}

/// Japanese's time at the start of `text`.
pub fn japanese(text: &str) -> Option<Time> {
    let parsers = &*JAPANESE;
    let caps = parsers.first.as_ref()?.captures(text)?;
    // `時(?!間)`: "3時間" is a length, and the pattern has no other way to
    // match there.
    if group(&caps, "sep") == Some("時") {
        let after = caps.name("sep").map_or(0, |m| m.end());
        if text.get(after..).is_some_and(|rest| rest.starts_with('間')) {
            return None;
        }
    }
    let start = japanese_clock(&caps)?;
    let len = end_of(&caps);
    let rest = text.get(len..).unwrap_or_default();
    let Some(second) = parsers.second.as_ref().and_then(|re| re.captures(rest)) else {
        return Some(Time::clock(len, text, start, None));
    };
    let mut end = japanese_clock(&second)?;
    // "午後3時 - 5時" ends at 17, "午後10時 - 1時" at 1 at night.
    if !end.meridiem
        && start.meridiem
        && start.hour >= 12
        && end.hour < 12
        && start.hour - 12 <= end.hour
    {
        end.hour += 12;
    }
    Some(Time::clock(len + end_of(&second), text, start, Some(end)))
}

/// Chinese's time in one script: chrono runs a parser for each.
struct Chinese {
    /// The first time, anchored.
    first: Option<Regex>,
    /// A range's separator and second time, anchored.
    second: Option<Regex>,
    /// The numerals of the script.
    numerals: &'static [(char, u32)],
}

/// The pattern both Chinese times share after their separator: a day word
/// and a part of the day, the hour and its mark, minutes and seconds, and a
/// Latin meridiem.
fn chinese_body(numerals: &[(char, u32)], hant: bool) -> String {
    let n = class(numerals);
    let (days, parts, marks) = if hant {
        (
            "今|明|前|大前|後|大後|聽|昨|尋|琴",
            "上(?:午|晝)|朝(?:早)|早(?:上)|下(?:午|晝)|晏(?:晝)|晚(?:上)|夜(?:晚)?|中(?:午)|凌(?:晨)",
            "點|時|:|：",
        )
    } else {
        (
            "今|明|前|大前|后|大后|昨",
            "上(?:午)|早(?:上)|下(?:午)|晚(?:上)|夜(?:晚)?|中(?:午)|凌(?:晨)",
            "点|时|:|：",
        )
    };
    format!(
        r"(?:(?P<d1>{days})(?P<t1>早|朝|晚)|(?P<t2>{parts})|(?P<d3>{days})(?:日|天)(?:[\s,，]*)(?:(?P<t3>{parts}))?)?(?:[\s,，]*)(?:(?P<h>[0-9]+|[{n}]+)(?:\s*)(?:{marks})(?:\s*)(?P<mi>[0-9]+|半|正|整|[{n}]+)?(?:\s*)(?:分|:|：)?(?:\s*)(?P<s>[0-9]+|[{n}]+)?(?:\s*)(?:秒)?)(?:\s*(?P<ap>A.M.|P.M.|AM?|PM?))?"
    )
}

/// Both scripts' patterns, compiled once.
static CHINESE: LazyLock<[Chinese; 2]> = LazyLock::new(|| {
    let build = |numerals: &'static [(char, u32)], hant: bool, from: &str| {
        let body = chinese_body(numerals, hant);
        Chinese {
            first: crate::text::compile(&format!("(?i)^(?:{from})?{body}")),
            second: crate::text::compile(&format!(r"(?i)^\s*(?:到|至|\-|–|~|〜)\s*{body}")),
            numerals,
        }
    };
    [
        build(HANT_NUMERALS, true, "由|從|自"),
        build(HANS_NUMERALS, false, "从|自"),
    ]
});

/// The hour and minute a Chinese time sets, whether it names its day, and
/// whether its meridiem is written; None where chrono rejects it.
fn chinese_clock(caps: &Captures<'_>, numerals: &[(char, u32)]) -> Option<(Clock24, bool)> {
    let second = group(caps, "s").map_or(Some(0), |s| number(s, numerals))?;
    if second >= 60 {
        return None;
    }
    let mut hour = number(group(caps, "h")?, numerals)?;
    let mut minute = 0;
    match group(caps, "mi") {
        Some("半") => minute = 30,
        Some("正" | "整") => {}
        Some(m) => minute = number(m, numerals)?,
        None if hour > 100 => {
            minute = hour % 100;
            hour /= 100;
        }
        None => {}
    }
    if minute >= 60 || hour > 24 {
        return None;
    }
    let mut written = false;
    let part = group(caps, "t1")
        .map(|t| (t, true))
        .or_else(|| group(caps, "t2").map(|t| (t, false)))
        .or_else(|| group(caps, "t3").map(|t| (t, false)));
    if let Some(ap) = group(caps, "ap") {
        if hour > 12 {
            return None;
        }
        written = true;
        match ap.chars().next().map(|c| c.to_ascii_lowercase()) {
            Some('a') if hour == 12 => hour = 0,
            Some('p') if hour != 12 => hour += 12,
            _ => {}
        }
    } else if let Some((word, day_part)) = part {
        let lead = word.chars().next();
        let morning = if day_part {
            lead == Some('早')
        } else {
            matches!(lead, Some('上' | '早' | '凌'))
        };
        let evening = if day_part {
            lead == Some('晚')
        } else {
            matches!(lead, Some('下' | '晚'))
        };
        if morning {
            written = true;
            if hour == 12 {
                hour = 0;
            }
        } else if evening {
            written = true;
            if hour != 12 {
                hour += 12;
            }
        }
    }
    let dated = group(caps, "d1").is_some() || group(caps, "d3").is_some();
    Some((
        Clock24::new(hour, minute, second, written || hour >= 12),
        dated,
    ))
}

/// One script's Chinese time at the start of `text`, and whether it names
/// its day.
fn chinese_one(parser: &Chinese, text: &str) -> Option<(Time, bool)> {
    let caps = parser.first.as_ref()?.captures(text)?;
    let (start, dated) = chinese_clock(&caps, parser.numerals)?;
    let len = end_of(&caps);
    let rest = text.get(len..).unwrap_or_default();
    let Some(second) = parser.second.as_ref().and_then(|re| re.captures(rest)) else {
        return Some((Time::clock(len, text, start, None), dated));
    };
    let (end, _) = chinese_clock(&second, parser.numerals)?;
    let mut start = start;
    // A Latin meridiem on the end sets the start's when the start writes
    // none ("3点到5点PM").
    if !start.meridiem {
        match group(&second, "ap")
            .and_then(|ap| ap.chars().next())
            .map(|c| c.to_ascii_lowercase())
        {
            Some('a') if start.hour == 12 => start.hour = 0,
            Some('p') if start.hour != 12 => start.hour += 12,
            _ => {}
        }
    }
    Some((
        Time::clock(len + end_of(&second), text, start, Some(end)),
        dated,
    ))
}

/// Chinese's time at the start of `text`: the longer of the two scripts'
/// readings, and whether it names its day ("明天下午3点").
pub fn chinese(text: &str) -> Option<(Time, bool)> {
    CHINESE
        .iter()
        .filter_map(|parser| chinese_one(parser, text))
        .reduce(|kept, next| {
            let span = |t: Time| super::width(text.get(..t.len).unwrap_or(text));
            if span(next.0) > span(kept.0) {
                next
            } else {
                kept
            }
        })
}

/// Vietnamese's time expression, compiled once: chrono's
/// `VITimeExpressionParser`, its right lookahead a consumed `post` group.
static VIETNAMESE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    crate::text::compile(
        r"(?i)^(?:lúc\s*|vào\s*)?(?P<h>[0-9]{1,2})(?:\s*giờ\s*(?P<mg>[0-9]{1,2})?\s*(?:phút\s*)?(?P<ap>sáng|trưa|chiều|tối|đêm)?|:(?P<mc>[0-9]{2}))(?P<post>[^A-Za-z0-9_]|$)",
    )
});

/// Vietnamese's time at the start of `text`.
pub fn vietnamese(text: &str) -> Option<Time> {
    let caps = VIETNAMESE.as_ref()?.captures(text)?;
    let hour = group(&caps, "h").and_then(|h| number(h, &[]))?;
    if hour > 23 {
        return None;
    }
    let minute = group(&caps, "mc")
        .or_else(|| group(&caps, "mg"))
        .map_or(Some(0), |m| number(m, &[]))?;
    if minute >= 60 {
        return None;
    }
    let ap = group(&caps, "ap").map(str::to_lowercase);
    let hour = match ap.as_deref() {
        Some("sáng") if hour == 12 => 0,
        Some("trưa") if hour < 10 => hour + 12,
        Some("chiều" | "tối" | "đêm") if hour < 12 => hour + 12,
        _ => hour,
    };
    let len = caps
        .name("post")
        .map_or_else(|| end_of(&caps), |m| m.start());
    Some(Time::clock(
        len,
        text,
        Clock24::new(hour, minute, 0, ap.is_some()),
        None,
    ))
}
