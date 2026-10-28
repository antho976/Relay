//! The assistant's asks (docs/THREADS.md, "The assistant"): what the Dev space's assistant may not
//! do alone — what a lone session may not do — it asks for, and the person approves or declines
//! each ask on a card.
//!
//! An ask is a few steps, each a user-only op from [`ASKABLE`] with its own payload, and a reason.
//! The person may let some kinds of step run without asking ([`Allowed`], the setting
//! `threads.assistant.without_asking`): an ask made only of those is claimed at once, runs after
//! its request's transaction, and its reply is what came of it (`auto`), so the agent hears it
//! there and not again with the next message. The same steps asked again within minutes are that
//! ask, not a second run; past [`MAX_AUTO_PER_HOUR`], and for an agent started outside a new
//! checkout of its own ([`reaches`]), it waits on a card all the same.
//! It lives in `threads.db`, tied to the assistant thread whose agent sent it (`Ctx::thread`, set
//! by the socket door from the process that connected). It is checked when it is made: each op on
//! the list, no flag that throws work away or widens what a session may do ([`REFUSED_FLAGS`]),
//! at any depth of the payload, at most [`MAX_STEPS`] steps. A `session.close` that leaves
//! `remove_worktree` out keeps the checkout, where the op itself would remove it ([`kept`]). The
//! op checks its own payload when it runs.
//!
//! Approving claims the ask (`pending` → `running`) in the answer's transaction, so a second
//! answer is refused, then runs the steps once, in order, as the person, each its own request
//! through the engine's own pipeline, after that transaction and before the reply. The first step
//! that fails stops the rest; which steps ran before it is kept with the failure. A `session` left
//! out of a step after `session.create` is the session that step made.
//!
//! The agent hears how each ask was answered at the head of the person's next message
//! ([`heard`]), once per answer.
use super::{cut, with_db, ThreadStore};
use crate::engine::Engine;
use relay_bus::ops::thread::{AskStep, AskView};
use relay_bus::types::Id;
use relay_bus::{Actor, BusError, Request};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

/// The ops an ask may hold: what changes a session, dispatching or deleting a task, and adding a
/// workspace or a project. Creating and editing tasks and notes the assistant does itself
/// ([`super::ASSISTANT_OPS`]). Never guardrails, settings, the app, removing a workspace or
/// project, worktrees, Git, files, money, keys or pairing: those stay the person's to do themselves.
pub const ASKABLE: &[&str] = &[
    "session.create",
    "session.spawn",
    "session.resume",
    "session.park",
    "session.wake",
    "session.close",
    "session.input",
    "session.update",
    "task.dispatch",
    "task.delete",
    "workspace.create",
    "project.add",
];

/// The kinds of step the person may let run without asking, each a key of
/// `threads.assistant.without_asking`: launching, waking and setting up agents; typing into a
/// session's terminal, which can answer the agent's own permission prompts; and stopping one.
/// A step of no kind (deleting a task, adding a workspace or a project) always waits on a card;
/// one the assistant may do itself ([`super::ASSISTANT_OPS`]) never needs to.
pub const KINDS: &[(&str, &[&str])] = &[
    ("launch", &["session.create", "session.spawn", "session.resume", "session.wake", "session.update", "task.dispatch"]),
    ("type", &["session.input"]),
    ("stop", &["session.park", "session.close"]),
];

/// Which [`KINDS`] the person lets run without asking, as read from the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowed(Vec<&'static str>);

impl Allowed {
    /// From `threads.assistant.without_asking`, `{kind: bool}`: only a kind set to `true`.
    pub fn from_setting(value: &Value) -> Allowed {
        Allowed(KINDS.iter().map(|(kind, _)| *kind).filter(|kind| value.get(*kind) == Some(&Value::Bool(true))).collect())
    }

    /// The setting, read in the request's transaction.
    pub fn read(tx: &rusqlite::Transaction) -> Allowed {
        crate::handlers::settings::get(tx, Some("threads.assistant.without_asking")).map(|v| Allowed::from_setting(&v)).unwrap_or_default()
    }

    /// Whether `op` runs without asking: the assistant's own, or of a kind let through.
    fn lets(&self, op: &str) -> bool {
        super::ASSISTANT_OPS.contains(&op) || KINDS.iter().any(|(kind, ops)| self.0.contains(kind) && ops.contains(&op))
    }

    /// Whether every step runs without asking: each of a kind let through, and none reaching
    /// past a fresh checkout of its own ([`reaches`]).
    pub fn lets_all(&self, steps: &[AskStep]) -> bool {
        steps.iter().all(|step| self.lets(&step.op) && !reaches(&step.payload))
    }
}

/// Whether a payload, at any depth (a `task.dispatch` carries a whole `session.create`), starts an
/// agent anywhere but a new checkout of its own: a `worktree` other than `"new"` (the person's
/// primary checkout, or a peer's) or a `pair_with`. Allowed, but only on a card that shows it.
fn reaches(value: &Value) -> bool {
    match value {
        Value::Object(fields) => {
            fields.get("worktree").is_some_and(|w| !w.is_null() && w.as_str() != Some("new"))
                || fields.get("pair_with").is_some_and(|p| !p.is_null())
                || fields.values().any(reaches)
        }
        Value::Array(items) => items.iter().any(reaches),
        _ => false,
    }
}

/// Payload fields an ask may not set to `true`, at any depth (a `task.dispatch` carries a whole
/// `session.create` in `create`): deleting uncommitted work or a worktree, and widening what a
/// session may do or what it bills.
const REFUSED_FLAGS: &[&str] = &["discard_changes", "remove_worktree", "remove_worktrees", "force", "bus_writes", "allow_ui", "use_api_key"];

/// The most steps one ask may hold.
pub const MAX_STEPS: usize = 5;
/// The most asks one thread may have waiting at once.
pub const MAX_PENDING: i64 = 20;
/// The most asks one thread runs without asking in an hour; past it, each waits on a card.
pub const MAX_AUTO_PER_HOUR: i64 = 10;
/// How long an ask that ran without asking stands for the same steps asked again: a model that
/// repeats a tool call gets the first one's answer, not a second agent.
const AUTO_SAME_MINUTES: i64 = 10;
/// The longest a reason may be, in characters.
const MAX_WHY: usize = 1_000;

pub(super) const SCHEMA: &str = "
CREATE TABLE ask (
    id INTEGER PRIMARY KEY,
    thread_id INTEGER NOT NULL REFERENCES thread(id) ON DELETE CASCADE,
    steps TEXT NOT NULL,
    why TEXT NOT NULL,
    summary TEXT NOT NULL,
    status TEXT NOT NULL,
    outcome TEXT,
    results TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    told TEXT
);
CREATE INDEX ask_thread ON ask(thread_id, id);
";

const COLUMNS: &str = "id, thread_id, steps, why, summary, status, outcome, results, created_at, resolved_at, auto";

/// An ask's answer, for the agent: (id, status, summary, outcome).
type Answer = (Id, String, String, Option<String>);

fn row(r: &rusqlite::Row) -> rusqlite::Result<AskView> {
    let steps: String = r.get(2)?;
    let results: String = r.get(7)?;
    Ok(AskView {
        id: r.get(0)?,
        thread_id: r.get(1)?,
        steps: serde_json::from_str(&steps).unwrap_or_default(),
        why: r.get(3)?,
        summary: r.get(4)?,
        status: r.get(5)?,
        outcome: r.get(6)?,
        results: serde_json::from_str(&results).unwrap_or_default(),
        created_at: r.get(8)?,
        resolved_at: r.get(9)?,
        auto: r.get(10)?,
    })
}

impl ThreadStore {
    /// An ask Relay stopped in the middle of running: said, never left running.
    pub(super) fn settle_interrupted_asks(&self, now: &str) -> rusqlite::Result<()> {
        self.conn.prepare_cached(
            "UPDATE ask SET status = 'failed', outcome = 'Relay stopped while it was running: check what it did before asking again', resolved_at = ?1 WHERE status = 'running'",
        )?.execute([now])?;
        Ok(())
    }

    /// Store an ask: `pending`, or `running` already when it runs without asking (`auto`).
    fn ask(&self, thread: Id, steps: &str, why: &str, summary: &str, auto: bool, now: &str) -> rusqlite::Result<Id> {
        self.conn
            .prepare_cached(
                "INSERT INTO ask (thread_id, steps, why, summary, status, auto, created_at)
                 VALUES (?1, ?2, ?3, ?4, CASE WHEN ?5 THEN 'running' ELSE 'pending' END, ?5, ?6)",
            )?
            .execute(params![thread, steps, why, summary, auto, now])?;
        Ok(self.conn.last_insert_rowid())
    }

    fn ask_row(&self, id: Id) -> rusqlite::Result<Option<AskView>> {
        self.conn.prepare_cached(&format!("SELECT {COLUMNS} FROM ask WHERE id = ?1"))?.query_row([id], row).optional()
    }

    fn asks(&self, thread: Id, pending: bool) -> rusqlite::Result<Vec<AskView>> {
        let mut stmt = self.conn.prepare_cached(&format!("SELECT {COLUMNS} FROM ask WHERE thread_id = ?1 AND (?2 = 0 OR status = 'pending') ORDER BY id"))?;
        let rows = stmt.query_map(params![thread, pending], row)?;
        rows.collect()
    }

    /// An ask of the same thread already waiting with the same steps, and how many wait.
    fn asks_waiting(&self, thread: Id, steps: &str) -> rusqlite::Result<(Option<Id>, i64)> {
        let same = self.conn
            .prepare_cached("SELECT id FROM ask WHERE thread_id = ?1 AND status = 'pending' AND steps = ?2 ORDER BY id LIMIT 1")?
            .query_row(params![thread, steps], |r| r.get(0))
            .optional()?;
        let count = self.conn.prepare_cached("SELECT COUNT(*) FROM ask WHERE thread_id = ?1 AND status = 'pending'")?.query_row([thread], |r| r.get(0))?;
        Ok((same, count))
    }

    /// An ask of the same thread that ran the same steps without asking since `since`, and is
    /// running or done, and how many ran without asking since `hour`.
    fn asks_ran(&self, thread: Id, steps: &str, since: &str, hour: &str) -> rusqlite::Result<(Option<Id>, i64)> {
        let same = self.conn
            .prepare_cached(
                "SELECT id FROM ask WHERE thread_id = ?1 AND auto AND status IN ('running', 'approved') AND steps = ?2
                 AND created_at >= ?3 ORDER BY id DESC LIMIT 1",
            )?
            .query_row(params![thread, steps, since], |r| r.get(0))
            .optional()?;
        let count = self.conn
            .prepare_cached("SELECT COUNT(*) FROM ask WHERE thread_id = ?1 AND auto AND created_at >= ?2")?
            .query_row(params![thread, hour], |r| r.get(0))?;
        Ok((same, count))
    }

    /// Move an ask from `from` to `to`; `false` when it was not `from` any more (another answer
    /// came first).
    fn move_ask(&self, id: Id, from: &str, to: &str, outcome: Option<&str>, results: &str, now: &str) -> rusqlite::Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE ask SET status = ?3, outcome = ?4, results = ?5, resolved_at = ?6 WHERE id = ?1 AND status = ?2")?
            .execute(params![id, from, to, outcome, results, now])? > 0)
    }

    /// Answers the thread's agent has not been told.
    fn untold_asks(&self, thread: Id) -> rusqlite::Result<Vec<Answer>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, status, summary, outcome FROM ask
             WHERE thread_id = ?1 AND status IN ('approved', 'declined', 'failed') AND told IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map([thread], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect()
    }

    fn ask_told(&self, id: Id, status: &str) -> rusqlite::Result<()> {
        self.conn.prepare_cached("UPDATE ask SET told = ?2 WHERE id = ?1")?.execute(params![id, status])?;
        Ok(())
    }
}

fn not_found(id: Id) -> BusError {
    BusError::not_found("thread.ask_not_found", format!("No ask {id}"))
}

fn invalid(message: impl Into<String>) -> BusError {
    BusError::invalid("thread.invalid", message)
}

pub fn get(engine: &Engine, id: Id) -> Result<AskView, BusError> {
    with_db(engine, |db| db.ask_row(id))?.ok_or_else(|| not_found(id))
}

pub fn list(engine: &Engine, thread: Id, pending: bool) -> Result<Vec<AskView>, BusError> {
    super::get(engine, thread)?;
    with_db(engine, |db| db.asks(thread, pending))
}

/// The first of [`REFUSED_FLAGS`] that `value` sets to anything but `false`, at any depth.
fn refused_flag(value: &Value) -> Option<&'static str> {
    match value {
        Value::Object(fields) => REFUSED_FLAGS.iter().copied()
            .find(|flag| fields.get(*flag).is_some_and(|v| v != &Value::Bool(false)))
            .or_else(|| fields.values().find_map(refused_flag)),
        Value::Array(items) => items.iter().find_map(refused_flag),
        _ => None,
    }
}

/// Steps as asked, with what they leave out filled the way that keeps work: `session.close`
/// removes the checkout unless told not to, and removing one is never the assistant's to ask.
fn kept(mut steps: Vec<AskStep>) -> Vec<AskStep> {
    for step in steps.iter_mut().filter(|step| step.op == "session.close") {
        if let Some(fields) = step.payload.as_object_mut() {
            fields.entry("remove_worktree").or_insert(Value::Bool(false));
        }
    }
    steps
}

/// Steps as asked, checked: each op on [`ASKABLE`] with an object payload that sets no
/// [`REFUSED_FLAGS`], at least one and at most [`MAX_STEPS`].
pub fn checked(steps: &[AskStep]) -> Result<(), BusError> {
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(invalid(format!("An ask holds 1 to {MAX_STEPS} steps")));
    }
    for (at, step) in steps.iter().enumerate() {
        let n = at + 1;
        if !ASKABLE.contains(&step.op.as_str()) {
            return Err(BusError::refused("thread.ask_op", format!("Step {n}: {} is not something the assistant may ask for", step.op))
                .with_hint(format!("an ask runs one of {}; the rest the person does themselves", ASKABLE.join(", "))));
        }
        if !step.payload.is_object() {
            return Err(invalid(format!("Step {n}: the payload is an object, the op's own")));
        }
        if let Some(flag) = refused_flag(&step.payload) {
            return Err(BusError::refused("thread.ask_flag", format!("Step {n}: an ask may not set {flag}"))
                .with_hint("deleting work, and widening what a session may do or what it bills, stay the person's"));
        }
    }
    Ok(())
}

/// A step in words, for the summary and the agent: the op and what it names.
fn step_words(step: &AskStep) -> String {
    let p = &step.payload;
    let text = |key: &str| p.get(key).and_then(Value::as_str).map(|s| cut(s, 60));
    let id = |key: &str| p.get(key).and_then(Value::as_i64);
    let session = text("session").unwrap_or_else(|| "the new session".into());
    match step.op.as_str() {
        "session.create" => {
            let provider = text("provider").unwrap_or_else(|| "an agent".into());
            let role = text("role").unwrap_or_else(|| "builder".into());
            let project = id("project_id").map(|p| format!(" in project {p}")).unwrap_or_default();
            format!("create a {provider} {role}{project}")
        }
        "session.spawn" => format!("start {session}"),
        "session.resume" => format!("resume {session}"),
        "session.park" => format!("park {session}"),
        "session.wake" => format!("wake {session}"),
        "session.close" => format!("close {session}"),
        "session.input" => format!("type \u{201c}{}\u{201d} into {session}", text("data").unwrap_or_default().trim_end()),
        "session.update" => format!("change {session}"),
        "task.create" => format!("create the task \u{201c}{}\u{201d}", text("title").unwrap_or_default()),
        "task.update" => format!("edit task #{}", id("task_id").unwrap_or_default()),
        "task.move" => format!("move task #{} to {}", id("task_id").unwrap_or_default(), text("column").unwrap_or_default()),
        "task.dispatch" => format!("dispatch task #{}", id("task_id").unwrap_or_default()),
        "task.delete" => format!("delete task #{}", id("task_id").unwrap_or_else(|| id("id").unwrap_or_default())),
        "notes.create" => format!("create the note \u{201c}{}\u{201d}", text("title").unwrap_or_default()),
        "notes.update" => format!("edit note {}", id("note_id").unwrap_or_default()),
        "workspace.create" => format!("add the workspace {}", text("path").or_else(|| text("name")).unwrap_or_default()),
        "project.add" => format!("add the project {}", text("path").unwrap_or_default()),
        op => op.to_string(),
    }
}

/// What approving does, in words: its steps, one after the other.
pub fn summary(steps: &[AskStep]) -> String {
    let words: Vec<String> = steps.iter().map(step_words).collect();
    let mut out = words.join(", then ");
    if let Some(first) = out.get(..1) {
        out = first.to_uppercase() + &out[1..];
    }
    out
}

/// `now` less `minutes`, in the stored format; `now` itself when it does not parse.
fn before(now: &str, minutes: i64) -> String {
    now.parse::<jiff::Timestamp>()
        .ok()
        .and_then(|t| t.checked_sub(jiff::SignedDuration::from_mins(minutes)).ok())
        .map_or_else(|| now.to_string(), crate::time::stamp)
}

/// Store an ask from thread `thread`'s agent, and whether it is new and runs now. The same steps
/// already waiting are that ask. When `allowed` lets every step through it is stored `running`,
/// for [`carry_out`] to run at once, unless the same steps ran without asking in the last
/// [`AUTO_SAME_MINUTES`] (that ask is the answer, and nothing runs again) or
/// [`MAX_AUTO_PER_HOUR`] already did this hour (it waits on a card).
pub fn ask(engine: &Engine, thread: Id, steps: Vec<AskStep>, why: &str, allowed: &Allowed, now: &str) -> Result<(AskView, bool), BusError> {
    if super::get(engine, thread)?.kind != super::ASSISTANT {
        return Err(BusError::refused("thread.not_assistant", "Only the assistant asks this way; a Threads conversation proposes (thread.propose)"));
    }
    let why = why.trim();
    if why.is_empty() || why.chars().count() > MAX_WHY {
        return Err(invalid(format!("Say why in 1 to {MAX_WHY} characters")));
    }
    let steps = kept(steps);
    checked(&steps)?;
    let stored = serde_json::to_string(&steps).map_err(|e| BusError::internal(e.to_string()))?;
    let summary = summary(&steps);
    let auto = allowed.lets_all(&steps);
    let (id, runs) = with_db(engine, |db| {
        if auto {
            let (same, ran) = db.asks_ran(thread, &stored, &before(now, AUTO_SAME_MINUTES), &before(now, 60))?;
            if let Some(same) = same {
                return Ok(Ok((same, false)));
            }
            if ran < MAX_AUTO_PER_HOUR {
                return db.ask(thread, &stored, why, &summary, true, now).map(|id| Ok((id, true)));
            }
        }
        let (same, waiting) = db.asks_waiting(thread, &stored)?;
        if let Some(same) = same {
            return Ok(Ok((same, false)));
        }
        if waiting >= MAX_PENDING {
            return Ok(Err(BusError::conflict("thread.ask_full", format!("{MAX_PENDING} asks already wait in this thread"))
                .with_hint("wait for the person to answer them")));
        }
        db.ask(thread, &stored, why, &summary, false, now).map(|id| Ok((id, false)))
    })??;
    Ok((get(engine, id)?, runs))
}

/// Answer ask `id`: claim it, so a second answer is refused. Declining is then done; an approval
/// is `running`, and its steps run in [`carry_out`] once the store's lock is released.
pub fn answer(engine: &Engine, id: Id, approve: bool, now: &str) -> Result<AskView, BusError> {
    get(engine, id)?;
    let (to, outcome) = if approve { ("running", None) } else { ("declined", Some("Not done.")) };
    if !with_db(engine, |db| db.move_ask(id, "pending", to, outcome, "[]", now))? {
        return Err(BusError::conflict("thread.ask_answered", format!("Ask {id} was already answered ({})", get(engine, id)?.status)));
    }
    get(engine, id)
}

/// Run approved ask `id`'s steps as the person, each its own request, and keep what came of it.
/// Called after the approval's transaction, with the store's lock released. One that ran without
/// asking is answered in its own reply, so the agent is not told it again.
pub fn carry_out(engine: &Engine, id: Id) -> Result<AskView, BusError> {
    let asked = get(engine, id)?;
    if asked.status != "running" {
        return Ok(asked);
    }
    let (status, outcome, results) = run(engine, &asked.steps);
    let results = Value::Array(results).to_string();
    with_db(engine, |db| db.move_ask(id, "running", status, Some(&outcome), &results, &crate::time::now()))?;
    if asked.auto {
        told(engine, &[(id, status.to_string())]);
    }
    get(engine, id)
}

/// Run `steps` in order as the person: the status, the outcome in words, and each result.
fn run(engine: &Engine, steps: &[AskStep]) -> (&'static str, String, Vec<Value>) {
    let mut results = Vec::new();
    let mut made: Option<String> = None;
    let mut done = Vec::new();
    for (at, step) in steps.iter().enumerate() {
        let payload = filled(step, &made);
        let answer = engine.dispatch(Request::new(Actor::User, step.op.clone(), payload), crate::engine::Door::InProcess).into_result();
        match answer {
            Ok(result) => {
                if step.op == "session.create" {
                    made = result["name"].as_str().map(str::to_string);
                }
                done.push(step_words(&AskStep { op: step.op.clone(), payload: result_named(&step.payload, &made) }));
                results.push(result);
            }
            Err(error) => {
                let ran = if done.is_empty() { String::new() } else { format!(" Done before it: {}.", done.join(", ")) };
                return ("failed", format!("Step {} ({}) failed: {}.{ran}", at + 1, step.op, error.message.trim_end_matches('.')), results);
            }
        }
    }
    let mut outcome = done.join(", then ");
    if let Some(first) = outcome.get(..1) {
        outcome = first.to_uppercase() + &outcome[1..];
    }
    ("approved", format!("{outcome}: done."), results)
}

/// `step`'s payload to run: a `session.*` step that names no session acts on the one the last
/// `session.create` made. A create takes no `session`, and a second one makes its own.
fn filled(step: &AskStep, made: &Option<String>) -> Value {
    let mut payload = step.payload.clone();
    if let (Some(name), Some(fields)) = (made, payload.as_object_mut()) {
        if step.op.starts_with("session.") && step.op != "session.create" && !fields.contains_key("session") {
            fields.insert("session".into(), json!(name));
        }
    }
    payload
}

/// `payload` with the session a `session.create` made named in it, for the words of what ran.
fn result_named(payload: &Value, made: &Option<String>) -> Value {
    let mut payload = payload.clone();
    if let (Some(name), Some(fields)) = (made, payload.as_object_mut()) {
        fields.entry("session").or_insert_with(|| json!(name));
    }
    payload
}

/// The answers thread `thread`'s agent has not heard, one line each, and which they are.
pub(super) fn heard(engine: &Engine, thread: Id) -> (Vec<String>, Vec<(Id, String)>) {
    let Ok(untold) = with_db(engine, |db| db.untold_asks(thread)) else { return (Vec::new(), Vec::new()) };
    let lines = untold.iter().map(|(id, status, summary, outcome)| {
        let summary = cut(summary, 240);
        match status.as_str() {
            "approved" => format!("[Relay: the person approved ask {id} ({summary}); it is done.]"),
            "declined" => format!("[Relay: the person declined ask {id} ({summary}); nothing was done.]"),
            _ => format!("[Relay: ask {id} ({summary}) was approved but failed: {}]", outcome.as_deref().unwrap_or("no reason given")),
        }
    }).collect();
    (lines, untold.into_iter().map(|(id, status, _, _)| (id, status)).collect())
}

/// The agent was given these answers.
pub(super) fn told(engine: &Engine, answers: &[(Id, String)]) {
    for (id, status) in answers {
        if let Err(e) = with_db(engine, |db| db.ask_told(*id, status)) {
            tracing::warn!(ask = id, "marking an ask's answer told failed: {}", e.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(op: &str, payload: Value) -> AskStep {
        AskStep { op: op.into(), payload }
    }

    #[test]
    fn only_askable_ops_without_destructive_flags_pass() {
        assert!(checked(&[step("session.park", json!({"session": "calm-otter"}))]).is_ok());
        assert!(checked(&[step("session.close", json!({"session": "calm-otter", "discard_changes": false}))]).is_ok());
        assert_eq!(checked(&[]).unwrap_err().code, "thread.invalid");
        assert_eq!(checked(&vec![step("session.park", json!({"session": "a"})); 6]).unwrap_err().code, "thread.invalid");
        for op in ["guardrail.confirm", "settings.set", "project.remove", "workspace.remove", "git.commit", "file.write", "thread.ask", "audit.undo"] {
            assert_eq!(checked(&[step(op, json!({}))]).unwrap_err().code, "thread.ask_op", "{op}");
        }
        for flag in REFUSED_FLAGS {
            let refused = checked(&[step("session.close", json!({"session": "a", *flag: true}))]).unwrap_err();
            assert_eq!(refused.code, "thread.ask_flag", "{flag}");
        }
        assert_eq!(checked(&[step("session.park", json!("calm-otter"))]).unwrap_err().code, "thread.invalid");
    }

    /// `task.dispatch` takes a whole `session.create` in `create`, and runs it as written.
    #[test]
    fn a_flag_nested_in_a_payload_is_refused_too() {
        for flag in ["bus_writes", "allow_ui", "use_api_key"] {
            let dispatch = step("task.dispatch", json!({"task_id": 12, "fanout": true, "create": {"project_id": 1, "provider": "claude", flag: true}}));
            assert_eq!(checked(&[dispatch]).unwrap_err().code, "thread.ask_flag", "{flag}");
        }
        let listed = step("task.dispatch", json!({"task_id": 1, "steps": [{"force": "yes"}]}));
        assert_eq!(checked(&[listed]).unwrap_err().code, "thread.ask_flag");
        assert!(checked(&[step("task.dispatch", json!({"task_id": 12, "create": {"project_id": 1, "provider": "claude", "bus_writes": false}}))]).is_ok());
    }

    /// `session.close` removes the checkout unless told not to; an ask never does.
    #[test]
    fn a_close_keeps_its_checkout_unless_the_ask_says_otherwise() {
        let steps = kept(vec![
            step("session.close", json!({"session": "calm-otter"})),
            step("session.close", json!({"session": "brisk-otter", "remove_worktree": true})),
            step("session.park", json!({"session": "calm-otter"})),
        ]);
        assert_eq!(steps[0].payload, json!({"session": "calm-otter", "remove_worktree": false}));
        assert_eq!(checked(&steps[1..2]).unwrap_err().code, "thread.ask_flag", "asking to remove it is still refused");
        assert_eq!(steps[2].payload, json!({"session": "calm-otter"}));
    }

    /// "Launch a builder and a reviewer": each start acts on the session the create before it made,
    /// and the second create makes its own instead of being handed the first one's name.
    #[test]
    fn a_step_acts_on_the_session_made_before_it_and_a_create_on_none() {
        let made = Some("calm-otter".to_string());
        assert_eq!(filled(&step("session.spawn", json!({"prompt": "Go"})), &made), json!({"prompt": "Go", "session": "calm-otter"}));
        assert_eq!(filled(&step("session.spawn", json!({"session": "brisk-otter"})), &made), json!({"session": "brisk-otter"}));
        assert_eq!(filled(&step("session.create", json!({"project_id": 1, "provider": "claude"})), &made), json!({"project_id": 1, "provider": "claude"}));
        assert_eq!(filled(&step("task.create", json!({"title": "T"})), &made), json!({"title": "T"}));
        assert_eq!(filled(&step("session.spawn", json!({})), &None), json!({}));
    }

    /// The person's settings let whole kinds through; anything of no kind, or of a kind left
    /// off, waits on a card, and a setting that is not `true` lets nothing through.
    #[test]
    fn only_the_kinds_the_person_allows_run_without_asking() {
        let launch = [step("session.create", json!({"project_id": 1, "provider": "claude"})), step("session.spawn", json!({"prompt": "Go"}))];
        let typing = [step("session.input", json!({"session": "calm-otter", "data": "yes\r"}))];
        let allowed = Allowed::from_setting(&json!({"launch": true, "type": false, "stop": "yes"}));
        assert!(allowed.lets_all(&launch));
        assert!(!allowed.lets_all(&typing), "typing can answer a session's permission prompt");
        assert!(!allowed.lets_all(&[step("session.park", json!({"session": "calm-otter"}))]), "only true lets a kind through");
        assert!(!allowed.lets_all(&[launch[0].clone(), step("task.delete", json!({"task_id": 3}))]), "one step of no kind holds the whole ask");
        assert!(!Allowed::from_setting(&Value::Null).lets_all(&launch));
        let every = Allowed::from_setting(&json!({"launch": true, "type": true, "stop": true}));
        for op in ["task.delete", "workspace.create", "project.add"] {
            assert!(!every.lets_all(&[step(op, json!({}))]), "{op} always asks");
        }
        for (_, ops) in KINDS {
            for op in *ops {
                assert!(ASKABLE.contains(op), "{op} is askable");
            }
        }
    }

    /// A launch into the person's checkout, a peer's, or beside a peer waits on a card even with
    /// launching let through, at any depth; a new checkout, said or left out, does not.
    #[test]
    fn a_launch_outside_a_new_checkout_of_its_own_waits_on_a_card() {
        let allowed = Allowed::from_setting(&json!({"launch": true}));
        let create = |extra: Value| {
            let mut payload = json!({"project_id": 1, "provider": "claude"});
            payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            payload
        };
        for fine in [json!({}), json!({"worktree": "new"}), json!({"worktree": null, "pair_with": null})] {
            assert!(allowed.lets_all(&[step("session.create", create(fine.clone()))]), "{fine}");
        }
        for reaching in [json!({"worktree": "primary"}), json!({"worktree": "/home/me/repo/.relay/worktrees/calm-otter"}), json!({"pair_with": "calm-otter"})] {
            assert!(!allowed.lets_all(&[step("session.create", create(reaching.clone()))]), "{reaching}");
            let dispatch = step("task.dispatch", json!({"task_id": 12, "create": create(reaching.clone())}));
            assert!(!allowed.lets_all(&[dispatch]), "nested {reaching}");
        }
    }

    #[test]
    fn before_counts_back_in_the_stored_format() {
        assert_eq!(before("2026-10-28T12:05:00.000000000Z", 10), "2026-10-28T11:55:00.000000000Z");
        assert_eq!(before("not a time", 10), "not a time");
    }

    #[test]
    fn a_summary_reads_its_steps_in_order() {
        let steps = [
            step("session.create", json!({"project_id": 1, "provider": "claude"})),
            step("session.spawn", json!({"prompt": "Fix the build"})),
        ];
        assert_eq!(summary(&steps), "Create a claude builder in project 1, then start the new session");
        assert_eq!(summary(&[step("session.input", json!({"session": "calm-otter", "data": "yes\r"}))]), "Type \u{201c}yes\u{201d} into calm-otter");
    }
}
