//! The Assets panel, beside Layers: the design's own components first, then Relay's packs —
//! UI kits, wireframes, flowchart shapes, device frames and templates — as a searchable grid
//! of live thumbnails drawn by the canvas's own painter. A click places the piece in the middle
//! of the view (inside the selected frame); a drag drops it where it lands.

use super::board::Board;
use super::paint::{fill_of, rgba, CANVAS};
use crate::app::label;
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp};
use relay_bus::kits;
use relay_client::design::{layout, Camera, Rect, Scene};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A tile's picture, in pixels at 2×.
const TILE_W: f64 = 208.0;
const TILE_H: f64 = 136.0;
/// What a drag from a tile carries.
pub const DRAG_PREFIX: &str = "relay-design-item:";

pub struct Library {
    pub root: gtk::Box,
    board: Weak<Board>,
    search: gtk::SearchEntry,
    body: gtk::Box,
    pictures: RefCell<Vec<(String, gtk::Picture)>>,
    thumbs: RefCell<HashMap<String, gdk::Texture>>,
    built: Cell<bool>,
    pending: Cell<bool>,
    me: Weak<Library>,
}

impl Library {
    pub fn new(b: &Rc<Board>) -> Rc<Library> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("design-library");
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search assets"));
        search.set_margin_start(10);
        search.set_margin_end(10);
        root.append(&search);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&body));
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        root.append(&scroll);
        let lib = Rc::new_cyclic(|me| Library {
            root,
            board: Rc::downgrade(b),
            search,
            body,
            pictures: RefCell::default(),
            thumbs: RefCell::default(),
            built: Cell::new(false),
            pending: Cell::new(false),
            me: me.clone(),
        });
        let weak = Rc::downgrade(&lib);
        lib.search.connect_search_changed(move |_| {
            if let Some(l) = weak.upgrade() {
                l.build();
            }
        });
        let weak = Rc::downgrade(&lib);
        lib.root.connect_map(move |_| {
            if let Some(l) = weak.upgrade() {
                if !l.built.replace(true) {
                    l.build();
                }
            }
        });
        lib
    }

    /// The design changed while the panel shows: its components and styles may have, so the
    /// grid is built again (a component's picture drawn afresh), at most once per main-loop turn.
    pub fn refresh_design(&self) {
        if self.pending.replace(true) {
            return;
        }
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(l) = weak.upgrade() {
                l.pending.set(false);
                l.thumbs.borrow_mut().retain(|k, _| !k.starts_with('#'));
                l.build();
            }
        });
    }

    /// Rebuild the grid for the search text.
    pub fn build(&self) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        while let Some(c) = self.body.first_child() {
            self.body.remove(&c);
        }
        self.pictures.borrow_mut().clear();
        let q = self.search.text().to_lowercase();
        let matches = |name: &str, category: &str| q.is_empty() || name.to_lowercase().contains(&q) || category.to_lowercase().contains(&q);
        // The design's own components.
        let components: Vec<Node> = {
            let scene = b.doc.scene.borrow();
            scene.to_nodes().into_iter().filter(|n| n.component == Some(true)).collect()
        };
        let mine: Vec<&Node> = components.iter().filter(|n| matches(&super::board::display_name(n), "component")).collect();
        if !mine.is_empty() {
            self.section("In this design", "Components: their copies follow them");
            let grid = self.grid();
            for n in mine {
                let path = format!("#{}", n.id);
                grid.insert(&self.tile(&path, &super::board::display_name(n), "Component"), -1);
            }
            self.body.append(&grid);
        }
        if let Some(list) = b.styles_list(&q) {
            self.section("Styles", "This design's shared colours, type and effects: a click gives the selection the style");
            self.body.append(&list);
        }
        for kit in kits::kits() {
            let items: Vec<_> = kit.items.iter().filter(|i| matches(&i.name, &i.category) || kit.name.to_lowercase().contains(&q)).collect();
            if items.is_empty() {
                continue;
            }
            self.section(&kit.name, &kit.description);
            let grid = self.grid();
            for item in items {
                let path = format!("{}/{}", kit.id, item.id);
                grid.insert(&self.tile(&path, &item.name, &item.category), -1);
            }
            self.body.append(&grid);
        }
        self.draw_next();
    }

    fn section(&self, title: &str, note: &str) {
        let head = label(title, "design-section-title");
        head.set_margin_start(12);
        head.set_margin_top(8);
        head.set_tooltip_text(Some(note));
        self.body.append(&head);
    }

    fn grid(&self) -> gtk::FlowBox {
        let grid = gtk::FlowBox::new();
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        grid.set_min_children_per_line(2);
        grid.set_max_children_per_line(2);
        grid.set_column_spacing(6);
        grid.set_row_spacing(6);
        grid.set_margin_start(8);
        grid.set_margin_end(8);
        grid
    }

    fn tile(&self, path: &str, name: &str, category: &str) -> gtk::Widget {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 3);
        tile.add_css_class("design-asset");
        let picture = gtk::Picture::new();
        picture.set_size_request((TILE_W / 2.0) as i32, (TILE_H / 2.0) as i32);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.add_css_class("design-asset-thumb");
        if let Some(t) = self.thumbs.borrow().get(path) {
            picture.set_paintable(Some(t));
        } else {
            self.pictures.borrow_mut().push((path.to_string(), picture.clone()));
        }
        tile.append(&picture);
        let caption = label(name.rsplit(" / ").next().unwrap_or(name), "design-asset-name");
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        tile.append(&caption);
        tile.set_tooltip_text(Some(&format!("{name} · {category}\nClick to place, or drag onto the canvas")));
        tile.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::new();
        let weak = self.board.clone();
        let p = path.to_string();
        click.connect_released(move |_, _, _, _| {
            if let Some(b) = weak.upgrade() {
                b.insert_item(&p, None);
            }
        });
        tile.add_controller(click);
        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::COPY);
        let p = format!("{DRAG_PREFIX}{path}");
        drag.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&p.to_value())));
        tile.add_controller(drag);
        tile.upcast()
    }

    /// Draw the next missing thumbnail, a few per main-loop turn so scrolling stays smooth.
    fn draw_next(&self) {
        let weak = self.me.clone();
        glib::idle_add_local_full(glib::Priority::LOW, move || {
            let Some(l) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            for _ in 0..4 {
                let next = l.pictures.borrow_mut().pop();
                let Some((path, picture)) = next else {
                    return glib::ControlFlow::Break;
                };
                if let Some(t) = l.render(&path) {
                    picture.set_paintable(Some(&t));
                    l.thumbs.borrow_mut().insert(path, t);
                }
            }
            glib::ControlFlow::Continue
        });
    }

    fn render(&self, path: &str) -> Option<gdk::Texture> {
        let b = self.board.upgrade()?;
        let mut nodes = vec![{
            let mut page = Node::new(NodeKind::Page);
            page.id = "thumb-page".into();
            page
        }];
        if let Some(local) = path.strip_prefix('#') {
            let scene = b.doc.scene.borrow();
            let subtree: Vec<Node> = scene.subtree(local).iter().filter_map(|id| scene.node(id).cloned()).collect();
            for (i, mut n) in subtree.into_iter().enumerate() {
                if i == 0 {
                    n.parent = Some("thumb-page".into());
                    (n.x, n.y) = (0.0, 0.0);
                }
                nodes.push(n);
            }
        } else {
            let (_, item) = kits::item(path)?;
            nodes.extend(kits::expand(item, "t", "thumb-page", 0.0, 0.0, None).ok()?);
        }
        let scene = Scene::new(nodes);
        let placed = layout::place(&scene, "thumb-page", &*b.painter);
        let bounds = Rect::bounds(scene.children("thumb-page").iter().filter_map(|id| placed.rects.get(id)))?;
        let snap = gtk::Snapshot::new();
        let ground = scene.node("thumb-page").and_then(fill_of).unwrap_or_else(|| CANVAS.into());
        snap.append_color(&rgba(&ground), &graphene::Rect::new(0.0, 0.0, TILE_W as f32, TILE_H as f32));
        let cam = Camera::fit(&bounds, TILE_W, TILE_H, 10.0, 2.0);
        snap.scale(cam.zoom as f32, cam.zoom as f32);
        snap.translate(&graphene::Point::new(-cam.x as f32, -cam.y as f32));
        b.painter.paint_page(&snap, &scene, &placed, "thumb-page", None);
        let node = snap.to_node()?;
        let renderer = b.canvas.native()?.renderer()?;
        Some(renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, TILE_W as f32, TILE_H as f32))))
    }
}

impl Board {
    /// Place a pack item (`kit/item`) or a copy of one of this design's components (`#id`) at
    /// `at` (page coordinates), or in the middle of the view; inside the selected frame as a
    /// paste would be.
    pub(super) fn insert_item(&self, path: &str, at: Option<(f64, f64)>) {
        let parent = self.paste_target();
        let origin = self.placed().rect(&parent).unwrap_or_default();
        let (w, h) = match path.strip_prefix('#') {
            Some(local) => self.doc.scene.borrow().node(local).map_or((0.0, 0.0), |n| (n.w, n.h)),
            None => kits::item(path).map_or((0.0, 0.0), |(_, i)| i.size()),
        };
        let (cx, cy) = at.unwrap_or_else(|| {
            let (vw, vh) = self.size.get();
            self.to_page(vw / 2.0, vh / 2.0)
        });
        let slug = path.trim_start_matches('#').rsplit('/').next().unwrap_or("item");
        let id = self.doc.scene.borrow_mut().fresh_named(slug);
        let op = NodeOp::Insert {
            item: path.to_string(),
            parent,
            x: (cx - origin.x - w / 2.0).round(),
            y: (cy - origin.y - h / 2.0).round(),
            index: None,
            id: Some(id.clone()),
            set: None,
        };
        if self.commit(vec![op]) {
            self.select(vec![id]);
        }
    }
}
