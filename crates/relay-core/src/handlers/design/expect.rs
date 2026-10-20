//! Expectations in `design.apply` and `audit.undo` (MODEL-SPEC §2.11; feature-agent-api §3.3): what
//! a list's nodes must still hold, the conflicts when they do not, and the expectations an undo
//! keeps. engine-expect's.
//!
//! An expectation compares values, not validity: a field holds what was expected when its JSON,
//! as this build stores it, is what the expected value reads as through a `{kind, key}` probe of
//! [`Node::read`] (so `12` is `12.0` and `width` is `w`). Nothing is validated, so a rule across
//! fields (`runs` against a changed `text`) never makes a conflict out of a field that is as it
//! was.

use super::apply::WALK;
use super::revs::removed_rev;
use super::{kind_of, node_of};
use crate::engine::IntoBus;
use relay_bus::design::{Conflict, ConflictReason, Expect, Node, NodeKind, NodeOp};
use relay_bus::error::BusError;
use relay_bus::ops::design::ApplyIn;
use relay_bus::types::{Design, Id};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Most ids an `inside` conflict names.
const INSIDE_IDS: usize = 20;
/// Most conflicts a `design.stale` refusal lists in `details.conflicts`, and names in its message.
const LISTED: usize = 50;
const NAMED: usize = 5;

const NODE_ROW: &str = "SELECT parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
/// Every node of design `?1` and its parent, from the parent index alone.
const PARENTS: &str = "SELECT node_id, parent_id FROM design_nodes WHERE design_id = ?1";

/// `width` and `height` as the node stores them.
fn canonical(key: &str) -> &str {
    match key {
        "width" => "w",
        "height" => "h",
        other => other,
    }
}

/// Whether `node` (as `stored`, its fields) no longer holds `value` at `key`: a `null` expects the
/// field unset. A value no field of its kind reads is compared as written, as an `extra` key.
fn differs(kind: NodeKind, stored: &Map<String, Value>, key: &str, value: &Value) -> bool {
    let key = canonical(key);
    let now = stored.get(key);
    if value.is_null() {
        return now.is_some();
    }
    let probe = Map::from_iter([("kind".to_string(), json!(kind.name())), (key.to_string(), value.clone())]);
    let want = match Node::read(Value::Object(probe)) {
        Ok(probe) => probe.to_map().remove(key),
        Err(_) => Some(value.clone()),
    };
    want.as_ref() != now
}

/// Every node of the design and its parent, read once: what `within` checks and an undo's
/// `within` are measured by (never one walk per node).
fn parent_map(conn: &Connection, design: Id) -> Result<HashMap<String, Option<String>>, BusError> {
    conn.prepare_cached(PARENTS).bus()?
        .query_map([design], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
        .collect::<rusqlite::Result<_>>().bus()
}

/// The children of each node, from a parent map, in no particular order.
fn children(parents: &HashMap<String, Option<String>>) -> HashMap<String, Vec<String>> {
    let mut kids: HashMap<String, Vec<String>> = HashMap::new();
    for (id, parent) in parents {
        if let Some(parent) = parent {
            kids.entry(parent.clone()).or_default().push(id.clone());
        }
    }
    kids
}

/// What is inside node `id`, every level of it, at most WALK deep, in no particular order.
fn subtree<'a>(kids: &'a HashMap<String, Vec<String>>, id: &'a str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut level = vec![id];
    for _ in 0..WALK {
        let next: Vec<&str> = level.iter().flat_map(|at| kids.get(*at).into_iter().flatten().map(String::as_str)).collect();
        if next.is_empty() {
            break;
        }
        found.extend(&next);
        level = next;
    }
    found
}

/// What has changed since a list's expectations were taken: each `expect` entry the stored
/// design no longer meets and, with `derive_exists`, each add of `ops` whose id is taken by a node
/// the list does not delete first (what an undo would re-add over). Each conflict's `at` is the
/// first edit of `ops` naming its node; an expectation no edit names has none.
pub(crate) fn expect_conflicts(conn: &Connection, design: Id, ops: &[NodeOp], expect: &[Expect], derive_exists: bool)
    -> Result<Vec<Conflict>, BusError> {
    // The first edit naming each node (a colour's, a style's or a copy's source name is not one).
    let mut first: HashMap<&str, u32> = HashMap::new();
    for (at, op) in ops.iter().enumerate() {
        let id = match op {
            NodeOp::Add { node, .. } => node.id.as_str(),
            NodeOp::Update { id, .. } | NodeOp::Move { id, .. } | NodeOp::Delete { id } => id.as_str(),
            _ => continue,
        };
        if !id.is_empty() {
            first.entry(id).or_insert(at as u32);
        }
    }
    let mut conflicts = Vec::new();
    let mut parents: Option<HashMap<String, Option<String>>> = None;
    let mut kids: Option<HashMap<String, Vec<String>>> = None;
    let mut row = conn.prepare_cached(NODE_ROW).bus()?;
    for want in expect {
        let at = first.get(want.id.as_str()).copied();
        let found: Option<(Option<String>, String, f64, String)> = row
            .query_row(params![design, want.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().bus()?;
        let Some((parent, kind, _, body)) = found else {
            let rev = removed_rev(conn, design, &want.id)?;
            conflicts.push(Conflict { rev, ..Conflict::new(at, want.id.clone(), ConflictReason::Missing) });
            continue;
        };
        if let Some(set) = want.set.as_ref().filter(|set| !set.is_empty()) {
            // A node this build cannot read is left to the edit, which skips it as unreadable.
            if let Some(node) = kind_of(&kind).and_then(|_| node_of(want.id.clone(), parent.clone(), &body).ok()) {
                let stored = node.to_map();
                let fields: Vec<String> = set.iter()
                    .filter(|(key, value)| differs(node.kind, &stored, key, value))
                    .map(|(key, _)| canonical(key).to_string())
                    .collect();
                if !fields.is_empty() {
                    conflicts.push(Conflict { fields, ..Conflict::new(at, want.id.clone(), ConflictReason::Fields) });
                }
            }
        }
        if want.parent.as_ref().is_some_and(|want| parent.as_ref() != Some(want)) {
            conflicts.push(Conflict::new(at, want.id.clone(), ConflictReason::Parent));
        }
        if let Some(within) = &want.within {
            // One read of where everything sits, and one map of children, for every `within`.
            if kids.is_none() {
                let map = parents.insert(parent_map(conn, design)?);
                kids = Some(children(map));
            }
            let kids = kids.as_ref().expect("built above");
            let allowed: HashSet<&str> = within.iter().map(String::as_str).collect();
            let mut added: Vec<&str> = subtree(kids, &want.id).into_iter().filter(|id| !allowed.contains(id)).collect();
            if !added.is_empty() {
                added.sort_unstable();
                let ids = added.into_iter().take(INSIDE_IDS).map(str::to_string).collect();
                conflicts.push(Conflict { ids, ..Conflict::new(at, want.id.clone(), ConflictReason::Inside) });
            }
        }
    }
    drop(row);
    if derive_exists {
        // An id is free once an earlier edit deleted it, or a node it sits in.
        let mut deleted: HashSet<String> = HashSet::new();
        let mut taken = conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM design_nodes WHERE design_id = ?1 AND node_id = ?2)").bus()?;
        for (at, op) in ops.iter().enumerate() {
            match op {
                NodeOp::Delete { id } => {
                    if kids.is_none() {
                        let map = parents.insert(parent_map(conn, design)?);
                        kids = Some(children(map));
                    }
                    deleted.extend(subtree(kids.as_ref().expect("built above"), id).into_iter().map(str::to_string));
                    deleted.insert(id.clone());
                }
                NodeOp::Add { node, .. } if !node.id.is_empty() && !deleted.contains(node.id.as_str())
                    && taken.query_row(params![design, node.id], |r| r.get::<_, bool>(0)).bus()? => {
                    conflicts.push(Conflict::new(Some(at as u32), node.id.clone(), ConflictReason::Exists));
                }
                _ => {}
            }
        }
    }
    conflicts.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    Ok(conflicts)
}

/// `design.stale` for a strict list whose expectations the design no longer meets: the first
/// few named with what moved (`"save" (fill)`), the first 50 in `details.conflicts`.
pub(super) fn stale(design: &Design, conflicts: &[Conflict]) -> BusError {
    let mut named: Vec<String> = conflicts.iter().take(NAMED).map(|c| {
        let what = match c.reason {
            ConflictReason::Fields => c.fields.join(", "),
            ConflictReason::Inside => format!("holds {} added since", c.ids.len()),
            reason => serde_json::to_value(reason).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        };
        format!("{} ({what})", c.id)
    }).collect();
    if conflicts.len() > NAMED {
        named.push(format!("and {} more", conflicts.len() - NAMED));
    }
    let mut details = json!({"rev": design.rev, "conflicts": &conflicts[..conflicts.len().min(LISTED)]});
    if conflicts.len() > LISTED {
        details["more"] = json!(conflicts.len() - LISTED);
    }
    BusError::conflict("design.stale", format!("nodes this list expects have changed: {}", named.join(", ")))
        .with_details(details)
        .with_hint("read them again with design.get {\"nodes\": [...]} and send the edits from what they hold now")
}

/// What an undo of a list expects to find, in the order the list first touched each node: each
/// field `updated` at the value the node holds now (`null` where unset), each node `moved` in the
/// parent it sits in now, and each node `added` that holds children and was not added inside
/// another, with the ids inside it now. `nodes` are the touched nodes as stored now. One read of
/// the parent index, and only when an added node can hold children.
pub(super) fn undo_expect(conn: &Connection, design: Id, nodes: &[Node], updated: &HashMap<String, BTreeSet<String>>,
    moved: &HashSet<String>, added: &HashSet<String>) -> Result<Vec<Expect>, BusError> {
    let roots: HashSet<&str> = nodes.iter()
        .filter(|n| added.contains(&n.id) && n.kind.holds_children() && n.parent.as_ref().is_none_or(|p| !added.contains(p)))
        .map(|n| n.id.as_str())
        .collect();
    let parents = if roots.is_empty() { HashMap::new() } else { parent_map(conn, design)? };
    let kids = children(&parents);
    let mut expect = Vec::new();
    for node in nodes {
        let set = updated.get(&node.id).map(|keys| {
            let keys: Map<String, Value> = keys.iter().map(|key| (key.clone(), Value::Null)).collect();
            node.before(&keys)
        });
        let parent = (moved.contains(&node.id) && node.kind != NodeKind::Page).then(|| node.parent.clone()).flatten();
        let within = roots.contains(node.id.as_str()).then(|| {
            let mut ids: Vec<String> = subtree(&kids, &node.id).into_iter().map(str::to_string).collect();
            ids.sort_unstable();
            ids
        });
        if set.is_some() || parent.is_some() || within.is_some() {
            expect.push(Expect { id: node.id.clone(), set, parent, within });
        }
    }
    Ok(expect)
}

/// For `audit.undo` of a `design.apply` row (B1): what the undo `payload` it kept would now
/// overwrite, by the expectations `nodes` it kept with it. A node removed since is not a
/// conflict: the lenient replay skips its edits, as before.
pub(crate) fn undo_conflicts(conn: &Connection, payload: &Value, nodes: &Value) -> Result<Vec<Conflict>, BusError> {
    let undo: ApplyIn = serde_json::from_value(payload.clone())
        .map_err(|why| BusError::internal(format!("audit: a kept design.apply undo does not read: {why}")))?;
    let expect: Vec<Expect> = serde_json::from_value(nodes.clone())
        .map_err(|why| BusError::internal(format!("audit: a kept design.apply expectation does not read: {why}")))?;
    let mut conflicts = expect_conflicts(conn, undo.design_id, &undo.ops, &expect, true)?;
    conflicts.retain(|c| c.reason != ConflictReason::Missing);
    Ok(conflicts)
}
