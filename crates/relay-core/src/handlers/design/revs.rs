//! Revisions (feature-agent-api W20; MODEL-SPEC §4): what the store's removals say about an id.
//!
//! Every node row carries the rev of the list that last wrote it, and every id a delete took is
//! on record with the rev it went at (`design_removed`, kept for [`SINCE_WINDOW`] revs).

use crate::engine::IntoBus;
use relay_bus::design::SINCE_WINDOW;
use relay_bus::error::BusError;
use relay_bus::types::Id;
use rusqlite::{params, Connection, OptionalExtension};

/// The rev node `id` of design `design` was removed at, while that is on record (the last
/// [`SINCE_WINDOW`] revs): for a refusal that names an id the design no longer holds (W19).
pub(super) fn removed_rev(conn: &Connection, design: Id, id: &str) -> Result<Option<i64>, BusError> {
    conn.prepare_cached("SELECT rev FROM design_removed WHERE design_id = ?1 AND node_id = ?2").bus()?
        .query_row(params![design, id], |r| r.get(0)).optional().bus()
}

/// Forget the removals no `since_rev` can reach any more, once a list at `rev` removed something:
/// a read may reach back [`SINCE_WINDOW`] revs from the design's rev, which is at least `rev`.
pub(super) fn prune(tx: &Connection, design: Id, rev: i64) -> Result<(), BusError> {
    tx.prepare_cached("DELETE FROM design_removed WHERE design_id = ?1 AND rev < ?2").bus()?
        .execute(params![design, rev - SINCE_WINDOW]).bus()?;
    Ok(())
}
