//! The project's videos, in the Design gallery after its designs (docs/VIDEO.md): a card per
//! video with its poster (the latest still an agent or you drew), its length and shape, its
//! terminal and its newest render, under the same chips and search as the designs. Where the
//! machine is not set up to render, a banner says what setting it up downloads and does it.
//!
//! The section shows where the Video plugin is on, or where the project has videos already.

use super::gallery::{Filter, Gallery};
use crate::app::{label, Ui};
use crate::relative::{ago, Form};
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use relay_bus::ops::video::{KitState, KitStatus, ListOut, RenderState, Video};
use relay_client::design::names::name_key;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::SystemTime;

type Opener = Rc<dyn Fn(i64)>;

pub struct Shelf {
    pub root: gtk::Box,
    ui: Weak<Ui>,
    gallery: Weak<Gallery>,
    count: gtk::Label,
    banner: gtk::Box,
    banner_text: gtk::Label,
    banner_key: gtk::Button,
    banner_spin: gtk::Spinner,
    grid: gtk::FlowBox,
    note: gtk::Label,
    videos: RefCell<Vec<Video>>,
    plugin_on: Cell<bool>,
    kit: RefCell<Option<KitStatus>>,
    /// The project the videos were read for.
    project: Cell<i64>,
    /// A read is under way; another is owed when it ends.
    reading: Cell<bool>,
    again: Cell<bool>,
    /// Posters decoded, by video, with the file's time when it was read.
    posters: RefCell<HashMap<i64, (SystemTime, gdk::Texture)>>,
    /// The cards' ids in the grid's order.
    order: RefCell<Vec<i64>>,
    pub open: RefCell<Option<Opener>>,
    me: Weak<Shelf>,
}

/// `0:08`, `1:02:05`: a length in frames at `fps`.
pub fn clock(frames: u32, fps: f64) -> String {
    let s = (frames as f64 / fps.max(1.0)).round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// `9:16`, `16:9`, `1:1`, `4:5`, else the size.
pub fn shape(w: u32, h: u32) -> String {
    let gcd = |mut a: u32, mut b: u32| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let g = gcd(w, h);
    let (a, b) = (w / g, h / g);
    if a <= 21 && b <= 21 { format!("{a}:{b}") } else { format!("{w}×{h}") }
}

/// A card's picture: 560 × 350 px for a crisp 280 × 175 card, as the designs' are.
const POSTER_W: u32 = 560;
const POSTER_H: u32 = 350;

/// `texture` fitted inside `w` × `h` on the video ground, each pixel the average of those it
/// covers: every card the same size whatever the video's shape (a vertical one is pillarboxed).
fn letterbox(texture: &gdk::Texture, w: u32, h: u32) -> gdk::Texture {
    let (tw, th) = (texture.width().max(1) as u32, texture.height().max(1) as u32);
    let stride = tw as usize * 4;
    let mut src = vec![0u8; stride * th as usize];
    // The default memory format: B8G8R8A8, premultiplied; averaging is the same in any order.
    texture.download(&mut src, stride);
    let scale = (w as f64 / tw as f64).min(h as f64 / th as f64);
    let (dw, dh) = (((tw as f64 * scale).round() as u32).clamp(1, w), ((th as f64 * scale).round() as u32).clamp(1, h));
    let (ox, oy) = ((w - dw) / 2, (h - dh) / 2);
    let mut out = vec![0u8; w as usize * h as usize * 4];
    for px in out.as_chunks_mut::<4>().0 {
        *px = [0x12, 0x0d, 0x0b, 0xff];
    }
    let span = |i: u32, n: u32, of: u32| -> (u32, u32) {
        let a = (i as f64 * of as f64 / n as f64) as u32;
        let b = (((i + 1) as f64 * of as f64 / n as f64) as u32).clamp(a + 1, of);
        (a, b)
    };
    for y in 0..dh {
        let (y0, y1) = span(y, dh, th);
        for x in 0..dw {
            let (x0, x1) = span(x, dw, tw);
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let at = sy as usize * stride + sx as usize * 4;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += src[at + c] as u32;
                    }
                }
            }
            let n = (x1 - x0) * (y1 - y0);
            let at = ((oy + y) as usize * w as usize + (ox + x) as usize) * 4;
            for c in 0..4 {
                out[at + c] = (sum[c] / n) as u8;
            }
        }
    }
    gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::B8g8r8a8Premultiplied, &glib::Bytes::from_owned(out), w as usize * 4).upcast()
}

/// What a card says of a video's newest render.
fn render_note(v: &Video) -> String {
    match &v.last_render {
        Some(r) if r.state == RenderState::Rendering => "rendering…".into(),
        Some(r) if r.state == RenderState::Queued => "render queued".into(),
        Some(r) if r.state == RenderState::Failed => "render failed".into(),
        Some(r) if r.state == RenderState::Done => match r.finished_at.as_deref().and_then(|t| ago(t, Form::CompactAgo)) {
            Some(when) => format!("rendered {when}"),
            None => "rendered".into(),
        },
        _ => ago(&v.updated_at, Form::CompactAgo).map_or_else(String::new, |when| format!("edited {when}")),
    }
}

impl Shelf {
    pub fn new(ui: &Rc<Ui>, gallery: &Rc<Gallery>) -> Rc<Shelf> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("video-shelf");
        root.set_visible(false);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.append(&label("Videos", "design-gallery-title"));
        let count = label("", "design-gallery-count");
        count.set_valign(gtk::Align::Baseline);
        head.append(&count);
        root.append(&head);

        let banner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        banner.add_css_class("video-kit-banner");
        let banner_spin = crate::app::spinner();
        banner_spin.set_visible(false);
        banner.append(&banner_spin);
        let banner_text = label("", "dim");
        banner_text.set_wrap(true);
        banner_text.set_xalign(0.0);
        banner_text.set_hexpand(true);
        banner.append(&banner_text);
        let banner_key = gtk::Button::with_label("Set up");
        banner_key.add_css_class("primary");
        banner_key.set_valign(gtk::Align::Center);
        banner.append(&banner_key);
        banner.set_visible(false);
        root.append(&banner);

        let grid = gtk::FlowBox::new();
        grid.add_css_class("design-grid");
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        grid.set_min_children_per_line(4);
        grid.set_max_children_per_line(4);
        grid.set_column_spacing(16);
        grid.set_row_spacing(20);
        grid.set_valign(gtk::Align::Start);
        root.append(&grid);
        let note = label("", "dim");
        note.set_xalign(0.0);
        note.set_wrap(true);
        root.append(&note);

        let shelf = Rc::new_cyclic(|me| Shelf {
            root,
            ui: Rc::downgrade(ui),
            gallery: Rc::downgrade(gallery),
            count,
            banner,
            banner_text,
            banner_key: banner_key.clone(),
            banner_spin,
            grid: grid.clone(),
            note,
            videos: RefCell::default(),
            plugin_on: Cell::new(false),
            kit: RefCell::default(),
            project: Cell::new(0),
            reading: Cell::new(false),
            again: Cell::new(false),
            posters: RefCell::default(),
            order: RefCell::default(),
            open: RefCell::default(),
            me: me.clone(),
        });
        let weak = Rc::downgrade(&shelf);
        banner_key.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                s.set_up();
            }
        });
        let weak = Rc::downgrade(&shelf);
        grid.connect_child_activated(move |_, child| {
            let Some(s) = weak.upgrade() else { return };
            let id = s.order.borrow().get(child.index().max(0) as usize).copied();
            let live = id.and_then(|id| s.videos.borrow().iter().find(|v| v.id == id).map(|v| v.deleted_at.is_none()));
            if let (Some(id), Some(true)) = (id, live) {
                let open = s.open.borrow().clone();
                if let Some(open) = open {
                    open(id);
                }
            }
        });
        gallery.videos.append(&shelf.root);
        let weak = Rc::downgrade(&shelf);
        *gallery.on_render.borrow_mut() = Some(Rc::new(move || {
            if let Some(s) = weak.upgrade() {
                s.render();
            }
        }));
        let weak = Rc::downgrade(&shelf);
        *gallery.on_refresh.borrow_mut() = Some(Rc::new(move || {
            if let Some(s) = weak.upgrade() {
                s.refresh();
            }
        }));
        shelf
    }

    /// Read the project's videos, whether Video is on, and the kit's state.
    pub fn refresh(&self) {
        let (Some(ui), Some(g)) = (self.ui.upgrade(), self.gallery.upgrade()) else { return };
        if self.reading.replace(true) {
            self.again.set(true);
            return;
        }
        let project = g.project.get();
        if project != self.project.get() {
            self.videos.borrow_mut().clear();
            self.project.set(project);
        }
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let listed = ui.call("video.list", json!({"project_id": project, "include_deleted": true})).await;
            let plugin = ui.call("plugin.get", json!({"plugin_id": "video"})).await;
            let kit = ui.call("video.kit.status", json!({})).await;
            let Some(s) = weak.upgrade() else { return };
            s.reading.set(false);
            if s.project.get() == project {
                if let Some(list) = listed.ok().and_then(|v| serde_json::from_value::<ListOut>(v).ok()) {
                    *s.videos.borrow_mut() = list.videos;
                }
                let on = plugin.ok().is_some_and(|v| v["plugin"]["enabled_in"].as_array().is_some_and(|ids| ids.iter().any(|i| i.as_i64() == Some(project))));
                s.plugin_on.set(on);
                *s.kit.borrow_mut() = kit.ok().and_then(|v| serde_json::from_value(v).ok());
                s.render();
            }
            if s.again.replace(false) {
                s.refresh();
            }
        });
    }

    /// An event about videos: read them again when it is this project's (or the kit's).
    pub fn event(&self, ev: &str, payload: &Value) {
        let mine = payload["project_id"].as_i64() == Some(self.project.get());
        if ev == "video.kit.changed" {
            *self.kit.borrow_mut() = serde_json::from_value(payload.clone()).ok();
            self.render();
        } else if mine && (ev == "video.changed" || ev == "video.render.finished" || payload["progress"].as_f64() == Some(0.0)) {
            // A render's tenths change no card but its own state, which the start and the end carry.
            self.refresh();
        }
    }

    fn visible(&self, filter: &Filter, q: &str) -> Vec<Video> {
        self.videos.borrow().iter().filter(|v| match filter {
            Filter::Deleted => v.deleted_at.is_some(),
            _ if v.deleted_at.is_some() => false,
            Filter::All => true,
            Filter::Terminal(s) => v.session.as_deref() == Some(s.as_str()),
            Filter::Project => v.session.is_none(),
        })
        .filter(|v| q.is_empty() || name_key(&v.title).contains(q) || v.session.as_deref().is_some_and(|s| name_key(s).contains(q)))
        .cloned()
        .collect()
    }

    fn render(&self) {
        let Some(g) = self.gallery.upgrade() else { return };
        let (filter, q) = (g.filter(), g.search_key());
        let list = self.visible(&filter, &q);
        let any = self.videos.borrow().iter().any(|v| v.deleted_at.is_none());
        let on = self.plugin_on.get();
        self.root.set_visible(on || any || !list.is_empty());
        self.count.set_text(&if list.is_empty() { String::new() } else { list.len().to_string() });

        // The kit, where videos can be made.
        let kit = self.kit.borrow().clone();
        let state = kit.as_ref().map(|k| k.state);
        let (show, text, key, spin) = match state {
            _ if !on && !any => (false, String::new(), None, false),
            None | Some(KitState::Ready) => (false, String::new(), None, false),
            Some(KitState::Installing) => (true, "Setting up video rendering… it downloads about 600 MB once.".to_string(), None, true),
            Some(KitState::NoNode) => (true, "Rendering videos needs Node.js 20 or newer. Install nodejs and npm with your package manager, then come back.".to_string(), None, false),
            Some(KitState::Outdated) => (true, "Video rendering was set up for an older Relay.".to_string(), Some("Update"), false),
            Some(KitState::Failed) => (true, format!("Setting up video rendering failed: {}", kit.as_ref().and_then(|k| k.error.clone()).unwrap_or_default()), Some("Try again"), false),
            Some(KitState::Missing) => (true, kit.as_ref().map_or_else(String::new, |k| k.about.clone()), Some("Set up"), false),
        };
        self.banner.set_visible(show);
        self.banner_text.set_text(&text);
        self.banner_key.set_visible(key.is_some());
        if let Some(key) = key {
            self.banner_key.set_label(key);
        }
        self.banner_spin.set_visible(spin);
        self.banner_spin.set_spinning(spin);

        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        *self.order.borrow_mut() = list.iter().map(|v| v.id).collect();
        for v in &list {
            self.grid.append(&self.card(v, g.card_width()));
        }
        // Videos to show: the designs' big "No designs yet" gives them the page.
        if !list.is_empty() {
            g.hide_empty();
        }
        self.note.set_visible(list.is_empty() && on);
        self.note.set_text(match filter {
            Filter::Deleted => "No deleted videos.",
            _ if !q.is_empty() => "No video matches.",
            _ => "No videos yet. Ask an agent in this project for one: a promo, an explainer or a social clip, written as code and rendered here.",
        });
    }

    fn card(&self, v: &Video, w: i32) -> gtk::FlowBoxChild {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("design-card");
        card.add_css_class("video-card");
        card.set_widget_name(&format!("video-card-{}", v.id));
        let frame = gtk::Overlay::new();
        let picture = gtk::Picture::new();
        picture.add_css_class("design-thumb");
        picture.add_css_class("video-thumb");
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(w, (w as f64 / 1.6).round() as i32);
        picture.set_can_shrink(true);
        frame.set_child(Some(&picture));
        let badges = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        badges.set_halign(gtk::Align::End);
        badges.set_valign(gtk::Align::End);
        badges.set_margin_end(8);
        badges.set_margin_bottom(8);
        badges.append(&label(&shape(v.width, v.height), "video-badge"));
        badges.append(&label(&clock(v.duration_frames, v.fps), "video-badge"));
        frame.add_overlay(&badges);
        let play = crate::icons::image("play", 22);
        play.add_css_class("video-play-glyph");
        play.set_halign(gtk::Align::Center);
        play.set_valign(gtk::Align::Center);
        frame.add_overlay(&play);
        card.append(&frame);
        let title = label(&v.title, "design-card-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_xalign(0.0);
        card.append(&title);
        let meta = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        meta.add_css_class("design-card-meta");
        if let Some(session) = &v.session {
            let state = self.gallery.upgrade().and_then(|g| g.session_state(session)).unwrap_or_else(|| "closed".into());
            meta.append(&super::ask::lamp(&state));
            meta.append(&label(session, "design-card-terminal"));
            meta.append(&label("·", "faint"));
        }
        let detail = label(&render_note(v), "design-card-when");
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.append(&detail);
        card.append(&meta);
        if v.deleted_at.is_some() {
            card.add_css_class("deleted");
            let restore = gtk::Button::with_label("Restore");
            restore.set_halign(gtk::Align::Start);
            let (weak, id) = (self.me.clone(), v.id);
            restore.connect_clicked(move |_| {
                if let Some(s) = weak.upgrade() {
                    s.call("video.restore", json!({"video_id": id}));
                }
            });
            card.append(&restore);
        } else {
            card.set_cursor_from_name(Some("pointer"));
        }
        self.poster(v, &picture);
        let child = gtk::FlowBoxChild::new();
        child.set_child(Some(&card));
        child
    }

    /// Show `v`'s poster on `picture`: decoded off the main thread once per change of the file.
    fn poster(&self, v: &Video, picture: &gtk::Picture) {
        let Some(path) = v.poster.clone() else { return };
        let id = v.id;
        let weak = self.me.clone();
        let picture = picture.downgrade();
        glib::spawn_future_local(async move {
            let p = path.clone();
            let modified = gio::spawn_blocking(move || std::fs::metadata(&p).and_then(|m| m.modified()).ok()).await.ok().flatten();
            let Some(modified) = modified else { return };
            let kept = weak.upgrade().and_then(|s| s.posters.borrow().get(&id).filter(|(at, _)| *at == modified).map(|(_, t)| t.clone()));
            let texture = match kept {
                Some(t) => Some(t),
                None => gio::spawn_blocking(move || gdk::Texture::from_filename(&path).ok().map(|t| letterbox(&t, POSTER_W, POSTER_H))).await.ok().flatten(),
            };
            let (Some(s), Some(picture), Some(texture)) = (weak.upgrade(), picture.upgrade(), texture) else { return };
            s.posters.borrow_mut().insert(id, (modified, texture.clone()));
            picture.set_paintable(Some(&texture));
        });
    }

    fn call(&self, op: &'static str, payload: Value) {
        let Some(ui) = self.ui.upgrade() else { return };
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let result = ui.call(op, payload).await;
            let Some(s) = weak.upgrade() else { return };
            match result {
                Ok(_) => s.refresh(),
                Err(e) => {
                    if let Some(g) = s.gallery.upgrade() {
                        g.toast(&format!("Could not do it: {e}"), None);
                    }
                }
            }
        });
    }

    fn set_up(&self) {
        self.banner_key.set_visible(false);
        self.banner_spin.set_visible(true);
        self.banner_spin.set_spinning(true);
        self.call("video.kit.install", json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_and_shapes_read_as_people_say_them() {
        assert_eq!(clock(240, 30.0), "0:08");
        assert_eq!(clock(30 * 62, 30.0), "1:02");
        assert_eq!(clock(30 * 3725, 30.0), "1:02:05");
        assert_eq!(shape(1080, 1920), "9:16");
        assert_eq!(shape(1920, 1080), "16:9");
        assert_eq!(shape(1080, 1350), "4:5");
        assert_eq!(shape(1000, 999), "1000×999");
    }
}
