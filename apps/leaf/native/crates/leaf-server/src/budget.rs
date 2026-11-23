//! Each source asked politely: a per-source request budget for everything that reaches a site
//! through Suwayomi (a search, a refresh, a chapter's page list, a download handed over).
//!
//! - **Background** work (the hourly pass, comparisons, recommendations, the download queue)
//!   books the source's next slot and waits for it: one request every [`DEFAULT_GAP_MS`]
//!   (MangaDex every 1.5 s), `LEAF_SOURCE_GAP_MS` to change them.
//! - **Foreground** work the phone is waiting on (pages, a search typed, a follow) is never
//!   delayed, only counted; the background waits a gap after it.
//! - An answer that looks like a rate limit (429, "Too Many Requests") or a Cloudflare check that
//!   failed pauses the source for background work for an hour, doubling on repeats up to 12 h.
//!
//! Time is passed in (ms since the epoch), so the rules are tested without sleeping.

use std::collections::HashMap;
use std::sync::Mutex;

pub const DEFAULT_GAP_MS: i64 = 3_000;
/// MangaDex asks clients for at most five requests a second; 1.5 s is well under it.
/// AniList allows 90 requests a minute, 30 while it says it is degraded: one every 2 s stays under both.
const KNOWN_GAPS: [(&str, i64); 2] = [("MangaDex", 1_500), ("AniList", 2_000)];
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

    /// Background: books the source's next slot and answers how long to wait for it (0: go now),
    /// or `Err(until)` while the source is paused (nothing is booked).
    pub fn book(&self, source: &str, now: i64) -> Result<i64, i64> {
        let gap = self.gap(source);
        let mut lines = self.lines();
        let line = lines.entry(source.to_string()).or_default();
        if line.pause.until > now {
            return Err(line.pause.until);
        }
        let at = line.next_at.max(now);
        line.next_at = at + gap;
        Self::count(line, Lane::Back, now);
        Ok(at - now)
    }

    /// Foreground: counted and never delayed; the next background request waits a gap after it.
    pub fn fore(&self, source: &str, now: i64) {
        let gap = self.gap(source);
        let mut lines = self.lines();
        let line = lines.entry(source.to_string()).or_default();
        line.next_at = line.next_at.max(now + gap);
        Self::count(line, Lane::Fore, now);
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
    fn foreground_is_never_delayed_but_counted() {
        let b = Budget::new(None);
        let t = 5_000_000;
        for _ in 0..5 {
            b.fore("Asura Scans", t);
        }
        assert_eq!(b.today("Asura Scans", t), Count { fore: 5, back: 0 });
        assert_eq!(b.book("Asura Scans", t + 1_000), Ok(2_000), "the background waits a gap after the reader");
        // Counts start again each day.
        assert_eq!(b.today("Asura Scans", t + DAY_MS), Count::default());
    }

    #[test]
    fn gaps_are_set_by_the_environment() {
        let b = Budget::new(Some("5000, MangaDex=800 ,Weeb Central=10000,bad=x"));
        assert_eq!(b.gap("Asura Scans"), 5_000);
        assert_eq!(b.gap("mangadex"), 800);
        assert_eq!(b.gap("Weeb Central"), 10_000);
        assert_eq!(b.gap("AniList"), 2_000);
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
        b.fore("Asura Scans", t + 10);
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
