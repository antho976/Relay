//! `audit.*` (BUS.md §10.3): list, get and undo.

use crate::audit as log;
use crate::engine::{Ctx, Engine, IntoBus, Prepared};
use relay_bus::error::BusError;
use relay_bus::ops::audit::*;
use relay_bus::types::{AuditKind, AuditRow, UndoOp};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

fn target_updated_at(conn: &Connection, op: &str, payload: &Value) -> Result<Option<String>, BusError> {
    if let Some((table, key)) = match op {
        "project.reorder" => Some(("projects", "project_id")),
        "workspace.reorder" => Some(("workspaces", "workspace_id")),
        _ => None,
    } {
        return reordered_at(conn, table, key, payload);
    }
    let (table, key) = if op.starts_with("task.") {
        ("tasks", "task_id")
    } else if op.starts_with("module.") {
        ("modules", "module_id")
    } else if op.starts_with("notes.") {
        ("notes", "note_id")
    } else if op == "project.update" || op == "project.relink" {
        ("projects", "project_id")
    } else if op.starts_with("workspace.") {
        ("workspaces", "workspace_id")
    } else if op.starts_with("skill.") {
        ("skills", "skill_id")
    } else if op == "session.update" {
        // Sessions are addressed by name, which a closed session hands on to a later one.
        let Some(name) = payload.get("session").and_then(Value::as_str) else { return Ok(None) };
        return conn.query_row(
            "SELECT updated_at FROM sessions WHERE name=?1 AND state!='closed' ORDER BY id DESC LIMIT 1",
            [name], |row| row.get(0),
        ).optional().bus();
    } else {
        return Err(BusError::conflict("audit.stale", format!("cannot verify that the target of {op} is unchanged; pass force to undo anyway")));
    };
    let Some(id) = payload.get(key).and_then(Value::as_i64) else { return Ok(None) };
    let sql = format!("SELECT updated_at FROM {table} WHERE id=?1");
    conn.query_row(&sql, [id], |row| row.get(0)).optional().bus()
}

/// The open session `session.update`'s inverse names, as JSON: names pass to later sessions.
fn open_session(conn: &Connection, payload: &Value) -> Result<Option<Value>, BusError> {
    let Some(name) = payload.get("session").and_then(Value::as_str) else { return Ok(None) };
    let id: Option<i64> = conn.prepare_cached(
        "SELECT id FROM sessions WHERE name=?1 AND state!='closed' ORDER BY id DESC LIMIT 1",
    ).bus()?.query_row([name], |row| row.get(0)).optional().bus()?;
    let Some(id) = id else { return Ok(None) };
    let Some(row) = crate::sessions::by_id(conn, id)? else { return Ok(None) };
    serde_json::to_value(&row.session).map(Some).bus()
}

/// A reorder stamps every row it writes with one `updated_at`. That stamp while all of them
/// still carry it; `None` (stale) once any one was edited since, removed, or not named.
fn reordered_at(conn: &Connection, table: &str, key: &str, payload: &Value) -> Result<Option<String>, BusError> {
    let sql = format!("SELECT updated_at FROM {table} WHERE id=?1");
    let mut stmt = conn.prepare_cached(&sql).bus()?;
    let mut shared: Option<String> = None;
    for entry in payload.get("orders").and_then(Value::as_array).into_iter().flatten() {
        let Some(id) = entry.get(key).and_then(Value::as_i64) else { return Ok(None) };
        let Some(at) = stmt.query_row([id], |row| row.get::<_, String>(0)).optional().bus()? else { return Ok(None) };
        match &shared {
            Some(first) if *first != at => return Ok(None),
            Some(_) => {}
            None => shared = Some(at),
        }
    }
    Ok(shared)
}

/// The settings subtree an op writes: `""` is the whole tree.
fn settings_path(op: &str, payload: &Value) -> Option<String> {
    match op {
        "settings.set" | "settings.reset" => Some(payload.get("path").and_then(Value::as_str).unwrap_or("").to_string()),
        "notify.settings.set" => Some("notifications".into()),
        _ => None,
    }
}

fn paths_overlap(a: &str, b: &str) -> bool {
    a.is_empty() || b.is_empty() || a == b || a.starts_with(&format!("{b}.")) || b.starts_with(&format!("{a}."))
}

/// For ops that record no `expect` — settings and guardrail config have no `updated_at` per
/// value — the staleness test is whether a later successful op wrote the same thing.
fn written_since(conn: &Connection, row: &relay_bus::types::AuditRow) -> Result<bool, BusError> {
    let payload = row.payload.clone().unwrap_or(Value::Null);
    let later = |ops: &[&str]| -> Result<Vec<(String, Value)>, BusError> {
        let mut stmt = conn.prepare_cached(
            "SELECT op, payload FROM audit WHERE id > ?1 AND kind='ok' AND op IN (SELECT value FROM json_each(?2))",
        ).bus()?;
        let rows = stmt.query_map(rusqlite::params![row.id, serde_json::to_string(ops).unwrap_or_default()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        }).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
        Ok(rows.into_iter().map(|(op, p)| (op, p.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or(Value::Null))).collect())
    };
    if let Some(path) = settings_path(&row.op, &payload) {
        return Ok(later(&["settings.set", "settings.reset", "notify.settings.set"])?.iter()
            .any(|(op, p)| settings_path(op, p).is_none_or(|other| paths_overlap(&path, &other))));
    }
    if row.op == "guardrail.config.set" {
        let scope = |p: &Value| (p.get("workspace_id").cloned(), p.get("project_id").cloned());
        return Ok(later(&["guardrail.config.set"])?.iter().any(|(_, p)| scope(p) == scope(&payload)));
    }
    Ok(false)
}

/// Inverses whose prepare phase only reads, so it may run before the undo's own checks are
/// final: a refusal in the transaction then leaves nothing behind.
const PREPARED_BEFORE: &[&str] = &["file.restore"];

pub fn register(e: &mut Engine) {
    // Unlocked (D149): a list is up to 1000 rows of up to 64 KB payload and result each. Only
    // copying the text out holds the store; parsing it runs after the lock is released.
    e.register_unlocked::<List>(|ctx, p| {
        let since = p.since.as_deref().map(|s| crate::time::bound("since", s)).transpose()?;
        let until = p.until.as_deref().map(|s| crate::time::bound("until", s)).transpose()?;
        let f = log::ListFilter {
            project_id: p.project_id,
            actor: p.actor.as_ref(),
            session_id: p.session_id,
            op_prefix: p.op_prefix.as_deref(),
            parent_req: p.parent_req.as_deref(),
            since: since.as_deref(),
            until: until.as_deref(),
            limit: p.limit.unwrap_or(200),
        };
        let raw = ctx.read(|conn| log::list_raw(conn, &f).bus())?;
        Ok(ListOut { rows: raw.into_iter().map(log::RawAudit::parse).collect() })
    });
    e.register_unlocked::<Get>(|ctx, p| {
        ctx.read(|conn| log::get_raw(conn, p.audit_id).bus())?
            .map(log::RawAudit::parse)
            .ok_or_else(|| BusError::not_found("audit.not_found", format!("no audit row {}", p.audit_id)))
    });
    // Staged (D149): the inverse is a registered op, and the read phase of one that only reads —
    // `file.restore` checking where the file goes back to and walking a trashed tree — runs here,
    // before the transaction, as the caller, for an undo that passes its checks now; the
    // transaction checks again. Any other inverse's prepare still runs inside the transaction,
    // after those checks: `session.update`'s renames a branch, which a refusal there could not
    // take back.
    e.register_staged::<Undo, Option<(UndoOp, Option<Result<Prepared, BusError>>)>>(
        |ctx, p| {
            let force = p.force.unwrap_or(false);
            // Refused now is refused in the transaction too, which says why.
            let Ok((_, inverse)) = ctx.read(|conn| undoable(conn, p.audit_id, force)) else { return Ok(None) };
            if !PREPARED_BEFORE.contains(&inverse.op.as_str()) {
                return Ok(None);
            }
            let prepared = ctx.prepare_registered(&inverse.op, &inverse.payload, ctx.actor.clone(), ctx.actor_session_id()).transpose();
            Ok(Some((inverse, prepared)))
        },
        |ctx: &mut Ctx, p, staged| {
            let (row, inverse) = undoable(ctx.tx(), p.audit_id, p.force.unwrap_or(false))?;
            let expect = inverse.expect.clone().unwrap_or(Value::Null);
            // An audit row's inverse never changes, so what was prepared is this one's; any other
            // inverse, or a row that only became undoable since, prepares here, as before.
            let prepared = match staged {
                Some((prepared_for, prepared)) if prepared_for.op == inverse.op && prepared_for.payload == inverse.payload => {
                    prepared.transpose()?
                }
                _ => None,
            };
            let by: i64 = ctx.tx().query_row("SELECT COALESCE(MAX(id),0)+1 FROM audit", [], |r| r.get(0)).bus()?;
            ctx.invoke_prepared(&inverse.op, inverse.payload, prepared)?;
            // The queue rows the update added, as (task, session) pairs, unless already worked.
            if inverse.op == "session.update" {
                for pair in expect.get("queued").and_then(Value::as_array).into_iter().flatten() {
                    let (Some(task_id), Some(session_id)) = (pair.get(0).and_then(Value::as_i64), pair.get(1).and_then(Value::as_i64)) else { continue };
                    ctx.tx().prepare_cached("DELETE FROM task_sessions WHERE task_id=?1 AND session_id=?2 AND completed_at IS NULL").bus()?
                        .execute([task_id, session_id]).bus()?;
                }
            }
            ctx.set_undo_of(row.id);
            Ok(UndoOut { undone: row.id, by })
        },
    );
}

/// Audit row `audit_id` and its inverse, if it may be undone now: it succeeded, recorded an
/// inverse, was not undone already and — unless `force` — nothing it wrote changed since (§5.5).
fn undoable(conn: &Connection, audit_id: i64, force: bool) -> Result<(AuditRow, UndoOp), BusError> {
    let row = log::get(conn, audit_id).bus()?
        .ok_or_else(|| BusError::not_found("audit.not_found", format!("no audit row {audit_id}")))?;
    if row.kind != AuditKind::Ok || row.undo_op.is_none() {
        return Err(BusError::conflict("audit.not_undoable", format!("audit row {} has no successful inverse", row.id)));
    }
    if row.undone_by.is_some() {
        return Err(BusError::conflict("audit.not_undoable", format!("audit row {} was already undone", row.id)));
    }
    let inverse = row.undo_op.clone().expect("checked above");
    if force {
        return Ok((row, inverse));
    }
    let expect = inverse.expect.clone().unwrap_or(Value::Null);
    // `session.update` records the fields it wrote: a running session's every report moves
    // its `updated_at`, so only one of those fields changing again makes the undo stale.
    // Rows recorded before that carry `updated_at` and take the general path (RA-403).
    let fields = (inverse.op == "session.update").then(|| expect.get("fields").and_then(Value::as_object)).flatten();
    if let Some(fields) = fields {
        let session = open_session(conn, &inverse.payload)?
            .ok_or_else(|| BusError::conflict("audit.stale", "the session was closed after the audited operation"))?;
        let moved: Vec<&String> = fields.iter()
            .filter(|(name, wrote)| session.get(name.as_str()).unwrap_or(&Value::Null) != *wrote)
            .map(|(name, _)| name).collect();
        if !moved.is_empty() {
            return Err(BusError::conflict("audit.stale", "the session's fields changed after the audited operation")
                .with_details(serde_json::json!({"fields": moved})));
        }
    } else if let Some(expected) = expect.get("updated_at").and_then(Value::as_str) {
        let actual = target_updated_at(conn, &inverse.op, &inverse.payload)?;
        if actual.as_deref() != Some(expected) {
            return Err(BusError::conflict("audit.stale", "the entity changed after the audited operation")
                .with_details(serde_json::json!({"expected": expected, "actual": actual})));
        }
    } else if written_since(conn, &row)? {
        return Err(BusError::conflict("audit.stale", "a later operation changed the same setting; pass force to undo anyway"));
    }
    Ok((row, inverse))
}
