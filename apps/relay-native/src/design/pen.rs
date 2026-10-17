//! The vector pen and point editing. The pen places anchors with clicks (a corner) or drags
//! (pulling out curve handles; Alt breaks their symmetry); clicking the first anchor closes
//! the shape, Enter or Escape leaves it open. Double-clicking a vector edits its points: drag
//! an anchor and its handles move with it, drag a handle to bend the curve. Any shape can be
//! turned into a vector to edit it this way.

use super::board::{Board, Gesture, Tool, SELECT};
use super::paint::rgba;
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp};
use relay_bus::svgpath::Seg;
use relay_client::design::layout::Placed;
use relay_client::design::{shapes, Rect, Scene};
use serde_json::{json, Map};

pub type Pt = (f64, f64);

/// One point of a path with the handles into and out of it (equal to the point: a corner).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub p: Pt,
    pub cin: Pt,
    pub cout: Pt,
}

impl Anchor {
    fn corner(p: Pt) -> Anchor {
        Anchor { p, cin: p, cout: p }
    }
}

/// Subpaths as anchors, each with whether it is closed.
pub type Subpaths = Vec<(Vec<Anchor>, bool)>;

/// A path under way with the pen, in page coordinates.
#[derive(Debug, Clone)]
pub struct PenPath {
    pub parent: String,
    pub anchors: Vec<Anchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Anchor,
    In,
    Out,
}

/// A point of a vector being edited: which subpath, which anchor, which part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointRef {
    pub sub: usize,
    pub index: usize,
    pub part: Part,
}

/// Subpaths as anchors, and whether each is closed.
pub fn anchors_of(segs: &[Seg]) -> Subpaths {
    let mut out: Subpaths = Vec::new();
    for s in segs {
        match *s {
            Seg::Move([x, y]) => out.push((vec![Anchor::corner((x, y))], false)),
            Seg::Line([x, y]) => {
                if let Some((a, _)) = out.last_mut() {
                    a.push(Anchor::corner((x, y)));
                }
            }
            Seg::Cubic(c1, c2, [x, y]) => {
                if let Some((a, _)) = out.last_mut() {
                    if let Some(last) = a.last_mut() {
                        last.cout = (c1[0], c1[1]);
                    }
                    a.push(Anchor { p: (x, y), cin: (c2[0], c2[1]), cout: (x, y) });
                }
            }
            Seg::Close => {
                if let Some((a, closed)) = out.last_mut() {
                    *closed = true;
                    // A closing segment drawn out to the first point: one anchor, not two.
                    if a.len() > 1 {
                        let (first, last) = (a[0], a[a.len() - 1]);
                        if (first.p.0 - last.p.0).abs() < 1e-6 && (first.p.1 - last.p.1).abs() < 1e-6 {
                            a[0].cin = last.cin;
                            a.pop();
                        }
                    }
                }
            }
        }
    }
    out
}

fn seg_to(from: &Anchor, to: &Anchor) -> Seg {
    if from.cout == from.p && to.cin == to.p {
        Seg::Line([to.p.0, to.p.1])
    } else {
        Seg::Cubic([from.cout.0, from.cout.1], [to.cin.0, to.cin.1], [to.p.0, to.p.1])
    }
}

/// Anchors back into segments.
pub fn segments(subs: &[(Vec<Anchor>, bool)]) -> Vec<Seg> {
    let mut out = Vec::new();
    for (a, closed) in subs {
        let Some(first) = a.first() else { continue };
        out.push(Seg::Move([first.p.0, first.p.1]));
        for w in a.windows(2) {
            out.push(seg_to(&w[0], &w[1]));
        }
        if *closed && a.len() > 1 {
            let last = &a[a.len() - 1];
            if last.cout != last.p || first.cin != first.p {
                out.push(seg_to(last, first));
            }
            out.push(Seg::Close);
        }
    }
    out
}

/// A shape's outline as path segments in page coordinates, to turn it into a vector.
pub fn outline_of(n: &Node, r: &Rect) -> Option<Vec<Seg>> {
    let k = 0.552_284_75;
    Some(match n.kind {
        NodeKind::Rect | NodeKind::Frame | NodeKind::Sticky | NodeKind::Image => {
            let rad = n.radius.unwrap_or(0.0).min(r.w / 2.0).min(r.h / 2.0);
            if rad <= 0.0 {
                vec![Seg::Move([r.x, r.y]), Seg::Line([r.right(), r.y]), Seg::Line([r.right(), r.bottom()]), Seg::Line([r.x, r.bottom()]), Seg::Close]
            } else {
                let c = rad * (1.0 - k);
                vec![
                    Seg::Move([r.x + rad, r.y]),
                    Seg::Line([r.right() - rad, r.y]),
                    Seg::Cubic([r.right() - c, r.y], [r.right(), r.y + c], [r.right(), r.y + rad]),
                    Seg::Line([r.right(), r.bottom() - rad]),
                    Seg::Cubic([r.right(), r.bottom() - c], [r.right() - c, r.bottom()], [r.right() - rad, r.bottom()]),
                    Seg::Line([r.x + rad, r.bottom()]),
                    Seg::Cubic([r.x + c, r.bottom()], [r.x, r.bottom() - c], [r.x, r.bottom() - rad]),
                    Seg::Line([r.x, r.y + rad]),
                    Seg::Cubic([r.x, r.y + c], [r.x + c, r.y], [r.x + rad, r.y]),
                    Seg::Close,
                ]
            }
        }
        NodeKind::Ellipse => {
            let (cx, cy, rx, ry) = (r.cx(), r.cy(), r.w / 2.0, r.h / 2.0);
            vec![
                Seg::Move([cx, cy - ry]),
                Seg::Cubic([cx + rx * k, cy - ry], [cx + rx, cy - ry * k], [cx + rx, cy]),
                Seg::Cubic([cx + rx, cy + ry * k], [cx + rx * k, cy + ry], [cx, cy + ry]),
                Seg::Cubic([cx - rx * k, cy + ry], [cx - rx, cy + ry * k], [cx - rx, cy]),
                Seg::Cubic([cx - rx, cy - ry * k], [cx - rx * k, cy - ry], [cx, cy - ry]),
                Seg::Close,
            ]
        }
        NodeKind::Polygon => {
            let pts = shapes::node_polygon(n, r);
            let mut out = vec![Seg::Move([pts.first()?.0, pts.first()?.1])];
            out.extend(pts[1..].iter().map(|p| Seg::Line([p.0, p.1])));
            out.push(Seg::Close);
            out
        }
        NodeKind::Line | NodeKind::Path => {
            let pts = n.points.as_deref()?;
            let at = |p: &[f64; 2]| [r.x + p[0], r.y + p[1]];
            let mut out = vec![Seg::Move(at(pts.first()?))];
            if n.kind == NodeKind::Path && pts.len() > 2 {
                // The canvas's smoothing (quadratics through the midpoints) as cubics.
                let mut cur = at(&pts[0]);
                for w in pts.windows(2).skip(1) {
                    let (c, nx) = (at(&w[0]), at(&w[1]));
                    let e = [(c[0] + nx[0]) / 2.0, (c[1] + nx[1]) / 2.0];
                    let c1 = [cur[0] + 2.0 / 3.0 * (c[0] - cur[0]), cur[1] + 2.0 / 3.0 * (c[1] - cur[1])];
                    let c2 = [e[0] + 2.0 / 3.0 * (c[0] - e[0]), e[1] + 2.0 / 3.0 * (c[1] - e[1])];
                    out.push(Seg::Cubic(c1, c2, e));
                    cur = e;
                }
                out.push(Seg::Line(at(pts.last()?)));
            } else {
                out.extend(pts[1..].iter().map(|p| Seg::Line(at(p))));
            }
            out
        }
        NodeKind::Vector => shapes::vector_segments(n, r),
        _ => return None,
    })
}

/// The fields that make `n` show `segs` (page coordinates) inside a parent at `origin`.
fn vector_set(segs: &[Seg], origin: Pt) -> Option<Map<String, serde_json::Value>> {
    let (rect, d, view) = shapes::vector_from(segs, origin)?;
    let mut set = Map::new();
    set.insert("x".into(), json!((rect.x * 100.0).round() / 100.0));
    set.insert("y".into(), json!((rect.y * 100.0).round() / 100.0));
    set.insert("w".into(), json!((rect.w * 100.0).round() / 100.0));
    set.insert("h".into(), json!((rect.h * 100.0).round() / 100.0));
    set.insert("d".into(), json!(d));
    set.insert("view".into(), json!([(view[0] * 100.0).round() / 100.0, (view[1] * 100.0).round() / 100.0]));
    Some(set)
}

const NEAR: f64 = 7.0;

impl Board {
    fn origin_of(&self, parent: &str) -> Pt {
        if *self.page.borrow() == parent {
            return (0.0, 0.0);
        }
        self.placed().rect(parent).map_or((0.0, 0.0), |r| (r.x, r.y))
    }

    /// A press with the vector pen at `p` (page coordinates).
    pub(super) fn pen_press(&self, p: Pt, parent: String) {
        let zoom = self.cam.get().zoom;
        let mut pen = self.extra.pen.borrow_mut();
        match pen.as_mut() {
            None => *pen = Some(PenPath { parent, anchors: vec![Anchor::corner(p)] }),
            Some(path) => {
                let first = path.anchors[0].p;
                if path.anchors.len() > 1 && ((first.0 - p.0).powi(2) + (first.1 - p.1).powi(2)).sqrt() * zoom <= NEAR {
                    drop(pen);
                    self.commit_pen(true);
                    return;
                }
                path.anchors.push(Anchor::corner(p));
            }
        }
        drop(pen);
        *self.gesture.borrow_mut() = Gesture::PenHandle { anchor: p };
        self.canvas.queue_draw();
    }

    /// Dragging out of the anchor just placed pulls its handles.
    pub(super) fn pen_drag(&self, anchor: Pt, p: Pt, alt: bool) {
        if let Some(a) = self.extra.pen.borrow_mut().as_mut().and_then(|path| path.anchors.last_mut()) {
            a.cout = p;
            if !alt {
                a.cin = (2.0 * anchor.0 - p.0, 2.0 * anchor.1 - p.1);
            }
            if ((p.0 - anchor.0).powi(2) + (p.1 - anchor.1).powi(2)).sqrt() * self.cam.get().zoom < 2.0 {
                *a = Anchor::corner(anchor);
            }
        }
        self.canvas.queue_draw();
    }

    /// Leave the pen's path open, as it stands.
    pub(super) fn finish_pen(&self) {
        if self.extra.pen.borrow().is_some() {
            self.commit_pen(false);
        }
    }

    fn commit_pen(&self, closed: bool) {
        let Some(path) = self.extra.pen.borrow_mut().take() else {
            return;
        };
        self.canvas.queue_draw();
        if path.anchors.len() < 2 {
            return;
        }
        let segs = segments(&[(path.anchors, closed)]);
        let Some(set) = vector_set(&segs, self.origin_of(&path.parent)) else {
            return;
        };
        let mut n = Node::new(NodeKind::Vector);
        n.id = self.doc.scene.borrow_mut().fresh_id(NodeKind::Vector);
        n.parent = Some(path.parent);
        n.x = set["x"].as_f64().unwrap_or(0.0);
        n.y = set["y"].as_f64().unwrap_or(0.0);
        n.w = set["w"].as_f64().unwrap_or(1.0);
        n.h = set["h"].as_f64().unwrap_or(1.0);
        n.d = set["d"].as_str().map(str::to_string);
        n.view = serde_json::from_value(set["view"].clone()).ok();
        n.stroke = Some(self.pen_color(false));
        n.stroke_width = Some(2.0);
        if closed {
            n.fill = Some("#D9D9D9".into());
        } else {
            n.open = Some(true);
        }
        let id = n.id.clone();
        if self.commit(vec![NodeOp::Add { node: n, index: None }]) {
            self.select(vec![id]);
            if !self.tool_lock.get() {
                self.set_tool(Tool::Select);
            }
        }
    }

    /// Edit the points of a vector (turning another shape into one first).
    pub(super) fn edit_points(&self, id: &str) {
        let Some(n) = self.doc.scene.borrow().node(id).cloned() else {
            return;
        };
        if n.kind != NodeKind::Vector {
            self.convert_to_vector(id);
        }
        self.end_edit(true);
        *self.extra.vector_edit.borrow_mut() = Some(id.to_string());
        self.select(vec![id.to_string()]);
        self.toast("Drag a point or a handle · Enter or Escape when done");
        self.canvas.queue_draw();
    }

    pub(super) fn stop_editing_points(&self) -> bool {
        let was = self.extra.vector_edit.borrow_mut().take().is_some();
        if was {
            self.canvas.queue_draw();
        }
        was
    }

    /// Replace a shape by a vector of its outline, keeping its look.
    pub(super) fn convert_to_vector(&self, id: &str) {
        let placed = self.placed();
        let (Some(n), Some(r)) = (self.doc.scene.borrow().node(id).cloned(), placed.rect(id)) else {
            return;
        };
        if matches!(n.kind, NodeKind::Vector | NodeKind::Page | NodeKind::Group | NodeKind::Text | NodeKind::Comment) {
            return;
        }
        let Some(segs) = outline_of(&n, &r) else {
            return;
        };
        let origin = n.parent.as_deref().map_or((0.0, 0.0), |p| self.origin_of(p));
        let Some(set) = vector_set(&segs, origin) else {
            return;
        };
        let mut v = n.clone();
        v.kind = NodeKind::Vector;
        v.d = set["d"].as_str().map(str::to_string);
        v.view = serde_json::from_value(set["view"].clone()).ok();
        (v.x, v.y, v.w, v.h) = (set["x"].as_f64().unwrap_or(n.x), set["y"].as_f64().unwrap_or(n.y), set["w"].as_f64().unwrap_or(n.w), set["h"].as_f64().unwrap_or(n.h));
        if matches!(n.kind, NodeKind::Line | NodeKind::Path) {
            v.open = Some(true);
            v.stroke = n.stroke.clone().or(Some(super::paint::STROKE.into()));
            v.stroke_width = n.stroke_width.or(Some(2.0));
        } else {
            v.fill = super::paint::fill_of(&n);
        }
        (v.points, v.sides, v.star, v.radius, v.radii, v.text, v.arrow_start, v.arrow_end, v.from, v.to, v.route) =
            (None, None, None, None, None, None, None, None, None, None, None);
        (v.layout, v.gap, v.padding, v.justify, v.items, v.clip) = (None, None, None, None, None, None);
        let index = {
            let scene = self.doc.scene.borrow();
            n.parent.as_deref().and_then(|p| scene.children(p).iter().position(|c| c == id)).map(|i| i as u32)
        };
        if self.doc.scene.borrow().children(id).is_empty() || !n.kind.holds_children() {
            self.commit(vec![NodeOp::Delete { id: id.to_string() }, NodeOp::Add { node: v, index }]);
            self.select(vec![id.to_string()]);
        }
    }

    /// The edited vector's subpaths in page coordinates, unturned.
    fn edit_anchors(&self, id: &str) -> Option<(Node, Rect, Subpaths)> {
        let n = self.doc.scene.borrow().node(id).cloned()?;
        let r = self.placed().rect(id)?;
        let subs = anchors_of(&shapes::vector_segments(&n, &r));
        Some((n, r, subs))
    }

    /// The edited vector's point under screen point `(x, y)`: handles of every anchor first,
    /// then the anchors.
    pub(super) fn point_at(&self, x: f64, y: f64) -> Option<(String, PointRef)> {
        let id = self.extra.vector_edit.borrow().clone()?;
        let (n, r, subs) = self.edit_anchors(&id)?;
        let cam = self.cam.get();
        let turn = shapes::rotation(&n);
        let (px, py) = cam.to_page(x, y);
        let (px, py) = shapes::rotate(px, py, r.cx(), r.cy(), -turn);
        let (x, y) = cam.to_screen(px, py);
        let near = |p: Pt| {
            let (sx, sy) = cam.to_screen(p.0, p.1);
            (sx - x).abs() <= NEAR && (sy - y).abs() <= NEAR
        };
        for (si, (a, _)) in subs.iter().enumerate() {
            for (i, an) in a.iter().enumerate() {
                for (part, at) in [(Part::Out, an.cout), (Part::In, an.cin)] {
                    if at != an.p && near(at) {
                        return Some((id.clone(), PointRef { sub: si, index: i, part }));
                    }
                }
            }
        }
        for (si, (a, _)) in subs.iter().enumerate() {
            for (i, an) in a.iter().enumerate() {
                if near(an.p) {
                    return Some((id, PointRef { sub: si, index: i, part: Part::Anchor }));
                }
            }
        }
        None
    }

    /// Drag `point` of the vector `orig` to page point `p`: the node changes live.
    pub(super) fn drag_point(&self, id: &str, point: PointRef, orig: &Node, r: &Rect, p: Pt, alt: bool) {
        let r = *r;
        let (px, py) = shapes::rotate(p.0, p.1, r.cx(), r.cy(), -shapes::rotation(orig));
        let mut subs = anchors_of(&shapes::vector_segments(orig, &r));
        let Some(an) = subs.get_mut(point.sub).and_then(|(a, _)| a.get_mut(point.index)) else {
            return;
        };
        match point.part {
            Part::Anchor => {
                let (dx, dy) = (px - an.p.0, py - an.p.1);
                an.p = (px, py);
                an.cin = (an.cin.0 + dx, an.cin.1 + dy);
                an.cout = (an.cout.0 + dx, an.cout.1 + dy);
            }
            Part::Out => {
                an.cout = (px, py);
                if !alt && an.cin != an.p {
                    an.cin = (2.0 * an.p.0 - px, 2.0 * an.p.1 - py);
                }
            }
            Part::In => {
                an.cin = (px, py);
                if !alt && an.cout != an.p {
                    an.cout = (2.0 * an.p.0 - px, 2.0 * an.p.1 - py);
                }
            }
        }
        let origin = orig.parent.as_deref().map_or((0.0, 0.0), |p| self.origin_of(p));
        let Some(set) = vector_set(&segments(&subs), origin) else { return };
        let Ok(next) = orig.patched(&set) else { return };
        if let Some(n) = self.doc.scene.borrow_mut().node_mut(id) {
            *n = next;
        }
        self.invalidate();
    }

    /// The pen's path so far, the edited vector's points, drawn over the canvas.
    pub(super) fn pen_overlay(&self, snap: &gtk::Snapshot, _scene: &Scene, _placed: &Placed) {
        let cam = self.cam.get();
        let scr = |p: Pt| {
            let (x, y) = cam.to_screen(p.0, p.1);
            (x as f32, y as f32)
        };
        let blue = rgba(SELECT);
        if let Some(path) = self.extra.pen.borrow().as_ref() {
            let segs = segments(&[(path.anchors.clone(), false)]);
            let pb = gsk::PathBuilder::new();
            add_screen(&pb, &segs, &scr);
            // From the last anchor to the pointer, as the next click would make it.
            if matches!(*self.gesture.borrow(), Gesture::Idle) {
                if let Some(last) = path.anchors.last() {
                    let (px, py) = self.pointer.get();
                    let (lx, ly) = scr(last.p);
                    let (cx, cy) = scr(last.cout);
                    pb.move_to(lx, ly);
                    pb.cubic_to(cx, cy, px as f32, py as f32, px as f32, py as f32);
                }
            }
            snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.5), &blue);
            if let Some(last) = path.anchors.last() {
                handles(snap, last, &scr, &blue);
            }
            for (i, a) in path.anchors.iter().enumerate() {
                square(snap, scr(a.p), i == 0 && path.anchors.len() > 1, &blue);
            }
        }
        let editing = self.extra.vector_edit.borrow().clone();
        if let Some(id) = editing {
            let Some((n, r, subs)) = self.edit_anchors(&id) else { return };
            let turn = shapes::rotation(&n);
            let scr = |p: Pt| {
                let (x, y) = shapes::rotate(p.0, p.1, r.cx(), r.cy(), turn);
                let (x, y) = cam.to_screen(x, y);
                (x as f32, y as f32)
            };
            let pb = gsk::PathBuilder::new();
            add_screen(&pb, &segments(&subs), &scr);
            snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.0), &blue);
            for (a, _) in &subs {
                for an in a {
                    handles(snap, an, &scr, &blue);
                }
                for an in a {
                    square(snap, scr(an.p), false, &blue);
                }
            }
        }
    }
}

fn add_screen(pb: &gsk::PathBuilder, segs: &[Seg], scr: &impl Fn(Pt) -> (f32, f32)) {
    for s in segs {
        match *s {
            Seg::Move([x, y]) => {
                let (x, y) = scr((x, y));
                pb.move_to(x, y);
            }
            Seg::Line([x, y]) => {
                let (x, y) = scr((x, y));
                pb.line_to(x, y);
            }
            Seg::Cubic(a, b, e) => {
                let (ax, ay) = scr((a[0], a[1]));
                let (bx, by) = scr((b[0], b[1]));
                let (ex, ey) = scr((e[0], e[1]));
                pb.cubic_to(ax, ay, bx, by, ex, ey);
            }
            Seg::Close => pb.close(),
        }
    }
}

fn handles(snap: &gtk::Snapshot, a: &Anchor, scr: &impl Fn(Pt) -> (f32, f32), c: &gdk::RGBA) {
    let (x, y) = scr(a.p);
    for h in [a.cin, a.cout] {
        if h == a.p {
            continue;
        }
        let (hx, hy) = scr(h);
        let pb = gsk::PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(hx, hy);
        snap.append_stroke(&pb.to_path(), &gsk::Stroke::new(1.0), c);
        let dot = gsk::RoundedRect::from_rect(graphene::Rect::new(hx - 3.5, hy - 3.5, 7.0, 7.0), 3.5);
        snap.push_rounded_clip(&dot);
        snap.append_color(&gdk::RGBA::WHITE, &dot.bounds().clone());
        snap.pop();
        snap.append_border(&dot, &[1.5; 4], &[*c, *c, *c, *c]);
    }
}

fn square(snap: &gtk::Snapshot, (x, y): (f32, f32), first: bool, c: &gdk::RGBA) {
    let r = graphene::Rect::new(x - 4.0, y - 4.0, 8.0, 8.0);
    snap.append_color(if first { c } else { &gdk::RGBA::WHITE }, &r);
    let rr = gsk::RoundedRect::from_rect(r, 0.0);
    snap.append_border(&rr, &[1.5; 4], &[*c, *c, *c, *c]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_round_trip_through_segments() {
        let segs = relay_bus::svgpath::parse("M0 0 C 10 0 20 10 20 20 L 0 20 Z").unwrap();
        let subs = anchors_of(&segs);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].0.len(), 3);
        assert!(subs[0].1);
        assert_eq!(subs[0].0[0].cout, (10.0, 0.0));
        assert_eq!(subs[0].0[1].cin, (20.0, 10.0));
        assert_eq!(segments(&subs), segs);
        // A closing curve back to the start is folded into the first anchor.
        let back = relay_bus::svgpath::parse("M0 0 L 10 0 C 10 10 5 5 0 0 Z").unwrap();
        let subs = anchors_of(&back);
        assert_eq!(subs[0].0.len(), 2);
        assert_eq!(subs[0].0[0].cin, (5.0, 5.0));
        assert_eq!(segments(&subs).len(), 4);
    }

    #[test]
    fn shapes_outline_where_they_are() {
        let mut e = Node::new(NodeKind::Ellipse);
        e.parent = Some("p".into());
        let segs = outline_of(&e, &Rect::new(10.0, 10.0, 100.0, 50.0)).unwrap();
        let b = relay_bus::svgpath::bounds(&segs).unwrap();
        assert!((b[0] - 10.0).abs() < 0.5 && (b[2] - 100.0).abs() < 0.5 && (b[3] - 50.0).abs() < 0.5, "{b:?}");
    }
}
