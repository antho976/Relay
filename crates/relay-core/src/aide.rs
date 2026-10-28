//! Aide, the person's own agent, in the engine (docs/AIDE.md). `relay_aide` holds the rules; this
//! wakes jobs and runs them.
//!
//! Two threads of its own, both started once by `serve`:
//! - the **scheduler** sleeps until the earliest thing due (a job's schedule, a follow-up, a
//!   run's deadline, a session that has waited long enough, an errand past its time) and wakes
//!   early when anything changes ([`poke`]). It is one deadline, not a tick; the wait is capped at
//!   [`CHECK`] only because a monotonic wait does not count time the PC was suspended.
//! - the **watcher** reads the engine's events and checks the data rules of the jobs that listen
//!   for them ([`rules`]), and notices an Aide thread's turn ending ([`runs::turn_ended`]).
//!
//! A run is a turn in its job's channel, an Aide thread (`threads::AIDE`), on the agent machinery
//! every thread uses: Claude Code with Relay's MCP door as its only tools, signed in to the
//! person's own Claude account, or pointed at a model on this PC or on the person's server
//! ([`models`]). Nothing here runs inside a handler or holds the store's lock while it waits;
//! `aide.db` has a mutex of its own.

pub mod models;
pub mod outbox;
pub mod rules;
pub mod runs;

use crate::engine::Engine;
use relay_aide::store::Aide;
use relay_bus::BusError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;

/// The longest the scheduler sleeps between looks, whatever is due: a guard against a clock
/// that jumped or a PC that slept through a deadline.
pub const CHECK: Duration = Duration::from_secs(15 * 60);

/// The engine's Aide: `aide.db`, opened on first use, and what its threads share.
#[derive(Default)]
pub struct Shelf {
    db: Mutex<Option<Aide>>,
    /// Bumped by [`poke`]; the scheduler waits on it.
    wake: Arc<(Mutex<u64>, Condvar)>,
    started: OnceLock<()>,
    /// Held while queued runs are started (`runs::advance`).
    pub(crate) starting: Mutex<()>,
    /// Sessions seen blocked, and since when (RFC 3339): the stuck rule's clock.
    pub(crate) blocked: Mutex<HashMap<String, String>>,
    /// Each Aide thread's running run's turn: the run, its turn's token and its wake message
    /// (`runs::turn_ended`).
    pub(crate) turns: Mutex<HashMap<i64, runs::RunTurn>>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn sql(e: rusqlite::Error) -> BusError {
    BusError::internal(format!("aide.db: {e}"))
}

/// Runs `f` on `aide.db`, opening it beside the store on first use (in memory when the store is,
/// as in tests).
pub fn with_db<T>(engine: &Engine, f: impl FnOnce(&Aide) -> rusqlite::Result<T>) -> Result<T, BusError> {
    let mut slot = lock(&engine.aide.db);
    if slot.is_none() {
        let store = engine.store.path();
        let opened = if store == Path::new(":memory:") {
            Aide::open_in_memory()
        } else {
            let path = store.with_file_name("aide.db");
            Aide::open(&path).inspect(|_| crate::private::private_db(&path))
        };
        *slot = Some(opened.map_err(sql)?);
    }
    f(slot.as_ref().expect("opened above")).map_err(sql)
}

pub fn settings(engine: &Engine) -> Result<relay_aide::model::Settings, BusError> {
    with_db(engine, |db| db.settings())
}

/// Something changed that may move the next wake: the scheduler looks again now.
pub fn poke(engine: &Engine) {
    let (count, cv) = &*engine.aide.wake;
    *lock(count) += 1;
    cv.notify_all();
}

/// `aide.changed`, from outside a handler.
pub fn changed(engine: &Engine, what: &str) {
    engine.emit_system("aide.changed", serde_json::json!({"what": what}));
}

/// Starts the scheduler and the watcher, once.
pub fn spawn(engine: &Arc<Engine>) {
    if engine.aide.started.set(()).is_err() {
        return;
    }
    // Runs this engine's last process left open did not finish: their agents went with it.
    runs::reconcile(engine);
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new().name("aide".into()).spawn(move || schedule_loop(weak)).ok();
    let weak = Arc::downgrade(engine);
    let events = engine.subscribe();
    std::thread::Builder::new().name("aide-watch".into()).spawn(move || rules::watch(weak, events)).ok();
}

fn schedule_loop(weak: Weak<Engine>) {
    crate::background_priority();
    loop {
        // The engine is held only while looking, never through the wait, so it can go away.
        let (wake, seen, wait) = {
            let Some(engine) = weak.upgrade() else { return };
            if engine.is_quitting() {
                return;
            }
            let wake = engine.aide.wake.clone();
            let seen = *lock(&wake.0);
            let next = runs::tick(&engine);
            let wait = next.map_or(CHECK, |at| {
                let ms = at.as_millisecond() - jiff::Timestamp::now().as_millisecond();
                Duration::from_millis(u64::try_from(ms.max(0)).unwrap_or(0)).min(CHECK)
            });
            (wake, seen, wait)
        };
        let (count, cv) = &*wake;
        let guard = lock(count);
        if *guard == seen {
            let _ = cv.wait_timeout_while(guard, wait.max(Duration::from_millis(50)), |c| *c == seen);
        }
    }
}

/// Wakes the scheduler so it sees the engine is quitting.
pub fn shutdown(engine: &Engine) {
    poke(engine);
}

/// `text` cut to about `max` characters, on a character boundary, marked when cut.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}…")
}
