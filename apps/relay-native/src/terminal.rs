use crate::client::{Client, Error, Notice};
use base64::Engine;
use gtk4 as gtk;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;
use tokio::runtime::Handle;
use vte4::prelude::*;

#[derive(Default, Debug)]
pub struct Sequence {
    pub last: Option<(u64, u64)>,
    pub catch_up: bool,
}
#[derive(Debug, PartialEq)]
pub enum Step {
    Feed,
    Reset,
    Duplicate,
    Gap,
}
impl Sequence {
    pub fn observe(&mut self, epoch: u64, seq: u64) -> Step {
        let catch_up = std::mem::take(&mut self.catch_up);
        let step = match self.last {
            None => Step::Feed,
            Some((e, _)) if epoch > e => Step::Reset,
            Some((e, _)) if epoch < e => Step::Duplicate,
            Some((_, s)) if seq <= s => Step::Duplicate,
            Some((_, s)) if catch_up || seq == s + 1 => Step::Feed,
            _ => Step::Gap,
        };
        if matches!(step, Step::Feed | Step::Reset) {
            self.last = Some((epoch, seq));
        }
        step
    }
}

/// The private modes a full-screen program sets once at start-up and a terminal fed only the
/// catch-up tail never sees: cursor keys, cursor visibility, the alternate screen, mouse
/// reporting, focus events and bracketed paste.
const TRACKED_MODES: [u16; 13] = [1, 25, 47, 1047, 1049, 1000, 1002, 1003, 1004, 1005, 1006, 1015, 2004];
/// Cursor visible (mode 25) is the only one a fresh terminal starts with.
const DEFAULT_MODES: u16 = 1 << 1;
/// Output fed to one terminal per frame, to begin with. `feed` only queues: VTE parses all of
/// it in its next frame tick, with no budget of its own, and every terminal's tick runs in the
/// same frame. Fed as fast as the socket delivered, one busy pane made that frame as long as
/// parsing everything it had been sent — tens of milliseconds, a quarter second under a flood —
/// and every other pane's echo and every paint waited for it. A pane that produces more than
/// its budget falls behind, and the engine's lag recovery skips it ahead (`Step::Gap`).
const FRAME_BYTES: usize = 128 * 1024;
/// The budget follows what the frames cost: never below this much a frame…
const FRAME_BYTES_MIN: usize = 16 * 1024;
/// …nor above this, which no screen shows anyway.
const FRAME_BYTES_MAX: usize = 1024 * 1024;
/// How long a paced pane waits for a frame that does not come (an unmapped pane, a covered
/// window the compositor stopped drawing): VTE's own fallback cadence.
const FRAME_FALLBACK: Duration = Duration::from_millis(100);

/// How long a frame clock's last update phase took. Every terminal parses what it was fed in
/// that phase, so this is what the panes' budgets are steered by: before-paint opens it, and
/// whichever of layout, paint or after-paint comes next closes it.
struct FramePace {
    clock: gtk::gdk::FrameClock,
    handlers: Vec<glib::SignalHandlerId>,
    update_us: Rc<Cell<i64>>,
}

impl FramePace {
    fn new(clock: &gtk::gdk::FrameClock) -> FramePace {
        let start = Rc::new(Cell::new(None::<i64>));
        let update_us = Rc::new(Cell::new(0));
        let begin = start.clone();
        let mut handlers = vec![clock.connect_before_paint(move |_| begin.set(Some(glib::monotonic_time())))];
        let end = |start: Rc<Cell<Option<i64>>>, update: Rc<Cell<i64>>| {
            move |_: &gtk::gdk::FrameClock| {
                if let Some(at) = start.take() {
                    update.set(glib::monotonic_time() - at);
                }
            }
        };
        handlers.push(clock.connect_layout(end(start.clone(), update_us.clone())));
        handlers.push(clock.connect_paint(end(start.clone(), update_us.clone())));
        handlers.push(clock.connect_after_paint(end(start, update_us.clone())));
        FramePace { clock: clock.clone(), handlers, update_us }
    }

    fn steer(&self, budget: usize) -> usize {
        let (refresh, _) = self.clock.refresh_info(self.clock.frame_time());
        next_budget(budget, self.update_us.get(), refresh)
    }
}

/// The budget after a frame whose update phase took `update_us` of a `refresh_us` interval:
/// shrunk in proportion when it overran half the interval (at most halved), grown by a quarter
/// while it fits. Half, so the frame keeps room to lay out and draw, and a keystroke's echo
/// queued behind a busy pane's output waits at most about a frame.
fn next_budget(budget: usize, update_us: i64, refresh_us: i64) -> usize {
    let target = refresh_us.max(4_000) / 2;
    let next = if update_us > target {
        (budget as f64 * (target as f64 / update_us as f64).max(0.5)) as usize
    } else {
        budget + budget / 4
    };
    next.clamp(FRAME_BYTES_MIN, FRAME_BYTES_MAX)
}

impl Drop for FramePace {
    fn drop(&mut self) {
        for handler in self.handlers.drain(..) {
            self.clock.disconnect(handler);
        }
    }
}

thread_local! {
    /// The modes each session's stream last left on, by name and epoch, so a pane built again
    /// for the same PTY (project switch, reconnect) restores what its catch-up tail no longer
    /// carries. A GUI restart starts empty: only the engine could know those (RA-242).
    static MODES: RefCell<HashMap<String, (u64, Modes)>> = RefCell::default();
    /// Every pane by its session's name, registered as it is built and gone with it: what Ask
    /// reads an agent's prompt from (TM-20) and the Show terminal key focuses (TM-1).
    static PANES: RefCell<HashMap<String, Weak<Pane>>> = RefCell::default();
}

/// The pane showing session `name`, if one is built.
pub fn pane_of(name: &str) -> Option<Rc<Pane>> {
    PANES.with(|panes| panes.borrow().get(name).and_then(Weak::upgrade))
}

/// The glyphs an agent's input line starts with: Claude Code's `>` (`❯` in some themes) and
/// Codex's `›`.
const PROMPT_GLYPHS: [char; 3] = ['>', '❯', '›'];

/// Space and the box drawing a TUI frames its input with (`│ > … │`).
fn frame_char(c: char) -> bool {
    c.is_whitespace() || ('\u{2500}'..='\u{257f}').contains(&c)
}

/// What follows a row's prompt glyph, its frame and spaces trimmed; `None` for a row that does
/// not start with one.
fn after_glyph(row: &str) -> Option<&str> {
    row.trim_start_matches(frame_char).strip_prefix(PROMPT_GLYPHS).map(|rest| rest.trim_matches(frame_char))
}

/// The text of the row session `name`'s agent types on: the cursor's row, or, for a TUI that
/// parks its cursor under its frame, the nearest row above it on the screen that starts with a
/// prompt glyph. `None` when no pane is attached to the session, so its screen is not current.
pub fn prompt_row(name: &str) -> Option<String> {
    let pane = pane_of(name).filter(|pane| pane.is_ready())?;
    let terminal = &pane.terminal;
    let (_, cursor) = terminal.cursor_position();
    let cols = terminal.column_count();
    let row = |at: libc::c_long| -> String {
        let (text, _) = terminal.text_range_format(vte4::Format::Text, at, 0, at, cols);
        text.map(|t| t.trim_end_matches(['\n', '\r']).to_string()).unwrap_or_default()
    };
    let at_cursor = row(cursor);
    if after_glyph(&at_cursor).is_some() {
        return Some(at_cursor);
    }
    let top = (cursor - terminal.row_count() + 1).max(0);
    Some((top..cursor).rev().map(row).find(|r| after_glyph(r).is_some()).unwrap_or(at_cursor))
}

/// Whether an agent's input `row` (from [`prompt_row`]) holds nothing typed, so a prompt put
/// there is not merged into half-written text or read after a leading `/` or `!` (TM-20). For
/// Claude, nothing but space after its `>`, box drawing aside; its dimmed `Try "…"` hint on an
/// empty prompt counts as nothing, since the screen's text cannot tell it from typing. For a
/// provider whose marker is not known, only a row blank once one prompt glyph is trimmed.
pub fn prompt_line_empty(row: &str, provider: &str) -> bool {
    match after_glyph(row) {
        Some(rest) if provider == "claude" => rest.is_empty() || rest.starts_with("Try \""),
        Some(rest) => rest.is_empty(),
        None => row.trim_matches(frame_char).is_empty(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modes(u16);
impl Default for Modes {
    fn default() -> Self {
        Self(DEFAULT_MODES)
    }
}
impl Modes {
    fn set(&mut self, mode: u16, on: bool) {
        if let Some(i) = TRACKED_MODES.iter().position(|m| *m == mode) {
            if on { self.0 |= 1 << i } else { self.0 &= !(1 << i) }
        }
    }
    /// The DECSET/DECRST sequences that take a fresh terminal to these modes.
    pub fn prelude(self) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, mode) in TRACKED_MODES.iter().enumerate() {
            let on = self.0 & (1 << i) != 0;
            if on != (DEFAULT_MODES & (1 << i) != 0) {
                out.extend_from_slice(format!("\x1b[?{mode}{}", if on { 'h' } else { 'l' }).as_bytes());
            }
        }
        out
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Scan {
    #[default]
    Ground,
    Escape,
    CsiStart,
    Private,
    Other,
}

/// Follows DECSET/DECRST in PTY output, across frame boundaries.
#[derive(Debug, Default)]
pub struct ModeTracker {
    pub modes: Modes,
    state: Scan,
    params: Vec<u8>,
}
impl ModeTracker {
    /// Scan `data`; true when it changed a tracked mode.
    pub fn scan(&mut self, data: &[u8]) -> bool {
        let before = self.modes;
        let mut rest = data;
        while !rest.is_empty() {
            if self.state == Scan::Ground {
                let Some(at) = rest.iter().position(|b| *b == 0x1b) else { break };
                rest = &rest[at..];
            }
            let byte = rest[0];
            rest = &rest[1..];
            self.state = match (self.state, byte) {
                (_, 0x1b) => Scan::Escape,
                (_, 0x18 | 0x1a) => Scan::Ground,
                (Scan::Escape, b'[') => {
                    self.params.clear();
                    Scan::CsiStart
                }
                // RIS: a full reset puts every mode back.
                (Scan::Escape, b'c') => {
                    self.modes = Modes::default();
                    Scan::Ground
                }
                (Scan::Ground | Scan::Escape, _) => Scan::Ground,
                (Scan::CsiStart, b'?') => Scan::Private,
                (Scan::Private, b'0'..=b'9' | b';') if self.params.len() < 64 => {
                    self.params.push(byte);
                    Scan::Private
                }
                (Scan::Private, b'h' | b'l') => {
                    for param in self.params.split(|b| *b == b';') {
                        if let Some(mode) = std::str::from_utf8(param).ok().and_then(|s| s.parse().ok()) {
                            self.modes.set(mode, byte == b'h');
                        }
                    }
                    Scan::Ground
                }
                (_, 0x40..=0x7e) => Scan::Ground,
                _ => Scan::Other,
            };
        }
        self.modes != before
    }
}

/// What a session's state reads as wherever it is shown (the strip, the slate, the agent menu),
/// in the phone's words (`Session.stateLabel`), and the lamp class beside it: lit for a live or
/// held agent, never for an idle one.
pub fn state_word(state: &str) -> (String, &'static str) {
    let word = match state {
        "running" => "LIVE".into(),
        "spawning" => "STARTING".into(),
        "blocked" => "NEEDS YOU".into(),
        "restorable" => "STOPPED".into(),
        "created" => "NOT STARTED".into(),
        _ => state.replace('_', " ").to_uppercase(),
    };
    let lamp = match state {
        "running" | "spawning" => "live",
        "blocked" => "held",
        "restorable" => "waiting",
        _ => "off",
    };
    (word, lamp)
}

pub struct Pane {
    pub root: gtk::Box,
    pub terminal: vte4::Terminal,
    pub caption: gtk::Label,
    pub actions: gtk::Box,
    pub header: gtk::Box,
    metadata: gtk::Label,
    state: gtk::Label,
    lamp: gtk::Box,
    branch: gtk::Label,
    branch_row: gtk::Box,
    /// Commits the freshest base has that this checkout does not (`worktree.drift`).
    behind: gtk::Box,
    behind_count: gtk::Label,
    lane: gtk::DrawingArea,
    pub slate_actions: gtk::Box,
    slate: gtk::Box,
    slate_state: gtk::Label,
    slate_hint: gtk::Label,
    /// "Where it left off" on a stopped session's slate (`session_context.rs`).
    slate_context: gtk::Box,
    context_serial: Cell<u64>,
    status: gtk::Label,
    name: String,
    path: PathBuf,
    rt: Handle,
    client: RefCell<Option<Client>>,
    stream: RefCell<Option<glib::JoinHandle<()>>>,
    input: RefCell<Option<glib::JoinHandle<()>>>,
    sequence: RefCell<Sequence>,
    modes: RefCell<ModeTracker>,
    size: Cell<(u16, u16)>,
    resize_pending: Cell<bool>,
    active: Cell<bool>,
    /// Refused attachments in a row, and until when the next one waits: every reconcile used
    /// to open a new socket for an attach the engine had just refused.
    refusals: Cell<u32>,
    retry_at: Cell<Option<std::time::Instant>>,
    /// Asked to attach while waiting out a refusal: it attaches when the wait ends.
    wanted: Cell<bool>,
    /// Output fed per frame (`FRAME_BYTES`) and what steers it.
    budget: Cell<usize>,
    pace: RefCell<Option<FramePace>>,
}

impl Pane {
    pub fn new(name: &str, path: PathBuf, rt: Handle) -> Rc<Self> {
        let terminal = vte4::Terminal::new();
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);
        terminal.set_size(40, 12);
        terminal.set_scrollback_lines(10_000);
        terminal.set_scroll_on_output(false);
        terminal.set_scroll_on_keystroke(true);
        // Font and colours come from `apply_appearance`, which the caller runs next.
        terminal.set_cell_height_scale(1.25);
        terminal.set_cursor_blink_mode(vte4::CursorBlinkMode::Off);
        terminal.set_margin_start(10);
        terminal.set_margin_end(2);
        terminal.set_margin_top(8);
        terminal.set_margin_bottom(4);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("terminal-plate");
        root.set_size_request(280, 280);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("umd");
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.set_valign(gtk::Align::Center);
        lamp.add_css_class("lamp");
        footer.append(&lamp);
        let caption = gtk::Label::new(Some(name));
        caption.set_xalign(0.0);
        caption.set_max_width_chars(24);
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        caption.add_css_class("session-name");
        // Empty until the pane attaches: a parked or exited pane never does, and a strip
        // saying "Connecting" under its slate would read as a stuck connection.
        let status = gtk::Label::new(None);
        status.add_css_class("dim");
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        footer.append(&caption);
        let metadata = gtk::Label::new(None);
        metadata.add_css_class("terminal-identity");
        metadata.set_ellipsize(gtk::pango::EllipsizeMode::End);

        metadata.set_xalign(0.0);
        footer.append(&metadata);
        let branch = gtk::Label::new(None);
        branch.add_css_class("session-branch");
        branch.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        branch.set_max_width_chars(24);
        let branch_row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        branch_row.add_css_class("session-branch");
        let lane = gtk::DrawingArea::new();
        lane.set_content_width(6);
        lane.set_content_height(6);
        lane.set_valign(gtk::Align::Center);
        branch_row.append(&lane);
        branch_row.append(&branch);
        footer.append(&branch_row);
        let state = gtk::Label::new(None);
        state.add_css_class("session-state");
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let behind = gtk::Box::new(gtk::Orientation::Horizontal, 1);
        behind.add_css_class("session-behind");
        behind.set_valign(gtk::Align::Center);
        behind.append(&crate::icons::image("arrow-down", 11));
        let behind_count = gtk::Label::new(None);
        behind.append(&behind_count);
        behind.set_visible(false);
        footer.append(&behind);
        footer.append(&state);
        footer.append(&actions);
        root.append(&footer);
        let screen = gtk::Overlay::new();
        screen.set_vexpand(true);
        let terminal_body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        terminal_body.append(&terminal);
        let scroll =
            gtk::Scrollbar::new(gtk::Orientation::Vertical, terminal.vadjustment().as_ref());
        scroll.add_css_class("terminal-scrollbar");
        scroll.set_widget_name("terminal-scrollback");
        terminal_body.append(&scroll);
        screen.set_child(Some(&terminal_body));
        let slate = gtk::Box::new(gtk::Orientation::Vertical, 8);
        slate.add_css_class("session-slate");
        slate.set_halign(gtk::Align::Fill);
        slate.set_valign(gtk::Align::Fill);
        let slate_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        slate_content.set_halign(gtk::Align::Center);
        slate_content.set_valign(gtk::Align::Center);
        slate_content.set_vexpand(true);
        let slate_state = gtk::Label::new(None);
        slate_state.add_css_class("slate-state");
        slate_content.append(&slate_state);
        let slate_hint = gtk::Label::new(None);
        slate_hint.add_css_class("slate-hint");
        slate_hint.set_wrap(true);
        slate_hint.set_max_width_chars(44);
        slate_hint.set_justify(gtk::Justification::Center);
        slate_content.append(&slate_hint);
        let slate_context = gtk::Box::new(gtk::Orientation::Vertical, 8);
        slate_context.add_css_class("slate-context");
        slate_context.set_visible(false);
        slate_content.append(&slate_context);
        let slate_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        slate_actions.set_halign(gtk::Align::Center);
        slate_content.append(&slate_actions);
        // A plate is only 280px square; a tall context card scrolls instead of clipping.
        let slate_scroll = gtk::ScrolledWindow::builder()
            .child(&slate_content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        slate.append(&slate_scroll);
        screen.add_overlay(&slate);
        root.append(&screen);
        status.set_wrap(true);
        status.set_max_width_chars(40);
        status.add_css_class("terminal-status");
        status.set_visible(false);
        status.connect_label_notify(|label| label.set_visible(!label.text().is_empty()));
        root.append(&status);
        let pane = Rc::new(Self {
            root,
            terminal,
            caption,
            actions,
            header: footer,
            metadata,
            state,
            lamp,
            branch,
            branch_row,
            behind,
            behind_count,
            lane,
            slate_actions,
            slate,
            slate_state,
            slate_hint,
            slate_context,
            context_serial: Cell::new(0),
            status,
            name: name.into(),
            path,
            rt,
            client: RefCell::new(None),
            stream: RefCell::new(None),
            input: RefCell::new(None),
            sequence: RefCell::new(Sequence::default()),
            modes: RefCell::default(),
            size: Cell::new((0, 0)),
            resize_pending: Cell::new(false),
            active: Cell::new(false),
            refusals: Cell::new(0),
            retry_at: Cell::new(None),
            wanted: Cell::new(false),
            budget: Cell::new(FRAME_BYTES),
            pace: RefCell::default(),
        });
        PANES.with(|panes| panes.borrow_mut().insert(name.to_string(), Rc::downgrade(&pane)));

        // Keystrokes are queued while a write is in flight and do not depend on the output
        // attachment, so typing while the pane (re)attaches still reaches the PTY. What cannot be
        // written (no engine to connect to, a refused write) is said under the terminal and not
        // sent later.
        let (tx, rx) = async_channel::unbounded::<String>();
        pane.terminal.connect_commit(move |_, text, _| {
            let _ = tx.try_send(text.into());
        });
        let weak = Rc::downgrade(&pane);
        *pane.input.borrow_mut() = Some(glib::spawn_future_local(async move {
            // Input has its own connection, opened on the first keystroke: on the attachment's,
            // each reply waited behind the PTY frames queued ahead of it.
            let mut connection: Option<(Client, async_channel::Receiver<Notice>)> = None;
            // The error this loop last put under the terminal, cleared by the next write that
            // goes through, unless something else has been said there since.
            let mut shown: Option<String> = None;
            while let Ok(mut data) = rx.recv().await {
                // What was typed while the last write was in flight goes as one write, in order.
                while let Ok(more) = rx.try_recv() {
                    data.push_str(&more);
                }
                let Some(p) = weak.upgrade() else {
                    break;
                };
                // A connection the engine closed is reopened before the write, never after it,
                // so a keystroke is not sent twice.
                if connection.as_ref().is_some_and(|(_, notices)| notices.is_closed()) {
                    connection = None;
                }
                if connection.is_none() {
                    match Client::connect(&p.rt, p.path.clone()).await {
                        Ok(opened) => connection = Some(opened),
                        Err(e) => {
                            let error = e.to_string();
                            p.status.set_text(&error);
                            shown = Some(error);
                            continue;
                        }
                    }
                }
                let Some((client, _)) = connection.as_ref() else { continue };
                match client
                    .request(&p.rt, "session.input", json!({"session":p.name,"data":data}))
                    .await
                {
                    Ok(_) => {
                        if shown.take().is_some_and(|error| p.status.text().as_str() == error) {
                            p.status.set_text("");
                        }
                    }
                    Err(e) => {
                        let error = e.to_string();
                        p.status.set_text(&error);
                        shown = Some(error);
                    }
                }
            }
        }));
        let weak = Rc::downgrade(&pane);
        pane.terminal.connect_char_size_changed(move |_, _, _| {
            if let Some(p) = weak.upgrade() {
                p.schedule_resize();
            }
        });
        let weak = Rc::downgrade(&pane);
        pane.terminal.connect_map(move |_| {
            if let Some(p) = weak.upgrade() {
                p.schedule_resize();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&pane);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers
                .contains(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK)
            {
                if let Some(p) = weak.upgrade() {
                    if key == gtk::gdk::Key::C || key == gtk::gdk::Key::c {
                        p.terminal.copy_clipboard_format(vte4::Format::Text);
                        return glib::Propagation::Stop;
                    }
                    if key == gtk::gdk::Key::V || key == gtk::gdk::Key::v {
                        p.terminal.paste_clipboard();
                        return glib::Propagation::Stop;
                    }
                }
            }
            glib::Propagation::Proceed
        });
        pane.terminal.add_controller(keys);
        pane
    }

    pub fn apply_appearance(&self, mode: &str, points: f64) {
        let background = crate::fonts::palette(mode)[crate::fonts::SCREEN];
        let color = |value: &str| gtk::gdk::RGBA::parse(value).expect("Relay terminal color");
        let palette = [
            "#1a1a1d", "#e5382e", "#3fbf74", "#e0b04a", "#5b9cf6", "#c979d6", "#3ec5cf", "#c8c8c6",
            "#5c5c60", "#ff6b5f", "#5fe08c", "#f2cf6b", "#8ab8ff", "#e19bea", "#68dfe8", "#f2f2f0",
        ]
        .map(color);
        // The plate behind the terminal paints the ground (`@plate`, which follows panel
        // opacity), so VTE's own default background is clear. Reverse video and explicit
        // cell colours still use the opaque RGB, and the cursor keeps an opaque glyph.
        let mut clear = color(background);
        clear.set_alpha(0.);
        self.terminal.set_colors(
            Some(&color("#dcdcda")),
            Some(&clear),
            &palette.iter().collect::<Vec<_>>(),
        );
        self.terminal.set_color_background(&clear);
        self.terminal.set_color_cursor(Some(&color("#ececea")));
        self.terminal
            .set_color_cursor_foreground(Some(&color(background)));
        self.terminal
            .set_color_highlight(Some(&color("rgba(236,236,234,0.22)")));
        let mut font = gtk::pango::FontDescription::from_string("Fira Mono");
        font.set_absolute_size(points.clamp(8.0, 24.0) * 96.0 / 72.0 * gtk::pango::SCALE as f64);
        self.terminal.set_font(Some(&font));
    }
    /// Shown and attached to its session, so input reaches the agent.
    pub fn is_ready(&self) -> bool {
        self.active.get() && self.client.borrow().is_some()
    }
    pub fn verify_ready(&self) {
        assert!(
            self.is_ready(),
            "{} has no ready terminal attachment (active={})",
            self.name,
            self.active.get()
        );
    }
    pub fn verify_session_rendered(&self, session: &serde_json::Value) {
        let state = crate::app::text(session, "state");
        assert!(!self.state.text().is_empty(), "{} has an uninitialized header", self.name);
        assert!(!self.metadata.text().is_empty(), "{} has no provider metadata", self.name);
        assert_eq!(self.slate.is_visible(), !matches!(state, "spawning" | "running" | "idle" | "blocked"),
            "{} has an incorrect terminal overlay", self.name);
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Empty and hide the slate's context card; the returned serial is the only one
    /// [`Pane::context_card`] still answers to, so an older, slower read cannot fill it.
    pub fn begin_context(&self) -> u64 {
        let serial = self.context_serial.get().wrapping_add(1);
        self.context_serial.set(serial);
        crate::app::clear(&self.slate_context);
        self.slate_context.set_visible(false);
        serial
    }
    pub fn context_card(&self, serial: u64) -> Option<&gtk::Box> {
        (self.context_serial.get() == serial).then_some(&self.slate_context)
    }
    /// How many commits `base` has that this pane's checkout does not; nothing shows at none.
    pub fn set_behind(&self, behind: Option<i64>, base: &str) {
        let Some(n) = behind.filter(|n| *n > 0) else {
            self.behind.set_visible(false);
            return;
        };
        self.behind_count.set_text(&n.to_string());
        self.behind.set_tooltip_text(Some(&format!(
            "{n} commit{} behind {base}",
            if n == 1 { "" } else { "s" }
        )));
        self.behind.set_visible(true);
    }
    pub fn update_session(&self, session: &serde_json::Value) {
        use crate::app::text;
        self.caption.set_text(text(session, "name"));
        let pair = text(session, "pair_with");
        let intent = text(session, "intent").trim();
        self.metadata.set_text(
            &format!(
                "{} · {}{}",
                text(session, "provider"),
                text(session, "role"),
                if !intent.is_empty() {
                    format!(" · {intent}")
                } else if pair.is_empty() {
                    String::new()
                } else {
                    format!(" · pair {pair}")
                }
            )
            .to_uppercase(),
        );
        self.branch.set_text(text(session, "branch"));
        let lane_key = if text(session, "worktree").is_empty() {
            text(session, "branch")
        } else {
            text(session, "worktree")
        };
        let hash = lane_key.encode_utf16().fold(0_i32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(unit as i32)
        });
        let color = gtk::gdk::RGBA::parse(
            ["#4f8fd9", "#c9a13b", "#a56cd6", "#3fb0a5", "#d97a4f"]
                [hash.unsigned_abs() as usize % 5],
        )
        .unwrap();
        self.lane.set_draw_func(move |_, cr, width, height| {
            cr.set_source_rgb(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
            );
            cr.rectangle(0.0, 0.0, width as f64, height as f64);
            let _ = cr.fill();
        });
        let tip: Vec<&str> = ["branch", "worktree", "intent"].iter().map(|key| text(session, key).trim()).filter(|part| !part.is_empty()).collect();
        self.header.set_tooltip_text((!tip.is_empty()).then(|| tip.join("\n")).as_deref());
        let state = text(session, "state");
        let (state_label, lamp) = state_word(state);
        self.state.set_text(&state_label);
        let live = matches!(state, "spawning" | "running" | "idle" | "blocked");
        self.slate.set_visible(!live);
        // The slate is as see-through as the plate, so it hides the stale screen under it.
        self.terminal.set_opacity(if live { 1. } else { 0. });
        self.slate_state.set_text(&state_label);
        self.slate_hint.set_text(match state {
            "parked" => "Process released. Scrollback and worktree kept; wake respawns with provider resume.",
            "restorable" => "The previous process ended. Resume restores scrollback and the worktree.",
            // Never started: a launch still on its way (the phone queues the start behind the
            // create), one cut short, or a start that failed. The row cannot tell them apart.
            // The state word above already says what happened; the hint says what to do.
            "created" => "Start launches the provider in its worktree.",
            "exited" => "Resume starts it again where it left off.",
            _ => "No signal from this session.",
        });
        // Only the classes that change: removing and adding them all restyled the whole pane,
        // terminal included, on every update.
        for class in ["live", "held", "waiting", "off", "starting"] {
            let on = class == lamp;
            for widget in [self.root.upcast_ref::<gtk::Widget>(), self.lamp.upcast_ref()] {
                if widget.has_css_class(class) != on {
                    if on {
                        widget.add_css_class(class);
                    } else {
                        widget.remove_css_class(class);
                    }
                }
            }
        }
    }

    /// A launch in flight: the pane is on the wall before its provider exists, and says which
    /// step it is on instead of reading as not started.
    pub fn show_progress(&self, step: &str) {
        self.state.set_text("STARTING");
        self.slate.set_visible(true);
        self.slate_state.set_text("STARTING");
        self.slate_hint.set_text(step);
        for class in ["live", "held", "waiting", "off", "starting"] {
            self.root.remove_css_class(class);
            self.lamp.remove_css_class(class);
        }
        self.root.add_css_class("starting");
        self.lamp.add_css_class("starting");
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        self.wanted.set(active);
        if active && self.retry_at.get().is_some_and(|at| at > std::time::Instant::now()) {
            return;
        }
        if self.active.replace(active) == active {
            return;
        }
        if !active {
            if let Some(task) = self.stream.borrow_mut().take() {
                task.abort();
            }
            self.client.borrow_mut().take(); // socket EOF releases only this attachment
            self.status.set_text("");
            return;
        }
        self.status.set_text("Connecting");
        let weak = Rc::downgrade(self);
        *self.stream.borrow_mut() = Some(glib::spawn_future_local(async move {
            let Some(p) = weak.upgrade() else {
                return;
            };
            let mut backoff = 1;
            loop {
                let result = p.attach_and_render().await;
                if p.client.borrow_mut().take().is_some() {
                    backoff = 1;
                }
                match result {
                    // A gap: the pane fell behind its output and the engine dropped what it
                    // could not hold. Attaching again skips it ahead at once, so nothing is said:
                    // the status line takes rows from the terminal while it shows, and the
                    // resize sent on attaching could measure the terminal in between.
                    Ok(()) => {}
                    // The engine refused the attachment (no live PTY, say). Mark the pane
                    // detached so the next reconcile attaches it again once there is one.
                    Err(e @ Error::Bus(_)) => {
                        // Again after 1, 2, 4… up to 30 s, or at once when a reconcile asks
                        // after that; an attach that works starts the count over.
                        let refusals = p.refusals.get() + 1;
                        p.refusals.set(refusals);
                        let wait = Duration::from_secs(1 << refusals.min(5).saturating_sub(1)).min(Duration::from_secs(30));
                        p.retry_at.set(Some(std::time::Instant::now() + wait));
                        p.status.set_text(&format!("{} Trying again in {} s.", crate::editor::readable(&e), wait.as_secs()));
                        p.active.set(false);
                        let weak = Rc::downgrade(&p);
                        glib::timeout_add_local_once(wait, move || {
                            if let Some(p) = weak.upgrade() {
                                p.retry_at.set(None);
                                if p.wanted.get() {
                                    p.set_active(true);
                                }
                            }
                        });
                        break;
                    }
                    // A timeout or a dropped connection passes; keep trying while shown. Their
                    // own words ask for a reconnect or a refresh, which the retry does itself.
                    Err(e) => {
                        let what = match e {
                            Error::Disconnected => String::from("Connection to the engine closed."),
                            Error::Timeout => String::from("The engine did not answer in time."),
                            e => e.to_string(),
                        };
                        p.status.set_text(&format!("{what} Retrying in {backoff} s."));
                        glib::timeout_future(Duration::from_secs(backoff)).await;
                        backoff = (backoff * 2).min(30);
                    }
                }
            }
        }));
    }

    async fn attach_and_render(self: &Rc<Self>) -> Result<(), Error> {
        let (client, rx) = Client::connect(&self.rt, self.path.clone()).await?;
        let last = self.sequence.borrow().last;
        self.sequence.borrow_mut().catch_up = true;
        client
            .request(
                &self.rt,
                "session.attach",
                json!({"session": self.name,
            "epoch":last.map(|v|v.0), "from_seq":last.map(|v|v.1)}),
            )
            .await?;
        *self.client.borrow_mut() = Some(client);
        self.refusals.set(0);
        self.size.set((0, 0));
        self.schedule_resize();
        self.status.set_text("");
        // Bytes fed since the terminal's last frame, which is when VTE parsed what came before.
        let mut bytes = 0;
        let mut since = None;
        while let Ok(notice) = rx.recv().await {
            match notice {
                Notice::Frame(frame)
                    if frame.stream == "pty" && frame.session.as_deref() == Some(&self.name) =>
                {
                    let epoch = frame
                        .epoch
                        .ok_or_else(|| Error::Protocol("PTY epoch missing".into()))?;
                    let data = base64::engine::general_purpose::STANDARD
                        .decode(
                            frame
                                .data
                                .as_str()
                                .ok_or_else(|| Error::Protocol("PTY data missing".into()))?,
                        )
                        .map_err(|e| Error::Protocol(e.to_string()))?;
                    let fresh = self.sequence.borrow().last.is_none();
                    let step = self.sequence.borrow_mut().observe(epoch, frame.seq);
                    match step {
                        Step::Duplicate => continue,
                        Step::Gap => return Ok(()), // reconnect from the last byte range fed
                        _ => {}
                    }
                    {
                        let mut modes = self.modes.borrow_mut();
                        match step {
                            Step::Duplicate | Step::Gap => {}
                            Step::Reset => {
                                self.terminal.reset(true, true);
                                modes.modes = Modes::default();
                            }
                            // A new terminal for a PTY an earlier pane followed: the catch-up is
                            // only a tail, so restore the modes that pane saw set before it.
                            Step::Feed if fresh => {
                                let known = MODES.with(|m| {
                                    m.borrow().get(&self.name).filter(|(e, _)| *e == epoch).map(|(_, m)| *m)
                                });
                                if let Some(known) = known {
                                    self.terminal.feed(&known.prelude());
                                    modes.modes = known;
                                }
                            }
                            Step::Feed => {}
                        }
                        self.terminal.feed(&data);
                        if modes.scan(&data) || step == Step::Reset {
                            let entry = (epoch, modes.modes);
                            MODES.with(|m| m.borrow_mut().insert(self.name.clone(), entry));
                        }
                    }
                    let frame = self.terminal.frame_clock().map(|clock| clock.frame_counter());
                    if frame != since {
                        since = frame;
                        bytes = 0;
                    }
                    bytes += data.len();
                    // A frame's worth at a time. This is output-driven, never an idle timer.
                    if bytes >= self.budget.get() {
                        self.next_frame().await;
                        since = self.terminal.frame_clock().map(|clock| clock.frame_counter());
                        bytes = 0;
                    }
                }
                Notice::Disconnected(e) => return Err(e),
                _ => {}
            }
        }
        Err(Error::Disconnected)
    }

    /// Until the terminal's next frame, or [`FRAME_FALLBACK`] when no frame comes (the pane is
    /// unmapped, or the compositor stopped sending frames to a covered window).
    /// The frame's cost then steers this pane's budget.
    async fn next_frame(&self) {
        let (send, recv) = async_channel::bounded(1);
        // Removed however this ends, the stream aborted mid-wait included: an unmapped pane's
        // callback would otherwise wait for a frame that may never come. Once it has run and
        // returned `Break`, removing it again does nothing.
        struct Tick(Option<gtk::TickCallbackId>);
        impl Drop for Tick {
            fn drop(&mut self) {
                if let Some(tick) = self.0.take() {
                    tick.remove();
                }
            }
        }
        let _tick = Tick(Some(self.terminal.add_tick_callback(move |_, clock| {
            let _ = send.try_send(clock.clone());
            glib::ControlFlow::Break
        })));
        // Resumed after the whole frame, so its update phase has been measured.
        if let Ok(Ok(clock)) = glib::future_with_timeout(FRAME_FALLBACK, recv.recv()).await {
            let mut pace = self.pace.borrow_mut();
            match pace.as_ref().filter(|pace| pace.clock == clock) {
                Some(pace) => self.budget.set(pace.steer(self.budget.get())),
                None => *pace = Some(FramePace::new(&clock)),
            }
        }
    }

    pub fn schedule_resize(self: &Rc<Self>) {
        if self.resize_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(p) = weak.upgrade() else {
                return;
            };
            p.resize_pending.set(false);
            p.metadata.set_visible(p.root.width() > 420);
            p.branch_row.set_visible(p.root.width() > 600);
            if !p.active.get() || !p.terminal.is_mapped() {
                return;
            }
            let size = (
                p.terminal.column_count().clamp(2, 500) as u16,
                p.terminal.row_count().clamp(2, 300) as u16,
            );
            if p.size.get() == size {
                return;
            }
            let client = p.client.borrow().clone();
            if let Some(client) = client {
                p.size.set(size);
                glib::spawn_future_local(async move {
                    if let Err(e) = client
                        .request(
                            &p.rt,
                            "session.resize",
                            json!({"session":p.name,"cols":size.0,"rows":size.1}),
                        )
                        .await
                    {
                        p.size.set((0, 0));
                        p.status.set_text(&e.to_string());
                    }
                });
            }
        });
    }

    pub fn stop(&self) {
        self.active.set(false);
        if let Some(task) = self.stream.borrow_mut().take() {
            task.abort();
        }
        if let Some(task) = self.input.borrow_mut().take() {
            task.abort();
        }
        self.client.borrow_mut().take();
    }
}

impl Drop for Pane {
    /// Out of the registry, unless a newer pane for the same session has taken its place.
    fn drop(&mut self) {
        let me: *const Pane = self;
        let _ = PANES.try_with(|panes| {
            let mut panes = panes.borrow_mut();
            if panes.get(&self.name).is_some_and(|pane| pane.as_ptr() == me || pane.strong_count() == 0) {
                panes.remove(&self.name);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_with_text_after_its_marker_is_not_empty() {
        // TM-20: Claude Code's input row, framed or bare.
        assert!(prompt_line_empty("│ >                                   │", "claude"));
        assert!(prompt_line_empty("> ", "claude"));
        assert!(prompt_line_empty("❯\u{a0}", "claude"));
        assert!(!prompt_line_empty("│ > fix the hea                       │", "claude"));
        assert!(!prompt_line_empty("> /compact", "claude"));
        // Its hint on an empty prompt is not typing.
        assert!(prompt_line_empty("│ > Try \"refactor <filepath>\"         │", "claude"));
        // A blank row is empty for anyone; a row of text with no marker is not.
        assert!(prompt_line_empty("", "claude") && prompt_line_empty("   ", "codex"));
        assert!(!prompt_line_empty("  ? for shortcuts", "claude"));
        // A provider whose marker is not known: blank after one glyph, nothing else.
        assert!(prompt_line_empty("› ", "codex"));
        assert!(!prompt_line_empty("› Explain this codebase", "codex"));
        assert!(!prompt_line_empty("$ ls", "gemini"));
    }
    #[test]
    fn the_feed_budget_follows_the_frames_cost() {
        let frame = 16_667;
        // Frames with room to spare: a quarter more each time, up to the ceiling.
        assert_eq!(next_budget(FRAME_BYTES, 2_000, frame), FRAME_BYTES + FRAME_BYTES / 4);
        assert_eq!(next_budget(FRAME_BYTES_MAX, 0, frame), FRAME_BYTES_MAX);
        // Parsing took three quarters of a frame where half was the aim: two thirds as much.
        let shrunk = next_budget(FRAME_BYTES, frame * 3 / 4, frame);
        assert!(shrunk > FRAME_BYTES * 66 / 100 && shrunk < FRAME_BYTES * 67 / 100, "{shrunk}");
        // Far over: halved, never less, and never under the floor.
        assert_eq!(next_budget(FRAME_BYTES, 250_000, frame), FRAME_BYTES / 2);
        assert_eq!(next_budget(FRAME_BYTES_MIN, 250_000, frame), FRAME_BYTES_MIN);
        // A clock that knows no refresh interval yet still steers.
        assert!(next_budget(FRAME_BYTES, 1_000, 0) > FRAME_BYTES);
    }

    #[test]
    fn catchup_duplicates_gaps_and_respawn_are_distinct() {
        let mut s = Sequence {
            catch_up: true,
            ..Default::default()
        };
        assert_eq!(s.observe(1, 12), Step::Feed);
        assert_eq!(s.observe(1, 12), Step::Duplicate);
        assert_eq!(s.observe(1, 14), Step::Gap);
        assert_eq!(s.last, Some((1, 12)));
        s.catch_up = true;
        assert_eq!(s.observe(1, 20), Step::Feed);
        assert_eq!(s.observe(1, 21), Step::Feed);
        assert_eq!(s.observe(2, 0), Step::Reset);
        assert_eq!(s.observe(1, 90), Step::Duplicate);
    }

    #[test]
    fn modes_follow_decset_across_frames_and_restore_in_a_prelude() {
        let mut t = ModeTracker::default();
        assert!(!t.scan(b"plain text \x1b[1mbold\x1b[0m"));
        assert_eq!(t.modes.prelude(), b"");
        // One sequence split over two frames, two modes in one sequence.
        assert!(!t.scan(b"\x1b[?104"));
        assert!(t.scan(b"9;2004h\x1b[?25l"));
        assert!(t.scan(b"\x1b[?1000h\x1b[?1006h\x1b[?1000l"));
        assert_eq!(t.modes.prelude(), b"\x1b[?25l\x1b[?1049h\x1b[?1006h\x1b[?2004h");
        // Unknown modes, other CSIs and an aborted sequence change nothing.
        assert!(!t.scan(b"\x1b[?7l\x1b[2004h\x1b[?1049\x1b[0m"));
        let mut fresh = ModeTracker::default();
        assert!(fresh.scan(&t.modes.prelude()));
        assert_eq!(fresh.modes, t.modes);
        assert!(t.scan(b"\x1bc"));
        assert_eq!(t.modes, Modes::default());
    }

    #[test]
    fn states_read_as_the_phone_says_them_and_idle_is_not_lit() {
        for (state, word, lamp) in [
            ("running", "LIVE", "live"),
            ("spawning", "STARTING", "live"),
            ("idle", "IDLE", "off"),
            ("blocked", "NEEDS YOU", "held"),
            ("restorable", "STOPPED", "waiting"),
            ("created", "NOT STARTED", "off"),
            ("parked", "PARKED", "off"),
        ] {
            assert_eq!(state_word(state), (String::from(word), lamp), "{state}");
        }
    }
}
