//! The autopilot: what makes following a series need nothing more from you.
//!
//! - **Follow** searches every installed source and pins the series to the one you can read the
//!   most chapters of in a row from where you are, ties to your preferred sources.
//! - **Every hour** it refreshes the series not checked for `CHECK_EVERY`, keeps the next
//!   `ahead` unread chapters downloaded (Suwayomi converts them to WebP), and once a day compares
//!   the other sources: when yours is down to its last few chapters in a row and another has more,
//!   or yours failed three refreshes in a row, the other takes over, your place carried across.
//! - **Every day** it updates the installed extensions, which is how a site's change gets fixed.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use leaf_proto::{Candidate, Chapter, Copy, Hit, Series, SeriesDetail, gaps, pick_copies, same_work};

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

pub struct Ctx {
    pub suwa: Suwa,
    pub store: Mutex<Store>,
    pub lang: String,
    /// Source names in the order you prefer them ("Asura Scans", "MangaDex").
    pub prefs: Vec<String>,
    /// How many unread chapters to keep downloaded per series.
    pub ahead: usize,
    /// Page addresses per chapter, so a page asks Suwayomi once for the list.
    pub pages: Mutex<HashMap<i64, Vec<String>>>,
    /// leaf-server's data folder (backups go in `backups/`).
    pub data: std::path::PathBuf,
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
    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The work's linked copies on other sources, as Suwayomi last stored them.
    fn extras(&self, w: &Work) -> Vec<Manga> {
        let links = self.store().links(w.id);
        links.into_iter().filter(|id| *id != w.manga_id).filter_map(|id| self.suwa.manga(id).ok()).collect()
    }

    fn detail_now(&self, w: &Work, m: &Manga) -> SeriesDetail {
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

    /// Every installed source searched at once, the results grouped by work.
    pub fn search(&self, query: &str) -> Result<Vec<Candidate>, String> {
        let sources = self.suwa.sources(&self.lang)?;
        let found: Vec<(String, Vec<crate::suwa::Found>)> = std::thread::scope(|s| {
            let handles: Vec<_> = sources
                .iter()
                .map(|src| {
                    s.spawn(move || {
                        let r = self.suwa.search(&src.id, query);
                        self.note_source(&src.name, r.as_ref().err());
                        (src.name.clone(), r)
                    })
                })
                .collect();
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
                    None => out.push(Candidate { title: f.title.clone(), cover_of: Some(f.id), hits: vec![hit], followed: None }),
                }
            }
        }
        for c in &mut out {
            c.followed = works.iter().find(|w| c.hits.iter().any(|h| h.manga_id == w.manga_id) || same_work(&w.title, &c.title)).map(|w| w.id);
            c.hits.sort_by_key(|h| rank(&h.source, &self.prefs));
        }
        // Found on more sources first: more likely the work you meant.
        out.sort_by_key(|c| std::cmp::Reverse(c.hits.len()));
        Ok(out)
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
            let hs: Vec<_> = manga_ids.iter().map(|id| s.spawn(move || self.suwa.refresh(*id))).collect();
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
    fn names(&self, w: &Work, now: i64) -> Vec<String> {
        let mut names = self.store().aliases(w.id);
        let key = format!("aliases_at:{}", w.id);
        let tried = self.store().get(&key).and_then(|v| v.parse::<i64>().ok()).is_some_and(|at| now - at < 7 * COMPARE_EVERY);
        if names.is_empty() && !tried {
            match crate::anilist::titles(&w.title) {
                Ok(found) => {
                    self.store().add_aliases(w.id, &found);
                    names = found;
                }
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

    /// A chapter downloaded because you asked (it is kept until you remove it).
    pub fn download(&self, chapter_id: i64, on: bool) -> Result<(), String> {
        if on { self.suwa.enqueue(&[chapter_id]) } else { self.suwa.delete_downloads(&[chapter_id]) }
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
    fn pages_elsewhere(&self, chapter_id: i64) -> Option<Vec<String>> {
        let (manga_id, number) = self.suwa.chapter_place(chapter_id).ok()?;
        let work = self.store().work_of_any(manga_id)?;
        let mut copies: Vec<Manga> = std::iter::once(work.manga_id).chain(self.store().links(work.id)).filter(|id| *id != manga_id).filter_map(|id| self.suwa.manga(id).ok()).collect();
        copies.sort_by_key(|m| rank(&m.source_name, &self.prefs));
        for m in copies {
            for c in m.chapters.iter().filter(|c| c.number == number) {
                if let Ok(p) = self.suwa.pages(c.id)
                    && !p.is_empty()
                {
                    log(&format!("chapter {chapter_id}: read from {} instead", m.source_name));
                    return Some(p);
                }
            }
        }
        None
    }

    /// Forgets a chapter's page list, so the next request asks again (and may fall back).
    pub fn forget_pages(&self, chapter_id: i64) {
        self.pages.lock().unwrap_or_else(|p| p.into_inner()).remove(&chapter_id);
    }

    pub fn pages(&self, chapter_id: i64) -> Result<Vec<String>, String> {
        if let Some(p) = self.pages.lock().unwrap_or_else(|p| p.into_inner()).get(&chapter_id) {
            return Ok(p.clone());
        }
        let p = match self.suwa.pages(chapter_id) {
            Ok(p) => p,
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

    /// The chapter after the last one you read (`ahead` of them, 1 by default), downloaded before
    /// you get there. Nothing for a series you have not started: downloads are otherwise yours to
    /// ask for.
    fn keep_ahead(&self, d: &SeriesDetail) {
        let Some(upto) = d.series.read_upto else { return };
        let want: Vec<i64> = d.chapters.iter().filter(|c| c.number > upto && !c.read).take(self.ahead).filter(|c| !c.downloaded).map(|c| c.id).collect();
        if let Err(e) = self.suwa.enqueue(&want) {
            log(&format!("download {}: {e}", d.series.title));
        }
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

    pub fn health(&self) -> Vec<leaf_proto::SourceHealth> {
        let names: Vec<String> = self.suwa.sources(&self.lang).map(|v| v.into_iter().map(|s| s.name).collect()).unwrap_or_default();
        let rows = self.store().health();
        names
            .into_iter()
            .map(|name| {
                let (ok_at, err_at, err) = rows.iter().find(|r| r.0 == name).map(|r| (r.1, r.2, r.3.clone())).unwrap_or((0, 0, None));
                let failing = err_at > ok_at;
                leaf_proto::SourceHealth {
                    blocked: failing && err.as_deref().is_some_and(|e| e.to_lowercase().contains("cloudflare")),
                    ok: !failing,
                    last_ok_at: ok_at,
                    last_error: if failing { err } else { None },
                    name,
                }
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
        let names = self.names(&w, now_ms());
        let names = &names;
        let sources = self.suwa.sources(&self.lang)?;
        let mut found: Vec<Manga> = std::thread::scope(|s| {
            let hs: Vec<_> = sources
                .iter()
                .map(|src| {
                    s.spawn(move || {
                        let mut hits: Vec<Manga> = Vec::new();
                        for query in names.iter().take(2) {
                            let Ok(list) = self.suwa.search(&src.id, query) else { continue };
                            for f in list.into_iter().filter(|f| names.iter().any(|n| same_work(&f.title, n))).take(3) {
                                if !hits.iter().any(|h| h.id == f.id)
                                    && let Ok(m) = self.suwa.refresh(f.id)
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
        let new = self.suwa.refresh(manga_id)?;
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
            let id_key = format!("al_id:{}", w.id);
            let known = self.store().get(&id_key);
            let media = match known {
                Some(v) => v.parse::<i64>().ok(),
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(900));
                    let found = crate::anilist::media_id(&w.title).ok().flatten();
                    self.store().set(&id_key, &found.map(|i| i.to_string()).unwrap_or_else(|| "none".into()));
                    found
                }
            };
            let Some(media) = media else { continue };
            std::thread::sleep(std::time::Duration::from_millis(900));
            match crate::anilist::save_progress(&token, media, progress, done) {
                Ok(()) => self.store().set(&sent_key, &want),
                Err(e) => {
                    log(&format!("anilist {}: {e}", w.title));
                    if e.contains("Invalid token") || e.contains("Unauthorized") {
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
        for w in works.iter().filter(|w| now - w.checked_at >= CHECK_EVERY || w.fails > 0) {
            self.check(w, now_ms());
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
    }

    fn check(&self, w: &Work, now: i64) {
        let fresh = self.suwa.refresh(w.manga_id);
        let (ok, m) = match fresh {
            Ok(m) => (true, m),
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
        if (fails >= FAILS_BEFORE_MOVING || now - w.compared_at >= COMPARE_EVERY)
            && self.compare(w, &m, &d, fails, now) {
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
        let names = self.names(w, now);
        let names = &names;
        let alts: Vec<Manga> = std::thread::scope(|s| {
            let hs: Vec<_> = sources
                .iter()
                .filter(|src| src.id != m.source_id)
                .map(|src| {
                    s.spawn(move || {
                        // Each title in turn (three at most), until one finds the work.
                        for query in names.iter().take(3) {
                            let Ok(found) = self.suwa.search(&src.id, query) else { continue };
                            if let Some(hit) = found.into_iter().find(|f| names.iter().any(|n| same_work(&f.title, n))) {
                                return self.suwa.refresh(hit.id).ok();
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
}
