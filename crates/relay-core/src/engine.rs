//! The engine: op handlers + the request pipeline (BUS.md §5).
//!
//! ```text
//! receive → parse → idempotency → actor → validate → authorize → policy → HANDLER → audit → events → respond
//! ```
//!
//! Handlers are synchronous and run inside one SQLite transaction with the store mutex held —
//! writes are serial by design (§5.1). Doors call [`Engine::dispatch`] from `spawn_blocking`,
//! except for the requests [`Engine::answers_from_memory`] admits, which run inline on the
//! door's own thread.

use crate::audit::{self, AuditEntry};
use crate::device::{DeviceWatchState, MirrorRuntime, RunRuntime};
use crate::paths::Instance;
use crate::pty::Pty;
use crate::store::Store;
use relay_bus::envelope::{Actor, Event, MailHint, Request, Response, ENVELOPE_V};
use relay_bus::error::BusError;
use relay_bus::registry::{Audit, Op, OpEntry, OpKind, Registry};
use relay_bus::types::{Id, Page, PaneInfo, PaneRef, UndoOp, WindowInfo};
use rusqlite::Transaction;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Notify};
use uuid::Uuid;

/// Which door a request came through (BUS.md §6). Rules differ per door (§4.2). The phone door
/// (`relay-remote`) is not a door here: it forwards its lines to the socket as `user`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    Socket,
    /// The in-process test harness: trusted, no token checks.
    InProcess,
}

#[derive(Debug)]
pub(crate) struct UiRuntime {
    pub project_id: Option<Id>,
    pub page: Page,
    pub panes: Vec<PaneInfo>,
    pub focused: Option<PaneRef>,
    pub windows: Vec<WindowInfo>,
    pub next_pane: u64,
    pub next_window: u64,
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self {
            project_id: None,
            page: Page::Dashboard,
            panes: Vec::new(),
            focused: None,
            windows: vec![WindowInfo {
                window_id: "main".into(),
                main: true,
                panes: Vec::new(),
            }],
            next_pane: 1,
            next_window: 1,
        }
    }
}

/// What a handler sees. One per request; dropped after the response is built.
/// Anything over this and the request was, for that long, the reason nothing else could run —
/// one frame at 60 Hz. It is a reporting threshold, not a limit: the pipeline never interrupts a
/// handler, it just says which op did it (D144).
const LOCK_BUDGET: std::time::Duration = std::time::Duration::from_millis(16);

/// The largest `session.input` a door may answer without leaving its own thread. Comfortably
/// above any keystroke and below a terminal's input queue, so the write cannot park there.
const INLINE_INPUT_BYTES: usize = 1024;

/// Store-lock accounting for one request. `wait` is how long it queued behind other handlers;
/// `held` is how long it kept every other op — keystrokes included — out of the connection.
/// Reports on `finish`, or on drop for the paths that leave the lock to scope exit.
struct LockSpan {
    op: &'static str,
    queued: Instant,
    wait: std::time::Duration,
    acquired: Instant,
    reported: bool,
}

impl LockSpan {
    fn opening(op: &'static str) -> LockSpan {
        LockSpan {
            op,
            queued: Instant::now(),
            wait: std::time::Duration::ZERO,
            acquired: Instant::now(),
            reported: false,
        }
    }
    /// Call the instant the lock is in hand.
    fn acquired(&mut self) {
        self.wait = self.queued.elapsed();
        self.acquired = Instant::now();
    }
    fn finish(mut self) {
        self.report();
    }
    fn report(&mut self) {
        if self.reported {
            return;
        }
        self.reported = true;
        let held = self.acquired.elapsed();
        if held >= LOCK_BUDGET || self.wait >= LOCK_BUDGET {
            tracing::warn!(
                op = self.op,
                wait_ms = self.wait.as_secs_f64() * 1000.0,
                held_ms = held.as_secs_f64() * 1000.0,
                "store lock over budget"
            );
        } else {
            tracing::debug!(
                op = self.op,
                wait_ms = self.wait.as_secs_f64() * 1000.0,
                held_ms = held.as_secs_f64() * 1000.0,
                "store lock"
            );
        }
    }
}

impl Drop for LockSpan {
    fn drop(&mut self) {
        self.report();
    }
}

/// The context for an op that must **not** hold the store lock while it works (D144, D149): anything
/// that shells out, walks a tree, or talks to the network. There is no open transaction. The
/// handler takes the connection itself, in short bursts, through [`Unlocked::read`], and does the
/// slow part with nothing locked.
///
/// Only queries register this way, so there is no audit row, no undo envelope and no transaction
/// to fail — which is exactly why the split is safe to make without touching the write pipeline.
pub struct Unlocked<'a> {
    engine: &'a Engine,
    /// Set only when this phase is running nested inside a transaction that already holds the
    /// store (`guardrail.confirm` replaying a held op). Then [`Unlocked::read`] uses that
    /// connection instead of taking the lock again — a `std::sync::Mutex` is not reentrant, and
    /// the nested case is rare enough to be worth exactly the cost it had before the split.
    held: Option<&'a rusqlite::Connection>,
    pub actor: Actor,
    pub req_id: Uuid,
    pub now: String,
    pub op: &'static str,
    session_id: Option<Id>,
    project_id: Option<Id>,
    events: Vec<Event>,
    deferred: Vec<Box<dyn FnOnce(Arc<Engine>) + Send + 'static>>,
}

/// A staged op's read/external phase, already run, waiting for its transaction: what
/// [`Unlocked::prepare_registered`] hands to [`Ctx::replay_prepared`].
pub(crate) struct Prepared {
    op: &'static str,
    staged: Box<dyn Any + Send>,
    events: Vec<Event>,
    deferred: Vec<Box<dyn FnOnce(Arc<Engine>) + Send + 'static>>,
}

impl Unlocked<'_> {
    pub fn engine(&self) -> &Engine {
        self.engine
    }
    /// Run `op`'s prepare phase now, unlocked, as `actor` and `session_id` — so a replay
    /// (`guardrail.confirm` of a held `git.commit`) does its slow work before the transaction
    /// opens instead of inside it. `Ok(None)` when `op` has no prepare phase.
    pub(crate) fn prepare_registered(
        &self,
        op: &str,
        payload: &Value,
        actor: Actor,
        session_id: Option<Id>,
    ) -> Result<Option<Prepared>, BusError> {
        let entry = Registry::global().get(op).ok_or_else(|| BusError::unknown_op(op))?;
        (entry.validate)(payload).map_err(|e| BusError::schema(entry.name, e))?;
        let Some(prepare) = self.engine.prepares.get(entry.name).cloned() else {
            return Ok(None);
        };
        let mut stage = Unlocked {
            engine: self.engine,
            held: self.held,
            actor,
            req_id: self.req_id,
            now: self.now.clone(),
            op: entry.name,
            session_id,
            project_id: None,
            events: Vec::new(),
            deferred: Vec::new(),
        };
        let staged = prepare(&mut stage, payload)?;
        Ok(Some(Prepared { op: entry.name, staged, events: stage.events, deferred: stage.deferred }))
    }
    pub fn instance(&self) -> Instance {
        self.engine.instance
    }
    /// The bound session id when the actor is an agent (BUS.md §4.2), else `None`.
    pub fn actor_session_id(&self) -> Option<Id> {
        self.session_id
    }
    /// Back the whole store up into `backups/`. Unlocked, the copy takes the store lock one
    /// batch of pages at a time ([`crate::store::Store::backup`]), so other requests run between
    /// batches instead of waiting out a copy of the whole file; nested inside a handler that
    /// already holds the lock it copies through that transaction.
    pub fn backup_store(&self, reason: &str) -> anyhow::Result<std::path::PathBuf> {
        match self.held {
            Some(conn) => self.engine.store.backup_with(conn, reason),
            None => self.engine.store.backup(reason),
        }
    }
    /// Take the store for one short read. Keep the closure to plain queries: whatever runs in
    /// here is back on the one global lock, which is the thing this context exists to avoid.
    pub fn read<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, BusError>,
    ) -> Result<T, BusError> {
        if let Some(conn) = self.held {
            return f(conn);
        }
        let mut span = LockSpan::opening(self.op);
        let conn = self.engine.store.lock();
        span.acquired();
        let out = f(&conn);
        drop(conn);
        span.finish();
        out
    }
    /// Queue an event; emitted once the handler returns.
    pub fn emit(&mut self, ev: &str, payload: Value) {
        let mut e =
            Event::new(ev, self.now.clone(), self.actor.clone(), payload).caused_by(self.req_id);
        e.project_id = self.project_id;
        self.events.push(e);
    }
    pub fn set_project(&mut self, id: Id) {
        self.project_id = Some(id);
    }
    /// Run once the handler returns. The name matches [`Ctx::after_commit`] so the same helpers
    /// serve both contexts; there is no transaction here, so "after commit" is simply "after".
    pub fn after_commit(&mut self, f: impl FnOnce(Arc<Engine>) + Send + 'static) {
        self.deferred.push(Box::new(f));
    }
}

pub struct Ctx<'a> {
    tx: &'a Transaction<'a>,
    engine: &'a Engine,
    pub actor: Actor,
    pub req_id: Uuid,
    pub now: String,
    pub op: &'static str,
    /// The request's payload. Borrowed from the request on the ordinary path — only one
    /// handler ever reads it back (`guardrail.gate`, to freeze the envelope), and copying
    /// every payload on the chance that it would was a second deep clone per request.
    payload: std::borrow::Cow<'a, Value>,
    skip_policy: Option<String>,
    events: Vec<Event>,
    undo: Option<UndoOp>,
    undo_of: Option<Id>,
    mark_undone: Option<Id>,
    project_id: Option<Id>,
    session_id: Option<Id>,
    /// A guardrail gate may need its hold/refusal and audit row committed together even
    /// though the bus response is an error (BUS.md §5.1 / §9.2).
    commit_error: bool,
    hold_id: Option<Id>,
    /// Confirmation is audited as the frozen op with the original actor in
    /// `on_behalf_of`, while retaining the confirm request id (BUS.md §9.4).
    audit_op: Option<String>,
    on_behalf_of: Option<Actor>,
    after_commit: Vec<Box<dyn FnOnce(Arc<Engine>) + Send + 'static>>,
    /// What the op's read/external phase produced, for handlers registered with
    /// [`Engine::register_staged`]. Taken once, by the matching `finish` closure.
    staged: Option<Box<dyn Any + Send>>,
}

impl<'a> Ctx<'a> {
    pub fn tx(&self) -> &'a Transaction<'a> {
        self.tx
    }
    pub fn engine(&self) -> &Engine {
        self.engine
    }
    pub fn instance(&self) -> Instance {
        self.engine.instance
    }
    /// The bound session id when the actor is an agent (BUS.md §4.2), else `None`.
    pub fn actor_session_id(&self) -> Option<Id> {
        self.session_id
    }
    pub(crate) fn payload(&self) -> &Value {
        &self.payload
    }
    pub(crate) fn skip_policy(&self) -> Option<&str> {
        self.skip_policy.as_deref()
    }
    /// Queue an event; emitted after commit, stamped with ts/actor/cause (BUS.md §1.3).
    pub fn emit(&mut self, ev: &str, payload: Value) {
        let mut e =
            Event::new(ev, self.now.clone(), self.actor.clone(), payload).caused_by(self.req_id);
        e.project_id = self.project_id;
        self.events.push(e);
    }
    /// Tag the audit row (and subsequent events) with a project.
    pub fn set_project(&mut self, id: Id) {
        self.project_id = Some(id);
    }
    pub fn set_session(&mut self, id: Id) {
        self.session_id = Some(id);
    }
    /// Commit this typed error instead of rolling the handler transaction back.
    pub fn commit_error(&mut self, hold_id: Option<Id>) {
        self.commit_error = true;
        self.hold_id = hold_id;
    }
    /// Change only the audit attribution for this request (guardrail confirmation).
    pub fn audit_as(&mut self, op: impl Into<String>, on_behalf_of: Actor) {
        self.audit_op = Some(op.into());
        self.on_behalf_of = Some(on_behalf_of);
    }
    /// Record the inverse envelope for `audit.undo` (BUS.md §5.5).
    pub fn set_undo(&mut self, op: &str, payload: Value, expect: Option<Value>) {
        self.undo = Some(UndoOp {
            op: op.to_string(),
            payload,
            expect,
        });
    }
    /// Mark this request as the inverse of an earlier audit row. The pipeline links both
    /// rows only after the undo audit row exists, keeping the foreign keys atomic.
    pub fn set_undo_of(&mut self, audit_id: Id) {
        self.undo_of = Some(audit_id);
        self.mark_undone = Some(audit_id);
    }
    /// Run after the transaction commits (side effects that must not precede state).
    pub fn after_commit(&mut self, f: impl FnOnce(Arc<Engine>) + Send + 'static) {
        self.after_commit.push(Box::new(f));
    }
    /// The value this op's read/external phase produced. Exactly one caller per request.
    fn take_staged<S: Any + Send>(&mut self) -> Result<S, BusError> {
        self.staged
            .take()
            .and_then(|value| value.downcast::<S>().ok())
            .map(|value| *value)
            .ok_or_else(|| BusError::internal(format!("{} ran without its staged phase", self.op)))
    }
    /// Call another typed handler without leaving this transaction. This is used only by
    /// composite bus operations whose contract explicitly includes the nested action.
    pub(crate) fn invoke_registered(
        &mut self,
        op: &str,
        payload: Value,
    ) -> Result<Value, BusError> {
        self.invoke_with(op, payload, None)
    }
    /// [`Ctx::invoke_registered`] with `op`'s prepare phase already run, unlocked, by
    /// [`Unlocked::prepare_registered`] — `project.remove {force}` closing its sessions.
    pub(crate) fn invoke_prepared(
        &mut self,
        op: &str,
        payload: Value,
        prepared: Option<Prepared>,
    ) -> Result<Value, BusError> {
        self.invoke_with(op, payload, prepared)
    }

    fn invoke_with(
        &mut self,
        op: &str,
        payload: Value,
        prepared: Option<Prepared>,
    ) -> Result<Value, BusError> {
        let entry = Registry::global()
            .get(op)
            .ok_or_else(|| BusError::unknown_op(op))?;
        (entry.validate)(&payload).map_err(|e| BusError::schema(entry.name, e))?;
        let handler = self
            .engine
            .handlers
            .get(entry.name)
            .ok_or_else(|| BusError::not_implemented(entry.name, entry.meta.phase))?
            .call
            .clone();
        // A staged op reached this way with nothing prepared — `project.remove {force}` closing
        // its sessions — has no read/external phase behind it, so run it here, against the
        // transaction already open: everything its prepare does then holds the store. Keep slow
        // staged ops off this path: `guardrail.confirm` prepares its replay before it opens
        // (`Unlocked::prepare_registered`), as `audit.undo` does an inverse whose prepare only
        // reads (`file.restore`), and `task.dispatch` creates and launches its sessions as
        // requests of their own.
        let staged = match (prepared, self.engine.prepares.get(entry.name).cloned()) {
            (Some(prepared), _) if prepared.op == entry.name => {
                self.events.extend(prepared.events);
                self.after_commit.extend(prepared.deferred);
                Some(prepared.staged)
            }
            (Some(prepared), _) => {
                return Err(BusError::internal(format!("{} was prepared for {}", entry.name, prepared.op)));
            }
            (None, Some(prepare)) => {
                let mut stage = Unlocked {
                    engine: self.engine,
                    held: Some(self.tx),
                    actor: self.actor.clone(),
                    req_id: self.req_id,
                    now: self.now.clone(),
                    op: entry.name,
                    session_id: self.session_id,
                    project_id: self.project_id,
                    events: Vec::new(),
                    deferred: Vec::new(),
                };
                let out = prepare(&mut stage, &payload)?;
                let Unlocked {
                    events, deferred, ..
                } = stage;
                self.events.extend(events);
                self.after_commit.extend(deferred);
                Some(out)
            }
            (None, None) => None,
        };
        let prior = self.op;
        let prior_payload =
            std::mem::replace(&mut self.payload, std::borrow::Cow::Owned(payload.clone()));
        let prior_staged = std::mem::replace(&mut self.staged, staged);
        self.op = entry.name;
        let result = handler(self, &payload);
        self.op = prior;
        self.payload = prior_payload;
        self.staged = prior_staged;
        result
    }

    /// Replay a held op as its original caller (`guardrail.confirm`), with the op's prepare
    /// phase already run, unlocked, by [`Unlocked::prepare_registered`]; `None` runs it here,
    /// inside this transaction. The session goes with the actor: a held agent `file.write`
    /// resolves its worktree, grants and any nested hold from it, and the confirmer's (none,
    /// for the user) would land it in the primary checkout.
    pub(crate) fn replay_prepared(
        &mut self,
        op: &str,
        payload: Value,
        actor: Actor,
        session_id: Option<Id>,
        skip_policy: String,
        prepared: Option<Prepared>,
    ) -> Result<Value, BusError> {
        let prior_actor = std::mem::replace(&mut self.actor, actor);
        let prior_session = std::mem::replace(&mut self.session_id, session_id);
        let prior_policy = self.skip_policy.replace(skip_policy);
        let result = self.invoke_with(op, payload, prepared);
        self.actor = prior_actor;
        self.session_id = prior_session;
        self.skip_policy = prior_policy;
        result
    }
}

/// Handlers read their payload *through* the request's copy: a `Value` passed by value had to
/// be cloned for every request, and the clone was thrown away the moment the typed payload
/// came out of it.
type HandlerFn = Arc<dyn Fn(&mut Ctx, &Value) -> Result<Value, BusError> + Send + Sync>;

struct Handler {
    call: HandlerFn,
}

type UnlockedFn = Arc<dyn Fn(&mut Unlocked, &Value) -> Result<Value, BusError> + Send + Sync>;

type PrepareFn =
    Arc<dyn Fn(&mut Unlocked, &Value) -> Result<Box<dyn Any + Send>, BusError> + Send + Sync>;

/// The engine. Build with [`Engine::new`] (registers phase-1 handlers) and share as `Arc`.
pub struct Engine {
    pub instance: Instance,
    pub store: Store,
    handlers: HashMap<&'static str, Handler>,
    /// Queries that run with no transaction open (D149). Kept in their own map so the write
    /// pipeline below stays exactly one shape.
    unlocked: HashMap<&'static str, UnlockedFn>,
    /// The read/external phase of a staged mutation, run before the transaction opens (D149).
    prepares: HashMap<&'static str, PrepareFn>,
    events_tx: broadcast::Sender<Event>,
    started: Instant,
    quit: Notify,
    quitting: AtomicBool,
    pub socket_path: std::sync::Mutex<Option<String>>,
    /// The engine's own `Arc`, for work that outlives a borrow of it (after-commit closures,
    /// the idle-edge worker). Set once, by [`Arc::new_cyclic`], before anything can read it.
    self_ref: std::sync::Weak<Engine>,
    /// Ids of audited requests between their idempotency lookup and their audit row (BUS.md
    /// §5.3). A duplicate that arrives in that window waits on `in_flight_done` and then replays
    /// the recorded response, instead of running the op a second time.
    in_flight: std::sync::Mutex<HashSet<Uuid>>,
    in_flight_done: std::sync::Condvar,
    /// Live PTYs by session id (phase 3). Not persisted: a restart makes sessions restorable.
    ptys: std::sync::Mutex<HashMap<Id, Arc<Pty>>>,
    /// Session name → id for the live PTYs above, so `session.input` can resolve a keystroke's
    /// target without a `SELECT` (D148).
    pty_ids: std::sync::Mutex<HashMap<String, Id>>,
    pub(crate) mirrors: std::sync::Mutex<HashMap<Id, Arc<MirrorRuntime>>>,
    pub(crate) device_runs: std::sync::Mutex<HashMap<Id, Arc<RunRuntime>>>,
    pub(crate) device_watch: std::sync::Mutex<DeviceWatchState>,
    pub(crate) next_mirror: AtomicI64,
    pub(crate) watchers: std::sync::Mutex<HashMap<String, crate::watch::Root>>,
    pub(crate) provider_updates: std::sync::Mutex<HashSet<String>>,
    pub(crate) creating_sessions: std::sync::Mutex<HashSet<String>>,
    pub(crate) watcher_registrations: std::sync::Mutex<HashSet<String>>,
    pub(crate) ui: std::sync::Mutex<UiRuntime>,
    pub(crate) resource_watch: AtomicBool,
    pub(crate) resource_watch_clients: std::sync::Mutex<usize>,
    pub(crate) resource_watch_epoch: AtomicI64,
    pub(crate) resource_cpu: std::sync::Mutex<HashMap<i64, (u64, Instant)>>,
    pub(crate) resource_disk: std::sync::Mutex<HashMap<String, (f64, Option<f64>)>>,
    pub(crate) resource_disk_refresh: std::sync::Mutex<(bool, Option<Instant>)>,
    /// Who holds which Android device (`device_lease`). Its own lock; never the store's.
    pub(crate) device_leases: crate::device_lease::Leases,
    /// Serializes skill-folder refreshes (`skills::refresh_all`).
    pub(crate) skill_refresh: crate::skills::Refresher,
    /// The Money space's ledger (`money.db` beside the store), opened on first use.
    pub(crate) money: std::sync::Mutex<Option<relay_money::ledger::Ledger>>,
    /// Threads (`threads.db` beside the store, opened on first use) and their running agents.
    pub(crate) threads: crate::threads::Hub,
    /// The local model server Threads may run on (`local_models`), when one runs.
    pub(crate) local: crate::local_models::Local,
    /// Arbiter's desk: its book (`arbiter.db`, opened on first use), the exchange and the lock
    /// orders are placed under.
    pub(crate) arbiter: crate::arbiter::Desk,
    /// Avex's training history (`gym.db`, opened on first use) and its loaded copy.
    pub(crate) gym: crate::gym::Shelf,
}

impl Engine {
    pub fn new(instance: Instance, store: Store) -> Arc<Engine> {
        let (events_tx, _) = broadcast::channel(4096);
        Arc::new_cyclic(|self_ref| {
            let mut engine = Engine {
                instance,
                store,
                handlers: HashMap::new(),
                unlocked: HashMap::new(),
                prepares: HashMap::new(),
                events_tx,
                started: Instant::now(),
                quit: Notify::new(),
                quitting: AtomicBool::new(false),
                socket_path: std::sync::Mutex::new(None),
                self_ref: self_ref.clone(),
                in_flight: std::sync::Mutex::new(HashSet::new()),
                in_flight_done: std::sync::Condvar::new(),
                ptys: std::sync::Mutex::new(HashMap::new()),
                pty_ids: std::sync::Mutex::new(HashMap::new()),
                mirrors: std::sync::Mutex::new(HashMap::new()),
                device_runs: std::sync::Mutex::new(HashMap::new()),
                device_watch: std::sync::Mutex::new(DeviceWatchState::default()),
                next_mirror: AtomicI64::new(1),
                watchers: std::sync::Mutex::new(HashMap::new()),
                provider_updates: std::sync::Mutex::new(HashSet::new()),
                creating_sessions: std::sync::Mutex::new(HashSet::new()),
                watcher_registrations: std::sync::Mutex::new(HashSet::new()),
                ui: std::sync::Mutex::new(UiRuntime::default()),
                resource_watch: AtomicBool::new(false),
                resource_watch_clients: std::sync::Mutex::new(0),
                resource_watch_epoch: AtomicI64::new(0),
                resource_cpu: std::sync::Mutex::new(HashMap::new()),
                resource_disk: std::sync::Mutex::new(HashMap::new()),
                resource_disk_refresh: std::sync::Mutex::new((false, None)),
                device_leases: Default::default(),
                skill_refresh: Default::default(),
                money: Default::default(),
                threads: Default::default(),
                local: Default::default(),
                arbiter: Default::default(),
                gym: Default::default(),
            };
            crate::handlers::register_all(&mut engine);
            engine
        })
    }

    /// Register a typed handler for `O`. Payload validation (BUS.md §5.2) is the typed
    /// deserialization with `deny_unknown_fields`.
    pub fn register<O: Op>(
        &mut self,
        f: impl Fn(&mut Ctx, O::Payload) -> Result<O::Result, BusError> + Send + Sync + 'static,
    ) {
        debug_assert!(
            Registry::global().get(O::NAME).is_some(),
            "{} is not in the registry",
            O::NAME
        );
        let call: HandlerFn = Arc::new(move |ctx: &mut Ctx, v: &Value| {
            let p: O::Payload =
                serde::Deserialize::deserialize(v).map_err(|e| BusError::schema(O::NAME, e))?;
            let r = f(ctx, p)?;
            serde_json::to_value(r)
                .map_err(|e| BusError::internal(format!("serializing {} result: {e}", O::NAME)))
        });
        self.handlers.insert(O::NAME, Handler { call });
    }

    /// Register `O` as an op that runs with no transaction open. Restricted to queries: an
    /// unlocked handler has no transaction to write into and no audit row to append (D149).
    pub fn register_unlocked<O: Op>(
        &mut self,
        f: impl Fn(&mut Unlocked, O::Payload) -> Result<O::Result, BusError> + Send + Sync + 'static,
    ) {
        debug_assert!(
            Registry::global().get(O::NAME).is_some(),
            "{} is not in the registry",
            O::NAME
        );
        debug_assert!(
            matches!(O::META.kind, OpKind::Query),
            "{} mutates; it cannot run outside the transaction",
            O::NAME
        );
        let call: UnlockedFn = Arc::new(move |ctx: &mut Unlocked, v: &Value| {
            let p: O::Payload =
                serde::Deserialize::deserialize(v).map_err(|e| BusError::schema(O::NAME, e))?;
            let r = f(ctx, p)?;
            serde_json::to_value(r)
                .map_err(|e| BusError::internal(format!("serializing {} result: {e}", O::NAME)))
        });
        self.unlocked.insert(O::NAME, call);
    }

    /// Register a mutation as three phases instead of one (D149): `prepare` reads what it needs
    /// in short bursts and does the slow external work — a subprocess, a network call, a tree
    /// walk — with the store lock released; `finish` then runs inside the request's transaction
    /// with what `prepare` produced, and is expected to be short.
    ///
    /// Events and deferred work queued during `prepare` are carried into the request, so they
    /// still fire after the commit, in order.
    pub fn register_staged<O: Op, S: Any + Send>(
        &mut self,
        prepare: impl Fn(&mut Unlocked, &O::Payload) -> Result<S, BusError> + Send + Sync + 'static,
        finish: impl Fn(&mut Ctx, O::Payload, S) -> Result<O::Result, BusError> + Send + Sync + 'static,
    ) {
        debug_assert!(
            Registry::global().get(O::NAME).is_some(),
            "{} is not in the registry",
            O::NAME
        );
        let stage: PrepareFn = Arc::new(move |ctx: &mut Unlocked, v: &Value| {
            let p: O::Payload =
                serde::Deserialize::deserialize(v).map_err(|e| BusError::schema(O::NAME, e))?;
            Ok(Box::new(prepare(ctx, &p)?) as Box<dyn Any + Send>)
        });
        self.prepares.insert(O::NAME, stage);
        let call: HandlerFn = Arc::new(move |ctx: &mut Ctx, v: &Value| {
            let p: O::Payload =
                serde::Deserialize::deserialize(v).map_err(|e| BusError::schema(O::NAME, e))?;
            let staged = ctx.take_staged::<S>()?;
            let r = finish(ctx, p, staged)?;
            serde_json::to_value(r)
                .map_err(|e| BusError::internal(format!("serializing {} result: {e}", O::NAME)))
        });
        self.handlers.insert(O::NAME, Handler { call });
    }

    /// Whether `op` is a query registered with [`Engine::register_unlocked`]: it takes the
    /// store only for short reads, so a door may run several at once on one connection.
    pub fn runs_unlocked(&self, op: &str) -> bool {
        self.unlocked.contains_key(op)
    }
    pub fn is_implemented(&self, op: &str) -> bool {
        self.handlers.contains_key(op) || self.unlocked.contains_key(op)
    }
    pub fn uptime_s(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }
    /// Emit a system-originated event (reconcile, watchers). Never fails.
    pub fn emit_system(&self, ev: &str, payload: Value) {
        let e = Event::new(ev, crate::time::now(), Actor::System, payload);
        let _ = self.events_tx.send(e);
    }
    pub fn request_quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.quit.notify_waiters();
        self.quit.notify_one();
    }
    pub async fn wait_quit(&self) {
        if self.quitting.load(Ordering::SeqCst) {
            return;
        }
        self.quit.notified().await;
    }
    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
    pub fn arc(&self) -> Option<Arc<Engine>> {
        self.self_ref.upgrade()
    }

    // ------------------------------------------------------------------ PTY registry

    pub fn pty(&self, session_id: Id) -> Option<Arc<Pty>> {
        self.ptys.lock().unwrap().get(&session_id).cloned()
    }
    /// The live PTY for a session name, if it has one. `None` also means "not in the map" —
    /// callers that need to tell an unknown session from an unspawned one must ask the store.
    pub fn pty_named(&self, name: &str) -> Option<(Id, Arc<Pty>)> {
        let id = *self.pty_ids.lock().unwrap().get(name)?;
        self.ptys
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .map(|pty| (id, pty))
    }
    pub fn set_pty(&self, session_id: Id, name: &str, pty: Arc<Pty>) -> Option<Arc<Pty>> {
        self.pty_ids
            .lock()
            .unwrap()
            .insert(name.to_string(), session_id);
        self.ptys.lock().unwrap().insert(session_id, pty)
    }
    pub fn take_pty(&self, session_id: Id) -> Option<Arc<Pty>> {
        self.pty_ids
            .lock()
            .unwrap()
            .retain(|_, id| *id != session_id);
        self.ptys.lock().unwrap().remove(&session_id)
    }
    pub fn live_pty_count(&self) -> usize {
        self.ptys
            .lock()
            .unwrap()
            .values()
            .filter(|p| !p.exited())
            .count()
    }
    /// Each live session's PTY child pid, with the session's name: the processes whose trees
    /// may not act as the user on the socket (RA-096). An exited child's pid may already be
    /// someone else's, so it is left out.
    pub fn session_pids(&self) -> HashMap<u32, String> {
        let names: Vec<(String, Id)> = self.pty_ids.lock().unwrap().iter().map(|(name, id)| (name.clone(), *id)).collect();
        let ptys = self.ptys.lock().unwrap();
        names
            .into_iter()
            .filter_map(|(name, id)| ptys.get(&id).filter(|pty| !pty.exited()).map(|pty| (pty.pid(), name)))
            .collect()
    }
    /// Kill every child (SPEC §14 persistence model: after a restart PTYs are dead and the
    /// resume flow takes over) and mark their sessions restorable. Idempotent.
    pub fn shutdown(&self) {
        crate::handlers::integration::shutdown(self);
        crate::local_models::stop(self);
        for (_, mirror) in self.mirrors.lock().unwrap().drain() {
            mirror.stop();
        }
        let runs: Vec<_> = self.device_runs.lock().unwrap().drain().collect();
        for (_, run) in &runs {
            run.stop();
        }
        if !runs.is_empty() {
            let now = crate::time::now();
            let conn = self.store.lock();
            let _ = conn.execute(
                "UPDATE device_runs SET state='stopped',finished_at=COALESCE(finished_at,?1) WHERE state IN ('building','running')",
                [now],
            );
        }
        self.pty_ids.lock().unwrap().clear();
        let ptys: Vec<(Id, Arc<Pty>)> = self.ptys.lock().unwrap().drain().collect();
        if ptys.is_empty() {
            return;
        }
        // mark first, then kill: the exit callback only touches live states, so it stays a no-op
        self.mark_restorable(&ptys);
        // All at once, not in turn: each kill waits up to its grace for the child to leave, and
        // ten slow CLIs one after another outlasted a logout's stop timeout. One TERM to every
        // group and one shared wait cost about one grace however many sessions are open.
        let all: Vec<&Pty> = ptys.iter().map(|(_, p)| &**p).collect();
        crate::pty::kill_all(&all, std::time::Duration::from_secs(2));
    }

    fn mark_restorable(&self, ptys: &[(Id, Arc<Pty>)]) {
        let now = crate::time::now();
        let mut conn = self.store.lock();
        let tx = match conn.transaction() {
            Ok(tx) => tx,
            Err(_) => return,
        };
        for (id, pty) in ptys {
            let (text, epoch, seq) = pty.scrollback(None);
            let _ = tx.execute(
                "INSERT INTO session_scrollback(session_id,text,epoch,seq,updated_at) VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(session_id) DO UPDATE SET text=excluded.text,epoch=excluded.epoch,seq=excluded.seq,updated_at=excluded.updated_at",
                rusqlite::params![id, text.as_bytes(), epoch as i64, seq as i64, now],
            );
            let _ = tx.execute(
                "UPDATE sessions SET state='restorable',pid=NULL,restore_reason='app_restart',updated_at=?1
                 WHERE id=?2 AND state IN ('spawning','running','idle','blocked')",
                rusqlite::params![now, id],
            );
        }
        let _ = tx.commit();
    }

    /// A system-originated mutation outside any request (process exit, watcher, recovery):
    /// runs `f` in a transaction, appends an audit row as `system` (`op`, `parent_req` per
    /// BUS.md §5.1) and emits the events `f` returns. Never called with the store lock held.
    pub fn system_write<T>(
        &self,
        op: &str,
        parent_req: Option<Uuid>,
        project_id: Option<Id>,
        session_id: Option<Id>,
        summary: Value,
        f: impl FnOnce(&Transaction, &str) -> Result<(T, Vec<(String, Value)>), BusError>,
    ) -> Result<T, BusError> {
        let now = crate::time::now();
        let mut conn = self.store.lock();
        let tx = conn.transaction().map_err(internal)?;
        let (out, events) = f(&tx, &now)?;
        let req_id = Uuid::new_v4();
        let e = AuditEntry {
            ts: &now,
            req_id,
            parent_req,
            actor: &Actor::System,
            on_behalf_of: None,
            session_id,
            op,
            project_id,
            payload: &Value::Object(Default::default()),
            outcome: &Ok(summary),
            hold_id: None,
            undo_op: None,
            undo_of: None,
        };
        audit::append(&tx, &e).map_err(internal)?;
        tx.commit().map_err(internal)?;
        // Published before the lock goes, like a request's events: see `publish`.
        for (ev, payload) in events {
            let mut e = Event::new(ev, now.clone(), Actor::System, payload);
            e.project_id = project_id;
            let _ = self.events_tx.send(e);
        }
        drop(conn);
        Ok(out)
    }

    // ------------------------------------------------------------------ pipeline

    /// Parse a raw line/JSON into a request, or the `bus.parse` response for it (§1.2).
    pub fn parse(raw: &str) -> Result<Request, Response> {
        // The common case is one parse straight into the envelope. Building a `Value` first
        // cost every request a second full parse and a throwaway tree; that work now happens
        // only on the failing path, where it buys the request id to answer with.
        match serde_json::from_str::<Request>(raw) {
            Ok(r) => return Ok(r),
            Err(e) if e.is_syntax() || e.is_eof() => {
                return Err(Response::unparsed(BusError::parse(e)))
            }
            Err(_) => {}
        }
        let v: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => return Err(Response::unparsed(BusError::parse(e))),
        };
        let id = v
            .get("id")
            .and_then(|i| i.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        match serde_json::from_value::<Request>(v) {
            Ok(r) => Ok(r),
            Err(e) => Err(match id {
                Some(id) => Response::err(id, BusError::envelope(e)),
                None => Response::unparsed(BusError::envelope(e)),
            }),
        }
    }

    /// The pipeline. Synchronous; holds the store lock for the handler's duration.
    pub fn dispatch(&self, req: Request, door: Door) -> Response {
        self.dispatch_with_watch_state(req, door, None)
    }

    pub(crate) fn dispatch_socket_watch(&self, req: Request, already_watching: bool) -> Response {
        self.dispatch_with_watch_state(req, Door::Socket, Some(already_watching))
    }

    fn dispatch_with_watch_state(&self, req: Request, door: Door, watching: Option<bool>) -> Response {
        let id = req.id;
        let mut response = match self.dispatch_inner(&req, door, watching) {
            Ok(resp) => resp,
            Err(e) => Response::err(id, e),
        };
        // Priority mail is current session state, not part of an operation's typed result or
        // audit record. Resolve the actor again after the transaction so acknowledgements and
        // idempotent replays always carry the live count. Authentication failures get no hint.
        response.mail = self.mail_hint(&req, door);
        response
    }

    /// The post-commit `mail` sideband: the same authentication [`Engine::resolve_actor`] does,
    /// and the count, under one acquisition of the store lock rather than two. A request that
    /// cannot be attributed to a live, authenticated session gets no hint, exactly as before.
    fn mail_hint(&self, req: &Request, door: Door) -> Option<MailHint> {
        let name = req.actor.session_name()?;
        let conn = self.store.lock();
        let row = crate::sessions::by_name(&conn, name).ok()?;
        if matches!(door, Door::Socket) {
            let token = req.token.as_deref()?;
            if row.token.is_empty() || row.token != token {
                return None;
            }
        }
        let count: i64 = conn
            .prepare_cached(UNREAD_PRIORITY_SQL)
            .ok()?
            .query_row([row.session.id], |row| row.get(0))
            .ok()?;
        (count > 0).then(|| MailHint {
            priority: u32::try_from(count).unwrap_or(u32::MAX),
        })
    }

    fn dispatch_inner(&self, req: &Request, door: Door, watching: Option<bool>) -> Result<Response, BusError> {
        // envelope
        if req.v != ENVELOPE_V {
            return Err(BusError::envelope(format!(
                "v {} is not supported (this engine speaks v{ENVELOPE_V})",
                req.v
            )));
        }
        if !req.payload.is_object() {
            return Err(BusError::envelope("payload must be an object"));
        }
        let entry = Registry::global()
            .get(&req.op)
            .ok_or_else(|| BusError::unknown_op(&req.op))?;
        let audited = should_audit(entry, &req.actor);
        // An audited request holds its id from the lookup below until its audit row is written,
        // so a duplicate sent meanwhile — a retry after a timeout, during a staged op's slow
        // prepare — waits and replays that row instead of running the op twice (§5.3). Taken
        // before the store: the request it may wait for needs the store to finish.
        let _reserved = audited.then(|| self.reserve_id(req.id));
        // Actor, identity and idempotency share one acquisition of the store and one read of
        // the session row. A user's unaudited request — every keystroke — takes no lock (D148).
        let (session_id, filled) = {
            let conn = (audited || req.actor.is_agent()).then(|| self.store.lock());
            // actor
            let row = self.resolve_actor(conn.as_deref(), req, door)?;
            // An agent already *is* a project and a session. Making it restate both on every
            // call turned each op into guess-and-retry, so the engine fills the two identity
            // fields when an op requires them and the caller left them out (D110). Only required
            // fields are filled: an optional `project_id` is usually one half of an either/or.
            let filled = Self::fill_identity(req, entry, row.as_ref());
            let req = filled.as_ref().unwrap_or(req);
            // idempotency
            if let (true, Some(conn)) = (audited, conn.as_deref()) {
                if let Some(rec) = audit::lookup(conn, req.id).map_err(internal)? {
                    if rec.payload_hash != audit::payload_hash(&req.payload) {
                        return Err(BusError::conflict(
                            "bus.id_reused",
                            "this request id was already used with a different payload",
                        ));
                    }
                    return Ok(rec.response.replayed());
                }
            }
            (row.map(|row| row.session.id), filled)
        };
        let req = filled.as_ref().unwrap_or(req);
        // validate (before authorization — BUS.md §5; for every op, implemented or not)
        (entry.validate)(&req.payload).map_err(|e| BusError::schema(entry.name, e))?;
        // authorize — fixed op-level allowlist, then the session role/options/scope.
        if !entry.meta.actors.admits(&req.actor) {
            let err = BusError::allowlist(
                entry.name,
                if req.actor.is_agent() {
                    "agent"
                } else {
                    "anonymous"
                },
            )
            .with_hint("this op is user-only (BUS.md §9.1 layer 1)");
            if audited {
                self.audit_only(req, entry, session_id, &Err(err.clone()))?;
            }
            return Err(err);
        }
        if let Some(sid) = session_id {
            let conn = self.store.lock();
            if let Err(err) = crate::guardrail::authorize(&conn, sid, entry, &req.payload) {
                drop(conn);
                if audited {
                    self.audit_only(req, entry, session_id, &Err(err.clone()))?;
                }
                return Err(err);
            }
        }
        // Socket watch leases are idempotent, but even duplicate calls must pass the
        // complete envelope, schema and actor checks above before becoming a no-op.
        if matches!(entry.name, "device.watch" | "app.resources.watch") && watching.is_some() && watching == req.payload["on"].as_bool() {
            return Ok(Response::ok(req.id, serde_json::json!({})));
        }
        // Keystrokes never touch SQLite (D148). The store is one connection behind one mutex, so
        // a per-character request that opened a transaction queued behind whatever slow op held
        // it — which is exactly why typing stalled while a git scan or `adb devices` ran. Both
        // ops are `UserOnly` with `AgentOnly` audit, so by this point nothing is left to
        // persist: the envelope, door, payload and actor allowlist checks have all run, the
        // actor is privileged (never a session), and `audited` is false.
        if matches!(entry.name, "session.input" | "session.resize")
            && !audited
            && session_id.is_none()
        {
            if let Some(result) = self.pty_fast_path(entry.name, &req.payload)? {
                return Ok(Response::ok(req.id, result));
            }
        }
        // A mirror touch-move is the same kind of request: one per pointer event, felt as lag,
        // and `UserOnly` with `Audit::Never` — so nothing is left to persist here either.
        if entry.name == "device.mirror.input" && !audited && session_id.is_none() {
            return self.mirror_fast_path(&req.payload).map(|result| Response::ok(req.id, result));
        }
        // Slow reads run with nothing locked (D149). Queries are never audited, so there is no
        // row to append and no transaction to keep open across an `adb`, `gh` or `gix` call.
        if let Some(h) = self.unlocked.get(entry.name).cloned() {
            return self.dispatch_unlocked(req, entry, session_id, h);
        }
        // Implementation availability is deliberately checked after authorization: an
        // unfinished surface must not become a side door around role/scope policy.
        let Some(h) = self.handlers.get(entry.name) else {
            let err = BusError::not_implemented(entry.name, entry.meta.phase);
            if audited {
                self.audit_only(req, entry, session_id, &Err(err.clone()))?;
            }
            return Err(err);
        };

        // The read/external phase of a staged mutation, before anything is locked (D149).
        let (staged, staged_events, staged_after) = match self.prepares.get(entry.name).cloned() {
            Some(prepare) => {
                let mut stage = Unlocked {
                    engine: self,
                    held: None,
                    actor: req.actor.clone(),
                    req_id: req.id,
                    now: crate::time::now(),
                    op: entry.name,
                    session_id,
                    project_id: None,
                    events: Vec::new(),
                    deferred: Vec::new(),
                };
                let out = prepare(&mut stage, &req.payload);
                let Unlocked {
                    events, deferred, ..
                } = stage;
                match out {
                    Ok(value) => (Some(value), events, deferred),
                    Err(err) => {
                        // Same rule as an ordinary handler failure below: audited, unless the
                        // request was simply malformed.
                        if audited && err.kind != relay_bus::ErrorKind::Invalid {
                            self.audit_only(req, entry, session_id, &Err(err.clone()))?;
                        }
                        return Err(err);
                    }
                }
            }
            None => (None, Vec::new(), Vec::new()),
        };
        // handler, in a transaction
        let mut span = LockSpan::opening(entry.name);
        let mut conn = self.store.lock();
        span.acquired();
        let tx = conn.transaction().map_err(internal)?;
        let now = crate::time::now();
        let mut ctx = Ctx {
            tx: &tx,
            engine: self,
            actor: req.actor.clone(),
            req_id: req.id,
            now: now.clone(),
            op: entry.name,
            payload: std::borrow::Cow::Borrowed(&req.payload),
            skip_policy: None,
            events: staged_events,
            undo: None,
            undo_of: None,
            mark_undone: None,
            project_id: None,
            session_id,
            commit_error: false,
            hold_id: None,
            audit_op: None,
            on_behalf_of: None,
            after_commit: staged_after,
            staged,
        };
        let outcome = (h.call)(&mut ctx, &req.payload);
        let Ctx {
            events,
            undo,
            undo_of,
            mark_undone,
            project_id,
            session_id,
            commit_error,
            hold_id,
            audit_op,
            on_behalf_of,
            after_commit,
            ..
        } = ctx;
        let audit_op = audit_op.as_deref().unwrap_or(entry.name);

        match outcome {
            Ok(result) => {
                if audited {
                    let outcome = Ok(result.clone());
                    let e = AuditEntry {
                        on_behalf_of: on_behalf_of.as_ref(),
                        hold_id,
                        undo_op: undo.as_ref(),
                        undo_of,
                        ..request_audit(req, &now, audit_op, session_id, project_id, &outcome)
                    };
                    let audit_id = audit::append(&tx, &e).map_err(internal)?;
                    if let Some(original) = mark_undone {
                        tx.execute(
                            "UPDATE audit SET undone_by=?1 WHERE id=?2 AND undone_by IS NULL",
                            rusqlite::params![audit_id, original],
                        )
                        .map_err(internal)?;
                    }
                }
                tx.commit().map_err(internal)?;
                self.publish(conn, span, events, after_commit);
                Ok(Response::ok(req.id, result))
            }
            Err(err) => {
                let outcome = Err(err.clone());
                if commit_error {
                    if audited {
                        let project_id = project_id.or_else(|| session_project(&tx, session_id));
                        let e = AuditEntry {
                            on_behalf_of: on_behalf_of.as_ref(),
                            hold_id,
                            ..request_audit(req, &now, audit_op, session_id, project_id, &outcome)
                        };
                        audit::append(&tx, &e).map_err(internal)?;
                    }
                    tx.commit().map_err(internal)?;
                    self.publish(conn, span, events, after_commit);
                    return Err(err);
                }
                // ordinary failure: roll back handler work; audit in its own transaction
                tx.rollback().map_err(internal)?;
                if audited && err.kind != relay_bus::ErrorKind::Invalid {
                    // A handler usually names its project last, so a failure falls back to the
                    // session's — the same project `audit_only` records for a refusal.
                    let project_id = project_id.or_else(|| session_project(&conn, session_id));
                    let tx = conn.transaction().map_err(internal)?;
                    let e = AuditEntry {
                        on_behalf_of: on_behalf_of.as_ref(),
                        hold_id,
                        ..request_audit(req, &now, audit_op, session_id, project_id, &outcome)
                    };
                    audit::append(&tx, &e).map_err(internal)?;
                    tx.commit().map_err(internal)?;
                }
                drop(conn);
                span.finish();
                Err(err)
            }
        }
    }

    /// After a commit: publish the request's events, release the store, then run its
    /// after-commit work. The events go out while the lock is still held, so two commits that
    /// touch the same row reach subscribers in commit order — the native client applies a
    /// `session.changed` in place, and one sent late would overwrite the newer state (or bring
    /// back a closed session). A broadcast send never blocks.
    fn publish(
        &self,
        conn: std::sync::MutexGuard<'_, rusqlite::Connection>,
        span: LockSpan,
        events: Vec<Event>,
        after_commit: Vec<Box<dyn FnOnce(Arc<Engine>) + Send + 'static>>,
    ) {
        for ev in events {
            let _ = self.events_tx.send(ev);
        }
        drop(conn);
        span.finish();
        self.run_deferred(after_commit);
    }

    /// Run a request's after-commit work, with nothing locked.
    fn run_deferred(&self, deferred: Vec<Box<dyn FnOnce(Arc<Engine>) + Send + 'static>>) {
        if deferred.is_empty() {
            return;
        }
        if let Some(arc) = self.arc() {
            for f in deferred {
                f(arc.clone());
            }
        }
    }

    /// Hold `id` for one audited request (BUS.md §5.3), first waiting out any request that
    /// already holds it. Released when the guard drops, after the audit row is written.
    fn reserve_id(&self, id: Uuid) -> ReservedId<'_> {
        let held = self.in_flight.lock().unwrap();
        let mut held = self.in_flight_done.wait_while(held, |held| held.contains(&id)).unwrap();
        held.insert(id);
        ReservedId { engine: self, id }
    }

    /// Write an audit row for a request that never reached a handler (refused / unavailable).
    fn audit_only(
        &self,
        req: &Request,
        entry: &OpEntry,
        session_id: Option<Id>,
        outcome: &Result<Value, BusError>,
    ) -> Result<(), BusError> {
        let now = crate::time::now();
        let mut conn = self.store.lock();
        let project_id = session_project(&conn, session_id);
        let tx = conn.transaction().map_err(internal)?;
        let e = request_audit(req, &now, entry.name, session_id, project_id, outcome);
        audit::append(&tx, &e).map_err(internal)?;
        tx.commit().map_err(internal)
    }

    /// Fill `project_id` / `session` from the authenticated agent's session row when the op
    /// requires them and the payload omits them. Returns `None` when nothing needed filling, so
    /// the common path never clones the request.
    fn fill_identity(
        req: &Request,
        entry: &OpEntry,
        row: Option<&crate::sessions::Row_>,
    ) -> Option<Request> {
        let row = row?;
        let object = req.payload.as_object().expect("payload is an object");
        let registry = Registry::global();
        let wants = |field: &str| {
            !object.contains_key(field) && registry.payload_requires(entry.name, field)
        };
        let (needs_project, needs_session) = (wants("project_id"), wants("session"));
        if !needs_project && !needs_session {
            return None;
        }
        let mut next = req.clone();
        let object = next.payload.as_object_mut().expect("payload is an object");
        if needs_project {
            object.insert("project_id".into(), Value::from(row.session.project_id));
        }
        if needs_session {
            object.insert("session".into(), Value::from(row.session.name.clone()));
        }
        Some(next)
    }

    /// Run a query registered with [`Engine::register_unlocked`]: no store lock held for the
    /// handler's duration, events emitted and deferred work run once it returns.
    fn dispatch_unlocked(
        &self,
        req: &Request,
        entry: &OpEntry,
        session_id: Option<Id>,
        h: UnlockedFn,
    ) -> Result<Response, BusError> {
        let mut ctx = Unlocked {
            engine: self,
            held: None,
            actor: req.actor.clone(),
            req_id: req.id,
            now: crate::time::now(),
            op: entry.name,
            session_id,
            project_id: None,
            events: Vec::new(),
            deferred: Vec::new(),
        };
        let outcome = h(&mut ctx, &req.payload);
        let Unlocked {
            events, deferred, ..
        } = ctx;
        // A failed query publishes nothing, the same way a rolled-back handler does.
        let result = outcome?;
        for ev in events {
            let _ = self.events_tx.send(ev);
        }
        self.run_deferred(deferred);
        Ok(Response::ok(req.id, result))
    }

    /// Whether this request is certain to be answered from memory alone, so a door can run it
    /// on the thread it arrived on instead of handing it to the blocking pool.
    ///
    /// True only for the two ops [`Engine::pty_fast_path`] serves, from an actor that is not a
    /// session, against a PTY that is in the map right now — every one of those an in-memory
    /// check under a short-lived map lock, never the store. The hop costs about 25 µs and a
    /// thread wake-up, which is most of what a keystroke spends before reaching the terminal.
    /// Being wrong is not a correctness problem: the request simply takes the ordinary
    /// pipeline, inline, and the two ops it admits never do more than one write to a file
    /// descriptor.
    pub fn answers_from_memory(&self, req: &Request) -> bool {
        if req.actor.is_agent() {
            return false;
        }
        match req.op.as_str() {
            // A resize is an ioctl; it cannot block.
            "session.resize" => {}
            // A keystroke is a handful of bytes, but a paste can exceed the child's input
            // queue, and that write blocks until the child drains it. Only the small case is
            // safe to run on a thread the runtime shares.
            "session.input" => {
                let small = req
                    .payload
                    .get("data")
                    .and_then(Value::as_str)
                    .is_some_and(|data| data.len() <= INLINE_INPUT_BYTES);
                if !small {
                    return false;
                }
                // Behind a paste that is still draining, even a small write waits (RA-341).
                let name = req.payload.get("session").and_then(Value::as_str).unwrap_or_default();
                return self.pty_named(name).is_some_and(|(_, pty)| pty.writer_idle());
            }
            // One control message to the device. Text and clipboard payloads can be large
            // enough to block on a full socket, so only the pointer/key events stay inline.
            "device.mirror.input" => {
                let inline = !req.payload["event"]["type"]
                    .as_str()
                    .is_some_and(|kind| matches!(kind, "text" | "setclipboard"));
                return inline
                    && req.payload["mirror_id"]
                        .as_i64()
                        .is_some_and(|id| self.mirrors.lock().unwrap().contains_key(&id));
            }
            _ => return false,
        }
        req.payload
            .get("session")
            .and_then(Value::as_str)
            .is_some_and(|name| self.pty_ids.lock().unwrap().contains_key(name))
    }

    /// `session.input` / `session.resize` against a live PTY, with no store lock and no
    /// transaction. `Ok(None)` means "not resolvable from memory" — an unknown or unspawned
    /// session — and the request falls through to the registered handler, which owns the exact
    /// typed error for that case.
    fn pty_fast_path(&self, op: &str, payload: &Value) -> Result<Option<Value>, BusError> {
        use relay_bus::ops::session::{InputIn, ResizeIn};
        use serde::Deserialize;
        let name = payload
            .get("session")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some((session_id, pty)) = self.pty_named(name) else {
            return Ok(None);
        };
        match op {
            "session.input" => {
                let p = InputIn::deserialize(payload)
                    .map_err(|e| BusError::schema("session.input", e))?;
                if pty.exited() {
                    return Err(BusError::conflict(
                        "session.exited",
                        format!("session {} has exited", p.session),
                    ));
                }
                // The one keystroke per idle period that owes the store a row: hand it to a
                // worker so the character itself is already in the PTY either way.
                if write_input(&pty, &p.data)? {
                    if let Some(engine) = self.arc() {
                        std::thread::spawn(move || engine.mark_session_running(session_id));
                    }
                }
            }
            "session.resize" => {
                let p = ResizeIn::deserialize(payload)
                    .map_err(|e| BusError::schema("session.resize", e))?;
                pty.resize(p.cols, p.rows)
                    .map_err(|e| BusError::conflict("session.io", e.to_string()))?;
            }
            _ => return Ok(None),
        }
        Ok(Some(json!({})))
    }

    /// `device.mirror.input` with no store lock and no transaction: a map lookup, an encode and
    /// one write to the device's control socket. The mirror registry is memory-only, so unlike
    /// the PTY path there is nothing to fall through to — an unknown id is answered here too.
    fn mirror_fast_path(&self, payload: &Value) -> Result<Value, BusError> {
        use serde::Deserialize;
        // Read through the payload, not out of a clone of it: this runs once per pointer event.
        let p = relay_bus::ops::device::MirrorInputIn::deserialize(payload)
            .map_err(|e| BusError::schema("device.mirror.input", e))?;
        let runtime = crate::handlers::device::mirror_by_id(self, p.mirror_id)?;
        crate::handlers::device::send_input(&runtime, &p.event)?;
        Ok(json!({}))
    }

    /// The deferred half of the input fast path: move an idle session back to `running` and tell
    /// the UI. Off the request thread, so a keystroke never waits for the store.
    fn mark_session_running(&self, session_id: Id) {
        let _ = self.system_write(
            "session.input",
            None,
            None,
            Some(session_id),
            Value::Null,
            |tx, now| {
                let changed = tx
                    .execute(
                        "UPDATE sessions SET state='running',updated_at=?1 WHERE id=?2 AND state='idle'",
                        rusqlite::params![now, session_id],
                    )
                    .map_err(internal)?;
                if changed == 0 {
                    return Ok(((), Vec::new()));
                }
                let events = match crate::sessions::by_id(tx, session_id)? {
                    Some(row) => vec![(
                        "session.changed".to_string(),
                        serde_json::to_value(&row.session).unwrap_or(Value::Null),
                    )],
                    None => Vec::new(),
                };
                Ok(((), events))
            },
        );
    }

    /// BUS.md §4.2. The bound session's row when the actor is an agent, read from `conn` —
    /// which the caller holds whenever the actor is an agent. On the socket door, `agent:<name>`
    /// and its token must match a non-closed session.
    fn resolve_actor(
        &self,
        conn: Option<&rusqlite::Connection>,
        req: &Request,
        door: Door,
    ) -> Result<Option<crate::sessions::Row_>, BusError> {
        let name = match &req.actor {
            Actor::System => {
                return Err(BusError::actor("\"system\" is internal and cannot be claimed"))
            }
            Actor::Test if !self.instance.accepts_test_actor() => {
                return Err(BusError::actor("\"test\" actor is only accepted by dev/test instances"))
            }
            Actor::User | Actor::Test => return Ok(None),
            Actor::Agent(name) => name,
        };
        let conn = conn.expect("an agent's request resolves under the store lock");
        match door {
            Door::InProcess => Ok(crate::sessions::by_name(conn, name).ok()),
            Door::Socket => {
                let Some(token) = req.token.as_deref() else {
                    return Err(BusError::actor(format!(
                        "agent:{name} requires a token on the socket door"
                    )));
                };
                let row = crate::sessions::by_name(conn, name)
                    .map_err(|_| BusError::actor(format!("unknown session {name:?}")))?;
                if row.token.is_empty() || row.token != token {
                    return Err(BusError::actor(format!("bad token for session {name:?}")));
                }
                Ok(Some(row))
            }
        }
    }

}

const UNREAD_PRIORITY_SQL: &str = "SELECT COUNT(*) FROM message_recipients r
     JOIN messages m ON m.id = r.message_id
     WHERE r.session_id = ?1 AND r.acked_at IS NULL AND m.priority = 1";

/// BUS.md §5.1a.
pub fn should_audit(entry: &OpEntry, actor: &Actor) -> bool {
    match (entry.meta.kind, entry.meta.audit) {
        (OpKind::Query, _) => false,
        (OpKind::Mutation, Audit::Always) => true,
        (OpKind::Mutation, Audit::AgentOnly) => actor.is_agent(),
        (OpKind::Mutation, Audit::Never) => false,
    }
}

/// An audited request's id, held from its idempotency lookup to its audit row.
struct ReservedId<'a> {
    engine: &'a Engine,
    id: Uuid,
}

impl Drop for ReservedId<'_> {
    fn drop(&mut self) {
        self.engine.in_flight.lock().unwrap().remove(&self.id);
        self.engine.in_flight_done.notify_all();
    }
}

/// The audit row for a request, with the fields only a handler sets left empty.
fn request_audit<'a>(
    req: &'a Request,
    now: &'a str,
    op: &'a str,
    session_id: Option<Id>,
    project_id: Option<Id>,
    outcome: &'a Result<Value, BusError>,
) -> AuditEntry<'a> {
    AuditEntry {
        ts: now,
        req_id: req.id,
        parent_req: None,
        actor: &req.actor,
        on_behalf_of: None,
        session_id,
        op,
        project_id,
        payload: &req.payload,
        outcome,
        hold_id: None,
        undo_op: None,
        undo_of: None,
    }
}

/// The project an agent's session belongs to.
fn session_project(conn: &rusqlite::Connection, session_id: Option<Id>) -> Option<Id> {
    let session_id = session_id?;
    conn.prepare_cached("SELECT project_id FROM sessions WHERE id = ?1")
        .ok()?
        .query_row([session_id], |row| row.get(0))
        .ok()
}

/// Map an infrastructure error to `internal`, logging it.
pub fn internal(e: impl std::fmt::Display) -> BusError {
    tracing::error!(error = %e, "internal error");
    BusError::internal(e.to_string())
}

/// `?` sugar for `anyhow`/`rusqlite` errors inside handlers.
pub trait IntoBus<T> {
    fn bus(self) -> Result<T, BusError>;
}
impl<T, E: std::fmt::Display> IntoBus<T> for Result<T, E> {
    fn bus(self) -> Result<T, BusError> {
        self.map_err(internal)
    }
}

/// Write a keystroke batch to a live PTY, shared by the in-memory fast path and the staged
/// `session.input` fallback. `true` when this write ends an idle period, which owes the store
/// a `running` row (RA-646).
pub(crate) fn write_input(pty: &crate::pty::Pty, data: &str) -> Result<bool, BusError> {
    pty.write(data.as_bytes())
        .map_err(|e| BusError::conflict("session.io", e.to_string()))?;
    Ok(!data.is_empty() && pty.claim_idle_edge())
}

#[cfg(test)]
mod inline_path_tests {
    use super::*;
    use relay_bus::Request;
    use serde_json::json;

    fn engine() -> Arc<Engine> {
        Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap())
    }

    fn input(session: &str, data: &str) -> Request {
        Request::new(Actor::User, "session.input", json!({"session":session,"data":data}))
    }

    /// The gate decides only *where* a request runs, so every case it turns down still has to
    /// be answered — by the ordinary pipeline, on the blocking pool.
    #[test]
    fn only_memory_resolvable_keystrokes_skip_the_blocking_pool() {
        let e = engine();
        // Nothing is spawned yet, so even a well-formed keystroke has no PTY to reach.
        assert!(!e.answers_from_memory(&input("brisk-otter", "a")));

        let pty = crate::pty::Pty::spawn(
            crate::pty::SpawnSpec {
                cmd: "/bin/cat".into(),
                args: Vec::new(),
                env: Vec::new(),
                cwd: std::env::temp_dir(),
                cols: 80,
                rows: 24,
                epoch: 1,
                initial_scrollback: Vec::new(),
            },
            |_| {},
        )
        .unwrap();
        e.set_pty(1, "brisk-otter", pty);

        assert!(e.answers_from_memory(&input("brisk-otter", "a")));
        assert!(e.answers_from_memory(&input("brisk-otter", "")));
        assert!(e.answers_from_memory(&Request::new(
            Actor::User,
            "session.resize",
            json!({"session":"brisk-otter","cols":100,"rows":40}),
        )));

        // A session nobody has spawned, an op that reaches the store, and an agent actor —
        // whose keystroke is audited and authorized against its session row — all take the pool.
        assert!(!e.answers_from_memory(&input("quiet-lemur", "a")));
        assert!(!e.answers_from_memory(&Request::new(
            Actor::User,
            "session.list",
            json!({"session":"brisk-otter"}),
        )));
        assert!(!e.answers_from_memory(&Request::new(
            Actor::agent("brisk-otter"),
            "session.input",
            json!({"session":"brisk-otter","data":"a"}),
        )));

        // A paste large enough to block on the child's input queue does not run inline.
        let big = "x".repeat(INLINE_INPUT_BYTES + 1);
        assert!(!e.answers_from_memory(&input("brisk-otter", &big)));
        assert!(e.answers_from_memory(&input("brisk-otter", &"x".repeat(INLINE_INPUT_BYTES))));

        // A malformed payload is the handler's error to report, not the gate's to guess at.
        assert!(!e.answers_from_memory(&Request::new(Actor::User, "session.input", json!({}))));
        assert!(!e.answers_from_memory(&Request::new(
            Actor::User,
            "session.input",
            json!({"session":"brisk-otter","data":7}),
        )));

        e.shutdown();
    }

    /// The same audited request sent from several threads at once runs once; every other copy
    /// replays it. Without the in-flight reservation a copy could pass the lookup while the
    /// first was still running and then fail the `audit.req_id` constraint as `internal`.
    #[test]
    fn concurrent_duplicates_of_one_request_run_once_and_replay() {
        let e = engine();
        for round in 0..100 {
            let req = Request::new(Actor::User, "settings.set", json!({"path": "a.b", "value": round}));
            let start = std::sync::Barrier::new(6);
            let responses: Vec<Response> = std::thread::scope(|scope| {
                let workers: Vec<_> = (0..6)
                    .map(|_| {
                        scope.spawn(|| {
                            let req = req.clone();
                            start.wait();
                            e.dispatch(req, Door::InProcess)
                        })
                    })
                    .collect();
                workers.into_iter().map(|w| w.join().unwrap()).collect()
            });
            for r in &responses {
                assert!(r.ok, "every copy succeeds: {:?}", r.error);
            }
            let ran = responses.iter().filter(|r| r.replayed.is_none()).count();
            assert_eq!(ran, 1, "exactly one copy executes in round {round}");
        }
    }
}
