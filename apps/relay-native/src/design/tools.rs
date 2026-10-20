//! The canvas's input: what a press, a drag, a release, the wheel, a pinch and the keyboard
//! do with each tool. A gesture previews on the scene as it goes and sends one edit when it
//! ends, so a drag is one undo step and the engine hears nothing until the pointer lets go.

use super::board::{Board, Gesture, Handle, Tool, HANDLES};
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp, Sizing};
use relay_client::design::edit::{self, Align};
use relay_client::design::geom::simplify;
use relay_client::design::hit;
use relay_client::design::layout;
use relay_client::design::snap::{self, Edges};
use relay_client::design::{shapes, Rect};
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
            b.drag_at.set(Some((sx + dx, sy + dy, dx, dy, mods)));
            autopan(&b);
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
            // Not in the middle of another gesture: its preview would never be sent.
            if !matches!(*b.gesture.borrow(), Gesture::Idle) {
                return;
            }
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
            if matches!(*b.gesture.borrow(), Gesture::Pan { .. }) {
                *b.gesture.borrow_mut() = Gesture::Idle;
                update_cursor(&b, None);
            }
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
        if !scroll_frame(&b, c, dx, dy) {
            scroll_cam(&b, c, dx, dy);
        }
        glib::Propagation::Stop
    });
    canvas.add_controller(scroll);
    canvas.add_controller(zoom_gesture(b));

    // Over the text being typed, the wheel, a pinch and the middle button move the camera as
    // they do over the canvas (TM-77): the editor, a sibling of the canvas in the overlay,
    // would otherwise take them, or let them go nowhere.
    let over_editor = |b: &Board, x: f64, y: f64| {
        let view = b.edit.borrow().as_ref().map(|e| e.view.clone().upcast::<gtk::Widget>());
        view.is_some_and(|v| b.overlay.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| w == v || w.is_ancestor(&v)))
    };
    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(b);
    motion.connect_motion(move |_, x, y| {
        if let Some(b) = weak.upgrade().filter(|b| over_editor(b, x, y)) {
            b.pointer.set((x, y));
        }
    });
    b.overlay.add_controller(motion);
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    let weak = Rc::downgrade(b);
    scroll.connect_scroll(move |c, dx, dy| {
        let Some(b) = weak.upgrade().filter(|b| b.edit.borrow().is_some()) else {
            return glib::Propagation::Proceed;
        };
        let (x, y) = b.pointer.get();
        if !over_editor(&b, x, y) {
            return glib::Propagation::Proceed;
        }
        scroll_cam(&b, c, dx, dy);
        glib::Propagation::Stop
    });
    b.overlay.add_controller(scroll);
    let pinch = zoom_gesture(b);
    pinch.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(b);
    pinch.connect_begin(move |g, _| {
        let over = weak.upgrade().is_some_and(|b| g.bounding_box_center().is_some_and(|(x, y)| over_editor(&b, x, y)));
        if !over {
            g.set_state(gtk::EventSequenceState::Denied);
        }
    });
    b.overlay.add_controller(pinch);
    let pan = gtk::GestureDrag::new();
    pan.set_button(gdk::BUTTON_MIDDLE);
    pan.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(b);
    let from = Rc::new(std::cell::Cell::new(None));
    let f = from.clone();
    pan.connect_drag_begin(move |g, x, y| {
        match weak.upgrade().filter(|b| over_editor(b, x, y)) {
            Some(b) => {
                g.set_state(gtk::EventSequenceState::Claimed);
                f.set(Some(b.cam.get()));
            }
            None => {
                g.set_state(gtk::EventSequenceState::Denied);
                f.set(None);
            }
        }
    });
    let weak = Rc::downgrade(b);
    pan.connect_drag_update(move |_, dx, dy| {
        if let (Some(b), Some(mut cam)) = (weak.upgrade(), from.get()) {
            cam.pan(dx, dy);
            b.set_cam(cam);
        }
    });
    b.overlay.add_controller(pan);

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
        }
    });
    canvas.add_controller(keys);

    // Space released elsewhere (another window, a field) would leave the hand on. The canvas
    // taking the keyboard back ends any typing, so its keys never act on a layer still being
    // typed into (TM-36).
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(b);
    focus.connect_enter(move |_| {
        if let Some(b) = weak.upgrade().filter(|b| b.edit.borrow().is_some()) {
            b.end_edit(true);
        }
    });
    let weak = Rc::downgrade(b);
    focus.connect_leave(move |_| {
        if let Some(b) = weak.upgrade() {
            if b.space.replace(false) {
                update_cursor(&b, None);
            }
        }
    });
    canvas.add_controller(focus);

    // F6 and Shift F6 take the keyboard from the canvas to the layers to the inspector and
    // round, from wherever it is on the board (I-16).
    let panes = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(b);
    panes.connect_key_pressed(move |_, key, _, mods| {
        match weak.upgrade().filter(|_| key == gdk::Key::F6) {
            Some(b) => {
                next_pane(&b, mods.contains(gdk::ModifierType::SHIFT_MASK));
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    });
    b.root.add_controller(panes);

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
}

/// How far a frame the camera moves, in screen pixels, with the pointer at `(x, y)` near an
/// edge of a `w` by `h` canvas: up to 12 within 24 pixels of it, more the closer.
fn edge_pan(x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
    const EDGE: f64 = 24.0;
    let along = |p: f64, size: f64| {
        if p < EDGE {
            (EDGE - p.max(0.0)) / EDGE * 12.0
        } else if p > size - EDGE {
            -((p.min(size) - (size - EDGE)) / EDGE * 12.0)
        } else {
            0.0
        }
    };
    (along(x, w), along(y, h))
}

/// A drag held near the canvas's edge moves the camera a little each frame, and the drag with
/// it, so a layer can be carried past what shows (I-16).
fn autopan(b: &Rc<Board>) {
    let dragging = matches!(
        *b.gesture.borrow(),
        Gesture::Move { moved: true, .. } | Gesture::Resize { .. } | Gesture::Marquee { moved: true, .. } | Gesture::Create { .. } | Gesture::Endpoint { .. }
    );
    let Some((x, y, ..)) = b.drag_at.get() else {
        return;
    };
    let (w, h) = b.size.get();
    if !dragging || edge_pan(x, y, w, h) == (0.0, 0.0) || b.autopan.replace(true) {
        return;
    }
    let weak = Rc::downgrade(b);
    b.canvas.add_tick_callback(move |_, _| {
        let Some(b) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let held = !matches!(*b.gesture.borrow(), Gesture::Idle | Gesture::Pan { .. });
        let (w, h) = b.size.get();
        match b.drag_at.get().filter(|_| held) {
            Some((x, y, dx, dy, mods)) if edge_pan(x, y, w, h) != (0.0, 0.0) => {
                let (vx, vy) = edge_pan(x, y, w, h);
                let mut cam = b.cam.get();
                cam.pan(vx, vy);
                b.set_cam(cam);
                update(&b, x, y, dx, dy, mods);
                glib::ControlFlow::Continue
            }
            _ => {
                b.autopan.set(false);
                glib::ControlFlow::Break
            }
        }
    });
}

/// Give the keyboard to the next pane on show after the one holding it (`back`: the one before):
/// the canvas, the layers, the inspector.
fn next_pane(b: &Board, back: bool) {
    let mut panes: Vec<gtk::Widget> = vec![b.canvas.clone().upcast()];
    if b.layers_panel.reveals_child() {
        panes.push(b.layers_panel.clone().upcast());
    }
    if b.inspector_scroll.is_visible() {
        panes.push(b.inspector_scroll.clone().upcast());
    }
    let focus = b.focus_widget();
    let here = panes.iter().position(|p| focus.as_ref().is_some_and(|f| f == p || f.is_ancestor(p))).unwrap_or(0);
    let n = panes.len();
    let next = &panes[if back { (here + n - 1) % n } else { (here + 1) % n }];
    if next.child_focus(gtk::DirectionType::TabForward) || next.grab_focus() {
        return;
    }
    b.canvas.grab_focus();
}

/// The wheel or touchpad over the canvas: Ctrl (or Cmd) zooms at the pointer, else it pans
/// (Shift turns a wheel's pan sideways).
fn scroll_cam(b: &Board, c: &gtk::EventControllerScroll, dx: f64, dy: f64) {
    let mods = c.current_event_state();
    let wheel = c.unit() == gdk::ScrollUnit::Wheel;
    let (px, py) = b.pointer.get();
    let mut cam = b.cam.get();
    if mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::META_MASK) {
        let factor = if wheel { (-dy * 0.18).exp() } else { (-dy * 0.012).exp() };
        cam.zoom_at(px, py, cam.zoom * factor);
    } else {
        let (mut sx, mut sy) = if wheel { (dx * 48.0, dy * 48.0) } else { (dx, dy) };
        if wheel && mods.contains(gdk::ModifierType::SHIFT_MASK) {
            std::mem::swap(&mut sx, &mut sy);
        }
        cam.pan(-sx, -sy);
    }
    b.set_cam(cam);
}

/// Alt and the wheel over a scrolling frame scroll it, the deepest under the pointer, as the
/// canvas's view only (nothing is stored: feature-layout §2.5). Whether one took the wheel;
/// with none there the wheel pans as before.
fn scroll_frame(b: &Board, c: &gtk::EventControllerScroll, dx: f64, dy: f64) -> bool {
    let mods = c.current_event_state();
    if !alt(mods) || mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::META_MASK) {
        return false;
    }
    let (px, py) = b.pointer.get();
    let p = b.to_page(px, py);
    let placed = b.placed();
    let page = b.page.borrow().clone();
    let found = {
        let scene = b.doc.scene.borrow();
        let chain = hit::chain(&scene, &placed, &page, p.0, p.1, 0.0);
        chain.into_iter().rev().find_map(|id| Some((scene.node(&id)?.scroll?, id)))
    };
    let Some((scroll, id)) = found else {
        return false;
    };
    let (sx, sy) = if c.unit() == gdk::ScrollUnit::Wheel { (dx * 48.0, dy * 48.0) } else { (dx, dy) };
    let zoom = b.cam.get().zoom;
    let (dx, dy) = super::board::scroll_delta(scroll, sx / zoom, sy / zoom);
    b.scroll_by(&id, dx, dy);
    true
}

/// A pinch that zooms the camera about its centre.
fn zoom_gesture(b: &Rc<Board>) -> gtk::GestureZoom {
    let zoom = gtk::GestureZoom::new();
    let start = Rc::new(std::cell::Cell::new(1.0));
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
    zoom
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
pub(super) fn container_at(b: &Board, p: (f64, f64)) -> String {
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
/// line or the page itself. A frame or a group holds on only along its border, so a line drawn
/// across an artboard's empty middle stays free rather than joining the artboard (TM-35).
fn attachable(b: &Board, p: (f64, f64)) -> Option<String> {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let page = b.page.borrow().clone();
    let band = 6.0 / b.cam.get().zoom;
    let chain = hit::chain(&scene, &placed, &page, p.0, p.1, 2.0 / b.cam.get().zoom);
    chain.into_iter().rev().find(|id| {
        let local = scene.parent(id).map_or(p, |parent| shapes::to_local(&scene, &placed, parent, p.0, p.1));
        scene.node(id).zip(placed.rect(id)).is_some_and(|(n, r)| attaches(n, &r, local, band))
    })
}

/// Whether a connector's end at `p` (in the coordinates `n`'s box `r` is in) holds on to `n`:
/// any shape but a line, a path or a page; a frame or group only within `band` of its border.
fn attaches(n: &Node, r: &Rect, p: (f64, f64), band: f64) -> bool {
    match n.kind {
        NodeKind::Line | NodeKind::Path | NodeKind::Page => false,
        NodeKind::Frame | NodeKind::Group => {
            let (cx, cy) = (r.cx(), r.cy());
            let (lx, ly) = shapes::rotate(p.0, p.1, cx, cy, -shapes::rotation(n));
            let inner = Rect::new(r.x + band, r.y + band, (r.w - 2.0 * band).max(0.0), (r.h - 2.0 * band).max(0.0));
            !(inner.w > 0.0 && inner.h > 0.0 && inner.contains(lx, ly))
        }
        _ => true,
    }
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
    scene.node(id).filter(|n| !matches!(n.kind, NodeKind::Page | NodeKind::Line) && !n.is_locked())?;
    // Where it is drawn: a child of a turned frame turns with it (B12).
    let (r, turn) = shapes::visual(&scene, &b.placed(), id)?;
    let r = b.cam.get().rect_to_screen(&r);
    for (cx, cy, ox, oy) in [(r.x, r.y, -1.0, -1.0), (r.right(), r.y, 1.0, -1.0), (r.right(), r.bottom(), 1.0, 1.0), (r.x, r.bottom(), -1.0, 1.0)] {
        let (hx, hy) = shapes::rotate(cx + ox * 10.0, cy + oy * 10.0, r.cx(), r.cy(), turn);
        if (x - hx).abs() <= 9.0 && (y - hy).abs() <= 9.0 {
            return Some(id.clone());
        }
    }
    None
}

fn handle_at(b: &Board, x: f64, y: f64) -> Option<Handle> {
    // A locked layer has no handles (B16), nor a group, whose box is its children's (B90).
    if !b.has_handles() {
        return None;
    }
    if let Some((_, r, turn)) = b.single_turn() {
        let s = b.cam.get().rect_to_screen(&r);
        return HANDLES.into_iter().find(|h| {
            let (hx, hy) = super::board::handle_point(&s, *h);
            let (hx, hy) = shapes::rotate(hx, hy, s.cx(), s.cy(), turn);
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

/// The one selected line, whose ends can be dragged: not a locked one (B16).
fn is_line_selected(b: &Board) -> Option<String> {
    let sel = b.selection.borrow();
    let scene = b.doc.scene.borrow();
    match sel.as_slice() {
        [one] if scene.node(one).is_some_and(|n| super::board::ends_dragged(n) && !n.is_locked()) => Some(one.clone()),
        _ => None,
    }
}

fn begin(b: &Rc<Board>, x: f64, y: f64, mods: gdk::ModifierType) {
    b.canvas.grab_focus();
    if b.edit.borrow().is_some() {
        b.end_edit(true);
    }
    let p = b.to_page(x, y);
    if b.tool.get() == Tool::Vector && !b.space.get() {
        vector_press(b, x, y, p, mods);
        return;
    }
    // A second press close by in time and place is a double click (which only the Move tool
    // reads: a pen's quick second stroke is a stroke).
    let now = glib::monotonic_time();
    let (t, lx, ly) = b.last_press.get();
    let settings = gtk::Settings::default();
    let window = settings.as_ref().map_or(400, |s| s.gtk_double_click_time()) as i64 * 1000;
    let distance = settings.as_ref().map_or(5, |s| s.gtk_double_click_distance()) as f64;
    if b.tool.get() == Tool::Select && now - t < window && (x - lx).abs() <= distance && (y - ly).abs() <= distance {
        b.last_press.set((0, x, y));
        *b.gesture.borrow_mut() = Gesture::Idle;
        double_click(b, x, y, p);
        return;
    }
    b.last_press.set((now, x, y));
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
            if room_for_one(b) {
                let parent = container_at(b, p);
                b.create_text(p, parent);
            }
        }
        Tool::Sticky => create_sticky(b, p),
        tool @ (Tool::Pen | Tool::Highlighter) => {
            let parent = container_at(b, p);
            *b.gesture.borrow_mut() = Gesture::Draw { parent, points: vec![[p.0, p.1]], tool };
        }
        Tool::Hand | Tool::Vector => {}
    }
}

/// A click of the vector pen: the first starts a path, each next adds a point (Shift: at 45°
/// steps), a click on the first point closes it and a click on the last ends it open.
fn vector_press(b: &Rc<Board>, x: f64, y: f64, p: (f64, f64), mods: gdk::ModifierType) {
    *b.gesture.borrow_mut() = Gesture::Idle;
    let cam = b.cam.get();
    let near = |q: &[f64; 2]| {
        let (sx, sy) = cam.to_screen(q[0], q[1]);
        (sx - x).abs() <= 8.0 && (sy - y).abs() <= 8.0
    };
    let ends = b.vector.borrow().as_ref().map(|(_, pts)| (pts.len(), pts.first().copied(), pts.last().copied()));
    match ends {
        None => {
            let parent = container_at(b, p);
            *b.vector.borrow_mut() = Some((parent, vec![[p.0, p.1]]));
        }
        Some((n, Some(first), _)) if n > 2 && near(&first) => finish_vector(b, true),
        Some((n, _, Some(last))) if n > 1 && near(&last) => finish_vector(b, false),
        Some((_, _, last)) => {
            let at = match last {
                Some(l) if mods.contains(gdk::ModifierType::SHIFT_MASK) => constrain45((l[0], l[1]), p),
                _ => p,
            };
            if let Some((_, pts)) = b.vector.borrow_mut().as_mut() {
                if pts.len() < relay_bus::design::MAX_POINTS {
                    pts.push([at.0, at.1]);
                }
            }
        }
    }
    b.canvas.queue_draw();
}

/// End the vector pen's path: a line through its points, closed or open, selected. Fewer than
/// two points is no path.
pub fn finish_vector(b: &Board, closed: bool) {
    let Some((parent, points)) = b.vector.borrow_mut().take() else {
        return;
    };
    b.canvas.queue_draw();
    if points.len() < 2 || b.doc.scene.borrow().node(&parent).is_none() {
        return;
    }
    let min_x = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let max_y = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let origin = b.origin_of(&parent);
    let mut n = Node::new(NodeKind::Line);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Line);
    n.name = Some(if closed { "Shape" } else { "Path" }.into());
    n.parent = Some(parent);
    n.x = (min_x - origin.x).round();
    n.y = (min_y - origin.y).round();
    n.w = (max_x - min_x).max(1.0).round();
    n.h = (max_y - min_y).max(1.0).round();
    // Whole pixels from the box's own rounded corner, so nothing shifts on the way in.
    let (ox, oy) = (origin.x + n.x, origin.y + n.y);
    n.points = Some(points.iter().map(|p| [(p[0] - ox).round(), (p[1] - oy).round()]).collect());
    n.stroke = Some(b.pen_color(Tool::Vector, n.parent.as_deref().unwrap_or_default()));
    n.stroke_width = Some(2.0);
    if closed && points.len() > 2 {
        n.closed = Some(true);
    }
    let id = n.id.clone();
    if b.commit(vec![NodeOp::Add { node: n, index: None }]) {
        b.select(vec![id]);
    }
    if !b.tool_lock.get() && b.tool.get() == Tool::Vector {
        b.set_tool(Tool::Select);
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
            // About where it is drawn; the pointer's angle there turns it by as much (B12).
            let drawn = shapes::visual(&b.doc.scene.borrow(), &placed, &id).map(|(r, _)| r);
            let Some((orig, r)) = b.doc.scene.borrow().node(&id).cloned().zip(drawn) else {
                return;
            };
            let start = (p.1 - r.cy()).atan2(p.0 - r.cx()).to_degrees();
            b.doc.set_hold(true);
            *b.gesture.borrow_mut() = Gesture::Rotate { id, centre: (r.cx(), r.cy()), start, orig, moved: false };
            return;
        }
    }
    // A resize handle of the selection: of its unlocked layers (B16).
    if is_line_selected(b).is_none() {
        if let Some(handle) = handle_at(b, x, y) {
            let placed = b.placed();
            let scene = b.doc.scene.borrow();
            let ids = edit::unlocked(&scene, b.selected());
            let orig: Vec<(String, Node, Rect)> =
                ids.iter().filter_map(|id| Some((id.clone(), scene.node(id)?.clone(), placed.rect(id)?))).collect();
            // One node drawn turned resizes in its parent's frame, about its own turn (B12).
            let turned = b.single_turn().and_then(|(id, _, _)| Some((scene.parent(&id)?.to_string(), shapes::rotation(scene.node(&id)?), placed.rect(&id)?)));
            drop(scene);
            let (bounds, frame, turn) = match turned {
                Some((parent, turn, r)) => (Some(r), Some(parent), turn),
                None => (b.selection_bounds(), None, 0.0),
            };
            if let (Some(bounds), false) = (bounds, orig.is_empty()) {
                // The pinned layers inside each frame resized, which the preview moves as the
                // follow rule will (feature-layout §3.5), as they were.
                let kept = {
                    let scene = b.doc.scene.borrow();
                    orig.iter()
                        .filter(|(_, n, _)| n.kind == NodeKind::Frame)
                        .flat_map(|(id, _, _)| scene.subtree(id))
                        .filter(|d| !orig.iter().any(|(o, _, _)| o == d))
                        .filter_map(|d| scene.node(&d).filter(|n| n.is_pinned()).cloned())
                        .collect()
                };
                b.doc.set_hold(true);
                *b.gesture.borrow_mut() = Gesture::Resize { handle, bounds, orig, kept, moved: false, frame, turn };
                return;
            }
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
    let in_selection = {
        let scene = b.doc.scene.borrow();
        b.selection.borrow().iter().any(|id| super::board::shown_box(&scene, &placed, id).is_some_and(|r| r.contains(p.0, p.1)))
    };
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
    // What snaps is what shows: a turned node's turned box, a line's points (B13).
    let rects = ids.iter().filter_map(|id| shapes::shown_rect(&scene, &placed, id)).collect();
    drop(scene);
    *b.gesture.borrow_mut() = Gesture::Move {
        ids,
        start: p,
        orig,
        rects,
        dup: alt(mods),
        copies: Vec::new(),
        moved: false,
        target: None,
        into: None,
        reorder: None,
        click,
        at: p,
        shown: (0.0, 0.0),
        sources: Vec::new(),
        ghost: None,
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
            let placed = b.placed();
            let origin = placed.rect(id).map(|r| (r.x, r.y)).unwrap_or((orig.x, orig.y));
            // A connector's end lets go of its shape as it is dragged, and takes hold of the
            // one it is dropped on; its other end stays where it is drawn.
            let connector = orig.from.is_some() || orig.to.is_some();
            let pts = if connector {
                let (drawn, _) = relay_client::design::shapes::line_points(&b.doc.scene.borrow(), &placed, orig);
                [drawn.first(), drawn.last()].into_iter().flatten().map(|d| [d.0 - origin.0, d.1 - origin.1]).collect()
            } else {
                orig.points.clone().unwrap_or_default()
            };
            let which = if connector && *which != 0 { 1 } else { *which };
            let other_end = if which == 0 { orig.to.clone() } else { orig.from.clone() };
            let held = if connector { attachable(b, p).filter(|t| *t != *id && Some(t) != other_end.as_ref()) } else { None };
            if shift(mods) {
                let other = if which == 0 { pts.last() } else { pts.first() }.copied().unwrap_or([0.0, 0.0]);
                q = constrain45((origin.0 + other[0], origin.1 + other[1]), p);
            }
            let mut pts = pts;
            if let Some(pt) = pts.get_mut(which) {
                *pt = [q.0 - origin.0, q.1 - origin.1];
            }
            let id = id.clone();
            drop(gesture);
            if let Some(n) = b.doc.scene.borrow_mut().node_mut(&id) {
                n.points = Some(pts);
                if connector {
                    if which == 0 {
                        n.from = held;
                    } else {
                        n.to = held;
                    }
                }
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
        b.doc.set_hold(true);
        let placed = b.placed();
        let (ops, roots) = edit::duplicate(&mut b.doc.scene.borrow_mut(), &placed, &ids, 0.0);
        let shown = b.doc.scene.borrow_mut().apply_all(&ops);
        if shown.is_ok() {
            b.invalidate();
            let placed = b.placed();
            let scene = b.doc.scene.borrow();
            if let Gesture::Move { ids, orig, rects, dup, copies, sources, .. } = &mut *b.gesture.borrow_mut() {
                *sources = std::mem::replace(ids, roots.clone());
                *orig = roots.iter().filter_map(|id| scene.node(id).map(|n| (id.clone(), n.x, n.y))).collect();
                *rects = roots.iter().filter_map(|id| shapes::shown_rect(&scene, &placed, id)).collect();
                *dup = false;
                *copies = ops;
            }
            drop(scene);
            *b.selection.borrow_mut() = roots;
        }
    }
    b.doc.set_hold(true);
    let placed = b.placed();
    let zoom = b.cam.get().zoom;
    let (ids, start, orig, rects, copied, ghosted) = match &mut *b.gesture.borrow_mut() {
        Gesture::Move { ids, start, orig, rects, moved, at, copies, dup, ghost, .. } => {
            *moved = true;
            *at = p;
            (ids.clone(), *start, orig.clone(), rects.clone(), !copies.is_empty() || *dup, ghost.is_some())
        }
        _ => return,
    };
    let scene_ref = b.doc.scene.borrow();
    let parent = ids.first().and_then(|id| scene_ref.parent(id)).map(str::to_string).unwrap_or_default();
    // Laid-out layers stay in their layout until the drop; an absolute one moves as a free one.
    let laid: Vec<&String> = ids.iter().filter(|id| layout::is_laid(&scene_ref, id)).collect();
    let ghost = (!laid.is_empty() && !ghosted).then(|| {
        let snap = gtk::Snapshot::new();
        for id in &laid {
            b.painter.paint(&snap, &scene_ref, &placed, id, None);
        }
        snap.to_node()
    })
    .flatten();
    // In a turned frame the nodes move in its turned coordinates: the pointer is taken there
    // (B12), and they snap there, the guides drawn turned with it.
    let turned = !shapes::turned_ancestors(&scene_ref, &placed, &parent).is_empty() || scene_ref.node(&parent).is_some_and(|n| shapes::rotation(n) != 0.0);
    let local = |q: (f64, f64)| if turned { shapes::to_local(&scene_ref, &placed, &parent, q.0, q.1) } else { q };
    let (from, to) = (local(start), local(p));
    let mut dx = to.0 - from.0;
    let mut dy = to.1 - from.1;
    // Shift keeps to one axis, and snaps along that one only (TM-65).
    let (mut free_x, mut free_y) = (true, true);
    if shift(mods) {
        if dx.abs() > dy.abs() {
            dy = 0.0;
            free_y = false;
        } else {
            dx = 0.0;
            free_x = false;
        }
    }
    let parent_rect = placed.rect(&parent);
    // Children of an auto-layout frame, still inside it: reorder them, as one block (B18), to
    // the index the engine reads, hidden siblings counted (B20).
    let siblings = ids.iter().all(|id| scene_ref.parent(id) == Some(parent.as_str()));
    if laid.len() == ids.len() && siblings && !ids.is_empty() && parent_rect.is_some_and(|r| r.contains(to.0, to.1)) {
        let index = layout::drop_index(&scene_ref, &placed, &parent, &ids, to);
        drop(scene_ref);
        if let Gesture::Move { reorder, target, into, ghost: g, .. } = &mut *b.gesture.borrow_mut() {
            *reorder = Some((parent, index));
            (*target, *into) = (None, None);
            if ghost.is_some() {
                *g = ghost;
            }
        }
        b.guides.borrow_mut().clear();
        b.canvas.queue_draw();
        return;
    }
    // Snap the moving box to its neighbours and its parent, as they show (B13): in the parent's
    // coordinates, which the boxes are in, turned or not.
    let moving = Rect::bounds(rects.iter()).unwrap_or_default().translate(dx, dy);
    let view = b.cam.get().visible(b.size.get().0, b.size.get().1);
    let mut others: Vec<Rect> = scene_ref
        .children(&parent)
        .iter()
        .filter(|c| !ids.contains(c))
        .filter(|c| scene_ref.node(c).is_some_and(|n| !n.is_hidden()))
        .filter_map(|c| shapes::shown_rect(&scene_ref, &placed, c))
        .filter(|r| turned || r.intersects(&view))
        .collect();
    if let Some(pr) = parent_rect.filter(|_| scene_ref.node(&parent).is_some_and(|n| n.kind != NodeKind::Page)) {
        others.push(pr);
    }
    let edges = Edges {
        left: free_x,
        center_x: free_x,
        right: free_x,
        top: free_y,
        center_y: free_y,
        bottom: free_y,
    };
    // Alt held for a duplicate drag still snaps; Alt pressed during a plain drag turns it off
    // (TM-72).
    let (sx, sy, guides) = if alt(mods) && !copied { (0.0, 0.0, Vec::new()) } else { snap::snap(&moving, &others, edges, SNAP / zoom) };
    if free_x {
        dx += sx;
    }
    if free_y {
        dy += sy;
    }
    // Where it would land: the deepest frame under the pointer that is not one of them, and
    // the place there when it lays its children out (its own coordinates, turned or not).
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
    let into = edit::lays_out(&scene_ref, &landing).then(|| {
        let at = shapes::to_local(&scene_ref, &placed, &landing, p.0, p.1);
        layout::drop_index(&scene_ref, &placed, &landing, &ids, at)
    });
    let target = (landing != parent).then_some(landing);
    // The free ones show where they go: a `scale` pin's stored x/y move as far as shown, scaled
    // to the parent's stored size.
    let fit = layout::fit_of(&scene_ref, &placed, &parent);
    let free: Vec<(String, f64, f64)> = orig
        .into_iter()
        .filter(|(id, _, _)| !layout::is_laid(&scene_ref, id))
        .map(|(id, ox, oy)| {
            let (sx, sy) = scene_ref.node(&id).map_or((dx, dy), |n| layout::stored_shift(n, fit, dx, dy));
            (id, ox + sx, oy + sy)
        })
        .collect();
    drop(scene_ref);
    {
        let mut scene = b.doc.scene.borrow_mut();
        for (id, x, y) in &free {
            if let Some(n) = scene.node_mut(id) {
                n.x = x.round();
                n.y = y.round();
            }
        }
    }
    if let Gesture::Move { target: t, into: i, reorder, shown, ghost: g, .. } = &mut *b.gesture.borrow_mut() {
        *t = target;
        *i = into.filter(|_| t.is_some());
        *reorder = None;
        *shown = (dx, dy);
        if ghost.is_some() {
            *g = ghost;
        }
    }
    *b.guides.borrow_mut() = guides;
    *b.guides_in.borrow_mut() = turned.then_some(parent);
    b.invalidate();
}

/// The edits that drop `id` into `target` at `index`: a free layer dragged `shown` in its
/// parent's coordinates (its preview, snapped), a laid-out one `page` on the page (shown only
/// as a ghost). Its centre lands where it was dropped, through the turns of the frames it
/// leaves and enters, and its turn on screen stays: a layer dropped into a frame turned 90°
/// is turned back 90° in it (`edit::reparent` counts in unturned page points).
fn drop_into(scene: &relay_client::design::Scene, placed: &layout::Placed, id: &str, target: &str, index: Option<u32>, page: (f64, f64), shown: (f64, f64)) -> Vec<NodeOp> {
    let (Some(n), Some(r)) = (scene.node(id), placed.rect(id)) else {
        return edit::reparent(scene, placed, id, target, index, page);
    };
    let parent = scene.parent(id).unwrap_or_default();
    let centre = if layout::is_laid(scene, id) {
        let v = shapes::visual(scene, placed, id).map_or(r, |(v, _)| v);
        (v.cx() + page.0, v.cy() + page.1)
    } else {
        shapes::to_page_pt(scene, placed, parent, r.cx() + shown.0, r.cy() + shown.1)
    };
    let (lx, ly) = shapes::to_local(scene, placed, target, centre.0, centre.1);
    let mut ops = edit::reparent(scene, placed, id, target, index, (lx - r.w / 2.0, ly - r.h / 2.0));
    // The turn of a frame and those it is in.
    let turn_of = |f: &str| shapes::turned_ancestors(scene, placed, f).iter().map(|t| t.2).sum::<f64>() + scene.node(f).map_or(0.0, shapes::rotation);
    let change = turn_of(parent) - turn_of(target);
    // A line's own turn is not drawn (B99): its points are what show.
    if change.abs() > 1e-9 && n.kind != NodeKind::Line {
        let turn = ((((shapes::rotation(n) + change + 180.0).rem_euclid(360.0) - 180.0) * 10.0).round()) / 10.0;
        let mut set = Map::new();
        set.insert("rotation".into(), if turn.abs() < 0.05 { Value::Null } else { json!(turn) });
        ops.push(NodeOp::Update { id: id.to_string(), set });
    }
    ops
}

fn resize_update(b: &Rc<Board>, p: (f64, f64), mods: gdk::ModifierType) {
    let (handle, bounds, orig, kept, frame, turn) = match &mut *b.gesture.borrow_mut() {
        Gesture::Resize { handle, bounds, orig, kept, moved, frame, turn } => {
            *moved = true;
            (*handle, *bounds, orig.clone(), kept.clone(), frame.clone(), *turn)
        }
        _ => return,
    };
    // The pinned layers the last update moved go back first: what follows reads them as stored.
    {
        let mut scene = b.doc.scene.borrow_mut();
        for k in &kept {
            if let Some(n) = scene.node_mut(&k.id) {
                n.clone_from(k);
            }
        }
    }
    let zoom = b.cam.get().zoom;
    let placed = b.placed();
    // One node drawn turned resizes in its own frame: the pointer is taken into its parent's
    // coordinates (through the turns of the frames it is in, B12), turned back about its centre
    // by its own turn, the dragged edges move there, and the new box is turned into place again.
    let p = match &frame {
        Some(f) => shapes::to_local(&b.doc.scene.borrow(), &placed, f, p.0, p.1),
        None => p,
    };
    let p = if turn != 0.0 { shapes::rotate(p.0, p.1, bounds.cx(), bounds.cy(), -turn) } else { p };
    let centred = alt(mods);
    let keep_ratio = shift(mods) || (orig.len() == 1 && orig[0].1.kind == NodeKind::Image);
    // Snap the dragged edges to the siblings as they show, those on show only (B13, TM-74).
    let page = b.page.borrow().clone();
    let parent = orig.first().and_then(|(id, _, _)| b.doc.scene.borrow().parent(id).map(str::to_string)).unwrap_or(page);
    let ids: Vec<&String> = orig.iter().map(|(id, _, _)| id).collect();
    let view = b.cam.get().visible(b.size.get().0, b.size.get().1);
    let others: Vec<Rect> = {
        let scene = b.doc.scene.borrow();
        scene
            .children(&parent)
            .iter()
            .filter(|c| !ids.contains(c) && scene.node(c).is_some_and(|n| !n.is_hidden()))
            .filter_map(|c| shapes::shown_rect(&scene, &placed, c))
            .filter(|r| r.intersects(&view))
            .collect()
    };
    let moving = resize_box(&bounds, handle, p, (0.0, 0.0), centred, false);
    let edges = Edges {
        left: handle.left,
        center_x: false,
        right: handle.right,
        top: handle.top,
        center_y: false,
        bottom: handle.bottom,
    };
    let (sx, sy, guides) = if turn != 0.0 || frame.is_some() { (0.0, 0.0, Vec::new()) } else { snap::snap(&moving, &others, edges, SNAP / zoom) };
    let mut new = resize_box(&bounds, handle, p, (sx, sy), centred, keep_ratio);
    if turn != 0.0 {
        let (cx, cy) = shapes::rotate(new.cx(), new.cy(), bounds.cx(), bounds.cy(), turn);
        new = Rect::new(cx - new.w / 2.0, cy - new.h / 2.0, new.w, new.h);
    }
    let sx = new.w / bounds.w.max(1e-9);
    let sy = new.h / bounds.h.max(1e-9);
    let resized: Vec<(String, Node)> = {
        let scene = b.doc.scene.borrow();
        orig.iter()
            .map(|(id, node, r)| {
                let at = Rect::new(new.x + (r.x - bounds.x) * sx, new.y + (r.y - bounds.y) * sy, (r.w * sx).max(1.0), (r.h * sy).max(1.0));
                (id.clone(), resized_node(&scene, &placed, node, r, at, handle, centred, (sx, sy)))
            })
            .collect()
    };
    let mut scene = b.doc.scene.borrow_mut();
    let mut follow = Vec::new();
    for (id, n) in resized {
        let before = orig.iter().find(|(o, _, _)| *o == id).map(|(_, node, _)| (node.w, node.h));
        if let Some(old) = before.filter(|old| n.kind == NodeKind::Frame && *old != (n.w, n.h)) {
            follow.push((id.clone(), old, (n.w, n.h)));
        }
        if let Some(at) = scene.node_mut(&id) {
            *at = n;
        }
    }
    // Their pinned children follow, as the edit's own follow rule will move them.
    let selected: Vec<String> = orig.iter().map(|(id, _, _)| id.clone()).collect();
    preview_followers(&mut scene, follow, &selected);
    drop(scene);
    *b.guides.borrow_mut() = guides;
    b.guides_in.borrow_mut().take();
    b.invalidate();
}

/// Where a resize handle drags `bounds` with the pointer at `p` (in the coordinates `bounds` is
/// in), the dragged edges snapped by `snap`. With Alt (`centred`) the opposite edge moves as far
/// the other way, snap included, so the centre stays. With `keep_ratio` (Shift, or a picture),
/// a corner keeps the box's proportions, about the centre when centred; an edge handle sizes the
/// other side to match, centred on the box's middle that way (TM-66).
fn resize_box(bounds: &Rect, handle: Handle, p: (f64, f64), snap: (f64, f64), centred: bool, keep_ratio: bool) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (bounds.x, bounds.y, bounds.right(), bounds.bottom());
    if handle.left {
        x0 = p.0 + snap.0;
        if centred {
            x1 = bounds.right() + (bounds.x - x0);
        }
    }
    if handle.right {
        x1 = p.0 + snap.0;
        if centred {
            x0 = bounds.x - (x1 - bounds.right());
        }
    }
    if handle.top {
        y0 = p.1 + snap.1;
        if centred {
            y1 = bounds.bottom() + (bounds.y - y0);
        }
    }
    if handle.bottom {
        y1 = p.1 + snap.1;
        if centred {
            y0 = bounds.y - (y1 - bounds.bottom());
        }
    }
    let (across, down) = (handle.left || handle.right, handle.top || handle.bottom);
    if keep_ratio && bounds.w > 0.0 && bounds.h > 0.0 {
        let ratio = bounds.w / bounds.h;
        let (cx, cy) = (bounds.cx(), bounds.cy());
        let (w, h) = ((x1 - x0).abs(), (y1 - y0).abs());
        // A side's sign: which way it was dragged past the other.
        let (sw, sh) = (if x1 < x0 { -1.0 } else { 1.0 }, if y1 < y0 { -1.0 } else { 1.0 });
        let (nw, nh) = match (across, down) {
            (true, true) if w / ratio > h => (w, w / ratio),
            (true, true) => (h * ratio, h),
            (true, false) => (w, w / ratio),
            (false, true) => (h * ratio, h),
            (false, false) => (w, h),
        };
        if nw != w {
            if centred || !across {
                (x0, x1) = (cx - nw / 2.0, cx + nw / 2.0);
            } else if handle.left {
                x0 = x1 - nw * sw;
            } else {
                x1 = x0 + nw * sw;
            }
        }
        if nh != h {
            if centred || !down {
                (y0, y1) = (cy - nh / 2.0, cy + nh / 2.0);
            } else if handle.top {
                y0 = y1 - nh * sh;
            } else {
                y1 = y0 + nh * sh;
            }
        }
    }
    Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0))
}

/// The sizing rules a resize writes: a width dragged while it hugged its words, a text's, or
/// filled its row is fixed, and so is a height that hugged or filled, so the size dragged to
/// takes effect (B19: a filling child kept its share whatever was dragged).
fn resize_rules(node: &Node, across: bool, down: bool) -> (Option<Sizing>, Option<Sizing>) {
    let w = (across && (node.kind == NodeKind::Text || layout::w_sizing(node) != Sizing::Fixed)).then_some(Sizing::Fixed);
    let h = (down && layout::h_sizing(node) != Sizing::Fixed).then_some(Sizing::Fixed);
    (w, h)
}

/// `node`, placed at `r`, as a resize that shows it at `at` (in the same coordinates) stores it:
/// its box through its parent's pins (`layout::unpinned`: a right-pinned child of a filling
/// frame stores where it sits at the frame's stored width), each size dragged held within its
/// min and max with the edge not dragged staying put, and the sizing rules that make it take
/// effect. A laid-out child keeps its `x`/`y`, which its layout ignores. A group, whose box is
/// its children's, only moves (B90); a flat line keeps its flat side (B36); a line's or path's
/// points scale with the selection, `scale`.
#[allow(clippy::too_many_arguments)]
fn resized_node(scene: &relay_client::design::Scene, placed: &layout::Placed, node: &Node, r: &Rect, mut at: Rect, handle: Handle, centred: bool, scale: (f64, f64)) -> Node {
    let mut n = node.clone();
    let parent = scene.parent(&node.id).unwrap_or_default();
    let laid = layout::is_laid(scene, &node.id);
    let fit = if laid { None } else { layout::fit_of(scene, placed, parent) };
    let (across, down) = (handle.left || handle.right, handle.top || handle.bottom);
    if node.kind == NodeKind::Group {
        let (dx, dy) = layout::stored_shift(node, fit, at.x - r.x, at.y - r.y);
        if !laid {
            n.x = (node.x + dx).round();
            n.y = (node.y + dy).round();
        }
        return n;
    }
    let flat = |a: usize| {
        matches!(node.kind, NodeKind::Line | NodeKind::Path) && node.points.as_ref().is_some_and(|pts| pts.iter().all(|q| (q[a] - pts[0][a]).abs() < 1e-9))
    };
    let (across, down) = (across && !flat(0), down && !flat(1));
    // Within its min and max, the side not dragged (its middle, centred) where it was.
    let held = |pos: f64, len: f64, bound: f64, start: bool| -> (f64, f64) {
        if bound == len {
            (pos, len)
        } else if centred {
            (pos + (len - bound) / 2.0, bound)
        } else if start {
            (pos + len - bound, bound)
        } else {
            (pos, bound)
        }
    };
    if across {
        (at.x, at.w) = held(at.x, at.w, node.clamp_w(at.w), handle.left);
    }
    if down {
        (at.y, at.h) = held(at.y, at.h, node.clamp_h(at.h), handle.top);
    }
    let (ox, oy) = placed.origin(parent);
    let stored = layout::unpinned(node, fit, Rect::new(at.x - ox, at.y - oy, at.w, at.h));
    if !laid {
        n.x = stored.x.round();
        n.y = stored.y.round();
    }
    let (w_rule, h_rule) = resize_rules(node, across, down);
    if across {
        n.w = node.clamp_w(stored.w.round().max(1.0));
        n.w_sizing = w_rule.or(n.w_sizing);
    } else {
        n.w = node.w;
    }
    if down {
        n.h = node.clamp_h(stored.h.round().max(1.0));
        n.h_sizing = h_rule.or(n.h_sizing);
    } else {
        n.h = node.h;
    }
    if let (Some(pts), true) = (&node.points, matches!(node.kind, NodeKind::Path | NodeKind::Line)) {
        n.points = Some(pts.iter().map(|q| [q[0] * scale.0, q[1] * scale.1]).collect());
    }
    n
}

/// A frame a resize changes: its id, and its stored size before and after.
type Resized = (String, (f64, f64), (f64, f64));

/// Show the pinned children of each frame `resized` (id, stored size before, after) where the
/// follow rule puts them (`Scene::followers`), level by level as the edit will once sent; the
/// layers of `skip`, which the drag sizes itself, keep what it gives them.
fn preview_followers(scene: &mut relay_client::design::Scene, resized: Vec<Resized>, skip: &[String]) {
    let mut queue: std::collections::VecDeque<(Resized, usize)> = resized.into_iter().map(|r| (r, 0)).collect();
    while let Some(((frame, old, new), level)) = queue.pop_front() {
        if level >= relay_bus::design::MAX_DEPTH {
            continue;
        }
        for (child, set) in scene.followers(&frame, old, new, |id, _| skip.iter().any(|s| s == id)) {
            let Some(next) = scene.node(&child).and_then(|n| n.patched(&set).ok()) else {
                continue;
            };
            let size = scene.node(&child).map(|n| (n.w, n.h)).unwrap_or_default();
            if next.kind == NodeKind::Frame && (next.w, next.h) != size {
                queue.push_back(((child.clone(), size, (next.w, next.h)), level + 1));
            }
            if let Some(n) = scene.node_mut(&child) {
                *n = next;
            }
        }
    }
}

fn finish(b: &Rc<Board>, _x: f64, _y: f64, mods: gdk::ModifierType) {
    b.drag_at.set(None);
    let gesture = std::mem::replace(&mut *b.gesture.borrow_mut(), Gesture::Idle);
    b.guides.borrow_mut().clear();
    b.guides_in.borrow_mut().take();
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
        Gesture::Move { ids, orig, moved, target, reorder, click, at, start, copies, shown, .. } => {
            if !moved {
                if !copies.is_empty() {
                    let gone: Vec<NodeOp> = ids.iter().map(|id| NodeOp::Delete { id: id.clone() }).collect();
                    let _ = b.doc.scene.borrow_mut().apply_all(&gone);
                    b.invalidate();
                }
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
            let mut ops = Vec::new();
            if let Some((parent, index)) = reorder {
                ops.extend(edit::reorder(&scene, &parent, &ids, index));
            } else if let Some(t) = target {
                // Into an auto layout at the place under the pointer (in its own coordinates,
                // turned or not), hidden children counted as the engine counts them (B20), one
                // after another in the order they paint (B18, TM-62).
                let index = edit::lays_out(&scene, &t).then(|| {
                    let at = shapes::to_local(&scene, &placed_before, &t, at.0, at.1);
                    layout::drop_index(&scene, &placed_before, &t, &ids, at)
                });
                for (k, id) in ids.iter().enumerate() {
                    ops.extend(drop_into(&scene, &placed_before, id, &t, index.map(|i| i + k as u32), (at.0 - start.0, at.1 - start.1), shown));
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
            let ops = if copies.is_empty() {
                ops
            } else {
                // The copies were only shown: they go out now, with their move, as one step.
                let gone: Vec<NodeOp> = ids.iter().map(|id| NodeOp::Delete { id: id.clone() }).collect();
                let _ = b.doc.scene.borrow_mut().apply_all(&gone);
                copies.into_iter().chain(ops).collect()
            };
            b.commit(ops);
            b.doc.set_hold(false);
        }
        Gesture::Resize { orig, kept, moved, .. } => {
            if moved {
                let finals: Vec<(String, Node)> = {
                    let scene = b.doc.scene.borrow();
                    orig.iter().filter_map(|(id, _, _)| Some((id.clone(), scene.node(id)?.clone()))).collect()
                };
                // Back as stored, the followers the preview moved too: the edit's follow rule
                // moves them again, here and on the engine alike.
                {
                    let mut scene = b.doc.scene.borrow_mut();
                    for node in orig.iter().map(|(_, node, _)| node).chain(&kept) {
                        if let Some(n) = scene.node_mut(&node.id) {
                            *n = node.clone();
                        }
                    }
                }
                let ops = finals
                    .into_iter()
                    .filter_map(|(id, after)| {
                        let before = orig.iter().find(|(o, _, _)| *o == id)?.1.clone();
                        diff_op(&id, &before, &after)
                    })
                    .collect();
                b.commit(ops);
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
            // Through the canvas's delete: a connector attached to a stroke rubbed out lets go
            // where it is drawn, laid out, not at the stored boxes (B26).
            let ids: Vec<String> = hits.into_iter().map(|(id, _)| id).collect();
            let placed = b.placed();
            let ops = edit::delete(&b.doc.scene.borrow(), &placed, &ids);
            b.commit(ops);
            b.invalidate();
        }
        Gesture::Draw { parent, points, tool } => finish_drawing(b, parent, points, tool),
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
    let origin = b.origin_of(&parent);
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
        // A connector sits on the page, but its ink is for where it starts (TM-42).
        n.stroke = Some(b.ink_for(&container_at(b, a)));
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
            let scene = b.doc.scene.borrow();
            let taken: Vec<String> = scene
                .children(&parent)
                .iter()
                .filter_map(|c| scene.node(c).filter(|n| n.kind == NodeKind::Frame).and_then(|n| n.name.clone()))
                .collect();
            n.name = Some(super::board::free_name("Frame", &taken));
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

fn finish_drawing(b: &Rc<Board>, parent: String, points: Vec<[f64; 2]>, tool: Tool) {
    let zoom = b.cam.get().zoom;
    let highlighter = tool == Tool::Highlighter;
    let simple = simplify(&points, 0.6 / zoom);
    let min_x = simple.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let min_y = simple.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_x = simple.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let max_y = simple.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    if !min_x.is_finite() {
        return;
    }
    let origin = b.origin_of(&parent);
    let mut n = Node::new(NodeKind::Path);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Path);
    let parent_id = parent.clone();
    n.parent = Some(parent);
    n.x = ((min_x - origin.x) * 100.0).round() / 100.0;
    n.y = ((min_y - origin.y) * 100.0).round() / 100.0;
    n.w = (max_x - min_x).max(1.0);
    n.h = (max_y - min_y).max(1.0);
    n.points = Some(simple.iter().map(|p| [((p[0] - min_x) * 10.0).round() / 10.0, ((p[1] - min_y) * 10.0).round() / 10.0]).collect());
    n.stroke = Some(b.pen_color(tool, &parent_id));
    n.stroke_width = Some(if highlighter { 16.0 } else { 3.0 });
    if highlighter {
        n.name = Some("Highlight".into());
    }
    b.commit(vec![NodeOp::Add { node: n, index: None }]);
}

/// A sticky at `p`, typed into at once. It is shown but not sent until the typing ends, and
/// then with its words, as one undo step; left empty, it is not made (E-text-8).
fn create_sticky(b: &Rc<Board>, p: (f64, f64)) {
    if !room_for_one(b) {
        return;
    }
    let parent = container_at(b, p);
    let origin = b.origin_of(&parent);
    let mut n = Node::new(NodeKind::Sticky);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Sticky);
    n.parent = Some(parent);
    n.x = (p.0 - origin.x - 100.0).round();
    n.y = (p.1 - origin.y - 100.0).round();
    n.w = 200.0;
    n.h = 200.0;
    n.fill = Some(STICKY_COLORS[0].1.into());
    n.text = Some(String::new());
    let id = n.id.clone();
    b.doc.scene.borrow_mut().insert_preview(n);
    b.invalidate();
    b.select(vec![id.clone()]);
    b.begin_edit(&id, true);
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

/// Put back what the gesture under way previewed, and end it: Esc, and an undo or redo pressed
/// in the middle of a drag (B11).
pub(super) fn cancel(b: &Board) {
    let gesture = std::mem::replace(&mut *b.gesture.borrow_mut(), Gesture::Idle);
    {
        let mut scene = b.doc.scene.borrow_mut();
        match &gesture {
            // Copies an Alt drag only showed simply go, and what was copied is selected again
            // (E-board-11).
            Gesture::Move { ids, copies, sources, .. } if !copies.is_empty() => {
                let gone: Vec<NodeOp> = ids.iter().map(|id| NodeOp::Delete { id: id.clone() }).collect();
                let _ = scene.apply_all(&gone);
                *b.selection.borrow_mut() = sources.clone();
            }
            Gesture::Move { orig, .. } => {
                for (id, x, y) in orig {
                    if let Some(n) = scene.node_mut(id) {
                        n.x = *x;
                        n.y = *y;
                    }
                }
            }
            Gesture::Resize { orig, kept, .. } => {
                for node in orig.iter().map(|(_, node, _)| node).chain(kept) {
                    if let Some(n) = scene.node_mut(&node.id) {
                        *n = node.clone();
                    }
                }
            }
            Gesture::Endpoint { id, orig, .. } | Gesture::Rotate { id, orig, .. } => {
                if let Some(n) = scene.node_mut(id) {
                    *n = orig.clone();
                }
            }
            // Esc during a marquee keeps the selection it started from.
            Gesture::Marquee { base, .. } => *b.selection.borrow_mut() = base.clone(),
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
    b.guides.borrow_mut().clear();
    b.guides_in.borrow_mut().take();
    b.drag_at.set(None);
    b.doc.set_hold(false);
    b.invalidate();
    update_cursor(b, None);
}

fn double_click(b: &Rc<Board>, x: f64, y: f64, p: (f64, f64)) {
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
        if room_for_one(b) {
            b.create_text(p, page);
        }
        return;
    };
    let kind = b.doc.scene.borrow().node(&deep).map(|n| n.kind);
    match kind {
        Some(NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) => edit_shown(b, &deep, Some((x, y))),
        Some(NodeKind::Image) => super::assets::replace_image(b, &deep),
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
    if b.vector.borrow().is_some() {
        b.canvas.queue_draw();
    }
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
    b.menu_at.set(Some(p));
    let placed = b.placed();
    let page = b.page.borrow().clone();
    let deep = hit::chain(&b.doc.scene.borrow(), &placed, &page, p.0, p.1, 4.0 / b.cam.get().zoom).last().cloned();
    if let Some(d) = &deep {
        if !b.selection.borrow().contains(d) {
            b.select(vec![d.clone()]);
        }
    }
    let item = super::board::menu_item;
    let section = |items: &[gio::MenuItem]| {
        let section = gio::Menu::new();
        for i in items {
            section.append_item(i);
        }
        section
    };
    let menu = gio::Menu::new();
    menu.append_section(None, &section(&[
        item("Cut", "cut", Some("<Control>x")),
        item("Copy", "copy", Some("<Control>c")),
        item("Paste here", "paste-here", Some("<Control>v")),
        item("Duplicate", "duplicate", Some("<Control>d")),
        item("Delete", "delete", Some("Delete")),
    ]));
    if !b.selection.borrow().is_empty() {
        let (lock, hide) = b.flag_captions();
        menu.append_section(None, &section(&[
            item("Bring to front", "front", Some("<Control><Shift>bracketright")),
            item("Bring forward", "forward", Some("<Control>bracketright")),
            item("Send backward", "backward", Some("<Control>bracketleft")),
            item("Send to back", "back", Some("<Control><Shift>bracketleft")),
        ]));
        menu.append_section(None, &section(&[
            item("Group", "group", Some("<Control>g")),
            item("Frame selection", "frame", Some("<Control><Alt>g")),
            item("Ungroup", "ungroup", Some("<Control><Shift>g")),
            item(lock, "lock", Some("<Control><Shift>l")),
            item(hide, "hide", Some("<Control><Shift>h")),
        ]));
        menu.append_section(None, &section(&[
            item("Flip horizontally", "flip-h", Some("<Shift>h")),
            item("Flip vertically", "flip-v", Some("<Shift>v")),
            item("Copy style", "copy-style", Some("<Control><Alt>c")),
            item("Paste style", "paste-style", Some("<Control><Alt>v")),
        ]));
        menu.append_section(None, &section(&[item("Ask the agent about this…", "ask", Some("<Control>Return"))]));
        menu.append_section(None, &super::board::export_menu());
    } else {
        menu.append_section(None, &section(&[
            item("Select all", "select-all", Some("<Control>a")),
            item("Zoom to fit", "zoom-fit", Some("<Shift>1")),
            item("Add a page", "add-page", None),
        ]));
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
    // Keys typed into a popover of the canvas's own (a frame's rename) are its own (E-board-19);
    // while a text is typed, every key but Esc is the editor's (TM-36).
    if b.focus_widget().is_some_and(|f| f.ancestor(gtk::Popover::static_type()).is_some()) {
        return glib::Propagation::Proceed;
    }
    if b.edit.borrow().is_some() && key != Key::Escape {
        return glib::Propagation::Proceed;
    }
    if key == Key::space && !ctrl {
        if !b.space.replace(true) {
            update_cursor(b, Some("grab"));
        }
        return stop;
    }
    if ctrl {
        match key.to_lower() {
            Key::c if alt => b.copy_style(),
            Key::v if alt => b.paste_style(),
            Key::z if shift => b.redo(),
            Key::z => b.undo(),
            Key::y => b.redo(),
            Key::c => b.copy(),
            Key::x => b.cut(),
            Key::v => b.paste(),
            Key::d => b.duplicate(),
            Key::a => b.select_all(),
            Key::g if shift => b.ungroup(),
            Key::g if alt => b.group(NodeKind::Frame),
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
            Key::Return | Key::KP_Enter => b.ask_agent(b.ask.upcast_ref()),
            // Nothing to save: kept here rather than reaching the Files editor (TM-70).
            Key::s => b.toast("Designs save as you go"),
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
    // The vector pen's path being placed takes Enter, Esc and Backspace.
    if b.vector.borrow().is_some() && !ctrl {
        match key {
            Key::Return | Key::KP_Enter | Key::Escape => {
                finish_vector(b, false);
                return stop;
            }
            Key::BackSpace | Key::Delete => {
                let empty = b.vector.borrow_mut().as_mut().is_some_and(|(_, pts)| {
                    pts.pop();
                    pts.is_empty()
                });
                if empty {
                    b.vector.borrow_mut().take();
                }
                b.canvas.queue_draw();
                return stop;
            }
            _ => {}
        }
    }
    if shift && !ctrl && !alt {
        match key.to_lower() {
            Key::h => {
                b.flip(true);
                return stop;
            }
            Key::v => {
                b.flip(false);
                return stop;
            }
            _ => {}
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
                    Some(NodeKind::Text | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon) => edit_shown(b, one, None),
                    // Into it: what shows and is not locked there, as Select all takes (TM-44).
                    Some(k) if k.holds_children() && !hidden_or_locked(b, one).1 => {
                        let kids: Vec<String> = {
                            let scene = b.doc.scene.borrow();
                            scene.children(one).iter().filter(|c| scene.node(c).is_some_and(|n| !n.is_hidden() && !n.is_locked())).cloned().collect()
                        };
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

/// Tab and Shift Tab: the next or previous layer among the selection's siblings that shows and
/// is not locked; with nothing selected, the page's first such layer (TM-44).
fn select_sibling(b: &Board, dir: i32) {
    let sel = b.selected();
    let scene = b.doc.scene.borrow();
    let open = |id: &String| scene.node(id).is_some_and(|n| !n.is_hidden() && !n.is_locked());
    let Some(first) = sel.first() else {
        let page = b.page.borrow().clone();
        let next = scene.children(&page).iter().find(|c| open(c)).cloned();
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
    let next = (1..=n).map(|k| &kids[((i + dir * k).rem_euclid(n.max(1))) as usize]).find(|c| open(c)).cloned();
    drop(scene);
    if let Some(next) = next {
        b.select(vec![next]);
    }
}

/// Whether `id`, or what holds it, is hidden, and whether it is locked.
fn hidden_or_locked(b: &Board, id: &str) -> (bool, bool) {
    let scene = b.doc.scene.borrow();
    let chain: Vec<&Node> = std::iter::once(id).chain(scene.ancestors(id)).filter_map(|a| scene.node(a)).collect();
    (chain.iter().any(|n| n.is_hidden()), chain.iter().any(|n| n.is_locked()))
}

/// Type into `id`, unless it, or what holds it, is hidden or locked, which says what to do
/// first rather than open an editor off the canvas (E-text-11). A double click at screen point
/// `at` puts the caret there, on the word under it (E-text-9); Enter selects every word.
fn edit_shown(b: &Rc<Board>, id: &str, at: Option<(f64, f64)>) {
    match hidden_or_locked(b, id) {
        (true, _) => b.toast("Show it first (Ctrl+Shift+H)"),
        (_, true) => b.toast("Unlock it first (Ctrl+Shift+L)"),
        _ => {
            b.select(vec![id.to_string()]);
            b.begin_edit(id, false);
            if let Some((x, y)) = at {
                caret_at(b, x, y);
            }
        }
    }
}

/// Once the editor is in place, its caret at canvas point `(x, y)`, the word there selected.
fn caret_at(b: &Rc<Board>, x: f64, y: f64) {
    let weak = Rc::downgrade(b);
    glib::idle_add_local_once(move || {
        let Some(b) = weak.upgrade() else {
            return;
        };
        let Some(view) = b.edit.borrow().as_ref().map(|e| e.view.clone()) else {
            return;
        };
        let Some(at) = b.canvas.compute_point(&view, &gtk::graphene::Point::new(x as f32, y as f32)) else {
            return;
        };
        let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, at.x() as i32, at.y() as i32);
        let Some(iter) = view.iter_at_location(bx, by) else {
            return;
        };
        let buffer = view.buffer();
        if iter.inside_word() {
            let (mut start, mut end) = (iter, iter);
            if !start.starts_word() {
                start.backward_word_start();
            }
            if !end.ends_word() {
                end.forward_word_end();
            }
            buffer.select_range(&start, &end);
        } else {
            buffer.place_cursor(&iter);
        }
    });
}

/// Whether the design has room for one more layer; says so when it has none (I-14).
fn room_for_one(b: &Board) -> bool {
    let room = b.doc.scene.borrow().len() < relay_bus::design::MAX_NODES;
    if !room {
        b.toast("This design has reached its 5,000-layer limit");
    }
    room
}

impl Board {
    pub(super) fn nudge(&self, dx: f64, dy: f64) {
        let placed = self.placed();
        let ops = edit::nudge(&self.doc.scene.borrow(), &placed, &self.selection.borrow(), dx, dy);
        self.commit(ops);
    }

    pub(super) fn align(&self, how: Align) {
        let placed = self.placed();
        let ops = edit::align(&self.doc.scene.borrow(), &placed, &self.selection.borrow(), how);
        self.commit(ops);
    }

    /// Flip a text style on every selected text, and every shape's label: bold, italic or
    /// underline (E-board-12: a shape with words takes them as a text does).
    pub(super) fn toggle_text(&self, field: &str) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let editing = self.edit.borrow().as_ref().map(|e| e.id.clone());
        let texts: Vec<&Node> = ids
            .iter()
            .filter_map(|id| scene.node(id))
            .filter(|n| relay_bus::design::takes(n.kind, field))
            .filter(|n| {
                matches!(n.kind, NodeKind::Text | NodeKind::Sticky)
                    || n.text.as_deref().is_some_and(relay_client::design::names::visible)
                    || editing.as_deref() == Some(n.id.as_str())
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::design::{Layout, PinX};
    use relay_client::design::layout::{place, Measure};
    use relay_client::design::Scene;

    /// No text here: nothing to measure.
    struct NoWords;

    impl Measure for NoWords {
        fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
            (0.0, 0.0)
        }
    }

    fn node(id: &str, parent: Option<&str>, kind: NodeKind, r: [f64; 4]) -> Node {
        let mut n = Node::new(kind);
        (n.id, n.parent, n.x, n.y, n.w, n.h) = (id.into(), parent.map(Into::into), r[0], r[1], r[2], r[3]);
        n
    }

    fn page(nodes: Vec<Node>) -> Scene {
        Scene::new(std::iter::once(node("p1", None, NodeKind::Page, [0.0; 4])).chain(nodes).collect())
    }

    const RIGHT: Handle = Handle { left: false, right: true, top: false, bottom: false };
    const LEFT: Handle = Handle { left: true, right: false, top: false, bottom: false };
    const BOTTOM_RIGHT: Handle = Handle { left: false, right: true, top: false, bottom: true };

    /// `id` resized so its placed box shows `at`, sent and laid out again: what it stores and
    /// where it is drawn.
    fn resize_to(scene: &mut Scene, id: &str, at: Rect, handle: Handle) -> (Node, Rect) {
        let placed = place(scene, "p1", &NoWords);
        let before = scene.node(id).unwrap().clone();
        let r = placed.rect(id).unwrap();
        let after = resized_node(scene, &placed, &before, &r, at, handle, false, (1.0, 1.0));
        if let Some(op) = diff_op(id, &before, &after) {
            scene.apply_all(&[op]).unwrap();
        }
        (scene.node(id).unwrap().clone(), place(scene, "p1", &NoWords).rect(id).unwrap())
    }

    #[test]
    fn a_centred_resize_keeps_its_centre_and_a_ratio_its_shape() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(resize_box(&b, BOTTOM_RIGHT, (150.0, 110.0), (0.0, 0.0), false, false), Rect::new(0.0, 0.0, 150.0, 110.0));
        // TM-66: Alt+Shift on a corner by (+50, +10) keeps the centre at 50.
        let r = resize_box(&b, BOTTOM_RIGHT, (150.0, 110.0), (0.0, 0.0), true, true);
        assert_eq!((r.cx(), r.cy(), r.w, r.h), (50.0, 50.0, 200.0, 200.0));
        // Shift on a corner alone grows from the opposite corner.
        assert_eq!(resize_box(&b, BOTTOM_RIGHT, (150.0, 110.0), (0.0, 0.0), false, true), Rect::new(0.0, 0.0, 150.0, 150.0));
        // An edge with the ratio kept: the other side follows, about the box's middle.
        let r = resize_box(&b, RIGHT, (200.0, 30.0), (0.0, 0.0), false, true);
        assert_eq!((r.x, r.w, r.h, r.cy()), (0.0, 200.0, 200.0, 50.0));
        // Alt with a snap moves both edges by it, the centre staying.
        let r = resize_box(&b, LEFT, (-20.0, 0.0), (3.0, 0.0), true, false);
        assert_eq!((r.x, r.right(), r.cx()), (-17.0, 117.0, 50.0));
        // Past the opposite edge it turns over rather than going negative.
        let r = resize_box(&b, RIGHT, (-40.0, 0.0), (0.0, 0.0), false, false);
        assert_eq!((r.x, r.w), (-40.0, 40.0));
    }

    #[test]
    fn a_filling_child_takes_the_size_it_is_dragged_to() {
        // B19: a row 300 wide holding a filling A and a fixed 100 B; A is drawn 200 wide.
        let mut row = node("row", Some("p1"), NodeKind::Frame, [0.0, 0.0, 300.0, 100.0]);
        row.layout = Some(Layout::Row);
        row.gap = Some(0.0);
        let mut a = node("a", Some("row"), NodeKind::Rect, [0.0, 0.0, 10.0, 40.0]);
        a.w_sizing = Some(Sizing::Fill);
        let mut scene = page(vec![row, a, node("b", Some("row"), NodeKind::Rect, [0.0, 0.0, 100.0, 40.0])]);
        assert_eq!(place(&scene, "p1", &NoWords).rect("a").unwrap().w, 200.0);
        let (a, shown) = resize_to(&mut scene, "a", Rect::new(0.0, 0.0, 50.0, 40.0), RIGHT);
        assert_eq!((a.w, a.w_sizing, shown.w), (50.0, Some(Sizing::Fixed), 50.0));
        assert_eq!(a.x, 0.0, "a laid-out child keeps the x its layout ignores");
        // The same down a column that fills its height.
        let mut col = node("col", Some("p1"), NodeKind::Frame, [400.0, 0.0, 100.0, 300.0]);
        col.layout = Some(Layout::Column);
        let mut c = node("c", Some("col"), NodeKind::Rect, [0.0, 0.0, 40.0, 10.0]);
        c.h_sizing = Some(Sizing::Fill);
        let mut scene = page(vec![col, c]);
        let (c, shown) = resize_to(&mut scene, "c", Rect::new(400.0, 0.0, 40.0, 70.0), Handle { left: false, right: false, top: false, bottom: true });
        assert_eq!((c.h, c.h_sizing, shown.h), (70.0, Some(Sizing::Fixed), 70.0));
        assert_eq!(resize_rules(&node("t", None, NodeKind::Text, [0.0, 0.0, 80.0, 20.0]), true, false).0, Some(Sizing::Fixed));
        assert_eq!(resize_rules(&node("r", None, NodeKind::Rect, [0.0, 0.0, 80.0, 20.0]), true, true), (None, None));
    }

    /// A row 400 wide whose one column fills it while storing 200: the column's pins act.
    fn filling_column(child: Node) -> Scene {
        let mut row = node("row", Some("p1"), NodeKind::Frame, [0.0, 0.0, 400.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut col = node("col", Some("row"), NodeKind::Frame, [0.0, 0.0, 200.0, 100.0]);
        col.w_sizing = Some(Sizing::Fill);
        page(vec![row, col, child])
    }

    #[test]
    fn a_pinned_child_resizes_where_it_shows_and_stores_its_place_unpinned() {
        let mut key = node("key", Some("col"), NodeKind::Rect, [150.0, 10.0, 40.0, 20.0]);
        key.pin_x = Some(PinX::Right);
        let mut scene = filling_column(key);
        assert_eq!(place(&scene, "p1", &NoWords).rect("key").unwrap().x, 350.0);
        let (key, shown) = resize_to(&mut scene, "key", Rect::new(350.0, 10.0, 60.0, 20.0), RIGHT);
        assert_eq!((key.x, key.w), (150.0, 60.0), "stored against the column's stored width");
        assert_eq!((shown.x, shown.w), (350.0, 60.0), "drawn where it was dragged");
        // Stretched left and right, it stores the width less what the column adds.
        let mut bar = node("bar", Some("col"), NodeKind::Rect, [10.0, 10.0, 40.0, 20.0]);
        bar.pin_x = Some(PinX::LeftRight);
        let mut scene = filling_column(bar);
        assert_eq!(place(&scene, "p1", &NoWords).rect("bar").unwrap().w, 240.0);
        let (bar, shown) = resize_to(&mut scene, "bar", Rect::new(10.0, 10.0, 260.0, 20.0), RIGHT);
        assert_eq!((bar.w, shown.w), (60.0, 260.0));
    }

    #[test]
    fn a_resize_stays_within_min_and_max_with_the_far_edge_still() {
        let mut r = node("r", Some("p1"), NodeKind::Rect, [100.0, 0.0, 100.0, 40.0]);
        r.max_w = Some(120.0);
        r.min_h = Some(30.0);
        let mut scene = page(vec![r]);
        let (r, _) = resize_to(&mut scene, "r", Rect::new(0.0, 0.0, 200.0, 40.0), LEFT);
        assert_eq!((r.x, r.w), (80.0, 120.0), "its right edge at 200 still");
        let (r, _) = resize_to(&mut scene, "r", Rect::new(80.0, 0.0, 120.0, 5.0), Handle { left: false, right: false, top: false, bottom: true });
        assert_eq!((r.y, r.h), (0.0, 30.0));
    }

    #[test]
    fn a_frames_pinned_children_follow_the_preview_as_the_edit_moves_them() {
        let f = node("f", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 100.0]);
        let mut key = node("key", Some("f"), NodeKind::Rect, [150.0, 0.0, 40.0, 20.0]);
        key.pin_x = Some(PinX::Right);
        let mut g = node("g", Some("f"), NodeKind::Frame, [10.0, 40.0, 180.0, 40.0]);
        g.pin_x = Some(PinX::LeftRight);
        let mut dot = node("dot", Some("g"), NodeKind::Rect, [100.0, 0.0, 20.0, 20.0]);
        dot.pin_x = Some(PinX::Right);
        let scene = page(vec![f, key, g, dot]);
        let mut preview = scene.clone();
        if let Some(f) = preview.node_mut("f") {
            f.w = 300.0;
        }
        preview_followers(&mut preview, vec![("f".into(), (200.0, 100.0), (300.0, 100.0))], &["f".into()]);
        let at = |s: &Scene, id: &str| s.node(id).map(|n| (n.x, n.w)).unwrap();
        assert_eq!(at(&preview, "key"), (250.0, 40.0));
        assert_eq!(at(&preview, "g"), (10.0, 280.0));
        assert_eq!(at(&preview, "dot"), (200.0, 20.0), "a level down, against g's new width");
        // The edit itself moves them the same way.
        let mut sent = scene.clone();
        let mut set = Map::new();
        set.insert("w".into(), json!(300));
        sent.apply_all(&[NodeOp::Update { id: "f".into(), set }]).unwrap();
        for id in ["key", "g", "dot"] {
            assert_eq!(at(&sent, id), at(&preview, id), "{id}");
        }
    }

    #[test]
    fn several_dropped_into_a_layout_keep_their_order() {
        // TM-62: three page layers, in paint order, dropped at the row's start.
        let mut row = node("row", Some("p1"), NodeKind::Frame, [0.0, 0.0, 400.0, 100.0]);
        row.layout = Some(Layout::Row);
        let mut scene = page(vec![row, node("old", Some("row"), NodeKind::Rect, [0.0, 0.0, 40.0, 40.0])]
            .into_iter()
            .chain(["a", "b", "c"].map(|id| node(id, Some("p1"), NodeKind::Rect, [500.0, 0.0, 20.0, 20.0])))
            .collect());
        let placed = place(&scene, "p1", &NoWords);
        let ops: Vec<NodeOp> = ["a", "b", "c"].iter().enumerate().flat_map(|(k, id)| drop_into(&scene, &placed, id, "row", Some(k as u32), (-480.0, 0.0), (-480.0, 0.0))).collect();
        scene.apply_all(&ops).unwrap();
        assert_eq!(scene.children("row"), ["a", "b", "c", "old"]);
    }

    #[test]
    fn a_layer_dropped_into_a_turned_frame_lands_where_it_was_dropped_and_keeps_its_turn() {
        let mut f = node("f", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 100.0]);
        f.rotation = Some(90.0);
        let mut scene = page(vec![f, node("r", Some("p1"), NodeKind::Rect, [300.0, 40.0, 20.0, 20.0])]);
        let placed = place(&scene, "p1", &NoWords);
        // Dragged 150 left, its centre over the frame at (160, 50) on the page.
        let ops = drop_into(&scene, &placed, "r", "f", None, (-150.0, 0.0), (-150.0, 0.0));
        scene.apply_all(&ops).unwrap();
        let placed = place(&scene, "p1", &NoWords);
        let (shown, turn) = shapes::visual(&scene, &placed, "r").unwrap();
        assert!((shown.cx() - 160.0).abs() < 0.6 && (shown.cy() - 50.0).abs() < 0.6, "{shown:?}");
        assert!(turn.rem_euclid(360.0).abs() < 0.1, "upright on screen: {turn}");
        assert_eq!(scene.node("r").unwrap().rotation, Some(-90.0));
        // Unturned frames: it lands at its box moved by the drag, as before.
        let mut scene = page(vec![node("g", Some("p1"), NodeKind::Frame, [0.0, 0.0, 200.0, 100.0]), node("q", Some("p1"), NodeKind::Rect, [300.0, 40.0, 20.0, 20.0])]);
        let placed = place(&scene, "p1", &NoWords);
        scene.apply_all(&drop_into(&scene, &placed, "q", "g", None, (0.0, 0.0), (-250.0, 10.0))).unwrap();
        let q = scene.node("q").unwrap();
        assert_eq!((q.x, q.y, q.rotation), (50.0, 50.0, None));
    }

    #[test]
    fn a_group_has_no_handles_and_only_moves_with_the_selection() {
        let g = node("g", Some("p1"), NodeKind::Group, [10.0, 10.0, 0.0, 0.0]);
        let scene = page(vec![g, node("in", Some("g"), NodeKind::Rect, [0.0, 0.0, 50.0, 50.0]), node("r", Some("p1"), NodeKind::Rect, [100.0, 0.0, 50.0, 50.0])]);
        assert!(!super::super::board::handled(&scene, &["g".into()]), "a group alone: B90");
        assert!(super::super::board::handled(&scene, &["g".into(), "r".into()]));
        let placed = place(&scene, "p1", &NoWords);
        let g = scene.node("g").unwrap();
        let r = placed.rect("g").unwrap();
        let moved = resized_node(&scene, &placed, g, &r, Rect::new(r.x + 5.0, r.y, r.w * 2.0, r.h), RIGHT, false, (2.0, 1.0));
        assert_eq!((moved.x, moved.y, moved.w, moved.h), (15.0, 10.0, 0.0, 0.0), "its size stays its children's");
    }

    #[test]
    fn a_drag_near_an_edge_pans_toward_it() {
        assert_eq!(edge_pan(400.0, 300.0, 800.0, 600.0), (0.0, 0.0));
        let (vx, vy) = edge_pan(2.0, 300.0, 800.0, 600.0);
        assert!(vx > 10.0 && vy == 0.0, "left edge: the page comes in from the left");
        let (vx, vy) = edge_pan(400.0, 590.0, 800.0, 600.0);
        assert!(vx == 0.0 && (-12.0..0.0).contains(&vy));
        assert_eq!(edge_pan(-50.0, 300.0, 800.0, 600.0).0, 12.0, "past the edge: as fast as it goes");
    }

    #[test]
    fn a_line_over_an_artboards_middle_stays_free() {
        let mut frame = Node::new(NodeKind::Frame);
        frame.id = "f".into();
        let r = Rect::new(0.0, 0.0, 390.0, 844.0);
        assert!(!attaches(&frame, &r, (200.0, 400.0), 6.0), "its empty middle");
        assert!(attaches(&frame, &r, (2.0, 400.0), 6.0), "its border");
        assert!(attaches(&frame, &r, (200.0, 841.0), 6.0), "its bottom edge");
        let mut rect = Node::new(NodeKind::Rect);
        rect.id = "r".into();
        assert!(attaches(&rect, &Rect::new(0.0, 0.0, 100.0, 40.0), (50.0, 20.0), 6.0), "a shape anywhere");
        assert!(!attaches(&Node::new(NodeKind::Line), &r, (2.0, 400.0), 6.0));
    }
}
