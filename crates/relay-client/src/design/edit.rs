//! The `design.apply` edits behind the canvas's commands: duplicate, paste, group and frame
//! the selection, ungroup, align and distribute, reorder, nudge, and put a node in another
//! parent without it jumping. Each returns the edits; the canvas runs them on its scene and
//! sends them as one list, so one undo reverts the whole command.

use super::geom::Rect;
use super::layout::Placed;
use super::scene::Scene;
use relay_bus::design::{Layout, Node, NodeKind, NodeOp};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

/// A position for a node: whole pixels, as designs are drawn.
pub fn px(v: f64) -> Value {
    json!(v.round())
}

/// The selected nodes without those inside another selected one, in paint order.
pub fn outermost(scene: &Scene, ids: &[String]) -> Vec<String> {
    let chosen: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut out: Vec<String> = ids
        .iter()
        .filter(|id| scene.node(id).is_some())
        .filter(|id| !scene.ancestors(id).iter().any(|a| chosen.contains(a)))
        .cloned()
        .collect();
    out.sort_by_key(|id| paint_key(scene, id));
    out.dedup();
    out
}

/// Sorts nodes as they paint: by the index path from the page down.
fn paint_key(scene: &Scene, id: &str) -> Vec<usize> {
    let mut path: Vec<usize> = std::iter::once(id)
        .chain(scene.ancestors(id))
        .map(|n| scene.index(n).unwrap_or(0))
        .collect();
    path.reverse();
    path
}

/// Whether `parent` places its children itself (auto layout), so their `x`/`y` mean nothing.
pub fn lays_out(scene: &Scene, parent: &str) -> bool {
    scene.node(parent).is_some_and(|p| p.layout_mode() != Layout::None)
}

/// Copies of `nodes` (whole subtrees, as `copy` gathered them) under fresh ids: roots go into
/// `parent` at `index`, moved by `(dx, dy)`. Returns the edits and the new roots.
pub fn clone_into(
    scene: &mut Scene,
    nodes: &[Node],
    parent: &str,
    index: Option<u32>,
    dx: f64,
    dy: f64,
) -> (Vec<NodeOp>, Vec<String>) {
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut fresh: HashMap<String, String> = HashMap::new();
    for n in nodes {
        fresh.insert(n.id.clone(), scene.fresh_id(n.kind));
    }
    let mut ops = Vec::new();
    let mut roots = Vec::new();
    let mut at = index;
    for n in nodes {
        if n.kind == NodeKind::Page {
            continue;
        }
        let mut copy = n.clone();
        copy.id = fresh[&n.id].clone();
        let root = n.parent.as_deref().is_none_or(|p| !ids.contains(p));
        if root {
            copy.parent = Some(parent.to_string());
            copy.x = (copy.x + dx).round();
            copy.y = (copy.y + dy).round();
            roots.push(copy.id.clone());
            ops.push(NodeOp::Add { node: copy, index: at });
            at = at.map(|i| i + 1);
        } else {
            copy.parent = n.parent.as_ref().map(|p| fresh[p].clone());
            ops.push(NodeOp::Add { node: copy, index: None });
        }
    }
    (ops, roots)
}

/// Every node of the selection's subtrees, parents first: what Copy puts on the clipboard.
pub fn gather(scene: &Scene, ids: &[String]) -> Vec<Node> {
    outermost(scene, ids)
        .iter()
        .flat_map(|id| scene.subtree(id))
        .filter_map(|id| scene.node(&id).cloned())
        .collect()
}

/// Duplicate the selection in place, each copy just above its original and `offset` away.
pub fn duplicate(scene: &mut Scene, ids: &[String], offset: f64) -> (Vec<NodeOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut roots = Vec::new();
    for id in outermost(scene, ids) {
        let Some(parent) = scene.parent(&id).map(str::to_string) else {
            continue;
        };
        let nodes: Vec<Node> = scene.subtree(&id).iter().filter_map(|n| scene.node(n).cloned()).collect();
        let index = scene.index(&id).map(|i| i as u32 + 1);
        let shift = if lays_out(scene, &parent) { 0.0 } else { offset };
        let (mut more, mut new_roots) = clone_into(scene, &nodes, &parent, index, shift, shift);
        ops.append(&mut more);
        roots.append(&mut new_roots);
    }
    (ops, roots)
}

/// Group (or frame) the selection: a new group in the selection's common parent, at the top
/// selected node's place, holding the nodes where they were. `None` when they do not share
/// a parent or the parent lays out its children.
pub fn group(scene: &mut Scene, placed: &Placed, ids: &[String], kind: NodeKind) -> Option<(Vec<NodeOp>, String)> {
    let ids = outermost(scene, ids);
    let parent = scene.parent(ids.first()?)?.to_string();
    if ids.iter().any(|id| scene.parent(id) != Some(parent.as_str())) {
        return None;
    }
    let bounds = Rect::bounds(ids.iter().filter_map(|id| placed.rects.get(id)))?;
    let origin = placed.rect(&parent).unwrap_or_default();
    let top = ids.iter().filter_map(|id| scene.index(id)).max()? as u32;
    let gid = scene.fresh_id(kind);
    let mut g = Node::new(kind);
    g.id = gid.clone();
    g.parent = Some(parent.clone());
    g.x = (bounds.x - origin.x).round();
    g.y = (bounds.y - origin.y).round();
    if kind == NodeKind::Frame {
        g.w = bounds.w.round();
        g.h = bounds.h.round();
        g.name = Some("Frame".into());
        g.fill = Some("#00000000".into());
        g.clip = Some(false);
    }
    let mut ops = vec![NodeOp::Add { node: g.clone(), index: Some(top + 1) }];
    let laid = lays_out(scene, &parent);
    for id in &ids {
        let r = placed.rect(id).unwrap_or_default();
        ops.push(NodeOp::Move { id: id.clone(), parent: Some(gid.clone()), index: None });
        let (x, y) = if laid { (r.x - origin.x, r.y - origin.y) } else { (scene.node(id)?.x, scene.node(id)?.y) };
        ops.push(NodeOp::Update { id: id.clone(), set: set(&[("x", px(x - g.x)), ("y", px(y - g.y))]) });
    }
    Some((ops, gid))
}

/// Ungroup a group or frame: its children take its place in its parent, where they showed.
pub fn ungroup(scene: &Scene, placed: &Placed, id: &str) -> Option<(Vec<NodeOp>, Vec<String>)> {
    let node = scene.node(id)?;
    if !matches!(node.kind, NodeKind::Group | NodeKind::Frame) {
        return None;
    }
    let parent = node.parent.clone()?;
    let origin = placed.rect(&parent).unwrap_or_default();
    let start = scene.index(id)? as u32;
    let mut ops = Vec::new();
    let kids = scene.children(id).to_vec();
    for (index, child) in (start..).zip(&kids) {
        let r = placed.rect(child).unwrap_or_default();
        ops.push(NodeOp::Move { id: child.clone(), parent: Some(parent.clone()), index: Some(index) });
        ops.push(NodeOp::Update { id: child.clone(), set: set(&[("x", px(r.x - origin.x)), ("y", px(r.y - origin.y))]) });
    }
    ops.push(NodeOp::Delete { id: id.to_string() });
    Some((ops, kids))
}

/// Put `id` into `parent` at `index` so it stays where it shows (`at` is its page position).
pub fn reparent(scene: &Scene, placed: &Placed, id: &str, parent: &str, index: Option<u32>, at: (f64, f64)) -> Vec<NodeOp> {
    let origin = placed.rect(parent).unwrap_or_default();
    let mut ops = vec![NodeOp::Move { id: id.to_string(), parent: Some(parent.to_string()), index }];
    if !lays_out(scene, parent) {
        ops.push(NodeOp::Update { id: id.to_string(), set: set(&[("x", px(at.0 - origin.x)), ("y", px(at.1 - origin.y))]) });
    }
    ops
}

/// Move the selection by `(dx, dy)` page pixels. Nodes a parent lays out stay put.
pub fn nudge(scene: &Scene, ids: &[String], dx: f64, dy: f64) -> Vec<NodeOp> {
    outermost(scene, ids)
        .into_iter()
        .filter(|id| scene.parent(id).is_some_and(|p| !lays_out(scene, p)))
        .filter_map(|id| {
            let n = scene.node(&id)?;
            let mut s = Map::new();
            if dx != 0.0 {
                s.insert("x".into(), px(n.x + dx));
            }
            if dy != 0.0 {
                s.insert("y".into(), px(n.y + dy));
            }
            Some(NodeOp::Update { id, set: s })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
    /// Equal gaps between, left to right.
    DistributeX,
    /// Equal gaps between, top to bottom.
    DistributeY,
}

/// Align the selection to its own bounds (or one node to its parent).
pub fn align(scene: &Scene, placed: &Placed, ids: &[String], how: Align) -> Vec<NodeOp> {
    let ids: Vec<String> = outermost(scene, ids)
        .into_iter()
        .filter(|id| scene.parent(id).is_some_and(|p| !lays_out(scene, p)))
        .collect();
    let rects: Vec<(String, Rect)> = ids.iter().filter_map(|id| Some((id.clone(), placed.rect(id)?))).collect();
    let target = match rects.as_slice() {
        [] => return Vec::new(),
        [(id, _)] => match scene.parent(id).and_then(|p| placed.rect(p)).filter(|r| r.w > 0.0) {
            Some(parent) => parent,
            None => return Vec::new(),
        },
        many => Rect::bounds(many.iter().map(|(_, r)| r)).unwrap_or_default(),
    };
    let mut moves: Vec<(String, f64, f64)> = Vec::new();
    match how {
        Align::DistributeX | Align::DistributeY if rects.len() > 2 => {
            let x_axis = how == Align::DistributeX;
            let mut sorted = rects.clone();
            sorted.sort_by(|a, b| {
                let (ka, kb) = if x_axis { (a.1.x, b.1.x) } else { (a.1.y, b.1.y) };
                ka.total_cmp(&kb)
            });
            let total: f64 = sorted.iter().map(|(_, r)| if x_axis { r.w } else { r.h }).sum();
            let span = if x_axis { target.w } else { target.h };
            let gap = (span - total) / (sorted.len() - 1) as f64;
            let mut at = if x_axis { target.x } else { target.y };
            for (id, r) in sorted {
                if x_axis {
                    moves.push((id, at - r.x, 0.0));
                    at += r.w + gap;
                } else {
                    moves.push((id, 0.0, at - r.y));
                    at += r.h + gap;
                }
            }
        }
        Align::DistributeX | Align::DistributeY => {}
        _ => {
            for (id, r) in &rects {
                let (dx, dy) = match how {
                    Align::Left => (target.x - r.x, 0.0),
                    Align::CenterX => (target.cx() - r.cx(), 0.0),
                    Align::Right => (target.right() - r.right(), 0.0),
                    Align::Top => (0.0, target.y - r.y),
                    Align::CenterY => (0.0, target.cy() - r.cy()),
                    _ => (0.0, target.bottom() - r.bottom()),
                };
                moves.push((id.clone(), dx, dy));
            }
        }
    }
    moves
        .into_iter()
        .filter(|(_, dx, dy)| dx.abs() > 1e-9 || dy.abs() > 1e-9)
        .flat_map(|(id, dx, dy)| nudge(scene, &[id], dx, dy))
        .collect()
}

/// Raise (`up`) or lower each selected node one step, or (`all`) to the top or bottom of its
/// siblings.
pub fn restack(scene: &Scene, ids: &[String], up: bool, all: bool) -> Vec<NodeOp> {
    let mut ids = outermost(scene, ids);
    if up {
        ids.reverse();
    }
    let mut ops = Vec::new();
    for id in ids {
        let (Some(i), Some(parent)) = (scene.index(&id), scene.parent(&id)) else {
            continue;
        };
        let last = scene.children(parent).len().saturating_sub(1);
        let to = match (up, all) {
            (true, true) => last,
            (true, false) => (i + 1).min(last),
            (false, true) => 0,
            (false, false) => i.saturating_sub(1),
        };
        if to != i {
            ops.push(NodeOp::Move { id, parent: None, index: Some(to as u32) });
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::{node, sample};
    use relay_bus::design::NodeKind as K;

    fn run(scene: &mut Scene, ops: &[NodeOp]) {
        scene.apply_all(ops).unwrap();
    }

    #[test]
    fn duplicate_copies_whole_subtrees_above_the_original() {
        let mut s = sample();
        let (ops, roots) = duplicate(&mut s, &["a".into(), "b".into()], 10.0);
        run(&mut s, &ops);
        assert_eq!(roots.len(), 1);
        let copy = &roots[0];
        assert_eq!(s.children("p1")[1], *copy);
        assert_eq!(s.children(copy).len(), 2);
        let n = s.node(copy).unwrap();
        assert_eq!((n.x, n.y, n.parent.as_deref()), (10.0, 10.0, Some("p1")));
    }

    #[test]
    fn group_then_ungroup_keeps_everything_in_place() {
        let mut s = sample();
        let before = place(&s, "p1", &Mono);
        let (ops, gid) = group(&mut s, &before, &["b".into(), "c".into()], K::Group).unwrap();
        run(&mut s, &ops);
        let grouped = place(&s, "p1", &Mono);
        assert_eq!(s.children(&gid), ["b", "c"]);
        for id in ["b", "c"] {
            assert_eq!(grouped.rect(id), before.rect(id), "{id} stays put");
        }
        let (ops, kids) = ungroup(&s, &grouped, &gid).unwrap();
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        assert_eq!(kids, ["b", "c"]);
        assert!(s.node(&gid).is_none());
        for id in ["b", "c"] {
            assert_eq!(after.rect(id), before.rect(id));
        }
    }

    #[test]
    fn frame_selection_sizes_to_the_selection() {
        let mut s = sample();
        let placed = place(&s, "p1", &Mono);
        let (ops, fid) = group(&mut s, &placed, &["a".into(), "d".into()], K::Frame).unwrap();
        run(&mut s, &ops);
        let f = s.node(&fid).unwrap();
        assert_eq!((f.x, f.y, f.w, f.h), (0.0, 0.0, 580.0, 300.0));
        // Nodes in different parents cannot be grouped.
        let mut s = sample();
        assert!(group(&mut s, &placed, &["b".into(), "d".into()], K::Group).is_none());
    }

    #[test]
    fn reparenting_keeps_the_page_position() {
        let mut s = sample();
        let placed = place(&s, "p1", &Mono);
        let ops = reparent(&s, &placed, "d", "a", None, (150.0, 60.0));
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        assert_eq!(after.rect("d").map(|r| (r.x, r.y)), Some((150.0, 60.0)));
        assert_eq!(s.parent("d"), Some("a"));
    }

    #[test]
    fn align_and_distribute() {
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            node("b", Some("p1"), K::Rect, [50.0, 30.0, 10.0, 20.0]),
            node("c", Some("p1"), K::Rect, [100.0, 5.0, 10.0, 10.0]),
        ]);
        let placed = place(&s, "p1", &Mono);
        let ids: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let ops = align(&s, &placed, &ids, Align::Bottom);
        run(&mut s, &ops);
        assert!(["a", "b", "c"].iter().all(|id| {
            let n = s.node(id).unwrap();
            n.y + n.h == 50.0
        }));
        let mut s2 = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("a", Some("p1"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            node("b", Some("p1"), K::Rect, [20.0, 0.0, 10.0, 10.0]),
            node("c", Some("p1"), K::Rect, [100.0, 0.0, 10.0, 10.0]),
        ]);
        let placed = place(&s2, "p1", &Mono);
        let ops = align(&s2, &placed, &ids, Align::DistributeX);
        run(&mut s2, &ops);
        assert_eq!(s2.node("b").unwrap().x, 50.0);
    }

    #[test]
    fn restack_and_nudge() {
        let mut s = sample();
        let ops = restack(&s, &["b".into()], true, true);
        run(&mut s, &ops);
        assert_eq!(s.children("a"), ["c", "b"]);
        let ops = restack(&s, &["b".into()], false, false);
        run(&mut s, &ops);
        assert_eq!(s.children("a"), ["b", "c"]);
        let ops = nudge(&s, &["b".into()], 1.0, -10.0);
        run(&mut s, &ops);
        let b = s.node("b").unwrap();
        assert_eq!((b.x, b.y), (11.0, 0.0));
    }

    #[test]
    fn paste_lands_in_the_target_with_fresh_ids() {
        let mut s = sample();
        let clip = gather(&s, &["a".into(), "c".into()]);
        assert_eq!(clip.len(), 3);
        let (ops, roots) = clone_into(&mut s, &clip, "p1", None, 600.0, 0.0);
        run(&mut s, &ops);
        let root = s.node(&roots[0]).unwrap();
        assert_eq!((root.x, root.kind), (600.0, K::Frame));
        assert_eq!(s.children(&roots[0]).len(), 2);
        assert_eq!(s.len(), 8);
    }
}
