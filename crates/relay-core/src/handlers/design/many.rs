//! `update_many` and `delete_many`: one edit of many nodes, picked by id and by a `where` that
//! finds them as design.find does. `design.apply` turns each into an edit a node before it runs
//! the list, so every rule an `update` or a `delete` keeps holds for each node, the stale checks
//! see the nodes themselves, and a refusal names the node at fault under the edit's own index.

use super::find::Query;
use relay_bus::design::{NodeOp, Where};
use relay_bus::error::BusError;
use relay_bus::types::Id;
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A list with each edit of many nodes made an edit a node.
pub(super) struct Expanded {
    pub ops: Vec<NodeOp>,
    /// For each edit of `ops`, the index of the edit it came from in the list as sent.
    pub origin: Vec<usize>,
    /// How many edits were sent.
    pub sent: usize,
    /// Each edit of many nodes, by its index as sent: how many nodes it picked.
    pub matched: BTreeMap<String, u32>,
    /// A `where` that found nothing, said.
    pub warnings: Vec<String>,
}

impl Expanded {
    /// The reply's `ids`, one an edit as sent: an edit of many nodes gives its first, "" for
    /// none, every other edit its own.
    pub fn collapse(&self, ids: Vec<String>) -> Vec<String> {
        let mut out = vec![String::new(); self.sent];
        for (id, &at) in ids.into_iter().zip(&self.origin).rev() {
            out[at] = id;
        }
        out
    }
}

/// `ops` with each `update_many` and `delete_many` made an `update` or `delete` of each node it
/// picks: those its `ids` names, then those its `where` finds in the design as stored, each
/// once. A delete_many leaves out a node inside another it deletes, which goes with it. A list
/// without one comes back as it was.
pub(super) fn expand(conn: &Connection, design: Id, ops: Vec<NodeOp>) -> Result<Expanded, BusError> {
    let sent = ops.len();
    if !ops.iter().any(|op| matches!(op, NodeOp::UpdateMany { .. } | NodeOp::DeleteMany { .. })) {
        return Ok(Expanded { ops, origin: (0..sent).collect(), sent, matched: BTreeMap::new(), warnings: Vec::new() });
    }
    // The design's rows, read once for every `where` of the list, and its parents for a delete.
    let mut rows: Option<(Vec<super::get::Raw>, String)> = None;
    let mut parents: Option<HashMap<String, Option<String>>> = None;
    let mut out = Expanded { ops: Vec::with_capacity(sent), origin: Vec::with_capacity(sent), sent, matched: BTreeMap::new(), warnings: Vec::new() };
    for (at, op) in ops.into_iter().enumerate() {
        let (ids, where_, set, delete) = match op {
            NodeOp::UpdateMany { ids, where_, set } => (ids, where_, Some(set), false),
            NodeOp::DeleteMany { ids, where_ } => (ids, where_, None, true),
            op => {
                out.ops.push(op);
                out.origin.push(at);
                continue;
            }
        };
        let name = if delete { "delete_many" } else { "update_many" };
        if ids.is_none() && where_.is_none() {
            return Err(many_invalid(at, name, "give ids, where, or both: the nodes it acts on"));
        }
        let mut picked: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for id in ids.into_iter().flatten() {
            if seen.insert(id.clone()) {
                picked.push(id);
            }
        }
        if let Some(w) = &where_ {
            let found = found(conn, design, w, &mut rows).map_err(|mut e| {
                e.message = format!("ops[{at}] {name}: where: {}", e.message);
                e
            })?;
            if found.is_empty() {
                out.warnings.push(format!("ops[{at}] {name}: where finds no node, so it changed nothing; design.find with the same filters lists what they find"));
            }
            for id in found {
                if seen.insert(id.clone()) {
                    picked.push(id);
                }
            }
        }
        if delete {
            let parents = match &mut parents {
                Some(parents) => parents,
                None => parents.insert(parent_map(conn, design)?),
            };
            let all: HashSet<&str> = picked.iter().map(String::as_str).collect();
            let inside = |id: &str| {
                let mut at = parents.get(id).cloned().flatten();
                for _ in 0..=parents.len() {
                    match at {
                        Some(up) if all.contains(up.as_str()) => return true,
                        Some(up) => at = parents.get(&up).cloned().flatten(),
                        None => return false,
                    }
                }
                false
            };
            let outer: Vec<String> = picked.iter().filter(|id| !inside(id)).cloned().collect();
            picked = outer;
        }
        out.matched.insert(at.to_string(), picked.len() as u32);
        for id in picked {
            out.ops.push(match &set {
                Some(set) => NodeOp::Update { id, set: set.clone() },
                None => NodeOp::Delete { id },
            });
            out.origin.push(at);
        }
    }
    Ok(out)
}

/// The ids `w` finds in `design`, its rows read the first time a list asks.
fn found(conn: &Connection, design: Id, w: &Where, rows: &mut Option<(Vec<super::get::Raw>, String)>) -> Result<Vec<String>, BusError> {
    if w.is_empty() {
        return Err(BusError::invalid("design.where", "name at least one filter (page, inside, kind, name, text, a colour or font): an empty where would find every node"));
    }
    let query = Query::new(w)?;
    let (raw, styles) = match rows {
        Some(read) => read,
        None => rows.insert(Query::rows(conn, design)?),
    };
    // A query moves the rows it reads: each `where` reads a copy.
    Ok(query.run(design, raw.clone(), styles)?.into_iter().map(|n| n.id).collect())
}

fn parent_map(conn: &Connection, design: Id) -> Result<HashMap<String, Option<String>>, BusError> {
    use crate::engine::IntoBus;
    conn.prepare_cached("SELECT node_id, parent_id FROM design_nodes WHERE design_id = ?1").bus()?
        .query_map([design], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
        .collect::<rusqlite::Result<_>>().bus()
}

fn many_invalid(at: usize, name: &str, why: &str) -> BusError {
    BusError::invalid("design.node_invalid", format!("ops[{at}] {name}: {why}"))
}
