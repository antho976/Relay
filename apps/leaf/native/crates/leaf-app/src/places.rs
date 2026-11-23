//! The exact place in each series (the chapter, the page and how far down it), kept on the phone
//! in `places.json` so a close of the app, or Android ending it in the background, never loses
//! it. The reader keeps it every time the page changes and at most every [`EVERY_NS`] while the
//! strip moves within a page ([`Gate`]), and at once when the app goes to the background or the
//! reader closes. Files are written on a thread of their own, whole or not at all.
//!
//! The server hears the page only (`/v1/chapters/{id}/progress`, debounced); reopening prefers
//! this place when it is not older than what the server says was read last ([`fresh`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};

use serde::{Deserialize, Serialize};

/// While the strip moves within one page, the place is kept at most this often.
pub const EVERY_NS: u64 = 2_000_000_000;
/// The server's "last read" may be this much later than the place and still be the same reading:
/// it is stamped when the debounced report lands.
pub const SLACK_MS: i64 = 120_000;
/// Places kept, the most recent.
const KEPT: usize = 400;

/// Where the top of the screen was in a series.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub series: i64,
    pub chapter: i64,
    /// The chapter's number, so the place survives the series moving to another source (its
    /// chapters' ids change).
    #[serde(default)]
    pub number: f64,
    /// The page at the top of the screen (0-based); on the chapter's heading, 0 with `head`.
    pub page: u32,
    #[serde(default)]
    pub head: bool,
    /// How far down that page (or heading) the top of the screen is, in layout units.
    pub off: f32,
    /// When it was kept, ms since the epoch.
    pub at_ms: i64,
}

impl Place {
    /// The same spot, give or take a fraction of a unit.
    pub fn same_spot(&self, o: &Place) -> bool {
        self.series == o.series && self.chapter == o.chapter && self.page == o.page && self.head == o.head && (self.off - o.off).abs() < 0.5
    }
}

/// Whether the phone's place is to be trusted over the server's page: kept no earlier than a
/// little before the server last heard of reading (another phone reading later wins).
pub fn fresh(p: &Place, server_read_at_ms: i64) -> bool {
    p.at_ms + SLACK_MS >= server_read_at_ms
}

/// When the reader keeps its place: a page change at once, a move within the page at most every
/// [`EVERY_NS`], never the same spot twice.
#[derive(Default, Debug)]
pub struct Gate {
    last: Option<Place>,
    last_ns: u64,
}

impl Gate {
    /// The reader is at `p` at `now_ns`: true when it is to be kept now.
    pub fn due(&mut self, p: &Place, now_ns: u64) -> bool {
        let keep = match &self.last {
            None => true,
            Some(l) if l.same_spot(p) => false,
            Some(l) if l.chapter != p.chapter || l.page != p.page || l.head != p.head => true,
            Some(_) => now_ns.saturating_sub(self.last_ns) >= EVERY_NS,
        };
        if keep {
            self.last = Some(*p);
            self.last_ns = now_ns;
        }
        keep
    }

    /// When a move the gate held back is to be kept, at the latest.
    pub fn next_ns(&self) -> u64 {
        self.last_ns + EVERY_NS
    }

    /// Kept whatever the gate says (the app goes to the background).
    pub fn force(&mut self, p: &Place, now_ns: u64) {
        self.last = Some(*p);
        self.last_ns = now_ns;
    }
}

/// The places, in memory, with the thread that writes them.
pub struct Places {
    map: HashMap<i64, Place>,
    tx: Sender<Vec<Place>>,
}

fn file(data: &Path) -> PathBuf {
    data.join("places.json")
}

impl Places {
    /// The places kept on disk (a few kilobytes, read once at start).
    pub fn load(data: &Path) -> Places {
        let list: Vec<Place> = std::fs::read(file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let (tx, rx) = channel::<Vec<Place>>();
        let path = file(data);
        let _ = std::thread::Builder::new().name("leaf-places".into()).spawn(move || {
            while let Ok(mut list) = rx.recv() {
                // Only the latest of what queued up meanwhile is written.
                while let Ok(newer) = rx.try_recv() {
                    list = newer;
                }
                let _ = crate::net::write_atomic(&path, &serde_json::to_vec(&list).unwrap_or_default());
            }
        });
        Places { map: list.into_iter().map(|p| (p.series, p)).collect(), tx }
    }

    pub fn get(&self, series: i64) -> Option<&Place> {
        self.map.get(&series)
    }

    /// Forget a series' place (its id now names another series: [`crate::known`]).
    pub fn forget(&mut self, series: i64) {
        if self.map.remove(&series).is_some() {
            let _ = self.tx.send(self.map.values().copied().collect());
        }
    }

    /// Keep a place: in memory now, on disk from the writer's thread.
    pub fn put(&mut self, p: Place) {
        self.map.insert(p.series, p);
        let mut list: Vec<Place> = self.map.values().copied().collect();
        if list.len() > KEPT {
            list.sort_by_key(|p| std::cmp::Reverse(p.at_ms));
            list.truncate(KEPT);
            self.map = list.iter().map(|p| (p.series, *p)).collect();
        }
        let _ = self.tx.send(list);
    }
}

/// Where Continue opens a series: the phone's own place when it is fresh ([`fresh`]) and its
/// chapter is not one finished since (then the next, as the server says); else the server's
/// [`crate::logic::continue_point`].
pub fn resume(places: &Places, d: &leaf_proto::SeriesDetail) -> Option<crate::logic::Continue> {
    let cp = crate::logic::continue_point(&d.chapters)?;
    let Some(p) = places.get(d.series.id).filter(|p| fresh(p, d.series.last_read_at)) else { return Some(cp) };
    let Some(i) = d.chapters.iter().position(|c| c.id == p.chapter).or_else(|| d.chapters.iter().position(|c| c.number == p.number)) else { return Some(cp) };
    if d.chapters[i].read && cp.index > i {
        return Some(cp);
    }
    // Another chapter than the server's next, only opened (a tap by mistake): the server's next.
    if i != cp.index && !d.chapters[i].read && !crate::logic::read_into(if p.head { 0 } else { p.page }, d.chapters[i].pages) {
        return Some(cp);
    }
    let page = if p.head { 0 } else { p.page };
    Some(crate::logic::Continue { index: i, page, label: format!("Continue ch. {}", crate::logic::num(d.chapters[i].number)) })
}

/// The place to open a chapter at, when it is the phone's: the chapter matches (by id, else by
/// number), the place is fresh, and the page asked for is where reading stopped (the server's
/// resume page, or the place's own). None opens at the page as asked.
pub fn exact(places: &Places, d: &leaf_proto::SeriesDetail, index: usize, page: u32) -> Option<Place> {
    let c = d.chapters.get(index)?;
    let p = places.get(d.series.id)?;
    let same = p.chapter == c.id || (p.number == c.number && !d.chapters.iter().any(|o| o.id == p.chapter));
    let asked = page == p.page || (!c.read && page == crate::logic::resume_page(c));
    (same && asked && fresh(p, d.series.last_read_at)).then_some(*p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(chapter: i64, page: u32, off: f32) -> Place {
        Place { series: 1, chapter, number: 1.0, page, head: false, off, at_ms: 0 }
    }

    #[test]
    fn the_place_is_kept_on_each_page_and_every_two_seconds_within_one() {
        let mut g = Gate::default();
        let s = 1_000_000_000u64;
        assert!(g.due(&at(10, 0, 0.0), 0), "the first");
        assert!(!g.due(&at(10, 0, 0.0), s), "the same spot is not kept twice");
        assert!(!g.due(&at(10, 0, 200.0), s), "within the page, not yet");
        assert!(g.due(&at(10, 0, 300.0), 2 * s), "two seconds on");
        assert!(g.due(&at(10, 1, 10.0), 2 * s + 1), "a new page at once");
        assert!(g.due(&at(11, 1, 10.0), 2 * s + 2), "a new chapter at once");
        g.force(&at(11, 1, 50.0), 2 * s + 3);
        assert!(!g.due(&at(11, 1, 50.0), 3 * s));
    }

    #[test]
    fn a_place_older_than_the_servers_reading_gives_way() {
        let p = Place { at_ms: 1_000_000, ..at(1, 3, 0.0) };
        assert!(fresh(&p, 900_000));
        assert!(fresh(&p, 1_000_000 + SLACK_MS), "the server stamped the same reading a little later");
        assert!(!fresh(&p, 1_000_000 + SLACK_MS + 1), "read later elsewhere");
    }

    #[test]
    fn places_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("leaf-places-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = Places::load(&dir);
        p.put(Place { series: 4, ..at(40, 7, 123.5) });
        p.put(Place { series: 5, ..at(50, 1, 0.0) });
        p.put(Place { series: 4, ..at(40, 8, 12.0) });
        let mut back = None;
        for _ in 0..200 {
            let again = Places::load(&dir);
            if again.get(4).is_some_and(|p| p.page == 8) && again.get(5).is_some() {
                back = Some(again);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let back = back.expect("written");
        assert_eq!(back.get(4).map(|p| (p.chapter, p.page, p.off)), Some((40, 8, 12.0)));
        let _ = std::fs::remove_dir_all(dir);
    }
}

