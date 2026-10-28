//! Everything Aide says goes through here: kept in `aide.db`, and an interrupt also shown as a
//! notification in the `aide` category (the desktop's bell, and the phone, which announces it).
//! An interrupt in quiet hours or past the day's cap is held for the next brief instead.
//!
//! Each job is a bot with its own chat (its channel), and the Chief of Staff ([`chief`]) is the
//! chat over them all. What a bot says is posted in both as a `note` ([`post`]), so the person
//! reads it as a message in a conversation, wherever they look.

use super::with_db;
use crate::engine::Engine;
use crate::threads;
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

/// `aide.db`'s meta key for the Chief of Staff's thread.
const CHIEF: &str = "chief_thread";

/// The Chief of Staff's thread, if it was made and is still there.
pub fn chief(engine: &Engine) -> Option<i64> {
    let id: i64 = with_db(engine, |db| db.meta(CHIEF)).ok().flatten()?.parse().ok()?;
    threads::get(engine, id).ok().filter(|t| t.kind == threads::AIDE).map(|_| id)
}

/// The Chief of Staff's thread, made on first use on Aide's default model; whether it was made now.
pub fn chief_or_make(engine: &Engine) -> Result<(i64, bool), BusError> {
    if let Some(id) = chief(engine) {
        return Ok((id, false));
    }
    let settings = super::settings(engine)?;
    let at = crate::time::now();
    // A default the thread cannot take (a server's model it does not know yet) leaves Claude's.
    let t = threads::create(engine, threads::AIDE, "Chief of Staff", settings.model.as_deref(), None, &at)
        .or_else(|_| threads::create(engine, threads::AIDE, "Chief of Staff", None, None, &at))?;
    with_db(engine, |db| db.set_meta(CHIEF, &t.id.to_string()))?;
    Ok((t.id, true))
}

/// Job `job`'s chat, its channel, made on first use on the model it names; whether it was made now.
/// A run moves it to the model that runs it.
pub fn bot_chat(engine: &Engine, job: &relay_aide::model::Job) -> Result<(i64, bool), BusError> {
    if let Some(id) = job.thread_id.filter(|id| threads::get(engine, *id).is_ok()) {
        return Ok((id, false));
    }
    let at = crate::time::now();
    let title: String = job.spec.name.chars().take(80).collect();
    let t = threads::create(engine, threads::AIDE, &title, job.spec.model.as_deref(), None, &at)
        .or_else(|_| threads::create(engine, threads::AIDE, &title, None, None, &at))?;
    with_db(engine, |db| db.set_job_thread(job.id, t.id))?;
    Ok((t.id, true))
}

/// Posts `m` as a note in its bot's chat and, unless it is quiet, in the Chief of Staff's.
/// `proposal` is the job a proposal drafted, so its note can offer to turn it on.
pub fn post(engine: &Engine, m: &Message, proposal: Option<i64>) {
    let job = m.job_id.and_then(|id| with_db(engine, |db| db.job(id)).ok().flatten());
    let body = json!({
        "message_id": m.id,
        "job_id": m.job_id,
        "bot": job.as_ref().map(|j| j.spec.name.clone()),
        "title": m.title,
        "body": m.body,
        "level": m.level,
        "notified": m.notified,
        "held": m.held,
        "proposal": proposal,
    });
    let mut to: Vec<i64> = job.and_then(|j| j.thread_id).into_iter().collect();
    if m.level != Level::Quiet || m.job_id.is_none() {
        to.extend(chief(engine).filter(|c| !to.contains(c)));
    }
    for id in to {
        if let Err(e) = threads::post_note(engine, id, &body, &m.created_at) {
            tracing::warn!(thread = id, error = %e.message, "could not post Aide's message in its chat");
        }
    }
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
    let Ok(message) = keep(engine, job_id, run_id, level, title, body, notified, held.is_some()) else {
        return;
    };
    post(engine, &message, None);
    if notified {
        let at = crate::time::now();
        let written = notification(&engine.store.lock(), title, body, &at);
        if let Ok(id) = written {
            engine.emit_system("notify.new", json!({"category": CATEGORY, "notification_id": id}));
        }
    }
    super::changed(engine, "message");
}
