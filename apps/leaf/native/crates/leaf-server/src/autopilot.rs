//! The autopilot: what makes following a series need nothing more from you.
//!
//! - **Follow** searches every installed source and pins the series to the one you can read the
//!   most chapters of in a row from where you are, ties to your preferred sources.
//! - **Every hour** it refreshes a sixth of the series (those checked longest ago, at most
//!   ceil(series / 6), so each is checked every `CHECK_EVERY` without a burst), keeps the next
//!   `ahead` unread chapters downloaded (Suwayomi converts them to WebP), and compares the other
//!   sources for at most ceil(series / 24) of them, so each is compared once a day: when yours is
//!   down to its last few chapters in a row and another has more, or yours failed three refreshes
//!   in a row, the other takes over, your place carried across ([`plan`]).
//! - **Every day** it updates the installed extensions, which is how a site's change gets fixed,
//!   makes the recommendations (`recs.rs`) and reads Webtoons.com's paid episodes ahead.
//! - Everything that reaches a source goes through its budget (`budget.rs`): the pass waits its
//!   turn and stays away from a source that asked for fewer requests; the phone never waits.
//! - Downloads wait in leaf-server's own line (`dl_queue`) and are handed to Suwayomi a couple
//!   per source at a time ([`Ctx::drain`]), so a big batch fills in steadily.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use leaf_proto::{Candidate, Chapter, Copy, Hit, PaidAhead, Series, SeriesDetail, SeriesDownload, gaps, pick_copies, same_work};
use serde_json::Value;

use crate::budget::{Budget, Lane};
use crate::store::{Store, Work};
use crate::suwa::{Manga, Suwa};

const HOUR: i64 = 3_600_000;
pub const CHECK_EVERY: i64 = 6 * HOUR;
pub const COMPARE_EVERY: i64 = 24 * HOUR;
pub const EXTENSIONS_EVERY: i64 = 24 * HOUR;
/// The most copies of one work a follow reads.
const MAX_FOLLOW_COPIES: usize = 24;
/// Failed refreshes in a row before another source is looked for at once.
pub const FAILS_BEFORE_MOVING: u32 = 3;
/// Downloaded chapters this many behind the last one read are removed from the server: you're
/// unlikely to go back that far, and if you do, opening one fetches it again.
pub const KEEP_BEHIND: f64 = 20.0;
/// A series moves for more chapters only when its own source has fewer than this many left to
/// read in a row: one chapter behind is filled from the linked copies, not worth a move.
pub const NEED_LEFT: u32 = 3;
/// And only when the other source has at least this many more: two sources that keep pace,
/// taking turns to post first, never trade the series back and forth.
pub const CLEAR_GAIN: u32 = 3;
/// How far a chapter number may stand from every other one and still count as the latest: a
/// source's misread "chapter 20241105" or a teaser numbered 1000 is not a new chapter.
const LONE_NUMBER: f64 = 50.0;
/// The start of the error a background request answers while its source is paused: not the
/// source's failure, so it never counts toward moving a series.
pub const PAUSED: &str = "paused: ";
/// Chapters of one source in Suwayomi's download queue at a time.
const DOWNLOADING_PER_SOURCE: usize = 2;
/// Chapter pictures made per hourly pass (more when asked for by the phone, a few a minute).
const THUMBS_PER_PASS: usize = 60;
pub(crate) const WEBTOONS: &str = "Webtoons.com";
/// At most this many Webtoon list pages read for one search's paid warnings.
const PAID_LOOKS: usize = 4;

/// A chapter's page addresses, the source they come from, and whether they are served from
/// Suwayomi's downloads (so asking for one reaches no site).
#[derive(Clone, Debug)]
pub struct PageList {
    pub urls: Vec<String>,
    pub source: String,
    pub local: bool,
}

pub struct Ctx {
    pub suwa: Suwa,
    pub store: Mutex<Store>,
    pub lang: String,
    /// Source names in the order you prefer them ("Asura Scans", "MangaDex").
    pub prefs: Vec<String>,
    /// How many unread chapters to keep downloaded per series.
    pub ahead: usize,
    /// Page addresses per chapter, so a page asks Suwayomi once for the list.
    pub pages: Mutex<HashMap<i64, PageList>>,
    /// leaf-server's data folder (backups go in `backups/`, pictures in `thumbs/`).
    pub data: std::path::PathBuf,
    /// Each source asked politely.
    pub budget: Budget,
    /// Suwayomi's download folder (`mangas/`), when shared: chapter pictures are cut from it.
    pub downloads: Option<PathBuf>,
    /// Chapters with a picture in `thumbs/`.
    pub thumbs: Mutex<HashSet<i64>>,
    /// Chapters whose picture the phone asked for before it was made.
    pub thumbs_wanted: Mutex<Vec<i64>>,
    /// Wakes the download line early (something was queued).
    pub kick: (Mutex<bool>, Condvar),
    /// The sources searched to tell whether a recommendation can be read (`LEAF_REC_SOURCES`).
    pub rec_source_names: Vec<String>,
    /// Titles looked for on them per day at most (`LEAF_REC_SEARCHES`).
    pub rec_searches: usize,
    /// The broad sources a search asks (`LEAF_SEARCH_SOURCES`); the others only when the phone
    /// asks for every source.
    pub search_sources: Vec<String>,
    /// What each source answered for a search lately: (normalised query, source) to (when, what).
    pub search_cache: Mutex<SearchCache>,
    /// Pages near chapters' edges by their pictures, and the ones hidden by hand (`promo.rs`).
    pub promo: Mutex<crate::promo::Seen>,
    /// Titles shown before they are added (`/v1/preview`): the entries asked, to (when, the detail).
    pub preview_cache: Mutex<HashMap<String, (i64, Value)>>,
    /// "Did you mean": a search's words to (when, AniList's title for them, if any).
    pub suggest_cache: Mutex<HashMap<String, (i64, Option<String>)>>,
    /// The collections' pictures, `<slug>.webp` (`LEAF_TAGART`).
    pub tagart: PathBuf,
}

/// Each source's answers to searches: (normalised query, source) to (when, what it found).
pub type SearchCache = HashMap<(String, String), (i64, Vec<crate::suwa::Found>)>;

/// A source's answer to a search is used again for this long.
pub const SEARCH_KEEP_MS: i64 = 10 * 60_000;
/// A preview of a title not added yet is used again for this long.
pub const PREVIEW_KEEP_MS: i64 = 10 * 60_000;
/// The most entries one preview tries, in the order the phone sent them.
const PREVIEW_TRIES: usize = 3;
/// AniList's answer to "did you mean" is used again for this long.
pub const SUGGEST_KEEP_MS: i64 = 24 * HOUR;
/// The most works a search answers with.
pub const SEARCH_MAX: usize = 20;

/// Which sources a search asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Scope {
    /// The broad ones (`LEAF_SEARCH_SOURCES`).
    Broad,
    /// Every installed source ("Search all sources").
    All,
    /// One, by name (the phone asks each broad one apart, to show results as they come).
    One(String),
}

/// The sources a search in `scope` asks, from those installed (`names`): the broad ones that are
/// installed (every one, when none of them is), all, or the one named.
pub fn search_scope(names: &[String], broad: &[String], scope: &Scope) -> Vec<String> {
    let is = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    match scope {
        Scope::All => names.to_vec(),
        Scope::One(n) => names.iter().filter(|s| is(s, n)).cloned().collect(),
        Scope::Broad => {
            let picked: Vec<String> = broad.iter().filter_map(|b| names.iter().find(|s| is(s, b)).cloned()).collect();
            if picked.is_empty() { names.to_vec() } else { picked }
        }
    }
}

/// One source's answer, kept to what names the search ([`leaf_proto::title_has_words`]).
pub fn keep_matching(found: Vec<crate::suwa::Found>, query: &str) -> Vec<crate::suwa::Found> {
    found.into_iter().filter(|f| leaf_proto::title_has_words(&f.title, query)).collect()
}

/// Nightly backups kept.
const BACKUPS_KEPT: usize = 14;

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn log(words: &str) {
    eprintln!("leaf-server: {words}");
}

/// One source's copy of a series, as the autopilot weighs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub manga_id: i64,
    pub source: String,
    pub numbers: Vec<f64>,
}

impl Offer {
    pub fn of(m: &Manga) -> Offer {
        Offer { manga_id: m.id, source: m.source_name.clone(), numbers: m.chapters.iter().map(|c| c.number).collect() }
    }
    fn latest(&self) -> f64 {
        self.numbers.iter().copied().fold(-1.0, f64::max)
    }
}

/// How many chapters can be read in a row after `read_upto` (from chapter 1 when nothing is read).
pub fn run_after(numbers: &[f64], read_upto: Option<f64>) -> u32 {
    let mut whole: Vec<i64> = numbers.iter().filter(|n| n.is_finite() && **n >= 0.0).map(|n| n.floor() as i64).collect();
    whole.sort_unstable();
    whole.dedup();
    let mut next = read_upto.map(|r| r.floor() as i64 + 1).unwrap_or(1);
    let mut run = 0;
    while whole.binary_search(&next).is_ok() {
        run += 1;
        next += 1;
    }
    run
}

fn rank(source: &str, prefs: &[String]) -> usize {
    prefs.iter().position(|p| p.eq_ignore_ascii_case(source)).unwrap_or(prefs.len())
}

/// The offer to read from: the most chapters in a row from where you are, then the preferred
/// source, then the furthest chapter.
pub fn best<'a>(offers: &'a [Offer], read_upto: Option<f64>, prefs: &[String]) -> Option<&'a Offer> {
    offers.iter().filter(|o| !o.numbers.is_empty()).max_by(|a, b| {
        run_after(&a.numbers, read_upto)
            .cmp(&run_after(&b.numbers, read_upto))
            .then(rank(&b.source, prefs).cmp(&rank(&a.source, prefs)))
            .then(a.latest().total_cmp(&b.latest()))
    })
}

#[derive(Debug, PartialEq)]
pub enum Move {
    Stay,
    Switch { to: Offer, why: String },
}

/// Whether to leave the current source. Only for a clear gain and only when it matters, so the
/// translation you are reading is kept as long as it lasts: the current source has fewer than
/// `need` chapters left to read in a row and another has at least [`CLEAR_GAIN`] more, or the
/// current one stopped answering and another has at least as many, and reaches as far.
pub fn decide(cur: &Offer, fails: u32, alts: &[Offer], read_upto: Option<f64>, prefs: &[String], need: u32) -> Move {
    let Some(alt) = best(alts, read_upto, prefs) else { return Move::Stay };
    let (mine, theirs) = (run_after(&cur.numbers, read_upto), run_after(&alt.numbers, read_upto));
    // Both at nothing to read (you are caught up) is no reason to take a source that is behind.
    if fails >= FAILS_BEFORE_MOVING && theirs >= mine && (theirs > 0 || alt.latest() >= cur.latest()) {
        return Move::Switch { to: alt.clone(), why: format!("{} stopped answering", cur.source) };
    }
    if mine < need && theirs >= mine + CLEAR_GAIN {
        let more = theirs - mine;
        let why = if mine == 0 {
            format!("{} has the next {} chapter{}", alt.source, theirs, if theirs == 1 { "" } else { "s" })
        } else {
            format!("{} has {} more chapter{} to read", alt.source, more, if more == 1 { "" } else { "s" })
        };
        return Move::Switch { to: alt.clone(), why };
    }
    Move::Stay
}

/// The latest chapter worth announcing: the highest number that has another within
/// [`LONE_NUMBER`] of it (the highest of all when every number stands alone).
pub fn plausible_latest(numbers: &[f64]) -> Option<f64> {
    let mut n: Vec<f64> = numbers.iter().copied().filter(|n| n.is_finite() && *n >= 0.0).collect();
    n.sort_by(f64::total_cmp);
    n.dedup();
    let near = |i: usize| (i > 0 && n[i] - n[i - 1] <= LONE_NUMBER) || (i + 1 < n.len() && n[i + 1] - n[i] <= LONE_NUMBER);
    (0..n.len()).rev().find(|&i| near(i)).map(|i| n[i]).or(n.last().copied())
}

/// The series as the phone shows it: one copy per chapter number, read when any copy is read.
/// Chapters come from the series' own source; a whole number it lacks is filled from the linked
/// copies on other sources (`extras`), preferred sources first, and says where it came from.
/// A title not added yet as `/v1/series/{id}` answers it: id 0, nothing read, no bell, no
/// pictures of chapters.
pub fn preview_of(m: &Manga, prefs: &[String]) -> Value {
    let w = Work {
        id: 0,
        title: m.title.clone(),
        manga_id: m.id,
        source: m.source_name.clone(),
        followed_at: 0,
        fails: 0,
        checked_at: 0,
        compared_at: 0,
        latest: -1.0,
        updated_at: 0,
        note: None,
        bell: false,
        shelf: None,
    };
    let d = detail_of(&w, m, &[], prefs);
    let mut v = serde_json::to_value(&d).unwrap_or(Value::Null);
    if let Some(list) = v["chapters"].as_array_mut() {
        for c in list {
            c["thumb"] = Value::Bool(false);
        }
    }
    v
}

pub fn detail_of(w: &Work, m: &Manga, extras: &[Manga], prefs: &[String]) -> SeriesDetail {
    use std::collections::HashSet;
    let floor = |n: f64| n.floor() as i64;
    let own: HashSet<i64> = m.chapters.iter().filter(|c| c.number >= 0.0).map(|c| floor(c.number)).collect();
    let mut from: HashMap<i64, &str> = HashMap::new();
    let mut pool: Vec<&crate::suwa::Ch> = m.chapters.iter().collect();
    let mut taken: HashSet<i64> = HashSet::new();
    let mut ordered: Vec<&Manga> = extras.iter().filter(|e| e.id != m.id).collect();
    ordered.sort_by_key(|e| rank(&e.source_name, prefs));
    for e in ordered {
        let copies: Vec<Copy> = e.chapters.iter().map(|c| Copy { id: c.id, number: c.number, group: c.group.clone(), uploaded_at: c.uploaded_at }).collect();
        let mut added: HashSet<i64> = HashSet::new();
        for c in pick_copies(&copies) {
            let f = floor(c.number);
            if own.contains(&f) || (taken.contains(&f) && !added.contains(&f)) {
                continue;
            }
            if let Some(ch) = e.chapters.iter().find(|x| x.id == c.id) {
                pool.push(ch);
                from.insert(ch.id, e.source_name.as_str());
                added.insert(f);
            }
        }
        taken.extend(added);
    }
    let copies: Vec<Copy> = pool.iter().map(|c| Copy { id: c.id, number: c.number, group: c.group.clone(), uploaded_at: c.uploaded_at }).collect();
    let by_id: HashMap<i64, &crate::suwa::Ch> = pool.iter().map(|c| (c.id, *c)).collect();
    let mut read_numbers: Vec<f64> = m.chapters.iter().chain(extras.iter().flat_map(|e| e.chapters.iter())).filter(|c| c.read).map(|c| c.number).collect();
    read_numbers.sort_by(f64::total_cmp);
    let chapters: Vec<Chapter> = pick_copies(&copies)
        .into_iter()
        .map(|c| {
            let ch = by_id[&c.id];
            Chapter {
                id: ch.id,
                number: ch.number,
                name: ch.name.clone(),
                group: ch.group.clone(),
                pages: ch.pages,
                read: read_numbers.binary_search_by(|n| n.total_cmp(&ch.number)).is_ok(),
                last_page: ch.last_page,
                downloaded: ch.downloaded,
                uploaded_at: ch.uploaded_at,
                from: from.get(&ch.id).map(|s| s.to_string()),
            }
        })
        .collect();
    let numbers: Vec<f64> = chapters.iter().map(|c| c.number).collect();
    let read_upto = chapters.iter().filter(|c| c.read).map(|c| c.number).fold(None, |a: Option<f64>, n| Some(a.map_or(n, |a| a.max(n))));
    let series = Series {
        id: w.id,
        // The title you followed, not the current source's: after a move it would change under you.
        title: w.title.clone(),
        author: m.author.clone(),
        source: m.source_name.clone(),
        status: m.status.clone(),
        chapters: chapters.len() as u32,
        read_upto,
        unread: chapters.iter().filter(|c| !c.read).count() as u32,
        latest: numbers.iter().copied().fold(None, |a: Option<f64>, n| Some(a.map_or(n, |a| a.max(n)))),
        gaps: gaps(&numbers),
        updated_at: w.updated_at,
        note: w.note.clone(),
        bell: w.bell,
        shelf: w.shelf.clone(),
        added_at: w.followed_at,
        last_read_at: m.chapters.iter().chain(extras.iter().flat_map(|e| e.chapters.iter())).map(|c| c.last_read_at).max().unwrap_or(0),
    };
    SeriesDetail { series, description: m.description.clone(), chapters }
}

impl Ctx {
    /// The store, locked. Never hold the guard across a call that may lock it again: keep each
    /// use to one statement (`let w = self.store().work(id);`), not an `if let` condition.
    pub(crate) fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A request that reaches `source` (a site, through Suwayomi, or AniList), through its
    /// budget: the foreground goes at once and is counted; the background waits its turn, or
    /// answers [`PAUSED`] while the source asked for fewer requests. A rate limit in the answer
    /// pauses the source's background work (kept across restarts).
    pub(crate) fn ask<T>(&self, source: &str, lane: Lane, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        match lane {
            Lane::Fore => self.budget.fore(source, now_ms()),
            Lane::Back => {
                let wait = self.budget.book(source, now_ms()).map_err(|until| format!("{PAUSED}{source} asked for fewer requests (until {until})"))?;
                if wait > 0 {
                    std::thread::sleep(Duration::from_millis(wait as u64));
                }
            }
        }
        let r = f();
        if let Err(e) = &r
            && let Some(p) = self.budget.answer(source, Some(e), now_ms())
        {
            log(&format!("{source}: background work paused for {} min: {e}", (p.until - p.last_at) / 60_000));
            self.store().set(&format!("pause:{source}"), &p.encode());
        }
        r
    }

    /// Pauses kept from before a restart.
    pub fn restore_pauses(&self) {
        let kept = self.store().with_prefix("pause:");
        for (k, v) in kept {
            if let Some(p) = crate::budget::Pause::decode(&v) {
                self.budget.restore(&k["pause:".len()..], p);
            }
        }
    }

    fn paused(&self, source: &str) -> bool {
        self.budget.paused(source, now_ms()).is_some()
    }

    /// A manga fetched from its source now, through the source's budget.
    fn refresh(&self, manga_id: i64, source: Option<&str>, lane: Lane) -> Result<Manga, String> {
        let source = match source {
            Some(s) => s.to_string(),
            None => self.suwa.source_of(manga_id)?,
        };
        self.ask(&source, lane, || self.suwa.refresh(manga_id))
    }

    /// The work's linked copies on other sources, as Suwayomi last stored them.
    fn extras(&self, w: &Work) -> Vec<Manga> {
        let links = self.store().links(w.id);
        links.into_iter().filter(|id| *id != w.manga_id).filter_map(|id| self.suwa.manga(id).ok()).collect()
    }

    pub(crate) fn detail_now(&self, w: &Work, m: &Manga) -> SeriesDetail {
        detail_of(w, m, &self.extras(w), &self.prefs)
    }

    pub fn library(&self) -> Vec<Series> {
        let works = self.store().works();
        works.iter().filter_map(|w| self.suwa.manga(w.manga_id).ok().map(|m| self.detail_now(w, &m).series)).collect()
    }

    pub fn detail(&self, id: i64) -> Result<SeriesDetail, String> {
        let w = self.store().work(id).ok_or("no such series")?;
        let m = self.suwa.manga(w.manga_id)?;
        Ok(self.detail_now(&w, &m))
    }

    /// Paid episodes ahead on Webtoons.com, as the daily look found them, less what the series
    /// has now (`latest`): a chapter another source brings leaves the range at once.
    pub fn paid(&self, id: i64, latest: Option<f64>) -> Option<PaidAhead> {
        let kept = self.store().get(&format!("paid:{id}"));
        kept.and_then(|v| serde_json::from_str(&v).ok()).and_then(|p| crate::webtoon::still_paid(p, latest))
    }

    /// Whether you asked to hear when a source other than Webtoon has the series: None when one
    /// has it already (nothing to wait for), else whether the watch is on.
    pub fn watch(&self, s: &Series) -> Option<bool> {
        let only_webtoon = s.source.eq_ignore_ascii_case(WEBTOONS) && self.store().link_sources(s.id).iter().all(|(_, src)| src.eq_ignore_ascii_case(WEBTOONS));
        only_webtoon.then(|| self.store().get(&format!("watch:{}", s.id)).is_some())
    }

    /// `POST /v1/series/{id}/watch`: tell me when a source other than Webtoon has it. The daily
    /// comparison looks at every source; it says so once ("elsewhere") and the watch ends.
    pub fn set_watch(&self, id: i64, on: bool) -> Result<(), String> {
        let w = self.store().work(id).ok_or("no such series")?;
        if on {
            self.store().set(&format!("watch:{}", w.id), "1");
        } else {
            self.store().del(&format!("watch:{}", w.id));
        }
        Ok(())
    }

    /// A series as sent: [`Series`] and the fields the phone's struct doesn't have yet (`paid`,
    /// `watch`).
    fn series_json(&self, s: &Series) -> Value {
        let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
        v["paid"] = serde_json::to_value(self.paid(s.id, s.latest)).unwrap_or(Value::Null);
        v["watch"] = serde_json::to_value(self.watch(s)).unwrap_or(Value::Null);
        v
    }

    /// `GET /v1/library`.
    pub fn library_json(&self) -> Value {
        Value::Array(self.library().iter().map(|s| self.series_json(s)).collect())
    }

    /// `GET /v1/series/{id}`: with `paid` on the series and `thumb` on each chapter.
    pub fn detail_json(&self, id: i64) -> Result<Value, String> {
        let d = self.detail(id)?;
        let mut v = serde_json::to_value(&d).map_err(|e| e.to_string())?;
        v["series"] = self.series_json(&d.series);
        let made = self.thumbs.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(list) = v["chapters"].as_array_mut() {
            for (c, json) in d.chapters.iter().zip(list.iter_mut()) {
                json["thumb"] = Value::Bool(made.contains(&c.id));
            }
        }
        Ok(v)
    }

    /// Looks for the work on every other source now (and links what it finds), as the daily
    /// pass would: right after a follow, so the best source and the missing chapters are there
    /// within a minute instead of tomorrow.
    pub fn compare_now(&self, id: i64) {
        let Some(w) = self.store().work(id) else { return };
        let Ok(m) = self.suwa.manga(w.manga_id) else { return };
        let d = self.detail_now(&w, &m);
        self.compare(&w, &m, &d, 0, now_ms());
        // Read into a local first: a guard in an `if let` lives through its block, and
        // `detail_now` takes the store again (a deadlock that froze every request, 2026-11-22).
        let work = self.store().work(id);
        if let Some(w) = work
            && let Ok(m) = self.suwa.manga(w.manga_id)
        {
            self.keep_ahead(&self.detail_now(&w, &m));
        }
    }

    /// One source's answer to a search: the one kept from the last [`SEARCH_KEEP_MS`] for the same
    /// words, else asked now (as the phone's request), kept to the titles that name the search.
    fn search_one(&self, src: &crate::suwa::Source, query: &str, now: i64) -> Result<Vec<crate::suwa::Found>, String> {
        let key = (leaf_proto::norm_title(query), src.name.clone());
        let kept = self.search_cache.lock().unwrap_or_else(|p| p.into_inner()).get(&key).filter(|(at, _)| now - at < SEARCH_KEEP_MS).map(|(_, v)| v.clone());
        if let Some(v) = kept {
            return Ok(v);
        }
        let r = self.ask(&src.name, Lane::Fore, || self.suwa.search(&src.id, query));
        self.note_source(&src.name, r.as_ref().err());
        let found = keep_matching(r?, query);
        let mut cache = self.search_cache.lock().unwrap_or_else(|p| p.into_inner());
        cache.retain(|_, (at, _)| now - *at < SEARCH_KEEP_MS);
        cache.insert(key, (now, found.clone()));
        Ok(found)
    }

    /// The names a search in `scope` asks (`GET /v1/search/sources` lists the broad ones).
    pub fn search_names(&self, scope: &Scope) -> Result<(Vec<String>, u32), String> {
        let installed: Vec<String> = self.suwa.sources(&self.lang)?.into_iter().map(|s| s.name).collect();
        Ok((search_scope(&installed, &self.search_sources, scope), installed.len() as u32))
    }

    /// A search: the sources in `scope` asked at once (each answer kept ten minutes), only titles
    /// that name the search, grouped by work, found on more sources first, at most [`SEARCH_MAX`].
    pub fn search(&self, query: &str, scope: &Scope) -> Result<Vec<Candidate>, String> {
        let installed = self.suwa.sources(&self.lang)?;
        let names = search_scope(&installed.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), &self.search_sources, scope);
        let sources: Vec<&crate::suwa::Source> = installed.iter().filter(|s| names.contains(&s.name)).collect();
        let now = now_ms();
        let found: Vec<(String, Vec<crate::suwa::Found>)> = std::thread::scope(|s| {
            let handles: Vec<_> = sources.iter().map(|src| s.spawn(move || (src.name.clone(), self.search_one(src, query, now)))).collect();
            handles
                .into_iter()
                .filter_map(|h| h.join().ok())
                .filter_map(|(name, r)| match r {
                    Ok(v) => Some((name, v)),
                    Err(e) => {
                        log(&format!("search {name}: {e}"));
                        None
                    }
                })
                .collect()
        });
        let works = self.store().works();
        let mut out: Vec<Candidate> = Vec::new();
        for (source, list) in found {
            for f in list {
                let hit = Hit { manga_id: f.id, source: source.clone() };
                match out.iter_mut().find(|c| same_work(&c.title, &f.title)) {
                    Some(c) if !c.hits.iter().any(|h| h.source == source) => c.hits.push(hit),
                    Some(_) => {}
                    None => out.push(Candidate { title: f.title.clone(), cover_of: Some(f.id), hits: vec![hit], followed: None, paid: None }),
                }
            }
        }
        for c in &mut out {
            c.followed = works.iter().find(|w| c.hits.iter().any(|h| h.manga_id == w.manga_id) || same_work(&w.title, &c.title)).map(|w| w.id);
            c.hits.sort_by_key(|h| rank(&h.source, &self.prefs));
        }
        // Found on more sources first: more likely the work you meant.
        out.sort_by_key(|c| std::cmp::Reverse(c.hits.len()));
        out.truncate(SEARCH_MAX);
        self.warn_paid(&mut out, now);
        Ok(out)
    }

    /// A result found only on Webtoon whose newer episodes are sold there: its paid range
    /// (`Candidate::paid`), from the list page (kept a day; asked as the phone waits, at most
    /// [`PAID_LOOKS`] in one search).
    fn warn_paid(&self, out: &mut [Candidate], now: i64) {
        let only: Vec<(usize, String)> = out
            .iter()
            .enumerate()
            .filter(|(_, c)| c.followed.is_none() && !c.hits.is_empty() && c.hits.iter().all(|h| h.source.eq_ignore_ascii_case(WEBTOONS)))
            .filter_map(|(i, c)| self.webtoon_url(c.hits[0].manga_id).map(|u| (i, u)))
            .take(PAID_LOOKS)
            .collect();
        let found: Vec<(usize, Option<PaidAhead>)> = std::thread::scope(|s| {
            let hs: Vec<_> = only
                .iter()
                .map(|(i, url)| {
                    s.spawn(move || {
                        let l = self.listing(url, Lane::Fore, now).ok().flatten();
                        (*i, l.and_then(|l| crate::webtoon::paid_ahead(&l, Some(l.free), url)))
                    })
                })
                .collect();
            hs.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        for (i, p) in found {
            out[i].paid = p;
        }
    }

    /// `GET /v1/preview?manga_id=a,b`: a title not in My space, in the shape `/v1/series/{id}`
    /// answers (its id 0), so the phone shows its page before anything is added. The entries are
    /// tried in the order sent until one answers with chapters, each asked as the phone waits
    /// (`Lane::Fore`, through its source's budget); the answer is kept ten minutes. Nothing is
    /// added, to My space or to Suwayomi's library. An entry already linked to a work answers that
    /// work's own detail (its id set), and the phone opens that page instead.
    pub fn preview_json(&self, manga_ids: &[i64]) -> Result<Value, String> {
        if manga_ids.is_empty() {
            return Err("preview what?".into());
        }
        let linked = manga_ids.iter().find_map(|id| self.store().work_of_any(*id));
        if let Some(w) = linked {
            return self.detail_json(w.id);
        }
        let key = manga_ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let now = now_ms();
        let kept = self.preview_cache.lock().unwrap_or_else(|p| p.into_inner()).get(&key).filter(|(at, _)| now - at < PREVIEW_KEEP_MS).map(|(_, v)| v.clone());
        if let Some(v) = kept {
            return Ok(v);
        }
        let mut last = Err("none of its entries answered".to_string());
        for &m in manga_ids.iter().take(PREVIEW_TRIES) {
            match self.refresh(m, None, Lane::Fore) {
                Ok(manga) if !manga.chapters.is_empty() => {
                    self.note_source(&manga.source_name, None);
                    let mut v = preview_of(&manga, &self.prefs);
                    // Only on Webtoon: its paid episodes ahead, and the watch for another source.
                    if manga_ids.len() == 1 && manga.source_name.eq_ignore_ascii_case(WEBTOONS) {
                        let numbers: Vec<f64> = manga.chapters.iter().map(|c| c.number).collect();
                        let paid = self.webtoon_url(m).and_then(|url| {
                            let l = self.listing(&url, Lane::Fore, now).ok().flatten()?;
                            crate::webtoon::paid_ahead(&l, plausible_latest(&numbers), &url)
                        });
                        v["series"]["paid"] = serde_json::to_value(paid).unwrap_or(Value::Null);
                        v["series"]["watch"] = Value::Bool(false);
                    }
                    let mut cache = self.preview_cache.lock().unwrap_or_else(|p| p.into_inner());
                    cache.retain(|_, (at, _)| now - *at < PREVIEW_KEEP_MS);
                    cache.insert(key, (now, v.clone()));
                    return Ok(v);
                }
                Ok(_) => last = Err("no chapters there yet".into()),
                Err(e) => {
                    log(&format!("preview {m}: {e}"));
                    last = Err(e);
                }
            }
        }
        last
    }

    /// `GET /v1/search/suggest?q=`: a title AniList knows that the words were probably meant to
    /// spell ("aweking contract": The Awakening Contract), for a search that found nothing. AniList's
    /// own search takes no typos, so each of the two longest words is searched alone and the titles
    /// (and their other names) are scored against all the words ([`crate::anilist::closest`]). At
    /// most two requests, as the phone waits, through AniList's budget; kept a day.
    pub fn suggest(&self, query: &str) -> Result<Option<String>, String> {
        let key = leaf_proto::norm_title(query);
        if key.chars().count() < 3 {
            return Ok(None);
        }
        let now = now_ms();
        let kept = self.suggest_cache.lock().unwrap_or_else(|p| p.into_inner()).get(&key).filter(|(at, _)| now - at < SUGGEST_KEEP_MS).map(|(_, v)| v.clone());
        if let Some(v) = kept {
            return Ok(v);
        }
        let mut words: Vec<&str> = key.split(' ').filter(|w| w.chars().count() >= 4).collect();
        words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
        words.dedup();
        let mut found = None;
        for w in words.into_iter().take(2) {
            let names = self.ask("AniList", Lane::Fore, || crate::anilist::names_for(w))?;
            if let Some(t) = crate::anilist::closest(&names, query) {
                found = Some(t);
                break;
            }
        }
        let mut cache = self.suggest_cache.lock().unwrap_or_else(|p| p.into_inner());
        cache.retain(|_, (at, _)| now - *at < SUGGEST_KEEP_MS);
        cache.insert(key, (now, found.clone()));
        Ok(found)
    }

    /// Follows a work found by search: each source's copy is read, the best one pinned.
    pub fn follow(&self, title: &str, manga_ids: &[i64]) -> Result<Series, String> {
        // A copy already linked to a work is that work, not a second one.
        if let Some(w) = manga_ids.iter().find_map(|id| self.store().work_of_any(*id)) {
            return self.detail(w.id).map(|d| d.series);
        }
        // One source asked per thread: as many as there are sources, not as many as were sent.
        let manga_ids = &manga_ids[..manga_ids.len().min(MAX_FOLLOW_COPIES)];
        let mangas: Vec<Manga> = std::thread::scope(|s| {
            let hs: Vec<_> = manga_ids.iter().map(|id| s.spawn(move || self.refresh(*id, None, Lane::Fore))).collect();
            hs.into_iter()
                .filter_map(|h| h.join().ok())
                .filter_map(|r| r.map_err(|e| log(&format!("follow {title}: a source left out: {e}"))).ok())
                .collect()
        });
        let offers: Vec<Offer> = mangas.iter().map(Offer::of).collect();
        let pick = best(&offers, None, &self.prefs).ok_or("no source has chapters of it")?;
        let m = mangas.iter().find(|m| m.id == pick.manga_id).expect("the offer's manga");
        self.suwa.set_in_library(m.id, true)?;
        let now = now_ms();
        let latest = plausible_latest(&pick.numbers).unwrap_or(-1.0);
        let id = self.store().add_work(if m.title.is_empty() { title } else { &m.title }, m.id, &m.source_name, latest, now)?;
        let others: Vec<String> = offers.iter().filter(|o| o.manga_id != m.id).map(|o| format!("{} ({})", o.source, run_after(&o.numbers, None))).collect();
        let words = if others.is_empty() {
            format!("Following {} on {}", m.title, m.source_name)
        } else {
            format!("Following {} on {}: {} chapters in a row, others: {}", m.title, m.source_name, run_after(&pick.numbers, None), others.join(", "))
        };
        self.store().event(now, Some(id), "info", &words);
        let w = self.store().work(id).ok_or("the series just added")?;
        let d = self.detail_now(&w, m);
        self.keep_ahead(&d);
        Ok(d.series)
    }

    /// Every title the work goes by: its own, then the ones AniList knows (fetched once a week at
    /// most, and only while none are known).
    fn names(&self, w: &Work, now: i64, lane: Lane) -> Vec<String> {
        let mut names = self.store().aliases(w.id);
        let key = format!("aliases_at:{}", w.id);
        let tried = self.store().get(&key).and_then(|v| v.parse::<i64>().ok()).is_some_and(|at| now - at < 7 * COMPARE_EVERY);
        if names.is_empty() && !tried {
            match self.ask("AniList", lane, || crate::anilist::titles(&w.title)) {
                Ok(found) => {
                    self.store().add_aliases(w.id, &found);
                    names = found;
                }
                Err(e) if e.starts_with(PAUSED) => return vec![w.title.clone()],
                Err(e) => log(&format!("titles of {}: {e}", w.title)),
            }
            self.store().set(&key, &now.to_string());
        }
        names.insert(0, w.title.clone());
        names
    }

    /// Marks every chapter numbered below `before` read, on the series' own source and its linked
    /// copies; the chapter you're on is then the one downloaded ahead.
    pub fn read_before(&self, id: i64, before: f64) -> Result<u32, String> {
        let w = self.store().work(id).ok_or("no such series")?;
        let m = self.suwa.manga(w.manga_id)?;
        let extras = self.extras(&w);
        let ids: Vec<i64> = m.chapters.iter().chain(extras.iter().flat_map(|e| e.chapters.iter())).filter(|c| c.number >= 0.0 && c.number < before && !c.read).map(|c| c.id).collect();
        for part in ids.chunks(200) {
            self.suwa.mark(part, Some(true), None)?;
        }
        let m = self.suwa.manga(w.manga_id)?;
        self.keep_ahead(&self.detail_now(&w, &m));
        Ok(ids.len() as u32)
    }

    /// Due for a comparison at the next hourly pass.
    pub fn compare_soon(&self, id: i64) {
        self.store().compared(id, 0);
    }

    pub fn set_bell(&self, id: i64, on: bool) -> Result<(), String> {
        let w = self.store().work(id).ok_or("no such series")?;
        self.store().set_bell(w.id, on);
        Ok(())
    }

    /// A chapter downloaded because you tapped it (kept until you remove it): handed to Suwayomi
    /// at once, counted as the reader's.
    pub fn download(&self, chapter_id: i64, on: bool) -> Result<(), String> {
        if !on {
            self.store().unqueue_downloads(&[chapter_id]);
            return self.suwa.delete_downloads(&[chapter_id]);
        }
        let info = self.suwa.chapter_info(chapter_id)?;
        self.store().unqueue_downloads(&[chapter_id]);
        self.ask(&info.source, Lane::Fore, || self.suwa.enqueue(&[chapter_id]))
    }

    /// Chapters to download, put in line: handed to Suwayomi as each source's budget allows.
    pub fn queue_downloads(&self, chapters: &[(i64, String)]) {
        if chapters.is_empty() {
            return;
        }
        self.store().queue_downloads(chapters, now_ms());
        self.wake();
    }

    /// `POST` / `DELETE /v1/downloads`: many chapters at once (500 at most).
    pub fn download_many(&self, ids: &[i64], on: bool) -> Result<u32, String> {
        let ids = &ids[..ids.len().min(500)];
        if !on {
            self.store().unqueue_downloads(ids);
            self.suwa.delete_downloads(ids)?;
            return Ok(ids.len() as u32);
        }
        let mut want = Vec::new();
        for id in ids {
            let info = self.suwa.chapter_info(*id)?;
            if !info.downloaded {
                want.push((*id, info.source));
            }
        }
        self.queue_downloads(&want);
        Ok(want.len() as u32)
    }

    /// `POST /v1/series/{id}/download`: the chapters numbered `from` to `to`, or every unread one,
    /// each from the copy the phone reads it from.
    pub fn download_series(&self, id: i64, r: SeriesDownload) -> Result<u32, String> {
        let d = self.detail(id)?;
        let (from, to) = (r.from.unwrap_or(f64::MIN), r.to.unwrap_or(f64::MAX));
        if !r.unread && r.from.is_none() && r.to.is_none() {
            return Err("which chapters? from and to, or unread".into());
        }
        let want: Vec<(i64, String)> = d
            .chapters
            .iter()
            .filter(|c| !c.downloaded && if r.unread { !c.read } else { c.number >= from && c.number <= to })
            .map(|c| (c.id, c.from.clone().unwrap_or_else(|| d.series.source.clone())))
            .collect();
        self.queue_downloads(&want);
        Ok(want.len() as u32)
    }

    pub(crate) fn wake(&self) {
        let (m, cv) = &self.kick;
        *m.lock().unwrap_or_else(|p| p.into_inner()) = true;
        cv.notify_all();
    }

    /// Waits until something is queued or `most` has passed.
    pub fn wait_for_work(&self, most: Duration) {
        let (m, cv) = &self.kick;
        let guard = m.lock().unwrap_or_else(|p| p.into_inner());
        let (mut woke, _) = cv.wait_timeout_while(guard, most, |k| !*k).unwrap_or_else(|p| p.into_inner());
        *woke = false;
    }

    /// Hands queued downloads to Suwayomi: per source, enough to keep
    /// [`DOWNLOADING_PER_SOURCE`] chapters downloading, each through the source's budget. A
    /// paused source's chapters wait.
    pub fn drain(&self) {
        let queued = self.store().queued_downloads();
        if queued.is_empty() {
            return;
        }
        let busy = match self.suwa.download_queue() {
            Ok(q) => q,
            Err(e) => return log(&format!("download queue: {e}")),
        };
        let mut by_source: Vec<(String, Vec<i64>)> = Vec::new();
        for (id, source) in queued {
            if busy.iter().any(|(b, _)| *b == id) {
                self.store().unqueue_downloads(&[id]);
                continue;
            }
            match by_source.iter_mut().find(|(s, _)| *s == source) {
                Some((_, v)) => v.push(id),
                None => by_source.push((source, vec![id])),
            }
        }
        for (source, ids) in by_source {
            if self.paused(&source) {
                continue;
            }
            let room = DOWNLOADING_PER_SOURCE.saturating_sub(busy.iter().filter(|(_, s)| *s == source).count());
            for id in ids.into_iter().take(room) {
                match self.ask(&source, Lane::Back, || self.suwa.enqueue(&[id])) {
                    Ok(()) => self.store().unqueue_downloads(&[id]),
                    Err(e) if e.starts_with(PAUSED) => break,
                    Err(e) => {
                        // A chapter gone from Suwayomi would otherwise wait for ever.
                        log(&format!("download {id}: {e}"));
                        self.store().unqueue_downloads(&[id]);
                    }
                }
            }
        }
    }

    pub fn unfollow(&self, id: i64) -> Result<(), String> {
        let w = self.store().work(id).ok_or("no such series")?;
        self.suwa.set_in_library(w.manga_id, false)?;
        self.store().remove_work(id);
        Ok(())
    }

    /// Reading progress from the phone. A chapter read to its end marks every copy of its number.
    pub fn progress(&self, chapter_id: i64, page: u32, done: bool) -> Result<(), String> {
        if !done {
            return self.suwa.mark(&[chapter_id], None, Some(page));
        }
        let (manga_id, number) = self.suwa.chapter_place(chapter_id)?;
        let m = self.suwa.manga(manga_id)?;
        let ids: Vec<i64> = m.chapters.iter().filter(|c| c.number == number || c.id == chapter_id).map(|c| c.id).collect();
        self.suwa.mark(&ids, Some(true), Some(page))?;
        // The chapter may be a linked copy's: the work is found either way.
        let work = self.store().work_of_any(manga_id);
        if let Some(w) = work
            && let Ok(own) = self.suwa.manga(w.manga_id)
        {
            self.keep_ahead(&self.detail_now(&w, &own));
        }
        Ok(())
    }

    /// The same chapter number from the work's other copies, preferred sources first: a source
    /// that refuses (rate limits, a Cloudflare check, a site down) doesn't stop the reading.
    fn pages_elsewhere(&self, chapter_id: i64) -> Option<PageList> {
        let (manga_id, number) = self.suwa.chapter_place(chapter_id).ok()?;
        let work = self.store().work_of_any(manga_id)?;
        let mut copies: Vec<Manga> = std::iter::once(work.manga_id).chain(self.store().links(work.id)).filter(|id| *id != manga_id).filter_map(|id| self.suwa.manga(id).ok()).collect();
        copies.sort_by_key(|m| rank(&m.source_name, &self.prefs));
        for m in copies {
            for c in m.chapters.iter().filter(|c| c.number == number) {
                if let Ok(p) = self.ask(&m.source_name, Lane::Fore, || self.suwa.pages(c.id))
                    && !p.is_empty()
                {
                    log(&format!("chapter {chapter_id}: read from {} instead", m.source_name));
                    return Some(PageList { urls: p, source: m.source_name.clone(), local: c.downloaded });
                }
            }
        }
        None
    }

    /// Forgets a chapter's page list, so the next request asks again (and may fall back).
    pub fn forget_pages(&self, chapter_id: i64) {
        self.pages.lock().unwrap_or_else(|p| p.into_inner()).remove(&chapter_id);
    }

    /// The chapter's pages: from Suwayomi's downloads when it has them (no site asked), else from
    /// its source as the reader's request, else from the work's other copies.
    pub fn pages(&self, chapter_id: i64) -> Result<PageList, String> {
        let cached = self.pages.lock().unwrap_or_else(|p| p.into_inner()).get(&chapter_id).cloned();
        if let Some(p) = cached {
            return Ok(p);
        }
        let info = self.suwa.chapter_info(chapter_id)?;
        let asked = if info.downloaded { self.suwa.pages(chapter_id) } else { self.ask(&info.source, Lane::Fore, || self.suwa.pages(chapter_id)) };
        let p = match asked {
            Ok(urls) => PageList { urls, source: info.source, local: info.downloaded },
            Err(e) => {
                log(&format!("chapter {chapter_id}: {e}; trying its other sources"));
                self.pages_elsewhere(chapter_id).ok_or(e)?
            }
        };
        let mut cache = self.pages.lock().unwrap_or_else(|p| p.into_inner());
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(chapter_id, p.clone());
        Ok(p)
    }

    /// One page's picture; counted against its source unless served from the downloads.
    pub fn page(&self, chapter_id: i64, n: usize) -> Result<(Vec<u8>, String), String> {
        let list = self.pages(chapter_id)?;
        let url = list.urls.get(n).ok_or("no such page")?;
        if list.local { self.suwa.bytes(url) } else { self.ask(&list.source, Lane::Fore, || self.suwa.bytes(url)) }
    }

    /// The chapter after the last one you read (`ahead` of them, 1 by default), downloaded before
    /// you get there. Nothing for a series you have not started: downloads are otherwise yours to
    /// ask for.
    fn keep_ahead(&self, d: &SeriesDetail) {
        let Some(upto) = d.series.read_upto else { return };
        let want: Vec<(i64, String)> = d
            .chapters
            .iter()
            .filter(|c| c.number > upto && !c.read)
            .take(self.ahead)
            .filter(|c| !c.downloaded)
            .map(|c| (c.id, c.from.clone().unwrap_or_else(|| d.series.source.clone())))
            .collect();
        self.queue_downloads(&want);
    }

    /// Removes downloads of chapters `KEEP_BEHIND` or more behind the last one read, on the
    /// series' own source and its linked copies.
    fn trim_behind(&self, w: &Work, m: &Manga, d: &SeriesDetail) {
        let Some(upto) = d.series.read_upto else { return };
        let extras = self.extras(w);
        let old: Vec<i64> = m
            .chapters
            .iter()
            .chain(extras.iter().flat_map(|e| e.chapters.iter()))
            .filter(|c| c.downloaded && c.number >= 0.0 && c.number <= upto - KEEP_BEHIND)
            .map(|c| c.id)
            .collect();
        if old.is_empty() {
            return;
        }
        match self.suwa.delete_downloads(&old) {
            Ok(()) => log(&format!("{}: removed {} chapters read long ago", w.title, old.len())),
            Err(e) => log(&format!("{}: removing old chapters: {e}", w.title)),
        }
    }

    /// A source answered (`err` None) or failed: kept for the health list.
    fn note_source(&self, name: &str, err: Option<&String>) {
        let now = now_ms();
        match err {
            None => self.store().source_ok(name, now),
            Some(e) => self.store().source_err(name, e, now),
        }
    }

    /// `GET /v1/health`: each source's last answer, and a pause in words while it lasts (the
    /// phone shows `last_error`), with `paused_until` and today's `requests_today` beside it.
    pub fn health(&self) -> Vec<Value> {
        let names: Vec<String> = self.suwa.sources(&self.lang).map(|v| v.into_iter().map(|s| s.name).collect()).unwrap_or_default();
        let rows = self.store().health();
        let now = now_ms();
        names
            .into_iter()
            .map(|name| {
                let (ok_at, err_at, err) = rows.iter().find(|r| r.0 == name).map(|r| (r.1, r.2, r.3.clone())).unwrap_or((0, 0, None));
                let failing = err_at > ok_at;
                let pause = self.budget.paused(&name, now);
                let last_error = match &pause {
                    Some(p) => Some(format!("Asked for fewer requests: background work paused for {} min ({})", ((p.until - now) / 60_000).max(1), p.why)),
                    None if failing => err.clone(),
                    None => None,
                };
                let h = leaf_proto::SourceHealth {
                    blocked: failing && err.as_deref().is_some_and(|e| e.to_lowercase().contains("cloudflare")),
                    ok: !failing && pause.is_none(),
                    last_ok_at: ok_at,
                    last_error,
                    name: name.clone(),
                };
                let mut v = serde_json::to_value(&h).unwrap_or(Value::Null);
                let c = self.budget.today(&name, now);
                v["paused_until"] = Value::from(pause.map_or(0, |p| p.until));
                v["requests_today"] = serde_json::json!({ "fore": c.fore, "back": c.back });
                v
            })
            .collect()
    }

    /// Chapters that arrived in the last `days` for series in your space, newest first.
    pub fn updates(&self, days: i64) -> Vec<leaf_proto::Update> {
        let since = now_ms() - days.clamp(1, 60) * 86_400_000;
        let arrivals = self.store().arrivals_since(since);
        let mut by_work: HashMap<i64, Vec<(f64, i64)>> = HashMap::new();
        for (w, n, at) in arrivals {
            by_work.entry(w).or_default().push((n, at));
        }
        let mut out = Vec::new();
        for (wid, list) in by_work {
            let work = self.store().work(wid);
            let Some(w) = work else { continue };
            let Ok(m) = self.suwa.manga(w.manga_id) else { continue };
            let d = self.detail_now(&w, &m);
            for (n, at) in list {
                if let Some(c) = d.chapters.iter().find(|c| c.number == n) {
                    out.push(leaf_proto::Update { series: w.id, title: w.title.clone(), chapter: c.id, number: n, name: c.name.clone(), at, read: c.read, bell: w.bell });
                }
            }
        }
        out.sort_by(|a, b| b.at.cmp(&a.at).then(b.number.total_cmp(&a.number)));
        out
    }

    /// A chapter marked read or unread by hand: every copy of its number, on every source.
    pub fn mark_chapter(&self, chapter_id: i64, read: bool) -> Result<(), String> {
        let (manga_id, number) = self.suwa.chapter_place(chapter_id)?;
        let work = self.store().work_of_any(manga_id);
        let mangas: Vec<Manga> = match &work {
            Some(w) => std::iter::once(w.manga_id).chain(self.store().links(w.id)).filter_map(|id| self.suwa.manga(id).ok()).collect(),
            None => self.suwa.manga(manga_id).into_iter().collect(),
        };
        let ids: Vec<i64> = mangas.iter().flat_map(|m| m.chapters.iter()).filter(|c| c.number == number || c.id == chapter_id).map(|c| c.id).collect();
        self.suwa.mark(&ids, Some(read), if read { None } else { Some(0) })
    }

    pub fn set_shelf(&self, id: i64, shelf: Option<&str>) -> Result<(), String> {
        const SHELVES: [&str; 5] = ["reading", "plan", "hold", "dropped", "done"];
        if let Some(s) = shelf
            && !SHELVES.contains(&s)
        {
            return Err(format!("no shelf called {s}"));
        }
        self.store().work(id).ok_or("no such series")?;
        self.store().set_shelf(id, shelf);
        Ok(())
    }

    /// Other entries for the work across sources, for "Wrong series?": its titles searched on
    /// every source, each hit read for its chapters (slow: sources are asked).
    pub fn alternatives(&self, id: i64) -> Result<Vec<leaf_proto::Alternative>, String> {
        let w = self.store().work(id).ok_or("no such series")?;
        let names = self.names(&w, now_ms(), Lane::Fore);
        let names = &names;
        let sources = self.suwa.sources(&self.lang)?;
        let mut found: Vec<Manga> = std::thread::scope(|s| {
            let hs: Vec<_> = sources
                .iter()
                .map(|src| {
                    s.spawn(move || {
                        let mut hits: Vec<Manga> = Vec::new();
                        for query in names.iter().take(2) {
                            let Ok(list) = self.ask(&src.name, Lane::Fore, || self.suwa.search(&src.id, query)) else { continue };
                            for f in list.into_iter().filter(|f| names.iter().any(|n| same_work(&f.title, n))).take(3) {
                                if !hits.iter().any(|h| h.id == f.id)
                                    && let Ok(m) = self.refresh(f.id, Some(&src.name), Lane::Fore)
                                {
                                    hits.push(m);
                                }
                            }
                        }
                        hits
                    })
                })
                .collect();
            hs.into_iter().filter_map(|h| h.join().ok()).flatten().collect()
        });
        if !found.iter().any(|m| m.id == w.manga_id)
            && let Ok(m) = self.suwa.manga(w.manga_id)
        {
            found.insert(0, m);
        }
        let mut out: Vec<leaf_proto::Alternative> = found
            .iter()
            .map(|m| {
                let numbers: Vec<f64> = m.chapters.iter().map(|c| c.number).filter(|n| *n >= 0.0).collect();
                let mut whole: Vec<i64> = numbers.iter().map(|n| n.floor() as i64).collect();
                whole.sort_unstable();
                whole.dedup();
                leaf_proto::Alternative {
                    manga_id: m.id,
                    title: m.title.clone(),
                    source: m.source_name.clone(),
                    chapters: whole.len() as u32,
                    latest: plausible_latest(&numbers),
                    current: m.id == w.manga_id,
                }
            })
            .collect();
        out.sort_by(|a, b| b.current.cmp(&a.current).then(b.chapters.cmp(&a.chapters)));
        Ok(out)
    }

    /// Reads the series from another entry, chosen by hand; your place comes across.
    pub fn repin(&self, id: i64, manga_id: i64) -> Result<leaf_proto::Series, String> {
        let w = self.store().work(id).ok_or("no such series")?;
        if w.manga_id == manga_id {
            return self.detail(id).map(|d| d.series);
        }
        let m = self.suwa.manga(w.manga_id)?;
        let d = self.detail_now(&w, &m);
        let new = self.refresh(manga_id, None, Lane::Fore)?;
        let now = now_ms();
        self.switch(&w, &d, &new, now)?;
        self.store().event(now, Some(w.id), "switched", &format!("Moved {} to {} by hand", w.title, new.source_name));
        self.detail(id).map(|d| d.series)
    }

    /// Everything needed to rebuild your library: each series, where it is read from, what is
    /// read, its bell and shelf, its other titles and copies.
    fn backup_json(&self) -> serde_json::Value {
        let works = self.store().works();
        let mut list = Vec::new();
        for w in &works {
            let read: Vec<f64> = self.suwa.manga(w.manga_id).ok().map(|m| self.detail_now(w, &m).chapters.iter().filter(|c| c.read).map(|c| c.number).collect()).unwrap_or_default();
            let aliases = self.store().aliases(w.id);
            let links = self.store().links(w.id);
            list.push(serde_json::json!({
                "title": w.title, "source": w.source, "manga_id": w.manga_id, "bell": w.bell, "shelf": w.shelf,
                "added_at": w.followed_at, "read": read, "aliases": aliases, "links": links,
            }));
        }
        serde_json::json!({ "leaf_backup": 1, "made_at": now_ms(), "series": list })
    }

    /// Writes a backup now into `backups/` (14 kept) and answers its path.
    pub fn backup_now(&self) -> Result<std::path::PathBuf, String> {
        let dir = self.data.join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("leaf-backup-{}.json", now_ms() / 1000));
        let body = serde_json::to_vec_pretty(&self.backup_json()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| e.to_string())?;
        let mut all: Vec<_> = std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        all.sort();
        while all.len() > BACKUPS_KEPT {
            let _ = std::fs::remove_file(all.remove(0));
        }
        Ok(path)
    }

    pub fn latest_backup(&self) -> Option<std::path::PathBuf> {
        let mut all: Vec<_> = std::fs::read_dir(self.data.join("backups")).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        all.sort();
        all.pop()
    }

    pub fn set_anilist(&self, token: Option<&str>) {
        match token.map(str::trim).filter(|t| !t.is_empty()) {
            Some(t) => self.store().set("anilist_token", t),
            None => self.store().set("anilist_token", ""),
        }
    }

    pub fn anilist_on(&self) -> bool {
        self.store().get("anilist_token").is_some_and(|t| !t.is_empty())
    }

    /// Sends what changed to AniList: each series' last chapter read, Completed when a finished
    /// series is read to its end. Paced to stay well under AniList's limit.
    fn sync_anilist(&self) {
        let token = self.store().get("anilist_token").filter(|t| !t.is_empty());
        let Some(token) = token else { return };
        let works = self.store().works();
        for w in works {
            let Ok(m) = self.suwa.manga(w.manga_id) else { continue };
            let d = self.detail_now(&w, &m);
            let Some(upto) = d.series.read_upto else { continue };
            let progress = upto.floor().max(0.0) as u32;
            let done = d.series.status == "completed" && d.series.unread == 0;
            let sent_key = format!("al_sent:{}", w.id);
            let want = format!("{progress}:{done}");
            let sent = self.store().get(&sent_key);
            if sent.as_deref() == Some(want.as_str()) {
                continue;
            }
            let Some(media) = self.anilist_id(w.id, &w.title, Lane::Back) else { continue };
            match self.ask("AniList", Lane::Back, || crate::anilist::save_progress(&token, media, progress, done)) {
                Ok(()) => self.store().set(&sent_key, &want),
                Err(e) => {
                    log(&format!("anilist {}: {e}", w.title));
                    if e.contains("Invalid token") || e.contains("Unauthorized") || e.starts_with(PAUSED) {
                        return;
                    }
                }
            }
        }
    }

    /// One pass over everything due. Slow (sources are asked); runs on its own thread.
    pub fn tick(&self) {
        let now = now_ms();
        let due_ext = self.store().get("extensions_at").and_then(|v| v.parse::<i64>().ok()).is_none_or(|at| now - at >= EXTENSIONS_EVERY);
        if due_ext {
            match self.suwa.update_extensions() {
                Ok(names) if !names.is_empty() => self.store().event(now, None, "info", &format!("Updated {}", names.join(", "))),
                Ok(_) => {}
                Err(e) => log(&format!("extensions: {e}")),
            }
            self.store().set("extensions_at", &now.to_string());
        }
        let works = self.store().works();
        let due: Vec<Due> = works.iter().map(|w| Due { id: w.id, checked_at: w.checked_at, compared_at: w.compared_at, fails: w.fails, paused: self.paused(&w.source) }).collect();
        let p = plan(&due, now);
        for w in works.iter().filter(|w| p.check.contains(&w.id)) {
            self.check(w, now_ms(), p.compare.contains(&w.id));
        }
        // The comparisons due for series not checked this pass, from what Suwayomi last stored.
        for id in p.compare.iter().filter(|id| !p.check.contains(id)) {
            let work = self.store().work(*id);
            let Some(w) = work else { continue };
            if now - w.compared_at < COMPARE_EVERY && w.fails < FAILS_BEFORE_MOVING {
                continue;
            }
            let Ok(m) = self.suwa.manga(w.manga_id) else { continue };
            let d = self.detail_now(&w, &m);
            self.compare(&w, &m, &d, w.fails, now_ms());
        }
        let backed = self.store().get("backup_at").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        if now - backed >= 86_400_000 {
            match self.backup_now() {
                Ok(p) => log(&format!("backup: {}", p.display())),
                Err(e) => log(&format!("backup: {e}")),
            }
            self.store().set("backup_at", &now.to_string());
        }
        self.sync_anilist();
        self.paid_pass(now_ms());
        let made = self.thumbs_pass(THUMBS_PER_PASS);
        if made > 0 {
            log(&format!("chapter pictures made: {made}"));
        }
        let recs_at = self.store().get("recs_at").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        if now - recs_at >= COMPARE_EVERY {
            match self.make_recs() {
                Ok(r) => log(&format!("recommendations: {} top, {} because, {} collections", r.top.len(), r.because.len(), r.collections.len())),
                Err(e) => log(&format!("recommendations: {e}")),
            }
            self.store().set("recs_at", &now.to_string());
        }
    }

    fn check(&self, w: &Work, now: i64, may_compare: bool) {
        let fresh = self.refresh(w.manga_id, Some(&w.source), Lane::Back);
        let (ok, m) = match fresh {
            Ok(m) => (true, m),
            // Paused for asking too much: not the source's failure, nothing counted.
            Err(e) if e.starts_with(PAUSED) => return,
            Err(e) => {
                log(&format!("refresh {}: {e}", w.title));
                match self.suwa.manga(w.manga_id) {
                    Ok(m) => (false, m),
                    Err(_) => {
                        self.store().checked(w.id, false, None, now);
                        return;
                    }
                }
            }
        };
        let failed = "no answer".to_string();
        self.note_source(&w.source, if ok { None } else { Some(&failed) });
        let d = self.detail_now(w, &m);
        let numbers: Vec<f64> = d.chapters.iter().map(|c| c.number).collect();
        let latest = plausible_latest(&numbers);
        if ok && let Some(l) = latest.filter(|l| *l > w.latest && w.latest >= 0.0) {
            let arrived: Vec<f64> = numbers.iter().copied().filter(|n| *n > w.latest && *n <= l).collect();
            self.store().add_arrivals(w.id, &arrived, now);
        }
        // Announced only for a series with its bell on.
        if ok && w.bell && let Some(l) = latest.filter(|l| *l > w.latest && w.latest >= 0.0) {
            let n = d.chapters.iter().filter(|c| c.number > w.latest && c.number <= l).count();
            self.store().event(now, Some(w.id), "new", &format!("{n} new chapter{} of {}, up to {}", if n == 1 { "" } else { "s" }, w.title, fmt_num(l)));
        }
        self.store().checked(w.id, ok, latest, now);
        let fails = if ok { 0 } else { w.fails + 1 };
        if may_compare && (fails >= FAILS_BEFORE_MOVING || now - w.compared_at >= COMPARE_EVERY) && self.compare(w, &m, &d, fails, now) {
            return;
        }
        if fails >= FAILS_BEFORE_MOVING && w.note.is_none() {
            let note = format!("{} hasn't answered since {} tries. Nothing else carries it yet", w.source, fails);
            self.store().set_note(w.id, Some(&note));
            self.store().event(now, Some(w.id), "blocked", &format!("{}: {note}", w.title));
        } else if ok && w.note.is_some() {
            self.store().set_note(w.id, None);
        }
        if ok {
            self.keep_ahead(&d);
        }
        self.trim_behind(w, &m, &d);
    }

    /// Looks at the other sources; moves the series when one is clearly better. True if moved.
    fn compare(&self, w: &Work, m: &Manga, d: &SeriesDetail, fails: u32, now: i64) -> bool {
        self.store().compared(w.id, now);
        let Ok(sources) = self.suwa.sources(&self.lang) else { return false };
        let names = self.names(w, now, Lane::Back);
        let names = &names;
        let alts: Vec<Manga> = std::thread::scope(|s| {
            let hs: Vec<_> = sources
                .iter()
                .filter(|src| src.id != m.source_id && !self.paused(&src.name))
                .map(|src| {
                    s.spawn(move || {
                        // Each title in turn (three at most), until one finds the work.
                        for query in names.iter().take(3) {
                            let found = match self.ask(&src.name, Lane::Back, || self.suwa.search(&src.id, query)) {
                                Ok(f) => f,
                                Err(e) if e.starts_with(PAUSED) => return None,
                                Err(_) => continue,
                            };
                            if let Some(hit) = found.into_iter().find(|f| names.iter().any(|n| same_work(&f.title, n))) {
                                return self.refresh(hit.id, Some(&src.name), Lane::Back).ok();
                            }
                        }
                        None
                    })
                })
                .collect();
            hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        });
        let linked = self.store().links(w.id);
        for a in &alts {
            self.store().add_link(w.id, a.id, &a.source_name);
        }
        self.found_elsewhere(w, &alts, now);
        // Chapters a newly linked copy fills in were there all along: not news for the bell.
        if alts.iter().any(|a| !linked.contains(&a.id)) {
            let filled = self.detail_now(w, m);
            let numbers: Vec<f64> = filled.chapters.iter().map(|c| c.number).collect();
            if let Some(l) = plausible_latest(&numbers) {
                self.store().raise_latest(w.id, l);
            }
        }
        let offers: Vec<Offer> = alts.iter().map(Offer::of).collect();
        let Move::Switch { to, why } = decide(&Offer::of(m), fails, &offers, d.series.read_upto, &self.prefs, NEED_LEFT) else { return false };
        let Some(new) = alts.iter().find(|a| a.id == to.manga_id) else { return false };
        if let Err(e) = self.switch(w, d, new, now) {
            log(&format!("move {} to {}: {e}", w.title, new.source_name));
            return false;
        }
        self.store().event(now, Some(w.id), "switched", &format!("Moved {} to {}: {why}", w.title, new.source_name));
        true
    }

    /// A watched series (only on Webtoon until now) that another source has: said once, as an
    /// "elsewhere" event the phone posts as a notice, and the watch ends.
    fn found_elsewhere(&self, w: &Work, alts: &[Manga], now: i64) {
        let key = format!("watch:{}", w.id);
        if self.store().get(&key).is_none() {
            return;
        }
        let found: Vec<(&str, Vec<f64>)> = alts.iter().map(|a| (a.source_name.as_str(), a.chapters.iter().map(|c| c.number).collect())).collect();
        if let Some(text) = elsewhere_words(&w.title, &found) {
            self.store().event(now, Some(w.id), "elsewhere", &text);
            self.store().del(&key);
        }
    }

    fn switch(&self, w: &Work, d: &SeriesDetail, new: &Manga, now: i64) -> Result<(), String> {
        self.suwa.set_in_library(new.id, true)?;
        let (read, part) = carried(d, new);
        self.suwa.mark(&read, Some(true), None)?;
        for (id, page) in part {
            self.suwa.mark(&[id], None, Some(page))?;
        }
        // Unfollowed meanwhile: the new copy goes back out of the library.
        if let Err(e) = self.store().move_work(w.id, new.id, &new.source_name, now) {
            let _ = self.suwa.set_in_library(new.id, false);
            return Err(e);
        }
        let _ = self.suwa.set_in_library(w.manga_id, false);
        // The old source stays linked: it may still fill a chapter the new one lacks.
        self.store().remove_link(new.id);
        self.store().add_link(w.id, w.manga_id, &w.source);
        let moved = self.store().work(w.id);
        if let Some(w2) = moved
            && let Ok(m) = self.suwa.manga(new.id)
        {
            self.keep_ahead(&self.detail_now(&w2, &m));
        }
        Ok(())
    }
}

/// A series as the hourly pass weighs when to look at it.
#[derive(Clone, Copy, Debug)]
pub struct Due {
    pub id: i64,
    pub checked_at: i64,
    pub compared_at: i64,
    pub fails: u32,
    /// Its source asked for fewer requests: left alone until the pause is over.
    pub paused: bool,
}

/// What one hourly pass does.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    /// Refreshed, oldest checked first.
    pub check: Vec<i64>,
    /// Compared with the other sources, oldest compared first.
    pub compare: Vec<i64>,
}

/// A failing series is tried again after this long (the next pass), not every few minutes.
const RETRY_AFTER: i64 = 50 * 60_000;

/// The pass spread over the day: at most ceil(series / 6) refreshed (each due every
/// [`CHECK_EVERY`]) and ceil(series / 24) compared (each due every [`COMPARE_EVERY`]), so a full
/// round takes 6 h and 24 h without a burst. A series whose source failed goes first (three
/// failures in a row move it), and one whose source is paused waits.
pub fn plan(works: &[Due], now: i64) -> Plan {
    let n = works.len();
    let mut check: Vec<&Due> = works.iter().filter(|w| !w.paused && (now - w.checked_at >= CHECK_EVERY || (w.fails > 0 && now - w.checked_at >= RETRY_AFTER))).collect();
    check.sort_by_key(|w| (w.fails == 0, w.checked_at, w.id));
    let failing = |w: &Due| w.fails + 1 >= FAILS_BEFORE_MOVING;
    let mut compare: Vec<&Due> = works.iter().filter(|w| !w.paused && (now - w.compared_at >= COMPARE_EVERY || failing(w))).collect();
    compare.sort_by_key(|w| (!failing(w), w.compared_at, w.id));
    Plan {
        check: check.into_iter().take(n.div_ceil((CHECK_EVERY / HOUR) as usize)).map(|w| w.id).collect(),
        compare: compare.into_iter().take(n.div_ceil((COMPARE_EVERY / HOUR) as usize)).map(|w| w.id).collect(),
    }
}

/// What a move carries to the new source: the chapters read there by number (exactly those, not
/// all up to the furthest one read: a peek at the latest chapter must not mark the ones between
/// read), and the page of each chapter begun but not finished.
pub fn carried(d: &SeriesDetail, new: &Manga) -> (Vec<i64>, Vec<(i64, u32)>) {
    let read: Vec<i64> = new.chapters.iter().filter(|c| !c.read && d.chapters.iter().any(|x| x.read && x.number == c.number)).map(|c| c.id).collect();
    let part = new
        .chapters
        .iter()
        .filter_map(|c| d.chapters.iter().find(|x| !x.read && x.last_page > 0 && x.number == c.number).filter(|_| c.last_page == 0).map(|x| (c.id, x.last_page)))
        .collect();
    (read, part)
}

/// What a watched series' notice says once a source other than Webtoon has chapters of it (the
/// copies found: source, chapter numbers); None while none does.
pub fn elsewhere_words(title: &str, found: &[(&str, Vec<f64>)]) -> Option<String> {
    let best = found.iter().filter(|(src, ns)| !src.eq_ignore_ascii_case(WEBTOONS) && !ns.is_empty()).filter_map(|(_, ns)| plausible_latest(ns)).max_by(f64::total_cmp)?;
    Some(format!("{title} is on another source now, up to chapter {}", fmt_num(best)))
}

pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{n}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(id: i64, source: &str, numbers: impl IntoIterator<Item = i64>) -> Offer {
        Offer { manga_id: id, source: source.into(), numbers: numbers.into_iter().map(|n| n as f64).collect() }
    }

    fn prefs() -> Vec<String> {
        vec!["Asura Scans".into(), "MangaDex".into()]
    }

    fn manga(id: i64, source: &str, numbers: &[f64], read: &[f64]) -> Manga {
        Manga {
            id,
            title: "Junk to King".into(),
            source_name: source.into(),
            chapters: numbers
                .iter()
                .enumerate()
                .map(|(i, n)| crate::suwa::Ch {
                    id: id * 1000 + i as i64,
                    number: *n,
                    name: String::new(),
                    group: Some(source.into()),
                    pages: 20,
                    read: read.contains(n),
                    last_page: 0,
                    downloaded: false,
                    uploaded_at: 0,
                    last_read_at: 0,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn work() -> Work {
        Work { id: 1, title: "Junk to King".into(), manga_id: 1, source: "MangaDex".into(), followed_at: 0, fails: 0, checked_at: 0, compared_at: 0, latest: 54.0, updated_at: 0, note: None, bell: false, shelf: None }
    }

    #[test]
    fn missing_chapters_are_filled_from_linked_sources() {
        let own: Vec<f64> = (1..=22).chain(45..=54).map(f64::from).collect();
        let drake: Vec<f64> = (1..=40).map(f64::from).chain([23.5]).collect();
        let other: Vec<f64> = (1..=84).map(f64::from).collect();
        let m = manga(1, "MangaDex", &own, &[1.0, 2.0]);
        let extras = [manga(3, "Other", &other, &[]), manga(2, "Drake Scans", &drake, &[23.0])];
        let prefs = vec!["Drake Scans".into(), "MangaDex".into()];
        let d = detail_of(&work(), &m, &extras, &prefs);
        assert!(d.series.gaps.is_empty(), "{:?}", d.series.gaps);
        let at = |n: f64| d.chapters.iter().find(|c| c.number == n).unwrap();
        assert_eq!(at(22.0).from, None, "the series' own source first");
        assert_eq!(at(23.0).from.as_deref(), Some("Drake Scans"), "then the preferred source");
        assert_eq!(at(23.5).from.as_deref(), Some("Drake Scans"), "with its part chapters");
        assert_eq!(at(41.0).from.as_deref(), Some("Other"), "then whoever has the rest");
        assert_eq!(at(45.0).from, None);
        assert_eq!(at(84.0).from.as_deref(), Some("Other"));
        assert!(at(23.0).read, "read on the linked copy counts");
        assert_eq!(d.series.latest, Some(84.0));
        assert_eq!(d.series.read_upto, Some(23.0));
        let alone = detail_of(&work(), &m, &[], &prefs);
        assert_eq!(alone.series.gaps.len(), 1, "no links, the gap stays honest");
    }

    #[test]
    fn runs_count_from_where_you_are() {
        let dex: Vec<f64> = (1..=22).chain(45..=54).map(f64::from).collect();
        assert_eq!(run_after(&dex, None), 22);
        assert_eq!(run_after(&dex, Some(10.0)), 12);
        assert_eq!(run_after(&dex, Some(22.0)), 0, "the gap is next");
        assert_eq!(run_after(&dex, Some(44.0)), 10);
        assert_eq!(run_after(&[1.0, 1.5, 2.0], Some(1.0)), 1);
    }

    #[test]
    fn the_source_you_can_keep_reading_wins() {
        let dex = offer(1, "MangaDex", (1..=22).chain(45..=54));
        let other = offer(2, "Other", 1..=40);
        let asura = offer(3, "Asura Scans", 1..=22);
        let all = [dex.clone(), other.clone(), asura.clone()];
        assert_eq!(best(&all, None, &prefs()).unwrap().manga_id, 2, "40 in a row beats 22");
        assert_eq!(best(&[dex.clone(), asura.clone()], None, &prefs()).unwrap().manga_id, 3, "a tie goes to the preferred source");
        assert_eq!(best(&all, Some(44.0), &prefs()).unwrap().manga_id, 1, "past the gap, MangaDex has the next ten");
        assert!(best(&[offer(4, "Empty", [])], None, &prefs()).is_none());
    }

    #[test]
    fn moves_only_for_a_clear_gain() {
        let dex = offer(1, "MangaDex", (1..=22).chain(45..=54));
        let other = offer(2, "Other", 1..=40);
        match decide(&dex, 0, std::slice::from_ref(&other), Some(22.0), &prefs(), 5) {
            Move::Switch { to, why } => {
                assert_eq!(to.manga_id, 2);
                assert_eq!(why, "Other has the next 18 chapters");
            }
            m => panic!("{m:?}"),
        }
        assert_eq!(decide(&dex, 0, std::slice::from_ref(&other), Some(10.0), &prefs(), 5), Move::Stay, "twelve left in a row here: keep this translation");
        let same = offer(3, "Asura Scans", (1..=22).chain(45..=54));
        assert_eq!(decide(&dex, 0, std::slice::from_ref(&same), Some(22.0), &prefs(), 5), Move::Stay, "never trade places for nothing, preferred or not");
        assert!(matches!(decide(&dex, 3, &[same], Some(22.0), &prefs(), 5), Move::Switch { .. }), "but a source that stopped answering is left");
        assert_eq!(decide(&dex, 3, &[], Some(22.0), &prefs(), 5), Move::Stay);
        let behind = offer(4, "Behind", 1..=5);
        assert_eq!(decide(&dex, 3, &[behind], Some(10.0), &prefs(), 5), Move::Stay, "not for a source with less to read");
    }

    #[test]
    fn sources_that_keep_pace_never_trade_places() {
        // Caught up at 101 on Asura; Flame posted 102 first. The linked copy fills 102: stay.
        let asura = offer(1, "Asura Scans", 1..=101);
        let flame = offer(2, "Flame Comics", 1..=102);
        assert_eq!(decide(&asura, 0, std::slice::from_ref(&flame), Some(101.0), &prefs(), NEED_LEFT), Move::Stay);
        // Next week, the other way round: still nothing to gain.
        let flame = offer(2, "Flame Comics", 1..=103);
        let asura = offer(1, "Asura Scans", 1..=104);
        assert_eq!(decide(&flame, 0, std::slice::from_ref(&asura), Some(103.0), &prefs(), NEED_LEFT), Move::Stay);
        // Asura has stopped at 101 while Flame went on to 110: that is a source run out.
        let flame = offer(2, "Flame Comics", 1..=110);
        let asura = offer(1, "Asura Scans", 1..=101);
        assert!(matches!(decide(&asura, 0, &[flame], Some(101.0), &prefs(), NEED_LEFT), Move::Switch { .. }));
    }

    #[test]
    fn a_caught_up_reader_is_not_moved_to_a_source_far_behind() {
        let dex = offer(1, "MangaDex", 1..=100);
        let behind = offer(4, "Behind", 1..=5);
        assert_eq!(decide(&dex, 3, std::slice::from_ref(&behind), Some(100.0), &prefs(), NEED_LEFT), Move::Stay);
        let level = offer(5, "Level", 1..=100);
        assert!(matches!(decide(&dex, 3, &[level], Some(100.0), &prefs(), NEED_LEFT), Move::Switch { .. }), "one as far along is fine");
    }

    #[test]
    fn a_move_carries_what_was_read_not_all_up_to_the_furthest() {
        let mut m = manga(1, "MangaDex", &(1..=54).map(f64::from).collect::<Vec<_>>(), &[]);
        for c in m.chapters.iter_mut() {
            c.read = c.number <= 10.0 || c.number == 54.0;
            if c.number == 11.0 {
                c.last_page = 7;
            }
        }
        let d = detail_of(&work(), &m, &[], &prefs());
        let new = manga(2, "Asura Scans", &(1..=54).map(f64::from).collect::<Vec<_>>(), &[]);
        let (read, part) = carried(&d, &new);
        let numbers: Vec<f64> = read.iter().map(|id| new.chapters.iter().find(|c| c.id == *id).unwrap().number).collect();
        assert_eq!(numbers, (1..=10).map(f64::from).chain([54.0]).collect::<Vec<_>>(), "11 to 53 stay unread");
        assert_eq!(part, vec![(2000 + 10, 7)], "chapter 11's page comes along");
    }

    #[test]
    fn passes_spread_the_checks_over_six_hours_and_comparisons_over_a_day() {
        let mut works: Vec<Due> = (1..=170).map(|id| Due { id, checked_at: id, compared_at: id, fails: 0, paused: false }).collect();
        let mut checked = std::collections::HashMap::new();
        let mut compared = std::collections::HashMap::new();
        let start = 10 * COMPARE_EVERY;
        for pass in 0..24 {
            let now = start + pass * HOUR;
            let p = plan(&works, now);
            assert!(p.check.len() <= 29, "ceil(170 / 6) at most: {}", p.check.len());
            assert!(p.compare.len() <= 8, "ceil(170 / 24) at most");
            for w in works.iter_mut() {
                if p.check.contains(&w.id) {
                    w.checked_at = now;
                    *checked.entry(w.id).or_insert(0) += 1;
                }
                if p.compare.contains(&w.id) {
                    w.compared_at = now;
                    *compared.entry(w.id).or_insert(0) += 1;
                }
            }
            if pass == 0 {
                assert_eq!(p.check[..3], [1, 2, 3], "oldest first");
            }
            if pass == 5 {
                assert_eq!(checked.len(), 170, "every series checked within six passes");
            }
        }
        assert!(checked.values().all(|n| *n == 4), "each checked once every six hours over a day: {:?}", checked.values().max());
        assert_eq!(compared.len(), 170, "every series compared within a day");
        assert!(compared.values().all(|n| *n == 1));
    }

    #[test]
    fn a_failing_series_goes_first_and_a_paused_source_waits() {
        let now = 10 * COMPARE_EVERY;
        let fresh = |id| Due { id, checked_at: now - HOUR, compared_at: now - HOUR, fails: 0, paused: false };
        let mut works: Vec<Due> = (1..=12).map(|id| Due { checked_at: 0, compared_at: 0, ..fresh(id) }).collect();
        works.push(Due { fails: 2, ..fresh(20) });
        works.push(Due { fails: 1, checked_at: now - 10 * 60_000, ..fresh(21) });
        works.push(Due { paused: true, checked_at: 0, compared_at: 0, ..fresh(22) });
        let p = plan(&works, now);
        assert_eq!(p.check.len(), 3, "ceil(15 / 6)");
        assert_eq!(p.check[0], 20, "the failing one first");
        assert!(!p.check.contains(&21), "tried ten minutes ago: next pass");
        assert!(!p.check.contains(&22) && !p.compare.contains(&22), "its source is paused");
        assert_eq!(p.compare, vec![20], "ceil(15 / 24) = 1, and a third failure may move it");
        assert_eq!(plan(&[], now), Plan::default());
    }

    #[test]
    fn a_search_asks_the_broad_sources_and_keeps_what_names_it() {
        let names: Vec<String> = ["Asura Scans", "MangaDex", "Drake Scans", "Weeb Central", "Toonily"].map(String::from).to_vec();
        let broad: Vec<String> = ["MangaDex", "asura scans", "MangaFire"].map(String::from).to_vec();
        // The broad ones installed, in their order; MangaFire isn't installed.
        assert_eq!(search_scope(&names, &broad, &Scope::Broad), vec!["MangaDex".to_string(), "Asura Scans".to_string()]);
        assert_eq!(search_scope(&names, &broad, &Scope::All).len(), 5);
        assert_eq!(search_scope(&names, &broad, &Scope::One("drake scans".into())), vec!["Drake Scans".to_string()]);
        assert!(search_scope(&names, &broad, &Scope::One("Nowhere".into())).is_empty());
        // None of the broad ones installed: every source, rather than none.
        assert_eq!(search_scope(&names, &["Gone".to_string()], &Scope::Broad).len(), 5);
        let found = |t: &str| crate::suwa::Found { id: 1, title: t.into() };
        let kept = keep_matching(vec![found("Solo Leveling"), found("Leveling With the Gods"), found("Solo Leveling: Ragnarok"), found("Solo Max-Level Newbie")], "solo leveling");
        assert_eq!(kept.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(), vec!["Solo Leveling", "Solo Leveling: Ragnarok"]);
    }

    #[test]
    fn a_lone_number_is_not_the_latest() {
        let mut n: Vec<f64> = (1..=150).map(f64::from).collect();
        assert_eq!(plausible_latest(&n), Some(150.0));
        n.push(20241105.0);
        n.push(1000.0);
        assert_eq!(plausible_latest(&n), Some(150.0), "a misread number and a teaser are left out");
        assert_eq!(plausible_latest(&[200.0]), Some(200.0), "a source with one chapter still has a latest");
        assert_eq!(plausible_latest(&(300..=350).chain(1..=10).map(f64::from).collect::<Vec<_>>()), Some(350.0));
        assert_eq!(plausible_latest(&[]), None);
    }

    #[test]
    fn a_watched_series_is_news_once_another_source_has_chapters() {
        assert_eq!(elsewhere_words("T", &[]), None);
        assert_eq!(elsewhere_words("T", &[("Webtoons.com", vec![1.0, 2.0])]), None, "Webtoon itself is not news");
        assert_eq!(elsewhere_words("T", &[("Asura Scans", vec![])]), None, "an entry with no chapters yet");
        assert_eq!(elsewhere_words("T", &[("Webtoons.com", vec![1.0, 2.0]), ("Asura Scans", vec![1.0, 2.0, 3.0]), ("MangaFire", vec![1.0, 2.0, 3.0, 4.0, 40.0, 41.0])]).as_deref(), Some("T is on another source now, up to chapter 41"));
    }
}
