//! The audit log (BUS.md §0.4, §5.1a, §5.3, §13): one row per audited mutation outcome,
//! `req_id` UNIQUE forever, replay-able.

use anyhow::Result;
use relay_bus::envelope::{Actor, Response};
use relay_bus::error::BusError;
use relay_bus::types::{AuditKind, AuditRow, Id, UndoOp};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Payloads and results larger than this are hashed but not stored (BUS.md §11.4).
pub const STORE_LIMIT: usize = 64 * 1024;

/// The provider hooks call `session.report` on every tool use, and an agent may send
/// `usage.report` as often as it likes (no hook sends it: the Claude status line writes a file
/// `usage.get` reads instead). Their payloads are high-volume and near-identical, and 64 KB of
/// each is what grew the audit table past the rest of the store put together. The hash still identifies the
/// request and the result summary still replays it; only the readable copy is bounded.
/// `guardrail.gate` is the hottest of all — the write hook sends it before every agent edit, with
/// the edit's full new text — and its hold, when there is one, keeps its own frozen envelope.
pub const CHATTY_STORE_LIMIT: usize = 2 * 1024;

/// What an allowed `guardrail.gate` or an accepted `session.report` keeps of its payload: who,
/// what kind of call, and the command, path or tool name — not the hook's whole envelope
/// (transcript path, working directory, the tool's full input). Those two ops are an audit row
/// per agent tool call, and their full payloads were 85 % of a 260 MB store
/// (BASELINE-2026-10-14 §1.2). Refusals and holds are not ok outcomes and keep everything. A
/// payload without a `kind` is not a hook's and is kept as sent. Migration v27 slims older rows
/// the same way.
fn slim(op: &str, payload: &Value) -> Option<String> {
    let kind = payload.get("kind").filter(|k| !k.is_null())?;
    let slim = match op {
        "guardrail.gate" => {
            let command = payload["command"].as_str().map(|c| c.chars().take(200).collect::<String>());
            serde_json::json!({"session": payload["session"], "kind": kind, "command": command, "path": payload["path"], "slim": true})
        }
        "session.report" => serde_json::json!({"session": payload["session"], "kind": kind, "tool": payload["data"]["tool_name"], "slim": true}),
        _ => return None,
    };
    serde_json::to_string(&slim).ok()
}

fn store_limit(op: &str) -> usize {
    match op {
        "session.report" | "usage.report" | "guardrail.gate" => CHATTY_STORE_LIMIT,
        _ => STORE_LIMIT,
    }
}

pub fn payload_hash(payload: &Value) -> String {
    hash_json(&serde_json::to_vec(payload).unwrap_or_default())
}

/// The same hash, over a payload that has already been serialized. `to_vec` and `to_string`
/// emit identical bytes, so [`append`] hashes the copy it is about to store rather than
/// serializing the payload a second time to produce the same digest.
fn hash_json(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    crate::hex(&h.finalize())
}

/// What the pipeline records for one request.
pub struct AuditEntry<'a> {
    pub ts: &'a str,
    pub req_id: Uuid,
    pub parent_req: Option<Uuid>,
    pub actor: &'a Actor,
    pub on_behalf_of: Option<&'a Actor>,
    pub session_id: Option<Id>,
    pub op: &'a str,
    pub project_id: Option<Id>,
    pub payload: &'a Value,
    /// `Ok(result)` or the error the request ended with.
    pub outcome: &'a Result<Value, BusError>,
    pub hold_id: Option<Id>,
    pub undo_op: Option<&'a UndoOp>,
    pub undo_of: Option<Id>,
}

pub fn kind_of(outcome: &Result<Value, BusError>) -> AuditKind {
    match outcome {
        Ok(_) => AuditKind::Ok,
        Err(e) => match e.kind.audit_kind() {
            "held" => AuditKind::Held,
            "refused" => AuditKind::Refused,
            _ => AuditKind::Error,
        },
    }
}

fn kind_str(k: AuditKind) -> &'static str {
    match k {
        AuditKind::Ok => "ok",
        AuditKind::Held => "held",
        AuditKind::Refused => "refused",
        AuditKind::Error => "error",
    }
}

/// Append one row inside the caller's transaction. Returns the row id.
pub fn append(tx: &Transaction, e: &AuditEntry) -> Result<Id> {
    let payload_json = serde_json::to_string(e.payload)?;
    let payload_hash = hash_json(payload_json.as_bytes());
    // Over the limit the row keeps a marker rather than NULL: "we chose not to store this" and
    // "there was nothing to store" are different answers to `audit.get`. Built only when the
    // payload is actually over the limit, which is the rare case.
    let oversize;
    let slimmed = if e.outcome.is_ok() { slim(e.op, e.payload) } else { None };
    let payload_col = if let Some(slimmed) = &slimmed {
        slimmed
    } else if payload_json.len() <= store_limit(e.op) {
        &payload_json
    } else {
        oversize = serde_json::json!({"truncated": true, "bytes": payload_json.len()}).to_string();
        &oversize
    };
    let (kind, code, summary) = match e.outcome {
        Ok(v) => {
            let s = serde_json::to_string(v)?;
            let s = if s.len() <= STORE_LIMIT { s } else { serde_json::json!({"truncated": true, "bytes": s.len()}).to_string() };
            (AuditKind::Ok, None, Some(s))
        }
        Err(err) => (kind_of(e.outcome), Some(err.code.clone()), Some(serde_json::to_string(err)?)),
    };
    tx.prepare_cached(
        "INSERT INTO audit (ts, req_id, parent_req, actor, on_behalf_of, session_id, op, project_id, kind, code, hold_id,
                            payload_hash, payload, result_summary, undo_op, undo_of)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
    )?
    .execute(
        params![
            e.ts,
            e.req_id.to_string(),
            e.parent_req.map(|u| u.to_string()),
            e.actor.to_string(),
            e.on_behalf_of.map(|a| a.to_string()),
            e.session_id,
            e.op,
            e.project_id,
            kind_str(kind),
            code,
            e.hold_id,
            payload_hash,
            payload_col.as_str(),
            summary,
            e.undo_op.map(|u| serde_json::to_string(u).unwrap_or_default()),
            e.undo_of,
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// A previously recorded outcome for `req_id`, for idempotent replay (BUS.md §5.3).
pub struct Recorded {
    pub payload_hash: String,
    pub response: Response,
}

pub fn lookup(conn: &Connection, req_id: Uuid) -> Result<Option<Recorded>> {
    // Runs before every audited mutation, to answer "have I already done this?".
    let row = conn
        .prepare_cached("SELECT payload_hash, kind, result_summary FROM audit WHERE req_id = ?1")?
        .query_row(
            [req_id.to_string()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
        )
        .optional()?;
    let Some((hash, kind, summary)) = row else { return Ok(None) };
    let response = match (kind.as_str(), summary) {
        ("ok", Some(s)) => {
            let v: Value = serde_json::from_str(&s)?;
            if v.get("truncated").is_some() && v.get("bytes").is_some() && v.as_object().map(|o| o.len() == 2).unwrap_or(false) {
                Response::err(req_id, BusError::conflict("bus.replay_unavailable", "the recorded result was too large to keep; re-issue with a new id"))
            } else {
                Response::ok(req_id, v)
            }
        }
        (_, Some(s)) => Response::err(req_id, serde_json::from_str(&s)?),
        (_, None) => Response::err(req_id, BusError::internal("audit row without a recorded outcome")),
    };
    Ok(Some(Recorded { payload_hash: hash, response }))
}

fn parse_actor(s: &str) -> Actor {
    Actor::parse(s).unwrap_or(Actor::System)
}

/// An audit row as stored: every column copied out, the JSON ones still text. Reading this is
/// the only part of `audit.list`/`audit.get` that holds the store; [`RawAudit::parse`] — up to
/// 1000 rows of 64 KB payload and result each — runs after the lock is released.
pub struct RawAudit {
    id: Id,
    ts: String,
    req_id: String,
    parent_req: Option<String>,
    actor: String,
    on_behalf_of: Option<String>,
    session_id: Option<Id>,
    op: String,
    project_id: Option<Id>,
    kind: String,
    code: Option<String>,
    hold_id: Option<Id>,
    payload_hash: String,
    payload: Option<String>,
    result_summary: Option<String>,
    undo_op: Option<String>,
    undo_of: Option<Id>,
    undone_by: Option<Id>,
}

const COLUMNS: &str = "id, ts, req_id, parent_req, actor, on_behalf_of, session_id, op, project_id, kind, code, hold_id,
                       payload_hash, payload, result_summary, undo_op, undo_of, undone_by";

fn raw_row(r: &Row) -> rusqlite::Result<RawAudit> {
    Ok(RawAudit {
        id: r.get(0)?, ts: r.get(1)?, req_id: r.get(2)?, parent_req: r.get(3)?, actor: r.get(4)?,
        on_behalf_of: r.get(5)?, session_id: r.get(6)?, op: r.get(7)?, project_id: r.get(8)?,
        kind: r.get(9)?, code: r.get(10)?, hold_id: r.get(11)?, payload_hash: r.get(12)?,
        payload: r.get(13)?, result_summary: r.get(14)?, undo_op: r.get(15)?, undo_of: r.get(16)?,
        undone_by: r.get(17)?,
    })
}

impl RawAudit {
    pub fn parse(self) -> AuditRow {
        let kind = match self.kind.as_str() {
            "ok" => AuditKind::Ok,
            "held" => AuditKind::Held,
            "refused" => AuditKind::Refused,
            _ => AuditKind::Error,
        };
        AuditRow {
            id: self.id,
            ts: self.ts,
            req_id: self.req_id,
            parent_req: self.parent_req,
            actor: parse_actor(&self.actor),
            on_behalf_of: self.on_behalf_of.map(|s| parse_actor(&s)),
            session_id: self.session_id,
            op: self.op,
            project_id: self.project_id,
            kind,
            code: self.code,
            hold_id: self.hold_id,
            payload_hash: self.payload_hash,
            payload: self.payload.and_then(|s| serde_json::from_str(&s).ok()),
            result_summary: self.result_summary.and_then(|s| serde_json::from_str(&s).ok()),
            undo_op: self.undo_op.and_then(|s| serde_json::from_str(&s).ok()),
            undo_of: self.undo_of,
            undone_by: self.undone_by,
        }
    }

    /// What `serde_json::to_value(self.parse())` gives, without copying the parsed JSON. The
    /// payload, result and inverse are most of a row and are already `Value`s once parsed: the
    /// typed row's fields are serialized with those three empty, and the parsed values are moved
    /// into their places, which keep their order. `audit.list` was half allocation, copying them.
    pub fn into_json(mut self) -> Value {
        let payload = self.payload.take().and_then(|s| serde_json::from_str::<Value>(&s).ok());
        let result = self.result_summary.take().and_then(|s| serde_json::from_str::<Value>(&s).ok());
        let undo = self.undo_op.take().and_then(|s| serde_json::from_str::<UndoOp>(&s).ok());
        let mut row = serde_json::to_value(self.parse()).unwrap_or_default();
        if let Value::Object(map) = &mut row {
            map.insert("payload".into(), payload.unwrap_or(Value::Null));
            map.insert("result_summary".into(), result.unwrap_or(Value::Null));
            map.insert("undo_op".into(), undo.map_or(Value::Null, |u| {
                let mut out = serde_json::Map::new();
                out.insert("op".into(), Value::String(u.op));
                out.insert("payload".into(), u.payload);
                if let Some(expect) = u.expect {
                    out.insert("expect".into(), expect);
                }
                Value::Object(out)
            }));
        }
        row
    }
}

pub struct ListFilter<'a> {
    pub project_id: Option<Id>,
    pub actor: Option<&'a Actor>,
    pub session_id: Option<Id>,
    pub op_prefix: Option<&'a str>,
    pub parent_req: Option<&'a str>,
    pub since: Option<&'a str>,
    pub until: Option<&'a str>,
    pub limit: u32,
}

/// The rows `f` selects, unparsed: run under the lock, then [`RawAudit::parse`] each outside it.
pub fn list_raw(conn: &Connection, f: &ListFilter) -> Result<Vec<RawAudit>> {
    let mut sql = format!("SELECT {COLUMNS} FROM audit WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = f.project_id { sql.push_str(" AND project_id = ?"); args.push(Box::new(p)); }
    if let Some(a) = f.actor { sql.push_str(" AND actor = ?"); args.push(Box::new(a.to_string())); }
    if let Some(s) = f.session_id { sql.push_str(" AND session_id = ?"); args.push(Box::new(s)); }
    if let Some(p) = f.op_prefix { sql.push_str(" AND op LIKE ? ESCAPE '\\'"); args.push(Box::new(format!("{}%", p.replace('%', "\\%").replace('_', "\\_")))); }
    if let Some(p) = f.parent_req { sql.push_str(" AND parent_req = ?"); args.push(Box::new(p.to_string())); }
    if let Some(s) = f.since { sql.push_str(" AND ts >= ?"); args.push(Box::new(s.to_string())); }
    if let Some(u) = f.until { sql.push_str(" AND ts <= ?"); args.push(Box::new(u.to_string())); }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    args.push(Box::new(f.limit.min(1000)));
    // Keyed by SQL text, so each combination of filters compiles once.
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), raw_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_raw(conn: &Connection, id: Id) -> Result<Option<RawAudit>> {
    Ok(conn.prepare_cached(&format!("SELECT {COLUMNS} FROM audit WHERE id = ?1"))?.query_row([id], raw_row).optional()?)
}

pub fn get(conn: &Connection, id: Id) -> Result<Option<AuditRow>> {
    Ok(get_raw(conn, id)?.map(RawAudit::parse))
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    fn write(tx: &Transaction, op: &str, payload: &Value, outcome: &std::result::Result<Value, BusError>) -> (String, String) {
        let actor = Actor::User;
        let id = append(tx, &AuditEntry {
            ts: "2026-10-14T00:00:00Z", req_id: Uuid::new_v4(), parent_req: None, actor: &actor, on_behalf_of: None,
            session_id: None, op, project_id: None, payload, outcome, hold_id: None, undo_op: None, undo_of: None,
        }).unwrap();
        tx.query_row("SELECT payload, payload_hash FROM audit WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
    }

    /// An allowed gate and an accepted report keep who, what and the command or tool; the hash
    /// still names the full payload, and a refusal keeps everything.
    #[test]
    fn allowed_hook_rows_keep_a_summary_and_refusals_keep_everything() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&dir.path().join("store.db"), false).unwrap();
        let mut conn = store.lock();
        let tx = conn.transaction().unwrap();
        let long = "x".repeat(500);
        let gate = serde_json::json!({"session": "s1", "kind": "exec", "command": format!("cargo test {long}"),
            "transcript_path": "/home/someone/.claude/t.jsonl", "cwd": "/w"});
        let (payload, hash) = write(&tx, "guardrail.gate", &gate, &Ok(serde_json::json!({"verdict": "allow"})));
        let kept: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(kept["session"], "s1");
        assert_eq!(kept["kind"], "exec");
        assert_eq!(kept["slim"], true);
        assert_eq!(kept["command"].as_str().unwrap().chars().count(), 200);
        assert!(kept.get("transcript_path").is_none() && kept.get("cwd").is_none());
        assert_eq!(hash, payload_hash(&gate), "the hash is the full payload's");

        let report = serde_json::json!({"session": "s1", "kind": "tool_use", "data": {"tool_name": "Bash", "tool_input": {"command": long}}});
        let (payload, _) = write(&tx, "session.report", &report, &Ok(Value::Null));
        assert_eq!(serde_json::from_str::<Value>(&payload).unwrap(), serde_json::json!({"session": "s1", "kind": "tool_use", "tool": "Bash", "slim": true}));

        let refused = Err(BusError::refused("guardrail.command", "denied"));
        let (payload, _) = write(&tx, "guardrail.gate", &gate, &refused);
        assert_eq!(serde_json::from_str::<Value>(&payload).unwrap(), gate, "a refusal keeps the whole payload");

        let other = serde_json::json!({"path": "appearance.mode", "value": "dark"});
        let (payload, _) = write(&tx, "settings.set", &other, &Ok(Value::Null));
        assert_eq!(serde_json::from_str::<Value>(&payload).unwrap(), other, "other ops are not slimmed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `audit.list` and `audit.get` answer with `into_json`, not the typed row, so it must be the
    /// typed row exactly, key order included: for every column empty or full, a payload or result
    /// that no longer parses, and an inverse with and without `expect` or of the wrong shape.
    #[test]
    fn into_json_is_the_typed_row_exactly() {
        let store = crate::Store::open_memory().unwrap();
        let rows = [
            ("ok", Some(r#"{"path":"a.b","value":{"x":[1,2,{"é":null}],"y":"s"}}"#), Some(r#"{"value":3}"#), Some(r#"{"op":"settings.set","payload":{"path":"a.b","value":1}}"#), None),
            ("ok", Some("[]"), Some("null"), Some(r#"{"op":"task.update","payload":{"task_id":4},"expect":{"updated_at":"t"}}"#), Some("builder:7")),
            ("held", None, None, None, None),
            ("refused", Some("not json"), Some("{truncated"), Some(r#"{"no":"op"}"#), None),
            ("error", Some("1.5e3"), Some(r#"{"code":"x.y","message":"m"}"#), None, Some("nonsense")),
        ];
        store.with_tx(|tx| {
            for (i, (kind, payload, result, undo, on_behalf)) in rows.iter().enumerate() {
                tx.execute(
                    "INSERT INTO audit (ts, req_id, parent_req, actor, on_behalf_of, session_id, op, project_id, kind, code, hold_id, payload_hash, payload, result_summary, undo_op)
                     VALUES ('2026-10-14T00:00:00Z', ?1, NULL, 'user', ?2, NULL, 'settings.set', NULL, ?3, NULL, NULL, 'h', ?4, ?5, ?6)",
                    params![format!("r{i}"), on_behalf, kind, payload, result, undo],
                )?;
            }
            Ok(())
        }).unwrap();
        store.with_tx(|tx| {
            let f = ListFilter { project_id: None, actor: None, session_id: None, op_prefix: None, parent_req: None, since: None, until: None, limit: 100 };
            let typed: Vec<Value> = list_raw(tx, &f)?.into_iter().map(|r| serde_json::to_value(r.parse()).unwrap()).collect();
            let direct: Vec<Value> = list_raw(tx, &f)?.into_iter().map(RawAudit::into_json).collect();
            assert_eq!(typed.len(), rows.len());
            for (t, d) in typed.iter().zip(&direct) {
                assert_eq!(serde_json::to_string(d).unwrap(), serde_json::to_string(t).unwrap());
            }
            Ok(())
        }).unwrap();
    }
}
