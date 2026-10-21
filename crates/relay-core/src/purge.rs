//! The reconcile pass's retention work (BUS.md §5.4, §10.2): soft-deleted rows are hard-deleted
//! once `undo.grace_days` has passed, and the notification and mailbox tables, which every agent
//! and guardrail writes into, are kept to a window instead of growing for the life of the store.
//!
//! [`run`] does only SQL, so it can run inside the request transaction. The files those rows
//! named (attachments, `.relay/trash/<id>/`) are returned for the caller to remove once the
//! transaction is gone — never under the store mutex, and so are the designs it purged, for the
//! caller to announce once they are gone. Pictures no design names any more are not rows: the
//! design-assets sweep (`disk_sweep::sweep_design_assets`) takes them, after the pass.

use rusqlite::{params, Connection, Transaction};
use serde_json::json;
use std::path::{Path, PathBuf};

const DEFAULT_GRACE_DAYS: i64 = 7;
/// A notification the user has read is history after this long.
const READ_NOTIFICATION_DAYS: i64 = 30;
/// One nobody read is stale after this long; the hold or run it pointed at has long moved on.
const NOTIFICATION_DAYS: i64 = 180;
/// An answered guardrail hold — whose envelope can carry a whole file — is history after this long.
const ANSWERED_HOLD_DAYS: i64 = 30;
/// The table never holds more than this many, newest kept.
const MAX_NOTIFICATIONS: i64 = 5000;
/// Allowed hook calls (`guardrail.gate`, `session.report`, `usage.report`) are an audit row per
/// agent tool call, thousands a day; they are history after this long. Refusals, holds and every
/// other op keep `audit.retention_days`.
const HOOK_AUDIT_DAYS: i64 = 30;
/// A message every recipient has acked is history after this long.
const ACKED_MESSAGE_DAYS: i64 = 30;
/// Any message, acked or not, after this long — the recipient sessions are long closed.
const MESSAGE_DAYS: i64 = 180;

#[derive(Default, Debug)]
pub struct Purged {
    /// What the pass did, in the `app.reconcile` / `app.recovery.last` vocabulary.
    pub actions: Vec<String>,
    /// Files and directories to remove after commit.
    pub paths: Vec<PathBuf>,
    /// The designs purged, as (id, project id): each is announced `design.deleted` after commit,
    /// as `design.purge` announces one, so an open board or Deleted list lets go of it (W72).
    pub designs: Vec<(i64, i64)>,
}

impl Purged {
    /// `design.deleted` for each design purged: what `design.purge` emits for one.
    pub fn deleted_designs(&self) -> impl Iterator<Item = serde_json::Value> + '_ {
        self.designs.iter().map(|(id, project_id)| json!({"id": id, "project_id": project_id}))
    }
}

fn setting_days(conn: &Connection, path: &str, default: i64) -> i64 {
    conn.prepare_cached("SELECT value FROM settings WHERE path=?1")
        .and_then(|mut stmt| stmt.query_row([path], |r| r.get::<_, String>(0)))
        .ok()
        .and_then(|value| serde_json::from_str::<i64>(&value).ok())
        .unwrap_or(default)
}

/// `undo.grace_days`: how long a soft-deleted row, and a picture nothing names, is kept; 0 or
/// less keeps them forever.
pub fn grace_days(conn: &Connection) -> i64 {
    setting_days(conn, "undo.grace_days", DEFAULT_GRACE_DAYS)
}

fn ids(tx: &Transaction, sql: &str, cutoff: &str) -> rusqlite::Result<Vec<i64>> {
    tx.prepare_cached(sql)?.query_map([cutoff], |r| r.get(0))?.collect()
}

/// Hard-delete what is past its window. `store_dir` is where attachments live, so a stored path
/// outside it is never handed back for removal.
pub fn run(tx: &Transaction, store_dir: &Path) -> rusqlite::Result<Purged> {
    let mut out = Purged::default();
    let mut note = |n: usize, what: &str| if n > 0 { out.actions.push(format!("purged {n} {what}")) };

    // Soft-deleted board rows, after the undo grace window. 0 (or less) keeps them forever.
    let grace = grace_days(tx);
    let mut paths = Vec::new();
    if grace > 0 {
        let cutoff = crate::time::days_ago(grace);
        let tasks = ids(tx, "SELECT id FROM tasks WHERE deleted_at IS NOT NULL AND deleted_at < ?1", &cutoff)?;
        let attachment_root = store_dir.join("attachments");
        for &id in &tasks {
            let files: Vec<String> = tx.prepare_cached("SELECT path FROM attachments WHERE task_id=?1")?
                .query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            paths.extend(files.into_iter().map(PathBuf::from).filter(|p| p.starts_with(&attachment_root)));
            // Everything that points at the task, so the delete passes the foreign keys. A live
            // sub-task of a purged parent becomes a root rather than keeping a dangling edge.
            tx.execute("DELETE FROM attachments WHERE task_id=?1", [id])?;
            tx.execute("DELETE FROM task_commits WHERE task_id=?1", [id])?;
            tx.execute("DELETE FROM task_sessions WHERE task_id=?1", [id])?;
            tx.execute("DELETE FROM module_unlinked_tasks WHERE task_id=?1", [id])?;
            tx.execute("UPDATE sessions SET task_id=NULL WHERE task_id=?1", [id])?;
            tx.execute("UPDATE messages SET re_task=NULL WHERE re_task=?1", [id])?;
            tx.execute("UPDATE tasks SET parent_id=NULL WHERE parent_id=?1", [id])?;
            tx.execute("DELETE FROM tasks WHERE id=?1", [id])?;
        }
        note(tasks.len(), "deleted task(s) past the undo window");

        // Attachments `task.detach` soft-deleted, row and file, once their undo is past (RA-409).
        let detached: Vec<(i64, String)> = tx.prepare_cached(
            "SELECT id, path FROM attachments WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        )?.query_map([&cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        for (id, path) in &detached {
            let path = PathBuf::from(path);
            if path.starts_with(&attachment_root) { paths.push(path); }
            tx.execute("DELETE FROM attachments WHERE id=?1", [id])?;
        }
        note(detached.len(), "detached attachment(s) past the undo window");

        let modules = ids(tx, "SELECT id FROM modules WHERE deleted_at IS NOT NULL AND deleted_at < ?1", &cutoff)?;
        for &id in &modules {
            tx.execute("UPDATE tasks SET module_id=NULL WHERE module_id=?1", [id])?;
            tx.execute("UPDATE sessions SET module_id=NULL WHERE module_id=?1", [id])?;
            tx.execute("DELETE FROM modules WHERE id=?1", [id])?;
        }
        note(modules.len(), "deleted module(s) past the undo window");

        // A task made from a purged note keeps its card: `ON DELETE SET NULL` clears its `note_id`.
        note(tx.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [&cutoff])?, "deleted note(s) past the undo window");

        // Deleted designs with their nodes. Their pictures stay in `design-assets/` for now: a
        // file is named by its content and may be any design's, a removal backup's or an undo's,
        // so the design-assets sweep, not this pass, decides when one goes.
        let designs: Vec<(i64, i64)> = tx.prepare_cached(
            "SELECT id, project_id FROM designs WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        )?.query_map([&cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        for &(id, _) in &designs {
            tx.prepare_cached("DELETE FROM design_removed WHERE design_id=?1")?.execute([id])?;
            tx.prepare_cached("DELETE FROM design_nodes WHERE design_id=?1")?.execute([id])?;
            tx.prepare_cached("DELETE FROM designs WHERE id=?1")?.execute([id])?;
        }
        note(designs.len(), "deleted design(s) past the undo window");

        // Deleted videos' rows. Their folders stay on disk: they are someone's code, and Relay does
        // no destructive cleanup of its own (D167); the person deletes a folder when they want it gone.
        let videos: Vec<i64> = tx.prepare_cached("SELECT id FROM videos WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?
            .query_map([&cutoff], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for &id in &videos {
            tx.prepare_cached("DELETE FROM videos WHERE id=?1")?.execute([id])?;
        }
        note(videos.len(), "deleted video(s) past the undo window");
        out.designs = designs;

        // Trashed files: the row and its `.relay/trash/<id>/` directory. Restored rows have no
        // payload left; their row is only history.
        let trash: Vec<(i64, String)> = tx.prepare_cached(
            "SELECT id, trash_path FROM file_trash WHERE created_at < ?1 AND restored_at IS NULL",
        )?.query_map([&cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        for (id, path) in &trash {
            if let Some(dir) = trash_dir(Path::new(path), *id) { paths.push(dir); }
        }
        let removed = tx.execute("DELETE FROM file_trash WHERE created_at < ?1", [&cutoff])?;
        note(removed, "trashed file record(s) past the undo window");

        // A closed session's scrollback is what made it restorable; past the window it is only
        // bytes. The row itself stays: audit, mailbox and task history still name it.
        note(tx.execute(
            "DELETE FROM session_scrollback WHERE session_id IN
               (SELECT id FROM sessions WHERE state='closed' AND closed_at IS NOT NULL AND closed_at < ?1)",
            [&cutoff],
        )?, "closed session scrollback(s)");
    }

    // Notifications: read ones after a month, any after half a year, and never more than the cap.
    let n = tx.execute("DELETE FROM notifications WHERE read=1 AND created_at < ?1", [crate::time::days_ago(READ_NOTIFICATION_DAYS)])?
        + tx.execute("DELETE FROM notifications WHERE created_at < ?1", [crate::time::days_ago(NOTIFICATION_DAYS)])?
        + tx.execute(
            "DELETE FROM notifications WHERE id NOT IN (SELECT id FROM notifications ORDER BY created_at DESC, id DESC LIMIT ?1+0)",
            [MAX_NOTIFICATIONS],
        )?;
    note(n, "old notification(s)");

    let (holds, held_texts) = crate::guardrail::prune_holds(tx, &crate::time::days_ago(ANSWERED_HOLD_DAYS))?;
    note(holds, "answered guardrail hold(s)");
    paths.extend(held_texts);

    // Mailbox: fully acked messages after a month, any after half a year. Recipients cascade.
    let n = tx.execute(
        "DELETE FROM messages WHERE sent_at < ?1
           AND NOT EXISTS (SELECT 1 FROM message_recipients r WHERE r.message_id=messages.id AND r.acked_at IS NULL)",
        [crate::time::days_ago(ACKED_MESSAGE_DAYS)],
    )? + tx.execute("DELETE FROM messages WHERE sent_at < ?1", params![crate::time::days_ago(MESSAGE_DAYS)])?;
    note(n, "old mailbox message(s)");

    // Hook rows past their window (`audit.hook_retention_days`, 0 keeps them for the general
    // window). Half of an undo pair is kept, as recovery's general pass keeps it.
    let hooks = setting_days(tx, "audit.hook_retention_days", HOOK_AUDIT_DAYS);
    if hooks > 0 {
        let cutoff = crate::time::days_ago(hooks);
        let n = tx.prepare_cached(
            "DELETE FROM audit
              WHERE op IN ('guardrail.gate', 'session.report', 'usage.report') AND kind = 'ok' AND ts < ?1
                AND undo_of IS NULL AND undone_by IS NULL",
        )?.execute([&cutoff])?;
        note(n, "allowed hook audit row(s) past their window");
    }
    out.paths = paths;
    Ok(out)
}

/// `<worktree>/.relay/trash/<id>` for a stored `.../.relay/trash/<id>/payload`, and nothing for a
/// path of any other shape: a corrupt row must not aim a recursive delete somewhere else.
fn trash_dir(payload: &Path, id: i64) -> Option<PathBuf> {
    let dir = payload.parent()?;
    let ok = payload.file_name()? == "payload"
        && dir.file_name()? == id.to_string().as_str()
        && dir.parent()?.file_name()? == "trash"
        && dir.parent()?.parent()?.file_name()? == ".relay";
    ok.then(|| dir.to_path_buf())
}

/// Remove what [`run`] handed back. Call it with no store lock held; a worker thread is best.
pub fn remove_paths(paths: &[PathBuf]) {
    for path in paths {
        // symlink_metadata: a link is removed as a link, never followed into what it names.
        let result = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
            Ok(_) => std::fs::remove_file(path),
            Err(_) => continue,
        };
        if let Err(error) = result {
            tracing::warn!(path = %path.display(), error = %error, "could not remove purged file");
        }
    }
}

/// First pass after start, then the interval between passes. Retention is measured in days, so
/// an hourly pass is plenty and costs a handful of indexed deletes.
const FIRST: std::time::Duration = std::time::Duration::from_secs(5 * 60);
const EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Run [`run`] on a worker, periodically, without going through the bus — a timer-driven
/// `app.reconcile` would land an audit row every pass. Holds only a weak reference, so it never
/// keeps an engine alive.
pub fn spawn_timer(engine: &std::sync::Arc<crate::engine::Engine>) {
    let weak = std::sync::Arc::downgrade(engine);
    std::thread::Builder::new().name("purge".into()).spawn(move || {
        crate::background_priority();
        let mut wait = FIRST;
        loop {
            let deadline = std::time::Instant::now() + wait;
            while std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_secs(1));
                match weak.upgrade() {
                    Some(engine) if !engine.is_quitting() => {}
                    _ => return,
                }
            }
            let Some(engine) = weak.upgrade() else { return };
            let store_dir = engine.store.path().parent().map(Path::to_path_buf).unwrap_or_default();
            let purged = {
                let mut conn = engine.store.lock();
                conn.transaction().and_then(|tx| {
                    let purged = run(&tx, &store_dir)?;
                    tx.commit().map(|_| purged)
                })
            };
            match purged {
                Ok(purged) => {
                    if !purged.actions.is_empty() {
                        tracing::info!(actions = ?purged.actions, "retention pass");
                        engine.emit_system("notify.changed", serde_json::json!({"purged": true}));
                    }
                    for deleted in purged.deleted_designs() {
                        engine.emit_system("design.deleted", deleted);
                    }
                    remove_paths(&purged.paths);
                }
                Err(error) => tracing::warn!(error = %error, "retention pass failed"),
            }
            // Pictures no design has named for an undo window (B66), then build output and
            // scratch space closed sessions left behind (`disk_sweep`).
            crate::disk_sweep::sweep_design_assets(&engine);
            crate::disk_sweep::run(&engine);
            drop(engine);
            wait = EVERY;
        }
    }).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_well_formed_trash_payload_names_a_directory_to_remove() {
        assert_eq!(trash_dir(Path::new("/w/.relay/trash/7/payload"), 7), Some(PathBuf::from("/w/.relay/trash/7")));
        assert_eq!(trash_dir(Path::new("/w/.relay/trash/7/payload"), 8), None);
        assert_eq!(trash_dir(Path::new("/w/src/7/payload"), 7), None);
        assert_eq!(trash_dir(Path::new("/w/.relay/trash/7"), 7), None);
        assert_eq!(trash_dir(Path::new(""), 7), None);
    }
}

#[cfg(test)]
mod hook_retention_tests {
    use super::*;

    /// Allowed hook rows go after `audit.hook_retention_days`; refusals, other ops and recent
    /// rows stay, and 0 keeps them all.
    #[test]
    fn allowed_hook_rows_past_their_window_are_purged() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&dir.path().join("store.db"), false).unwrap();
        let mut conn = store.lock();
        let row = |tx: &Transaction, op: &str, kind: &str, ts: &str| {
            tx.execute(
                "INSERT INTO audit (ts, req_id, actor, op, kind, payload_hash) VALUES (?1, lower(hex(randomblob(16))), 'user', ?2, ?3, 'h')",
                params![ts, op, kind],
            ).unwrap();
        };
        let old = crate::time::days_ago(40);
        let recent = crate::time::days_ago(2);
        let tx = conn.transaction().unwrap();
        row(&tx, "guardrail.gate", "ok", &old);
        row(&tx, "session.report", "ok", &old);
        row(&tx, "guardrail.gate", "refused", &old);
        row(&tx, "settings.set", "ok", &old);
        row(&tx, "guardrail.gate", "ok", &recent);
        run(&tx, dir.path()).unwrap();
        let left: Vec<(String, String)> = tx.prepare("SELECT op, kind FROM audit ORDER BY id").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
        let pairs: Vec<(&str, &str)> = left.iter().map(|(o, k)| (o.as_str(), k.as_str())).collect();
        assert_eq!(pairs, vec![("guardrail.gate", "refused"), ("settings.set", "ok"), ("guardrail.gate", "ok")]);

        tx.execute("INSERT INTO settings (path, value, updated_at) VALUES ('audit.hook_retention_days', '0', '')", []).unwrap();
        row(&tx, "session.report", "ok", &old);
        run(&tx, dir.path()).unwrap();
        let n: i64 = tx.query_row("SELECT COUNT(*) FROM audit WHERE op='session.report'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "0 keeps hook rows for the general window");
    }
}
