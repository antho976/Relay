//! What the canvas draws over a design to help lay it out, none of it part of the design:
//! frames' layout grids, the distances Alt shows from the selection to what the pointer is
//! over, and prototype links as arrows to the frames they go to. Also what follows a frame
//! resized by hand (children keeping to their constraints) and mirroring.

use super::board::{Board, GUIDE, SELECT};
use super::paint::{grect, rgba};
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use gtk4 as gtk;
use relay_bus::design::{Grid, GridKind, Layout, Node, NodeKind, NodeOp, Pin};
use relay_client::design::layout::Placed;
use relay_client::design::{Rect, Scene};
use serde_json::{json, Map, Value};

/// Where a grid's cells sit across `span` from `start`: `(offset, size)` of each.
pub fn bands(g: &Grid, start: f64, span: f64) -> Vec<(f64, f64)> {
    let count = g.count.unwrap_or(if g.kind == GridKind::Rows { 4 } else { 12 }).max(1) as f64;
    let gutter = g.gutter.unwrap_or(if g.kind == GridKind::Rows { 16.0 } else { 20.0 });
    let margin = g.margin.unwrap_or(if g.kind == GridKind::Rows { 0.0 } else { 24.0 });
    let size = ((span - 2.0 * margin - gutter * (count - 1.0)) / count).max(0.0);
    (0..count as usize).map(|i| (start + margin + i as f64 * (size + gutter), size)).collect()
}

/// The edges a grid gives a frame at `r`, vertical (x) and horizontal (y), to snap to.
pub fn grid_edges(n: &Node, r: &Rect) -> (Vec<f64>, Vec<f64>) {
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for g in n.grids.iter().flatten().filter(|g| g.hidden != Some(true)) {
        match g.kind {
            GridKind::Columns => {
                for (at, size) in bands(g, r.x, r.w) {
                    xs.extend([at, at + size]);
                }
            }
            GridKind::Rows => {
                for (at, size) in bands(g, r.y, r.h) {
                    ys.extend([at, at + size]);
                }
            }
            GridKind::Square => {}
        }
    }
    (xs, ys)
}

/// A child's new box in its frame when the frame goes from `old` to `new` (sizes), by its
/// constraints.
pub fn follow(n: &Node, old: (f64, f64), new: (f64, f64)) -> (f64, f64, f64, f64) {
    let c = n.constraints.unwrap_or(relay_bus::design::Constraints { h: Pin::Start, v: Pin::Start });
    let axis = |pin: Pin, at: f64, size: f64, was: f64, now: f64| -> (f64, f64) {
        let grow = now - was;
        match pin {
            Pin::Start => (at, size),
            Pin::End => (at + grow, size),
            Pin::Both => (at, (size + grow).max(1.0)),
            Pin::Center => (at + grow / 2.0, size),
            Pin::Scale if was > 0.0 => (at * now / was, (size * now / was).max(1.0)),
            Pin::Scale => (at, size),
        }
    };
    let (x, w) = axis(c.h, n.x, n.w, old.0, new.0);
    let (y, h) = axis(c.v, n.y, n.h, old.1, new.1);
    (x, y, w, h)
}

impl Board {
    /// Each frame's columns, rows or square grid, faintly over it.
    pub(super) fn layout_grids(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed) {
        if self.extra.hide_grids.get() {
            return;
        }
        let page = self.page.borrow().clone();
        let cam = self.cam.get();
        let (vw, vh) = self.size.get();
        let view = Rect::new(0.0, 0.0, vw, vh);
        for n in scene.nodes().filter(|n| n.grids.is_some() && n.kind == NodeKind::Frame && !n.is_hidden()) {
            if scene.page_of(&n.id) != Some(page.as_str()) {
                continue;
            }
            let Some(r) = placed.rect(&n.id) else { continue };
            let s = cam.rect_to_screen(&r);
            if !s.intersects(&view) {
                continue;
            }
            snap.push_clip(&grect(&s));
            for g in n.grids.iter().flatten().filter(|g| g.hidden != Some(true)) {
                let mut c = rgba(g.color.as_deref().unwrap_or("#FF3D711A"));
                if g.color.is_none() {
                    c.set_alpha(0.10);
                }
                match g.kind {
                    GridKind::Columns => {
                        for (at, size) in bands(g, r.x, r.w) {
                            let (x0, _) = cam.to_screen(at, 0.0);
                            let (x1, _) = cam.to_screen(at + size, 0.0);
                            snap.append_color(&c, &graphene::Rect::new(x0 as f32, s.y as f32, (x1 - x0) as f32, s.h as f32));
                        }
                    }
                    GridKind::Rows => {
                        for (at, size) in bands(g, r.y, r.h) {
                            let (_, y0) = cam.to_screen(0.0, at);
                            let (_, y1) = cam.to_screen(0.0, at + size);
                            snap.append_color(&c, &graphene::Rect::new(s.x as f32, y0 as f32, s.w as f32, (y1 - y0) as f32));
                        }
                    }
                    GridKind::Square => {
                        let step = g.size.unwrap_or(8.0) * cam.zoom;
                        if step < 4.0 {
                            continue;
                        }
                        c.set_alpha(c.alpha().max(0.18));
                        let mut x = s.x;
                        while x <= s.right() {
                            snap.append_color(&c, &graphene::Rect::new(x as f32, s.y as f32, 1.0, s.h as f32));
                            x += step;
                        }
                        let mut y = s.y;
                        while y <= s.bottom() {
                            snap.append_color(&c, &graphene::Rect::new(s.x as f32, y as f32, s.w as f32, 1.0));
                            y += step;
                        }
                    }
                }
            }
            snap.pop();
        }
    }

    /// Alt's distances, the prototype's arrows and its start marks.
    pub(super) fn marks(&self, snap: &gtk::Snapshot, scene: &Scene, placed: &Placed) {
        let cam = self.cam.get();
        let selection = self.selection.borrow().clone();
        // Prototype: where the selection goes when clicked, and where presenting starts.
        let page = self.page.borrow().clone();
        for id in scene.children(&page) {
            if let (Some(n), Some(r)) = (scene.node(id).filter(|n| n.start == Some(true)), placed.rect(id)) {
                let _ = n;
                let s = cam.rect_to_screen(&r);
                self.pill(snap, "▶ Start", s.x, s.y - 40.0, &rgba(SELECT));
            }
        }
        for id in &selection {
            let Some(n) = scene.node(id) else { continue };
            let (Some(to), Some(from)) = (n.goto.as_deref().and_then(|t| placed.rect(t)), placed.rect(id)) else { continue };
            let (a, z) = (cam.rect_to_screen(&from), cam.rect_to_screen(&to));
            let (sx, sy) = (a.right(), a.cy());
            let (ex, ey) = if z.x >= a.right() { (z.x, z.y + 24.0f64.min(z.h / 2.0)) } else { (z.right(), z.y + 24.0f64.min(z.h / 2.0)) };
            let bend = ((ex - sx).abs() / 2.0).max(40.0);
            let dir = if ex >= sx { 1.0 } else { -1.0 };
            let pb = gsk::PathBuilder::new();
            pb.move_to(sx as f32, sy as f32);
            pb.cubic_to((sx + bend) as f32, sy as f32, (ex - bend * dir) as f32, ey as f32, ex as f32, ey as f32);
            let blue = rgba(SELECT);
            let stroke = gsk::Stroke::new(2.0);
            stroke.set_dash(&[6.0, 4.0]);
            snap.append_stroke(&pb.to_path(), &stroke, &blue);
            let head = gsk::PathBuilder::new();
            head.move_to(ex as f32, ey as f32);
            head.line_to((ex - 9.0 * dir) as f32, (ey - 5.0) as f32);
            head.line_to((ex - 9.0 * dir) as f32, (ey + 5.0) as f32);
            head.close();
            snap.append_fill(&head.to_path(), gsk::FillRule::Winding, &blue);
            let dot = gsk::RoundedRect::from_rect(graphene::Rect::new(sx as f32 - 4.0, sy as f32 - 4.0, 8.0, 8.0), 4.0);
            snap.push_rounded_clip(&dot);
            snap.append_color(&blue, dot.bounds());
            snap.pop();
        }
        // Alt: how far the selection is from what the pointer is over.
        if !self.extra.alt.get() || selection.is_empty() {
            return;
        }
        let Some(sel) = Rect::bounds(selection.iter().filter_map(|id| placed.rects.get(id))) else { return };
        let other = self.hover.borrow().clone().filter(|h| !selection.contains(h)).or_else(|| {
            selection.first().and_then(|id| scene.parent(id)).filter(|p| *p != page).map(str::to_string)
        });
        let Some(other) = other.and_then(|o| placed.rect(&o)) else { return };
        let red = rgba(GUIDE);
        let measure = |x0: f64, y0: f64, x1: f64, y1: f64| {
            let len = (x1 - x0).abs().max((y1 - y0).abs());
            if len < 0.5 {
                return;
            }
            let (ax, ay) = cam.to_screen(x0, y0);
            let (bx, by) = cam.to_screen(x1, y1);
            let r = graphene::Rect::new(ax.min(bx) as f32 - 0.5, ay.min(by) as f32 - 0.5, ((bx - ax).abs() as f32).max(1.0), ((by - ay).abs() as f32).max(1.0));
            snap.append_color(&red, &r);
            self.pill(snap, &super::board::fmt(len), (ax + bx) / 2.0, (ay + by) / 2.0, &red);
        };
        let inside = other.x <= sel.x && other.y <= sel.y && other.right() >= sel.right() && other.bottom() >= sel.bottom();
        if inside {
            measure(other.x, sel.cy(), sel.x, sel.cy());
            measure(sel.right(), sel.cy(), other.right(), sel.cy());
            measure(sel.cx(), other.y, sel.cx(), sel.y);
            measure(sel.cx(), sel.bottom(), sel.cx(), other.bottom());
            return;
        }
        let y = (sel.y.max(other.y) + sel.bottom().min(other.bottom())) / 2.0;
        let x = (sel.x.max(other.x) + sel.right().min(other.right())) / 2.0;
        if other.x >= sel.right() {
            measure(sel.right(), y, other.x, y);
        } else if other.right() <= sel.x {
            measure(other.right(), y, sel.x, y);
        }
        if other.y >= sel.bottom() {
            measure(x, sel.bottom(), x, other.y);
        } else if other.bottom() <= sel.y {
            measure(x, other.bottom(), x, sel.y);
        }
    }

    /// A small label centred on `(x, y)` (screen pixels) on a tinted pill.
    fn pill(&self, snap: &gtk::Snapshot, text: &str, x: f64, y: f64, color: &gdk::RGBA) {
        let layout = self.canvas.create_pango_layout(Some(text));
        let mut d = pango::FontDescription::new();
        d.set_family("Geist");
        d.set_weight(pango::Weight::Medium);
        d.set_absolute_size(11.0 * pango::SCALE as f64);
        layout.set_font_description(Some(&d));
        let (w, h) = layout.pixel_size();
        let r = graphene::Rect::new((x - w as f64 / 2.0 - 5.0) as f32, (y - h as f64 / 2.0 - 2.0) as f32, w as f32 + 10.0, h as f32 + 4.0);
        let rr = gsk::RoundedRect::from_rect(r, 4.0);
        snap.push_rounded_clip(&rr);
        snap.append_color(color, &r);
        snap.pop();
        snap.save();
        snap.translate(&graphene::Point::new(r.x() + 5.0, r.y() + 2.0));
        snap.append_layout(&layout, &gdk::RGBA::WHITE);
        snap.restore();
    }

    /// A frame about to be resized by hand: note its children that keep to constraints.
    pub(super) fn pinned_begin(&self, orig: &[(String, Node, Rect)]) {
        let mut pinned = self.extra.pinned.borrow_mut();
        pinned.clear();
        let [(id, frame, _)] = orig else { return };
        if frame.kind != NodeKind::Frame || frame.layout_mode() != Layout::None {
            return;
        }
        let scene = self.doc.scene.borrow();
        for c in scene.children(id) {
            if let Some(n) = scene.node(c).filter(|n| n.constraints.is_some_and(|k| k.h != Pin::Start || k.v != Pin::Start)) {
                pinned.push((c.clone(), n.clone()));
            }
        }
    }

    /// The frame `orig` is now `w`×`h`: its pinned children follow, live.
    pub(super) fn pinned_follow(&self, orig: &Node, w: f64, h: f64) {
        let pinned = self.extra.pinned.borrow();
        if pinned.is_empty() {
            return;
        }
        let mut scene = self.doc.scene.borrow_mut();
        for (id, was) in pinned.iter() {
            let (x, y, cw, ch) = follow(was, (orig.w, orig.h), (w, h));
            if let Some(n) = scene.node_mut(id) {
                (n.x, n.y, n.w, n.h) = (x.round(), y.round(), cw.round(), ch.round());
            }
        }
    }

    /// The resize ends: the children back as they were, and the edits that move them for real.
    pub(super) fn pinned_finish(&self, commit: bool) -> Vec<NodeOp> {
        let pinned = std::mem::take(&mut *self.extra.pinned.borrow_mut());
        let mut scene = self.doc.scene.borrow_mut();
        let mut ops = Vec::new();
        for (id, was) in pinned {
            let Some(n) = scene.node_mut(&id) else { continue };
            let now = n.clone();
            *n = was.clone();
            if commit {
                let mut set = Map::new();
                for (k, a, z) in [("x", was.x, now.x), ("y", was.y, now.y), ("w", was.w, now.w), ("h", was.h, now.h)] {
                    if (a - z).abs() >= 0.5 {
                        set.insert(k.into(), json!(z));
                    }
                }
                if !set.is_empty() {
                    ops.push(NodeOp::Update { id, set });
                }
            }
        }
        ops
    }

    /// Mirror the selection across or down.
    pub(super) fn flip(&self, field: &str) {
        let ids = self.selected();
        if ids.is_empty() {
            return;
        }
        let scene = self.doc.scene.borrow();
        let all = ids.iter().filter_map(|id| scene.node(id)).all(|n| if field == "flip_x" { n.flip_x == Some(true) } else { n.flip_y == Some(true) });
        drop(scene);
        let mut set = Map::new();
        set.insert(field.into(), if all { Value::Null } else { json!(true) });
        self.set_selected(set);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_share_the_room_after_margins_and_gutters() {
        let g = Grid { kind: GridKind::Columns, count: Some(4), size: None, gutter: Some(10.0), margin: Some(20.0), color: None, hidden: None };
        let b = bands(&g, 0.0, 470.0);
        assert_eq!(b.len(), 4);
        assert_eq!(b[0], (20.0, 100.0));
        assert_eq!(b[3], (350.0, 100.0));
    }

    #[test]
    fn children_keep_to_their_constraints() {
        let mut n = Node::new(NodeKind::Rect);
        (n.x, n.y, n.w, n.h) = (10.0, 10.0, 20.0, 20.0);
        let grown = |n: &Node| follow(n, (100.0, 100.0), (200.0, 150.0));
        assert_eq!(grown(&n), (10.0, 10.0, 20.0, 20.0));
        n.constraints = Some(relay_bus::design::Constraints { h: Pin::End, v: Pin::Both });
        assert_eq!(grown(&n), (110.0, 10.0, 20.0, 70.0));
        n.constraints = Some(relay_bus::design::Constraints { h: Pin::Center, v: Pin::Scale });
        assert_eq!(grown(&n), (60.0, 15.0, 20.0, 30.0));
    }
}
