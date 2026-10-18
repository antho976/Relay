//! The project's designs: a grid of cards with a picture of each design's first page, which
//! terminal it belongs to and when it last changed. Chips filter it to one terminal (an
//! agent's designs), to the project's own, or to the deleted ones, which can come back. A
//! delete says so with an Undo rather than asking first.

use super::paint::Painter;
use crate::app::{label, rows, text, Ui};
use crate::relative::{ago, Form};
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::ops::design::GetOut;
use relay_bus::types::Design;
use relay_client::design::{layout, Camera, Rect, Scene};
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

type Opener = Rc<dyn Fn(i64)>;

/// A card's picture, in pixels at 2× for a crisp 280 by 175 thumbnail.
const THUMB_W: f64 = 560.0;
const THUMB_H: f64 = 350.0;

/// Marks a menu caption as the current choice (drawn as a check).
const CHECKED: &str = "\u{0}checked";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    All,
    Terminal(String),
    Project,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    Recent,
    Name,
    Created,
}

pub struct Gallery {
    pub root: gtk::Overlay,
    ui: Weak<Ui>,
    pub project: Cell<i64>,
    pub filter: RefCell<Filter>,
    sort: Cell<Sort>,
    search: gtk::SearchEntry,
    chips: gtk::Box,
    grid: gtk::FlowBox,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_body: gtk::Label,
    count: gtk::Label,
    designs: RefCell<Vec<Design>>,
    sessions: RefCell<HashMap<String, String>>,
    loading: Cell<bool>,
    dirty: Cell<bool>,
    /// Open a design on the board.
    pub open: RefCell<Option<Opener>>,
    pictures: RefCell<HashMap<i64, gtk::Picture>>,
    queue: RefCell<VecDeque<(i64, i64)>>,
    rendering: Cell<bool>,
    painter: Rc<Painter>,
    toast: gtk::Revealer,
    toast_label: gtk::Label,
    toast_undo: gtk::Button,
    undo: RefCell<Option<Rc<dyn Fn()>>>,
    toast_serial: Cell<u64>,
    /// The width each card has: the page's width shared by as many 260 px columns as fit.
    card_width: Cell<i32>,
    me: Weak<Gallery>,
}

fn thumbs_dir() -> PathBuf {
    glib::user_cache_dir().join("relay-v4").join("design-thumbs")
}

fn thumb_path(id: i64, rev: i64) -> PathBuf {
    thumbs_dir().join(format!("{id}-{rev}.png"))
}

impl Gallery {
    pub fn new(ui: &Rc<Ui>) -> Rc<Gallery> {
        let root = gtk::Overlay::new();
        root.add_css_class("design-gallery");
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("design-gallery-page");
        let scroll = gtk::ScrolledWindow::new();
        // External, not Never: the cards are sized from the page's width, and Never made that
        // width the page's minimum, so the window could not be made narrower while the gallery
        // showed (at 1440px it held the window to 1432).
        scroll.set_hscrollbar_policy(gtk::PolicyType::External);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&page));
        root.set_child(Some(&scroll));

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.add_css_class("design-gallery-head");
        let title_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        title_box.set_hexpand(true);
        title_box.set_valign(gtk::Align::Center);
        title_box.append(&label("Designs", "design-gallery-title"));
        let count = label("", "design-gallery-count");
        count.set_valign(gtk::Align::Baseline);
        title_box.append(&count);
        head.append(&title_box);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search designs"));
        search.set_width_chars(22);
        search.set_valign(gtk::Align::Center);
        head.append(&search);
        let sort = gtk::DropDown::from_strings(&["Last edited", "Name", "Created"]);
        sort.add_css_class("design-dropdown");
        sort.set_tooltip_text(Some("Sort"));
        sort.set_valign(gtk::Align::Center);
        head.append(&sort);
        let new = gtk::Button::new();
        new.add_css_class("primary");
        new.set_widget_name("design-new");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&crate::icons::image("plus", 14));
        row.append(&label("New design", ""));
        new.set_child(Some(&row));
        new.set_valign(gtk::Align::Center);
        head.append(&new);
        page.append(&head);

        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chips.add_css_class("design-chips");
        page.append(&chips);

        let grid = gtk::FlowBox::new();
        grid.add_css_class("design-grid");
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        grid.set_min_children_per_line(4);
        grid.set_max_children_per_line(4);
        grid.set_column_spacing(16);
        grid.set_row_spacing(20);
        grid.set_valign(gtk::Align::Start);
        page.append(&grid);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 10);
        empty.add_css_class("design-empty");
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Start);
        let glyph = super::glyphs::image("frame", 28);
        glyph.add_css_class("faint");
        empty.append(&glyph);
        let empty_title = label("No designs yet", "title");
        empty_title.set_xalign(0.5);
        empty.append(&empty_title);
        let empty_body = label("", "dim");
        empty_body.set_wrap(true);
        empty_body.set_justify(gtk::Justification::Center);
        empty_body.set_xalign(0.5);
        empty.append(&empty_body);
        let empty_new = gtk::Button::with_label("New design");
        empty_new.add_css_class("primary");
        empty_new.set_halign(gtk::Align::Center);
        empty_new.set_margin_top(8);
        empty.append(&empty_new);
        empty.set_visible(false);
        page.append(&empty);

        let toast = gtk::Revealer::new();
        toast.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        toast.set_halign(gtk::Align::Center);
        toast.set_valign(gtk::Align::End);
        toast.set_margin_bottom(20);
        let toast_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        toast_row.add_css_class("design-toast");
        let toast_label = label("", "");
        toast_row.append(&toast_label);
        let toast_undo = gtk::Button::with_label("Undo");
        toast_undo.add_css_class("design-toast-undo");
        toast_row.append(&toast_undo);
        toast.set_child(Some(&toast_row));
        root.add_overlay(&toast);

        let painter = Painter::new(&root);
        let gallery = Rc::new_cyclic(|me| Gallery {
            root,
            ui: Rc::downgrade(ui),
            project: Cell::new(0),
            filter: RefCell::new(Filter::All),
            sort: Cell::new(Sort::Recent),
            search,
            chips,
            grid,
            empty,
            empty_title,
            empty_body,
            count,
            designs: RefCell::default(),
            sessions: RefCell::default(),
            loading: Cell::new(false),
            dirty: Cell::new(false),
            open: RefCell::default(),
            pictures: RefCell::default(),
            queue: RefCell::default(),
            rendering: Cell::new(false),
            painter,
            toast,
            toast_label,
            toast_undo,
            undo: RefCell::default(),
            toast_serial: Cell::new(0),
            card_width: Cell::new(280),
            me: me.clone(),
        });
        // Columns follow the page's width; the cards share it, their pictures 16:10.
        let weak = Rc::downgrade(&gallery);
        scroll.hadjustment().connect_page_size_notify(move |adj| {
            if let Some(g) = weak.upgrade() {
                g.fit_columns(adj.page_size());
            }
        });
        let weak = Rc::downgrade(&gallery);
        gallery.root.connect_map(move |_| {
            if let Some(g) = weak.upgrade() {
                g.next_thumb();
            }
        });
        for key in [&new, &empty_new] {
            let weak = Rc::downgrade(&gallery);
            key.connect_clicked(move |_| {
                if let Some(g) = weak.upgrade() {
                    g.create();
                }
            });
        }
        let weak = Rc::downgrade(&gallery);
        gallery.search.connect_search_changed(move |_| {
            if let Some(g) = weak.upgrade() {
                g.render();
            }
        });
        let weak = Rc::downgrade(&gallery);
        sort.connect_selected_notify(move |dd| {
            if let Some(g) = weak.upgrade() {
                g.sort.set(match dd.selected() {
                    1 => Sort::Name,
                    2 => Sort::Created,
                    _ => Sort::Recent,
                });
                g.render();
            }
        });
        let weak = Rc::downgrade(&gallery);
        gallery.toast_undo.connect_clicked(move |_| {
            if let Some(g) = weak.upgrade() {
                g.toast.set_reveal_child(false);
                if let Some(undo) = g.undo.borrow_mut().take() {
                    undo();
                }
            }
        });
        gallery
    }

    fn me(&self) -> Rc<Gallery> {
        self.me.upgrade().expect("gallery alive while called")
    }

    fn fit_columns(&self, width: f64) {
        const MIN: f64 = 260.0;
        const GAP: f64 = 16.0;
        let inner = (width - 56.0).max(MIN);
        let columns = ((inner + GAP) / (MIN + GAP)).floor().clamp(1.0, 6.0);
        let card = ((inner - GAP * (columns - 1.0)) / columns).floor() as i32 - 1;
        if card == self.card_width.get() && self.grid.max_children_per_line() == columns as u32 {
            return;
        }
        self.card_width.set(card);
        self.grid.set_min_children_per_line(columns as u32);
        self.grid.set_max_children_per_line(columns as u32);
        for p in self.pictures.borrow().values() {
            p.set_size_request(card, (card as f64 / 1.6).round() as i32);
        }
    }

    pub fn live_count(&self) -> usize {
        self.designs.borrow().iter().filter(|d| d.deleted_at.is_none()).count()
    }

    /// The first card shown, for the smoke capture.
    pub fn first(&self) -> Option<i64> {
        self.visible().first().map(|d| d.id)
    }

    /// Read the project's designs again (coalesced: one read in flight at a time).
    pub fn refresh(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.dirty.set(true);
        if self.loading.replace(true) {
            return;
        }
        let g = self.me();
        glib::spawn_future_local(async move {
            while g.dirty.replace(false) {
                let project = g.project.get();
                if project <= 0 {
                    break;
                }
                let listed = ui.call("design.list", json!({"project_id": project, "include_deleted": true})).await;
                let sessions = ui.call("session.list", json!({"project_id": project, "include_closed": true})).await;
                if project != g.project.get() {
                    g.dirty.set(true);
                    continue;
                }
                match listed {
                    Ok(v) => {
                        *g.designs.borrow_mut() = serde_json::from_value(v["designs"].clone()).unwrap_or_default();
                    }
                    Err(e) => {
                        g.empty_title.set_text("Designs could not be read");
                        g.empty_body.set_text(&e.to_string());
                    }
                }
                if let Ok(v) = sessions {
                    *g.sessions.borrow_mut() = rows(&v, "sessions")
                        .iter()
                        .map(|s| (text(s, "name").to_string(), text(s, "state").to_string()))
                        .collect();
                }
                g.render();
            }
            g.loading.set(false);
        });
    }

    fn visible(&self) -> Vec<Design> {
        let filter = self.filter.borrow().clone();
        let q = self.search.text().to_lowercase();
        let mut list: Vec<Design> = self
            .designs
            .borrow()
            .iter()
            .filter(|d| match &filter {
                Filter::Deleted => d.deleted_at.is_some(),
                _ if d.deleted_at.is_some() => false,
                Filter::All => true,
                Filter::Terminal(s) => d.session.as_deref() == Some(s.as_str()),
                Filter::Project => d.session.is_none(),
            })
            .filter(|d| q.is_empty() || d.title.to_lowercase().contains(&q) || d.session.as_deref().is_some_and(|s| s.contains(&q)))
            .cloned()
            .collect();
        match self.sort.get() {
            Sort::Recent => list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
            Sort::Name => list.sort_by_key(|a| a.title.to_lowercase()),
            Sort::Created => list.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        }
        list
    }

    fn render(&self) {
        let designs = self.designs.borrow().clone();
        let live: Vec<&Design> = designs.iter().filter(|d| d.deleted_at.is_none()).collect();
        let deleted = designs.len() - live.len();
        self.count.set_text(&if live.is_empty() { String::new() } else { live.len().to_string() });
        // Chips: all, each terminal with designs, the project's own, deleted.
        while let Some(c) = self.chips.first_child() {
            self.chips.remove(&c);
        }
        let mut terminals: Vec<(String, usize)> = Vec::new();
        for d in &live {
            if let Some(s) = &d.session {
                match terminals.iter_mut().find(|(n, _)| n == s) {
                    Some(t) => t.1 += 1,
                    None => terminals.push((s.clone(), 1)),
                }
            }
        }
        terminals.sort();
        let project_only = live.iter().filter(|d| d.session.is_none()).count();
        let current = self.filter.borrow().clone();
        // A filter whose designs are all gone falls back to all.
        let current = match &current {
            Filter::Terminal(s) if !terminals.iter().any(|(n, _)| n == s) && !self.sessions.borrow().contains_key(s) => Filter::All,
            Filter::Deleted if deleted == 0 => Filter::All,
            _ => current,
        };
        *self.filter.borrow_mut() = current.clone();
        // Nothing to filter yet: no chips at all.
        self.chips.set_visible(!designs.is_empty());
        self.chip("All", live.len(), None, Filter::All, &current);
        if let Filter::Terminal(s) = &current {
            if !terminals.iter().any(|(n, _)| n == s) {
                terminals.push((s.clone(), 0));
            }
        }
        for (name, n) in &terminals {
            let state = self.sessions.borrow().get(name).cloned().unwrap_or_else(|| "closed".into());
            self.chip(name, *n, Some(&state), Filter::Terminal(name.clone()), &current);
        }
        if project_only > 0 && !terminals.is_empty() {
            self.chip("Project only", project_only, None, Filter::Project, &current);
        }
        if deleted > 0 {
            let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            spacer.set_hexpand(true);
            self.chips.append(&spacer);
            self.chip("Deleted", deleted, None, Filter::Deleted, &current);
        }
        // Cards.
        while let Some(c) = self.grid.first_child() {
            self.grid.remove(&c);
        }
        self.pictures.borrow_mut().clear();
        let list = self.visible();
        for d in &list {
            self.grid.insert(&self.card(d), -1);
        }
        let none = list.is_empty();
        self.grid.set_visible(!none);
        self.empty.set_visible(none);
        if none {
            let (title, body) = match &current {
                Filter::Deleted => ("Nothing deleted", "Deleted designs wait here until you delete them for good.".to_string()),
                Filter::Terminal(s) => (
                    "No designs from this terminal",
                    format!("Ask {s} to sketch something — “design the settings screen” — and it appears here as it draws.\nOr start one yourself: it belongs to {s}."),
                ),
                _ if !self.search.text().is_empty() => ("No design matches", "Try another word, or clear the search.".to_string()),
                _ => (
                    "No designs yet",
                    "Sketch screens, flows and ideas on an infinite canvas: frames, text, shapes, stickies, pictures, auto layout.\nAgents in this project make and change designs too — ask one, and watch it draw.".to_string(),
                ),
            };
            self.empty_title.set_text(title);
            self.empty_body.set_text(&body);
        }
        self.queue_thumbs(&list);
    }

    fn chip(&self, caption: &str, count: usize, state: Option<&str>, filter: Filter, current: &Filter) {
        let chip = gtk::Button::new();
        chip.add_css_class("design-chip");
        if *current == filter {
            chip.add_css_class("selected");
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if let Some(state) = state {
            let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            lamp.add_css_class("design-lamp");
            lamp.add_css_class(state);
            lamp.set_valign(gtk::Align::Center);
            row.append(&lamp);
        }
        row.append(&label(caption, ""));
        row.append(&label(&count.to_string(), "design-chip-count"));
        chip.set_child(Some(&row));
        if state.is_some() {
            chip.set_tooltip_text(Some(&format!("Designs of {caption}'s terminal")));
        }
        let weak = self.me.clone();
        chip.connect_clicked(move |_| {
            if let Some(g) = weak.upgrade() {
                *g.filter.borrow_mut() = filter.clone();
                g.render();
            }
        });
        self.chips.append(&chip);
    }

    fn card(&self, d: &Design) -> gtk::Widget {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("design-card");
        card.set_widget_name(&format!("design-card-{}", d.id));
        let frame = gtk::Overlay::new();
        let picture = gtk::Picture::new();
        picture.add_css_class("design-thumb");
        picture.set_content_fit(gtk::ContentFit::Cover);
        let w = self.card_width.get();
        picture.set_size_request(w, (w as f64 / 1.6).round() as i32);
        picture.set_can_shrink(true);
        frame.set_child(Some(&picture));
        self.pictures.borrow_mut().insert(d.id, picture.clone());
        let deleted = d.deleted_at.is_some();
        let more = gtk::MenuButton::new();
        more.add_css_class("design-card-more");
        more.set_child(Some(&crate::icons::image("more", 16)));
        more.set_tooltip_text(Some("Design actions"));
        more.set_halign(gtk::Align::End);
        more.set_valign(gtk::Align::Start);
        more.set_margin_top(8);
        more.set_margin_end(8);
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        more.set_popover(Some(&pop));
        let weak = self.me.clone();
        let design = d.clone();
        pop.connect_show(move |pop| {
            if let Some(g) = weak.upgrade() {
                pop.set_child(Some(&g.card_menu(&design, pop)));
            }
        });
        frame.add_overlay(&more);
        card.append(&frame);
        let title = label(&d.title, "design-card-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        card.append(&title);
        let meta = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        meta.add_css_class("design-card-meta");
        match &d.session {
            Some(s) => {
                let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                lamp.add_css_class("design-lamp");
                lamp.add_css_class(&self.sessions.borrow().get(s).cloned().unwrap_or_else(|| "closed".into()));
                lamp.set_valign(gtk::Align::Center);
                meta.append(&lamp);
                meta.append(&label(s, "design-card-terminal"));
            }
            None => meta.append(&label("Project", "design-card-terminal")),
        }
        let when = if deleted {
            format!("deleted {}", d.deleted_at.as_deref().and_then(|t| ago(t, Form::CompactAgo)).unwrap_or_default())
        } else {
            let by = if d.updated_by == "user" { String::new() } else { format!(" by {}", d.updated_by) };
            format!("{}{by}", ago(&d.updated_at, Form::CompactAgo).unwrap_or_default())
        };
        let detail = label(&format!("· {when} · {} {}", d.pages, if d.pages == 1 { "page" } else { "pages" }), "design-card-when");
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.append(&detail);
        card.append(&meta);
        if deleted {
            card.add_css_class("deleted");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let restore = gtk::Button::with_label("Restore");
            let forever = gtk::Button::with_label("Delete forever");
            forever.add_css_class("destructive-quiet");
            let weak = self.me.clone();
            let id = d.id;
            restore.connect_clicked(move |_| {
                if let Some(g) = weak.upgrade() {
                    g.mutate("design.restore", id, None);
                }
            });
            let weak = self.me.clone();
            let title = d.title.clone();
            forever.connect_clicked(move |key| {
                if let Some(g) = weak.upgrade() {
                    g.confirm_purge(key.upcast_ref(), id, &title);
                }
            });
            row.append(&restore);
            row.append(&forever);
            card.append(&row);
        } else {
            let click = gtk::GestureClick::new();
            let weak = self.me.clone();
            let id = d.id;
            click.connect_released(move |g, _, x, y| {
                // The ⋯ key is its own click.
                if let Some(w) = g.widget() {
                    if let Some(picked) = w.pick(x, y, gtk::PickFlags::DEFAULT) {
                        if picked.ancestor(gtk::MenuButton::static_type()).is_some() {
                            return;
                        }
                    }
                }
                if let Some(gal) = weak.upgrade() {
                    if let Some(open) = gal.open.borrow().clone() {
                        open(id);
                    }
                }
            });
            card.add_controller(click);
            card.set_cursor_from_name(Some("pointer"));
        }
        let child = gtk::FlowBoxChild::new();
        child.set_child(Some(&card));
        child.set_focusable(false);
        child.upcast()
    }

    fn card_menu(&self, d: &Design, pop: &gtk::Popover) -> gtk::Widget {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let item = |caption: &str, f: Rc<dyn Fn()>| {
            let b = gtk::Button::new();
            b.add_css_class("design-menu-row");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let (caption, checked) = match caption.strip_suffix(CHECKED) {
                Some(c) => (c, true),
                None => (caption, false),
            };
            let name = label(caption, "");
            name.set_hexpand(true);
            row.append(&name);
            if checked {
                row.append(&crate::icons::image("check", 14));
            }
            b.set_child(Some(&row));
            let p = pop.downgrade();
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                f();
            });
            list.append(&b);
            b
        };
        let id = d.id;
        if d.deleted_at.is_none() {
            let weak = self.me.clone();
            item("Open", Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    if let Some(open) = g.open.borrow().clone() {
                        open(id);
                    }
                }
            }));
            let weak = self.me.clone();
            let project = d.project_id;
            item("Open in a new window", Rc::new(move || {
                if let Some(ui) = weak.upgrade().and_then(|g| g.ui.upgrade()) {
                    super::window::open(&ui, project, id);
                }
            }));
            let weak = self.me.clone();
            let title = d.title.clone();
            let anchor = pop.parent();
            item("Rename…", Rc::new(move || {
                if let (Some(g), Some(a)) = (weak.upgrade(), anchor.clone()) {
                    g.rename(&a, id, &title);
                }
            }));
            let weak = self.me.clone();
            item("Duplicate", Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    g.mutate("design.duplicate", id, None);
                }
            }));
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            list.append(&label("Belongs to", "design-menu-heading"));
            let mut names: Vec<(String, String)> = self.sessions.borrow().iter().map(|(n, s)| (n.clone(), s.clone())).collect();
            names.sort();
            names.retain(|(n, s)| s != "closed" || d.session.as_deref() == Some(n.as_str()));
            for (name, _) in names {
                let mark = if d.session.as_deref() == Some(name.as_str()) { CHECKED } else { "" };
                let weak = self.me.clone();
                let n = name.clone();
                item(&format!("{name}{mark}"), Rc::new(move || {
                    if let Some(g) = weak.upgrade() {
                        g.mutate("design.update", id, Some(json!({"session": n})));
                    }
                }));
            }
            let weak = self.me.clone();
            item(&format!("The project only{}", if d.session.is_none() { CHECKED } else { "" }), Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    g.mutate("design.update", id, Some(json!({"session": null})));
                }
            }));
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let weak = self.me.clone();
            let title = d.title.clone();
            let del = item("Delete", Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    g.delete(id, &title);
                }
            }));
            del.add_css_class("destructive");
        }
        list.upcast()
    }

    /// A mutation of one design, then a fresh list.
    fn mutate(&self, op: &str, id: i64, extra: Option<serde_json::Value>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let mut payload = json!({"design_id": id});
        if let (Some(serde_json::Value::Object(extra)), Some(map)) = (extra, payload.as_object_mut()) {
            map.extend(extra);
        }
        let op = op.to_string();
        let g = self.me();
        glib::spawn_future_local(async move {
            match ui.call(&op, payload).await {
                Ok(v) => {
                    if op == "design.duplicate" {
                        if let Some(new) = v["id"].as_i64() {
                            g.toast(&format!("Duplicated as “{}”", text(&v, "title")), None);
                            let _ = new;
                        }
                    }
                    g.refresh();
                }
                Err(e) => g.toast(&e.to_string(), None),
            }
        });
    }

    pub fn delete(&self, id: i64, title: &str) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let g = self.me();
        let title = title.to_string();
        glib::spawn_future_local(async move {
            match ui.call("design.delete", json!({"design_id": id})).await {
                Ok(_) => {
                    super::window::close(id);
                    let weak = Rc::downgrade(&g);
                    g.toast(
                        &format!("Deleted “{title}”"),
                        Some(Rc::new(move || {
                            if let Some(g) = weak.upgrade() {
                                g.mutate("design.restore", id, None);
                            }
                        })),
                    );
                    g.refresh();
                }
                Err(e) => g.toast(&e.to_string(), None),
            }
        });
    }

    /// Deleting for good is the one step that cannot come back, so it asks.
    fn confirm_purge(&self, anchor: &gtk::Widget, id: i64, title: &str) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(anchor);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let question = label(&format!("Delete “{title}” for good? Its pages and layers cannot be brought back."), "");
        question.set_wrap(true);
        question.set_max_width_chars(34);
        body.append(&question);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_halign(gtk::Align::End);
        let keep = gtk::Button::with_label("Keep it");
        let go = gtk::Button::with_label("Delete forever");
        go.add_css_class("destructive");
        row.append(&keep);
        row.append(&go);
        body.append(&row);
        pop.set_child(Some(&body));
        let p = pop.downgrade();
        keep.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
        });
        let p = pop.downgrade();
        let weak = self.me.clone();
        go.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(g) = weak.upgrade() {
                g.mutate("design.purge", id, None);
            }
        });
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    }

    fn rename(&self, anchor: &gtk::Widget, id: i64, title: &str) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(anchor);
        let entry = gtk::Entry::new();
        entry.set_text(title);
        entry.set_width_chars(28);
        pop.set_child(Some(&entry));
        let weak = self.me.clone();
        let p = pop.downgrade();
        let original = title.to_string();
        entry.connect_activate(move |e| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            let t = e.text().trim().to_string();
            if let Some(g) = weak.upgrade() {
                if !t.is_empty() && t != original {
                    g.mutate("design.update", id, Some(json!({"title": t})));
                }
            }
        });
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
        entry.grab_focus();
    }

    /// A new design, tied to the terminal the gallery shows, and opened.
    pub fn create(&self) {
        let session = match &*self.filter.borrow() {
            Filter::Terminal(s) => Some(s.clone()),
            _ => None,
        };
        super::create(&self.ui, self.project.get(), session);
    }

    pub fn toast(&self, text: &str, undo: Option<Rc<dyn Fn()>>) {
        self.toast_label.set_text(text);
        self.toast_undo.set_visible(undo.is_some());
        *self.undo.borrow_mut() = undo;
        self.toast.set_reveal_child(true);
        let serial = self.toast_serial.get() + 1;
        self.toast_serial.set(serial);
        let weak = self.me.clone();
        glib::timeout_add_local_once(std::time::Duration::from_secs(7), move || {
            if let Some(g) = weak.upgrade().filter(|g| g.toast_serial.get() == serial) {
                g.toast.set_reveal_child(false);
                g.undo.borrow_mut().take();
            }
        });
    }

    // ------------------------------------------------------------------ thumbnails

    fn queue_thumbs(&self, list: &[Design]) {
        let mut queue = self.queue.borrow_mut();
        queue.clear();
        for d in list {
            let path = thumb_path(d.id, d.rev);
            if path.exists() {
                if let Some(p) = self.pictures.borrow().get(&d.id) {
                    load_picture(p, path);
                }
            } else {
                queue.push_back((d.id, d.rev));
            }
        }
        drop(queue);
        self.next_thumb();
    }

    /// Draw the next missing thumbnail: the design's first page, fitted.
    fn next_thumb(&self) {
        // Drawing needs the window's renderer: wait until the gallery is on screen.
        if self.rendering.get() || self.root.native().and_then(|n| n.renderer()).is_none() {
            return;
        }
        let Some((id, rev)) = self.queue.borrow_mut().pop_front() else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.rendering.set(true);
        let g = self.me();
        glib::spawn_future_local(async move {
            let got = ui.call("design.get", json!({"design_id": id})).await;
            if let Ok(got) = got.map_err(|e| e.to_string()).and_then(|v| serde_json::from_value::<GetOut>(v).map_err(|e| e.to_string())) {
                *g.painter.assets_dir.borrow_mut() = got.assets_dir.clone();
                let rev = got.design.rev.max(rev);
                let scene = Scene::new(got.nodes);
                // Pictures decode off the main thread: wait for them (a few seconds at most), or
                // the thumbnail would keep their grey placeholders until the design changes.
                for _ in 0..100 {
                    if !g.painter.loading(&scene) {
                        break;
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                if let Some(texture) = g.draw_thumb(scene) {
                    let path = thumb_path(id, rev);
                    let rt = ui.rt.clone();
                    let p = path.clone();
                    let saved = rt
                        .spawn_blocking(move || {
                            let dir = thumbs_dir();
                            std::fs::create_dir_all(&dir)?;
                            // Older pictures of the same design are stale now.
                            if let Ok(entries) = std::fs::read_dir(&dir) {
                                for e in entries.flatten() {
                                    let name = e.file_name().to_string_lossy().to_string();
                                    if name.starts_with(&format!("{id}-")) {
                                        let _ = std::fs::remove_file(e.path());
                                    }
                                }
                            }
                            std::fs::write(&p, texture.save_to_png_bytes())
                        })
                        .await;
                    if matches!(saved, Ok(Ok(()))) {
                        if let Some(p) = g.pictures.borrow().get(&id) {
                            load_picture(p, path);
                        }
                    }
                }
            }
            g.rendering.set(false);
            // Low priority: the gallery stays responsive while the rest draw.
            let weak = Rc::downgrade(&g);
            glib::idle_add_local_full(glib::Priority::LOW, move || {
                if let Some(g) = weak.upgrade() {
                    g.next_thumb();
                }
                glib::ControlFlow::Break
            });
        });
    }

    fn draw_thumb(&self, scene: Scene) -> Option<gdk::Texture> {
        let page = scene.pages().first()?.clone();
        let placed = layout::place(&scene, &page, &*self.painter);
        let bounds = Rect::bounds(scene.children(&page).iter().filter_map(|id| placed.rects.get(id)));
        let canvas = scene.node(&page).and_then(super::paint::fill_of).unwrap_or_else(|| super::paint::CANVAS.into());
        let snap = gtk::Snapshot::new();
        snap.append_color(&super::paint::rgba(&canvas), &graphene::Rect::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32));
        if let Some(bounds) = bounds {
            let cam = Camera::fit(&bounds, THUMB_W, THUMB_H, 28.0, 2.0);
            snap.save();
            snap.scale(cam.zoom as f32, cam.zoom as f32);
            snap.translate(&graphene::Point::new(-cam.x as f32, -cam.y as f32));
            self.painter.paint_page(&snap, &scene, &placed, &page, None);
            snap.restore();
        }
        let node = snap.to_node()?;
        let renderer = self.root.native()?.renderer()?;
        Some(renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32))))
    }
}

/// Load a cached thumbnail off the main thread.
fn load_picture(picture: &gtk::Picture, path: PathBuf) {
    let weak = picture.downgrade();
    glib::spawn_future_local(async move {
        let texture = gtk::gio::spawn_blocking(move || gdk::Texture::from_filename(&path).ok()).await.ok().flatten();
        if let (Some(p), Some(t)) = (weak.upgrade(), texture) {
            p.set_paintable(Some(&t));
        }
    });
}

