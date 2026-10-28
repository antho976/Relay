//! The assistant (docs/THREADS.md, "The assistant"): a drawer the title bar's key opens over the
//! Dev space, holding a conversation with an agent about Relay itself.
//!
//! The conversation is a thread of the `assistant` kind, kept by the engine like any thread, so
//! the drawer reopens on the newest one without asking the agent anything, and its history lists
//! the rest. The agent reads freely, keeps the board and notes, mails sessions and switches skills
//! and plugins itself; what changes a session it asks for, and each ask shows here as a card the
//! person approves or declines (`thread.ask.*`) unless the person lets that kind run without
//! asking (the gear's switches, `threads.assistant.without_asking`). The card's state is the
//! engine's, read again when the thread opens and on every `thread.ask`, so an answer stays
//! answered.
//!
//! A reply may show live cards of tasks, sessions and notes, and open a page of the app
//! (`relay_client::assistant_view`): a card is read from Relay when drawn and follows its row's
//! events; a page opens only for a reply to a message sent from this drawer in this run, never
//! for one read back from history.
//!
//! The agent acts only on what the person writes here: nothing starts it but a message.
use crate::app::{button, clear, icon_button, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::assistant_view::{self as view, Part, Target};
use relay_client::thread_view::tool_op;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// The drawer's width, in px.
const WIDTH: i32 = 420;

/// The Claude models the assistant may run on: (id, caption). An empty id is Claude's default.
/// The models on this PC follow them in the picker (`thread.models`).
const MODELS: [(&str, &str); 5] = relay_client::models::CLAUDE;

/// How hard a Claude model thinks: (value, caption), as `claude --effort` takes them. A model on
/// this PC has none.
const EFFORTS: [(&str, &str); 6] = [
    ("", "Default effort"),
    ("low", "Low"),
    ("medium", "Medium"),
    ("high", "High"),
    ("xhigh", "Extra high"),
    ("max", "Max"),
];

/// The message box's placeholder.
const HINT: &str = "Ask about Relay, or tell it what to do";

/// The subtitle while no conversation is open.
const SUBTITLE: &str = "Reads Relay, keeps your board, launches agents";

/// What the empty drawer offers: (words, whether they are sent as they are). Words that need
/// finishing go into the message box instead.
const SUGGESTIONS: [(&str, bool); 5] = [
    ("Is anything waiting on me?", true),
    ("What are my sessions doing right now?", true),
    ("Open the board", true),
    ("Is there a task for ", false),
    ("Launch a builder to ", false),
];

/// The kinds of ask the person may let run without asking (`threads.assistant.without_asking`,
/// the engine's `threads::asks::KINDS`): (key, caption, detail).
const KINDS: [(&str, &str, &str); 3] = [
    ("launch", "Launch and wake agents", "Start, resume, wake and set up sessions, and dispatch tasks"),
    ("type", "Type into a session", "Typed words can answer an agent's own permission prompt"),
    ("stop", "Park and close sessions", "Closing keeps the session's worktree"),
];

/// A tool call drawn as a line: what it ran, with what, and the line, for its answer to finish.
struct Tool {
    op: String,
    input: Value,
    line: glib::WeakRef<gtk::Box>,
}

/// A live card on screen: its kind (`task`, `session`, `note`), what it names, and its key.
struct Card {
    kind: &'static str,
    item: String,
    key: glib::WeakRef<gtk::Button>,
}

struct Assistant {
    ui: Weak<Ui>,
    /// The title bar's key, lit while the drawer is open.
    key: gtk::Button,
    revealer: gtk::Revealer,
    /// The conversation's title under "Assistant".
    subtitle: gtk::Label,
    history_key: gtk::MenuButton,
    history_list: gtk::Box,
    settings_key: gtk::MenuButton,
    /// The switches for what runs without asking, by kind; `filling` while they are set from
    /// the settings, so setting them writes nothing back.
    allow: Vec<(&'static str, gtk::CheckButton)>,
    filling: Cell<bool>,
    scroll: gtk::ScrolledWindow,
    column: gtk::Box,
    empty: gtk::Box,
    input: gtk::TextView,
    /// The message box's placeholder, shown while it is empty.
    hint: gtk::Label,
    send: gtk::Button,
    /// The model picker: its key, the caption it shows, and its list.
    model_key: gtk::MenuButton,
    model_shown: gtk::Label,
    model_list: gtk::Box,
    /// Each choice's check, by model id, and by effort.
    marks: RefCell<Vec<(String, gtk::Image)>>,
    effort_marks: RefCell<Vec<(String, gtk::Image)>>,
    /// The picker's efforts, hidden while a model on this PC is shown.
    effort_box: gtk::Box,
    /// The model and effort a new chat starts on: the last picked, kept across runs.
    choice: RefCell<String>,
    effort_choice: RefCell<String>,
    /// The model and effort of the conversation shown (a new chat's are the choices).
    shown_model: RefCell<String>,
    shown_effort: RefCell<String>,
    /// The models on this PC that can run: (id, name).
    local: RefCell<Vec<(String, String)>>,
    /// The local model the server runs and whether it is `loading` or `ready`.
    local_state: RefCell<(String, String)>,
    working: gtk::Box,
    pending: gtk::Label,
    /// The thread shown, once there is one; a new chat has none until its first message.
    thread: Cell<Option<i64>>,
    /// The newest message drawn, so a catch-up draws each message once.
    last: Cell<i64>,
    busy: Cell<bool>,
    /// A message sent from here in this run is being answered: its reply's `open` blocks open.
    armed: Cell<bool>,
    /// Each tool call drawn, by its id, so its answer knows what it answers.
    tools: RefCell<HashMap<String, Tool>>,
    /// The ask cards on screen, by ask id.
    cards: RefCell<HashMap<i64, Vec<glib::WeakRef<gtk::Box>>>>,
    /// The task, session and note cards on screen.
    live: RefCell<Vec<Card>>,
    /// The drawer was opened once, and the newest thread read.
    loaded: Cell<bool>,
    /// What had the keyboard when the drawer opened (a terminal, mostly), which gets it back.
    before: RefCell<Option<glib::WeakRef<gtk::Widget>>>,
}

thread_local! {
    static ASSISTANT: RefCell<Option<Rc<Assistant>>> = const { RefCell::new(None) };
}

fn the() -> Option<Rc<Assistant>> {
    ASSISTANT.with(|a| a.borrow().clone())
}

/// A key in the drawer's head that opens a menu.
fn head_menu(icon: &str, caption: &str, name: &str, child: &gtk::Widget) -> gtk::MenuButton {
    let key = gtk::MenuButton::new();
    key.add_css_class("assistant-head-key");
    key.set_widget_name(name);
    key.set_child(Some(&crate::icons::image(icon, 16)));
    key.set_tooltip_text(Some(caption));
    key.update_property(&[gtk::accessible::Property::Label(caption)]);
    key.set_valign(gtk::Align::Center);
    let popover = gtk::Popover::new();
    popover.add_css_class("assistant-popover");
    popover.set_child(Some(child));
    // The keys sit by the window's right edge: the menu opens leftwards from them, whole, rather
    // than being pushed and squeezed back on screen.
    popover.set_halign(gtk::Align::End);
    key.set_popover(Some(&popover));
    key
}

/// Build the drawer over `host` and wire the title bar's `key` to open and close it.
pub fn install(ui: &Rc<Ui>, key: &gtk::Button, host: &gtk::Overlay) {
    let drawer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    drawer.add_css_class("assistant-drawer");
    drawer.set_widget_name("assistant");
    drawer.set_size_request(WIDTH, -1);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    head.add_css_class("assistant-head");
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
    titles.set_hexpand(true);
    titles.append(&label("Assistant", "assistant-title"));
    let subtitle = label(SUBTITLE, "assistant-subtitle");
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    titles.append(&subtitle);
    head.append(&titles);

    // Earlier chats, newest first.
    let history_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    history_list.add_css_class("threads-menu");
    history_list.set_size_request(320, -1);
    let history_scroll = gtk::ScrolledWindow::builder().child(&history_list).hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true).propagate_natural_width(true).max_content_height(420).build();
    let history_key = head_menu("history", "Earlier chats", "assistant-history", history_scroll.upcast_ref());
    head.append(&history_key);

    // What it may do without asking.
    let settings = gtk::Box::new(gtk::Orientation::Vertical, 8);
    settings.add_css_class("assistant-settings");
    settings.set_size_request(340, -1);
    settings.append(&label("Without asking, the assistant may", "threads-menu-heading"));
    let mut allow = Vec::new();
    for (kind, caption, detail) in KINDS {
        let check = gtk::CheckButton::new();
        check.set_widget_name(&format!("assistant-allow-{kind}"));
        let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
        words.append(&label(caption, "assistant-allow-caption"));
        let more = label(detail, "assistant-allow-detail");
        more.set_wrap(true);
        more.set_width_chars(38);
        more.set_max_width_chars(38);
        words.append(&more);
        check.set_child(Some(&words));
        check.connect_toggled(move |check| allow_changed(kind, check.is_active()));
        settings.append(&check);
        allow.push((kind, check));
    }
    let always = label(
        "It always keeps tasks and notes, sends mail and switches skills and plugins itself. Deleting a task, and adding a workspace or a project, always ask first.",
        "assistant-allow-detail",
    );
    always.set_wrap(true);
    always.set_width_chars(44);
    always.set_max_width_chars(44);
    always.add_css_class("assistant-allow-note");
    settings.append(&always);
    let settings_key = head_menu("gear", "What it does without asking", "assistant-settings", settings.upcast_ref());
    head.append(&settings_key);

    let fresh = icon_button("plus", "New chat");
    fresh.set_widget_name("assistant-new");
    fresh.set_valign(gtk::Align::Center);
    head.append(&fresh);
    let close = icon_button("close", "Close the assistant");
    close.set_widget_name("assistant-close");
    close.set_valign(gtk::Align::Center);
    head.append(&close);
    drawer.append(&head);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
    column.add_css_class("assistant-column");
    let empty = empty_state();
    column.append(&empty);
    let scroll = gtk::ScrolledWindow::builder().child(&column).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
    scroll.add_css_class("assistant-scroll");
    drawer.append(&scroll);

    let working = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    working.add_css_class("threads-tool");
    let spinner = crate::app::spinner();
    spinner.set_size_request(13, 13);
    working.append(&spinner);
    working.append(&label("Working…", "threads-tool-text"));
    let pending = label("", "threads-reply");
    pending.set_wrap(true);
    pending.set_wrap_mode(gtk::pango::WrapMode::WordChar);

    let composer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    composer.add_css_class("threads-composer");
    composer.add_css_class("assistant-composer");
    let input = gtk::TextView::new();
    input.add_css_class("threads-input");
    input.set_wrap_mode(gtk::WrapMode::WordChar);
    input.set_hexpand(true);
    // Tab leaves the box, for the picker and Send, as in Threads.
    input.set_accepts_tab(false);
    input.update_property(&[gtk::accessible::Property::Label("Message the assistant"), gtk::accessible::Property::Placeholder(HINT)]);
    let input_scroll = gtk::ScrolledWindow::builder().child(&input).hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).max_content_height(160).build();
    input_scroll.set_hexpand(true);
    // The placeholder: a text view has none of its own.
    let field = gtk::Overlay::new();
    field.set_child(Some(&input_scroll));
    let hint = label(HINT, "threads-hint");
    hint.set_can_target(false);
    hint.set_valign(gtk::Align::Start);
    hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    field.add_overlay(&hint);
    // Under the words: the model picker at the start, Send at the end.
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let model_key = gtk::MenuButton::new();
    model_key.add_css_class("threads-picker");
    model_key.set_widget_name("assistant-model");
    model_key.set_tooltip_text(Some("The model the assistant runs on"));
    let model_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let model_shown = label(MODELS[0].1, "");
    model_shown.set_ellipsize(gtk::pango::EllipsizeMode::End);
    model_row.append(&model_shown);
    model_row.append(&crate::icons::image("chevron-down", 10));
    model_key.set_child(Some(&model_row));
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.add_css_class("threads-menu");
    let model_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.append(&model_list);
    let effort_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.append(&effort_box);
    let model_popover = gtk::Popover::new();
    model_popover.set_child(Some(&menu));
    model_key.set_popover(Some(&model_popover));
    model_key.set_valign(gtk::Align::End);
    foot.append(&model_key);
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    foot.append(&gap);
    let send = button("", "primary");
    send.add_css_class("assistant-send");
    send.set_valign(gtk::Align::End);
    send.set_child(Some(&crate::icons::image("arrow-up", 16)));
    send.set_tooltip_text(Some("Send (Enter)"));
    send.update_property(&[gtk::accessible::Property::Label("Send")]);
    foot.append(&send);
    let stack = gtk::Box::new(gtk::Orientation::Vertical, 6);
    stack.set_hexpand(true);
    stack.append(&field);
    stack.append(&foot);
    composer.append(&stack);
    drawer.append(&composer);

    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
    revealer.set_transition_duration(160);
    revealer.set_halign(gtk::Align::End);
    revealer.set_valign(gtk::Align::Fill);
    revealer.set_child(Some(&drawer));
    revealer.set_reveal_child(false);
    // Closed, it takes no clicks from the panes under it.
    revealer.set_can_target(false);
    host.add_overlay(&revealer);

    let a = Rc::new(Assistant {
        ui: Rc::downgrade(ui),
        key: key.clone(),
        revealer,
        subtitle,
        history_key,
        history_list,
        settings_key,
        allow,
        filling: Cell::new(false),
        scroll,
        column,
        empty,
        input,
        hint,
        send,
        model_key,
        model_shown,
        model_list,
        marks: RefCell::default(),
        effort_marks: RefCell::default(),
        effort_box,
        choice: RefCell::default(),
        effort_choice: RefCell::default(),
        shown_model: RefCell::default(),
        shown_effort: RefCell::default(),
        local: RefCell::default(),
        local_state: RefCell::default(),
        working,
        pending,
        thread: Cell::new(None),
        last: Cell::new(0),
        busy: Cell::new(false),
        armed: Cell::new(false),
        tools: RefCell::default(),
        cards: RefCell::default(),
        live: RefCell::default(),
        loaded: Cell::new(false),
        before: RefCell::default(),
    });
    ASSISTANT.with(|slot| *slot.borrow_mut() = Some(a.clone()));
    show_tip(&ui.shortcut_tip("Assistant", "assistant"));
    fill_models(&a);
    fill_efforts(&a);
    restore_choice(ui);

    // Each menu reads what it shows as it opens: chats come and go, and the settings may have
    // been changed elsewhere.
    if let Some(popover) = a.history_key.popover() {
        popover.connect_show(|_| fill_history());
    }
    if let Some(popover) = a.settings_key.popover() {
        popover.connect_show(|_| read_allowed());
    }

    // Pressing the key leaves the keyboard where it was, for the drawer to give back on closing.
    key.set_focus_on_click(false);
    key.update_state(&[gtk::accessible::State::Expanded(Some(false))]);
    key.connect_clicked(|_| toggle());
    // The key is the Dev space's: in Threads it hides, and the drawer closes with it.
    key.connect_visible_notify(|key| {
        if !key.is_visible() {
            show(false);
        }
    });
    close.connect_clicked(|_| show(false));
    fresh.connect_clicked(|_| new_chat());
    // The key stops a reply while one runs; Enter only ever sends.
    a.send.connect_clicked(|_| if the().is_some_and(|a| a.busy.get()) { stop() } else { submit() });
    a.input.buffer().connect_changed(|_| {
        if let Some(a) = the() {
            update_send(&a);
        }
    });
    update_send(&a);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|_, key, _, mods| {
        match key {
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter if !mods.contains(gtk::gdk::ModifierType::SHIFT_MASK) => {
                submit();
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Escape => {
                show(false);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    a.input.add_controller(keys);
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(|_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            show(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    drawer.add_controller(escape);
}

fn empty_state() -> gtk::Box {
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
    empty.add_css_class("assistant-empty");
    let intro = label(
        "Ask about your sessions, tasks and projects. It keeps the board and notes, writes to sessions, switches \
         skills and plugins, launches agents and opens pages for you. The gear sets what else it does without asking; \
         the rest waits for your approval on a card.",
        "assistant-intro",
    );
    intro.set_wrap(true);
    empty.append(&intro);
    for (n, (words, whole)) in SUGGESTIONS.into_iter().enumerate() {
        let chip = button(words, "assistant-suggestion");
        chip.set_widget_name(&format!("assistant-suggestion-{n}"));
        if let Some(l) = chip.child().and_downcast::<gtk::Label>() {
            l.set_xalign(0.0);
            l.set_wrap(true);
            if !whole {
                l.set_text(&format!("{}\u{2026}", words.trim_end()));
            }
        }
        chip.connect_clicked(move |_| if whole { send(words) } else { draft(words) });
        empty.append(&chip);
    }
    empty
}

/// Put `words` in the message box to be finished, the caret after them.
fn draft(words: &str) {
    let Some(a) = the() else { return };
    let buffer = a.input.buffer();
    buffer.set_text(words);
    buffer.place_cursor(&buffer.end_iter());
    a.input.grab_focus();
}

fn toggle() {
    if let Some(a) = the() {
        show(!a.revealer.reveals_child());
    }
}

/// The shortcut (`shortcuts::DEFAULTS` "assistant"): the key's toggle, where the key shows;
/// `false` in Threads, where it does not, so the chord passes on.
pub fn shortcut() -> bool {
    if !the().is_some_and(|a| a.key.is_visible()) {
        return false;
    }
    toggle();
    true
}

/// The key's tooltip, naming the shortcut as it is bound.
pub fn show_tip(tip: &str) {
    if let Some(a) = the() {
        a.key.set_tooltip_text(Some(tip));
    }
}

fn show(open: bool) {
    let Some(a) = the() else { return };
    let focus = a.revealer.root().and_then(|root| root.focus());
    if open && !a.revealer.reveals_child() {
        *a.before.borrow_mut() = focus.filter(|w| !w.is_ancestor(&a.revealer)).map(|w| w.downgrade());
    } else if !open && a.revealer.reveals_child() {
        // Closing takes the keyboard out of the drawer, back to what had it; focus moved elsewhere
        // since stays there.
        let held = focus.is_none_or(|w| w.is_ancestor(&a.revealer));
        if let Some(w) = a.before.take().and_then(|w| w.upgrade()).filter(|w| held && w.is_mapped()) {
            w.grab_focus();
        }
    }
    a.revealer.set_reveal_child(open);
    a.revealer.set_can_target(open);
    if open {
        a.key.add_css_class("assistant-open");
    } else {
        a.key.remove_css_class("assistant-open");
    }
    a.key.update_state(&[gtk::accessible::State::Expanded(Some(open))]);
    if open {
        a.input.grab_focus();
        // A model file added since shows up the next time the drawer opens.
        load_local();
        if !a.loaded.replace(true) {
            load_newest();
        } else {
            resync();
            refresh_cards();
        }
    }
}

/// Bring the conversation shown up to date with Relay's copy: what was said while the window
/// heard no events (it reconnected, or fell behind), whether a reply still runs, and its asks.
/// Without it a reply that ended unheard left the drawer working, its key a Stop, for good.
pub fn resync() {
    let Some(id) = the().and_then(|a| a.thread.get()) else { return };
    glib::spawn_future_local(async move {
        catch_up(id, false).await;
        refresh_asks().await;
    });
}

/// Open the newest assistant thread, if there is one.
fn load_newest() {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    glib::spawn_future_local(async move {
        let newest = ui.call("thread.list", json!({"kind": "assistant"})).await.ok().and_then(|l| l["threads"][0]["id"].as_i64());
        if let Some(id) = newest {
            open(id).await;
        }
    });
}

/// Draw thread `id` from Relay's copy, then its asks as they stand.
async fn open(id: i64) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    let read = match ui.call("thread.get", json!({"id": id})).await {
        Ok(read) => read,
        Err(e) => return ui.show_error(&format!("The assistant's conversation did not open: {e}")),
    };
    reset(Some(id));
    *a.shown_model.borrow_mut() = read["thread"]["model"].as_str().unwrap_or("").to_string();
    *a.shown_effort.borrow_mut() = read["thread"]["effort"].as_str().unwrap_or("").to_string();
    show_model(&a);
    show_title(&a, &read["thread"]);
    for m in read["messages"].as_array().into_iter().flatten() {
        draw_message(&a, m);
    }
    set_busy(&a, read["thread"]["working"] == true);
    refresh_asks().await;
    stick_to_bottom(&a, true);
}

/// The subtitle: the conversation's title, or what the assistant does while none is open.
fn show_title(a: &Assistant, thread: &Value) {
    let title = thread["title"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or(SUBTITLE);
    a.subtitle.set_text(title);
    a.subtitle.set_tooltip_text(Some(title));
}

/// Clear the drawer for thread `id` (none: a new chat).
fn reset(id: Option<i64>) {
    let Some(a) = the() else { return };
    a.thread.set(id);
    a.last.set(0);
    a.tools.borrow_mut().clear();
    a.cards.borrow_mut().clear();
    a.live.borrow_mut().clear();
    clear(&a.column);
    a.pending.set_text("");
    a.column.append(&a.empty);
    a.empty.set_visible(id.is_none());
    if id.is_none() {
        show_title(&a, &Value::Null);
    }
    set_busy(&a, false);
}

fn new_chat() {
    reset(None);
    if let Some(a) = the() {
        *a.shown_model.borrow_mut() = a.choice.borrow().clone();
        *a.shown_effort.borrow_mut() = a.effort_choice.borrow().clone();
        show_model(&a);
        a.input.buffer().set_text("");
        a.input.grab_focus();
    }
}

fn submit() {
    let Some(a) = the() else { return };
    let buffer = a.input.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
    // Words written while it answers stay in the box, and Enter says why.
    if a.busy.get() {
        if let Some(ui) = a.ui.upgrade().filter(|_| !text.is_empty()) {
            ui.show_info("The assistant is still answering: stop it first, or wait");
        }
        return;
    }
    if text.is_empty() {
        return;
    }
    buffer.set_text("");
    send(&text);
}

fn send(text: &str) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    if a.busy.get() {
        return;
    }
    set_busy(&a, true);
    a.armed.set(true);
    a.empty.set_visible(false);
    let text = text.to_string();
    glib::spawn_future_local(async move {
        let Some(a) = the() else { return };
        let sent = match a.thread.get() {
            Some(id) => ui.call("thread.send", json!({"id": id, "text": text})).await,
            None => {
                let mut create = json!({"kind": "assistant", "text": text});
                let model = a.shown_model.borrow().clone();
                if !model.is_empty() {
                    create["model"] = json!(model);
                }
                let effort = a.shown_effort.borrow().clone();
                if !effort.is_empty() && !is_local(&a, &model) {
                    create["effort"] = json!(effort);
                }
                ui.call("thread.create", create).await.inspect(|thread| {
                    a.thread.set(thread["id"].as_i64());
                    show_title(&a, thread);
                })
            }
        };
        if let Err(e) = sent {
            set_busy(&a, false);
            a.input.buffer().set_text(&text);
            ui.show_error(&format!("The assistant did not get that: {e}"));
            return;
        }
        // A new thread's first message arrived as an event before its id was known here.
        if let Some(id) = a.thread.get() {
            catch_up(id, true).await;
        }
    });
}

fn stop() {
    let Some(a) = the() else { return };
    let (Some(ui), Some(id)) = (a.ui.upgrade(), a.thread.get()) else { return };
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("thread.stop", json!({"id": id})).await {
            ui.show_error(&e.to_string());
        }
    });
}

/// Draw the messages stored after the newest one drawn, going to the newest when `follow` and
/// otherwise only if the reader was there.
async fn catch_up(id: i64, follow: bool) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    let after = a.last.get();
    if let Ok(read) = ui.call("thread.get", json!({"id": id, "after": after})).await {
        if a.thread.get() == Some(id) {
            for m in read["messages"].as_array().into_iter().flatten() {
                draw_message(&a, m);
            }
            // Its `thread.changed` may have come before its id was known here.
            set_busy(&a, read["thread"]["working"] == true);
            stick_to_bottom(&a, follow);
        }
    }
}

fn set_busy(a: &Assistant, busy: bool) {
    a.busy.set(busy);
    if !busy {
        // The reply is over: a later read of it opens nothing.
        a.armed.set(false);
    }
    // The model changes between replies, not during one (`thread.set` refuses a busy thread).
    a.model_key.set_sensitive(!busy);
    a.model_key.set_tooltip_text(Some(if busy { "Waits for this reply to finish" } else { "The model the assistant runs on" }));
    a.send.set_child(Some(&crate::icons::image(if busy { "stop" } else { "arrow-up" }, 16)));
    a.send.set_tooltip_text(Some(if busy { "Stop" } else { "Send (Enter)" }));
    a.send.update_property(&[gtk::accessible::Property::Label(if busy { "Stop" } else { "Send" })]);
    update_send(a);
    if busy {
        if a.working.parent().is_none() {
            a.column.append(&a.working);
        }
    } else {
        if a.working.parent().is_some() {
            a.column.remove(&a.working);
        }
        if a.pending.parent().is_some() {
            a.column.remove(&a.pending);
        }
        a.pending.set_text("");
    }
}

/// The send key: Stop while a reply runs, else Send once there are words to send. The placeholder
/// shows while the box is empty.
fn update_send(a: &Assistant) {
    let buffer = a.input.buffer();
    a.hint.set_visible(buffer.char_count() == 0);
    let written = !buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().is_empty();
    a.send.set_sensitive(a.busy.get() || written);
}

/// Scroll to the newest: on the next frame, and once more after it, when wrapped text and a card
/// filled from a later read have taken their height. Always when `force`, else only when the reader
/// was already at the bottom, so new words never pull them away from what they scrolled up to.
fn stick_to_bottom(a: &Assistant, force: bool) {
    let adj = a.scroll.vadjustment();
    if !force && adj.value() + adj.page_size() < adj.upper() - 48.0 {
        return;
    }
    a.scroll.add_tick_callback(move |_, _| {
        adj.set_value(adj.upper() - adj.page_size());
        let adj = adj.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(120), move || adj.set_value(adj.upper() - adj.page_size()));
        glib::ControlFlow::Break
    });
}

/// Keep the working row last while a turn runs.
fn keep_working_last(a: &Assistant) {
    for w in [a.pending.clone().upcast::<gtk::Widget>(), a.working.clone().upcast()] {
        if w.parent().is_some() {
            a.column.remove(&w);
            a.column.append(&w);
        }
    }
}

fn draw_message(a: &Assistant, m: &Value) {
    let id = m["id"].as_i64().unwrap_or(0);
    if id <= a.last.get() {
        return;
    }
    a.last.set(id);
    a.empty.set_visible(false);
    let body = &m["body"];
    match m["role"].as_str().unwrap_or("") {
        "user" => {
            let bubble = label(body["text"].as_str().unwrap_or(""), "threads-user");
            bubble.set_wrap(true);
            bubble.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            bubble.set_selectable(true);
            bubble.set_focusable(false);
            bubble.set_halign(gtk::Align::End);
            bubble.set_max_width_chars(40);
            a.column.append(&bubble);
        }
        "assistant" => {
            if a.pending.parent().is_some() {
                a.column.remove(&a.pending);
            }
            a.pending.set_text("");
            for block in body["blocks"].as_array().into_iter().flatten() {
                match block["type"].as_str() {
                    Some("text") => {
                        let turn = gtk::Box::new(gtk::Orientation::Vertical, 8);
                        turn.add_css_class("assistant-reply");
                        draw_reply(a, &turn, block["text"].as_str().unwrap_or(""));
                        a.column.append(&turn);
                    }
                    Some("tool_use") => {
                        let name = block["name"].as_str().unwrap_or("");
                        let op = tool_op(name).unwrap_or_default();
                        // The ask shows as its card, from its answer.
                        let line = (op != "thread.ask").then(|| tool_line(&op, &block["input"]));
                        if let Some(line) = &line {
                            a.column.append(line);
                        }
                        if let Some(tool) = block["id"].as_str() {
                            let drawn = line.as_ref().map(|l| l.downgrade()).unwrap_or_default();
                            a.tools.borrow_mut().insert(tool.to_string(), Tool { op, input: block["input"].clone(), line: drawn });
                        }
                    }
                    _ => {}
                }
            }
        }
        "tool" => {
            let tools = a.tools.borrow();
            let tool = body["tool_use_id"].as_str().and_then(|t| tools.get(t));
            let op = tool.map(|t| t.op.as_str()).unwrap_or_default();
            let text = body["text"].as_str().unwrap_or("");
            let failed = body["is_error"] == true;
            if op == "thread.ask" && !failed {
                let ask: Value = serde_json::from_str(text).unwrap_or(Value::Null);
                if let Some(ask_id) = ask["id"].as_i64() {
                    a.column.append(&card(a, ask_id, &ask));
                }
            } else if failed {
                if let Some(line) = tool.and_then(|t| t.line.upgrade()) {
                    line.add_css_class("threads-tool-failed");
                }
                let caption = tool.map_or_else(|| "A tool".to_string(), |t| view::caption(&t.op, &t.input));
                let why = refusal(text);
                let words = if why.is_empty() { format!("{caption}: refused") } else { format!("{caption}: refused. {why}") };
                let line = label(&words, "threads-tool-error");
                line.set_wrap(true);
                line.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                a.column.append(&line);
            } else if let Some(tool) = tool {
                let answer: Value = serde_json::from_str(text).unwrap_or(Value::Null);
                if let (Some(words), Some(line)) = (view::done_caption(&tool.op, &tool.input, &answer), tool.line.upgrade()) {
                    if let Some(caption) = line.last_child().and_downcast::<gtk::Label>() {
                        caption.set_text(&words);
                    }
                }
            }
        }
        "error" => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("threads-error-row");
            row.append(&crate::icons::image("close", 13));
            let text = label(body["text"].as_str().unwrap_or("Something went wrong"), "threads-error");
            text.set_wrap(true);
            text.set_hexpand(true);
            text.set_selectable(true);
            text.set_focusable(false);
            row.append(&text);
            a.column.append(&row);
        }
        _ => {}
    }
    keep_working_last(a);
}

/// A refused call's reason, one line of it: the bus's own words, without its code or hint.
fn refusal(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first = first.strip_prefix("Not done").map(|rest| rest.trim_start_matches(|c: char| c != ':').trim_start_matches(':').trim()).unwrap_or(first);
    let first = first.split(" Hint:").next().unwrap_or(first);
    let mut out: String = first.chars().take(200).collect();
    if first.chars().count() > 200 {
        out.push('\u{2026}');
    }
    out
}

/// A reply's words, with its cards and the pages it opens in their places.
fn draw_reply(a: &Assistant, turn: &gtk::Box, text: &str) {
    for part in view::parts(text) {
        match part {
            Part::Text(markdown) => crate::money::threads::draw_reply(turn, &markdown),
            Part::Cards { kind, items } => {
                let cards = gtk::Box::new(gtk::Orientation::Vertical, 6);
                for item in items {
                    cards.append(&live_card(a, kind, &item));
                }
                turn.append(&cards);
            }
            Part::Open(line) => turn.append(&open_line(a, &line)),
        }
    }
}

/// A tool call, as a quiet line saying what the agent looked at or did.
fn tool_line(op: &str, input: &Value) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("threads-tool");
    line.append(&crate::icons::image(if view::writes(op) { "check" } else { "search" }, 13));
    let words = label(&view::caption(op, input), "threads-tool-text");
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&words);
    line
}

/// An `open` block: where it goes, as a key that goes there again, and the page itself, at once,
/// when this reply answers a message sent from here in this run.
fn open_line(a: &Assistant, line: &str) -> gtk::Widget {
    let Some(target) = view::target(line) else {
        let words = label(&format!("Could not open \u{201c}{line}\u{201d}: not a page Relay has"), "threads-tool-error");
        words.set_wrap(true);
        return words.upcast();
    };
    let acted = a.armed.get();
    let key = button("", "assistant-open-key");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&crate::icons::image("external", 13));
    let words = view::target_words(&target);
    row.append(&label(&if acted { format!("Opened {words}") } else { format!("Open {words}") }, ""));
    key.set_child(Some(&row));
    key.set_halign(gtk::Align::Start);
    key.set_tooltip_text(Some(&format!("Open {words}")));
    let again = target.clone();
    key.connect_clicked(move |_| go(&again));
    if acted {
        // After this frame's drawing: going there may close the drawer or switch the project.
        glib::idle_add_local_once(move || go(&target));
    }
    key.upcast()
}

/// Go where an `open` block says. A task's page covers the window, so the drawer makes way for
/// it; a page or a session's terminal shows beside the drawer.
fn go(target: &Target) {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    match target.clone() {
        Target::Page { page, project } => match project {
            Some(project) if project != ui.project.get() => ui.open_project(project, page),
            _ => ui.navigate(page),
        },
        Target::Project(project) => ui.switch_project(project),
        Target::Task(id) => {
            show(false);
            crate::pages::open_task(&ui, id);
        }
        Target::Session(name) => {
            glib::spawn_future_local(async move {
                match ui.call("session.get", json!({"session": name})).await {
                    Ok(session) => {
                        let project = session["project_id"].as_i64().unwrap_or(ui.project.get());
                        ui.open_project(project, "agents");
                        ui.focus_session(&name);
                    }
                    Err(e) => ui.show_error(&format!("Could not show {name}: {e}")),
                }
            });
        }
        Target::Note(id) => {
            glib::spawn_future_local(async move {
                match ui.call("notes.get", json!({"note_id": id})).await {
                    Ok(note) => crate::pages::open_note(&ui, note),
                    Err(e) => ui.show_error(&format!("Could not open note {id}: {e}")),
                }
            });
        }
    }
}

/// A card of a task, a session or a note, read from Relay now and following its events; a click
/// opens it.
fn live_card(a: &Assistant, kind: &'static str, item: &str) -> gtk::Button {
    let key = button("", "assistant-card");
    key.add_css_class("threads-card");
    key.set_widget_name(&format!("assistant-card-{kind}"));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.append(&label(&format!("Reading {kind} {item}\u{2026}"), "threads-card-detail"));
    key.set_child(Some(&body));
    let target = match kind {
        "task" => item.parse().ok().map(Target::Task),
        "note" => item.parse().ok().map(Target::Note),
        _ => Some(Target::Session(item.to_string())),
    };
    match target {
        Some(target) => {
            key.connect_clicked(move |_| go(&target));
        }
        None => key.set_sensitive(false),
    }
    a.live.borrow_mut().push(Card { kind, item: item.to_string(), key: key.downgrade() });
    read_card(kind, item.to_string(), key.downgrade());
    key
}

/// Read a card's row from Relay and draw it.
fn read_card(kind: &'static str, item: String, key: glib::WeakRef<gtk::Button>) {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    glib::spawn_future_local(async move {
        let read = match (kind, item.parse::<i64>()) {
            ("task", Ok(id)) => ui.call("task.get", json!({"task_id": id})).await.ok(),
            ("note", Ok(id)) => ui.call("notes.get", json!({"note_id": id})).await.ok(),
            ("session", _) => ui.call("session.get", json!({"session": item})).await.ok(),
            _ => None,
        };
        if let Some(key) = key.upgrade() {
            fill_card(&ui, &key, kind, &item, read.as_ref());
        }
    });
}

/// Draw a card from its row; none is a card saying it is not there.
fn fill_card(ui: &Ui, key: &gtk::Button, kind: &str, item: &str, row: Option<&Value>) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let Some(row) = row.filter(|r| r.is_object() && r["deleted_at"].is_null()) else {
        let what = match kind {
            "task" => format!("Task #{item} is not on the board"),
            "note" => format!("Note {item} is not there"),
            _ => format!("No session named {item}"),
        };
        body.append(&label(&what, "threads-card-detail"));
        key.set_child(Some(&body));
        key.set_sensitive(false);
        return;
    };
    key.set_sensitive(true);
    let project = |id: &Value| ui.projects.borrow().iter().find(|p| &p["id"] == id).and_then(|p| p["name"].as_str().map(str::to_string));
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    match kind {
        "task" => {
            // As the board shows it.
            let id = row["id"].as_i64().unwrap_or_default();
            let key = project(&row["project_id"]).map_or_else(|| format!("#{id}"), |name| relay_board::task_key(&name, id));
            top.append(&label(&key, "assistant-card-id"));
            top.append(&pill(view::column_words(row["column"].as_str().unwrap_or("")), "quiet"));
            let state = row["state"].as_str().unwrap_or("none");
            if state != "none" {
                let tone = match state {
                    "running" | "dispatched" => "done",
                    "blocked" | "awaiting_review" => "waiting",
                    "failed" => "failed",
                    _ => "quiet",
                };
                top.append(&pill(&state.replace('_', " "), tone));
            }
        }
        "session" => {
            let state = row["state"].as_str().unwrap_or("");
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("assistant-card-dot");
            dot.add_css_class(&format!("assistant-card-dot-{}", match state {
                "running" | "spawning" => "live",
                "idle" => "idle",
                "blocked" => "waiting",
                "exited" | "closed" => "held",
                _ => "quiet",
            }));
            dot.set_valign(gtk::Align::Center);
            top.append(&dot);
            top.append(&label(row["name"].as_str().unwrap_or(item), "threads-card-title"));
            top.append(&pill(state, "quiet"));
        }
        _ => {
            top.append(&crate::icons::image("notes", 13));
            let title = label(row["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Untitled note"), "threads-card-title");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            top.append(&title);
        }
    }
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_hexpand(true);
    top.append(&gap);
    top.append(&crate::icons::image("chevron-right", 12));
    body.append(&top);
    let wrapped = |text: &str, class: &str, lines: i32| {
        let l = label(text, class);
        l.set_wrap(true);
        l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        l.set_lines(lines);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        l
    };
    match kind {
        "task" => {
            body.append(&wrapped(row["title"].as_str().unwrap_or(""), "threads-card-title", 3));
            let mut detail: Vec<String> = project(&row["project_id"]).into_iter().collect();
            if let Some(priority) = row["priority"].as_str().filter(|p| matches!(*p, "high" | "urgent")) {
                detail.push(format!("{priority} priority"));
            }
            let sessions: Vec<&str> = row["sessions"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !sessions.is_empty() {
                detail.push(sessions.join(", "));
            }
            if !detail.is_empty() {
                body.append(&wrapped(&detail.join(" \u{b7} "), "threads-card-detail", 1));
            }
        }
        "session" => {
            let mut detail: Vec<String> = [row["role"].as_str(), row["provider"].as_str()].into_iter().flatten().map(str::to_string).collect();
            detail.extend(project(&row["project_id"]));
            body.append(&wrapped(&detail.join(" \u{b7} "), "threads-card-detail", 1));
            if let Some(intent) = row["intent"].as_str().filter(|i| !i.trim().is_empty()) {
                body.append(&wrapped(intent, "assistant-card-text", 2));
            }
        }
        _ => {
            let first = row["body"].as_str().unwrap_or("").lines().map(|l| l.trim().trim_start_matches(['#', '-', '*', '>', ' '])).find(|l| !l.is_empty());
            body.append(&wrapped(first.unwrap_or("Empty"), "assistant-card-text", 2));
            if let Some(name) = project(&row["project_id"]) {
                body.append(&wrapped(&name, "threads-card-detail", 1));
            }
        }
    }
    key.set_child(Some(&body));
    let words = match kind {
        "task" => format!("Open task #{}", row["id"]),
        "session" => format!("Show {}", row["name"].as_str().unwrap_or(item)),
        _ => "Open the note".to_string(),
    };
    key.set_tooltip_text(Some(&words));
    key.update_property(&[gtk::accessible::Property::Label(&words)]);
}

fn pill(text: &str, tone: &str) -> gtk::Label {
    let mark = label(text, "assistant-pill");
    mark.add_css_class(&format!("assistant-pill-{tone}"));
    mark
}

/// Read every card on screen again (the drawer reopened: their rows may have moved unseen).
fn refresh_cards() {
    let Some(a) = the() else { return };
    a.live.borrow_mut().retain(|c| c.key.upgrade().is_some());
    for card in a.live.borrow().iter() {
        read_card(card.kind, card.item.clone(), card.key.clone());
    }
}

/// A task, session or note event: the cards of that row redraw. A task's and a session's event
/// carries the whole row, so nothing is read; a note's leaves out its body, and a deletion says
/// nothing of the row, so those read it again.
pub fn data_changed(ev: &str, payload: &Value) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    let (kind, item) = match ev {
        "task.changed" | "task.deleted" => ("task", payload["id"].as_i64().or_else(|| payload["task_id"].as_i64()).map(|id| id.to_string())),
        "notes.changed" | "notes.deleted" => ("note", payload["id"].as_i64().or_else(|| payload["note_id"].as_i64()).map(|id| id.to_string())),
        "session.changed" => ("session", payload["name"].as_str().map(str::to_string)),
        _ => return,
    };
    let Some(item) = item else { return };
    let whole = !ev.ends_with(".deleted") && payload.get(match kind { "session" => "state", "note" => "body", _ => "title" }).is_some();
    let keys: Vec<gtk::Button> = a.live.borrow().iter().filter(|c| c.kind == kind && c.item == item).filter_map(|c| c.key.upgrade()).collect();
    for key in keys {
        if whole {
            fill_card(&ui, &key, kind, &item, Some(payload));
        } else {
            read_card(kind, item.clone(), key.downgrade());
        }
    }
}

/// An ask's card: what approving does, why, its steps, and the keys or what came of it.
fn card(a: &Assistant, id: i64, ask: &Value) -> gtk::Box {
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    a.cards.borrow_mut().entry(id).or_default().push(slot.downgrade());
    fill(&slot, ask);
    slot
}

fn fill(slot: &gtk::Box, ask: &Value) {
    clear(slot);
    let status = ask["status"].as_str().unwrap_or("pending");
    let auto = ask["auto"] == true;
    let id = ask["id"].as_i64().unwrap_or(0);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("threads-card");
    card.add_css_class("assistant-ask");
    if status == "pending" {
        card.add_css_class("assistant-ask-waiting");
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let (words, tone) = match (status, auto) {
        ("pending", _) => ("Needs your approval", "waiting"),
        ("running", _) => ("Running", "waiting"),
        ("approved", true) => ("Done without asking", "done"),
        ("approved", false) => ("Done", "done"),
        ("declined", _) => ("Not done", "quiet"),
        _ => ("Failed", "failed"),
    };
    top.append(&pill(words, tone));
    card.append(&top);
    let title = label(ask["summary"].as_str().unwrap_or(""), "threads-card-title");
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    card.append(&title);
    if let Some(why) = ask["why"].as_str().filter(|w| !w.is_empty()) {
        let quoted = label(&format!("\u{201c}{why}\u{201d}"), "threads-ask-why");
        quoted.set_wrap(true);
        card.append(&quoted);
    }
    // Exactly what runs: each step's op and payload, as the engine will run them.
    let steps = gtk::Box::new(gtk::Orientation::Vertical, 4);
    steps.add_css_class("assistant-steps");
    for (n, step) in ask["steps"].as_array().into_iter().flatten().enumerate() {
        let line = label(&format!("{}. {} {}", n + 1, step["op"].as_str().unwrap_or(""), compact(&step["payload"])), "assistant-step");
        line.set_wrap(true);
        line.set_wrap_mode(gtk::pango::WrapMode::Char);
        line.set_selectable(true);
        line.set_focusable(false);
        steps.append(&line);
    }
    card.append(&steps);
    match status {
        "pending" => {
            let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            keys.set_halign(gtk::Align::End);
            let decline = button("Not now", "quiet");
            decline.add_css_class("threads-card-key");
            let approve = button("Approve", "primary");
            approve.add_css_class("threads-card-key");
            for (key, yes) in [(&decline, false), (&approve, true)] {
                // Weak: a key holding itself in its own handler would outlive its card.
                let pair = [approve.downgrade(), decline.downgrade()];
                key.connect_clicked(move |_| answer(id, yes, pair.iter().filter_map(|k| k.upgrade()).collect()));
            }
            keys.append(&decline);
            keys.append(&approve);
            card.append(&keys);
        }
        "running" => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&crate::app::spinner());
            row.append(&label("Running its steps…", "threads-card-detail"));
            card.append(&row);
        }
        _ => {
            if let Some(outcome) = ask["outcome"].as_str() {
                let words = label(outcome, if status == "failed" { "threads-ask-gone" } else { "threads-card-detail" });
                words.set_wrap(true);
                words.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                card.append(&words);
            }
        }
    }
    if auto {
        // Said, with the way back to asking first.
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let note = label("Ran without asking, as you allow.", "threads-card-detail");
        note.set_wrap(true);
        row.append(&note);
        let change = button("Change", "assistant-link");
        change.set_tooltip_text(Some("What the assistant does without asking"));
        change.connect_clicked(|_| {
            if let Some(a) = the() {
                a.settings_key.popup();
            }
        });
        row.append(&change);
        card.append(&row);
    }
    slot.append(&card);
}

/// A payload on one line: `key: value` pairs, strings unquoted and every control character
/// shown, as the engine's summary shows them (`thread::shown`).
fn compact(payload: &Value) -> String {
    let Some(fields) = payload.as_object() else { return payload.to_string() };
    fields.iter().map(|(k, v)| {
        let v = match v {
            Value::String(s) => relay_bus::ops::thread::shown(s),
            other => other.to_string(),
        };
        format!("{k}: {v}")
    }).collect::<Vec<_>>().join(" · ")
}

fn answer(id: i64, approve: bool, keys: Vec<gtk::Button>) {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    for key in &keys {
        key.set_sensitive(false);
    }
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("thread.ask.resolve", json!({"id": id, "approve": approve})).await {
            for key in &keys {
                key.set_sensitive(true);
            }
            ui.show_error(&format!("That answer did not reach Relay: {e}"));
        }
        // The card follows the `thread.ask` events; read once more in case one was missed.
        refresh_asks().await;
    });
}

/// Read the shown thread's asks again and redraw their cards.
async fn refresh_asks() {
    let Some(a) = the() else { return };
    let (Some(ui), Some(id)) = (a.ui.upgrade(), a.thread.get()) else { return };
    if let Ok(read) = ui.call("thread.ask.list", json!({"thread_id": id})).await {
        for ask in read["asks"].as_array().into_iter().flatten() {
            changed(&a, ask);
        }
    }
}

fn changed(a: &Assistant, ask: &Value) {
    let Some(id) = ask["id"].as_i64() else { return };
    let slots: Vec<gtk::Box> = a.cards.borrow().get(&id).into_iter().flatten().filter_map(|w| w.upgrade()).collect();
    for slot in slots {
        fill(&slot, ask);
    }
}

/// The history menu: every assistant chat, newest first, the one shown marked; each opens with a
/// click and is deleted with a second click on its bin.
fn fill_history() {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    glib::spawn_future_local(async move {
        let read = ui.call("thread.list", json!({"kind": "assistant"})).await;
        let Some(a) = the() else { return };
        clear(&a.history_list);
        a.history_list.append(&label("Earlier chats", "threads-menu-heading"));
        let threads = match read {
            Ok(read) => read["threads"].as_array().cloned().unwrap_or_default(),
            Err(e) => {
                a.history_list.append(&label(&format!("Could not read them: {e}"), "threads-menu-idle"));
                return;
            }
        };
        if threads.is_empty() {
            a.history_list.append(&label("No chats yet", "threads-menu-idle"));
        }
        for thread in threads {
            let Some(id) = thread["id"].as_i64() else { continue };
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            let open_key = button("", "nav");
            open_key.add_css_class("assistant-history-row");
            open_key.set_hexpand(true);
            if a.thread.get() == Some(id) {
                open_key.add_css_class("assistant-history-current");
            }
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            let title = label(thread["title"].as_str().unwrap_or("Chat"), "assistant-history-title");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_width_chars(30);
            title.set_max_width_chars(30);
            words.append(&title);
            let when = thread["updated_at"].as_str().and_then(|t| crate::relative::ago(t, crate::relative::Form::Long)).unwrap_or_default();
            let model = model_caption(&a, thread["model"].as_str().unwrap_or(""));
            let detail = [when, model].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" \u{b7} ");
            words.append(&label(&detail, "assistant-history-detail"));
            open_key.set_child(Some(&words));
            open_key.connect_clicked(move |key| {
                if let Some(popover) = key.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                    popover.popdown();
                }
                glib::spawn_future_local(open(id));
            });
            row.append(&open_key);
            let bin = icon_button("trash", "Delete this chat");
            bin.set_valign(gtk::Align::Center);
            bin.connect_clicked(move |bin| {
                // The first click asks; the second deletes.
                if !bin.has_css_class("assistant-bin-armed") {
                    bin.add_css_class("assistant-bin-armed");
                    bin.set_tooltip_text(Some("Click again to delete it"));
                    return;
                }
                delete(id);
            });
            row.append(&bin);
            a.history_list.append(&row);
        }
    });
}

/// Delete chat `id`; the drawer starts a new one when it was the one shown.
fn delete(id: i64) {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    glib::spawn_future_local(async move {
        if let Err(e) = ui.call("thread.delete", json!({"id": id})).await {
            return ui.show_error(&format!("The chat was not deleted: {e}"));
        }
        if the().is_some_and(|a| a.thread.get() == Some(id)) {
            new_chat();
        }
        fill_history();
    });
}

/// Set the switches from `threads.assistant.without_asking`.
fn read_allowed() {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    glib::spawn_future_local(async move {
        let Ok(read) = ui.call("settings.get", json!({"path": "threads.assistant.without_asking"})).await else { return };
        let Some(a) = the() else { return };
        a.filling.set(true);
        for (kind, check) in &a.allow {
            check.set_active(read["value"][*kind] == true);
        }
        a.filling.set(false);
    });
}

/// A switch flipped by the person: kept, or put back with the reason when Relay refuses.
fn allow_changed(kind: &'static str, on: bool) {
    let Some(a) = the() else { return };
    if a.filling.get() {
        return;
    }
    let Some(ui) = a.ui.upgrade() else { return };
    glib::spawn_future_local(async move {
        let path = format!("threads.assistant.without_asking.{kind}");
        if let Err(e) = ui.call("settings.set", json!({"path": path, "value": on})).await {
            ui.show_error(&format!("That was not kept: {e}"));
            read_allowed();
        }
    });
}

/// The picker's models: Claude's, then the models on this PC that can run, each with its check.
fn fill_models(a: &Assistant) {
    clear(&a.model_list);
    a.marks.borrow_mut().clear();
    let popover = a.model_key.popover();
    let add = |list: &gtk::Box, id: &str, caption: &str, tip: Option<&str>| {
        let item = button("", "nav");
        item.set_widget_name(&format!("assistant-model-{}", if id.is_empty() { "default" } else { id }));
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let mark = crate::icons::image("check", 13);
        line.append(&mark);
        a.marks.borrow_mut().push((id.to_string(), mark));
        line.append(&label(caption, ""));
        item.set_child(Some(&line));
        item.set_tooltip_text(tip);
        let (id, popover) = (id.to_string(), popover.clone());
        item.connect_clicked(move |_| {
            if let Some(p) = &popover {
                p.popdown();
            }
            choose(&id);
        });
        list.append(&item);
    };
    a.model_list.append(&label("Claude", "threads-menu-heading"));
    for (id, caption) in MODELS {
        add(&a.model_list, id, caption, None);
    }
    let local = a.local.borrow();
    a.model_list.append(&label("On this PC", "threads-menu-heading"));
    if local.is_empty() {
        let none = label("No model files found. Threads' settings say where they go.", "threads-menu-idle");
        none.set_wrap(true);
        none.set_max_width_chars(30);
        a.model_list.append(&none);
    }
    for (id, name) in local.iter() {
        add(&a.model_list, id, name, Some("Runs on your graphics card with a shorter prompt and fewer tools: it loads when picked"));
    }
    drop(local);
    show_model(a);
}

/// The picker's efforts, for Claude's models.
fn fill_efforts(a: &Assistant) {
    clear(&a.effort_box);
    a.effort_marks.borrow_mut().clear();
    a.effort_box.append(&label("Effort", "threads-menu-heading"));
    let popover = a.model_key.popover();
    for (value, caption) in EFFORTS {
        let item = button("", "nav");
        item.set_widget_name(&format!("assistant-effort-{}", if value.is_empty() { "default" } else { value }));
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let mark = crate::icons::image("check", 13);
        line.append(&mark);
        a.effort_marks.borrow_mut().push((value.to_string(), mark));
        line.append(&label(caption, ""));
        item.set_child(Some(&line));
        let popover = popover.clone();
        item.connect_clicked(move |_| {
            if let Some(p) = &popover {
                p.popdown();
            }
            choose_effort(value);
        });
        a.effort_box.append(&item);
    }
    show_model(a);
}

fn is_local(a: &Assistant, model: &str) -> bool {
    relay_client::models::is_local(model, a.local.borrow().iter().map(|(id, _)| id.as_str()))
}

/// A model's caption: Claude's name for it, the local model's, or its id.
fn model_caption(a: &Assistant, model: &str) -> String {
    MODELS.iter().find(|(id, _)| *id == model).map(|(_, c)| c.to_string())
        .or_else(|| a.local.borrow().iter().find(|(id, _)| id == model).map(|(_, n)| n.clone()))
        .unwrap_or_else(|| model.to_string())
}

/// The caption and checks for the model and effort shown; a local one says while it loads, and
/// has no effort.
fn show_model(a: &Assistant) {
    let model = a.shown_model.borrow().clone();
    let effort = a.shown_effort.borrow().clone();
    let local = is_local(a, &model);
    let mut caption = model_caption(a, &model);
    if local {
        let state = a.local_state.borrow();
        if state.0 == model && state.1 == "loading" {
            caption.push_str(" \u{b7} loading");
        }
    } else if let Some((_, words)) = EFFORTS.iter().find(|(value, _)| !value.is_empty() && *value == effort) {
        caption.push_str(&format!(" \u{b7} {words}"));
    }
    a.model_shown.set_text(&caption);
    for (id, mark) in a.marks.borrow().iter() {
        mark.set_opacity(if *id == model { 1.0 } else { 0.0 });
    }
    for (value, mark) in a.effort_marks.borrow().iter() {
        mark.set_opacity(if *value == effort { 1.0 } else { 0.0 });
    }
    a.effort_box.set_visible(!local);
}

/// The models on this PC that can run (`thread.models`), and the one loaded now.
fn load_local() {
    let Some(ui) = the().and_then(|a| a.ui.upgrade()) else { return };
    glib::spawn_future_local(async move {
        let Ok(found) = ui.call("thread.models", json!({})).await else { return };
        let Some(a) = the() else { return };
        let models: Vec<(String, String)> = found["models"].as_array().into_iter().flatten()
            .filter(|m| m["present"] == true)
            .filter_map(|m| Some((m["id"].as_str()?.to_string(), m["name"].as_str()?.to_string())))
            .collect();
        if let Some(running) = found["running"].as_str() {
            *a.local_state.borrow_mut() = (running.to_string(), if found["ready"] == true { "ready" } else { "loading" }.to_string());
        }
        if *a.local.borrow() != models {
            *a.local.borrow_mut() = models;
            fill_models(&a);
        } else {
            show_model(&a);
        }
    });
}

/// The last model and effort picked, from the last run (`native.assistant`).
fn restore_choice(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let kept = ui.call("settings.get", json!({"path": "native.assistant"})).await.map(|v| v["value"].clone()).unwrap_or_default();
        let Some(a) = the() else { return };
        if let Some(model) = kept["model"].as_str().filter(|m| m.len() <= 64) {
            *a.choice.borrow_mut() = model.to_string();
        }
        if let Some(effort) = kept["effort"].as_str().filter(|e| EFFORTS.iter().any(|(value, _)| value == e)) {
            *a.effort_choice.borrow_mut() = effort.to_string();
        }
        if a.thread.get().is_none() {
            *a.shown_model.borrow_mut() = a.choice.borrow().clone();
            *a.shown_effort.borrow_mut() = a.effort_choice.borrow().clone();
            show_model(&a);
        }
    });
}

/// Move the conversation shown to `model` and `effort` (`thread.set`; its next message resumes
/// on them); refused, the picker says what it kept.
fn set_shown(a: &Assistant, model: String, effort: String, was: (String, String)) {
    let (Some(ui), Some(id)) = (a.ui.upgrade(), a.thread.get()) else { return };
    glib::spawn_future_local(async move {
        let mut set = json!({"id": id, "model": model});
        if !effort.is_empty() {
            set["effort"] = json!(effort);
        }
        if let Err(e) = ui.call("thread.set", set).await {
            if let Some(a) = the().filter(|a| a.thread.get() == Some(id) && *a.shown_model.borrow() == model && *a.shown_effort.borrow() == effort) {
                *a.shown_model.borrow_mut() = was.0;
                *a.shown_effort.borrow_mut() = was.1;
                show_model(&a);
            }
            ui.show_error(&format!("The assistant kept its model: {e}"));
        }
    });
}

/// A model picked: new chats start on it, in this run and the next, and the conversation shown
/// moves to it. A local model starts loading at once.
fn choose(model: &str) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    *a.choice.borrow_mut() = model.to_string();
    let was = (a.shown_model.replace(model.to_string()), a.shown_effort.borrow().clone());
    show_model(&a);
    let local = is_local(&a, model);
    // A local model has no effort: the conversation keeps none while on it.
    let effort = if local { String::new() } else { a.shown_effort.borrow().clone() };
    set_shown(&a, model.to_string(), effort, was);
    let model = model.to_string();
    glib::spawn_future_local(async move {
        let _ = ui.call("settings.set", json!({"path": "native.assistant.model", "value": model})).await;
        // Only a local pick goes to `thread.pick`: picking Claude there would stop a local model
        // a Threads conversation may be using.
        if local {
            if let Err(e) = ui.call("thread.pick", json!({"model": model})).await {
                ui.show_error(&e.to_string());
            }
        }
    });
}

/// An effort picked, for Claude's models: new chats start on it, and the conversation shown
/// moves to it.
fn choose_effort(effort: &str) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    *a.effort_choice.borrow_mut() = effort.to_string();
    let was = (a.shown_model.borrow().clone(), a.shown_effort.replace(effort.to_string()));
    show_model(&a);
    set_shown(&a, was.0.clone(), effort.to_string(), was.clone());
    let effort = effort.to_string();
    glib::spawn_future_local(async move {
        let _ = ui.call("settings.set", json!({"path": "native.assistant.effort", "value": effort})).await;
    });
}

/// A `thread.*` event: `true` when it was the assistant's, which Threads then does not see.
pub fn event(_ui: &Rc<Ui>, ev: &str, payload: &Value) -> bool {
    let Some(a) = the() else { return false };
    // The local model server's state is Threads' too: shown here, and passed on.
    if ev == "thread.local" {
        *a.local_state.borrow_mut() = (payload["model"].as_str().unwrap_or("").to_string(), payload["state"].as_str().unwrap_or("").to_string());
        show_model(&a);
        return false;
    }
    let thread = payload["thread"].as_i64().or_else(|| payload["thread"]["id"].as_i64()).or_else(|| payload["id"].as_i64());
    let ours = ev == "thread.ask" || payload["thread"]["kind"] == "assistant" || (thread.is_some() && thread == a.thread.get());
    if !ours {
        return false;
    }
    let shown = thread.is_some() && thread == a.thread.get();
    match ev {
        "thread.delta" if shown => {
            a.pending.set_text(&format!("{}{}", a.pending.text(), payload["text"].as_str().unwrap_or("")));
            if a.pending.parent().is_none() {
                a.column.append(&a.pending);
            }
            keep_working_last(&a);
            stick_to_bottom(&a, false);
        }
        "thread.message" if shown => {
            draw_message(&a, &payload["message"]);
            stick_to_bottom(&a, false);
        }
        "thread.changed" if shown => {
            if payload["deleted"] == true {
                reset(None);
            } else {
                set_busy(&a, payload["thread"]["working"] == true);
                show_title(&a, &payload["thread"]);
            }
        }
        "thread.ask" => changed(&a, &payload["ask"]),
        _ => {}
    }
    true
}
