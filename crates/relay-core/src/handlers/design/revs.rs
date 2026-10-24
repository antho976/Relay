//! Revisions (feature-agent-api W6, W19, W20; MODEL-SPEC §3.2, §4): what a `design.apply` sent
//! against an older rev may still do, and what the store's removals say about an id.
//!
//! Every node row carries the rev of the list that last wrote it, and every id a delete took is
//! on record with the rev it went at (`design_removed`, kept for [`SINCE_WINDOW`] revs). A list
//! sent with an older `base_rev` is refused only when a node it writes changed since; anything
//! else it applies on top of.

use super::apply::WALK;
use crate::engine::IntoBus;
use relay_bus::design::{copy_id_renamed, renamed, Conflict, ConflictReason, NodeOp, MAX_DEPTH, SINCE_WINDOW};
use relay_bus::error::BusError;
use relay_bus::types::{Design, Id};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use std::collections::{HashMap, HashSet};

/// Most conflicts a `design.stale` refusal lists in `details.conflicts`; `details.more` counts
/// the rest.
const LISTED: usize = 50;
/// Most conflicts its message names.
const NAMED: usize = 5;
/// Most ids an `inside` conflict names.
const INSIDE_IDS: usize = 20;

/// The rows of design `?1` written after rev `?2`, and the rev each was: through `design_nodes_rev`.
const CHANGED: &str = "SELECT node_id, rev FROM design_nodes WHERE design_id = ?1 AND rev > ?2";
/// The ids of design `?1` removed after rev `?2`, and the rev each went at.
const GONE: &str = "SELECT node_id, rev FROM design_removed WHERE design_id = ?1 AND rev > ?2";
/// Node `?2` of design `?1` and every node inside it, as far as `?3` levels down, through the
/// parent index: the ids a copy of it makes.
const SUBTREE_IDS: &str = "WITH RECURSIVE down(node_id, n) AS (
        SELECT ?2, 0
        UNION ALL
        SELECT d.node_id, down.n + 1 FROM down CROSS JOIN design_nodes d ON d.design_id = ?1 AND d.parent_id = down.node_id
        WHERE down.n < ?3)
    SELECT node_id FROM down";
/// Every node of design `?1` and its parent, from the parent index alone.
const PARENTS: &str = "SELECT node_id, parent_id FROM design_nodes WHERE design_id = ?1";

/// W6: what has changed since `base` among the nodes `ops` writes. Each update, move or delete
/// names a node the list writes, unless an earlier edit of the list added it: one written after
/// `base` is `changed` (at the rev it was), one removed after it `missing` (at the rev it went),
/// and a node a delete takes that holds one written after `base` is `inside` (naming up to 20 of
/// them, which the delete would take with it). One conflict per node, `at` the first edit that
/// writes it, in edit order. Named colours and styles never conflict.
///
/// One indexed read of the rows written since, one of the removals since, and only when a delete
/// meets a change, one read of where every node sits (the parent index alone).
pub(super) fn base_conflicts(tx: &Connection, design: Id, ops: &[NodeOp], base: i64) -> Result<Vec<Conflict>, BusError> {
    let mut own: HashSet<String> = HashSet::new();
    let mut written: Vec<(usize, &str)> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut deletes: HashMap<&str, usize> = HashMap::new();
    for (at, op) in ops.iter().enumerate() {
        match op {
            NodeOp::Add { node, .. } if !node.id.is_empty() => {
                own.insert(node.id.clone());
            }
            NodeOp::Copy { id: from, as_id, rename, .. } => {
                // Its root, the one `as` or a rename names, and the ids it makes inside it.
                let Some(root) = as_id.clone().or_else(|| renamed(from, rename.as_ref())) else { continue };
                let inside: Vec<String> = tx.prepare_cached(SUBTREE_IDS).bus()?
                    .query_map(params![design, from, MAX_DEPTH as i64], |r| r.get(0)).bus()?
                    .collect::<rusqlite::Result<_>>().bus()?;
                own.extend(inside.iter().filter_map(|id| copy_id_renamed(from, &root, id, rename.as_ref())));
                own.insert(root);
            }
            NodeOp::Update { id, .. } | NodeOp::Move { id, .. } | NodeOp::Delete { id } if !own.contains(id.as_str()) => {
                if seen.insert(id) {
                    written.push((at, id.as_str()));
                }
                if matches!(op, NodeOp::Delete { .. }) {
                    deletes.entry(id).or_insert(at);
                }
            }
            _ => {}
        }
    }
    if written.is_empty() {
        return Ok(Vec::new());
    }
    let changed: HashMap<String, i64> = tx.prepare_cached(CHANGED).bus()?
        .query_map(params![design, base], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
        .collect::<rusqlite::Result<_>>().bus()?;
    let gone: HashMap<String, i64> = tx.prepare_cached(GONE).bus()?
        .query_map(params![design, base], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
        .collect::<rusqlite::Result<_>>().bus()?;
    let mut conflicts = Vec::new();
    let mut flagged: HashSet<&str> = HashSet::new();
    for &(at, id) in &written {
        let (reason, rev) = match (changed.get(id), gone.get(id)) {
            (Some(&rev), _) => (ConflictReason::Changed, rev),
            (None, Some(&rev)) => (ConflictReason::Missing, rev),
            (None, None) => continue,
        };
        flagged.insert(id);
        conflicts.push(Conflict { rev: Some(rev), ..Conflict::new(Some(at as u32), id, reason) });
    }
    // A delete takes what is inside it: a node written since inside one is a change it would undo.
    let open: Vec<(&str, usize)> = deletes.iter().filter(|(id, _)| !flagged.contains(**id)).map(|(id, at)| (*id, *at)).collect();
    if !open.is_empty() && !changed.is_empty() {
        let parents: HashMap<String, Option<String>> = tx.prepare_cached(PARENTS).bus()?
            .query_map([design], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let open: HashMap<&str, usize> = open.into_iter().collect();
        let mut inside: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut changed_ids: Vec<&str> = changed.keys().map(String::as_str).collect();
        changed_ids.sort_unstable();
        for id in changed_ids {
            let mut up = parents.get(id).and_then(Option::as_deref);
            for _ in 0..WALK {
                let Some(parent) = up else { break };
                if let Some((&d, _)) = open.get_key_value(parent) {
                    inside.entry(d).or_default().push(id);
                }
                up = parents.get(parent).and_then(Option::as_deref);
            }
        }
        for (d, ids) in inside {
            let mut conflict = Conflict::new(Some(open[d] as u32), d, ConflictReason::Inside);
            conflict.ids = ids.into_iter().take(INSIDE_IDS).map(str::to_string).collect();
            conflicts.push(conflict);
        }
    }
    conflicts.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    Ok(conflicts)
}

/// `design.stale` for a list sent against `base`, behind `design`, whose `conflicts` say what
/// it would have overwritten: the first few named, the first 50 in `details`, and how to redo it.
pub(super) fn stale(design: &Design, base: i64, conflicts: &[Conflict]) -> BusError {
    let mut named: Vec<String> = conflicts.iter().take(NAMED).map(described).collect();
    if conflicts.len() > NAMED {
        named.push(format!("and {} more", conflicts.len() - NAMED));
    }
    let message = format!("design {} is at rev {}; since rev {base} other edits changed what this list writes: {}",
        design.id, design.rev, named.join(", "));
    let hint = if base >= design.rev - SINCE_WINDOW {
        format!("read what changed with design.get {{\"design_id\": {}, \"since_rev\": {base}}}, redo the edits that touch it, and send them with base_rev {}",
            design.id, design.rev)
    } else {
        format!("read it again with design.get, redo the edits that touch what changed, and send them with base_rev {}", design.rev)
    };
    let mut details = json!({"rev": design.rev, "base_rev": base, "conflicts": &conflicts[..conflicts.len().min(LISTED)]});
    if conflicts.len() > LISTED {
        details["more"] = json!(conflicts.len() - LISTED);
    }
    BusError::conflict("design.stale", message).with_details(details).with_hint(hint)
}

/// One conflict as a refusal names it: `"card" (changed at rev 7)`.
fn described(conflict: &Conflict) -> String {
    let id = &conflict.id;
    let rev = conflict.rev.unwrap_or_default();
    match (conflict.reason, conflict.ids.as_slice()) {
        (ConflictReason::Changed, _) => format!("{id:?} (changed at rev {rev})"),
        (ConflictReason::Missing, _) => format!("{id:?} (removed at rev {rev})"),
        (ConflictReason::Inside, [one]) => format!("{id:?} (holds {one:?}, changed since)"),
        (ConflictReason::Inside, ids) => format!("{id:?} (holds {} nodes changed since)", ids.len()),
        (reason, _) => format!("{id:?} ({})", serde_json::to_value(reason).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()),
    }
}

/// `design.stale` for a `base_rev` the design has not reached: no read gave it.
pub(super) fn ahead(design: &Design, base: i64) -> BusError {
    BusError::conflict("design.stale", format!("design {} is at rev {}; base_rev {base} is ahead of it", design.id, design.rev))
        .with_details(json!({"rev": design.rev, "base_rev": base}))
        .with_hint("send the rev design.get or the last design.apply answered with")
}

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
