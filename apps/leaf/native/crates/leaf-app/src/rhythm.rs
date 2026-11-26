//! When a series' chapters come out (the owner, 2026-11-25), read from the dates of its newest
//! chapters (`uploads`, sent beside the series: no request of its own): its usual day ("New
//! chapter usually Wednesday"), whether one is due today, and a break, six weeks or more without
//! a chapter ([`leaf_proto::BREAK_MS`]), which moves it down in Updates.
//!
//! A source that dates every chapter the day it was copied (many chapters on one day) gives no
//! rhythm: `uploads` keeps one a day, and fewer than [`MIN_UPLOADS`] say nothing.

const DAY: i64 = 86_400_000;
/// Fewer dates than this tell no rhythm.
pub const MIN_UPLOADS: usize = 4;
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// How often a series has had chapters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usual {
    /// Days between chapters, the middle of the last ones.
    pub every: u32,
    /// The weekday most came on (0 is Monday), when nearly all came on it (weekly series only).
    pub weekday: Option<u8>,
    /// The newest chapter's date (ms).
    pub last_at: i64,
}

/// The local day number (days from 1970-01-01) of a moment, `offset_s` from UTC.
pub fn local_day(ms: i64, offset_s: i64) -> i64 {
    (ms.div_euclid(1000) + offset_s).div_euclid(86_400)
}

/// The weekday of a local day number, 0 is Monday (1970-01-01 was a Thursday).
pub fn weekday(day: i64) -> u8 {
    (day + 3).rem_euclid(7) as u8
}

/// The rhythm of `uploads` (newest first, ms), None when they tell none.
pub fn usual(uploads: &[i64], offset_s: i64) -> Option<Usual> {
    if uploads.len() < MIN_UPLOADS {
        return None;
    }
    let mut gaps: Vec<i64> = uploads.windows(2).map(|w| (w[0] - w[1]).max(0)).collect();
    gaps.sort_unstable();
    let every = ((gaps[gaps.len() / 2] as f64 / DAY as f64).round() as u32).max(1);
    let weekday = (5..=9).contains(&every).then(|| {
        let mut count = [0usize; 7];
        for u in uploads {
            count[weekday(local_day(*u, offset_s)) as usize] += 1;
        }
        let (best, n) = count.iter().enumerate().max_by_key(|(i, n)| (**n, 7 - *i)).map(|(i, n)| (i as u8, *n)).unwrap_or((0, 0));
        (n * 4 >= uploads.len() * 3).then_some(best)
    });
    Some(Usual { every, weekday: weekday.flatten(), last_at: uploads[0] })
}

/// No chapter for six weeks or more: on a break (its status isn't finished).
pub fn on_break(uploads: &[i64], status: &str, now_ms: i64) -> bool {
    status != "completed" && uploads.first().is_some_and(|last| now_ms - last >= leaf_proto::BREAK_MS)
}

/// A chapter is due today: a weekly series on its day, or one whose usual gap ends today, and
/// none came today yet. Never while on a break.
pub fn due_today(u: &Usual, now_ms: i64, offset_s: i64) -> bool {
    let today = local_day(now_ms, offset_s);
    let last = local_day(u.last_at, offset_s);
    if last >= today || now_ms - u.last_at >= leaf_proto::BREAK_MS {
        return false;
    }
    match u.weekday {
        Some(w) => weekday(today) == w && today - last >= 5,
        None => last + u.every as i64 == today,
    }
}

/// The words under a series' title: "New chapter usually Wednesday", "About every 2 weeks",
/// "On a break · last chapter 9 weeks ago".
pub fn words(uploads: &[i64], status: &str, now_ms: i64, offset_s: i64) -> Option<String> {
    if status == "completed" {
        return None;
    }
    if on_break(uploads, status, now_ms) {
        let weeks = (now_ms - uploads[0]) / (7 * DAY);
        return Some(format!("On a break · last chapter {weeks} weeks ago"));
    }
    let u = usual(uploads, offset_s)?;
    if due_today(&u, now_ms, offset_s) {
        return Some("New chapter due today".into());
    }
    Some(usual_words(&u))
}

/// "New chapter usually Wednesday", "New chapter about every 2 weeks".
pub fn usual_words(u: &Usual) -> String {
    match (u.weekday, u.every) {
        (Some(w), _) => format!("New chapter usually {}", WEEKDAYS[w as usize]),
        (None, 1) => "New chapters most days".into(),
        (None, d @ 2..=10) => format!("New chapter about every {d} days"),
        (None, d) if d < 45 => format!("New chapter about every {} weeks", ((d as f64) / 7.0).round() as u32),
        (None, d) => format!("New chapter about every {} months", ((d as f64) / 30.0).round().max(1.0) as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noon UTC on local day `d`.
    fn at(d: i64) -> i64 {
        d * DAY + DAY / 2
    }

    #[test]
    fn a_weekly_series_has_its_day_and_is_due_on_it() {
        assert_eq!(weekday(0), 3, "1970-01-01 was a Thursday");
        let wed = (0..7).map(|k| 20_000 - k).find(|d| weekday(*d) == 2).unwrap();
        let ups: Vec<i64> = (0..8).map(|k| at(wed - 7 * k)).collect();
        let u = usual(&ups, 0).unwrap();
        assert_eq!((u.every, u.weekday), (7, Some(2)));
        assert_eq!(words(&ups, "ongoing", at(wed + 1), 0).as_deref(), Some("New chapter usually Wednesday"));
        assert!(due_today(&u, at(wed + 7), 0));
        assert!(!due_today(&u, at(wed + 6), 0));
        assert_eq!(words(&ups, "ongoing", at(wed + 7), 0).as_deref(), Some("New chapter due today"));
    }

    #[test]
    fn days_move_with_the_zone_and_loose_weeks_have_no_day() {
        // 23:00 UTC is the next day eight hours east.
        let late = 20_000 * DAY + 23 * 3_600_000;
        assert_eq!(local_day(late, 8 * 3600), 20_001);
        // Every 7 days but on mixed weekdays: a rhythm, no day.
        let ups = vec![at(100), at(93), at(85), at(78), at(72)];
        let u = usual(&ups, 0).unwrap();
        assert_eq!(u.weekday, None);
        assert_eq!(words(&ups, "ongoing", at(101), 0).as_deref(), Some("New chapter about every 7 days"));
        let ups = vec![at(100), at(86), at(72), at(58)];
        assert_eq!(words(&ups, "ongoing", at(101), 0).as_deref(), Some("New chapter about every 2 weeks"));
    }

    #[test]
    fn six_weeks_without_a_chapter_is_a_break_and_finished_says_nothing() {
        let ups = vec![at(100), at(93), at(86), at(79)];
        assert!(on_break(&ups, "ongoing", at(100 + 42)));
        assert!(!on_break(&ups, "ongoing", at(100 + 41)));
        assert_eq!(words(&ups, "ongoing", at(100 + 63), 0).as_deref(), Some("On a break · last chapter 9 weeks ago"));
        assert_eq!(words(&ups, "completed", at(101), 0), None);
        assert_eq!(words(&ups[..3], "ongoing", at(101), 0), None, "three dates tell no rhythm");
    }
}
