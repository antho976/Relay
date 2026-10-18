//! `session.*` — worktree + PTY lifecycle: create/spawn/resume/park/wake/close,
//! attach/input/resize/scrollback, recovery, PAIR ownership, get/list/update.

use crate::awareness::SESSION_SKILLS_PATH;
use crate::engine::{Ctx, Engine, IntoBus};
use crate::pty::{Pty, SpawnSpec};
use crate::sessions::{self, Row_};
use crate::worktree;
use relay_bus::error::BusError;
use relay_bus::ops::session::*;
use relay_bus::types::{Id, Provider, Role, Session, SessionState};
use relay_bus::Empty;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Where a completion notification points: the task if the session carries one, else the
/// session itself. `session.done` and the Stop-hook fallback must agree, so both call this.
fn done_link(s: &Session) -> String {
    match s.task_id {
        Some(task_id) => json!({"op":"task.get", "payload":{"task_id":task_id}}).to_string(),
        None => json!({"op":"session.get", "payload":{"session":s.name}}).to_string(),
    }
}

/// The session's first open, active queued task other than `except`, with its module. The one
/// "what is next" query: group advance and task dispatch both pick from it (RA-645).
pub(crate) fn next_queued_task(
    conn: &Connection,
    session_id: Id,
    except: Option<Id>,
) -> Result<Option<(Id, Option<Id>)>, BusError> {
    conn.prepare_cached(
        "SELECT t.id,t.module_id
         FROM task_sessions ts JOIN tasks t ON t.id=ts.task_id
         WHERE ts.session_id=?1 AND (?2 IS NULL OR t.id!=?2) AND t.deleted_at IS NULL AND t.col='active' AND ts.completed_at IS NULL
         ORDER BY ts.queue_ord,t.position,t.id LIMIT 1",
    ).bus()?
    .query_row(params![session_id, except], |row| Ok((row.get(0)?, row.get(1)?)))
    .optional()
    .bus()
}

/// Queue `task_id` at the end of each session's queue, after the task's existing assignees.
/// Already queued is a no-op. Every attach path — create, update, task dispatch — goes through
/// here for the whole review group, so members' queues cannot diverge (RA-645). Returns the
/// sessions it queued the task for, those that were not already: what an undo takes back.
pub(crate) fn enqueue(conn: &Connection, task_id: Id, session_ids: &[Id]) -> Result<Vec<Id>, BusError> {
    let mut added = Vec::new();
    for &session_id in session_ids {
        let inserted = conn.prepare_cached(
            "INSERT OR IGNORE INTO task_sessions(task_id,session_id,ord,queue_ord) VALUES (?1,?2,
               (SELECT COALESCE(MAX(ord),-1)+1 FROM task_sessions WHERE task_id=?1),
               (SELECT COALESCE(MAX(queue_ord),-1)+1 FROM task_sessions WHERE session_id=?2))",
        ).bus()?
        .execute(params![task_id, session_id]).bus()?;
        if inserted > 0 { added.push(session_id); }
    }
    Ok(added)
}

/// The running PTY of a session, or the typed refusal the input fallbacks give for one that
/// does not exist, was never spawned, or has exited. One short read; the PTY itself is memory.
fn live_pty(ctx: &crate::engine::Unlocked, name: &str) -> Result<(Id, Arc<Pty>), BusError> {
    let id = ctx.read(|conn| Ok(sessions::by_name(conn, name)?.session.id))?;
    let pty = ctx.engine().pty(id)
        .ok_or_else(|| BusError::conflict("session.not_spawned", format!("session {name} has no PTY")))?;
    if pty.exited() { return Err(BusError::conflict("session.exited", format!("session {name} has exited"))); }
    Ok((id, pty))
}

/// The session's review group, by id.
fn group_ids(conn: &Connection, session: &Session) -> Result<Vec<Id>, BusError> {
    Ok(sessions::review_group(conn, session)?.into_iter().map(|(id, _)| id).collect())
}

/// Refuse a task or module that is not a live record of `project_id`.
fn check_task_module(conn: &Connection, project_id: Id, task_id: Option<Id>, module_id: Option<Id>) -> Result<(), BusError> {
    if let Some(task_id) = task_id {
        let valid: bool = conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2 AND deleted_at IS NULL)").bus()?
            .query_row(params![task_id, project_id], |row| row.get(0)).bus()?;
        if !valid { return Err(BusError::not_found("task.not_found", format!("no task {task_id} in project {project_id}"))); }
    }
    if let Some(module_id) = module_id {
        let valid: bool = conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM modules WHERE id=?1 AND project_id=?2 AND deleted_at IS NULL)").bus()?
            .query_row(params![module_id, project_id], |row| row.get(0)).bus()?;
        if !valid { return Err(BusError::not_found("module.not_found", format!("no module {module_id} in project {project_id}"))); }
    }
    Ok(())
}

fn assignment_text(task_id: Id, review: bool) -> String {
    if review {
        format!("All builders finished Task #{task_id}. Review the current task in the shared worktree; report your review outcome before the group advances.")
    } else {
        format!("Your current assignment is Task #{task_id}. Read session.bootstrap for the task and group context before continuing.")
    }
}

pub(crate) fn announce_assignment(ctx: &mut Ctx, session: &Session, task_id: Id, review: bool) -> Result<(), BusError> {
    let text = assignment_text(task_id, review);
    if let Some(message) = crate::handlers::notes::send_system(ctx.tx(), session.project_id, &session.name, &text, Some(task_id), &ctx.now)? {
        ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
    }
    deliver_assignment(ctx, session, task_id, review)
}

/// `sessions.done_pending_stop` holds the task a `session.done` closed in the provider's
/// current turn (0 for none) until that turn's Stop. This value instead says no done is
/// pending, only an assignment the session could not take when it was made — busy with a
/// turn, or stopped at a prompt — which its next Stop types in.
const ASSIGNMENT_AT_STOP: Id = -1;

/// The `session.done` still waiting for its turn's Stop, if any.
fn done_pending(conn: &Connection, session_id: Id) -> Result<Option<Id>, BusError> {
    let marker: Option<Id> = conn.prepare_cached("SELECT done_pending_stop FROM sessions WHERE id=?1").bus()?
        .query_row([session_id], |row| row.get(0)).bus()?;
    Ok(marker.filter(|id| *id != ASSIGNMENT_AT_STOP))
}

fn deliver_assignment(ctx: &mut Ctx, session: &Session, task_id: Id, review: bool) -> Result<(), BusError> {
    // Done runs inside the provider's old turn. Its trailing Stop is the safe handoff
    // edge; hookless providers retain the assignment in durable mail instead.
    if done_pending(ctx.tx(), session.id)?.is_some() { return Ok(()); }
    // Typed now if the session is idle; otherwise this stays set and its next Stop delivers.
    ctx.tx().execute("UPDATE sessions SET done_pending_stop=?1 WHERE id=?2", params![ASSIGNMENT_AT_STOP, session.id]).bus()?;
    let text = assignment_text(task_id, review);
    let (id, project) = (session.id, session.project_id);
    ctx.after_commit(move |engine| {
        let Some(pty) = engine.pty(id) else { return; };
        if !pty.claim_idle_edge() { return; }
        let changed = engine.system_write("session.assignment.ready", None, Some(project), Some(id), json!({"task_id":task_id}), |tx, now| {
            let changed = tx.execute("UPDATE sessions SET state='running',done_pending_stop=NULL,updated_at=?1 WHERE id=?2 AND task_id=?3 AND state='idle' AND done_pending_stop=?4", params![now,id,task_id,ASSIGNMENT_AT_STOP]).bus()?;
            let mut events = Vec::new();
            if changed > 0 {
                if let Some(row) = sessions::by_id(tx,id)? { events.push(("session.changed".into(),serde_json::to_value(row.session).bus()?)); }
            }
            Ok((changed > 0, events))
        });
        if matches!(changed, Ok(true)) {
            if let Err(error) = pty.write(format!("{text}\r").as_bytes()) {
                tracing::warn!(session_id=id, %error, "assignment remains available in Relay mailbox");
            }
        }
    });
    Ok(())
}

fn complete_group_assignment(ctx: &mut Ctx, session: &Session, task_id: Id, sha: Option<&str>) -> Result<(), BusError> {
    // A task deleted while the group held it has nothing left to build or review: whoever
    // reports done on it is released, and the group moves on without waiting for a review.
    let deleted: bool = ctx.tx().query_row("SELECT NOT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND deleted_at IS NULL)",[task_id],|row|row.get(0)).bus()?;
    if session.role == Role::Reviewer && !deleted {
        let ready: bool = ctx.tx().query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND col IN ('in_review','done') AND deleted_at IS NULL)",[task_id],|row|row.get(0)).bus()?;
        if !ready { return Err(BusError::conflict("session.review_not_ready", "Builders must finish the current task before its review can complete")); }
    }
    ctx.tx().execute("UPDATE task_sessions SET completed_at=COALESCE(completed_at,?1) WHERE task_id=?2 AND session_id=?3",params![ctx.now,task_id,session.id]).bus()?;
    // A reviewer's role may not link commits (task.link_commit is not on its allowlist), so
    // only a builder's sha is recorded, trimmed as task.link_commit trims it (RA-396).
    if let Some(sha) = sha.map(str::trim).filter(|sha| !sha.is_empty() && session.role != Role::Reviewer) {
        ctx.tx().execute("INSERT OR IGNORE INTO task_commits(task_id,sha,branch,linked_at) VALUES (?1,?2,?3,?4)",params![task_id,sha,session.branch,ctx.now]).bus()?;
    }
    // Only this session's review group waits on itself: an unrelated agent that shares the
    // checkout (the primary, D160) and happens to hold a queue row for the task is not in it.
    let group = sessions::review_group(ctx.tx(), session)?;
    let mut stmt = ctx.tx().prepare_cached("SELECT session_id, completed_at IS NULL FROM task_sessions WHERE task_id=?1").bus()?;
    let assigned = stmt.query_map([task_id],|row|Ok((row.get::<_,Id>(0)?,row.get::<_,bool>(1)?))).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
    drop(stmt);
    let members: Vec<(Id, Role, bool)> = group.iter()
        .filter_map(|(id, role)| assigned.iter().find(|(sid, _)| sid == id).map(|(_, open)| (*id, *role, *open)))
        .collect();
    let left = |want: Role| members.iter().filter(|(_, role, open)| *role == want && *open).count();
    if left(Role::Builder) != 0 { return Ok(()); }
    let changed = ctx.tx().execute("UPDATE tasks SET col='in_review',state='awaiting_review',updated_at=?1 WHERE id=?2 AND col='active' AND deleted_at IS NULL",params![ctx.now,task_id]).bus()?;
    let ids: Vec<Id> = members.iter().map(|(id, _, _)| *id).collect();
    if changed > 0 {
        ctx.emit("task.changed",json!({"task_id":task_id,"col":"in_review","state":"awaiting_review"}));
        for id in &ids {
            if let Some(row) = sessions::by_id(ctx.tx(),*id)? {
                if row.session.role == Role::Reviewer { announce_assignment(ctx,&row.session,task_id,true)?; }
            }
        }
    }
    if left(Role::Reviewer) != 0 && !deleted { return Ok(()); }
    let next = next_queued_task(ctx.tx(),session.id,Some(task_id))?;
    for id in ids {
        let changed = ctx.tx().execute("UPDATE sessions SET task_id=?1,module_id=?2,updated_at=?3 WHERE id=?4 AND task_id=?5",params![next.map(|v|v.0),next.and_then(|v|v.1),ctx.now,id,task_id]).bus()?;
        if changed > 0 {
            if let Some(row) = sessions::by_id(ctx.tx(),id)? {
                emit_session(ctx,&row.session);
                if let Some((next_id,_)) = next { announce_assignment(ctx,&row.session,next_id,false)?; }
            }
        }
    }
    Ok(())

}

/// What a fresh launch types into the new PTY. The launch assignment is handed over as the
/// work to begin only on the session's first spawn: a later fresh start
/// (`session.clear_restorable`) may come long after that work was finished, so it begins the
/// current task instead and offers the launch text only as background (RA-397).
fn launch_nudge(row: &Row_, first_launch: bool) -> Option<String> {
    let assignment = row.launch_prompt.as_deref().map(str::trim).filter(|text| !text.is_empty());
    if let Some(assignment) = assignment.filter(|_| first_launch) {
        return Some(format!(
            "Call session.bootstrap first, then begin this assignment: {assignment}",
        ));
    }
    if row.session.task_id.is_none() {
        return assignment.map(|assignment| format!(
            "Call session.bootstrap first. This session was launched earlier with these instructions, which may already be done; check before acting on them: {assignment}",
        ));
    }
    row.session.task_id.map(|task_id| match row.session.role {
        Role::Builder => format!(
            "Call session.bootstrap first, then begin current Task #{task_id}. Work through queued tasks one at a time; session.done completes only the current task."
        ),
        Role::Reviewer => format!(
            "Call session.bootstrap first, then begin the review-group work for current Task #{task_id}. Watch Relay mailbox for file-ready messages from the builders."
        ),
        Role::Docs => format!(
            "Call session.bootstrap first, then begin the documentation work for current Task #{task_id}."
        ),
    })
}

/// Agents act on their own session (or their PAIR partner for reads); the user on any.
fn assert_own(ctx: &Ctx, row: &Row_, reads_ok_for_pair: bool) -> Result<(), BusError> {
    owns(&ctx.actor, ctx.actor_session_id(), row, reads_ok_for_pair, |sid| sessions::by_id(ctx.tx(), sid))
}

/// [`assert_own`] for a handler with no transaction. `actor_row` looks up the actor's own row,
/// and is called only for a PAIR read the target's own link does not already admit.
fn owns(
    actor: &relay_bus::envelope::Actor, actor_session_id: Option<Id>, row: &Row_, reads_ok_for_pair: bool,
    actor_row: impl FnOnce(Id) -> Result<Option<Row_>, BusError>,
) -> Result<(), BusError> {
    if let Some(sid) = actor_session_id {
        if sid == row.session.id {
            return Ok(());
        }
        if reads_ok_for_pair {
            if let Some(pair) = &row.session.pair_with {
                if Some(pair.as_str()) == actor.session_name() {
                    return Ok(());
                }
            }
            // PAIR runs both ways. A reviewer names only one of its two builders, so the
            // other is its partner by its own link, as guardrail::authorize reads it (RA-398).
            if actor_row(sid)?.is_some_and(|own| own.session.pair_with.as_deref() == Some(row.session.name.as_str())) {
                return Ok(());
            }
        }
        return Err(BusError::not_own("session"));
    }
    if actor.is_agent() {
        // an agent through the in-process door without a bound session: only its own name
        if actor.session_name() != Some(row.session.name.as_str()) {
            return Err(BusError::not_own("session"));
        }
    }
    Ok(())
}

/// Where `session.upload` puts pictures, from a session's checkout: inside `.relay/`, which
/// Relay keeps out of git in every checkout it manages.
const ATTACHMENTS: &str = ".relay/attachments";

/// Pictures larger than this are refused; the phone sends a photo shrunk well under it, and a
/// door takes at most 8 MiB in one message.
const UPLOAD_MAX: usize = 4 << 20;

fn decode_upload(encoded: &str) -> Result<Vec<u8>, BusError> {
    use base64::Engine as _;
    let too_big = || BusError::invalid("session.upload", format!("the picture is over {} MiB", UPLOAD_MAX >> 20))
        .with_hint("send it smaller");
    if encoded.len() > UPLOAD_MAX.div_ceil(3) * 4 + 4 {
        return Err(too_big());
    }
    let compact: String = encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
        .map_err(|_| BusError::invalid("session.upload", "bytes_b64 is not base64"))?;
    if bytes.len() > UPLOAD_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

/// `.relay/attachments` in `worktree`, made if missing. The agent owns its checkout, so either
/// folder may be a link it planted: a link is refused rather than written through.
fn attachments_dir(worktree: &Path) -> Result<PathBuf, BusError> {
    let mut dir = worktree.to_path_buf();
    for part in ATTACHMENTS.split('/') {
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(BusError::conflict("session.upload_path", format!("{} is not a folder", dir.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(&dir) {
                Ok(()) => {}
                // Another upload made it first: fine, if it is a real folder.
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) => {}
                Err(error) => return Err(crate::engine::internal(error)),
            },
            Err(error) => return Err(crate::engine::internal(error)),
        }
    }
    Ok(dir)
}

/// Write `bytes` as `name` in `dir` unless it is there already (a name is its content's hash),
/// through a temporary file renamed into place, so the agent never reads half a picture.
fn store_upload(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), BusError> {
    use std::io::Write as _;
    let path = dir.join(name);
    let present = || fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file());
    if present() {
        return Ok(());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(dir).bus()?;
    tmp.write_all(bytes).bus()?;
    match tmp.persist_noclobber(&path) {
        Ok(_) => Ok(()),
        Err(_) if present() => Ok(()),
        Err(error) => Err(crate::engine::internal(error.error)),
    }
}

/// [`assert_own`] with PAIR reads allowed, for a handler with no transaction: one short read
/// for the target row and the actor's own.
fn read_own_or_pair(ctx: &crate::engine::Unlocked, session: &str) -> Result<Row_, BusError> {
    let (row, own) = ctx.read(|conn| Ok((
        sessions::by_name(conn, session)?,
        ctx.actor_session_id().map(|sid| sessions::by_id(conn, sid)).transpose()?.flatten(),
    )))?;
    owns(&ctx.actor, ctx.actor_session_id(), &row, true, |_| Ok(own))?;
    Ok(row)
}

/// The project an agent actor is confined to, or `None` for the user.
fn actor_project(ctx: &Ctx) -> Result<Option<Id>, BusError> {
    let Some(sid) = ctx.actor_session_id() else {
        if ctx.actor.is_agent() {
            return Err(BusError::actor(
                "agent actor is not bound to a live session",
            ));
        }
        return Ok(None);
    };
    let own =
        sessions::by_id(ctx.tx(), sid)?.ok_or_else(|| BusError::actor("bound session vanished"))?;
    Ok(Some(own.session.project_id))
}

/// Metadata reads are project-scoped for agents; the private surfaces (scrollback, brief)
/// keep using [`assert_own`].
fn assert_same_project(ctx: &Ctx, row: &Row_) -> Result<(), BusError> {
    match actor_project(ctx)? {
        Some(project_id) if project_id != row.session.project_id => {
            Err(BusError::not_own("project"))
        }
        _ => Ok(()),
    }
}

/// A running PTY knows when its child last spoke even when the provider files no lifecycle
/// report; without this, an unhooked provider reads as a session that has never done anything.
fn overlay_live_output(ctx: &Ctx, sessions: &mut [Session]) {
    for session in sessions {
        if let Some(at) = ctx
            .engine()
            .pty(session.id)
            .and_then(|pty| pty.last_output_at())
        {
            session.last_output_at = Some(at);
        }
    }
}

/// Claims name worktree-relative paths, like every other path on the bus, spelled as
/// `overlap.flag` spells them so the two always meet (RA-387).
fn claim_path(raw: &str) -> Result<String, BusError> {
    crate::handlers::overlap::normalize_relative(raw).ok_or_else(|| BusError::invalid(
        "session.claim_path",
        format!("{raw:?} must be worktree-relative and contain no .."),
    ))
}

fn emit_session(ctx: &mut Ctx, s: &Session) {
    ctx.set_project(s.project_id);
    ctx.set_session(s.id);
    ctx.emit(
        "session.changed",
        serde_json::to_value(s).unwrap_or(Value::Null),
    );
}

fn release_claims(ctx: &mut Ctx, session: &Session) -> Result<(), BusError> {
    ctx.tx().execute("DELETE FROM claims WHERE session_id=?1", [session.id]).bus()?;
    ctx.tx().execute(
        "UPDATE overlaps SET active=0,last_seen=?1 WHERE project_id=?2 AND active=1
         AND EXISTS (SELECT 1 FROM json_each(overlaps.sessions) WHERE value=?3)",
        params![ctx.now, session.project_id, session.name],
    ).bus()?;
    ctx.emit("overlap.changed", json!({"project_id":session.project_id,"session":session.name}));
    Ok(())
}

/// How a person hears an agent's notification.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ping {
    /// It needs the person: a permission prompt, a blocked or partial done. Each one sounds.
    Always,
    /// A finished turn. A terminal sounds one until the person types into it again
    /// ([`crate::pty::Pty::claim_ping`]); the turns an agent takes on its own after that (a
    /// background task, a peer's mail, its next assignment) only update the card.
    Finished,
}

// Hooks and explicit reports can describe the same result more than once, and an agent left
// alone finishes turn after turn. Keep one card per result, and sound only what `ping` allows.
// True when this one sounds: the caller emits `notify.new` and tells the peers.
fn record_agent_notification(ctx: &mut Ctx, s: &Session, ping: Ping, category: &str, title: &str, body: &str, link: &str) -> Result<bool, BusError> {
    let (latest, unread) = ctx.tx().prepare_cached(
        "SELECT id,read=0 FROM notifications WHERE project_id=?1 AND category=?2 AND title=?3 AND link=?4 ORDER BY id DESC LIMIT 1",
    ).bus()?.query_row(params![s.project_id, category, title, link], |row| Ok((Some(row.get::<_, i64>(0)?), row.get::<_, bool>(1)?)))
        .optional().bus()?.unwrap_or((None, false));
    let loud = match (ping, ctx.engine().pty(s.id)) {
        (Ping::Always, _) => true,
        (Ping::Finished, Some(pty)) => pty.claim_ping(),
        // No terminal to type into, so nothing says the person came back: one ping per unread card.
        (Ping::Finished, None) => !unread,
    };
    if let (false, Some(id)) = (loud, latest) {
        // The card now stands for this report, so it takes this report's time: notify.list and
        // the client's LAST REPORT order by created_at, and a stale one buried it (RA-234). The
        // generic hook body never replaces a real summary. A card already read stays read.
        let keep_body = body == "Agent turn completed";
        ctx.tx().prepare_cached("UPDATE notifications SET body=CASE WHEN ?1 THEN body ELSE ?2 END,created_at=?3 WHERE id=?4").bus()?
            .execute(params![keep_body, body, ctx.now, id]).bus()?;
        ctx.emit("notify.changed", json!({"notification_id":id,"project_id":s.project_id}));
        return Ok(false);
    }
    // A loud card replaces the unread one it repeats, so the phone, which alerts on new rows,
    // hears it too. A quiet first card is filed already read.
    if loud && unread {
        ctx.tx().prepare_cached(
            "UPDATE notifications SET read=1 WHERE project_id=?1 AND category=?2 AND title=?3 AND link=?4 AND read=0",
        ).bus()?.execute(params![s.project_id, category, title, link]).bus()?;
    }
    ctx.tx().prepare_cached(
        "INSERT INTO notifications(project_id,category,title,body,link,read,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
    ).bus()?.execute(params![s.project_id, category, title, body, link, !loud, ctx.now]).bus()?;
    if !loud {
        ctx.emit("notify.changed", json!({"project_id":s.project_id}));
    }
    Ok(loud)
}

fn notify_completion(ctx: &mut Ctx, s: &Session, summary: &str) -> Result<(), BusError> {
    let link = done_link(s);
    if !record_agent_notification(ctx, s, Ping::Finished, "agent_done", &format!("{} finished", s.name), summary, &link)? {
        return Ok(());
    }
    if let Some(message) = crate::handlers::notes::send_system(
        ctx.tx(),
        s.project_id,
        "*",
        &format!("{} finished: {}", s.name, summary),
        s.task_id,
        &ctx.now,
    )? {
        ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
    }
    ctx.emit(
        "notify.new",
        json!({"category":"agent_done", "project_id":s.project_id, "session":s.name}),
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum LaunchKind {
    Fresh,
    Resume,
}

/// A repository that tracks `.codex/hooks.json` is the person's to fix, not an engine fault.
fn codex_hooks_error(error: anyhow::Error) -> BusError {
    match error.downcast_ref::<crate::hooks::CodexHooksTracked>() {
        Some(tracked) => BusError::conflict("session.codex_hooks_tracked", tracked.to_string()),
        None => crate::engine::internal(error),
    }
}

/// Write one of Relay's launch files into a worktree the agent can write to. A symlink the
/// agent planted on the way — a directory of the path, or the file itself — would otherwise
/// carry Relay's write outside the sandbox: such links are replaced by real directories, and
/// the file lands through a fresh temporary file (never opened through a link) renamed over
/// whatever is at the path, which replaces a link rather than following it.
fn write_session_file(
    worktree: &Path,
    relative: &str,
    text: &str,
    code: &'static str,
) -> Result<(), BusError> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let fail = |what: &str, path: &Path, error: std::io::Error| {
        BusError::unavailable(code, format!("cannot {what} {}: {error}", path.display()))
    };
    let relative = Path::new(relative);
    let file_name = relative.file_name()
        .ok_or_else(|| BusError::internal("session file path has no file name"))?;
    let mut dir = worktree.to_path_buf();
    for part in relative.parent().into_iter().flat_map(Path::components) {
        let std::path::Component::Normal(part) = part else {
            return Err(BusError::internal(format!("session file path {} must be plain", relative.display())));
        };
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => continue,
            Ok(meta) if meta.file_type().is_symlink() => {
                tracing::warn!(path = %dir.display(), "replacing a symlink in the way of a session file");
                fs::remove_file(&dir).map_err(|error| fail("remove", &dir, error))?;
            }
            Ok(_) => return Err(BusError::unavailable(code, format!("{} is not a directory", dir.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(fail("inspect", &dir, error)),
        }
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) => {}
            Err(error) => return Err(fail("create", &dir, error)),
        }
    }
    let path = dir.join(file_name);
    let tmp = dir.join(format!(
        ".{}.relay-{}-{:x}",
        file_name.to_string_lossy(), std::process::id(), rand::random::<u64>(),
    ));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o644)
        .open(&tmp)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .and_then(|()| fs::rename(&tmp, &path));
    written.map_err(|error| {
        let _ = fs::remove_file(&tmp);
        fail("write", &path, error)
    })
}

/// What undoing Relay's wiring in one worktree needs from the store: every session name that
/// ever owned its hook path, and which provider adapters were ever installed there.
struct Teardown {
    names: Vec<String>,
    any_claude: bool,
    any_codex: bool,
}

fn read_teardown(conn: &Connection, worktree: &str) -> Result<Teardown, BusError> {
    let mut stmt = conn
        .prepare_cached("SELECT name, provider FROM sessions WHERE worktree=?1 ORDER BY id DESC")
        .bus()?;
    let rows = stmt
        .query_map([worktree], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    Ok(Teardown {
        any_claude: rows.iter().any(|(_, provider)| provider == "claude"),
        any_codex: rows.iter().any(|(_, provider)| provider == "codex"),
        names: rows.into_iter().map(|(name, _)| name).collect(),
    })
}

/// What closing a session does to its checkout's hooks: the full teardown when it is the last
/// session there, else a hand-over of the hook path to `survivor`.
struct Hooks {
    teardown: Option<Teardown>,
    survivor: Option<String>,
}

/// Another open session on the same checkout, newest first.
fn survivor_on(conn: &Connection, session_id: Id, worktree: &str) -> Result<Option<String>, BusError> {
    conn.prepare_cached(
        "SELECT name FROM sessions WHERE id!=?1 AND worktree=?2 AND state!='closed' ORDER BY id DESC LIMIT 1",
    ).bus()?.query_row(params![session_id, worktree], |row| row.get(0)).optional().bus()
}

/// Delete a departing session's hook directory, first handing the checkout's hook path to
/// `survivor` when the directory is the active one ([`crate::hooks::hand_over`]).
fn retire_hook_dir(instance: crate::Instance, repo: &Path, worktree: &Path, closing: &str, survivor: Option<&str>) {
    let _writes = crate::hooks::writes();
    let Some(survivor) = survivor else {
        crate::hooks::remove_hook_dir(repo, closing);
        return;
    };
    if let Err(error) = crate::hooks::hand_over(repo, worktree, closing, survivor, instance, &crate::hooks::relay_bin()) {
        tracing::warn!(session = closing, %survivor, %error, "handing the commit hook to a surviving session");
    }
}

/// The git and file half of a teardown. Subprocesses and file rewrites, so it runs with the
/// store unlocked; one `git config --get` covers every name in the chain.
fn run_teardown(repo: &Path, worktree: &Path, teardown: &Teardown) -> Result<(), BusError> {
    let _writes = crate::hooks::writes();
    crate::hooks::uninstall_git_any(repo, worktree, &teardown.names).bus()?;
    if teardown.any_claude {
        crate::hooks::uninstall_claude(worktree).bus()?;
    }
    if teardown.any_codex {
        crate::hooks::uninstall_codex(worktree).bus()?;
    }
    Ok(())
}

/// A checkout a removal would delete, and the sessions on it.
pub(crate) type Checkout = (PathBuf, Vec<String>);

/// What removing one checkout would delete that git could not bring back.
pub(crate) struct Dirt {
    /// Entries `git status` lists; `None` when a scan failed, size unknown.
    changed: Option<usize>,
    /// Git-ignored secrets and local state nothing rebuilds: an `.env`, a keystore,
    /// `local.properties`, a local database ([`crate::branch_cleanup::ignored_keepers`]). No
    /// status without `--ignored` shows them.
    ignored: Vec<String>,
    /// Linked repositories (`.relay/linked/`) holding work their remote does not have
    /// ([`crate::linked::unpushed`]). Git status of the checkout never sees them: `.relay/` is
    /// excluded.
    linked: Vec<(String, relay_bus::ops::session::LinkedRepoUnpushed)>,
}

impl Dirt {
    fn unknown() -> Self { Dirt { changed: None, ignored: Vec::new(), linked: Vec::new() } }
    fn any(&self) -> bool { self.changed != Some(0) || !self.ignored.is_empty() || !self.linked.is_empty() }
}

/// The checkouts of `checkouts` a removal would lose work from: tracked edits, staged changes,
/// untracked files (RA-405) or git-ignored keepers ([`crate::branch_cleanup::ignored_keepers`]).
/// Two `git status` per checkout, each bounded by its timeout and run a few checkouts at a time
/// as `session.restorable` does, so this runs with the store unlocked. A checkout already gone
/// is skipped; one whose scan fails counts as dirty, size unknown.
pub(crate) fn dirty_checkouts(checkouts: &[Checkout]) -> Vec<(&Checkout, Dirt)> {
    let live: Vec<&Checkout> = checkouts.iter().filter(|(wt, _)| wt.exists()).collect();
    if live.is_empty() { return Vec::new(); }
    let scan = |wt: &Path| {
        // Without a `.git` of its own (half deleted) git would answer for the primary checkout
        // that holds it: the primary's changes and ignored files are not this checkout's to lose.
        let Ok(files) = worktree::checkout_changes(wt, worktree::Untracked::Directories) else { return Dirt::unknown() };
        let ignored = if wt.join(".git").exists() { crate::branch_cleanup::ignored_keepers(wt) } else { Ok(Vec::new()) };
        let linked = crate::linked::holding_work(wt);
        match ignored {
            Ok(ignored) => Dirt { changed: Some(files.len()), ignored, linked },
            Err(_) => Dirt { linked, ..Dirt::unknown() },
        }
    };
    let per_worker = live.len().div_ceil(4).max(1);
    let found: Vec<Dirt> = std::thread::scope(|scope| {
        let workers: Vec<_> = live.chunks(per_worker)
            .map(|chunk| scope.spawn(move || chunk.iter().map(|(wt, _)| scan(wt)).collect::<Vec<_>>()))
            .collect();
        workers.into_iter().zip(live.chunks(per_worker))
            .flat_map(|(worker, chunk)| worker.join().unwrap_or_else(|_| chunk.iter().map(|_| Dirt::unknown()).collect()))
            .collect()
    });
    live.into_iter().zip(found).filter(|(_, dirt)| dirt.any()).collect()
}

/// Refuse `worktree.dirty` when a checkout a removal is about to delete holds work
/// [`dirty_checkouts`] finds. Each entry is a checkout and the sessions on it.
pub(crate) fn refuse_dirty(checkouts: &[Checkout]) -> Result<(), BusError> {
    let dirty = dirty_checkouts(checkouts);
    if dirty.is_empty() { return Ok(()); }
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let describe = |((wt, sessions), dirt): &(&Checkout, Dirt)| {
        let mut what = Vec::new();
        match dirt.changed {
            Some(0) => {}
            Some(n) => what.push(format!("{n} uncommitted change{}", plural(n))),
            None => what.push("changes that could not be checked".to_string()),
        }
        if !dirt.ignored.is_empty() {
            let n = dirt.ignored.len();
            let named = dirt.ignored.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
            what.push(format!("{n} git-ignored file{} ({named}{})", plural(n), if n > 3 { ", …" } else { "" }));
        }
        for (name, found) in &dirt.linked {
            what.push(format!("linked repo {name} with {}", crate::linked::summary(found)));
        }
        let whose = if sessions.is_empty() { String::new() } else { format!(" (session {})", sessions.join(", ")) };
        format!("{}{whose} has {}", wt.display(), what.join(" and "))
    };
    let message = match dirty.as_slice() {
        [one] => format!("worktree {}; removing it would delete them", describe(one)),
        many => format!("{} worktrees hold uncommitted work that removing them would delete: {}",
            many.len(), many.iter().map(describe).collect::<Vec<_>>().join("; ")),
    };
    let details: Vec<Value> = dirty.iter()
        .map(|((wt, sessions), dirt)| json!({"worktree": wt.display().to_string(), "sessions": sessions, "changed": dirt.changed,
            "ignored": dirt.ignored.iter().take(20).collect::<Vec<_>>(),
            "linked": dirt.linked.iter().map(|(name, found)| json!({"name": name, "unpushed": found})).collect::<Vec<_>>()}))
        .collect();
    Err(BusError::conflict("worktree.dirty", message)
        .with_details(json!({"worktrees": details}))
        .with_hint("commit or stash the changes, push a linked repo's commits (and move out any git-ignored files worth keeping) first, or pass discard_changes: true to delete them with the worktree"))
}

/// Kill a PTY that is already out of the registry without making the caller wait for it.
/// The SIGTERM grace and the SIGKILL fallback can take seconds; nothing reads the child again.
fn kill_detached(pty: Arc<Pty>, grace: Duration) {
    let spawned = std::thread::Builder::new()
        .name(format!("pty-kill-{}", pty.pid()))
        .spawn({
            let pty = pty.clone();
            move || pty.kill(grace)
        });
    if spawned.is_err() {
        pty.kill(grace);
    }
}

/// Record that a session's child is gone after a request that stopped it failed before it
/// could write the session's next state. On its own thread: the failing request may still be
/// running inside a transaction that holds the store (a guardrail replay).
fn mark_exited_detached(engine: &Engine, sid: Id, project_id: Id) {
    let Some(engine) = engine.arc() else { return };
    std::thread::spawn(move || {
        let _ = engine.system_write("session.stop.failed", None, Some(project_id), Some(sid), json!({}), |tx, now| {
            let changed = tx.execute(
                "UPDATE sessions SET state='exited',pid=NULL,updated_at=?1
                 WHERE id=?2 AND state IN ('spawning','running','idle','blocked')",
                params![now, sid],
            ).map_err(crate::engine::internal)?;
            let mut events = Vec::new();
            if changed > 0 {
                if let Some(row) = sessions::by_id(tx, sid)? {
                    events.push(("session.changed".into(), serde_json::to_value(&row.session).unwrap_or(Value::Null)));
                }
            }
            Ok(((), events))
        });
    });
}

/// The locked half of retiring a session, shared by `session.close` and
/// `session.discard_restorable` so the two cannot drift apart: the row goes `closed`, and
/// everything that hung off it while it lived — PTY, partner link, scrollback, claims, open
/// holds, device leases, a merged branch — goes with it.
fn retire_session(ctx: &mut Ctx, s: &Session) -> Result<(), BusError> {
    if let Some(pty) = ctx.engine().take_pty(s.id) {
        // Provider shutdown hooks may need the bus, and the grace period is the child's to
        // spend: neither is worth a request, let alone the store lock. The row is `closed`
        // by the time the child exits, so its exit callback changes nothing.
        ctx.after_commit(move |_| kill_detached(pty, Duration::from_millis(150)));
    }
    ctx.tx().execute("UPDATE sessions SET state = 'closed', pid = NULL, closed_at = ?1, updated_at = ?1 WHERE id = ?2", params![ctx.now, s.id]).bus()?;
    // A partner left pointing at the closed session takes the next member still linked to it:
    // a reviewer whose first builder closes keeps its second one, so the group stays whole
    // for validate_pair and assert_own. Anyone else is unpaired (RA-398).
    ctx.tx().execute(
        "UPDATE sessions SET updated_at=?1, pair_with=(
             SELECT other.name FROM sessions other WHERE other.pair_with=sessions.name AND other.state!='closed'
             AND other.worktree=sessions.worktree ORDER BY other.id LIMIT 1)
         WHERE pair_with=?2 AND state!='closed'",
        params![ctx.now, s.name],
    ).bus()?;
    ctx.tx().execute("DELETE FROM session_scrollback WHERE session_id=?1", [s.id]).bus()?;
    release_claims(ctx, s)?;
    let expired = ctx.tx().execute(
        "UPDATE holds SET state = 'expired', resolved_at = ?1, resolved_by = 'system'
         WHERE session_id = ?2 AND state = 'open'",
        params![ctx.now, s.id],
    ).bus()?;
    let row = sessions::by_id(ctx.tx(), s.id)?.ok_or_else(|| BusError::internal("session vanished"))?;
    emit_session(ctx, &row.session);
    if expired > 0 {
        ctx.emit("guardrail.resolved", json!({
            "session": s.name,
            "state": "expired",
            "count": expired,
        }));
    }
    ctx.emit("worktree.changed", json!({ "project_id": s.project_id }));
    // Off the request: the session's device leases go, and its branch is deleted if its
    // work is already merged (or kept, with the reason audited) — `branch_cleanup`.
    let (session_id, project_id, branch) = (s.id, s.project_id, s.branch.clone());
    ctx.after_commit(move |engine| {
        crate::handlers::device_lease::release_session(&engine, session_id);
        // Tests drive the cleanup synchronously instead (`branch_cleanup::run`).
        if engine.instance != crate::Instance::Test {
            crate::branch_cleanup::after_close(engine, project_id, branch);
        }
    });
    Ok(())
}

/// Everything a launch reads from the store, gathered in one short read. The hooks, skill
/// folders and brief files it implies are then written with the store unlocked (D149).
struct LaunchPlan {
    row: Row_,
    kind: LaunchKind,
    cmd: PathBuf,
    project_path: PathBuf,
    relay_bin: PathBuf,
    plugin_servers: Vec<crate::plugins::LaunchServer>,
    skills: Vec<crate::skills::Plan>,
    brief_compact: String,
    brief_skills: String,
    write_roots: Vec<PathBuf>,
    initial_scrollback: Vec<u8>,
}

/// A launch whose external work is done: only the PTY and the row update remain.
struct PreparedLaunch {
    session_id: Id,
    name: String,
    /// The state and epoch the row had when it was read. A launch whose row moved on meanwhile
    /// (closed, parked, launched by a concurrent request) is refused rather than doubled.
    expect: SessionState,
    epoch: u64,
    kind: LaunchKind,
    /// `session.spawn {prompt}`: the launch assignment to record with the spawn.
    launch_prompt: Option<Option<String>>,
    /// `session.clear_restorable`: forget the provider conversation and its scrollback.
    clear: bool,
    spec: SpawnSpec,
}

fn plan_launch(conn: &Connection, engine: &Engine, row: Row_, kind: LaunchKind, git: &str) -> Result<LaunchPlan, BusError> {
    let cmd = crate::providers::executable(conn, row.session.provider)?;
    let relay_bin = crate::hooks::relay_bin();
    let project = crate::handlers::workspace::get_project(conn, row.session.project_id)?;
    // A plugin that is on brings its MCP servers to every agent of the project (D159).
    let plugin_servers = crate::plugins::mcp_servers(conn, row.session.project_id, &relay_bin).bus()?;
    let cwd = PathBuf::from(&row.session.worktree);
    let mut skills = Vec::new();
    match crate::skills::plan(conn, &cwd, row.session.project_id) {
        Ok(plan) => skills.push(plan),
        Err(error) => tracing::warn!(session = %row.session.name, error = %error, "planning skills"),
    }
    match crate::skills::plan_user(conn, engine.instance) {
        Ok(plan) => skills.push(plan),
        Err(error) => tracing::warn!(session = %row.session.name, error = %error, "planning user skills"),
    }
    // The brief is delivered on *every* spawn, not only with a dispatched task: an agent with
    // no assignment is exactly the one that most needs to know who its peers are (D101).
    let brief = crate::awareness::brief(conn, &row.session.name, Some(engine), git)?;
    let cfg = crate::guardrail::config(conn, Some(row.session.project_id))?;
    let write_roots = crate::guardrail::write_roots(&cfg, &cwd).into_iter().filter(|root| root != &cwd).collect();
    let initial_scrollback = match kind {
        LaunchKind::Fresh => Vec::new(),
        // An exited session's history is still in its dead PTY: nothing saves it at exit, so
        // the stored copy is stale or missing until the engine shuts down.
        LaunchKind::Resume => match engine.pty(row.session.id).filter(|_| row.session.state == SessionState::Exited) {
            Some(dead) => dead.scrollback(None).0.into_bytes(),
            None => sessions::load_scrollback(conn, row.session.id)?
                .map(|saved| saved.0.into_bytes())
                .unwrap_or_default(),
        },
    };
    Ok(LaunchPlan {
        row, kind, cmd, project_path: PathBuf::from(&project.path), relay_bin, plugin_servers, skills,
        brief_compact: brief.compact, brief_skills: brief.parts.skills, write_roots, initial_scrollback,
    })
}

/// The external half of a launch: git hooks, provider adapters, skill folders, the brief and
/// role files. All of it is idempotent, so a launch refused afterwards leaves nothing wrong.
fn prepare_launch(engine: &Engine, plan: LaunchPlan) -> Result<PreparedLaunch, BusError> {
    let LaunchPlan { row, kind, cmd, project_path, relay_bin, plugin_servers, skills, brief_compact, brief_skills, write_roots, initial_scrollback } = plan;
    let instance = engine.instance;
    let cwd = PathBuf::from(&row.session.worktree);
    if !cwd.is_dir() {
        return Err(BusError::conflict(
            "session.worktree_missing",
            format!("worktree {} is gone", cwd.display()),
        ));
    }
    // A pooled checkout that lost its `.git` (a removal or a crash half done) is a plain folder
    // inside the primary: git there answers for the primary, so the hooks would be installed
    // on it and the agent would work on its branch. Only a checkout git calls its own is used.
    if cwd.starts_with(worktree::pool_dir(&project_path)) {
        let toplevel = cwd.join(".git").exists()
            .then(|| worktree::git_mutate(&cwd, &["rev-parse", "--show-toplevel"]).ok())
            .flatten()
            .and_then(|top| std::fs::canonicalize(top.trim()).ok());
        if toplevel.is_none() || toplevel != std::fs::canonicalize(&cwd).ok() {
            return Err(BusError::conflict(
                "session.worktree_missing",
                format!("worktree {} is no longer a git checkout (its .git is gone); git there would act on the primary checkout instead", cwd.display()),
            ).with_hint("discard the session (session.discard_restorable) and start a new one; move out anything in the folder worth keeping first"));
        }
    }
    // Concurrent launches (two sessions of one repository, or a double-clicked Start) would
    // otherwise race on `.git/config` and the provider adapters' temp files.
    let writes = crate::hooks::writes();
    crate::hooks::install_git(&project_path, &cwd, &row.session.name, instance, &relay_bin).bus()?;
    if row.session.provider == Provider::Claude {
        crate::hooks::install_claude(&cwd, instance, &relay_bin).bus()?;
        crate::hooks::add_claude_mcp_servers(&cwd, &plugin_servers).bus()?;
    } else if row.session.provider == Provider::Codex {
        crate::hooks::install_codex(&cwd, instance, &relay_bin).map_err(codex_hooks_error)?;
    }
    // Skill folders have their own lock (`skills::apply`); copying them under this one held up
    // every other launch, create, close and bus commit for as long as the copy took (RA-322).
    drop(writes);
    // Every enabled skill becomes a real provider skill folder in this worktree, whatever
    // project it belongs to (D147); Codex reads skills only from its own home, so the machine
    // folders are refreshed too. A skill folder that cannot be written is never worth failing
    // a launch over, so this reports and continues.
    for plan in &skills {
        if let Err(error) = crate::skills::apply(plan, &engine.store) {
            tracing::warn!(session = %row.session.name, error = %error, "materializing skills");
        }
    }
    // The file lives inside the private worktree, so it may carry the launch assignment. The
    // injected copy may not: for Codex it ends up in argv, which anyone can read with `ps`.
    let mut on_disk = brief_compact.clone();
    if let Some(assignment) = row
        .launch_prompt
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        // Relaunched, the session may be long past it: name it for what it now is (RA-397).
        let label = if row.session.spawned_at.is_none() { "Launch assignment" } else { "Original launch assignment (from the first launch; may already be done)" };
        on_disk.push_str(&format!("\n\n{label}:\n{assignment}"));
    }
    let brief_path = crate::awareness::session_brief_path(&row.session.name);
    let role_path = crate::providers::role_instructions_relative(&row.session.name);
    write_session_file(&cwd, &brief_path, &on_disk, "session.brief_write_failed")?;
    write_session_file(
        &cwd,
        SESSION_SKILLS_PATH,
        &brief_skills,
        "session.brief_write_failed",
    )?;
    let instructions = format!(
        "{}\n\n{}",
        crate::providers::role_instructions(row.session.role),
        brief_compact,
    );
    write_session_file(
        &cwd,
        &role_path,
        &instructions,
        "session.role_instructions_write_failed",
    )?;
    let mut args = match kind {
        LaunchKind::Fresh => crate::providers::driver(row.session.provider).args(
            &row.session,
            crate::providers::Launch::Fresh,
            Some(&brief_compact),
        ),
        LaunchKind::Resume => crate::providers::driver(row.session.provider).args(
            &row.session,
            crate::providers::Launch::Resume {
                provider_ref: row.session.provider_ref.as_deref().filter(|value| crate::providers::is_provider_ref(value)),
            },
            Some(&brief_compact),
        ),
    };
    if row.session.provider == Provider::Codex {
        args.extend([
            "--config".into(),
            crate::providers::codex_notify_config(&relay_bin),
        ]);
        for (name, command, server_args, env) in &plugin_servers {
            args.extend(crate::providers::codex_mcp_config(name, command, server_args, env));
        }
    }
    for root in write_roots {
        // Provider directory grants need a real directory. Test engines never
        // create directories in the user's provider homes.
        if instance != crate::Instance::Test && !root.is_dir() {
            fs::create_dir_all(&root).map_err(|error| BusError::unavailable(
                "session.write_root", format!("cannot prepare {}: {error}", root.display()),
            ))?;
        }
        args.extend(["--add-dir".into(), root.display().to_string()]);
    }
    let spec = SpawnSpec {
        cmd: cmd.display().to_string(),
        args,
        env: vec![
            ("RELAY_SESSION".into(), row.session.name.clone()),
            ("RELAY_TOKEN".into(), row.token.clone()),
            ("RELAY_INSTANCE".into(), instance.as_str().to_string()),
            ("RELAY_PROJECT".into(), row.session.project_id.to_string()),
            ("RELAY_WORKTREE".into(), row.session.worktree.clone()),
            ("RELAY_BRIEF".into(), brief_path),
            ("RELAY_BIN".into(), relay_bin.display().to_string()),
            (
                "RELAY_STORE".into(),
                engine.store.path().display().to_string(),
            ),
        ],
        cwd,
        cols: 120,
        rows: 40,
        epoch: row.epoch + 1,
        initial_scrollback,
    };
    Ok(PreparedLaunch {
        session_id: row.session.id,
        name: row.session.name.clone(),
        expect: row.session.state,
        epoch: row.epoch,
        kind,
        launch_prompt: None,
        clear: false,
        spec,
    })
}

/// The brief's git lines for `session`. They walk commit history, so the store is taken
/// only to find the checkout and released before git is read.
fn brief_git(ctx: &crate::engine::Unlocked, session: &str) -> Result<String, BusError> {
    let (worktree, branch, base) = ctx.read(|conn| {
        let row = sessions::by_name(conn, session)?;
        let project = crate::handlers::workspace::get_project(conn, row.session.project_id)?;
        Ok((row.session.worktree, row.session.branch, project.base_branch))
    })?;
    Ok(crate::handlers::git::briefing_state(Path::new(&worktree), &branch, &base))
}

/// Read and prepare a launch from the unlocked phase of a staged handler.
fn stage_launch(
    ctx: &crate::engine::Unlocked,
    session: &str,
    kind: LaunchKind,
    check: impl FnOnce(&Row_) -> Result<(), BusError>,
    adjust: impl FnOnce(&mut Row_),
) -> Result<PreparedLaunch, BusError> {
    let engine = ctx.engine();
    let git = brief_git(ctx, session)?;
    let plan = ctx.read(|conn| {
        let mut row = sessions::by_name(conn, session)?;
        check(&row)?;
        adjust(&mut row);
        plan_launch(conn, engine, row, kind, &git)
    })?;
    prepare_launch(engine, plan)
}

/// The locked half of a launch: recheck the row, start the PTY, record it. Short by design —
/// everything slow already happened in [`prepare_launch`].
fn finish_launch(ctx: &mut Ctx, prepared: PreparedLaunch) -> Result<Session, BusError> {
    let PreparedLaunch { session_id: sid, name, expect, epoch: seen_epoch, kind, launch_prompt, clear, spec } = prepared;
    let current = sessions::by_id(ctx.tx(), sid)?
        .ok_or_else(|| BusError::not_found("session.not_found", format!("no session {name}")))?;
    if current.session.state != expect || current.epoch != seen_epoch {
        let state = sessions::state_str(current.session.state);
        return Err(if sessions::is_live(current.session.state) {
            BusError::conflict("session.already_spawned", format!("session {name} is {state}"))
        } else {
            BusError::conflict("session.state", format!("session {name} became {state} while it was launching"))
        });
    }
    if let Some(launch_prompt) = launch_prompt {
        ctx.tx().execute(
            "UPDATE sessions SET launch_prompt=?1,updated_at=?2 WHERE id=?3",
            params![launch_prompt, ctx.now, sid],
        ).bus()?;
    }
    if clear {
        // Unlike Resume, Clear must not pass the provider's saved conversation. Unlike Discard,
        // it keeps the durable Relay session and worktree, then launches a new PTY epoch in place.
        ctx.tx().execute(
            "UPDATE sessions SET provider_ref=NULL,restore_reason=NULL,exit_code=NULL,updated_at=?1 WHERE id=?2",
            params![ctx.now, sid],
        ).bus()?;
        ctx.tx().execute("DELETE FROM session_scrollback WHERE session_id=?1", [sid]).bus()?;
    }
    let epoch = spec.epoch;
    let engine = ctx.engine().arc().map(|engine| Arc::downgrade(&engine));
    let project_id = current.session.project_id;
    let req_id = ctx.req_id;
    let pty = Pty::spawn(spec, move |code| {
        let Some(engine) = engine.and_then(|weak| weak.upgrade()) else { return };
        let exited = engine.system_write(
            "session.spawn.completed", Some(req_id), Some(project_id), Some(sid), json!({ "exit_code": code }),
            |tx, now| {
                let mut events = Vec::new();
                let changed = tx.execute(
                    "UPDATE sessions SET state='exited',exit_code=?1,pid=NULL,updated_at=?2
                     WHERE id=?3 AND epoch=?4 AND state IN ('spawning','running','idle','blocked')",
                    params![code, now, sid, epoch as i64],
                ).map_err(crate::engine::internal)?;
                if changed > 0 {
                    if let Some(row) = sessions::by_id(tx, sid)? {
                        if row.session.role == Role::Builder {
                          if let Some(task_id) = row.session.task_id {
                            let changed = exit_fails_task(tx, &row.session, task_id, now).map_err(crate::engine::internal)?;
                            if changed > 0 {
                                events.push(("task.changed".into(), json!({"task_id":task_id,"state":"failed"})));
                            }
                          }
                        }
                        events.push(("session.changed".into(), serde_json::to_value(&row.session).unwrap_or(Value::Null)));
                    }
                }
                Ok((changed > 0, events))
            },
        );
        if exited.unwrap_or(false) {
            crate::handlers::device_lease::release_session(&engine, sid);
        }
    }).map_err(|error| match error.downcast_ref::<crate::pty::CwdMissing>() {
        Some(gone) => BusError::conflict("session.worktree_missing", gone.to_string()),
        None => BusError::unavailable("session.spawn_failed", error.to_string()),
    })?;
    ctx.tx().execute(
        "UPDATE sessions SET state='running',pid=?1,epoch=?2,spawned_at=COALESCE(spawned_at,?3),
         exit_code=NULL,restore_reason=NULL,done_pending_stop=NULL,updated_at=?3 WHERE id=?4",
        params![pty.pid() as i64, epoch as i64, ctx.now, sid],
    ).bus()?;
    // Into the agents' slice, where gaming mode can cap it and everything it starts.
    crate::gaming::adopt(pty.pid(), &name);
    if let Some(old) = ctx.engine().set_pty(sid, &name, pty) {
        kill_detached(old, Duration::from_millis(500));
    }
    if expect == SessionState::Exited && current.session.role == Role::Builder {
        // The exit marked the builder's task failed; relaunched, it is being worked on again.
        if let Some(task_id) = current.session.task_id {
            let revived = ctx.tx().execute(
                "UPDATE tasks SET state='running',updated_at=?1 WHERE id=?2 AND col='active' AND state='failed' AND deleted_at IS NULL",
                params![ctx.now, task_id],
            ).bus()?;
            if revived > 0 {
                ctx.emit("task.changed", json!({"task_id": task_id, "state": "running"}));
            }
        }
    }
    let updated =
        sessions::by_id(ctx.tx(), sid)?.ok_or_else(|| BusError::internal("session vanished"))?;
    if matches!(kind, LaunchKind::Fresh) {
        if let Some(prompt) = launch_nudge(&updated, current.session.spawned_at.is_none()) {
            ctx.after_commit(move |engine| {
                if let Some(pty) = engine.pty(sid) {
                    if let Err(error) = pty.write(format!("{prompt}\r").as_bytes()) {
                        tracing::warn!(session_id = sid, error = %error, "could not send fresh-session start prompt");
                    }
                }
            });
        }
    }
    emit_session(ctx, &updated.session);
    Ok(updated.session)
}

/// A builder's exit marks its current task failed, unless this builder already finished it
/// or another live builder on the same checkout is still working on it (RA-399).
fn exit_fails_task(conn: &Connection, s: &Session, task_id: Id, now: &str) -> rusqlite::Result<usize> {
    conn.prepare_cached(
        "UPDATE tasks SET state='failed',updated_at=?1 WHERE id=?2 AND col='active' AND deleted_at IS NULL
         AND NOT EXISTS(SELECT 1 FROM task_sessions WHERE task_id=?2 AND session_id=?3 AND completed_at IS NOT NULL)
         AND NOT EXISTS(SELECT 1 FROM task_sessions ts JOIN sessions o ON o.id=ts.session_id
             WHERE ts.task_id=?2 AND ts.session_id!=?3 AND ts.completed_at IS NULL AND o.role='builder'
             AND o.worktree=?4 AND o.state IN ('spawning','running','idle','blocked'))",
    )?.execute(params![now, task_id, s.id, s.worktree])
}

/// Model, effort, task and module as `session.update` would write them.
type UpdateValues = (Option<String>, Option<String>, Option<Id>, Option<Id>);

/// What `session.update` would write, refused as it would be: both of its phases ask, so a
/// rename never runs for an update the transaction would then refuse.
fn check_update(conn: &Connection, before: &Row_, p: &UpdateIn) -> Result<UpdateValues, BusError> {
    let s = &before.session;
    let spawned = before.epoch > 0 || s.spawned_at.is_some();
    if spawned && (p.branch.is_some() || p.model.is_some() || p.effort.is_some()) {
        return Err(BusError::conflict("session.already_spawned", "branch, model, and effort cannot change after first spawn"));
    }
    let model = p.model.clone().map(Some).unwrap_or_else(|| s.model.clone());
    let effort = p.effort.clone().map(Some).unwrap_or_else(|| s.effort.clone());
    crate::providers::validate_options(s.provider, model.as_deref(), effort.as_deref())?;
    let task_id = p.task_id.unwrap_or(s.task_id);
    let module_id = p.module_id.unwrap_or(s.module_id);
    check_task_module(conn, s.project_id, task_id, module_id)?;
    Ok((model, effort, task_id, module_id))
}

/// Mirror the session state we just persisted onto its live PTY. `session.input` reads this
/// instead of the session row, so the idle→running edge still produces exactly one write while
/// every other keystroke stays off the store entirely (D148).
fn note_pty_state(ctx: &Ctx, session_id: Id, state: &str) {
    if let Some(pty) = ctx.engine().pty(session_id) {
        pty.set_idle(state == "idle");
    }
}

/// Reserve the public name across the unlocked checkout phase. A failed checkout
/// releases the name reservation but preserves any files Git already created.
struct PreparedCreate {
    engine: std::sync::Weak<Engine>,
    name: String,
    project: relay_bus::types::Project,
    /// The checkout the session will run in, resolved (and for "new", created) unlocked.
    checkout: Option<(String, String)>,
    /// Relay's commit hook was installed for `name` in that checkout before the row exists.
    hooked: bool,
    /// Read once in the prepare phase, so both phases agree even if a plugin is switched
    /// meanwhile (D160).
    default_checkout: &'static str,
}

impl Drop for PreparedCreate {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.upgrade() {
            engine.creating_sessions.lock().unwrap().remove(&self.name);
        }
    }
}

fn validate_create(conn: &Connection, p: &CreateIn) -> Result<relay_bus::types::Project, BusError> {
    let project = crate::handlers::workspace::get_project(conn, p.project_id)?;
    crate::providers::validate_options(p.provider, p.model.as_deref(), p.effort.as_deref())?;
    check_task_module(conn, project.id, p.task_id, p.module_id)?;
    Ok(project)
}

/// The PAIR partner a create joins, with every refusal that depends on it. Read-only, so the
/// prepare phase can refuse a bad pairing before it touches the partner's checkout.
fn validate_pair(conn: &Connection, p: &CreateIn, project_id: Id, name: &str) -> Result<Row_, BusError> {
    let pair = sessions::by_name(conn, name)?;
    if pair.session.project_id != project_id { return Err(BusError::invalid("session.pair_project", "PAIR sessions must belong to the same project")); }
    if pair.session.pair_with.is_some() {
        let role = p.role.unwrap_or(Role::Builder);
        let group = sessions::review_group(conn, &pair.session)?;
        let builders = group.iter().filter(|(_, role)| *role == Role::Builder).count();
        let reviewers = group.iter().filter(|(_, role)| *role == Role::Reviewer).count();
        if role != Role::Builder || pair.session.role != Role::Reviewer || reviewers != 1 {
            return Err(BusError::conflict("session.pair_exists", format!("session {name} is already paired")));
        }
        if builders >= 2 {
            return Err(BusError::conflict("session.review_group_full", "one reviewer can supervise at most two builders"));
        }
    }
    if p.branch.as_deref().is_some_and(|branch| branch != pair.session.branch) {
        return Err(BusError::invalid("session.pair_branch", "PAIR sessions share one branch"));
    }
    if p.worktree.as_deref().is_some_and(|worktree| worktree != pair.session.worktree) {
        return Err(BusError::invalid("session.pair_worktree", "omit worktree for a PAIR session; Relay reuses its partner's checkout"));
    }
    Ok(pair)
}

fn finish_create(ctx: &mut Ctx, p: &CreateIn, prepared: &mut PreparedCreate) -> Result<Session, BusError> {
    // Recheck mutable records after the external work completes.
    let project = validate_create(ctx.tx(), p)?;
    if project.path != prepared.project.path || project.base_branch != prepared.project.base_branch {
        return Err(BusError::conflict("session.project_changed",
            "Project changed during creation; any created worktree is preserved"));
    }
    let pair = match p.pair_with.as_deref() {
        Some(name) => Some(validate_pair(ctx.tx(), p, project.id, name)?),
        None => None,
    };
    let name = prepared.name.clone();
    let token = sessions::new_token();
    let (worktree_path, branch) = prepared.checkout.take().ok_or_else(|| BusError::internal("checkout was not prepared"))?;
    if let Some(pair) = &pair {
        if worktree_path != pair.session.worktree {
            return Err(BusError::invalid("session.pair_worktree", "PAIR sessions must share one worktree"));
        }
    }
    ctx.tx().execute(
        "INSERT INTO sessions(name, project_id, provider, role, model, effort, branch, worktree, task_id, module_id, pair_with,
                              bus_writes, allow_ui, state, token, epoch, created_at, updated_at, launch_prompt)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'created', ?14, 0, ?15, ?15, ?16)",
        params![name, project.id, sessions::provider_str(p.provider), sessions::role_str(p.role.unwrap_or(relay_bus::types::Role::Builder)),
                p.model, p.effort, branch, worktree_path, p.task_id, p.module_id, p.pair_with,
                p.bus_writes.unwrap_or(false) as i64, p.allow_ui.unwrap_or(false) as i64, token, ctx.now,
                p.prompt.as_deref().filter(|prompt| !prompt.trim().is_empty())],
    ).bus()?;
    let id = ctx.tx().last_insert_rowid();
    if let Some(pair) = &pair {
        if pair.session.pair_with.is_none() {
        ctx.tx().execute(
            "UPDATE sessions SET pair_with=?1,updated_at=?2 WHERE id=?3",
            params![name, ctx.now, pair.session.id],
        ).bus()?;
        }
    }
    let row = sessions::by_id(ctx.tx(), id)?.ok_or_else(|| BusError::internal("session vanished"))?;
    if let Some(task_id) = p.task_id {
        // A session joining a pair queues its task for the whole group, as dispatch does.
        enqueue(ctx.tx(), task_id, &group_ids(ctx.tx(), &row.session)?)?;
    }
    emit_session(ctx, &row.session);
    ctx.emit("worktree.changed", json!({ "project_id": project.id }));
    Ok(row.session)
}

/// The branch Relay names for a new session. Session names are reused once a session closes,
/// and its `relay/<name>` branch may outlive it, unmerged; a session given that name starts
/// from base on a branch of its own rather than silently picking up the earlier one's work.
/// A branch the caller names is reattached on purpose, so this applies only to Relay's choice.
fn own_branch(repo: &Path, name: &str) -> Result<String, BusError> {
    let first = worktree::branch_for(name);
    for n in 1..1000 {
        let candidate = if n == 1 { first.clone() } else { format!("{first}-{n}") };
        if !super::git::existing_worktree_branch(repo, Some(&candidate))? {
            return Ok(candidate);
        }
    }
    Err(BusError::conflict("session.branch", format!("every {first}-<n> branch is taken")))
}

pub fn register(e: &mut Engine) {
    e.register_staged::<Create, _>(|ctx, p| {
        let mut prepared = ctx.read(|conn| {
            let project = validate_create(conn, p)?;
            let default_checkout = crate::plugins::default_checkout(conn, project.id).bus()?;
            let mut reservations = ctx.engine().creating_sessions.lock().unwrap();
            for _ in 0..200 {
                let name = sessions::new_name(conn).bus()?;
                if worktree::pooled_path(Path::new(&project.path), &name).exists()
                    || !reservations.insert(name.clone()) { continue; }
                return Ok(PreparedCreate {
                    engine: Arc::downgrade(&ctx.engine().arc().ok_or_else(|| BusError::internal("engine unavailable"))?),
                    name, project, checkout: None, hooked: false, default_checkout,
                });
            }
            Err(BusError::conflict("session.names_busy", "Could not reserve a free session name"))
        })?;
        // A refused pairing is refused here, before anything touches the partner's checkout.
        let pair_worktree = match p.pair_with.as_deref() {
            Some(name) => Some(ctx.read(|conn| validate_pair(conn, p, prepared.project.id, name))?.session.worktree),
            None => None,
        };
        // Fetch, checkout, `git worktree list` and the hook install all shell out, and fetch and
        // checkout can invoke slow network/LFS filters. None of it belongs under the global
        // store mutex: existing sessions must remain responsive.
        let repo = PathBuf::from(&prepared.project.path);
        let requested = p.worktree.as_deref().or(pair_worktree.as_deref()).unwrap_or(prepared.default_checkout);
        let (worktree_path, branch) = match requested {
            "primary" => {
                let all = worktree::list_with_dirty(&repo, false).bus()?;
                let primary = all.first().ok_or_else(|| BusError::internal("no primary worktree"))?;
                (primary.path.clone(), primary.branch.clone())
            }
            "new" => {
                let branch = match p.branch.clone() {
                    Some(branch) => branch,
                    None => own_branch(&repo, &prepared.name)?,
                };
                let path = worktree::pooled_path(&repo, &prepared.name);
                let start = super::git::new_worktree_start(&repo, &branch, &prepared.project.base_branch, None)?;
                let wt = worktree::create_at(&repo, &path, &branch, &start)
                    .map_err(|e| BusError::conflict("worktree.create_failed", e.to_string()))?;
                // Its `target/` arrives on a thread of its own: a copy of the project's prebuilt
                // dependencies, which its first cargo command waits for (build_seed).
                if let Some(engine) = ctx.engine().arc() {
                    crate::build_seed::seed_new_worktree(&engine, ctx.instance(), prepared.project.id, &repo, Path::new(&wt.path));
                }
                (wt.path, wt.branch)
            }
            other => {
                let path = Path::new(other);
                if !path.is_absolute() || !path.join(".git").exists() {
                    return Err(BusError::invalid("session.worktree", format!("{other:?} is not an existing worktree path")));
                }
                let all = worktree::list_with_dirty(&repo, false).bus()?;
                let want = std::fs::canonicalize(path).map(|p| p.display().to_string()).unwrap_or(other.to_string());
                let wt = all.into_iter().find(|w| w.path == want)
                    .ok_or_else(|| BusError::invalid("session.worktree", format!("{other:?} is not a worktree of this project")))?;
                (wt.path, wt.branch)
            }
        };
        {
            let _writes = crate::hooks::writes();
            crate::hooks::install_git(&repo, Path::new(&worktree_path), &prepared.name, ctx.instance(), &crate::hooks::relay_bin()).bus()?;
        }
        prepared.checkout = Some((worktree_path, branch));
        prepared.hooked = true;
        Ok(prepared)
    }, |ctx: &mut Ctx, p, mut prepared| {
        let checkout = prepared.checkout.clone();
        let created = finish_create(ctx, &p, &mut prepared);
        if created.is_err() && prepared.hooked {
            // A create refused after its hook went in leaves the checkout's hook path as it found
            // it: wired to a session still on that checkout (a PAIR partner, or anyone sharing the
            // primary), or restored to the pre-Relay path when there is none.
            if let Some((path, _)) = checkout {
                let repo = Path::new(&prepared.project.path);
                let survivor: Option<String> = ctx.tx().query_row(
                    "SELECT name FROM sessions WHERE worktree=?1 AND name!=?2 AND state!='closed' ORDER BY id DESC LIMIT 1",
                    params![path, prepared.name], |row| row.get(0),
                ).optional().unwrap_or(None);
                match survivor {
                    Some(survivor) => retire_hook_dir(ctx.instance(), repo, Path::new(&path), &prepared.name, Some(&survivor)),
                    None => {
                        // Lock order is store, then hook writes: nothing holding the second takes the first.
                        let _writes = crate::hooks::writes();
                        if let Err(error) = crate::hooks::uninstall_git(repo, Path::new(&path), &prepared.name) {
                            tracing::warn!(session = %prepared.name, %error, "restoring hooks after a refused create");
                        }
                        crate::hooks::remove_hook_dir(repo, &prepared.name);
                    }
                }
            }
        }
        created
    });

    // Launches are staged (D149): the hooks, skill folders and brief files are written with
    // the store unlocked, and only the PTY start and the row update hold it.
    e.register_staged::<Spawn, _>(|ctx, p| {
        let mut prepared = stage_launch(ctx, &p.session, LaunchKind::Fresh, |row| match row.session.state {
            SessionState::Created => Ok(()),
            s if sessions::is_live(s) => Err(BusError::conflict(
                "session.already_spawned",
                format!("session {} is {}", row.session.name, sessions::state_str(s)),
            )),
            s => Err(BusError::conflict(
                "session.state",
                format!(
                    "session {} is {}; {}",
                    row.session.name,
                    sessions::state_str(s),
                    match s {
                        SessionState::Parked => "use session.wake",
                        SessionState::Closed => "a closed session cannot be relaunched",
                        _ => "use session.resume, or session.clear_restorable for a fresh conversation",
                    },
                ),
            )),
        }, |row| {
            if let Some(prompt) = &p.prompt {
                row.launch_prompt = (!prompt.trim().is_empty()).then(|| prompt.clone());
            }
        })?;
        if let Some(prompt) = &p.prompt {
            prepared.launch_prompt = Some((!prompt.trim().is_empty()).then(|| prompt.clone()));
        }
        Ok(prepared)
    }, |ctx: &mut Ctx, _p, prepared| finish_launch(ctx, prepared));

    e.register_staged::<Resume, _>(|ctx, p| {
        // An exited session — the provider quit, crashed or lost its login — relaunches in
        // place exactly like one Relay itself stopped, keeping its conversation and its group.
        stage_launch(ctx, &p.session, LaunchKind::Resume, |row| {
            if !matches!(row.session.state, SessionState::Restorable | SessionState::Exited) {
                return Err(BusError::conflict(
                    "session.state",
                    format!(
                        "session {} is {}; only restorable or exited sessions resume",
                        row.session.name,
                        sessions::state_str(row.session.state)
                    ),
                ));
            }
            Ok(())
        }, |_| {})
    }, |ctx: &mut Ctx, _p, prepared| finish_launch(ctx, prepared));

    e.register_staged::<ClearRestorable, _>(|ctx, p| {
        // A fresh conversation starts from fresh code: the session's own checkout is
        // fast-forwarded to the freshest base first, so the brief it is handed reads the new
        // state. Never the primary checkout, never one a live peer is working in, and never for a
        // clear that is about to be refused.
        let (root, checkout, base, skip) = ctx.read(|conn| {
            let row = sessions::by_name(conn, &p.session)?;
            let project = crate::handlers::workspace::get_project(conn, row.session.project_id)?;
            let shared: bool = conn.prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE worktree=?1 AND id!=?2
                 AND state IN ('spawning','running','idle','blocked'))",
            ).bus()?.query_row(params![row.session.worktree, row.session.id], |r| r.get(0)).bus()?;
            let stopped = matches!(row.session.state, SessionState::Restorable | SessionState::Exited);
            Ok((PathBuf::from(project.path), PathBuf::from(row.session.worktree), project.base_branch, shared || !stopped))
        })?;
        if !skip && root != checkout && checkout.is_dir() {
            match crate::handlers::git::sync_to_base(&root, &checkout, &base) {
                Ok(outcome) => tracing::info!(session = %p.session, ?outcome, "clearing a session: base sync"),
                Err(error) => tracing::warn!(session = %p.session, %error, "clearing a session: base sync failed"),
            }
        }
        let mut prepared = stage_launch(ctx, &p.session, LaunchKind::Fresh, |row| {
            if !matches!(row.session.state, SessionState::Restorable | SessionState::Exited) {
                return Err(BusError::conflict("session.state", format!("session {} is not restorable or exited", row.session.name)));
            }
            Ok(())
        }, |row| row.session.provider_ref = None)?;
        prepared.clear = true;
        Ok(prepared)
    }, |ctx: &mut Ctx, _p, prepared| finish_launch(ctx, prepared));

    // The child gets up to three seconds to exit, so it is stopped before the transaction
    // opens. Its exit callback is silenced first: this request records the outcome, `parked`.
    e.register_staged::<Park, _>(|ctx, p| {
        let row = ctx.read(|conn| sessions::by_name(conn, &p.session))?;
        if !sessions::is_live(row.session.state) {
            return Err(BusError::conflict("session.state", format!("session {} is {}; only live sessions park", row.session.name, sessions::state_str(row.session.state))));
        }
        let pty = ctx.engine().take_pty(row.session.id)
            .ok_or_else(|| BusError::conflict("session.not_spawned", format!("session {} has no PTY", row.session.name)))?;
        pty.silence_exit();
        pty.kill(Duration::from_secs(3));
        let (text, epoch, seq) = pty.scrollback(None);
        Ok((row.session.id, text, epoch, seq))
    }, |ctx: &mut Ctx, p, (id, text, epoch, seq): (Id, String, u64, u64)| {
        let row = sessions::by_id(ctx.tx(), id)?
            .ok_or_else(|| BusError::not_found("session.not_found", format!("no session {}", p.session)))?;
        if !sessions::is_live(row.session.state) {
            return Err(BusError::conflict("session.state", format!("session {} became {} while it was parking", row.session.name, sessions::state_str(row.session.state))));
        }
        sessions::save_scrollback(ctx.tx(), id, &text, epoch, seq, &ctx.now)?;
        ctx.tx().execute(
            "UPDATE sessions SET state='parked',pid=NULL,restore_reason=NULL,updated_at=?1 WHERE id=?2",
            params![ctx.now, id],
        ).bus()?;
        let updated = sessions::by_id(ctx.tx(), id)?.ok_or_else(|| BusError::internal("session vanished"))?;
        emit_session(ctx, &updated.session);
        Ok(updated.session)
    });

    e.register_staged::<Wake, _>(|ctx, p| {
        stage_launch(ctx, &p.session, LaunchKind::Resume, |row| {
            if row.session.state != SessionState::Parked {
                return Err(BusError::conflict(
                    "session.state",
                    format!(
                        "session {} is {}; only parked sessions wake",
                        row.session.name,
                        sessions::state_str(row.session.state)
                    ),
                ));
            }
            Ok(())
        }, |_| {})
    }, |ctx: &mut Ctx, _p, prepared| finish_launch(ctx, prepared));

    e.register::<Get>(|ctx, p| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_same_project(ctx, &row)?;
        let mut session = row.session;
        overlay_live_output(ctx, std::slice::from_mut(&mut session));
        Ok(session)
    });

    e.register::<List>(|ctx, p| {
        // One boundary, stated once: an agent sees the metadata of every session in its own
        // project and nothing outside it (D106). `session.get` draws the same line.
        let scope = actor_project(ctx)?;
        if let (Some(own), Some(asked)) = (scope, p.project_id) {
            if own != asked {
                return Err(BusError::not_own("project"));
            }
        }
        let project_id = scope.or(p.project_id);
        let mut sql = String::from("SELECT * FROM sessions WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(pid) = project_id {
            sql.push_str(" AND project_id = ?");
            args.push(Box::new(pid));
        }
        if !p.include_closed.unwrap_or(false) {
            sql.push_str(" AND state != 'closed'");
        }
        if let Some(states) = &p.state {
            if !states.is_empty() {
                let marks = states.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                sql.push_str(&format!(" AND state IN ({marks})"));
                for s in states {
                    args.push(Box::new(sessions::state_str(*s).to_string()));
                }
            }
        }
        sql.push_str(" ORDER BY id");
        // The shell re-lists sessions on every `session.changed`. The filter combinations are
        // a bounded handful, so the built SQL is cached like any fixed statement.
        let mut st = ctx.tx().prepare_cached(&sql).bus()?;
        let rows = st
            .query_map(
                rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
                sessions::row,
            )
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>();
        let mut sessions: Vec<Session> = rows.bus()?.into_iter().map(|r| r.session).collect();
        overlay_live_output(ctx, &mut sessions);
        Ok(ListOut { sessions })
    });

    e.register::<Peers>(|ctx, p| {
        let (project_id, except): (Id, Option<String>) = match (p.session.as_deref(), p.project_id)
        {
            (Some(name), None) => {
                let row = sessions::by_name(ctx.tx(), name)?;
                assert_own(ctx, &row, true)?;
                (row.session.project_id, Some(name.to_string()))
            }
            (None, Some(project_id)) => {
                crate::handlers::workspace::get_project(ctx.tx(), project_id)?;
                if let Some(sid) = ctx.actor_session_id() {
                    let own = sessions::by_id(ctx.tx(), sid)?
                        .ok_or_else(|| BusError::actor("bound session vanished"))?;
                    if own.session.project_id != project_id {
                        return Err(BusError::not_own("project"));
                    }
                    (project_id, Some(own.session.name))
                } else if ctx.actor.is_agent() {
                    return Err(BusError::actor(
                        "agent actor is not bound to a live session",
                    ));
                } else {
                    (project_id, None)
                }
            }
            // An agent asking "who else is here?" means its own project. Requiring it to say
            // which project, when it only ever has one, was pure ceremony.
            (None, None) => match ctx.actor_session_id() {
                Some(sid) => {
                    let own = sessions::by_id(ctx.tx(), sid)?
                        .ok_or_else(|| BusError::actor("bound session vanished"))?;
                    (own.session.project_id, Some(own.session.name))
                }
                None => {
                    return Err(BusError::invalid(
                        "session.peers_target",
                        "provide exactly one of session or project_id",
                    ))
                }
            },
            _ => {
                return Err(BusError::invalid(
                    "session.peers_target",
                    "provide exactly one of session or project_id",
                ))
            }
        };
        Ok(PeersOut {
            peers: crate::awareness::peers(
                ctx.tx(),
                project_id,
                except.as_deref(),
                Some(ctx.engine()),
            )?,
        })
    });

    e.register_unlocked::<Brief>(|ctx, p| {
        // Metadata is project-visible; a peer's brief and scrollback are not.
        read_own_or_pair(ctx, &p.session)?;
        let git = brief_git(ctx, &p.session)?;
        ctx.read(|conn| crate::awareness::brief(conn, &p.session, Some(ctx.engine()), &git))
    });

    e.register::<Bootstrap>(|ctx, _| {
        let session_id = ctx
            .actor_session_id()
            .ok_or_else(|| BusError::actor("session.bootstrap requires a bound agent session"))?;
        let row = sessions::by_id(ctx.tx(), session_id)?
            .ok_or_else(|| BusError::actor("bound session vanished"))?;
        crate::awareness::bootstrap(ctx.tx(), &row.session.name, ctx.engine())
    });

    // Staged (D149) for the per-task caps and the branch re-check (BUS.md §9.2, RA-109):
    // measuring the task's work is git subprocesses, so it runs before the transaction opens.
    e.register_staged::<Done, _>(|ctx, p| {
        if !ctx.actor.is_agent() || !matches!(p.status.as_deref(), None | Some("completed")) {
            return Ok(None);
        }
        let measure = ctx.read(|conn| {
            let row = sessions::by_name(conn, &p.session)?;
            let s = &row.session;
            if s.role == Role::Reviewer || s.task_id.is_none() || done_pending(conn, s.id)?.is_some() {
                return Ok(None);
            }
            let project = crate::handlers::workspace::get_project(conn, s.project_id)?;
            let cfg = crate::guardrail::config(conn, Some(s.project_id))?;
            Ok(Some((PathBuf::from(&s.worktree), project.base_branch, cfg)))
        })?;
        Ok(measure.and_then(|(worktree, base, cfg)| crate::guardrail::measure_branch(&cfg, &worktree, &base)))
    }, |ctx: &mut Ctx, p, work: Option<crate::guardrail::BranchWork>| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_own(ctx, &row, false)?;
        let s = &row.session;
        // An op that can only report success gets reported success (D118). `blocked` and
        // `partial` leave the task where it is and hand the reason to the person.
        let status = p.status.as_deref().unwrap_or("completed");
        if !matches!(status, "completed" | "blocked" | "partial") {
            return Err(BusError::invalid(
                "session.done_status",
                "status must be completed, blocked, or partial",
            ));
        }
        let blockers: Vec<String> = p.blockers.unwrap_or_default()
            .into_iter().map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).collect();
        if status != "completed" && blockers.is_empty() {
            return Err(BusError::invalid(
                "session.done_blockers",
                format!("status {status:?} needs at least one blocker: say what stopped it"),
            ));
        }
        let current_task_id = s.task_id;
        // A done already landed in this turn and moved the session on to a task it has not
        // been told about yet (the turn's Stop does that): a second call — a retry, or a
        // report on the same work — must not complete that task too.
        let pending = done_pending(ctx.tx(), s.id)?;
        if pending.is_some_and(|done| current_task_id.is_some_and(|current| current != done)) {
            return Ok(row.session);
        }
        // Agents may commit past the commit gate (--no-verify, core.hooksPath, cherry-pick): the
        // branch the task brings to review is judged here as commits, against the same caps,
        // protected paths and shape gates (BUS.md §9.2, RA-109). Never a blocked or partial done.
        if let (Some(work), "completed") = (work.as_ref(), status) {
            crate::guardrail::recheck_branch(ctx.tx(), &ctx.actor, s.project_id, Path::new(&s.worktree), s.id, work)?;
        }
        // Every outcome holds until the turn's Stop: a `blocked` done must survive the tool
        // and Stop reports that trail it in the same turn.
        ctx.tx().execute("UPDATE sessions SET done_pending_stop=?1 WHERE id=?2", params![current_task_id.unwrap_or(0),s.id]).bus()?;
        if matches!(s.role, Role::Builder | Role::Reviewer) && status == "completed" {
            if let Some(task_id) = current_task_id { complete_group_assignment(ctx,s,task_id,p.sha.as_deref())?; }
        }
        if status != "completed" {
            if let Some(task_id) = current_task_id {
                ctx.tx().execute("UPDATE task_sessions SET completed_at=NULL WHERE task_id=?1 AND session_id=?2",params![task_id,s.id]).bus()?;
            }
        }
        // A builder that stopped short leaves its task visible as stopped, not silently active.
        if s.role == Role::Builder && status != "completed" {
            if let Some(task_id) = current_task_id {
                let changed = ctx.tx().execute(
                    "UPDATE tasks SET state='blocked', updated_at=?1
                     WHERE id=?2 AND project_id=?3 AND deleted_at IS NULL AND col='active'",
                    params![ctx.now, task_id, s.project_id],
                ).bus()?;
                if changed > 0 {
                    ctx.emit("task.changed", json!({"task_id": task_id, "state": "blocked"}));
                }
            }
        }
        let session_state = if status == "completed" { "idle" } else { "blocked" };
        // A blocked or partial task stays current and the agent comes back to it, so the files
        // it declared stay its own, as they do through a blocked session.report (RA-400).
        if status == "completed" {
            release_claims(ctx, s)?;
        }
        ctx.tx().execute(
            "UPDATE sessions SET state=?1, last_output_at=?2, updated_at=?2 WHERE id=?3",
            params![session_state, ctx.now, s.id],
        ).bus()?;
        note_pty_state(ctx, s.id, session_state);
        let reported = p.summary.as_deref().map(str::trim).filter(|v| !v.is_empty());
        let summary = match (status, reported) {
            ("completed", Some(text)) => text.to_string(),
            ("completed", None) => "Agent reported completion".to_string(),
            (_, text) => format!(
                "{} — {}: {}",
                text.unwrap_or("Agent stopped short"),
                if status == "blocked" { "blocked by" } else { "still open" },
                blockers.join("; "),
            ),
        };
        let summary = summary.as_str();
        let category = if status == "completed" { "agent_done" } else { "agent_blocked" };
        let link = done_link(s);
        let headline = match status {
            "completed" => format!("{} finished", s.name),
            "blocked" => format!("{} is blocked", s.name),
            _ => format!("{} finished part of its work", s.name),
        };
        let ping = if status == "completed" { Ping::Finished } else { Ping::Always };
        let fresh_notification = record_agent_notification(ctx, s, ping, category, &headline, summary, &link)?;
        if fresh_notification {
          if let Some(message) = crate::handlers::notes::send_system(
            ctx.tx(), s.project_id, "*", &format!("{headline}: {summary}"), s.task_id, &ctx.now,
          )? {
            ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
          }
        }
        let updated = sessions::by_id(ctx.tx(), s.id)?.ok_or_else(|| BusError::internal("session vanished"))?;
        emit_session(ctx, &updated.session);
        if fresh_notification {
            ctx.emit("notify.new", json!({"category":category, "project_id":s.project_id, "session":s.name, "status":status}));
        }
        Ok(updated.session)
    });

    // ---------------------------------------------------------------- intent and claims
    e.register::<Intent>(|ctx: &mut Ctx, p| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_own(ctx, &row, false)?;
        let text = p.text.trim();
        if text.chars().count() > 200 {
            return Err(BusError::invalid(
                "session.intent",
                "intent is one line: 200 characters at most",
            ));
        }
        let intent = (!text.is_empty()).then(|| text.to_string());
        ctx.tx()
            .execute(
                "UPDATE sessions SET intent=?1, updated_at=?2 WHERE id=?3",
                params![intent, ctx.now, row.session.id],
            )
            .bus()?;
        let updated = sessions::by_id(ctx.tx(), row.session.id)?
            .ok_or_else(|| BusError::internal("session vanished"))?;
        emit_session(ctx, &updated.session);
        Ok(updated.session)
    });

    e.register::<Claim>(|ctx: &mut Ctx, p| {
        let session_id = ctx.actor_session_id()
            .ok_or_else(|| BusError::actor("session.claim requires a bound agent session"))?;
        let own = sessions::by_id(ctx.tx(), session_id)?
            .ok_or_else(|| BusError::actor("bound session vanished"))?.session;
        if p.paths.is_empty() {
            return Err(BusError::invalid("session.claim", "claim at least one path"));
        }
        let symbol = p.symbol.unwrap_or_default().trim().to_string();
        let mut paths = Vec::with_capacity(p.paths.len());
        for raw in &p.paths {
            paths.push(claim_path(raw)?);
        }

        // Who already holds these. Reported either way: a collision an agent is told about is
        // one it can negotiate, which is the whole point of a claim (D116).
        let mut collisions = Vec::new();
        for path in &paths {
            let mut stmt = ctx.tx().prepare_cached(
                "SELECT session, symbol, created_at FROM claims
                 WHERE project_id=?1 AND path=?2 AND session_id!=?3
                   AND (symbol='' OR ?4='' OR symbol=?4) ORDER BY session",
            ).bus()?;
            let held = stmt.query_map(params![own.project_id, path, session_id, symbol], |row| {
                Ok(Collision {
                    path: path.clone(),
                    symbol: row.get("symbol")?,
                    session: row.get("session")?,
                    since: row.get("created_at")?,
                })
            }).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            collisions.extend(held);
        }
        if p.exclusive.unwrap_or(false) && !collisions.is_empty() {
            return Err(BusError::conflict(
                "session.claim_held",
                format!(
                    "{} already claimed by {}",
                    collisions[0].path,
                    collisions.iter().map(|c| c.session.as_str()).collect::<Vec<_>>().join(", "),
                ),
            ).with_details(serde_json::to_value(&collisions).bus()?)
             .with_hint("drop exclusive to work alongside them, or coordinate with mailbox.send"));
        }
        for path in &paths {
            ctx.tx().execute(
                "INSERT INTO claims(project_id, session_id, session, path, symbol, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(session_id, path, symbol) DO UPDATE SET note=excluded.note, updated_at=excluded.updated_at",
                params![own.project_id, session_id, own.name, path, symbol, p.note, ctx.now],
            ).bus()?;
        }
        ctx.set_project(own.project_id);
        ctx.emit("overlap.changed", json!({"project_id": own.project_id, "session": own.name, "claimed": paths}));
        Ok(ClaimOut {
            claimed: paths,
            collisions,
        })
    });

    e.register::<Release>(|ctx: &mut Ctx, p| {
        let session_id = ctx
            .actor_session_id()
            .ok_or_else(|| BusError::actor("session.release requires a bound agent session"))?;
        let own = sessions::by_id(ctx.tx(), session_id)?
            .ok_or_else(|| BusError::actor("bound session vanished"))?
            .session;
        let released = match &p.paths {
            None => ctx
                .tx()
                .execute("DELETE FROM claims WHERE session_id=?1", [session_id])
                .bus()?,
            Some(paths) => {
                let mut removed = 0;
                for raw in paths {
                    let path = claim_path(raw)?;
                    removed += ctx
                        .tx()
                        .execute(
                            "DELETE FROM claims WHERE session_id=?1 AND path=?2",
                            params![session_id, path],
                        )
                        .bus()?;
                }
                removed
            }
        };
        ctx.set_project(own.project_id);
        ctx.emit(
            "overlap.changed",
            json!({"project_id": own.project_id, "session": own.name, "released": released}),
        );
        Ok(ReleaseOut {
            released: released as u32,
        })
    });

    e.register::<Report>(|ctx: &mut Ctx, p| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_own(ctx, &row, false)?;
        let s = &row.session;
        let data = p.data.unwrap_or_else(|| json!({}));
        // Every agent tool call lands here, so its statements are all cached (RA-401).
        let pending_stop: Option<Id> = ctx.tx().prepare_cached("SELECT done_pending_stop FROM sessions WHERE id=?1").bus()?
            .query_row([s.id], |row| row.get(0)).bus()?;
        let done = pending_stop.filter(|id| *id != ASSIGNMENT_AT_STOP);
        let completed_without_done = p.kind == "stop" && done.is_none() && s.state == SessionState::Running;
        let provider_ref = data.get("session_id").or_else(|| data.get("provider_ref")).and_then(Value::as_str);
        // It is replayed into the provider's argv on resume (`--resume <id>`), so a value a CLI
        // could parse as an option must never be stored.
        if let Some(value) = provider_ref.filter(|value| !crate::providers::is_provider_ref(value)) {
            return Err(BusError::invalid("session.provider_ref", format!("{value:?} is not a provider session id"))
                .with_hint("provider_ref is the provider's own conversation id: letters, digits, '-' and '_', starting with a letter or digit"));
        }
        let state = match p.kind.as_str() {
            "session_start" | "tool_use" => "running",
            "stop" | "idle" => "idle",
            "blocked" => "blocked",
            "notification" => {
                let type_ = data.get("notification_type").or_else(|| data.get("type")).and_then(Value::as_str).unwrap_or("");
                // Claude's `idle_prompt` ("waiting for your input") follows every finished turn a
                // minute later: the turn's Stop already said so, and it needs nothing new.
                if matches!(type_, "permission_prompt" | "elicitation_dialog" | "agent_needs_input" | "blocked") { "blocked" } else { sessions::state_str(s.state) }
            }
            _ => return Err(BusError::invalid("session.report_kind", "kind must be session_start, tool_use, stop, notification, idle, or blocked")),
        };
        let provider_ref_changed = provider_ref.is_some_and(|value| s.provider_ref.as_deref() != Some(value));
        // A hook that lands after its session was parked, exited or closed speaks for a process
        // that is gone. It may still name the conversation; it changes nothing else.
        if !sessions::is_live(s.state) && s.state != SessionState::Created {
            if provider_ref_changed {
                ctx.tx().execute("UPDATE sessions SET provider_ref=?1, updated_at=?2 WHERE id=?3", params![provider_ref, ctx.now, s.id]).bus()?;
                let updated = sessions::by_id(ctx.tx(), s.id)?.ok_or_else(|| BusError::internal("session vanished"))?;
                emit_session(ctx, &updated.session);
            }
            return Ok(Empty {});
        }
        // `session.done` decided how this turn ends. The tool events that trail it in the same
        // turn — the done call's own PostToolUse among them — and the turn's Stop keep that
        // outcome: a blocked done stays blocked.
        let state = match (done, p.kind.as_str()) {
            (Some(_), "tool_use" | "idle") => sessions::state_str(s.state),
            (Some(_), "stop") if s.state == SessionState::Blocked => "blocked",
            _ => state,
        };
        let state_changed = state != sessions::state_str(s.state);
        // A Stop ends the turn a pending done belonged to. A fresh provider session starts no
        // turn a done could belong to; an assignment still waiting for a Stop keeps waiting.
        ctx.tx().prepare_cached(
            "UPDATE sessions SET state=?1, provider_ref=COALESCE(?2, provider_ref), last_output_at=?3, updated_at=?3,
             done_pending_stop=CASE WHEN ?5='stop' THEN NULL WHEN ?5='session_start' AND done_pending_stop!=?6 THEN NULL
                                    ELSE done_pending_stop END
             WHERE id=?4",
        ).bus()?.execute(params![state, provider_ref, ctx.now, s.id, p.kind, ASSIGNMENT_AT_STOP]).bus()?;
        note_pty_state(ctx, s.id, state);
        if matches!(p.kind.as_str(), "tool_use" | "stop") {
            // The command behind a device lease has finished; keep the lease only briefly.
            crate::handlers::device_lease::command_finished(ctx.engine(), s.id, data.get("tool_input"), p.kind == "stop");
        }
        if p.kind == "stop" {
            if let (Some(completed), Some(current)) = (pending_stop, s.task_id) {
                if completed != current { deliver_assignment(ctx, s, current, false)?; }
            }
        }
        ctx.set_project(s.project_id);
        if s.role == Role::Builder {
            let task_state = match state { "running" => Some("running"), "blocked" => Some("blocked"), _ => None };
            if let (Some(task_state), Some(task_id)) = (task_state, s.task_id) {
                let changed = ctx.tx().prepare_cached(
                    "UPDATE tasks SET state=?1,updated_at=?2 WHERE id=?3 AND col='active' AND deleted_at IS NULL AND state!=?1",
                ).bus()?.execute(params![task_state, ctx.now, task_id]).bus()?;
                if changed > 0 {
                    ctx.emit("task.changed", json!({"task_id":task_id,"state":task_state}));
                }
            }
        }
        let blocked = state == "blocked";
        if blocked && state_changed {
            let body = data.get("message").and_then(Value::as_str).unwrap_or("Agent needs attention");
            let link = json!({"op":"session.get", "payload":{"session":s.name}}).to_string();
            if record_agent_notification(ctx, s, Ping::Always, "agent_blocked", &format!("{} needs attention", s.name), body, &link)? {
            ctx.emit("notify.new", json!({"category":"agent_blocked", "project_id":s.project_id, "session":s.name}));
            if let Some(message) = crate::handlers::notes::send_system(
                ctx.tx(), s.project_id, "*", &format!("{} is blocked: {}", s.name, body), s.task_id, &ctx.now,
            )? {
                ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
            }
            }
        }
        if completed_without_done {
            let summary = data.get("last-assistant-message")
                .or_else(|| data.get("last_assistant_message"))
                .or_else(|| data.get("message"))
                .and_then(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("Agent turn completed");
            notify_completion(ctx, s, summary)?;
        }
        if state_changed || provider_ref_changed {
            let updated = sessions::by_id(ctx.tx(), s.id)?.ok_or_else(|| BusError::internal("session vanished"))?;
            emit_session(ctx, &updated.session);
        }
        Ok(Empty {})
    });

    // A branch rename is git subprocesses, so it runs before the transaction opens (RA-402);
    // the transaction then requires the row to still carry the branch that was renamed.
    e.register_staged::<Update, _>(|ctx, p| {
        let Some(branch) = p.branch.as_deref() else { return Ok(None) };
        let (before, project_path, shared) = ctx.read(|conn| {
            let before = sessions::by_name(conn, &p.session)?;
            check_update(conn, &before, p)?;
            let project = crate::handlers::workspace::get_project(conn, before.session.project_id)?;
            let shared = survivor_on(conn, before.session.id, &before.session.worktree)?;
            Ok((before, project.path, shared))
        })?;
        let s = &before.session;
        if branch == s.branch { return Ok(None); }
        if s.pair_with.is_some() { return Err(BusError::conflict("session.pair_branch", "a PAIR branch cannot be renamed independently")); }
        // `git branch -m` renames whatever the checkout has out: on the primary that is the
        // project's own base branch, and on a shared checkout every other session's branch too.
        let canonical = |path: &str| fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
        if canonical(&s.worktree) == canonical(&project_path) {
            return Err(BusError::conflict("session.branch_primary", "a session on the primary checkout cannot rename its branch: it is the project's own"));
        }
        if let Some(other) = shared {
            return Err(BusError::conflict("session.branch_shared", format!("session {other} shares this checkout, so its branch cannot be renamed for one session")));
        }
        worktree::rename_branch(Path::new(&s.worktree), branch)
            .map_err(|error| BusError::conflict("session.branch", error.to_string()))?;
        Ok(Some(s.branch.clone()))
    }, |ctx: &mut Ctx, p, renamed_from: Option<String>| {
        let before = sessions::by_name(ctx.tx(), &p.session)?;
        let s = &before.session;
        let (model, effort, task_id, module_id) = check_update(ctx.tx(), &before, &p)?;
        let branch = p.branch.clone().unwrap_or_else(|| s.branch.clone());
        if branch != s.branch && renamed_from.as_deref() != Some(s.branch.as_str()) {
            return Err(BusError::conflict("session.branch", format!("session {} changed while its branch was being renamed", s.name)));
        }
        ctx.tx().execute(
            "UPDATE sessions SET branch=?1,model=?2,effort=?3,task_id=?4,module_id=?5,bus_writes=?6,allow_ui=?7,updated_at=?8 WHERE id=?9",
            params![branch, model, effort, task_id, module_id, p.bus_writes.unwrap_or(s.bus_writes) as i64,
                p.allow_ui.unwrap_or(s.allow_ui) as i64, ctx.now, s.id],
        ).bus()?;
        // A task attached here is queued for the whole group, as dispatch does; an update that
        // leaves the task alone only re-asserts this session's own row.
        let mut queued = Vec::new();
        if let Some(task_id) = task_id {
            let ids = if p.task_id.is_some() { group_ids(ctx.tx(), s)? } else { vec![s.id] };
            queued = enqueue(ctx.tx(), task_id, &ids)?.into_iter().map(|sid| json!([task_id, sid])).collect();
        }
        let updated = sessions::by_id(ctx.tx(), s.id)?.ok_or_else(|| BusError::internal("session vanished"))?;
        // The inverse names only what this update changed. Branch, model and effort change only
        // before the first spawn, so an inverse that always carried them was refused as soon as
        // the session had started, whatever the update was about (RA-403).
        // Its precondition is those fields still holding what this update wrote, not the row's
        // `updated_at`, which every `session.report` of a running session moves on. And the
        // queue rows it added go with it: `expect.queued` names the (task, session) pairs that
        // were not queued before, as UpdateIn has no field for them.
        let mut inverse = serde_json::Map::new();
        let mut fields = serde_json::Map::new();
        inverse.insert("session".into(), json!(s.name));
        let u = &updated.session;
        let mut changed = |name: &str, before: Value, after: Value| if before != after {
            inverse.insert(name.into(), before);
            fields.insert(name.into(), after);
        };
        changed("branch", json!(s.branch), json!(u.branch));
        changed("model", json!(s.model), json!(u.model));
        changed("effort", json!(s.effort), json!(u.effort));
        changed("task_id", json!(s.task_id), json!(u.task_id));
        changed("module_id", json!(s.module_id), json!(u.module_id));
        changed("bus_writes", json!(s.bus_writes), json!(u.bus_writes));
        changed("allow_ui", json!(s.allow_ui), json!(u.allow_ui));
        ctx.set_undo("session.update", Value::Object(inverse), Some(json!({"fields": fields, "queued": queued})));
        emit_session(ctx, &updated.session);
        Ok(updated.session)
    });

    e.register_unlocked::<RestorableList>(|ctx, p| {
        let rows = ctx.read(|conn| {
            // An agent sees its own project's sessions and nothing outside it, as in
            // session.list (D106, RA-404).
            let scope = match ctx.actor_session_id() {
                Some(sid) => Some(sessions::by_id(conn, sid)?.ok_or_else(|| BusError::actor("bound session vanished"))?.session.project_id),
                None if ctx.actor.is_agent() => return Err(BusError::actor("agent actor is not bound to a live session")),
                None => None,
            };
            if let (Some(own), Some(asked)) = (scope, p.project_id) {
                if own != asked { return Err(BusError::not_own("project")); }
            }
            let mut stmt = conn.prepare_cached(
                "SELECT * FROM sessions WHERE state='restorable' AND (?1 IS NULL OR project_id=?1) AND (?2 IS NULL OR name=?2) ORDER BY id",
            ).bus()?;
            let rows = stmt.query_map(params![scope.or(p.project_id), p.session], |row| Ok((sessions::row(row)?, row.get::<_, Option<String>>("restore_reason")?)))
                .bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(rows)
        })?;
        // One `git status` per worktree, each up to its 10 s timeout. Run side by side, a few at
        // a time, so the answer costs the slowest checkout rather than the sum of them (RA-235).
        let dirty = |checkout: &str| gix::open(checkout).ok().is_some_and(|repo| worktree::is_dirty(&repo));
        let per_worker = rows.len().div_ceil(4).max(1);
        let flags: Vec<bool> = std::thread::scope(|scope| {
            let workers: Vec<_> = rows.chunks(per_worker)
                .map(|chunk| scope.spawn(move || chunk.iter().map(|(row, _)| dirty(&row.session.worktree)).collect::<Vec<_>>()))
                .collect();
            // A worker that panicked reports its checkouts dirty, as a failed scan does.
            workers.into_iter().zip(rows.chunks(per_worker))
                .flat_map(|(worker, chunk)| worker.join().unwrap_or_else(|_| vec![true; chunk.len()]))
                .collect()
        });
        let out = rows.into_iter().zip(flags).map(|((row, reason), dirty)| Restorable {
            session: row.session,
            reason: reason.unwrap_or_else(|| "app_restart".into()),
            worktree_dirty: dirty,
        }).collect();
        Ok(RestorableOut { sessions: out })
    });

    // Both of these undo hooks and may delete a checkout: git subprocesses and a tree walk.
    // That half runs before the transaction opens; the transaction only records the result.
    e.register_staged::<DiscardRestorable, _>(|ctx, p| {
        let (row, repo, teardown) = ctx.read(|conn| {
            let row = sessions::by_name(conn, &p.session)?;
            if !matches!(row.session.state, SessionState::Restorable | SessionState::Exited) {
                return Err(BusError::conflict("session.state", format!("session {} is not restorable or exited", row.session.name)));
            }
            let project = crate::handlers::workspace::get_project(conn, row.session.project_id)?;
            let survivor = survivor_on(conn, row.session.id, &row.session.worktree)?;
            let teardown = if survivor.is_none() { Some(read_teardown(conn, &row.session.worktree)?) } else { None };
            Ok((row, PathBuf::from(project.path), Hooks { teardown, survivor }))
        })?;
        let Hooks { teardown, survivor } = teardown;
        let wt = Path::new(&row.session.worktree);
        // Only the last session on a pooled checkout deletes it, so only then can it lose work.
        if teardown.is_some() && wt.starts_with(worktree::pool_dir(&repo)) && !p.discard_changes.unwrap_or(false) {
            refuse_dirty(&[(wt.to_path_buf(), vec![row.session.name.clone()])])?;
        }
        if let Some(teardown) = &teardown {
            run_teardown(&repo, wt, teardown)?;
            if wt.starts_with(worktree::pool_dir(&repo)) {
                worktree::remove(&repo, wt, true).map_err(|error| BusError::conflict("worktree.remove_failed", error.to_string()))?;
            }
        }
        retire_hook_dir(ctx.instance(), &repo, wt, &row.session.name, survivor.as_deref());
        Ok(row.session.id)
    }, |ctx: &mut Ctx, p, id: Id| {
        let row = sessions::by_id(ctx.tx(), id)?
            .ok_or_else(|| BusError::not_found("session.not_found", format!("no session {}", p.session)))?;
        let s = &row.session;
        if !matches!(s.state, SessionState::Restorable | SessionState::Exited) {
            return Err(BusError::conflict("session.state", format!("session {} is not restorable or exited", s.name)));
        }
        retire_session(ctx, s)?;
        Ok(Empty {})
    });

    // Closing what the shell closes (remove_worktree:false) holds the store for a few row
    // updates and answers at once: the child is killed on its own thread after the commit.
    // Removing the checkout stops the child first, with the store unlocked, and the exit
    // callback silenced so the row goes straight to `closed`.
    e.register_staged::<Close, _>(|ctx, p| {
        let remove = p.remove_worktree.unwrap_or(true);
        let (row, repo, teardown) = ctx.read(|conn| {
            let row = sessions::by_name(conn, &p.session)?;
            let s = &row.session;
            let project = crate::handlers::workspace::get_project(conn, s.project_id)?;
            let survivor = survivor_on(conn, s.id, &s.worktree)?;
            // Only a pooled checkout is ever removed, so only there does a partner still on it
            // stand in the way; an independent agent sharing the primary does not.
            let pooled = Path::new(&s.worktree).starts_with(worktree::pool_dir(Path::new(&project.path)));
            if survivor.is_some() && remove && pooled {
                return Err(BusError::conflict("session.pair_live", "close the PAIR partner first or pass remove_worktree:false"));
            }
            let teardown = if survivor.is_none() { Some(read_teardown(conn, &s.worktree)?) } else { None };
            Ok((row, PathBuf::from(project.path), Hooks { teardown, survivor }))
        })?;
        let Hooks { teardown, survivor } = teardown;
        let s = &row.session;
        let wt = Path::new(&s.worktree);
        // Before anything is stopped or undone: a refused close leaves the agent running.
        let checked = remove && wt.starts_with(worktree::pool_dir(&repo)) && !p.discard_changes.unwrap_or(false);
        if checked {
            refuse_dirty(&[(wt.to_path_buf(), vec![s.name.clone()])])?;
        }
        // Stop writers before undoing their hooks and deleting their checkout, with a bounded
        // grace period.
        let stopped = remove
            .then(|| ctx.engine().take_pty(s.id))
            .flatten()
            .map(|pty| {
                pty.silence_exit();
                pty.kill(Duration::from_millis(150));
            });
        // The check above ran while the agent could still write. What it wrote since is found
        // here, now that it has stopped: the checkout and its hooks are kept, and the session,
        // its agent gone, reads `exited` rather than `closed` — a later close or
        // discard_restorable checks it again.
        if checked && stopped.is_some() {
            if let Err(mut error) = refuse_dirty(&[(wt.to_path_buf(), vec![s.name.clone()])]) {
                mark_exited_detached(ctx.engine(), s.id, s.project_id);
                error.message = format!("{}. The agent wrote it after the close began: it was stopped, the session is now exited, and the worktree was kept", error.message);
                // There is no agent left to keep: a dialog offering "Keep the agent" reads
                // this and offers keeping the worktree instead.
                if let Some(details) = error.details.as_mut().and_then(Value::as_object_mut) {
                    details.insert("stopped".into(), Value::Bool(true));
                }
                return Err(error);
            }
        }
        let cleaned = (|| {
            if let Some(teardown) = &teardown {
                run_teardown(&repo, wt, teardown)?;
            }
            if remove && wt.starts_with(worktree::pool_dir(&repo)) {
                return worktree::remove(&repo, wt, p.purge_build.unwrap_or(true))
                    .map_err(|e| BusError::conflict("worktree.remove_failed", e.to_string()));
            }
            Ok(0u64)
        })();
        let freed = match cleaned {
            Ok(freed) => freed,
            Err(error) => {
                // The child is gone and its exit callback was silenced: say so in the row
                // rather than leave a live session with no process behind it.
                if stopped.is_some() {
                    mark_exited_detached(ctx.engine(), s.id, s.project_id);
                }
                return Err(error);
            }
        };
        // The generated hook directory outlives nothing: leaving one per dead session behind
        // makes `.relay/hooks` read like a fleet that never shut down. On a shared checkout it
        // may be the one the checkout's hooksPath points at, so it is handed over first.
        retire_hook_dir(ctx.instance(), &repo, wt, &s.name, survivor.as_deref());
        Ok((s.id, freed))
    }, |ctx: &mut Ctx, p, (id, freed): (Id, u64)| {
        let row = sessions::by_id(ctx.tx(), id)?
            .ok_or_else(|| BusError::not_found("session.not_found", format!("no session {}", p.session)))?;
        retire_session(ctx, &row.session)?;
        Ok(CloseOut { freed_mb: freed as f64 / (1024.0 * 1024.0) })
    });

    e.register::<Attach>(|ctx, p| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_own(ctx, &row, true)?;
        if ctx.engine().pty(row.session.id).is_none() {
            return Err(BusError::conflict(
                "session.not_spawned",
                format!(
                    "session {} has no PTY (state {})",
                    row.session.name,
                    sessions::state_str(row.session.state)
                ),
            ));
        }
        Ok(Empty {})
    });
    e.register::<Detach>(|ctx, p| {
        let row = sessions::by_name(ctx.tx(), &p.session)?;
        assert_own(ctx, &row, true)?;
        Ok(Empty {})
    });
    // The engine answers every user keystroke and resize against a registered PTY from memory
    // (`Engine::pty_fast_path`, D148). These are the fallbacks for the rest: they own the typed
    // not_found / not_spawned / exited refusals, and in the narrow race where the PTY registers
    // between the fast-path miss and here, they write the way the fast path does — with the
    // store unlocked, so a paste into a full input queue blocks only itself, and the idle edge
    // claimed from the PTY rather than read from the row (RA-646).
    e.register_staged::<Input, _>(|ctx, p| {
        let (id, pty) = live_pty(ctx, &p.session)?;
        Ok((id, crate::engine::write_input(&pty, &p.data)?))
    }, |ctx: &mut Ctx, _p, (id, edge): (Id, bool)| {
        if edge && ctx.tx().execute("UPDATE sessions SET state='running',updated_at=?1 WHERE id=?2 AND state='idle'", params![ctx.now, id]).bus()? > 0 {
            let updated = sessions::by_id(ctx.tx(), id)?.ok_or_else(|| BusError::internal("session vanished"))?;
            emit_session(ctx, &updated.session);
        }
        Ok(Empty {})
    });
    e.register_staged::<Resize, _>(|ctx, p| {
        let (_, pty) = live_pty(ctx, &p.session)?;
        pty.resize(p.cols, p.rows).map_err(|e| BusError::conflict("session.io", e.to_string()))
    }, |_ctx: &mut Ctx, _p, ()| Ok(Empty {}));
    // Up to the whole 8 MiB ring, copied, decoded and serialized: the store is taken only to
    // resolve the row and, for a session with no PTY, to load its saved copy (RA-406).
    e.register_unlocked::<Scrollback>(|ctx, p| {
        let row = read_own_or_pair(ctx, &p.session)?;
        let pty = ctx.engine().pty(row.session.id);
        let size = pty.as_ref().map(|pty| pty.size());
        let (text, epoch, seq) = match pty {
            Some(pty) => pty.scrollback(p.lines.map(|n| n as usize)),
            None => {
                let (text, epoch, seq) = ctx.read(|conn| sessions::load_scrollback(conn, row.session.id))?
                    .ok_or_else(|| {
                        BusError::conflict(
                            "session.not_spawned",
                            format!(
                                "session {} has no PTY or saved scrollback",
                                row.session.name
                            ),
                        )
                    })?;
                let text = match p.lines {
                    Some(lines) => {
                        let all = text.lines().collect::<Vec<_>>();
                        all[all.len().saturating_sub(lines as usize)..].join("\n")
                    }
                    None => text,
                };
                (text, epoch, seq)
            }
        };
        Ok(ScrollbackOut { text, epoch, seq, cols: size.map(|s| s.0), rows: size.map(|s| s.1) })
    });
    // A file read of up to 8 MiB, and for Codex a walk to find the file: never under the lock.
    e.register_unlocked::<Transcript>(|ctx, p| {
        let row = read_own_or_pair(ctx, &p.session)?;
        let s = &row.session;
        let worktree = Path::new(&s.worktree);
        let path = s.provider_ref.as_deref().filter(|reference| crate::providers::is_provider_ref(reference))
            .and_then(|reference| crate::transcript::locate(s.provider, reference, worktree));
        let Some(path) = path else {
            return Ok(TranscriptOut { messages: Vec::new(), cursor: 0, reset: false, found: false });
        };
        let limit = p.limit.unwrap_or(200).clamp(1, 1000) as usize;
        crate::transcript::read(s.provider, &path, worktree, p.after, limit)
            .map_err(|e| BusError::unavailable("session.transcript_unreadable", format!("{}: {e}", path.display())))
    });

    // A picture from the phone for the agent: decoded and written with the store lock released;
    // the transaction only names the project.
    e.register_staged::<Upload, _>(|ctx, p| {
        let row = read_own_or_pair(ctx, &p.session)?;
        let bytes = decode_upload(&p.bytes_b64)?;
        let picture = crate::handlers::design::picture::sniff(&bytes)
            .filter(|picture| picture.ext != "svg")
            .ok_or_else(|| BusError::invalid("session.upload", "not a PNG, JPEG, GIF or WebP picture whose size can be read"))?;
        let s = &row.session;
        let worktree = Path::new(&s.worktree);
        if s.worktree.is_empty() || !worktree.is_dir() {
            return Err(BusError::conflict("session.upload_no_checkout", format!("session {} has no checkout to put a picture in", s.name)));
        }
        let digest = {
            use sha2::Digest;
            crate::hex(&sha2::Sha256::digest(&bytes))
        };
        let name = format!("phone-{}.{}", &digest[..16], picture.ext);
        let dir = attachments_dir(worktree)?;
        store_upload(&dir, &name, &bytes)?;
        Ok((row, UploadOut {
            path: format!("{ATTACHMENTS}/{name}"),
            absolute: dir.join(&name).display().to_string(),
            w: picture.w,
            h: picture.h,
        }))
    }, |ctx: &mut Ctx, _p, (row, out): (Row_, UploadOut)| {
        ctx.set_project(row.session.project_id);
        Ok(out)
    });

    // Linked repositories (crate::linked). The clone is network work and the status scans are
    // git subprocesses, so both run before the transaction opens; the transaction only tells
    // the agent and emits the event.
    e.register_staged::<RepoAdd, _>(|ctx, p| {
        let row = read_linkable(ctx, &p.session)?;
        let url = crate::linked::clone_url(&p.url).map_err(|e| BusError::invalid("session.repo_url", e))?;
        let name = crate::linked::name_for(&url, p.name.as_deref()).map_err(|e| BusError::invalid("session.repo_name", e))?;
        let wt = Path::new(&row.session.worktree);
        if crate::linked::dir(wt).join(&name).exists() {
            return Err(BusError::conflict("session.repo_exists", format!("session {} already links a repository named {name}", row.session.name))
                .with_hint("pass another name, or remove the one already linked"));
        }
        let path = crate::linked::clone(wt, &url, &name, p.branch.as_deref())
            .map_err(|e| BusError::unavailable("session.repo_clone_failed", e))?;
        Ok((row, crate::linked::describe(&name, &path)))
    }, |ctx: &mut Ctx, _p, (row, repo): (Row_, LinkedRepo)| {
        let s = &row.session;
        // The agent learns of a repository someone else linked for it; one it linked itself
        // has this answer.
        if ctx.actor_session_id() != Some(s.id) {
            let from = repo.url.as_deref().map(|url| format!(" from {url}")).unwrap_or_default();
            let text = format!("The user linked repository {} to this session: cloned{from} at {}. Read and edit it there; push its own branches from inside it. It is deleted with this session's worktree.", repo.name, repo.path);
            super::notes::send_system(ctx.tx(), s.project_id, &s.name, &text, None, &ctx.now)?;
        }
        ctx.emit("session.repo.changed", json!({"session": s.name, "project_id": s.project_id}));
        Ok(repo)
    });
    e.register_unlocked::<RepoList>(|ctx, p| {
        let row = read_own_or_pair(ctx, &p.session)?;
        let repos = crate::linked::list(Path::new(&row.session.worktree)).into_iter().map(|(name, path)| {
            let found = crate::linked::unpushed(&path);
            let mut repo = crate::linked::describe(&name, &path);
            repo.unpushed = crate::linked::holds_work(&found).then_some(found);
            repo
        }).collect();
        Ok(RepoListOut { repos })
    });
    e.register_staged::<RepoRemove, _>(|ctx, p| {
        let row = ctx.read(|conn| sessions::by_name(conn, &p.session))?;
        let wt = PathBuf::from(&row.session.worktree);
        let path = crate::linked::dir(&wt).join(&p.name);
        if !crate::linked::list(&wt).iter().any(|(name, _)| *name == p.name) {
            return Err(BusError::not_found("session.repo_not_found", format!("session {} links no repository named {}", row.session.name, p.name)));
        }
        if !p.discard_changes.unwrap_or(false) {
            let found = crate::linked::unpushed(&path);
            if crate::linked::holds_work(&found) {
                return Err(BusError::conflict("session.repo_unpushed",
                    format!("linked repo {} has {}; removing it would delete them", p.name, crate::linked::summary(&found)))
                    .with_details(json!({"name": p.name, "path": path.display().to_string(), "unpushed": found}))
                    .with_hint("commit and push them from inside the repository first, or pass discard_changes: true to delete them with it"));
            }
        }
        let freed = crate::linked::remove(&wt, &p.name).map_err(|e| BusError::conflict("session.repo_remove_failed", e))?;
        Ok((row, freed))
    }, |ctx: &mut Ctx, p, (row, freed): (Row_, u64)| {
        let s = &row.session;
        if ctx.actor_session_id() != Some(s.id) && !matches!(s.state, SessionState::Closed) {
            let text = format!("The user removed linked repository {} from this session; {} is gone.", p.name, crate::linked::dir(Path::new(&s.worktree)).join(&p.name).display());
            super::notes::send_system(ctx.tx(), s.project_id, &s.name, &text, None, &ctx.now)?;
        }
        ctx.emit("session.repo.changed", json!({"session": s.name, "project_id": s.project_id}));
        Ok(RepoRemoveOut { freed_mb: freed as f64 / 1_048_576.0 })
    });
}

/// The session a repository is being linked to: the actor's own (or any, for the user), still
/// open, its checkout on disk.
fn read_linkable(ctx: &crate::engine::Unlocked, session: &str) -> Result<Row_, BusError> {
    let row = ctx.read(|conn| sessions::by_name(conn, session))?;
    owns(&ctx.actor, ctx.actor_session_id(), &row, false, |_| Ok(None))?;
    if matches!(row.session.state, SessionState::Closed) {
        return Err(BusError::conflict("session.state", format!("session {} is closed", row.session.name)));
    }
    if !Path::new(&row.session.worktree).is_dir() {
        return Err(BusError::conflict("session.worktree_missing", format!("session {}'s worktree {} is not on disk", row.session.name, row.session.worktree)));
    }
    Ok(row)
}

/// For the socket door: resolve a session name to its PTY (after `session.attach` succeeded).
pub fn pty_by_name(engine: &Engine, name: &str) -> Result<(Id, Arc<Pty>), BusError> {
    let conn = engine.store.lock();
    let row = sessions::by_name(&conn, name)?;
    drop(conn);
    let pty = engine.pty(row.session.id).ok_or_else(|| {
        BusError::conflict("session.not_spawned", format!("session {name} has no PTY"))
    })?;
    Ok((row.session.id, pty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn launch_files_replace_planted_symlinks_instead_of_following_them() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path().join("wt");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(worktree.join(".relay/sessions/s")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("victim"), "keep").unwrap();
        // The file itself, then a directory on its path, pointed outside the worktree.
        symlink(outside.join("victim"), worktree.join(".relay/sessions/s/brief.md")).unwrap();
        write_session_file(&worktree, ".relay/sessions/s/brief.md", "brief", "test").unwrap();
        assert_eq!(fs::read_to_string(outside.join("victim")).unwrap(), "keep");
        assert!(!fs::symlink_metadata(worktree.join(".relay/sessions/s/brief.md")).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(worktree.join(".relay/sessions/s/brief.md")).unwrap(), "brief");

        fs::remove_dir_all(worktree.join(".relay/sessions")).unwrap();
        symlink(&outside, worktree.join(".relay/sessions")).unwrap();
        write_session_file(&worktree, ".relay/sessions/victim", "brief", "test").unwrap();
        assert_eq!(fs::read_to_string(outside.join("victim")).unwrap(), "keep");
        assert_eq!(fs::read_to_string(worktree.join(".relay/sessions/victim")).unwrap(), "brief");
        assert!(fs::read_dir(worktree.join(".relay/sessions")).unwrap().count() == 1, "no temporary file left behind");
    }

    #[test]
    fn a_new_session_does_not_reattach_a_namesakes_leftover_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(repo).args(args).status().unwrap().success());
        git(&["init", "-q", "-b", "main"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
        assert_eq!(own_branch(repo, "amber-heron").unwrap(), "relay/amber-heron");
        git(&["branch", "relay/amber-heron"]);
        git(&["branch", "relay/amber-heron-2"]);
        assert_eq!(own_branch(repo, "amber-heron").unwrap(), "relay/amber-heron-3");
    }
}
