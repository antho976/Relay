//! The Threads space, beside the dev workspace: conversations with an agent about the person's
//! own data, and Tally's ledger beside them (docs/THREADS.md). And the Aide space, the third:
//! the person's own bots, each a chat, under a Chief of Staff (`aide_pages.rs`, docs/AIDE.md).
//! And Clips, the fourth: a stream's VODs in, Shorts out (`clips_pages.rs`, docs/CLIPS.md).
//!
//! The space is a mode of the one window, in the dev space's own look: only the sidebar's keys
//! and the middle change. Its pages live in the main content stack: `threads` (`threads_view.rs`)
//! and Tally's `money-*`. The ledger and the threads are the engine's (`money.*`, `thread.*`);
//! this module only reads and writes them through the bus. Its names still say money: the space
//! began as Tally's.
use crate::app::{button, clear, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_money::money::{Locale, MoneyFormatter};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

#[path = "money_entry.rs"]
mod entry;
#[path = "money_invest.rs"]
pub(crate) mod invest;
#[path = "money_pages.rs"]
mod pages;
#[path = "threads_view.rs"]
pub(crate) mod threads;
#[path = "threads_chart.rs"]
mod chart;
#[path = "threads_ask.rs"]
mod ask;
#[path = "threads_pictures.rs"]
pub(crate) mod pictures;
#[path = "arbiter_pages.rs"]
mod arbiter;
#[path = "avex_pages.rs"]
mod avex;
#[path = "aide_pages.rs"]
mod aide;
#[path = "clips_pages.rs"]
mod clips;
#[path = "space_kit.rs"]
mod kit;

/// Tally's pages, in tab order: (stack name, caption, glyph).
pub const PAGES: [(&str, &str, &str); 5] = [
    ("money-home", "Overview", "home"),
    ("money-transactions", "Entries", "list"),
    ("money-plan", "Plan", "plan"),
    ("money-invest", "Investments", "chart"),
    ("money-data", "Data", "data"),
];

/// The window's four spaces, as the title bar's switch shows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Space {
    #[default]
    Dev,
    Threads,
    Aide,
    Clips,
}

impl Space {
    pub const ALL: [Space; 4] = [Space::Dev, Space::Threads, Space::Aide, Space::Clips];

    pub fn caption(self) -> &'static str {
        match self {
            Space::Dev => "Dev",
            Space::Threads => "Threads",
            Space::Aide => "Aide",
            Space::Clips => "Clips",
        }
    }

    /// Its value in `native.space`; Threads keeps the name it had as Money's.
    fn setting(self) -> &'static str {
        match self {
            Space::Dev => "dev",
            Space::Threads => "money",
            Space::Aide => "aide",
            Space::Clips => "clips",
        }
    }

    fn from_setting(value: Option<&str>) -> Space {
        Space::ALL.into_iter().find(|s| Some(s.setting()) == value).unwrap_or_default()
    }

    /// Its place on the switch and the start screen: 0, 1, 2.
    pub fn index(self) -> usize {
        Space::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The one after it, round: what Ctrl ` switches to.
    fn next(self) -> Space {
        Space::ALL[(self.index() + 1) % Space::ALL.len()]
    }
}

/// What a toast's Undo key runs.
pub(crate) type Action = Rc<dyn Fn()>;

/// One Money page: a header built once (search and period keys keep their focus) and a body
/// rebuilt on every read, under an overlay that carries the page's Undo toast ([`Toast`]).
pub(crate) struct Page {
    pub head: gtk::Box,
    pub body: gtk::Box,
}

/// A page's toast at its foot: what was just done, and Undo when it can be undone. Each Money
/// page has one, the Threads page too, for what its panel's sheets save and delete.
struct Toast {
    revealer: gtk::Revealer,
    text: gtk::Label,
    undo_key: gtk::Button,
    undo: Rc<RefCell<Option<Action>>>,
    serial: Cell<u64>,
}

#[derive(Default)]
struct State {
    space: Cell<Space>,
    /// The space last read from or written to `native.space`; `None` until it was read.
    saved: Cell<Option<Space>>,
    started: Cell<bool>,
    /// The switch's keys, in [`Space::ALL`]'s order.
    toggles: RefCell<Vec<gtk::ToggleButton>>,
    dev_only: RefCell<Vec<gtk::Widget>>,
    money_only: RefCell<Vec<gtk::Widget>>,
    aide_only: RefCell<Vec<gtk::Widget>>,
    clips_only: RefCell<Vec<gtk::Widget>>,
    /// The title bar's + Entry: Tally's, so it shows on the threads and Tally's pages only.
    add_key: RefCell<Option<gtk::Button>>,
    pages: RefCell<BTreeMap<&'static str, Rc<Page>>>,
    toasts: RefCell<BTreeMap<&'static str, Rc<Toast>>>,
    start: RefCell<Option<gtk::Widget>>,
    period_offset: Cell<i64>,
    query: RefCell<String>,
    /// Entries' account and category filters.
    filters: Cell<(Option<i64>, Option<i64>)>,
    currency: RefCell<String>,
    /// An `arbiter.changed` is being waited on: the runner can announce several in a moment.
    arbiter_pending: Cell<bool>,
}

thread_local! {
    static STATE: State = State::default();
}

pub fn is_page(page: &str) -> bool {
    page == threads::PAGE || page.starts_with("money-") || page.starts_with("arbiter-") || page.starts_with("avex-") || page.starts_with("aide-")
        || page.starts_with("clips-")
}

/// The space showing.
pub fn space() -> Space {
    STATE.with(|s| s.space.get())
}

/// The space page `page` belongs to; `None` for Settings, which every space shares. The threads
/// page is Aide's while it shows an Aide chat.
pub fn space_of(page: &str) -> Option<Space> {
    if page == "settings" {
        None
    } else if page.starts_with("aide-") || (page == threads::PAGE && threads::aide().is_some()) {
        Some(Space::Aide)
    } else if page.starts_with("clips-") {
        Some(Space::Clips)
    } else if is_page(page) {
        Some(Space::Threads)
    } else {
        Some(Space::Dev)
    }
}

pub(crate) fn page(name: &str) -> Option<Rc<Page>> {
    STATE.with(|s| s.pages.borrow().get(name).cloned())
}

/// Whether page `name` has a toast, so a change made on it can offer Undo there ([`toast`]).
pub(crate) fn has_toast(name: &str) -> bool {
    STATE.with(|s| s.toasts.borrow().contains_key(name))
}

/// Formats amounts in the ledger's currency, as the last read reported it.
pub(crate) fn formatter() -> MoneyFormatter {
    let currency = STATE.with(|s| s.currency.borrow().clone());
    MoneyFormatter::new(if currency.is_empty() { "CAD" } else { &currency }, Locale::from_env())
}

/// The ledger's currency as the last read reported it (empty before one did): what [`formatter`]
/// formats in.
pub(crate) fn currency() -> String {
    STATE.with(|s| s.currency.borrow().clone())
}

pub(crate) fn remember_currency(v: &Value) {
    if let Some(code) = v["currency"].as_str().filter(|c| !c.is_empty()) {
        STATE.with(|s| *s.currency.borrow_mut() = code.to_string());
    }
}

pub(crate) fn period_offset() -> i64 {
    STATE.with(|s| s.period_offset.get())
}
pub(crate) fn set_period_offset(offset: i64) {
    STATE.with(|s| s.period_offset.set(offset));
}
pub(crate) fn query() -> String {
    STATE.with(|s| s.query.borrow().clone())
}
pub(crate) fn set_query(query: &str) {
    STATE.with(|s| *s.query.borrow_mut() = query.to_string());
}
/// Entries' filters: (account id, category id), `None` for all.
pub(crate) fn filters() -> (Option<i64>, Option<i64>) {
    STATE.with(|s| s.filters.get())
}
pub(crate) fn set_filters(account: Option<i64>, category: Option<i64>) {
    STATE.with(|s| s.filters.set((account, category)));
}

/// The title bar's Dev | Threads | Aide pill. Wired by [`install`].
pub fn switcher() -> gtk::Box {
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    pill.add_css_class("space-switch");
    pill.set_valign(gtk::Align::Center);
    pill.set_margin_start(16);
    let mut keys: Vec<gtk::ToggleButton> = Vec::new();
    for (space, name, about) in [
        (Space::Dev, "space-dev", "Dev: agents, tasks and code · Ctrl `"),
        (Space::Threads, "space-money", "Threads: ask about your data, with Tally beside you · Ctrl `"),
        (Space::Aide, "space-aide", "Aide: your bots, working while you are away · Ctrl `"),
        (Space::Clips, "space-clips", "Clips: Shorts from a stream's VODs · Ctrl `"),
    ] {
        let key = gtk::ToggleButton::with_label(space.caption());
        key.set_widget_name(name);
        key.set_tooltip_text(Some(about));
        if let Some(first) = keys.first() {
            key.set_group(Some(first));
        }
        pill.append(&key);
        keys.push(key);
    }
    keys[0].set_active(true);
    STATE.with(|s| *s.toggles.borrow_mut() = keys);
    pill
}

/// The title bar's add key, shown only in Money.
pub fn add_key() -> gtk::Button {
    let key = button("", "primary");
    key.set_widget_name("money-add");
    key.add_css_class("launch-key");
    key.add_css_class("money-add-key");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&crate::icons::image_with_stroke("plus", 14, 2.0));
    row.append(&label("Entry", ""));
    key.set_child(Some(&row));
    key.set_valign(gtk::Align::Center);
    key.set_tooltip_text(Some("Add an entry · Ctrl E"));
    key.set_visible(false);
    STATE.with(|s| *s.add_key.borrow_mut() = Some(key.clone()));
    key
}

/// The Threads space's sidebar: its keys and the thread list, hidden until the space shows.
pub fn sidebar_keys() -> gtk::Box {
    let nav = gtk::Box::new(gtk::Orientation::Vertical, 1);
    nav.add_css_class("navigation");
    nav.add_css_class("money-nav");
    nav.set_vexpand(true);
    nav.set_visible(false);
    STATE.with(|s| s.money_only.borrow_mut().push(nav.clone().upcast()));
    nav
}

/// The Aide space's sidebar: New bot, the Chief of Staff, the bots, and its pages at the foot,
/// hidden until the space shows.
pub fn aide_sidebar() -> gtk::Box {
    let nav = aide::sidebar();
    nav.set_visible(false);
    STATE.with(|s| s.aide_only.borrow_mut().push(nav.clone().upcast()));
    nav
}

/// The Clips space's sidebar: Import, the Overview, the Queue, the VODs and Settings, hidden
/// until the space shows.
pub fn clips_sidebar() -> gtk::Box {
    let nav = clips::sidebar();
    nav.set_visible(false);
    STATE.with(|s| s.clips_only.borrow_mut().push(nav.clone().upcast()));
    nav
}

/// Money's pages, added to the window's content stack: the threads, Tally's, Arbiter's, Avex's
/// and Aide's. Aide's need no tabs: its sidebar holds its pages.
pub fn add_pages(content: &gtk::Stack) {
    content.add_named(&threads::page(), Some(threads::PAGE));
    let tally = PAGES.iter().map(|(name, _, _)| (*name, tally_tabs(name)));
    let arbiter = arbiter::ALL.iter().map(|name| (*name, arbiter::tabs(name)));
    let avex = avex::ALL.iter().map(|name| (*name, avex::tabs(name)));
    let aide = aide::ALL.iter().map(|name| (*name, gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    let clips = clips::ALL.iter().map(|name| (*name, gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    for (name, tabs) in tally.chain(arbiter).chain(avex).chain(aide).chain(clips) {
        let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
        head.add_css_class("money-head");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body.add_css_class("money-body");
        let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
        column.add_css_class("money-page");
        column.append(&tabs);
        column.append(&head);
        column.append(&body);
        let scroll = crate::app::scrolled(&column);
        scroll.add_css_class("money-scroll");
        // External: the column's minimum width never reaches the window, so the window can
        // shrink; the column follows the width instead (`pages::fit`).
        scroll.set_policy(gtk::PolicyType::External, gtk::PolicyType::Automatic);
        // The probe measures the page outside the scroller, where its content cannot widen it.
        let probe = gtk::DrawingArea::new();
        probe.set_hexpand(true);
        probe.set_can_target(false);
        probe.set_focusable(false);
        probe.set_accessible_role(gtk::AccessibleRole::Presentation);
        let fitted = column.downgrade();
        probe.connect_resize(move |_, width, _| {
            let fitted = fitted.clone();
            // Never resize inside an allocation: the next idle applies it.
            glib::idle_add_local_once(move || {
                if let Some(column) = fitted.upgrade() {
                    pages::fit(&column, width);
                }
            });
        });
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.append(&probe);
        frame.append(&scroll);
        let root = gtk::Overlay::new();
        root.add_css_class("money-root");
        root.set_child(Some(&frame));
        add_toast(&root, name, 24);
        content.add_named(&root, Some(name));
        STATE.with(|s| s.pages.borrow_mut().insert(name, Rc::new(Page { head, body })));
    }
}

/// Page `name`'s toast, over the foot of `over`, `margin` pixels up: [`toast`] shows it.
fn add_toast(over: &gtk::Overlay, name: &'static str, margin: i32) {
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    revealer.set_transition_duration(160);
    revealer.set_halign(gtk::Align::Center);
    revealer.set_valign(gtk::Align::End);
    revealer.set_margin_bottom(margin);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    bar.add_css_class("money-toast");
    let text = label("", "");
    bar.append(&text);
    let undo_key = button("Undo", "money-text-action");
    undo_key.set_widget_name("money-undo");
    bar.append(&undo_key);
    revealer.set_child(Some(&bar));
    revealer.set_can_target(false);
    over.add_overlay(&revealer);
    let toast = Rc::new(Toast { revealer, text, undo_key, undo: Rc::default(), serial: Cell::default() });
    let undo = toast.undo.clone();
    let revealer = toast.revealer.clone();
    toast.undo_key.connect_clicked(move |_| {
        revealer.set_reveal_child(false);
        revealer.set_can_target(false);
        if let Some(run) = undo.borrow_mut().take() {
            run();
        }
    });
    STATE.with(|s| s.toasts.borrow_mut().insert(name, toast));
}

/// Tally's views as Dev's segmented control, above each Tally page.
fn tally_tabs(showing: &'static str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("money-tabs");
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("view-tabs");
    tabs.set_halign(gtk::Align::Start);
    for (name, caption, _) in PAGES {
        let tab = button(caption, "quiet");
        if name == showing {
            tab.add_css_class("selected");
        }
        tab.connect_clicked(move |_| {
            if let Some(ui) = threads::the_ui_pub() {
                ui.navigate(name);
            }
        });
        tabs.append(&tab);
    }
    row.append(&tabs);
    row
}

/// A toast on the page showing now, a Money page or Threads: `text`, and an Undo key when
/// `undo` is given. A page with no toast shows `text` in the window's good news banner, with no
/// Undo. On a Tally page, where Ctrl Z presses the Undo key ([`install`]), the toast says so.
pub(crate) fn toast(ui: &Ui, text: &str, undo: Option<Action>) {
    let Some(toast) = STATE.with(|s| s.toasts.borrow().get(ui.page.borrow().as_str()).cloned()) else {
        ui.show_info(text);
        return;
    };
    if undo.is_some() && tally_page(ui.page.borrow().as_str()) {
        toast.text.set_text(&format!("{text} · Ctrl Z"));
    } else {
        toast.text.set_text(text);
    }
    toast.undo_key.set_visible(undo.is_some());
    *toast.undo.borrow_mut() = undo;
    toast.revealer.set_can_target(true);
    toast.revealer.set_reveal_child(true);
    let serial = toast.serial.get().wrapping_add(1);
    toast.serial.set(serial);
    let weak = Rc::downgrade(&toast);
    glib::timeout_add_local_once(std::time::Duration::from_secs(7), move || {
        if let Some(toast) = weak.upgrade().filter(|t| t.serial.get() == serial) {
            toast.revealer.set_reveal_child(false);
            toast.revealer.set_can_target(false);
            toast.undo.borrow_mut().take();
        }
    });
}

/// Whether `name` is one of Tally's own pages ([`PAGES`]).
fn tally_page(name: &str) -> bool {
    PAGES.iter().any(|(page, _, _)| *page == name)
}

/// Whether the focus is in a field or a menu, which keep their keys: Ctrl Z there undoes typing.
fn typing(ui: &Ui) -> bool {
    let mut at = GtkWindowExt::focus(&ui.window);
    while let Some(widget) = at {
        if widget.is::<gtk::Editable>() || widget.is::<gtk::TextView>() || widget.is::<gtk::Popover>() {
            return true;
        }
        at = widget.parent();
    }
    false
}

/// Wires the switcher, Money's sidebar keys and its add key. `nav` is Money's sidebar box,
/// `dev_only` what the dev space shows and Money hides.
pub fn install(ui: &Rc<Ui>, nav: &gtk::Box, add: &gtk::Button, dev_only: Vec<gtk::Widget>) {
    STATE.with(|s| *s.dev_only.borrow_mut() = dev_only);
    let toggles = STATE.with(|s| s.toggles.borrow().clone());
    for (key, space) in toggles.into_iter().zip(Space::ALL) {
        let weak = Rc::downgrade(ui);
        key.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                set_space(&ui, space);
            }
        });
    }
    threads::install(ui);
    // New thread, then Tally (its pages carry their own tabs), then the threads.
    let new_key = button("", "nav");
    new_key.set_widget_name("nav-new-thread");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(&crate::icons::image("plus", 16));
    let caption = label("New thread", "nav-label");
    caption.set_hexpand(true);
    row.append(&caption);
    // The keycap keeps its own height: a box row would stretch it to the key's.
    let keycap = label("Ctrl N", "keycap");
    keycap.set_valign(gtk::Align::Center);
    row.append(&keycap);
    new_key.set_child(Some(&row));
    let weak = Rc::downgrade(ui);
    new_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            threads::new_thread(&ui);
        }
    });
    nav.append(&new_key);
    let tally_key = button("", "nav");
    tally_key.set_widget_name("nav-money-home");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(&pages::glyph_image("home", 16));
    row.append(&label("Tally", "nav-label"));
    tally_key.set_child(Some(&row));
    let weak = Rc::downgrade(ui);
    tally_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("money-home");
        }
    });
    nav.append(&tally_key);
    // Arbiter, Tally's sibling, with a lamp for what its strategies are doing.
    let arbiter_key = button("", "nav");
    arbiter_key.set_widget_name("nav-arbiter-home");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(&arbiter::icon("arbiter", 16));
    let caption = label("Arbiter", "nav-label");
    caption.set_hexpand(true);
    row.append(&caption);
    row.append(&arbiter::lamp_widget());
    arbiter_key.set_child(Some(&row));
    let weak = Rc::downgrade(ui);
    arbiter_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("arbiter-home");
        }
    });
    nav.append(&arbiter_key);
    // Avex, the gym app: its training history as last exported from the phone.
    let avex_key = button("", "nav");
    avex_key.set_widget_name("nav-avex-home");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(&avex::icon("avex", 16));
    row.append(&label("Avex", "nav-label"));
    avex_key.set_child(Some(&row));
    let weak = Rc::downgrade(ui);
    avex_key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.navigate("avex-home");
        }
    });
    nav.append(&avex_key);
    let keys = [(tally_key.downgrade(), "money-"), (arbiter_key.downgrade(), "arbiter-"), (avex_key.downgrade(), "avex-")];
    ui.content.connect_visible_child_name_notify(move |stack| {
        let showing = stack.visible_child_name();
        for (key, prefix) in &keys {
            if let Some(key) = key.upgrade() {
                if showing.as_ref().is_some_and(|n| n.starts_with(prefix)) {
                    key.add_css_class("selected");
                } else {
                    key.remove_css_class("selected");
                }
            }
        }
    });
    nav.append(&threads::sidebar_list());
    let weak = Rc::downgrade(ui);
    add.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            add_entry(&ui);
        }
    });
    // Ctrl E in Threads logs an entry, as the Entry key does: from a page or from a thread. A
    // sheet already open, or the start screen, keeps the key.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, keycode, mods| {
        use gtk::gdk::ModifierType;
        let Some(ui) = weak.upgrade() else { return glib::Propagation::Proceed };
        let mods = mods & (ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK | ModifierType::ALT_MASK | ModifierType::META_MASK);
        let open = space() == Space::Threads && ui.panels.borrow().is_empty() && STATE.with(|s| s.start.borrow().is_none());
        if !open || mods != ModifierType::CONTROL_MASK || crate::shortcuts::latin(key, keycode).to_lower() != gtk::gdk::Key::e {
            return glib::Propagation::Proceed;
        }
        add_entry(&ui);
        glib::Propagation::Stop
    });
    ui.window.add_controller(keys);
    // Ctrl Z on a Tally page presses its toast's Undo while that shows, as a click does: Delete on
    // a focused row deletes, and the keyboard takes it back. A field keeps the key for its text;
    // a sheet open, or the start screen, keeps it too.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, keycode, mods| {
        use gtk::gdk::ModifierType;
        let Some(ui) = weak.upgrade() else { return glib::Propagation::Proceed };
        let mods = mods & (ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK | ModifierType::ALT_MASK | ModifierType::META_MASK);
        if mods != ModifierType::CONTROL_MASK || crate::shortcuts::latin(key, keycode).to_lower() != gtk::gdk::Key::z {
            return glib::Propagation::Proceed;
        }
        let showing = ui.page.borrow().clone();
        let open = tally_page(&showing) && ui.panels.borrow().is_empty() && STATE.with(|s| s.start.borrow().is_none()) && !typing(&ui);
        let toast = STATE.with(|s| s.toasts.borrow().get(showing.as_str()).cloned()).filter(|t| open && t.undo.borrow().is_some());
        let Some(toast) = toast else { return glib::Propagation::Proceed };
        toast.undo_key.emit_clicked();
        glib::Propagation::Stop
    });
    ui.window.add_controller(keys);
    pages::install(ui);
    arbiter::install();
    avex::install();
    aide::install(ui);
    clips::install(ui);
}

/// Shows `space`'s keys and look. Navigation calls it through [`follow`].
fn apply(ui: &Ui, space: Space) {
    STATE.with(|s| {
        s.space.set(space);
        for w in s.dev_only.borrow().iter() {
            w.set_visible(space == Space::Dev);
        }
        for w in s.money_only.borrow().iter() {
            w.set_visible(space == Space::Threads);
        }
        for w in s.aide_only.borrow().iter() {
            w.set_visible(space == Space::Aide);
        }
        for w in s.clips_only.borrow().iter() {
            w.set_visible(space == Space::Clips);
        }
    });
    // Both spaces past Dev hide what is Dev's alone (`space-money`); Aide adds its own class.
    if space == Space::Dev {
        ui.window.remove_css_class("space-money");
    } else {
        ui.window.add_css_class("space-money");
    }
    if space == Space::Aide {
        ui.window.add_css_class("space-aide");
    } else {
        ui.window.remove_css_class("space-aide");
    }
    if space == Space::Clips {
        ui.window.add_css_class("space-clips");
    } else {
        ui.window.remove_css_class("space-clips");
    }
    sync_toggles();
}

fn sync_toggles() {
    let showing = space();
    let toggles = STATE.with(|s| s.toggles.borrow().clone());
    for (key, space) in toggles.iter().zip(Space::ALL) {
        key.set_active(space == showing);
    }
}

/// Called by `Ui::navigate` once `page` shows, and by the threads page when it moves in or out of
/// an Aide chat. A page puts the window in its space; Settings, which every space shares, leaves
/// it where it is. `restoring` is a layout restore or an engine echo: it changes the space shown
/// but is never remembered as the person's choice.
pub fn follow(ui: &Rc<Ui>, page: &str, restoring: bool) {
    let Some(to) = space_of(page) else { return };
    if space() != to {
        apply(ui, to);
    }
    if to == Space::Aide {
        aide::mark_sidebar(ui);
    }
    if to == Space::Clips {
        clips::mark_sidebar(ui);
    }
    // Avex and Arbiter have no entries to add: no Tally key over them.
    let entries = to == Space::Threads && !["avex-", "arbiter-"].iter().any(|p| page.starts_with(p));
    if let Some(key) = STATE.with(|s| s.add_key.borrow().clone()) {
        key.set_visible(entries);
    }
    let start_open = STATE.with(|s| s.start.borrow().is_some());
    let saved = STATE.with(|s| s.saved.get());
    if !restoring && !start_open && saved.is_some_and(|saved| saved != to) {
        remember(ui, to);
    }
}

fn remember(ui: &Rc<Ui>, space: Space) {
    STATE.with(|s| s.saved.set(Some(space)));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("settings.set", json!({"path":"native.space","value":space.setting()})).await {
            ui.show_error(&e.to_string());
        }
    });
}

/// Switch space: Threads lands on its threads, Aide on the Chief of Staff, Dev on the agent wall.
pub fn set_space(ui: &Rc<Ui>, to: Space) {
    dismiss_start(ui, None);
    let showing = space_of(&ui.page.borrow());
    if space() != to || showing != Some(to) {
        match to {
            Space::Dev => ui.navigate("agents"),
            // The threads page showing an Aide chat leaves it for a new thread of its own.
            Space::Threads if threads::aide().is_some() => threads::new_thread(ui),
            Space::Threads => ui.navigate(threads::PAGE),
            Space::Aide => aide::open_chief(ui, None),
            Space::Clips => ui.navigate(clips::HOME),
        }
    }
    // A refused navigation (unsaved work in a panel) leaves the pill where the window is.
    sync_toggles();
}

pub fn toggle_space(ui: &Rc<Ui>) {
    set_space(ui, space().next());
}

/// Ctrl N in the space showing past Dev: a new thread in Threads, a new bot in Aide.
pub fn new_in_space(ui: &Rc<Ui>) {
    if space() == Space::Aide {
        aide::new_bot(ui);
    } else if space() == Space::Clips {
        clips::import(ui, None);
    } else {
        threads::new_thread(ui);
    }
}

/// The capture harness's way into Aide (smoke.rs `aide` and `aide-bot`): the Chief of Staff, or
/// the first bot's chat.
pub fn aide_smoke_open(ui: &Rc<Ui>, bot: bool) {
    if !bot {
        aide::open_chief(ui, None);
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let jobs = ui.call("aide.job.list", json!({})).await.map(|j| crate::app::rows(&j, "jobs")).unwrap_or_default();
        if let Some(j) = jobs.first() {
            aide::open_bot(&ui, j["id"].as_i64().unwrap_or(0), crate::app::text(j, "name"));
        }
    });
}

/// Aide's Chief of Staff: a notification from Aide opens it.
pub fn open_aide(ui: &Rc<Ui>) {
    dismiss_start(ui, None);
    aide::open_chief(ui, None);
}

pub fn add_entry(ui: &Rc<Ui>) {
    entry::open(ui, None);
}

pub(crate) fn add_account(ui: &Rc<Ui>) {
    entry::account(ui);
}

/// The add account sheet with Investment chosen, its registration and institution showing.
pub(crate) fn add_investment_account(ui: &Rc<Ui>) {
    entry::account_of(ui, Some("INVESTMENT"));
}

/// The record a value sheet for an investment account (`money.value.set`).
pub(crate) fn record_value(ui: &Rc<Ui>, account: &Value) {
    entry::value(ui, account);
}

pub(crate) fn edit_entry(ui: &Rc<Ui>, tx: Value) {
    entry::open(ui, Some(tx));
}

/// Re-reads the page of this space showing, if one is.
pub async fn refresh(ui: &Rc<Ui>, page: &str) {
    if page == threads::PAGE {
        threads::refresh(ui).await;
    } else {
        // The sidebar's thread list shows over Tally's pages too. A read that changed nothing
        // changes no row (`threads::draw_list`).
        threads::refresh_list_shown(ui);
        if page.starts_with("arbiter-") {
            arbiter::refresh(ui, page).await;
        } else if page.starts_with("avex-") {
            avex::refresh(ui, page).await;
        } else if page.starts_with("aide-") {
            aide::refresh(ui, page).await;
        } else if page.starts_with("clips-") {
            clips::refresh(ui, page).await;
        } else {
            pages::refresh(ui, page).await;
        }
    }
}

/// `money.changed`: the ledger moved, here or elsewhere.
pub fn changed(ui: &Rc<Ui>) {
    pages::forget_lists();
    let page = ui.page.borrow().clone();
    // A thread's charts read again in view; out of it, with another page showing, they wait to
    // read when they show again (`chart::load_stale`).
    chart::refresh_all();
    // A proposal card waiting in the open thread shows the ledger as it is now.
    ask::refresh();
    if page == threads::PAGE {
        threads::refresh_panel(ui);
    } else if is_page(&page) {
        ui.refresh_page();
    }
}

/// `arbiter.changed`: a strategy, an order or a setting moved. Several in a moment are read once:
/// the sidebar lamp, the cards in the conversation, and the page showing.
pub fn arbiter_changed(ui: &Rc<Ui>) {
    if STATE.with(|s| s.arbiter_pending.replace(true)) {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
        STATE.with(|s| s.arbiter_pending.set(false));
        let Some(ui) = weak.upgrade() else { return };
        arbiter::changed(&ui);
        let page = ui.page.borrow().clone();
        if page == threads::PAGE {
            threads::refresh_arbiter_panel(&ui);
            chart::refresh_prices();
        } else if page.starts_with("arbiter-") {
            ui.refresh_page();
        }
    });
}

/// `gym.changed`: a newer Avex export came in, or the copy was forgotten. The panel, the thread's
/// training charts and the Avex page showing read again.
pub fn gym_changed(ui: &Rc<Ui>) {
    let page = ui.page.borrow().clone();
    chart::refresh_gym();
    if page == threads::PAGE {
        threads::refresh_avex_panel(ui);
    } else if page.starts_with("avex-") {
        ui.refresh_page();
    }
}

/// `aide.changed`: a job, a run, a fact, an errand, a message, a page or the settings moved. The
/// Aide page showing and its sidebar read again, once for several in a moment.
pub fn aide_changed(ui: &Rc<Ui>) {
    aide::changed(ui);
}

/// `clips.changed`: a VOD's stage moved, a clip was drawn, approved or posted, or the settings
/// changed. The Clips page showing and its sidebar read again, once for several in a moment.
pub fn clips_changed(ui: &Rc<Ui>) {
    clips::changed(ui);
}

/// The capture harness's way into Clips (smoke.rs): `clips-vod-<id>`, `clips-clip-<id>` or
/// `clips-import`.
pub fn clips_smoke_open(ui: &Rc<Ui>, page: &str) {
    if let Some(id) = page.strip_prefix("clips-vod-").and_then(|i| i.parse().ok()) {
        clips::open_vod(ui, id);
    } else if let Some(id) = page.strip_prefix("clips-clip-").and_then(|i| i.parse().ok()) {
        clips::open_clip(ui, id);
    } else {
        ui.navigate(clips::HOME);
        clips::import(ui, None);
    }
}

/// Clips' Overview: a notification from Clips opens it.
pub fn open_clips(ui: &Rc<Ui>) {
    dismiss_start(ui, None);
    ui.navigate(clips::HOME);
}

/// A Tally glyph, for the Threads page's own marks.
pub(crate) fn pages_glyph(key: &str, size: i32) -> gtk::Image {
    pages::glyph_image(key, size)
}

/// `thread.*`: a thread moved.
pub fn thread_event(ui: &Rc<Ui>, ev: &str, payload: &Value) {
    threads::event(ui, ev, payload);
}

/// Entries for the command palette: (key, caption).
pub fn palette_entries() -> Vec<(&'static str, String)> {
    let showing = space();
    let mut entries: Vec<(&'static str, String)> = [("space-dev", Space::Dev), ("space-money", Space::Threads), ("space-aide", Space::Aide), ("space-clips", Space::Clips)]
        .into_iter()
        .filter(|(_, s)| *s != showing)
        .map(|(key, s)| (key, format!("Switch to {}", s.caption())))
        .collect();
    entries.extend([
        ("start", String::from("Start screen")),
        ("thread", String::from("New thread")),
        ("add", String::from("Add a Tally entry")),
    ]);
    for (name, caption, _) in PAGES {
        entries.push((name, format!("Tally {}", caption.to_lowercase())));
    }
    for (name, caption) in arbiter::PAGES {
        entries.push((name, format!("Arbiter {}", caption.to_lowercase())));
    }
    for (name, caption) in avex::PAGES {
        entries.push((name, format!("Avex {}", caption.to_lowercase())));
    }
    for (name, caption) in aide::PAGES {
        entries.push((name, format!("Aide {}", caption.to_lowercase())));
    }
    for (name, caption) in clips::PAGES {
        entries.push((name, format!("Clips {}", caption.to_lowercase())));
    }
    entries.push(("clips-import", String::from("Clips: import a VOD")));
    entries.push(("aide-chief", String::from("Aide: Chief of Staff")));
    entries.push(("aide-new-bot", String::from("Aide: new bot")));
    entries.push(("aide-new-job", String::from("Aide: new bot from a form")));
    entries.push(("avex-import", String::from("Avex: import training history")));
    // The kill switch, from anywhere (docs/ARBITER.md).
    entries.push(("arbiter-halt", String::from("Arbiter: halt all")));
    entries
}

pub fn run_palette(ui: &Rc<Ui>, key: &str) {
    match key {
        "space-dev" => set_space(ui, Space::Dev),
        "space-money" => set_space(ui, Space::Threads),
        "space-aide" => set_space(ui, Space::Aide),
        "space-clips" => set_space(ui, Space::Clips),
        "clips-import" => clips::import(ui, None),
        "start" => show_start(ui),
        "thread" => threads::new_thread(ui),
        "add" => add_entry(ui),
        "arbiter-halt" => arbiter::halt_all(ui),
        "avex-import" => avex::import(ui),
        "aide-chief" => open_aide(ui),
        "aide-new-bot" => aide::new_bot(ui),
        "aide-new-job" => aide::editor(ui, None, &Value::Null),
        page => ui.navigate(page),
    }
}

/// After the first connection: read the remembered space and the Threads page as it was left,
/// then greet. Display smoke runs (RELAY_NATIVE_SCREENSHOT) start in Dev with no greeting and
/// the Threads page's defaults, as they always have.
pub fn startup(ui: &Rc<Ui>) {
    if STATE.with(|s| s.started.replace(true)) {
        return;
    }
    // The sidebar's Arbiter lamp, lit before its page is ever opened.
    arbiter::read_summary(ui);
    if std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_some() {
        return;
    }
    threads::restore(ui);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let space = match ui.call("settings.get", json!({"path":"native.space"})).await {
            Ok(v) => Space::from_setting(v["value"].as_str()),
            Err(_) => Space::Dev,
        };
        STATE.with(|s| s.saved.set(Some(space)));
        show_start(&ui);
    });
}

/// "Good evening, Antho", by the local hour.
pub fn greeting() -> String {
    let hour = glib::DateTime::now_local().map(|now| now.hour()).unwrap_or(12);
    let part = match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        18..=22 => "Good evening",
        _ => "Good night",
    };
    let name = crate::app::first_name();
    if name.is_empty() {
        part.to_string()
    } else {
        format!("{part}, {name}")
    }
}

/// The start screen: a greeting and one card per space. A card picks its space; Escape picks
/// the last one used.
pub fn show_start(ui: &Rc<Ui>) {
    if STATE.with(|s| s.start.borrow().is_some()) {
        return;
    }
    if !ui.dismiss_panels() {
        return;
    }
    let remembered = STATE.with(|s| s.saved.get()).unwrap_or(space());
    let weak = Rc::downgrade(ui);
    let pick: Rc<dyn Fn(usize)> = Rc::new(move |index| {
        if let Some(ui) = weak.upgrade() {
            dismiss_start(&ui, Some(Space::ALL[index]));
        }
    });
    let crate::start::Built { screen, lines, rows, keys } = crate::start::build(&greeting(), remembered.caption(), pick);
    // Escape opens the space used last; 1 to 4 pick Dev, Threads, Aide and Clips, as the cards say.
    let escape = gtk::EventControllerKey::new();
    escape.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(ui);
    escape.connect_key_pressed(move |_, key, _, _| {
        use gtk::gdk::Key;
        let pick = match key {
            Key::Escape => remembered,
            Key::_1 | Key::KP_1 => Space::Dev,
            Key::_2 | Key::KP_2 => Space::Threads,
            Key::_3 | Key::KP_3 => Space::Aide,
            Key::_4 | Key::KP_4 => Space::Clips,
            _ => return glib::Propagation::Proceed,
        };
        if let Some(ui) = weak.upgrade() {
            dismiss_start(&ui, Some(pick));
        }
        glib::Propagation::Stop
    });
    screen.add_controller(escape);
    if let Some(content) = ui.overlay.child() {
        content.set_sensitive(false);
    }
    // The bars keep their place and the window controls; their keys and readings step back.
    ui.window.add_css_class("starting");
    for part in &ui.start_chrome {
        part.set_opacity(0.0);
        part.set_sensitive(false);
    }
    ui.overlay.add_overlay(&screen);
    STATE.with(|s| *s.start.borrow_mut() = Some(screen.clone().upcast()));
    keys[remembered.index()].grab_focus();
    let [dev_line, money_line, aide_line, clips_line] = lines;
    let [dev_rows, thread_rows, aide_rows, clips_rows] = rows;
    let reader = ui.clone();
    glib::spawn_future_local(async move {
        use crate::start::Lamp;
        let (dashboard, summary, threads, aide_summary, bots, clips_summary, vods) = tokio::join!(
            reader.call("dashboard.get", json!({})),
            reader.call("money.summary", json!({})),
            reader.call("thread.list", json!({})),
            reader.call("aide.summary", json!({})),
            reader.call("aide.job.list", json!({})),
            reader.call("clips.summary", json!({})),
            reader.call("clips.vod.list", json!({}))
        );
        match clips_summary {
            Ok(c) => {
                let (reading, lamp) = clips::start_reading(&c);
                crate::start::reading(&clips_line, &reading, lamp);
            }
            Err(e) => crate::start::reading(&clips_line, &pages::unavailable(&e), Lamp::Idle),
        }
        let vods = vods.map(|v| clips::start_rows(&v)).unwrap_or_default();
        crate::start::recent(&clips_rows, &vods, "No VODs yet");
        match aide_summary {
            Ok(a) => {
                let (reading, lamp) = aide::start_reading(&a);
                crate::start::reading(&aide_line, &reading, lamp);
            }
            Err(e) => crate::start::reading(&aide_line, &pages::unavailable(&e), Lamp::Idle),
        }
        let bots = bots.map(|b| aide::start_rows(&b)).unwrap_or_default();
        crate::start::recent(&aide_rows, &bots, "No bots yet");
        match dashboard {
            Ok(d) => {
                let lamp = if waiting_on_you(&d) > 0 {
                    Lamp::Attention
                } else if d["sessions_live"].as_array().is_none_or(Vec::is_empty) {
                    Lamp::Idle
                } else {
                    Lamp::Live
                };
                crate::start::reading(&dev_line, &dev_reading(&d), lamp);
                crate::start::recent(&dev_rows, &dev_projects(&d), "No projects yet");
            }
            Err(e) => {
                crate::start::reading(&dev_line, &pages::unavailable(&e), Lamp::Idle);
                crate::start::recent(&dev_rows, &[], "");
            }
        }
        match summary {
            Ok(s) => crate::start::reading(&money_line, &money_reading(&s), if s["empty"] == true { Lamp::Idle } else { Lamp::Live }),
            Err(e) => crate::start::reading(&money_line, &pages::unavailable(&e), Lamp::Idle),
        }
        match threads {
            Ok(t) => crate::start::recent(&thread_rows, &recent_threads(&t), "No conversations yet"),
            Err(_) => crate::start::recent(&thread_rows, &[], ""),
        }
    });
}

/// Held actions waiting for a decision, and sessions stopped on a question.
fn waiting_on_you(d: &Value) -> usize {
    let holds = d["holds_open"].as_array().map_or(0, Vec::len);
    let blocked: u64 = d["projects"].as_array().into_iter().flatten().filter_map(|p| p["blocked_sessions"].as_u64()).sum();
    holds + blocked as usize
}

/// The two busiest projects, each with what is going on in it: "6 agents · 1 open".
fn dev_projects(d: &Value) -> Vec<(String, String)> {
    let count = |p: &Value, key: &str| p[key].as_u64().unwrap_or(0);
    let mut projects: Vec<&Value> = d["projects"].as_array().into_iter().flatten().collect();
    projects.sort_by_key(|p| std::cmp::Reverse((count(p, "blocked_sessions"), count(p, "live_sessions"), count(p, "tasks_open"))));
    projects
        .into_iter()
        .take(2)
        .map(|p| {
            let mut parts = Vec::new();
            match count(p, "live_sessions") {
                0 => {}
                1 => parts.push(String::from("1 agent")),
                n => parts.push(format!("{n} agents")),
            }
            if count(p, "in_review") > 0 {
                parts.push(format!("{} in review", count(p, "in_review")));
            }
            if count(p, "tasks_open") > 0 {
                parts.push(format!("{} open", count(p, "tasks_open")));
            }
            let detail = if parts.is_empty() { String::from("quiet") } else { parts.join(" · ") };
            (p["name"].as_str().unwrap_or("Project").to_string(), detail)
        })
        .collect()
}

/// The two threads used last, with when: "yesterday", or "replying" while one is.
fn recent_threads(t: &Value) -> Vec<(String, String)> {
    t["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .take(2)
        .map(|thread| {
            let title = thread["title"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("New thread").to_string();
            let when = if thread["working"] == true {
                String::from("replying")
            } else {
                thread["updated_at"]
                    .as_str()
                    .and_then(|at| crate::relative::ago(at, crate::relative::Form::Long))
                    .unwrap_or_default()
            };
            (title, when)
        })
        .collect()
}

/// "3 agents running · 2 need you · 1 in review", from `dashboard.get`.
fn dev_reading(d: &Value) -> String {
    let live = d["sessions_live"].as_array().map_or(0, Vec::len);
    let review = d["in_review"].as_array().map_or(0, Vec::len);
    let mut parts = vec![match live {
        0 => String::from("No agents running"),
        1 => String::from("1 agent running"),
        n => format!("{n} agents running"),
    }];
    match waiting_on_you(d) {
        0 => {}
        1 => parts.push(String::from("1 needs you")),
        n => parts.push(format!("{n} need you")),
    }
    if review > 0 {
        parts.push(format!("{review} in review"));
    }
    parts.join(" · ")
}

/// "Budget · $412 left · on pace", from `money.summary`: short enough for its card's one line.
fn money_reading(s: &Value) -> String {
    if s["empty"] == true {
        return String::from("Start with your budget");
    }
    remember_currency(s);
    let margin = s["lines"]["margin"].as_str().unwrap_or("");
    match s["pace"]["status"].as_str().unwrap_or("") {
        "ON_PACE" | "UNDER_PACE" | "OVER_PACE" => {
            let left = formatter().format_whole(s["pace"]["remaining"].as_i64().unwrap_or(0));
            let pace = match s["pace"]["status"].as_str() {
                Some("OVER_PACE") => "over pace",
                Some("UNDER_PACE") => "under pace",
                _ => "on pace",
            };
            format!("Budget · {left} left · {pace}")
        }
        _ if margin.is_empty() => String::from("Budget"),
        _ => format!("Budget · {margin}"),
    }
}

/// Escape while the start screen shows: open the space last used. False when it is not showing.
pub fn escape_start(ui: &Rc<Ui>) -> bool {
    if STATE.with(|s| s.start.borrow().is_none()) {
        return false;
    }
    let remembered = STATE.with(|s| s.saved.get()).unwrap_or(space());
    dismiss_start(ui, Some(remembered));
    true
}

/// Closes the start screen; with `pick`, into that space.
fn dismiss_start(ui: &Rc<Ui>, pick: Option<Space>) {
    let Some(screen) = STATE.with(|s| s.start.borrow_mut().take()) else {
        return;
    };
    // It fades out over what it opened into; the bars come back at once.
    let overlay = ui.overlay.clone();
    let leaving = screen.clone();
    crate::start::leave(&screen, move || overlay.remove_overlay(&leaving));
    ui.window.remove_css_class("starting");
    for part in &ui.start_chrome {
        part.set_opacity(1.0);
        part.set_sensitive(true);
    }
    if ui.panels.borrow().is_empty() {
        if let Some(content) = ui.overlay.child() {
            content.set_sensitive(true);
        }
    }
    if let Some(to) = pick {
        set_space(ui, to);
        // Picking the space already showing navigates nowhere, so `follow` never saw it.
        if STATE.with(|s| s.saved.get()).is_some_and(|saved| saved != to) && space() == to {
            remember(ui, to);
        }
    }
}

/// An empty column with a message: what a page shows before its first read, or when it fails.
pub(crate) fn message(body: &gtk::Box, text: &str) {
    clear(body);
    let l = label(text, "money-muted");
    l.set_wrap(true);
    body.append(&l);
}
