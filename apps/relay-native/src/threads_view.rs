//! The Threads page: the thread list in the sidebar, the conversation with its message box, and
//! the panel beside it, Tally's, Arbiter's or Avex's (docs/THREADS.md, docs/ARBITER.md, docs/GYM.md).
//!
//! Threads are the engine's (`thread.*`); this page draws them and follows `thread.changed`,
//! `thread.message` and `thread.delta`. The page is one stack page, `threads`, whose middle shows
//! either a new thread (a greeting and the message box) or the open one.
//!
//! The page never holds the window open: its scrollers keep their content's width to themselves
//! (`PolicyType::External`), and width probes size the conversation (`fit`) and put the panel away
//! on a narrow page (`apply_panel`).
use super::pages::{badge, badge_sized, human_date, meter_in, tile, tone, tone_class, Tone};
use crate::app::{button, clear, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
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
    ("Which entries have no category?", "It suggests one for each, you approve"),
];

/// The models a thread may run on: (id, caption). An empty id is Claude's own default.
const MODELS: [(&str, &str); 5] = [
    ("", "Default model"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
];

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

/// The Tally panel's views: (key, caption).
const PANEL_TABS: [(&str, &str); 4] = [("overview", "Overview"), ("entries", "Entries"), ("budgets", "Budgets"), ("invest", "Invest")];

/// The panel's sources, as the message box's chips name them: (key, caption, what it means).
/// Choosing one shows its panel; the agent reaches all of them either way.
const SOURCES: [(&str, &str, &str); 3] = [
    ("tally", "Tally", "Show Tally beside the thread: the agent reads and changes your budget and investments"),
    ("arbiter", "Arbiter", "Show Arbiter beside the thread: the agent reads, backtests and proposes; orders follow each strategy's mode"),
    ("avex", "Avex", "Show Avex beside the thread: the agent reads your training as last exported from the phone"),
];

/// The widest the conversation and its message box grow (the mockup's column).
const COLUMN_MAX: i32 = 720;
/// Under this page width the Tally panel steps aside, unless the person opened it.
const PANEL_FROM: i32 = 1000;
/// What the working row says before the agent has started.
const STARTING: &str = "Starting Claude…";
/// What a new thread asks Claude when the person picks "Connect Wealthsimple".
const CONNECT: &str = "Help me connect my Wealthsimple accounts";

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
    input: gtk::TextView,
    hint: gtk::Label,
    send: gtk::Button,
    source: gtk::Box,
    limits: gtk::Label,
    model: Picker,
    effort: Picker,
    scroll: gtk::ScrolledWindow,
    jump: gtk::Button,
    column: gtk::Box,
    /// The reply being written: its label, shown while `thread.delta` text arrives.
    pending: gtk::Label,
    working: gtk::Box,
    working_text: gtk::Label,
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
    /// The source switch above the panel's tabs, and the message box's chips.
    sources: Vec<(&'static str, gtk::Button)>,
    chips: Vec<(&'static str, gtk::Button)>,
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

#[derive(Default)]
struct State {
    view: RefCell<Option<Rc<View>>>,
    list: RefCell<Option<gtk::Box>>,
    current: Cell<Option<i64>>,
    working: Cell<bool>,
    /// The thread's agent is running, so a reply starts without a resume.
    live: Cell<bool>,
    /// Stop was pressed: the reply's end keeps what it wrote and says so.
    stopping: Cell<bool>,
    /// The newest message drawn, so an event already drawn by a read is not drawn twice.
    last: Cell<i64>,
    /// The local day of the last message drawn, for the separators between days.
    last_day: RefCell<String>,
    pending: RefCell<String>,
    /// Tool calls drawn in this thread, by id.
    tools: RefCell<HashMap<String, Tool>>,
    turn: RefCell<Option<Turn>>,
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
    narrow_page: Cell<bool>,
    listing: Cell<bool>,
    /// The list changed while it was being read: read it again.
    list_dirty: Cell<bool>,
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
    }
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
    conversation.append(&middle);

    let pending = label("", "threads-reply");
    pending.set_wrap(true);
    pending.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    pending.set_selectable(false);
    let working = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    working.add_css_class("threads-working");
    let spinner = gtk::Spinner::new();
    spinner.start();
    working.append(&spinner);
    let working_text = label("Working…", "threads-working-text");
    working.append(&working_text);

    // The message box: what you write, where it goes, and send or stop.
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
    composer.append(&field);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // Tally, Arbiter or Avex: which panel shows beside the thread.
    let source = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let mut chips = Vec::new();
    for (key, caption, about) in SOURCES {
        let chip = gtk::Button::new();
        chip.add_css_class("threads-chip");
        chip.add_css_class("threads-source-chip");
        chip.set_widget_name(&format!("threads-chip-{key}"));
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("threads-dot");
        dot.set_valign(gtk::Align::Center);
        inner.append(&dot);
        inner.append(&label(caption, ""));
        chip.set_child(Some(&inner));
        chip.set_tooltip_text(Some(about));
        source.append(&chip);
        chips.push((key, chip));
    }
    row.append(&source);
    let model = picker("threads-model", "The model this thread's agent runs on", &MODELS, |value| choose(Some(value), None));
    row.append(&model.key);
    let effort = picker("threads-effort", "How hard the agent thinks before answering", &EFFORTS, |value| choose(None, Some(value)));
    row.append(&effort.key);
    // What the agent may do, said plainly: words, not a key.
    let limits = label("Edits with Undo", "threads-limits");
    limits.set_valign(gtk::Align::Center);
    limits.set_tooltip_text(Some("It adds and changes entries and investment activity, each with an Undo. Deleting, budgets and accounts stay yours."));
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
    settings_key.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            ui.navigate("settings");
        }
    });
    notice.append(&settings_key);
    notice.set_visible(false);
    let message_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    message_box.append(&notice);
    message_box.append(&composer);

    // An open thread's message box sits under the conversation, as wide as its column; its
    // scroller keeps the chips' width from holding the window open.
    let dock_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dock_slot.add_css_class("threads-dock");
    let dock = gtk::ScrolledWindow::builder().child(&dock_slot).propagate_natural_height(true).build();
    dock.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
    dock.add_css_class("threads-dock-scroll");
    conversation.append(&dock);
    body.set_start_child(Some(&conversation));
    body.set_resize_start_child(true);
    body.set_shrink_start_child(false);

    // The panel: 340 pixels as it opens, 300 at the least. Its source, Tally, Arbiter or Avex,
    // above that source's tabs.
    let side = gtk::Box::new(gtk::Orientation::Vertical, 18);
    side.add_css_class("threads-panel");
    side.set_size_request(300, -1);
    let source_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    source_row.add_css_class("threads-sources");
    let mut sources = Vec::new();
    for (key, caption, _) in SOURCES {
        let tab = button(caption, "quiet");
        tab.set_widget_name(&format!("threads-source-{key}"));
        source_row.append(&tab);
        sources.push((key, tab));
    }
    side.append(&source_row);
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("view-tabs");
    // Each tab as wide as its caption needs, the spare width shared: four fit in 300.
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
    // Place the split once the page has a width: the panel opens at 340.
    body.add_tick_callback(|split, _| {
        if split.width() <= 0 {
            return glib::ControlFlow::Continue;
        }
        split.set_position((split.width() - 340).max(0));
        glib::ControlFlow::Break
    });
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
        title, title_key, rename, middle, empty, empty_slot, empty_line, suggestions, dock, dock_slot, message_box,
        notice, notice_text, input, hint, send: send_key, source, limits, model, effort, scroll, jump, column, pending,
        working, working_text, panel, panel_key, panel_body, panel_tabs,
        tally_tabs: tabs, arbiter_tabs, arbiter_panel_tabs, avex_tabs, avex_panel_tabs, sources, chips,
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

/// The panel's source switch and the message box's chips, and Arbiter's and Avex's tabs.
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
    for (key, tab) in v.sources.iter().chain(&v.chips) {
        let key = *key;
        tab.connect_clicked(move |_| set_source(key));
    }
}

/// Show `source`'s panel, open, and its chip lit.
fn set_source(source: &'static str) {
    let Some(v) = view() else { return };
    STATE.with(|s| s.source.set(source));
    show_source(&v);
    if !v.panel_key.is_active() {
        v.panel_key.set_active(true);
    }
    if let Some(ui) = the_ui() {
        refresh_panel(&ui);
    }
}

/// The source showing, on the switch, the tabs and the chips, and what the agent may do alone.
fn show_source(v: &View) {
    let source = STATE.with(|s| s.source.get());
    for (key, tab) in v.sources.iter().chain(&v.chips) {
        if *key == source {
            tab.add_css_class("selected");
        } else {
            tab.remove_css_class("selected");
        }
    }
    v.tally_tabs.set_visible(source == "tally");
    v.arbiter_tabs.set_visible(source == "arbiter");
    v.avex_tabs.set_visible(source == "avex");
    if source == "arbiter" {
        v.limits.set_text("Proposes, you approve");
        v.limits.set_tooltip_text(Some("It reads, backtests and proposes. Orders follow each strategy's mode, always within its limits, and only you change those."));
    } else if source == "avex" {
        v.limits.set_text("Reads only");
        v.limits.set_tooltip_text(Some("It reads your training as last exported from Avex. It cannot change anything in Avex."));
    } else {
        v.limits.set_text("Edits with Undo");
        v.limits.set_tooltip_text(Some("It adds and changes entries and investment activity, each with an Undo. Deleting, budgets and accounts stay yours."));
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
            if let Some(v) = view() {
                clear(&v.panel_body);
            }
            if let Some(ui) = the_ui() {
                refresh_panel(&ui);
            }
        });
    }
    v.title_key.connect_notify_local(Some("active"), |key, _| {
        if key.is_active() {
            if let Some(v) = view() {
                v.rename.set_text(&v.title.text());
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
/// placed by margins, not requests, so their content never widens them. The chips that matter
/// least step aside first, and the questions stack under 600.
fn fit(v: &View, width: i32) {
    let gutter = if width < 640 { 16 } else { 28 };
    let inner = (width - 2 * gutter).clamp(240, COLUMN_MAX);
    let start = ((width - inner) / 2).max(0);
    let end = (width - inner - start).max(0);
    for part in [v.column.upcast_ref::<gtk::Widget>(), v.empty.upcast_ref(), v.dock_slot.upcast_ref()] {
        part.set_margin_start(start);
        part.set_margin_end(end);
    }
    v.limits.set_visible(width >= 640);
    v.source.set_visible(width >= 520);
    v.effort.key.set_visible(width >= 420);
    let one = width < 600;
    if STATE.with(|s| s.one_column.replace(one)) != one {
        fill_suggestions(v);
    }
}

/// Show or hide the Tally panel: the person's choice, else whether the page is wide enough.
fn apply_panel(v: &View) {
    let open = STATE.with(|s| s.panel_pinned.get().unwrap_or(!s.narrow_page.get()));
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
    let hint = if empty { "What do you want to know?" } else { "Ask about your money, or tell it what to change" };
    v.hint.set_text(hint);
    v.input.update_property(&[gtk::accessible::Property::Placeholder(hint)]);
}

/// What the send key does now: stop a reply, or send what is written when there is something to
/// send and someone to answer it. The placeholder shows while the box is empty.
fn update_send(v: &View) {
    let buffer = v.input.buffer();
    v.hint.set_visible(buffer.char_count() == 0);
    let written = !buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().is_empty();
    let working = STATE.with(|s| s.working.get());
    let missing = STATE.with(|s| s.claude_missing.get());
    let reachable = the_ui().is_none_or(|ui| ui.is_connected()) && !missing;
    v.send.set_sensitive(working || (written && reachable));
    v.suggestions.set_sensitive(!missing);
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
    !model.is_empty() && STATE.with(|s| s.local_models.borrow().iter().any(|(id, _)| id == model))
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
}

/// A model or an effort picked: the thread showing takes it (`thread.set`), and new threads start
/// with it.
fn choose(model: Option<&str>, effort: Option<&str>) {
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
    if picked.is_some() {
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
    glib::spawn_future_local(async move {
        match ui.call("thread.set", json!({"id": id, "model": model, "effort": effort})).await {
            Ok(thread) => {
                if let Some(v) = view() {
                    header(&v, &thread);
                }
            }
            Err(e) => ui.show_error(&e.to_string()),
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
}

/// The effort picker waits for a reply like the model picker, and a local model has no efforts.
fn effort_sensitive(v: &View) {
    let local = is_local(&STATE.with(|s| s.shown_model.borrow().clone()));
    v.effort.key.set_sensitive(!STATE.with(|s| s.working.get()) && !local);
    v.effort.key.set_tooltip_text(Some(if local { "A model on this PC has no effort setting" } else { "How hard the agent thinks before answering" }));
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

/// Forget the thread drawn: `id` shows next, or a new thread when `None`.
fn reset(id: Option<i64>) {
    STATE.with(|s| {
        s.current.set(id);
        s.last.set(0);
        s.last_day.borrow_mut().clear();
        s.pending.borrow_mut().clear();
        s.tools.borrow_mut().clear();
        *s.turn.borrow_mut() = None;
        *s.group.borrow_mut() = None;
        s.added.borrow_mut().clear();
        s.stopping.set(false);
        if let Some(id) = id {
            s.unread.borrow_mut().remove(&id);
        }
    });
}

/// Start a new thread: the page shows the greeting until the first message is sent.
pub fn new_thread(ui: &Rc<Ui>) {
    reset(None);
    if let Some(v) = view() {
        show_empty(&v);
        v.empty_line.set_text("");
        v.input.grab_focus();
    }
    if *ui.page.borrow() != PAGE {
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

/// A new thread's words from the ledger: the month in a line, and the questions to start with.
async fn greet(ui: &Rc<Ui>) {
    let (summary, invest) = tokio::join!(ui.call("money.summary", json!({})), ui.call("money.invest.summary", json!({})));
    let Some(v) = view().filter(|_| current().is_none()) else { return };
    if let Ok(s) = &summary {
        v.empty_line.set_text(&month_line(s));
    }
    let items = suggestions(summary.as_ref().ok(), invest.as_ref().ok());
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
        format!("{when} so far: {spent} spent, {} left. Ask anything, or tell it what to change.", fmt.format_whole(left))
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
                let name = b["name"].as_str().unwrap_or("this bill");
                let when = match b["days_until"].as_i64().unwrap_or(0) {
                    0 => String::from("today"),
                    1 => String::from("tomorrow"),
                    _ => format!("on {}", human_date(b["next_date"].as_str().unwrap_or(""))),
                };
                let about = format!("{} · {}", fmt.format(b["amount"].as_i64().unwrap_or(0)), b["due_line"].as_str().unwrap_or(""));
                out.push((format!("Can I cover {name} {when}?"), about));
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
            "Claude Code isn't signed in on this PC. Sign in, then refresh it in Settings."
        } else {
            ""
        };
        v.notice_text.set_text(why);
        v.notice.set_visible(!why.is_empty());
        update_send(&v);
    });
}

/// Open thread `id`: read it whole and draw it.
pub fn open(ui: &Rc<Ui>, id: i64) {
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
            Ok(read) => draw_thread(&read),
            Err(e) => failed_open(&ui, id, &super::pages::unavailable(&e)),
        }
    });
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
            open(&ui, id);
        }
    });
    v.column.append(&again);
}

fn draw_thread(read: &Value) {
    let Some(v) = view() else { return };
    clear(&v.column);
    STATE.with(|s| {
        s.last.set(0);
        s.last_day.borrow_mut().clear();
        s.tools.borrow_mut().clear();
        *s.turn.borrow_mut() = None;
        *s.group.borrow_mut() = None;
    });
    header(&v, &read["thread"]);
    for message in read["messages"].as_array().into_iter().flatten() {
        draw_message(&v, message);
    }
    if !STATE.with(|s| s.working.get()) {
        settle_tools();
    }
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
    // `thread.set` closes a running agent: the choice waits for the reply to end.
    v.model.key.set_sensitive(!working);
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
    } else {
        if let Some(parent) = v.working.parent().and_downcast::<gtk::Box>() {
            parent.remove(&v.working);
        }
        if let Some(parent) = v.pending.parent().and_downcast::<gtk::Box>() {
            parent.remove(&v.pending);
        }
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
            turn.body.append(&label("Stopped", "threads-note"));
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
    let copy = crate::app::icon_button("copy", "Copy this reply");
    copy.add_css_class("threads-copy");
    copy.set_halign(gtk::Align::Start);
    copy.set_visible(false);
    let words = text.clone();
    copy.connect_clicked(move |key| {
        key.clipboard().set_text(&words.borrow());
        key.set_child(Some(&crate::icons::image("check", 16)));
        key.set_tooltip_text(Some("Copied"));
        let key = key.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1400), move || {
            if let Some(key) = key.upgrade() {
                key.set_child(Some(&crate::icons::image("copy", 16)));
                key.set_tooltip_text(Some("Copy this reply"));
            }
        });
    });
    wrap.append(&copy);
    v.column.append(&wrap);
    let turn = Turn { body, text, copy };
    STATE.with(|s| *s.turn.borrow_mut() = Some(turn.clone()));
    turn
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
            STATE.with(|s| {
                *s.turn.borrow_mut() = None;
                *s.group.borrow_mut() = None;
            });
            let bubble = label(body["text"].as_str().unwrap_or(""), "threads-user");
            bubble.set_wrap(true);
            bubble.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            bubble.set_selectable(true);
            // Selectable for the mouse, not a Tab stop (set after `selectable`, which sets it).
            bubble.set_focusable(false);
            bubble.set_halign(gtk::Align::End);
            bubble.set_max_width_chars(60);
            v.column.append(&bubble);
        }
        "assistant" => {
            if let Some(parent) = v.pending.parent().and_downcast::<gtk::Box>() {
                parent.remove(&v.pending);
            }
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
            turn(v).append(&row);
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

/// An agent's text, its Markdown drawn as blocks.
fn draw_reply(parent: &gtk::Box, text: &str) {
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
                code.upcast()
            }
            Block::Table(rows) => table(&rows).upcast(),
            Block::Rule => {
                let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
                rule.add_css_class("threads-rule");
                rule.upcast()
            }
        };
        parent.append(&widget);
    }
}

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
    let scroll = gtk::ScrolledWindow::builder().child(&grid).propagate_natural_height(true).build();
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
        Some("bus.schema") => "Checking how a tool works…",
        _ => "Working…",
    }
}

/// A tool line's state mark: turning while it runs, then what it did, a cross when it was refused,
/// or a dash when the reply stopped before it answered.
fn show_step(mark: &gtk::Box, step: Step, writes: bool) {
    clear(mark);
    match step {
        Step::Running => {
            let spinner = gtk::Spinner::new();
            spinner.set_size_request(13, 13);
            spinner.start();
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
    let writes = tool_writes(&name) || super::arbiter::tool_writes(&op);
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
    let caption = label(&super::arbiter::tool_caption(&op).map_or_else(|| tool_caption(&name), str::to_string), "threads-tool-text");
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
        let tool = Tool { slot, mark, name, step: Step::Running, group: group.clone() };
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

/// The turn ended: a tool line still turning got no answer (the reply was stopped). It stops.
fn settle_tools() {
    let open: Vec<(gtk::Box, Option<Group>)> = STATE.with(|s| {
        s.tools
            .borrow_mut()
            .values_mut()
            .filter(|t| t.step == Step::Running)
            .map(|t| {
                t.step = Step::Cut;
                (t.mark.clone(), t.group.clone())
            })
            .collect()
    });
    for (mark, group) in open {
        show_step(&mark, Step::Cut, false);
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
            (t.slot.clone(), t.mark.clone(), t.name.clone(), t.group.clone())
        })
    });
    let Some((slot, mark, name, group)) = found else { return };
    let op = tool_op(&name).unwrap_or_default();
    let writes = tool_writes(&name) || super::arbiter::tool_writes(&op);
    show_step(&mark, step, writes);
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
    let mut detail = Vec::new();
    if let Some(category) = tx["category"].as_str().filter(|_| !transfer) {
        detail.push(category.to_string());
    }
    if let Some(date) = tx["date"].as_str() {
        detail.push(human_date(date));
    }
    if let Some(account) = tx["account"].as_str().filter(|_| !transfer) {
        detail.push(account.to_string());
    }
    let about = label(&detail.join(" · "), "threads-card-detail");
    about.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&about);
    card.append(&words);
    let figure = label(&amount_text(tx), "money-amount");
    figure.set_valign(gtk::Align::Center);
    if kind == "income" {
        figure.add_css_class("money-in");
    }
    card.append(&figure);
    if let Some(id) = tx["id"].as_i64().filter(|_| op == "money.tx.add") {
        STATE.with(|s| s.added.borrow_mut().insert(id));
        card.append(&undoable(&card, &title, &what, ("money.tx.delete", "money.tx.restore"), id));
    }
    card
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
    // Money in, money out, or neither (an exchange, a split, a reinvested dividend).
    let (icon, sign) = match kind {
        "DEPOSIT" | "TRANSFER_IN" | "DIVIDEND" | "INTEREST" | "CREDIT" | "RETURN_OF_CAPITAL" | "SELL" => ("income", "+"),
        "WITHDRAWAL" | "TRANSFER_OUT" | "FEE" | "TAX" | "BUY" => ("spend", "−"),
        _ => ("repeat", ""),
    };
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
    let amount = fmt.format(minor(&a["amount"], fmt.fraction_digits).unwrap_or(0));
    let to_fmt = a["to_currency"].as_str().map(|to_currency| MoneyFormatter::new(to_currency, Locale::from_env()));
    let shown = match (kind, to_fmt.and_then(|f| Some((minor(&a["to_amount"], f.fraction_digits)?, f)))) {
        ("FX", Some((to, to_fmt))) => format!("{amount} → {}", to_fmt.format(to)),
        _ => format!("{sign}{amount}"),
    };
    let figure = label(&shown, "money-amount");
    figure.set_valign(gtk::Align::Center);
    if matches!(kind, "DIVIDEND" | "INTEREST") {
        figure.add_css_class("money-in");
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
        about.push(format!("As of {}", human_date(day.get(..10).unwrap_or(day))));
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
    if STATE.with(|s| s.working.get()) || !v.send.is_sensitive() {
        return;
    }
    let buffer = v.input.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
    if text.is_empty() {
        return;
    }
    buffer.set_text("");
    send(ui, &text);
}

/// Send `text`: in the thread showing, or as the first message of a new one.
fn send(ui: &Rc<Ui>, text: &str) {
    let ui = ui.clone();
    let text = text.to_string();
    glib::spawn_future_local(async move {
        let result = match current() {
            Some(id) => ui.call("thread.send", json!({"id": id, "text": text})).await.map(|_| id),
            None => {
                let (model, effort) = STATE.with(|s| s.choice.borrow().clone());
                let payload = json!({"text": text, "model": (!model.is_empty()).then_some(model), "effort": (!effort.is_empty()).then_some(effort)});
                ui.call("thread.create", payload).await.map(|t| t["id"].as_i64().unwrap_or(0))
            }
        };
        match result {
            Ok(id) if current() != Some(id) => open(&ui, id),
            Ok(_) => {}
            Err(e) => {
                if let Some(v) = view() {
                    v.input.buffer().set_text(&text);
                }
                ui.show_error(&e.to_string());
            }
        }
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
            let text = payload["text"].as_str().unwrap_or("");
            let pending = STATE.with(|s| {
                let mut p = s.pending.borrow_mut();
                p.push_str(text);
                p.clone()
            });
            if v.pending.parent().is_none() {
                turn(&v).append(&v.pending);
            }
            v.pending.set_text(&pending);
            if v.working_text.text() == STARTING {
                v.working_text.set_text("Working…");
            }
            stick_to_bottom(&v, false);
        }
        "thread.message" if thread == current() => {
            draw_message(&v, &payload["message"]);
            stick_to_bottom(&v, false);
        }
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
                    new_thread(ui);
                } else {
                    header(&v, &payload["thread"]);
                    if payload["thread"]["working"] == true {
                        stick_to_bottom(&v, false);
                    }
                }
            }
            refresh_list(ui);
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
    let scroll = crate::app::scrolled(&list);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll
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
                Ok(listed) => draw_list(&listed["threads"]),
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

/// Which heading a thread used last on `day` sits under.
fn age(updated_at: &str) -> &'static str {
    let (Ok(then), Ok(now)) = (glib::DateTime::from_iso8601(updated_at, None), glib::DateTime::now_local()) else { return "Earlier" };
    let Ok(then) = then.to_local() else { return "Earlier" };
    let day = |d: &glib::DateTime| glib::DateTime::from_local(d.year(), d.month(), d.day_of_month(), 0, 0, 0.0).ok();
    let (Some(then), Some(today)) = (day(&then), day(&now)) else { return "Earlier" };
    match today.difference(&then).as_days() {
        ..=0 => "Today",
        1 => "Yesterday",
        2..=6 => "This week",
        _ => "Earlier",
    }
}

fn draw_list(threads: &Value) {
    let Some(list) = STATE.with(|s| s.list.borrow().clone()) else { return };
    clear(&list);
    let mut heading = "";
    for t in threads.as_array().into_iter().flatten() {
        let group = age(t["updated_at"].as_str().unwrap_or(""));
        if group != heading {
            heading = group;
            list.append(&label(group, "section-label"));
        }
        let id = t["id"].as_i64().unwrap_or(0);
        let key = button("", "nav");
        key.add_css_class("threads-row");
        key.set_widget_name(&format!("thread-{id}"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = label(t["title"].as_str().unwrap_or("Thread"), "nav-label");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        row.append(&title);
        // Green while the agent answers; an ink dot for a reply not yet read. A finished reply is
        // news, not an alarm.
        if t["working"] == true {
            STATE.with(|s| s.was_working.borrow_mut().insert(id));
            let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            lamp.add_css_class("threads-lamp");
            lamp.set_valign(gtk::Align::Center);
            row.append(&lamp);
            key.update_property(&[gtk::accessible::Property::Description("Claude is answering")]);
        } else if STATE.with(|s| s.unread.borrow().contains(&id)) {
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("threads-unread");
            dot.set_valign(gtk::Align::Center);
            row.append(&dot);
            key.update_property(&[gtk::accessible::Property::Description("New reply")]);
        }
        key.set_child(Some(&row));
        if let Some(preview) = t["preview"].as_str() {
            key.set_tooltip_text(Some(preview));
        }
        key.connect_clicked(move |_| {
            if let Some(ui) = the_ui() {
                open(&ui, id);
            }
        });
        list.append(&key);
    }
    if threads.as_array().is_none_or(Vec::is_empty) {
        let note = label("No threads yet. Ctrl N starts one.", "threads-list-note");
        note.set_wrap(true);
        list.append(&note);
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

/// `arbiter.changed`: re-read the panel when it shows Arbiter.
pub fn refresh_arbiter_panel(ui: &Rc<Ui>) {
    if STATE.with(|s| s.source.get()) == "arbiter" {
        refresh_panel(ui);
    }
}

/// Re-read Arbiter's panel view; one source's read never lands on the other's panel.
fn refresh_arbiter(ui: &Rc<Ui>, v: &View) {
    let tab = STATE.with(|s| s.arbiter_tab.get());
    for (key, button) in &v.arbiter_panel_tabs {
        if *key == tab {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
    }
    let serial = STATE.with(|s| {
        let next = s.panel_serial.get().wrapping_add(1);
        s.panel_serial.set(next);
        next
    });
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
            Err(e) => {
                let l = label(&super::arbiter::said(&e), "money-muted");
                l.set_wrap(true);
                v.panel_body.append(&l);
            }
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
    for (key, button) in &v.avex_panel_tabs {
        if *key == tab {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
    }
    let serial = STATE.with(|s| {
        let next = s.panel_serial.get().wrapping_add(1);
        s.panel_serial.set(next);
        next
    });
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
            Err(e) => {
                let l = label(&super::avex::said(&e), "money-muted");
                l.set_wrap(true);
                v.panel_body.append(&l);
            }
        }
    });
}

/// Re-read the panel's view, Tally's, Arbiter's or Avex's, if it is open. Only the newest read
/// draws.
pub fn refresh_panel(ui: &Rc<Ui>) {
    let Some(v) = view() else { return };
    if !v.panel.is_visible() {
        return;
    }
    match STATE.with(|s| s.source.get()) {
        "arbiter" => return refresh_arbiter(ui, &v),
        "avex" => return refresh_avex(ui, &v),
        _ => {}
    }
    let tab = STATE.with(|s| s.panel_tab.get());
    for (key, button) in &v.panel_tabs {
        if *key == tab {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
    }
    let serial = STATE.with(|s| {
        let next = s.panel_serial.get().wrapping_add(1);
        s.panel_serial.set(next);
        next
    });
    if v.panel_body.first_child().is_none() {
        v.panel_body.append(&label("Reading the ledger…", "money-muted"));
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        // The Invest view also reads the balances: an account valued by hand is Tally's balance.
        let mut lists = None;
        let read = match tab {
            "entries" => ui.call("money.tx.list", json!({"limit": 60})).await,
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
                "budgets" => panel_budgets(&ui, &v.panel_body, &value, usize::MAX),
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
                let l = label(&why, "money-muted");
                l.set_wrap(true);
                v.panel_body.append(&l);
            }
        }
    });
}

fn caption(text: &str) -> gtk::Label {
    label(text, "threads-caption")
}

/// A titled block of the panel: its caption, an optional count on the right, and its rows.
fn block(body: &gtk::Box, name: &str, aside: &str) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.add_css_class("threads-block");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = caption(name);
    title.set_hexpand(true);
    head.append(&title);
    if !aside.is_empty() {
        head.append(&label(aside, "threads-panel-aside"));
    }
    block.append(&head);
    body.append(&block);
    block
}

/// `row` as a key: the panel's rows open what they show.
fn row_key(row: &gtk::Box, about: &str, run: impl Fn() + 'static) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("threads-row-key");
    key.set_child(Some(row));
    key.set_tooltip_text(Some(about));
    key.connect_clicked(move |_| run());
    key
}

/// A run that opens `page`.
fn go(ui: &Rc<Ui>, page: &'static str) -> impl Fn() + 'static {
    let weak = Rc::downgrade(ui);
    move || {
        if let Some(ui) = weak.upgrade() {
            ui.navigate(page);
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
        let figure = label(&format!("{} left", fmt.format_whole(left)), "threads-panel-figure");
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

    panel_budgets(ui, body, s, 5);

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
            list.append(&row);
        }
    }

    let recent: Vec<&Value> = s["recent"].as_array().into_iter().flatten().take(5).collect();
    let list = block(body, "Recent", "");
    if recent.is_empty() {
        list.append(&label("Nothing logged this period yet.", "money-muted"));
    }
    for tx in recent {
        list.append(&entry_row(ui, tx, true));
    }
}

fn panel_budgets(ui: &Rc<Ui>, body: &gtk::Box, s: &Value, most: usize) {
    let mut budgets: Vec<&Value> = s["budgets"].as_array().into_iter().flatten().filter(|b| b["budget"].as_i64().unwrap_or(0) > 0).collect();
    let pace = s["pace"]["pace_fraction"].as_f64();
    let ahead = |b: &Value| b["spent"].as_i64().unwrap_or(0) as f64 / b["budget"].as_i64().unwrap_or(1).max(1) as f64;
    budgets.sort_by(|a, b| ahead(b).total_cmp(&ahead(a)));
    let total = budgets.len();
    let aside = if total > most { format!("{most} of {total}") } else { String::new() };
    let list = block(body, "Budgets", &aside);
    if budgets.is_empty() {
        let note = label("No budgets yet. Set them in Tally's Plan.", "money-muted");
        note.set_wrap(true);
        list.append(&note);
    }
    let fmt = super::formatter();
    for b in budgets.into_iter().take(most) {
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
        let rest = label(&if left < 0 { format!("{} over", fmt.format_whole(-left)) } else { format!("{} left", fmt.format_whole(left)) }, "threads-panel-figures");
        if let Some(class) = tone_class(status) {
            rest.add_css_class(class);
        }
        line.append(&rest);
        right.append(&line);
        let bar = meter_in(spent as f64 / budget.max(1) as f64, pace, tone(status, b["color"].as_i64()), 4);
        bar.set_tooltip_text(Some(&format!("{} of {}", fmt.format(spent), fmt.format_whole(budget))));
        right.append(&bar);
        row.append(&right);
        list.append(&row_key(&row, "Open the plan", go(ui, "money-plan")));
    }
}

/// One entry in the panel: its badge, what it was over its category and day, and the amount.
/// Clicking it edits it; an entry this thread added says so.
fn entry_row(ui: &Rc<Ui>, tx: &Value, dated: bool) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("threads-entry");
    let transfer = tx["type"] == "TRANSFER";
    row.append(&badge_sized(if transfer { "repeat" } else { tx["icon"].as_str().unwrap_or("dots") }, if transfer { 10 } else { tx["color"].as_i64().unwrap_or(10) }, 24));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let category = tx["category"].as_str().unwrap_or(if transfer { "Transfer" } else { "Entry" });
    let note = tx["note"].as_str().map(str::trim).filter(|n| !n.is_empty());
    let title = label(note.unwrap_or(category), "threads-panel-name");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    let mut detail = Vec::new();
    if note.is_some() {
        detail.push(category.to_string());
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

fn panel_entries(ui: &Rc<Ui>, body: &gtk::Box, page: &Value) {
    let rows: Vec<&Value> = page["transactions"].as_array().into_iter().flatten().collect();
    let fmt = super::formatter();
    let (income, spent) = (page["income"].as_i64().unwrap_or(0), page["spent"].as_i64().unwrap_or(0));
    let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
    head.add_css_class("threads-summary");
    head.append(&label(&super::pages::period_name(&page["period"]), "threads-panel-eyebrow"));
    head.append(&label(&format!("Out {} · In {}", fmt.format_whole(spent), fmt.format_whole(income)), "threads-panel-figures"));
    body.append(&head);
    if rows.is_empty() {
        body.append(&label("No entries this month.", "money-muted"));
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
}

/// A registration's hue and name, as the Investments page draws them (`money_invest.rs`): the hue
/// is data, on badges and the allocation bar, never on words.
fn registration_hue(registration: Option<&str>) -> i64 {
    super::invest::registration_hue(registration.unwrap_or(""))
}

fn registration_name(registration: Option<&str>) -> &'static str {
    super::invest::registration_label(registration.unwrap_or(""))
}

/// "+0.7%", "−1.2%": basis points as a signed share.
fn percent(bps: i64) -> String {
    let sign = match bps.signum() {
        1 => "+",
        -1 => "−",
        _ => "",
    };
    format!("{sign}{:.1}%", bps.unsigned_abs() as f64 / 100.0)
}

/// "52%": a share in basis points, whole.
fn share(bps: i64) -> String {
    format!("{:.0}%", bps as f64 / 100.0)
}

/// Up is money in's green, down the over red; a sign says it too.
fn gain_class(n: i64) -> Option<&'static str> {
    match n.signum() {
        1 => Some("money-in"),
        -1 => Some("money-over"),
        _ => None,
    }
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

/// A row of the Invest view: its mark, a name over its detail, the value over its change. It opens
/// the Investments page.
fn invest_row(ui: &Rc<Ui>, lead: &gtk::Box, title: &str, detail: &str, value: &str, change: Option<(String, Option<&'static str>)>) -> gtk::Button {
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
    row_key(&row, "Open Investments", go(ui, "money-invest"))
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
    let as_of = p["as_of"].as_str().filter(|d| !d.is_empty()).map(|d| format!("As of {}", human_date(d.get(..10).unwrap_or(d))));
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
            (registration_hue(registration), bps as f64 / 10_000.0, format!("{} {}", registration_name(registration), share(bps)))
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
            list.append(&invest_row(ui, &lead, a["name"].as_str().unwrap_or(""), &detail.join(" · "), &value, change));
        }
    }

    // The room left this year: the CRA's figure less what went in. A share has no pace, so the
    // meters carry no tick.
    let room: Vec<&Value> = p["room"].as_array().into_iter().flatten().collect();
    if let Some(year) = room.first().and_then(|r| r["year"].as_i64()) {
        let list = block(body, &format!("Room left in {year}"), "");
        for r in room {
            let registration = r["registration"].as_str();
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.add_css_class("threads-budget");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let name = label(registration_name(registration), "threads-panel-name");
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
                detail.push(format!("{} of the total", share(h["weight_bps"].as_i64().unwrap_or(0))));
            }
            let change = h["gain_bps"].as_i64().map(|bps| (percent(bps), gain_class(bps)));
            let value = fmt.format_whole(h["value"].as_i64().unwrap_or(0));
            list.append(&invest_row(ui, &symbol_tile(symbol), name, &detail.join(" · "), &value, change));
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
