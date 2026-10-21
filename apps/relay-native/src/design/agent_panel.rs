//! The agent panel: the design's terminal on the canvas, a card to move by its head and resize
//! from its corner, shown either as the terminal itself or as a conversation.
//!
//! As a terminal it is the wall's own pane, lent while it shows here (`Ui::lend_pane`): one
//! PTY, one size, so nothing on the wall redraws at another width, and it stays attached while
//! the agents page is out of sight. As a conversation it is what the agent's provider logged
//! (`session.transcript`), read again about every second while the agent works and less often
//! while it waits, as the phone's chat reads it (`SessionChat.kt`); what is written in its box
//! goes as an Ask about the selection, or, with that off, is typed at the agent's prompt as it
//! stands. Ask stays for a request about the selection from the top bar.

use super::board::Board;
use crate::app::{label, Ui};
use crate::terminal::Pane;
use gtk::prelude::*;
use gtk::gdk;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::time::Duration;

/// The smallest the card is resized to: a terminal of about sixty columns and a dozen rows.
const MIN_W: f64 = 380.0;
const MIN_H: f64 = 300.0;
/// Messages read at a time, and kept on show.
const READ: u32 = 200;
const KEEP: usize = 400;

thread_local! {
    /// Where the card was left, its size, and how it showed, for the next design opened.
    static LAST: Cell<Option<Place>> = const { Cell::new(None) };
}

/// The card's place on the canvas (its top left corner, from the canvas's) and size, and
/// whether it shows the conversation rather than the terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub chat: bool,
}

impl Place {
    /// Kept on a canvas `cw` by `ch`: no smaller than the least, no bigger than the canvas,
    /// and wholly on it. A canvas smaller than the least keeps the card at its corner.
    pub fn within(self, cw: f64, ch: f64) -> Place {
        let w = self.w.max(MIN_W).min(cw.max(MIN_W));
        let h = self.h.max(MIN_H).min(ch.max(MIN_H));
        let x = self.x.min(cw - w).max(0.0);
        let y = self.y.min(ch - h).max(0.0);
        Place { x, y, w, h, ..self }
    }

    /// Where a card first opens on a canvas `cw` by `ch`: at its bottom left, clear of the
    /// inspector on the right and the dock above.
    fn first(cw: f64, ch: f64) -> Place {
        let (w, h) = (560.0, 380.0);
        Place { x: 12.0, y: ch - h - 12.0, w, h, chat: false }.within(cw, ch)
    }
}

/// How long to wait before reading the conversation again: soon while the agent works or a
/// message is on its way, less often while it waits on the person, rarely once it has stopped.
pub fn read_delay(state: &str, sent: bool) -> Duration {
    Duration::from_millis(match state {
        _ if sent => 1_000,
        "running" | "spawning" => 1_000,
        "idle" | "blocked" => 3_000,
        _ => 15_000,
    })
}

/// What the person asked, out of an Ask's prompt (`ask::prompt`): the request's words, without
/// the design, the ids and the read the agent is told to make. `None` for any other message.
pub fn asked(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("[Relay design]")?;
    let (_, request) = rest.split_once(" Request: ")?;
    Some(request.rsplit_once(" — read it with design.get").map_or(request, |(words, _)| words).trim())
}

/// A tool call as the conversation shows it: its name and what it was about, on one line.
pub fn tool_words(block: &Value) -> String {
    let name = block["name"].as_str().unwrap_or("tool");
    // Relay's own ops come as `mcp__relay__design_apply`; the op reads better as written.
    let name = name.strip_prefix("mcp__relay__").map(|op| op.replacen('_', ".", 1)).unwrap_or_else(|| name.to_string());
    match block["summary"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(summary) => format!("{name} · {}", summary.lines().next().unwrap_or(summary)),
        None => name,
    }
}

pub struct AgentPanel {
    pub root: gtk::Box,
    board: Weak<Board>,
    /// The shell the terminal is lent from: reached as the board itself is dropped, so the pane
    /// still goes back.
    ui: Weak<Ui>,
    title: gtk::Label,
    terminal_key: gtk::ToggleButton,
    chat_key: gtk::ToggleButton,
    stack: gtk::Stack,
    /// Where the lent pane goes, and what shows when none can be.
    slot: gtk::Box,
    missing: gtk::Box,
    missing_note: gtk::Label,
    column: gtk::Box,
    scroll: gtk::ScrolledWindow,
    empty: gtk::Label,
    input: gtk::TextView,
    about: gtk::CheckButton,
    note: gtk::Label,
    place: Cell<Place>,
    /// The session shown, and the pane lent for it while the terminal shows.
    session: RefCell<Option<String>>,
    lent: RefCell<Option<(String, Rc<Pane>)>>,
    /// The conversation read so far: the cursor to read on from, the messages drawn.
    cursor: Cell<Option<u64>>,
    seen: RefCell<HashSet<String>>,
    drawn: Cell<usize>,
    /// Counts the reads started, so a read of a conversation since left is dropped.
    serial: Cell<u64>,
    reading: Cell<bool>,
    /// Something was sent: read again soon, whatever the agent is doing.
    sent: Cell<bool>,
    /// The conversation shows its end: what comes in keeps it there, until the person scrolls up.
    stick: Cell<bool>,
}

impl AgentPanel {
    pub fn new(b: &Rc<Board>) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("design-agent-panel");
        root.set_widget_name("design-agent-panel");
        root.set_overflow(gtk::Overflow::Hidden);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("design-agent-head");
        head.set_cursor_from_name(Some("move"));
        let grip = crate::icons::image("modules", 13);
        grip.add_css_class("design-agent-grip");
        head.append(&grip);
        let title = label("", "design-agent-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        title.set_xalign(0.0);
        head.append(&title);
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        modes.add_css_class("design-segmented");
        let terminal_key = gtk::ToggleButton::with_label("Terminal");
        let chat_key = gtk::ToggleButton::with_label("Chat");
        chat_key.set_group(Some(&terminal_key));
        for (key, tip) in [(&terminal_key, "The agent's terminal, as on the wall"), (&chat_key, "The agent's conversation, to read and write to")] {
            key.add_css_class("quiet");
            key.set_tooltip_text(Some(tip));
            key.set_cursor_from_name(Some("default"));
            modes.append(key);
        }
        head.append(&modes);
        let close = crate::app::icon_button("close", "Close the agent panel");
        close.set_cursor_from_name(Some("default"));
        head.append(&close);
        root.append(&head);

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_hhomogeneous(false);
        stack.set_vhomogeneous(false);
        let terminal_page = gtk::Overlay::new();
        let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slot.add_css_class("design-agent-slot");
        slot.set_vexpand(true);
        terminal_page.set_child(Some(&slot));
        let missing = gtk::Box::new(gtk::Orientation::Vertical, 10);
        missing.set_halign(gtk::Align::Center);
        missing.set_valign(gtk::Align::Center);
        let missing_note = label("", "dim");
        missing_note.set_wrap(true);
        missing_note.set_max_width_chars(40);
        missing_note.set_justify(gtk::Justification::Center);
        missing.append(&missing_note);
        let to_chat = gtk::Button::with_label("Show the conversation");
        to_chat.set_halign(gtk::Align::Center);
        missing.append(&to_chat);
        missing.set_visible(false);
        terminal_page.add_overlay(&missing);
        stack.add_named(&terminal_page, Some("terminal"));

        let chat_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroll = gtk::ScrolledWindow::new();
        scroll.add_css_class("design-agent-scroll");
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_vexpand(true);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.add_css_class("design-agent-column");
        let empty = label("", "dim");
        empty.set_wrap(true);
        empty.set_xalign(0.0);
        column.append(&empty);
        scroll.set_child(Some(&column));
        chat_page.append(&scroll);
        let composer = gtk::Box::new(gtk::Orientation::Vertical, 6);
        composer.add_css_class("design-agent-composer");
        let field = gtk::ScrolledWindow::new();
        field.add_css_class("design-ask-field");
        field.set_max_content_height(140);
        field.set_propagate_natural_height(true);
        field.set_hscrollbar_policy(gtk::PolicyType::Never);
        let input = gtk::TextView::new();
        input.add_css_class("design-ask-text");
        input.set_wrap_mode(gtk::WrapMode::WordChar);
        input.set_accepts_tab(false);
        input.set_size_request(-1, 44);
        field.set_child(Some(&input));
        composer.append(&field);
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let about = gtk::CheckButton::with_label("About the selection");
        about.set_active(true);
        about.set_tooltip_text(Some("Send it as an Ask: the design, the page and the selected layers by id, with a picture. Off, it is typed at the agent's prompt as written, to answer a question"));
        about.set_hexpand(true);
        foot.append(&about);
        let send = gtk::Button::new();
        send.add_css_class("primary");
        let send_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        send_row.append(&label("Send", ""));
        send_row.append(&label("Enter", "design-keycap-dark"));
        send.set_child(Some(&send_row));
        foot.append(&send);
        composer.append(&foot);
        let note = label("", "dim");
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.set_visible(false);
        composer.append(&note);
        chat_page.append(&composer);
        stack.add_named(&chat_page, Some("chat"));
        root.append(&stack);

        // The corner to resize from, over the card's own.
        let corner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        corner.add_css_class("design-agent-corner");
        corner.set_size_request(16, 16);
        corner.set_halign(gtk::Align::End);
        corner.set_cursor_from_name(Some("se-resize"));
        corner.set_tooltip_text(Some("Drag to resize"));
        corner.append(&crate::icons::image("expand", 12));
        root.append(&corner);

        let place = LAST.with(Cell::get).unwrap_or_else(|| Place::first(b.size.get().0, b.size.get().1));
        let panel = Rc::new(AgentPanel {
            root,
            board: Rc::downgrade(b),
            ui: b.ui.clone(),
            title,
            terminal_key,
            chat_key,
            stack,
            slot,
            missing,
            missing_note,
            column,
            scroll,
            empty,
            input,
            about,
            note,
            place: Cell::new(place),
            session: RefCell::default(),
            lent: RefCell::default(),
            cursor: Cell::new(None),
            seen: RefCell::default(),
            drawn: Cell::new(0),
            serial: Cell::new(0),
            reading: Cell::new(false),
            sent: Cell::new(false),
            stick: Cell::new(true),
        });
        // Kept at the end as it grows, once its new height is known; scrolled up, it stays put.
        // Only a scroll at an unchanged height is the person's: one that comes with a new
        // height is the layout's, and says nothing of where they want to be.
        let adj = panel.scroll.vadjustment();
        let height = Rc::new(Cell::new(adj.upper()));
        let (weak, h) = (Rc::downgrade(&panel), height.clone());
        adj.connect_upper_notify(move |adj| {
            h.set(adj.upper());
            if weak.upgrade().is_some_and(|p| p.stick.get()) {
                adj.set_value(adj.upper() - adj.page_size());
            }
        });
        let weak = Rc::downgrade(&panel);
        adj.connect_value_changed(move |adj| {
            if let Some(p) = weak.upgrade().filter(|_| adj.upper() == height.get()) {
                p.stick.set(adj.value() + adj.page_size() >= adj.upper() - 24.0);
            }
        });

        // Moved by its head, resized from its corner; the board places it (`placed`).
        let moving = gtk::GestureDrag::new();
        let (weak, from) = (Rc::downgrade(&panel), Rc::new(Cell::new(place)));
        let f = from.clone();
        moving.connect_drag_begin(move |g, x, y| {
            let Some(p) = weak.upgrade() else {
                return;
            };
            // The mode keys and Close are pressed, not dragged.
            if p.root.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| w.ancestor(gtk::Button::static_type()).is_some()) {
                g.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            g.set_state(gtk::EventSequenceState::Claimed);
            f.set(p.place.get());
        });
        let weak = Rc::downgrade(&panel);
        let f = from.clone();
        moving.connect_drag_update(move |_, dx, dy| {
            if let Some(p) = weak.upgrade() {
                let start = f.get();
                p.set_place(Place { x: start.x + dx, y: start.y + dy, ..p.place.get() });
            }
        });
        head.add_controller(moving);
        let sizing = gtk::GestureDrag::new();
        let weak = Rc::downgrade(&panel);
        let f = from.clone();
        sizing.connect_drag_begin(move |g, _, _| {
            if let Some(p) = weak.upgrade() {
                g.set_state(gtk::EventSequenceState::Claimed);
                f.set(p.place.get());
            }
        });
        let weak = Rc::downgrade(&panel);
        sizing.connect_drag_update(move |_, dx, dy| {
            if let Some(p) = weak.upgrade() {
                let start = from.get();
                p.set_place(Place { w: start.w + dx, h: start.h + dy, ..p.place.get() });
            }
        });
        corner.add_controller(sizing);
        let weak = Rc::downgrade(&panel);
        panel.terminal_key.connect_toggled(move |key| {
            if let Some(p) = weak.upgrade().filter(|_| key.is_active()) {
                p.show_mode(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.chat_key.connect_toggled(move |key| {
            if let Some(p) = weak.upgrade().filter(|_| key.is_active()) {
                p.show_mode(true);
            }
        });
        let weak = Rc::downgrade(&panel);
        to_chat.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.chat_key.set_active(true);
            }
        });
        let weak = Rc::downgrade(&panel);
        close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade().and_then(|p| p.board.upgrade()) {
                b.set_agent_panel(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        send.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.send();
            }
        });
        // Enter sends, Shift+Enter starts a new line, as in Threads.
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&panel);
        keys.connect_key_pressed(move |_, key, _, mods| {
            let enter = matches!(key, gdk::Key::Return | gdk::Key::KP_Enter);
            match weak.upgrade().filter(|_| enter && !mods.contains(gdk::ModifierType::SHIFT_MASK)) {
                Some(p) => {
                    p.send();
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        panel.input.add_controller(keys);
        panel
    }

    /// Show the card for the design's terminal, as it was last left.
    pub fn open(self: &Rc<Self>) {
        let session = self.board.upgrade().and_then(|b| b.doc.session());
        self.title.set_text(&session.clone().unwrap_or_else(|| "No terminal".into()));
        self.title.set_tooltip_text(session.as_ref().map(|s| format!("This design's terminal, {s}")).as_deref());
        if *self.session.borrow() != session {
            self.give_back();
            *self.session.borrow_mut() = session;
            self.forget_chat();
        }
        let chat = self.place.get().chat;
        // Toggling runs `show_mode`; the key already on shows it here.
        let key = if chat { &self.chat_key } else { &self.terminal_key };
        if key.is_active() {
            self.show_mode(chat);
        } else {
            key.set_active(true);
        }
    }

    /// Put away: the terminal goes back to the wall, and nothing more is read.
    pub fn close(&self) {
        // A board dropped as the app quits, its thread's storage going: nothing to hand back to.
        if LAST.try_with(|l| l.set(Some(self.place.get()))).is_err() {
            return;
        }
        self.give_back();
        self.serial.set(self.serial.get() + 1);
    }

    /// Where the board places the card on a canvas `cw` by `ch`.
    pub fn placed(&self, cw: f64, ch: f64) -> gdk::Rectangle {
        let p = self.place.get().within(cw, ch);
        gdk::Rectangle::new(p.x.round() as i32, p.y.round() as i32, p.w.round() as i32, p.h.round() as i32)
    }

    /// Show session `name`'s terminal in the card when it is the one shown: true when it now
    /// is (Ask's Show terminal, TM-1).
    pub fn show_terminal_of(self: &Rc<Self>, name: &str) -> bool {
        if self.session.borrow().as_deref() != Some(name) || !self.root.is_mapped() {
            return false;
        }
        self.terminal_key.set_active(true);
        self.lent.borrow().as_ref().is_some_and(|(_, pane)| pane.terminal.grab_focus())
    }

    /// Show the conversation, or the terminal, as the mode keys do.
    pub fn set_chat(&self, chat: bool) {
        if chat { &self.chat_key } else { &self.terminal_key }.set_active(true);
    }

    /// Send `text` from the box, as Enter does: the smoke's `panel-send:` step.
    pub fn send_text(self: &Rc<Self>, text: &str) {
        self.input.buffer().set_text(text);
        self.send();
    }

    /// What it shows, for the smoke's `panel` step: its mode, the terminal lent, how many
    /// messages are drawn and what is said under the box.
    pub fn state(&self) -> Value {
        let alloc = self.root.allocation();
        json!({
            "chat": self.place.get().chat,
            "session": *self.session.borrow(),
            "lent": self.lent.borrow().as_ref().map(|(name, _)| name.clone()),
            "missing": self.missing.is_visible().then(|| self.missing_note.text().to_string()),
            "messages": self.drawn.get(),
            "note": self.note.is_visible().then(|| self.note.text().to_string()),
            "box": [alloc.x(), alloc.y(), alloc.width(), alloc.height()],
        })
    }

    fn set_place(&self, to: Place) {
        let Some(b) = self.board.upgrade() else {
            return;
        };
        let (cw, ch) = b.size.get();
        self.place.set(to.within(cw, ch));
        LAST.with(|l| l.set(Some(self.place.get())));
        b.overlay.queue_allocate();
    }

    fn show_mode(self: &Rc<Self>, chat: bool) {
        self.place.set(Place { chat, ..self.place.get() });
        LAST.with(|l| l.set(Some(self.place.get())));
        self.stack.set_visible_child_name(if chat { "chat" } else { "terminal" });
        if chat {
            self.give_back();
            self.read_chat();
            self.input.grab_focus();
        } else {
            self.borrow_terminal();
        }
    }

    /// Lend the session's pane from the wall into the card, or say why it cannot come.
    fn borrow_terminal(&self) {
        let Some(session) = self.session.borrow().clone() else {
            return self.say_missing("This design has no terminal. Tie it to one from the terminal key in the top bar.");
        };
        if self.lent.borrow().as_ref().is_some_and(|(name, _)| *name == session) {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let lent = ui.lend_pane(&session);
        let Some(pane) = lent else {
            let why = match ui.lent_pane() {
                Some(other) => format!("{other}'s terminal is shown in another design's agent panel. Close that one to show {session}'s here."),
                _ => format!("{session}'s terminal is not open in this project's wall, so it cannot show here."),
            };
            return self.say_missing(&why);
        };
        // Off the wall's grid already; anywhere else it is let go of first.
        if pane.root.parent().is_some() {
            pane.root.unparent();
        }
        self.slot.append(&pane.root);
        self.missing.set_visible(false);
        pane.schedule_resize();
        *self.lent.borrow_mut() = Some((session, pane));
    }

    fn say_missing(&self, why: &str) {
        self.missing_note.set_text(why);
        self.missing.set_visible(true);
    }

    /// Hand the lent pane back to the wall.
    fn give_back(&self) {
        let Some((name, pane)) = self.lent.borrow_mut().take() else {
            return;
        };
        if pane.root.parent().as_ref() == Some(self.slot.upcast_ref()) {
            self.slot.remove(&pane.root);
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.return_pane(&name);
        }
    }

    fn forget_chat(&self) {
        self.serial.set(self.serial.get() + 1);
        self.cursor.set(None);
        self.seen.borrow_mut().clear();
        self.drawn.set(0);
        while let Some(child) = self.column.last_child().filter(|c| c != self.empty.upcast_ref::<gtk::Widget>()) {
            self.column.remove(&child);
        }
        self.empty.set_visible(true);
        self.empty.set_text("Reading the conversation…");
    }

    /// Read what the agent logged since the last read, draw it, and read again later while the
    /// conversation shows.
    fn read_chat(self: &Rc<Self>) {
        let (Some(b), Some(session)) = (self.board.upgrade(), self.session.borrow().clone()) else {
            self.empty.set_text("This design has no terminal to talk to. Tie it to one from the terminal key in the top bar.");
            return;
        };
        let Some(ui) = b.ui.upgrade() else {
            return;
        };
        if self.reading.replace(true) {
            return;
        }
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let mut payload = json!({"session": session, "limit": READ});
        if let Some(cursor) = self.cursor.get() {
            payload["after"] = json!(cursor);
        }
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let read = ui.call("session.transcript", payload).await;
            let Some(p) = weak.upgrade() else {
                return;
            };
            p.reading.set(false);
            if p.serial.get() != serial {
                return;
            }
            match read {
                Ok(v) => p.show_read(&v),
                Err(e) => {
                    if p.drawn.get() == 0 {
                        p.empty.set_text(&format!("Could not read {session}'s conversation: {e}"));
                    }
                }
            }
            let state = p.board.upgrade().map(|b| b.lamp_state()).unwrap_or_default();
            let wait = read_delay(&state, p.sent.replace(false));
            glib::timeout_future(wait).await;
            if p.serial.get() == serial && p.place.get().chat && p.root.is_mapped() {
                p.read_chat();
            }
        });
    }

    fn show_read(&self, v: &Value) {
        if v["reset"] == true {
            self.cursor.set(None);
            self.seen.borrow_mut().clear();
            self.drawn.set(0);
            while let Some(child) = self.column.last_child().filter(|c| c != self.empty.upcast_ref::<gtk::Widget>()) {
                self.column.remove(&child);
            }
        }
        self.cursor.set(v["cursor"].as_u64().or(self.cursor.get()));
        if self.drawn.get() == 0 {
            self.stick.set(true);
        }
        for m in v["messages"].as_array().into_iter().flatten() {
            let id = m["id"].as_str().unwrap_or_default().to_string();
            if !self.seen.borrow_mut().insert(id) {
                continue;
            }
            self.draw_message(m);
        }
        // Only the newest part stays on show.
        let mut extra = self.drawn.get().saturating_sub(KEEP);
        while extra > 0 {
            match self.empty.next_sibling() {
                Some(oldest) => self.column.remove(&oldest),
                None => break,
            }
            extra -= 1;
            self.drawn.set(self.drawn.get() - 1);
        }
        if self.drawn.get() == 0 {
            let found = v["found"] != false;
            self.empty.set_text(if found { "Nothing said yet. Write below to ask about the selection." } else { "This agent's provider keeps no conversation Relay can read; the terminal shows it all." });
            self.empty.set_visible(true);
            return;
        }
        self.empty.set_visible(false);
    }

    /// One logged message: the person's words in a bubble, the agent's as Threads draws a
    /// reply, each tool it ran as one quiet line; a tool's answer is left to the terminal.
    fn draw_message(&self, m: &Value) -> bool {
        let body = &m["body"];
        let mut drawn = false;
        match m["role"].as_str().unwrap_or("") {
            "user" => {
                let text = body["text"].as_str().unwrap_or("").trim();
                if text.is_empty() {
                    return false;
                }
                // An Ask shows as what was asked; the agent read the rest.
                let request = asked(text);
                if request.is_some() {
                    let caption = label("Asked about the selection", "design-agent-asked");
                    caption.set_halign(gtk::Align::End);
                    self.column.append(&caption);
                    self.drawn.set(self.drawn.get() + 1);
                }
                let bubble = label(request.unwrap_or(text), "threads-user");
                bubble.set_wrap(true);
                bubble.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                bubble.set_selectable(true);
                bubble.set_focusable(false);
                bubble.set_halign(gtk::Align::End);
                bubble.set_xalign(0.0);
                bubble.set_max_width_chars(48);
                self.column.append(&bubble);
                drawn = true;
            }
            "assistant" => {
                for block in body["blocks"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") if !block["text"].as_str().unwrap_or("").trim().is_empty() => {
                            let turn = gtk::Box::new(gtk::Orientation::Vertical, 8);
                            turn.add_css_class("design-agent-reply");
                            crate::money::threads::draw_reply(&turn, block["text"].as_str().unwrap_or(""));
                            self.column.append(&turn);
                            self.drawn.set(self.drawn.get() + 1);
                            drawn = true;
                        }
                        Some("tool_use") => {
                            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                            line.add_css_class("threads-tool");
                            line.append(&crate::icons::image("terminal", 13));
                            let words = label(&tool_words(block), "threads-tool-text");
                            words.set_ellipsize(gtk::pango::EllipsizeMode::End);
                            words.set_xalign(0.0);
                            line.append(&words);
                            self.column.append(&line);
                            self.drawn.set(self.drawn.get() + 1);
                            drawn = true;
                        }
                        _ => {}
                    }
                }
                return drawn;
            }
            _ => {}
        }
        if drawn {
            self.drawn.set(self.drawn.get() + 1);
        }
        drawn
    }

    /// Send what is written: an Ask about the selection, or typed at the prompt as it stands.
    fn send(self: &Rc<Self>) {
        let buffer = self.input.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
        let (Some(b), Some(session)) = (self.board.upgrade(), self.session.borrow().clone()) else {
            return self.say("This design has no terminal to send to.");
        };
        if text.is_empty() {
            return;
        }
        self.note.set_visible(false);
        let weak = Rc::downgrade(self);
        if self.about.is_active() {
            buffer.set_text("");
            let kept = text.clone();
            super::ask::ask_from_panel(&b, session, text, move |why| {
                if let Some(p) = weak.upgrade() {
                    // Held, so the words come back to send again.
                    let buffer = p.input.buffer();
                    if buffer.char_count() == 0 {
                        buffer.set_text(&kept);
                    }
                    p.say(&why);
                }
            });
            self.sent.set(true);
            return;
        }
        let Some(ui) = b.ui.upgrade() else {
            return;
        };
        buffer.set_text("");
        self.sent.set(true);
        glib::spawn_future_local(async move {
            // One paste, then Enter on its own once it has landed (I-4).
            let typed = match ui.call("session.input", json!({"session": session, "data": text, "paste": true})).await {
                Ok(_) => {
                    glib::timeout_future(Duration::from_millis(60)).await;
                    ui.call("session.input", json!({"session": session, "data": "\r"})).await.map(|_| ())
                }
                Err(e) => Err(e),
            };
            if let (Err(e), Some(p)) = (typed, weak.upgrade()) {
                let buffer = p.input.buffer();
                if buffer.char_count() == 0 {
                    buffer.set_text(&text);
                }
                p.say(&format!("Could not reach {session}: {e}"));
            }
        });
    }

    fn say(&self, why: &str) {
        self.note.set_text(why);
        self.note.set_visible(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_stays_on_the_canvas() {
        let p = Place { x: 900.0, y: -40.0, w: 200.0, h: 2000.0, chat: true };
        // No smaller than the least, no taller than the canvas, wholly on it.
        assert_eq!(p.within(1200.0, 800.0), Place { x: 820.0, y: 0.0, w: MIN_W, h: 800.0, chat: true });
        // A canvas too small for the least keeps it at its corner.
        let tiny = Place { x: 50.0, y: 50.0, w: 500.0, h: 400.0, chat: false }.within(200.0, 100.0);
        assert_eq!((tiny.x, tiny.y, tiny.w, tiny.h), (0.0, 0.0, MIN_W, MIN_H));
        // First opened at the bottom left, clear of the inspector.
        let first = Place::first(1600.0, 900.0);
        assert_eq!((first.x, first.y), (12.0, 900.0 - 380.0 - 12.0));
    }

    #[test]
    fn the_conversation_is_read_as_often_as_it_moves() {
        assert_eq!(read_delay("running", false), Duration::from_secs(1));
        assert_eq!(read_delay("idle", false), Duration::from_secs(3));
        assert_eq!(read_delay("idle", true), Duration::from_secs(1), "a message on its way");
        assert_eq!(read_delay("exited", false), Duration::from_secs(15));
    }

    #[test]
    fn an_ask_shows_as_what_was_asked() {
        let prompt = super::super::ask::prompt(&super::super::ask::Prompt {
            title: "Live workout",
            design: 4,
            page: "page-1",
            page_name: "Page 1",
            selected: &[("glow".into(), "rect".into(), "Glow".into())],
            picture: Ok(Some("/tmp/a.png")),
            motion: None,
            request: "make the glow warmer",
        });
        assert_eq!(asked(&prompt), Some("make the glow warmer"));
        assert_eq!(asked("make the glow warmer"), None, "typed at the prompt as written");
    }

    #[test]
    fn a_tool_call_is_one_line() {
        assert_eq!(tool_words(&json!({"name": "mcp__relay__design_apply", "summary": "3 ops\nmore"})), "design.apply · 3 ops");
        assert_eq!(tool_words(&json!({"name": "Read", "summary": ""})), "Read");
        assert_eq!(tool_words(&json!({})), "tool");
    }
}
