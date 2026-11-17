//! Calendar days, without time zones. A `Day` is a date on the wall
//! calendar; turning an instant into the local `Day` (and back) needs the
//! machine's time zone and lives in `crate::time`, at the browser boundary.

use std::fmt;

/// Which weekday a week starts on, as locales differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekStart {
    Sunday,
    Monday,
    Saturday,
}

impl WeekStart {
    /// Days from Sunday.
    fn offset(self) -> i64 {
        match self {
            WeekStart::Sunday => 0,
            WeekStart::Monday => 1,
            WeekStart::Saturday => 6,
        }
    }
}

/// One day on the proleptic Gregorian calendar. Ordered by date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
}

impl Day {
    /// The day for these fields, or `None` when no such date exists.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Day> {
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        valid.then_some(Day { year, month, day })
    }

    /// Reads `YYYY-MM-DD`, the form the shell and `key()` use.
    pub fn parse(key: &str) -> Option<Day> {
        let mut parts = key.split('-');
        let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
            return None;
        }
        Day::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
    }

    /// `YYYY-MM-DD`.
    pub fn key(self) -> String {
        self.to_string()
    }

    /// Days since 1970-01-01 (negative before it).
    fn number(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year.rem_euclid(400);
        let month = i64::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    fn from_number(number: i64) -> Day {
        // Howard Hinnant's civil_from_days.
        let shifted = number + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Day {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    }

    pub fn plus_days(self, days: i64) -> Day {
        Day::from_number(self.number() + days)
    }

    /// The same day of the month `months` away, or that month's last day
    /// when it is shorter (31 January + 1 month = 28 or 29 February).
    pub fn plus_months(self, months: i32) -> Day {
        let index = self.year * 12 + i32::from(self.month) - 1 + months;
        let year = index.div_euclid(12);
        let month = (index.rem_euclid(12) + 1) as u8;
        Day {
            year,
            month,
            day: self.day.min(days_in_month(year, month)),
        }
    }

    /// Whole days from `self` to `later` (negative when `later` is earlier).
    pub fn days_until(self, later: Day) -> i64 {
        later.number() - self.number()
    }

    /// Days from Sunday: 0 Sunday to 6 Saturday.
    fn weekday(self) -> i64 {
        // 1970-01-01 was a Thursday.
        (self.number() + 4).rem_euclid(7)
    }

    pub fn same_month(self, other: Day) -> bool {
        self.year == other.year && self.month == other.month
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

/// One square of a month calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthCell {
    pub day: Day,
    /// False for the days of the months before and after that fill the grid.
    pub in_month: bool,
}

/// Six weeks covering the month `any_day` is in, starting on `week_start`.
pub fn month_grid(any_day: Day, week_start: WeekStart) -> Vec<MonthCell> {
    let first = Day { day: 1, ..any_day };
    let lead = (first.weekday() - week_start.offset()).rem_euclid(7);
    (0..42)
        .map(|i| {
            let day = first.plus_days(i - lead);
            MonthCell {
                day,
                in_month: day.same_month(first),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(key: &str) -> Day {
        Day::parse(key).unwrap()
    }

    #[test]
    fn reads_and_writes_the_shells_day_format() {
        assert_eq!(day("2026-03-09").key(), "2026-03-09");
        assert_eq!(
            day("2026-03-09"),
            Day {
                year: 2026,
                month: 3,
                day: 9
            }
        );
        for bad in [
            "",
            "2026-3-9",
            "2026-02-30",
            "2026-13-01",
            "2026-00-10",
            "x026-01-01",
            "2026-01-01-01",
        ] {
            assert_eq!(Day::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn steps_days_across_months_years_and_leap_days() {
        assert_eq!(day("2026-03-01").plus_days(-1), day("2026-02-28"));
        assert_eq!(day("2024-03-01").plus_days(-1), day("2024-02-29"));
        assert_eq!(day("2025-12-31").plus_days(1), day("2026-01-01"));
        assert_eq!(day("2026-11-16").plus_days(-7), day("2026-11-09"));
        assert_eq!(day("1969-12-31").plus_days(1), day("1970-01-01"));
    }

    #[test]
    fn steps_months_keeping_the_day_or_the_months_last() {
        assert_eq!(day("2026-01-31").plus_months(1), day("2026-02-28"));
        assert_eq!(day("2024-01-31").plus_months(1), day("2024-02-29"));
        assert_eq!(day("2026-01-15").plus_months(-1), day("2025-12-15"));
        assert_eq!(day("2026-12-10").plus_months(1), day("2027-01-10"));
        assert_eq!(day("2026-03-31").plus_months(-13), day("2025-02-28"));
    }

    #[test]
    fn counts_whole_days_between_days() {
        assert_eq!(day("2026-11-14").days_until(day("2026-11-16")), 2);
        assert_eq!(day("2026-11-16").days_until(day("2026-11-14")), -2);
        assert_eq!(day("2025-12-31").days_until(day("2026-01-01")), 1);
        assert_eq!(day("2026-11-16").days_until(day("2026-11-16")), 0);
    }

    #[test]
    fn days_order_by_date() {
        assert!(day("2025-12-31") < day("2026-01-01"));
        assert!(day("2026-02-01") > day("2026-01-31"));
    }

    #[test]
    fn a_month_grid_is_six_weeks_from_the_locales_first_weekday() {
        // November 2026 starts on a Sunday.
        let monday = month_grid(day("2026-11-16"), WeekStart::Monday);
        assert_eq!(monday.len(), 42);
        assert_eq!(monday[0].day, day("2026-10-26"));
        assert!(!monday[0].in_month);
        assert_eq!(monday[6].day, day("2026-11-01"));
        assert!(monday[6].in_month);
        assert_eq!(monday[41].day, day("2026-12-06"));

        let sunday = month_grid(day("2026-11-16"), WeekStart::Sunday);
        assert_eq!(sunday[0].day, day("2026-11-01"));
        let saturday = month_grid(day("2026-11-16"), WeekStart::Saturday);
        assert_eq!(saturday[0].day, day("2026-10-31"));
    }
}
