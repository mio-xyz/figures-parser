//! The calendar rules chrono applies to a date written without a year, on
//! `jiff`'s civil dates.

use jiff::civil::{Date, Weekday};
use jiff::{ToSpan, Unit};

/// `day` of `month` in `year`, each allowed past its range the way a
/// JavaScript `Date` rolls it over: month 13 is the next January, the 31st
/// of a 30-day month the next 1st.
fn rolled(year: i64, month: i64, day: i64) -> Option<Date> {
    let index = month - 1;
    let rolled_year = i16::try_from(year + index.div_euclid(12)).ok()?;
    let rolled_month = i8::try_from(index.rem_euclid(12) + 1).ok()?;
    Date::new(rolled_year, rolled_month, 1)
        .ok()?
        .checked_add((day - 1).days())
        .ok()
}

/// Days from `from` to `to`.
fn days_between(from: Date, to: Date) -> Option<i64> {
    Some(i64::from(from.until((Unit::Day, to)).ok()?.get_days()))
}

/// The year chrono gives a date written without one: of that date in the
/// last, this and the next year, the one nearest `today`. The month and day
/// are set on `today` as JavaScript's `setMonth` and `setDate` set them,
/// overflow included, so a 31st read in a 30-day month lands as chrono's.
pub fn nearest_year(today: Date, month: u8, day: u8) -> Option<i16> {
    let set = rolled(
        i64::from(today.year()),
        i64::from(month),
        i64::from(today.day()),
    )?;
    let candidate = |offset: i64| {
        rolled(
            i64::from(set.year()) + offset,
            i64::from(set.month()),
            i64::from(day),
        )
    };
    let distance = |date: Date| days_between(today, date).map(i64::abs);
    let this_year = candidate(0)?;
    let closer = |other: Option<Date>| {
        other.filter(|&d| {
            distance(d)
                .zip(distance(this_year))
                .is_some_and(|(a, b)| a < b)
        })
    };
    let chosen = closer(candidate(1))
        .or_else(|| closer(candidate(-1)))
        .unwrap_or(this_year);
    Some(chosen.year())
}

/// The weekday counted from Sunday as 0, as chrono counts it.
pub fn from_sunday(date: Date) -> u8 {
    u8::try_from(date.weekday().to_sunday_zero_offset()).unwrap_or(0)
}

/// The weekday's English name.
pub const fn name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}
