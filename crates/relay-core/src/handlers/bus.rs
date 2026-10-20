//! `bus.*` (BUS.md §10.1). `bus.wait`/`subscribe`/`unsubscribe` are answered by the socket
//! door; if they reach the engine (in-process) they say so. The door dispatches them first all
//! the same, for the pipeline's envelope, actor and authorization checks: [`DOOR_ONLY`] coming
//! back means every one of them passed.

use crate::engine::{Engine, IntoBus};
use crate::sessions;
use relay_bus::error::BusError;
use relay_bus::ops::bus::*;
use relay_bus::registry::{Callable, OpInfo, Registry};
use relay_bus::types::Session;
use relay_bus::Op;
use rusqlite::OptionalExtension;

pub fn register(e: &mut Engine) {
    e.register::<Ping>(|ctx, _| {
        Ok(Pong {
            pong: true,
            instance: ctx.instance().as_str().to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_s: ctx.engine().uptime_s(),
        })
    });
    e.register::<Schema>(|_, p| {
        let schema = match p.op {
            None => relay_bus::schema::render(),
            Some(op) => relay_bus::schema::render_op(&op).ok_or_else(|| BusError::unknown_op(&op))?,
        };
        Ok(SchemaOut { schema })
    });
    e.register::<Ops>(|ctx, p| {
        // Every row carries the all-layers verdict. Reporting only the registry layer made
        // this tool overstate a builder's write surface tenfold, which is worse than having
        // no discovery at all: the refusal moved from planning time to execution time (D104).
        let actor = p.actor.unwrap_or_else(|| ctx.actor.clone());
        let session: Option<Session> = match (actor == ctx.actor).then(|| ctx.actor_session_id()).flatten() {
            Some(id) => sessions::by_id(ctx.tx(), id)?.map(|row| row.session),
            None => match actor.session_name() {
                Some(name) => sessions::by_name(ctx.tx(), name).ok().map(|row| row.session),
                None => None,
            },
        };
        let cfg = match &session {
            Some(session) => Some(crate::guardrail::config(ctx.tx(), Some(session.project_id))?),
            None => None,
        };
        let ops = Registry::global()
            .entries()
            .iter()
            .filter(|e| e.meta.actors.admits(&actor))
            .map(|e| {
                let (call, why) =
                    crate::guardrail::callability(e, &actor, session.as_ref(), cfg.as_ref());
                OpInfo::from_entry(e, ctx.engine().is_implemented(e.name)).with_call(call, why)
            })
            .collect();
        Ok(OpsOut { ops })
    });
    e.register::<Whoami>(|ctx, _| {
        // The same three-layer answer `bus.ops` gives, reduced to the question an actor asks
        // on arrival. Unlike `session.bootstrap` this works for the user too (D119).
        let session = match ctx.actor_session_id() {
            Some(id) => sessions::by_id(ctx.tx(), id)?.map(|row| row.session),
            None => None,
        };
        let cfg = match &session {
            Some(session) => Some(crate::guardrail::config(ctx.tx(), Some(session.project_id))?),
            None => None,
        };
        let project = match &session {
            Some(session) => ctx.tx().query_row(
                "SELECT name FROM projects WHERE id=?1",
                [session.project_id],
                |row| row.get::<_, String>(0),
            ).optional().bus()?,
            None => None,
        };
        let can_call = Registry::global()
            .entries()
            .iter()
            .filter(|entry| ctx.engine().is_implemented(entry.name))
            .filter(|entry| {
                crate::guardrail::callability(entry, &ctx.actor, session.as_ref(), cfg.as_ref()).0
                    != Callable::No
            })
            .map(|entry| entry.name.to_string())
            .collect();
        let write_roots = match (&session, &cfg) {
            (Some(session), Some(cfg)) => {
                crate::guardrail::write_roots(cfg, std::path::Path::new(&session.worktree))
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect()
            }
            _ => Vec::new(),
        };
        Ok(WhoamiOut {
            actor: ctx.actor.to_string(),
            is_agent: ctx.actor.is_agent(),
            session: session.as_ref().map(|s| s.name.clone()),
            role: session.as_ref().map(|s| s.role),
            project_id: session.as_ref().map(|s| s.project_id),
            project,
            worktree: session.as_ref().map(|s| s.worktree.clone()),
            branch: session.as_ref().map(|s| s.branch.clone()),
            can_call,
            write_roots,
        })
    });
    e.register::<Wait>(|_, _| Err(door_only(Wait::NAME)));
    e.register::<Subscribe>(|_, _| Err(door_only(Subscribe::NAME)));
    e.register::<Unsubscribe>(|_, _| Err(door_only(Unsubscribe::NAME)));
}

/// The code of the refusal a socket-door op's handler gives.
pub(crate) const DOOR_ONLY: &str = "bus.door";

pub(crate) fn door_only(op: &str) -> BusError {
    BusError::invalid(DOOR_ONLY, format!("{op} is answered by the socket door, not the engine"))
}
