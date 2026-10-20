//! The project's designs: a grid of cards with a picture of each design's first page, which
//! terminal it belongs to and when it last changed. Chips filter it to one terminal (an
//! agent's designs), to the project's own, or to the deleted ones, which can come back. A
//! delete says so with an Undo rather than asking first.
//!
//! The grid is kept, not rebuilt: a change to one design updates its card where it is, and
//! while a menu, a rename or a confirmation is open, or the pointer is over the cards, nothing
//! moves under it (TM-40). Pictures are drawn only while the gallery is on screen, for the
//! cards in view, and saved per Relay instance and painter version (B53, TM-53).

use super::paint::{Painter, PAINTER_VERSION};
use crate::app::{label, rows, text, Ui};
use crate::relative::{ago, Form};
use gtk::prelude::*;
use gtk::{gdk, gio, graphene};
use gtk4 as gtk;
use relay_bus::design::{strip_controls, Styles};
use relay_bus::ops::design::GetOut;
use relay_bus::types::Design;
use relay_client::design::layout::{self, Measure, Placed};
use relay_client::design::names::name_key;
use relay_client::design::{bounds, Camera, Rect, Scene};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

type Opener = Rc<dyn Fn(i64)>;
type Then = Box<dyn FnOnce(&Rc<Gallery>, &Value)>;

/// A card's picture, in pixels at 2× for a crisp 280 by 175 thumbnail.
const THUMB_W: f64 = 560.0;
const THUMB_H: f64 = 350.0;

/// Marks a menu caption as the current choice (drawn as a check).
const CHECKED: &str = "\u{0}checked";

/// Closed terminals shown as chips before the rest fold under "More" (TM-56).
const CLOSED_CHIPS: usize = 3;

/// How long a deleted design waits when the setting cannot be read: the engine's default.
const GRACE_DAYS: i64 = 7;

/// Where the person's sort and filter of each project's gallery are kept (TM-53).
const PREFS: &str = "native.design.gallery";

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

/// Where a delete was asked from, so its Undo brings the design back there (E-gallery-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Gallery,
    Board,
    /// A window of its own, in this project.
    Window(i64),
}

/// What had the keyboard before the gallery changed, to give it back after (B107).
enum Focus {
    Chip,
    Card { id: i64, index: i32 },
}

/// One design's card, kept across renders and updated in place (TM-40).
struct Card {
    child: gtk::FlowBoxChild,
    picture: gtk::Picture,
    title: gtk::Label,
    lamp: gtk::Box,
    terminal: gtk::Label,
    detail: gtk::Label,
    more: Option<gtk::MenuButton>,
    /// Restore and Delete forever, on a deleted card.
    keys: Vec<gtk::Button>,
    deleted: bool,
    /// The rev of the picture it shows; 0 for none.
    shown: Cell<i64>,
}

pub struct Gallery {
    pub root: gtk::Overlay,
    ui: Weak<Ui>,
    pub project: Cell<i64>,
    filter: RefCell<Filter>,
    /// Bumped whenever a filter is chosen, so a saved one read meanwhile does not replace it.
    chosen: Cell<u64>,
    sort: Cell<Sort>,
    sort_key: gtk::DropDown,
    /// The sort is being set from the saved choice: not saved again.
    quiet: Cell<bool>,
    search: gtk::SearchEntry,
    scroll: gtk::ScrolledWindow,
    chips_row: gtk::Box,
    chips: gtk::FlowBox,
    chips_end: gtk::Box,
    /// What the chips showed last, so an unchanged row is left alone (and its focus kept).
    chips_sig: RefCell<String>,
    selected_chip: RefCell<Option<gtk::Button>>,
    grid: gtk::FlowBox,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_body: gtk::Label,
    empty_new: gtk::Button,
    empty_retry: gtk::Button,
    pending: gtk::Box,
    count: gtk::Label,
    new_keys: Vec<gtk::Button>,
    designs: RefCell<Vec<Design>>,
    sessions: RefCell<HashMap<String, String>>,
    /// The terminals were read for this project; design events then reread designs alone.
    sessions_read: Cell<bool>,
    /// The next read takes the terminals too.
    want_sessions: Cell<bool>,
    /// `undo.grace_days`, once read.
    grace: Cell<Option<i64>>,
    /// The project's designs have been read (since it was shown, or switched to).
    listed: Cell<bool>,
    /// Why they could not be read the last time.
    error: RefCell<Option<String>>,
    loading: Cell<bool>,
    dirty: Cell<bool>,
    /// A read is owed for when the gallery is next on screen (B53).
    pub(super) stale: Cell<bool>,
    /// A render waits for a menu, rename or confirmation to close (TM-40).
    owed: Cell<bool>,
    /// The cards are in an older order, kept while the pointer is over them (TM-40).
    resort: Cell<bool>,
    over_grid: Cell<bool>,
    /// Calls in flight, by op and design, so a second click does not send them again
    /// (E-gallery-7).
    busy: RefCell<HashSet<(&'static str, i64)>>,
    /// Open a design on the board.
    pub open: RefCell<Option<Opener>>,
    cards: RefCell<HashMap<i64, Card>>,
    /// The cards' ids in the grid's order.
    order: RefCell<Vec<i64>>,
    /// The newest picture drawn or decoded of each design, by rev (TM-40).
    textures: RefCell<HashMap<i64, (i64, gdk::Texture)>>,
    /// The newest saved picture of each design, by rev, as the last sweep and saves found.
    on_disk: RefCell<HashMap<i64, (i64, PathBuf)>>,
    decoding: RefCell<HashSet<(i64, i64)>>,
    queue: RefCell<VecDeque<(i64, i64)>>,
    rendering: Cell<bool>,
    /// Designs whose picture was drawn before their pictures loaded, drawn once more (B47).
    retried: RefCell<HashSet<i64>>,
    /// Cards showing a picture drawn before its pictures loaded: shown, not saved or kept, so
    /// the next render draws it again (B47).
    unsettled: RefCell<HashSet<i64>>,
    /// No picture is drawn before this: the board just closed (TM-80).
    hold: Cell<Option<Instant>>,
    /// A frame is awaited for cards not laid out yet.
    ticking: Cell<bool>,
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

/// The thumbnails of this Relay instance: the dev, test and stable instances number designs
/// alike, so they keep theirs apart (TM-53).
fn thumbs_dir() -> PathBuf {
    let instance: String = std::env::var("RELAY_INSTANCE").unwrap_or_default().chars().filter(char::is_ascii_alphanumeric).collect();
    glib::user_cache_dir().join("relay-v4").join("design-thumbs").join(if instance.is_empty() { "dev".into() } else { instance })
}

/// A saved thumbnail's name: what the painter drew it with, the design and its rev (TM-53).
fn thumb_name(id: i64, rev: i64) -> String {
    format!("{PAINTER_VERSION}-{id}-{rev}.png")
}

/// A saved thumbnail's painter version, design and rev; `None` for any other file.
fn parse_thumb(name: &str) -> Option<(u32, i64, i64)> {
    let mut parts = name.strip_suffix(".png")?.splitn(3, '-');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// What a sweep of the thumbnails does with `names` (TM-53): it removes another painter's, those
/// of designs not `listed` (purged, or never this instance's) and older revs of a design, and
/// keeps the newest of each listed design, which it answers by design as (rev, name).
fn sweep_plan(names: &[String], listed: &HashSet<i64>) -> (Vec<String>, HashMap<i64, (i64, String)>) {
    let mut newest: HashMap<i64, (i64, String)> = HashMap::new();
    let mut gone = Vec::new();
    for name in names {
        let Some((version, id, rev)) = parse_thumb(name) else {
            continue;
        };
        if version != PAINTER_VERSION || !listed.contains(&id) {
            gone.push(name.clone());
            continue;
        }
        match newest.get(&id) {
            Some((kept, _)) if *kept >= rev => gone.push(name.clone()),
            _ => {
                if let Some((_, older)) = newest.insert(id, (rev, name.clone())) {
                    gone.push(older);
                }
            }
        }
    }
    (gone, newest)
}

/// Sweep `dir` for the designs `listed` (on the blocking pool): what [`sweep_plan`] removes goes,
/// as do a write left half done a minute ago and the pictures saved before instances had
/// folders of their own. The newest picture of each design, by design.
fn sweep(dir: &Path, listed: &HashSet<i64>) -> HashMap<i64, (i64, PathBuf)> {
    if let Some(parent) = dir.parent() {
        for e in std::fs::read_dir(parent).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let old = name.strip_suffix(".png").is_some_and(|s| s.split('-').count() == 2 && s.split('-').all(|p| p.parse::<i64>().is_ok()));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let mut names = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".tmp") {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > Duration::from_secs(60));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
            continue;
        }
        names.push(name);
    }
    let (gone, newest) = sweep_plan(&names, listed);
    for name in gone {
        let _ = std::fs::remove_file(dir.join(name));
    }
    newest.into_iter().map(|(id, (rev, name))| (id, (rev, dir.join(name)))).collect()
}

/// Save a thumbnail whole or not at all (TM-57): written beside its place, then moved there, so
/// a crash never leaves half a picture under the final name. Older ones of the design go.
fn save_thumb(dir: &Path, id: i64, rev: i64, png: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = thumb_name(id, rev);
    let path = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, png)?;
    std::fs::rename(&tmp, &path)?;
    let prefix = format!("{PAINTER_VERSION}-{id}-");
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let other = e.file_name().to_string_lossy().to_string();
        if other != name && other.starts_with(&prefix) && parse_thumb(&other).is_some() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    Ok(path)
}

/// What a page's thumbnail frames: everything its nodes paint, a line's points beyond its box,
/// a turned node's corners, shadows and words included (B3, E-export-8).
fn page_fit(scene: &Scene, placed: &Placed, measure: &dyn Measure, page: &str) -> Option<Rect> {
    scene.children(page).iter().filter_map(|id| bounds::painted_bounds(scene, placed, measure, id)).reduce(|a, b| a.union(&b))
}

/// A rename as typed: the title to send, or `None` when it leaves the title as it was. Control
/// characters go (B43); blank keeps the title.
fn rename_to(typed: &str, original: &str) -> Option<String> {
    let title = strip_controls(typed, false).trim().to_string();
    (!title.is_empty() && title != original.trim()).then_some(title)
}

/// Sort the cards: newest change or creation first, or by name whatever the accents and case
/// (B104), names that differ only in those in one order.
fn sort_designs(list: &mut [Design], sort: Sort) {
    match sort {
        Sort::Recent => list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        Sort::Name => list.sort_by_cached_key(|a| (name_key(&a.title), a.title.clone())),
        Sort::Created => list.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
    }
}

/// The filter shown: one whose designs are all gone falls back to all (TM-51).
fn fallback(current: Filter, terminals: &[(String, usize)], known: impl Fn(&str) -> bool, project_only: usize, deleted: usize) -> Filter {
    match &current {
        Filter::Terminal(s) if !terminals.iter().any(|(n, _)| n == s) && !known(s) => Filter::All,
        Filter::Project if project_only == 0 || terminals.is_empty() => Filter::All,
        Filter::Deleted if deleted == 0 => Filter::All,
        _ => current,
    }
}

/// The header's count: every design, or how many of them show while a filter or a search
/// narrows them (E-gallery-2); under Deleted, how many are deleted.
fn count_text(shown: usize, live: usize, narrowed: bool, deleted: bool) -> String {
    match () {
        _ if deleted && shown == 0 => String::new(),
        _ if deleted => format!("{shown} deleted"),
        _ if live == 0 => String::new(),
        _ if narrowed => format!("{shown} of {live}"),
        _ => live.to_string(),
    }
}

/// When a design deleted `ago` seconds ago is removed for good, with `grace` days to wait;
/// `None` when deleted designs are kept (E-gallery-1).
fn removal_note(ago: i64, grace: i64) -> Option<String> {
    if grace <= 0 {
        return None;
    }
    let left = grace * 86_400 - ago.max(0);
    Some(match (left + 86_399) / 86_400 {
        ..=1 => "removed within a day".into(),
        days => format!("removed in {days} days"),
    })
}

/// The Deleted chip's empty note.
fn deleted_note(grace: i64) -> String {
    match grace {
        ..=0 => "Deleted designs wait here until you delete them for good.".into(),
        1 => "Deleted designs are removed for good after a day.".into(),
        days => format!("Deleted designs are removed for good after {days} days."),
    }
}

/// A card's line after its terminal: when it changed and its pages, or when it was deleted and
/// when it goes, each after the same separator (E-board-23).
fn detail_line(when: &str, pages: i64, removal: Option<&str>) -> String {
    match removal {
        Some(removal) => format!(" · {when} · {removal}"),
        None => format!(" · {when} · {pages} {}", if pages == 1 { "page" } else { "pages" }),
    }
}

/// What the person reads of a refused call (E-gallery-7).
fn refusal(op: &str, err: &crate::client::Error) -> String {
    let code = match err {
        crate::client::Error::Bus(e) => e.code.as_str(),
        _ => "",
    };
    match (op, code) {
        (_, "design.not_found") => "That design was deleted for good meanwhile.".into(),
        ("design.restore", "design.not_deleted") => "That design is back already.".into(),
        ("design.purge", "design.not_deleted") => "That design was restored meanwhile, so it stays.".into(),
        _ => err.to_string(),
    }
}

fn sort_name(sort: Sort) -> &'static str {
    match sort {
        Sort::Recent => "recent",
        Sort::Name => "name",
        Sort::Created => "created",
    }
}

fn sort_from(name: &str) -> Option<Sort> {
    Some(match name {
        "recent" => Sort::Recent,
        "name" => Sort::Name,
        "created" => Sort::Created,
        _ => return None,
    })
}

fn filter_name(filter: &Filter) -> String {
    match filter {
        Filter::All => "all".into(),
        Filter::Project => "project".into(),
        Filter::Deleted => "deleted".into(),
        Filter::Terminal(s) => format!("terminal:{s}"),
    }
}

fn filter_from(name: &str) -> Option<Filter> {
    Some(match name {
        "all" => Filter::All,
        "project" => Filter::Project,
        "deleted" => Filter::Deleted,
        _ => Filter::Terminal(name.strip_prefix("terminal:").filter(|s| !s.is_empty())?.to_string()),
    })
}

/// Seconds since an engine timestamp; 0 when it does not read.
fn seconds_since(ts: &str) -> i64 {
    let (Ok(then), Ok(now)) = (glib::DateTime::from_iso8601(ts, Some(&glib::TimeZone::utc())), glib::DateTime::now_utc()) else {
        return 0;
    };
    now.difference(&then).as_seconds()
}

/// `RELAY_DESIGN_TRACE` set: print what the gallery renders and draws, and how long it takes,
/// so its cost behind a board can be read (B53).
fn trace() -> bool {
    thread_local! {
        static ON: bool = std::env::var_os("RELAY_DESIGN_TRACE").is_some();
    }
    ON.with(|on| *on)
}

/// The design a grid slot shows.
fn card_id(child: &gtk::FlowBoxChild) -> Option<i64> {
    child.child().and_then(|c| c.widget_name().strip_prefix("design-card-").and_then(|id| id.parse::<i64>().ok()))
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

        // The chips wrap rather than widen the page past the window, and Deleted keeps its place
        // at the row's end (TM-56).
        let chips_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chips_row.add_css_class("design-chips");
        let chips = gtk::FlowBox::new();
        chips.set_selection_mode(gtk::SelectionMode::None);
        chips.set_homogeneous(false);
        chips.set_row_spacing(6);
        chips.set_column_spacing(6);
        chips.set_max_children_per_line(64);
        chips.set_hexpand(true);
        chips.set_valign(gtk::Align::Start);
        chips.add_css_class("design-chip-flow");
        chips_row.append(&chips);
        let chips_end = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chips_end.set_valign(gtk::Align::Start);
        chips_row.append(&chips_end);
        page.append(&chips_row);

        // Shown when the first read takes a while (E-gallery-9).
        let pending = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        pending.add_css_class("design-pending");
        pending.set_halign(gtk::Align::Center);
        pending.append(&crate::app::spinner());
        pending.append(&label("Reading designs…", "dim"));
        pending.set_visible(false);
        page.append(&pending);

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
        let empty_retry = gtk::Button::with_label("Try again");
        empty_retry.set_halign(gtk::Align::Center);
        empty_retry.set_margin_top(8);
        empty_retry.set_visible(false);
        empty.append(&empty_retry);
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
            chosen: Cell::new(0),
            sort: Cell::new(Sort::Recent),
            sort_key: sort.clone(),
            quiet: Cell::new(false),
            search,
            scroll: scroll.clone(),
            chips_row,
            chips,
            chips_end,
            chips_sig: RefCell::default(),
            selected_chip: RefCell::default(),
            grid,
            empty,
            empty_title,
            empty_body,
            empty_new: empty_new.clone(),
            empty_retry: empty_retry.clone(),
            pending,
            count,
            new_keys: vec![new.clone(), empty_new.clone()],
            designs: RefCell::default(),
            sessions: RefCell::default(),
            sessions_read: Cell::new(false),
            want_sessions: Cell::new(false),
            grace: Cell::new(None),
            listed: Cell::new(false),
            error: RefCell::default(),
            loading: Cell::new(false),
            dirty: Cell::new(false),
            stale: Cell::new(false),
            owed: Cell::new(false),
            resort: Cell::new(false),
            over_grid: Cell::new(false),
            busy: RefCell::default(),
            open: RefCell::default(),
            cards: RefCell::default(),
            order: RefCell::default(),
            textures: RefCell::default(),
            on_disk: RefCell::default(),
            decoding: RefCell::default(),
            queue: RefCell::default(),
            rendering: Cell::new(false),
            retried: RefCell::default(),
            unsettled: RefCell::default(),
            hold: Cell::new(None),
            ticking: Cell::new(false),
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
                g.next_thumb();
            }
        });
        // Cards scrolled into view get their pictures (TM-53).
        let weak = Rc::downgrade(&gallery);
        scroll.vadjustment().connect_value_changed(move |_| {
            if let Some(g) = weak.upgrade() {
                g.next_thumb();
            }
        });
        // Back on screen: what changed meanwhile is read now (B53), and pictures carry on.
        let weak = Rc::downgrade(&gallery);
        gallery.root.connect_map(move |_| {
            if let Some(g) = weak.upgrade() {
                if g.stale.get() {
                    g.refresh_designs();
                }
                g.next_thumb();
            }
        });
        let weak = Rc::downgrade(&gallery);
        gallery.root.connect_unmap(move |_| {
            if let Some(g) = weak.upgrade() {
                g.over_grid.set(false);
            }
        });
        // The cards keep their order under the pointer, and take the new one when it leaves
        // (TM-40).
        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(&gallery);
        motion.connect_enter(move |_, _, _| {
            if let Some(g) = weak.upgrade() {
                g.over_grid.set(true);
            }
        });
        let weak = Rc::downgrade(&gallery);
        motion.connect_leave(move |_| {
            if let Some(g) = weak.upgrade() {
                g.over_grid.set(false);
                if g.resort.get() {
                    g.render();
                }
            }
        });
        gallery.grid.add_controller(motion);
        for key in [&new, &empty_new] {
            let weak = Rc::downgrade(&gallery);
            key.connect_clicked(move |_| {
                if let Some(g) = weak.upgrade() {
                    g.create();
                }
            });
        }
        let weak = Rc::downgrade(&gallery);
        empty_retry.connect_clicked(move |_| {
            if let Some(g) = weak.upgrade() {
                g.refresh();
            }
        });
        let weak = Rc::downgrade(&gallery);
        gallery.search.connect_search_changed(move |_| {
            if let Some(g) = weak.upgrade() {
                g.render_now();
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
                if !g.quiet.get() {
                    g.save_prefs();
                }
                g.render_now();
            }
        });
        // A card opens with a click, or Enter once it has the keyboard; a deleted one waits for
        // its Restore.
        let weak = Rc::downgrade(&gallery);
        gallery.grid.connect_child_activated(move |_, child| {
            let Some(g) = weak.upgrade() else {
                return;
            };
            let id = card_id(child);
            let live = id.filter(|id| g.designs.borrow().iter().any(|d| d.id == *id && d.deleted_at.is_none()));
            let open = g.open.borrow().clone();
            if let (Some(id), Some(open)) = (live, open) {
                open(id);
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
        for c in self.cards.borrow().values() {
            c.picture.set_size_request(card, (card as f64 / 1.6).round() as i32);
        }
    }

    /// The titles of the designs not deleted, for naming a new one.
    pub fn titles(&self) -> Vec<String> {
        self.designs.borrow().iter().filter(|d| d.deleted_at.is_none()).map(|d| d.title.clone()).collect()
    }

    fn design(&self, id: i64) -> Option<Design> {
        self.designs.borrow().iter().find(|d| d.id == id).cloned()
    }

    /// Another project: nothing of the last one shows (its cards, its search) while this
    /// one's designs are read; its sort and filter are those the person left it with.
    pub fn switch(&self, project: i64) {
        self.project.set(project);
        self.choose(Filter::All);
        self.designs.borrow_mut().clear();
        self.sessions.borrow_mut().clear();
        self.sessions_read.set(false);
        self.error.borrow_mut().take();
        self.listed.set(false);
        self.search.set_text("");
        for (_, c) in self.cards.borrow_mut().drain() {
            self.grid.remove(&c.child);
        }
        self.order.borrow_mut().clear();
        self.textures.borrow_mut().clear();
        self.on_disk.borrow_mut().clear();
        self.queue.borrow_mut().clear();
        self.retried.borrow_mut().clear();
        self.render_now();
        self.load_prefs();
    }

    /// Show `filter`, as chosen now rather than as last saved.
    pub fn choose(&self, filter: Filter) {
        self.chosen.set(self.chosen.get() + 1);
        *self.filter.borrow_mut() = filter;
    }

    /// The search is cleared when the view is opened afresh (TM-52).
    pub fn clear_search(&self) {
        self.search.set_text("");
    }

    /// The project's saved sort and filter (TM-53), unless the person chose one meanwhile.
    fn load_prefs(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let (g, project, chosen) = (self.me(), self.project.get(), self.chosen.get());
        glib::spawn_future_local(async move {
            let Ok(v) = ui.call("settings.get", json!({"path": format!("{PREFS}.{project}")})).await else {
                return;
            };
            if g.project.get() != project {
                return;
            }
            let v = &v["value"];
            if let Some(sort) = v["sort"].as_str().and_then(sort_from) {
                g.quiet.set(true);
                g.sort_key.set_selected(match sort {
                    Sort::Recent => 0,
                    Sort::Name => 1,
                    Sort::Created => 2,
                });
                g.quiet.set(false);
            }
            if let Some(filter) = v["filter"].as_str().and_then(filter_from).filter(|_| g.chosen.get() == chosen) {
                *g.filter.borrow_mut() = filter;
            }
            g.render();
        });
    }

    fn save_prefs(&self) {
        let (Some(ui), project) = (self.ui.upgrade(), self.project.get()) else {
            return;
        };
        if project <= 0 {
            return;
        }
        let value = json!({"sort": sort_name(self.sort.get()), "filter": filter_name(&self.filter.borrow())});
        glib::spawn_future_local(async move {
            let _ = ui.call("settings.set", json!({"path": format!("{PREFS}.{project}"), "value": value})).await;
        });
    }

    /// The first card shown, for the smoke capture.
    pub fn first(&self) -> Option<i64> {
        self.visible().first().map(|d| d.id)
    }

    /// Read the project's designs and terminals again (coalesced: one read in flight at a time).
    pub fn refresh(&self) {
        self.want_sessions.set(true);
        self.refresh_designs();
    }

    /// Read the project's designs again; the terminals only if never read for this project,
    /// since a design event changes none of them (TM-40).
    pub fn refresh_designs(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.stale.set(false);
        self.dirty.set(true);
        if self.loading.replace(true) {
            return;
        }
        if !self.listed.get() {
            let weak = self.me.clone();
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                if let Some(g) = weak.upgrade().filter(|g| !g.listed.get() && g.loading.get()) {
                    g.pending.set_visible(true);
                }
            });
        }
        let g = self.me();
        glib::spawn_future_local(async move {
            while g.dirty.replace(false) {
                let project = g.project.get();
                if project <= 0 {
                    break;
                }
                let listed = ui.call("design.list", json!({"project_id": project, "include_deleted": true})).await;
                let sessions = match g.want_sessions.replace(false) || !g.sessions_read.get() {
                    true => Some(ui.call("session.list", json!({"project_id": project, "include_closed": true})).await),
                    false => None,
                };
                if g.grace.get().is_none() {
                    let read = ui.call("settings.get", json!({"path": "undo.grace_days"})).await;
                    g.grace.set(Some(read.ok().and_then(|v| v["value"].as_i64()).unwrap_or(GRACE_DAYS)));
                }
                if project != g.project.get() {
                    g.dirty.set(true);
                    continue;
                }
                match listed {
                    Ok(v) => {
                        let designs: Vec<Design> = serde_json::from_value(v["designs"].clone()).unwrap_or_default();
                        // What is saved of these, and nothing of designs gone (TM-53).
                        let ids: HashSet<i64> = designs.iter().map(|d| d.id).collect();
                        let (dir, keep) = (thumbs_dir(), ids.clone());
                        let found = gio::spawn_blocking(move || sweep(&dir, &keep)).await.unwrap_or_default();
                        if project != g.project.get() {
                            g.dirty.set(true);
                            continue;
                        }
                        {
                            let mut on_disk = g.on_disk.borrow_mut();
                            on_disk.retain(|id, _| ids.contains(id));
                            for (id, (rev, path)) in found {
                                if on_disk.get(&id).is_none_or(|(kept, _)| *kept < rev) {
                                    on_disk.insert(id, (rev, path));
                                }
                            }
                        }
                        g.textures.borrow_mut().retain(|id, _| ids.contains(id));
                        *g.designs.borrow_mut() = designs;
                        g.error.borrow_mut().take();
                    }
                    Err(e) => {
                        g.toast(&format!("Designs could not be read: {e}"), None);
                        *g.error.borrow_mut() = Some(e.to_string());
                    }
                }
                g.listed.set(true);
                if let Some(Ok(v)) = sessions {
                    g.sessions_read.set(true);
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

    /// The next read takes the terminals too.
    pub(super) fn owe_sessions(&self) {
        self.want_sessions.set(true);
    }

    /// `session.changed`: the terminal's chip and cards show its state.
    pub fn session_changed(&self, name: &str, state: &str) {
        let same = self.sessions.borrow().get(name).is_some_and(|s| s == state);
        if !same && self.sessions_read.get() {
            self.sessions.borrow_mut().insert(name.to_string(), state.to_string());
            self.render();
        }
    }

    fn visible(&self) -> Vec<Design> {
        let filter = self.filter.borrow().clone();
        let q = name_key(self.search.text().trim());
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
            .filter(|d| q.is_empty() || name_key(&d.title).contains(&q) || d.session.as_deref().is_some_and(|s| name_key(s).contains(&q)))
            .cloned()
            .collect();
        sort_designs(&mut list, self.sort.get());
        list
    }

    /// Whether a menu, rename or confirmation of the gallery is open.
    fn held(&self) -> bool {
        crate::app::open_popover(self.root.upcast_ref()).is_some()
    }

    /// Render, unless a popover is open: then once it closes, so nothing it was opened on goes
    /// from under it (TM-40).
    fn render(&self) {
        if self.held() {
            self.owed.set(true);
            return;
        }
        self.render_now();
    }

    /// A popover of the gallery closed: what waited for it shows.
    fn flush(&self) {
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(g) = weak.upgrade().filter(|g| g.owed.get() && !g.held()) {
                g.render_now();
            }
        });
    }

    /// `pop` holds renders while it is open.
    fn holds(&self, pop: &gtk::Popover) {
        let weak = self.me.clone();
        pop.connect_hide(move |_| {
            if let Some(g) = weak.upgrade() {
                g.flush();
            }
        });
    }

    fn render_now(&self) {
        let started = Instant::now();
        self.owed.set(false);
        let focus = self.focus_now();
        let designs = self.designs.borrow().clone();
        let live: Vec<&Design> = designs.iter().filter(|d| d.deleted_at.is_none()).collect();
        let deleted = designs.len() - live.len();
        let mut terminals: Vec<(String, usize)> = Vec::new();
        for d in &live {
            if let Some(s) = &d.session {
                match terminals.iter_mut().find(|(n, _)| n == s) {
                    Some(t) => t.1 += 1,
                    None => terminals.push((s.clone(), 1)),
                }
            }
        }
        terminals.sort_by_cached_key(|(n, _)| (name_key(n), n.clone()));
        let project_only = live.iter().filter(|d| d.session.is_none()).count();
        let current = self.filter.borrow().clone();
        let current = {
            let sessions = self.sessions.borrow();
            fallback(current, &terminals, |s| sessions.contains_key(s), project_only, deleted)
        };
        *self.filter.borrow_mut() = current.clone();
        self.render_chips(&designs, live.len(), terminals, project_only, deleted, &current);
        // Cards.
        let list = self.visible();
        let narrowed = current != Filter::All || !self.search.text().is_empty();
        self.count.set_text(&count_text(list.len(), live.len(), narrowed, current == Filter::Deleted));
        self.render_cards(&list);
        let none = list.is_empty();
        self.grid.set_visible(!none);
        if self.listed.get() {
            self.pending.set_visible(false);
        }
        // Nothing read yet (a project just switched to): no "No designs yet" before the list.
        self.empty.set_visible(none && self.listed.get());
        if none {
            let error = self.error.borrow().clone();
            // A search that finds nothing says so, whatever the chip (TM-52).
            let (title, body) = match &current {
                _ if error.is_some() => ("Designs could not be read", error.clone().unwrap_or_default()),
                _ if !self.search.text().is_empty() => ("No design matches", "Try another word, or clear the search.".to_string()),
                Filter::Deleted => ("Nothing deleted", deleted_note(self.grace.get().unwrap_or(GRACE_DAYS))),
                Filter::Terminal(s) => (
                    "No designs from this terminal",
                    format!("Ask {s} to sketch something — “design the settings screen” — and it appears here as it draws.\nOr start one yourself: it belongs to {s}."),
                ),
                _ => (
                    "No designs yet",
                    "Sketch screens, flows and ideas on an infinite canvas: frames, text, shapes, stickies, pictures, auto layout.\nAgents in this project make and change designs too — ask one, and watch it draw.".to_string(),
                ),
            };
            self.empty_title.set_text(title);
            self.empty_body.set_text(&body);
            self.empty_retry.set_visible(error.is_some());
            self.empty_new.set_visible(error.is_none() && current != Filter::Deleted);
        }
        self.restore_focus(focus);
        self.queue_thumbs(&list);
        if trace() {
            println!("DESIGN_TRACE gallery render {:.3} ms {} cards", started.elapsed().as_secs_f64() * 1e3, list.len());
        }
    }

    // ------------------------------------------------------------------ chips

    fn render_chips(&self, designs: &[Design], live: usize, mut terminals: Vec<(String, usize)>, project_only: usize, deleted: usize, current: &Filter) {
        let sessions = self.sessions.borrow().clone();
        let state_of = |name: &str| sessions.get(name).cloned().unwrap_or_else(|| "closed".into());
        if let Filter::Terminal(s) = current {
            if !terminals.iter().any(|(n, _)| n == s) {
                terminals.push((s.clone(), 0));
            }
        }
        // Closed terminals past the first few fold under More (TM-56); the one shown stays out.
        let (mut shown, mut more) = (Vec::new(), Vec::new());
        let mut closed = 0;
        for (name, n) in terminals {
            let state = state_of(&name);
            if matches!(state.as_str(), "closed" | "exited" | "parked") && *current != Filter::Terminal(name.clone()) {
                closed += 1;
                if closed > CLOSED_CHIPS {
                    more.push((name, n));
                    continue;
                }
            }
            shown.push((name, n, state));
        }
        let project = project_only > 0 && !(shown.is_empty() && more.is_empty());
        let sig = format!("{current:?}|{live}|{shown:?}|{more:?}|{project}/{project_only}|{deleted}|{}", designs.is_empty());
        if *self.chips_sig.borrow() == sig {
            return;
        }
        *self.chips_sig.borrow_mut() = sig;
        while let Some(c) = self.chips.first_child() {
            self.chips.remove(&c);
        }
        while let Some(c) = self.chips_end.first_child() {
            self.chips_end.remove(&c);
        }
        self.selected_chip.borrow_mut().take();
        // Nothing to filter yet, or nothing but All: no chips at all (E-gallery-10).
        self.chips_row.set_visible(!designs.is_empty() && (!shown.is_empty() || !more.is_empty() || project || deleted > 0));
        self.chip("All", live, None, Filter::All, current, None);
        for (name, n, state) in &shown {
            self.chip(name, *n, Some(state), Filter::Terminal(name.clone()), current, None);
        }
        if !more.is_empty() {
            self.more_chip(&more);
        }
        if project {
            self.chip("Project only", project_only, None, Filter::Project, current, None);
        }
        if deleted > 0 {
            let end = self.chips_end.clone();
            self.chip("Deleted", deleted, None, Filter::Deleted, current, Some(&end));
        }
    }

    /// A chip; in the wrapping row unless `into` is given.
    fn chip(&self, caption: &str, count: usize, state: Option<&str>, filter: Filter, current: &Filter, into: Option<&gtk::Box>) {
        let chip = gtk::Button::new();
        chip.add_css_class("design-chip");
        if *current == filter {
            chip.add_css_class("selected");
            *self.selected_chip.borrow_mut() = Some(chip.clone());
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if let Some(state) = state {
            let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            lamp.add_css_class("design-lamp");
            lamp.add_css_class(state);
            lamp.set_valign(gtk::Align::Center);
            row.append(&lamp);
        }
        let name = label(caption, "");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(18);
        row.append(&name);
        row.append(&label(&count.to_string(), "design-chip-count"));
        chip.set_child(Some(&row));
        match state {
            Some(_) => chip.set_tooltip_text(Some(&format!("Designs of {caption}'s terminal"))),
            None if caption.chars().count() > 18 => chip.set_tooltip_text(Some(caption)),
            None => {}
        }
        let weak = self.me.clone();
        chip.connect_clicked(move |_| {
            if let Some(g) = weak.upgrade() {
                g.choose(filter.clone());
                g.save_prefs();
                g.render_now();
            }
        });
        match into {
            Some(b) => b.append(&chip),
            None => self.flow_chip(&chip),
        }
    }

    /// Put `w` in the wrapping row, reached by Tab as itself rather than through its slot.
    fn flow_chip(&self, w: &impl IsA<gtk::Widget>) {
        self.chips.insert(w, -1);
        if let Some(slot) = w.as_ref().parent() {
            slot.set_focusable(false);
        }
    }

    /// The closed terminals that did not get a chip, in a menu (TM-56).
    fn more_chip(&self, more: &[(String, usize)]) {
        let key = gtk::MenuButton::new();
        key.add_css_class("design-chip-more");
        key.set_label(&format!("{} more", more.len()));
        key.set_tooltip_text(Some("Designs of closed terminals"));
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        for (name, n) in more {
            let b = gtk::Button::new();
            b.add_css_class("design-menu-row");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let caption = label(name, "");
            caption.set_hexpand(true);
            caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
            caption.set_max_width_chars(28);
            row.append(&caption);
            row.append(&label(&n.to_string(), "design-chip-count"));
            b.set_child(Some(&row));
            let (weak, p, name) = (self.me.clone(), pop.downgrade(), name.clone());
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(g) = weak.upgrade() {
                    g.choose(Filter::Terminal(name.clone()));
                    g.save_prefs();
                    g.render_now();
                }
            });
            list.append(&b);
        }
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(360);
        scroll.set_child(Some(&list));
        pop.set_child(Some(&scroll));
        key.set_popover(Some(&pop));
        self.holds(&pop);
        self.flow_chip(&key);
    }

    // ------------------------------------------------------------------ cards

    /// Bring the grid to `list`: kept cards updated where they are, new ones built, gone ones
    /// removed, and the order changed only while the pointer is elsewhere (TM-40).
    fn render_cards(&self, list: &[Design]) {
        let ids: Vec<i64> = list.iter().map(|d| d.id).collect();
        let shown: HashSet<i64> = ids.iter().copied().collect();
        self.cards.borrow_mut().retain(|id, c| {
            let keep = list.iter().find(|d| d.id == *id).is_some_and(|d| d.deleted_at.is_some() == c.deleted);
            if !keep {
                self.grid.remove(&c.child);
            }
            keep
        });
        for d in list {
            if !self.cards.borrow().contains_key(&d.id) {
                let card = self.card(d);
                self.cards.borrow_mut().insert(d.id, card);
            }
            if let Some(c) = self.cards.borrow().get(&d.id) {
                self.update_card(c, d);
            }
        }
        let target: Vec<i64> = if self.over_grid.get() {
            // Kept cards stay in their order; new ones go where they belong.
            let mut kept: Vec<i64> = self.order.borrow().iter().copied().filter(|id| shown.contains(id) && self.cards.borrow().get(id).is_some_and(|c| c.child.parent().is_some())).collect();
            for (i, id) in ids.iter().enumerate() {
                if !kept.contains(id) {
                    kept.insert(i.min(kept.len()), *id);
                }
            }
            kept
        } else {
            ids.clone()
        };
        self.resort.set(target != ids);
        let cards = self.cards.borrow();
        for (i, id) in target.iter().enumerate() {
            let Some(c) = cards.get(id) else {
                continue;
            };
            if c.child.parent().is_some() {
                if c.child.index() == i as i32 {
                    continue;
                }
                self.grid.remove(&c.child);
            }
            self.grid.insert(&c.child, i as i32);
        }
        *self.order.borrow_mut() = target;
    }

    fn card(&self, d: &Design) -> Card {
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
        let deleted = d.deleted_at.is_some();
        // A deleted design has no actions but its Restore and Delete forever (B87).
        let more = (!deleted).then(|| {
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
            // Built as it opens, from the design as it is then.
            let weak = self.me.clone();
            let id = d.id;
            pop.connect_show(move |pop| {
                if let Some((g, d)) = weak.upgrade().and_then(|g| g.design(id).map(|d| (g, d))) {
                    pop.set_child(Some(&g.card_menu(&d, pop)));
                }
            });
            self.holds(&pop);
            frame.add_overlay(&more);
            more
        });
        card.append(&frame);
        let title = label("", "design-card-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        card.append(&title);
        // One line, its parts after one separator each (E-board-23).
        let meta = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        meta.add_css_class("design-card-meta");
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.set_valign(gtk::Align::Center);
        lamp.set_margin_end(6);
        meta.append(&lamp);
        let terminal = label("", "design-card-terminal");
        meta.append(&terminal);
        let detail = label("", "design-card-when");
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.append(&detail);
        card.append(&meta);
        let mut keys = Vec::new();
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
                    g.restore(id, Origin::Gallery);
                }
            });
            let weak = self.me.clone();
            forever.connect_clicked(move |key| {
                if let Some((g, d)) = weak.upgrade().and_then(|g| g.design(id).map(|d| (g, d))) {
                    g.confirm_purge(key.upcast_ref(), id, &d.title);
                }
            });
            row.append(&restore);
            row.append(&forever);
            card.append(&row);
            keys = vec![restore, forever];
        } else {
            // Opened by the grid's activation (a click, or Enter); the ⋯ key keeps its click.
            card.set_cursor_from_name(Some("pointer"));
        }
        let child = gtk::FlowBoxChild::new();
        child.set_child(Some(&card));
        Card { child, picture, title, lamp, terminal, detail, more, keys, deleted, shown: Cell::new(0) }
    }

    /// Show `d` on its card: its title, terminal and times, and whether a call on it is in
    /// flight.
    fn update_card(&self, c: &Card, d: &Design) {
        // A title stored with control characters still shows (B43).
        let title = strip_controls(&d.title, false);
        c.title.set_text(&title);
        c.title.set_tooltip_text(Some(&title));
        c.child.update_property(&[gtk::accessible::Property::Label(&title)]);
        c.picture.set_alternative_text(Some(&format!("{title}, its first page")));
        // Read with its design, not as one more "Design actions" (B106).
        if let Some(more) = &c.more {
            more.update_property(&[gtk::accessible::Property::Label(&format!("Actions for {title}"))]);
        }
        match &d.session {
            Some(s) => {
                let state = self.sessions.borrow().get(s).cloned().unwrap_or_else(|| "closed".into());
                c.lamp.set_css_classes(&["design-lamp", &state]);
                c.lamp.set_visible(true);
                c.terminal.set_text(s);
            }
            None => {
                c.lamp.set_visible(false);
                c.terminal.set_text("Project only");
            }
        }
        let detail = match &d.deleted_at {
            Some(at) => {
                let when = format!("deleted {}", ago(at, Form::CompactAgo).unwrap_or_default());
                let removal = removal_note(seconds_since(at), self.grace.get().unwrap_or(GRACE_DAYS));
                detail_line(&when, d.pages, Some(removal.as_deref().unwrap_or("kept until deleted for good")))
            }
            None => {
                let by = if d.updated_by == "user" { String::new() } else { format!(" by {}", d.updated_by) };
                detail_line(&format!("{}{by}", ago(&d.updated_at, Form::CompactAgo).unwrap_or_default()), d.pages, None)
            }
        };
        c.detail.set_text(&detail);
        let busy = self.busy.borrow().iter().any(|(_, id)| *id == d.id);
        for k in &c.keys {
            k.set_sensitive(!busy);
        }
        if let Some(more) = &c.more {
            more.set_sensitive(!busy);
        }
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
            // Weak: this closure lives in the menu, under the key it would hold (TM-55).
            let weak = self.me.clone();
            let title = d.title.clone();
            let anchor = pop.parent().map(|p| p.downgrade());
            item("Rename…", Rc::new(move || {
                if let (Some(g), Some(a)) = (weak.upgrade(), anchor.as_ref().and_then(|a| a.upgrade())) {
                    g.rename(&a, id, &title);
                }
            }));
            let weak = self.me.clone();
            item("Duplicate", Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    g.duplicate(id);
                }
            }));
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            list.append(&label("Belongs to", "design-menu-heading"));
            let mut names: Vec<(String, String)> = self.sessions.borrow().iter().map(|(n, s)| (n.clone(), s.clone())).collect();
            names.sort_by_cached_key(|(n, _)| (name_key(n), n.clone()));
            names.retain(|(n, s)| s != "closed" || d.session.as_deref() == Some(n.as_str()));
            // The current choice is checked, not sent again (TM-58).
            for (name, _) in names {
                let mine = d.session.as_deref() == Some(name.as_str());
                let weak = self.me.clone();
                let n = name.clone();
                item(&format!("{name}{}", if mine { CHECKED } else { "" }), Rc::new(move || {
                    if let Some(g) = weak.upgrade().filter(|_| !mine) {
                        g.mutate("design.update", id, Some(json!({"session": n})), None);
                    }
                }));
            }
            let weak = self.me.clone();
            let mine = d.session.is_none();
            item(&format!("Project only{}", if mine { CHECKED } else { "" }), Rc::new(move || {
                if let Some(g) = weak.upgrade().filter(|_| !mine) {
                    g.mutate("design.update", id, Some(json!({"session": null})), None);
                }
            }));
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let weak = self.me.clone();
            let title = d.title.clone();
            let del = item("Delete", Rc::new(move || {
                if let Some(g) = weak.upgrade() {
                    g.delete(id, &title, Origin::Gallery);
                }
            }));
            del.add_css_class("destructive");
        }
        list.upcast()
    }

    /// The card that has the keyboard, or a chip (B107).
    fn focus_now(&self) -> Option<Focus> {
        let focus = self.root.root()?.focus()?;
        if focus.is_ancestor(&self.chips_row) {
            return Some(Focus::Chip);
        }
        if !focus.is_ancestor(&self.grid) {
            return None;
        }
        let mut w = Some(focus);
        while let Some(widget) = w {
            if let Some(child) = widget.downcast_ref::<gtk::FlowBoxChild>() {
                return Some(Focus::Card { id: card_id(child)?, index: child.index() });
            }
            w = widget.parent();
        }
        None
    }

    /// Give the keyboard back where it was if the render took it: the same card, or the one
    /// now in its place, or the chosen chip (B107). Focus elsewhere is never moved.
    fn restore_focus(&self, focus: Option<Focus>) {
        let Some(focus) = focus else {
            return;
        };
        let kept = self.root.root().and_then(|r| r.focus()).is_some_and(|f| f.is_ancestor(&self.grid) || f.is_ancestor(&self.chips_row));
        if kept {
            return;
        }
        let chip = || self.selected_chip.borrow().clone().is_some_and(|c| c.grab_focus());
        let done = match focus {
            Focus::Chip => chip(),
            Focus::Card { id, index } => {
                let same = self.cards.borrow().get(&id).filter(|c| c.child.parent().is_some()).map(|c| c.child.clone());
                let n = self.order.borrow().len() as i32;
                let near = same.or_else(|| if n > 0 { self.grid.child_at_index(index.min(n - 1)) } else { None });
                near.is_some_and(|c| c.grab_focus()) || chip()
            }
        };
        if !done {
            self.search.grab_focus();
        }
    }

    /// A call on one design, then a fresh list. A second one while it is in flight is
    /// ignored, and its card's keys wait (E-gallery-7).
    fn mutate(&self, op: &'static str, id: i64, extra: Option<Value>, then: Option<Then>) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if !self.busy.borrow_mut().insert((op, id)) {
            return;
        }
        self.sync_card(id);
        let mut payload = json!({"design_id": id});
        if let (Some(Value::Object(extra)), Some(map)) = (extra, payload.as_object_mut()) {
            map.extend(extra);
        }
        let g = self.me();
        glib::spawn_future_local(async move {
            let done = ui.call(op, payload).await;
            g.busy.borrow_mut().remove(&(op, id));
            g.sync_card(id);
            match done {
                Ok(v) => {
                    if let Some(then) = then {
                        then(&g, &v);
                    }
                    g.refresh_designs();
                }
                Err(e) => g.toast(&refusal(op, &e), None),
            }
        });
    }

    /// Show one card's design as it is now.
    fn sync_card(&self, id: i64) {
        if let (Some(d), Some(c)) = (self.design(id), self.cards.borrow().get(&id)) {
            self.update_card(c, &d);
        }
    }

    /// Duplicate design `id`; the toast opens the copy (E-gallery-5).
    fn duplicate(&self, id: i64) {
        self.mutate("design.duplicate", id, None, Some(Box::new(|g, v| {
            let Some(new) = v["id"].as_i64() else {
                return;
            };
            let weak = Rc::downgrade(g);
            g.toast_with(&format!("Duplicated as “{}”", text(v, "title")), "Open", Some(Rc::new(move || {
                if let Some(open) = weak.upgrade().and_then(|g| g.open.borrow().clone()) {
                    open(new);
                }
            })));
        })));
    }

    /// Delete design `id`, with an Undo that brings it back where it was deleted from
    /// (E-gallery-3, E-gallery-4).
    pub fn delete(&self, id: i64, title: &str, origin: Origin) {
        let note = format!("Deleted “{}”", strip_controls(title, false));
        self.mutate("design.delete", id, None, Some(Box::new(move |g, _| {
            super::window::close(id);
            if g.root.is_mapped() {
                let weak = Rc::downgrade(g);
                g.toast(&note, Some(Rc::new(move || {
                    if let Some(g) = weak.upgrade() {
                        g.restore(id, origin);
                    }
                })));
            } else if let Some(ui) = g.ui.upgrade() {
                // No gallery on screen to offer the Undo: Deleted designs keeps it.
                ui.show_info(&format!("{note}. It waits in Deleted designs."));
            }
        })));
    }

    /// Bring a deleted design back, and open it again where it was deleted from (E-gallery-4).
    pub(super) fn restore(&self, id: i64, origin: Origin) {
        self.mutate("design.restore", id, None, Some(Box::new(move |g, _| match origin {
            Origin::Gallery => {}
            Origin::Board => {
                if let Some(open) = g.open.borrow().clone() {
                    open(id);
                }
            }
            Origin::Window(project) => {
                if let Some(ui) = g.ui.upgrade() {
                    super::window::open(&ui, project, id);
                }
            }
        })));
    }

    /// Deleting for good is the one step that cannot come back, so it asks.
    fn confirm_purge(&self, anchor: &gtk::Widget, id: i64, title: &str) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(anchor);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let question = label(&format!("Delete “{}” for good? Its pages and layers cannot be brought back.", strip_controls(title, false)), "");
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
                g.mutate("design.purge", id, None, None);
            }
        });
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        self.holds(&pop);
        pop.popup();
    }

    /// Rename design `id` in place. The name is taken when the popover closes, by Enter or by
    /// clicking away; Escape leaves it as it was (B88).
    fn rename(&self, anchor: &gtk::Widget, id: i64, title: &str) {
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        pop.set_parent(anchor);
        let entry = gtk::Entry::new();
        entry.set_text(&strip_controls(title, false));
        entry.set_width_chars(28);
        pop.set_child(Some(&entry));
        // The entry hears Escape before the popover closes on it.
        let cancel = Rc::new(Cell::new(false));
        let keys = gtk::EventControllerKey::new();
        let c = cancel.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
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
        let weak = self.me.clone();
        let typed = entry.downgrade();
        let original = title.to_string();
        let done = Cell::new(false);
        pop.connect_closed(move |p| {
            if !done.replace(true) && !cancel.get() {
                let title = typed.upgrade().and_then(|e| rename_to(&e.text(), &original));
                if let (Some(g), Some(t)) = (weak.upgrade(), title) {
                    g.mutate("design.update", id, Some(json!({"title": t})), None);
                }
            }
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        self.holds(&pop);
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

    /// A design.create is in flight: New waits for it (E-gallery-7).
    pub fn set_creating(&self, on: bool) {
        for k in &self.new_keys {
            k.set_sensitive(!on);
        }
    }

    pub fn toast(&self, text: &str, undo: Option<Rc<dyn Fn()>>) {
        self.toast_with(text, "Undo", undo);
    }

    /// A toast whose key, `key`, runs `undo`.
    pub fn toast_with(&self, text: &str, key: &str, undo: Option<Rc<dyn Fn()>>) {
        self.toast_label.set_text(text);
        self.toast_undo.set_label(key);
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

    /// Draw no picture for `wait`: the board just closed, and the gallery comes back first
    /// (TM-80).
    pub fn hold_thumbs(&self, wait: Duration) {
        self.hold.set(Some(Instant::now() + wait));
        let weak = self.me.clone();
        glib::timeout_add_local_once(wait, move || {
            if let Some(g) = weak.upgrade() {
                g.next_thumb();
            }
        });
    }

    /// Give each card the picture it has: the one drawn of its rev, else the one saved, else an
    /// older one while its own is drawn. Nothing is read from disk here: the sweep after each
    /// list said what is saved (TM-40, TM-53).
    fn queue_thumbs(&self, list: &[Design]) {
        let mut missing = VecDeque::new();
        for d in list {
            let Some(shown) = self.cards.borrow().get(&d.id).map(|c| c.shown.get()) else {
                continue;
            };
            if shown == d.rev && !self.unsettled.borrow().contains(&d.id) {
                continue;
            }
            let drawn = self.textures.borrow().get(&d.id).cloned();
            if let Some((rev, texture)) = drawn.as_ref().filter(|(rev, _)| *rev <= d.rev && *rev > shown) {
                self.set_picture(d.id, *rev, texture);
            }
            if drawn.as_ref().is_some_and(|(rev, _)| *rev == d.rev) {
                continue;
            }
            let saved = self.on_disk.borrow().get(&d.id).cloned();
            match saved {
                Some((rev, path)) if rev == d.rev => self.load_saved(d.id, rev, path),
                Some((rev, path)) => {
                    if drawn.is_none() && rev < d.rev {
                        self.load_saved(d.id, rev, path);
                    }
                    missing.push_back((d.id, d.rev));
                }
                None => missing.push_back((d.id, d.rev)),
            }
        }
        *self.queue.borrow_mut() = missing;
        self.next_thumb();
    }

    fn set_picture(&self, id: i64, rev: i64, texture: &gdk::Texture) {
        if let Some(c) = self.cards.borrow().get(&id).filter(|c| c.shown.get() <= rev) {
            c.picture.set_paintable(Some(texture));
            c.shown.set(rev);
        }
    }

    /// A picture drawn or decoded: kept by rev, and shown on its card.
    fn keep_texture(&self, id: i64, rev: i64, texture: &gdk::Texture) {
        let newer = self.textures.borrow().get(&id).is_some_and(|(kept, _)| *kept > rev);
        if !newer {
            self.textures.borrow_mut().insert(id, (rev, texture.clone()));
        }
        self.set_picture(id, rev, texture);
    }

    /// Decode a saved thumbnail off the main thread. One that does not decode (cut short) is
    /// removed and drawn again (TM-57).
    fn load_saved(&self, id: i64, rev: i64, path: PathBuf) {
        if !self.decoding.borrow_mut().insert((id, rev)) {
            return;
        }
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let p = path.clone();
            let texture = gio::spawn_blocking(move || gdk::Texture::from_filename(&p).ok()).await.ok().flatten();
            let Some(g) = weak.upgrade() else {
                return;
            };
            g.decoding.borrow_mut().remove(&(id, rev));
            if let Some(t) = texture {
                return g.keep_texture(id, rev, &t);
            }
            let _ = gio::spawn_blocking(move || std::fs::remove_file(&path)).await;
            if g.on_disk.borrow().get(&id).is_some_and(|(kept, _)| *kept == rev) {
                g.on_disk.borrow_mut().remove(&id);
            }
            if let Some(d) = g.design(id) {
                if !g.queue.borrow().iter().any(|(q, _)| *q == id) {
                    g.queue.borrow_mut().push_back((id, d.rev));
                }
                g.next_thumb();
            }
        });
    }

    /// The first queued design whose card is in view or just below it (TM-53); cards not laid
    /// out yet are looked at again on the next frame.
    fn pick(&self) -> Option<(i64, i64)> {
        let cards = self.cards.borrow();
        let reach = self.scroll.height() as f32 * 1.5;
        let mut queue = self.queue.borrow_mut();
        queue.retain(|(id, _)| cards.get(id).is_some_and(|c| c.child.parent().is_some()));
        let mut unplaced = false;
        let mut found = None;
        for (i, (id, _)) in queue.iter().enumerate() {
            let p = &cards[id].picture;
            match p.compute_bounds(&self.scroll).filter(|b| p.is_mapped() && b.height() > 0.0) {
                Some(b) if b.y() + b.height() > 0.0 && b.y() < reach => {
                    found = Some(i);
                    break;
                }
                Some(_) => {}
                None => unplaced = true,
            }
        }
        if let Some(i) = found {
            return queue.remove(i);
        }
        if unplaced && !self.ticking.replace(true) {
            let weak = self.me.clone();
            self.grid.add_tick_callback(move |_, _| {
                if let Some(g) = weak.upgrade() {
                    g.ticking.set(false);
                    g.next_thumb();
                }
                glib::ControlFlow::Break
            });
        }
        None
    }

    /// Draw the next missing thumbnail: the design's first page, fitted. Only while the gallery
    /// is on screen (B53), and not just after the board closed (TM-80).
    fn next_thumb(&self) {
        // Drawing needs the window's renderer: wait until the gallery is on screen.
        if self.rendering.get() || !self.root.is_mapped() || self.root.native().and_then(|n| n.renderer()).is_none() {
            return;
        }
        if self.hold.get().is_some_and(|t| Instant::now() < t) {
            return;
        }
        let Some((id, rev)) = self.pick() else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.rendering.set(true);
        let g = self.me();
        let started = Instant::now();
        glib::spawn_future_local(async move {
            let got = ui.call("design.get", json!({"design_id": id})).await;
            if let Ok(got) = got.map_err(|e| e.to_string()).and_then(|v| serde_json::from_value::<GetOut>(v).map_err(|e| e.to_string())) {
                *g.painter.assets_dir.borrow_mut() = got.assets_dir.clone();
                let rev = got.design.rev.max(rev);
                // Drawn as the board draws it: named colours and styles resolved.
                let scene = Scene::new(got.nodes).with_styles(Styles { colors: got.colors, styles: got.styles });
                let first = scene.pages().first().cloned();
                // Pictures decode off the main thread: wait for the first page's (a few seconds
                // at most), or the thumbnail would keep their grey placeholders (E-export-9).
                let loading = |g: &Gallery| first.as_deref().is_some_and(|page| g.painter.loading_under(&scene, page));
                for _ in 0..100 {
                    if !loading(&g) {
                        break;
                    }
                    glib::timeout_future(Duration::from_millis(50)).await;
                }
                let settled = !loading(&g);
                let texture = g.draw_thumb(&scene);
                // Only this design's pictures stay decoded (B33).
                g.painter.retain(&scene);
                g.painter.retain_pictures(&scene, true);
                if let Some(texture) = texture {
                    if settled {
                        g.unsettled.borrow_mut().remove(&id);
                        g.keep_texture(id, rev, &texture);
                        let dir = thumbs_dir();
                        let saved = gio::spawn_blocking(move || save_thumb(&dir, id, rev, &texture.save_to_png_bytes())).await;
                        if let Ok(Ok(path)) = saved {
                            if !g.on_disk.borrow().get(&id).is_some_and(|(kept, _)| *kept > rev) {
                                g.on_disk.borrow_mut().insert(id, (rev, path));
                            }
                        }
                    } else {
                        // Shown with its placeholders, but neither kept nor saved, and drawn
                        // once more now (B47); a later render draws it again if need be.
                        g.unsettled.borrow_mut().insert(id);
                        g.set_picture(id, rev, &texture);
                        if g.retried.borrow_mut().insert(id) {
                            g.queue.borrow_mut().push_front((id, rev));
                        }
                    }
                }
            }
            if trace() {
                println!("DESIGN_TRACE thumb {id} {:.3} ms", started.elapsed().as_secs_f64() * 1e3);
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

    fn draw_thumb(&self, scene: &Scene) -> Option<gdk::Texture> {
        let page = scene.pages().first()?.clone();
        let placed = layout::place(scene, &page, &*self.painter);
        let bounds = page_fit(scene, &placed, &*self.painter, &page);
        let canvas = scene.node(&page).and_then(super::paint::fill_of).unwrap_or_else(|| super::paint::CANVAS.into());
        let snap = gtk::Snapshot::new();
        snap.append_color(&super::paint::rgba(&canvas), &graphene::Rect::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32));
        if let Some(bounds) = bounds {
            let cam = Camera::fit(&bounds, THUMB_W, THUMB_H, 28.0, 2.0);
            snap.save();
            snap.scale(cam.zoom as f32, cam.zoom as f32);
            snap.translate(&graphene::Point::new(-cam.x as f32, -cam.y as f32));
            // Top-level blends mix with the page's fill, as on the board (B30).
            self.painter.paint_page_on(&snap, scene, &placed, &page, None, Some(&canvas));
            snap.restore();
        }
        let node = snap.to_node()?;
        let renderer = self.root.native()?.renderer()?;
        Some(renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::design::{Node, NodeKind};

    /// Every glyph 8 px wide and one 16 px line.
    struct Mono;

    impl Measure for Mono {
        fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
            let w = n.text.as_deref().unwrap_or("").chars().count() as f64 * 8.0;
            (wrap.map_or(w, |wrap| w.min(wrap)), 16.0)
        }
    }

    fn node(id: &str, parent: Option<&str>, kind: NodeKind, rect: [f64; 4]) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        [n.x, n.y, n.w, n.h] = rect;
        n
    }

    fn design(id: i64, title: &str, updated: &str) -> Design {
        serde_json::from_value(json!({
            "id": id, "project_id": 1, "session": null, "title": title, "rev": 1, "pages": 1, "nodes": 0,
            "created_by": "user", "updated_by": "user", "created_at": updated, "updated_at": updated, "deleted_at": null,
        }))
        .unwrap()
    }

    /// B3, E-export-8: a thumbnail frames what a free line draws past its empty box, and a turned
    /// node's corners, not their stored boxes.
    #[test]
    fn a_thumbnail_frames_what_the_page_paints() {
        let mut line = node("l", Some("p"), NodeKind::Line, [100.0, 100.0, 0.0, 0.0]);
        line.points = Some(vec![[0.0, 0.0], [300.0, -50.0]]);
        let mut turned = node("r", Some("p"), NodeKind::Rect, [0.0, 200.0, 100.0, 20.0]);
        turned.rotation = Some(90.0);
        let scene = Scene::new(vec![node("p", None, NodeKind::Page, [0.0; 4]), line, turned]);
        let placed = layout::place(&scene, "p", &Mono);
        let fit = page_fit(&scene, &placed, &Mono, "p").unwrap();
        assert!(fit.right() >= 400.0 && fit.y <= 50.0, "the line's far end is in: {fit:?}");
        assert!(fit.bottom() >= 259.0, "the turned rect stands 100 tall about its centre: {fit:?}");
        let stored = Rect::bounds(scene.children("p").iter().filter_map(|id| placed.rects.get(id))).unwrap();
        assert!(stored.right() < 400.0 && stored.bottom() < 259.0, "the stored boxes alone would cut both short: {stored:?}");
        let bare = Scene::new(vec![node("p", None, NodeKind::Page, [0.0; 4])]);
        assert!(page_fit(&bare, &layout::place(&bare, "p", &Mono), &Mono, "p").is_none());
    }

    /// B43, B88: what a rename sends.
    #[test]
    fn a_rename_sends_a_clean_changed_title() {
        assert_eq!(rename_to("Home", "Home"), None);
        assert_eq!(rename_to("  ", "Home"), None, "blank keeps the title");
        assert_eq!(rename_to(" Settings ", "Home").as_deref(), Some("Settings"));
        assert_eq!(rename_to("Ho\u{0}me\u{7}", "Old").as_deref(), Some("Home"), "control characters go");
        assert_eq!(rename_to("\u{1}", "Old"), None);
    }

    /// B104: names sort by their letters, whatever the accents and case.
    #[test]
    fn names_sort_without_regard_to_accents_or_case() {
        let mut list: Vec<Design> =
            ["Zebra", "École", "apple", "Été", "banana", "ecole"].iter().enumerate().map(|(i, t)| design(i as i64, t, "2026-10-01T00:00:00Z")).collect();
        sort_designs(&mut list, Sort::Name);
        let titles: Vec<&str> = list.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, ["apple", "banana", "ecole", "École", "Été", "Zebra"]);
        let mut list = vec![design(1, "a", "2026-10-01T00:00:00Z"), design(2, "b", "2026-10-02T00:00:00Z")];
        sort_designs(&mut list, Sort::Recent);
        assert_eq!(list[0].id, 2);
    }

    /// TM-53: thumbnails are named by painter version, design and rev, and a sweep keeps only
    /// the newest of each listed design.
    #[test]
    fn a_sweep_keeps_the_newest_thumbnail_of_each_listed_design() {
        assert_eq!(parse_thumb(&thumb_name(12, 30)), Some((PAINTER_VERSION, 12, 30)));
        assert_eq!(parse_thumb("12-30.png"), None, "the old layout is no thumbnail of this painter");
        assert_eq!(parse_thumb("notes.txt"), None);
        let old = PAINTER_VERSION - 1;
        let names: Vec<String> = vec![thumb_name(1, 4), thumb_name(1, 9), thumb_name(2, 3), thumb_name(3, 1), format!("{old}-1-9.png"), "x.png".into()];
        let (mut gone, newest) = sweep_plan(&names, &[1, 2].into());
        gone.sort();
        let mut want = vec![thumb_name(1, 4), thumb_name(3, 1), format!("{old}-1-9.png")];
        want.sort();
        assert_eq!(gone, want, "an older rev, a design gone and another painter's");
        assert_eq!(newest.get(&1), Some(&(9, thumb_name(1, 9))));
        assert_eq!(newest.get(&2), Some(&(3, thumb_name(2, 3))));
        assert_eq!(newest.len(), 2);
    }

    /// TM-57: a thumbnail is saved whole under its final name and replaces its design's older
    /// ones alone; a sweep then drops a design no longer listed.
    #[test]
    fn a_saved_thumbnail_replaces_its_older_ones() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch/gallery-thumbs-test/dev");
        let _ = std::fs::remove_dir_all(&dir);
        save_thumb(&dir, 5, 1, b"one").unwrap();
        save_thumb(&dir, 6, 1, b"other").unwrap();
        let path = save_thumb(&dir, 5, 2, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, [thumb_name(5, 2), thumb_name(6, 1)], "no half-written file is left either");
        let found = sweep(&dir, &[5].into());
        assert_eq!(found.get(&5).map(|(rev, _)| *rev), Some(2));
        assert!(!dir.join(thumb_name(6, 1)).exists(), "a design no longer listed loses its picture");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// TM-51: a filter whose designs are gone falls back to all.
    #[test]
    fn a_filter_with_nothing_left_falls_back_to_all() {
        let none = |_: &str| false;
        let terminals = vec![("ana".to_string(), 2)];
        assert_eq!(fallback(Filter::Project, &terminals, none, 0, 0), Filter::All, "no project-only design left");
        assert_eq!(fallback(Filter::Project, &[], none, 3, 0), Filter::All, "no terminals: its chip is not shown");
        assert_eq!(fallback(Filter::Project, &terminals, none, 3, 0), Filter::Project);
        assert_eq!(fallback(Filter::Deleted, &terminals, none, 0, 0), Filter::All);
        assert_eq!(fallback(Filter::Terminal("bo".into()), &terminals, none, 0, 0), Filter::All);
        assert_eq!(fallback(Filter::Terminal("bo".into()), &terminals, |s| s == "bo", 0, 0), Filter::Terminal("bo".into()), "a known terminal keeps its chip");
    }

    /// E-gallery-1, E-gallery-2, E-board-23: the header's count and a card's line.
    #[test]
    fn counts_and_card_lines_read_as_meant() {
        assert_eq!(count_text(4, 4, false, false), "4");
        assert_eq!(count_text(1, 4, true, false), "1 of 4");
        assert_eq!(count_text(0, 0, false, false), "");
        assert_eq!(count_text(2, 4, true, true), "2 deleted");
        assert_eq!(removal_note(0, 7).as_deref(), Some("removed in 7 days"));
        assert_eq!(removal_note(86_400 * 3 + 60, 7).as_deref(), Some("removed in 4 days"));
        assert_eq!(removal_note(86_400 * 6 + 60, 7).as_deref(), Some("removed within a day"));
        assert_eq!(removal_note(86_400 * 30, 7).as_deref(), Some("removed within a day"), "overdue: the next sweep takes it");
        assert_eq!(removal_note(0, 0), None, "kept for good");
        assert_eq!(deleted_note(7), "Deleted designs are removed for good after 7 days.");
        assert_eq!(detail_line("2h ago", 1, None), " · 2h ago · 1 page");
        assert_eq!(detail_line("2h ago", 3, None), " · 2h ago · 3 pages");
        assert_eq!(detail_line("deleted 1d ago", 3, Some("removed in 6 days")), " · deleted 1d ago · removed in 6 days");
    }

    /// TM-53: the sort and filter kept per project read back as written.
    #[test]
    fn saved_sorts_and_filters_read_back() {
        for sort in [Sort::Recent, Sort::Name, Sort::Created] {
            assert_eq!(sort_from(sort_name(sort)), Some(sort));
        }
        for filter in [Filter::All, Filter::Project, Filter::Deleted, Filter::Terminal("ana-2".into())] {
            assert_eq!(filter_from(&filter_name(&filter)), Some(filter));
        }
        assert_eq!(filter_from("terminal:"), None);
        assert_eq!(sort_from("size"), None);
    }

    /// E-gallery-7: refusals the person can act on read plainly.
    #[test]
    fn refusals_read_plainly() {
        let bus = |code: &str| crate::client::Error::Bus(Box::new(relay_bus::BusError::conflict(code, "x")));
        assert_eq!(refusal("design.restore", &bus("design.not_deleted")), "That design is back already.");
        assert_eq!(refusal("design.purge", &bus("design.not_deleted")), "That design was restored meanwhile, so it stays.");
        assert_eq!(refusal("design.restore", &bus("design.not_found")), "That design was deleted for good meanwhile.");
        assert_eq!(refusal("design.update", &crate::client::Error::Timeout), crate::client::Error::Timeout.to_string());
    }
}

