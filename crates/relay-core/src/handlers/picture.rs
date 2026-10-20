//! `design.picture.*` (pictures-SPEC-v2 §5): stock photo search, thumbnails, adding one to a
//! design with its credit, and the Pexels key.
//!
//! Search and thumb are unlocked queries: one short read checks the design, and the request to
//! the provider runs with nothing held. An agent reaches them only where the Pictures plugin is on:
//! `guardrail::authorize` refuses `plugin.off` before the handler runs, so no handler checks it.
//! The person is not gated by the engine.
//!
//! Add and both key ops are staged: add's download and the files it keeps, the check with Pexels
//! and the keyring (`secret-tool`) run in prepare, with nothing locked, and the finish writes
//! nothing. None emits an event: add answers its `src` as design.asset.add does, and only the
//! picker that sent a key op shows the key's state.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::handlers::notes::{assert_actor_project, check_project};
use crate::pictures::{self, key, Added, SearchKey};
use relay_bus::ops::picture::{Add, AddOut, KeyOut, KeyRemove, KeySet, Search, Thumb};
use relay_bus::types::Id;
use relay_bus::BusError;
use rusqlite::{Connection, OptionalExtension};

pub fn register(e: &mut Engine) {
    // Unlocked (D149): the provider may take 8 s to answer.
    e.register_unlocked::<Search>(|ctx, p| {
        let search = SearchKey::of(&p)?;
        check_design(ctx, p.design_id)?;
        pictures::search(ctx.engine(), ctx.actor.is_agent(), search)
    });
    e.register_unlocked::<Thumb>(|ctx, p| {
        let id = pictures::parse_id(&p.id)?;
        check_design(ctx, p.design_id)?;
        pictures::thumb(ctx.engine(), ctx.actor.is_agent(), &id)
    });
    // Staged: the download takes up to 30 s and the picture is synced to disk; the finish only
    // checks the design is still there, as design.asset.add's does. No undo and no event.
    e.register_staged::<Add, Added>(|ctx, p| {
        let id = pictures::parse_id(&p.id)?;
        check_design(ctx, p.design_id)?;
        pictures::add(ctx.engine(), ctx.actor.is_agent(), &id, &ctx.engine().store.design_assets_dir())
    }, |ctx: &mut Ctx, p, added| {
        let project = live_design_project(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, project)?;
        ctx.set_project(project);
        Ok(AddOut { src: added.src, path: added.path.display().to_string(), w: added.w, h: added.h, credit: added.credit })
    });
    e.register_staged::<KeySet, ()>(|ctx, p| key::set(ctx.engine(), &p.key), |_, _, ()| Ok(KeyOut { saved: true }));
    e.register_staged::<KeyRemove, ()>(|ctx, _| key::remove(ctx.engine()), |_, _, ()| Ok(KeyOut { saved: false }));
}

/// The design a picture is for, checked in one short hold of the store before anything is sent:
/// it is there and not deleted, and an agent's is in its own project.
fn check_design(ctx: &Unlocked, design_id: Id) -> Result<(), BusError> {
    let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
    ctx.read(|conn| check_project(conn, live_design_project(conn, design_id)?, session_id, agent))
}

/// The project of design `id`, refused as `design.not_found` or `design.deleted` the way every
/// change to a design is.
fn live_design_project(conn: &Connection, id: Id) -> Result<Id, BusError> {
    let row: Option<(Id, Option<String>)> = conn
        .prepare_cached("SELECT project_id, deleted_at FROM designs WHERE id = ?1").bus()?
        .query_row([id], |r| Ok((r.get(0)?, r.get(1)?))).optional().bus()?;
    match row {
        None => Err(BusError::not_found("design.not_found", format!("no design {id}"))),
        Some((_, Some(_))) => Err(BusError::conflict("design.deleted", format!("design {id} is deleted"))
            .with_hint("design.restore brings it back")),
        Some((project_id, None)) => Ok(project_id),
    }
}
