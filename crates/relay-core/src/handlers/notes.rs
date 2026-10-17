//! `notes.*` and `mailbox.*` — durable project context and per-recipient agent messages.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::sessions;
use relay_bus::error::BusError;
use relay_bus::ops::notes::*;
use relay_bus::types::{Id, Message, Note, SessionState};
use relay_bus::Empty;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::json;

fn note_row(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        body: row.get("body")?,
        pinned: row.get::<_, i64>("pinned")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn get_note(
    tx: &Transaction,
    id: Id,
    include_deleted: bool,
) -> Result<(Note, bool, bool), BusError> {
    let sql = if include_deleted {
        "SELECT *, standing, suggestions FROM notes WHERE id = ?1"
    } else {
        "SELECT *, standing, suggestions FROM notes WHERE id = ?1 AND deleted_at IS NULL"
    };
    tx.query_row(sql, [id], |row| {
        Ok((
            note_row(row)?,
            row.get::<_, i64>("standing")? != 0,
            row.get::<_, i64>("suggestions")? != 0,
        ))
    })
    .optional()
    .bus()?
    .ok_or_else(|| BusError::not_found("notes.not_found", format!("no note {id}")))
}

/// The project a bound session acts in; `None` for the person, who reaches every project.
pub(crate) fn actor_project(ctx: &Ctx) -> Result<Option<Id>, BusError> {
    session_project(ctx.tx(), ctx.actor_session_id(), ctx.actor.is_agent())
}

/// [`actor_project`] on a bare connection, for a handler that runs unlocked.
fn session_project(conn: &Connection, session_id: Option<Id>, agent: bool) -> Result<Option<Id>, BusError> {
    if let Some(session_id) = session_id {
        let row = sessions::by_id(conn, session_id)?
            .ok_or_else(|| BusError::actor("bound session no longer exists"))?;
        Ok(Some(row.session.project_id))
    } else if agent {
        Err(BusError::actor(
            "agent actor is not bound to a live session",
        ))
    } else {
        Ok(None)
    }
}

pub(crate) fn assert_actor_project(ctx: &Ctx, project_id: Id) -> Result<(), BusError> {
    check_project(ctx.tx(), project_id, ctx.actor_session_id(), ctx.actor.is_agent())
}

pub(crate) fn check_project(conn: &Connection, project_id: Id, session_id: Option<Id>, agent: bool) -> Result<(), BusError> {
    crate::handlers::workspace::get_project(conn, project_id)?;
    if session_project(conn, session_id, agent)?.is_some_and(|own| own != project_id) {
        return Err(BusError::not_own("project"));
    }
    Ok(())
}

/// `notes.changed` names the note and its new state, not its body: every subscribed client
/// receives the event, and one large note pushed whole to each of them on every keystroke-sized
/// append dropped the desktop's connection (RA-227). `notes.get` has the body.
fn emit_note(ctx: &mut Ctx, note: &Note) -> Result<(), BusError> {
    ctx.set_project(note.project_id);
    let mut payload = serde_json::to_value(note).bus()?;
    if let Some(map) = payload.as_object_mut() {
        map.remove("body");
        map.insert("body_bytes".into(), json!(note.body.len()));
    }
    ctx.emit("notes.changed", payload);
    Ok(())
}

/// The largest note body, in bytes. A body is returned whole by `notes.get` and the mutations,
/// and the clients' sockets cap a line at 2 MiB (RA-227).
const BODY_MAX: usize = 1024 * 1024;
/// `notes.list {summary}` cuts each body to this many characters.
const PREVIEW_CHARS: usize = 240;

fn check_body(len: usize) -> Result<(), BusError> {
    if len > BODY_MAX {
        return Err(BusError::invalid(
            "notes.body",
            format!("the note body would be {len} bytes; the limit is {BODY_MAX}"),
        ).with_hint("split it across notes, or put long material in a file and link it"));
    }
    Ok(())
}

fn standing_id(tx: &Transaction, project_id: Id, now: &str) -> Result<Id, BusError> {
    let found = tx
        .query_row(
            "SELECT id FROM notes WHERE project_id = ?1 AND standing = 1 AND deleted_at IS NULL",
            [project_id],
            |row| row.get(0),
        )
        .optional()
        .bus()?;
    if let Some(id) = found {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO notes(project_id, title, body, pinned, standing, created_at, updated_at)
         VALUES (?1, 'Standing notes', '', 1, 1, ?2, ?2)",
        params![project_id, now],
    )
    .bus()?;
    Ok(tx.last_insert_rowid())
}

fn suggestions_id(tx: &Transaction, project_id: Id, now: &str) -> Result<Id, BusError> {
    let found = tx
        .query_row(
            "SELECT id FROM notes
             WHERE project_id = ?1 AND suggestions = 1 AND deleted_at IS NULL",
            [project_id],
            |row| row.get(0),
        )
        .optional()
        .bus()?;
    if let Some(id) = found {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO notes(project_id, title, body, pinned, standing, suggestions, created_at, updated_at)
         VALUES (?1, 'Agent suggestions', '', 0, 0, 1, ?2, ?2)",
        params![project_id, now],
    )
    .bus()?;
    Ok(tx.last_insert_rowid())
}

fn message_row(row: &Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        from: row.get("from_session")?,
        to: row.get("to_spec")?,
        text: row.get("text")?,
        re_task: row.get("re_task")?,
        priority: row.get::<_, i64>("priority")? != 0,
        sent_at: row.get("sent_at")?,
        acked_at: row.get("acked_at")?,
    })
}

/// Who a message was addressed to, and where each of them stands right now.
fn recipients_of(tx: &Transaction, message_id: Id) -> Result<Vec<Recipient>, BusError> {
    let mut stmt = tx
        .prepare_cached(
            "SELECT r.session, r.acked_at, s.state FROM message_recipients r
             LEFT JOIN sessions s ON s.id = r.session_id
             WHERE r.message_id = ?1 ORDER BY r.session",
        )
        .bus()?;
    let rows = stmt
        .query_map([message_id], |row| {
            Ok(Recipient {
                session: row.get("session")?,
                state: sessions::parse_state(
                    &row.get::<_, Option<String>>("state")?
                        .unwrap_or_else(|| "closed".into()),
                ),
                acked_at: row.get("acked_at")?,
            })
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    Ok(rows)
}

/// Relay stores a message; the recipient reads it when it next runs. There is no injection
/// step to timestamp, so the honest answer is the addressees' states, not a `delivered_at`
/// that would only ever restate `sent_at` (D109).
fn delivery_hint(recipients: &[Recipient]) -> String {
    if recipients.is_empty() {
        return "no_recipients".to_string();
    }
    if recipients.iter().any(|r| sessions::is_live(r.state)) {
        "queued".to_string()
    } else if recipients.iter().any(|r| r.state == SessionState::Parked) {
        "session_parked".to_string()
    } else {
        "not_running".to_string()
    }
}

pub(crate) fn actor_name(ctx: &Ctx) -> Result<String, BusError> {
    if let Some(name) = ctx.actor.session_name() {
        if ctx.actor_session_id().is_none() {
            return Err(BusError::actor(
                "agent actor is not bound to a live session",
            ));
        }
        Ok(name.to_string())
    } else {
        Ok("user".to_string())
    }
}

/// `mailbox.list` page size when the caller names none, and the most it may ask for.
const MAILBOX_PAGE: u32 = 200;
const MAILBOX_PAGE_MAX: u32 = 1000;
/// The longest message text anyone may send.
const MAILBOX_TEXT_MAX: usize = 64 * 1024;
/// System notices are clipped here; the full text lives in the matching notification.
const SYSTEM_TEXT_MAX: usize = 2000;

/// Persist one system-originated message. Used by lifecycle and guardrail handlers inside
/// their existing request transaction, preserving a single audit boundary.
pub(crate) fn send_system(
    tx: &Transaction,
    project_id: Id,
    to: &str,
    text: &str,
    re_task: Option<Id>,
    now: &str,
) -> Result<Option<Message>, BusError> {
    send(
        tx, project_id, "system", to, text, re_task, false, now, true,
    )
}

/// Guardrail decisions need prompt attention, but still wait for the agent's next chosen call.
pub(crate) fn send_system_priority(
    tx: &Transaction,
    project_id: Id,
    to: &str,
    text: &str,
    re_task: Option<Id>,
    now: &str,
) -> Result<Option<Message>, BusError> {
    send(tx, project_id, "system", to, text, re_task, true, now, true)
}

#[allow(clippy::too_many_arguments)] // one transaction-local message envelope; no durable DTO exists
fn send(
    tx: &Transaction,
    project_id: Id,
    from: &str,
    to: &str,
    text: &str,
    re_task: Option<Id>,
    priority: bool,
    now: &str,
    allow_empty: bool,
) -> Result<Option<Message>, BusError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(BusError::invalid(
            "mailbox.text",
            "message text cannot be empty",
        ));
    }
    // Every message is pushed whole to every connected client in `mailbox.new`, and kept
    // forever. A system notice (often an agent's last reply) is cut short — the notification
    // carries the full text — and a person's or agent's message has a ceiling.
    let clipped;
    let text = if from == "system" && text.len() > SYSTEM_TEXT_MAX {
        let mut end = SYSTEM_TEXT_MAX;
        while !text.is_char_boundary(end) { end -= 1; }
        clipped = format!("{}…", text[..end].trim_end());
        clipped.as_str()
    } else {
        text
    };
    if text.len() > MAILBOX_TEXT_MAX {
        return Err(BusError::invalid(
            "mailbox.text",
            format!("message text is {} bytes; the limit is {MAILBOX_TEXT_MAX}", text.len()),
        ).with_hint("put long material in a note or a file and send its name"));
    }
    let mut re_task = re_task;
    if let Some(task_id) = re_task {
        let project: Option<Id> = tx
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [task_id],
                |r| r.get(0),
            )
            .optional()
            .bus()?;
        // A lifecycle notice about a task deleted while an agent held it still goes out, just
        // without the link: refusing it would fail the agent's `session.done` or Stop report.
        if project != Some(project_id) && from == "system" {
            re_task = None;
        } else if project != Some(project_id) {
            return Err(BusError::not_found(
                "task.not_found",
                format!("no task {task_id} in project {project_id}"),
            ));
        }
    }

    let mut recipients = Vec::<(Id, String)>::new();
    if to == "*" {
        let mut stmt = tx
            .prepare_cached(
                "SELECT id, name FROM sessions
             WHERE project_id = ?1 AND state != 'closed' AND name != ?2 ORDER BY id",
            )
            .bus()?;
        recipients = stmt
            .query_map(params![project_id, from], |r| Ok((r.get(0)?, r.get(1)?)))
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
    } else {
        let recipient = tx.query_row(
            "SELECT id, name FROM sessions WHERE project_id = ?1 AND name = ?2 AND state != 'closed'
             ORDER BY id DESC LIMIT 1",
            params![project_id, to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional().bus()?;
        if let Some(recipient) = recipient {
            recipients.push(recipient);
        } else if allow_empty {
            return Ok(None);
        } else {
            return Err(BusError::not_found(
                "mailbox.recipient",
                format!("no live session {to:?} in project {project_id}"),
            ));
        }
    }
    if recipients.is_empty() {
        if allow_empty {
            return Ok(None);
        }
        return Err(BusError::conflict(
            "mailbox.no_recipients",
            "broadcast has no live peer recipients",
        ));
    }

    if priority && from != "user" && from != "system" {
        for (session_id, session) in &recipients {
            let pending: bool = tx
                .query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM message_recipients r
                       JOIN messages m ON m.id = r.message_id
                       WHERE r.session_id = ?1 AND r.acked_at IS NULL
                         AND m.priority = 1 AND m.from_session = ?2
                         AND julianday(m.sent_at) >= julianday(COALESCE((
                           SELECT created_at FROM sessions WHERE project_id = m.project_id
                             AND name = ?2 AND state != 'closed' ORDER BY id DESC LIMIT 1), m.sent_at))
                     )",
                    params![session_id, from],
                    |row| row.get(0),
                )
                .bus()?;
            if pending {
                return Err(BusError::conflict(
                    "mailbox.priority_pending",
                    format!("{session:?} already has unread priority mail from {from:?}"),
                )
                .with_hint(
                    "wait for the recipient to acknowledge it before prioritizing another message",
                ));
            }
        }
    }

    tx.execute(
        "INSERT INTO messages(project_id, from_session, to_spec, text, re_task, priority, sent_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![project_id, from, to, text, re_task, priority as i64, now],
    )
    .bus()?;
    let id = tx.last_insert_rowid();
    for (session_id, session) in recipients {
        tx.execute(
            "INSERT INTO message_recipients(message_id, session_id, session) VALUES (?1, ?2, ?3)",
            params![id, session_id, session],
        )
        .bus()?;
    }
    Ok(Some(Message {
        id,
        project_id,
        from: from.to_string(),
        to: to.to_string(),
        text: text.to_string(),
        re_task,
        priority,
        sent_at: now.to_string(),
        acked_at: None,
    }))
}

pub fn register(e: &mut Engine) {
    // Unlocked (D149): a pure read, which the desktop's Notes library repeats on every
    // `notes.changed`; it takes the store only for the one query, with no transaction around it.
    e.register_unlocked::<List>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        // `summary` cuts the body in SQL (substr counts characters), so a large note is never
        // read whole for a list that shows a line of it.
        let body = if p.summary.unwrap_or(false) { format!("substr(body, 1, {PREVIEW_CHARS}) AS body") } else { "body".into() };
        let deleted = if p.include_deleted.unwrap_or(false) { "" } else { " AND deleted_at IS NULL" };
        let (pinned, order) = if p.pinned_only.unwrap_or(false) {
            (" AND pinned = 1", "standing DESC, updated_at DESC, id DESC")
        } else {
            ("", "standing DESC, pinned DESC, updated_at DESC, id DESC")
        };
        let sql = format!(
            "SELECT id, project_id, title, {body}, pinned, created_at, updated_at, deleted_at FROM notes
             WHERE project_id = ?1{deleted}{pinned} ORDER BY {order}"
        );
        let notes = ctx.read(|conn| {
            check_project(conn, p.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(&sql).bus()?;
            let notes = stmt.query_map([p.project_id], note_row).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(notes)
        })?;
        Ok(ListOut { notes })
    });
    e.register::<Get>(|ctx, p| {
        let note = get_note(ctx.tx(), p.note_id, false)?.0;
        assert_actor_project(ctx, note.project_id)?;
        Ok(note)
    });
    e.register::<Create>(|ctx: &mut Ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        check_body(p.body.len())?;
        ctx.tx()
            .execute(
                "INSERT INTO notes(project_id, title, body, pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    p.project_id,
                    p.title,
                    p.body,
                    p.pinned.unwrap_or(false) as i64,
                    ctx.now
                ],
            )
            .bus()?;
        let note = get_note(ctx.tx(), ctx.tx().last_insert_rowid(), false)?.0;
        ctx.set_undo("notes.delete", json!({"note_id": note.id}), None);
        emit_note(ctx, &note)?;
        Ok(note)
    });
    e.register::<Update>(|ctx: &mut Ctx, p| {
        let (before, standing, suggestions) = get_note(ctx.tx(), p.note_id, false)?;
        assert_actor_project(ctx, before.project_id)?;
        if let Some(expected) = &p.expected {
            let current = serde_json::to_value(&before).bus()?;
            for (field, value) in expected {
                if !["title", "body", "pinned"].contains(&field.as_str()) {
                    return Err(BusError::invalid("notes.expected_field", format!("{field} is not an editable note field")));
                }
                if current.get(field) != Some(value) {
                    return Err(BusError::conflict("notes.edit_conflict", format!("Note {field} changed elsewhere; your draft was not saved")));
                }
            }
        }
        let title = if suggestions {
            Some("Agent suggestions".to_string())
        } else {
            p.title.clone().unwrap_or_else(|| before.title.clone())
        };
        if let Some(body) = &p.body {
            check_body(body.len())?;
        }
        let body = p.body.clone().unwrap_or_else(|| before.body.clone());
        let pinned = if standing {
            true
        } else if suggestions {
            false
        } else {
            p.pinned.unwrap_or(before.pinned)
        };
        ctx.tx().execute(
            "UPDATE notes SET title = ?1, body = ?2, pinned = ?3, updated_at = ?4 WHERE id = ?5",
            params![title, body, pinned as i64, ctx.now, before.id],
        ).bus()?;
        let note = get_note(ctx.tx(), before.id, false)?.0;
        ctx.set_undo("notes.update", json!({"note_id": before.id, "title": before.title, "body": before.body, "pinned": before.pinned}), Some(json!({"updated_at": note.updated_at})));
        emit_note(ctx, &note)?;
        Ok(note)
    });
    e.register::<Append>(|ctx: &mut Ctx, p| {
        let mut target = p.target.as_deref().unwrap_or("standing");
        let mut note_id = p.note_id;
        let mut project_id = p.project_id;
        // The suggestions note is written only through its own arm, stamped and capped. Named
        // by id it took unstamped text of any length that read as another session's entry
        // (RA-385).
        if let (Some(id), None, None) = (p.note_id, p.project_id, p.target.as_deref()) {
            if let Ok((note, _, true)) = get_note(ctx.tx(), id, false) {
                (target, note_id, project_id) = ("suggestions", None, Some(note.project_id));
            }
        }
        let (id, text) = match (note_id, project_id, target) {
            (Some(id), None, "standing") if p.target.is_none() => (id, p.text),
            (None, Some(project_id), "standing") => {
                assert_actor_project(ctx, project_id)?;
                (standing_id(ctx.tx(), project_id, &ctx.now)?, p.text)
            }
            (None, project_id, "suggestions") => {
                let session_id = ctx.actor_session_id().ok_or_else(|| {
                    BusError::refused(
                        "notes.suggestion_actor",
                        "only a bound agent can record an observed workflow suggestion",
                    )
                })?;
                let session = sessions::by_id(ctx.tx(), session_id)?
                    .ok_or_else(|| BusError::actor("bound session no longer exists"))?
                    .session;
                let project_id = project_id.unwrap_or(session.project_id);
                assert_actor_project(ctx, project_id)?;
                let task_id = session.task_id.ok_or_else(|| {
                    BusError::conflict(
                        "notes.suggestion_task",
                        "an agent suggestion requires a current task",
                    )
                })?;
                let observation = p.text.trim();
                if observation.is_empty() {
                    return Err(BusError::invalid(
                        "notes.text",
                        "suggestion text cannot be empty",
                    ));
                }
                if observation.chars().count() > 500 {
                    return Err(BusError::invalid(
                        "notes.suggestion_length",
                        "suggestions are limited to 500 characters",
                    ));
                }
                let text = format!(
                    "- {} | {} | task #{} | {}",
                    ctx.now, session.name, task_id, observation
                );
                (suggestions_id(ctx.tx(), project_id, &ctx.now)?, text)
            }
            _ => {
                return Err(BusError::invalid(
                    "notes.target",
                    "provide note_id alone, project_id for standing, or target suggestions as a bound agent",
                ))
            }
        };
        let (before, _, _) = get_note(ctx.tx(), id, false)?;
        assert_actor_project(ctx, before.project_id)?;
        // Optional, as in task.changelog.write: refused rather than appended to a note that
        // changed since the caller read it (RA-413).
        if p.expected_updated_at.as_ref().is_some_and(|at| *at != before.updated_at) {
            return Err(BusError::conflict("notes.edit_conflict", "Note changed elsewhere since you read it; your text was not appended")
                .with_details(json!({"expected": p.expected_updated_at, "actual": before.updated_at})));
        }
        if text.is_empty() {
            return Err(BusError::invalid(
                "notes.text",
                "append text cannot be empty",
            ));
        }
        let separator = if before.body.is_empty() || before.body.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        // The whole note counts, not the text added: appends are how a note grows past the cap.
        check_body(before.body.len() + separator.len() + text.len())?;
        let body = format!("{}{separator}{text}", before.body);
        ctx.tx()
            .execute(
                "UPDATE notes SET body = ?1, updated_at = ?2 WHERE id = ?3",
                params![body, ctx.now, id],
            )
            .bus()?;
        let note = get_note(ctx.tx(), id, false)?.0;
        emit_note(ctx, &note)?;
        Ok(note)
    });
    e.register::<Pin>(|ctx: &mut Ctx, p| {
        let (before, standing, suggestions) = get_note(ctx.tx(), p.note_id, false)?;
        assert_actor_project(ctx, before.project_id)?;
        if standing && !p.pinned {
            return Err(BusError::conflict(
                "notes.standing",
                "the standing note is always pinned",
            ));
        }
        if suggestions && p.pinned {
            return Err(BusError::conflict(
                "notes.suggestions_unpinned",
                "the agent suggestions note is always unpinned",
            ));
        }
        ctx.tx()
            .execute(
                "UPDATE notes SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
                params![p.pinned as i64, ctx.now, p.note_id],
            )
            .bus()?;
        let note = get_note(ctx.tx(), p.note_id, false)?.0;
        ctx.set_undo(
            "notes.pin",
            json!({"note_id": before.id, "pinned": before.pinned}),
            Some(json!({"updated_at": note.updated_at})),
        );
        emit_note(ctx, &note)?;
        Ok(note)
    });
    e.register::<Delete>(|ctx: &mut Ctx, p| {
        let (note, standing, _) = get_note(ctx.tx(), p.note_id, false)?;
        assert_actor_project(ctx, note.project_id)?;
        if standing {
            return Err(BusError::conflict(
                "notes.standing",
                "the standing note cannot be deleted",
            ));
        }
        ctx.tx()
            .execute(
                "UPDATE notes SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                params![ctx.now, note.id],
            )
            .bus()?;
        ctx.set_project(note.project_id);
        ctx.set_undo("notes.restore", json!({"note_id": note.id}), None);
        ctx.emit(
            "notes.deleted",
            json!({"id": note.id, "project_id": note.project_id}),
        );
        Ok(Empty {})
    });
    e.register::<Restore>(|ctx: &mut Ctx, p| {
        let (before, _, suggestions) = get_note(ctx.tx(), p.note_id, true)?;
        assert_actor_project(ctx, before.project_id)?;
        if before.deleted_at.is_none() {
            return Err(BusError::conflict(
                "notes.not_deleted",
                format!("note {} is not deleted", before.id),
            ));
        }
        if suggestions {
            let active: bool = ctx
                .tx()
                .query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM notes
                       WHERE project_id = ?1 AND suggestions = 1
                         AND deleted_at IS NULL AND id != ?2
                     )",
                    params![before.project_id, before.id],
                    |row| row.get(0),
                )
                .bus()?;
            if active {
                return Err(BusError::conflict(
                    "notes.suggestions_exists",
                    "this project already has an active agent suggestions note",
                ));
            }
        }
        ctx.tx()
            .execute(
                "UPDATE notes SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
                params![ctx.now, before.id],
            )
            .bus()?;
        let note = get_note(ctx.tx(), before.id, false)?.0;
        ctx.set_undo("notes.delete", json!({"note_id": note.id}), None);
        emit_note(ctx, &note)?;
        Ok(note)
    });
    e.register::<Standing>(|ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let text = ctx.tx().query_row(
            "SELECT body FROM notes WHERE project_id = ?1 AND standing = 1 AND deleted_at IS NULL",
            [p.project_id], |r| r.get(0),
        ).optional().bus()?.unwrap_or_default();
        Ok(StandingOut { text })
    });

    e.register::<MailboxSend>(|ctx: &mut Ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let from = actor_name(ctx)?;
        let priority = p.priority.unwrap_or(false);
        if priority && ctx.actor.is_agent() {
            if p.to == "*" {
                return Err(BusError::conflict(
                    "mailbox.priority_broadcast",
                    "agents cannot mark broadcasts as priority",
                ));
            }
            if p.re_task.is_none() {
                return Err(BusError::invalid(
                    "mailbox.priority_task",
                    "agent priority mail must reference the task that makes it urgent",
                ));
            }
        }
        let message = send(
            ctx.tx(),
            p.project_id,
            &from,
            &p.to,
            &p.text,
            p.re_task,
            priority,
            &ctx.now,
            false,
        )?
        .ok_or_else(|| BusError::internal("mailbox send produced no message"))?;
        // "Stored" is not "will be read". Naming the addressees and their states is the
        // difference between a send an agent can report on and one it can only hope about.
        let recipients = recipients_of(ctx.tx(), message.id)?;
        let delivery = delivery_hint(&recipients);
        ctx.set_project(p.project_id);
        ctx.emit("mailbox.new", serde_json::to_value(&message).bus()?);
        Ok(SendOut {
            message,
            recipients,
            delivery,
        })
    });
    e.register::<MailboxOutbox>(|ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let from = actor_name(ctx)?;
        // Messages carry the sender's name, and a name is reused once its session closes:
        // what an earlier namesake sent predates this session and is not its outbox.
        let born = match ctx.actor_session_id() {
            Some(id) => sessions::by_id(ctx.tx(), id)?.map(|row| row.session.created_at),
            None => None,
        };
        let since = p.since.as_deref().map(|raw| crate::time::bound("since", raw)).transpose()?;
        let limit = p.limit.unwrap_or(100).min(1000);
        let mut stmt = ctx
            .tx()
            .prepare_cached(
                "SELECT m.*, NULL AS acked_at FROM messages m
             WHERE m.project_id = ?1 AND m.from_session = ?2 AND (?3 IS NULL OR m.sent_at >= ?3)
               AND (?5 IS NULL OR julianday(m.sent_at) >= julianday(?5))
             ORDER BY m.sent_at DESC, m.id DESC LIMIT ?4+0",
            )
            .bus()?;
        let messages = stmt
            .query_map(params![p.project_id, from, since, limit, born], message_row)
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        drop(stmt);
        let mut sent = Vec::with_capacity(messages.len());
        for message in messages {
            let recipients = recipients_of(ctx.tx(), message.id)?;
            sent.push(OutboxEntry {
                message,
                recipients,
            });
        }
        Ok(OutboxOut { sent })
    });
    e.register::<MailboxList>(|ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        // A recipient is a session, not a name: names are reused once a session closes, and
        // a new session must not inherit an earlier namesake's inbox.
        let session: Option<Id> = match (&p.session, ctx.actor.session_name()) {
            (Some(requested), Some(actor)) if requested != actor => return Err(BusError::not_own("mailbox")),
            (_, Some(_)) => Some(ctx.actor_session_id()
                .ok_or_else(|| BusError::actor("agent actor is not bound to a live session"))?),
            // The person may read any session's inbox, a closed one's included: the newest of
            // that name, which is the live one when there is one.
            (Some(requested), None) => {
                let id: Option<Id> = ctx.tx().prepare_cached(
                    "SELECT id FROM sessions WHERE project_id=?1 AND name=?2 ORDER BY id DESC LIMIT 1",
                ).bus()?.query_row(params![p.project_id, requested], |row| row.get(0)).optional().bus()?;
                match id {
                    Some(id) => Some(id),
                    None => return Ok(MailboxListOut { messages: Vec::new(), next_before: None, more_unread: false }),
                }
            }
            (None, None) => None,
        };
        let mut sql = String::from(
            "SELECT m.*, CASE WHEN ?2 IS NOT NULL THEN mr.acked_at
              WHEN NOT EXISTS (SELECT 1 FROM message_recipients pending WHERE pending.message_id=m.id AND pending.acked_at IS NULL)
              THEN (SELECT MAX(done.acked_at) FROM message_recipients done WHERE done.message_id=m.id) ELSE NULL END AS acked_at
             FROM messages m LEFT JOIN message_recipients mr ON mr.message_id=m.id AND mr.session_id=?2
             WHERE m.project_id=?1",
        );
        if session.is_some() { sql.push_str(" AND mr.session IS NOT NULL"); }
        if p.unread_only.unwrap_or(false) {
            if session.is_some() {
                sql.push_str(" AND mr.acked_at IS NULL");
            } else {
                sql.push_str(" AND EXISTS (SELECT 1 FROM message_recipients pending WHERE pending.message_id=m.id AND pending.acked_at IS NULL)");
            }
        }
        sql.push_str(" AND (?3 IS NULL OR m.sent_at >= ?3) AND (?4 IS NULL OR m.id < ?4)");
        // History is never pruned (D26), so the reply is a page. A reader working through its
        // unread mail takes the oldest first and acks as it goes; every other view wants the
        // newest. Either way one row past the page says whether there is more.
        let unread = p.unread_only.unwrap_or(false);
        // `?5+0`: SQLite re-prepares a statement whose LIMIT is a bare parameter on every run.
        sql.push_str(if unread { " ORDER BY m.id ASC LIMIT ?5+0" } else { " ORDER BY m.id DESC LIMIT ?5+0" });
        let limit = p.limit.unwrap_or(MAILBOX_PAGE).clamp(1, MAILBOX_PAGE_MAX) as usize;
        let since = p.since.as_deref().map(|raw| crate::time::bound("since", raw)).transpose()?;
        let mut stmt = ctx.tx().prepare_cached(&sql).bus()?;
        let mut messages = stmt.query_map(params![p.project_id, session, since, p.before, limit as i64 + 1], message_row).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        let more = messages.len() > limit;
        messages.truncate(limit);
        if unread {
            return Ok(MailboxListOut { messages, next_before: None, more_unread: more });
        }
        messages.reverse();
        let next_before = more.then(|| messages.first().map(|m| m.id)).flatten();
        Ok(MailboxListOut { messages, next_before, more_unread: false })
    });
    e.register::<MailboxAck>(|ctx: &mut Ctx, p| {
        let session_id = ctx.actor_session_id().ok_or_else(|| BusError::actor("mailbox.ack requires a bound agent session"))?;
        let changed = ctx.tx().execute(
            "UPDATE message_recipients SET acked_at = COALESCE(acked_at, ?1)
             WHERE message_id = ?2 AND session_id = ?3",
            params![ctx.now, p.message_id, session_id],
        ).bus()?;
        if changed == 0 {
            return Err(BusError::not_found("mailbox.message", format!("message {} is not addressed to this session", p.message_id)));
        }
        let project_id: Id = ctx.tx().query_row("SELECT project_id FROM messages WHERE id = ?1", [p.message_id], |r| r.get(0)).bus()?;
        ctx.set_project(project_id);
        ctx.emit("mailbox.changed", json!({"message_id": p.message_id, "session": ctx.actor.session_name(), "acked_at": ctx.now}));
        Ok(Empty {})
    });
}
