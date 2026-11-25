//! Which series each id named when this phone last kept something for it (`known.json`): its
//! title and when it was added. The phone keeps covers, banners, places, a copy of the series and
//! downloads by series id; leaf-server once gave a removed series' id to the next one added
//! ("Invincible Romance" was 172, then "The Awakening Contract" was, and the old cover showed,
//! 2026-11-23). It no longer does, but when an id comes back naming another series, what was kept
//! for the old one is let go ([`crate::app::App::forget_series`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use leaf_proto::Series;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Named {
    title: String,
    /// When it was added (ms; 0 when the server didn't say).
    #[serde(default)]
    added: i64,
}

#[derive(Default)]
pub struct Known {
    map: HashMap<i64, Named>,
    path: PathBuf,
    dirty: bool,
}

/// The id stands for another series than the one kept: another title, or (both known) another
/// time it was added.
fn other(k: &Named, s: &Series) -> bool {
    leaf_proto::norm_title(&k.title) != leaf_proto::norm_title(&s.title) || (k.added > 0 && s.added_at > 0 && k.added != s.added_at)
}

impl Known {
    /// Read at start (a few kilobytes).
    pub fn load(data: &Path) -> Known {
        let path = data.join("known.json");
        if !path.exists() {
            // Before this file, the phone never noticed an id the server gave out again, and may
            // hold another series' cover under it (172, 2026-11-23): series covers are cleared
            // once, here, and come back from the server as the screens ask for them.
            for f in std::fs::read_dir(data.join("covers")).into_iter().flatten().flatten() {
                if f.file_name().to_str().is_some_and(|n| n.starts_with("c-s") || n.starts_with("b-c-s") || n.starts_with("h-c-s") || n.starts_with("hp-c-s")) {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
        let map = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<HashMap<i64, Named>>(&b).ok()).unwrap_or_default();
        Known { map, path, dirty: false }
    }

    /// A series as the server sent it now (`live`) or as the copy on disk had it. A copy on disk
    /// only fills in an id not known yet (so an old copy names what was kept before); the
    /// server's answer is compared: true when the id now names another series, which is kept
    /// from now on.
    pub fn see(&mut self, s: &Series, live: bool) -> bool {
        if s.id <= 0 {
            return false;
        }
        let now = Named { title: s.title.clone(), added: s.added_at };
        match self.map.get(&s.id) {
            None => {
                self.map.insert(s.id, now);
                self.dirty = true;
                false
            }
            Some(k) if live && other(k, s) => {
                self.map.insert(s.id, now);
                self.dirty = true;
                true
            }
            Some(k) if live && k.added == 0 && s.added_at > 0 => {
                self.map.insert(s.id, now);
                self.dirty = true;
                false
            }
            Some(_) => false,
        }
    }

    /// Written when something changed (from the UI thread: it is small and changes rarely).
    pub fn save(&mut self) {
        if self.dirty {
            self.dirty = false;
            let _ = crate::net::write_atomic(&self.path, &serde_json::to_vec(&self.map).unwrap_or_default());
        }
    }
}

/// The series id downloads of a series whose id was given to another one are kept under, so they
/// never show under the new one's title (negative, far from a preview's `-manga_id`).
pub fn orphan_id(id: i64) -> i64 {
    -(1 << 40) - id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: i64, title: &str, added: i64) -> Series {
        Series {
            id,
            title: title.into(),
            author: None,
            source: String::new(),
            status: String::new(),
            chapters: 0,
            read_upto: None,
            unread: 0,
            latest: None,
            gaps: vec![],
            updated_at: 0,
            note: None,
            bell: false,
            added_at: added,
            last_read_at: 0,
            shelf: None,
        }
    }

    #[test]
    fn an_id_naming_another_series_is_told() {
        let dir = std::env::temp_dir().join(format!("leaf-known-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut k = Known::load(&dir);
        // The copy on disk names what was kept; the server's answer names another.
        assert!(!k.see(&s(172, "Invincible Romance", 10), false));
        assert!(!k.see(&s(172, "Invincible Romance", 10), true), "the same one");
        assert!(!k.see(&s(172, "Invincible Romance!", 10), false), "a copy on disk never tells");
        assert!(k.see(&s(172, "The Awakening Contract", 20), true));
        assert!(!k.see(&s(172, "The Awakening Contract", 20), true), "told once");
        // Same title, added again later: another series all the same.
        assert!(k.see(&s(172, "The Awakening Contract", 30), true));
        // Unknown time first: learnt, not told.
        assert!(!k.see(&s(5, "Nano Machine", 0), true));
        assert!(!k.see(&s(5, "Nano Machine", 40), true));
        assert!(!k.see(&s(-3, "A preview", 0), true), "previews are not kept by id");
        k.save();
        let again = Known::load(&dir);
        assert_eq!(again.map.get(&172).map(|n| n.added), Some(30), "kept on disk");
        assert!(orphan_id(172) < -(1 << 39));
        let _ = std::fs::remove_dir_all(dir);
    }
}
