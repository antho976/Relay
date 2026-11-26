//! The numbers behind Wrapped (design 45; the owner, 2026-11-25: "new series started, longest
//! streak, most read in a day"), worked out on the phone from what is kept already: each chapter
//! read and when (`/v1/history`, the server's own records) and the books' reading time
//! (`shelf.db`). They are shown only in Wrapped (no stats tab).
//!
//! - Chapters marked read in bulk (progress brought from elsewhere, "I've read up to…") are not
//!   reading: three or more of a series within [`BULK_MS`] of each other are left out ([`clean`]).
//! - A comic's reading time is not clocked: it is told from the gaps between chapters finished
//!   one after another (at most [`SESSION_GAP_MS`] apart), and a chapter that starts a sitting
//!   counts [`FIRST_CHAPTER_S`]. The cards say "about".

use std::collections::{HashMap, HashSet};

use leaf_proto::HistoryRead;

/// Chapters of a series read this close together, three or more in a row, were marked, not read.
pub const BULK_MS: i64 = 3_000;
/// Two chapters finished this close are one sitting; the gap between them is reading.
pub const SESSION_GAP_MS: i64 = 30 * 60_000;
/// The first chapter of a sitting counts this long.
pub const FIRST_CHAPTER_S: i64 = 5 * 60;
/// A day counts toward a streak with a chapter, or a book read this long.
pub const BOOK_DAY_S: i64 = 60;
const DAY_S: i64 = 86_400;

/// The days of a month or a year (local day numbers, `to` not included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Period {
    pub year: i32,
    /// 1 to 12; None is the whole year.
    pub month: Option<u32>,
    pub from: i64,
    pub to: i64,
}

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Days from 1970-01-01 of a date (proleptic Gregorian).
pub fn day_of_date(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y as i64 - 1 } else { y as i64 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number: (year, month 1–12, day 1–31).
pub fn date_of_day(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

impl Period {
    pub fn month(year: i32, month: u32) -> Period {
        let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        Period { year, month: Some(month), from: day_of_date(year, month, 1), to: day_of_date(ny, nm, 1) }
    }

    pub fn year(year: i32) -> Period {
        Period { year, month: None, from: day_of_date(year, 1, 1), to: day_of_date(year + 1, 1, 1) }
    }

    /// The month or year before this one.
    pub fn before(&self) -> Period {
        match self.month {
            Some(1) => Period::month(self.year - 1, 12),
            Some(m) => Period::month(self.year, m - 1),
            None => Period::year(self.year - 1),
        }
    }

    pub fn days(&self) -> usize {
        (self.to - self.from).max(0) as usize
    }

    /// "November", "2026".
    pub fn name(&self) -> String {
        match self.month {
            Some(m) => MONTHS[(m - 1) as usize].to_string(),
            None => self.year.to_string(),
        }
    }

    /// "month:2026-11", "year:2026": the route's words.
    pub fn key(&self) -> String {
        match self.month {
            Some(m) => format!("month:{}-{m:02}", self.year),
            None => format!("year:{}", self.year),
        }
    }

    pub fn parse(key: &str) -> Option<Period> {
        if let Some(v) = key.strip_prefix("month:") {
            let (y, m) = v.split_once('-')?;
            let (y, m) = (y.parse().ok()?, m.parse().ok()?);
            return (1..=12).contains(&m).then(|| Period::month(y, m));
        }
        key.strip_prefix("year:").and_then(|y| y.parse().ok()).map(Period::year)
    }
}

/// Which Wrapped Home offers on `today` (a local day): the year's in its first two weeks, the
/// month's in its first week; the month first when both.
pub fn offered(today: i64) -> Vec<Period> {
    let (y, m, d) = date_of_day(today);
    let mut v = Vec::new();
    if d <= 7 {
        v.push(Period::month(y, m).before());
    }
    if m == 1 && d <= 14 {
        v.push(Period::year(y - 1));
    }
    v
}

/// Reads marked in bulk left out (see the module's notes); what is left, oldest first.
pub fn clean(reads: &[HistoryRead]) -> Vec<HistoryRead> {
    let mut by: HashMap<i64, Vec<HistoryRead>> = HashMap::new();
    for r in reads {
        by.entry(r.series).or_default().push(*r);
    }
    let mut out = Vec::new();
    for (_, mut v) in by {
        v.sort_by(|a, b| a.at.cmp(&b.at).then(a.number.total_cmp(&b.number)));
        let mut i = 0;
        while i < v.len() {
            let mut j = i + 1;
            while j < v.len() && v[j].at - v[j - 1].at <= BULK_MS {
                j += 1;
            }
            if j - i < 3 {
                out.extend_from_slice(&v[i..j]);
            }
            i = j;
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then(a.series.cmp(&b.series)));
    out
}

/// Seconds of reading told from chapters finished one after another (oldest first): each gap
/// within a sitting, and [`FIRST_CHAPTER_S`] for the first of each; per series too (a gap goes to
/// the chapter that ends it).
pub fn sittings(reads: &[HistoryRead]) -> (i64, HashMap<i64, i64>) {
    let mut total = 0;
    let mut per: HashMap<i64, i64> = HashMap::new();
    let mut last: Option<i64> = None;
    for r in reads {
        let s = match last {
            Some(t) if r.at - t <= SESSION_GAP_MS => (r.at - t) / 1000,
            _ => FIRST_CHAPTER_S,
        };
        total += s;
        *per.entry(r.series).or_default() += s;
        last = Some(r.at);
    }
    (total, per)
}

/// The longest run of days in `days` (sorted or not): (length, first day).
pub fn longest_run(days: &HashSet<i64>) -> (usize, i64) {
    let mut v: Vec<i64> = days.iter().copied().collect();
    v.sort_unstable();
    let (mut best, mut best_at, mut run, mut at) = (0usize, 0i64, 0usize, 0i64);
    for (k, d) in v.iter().enumerate() {
        if k > 0 && v[k - 1] == d - 1 {
            run += 1;
        } else {
            run = 1;
            at = *d;
        }
        if run > best {
            best = run;
            best_at = at;
        }
    }
    (best, best_at)
}

/// A series as Wrapped needs it beside its reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub id: i64,
    pub title: String,
    /// Every chapter there is, read.
    pub caught_up: bool,
}

/// A book as Wrapped needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct BookDone {
    pub title: String,
    /// When it was finished (last opened, ms), if it is.
    pub finished_ms: Option<i64>,
    pub pages: u32,
}

/// The best day: (day, chapters, first and last read (ms), the three series read most that day).
pub type BestDay = (i64, usize, i64, i64, Vec<(i64, usize)>);

/// Everything the cards show, for one period.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub chapters: usize,
    /// Chapters each day of the period.
    pub per_day: Vec<usize>,
    /// Chapters each month (a year only).
    pub per_month: [usize; 12],
    pub before: usize,
    /// (days, first day) of the longest run with reading, in the period and ever.
    pub streak: (usize, i64),
    pub best_streak: usize,
    pub days_read: usize,
    /// (day, chapters, first and last read (ms), the three series read most that day).
    pub best_day: Option<BestDay>,
    /// Series by chapters read: (series, chapters, seconds, started in the period from chapter 1).
    pub top: Vec<(i64, usize, i64, bool)>,
    /// Series first read in the period, in the order they were begun.
    pub new_series: Vec<i64>,
    /// Of those, the one read most in its first two days: (series, chapters).
    pub took: Option<(i64, usize)>,
    pub comic_secs: i64,
    pub book_secs: i64,
    /// Chapters by the local hour they were finished.
    pub hours: [usize; 24],
    pub caught_up: Vec<i64>,
    pub books_done: Vec<(String, u32)>,
    /// The first chapter of the period: (series, ms).
    pub first: Option<(i64, i64)>,
    /// The most chapters of one series in two days running: (series, chapters).
    pub binge: Option<(i64, usize)>,
}

/// Work out a period's numbers. `reads` are every read kept ([`clean`]ed), `book_days` (local
/// day, seconds) the books' reading time, `offset_s` the zone's offset from UTC.
pub fn compute(p: &Period, reads: &[HistoryRead], book_days: &[(i64, i64)], known: &[Known], books: &[BookDone], offset_s: i64) -> Stats {
    let day = |ms: i64| (ms.div_euclid(1000) + offset_s).div_euclid(DAY_S);
    let inside = |d: i64| d >= p.from && d < p.to;
    let here: Vec<HistoryRead> = reads.iter().copied().filter(|r| inside(day(r.at))).collect();
    let mut s = Stats { chapters: here.len(), per_day: vec![0; p.days()], ..Default::default() };
    let before = p.before();
    s.before = reads.iter().filter(|r| day(r.at) >= before.from && day(r.at) < before.to).count();
    for r in &here {
        let d = day(r.at);
        s.per_day[(d - p.from) as usize] += 1;
        let (_, m, _) = date_of_day(d);
        s.per_month[(m - 1) as usize] += 1;
        let local_s = (r.at.div_euclid(1000) + offset_s).rem_euclid(DAY_S);
        s.hours[(local_s / 3600) as usize] += 1;
    }
    // Days with reading: a chapter, or a book a minute or more.
    let mut all_days: HashSet<i64> = reads.iter().map(|r| day(r.at)).collect();
    all_days.extend(book_days.iter().filter(|(_, secs)| *secs >= BOOK_DAY_S).map(|(d, _)| *d));
    let period_days: HashSet<i64> = all_days.iter().copied().filter(|d| inside(*d)).collect();
    s.days_read = period_days.len();
    s.streak = longest_run(&period_days);
    s.best_streak = longest_run(&all_days).0;
    // The best day, and what was read most on it.
    if let Some((i, n)) = s.per_day.iter().enumerate().filter(|(_, n)| **n > 0).max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0))) {
        let d = p.from + i as i64;
        let that: Vec<&HistoryRead> = here.iter().filter(|r| day(r.at) == d).collect();
        let mut per: HashMap<i64, usize> = HashMap::new();
        for r in &that {
            *per.entry(r.series).or_default() += 1;
        }
        let mut top: Vec<(i64, usize)> = per.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top.truncate(3);
        s.best_day = Some((d, *n, that.first().map_or(0, |r| r.at), that.last().map_or(0, |r| r.at), top));
    }
    // Time: comics from their sittings, books as clocked.
    let (secs, per_secs) = sittings(&here);
    s.comic_secs = secs;
    s.book_secs = book_days.iter().filter(|(d, _)| inside(*d)).map(|(_, secs)| secs).sum();
    // Series: first read ever, and how much in the period.
    let mut first_ever: HashMap<i64, (i64, f64)> = HashMap::new();
    for r in reads {
        let e = first_ever.entry(r.series).or_insert((r.at, r.number));
        if r.at < e.0 {
            *e = (r.at, r.number);
        }
    }
    let mut count: HashMap<i64, usize> = HashMap::new();
    for r in &here {
        *count.entry(r.series).or_default() += 1;
    }
    let mut news: Vec<(i64, i64)> = first_ever.iter().filter(|(_, (at, _))| inside(day(*at))).map(|(id, (at, _))| (*id, *at)).collect();
    news.sort_by_key(|(id, at)| (*at, *id));
    s.new_series = news.iter().map(|(id, _)| *id).collect();
    s.took = news
        .iter()
        .map(|(id, at)| (*id, here.iter().filter(|r| r.series == *id && r.at >= *at && r.at < at + 2 * DAY_S * 1000).count()))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .filter(|(_, n)| *n > 1);
    let mut top: Vec<(i64, usize, i64, bool)> = count
        .iter()
        .map(|(id, n)| {
            let from_one = first_ever.get(id).is_some_and(|(at, number)| inside(day(*at)) && *number <= 1.0);
            (*id, *n, per_secs.get(id).copied().unwrap_or(0), from_one)
        })
        .collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
    s.top = top;
    // Caught up: every chapter read, the last of them in the period.
    let last_read: HashMap<i64, i64> = here.iter().map(|r| (r.series, r.at)).collect();
    s.caught_up = known.iter().filter(|k| k.caught_up && last_read.contains_key(&k.id)).map(|k| k.id).collect();
    s.books_done = books.iter().filter(|b| b.finished_ms.is_some_and(|ms| inside(day(ms)))).map(|b| (b.title.clone(), b.pages)).collect();
    s.first = here.first().map(|r| (r.series, r.at));
    // The biggest binge: one series, two days running.
    let mut per_day_series: HashMap<(i64, i64), usize> = HashMap::new();
    for r in &here {
        *per_day_series.entry((r.series, day(r.at))).or_default() += 1;
    }
    s.binge = per_day_series
        .iter()
        .map(|((id, d), n)| (*id, n + per_day_series.get(&(*id, d + 1)).copied().unwrap_or(0)))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
    s
}

/// "31 h 20 m", "45 m", "0 m".
pub fn hours_words(secs: i64) -> String {
    let m = (secs.max(0) + 30) / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{m} m"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} m"),
    }
}

/// When most reading happened, in words: "Most of it after 10 at night".
pub fn hour_words(hours: &[usize; 24]) -> Option<String> {
    let total: usize = hours.iter().sum();
    if total < 10 {
        return None;
    }
    let share = |range: &[usize]| range.iter().map(|h| hours[*h]).sum::<usize>() as f32 / total as f32;
    Some(if share(&[22, 23, 0, 1, 2, 3]) >= 0.4 {
        "Most of it after 10 at night.".into()
    } else if share(&[5, 6, 7, 8, 9, 10]) >= 0.4 {
        "Most of it in the morning.".into()
    } else if share(&[12, 13, 14, 15, 16, 17]) >= 0.4 {
        "Most of it in the afternoon.".into()
    } else if share(&[18, 19, 20, 21]) >= 0.4 {
        "Most of it in the evening.".into()
    } else {
        "Spread across the day.".into()
    })
}

/// "Up 43% on October", "Down 12% on October", "As much as October": None when the month before
/// had nothing.
pub fn versus_words(now: usize, before: usize, name: &str) -> Option<String> {
    if before == 0 {
        return None;
    }
    let pct = ((now as f64 / before as f64 - 1.0) * 100.0).round() as i64;
    Some(match pct {
        0 => format!("As much as {name}"),
        p if p > 0 => format!("Up {p}% on {name}"),
        p => format!("Down {}% on {name}", -p),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600_000;

    fn r(series: i64, number: f64, at: i64) -> HistoryRead {
        HistoryRead { series, number, at }
    }

    #[test]
    fn dates_go_both_ways_and_periods_know_their_days() {
        assert_eq!(day_of_date(1970, 1, 1), 0);
        assert_eq!(date_of_day(day_of_date(2026, 11, 25)), (2026, 11, 25));
        assert_eq!(date_of_day(day_of_date(2024, 2, 29)), (2024, 2, 29));
        let nov = Period::month(2026, 11);
        assert_eq!(nov.days(), 30);
        assert_eq!(nov.before(), Period::month(2026, 10));
        assert_eq!(Period::month(2026, 1).before(), Period::month(2025, 12));
        assert_eq!(Period::year(2024).days(), 366);
        assert_eq!(Period::parse(&nov.key()), Some(nov));
        assert_eq!(Period::parse("year:2026"), Some(Period::year(2026)));
        // On December 3rd, November's; on January 5th, December's and the year's.
        assert_eq!(offered(day_of_date(2026, 12, 3)), vec![nov]);
        assert_eq!(offered(day_of_date(2027, 1, 5)), vec![Period::month(2026, 12), Period::year(2026)]);
        assert_eq!(offered(day_of_date(2027, 1, 10)), vec![Period::year(2026)]);
        assert!(offered(day_of_date(2026, 11, 25)).is_empty());
    }

    #[test]
    fn chapters_marked_in_bulk_are_not_reading() {
        let t = 1_000_000_000_000;
        let reads = vec![r(1, 1.0, t), r(1, 2.0, t + 500), r(1, 3.0, t + 900), r(1, 4.0, t + 20 * 60_000), r(2, 1.0, t + 100), r(2, 2.0, t + 400)];
        let c = clean(&reads);
        // Series 1's first three came within a second of each other: marked, not read. Series 2's
        // two are too few to tell.
        assert_eq!(c.iter().map(|x| (x.series, x.number as i64)).collect::<Vec<_>>(), vec![(2, 1), (2, 2), (1, 4)]);
    }

    #[test]
    fn a_month_counts_its_chapters_streak_best_day_and_new_series() {
        let nov = Period::month(2026, 11);
        let at = |d: u32, h: i64| day_of_date(2026, 11, d) * 86_400_000 + h * H;
        let mut reads = vec![r(1, 1.0, day_of_date(2026, 10, 20) * 86_400_000)];
        // Series 1 every day from the 9th to the 12th; series 2 begun on the 16th, 6 that day.
        for d in 9..=12 {
            reads.push(r(1, d as f64, at(d, 21)));
        }
        for k in 0..6 {
            reads.push(r(2, 1.0 + k as f64, at(16, 22) + k * 10 * 60_000));
        }
        reads.push(r(1, 13.0, at(16, 23)));
        let known = vec![Known { id: 2, title: "B".into(), caught_up: true }, Known { id: 1, title: "A".into(), caught_up: false }];
        let s = compute(&nov, &reads, &[(day_of_date(2026, 11, 13), 600)], &known, &[], 0);
        assert_eq!(s.chapters, 11);
        assert_eq!(s.before, 1);
        assert_eq!(s.streak, (5, day_of_date(2026, 11, 9)), "the 9th to the 13th, the book's day too");
        assert_eq!(s.days_read, 6);
        let (d, n, first, last, top) = s.best_day.clone().unwrap();
        assert_eq!((d, n), (day_of_date(2026, 11, 16), 7));
        assert_eq!((first, last), (at(16, 22), at(16, 23)));
        assert_eq!(top, vec![(2, 6), (1, 1)]);
        assert_eq!(s.new_series, vec![2], "series 1 was begun in October");
        assert_eq!(s.took, Some((2, 6)));
        assert_eq!(s.top[0].0, 2);
        assert!(s.top[0].3, "series 2 from chapter 1");
        assert_eq!(s.caught_up, vec![2]);
        assert_eq!(s.hours[22], 6);
        // Six chapters ten minutes apart, then one ten minutes on: a sitting of 5 + 60 minutes;
        // series 1's four lone chapters, 5 each.
        assert_eq!(s.comic_secs, (5 + 60 + 4 * 5) * 60);
        assert_eq!(s.book_secs, 600);
        assert_eq!(s.binge, Some((2, 6)));
    }

    #[test]
    fn words_for_the_cards() {
        assert_eq!(hours_words(31 * 3600 + 20 * 60), "31 h 20 m");
        assert_eq!(hours_words(45 * 60), "45 m");
        assert_eq!(hours_words(2 * 3600), "2 h");
        assert_eq!(versus_words(412, 288, "October").as_deref(), Some("Up 43% on October"));
        assert_eq!(versus_words(100, 0, "October"), None);
        let mut h = [0usize; 24];
        h[23] = 8;
        h[14] = 4;
        assert_eq!(hour_words(&h).as_deref(), Some("Most of it after 10 at night."));
    }
}
