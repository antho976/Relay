//! Auto downloads (the owner, 2026-11-27): chapters kept on this phone, chosen by how much you
//! are into each series *now*, not by how much of it you ever read. Each chapter read counts
//! e^(−days ago / 7) ([`momentum`]): fifty chapters read in two days outweigh three hundred read
//! three months ago (chapters the import marked read count nothing, they have no time).
//!
//! - **Which series** ([`rank`]): ones you are reading now, best first; ones you have only started
//!   (fewer than [`STARTED`] chapters read) and ones you slowed down on (nothing read for
//!   [`SLOWED_DAYS`], or read only before Leaf; on hold) only when their setting is on. Never one
//!   not started, dropped or done.
//! - **How many**: the next unread chapters of each, up to the chapters per series set, until the
//!   room set on this phone is full ([`choose`]; a chapter's size guessed from those saved).
//! - **When** ([`due`]): once a night after midnight, from Android's job while Leaf is closed
//!   (`LeafNightJob`, [`night`]) or from the app when it runs then; and at once when a setting
//!   changes. Each run deletes the ones you have read ([`read_autos`]) and the ones past the room,
//!   then tops up: the same number, newer chapters. It records itself in `offline/smart.json`.
//! - Only its own: chapters you downloaded yourself are never touched ([`crate::offline`]'s
//!   `auto`). Its downloads go through the same line, so the server fetches each chapter once,
//!   inside each source's budget.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use leaf_proto::{Chapter, HistoryRead, Series, SeriesDetail};
use serde::{Deserialize, Serialize};

use crate::net::{Client, Server};
use crate::offline::{Manifest, Saver};

/// The settings (Settings › Auto downloads), in `config.json` under `reading.smart`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct SmartPrefs {
    pub on: bool,
    /// The room its chapters may take on this phone, in MB.
    pub room_mb: u32,
    /// At most this many unread chapters of one series.
    pub per_series: u32,
    /// Series only started are in too.
    pub started: bool,
    /// Series you slowed down on are in too.
    pub slowed: bool,
    /// Only on Wi-Fi (an unmetered network).
    pub wifi_only: bool,
}

impl Default for SmartPrefs {
    fn default() -> SmartPrefs {
        SmartPrefs { on: false, room_mb: 2000, per_series: 5, started: true, slowed: false, wifi_only: true }
    }
}

/// The rooms offered, in MB.
pub const ROOMS_MB: [u32; 9] = [500, 1000, 2000, 3000, 5000, 8000, 10000, 15000, 20000];
/// The chapters per series offered.
pub const PER_SERIES: [u32; 7] = [2, 3, 5, 8, 10, 15, 20];
/// Fewer chapters read than this: a series only started.
pub const STARTED: f64 = 10.0;
/// Nothing read for this many days: a series you slowed down on.
pub const SLOWED_DAYS: i64 = 21;
/// How fast a chapter read stops counting: e^(−days / this).
const FADE_DAYS: f64 = 7.0;
/// A chapter's size before any is saved to measure.
const CHAPTER_GUESS: u64 = 8 << 20;
const DAY_MS: i64 = 86_400_000;

/// The next value of `list` after (or before) `now`, held at its ends.
pub fn step<T: Copy + PartialOrd>(list: &[T], now: T, up: bool) -> T {
    if up { list.iter().copied().find(|v| *v > now).unwrap_or(list[list.len() - 1]) } else { list.iter().rev().copied().find(|v| *v < now).unwrap_or(list[0]) }
}

/// How much you are into each series now: each chapter read counts e^(−days ago / 7).
pub fn momentum(reads: &[HistoryRead], now_ms: i64) -> HashMap<i64, f64> {
    let mut m = HashMap::new();
    for r in reads.iter().filter(|r| r.at > 0) {
        let days = (now_ms - r.at).max(0) as f64 / DAY_MS as f64;
        *m.entry(r.series).or_insert(0.0) += (-days / FADE_DAYS).exp();
    }
    m
}

/// Why a series is in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Now,
    Started,
    Slowed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub id: i64,
    pub title: String,
    pub score: f64,
    /// Chapters read in the last 7 days.
    pub week: u32,
    pub kind: Kind,
    pub read_upto: f64,
}

/// The series worth keeping chapters of, best first.
pub fn rank(lib: &[Series], reads: &[HistoryRead], prefs: &SmartPrefs, now_ms: i64) -> Vec<Ranked> {
    let score = momentum(reads, now_ms);
    let mut week: HashMap<i64, u32> = HashMap::new();
    let mut last: HashMap<i64, i64> = HashMap::new();
    for r in reads {
        if now_ms - r.at < 7 * DAY_MS {
            *week.entry(r.series).or_default() += 1;
        }
        let l = last.entry(r.series).or_default();
        *l = (*l).max(r.at);
    }
    let mut out: Vec<(Ranked, i64)> = Vec::new();
    for s in lib {
        let Some(upto) = s.read_upto else { continue };
        if matches!(s.shelf.as_deref(), Some("dropped") | Some("done")) || s.latest.is_none_or(|l| l <= upto) {
            continue;
        }
        let seen = s.last_read_at.max(last.get(&s.id).copied().unwrap_or(0));
        let kind = if seen == 0 || now_ms - seen > SLOWED_DAYS * DAY_MS || s.shelf.as_deref() == Some("hold") {
            Kind::Slowed
        } else if upto < STARTED {
            Kind::Started
        } else {
            Kind::Now
        };
        let wanted = match kind {
            Kind::Now => true,
            Kind::Started => prefs.started,
            Kind::Slowed => prefs.slowed,
        };
        if wanted {
            let r = Ranked { id: s.id, title: s.title.clone(), score: score.get(&s.id).copied().unwrap_or(0.0), week: week.get(&s.id).copied().unwrap_or(0), kind, read_upto: upto };
            out.push((r, seen));
        }
    }
    // Most into it first; among equals (none read lately), the one read last, then the one read
    // furthest.
    out.sort_by(|(a, la), (b, lb)| b.score.total_cmp(&a.score).then(lb.cmp(la)).then(b.read_upto.total_cmp(&a.read_upto)));
    out.into_iter().map(|(r, _)| r).collect()
}

/// Auto downloads you have read (its number at or below where you are in its series), or whose
/// series is gone from your library: to delete.
pub fn read_autos(lib: &[Series], autos: &[(i64, f64, i64)]) -> Vec<i64> {
    let upto: HashMap<i64, Option<f64>> = lib.iter().map(|s| (s.id, s.read_upto)).collect();
    autos
        .iter()
        .filter(|(series, number, _)| match upto.get(series) {
            None => true,
            Some(u) => u.is_some_and(|u| *number <= u),
        })
        .map(|(_, _, id)| *id)
        .collect()
}

/// When the room was made smaller: auto downloads past it, the lowest ranked series' newest
/// chapters first (a series no longer ranked before any).
pub fn over_room(ranked: &[Ranked], kept: &[(i64, f64, i64, u64)], room: u64) -> Vec<i64> {
    let mut used: u64 = kept.iter().map(|k| k.3).sum();
    if used <= room {
        return Vec::new();
    }
    let place: HashMap<i64, usize> = ranked.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    let mut order: Vec<&(i64, f64, i64, u64)> = kept.iter().collect();
    order.sort_by(|a, b| {
        let (pa, pb) = (place.get(&a.0).copied().unwrap_or(usize::MAX), place.get(&b.0).copied().unwrap_or(usize::MAX));
        pb.cmp(&pa).then(b.1.total_cmp(&a.1))
    });
    let mut out = Vec::new();
    for k in order {
        if used <= room {
            break;
        }
        used = used.saturating_sub(k.3);
        out.push(k.2);
    }
    out
}

/// A chapter's size, from those saved on this phone (their average), else a guess.
pub fn chapter_bytes(saved: &[Manifest]) -> u64 {
    let sizes: Vec<u64> = saved.iter().map(|m| m.bytes).filter(|b| *b > 0).collect();
    if sizes.is_empty() { CHAPTER_GUESS } else { sizes.iter().sum::<u64>() / sizes.len() as u64 }
}

/// What to put in line: for each series in rank order, its next unread chapters not kept or in
/// line yet, up to the chapters per series counting those it has, while the room holds. `have`:
/// the auto downloads kept or in line after deleting (series, chapter id, bytes); `detail` asks
/// for a series' chapters (the server), only while there is room left.
pub fn choose(ranked: &[Ranked], have: &[(i64, i64, u64)], prefs: &SmartPrefs, each: u64, mut detail: impl FnMut(i64) -> Option<SeriesDetail>) -> Vec<(i64, String, Vec<Chapter>)> {
    let room = prefs.room_mb as u64 * 1_000_000;
    let mut used: u64 = have.iter().map(|h| h.2).sum();
    let taken: HashSet<i64> = have.iter().map(|h| h.1).collect();
    let mut out = Vec::new();
    for r in ranked {
        if used + each > room {
            break;
        }
        let mine = have.iter().filter(|h| h.0 == r.id).count() as u32;
        let mut need = prefs.per_series.saturating_sub(mine);
        if need == 0 {
            continue;
        }
        let Some(d) = detail(r.id) else { continue };
        let upto = d.series.read_upto.unwrap_or(r.read_upto);
        let mut next: Vec<&Chapter> = d.chapters.iter().filter(|c| c.number > upto && !c.read && !taken.contains(&c.id)).collect();
        next.sort_by(|a, b| a.number.total_cmp(&b.number));
        // One copy per number: the first listed.
        next.dedup_by(|a, b| a.number == b.number);
        let mut pick = Vec::new();
        for c in next {
            if need == 0 || used + each > room {
                break;
            }
            pick.push(c.clone());
            used += each;
            need -= 1;
        }
        if !pick.is_empty() {
            out.push((r.id, d.series.title.clone(), pick));
        }
    }
    out
}

/// A run's record (`offline/smart.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Night {
    /// The local day it ran on (days since the epoch), and when (ms).
    pub day: i64,
    pub at: i64,
    pub added: u32,
    pub removed: u32,
    pub error: Option<String>,
}

pub fn day_of(ms: i64, offset_s: i64) -> i64 {
    (ms / 1000 + offset_s).div_euclid(86_400)
}

fn night_file(data: &Path) -> std::path::PathBuf {
    data.join("offline").join("smart.json")
}

pub fn last_night(data: &Path) -> Night {
    std::fs::read(night_file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn keep_night(data: &Path, n: &Night) {
    let _ = std::fs::create_dir_all(data.join("offline"));
    let _ = crate::net::write_atomic(&night_file(data), &serde_json::to_vec(n).unwrap_or_default());
}

/// Tonight's run is still to do: on, and not run yet since midnight.
pub fn due(prefs: &SmartPrefs, last: &Night, now_ms: i64, offset_s: i64) -> bool {
    prefs.on && day_of(now_ms, offset_s) > last.day
}

/// One run at a time in a process (the app's and the night job's).
static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

/// One run: deletes the auto downloads read and those past the room, then puts the next ones in
/// line. Blocking (the server is asked for the library, the last weeks' reading and the details
/// of the series chosen). Recorded in `offline/smart.json`.
pub fn refresh(client: &Client, server: &Server, saver: &Saver, data: &Path, prefs: &SmartPrefs, now_ms: i64, offset_s: i64) -> Night {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return last_night(data);
    }
    let mut n = Night { day: day_of(now_ms, offset_s), at: now_ms, ..Night::default() };
    match run(client, server, saver, prefs, now_ms) {
        Ok((added, removed)) => (n.added, n.removed) = (added, removed),
        Err(e) => {
            // Not counted as tonight's: the next look tries again.
            n.day = last_night(data).day;
            n.error = Some(e);
        }
    }
    keep_night(data, &n);
    RUNNING.store(false, Ordering::SeqCst);
    n
}

fn run(client: &Client, server: &Server, saver: &Saver, prefs: &SmartPrefs, now_ms: i64) -> Result<(u32, u32), String> {
    let words = |f: crate::net::Fail| f.words("your server");
    let lib: Vec<Series> = client.json(server, "/v1/library").map_err(words)?;
    let reads: Vec<HistoryRead> = client.json(server, &format!("/v1/history?since={}", now_ms - 60 * DAY_MS)).map_err(words)?;
    let ranked = rank(&lib, &reads, prefs, now_ms);
    let (saved, queued) = saver.autos();
    let mut gone: Vec<i64> = read_autos(&lib, &saved.iter().map(|m| (m.series, m.chapter.number, m.chapter.id)).chain(queued.iter().map(|i| (i.series, i.chapter.number, i.chapter.id))).collect::<Vec<_>>());
    let each = chapter_bytes(&saver.all_saved());
    let kept: Vec<(i64, f64, i64, u64)> = saved.iter().filter(|m| !gone.contains(&m.chapter.id)).map(|m| (m.series, m.chapter.number, m.chapter.id, m.bytes)).chain(queued.iter().filter(|i| !gone.contains(&i.chapter.id)).map(|i| (i.series, i.chapter.number, i.chapter.id, each))).collect();
    gone.extend(over_room(&ranked, &kept, prefs.room_mb as u64 * 1_000_000));
    let removed = if gone.is_empty() { 0 } else { saver.delete(&gone).0 as u32 };
    let have: Vec<(i64, i64, u64)> = kept.iter().filter(|k| !gone.contains(&k.2)).map(|k| (k.0, k.2, k.3)).collect();
    let picks = choose(&ranked, &have, prefs, each, |id| client.json::<SeriesDetail>(server, &format!("/v1/series/{id}")).ok());
    let mut added = 0;
    for (series, title, chapters) in picks {
        added += saver.add_auto(series, &title, &chapters) as u32;
    }
    Ok((added, removed))
}

/// What the night job reads of `config.json`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct JobConfig {
    server: Option<Server>,
    reading: JobReading,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct JobReading {
    smart: SmartPrefs,
    skip_promo: bool,
}

/// Android's night job (`LeafNightJob`, about hourly while Leaf is closed): tonight's run when it
/// is due, then the chapters saved before the job's time is up. Answers what it did, in words for
/// the log.
pub fn night(data: &Path, now_ms: i64, offset_s: i64, unmetered: bool) -> String {
    let config: JobConfig = std::fs::read(data.join("config.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let prefs = config.reading.smart;
    let Some(server) = config.server else { return "no server".into() };
    if !due(&prefs, &last_night(data), now_ms, offset_s) {
        return "not due".into();
    }
    if prefs.wifi_only && !unmetered {
        return "waiting for Wi-Fi".into();
    }
    let saver = Saver::shared(data.to_path_buf());
    saver.set_server(Some(server.clone()));
    saver.set_show_promo(!config.reading.skip_promo);
    let n = refresh(&Client::new(), &server, &saver, data, &prefs, now_ms, offset_s);
    // The job may run 10 minutes: the line gets most of them.
    let done = saver.wait_idle(8.0 * 60.0);
    format!("added {}, removed {}{}{}", n.added, n.removed, n.error.map(|e| format!(", {e}")).unwrap_or_default(), if done { "" } else { ", still saving" })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn series(id: i64, upto: Option<f64>, latest: f64, last_read_at: i64) -> Series {
        Series {
            id,
            title: format!("S{id}"),
            author: None,
            source: "Asura Scans".into(),
            status: "ongoing".into(),
            chapters: latest as u32,
            read_upto: upto,
            unread: (latest - upto.unwrap_or(0.0)) as u32,
            latest: Some(latest),
            gaps: vec![],
            updated_at: 0,
            note: None,
            bell: false,
            added_at: 0,
            last_read_at,
            shelf: None,
        }
    }

    fn reads(series: i64, n: u32, days_ago: f64) -> Vec<HistoryRead> {
        (0..n).map(|i| HistoryRead { series, number: i as f64, at: NOW - (days_ago * DAY_MS as f64) as i64 - i as i64 * 1000 }).collect()
    }

    #[test]
    fn fifty_chapters_in_two_days_outrank_three_hundred_three_months_ago() {
        // Return of the Frozen Player (50 read these two days) and one read far, then left.
        let lib = vec![series(1, Some(300.0), 320.0, NOW - 90 * DAY_MS), series(2, Some(57.0), 235.0, NOW - DAY_MS)];
        let mut h = reads(1, 300, 90.0);
        h.extend(reads(2, 50, 1.0));
        let on = SmartPrefs { slowed: true, ..SmartPrefs::default() };
        let r = rank(&lib, &h, &on, NOW);
        assert_eq!(r.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!((r[0].kind, r[0].week), (Kind::Now, 50));
        assert_eq!(r[1].kind, Kind::Slowed);
        // Slowed down on: out unless asked for.
        let r = rank(&lib, &h, &SmartPrefs::default(), NOW);
        assert_eq!(r.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn only_started_ones_follow_their_setting_and_never_one_not_started_or_finished() {
        let mut done = series(4, Some(40.0), 60.0, NOW - DAY_MS);
        done.shelf = Some("dropped".into());
        let lib = vec![series(1, Some(3.0), 80.0, NOW - DAY_MS), series(2, None, 80.0, 0), series(3, Some(80.0), 80.0, NOW - DAY_MS), done];
        let h = reads(1, 3, 1.0);
        assert_eq!(rank(&lib, &h, &SmartPrefs::default(), NOW).iter().map(|r| (r.id, r.kind)).collect::<Vec<_>>(), vec![(1, Kind::Started)]);
        assert!(rank(&lib, &h, &SmartPrefs { started: false, ..SmartPrefs::default() }, NOW).is_empty());
    }

    #[test]
    fn read_ones_go_and_the_next_unread_take_their_place_within_the_room() {
        let lib = vec![series(1, Some(12.0), 30.0, NOW - DAY_MS)];
        // Chapters 11 to 15 kept: 11 and 12 read since.
        let autos: Vec<(i64, f64, i64)> = (11..=15).map(|n| (1, n as f64, 100 + n)).collect();
        assert_eq!(read_autos(&lib, &autos), vec![111, 112]);
        let ranked = rank(&lib, &reads(1, 12, 1.0), &SmartPrefs::default(), NOW);
        let have: Vec<(i64, i64, u64)> = (13..=15).map(|n| (1, 100 + n, 8_000_000)).collect();
        let d = SeriesDetail {
            series: series(1, Some(12.0), 30.0, NOW),
            description: None,
            chapters: (1..=30).map(|n| Chapter { id: 100 + n, number: n as f64, name: String::new(), group: None, pages: 40, read: n <= 12, last_page: 0, downloaded: false, uploaded_at: 0, from: None }).collect(),
        };
        let prefs = SmartPrefs { on: true, per_series: 5, room_mb: 1000, ..SmartPrefs::default() };
        let picks = choose(&ranked, &have, &prefs, 8_000_000, |_| Some(d.clone()));
        assert_eq!(picks[0].2.iter().map(|c| c.number).collect::<Vec<_>>(), vec![16.0, 17.0], "the same number kept: five");
        // A room for four chapters in all: one more only.
        let small = SmartPrefs { room_mb: 32, ..prefs };
        assert_eq!(choose(&ranked, &have, &small, 8_000_000, |_| Some(d.clone()))[0].2.len(), 1);
    }

    #[test]
    fn a_smaller_room_lets_the_lowest_ranked_newest_chapters_go_first() {
        let ranked = vec![
            Ranked { id: 1, title: "A".into(), score: 9.0, week: 9, kind: Kind::Now, read_upto: 1.0 },
            Ranked { id: 2, title: "B".into(), score: 1.0, week: 1, kind: Kind::Now, read_upto: 1.0 },
        ];
        let kept = vec![(1, 2.0, 12, 10), (1, 3.0, 13, 10), (2, 2.0, 22, 10), (2, 3.0, 23, 10), (9, 5.0, 95, 10)];
        assert_eq!(over_room(&ranked, &kept, 20), vec![95, 23, 22]);
        assert!(over_room(&ranked, &kept, 50).is_empty());
    }

    #[test]
    fn it_runs_once_a_night_after_midnight() {
        let off = -5 * 3600;
        let on = SmartPrefs { on: true, ..SmartPrefs::default() };
        let tonight = Night { day: day_of(NOW, off), ..Night::default() };
        assert!(!due(&on, &tonight, NOW, off));
        assert!(due(&on, &tonight, NOW + DAY_MS, off));
        assert!(!due(&SmartPrefs::default(), &Night::default(), NOW, off), "off: never");
        assert_eq!(step(&ROOMS_MB, 2000, true), 3000);
        assert_eq!(step(&ROOMS_MB, 500, false), 500);
    }
}
