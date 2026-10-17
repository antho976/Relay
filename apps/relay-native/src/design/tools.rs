//! The canvas's input: what a press, a drag, a release, the wheel, a pinch and the keyboard
//! do with each tool. A gesture previews on the scene as it goes and sends one edit when it
//! ends, so a drag is one undo step and the engine hears nothing until the pointer lets go.

use super::board::{Board, Gesture, Handle, Tool, HANDLES};
use super::paint::{fill_of, luminance, CANVAS};
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use relay_bus::design::{Layout, Node, NodeKind, NodeOp, Sizing};
use relay_client::design::edit::{self, Align};
use relay_client::design::geom::simplify;
use relay_client::design::hit;
use relay_client::design::layout;
use relay_client::design::snap::{self, Edges};
use relay_client::design::Rect;
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// How close (in screen pixels) a box comes before it snaps.
const SNAP: f64 = 6.0;
/// How far the pointer moves before a press becomes a drag.
const DRAG: f64 = 3.0;

pub fn install(b: &Rc<Board>) {
    let canvas = &b.canvas;

    let drag = gtk::GestureDrag::new();
    drag.set_button(gdk::BUTTON_PRIMARY);
    let weak = Rc::downgrade(b);
    drag.connect_drag_begin(move |g, x, y| {
        if let Some(b) = weak.upgrade() {
            let mods = g.current_event_state();
            begin(&b, x, y, mods);
        }
    });
    let weak = Rc::downgrade(b);
    drag.connect_drag_update(move |g, dx, dy| {
        if let Some(b) = weak.upgrade() {
            let (sx, sy) = g.start_point().unwrap_or((0.0, 0.0));
            let mods = g.current_event_state();
            update(&b, sx + dx, sy + dy, dx, dy, mods);
        }
    });
    let weak = Rc::downgrade(b);
    drag.connect_drag_end(move |g, dx, dy| {
        if let Some(b) = weak.upgrade() {
            let (sx, sy) = g.start_point().unwrap_or((0.0, 0.0));
            finish(&b, sx + dx, sy + dy, g.current_event_state());
        }
    });
    let weak = Rc::downgrade(b);
    drag.connect_cancel(move |_, _| {
        if let Some(b) = weak.upgrade() {
            cancel(&b);
        }
    });
    canvas.add_controller(drag);

    // The middle button pans whatever the tool.
    let pan = gtk::GestureDrag::new();
    pan.set_button(gdk::BUTTON_MIDDLE);
    let weak = Rc::downgrade(b);
    pan.connect_drag_begin(move |_, _, _| {
        if let Some(b) = weak.upgrade() {
            *b.gesture.borrow_mut() = Gesture::Pan { cam: b.cam.get() };
            update_cursor(&b, Some("grabbing"));
        }
    });
    let weak = Rc::downgrade(b);
    pan.connect_drag_update(move |_, dx, dy| {
        if let Some(b) = weak.upgrade() {
            if let Gesture::Pan { cam, .. } = &*b.gesture.borrow() {
                let mut cam = *cam;
                cam.pan(dx, dy);
                b.set_cam(cam);
            }
        }
    });
    let weak = Rc::downgrade(b);
    pan.connect_drag_end(move |_, _, _| {
        if let Some(b) = weak.upgrade() {
            *b.gesture.borrow_mut() = Gesture::Idle;
            update_cursor(&b, None);
        }
    });
    canvas.add_controller(pan);

    let menu = gtk::GestureClick::new();
    menu.set_button(gdk::BUTTON_SECONDARY);
    let weak = Rc::downgrade(b);
    menu.connect_pressed(move |_, _, x, y| {
        if let Some(b) = weak.upgrade() {
            context_menu(&b, x, y);
        }
    });
    canvas.add_controller(menu);

    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(b);
    motion.connect_motion(move |_, x, y| {
        if let Some(b) = weak.upgrade() {
            hover(&b, x, y);
        }
    });
    let weak = Rc::downgrade(b);
    motion.connect_leave(move |_| {
        if let Some(b) = weak.upgrade() {
            if b.hover.borrow_mut().take().is_some() {
                b.canvas.queue_draw();
            }
        }
    });
    canvas.add_controller(motion);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    let weak = Rc::downgrade(b);
    scroll.connect_scroll(move |c, dx, dy| {
        let Some(b) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let mods = c.current_event_state();
        let wheel = c.unit() == gdk::ScrollUnit::Wheel;
        let (px, py) = b.pointer.get();
        if mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::META_MASK) {
            let factor = if wheel { (-dy * 0.18).exp() } else { (-dy * 0.012).exp() };
            let mut cam = b.cam.get();
            cam.zoom_at(px, py, cam.zoom * factor);
            b.set_cam(cam);
        } else {
            let (mut sx, mut sy) = if wheel { (dx * 48.0, dy * 48.0) } else { (dx, dy) };
            if wheel && mods.contains(gdk::ModifierType::SHIFT_MASK) {
                std::mem::swap(&mut sx, &mut sy);
            }
            let mut cam = b.cam.get();
            cam.pan(-sx, -sy);
            b.set_cam(cam);
        }
        glib::Propagation::Stop
    });
    canvas.add_controller(scroll);

    let zoom = gtk::GestureZoom::new();
    let start = std::rc::Rc::new(std::cell::Cell::new(1.0));
    let weak = Rc::downgrade(b);
    let s = start.clone();
    zoom.connect_begin(move |_, _| {
        if let Some(b) = weak.upgrade() {
            s.set(b.cam.get().zoom);
        }
    });
    let weak = Rc::downgrade(b);
    zoom.connect_scale_changed(move |g, scale| {
        if let Some(b) = weak.upgrade() {
            let (cx, cy) = g.bounding_box_center().unwrap_or(b.pointer.get());
            let mut cam = b.cam.get();
            cam.zoom_at(cx, cy, start.get() * scale);
            b.set_cam(cam);
        }
    });
    canvas.add_controller(zoom);

    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(b);
    keys.connect_key_pressed(move |c, key, code, mods| {
        let Some(b) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        key_pressed(&b, c, key, code, mods)
    });
    let weak = Rc::downgrade(b);
    keys.connect_key_released(move |_, key, _, _| {
        if let Some(b) = weak.upgrade() {
            if key == gdk::Key::space {
                b.space.set(false);
                update_cursor(&b, None);
            }
            if matches!(key, gdk::Key::Alt_L | gdk::Key::Alt_R) && b.extra.alt.replace(false) {
                b.canvas.queue_draw();
            }
        }
    });
    canvas.add_controller(keys);

    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    let weak = Rc::downgrade(b);
    drop.connect_drop(move |_, value, x, y| {
        let Some(b) = weak.upgrade() else {
            return false;
        };
        let Ok(files) = value.get::<gdk::FileList>() else {
            return false;
        };
        let paths: Vec<std::path::PathBuf> = files.files().iter().filter_map(gio::prelude::FileExt::path).collect();
        let at = b.to_page(x, y);
        super::assets::place_files(&b, paths, Some(at));
        true
    });
    canvas.add_controller(drop);

    // An asset dragged from the Assets panel lands where it is dropped.
    let assets = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
    let weak = Rc::downgrade(b);
    assets.connect_drop(move |_, value, x, y| {
        let (Some(b), Ok(text)) = (weak.upgrade(), value.get::<String>()) else {
            return false;
        };
        let Some(path) = text.strip_prefix(super::library::DRAG_PREFIX) else {
            return false;
        };
        let at = b.to_page(x, y);
        // Dropped on a frame: into it.
        b.select(Vec::new());
        let target = container_at(&b, at);
        if target != *b.page.borrow() {
            b.select(vec![target]);
        }
        b.insert_item(path, Some(at));
        true
    });
    canvas.add_controller(assets);
}

/// A whole pointer drag from page point `from` to `to`, as the pointer would make it: for the
/// smoke harness's scripted runs (`drive.rs`). A drag of no length is a click.
pub(super) fn simulate(b: &Rc<Board>, from: (f64, f64), to: (f64, f64), mods: gdk::ModifierType) {
    let cam = b.cam.get();
    let (x0, y0) = cam.to_screen(from.0, from.1);
    let (x1, y1) = cam.to_screen(to.0, to.1);
    begin(b, x0, y0, mods);
    if (x1 - x0).abs() + (y1 - y0).abs() > 0.0 {
        for i in 1..=8 {
            let t = i as f64 / 8.0;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            update(b, x, y, x - x0, y - y0, mods);
        }
    }
    finish(b, x1, y1, mods);
}

pub fn update_cursor(b: &Board, name: Option<&str>) {
    let name = name.unwrap_or(if b.space.get() || b.tool.get() == Tool::Hand {
        "grab"
    } else {
        match b.tool.get() {
            Tool::Select => "default",
            Tool::Text => "text",
            _ => "crosshair",
        }
    });
    b.canvas.set_cursor_from_name(Some(name));
}

fn shift(m: gdk::ModifierType) -> bool {
    m.contains(gdk::ModifierType::SHIFT_MASK)
}

fn alt(m: gdk::ModifierType) -> bool {
    m.contains(gdk::ModifierType::ALT_MASK)
}

/// The container a new shape goes into: the deepest frame under the point, else the page.
fn container_at(b: &Board, p: (f64, f64)) -> String {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let chain = hit::chain(&scene, &placed, &page, p.0, p.1, 0.0);
    chain
        .iter()
        .rev()
        .find(|id| scene.node(id).is_some_and(|n| n.kind == NodeKind::Frame && !n.is_locked()))
        .cloned()
        .unwrap_or(page)
}

/// The node a connector's end attaches to under `p`: the deepest shape there that is not a
/// line or the page itself.
fn attachable(b: &Board, p: (f64, f64)) -> Option<String> {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let chain = hit::chain(&scene, &placed, &page, p.0, p.1, 2.0 / b.cam.get().zoom);
    chain
        .into_iter()
        .rev()
        .find(|id| scene.node(id).is_some_and(|n| !matches!(n.kind, NodeKind::Line | NodeKind::Path | NodeKind::Page)))
}

/// Rub out what the eraser touches at `p`: pen strokes and lines, shown faint until it lets go.
fn erase_at(b: &Board, p: (f64, f64)) {
    let placed = b.placed();
    let page = b.page.borrow().clone();
    let hit = {
        let scene = b.doc.scene.borrow();
        hit::chain(&scene, &placed, &page, p.0, p.1, 6.0 / b.cam.get().zoom)
            .last()
            .filter(|id| scene.node(id).is_some_and(|n| matches!(n.kind, NodeKind::Path | NodeKind::Line)))
            .cloned()
    };
    let Some(id) = hit else {
        return;
    };
    let mut gesture = b.gesture.borrow_mut();
    let Gesture::Erase { hits } = &mut *gesture else {
        return;
    };
    if hits.iter().any(|(h, _)| *h == id) {
        return;
    }
    let mut scene = b.doc.scene.borrow_mut();
    if let Some(n) = scene.node_mut(&id) {
        hits.push((id.clone(), n.opacity));
        n.opacity = Some(0.25);
    }
    drop(scene);
    drop(gesture);
    b.invalidate();
}

/// Just outside a corner of the one selected node: where a drag turns it.
fn rotate_at(b: &Board, x: f64, y: f64) -> Option<String> {
    let sel = b.selection.borrow();
    let [id] = sel.as_slice() else {
        return None;
    };
    let scene = b.doc.scene.borrow();
    let n = scene.node(id).filter(|n| !matches!(n.kind, NodeKind::Page | NodeKind::Line) && !n.is_locked())?;
    let r = b.cam.get().rect_to_screen(&b.placed().rect(id)?);
    let turn = relay_client::design::shapes::rotation(n);
    for (cx, cy, ox, oy) in [(r.x, r.y, -1.0, -1.0), (r.right(), r.y, 1.0, -1.0), (r.right(), r.bottom(), 1.0, 1.0), (r.x, r.bottom(), -1.0, 1.0)] {
        let (hx, hy) = relay_client::design::shapes::rotate(cx + ox * 10.0, cy + oy * 10.0, r.cx(), r.cy(), turn);
        if (x - hx).abs() <= 9.0 && (y - hy).abs() <= 9.0 {
            return Some(id.clone());
        }
    }
    None
}

/// The colour a mark reads well on inside `container`.
fn ink_for(b: &Board, container: &str) -> String {
    let scene = b.doc.scene.borrow();
    let ground = std::iter::once(container)
        .chain(scene.ancestors(container))
        .filter_map(|id| scene.node(id))
        .find_map(fill_of)
        .unwrap_or_else(|| CANVAS.into());
    if luminance(&ground) > 0.35 { "#1A1A1A".into() } else { "#EDE9E2".into() }
}

fn handle_at(b: &Board, x: f64, y: f64) -> Option<Handle> {
    if let Some((_, r, turn)) = b.single_turn() {
        let s = b.cam.get().rect_to_screen(&r);
        return HANDLES.into_iter().find(|h| {
            let (hx, hy) = super::board::handle_point(&s, *h);
            let (hx, hy) = relay_client::design::shapes::rotate(hx, hy, s.cx(), s.cy(), turn);
            (x - hx).abs() <= 6.0 && (y - hy).abs() <= 6.0
        });
    }
    let bounds = b.selection_bounds()?;
    let s = b.cam.get().rect_to_screen(&bounds);
    HANDLES.into_iter().find(|h| {
        let (hx, hy) = super::board::handle_point(&s, *h);
        let small = s.w.min(s.h) <= 24.0;
        let corner = (h.left || h.right) && (h.top || h.bottom);
        (corner || !small) && (x - hx).abs() <= 6.0 && (y - hy).abs() <= 6.0
    })
    .or_else(|| {
        // An edge anywhere along its length.
        let near = |a: f64, b: f64| (a - b).abs() <= 4.0;
        let inside_x = x > s.x + 6.0 && x < s.right() - 6.0;
        let inside_y = y > s.y + 6.0 && y < s.bottom() - 6.0;
        if s.w < 16.0 || s.h < 16.0 {
            return None;
        }
        if near(x, s.x) && inside_y {
            Some(HANDLES[7])
        } else if near(x, s.right()) && inside_y {
            Some(HANDLES[3])
        } else if near(y, s.y) && inside_x {
            Some(HANDLES[1])
        } else if near(y, s.bottom()) && inside_x {
            Some(HANDLES[5])
        } else {
            None
        }
    })
}

fn is_line_selected(b: &Board) -> Option<String> {
    let sel = b.selection.borrow();
    let scene = b.doc.scene.borrow();
    match sel.as_slice() {
        [one] if scene.node(one).is_some_and(|n| n.kind == NodeKind::Line) => Some(one.clone()),
        _ => None,
    }
}

fn begin(b: &Rc<Board>, x: f64, y: f64, mods: gdk::ModifierType) {
    b.canvas.grab_focus();
    if b.edit.borrow().is_some() {
        b.end_edit(true);
    }
    let p = b.to_page(x, y);
    // A second press close by in time and place is a double click.
    let now = glib::monotonic_time();
    let (t, lx, ly) = b.last_press.get();
    let settings = gtk::Settings::default();
    let window = settings.as_ref().map_or(400, |s| s.gtk_double_click_time()) as i64 * 1000;
    let distance = settings.as_ref().map_or(5, |s| s.gtk_double_click_distance()) as f64;
    if now - t < window && (x - lx).abs() <= distance && (y - ly).abs() <= distance {
        b.last_press.set((0, x, y));
        *b.gesture.borrow_mut() = Gesture::Idle;
        double_click(b, x, y, p);
        return;
    }
    b.last_press.set((now, x, y));
    if b.extra.pen.borrow().is_none() && !b.space.get() {
        if let Some(id) = b.comment_at(x, y) {
            b.open_comment(&id);
            return;
        }
    }
    if b.extra.vector_edit.borrow().is_some() && b.tool.get() == Tool::Select && !b.space.get() {
        if let Some((id, point)) = b.point_at(x, y) {
            let orig = b.doc.scene.borrow().node(&id).cloned();
            if let (Some(orig), Some(rect)) = (orig, b.placed().rect(&id)) {
                b.doc.set_hold(true);
                *b.gesture.borrow_mut() = Gesture::VectorPoint { id, point, orig, rect, moved: false };
                return;
            }
        }
        b.stop_editing_points();
    }
    if b.space.get() || b.tool.get() == Tool::Hand {
        *b.gesture.borrow_mut() = Gesture::Pan { cam: b.cam.get() };
        update_cursor(b, Some("grabbing"));
        return;
    }
    match b.tool.get() {
        Tool::Select => begin_select(b, x, y, p, mods),
        Tool::Frame | Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow | Tool::Triangle | Tool::Diamond | Tool::Hexagon | Tool::Star => {
            let tool = b.tool.get();
            let line = matches!(tool, Tool::Line | Tool::Arrow);
            // A line drawn from a shape attaches to it: a connector, which follows the shape.
            let from = if line { attachable(b, p) } else { None };
            let parent = if line && from.is_some() {
                b.page.borrow().clone()
            } else if tool == Tool::Frame {
                frame_parent(b, p)
            } else {
                container_at(b, p)
            };
            *b.gesture.borrow_mut() = Gesture::Create { tool, parent, a: p, b: p, from, to: None };
        }
        Tool::Eraser => {
            *b.gesture.borrow_mut() = Gesture::Erase { hits: Vec::new() };
            b.doc.set_hold(true);
            erase_at(b, p);
        }
        Tool::Text => {
            let parent = container_at(b, p);
            b.create_text(p, parent);
        }
        Tool::Sticky => create_sticky(b, p),
        Tool::Vector => {
            let parent = b.extra.pen.borrow().as_ref().map(|pen| pen.parent.clone()).unwrap_or_else(|| container_at(b, p));
            b.pen_press(p, parent);
        }
        Tool::Comment => {
            let parent = container_at(b, p);
            b.new_comment(p, parent);
            if !b.tool_lock.get() {
                b.set_tool(Tool::Select);
            }
        }
        Tool::Pen | Tool::Highlighter => {
            let parent = container_at(b, p);
            *b.gesture.borrow_mut() = Gesture::Draw { parent, points: vec![[p.0, p.1]] };
        }
        Tool::Hand => {}
    }
}

/// A frame drawn inside another frame nests in it; one drawn on the canvas is a new artboard.
fn frame_parent(b: &Board, p: (f64, f64)) -> String {
    container_at(b, p)
}

fn begin_select(b: &Rc<Board>, x: f64, y: f64, p: (f64, f64), mods: gdk::ModifierType) {
    // A line's ends.
    if let Some(id) = is_line_selected(b) {
        for (which, (ex, ey)) in b.endpoints(&id).into_iter().enumerate() {
            if (x - ex).abs() <= 7.0 && (y - ey).abs() <= 7.0 {
                let Some(orig) = b.doc.scene.borrow().node(&id).cloned() else {
                    return;
                };
                let last = orig.points.as_ref().map_or(1, |p| p.len().saturating_sub(1));
                b.doc.set_hold(true);
                *b.gesture.borrow_mut() = Gesture::Endpoint { id, which: if which == 0 { 0 } else { last }, orig, moved: false };
                return;
            }
        }
    }
    // Just outside a corner: turn it.
    if handle_at(b, x, y).is_none() {
        if let Some(id) = rotate_at(b, x, y) {
            let placed = b.placed();
            let Some((orig, r)) = b.doc.scene.borrow().node(&id).cloned().zip(placed.rect(&id)) else {
                return;
            };
            let start = (p.1 - r.cy()).atan2(p.0 - r.cx()).to_degrees();
            b.doc.set_hold(true);
            *b.gesture.borrow_mut() = Gesture::Rotate { id, centre: (r.cx(), r.cy()), start, orig, moved: false };
            return;
        }
    }
    // A resize handle of the selection.
    if is_line_selected(b).is_none() {
        if let (Some(handle), Some(bounds)) = (handle_at(b, x, y), b.selection_bounds()) {
            let placed = b.placed();
            let scene = b.doc.scene.borrow();
            let orig: Vec<(String, Node, Rect)> = b
                .selected()
                .into_iter()
                .filter_map(|id| Some((id.clone(), scene.node(&id)?.clone(), placed.rect(&id)?)))
                .collect();
            drop(scene);
            b.doc.set_hold(true);
            b.pinned_begin(&orig);
            *b.gesture.borrow_mut() = Gesture::Resize { handle, bounds, orig, moved: false };
            return;
        }
    }
    let placed = b.placed();
    let page = b.page.borrow().clone();
    // A frame's name above it.
    if let Some(id) = b.frame_label_at(x, y) {
        if shift(mods) {
            toggle(b, &id);
        } else if !b.selection.borrow().contains(&id) {
            b.select(vec![id.clone()]);
        }
        start_move(b, p, mods, None);
        return;
    }
    let chain = {
        let scene = b.doc.scene.borrow();
        hit::chain(&scene, &placed, &page, p.0, p.1, 4.0 / b.cam.get().zoom)
    };
    let deep = chain.last().cloned();
    // Inside what is selected: drag moves it; a click selects what is under the pointer.
    let in_selection = b.selection.borrow().iter().any(|id| placed.rect(id).is_some_and(|r| r.contains(p.0, p.1)));
    if in_selection && !shift(mods) {
        start_move(b, p, mods, deep);
        return;
    }
    let Some(deep) = deep else {
        let base = if shift(mods) { b.selection.borrow().clone() } else { Vec::new() };
        if !shift(mods) {
            b.select(Vec::new());
        }
        *b.gesture.borrow_mut() = Gesture::Marquee { scope: page, a: p, b: p, base, moved: false };
        return;
    };
    // The empty part of an artboard: drag draws a marquee inside it, a click selects it.
    let artboard = {
        let scene = b.doc.scene.borrow();
        chain.len() == 1
            && scene.node(&deep).is_some_and(|n| n.kind == NodeKind::Frame && n.parent.as_deref() == Some(page.as_str()))
            && !scene.children(&deep).is_empty()
    };
    if artboard {
        let base = if shift(mods) { b.selection.borrow().clone() } else { Vec::new() };
        *b.gesture.borrow_mut() = Gesture::Marquee { scope: deep, a: p, b: p, base, moved: false };
        return;
    }
    if shift(mods) {
        toggle(b, &deep);
        if !b.selection.borrow().contains(&deep) {
            *b.gesture.borrow_mut() = Gesture::Idle;
            return;
        }
    } else {
        b.select(vec![deep]);
    }
    start_move(b, p, mods, None);
}

fn toggle(b: &Board, id: &str) {
    let mut sel = b.selection.borrow().clone();
    if let Some(i) = sel.iter().position(|s| s == id) {
        sel.remove(i);
    } else {
        // A node and its own ancestor are not selected together.
        let scene = b.doc.scene.borrow();
        sel.retain(|s| !scene.is_inside(id, s) && !scene.is_inside(s, id));
        sel.push(id.to_string());
    }
    b.select(sel);
}

fn start_move(b: &Rc<Board>, p: (f64, f64), mods: gdk::ModifierType, click: Option<String>) {
    let ids = b.selected();
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let ids: Vec<String> = ids.into_iter().filter(|id| scene.node(id).is_some_and(|n| !n.is_locked())).collect();
    let orig = ids.iter().filter_map(|id| scene.node(id).map(|n| (id.clone(), n.x, n.y))).collect();
    let rects = ids.iter().filter_map(|id| placed.rect(id)).collect();
    drop(scene);
    *b.gesture.borrow_mut() = Gesture::Move {
        ids,
        start: p,
        orig,
        rects,
        dup: alt(mods),
        moved: false,
        target: None,
        reorder: None,
        click,
        at: p,
    };
}

fn update(b: &Rc<Board>, x: f64, y: f64, dx: f64, dy: f64, mods: gdk::ModifierType) {
    b.pointer.set((x, y));
    let p = b.to_page(x, y);
    let far = dx.abs() > DRAG || dy.abs() > DRAG;
    let zoom = b.cam.get().zoom;
    let mut gesture = b.gesture.borrow_mut();
    match &mut *gesture {
        Gesture::Idle => {}
        Gesture::Pan { cam, .. } => {
            let mut cam = *cam;
            cam.pan(dx, dy);
            drop(gesture);
            b.set_cam(cam);
        }
        Gesture::Marquee { scope, a, b: end, base, moved } => {
            *end = p;
            *moved |= far;
            if !*moved {
                return;
            }
            let area = Rect::spanning(*a, p);
            let scope = scope.clone();
            let mut sel = base.clone();
            drop(gesture);
            let placed = b.placed();
            let picked = hit::marquee(&b.doc.scene.borrow(), &placed, &scope, &area);
            for id in picked {
                if !sel.contains(&id) {
                    sel.push(id);
                }
            }
            b.select(sel);
            b.canvas.queue_draw();
        }
        Gesture::Move { moved, .. } if !*moved && !far => {}
        Gesture::Move { .. } => {
            drop(gesture);
            move_update(b, p, mods);
        }
        Gesture::Resize { .. } => {
            drop(gesture);
            resize_update(b, p, mods);
        }
        Gesture::Endpoint { id, which, orig, moved } => {
            *moved = true;
            let mut q = p;
            let pts = orig.points.clone().unwrap_or_default();
            let placed = b.placed();
            let origin = placed.rect(id).map(|r| (r.x, r.y)).unwrap_or((orig.x, orig.y));
            if shift(mods) {
                let other = if *which == 0 { pts.last() } else { pts.first() }.copied().unwrap_or([0.0, 0.0]);
                q = constrain45((origin.0 + other[0], origin.1 + other[1]), p);
            }
            let mut pts = pts;
            if let Some(pt) = pts.get_mut(*which) {
                *pt = [q.0 - origin.0, q.1 - origin.1];
            }
            let id = id.clone();
            drop(gesture);
            if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                n.points = Some(pts);
            }
            b.invalidate();
        }
        Gesture::Create { tool, a, b: end, from, to, .. } => {
            if matches!(tool, Tool::Line | Tool::Arrow) {
                *to = attachable(b, p).filter(|t| Some(t) != from.as_ref());
            }
            *end = if shift(mods) {
                if matches!(tool, Tool::Line | Tool::Arrow) {
                    constrain45(*a, p)
                } else {
                    let side = (p.0 - a.0).abs().max((p.1 - a.1).abs());
                    (a.0 + side * (p.0 - a.0).signum(), a.1 + side * (p.1 - a.1).signum())
                }
            } else {
                p
            };
            drop(gesture);
            b.canvas.queue_draw();
        }
        Gesture::Rotate { id, centre, start, orig, moved } => {
            *moved = true;
            let angle = (p.1 - centre.1).atan2(p.0 - centre.0).to_degrees();
            let mut turn = orig.rotation.unwrap_or(0.0) + angle - *start;
            if shift(mods) {
                turn = (turn / 15.0).round() * 15.0;
            }
            turn = ((turn + 180.0).rem_euclid(360.0) - 180.0) * 10.0;
            let turn = turn.round() / 10.0;
            let id = id.clone();
            drop(gesture);
            if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                n.rotation = Some(turn);
            }
            b.invalidate();
        }
        Gesture::Erase { .. } => {
            drop(gesture);
            erase_at(b, p);
        }
        Gesture::PenHandle { anchor } => {
            let anchor = *anchor;
            drop(gesture);
            b.pen_drag(anchor, p, alt(mods));
        }
        Gesture::VectorPoint { id, point, orig, rect, moved } => {
            *moved = true;
            let (id, point, orig, rect) = (id.clone(), *point, orig.clone(), *rect);
            drop(gesture);
            b.drag_point(&id, point, &orig, &rect, p, alt(mods));
        }
        Gesture::Draw { points, .. } => {
            let last = points.last().copied().unwrap_or([p.0, p.1]);
            if ((p.0 - last[0]).powi(2) + (p.1 - last[1]).powi(2)).sqrt() * zoom >= 1.5 {
                points.push([p.0, p.1]);
                drop(gesture);
                b.canvas.queue_draw();
            }
        }
    }
}

/// A frame's layout grid edges as lines to snap to (boxes of no width or height).
fn grid_lines(b: &Board, frame: Option<&Node>, r: &Rect) -> Vec<Rect> {
    let Some(frame) = frame.filter(|f| f.grids.is_some() && !b.extra.hide_grids.get()) else {
        return Vec::new();
    };
    let (xs, ys) = super::marks::grid_edges(frame, r);
    xs.into_iter().map(|x| Rect::new(x, r.y, 0.0, r.h)).chain(ys.into_iter().map(|y| Rect::new(r.x, y, r.w, 0.0))).collect()
}

fn constrain45(a: (f64, f64), p: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (p.0 - a.0, p.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);
    let step = std::f64::consts::FRAC_PI_4;
    let snapped = (angle / step).round() * step;
    (a.0 + len * snapped.cos(), a.1 + len * snapped.sin())
}

fn move_update(b: &Rc<Board>, p: (f64, f64), mods: gdk::ModifierType) {
    // Alt held at the press: the first move leaves a copy behind and drags the original's twin.
    let dup = matches!(&*b.gesture.borrow(), Gesture::Move { dup: true, moved: false, .. });
    if dup {
        let ids = match &*b.gesture.borrow() {
            Gesture::Move { ids, .. } => ids.clone(),
            _ => Vec::new(),
        };
        let (ops, roots) = edit::duplicate(&mut b.doc.scene.borrow_mut(), &ids, 0.0);
        if b.commit(ops) {
            let placed = b.placed();
            let scene = b.doc.scene.borrow();
            if let Gesture::Move { ids, orig, rects, dup, .. } = &mut *b.gesture.borrow_mut() {
                *ids = roots.clone();
                *orig = roots.iter().filter_map(|id| scene.node(id).map(|n| (id.clone(), n.x, n.y))).collect();
                *rects = roots.iter().filter_map(|id| placed.rect(id)).collect();
                *dup = false;
            }
            drop(scene);
            *b.selection.borrow_mut() = roots;
        }
    }
    b.doc.set_hold(true);
    let placed = b.placed();
    let zoom = b.cam.get().zoom;
    let (ids, start, orig, rects) = match &mut *b.gesture.borrow_mut() {
        Gesture::Move { ids, start, orig, rects, moved, at, .. } => {
            *moved = true;
            *at = p;
            (ids.clone(), *start, orig.clone(), rects.clone())
        }
        _ => return,
    };
    let mut dx = p.0 - start.0;
    let mut dy = p.1 - start.1;
    if shift(mods) {
        if dx.abs() > dy.abs() { dy = 0.0 } else { dx = 0.0 }
    }
    let scene_ref = b.doc.scene.borrow();
    let parent = ids.first().and_then(|id| scene_ref.parent(id)).map(str::to_string).unwrap_or_default();
    let laid = edit::lays_out(&scene_ref, &parent);
    let parent_rect = placed.rect(&parent);
    // One node of an auto-layout frame, still inside it: reorder.
    if laid && ids.len() == 1 && parent_rect.is_some_and(|r| r.contains(p.0, p.1)) {
        let row = scene_ref.node(&parent).is_some_and(|n| n.layout_mode() == Layout::Row);
        let siblings: Vec<Rect> = scene_ref
            .children(&parent)
            .iter()
            .filter(|c| **c != ids[0])
            .filter(|c| scene_ref.node(c).is_some_and(|n| !n.is_hidden()))
            .filter_map(|c| placed.rect(c))
            .collect();
        let index = siblings.iter().filter(|r| if row { p.0 > r.cx() } else { p.1 > r.cy() }).count() as u32;
        drop(scene_ref);
        if let Gesture::Move { reorder, target, .. } = &mut *b.gesture.borrow_mut() {
            *reorder = Some((parent, index));
            *target = None;
        }
        b.guides.borrow_mut().clear();
        b.canvas.queue_draw();
        return;
    }
    // Snap the moving box to its neighbours and its parent.
    let moving = Rect::bounds(rects.iter()).unwrap_or_default().translate(dx, dy);
    let view = b.cam.get().visible(b.size.get().0, b.size.get().1);
    let mut others: Vec<Rect> = scene_ref
        .children(&parent)
        .iter()
        .filter(|c| !ids.contains(c))
        .filter(|c| scene_ref.node(c).is_some_and(|n| !n.is_hidden()))
        .filter_map(|c| placed.rect(c))
        .filter(|r| r.intersects(&view))
        .collect();
    if let Some(pr) = parent_rect.filter(|_| scene_ref.node(&parent).is_some_and(|n| n.kind != NodeKind::Page)) {
        others.push(pr);
        others.extend(grid_lines(b, scene_ref.node(&parent), &pr));
    }
    let (sx, sy, guides) = if alt(mods) && !dup {
        (0.0, 0.0, Vec::new())
    } else {
        snap::snap(&moving, &others, Edges::ALL, SNAP / zoom)
    };
    dx += sx;
    dy += sy;
    // Where it would land: the deepest frame under the pointer that is not one of them.
    let page = b.page.borrow().clone();
    let chain = hit::chain(&scene_ref, &placed, &page, p.0, p.1, 0.0);
    let landing = chain
        .iter()
        .rev()
        .find(|id| {
            scene_ref.node(id).is_some_and(|n| n.kind == NodeKind::Frame && !n.is_locked())
                && !ids.iter().any(|m| *m == **id || scene_ref.is_inside(id, m))
        })
        .cloned()
        .unwrap_or(page);
    let target = if landing != parent { Some(landing) } else { None };
    let free: Vec<(String, f64, f64)> = orig
        .into_iter()
        .filter(|(id, _, _)| scene_ref.parent(id).is_some_and(|p| !edit::lays_out(&scene_ref, p)))
        .collect();
    drop(scene_ref);
    {
        let mut scene = b.doc.scene.borrow_mut();
        for (id, ox, oy) in &free {
            if let Some(n) = scene.node_mut(id) {
                n.x = (ox + dx).round();
                n.y = (oy + dy).round();
            }
        }
    }
    if let Gesture::Move { target: t, reorder, .. } = &mut *b.gesture.borrow_mut() {
        *t = target;
        *reorder = None;
    }
    *b.guides.borrow_mut() = guides;
    b.invalidate();
}

fn resize_update(b: &Rc<Board>, p: (f64, f64), mods: gdk::ModifierType) {
    let (handle, bounds, orig) = match &mut *b.gesture.borrow_mut() {
        Gesture::Resize { handle, bounds, orig, moved } => {
            *moved = true;
            (*handle, *bounds, orig.clone())
        }
        _ => return,
    };
    let zoom = b.cam.get().zoom;
    // One turned node resizes in its own frame: the pointer is turned back about its centre,
    // the dragged edges move there, and the new box is turned into place again.
    let turn = match orig.as_slice() {
        [(_, node, _)] => relay_client::design::shapes::rotation(node),
        _ => 0.0,
    };
    let p = if turn != 0.0 { relay_client::design::shapes::rotate(p.0, p.1, bounds.cx(), bounds.cy(), -turn) } else { p };
    let (mut x0, mut y0, mut x1, mut y1) = (bounds.x, bounds.y, bounds.right(), bounds.bottom());
    let centred = alt(mods);
    if handle.left {
        x0 = p.0;
        if centred {
            x1 = bounds.right() + (bounds.x - p.0);
        }
    }
    if handle.right {
        x1 = p.0;
        if centred {
            x0 = bounds.x - (p.0 - bounds.right());
        }
    }
    if handle.top {
        y0 = p.1;
        if centred {
            y1 = bounds.bottom() + (bounds.y - p.1);
        }
    }
    if handle.bottom {
        y1 = p.1;
        if centred {
            y0 = bounds.y - (p.1 - bounds.bottom());
        }
    }
    // Snap the dragged edges.
    let page = b.page.borrow().clone();
    let placed = b.placed();
    let parent = orig.first().and_then(|(id, _, _)| b.doc.scene.borrow().parent(id).map(str::to_string)).unwrap_or(page);
    let ids: Vec<&String> = orig.iter().map(|(id, _, _)| id).collect();
    let others: Vec<Rect> = {
        let scene = b.doc.scene.borrow();
        let mut others: Vec<Rect> = scene.children(&parent).iter().filter(|c| !ids.contains(c)).filter_map(|c| placed.rect(c)).collect();
        if let Some(pr) = placed.rect(&parent) {
            others.extend(grid_lines(b, scene.node(&parent), &pr));
        }
        others
    };
    let moving = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
    let edges = Edges {
        left: handle.left,
        center_x: false,
        right: handle.right,
        top: handle.top,
        center_y: false,
        bottom: handle.bottom,
    };
    let (sx, sy, guides) = if turn != 0.0 { (0.0, 0.0, Vec::new()) } else { snap::snap(&moving, &others, edges, SNAP / zoom) };
    if handle.left { x0 += sx } else if handle.right { x1 += sx }
    if handle.top { y0 += sy } else if handle.bottom { y1 += sy }
    let keep_ratio = shift(mods) || (orig.len() == 1 && orig[0].1.kind == NodeKind::Image && !shift(mods));
    if keep_ratio && bounds.w > 0.0 && bounds.h > 0.0 && (handle.left || handle.right) && (handle.top || handle.bottom) {
        let ratio = bounds.w / bounds.h;
        let w = (x1 - x0).abs();
        let h = (y1 - y0).abs();
        if w / ratio > h {
            let nh = w / ratio;
            if handle.top { y0 = y1 - nh * (y1 - y0).signum() } else { y1 = y0 + nh * (y1 - y0).signum() }
        } else {
            let nw = h * ratio;
            if handle.left { x0 = x1 - nw * (x1 - x0).signum() } else { x1 = x0 + nw * (x1 - x0).signum() }
        }
    }
    let mut new = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0));
    if turn != 0.0 {
        let (cx, cy) = relay_client::design::shapes::rotate(new.cx(), new.cy(), bounds.cx(), bounds.cy(), turn);
        new = Rect::new(cx - new.w / 2.0, cy - new.h / 2.0, new.w, new.h);
    }
    let sx = new.w / bounds.w.max(1e-9);
    let sy = new.h / bounds.h.max(1e-9);
    {
        let mut scene = b.doc.scene.borrow_mut();
        for (id, node, r) in &orig {
            let nx = new.x + (r.x - bounds.x) * sx;
            let ny = new.y + (r.y - bounds.y) * sy;
            let (nw, nh) = ((r.w * sx).max(1.0), (r.h * sy).max(1.0));
            if let Some(n) = scene.node_mut(id) {
                n.x = (node.x + (nx - r.x)).round();
                n.y = (node.y + (ny - r.y)).round();
                if handle.left || handle.right {
                    n.w = nw.round();
                    if n.kind == NodeKind::Text || layout::w_sizing(node) == Sizing::Hug {
                        n.w_sizing = Some(Sizing::Fixed);
                    }
                }
                if handle.top || handle.bottom {
                    n.h = nh.round();
                    if layout::h_sizing(node) == Sizing::Hug {
                        n.h_sizing = Some(Sizing::Fixed);
                    }
                }
                if let (Some(pts), true) = (&node.points, matches!(node.kind, NodeKind::Path | NodeKind::Line)) {
                    n.points = Some(pts.iter().map(|q| [q[0] * sx, q[1] * sy]).collect());
                }
            }
        }
    }
    if let [(id, node, _)] = orig.as_slice() {
        let size = b.doc.scene.borrow().node(id).map(|n| (n.w, n.h));
        if let Some((w, h)) = size {
            b.pinned_follow(node, w, h);
        }
    }
    *b.guides.borrow_mut() = guides;
    b.invalidate();
}

fn finish(b: &Rc<Board>, _x: f64, _y: f64, mods: gdk::ModifierType) {
    let gesture = std::mem::replace(&mut *b.gesture.borrow_mut(), Gesture::Idle);
    b.guides.borrow_mut().clear();
    match gesture {
        Gesture::Idle => {}
        Gesture::Pan { .. } => update_cursor(b, None),
        Gesture::Marquee { scope, moved, .. } => {
            if !moved && scope != *b.page.borrow() {
                if shift(mods) {
                    toggle(b, &scope);
                } else {
                    b.select(vec![scope]);
                }
            }
            b.canvas.queue_draw();
        }
        Gesture::Move { ids, orig, moved, target, reorder, click, at, start, .. } => {
            if !moved {
                if let Some(c) = click.filter(|_| !shift(mods)) {
                    b.select(vec![c]);
                }
                b.doc.set_hold(false);
                b.canvas.queue_draw();
                return;
            }
            // The preview's final (snapped) positions, then the originals back, then the edits.
            let finals: Vec<(String, f64, f64)> = {
                let scene = b.doc.scene.borrow();
                orig.iter().filter_map(|(id, _, _)| scene.node(id).map(|n| (id.clone(), n.x, n.y))).collect()
            };
            {
                let mut scene = b.doc.scene.borrow_mut();
                for (id, ox, oy) in &orig {
                    if let Some(n) = scene.node_mut(id) {
                        n.x = *ox;
                        n.y = *oy;
                    }
                }
            }
            b.invalidate();
            let placed_before = b.placed();
            let scene = b.doc.scene.borrow();
            let delta = |id: &str| -> (f64, f64) {
                let free = scene.parent(id).is_some_and(|p| !edit::lays_out(&scene, p));
                match (free, orig.iter().find(|o| o.0 == id), finals.iter().find(|f| f.0 == id)) {
                    (true, Some(o), Some(f)) => (f.1 - o.1, f.2 - o.2),
                    _ => (at.0 - start.0, at.1 - start.1),
                }
            };
            let mut ops = Vec::new();
            if let Some((parent, index)) = reorder {
                ops.push(NodeOp::Move { id: ids[0].clone(), parent: Some(parent), index: Some(index) });
            } else if let Some(t) = target {
                let index = edit::lays_out(&scene, &t).then(|| {
                    let row = scene.node(&t).is_some_and(|n| n.layout_mode() == Layout::Row);
                    scene
                        .children(&t)
                        .iter()
                        .filter_map(|c| placed_before.rect(c))
                        .filter(|r| if row { at.0 > r.cx() } else { at.1 > r.cy() })
                        .count() as u32
                });
                for id in &ids {
                    let r = placed_before.rect(id).unwrap_or_default();
                    let (dx, dy) = delta(id);
                    ops.extend(edit::reparent(&scene, &placed_before, id, &t, index, (r.x + dx, r.y + dy)));
                }
            } else {
                for (id, ox, oy) in &orig {
                    let Some((_, fx, fy)) = finals.iter().find(|f| f.0 == *id) else {
                        continue;
                    };
                    if (fx - ox).abs() >= 0.5 || (fy - oy).abs() >= 0.5 {
                        let mut set = Map::new();
                        set.insert("x".into(), json!(fx));
                        set.insert("y".into(), json!(fy));
                        ops.push(NodeOp::Update { id: id.clone(), set });
                    }
                }
            }
            drop(scene);
            b.commit(ops);
            b.doc.set_hold(false);
        }
        Gesture::Resize { orig, moved, .. } => {
            if moved {
                let finals: Vec<(String, Node)> = {
                    let scene = b.doc.scene.borrow();
                    orig.iter().filter_map(|(id, _, _)| Some((id.clone(), scene.node(id)?.clone()))).collect()
                };
                {
                    let mut scene = b.doc.scene.borrow_mut();
                    for (id, node, _) in &orig {
                        if let Some(n) = scene.node_mut(id) {
                            *n = node.clone();
                        }
                    }
                }
                let mut ops: Vec<NodeOp> = finals
                    .into_iter()
                    .filter_map(|(id, after)| {
                        let before = orig.iter().find(|(o, _, _)| *o == id)?.1.clone();
                        diff_op(&id, &before, &after)
                    })
                    .collect();
                ops.extend(b.pinned_finish(true));
                b.commit(ops);
            } else {
                b.pinned_finish(false);
            }
            b.doc.set_hold(false);
            b.invalidate();
        }
        Gesture::Endpoint { id, orig, moved, .. } => {
            if moved {
                let after = b.doc.scene.borrow().node(&id).cloned();
                if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                    *n = orig.clone();
                }
                if let Some(after) = after {
                    let normalized = normalize_line(after);
                    if let Some(op) = diff_op(&id, &orig, &normalized) {
                        b.commit(vec![op]);
                    }
                }
            }
            b.doc.set_hold(false);
            b.invalidate();
        }
        Gesture::Create { tool, parent, a, b: end, from, to } => create_shape(b, tool, parent, a, end, from, to),
        Gesture::Rotate { id, orig, moved, .. } => {
            if moved {
                let turned = b.doc.scene.borrow().node(&id).and_then(|n| n.rotation);
                if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                    n.rotation = orig.rotation;
                }
                let mut set = Map::new();
                set.insert("rotation".into(), turned.filter(|r| r.abs() > 0.05).map_or(Value::Null, |r| json!(r)));
                b.commit(vec![NodeOp::Update { id, set }]);
            }
            b.doc.set_hold(false);
            b.invalidate();
        }
        Gesture::Erase { hits } => {
            {
                let mut scene = b.doc.scene.borrow_mut();
                for (id, opacity) in &hits {
                    if let Some(n) = scene.node_mut(id) {
                        n.opacity = *opacity;
                    }
                }
            }
            b.doc.set_hold(false);
            let ops = hits.into_iter().map(|(id, _)| NodeOp::Delete { id }).collect();
            b.commit(ops);
            b.invalidate();
        }
        Gesture::Draw { parent, points } => finish_drawing(b, parent, points),
        Gesture::PenHandle { .. } => {}
        Gesture::VectorPoint { id, orig, moved, .. } => {
            if moved {
                let after = b.doc.scene.borrow().node(&id).cloned();
                if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                    *n = orig.clone();
                }
                if let Some(op) = after.and_then(|a| diff_op(&id, &orig, &a)) {
                    b.commit(vec![op]);
                }
            }
            b.doc.set_hold(false);
            b.invalidate();
        }
    }
    update_cursor(b, None);
}

/// The fields that differ between two versions of a node, as one update.
fn diff_op(id: &str, before: &Node, after: &Node) -> Option<NodeOp> {
    let (Ok(Value::Object(a)), Ok(Value::Object(z))) = (serde_json::to_value(before), serde_json::to_value(after)) else {
        return None;
    };
    let mut set = Map::new();
    for (k, v) in &z {
        if a.get(k) != Some(v) && !matches!(k.as_str(), "id" | "parent" | "kind") {
            set.insert(k.clone(), v.clone());
        }
    }
    for k in a.keys() {
        if !z.contains_key(k) && !matches!(k.as_str(), "id" | "parent" | "kind") {
            set.insert(k.clone(), Value::Null);
        }
    }
    (!set.is_empty()).then(|| NodeOp::Update { id: id.to_string(), set })
}

/// A line whose points start at its own origin again, with its box around them.
fn normalize_line(mut n: Node) -> Node {
    let Some(pts) = n.points.clone().filter(|p| !p.is_empty()) else {
        return n;
    };
    let min_x = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let min_y = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let max_y = pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    n.x = (n.x + min_x).round();
    n.y = (n.y + min_y).round();
    n.w = (max_x - min_x).round();
    n.h = (max_y - min_y).round();
    n.points = Some(pts.iter().map(|p| [(p[0] - min_x).round(), (p[1] - min_y).round()]).collect());
    n
}

fn create_shape(b: &Rc<Board>, tool: Tool, parent: String, a: (f64, f64), end: (f64, f64), from: Option<String>, to: Option<String>) {
    let zoom = b.cam.get().zoom;
    let dragged = ((end.0 - a.0).abs().max((end.1 - a.1).abs())) * zoom > DRAG;
    let origin = b.placed().rect(&parent).unwrap_or_default();
    let kind = match tool {
        Tool::Frame => NodeKind::Frame,
        Tool::Ellipse => NodeKind::Ellipse,
        Tool::Line | Tool::Arrow => NodeKind::Line,
        t if super::board::polygon_of(t).is_some() => NodeKind::Polygon,
        _ => NodeKind::Rect,
    };
    let mut n = Node::new(kind);
    n.id = b.doc.scene.borrow_mut().fresh_id(kind);
    n.parent = Some(parent.clone());
    if kind == NodeKind::Line {
        let end = if dragged { end } else { (a.0 + 120.0, a.1) };
        let r = Rect::spanning(a, end);
        n.x = (r.x - origin.x).round();
        n.y = (r.y - origin.y).round();
        n.w = r.w.round();
        n.h = r.h.round();
        n.points = Some(vec![[(a.0 - r.x).round(), (a.1 - r.y).round()], [(end.0 - r.x).round(), (end.1 - r.y).round()]]);
        n.stroke = Some(ink_for(b, &parent));
        n.stroke_width = Some(2.0);
        if tool == Tool::Arrow {
            n.arrow_end = Some(true);
        }
        n.from = from;
        n.to = to;
    } else {
        let r = if dragged {
            Rect::spanning(a, end)
        } else {
            let (w, h) = if kind == NodeKind::Frame { (390.0, 844.0) } else { (120.0, 120.0) };
            Rect::new(a.0 - if kind == NodeKind::Frame { 0.0 } else { w / 2.0 }, a.1 - if kind == NodeKind::Frame { 0.0 } else { h / 2.0 }, w, h)
        };
        n.x = (r.x - origin.x).round();
        n.y = (r.y - origin.y).round();
        n.w = r.w.round().max(1.0);
        n.h = r.h.round().max(1.0);
        if let Some((sides, star)) = super::board::polygon_of(tool) {
            n.sides = Some(sides);
            n.star = star;
        }
        if kind == NodeKind::Frame {
            let count = b.doc.scene.borrow().children(&parent).iter().filter(|c| b.doc.scene.borrow().node(c).is_some_and(|n| n.kind == NodeKind::Frame)).count();
            n.name = Some(format!("Frame {}", count + 1));
        }
    }
    let id = n.id.clone();
    if b.commit(vec![NodeOp::Add { node: n, index: None }]) {
        b.select(vec![id]);
    }
    if !b.tool_lock.get() {
        b.set_tool(Tool::Select);
    }
}

fn finish_drawing(b: &Rc<Board>, parent: String, points: Vec<[f64; 2]>) {
    let zoom = b.cam.get().zoom;
    let highlighter = b.tool.get() == Tool::Highlighter;
    let simple = simplify(&points, 0.6 / zoom);
    let min_x = simple.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let min_y = simple.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_x = simple.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let max_y = simple.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    if !min_x.is_finite() {
        return;
    }
    let origin = b.placed().rect(&parent).unwrap_or_default();
    let mut n = Node::new(NodeKind::Path);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Path);
    n.parent = Some(parent);
    n.x = ((min_x - origin.x) * 100.0).round() / 100.0;
    n.y = ((min_y - origin.y) * 100.0).round() / 100.0;
    n.w = (max_x - min_x).max(1.0);
    n.h = (max_y - min_y).max(1.0);
    n.points = Some(simple.iter().map(|p| [((p[0] - min_x) * 10.0).round() / 10.0, ((p[1] - min_y) * 10.0).round() / 10.0]).collect());
    n.stroke = Some(b.pen_color(highlighter));
    n.stroke_width = Some(if highlighter { 16.0 } else { 3.0 });
    if highlighter {
        n.name = Some("Highlight".into());
    }
    b.commit(vec![NodeOp::Add { node: n, index: None }]);
}

fn create_sticky(b: &Rc<Board>, p: (f64, f64)) {
    let parent = container_at(b, p);
    let origin = b.placed().rect(&parent).unwrap_or_default();
    let mut n = Node::new(NodeKind::Sticky);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Sticky);
    n.parent = Some(parent);
    n.x = (p.0 - origin.x - 100.0).round();
    n.y = (p.1 - origin.y - 100.0).round();
    n.w = 200.0;
    n.h = 200.0;
    n.fill = Some(STICKY_COLORS[0].1.into());
    let id = n.id.clone();
    if b.commit(vec![NodeOp::Add { node: n, index: None }]) {
        b.select(vec![id.clone()]);
        b.begin_edit(&id, false);
    }
    if !b.tool_lock.get() {
        b.set_tool(Tool::Select);
    }
}

/// Sticky papers, FigJam's family of soft tones.
pub const STICKY_COLORS: [(&str, &str); 6] = [
    ("Yellow", "#FFE58A"),
    ("Orange", "#FFC48A"),
    ("Pink", "#FFB3C7"),
    ("Violet", "#D3C2FF"),
    ("Blue", "#AEDBFF"),
    ("Green", "#B8EDB0"),
];

fn cancel(b: &Rc<Board>) {
    let gesture = std::mem::replace(&mut *b.gesture.borrow_mut(), Gesture::Idle);
    {
        let mut scene = b.doc.scene.borrow_mut();
        match &gesture {
            Gesture::Move { orig, .. } => {
                for (id, x, y) in orig {
                    if let Some(n) = scene.node_mut(id) {
                        n.x = *x;
                        n.y = *y;
                    }
                }
            }
            Gesture::Resize { orig, .. } => {
                for (id, node, _) in orig {
                    if let Some(n) = scene.node_mut(id) {
                        *n = node.clone();
                    }
                }
            }
            Gesture::Endpoint { id, orig, .. } | Gesture::Rotate { id, orig, .. } | Gesture::VectorPoint { id, orig, .. } => {
                if let Some(n) = scene.node_mut(id) {
                    *n = orig.clone();
                }
            }
            Gesture::Erase { hits } => {
                for (id, opacity) in hits {
                    if let Some(n) = scene.node_mut(id) {
                        n.opacity = *opacity;
                    }
                }
            }
            _ => {}
        }
    }
    if matches!(gesture, Gesture::Resize { .. }) {
        b.pinned_finish(false);
    }
    b.guides.borrow_mut().clear();
    b.doc.set_hold(false);
    b.invalidate();
    update_cursor(b, None);
}

fn double_click(b: &Rc<Board>, x: f64, y: f64, p: (f64, f64)) {
    if b.tool.get() == Tool::Vector {
        b.finish_pen();
        return;
    }
    if b.tool.get() != Tool::Select {
        return;
    }
    if let Some(id) = b.frame_label_at(x, y) {
        super::layers::rename_node(b, &id, None);
        return;
    }
    let placed = b.placed();
    let page = b.page.borrow().clone();
    let chain = hit::chain(&b.doc.scene.borrow(), &placed, &page, p.0, p.1, 4.0 / b.cam.get().zoom);
    let Some(deep) = chain.last().cloned() else {
        let parent = page;
        b.create_text(p, parent);
        return;
    };
    let kind = b.doc.scene.borrow().node(&deep).map(|n| n.kind);
    match kind {
        Some(NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) => {
            b.select(vec![deep.clone()]);
            b.begin_edit(&deep, false);
        }
        Some(NodeKind::Image) => super::assets::replace_image(b, &deep),
        Some(NodeKind::Vector) => b.edit_points(&deep),
        _ => {
            // Into the selected container: the next node down under the pointer.
            let selected = b.selection.borrow().clone();
            let next = match selected.as_slice() {
                [one] => chain.iter().position(|c| c == one).and_then(|i| chain.get(i + 1)).cloned(),
                _ => None,
            };
            b.select(vec![next.unwrap_or(deep)]);
        }
    }
}

fn hover(b: &Rc<Board>, x: f64, y: f64) {
    b.pointer.set((x, y));
    if !matches!(*b.gesture.borrow(), Gesture::Idle) {
        return;
    }
    if b.space.get() || b.tool.get() == Tool::Hand {
        update_cursor(b, None);
        return;
    }
    if b.tool.get() == Tool::Select {
        if is_line_selected(b).is_some_and(|id| b.endpoints(&id).iter().any(|(ex, ey)| (x - ex).abs() <= 7.0 && (y - ey).abs() <= 7.0)) {
            update_cursor(b, Some("crosshair"));
        } else if let Some(h) = handle_at(b, x, y).filter(|_| is_line_selected(b).is_none()) {
            update_cursor(b, Some(h.cursor()));
        } else if rotate_at(b, x, y).is_some() {
            update_cursor(b, Some("grab"));
        } else {
            update_cursor(b, None);
        }
    }
    let hovered = if b.tool.get() == Tool::Select {
        b.frame_label_at(x, y).or_else(|| {
            let p = b.to_page(x, y);
            let placed = b.placed();
            let page = b.page.borrow().clone();
            hit::chain(&b.doc.scene.borrow(), &placed, &page, p.0, p.1, 4.0 / b.cam.get().zoom).last().cloned()
        })
    } else {
        None
    };
    if *b.hover.borrow() != hovered {
        *b.hover.borrow_mut() = hovered;
        b.canvas.queue_draw();
    }
}

fn context_menu(b: &Rc<Board>, x: f64, y: f64) {
    let p = b.to_page(x, y);
    let placed = b.placed();
    let page = b.page.borrow().clone();
    let deep = hit::chain(&b.doc.scene.borrow(), &placed, &page, p.0, p.1, 4.0 / b.cam.get().zoom).last().cloned();
    if let Some(d) = &deep {
        if !b.selection.borrow().contains(d) {
            b.select(vec![d.clone()]);
        }
    }
    let menu = gio::Menu::new();
    let section = gio::Menu::new();
    section.append(Some("Cut"), Some("design.cut"));
    section.append(Some("Copy"), Some("design.copy"));
    section.append(Some("Paste"), Some("design.paste"));
    section.append(Some("Duplicate"), Some("design.duplicate"));
    section.append(Some("Delete"), Some("design.delete"));
    menu.append_section(None, &section);
    if !b.selection.borrow().is_empty() {
        let section = gio::Menu::new();
        section.append(Some("Bring to front"), Some("design.front"));
        section.append(Some("Bring forward"), Some("design.forward"));
        section.append(Some("Send backward"), Some("design.backward"));
        section.append(Some("Send to back"), Some("design.back"));
        menu.append_section(None, &section);
        let section = gio::Menu::new();
        section.append(Some("Group"), Some("design.group"));
        section.append(Some("Frame selection"), Some("design.frame"));
        section.append(Some("Ungroup"), Some("design.ungroup"));
        section.append(Some("Lock / unlock"), Some("design.lock"));
        let comps = gio::Menu::new();
        let (component, copy) = {
            let sel = b.selected();
            let scene = b.doc.scene.borrow();
            (
                sel.first().and_then(|id| scene.node(id)).is_some_and(|n| n.component == Some(true)),
                sel.first().and_then(|id| b.instance_root(id)).is_some(),
            )
        };
        if component {
            comps.append(Some("Make a copy of the component"), Some("design.create-instance"));
        } else if copy {
            comps.append(Some("Go to its component"), Some("design.go-to-component"));
            comps.append(Some("Reset its own changes"), Some("design.reset-overrides"));
            comps.append(Some("Detach from the component"), Some("design.detach-instance"));
        } else {
            comps.append(Some("Make a component"), Some("design.make-component"));
        }
        menu.append_section(None, &comps);
        section.append(Some("Hide"), Some("design.hide"));
        menu.append_section(None, &section);
        let shape = gio::Menu::new();
        shape.append(Some("Flip horizontal"), Some("design.flip-x"));
        shape.append(Some("Flip vertical"), Some("design.flip-y"));
        let one = b.selected();
        let kind = match one.as_slice() {
            [id] => b.doc.scene.borrow().node(id).map(|n| n.kind),
            _ => None,
        };
        match kind {
            Some(NodeKind::Vector) => shape.append(Some("Edit points"), Some("design.edit-points")),
            Some(NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Line | NodeKind::Path) => {
                shape.append(Some("Turn into a vector"), Some("design.to-vector"))
            }
            _ => {}
        }
        if matches!(kind, Some(NodeKind::Frame)) {
            shape.append(Some("Copy as HTML and CSS"), Some("design.copy-code"));
        }
        if matches!(kind, Some(k) if k != NodeKind::Frame && k != NodeKind::Group) {
            shape.append(Some("Make a style of this"), Some("design.make-style"));
        }
        menu.append_section(None, &shape);
        let section = gio::Menu::new();
        section.append(Some("Ask the agent about this…"), Some("design.ask"));
        section.append(Some("Export as PNG…"), Some("design.export"));
        section.append(Some("Export as SVG…"), Some("design.export-svg"));
        menu.append_section(None, &section);
    } else {
        let section = gio::Menu::new();
        section.append(Some("Select all"), Some("design.select-all"));
        section.append(Some("Zoom to fit"), Some("design.zoom-fit"));
        section.append(Some("Add a page"), Some("design.add-page"));
        menu.append_section(None, &section);
    }
    let pop = gtk::PopoverMenu::from_model(Some(&menu));
    pop.add_css_class("design-menu");
    pop.set_parent(&b.canvas);
    pop.set_has_arrow(false);
    pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

fn key_pressed(
    b: &Rc<Board>,
    c: &gtk::EventControllerKey,
    key: gdk::Key,
    code: u32,
    mods: gdk::ModifierType,
) -> glib::Propagation {
    use gdk::Key;
    let ctrl = mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::META_MASK);
    let shift = mods.contains(gdk::ModifierType::SHIFT_MASK);
    let alt = mods.contains(gdk::ModifierType::ALT_MASK);
    let key = crate::shortcuts::latin(key, code);
    // The unshifted key, for Shift+digit chords on any layout.
    let base = c
        .widget()
        .map(|w| WidgetExt::display(&w))
        .and_then(|d| d.translate_key(code, gdk::ModifierType::empty(), c.group() as i32))
        .map(|(k, ..)| k)
        .unwrap_or(key);
    let stop = glib::Propagation::Stop;
    if matches!(key, Key::Alt_L | Key::Alt_R) {
        if !b.extra.alt.replace(true) {
            b.canvas.queue_draw();
        }
        return glib::Propagation::Proceed;
    }
    // The pen's path, or a vector's points, end with Enter or Escape.
    if matches!(key, Key::Return | Key::KP_Enter | Key::Escape) && !ctrl {
        if b.extra.pen.borrow().is_some() {
            b.finish_pen();
            return stop;
        }
        if b.stop_editing_points() {
            return stop;
        }
    }
    if key == Key::space && !ctrl {
        if !b.space.replace(true) {
            update_cursor(b, Some("grab"));
        }
        return stop;
    }
    if ctrl {
        match key.to_lower() {
            Key::z if shift => b.redo(),
            Key::z => b.undo(),
            Key::y => b.redo(),
            Key::c => b.copy(),
            Key::x => {
                b.copy();
                b.delete_selection();
            }
            Key::v => b.paste(),
            Key::d => b.duplicate(),
            Key::a => b.select_all(),
            Key::g if shift => b.ungroup(),
            Key::g if alt => b.group(NodeKind::Frame),
            Key::k if alt => b.make_component(),
            Key::g => b.group(NodeKind::Group),
            Key::bracketright | Key::braceright => b.restack(true, shift),
            Key::bracketleft | Key::braceleft => b.restack(false, shift),
            Key::equal | Key::plus | Key::KP_Add => b.step_zoom(1),
            Key::minus | Key::underscore | Key::KP_Subtract => b.step_zoom(-1),
            Key::_0 | Key::KP_0 => b.zoom_to(1.0),
            Key::period => b.inspector_key.set_active(!b.inspector_key.is_active()),
            Key::b => b.toggle_text("font_weight"),
            Key::i => b.toggle_text("italic"),
            Key::u => b.toggle_text("underline"),
            Key::l if shift => b.toggle_flag("locked"),
            Key::h if shift => b.toggle_flag("hidden"),
            Key::k if shift => super::assets::choose_images(b),
            Key::Return | Key::KP_Enter if alt => b.present(),
            Key::Return | Key::KP_Enter => b.ask_agent(b.ask.upcast_ref()),
            Key::e if shift => super::export::export_vector(b, super::export::Vector::Svg),
            _ => return glib::Propagation::Proceed,
        }
        return stop;
    }
    if alt {
        let how = match key.to_lower() {
            Key::a => Some(Align::Left),
            Key::d => Some(Align::Right),
            Key::w => Some(Align::Top),
            Key::s => Some(Align::Bottom),
            Key::h => Some(Align::CenterX),
            Key::v => Some(Align::CenterY),
            _ => None,
        };
        if let Some(how) = how {
            b.align(how);
            return stop;
        }
        if base == Key::_1 {
            if let Some(key) = find_layers_key(b) {
                key.set_active(!key.is_active());
            }
            return stop;
        }
        return glib::Propagation::Proceed;
    }
    if shift && matches!(key.to_lower(), Key::h | Key::v) {
        b.flip(if key.to_lower() == Key::h { "flip_x" } else { "flip_y" });
        return stop;
    }
    if shift {
        match base {
            Key::_0 => b.zoom_to(1.0),
            Key::_1 => {
                let r = b.page_bounds();
                b.fit(r, false);
            }
            Key::_2 => {
                let r = b.selection_bounds().or_else(|| b.page_bounds());
                b.fit(r, true);
            }
            _ => {}
        }
        if matches!(base, Key::_0 | Key::_1 | Key::_2) {
            return stop;
        }
    }
    let step = if shift { 10.0 } else { 1.0 };
    match key {
        Key::Escape => {
            if !matches!(*b.gesture.borrow(), Gesture::Idle) {
                cancel(b);
            } else if b.tool.get() != Tool::Select {
                b.set_tool(Tool::Select);
            } else {
                select_parent(b);
            }
        }
        Key::Delete | Key::BackSpace => b.delete_selection(),
        Key::Return | Key::KP_Enter => {
            let sel = b.selected();
            if let [one] = sel.as_slice() {
                let kind = b.doc.scene.borrow().node(one).map(|n| n.kind);
                match kind {
                    Some(NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) => b.begin_edit(one, false),
                    Some(k) if k.holds_children() => {
                        let kids = b.doc.scene.borrow().children(one).to_vec();
                        if !kids.is_empty() {
                            b.select(kids);
                        }
                    }
                    _ => {}
                }
            }
        }
        Key::Tab | Key::ISO_Left_Tab => select_sibling(b, if shift || key == Key::ISO_Left_Tab { -1 } else { 1 }),
        Key::Left => b.nudge(-step, 0.0),
        Key::Right => b.nudge(step, 0.0),
        Key::Up => b.nudge(0.0, -step),
        Key::Down => b.nudge(0.0, step),
        Key::question | Key::slash if shift || key == Key::question => {
            if let Some(help) = find_help_key(b) {
                help.emit_clicked();
            }
        }
        _ => {
            let tool = match key.to_lower() {
                Key::v => Some(Tool::Select),
                Key::h => Some(Tool::Hand),
                Key::f | Key::numbersign => Some(Tool::Frame),
                Key::r if shift => Some(Tool::Triangle),
                Key::r => Some(Tool::Rect),
                Key::e => Some(Tool::Eraser),
                Key::o => Some(Tool::Ellipse),
                Key::l if shift => Some(Tool::Arrow),
                Key::l => Some(Tool::Line),
                Key::t => Some(Tool::Text),
                Key::s if shift => Some(Tool::Star),
                Key::s => Some(Tool::Sticky),
                Key::p if shift => Some(Tool::Highlighter),
                Key::p => Some(Tool::Pen),
                Key::b => Some(Tool::Vector),
                Key::c => Some(Tool::Comment),
                _ => None,
            };
            if key.to_lower() == Key::k {
                if let Some(picker) = find_named(b.overlay.upcast_ref(), "design-icon-key").and_downcast::<gtk::MenuButton>() {
                    picker.popup();
                }
                return stop;
            }
            match tool {
                Some(t) => b.set_tool(t),
                None => return glib::Propagation::Proceed,
            }
        }
    }
    stop
}

fn find_layers_key(b: &Board) -> Option<gtk::ToggleButton> {
    find_named(b.overlay.upcast_ref(), "design-layers-key").and_downcast()
}

fn find_help_key(b: &Board) -> Option<gtk::Button> {
    let mut stack = vec![b.overlay.clone().upcast::<gtk::Widget>()];
    while let Some(w) = stack.pop() {
        if let Some(button) = w.downcast_ref::<gtk::Button>() {
            if button.tooltip_text().is_some_and(|t| t.starts_with("Keyboard shortcuts")) {
                return Some(button.clone());
            }
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
    None
}

pub fn find_named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    let mut stack = vec![root.clone()];
    while let Some(w) = stack.pop() {
        if w.widget_name() == name {
            return Some(w);
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
    None
}

fn select_parent(b: &Board) {
    let sel = b.selected();
    let scene = b.doc.scene.borrow();
    let parents: Vec<String> = sel
        .iter()
        .filter_map(|id| scene.parent(id))
        .filter(|p| scene.node(p).is_some_and(|n| n.kind != NodeKind::Page))
        .map(str::to_string)
        .collect();
    drop(scene);
    match parents.first() {
        Some(p) => b.select(vec![p.clone()]),
        None => b.select(Vec::new()),
    }
}

fn select_sibling(b: &Board, dir: i32) {
    let sel = b.selected();
    let scene = b.doc.scene.borrow();
    let Some(first) = sel.first() else {
        let page = b.page.borrow().clone();
        let next = scene.children(&page).first().cloned();
        drop(scene);
        if let Some(n) = next {
            b.select(vec![n]);
        }
        return;
    };
    let Some(parent) = scene.parent(first).map(str::to_string) else {
        return;
    };
    let kids = scene.children(&parent);
    let i = scene.index(first).unwrap_or(0) as i32;
    let n = kids.len() as i32;
    if n == 0 {
        return;
    }
    let next = kids[((i + dir).rem_euclid(n)) as usize].clone();
    drop(scene);
    b.select(vec![next]);
}

impl Board {
    pub(super) fn nudge(&self, dx: f64, dy: f64) {
        let ops = edit::nudge(&self.doc.scene.borrow(), &self.selection.borrow(), dx, dy);
        self.commit(ops);
    }

    pub(super) fn align(&self, how: Align) {
        let placed = self.placed();
        let ops = edit::align(&self.doc.scene.borrow(), &placed, &self.selection.borrow(), how);
        self.commit(ops);
    }

    /// Flip a text style on every selected text: bold, italic or underline.
    pub(super) fn toggle_text(&self, field: &str) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let texts: Vec<&Node> = ids
            .iter()
            .filter_map(|id| scene.node(id))
            .filter(|n| matches!(n.kind, NodeKind::Text | NodeKind::Sticky))
            .collect();
        if texts.is_empty() {
            return;
        }
        let on = match field {
            "font_weight" => texts.iter().all(|n| n.font_weight.unwrap_or(400) >= 600),
            "italic" => texts.iter().all(|n| n.italic == Some(true)),
            _ => texts.iter().all(|n| n.underline == Some(true)),
        };
        let value = match field {
            "font_weight" => if on { Value::Null } else { json!(700) },
            _ => if on { Value::Null } else { json!(true) },
        };
        let ops = texts
            .iter()
            .map(|n| {
                let mut set = Map::new();
                set.insert(field.into(), value.clone());
                NodeOp::Update { id: n.id.clone(), set }
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }
}
