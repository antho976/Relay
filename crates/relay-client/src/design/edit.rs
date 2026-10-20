//! The `design.apply` edits behind the canvas's commands: duplicate, paste, group and frame
//! the selection, ungroup, delete, align and distribute, reorder, nudge, and put a node in
//! another parent without it jumping. Each returns the edits; the canvas runs them on its scene and
//! sends them as one list, so one undo reverts the whole command.

use super::geom::Rect;
use super::layout::Placed;
use super::scene::Scene;
use super::shapes;
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

/// Fresh ids for every node of `nodes`, keyed by the old.
fn fresh_ids(scene: &mut Scene, nodes: &[Node]) -> HashMap<String, String> {
    nodes.iter().map(|n| (n.id.clone(), scene.fresh_id(n.kind))).collect()
}

/// The page point a node's `x`/`y` put it at, read from the stored tree (no layout): `nodes`
/// first, then the scene.
fn stored_point(scene: &Scene, nodes: &HashMap<&str, &Node>, id: &str) -> Option<(f64, f64)> {
    let (mut x, mut y, mut at) = (0.0, 0.0, id);
    for _ in 0..=scene.len() + nodes.len() {
        let n = nodes.get(at).copied().or_else(|| scene.node(at))?;
        (x, y) = (x + n.x, y + n.y);
        match n.parent.as_deref() {
            Some(parent) => at = parent,
            None => return Some((x, y)),
        }
    }
    None
}

/// A connector's two ends relative to its own `x`/`y`, from the stored boxes of the nodes it is
/// attached to (their centres), for a line that has no points of its own to fall back on.
fn stored_ends(scene: &Scene, nodes: &HashMap<&str, &Node>, line: &Node) -> Vec<[f64; 2]> {
    let centre = |end: &Option<String>| {
        let id = end.as_deref()?;
        let n = nodes.get(id).copied().or_else(|| scene.node(id))?;
        let (x, y) = stored_point(scene, nodes, id)?;
        Some((x + n.w / 2.0, y + n.h / 2.0))
    };
    match (stored_point(scene, nodes, &line.id), centre(&line.from), centre(&line.to)) {
        (Some((ox, oy)), Some(a), Some(b)) => vec![[(a.0 - ox).round(), (a.1 - oy).round()], [(b.0 - ox).round(), (b.1 - oy).round()]],
        _ => vec![[0.0, 0.0], [line.w, line.h]],
    }
}

/// A copied connector's ends: one attached to a node copied with it follows the copy; one
/// attached to a node left behind lets go, the line keeping where it was last drawn.
fn rewire(scene: &Scene, nodes: &HashMap<&str, &Node>, fresh: &HashMap<String, String>, copy: &mut Node) {
    let original = nodes.get(copy.id.as_str()).copied();
    let mut freed = false;
    for end in [&mut copy.from, &mut copy.to] {
        if let Some(id) = end.take() {
            match fresh.get(&id) {
                Some(new) => *end = Some(new.clone()),
                None => freed = true,
            }
        }
    }
    if freed && copy.points.as_ref().is_none_or(|p| p.len() < 2) {
        copy.points = Some(original.map_or_else(|| vec![[0.0, 0.0], [copy.w, copy.h]], |o| stored_ends(scene, nodes, o)));
    }
}

/// Copies of `nodes` (whole subtrees, parents first) under the ids in `fresh`. `place` says
/// where each root goes: its parent, index and shift. Returns the edits and the new roots.
fn copies(
    scene: &Scene,
    nodes: &[Node],
    fresh: &HashMap<String, String>,
    mut place: impl FnMut(&Node) -> (String, Option<u32>, f64, f64),
) -> (Vec<NodeOp>, Vec<String>) {
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut ops = Vec::new();
    let mut roots = Vec::new();
    for n in nodes {
        if n.kind == NodeKind::Page {
            continue;
        }
        // A fresh add, held to what its kind uses: a stale field or one a newer build wrote
        // stays with the original.
        let mut copy = n.without_foreign();
        rewire(scene, &by_id, fresh, &mut copy);
        copy.id = fresh[&n.id].clone();
        let root = n.parent.as_deref().is_none_or(|p| !by_id.contains_key(p));
        if root {
            let (parent, index, dx, dy) = place(n);
            copy.parent = Some(parent);
            copy.x = (copy.x + dx).round();
            copy.y = (copy.y + dy).round();
            roots.push(copy.id.clone());
            ops.push(NodeOp::Add { node: copy, index });
        } else {
            copy.parent = n.parent.as_ref().map(|p| fresh[p].clone());
            ops.push(NodeOp::Add { node: copy, index: None });
        }
    }
    (ops, roots)
}

/// Copies of `nodes` (whole subtrees, as `copy` gathered them) under fresh ids: roots go into
/// `parent` at `index`, moved by `(dx, dy)`. A connector between two copied nodes joins the
/// copies; an end attached to a node not copied lets go. Returns the edits and the new roots.
pub fn clone_into(
    scene: &mut Scene,
    nodes: &[Node],
    parent: &str,
    index: Option<u32>,
    dx: f64,
    dy: f64,
) -> (Vec<NodeOp>, Vec<String>) {
    let fresh = fresh_ids(scene, nodes);
    let mut at = index;
    copies(scene, nodes, &fresh, |_| {
        let here = at;
        at = at.map(|i| i + 1);
        (parent.to_string(), here, dx, dy)
    })
}

/// Every node of the selection's subtrees, parents first, as stored (their colour and style
/// names kept): what Copy puts on the clipboard.
pub fn gather(scene: &Scene, ids: &[String]) -> Vec<Node> {
    outermost(scene, ids)
        .iter()
        .flat_map(|id| scene.subtree(id))
        .filter_map(|id| scene.stored(&id).cloned())
        .collect()
}

/// `nodes` as they draw, naming no colour or style: for another design, which may not hold the
/// names this one does. Each takes its style laid under it and each `$name` as the colour it
/// names (`Styles::resolve`), then lets go of the style's name.
pub fn baked(scene: &Scene, nodes: &[Node]) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| {
            let mut drawn = scene.styles().resolve(n);
            drawn.style = None;
            drawn
        })
        .collect()
}

/// Duplicate the selection in place, each copy just above its original and `offset` away, as
/// stored (a copy names the colours and style its original names). One set of fresh ids covers
/// the whole selection, so a connector between two duplicated nodes joins their copies.
pub fn duplicate(scene: &mut Scene, ids: &[String], offset: f64) -> (Vec<NodeOp>, Vec<String>) {
    let roots: Vec<String> = outermost(scene, ids).into_iter().filter(|id| scene.parent(id).is_some()).collect();
    let nodes: Vec<Node> = roots.iter().flat_map(|id| scene.subtree(id)).filter_map(|n| scene.stored(&n).cloned()).collect();
    let fresh = fresh_ids(scene, &nodes);
    // Copies already put in a parent, all below the next original there (roots run in paint
    // order): each later original sits that much higher.
    let mut added: HashMap<String, u32> = HashMap::new();
    copies(scene, &nodes, &fresh, |n| {
        let parent = n.parent.clone().unwrap_or_default();
        let before = added.entry(parent.clone()).or_default();
        let index = scene.index(&n.id).map(|i| i as u32 + 1 + *before);
        *before += 1;
        let shift = if lays_out(scene, &parent) { 0.0 } else { offset };
        (parent, index, shift, shift)
    })
}

/// Group (or frame) the selection: a new group in the selection's common parent, at the top
/// selected node's place, holding the nodes where they were. `None` when they do not share
/// a parent, or for a group when the parent lays out its children (a group has no size of
/// its own to be laid out by); a frame there is laid out like any child.
pub fn group(scene: &mut Scene, placed: &Placed, ids: &[String], kind: NodeKind) -> Option<(Vec<NodeOp>, String)> {
    let ids = outermost(scene, ids);
    let parent = scene.parent(ids.first()?)?.to_string();
    if ids.iter().any(|id| scene.parent(id) != Some(parent.as_str())) {
        return None;
    }
    let laid = lays_out(scene, &parent);
    if laid && kind != NodeKind::Frame {
        return None;
    }
    let bounds = Rect::bounds(ids.iter().filter_map(|id| placed.rects.get(id)))?;
    let origin = placed.origin(&parent);
    let top = ids.iter().filter_map(|id| scene.index(id)).max()? as u32;
    let gid = scene.fresh_id(kind);
    let mut g = Node::new(kind);
    g.id = gid.clone();
    g.parent = Some(parent.clone());
    g.x = (bounds.x - origin.0).round();
    g.y = (bounds.y - origin.1).round();
    if kind == NodeKind::Frame {
        g.w = bounds.w.round();
        g.h = bounds.h.round();
        g.name = Some("Frame".into());
        g.fill = Some("#00000000".into());
        g.clip = Some(false);
    }
    let mut ops = vec![NodeOp::Add { node: g.clone(), index: Some(top + 1) }];
    for id in &ids {
        ops.push(NodeOp::Move { id: id.clone(), parent: Some(gid.clone()), index: None, keep_position: None });
        let (x, y) = if laid {
            let (x, y) = placed.origin(id);
            (x - origin.0, y - origin.1)
        } else {
            (scene.node(id)?.x, scene.node(id)?.y)
        };
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
    // A child of a group or a free frame sits at the container's `x`/`y` plus its own, hidden or
    // not; one a frame lays out sits where the layout put it.
    let laid = lays_out(scene, id);
    let origin = placed.origin(&parent);
    let start = scene.index(id)? as u32;
    let mut ops = Vec::new();
    let kids = scene.children(id).to_vec();
    for (index, child) in (start..).zip(&kids) {
        let c = scene.node(child)?;
        let (x, y) = if laid {
            let (x, y) = placed.origin(child);
            (x - origin.0, y - origin.1)
        } else {
            (node.x + c.x, node.y + c.y)
        };
        ops.push(NodeOp::Move { id: child.clone(), parent: Some(parent.clone()), index: Some(index), keep_position: None });
        ops.push(NodeOp::Update { id: child.clone(), set: set(&[("x", px(x)), ("y", px(y))]) });
    }
    ops.push(NodeOp::Delete { id: id.to_string() });
    Some((ops, kids))
}

/// Ungroup every group and frame among `ids` in one list: one inside another selected one goes
/// with it, and each is ungrouped against the tree the ones before it left. Returns the edits
/// and the children set free.
pub fn ungroup_all(scene: &Scene, placed: &Placed, ids: &[String]) -> (Vec<NodeOp>, Vec<String>) {
    let groups: Vec<String> = ids
        .iter()
        .filter(|id| scene.node(id).is_some_and(|n| matches!(n.kind, NodeKind::Group | NodeKind::Frame)))
        .cloned()
        .collect();
    let mut trial = scene.clone();
    let mut ops = Vec::new();
    let mut kids = Vec::new();
    for id in outermost(scene, &groups) {
        let Some((more, freed)) = ungroup(&trial, placed, &id) else {
            continue;
        };
        if trial.apply_all(&more).is_ok() {
            ops.extend(more);
            kids.extend(freed);
        }
    }
    (ops, kids)
}

/// Put `id` into `parent` at `index` so it stays where it shows (`at` is where its placed box
/// lands on the page).
pub fn reparent(scene: &Scene, placed: &Placed, id: &str, parent: &str, index: Option<u32>, at: (f64, f64)) -> Vec<NodeOp> {
    let origin = placed.origin(parent);
    // A group's `x`/`y` is its own point, not its box (its children's bounds).
    let (ox, oy) = placed.origin(id);
    let (bx, by) = placed.rect(id).map_or((ox, oy), |r| (r.x, r.y));
    let (x, y) = (at.0 + ox - bx, at.1 + oy - by);
    let mut ops = vec![NodeOp::Move { id: id.to_string(), parent: Some(parent.to_string()), index, keep_position: None }];
    if !lays_out(scene, parent) {
        ops.push(NodeOp::Update { id: id.to_string(), set: set(&[("x", px(x - origin.0)), ("y", px(y - origin.1))]) });
    }
    ops
}

/// Delete the selection. A connector attached to a node going lets go of that end first, where
/// it is drawn now (its points become its own), so no line names a node that is gone; the
/// list's undo attaches it again.
pub fn delete(scene: &Scene, placed: &Placed, ids: &[String]) -> Vec<NodeOp> {
    let roots = outermost(scene, ids);
    let doomed: HashSet<String> = roots.iter().flat_map(|id| scene.subtree(id)).collect();
    let mut ops = Vec::new();
    for page in scene.pages() {
        for id in scene.subtree(page) {
            let Some(n) = scene.node(&id).filter(|_| !doomed.contains(&id)) else {
                continue;
            };
            let gone = |end: &Option<String>| end.as_ref().is_some_and(|e| doomed.contains(e));
            if gone(&n.from) || gone(&n.to) {
                ops.push(NodeOp::Update { id: id.clone(), set: let_go(scene, placed, n, gone(&n.from), gone(&n.to)) });
            }
        }
    }
    ops.extend(roots.into_iter().map(|id| NodeOp::Delete { id }));
    ops
}

/// The update that frees a connector's `from` and/or `to` where the line is drawn now.
fn let_go(scene: &Scene, placed: &Placed, n: &Node, from: bool, to: bool) -> Map<String, Value> {
    let mut s = Map::new();
    for (end, freed) in [("from", from), ("to", to)] {
        if freed {
            s.insert(end.into(), Value::Null);
        }
    }
    let drawn = placed.rect(&n.id).map(|_| shapes::line_points(scene, placed, n).0);
    match drawn.as_deref() {
        Some([a, .., b]) => {
            let (a, b) = (*a, *b);
            let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
            let origin = n.parent.as_deref().map_or((0.0, 0.0), |p| placed.origin(p));
            s.insert("x".into(), px(x0 - origin.0));
            s.insert("y".into(), px(y0 - origin.1));
            s.insert("w".into(), px((b.0 - a.0).abs()));
            s.insert("h".into(), px((b.1 - a.1).abs()));
            s.insert("points".into(), json!([[(a.0 - x0).round(), (a.1 - y0).round()], [(b.0 - x0).round(), (b.1 - y0).round()]]));
        }
        // Not on the page shown: keep its own points, or take its ends' stored centres.
        _ if n.points.as_ref().is_some_and(|p| p.len() >= 2) => {}
        _ => {
            s.insert("points".into(), json!(stored_ends(scene, &HashMap::new(), n)));
        }
    }
    s
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
/// siblings. Selected siblings keep their order among themselves, and one step never takes a
/// node past another selected one: a node already at the top holds the one below it.
pub fn restack(scene: &Scene, ids: &[String], up: bool, all: bool) -> Vec<NodeOp> {
    let ids = outermost(scene, ids);
    let chosen: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut parents: Vec<&str> = Vec::new();
    for id in &ids {
        if let Some(p) = scene.parent(id).filter(|p| !parents.contains(p)) {
            parents.push(p);
        }
    }
    let mut ops = Vec::new();
    for parent in parents {
        let mut order: Vec<String> = scene.children(parent).to_vec();
        let last = order.len().saturating_sub(1);
        // Nearest the way they go first for a step, so the one ahead has moved (or holds);
        // farthest first to the top or bottom, so each lands beyond the ones before it.
        let mut moving: Vec<String> = order.iter().filter(|c| chosen.contains(c.as_str())).cloned().collect();
        if up != all {
            moving.reverse();
        }
        for id in moving {
            let Some(i) = order.iter().position(|c| *c == id) else {
                continue;
            };
            let to = match (up, all) {
                (true, true) => last,
                (false, true) => 0,
                (true, false) if i < last && !chosen.contains(order[i + 1].as_str()) => i + 1,
                (false, false) if i > 0 && !chosen.contains(order[i - 1].as_str()) => i - 1,
                _ => i,
            };
            if to == i {
                continue;
            }
            order.remove(i);
            order.insert(to, id.clone());
            ops.push(NodeOp::Move { id, parent: None, index: Some(to as u32), keep_position: None });
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::layout::{place, tests::Mono};
    use crate::design::scene::tests::{node, sample, styled};
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

    /// A copy is a fresh add: what the original's kind does not use, or a newer build wrote,
    /// stays behind, so the copy passes a strict list.
    #[test]
    fn a_copy_leaves_stale_fields_behind() {
        let mut s = sample();
        let legacy = Node::read(json!({"id": "g", "parent": "p1", "kind": "group", "future": 1,
            "shadow": {"blur": 4, "color": "#000"}})).unwrap();
        s.apply(&NodeOp::Add { node: legacy, index: None }).unwrap();
        s.apply(&NodeOp::Add { node: node("in", Some("g"), K::Rect, [0.0, 0.0, 5.0, 5.0]), index: None }).unwrap();
        let (ops, roots) = duplicate(&mut s, &["g".into()], 10.0);
        run(&mut s, &ops);
        let copy = s.node(&roots[0]).unwrap();
        assert_eq!((copy.shadow.clone(), copy.extra.len(), copy.x), (None, 0, 10.0));
        assert_eq!(s.children(&roots[0]).len(), 1);
        assert!(s.node("g").unwrap().shadow.is_some(), "the original keeps it");
    }

    /// A duplicate and a copy name what their originals name: the colours and styles stay
    /// linked in the same design.
    #[test]
    fn duplicate_keeps_style_links() {
        let mut s = styled();
        let (ops, roots) = duplicate(&mut s, &ids(&["a", "d"]), 10.0);
        run(&mut s, &ops);
        let (frame, ellipse) = (&roots[0], &roots[1]);
        let b = s.children(frame)[0].clone();
        assert_eq!(s.stored(&b).unwrap().style.as_deref(), Some("card"));
        assert_eq!(s.stored(ellipse).unwrap().fill.as_deref(), Some("$accent"));
        assert_eq!(s.node(ellipse).unwrap().fill.as_deref(), Some("#4C9DFF"), "and draws it");
        let clip = gather(&s, &ids(&["b", "c"]));
        assert_eq!(clip.iter().map(|n| n.style.as_deref()).collect::<Vec<_>>(), [Some("card"), Some("title")]);
        assert_eq!(clip[0].fill, None, "the style's fill is not baked in");
    }

    /// Baked, a copy draws the same with no name left: what goes to another design.
    #[test]
    fn baked_copies_name_nothing() {
        let s = styled();
        let clip = gather(&s, &ids(&["a", "d"]));
        let baked = baked(&s, &clip);
        assert_eq!(baked.len(), clip.len());
        for n in &baked {
            assert!(n.names().is_empty(), "{} names {:?}", n.id, n.names());
            let mut drawn = s.node(&n.id).unwrap().clone();
            drawn.style = None;
            assert_eq!(*n, drawn, "{} draws as it did", n.id);
        }
        let b = baked.iter().find(|n| n.id == "b").unwrap();
        assert_eq!((b.fill.as_deref(), b.radius, b.stroke_width), (Some("#4C9DFF"), Some(12.0), Some(2.0)));
        // Pasted into a design with no names, it draws as here.
        let mut other = Scene::new(vec![node("q1", None, K::Page, [0.0; 4])]);
        let (ops, roots) = clone_into(&mut other, &baked, "q1", None, 0.0, 0.0);
        run(&mut other, &ops);
        assert_eq!(other.node(&roots[1]).unwrap().fill.as_deref(), Some("#4C9DFF"));
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

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn row(n: usize) -> Scene {
        let mut nodes = vec![node("p1", None, K::Page, [0.0; 4])];
        nodes.extend((0..n).map(|i| node(&format!("x{i}"), Some("p1"), K::Rect, [i as f64 * 20.0, 0.0, 10.0, 10.0])));
        Scene::new(nodes)
    }

    #[test]
    fn duplicating_siblings_puts_each_copy_just_above_its_own_original() {
        let mut s = row(3);
        let (ops, roots) = duplicate(&mut s, &ids(&["x0", "x2"]), 10.0);
        run(&mut s, &ops);
        let kids = s.children("p1");
        assert_eq!(kids.len(), 5);
        assert_eq!([&kids[0], &kids[2], &kids[3]], ["x0", "x1", "x2"]);
        assert_eq!([&kids[1], &kids[4]], [&roots[0], &roots[1]]);
        assert_eq!(s.node(&roots[1]).unwrap().x, 50.0);
    }

    fn connected() -> Scene {
        let mut line = node("ln", Some("p1"), K::Line, [0.0; 4]);
        line.from = Some("x0".into());
        line.to = Some("x1".into());
        let mut s = row(3);
        s.apply(&NodeOp::Add { node: line, index: None }).unwrap();
        s
    }

    #[test]
    fn a_duplicated_connector_joins_the_copies_or_lets_go() {
        // Both ends and the line copied together: the copy runs between the copies.
        let mut s = connected();
        let (ops, roots) = duplicate(&mut s, &ids(&["x0", "x1", "ln"]), 10.0);
        run(&mut s, &ops);
        let copy = s.node(&roots[2]).unwrap();
        assert_eq!((copy.from.as_ref(), copy.to.as_ref()), (Some(&roots[0]), Some(&roots[1])));
        // Only the line and one end: the other end lets go where the line was drawn.
        let mut s = connected();
        let (ops, roots) = duplicate(&mut s, &ids(&["x0", "ln"]), 0.0);
        run(&mut s, &ops);
        let copy = s.node(&roots[1]).unwrap();
        assert_eq!((copy.from.as_ref(), copy.to.as_deref()), (Some(&roots[0]), None));
        assert_eq!(copy.points, Some(vec![[5.0, 5.0], [25.0, 5.0]]), "its ends' last centres");
        // Pasted alone: both ends let go; its own points are kept.
        let mut s = connected();
        s.node_mut("ln").unwrap().points = Some(vec![[1.0, 2.0], [3.0, 4.0]]);
        let clip = gather(&s, &ids(&["ln"]));
        let (ops, roots) = clone_into(&mut s, &clip, "p1", None, 0.0, 0.0);
        run(&mut s, &ops);
        let copy = s.node(&roots[0]).unwrap();
        assert_eq!((copy.from.as_deref(), copy.to.as_deref(), copy.points.clone()), (None, None, Some(vec![[1.0, 2.0], [3.0, 4.0]])));
    }

    #[test]
    fn deleting_a_shape_lets_its_connectors_go_where_they_are_drawn() {
        let mut s = connected();
        let placed = place(&s, "p1", &Mono);
        let drawn = shapes::line_points(&s, &placed, s.node("ln").unwrap()).0;
        let ops = delete(&s, &placed, &ids(&["x1"]));
        assert!(matches!(&ops[..], [NodeOp::Update { .. }, NodeOp::Delete { id }] if id == "x1"), "{ops:?}");
        run(&mut s, &ops);
        let line = s.node("ln").unwrap();
        assert_eq!((line.from.as_deref(), line.to.as_deref()), (Some("x0"), None));
        let placed = place(&s, "p1", &Mono);
        assert_eq!(shapes::line_points(&s, &placed, line).0, drawn, "the line stays where it was");
        // A line going with the shape is just deleted.
        let s = connected();
        let placed = place(&s, "p1", &Mono);
        assert_eq!(delete(&s, &placed, &ids(&["x1", "ln"])).len(), 2);
    }

    #[test]
    fn restacking_several_keeps_their_order() {
        let order = |s: &Scene| s.children("p1").to_vec();
        let mut s = row(4);
        let ops = restack(&s, &ids(&["x0", "x1"]), true, true);
        run(&mut s, &ops);
        assert_eq!(order(&s), ["x2", "x3", "x0", "x1"]);
        let mut s = row(4);
        let ops = restack(&s, &ids(&["x2", "x3"]), false, true);
        run(&mut s, &ops);
        assert_eq!(order(&s), ["x2", "x3", "x0", "x1"]);
        // One step: a node at the top holds the selected one below it.
        let mut s = row(4);
        assert!(restack(&s, &ids(&["x2", "x3"]), true, false).is_empty());
        let ops = restack(&s, &ids(&["x1", "x3"]), true, false);
        run(&mut s, &ops);
        assert_eq!(order(&s), ["x0", "x2", "x1", "x3"]);
        let ops = restack(&s, &ids(&["x0", "x2"]), false, false);
        run(&mut s, &ops);
        assert_eq!(order(&s), ["x0", "x2", "x1", "x3"]);
        let ops = restack(&s, &ids(&["x2", "x3"]), false, false);
        run(&mut s, &ops);
        assert_eq!(order(&s), ["x2", "x0", "x3", "x1"]);
    }

    #[test]
    fn a_group_is_not_made_in_a_laid_out_frame_but_a_frame_is() {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 200.0, 200.0]);
        col.layout = Some(Layout::Column);
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            col,
            node("r1", Some("col"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            node("r2", Some("col"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
        ]);
        let placed = place(&s, "p1", &Mono);
        assert!(group(&mut s, &placed, &ids(&["r1", "r2"]), K::Group).is_none());
        let (ops, fid) = group(&mut s, &placed, &ids(&["r1", "r2"]), K::Frame).unwrap();
        run(&mut s, &ops);
        assert_eq!(s.children(&fid), ["r1", "r2"]);
        assert_eq!(s.node("r2").unwrap().y, 10.0, "where the column had put it");
    }

    /// A page with group `g1` at (100, 100) holding group `g2` at (10, 10) (its rect at (5, 5))
    /// and a rect at (50, 50); and a lone group `g3`.
    fn nested() -> Scene {
        Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("g1", Some("p1"), K::Group, [100.0, 100.0, 0.0, 0.0]),
            node("g2", Some("g1"), K::Group, [10.0, 10.0, 0.0, 0.0]),
            node("r", Some("g2"), K::Rect, [5.0, 5.0, 10.0, 10.0]),
            node("r2", Some("g1"), K::Rect, [50.0, 50.0, 10.0, 10.0]),
            node("g3", Some("p1"), K::Group, [300.0, 0.0, 0.0, 0.0]),
            node("r3", Some("g3"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
        ])
    }

    #[test]
    fn ungrouping_and_reparenting_count_from_a_groups_own_point() {
        let mut s = nested();
        let before = place(&s, "p1", &Mono);
        let (ops, _) = ungroup(&s, &before, "g1").unwrap();
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        for id in ["r", "r2", "g2"] {
            assert_eq!(after.rect(id), before.rect(id), "{id} stays put");
        }
        // Into a group whose children's bounds are not its own point.
        let mut s = nested();
        let placed = place(&s, "p1", &Mono);
        let ops = reparent(&s, &placed, "r3", "g2", None, (200.0, 200.0));
        run(&mut s, &ops);
        assert_eq!(place(&s, "p1", &Mono).rect("r3").map(|r| (r.x, r.y)), Some((200.0, 200.0)));
        // A group moved into another keeps its children where they were.
        let mut s = nested();
        let placed = place(&s, "p1", &Mono);
        let at = placed.rect("g2").map(|r| (r.x, r.y)).unwrap();
        let ops = reparent(&s, &placed, "g2", "g3", None, at);
        run(&mut s, &ops);
        assert_eq!(place(&s, "p1", &Mono).rect("r"), placed.rect("r"));
    }

    #[test]
    fn ungroup_all_takes_several_groups_at_once() {
        let mut s = nested();
        let before = place(&s, "p1", &Mono);
        let (ops, kids) = ungroup_all(&s, &before, &ids(&["g2", "g3", "g1", "r"]));
        run(&mut s, &ops);
        // g2 went with g1, which was selected too; g3 and g1 are gone.
        assert_eq!(kids, ["g2", "r2", "r3"]);
        assert_eq!(s.children("p1"), ["g2", "r2", "r3"]);
        let after = place(&s, "p1", &Mono);
        for id in ["r", "r2", "r3"] {
            assert_eq!(after.rect(id), before.rect(id), "{id} stays put");
        }
    }
}
