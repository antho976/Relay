//! The assistant (docs/THREADS.md, "The assistant"): a drawer the title bar's key opens over the
//! Dev space, holding a conversation with an agent about Relay itself.
//!
//! The conversation is a thread of the `assistant` kind, kept by the engine like any thread, so
//! the drawer reopens on the newest one without asking the agent anything. The agent reads freely
//! and may mail sessions; what a lone session may not do it asks for, and each ask shows here as a
//! card the person approves or declines (`thread.ask.*`). The card's state is the engine's, read
//! again when the thread opens and on every `thread.ask`, so an answer stays answered.
//!
//! The agent acts only on what the person writes here: nothing starts it but a message.
use crate::app::{button, clear, icon_button, label, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::thread_view::tool_op;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// The drawer's width, in px.
const WIDTH: i32 = 420;

/// The Claude models the assistant may run on: (id, caption). An empty id is Claude's default.
/// The models on this PC follow them in the picker (`thread.models`).
const MODELS: [(&str, &str); 5] = [
    ("", "Default model"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
];

/// The message box's placeholder.
const HINT: &str = "Ask about your sessions, tasks or projects";

/// What the empty drawer offers to ask.
const SUGGESTIONS: [&str; 3] = [
    "What are my sessions doing right now?",
    "Which sessions are idle, and should any be parked?",
    "Is anything waiting on me?",
];

struct Assistant {
    ui: Weak<Ui>,
    /// The title bar's key, lit while the drawer is open.
    key: gtk::Button,
    revealer: gtk::Revealer,
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
    /// Each choice's check, by model id.
    marks: RefCell<Vec<(String, gtk::Image)>>,
    /// The model a new chat starts on: the last one picked, kept across runs.
    choice: RefCell<String>,
    /// The model of the conversation shown (a new chat's is `choice`).
    shown_model: RefCell<String>,
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
    /// The op each tool call ran, by its id, so its answer knows what it answers.
    tools: RefCell<HashMap<String, String>>,
    /// The ask cards on screen, by ask id.
    cards: RefCell<HashMap<i64, Vec<glib::WeakRef<gtk::Box>>>>,
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

/// Build the drawer over `host` and wire the title bar's `key` to open and close it.
pub fn install(ui: &Rc<Ui>, key: &gtk::Button, host: &gtk::Overlay) {
    let drawer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    drawer.add_css_class("assistant-drawer");
    drawer.set_widget_name("assistant");
    drawer.set_size_request(WIDTH, -1);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.add_css_class("assistant-head");
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
    titles.set_hexpand(true);
    titles.append(&label("Assistant", "assistant-title"));
    titles.append(&label("Reads Relay freely. Asks you before it changes anything.", "assistant-subtitle"));
    head.append(&titles);
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
    model_row.append(&model_shown);
    model_row.append(&crate::icons::image("chevron-down", 10));
    model_key.set_child(Some(&model_row));
    let model_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    model_list.add_css_class("threads-menu");
    let model_popover = gtk::Popover::new();
    model_popover.set_child(Some(&model_list));
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
        choice: RefCell::default(),
        shown_model: RefCell::default(),
        local: RefCell::default(),
        local_state: RefCell::default(),
        working,
        pending,
        thread: Cell::new(None),
        last: Cell::new(0),
        busy: Cell::new(false),
        tools: RefCell::default(),
        cards: RefCell::default(),
        loaded: Cell::new(false),
        before: RefCell::default(),
    });
    ASSISTANT.with(|slot| *slot.borrow_mut() = Some(a.clone()));
    fill_models(&a);
    restore_choice(ui);

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
        "Ask about your sessions, tasks and projects, or have it write to a session. Launching, parking, closing \
         or typing into a session, and changing tasks, workspaces or projects, wait for your approval on a card.",
        "assistant-intro",
    );
    intro.set_wrap(true);
    empty.append(&intro);
    for words in SUGGESTIONS {
        let chip = button(words, "assistant-suggestion");
        if let Some(l) = chip.child().and_downcast::<gtk::Label>() {
            l.set_xalign(0.0);
            l.set_wrap(true);
        }
        chip.connect_clicked(move |_| send(words));
        empty.append(&chip);
    }
    empty
}

fn toggle() {
    if let Some(a) = the() {
        show(!a.revealer.reveals_child());
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
    show_model(&a);
    for m in read["messages"].as_array().into_iter().flatten() {
        draw_message(&a, m);
    }
    set_busy(&a, read["thread"]["working"] == true);
    refresh_asks().await;
    stick_to_bottom(&a, true);
}

/// Clear the drawer for thread `id` (none: a new chat).
fn reset(id: Option<i64>) {
    let Some(a) = the() else { return };
    a.thread.set(id);
    a.last.set(0);
    a.tools.borrow_mut().clear();
    a.cards.borrow_mut().clear();
    clear(&a.column);
    a.pending.set_text("");
    a.column.append(&a.empty);
    a.empty.set_visible(id.is_none());
    set_busy(&a, false);
}

fn new_chat() {
    reset(None);
    if let Some(a) = the() {
        let choice = a.choice.borrow().clone();
        *a.shown_model.borrow_mut() = choice;
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
    a.empty.set_visible(false);
    let text = text.to_string();
    glib::spawn_future_local(async move {
        let Some(a) = the() else { return };
        let sent = match a.thread.get() {
            Some(id) => ui.call("thread.send", json!({"id": id, "text": text})).await,
            None => {
                let model = a.shown_model.borrow().clone();
                let mut create = json!({"kind": "assistant", "text": text});
                if !model.is_empty() {
                    create["model"] = json!(model);
                }
                ui.call("thread.create", create).await.inspect(|thread| {
                    a.thread.set(thread["id"].as_i64());
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
                        crate::money::threads::draw_reply(&turn, block["text"].as_str().unwrap_or(""));
                        a.column.append(&turn);
                    }
                    Some("tool_use") => {
                        let name = block["name"].as_str().unwrap_or("");
                        let op = tool_op(name).unwrap_or_default();
                        if let Some(tool) = block["id"].as_str() {
                            a.tools.borrow_mut().insert(tool.to_string(), op.clone());
                        }
                        // The ask shows as its card, from its answer.
                        if op != "thread.ask" {
                            a.column.append(&tool_line(&op));
                        }
                    }
                    _ => {}
                }
            }
        }
        "tool" => {
            let op = body["tool_use_id"].as_str().and_then(|t| a.tools.borrow().get(t).cloned()).unwrap_or_default();
            let failed = body["is_error"] == true;
            if op == "thread.ask" && !failed {
                let ask: Value = serde_json::from_str(body["text"].as_str().unwrap_or("")).unwrap_or(Value::Null);
                if let Some(ask_id) = ask["id"].as_i64() {
                    a.column.append(&card(a, ask_id, &ask));
                }
            } else if failed {
                let line = label(&format!("{} was refused", caption(&op)), "threads-tool-error");
                line.set_wrap(true);
                a.column.append(&line);
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

/// A tool call, as a quiet line saying what the agent looked at or did.
fn tool_line(op: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("threads-tool");
    let writes = matches!(op, "mailbox.send" | "notes.append" | "task.comment");
    line.append(&crate::icons::image(if writes { "check" } else { "search" }, 13));
    let words = label(&caption(op), "threads-tool-text");
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&words);
    line
}

/// What a tool call did, in words.
fn caption(op: &str) -> String {
    match op {
        "session.list" | "session.peers" => "Read sessions".into(),
        "session.get" => "Read a session".into(),
        "session.transcript" => "Read a session's conversation".into(),
        "session.scrollback" => "Read a session's terminal".into(),
        "session.restorable" => "Read restorable sessions".into(),
        "task.list" | "task.children" => "Read the board".into(),
        "task.get" => "Read a task".into(),
        "project.list" | "project.get" | "project.stats" => "Read projects".into(),
        "workspace.list" | "workspace.discover" => "Read workspaces".into(),
        "mailbox.list" | "mailbox.outbox" => "Read mail".into(),
        "mailbox.send" => "Sent mail".into(),
        "notes.append" => "Added to a note".into(),
        "task.comment" => "Commented on a task".into(),
        "notes.list" | "notes.get" | "notes.standing" => "Read notes".into(),
        "guardrail.holds.list" | "guardrail.requests.list" => "Read what waits on you".into(),
        "usage.get" => "Read plan usage".into(),
        "git.status" | "git.log" | "git.branches" | "git.pr.list" => "Read Git".into(),
        "worktree.list" | "worktree.disk" => "Read worktrees".into(),
        "bus.schema" => "Checked how a tool works".into(),
        "thread.ask" => "Asked you".into(),
        "" => "Used a tool".into(),
        other => format!("Used {other}"),
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
    let id = ask["id"].as_i64().unwrap_or(0);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("threads-card");
    card.add_css_class("assistant-ask");
    if status == "pending" {
        card.add_css_class("assistant-ask-waiting");
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let (pill, tone) = match status {
        "pending" => ("Needs your approval", "waiting"),
        "running" => ("Running", "waiting"),
        "approved" => ("Done", "done"),
        "declined" => ("Not done", "quiet"),
        _ => ("Failed", "failed"),
    };
    let mark = label(pill, "assistant-pill");
    mark.add_css_class(&format!("assistant-pill-{tone}"));
    top.append(&mark);
    card.append(&top);
    let title = label(ask["summary"].as_str().unwrap_or(""), "threads-card-title");
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    card.append(&title);
    if let Some(why) = ask["why"].as_str().filter(|w| !w.is_empty()) {
        let quoted = label(&format!("\u{201c}{why}\u{201d} \u{2014} Claude"), "threads-ask-why");
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
    slot.append(&card);
}

/// A payload on one line: `key: value` pairs, strings unquoted and control characters shown.
fn compact(payload: &Value) -> String {
    let Some(fields) = payload.as_object() else { return payload.to_string() };
    fields.iter().map(|(k, v)| {
        let v = match v {
            Value::String(s) => s.replace('\r', "⏎").replace('\n', "↵"),
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

/// The picker's list: Claude's models, then the models on this PC that can run, each with its check.
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
    for (id, caption) in MODELS {
        add(&a.model_list, id, caption, None);
    }
    let local = a.local.borrow();
    if !local.is_empty() {
        let heading = label("On this PC", "threads-menu-heading");
        a.model_list.append(&heading);
        for (id, name) in local.iter() {
            add(&a.model_list, id, name, Some("Runs on your graphics card with a shorter prompt and fewer tools: it loads when picked"));
        }
    }
    drop(local);
    show_model(a);
}

/// The caption and checks for the model shown; a local one says while it loads.
fn show_model(a: &Assistant) {
    let model = a.shown_model.borrow().clone();
    let caption = MODELS.iter().find(|(id, _)| *id == model).map(|(_, c)| c.to_string()).or_else(|| {
        let name = a.local.borrow().iter().find(|(id, _)| *id == model).map(|(_, n)| n.clone())?;
        let state = a.local_state.borrow();
        Some(if state.0 == model && state.1 == "loading" { format!("{name} · loading") } else { name })
    }).unwrap_or_else(|| model.clone());
    a.model_shown.set_text(&caption);
    for (id, mark) in a.marks.borrow().iter() {
        mark.set_opacity(if *id == model { 1.0 } else { 0.0 });
    }
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

/// The last model picked, from the last run (`native.assistant.model`).
fn restore_choice(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let kept = ui.call("settings.get", json!({"path": "native.assistant"})).await.map(|v| v["value"].clone()).unwrap_or_default();
        let Some(model) = kept["model"].as_str().filter(|m| m.len() <= 64) else { return };
        let Some(a) = the() else { return };
        *a.choice.borrow_mut() = model.to_string();
        if a.thread.get().is_none() {
            *a.shown_model.borrow_mut() = model.to_string();
            show_model(&a);
        }
    });
}

/// A model picked: new chats start on it, in this run and the next, and the conversation shown
/// moves to it (`thread.set`; its next message resumes on the new model). A local model starts
/// loading at once.
fn choose(model: &str) {
    let Some(a) = the() else { return };
    let Some(ui) = a.ui.upgrade() else { return };
    *a.choice.borrow_mut() = model.to_string();
    let was = a.shown_model.replace(model.to_string());
    show_model(&a);
    let (model, thread) = (model.to_string(), a.thread.get());
    let local = a.local.borrow().iter().any(|(id, _)| *id == model);
    glib::spawn_future_local(async move {
        let _ = ui.call("settings.set", json!({"path": "native.assistant.model", "value": model})).await;
        // Only a local pick goes to `thread.pick`: picking Claude there would stop a local model
        // a Threads conversation may be using.
        if local {
            if let Err(e) = ui.call("thread.pick", json!({"model": model})).await {
                ui.show_error(&e.to_string());
            }
        }
        if let Some(id) = thread {
            if let Err(e) = ui.call("thread.set", json!({"id": id, "model": model})).await {
                // The conversation stays on its model, and the picker says which.
                if let Some(a) = the().filter(|a| a.thread.get() == Some(id) && *a.shown_model.borrow() == model) {
                    *a.shown_model.borrow_mut() = was;
                    show_model(&a);
                }
                ui.show_error(&format!("The assistant kept its model: {e}"));
            }
        }
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
            }
        }
        "thread.ask" => changed(&a, &payload["ask"]),
        _ => {}
    }
    true
}
