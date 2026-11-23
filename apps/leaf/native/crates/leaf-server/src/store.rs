//! leaf-server's own record: the series you follow (each pinned to one Suwayomi manga, which the
//! autopilot may change), what the autopilot did, and a few timestamps. Everything about chapters
//! and reading lives in Suwayomi.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

pub struct Store {
    db: Connection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Work {
    pub id: i64,
    pub title: String,
    pub manga_id: i64,
    pub source: String,
    pub followed_at: i64,
    /// Failed refreshes in a row on the current source.
    pub fails: u32,
    pub checked_at: i64,
    /// When the autopilot last looked for a better source.
    pub compared_at: i64,
    /// The highest chapter number seen, to tell new chapters apart.
    pub latest: f64,
    pub updated_at: i64,
    pub note: Option<String>,
    /// New chapters are announced.
    pub bell: bool,
    /// The shelf chosen for it ("reading", "plan", "hold", "dropped", "done").
    pub shelf: Option<String>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS work (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    manga_id INTEGER NOT NULL UNIQUE,
    source TEXT NOT NULL,
    followed_at INTEGER NOT NULL,
    fails INTEGER NOT NULL DEFAULT 0,
    checked_at INTEGER NOT NULL DEFAULT 0,
    compared_at INTEGER NOT NULL DEFAULT 0,
    latest REAL NOT NULL DEFAULT -1,
    updated_at INTEGER NOT NULL DEFAULT 0,
    note TEXT
);
CREATE TABLE IF NOT EXISTS event (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    work_id INTEGER,
    kind TEXT NOT NULL,
    text TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS alias (work_id INTEGER NOT NULL, title TEXT NOT NULL, UNIQUE(work_id, title));
CREATE TABLE IF NOT EXISTS link (work_id INTEGER NOT NULL, manga_id INTEGER NOT NULL UNIQUE, source TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS arrival (work_id INTEGER NOT NULL, number REAL NOT NULL, at INTEGER NOT NULL, UNIQUE(work_id, number));
CREATE TABLE IF NOT EXISTS source_health (name TEXT PRIMARY KEY, ok_at INTEGER NOT NULL DEFAULT 0, err_at INTEGER NOT NULL DEFAULT 0, err TEXT);
CREATE TABLE IF NOT EXISTS dl_queue (seq INTEGER PRIMARY KEY, chapter_id INTEGER NOT NULL UNIQUE, source TEXT NOT NULL, at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS rec_hidden (anilist_id INTEGER PRIMARY KEY, at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS rec_tag (tag TEXT PRIMARY KEY, less INTEGER NOT NULL);
";

const WORK_COLS: &str = "id, title, manga_id, source, followed_at, fails, checked_at, compared_at, latest, updated_at, note, bell, shelf";

fn work_of(r: &rusqlite::Row) -> rusqlite::Result<Work> {
    Ok(Work {
        id: r.get(0)?,
        title: r.get(1)?,
        manga_id: r.get(2)?,
        source: r.get(3)?,
        followed_at: r.get(4)?,
        fails: r.get(5)?,
        checked_at: r.get(6)?,
        compared_at: r.get(7)?,
        latest: r.get(8)?,
        updated_at: r.get(9)?,
        note: r.get(10)?,
        bell: r.get::<_, i64>(11)? != 0,
        shelf: r.get(12)?,
    })
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, String> {
        let db = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Store::with(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store::with(Connection::open_in_memory().expect("sqlite in memory")).expect("the schema")
    }

    fn with(db: Connection) -> Result<Store, String> {
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").map_err(|e| e.to_string())?;
        db.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        // Added after the first deploy: a store from before has no bell column yet.
        let has_bell: bool = db.prepare("SELECT 1 FROM pragma_table_info('work') WHERE name = 'bell'").and_then(|mut s| s.exists([])).unwrap_or(false);
        if !has_bell {
            db.execute_batch("ALTER TABLE work ADD COLUMN bell INTEGER NOT NULL DEFAULT 0").map_err(|e| e.to_string())?;
        }
        let has_shelf: bool = db.prepare("SELECT 1 FROM pragma_table_info('work') WHERE name = 'shelf'").and_then(|mut s| s.exists([])).unwrap_or(false);
        if !has_shelf {
            db.execute_batch("ALTER TABLE work ADD COLUMN shelf TEXT").map_err(|e| e.to_string())?;
        }
        Ok(Store { db })
    }

    pub fn works(&self) -> Vec<Work> {
        let mut st = self.db.prepare_cached(&format!("SELECT {WORK_COLS} FROM work ORDER BY updated_at DESC, id DESC")).expect("a statement");
        st.query_map([], work_of).expect("works").filter_map(Result::ok).collect()
    }

    pub fn work(&self, id: i64) -> Option<Work> {
        self.db.prepare_cached(&format!("SELECT {WORK_COLS} FROM work WHERE id = ?1")).ok()?.query_row([id], work_of).optional().ok()?
    }

    pub fn work_of_manga(&self, manga_id: i64) -> Option<Work> {
        self.db.prepare_cached(&format!("SELECT {WORK_COLS} FROM work WHERE manga_id = ?1")).ok()?.query_row([manga_id], work_of).optional().ok()?
    }

    pub fn add_work(&self, title: &str, manga_id: i64, source: &str, latest: f64, now: i64) -> Result<i64, String> {
        self.db
            .execute(
                "INSERT INTO work (title, manga_id, source, followed_at, checked_at, compared_at, latest, updated_at) VALUES (?1, ?2, ?3, ?4, ?4, ?4, ?5, ?4)",
                params![title, manga_id, source, now, latest],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn remove_work(&self, id: i64) {
        let _ = self.db.execute("DELETE FROM work WHERE id = ?1", [id]);
        let _ = self.db.execute("DELETE FROM alias WHERE work_id = ?1", [id]);
        let _ = self.db.execute("DELETE FROM link WHERE work_id = ?1", [id]);
    }

    /// The same work on another source, kept to fill the chapters the work's own source lacks.
    /// Nothing for a work unfollowed meanwhile, or for another work's own copy.
    pub fn add_link(&self, id: i64, manga_id: i64, source: &str) {
        let _ = self.db.execute(
            "INSERT INTO link (work_id, manga_id, source) SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM work WHERE id = ?1) AND NOT EXISTS (SELECT 1 FROM work WHERE manga_id = ?2 AND id != ?1)
             ON CONFLICT(manga_id) DO UPDATE SET work_id = excluded.work_id, source = excluded.source",
            params![id, manga_id, source],
        );
    }

    pub fn remove_link(&self, manga_id: i64) {
        let _ = self.db.execute("DELETE FROM link WHERE manga_id = ?1", [manga_id]);
    }

    /// The linked copies' Suwayomi manga ids.
    pub fn links(&self, id: i64) -> Vec<i64> {
        let mut st = self.db.prepare_cached("SELECT manga_id FROM link WHERE work_id = ?1 ORDER BY rowid").expect("a statement");
        st.query_map([id], |r| r.get(0)).expect("links").filter_map(Result::ok).collect()
    }

    /// The work a manga belongs to, as its own copy or a linked one.
    pub fn work_of_any(&self, manga_id: i64) -> Option<Work> {
        if let Some(w) = self.work_of_manga(manga_id) {
            return Some(w);
        }
        let id: i64 = self.db.prepare_cached("SELECT work_id FROM link WHERE manga_id = ?1").ok()?.query_row([manga_id], |r| r.get(0)).optional().ok()??;
        self.work(id)
    }

    /// Other titles the work goes by on other sources.
    pub fn add_aliases(&self, id: i64, titles: &[String]) {
        for t in titles {
            let _ = self.db.execute("INSERT OR IGNORE INTO alias (work_id, title) VALUES (?1, ?2)", params![id, t]);
        }
    }

    pub fn aliases(&self, id: i64) -> Vec<String> {
        let mut st = self.db.prepare_cached("SELECT title FROM alias WHERE work_id = ?1 ORDER BY rowid").expect("a statement");
        st.query_map([id], |r| r.get(0)).expect("aliases").filter_map(Result::ok).collect()
    }

    pub fn move_work(&self, id: i64, manga_id: i64, source: &str, now: i64) -> Result<(), String> {
        match self.db.execute("UPDATE work SET manga_id = ?2, source = ?3, fails = 0, compared_at = ?4, note = NULL WHERE id = ?1", params![id, manga_id, source, now]) {
            Ok(0) => Err("the series is no longer followed".into()),
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// The latest chapter raised without it counting as an update (chapters filled in, not new).
    pub fn raise_latest(&self, id: i64, latest: f64) {
        let _ = self.db.execute("UPDATE work SET latest = ?2 WHERE id = ?1 AND latest < ?2", params![id, latest]);
    }

    pub fn checked(&self, id: i64, ok: bool, latest: Option<f64>, now: i64) {
        let _ = if ok {
            self.db.execute("UPDATE work SET fails = 0, checked_at = ?2 WHERE id = ?1", params![id, now])
        } else {
            self.db.execute("UPDATE work SET fails = fails + 1, checked_at = ?2 WHERE id = ?1", params![id, now])
        };
        if let Some(l) = latest {
            let _ = self.db.execute("UPDATE work SET latest = ?2, updated_at = ?3 WHERE id = ?1 AND latest < ?2", params![id, l, now]);
        }
    }

    pub fn set_shelf(&self, id: i64, shelf: Option<&str>) {
        let _ = self.db.execute("UPDATE work SET shelf = ?2 WHERE id = ?1", params![id, shelf]);
    }

    /// Chapters that arrived for a work (its latest number grew), for the Updates list.
    pub fn add_arrivals(&self, id: i64, numbers: &[f64], at: i64) {
        for n in numbers {
            let _ = self.db.execute("INSERT OR IGNORE INTO arrival (work_id, number, at) VALUES (?1, ?2, ?3)", params![id, n, at]);
        }
        let _ = self.db.execute("DELETE FROM arrival WHERE at < ?1", [at - 60 * 86_400_000]);
    }

    /// (work, number, at) since `since`, newest first.
    pub fn arrivals_since(&self, since: i64) -> Vec<(i64, f64, i64)> {
        let mut st = self.db.prepare_cached("SELECT work_id, number, at FROM arrival WHERE at >= ?1 ORDER BY at DESC, number DESC LIMIT 1000").expect("a statement");
        st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).expect("arrivals").filter_map(Result::ok).collect()
    }

    pub fn source_ok(&self, name: &str, at: i64) {
        let _ = self.db.execute("INSERT INTO source_health (name, ok_at) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET ok_at = excluded.ok_at", params![name, at]);
    }

    pub fn source_err(&self, name: &str, err: &str, at: i64) {
        let _ = self.db.execute(
            "INSERT INTO source_health (name, err_at, err) VALUES (?1, ?2, ?3) ON CONFLICT(name) DO UPDATE SET err_at = excluded.err_at, err = excluded.err",
            params![name, at, err],
        );
    }

    /// name → (last ok, last error time, last error).
    pub fn health(&self) -> Vec<(String, i64, i64, Option<String>)> {
        let mut st = self.db.prepare_cached("SELECT name, ok_at, err_at, err FROM source_health").expect("a statement");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).expect("health").filter_map(Result::ok).collect()
    }

    pub fn set_bell(&self, id: i64, on: bool) {
        let _ = self.db.execute("UPDATE work SET bell = ?2 WHERE id = ?1", params![id, on as i64]);
    }

    /// Events after `since` (ms since the epoch), oldest first: what the phone has not shown yet.
    pub fn events_since(&self, since: i64) -> Vec<leaf_proto::Event> {
        let mut st = self.db.prepare_cached("SELECT at, work_id, kind, text FROM event WHERE at > ?1 ORDER BY id LIMIT 100").expect("a statement");
        st.query_map([since], |r| Ok(leaf_proto::Event { at: r.get(0)?, series: r.get(1)?, kind: r.get(2)?, text: r.get(3)? }))
            .expect("events")
            .filter_map(Result::ok)
            .collect()
    }

    pub fn compared(&self, id: i64, now: i64) {
        let _ = self.db.execute("UPDATE work SET compared_at = ?2 WHERE id = ?1", params![id, now]);
    }

    pub fn set_note(&self, id: i64, note: Option<&str>) {
        let _ = self.db.execute("UPDATE work SET note = ?2 WHERE id = ?1", params![id, note]);
    }

    /// An event, stamped after every one before it: the phone reads `?since=<the last at it saw>`,
    /// so one written late with an earlier moment (a check that took minutes) would be skipped.
    pub fn event(&self, at: i64, work: Option<i64>, kind: &str, text: &str) {
        let _ = self.db.execute(
            "INSERT INTO event (at, work_id, kind, text) VALUES (MAX(?1, ?5, COALESCE((SELECT MAX(at) FROM event), 0) + 1), ?2, ?3, ?4)",
            params![at, work, kind, text, crate::autopilot::now_ms()],
        );
        let _ = self.db.execute("DELETE FROM event WHERE id NOT IN (SELECT id FROM event ORDER BY id DESC LIMIT 500)", []);
    }

    pub fn events(&self, limit: u32) -> Vec<leaf_proto::Event> {
        let mut st = self.db.prepare_cached("SELECT at, work_id, kind, text FROM event ORDER BY id DESC LIMIT ?1").expect("a statement");
        st.query_map([limit], |r| Ok(leaf_proto::Event { at: r.get(0)?, series: r.get(1)?, kind: r.get(2)?, text: r.get(3)? }))
            .expect("events")
            .filter_map(Result::ok)
            .collect()
    }

    pub fn get(&self, k: &str) -> Option<String> {
        self.db.prepare_cached("SELECT v FROM kv WHERE k = ?1").ok()?.query_row([k], |r| r.get(0)).optional().ok()?
    }

    pub fn set(&self, k: &str, v: &str) {
        let _ = self.db.execute("INSERT INTO kv (k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v", params![k, v]);
    }

    pub fn del(&self, k: &str) {
        let _ = self.db.execute("DELETE FROM kv WHERE k = ?1", [k]);
    }

    /// Every (key, value) whose key starts with `prefix`.
    pub fn with_prefix(&self, prefix: &str) -> Vec<(String, String)> {
        let mut st = self.db.prepare_cached("SELECT k, v FROM kv WHERE substr(k, 1, length(?1)) = ?1 ORDER BY k").expect("a statement");
        st.query_map([prefix], |r| Ok((r.get(0)?, r.get(1)?))).expect("kv").filter_map(Result::ok).collect()
    }

    /// The linked copies with their sources.
    pub fn link_sources(&self, id: i64) -> Vec<(i64, String)> {
        let mut st = self.db.prepare_cached("SELECT manga_id, source FROM link WHERE work_id = ?1 ORDER BY rowid").expect("a statement");
        st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?))).expect("links").filter_map(Result::ok).collect()
    }

    /// Chapters to hand to Suwayomi's downloader as their source's budget allows; one asked
    /// again keeps its place in line.
    pub fn queue_downloads(&self, chapters: &[(i64, String)], at: i64) {
        for (id, source) in chapters {
            let _ = self.db.execute("INSERT OR IGNORE INTO dl_queue (chapter_id, source, at) VALUES (?1, ?2, ?3)", params![id, source, at]);
        }
    }

    pub fn unqueue_downloads(&self, ids: &[i64]) {
        for id in ids {
            let _ = self.db.execute("DELETE FROM dl_queue WHERE chapter_id = ?1", [id]);
        }
    }

    /// (chapter, source), first asked first.
    pub fn queued_downloads(&self) -> Vec<(i64, String)> {
        let mut st = self.db.prepare_cached("SELECT chapter_id, source FROM dl_queue ORDER BY seq").expect("a statement");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).expect("queue").filter_map(Result::ok).collect()
    }

    /// "Not interested": kept for good.
    pub fn hide_rec(&self, anilist_id: i64, at: i64) {
        let _ = self.db.execute("INSERT OR IGNORE INTO rec_hidden (anilist_id, at) VALUES (?1, ?2)", params![anilist_id, at]);
    }

    pub fn hidden_recs(&self) -> std::collections::HashSet<i64> {
        let mut st = self.db.prepare_cached("SELECT anilist_id FROM rec_hidden").expect("a statement");
        st.query_map([], |r| r.get(0)).expect("hidden").filter_map(Result::ok).collect()
    }

    /// Fewer recommendations with this tag (`less`), or as before.
    pub fn set_rec_tag(&self, tag: &str, less: bool) {
        let tag = tag.trim().to_lowercase();
        let _ = if less {
            self.db.execute("INSERT INTO rec_tag (tag, less) VALUES (?1, 1) ON CONFLICT(tag) DO UPDATE SET less = 1", [&tag])
        } else {
            self.db.execute("DELETE FROM rec_tag WHERE tag = ?1", [&tag])
        };
    }

    /// The tags marked "fewer like this", lower case.
    pub fn less_tags(&self) -> std::collections::HashSet<String> {
        let mut st = self.db.prepare_cached("SELECT tag FROM rec_tag WHERE less = 1").expect("a statement");
        st.query_map([], |r| r.get(0)).expect("tags").filter_map(Result::ok).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_moves_and_keeps_its_id() {
        let s = Store::in_memory();
        let id = s.add_work("Junk to King", 1, "MangaDex", 54.0, 100).unwrap();
        assert!(s.add_work("Junk to King", 1, "MangaDex", 54.0, 100).is_err(), "one work per manga");
        s.checked(id, false, None, 200);
        s.checked(id, false, None, 300);
        assert_eq!(s.work(id).unwrap().fails, 2);
        s.move_work(id, 9, "Asura Scans", 400).unwrap();
        let w = s.work(id).unwrap();
        assert_eq!((w.manga_id, w.source.as_str(), w.fails), (9, "Asura Scans", 0));
        assert!(!w.bell, "adding is not following");
        assert_eq!(w.shelf, None);
        s.set_shelf(id, Some("hold"));
        assert_eq!(s.work(id).unwrap().shelf.as_deref(), Some("hold"));
        s.add_arrivals(id, &[61.0, 62.0], 1000);
        s.add_arrivals(id, &[62.0], 2000);
        assert_eq!(s.arrivals_since(0), vec![(id, 62.0, 1000), (id, 61.0, 1000)], "a chapter arrives once");
        s.source_err("Asura Scans", "Cloudflare bypass currently disabled", 10);
        s.source_ok("Asura Scans", 20);
        assert_eq!(s.health(), vec![("Asura Scans".to_string(), 20, 10, Some("Cloudflare bypass currently disabled".to_string()))]);
        s.set_bell(id, true);
        assert!(s.work(id).unwrap().bell);
        assert_eq!(s.work_of_manga(9).unwrap().id, id);
        s.add_aliases(id, &["Became the King by Scavenging".into(), "Became the King by Scavenging".into()]);
        assert_eq!(s.aliases(id), vec!["Became the King by Scavenging"]);
        s.add_link(id, 12, "Drake Scans");
        s.add_link(id, 12, "Drake Scans");
        assert_eq!(s.links(id), vec![12]);
        assert_eq!(s.work_of_any(12).unwrap().id, id);
        assert_eq!(s.work_of_any(9).unwrap().id, id);
        s.remove_link(12);
        assert!(s.work_of_any(12).is_none());
        assert!(s.work_of_manga(1).is_none());
        s.checked(id, true, Some(60.0), 500);
        s.checked(id, true, Some(58.0), 600);
        let w = s.work(id).unwrap();
        assert_eq!((w.latest, w.updated_at), (60.0, 500), "only a newer chapter moves updated_at");
    }

    #[test]
    fn links_need_a_followed_work_and_moves_a_followed_one() {
        let s = Store::in_memory();
        let id = s.add_work("Junk to King", 1, "MangaDex", 54.0, 100).unwrap();
        let other = s.add_work("Nano Machine", 2, "MangaDex", 10.0, 100).unwrap();
        s.add_link(99, 12, "Drake Scans");
        assert!(s.work_of_any(12).is_none(), "no link for a work that is gone");
        s.add_link(id, 2, "MangaDex");
        assert_eq!(s.links(id), Vec::<i64>::new(), "another work's own copy is not taken as a link");
        assert_eq!(s.work_of_any(2).unwrap().id, other);
        s.remove_work(id);
        assert!(s.move_work(id, 9, "Asura Scans", 400).is_err(), "an unfollowed work does not move");
    }

    #[test]
    fn events_stamped_late_are_not_skipped() {
        let s = Store::in_memory();
        s.event(5_000, None, "info", "a");
        let seen = s.events(10)[0].at;
        // A check that began earlier writes its event now: it still comes after what was seen.
        s.event(1_000, Some(1), "new", "b");
        let after = s.events_since(seen);
        assert_eq!(after.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), vec!["b"]);
    }

    #[test]
    fn downloads_wait_in_line_and_recs_keep_what_you_said() {
        let s = Store::in_memory();
        s.queue_downloads(&[(7, "Asura Scans".into()), (3, "MangaDex".into())], 10);
        s.queue_downloads(&[(7, "Asura Scans".into()), (9, "Asura Scans".into())], 20);
        assert_eq!(s.queued_downloads(), vec![(7, "Asura Scans".to_string()), (3, "MangaDex".to_string()), (9, "Asura Scans".to_string())], "asked again keeps its place");
        s.unqueue_downloads(&[7, 3]);
        assert_eq!(s.queued_downloads(), vec![(9, "Asura Scans".to_string())]);
        s.hide_rec(30, 1);
        s.hide_rec(30, 2);
        assert_eq!(s.hidden_recs().into_iter().collect::<Vec<_>>(), vec![30]);
        s.set_rec_tag(" Isekai ", true);
        s.set_rec_tag("Murim", true);
        s.set_rec_tag("murim", false);
        assert_eq!(s.less_tags().into_iter().collect::<Vec<_>>(), vec!["isekai".to_string()]);
        s.set("pause:A", "1");
        s.set("pause:B", "2");
        s.set("paused", "3");
        assert_eq!(s.with_prefix("pause:").len(), 2);
        s.del("pause:A");
        assert_eq!(s.get("pause:A"), None);
    }

    #[test]
    fn events_newest_first() {
        let s = Store::in_memory();
        s.event(1, None, "info", "a");
        s.event(2, Some(3), "switched", "b");
        let e = s.events(10);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].text.as_str(), e[0].series), ("b", Some(3)));
    }
}
