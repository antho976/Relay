//! The Threads page: the thread list in the sidebar, the conversation with its message box, and
//! the panel beside it, Tally's, Arbiter's or Avex's (docs/THREADS.md, docs/ARBITER.md, docs/GYM.md).
//!
//! Threads are the engine's (`thread.*`); this page draws them and follows `thread.changed`,
//! `thread.message` and `thread.delta`. The page is one stack page, `threads`, whose middle shows
//! either a new thread (a greeting and the message box) or the open one.
//!
//! Pictures go with a message when the thread's model reads them: the message box's picture key,
//! a paste or a drop adds one, and each thread's draft keeps its own. Preparing, sending and
//! showing them is `threads_pictures.rs`'s.
//!
//! The page never holds the window open: its scrollers keep their content's width to themselves
//! (`PolicyType::External`), and width probes size the conversation (`fit`) and put the panel away
//! on a narrow page (`apply_panel`).
use super::invest::gain_class;
use super::kit::{block_link, panel_block as block};
use super::pages::{badge, badge_sized, human_date, left_or_over, meter_in, tile, tone, tone_class, Tone};
use super::pictures::{self, Attached, Source, MAX_PICTURES};
use crate::app::{button, clear, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::models;
use relay_client::thread_view::{chart_spec, import_spec, markdown, tool_caption, tool_op, tool_writes, Block};
use relay_money::copy::{plural, plural_as};
use relay_money::money::{Locale, MoneyFormatter};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

/// The stack page.
pub const PAGE: &str = "threads";

/// Questions a new thread offers when the ledger suggests fewer of its own: (what is asked, what
/// it does).
const SUGGESTIONS: [(&str, &str); 4] = [
    ("Where did my money go this week?", "Spending by category"),
    ("Am I on pace this month?", "Each budget against the day of the month"),
    ("What bills are coming up?", "Due dates and whether you can cover them"),
    ("Any duplicates or odd entries this month?", "Entries that look doubled or out of place"),
];

/// Avex's questions, when its export suggests fewer of its own ([`avex_suggestions`]).
const AVEX_SUGGESTIONS: [(&str, &str); 4] = [
    ("How is my week going?", "Workouts and volume against last week"),
    ("Which lifts have stalled?", "Flat or down over four weeks"),
    ("What are my latest records?", "Personal bests and the sets behind them"),
    ("Am I training consistently?", "Weeks in a row and days a week"),
];

/// The Claude models a thread may run on: (id, caption). An empty id is Claude's own default.
const MODELS: [(&str, &str); 5] = models::CLAUDE;

/// How long a local model may sit unused before its server stops: (minutes, caption); 0 never.
const IDLE_CHOICES: [(u64, &str); 5] = [(5, "5 min"), (15, "15 min"), (30, "30 min"), (60, "1 hour"), (0, "Never")];

/// How hard the agent thinks: (value, caption), as `claude --effort` takes them.
const EFFORTS: [(&str, &str); 6] = [
    ("", "Default effort"),
    ("low", "Low effort"),
    ("medium", "Medium effort"),
    ("high", "High effort"),
    ("xhigh", "Extra high effort"),
    ("max", "Max effort"),
];

/// The Tally panel's views: (key, caption). Budgets are Overview's, all of them a key away.
const PANEL_TABS: [(&str, &str); 3] = [("overview", "Overview"), ("entries", "Entries"), ("invest", "Invest")];
/// How many budgets Overview shows before "Show all".
const BUDGETS_SHOWN: usize = 5;
/// How many of the period's entries, newest first, the Entries view reads.
const ENTRIES_SHOWN: u32 = 60;

/// The panel's sources, as the switch above it names them: (key, caption, what it means).
/// Choosing one shows its panel, and a new thread's greeting and questions follow it; the agent
/// reaches all of them either way.
const SOURCES: [(&str, &str, &str); 3] = [
    ("tally", "Tally", "Show Tally beside the thread: the agent reads and changes your budget and investments"),
    ("arbiter", "Arbiter", "Show Arbiter beside the thread: the agent reads, backtests and proposes; orders follow each strategy's mode"),
    ("avex", "Avex", "Show Avex beside the thread: the agent reads your training as last exported from the phone"),
];

/// The widest the conversation and its message box grow (the mockup's column).
const COLUMN_MAX: i32 = 720;
/// Under this page width the Tally panel steps aside, unless the person opened it.
const PANEL_FROM: i32 = 1000;
/// The panel's width until the person drags it, and the least it takes ([`restore`]).
const PANEL_WIDTH: i32 = 340;
const PANEL_MIN: i32 = 300;
/// What the working row says before the agent has started.
const STARTING: &str = "Starting Claude…";
/// What a new thread asks Claude when the person picks "Connect Wealthsimple".
const CONNECT: &str = "Help me connect my Wealthsimple accounts";
/// The model picker's tooltip, and what it and the effort picker say while a reply is written.
const MODEL_ABOUT: &str = "The model this thread's agent runs on";
const WAITS: &str = "Waits for this reply to finish";
/// What Enter says while the agent is still answering, as the phone does.
const STILL_ANSWERING: &str = "Claude is still answering: stop it first, or wait";
/// What the message box says while a picture keeps it from sending.
const STILL_GOING: &str = "A picture is still on its way: send once it is up";
const REFUSED_PICTURE: &str = "A picture was not added: remove it to send";
/// What a drop of files that are not pictures says.
const ONLY_PICTURES: &str = "Only PNG, JPEG, GIF or WebP pictures go in a message";

/// A chip that opens a list of choices: the key, the caption it shows, and each choice's check.
struct Picker {
    key: gtk::MenuButton,
    shown: gtk::Label,
    marks: RefCell<Vec<(String, gtk::Image)>>,
    popover: gtk::Popover,
    pick: fn(&str),
    /// The model picker's own section: the models on this PC ([`load_local`]).
    local: gtk::Box,
}

struct View {
    title: gtk::Label,
    title_key: gtk::MenuButton,
    /// An Aide chat's keys in the strip (`aide::fill_strip`): a bot's state, Turn on, Run, Details.
    aide_keys: gtk::Box,
    rename: gtk::Entry,
    middle: gtk::Stack,
    /// A new thread: the greeting, where the message box sits, and questions to start with.
    empty: gtk::Box,
    empty_slot: gtk::Box,
    empty_line: gtk::Label,
    suggestions: gtk::Grid,
    /// An open thread's foot, and the box in it the message box sits in.
    dock: gtk::ScrolledWindow,
    dock_slot: gtk::Box,
    /// What moves between the two: the notice above the composer and the composer.
    message_box: gtk::Box,
    notice: gtk::Box,
    notice_text: gtk::Label,
    composer: gtk::Box,
    input: gtk::TextView,
    hint: gtk::Label,
    /// The pictures going with the message: a chip each, above the words ([`set_pictures`]); the
    /// key that adds one; and a quiet line on why one did not go in or the message waits.
    chips: gtk::Box,
    chips_scroll: gtk::ScrolledWindow,
    picture_key: gtk::Button,
    picture_note: gtk::Label,
    send: gtk::Button,
    limits: gtk::Label,
    model: Picker,
    effort: Picker,
    scroll: gtk::ScrolledWindow,
    jump: gtk::Button,
    column: gtk::Box,
    /// The reply being written, shown while `thread.delta` text arrives: a label per finished
    /// stretch of it and, last, the one still being written ([`show_pending`]).
    pending: gtk::Box,
    working: gtk::Box,
    working_text: gtk::Label,
    /// The conversation and the panel, side by side: its position is the panel's width.
    split: gtk::Paned,
    panel: gtk::Box,
    panel_key: gtk::ToggleButton,
    panel_body: gtk::Box,
    panel_tabs: Vec<(&'static str, gtk::Button)>,
    /// The rows of panel tabs, Tally's, Arbiter's and Avex's, one shown at a time.
    tally_tabs: gtk::Box,
    arbiter_tabs: gtk::Box,
    arbiter_panel_tabs: Vec<(&'static str, gtk::Button)>,
    avex_tabs: gtk::Box,
    avex_panel_tabs: Vec<(&'static str, gtk::Button)>,
    /// The source switch above the panel's tabs: the one place a source is chosen.
    sources: Vec<(&'static str, gtk::Button)>,
}

/// Where a tool call stands: called, answered, refused, or cut off by a stop with no answer.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Running,
    Done,
    Failed,
    Cut,
}

/// A tool call drawn in this thread: where its line (and later its card) is, its state mark,
/// what it called, and the run of reads it sits in.
struct Tool {
    slot: gtk::Box,
    mark: gtk::Box,
    /// What the line says: what the agent is doing while it runs, what it did after.
    caption: gtk::Label,
    name: String,
    step: Step,
    group: Option<Group>,
}

/// A run of the agent's reads in one turn: lines while there are two, folded under one line from
/// three, which opens them again.
#[derive(Clone)]
struct Group {
    outer: gtk::Box,
    summary: gtk::Button,
    summary_mark: gtk::Box,
    summary_text: gtk::Label,
    revealer: gtk::Revealer,
    lines: gtk::Box,
    /// Its tool calls, by id, in order.
    members: Rc<RefCell<Vec<String>>>,
    first: String,
    folded: Rc<Cell<bool>>,
}

/// The agent's current turn: where its pieces go, its words so far and the Copy key for them.
#[derive(Clone)]
struct Turn {
    body: gtk::Box,
    text: Rc<RefCell<String>>,
    copy: gtk::Button,
}

/// What a thread's message box held when the person left it: its words and its pictures.
#[derive(Default)]
struct Draft {
    text: String,
    pictures: Vec<Rc<Attached>>,
}

/// The Aide chat showing ([`open_aide`]): a bot's, by its job, or the Chief of Staff's.
#[derive(Clone, PartialEq, Debug)]
pub struct AideChat {
    pub job: Option<i64>,
    pub name: String,
}

#[derive(Default)]
struct State {
    view: RefCell<Option<Rc<View>>>,
    list: RefCell<Option<gtk::Box>>,
    current: Cell<Option<i64>>,
    /// The page shows an Aide chat, in the Aide space: no Threads panel, its own keys in the strip.
    aide: RefCell<Option<AideChat>>,
    /// The conversation's width as [`fit`] last saw it.
    width: Cell<i32>,
    working: Cell<bool>,
    /// The thread's agent is running, so a reply starts without a resume.
    live: Cell<bool>,
    /// Stop was pressed: the reply's end keeps what it wrote and says so.
    stopping: Cell<bool>,
    /// The newest message drawn, so an event already drawn by a read is not drawn twice.
    last: Cell<i64>,
    /// Bumped by every thread shown: an older one's history stops showing in steps ([`show_history`]).
    shown_serial: Cell<u64>,
    /// The local day of the last message drawn, for the separators between days.
    last_day: RefCell<String>,
    pending: RefCell<String>,
    /// How much of `pending` its finished labels hold (up to and including a line break): only
    /// the text after it is set again as words arrive.
    pending_kept: Cell<usize>,
    /// How much of `pending` shows, and whether a frame is due to show the rest.
    pending_shown: Cell<usize>,
    pending_due: Cell<bool>,
    /// Tool calls drawn in this thread, by id.
    tools: RefCell<HashMap<String, Tool>>,
    turn: RefCell<Option<Turn>>,
    /// The newest message the person sent: its Edit key, its words and its pictures, which Try
    /// again sends again.
    last_user: RefCell<Option<(gtk::Button, String, Vec<Value>)>>,
    /// The Try again key under the last error or stopped reply, while nothing came after it.
    retry: RefCell<Option<gtk::Button>>,
    /// The run of reads the next read joins, while it is the last thing in its turn.
    group: RefCell<Option<Group>>,
    /// Entries this thread added, marked in the panel.
    added: RefCell<HashSet<i64>>,
    panel_tab: Cell<&'static str>,
    /// The panel's source, `tally`, `arbiter` or `avex`, and Arbiter's and Avex's tabs.
    source: Cell<&'static str>,
    arbiter_tab: Cell<&'static str>,
    avex_tab: Cell<&'static str>,
    /// Bumped by every panel read: only the newest one draws.
    panel_serial: Cell<u64>,
    /// `None` until the person opens or closes the panel; until then it follows the page width.
    panel_pinned: Cell<Option<bool>>,
    /// The panel's width as last dragged (0 for [`PANEL_WIDTH`]), and the write of a new one,
    /// waiting for the drag to settle ([`keep_width`]).
    panel_width: Cell<i32>,
    width_due: RefCell<Option<glib::SourceId>>,
    /// What the last run kept (`native.threads`) has been read: until then nothing is kept, so a
    /// default never writes over it ([`restore`]).
    restored: Cell<bool>,
    /// Overview shows every budget, not the first [`BUDGETS_SHOWN`].
    all_budgets: Cell<bool>,
    /// A budget's editor is open over the panel: a read waits for it to close ([`edit_budget`]),
    /// and `panel_stale` says one is waiting.
    editing_budget: Cell<bool>,
    panel_stale: Cell<bool>,
    narrow_page: Cell<bool>,
    listing: Cell<bool>,
    /// The list changed while it was being read: read it again.
    list_dirty: Cell<bool>,
    /// The list changed while it was off screen: read it when it shows.
    list_stale: Cell<bool>,
    /// Threads seen working, and those that finished while another showed.
    was_working: RefCell<HashSet<i64>>,
    unread: RefCell<HashSet<i64>>,
    /// The questions a new thread offers, and whether they stack in one column.
    suggested: RefCell<Vec<(String, String)>>,
    one_column: Cell<bool>,
    /// Claude Code is not on this PC: a thread cannot answer, so nothing is sent.
    claude_missing: Cell<bool>,
    /// What [`tell`] could not send while the agent was still answering in its thread: (thread,
    /// words), oldest first. Each goes when its thread's turn ends ([`tell_waiting`]).
    to_tell: RefCell<Vec<(i64, String)>>,
    /// What the message box held in each thread left with words or pictures in it, `None` a new
    /// thread's: they come back when that thread shows again ([`reset`]).
    drafts: RefCell<HashMap<Option<i64>, Draft>>,
    /// The pictures in the message box now, in order.
    pictures: RefCell<Vec<Rc<Attached>>>,
    ui: RefCell<std::rc::Weak<Ui>>,
    /// The model and effort a new thread starts with: the last ones chosen.
    choice: RefCell<(String, String)>,
    /// The model the picker shows now.
    shown_model: RefCell<String>,
    /// The models on this PC (`thread.models`): (id, name). Only those whose file is there.
    local_models: RefCell<Vec<(String, String)>>,
    /// The local model server's last word (`thread.local`): (model, state).
    local_state: RefCell<(String, String)>,
}

thread_local! {
    static STATE: State = State::default();
}

/// Remember the window, for the page's own keys: `money::install` calls it.
pub fn install(ui: &Rc<Ui>) {
    STATE.with(|s| *s.ui.borrow_mut() = Rc::downgrade(ui));
    // Display smoke runs (RELAY_NATIVE_SCREENSHOT) open a thread by id to capture it, and show a
    // panel source by key (RELAY_NATIVE_SOURCE=avex).
    if std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_some() {
        if let Some(source) = std::env::var("RELAY_NATIVE_SOURCE").ok().and_then(|k| SOURCES.iter().find(|(key, _, _)| *key == k)).map(|(key, _, _)| *key) {
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || set_source(source));
        }
        if let Some(id) = std::env::var("RELAY_NATIVE_THREAD").ok().and_then(|id| id.parse::<i64>().ok()) {
            let weak = Rc::downgrade(ui);
            glib::timeout_add_local_once(std::time::Duration::from_millis(2600), move || {
                if let Some(ui) = weak.upgrade() {
                    open(&ui, id);
                }
            });
        }
        // RELAY_NATIVE_PANEL picks the Tally panel's view (`entries`, `invest`), shows every budget
        // (`budgets`) or opens the first one's editor (`budget`); RELAY_NATIVE_DRAFT drafts words
        // into the thread opened.
        if let Ok(what) = std::env::var("RELAY_NATIVE_PANEL") {
            glib::timeout_add_local_once(std::time::Duration::from_millis(3400), move || smoke_panel(&what));
        }
        if let Ok(words) = std::env::var("RELAY_NATIVE_DRAFT") {
            let weak = Rc::downgrade(ui);
            glib::timeout_add_local_once(std::time::Duration::from_millis(3800), move || {
                if let Some(ui) = weak.upgrade() {
                    draft(&ui, &words);
                }
            });
        }
    }
}

/// A display smoke run's panel view ([`install`]): a Tally tab by key, or a budgets key.
fn smoke_panel(what: &str) {
    let Some(v) = view() else { return };
    if let Some((_, tab)) = v.panel_tabs.iter().find(|(key, _)| *key == what) {
        tab.emit_clicked();
        return;
    }
    let class = if what == "budgets" { "threads-more" } else { "threads-budget-key" };
    if let Some(key) = first_of(v.panel_body.upcast_ref(), class) {
        key.emit_clicked();
    }
}

/// The first key under `root` with `class`, depth first.
fn first_of(root: &gtk::Widget, class: &str) -> Option<gtk::Button> {
    let mut child = root.first_child();
    while let Some(w) = child {
        if w.has_css_class(class) {
            return w.downcast::<gtk::Button>().ok();
        }
        if let Some(found) = first_of(&w, class) {
            return Some(found);
        }
        child = w.next_sibling();
    }
    None
}

fn the_ui() -> Option<Rc<Ui>> {
    STATE.with(|s| s.ui.borrow().upgrade())
}

/// The window, for keys built before it existed (the Tally tabs).
pub fn the_ui_pub() -> Option<Rc<Ui>> {
    the_ui()
}

fn view() -> Option<Rc<View>> {
    STATE.with(|s| s.view.borrow().clone())
}

/// The id of the thread showing, if one is.
pub fn current() -> Option<i64> {
    STATE.with(|s| s.current.get())
}

/// The conversation's scroller: charts outside it wait to be read (`chart::refresh_all`).
pub fn chart_viewport() -> Option<gtk::Widget> {
    view().map(|v| v.scroll.clone().upcast())
}

/// The message pictures on screen in the conversation, by widget name, and whether each shows yet;
/// `None` until the conversation is taller than its window and has landed on its newest message.
/// For `scripts/perf/thumbs-reopen.py`.
pub(crate) fn pictures_on_screen() -> Option<Vec<(String, bool)>> {
    let v = view()?;
    let adj = v.scroll.vadjustment();
    if adj.upper() <= adj.page_size() || !at_bottom(&adj) {
        return None;
    }
    let mut found = Vec::new();
    let mut stack = vec![v.column.clone().upcast::<gtk::Widget>()];
    while let Some(widget) = stack.pop() {
        if widget.widget_name().starts_with("thread-picture-") {
            let shown = widget.compute_bounds(&v.scroll).is_some_and(|r| r.y() + r.height() > 0.0 && r.y() < v.scroll.height() as f32);
            if shown {
                let mut painted = false;
                let mut inner = widget.first_child();
                while let Some(child) = inner {
                    inner = child.first_child().or_else(|| child.next_sibling());
                    if let Some(picture) = child.downcast_ref::<gtk::Picture>() {
                        painted = picture.paintable().is_some();
                        break;
                    }
                }
                found.push((widget.widget_name().to_string(), painted));
            }
            continue;
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            stack.push(c);
        }
    }
    Some(found)
}

/// The page, built once and added to the window's stack by `money::add_pages`.
pub fn page() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("threads-root");

    // The strip above the conversation: the thread's name (its menu renames and deletes) and
    // the Tally panel's key.
    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    strip.add_css_class("workspace-strip");
    strip.add_css_class("threads-strip");
    let title = label("New thread", "threads-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(48);
    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    title_row.append(&title);
    title_row.append(&crate::icons::image("chevron-down", 12));
    let title_key = gtk::MenuButton::new();
    title_key.set_child(Some(&title_row));
    title_key.add_css_class("threads-title-key");
    title_key.set_widget_name("threads-title");
    // 34px and centred in the strip, as the Dev view's project pill: filling the strip's 52px it
    // ran down onto the conversation's first line.
    title_key.set_valign(gtk::Align::Center);
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 6);
    menu.add_css_class("threads-menu");
    let rename = gtk::Entry::new();
    rename.set_placeholder_text(Some("Thread name"));
    rename.set_width_chars(28);
    menu.append(&label("Rename", "money-label"));
    menu.append(&rename);
    let delete = button("Delete thread", "money-destructive");
    menu.append(&delete);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu));
    title_key.set_popover(Some(&popover));
    strip.append(&title_key);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    strip.append(&spacer);
    let aide_keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    aide_keys.add_css_class("aide-strip");
    aide_keys.set_valign(gtk::Align::Center);
    aide_keys.set_visible(false);
    strip.append(&aide_keys);
    // A toggle, so it shows whether the panel is open.
    let panel_key = gtk::ToggleButton::new();
    panel_key.set_child(Some(&crate::icons::image("sidebar", 16)));
    panel_key.add_css_class("quiet");
    panel_key.add_css_class("icon-key");
    panel_key.add_css_class("threads-panel-key");
    panel_key.set_widget_name("threads-panel-key");
    panel_key.set_active(true);
    panel_key.set_tooltip_text(Some("Hide the panel"));
    panel_key.update_property(&[gtk::accessible::Property::Label("Panel")]);
    strip.append(&panel_key);
    root.append(&strip);

    // The conversation and the Tally panel share a split the person can drag.
    let body = gtk::Paned::new(gtk::Orientation::Horizontal);
    body.add_css_class("threads-split");
    body.set_vexpand(true);
    let conversation = gtk::Box::new(gtk::Orientation::Vertical, 0);
    conversation.set_hexpand(true);

    let middle = gtk::Stack::new();
    middle.set_vexpand(true);
    middle.set_transition_type(gtk::StackTransitionType::Crossfade);
    middle.set_transition_duration(120);

    // A new thread: the greeting, the month in a line, the message box, and questions to start.
    // Its width is the conversation's (`fit`); it scrolls when the window is short.
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 22);
    empty.add_css_class("threads-empty");
    empty.set_valign(gtk::Align::Center);
    let greeting = label(&super::greeting(), "threads-greeting");
    greeting.set_xalign(0.5);
    greeting.set_wrap(true);
    greeting.set_justify(gtk::Justification::Center);
    let empty_line = label("", "threads-greeting-line");
    empty_line.set_xalign(0.5);
    empty_line.set_wrap(true);
    empty_line.set_justify(gtk::Justification::Center);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
    words.append(&greeting);
    words.append(&empty_line);
    empty.append(&words);
    let empty_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty.append(&empty_slot);
    let suggestions = gtk::Grid::new();
    suggestions.set_row_spacing(8);
    suggestions.set_column_spacing(8);
    suggestions.set_column_homogeneous(true);
    empty.append(&suggestions);
    let empty_scroll = crate::app::scrolled(&empty);
    empty_scroll.set_policy(gtk::PolicyType::External, gtk::PolicyType::Automatic);
    empty_scroll.add_css_class("threads-scroll");
    middle.add_named(&empty_scroll, Some("empty"));

    // The open thread: its messages in a centred column, scrolled, with a key back to the newest.
    let column = gtk::Box::new(gtk::Orientation::Vertical, 22);
    column.add_css_class("threads-column");
    let scroll = crate::app::scrolled(&column);
    scroll.set_policy(gtk::PolicyType::External, gtk::PolicyType::Automatic);
    scroll.add_css_class("threads-scroll");
    let reading = gtk::Overlay::new();
    reading.set_child(Some(&scroll));
    let jump = gtk::Button::new();
    jump.set_child(Some(&crate::icons::image("arrow-down", 16)));
    jump.add_css_class("threads-jump");
    jump.set_halign(gtk::Align::Center);
    jump.set_valign(gtk::Align::End);
    jump.set_margin_bottom(12);
    jump.set_tooltip_text(Some("Jump to the newest message"));
    jump.update_property(&[gtk::accessible::Property::Label("Jump to the newest message")]);
    jump.set_visible(false);
    reading.add_overlay(&jump);
    middle.add_named(&reading, Some("thread"));
    // The page's toast, as a Tally page's (`money::toast`): what a sheet saved or deleted, with
    // Undo. It sits over the conversation's foot, never over the message box.
    let foot = gtk::Overlay::new();
    foot.set_vexpand(true);
    foot.set_child(Some(&middle));
    super::add_toast(&foot, PAGE, 12);
    conversation.append(&foot);

    let pending = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pending.append(&pending_label());
    let working = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    working.add_css_class("threads-working");
    working.append(&crate::app::spinner());
    let working_text = label("Working…", "threads-working-text");
    working.append(&working_text);

    // The message box: what you write, the model and effort, what the agent may do, and send or
    // stop. Where it looks is the panel's source switch, not chosen here a second time.
    let composer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    composer.add_css_class("threads-composer");
    let input = gtk::TextView::new();
    input.set_wrap_mode(gtk::WrapMode::WordChar);
    input.add_css_class("threads-input");
    input.set_widget_name("threads-input");
    input.set_accepts_tab(false);
    input.update_property(&[gtk::accessible::Property::Label("Message")]);
    let input_scroll = gtk::ScrolledWindow::builder().child(&input).propagate_natural_height(true).max_content_height(180).build();
    input_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    // The placeholder: a text view has none of its own.
    let field = gtk::Overlay::new();
    field.set_child(Some(&input_scroll));
    let hint = label("What do you want to know?", "threads-hint");
    hint.set_can_target(false);
    hint.set_valign(gtk::Align::Start);
    hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    field.add_overlay(&hint);
    // The pictures going with the message, above the words; they scroll sideways in a narrow box
    // rather than hold the window wide.
    let chips = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    chips.add_css_class("threads-chips");
    let chips_scroll = gtk::ScrolledWindow::builder().child(&chips).propagate_natural_height(true).build();
    chips_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    chips_scroll.set_visible(false);
    composer.append(&chips_scroll);
    composer.append(&field);
    let picture_note = label("", "threads-picture-note");
    picture_note.set_wrap(true);
    picture_note.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    picture_note.set_visible(false);
    composer.append(&picture_note);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // Pictures, for a model that reads them ([`picture_key_shown`]).
    let picture_key = crate::app::icon_button("file-image", "Add a picture");
    picture_key.add_css_class("threads-picture-key");
    picture_key.set_widget_name("threads-picture");
    picture_key.set_valign(gtk::Align::Center);
    row.append(&picture_key);
    let model = picker("threads-model", MODEL_ABOUT, &MODELS, |value| choose(Some(value), None));
    row.append(&model.key);
    let effort = picker("threads-effort", "How hard the agent thinks before answering", &EFFORTS, |value| choose(None, Some(value)));
    row.append(&effort.key);
    // What the agent may do, said plainly: words, not a key.
    let limits = label("Edits with Undo · asks first", "threads-limits");
    limits.set_valign(gtk::Align::Center);
    limits.set_tooltip_text(Some("It adds and changes entries and investment activity, each with an Undo. Before deleting, changing many at once, budgets or accounts, it asks you on a card."));
    row.append(&limits);
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    row.append(&gap);
    let send_key = gtk::Button::new();
    send_key.add_css_class("primary");
    send_key.add_css_class("threads-send");
    send_key.set_widget_name("threads-send");
    send_key.set_child(Some(&crate::icons::image_with_stroke("arrow-up", 15, 2.0)));
    send_key.set_tooltip_text(Some("Send · Enter"));
    send_key.update_property(&[gtk::accessible::Property::Label("Send")]);
    send_key.set_valign(gtk::Align::Center);
    row.append(&send_key);
    composer.append(&row);

    // Above the composer when a thread cannot answer: why, and where to fix it.
    let notice = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    notice.add_css_class("threads-notice");
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("threads-notice-lamp");
    lamp.set_valign(gtk::Align::Center);
    notice.append(&lamp);
    let notice_text = label("", "threads-notice-text");
    notice_text.set_wrap(true);
    notice_text.set_hexpand(true);
    notice.append(&notice_text);
    let settings_key = button("Open Settings", "quiet");
    settings_key.set_valign(gtk::Align::Center);
    // On Agents, where Claude Code's row and its refresh key are.
    settings_key.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            crate::tools::settings::open_on(&ui, "agents");
        }
    });
    notice.append(&settings_key);
    notice.set_visible(false);
    let message_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    message_box.append(&notice);
    message_box.append(&composer);
    // Pictures dropped on the message box go with the message, before its text field would type
    // their names; dropped anywhere else on the conversation, too.
    composer.add_controller(picture_drop(true));
    conversation.add_controller(picture_drop(false));

    // An open thread's message box sits under the conversation, as wide as its column; its
    // scroller keeps the pickers' width from holding the window open.
    let dock_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dock_slot.add_css_class("threads-dock");
    let dock = gtk::ScrolledWindow::builder().child(&dock_slot).propagate_natural_height(true).build();
    dock.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
    dock.add_css_class("threads-dock-scroll");
    conversation.append(&dock);
    body.set_start_child(Some(&conversation));
    body.set_resize_start_child(true);
    body.set_shrink_start_child(false);

    // The panel: 340 pixels as it first opens, then as wide as it was left; 300 at the least.
    // Its source, Tally, Arbiter or Avex, above that source's tabs: the one switch that says
    // where the thread looks.
    let side = gtk::Box::new(gtk::Orientation::Vertical, 18);
    side.add_css_class("threads-panel");
    side.set_size_request(PANEL_MIN, -1);
    let source_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    source_row.add_css_class("threads-sources");
    let mut sources = Vec::new();
    for (key, caption, about) in SOURCES {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("threads-source-{key}"));
        tab.set_tooltip_text(Some(about));
        source_row.append(&tab);
        sources.push((key, tab));
    }
    side.append(&source_row);
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("view-tabs");
    // Each tab as wide as its caption needs, the spare width shared.
    tabs.set_homogeneous(false);
    let mut panel_tabs = Vec::new();
    for (key, caption) in PANEL_TABS {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("threads-panel-{key}"));
        tab.set_hexpand(true);
        tabs.append(&tab);
        panel_tabs.push((key, tab));
    }
    side.append(&tabs);
    let arbiter_tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    arbiter_tabs.add_css_class("view-tabs");
    arbiter_tabs.set_homogeneous(true);
    arbiter_tabs.set_visible(false);
    let mut arbiter_panel_tabs = Vec::new();
    for (key, caption) in super::arbiter::PANEL_TABS {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("threads-panel-{key}"));
        arbiter_tabs.append(&tab);
        arbiter_panel_tabs.push((key, tab));
    }
    side.append(&arbiter_tabs);
    let avex_tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    avex_tabs.add_css_class("view-tabs");
    avex_tabs.set_homogeneous(true);
    avex_tabs.set_visible(false);
    let mut avex_panel_tabs = Vec::new();
    for (key, caption) in super::avex::PANEL_TABS {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("threads-panel-{key}"));
        avex_tabs.append(&tab);
        avex_panel_tabs.push((key, tab));
    }
    side.append(&avex_tabs);
    let panel_body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let panel_scroll = crate::app::scrolled(&panel_body);
    panel_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    side.append(&panel_scroll);
    body.set_end_child(Some(&side));
    body.set_resize_end_child(false);
    body.set_shrink_end_child(false);
    // Place the split once the page has a width, the panel as wide as it was left; a drag is
    // kept for the next run.
    body.add_tick_callback(|split, _| {
        if split.width() <= 0 {
            return glib::ControlFlow::Continue;
        }
        split.set_position((split.width() - panel_width()).max(0));
        glib::ControlFlow::Break
    });
    body.connect_position_notify(|_| keep_width());
    let panel = side.clone();
    root.append(&body);

    // Escape stops a reply wherever the focus is on the page; a popover's own Escape comes first.
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(|_, key, _, _| {
        if key == gtk::gdk::Key::Escape && STATE.with(|s| s.working.get()) {
            if let Some(ui) = the_ui() {
                stop(&ui);
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    root.add_controller(escape);

    // The conversation follows its own width; the panel steps aside on a narrow page.
    crate::app::watch_width(&conversation, |width| {
        if let Some(v) = view() {
            fit(&v, width);
        }
    });
    crate::app::watch_width(&root, |width| {
        STATE.with(|s| s.narrow_page.set(width < PANEL_FROM));
        if let Some(v) = view() {
            apply_panel(&v);
        }
    });

    let built = Rc::new(View {
        title, title_key, aide_keys, rename, middle, empty, empty_slot, empty_line, suggestions, dock, dock_slot, message_box,
        notice, notice_text, composer, input, hint, chips, chips_scroll, picture_key, picture_note, send: send_key,
        limits, model, effort, scroll, jump, column, pending,
        working, working_text, split: body.clone(), panel, panel_key, panel_body, panel_tabs,
        tally_tabs: tabs, arbiter_tabs, arbiter_panel_tabs, avex_tabs, avex_panel_tabs, sources,
    });
    STATE.with(|s| {
        s.panel_tab.set("overview");
        s.source.set("tally");
        s.arbiter_tab.set(super::arbiter::PANEL_TABS[0].0);
        s.avex_tab.set(super::avex::PANEL_TABS[0].0);
        *s.suggested.borrow_mut() = SUGGESTIONS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        *s.view.borrow_mut() = Some(built.clone());
    });
    fit(&built, 760);
    fill_suggestions(&built);
    wire(&built, &delete);
    wire_sources(&built);
    show_source(&built);
    show_empty(&built);
    root
}

/// The panel's source switch, and Arbiter's and Avex's tabs.
fn wire_sources(v: &Rc<View>) {
    for (key, tab) in &v.avex_panel_tabs {
        let key = *key;
        tab.connect_clicked(move |_| {
            STATE.with(|s| s.avex_tab.set(key));
            if let Some(ui) = the_ui() {
                refresh_panel(&ui);
            }
        });
    }
    for (key, tab) in &v.arbiter_panel_tabs {
        let key = *key;
        tab.connect_clicked(move |_| {
            STATE.with(|s| s.arbiter_tab.set(key));
            if let Some(ui) = the_ui() {
                refresh_panel(&ui);
            }
        });
    }
    for (key, tab) in &v.sources {
        let key = *key;
        tab.connect_clicked(move |_| set_source(key));
    }
}

/// Show `source`'s panel, open, and lit on the switch. A new thread's greeting and questions
/// follow it.
fn set_source(source: &'static str) {
    let Some(v) = view() else { return };
    let changed = STATE.with(|s| s.source.replace(source)) != source;
    show_source(&v);
    if !v.panel_key.is_active() {
        v.panel_key.set_active(true);
    }
    if changed {
        keep("source", json!(source));
    }
    let Some(ui) = the_ui() else { return };
    refresh_panel(&ui);
    if changed && current().is_none() {
        glib::spawn_future_local(async move { greet(&ui).await });
    }
}

/// The source showing, on the switch and the tabs, and what the agent may do alone.
fn show_source(v: &View) {
    let source = STATE.with(|s| s.source.get());
    show_hint(v, current().is_none());
    for (key, tab) in &v.sources {
        if *key == source {
            tab.add_css_class("selected");
        } else {
            tab.remove_css_class("selected");
        }
    }
    v.tally_tabs.set_visible(source == "tally");
    v.arbiter_tabs.set_visible(source == "arbiter");
    v.avex_tabs.set_visible(source == "avex" && super::avex::PANEL_TABS.len() > 1);
    if source == "arbiter" {
        v.limits.set_text("Proposes, you approve");
        v.limits.set_tooltip_text(Some("It reads, backtests and proposes. Orders follow each strategy's mode, always within its limits, and only you change those."));
    } else if source == "avex" {
        v.limits.set_text("Reads only");
        v.limits.set_tooltip_text(Some("It reads your training as last exported from Avex. It cannot change anything in Avex."));
    } else {
        v.limits.set_text("Edits with Undo · asks first");
        v.limits.set_tooltip_text(Some("It adds and changes entries and investment activity, each with an Undo. Before deleting, changing many at once, budgets or accounts, it asks you on a card."));
    }
}

fn wire(v: &Rc<View>, delete: &gtk::Button) {
    // Enter sends; Shift+Enter is a new line; Escape stops a reply.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) && !modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
            if let Some(ui) = the_ui() {
                submit(&ui);
            }
            return glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::Escape && STATE.with(|s| s.working.get()) {
            if let Some(ui) = the_ui() {
                stop(&ui);
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    v.input.add_controller(keys);
    v.input.buffer().connect_changed(|_| {
        if let Some(v) = view() {
            update_send(&v);
        }
    });
    // A picture on the clipboard, or picture files copied in a file manager, go with the message.
    v.input.connect_paste_clipboard(|input| {
        if paste_pictures(input) {
            input.stop_signal_emission_by_name("paste-clipboard");
        }
    });
    v.picture_key.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            choose_pictures(&ui);
        }
    });
    v.send.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            if STATE.with(|s| s.working.get()) {
                stop(&ui);
            } else {
                submit(&ui);
            }
        }
    });
    v.rename.connect_activate(|entry| {
        let title = entry.text().trim().to_string();
        let (Some(ui), Some(id)) = (the_ui(), current()) else { return };
        if title.is_empty() {
            return;
        }
        if let Some(popover) = view().and_then(|v| v.title_key.popover()) {
            popover.popdown();
        }
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("thread.rename", json!({"id": id, "title": title})).await {
                ui.show_error(&e.to_string());
            }
        });
    });
    crate::app::confirm_inline(delete, "Delete this thread?", |_| {
        let (Some(ui), Some(id)) = (the_ui(), current()) else { return };
        if let Some(popover) = view().and_then(|v| v.title_key.popover()) {
            popover.popdown();
        }
        glib::spawn_future_local(async move {
            match ui.call("thread.delete", json!({"id": id})).await {
                Ok(_) => new_thread(&ui),
                Err(e) => ui.show_error(&e.to_string()),
            }
        });
    });
    // The person's choice holds over the page width from then on.
    v.panel_key.connect_toggled(|key| {
        let Some(v) = view() else { return };
        if key.is_active() == v.panel.is_visible() {
            return; // `apply_panel` set it to match
        }
        STATE.with(|s| s.panel_pinned.set(Some(key.is_active())));
        apply_panel(&v);
    });
    for (key, tab) in &v.panel_tabs {
        let key = *key;
        tab.connect_clicked(move |_| {
            if STATE.with(|s| s.panel_tab.replace(key)) == key {
                return;
            }
            keep("panel_tab", json!(key));
            if let Some(v) = view() {
                clear(&v.panel_body);
            }
            if let Some(ui) = the_ui() {
                refresh_panel(&ui);
            }
        });
    }
    // The name shows selected, so typing replaces it.
    v.title_key.connect_notify_local(Some("active"), |key, _| {
        if key.is_active() {
            if let Some(v) = view() {
                v.rename.set_text(&v.title.text());
                v.rename.grab_focus();
                v.rename.select_region(0, -1);
            }
        }
    });
    // Away from the newest message, a key leads back to it; charts that come into view read the
    // ledger they missed.
    let jump = v.jump.clone();
    v.scroll.vadjustment().connect_value_changed(move |adj| {
        jump.set_visible(!at_bottom(adj));
        super::chart::load_stale();
    });
    // The page comes back into view: charts the ledger moved under meanwhile read again, without
    // waiting for a scroll. The next idle has them laid out.
    v.scroll.connect_map(|_| {
        glib::idle_add_local_once(super::chart::load_stale);
    });
    v.jump.connect_clicked(|_| {
        if let Some(v) = view() {
            stick_to_bottom(&v, true);
        }
    });
}

/// Size the conversation to its width: the column, the message box and a new thread's block share
/// one width, at most [`COLUMN_MAX`], centred between gutters that narrow on a narrow page. They are
/// placed by margins, not requests, so their content never widens them. What matters least in
/// the message box steps aside first, and the questions stack under 600.
fn fit(v: &View, width: i32) {
    let gutter = if width < 640 { 16 } else { 28 };
    let inner = (width - 2 * gutter).clamp(240, COLUMN_MAX);
    let start = ((width - inner) / 2).max(0);
    let end = (width - inner - start).max(0);
    for part in [v.column.upcast_ref::<gtk::Widget>(), v.empty.upcast_ref(), v.dock_slot.upcast_ref()] {
        part.set_margin_start(start);
        part.set_margin_end(end);
    }
    STATE.with(|s| s.width.set(width));
    v.limits.set_visible(width >= 640 && aide().is_none());
    v.effort.key.set_visible(width >= 420);
    let one = width < 600;
    if STATE.with(|s| s.one_column.replace(one)) != one {
        fill_suggestions(v);
    }
}

/// Show or hide the Tally panel: the person's choice, else whether the page is wide enough.
fn apply_panel(v: &View) {
    // An Aide chat has no Threads panel: Tally, Arbiter and Avex are not what it is about.
    let open = aide().is_none() && STATE.with(|s| s.panel_pinned.get().unwrap_or(!s.narrow_page.get()));
    let was = v.panel.is_visible();
    v.panel.set_visible(open);
    if v.panel_key.is_active() != open {
        v.panel_key.set_active(open);
    }
    v.panel_key.set_tooltip_text(Some(if open { "Hide the panel" } else { "Show the panel" }));
    if open && !was {
        if let Some(ui) = the_ui() {
            refresh_panel(&ui);
        }
    }
}

/// The panel's width: as last dragged, else [`PANEL_WIDTH`].
fn panel_width() -> i32 {
    match STATE.with(|s| s.panel_width.get()) {
        0 => PANEL_WIDTH,
        width => width,
    }
}

/// Keeps the panel's width once a drag of the split settles. A window resized moves the split
/// too, but leaves the panel as wide as it was, so nothing new is kept.
fn keep_width() {
    if !STATE.with(|s| s.restored.get()) {
        return;
    }
    if let Some(due) = STATE.with(|s| s.width_due.take()) {
        due.remove();
    }
    let due = glib::timeout_add_local_once(std::time::Duration::from_millis(500), || {
        // It has fired: forget it rather than remove it.
        STATE.with(|s| s.width_due.take());
        let Some(v) = view().filter(|v| v.panel.is_visible() && v.split.width() > 0) else { return };
        let width = v.split.width() - v.split.position();
        if width >= PANEL_MIN && width != panel_width() {
            STATE.with(|s| s.panel_width.set(width));
            keep("panel_width", json!(width));
        }
    });
    STATE.with(|s| *s.width_due.borrow_mut() = Some(due));
}

/// Keeps `key` under `native.threads` for the next run ([`restore`]): nothing before the last
/// run's are read. A write that fails is let go, as the notes window's are: it only remembers.
fn keep(key: &str, value: Value) {
    let Some(ui) = the_ui().filter(|_| STATE.with(|s| s.restored.get())) else { return };
    let path = format!("native.threads.{key}");
    glib::spawn_future_local(async move {
        let _ = ui.call("settings.set", json!({"path": path, "value": value})).await;
    });
}

/// What the last run left (`native.threads`): the panel's source, the Tally panel's tab and the
/// panel's width, and the model and effort a new thread starts with. Read once, after the first
/// connection (`money::startup`); from then on each is kept as it changes ([`keep`]).
pub fn restore(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        // Unread, the defaults stand, and what the person changes from here is still kept.
        let kept = ui.call("settings.get", json!({"path": "native.threads"})).await.map(|v| v["value"].clone()).unwrap_or_default();
        let source = SOURCES.iter().map(|(key, _, _)| *key).find(|key| kept["source"].as_str() == Some(*key));
        let tab = PANEL_TABS.iter().map(|(key, _)| *key).find(|key| kept["panel_tab"].as_str() == Some(*key));
        let source_moved = source.is_some_and(|key| STATE.with(|s| s.source.replace(key)) != key);
        let tab_moved = tab.is_some_and(|key| STATE.with(|s| s.panel_tab.replace(key)) != key);
        if let Some(width) = kept["panel_width"].as_i64() {
            let width = width.clamp(i64::from(PANEL_MIN), 2000) as i32;
            STATE.with(|s| s.panel_width.set(width));
            // Placed already, it moves; otherwise the first frame places it so.
            if let Some(v) = view().filter(|v| v.split.width() > 0) {
                v.split.set_position((v.split.width() - width).max(0));
            }
        }
        let model = kept["model"].as_str().filter(|m| m.len() <= 64).map(str::to_string);
        let effort = kept["effort"].as_str().filter(|e| EFFORTS.iter().any(|(value, _)| value == e)).map(str::to_string);
        if model.is_some() || effort.is_some() {
            let (model, effort) = STATE.with(|s| {
                let mut choice = s.choice.borrow_mut();
                if let Some(m) = model {
                    choice.0 = m;
                }
                if let Some(e) = effort {
                    choice.1 = e;
                }
                choice.clone()
            });
            // A new thread shows them; an open one keeps showing its own.
            if let Some(v) = view().filter(|_| current().is_none()) {
                show_choice(&v, &model, &effort);
            }
        }
        STATE.with(|s| s.restored.set(true));
        if !(source_moved || tab_moved) {
            return;
        }
        if let Some(v) = view() {
            show_source(&v);
            clear(&v.panel_body);
        }
        // Off the page, its next showing reads them.
        if *ui.page.borrow() != PAGE {
            return;
        }
        refresh_panel(&ui);
        if source_moved && current().is_none() {
            greet(&ui).await;
        }
    });
}

/// The message box goes where the middle is: centred on a new thread, at the foot of an open one.
fn place_composer(v: &View, empty: bool) {
    let target = if empty { &v.empty_slot } else { &v.dock_slot };
    if v.message_box.parent().as_ref() != Some(target.upcast_ref()) {
        if let Some(parent) = v.message_box.parent().and_downcast::<gtk::Box>() {
            parent.remove(&v.message_box);
        }
        target.append(&v.message_box);
    }
    v.dock.set_visible(!empty);
    show_hint(v, empty);
}

/// The message box's placeholder: an open question on a new thread, then what the source showing
/// is about.
fn show_hint(v: &View, empty: bool) {
    let hint = match (aide(), STATE.with(|s| s.source.get())) {
        (Some(AideChat { job: None, .. }), _) => String::from("Message Aide, or ask for a new bot"),
        (Some(chat), _) => format!("Message {}", chat.name),
        (None, _) if empty => String::from("What do you want to know?"),
        (None, "avex") => String::from("Ask about your training"),
        (None, "arbiter") => String::from("Ask about your strategies and orders"),
        (None, _) => String::from("Ask about your money, or tell it what to change"),
    };
    v.hint.set_text(&hint);
    v.input.update_property(&[gtk::accessible::Property::Placeholder(&hint)]);
}

/// What the send key does now: stop a reply, or send what is written when there is something to
/// send and someone to answer it. The placeholder shows while the box is empty.
fn update_send(v: &View) {
    let buffer = v.input.buffer();
    v.hint.set_visible(buffer.char_count() == 0);
    let written = !buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().is_empty();
    let pictures = STATE.with(|s| !s.pictures.borrow().is_empty());
    let held = pictures_hold();
    let working = STATE.with(|s| s.working.get());
    let missing = STATE.with(|s| s.claude_missing.get());
    let reachable = the_ui().is_none_or(|ui| ui.is_connected()) && !missing;
    // Pictures alone are a message; one still going up, or refused, keeps it back and says why.
    v.send.set_sensitive(working || ((written || pictures) && reachable && held.is_none()));
    if !working {
        v.send.set_tooltip_text(Some(held.unwrap_or("Send · Enter")));
    }
    v.suggestions.set_sensitive(!missing);
}

/// What keeps the message box's pictures from going: a model on this PC, which cannot read them,
/// one still going up, or one refused. `None` when they may go, or there are none.
fn pictures_hold() -> Option<&'static str> {
    STATE.with(|s| {
        let pictures = s.pictures.borrow();
        if pictures.is_empty() {
            None
        } else if !pictures_allowed() {
            Some(pictures::LOCAL_CANNOT)
        } else if pictures.iter().any(|p| p.going()) {
            Some(STILL_GOING)
        } else if pictures.iter().any(|p| p.refused().is_some()) {
            Some(REFUSED_PICTURE)
        } else {
            None
        }
    })
}

/// Whether the model showing reads pictures: Claude's do, a model on this PC does not.
fn pictures_allowed() -> bool {
    !is_local(&STATE.with(|s| s.shown_model.borrow().clone()))
}

/// The picture key shows while the model reads pictures. On a model on this PC it steps aside,
/// and pictures already in the box say why they cannot go.
fn picture_key_shown(v: &View) {
    let allowed = pictures_allowed();
    v.picture_key.set_visible(allowed);
    let held = STATE.with(|s| !s.pictures.borrow().is_empty());
    if !allowed && held {
        note(v, pictures::LOCAL_CANNOT);
    } else if allowed && v.picture_note.text() == pictures::LOCAL_CANNOT {
        note(v, "");
    }
    update_send(v);
}

/// The message box's quiet line about its pictures; empty hides it.
fn note(v: &View, text: &str) {
    v.picture_note.set_text(text);
    v.picture_note.set_visible(!text.is_empty());
}

/// Show `pictures` in the message box, in place of what it held.
fn set_pictures(v: &View, pictures: Vec<Rc<Attached>>) {
    clear(&v.chips);
    for picture in &pictures {
        v.chips.append(picture.widget());
    }
    v.chips_scroll.set_visible(!pictures.is_empty());
    STATE.with(|s| *s.pictures.borrow_mut() = pictures);
    update_send(v);
}

/// Add pictures to the message box, as many as it has room for, each going up at once.
fn attach(sources: Vec<Source>) {
    let (Some(v), Some(ui)) = (view(), the_ui()) else { return };
    if sources.is_empty() {
        return;
    }
    if !pictures_allowed() {
        note(&v, pictures::LOCAL_CANNOT);
        return;
    }
    let mut held = STATE.with(|s| s.pictures.borrow().clone());
    let room = MAX_PICTURES.saturating_sub(held.len());
    let left_out = sources.len().saturating_sub(room);
    for source in sources.into_iter().take(room) {
        held.push(Attached::add(&ui, source, picture_done, remove_picture));
    }
    set_pictures(&v, held);
    let said = match (left_out, room) {
        (0, _) => String::new(),
        (_, 0) => format!("At most {MAX_PICTURES} pictures go in one message"),
        (n, _) => format!("At most {MAX_PICTURES} pictures go in one message: {} left out", plural(n as i64, "picture")),
    };
    note(&v, &said);
    v.input.grab_focus();
}

/// A picture went up, or was refused: the message box says so if it holds it. One in a draft put
/// away shows how it went on its chip when its thread comes back.
fn picture_done(picture: &Attached) {
    let Some(v) = view() else { return };
    if !STATE.with(|s| s.pictures.borrow().iter().any(|p| p.serial == picture.serial)) {
        return;
    }
    if let Some(why) = picture.refused() {
        note(&v, &format!("{} was not added: {why}", picture.name()));
    } else if v.picture_note.text() == STILL_GOING && pictures_hold() != Some(STILL_GOING) {
        note(&v, "");
    }
    update_send(&v);
}

/// A chip's remove key: the picture leaves the message box.
fn remove_picture(serial: u64) {
    let Some(v) = view() else { return };
    let left: Vec<Rc<Attached>> = STATE.with(|s| s.pictures.borrow().iter().filter(|p| p.serial != serial).cloned().collect());
    set_pictures(&v, left);
    // What it said is done with; that a model on this PC cannot read the others still holds.
    note(&v, if pictures_hold() == Some(pictures::LOCAL_CANNOT) { pictures::LOCAL_CANNOT } else { "" });
    v.input.grab_focus();
}

/// The picture key: a chooser of PNG, JPEG, GIF and WebP pictures, several at once.
fn choose_pictures(ui: &Rc<Ui>) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Pictures (PNG, JPEG, GIF, WebP)"));
    for mime in pictures::MEDIA_TYPES {
        filter.add_mime_type(mime);
    }
    for suffix in pictures::SUFFIXES {
        filter.add_suffix(suffix);
    }
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder().title("Add pictures").modal(true).filters(&filters).default_filter(&filter).build();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        // Closing the chooser is not an error.
        let Ok(files) = dialog.open_multiple_future(Some(&ui.window)).await else { return };
        let chosen = (0..files.n_items())
            .filter_map(|i| files.item(i).and_downcast::<gtk::gio::File>())
            .filter_map(|f| f.path())
            .map(Source::Path)
            .collect();
        attach(chosen);
    });
}

/// Ctrl+V in the message box with a picture on the clipboard, or picture files copied in a file
/// manager: they go with the message instead of their names. Whether it took the paste; words, and
/// anything Relay copied itself, paste as words.
fn paste_pictures(input: &gtk::TextView) -> bool {
    let clipboard = input.clipboard();
    if clipboard.is_local() {
        return false;
    }
    let formats = clipboard.formats();
    let files = formats.contains_type(gtk::gdk::FileList::static_type()) || formats.contain_mime_type("text/uri-list");
    let picture = formats.contains_type(gtk::gdk::Texture::static_type());
    let words = formats.contains_type(glib::GString::static_type()) || formats.contain_mime_type("text/plain") || formats.contain_mime_type("text/plain;charset=utf-8");
    if !files && !(picture && !words) {
        return false;
    }
    let input = input.downgrade();
    glib::spawn_future_local(async move {
        if files {
            if let Ok(value) = clipboard.read_value_future(gtk::gdk::FileList::static_type(), glib::Priority::DEFAULT).await {
                let found: Vec<Source> = value
                    .get::<gtk::gdk::FileList>()
                    .map(|list| list.files().iter().filter_map(|f| f.path()).filter(|p| pictures::is_picture(p)).map(Source::Path).collect())
                    .unwrap_or_default();
                if !found.is_empty() {
                    attach(found);
                    return;
                }
            }
        }
        if picture {
            if let Ok(Some(texture)) = clipboard.read_texture_future().await {
                attach(vec![Source::Texture(texture)]);
                return;
            }
        }
        // No picture after all: the words paste as they would have.
        if let Some(input) = input.upgrade() {
            input.buffer().paste_clipboard(&clipboard, None, input.is_editable());
        }
    });
    true
}

/// Pictures dropped on the conversation, or on the message box (`in_box`), whose text field would
/// otherwise type their names: its target listens first. Anything else dropped on the box goes in
/// as words, as the field would have put it; on the conversation it says what goes.
fn picture_drop(in_box: bool) -> gtk::DropTarget {
    let target = gtk::DropTarget::new(glib::Type::INVALID, gtk::gdk::DragAction::COPY);
    // A picture dragged out of a browser also offers its address as a file: the picture first.
    target.set_types(&[gtk::gdk::Texture::static_type(), gtk::gdk::FileList::static_type()]);
    if in_box {
        target.set_propagation_phase(gtk::PropagationPhase::Capture);
    } else {
        // The conversation's lights the message box while a drag is over it.
        target.connect_enter(|_, _, _| {
            if let Some(v) = view() {
                v.composer.add_css_class("threads-dropping");
            }
            gtk::gdk::DragAction::COPY
        });
        target.connect_leave(|_| {
            if let Some(v) = view() {
                v.composer.remove_css_class("threads-dropping");
            }
        });
    }
    target.connect_drop(move |_, value, _, _| {
        let Some(v) = view() else { return false };
        v.composer.remove_css_class("threads-dropping");
        if let Ok(texture) = value.get::<gtk::gdk::Texture>() {
            attach(vec![Source::Texture(texture)]);
            return true;
        }
        let Ok(list) = value.get::<gtk::gdk::FileList>() else { return false };
        let files = list.files();
        let (found, others): (Vec<PathBuf>, Vec<PathBuf>) = files.iter().filter_map(|f| f.path()).partition(|p| pictures::is_picture(p));
        if found.is_empty() && in_box {
            // A link, or a file that is not a picture: its address, as the text field takes one.
            let words: Vec<String> = files.iter().map(|f| f.path().map_or_else(|| f.uri().to_string(), |p| p.to_string_lossy().to_string())).collect();
            if words.is_empty() {
                return false;
            }
            v.input.buffer().insert_at_cursor(&words.join(" "));
            return true;
        }
        if found.is_empty() {
            note(&v, ONLY_PICTURES);
            return false;
        }
        attach(found.into_iter().map(Source::Path).collect());
        if !others.is_empty() && v.picture_note.text().is_empty() {
            note(&v, &format!("{ONLY_PICTURES}: {} left out", plural(others.len() as i64, "file")));
        }
        true
    });
    target
}

/// A chip that opens a list of `options` and runs `pick` with the one chosen. The current choice
/// carries a check ([`show_choice`]).
fn picker(name: &str, about: &str, options: &'static [(&'static str, &'static str)], pick: fn(&str)) -> Picker {
    let key = gtk::MenuButton::new();
    key.add_css_class("threads-picker");
    key.set_widget_name(name);
    key.set_tooltip_text(Some(about));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let shown = label(options[0].1, "");
    row.append(&shown);
    row.append(&crate::icons::image("chevron-down", 10));
    key.set_child(Some(&row));
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.add_css_class("threads-menu");
    let popover = gtk::Popover::new();
    let marks = options.iter().map(|(value, caption)| {
        let (item, mark) = option(name, value, caption, &popover, pick);
        list.append(&item);
        (value.to_string(), mark)
    }).collect();
    let local = gtk::Box::new(gtk::Orientation::Vertical, 2);
    local.set_visible(false);
    list.append(&local);
    popover.set_child(Some(&list));
    key.set_popover(Some(&popover));
    Picker { key, shown, marks: RefCell::new(marks), popover, pick, local }
}

/// One choice in a picker's list, and its check.
fn option(name: &str, value: &str, caption: &str, popover: &gtk::Popover, pick: fn(&str)) -> (gtk::Button, gtk::Image) {
    let item = button("", "nav");
    item.set_widget_name(&format!("{name}-{}", if value.is_empty() { "default" } else { value }));
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let mark = crate::icons::image("check", 13);
    mark.set_opacity(0.0);
    line.append(&mark);
    line.append(&label(caption, ""));
    item.set_child(Some(&line));
    let popover = popover.downgrade();
    let value = value.to_string();
    item.connect_clicked(move |_| {
        if let Some(p) = popover.upgrade() {
            p.popdown();
        }
        pick(&value);
    });
    (item, mark)
}

/// The models on this PC (`thread.models`), offered under the provider's in the model picker
/// with how long one may sit unused. Absent until one's file is in the models folder.
fn load_local(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let Ok(found) = ui.call("thread.models", json!({})).await else { return };
        let models: Vec<(String, String)> = found["models"].as_array().into_iter().flatten()
            .filter(|m| m["present"] == true)
            .filter_map(|m| Some((m["id"].as_str()?.to_string(), m["name"].as_str()?.to_string())))
            .collect();
        if let Some(running) = found["running"].as_str() {
            let state = if found["ready"] == true { "ready" } else { "loading" };
            STATE.with(|s| *s.local_state.borrow_mut() = (running.to_string(), state.to_string()));
        }
        let unchanged = STATE.with(|s| *s.local_models.borrow() == models);
        STATE.with(|s| *s.local_models.borrow_mut() = models.clone());
        let Some(v) = view() else { return };
        if !unchanged || v.model.local.first_child().is_none() {
            fill_local(&v, &models, found["idle_minutes"].as_u64().unwrap_or(15));
        }
        reshow_model(&v);
    });
}

fn fill_local(v: &View, models: &[(String, String)], idle_minutes: u64) {
    let picker = &v.model;
    while let Some(child) = picker.local.first_child() {
        picker.local.remove(&child);
    }
    picker.marks.borrow_mut().retain(|(value, _)| MODELS.iter().any(|(m, _)| m == value));
    picker.local.set_visible(!models.is_empty());
    if models.is_empty() {
        return;
    }
    let heading = label("On this PC", "threads-menu-heading");
    heading.set_xalign(0.0);
    picker.local.append(&heading);
    for (id, name) in models {
        let (item, mark) = option("threads-model", id, name, &picker.popover, picker.pick);
        item.set_tooltip_text(Some("Runs on your graphics card: it loads when picked, and stops when you pick another model or leave it unused"));
        picker.local.append(&item);
        picker.marks.borrow_mut().push((id.clone(), mark));
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("threads-menu-idle");
    let caption = label("Turn off after", "");
    caption.set_hexpand(true);
    caption.set_xalign(0.0);
    row.append(&caption);
    let captions: Vec<&str> = IDLE_CHOICES.iter().map(|(_, c)| *c).collect();
    let idle = gtk::DropDown::from_strings(&captions);
    idle.set_widget_name("threads-model-idle");
    idle.set_tooltip_text(Some("How long a local model may sit unused before it stops and frees the graphics card"));
    let at = IDLE_CHOICES.iter().position(|(m, _)| *m == idle_minutes).unwrap_or(1);
    idle.set_selected(at as u32);
    idle.connect_selected_notify(|dd| {
        let Some((minutes, _)) = IDLE_CHOICES.get(dd.selected() as usize) else { return };
        let Some(ui) = the_ui() else { return };
        let minutes = *minutes;
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("settings.set", json!({"path": "threads.local.idle_minutes", "value": minutes})).await {
                ui.show_error(&e.to_string());
            }
        });
    });
    row.append(&idle);
    picker.local.append(&row);
}

fn is_local(model: &str) -> bool {
    STATE.with(|s| models::is_local(model, s.local_models.borrow().iter().map(|(id, _)| id.as_str())))
}

fn caption_of(options: &[(&str, &'static str)], value: &str) -> String {
    if let Some((_, c)) = options.iter().find(|(v, _)| *v == value) {
        return c.to_string();
    }
    let Some(name) = STATE.with(|s| s.local_models.borrow().iter().find(|(id, _)| id == value).map(|(_, n)| n.clone())) else {
        return value.to_string();
    };
    // A local model says when it is not ready to answer yet.
    let state = STATE.with(|s| s.local_state.borrow().clone());
    match state.1.as_str() {
        "loading" if state.0 == value => format!("{name} · loading"),
        _ => name,
    }
}

/// Show `model` and `effort` on the pickers, each checked in its list.
fn show_choice(v: &View, model: &str, effort: &str) {
    STATE.with(|s| *s.shown_model.borrow_mut() = model.to_string());
    for (picker, options, value) in [(&v.model, &MODELS[..], model), (&v.effort, &EFFORTS[..], effort)] {
        picker.shown.set_text(&caption_of(options, value));
        for (choice, mark) in picker.marks.borrow().iter() {
            mark.set_opacity(if choice == value { 1.0 } else { 0.0 });
        }
    }
    effort_sensitive(v);
    picture_key_shown(v);
}

/// A model or an effort picked: the thread showing takes it (`thread.set`), and new threads start
/// with it, in this run and the next.
fn choose(model: Option<&str>, effort: Option<&str>) {
    for (key, value) in [("model", model), ("effort", effort)] {
        if let Some(value) = value {
            keep(key, json!(value));
        }
    }
    let picked = model;
    let (model, effort) = STATE.with(|s| {
        let mut choice = s.choice.borrow_mut();
        if let Some(m) = model {
            choice.0 = m.to_string();
        }
        if let Some(e) = effort {
            choice.1 = e.to_string();
        }
        choice.clone()
    });
    if let Some(v) = view() {
        show_choice(&v, &model, &effort);
    }
    // Only a local pick goes to `thread.pick`: picking Claude there would stop the local model
    // the assistant's drawer, or another conversation, may be answering on.
    if picked.is_some() && is_local(&model) {
        if let Some(ui) = the_ui() {
            let model = model.clone();
            glib::spawn_future_local(async move {
                if let Err(e) = ui.call("thread.pick", json!({"model": model})).await {
                    ui.show_error(&e.to_string());
                }
            });
        }
    }
    let (Some(ui), Some(id)) = (the_ui(), current()) else { return };
    // A local model has no effort: the thread keeps none while on it.
    let effort = if is_local(&model) { String::new() } else { effort };
    glib::spawn_future_local(async move {
        match ui.call("thread.set", json!({"id": id, "model": model, "effort": effort})).await {
            Ok(thread) => {
                if let Some(v) = view() {
                    header(&v, &thread);
                }
            }
            Err(e) => {
                ui.show_error(&e.to_string());
                // Refused (a turn under way, say): the pickers show what the thread still runs on,
                // read without its messages (none come after the last possible id).
                if let Ok(read) = ui.call("thread.get", json!({"id": id, "after": i64::MAX})).await {
                    if let (Some(v), true) = (view(), current() == Some(id)) {
                        header(&v, &read["thread"]);
                    }
                }
            }
        }
    });
}

/// Draw the model picker again for what it shows: the local models or their state moved.
fn reshow_model(v: &View) {
    let model = STATE.with(|s| s.shown_model.borrow().clone());
    v.model.shown.set_text(&caption_of(&MODELS, &model));
    for (choice, mark) in v.model.marks.borrow().iter() {
        mark.set_opacity(if *choice == model { 1.0 } else { 0.0 });
    }
    effort_sensitive(v);
    // The models on this PC came in: the one shown may be among them.
    picture_key_shown(v);
}

/// The effort picker waits for a reply like the model picker, and a local model has no efforts.
fn effort_sensitive(v: &View) {
    let local = is_local(&STATE.with(|s| s.shown_model.borrow().clone()));
    let working = STATE.with(|s| s.working.get());
    v.effort.key.set_sensitive(!working && !local);
    v.effort.key.set_tooltip_text(Some(if local {
        "A model on this PC has no effort setting"
    } else if working {
        WAITS
    } else {
        "How hard the agent thinks before answering"
    }));
}

fn show_empty(v: &View) {
    v.middle.set_visible_child_name("empty");
    let (model, effort) = STATE.with(|s| s.choice.borrow().clone());
    show_choice(v, &model, &effort);
    v.title.set_text("New thread");
    v.title_key.set_visible(false);
    place_composer(v, true);
    set_working(v, false);
}

/// Forget the thread drawn: `id` shows next, or a new thread when `None`. The message box keeps
/// what was written, and the pictures added, for the thread they were meant for, and shows `id`'s
/// own. A picture still going up goes on meanwhile.
fn reset(id: Option<i64>) {
    if let Some(v) = view() {
        drop_pending(&v);
        let buffer = v.input.buffer();
        let written = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        let pictures = STATE.with(|s| std::mem::take(&mut *s.pictures.borrow_mut()));
        let back = STATE.with(|s| {
            let mut drafts = s.drafts.borrow_mut();
            if written.trim().is_empty() && pictures.is_empty() {
                drafts.remove(&s.current.get());
            } else {
                drafts.insert(s.current.get(), Draft { text: written, pictures });
            }
            drafts.remove(&id).unwrap_or_default()
        });
        buffer.set_text(&back.text);
        buffer.place_cursor(&buffer.end_iter());
        set_pictures(&v, back.pictures);
        note(&v, "");
    }
    STATE.with(|s| {
        s.shown_serial.set(s.shown_serial.get() + 1);
        s.current.set(id);
        s.last.set(0);
        s.last_day.borrow_mut().clear();
        s.pending.borrow_mut().clear();
        s.tools.borrow_mut().clear();
        *s.turn.borrow_mut() = None;
        *s.group.borrow_mut() = None;
        *s.last_user.borrow_mut() = None;
        *s.retry.borrow_mut() = None;
        s.added.borrow_mut().clear();
        s.stopping.set(false);
        if let Some(id) = id {
            s.unread.borrow_mut().remove(&id);
        }
    });
}

/// Start a new thread: the page shows the greeting until the first message is sent.
pub fn new_thread(ui: &Rc<Ui>) {
    start_thread(ui, true);
}

/// [`new_thread`]; with `show` false the window stays on the page it shows, its focus too.
fn start_thread(ui: &Rc<Ui>, show: bool) {
    set_aide(ui, None);
    reset(None);
    if let Some(v) = view() {
        show_empty(&v);
        v.empty_line.set_text("");
        if show {
            v.input.grab_focus();
        }
    }
    if show && *ui.page.borrow() != PAGE {
        ui.navigate(PAGE);
    }
    refresh_list(ui);
    check_claude(ui);
    load_local(ui);
    let ui = ui.clone();
    glib::spawn_future_local(async move { greet(&ui).await });
}

/// Start a thread with the person's own words, as if they had typed them into a new thread: the
/// page opens on it and the agent starts. "Connect Wealthsimple" comes here.
pub(crate) fn ask(ui: &Rc<Ui>, text: &str) {
    new_thread(ui);
    // Without Claude Code nothing can answer, and the send key is off: the words wait in the
    // message box, under the notice that says why.
    if STATE.with(|s| s.claude_missing.get()) {
        if let Some(v) = view() {
            v.input.buffer().set_text(text);
        }
        return;
    }
    send(ui, text);
}

/// Put `text` in the open thread's message box, focused with the cursor at its end, without
/// sending it: a question from the panel the person finishes or sends. What they had written
/// stays, the draft on a line after it. With no thread open the words wait in a new thread's
/// message box, still not sent: a draft such as "About Legs A on Thu 8 Oct: " is unfinished.
pub(crate) fn draft(ui: &Rc<Ui>, text: &str) {
    if current().is_none() {
        new_thread(ui);
    } else if *ui.page.borrow() != PAGE {
        ui.navigate(PAGE);
    }
    let Some(v) = view() else { return };
    let buffer = v.input.buffer();
    let written = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
    if written.trim().is_empty() {
        buffer.set_text(text);
    } else if !written.trim_end().ends_with(text.trim()) {
        let mut end = buffer.end_iter();
        buffer.insert(&mut end, &format!("{}{text}", if written.ends_with('\n') { "" } else { "\n" }));
    }
    buffer.place_cursor(&buffer.end_iter());
    v.input.grab_focus();
    v.input.scroll_mark_onscreen(&buffer.get_insert());
}

/// A new thread's words from the source showing: Tally's month or Avex's week in a line, and the
/// questions to start with. Arbiter's thread starts with Tally's.
async fn greet(ui: &Rc<Ui>) {
    let source = STATE.with(|s| s.source.get());
    let (line, items) = if source == "avex" {
        let summary = ui.call("gym.summary", json!({})).await;
        let today = super::pages::today();
        (summary.as_ref().ok().map(|s| week_line(s, &today)), avex_suggestions(summary.as_ref().ok()))
    } else {
        let (summary, invest) = tokio::join!(ui.call("money.summary", json!({})), ui.call("money.invest.summary", json!({})));
        (summary.as_ref().ok().map(month_line), suggestions(summary.as_ref().ok(), invest.as_ref().ok()))
    };
    // A thread opened, or another source chosen, while this read: it is not this greeting's.
    let Some(v) = view().filter(|_| current().is_none() && STATE.with(|s| s.source.get()) == source) else { return };
    v.empty_line.set_text(line.as_deref().unwrap_or(""));
    let same = STATE.with(|s| *s.suggested.borrow() == items);
    if !same {
        STATE.with(|s| *s.suggested.borrow_mut() = items);
        fill_suggestions(&v);
    }
}

/// "October so far: $1,616 spent, $1,284 left."
fn month_line(s: &Value) -> String {
    if s["empty"] == true {
        return String::from("Tally has nothing yet. Ask how to start, or add an account in the Tally panel.");
    }
    super::remember_currency(s);
    let fmt = super::formatter();
    let spent = fmt.format_whole(s["spent"].as_i64().unwrap_or(0));
    let left = s["pace"]["remaining"].as_i64().unwrap_or(0);
    let name = super::pages::period_name(&s["period"]);
    // A period that does not start on the 1st has a range for a name: it reads as "this period".
    let when = if name.contains(" to ") { String::from("This period") } else { name };
    if s["pace"]["budget"].as_i64().unwrap_or(0) > 0 {
        format!("{when} so far: {spent} spent, {}. Ask anything, or tell it what to change.", left_or_over(&fmt, left))
    } else {
        format!("{when} so far: {spent} spent. Ask anything, or tell it what to change.")
    }
}

/// The questions a new thread offers, from what the ledger shows: the budget furthest ahead of its
/// pace, a bill due within three days, and the investments (or bringing them in), then the usual
/// four. At most four.
fn suggestions(summary: Option<&Value>, invest: Option<&Value>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if let Some(s) = summary {
        if s["empty"] == true {
            out.push((String::from("How do I start with Tally?"), String::from("Accounts, a budget and your first entries")));
        } else {
            super::remember_currency(s);
            let fmt = super::formatter();
            let used = |b: &Value| b["spent"].as_i64().unwrap_or(0) as f64 / b["budget"].as_i64().unwrap_or(1).max(1) as f64;
            let ahead = s["budgets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| matches!(b["status"].as_str(), Some("OVER_PACE" | "OVER_BUDGET")))
                .max_by(|a, b| used(a).total_cmp(&used(b)));
            if let Some(b) = ahead {
                let name = b["name"].as_str().unwrap_or("this budget");
                let how = if b["status"] == "OVER_BUDGET" { "over budget" } else { "ahead of pace" };
                let days = s["period"]["days"].as_i64().unwrap_or(0);
                let day = (days - s["period"]["days_left"].as_i64().unwrap_or(0) + 1).clamp(1, days.max(1));
                let about = format!(
                    "{} of {} on day {day}",
                    fmt.format_whole(b["spent"].as_i64().unwrap_or(0)),
                    fmt.format_whole(b["budget"].as_i64().unwrap_or(0))
                );
                out.push((format!("Why is {name} {how}?"), about));
            }
            let due = s["bills"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|b| b["type"] != "INCOME" && (0..=3).contains(&b["days_until"].as_i64().unwrap_or(99)));
            if let Some(b) = due {
                let about = format!("{} · {}", fmt.format(b["amount"].as_i64().unwrap_or(0)), b["due_line"].as_str().unwrap_or(""));
                out.push((cover_question(b), about));
            }
        }
    }
    // Wealthsimple's files go only into a ledger kept in Canadian dollars.
    match invest {
        Some(p) if p["empty"] == true && p["currency"] == "CAD" => {
            out.push((String::from("Connect my Wealthsimple accounts"), String::from("A guided import from the files Wealthsimple gives you")))
        }
        Some(p) if p["empty"] == true => {}
        Some(_) => out.push((String::from("How are my investments doing?"), String::from("Value, gain and the room left this year"))),
        None => {}
    }
    for (ask, about) in SUGGESTIONS {
        if out.len() >= 4 {
            break;
        }
        out.push((ask.to_string(), about.to_string()));
    }
    out.truncate(4);
    out
}

/// "Can I cover Hydro-Québec on Sun 18 Oct?": a bill from `money.summary`, as a question.
fn cover_question(bill: &Value) -> String {
    let when = match bill["days_until"].as_i64().unwrap_or(0) {
        0 => String::from("today"),
        1 => String::from("tomorrow"),
        _ => format!("on {}", human_date(bill["next_date"].as_str().unwrap_or(""))),
    };
    format!("Can I cover {} {when}?", bill["name"].as_str().unwrap_or("this bill"))
}

/// The day Avex's export was written, `YYYY-MM-DD` (empty before one was imported).
fn export_day(s: &Value) -> &str {
    s["imported"]["exported_at"].as_str().and_then(|d| d.get(..10)).unwrap_or("")
}

/// Whole days from `from` to `to`, both `YYYY-MM-DD`; a daylight-saving hour does not shorten one.
fn days_between(from: &str, to: &str) -> Option<i64> {
    let (from, to) = (super::pages::date(from)?, super::pages::date(to)?);
    Some((to.difference(&from).as_hours() as f64 / 24.0).round() as i64)
}

/// A week of `gym.summary`'s in words: "3 of 4 workouts, 46,330 lb lifted", or "no workouts".
fn week_words(s: &Value, week: &Value) -> String {
    let sessions = week["sessions"].as_i64().unwrap_or(0);
    if sessions <= 0 {
        return String::from("no workouts");
    }
    let target = s["imported"]["days_per_week"].as_i64().unwrap_or(0);
    let count = if target > 0 { format!("{sessions} of {}", plural(target, "workout")) } else { plural(sessions, "workout") };
    let volume = week["volume"].as_f64().unwrap_or(0.0).round() as i64;
    format!("{count}, {} {} lifted", super::avex::group(volume), s["unit"].as_str().unwrap_or("lb"))
}

/// A new thread's line from Avex (`gym.summary`): the week its export reaches, and how old the
/// copy is once that week is over, since this week then reads as empty. `today` is `YYYY-MM-DD`.
fn week_line(s: &Value, today: &str) -> String {
    if s["imported"].is_null() {
        return String::from("Nothing from Avex yet. Export your training history in Avex, then import it from the panel.");
    }
    let day = export_day(s);
    let when = human_date(day);
    if day.is_empty() || day >= s["this_week"]["start"].as_str().unwrap_or("") {
        let words = if s["this_week"]["sessions"].as_i64().unwrap_or(0) > 0 { week_words(s, &s["this_week"]) } else { String::from("no workouts yet") };
        return match when.as_str() {
            _ if day.is_empty() || day == today => format!("This week so far: {words}."),
            "Yesterday" => format!("This week, to yesterday's export: {words}."),
            _ => format!("This week, to your export on {when}: {words}."),
        };
    }
    let age = days_between(day, today).filter(|d| *d >= 2).map(|d| format!(", {} ago", plural(d, "day"))).unwrap_or_default();
    let from = if when == "Yesterday" { String::from("yesterday") } else { when };
    // The week the export falls in, among the twelve the summary keeps.
    let week = s["weeks"].as_array().into_iter().flatten().find(|w| {
        let start = w["start"].as_str().unwrap_or("");
        let end = super::pages::date(start).and_then(|d| d.add_days(7).ok()).and_then(|d| d.format("%Y-%m-%d").ok()).map(|d| d.to_string());
        !start.is_empty() && start <= day && end.is_some_and(|end| day < end.as_str())
    });
    match week {
        Some(w) => format!("Your Avex copy is from {from}{age}. That week: {}.", week_words(s, w)),
        None => format!("Your Avex copy is from {from}{age}. Import a newer export to see the weeks since."),
    }
}

/// The questions a new thread offers with Avex showing, from its export: the week it reaches,
/// the lifts that stalled, the lift done most and the workout done last. At most four.
fn avex_suggestions(summary: Option<&Value>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stale = false;
    if let Some(s) = summary {
        if s["imported"].is_null() {
            return vec![(String::from("How do I bring my training in from Avex?"), String::from("Export it on the phone, then import it here"))];
        }
        let day = export_day(s);
        stale = !day.is_empty() && day < s["this_week"]["start"].as_str().unwrap_or("");
        let (ask, about) = AVEX_SUGGESTIONS[0];
        if stale {
            out.push((String::from("How did my last week of training go?"), format!("Your copy ends {}", human_date(day))));
        } else {
            out.push((ask.to_string(), about.to_string()));
        }
        out.push((AVEX_SUGGESTIONS[1].0.to_string(), AVEX_SUGGESTIONS[1].1.to_string()));
        if let Some(lift) = s["lifts"].as_array().and_then(|l| l.first()).and_then(|l| l["name"].as_str()).filter(|n| !n.is_empty()) {
            out.push((format!("How is my {lift} trending?"), String::from("Its estimated max, week by week")));
        }
        let last = s["recent"].as_array().into_iter().flatten().filter_map(|w| w["title"].as_str()).find(|t| !t.is_empty() && *t != "Freestyle");
        if let Some(title) = last {
            out.push((format!("Plan my next {title}"), String::from("Sets and weights from your last one")));
        }
    }
    for (ask, about) in AVEX_SUGGESTIONS {
        if out.len() >= 4 {
            break;
        }
        // A stale copy has no "this week" to ask about.
        if (stale && ask == AVEX_SUGGESTIONS[0].0) || out.iter().any(|(a, _)| a == ask) {
            continue;
        }
        out.push((ask.to_string(), about.to_string()));
    }
    out.truncate(4);
    out
}

/// Draw the questions a new thread offers, two to a row, or one under 600 pixels.
fn fill_suggestions(v: &View) {
    while let Some(child) = v.suggestions.first_child() {
        v.suggestions.remove(&child);
    }
    let columns = if STATE.with(|s| s.one_column.get()) { 1 } else { 2 };
    let items = STATE.with(|s| s.suggested.borrow().clone());
    for (index, (ask, about)) in items.into_iter().enumerate() {
        let key = gtk::Button::new();
        key.add_css_class("threads-suggestion");
        let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
        for (text, class) in [(&ask, "threads-suggestion-ask"), (&about, "threads-suggestion-about")] {
            let l = label(text, class);
            l.set_wrap(true);
            l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            words.append(&l);
        }
        key.set_child(Some(&words));
        key.connect_clicked(move |_| {
            if let Some(ui) = the_ui() {
                send(&ui, &ask);
            }
        });
        v.suggestions.attach(&key, (index % columns) as i32, (index / columns) as i32, 1, 1);
    }
}

/// Whether Claude Code is on this PC (`provider.list`): without it a thread cannot answer, so the
/// message box says so and does not send. Its sign-in is only as fresh as the last refresh in
/// Settings, so a signed-out reading warns and still lets the person try.
fn check_claude(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let Ok(found) = ui.call("provider.list", json!({})).await else { return };
        let claude = found["providers"].as_array().into_iter().flatten().find(|p| p["provider"] == "claude").cloned();
        let missing = claude.as_ref().is_none_or(|c| c["installed"] != true);
        let signed_out = claude.as_ref().is_some_and(|c| c["installed"] == true && !c["version"].is_null() && c["signed_in_as"].is_null());
        STATE.with(|s| s.claude_missing.set(missing));
        let Some(v) = view() else { return };
        let why = if missing {
            "Threads need Claude Code on this PC. Install it, then refresh it in Settings."
        } else if signed_out {
            "Claude Code is not signed in on this PC. Sign in, then refresh it in Settings."
        } else {
            ""
        };
        v.notice_text.set_text(why);
        v.notice.set_visible(!why.is_empty());
        update_send(&v);
    });
}

/// The capture harness's way in (smoke.rs `thread` and `threads`): the newest thread, or a new one.
pub fn smoke_open(ui: &Rc<Ui>, newest: bool) {
    crate::money::set_space(ui, crate::money::Space::Threads);
    if !newest {
        new_thread(ui);
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if let Some(id) = ui.call("thread.list", json!({})).await.ok().and_then(|list| list["threads"][0]["id"].as_i64()) {
            open(&ui, id);
        }
        // scripts/threads-smoke.py thread-edit: the newest message's Edit key, pressed.
        if std::env::var("RELAY_SMOKE_THREAD_EDIT").as_deref() == Ok("1") {
            glib::timeout_future(std::time::Duration::from_millis(1500)).await;
            if let Some(edit) = STATE.with(|s| s.last_user.borrow().as_ref().map(|(e, _, _)| e.clone())) {
                edit.emit_clicked();
            }
        }
    });
}

/// Open thread `id`: read it whole and draw it.
pub fn open(ui: &Rc<Ui>, id: i64) {
    set_aide(ui, None);
    show(ui, id);
}

/// Open Aide chat `chat`, its thread `id`: the page shows it in the Aide space, without Threads'
/// panel, its keys in the strip.
pub fn open_aide(ui: &Rc<Ui>, id: i64, chat: AideChat) {
    set_aide(ui, Some(chat));
    show(ui, id);
}

/// The Aide chat showing, if the page shows one.
pub fn aide() -> Option<AideChat> {
    STATE.with(|s| s.aide.borrow().clone())
}

/// Put the page in or out of an Aide chat. Showing already, the window follows it into its space.
fn set_aide(ui: &Rc<Ui>, chat: Option<AideChat>) {
    let changed = STATE.with(|s| s.aide.replace(chat.clone()) != chat);
    if let Some(v) = view() {
        let on = chat.is_some();
        v.panel_key.set_visible(!on);
        v.aide_keys.set_visible(on);
        super::aide::fill_strip(ui, &v.aide_keys, chat.as_ref());
        apply_panel(&v);
        let width = STATE.with(|s| s.width.get());
        v.limits.set_visible(width >= 640 && !on);
        show_hint(&v, current().is_none());
    }
    if changed && *ui.page.borrow() == PAGE {
        crate::money::follow(ui, PAGE, false);
    }
}

/// Read thread `id` whole and draw it, on the page.
fn show(ui: &Rc<Ui>, id: i64) {
    reset(Some(id));
    if let Some(v) = view() {
        clear(&v.column);
        v.column.append(&label("Opening…", "threads-note"));
        v.middle.set_visible_child_name("thread");
        v.title_key.set_visible(true);
        place_composer(&v, false);
    }
    if *ui.page.borrow() != PAGE {
        ui.navigate(PAGE);
    }
    mark_selected();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let read = ui.call("thread.get", json!({"id": id})).await;
        if current() != Some(id) {
            return;
        }
        match read {
            Ok(read) => draw_thread(&read, None),
            Err(e) => failed_open(&ui, id, &super::pages::unavailable(&e)),
        }
    });
}

/// The key above a long thread's first page: reads the page before it, and whatever arrived
/// after what `read` holds, and draws the thread again from all of it, the view held where it
/// was.
fn earlier_key(read: &Value) -> gtk::Button {
    let key = button("Show earlier messages", "");
    key.set_halign(gtk::Align::Center);
    let read = read.clone();
    key.connect_clicked(move |key| {
        let (Some(ui), Some(id)) = (the_ui(), current()) else { return };
        let messages = read["messages"].as_array().cloned().unwrap_or_default();
        let (Some(first), Some(last)) = (messages.first().and_then(|m| m["id"].as_i64()), messages.last().and_then(|m| m["id"].as_i64())) else { return };
        key.set_sensitive(false);
        key.set_label("Reading…");
        let (key, thread) = (key.clone(), read["thread"].clone());
        glib::spawn_future_local(async move {
            let older = ui.call("thread.get", json!({"id": id, "before": first})).await;
            let mut newer = Vec::new();
            let mut after = last;
            loop {
                let Ok(page) = ui.call("thread.get", json!({"id": id, "after": after})).await else { break };
                let got = page["messages"].as_array().cloned().unwrap_or_default();
                after = got.last().and_then(|m| m["id"].as_i64()).unwrap_or(after);
                newer.extend(got);
                if page["later"] != true {
                    break;
                }
            }
            if current() != Some(id) {
                return;
            }
            let Ok(older) = older else {
                key.set_sensitive(true);
                key.set_label("Could not read them. Try again");
                return;
            };
            let mut all = older["messages"].as_array().cloned().unwrap_or_default();
            all.extend(messages);
            all.extend(newer);
            let Some(v) = view() else { return };
            let adj = v.scroll.vadjustment();
            let from_bottom = adj.upper() - adj.value();
            draw_thread(&json!({"thread": thread, "messages": all, "earlier": older["earlier"]}), Some(from_bottom));
        });
    });
    key
}

/// A thread that could not be read: why, in the column, and a key to read it again.
fn failed_open(ui: &Rc<Ui>, id: i64, why: &str) {
    let Some(v) = view() else { return };
    clear(&v.column);
    let note = label(&format!("This thread could not be read: {why}"), "threads-error");
    note.set_wrap(true);
    v.column.append(&note);
    let again = button("Try again", "");
    again.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    again.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            show(&ui, id);
        }
    });
    v.column.append(&again);
}

/// Draw a thread as `thread.get` read it, landing on its newest message, or with `hold` this far
/// above the end of the conversation (after earlier messages were added above what was shown).
fn draw_thread(read: &Value, hold: Option<f64>) {
    let Some(v) = view() else { return };
    clear(&v.column);
    STATE.with(|s| {
        s.last.set(0);
        s.last_day.borrow_mut().clear();
        s.tools.borrow_mut().clear();
        *s.turn.borrow_mut() = None;
        *s.group.borrow_mut() = None;
    });
    // An Aide thread opened as one of Threads' (a thread restored as it was left) is Aide's to show.
    if read["thread"]["kind"] == "aide" && aide().is_none() {
        if let (Some(ui), Some(id)) = (the_ui(), read["thread"]["id"].as_i64()) {
            super::aide::open_thread(&ui, id);
        }
        return;
    }
    header(&v, &read["thread"]);
    // A long thread is read its newest messages first (about 1 MiB); the rest a key away.
    if read["earlier"] == true {
        v.column.append(&earlier_key(read));
    }
    for message in read["messages"].as_array().into_iter().flatten() {
        draw_message(&v, message);
    }
    if !STATE.with(|s| s.working.get()) {
        settle_tools();
    }
    if let Some(from_bottom) = hold {
        hold_from_bottom(&v, from_bottom);
        v.input.grab_focus();
        return;
    }
    show_history(&v);
    stick_to_bottom(&v, true);
    // Charts read their numbers after this and grow the column: land on the newest message again.
    glib::timeout_add_local_once(std::time::Duration::from_millis(400), || {
        if let Some(v) = view() {
            stick_to_bottom(&v, true);
        }
    });
    v.input.grab_focus();
    // The panel marks the entries this thread added.
    if STATE.with(|s| !s.added.borrow().is_empty()) {
        if let Some(ui) = the_ui() {
            refresh_panel(&ui);
        }
    }
}

/// How much of a thread's history its first frame lays out: its newest messages, this many views
/// high. Laying out every message of a long thread at once holds the window for a moment.
const FIRST_VIEWS: i32 = 2;
/// How long each later frame spends showing more of it, older messages first after newer.
const HISTORY_STEP: std::time::Duration = std::time::Duration::from_millis(4);

/// A thread was just drawn whole: show its newest messages now and the older ones over the next
/// frames, newest first, the view holding still over them as they come in above it.
fn show_history(v: &View) {
    let width = v.column.width();
    if width <= 0 {
        return; // never laid out: there is no view to fill first
    }
    let budget = FIRST_VIEWS * v.scroll.height().max(600);
    let mut older: std::collections::VecDeque<gtk::Widget> = std::collections::VecDeque::new();
    let mut height = 0;
    let mut child = v.column.last_child();
    while let Some(w) = child {
        child = w.prev_sibling();
        if height > budget {
            older.push_back(w);
        } else if w != *v.working.upcast_ref::<gtk::Widget>() {
            height += w.measure(gtk::Orientation::Vertical, width).1 + v.column.spacing();
        }
    }
    if older.is_empty() {
        return;
    }
    for w in &older {
        w.set_visible(false);
    }
    let serial = STATE.with(|s| s.shown_serial.get());
    let column = v.column.clone();
    let older = RefCell::new(older);
    let first = Cell::new(true);
    v.column.add_tick_callback(move |_, _| {
        let Some(v) = view().filter(|_| STATE.with(|s| s.shown_serial.get()) == serial) else {
            return glib::ControlFlow::Break;
        };
        let adj = v.scroll.vadjustment();
        if first.replace(false) {
            // The first frame lands on the newest message.
            hold_from_bottom(&v, adj.page_size());
            return glib::ControlFlow::Continue;
        }
        hold_from_bottom(&v, adj.upper() - adj.value());
        let started = std::time::Instant::now();
        let width = column.width();
        let mut older = older.borrow_mut();
        while let Some(w) = older.pop_front() {
            if w.parent().as_ref() != Some(column.upcast_ref()) {
                continue;
            }
            w.set_visible(true);
            w.measure(gtk::Orientation::Vertical, width);
            if started.elapsed() >= HISTORY_STEP {
                break;
            }
        }
        if older.is_empty() {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

/// After this frame's layout, put the view `from_bottom` above the end of the conversation, so
/// what grew above it does not move what it shows.
fn hold_from_bottom(v: &View, from_bottom: f64) {
    let Some(clock) = v.scroll.frame_clock() else { return };
    let adj = v.scroll.vadjustment();
    let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
    let held = handler.clone();
    // Connected after the window's own, so it runs once the frame is laid out; the value it sets
    // lays the scroller out again before the frame is drawn.
    handler.set(Some(clock.connect_layout(move |clock| {
        adj.set_value(adj.upper() - from_bottom);
        if let Some(id) = held.take() {
            clock.disconnect(id);
        }
    })));
}

fn header(v: &View, thread: &Value) {
    v.title.set_text(thread["title"].as_str().unwrap_or("Thread"));
    show_choice(v, thread["model"].as_str().unwrap_or(""), thread["effort"].as_str().unwrap_or(""));
    let live = thread["live"] == true;
    STATE.with(|s| s.live.set(live));
    if live && v.working_text.text() == STARTING {
        v.working_text.set_text("Working…");
    }
    set_working(v, thread["working"] == true);
}

fn set_working(v: &View, working: bool) {
    let was = STATE.with(|s| s.working.replace(working));
    let (icon, tip, name) = if working { ("stop", "Stop the reply · Esc", "Stop the reply") } else { ("arrow-up", "Send · Enter", "Send") };
    v.send.set_child(Some(&crate::icons::image_with_stroke(icon, 15, 2.0)));
    v.send.set_tooltip_text(Some(tip));
    v.send.update_property(&[gtk::accessible::Property::Label(name)]);
    // `thread.set` closes a running agent: the choice waits for the reply to end, and says so.
    v.model.key.set_sensitive(!working);
    v.model.key.set_tooltip_text(Some(if working { WAITS } else { MODEL_ABOUT }));
    STATE.with(|s| {
        if let Some((edit, _, _)) = s.last_user.borrow().as_ref() {
            edit.set_sensitive(!working);
        }
        if let Some(retry) = s.retry.borrow().as_ref() {
            retry.set_sensitive(!working);
        }
    });
    effort_sensitive(v);
    if working {
        if !was {
            // A stop that raced the end of the last reply does not mark this one.
            STATE.with(|s| s.stopping.set(false));
            v.working_text.set_text(if STATE.with(|s| s.live.get()) { "Working…" } else { STARTING });
        }
        if v.working.parent().is_none() && v.middle.visible_child_name().as_deref() == Some("thread") {
            v.column.append(&v.working);
        }
        v.working.set_visible(!running_tools());
    } else {
        if let Some(parent) = v.working.parent().and_downcast::<gtk::Box>() {
            parent.remove(&v.working);
        }
        drop_pending(v);
        let pending = STATE.with(|s| std::mem::take(&mut *s.pending.borrow_mut()));
        // A stopped reply keeps what it wrote, said to be cut short. The engine stores only
        // finished replies, so a reload no longer shows it.
        let stopped = STATE.with(|s| s.stopping.replace(false));
        if stopped && was && v.middle.visible_child_name().as_deref() == Some("thread") {
            let turn = turn_of(v);
            if !pending.trim().is_empty() {
                draw_reply(&turn.body, &pending);
                note_text(&turn, &pending);
            }
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&label("Stopped", "threads-note"));
            turn.body.append(&row);
            retry_key(&row);
        }
        settle_tools();
    }
    update_send(v);
}

fn at_bottom(adj: &gtk::Adjustment) -> bool {
    adj.value() + adj.page_size() >= adj.upper() - 48.0
}

/// Keep the newest message in view: always when `force`, else only when the reader was already
/// at the bottom.
fn stick_to_bottom(v: &View, force: bool) {
    let adj = v.scroll.vadjustment();
    if force || at_bottom(&adj) {
        glib::idle_add_local_once(move || {
            adj.set_value(adj.upper() - adj.page_size());
        });
    }
}

/// The agent's current turn, made when its first piece arrives: its pieces, then a quiet Copy key
/// for its words once it has some.
fn turn_of(v: &View) -> Turn {
    if let Some(turn) = STATE.with(|s| s.turn.borrow().clone()) {
        return turn;
    }
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 4);
    wrap.add_css_class("threads-turn");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
    wrap.append(&body);
    let text: Rc<RefCell<String>> = Rc::default();
    let words = text.clone();
    let copy = copy_key("Copy this reply", move || words.borrow().clone());
    copy.add_css_class("threads-copy");
    copy.set_halign(gtk::Align::Start);
    copy.set_visible(false);
    wrap.append(&copy);
    v.column.append(&wrap);
    let turn = Turn { body, text, copy };
    STATE.with(|s| *s.turn.borrow_mut() = Some(turn.clone()));
    turn
}

/// A quiet Copy key: puts [`words`] on the clipboard and says so with a tick for a moment.
fn copy_key(tip: &'static str, words: impl Fn() -> String + 'static) -> gtk::Button {
    let copy = crate::app::icon_button("copy", tip);
    copy.connect_clicked(move |key| {
        key.clipboard().set_text(&words());
        key.set_child(Some(&crate::icons::image("check", 16)));
        key.set_tooltip_text(Some("Copied"));
        let key = key.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1400), move || {
            if let Some(key) = key.upgrade() {
                key.set_child(Some(&crate::icons::image("copy", 16)));
                key.set_tooltip_text(Some(tip));
            }
        });
    });
    copy
}

/// Put the person's newest message back in the box, its pictures too, to change and send again.
fn edit_last(words: &str, images: &[Value]) {
    let (Some(v), Some(ui)) = (view(), the_ui()) else { return };
    if STATE.with(|s| s.working.get()) {
        return;
    }
    let buffer = v.input.buffer();
    buffer.set_text(words);
    buffer.place_cursor(&buffer.end_iter());
    set_pictures(&v, images.iter().take(MAX_PICTURES).map(|image| Attached::again(&ui, image, remove_picture)).collect());
    note(&v, "");
    v.input.grab_focus();
}

/// Try again, after a reply that failed or was stopped: sends the person's newest message once
/// more. Hidden by the next message.
fn retry_key(parent: &gtk::Box) {
    let Some((words, images)) = STATE.with(|s| s.last_user.borrow().as_ref().map(|(_, w, i)| (w.clone(), i.clone()))) else {
        return;
    };
    let key = gtk::Button::new();
    key.add_css_class("threads-retry");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&crate::icons::image("refresh", 13));
    row.append(&label("Try again", ""));
    key.set_child(Some(&row));
    key.set_tooltip_text(Some("Send your last message again"));
    key.set_halign(gtk::Align::Start);
    key.set_sensitive(!STATE.with(|s| s.working.get()));
    key.connect_clicked(move |_| {
        if let Some(ui) = the_ui() {
            if !STATE.with(|s| s.working.get()) {
                send_with(&ui, &words, images.clone(), Vec::new());
            }
        }
    });
    parent.append(&key);
    if let Some(old) = STATE.with(|s| s.retry.replace(Some(key))) {
        old.set_visible(false);
    }
}

/// The box the agent's current turn draws into.
fn turn(v: &View) -> gtk::Box {
    turn_of(v).body
}

/// Keep `text` for the turn's Copy key, which shows from its first words.
fn note_text(turn: &Turn, text: &str) {
    let mut all = turn.text.borrow_mut();
    if !all.is_empty() {
        all.push_str("\n\n");
    }
    all.push_str(text.trim());
    turn.copy.set_visible(true);
}

/// A day's name above the first message of a day other than the one before it. A thread that
/// began today says nothing: its replies are as fresh as the ledger.
fn day_break(v: &View, created_at: &str) {
    let Some(day) = glib::DateTime::from_iso8601(created_at, None)
        .ok()
        .and_then(|t| t.to_local().ok())
        .and_then(|t| t.format("%Y-%m-%d").ok())
        .map(|d| d.to_string())
    else {
        return;
    };
    let previous = STATE.with(|s| s.last_day.replace(day.clone()));
    if previous == day || (previous.is_empty() && day == super::pages::today()) {
        return;
    }
    STATE.with(|s| {
        *s.turn.borrow_mut() = None;
        *s.group.borrow_mut() = None;
    });
    let name = label(&human_date(&day), "threads-day");
    name.set_xalign(0.5);
    v.column.append(&name);
}

fn draw_message(v: &View, m: &Value) {
    let id = m["id"].as_i64().unwrap_or(0);
    if id <= STATE.with(|s| s.last.get()) {
        return;
    }
    STATE.with(|s| s.last.set(id));
    day_break(v, m["created_at"].as_str().unwrap_or(""));
    let body = &m["body"];
    match m["role"].as_str().unwrap_or("") {
        "user" => {
            let text = body["text"].as_str().unwrap_or("");
            let images: Vec<Value> = body["images"].as_array().cloned().unwrap_or_default();
            STATE.with(|s| {
                *s.turn.borrow_mut() = None;
                *s.group.borrow_mut() = None;
                // Only the newest message is edited, and only the newest failure retried.
                if let Some((edit, _, _)) = s.last_user.borrow_mut().take() {
                    edit.set_visible(false);
                }
                if let Some(retry) = s.retry.borrow_mut().take() {
                    retry.set_visible(false);
                }
            });
            let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            wrap.add_css_class("threads-user-wrap");
            wrap.set_halign(gtk::Align::End);
            let bubble = label(text, "threads-user");
            bubble.set_wrap(true);
            bubble.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            bubble.set_selectable(true);
            // Selectable for the mouse, not a Tab stop (set after `selectable`, which sets it).
            bubble.set_focusable(false);
            bubble.set_halign(gtk::Align::End);
            bubble.set_max_width_chars(60);
            // Beside it on hover, so the conversation keeps its spacing: Copy, and on the newest
            // one Edit, which puts the words back in the box to change and send again. The agent
            // keeps the first version too; it reads the new one as the person's correction.
            let keys = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            keys.add_css_class("threads-user-keys");
            keys.set_valign(gtk::Align::End);
            if !text.is_empty() {
                let words = text.to_string();
                keys.append(&copy_key("Copy this message", move || words.clone()));
            }
            let edit = crate::app::icon_button("edit", "Edit and send again");
            edit.set_child(Some(&crate::icons::image("edit", 16)));
            let (words, sent) = (text.to_string(), images.clone());
            edit.connect_clicked(move |_| edit_last(&words, &sent));
            edit.set_sensitive(!STATE.with(|s| s.working.get()));
            keys.append(&edit);
            wrap.append(&keys);
            // Its pictures above its words, both at the column's end.
            let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
            content.set_halign(gtk::Align::End);
            if let Some(ui) = the_ui().filter(|_| !images.is_empty()) {
                content.append(&pictures::in_message(&ui, &images));
            }
            if !text.is_empty() || images.is_empty() {
                content.append(&bubble);
            }
            wrap.append(&content);
            STATE.with(|s| *s.last_user.borrow_mut() = Some((edit, text.to_string(), images)));
            v.column.append(&wrap);
        }
        "assistant" => {
            drop_pending(v);
            STATE.with(|s| s.pending.borrow_mut().clear());
            let turn = turn_of(v);
            for block in body["blocks"].as_array().into_iter().flatten() {
                match block["type"].as_str() {
                    Some("text") => {
                        let text = block["text"].as_str().unwrap_or("");
                        draw_reply(&turn.body, text);
                        note_text(&turn, text);
                    }
                    Some("tool_use") => draw_tool(v, &turn.body, block),
                    _ => {}
                }
            }
        }
        "tool" => draw_tool_answer(v, body),
        // An Aide channel: Relay's words that started a run, quiet beside the conversation.
        "wake" => {
            STATE.with(|s| {
                *s.turn.borrow_mut() = None;
                *s.group.borrow_mut() = None;
            });
            v.column.append(&super::aide::wake_note(body["text"].as_str().unwrap_or("")));
        }
        // What a bot said, in its chat and the Chief of Staff's: a message from it, its keys
        // for a bot it proposes.
        "note" => {
            STATE.with(|s| {
                *s.turn.borrow_mut() = None;
                *s.group.borrow_mut() = None;
            });
            if let Some(ui) = the_ui() {
                v.column.append(&super::aide::note(&ui, body));
            }
        }
        "error" => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("threads-error-row");
            let mark = crate::icons::image("close", 13);
            mark.set_valign(gtk::Align::Start);
            mark.set_margin_top(3);
            row.append(&mark);
            let text = label(body["text"].as_str().unwrap_or("Something went wrong"), "threads-error");
            text.set_wrap(true);
            text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            text.set_hexpand(true);
            text.set_selectable(true);
            text.set_focusable(false);
            row.append(&text);
            let turn = turn(v);
            turn.append(&row);
            retry_key(&turn);
        }
        _ => {}
    }
    // The working row stays last while the turn runs.
    if v.working.parent().is_some() {
        v.column.remove(&v.working);
        v.column.append(&v.working);
    }
}

/// Whether a table cell is a figure ("$1,284.50", "−$96", "45%", "12"), which aligns right.
fn numeric(markup: &str) -> bool {
    let mut plain = String::new();
    let mut in_tag = false;
    for c in markup.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => plain.push(c),
            _ => {}
        }
    }
    let t = plain.trim().trim_start_matches(['−', '-', '+', '(']).trim_end_matches(')');
    let t = t.trim_start_matches(['$', '€', '£']).trim_end_matches(['%', '$', '€']).trim();
    t.starts_with(|c: char| c.is_ascii_digit())
        && t.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | ' ' | '\u{a0}' | '\u{202f}'))
}

/// The longest the last label of a reply being written grows before the text up to its last line
/// break moves into a label of its own. Setting a label's text lays all of it out again, so a long
/// reply set whole on every word costs more with each word.
pub(crate) const PENDING_KEEP: usize = 240;

/// One label of the reply being written: its words as they came, not yet read as Markdown.
pub(crate) fn pending_label() -> gtk::Label {
    let l = label("", "threads-reply");
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_selectable(false);
    l
}

/// Where the last label of a reply being written, holding `rest`, splits: at its last line break,
/// once it is longer than [`PENDING_KEEP`] and has text after that break (so the new last label is
/// never empty). The break itself becomes the boundary between the two labels.
pub(crate) fn pending_split(rest: &str) -> Option<usize> {
    if rest.len() <= PENDING_KEEP {
        return None;
    }
    rest.rfind('\n').filter(|at| at + 1 < rest.len())
}

/// Show the reply being written as it now stands. Its labels split it at line breaks, each break
/// the boundary between two labels, so the lines fall exactly where one label holding it all would
/// put them; only the last label is set again.
fn show_pending() {
    STATE.with(|s| s.pending_due.set(false));
    let Some(v) = view() else { return };
    let Some(mut last) = v.pending.last_child().and_downcast::<gtk::Label>() else { return };
    STATE.with(|s| {
        let text = s.pending.borrow();
        if s.pending_shown.replace(text.len()) == text.len() {
            return;
        }
        let mut kept = s.pending_kept.get().min(text.len());
        if let Some(at) = pending_split(&text[kept..]) {
            last.set_text(&text[kept..kept + at]);
            last = pending_label();
            v.pending.append(&last);
            kept += at + 1;
            s.pending_kept.set(kept);
        }
        last.set_text(&text[kept..]);
    });
    stick_to_bottom(&v, false);
}

/// Take the reply being written out of the conversation, emptied for the next one.
fn drop_pending(v: &View) {
    if let Some(parent) = v.pending.parent().and_downcast::<gtk::Box>() {
        parent.remove(&v.pending);
    }
    while let Some(first) = v.pending.first_child().filter(|w| w.next_sibling().is_some()) {
        v.pending.remove(&first);
    }
    if let Some(last) = v.pending.last_child().and_downcast::<gtk::Label>() {
        last.set_text("");
    }
    STATE.with(|s| {
        s.pending_kept.set(0);
        s.pending_shown.set(0);
    });
}

/// An agent's text, its Markdown drawn as blocks.
pub(crate) fn draw_reply(parent: &gtk::Box, text: &str) {
    for block in markdown(text) {
        let widget: gtk::Widget = match block {
            Block::Paragraph(markup) => reply_label(&markup, "threads-reply").upcast(),
            Block::Heading { level, text } => reply_label(&text, if level <= 2 { "threads-h2" } else { "threads-h3" }).upcast(),
            Block::List { ordered, start, items } => {
                let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
                for (n, item) in items.iter().enumerate() {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let mark = if ordered { format!("{}.", start as usize + n) } else { String::from("•") };
                    let marker = label(&mark, "threads-marker");
                    marker.set_valign(gtk::Align::Start);
                    row.append(&marker);
                    let words = reply_label(item, "threads-reply");
                    words.set_hexpand(true);
                    row.append(&words);
                    list.append(&row);
                }
                list.upcast()
            }
            Block::Code { lang, text } if lang == "chart" && super::chart::price_spec(&text).is_some() => {
                super::chart::price_card(super::chart::price_spec(&text).expect("checked")).upcast()
            }
            Block::Code { lang, text } if lang == "chart" && chart_spec(&text).is_some() => {
                super::chart::card(chart_spec(&text).expect("checked")).upcast()
            }
            Block::Code { lang, text } if lang == "import" && import_spec(&text).is_some() => {
                let spec = import_spec(&text).expect("checked");
                import_card(&spec.title, &spec.expects).upcast()
            }
            Block::Code { text, .. } => {
                let code = label(&text, "threads-code");
                code.set_selectable(true);
                code.set_focusable(false);
                code.set_wrap(true);
                code.set_wrap_mode(gtk::pango::WrapMode::Char);
                // Code is what gets pasted somewhere: its own Copy key, in its corner on hover.
                let block = gtk::Overlay::new();
                block.add_css_class("threads-code-block");
                block.set_child(Some(&code));
                let copy = copy_key("Copy the code", move || text.clone());
                copy.add_css_class("threads-code-copy");
                copy.set_halign(gtk::Align::End);
                copy.set_valign(gtk::Align::Start);
                block.add_overlay(&copy);
                block.upcast()
            }
            Block::Table(rows) => table(&rows).upcast(),
            Block::Rule => {
                let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
                rule.add_css_class("threads-rule");
                rule.upcast()
            }
            Block::Quote(markup) => reply_label(&markup, "threads-quote").upcast(),
        };
        parent.append(&widget);
    }
}

/// The width in characters a table cell is measured at before it may wrap.
const TABLE_CELL_CHARS: i32 = 24;

/// A Markdown table: its header over a rule, figures right-aligned in Geist Mono, and a scroller
/// of its own when it is wider than the column.
fn table(rows: &[Vec<String>]) -> gtk::ScrolledWindow {
    let grid = gtk::Grid::new();
    grid.add_css_class("threads-table");
    grid.set_column_spacing(18);
    grid.set_row_spacing(6);
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    // A column of figures (every body cell a figure or empty) aligns right, its heading too.
    let figures: Vec<bool> = (0..width)
        .map(|c| {
            let cells: Vec<&String> = rows.iter().skip(1).filter_map(|r| r.get(c)).filter(|t| !t.trim().is_empty()).collect();
            !cells.is_empty() && cells.iter().all(|t| numeric(t))
        })
        .collect();
    for (r, row) in rows.iter().enumerate() {
        // The rule sits in row 1, under the header.
        let at = if r == 0 { 0 } else { r as i32 + 1 };
        for (c, cell) in row.iter().enumerate() {
            let l = reply_label(cell, if r == 0 { "threads-table-head" } else { "threads-table-cell" });
            // In a scroller of its own, a wrapping cell is measured at its narrowest, a letter a
            // line, and the table took that height: four rows stood 750px tall, mostly blank. A
            // cell keeps its words on one line up to TABLE_CELL_CHARS, and wraps past twice that.
            let chars = l.text().chars().count() as i32;
            l.set_width_chars(chars.min(TABLE_CELL_CHARS));
            l.set_max_width_chars(TABLE_CELL_CHARS * 2);
            if figures.get(c).copied().unwrap_or(false) {
                l.set_xalign(1.0);
                if r > 0 {
                    l.add_css_class("threads-table-num");
                }
            }
            grid.attach(&l, c as i32, at, 1, 1);
        }
    }
    if rows.len() > 1 && width > 0 {
        let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
        rule.add_css_class("threads-table-rule");
        grid.attach(&rule, 0, 1, width as i32, 1);
    }
    // As wide as its columns, up to the column's width, where it scrolls.
    let scroll = gtk::ScrolledWindow::builder().child(&grid).propagate_natural_height(true).propagate_natural_width(true).build();
    scroll.set_halign(gtk::Align::Start);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.add_css_class("threads-table-scroll");
    scroll
}

fn reply_label(markup: &str, class: &str) -> gtk::Label {
    let l = label("", class);
    l.set_markup(markup);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_selectable(true);
    // Read, not edited: the mouse selects and copies, Tab passes it by.
    l.set_focusable(false);
    l
}

/// What the working row says while a tool runs.
fn doing(name: &str) -> &'static str {
    match tool_op(name).as_deref() {
        Some("money.summary") => "Reading the budget…",
        Some("money.lists") => "Reading accounts and categories…",
        Some("money.tx.list") => "Reading entries…",
        Some("money.series") => "Adding up spending…",
        Some("money.tx.add") => "Adding an entry…",
        Some("money.tx.update") => "Changing an entry…",
        Some("money.tx.restore") => "Restoring an entry…",
        Some("money.invest.summary") => "Reading your investments…",
        Some("money.invest.list") => "Reading investment activity…",
        Some("money.invest.add") => "Recording investment activity…",
        Some("thread.propose") => "Asking you first…",
        Some("bus.schema") => "Checking how a tool works…",
        // Avex's and Arbiter's: what changes or adds up by name, every other read as the phone
        // says it.
        Some("gym.series") => "Adding up training…",
        Some("arbiter.backtest") => "Running a backtest…",
        Some("arbiter.propose") => "Proposing a change…",
        Some("arbiter.order.place") => "Asking for an order…",
        Some("arbiter.halt") => "Halting Arbiter…",
        Some(op) if op.starts_with("gym.") => "Reading your training…",
        Some(op) if op.starts_with("arbiter.") => "Reading Arbiter…",
        _ => "Working…",
    }
}

/// A tool line's state mark: turning while it runs, then what it did, a cross when it was refused,
/// or a dash when the reply stopped before it answered.
fn show_step(mark: &gtk::Box, step: Step, writes: bool) {
    clear(mark);
    match step {
        Step::Running => {
            let spinner = crate::app::spinner();
            spinner.set_size_request(13, 13);
            mark.append(&spinner);
        }
        Step::Done => mark.append(&crate::icons::image(if writes { "check" } else { "search" }, 13)),
        Step::Failed => mark.append(&crate::icons::image("close", 13)),
        Step::Cut => mark.append(&crate::icons::image("minus", 13)),
    }
}

/// A tool call: a quiet line saying what the agent does, its mark turning until the answer. Reads
/// in a row gather in a [`Group`]; a change stands alone, since its answer may become a card.
fn draw_tool(v: &View, parent: &gtk::Box, block: &Value) {
    let name = block["name"].as_str().unwrap_or("").to_string();
    let op = tool_op(&name).unwrap_or_default();
    // In an Aide chat, what a bot says and proposes shows as its note; how a run ended, on its page.
    if aide().is_some() && matches!(op.as_str(), "aide.notify" | "aide.job.propose" | "aide.run.finish") {
        return;
    }
    let writes = tool_writes(&name) || super::arbiter::tool_writes(&op) || op == "thread.propose";
    let id = block["id"].as_str().map(str::to_string);
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("threads-tool");
    let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mark.add_css_class("threads-tool-mark");
    mark.set_valign(gtk::Align::Center);
    // A call with no id can never be answered: it shows as done.
    show_step(&mark, if id.is_some() { Step::Running } else { Step::Done }, writes);
    line.append(&mark);
    let did = super::arbiter::tool_caption(&op).map_or_else(|| tool_caption(&name), str::to_string);
    // While it runs the line says what the agent is doing, and is the only working line: the
    // working row under it said the same again ("Added up spending" over "Adding up spending…").
    let now = doing(&name);
    let caption = label(if id.is_some() && now != "Working…" { now } else { did.as_str() }, "threads-tool-text");
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&caption);
    if let Some(op) = tool_op(&name) {
        line.set_tooltip_text(Some(&op));
    }
    slot.append(&line);
    let group = if writes {
        STATE.with(|s| *s.group.borrow_mut() = None);
        parent.append(&slot);
        None
    } else {
        let open = STATE.with(|s| s.group.borrow().clone()).filter(|g| parent.last_child().as_ref() == Some(g.outer.upcast_ref()));
        let group = open.unwrap_or_else(|| {
            let g = new_group(parent, &super::arbiter::tool_caption(&op).map_or_else(|| tool_caption(&name), str::to_string));
            STATE.with(|s| *s.group.borrow_mut() = Some(g.clone()));
            g
        });
        group.lines.append(&slot);
        Some(group)
    };
    if STATE.with(|s| s.working.get()) {
        v.working_text.set_text(doing(&name));
    }
    if let Some(id) = id {
        if let Some(g) = &group {
            g.members.borrow_mut().push(id.clone());
        }
        v.working.set_visible(false);
        let tool = Tool { slot, mark, caption, name, step: Step::Running, group: group.clone() };
        STATE.with(|s| s.tools.borrow_mut().insert(id, tool));
    }
    if let Some(g) = &group {
        refresh_group(g);
    }
}

/// A new run of reads at the end of `parent`, named after its first.
fn new_group(parent: &gtk::Box, first: &str) -> Group {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 2);
    outer.add_css_class("threads-tools");
    let summary = gtk::Button::new();
    summary.add_css_class("quiet");
    summary.add_css_class("threads-tools-key");
    summary.set_halign(gtk::Align::Start);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let summary_mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    summary_mark.add_css_class("threads-tool-mark");
    summary_mark.set_valign(gtk::Align::Center);
    row.append(&summary_mark);
    let summary_text = label("", "threads-tool-text");
    summary_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&summary_text);
    let chevron = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    chevron.append(&crate::icons::image("chevron-down", 12));
    row.append(&chevron);
    summary.set_child(Some(&row));
    summary.set_visible(false);
    outer.append(&summary);
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(120);
    revealer.set_reveal_child(true);
    let lines = gtk::Box::new(gtk::Orientation::Vertical, 2);
    revealer.set_child(Some(&lines));
    outer.append(&revealer);
    let shown = revealer.clone();
    summary.connect_clicked(move |key| {
        let open = !shown.reveals_child();
        shown.set_reveal_child(open);
        clear(&chevron);
        chevron.append(&crate::icons::image(if open { "chevron-up" } else { "chevron-down" }, 12));
        key.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
    });
    parent.append(&outer);
    Group { outer, summary, summary_mark, summary_text, revealer, lines, members: Rc::default(), first: first.to_string(), folded: Rc::default() }
}

/// A run's summary line: from three reads they fold under "Read the budget and 2 more", marked
/// with the run's state: turning while one runs, a cross when one was refused.
fn refresh_group(g: &Group) {
    let ids = g.members.borrow().clone();
    if ids.len() < 3 {
        g.summary.set_visible(false);
        return;
    }
    if !g.folded.replace(true) {
        g.revealer.set_reveal_child(false);
        g.summary.update_state(&[gtk::accessible::State::Expanded(Some(false))]);
    }
    g.summary.set_visible(true);
    let steps: Vec<Step> = STATE.with(|s| {
        let tools = s.tools.borrow();
        ids.iter().filter_map(|id| tools.get(id).map(|t| t.step)).collect()
    });
    let step = if steps.contains(&Step::Running) {
        Step::Running
    } else if steps.contains(&Step::Failed) {
        Step::Failed
    } else {
        Step::Done
    };
    show_step(&g.summary_mark, step, false);
    if step == Step::Failed {
        g.summary.add_css_class("threads-tool-failed");
    } else {
        g.summary.remove_css_class("threads-tool-failed");
    }
    let text = format!("{} and {} more", g.first, ids.len() - 1);
    g.summary_text.set_text(&text);
    g.summary.update_property(&[gtk::accessible::Property::Label(&text)]);
}

/// Whether a tool line is still turning.
fn running_tools() -> bool {
    STATE.with(|s| s.tools.borrow().values().any(|t| t.step == Step::Running))
}

/// The turn ended: a tool line still turning got no answer (the reply was stopped). It stops.
fn settle_tools() {
    let open: Vec<(gtk::Box, gtk::Label, String, Option<Group>)> = STATE.with(|s| {
        s.tools
            .borrow_mut()
            .values_mut()
            .filter(|t| t.step == Step::Running)
            .map(|t| {
                t.step = Step::Cut;
                (t.mark.clone(), t.caption.clone(), t.name.clone(), t.group.clone())
            })
            .collect()
    });
    for (mark, caption, name, group) in open {
        show_step(&mark, Step::Cut, false);
        let op = tool_op(&name).unwrap_or_default();
        caption.set_text(&super::arbiter::tool_caption(&op).map_or_else(|| tool_caption(&name), str::to_string));
        if let Some(g) = group {
            refresh_group(&g);
        }
    }
}

/// A tool's answer: its line stops turning; a refusal says why under it; an entry or an activity
/// the agent wrote becomes a card.
fn draw_tool_answer(v: &View, body: &Value) {
    let Some(id) = body["tool_use_id"].as_str() else { return };
    let failed = body["is_error"] == true;
    let step = if failed { Step::Failed } else { Step::Done };
    let found = STATE.with(|s| {
        s.tools.borrow_mut().get_mut(id).map(|t| {
            t.step = step;
            (t.slot.clone(), t.mark.clone(), t.caption.clone(), t.name.clone(), t.group.clone())
        })
    });
    let Some((slot, mark, caption, name, group)) = found else { return };
    let op = tool_op(&name).unwrap_or_default();
    let writes = tool_writes(&name) || super::arbiter::tool_writes(&op) || op == "thread.propose";
    show_step(&mark, step, writes);
    caption.set_text(&super::arbiter::tool_caption(&op).map_or_else(|| tool_caption(&name), str::to_string));
    // The working row comes back once no line is turning.
    v.working.set_visible(!running_tools());
    if let Some(g) = &group {
        refresh_group(g);
    }
    if STATE.with(|s| s.working.get()) {
        v.working_text.set_text("Working…");
    }
    if failed {
        if let Some(line) = slot.first_child() {
            line.add_css_class("threads-tool-failed");
        }
        let text = body["text"].as_str().unwrap_or("").trim();
        let why = label(if text.is_empty() { "It was refused." } else { text }, "threads-tool-error");
        why.set_wrap(true);
        why.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        why.set_lines(2);
        why.set_ellipsize(gtk::pango::EllipsizeMode::End);
        why.set_margin_start(21);
        why.set_tooltip_text(Some(text));
        if matches!(op.as_str(), "arbiter.order.place" | "arbiter.propose") {
            // An order or a proposal the engine refused says why in the conversation's own words.
            slot.append(&super::arbiter::refusal_line(&super::arbiter::refusal_words(text)));
        } else {
            slot.append(&why);
        }
        return;
    }
    if op == "thread.propose" {
        // What the agent asked for becomes a card drawn from the ledger, never from its words:
        // only the proposal's id is taken from its answer.
        let Some(id) = serde_json::from_str::<Value>(body["text"].as_str().unwrap_or("")).ok().and_then(|a| a["id"].as_i64()) else { return };
        clear(&slot);
        slot.append(&super::ask::card(id));
        return;
    }
    if op.starts_with("arbiter.") {
        // A proposal becomes a card to approve; an order placed or refused, a line.
        let Ok(answer) = serde_json::from_str::<Value>(body["text"].as_str().unwrap_or("")) else { return };
        if let Some(card) = super::arbiter::tool_card(&op, &answer) {
            clear(&slot);
            slot.append(&card);
        }
        return;
    }
    if !writes {
        return;
    }
    let Ok(row) = serde_json::from_str::<Value>(body["text"].as_str().unwrap_or("")) else { return };
    if row["id"].as_i64().is_none() {
        return;
    }
    clear(&slot);
    slot.append(&if op == "money.invest.add" { activity_card(&row) } else { entry_card(&row, &op) });
}

/// Whether a failed request found nothing to act on: an Undo of something already removed.
fn gone(error: &crate::client::Error) -> bool {
    matches!(error, crate::client::Error::Bus(e) if e.code == "money.not_found")
}

/// Undo on a card for what the agent added: `ops.0` removes it, after which the card dims, says
/// so, and offers `ops.1` to bring it back. Something already removed (an Undo in an earlier
/// sitting) reads as removed, quietly.
fn undoable(card: &gtk::Box, title: &gtk::Label, what: &str, ops: (&'static str, &'static str), id: i64) -> gtk::Button {
    let key = button("Undo", "quiet");
    key.add_css_class("threads-undo");
    key.set_valign(gtk::Align::Center);
    let added = title.text().to_string();
    let removed = format!("Removed: {what}");
    let undone = Rc::new(Cell::new(false));
    let (card, title) = (card.downgrade(), title.downgrade());
    key.connect_clicked(move |key| {
        let Some(ui) = the_ui() else { return };
        key.set_sensitive(false);
        let restoring = undone.get();
        let op = if restoring { ops.1 } else { ops.0 };
        let (key, card, title, undone, added, removed) = (key.clone(), card.clone(), title.clone(), undone.clone(), added.clone(), removed.clone());
        glib::spawn_future_local(async move {
            let result = ui.call(op, json!({"id": id})).await;
            key.set_sensitive(true);
            let now_undone = match result {
                Ok(_) => !restoring,
                Err(e) if !restoring && gone(&e) => true,
                Err(e) => {
                    ui.show_error(&e.to_string());
                    return;
                }
            };
            undone.set(now_undone);
            key.set_label(if now_undone { "Restore" } else { "Undo" });
            if let (Some(card), Some(title)) = (card.upgrade(), title.upgrade()) {
                if now_undone {
                    card.add_css_class("threads-card-undone");
                    title.set_text(&removed);
                } else {
                    card.remove_css_class("threads-card-undone");
                    title.set_text(&added);
                }
            }
        });
    });
    key
}

/// What the agent did to an entry (`op`), with Undo when it added it.
fn entry_card(tx: &Value, op: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("threads-card");
    let transfer = tx["type"] == "TRANSFER";
    card.append(&if transfer { badge("repeat", 10) } else { badge(tx["icon"].as_str().unwrap_or("dot"), tx["color"].as_i64().unwrap_or(10)) });
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let kind = match tx["type"].as_str() {
        Some("INCOME") => "income",
        Some("TRANSFER") => "transfer",
        _ => "expense",
    };
    let what = if transfer {
        format!("{} → {}", tx["account"].as_str().unwrap_or(""), tx["to_account"].as_str().unwrap_or(""))
    } else {
        tx["note"].as_str().filter(|n| !n.trim().is_empty()).or(tx["category"].as_str()).unwrap_or(kind).to_string()
    };
    let verb = match op {
        "money.tx.add" => "Added",
        "money.tx.restore" => "Restored",
        _ => "Changed",
    };
    let title = label(&format!("{verb} {kind}: {what}"), "threads-card-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    // An edit answers with the entry as it was: the card says what it changed, before → after.
    let was = &tx["previous"];
    let edited = op == "money.tx.update" && was.is_object();
    let mut detail = if edited { changes(was, tx) } else { Vec::new() };
    if detail.is_empty() {
        if let Some(category) = tx["category"].as_str().filter(|_| !transfer) {
            detail.push(category.to_string());
        }
        if let Some(date) = tx["date"].as_str() {
            detail.push(human_date(date));
        }
        if let Some(account) = tx["account"].as_str().filter(|_| !transfer) {
            detail.push(account.to_string());
        }
    }
    let about = label(&detail.join(" · "), "threads-card-detail");
    about.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&about);
    card.append(&words);
    let places = super::formatter().fraction_digits;
    let amount_moved = edited && (minor(&was["amount"], places) != minor(&tx["amount"], places) || was["type"] != tx["type"]);
    let shown = if amount_moved { format!("{} → {}", amount_text(was), amount_text(tx)) } else { amount_text(tx) };
    let figure = label(&shown, "money-amount");
    figure.set_valign(gtk::Align::Center);
    if kind == "income" {
        figure.add_css_class("money-in");
    }
    card.append(&figure);
    if let Some(id) = tx["id"].as_i64().filter(|_| op == "money.tx.add") {
        STATE.with(|s| s.added.borrow_mut().insert(id));
        card.append(&undoable(&card, &title, &what, ("money.tx.delete", "money.tx.restore"), id));
    }
    if let Some(key) = edited.then(|| edit_undo(&card, &title, &what, tx)).flatten() {
        card.append(&key);
    }
    card
}

/// What an edit changed, before → after; what it left alone (and the amount, which the figure
/// shows) is not said.
fn changes(was: &Value, now: &Value) -> Vec<String> {
    let text = |v: &Value, key: &str| v[key].as_str().unwrap_or("").to_string();
    let mut out = Vec::new();
    if was["category"] != now["category"] {
        out.push(format!("{} → {}", was["category"].as_str().unwrap_or("No category"), now["category"].as_str().unwrap_or("No category")));
    }
    if was["date"] != now["date"] {
        out.push(format!("{} → {}", human_date(&text(was, "date")), human_date(&text(now, "date"))));
    }
    if was["account"] != now["account"] || was["to_account"] != now["to_account"] {
        out.push(format!("{} → {}", text(was, "account"), text(now, "account")));
    }
    if was["note"] != now["note"] {
        out.push(format!("\u{201c}{}\u{201d} → \u{201c}{}\u{201d}", text(was, "note"), text(now, "note")));
    }
    out
}

/// Undo on a card for an edit the agent made: the entry goes back as it was (`previous`), and
/// Redo makes the edit again. An edit that gave an entry its first category has none: the ledger
/// cannot take a category away (Tally's entry sheet can).
fn edit_undo(card: &gtk::Box, title: &gtk::Label, what: &str, tx: &Value) -> Option<gtk::Button> {
    let (was, id) = (&tx["previous"], tx["id"].as_i64()?);
    if was["category_id"].is_null() && !tx["category_id"].is_null() {
        return None;
    }
    let places = super::formatter().fraction_digits;
    let patch = |t: &Value| json!({
        "id": id, "type": t["type"], "amount": minor(&t["amount"], places), "date": t["date"], "account_id": t["account_id"],
        "to_account_id": t["to_account_id"], "category_id": t["category_id"], "note": t["note"],
    });
    let (back, again) = (patch(was), patch(tx));
    let key = button("Undo", "quiet");
    key.add_css_class("threads-undo");
    key.set_valign(gtk::Align::Center);
    let changed = title.text().to_string();
    let put_back = format!("Put back: {what}");
    let undone = Rc::new(Cell::new(false));
    let (card, title) = (card.downgrade(), title.downgrade());
    key.connect_clicked(move |key| {
        let Some(ui) = the_ui() else { return };
        key.set_sensitive(false);
        let undoing = !undone.get();
        let payload = if undoing { back.clone() } else { again.clone() };
        let (key, card, title, undone, changed, put_back) = (key.clone(), card.clone(), title.clone(), undone.clone(), changed.clone(), put_back.clone());
        glib::spawn_future_local(async move {
            let result = ui.call("money.tx.update", payload).await;
            key.set_sensitive(true);
            if let Err(e) = result {
                ui.show_error(&e.to_string());
                return;
            }
            undone.set(undoing);
            key.set_label(if undoing { "Redo" } else { "Undo" });
            if let (Some(card), Some(title)) = (card.upgrade(), title.upgrade()) {
                if undoing {
                    card.add_css_class("threads-card-undone");
                    title.set_text(&put_back);
                } else {
                    card.remove_css_class("threads-card-undone");
                    title.set_text(&changed);
                }
            }
        });
    });
    Some(key)
}

fn amount_text(tx: &Value) -> String {
    let fmt = super::formatter();
    let amount = fmt.format(minor(&tx["amount"], fmt.fraction_digits).unwrap_or(0));
    match tx["type"].as_str() {
        Some("EXPENSE") => format!("−{amount}"),
        Some("INCOME") => format!("+{amount}"),
        _ => amount,
    }
}

/// An amount from a tool's answer at `places` decimals: the bus's integer, or the decimal string
/// the MCP door gives a model instead ("157.99"; relay-cli's units.rs). Threads stored before the
/// door converted hold the integer.
fn minor(v: &Value, places: u32) -> Option<i64> {
    v.as_i64().or_else(|| relay_money::invest::parse_scaled(v.as_str()?, places).map(|s| s.value))
}

/// An investment activity in words, with its article: "a buy", "a dividend".
fn activity_word(kind: &str) -> &'static str {
    match kind {
        "DEPOSIT" => "a deposit",
        "TRANSFER_IN" => "a transfer in",
        "BUY" => "a buy",
        "REINVEST" => "a reinvested dividend",
        "SPLIT" => "a split",
        "DIVIDEND" => "a dividend",
        "INTEREST" => "interest",
        "CREDIT" => "a credit",
        "NOTIONAL_DISTRIBUTION" => "a notional distribution",
        "RETURN_OF_CAPITAL" => "a return of capital",
        "SELL" => "a sale",
        "TRANSFER_OUT" => "a transfer out",
        "FEE" => "a fee",
        "TAX" => "tax withheld",
        "FX" => "a currency exchange",
        "WITHDRAWAL" => "a withdrawal",
        _ => "an activity",
    }
}

/// Units held, at 1e-8 a unit, as written: "10", "0.5", "41.2".
fn units(quantity: i64) -> String {
    let (whole, part) = (quantity / 100_000_000, (quantity % 100_000_000).abs());
    if part == 0 {
        return whole.to_string();
    }
    let digits = format!("{part:08}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

/// An investment activity the agent recorded (`money.invest.add`), with Undo.
fn activity_card(a: &Value) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("threads-card");
    let kind = a["type"].as_str().unwrap_or("");
    // As the Investments page shows it: money in, money out, or neither.
    let (icon, flow, class) = super::invest::activity_look(kind, a["symbol"].as_str().is_some_and(|s| !s.is_empty()));
    card.append(&tile(icon));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let account = a["account"].as_str().unwrap_or("");
    let quantity = minor(&a["quantity"], 8).unwrap_or(0);
    let subject = match a["symbol"].as_str().filter(|s| !s.is_empty()) {
        Some(symbol) if quantity > 0 => format!("{} {symbol}", units(quantity)),
        Some(symbol) => symbol.to_string(),
        None => account.to_string(),
    };
    let what = format!("{}, {subject}", activity_word(kind));
    let title = label(&format!("Recorded {}: {subject}", activity_word(kind)), "threads-card-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    let currency = a["currency"].as_str().unwrap_or("CAD");
    let fmt = MoneyFormatter::new(currency, Locale::from_env());
    let mut detail = Vec::new();
    if !account.is_empty() && a["symbol"].as_str().is_some_and(|s| !s.is_empty()) {
        detail.push(account.to_string());
    }
    if let Some(date) = a["date"].as_str() {
        detail.push(human_date(date));
    }
    let fee = minor(&a["fee"], fmt.fraction_digits).unwrap_or(0);
    if fee > 0 {
        detail.push(format!("{} fee", fmt.format(fee)));
    }
    if let Some(note) = a["note"].as_str().map(str::trim).filter(|n| !n.is_empty()) {
        detail.push(note.to_string());
    }
    let about = label(&detail.join(" · "), "threads-card-detail");
    about.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&about);
    card.append(&words);
    let amount = minor(&a["amount"], fmt.fraction_digits).unwrap_or(0);
    let to_fmt = a["to_currency"].as_str().map(|to_currency| MoneyFormatter::new(to_currency, Locale::from_env()));
    let shown = match (kind, to_fmt.and_then(|f| Some((minor(&a["to_amount"], f.fraction_digits)?, f)))) {
        ("FX", Some((to, to_fmt))) => format!("{} → {}", fmt.format(amount), to_fmt.format(to)),
        ("SPLIT", _) => String::new(),
        _ => super::invest::activity_figure(&fmt, amount, flow),
    };
    let figure = label(&shown, "money-amount");
    figure.set_valign(gtk::Align::Center);
    if !class.is_empty() {
        figure.add_css_class(class);
    }
    card.append(&figure);
    if let Some(id) = a["id"].as_i64() {
        card.append(&undoable(&card, &title, &what, ("money.invest.delete", "money.invest.restore"), id));
    }
    card
}

/// The agent asks for a Wealthsimple file (an ```` ```import ```` block): a card to choose it or
/// drop it on. The shared flow previews it, maps its accounts and imports; then the card says what
/// came in, and a message in the person's name tells the agent, which carries on from there.
fn import_card(title: &str, expects: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("threads-card");
    card.add_css_class("threads-import");
    card.append(&tile("download"));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let heading = label(if title.trim().is_empty() { "Your Wealthsimple file" } else { title }, "threads-card-title");
    heading.set_wrap(true);
    words.append(&heading);
    let detail = label("The CSV you downloaded. Choose it, or drop it here.", "threads-card-detail");
    detail.set_wrap(true);
    words.append(&detail);
    card.append(&words);
    let key = button("Choose file…", "");
    key.add_css_class("threads-card-key");
    key.set_valign(gtk::Align::Center);
    card.append(&key);

    let thread = current();
    let expects = expects.to_string();
    let (shell, heading, detail, chooser) = (card.downgrade(), heading.downgrade(), detail.downgrade(), key.downgrade());
    // With no path the flow asks for the file; a drop gives it one, and with it the name of the
    // next file when several were dropped: one is imported at a time.
    let start: Rc<dyn Fn(Option<PathBuf>, Option<String>)> = Rc::new(move |path: Option<PathBuf>, next: Option<String>| {
        let Some(ui) = the_ui() else { return };
        let (shell, heading, detail, chooser) = (shell.clone(), heading.clone(), detail.clone(), chooser.clone());
        let then = next.map(|name| format!("One file at a time: drop {name} here next."));
        if let (Some(detail), Some(then)) = (detail.upgrade(), &then) {
            detail.set_text(then);
        }
        let weak = Rc::downgrade(&ui);
        super::invest::import_flow(&ui, path, &expects, move |result: Value| {
            let (title, about) = imported(&result);
            if let Some(heading) = heading.upgrade() {
                heading.set_text(&title);
            }
            if let Some(detail) = detail.upgrade() {
                detail.set_text(&match &then {
                    Some(then) => format!("{about}. {then}"),
                    None => about,
                });
            }
            if let (Some(card), Some(old)) = (shell.upgrade(), chooser.upgrade()) {
                card.remove(&old);
                let open = button("Open Investments", "quiet");
                open.set_valign(gtk::Align::Center);
                open.connect_clicked(|_| {
                    if let Some(ui) = the_ui() {
                        ui.navigate("money-invest");
                    }
                });
                card.append(&open);
            }
            if let (Some(ui), Some(id)) = (weak.upgrade(), thread) {
                tell(&ui, id, &told(&result));
            }
        });
    });
    let by_key = start.clone();
    key.connect_clicked(move |_| by_key(None, None));
    let target = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
    target.set_types(&[gtk::gdk::FileList::static_type(), gtk::gio::File::static_type()]);
    target.connect_drop(move |_, value, _, _| {
        let files = match value.get::<gtk::gdk::FileList>() {
            Ok(list) => list.files(),
            Err(_) => value.get::<gtk::gio::File>().ok().into_iter().collect(),
        };
        let next = files.get(1).and_then(|f| f.basename()).map(|name| name.to_string_lossy().to_string());
        match files.first().and_then(|f| f.path()) {
            Some(path) => {
                start(Some(path), next);
                true
            }
            None => false,
        }
    });
    card.add_controller(target);
    card
}

/// A count in an import's result: a number, or a list's length (`skipped`).
fn count(result: &Value, key: &str) -> i64 {
    result[key].as_i64().or_else(|| result[key].as_array().map(|a| a.len() as i64)).unwrap_or(0)
}

/// What came in (`ImportResult`): the card's title and its detail.
fn imported(result: &Value) -> (String, String) {
    let mut what = Vec::new();
    let (holdings, activities) = (count(result, "holdings"), count(result, "activities"));
    if holdings > 0 {
        what.push(plural(holdings, "holding"));
    }
    if activities > 0 {
        what.push(plural_as(activities, "activity", "activities"));
    }
    let title = if what.is_empty() { String::from("Nothing new in this file") } else { format!("Imported {}", what.join(", ")) };
    let mut about = Vec::new();
    if let Some(day) = result["as_of"].as_str().filter(|d| !d.is_empty()) {
        about.push(format!("As of {}", super::invest::as_of(day.get(..10).unwrap_or(day))));
    }
    let created = count(result, "accounts_created");
    if created > 0 {
        about.push(plural(created, "new account"));
    }
    let duplicates = count(result, "duplicates");
    if duplicates > 0 {
        about.push(format!("{duplicates} already in Tally"));
    }
    let skipped = count(result, "skipped");
    if skipped > 0 {
        about.push(format!("{} skipped", plural(skipped, "line")));
    }
    (title, if about.is_empty() { String::from("In Tally now") } else { about.join(" · ") })
}

/// The person's message after an import, so the agent carries on from what the file held. It
/// names the kind of file the engine read, which is the one imported even when the sheet chose
/// another after a refusal.
fn told(result: &Value) -> String {
    let file = match result["kind"].as_str() {
        Some("holdings") => "my Wealthsimple holdings report",
        Some("activities") => "my Wealthsimple activities export",
        Some("statement") => "my Wealthsimple monthly statement",
        _ => "my Wealthsimple file",
    };
    let (holdings, activities) = (count(result, "holdings"), count(result, "activities"));
    let mut what = Vec::new();
    if holdings > 0 {
        what.push(plural(holdings, "holding"));
    }
    if activities > 0 {
        what.push(plural_as(activities, "activity", "activities"));
    }
    let mut text = if what.is_empty() { format!("I imported {file}; nothing in it was new.") } else { format!("I imported {file}: {}.", what.join(" and ")) };
    let created = count(result, "accounts_created");
    if created > 0 {
        text.push_str(&format!(" It added {}.", plural(created, "new account")));
    }
    let duplicates = count(result, "duplicates");
    if duplicates > 0 {
        text.push_str(&format!(" {} already in Tally.", if duplicates == 1 { String::from("1 line was") } else { format!("{duplicates} lines were") }));
    }
    let skipped = count(result, "skipped");
    if skipped > 0 {
        text.push_str(&format!(" Tally skipped {} it could not read.", plural(skipped, "line")));
    }
    text
}

/// Send `text` in thread `id` in the person's name: something they did outside the message box.
/// While the agent is still answering there (`thread.busy`) the words wait, unseen, and go when
/// that turn ends. Anything else that refuses them leaves them in the message box when that thread
/// is showing, for the person to send.
fn tell(ui: &Rc<Ui>, id: i64, text: &str) {
    let ui = ui.clone();
    let text = text.to_string();
    glib::spawn_future_local(async move {
        match ui.call("thread.send", json!({"id": id, "text": text})).await {
            Ok(_) => {}
            Err(crate::client::Error::Bus(e)) if e.code == "thread.busy" => {
                STATE.with(|s| s.to_tell.borrow_mut().push((id, text)));
                // The turn may have ended, and its `thread.changed` gone by, before the refusal
                // came back: read whether it still runs (no messages, only the thread).
                let read = ui.call("thread.get", json!({"id": id, "after": i64::MAX})).await;
                if read.is_ok_and(|r| r["thread"]["working"] != true) {
                    tell_waiting(&ui, id);
                }
            }
            Err(e) => {
                if let Some(v) = view().filter(|_| current() == Some(id)) {
                    let buffer = v.input.buffer();
                    if buffer.char_count() == 0 {
                        buffer.set_text(&text);
                    }
                }
                ui.show_error(&e.to_string());
            }
        }
    });
}

/// Send the oldest words [`tell`] keeps for thread `id`, whose agent's turn has ended. The rest
/// wait for the end of the turn that one starts.
fn tell_waiting(ui: &Rc<Ui>, id: i64) {
    let next = STATE.with(|s| {
        let mut waiting = s.to_tell.borrow_mut();
        waiting.iter().position(|(thread, _)| *thread == id).map(|i| waiting.remove(i))
    });
    if let Some((id, text)) = next {
        tell(ui, id, &text);
    }
}

fn submit(ui: &Rc<Ui>) {
    let Some(v) = view() else { return };
    let buffer = v.input.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
    let attached = STATE.with(|s| s.pictures.borrow().clone());
    // Words written while the agent answers stay in the box, and Enter says why.
    if STATE.with(|s| s.working.get()) {
        if !text.is_empty() || !attached.is_empty() {
            ui.show_info(STILL_ANSWERING);
        }
        return;
    }
    if text.is_empty() && attached.is_empty() {
        return;
    }
    // A picture still going up, or refused, keeps the message back: the box says which.
    if let Some(why) = pictures_hold() {
        note(&v, why);
        return;
    }
    if !v.send.is_sensitive() {
        return;
    }
    buffer.set_text("");
    let images = attached.iter().filter_map(|p| p.image()).collect();
    set_pictures(&v, Vec::new());
    note(&v, "");
    send_with(ui, &text, images, attached);
}

/// Send `text`: in the thread showing, or as the first message of a new one.
fn send(ui: &Rc<Ui>, text: &str) {
    send_with(ui, text, Vec::new(), Vec::new());
}

/// Send `text` with `images` (the engine's, already up): in the thread showing, or as the first
/// message of a new one. Refused, the words and the pictures (`attached`) go back where they were
/// written.
fn send_with(ui: &Rc<Ui>, text: &str, images: Vec<Value>, attached: Vec<Rc<Attached>>) {
    let ui = ui.clone();
    let text = text.to_string();
    glib::spawn_future_local(async move {
        let ids: Vec<Value> = images.iter().map(|image| image["id"].clone()).collect();
        let to = current();
        let result = match to {
            Some(id) => {
                let mut payload = json!({"id": id, "text": text});
                if !ids.is_empty() {
                    payload["images"] = json!(ids);
                }
                ui.call("thread.send", payload).await.map(|_| id)
            }
            None => {
                let (model, effort) = STATE.with(|s| s.choice.borrow().clone());
                let mut payload = json!({"model": (!model.is_empty()).then_some(model), "effort": (!effort.is_empty()).then_some(effort)});
                // Pictures alone start a thread too.
                if !text.is_empty() {
                    payload["text"] = json!(text);
                }
                if !ids.is_empty() {
                    payload["images"] = json!(ids);
                }
                ui.call("thread.create", payload).await.map(|t| t["id"].as_i64().unwrap_or(0))
            }
        };
        match result {
            Ok(id) if current() != Some(id) => open(&ui, id),
            Ok(_) => {}
            Err(e) => {
                let why = pictures::said(&e);
                Attached::expire(&e, &attached);
                put_back(to, &text, attached);
                if let Some(v) = view().filter(|_| current() == to && !ids.is_empty()) {
                    note(&v, &why);
                }
                ui.show_error(&why);
            }
        }
    });
}

/// What a refused send gives back: into the message box when its thread shows, the words in place
/// of what it holds; else into that thread's draft, to wait there.
fn put_back(to: Option<i64>, text: &str, attached: Vec<Rc<Attached>>) {
    let Some(v) = view() else { return };
    if current() == to {
        v.input.buffer().set_text(text);
        if !attached.is_empty() {
            let mut all = attached;
            all.extend(STATE.with(|s| s.pictures.borrow().clone()));
            all.truncate(MAX_PICTURES);
            set_pictures(&v, all);
        }
        return;
    }
    STATE.with(|s| {
        let mut drafts = s.drafts.borrow_mut();
        let draft = drafts.entry(to).or_default();
        if draft.text.trim().is_empty() {
            draft.text = text.to_string();
        }
        let mut all = attached;
        all.append(&mut draft.pictures);
        all.truncate(MAX_PICTURES);
        draft.pictures = all;
    });
}

fn stop(ui: &Rc<Ui>) {
    let Some(id) = current() else { return };
    STATE.with(|s| s.stopping.set(true));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("thread.stop", json!({"id": id})).await {
            STATE.with(|s| s.stopping.set(false));
            ui.show_error(&e.to_string());
        }
    });
}

/// `thread.*` from the engine.
pub fn event(ui: &Rc<Ui>, ev: &str, payload: &Value) {
    let Some(v) = view() else { return };
    let thread = payload["thread"].as_i64().or_else(|| payload["thread"]["id"].as_i64()).or_else(|| payload["id"].as_i64());
    match ev {
        "thread.delta" if thread == current() => {
            STATE.with(|s| s.pending.borrow_mut().push_str(payload["text"].as_str().unwrap_or("")));
            if v.pending.parent().is_none() {
                turn(&v).append(&v.pending);
            }
            // Words arrive faster than frames: the next frame shows all that came before it.
            if !STATE.with(|s| s.pending_due.replace(true)) {
                v.pending.add_tick_callback(|_, _| {
                    show_pending();
                    glib::ControlFlow::Break
                });
            }
            if v.working_text.text() == STARTING {
                v.working_text.set_text("Working…");
            }
        }
        "thread.message" if thread == current() => {
            draw_message(&v, &payload["message"]);
            stick_to_bottom(&v, false);
        }
        "thread.proposal" => super::ask::changed(&payload["proposal"]),
        "thread.local" => {
            let (model, state) = (payload["model"].as_str().unwrap_or(""), payload["state"].as_str().unwrap_or(""));
            STATE.with(|s| *s.local_state.borrow_mut() = (model.to_string(), state.to_string()));
            reshow_model(&v);
            if state == "failed" {
                ui.show_error(payload["error"].as_str().unwrap_or("The local model stopped"));
            }
        }
        "thread.changed" => {
            note_finished(payload);
            if let Some(id) = thread {
                if payload["deleted"] == true {
                    STATE.with(|s| s.to_tell.borrow_mut().retain(|(waiting, _)| *waiting != id));
                } else if payload["thread"]["working"] != true {
                    tell_waiting(ui, id);
                }
            }
            if thread == current() {
                if payload["deleted"] == true {
                    // Deleted elsewhere: a new thread takes its place, without taking the window
                    // from the page it shows.
                    let showing = *ui.page.borrow() == PAGE;
                    start_thread(ui, showing);
                } else {
                    header(&v, &payload["thread"]);
                    if payload["thread"]["working"] == true {
                        stick_to_bottom(&v, false);
                    }
                }
            }
            // A deleted thread's draft goes with it; leaving the thread above had kept it.
            if let Some(id) = thread.filter(|_| payload["deleted"] == true) {
                STATE.with(|s| s.drafts.borrow_mut().remove(&Some(id)));
            }
            refresh_list_shown(ui);
        }
        _ => {}
    }
}

/// A thread that finishes its reply while it is not the one in view gets a dot in the list until
/// it is opened.
fn note_finished(payload: &Value) {
    let Some(id) = payload["thread"]["id"].as_i64() else {
        if let Some(id) = payload["id"].as_i64() {
            STATE.with(|s| {
                s.unread.borrow_mut().remove(&id);
                s.was_working.borrow_mut().remove(&id);
            });
        }
        return;
    };
    let working = payload["thread"]["working"] == true;
    let seen = current() == Some(id) && the_ui().is_some_and(|ui| *ui.page.borrow() == PAGE);
    STATE.with(|s| {
        if working {
            s.was_working.borrow_mut().insert(id);
        } else if s.was_working.borrow_mut().remove(&id) && !seen {
            s.unread.borrow_mut().insert(id);
        }
    });
}

/// The sidebar's thread list: a box under the Threads keys, filled from `thread.list`.
pub fn sidebar_list() -> gtk::ScrolledWindow {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 1);
    list.add_css_class("threads-list");
    STATE.with(|s| *s.list.borrow_mut() = Some(list.clone()));
    list.connect_map(|_| {
        if STATE.with(|s| s.list_stale.replace(false)) {
            if let Some(ui) = the_ui() {
                refresh_list(&ui);
            }
        }
    });
    let scroll = crate::app::scrolled(&list);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll
}

/// [`refresh_list`] while the list is on screen; off it (the Dev space, a folded sidebar) the read
/// waits for it to show.
pub fn refresh_list_shown(ui: &Rc<Ui>) {
    if STATE.with(|s| s.list.borrow().as_ref().is_some_and(|l| l.is_mapped())) {
        refresh_list(ui);
    } else {
        STATE.with(|s| s.list_stale.set(true));
    }
}

/// Re-read the thread list: one read at a time, and a change during a read reads it once more,
/// so the last change always shows.
pub fn refresh_list(ui: &Rc<Ui>) {
    STATE.with(|s| s.list_dirty.set(true));
    if STATE.with(|s| s.listing.replace(true)) {
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        while STATE.with(|s| s.list_dirty.replace(false)) {
            match ui.call("thread.list", json!({})).await {
                Ok(listed) => {
                    forget_drafts(&listed["threads"]);
                    draw_list(&listed["threads"]);
                }
                Err(e) => {
                    if let Some(list) = STATE.with(|s| s.list.borrow().clone()) {
                        clear(&list);
                        let l = label(&super::pages::unavailable(&e), "threads-list-note");
                        l.set_wrap(true);
                        list.append(&l);
                    }
                }
            }
        }
        STATE.with(|s| s.listing.set(false));
    });
}

/// Let go of the drafts of threads gone from the list: deleted while this window heard no
/// `thread.changed`, which would have taken them.
fn forget_drafts(threads: &Value) {
    let listed: HashSet<i64> = threads.as_array().into_iter().flatten().filter_map(|t| t["id"].as_i64()).collect();
    STATE.with(|s| s.drafts.borrow_mut().retain(|id, _| kept_draft(*id, &listed)));
}

/// Whether the draft for thread `id` (none: a new thread) still has its thread, as far as `listed`
/// shows. A thread newer than any listed may have been made after the list was read, so it keeps
/// its draft.
fn kept_draft(id: Option<i64>, listed: &HashSet<i64>) -> bool {
    let newest = listed.iter().max().copied().unwrap_or(0);
    id.is_none_or(|id| listed.contains(&id) || id > newest)
}

/// Which heading a thread used last on `day` sits under.
fn age(updated_at: &str) -> &'static str {
    let (Ok(then), Ok(now)) = (glib::DateTime::from_iso8601(updated_at, None), glib::DateTime::now_local()) else { return "Earlier" };
    let Ok(then) = then.to_local() else { return "Earlier" };
    let day = |d: &glib::DateTime| glib::DateTime::from_local(d.year(), d.month(), d.day_of_month(), 0, 0, 0.0).ok();
    let (Some(then), Some(today)) = (day(&then), day(&now)) else { return "Earlier" };
    // Hours over 24, rounded: across a daylight-saving change a day is 23 or 25 hours, and
    // whole days counted down put yesterday's threads under Today (as `days_between` does).
    match (today.difference(&then).as_hours() as f64 / 24.0).round() as i64 {
        ..=0 => "Today",
        1 => "Yesterday",
        2..=6 => "This week",
        _ => "Earlier",
    }
}

/// What a thread's row shows at its end: the green lamp while the agent answers, an ink dot for a
/// reply not yet read, or nothing.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    None,
    Working,
    Unread,
}

/// A thread's row in the sidebar list, kept from one read of the list to the next and changed in
/// place, with what it shows now.
struct ListRow {
    key: gtk::Button,
    row: gtk::Box,
    title: gtk::Label,
    mark: Option<gtk::Box>,
    shown: (String, Mark, Option<String>),
}

#[derive(Default)]
struct ListRows {
    rows: HashMap<i64, ListRow>,
    headings: HashMap<&'static str, gtk::Label>,
    note: Option<gtk::Label>,
}

thread_local! {
    static LIST_ROWS: RefCell<ListRows> = RefCell::default();
}

fn list_row(id: i64) -> ListRow {
    let key = button("", "nav");
    key.add_css_class("threads-row");
    key.set_widget_name(&format!("thread-{id}"));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = label("", "nav-label");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_hexpand(true);
    row.append(&title);
    key.set_child(Some(&row));
    key.connect_clicked(move |_| {
        if let Some(ui) = the_ui() {
            open(&ui, id);
        }
    });
    ListRow { key, row, title, mark: None, shown: (String::new(), Mark::None, None) }
}

/// Show `title`, `mark` and `preview` in `r`, touching only what changed.
fn show_list_row(r: &mut ListRow, title: &str, mark: Mark, preview: Option<&str>) {
    if r.shown.0 != title {
        r.title.set_text(title);
        r.shown.0 = title.to_string();
    }
    if r.shown.1 != mark {
        if let Some(old) = r.mark.take() {
            r.row.remove(&old);
        }
        // Green while the agent answers; an ink dot for a reply not yet read. A finished reply is
        // news, not an alarm.
        let (class, said) = match mark {
            Mark::Working => ("threads-lamp", "Claude is answering"),
            Mark::Unread => ("threads-unread", "New reply"),
            Mark::None => ("", ""),
        };
        if class.is_empty() {
            r.key.reset_property(gtk::AccessibleProperty::Description);
        } else {
            let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            lamp.add_css_class(class);
            lamp.set_valign(gtk::Align::Center);
            r.row.append(&lamp);
            r.mark = Some(lamp);
            r.key.update_property(&[gtk::accessible::Property::Description(said)]);
        }
        r.shown.1 = mark;
    }
    if r.shown.2.as_deref() != preview {
        r.key.set_tooltip_text(preview);
        r.shown.2 = preview.map(str::to_string);
    }
}

/// Draw the list from `thread.list`: each thread keeps its row, changed in place, and rows move
/// only when the order changed. A read that changed nothing changes no widget.
fn draw_list(threads: &Value) {
    let Some(list) = STATE.with(|s| s.list.borrow().clone()) else { return };
    let threads: &[Value] = threads.as_array().map_or(&[], Vec::as_slice);
    let mut wanted: Vec<gtk::Widget> = Vec::with_capacity(threads.len() + 4);
    LIST_ROWS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = &mut *cache;
        let mut heading = "";
        let mut headed: HashSet<&'static str> = HashSet::new();
        let mut seen = HashSet::new();
        for t in threads {
            let group = age(t["updated_at"].as_str().unwrap_or(""));
            if group != heading {
                heading = group;
                // A heading shows once; were the list ever out of order, a repeat gets its own.
                let name = if headed.insert(group) {
                    cache.headings.entry(group).or_insert_with(|| label(group, "section-label")).clone()
                } else {
                    label(group, "section-label")
                };
                wanted.push(name.upcast());
            }
            let id = t["id"].as_i64().unwrap_or(0);
            if !seen.insert(id) {
                continue;
            }
            let mark = if t["working"] == true {
                STATE.with(|s| s.was_working.borrow_mut().insert(id));
                Mark::Working
            } else if STATE.with(|s| s.unread.borrow().contains(&id)) {
                Mark::Unread
            } else {
                Mark::None
            };
            let r = cache.rows.entry(id).or_insert_with(|| list_row(id));
            show_list_row(r, t["title"].as_str().unwrap_or("Thread"), mark, t["preview"].as_str());
            wanted.push(r.key.clone().upcast());
        }
        cache.rows.retain(|id, _| seen.contains(id));
        if threads.is_empty() {
            let note = cache.note.get_or_insert_with(|| {
                let note = label("No threads yet. Ctrl N starts one.", "threads-list-note");
                note.set_wrap(true);
                note
            });
            wanted.push(note.clone().upcast());
        }
    });
    let mut previous: Option<gtk::Widget> = None;
    for w in &wanted {
        if w.parent().as_ref() == Some(list.upcast_ref()) {
            if w.prev_sibling() != previous {
                list.reorder_child_after(w, previous.as_ref());
            }
        } else {
            if let Some(parent) = w.parent().and_downcast::<gtk::Box>() {
                parent.remove(w);
            }
            list.insert_child_after(w, previous.as_ref());
        }
        previous = Some(w.clone());
    }
    // What is left after the last wanted widget: rows of deleted threads, headings no longer
    // used, a note or an error from an earlier read.
    while let Some(extra) = previous.as_ref().map_or_else(|| list.first_child(), |w| w.next_sibling()) {
        list.remove(&extra);
    }
    mark_selected();
}

fn mark_selected() {
    let Some(list) = STATE.with(|s| s.list.borrow().clone()) else { return };
    let selected = current().map(|id| format!("thread-{id}"));
    let mut child = list.first_child();
    while let Some(w) = child {
        if w.has_css_class("threads-row") {
            if Some(w.widget_name().to_string()) == selected {
                w.add_css_class("selected");
            } else {
                w.remove_css_class("selected");
            }
        }
        child = w.next_sibling();
    }
}

/// The page came into view, or something it shows moved: the list, the thread, the panel.
pub async fn refresh(ui: &Rc<Ui>) {
    if let Some(id) = current() {
        STATE.with(|s| s.unread.borrow_mut().remove(&id));
    }
    refresh_list(ui);
    check_claude(ui);
    load_local(ui);
    match current() {
        Some(id) => {
            let after = STATE.with(|s| s.last.get());
            if let Ok(read) = ui.call("thread.get", json!({"id": id, "after": after})).await {
                if current() == Some(id) {
                    if let Some(v) = view() {
                        header(&v, &read["thread"]);
                        for m in read["messages"].as_array().into_iter().flatten() {
                            draw_message(&v, m);
                        }
                        if !STATE.with(|s| s.working.get()) {
                            settle_tools();
                        }
                    }
                }
            }
        }
        None => greet(ui).await,
    }
    if let Some(v) = view() {
        update_send(&v);
    }
    refresh_panel(ui);
}

/// A panel read of `tab` starts: its key is marked in `tabs`, and the serial it returns is the
/// newest, so an older read that lands after it draws nothing.
fn panel_read_starts(tabs: &[(&'static str, gtk::Button)], tab: &str) -> u64 {
    for (key, button) in tabs {
        if *key == tab {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
    }
    STATE.with(|s| {
        let next = s.panel_serial.get().wrapping_add(1);
        s.panel_serial.set(next);
        next
    })
}

/// `arbiter.changed`: re-read the panel when it shows Arbiter.
pub fn refresh_arbiter_panel(ui: &Rc<Ui>) {
    if STATE.with(|s| s.source.get()) == "arbiter" {
        refresh_panel(ui);
    }
}

/// Re-read Arbiter's panel view; one source's read never lands on the other's panel.
fn refresh_arbiter(ui: &Rc<Ui>, v: &View) {
    let tab = STATE.with(|s| s.arbiter_tab.get());
    let serial = panel_read_starts(&v.arbiter_panel_tabs, tab);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let read = super::arbiter::panel_read(&ui, tab).await;
        if STATE.with(|s| s.panel_serial.get() != serial || s.source.get() != "arbiter" || s.arbiter_tab.get() != tab) {
            return;
        }
        let Some(v) = view() else { return };
        clear(&v.panel_body);
        match read {
            Ok(value) => super::arbiter::panel_draw(&ui, &v.panel_body, tab, &value),
            Err(e) => panel_failed(&ui, &v.panel_body, &super::arbiter::said(&e)),
        }
    });
}

/// `gym.changed`: re-read the panel when it shows Avex.
pub fn refresh_avex_panel(ui: &Rc<Ui>) {
    if STATE.with(|s| s.source.get()) == "avex" {
        refresh_panel(ui);
    }
}

/// Re-read Avex's panel view; one source's read never lands on another's panel.
fn refresh_avex(ui: &Rc<Ui>, v: &View) {
    let tab = STATE.with(|s| s.avex_tab.get());
    let serial = panel_read_starts(&v.avex_panel_tabs, tab);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let read = super::avex::panel_read(&ui, tab).await;
        if STATE.with(|s| s.panel_serial.get() != serial || s.source.get() != "avex" || s.avex_tab.get() != tab) {
            return;
        }
        let Some(v) = view() else { return };
        clear(&v.panel_body);
        match read {
            Ok(value) => super::avex::panel_draw(&ui, &v.panel_body, tab, &value),
            Err(e) => panel_failed(&ui, &v.panel_body, &super::avex::said(&e)),
        }
    });
}

/// A panel read that failed: why, and a key to read it again, as every page has (`failed`).
fn panel_failed(ui: &Rc<Ui>, body: &gtk::Box, why: &str) {
    let l = label(why, "money-muted");
    l.set_wrap(true);
    body.append(&l);
    body.append(&super::kit::retry_key(ui, refresh_panel));
}

/// Re-read the panel's view, Tally's, Arbiter's or Avex's, if it is open. Only the newest read
/// draws, and none while a budget is being changed in it: that one comes when its editor closes.
pub fn refresh_panel(ui: &Rc<Ui>) {
    let Some(v) = view() else { return };
    if !v.panel.is_visible() || aide().is_some() {
        return;
    }
    if STATE.with(|s| s.editing_budget.get()) {
        STATE.with(|s| s.panel_stale.set(true));
        return;
    }
    match STATE.with(|s| s.source.get()) {
        "arbiter" => return refresh_arbiter(ui, &v),
        "avex" => return refresh_avex(ui, &v),
        _ => {}
    }
    let tab = STATE.with(|s| s.panel_tab.get());
    let serial = panel_read_starts(&v.panel_tabs, tab);
    if v.panel_body.first_child().is_none() {
        v.panel_body.append(&label("Reading the ledger…", "money-muted"));
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        // The Invest view also reads the balances: an account valued by hand is Tally's balance.
        let mut lists = None;
        let read = match tab {
            "entries" => ui.call("money.tx.list", json!({"limit": ENTRIES_SHOWN})).await,
            "invest" => {
                let (summary, listed) = tokio::join!(ui.call("money.invest.summary", json!({})), ui.call("money.lists", json!({})));
                lists = listed.ok();
                summary
            }
            _ => ui.call("money.summary", json!({})).await,
        };
        if STATE.with(|s| s.panel_serial.get() != serial || s.panel_tab.get() != tab || s.source.get() != "tally") {
            return;
        }
        let Some(v) = view() else { return };
        clear(&v.panel_body);
        match read {
            Ok(value) => match tab {
                "entries" => panel_entries(&ui, &v.panel_body, &value),
                "invest" => panel_invest(&ui, &v.panel_body, &value, lists.as_ref()),
                _ => panel_overview(&ui, &v.panel_body, &value),
            },
            Err(e) => {
                let why = match &e {
                    crate::client::Error::Bus(b) if tab == "invest" && b.code == "bus.unknown_op" => {
                        String::from("This engine does not read investments yet. Update Relay's engine to see them here.")
                    }
                    e => super::pages::unavailable(e),
                };
                panel_failed(&ui, &v.panel_body, &why);
            }
        }
    });
}

fn caption(text: &str) -> gtk::Label {
    label(text, "threads-caption")
}

/// `row` as a key. The panel's rows act where the person is: an entry opens its editor, a budget
/// its own ([`edit_budget`]), the rest draft a question into the thread ([`asking`]).
fn row_key(row: &gtk::Box, about: &str, run: impl Fn() + 'static) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("threads-row-key");
    key.set_child(Some(row));
    key.set_tooltip_text(Some(about));
    key.connect_clicked(move |_| run());
    key
}

/// A run that opens `page`: the panel's own "Open …" keys, never its rows.
fn go(ui: &Rc<Ui>, page: &'static str) -> impl Fn() + 'static {
    let weak = Rc::downgrade(ui);
    move || {
        if let Some(ui) = weak.upgrade() {
            ui.navigate(page);
        }
    }
}

/// A run that drafts `question` into the thread showing, or asks it in a new one ([`draft`]).
fn asking(ui: &Rc<Ui>, question: String) -> impl Fn() + 'static {
    let weak = Rc::downgrade(ui);
    move || {
        if let Some(ui) = weak.upgrade() {
            draft(&ui, &question);
        }
    }
}

fn panel_overview(ui: &Rc<Ui>, body: &gtk::Box, s: &Value) {
    super::remember_currency(s);
    if s["empty"] == true {
        let note = label("Tally has nothing yet. Add an account, load the sample household, or import a backup.", "money-muted");
        note.set_wrap(true);
        body.append(&note);
        let open = button("Open Tally", "");
        open.set_halign(gtk::Align::Start);
        let run = go(ui, "money-home");
        open.connect_clicked(move |_| run());
        body.append(&open);
        return;
    }
    let fmt = super::formatter();
    let period = &s["period"];
    let days = period["days"].as_i64().unwrap_or(0);
    let days_left = period["days_left"].as_i64().unwrap_or(0);
    let pace = &s["pace"];
    let status = pace["status"].as_str().unwrap_or("");
    let budget = pace["budget"].as_i64().unwrap_or(0);
    let spent = s["spent"].as_i64().unwrap_or(0);
    let income = s["income"].as_i64().unwrap_or(0);

    // What is left, the pace across the period, and the month's money in one line.
    let head = gtk::Box::new(gtk::Orientation::Vertical, 6);
    head.add_css_class("threads-summary");
    let when = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    when.append(&crate::money::pages_glyph("calendar", 13));
    let day = (days - days_left + 1).clamp(1, days.max(1));
    when.append(&label(&format!("{} · day {day} of {days}", super::pages::period_name(period)), "threads-panel-eyebrow"));
    head.append(&when);
    if budget > 0 {
        let left = pace["remaining"].as_i64().unwrap_or(0);
        let figure = label(&left_or_over(&fmt, left), "threads-panel-figure");
        if left < 0 {
            figure.add_css_class("money-over");
        }
        head.append(&figure);
        let daily = pace["daily_allowance"].as_i64().unwrap_or(0);
        let sub = if daily > 0 {
            format!("of {} · {} a day from here", fmt.format_whole(budget), fmt.format_whole(daily))
        } else {
            format!("of {} planned this month", fmt.format_whole(budget))
        };
        head.append(&label(&sub, "money-muted"));
        let bar = meter_in(pace["spent_fraction"].as_f64().unwrap_or(0.0), pace["pace_fraction"].as_f64(), tone(status, None), 6);
        bar.set_margin_top(6);
        bar.set_tooltip_text(Some("The tick is where an even spend would be today"));
        head.append(&bar);
        if let Some(line) = s["lines"]["pace"].as_str().filter(|l| !l.is_empty()) {
            let l = label(line, "threads-panel-detail");
            l.set_wrap(true);
            if let Some(class) = tone_class(status) {
                l.add_css_class(class);
            }
            head.append(&l);
        }
        head.append(&label(&format!("Spent {} · In {}", fmt.format_whole(spent), fmt.format_whole(income)), "threads-panel-figures"));
    } else {
        head.append(&label(&format!("{} spent", fmt.format_whole(spent)), "threads-panel-figure"));
        head.append(&label(&format!("In {} · no monthly budget yet: set one in Tally's Plan.", fmt.format_whole(income)), "money-muted"));
    }
    body.append(&head);

    panel_budgets(ui, body, s);

    // A bill to pay asks whether it can be covered.
    let bills: Vec<&Value> = s["bills"].as_array().into_iter().flatten().take(3).collect();
    if !bills.is_empty() {
        let list = block(body, "Coming up", "");
        for bill in bills {
            let income = bill["type"] == "INCOME";
            let amount = bill["amount"].as_i64().unwrap_or(0);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("threads-entry");
            row.append(&tile(if income { "income" } else { "calendar" }));
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            words.set_hexpand(true);
            let name = label(bill["name"].as_str().unwrap_or(""), "threads-panel-name");
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.append(&name);
            let due = label(bill["due_line"].as_str().unwrap_or(""), "threads-panel-detail");
            if bill["days_until"].as_i64().unwrap_or(99) <= 3 && !income {
                due.add_css_class("money-ahead");
            }
            words.append(&due);
            row.append(&words);
            let figure = label(&if income { fmt.format_signed(amount) } else { format!("−{}", fmt.format(amount)) }, "threads-panel-figures");
            if income {
                figure.add_css_class("money-in");
            }
            figure.set_valign(gtk::Align::Center);
            row.append(&figure);
            if income {
                list.append(&row);
            } else {
                list.append(&row_key(&row, "Ask whether you can cover it", asking(ui, cover_question(bill))));
            }
        }
    }

    let recent: Vec<&Value> = s["recent"].as_array().into_iter().flatten().take(5).collect();
    let list = block(body, "Recent", "");
    block_link(&list, "All entries", go(ui, "money-transactions"));
    if recent.is_empty() {
        list.append(&label("Nothing logged this period yet.", "money-muted"));
    }
    for tx in recent {
        list.append(&entry_row(ui, tx, true));
    }
}

/// Overview's budgets, the furthest ahead of pace first: [`BUDGETS_SHOWN`] of them, or all once
/// the person asks. Each changes its amount in place ([`edit_budget`]).
fn panel_budgets(ui: &Rc<Ui>, body: &gtk::Box, s: &Value) {
    let mut budgets: Vec<Value> = s["budgets"].as_array().into_iter().flatten().filter(|b| b["budget"].as_i64().unwrap_or(0) > 0).cloned().collect();
    let ahead = |b: &Value| b["spent"].as_i64().unwrap_or(0) as f64 / b["budget"].as_i64().unwrap_or(1).max(1) as f64;
    budgets.sort_by(|a, b| ahead(b).total_cmp(&ahead(a)));
    let list = block(body, "Budgets", "");
    block_link(&list, "Plan", go(ui, "money-plan"));
    if budgets.is_empty() {
        let note = label("No budgets yet. Set them in Tally's Plan.", "money-muted");
        note.set_wrap(true);
        list.append(&note);
        return;
    }
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.append(&rows);
    fill_budgets(ui, &rows, Rc::new(budgets), s["pace"]["pace_fraction"].as_f64());
}

/// The budget rows in `rows`, and the key that shows them all or fewer again: it redraws them
/// from the same reading, so the panel keeps its place.
fn fill_budgets(ui: &Rc<Ui>, rows: &gtk::Box, budgets: Rc<Vec<Value>>, pace: Option<f64>) {
    clear(rows);
    let all = STATE.with(|s| s.all_budgets.get());
    let shown = if all { budgets.len() } else { budgets.len().min(BUDGETS_SHOWN) };
    for b in &budgets[..shown] {
        rows.append(&budget_row(ui, b, pace));
    }
    if budgets.len() <= BUDGETS_SHOWN {
        return;
    }
    let more = button(&if all { String::from("Show fewer") } else { format!("Show all {}", budgets.len()) }, "quiet");
    more.add_css_class("threads-more");
    more.set_halign(gtk::Align::Start);
    let (weak_ui, weak_rows) = (Rc::downgrade(ui), rows.downgrade());
    more.connect_clicked(move |_| {
        let (Some(ui), Some(rows)) = (weak_ui.upgrade(), weak_rows.upgrade()) else { return };
        STATE.with(|s| s.all_budgets.set(!s.all_budgets.get()));
        fill_budgets(&ui, &rows, budgets.clone(), pace);
    });
    rows.append(&more);
}

/// One budget: its badge, name and what is left over the meter of what it used. It opens its
/// editor beside the panel.
fn budget_row(ui: &Rc<Ui>, b: &Value, pace: Option<f64>) -> gtk::Button {
    let fmt = super::formatter();
    let (spent, budget) = (b["spent"].as_i64().unwrap_or(0), b["budget"].as_i64().unwrap_or(0));
    let status = b["status"].as_str().unwrap_or("");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("threads-budget");
    row.append(&badge_sized(b["icon"].as_str().unwrap_or("dots"), b["color"].as_i64().unwrap_or(10), 24));
    let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
    right.set_hexpand(true);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(b["name"].as_str().unwrap_or(""), "threads-panel-name");
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&name);
    let left = budget - spent;
    let rest = label(&left_or_over(&fmt, left), "threads-panel-figures");
    if let Some(class) = tone_class(status) {
        rest.add_css_class(class);
    }
    line.append(&rest);
    right.append(&line);
    let bar = meter_in(spent as f64 / budget.max(1) as f64, pace, tone(status, b["color"].as_i64()), 4);
    bar.set_tooltip_text(Some(&format!("{} of {}", fmt.format_whole(spent), fmt.format_whole(budget))));
    right.append(&bar);
    row.append(&right);
    let key = gtk::Button::new();
    key.add_css_class("threads-row-key");
    key.add_css_class("threads-budget-key");
    key.set_child(Some(&row));
    key.set_tooltip_text(Some("Change this budget"));
    let weak = Rc::downgrade(ui);
    let b = b.clone();
    key.connect_clicked(move |key| {
        if let Some(ui) = weak.upgrade() {
            edit_budget(&ui, key, &b);
        }
    });
    key
}

/// A budget editor's line from the last whole months, oldest first: spending in its category
/// (`here`) and in the whole ledger (`all`). A month with no spending anywhere came before the
/// ledger was kept, so it is left out rather than averaged in as a zero. Empty when nothing is
/// known. "Last month $298 · 3-month average $310".
fn month_history(here: &[i64], all: &[i64]) -> String {
    let kept: Vec<i64> = here.iter().zip(all).filter(|(_, any)| **any > 0).map(|(v, _)| *v).collect();
    match kept.len() {
        0 => return String::new(),
        _ if kept.iter().any(|v| *v != 0) => {}
        1 => return String::from("Nothing spent here last month"),
        n => return format!("Nothing spent here in the last {n} months"),
    }
    let fmt = super::formatter();
    let mut parts = Vec::new();
    let last_kept = here.len() == all.len() && all.last().is_some_and(|a| *a > 0);
    if let Some(last) = here.last().filter(|_| last_kept) {
        parts.push(format!("Last month {}", fmt.format_whole(*last)));
    }
    if kept.len() > 1 {
        parts.push(format!("{}-month average {}", kept.len(), fmt.format_whole(kept.iter().sum::<i64>() / kept.len() as i64)));
    }
    parts.join(" · ")
}

/// A budget's editor, a popover beside its row: the amount, what was spent in the last three
/// months to judge it by (`money.series`), and Save (`money.budget.set`). An empty amount is no
/// budget. The panel holds still while it is open ([`refresh_panel`]).
fn edit_budget(ui: &Rc<Ui>, anchor: &gtk::Button, b: &Value) {
    let fmt = super::formatter();
    let name = b["name"].as_str().unwrap_or("").to_string();
    let category = b["category_id"].as_i64();
    let amount = b["budget"].as_i64().unwrap_or(0);
    let pop = gtk::Popover::new();
    pop.add_css_class("threads-budget-pop");
    pop.set_position(gtk::PositionType::Left);
    pop.set_parent(anchor);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.add_css_class("threads-budget-edit");
    body.set_size_request(268, -1);
    let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let title = label(&format!("{name} budget"), "threads-card-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(&title);
    head.append(&label(&format!("{} spent so far this month", fmt.format_whole(b["spent"].as_i64().unwrap_or(0))), "threads-panel-detail"));
    body.append(&head);
    let field_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let symbol = label(&fmt.symbol, "money-suffix");
    symbol.set_valign(gtk::Align::Center);
    field_row.append(&symbol);
    let field = gtk::Entry::new();
    field.add_css_class("threads-budget-field");
    field.set_hexpand(true);
    gtk::prelude::EntryExt::set_alignment(&field, 1.0);
    field.set_placeholder_text(Some("No budget"));
    if amount > 0 {
        field.set_text(&fmt.format_input(amount));
    }
    field.update_property(&[gtk::accessible::Property::Label(&format!("Monthly budget for {name}"))]);
    field_row.append(&field);
    body.append(&field_row);
    let history = label("Reading the last three months…", "threads-panel-detail");
    history.set_xalign(0.0);
    history.set_wrap(true);
    body.append(&history);
    let error = label("", "threads-error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    body.append(&error);
    let save = button("Save", "primary");
    save.set_halign(gtk::Align::End);
    body.append(&save);
    pop.set_child(Some(&body));

    // The panel waits while this is open, then reads what changed meanwhile.
    STATE.with(|s| s.editing_budget.set(true));
    pop.connect_closed(|pop| {
        let pop = pop.clone();
        glib::idle_add_local_once(move || {
            if pop.parent().is_some() {
                pop.unparent();
            }
            STATE.with(|s| s.editing_budget.set(false));
            if STATE.with(|s| s.panel_stale.replace(false)) {
                if let Some(ui) = the_ui() {
                    refresh_panel(&ui);
                }
            }
        });
    });

    let commit = {
        let (weak, pop, field, error, save) = (Rc::downgrade(ui), pop.downgrade(), field.downgrade(), error.downgrade(), save.downgrade());
        let name = name.clone();
        move || {
            let (Some(ui), Some(pop), Some(field), Some(error), Some(save)) = (weak.upgrade(), pop.upgrade(), field.upgrade(), error.upgrade(), save.upgrade()) else { return };
            let typed = field.text();
            let value = if typed.trim().is_empty() { Some(0) } else { super::formatter().parse(&typed).filter(|v| *v >= 0) };
            let Some(value) = value else {
                field.add_css_class("error");
                error.set_text("Type an amount, like 250 or 250.00. Leave it empty for no budget.");
                error.set_visible(true);
                return;
            };
            if value == amount {
                pop.popdown();
                return;
            }
            save.set_sensitive(false);
            let payload = |amount: i64| match category {
                Some(id) => json!({"amount": amount, "category_id": id}),
                None => json!({"amount": amount}),
            };
            // Said, with Undo (the amount it had), as an entry's save is.
            let said = if value > 0 {
                format!("Budget for {name}: {}", super::formatter().format_whole(value))
            } else {
                format!("No budget for {name}")
            };
            let back = payload(amount);
            let undo: super::Action = {
                let weak = Rc::downgrade(&ui);
                Rc::new(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let back = back.clone();
                    glib::spawn_future_local(async move {
                        if let Err(e) = ui.call("money.budget.set", back).await {
                            ui.show_error(&super::pages::unavailable(&e));
                        }
                    });
                })
            };
            let set = payload(value);
            glib::spawn_future_local(async move {
                match ui.call("money.budget.set", set).await {
                    Ok(_) => {
                        pop.popdown();
                        super::toast(&ui, &said, Some(undo));
                    }
                    Err(e) => {
                        error.set_text(&super::pages::unavailable(&e));
                        error.set_visible(true);
                        save.set_sensitive(true);
                    }
                }
            });
        }
    };
    let commit = Rc::new(commit);
    let on_enter = commit.clone();
    field.connect_activate(move |_| on_enter());
    save.connect_clicked(move |_| commit());
    field.connect_changed(|field| field.remove_css_class("error"));

    // The last three whole months, oldest first: the latest and their average. A month with no
    // spending anywhere in the ledger came before it was kept, so it does not count as a zero.
    let weak = history.downgrade();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let months = |category: Option<&str>| json!({"measure": "spending", "by": "period", "periods": 3, "period_offset": -1, "category": category});
        let (here, all) = tokio::join!(ui.call("money.series", months(Some(&name))), ui.call("money.series", months(None)));
        let Some(history) = weak.upgrade() else { return };
        let values = |read: Result<Value, crate::client::Error>| -> Vec<i64> {
            read.ok().and_then(|r| r["series"][0]["values"].as_array().map(|v| v.iter().filter_map(Value::as_i64).collect())).unwrap_or_default()
        };
        history.set_text(&month_history(&values(here), &values(all)));
        history.set_visible(!history.text().is_empty());
    });
    pop.popup();
    field.grab_focus();
    field.select_region(0, -1);
}

/// One entry in the panel: its badge, what it was over its category and day, and the amount.
/// Clicking it edits it; an entry this thread added says so.
fn entry_row(ui: &Rc<Ui>, tx: &Value, dated: bool) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("threads-entry");
    let transfer = tx["type"] == "TRANSFER";
    row.append(&super::pages::entry_badge(tx, 24));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let category = super::pages::entry_category(tx);
    let note = tx["note"].as_str().map(str::trim).filter(|n| !n.is_empty());
    let title = label(note.unwrap_or(&category), "threads-panel-name");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    let mut detail = Vec::new();
    if note.is_some() {
        detail.push(category.clone());
    }
    if transfer {
        detail.push(format!("{} → {}", tx["account"].as_str().unwrap_or(""), tx["to_account"].as_str().unwrap_or("")));
    }
    if dated {
        detail.push(human_date(tx["date"].as_str().unwrap_or("")));
    }
    if detail.is_empty() {
        detail.push(tx["account"].as_str().unwrap_or("").to_string());
    }
    let ours = tx["id"].as_i64().is_some_and(|id| STATE.with(|s| s.added.borrow().contains(&id)));
    if ours {
        detail.push(String::from("from this thread"));
    }
    let d = label(&detail.join(" · "), "threads-panel-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    row.append(&words);
    let figure = label(&amount_text(tx), "threads-panel-figures");
    if tx["type"] == "INCOME" {
        figure.add_css_class("money-in");
    }
    figure.set_valign(gtk::Align::Center);
    row.append(&figure);
    let weak = Rc::downgrade(ui);
    let edit = tx.clone();
    let key = row_key(&row, "Edit this entry", move || {
        if let Some(ui) = weak.upgrade() {
            super::edit_entry(&ui, edit.clone());
        }
    });
    if ours {
        key.add_css_class("from-thread");
    }
    key
}

/// The Entries view: the period's in, out and net (over all of it: `money.tx.list` totals every
/// entry, listed or not), its newest [`ENTRIES_SHOWN`] entries by day, and the key to the
/// Entries page for the rest.
fn panel_entries(ui: &Rc<Ui>, body: &gtk::Box, page: &Value) {
    let rows: Vec<&Value> = page["transactions"].as_array().into_iter().flatten().collect();
    let fmt = super::formatter();
    let (income, spent) = (page["income"].as_i64().unwrap_or(0), page["spent"].as_i64().unwrap_or(0));
    let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
    head.add_css_class("threads-summary");
    // A full read lists only the newest of the period; its figures count all of it.
    let mut when = super::pages::period_name(&page["period"]);
    if rows.len() >= ENTRIES_SHOWN as usize {
        when.push_str(&format!(" · latest {ENTRIES_SHOWN}"));
    }
    head.append(&label(&when, "threads-panel-eyebrow"));
    let total = super::pages::signed_whole(&fmt, income - spent);
    head.append(&label(&format!("In {} · Out {} · Net {total}", fmt.format_whole(income), fmt.format_whole(spent)), "threads-panel-figures"));
    body.append(&head);
    let open = button("Open Entries", "quiet");
    open.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    open.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::pages::show_entries(&ui, None, None);
        }
    });
    if rows.is_empty() {
        body.append(&label("No entries this period.", "money-muted"));
        body.append(&open);
        return;
    }
    let mut start = 0;
    while start < rows.len() {
        let day = rows[start]["date"].as_str().unwrap_or("").to_string();
        let end = rows[start..].iter().position(|t| t["date"].as_str().unwrap_or("") != day).map_or(rows.len(), |p| start + p);
        let net: i64 = rows[start..end]
            .iter()
            .map(|t| match t["type"].as_str() {
                Some("INCOME") => t["amount"].as_i64().unwrap_or(0),
                Some("EXPENSE") => -t["amount"].as_i64().unwrap_or(0),
                _ => 0,
            })
            .sum();
        let list = block(body, &human_date(&day), &fmt.format_signed(net));
        for tx in &rows[start..end] {
            list.append(&entry_row(ui, tx, false));
        }
        start = end;
    }
    body.append(&open);
}

/// A registration's hue and name, as the Investments page draws them (`money_invest.rs`): the hue
/// is data, on badges and the allocation bar, never on words.
fn registration_hue(registration: Option<&str>) -> i64 {
    super::invest::registration_hue(registration.unwrap_or(""))
}

fn registration_name(registration: Option<&str>) -> &'static str {
    super::invest::registration_label(registration.unwrap_or(""))
}

/// "+0.7%", "−1.2%": basis points as a signed share, as the Investments page writes them.
fn percent(bps: i64) -> String {
    relay_client::figures::percent_bps(bps, true)
}

/// A holding's symbol as its mark.
fn symbol_tile(symbol: &str) -> gtk::Box {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("threads-symbol");
    tile.set_valign(gtk::Align::Center);
    tile.set_size_request(40, 24);
    let l = label(symbol, "");
    l.set_xalign(0.5);
    l.set_hexpand(true);
    l.set_max_width_chars(5);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tile.append(&l);
    tile
}

/// A row of the Invest view: its mark, a name over its detail, the value over its change. It drafts
/// `question` into the thread; the view's last key opens the Investments page.
fn invest_row(ui: &Rc<Ui>, lead: &gtk::Box, title: &str, detail: &str, value: &str, change: Option<(String, Option<&'static str>)>, question: String) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("threads-entry");
    row.append(lead);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let name = label(title, "threads-panel-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let d = label(detail, "threads-panel-detail");
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    row.append(&words);
    let figures = gtk::Box::new(gtk::Orientation::Vertical, 1);
    figures.set_valign(gtk::Align::Center);
    let shown = label(value, "threads-panel-figures");
    shown.set_xalign(1.0);
    figures.append(&shown);
    if let Some((text, class)) = change {
        let l = label(&text, "threads-panel-figures");
        l.set_xalign(1.0);
        if let Some(class) = class {
            l.add_css_class(class);
        }
        figures.append(&l);
    }
    row.append(&figures);
    row_key(&row, "Ask about this", asking(ui, question))
}

/// The Invest view (`money.invest.summary`): what the portfolio is worth and has gained, how it is
/// split, the accounts, the room left this year and the largest holdings. An account no file has
/// filled is worth its balance in `lists` (`money.lists`), as the Investments page counts it.
fn panel_invest(ui: &Rc<Ui>, body: &gtk::Box, p: &Value, lists: Option<&Value>) {
    super::remember_currency(p);
    if p["empty"] == true {
        invest_empty(ui, body, p["currency"].as_str().unwrap_or(""));
        return;
    }
    let fmt = super::formatter();
    let head = gtk::Box::new(gtk::Orientation::Vertical, 6);
    head.add_css_class("threads-summary");
    let when = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    when.append(&crate::money::pages_glyph("chart", 13));
    // The newest report's date: a portfolio is only as fresh as its last file.
    let as_of = p["as_of"].as_str().filter(|d| !d.is_empty()).map(|d| format!("As of {}", super::invest::as_of(d.get(..10).unwrap_or(d))));
    when.append(&label(&as_of.unwrap_or_else(|| String::from("No report imported yet")), "threads-panel-eyebrow"));
    head.append(&when);
    let balances = lists.map(|l| crate::app::rows(l, "accounts")).unwrap_or_default();
    let by_hand = |a: &Value| super::invest::by_hand(a).then(|| balances.iter().find(|b| b["id"] == a["id"]).and_then(|b| b["balance"].as_i64())).flatten();
    let hand: i64 = p["accounts"].as_array().into_iter().flatten().filter_map(&by_hand).sum();
    head.append(&label(&fmt.format_whole(p["value"].as_i64().unwrap_or(0) + hand), "threads-panel-figure"));
    let gain = p["gain"].as_i64().unwrap_or(0);
    let mut line = format!("{} gain", super::pages::signed_whole(&fmt, gain));
    if let Some(bps) = p["gain_bps"].as_i64() {
        line.push_str(&format!(" · {} on book value", percent(bps)));
    }
    let gained = label(&line, "threads-panel-detail");
    gained.set_wrap(true);
    if let Some(class) = gain_class(gain) {
        gained.add_css_class(class);
    }
    head.append(&gained);
    // How it is split by registration: the bar and its legend.
    let parts: Vec<(i64, f64, String)> = p["allocation"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["value"].as_i64().unwrap_or(0) > 0)
        .map(|a| {
            let registration = a["registration"].as_str();
            let bps = a["share_bps"].as_i64().unwrap_or(0);
            (registration_hue(registration), bps as f64 / 10_000.0, format!("{} {}", registration_name(registration), super::invest::share(bps)))
        })
        .collect();
    if !parts.is_empty() {
        let shares: Vec<(i64, f64)> = parts.iter().map(|(hue, part, _)| (*hue, *part)).collect();
        let bar = super::pages::allocation_bar(&shares, 8);
        bar.set_margin_top(6);
        let spoken: Vec<&str> = parts.iter().map(|(_, _, words)| words.as_str()).collect();
        bar.update_property(&[gtk::accessible::Property::Label(&spoken.join(", "))]);
        head.append(&bar);
        let legend = gtk::FlowBox::new();
        legend.add_css_class("threads-legend");
        legend.set_selection_mode(gtk::SelectionMode::None);
        legend.set_column_spacing(12);
        legend.set_row_spacing(4);
        legend.set_max_children_per_line(4);
        for (hue, _, words) in &parts {
            let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            item.append(&super::pages::dot(*hue));
            item.append(&label(words, "threads-chart-legend"));
            let child = gtk::FlowBoxChild::new();
            child.set_child(Some(&item));
            child.set_focusable(false);
            legend.insert(&child, -1);
        }
        head.append(&legend);
    }
    body.append(&head);

    let accounts: Vec<&Value> = p["accounts"].as_array().into_iter().flatten().collect();
    if !accounts.is_empty() {
        let list = block(body, "Accounts", "");
        for a in accounts {
            let registration = a["registration"].as_str();
            let mut detail = vec![registration_name(registration).to_string()];
            if let Some(institution) = a["institution"].as_str().filter(|i| !i.is_empty()) {
                detail.push(institution.to_string());
            }
            let held = a["holdings"].as_i64().unwrap_or(0);
            if held > 0 {
                detail.push(plural(held, "holding"));
            }
            let hand = by_hand(a);
            if hand.is_some() {
                detail.push(String::from("value recorded by hand"));
            }
            let change = a["gain_bps"].as_i64().map(|bps| (percent(bps), gain_class(bps)));
            let lead = badge_sized("chart", registration_hue(registration), 24);
            let value = fmt.format_whole(hand.unwrap_or_else(|| a["value"].as_i64().unwrap_or(0)));
            let name = a["name"].as_str().unwrap_or("");
            list.append(&invest_row(ui, &lead, name, &detail.join(" · "), &value, change, format!("How is my {name} account doing?")));
        }
    }

    // The room left this year: the CRA's figure less what went in. A share has no pace, so the
    // meters carry no tick.
    let room: Vec<&Value> = p["room"].as_array().into_iter().flatten().collect();
    if let Some(year) = room.first().and_then(|r| r["year"].as_i64()) {
        // Until about March the RRSP's room is last year's: rows of two years each say theirs.
        let one_year = room.iter().all(|r| r["year"].as_i64() == Some(year));
        let list = block(body, &if one_year { format!("Room left in {year}") } else { String::from("Room left") }, "");
        for r in room {
            let registration = r["registration"].as_str();
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.add_css_class("threads-budget");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let shown = match r["year"].as_i64() {
                Some(own) if !one_year => format!("{} · {own}", registration_name(registration)),
                _ => registration_name(registration).to_string(),
            };
            let name = label(&shown, "threads-panel-name");
            name.set_hexpand(true);
            line.append(&name);
            let over = r["over"].as_i64().unwrap_or(0);
            let contributed = r["contributed"].as_i64().unwrap_or(0);
            let total = r["room"].as_i64();
            let text = match (total, r["left"].as_i64()) {
                _ if over > 0 => format!("Over by {}", fmt.format_whole(over)),
                (Some(total), Some(left)) => format!("{} left of {}", fmt.format_whole(left), fmt.format_whole(total)),
                _ => String::from("Room not set"),
            };
            let figure = label(&text, "threads-panel-figures");
            if over > 0 {
                figure.add_css_class("money-over");
            }
            line.append(&figure);
            row.append(&line);
            match total.filter(|t| *t > 0) {
                Some(total) => {
                    let fill = if over > 0 { Tone::Over } else { Tone::Hue(registration_hue(registration)) };
                    let bar = meter_in(contributed as f64 / total as f64, None, fill, 4);
                    bar.set_tooltip_text(Some(&format!("{} put in of {}", fmt.format(contributed), fmt.format_whole(total))));
                    row.append(&bar);
                }
                None => {
                    let how = label("Add it on the Investments page, from CRA My Account.", "threads-panel-detail");
                    how.set_wrap(true);
                    row.append(&how);
                }
            }
            list.append(&row);
        }
    }

    let holdings: Vec<&Value> = p["holdings"].as_array().into_iter().flatten().collect();
    if !holdings.is_empty() {
        let shown = holdings.len().min(5);
        let aside = if holdings.len() > shown { format!("{shown} of {}", holdings.len()) } else { String::new() };
        let list = block(body, "Top holdings", &aside);
        for h in holdings.into_iter().take(shown) {
            let symbol = h["symbol"].as_str().unwrap_or("");
            let name = h["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(symbol);
            let mut detail = vec![h["account"].as_str().unwrap_or("").to_string()];
            if h["no_price"] == true {
                detail.push(String::from("no price yet"));
            } else {
                detail.push(format!("{} of the total", super::invest::share(h["weight_bps"].as_i64().unwrap_or(0))));
            }
            let change = h["gain_bps"].as_i64().map(|bps| (percent(bps), gain_class(bps)));
            let value = fmt.format_whole(h["value"].as_i64().unwrap_or(0));
            list.append(&invest_row(ui, &symbol_tile(symbol), name, &detail.join(" · "), &value, change, format!("How is {symbol} doing?")));
        }
    }

    let income = p["income_12m"].as_i64().unwrap_or(0);
    if income > 0 {
        let list = block(body, "Income", "12 months");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("threads-entry");
        row.append(&tile("income"));
        let name = label("Dividends and interest", "threads-panel-name");
        name.set_hexpand(true);
        name.set_valign(gtk::Align::Center);
        row.append(&name);
        let figure = label(&fmt.format_signed(income), "threads-panel-figures");
        figure.add_css_class("money-in");
        figure.set_valign(gtk::Align::Center);
        row.append(&figure);
        list.append(&row);
    }

    let issues: Vec<&str> = p["issues"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !issues.is_empty() {
        let list = block(body, "To check", "");
        for issue in issues.into_iter().take(3) {
            let l = label(issue, "threads-panel-detail");
            l.set_wrap(true);
            l.add_css_class("threads-issue");
            list.append(&l);
        }
    }

    let open = button("Open Investments", "quiet");
    open.set_halign(gtk::Align::Start);
    let run = go(ui, "money-invest");
    open.connect_clicked(move |_| run());
    body.append(&open);
}

/// No investment account yet: what the view will show, and the way in. Connect only for a ledger
/// kept in Canadian dollars (`currency`), the only one Wealthsimple's files go into.
fn invest_empty(ui: &Rc<Ui>, body: &gtk::Box, currency: &str) {
    let head = gtk::Box::new(gtk::Orientation::Vertical, 8);
    head.add_css_class("threads-summary");
    head.append(&caption("Investments"));
    let about = label("See your TFSA, RRSP and FHSA together: what they're worth, what they've earned, and the room left this year.", "money-muted");
    about.set_wrap(true);
    head.append(&about);
    if currency == "CAD" {
        let connect = button("Connect Wealthsimple", "primary");
        connect.set_halign(gtk::Align::Start);
        connect.set_margin_top(4);
        let weak = Rc::downgrade(ui);
        connect.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ask(&ui, CONNECT);
            }
        });
        head.append(&connect);
        let how = label("A thread walks you through downloading your Wealthsimple files; Tally reads them on this PC.", "threads-panel-detail");
        how.set_wrap(true);
        head.append(&how);
    } else {
        let how = label("Wealthsimple's files need a ledger kept in Canadian dollars. Add your investment accounts by hand on the Investments page.", "threads-panel-detail");
        how.set_wrap(true);
        head.append(&how);
    }
    let open = button("Open Investments", "quiet");
    open.set_halign(gtk::Align::Start);
    let run = go(ui, "money-invest");
    open.connect_clicked(move |_| run());
    head.append(&open);
    body.append(&head);
}

#[cfg(test)]
mod tests {
    use super::{avex_suggestions, kept_draft, month_history, pending_split, suggestions, week_line, PENDING_KEEP};
    use serde_json::{json, Value};

    #[test]
    fn a_draft_goes_with_its_deleted_thread() {
        let listed = [3, 5, 8].into_iter().collect();
        assert!(kept_draft(None, &listed), "a new thread's draft has no thread to lose");
        assert!(kept_draft(Some(5), &listed));
        assert!(!kept_draft(Some(4), &listed), "thread 4 was deleted unheard");
        // A thread made after the list was read is not in it yet, and keeps its draft.
        assert!(kept_draft(Some(9), &listed));
    }

    /// `gym.summary` for a copy exported on `day`, read on Thu 15 Oct 2026 (this week from Mon 12).
    fn avex(day: &str) -> Value {
        json!({
            "imported": {"exported_at": format!("{day} 21:14"), "days_per_week": 4},
            "unit": "lb",
            "this_week": {"start": "2026-10-12", "sessions": 0, "volume": 0.0},
            "weeks": [
                {"start": "2026-09-28", "sessions": 4, "volume": 51200.0},
                {"start": "2026-10-05", "sessions": 3, "volume": 46330.4},
                {"start": "2026-10-12", "sessions": 0, "volume": 0.0},
            ],
            "lifts": [{"name": "Bench Press"}],
            "recent": [{"title": "Freestyle"}, {"title": "Legs A"}],
        })
    }

    #[test]
    fn a_stale_avex_copy_reads_its_own_week_not_an_empty_one() {
        let line = week_line(&avex("2026-10-08"), "2026-10-15");
        assert!(line.contains("7 days ago"), "{line}");
        assert!(line.ends_with("That week: 3 of 4 workouts, 46,330 lb lifted."), "{line}");
        assert!(!line.contains("no workouts"), "{line}");
        let asks: Vec<String> = avex_suggestions(Some(&avex("2026-10-08"))).into_iter().map(|(a, _)| a).collect();
        assert_eq!(asks, ["How did my last week of training go?", "Which lifts have stalled?", "How is my Bench Press trending?", "Plan my next Legs A"]);
    }

    #[test]
    fn a_fresh_avex_copy_reads_this_week() {
        let mut s = avex("2026-10-15");
        s["this_week"] = json!({"start": "2026-10-12", "sessions": 2, "volume": 18450.0});
        assert_eq!(week_line(&s, "2026-10-15"), "This week so far: 2 of 4 workouts, 18,450 lb lifted.");
        s["this_week"]["sessions"] = json!(0);
        assert_eq!(week_line(&s, "2026-10-15"), "This week so far: no workouts yet.");
        assert_eq!(avex_suggestions(Some(&s))[0].0, "How is my week going?");
        // Nothing imported: one way in, no questions about lifts that are not there.
        let none = json!({"imported": null});
        assert!(week_line(&none, "2026-10-15").starts_with("Nothing from Avex yet"));
        assert_eq!(avex_suggestions(Some(&none)).len(), 1);
        // A failed read still offers four.
        assert_eq!(avex_suggestions(None).len(), 4);
    }

    #[test]
    fn a_budgets_history_leaves_out_months_before_the_ledger() {
        // The ledger began two months ago: rent averages over those two, not over three.
        let line = month_history(&[0, 135_000, 135_000], &[0, 200_000, 250_000]);
        assert!(line.starts_with("Last month ") && line.contains(" · 2-month average "), "{line}");
        assert!(month_history(&[100, 200, 300], &[1, 1, 1]).contains("3-month average"));
        assert_eq!(month_history(&[0, 0, 0], &[5, 5, 5]), "Nothing spent here in the last 3 months");
        assert_eq!(month_history(&[0, 0, 0], &[0, 0, 5]), "Nothing spent here last month");
        // A read that failed says nothing.
        assert_eq!(month_history(&[], &[]), "");
        assert_eq!(month_history(&[1, 2, 3], &[]), "");
    }

    #[test]
    fn tally_starters_promise_only_what_the_agent_does() {
        let asks: Vec<String> = suggestions(None, None).into_iter().map(|(a, _)| a).collect();
        assert_eq!(asks.len(), 4);
        assert!(asks.iter().all(|a| !a.contains("no category")), "{asks:?}");
    }

    /// Labels a reply streamed in `pieces` ends up in, split as [`super::show_pending`] splits it.
    fn stream(pieces: &[&str]) -> Vec<String> {
        let (mut text, mut kept, mut labels) = (String::new(), 0, vec![String::new()]);
        for piece in pieces {
            text.push_str(piece);
            if let Some(at) = pending_split(&text[kept..]) {
                *labels.last_mut().unwrap() = text[kept..kept + at].to_string();
                labels.push(String::new());
                kept += at + 1;
            }
            *labels.last_mut().unwrap() = text[kept..].to_string();
        }
        labels
    }

    #[test]
    fn a_streamed_reply_splits_only_at_line_breaks_and_loses_nothing() {
        let paragraph = "A sentence that goes on for a while, more or less a line. ".repeat(3);
        let reply = format!("## Heading\n\n{paragraph}\n\n- one\n- two\n\n{paragraph}\nA line\nand another\n\n{paragraph}");
        let pieces: Vec<String> = reply.chars().collect::<Vec<_>>().chunks(5).map(|c| c.iter().collect()).collect();
        let labels = stream(&pieces.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(labels.len() > 2, "a long reply moves into several labels: {labels:?}");
        // Each boundary stands for one line break: joined with one, the labels are the reply.
        assert_eq!(labels.join("\n"), reply);
        assert!(labels.iter().all(|l| !l.is_empty()));
        // Only the last label is still being written; the others were cut at a break.
        assert!(labels[..labels.len() - 1].iter().all(|l| l.len() <= PENDING_KEEP + 5 + paragraph.len()));
    }

    #[test]
    fn a_short_or_unbroken_reply_stays_in_one_label() {
        assert_eq!(pending_split("short\n\nreply"), None);
        assert_eq!(pending_split(&"x".repeat(PENDING_KEEP * 4)), None);
        // A break at the very end waits for the text after it.
        let ends_broken = format!("{}\n", "x".repeat(PENDING_KEEP + 1));
        assert_eq!(pending_split(&ends_broken), None);
        let broken = format!("{}\nmore", "x".repeat(PENDING_KEEP + 1));
        assert_eq!(pending_split(&broken), Some(PENDING_KEEP + 1));
    }
}
