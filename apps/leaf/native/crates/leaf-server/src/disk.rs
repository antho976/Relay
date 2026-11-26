//! The room chapters may take on the server (the owner, 2026-11-25: "have me set how many GB I'd
//! want to allocate to it and if it gets at 90% used it deletes some of the early chapters I've
//! read a while ago").
//!
//! The owner sets a size in Settings (`POST /v1/settings/disk`, kept as `disk_cap_gb`; none is no
//! limit). Each pass measures Suwayomi's downloads; at [`FULL_AT`] of the size it deletes
//! downloaded chapters already read, those read longest ago first, until they are down to
//! [`DOWN_TO`]. Never a chapter not read, and never the last [`KEEP_LAST`] read of a series (the
//! one you may open again). Deleting asks no site: it is Suwayomi's own folder. A chapter deleted
//! can still be read: it comes from its source again, as any chapter not downloaded does.

use std::collections::HashMap;
use std::path::Path;

use crate::autopilot::{log, now_ms};
use crate::suwa::Manga;

/// Cleaning starts at this share of the size set.
pub const FULL_AT: f64 = 0.9;
/// And goes on until the chapters take this share.
pub const DOWN_TO: f64 = 0.8;
/// The chapters read last in each series are kept.
pub const KEEP_LAST: usize = 2;
/// Chapters deleted at a time before measuring again.
const BATCH: usize = 25;
/// At most this many batches a pass.
const MAX_BATCHES: usize = 40;
const GB: u64 = 1 << 30;

/// The size of everything under `root`, in bytes (links not followed).
pub fn dir_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else if m.is_file() {
                total += m.len();
            }
        }
    }
    total
}

/// How many bytes to free: none below [`FULL_AT`] of `cap`, else down to [`DOWN_TO`] of it.
pub fn to_free(used: u64, cap: u64) -> u64 {
    if cap == 0 || (used as f64) < cap as f64 * FULL_AT {
        return 0;
    }
    used.saturating_sub((cap as f64 * DOWN_TO) as u64)
}

/// The downloaded chapters that may go, read longest ago first: (chapter id, when read). A
/// chapter counts as read when any copy of its number is (as the series page says); its time is
/// the latest any copy was read (0 for progress that came without one: those go first). The last
/// [`KEEP_LAST`] numbers read stay.
pub fn deletable(copies: &[&Manga]) -> Vec<(i64, i64)> {
    let mut read_at: HashMap<u64, (bool, i64)> = HashMap::new();
    for c in copies.iter().flat_map(|m| m.chapters.iter()) {
        let e = read_at.entry(c.number.to_bits()).or_insert((false, 0));
        e.0 |= c.read;
        e.1 = e.1.max(c.last_read_at);
    }
    let mut read: Vec<(f64, i64)> = read_at.iter().filter(|(_, (r, _))| *r).map(|(n, (_, at))| (f64::from_bits(*n), *at)).collect();
    read.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.total_cmp(&a.0)));
    let kept: Vec<u64> = read.iter().take(KEEP_LAST).map(|(n, _)| n.to_bits()).collect();
    let mut out: Vec<(i64, i64)> = copies
        .iter()
        .flat_map(|m| m.chapters.iter())
        .filter(|c| c.downloaded && c.number >= 0.0)
        .filter_map(|c| {
            let (r, at) = read_at.get(&c.number.to_bits())?;
            (*r && !kept.contains(&c.number.to_bits())).then_some((c.id, *at))
        })
        .collect();
    out.sort_by_key(|(id, at)| (*at, *id));
    out
}

/// What Settings shows of the room chapters take.
#[derive(serde::Serialize, Clone, Debug, Default, PartialEq)]
pub struct Room {
    /// Bytes the downloads take, as last measured (None: the folder isn't shared with leaf-server).
    pub used: Option<u64>,
    /// The size set, in GB; None is no limit.
    pub cap_gb: Option<u32>,
    /// When the last cleaning ran, and how many chapters it deleted.
    pub cleaned_at: i64,
    pub cleaned: u32,
    /// Over the size with nothing read left to delete.
    pub stuck: bool,
}

impl crate::autopilot::Ctx {
    pub fn disk_cap_gb(&self) -> Option<u32> {
        let v = self.store().get("disk_cap_gb");
        v.and_then(|v| v.parse().ok()).filter(|g| *g > 0)
    }

    pub fn set_disk_cap(&self, gb: Option<u32>) {
        match gb.filter(|g| *g > 0) {
            Some(g) => self.store().set("disk_cap_gb", &g.to_string()),
            None => self.store().del("disk_cap_gb"),
        }
    }

    fn measure(&self) -> Option<u64> {
        let root = self.downloads.as_ref()?;
        let used = dir_size(root);
        self.store().set("disk_used", &used.to_string());
        Some(used)
    }

    pub fn room(&self) -> Room {
        let get = |k: &str| self.store().get(k);
        let used = if self.downloads.is_some() { get("disk_used").and_then(|v| v.parse().ok()).or_else(|| self.measure()) } else { None };
        Room {
            used,
            cap_gb: self.disk_cap_gb(),
            cleaned_at: get("disk_cleaned_at").and_then(|v| v.parse().ok()).unwrap_or(0),
            cleaned: get("disk_cleaned").and_then(|v| v.parse().ok()).unwrap_or(0),
            stuck: get("disk_stuck").is_some(),
        }
    }

    /// The pass's look at the room: measured, and cleaned past [`FULL_AT`] of the size set.
    pub fn disk_pass(&self) {
        let Some(mut used) = self.measure() else { return };
        let Some(cap) = self.disk_cap_gb().map(|g| g as u64 * GB) else {
            self.store().del("disk_stuck");
            return;
        };
        if to_free(used, cap) == 0 {
            self.store().del("disk_stuck");
            return;
        }
        let works = self.store().works();
        let mut all: Vec<(i64, i64)> = Vec::new();
        for w in &works {
            let mut ids = vec![w.manga_id];
            ids.extend(self.store().links(w.id));
            let copies: Vec<Manga> = ids.iter().filter_map(|id| self.suwa.manga(*id).ok()).collect();
            all.extend(deletable(&copies.iter().collect::<Vec<_>>()));
        }
        all.sort_by_key(|(id, at)| (*at, *id));
        let mut deleted = 0u32;
        for batch in all.chunks(BATCH).take(MAX_BATCHES) {
            if to_free(used, cap) == 0 && used as f64 <= cap as f64 * DOWN_TO {
                break;
            }
            let ids: Vec<i64> = batch.iter().map(|(id, _)| *id).collect();
            if let Err(e) = self.suwa.delete_downloads(&ids) {
                log(&format!("room: deleting {} chapters: {e}", ids.len()));
                break;
            }
            deleted += ids.len() as u32;
            used = self.measure().unwrap_or(used);
            if used as f64 <= cap as f64 * DOWN_TO {
                break;
            }
        }
        let now = now_ms();
        if deleted > 0 {
            log(&format!("room: deleted {deleted} chapters read long ago ({} MB left of {} GB)", used >> 20, cap / GB));
            self.store().set("disk_cleaned_at", &now.to_string());
            self.store().set("disk_cleaned", &deleted.to_string());
        }
        if used as f64 >= cap as f64 * FULL_AT {
            self.store().set("disk_stuck", "1");
        } else {
            self.store().del("disk_stuck");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suwa::Ch;

    fn ch(id: i64, number: f64, read: bool, downloaded: bool, at: i64) -> Ch {
        Ch { id, number, name: String::new(), group: None, pages: 10, read, last_page: 0, downloaded, uploaded_at: 0, last_read_at: at }
    }

    #[test]
    fn cleaning_starts_at_ninety_percent_and_goes_down_to_eighty() {
        let cap = 10 * GB;
        assert_eq!(to_free(8 * GB, cap), 0);
        assert_eq!(to_free(9 * GB - 1, cap), 0);
        assert_eq!(to_free(9 * GB, cap), GB);
        assert_eq!(to_free(12 * GB, 0), 0, "no size set: no limit");
    }

    #[test]
    fn only_chapters_read_go_oldest_first_and_the_last_two_read_stay() {
        let a = Manga {
            chapters: vec![ch(1, 1.0, true, true, 100), ch(2, 2.0, true, true, 300), ch(3, 3.0, true, true, 500), ch(4, 4.0, true, true, 600), ch(5, 5.0, false, true, 0)],
            ..Default::default()
        };
        // Chapter 1 was read on another copy, imported with no time: it counts read, first to go.
        let b = Manga { chapters: vec![ch(11, 1.0, false, true, 0), ch(12, 2.0, false, false, 0)], ..Default::default() };
        let d = deletable(&[&a, &b]);
        assert_eq!(d, vec![(1, 100), (11, 100), (2, 300)], "3 and 4 read last stay, 5 is unread");
    }
}
