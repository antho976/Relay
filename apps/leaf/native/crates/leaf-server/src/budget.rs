//! Each source asked politely: a per-source request budget for everything that reaches a site
//! through Suwayomi (a search, a refresh, a chapter's page list, a page's picture, a download
//! handed over) and AniList.
//!
//! - **Background** work (the hourly pass, comparisons, recommendations, the download queue)
//!   books the source's next slot and waits for it: one request every [`DEFAULT_GAP_MS`]
//!   (MangaDex every 1.5 s, AniList every 3 s), `LEAF_SOURCE_GAP_MS` to change them, and at most
//!   [`Limits::back_per_day`] a day (then it waits for tomorrow, as for a pause).
//! - **Foreground** work the phone is waiting on (pages, a search typed, a follow) goes at once
//!   while the source is under its ceilings; over one it waits for the next free moment (a
//!   second or two), and is refused as busy if that is more than [`FORE_MAX_WAIT_MS`] away. The
//!   background waits a gap after it.
//! - **Ceilings** ([`Limits`], [`limits_for`]) count both lanes together, per second and per
//!   minute, with pictures, page lists and the rest counted apart: below what each provider
//!   publishes (MangaDex: 5 a second and 40 page lists a minute; AniList: 90 a minute, 30 while
//!   degraded), and modest where a site publishes nothing.
//! - An answer that looks like a rate limit (429, "Too Many Requests") or a Cloudflare check that
//!   failed pauses the source's background work for an hour, doubling on repeats up to 12 h;
//!   meanwhile the foreground is held to a quarter of its ceilings.
//!
//! Time is passed in (ms since the epoch), so the rules are tested without sleeping.

use std::collections::HashMap;
use std::sync::Mutex;

pub const DEFAULT_GAP_MS: i64 = 3_000;
/// MangaDex asks clients for at most five requests a second; 1.5 s is well under it.
/// AniList allows 90 requests a minute, 30 while it says it is degraded: one every 3 s is 20.
const KNOWN_GAPS: [(&str, i64); 2] = [("MangaDex", 1_500), ("AniList", 3_000)];
/// The longest the phone's request waits for a source's ceiling before it is told "busy".
pub const FORE_MAX_WAIT_MS: i64 = 5_000;

/// What a request is, for its source's ceilings: pictures come from image servers and are counted
/// apart from the site's pages and API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A search, a series refreshed, a download handed over, a list page, an AniList query.
    Api,
    /// A chapter's list of pages (MangaDex's at-home server allows 40 a minute).
    Pages,
    /// One page's picture.
    Images,
}

/// A ceiling: at most this many a second and a minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub per_sec: usize,
    pub per_min: usize,
}

/// A source's ceilings, both lanes together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub api: Rate,
    pub pages: Rate,
    pub images: Rate,
    /// Background requests a day (UTC), every kind: past it the autopilot waits for tomorrow.
    pub back_per_day: u64,
}

impl Limits {
    pub fn rate(&self, kind: Kind) -> Rate {
        match kind {
            Kind::Api => self.api,
            Kind::Pages => self.pages,
            Kind::Images => self.images,
        }
    }
}

/// The ceilings for a source.
/// - MangaDex publishes 5 requests a second per address and 40 a minute for page lists: we stay
///   at 2 a second, 60 a minute, and 20 page lists a minute.
/// - AniList publishes 90 a minute (30 while degraded): 1 a second, 20 a minute.
/// - Webtoons.com publishes nothing and is read only for its list pages: 1 a second, 10 a minute.
/// - Every other site publishes nothing: 1 a second, 20 a minute for its pages and API, and a
///   steady 4 pictures a second (a chapter read as it comes, not downloaded first).
pub fn limits_for(source: &str) -> Limits {
    let s = source.to_lowercase();
    // Pictures: a steady 4 a second (a browser asks for a whole chapter's at once).
    let pics = Rate { per_sec: 4, per_min: 240 };
    match s.as_str() {
        "mangadex" => Limits { api: Rate { per_sec: 2, per_min: 60 }, pages: Rate { per_sec: 1, per_min: 20 }, images: pics, back_per_day: 5_000 },
        "anilist" => Limits { api: Rate { per_sec: 1, per_min: 20 }, pages: Rate { per_sec: 1, per_min: 20 }, images: pics, back_per_day: 3_000 },
        "webtoons.com" => Limits { api: Rate { per_sec: 1, per_min: 10 }, pages: Rate { per_sec: 1, per_min: 20 }, images: pics, back_per_day: 1_000 },
        _ => Limits { api: Rate { per_sec: 1, per_min: 20 }, pages: Rate { per_sec: 1, per_min: 20 }, images: pics, back_per_day: 3_000 },
    }
}
pub const PAUSE_FIRST_MS: i64 = 3_600_000;
pub const PAUSE_MAX_MS: i64 = 12 * 3_600_000;
/// A rate limit this long after the last one starts again at an hour.
const PAUSE_FORGET_MS: i64 = 24 * 3_600_000;
const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// The phone is waiting: never delayed, counted.
    Fore,
    /// The autopilot: waits its turn, and not at all while the source is paused.
    Back,
}

/// Requests sent to a source today (UTC), by lane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub fore: u64,
    pub back: u64,
}

/// A source's pause, as kept across restarts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pause {
    pub until: i64,
    /// Pauses in a row (each twice the one before).
    pub level: u32,
    pub last_at: i64,
    pub why: String,
}

#[derive(Default)]
struct Line {
    next_at: i64,
    pause: Pause,
    day: i64,
    count: Count,
    /// When requests went (or are booked to go) in the last minute, by kind.
    sent: HashMap<Kind, Vec<i64>>,
}

impl Line {
    /// The first moment from `at` when one more request of `kind` stays under `rate`.
    fn free_at(&mut self, kind: Kind, rate: Rate, at: i64) -> i64 {
        let sent = self.sent.entry(kind).or_default();
        let mut at = at;
        // Each step moves past the oldest request that keeps it over, so this ends.
        for _ in 0..(rate.per_min + rate.per_sec + 2) {
            let late = |span: i64, max: usize| -> Option<i64> {
                let mut inside: Vec<i64> = sent.iter().copied().filter(|t| *t > at - span && *t <= at).collect();
                if inside.len() < max.max(1) {
                    return None;
                }
                inside.sort_unstable();
                Some(inside[inside.len() - max.max(1)] + span)
            };
            match late(1_000, rate.per_sec).or_else(|| late(60_000, rate.per_min)) {
                Some(t) => at = t,
                None => break,
            }
        }
        at
    }

    fn sent(&mut self, kind: Kind, at: i64, now: i64) {
        let v = self.sent.entry(kind).or_default();
        v.retain(|t| *t > now - 60_000);
        v.push(at);
    }
}

pub struct Budget {
    default_gap: i64,
    gaps: HashMap<String, i64>,
    lines: Mutex<HashMap<String, Line>>,
}

/// Whether a source's error says it wants fewer requests: a 429, "Too Many Requests", a rate
/// limit, or a Cloudflare check that could not be passed.
pub fn looks_limited(err: &str) -> bool {
    let e = err.to_lowercase();
    let a_429 = e.split(|c: char| !c.is_ascii_digit()).any(|n| n == "429");
    a_429 || e.contains("too many requests") || e.contains("rate limit") || e.contains("ratelimit") || e.contains("cloudflare")
}

impl Budget {
    /// `spec` is `LEAF_SOURCE_GAP_MS`: a number for every source, `Name=ms` for one, or both
    /// (`"3000,MangaDex=1500"`).
    pub fn new(spec: Option<&str>) -> Budget {
        let mut default_gap = DEFAULT_GAP_MS;
        let mut gaps: HashMap<String, i64> = KNOWN_GAPS.iter().map(|(n, g)| (n.to_lowercase(), *g)).collect();
        for part in spec.unwrap_or("").split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.rsplit_once('=') {
                Some((name, ms)) => {
                    if let Ok(ms) = ms.trim().parse::<i64>() {
                        gaps.insert(name.trim().to_lowercase(), ms.max(0));
                    }
                }
                None => {
                    if let Ok(ms) = part.parse::<i64>() {
                        default_gap = ms.max(0);
                    }
                }
            }
        }
        Budget { default_gap, gaps, lines: Mutex::new(HashMap::new()) }
    }

    pub fn gap(&self, source: &str) -> i64 {
        self.gaps.get(&source.to_lowercase()).copied().unwrap_or(self.default_gap)
    }

    fn lines(&self) -> std::sync::MutexGuard<'_, HashMap<String, Line>> {
        self.lines.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn count(line: &mut Line, lane: Lane, now: i64) {
        if line.day != now / DAY_MS {
            line.day = now / DAY_MS;
            line.count = Count::default();
        }
        match lane {
            Lane::Fore => line.count.fore += 1,
            Lane::Back => line.count.back += 1,
        }
    }

    /// Background: books the source's next slot (an API request) and answers how long to wait for
    /// it (0: go now), or `Err(until)` while the source is paused or has had its day's share
    /// (nothing is booked).
    #[cfg(test)]
    pub fn book(&self, source: &str, now: i64) -> Result<i64, i64> {
        self.book_as(source, Kind::Api, now)
    }

    pub fn book_as(&self, source: &str, kind: Kind, now: i64) -> Result<i64, i64> {
        let gap = self.gap(source);
        let limits = limits_for(source);
        let mut lines = self.lines();
        let line = lines.entry(source.to_string()).or_default();
        if line.pause.until > now {
            return Err(line.pause.until);
        }
        if line.day == now / DAY_MS && line.count.back >= limits.back_per_day {
            return Err((now / DAY_MS + 1) * DAY_MS);
        }
        let at = line.free_at(kind, limits.rate(kind), line.next_at.max(now));
        line.next_at = at + gap;
        line.sent(kind, at, now);
        Self::count(line, Lane::Back, now);
        Ok(at - now)
    }

    /// Foreground: goes at once under the source's ceilings, else answers how long to wait for
    /// the next free moment; `Err(at)` (nothing counted) when that is more than
    /// [`FORE_MAX_WAIT_MS`] away. While the source is paused, a quarter of its ceilings. The next
    /// background request waits a gap after it.
    pub fn fore(&self, source: &str, kind: Kind, now: i64) -> Result<i64, i64> {
        let gap = self.gap(source);
        let mut rate = limits_for(source).rate(kind);
        let mut lines = self.lines();
        let line = lines.entry(source.to_string()).or_default();
        if line.pause.until > now {
            rate = Rate { per_sec: 1, per_min: (rate.per_min / 4).max(2) };
        }
        let at = line.free_at(kind, rate, now);
        if at - now > FORE_MAX_WAIT_MS {
            return Err(at);
        }
        line.sent(kind, at, now);
        line.next_at = line.next_at.max(at + gap);
        Self::count(line, Lane::Fore, now);
        Ok(at - now)
    }

    /// What the source answered. A rate limit pauses it (an hour, twice as long on each repeat,
    /// 12 h at most); answers the new pause when one began, to be kept and told.
    pub fn answer(&self, source: &str, err: Option<&str>, now: i64) -> Option<Pause> {
        let e = err.filter(|e| looks_limited(e))?;
        let mut lines = self.lines();
        let line = lines.entry(source.to_string()).or_default();
        if line.pause.until > now {
            // Already paused: a request sent before the pause answered late.
            return None;
        }
        if now - line.pause.last_at > PAUSE_FORGET_MS {
            line.pause.level = 0;
        }
        line.pause.level += 1;
        let long = PAUSE_FIRST_MS.saturating_mul(1 << (line.pause.level - 1).min(8)).min(PAUSE_MAX_MS);
        line.pause = Pause { until: now + long, level: line.pause.level, last_at: now, why: e.chars().take(200).collect() };
        Some(line.pause.clone())
    }

    /// The source's pause, while it lasts.
    pub fn paused(&self, source: &str, now: i64) -> Option<Pause> {
        self.lines().get(source).map(|l| l.pause.clone()).filter(|p| p.until > now)
    }

    /// A pause kept from before a restart.
    pub fn restore(&self, source: &str, pause: Pause) {
        self.lines().entry(source.to_string()).or_default().pause = pause;
    }

    /// Requests sent today to the source.
    pub fn today(&self, source: &str, now: i64) -> Count {
        self.lines().get(source).filter(|l| l.day == now / DAY_MS).map(|l| l.count).unwrap_or_default()
    }
}

impl Pause {
    /// Kept in the store as `until|level|last_at|why`.
    pub fn encode(&self) -> String {
        format!("{}|{}|{}|{}", self.until, self.level, self.last_at, self.why)
    }

    pub fn decode(s: &str) -> Option<Pause> {
        let mut p = s.splitn(4, '|');
        Some(Pause { until: p.next()?.parse().ok()?, level: p.next()?.parse().ok()?, last_at: p.next()?.parse().ok()?, why: p.next().unwrap_or("").to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_waits_its_turn_per_source() {
        let b = Budget::new(None);
        let t = 1_000_000;
        assert_eq!(b.book("Asura Scans", t), Ok(0), "the first goes at once");
        assert_eq!(b.book("Asura Scans", t), Ok(3_000), "the next a gap later");
        assert_eq!(b.book("Asura Scans", t + 1_000), Ok(5_000), "and the one after, booked behind it");
        assert_eq!(b.book("Drake Scans", t), Ok(0), "another source has its own turn");
        assert_eq!(b.book("MangaDex", t), Ok(0));
        assert_eq!(b.book("MangaDex", t), Ok(1_500), "MangaDex every 1.5 s");
        assert_eq!(b.book("Asura Scans", t + 60_000), Ok(0), "a quiet source goes at once");
        assert_eq!(b.today("Asura Scans", t + 60_000), Count { fore: 0, back: 4 });
    }

    #[test]
    fn foreground_goes_at_once_under_the_ceilings_and_is_counted() {
        let b = Budget::new(None);
        let t = 5_000_000;
        assert_eq!(b.fore("Asura Scans", Kind::Api, t), Ok(0));
        assert_eq!(b.fore("Asura Scans", Kind::Images, t), Ok(0), "pictures are counted apart");
        assert_eq!(b.fore("Asura Scans", Kind::Pages, t), Ok(0));
        assert_eq!(b.today("Asura Scans", t), Count { fore: 3, back: 0 });
        assert_eq!(b.book("Asura Scans", t + 1_000), Ok(2_000), "the background waits a gap after the reader");
        // Counts start again each day.
        assert_eq!(b.today("Asura Scans", t + DAY_MS), Count::default());
    }

    #[test]
    fn the_phone_waits_for_a_source_over_its_ceiling_and_is_told_busy_past_a_few_seconds() {
        let b = Budget::new(None);
        let t = 7_000_000;
        // Pictures: 4 a second.
        let waits: Vec<i64> = (0..9).map(|_| b.fore("Asura Scans", Kind::Images, t).unwrap()).collect();
        assert_eq!(waits, vec![0, 0, 0, 0, 1_000, 1_000, 1_000, 1_000, 2_000]);
        // A page list a second: the next is a second away; five seconds of them, then busy.
        assert_eq!(b.fore("Asura Scans", Kind::Pages, t), Ok(0));
        for s in 1..=5 {
            assert_eq!(b.fore("Asura Scans", Kind::Pages, t), Ok(s * 1_000));
        }
        assert_eq!(b.fore("Asura Scans", Kind::Pages, t), Err(t + 6_000), "too far off: busy, and not counted");
        assert_eq!(b.today("Asura Scans", t).fore, 15);
        // A minute's share: twenty searches spread, the twenty-first waits for the first to age out.
        let b = Budget::new(None);
        for i in 0..20 {
            assert_eq!(b.fore("Weeb Central", Kind::Api, t + i * 1_000), Ok(0));
        }
        assert_eq!(b.fore("Weeb Central", Kind::Api, t + 59_000), Ok(1_000));
    }

    #[test]
    fn every_ceiling_is_below_what_the_provider_publishes() {
        let md = limits_for("MangaDex");
        assert!(md.api.per_sec < 5 && md.api.per_min < 5 * 60, "MangaDex: 5 a second");
        assert!(md.pages.per_min < 40, "MangaDex at-home: 40 page lists a minute");
        let al = limits_for("AniList");
        assert!(al.api.per_min < 30, "AniList: 90 a minute, 30 while degraded");
        assert!(60_000 / Budget::new(None).gap("AniList") < 30);
        assert!(60_000 / Budget::new(None).gap("MangaDex") < 5 * 60);
        let other = limits_for("Asura Scans");
        assert!(other.api.per_min <= 20 && other.images.per_sec <= 4);
    }

    #[test]
    fn the_background_stops_at_its_days_share() {
        let b = Budget::new(Some("0"));
        let t = 20 * DAY_MS + 1_000;
        let day = limits_for("Webtoons.com").back_per_day;
        let mut at = t;
        for _ in 0..day {
            let w = b.book("Webtoons.com", at).unwrap();
            at += w;
        }
        assert_eq!(b.book("Webtoons.com", at), Err(21 * DAY_MS), "tomorrow");
        assert!(b.fore("Webtoons.com", Kind::Api, at + 60_000).is_ok(), "the phone still goes");
        assert_eq!(b.book("Webtoons.com", 21 * DAY_MS), Ok(0), "a new day");
    }

    #[test]
    fn while_paused_the_phone_gets_a_quarter() {
        let b = Budget::new(None);
        let t = 30 * DAY_MS;
        let at_once = |b: &Budget, t: i64| (0..10).filter(|i| b.fore("Asura Scans", Kind::Api, t + i * 1_000) == Ok(0)).count();
        assert_eq!(at_once(&b, t), 10, "a search a second is under its ceiling");
        b.answer("Asura Scans", Some("429"), t + 61_000).unwrap();
        assert_eq!(at_once(&b, t + 122_000), 5, "paused: five a minute");
    }

    #[test]
    fn gaps_are_set_by_the_environment() {
        let b = Budget::new(Some("5000, MangaDex=800 ,Weeb Central=10000,bad=x"));
        assert_eq!(b.gap("Asura Scans"), 5_000);
        assert_eq!(b.gap("mangadex"), 800);
        assert_eq!(b.gap("Weeb Central"), 10_000);
        assert_eq!(b.gap("AniList"), 3_000);
        assert_eq!(Budget::new(Some("")).gap("Asura Scans"), DEFAULT_GAP_MS);
    }

    #[test]
    fn a_rate_limit_pauses_the_background_and_doubles_on_repeats() {
        let b = Budget::new(None);
        let h = 3_600_000;
        let t = 10 * DAY_MS;
        assert!(!looks_limited("chapter 4291 has no pages"), "429 only as a number of its own");
        assert!(looks_limited("Cloudflare bypass currently disabled"));
        assert!(b.answer("Asura Scans", None, t).is_none());
        assert!(b.answer("Asura Scans", Some("timeout"), t).is_none(), "a slow site is not a rate limit");
        let p = b.answer("Asura Scans", Some("HTTP error 429"), t).expect("paused");
        assert_eq!((p.until, p.level), (t + h, 1));
        assert_eq!(b.book("Asura Scans", t + 10), Err(t + h), "background waits out the pause");
        let _ = b.fore("Asura Scans", Kind::Api, t + 10);
        assert!(b.answer("Asura Scans", Some("Too Many Requests"), t + 20).is_none(), "a late answer does not double it");
        assert_eq!(b.book("Asura Scans", t + h), Ok(0), "over after an hour");
        let p = b.answer("Asura Scans", Some("Failed to bypass Cloudflare"), t + h + 5).unwrap();
        assert_eq!(p.until - (t + h + 5), 2 * h, "a repeat is twice as long");
        let mut at = p.until;
        for _ in 0..6 {
            at = b.answer("Asura Scans", Some("429"), at).unwrap().until;
        }
        assert_eq!(b.paused("Asura Scans", at - 1).unwrap().until - b.paused("Asura Scans", at - 1).unwrap().last_at, PAUSE_MAX_MS, "12 h at most");
        // A day and more without one: back to an hour.
        let later = at + 2 * DAY_MS;
        let p = b.answer("Asura Scans", Some("429"), later).unwrap();
        assert_eq!(p.until - later, h);
        assert!(b.paused("Drake Scans", later).is_none(), "other sources go on");
    }

    #[test]
    fn a_pause_survives_a_restart() {
        let p = Pause { until: 99, level: 2, last_at: 50, why: "HTTP 429 | slow down".into() };
        assert_eq!(Pause::decode(&p.encode()), Some(p.clone()));
        let b = Budget::new(None);
        b.restore("Asura Scans", p);
        assert_eq!(b.book("Asura Scans", 60), Err(99));
        assert_eq!(Pause::decode("nonsense"), None);
    }
}
