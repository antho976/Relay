//! Stock photos on the canvas (native-pictures; pictures-SPEC-v2 §13): the picker in the image key
//! and the credit in the inspector.
//!
//! The image key's popover searches free photos (`design.picture.search`: Openverse, or Pexels
//! with the person's key) and places the one chosen (`design.picture.add`) as `add_images` places
//! a file, one undo step, with its credit kept in `doc.credits` for the inspector. Every call goes
//! out on a connection of its own (`client::is_lifecycle_request`), so a slow provider never holds
//! the board's edits; nothing here waits on the main loop.
//!
//! A search runs on Enter only, to spare the anonymous budget. Its thumbnails come through a
//! queue of at most four calls ([`Queue`]); a newer search or closing the popover empties it, and
//! what an older search's calls bring back is dropped.

use super::board::Board;
use crate::app::{label, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk::{gdk, gio, pango};
use gtk4 as gtk;
use relay_bus::design::{Layout, Node, NodeKind, NodeOp, Sizing};
use relay_bus::ops::design::AssetAddOut;
use relay_bus::ops::picture::{AddOut, Orientation, PictureHit, PictureSource, SearchOut, ThumbOut, PLUGIN};
use relay_client::design::{names, Scene};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

/// Results a search asks for: six lines of three.
const PER_SEARCH: u32 = 18;
/// Thumbnails asked for at once (§13.4).
const IN_FLIGHT: usize = 4;
/// A cell's picture, in px.
const THUMB: (i32, i32) = (160, 108);
/// Between a photo and the caption under it, and around both when they are wrapped (§13.3).
const CAPTION_GAP: f64 = 6.0;
const CAPTION_SIZE: f64 = 11.0;
const CAPTION_COLOR: &str = "#6B7280";

const HINT: &str = "Two or three words, like sunlit kitchen or mountain lake.";
const NONE_MATCH: &str = "No photos match. Try fewer words.";
const OFF: &str = "Stock photos come from the Pictures plugin, which is off for this project.";

const SOURCES: [(PictureSource, &str); 2] = [(PictureSource::Openverse, "Openverse"), (PictureSource::Pexels, "Pexels")];
const SHAPES: [(Option<Orientation>, &str); 4] =
    [(None, "Any shape"), (Some(Orientation::Landscape), "Landscape"), (Some(Orientation::Portrait), "Portrait"), (Some(Orientation::Square), "Square")];

/// The last search, which reopening the popover shows again (§13.1).
#[derive(Debug, Clone)]
pub(super) struct Last {
    pub query: String,
    pub source: PictureSource,
    pub orientation: Option<Orientation>,
}

impl Default for Last {
    fn default() -> Last {
        Last { query: String::new(), source: PictureSource::Openverse, orientation: None }
    }
}

/// What the board keeps of the picker: its last search, and the picker while the popover shows.
#[derive(Default)]
pub(super) struct State {
    pub last: Last,
    open: Option<Rc<Picker>>,
}

/// The thumbnail queue of one search (§13.4): its cells in grid order, at most [`IN_FLIGHT`]
/// asked at once. It belongs to one search number; what comes back for another is dropped.
#[derive(Debug, Default)]
struct Queue {
    search: u64,
    waiting: VecDeque<usize>,
    flying: usize,
}

impl Queue {
    /// A new search's grid: what waited for the last one is dropped, and so is what its calls
    /// still out bring back.
    fn start(&mut self, search: u64) {
        self.search = search;
        self.waiting.clear();
        self.flying = 0;
    }

    /// The cells a page of results added, after those already there.
    fn extend(&mut self, cells: std::ops::Range<usize>) {
        self.waiting.extend(cells);
    }

    /// The next cell to ask for, and the search it is for, while fewer than [`IN_FLIGHT`] are out.
    fn next(&mut self) -> Option<(u64, usize)> {
        if self.flying >= IN_FLIGHT {
            return None;
        }
        let cell = self.waiting.pop_front()?;
        self.flying += 1;
        Some((self.search, cell))
    }

    /// A call for `search` came back: whether what it brought is shown (it is this search's).
    fn done(&mut self, search: u64) -> bool {
        if search != self.search {
            return false;
        }
        self.flying = self.flying.saturating_sub(1);
        true
    }

    /// The popover closed: nothing more is asked, and nothing that comes back is shown.
    fn stop(&mut self) {
        self.start(self.search + 1);
    }
}

/// What the picker shows for a refusal (§13.2, step 4).
#[derive(Debug, PartialEq)]
enum Said {
    /// A line under the search row.
    Line(String),
    /// The key row opens with this note.
    Key(String),
    /// Pictures is off for the design's project.
    Off,
}

fn source_name(source: &str) -> &'static str {
    match source {
        "pexels" => "Pexels",
        _ => "Openverse",
    }
}

fn said(e: &Error) -> Said {
    let Error::Bus(e) = e else {
        return Said::Line(e.to_string());
    };
    let detail = |key: &str| e.details.as_ref().map(|d| d[key].clone()).unwrap_or(Value::Null);
    match e.code.as_str() {
        "design.picture.rate_limited" => {
            let source = detail("source").as_str().map_or("The photo source", source_name);
            let wait = detail("retry_after_s").as_u64().unwrap_or(60);
            Said::Line(format!("{source} needs a pause: try again in {wait} s."))
        }
        "design.picture.offline" => Said::Line("No connection. Photos already placed still work.".into()),
        "design.picture.gone" => Said::Line("That photo is no longer available. Pick another.".into()),
        "design.picture.no_key" => Said::Key("Pexels needs an API key (free at pexels.com/api). It stays in this computer's keyring.".into()),
        "design.picture.key_rejected" => Said::Key("Pexels did not accept the key. Save a new one, or search Openverse.".into()),
        "design.picture.key_format" => Said::Key("That is not a whole key: paste it again, with no spaces.".into()),
        "design.picture.keyring" => Said::Key("The keyring could not be reached. Unlock it (a prompt may be waiting) and try again.".into()),
        "plugin.off" => Said::Off,
        _ => Said::Line(e.message.clone()),
    }
}

/// A cell's second line: "creator · licence", or the licence alone.
fn byline(hit: &PictureHit) -> String {
    match hit.creator.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(creator) => format!("{creator} · {}", hit.licence),
        None => hit.licence.clone(),
    }
}

fn decode<T: DeserializeOwned>(answer: Result<Value, Error>) -> Result<T, Error> {
    answer.and_then(|v| serde_json::from_value(v).map_err(|e| Error::Protocol(e.to_string())))
}

/// One result in the grid.
struct Shown {
    hit: PictureHit,
    picture: gtk::Picture,
    busy: gtk::Spinner,
}

/// The popover's body while it shows. Its own handlers hold it weakly; the board holds it until
/// the popover closes.
pub(super) struct Picker {
    b: Weak<Board>,
    ui: Weak<Ui>,
    pop: glib::WeakRef<gtk::Popover>,
    design: i64,
    project: i64,
    root: gtk::Box,
    /// Under "From this computer…": the check, the search, or the Turn on note.
    content: gtk::Box,
    search_box: gtk::Box,
    entry: gtk::SearchEntry,
    source: gtk::DropDown,
    shape: gtk::DropDown,
    spinner: gtk::Spinner,
    status: gtk::Label,
    grid: gtk::FlowBox,
    scroll: gtk::ScrolledWindow,
    more: gtk::Button,
    link: gtk::LinkButton,
    key_open: gtk::Button,
    key_row: gtk::Revealer,
    key_note: gtk::Label,
    key_entry: gtk::PasswordEntry,
    key_save: gtk::Button,
    key_forget: gtk::Button,
    /// The search the grid shows: each new one takes the next number (§13.2, step 9).
    serial: Cell<u64>,
    /// The last page shown: the engine drops results whose licence it does not keep, so a page
    /// may hold fewer than [`PER_SEARCH`].
    page: Cell<u32>,
    /// A search has answered, so changing the source or shape asks again.
    searched: Cell<bool>,
    cells: RefCell<Vec<Shown>>,
    queue: RefCell<Queue>,
    choosing: Cell<bool>,
}

/// The popover's body, built each time it shows, as the icon key's is.
pub(super) fn picker(b: &Rc<Board>, pop: &gtk::Popover) -> gtk::Widget {
    let p = Picker::new(b, pop);
    if let Some(old) = b.pictures.borrow_mut().open.replace(p.clone()) {
        old.stop();
    }
    p.check();
    p.root.clone().upcast()
}

/// The popover closed: its picker stops, and its thumbnails go with its body.
pub(super) fn closed(b: &Board, pop: &gtk::Popover) {
    if let Some(p) = b.pictures.borrow_mut().open.take() {
        p.stop();
    }
    pop.set_child(None::<&gtk::Widget>);
}

impl Picker {
    fn new(b: &Rc<Board>, pop: &gtk::Popover) -> Rc<Picker> {
        let last = b.pictures.borrow().last.clone();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("design-picture-picker");
        root.set_size_request(540, -1);
        let from_disk = gtk::Button::with_label("From this computer…");
        from_disk.add_css_class("design-menu-row");
        if let Some(l) = from_disk.child().and_downcast::<gtk::Label>() {
            l.set_xalign(0.0);
        }
        from_disk.set_tooltip_text(Some("Place pictures from your files (Ctrl+Shift+K)"));
        let (weak, p) = (Rc::downgrade(b), pop.downgrade());
        from_disk.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(b) = weak.upgrade() {
                super::assets::choose_images(&b);
            }
        });
        root.append(&from_disk);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.append(&content);

        let search_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let entry = gtk::SearchEntry::new();
        entry.set_placeholder_text(Some("Search free photos"));
        entry.set_hexpand(true);
        entry.set_text(&last.query);
        row.append(&entry);
        let source = gtk::DropDown::from_strings(&SOURCES.map(|(_, name)| name));
        source.add_css_class("design-dropdown");
        source.set_tooltip_text(Some("Where the photos come from"));
        source.set_selected(SOURCES.iter().position(|(s, _)| *s == last.source).unwrap_or(0) as u32);
        row.append(&source);
        let shape = gtk::DropDown::from_strings(&SHAPES.map(|(_, name)| name));
        shape.add_css_class("design-dropdown");
        shape.set_tooltip_text(Some("The photos' shape"));
        shape.set_selected(SHAPES.iter().position(|(o, _)| *o == last.orientation).unwrap_or(0) as u32);
        row.append(&shape);
        search_box.append(&row);
        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let spinner = crate::app::spinner();
        spinner.set_visible(false);
        status_row.append(&spinner);
        let status = label(HINT, "design-picture-status");
        status.set_wrap(true);
        status.set_hexpand(true);
        status_row.append(&status);
        search_box.append(&status_row);
        let grid = gtk::FlowBox::new();
        grid.set_min_children_per_line(3);
        grid.set_max_children_per_line(3);
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        grid.set_valign(gtk::Align::Start);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&grid));
        scroll.set_min_content_height(320);
        scroll.set_max_content_height(440);
        scroll.set_propagate_natural_height(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_visible(false);
        search_box.append(&scroll);
        let more = gtk::Button::with_label("More photos");
        more.add_css_class("quiet");
        more.set_halign(gtk::Align::Center);
        more.set_visible(false);
        search_box.append(&more);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        footer.add_css_class("design-picture-footer");
        let link = gtk::LinkButton::with_label("https://openverse.org", "Openverse · Creative Commons licences");
        link.set_hexpand(true);
        link.set_halign(gtk::Align::Start);
        footer.append(&link);
        let key_open = gtk::Button::with_label("Pexels key…");
        key_open.add_css_class("quiet");
        footer.append(&key_open);
        search_box.append(&footer);
        let key_row = gtk::Revealer::new();
        let key_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        key_box.add_css_class("design-picture-key");
        let key_note = label("Pexels needs an API key (free at pexels.com/api). It stays in this computer's keyring.", "design-picture-status");
        key_note.set_wrap(true);
        key_box.append(&key_note);
        let key_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let key_entry = gtk::PasswordEntry::builder().placeholder_text("Pexels API key").show_peek_icon(true).hexpand(true).build();
        key_line.append(&key_entry);
        let key_save = gtk::Button::with_label("Save key");
        key_save.add_css_class("primary");
        key_line.append(&key_save);
        let key_forget = gtk::Button::with_label("Forget key");
        key_forget.add_css_class("quiet");
        key_line.append(&key_forget);
        key_box.append(&key_line);
        key_row.set_child(Some(&key_box));
        search_box.append(&key_row);

        let checking = label("Checking Pictures…", "design-picture-status");
        content.append(&checking);

        let p = Rc::new(Picker {
            b: Rc::downgrade(b),
            ui: b.ui.clone(),
            pop: pop.downgrade(),
            design: b.doc.id,
            project: b.doc.design.borrow().project_id,
            root,
            content,
            search_box,
            entry,
            source,
            shape,
            spinner,
            status,
            grid,
            scroll,
            more,
            link,
            key_open,
            key_row,
            key_note,
            key_entry,
            key_save,
            key_forget,
            serial: Cell::new(0),
            page: Cell::new(0),
            searched: Cell::new(false),
            cells: RefCell::default(),
            queue: RefCell::default(),
            choosing: Cell::new(false),
        });
        p.footer();
        let weak = Rc::downgrade(&p);
        p.entry.connect_activate(move |_| {
            if let Some(p) = weak.upgrade() {
                p.search(1);
            }
        });
        for dd in [&p.source, &p.shape] {
            let weak = Rc::downgrade(&p);
            dd.connect_selected_notify(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.footer();
                    // The person chose another source or shape for the words already searched.
                    if p.searched.get() {
                        p.search(1);
                    }
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.more.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.search(p.page.get() + 1);
            }
        });
        let weak = Rc::downgrade(&p);
        p.key_open.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.key_row.set_reveal_child(!p.key_row.reveals_child());
            }
        });
        let weak = Rc::downgrade(&p);
        p.key_save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save_key();
            }
        });
        let weak = Rc::downgrade(&p);
        p.key_entry.connect_activate(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save_key();
            }
        });
        let weak = Rc::downgrade(&p);
        p.key_forget.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.forget_key();
            }
        });
        p
    }

    fn chosen_source(&self) -> PictureSource {
        SOURCES.get(self.source.selected() as usize).map_or(PictureSource::Openverse, |(s, _)| *s)
    }

    fn chosen_shape(&self) -> Option<Orientation> {
        SHAPES.get(self.shape.selected() as usize).and_then(|(o, _)| *o)
    }

    /// The footer names the source chosen, with the credit Pexels asks for, and Pexels has its key.
    fn footer(&self) {
        let (uri, words) = match self.chosen_source() {
            PictureSource::Openverse => ("https://openverse.org", "Openverse · Creative Commons licences"),
            PictureSource::Pexels => ("https://www.pexels.com", "Photos provided by Pexels"),
        };
        self.link.set_uri(uri);
        self.link.set_label(words);
        let pexels = self.chosen_source() == PictureSource::Pexels;
        self.key_open.set_visible(pexels);
        if !pexels {
            self.key_row.set_reveal_child(false);
        }
    }

    fn stop(&self) {
        self.queue.borrow_mut().stop();
        self.serial.set(self.serial.get() + 1);
    }

    fn show(&self, child: &impl IsA<gtk::Widget>) {
        crate::app::clear(&self.content);
        self.content.append(child);
    }

    fn say(&self, words: &str) {
        self.status.set_text(words);
        self.status.set_visible(!words.is_empty());
    }

    /// Whether Pictures is on for the design's project: the search if so, else the switch.
    fn check(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let got = ui.call("plugin.get", json!({"plugin_id": PLUGIN})).await;
            let Some(p) = weak.upgrade() else {
                return;
            };
            match got {
                Ok(v) => {
                    let on = v["plugin"]["enabled_in"].as_array().is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(p.project)));
                    if on {
                        p.show_search();
                    } else {
                        p.show_off();
                    }
                }
                // An engine without the plugin: the search says why when asked.
                Err(e) => {
                    p.show_search();
                    p.say(&e.to_string());
                }
            }
        });
    }

    fn show_search(self: &Rc<Self>) {
        self.show(&self.search_box);
        if self.entry.text().trim().is_empty() {
            self.say(HINT);
        } else {
            // The last search again: the engine answers it from memory within the hour.
            self.search(1);
        }
        let entry = self.entry.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(e) = entry.upgrade() {
                e.grab_focus();
            }
        });
    }

    fn show_off(self: &Rc<Self>) {
        let off = gtk::Box::new(gtk::Orientation::Vertical, 10);
        off.add_css_class("design-picture-off");
        let words = label(OFF, "design-picture-status");
        words.set_wrap(true);
        off.append(&words);
        let on = gtk::Button::with_label("Turn on Pictures");
        on.add_css_class("primary");
        on.set_halign(gtk::Align::Start);
        off.append(&on);
        let note = label("", "design-picture-status");
        note.set_wrap(true);
        note.set_visible(false);
        off.append(&note);
        let weak = Rc::downgrade(self);
        on.connect_clicked(move |key| {
            let Some(p) = weak.upgrade() else {
                return;
            };
            let Some(ui) = p.ui.upgrade() else {
                return;
            };
            key.set_sensitive(false);
            let (weak, key, note) = (weak.clone(), key.downgrade(), note.downgrade());
            let project = p.project;
            glib::spawn_future_local(async move {
                let done = ui.call("plugin.enable", json!({"plugin_id": PLUGIN, "project_id": project, "enabled": true})).await;
                match done {
                    Ok(_) => {
                        if let Some(p) = weak.upgrade() {
                            p.show_search();
                        }
                    }
                    Err(e) => {
                        if let Some(key) = key.upgrade() {
                            key.set_sensitive(true);
                        }
                        if let Some(note) = note.upgrade() {
                            note.set_text(&format!("Could not turn on Pictures: {e}"));
                            note.set_visible(true);
                        }
                    }
                }
            });
        });
        self.show(&off);
    }

    /// Search for what the entry says; page 1 is a new search, a later page adds to the grid.
    fn search(self: &Rc<Self>, page: u32) {
        let query = self.entry.text().trim().to_string();
        if query.is_empty() {
            self.say(HINT);
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let (source, orientation) = (self.chosen_source(), self.chosen_shape());
        if let Some(b) = self.b.upgrade() {
            b.pictures.borrow_mut().last = Last { query: query.clone(), source, orientation };
        }
        if page == 1 {
            self.serial.set(self.serial.get() + 1);
        }
        let n = self.serial.get();
        // The results so far stay while it searches.
        self.spinner.set_visible(true);
        self.say("Searching…");
        self.more.set_sensitive(false);
        let mut payload = json!({"design_id": self.design, "query": query, "source": source, "page": page, "count": PER_SEARCH});
        if let Some(o) = orientation {
            payload["orientation"] = json!(o);
        }
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let answer = decode::<SearchOut>(ui.call("design.picture.search", payload).await);
            // A newer search, or the popover closed: this answer is not shown.
            let Some(p) = weak.upgrade().filter(|p| p.serial.get() == n) else {
                return;
            };
            p.spinner.set_visible(false);
            p.more.set_sensitive(true);
            match answer {
                Ok(out) => p.results(n, page, out),
                Err(e) => p.refused(&e),
            }
        });
    }

    fn results(self: &Rc<Self>, n: u64, page: u32, out: SearchOut) {
        self.searched.set(true);
        self.page.set(out.page);
        if page == 1 {
            while let Some(child) = self.grid.first_child() {
                self.grid.remove(&child);
            }
            self.cells.borrow_mut().clear();
            self.queue.borrow_mut().start(n);
            self.scroll.vadjustment().set_value(0.0);
        }
        let from = self.cells.borrow().len();
        for hit in out.results {
            let at = self.cells.borrow().len();
            let shown = self.cell(hit, at);
            self.cells.borrow_mut().push(shown);
        }
        let to = self.cells.borrow().len();
        self.queue.borrow_mut().extend(from..to);
        self.more.set_visible(out.has_more && to > 0);
        self.scroll.set_visible(to > 0);
        self.say(if to == 0 { NONE_MATCH } else { "" });
        self.pump();
    }

    /// Result `at`: its picture (grey until its thumbnail comes), title and byline, all as text.
    fn cell(self: &Rc<Self>, hit: PictureHit, at: usize) -> Shown {
        let key = gtk::Button::new();
        key.add_css_class("design-picture-cell");
        let column = gtk::Box::new(gtk::Orientation::Vertical, 3);
        // The grey box sizes the cell; the picture lies over it, cropped to it, so a photo's own
        // shape never makes its cell taller (a Picture measures its height from its width).
        let ground = gtk::Box::new(gtk::Orientation::Vertical, 0);
        ground.set_size_request(THUMB.0, THUMB.1);
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        let busy = crate::app::spinner();
        busy.set_visible(false);
        busy.set_halign(gtk::Align::Center);
        busy.set_valign(gtk::Align::Center);
        let over = gtk::Overlay::new();
        over.add_css_class("design-picture-thumb");
        over.set_overflow(gtk::Overflow::Hidden);
        over.set_child(Some(&ground));
        over.add_overlay(&picture);
        over.add_overlay(&busy);
        column.append(&over);
        let title = label(if hit.title.trim().is_empty() { "Untitled" } else { &hit.title }, "design-picture-title");
        let by = label(&byline(&hit), "design-picture-meta");
        for l in [&title, &by] {
            l.set_ellipsize(pango::EllipsizeMode::End);
            l.set_max_width_chars(1);
            l.set_hexpand(true);
            column.append(l);
        }
        key.set_child(Some(&column));
        key.set_tooltip_text(Some(&format!("{}\n{}\n{} × {} px", title.text(), by.text(), hit.w, hit.h)));
        let weak = Rc::downgrade(self);
        key.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.choose(at);
            }
        });
        self.grid.insert(&key, -1);
        Shown { hit, picture, busy }
    }

    /// Ask for thumbnails while fewer than four are out, in the grid's order.
    fn pump(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        loop {
            let Some((n, at)) = self.queue.borrow_mut().next() else {
                break;
            };
            let Some(id) = self.cells.borrow().get(at).map(|c| c.hit.id.clone()) else {
                self.queue.borrow_mut().done(n);
                continue;
            };
            let (ui, weak, design) = (ui.clone(), Rc::downgrade(self), self.design);
            glib::spawn_future_local(async move {
                let thumb = decode::<ThumbOut>(ui.call("design.picture.thumb", json!({"design_id": design, "id": id})).await);
                // Decoded off the main thread, as the canvas's pictures are.
                let texture = match thumb {
                    Ok(t) => gio::spawn_blocking(move || gdk::Texture::from_filename(&t.path).ok()).await.ok().flatten(),
                    Err(_) => None,
                };
                let Some(p) = weak.upgrade() else {
                    return;
                };
                if !p.queue.borrow_mut().done(n) {
                    return;
                }
                if let (Some(texture), Some(cell)) = (texture, p.cells.borrow().get(at)) {
                    cell.picture.set_paintable(Some(&texture));
                }
                p.pump();
            });
        }
    }

    /// Store result `at` and place it in the middle of the view, with its credit.
    fn choose(self: &Rc<Self>, at: usize) {
        if self.choosing.get() {
            return;
        }
        let Some((hit, busy)) = self.cells.borrow().get(at).map(|c| (c.hit.clone(), c.busy.clone())) else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.choosing.set(true);
        busy.set_visible(true);
        self.say("Adding the photo…");
        let (weak, board, pop, design) = (Rc::downgrade(self), self.b.clone(), self.pop.clone(), self.design);
        glib::spawn_future_local(async move {
            let added = decode::<AddOut>(ui.call("design.picture.add", json!({"design_id": design, "id": hit.id})).await);
            busy.set_visible(false);
            if let Some(p) = weak.upgrade() {
                p.choosing.set(false);
                if let Err(e) = &added {
                    p.refused(e);
                }
            }
            let (Ok(out), Some(b)) = (added, board.upgrade()) else {
                return;
            };
            // Before the image lands, so the inspector it selects shows the credit.
            b.doc.credits.borrow_mut().insert(out.src.clone(), out.credit);
            let name = names::layer_name(&hit.title).unwrap_or_else(|| "Photo".into());
            super::assets::add_images(&b, vec![(AssetAddOut { src: out.src, path: out.path, w: out.w, h: out.h }, Some(name))], None);
            if let Some(pop) = pop.upgrade() {
                pop.popdown();
            }
        });
    }

    fn refused(self: &Rc<Self>, e: &Error) {
        match said(e) {
            Said::Line(words) => self.say(&words),
            Said::Key(note) => {
                self.say("");
                self.key_note.set_text(&note);
                self.key_row.set_reveal_child(true);
            }
            Said::Off => self.show_off(),
        }
    }

    fn keys_sensitive(&self, on: bool) {
        self.key_save.set_sensitive(on);
        self.key_forget.set_sensitive(on);
        self.key_entry.set_sensitive(on);
    }

    /// Check the key with Pexels and keep it in the keyring; then search again with it.
    fn save_key(self: &Rc<Self>) {
        let key = self.key_entry.text().trim().to_string();
        if key.is_empty() {
            self.key_note.set_text("Paste your Pexels API key first.");
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.keys_sensitive(false);
        self.key_note.set_text("Checking the key with Pexels…");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let saved = ui.call("design.picture.key_set", json!({"key": key})).await;
            let Some(p) = weak.upgrade() else {
                return;
            };
            p.keys_sensitive(true);
            match saved {
                Ok(_) => {
                    p.key_entry.set_text("");
                    p.key_note.set_text("Saved in this computer's keyring.");
                    p.key_row.set_reveal_child(false);
                    p.search(1);
                }
                Err(e) => p.key_note.set_text(&match said(&e) {
                    Said::Key(words) | Said::Line(words) => words,
                    Said::Off => OFF.to_string(),
                }),
            }
        });
    }

    fn forget_key(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.keys_sensitive(false);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let forgot = ui.call("design.picture.key_remove", json!({})).await;
            let Some(p) = weak.upgrade() else {
                return;
            };
            p.keys_sensitive(true);
            p.key_note.set_text(&match forgot {
                Ok(_) => "Forgotten. Pexels needs a key again.".to_string(),
                Err(e) => format!("Could not forget the key: {e}"),
            });
        });
    }
}

/// `words[@pexels|@openverse]`: the words and the source to search, if one is named.
fn drive_words(arg: &str) -> (&str, Option<PictureSource>) {
    match arg.rsplit_once('@') {
        Some((words, "pexels")) => (words, Some(PictureSource::Pexels)),
        Some((words, "openverse")) => (words, Some(PictureSource::Openverse)),
        _ => (arg, None),
    }
}

/// The smoke script's `pictures:words[@source]`: the image key's popover, searching for `words`.
pub(super) fn drive_search(b: &Rc<Board>, arg: &str) {
    let (query, source) = drive_words(arg);
    let open = b.pictures.borrow().open.clone();
    if let Some(p) = open.filter(|p| p.search_box.parent().is_some()) {
        p.entry.set_text(query);
        if let Some(at) = source.and_then(|s| SOURCES.iter().position(|(x, _)| *x == s)) {
            p.searched.set(false);
            p.source.set_selected(at as u32);
        }
        p.search(1);
        return;
    }
    let mut state = b.pictures.borrow_mut();
    state.last.query = query.to_string();
    state.last.source = source.unwrap_or(state.last.source);
    drop(state);
    if let Some(key) = super::tools::find_named(b.overlay.upcast_ref(), "design-image-key").and_downcast::<gtk::MenuButton>() {
        key.popup();
    }
}

/// The smoke script's `picture-pick:<n>`: choose result `n` of the open picker.
pub(super) fn drive_pick(b: &Rc<Board>, n: usize) {
    let open = b.pictures.borrow().open.clone();
    match open {
        Some(p) => p.choose(n),
        None => eprintln!("design script: picture-pick with no picker open"),
    }
}

/// "Add caption" (§13.3): `credit.caption` under image `id`, as one undo step.
pub(super) fn add_caption(b: &Rc<Board>, id: &str) {
    let src = b.doc.scene.borrow().node(id).and_then(|n| n.src.clone());
    let Some(caption) = src.and_then(|src| b.doc.credits.borrow().get(&src).map(|c| c.caption.clone())).filter(|c| !c.trim().is_empty()) else {
        return;
    };
    let (text, frame) = {
        let mut scene = b.doc.scene.borrow_mut();
        (scene.fresh_id(NodeKind::Text), scene.fresh_id(NodeKind::Frame))
    };
    let ops = caption_ops(&b.doc.scene.borrow(), id, &caption, text, frame);
    if !ops.is_empty() {
        b.commit(ops);
    }
}

/// The edits that put `caption` under image `id`, by its parent's layout: in a column, its next
/// sibling, flowing under it; in a free frame, group or page, its next sibling 6 px under it; in a
/// row (or wrap), where a sibling would sit beside it, a hugging column takes the image's place
/// and holds the image and then the caption. `text` and `frame` are fresh ids.
pub(super) fn caption_ops(scene: &Scene, id: &str, caption: &str, text: String, frame: String) -> Vec<NodeOp> {
    let (Some(image), Some(parent)) = (scene.node(id), scene.parent(id)) else {
        return Vec::new();
    };
    let index = scene.index(id).unwrap_or(0) as u32;
    let laid = image.absolute != Some(true);
    let layout = if laid { scene.node(parent).map_or(Layout::None, |p| p.layout_mode()) } else { Layout::None };
    let mut words = Node::new(NodeKind::Text);
    words.id = text;
    words.name = Some("Credit".into());
    words.text = Some(caption.to_string());
    words.font_size = Some(CAPTION_SIZE);
    words.color = Some(CAPTION_COLOR.into());
    // As wide as the photo, so a long credit wraps under it.
    (words.w, words.h) = (image.w, (CAPTION_SIZE * 1.3).ceil());
    match layout {
        Layout::Column => {
            words.parent = Some(parent.to_string());
            words.w_sizing = image.w_sizing.filter(|s| *s == Sizing::Fill);
            vec![NodeOp::Add { node: words, index: Some(index + 1) }]
        }
        Layout::Row | Layout::Wrap => {
            let mut wrap = Node::new(NodeKind::Frame);
            wrap.id = frame.clone();
            wrap.parent = Some(parent.to_string());
            wrap.name = Some("Photo and credit".into());
            wrap.fill = Some("#00000000".into());
            wrap.layout = Some(Layout::Column);
            wrap.gap = Some(CAPTION_GAP);
            (wrap.w_sizing, wrap.h_sizing) = (Some(Sizing::Hug), Some(Sizing::Hug));
            (wrap.w, wrap.h) = (image.w, image.h + CAPTION_GAP + words.h);
            words.parent = Some(frame.clone());
            vec![
                NodeOp::Add { node: wrap, index: Some(index) },
                NodeOp::Move { id: id.to_string(), parent: Some(frame), index: None, keep_position: None },
                NodeOp::Add { node: words, index: None },
            ]
        }
        Layout::None => {
            words.parent = Some(parent.to_string());
            (words.x, words.y) = (image.x, image.y + image.h + CAPTION_GAP);
            vec![NodeOp::Add { node: words, index: Some(index + 1) }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::{BusError, ErrorKind};

    fn bus(code: &str, details: Value) -> Error {
        let mut e = BusError::new(ErrorKind::Unavailable, code, "the engine's words");
        e.details = Some(details);
        Error::Bus(Box::new(e))
    }

    /// §13.4: at most four thumbnails out, asked in the grid's order; a new search empties what
    /// waited and drops what the old one's calls bring back; closing does the same.
    #[test]
    fn the_thumbnail_queue_keeps_four_out_in_cell_order() {
        let mut q = Queue::default();
        q.start(1);
        q.extend(0..18);
        let asked: Vec<_> = std::iter::from_fn(|| q.next()).collect();
        assert_eq!(asked, [(1, 0), (1, 1), (1, 2), (1, 3)], "four at once, in order");
        assert!(q.done(1));
        assert_eq!(q.next(), Some((1, 4)), "one back, the next asked");
        assert_eq!(q.next(), None);
        // More photos: their cells wait behind the first page's.
        q.extend(18..20);
        q.start(2);
        q.extend(0..2);
        assert!(!q.done(1), "an older search's thumbnail is dropped");
        assert_eq!(std::iter::from_fn(|| q.next()).collect::<Vec<_>>(), [(2, 0), (2, 1)], "nothing of the old grid waits");
        assert!(q.done(2) && q.done(2));
        assert_eq!(q.flying, 0);
        q.extend(2..5);
        q.stop();
        assert_eq!(q.next(), None, "closed: nothing more is asked");
        assert!(!q.done(2), "and nothing that comes back is shown");
    }

    /// §13.2 step 4: each refusal has its state.
    #[test]
    fn refusals_map_to_the_pickers_states() {
        assert_eq!(
            said(&bus("design.picture.rate_limited", json!({"source": "openverse", "retry_after_s": 30}))),
            Said::Line("Openverse needs a pause: try again in 30 s.".into())
        );
        assert_eq!(said(&bus("design.picture.rate_limited", json!({"source": "pexels"}))), Said::Line("Pexels needs a pause: try again in 60 s.".into()));
        assert_eq!(said(&bus("design.picture.offline", json!({}))), Said::Line("No connection. Photos already placed still work.".into()));
        assert_eq!(said(&bus("design.picture.gone", json!({}))), Said::Line("That photo is no longer available. Pick another.".into()));
        for code in ["design.picture.no_key", "design.picture.key_rejected", "design.picture.key_format", "design.picture.keyring"] {
            assert!(matches!(said(&bus(code, json!({}))), Said::Key(_)), "{code} opens the key row");
        }
        assert_eq!(said(&bus("plugin.off", json!({"plugin": "pictures"}))), Said::Off);
        assert_eq!(said(&bus("design.picture.provider", json!({}))), Said::Line("the engine's words".into()), "others: the message");
        assert_eq!(said(&Error::Timeout), Said::Line(Error::Timeout.to_string()));
    }

    #[test]
    fn a_script_names_the_words_and_the_source() {
        assert_eq!(drive_words("sunlit kitchen"), ("sunlit kitchen", None));
        assert_eq!(drive_words("kitchen@pexels"), ("kitchen", Some(PictureSource::Pexels)));
        assert_eq!(drive_words("me@home"), ("me@home", None));
    }

    #[test]
    fn a_cell_names_the_creator_and_licence() {
        let mut hit = PictureHit { id: "openverse:x".into(), title: "Lake".into(), creator: Some("Ann".into()), licence: "CC BY 2.0".into(), w: 4, h: 3 };
        assert_eq!(byline(&hit), "Ann · CC BY 2.0");
        hit.creator = Some("  ".into());
        assert_eq!(byline(&hit), "CC BY 2.0");
    }

    fn node(id: &str, parent: Option<&str>, kind: NodeKind) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        (n.x, n.y, n.w, n.h) = (20.0, 30.0, 200.0, 150.0);
        n
    }

    /// p1 › box (a frame laid out `layout`) › before, photo, after.
    fn scene(layout: Option<Layout>) -> Scene {
        let mut frame = node("box", Some("p1"), NodeKind::Frame);
        frame.layout = layout;
        Scene::new(vec![
            node("p1", None, NodeKind::Page),
            frame,
            node("before", Some("box"), NodeKind::Rect),
            node("photo", Some("box"), NodeKind::Image),
            node("after", Some("box"), NodeKind::Rect),
        ])
    }

    fn caption(scene: &Scene) -> Vec<NodeOp> {
        caption_ops(scene, "photo", "Photo: Ann, CC BY 2.0", "text-1".into(), "frame-1".into())
    }

    fn added(op: &NodeOp) -> (&Node, Option<u32>) {
        match op {
            NodeOp::Add { node, index } => (node, *index),
            other => panic!("not an add: {other:?}"),
        }
    }

    /// §13.3: Add caption in a column, a free frame and a row, each one list (one undo step),
    /// and the list runs on the scene.
    #[test]
    fn add_caption_flows_under_the_photo() {
        // A column: the next sibling, which flows under it.
        let column = scene(Some(Layout::Column));
        let ops = caption(&column);
        assert_eq!(ops.len(), 1);
        let (text, index) = added(&ops[0]);
        assert_eq!((text.parent.as_deref(), index, text.kind), (Some("box"), Some(2), NodeKind::Text));
        assert_eq!((text.text.as_deref(), text.font_size, text.color.as_deref()), (Some("Photo: Ann, CC BY 2.0"), Some(11.0), Some(CAPTION_COLOR)));
        assert_eq!(text.w, 200.0, "as wide as the photo");
        let mut s = column.clone();
        s.apply_all(&ops).unwrap();
        assert_eq!(s.children("box"), ["before", "photo", "text-1", "after"]);

        // A free frame: the next sibling, 6 px under the photo.
        let free = scene(None);
        let ops = caption(&free);
        let (text, index) = added(&ops[0]);
        assert_eq!((index, text.x, text.y), (Some(2), 20.0, 30.0 + 150.0 + 6.0));
        let mut s = free.clone();
        s.apply_all(&ops).unwrap();
        assert_eq!(s.children("box"), ["before", "photo", "text-1", "after"]);

        // A row: a hugging column takes the photo's place and holds it, then the caption.
        let row = scene(Some(Layout::Row));
        let ops = caption(&row);
        assert_eq!(ops.len(), 3);
        let (wrap, index) = added(&ops[0]);
        assert_eq!((wrap.id.as_str(), wrap.parent.as_deref(), index), ("frame-1", Some("box"), Some(1)));
        assert_eq!((wrap.layout, wrap.gap, wrap.fill.as_deref()), (Some(Layout::Column), Some(6.0), Some("#00000000")));
        assert_eq!((wrap.w_sizing, wrap.h_sizing), (Some(Sizing::Hug), Some(Sizing::Hug)));
        assert!(matches!(&ops[1], NodeOp::Move { id, parent: Some(p), index: None, .. } if id == "photo" && p == "frame-1"));
        let (text, index) = added(&ops[2]);
        assert_eq!((text.parent.as_deref(), index), (Some("frame-1"), None));
        let mut s = row.clone();
        s.apply_all(&ops).unwrap();
        assert_eq!(s.children("box"), ["before", "frame-1", "after"]);
        assert_eq!(s.children("frame-1"), ["photo", "text-1"]);

        // A photo placed absolutely in a row is free: no wrapping frame.
        let mut row = scene(Some(Layout::Row));
        row.node_mut("photo").unwrap().absolute = Some(true);
        assert_eq!(caption(&row).len(), 1);
        assert!(caption_ops(&row, "nothing", "x", "t".into(), "f".into()).is_empty());
    }
}
