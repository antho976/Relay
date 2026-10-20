//! The `design.apply` edits behind the canvas's commands: duplicate, paste, group and frame
//! the selection, ungroup, delete, align and distribute, reorder, nudge, and put a node in
//! another parent without it jumping. Each returns the edits; the canvas runs them on its scene and
//! sends them as one list, so one undo reverts the whole command.

use super::geom::Rect;
use super::layout::{fit_of, is_laid, stored_shift, unpinned, Fit, Placed};
use super::scene::Scene;
use super::shapes;
use relay_bus::design::{Items, Layout, Node, NodeKind, NodeOp, Sizing, MAX_COORD};
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
/// One `absolute` child it does not place: [`layout::is_laid`](super::layout::is_laid) asks of
/// the child.
pub fn lays_out(scene: &Scene, parent: &str) -> bool {
    scene.node(parent).is_some_and(|p| p.layout_mode() != Layout::None)
}

/// A position within what `validate` takes (±`MAX_COORD`), in whole pixels.
fn coord(v: f64) -> f64 {
    v.round().clamp(-MAX_COORD, MAX_COORD)
}

/// Where `id` shows: its own point (a group's, else its box's corner) less `origin`, and its
/// box's size. `None` for what is not placed (hidden, or on another page).
fn shown(placed: &Placed, id: &str, origin: (f64, f64)) -> Option<Rect> {
    let r = placed.rect(id)?;
    let (x, y) = placed.point(id);
    Some(Rect::new(x - origin.0, y - origin.1, r.w, r.h))
}

/// The axes along which `id`'s parent stretches it across a line of its layout (`items:
/// stretch`): a column's width, a row's or a wrap's height. Its size there is the line's, not
/// what its own rule gives.
fn stretched(scene: &Scene, id: &str) -> (bool, bool) {
    let Some(p) = scene.parent(id).and_then(|p| scene.node(p)).filter(|_| is_laid(scene, id)) else {
        return (false, false);
    };
    match (p.items, p.layout_mode()) {
        (Some(Items::Stretch), Layout::Column) => (true, false),
        (Some(Items::Stretch), Layout::Row | Layout::Wrap) => (false, true),
        _ => (false, false),
    }
}

/// What [`settle`] does with a size its node's own rule would not give it out of a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sizes {
    /// Fixed at the size shown: a fill, and a hug of what no longer measures it.
    Freeze,
}

/// The fields that put `n` (as stored) at `rel` out of any auto layout (TM-8): `rel` is the box
/// it shows, from its new parent's origin (a group's own point and its children's bounds' size),
/// in a parent drawn as `fit`, through whose pins the stored box is worked out
/// (`layout::unpinned`). `x`/`y` always, `w`/`h` where they change. An axis that fills or hugs
/// is fixed at the size shown ([`Sizes`]), but a text's, a sticky's or a laid-out frame's hug,
/// which measures what it holds wherever it is, unless the layout it leaves stretched it
/// (`stretched`). A group's size is its children's and a line's or path's its points', so
/// theirs stay as they are.
fn settle(n: &Node, rel: Rect, fit: Option<Fit>, stretched: (bool, bool), sizes: Sizes) -> Map<String, Value> {
    let r = unpinned(n, fit, rel);
    let mut s = set(&[("x", json!(coord(r.x))), ("y", json!(coord(r.y)))]);
    if matches!(n.kind, NodeKind::Group | NodeKind::Line | NodeKind::Path) {
        return s;
    }
    let measures = matches!(n.kind, NodeKind::Text | NodeKind::Sticky) || n.layout_mode() != Layout::None;
    let axes = [
        ("w", "w_sizing", n.w, n.w_rule(), stretched.0, n.clamp_w(r.w.round())),
        ("h", "h_sizing", n.h, n.h_rule(), stretched.1, n.clamp_h(r.h.round())),
    ];
    for (key, rule_key, stored, rule, stretched, size) in axes {
        if rule == Sizing::Hug && measures && !stretched {
            continue;
        }
        let size = size.clamp(0.0, MAX_COORD);
        if size != stored {
            s.insert(key.into(), json!(size));
        }
        let fixed = match rule {
            Sizing::Fixed => false,
            Sizing::Hug => sizes == Sizes::Freeze || measures,
            Sizing::Fill => sizes == Sizes::Freeze,
        };
        if fixed {
            s.insert(rule_key.into(), json!("fixed"));
        }
    }
    s
}

/// The update that keeps `id` where and as big as it shows (TM-8) once it sits out of any auto
/// layout, in a parent whose children count from page point `origin` and which is drawn as `fit`:
/// a node leaving its layout, a layout turned off, a frame ungrouped. Its box as placed, written
/// through the parent's pins, with each size a fill or a hug gave it fixed, but a text's, a
/// sticky's or a laid-out frame's hug, which measures what it holds wherever it is. `None` for a
/// node not placed (hidden): nothing shows where it would be.
pub fn freeze(scene: &Scene, placed: &Placed, id: &str, origin: (f64, f64), fit: Option<Fit>) -> Option<Map<String, Value>> {
    let n = scene.stored(id)?;
    Some(settle(n, shown(placed, id, origin)?, fit, stretched(scene, id), Sizes::Freeze))
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
        let shift = if is_laid(scene, &n.id) { 0.0 } else { offset };
        (parent, index, shift, shift)
    })
}

/// Group (or frame) the selection: a new group in the selection's common parent, at the top
/// selected node's place, holding the nodes where and as big as they showed ([`freeze`]: one
/// the parent laid out keeps its box). `None` when they do not share a parent, or for a group
/// when the parent lays out its children (a group has no size of its own to be laid out by); a
/// frame there is laid out like any child.
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
    // Its children count from its own point, which is its box's corner.
    let inside = (origin.0 + g.x, origin.1 + g.y);
    for id in &ids {
        ops.push(NodeOp::Move { id: id.clone(), parent: Some(gid.clone()), index: None, keep_position: None });
        let s = match freeze(scene, placed, id, inside, None) {
            Some(s) => s,
            // Hidden: where its own `x`/`y` put it, or the corner if the layout placed it.
            None if laid => set(&[("x", px(0.0)), ("y", px(0.0))]),
            None => set(&[("x", px(scene.stored(id)?.x - g.x)), ("y", px(scene.stored(id)?.y - g.y))]),
        };
        ops.push(NodeOp::Update { id: id.clone(), set: s });
    }
    Some((ops, gid))
}

/// Ungroup a group or frame: its children take its place in its parent, where and as big as
/// they showed ([`freeze`]: one the frame laid out keeps its box, a filling one its width),
/// written through the parent's pins. Into a parent that lays them out they only move.
pub fn ungroup(scene: &Scene, placed: &Placed, id: &str) -> Option<(Vec<NodeOp>, Vec<String>)> {
    let node = scene.stored(id)?;
    if !matches!(node.kind, NodeKind::Group | NodeKind::Frame) {
        return None;
    }
    let parent = node.parent.clone()?;
    let laid = lays_out(scene, id);
    let joins = lays_out(scene, &parent);
    let origin = placed.origin(&parent);
    let fit = fit_of(scene, placed, &parent);
    // Where its children count from on the page: as placed, or (hidden) at its own `x`/`y`.
    let inner = if placed.rect(id).is_some() { placed.origin(id) } else { (origin.0 + node.x, origin.1 + node.y) };
    let start = scene.index(id)? as u32;
    let mut ops = Vec::new();
    let kids = scene.children(id).to_vec();
    for (index, child) in (start..).zip(&kids) {
        let c = scene.stored(child)?;
        let s = match shown(placed, child, origin) {
            Some(rel) if joins && !c.is_absolute() => set(&[("x", json!(coord(rel.x))), ("y", json!(coord(rel.y)))]),
            Some(rel) => settle(c, rel, fit, stretched(scene, child), Sizes::Freeze),
            None => {
                // Hidden: at its own `x`/`y` in a group or a free frame; in a laid-out frame,
                // whose layout never read them, at its content corner (B113).
                let (x, y) = if laid {
                    let [top, _, _, left] = node.padding_edges();
                    (inner.0 + left, inner.1 + top)
                } else {
                    (inner.0 + c.x, inner.1 + c.y)
                };
                let r = unpinned(c, fit.filter(|_| !joins), Rect::new(x - origin.0, y - origin.1, c.w, c.h));
                set(&[("x", json!(coord(r.x))), ("y", json!(coord(r.y)))])
            }
        };
        ops.push(NodeOp::Move { id: child.clone(), parent: Some(parent.clone()), index: Some(index), keep_position: None });
        ops.push(NodeOp::Update { id: child.clone(), set: s });
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
/// lands on the page) and as big ([`freeze`]: one leaving a layout keeps its box), written
/// through `parent`'s pins. Into a parent that lays it out it only moves.
pub fn reparent(scene: &Scene, placed: &Placed, id: &str, parent: &str, index: Option<u32>, at: (f64, f64)) -> Vec<NodeOp> {
    let mut ops = vec![NodeOp::Move { id: id.to_string(), parent: Some(parent.to_string()), index, keep_position: None }];
    let Some(n) = scene.stored(id).filter(|n| n.is_absolute() || !lays_out(scene, parent)) else {
        return ops;
    };
    let origin = placed.origin(parent);
    // A group's `x`/`y` is its own point, not its box (its children's bounds).
    let (ox, oy) = placed.point(id);
    let s = match placed.rect(id) {
        Some(r) => {
            let rel = Rect::new(at.0 + ox - r.x - origin.0, at.1 + oy - r.y - origin.1, r.w, r.h);
            settle(n, rel, fit_of(scene, placed, parent), stretched(scene, id), Sizes::Freeze)
        }
        None => set(&[("x", json!(coord(at.0 - origin.0))), ("y", json!(coord(at.1 - origin.1)))]),
    };
    ops.push(NodeOp::Update { id: id.to_string(), set: s });
    ops
}

/// Turn auto layout on `frames` to `mode` (`None`: off), as the context bar's layout key does.
/// On for the first time, a frame gets a 12 px gap and 16 px padding where it names none, so
/// its children do not touch its edges. Off, each child it laid out keeps the box it shows
/// ([`freeze`]: a filling one its width) and the frame its size where it hugged them (TM-8):
/// `layout: null` alone drops them at their stale `x`/`y` and sizes, a filling child at its
/// stored width, often 0, and a hugging frame at its stored size. A hidden child goes to the
/// frame's content corner. A frame not placed (hidden) only loses its layout.
pub fn set_layout(scene: &Scene, placed: &Placed, frames: &[String], mode: Option<Layout>) -> Vec<NodeOp> {
    let mut ops = Vec::new();
    for id in frames {
        let Some(f) = scene.stored(id).filter(|f| f.kind == NodeKind::Frame) else {
            continue;
        };
        if let Some(mode) = mode.filter(|m| *m != Layout::None) {
            let mut s = set(&[("layout", json!(mode))]);
            if f.layout.is_none() {
                if f.gap.is_none() {
                    s.insert("gap".into(), json!(12));
                }
                if f.padding.is_none() {
                    s.insert("padding".into(), json!([16]));
                }
            }
            ops.push(NodeOp::Update { id: id.clone(), set: s });
            continue;
        }
        if f.layout.is_none() {
            continue;
        }
        let mut s = set(&[("layout", Value::Null)]);
        let Some(r) = placed.rect(id).filter(|_| f.layout_mode() != Layout::None) else {
            ops.push(NodeOp::Update { id: id.clone(), set: s });
            continue;
        };
        // Its size as stored from now on: a hug of its children is fixed at what it showed.
        let mut stored = (f.w, f.h);
        for (key, rule_key, rule, size, at) in [("w", "w_sizing", f.w_rule(), f.clamp_w(r.w.round()), &mut stored.0), ("h", "h_sizing", f.h_rule(), f.clamp_h(r.h.round()), &mut stored.1)] {
            if rule == Sizing::Hug {
                *at = size.clamp(0.0, MAX_COORD);
                s.insert(key.into(), json!(*at));
                s.insert(rule_key.into(), json!("fixed"));
            }
        }
        ops.push(NodeOp::Update { id: id.clone(), set: s });
        let origin = placed.origin(id);
        let fit = Some(Fit { stored, placed: (r.w, r.h) });
        let [top, _, _, left] = f.padding_edges();
        for child in scene.children(id) {
            let Some(c) = scene.stored(child).filter(|c| !c.is_absolute()) else {
                continue;
            };
            let s = match shown(placed, child, origin) {
                Some(rel) => settle(c, rel, fit, stretched(scene, child), Sizes::Freeze),
                None => {
                    let r = unpinned(c, fit, Rect::new(left, top, c.w, c.h));
                    set(&[("x", json!(coord(r.x))), ("y", json!(coord(r.y)))])
                }
            };
            ops.push(NodeOp::Update { id: child.clone(), set: s });
        }
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

/// Move the selection by `(dx, dy)` page pixels, each node as far as it shows: along a `scale`
/// pin its stored offset moves by the frame's stored size over its placed one
/// (`layout::stored_shift`). A node its parent lays out stays put; an `absolute` one moves.
pub fn nudge(scene: &Scene, placed: &Placed, ids: &[String], dx: f64, dy: f64) -> Vec<NodeOp> {
    outermost(scene, ids)
        .into_iter()
        .filter(|id| scene.parent(id).is_some() && !is_laid(scene, id))
        .filter_map(|id| {
            let n = scene.stored(&id)?;
            let (sx, sy) = stored_shift(n, n.parent.as_deref().and_then(|p| fit_of(scene, placed, p)), dx, dy);
            let mut s = Map::new();
            if dx != 0.0 {
                s.insert("x".into(), json!(coord(n.x + sx)));
            }
            if dy != 0.0 {
                s.insert("y".into(), json!(coord(n.y + sy)));
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

/// Align the selection to its own bounds (or one node to its parent). A node its parent lays
/// out stays put; an `absolute` one moves.
pub fn align(scene: &Scene, placed: &Placed, ids: &[String], how: Align) -> Vec<NodeOp> {
    let ids: Vec<String> = outermost(scene, ids).into_iter().filter(|id| scene.parent(id).is_some() && !is_laid(scene, id)).collect();
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
        .flat_map(|(id, dx, dy)| nudge(scene, placed, &[id], dx, dy))
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
    use relay_bus::design::{NodeKind as K, PinX};

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
        let placed = place(&s, "p1", &Mono);
        let ops = nudge(&s, &placed, &["b".into()], 1.0, -10.0);
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

    /// SKILL.md's login screen: a 390 px column, padding 24 at the sides, holding a hugging
    /// title, a filling paragraph, a filling field 52 high and a filling button that hugs its
    /// label's height.
    fn login() -> Scene {
        let nodes: Vec<Node> = serde_json::from_value(json!([
            {"id": "p1", "kind": "page"},
            {"id": "login", "parent": "p1", "kind": "frame", "x": 0, "y": 0, "w": 390, "h": 844,
                "layout": "column", "gap": 16, "padding": [72, 24, 32, 24]},
            {"id": "login-title", "parent": "login", "kind": "text", "text": "Welcome back", "font_size": 30},
            {"id": "login-hint", "parent": "login", "kind": "text", "w_sizing": "fill",
                "text": "Sign in to pick up where your agents left off."},
            {"id": "email", "parent": "login", "kind": "frame", "w_sizing": "fill", "h": 52, "layout": "row",
                "items": "center", "padding": [0, 16]},
            {"id": "email-text", "parent": "email", "kind": "text", "text": "you@example.com"},
            {"id": "login-button", "parent": "login", "kind": "frame", "w_sizing": "fill", "h_sizing": "hug",
                "layout": "row", "justify": "center", "padding": [16]},
            {"id": "login-button-label", "parent": "login-button", "kind": "text", "text": "Continue"},
        ]))
        .unwrap();
        Scene::new(nodes)
    }

    /// Each of `ids` is placed in `after` as in `before`.
    fn kept(before: &Placed, after: &Placed, ids: &[&str]) {
        for id in ids {
            assert_eq!(after.rect(id), before.rect(id), "{id} keeps its box");
        }
    }

    const LOGIN: [&str; 6] = ["login-title", "login-hint", "email", "email-text", "login-button", "login-button-label"];

    /// TM-8: a filling field dragged out of its column keeps the width it showed.
    #[test]
    fn a_field_dragged_out_of_its_column_keeps_its_width() {
        let mut s = login();
        let before = place(&s, "p1", &Mono);
        assert_eq!(before.rect("email").map(|r| (r.w, r.h)), Some((342.0, 52.0)));
        let at = before.rect("email").map(|r| (r.x, r.y)).unwrap();
        let ops = reparent(&s, &before, "email", "p1", None, at);
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        kept(&before, &after, &["email", "email-text"]);
        let email = s.node("email").unwrap();
        assert_eq!((email.w, email.w_sizing), (342.0, Some(Sizing::Fixed)));
    }

    /// TM-8: with its layout turned off, every child keeps the box the layout gave it; the
    /// title and the button keep hugging what they hold.
    #[test]
    fn turning_layout_off_keeps_every_childs_box() {
        let mut s = login();
        let before = place(&s, "p1", &Mono);
        let ops = set_layout(&s, &before, &ids(&["login"]), None);
        run(&mut s, &ops);
        assert_eq!(s.node("login").unwrap().layout, None);
        let after = place(&s, "p1", &Mono);
        kept(&before, &after, &LOGIN);
        let (title, button) = (s.node("login-title").unwrap(), s.node("login-button").unwrap());
        assert_eq!((title.w_rule(), button.h_rule(), button.w_rule()), (Sizing::Hug, Sizing::Hug, Sizing::Fixed));
        // A frame that hugged its children keeps the size they gave it.
        let mut s = login();
        let ops = set_layout(&s, &before, &ids(&["login-button"]), None);
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        kept(&before, &after, &["login-button", "login-button-label"]);
        assert_eq!(s.node("login-button").unwrap().h_sizing, Some(Sizing::Fixed));
        // On again, a gap and padding come only the first time, where it names none.
        let ops = set_layout(&s, &after, &ids(&["login-button", "email-text"]), Some(Layout::Column));
        assert_eq!(ops.len(), 1, "a text has no layout");
        assert!(matches!(&ops[0], NodeOp::Update { set, .. } if set.len() == 2 && set["gap"] == json!(12)), "it has padding: {ops:?}");
        let ops = set_layout(&s, &after, &ids(&["email"]), Some(Layout::Column));
        assert!(matches!(&ops[0], NodeOp::Update { set, .. } if set.len() == 1), "{ops:?}");
        let ops = set_layout(&Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), node("f", Some("p1"), K::Frame, [0.0; 4])]), &after, &ids(&["f"]), Some(Layout::Wrap));
        assert!(matches!(&ops[0], NodeOp::Update { set, .. } if set["layout"] == json!("wrap") && set["padding"] == json!([16])), "{ops:?}");
    }

    /// TM-8: ungrouping a column that stretches its children keeps their widths: a filling
    /// frame, a stretched fixed rect and a filling paragraph.
    #[test]
    fn ungrouping_a_stretch_column_keeps_the_widths() {
        let mut col = node("col", Some("p1"), K::Frame, [40.0, 40.0, 180.0, 300.0]);
        col.layout = Some(Layout::Column);
        col.items = Some(Items::Stretch);
        let mut fill = node("f", Some("col"), K::Frame, [0.0, 0.0, 0.0, 20.0]);
        fill.w_sizing = Some(Sizing::Fill);
        let mut para = node("t", Some("col"), K::Text, [0.0; 4]);
        para.w_sizing = Some(Sizing::Fill);
        para.text = Some("a paragraph long enough to wrap twice".into());
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            col,
            fill,
            node("r", Some("col"), K::Rect, [0.0, 0.0, 20.0, 10.0]),
            para,
            node("hug", Some("col"), K::Text, [0.0; 4]),
        ]);
        let before = place(&s, "p1", &Mono);
        for id in ["f", "r", "t", "hug"] {
            assert_eq!(before.rect(id).unwrap().w, 180.0, "{id} is stretched");
        }
        let (ops, _) = ungroup(&s, &before, "col").unwrap();
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        kept(&before, &after, &["f", "r", "t", "hug"]);
        // The hugging text the column stretched wraps at the width it was given.
        assert_eq!(s.node("hug").unwrap().w_sizing, Some(Sizing::Fixed));
    }

    /// TM-8: a frame made of a column's children keeps each one's box.
    #[test]
    fn framing_laid_children_keeps_their_boxes() {
        let mut s = login();
        let before = place(&s, "p1", &Mono);
        let (ops, fid) = group(&mut s, &before, &ids(&["login-hint", "email"]), K::Frame).unwrap();
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        for id in ["login-hint", "email"] {
            assert_eq!(after.rect(id).map(|r| (r.w, r.h)), before.rect(id).map(|r| (r.w, r.h)), "{id}");
        }
        let f = after.rect(&fid).unwrap();
        assert_eq!(after.rect("email").map(|r| (r.x - f.x, r.y - f.y)), Some((0.0, 56.0)), "where the column had put it");
    }

    /// B113: a hidden child of a laid-out frame, which the layout never placed, lands at the
    /// frame's content corner, not at the page's origin.
    #[test]
    fn ungrouping_a_laid_out_frame_keeps_hidden_children_near_it() {
        let mut col = node("col", Some("outer"), K::Frame, [100.0, 100.0, 200.0, 200.0]);
        col.layout = Some(Layout::Column);
        col.padding = Some(vec![8.0]);
        let mut hidden = node("r2", Some("col"), K::Rect, [0.0, 0.0, 10.0, 10.0]);
        hidden.hidden = Some(true);
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            node("outer", Some("p1"), K::Frame, [50.0, 50.0, 400.0, 400.0]),
            col,
            node("r1", Some("col"), K::Rect, [0.0, 0.0, 10.0, 10.0]),
            hidden,
        ]);
        let before = place(&s, "p1", &Mono);
        let (ops, _) = ungroup(&s, &before, "col").unwrap();
        run(&mut s, &ops);
        let after = place(&s, "p1", &Mono);
        kept(&before, &after, &["r1"]);
        let r2 = s.node("r2").unwrap();
        assert_eq!((r2.parent.as_deref(), r2.x, r2.y), (Some("outer"), 108.0, 108.0));
        // A hidden child of a group sits at the group's point plus its own.
        let mut s = nested();
        s.node_mut("r").unwrap().hidden = Some(true);
        let placed = place(&s, "p1", &Mono);
        let (ops, _) = ungroup(&s, &placed, "g2").unwrap();
        run(&mut s, &ops);
        assert_eq!(s.node("r").map(|r| (r.x, r.y)), Some((15.0, 15.0)));
    }

    /// A column whose card fills it: stored 200 wide, drawn 400, with a child pinned right.
    fn filling_card() -> Scene {
        let mut col = node("col", Some("p1"), K::Frame, [0.0, 0.0, 400.0, 300.0]);
        col.layout = Some(Layout::Column);
        let mut card = node("card", Some("col"), K::Frame, [0.0, 0.0, 200.0, 100.0]);
        card.w_sizing = Some(Sizing::Fill);
        let mut right = node("r", Some("p1"), K::Rect, [500.0, 0.0, 40.0, 20.0]);
        right.pin_x = Some(PinX::Right);
        let mut wide = node("s", Some("p1"), K::Rect, [500.0, 50.0, 300.0, 20.0]);
        wide.pin_x = Some(PinX::LeftRight);
        let mut scaled = node("k", Some("card"), K::Rect, [10.0, 10.0, 20.0, 20.0]);
        scaled.pin_x = Some(PinX::Scale);
        Scene::new(vec![node("p1", None, K::Page, [0.0; 4]), col, card, scaled, right, wide])
    }

    #[test]
    fn reparent_into_a_filling_frame_writes_stored_place_for_a_pinned_child() {
        let mut s = filling_card();
        let placed = place(&s, "p1", &Mono);
        assert_eq!(placed.rect("card").map(|r| r.w), Some(400.0));
        let mut ops = reparent(&s, &placed, "r", "card", None, (300.0, 10.0));
        ops.extend(reparent(&s, &placed, "s", "card", None, (50.0, 50.0)));
        run(&mut s, &ops);
        // Stored as at the card's stored width: 200 to the left of where it shows.
        assert_eq!(s.node("r").map(|r| (r.x, r.w)), Some((100.0, 40.0)));
        assert_eq!(s.node("s").map(|r| (r.x, r.w)), Some((50.0, 100.0)));
        let after = place(&s, "p1", &Mono);
        assert_eq!(after.rect("r"), Some(Rect::new(300.0, 10.0, 40.0, 20.0)));
        assert_eq!(after.rect("s"), Some(Rect::new(50.0, 50.0, 300.0, 20.0)));
    }

    #[test]
    fn nudge_and_align_move_absolute_children_of_a_laid_frame() {
        let mut s = filling_card();
        let mut badge = node("badge", Some("col"), K::Rect, [10.0, 10.0, 20.0, 20.0]);
        badge.absolute = Some(true);
        s.apply(&NodeOp::Add { node: badge, index: None }).unwrap();
        let placed = place(&s, "p1", &Mono);
        let ops = nudge(&s, &placed, &ids(&["card", "badge"]), 5.0, 0.0);
        assert!(matches!(&ops[..], [NodeOp::Update { id, set }] if id == "badge" && set["x"] == json!(15.0)), "{ops:?}");
        // The card stays where its column put it; the badge, alone, aligns to its frame.
        let ops = align(&s, &placed, &ids(&["card", "badge"]), Align::Left);
        assert!(matches!(&ops[..], [NodeOp::Update { id, set }] if id == "badge" && set["x"] == json!(0.0)), "{ops:?}");
        let ops = align(&s, &placed, &ids(&["badge"]), Align::Right);
        run(&mut s, &ops);
        assert_eq!(place(&s, "p1", &Mono).rect("badge").map(|r| r.right()), Some(400.0));
        // Along a scale pin, the stored offset moves by the card's stored width over its drawn.
        let ops = nudge(&s, &placed, &ids(&["k"]), 10.0, 0.0);
        run(&mut s, &ops);
        assert_eq!(s.node("k").unwrap().x, 15.0);
        assert_eq!(place(&s, "p1", &Mono).rect("k").map(|r| r.x), placed.rect("k").map(|r| r.x + 10.0));
    }

    /// A hugging card with auto layout, holding an absolute badge pinned left and right: the
    /// card is drawn wider than it stores, so the badge is too.
    #[test]
    fn ungroup_of_a_filling_frame_keeps_pinned_children_where_they_show() {
        let mut card = node("card", Some("p1"), K::Frame, [20.0, 20.0, 100.0, 0.0]);
        card.layout = Some(Layout::Column);
        card.w_sizing = Some(Sizing::Hug);
        card.h_sizing = Some(Sizing::Hug);
        let mut badge = node("badge", Some("card"), K::Rect, [10.0, 5.0, 20.0, 10.0]);
        badge.absolute = Some(true);
        badge.pin_x = Some(PinX::LeftRight);
        let mut s = Scene::new(vec![
            node("p1", None, K::Page, [0.0; 4]),
            card,
            node("r", Some("card"), K::Rect, [0.0, 0.0, 200.0, 50.0]),
            badge,
        ]);
        let before = place(&s, "p1", &Mono);
        assert_eq!(before.rect("badge").map(|r| r.w), Some(120.0));
        let (ops, _) = ungroup(&s, &before, "card").unwrap();
        run(&mut s, &ops);
        kept(&before, &place(&s, "p1", &Mono), &["r", "badge"]);
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
