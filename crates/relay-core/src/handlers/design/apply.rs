//! `design.apply`: a list of node edits run in order against the request's transaction.

use super::{body_of, emit_design, kind_of, live_design, node_of};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{actor_name, assert_actor_project};
use relay_bus::design::{Node, NodeKind, NodeOp, MAX_DESIGN_BYTES, MAX_LENIENT_OPS, MAX_NODES, MAX_OPS};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Apply, ApplyOut};
use relay_bus::types::{Design, Id};
use relay_bus::ErrorKind;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// The largest `design.apply` undo the audit keeps for `audit.undo`.
const UNDO_KEPT: usize = 256 * 1024;
/// Most `warnings` one answer carries.
const MAX_WARNINGS: usize = 20;

/// A row's kind: `Err` holds the name of one this build does not know (a newer build's), whose
/// row is a leaf it can neither read nor edit.
pub(super) type RowKind = Result<NodeKind, String>;

fn row_kind(text: String) -> RowKind {
    kind_of(&text).ok_or(text)
}

/// The refusal of an edit naming node `id`, of a kind this build does not know.
fn unknown_kind(id: &str, kind: &str) -> String {
    format!("node {id:?} is of a kind this Relay cannot read ({kind:?}); open it in a newer Relay")
}

/// A node row: where it sits, what it is, and its JSON.
pub(super) struct Stored {
    pub(super) parent: Option<String>,
    pub(super) kind: RowKind,
    pub(super) ord: f64,
    pub(super) body: String,
}

/// Where edit `at` of a `design.apply` stands, for its refusals: `ops[3] update "hero"`.
fn label(at: usize, op: &NodeOp) -> String {
    match op {
        NodeOp::Add { node, .. } if node.id.is_empty() => format!("ops[{at}] add {}", node.kind.name()),
        NodeOp::Add { node, .. } => format!("ops[{at}] add {:?}", node.id),
        NodeOp::Update { id, .. } => format!("ops[{at}] update {id:?}"),
        NodeOp::Move { id, .. } => format!("ops[{at}] move {id:?}"),
        NodeOp::Delete { id } => format!("ops[{at}] delete {id:?}"),
    }
}

/// One `design.apply` under way: the design's counters as its edits change them, and what the
/// answer reports. Every edit reads and writes the transaction, so the next one sees it.
pub(super) struct Run<'a> {
    pub(super) tx: &'a Connection,
    pub(super) design: Id,
    /// An undo or a redo: a written rule's breach is applied as written, and an edit of a node
    /// that is gone or cannot be read is skipped, where a strict list refuses either.
    pub(super) lenient: bool,
    /// An agent asked: what a lenient list let through is told it in `warnings`.
    pub(super) agent: bool,
    pub(super) warnings: Vec<String>,
    pub(super) pages: i64,
    pub(super) nodes: i64,
    pub(super) bytes: i64,
    pub(super) next_node: i64,
    /// The edit under way: its index and [`label`].
    pub(super) at: usize,
    pub(super) label: String,
    /// Nodes an edit added, changed or moved, in the order first touched.
    pub(super) touched: Vec<String>,
    pub(super) seen: HashSet<String>,
    pub(super) removed: Vec<String>,
    pub(super) ids: Vec<String>,
    /// Each applied edit's undo, in edit order; a delete's is the whole subtree's re-adds.
    pub(super) inverse: Vec<Vec<NodeOp>>,
}

const NODE_META: &str = "SELECT parent_id, kind, ord FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
const NODE_ROW: &str = "SELECT parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
/// Siblings in paint order from `?4`, leaving out `?3` (the node being moved, if any).
const SIBLING_ORDS: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord, node_id LIMIT 2 OFFSET ?4+0";
const TOP_ORD: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord DESC, node_id DESC LIMIT 1";

impl Run<'_> {
    pub(super) fn fail(&self, kind: ErrorKind, code: &str, why: impl std::fmt::Display) -> BusError {
        BusError::new(kind, code, format!("{}: {why}", self.label)).with_details(json!({"op": self.at}))
    }

    /// The node an edit names is not there: skipped when lenient, else refused.
    pub(super) fn missing(&self, what: impl std::fmt::Display) -> Result<Option<String>, BusError> {
        if self.lenient {
            return Ok(None);
        }
        Err(self.fail(ErrorKind::NotFound, "design.node_not_found", what))
    }

    /// Tell an agent what its lenient list let through or skipped (at most [`MAX_WARNINGS`]).
    pub(super) fn warn(&mut self, what: impl std::fmt::Display) {
        if self.agent && self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(format!("{}: {what}", self.label));
        }
    }

    /// A written rule `why` is broken: refused in a strict list; a lenient one (an undo putting
    /// back what was stored) applies it as written and tells an agent.
    pub(super) fn breach(&mut self, why: String) -> Result<(), BusError> {
        if !self.lenient {
            return Err(self.fail(ErrorKind::Invalid, "design.node_invalid", why));
        }
        self.warn(format_args!("{why}; kept as it was"));
        Ok(())
    }

    /// The node row `id`, if this build can read it: an edit naming one it cannot (a kind it
    /// does not know, a body it cannot parse) is refused, or skipped when lenient. Nothing of
    /// such a row is lost: it is left as it is.
    pub(super) fn readable(&mut self, id: &str, row: &Stored) -> Result<Option<Node>, BusError> {
        let why = match &row.kind {
            Err(kind) => unknown_kind(id, kind),
            Ok(_) => match node_of(id.to_string(), row.parent.clone(), &row.body) {
                Ok(node) => return Ok(Some(node)),
                Err(error) => error.message,
            },
        };
        self.unreadable(why)?;
        Ok(None)
    }

    /// An edit cannot touch a node this build cannot read (`why`): refused, or skipped (`Ok`)
    /// when lenient.
    pub(super) fn unreadable(&mut self, why: String) -> Result<(), BusError> {
        if !self.lenient {
            return Err(self.fail(ErrorKind::Invalid, "design.node_unreadable", why));
        }
        self.warn(format_args!("{why}; skipped"));
        Ok(())
    }

    pub(super) fn meta(&self, id: &str) -> Result<Option<(Option<String>, RowKind, f64)>, BusError> {
        let found: Option<(Option<String>, String, f64)> = self.tx.prepare_cached(NODE_META).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().bus()?;
        Ok(found.map(|(parent, kind, ord)| (parent, row_kind(kind), ord)))
    }

    pub(super) fn row(&self, id: &str) -> Result<Option<Stored>, BusError> {
        let found: Option<(Option<String>, String, f64, String)> = self.tx.prepare_cached(NODE_ROW).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().bus()?;
        Ok(found.map(|(parent, kind, ord, body)| Stored { parent, kind: row_kind(kind), ord, body }))
    }

    pub(super) fn touch(&mut self, id: &str) {
        if self.seen.insert(id.to_string()) {
            self.touched.push(id.to_string());
        }
    }

    /// The `ord` that puts a node at `index` among `parent`'s children (the pages, for `None`),
    /// `skip` aside: 0 at the bottom, absent or past the end on top.
    pub(super) fn slot(&self, parent: Option<&str>, index: Option<u32>, skip: Option<&str>) -> Result<f64, BusError> {
        let top = || -> Result<f64, BusError> {
            let last: Option<f64> = self.tx.prepare_cached(TOP_ORD).bus()?
                .query_row(params![self.design, parent, skip], |r| r.get(0)).optional().bus()?;
            Ok(last.map_or(1.0, |ord| ord + 1.0))
        };
        let Some(index) = index else { return top() };
        let around: Vec<f64> = self.tx.prepare_cached(SIBLING_ORDS).bus()?
            .query_map(params![self.design, parent, skip, i64::from(index.saturating_sub(1))], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        match (index, around.as_slice()) {
            (0, []) => Ok(1.0),
            (0, [first, ..]) => Ok(first - 1.0),
            (_, []) => top(),
            (_, [last]) => Ok(last + 1.0),
            (_, &[below, above, ..]) => {
                let mid = below + (above - below) / 2.0;
                if above - below > 1e-9 && below < mid && mid < above {
                    return Ok(mid);
                }
                // The gap is spent: space the siblings 1 apart and look again.
                self.renumber(parent)?;
                self.slot(parent, Some(index), skip)
            }
        }
    }

    pub(super) fn renumber(&self, parent: Option<&str>) -> Result<(), BusError> {
        let ids: Vec<String> = self.tx
            .prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 ORDER BY ord, node_id").bus()?
            .query_map(params![self.design, parent], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let mut set = self.tx.prepare_cached("UPDATE design_nodes SET ord = ?3 WHERE design_id = ?1 AND node_id = ?2").bus()?;
        for (i, id) in ids.iter().enumerate() {
            set.execute(params![self.design, id, (i + 1) as f64]).bus()?;
        }
        Ok(())
    }

    /// Where node `id` (at `ord`) sits among its siblings: how many paint below it.
    pub(super) fn index_of(&self, parent: Option<&str>, id: &str, ord: f64) -> Result<u32, BusError> {
        let below: i64 = self.tx
            .prepare_cached("SELECT COUNT(*) FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2
                AND (ord < ?3 OR (ord = ?3 AND node_id < ?4))").bus()?
            .query_row(params![self.design, parent, ord, id], |r| r.get(0)).bus()?;
        Ok(below as u32)
    }

    /// `{kind}-{n}` from the design's counter, past any id already taken.
    pub(super) fn assign(&mut self, kind: NodeKind) -> Result<String, BusError> {
        loop {
            let id = format!("{}-{}", kind.name(), self.next_node);
            self.next_node += 1;
            if self.meta(&id)?.is_none() {
                return Ok(id);
            }
        }
    }

    /// `parent` can take a child: it is there and is a page, frame or group. `Ok(false)` when
    /// it is gone and the list is lenient. A node of a kind this build cannot read holds
    /// nothing it adds or moves (an undo skips the edit).
    pub(super) fn container(&mut self, parent: &str) -> Result<bool, BusError> {
        match self.meta(parent)? {
            None if self.lenient => Ok(false),
            None => Err(self.fail(ErrorKind::NotFound, "design.parent", format!("parent {parent:?} is not in the design"))),
            Some((_, Ok(kind), _)) if !kind.holds_children() => Err(self.fail(ErrorKind::Invalid, "design.parent",
                format!("parent {parent:?} is a {}, which holds nothing; use a page, frame or group", kind.name()))),
            Some((_, Err(kind), _)) if self.lenient => {
                self.warn(format_args!("parent {parent:?} is of a kind this Relay cannot read ({kind:?}); skipped"));
                Ok(false)
            }
            Some((_, Err(kind), _)) => Err(self.fail(ErrorKind::Invalid, "design.parent",
                format!("parent {parent:?} is of a kind this Relay cannot read ({kind:?}); use a page, frame or group"))),
            Some(_) => Ok(true),
        }
    }

    pub(super) fn add(&mut self, mut node: Node, index: Option<u32>) -> Result<Option<String>, BusError> {
        node.validate().map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        if let Err(why) = node.check_new() {
            self.breach(why)?;
        }
        if let Some(parent) = node.parent.as_deref() {
            if !self.container(parent)? {
                return Ok(None);
            }
        }
        if node.id.is_empty() {
            node.id = self.assign(node.kind)?;
        } else if self.meta(&node.id)?.is_some() {
            return Err(self.fail(ErrorKind::Conflict, "design.node_exists", format!("{:?} is already in the design", node.id))
                .with_hint("update or move that node, or leave the id out to have one assigned"));
        }
        if self.nodes >= MAX_NODES as i64 {
            return Err(self.fail(ErrorKind::Invalid, "design.too_large", format!("a design holds at most {MAX_NODES} nodes")));
        }
        let ord = self.slot(node.parent.as_deref(), index, None)?;
        let body = body_of(&mut node)?;
        self.tx.prepare_cached("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)").bus()?
            .execute(params![self.design, node.id, node.parent, node.kind.name(), ord, body]).bus()?;
        self.nodes += 1;
        self.pages += i64::from(node.kind == NodeKind::Page);
        self.bytes += body.len() as i64;
        self.touch(&node.id);
        self.inverse.push(vec![NodeOp::Delete { id: node.id.clone() }]);
        Ok(Some(node.id))
    }

    pub(super) fn update(&mut self, id: String, set: Map<String, Value>) -> Result<Option<String>, BusError> {
        let Some(row) = self.row(&id)? else { return self.missing("no such node") };
        let Some(node) = self.readable(&id, &row)? else { return Ok(None) };
        let mut next = node.patched(&set).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        if let Err(why) = Node::check_update(&node, &set, &next) {
            self.breach(why)?;
        }
        let body = body_of(&mut next)?;
        if body != row.body {
            self.tx.prepare_cached("UPDATE design_nodes SET body = ?3 WHERE design_id = ?1 AND node_id = ?2").bus()?
                .execute(params![self.design, next.id, body]).bus()?;
            self.bytes += body.len() as i64 - row.body.len() as i64;
        }
        self.touch(&next.id);
        self.inverse.push(vec![NodeOp::Update { id: next.id.clone(), set: node.before(&set) }]);
        Ok(Some(next.id))
    }

    pub(super) fn move_(&mut self, id: String, parent: Option<String>, index: Option<u32>) -> Result<Option<String>, BusError> {
        let Some((from, kind, ord)) = self.meta(&id)? else { return self.missing("no such node") };
        let kind = match kind {
            Ok(kind) => kind,
            Err(kind) => {
                self.unreadable(unknown_kind(&id, &kind))?;
                return Ok(None);
            }
        };
        let to = if kind == NodeKind::Page {
            if parent.is_some() {
                return Err(self.fail(ErrorKind::Invalid, "design.parent", "a page sits in no parent; a move only reorders it among the pages"));
            }
            None
        } else {
            let to = parent.or_else(|| from.clone()).unwrap_or_default();
            if to == id {
                return Err(self.fail(ErrorKind::Invalid, "design.cycle", "a node cannot sit inside itself"));
            }
            if !self.container(&to)? {
                return Ok(None);
            }
            // Into another parent: not one inside the node itself.
            if from.as_deref() != Some(to.as_str()) {
                let mut up = self.meta(&to)?.and_then(|(parent, _, _)| parent);
                while let Some(ancestor) = up {
                    if ancestor == id {
                        return Err(self.fail(ErrorKind::Invalid, "design.cycle", format!("{to:?} is inside {id:?}")));
                    }
                    up = self.meta(&ancestor)?.and_then(|(parent, _, _)| parent);
                }
            }
            Some(to)
        };
        let old_index = self.index_of(from.as_deref(), &id, ord)?;
        let ord = self.slot(to.as_deref(), index, Some(&id))?;
        self.tx.prepare_cached("UPDATE design_nodes SET parent_id = ?3, ord = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, id, to, ord]).bus()?;
        self.touch(&id);
        self.inverse.push(vec![NodeOp::Move { id: id.clone(), parent: from, index: Some(old_index) }]);
        Ok(Some(id))
    }

    pub(super) fn delete(&mut self, id: String) -> Result<Option<String>, BusError> {
        let Some(root) = self.row(&id)? else { return self.missing("no such node") };
        if root.kind == Ok(NodeKind::Page) && self.pages <= 1 {
            return Err(self.fail(ErrorKind::Conflict, "design.last_page", "a design keeps at least one page")
                .with_hint("add another page first, or delete the design"));
        }
        let index = self.index_of(root.parent.as_deref(), &id, root.ord)?;
        // The subtree, each parent before its children and siblings bottom to top: the order
        // its undo adds them back in, so each lands where it was.
        let mut doomed = vec![(id, root)];
        let mut next = 0;
        let mut children = self.tx
            .prepare_cached("SELECT node_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id").bus()?;
        while next < doomed.len() {
            if matches!(doomed[next].1.kind, Ok(kind) if kind.holds_children()) {
                let parent = doomed[next].0.clone();
                let found = children.query_map(params![self.design, parent], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?))).bus()?
                    .collect::<rusqlite::Result<Vec<(String, String, f64, String)>>>().bus()?;
                for (child, kind, ord, body) in found {
                    doomed.push((child, Stored { parent: Some(parent.clone()), kind: row_kind(kind), ord, body }));
                }
            }
            next += 1;
        }
        drop(children);
        // Every node of it is read before any goes: the undo re-adds each, and a node this build
        // cannot read could not be put back, so a subtree holding one is not deleted.
        let mut undo = Vec::with_capacity(doomed.len());
        for (i, (node_id, row)) in doomed.iter().enumerate() {
            let Some(node) = self.readable(node_id, row)? else { return Ok(None) };
            undo.push(NodeOp::Add { node, index: (i == 0).then_some(index) });
        }
        let mut remove = self.tx.prepare_cached("DELETE FROM design_nodes WHERE design_id = ?1 AND node_id = ?2").bus()?;
        for (node_id, row) in doomed {
            remove.execute(params![self.design, node_id]).bus()?;
            self.bytes -= row.body.len() as i64;
            self.removed.push(node_id);
        }
        self.nodes -= undo.len() as i64;
        self.pages -= i64::from(matches!(&undo[0], NodeOp::Add { node, .. } if node.kind == NodeKind::Page));
        let id = undo[0].target().to_string();
        self.inverse.push(undo);
        Ok(Some(id))
    }

    /// Node `id` as stored now, or as an edit of this list deleted it (its undo re-adds it).
    /// A node this build cannot read has no box it can use: `None`.
    pub(super) fn stored(&self, id: &str) -> Result<Option<Node>, BusError> {
        if let Some(row) = self.row(id)? {
            return Ok(node_of(id.to_string(), row.parent, &row.body).ok());
        }
        Ok(self.inverse.iter().rev().flatten().find_map(|op| match op {
            NodeOp::Add { node, .. } if node.id == id => Some(node.clone()),
            _ => None,
        }))
    }

    /// The page box of node `id` from the stored `x`/`y` up its parents (no auto layout).
    pub(super) fn stored_box(&self, id: &str) -> Result<Option<(f64, f64, f64, f64)>, BusError> {
        let Some(node) = self.stored(id)? else { return Ok(None) };
        let (w, h) = (node.w, node.h);
        let (mut x, mut y, mut up) = (node.x, node.y, node.parent);
        for _ in 0..MAX_NODES {
            let Some(parent) = up else { return Ok(Some((x, y, w, h))) };
            let Some(p) = self.stored(&parent)? else { return Ok(None) };
            (x, y, up) = (x + p.x, y + p.y, p.parent);
        }
        Ok(None)
    }

    /// Connectors attached to a node this list deleted let go of that end, so no line names a
    /// node that is gone. A line keeps its own points (where it was last drawn); one with none
    /// is given its ends' stored centres. The undo attaches it again.
    pub(super) fn release_ends(&mut self) -> Result<(), BusError> {
        if self.removed.is_empty() {
            return Ok(());
        }
        let removed: HashSet<&str> = self.removed.iter().map(String::as_str).collect();
        // The text test is loose (a text could hold the words); each body is read to be sure.
        let attached: Vec<(String, Option<String>, String)> = self.tx
            .prepare_cached("SELECT node_id, parent_id, body FROM design_nodes WHERE design_id = ?1
                AND (instr(body, '\"from\":') > 0 OR instr(body, '\"to\":') > 0)").bus()?
            .query_map([self.design], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let mut releases = Vec::new();
        for (id, parent, body) in attached {
            // A row this build cannot read is left as it is (a newer build's own kind of line).
            let node = match node_of(id, parent, &body) {
                Ok(node) => node,
                Err(error) => {
                    tracing::warn!(design = self.design, "design.apply: {}; its ends are left as they are", error.message);
                    continue;
                }
            };
            let mut set = Map::new();
            for (field, end) in [("from", &node.from), ("to", &node.to)] {
                if let Some(end) = end.as_deref().filter(|e| removed.contains(e)) {
                    if self.meta(end)?.is_none() {
                        set.insert(field.to_string(), Value::Null);
                    }
                }
            }
            if !set.is_empty() {
                releases.push((node, set, body.len() as i64));
            }
        }
        drop(removed);
        for (node, mut set, old_len) in releases {
            if node.points.as_ref().is_none_or(|p| p.len() < 2) {
                let centre = |b: Option<(f64, f64, f64, f64)>| b.map(|(x, y, w, h)| (x + w / 2.0, y + h / 2.0));
                let ends = match node.from.as_deref().zip(node.to.as_deref()) {
                    Some((from, to)) => (self.stored_box(&node.id)?, centre(self.stored_box(from)?), centre(self.stored_box(to)?)),
                    None => (None, None, None),
                };
                let points = match ends {
                    (Some((ox, oy, _, _)), Some(a), Some(b)) => json!([[(a.0 - ox).round(), (a.1 - oy).round()], [(b.0 - ox).round(), (b.1 - oy).round()]]),
                    _ => json!([[0.0, 0.0], [node.w, node.h]]),
                };
                set.insert("points".into(), points);
            }
            // A line that cannot be freed (none should fail) keeps its attachment.
            let Ok(mut next) = node.patched(&set) else { continue };
            let body = body_of(&mut next)?;
            self.tx.prepare_cached("UPDATE design_nodes SET body = ?3 WHERE design_id = ?1 AND node_id = ?2").bus()?
                .execute(params![self.design, next.id, body]).bus()?;
            self.bytes += body.len() as i64 - old_len;
            self.touch(&next.id);
            self.inverse.push(vec![NodeOp::Update { id: next.id.clone(), set: node.before(&set) }]);
        }
        Ok(())
    }
}

pub(super) fn register(e: &mut Engine) {
    e.register::<Apply>(|ctx: &mut Ctx, p| {
        // An undo re-adds every node a delete took, one edit each, so a lenient list of the
        // person's (an undo or a redo) may hold a whole design's worth besides a list's. An
        // agent's lists, lenient or not, stay within MAX_OPS.
        let (lenient, agent) = (p.lenient.unwrap_or(false), ctx.actor.is_agent());
        let most = if lenient && !agent { MAX_LENIENT_OPS } else { MAX_OPS };
        if p.ops.is_empty() || p.ops.len() > most {
            return Err(BusError::invalid("design.ops", format!("give 1 to {most} edits; this list has {}", p.ops.len())));
        }
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        if p.base_rev.is_some_and(|rev| rev != design.rev) {
            return Err(BusError::conflict("design.stale", format!("design {} is at rev {}, not {}", design.id, design.rev, p.base_rev.unwrap_or_default()))
                .with_details(json!({"rev": design.rev}))
                .with_hint("read it again with design.get and redo the edits against what is there"));
        }
        let by = actor_name(ctx)?;
        let (bytes, next_node): (i64, i64) = ctx.tx().prepare_cached("SELECT bytes, next_node FROM designs WHERE id = ?1").bus()?
            .query_row([design.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?;
        let mut run = Run {
            tx: ctx.tx(),
            design: design.id,
            lenient,
            agent,
            warnings: Vec::new(),
            pages: design.pages,
            nodes: design.nodes,
            bytes,
            next_node,
            at: 0,
            label: String::new(),
            touched: Vec::new(),
            seen: HashSet::new(),
            removed: Vec::new(),
            ids: Vec::with_capacity(p.ops.len()),
            inverse: Vec::with_capacity(p.ops.len()),
        };
        for (at, op) in p.ops.into_iter().enumerate() {
            (run.at, run.label) = (at, label(at, &op));
            let id = match op {
                NodeOp::Add { node, index } => run.add(node, index)?,
                NodeOp::Update { id, set } => run.update(id, set)?,
                NodeOp::Move { id, parent, index } => run.move_(id, parent, index)?,
                NodeOp::Delete { id } => run.delete(id)?,
            };
            run.ids.push(id.unwrap_or_default());
        }
        run.release_ends()?;
        if run.bytes > MAX_DESIGN_BYTES {
            return Err(BusError::invalid("design.too_large",
                format!("the design would hold {} KiB of nodes; the limit is {} KiB", run.bytes >> 10, MAX_DESIGN_BYTES >> 10))
                .with_hint("split it across designs, or keep long text and big paths out of it"));
        }
        ctx.tx().prepare_cached(
            "UPDATE designs SET rev = rev + 1, pages = ?2, nodes = ?3, bytes = ?4, next_node = ?5, updated_by = ?6, updated_at = ?7 WHERE id = ?1",
        ).bus()?.execute(params![design.id, run.pages, run.nodes, run.bytes, run.next_node, by, ctx.now]).bus()?;
        let mut nodes = Vec::with_capacity(run.touched.len());
        for id in std::mem::take(&mut run.touched) {
            if let Some(row) = run.row(&id)? {
                nodes.push(node_of(id, row.parent, &row.body)?);
            }
        }
        // What is gone now: a node deleted and added back in the same list is in `nodes`.
        let mut gone: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let removed: Vec<String> = std::mem::take(&mut run.removed).into_iter()
            .filter(|id| gone.insert(id.clone()))
            .collect();
        let inverse: Vec<NodeOp> = std::mem::take(&mut run.inverse).into_iter().rev().flatten().collect();
        let ids = std::mem::take(&mut run.ids);
        let warnings = std::mem::take(&mut run.warnings);
        let (pages, count) = (run.pages, run.nodes);
        let prev_rev = design.rev;
        let design = Design { rev: prev_rev + 1, pages, nodes: count, updated_by: by, updated_at: ctx.now.clone(), ..design };
        // The audit keeps the undo uncapped (payloads and results stop at 64 KiB): a big delete's
        // would store every node it took. Past UNDO_KEPT only the caller's copy, in the answer,
        // can undo it.
        let undo = json!({"design_id": design.id, "ops": inverse, "lenient": true});
        if serde_json::to_vec(&undo).is_ok_and(|bytes| bytes.len() <= UNDO_KEPT) {
            ctx.set_undo("design.apply", undo, None);
        }
        emit_design(ctx, &design)?;
        Ok(ApplyOut { design, prev_rev, nodes, removed, ids, inverse, warnings })
    });
}
