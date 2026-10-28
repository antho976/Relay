//! When a schedule next comes round, in the PC's own time zone: "every weekday at 7" means 7 on
//! the wall clock, through daylight saving changes.

use crate::model::{Every, Schedule};
use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

/// How long after its time a schedule missed while the PC was off still runs: yesterday's brief is
/// not sent today.
pub const GRACE_SECONDS: i64 = 3600;

pub fn check(s: &Schedule) -> Result<(), String> {
    if s.hour > 23 || s.minute > 59 {
        return Err("A schedule's time is 0:00 to 23:59".into());
    }
    if s.weekday.is_some_and(|d| !(1..=7).contains(&d)) {
        return Err("A schedule's weekday is 1 (Monday) to 7 (Sunday)".into());
    }
    if s.day.is_some_and(|d| !(1..=28).contains(&d)) {
        return Err("A schedule's day of the month is 1 to 28".into());
    }
    Ok(())
}

fn falls_on(s: &Schedule, date: Date) -> bool {
    let weekday = date.weekday().to_monday_one_offset();
    match s.every {
        Every::Day => true,
        Every::Weekdays => weekday <= 5,
        Every::Week => i8::try_from(s.weekday.unwrap_or(1)).is_ok_and(|w| w == weekday),
        Every::Month => i8::try_from(s.day.unwrap_or(1)).is_ok_and(|d| d == date.day()),
    }
}

/// The first time the schedule comes round strictly after `after`, in `tz`.
pub fn next_after_in(s: &Schedule, after: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
    let time = Time::new(i8::try_from(s.hour).ok()?, i8::try_from(s.minute).ok()?, 0, 0).ok()?;
    let start = after.to_zoned(tz.clone()).date();
    // A month's day comes round within 31 days; two months covers any start.
    for d in 0..=62 {
        let date = start.checked_add(jiff::Span::new().days(d)).ok()?;
        if !falls_on(s, date) {
            continue;
        }
        // A time a spring change skips is taken as the hour after (jiff's compatible rule).
        let at: Zoned = date.to_datetime(time).to_zoned(tz.clone()).ok()?;
        if at.timestamp() > after {
            return Some(at.timestamp());
        }
    }
    None
}

/// [`next_after_in`] in the PC's time zone.
pub fn next_after(s: &Schedule, after: Timestamp) -> Option<Timestamp> {
    next_after_in(s, after, &TimeZone::system())
}

/// The last time the schedule came round at or before `upto` and after `since`, if it did: what a
/// PC that was off missed.
pub fn missed_in(s: &Schedule, since: Timestamp, upto: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
    let mut last = None;
    let mut t = since;
    while let Some(next) = next_after_in(s, t, tz) {
        if next > upto {
            break;
        }
        last = Some(next);
        t = next;
    }
    last
}

/// The hour (0 to 23) on the PC's clock at `t`.
pub fn local_hour(t: Timestamp) -> u8 {
    u8::try_from(t.to_zoned(TimeZone::system()).hour()).unwrap_or(0)
}

/// The start of the PC's day holding `t`, for counting a day's runs and interrupts.
pub fn day_start(t: Timestamp) -> Timestamp {
    let z = t.to_zoned(TimeZone::system());
    z.start_of_day().map(|s| s.timestamp()).unwrap_or(t)
}

/// One line for a person: "weekdays at 07:00".
pub fn describe(s: &Schedule) -> String {
    const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    let at = format!("{:02}:{:02}", s.hour, s.minute);
    match s.every {
        Every::Day => format!("every day at {at}"),
        Every::Weekdays => format!("weekdays at {at}"),
        Every::Week => format!("every {} at {at}", DAYS[usize::from(s.weekday.unwrap_or(1).clamp(1, 7) - 1)]),
        Every::Month => format!("on the {} of every month at {at}", ordinal(s.day.unwrap_or(1))),
    }
}

fn ordinal(n: u8) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn tz() -> TimeZone {
        TimeZone::get("America/Toronto").unwrap()
    }

    fn sched(every: Every, hour: u8) -> Schedule {
        Schedule { every, hour, minute: 0, weekday: None, day: None }
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        // Friday 2026-10-30 08:00 in Toronto (12:00 UTC): the next weekday 07:00 is Monday.
        let next = next_after_in(&sched(Every::Weekdays, 7), at("2026-10-30T12:00:00Z"), &tz()).unwrap();
        assert_eq!(next.to_zoned(tz()).date().to_string(), "2026-11-02");
        assert_eq!(next.to_zoned(tz()).hour(), 7);
    }

    #[test]
    fn the_wall_clock_holds_through_daylight_saving() {
        // Toronto leaves daylight saving on 2026-11-01: 07:00 is 11:00 UTC before, 12:00 after.
        let s = sched(Every::Day, 7);
        let before = next_after_in(&s, at("2026-10-31T00:00:00Z"), &tz()).unwrap();
        let after = next_after_in(&s, before, &tz()).unwrap();
        assert_eq!(before, at("2026-10-31T11:00:00Z"));
        assert_eq!(after, at("2026-11-01T12:00:00Z"));
    }

    #[test]
    fn strictly_after() {
        let s = sched(Every::Day, 7);
        let seven = at("2026-10-28T11:00:00Z");
        assert_eq!(next_after_in(&s, seven, &tz()).unwrap(), at("2026-10-29T11:00:00Z"));
    }

    #[test]
    fn week_and_month() {
        let sunday = Schedule { every: Every::Week, hour: 18, minute: 30, weekday: Some(7), day: None };
        let next = next_after_in(&sunday, at("2026-10-28T12:00:00Z"), &tz()).unwrap().to_zoned(tz());
        assert_eq!((next.date().to_string(), next.hour(), next.minute()), ("2026-11-01".into(), 18, 30));
        let first = Schedule { every: Every::Month, hour: 9, minute: 0, weekday: None, day: None };
        let next = next_after_in(&first, at("2026-10-28T12:00:00Z"), &tz()).unwrap().to_zoned(tz());
        assert_eq!(next.date().to_string(), "2026-11-01");
    }

    #[test]
    fn what_a_pc_that_was_off_missed() {
        let s = sched(Every::Day, 7);
        // Off from Monday 06:00 to Wednesday 09:00 Toronto: the last 07:00 missed is Wednesday's.
        let missed = missed_in(&s, at("2026-10-26T10:00:00Z"), at("2026-10-28T13:00:00Z"), &tz()).unwrap();
        assert_eq!(missed, at("2026-10-28T11:00:00Z"));
        assert!(missed_in(&s, at("2026-10-28T11:30:00Z"), at("2026-10-28T13:00:00Z"), &tz()).is_none());
    }

    #[test]
    fn checks_and_describes() {
        assert!(check(&Schedule { every: Every::Day, hour: 24, minute: 0, weekday: None, day: None }).is_err());
        assert!(check(&Schedule { every: Every::Month, hour: 1, minute: 0, weekday: None, day: Some(31) }).is_err());
        assert_eq!(describe(&sched(Every::Weekdays, 7)), "weekdays at 07:00");
        assert_eq!(describe(&Schedule { every: Every::Month, hour: 9, minute: 5, weekday: None, day: Some(22) }), "on the 22nd of every month at 09:05");
    }
}
