//! A calendar date with no time zone, stored as days since 1970-01-01.
//!
//! Scheduling only needs whole days, so this avoids pulling in a date crate.
//! The conversions are Howard Hinnant's `days_from_civil` / `civil_from_days`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date(i32);

const WEEKDAYS: [&str; 7] = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

impl Date {
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Option<Date> {
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        Some(Date(days_from_civil(y, m, d)))
    }

    /// Local date for a Unix timestamp in milliseconds. `offset_min` is minutes
    /// *ahead* of UTC (so +60 for British Summer Time, +330 for India).
    pub fn from_unix_ms(ms: f64, offset_min: i32) -> Date {
        let local_ms = ms + offset_min as f64 * 60_000.0;
        Date((local_ms / 86_400_000.0).floor() as i32)
    }

    pub fn parse(s: &str) -> Option<Date> {
        let mut it = s.trim().splitn(3, '-');
        let y = it.next()?.parse().ok()?;
        let m = it.next()?.parse().ok()?;
        let d = it.next()?.parse().ok()?;
        Date::from_ymd(y, m, d)
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(self.0)
    }

    pub fn add_days(self, n: i32) -> Date {
        Date(self.0 + n)
    }

    pub fn days_since(self, earlier: Date) -> i32 {
        self.0 - earlier.0
    }

    pub fn weekday_name(self) -> &'static str {
        // 1970-01-01 was a Thursday.
        WEEKDAYS[(self.0 + 4).rem_euclid(7) as usize]
    }

    pub fn month_name(self) -> &'static str {
        MONTHS[self.ymd().1 as usize - 1]
    }

    /// Number of days in this date's month.
    pub fn month_len(self) -> u32 {
        let (y, m, _) = self.ymd();
        days_in_month(y, m)
    }

    /// "Wednesday 30 September"
    pub fn long_label(self) -> String {
        let (_, _, d) = self.ymd();
        format!("{} {} {}", self.weekday_name(), d, self.month_name())
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        let s = String::deserialize(d)?;
        Date::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad date: {s}")))
    }
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i32 - 719_468
}

fn civil_from_days(z: i32) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let d = Date::parse("2026-09-30").unwrap();
        assert_eq!(d.to_string(), "2026-09-30");
        assert_eq!(d.add_days(1).to_string(), "2026-10-01");
        assert_eq!(Date::parse("2028-02-28").unwrap().add_days(1).to_string(), "2028-02-29");
        assert_eq!(Date::from_ymd(1970, 1, 1), Some(Date(0)));
    }

    #[test]
    fn labels() {
        let d = Date::parse("2026-09-30").unwrap();
        assert_eq!(d.long_label(), "Wednesday 30 September");
        assert_eq!(d.month_len(), 30);
    }

    #[test]
    fn rejects_bad_dates() {
        assert!(Date::parse("2026-02-30").is_none());
        assert!(Date::parse("nonsense").is_none());
    }

    #[test]
    fn local_offset() {
        // 2026-09-29 23:30 UTC is already the 30th in India.
        let ms = Date::parse("2026-09-29").unwrap().0 as f64 * 86_400_000.0 + 23.5 * 3_600_000.0;
        assert_eq!(Date::from_unix_ms(ms, 0).to_string(), "2026-09-29");
        assert_eq!(Date::from_unix_ms(ms, 330).to_string(), "2026-09-30");
    }
}
