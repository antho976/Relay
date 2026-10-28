//! `device.leases`, `device.claim`, `device.release`, and the lease plumbing the rest of the
//! engine calls: `device.run`, the guardrail hook's shell commands, session exit and close.
//! The lease map itself is `crate::device_lease`.

use crate::device_lease::{self, Holder, Kind, Lease};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::sessions;
use relay_bus::error::BusError;
use relay_bus::ops::device::{Claim, Leases, LeasesOut, Release, ReleaseOut};
use relay_bus::types::{Id, SessionState};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

pub const ACQUIRED: &str = "device.lease.acquired";
pub const RELEASED: &str = "device.lease.released";
/// The longest the expiry sweeper sleeps before it looks again (and checks the engine is alive).
const SWEEP_CAP: Duration = Duration::from_secs(30);

pub fn register(e: &mut Engine) {
    e.register_unlocked::<Leases>(|ctx, _| {
        let released = ctx.read(|conn| Ok(prune(ctx.engine(), conn)))?;
        for lease in released { ctx.emit(RELEASED, lease.event()); }
        Ok(LeasesOut { leases: ctx.engine().device_leases.list().iter().map(Lease::view).collect() })
    });

    e.register::<Claim>(|ctx: &mut Ctx, p| {
        let device = p.device.as_deref().map(str::trim).unwrap_or(device_lease::ANY_DEVICE);
        if device.is_empty() || device.chars().any(char::is_whitespace) {
            return Err(BusError::invalid("device.serial", "device must be a serial from device.list, or omitted for every device"));
        }
        if p.action.trim().is_empty() {
            return Err(BusError::invalid("device.claim_action", "say what you will be doing with the device"));
        }
        let minutes = p.minutes.unwrap_or(device_lease::CLAIM_DEFAULT_MINUTES).clamp(1, device_lease::CLAIM_MAX_MINUTES);
        let holder = actor_holder(ctx)?;
        let lease = Lease::new(device, holder, Kind::Claim, &p.action, Some(Duration::from_secs(u64::from(minutes) * 60)));
        acquire(ctx, lease.clone())?;
        Ok(ctx.engine().device_leases.holder_of(&lease.device).map(|held| held.view()).unwrap_or_else(|| lease.view()))
    });

    e.register::<Release>(|ctx: &mut Ctx, p| {
        let holder = actor_holder(ctx)?;
        let user = ctx.actor_session_id().is_none() && !ctx.actor.is_agent();
        let device = p.device.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_string);
        let leases = &ctx.engine().device_leases;
        if let Some(device) = &device {
            if let Some(held) = leases.list().into_iter().find(|lease| &lease.device == device) {
                if let Kind::Run(run_id) = held.kind {
                    return Err(BusError::conflict("device.lease_run", format!("device {device} is held by device run {run_id}"))
                        .with_hint("stop the run with device.run.stop; its lease goes with it"));
                }
                if held.holder != holder && !user {
                    return Err(device_lease::busy(&held).with_hint("only the holder or the Relay user can release a lease"));
                }
            }
        }
        let selected = |lease: &Lease| {
            !matches!(lease.kind, Kind::Run(_))
                && device.as_ref().is_none_or(|device| &lease.device == device)
                && (lease.holder == holder || (user && device.is_some()))
        };
        let released = leases.release_where(selected);
        // release_where also drops every lapsed lease, whoever held it: those are announced, but
        // the answer lists only what the caller asked to give back.
        let views = released.iter().filter(|lease| selected(lease)).map(Lease::view).collect();
        for lease in released { ctx.emit(RELEASED, lease.event()); }
        Ok(ReleaseOut { released: views })
    });
}

/// The calling session, or the user.
fn actor_holder(ctx: &Ctx) -> Result<Holder, BusError> {
    match ctx.actor_session_id() {
        Some(id) => {
            let row = sessions::by_id(ctx.tx(), id)?.ok_or_else(|| BusError::internal("bound session vanished"))?;
            Ok(Holder::Session { id, name: row.session.name })
        }
        None if ctx.actor.is_agent() => Err(BusError::actor("device leases need a bound agent session")),
        None => Ok(Holder::User),
    }
}

/// Who holds a run started from `worktree`: the live session that owns that checkout, or the user.
pub(crate) fn worktree_holder(conn: &Connection, project_id: Id, worktree: &str) -> Result<Holder, BusError> {
    let owner: Option<(Id, String)> = conn
        .prepare_cached("SELECT id,name FROM sessions WHERE project_id=?1 AND worktree=?2 AND state!='closed' ORDER BY id LIMIT 1")
        .bus()?
        .query_row(rusqlite::params![project_id, worktree], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
        .bus()?;
    Ok(owner.map_or(Holder::User, |(id, name)| Holder::Session { id, name }))
}

/// Drop leases whose holder is gone: a run no longer live, a session that is not running.
pub(crate) fn prune(engine: &Engine, conn: &Connection) -> Vec<Lease> {
    if engine.device_leases.is_empty() {
        return Vec::new();
    }
    let live_runs: Vec<Id> = engine.device_runs.lock().unwrap_or_else(PoisonError::into_inner).keys().copied().collect();
    engine.device_leases.prune(|lease| match (lease.kind, &lease.holder) {
        (Kind::Run(id), _) => !live_runs.contains(&id),
        (_, Holder::Session { id, .. }) => !session_running(conn, *id),
        (_, Holder::User) => false,
    })
}

fn session_running(conn: &Connection, id: Id) -> bool {
    let state: Option<String> = conn
        .prepare_cached("SELECT state FROM sessions WHERE id=?1")
        .and_then(|mut statement| statement.query_row([id], |row| row.get(0)).optional())
        .ok()
        .flatten();
    state.is_some_and(|state| sessions::is_live(sessions::parse_state(&state)) || sessions::parse_state(&state) == SessionState::Created)
}

/// Take a lease inside a request: prune, acquire, queue the events. `device.busy` on conflict.
/// What the prune dropped is gone whatever the request's outcome, so its releases go out at
/// once rather than with the request's events, which a refusal rolls back unsent.
///
/// The acquisition itself is the request's, like its rows and its `acquired` event: it stands
/// once the transaction commits, and is taken back if a later step fails or the commit does
/// (RA-358). A rollback drops the after-commit closure, and the [`device_lease::Pending`] in it
/// with it.
pub(crate) fn acquire(ctx: &mut Ctx, lease: Lease) -> Result<(), BusError> {
    let mut released = prune(ctx.engine(), ctx.tx());
    let event = lease.event();
    let taken = ctx.engine().device_leases.acquire_pending(lease, &mut released);
    for gone in released { ctx.engine().emit_system(RELEASED, gone.event()); }
    let (taken, pending) = taken?;
    if taken { ctx.emit(ACQUIRED, event); }
    ctx.after_commit(move |_| pending.keep());
    ensure_sweeper(ctx.engine());
    Ok(())
}

/// A lease that simply runs out is released by nobody, yet `device.busy` tells the waiting
/// agent to `bus.wait` for `device.lease.released`. One thread per engine, alive only while a
/// lease is held, sleeps on the lease map until the next expiry and announces it. It holds the
/// engine weakly, so a dropped engine's sweeper ends at its next wake.
fn ensure_sweeper(engine: &Engine) {
    let Some(arc) = engine.arc() else { return };
    if !engine.device_leases.start_sweeper() { return; }
    let weak = Arc::downgrade(&arc);
    drop(arc);
    let leases = engine.device_leases.clone();
    let _ = std::thread::Builder::new().name("lease-expiry".into()).spawn(move || {
        while let Some(gone) = leases.wait_expired(SWEEP_CAP) {
            let Some(engine) = weak.upgrade() else { return };
            if engine.is_quitting() { return; }
            for lease in gone { engine.emit_system(RELEASED, lease.event()); }
        }
    });
}

/// Refuse early, before any slow work, when `holder` could not take `device`. Releases go out
/// at once, as in [`acquire`]: the refusal this exists to return would discard them.
pub(crate) fn check(ctx: &mut Ctx, device: &str, holder: &Holder) -> Result<(), BusError> {
    for gone in prune(ctx.engine(), ctx.tx()) { ctx.engine().emit_system(RELEASED, gone.event()); }
    ctx.engine().device_leases.check(device, holder)
}

/// The guardrail hook saw an agent about to run `command`. A device-writing command takes the
/// session's shell lease, or is refused with `device.busy` naming whoever holds the device.
pub(crate) fn gate_command(ctx: &mut Ctx, session_id: Id, session: &str, command: &str) -> Result<(), BusError> {
    let Some(found) = device_lease::device_command(command) else { return Ok(()) };
    let holder = Holder::Session { id: session_id, name: session.to_string() };
    acquire(ctx, Lease::new(&found.device, holder, Kind::Shell, &found.action, Some(device_lease::SHELL_RUNNING)))
}

/// A tool call finished (PostToolUse) or the turn stopped. `tool_input` is the hook's, when it
/// has one. A shell lease lasts only the grace period once every device command under it has
/// finished; a command sent to the background (`run_in_background`, or a trailing `&`) has
/// not, though its PostToolUse arrives at once, so its lease keeps the full term.
pub(crate) fn command_finished(engine: &Engine, session_id: Id, tool_input: Option<&Value>, turn_ended: bool) {
    if turn_ended {
        engine.device_leases.shell_turn_ended(session_id);
        return;
    }
    let Some(command) = tool_input.and_then(|input| input.get("command")).and_then(Value::as_str) else { return };
    let Some(found) = device_lease::device_command(command) else { return };
    let line = command.trim_end();
    let background = tool_input.and_then(|input| input.get("run_in_background")).and_then(Value::as_bool).unwrap_or(false)
        || (line.ends_with('&') && !line.ends_with("&&"));
    engine.device_leases.shell_command_finished(session_id, &found.device, background);
}

/// Release a finished run's lease, from its worker (no request open).
pub fn release_run(engine: &Engine, run_id: Id) {
    for lease in engine.device_leases.release_run(run_id) {
        engine.emit_system(RELEASED, lease.event());
    }
}

/// A session exited or closed: its shell leases and claims go with it, with events.
pub fn release_session(engine: &Engine, session_id: Id) {
    for lease in engine.device_leases.release_session(session_id) {
        engine.emit_system(RELEASED, lease.event());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::{Actor, Request};

    #[test]
    fn a_release_answers_only_for_the_callers_own_leases() {
        let engine = Engine::new(crate::paths::Instance::Test, crate::Store::open_memory().unwrap());
        let mut events = engine.subscribe();
        let other = Holder::Session { id: 99, name: "other".into() };
        engine.device_leases.acquire(Lease::new("phone-b", other, Kind::Claim, "testing", Some(Duration::from_millis(1))), &mut Vec::new()).unwrap();
        engine.device_leases.acquire(Lease::new("phone-a", Holder::User, Kind::Claim, "testing", Some(Duration::from_secs(60))), &mut Vec::new()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let out = engine.dispatch(Request::new(Actor::User, "device.release", serde_json::json!({})), crate::engine::Door::InProcess)
            .into_result().unwrap();
        let released = out["released"].as_array().unwrap();
        assert_eq!(released.len(), 1, "{out}");
        assert_eq!(released[0]["device"], "phone-a");
        // The other session's lapsed lease is announced, just not claimed as the caller's.
        let mut announced = Vec::new();
        while let Ok(event) = events.try_recv() { if event.ev == RELEASED { announced.push(event.payload["device"].clone()); } }
        announced.sort_by_key(|device| device.to_string());
        assert_eq!(announced, ["phone-a", "phone-b"]);
    }
}
