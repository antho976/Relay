//! Chapter notices while Leaf is closed: when they are posted. Android's `LeafChaptersJob` asks
//! the server about hourly for new chapters of belled series and hands them here
//! (`nativePlan`, in `android.rs`) with what it held back before; this says what to post now and
//! what to keep for later.
//!
//! - **Quiet hours** (default 23:00 to 08:00): notices are held, and posted at the first run after.
//! - **One summary a day**: nothing is posted one by one; at the first run past the chosen time
//!   (default 09:00) one notice says "12 new chapters in 5 series", once a day.
//!
//! The settings live in `files/config.json` under `notify`, where the job reads them.

use leaf_proto::Event;
use serde::{Deserialize, Serialize};

/// The notice settings, as kept in `config.json` (`notify`). Times are minutes into the day.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Notify {
    pub quiet: bool,
    pub quiet_from: u16,
    pub quiet_to: u16,
    pub summary: bool,
    pub summary_at: u16,
}

impl Default for Notify {
    fn default() -> Notify {
        Notify { quiet: true, quiet_from: 23 * 60, quiet_to: 8 * 60, summary: false, summary_at: 9 * 60 }
    }
}

/// A notice not posted yet.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Held {
    pub at: i64,
    pub series: Option<i64>,
    /// How many chapters it tells of.
    pub chapters: u32,
    pub text: String,
}

/// What the job keeps between runs.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct JobState {
    pub held: Vec<Held>,
    /// The local day the last summary went out (days from 1970-01-01).
    pub summary_day: i64,
}

/// A notice to post now.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Post {
    pub id: i32,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Plan {
    pub post: Vec<Post>,
    pub state: JobState,
}

/// `minute` (into the day) falls in the quiet hours. A window across midnight (23:00 to 08:00)
/// wraps; one that starts where it ends is empty.
pub fn in_quiet(n: &Notify, minute: u16) -> bool {
    let (from, to) = (n.quiet_from % 1440, n.quiet_to % 1440);
    n.quiet && if from <= to { minute >= from && minute < to } else { minute >= from || minute < to }
}

/// How many chapters an event tells of: the number it starts with ("3 new chapters of ..."), else 1.
pub fn chapters_in(text: &str) -> u32 {
    let n: String = text.trim_start().chars().take_while(char::is_ascii_digit).collect();
    n.parse().ok().filter(|n| *n > 0).unwrap_or(1)
}

/// "12 new chapters in 5 series"; one series: "4 new chapters of Nano Machine" (its title taken
/// from the server's own words when they read "... of TITLE, up to N").
pub fn summary_words(held: &[Held]) -> String {
    let total: u32 = held.iter().map(|h| h.chapters).sum();
    let mut series: Vec<String> = held.iter().map(|h| h.series.map_or_else(|| h.text.clone(), |s| s.to_string())).collect();
    series.sort();
    series.dedup();
    let chapters = if total == 1 { "1 new chapter".to_string() } else { format!("{total} new chapters") };
    if series.len() == 1 {
        if held.len() == 1 {
            return held[0].text.clone();
        }
        if let Some(title) = held.last().and_then(|h| h.text.split_once(" of ")).and_then(|(_, r)| r.rsplit_once(", up to")).map(|(t, _)| t.trim()) {
            return format!("{chapters} of {title}");
        }
    }
    format!("{chapters} in {} series", series.len())
}

/// What to post now, given the settings, what was held, the events just fetched (only "new"
/// ones are notices), the time and the zone's offset from UTC in minutes.
pub fn plan(n: &Notify, mut state: JobState, events: &[Event], now_ms: i64, offset_min: i32) -> Plan {
    let local = now_ms.div_euclid(60_000) + offset_min as i64;
    let minute = local.rem_euclid(1440) as u16;
    let day = local.div_euclid(1440);
    for e in events.iter().filter(|e| e.kind == "new") {
        state.held.push(Held { at: e.at, series: e.series, chapters: chapters_in(&e.text), text: e.text.clone() });
    }
    let mut post = Vec::new();
    if n.summary {
        if minute >= n.summary_at % 1440 && state.summary_day < day {
            if !state.held.is_empty() {
                post.push(Post { id: 0x5000_0000 | (day as i32 & 0x0FFF_FFFF), text: summary_words(&state.held) });
                state.held.clear();
            }
            state.summary_day = day;
        }
    } else if !in_quiet(n, minute) {
        for h in state.held.drain(..) {
            post.push(Post { id: (h.at % i32::MAX as i64) as i32, text: h.text });
        }
    }
    Plan { post, state }
}

/// The job's question and answer as JSON, for `LeafChaptersJob` (through JNI): `notify` is the
/// config's `notify` object, `state` what the last run kept ({} the first time), `events` the
/// server's answer. Answers `{"post":[{"id","text"}],"state":{...}}`.
pub fn plan_json(notify: &str, state: &str, events: &str, now_ms: i64, offset_min: i32) -> String {
    let n: Notify = serde_json::from_str(notify).unwrap_or_default();
    let st: JobState = serde_json::from_str(state).unwrap_or_default();
    let ev: Vec<Event> = serde_json::from_str(events).unwrap_or_default();
    serde_json::to_string(&plan(&n, st, &ev, now_ms, offset_min)).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600_000;

    fn ev(at: i64, series: i64, text: &str) -> Event {
        Event { at, series: Some(series), kind: "new".into(), text: text.into() }
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        let n = Notify::default();
        assert!(in_quiet(&n, 23 * 60) && in_quiet(&n, 0) && in_quiet(&n, 7 * 60 + 59));
        assert!(!in_quiet(&n, 8 * 60) && !in_quiet(&n, 22 * 60 + 59));
        let day = Notify { quiet_from: 13 * 60, quiet_to: 14 * 60, ..n };
        assert!(in_quiet(&day, 13 * 60 + 30) && !in_quiet(&day, 14 * 60));
        assert!(!in_quiet(&Notify { quiet: false, ..n }, 0));
        assert!(!in_quiet(&Notify { quiet_from: 60, quiet_to: 60, ..n }, 60));
    }

    #[test]
    fn notices_in_quiet_hours_wait_for_the_morning() {
        let n = Notify::default();
        let day0 = 20_000 * 24 * H;
        // 23:30 UTC+1 (22:30 UTC): held.
        let p = plan(&n, JobState::default(), &[ev(day0 + 22 * H, 1, "2 new chapters of A, up to 5")], day0 + 22 * H + 30 * 60_000, 60);
        assert!(p.post.is_empty());
        assert_eq!(p.state.held.len(), 1);
        // 03:00 local: still held, one more comes; a switch is not a notice.
        let mut more = vec![ev(day0 + 26 * H, 2, "1 new chapter of B, up to 9")];
        more.push(Event { at: day0 + 26 * H, series: Some(1), kind: "switched".into(), text: "moved".into() });
        let p = plan(&n, p.state, &more, day0 + 26 * H, 60);
        assert!(p.post.is_empty());
        assert_eq!(p.state.held.len(), 2);
        // 08:10 local: both go, in order, and nothing is kept.
        let p = plan(&n, p.state, &[], day0 + 31 * H + 10 * 60_000, 60);
        assert_eq!(p.post.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), vec!["2 new chapters of A, up to 5", "1 new chapter of B, up to 9"]);
        assert!(p.state.held.is_empty());
        // Outside quiet hours a notice goes at once.
        let p = plan(&n, JobState::default(), &[ev(day0 + 12 * H, 1, "x")], day0 + 12 * H, 0);
        assert_eq!(p.post.len(), 1);
    }

    #[test]
    fn a_summary_goes_once_a_day_at_its_time() {
        let n = Notify { summary: true, ..Notify::default() };
        let day0 = 20_000 * 24 * H;
        let events = [ev(day0 + 2 * H, 1, "3 new chapters of A, up to 5"), ev(day0 + 3 * H, 2, "1 new chapter of B, up to 9"), ev(day0 + 4 * H, 1, "2 new chapters of A, up to 7")];
        let p = plan(&n, JobState::default(), &events, day0 + 8 * H, 0);
        assert!(p.post.is_empty(), "before nine, held");
        let p = plan(&n, p.state, &[ev(day0 + 9 * H, 3, "6 new chapters of C, up to 60")], day0 + 9 * H + 5 * 60_000, 0);
        assert_eq!(p.post.len(), 1);
        assert_eq!(p.post[0].text, "12 new chapters in 3 series");
        assert!(p.state.held.is_empty());
        // Later the same day: held for tomorrow's.
        let p = plan(&n, p.state, &[ev(day0 + 12 * H, 1, "1 new chapter of A, up to 8")], day0 + 12 * H, 0);
        assert!(p.post.is_empty());
        assert_eq!(p.state.held.len(), 1);
        let p = plan(&n, p.state, &[ev(day0 + 30 * H, 1, "1 new chapter of A, up to 9")], day0 + 33 * H, 0);
        assert_eq!(p.post[0].text, "2 new chapters of A");
        // Nothing new: no notice, the day is still marked.
        let p = plan(&n, p.state, &[], day0 + 57 * H + 60_000, 0);
        assert!(p.post.is_empty());
        assert_eq!(p.state.summary_day, 20_002);
    }

    #[test]
    fn the_job_speaks_json() {
        assert_eq!(chapters_in("12 new chapters of X"), 12);
        assert_eq!(chapters_in("New chapters"), 1);
        let out = plan_json(r#"{"quiet":false}"#, "{}", r#"[{"at":5,"series":1,"kind":"new","text":"1 new chapter of A, up to 2"}]"#, 10, 0);
        let p: Plan = serde_json::from_str(&out).unwrap();
        assert_eq!(p.post, vec![Post { id: 5, text: "1 new chapter of A, up to 2".into() }]);
        // Garbage in: nothing posted, nothing lost.
        let p: Plan = serde_json::from_str(&plan_json("x", "y", "z", 0, 0)).unwrap();
        assert!(p.post.is_empty());
    }
}
