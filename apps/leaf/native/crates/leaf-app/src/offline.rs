//! Downloads: chapters kept on this phone for reading offline. The server's own copies are its
//! business (the autopilot keeps the next chapter, trims old ones); a download here is a phone copy.
//!
//! - **The store** is `offline/<chapter>/` in the data folder, apart from the page cache
//!   (`cache/`, which [`crate::images::prune_cache`] trims): the pages as the server sent them
//!   (`0`, `1`, …), a scanlator's promo page the server skipped as `<n>.skip`, and
//!   `manifest.json` ([`Manifest`]: the series, the chapter, its pages and bytes), written last, so
//!   a folder without one is a download not finished. The reader reads a page from here before
//!   anything else ([`crate::images`]), so a saved chapter needs no network.
//! - **The line** is `offline/queue.json` ([`Item`]), kept on every change and read again at
//!   start, so it survives the app being closed. One worker thread takes one chapter at a time,
//!   its pages one after another, with a pause between chapters. A chapter the server has not
//!   downloaded itself is first put in the server's own line (`POST /v1/downloads`, with the next
//!   few behind it), and its pages are fetched once the server has it: they then come from the
//!   server's disk, never from a site, so the server's budget per source is kept. A folder left
//!   half done is taken up again where it stopped.
//! - Removing a download removes the phone's copy only; nothing is asked of the server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use leaf_proto::{Chapter, DownloadIds, Pages, Series, SeriesDetail};
use serde::{Deserialize, Serialize};

use crate::net::{Client, Fail, Server, write_atomic};

/// A chapter kept on this phone (`offline/<chapter>/manifest.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    pub series: i64,
    /// The series' title, for Downloads with no server.
    pub title: String,
    /// The chapter as the server listed it when it was saved (number, name, read).
    pub chapter: Chapter,
    pub pages: u32,
    /// Pages the server skipped (promo pages), kept as `<n>.skip`.
    #[serde(default)]
    pub skipped: Vec<u32>,
    pub bytes: u64,
    /// When it was saved (ms).
    pub saved_at: i64,
}

/// A chapter waiting to be saved (`offline/queue.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub series: i64,
    pub title: String,
    pub chapter: Chapter,
    /// The server was asked to download it into its own line.
    #[serde(default)]
    pub asked: bool,
    /// Why it didn't save, until it is tried again.
    #[serde(default)]
    pub failed: Option<String>,
}

/// A chapter as this phone holds it.
#[derive(Clone, Debug, PartialEq)]
pub enum Kept {
    None,
    /// In line, or the server is getting it first.
    Waiting,
    /// Pages saved so far, of how many.
    Saving(u32, u32),
    Saved,
    /// Didn't save, and why.
    Failed(String),
}

/// One series in Downloads.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub series: i64,
    pub title: String,
    pub chapters: usize,
    pub bytes: u64,
    /// The highest chapter number saved, and when the last one was saved (ms).
    pub newest: f64,
    pub saved_at: i64,
}

/// What the line is doing, for a line in Downloads.
#[derive(Clone, Debug, PartialEq)]
pub struct Running {
    /// The chapter being saved: its series' title, number, pages done and in all (0 while the
    /// server gets it).
    pub now: Option<(String, f64, u32, u32)>,
    /// Chapters in line (the one being saved too).
    pub waiting: usize,
    pub failed: usize,
}

/// How long the line waits, between chapters and for the server.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// A pause between two chapters.
    pub between: Duration,
    /// How often the server is asked whether it has a chapter yet.
    pub poll: Duration,
    /// A chapter the server hasn't got after this long didn't save.
    pub give_up: Duration,
    /// The server couldn't be reached: the line rests this long before trying again.
    pub rest: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing { between: Duration::from_secs(2), poll: Duration::from_secs(15), give_up: Duration::from_secs(20 * 60), rest: Duration::from_secs(60) }
    }
}

/// How many chapters behind the one being saved go to the server's line with it.
const AHEAD: usize = 4;

struct Inner {
    queue: Vec<Item>,
    saved: HashMap<i64, Manifest>,
    /// The folder was read (Downloads shows nothing before).
    scanned: bool,
    /// The chapter being saved: (id, pages done, pages in all).
    now: Option<(i64, u32, u32)>,
    server: Option<Server>,
    /// Promo pages are wanted too (the setting off).
    show_promo: bool,
    /// The line rests until then, the server out of reach.
    rest_until: Option<std::time::Instant>,
    timing: Timing,
    /// Each change to the line, so its writes land in order.
    queue_seq: u64,
}

struct Shared {
    m: Mutex<Inner>,
    cv: Condvar,
    data: PathBuf,
    /// The line's last write.
    written: Mutex<u64>,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The downloads and their line. The app holds one; its worker thread does the fetching.
pub struct Saver {
    sh: Arc<Shared>,
}

/// The folder a saved chapter's pages are in.
pub fn chapter_dir(data: &Path, chapter: i64) -> PathBuf {
    data.join("offline").join(chapter.to_string())
}

fn queue_file(data: &Path) -> PathBuf {
    data.join("offline").join("queue.json")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl Saver {
    pub fn new(data: PathBuf) -> Saver {
        Saver::with_timing(data, Timing::default())
    }

    pub fn with_timing(data: PathBuf, timing: Timing) -> Saver {
        let sh = Arc::new(Shared {
            m: Mutex::new(Inner { queue: Vec::new(), saved: HashMap::new(), scanned: false, now: None, server: None, show_promo: false, rest_until: None, timing, queue_seq: 0 }),
            cv: Condvar::new(),
            data,
            written: Mutex::new(0),
        });
        let w = sh.clone();
        std::thread::Builder::new().name("leaf-save".into()).spawn(move || worker(&w)).expect("the download thread starts");
        Saver { sh }
    }

    /// The server to save from (none: the line waits).
    pub fn set_server(&self, s: Option<Server>) {
        let mut g = self.sh.lock();
        if g.server != s {
            g.server = s;
            g.rest_until = None;
            self.sh.cv.notify_all();
        }
    }

    /// Promo pages are saved too (the setting "Skip scanlator promo pages" off).
    pub fn set_show_promo(&self, on: bool) {
        self.sh.lock().show_promo = on;
    }

    /// Put chapters in line: those not saved or waiting already (one that didn't save is tried
    /// again). How many were added.
    pub fn add(&self, series: i64, title: &str, chapters: &[Chapter]) -> usize {
        let mut g = self.sh.lock();
        let mut n = 0;
        for c in chapters {
            if g.saved.contains_key(&c.id) {
                continue;
            }
            match g.queue.iter_mut().find(|i| i.chapter.id == c.id) {
                Some(i) => {
                    if i.failed.take().is_some() {
                        n += 1;
                    }
                }
                None => {
                    g.queue.push(Item { series, title: title.to_string(), chapter: c.clone(), asked: false, failed: None });
                    n += 1;
                }
            }
        }
        if n > 0 {
            g.rest_until = None;
            drop(g);
            self.changed();
        }
        n
    }

    /// Try again the chapters that didn't save (all of them when `ids` is empty).
    pub fn retry(&self, ids: &[i64]) {
        let mut g = self.sh.lock();
        for i in g.queue.iter_mut().filter(|i| ids.is_empty() || ids.contains(&i.chapter.id)) {
            i.failed = None;
        }
        g.rest_until = None;
        drop(g);
        self.changed();
    }

    /// Remove these chapters from this phone (and from the line): (how many, bytes freed). The
    /// folders go on a thread.
    pub fn delete(&self, ids: &[i64]) -> (usize, u64) {
        let mut g = self.sh.lock();
        let (mut n, mut bytes) = (0, 0);
        for id in ids {
            if let Some(m) = g.saved.remove(id) {
                n += 1;
                bytes += m.bytes;
            } else if g.queue.iter().any(|i| i.chapter.id == *id) {
                n += 1;
            }
        }
        g.queue.retain(|i| !ids.contains(&i.chapter.id));
        drop(g);
        let (data, ids) = (self.sh.data.clone(), ids.to_vec());
        let _ = std::thread::Builder::new().name("leaf-delete".into()).spawn(move || {
            for id in ids {
                let _ = std::fs::remove_dir_all(chapter_dir(&data, id));
            }
            crate::wake::wake();
        });
        self.changed();
        (n, bytes)
    }

    /// Stop every download not finished yet: the line emptied, half-saved folders removed (the
    /// worker drops the chapter it is on at its next page). How many were stopped.
    pub fn cancel_all(&self) -> usize {
        let ids: Vec<i64> = self.sh.lock().queue.iter().map(|i| i.chapter.id).collect();
        self.delete(&ids).0
    }

    /// The line changed: kept on disk, the worker told.
    fn changed(&self) {
        persist(&self.sh);
        self.sh.cv.notify_all();
        crate::wake::wake();
    }

    /// The folder was read: Downloads can show what it holds.
    pub fn scanned(&self) -> bool {
        self.sh.lock().scanned
    }

    pub fn state(&self, chapter: i64) -> Kept {
        let g = self.sh.lock();
        if g.saved.contains_key(&chapter) {
            return Kept::Saved;
        }
        match g.queue.iter().find(|i| i.chapter.id == chapter) {
            None => Kept::None,
            Some(i) => match (&i.failed, g.now) {
                (Some(why), _) => Kept::Failed(why.clone()),
                (None, Some((id, done, all))) if id == chapter && all > 0 => Kept::Saving(done, all),
                _ => Kept::Waiting,
            },
        }
    }

    pub fn is_saved(&self, chapter: i64) -> bool {
        self.sh.lock().saved.contains_key(&chapter)
    }

    pub fn manifest(&self, chapter: i64) -> Option<Manifest> {
        self.sh.lock().saved.get(&chapter).cloned()
    }

    /// A series' saved chapters, oldest first.
    pub fn saved_of(&self, series: i64) -> Vec<Manifest> {
        let mut out: Vec<Manifest> = self.sh.lock().saved.values().filter(|m| m.series == series).cloned().collect();
        out.sort_by(|a, b| a.chapter.number.total_cmp(&b.chapter.number));
        out
    }

    /// Downloads kept under series `old` are kept under `new` from now on (an id the server gave
    /// to another series: [`crate::known`]), in memory now and in their manifests on a thread.
    pub fn rekey(&self, old: i64, new: i64) {
        let mut g = self.sh.lock();
        let mut moved = 0;
        for m in g.saved.values_mut().filter(|m| m.series == old) {
            m.series = new;
            moved += 1;
        }
        for i in g.queue.iter_mut().filter(|i| i.series == old) {
            i.series = new;
            moved += 1;
        }
        drop(g);
        let data = self.sh.data.clone();
        let _ = std::thread::Builder::new().name("leaf-rekey".into()).spawn(move || {
            for d in std::fs::read_dir(data.join("offline")).into_iter().flatten().flatten() {
                let file = d.path().join("manifest.json");
                if let Some(mut m) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Manifest>(&b).ok())
                    && m.series == old
                {
                    m.series = new;
                    let _ = write_atomic(&file, &serde_json::to_vec(&m).unwrap_or_default());
                }
            }
        });
        if moved > 0 {
            self.changed();
        }
    }

    /// Every series with a chapter on this phone or in the line.
    pub fn series_ids(&self) -> std::collections::HashSet<i64> {
        let g = self.sh.lock();
        g.saved.values().map(|m| m.series).chain(g.queue.iter().map(|i| i.series)).collect()
    }

    /// Every saved chapter's (id, series), for choosing what to delete.
    pub fn saved_ids(&self) -> Vec<(i64, i64)> {
        self.sh.lock().saved.values().map(|m| (m.chapter.id, m.series)).collect()
    }

    /// Downloads, per series: the one saved to last first.
    pub fn groups(&self) -> Vec<Group> {
        let g = self.sh.lock();
        let mut by: HashMap<i64, Group> = HashMap::new();
        for m in g.saved.values() {
            let e = by.entry(m.series).or_insert_with(|| Group { series: m.series, title: m.title.clone(), chapters: 0, bytes: 0, newest: f64::MIN, saved_at: 0 });
            e.chapters += 1;
            e.bytes += m.bytes;
            e.newest = e.newest.max(m.chapter.number);
            if m.saved_at >= e.saved_at {
                e.saved_at = m.saved_at;
                e.title = m.title.clone();
            }
        }
        let mut out: Vec<Group> = by.into_values().collect();
        out.sort_by_key(|g| (std::cmp::Reverse(g.saved_at), g.series));
        out
    }

    /// What the line is doing.
    pub fn running(&self) -> Running {
        let g = self.sh.lock();
        let now = g.now.and_then(|(id, done, all)| g.queue.iter().find(|i| i.chapter.id == id).map(|i| (i.title.clone(), i.chapter.number, done, all)));
        Running { now, waiting: g.queue.iter().filter(|i| i.failed.is_none()).count(), failed: g.queue.iter().filter(|i| i.failed.is_some()).count() }
    }

    /// Chapters that didn't save, for Retry.
    pub fn failed_ids(&self) -> Vec<i64> {
        self.sh.lock().queue.iter().filter(|i| i.failed.is_some()).map(|i| i.chapter.id).collect()
    }

    /// Screenshots: chapters shown as saved, without a page.
    pub fn shot_saved(&self, list: Vec<Manifest>) {
        let mut g = self.sh.lock();
        g.scanned = true;
        for m in list {
            g.saved.insert(m.chapter.id, m);
        }
    }

    /// Screenshots: chapters shown in line, the first being saved (`done` of `all` pages).
    pub fn shot_line(&self, items: Vec<Item>, done: u32, all: u32) {
        let mut g = self.sh.lock();
        g.now = items.first().map(|i| (i.chapter.id, done, all));
        g.queue = items;
    }

    #[cfg(test)]
    pub fn wait_idle(&self, secs: f32) -> bool {
        for _ in 0..(secs * 200.0) as usize {
            {
                let g = self.sh.lock();
                if g.scanned && g.now.is_none() && g.queue.iter().all(|i| i.failed.is_some()) {
                    return true;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }
}

/// A series' page from what is saved alone (no server): its saved chapters, oldest first, under
/// `series` when the library knows it.
pub fn saved_detail(series: Option<Series>, saved: &[Manifest]) -> Option<SeriesDetail> {
    let first = saved.first()?;
    let latest = saved.iter().map(|m| m.chapter.number).fold(f64::MIN, f64::max);
    let series = series.unwrap_or_else(|| Series {
        id: first.series,
        title: first.title.clone(),
        author: None,
        source: String::new(),
        status: "unknown".into(),
        chapters: saved.len() as u32,
        read_upto: None,
        unread: saved.iter().filter(|m| !m.chapter.read).count() as u32,
        latest: Some(latest),
        gaps: Vec::new(),
        updated_at: 0,
        note: None,
        bell: false,
        added_at: 0,
        last_read_at: 0,
        shelf: None,
    });
    let chapters = saved.iter().map(|m| Chapter { pages: m.pages, ..m.chapter.clone() }).collect();
    Some(SeriesDetail { series: Series { gaps: Vec::new(), ..series }, description: None, chapters })
}

/// Keep the line on disk, on a thread: writes land in the order they were asked.
fn persist(sh: &Arc<Shared>) {
    let (list, seq) = {
        let mut g = sh.lock();
        g.queue_seq += 1;
        (g.queue.clone(), g.queue_seq)
    };
    let sh = sh.clone();
    let _ = std::thread::Builder::new().name("leaf-line".into()).spawn(move || {
        let mut written = sh.written.lock().unwrap_or_else(|e| e.into_inner());
        if *written > seq {
            return;
        }
        let file = queue_file(&sh.data);
        let _ = if list.is_empty() { std::fs::remove_file(&file).or(Ok(())) } else { write_atomic(&file, &serde_json::to_vec(&list).unwrap_or_default()) };
        *written = seq;
    });
}

/// What `offline/` holds: the finished chapters, and the line. Folders neither finished nor in
/// line (a download removed while it was being saved) are removed.
fn scan(data: &Path) -> (HashMap<i64, Manifest>, Vec<Item>) {
    let queue: Vec<Item> = std::fs::read(queue_file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let mut saved = HashMap::new();
    for d in std::fs::read_dir(data.join("offline")).into_iter().flatten().flatten() {
        let Some(id) = d.file_name().to_str().and_then(|n| n.parse::<i64>().ok()) else { continue };
        match std::fs::read(d.path().join("manifest.json")).ok().and_then(|b| serde_json::from_slice::<Manifest>(&b).ok()) {
            Some(m) => {
                saved.insert(id, m);
            }
            None if !queue.iter().any(|i| i.chapter.id == id) => {
                let _ = std::fs::remove_dir_all(d.path());
            }
            None => {}
        }
    }
    (saved, queue)
}

/// How a chapter's save ended.
enum End {
    Saved(Manifest),
    /// The server couldn't be reached: the chapter stays in line.
    Offline,
    Failed(String),
    /// Removed from the line meanwhile.
    Gone,
}

fn worker(sh: &Arc<Shared>) {
    let (saved, disk) = scan(&sh.data);
    {
        let mut g = sh.lock();
        // Chapters added before the folder was read go behind the ones from before.
        let mine = std::mem::take(&mut g.queue);
        g.queue = disk.into_iter().filter(|i| !saved.contains_key(&i.chapter.id)).collect();
        for i in mine {
            if !saved.contains_key(&i.chapter.id) && !g.queue.iter().any(|q| q.chapter.id == i.chapter.id) {
                g.queue.push(i);
            }
        }
        // Deleted before the folder was read: none here yet, so only those found are kept.
        g.saved = saved;
        g.scanned = true;
    }
    crate::wake::wake();
    let client = Client::new();
    loop {
        let (item, server, promo, timing) = {
            let mut g = sh.lock();
            loop {
                let resting = g.rest_until.is_some_and(|t| std::time::Instant::now() < t);
                if let (Some(s), false) = (&g.server, resting)
                    && let Some(i) = g.queue.iter().find(|i| i.failed.is_none())
                {
                    break (i.clone(), s.clone(), g.show_promo, g.timing);
                }
                g = match g.rest_until.filter(|_| resting) {
                    Some(t) => sh.cv.wait_timeout(g, t.saturating_duration_since(std::time::Instant::now())).unwrap_or_else(|e| e.into_inner()).0,
                    None => sh.cv.wait(g).unwrap_or_else(|e| e.into_inner()),
                };
            }
        };
        let id = item.chapter.id;
        sh.lock().now = Some((id, 0, 0));
        crate::wake::wake();
        let end = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| save(sh, &client, &server, &item, promo, &timing))).unwrap_or_else(|_| End::Failed("Something went wrong saving it".into()));
        {
            let mut g = sh.lock();
            g.now = None;
            match end {
                End::Saved(m) => {
                    if g.queue.iter().any(|i| i.chapter.id == id) {
                        g.queue.retain(|i| i.chapter.id != id);
                        g.saved.insert(id, m);
                    } else {
                        let _ = std::fs::remove_dir_all(chapter_dir(&sh.data, id));
                    }
                }
                End::Offline => g.rest_until = Some(std::time::Instant::now() + timing.rest),
                End::Failed(why) => {
                    if let Some(i) = g.queue.iter_mut().find(|i| i.chapter.id == id) {
                        i.failed = Some(why);
                    }
                }
                End::Gone => {
                    let _ = std::fs::remove_dir_all(chapter_dir(&sh.data, id));
                }
            }
        }
        persist(sh);
        crate::wake::wake();
        // A breath between chapters; a change to the line ends it early only by its own notify.
        let g = sh.lock();
        drop(sh.cv.wait_timeout(g, timing.between));
    }
}

fn in_line(sh: &Shared, id: i64) -> bool {
    sh.lock().queue.iter().any(|i| i.chapter.id == id)
}

fn words(f: Fail) -> String {
    match f {
        Fail::Answer(404, _) => "Your server doesn't have this chapter any more".into(),
        f => f.words("your server"),
    }
}

/// Put the chapter, and the next few in line, in the server's own download line.
fn ask_server(sh: &Arc<Shared>, client: &Client, s: &Server, id: i64) -> Result<(), Fail> {
    let ids: Vec<i64> = {
        let g = sh.lock();
        let mut ids = vec![id];
        ids.extend(g.queue.iter().filter(|i| i.chapter.id != id && !i.asked && i.failed.is_none()).take(AHEAD).map(|i| i.chapter.id));
        ids
    };
    let body = serde_json::to_string(&DownloadIds { ids: ids.clone() }).map_err(|_| Fail::Garbled)?;
    client.post(s, "/v1/downloads", body)?;
    let mut g = sh.lock();
    for i in g.queue.iter_mut().filter(|i| ids.contains(&i.chapter.id)) {
        i.asked = true;
    }
    drop(g);
    persist(sh);
    Ok(())
}

fn progress(sh: &Shared, id: i64, done: u32, all: u32) {
    sh.lock().now = Some((id, done, all));
    crate::wake::wake();
}

/// Save one chapter: once the server has it, its length, then each page not saved yet.
fn save(sh: &Arc<Shared>, client: &Client, s: &Server, item: &Item, promo: bool, timing: &Timing) -> End {
    let id = item.chapter.id;
    let dir = chapter_dir(&sh.data, id);
    // The server keeps it first, so its pages come from the server's disk.
    let since = std::time::Instant::now();
    let mut asked = item.asked;
    let chapter = loop {
        if !in_line(sh, id) {
            return End::Gone;
        }
        let d = match client.json::<SeriesDetail>(s, &format!("/v1/series/{}", item.series)) {
            Ok(d) => d,
            Err(Fail::Unreachable) => return End::Offline,
            Err(f) => return End::Failed(words(f)),
        };
        let Some(c) = d.chapters.into_iter().find(|c| c.id == id) else { return End::Failed("Your server doesn't list this chapter any more".into()) };
        if c.downloaded {
            break c;
        }
        if !asked {
            match ask_server(sh, client, s, id) {
                Ok(()) => asked = true,
                Err(Fail::Unreachable) => return End::Offline,
                Err(f) => return End::Failed(words(f)),
            }
        }
        if since.elapsed() >= timing.give_up {
            return End::Failed("Your server couldn't get this chapter yet".into());
        }
        let g = sh.lock();
        drop(sh.cv.wait_timeout(g, timing.poll));
    };
    let all = match client.json::<Pages>(s, &format!("/v1/chapters/{id}/pages")) {
        Ok(p) => p.count,
        Err(Fail::Unreachable) => return End::Offline,
        Err(f) => return End::Failed(words(f)),
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return End::Failed("This phone couldn't keep it".into());
    }
    let cache = crate::images::chapter_dir(&sh.data, id);
    let mut skipped = Vec::new();
    for n in 0..all {
        if !in_line(sh, id) {
            return End::Gone;
        }
        progress(sh, id, n, all);
        let (file, mark) = (dir.join(n.to_string()), dir.join(format!("{n}.skip")));
        if mark.is_file() {
            skipped.push(n);
            continue;
        }
        if std::fs::metadata(&file).is_ok_and(|m| m.len() > 0) {
            continue;
        }
        // Read lately: the page cache has it already.
        if let Ok(b) = std::fs::read(cache.join(n.to_string()))
            && !b.is_empty()
            && !crate::images::blank(&b)
        {
            if write_atomic(&file, &b).is_err() {
                return End::Failed("This phone couldn't keep it".into());
            }
            continue;
        }
        let path = format!("/v1/chapters/{id}/pages/{n}{}", if promo { "?promo=1" } else { "" });
        let mut tries = 0;
        loop {
            match client.get_page(s, &path) {
                // A page sent blank (zeros) is not a page: nothing saved, the chapter failed.
                Ok(Some(b)) if crate::images::blank(&b) => return End::Failed(format!("Page {} came blank from its source", n + 1)),
                Ok(Some(b)) => {
                    if write_atomic(&file, &b).is_err() {
                        return End::Failed("This phone couldn't keep it".into());
                    }
                    break;
                }
                // A scanlator's promo page: left out, as the reader leaves it out.
                Ok(None) => {
                    let _ = std::fs::write(&mark, b"");
                    skipped.push(n);
                    break;
                }
                Err(Fail::Unreachable) => return End::Offline,
                Err(_) if tries < 2 => {
                    tries += 1;
                    let g = sh.lock();
                    drop(sh.cv.wait_timeout(g, timing.between));
                }
                Err(f) => return End::Failed(words(f)),
            }
        }
    }
    progress(sh, id, all, all);
    let bytes = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum();
    let m = Manifest { series: item.series, title: item.title.clone(), chapter: Chapter { pages: all, downloaded: false, ..chapter }, pages: all, skipped, bytes, saved_at: now_ms() };
    match write_atomic(&dir.join("manifest.json"), &serde_json::to_vec(&m).unwrap_or_default()) {
        Ok(()) => End::Saved(m),
        Err(_) => End::Failed("This phone couldn't keep it".into()),
    }
}

#[cfg(test)]
pub fn quick() -> Timing {
    Timing { between: Duration::from_millis(5), poll: Duration::from_millis(20), give_up: Duration::from_secs(5), rest: Duration::from_millis(200) }
}
