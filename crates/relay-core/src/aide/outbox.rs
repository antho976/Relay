//! Everything Aide says goes through here: kept in `aide.db`, and an interrupt also shown as a
//! notification in the `aide` category (the desktop's bell, and the phone, which announces it).
//! An interrupt in quiet hours or past the day's cap is held for the next brief instead.

use super::with_db;
use crate::engine::Engine;
use jiff::Timestamp;
use relay_aide::model::{Level, Message};
use relay_aide::schedule;
use relay_aide::store::NewMessage;
use relay_bus::BusError;
use serde_json::json;

pub const CATEGORY: &str = "aide";

/// Whether a message at `level` is shown now, and why it is held when it is.
pub fn decide(engine: &Engine, level: Level) -> Result<(bool, Option<String>), BusError> {
    let settings = super::settings(engine)?;
    Ok(match level {
        Level::Quiet => (false, None),
        Level::Brief => (false, Some("for the brief".into())),
        Level::Interrupt if settings.paused => (false, Some("Aide is paused".into())),
        Level::Interrupt if settings.quiet_at(schedule::local_hour(Timestamp::now())) => (false, Some("quiet hours".into())),
        Level::Interrupt => {
            let today = crate::time::stamp(schedule::day_start(Timestamp::now()));
            let shown = with_db(engine, |db| db.notified_since(&today))?;
            if shown >= settings.interrupt_cap {
                (false, Some(format!("the day's {} interrupts were used", settings.interrupt_cap)))
            } else {
                (true, None)
            }
        }
    })
}

/// Keeps a message as [`decide`] decided.
#[allow(clippy::too_many_arguments)]
pub fn keep(engine: &Engine, job_id: Option<i64>, run_id: Option<i64>, level: Level, title: &str, body: &str, notified: bool, held: bool) -> Result<Message, BusError> {
    let at = crate::time::now();
    let m = NewMessage { job_id, run_id, level, title, body, notified, held };
    let id = with_db(engine, |db| db.add_message(&m, &at))?;
    let kept = with_db(engine, |db| db.messages(1))?.into_iter().find(|m| m.id == id);
    kept.ok_or_else(|| BusError::internal("the message went missing"))
}

/// The notification row for a shown interrupt, on the connection given.
pub fn notification(conn: &rusqlite::Connection, title: &str, body: &str, now: &str) -> rusqlite::Result<i64> {
    conn.prepare_cached("INSERT INTO notifications(project_id, category, title, body, link, read, created_at) VALUES (NULL, ?1, ?2, ?3, NULL, 0, ?4)")?
        .execute(rusqlite::params![CATEGORY, title, body, now])?;
    Ok(conn.last_insert_rowid())
}

/// Says something from outside a handler (the scheduler, a run ending).
pub fn say_now(engine: &Engine, job_id: Option<i64>, run_id: Option<i64>, level: Level, title: &str, body: &str) {
    let Ok((notified, held)) = decide(engine, level) else { return };
    if keep(engine, job_id, run_id, level, title, body, notified, held.is_some()).is_err() {
        return;
    }
    if notified {
        let at = crate::time::now();
        let written = notification(&engine.store.lock(), title, body, &at);
        if let Ok(id) = written {
            engine.emit_system("notify.new", json!({"category": CATEGORY, "notification_id": id}));
        }
    }
    super::changed(engine, "message");
}
