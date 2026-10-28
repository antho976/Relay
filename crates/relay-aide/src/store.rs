//! `aide.db` beside the store: Aide's own SQLite file, as Tally's ledger and Avex's history are.
//! Small rows, written a few at a time; the engine keeps it behind one mutex and never holds the
//! store's lock with it.
//!
//! A row is **private** when a model of the person's own wrote it while some source was for
//! their own models only: it may hold what that model read. The lists take `with_private`, and
//! what Claude's models are handed leaves private rows out.

use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

pub type Result<T> = rusqlite::Result<T>;

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE job (
    id INTEGER PRIMARY KEY,
    spec TEXT NOT NULL,
    state TEXT NOT NULL,
    thread_id INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_run_at TEXT,
    next_due_at TEXT
);
CREATE TABLE run (
    id INTEGER PRIMARY KEY,
    job_id INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    trigger TEXT NOT NULL,
    state TEXT NOT NULL,
    model TEXT,
    host TEXT,
    note TEXT,
    thread_id INTEGER,
    sources TEXT NOT NULL,
    started_at TEXT NOT NULL,
    deadline_at TEXT,
    ended_at TEXT
);
CREATE INDEX run_job ON run(job_id, id);
CREATE INDEX run_state ON run(state);
CREATE TABLE fact (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    source TEXT NOT NULL,
    job_id INTEGER REFERENCES job(id) ON DELETE SET NULL,
    confirmed INTEGER NOT NULL,
    pinned INTEGER NOT NULL,
    learned_at TEXT NOT NULL,
    used_at TEXT
);
CREATE INDEX fact_job ON fact(job_id);
CREATE TABLE errand (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    detail TEXT NOT NULL,
    job_id INTEGER REFERENCES job(id) ON DELETE SET NULL,
    state TEXT NOT NULL,
    due_at TEXT,
    proposed_done INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT
);
CREATE INDEX errand_state ON errand(state);
CREATE TABLE message (
    id INTEGER PRIMARY KEY,
    job_id INTEGER REFERENCES job(id) ON DELETE SET NULL,
    run_id INTEGER REFERENCES run(id) ON DELETE SET NULL,
    level TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    notified INTEGER NOT NULL,
    held INTEGER NOT NULL,
    briefed_at TEXT
);
CREATE INDEX message_held ON message(held);
CREATE TABLE follow_up (
    id INTEGER PRIMARY KEY,
    job_id INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
    due_at TEXT NOT NULL,
    note TEXT NOT NULL
);
CREATE INDEX follow_up_due ON follow_up(due_at);
CREATE TABLE page (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL UNIQUE,
    question TEXT NOT NULL,
    body TEXT NOT NULL,
    job_id INTEGER REFERENCES job(id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE seen (
    job_id INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    at TEXT NOT NULL,
    PRIMARY KEY (job_id, key)
);
"#, r#"
ALTER TABLE job ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE fact ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE errand ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE message ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE page ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
-- Kept before rows were marked: what a job that ever ran on the person's own model wrote is
-- taken as private.
UPDATE fact SET private = 1 WHERE job_id IN (SELECT job_id FROM run WHERE host IN ('pc', 'server'));
UPDATE errand SET private = 1 WHERE job_id IN (SELECT job_id FROM run WHERE host IN ('pc', 'server'));
UPDATE message SET private = 1 WHERE job_id IN (SELECT job_id FROM run WHERE host IN ('pc', 'server'));
UPDATE page SET private = 1 WHERE job_id IN (SELECT job_id FROM run WHERE host IN ('pc', 'server'));
"#];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// How many runs a job keeps; older ones go as new ones start.
pub const KEEP_RUNS: i64 = 500;

pub struct Aide {
    conn: Connection,
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("aide rows serialize")
}

fn parsed<T: serde::de::DeserializeOwned>(text: &str, col: usize) -> rusqlite::Result<T> {
    serde_json::from_str(text).map_err(|e| rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e)))
}

fn named<T>(text: &str, col: usize, parse: fn(&str) -> Option<T>) -> rusqlite::Result<T> {
    parse(text).ok_or_else(|| rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, format!("unknown value {text:?}").into()))
}

const JOB: &str = "id, spec, state, thread_id, created_at, updated_at, last_run_at, next_due_at";
fn job(r: &Row) -> rusqlite::Result<Job> {
    Ok(Job {
        id: r.get(0)?,
        spec: parsed(&r.get::<_, String>(1)?, 1)?,
        state: named(&r.get::<_, String>(2)?, 2, JobState::parse)?,
        thread_id: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        last_run_at: r.get(6)?,
        next_due_at: r.get(7)?,
    })
}

const RUN: &str = "r.id, r.job_id, j.spec, r.reason, r.trigger, r.state, r.model, r.host, r.note, r.thread_id, r.sources, r.started_at, r.deadline_at, r.ended_at";
fn run(r: &Row) -> rusqlite::Result<Run> {
    let spec: JobSpec = parsed(&r.get::<_, String>(2)?, 2)?;
    Ok(Run {
        id: r.get(0)?,
        job_id: r.get(1)?,
        job_name: spec.name,
        reason: named(&r.get::<_, String>(3)?, 3, Reason::parse)?,
        trigger: r.get(4)?,
        state: named(&r.get::<_, String>(5)?, 5, RunState::parse)?,
        model: r.get(6)?,
        host: r.get(7)?,
        note: r.get(8)?,
        thread_id: r.get(9)?,
        sources: parsed(&r.get::<_, String>(10)?, 10)?,
        started_at: r.get(11)?,
        deadline_at: r.get(12)?,
        ended_at: r.get(13)?,
    })
}

const FACT: &str = "id, text, source, job_id, confirmed, pinned, learned_at, used_at";
fn fact(r: &Row) -> rusqlite::Result<Fact> {
    Ok(Fact {
        id: r.get(0)?,
        text: r.get(1)?,
        source: r.get(2)?,
        job_id: r.get(3)?,
        confirmed: r.get(4)?,
        pinned: r.get(5)?,
        learned_at: r.get(6)?,
        used_at: r.get(7)?,
    })
}

const ERRAND: &str = "id, title, detail, job_id, state, due_at, proposed_done, created_at, updated_at, closed_at";
fn errand(r: &Row) -> rusqlite::Result<Errand> {
    Ok(Errand {
        id: r.get(0)?,
        title: r.get(1)?,
        detail: r.get(2)?,
        job_id: r.get(3)?,
        state: named(&r.get::<_, String>(4)?, 4, ErrandState::parse)?,
        due_at: r.get(5)?,
        proposed_done: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        closed_at: r.get(9)?,
    })
}

const MESSAGE: &str = "id, job_id, run_id, level, title, body, created_at, notified, held, briefed_at";
fn message(r: &Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: r.get(0)?,
        job_id: r.get(1)?,
        run_id: r.get(2)?,
        level: named(&r.get::<_, String>(3)?, 3, Level::parse)?,
        title: r.get(4)?,
        body: r.get(5)?,
        created_at: r.get(6)?,
        notified: r.get(7)?,
        held: r.get(8)?,
        briefed_at: r.get(9)?,
    })
}

const PAGE: &str = "id, title, question, body, job_id, updated_at";
fn page(r: &Row) -> rusqlite::Result<Page> {
    Ok(Page { id: r.get(0)?, title: r.get(1)?, question: r.get(2)?, body: r.get(3)?, job_id: r.get(4)?, updated_at: r.get(5)? })
}

/// A message to keep: [`Aide::add_message`].
pub struct NewMessage<'a> {
    pub job_id: Option<i64>,
    pub run_id: Option<i64>,
    pub level: Level,
    pub title: &'a str,
    pub body: &'a str,
    pub notified: bool,
    pub held: bool,
    /// Written from private data (the module's head).
    pub private: bool,
}

impl Aide {
    pub fn open(path: &Path) -> Result<Aide> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Aide::init(conn)
    }

    pub fn open_in_memory() -> Result<Aide> {
        Aide::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Aide> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let have: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if have > SCHEMA_VERSION {
            return Err(rusqlite::Error::InvalidParameterName(format!("aide.db is version {have}; this build reads up to {SCHEMA_VERSION}")));
        }
        let tx = conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(have as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        Ok(Aide { conn })
    }

    // Meta: settings and the engine's last sign of life.

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        self.conn.prepare_cached("SELECT value FROM meta WHERE key = ?1")?.query_row([key], |r| r.get(0)).optional()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.prepare_cached("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?.execute([key, value])?;
        Ok(())
    }

    /// The settings; the defaults until any are saved, and for a file written by a newer build.
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.meta("settings")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
    }

    pub fn set_settings(&self, s: &Settings) -> Result<()> {
        self.set_meta("settings", &json(s))
    }

    // Jobs.

    pub fn jobs(&self) -> Result<Vec<Job>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {JOB} FROM job ORDER BY id"))?;
        let rows = st.query_map([], job)?;
        rows.collect()
    }

    pub fn job(&self, id: i64) -> Result<Option<Job>> {
        self.conn.prepare_cached(&format!("SELECT {JOB} FROM job WHERE id = ?1"))?.query_row([id], job).optional()
    }

    /// Jobs, leaving out a proposal written from private data unless `with_private`: a job the
    /// person turned on is theirs, whoever drafted it.
    pub fn jobs_listed(&self, with_private: bool) -> Result<Vec<Job>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {JOB} FROM job WHERE ?1 OR private = 0 OR state != 'proposed' ORDER BY id"))?;
        let rows = st.query_map([with_private], job)?;
        rows.collect()
    }

    pub fn add_job(&self, spec: &JobSpec, state: JobState, next_due_at: Option<&str>, private: bool, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached("INSERT INTO job (spec, state, created_at, updated_at, next_due_at, private) VALUES (?1, ?2, ?3, ?3, ?4, ?5)")?
            .execute(params![json(spec), state.as_str(), now, next_due_at, private])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Rewrites a job; written from private data, it stays private.
    pub fn update_job(&self, id: i64, spec: &JobSpec, next_due_at: Option<&str>, private: bool, now: &str) -> Result<bool> {
        Ok(self.conn.prepare_cached("UPDATE job SET spec = ?2, next_due_at = ?3, updated_at = ?4, private = private OR ?5 WHERE id = ?1")?
            .execute(params![id, json(spec), next_due_at, now, private])? > 0)
    }

    pub fn set_job_state(&self, id: i64, state: JobState, next_due_at: Option<&str>, now: &str) -> Result<bool> {
        Ok(self.conn.prepare_cached("UPDATE job SET state = ?2, next_due_at = ?3, updated_at = ?4 WHERE id = ?1")?
            .execute(params![id, state.as_str(), next_due_at, now])? > 0)
    }

    pub fn set_job_due(&self, id: i64, next_due_at: Option<&str>) -> Result<()> {
        self.conn.prepare_cached("UPDATE job SET next_due_at = ?2 WHERE id = ?1")?.execute(params![id, next_due_at])?;
        Ok(())
    }

    pub fn set_job_thread(&self, id: i64, thread_id: i64) -> Result<()> {
        self.conn.prepare_cached("UPDATE job SET thread_id = ?2 WHERE id = ?1")?.execute(params![id, thread_id])?;
        Ok(())
    }

    pub fn delete_job(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("DELETE FROM job WHERE id = ?1")?.execute([id])? > 0)
    }

    /// The job whose channel is `thread_id`.
    pub fn job_of_thread(&self, thread_id: i64) -> Result<Option<Job>> {
        self.conn.prepare_cached(&format!("SELECT {JOB} FROM job WHERE thread_id = ?1"))?.query_row([thread_id], job).optional()
    }

    // Runs.

    #[allow(clippy::too_many_arguments)]
    pub fn add_run(&self, job_id: i64, reason: Reason, trigger: &str, state: RunState, note: Option<&str>, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached("INSERT INTO run (job_id, reason, trigger, state, note, sources, started_at) VALUES (?1, ?2, ?3, ?4, ?5, '[]', ?6)")?
            .execute(params![job_id, reason.as_str(), trigger, state.as_str(), note, now])?;
        let id = self.conn.last_insert_rowid();
        self.conn.prepare_cached("UPDATE job SET last_run_at = ?2 WHERE id = ?1")?.execute(params![job_id, now])?;
        self.conn
            .prepare_cached("DELETE FROM run WHERE job_id = ?1 AND id <= (SELECT id FROM run WHERE job_id = ?1 ORDER BY id DESC LIMIT 1 OFFSET ?2)")?
            .execute(params![job_id, KEEP_RUNS])?;
        Ok(id)
    }

    pub fn run(&self, id: i64) -> Result<Option<Run>> {
        self.conn.prepare_cached(&format!("SELECT {RUN} FROM run r JOIN job j ON j.id = r.job_id WHERE r.id = ?1"))?.query_row([id], run).optional()
    }

    /// Runs, newest first; of one job when given.
    pub fn runs(&self, job_id: Option<i64>, limit: u32) -> Result<Vec<Run>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {RUN} FROM run r JOIN job j ON j.id = r.job_id WHERE (?1 IS NULL OR r.job_id = ?1) ORDER BY r.id DESC LIMIT ?2"
        ))?;
        let rows = st.query_map(params![job_id, limit], run)?;
        rows.collect()
    }

    /// Runs still queued or running.
    pub fn open_runs(&self) -> Result<Vec<Run>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {RUN} FROM run r JOIN job j ON j.id = r.job_id WHERE r.state IN ('queued', 'running') ORDER BY r.id"))?;
        let rows = st.query_map([], run)?;
        rows.collect()
    }

    /// How many runs of the job started at or after `since`, skipped ones left out.
    pub fn runs_since(&self, job_id: i64, since: &str) -> Result<u32> {
        self.conn.prepare_cached("SELECT COUNT(*) FROM run WHERE job_id = ?1 AND started_at >= ?2 AND state != 'skipped'")?
            .query_row(params![job_id, since], |r| r.get(0))
    }

    /// Claims a queued run as running; whether it was still queued.
    pub fn start_run(&self, id: i64, model: Option<&str>, host: &str, thread_id: i64, sources: &[SourceRead], deadline_at: &str) -> Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE run SET state = 'running', model = ?2, host = ?3, thread_id = ?4, sources = ?5, deadline_at = ?6 WHERE id = ?1 AND state = 'queued'")?
            .execute(params![id, model, host, thread_id, json(&sources), deadline_at])? > 0)
    }

    /// Ends the run, if it is still open; whether it was.
    pub fn end_run(&self, id: i64, state: RunState, note: Option<&str>, now: &str) -> Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE run SET state = ?2, note = COALESCE(?3, note), ended_at = ?4 WHERE id = ?1 AND state IN ('queued', 'running')")?
            .execute(params![id, state.as_str(), note, now])? > 0)
    }

    /// Sets how an ended run went, as the agent reports it: once the turn ended, Relay's own
    /// guess (done) gives way to what the agent said.
    pub fn report_run(&self, id: i64, state: RunState, note: Option<&str>) -> Result<bool> {
        Ok(self.conn.prepare_cached("UPDATE run SET state = ?2, note = COALESCE(?3, note) WHERE id = ?1")?.execute(params![id, state.as_str(), note])? > 0)
    }

    /// The open run talking in `thread_id`.
    pub fn open_run_of_thread(&self, thread_id: i64) -> Result<Option<Run>> {
        self.conn
            .prepare_cached(&format!("SELECT {RUN} FROM run r JOIN job j ON j.id = r.job_id WHERE r.thread_id = ?1 AND r.state IN ('queued', 'running') ORDER BY r.id DESC LIMIT 1"))?
            .query_row([thread_id], run)
            .optional()
    }

    /// The last run in `thread_id`, open or not.
    pub fn last_run_of_thread(&self, thread_id: i64) -> Result<Option<Run>> {
        self.conn
            .prepare_cached(&format!("SELECT {RUN} FROM run r JOIN job j ON j.id = r.job_id WHERE r.thread_id = ?1 ORDER BY r.id DESC LIMIT 1"))?
            .query_row([thread_id], run)
            .optional()
    }

    // Rule findings already acted on.

    /// Marks `key` seen for the job; whether it was new.
    pub fn see(&self, job_id: i64, key: &str, now: &str) -> Result<bool> {
        Ok(self.conn.prepare_cached("INSERT OR IGNORE INTO seen (job_id, key, at) VALUES (?1, ?2, ?3)")?.execute(params![job_id, key, now])? > 0)
    }

    pub fn forget_seen_before(&self, before: &str) -> Result<()> {
        self.conn.prepare_cached("DELETE FROM seen WHERE at < ?1")?.execute([before])?;
        Ok(())
    }

    /// Forgets the job's keys that start with `prefix`, but those in `keep`: a condition that
    /// lasts (a halt, an outage) is seen while it lasts, and is new again once it has passed.
    pub fn forget_seen_but(&self, job_id: i64, prefix: &str, keep: &[&str]) -> Result<()> {
        let keys: Vec<String> = {
            let mut st = self.conn.prepare_cached("SELECT key FROM seen WHERE job_id = ?1 AND substr(key, 1, length(?2)) = ?2")?;
            let rows = st.query_map(params![job_id, prefix], |r| r.get(0))?;
            rows.collect::<Result<_>>()?
        };
        let mut st = self.conn.prepare_cached("DELETE FROM seen WHERE job_id = ?1 AND key = ?2")?;
        for key in keys.iter().filter(|k| !keep.contains(&k.as_str())) {
            st.execute(params![job_id, key])?;
        }
        Ok(())
    }

    // Follow-ups.

    pub fn add_follow_up(&self, job_id: i64, due_at: &str, note: &str) -> Result<i64> {
        self.conn.prepare_cached("INSERT INTO follow_up (job_id, due_at, note) VALUES (?1, ?2, ?3)")?.execute(params![job_id, due_at, note])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn follow_ups(&self) -> Result<Vec<FollowUp>> {
        let mut st = self.conn.prepare_cached("SELECT id, job_id, due_at, note FROM follow_up ORDER BY due_at")?;
        let rows = st.query_map([], |r| Ok(FollowUp { id: r.get(0)?, job_id: r.get(1)?, due_at: r.get(2)?, note: r.get(3)? }))?;
        rows.collect()
    }

    pub fn follow_ups_of(&self, job_id: i64) -> Result<u32> {
        self.conn.prepare_cached("SELECT COUNT(*) FROM follow_up WHERE job_id = ?1")?.query_row([job_id], |r| r.get(0))
    }

    pub fn drop_follow_up(&self, id: i64) -> Result<()> {
        self.conn.prepare_cached("DELETE FROM follow_up WHERE id = ?1")?.execute([id])?;
        Ok(())
    }

    // Facts.

    pub fn facts(&self, with_private: bool) -> Result<Vec<Fact>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {FACT} FROM fact WHERE ?1 OR private = 0 ORDER BY pinned DESC, id DESC"))?;
        let rows = st.query_map([with_private], fact)?;
        rows.collect()
    }

    pub fn fact(&self, id: i64) -> Result<Option<Fact>> {
        self.conn.prepare_cached(&format!("SELECT {FACT} FROM fact WHERE id = ?1"))?.query_row([id], fact).optional()
    }

    pub fn add_fact(&self, text: &str, source: &str, job_id: Option<i64>, confirmed: bool, private: bool, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached("INSERT INTO fact (text, source, job_id, confirmed, pinned, learned_at, private) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)")?
            .execute(params![text, source, job_id, confirmed, now, private])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_fact(&self, id: i64, text: Option<&str>, confirmed: Option<bool>, pinned: Option<bool>) -> Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE fact SET text = COALESCE(?2, text), confirmed = COALESCE(?3, confirmed), pinned = COALESCE(?4, pinned) WHERE id = ?1")?
            .execute(params![id, text, confirmed, pinned])? > 0)
    }

    pub fn delete_fact(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("DELETE FROM fact WHERE id = ?1")?.execute([id])? > 0)
    }

    /// Forgets every fact learned from `source`; how many.
    pub fn forget_source(&self, source: &str) -> Result<usize> {
        self.conn.prepare_cached("DELETE FROM fact WHERE source = ?1")?.execute([source])
    }

    pub fn used_facts(&self, ids: &[i64], now: &str) -> Result<()> {
        let mut st = self.conn.prepare_cached("UPDATE fact SET used_at = ?2 WHERE id = ?1")?;
        for id in ids {
            st.execute(params![id, now])?;
        }
        Ok(())
    }

    // Errands.

    /// Errands, open and waiting first; closed ones only when `closed`.
    pub fn errands(&self, closed: bool, with_private: bool) -> Result<Vec<Errand>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ERRAND} FROM errand WHERE (?1 OR state IN ('open', 'waiting')) AND (?2 OR private = 0)
             ORDER BY state IN ('done', 'dropped'), CASE state WHEN 'waiting' THEN 0 ELSE 1 END, COALESCE(due_at, '9999'), id DESC LIMIT 500"
        ))?;
        let rows = st.query_map([closed, with_private], errand)?;
        rows.collect()
    }

    pub fn errand(&self, id: i64) -> Result<Option<Errand>> {
        self.conn.prepare_cached(&format!("SELECT {ERRAND} FROM errand WHERE id = ?1"))?.query_row([id], errand).optional()
    }

    pub fn errand_is_private(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("SELECT private FROM errand WHERE id = ?1")?.query_row([id], |r| r.get(0)).optional()?.unwrap_or(false))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_errand(&self, title: &str, detail: &str, job_id: Option<i64>, state: ErrandState, due_at: Option<&str>, private: bool, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached("INSERT INTO errand (title, detail, job_id, state, due_at, proposed_done, created_at, updated_at, private) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, ?7)")?
            .execute(params![title, detail, job_id, state.as_str(), due_at, now, private])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes an errand; changed from private data, it is private from then on.
    #[allow(clippy::too_many_arguments)]
    pub fn update_errand(&self, id: i64, title: Option<&str>, detail: Option<&str>, state: Option<ErrandState>, due_at: Option<Option<&str>>, proposed_done: Option<bool>, private: bool, now: &str) -> Result<bool> {
        let closed = state.map(|s| matches!(s, ErrandState::Done | ErrandState::Dropped));
        Ok(self.conn
            .prepare_cached(
                "UPDATE errand SET title = COALESCE(?2, title), detail = COALESCE(?3, detail), state = COALESCE(?4, state),
                 due_at = CASE WHEN ?5 THEN ?6 ELSE due_at END, proposed_done = COALESCE(?7, proposed_done), updated_at = ?8,
                 closed_at = CASE WHEN ?9 IS NULL THEN closed_at WHEN ?9 THEN ?8 ELSE NULL END, private = private OR ?10 WHERE id = ?1",
            )?
            .execute(params![id, title, detail, state.map(ErrandState::as_str), due_at.is_some(), due_at.flatten(), proposed_done, now, closed, private])? > 0)
    }

    // Messages.

    pub fn add_message(&self, m: &NewMessage, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached("INSERT INTO message (job_id, run_id, level, title, body, created_at, notified, held, private) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?
            .execute(params![m.job_id, m.run_id, m.level.as_str(), m.title, m.body, now, m.notified, m.held, m.private])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn messages(&self, limit: u32) -> Result<Vec<Message>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {MESSAGE} FROM message ORDER BY id DESC LIMIT ?1"))?;
        let rows = st.query_map([limit], message)?;
        rows.collect()
    }

    /// Messages held for the next brief, oldest first.
    pub fn held(&self, with_private: bool) -> Result<Vec<Message>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {MESSAGE} FROM message WHERE held = 1 AND (?1 OR private = 0) ORDER BY id"))?;
        let rows = st.query_map([with_private], message)?;
        rows.collect()
    }

    pub fn briefed(&self, ids: &[i64], now: &str) -> Result<()> {
        let mut st = self.conn.prepare_cached("UPDATE message SET held = 0, briefed_at = ?2 WHERE id = ?1")?;
        for id in ids {
            st.execute(params![id, now])?;
        }
        Ok(())
    }

    /// Interrupts shown as notifications at or after `since`.
    pub fn notified_since(&self, since: &str) -> Result<u32> {
        self.conn.prepare_cached("SELECT COUNT(*) FROM message WHERE notified = 1 AND created_at >= ?1")?.query_row([since], |r| r.get(0))
    }

    pub fn messages_of_run(&self, run_id: i64) -> Result<u32> {
        self.conn.prepare_cached("SELECT COUNT(*) FROM message WHERE run_id = ?1")?.query_row([run_id], |r| r.get(0))
    }

    // Pages.

    pub fn pages(&self, with_private: bool) -> Result<Vec<Page>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {PAGE} FROM page WHERE ?1 OR private = 0 ORDER BY updated_at DESC"))?;
        let rows = st.query_map([with_private], page)?;
        rows.collect()
    }

    pub fn page(&self, id: i64, with_private: bool) -> Result<Option<Page>> {
        self.conn.prepare_cached(&format!("SELECT {PAGE} FROM page WHERE id = ?1 AND (?2 OR private = 0)"))?.query_row(params![id, with_private], page).optional()
    }

    /// The job that wrote the page called `title`, if there is one: `Some(None)` for a page of
    /// no job's.
    pub fn page_owner(&self, title: &str) -> Result<Option<Option<i64>>> {
        self.conn.prepare_cached("SELECT job_id FROM page WHERE title = ?1")?.query_row([title], |r| r.get(0)).optional()
    }

    /// Writes the page called `title`, making it when it is new; its id. The whole page is
    /// written again, so it is as private as what wrote it last.
    pub fn write_page(&self, title: &str, question: &str, body: &str, job_id: Option<i64>, private: bool, now: &str) -> Result<i64> {
        self.conn
            .prepare_cached(
                "INSERT INTO page (title, question, body, job_id, updated_at, private) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(title) DO UPDATE SET question = excluded.question, body = excluded.body,
                 job_id = COALESCE(excluded.job_id, page.job_id), updated_at = excluded.updated_at, private = excluded.private",
            )?
            .execute(params![title, question, body, job_id, now, private])?;
        self.conn.prepare_cached("SELECT id FROM page WHERE title = ?1")?.query_row([title], |r| r.get(0))
    }

    pub fn delete_page(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("DELETE FROM page WHERE id = ?1")?.execute([id])? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-10-28T12:00:00.000000000Z";

    fn spec(name: &str) -> JobSpec {
        JobSpec {
            name: name.into(),
            instructions: "Look".into(),
            schedule: None,
            rules: vec![],
            sources: vec![Source::Dev],
            model: None,
            fallback: None,
            effort: None,
            level: Level::Brief,
            runs_per_day: None,
            minutes_per_run: None,
        }
    }

    #[test]
    fn jobs_and_runs_round_trip() {
        let a = Aide::open_in_memory().unwrap();
        let id = a.add_job(&spec("Brief"), JobState::Active, Some(NOW), false, NOW).unwrap();
        a.set_job_thread(id, 9).unwrap();
        let j = a.job(id).unwrap().unwrap();
        assert_eq!((j.spec.name.as_str(), j.state, j.thread_id), ("Brief", JobState::Active, Some(9)));
        assert_eq!(a.job_of_thread(9).unwrap().unwrap().id, id);

        let run = a.add_run(id, Reason::Manual, "pressed Run", RunState::Queued, None, NOW).unwrap();
        let read = SourceRead { source: Source::Dev, ok: true, detail: "3 sessions".into() };
        assert!(a.start_run(run, Some("sonnet"), "claude", 9, std::slice::from_ref(&read), NOW).unwrap());
        assert!(!a.start_run(run, Some("sonnet"), "claude", 9, &[], NOW).unwrap(), "a run is claimed once");
        assert_eq!(a.open_run_of_thread(9).unwrap().unwrap().id, run);
        assert_eq!(a.runs_since(id, NOW).unwrap(), 1);
        assert!(a.end_run(run, RunState::Done, Some("said it"), NOW).unwrap());
        assert!(!a.end_run(run, RunState::Failed, None, NOW).unwrap(), "an ended run stays ended");
        let r = a.run(run).unwrap().unwrap();
        assert_eq!((r.state, r.note.as_deref(), r.sources, r.job_name.as_str()), (RunState::Done, Some("said it"), vec![read], "Brief"));
        assert!(a.open_run_of_thread(9).unwrap().is_none());
        assert_eq!(a.job(id).unwrap().unwrap().last_run_at.as_deref(), Some(NOW));
    }

    #[test]
    fn a_job_keeps_its_last_runs() {
        let a = Aide::open_in_memory().unwrap();
        let id = a.add_job(&spec("Busy"), JobState::Active, None, false, NOW).unwrap();
        for _ in 0..(KEEP_RUNS + 5) {
            a.add_run(id, Reason::Rule, "x", RunState::Nothing, None, NOW).unwrap();
        }
        assert_eq!(a.runs(Some(id), 10_000).unwrap().len() as i64, KEEP_RUNS);
    }

    #[test]
    fn a_rule_finding_is_seen_once() {
        let a = Aide::open_in_memory().unwrap();
        let id = a.add_job(&spec("Watch"), JobState::Active, None, false, NOW).unwrap();
        assert!(a.see(id, "tx:5", NOW).unwrap());
        assert!(!a.see(id, "tx:5", NOW).unwrap());
        a.forget_seen_before("2027-01-01").unwrap();
        assert!(a.see(id, "tx:5", NOW).unwrap());
    }

    #[test]
    fn facts_errands_messages_pages() {
        let a = Aide::open_in_memory().unwrap();
        let f = a.add_fact("Antho trains on Mondays", "gym", None, false, false, NOW).unwrap();
        a.add_fact("Prefers short briefs", "person", None, true, false, NOW).unwrap();
        a.update_fact(f, None, Some(true), Some(true)).unwrap();
        assert!(a.facts(true).unwrap()[0].pinned);
        assert_eq!(a.forget_source("gym").unwrap(), 1);
        assert_eq!(a.facts(true).unwrap().len(), 1);

        let e = a.add_errand("Check the PR", "", None, ErrandState::Open, Some(NOW), false, NOW).unwrap();
        a.update_errand(e, None, None, Some(ErrandState::Waiting), None, Some(true), false, NOW).unwrap();
        let got = a.errand(e).unwrap().unwrap();
        assert_eq!((got.state, got.proposed_done, got.due_at.as_deref()), (ErrandState::Waiting, true, Some(NOW)));
        a.update_errand(e, None, None, Some(ErrandState::Done), Some(None), None, false, NOW).unwrap();
        let got = a.errand(e).unwrap().unwrap();
        assert_eq!((got.closed_at.as_deref(), got.due_at), (Some(NOW), None));
        assert!(a.errands(false, true).unwrap().is_empty());
        assert_eq!(a.errands(true, true).unwrap().len(), 1);

        let m = a.add_message(&NewMessage { job_id: None, run_id: None, level: Level::Interrupt, title: "PR merged", body: "", notified: false, held: true, private: false }, NOW).unwrap();
        assert_eq!(a.held(true).unwrap().len(), 1);
        a.briefed(&[m], NOW).unwrap();
        assert!(a.held(true).unwrap().is_empty());
        assert_eq!(a.notified_since("2000").unwrap(), 0);

        let p = a.write_page("Weekly", "How was the week?", "v1", None, false, NOW).unwrap();
        assert_eq!(a.write_page("Weekly", "How was the week?", "v2", None, false, NOW).unwrap(), p);
        assert_eq!(a.page(p, true).unwrap().unwrap().body, "v2");
    }

    #[test]
    fn a_lasting_finding_is_new_again_once_it_has_passed() {
        let a = Aide::open_in_memory().unwrap();
        let id = a.add_job(&spec("Watch"), JobState::Active, None, false, NOW).unwrap();
        a.see(id, "halt:loss", NOW).unwrap();
        a.see(id, "halt:other", NOW).unwrap();
        a.see(id, "tx:5", NOW).unwrap();
        a.forget_seen_but(id, "halt:", &["halt:loss"]).unwrap();
        assert!(!a.see(id, "halt:loss", NOW).unwrap(), "still there: still seen");
        assert!(a.see(id, "halt:other", NOW).unwrap(), "gone, so new again");
        assert!(!a.see(id, "tx:5", NOW).unwrap(), "other keys are kept");
    }

    #[test]
    fn private_rows_are_left_out_unless_asked_for() {
        let a = Aide::open_in_memory().unwrap();
        a.add_fact("Shared", "person", None, true, false, NOW).unwrap();
        a.add_fact("Spent SECRET412", "money", None, true, true, NOW).unwrap();
        assert_eq!(a.facts(false).unwrap().len(), 1);
        assert_eq!(a.facts(true).unwrap().len(), 2);

        let e = a.add_errand("Plain", "", None, ErrandState::Open, None, false, NOW).unwrap();
        a.add_errand("Private", "", None, ErrandState::Open, None, true, NOW).unwrap();
        assert_eq!(a.errands(false, false).unwrap().len(), 1);
        // Changed from private data, an errand is private from then on.
        a.update_errand(e, None, Some("SECRET412"), None, None, None, true, NOW).unwrap();
        assert!(a.errands(false, false).unwrap().is_empty());
        assert!(a.errand_is_private(e).unwrap());

        let m = |private| NewMessage { job_id: None, run_id: None, level: Level::Brief, title: "t", body: "", notified: false, held: true, private };
        a.add_message(&m(false), NOW).unwrap();
        a.add_message(&m(true), NOW).unwrap();
        assert_eq!((a.held(false).unwrap().len(), a.held(true).unwrap().len()), (1, 2));

        let p = a.write_page("Money", "How is the month?", "SECRET412", None, true, NOW).unwrap();
        assert!(a.pages(false).unwrap().is_empty());
        assert!(a.page(p, false).unwrap().is_none());
        assert_eq!(a.page_owner("Money").unwrap(), Some(None));
        assert_eq!(a.page_owner("Elsewhere").unwrap(), None);
        // Written again whole from shared data, it is shared.
        a.write_page("Money", "How is the month?", "fine", None, false, NOW).unwrap();
        assert_eq!(a.pages(false).unwrap().len(), 1);

        let mine = a.add_job(&spec("Mine"), JobState::Active, None, false, NOW).unwrap();
        let drafted = a.add_job(&spec("Drafted"), JobState::Proposed, None, true, NOW).unwrap();
        assert_eq!(a.jobs_listed(false).unwrap().iter().map(|j| j.id).collect::<Vec<_>>(), vec![mine]);
        a.set_job_state(drafted, JobState::Active, None, NOW).unwrap();
        assert_eq!(a.jobs_listed(false).unwrap().len(), 2, "turned on by the person, it is theirs");
    }

    #[test]
    fn a_file_from_before_private_rows_marks_what_its_own_models_wrote() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        let spec = json(&spec("Local"));
        conn.execute_batch(&format!(
            "INSERT INTO job (id, spec, state, created_at, updated_at) VALUES (1, '{spec}', 'active', '{NOW}', '{NOW}'), (2, '{spec}', 'active', '{NOW}', '{NOW}');
             INSERT INTO run (job_id, reason, trigger, state, host, sources, started_at) VALUES (1, 'manual', '', 'done', 'server', '[]', '{NOW}'),
                 (2, 'manual', '', 'done', 'claude', '[]', '{NOW}');
             INSERT INTO page (title, question, body, job_id, updated_at) VALUES ('Local', 'q', 'SECRET412', 1, '{NOW}'), ('Claude', 'q', 'b', 2, '{NOW}');"
        )).unwrap();
        let a = Aide::init(conn).unwrap();
        let shared: Vec<String> = a.pages(false).unwrap().into_iter().map(|p| p.title).collect();
        assert_eq!(shared, vec!["Claude".to_string()]);
    }

    #[test]
    fn settings_default_until_saved() {
        let a = Aide::open_in_memory().unwrap();
        assert_eq!(a.settings().unwrap(), Settings::default());
        let s = Settings { interrupt_cap: 2, ..Settings::default() };
        a.set_settings(&s).unwrap();
        assert_eq!(a.settings().unwrap(), s);
    }
}
