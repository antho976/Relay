//! The follow rule and the writes `design.apply` normalizes (MODEL-SPEC §2.8, §2.11): a resized
//! frame's pinned children follow it, `move` with `keep_position`, and the B3/B49 fits of what an
//! add writes, as the canvas's Scene runs them. An `impl Run` of engine-pins'.

use super::apply::Run;
use super::{body_of, node_of};
use crate::engine::IntoBus;
use relay_bus::design::{page_point, Layout, Node, NodeKind, NodeOp, MAX_DEPTH, MAX_NODES};
use relay_bus::error::BusError;
use relay_bus::ErrorKind;
use rusqlite::params;
use serde_json::Value;
use std::collections::VecDeque;

/// A frame's children that may be pinned: the text test is loose (a text could hold the words),
/// and each body is read to be sure, as `release_ends` reads its connectors.
const PINNED_CHILDREN: &str = "SELECT node_id, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 AND instr(body, '\"pin_') > 0
    ORDER BY ord, node_id";

/// A frame to follow: its id, its stored size before and after, and how many levels down.
type Resize = (String, (f64, f64), (f64, f64), usize);

impl Run<'_> {
    /// Note what an edit of node `id` sets for the follow rule (feature-layout §2.1): a frame
    /// whose stored size an update changes, at its size before this list first changed it; and
    /// each axis the list set on a node itself (`x`/`w` across, `y`/`h` down), which it then
    /// keeps rather than follows.
    pub(super) fn set_axes(&mut self, id: &str, across: bool, down: bool) {
        if across {
            self.set_here.insert((id.to_string(), true));
        }
        if down {
            self.set_here.insert((id.to_string(), false));
        }
    }

    /// Note frame `n`'s size before an update changes it, once per list.
    pub(super) fn note_resize(&mut self, n: &Node) {
        if n.kind == NodeKind::Frame && !self.resized.iter().any(|(id, _)| *id == n.id) {
            self.resized.push((n.id.clone(), (n.w, n.h)));
        }
    }

    /// The follow rule, once the list's edits have run: each frame whose stored size the list
    /// changed moves its pinned free and absolute children (`Node::follow`) on each axis the list
    /// did not set on them itself, and a child frame that changes size so does the same, level by
    /// level, never more than [`MAX_DEPTH`] levels down (a deeper chain another build left is
    /// left as it is). A worklist, not recursion. Each follow is an update like any other: dated,
    /// counted, undone and expected.
    pub(super) fn follow_pins(&mut self) -> Result<(), BusError> {
        let mut queue: VecDeque<Resize> = VecDeque::new();
        for (frame, old) in std::mem::take(&mut self.resized) {
            if let Some(now) = self.row(&frame)?.and_then(|row| node_of(frame.clone(), row.parent, &row.body).ok()) {
                if (now.w, now.h) != old {
                    queue.push_back((frame, old, (now.w, now.h), 0));
                }
            }
        }
        while let Some((frame, old, new, level)) = queue.pop_front() {
            if level >= MAX_DEPTH {
                continue;
            }
            let Some(row) = self.row(&frame)? else { continue };
            let Ok(holder) = node_of(frame.clone(), row.parent, &row.body) else { continue };
            let laid = holder.layout_mode() != Layout::None;
            let children: Vec<(String, String)> = self.tx.prepare_cached(PINNED_CHILDREN).bus()?
                .query_map(params![self.design, frame], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
                .collect::<rusqlite::Result<_>>().bus()?;
            for (id, body) in children {
                let Ok(child) = node_of(id.clone(), Some(frame.clone()), &body) else { continue };
                if !child.is_pinned() || (laid && !child.is_absolute()) {
                    continue;
                }
                let across = !self.set_here.contains(&(id.clone(), true));
                let down = !self.set_here.contains(&(id.clone(), false));
                let set = child.follow(old, new, across, down);
                if set.is_empty() {
                    continue;
                }
                // One that cannot take it (none should fail) stays where it is.
                let Ok(mut next) = child.patched(&set) else { continue };
                let written = body_of(&mut next)?;
                self.tx.prepare_cached("UPDATE design_nodes SET body = ?3, rev = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
                    .execute(params![self.design, id, written, self.rev]).bus()?;
                self.bytes += written.len() as i64 - body.len() as i64;
                self.touch(&id);
                let before = child.before(&set);
                self.updated.entry(id.clone()).or_default().extend(before.keys().cloned());
                self.inverse.push(vec![NodeOp::Update { id: id.clone(), set: before }]);
                if child.kind == NodeKind::Frame && (next.w, next.h) != (child.w, child.h) {
                    if next.chart.is_some() {
                        self.chart_changed(&id);
                    }
                    queue.push_back((id, (child.w, child.h), (next.w, next.h), level + 1));
                }
            }
        }
        Ok(())
    }

    /// Node `id` and its ancestors as stored (or as this list deleted them), nearest first, up to
    /// its page; bounded by the node cap, as `stored_box` is.
    fn node_chain(&self, id: &str) -> Result<Vec<Node>, BusError> {
        let mut chain = Vec::new();
        let mut at = Some(id.to_string());
        while let Some(id) = at {
            if chain.len() > MAX_NODES {
                break;
            }
            let Some(node) = self.stored(&id)? else { break };
            at = node.parent.clone();
            chain.push(node);
        }
        Ok(chain)
    }

    /// `move` with `keep_position` (feature-layout §2.6), before node `id` (`moving`, as stored)
    /// leaves its parent for `to`: the `x`/`y` that keep it where it is on its page in `to`, or
    /// none when `to` lays it out (its `x`/`y` are unused there). Refused (`design.position`)
    /// when where it is, or where `to`'s children count from, is not stored: a link laid out by
    /// auto layout, or an absolute child pinned in one.
    pub(super) fn kept_position(&self, moving: &Node, to: &str) -> Result<Option<(f64, f64)>, BusError> {
        let dest = self.node_chain(to)?;
        let Some(holder) = dest.first() else { return Ok(None) };
        if holder.layout_mode() != Layout::None {
            if !moving.is_absolute() {
                return Ok(None);
            }
            if moving.is_pinned() {
                return Err(self.position(format!("{:?} is pinned, and {to:?} lays out its children, so where it would be drawn is not stored", moving.id)));
            }
        }
        let here = self.node_chain(&moving.id)?;
        let (ox, oy) = page_point(&here.iter().collect::<Vec<_>>()).map_err(|why| self.position(why))?;
        let (bx, by) = page_point(&dest.iter().collect::<Vec<_>>()).map_err(|why| self.position(why))?;
        Ok(Some((ox - bx, oy - by)))
    }

    fn position(&self, why: String) -> BusError {
        self.fail_on("keep_position", ErrorKind::Invalid, "design.position", format!("keep_position: {why}"))
            .with_hint("move it without keep_position and set x and y in an update")
    }

    /// After a `keep_position` move of `node` (stored as `old`), write `x`/`y` so it shows where
    /// it was: the update's undo goes with the move's.
    pub(super) fn keep_at(&mut self, node: &Node, old: &str, (x, y): (f64, f64)) -> Result<Option<NodeOp>, BusError> {
        let set = serde_json::Map::from_iter([("x".to_string(), Value::from(x)), ("y".to_string(), Value::from(y))]);
        let mut next = node.patched(&set).map_err(|why| self.position(why))?;
        if next.x == node.x && next.y == node.y {
            return Ok(None);
        }
        let written = body_of(&mut next)?;
        self.tx.prepare_cached("UPDATE design_nodes SET body = ?3, rev = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, node.id, written, self.rev]).bus()?;
        self.bytes += written.len() as i64 - old.len() as i64;
        let before = node.before(&set);
        self.updated.entry(node.id.clone()).or_default().extend(before.keys().cloned());
        Ok(Some(NodeOp::Update { id: node.id.clone(), set: before }))
    }
}
