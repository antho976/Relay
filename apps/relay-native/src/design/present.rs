//! Presenting: the page's top-level frames one at a time, filling a window of their own, and
//! clicked through as a prototype. A node with `goto` takes you to that frame with its
//! transition; clicking where nothing links flashes what does. The arrows step through the
//! frames in order, F fills the screen, Escape leaves. It draws from the board's own scene, so
//! a change made while presenting (by you or an agent) shows at once.

use super::board::{Board, SELECT};
use super::canvas::Canvas;
use super::paint::{grect, rgba, Painter};
use crate::app::label;
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::design::{NodeKind, Transition};
use relay_client::design::{hit, Rect};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

const SLIDE_US: i64 = 320_000;

thread_local! {
    /// The presentation windows open, for the smoke harness to capture and close.
    static SHOWS: RefCell<Vec<gtk::Window>> = const { RefCell::new(Vec::new()) };
}

/// The smoke harness: save what each presentation shows as a PNG at `path`, then close them.
pub fn capture_and_close(path: Option<&str>) {
    let windows: Vec<gtk::Window> = SHOWS.with(|s| s.borrow_mut().drain(..).collect());
    for w in windows {
        if let Some(path) = path {
            let paintable = gtk::WidgetPaintable::new(w.child().as_ref());
            let (pw, ph) = (w.width() as f64, w.height() as f64);
            let snap = gtk::Snapshot::new();
            paintable.snapshot(snap.upcast_ref::<gdk::Snapshot>(), pw, ph);
            if let (Some(node), Some(renderer)) = (snap.to_node(), w.native().and_then(|n| n.renderer())) {
                let texture = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, pw as f32, ph as f32)));
                let saved = texture.save_to_png(path);
                println!("DESIGN_PRESENT_SHOT {path} {}", saved.is_ok());
            }
        }
        w.close();
    }
}

struct Show {
    board: Weak<Board>,
    canvas: Canvas,
    painter: Rc<Painter>,
    frames: Vec<String>,
    current: RefCell<String>,
    /// The frame left, how, and when (µs).
    leaving: RefCell<Option<(String, Transition, i64)>>,
    /// Hotspots flashed after a click that hit none, until (µs).
    flash: Cell<i64>,
    /// Drawing every frame until then (µs), while a transition or a flash runs.
    until: Rc<Cell<i64>>,
    ticking: Rc<Cell<bool>>,
}

impl Board {
    /// Present the page, from its start frame (or the selected frame, or the first).
    pub(super) fn present(&self) {
        self.end_edit(true);
        let page = self.page.borrow().clone();
        let (frames, start) = {
            let scene = self.doc.scene.borrow();
            let frames: Vec<String> = scene
                .children(&page)
                .iter()
                .filter(|id| scene.node(id).is_some_and(|n| n.kind == NodeKind::Frame && !n.is_hidden()))
                .cloned()
                .collect();
            let selected = self.selected().into_iter().find_map(|id| {
                std::iter::once(id.clone()).chain(scene.ancestors(&id).into_iter().map(str::to_string)).find(|a| frames.contains(a))
            });
            let flagged = frames.iter().find(|id| scene.node(id).is_some_and(|n| n.start == Some(true))).cloned();
            let start = selected.or(flagged).or_else(|| frames.first().cloned());
            (frames, start)
        };
        let Some(start) = start else {
            self.toast("Add a frame to present: presenting shows the page's frames as screens");
            return;
        };
        let window = gtk::Window::builder().title(format!("{} — Presenting", self.doc.title())).default_width(1280).default_height(820).build();
        if let Some(app) = self.root.root().and_downcast::<gtk::Window>().and_then(|w| w.application()) {
            window.set_application(Some(&app));
        }
        window.add_css_class("design-present");
        let overlay = gtk::Overlay::new();
        let canvas = Canvas::new();
        canvas.set_widget_name("design-present-canvas");
        overlay.set_child(Some(&canvas));
        let hint = label("← → to move between screens · F to fill the screen · Esc to leave", "design-present-hint");
        hint.set_halign(gtk::Align::Center);
        hint.set_valign(gtk::Align::End);
        hint.set_margin_bottom(14);
        hint.set_can_target(false);
        overlay.add_overlay(&hint);
        window.set_child(Some(&overlay));
        let painter = Painter::new(&canvas);
        *painter.assets_dir.borrow_mut() = self.doc.assets_dir.borrow().clone();
        let show = Rc::new(Show {
            board: Rc::downgrade(&self.me()),
            canvas: canvas.clone(),
            painter,
            frames,
            current: RefCell::new(start),
            leaving: RefCell::default(),
            flash: Cell::new(0),
            until: Rc::default(),
            ticking: Rc::default(),
        });
        let weak = Rc::downgrade(&show);
        show.painter.on_loaded(move || {
            if let Some(s) = weak.upgrade() {
                s.canvas.queue_draw();
            }
        });
        let weak = Rc::downgrade(&show);
        canvas.set_draw(move |snap, w, h| {
            if let Some(s) = weak.upgrade() {
                s.draw(snap, w, h);
            }
        });
        let click = gtk::GestureClick::new();
        let weak = Rc::downgrade(&show);
        click.connect_released(move |_, _, x, y| {
            if let Some(s) = weak.upgrade() {
                s.click(x, y);
            }
        });
        canvas.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&show);
        let win = window.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            let (Some(s), Some(win)) = (weak.upgrade(), win.upgrade()) else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Escape => win.close(),
                gdk::Key::Right | gdk::Key::space | gdk::Key::Page_Down | gdk::Key::Down => s.step(1),
                gdk::Key::Left | gdk::Key::BackSpace | gdk::Key::Page_Up | gdk::Key::Up => s.step(-1),
                gdk::Key::f | gdk::Key::F | gdk::Key::F11 => {
                    if win.is_fullscreen() {
                        win.unfullscreen();
                    } else {
                        win.fullscreen();
                    }
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        canvas.add_controller(keys);
        // The board redraws the show when the design changes.
        let weak = Rc::downgrade(&show);
        self.doc.listen(move |_| {
            if let Some(s) = weak.upgrade() {
                s.canvas.queue_draw();
            }
        });
        let keep = RefCell::new(Some(show));
        window.connect_close_request(move |_| {
            keep.borrow_mut().take();
            glib::Propagation::Proceed
        });
        SHOWS.with(|s| s.borrow_mut().push(window.clone()));
        window.present();
        canvas.grab_focus();
    }
}

impl Show {
    /// The frame `id` fitted in the window: its page box, the scale, and the screen offset.
    fn fit(&self, b: &Board, id: &str, w: f64, h: f64) -> Option<(Rect, f64, f64, f64)> {
        let r = b.placed().rect(id)?;
        let scale = (w / r.w.max(1.0)).min(h / r.h.max(1.0));
        let scale = if scale > 1.0 { scale.min(4.0) } else { scale };
        Some((r, scale, (w - r.w * scale) / 2.0, (h - r.h * scale) / 2.0))
    }

    fn draw(&self, snap: &gtk::Snapshot, w: f64, h: f64) {
        let Some(b) = self.board.upgrade() else { return };
        snap.append_color(&gdk::RGBA::new(0.07, 0.07, 0.07, 1.0), &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
        let now = self.canvas.frame_clock().map(|c| c.frame_time()).unwrap_or_else(glib::monotonic_time);
        let current = self.current.borrow().clone();
        let leaving = self.leaving.borrow().clone();
        let t = leaving.as_ref().map(|(_, _, at)| ((now - at) as f64 / SLIDE_US as f64).clamp(0.0, 1.0));
        let ease = t.map(|t| 1.0 - (1.0 - t).powi(3));
        if let (Some((from, how, _)), Some(e)) = (&leaving, ease) {
            if e < 1.0 {
                let (dx, dy) = slide(*how, w, h);
                match how {
                    Transition::Dissolve => self.paint(&b, snap, from, w, h, (0.0, 0.0), 1.0),
                    Transition::Instant => {}
                    _ => self.paint(&b, snap, from, w, h, (-dx * e, -dy * e), 1.0),
                }
                let (ox, oy, alpha) = match how {
                    Transition::Dissolve => (0.0, 0.0, e),
                    Transition::Instant => (0.0, 0.0, 1.0),
                    _ => (dx * (1.0 - e), dy * (1.0 - e), 1.0),
                };
                self.paint(&b, snap, &current, w, h, (ox, oy), alpha);
                return;
            }
        }
        self.paint(&b, snap, &current, w, h, (0.0, 0.0), 1.0);
        if now < self.flash.get() {
            if let Some((r, scale, ox, oy)) = self.fit(&b, &current, w, h) {
                let scene = b.doc.scene.borrow();
                let placed = b.placed();
                let mut c = rgba(SELECT);
                c.set_alpha(0.35);
                for id in scene.subtree(&current) {
                    if scene.node(&id).is_some_and(|n| n.goto.is_some()) {
                        if let Some(q) = placed.rect(&id) {
                            let s = Rect::new(ox + (q.x - r.x) * scale, oy + (q.y - r.y) * scale, q.w * scale, q.h * scale);
                            snap.append_color(&c, &grect(&s));
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(&self, b: &Board, snap: &gtk::Snapshot, id: &str, w: f64, h: f64, shift: (f64, f64), alpha: f64) {
        let Some((r, scale, ox, oy)) = self.fit(b, id, w, h) else { return };
        let scene = b.doc.scene.borrow();
        let placed = b.placed();
        if alpha < 1.0 {
            snap.push_opacity(alpha);
        }
        snap.save();
        snap.translate(&graphene::Point::new((ox + shift.0) as f32, (oy + shift.1) as f32));
        snap.scale(scale as f32, scale as f32);
        snap.translate(&graphene::Point::new(-r.x as f32, -r.y as f32));
        snap.push_clip(&grect(&r));
        self.painter.paint(snap, &scene, &placed, id, None);
        snap.pop();
        snap.restore();
        if alpha < 1.0 {
            snap.pop();
        }
    }

    fn click(&self, x: f64, y: f64) {
        let Some(b) = self.board.upgrade() else { return };
        let current = self.current.borrow().clone();
        let (w, h) = (self.canvas.width() as f64, self.canvas.height() as f64);
        let Some((r, scale, ox, oy)) = self.fit(&b, &current, w, h) else { return };
        let p = (r.x + (x - ox) / scale, r.y + (y - oy) / scale);
        let target = {
            let scene = b.doc.scene.borrow();
            let placed = b.placed();
            let page = scene.page_of(&current).unwrap_or_default().to_string();
            let chain = hit::chain(&scene, &placed, &page, p.0, p.1, 2.0);
            chain
                .iter()
                .rev()
                .take_while(|id| **id != current || scene.node(id).is_some_and(|n| n.goto.is_some()))
                .chain(std::iter::once(&current))
                .find_map(|id| scene.node(id).and_then(|n| Some((n.goto.clone()?, n.transition.unwrap_or(Transition::Dissolve)))))
                .filter(|(to, _)| scene.node(to).is_some())
        };
        match target {
            Some((to, how)) => self.go(to, how),
            None => {
                self.flash.set(glib::monotonic_time() + 500_000);
                self.animate();
            }
        }
    }

    fn step(&self, dir: i32) {
        let current = self.current.borrow().clone();
        let Some(i) = self.frames.iter().position(|f| *f == current) else { return };
        let next = i as i32 + dir;
        if next < 0 || next as usize >= self.frames.len() {
            return;
        }
        let how = if dir > 0 { Transition::SlideLeft } else { Transition::SlideRight };
        self.go(self.frames[next as usize].clone(), how);
    }

    fn go(&self, to: String, how: Transition) {
        let from = self.current.replace(to);
        *self.leaving.borrow_mut() = Some((from, how, glib::monotonic_time()));
        self.animate();
    }

    /// Redraw every frame while a transition or a flash is under way.
    fn animate(&self) {
        let end = self.leaving.borrow().as_ref().map_or(0, |(_, _, at)| at + SLIDE_US).max(self.flash.get());
        self.until.set(self.until.get().max(end));
        if self.ticking.replace(true) {
            return;
        }
        let (until, ticking) = (self.until.clone(), self.ticking.clone());
        self.canvas.add_tick_callback(move |c, clock| {
            c.queue_draw();
            if clock.frame_time() > until.get() {
                ticking.set(false);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
}

/// Where a slide brings the next screen in from, in pixels.
fn slide(how: Transition, w: f64, h: f64) -> (f64, f64) {
    match how {
        Transition::SlideLeft => (w, 0.0),
        Transition::SlideRight => (-w, 0.0),
        Transition::SlideUp => (0.0, h),
        Transition::SlideDown => (0.0, -h),
        _ => (0.0, 0.0),
    }
}
