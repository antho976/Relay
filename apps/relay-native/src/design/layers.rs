//! Pages and layers. The layer list is the page's tree, top layer first as design tools show
//! it, folded to what you opened plus the way down to the selection. It is a recycling list
//! view: a design of thousands of layers builds rows only for those on screen. A row is dragged
//! to restack its layer, or onto a frame or group to put it inside.

use super::board::{kind_glyph, Board};
use super::glyphs;
use super::widgets::{color_key, tip};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{NodeKind, NodeOp};
use relay_client::design::edit;
use relay_client::design::names::{copy_name, display_name};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

/// A layer row's indent at `depth` (a page's children are 0 deep): 14 px a level for the first
/// six, then 4, at most 108 px, so a deep nest keeps room for its names and never widens the
/// panel (B92).
fn indent_px(depth: u32) -> i32 {
    let d = depth as i32;
    d.min(6) * 14 + (d - 6).clamp(0, 6) * 4
}

/// What a name typed into a rename popover writes over `shown`, the name it showed: nothing
/// when unchanged (a text keeps following its words, a page its place's name), a cleared name
/// when emptied, else the name trimmed.
fn rename_set(text: &str, shown: &str) -> Option<Value> {
    let text = text.trim();
    match text {
        _ if text == shown.trim() => None,
        "" => Some(Value::Null),
        _ => Some(json!(text)),
    }
}

#[derive(Clone, PartialEq)]
struct Row {
    id: String,
    depth: u32,
    folder: bool,
    open: bool,
}

/// What a pages list was built for: each page, its name and whether it is hidden, and
/// whether the fold of hidden pages is open.
pub(super) type PageSig = (Vec<(String, String, bool)>, bool);

thread_local! {
    /// Whether the pages lists open their fold of hidden pages.
    static HIDDEN_OPEN: Cell<bool> = const { Cell::new(false) };
}

pub struct Layers {
    pub root: gtk::Box,
    board: Weak<Board>,
    pages: gtk::Box,
    /// The pages and names the page rows were built for: while they stay, a page switch only
    /// relights the rows, so a row's double-click (rename) lands on the row it started on (B52).
    page_sig: RefCell<PageSig>,
    model: gtk::StringList,
    rows: RefCell<Vec<Row>>,
    /// What each row was last drawn showing (its layer's name, kind, eye, lock and whether it
    /// is selected): a refresh with the same rows rebinds only those whose look changed.
    looks: RefCell<Vec<u64>>,
    open: RefCell<HashSet<String>>,
    /// Said in place of the list when the page has nothing on it.
    empty: gtk::Label,
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
        // A row too wide is cut, not the panel widened over the canvas (B92).
        scroll.set_hscrollbar_policy(gtk::PolicyType::External);
        root.append(&scroll);
        let empty = label("Nothing on this page yet: draw with the tools above, or ask an agent.", "design-note");
        empty.set_wrap(true);
        empty.add_css_class("design-layers-empty");
        empty.set_visible(false);
        root.append(&empty);
        let layers = Rc::new(Layers {
            root,
            board: Rc::downgrade(b),
            pages,
            page_sig: RefCell::default(),
            model,
            rows: RefCell::default(),
            looks: RefCell::default(),
            open: RefCell::default(),
            empty,
        });

        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            // A row picks a layer; the keyboard stays with the canvas.
            item.set_focusable(false);
            item.set_activatable(false);
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
            super::board::keep_canvas_focus(row.upcast_ref());
            item.set_child(Some(&row));
        });
        let weak = Rc::downgrade(&layers);
        factory.connect_bind(move |_, item| {
            let (Some(item), Some(layers)) = (item.downcast_ref::<gtk::ListItem>(), weak.upgrade()) else {
                return;
            };
            layers.bind(item);
        });
        super::board::keep_canvas_focus(layers.root.upcast_ref());
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
        indent.set_size_request(indent_px(row.depth), -1);
        let disclose = disclose.clone().downcast::<gtk::Button>().expect("button");
        disclose.set_child(row.folder.then(|| crate::icons::image(if row.open { "chevron-down" } else { "chevron-right" }, 10)).as_ref());
        disclose.set_sensitive(row.folder);
        if row.folder {
            tip(&disclose, if row.open { "Collapse" } else { "Expand" });
        } else {
            disclose.set_tooltip_text(None);
        }
        let glyph = glyph.clone().downcast::<gtk::Box>().expect("box");
        while let Some(c) = glyph.first_child() {
            glyph.remove(&c);
        }
        glyph.append(&glyphs::image(kind_glyph(n.kind), 14));
        // The whole name under the pointer, where the row cuts it short; never blank (B42), and
        // how deep it sits where the indent stops growing.
        let shown = display_name(&n);
        let name = name.clone().downcast::<gtk::Label>().expect("label");
        name.set_text(&shown);
        name.set_tooltip_text(Some(&if row.depth > 6 { format!("{shown} · level {}", row.depth + 1) } else { shown.clone() }));
        let eye = eye.clone().downcast::<gtk::Button>().expect("button");
        eye.set_child(Some(&glyphs::image(if n.is_hidden() { "eye-off" } else { "eye" }, 14)));
        let show = if n.is_hidden() { "Show" } else { "Hide" };
        tip(&eye, show);
        // A screen reader hears which layer the key is for, row after row (B106).
        eye.update_property(&[gtk::accessible::Property::Label(&format!("{show} {shown}"))]);
        let lock = lock.clone().downcast::<gtk::Button>().expect("button");
        lock.set_child(Some(&glyphs::image(if n.is_locked() { "lock" } else { "unlock" }, 14)));
        let locking = if n.is_locked() { "Unlock" } else { "Lock" };
        tip(&lock, locking);
        lock.update_property(&[gtk::accessible::Property::Label(&format!("{locking} {shown}"))]);
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
                    // A layer hidden from its row leaves the selection, with what is in it, as
                    // Hide does on the canvas (TM-67): no handles round what cannot be seen.
                    if on && flag == "hidden" {
                        let scene = b.doc.scene.borrow();
                        let kept: Vec<String> = b.selection.borrow().iter().filter(|s| **s != row.id && !scene.is_inside(s, &row.id)).cloned().collect();
                        drop(scene);
                        b.select(kept);
                    }
                });
            }
            let click = gtk::GestureClick::new();
            let weak = Rc::downgrade(self);
            let item_ref = item.downgrade();
            click.connect_pressed(move |g, n_press, x, y| {
                let (Some(l), Some(item)) = (weak.upgrade(), item_ref.upgrade()) else {
                    return;
                };
                let Some(b) = l.board.upgrade() else {
                    return;
                };
                let Some(row) = l.rows.borrow().get(item.position() as usize).cloned() else {
                    return;
                };
                // The eye, the lock and the disclosure are keys of their own, not the row (TM-67).
                let on_key = g.widget().and_then(|w| w.pick(x, y, gtk::PickFlags::DEFAULT)).is_some_and(|w| w.is::<gtk::Button>() || w.ancestor(gtk::Button::static_type()).is_some());
                if on_key {
                    return;
                }
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
                    // Not with what holds it or what it holds, as a Shift-click on the canvas
                    // (E-board-13): a frame and its child selected together move twice.
                    let scene = b.doc.scene.borrow();
                    sel.retain(|s| !scene.is_inside(s, &row.id) && !scene.is_inside(&row.id, s));
                    drop(scene);
                    sel.push(row.id.clone());
                }
                // The canvas keeps its shortcuts (Delete, arrows, V…) after a row is picked:
                // now, and once the list has handled the click, should it have taken the keyboard.
                b.canvas.grab_focus();
                b.select(sel);
                let weak = Rc::downgrade(&b);
                glib::idle_add_local_once(move || {
                    if let Some(b) = weak.upgrade() {
                        if b.focus_widget().is_some_and(|f| f.is_ancestor(&b.layers_panel)) {
                            b.canvas.grab_focus();
                        }
                    }
                });
            });
            widget.add_controller(click);
            // Drag a row to restack its layer, or onto a frame or group to put it inside (I-16).
            let drag = gtk::DragSource::new();
            drag.set_actions(gtk::gdk::DragAction::MOVE);
            let weak = Rc::downgrade(self);
            let item_ref = item.downgrade();
            drag.connect_prepare(move |_, _, _| {
                let (l, item) = (weak.upgrade()?, item_ref.upgrade()?);
                let row = l.rows.borrow().get(item.position() as usize).cloned()?;
                Some(gtk::gdk::ContentProvider::for_value(&row.id.to_value()))
            });
            widget.add_controller(drag);
            let drop = gtk::DropTarget::new(glib::Type::STRING, gtk::gdk::DragAction::MOVE);
            let weak = Rc::downgrade(self);
            let item_ref = item.downgrade();
            drop.connect_drop(move |t, value, _, y| {
                let (Some(l), Some(item), Ok(dragged)) = (weak.upgrade(), item_ref.upgrade(), value.get::<String>()) else {
                    return false;
                };
                let Some(row) = l.rows.borrow().get(item.position() as usize).cloned() else {
                    return false;
                };
                let height = t.widget().map_or(1, |w| w.height()).max(1) as f64;
                l.drop_layer(&dragged, &row.id, y / height)
            });
            widget.add_controller(drop);
        }
    }

    /// Put layer `id` where it was dropped on row `target`, `at` down that row (0 its top, 1 its
    /// bottom): in front of it on the row's upper half, behind it on the lower, and inside it on
    /// its middle when it is a frame or group, as the top layer there. One step, which keeps the
    /// layer where it shows when it changes parent. A locked layer stays, and nothing goes into
    /// itself or what it holds.
    fn drop_layer(&self, id: &str, target: &str, at: f64) -> bool {
        let Some(b) = self.board.upgrade() else {
            return false;
        };
        let placed = b.placed();
        let scene = b.doc.scene.borrow();
        let (Some(n), Some(t)) = (scene.node(id), scene.node(target)) else {
            return false;
        };
        // A page row dragged from the pages list is not a layer: it has no parent to change.
        if id == target || n.is_locked() || n.kind == NodeKind::Page || scene.is_inside(target, id) {
            return false;
        }
        let into = t.kind.holds_children() && (0.3..0.7).contains(&at);
        // Where it goes among the new parent's children (bottom to top), counted with it there.
        let (parent, point) = if into {
            (target.to_string(), scene.children(target).len())
        } else {
            let (Some(parent), Some(i)) = (t.parent.clone(), scene.index(target)) else {
                return false;
            };
            // Rows run top layer first: above a row is in front of its layer.
            (parent, if at < 0.5 { i + 1 } else { i })
        };
        let ops = match scene.index(id).filter(|_| scene.parent(id) == Some(parent.as_str())) {
            // Among its own siblings, counted without it (`edit::reorder`).
            Some(own) => edit::reorder(&scene, &parent, &[id.to_string()], (point - usize::from(own < point)) as u32),
            None => {
                let from = n.parent.as_deref().map_or((0.0, 0.0), |p| placed.origin(p));
                let shows = placed.rect(id).map_or((from.0 + n.x, from.1 + n.y), |r| (r.x, r.y));
                edit::reparent(&scene, &placed, id, &parent, Some(point as u32), shows)
            }
        };
        drop(scene);
        !ops.is_empty() && b.commit(ops)
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
        let selection = b.selection.borrow();
        let looks: Vec<u64> = rows.iter().map(|r| {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            if let Some(n) = scene.node(&r.id) {
                (display_name(n), kind_glyph(n.kind), n.is_hidden(), n.is_locked()).hash(&mut h);
            }
            selection.contains(&r.id).hash(&mut h);
            h.finish()
        }).collect();
        drop(selection);
        drop(scene);
        self.empty.set_visible(rows.is_empty());
        let same_rows = *self.rows.borrow() == rows;
        let before = self.looks.replace(looks.clone());
        if same_rows && before.len() == looks.len() {
            // The same rows: only those that look otherwise are bound again, one at a time; on
            // a page of thousands of layers a whole splice is a new object and a rebind a row.
            for (i, row) in rows.iter().enumerate().filter(|(i, _)| before[*i] != looks[*i]) {
                self.model.splice(i as u32, 1, &[row.id.as_str()]);
            }
            return;
        }
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        *self.rows.borrow_mut() = rows.clone();
        self.model.splice(0, self.model.n_items(), &ids);
    }

    pub fn selection_changed(&self) {
        self.refresh();
    }

    /// The page rows as the design has them. The same pages under the same names keep their
    /// rows, only relit for the page on show, so the second press of a double-click lands on
    /// the gesture that had the first and renames (B52).
    pub fn pages_changed(&self) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        let current = b.page.borrow().clone();
        let sig = page_sig(&b);
        if *self.page_sig.borrow() == sig {
            let mut child = self.pages.first_child();
            while let Some(row) = child {
                child = row.next_sibling();
                if let Some(page) = row.widget_name().strip_prefix("design-page:") {
                    light_page_row(&row, page == current);
                }
            }
        } else {
            while let Some(c) = self.pages.first_child() {
                self.pages.remove(&c);
            }
            for row in page_rows(&b, None) {
                self.pages.append(&row);
            }
            *self.page_sig.borrow_mut() = sig;
        }
        self.refresh();
    }
}

impl Layers {
    /// The pages list as drawn, for the smoke's `pages` step: each row's page, whether it is
    /// hidden, and its and its eye's boxes on the window, the fold's box, and the pages' order.
    pub fn pages_state(&self) -> Value {
        let on_window = |w: &gtk::Widget| w.root().and_then(|r| w.compute_bounds(&r)).map(|b| [b.x(), b.y(), b.width(), b.height()]);
        let mut rows = Vec::new();
        let mut fold = None;
        let mut child = self.pages.first_child();
        while let Some(row) = child {
            child = row.next_sibling();
            if row.has_css_class("design-page-fold") {
                fold = on_window(&row);
            } else if let Some(page) = row.widget_name().strip_prefix("design-page:") {
                let eye = row.first_child().and_then(|g| g.next_sibling()).and_then(|n| n.next_sibling());
                rows.push(json!({"page": page, "hidden": row.has_css_class("hidden"), "box": on_window(&row),
                    "eye": eye.as_ref().filter(|e| e.is_visible()).and_then(on_window)}));
            }
        }
        let order = self.board.upgrade().map(|b| b.doc.scene.borrow().pages().to_vec()).unwrap_or_default();
        json!({"rows": rows, "fold": fold, "order": order})
    }
}

/// The pages, their names and whether each is hidden, and whether the fold of hidden pages
/// shows: a pages list built for them stays.
pub(super) fn page_sig(b: &Rc<Board>) -> PageSig {
    let scene = b.doc.scene.borrow();
    let pages: Vec<_> = scene.pages().iter().map(|p| (p.clone(), b.page_name(p), scene.node(p).is_some_and(|n| n.is_hidden()))).collect();
    let current = b.page.borrow().clone();
    let open = HIDDEN_OPEN.with(Cell::get) || pages.iter().any(|(p, _, hidden)| *hidden && *p == current);
    (pages, open)
}

/// A pages list's rows in the design's order: the pages shown, then a fold holding the hidden
/// ones (open while one of them is on show), each with an eye to show it again.
fn page_rows(b: &Rc<Board>, pop: Option<&gtk::Popover>) -> Vec<gtk::Widget> {
    let (pages, open) = page_sig(b);
    let current = b.page.borrow().clone();
    let mut rows: Vec<gtk::Widget> = pages.iter().filter(|p| !p.2).map(|(p, ..)| page_row(b, p, *p == current, pop, false)).collect();
    let hidden: Vec<&String> = pages.iter().filter(|p| p.2).map(|(p, ..)| p).collect();
    if hidden.is_empty() {
        return rows;
    }
    let fold = gtk::Button::new();
    fold.add_css_class("design-page-fold");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&crate::icons::image(if open { "chevron-down" } else { "chevron-right" }, 10));
    row.append(&label(&format!("Hidden · {}", hidden.len()), ""));
    fold.set_child(Some(&row));
    tip(&fold, if open { "Fold the hidden pages away" } else { "Show the hidden pages, to open or show them again" });
    let weak = Rc::downgrade(b);
    let p = pop.map(|p| p.downgrade());
    fold.connect_clicked(move |_| {
        HIDDEN_OPEN.with(|o| o.set(!open));
        if let Some(b) = weak.upgrade() {
            refresh_pages(&b, p.as_ref().and_then(|p| p.upgrade()).as_ref());
        }
    });
    rows.push(fold.upcast());
    if open {
        rows.extend(hidden.into_iter().map(|p| page_row(b, p, *p == current, pop, true)));
    }
    rows
}

/// Both pages lists built again: the Layers panel's, and the dock's in `pop` when it is open.
fn refresh_pages(b: &Rc<Board>, pop: Option<&gtk::Popover>) {
    if let Some(l) = b.layers.borrow().as_ref() {
        l.pages_changed();
    }
    if let Some(pop) = pop {
        pop.set_child(Some(&pages_menu(b, pop)));
    }
}

/// A page row lit, with its check, as the page on show, or not.
fn light_page_row(row: &gtk::Widget, current: bool) {
    if current {
        row.add_css_class("selected");
    } else {
        row.remove_css_class("selected");
    }
    if let Some(check) = row.last_child() {
        check.set_visible(current);
    }
}

/// Hide `page` from the pages lists, or show it again: one page always stays shown. Hidden,
/// it is still opened from the fold, and the board goes to the next page shown when it was on
/// show.
fn set_page_hidden(b: &Rc<Board>, page: &str, hide: bool) -> bool {
    let scene = b.doc.scene.borrow();
    let shown: Vec<String> = scene.pages().iter().filter(|p| scene.node(p).is_some_and(|n| !n.is_hidden())).cloned().collect();
    drop(scene);
    if hide && shown.iter().all(|p| p == page) {
        b.toast("One page stays shown: show another before hiding this one");
        return false;
    }
    let mut set = Map::new();
    set.insert("hidden".into(), if hide { json!(true) } else { Value::Null });
    if !b.commit(vec![NodeOp::Update { id: page.to_string(), set }]) {
        return false;
    }
    if hide {
        if *b.page.borrow() == page {
            let all = b.doc.scene.borrow().pages().to_vec();
            let at = all.iter().position(|p| p == page).unwrap_or(0);
            let next = all[at..].iter().chain(all[..at].iter().rev()).find(|p| *p != page && shown.contains(p)).cloned();
            if let Some(next) = next {
                b.set_page(&next);
            }
        }
        b.toast("Page hidden · it waits under Hidden at the end of the pages");
    }
    true
}

/// The page next to `page` in its own list, shown or hidden: above it with `up`.
fn page_neighbour(b: &Rc<Board>, page: &str, up: bool) -> Option<String> {
    let scene = b.doc.scene.borrow();
    let hidden = |p: &String| scene.node(p).is_some_and(|n| n.is_hidden());
    let mine = scene.node(page)?.is_hidden();
    let group: Vec<&String> = scene.pages().iter().filter(|p| hidden(p) == mine).collect();
    let at = group.iter().position(|p| *p == page)?;
    let next = if up { at.checked_sub(1)? } else { at + 1 };
    group.get(next).map(|p| p.to_string())
}

/// Put `page` before `target` (after it, with `after`) among the pages: one undo step.
fn move_page(b: &Rc<Board>, page: &str, target: &str, after: bool) -> bool {
    let op = edit::move_page(&b.doc.scene.borrow(), page, target, after);
    op.is_some_and(|op| b.commit(vec![op]))
}

/// A layer row's eye and lock: shown under the pointer, and otherwise only while they are on.
fn show_keys(row: &gtk::Box, eye: &gtk::Button, lock: &gtk::Button, hover: bool) {
    eye.set_visible(hover || row.has_css_class("hidden"));
    lock.set_visible(hover || lock.has_css_class("on"));
}

/// One page's row: click to open, double click to rename, drag to reorder, its eye to hide or
/// show it, its menu for the rest.
fn page_row(b: &Rc<Board>, page: &str, current: bool, pop: Option<&gtk::Popover>, hidden: bool) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("design-page-row");
    row.set_widget_name(&format!("design-page:{page}"));
    if current {
        row.add_css_class("selected");
    }
    if hidden {
        row.add_css_class("hidden");
    }
    row.append(&glyphs::image("page", 14));
    let name = label(&b.page_name(page), "");
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&name);
    // The eye: always on a hidden page's row, under the pointer on a shown one.
    let eye = gtk::Button::new();
    eye.add_css_class("design-layer-eye");
    eye.add_css_class("flat");
    eye.set_child(Some(&glyphs::image(if hidden { "eye-off" } else { "eye" }, 14)));
    let show = if hidden { "Show this page" } else { "Hide this page" };
    tip(&eye, show);
    eye.update_property(&[gtk::accessible::Property::Label(&format!("{show}: {}", b.page_name(page)))]);
    eye.set_visible(hidden);
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    eye.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            set_page_hidden(&b, &id, !hidden);
        }
    });
    row.append(&eye);
    if !hidden {
        let hover = gtk::EventControllerMotion::new();
        let e = eye.downgrade();
        hover.connect_enter(move |_, _, _| {
            if let Some(e) = e.upgrade() {
                e.set_visible(true);
            }
        });
        let e = eye.downgrade();
        hover.connect_leave(move |_| {
            if let Some(e) = e.upgrade() {
                e.set_visible(false);
            }
        });
        row.add_controller(hover);
    }
    // Dragged onto another page's row: before it on its upper half, after it on its lower.
    let drag = gtk::DragSource::new();
    drag.set_actions(gtk::gdk::DragAction::MOVE);
    let id = page.to_string();
    drag.connect_prepare(move |_, _, _| Some(gtk::gdk::ContentProvider::for_value(&id.to_value())));
    row.add_controller(drag);
    let drop = gtk::DropTarget::new(glib::Type::STRING, gtk::gdk::DragAction::MOVE);
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    drop.connect_drop(move |t, value, _, y| {
        let (Some(b), Ok(dragged)) = (weak.upgrade(), value.get::<String>()) else {
            return false;
        };
        let height = t.widget().map_or(1, |w| w.height()).max(1) as f64;
        move_page(&b, &dragged, &id, y / height >= 0.5)
    });
    row.add_controller(drop);
    // Always there, shown on the page on show: a page switch relights the rows in place.
    let check = crate::icons::image("check", 12);
    check.set_visible(current);
    row.append(&check);
    let click = gtk::GestureClick::new();
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let p = pop.map(|p| p.downgrade());
    let w = row.downgrade();
    click.connect_pressed(move |_, n, x, y| {
        let Some(b) = weak.upgrade() else {
            return;
        };
        // Its eye is pressed, not the row: hiding a page does not open it first.
        if w.upgrade().and_then(|w| w.pick(x, y, gtk::PickFlags::DEFAULT)).is_some_and(|t| t.ancestor(gtk::Button::static_type()).is_some()) {
            return;
        }
        if n == 2 {
            if let Some(w) = w.upgrade() {
                rename_node(&b, &id, Some(w.upcast_ref()));
            }
            return;
        }
        // The dock's list stays open on the page already shown, so a double-click renames it.
        let showing = *b.page.borrow() == id;
        b.set_page(&id);
        if let Some(p) = p.as_ref().and_then(|p| p.upgrade()).filter(|_| !showing) {
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

/// A row of a page's menu titled `title`: it closes the menu, then does `act`.
fn menu_key(b: &Rc<Board>, list: &gtk::Box, pop: &gtk::Popover, title: &str, act: impl Fn(&Rc<Board>) -> bool + 'static) {
    let key = gtk::Button::with_label(title);
    key.add_css_class("design-menu-row");
    let weak = Rc::downgrade(b);
    let p = pop.downgrade();
    key.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        if let Some(b) = weak.upgrade() {
            act(&b);
        }
    });
    list.append(&key);
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
    let hidden = b.doc.scene.borrow().node(page).is_some_and(|n| n.is_hidden());
    for up in [true, false] {
        if let Some(next) = page_neighbour(b, page, up) {
            let id = page.to_string();
            menu_key(b, &list, &pop, if up { "Move up" } else { "Move down" }, move |b| move_page(b, &id, &next, !up));
        }
    }
    let id = page.to_string();
    menu_key(b, &list, &pop, if hidden { "Show page" } else { "Hide page" }, move |b| set_page_hidden(b, &id, !hidden));
    // The dock's pages list this row sits in, if it does: closed once a page is added or gone,
    // rather than left open on rows of pages that changed (TM-69).
    let pages_list = anchor.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>().map(|p| p.downgrade());
    let duplicate = gtk::Button::with_label("Duplicate page");
    duplicate.add_css_class("design-menu-row");
    let weak = Rc::downgrade(b);
    let id = page.to_string();
    let p = pop.downgrade();
    let list_pop = pages_list.clone();
    duplicate.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
        if let Some(b) = weak.upgrade() {
            if duplicate_page(&b, &id) {
                if let Some(l) = list_pop.as_ref().and_then(|l| l.upgrade()) {
                    l.popdown();
                }
            }
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
                if delete_page(&b, &id) {
                    if let Some(l) = pages_list.as_ref().and_then(|l| l.upgrade()) {
                        l.popdown();
                    }
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

/// Delete `page`, going to another first when it is the one on show (which ends a text typed on
/// it). A connector elsewhere attached to what the page holds lets go where it is drawn
/// (`edit::delete`, B26). Said only when it was deleted (B89): refused (the page gone meanwhile,
/// or the last one), the board says why and goes back to the page it showed.
fn delete_page(b: &Rc<Board>, page: &str) -> bool {
    let was = b.page.borrow().clone();
    let next = b.doc.scene.borrow().pages().iter().find(|p| *p != page).cloned();
    if let Some(next) = next.filter(|_| was == page) {
        b.set_page(&next);
    }
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let mut ops = edit::delete(&scene, &placed, &[page.to_string()]);
    // A locked page goes too: the menu asked for it by name.
    if !ops.iter().any(|op| matches!(op, NodeOp::Delete { id } if id == page)) {
        ops.push(NodeOp::Delete { id: page.to_string() });
    }
    drop(scene);
    let deleted = b.commit(ops);
    if deleted {
        b.toast("Page deleted · Ctrl+Z brings it back");
    } else if b.doc.scene.borrow().node(&was).is_some() {
        b.set_page(&was);
    }
    if let Some(l) = b.layers.borrow().as_ref() {
        l.pages_changed();
    }
    deleted
}

/// A copy of `page` and all it holds after it, as stored (named colours and styles kept), named
/// "<name> copy" within the length a name may have (TM-63); the board then shows it.
fn duplicate_page(b: &Rc<Board>, page: &str) -> bool {
    let scene = b.doc.scene.borrow();
    let nodes: Vec<_> = scene.subtree(page).iter().filter_map(|id| scene.stored(id).cloned()).collect();
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
            copy.name = Some(copy_name(&b.page_name(page)));
            ops.push(NodeOp::Add { node: copy, index });
        } else {
            ops.push(NodeOp::Add { node: copy, index: None });
        }
    }
    let new_page = fresh[page].clone();
    let made = b.commit(ops);
    if made {
        b.set_page(&new_page);
        if let Some(l) = b.layers.borrow().as_ref() {
            l.pages_changed();
        }
    }
    made
}

/// The pages popover of the dock: every page, a new one, and this page's canvas colour.
pub fn pages_menu(b: &Rc<Board>, pop: &gtk::Popover) -> gtk::Widget {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.add_css_class("design-pages-menu");
    list.append(&label("Pages", "design-menu-heading"));
    let current = b.page.borrow().clone();
    for row in page_rows(b, Some(pop)) {
        list.append(&row);
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
    canvas.append(&color_key(b, Some(fill.as_deref().unwrap_or(super::paint::CANVAS)), "Canvas colour", Some("Automatic"), move |c| {
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
    let shown = if n.kind == NodeKind::Page { b.page_name(id) } else { display_name(&n) };
    entry.set_text(&shown);
    entry.set_width_chars(24);
    pop.set_child(Some(&entry));
    // Enter, or a click elsewhere that closes it, names the node, as leaving the inspector's
    // name field does (B88); Esc closes it with the name as it was.
    let cancel = Rc::new(Cell::new(false));
    let keys = gtk::EventControllerKey::new();
    let c = cancel.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            c.set(true);
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(keys);
    let p = pop.downgrade();
    entry.connect_activate(move |_| {
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
    });
    let weak = Rc::downgrade(b);
    let id = id.to_string();
    let e = entry.downgrade();
    pop.connect_closed(move |p| {
        let set = e.upgrade().filter(|_| !cancel.replace(true)).and_then(|e| rename_set(&e.text(), &shown));
        let (p, weak, id) = (p.clone(), weak.clone(), id.clone());
        // After the popover has closed: the rename can rebuild the row it hangs from.
        glib::idle_add_local_once(move || {
            if let (Some(b), Some(name)) = (weak.upgrade(), set) {
                let mut set = Map::new();
                set.insert("name".into(), name);
                b.commit(vec![NodeOp::Update { id, set }]);
                let layers = b.layers.borrow().clone();
                if let Some(l) = layers {
                    l.pages_changed();
                }
            }
            p.unparent();
        });
    });
    pop.popup();
    entry.grab_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deep_nest_keeps_its_indent_in_the_panel() {
        assert_eq!(indent_px(0), 0);
        assert_eq!(indent_px(1), 14);
        assert_eq!(indent_px(6), 84);
        assert_eq!(indent_px(7), 88);
        assert_eq!(indent_px(12), 108);
        assert_eq!(indent_px(40), 108);
    }

    #[test]
    fn a_rename_writes_only_a_change() {
        assert_eq!(rename_set("A", "A"), None);
        assert_eq!(rename_set(" A ", "A"), None);
        assert_eq!(rename_set("", "A"), Some(Value::Null));
        assert_eq!(rename_set("   ", "A"), Some(Value::Null));
        assert_eq!(rename_set(" B ", "A"), Some(json!("B")));
    }
}
