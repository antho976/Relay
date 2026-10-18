//! Pages and layers. The layer list is the page's tree, top layer first as design tools show
//! it, folded to what you opened plus the way down to the selection. It is a recycling list
//! view: a design of thousands of layers builds rows only for those on screen.

use super::board::{display_name, kind_glyph, Board};
use super::glyphs;
use super::widgets::color_key;
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{NodeKind, NodeOp};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

#[derive(Clone)]
struct Row {
    id: String,
    depth: u32,
    folder: bool,
    open: bool,
}

pub struct Layers {
    pub root: gtk::Box,
    board: Weak<Board>,
    pages: gtk::Box,
    model: gtk::StringList,
    rows: RefCell<Vec<Row>>,
    open: RefCell<HashSet<String>>,
}

impl Layers {
    pub fn new(b: &Rc<Board>) -> Rc<Layers> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("design-layers");
        root.set_size_request(248, -1);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        head.add_css_class("design-layers-head");
        let title = label("Pages", "design-section-title");
        title.set_hexpand(true);
        head.append(&title);
        let add = crate::app::icon_button("plus", "Add a page");
        add.set_action_name(Some("design.add-page"));
        head.append(&add);
        root.append(&head);
        let pages = gtk::Box::new(gtk::Orientation::Vertical, 1);
        pages.add_css_class("design-pages");
        root.append(&pages);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let layers_title = label("Layers", "design-section-title");
        layers_title.add_css_class("design-layers-head");
        root.append(&layers_title);
        let model = gtk::StringList::new(&[]);
        let factory = gtk::SignalListItemFactory::new();
        let view = gtk::ListView::new(Some(gtk::NoSelection::new(Some(model.clone()))), Some(factory.clone()));
        view.add_css_class("design-layer-list");
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&view));
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        root.append(&scroll);
        let layers = Rc::new(Layers {
            root,
            board: Rc::downgrade(b),
            pages,
            model,
            rows: RefCell::default(),
            open: RefCell::default(),
        });

        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.add_css_class("design-layer");
            let indent = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            row.append(&indent);
            let disclose = gtk::Button::new();
            disclose.add_css_class("design-disclose");
            row.append(&disclose);
            let glyph = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            row.append(&glyph);
            let name = gtk::Label::new(None);
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&name);
            let eye = gtk::Button::new();
            eye.add_css_class("design-layer-key");
            eye.add_css_class("design-layer-eye");
            row.append(&eye);
            let lock = gtk::Button::new();
            lock.add_css_class("design-layer-key");
            row.append(&lock);
            // The keys take room only under the pointer or while on (hidden, locked); invisible,
            // they still held ~50px a name could use ("Weekly usage …" in a 248px panel).
            let motion = gtk::EventControllerMotion::new();
            let (r, e, l) = (row.downgrade(), eye.downgrade(), lock.downgrade());
            motion.connect_enter(move |_, _, _| {
                if let (Some(r), Some(e), Some(l)) = (r.upgrade(), e.upgrade(), l.upgrade()) {
                    show_keys(&r, &e, &l, true);
                }
            });
            let (r, e, l) = (row.downgrade(), eye.downgrade(), lock.downgrade());
            motion.connect_leave(move |_| {
                if let (Some(r), Some(e), Some(l)) = (r.upgrade(), e.upgrade(), l.upgrade()) {
                    show_keys(&r, &e, &l, false);
                }
            });
            row.add_controller(motion);
            item.set_child(Some(&row));
        });
        let weak = Rc::downgrade(&layers);
        factory.connect_bind(move |_, item| {
            let (Some(item), Some(layers)) = (item.downcast_ref::<gtk::ListItem>(), weak.upgrade()) else {
                return;
            };
            layers.bind(item);
        });
        layers.pages_changed();
        layers
    }

    fn bind(self: &Rc<Self>, item: &gtk::ListItem) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        let Some(row) = self.rows.borrow().get(item.position() as usize).cloned() else {
            return;
        };
        let Some(widget) = item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let scene = b.doc.scene.borrow();
        let Some(n) = scene.node(&row.id).cloned() else {
            return;
        };
        drop(scene);
        let selected = b.selection.borrow().contains(&row.id);
        if selected {
            widget.add_css_class("selected");
        } else {
            widget.remove_css_class("selected");
        }
        if n.is_hidden() {
            widget.add_css_class("hidden");
        } else {
            widget.remove_css_class("hidden");
        }
        let mut child = widget.first_child();
        let mut parts = Vec::new();
        while let Some(c) = child {
            child = c.next_sibling();
            parts.push(c);
        }
        let [indent, disclose, glyph, name, eye, lock] = parts.as_slice() else {
            return;
        };
        indent.set_size_request(row.depth as i32 * 14, -1);
        let disclose = disclose.clone().downcast::<gtk::Button>().expect("button");
        disclose.set_child(row.folder.then(|| crate::icons::image(if row.open { "chevron-down" } else { "chevron-right" }, 10)).as_ref());
        disclose.set_sensitive(row.folder);
        let glyph = glyph.clone().downcast::<gtk::Box>().expect("box");
        while let Some(c) = glyph.first_child() {
            glyph.remove(&c);
        }
        glyph.append(&glyphs::image(kind_glyph(n.kind), 14));
        name.clone().downcast::<gtk::Label>().expect("label").set_text(&display_name(&n));
        let eye = eye.clone().downcast::<gtk::Button>().expect("button");
        eye.set_child(Some(&glyphs::image(if n.is_hidden() { "eye-off" } else { "eye" }, 14)));
        eye.set_tooltip_text(Some(if n.is_hidden() { "Show" } else { "Hide" }));
        let lock = lock.clone().downcast::<gtk::Button>().expect("button");
        lock.set_child(Some(&glyphs::image(if n.is_locked() { "lock" } else { "unlock" }, 14)));
        lock.set_tooltip_text(Some(if n.is_locked() { "Unlock" } else { "Lock" }));
        if n.is_locked() {
            lock.add_css_class("on");
        } else {
            lock.remove_css_class("on");
        }
        show_keys(&widget, &eye, &lock, widget.state_flags().contains(gtk::StateFlags::PRELIGHT));
        // Handlers are bound once per row widget and read the row they show now.
        if widget.widget_name() != "bound" {
            widget.set_widget_name("bound");
            let weak = Rc::downgrade(self);
            let item_ref = item.downgrade();
            disclose.connect_clicked(move |_| {
                let (Some(l), Some(item)) = (weak.upgrade(), item_ref.upgrade()) else {
                    return;
                };
                let Some(row) = l.rows.borrow().get(item.position() as usize).cloned() else {
                    return;
                };
                {
                    let mut open = l.open.borrow_mut();
                    if !open.remove(&row.id) {
                        open.insert(row.id.clone());
                    }
                }
                l.refresh();
            });
            for (key, flag) in [(eye.clone(), "hidden"), (lock.clone(), "locked")] {
                let weak = Rc::downgrade(self);
                let item_ref = item.downgrade();
                key.connect_clicked(move |_| {
                    let (Some(l), Some(item)) = (weak.upgrade(), item_ref.upgrade()) else {
                        return;
                    };
                    let Some(b) = l.board.upgrade() else {
                        return;
                    };
                    let Some(row) = l.rows.borrow().get(item.position() as usize).cloned() else {
                        return;
                    };
                    let on = b
                        .doc
                        .scene
                        .borrow()
                        .node(&row.id)
                        .is_some_and(|n| if flag == "hidden" { !n.is_hidden() } else { !n.is_locked() });
                    let mut set = Map::new();
                    set.insert(flag.into(), if on { json!(true) } else { Value::Null });
                    b.commit(vec![NodeOp::Update { id: row.id.clone(), set }]);
                });
            }
            let click = gtk::GestureClick::new();
            let weak = Rc::downgrade(self);
            let item_ref = item.downgrade();
            click.connect_pressed(move |g, n_press, _, _| {
                let (Some(l), Some(item)) = (weak.upgrade(), item_ref.upgrade()) else {
                    return;
                };
                let Some(b) = l.board.upgrade() else {
                    return;
                };
                let Some(row) = l.rows.borrow().get(item.position() as usize).cloned() else {
                    return;
                };
                if n_press == 2 {
                    if let Some(w) = item.child() {
                        rename_node(&b, &row.id, Some(&w));
                    }
                    return;
                }
                let shift = g.current_event_state().contains(gtk::gdk::ModifierType::SHIFT_MASK);
                let mut sel = if shift { b.selection.borrow().clone() } else { Vec::new() };
                if let Some(i) = sel.iter().position(|s| *s == row.id) {
                    sel.remove(i);
                } else {
                    sel.push(row.id.clone());
                }
                b.select(sel);
            });
            widget.add_controller(click);
        }
    }

    /// Rebuild the rows from the scene: the page's tree, top layer first.
    pub fn refresh(&self) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        if !b.layers_panel.reveals_child() {
            return;
        }
        let scene = b.doc.scene.borrow();
        let page = b.page.borrow().clone();
        // The way down to the selection opens by itself.
        {
            let mut open = self.open.borrow_mut();
            for id in b.selection.borrow().iter() {
                for a in scene.ancestors(id) {
                    open.insert(a.to_string());
                }
            }
        }
        let open = self.open.borrow();
        let mut rows = Vec::new();
        let mut stack: Vec<(String, u32)> = scene.children(&page).iter().map(|c| (c.clone(), 0)).collect();
        // Children are bottom to top; the stack pops the top layer first.
        while let Some((id, depth)) = stack.pop() {
            let kids = scene.children(&id);
            let folder = !kids.is_empty();
            let is_open = folder && open.contains(&id);
            rows.push(Row { id: id.clone(), depth, folder, open: is_open });
            if is_open {
                stack.extend(kids.iter().map(|c| (c.clone(), depth + 1)));
            }
        }
        drop(open);
        drop(scene);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        *self.rows.borrow_mut() = rows.clone();
        self.model.splice(0, self.model.n_items(), &ids);
    }

    pub fn selection_changed(&self) {
        self.refresh();
    }

    pub fn pages_changed(&self) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        while let Some(c) = self.pages.first_child() {
            self.pages.remove(&c);
        }
        let pages = b.doc.scene.borrow().pages().to_vec();
        let current = b.page.borrow().clone();
        for page in pages {
            self.pages.append(&page_row(&b, &page, page == current, None));
        }
        self.refresh();
    }
}

/// A layer row's eye and lock: shown under the pointer, and otherwise only while they are on.
fn show_keys(row: &gtk::Box, eye: &gtk::Button, lock: &gtk::Button, hover: bool) {
    eye.set_visible(hover || row.has_css_class("hidden"));
    lock.set_visible(hover || lock.has_css_class("on"));
}

/// One page's row: click to open, double click to rename, its menu to delete.
fn page_row(b: &Rc<Board>, page: &str, current: bool, pop: Option<&gtk::Popover>) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("design-page-row");
    if current {
        row.add_css_class("selected");
    }
    row.append(&glyphs::image("page", 14));
    let name = label(&b.page_name(page), "");
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&name);
    if current {
        row.append(&crate::icons::image("check", 12));
    }
    let click = gtk::GestureClick::new();
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let p = pop.map(|p| p.downgrade());
    let w = row.downgrade();
    click.connect_pressed(move |_, n, _, _| {
        let Some(b) = weak.upgrade() else {
            return;
        };
        if n == 2 {
            if let Some(w) = w.upgrade() {
                rename_node(&b, &id, Some(w.upcast_ref()));
            }
            return;
        }
        b.set_page(&id);
        if let Some(p) = p.as_ref().and_then(|p| p.upgrade()) {
            p.popdown();
        }
        let layers = b.layers.borrow().clone();
        if let Some(l) = layers {
            l.pages_changed();
        }
    });
    row.add_controller(click);
    let menu = gtk::GestureClick::new();
    menu.set_button(gtk::gdk::BUTTON_SECONDARY);
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let w = row.downgrade();
    menu.connect_pressed(move |_, _, _, _| {
        let (Some(b), Some(w)) = (weak.upgrade(), w.upgrade()) else {
            return;
        };
        page_menu(&b, &id, w.upcast_ref());
    });
    row.add_controller(menu);
    row.upcast()
}

fn page_menu(b: &Rc<Board>, page: &str, anchor: &gtk::Widget) {
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.set_parent(anchor);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let rename = gtk::Button::with_label("Rename");
    rename.add_css_class("design-menu-row");
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let a = anchor.downgrade();
    let p = pop.downgrade();
    rename.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        if let (Some(b), Some(a)) = (weak.upgrade(), a.upgrade()) {
            rename_node(&b, &id, Some(&a));
        }
    });
    list.append(&rename);
    let duplicate = gtk::Button::with_label("Duplicate page");
    duplicate.add_css_class("design-menu-row");
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let p = pop.downgrade();
    duplicate.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        if let Some(b) = weak.upgrade() {
            duplicate_page(&b, &id);
        }
    });
    list.append(&duplicate);
    if b.doc.scene.borrow().pages().len() > 1 {
        let delete = gtk::Button::with_label("Delete page");
        delete.add_css_class("design-menu-row");
        delete.add_css_class("destructive");
        let weak = Rc::downgrade(b);
        let id = page.to_string();
        let p = pop.downgrade();
        delete.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(b) = weak.upgrade() {
                // Away from it only when it is the page on show.
                let next = b.doc.scene.borrow().pages().iter().find(|p| **p != id).cloned();
                if let Some(next) = next.filter(|_| *b.page.borrow() == id) {
                    b.set_page(&next);
                }
                b.commit(vec![NodeOp::Delete { id: id.clone() }]);
                b.toast("Page deleted · Ctrl+Z brings it back");
                if let Some(l) = b.layers.borrow().as_ref() {
                    l.pages_changed();
                }
            }
        });
        list.append(&delete);
    }
    pop.set_child(Some(&list));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

fn duplicate_page(b: &Rc<Board>, page: &str) {
    let scene = b.doc.scene.borrow();
    let nodes: Vec<_> = scene.subtree(page).iter().filter_map(|id| scene.node(id).cloned()).collect();
    drop(scene);
    let mut ops = Vec::new();
    let mut fresh = std::collections::HashMap::new();
    {
        let mut scene = b.doc.scene.borrow_mut();
        for n in &nodes {
            fresh.insert(n.id.clone(), scene.fresh_id(n.kind));
        }
    }
    let index = b.doc.scene.borrow().index(page).map(|i| i as u32 + 1);
    for n in &nodes {
        let mut copy = n.clone();
        copy.id = fresh[&n.id].clone();
        copy.parent = n.parent.as_ref().map(|p| fresh[p].clone());
        // Connectors follow the copies of the shapes they joined, all on the new page.
        copy.from = n.from.as_ref().map(|e| fresh.get(e).cloned().unwrap_or_else(|| e.clone()));
        copy.to = n.to.as_ref().map(|e| fresh.get(e).cloned().unwrap_or_else(|| e.clone()));
        if n.kind == NodeKind::Page {
            copy.name = Some(format!("{} copy", b.page_name(page)));
            ops.push(NodeOp::Add { node: copy, index });
        } else {
            ops.push(NodeOp::Add { node: copy, index: None });
        }
    }
    let new_page = fresh[page].clone();
    if b.commit(ops) {
        b.set_page(&new_page);
        if let Some(l) = b.layers.borrow().as_ref() {
            l.pages_changed();
        }
    }
}

/// The pages popover of the dock: every page, a new one, and this page's canvas colour.
pub fn pages_menu(b: &Rc<Board>, pop: &gtk::Popover) -> gtk::Widget {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.add_css_class("design-pages-menu");
    list.append(&label("Pages", "design-menu-heading"));
    let current = b.page.borrow().clone();
    for page in b.doc.scene.borrow().pages().to_vec() {
        list.append(&page_row(b, &page, page == current, Some(pop)));
    }
    let add = gtk::Button::new();
    add.add_css_class("design-menu-row");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&crate::icons::image("plus", 14));
    row.append(&label("Add a page", ""));
    add.set_child(Some(&row));
    let weak = Rc::downgrade(b);
    let p = pop.downgrade();
    add.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        if let Some(b) = weak.upgrade() {
            b.add_page();
            if let Some(l) = b.layers.borrow().as_ref() {
                l.pages_changed();
            }
        }
    });
    list.append(&add);
    list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let canvas = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let caption = label("Canvas colour", "dim");
    caption.set_hexpand(true);
    canvas.append(&caption);
    let fill = b.doc.scene.borrow().node(&current).and_then(|n| n.fill.clone());
    let weak = Rc::downgrade(b);
    let page = current.clone();
    canvas.append(&color_key(b, Some(fill.as_deref().unwrap_or(super::paint::CANVAS)), "Canvas colour", true, move |c| {
        if let Some(b) = weak.upgrade() {
            let mut set = Map::new();
            set.insert("fill".into(), json!(c));
            b.commit(vec![NodeOp::Update { id: page.clone(), set }]);
        }
    }));
    list.append(&canvas);
    list.set_size_request(220, -1);
    list.upcast()
}

/// Rename a node (a page, a frame, any layer) in a small popover by `anchor`, or by the
/// frame's name on the canvas.
pub fn rename_node(b: &Rc<Board>, id: &str, anchor: Option<&gtk::Widget>) {
    let Some(n) = b.doc.scene.borrow().node(id).cloned() else {
        return;
    };
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    match anchor {
        Some(a) => pop.set_parent(a),
        None => {
            pop.set_parent(&b.canvas);
            if let Some(r) = b.placed().rect(id) {
                let s = b.cam.get().rect_to_screen(&r);
                pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(s.x as i32, (s.y - 14.0) as i32, 1, 1)));
            }
        }
    }
    let entry = gtk::Entry::new();
    entry.set_text(&if n.kind == NodeKind::Page { b.page_name(id) } else { display_name(&n) });
    entry.set_width_chars(24);
    pop.set_child(Some(&entry));
    let weak = Rc::downgrade(b);
    let id = id.to_string();
    let p = pop.downgrade();
    entry.connect_activate(move |e| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        let Some(b) = weak.upgrade() else {
            return;
        };
        let text = e.text().trim().to_string();
        let mut set = Map::new();
        set.insert("name".into(), if text.is_empty() { Value::Null } else { json!(text) });
        b.commit(vec![NodeOp::Update { id: id.clone(), set }]);
        let layers = b.layers.borrow().clone();
        if let Some(l) = layers {
            l.pages_changed();
        }
    });
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
    entry.grab_focus();
}
