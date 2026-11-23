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
";

const WORK_COLS: &str = "id, title, manga_id, source, followed_at, fails, checked_at, compared_at, latest, updated_at, note";

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
        self.db
            .execute("UPDATE work SET manga_id = ?2, source = ?3, fails = 0, compared_at = ?4, note = NULL WHERE id = ?1", params![id, manga_id, source, now])
            .map(|_| ())
            .map_err(|e| e.to_string())
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

    pub fn compared(&self, id: i64, now: i64) {
        let _ = self.db.execute("UPDATE work SET compared_at = ?2 WHERE id = ?1", params![id, now]);
    }

    pub fn set_note(&self, id: i64, note: Option<&str>) {
        let _ = self.db.execute("UPDATE work SET note = ?2 WHERE id = ?1", params![id, note]);
    }

    pub fn event(&self, at: i64, work: Option<i64>, kind: &str, text: &str) {
        let _ = self.db.execute("INSERT INTO event (at, work_id, kind, text) VALUES (?1, ?2, ?3, ?4)", params![at, work, kind, text]);
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
        assert_eq!(s.work_of_manga(9).unwrap().id, id);
        s.add_aliases(id, &["Became the King by Scavenging".into(), "Became the King by Scavenging".into()]);
        assert_eq!(s.aliases(id), vec!["Became the King by Scavenging"]);
        assert!(s.work_of_manga(1).is_none());
        s.checked(id, true, Some(60.0), 500);
        s.checked(id, true, Some(58.0), 600);
        let w = s.work(id).unwrap();
        assert_eq!((w.latest, w.updated_at), (60.0, 500), "only a newer chapter moves updated_at");
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
