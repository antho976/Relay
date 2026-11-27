//! An agent's conversation, drawn as Claude's app draws one (design 44, page 3): your words in a
//! rounded bubble, the agent's in a serif across the column, what it did as quiet lines that fold
//! ("Read 2 files, ran 1 command") and open to show each call, its reasoning folded.
//!
//! Your message shows once (D7): a message you sent waits under its own bubble ("Queued") until
//! the agent's log holds it, matched by count, never by words (`data::Sending`). The composer is
//! Claude's: the words, a model chip that opens the model sheet (D8), and send, which turns into
//! stop while the agent works. Typing `/` or `!` lists the agent's commands and the repo's pinned
//! shell commands (D9); `!` runs through Claude Code's own bash mode, in the agent's worktree.

use std::collections::HashSet;
use std::rc::Rc;

use crate::app::App;
use crate::data::{self, Agent, Item, Piece, Stage};
use crate::draw::{Text, WIDTH, Weight};
use crate::kit::{self, Kb};
use crate::theme::{self as t, ty};

const TOP_H: f32 = 60.0;
const FIELD: &str = "chat";
/// The bubble's widest.
const BUBBLE_W: f32 = 300.0;
/// The most items drawn: the newest.
const SHOWN: usize = 160;

#[derive(Default)]
pub struct State {
    /// Kept at the newest message until the person scrolls up.
    unstuck: bool,
    /// Folded lines opened, by their place in the list.
    open: HashSet<usize>,
    /// Diffs shown whole.
    open_diffs: HashSet<usize>,
    /// Calls whose fold (a command's output, a plan) was flipped from how it starts, by their
    /// item's place in the list and their own in it.
    toggled: HashSet<(usize, usize)>,
    /// The newest to-do list in the conversation, pinned above the message box while it works.
    todos: Option<Vec<(data::Todo, String)>>,
    /// That pinned list opened whole.
    todos_open: bool,
    /// Code blocks shown whole, by their words.
    open_code: HashSet<u64>,
    /// Wide tables' sideways scroll, by their words.
    tables: std::collections::HashMap<u64, crate::ui::HScroll>,
    /// This frame's step (seconds), for a table coasting sideways.
    dt: f32,
    /// A table still coasts: draw again.
    moving: bool,
    pub model_sheet: bool,
    /// Where the message box starts on screen, for the model picker above it.
    pub box_top: f32,
    /// Pictures on their way to the agent's checkout.
    pub uploading: usize,
    /// The conversation as drawn, read from the log once per change, not once per frame: which
    /// log it came from, its place in the whole list (folds are keyed by it), each one's height.
    shown_from: Option<(String, usize, Option<u64>, String)>,
    items: Rc<Vec<Item>>,
    base: usize,
    heights: Rc<Vec<f32>>,
    /// When the last of your messages was written (ms), for the live line without the terminal.
    last_said_ms: Option<u64>,
}

impl State {
    /// Scrolled up from the newest.
    pub fn unstuck(&self) -> bool {
        self.unstuck
    }

    /// Read the log into items again on the next frame, as when a message comes in.
    #[cfg(test)]
    pub fn forget_layout(&mut self) {
        if let Some(f) = &mut self.shown_from {
            f.3.clear();
        }
    }

    pub fn close_over(&mut self) -> bool {
        std::mem::take(&mut self.model_sheet)
    }
}

/// The models the sheet offers, as `/model` takes them.
pub const MODELS: &[(&str, &str, &str)] = &[
    ("opus", "Opus", "The most capable, and the slowest"),
    ("sonnet", "Sonnet", "Fast and strong, for most coding"),
    ("haiku", "Haiku", "The quickest, for small edits"),
];

/// The effort levels `/effort` takes, as the picker words them.
pub const EFFORTS: &[(&str, &str)] = &[("low", "Low"), ("medium", "Medium"), ("high", "High"), ("xhigh", "Extra"), ("max", "Max")];

/// A model id as the chip shows it: "claude-opus-5-5" is "Opus 5.5".
pub fn model_words(m: Option<&str>) -> String {
    let Some(m) = m.filter(|m| !m.is_empty()) else { return "Default model".into() };
    let m = m.strip_prefix("claude-").unwrap_or(m);
    let mut parts = m.split('-');
    let family = parts.next().unwrap_or(m);
    let nums: Vec<&str> = parts.take_while(|p| p.chars().all(|c| c.is_ascii_digit()) && p.len() <= 2).collect();
    let mut name = family.to_string();
    if let Some(f) = name.get_mut(..1) {
        f.make_ascii_uppercase();
    }
    if nums.is_empty() { name } else { format!("{name} {}", nums.join(".")) }
}

fn header(app: &mut App, a: Option<&Agent>, name: &str) {
    app.top_band(TOP_H);
    let y = app.canvas.inset_top + 8.0;
    let mut cx = crate::cx!(app);
    if kit::icon_key(&mut cx, 6.0, y, "menu") {
        app.st.drawer.open = true;
    }
    let title = a.map_or(name, Agent::title).to_string();
    let sub = match a {
        // Its terminal says when it works (`app.busy`): the list's freshness can lag it.
        Some(a) => format!("{} · {} · {}", a.name, app.repo_name(), if a.alive() && !a.needs_you() && app.busy(name) { "working" } else { a.state_words() }),
        None => name.to_string(),
    };
    let mut x = 56.0;
    if let Some(a) = a {
        crate::drawer::badge(&mut app.canvas, a, x, y + 7.0, 30.0);
        x += 42.0;
    }
    // Pause and Close, at the right (D1).
    let w = WIDTH - x - 16.0 - 44.0;
    if let Some(a) = a {
        crate::agents::more_key(app, a, WIDTH - 50.0, y + 1.0);
    }
    app.canvas.text_fit(&title, x, y + 2.0, w, Text::new(ty::BODY + 1.0, t::INK).semi());
    let sub_ink = if a.is_some_and(Agent::needs_you) { t::WARN } else { t::INK3 };
    app.canvas.text_fit(&sub, x, y + 24.0, w, Text::new(ty::CAPTION, sub_ink));
}

/// Heights and drawing share one walk: `draw` false only measures.
fn item(app: &mut App, it: &Item, index: usize, y: f32, draw: bool) -> f32 {
    let c_w = WIDTH - 2.0 * t::PAD;
    match it {
        Item::You(text) => bubble(app, text, y, draw),
        Item::Said(md) => {
            let st = Text::new(ty::READ, t::INK).face(Weight::Serif).line(1.55);
            6.0 + markdown(app, md, t::PAD, y + 6.0, c_w, st, draw)
        }
        Item::Did { line, failed, calls } => {
            let open = app.st.chat.open.contains(&index);
            // Plans and edits show under the line by themselves; the rest open with it.
            let folded = calls.iter().any(|c| !c.is_edit() && !c.is_plan());
            let mut h = 32.0;
            if draw {
                let mut cx = crate::cx!(app);
                if folded && cx.pressed(0.0, y, WIDTH, 32.0) {
                    cx.c.rect(8.0, y, WIDTH - 16.0, 32.0, 8.0, t::WASH);
                }
                let (g, ink) = if *failed { ("close", t::DANGER) } else { ("check", t::INK3) };
                cx.c.icon(g, t::PAD + 7.0, y + 16.0, 13.0, ink);
                cx.c.text_fit(line, t::PAD + 24.0, y + 7.0, c_w - 50.0, Text::new(ty::CAPTION, t::INK2));
                if folded {
                    cx.c.icon(if open { "chevron-up" } else { "chevron-down" }, WIDTH - t::PAD - 8.0, y + 16.0, 12.0, t::INK3);
                    if cx.tap(0.0, y, WIDTH, 32.0) {
                        if !app.st.chat.open.remove(&index) {
                            app.st.chat.open.insert(index);
                        }
                        app.st.chat.heights = Rc::default();
                    }
                }
            }
            // In the order it made them: every edit's change (the owner: "I don't see the code
            // change"), a plan, and, opened or failed, what each call ran and what it answered.
            for (k, c) in calls.iter().enumerate() {
                if c.is_edit() {
                    h += diff(app, c, index * 64 + k, y + h, draw);
                } else if c.is_plan() {
                    h += plan(app, c, (index, k), y + h, draw);
                } else if open || c.failed {
                    h += call(app, c, (index, k), y + h, draw);
                }
            }
            h + 2.0
        }
        Item::Thought(text) => {
            // Shown, not folded away: Claude Code's thinking is often the summary of what it does.
            let open = app.st.chat.open.contains(&index);
            let st = Text::new(ty::CAPTION, t::INK2).line(1.5);
            let clean = text.replace("**", "");
            let all = app.canvas.wrap(&clean, c_w - 14.0, st, 400).len();
            let shown = if open { all } else { all.min(5) };
            let body = shown as f32 * st.lh();
            if draw {
                app.canvas.rect(t::PAD, y + 8.0, 2.0, body + 18.0, 1.0, t::S3);
                app.canvas.text("Thinking", t::PAD + 12.0, y + 6.0, Text::new(ty::SMALL, t::INK3).semi());
                app.canvas.para(&clean, t::PAD + 12.0, y + 24.0, c_w - 14.0, st, shown);
                if all > 5 {
                    let mut cx = crate::cx!(app);
                    if cx.tap(0.0, y, WIDTH, body + 28.0) {
                        if open {
                            app.st.chat.open.remove(&index);
                            app.st.chat.heights = Rc::default();
                        } else {
                            app.st.chat.open.insert(index);
                            app.st.chat.heights = Rc::default();
                        }
                    }
                }
            }
            body + 36.0
        }
        Item::Note(text) => {
            let st = Text::new(ty::SMALL, t::INK3).center().line(1.4);
            let h = app.canvas.para_h(text, c_w - 40.0, st, 3);
            if draw {
                app.canvas.para(text, t::PAD + 20.0, y + 8.0, c_w - 40.0, st, 3);
            }
            h + 16.0
        }
        Item::Error(text) => {
            let st = Text::new(ty::CAPTION, t::DANGER).line(1.45);
            let h = app.canvas.para_h(text, c_w, st, 6);
            if draw {
                app.canvas.para(text, t::PAD, y + 6.0, c_w, st, 6);
            }
            h + 12.0
        }
    }
}

/// Markdown as [`data::pieces`] reads it, at `x`, `y`, `w` wide, its paragraphs in `prose`:
/// headings in the bold of that face, items with their mark hanging, inline marks as runs
/// ([`spans`]), code on its own ground, tables as a grid. Returns the height; `draw` false only
/// measures, by the same layout.
pub fn markdown(app: &mut App, md: &str, x: f32, y: f32, w: f32, prose: Text, draw: bool) -> f32 {
    let mut h = 0.0;
    for p in data::pieces(md) {
        match p {
            Piece::Prose(s) => {
                h += spans(app, &s, x, y + h, w, prose, 400, draw);
                h += 10.0;
            }
            Piece::Heading(s) => {
                let bold = if prose.weight == Weight::Serif { Weight::SerifBold } else { Weight::Semi };
                let st = Text { size: prose.size + 1.0, ..prose }.face(bold).line(1.35);
                h += 4.0;
                h += spans(app, &s, x, y + h, w, st, 6, draw);
                h += 8.0;
            }
            Piece::Item { mark, text, depth } => {
                let ix = x + depth as f32 * 18.0;
                let hang = 20.0;
                let tw = w - (ix - x) - hang;
                if draw {
                    app.canvas.text(&mark, ix + 2.0, y + h, prose.color(t::INK3));
                }
                h += spans(app, &text, ix + hang, y + h, tw, prose, 200, draw);
                h += 6.0;
            }
            Piece::Code(code) => {
                h += code_block(app, &code, x, y + h, w, draw) + 12.0;
            }
            Piece::Table(rows) => {
                h += table(app, &rows, x, y + h, w, draw) + 12.0;
            }
        }
    }
    h
}

/// A key for something drawn from these words (a code block, a table), to keep its fold or its
/// sideways scroll from frame to frame.
fn key_of(words: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    words.hash(&mut h);
    h.finish()
}

/// Lines of a code block shown before "Show all".
const CODE_LINES: usize = 40;

/// A fenced code block on its own ground, with a copy key. A long one shows its first
/// [`CODE_LINES`] and a key for the rest.
fn code_block(app: &mut App, code: &str, x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let st = Text::new(ty::MONO, t::INK).face(Weight::Mono).line(1.5);
    // The words keep clear of the copy key at the top right.
    let cw = w - 28.0 - 26.0;
    let key = key_of(code);
    // Folded by the code's own lines, each wrapped as it must be.
    let src = code.lines().count();
    let open = app.st.chat.open_code.contains(&key);
    let rows = app.canvas.wrap(code, cw, st, 4000);
    let more = src > CODE_LINES;
    let shown = if open || !more { rows.len() } else { code.lines().take(CODE_LINES).map(|l| app.canvas.wrap(l, cw, st, 4000).len()).sum::<usize>().min(rows.len()) };
    let ch = shown as f32 * st.lh() + if more { 30.0 } else { 0.0 };
    if draw {
        app.canvas.rect(x, y, w, ch + 24.0, 12.0, t::CODE);
        app.canvas.outline(x, y, w, ch + 24.0, 12.0, 1.0, t::LINE);
        for (i, l) in rows.iter().take(shown).enumerate() {
            app.canvas.text(l, x + 14.0, y + 12.0 + i as f32 * st.lh(), st);
        }
        let (kx, ky) = (x + w - 22.0, y + 21.0);
        let mut cx = crate::cx!(app);
        if cx.pressed(kx - 16.0, ky - 16.0, 32.0, 32.0) {
            cx.c.circle(kx, ky, 14.0, t::WASH);
        }
        cx.c.icon("copy", kx, ky, 14.0, t::INK3);
        if cx.tap(kx - 20.0, ky - 20.0, 40.0, 40.0) {
            app.host.copy_text(code);
            app.toast("Copied");
        }
        if more {
            let ky = y + 12.0 + shown as f32 * st.lh() + 6.0;
            let words = if open { "Show less".to_string() } else { format!("Show all {src} lines") };
            app.canvas.text(&words, x + 14.0, ky, Text::new(ty::SMALL, t::INK3).medium());
            let mut cx = crate::cx!(app);
            if cx.tap(x, ky - 8.0, w, 34.0) {
                if !app.st.chat.open_code.remove(&key) {
                    app.st.chat.open_code.insert(key);
                }
                app.st.chat.heights = Rc::default();
            }
        }
    }
    ch + 24.0
}

/// A table as a grid: columns as wide as their widest cell (up to a limit, past which cells
/// wrap), the head in the bold, rules between rows. One wider than the column scrolls sideways.
fn table(app: &mut App, rows: &[Vec<String>], x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return 0.0;
    }
    let body = Text::new(ty::CAPTION, t::INK2).line(1.4);
    let head = body.color(t::INK).semi();
    let pad = 10.0;
    let cell = |r: usize, c: usize| rows[r].get(c).map_or("", String::as_str);
    let widths: Vec<f32> = (0..cols)
        .map(|c| (0..rows.len()).map(|r| app.canvas.measure(cell(r, c), if r == 0 { head } else { body })).fold(0.0, f32::max).clamp(24.0, 220.0).ceil() + 2.0 + 2.0 * pad)
        .collect();
    let heights: Vec<f32> = (0..rows.len())
        .map(|r| (0..cols).map(|c| app.canvas.para_h(cell(r, c), widths[c] - 2.0 * pad, if r == 0 { head } else { body }, 8)).fold(body.lh(), f32::max) + 2.0 * 7.0)
        .collect();
    let total_w: f32 = widths.iter().sum();
    let h: f32 = heights.iter().sum();
    if !draw {
        return h;
    }
    let key = key_of(&rows.concat().join("|"));
    let wide = total_w > w;
    let mut off = 0.0;
    if wide {
        let area = [x, y + app.canvas.dy, w, h];
        app.ui.claim_h(area);
        let mut hs = app.st.chat.tables.get(&key).copied().unwrap_or_default();
        hs.max = total_w - w;
        let dt = app.st.chat.dt;
        app.st.chat.moving |= hs.update(&app.ui, area, dt);
        off = hs.off;
        app.st.chat.tables.insert(key, hs);
        app.canvas.push_clip(x, y + app.canvas.dy, w, h);
    }
    app.canvas.rect(x - off, y, widths.iter().sum(), heights[0], 0.0, t::WASH);
    let mut ry = y;
    for (r, rh) in heights.iter().enumerate() {
        let mut cx_ = x - off;
        for (c, cw) in widths.iter().enumerate() {
            app.canvas.para(cell(r, c), cx_ + pad, ry + 7.0, cw - 2.0 * pad, if r == 0 { head } else { body }, 8);
            cx_ += cw;
        }
        ry += rh;
        if r + 1 < rows.len() {
            app.canvas.rect(x - off, ry - 0.5, total_w, 1.0, 0.0, t::LINE);
        }
    }
    if wide {
        app.canvas.pop_clip();
    }
    h
}

/// A paragraph's inline Markdown, set: bold in the face's bold, italic in Literata's italic (Geist
/// has none here, so a sans paragraph keeps it upright), code in mono on a chip, links in the
/// accent, underlined, opening on a tap. Returns the height.
#[allow(clippy::too_many_arguments)]
fn spans(app: &mut App, s: &str, x: f32, y: f32, w: f32, st: Text, max_lines: usize, draw: bool) -> f32 {
    let serif = matches!(st.weight, Weight::Serif | Weight::SerifBold);
    let mut runs = Vec::new();
    let mut links = Vec::new();
    for sp in data::spans(s) {
        let m = sp.marks;
        let mut style = st;
        if m.code {
            style = Text::new(st.size - 2.0, t::INK).face(Weight::Mono);
        } else if m.bold {
            style = style.face(if serif { Weight::SerifBold } else { Weight::Semi });
        } else if m.italic && serif {
            style = style.face(Weight::SerifItalic);
        }
        if sp.link.is_some() {
            style = style.color(t::ACCENT);
        }
        links.push(sp.link.clone());
        runs.push(crate::draw::Run { text: sp.text, style, chip: m.code.then_some(t::S2), underline: sp.link.is_some() });
    }
    let (h, boxes) = app.canvas.rich(&runs, x, y, w, st.lh(), max_lines, draw);
    let mut open = None;
    for (r, [bx, by, bw, bh]) in boxes {
        let Some(url) = &links[r] else { continue };
        let mut cx = crate::cx!(app);
        if cx.pressed(bx - 2.0, by, bw + 4.0, bh) {
            cx.c.rect(bx - 2.0, by, bw + 4.0, bh, 4.0, t::WASH);
        }
        if cx.tap(bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0) {
            open = Some(url.clone());
        }
    }
    if let Some(url) = open {
        app.host.open_url(&url);
    }
    h
}

/// A question's words as its terminal set them: a first line naming the tool ("Bash command"),
/// what it would run in monospace, and the question itself. Returns the height.
fn question_words(app: &mut App, q: &str, x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let lines: Vec<&str> = q.lines().collect();
    let plain = Text::new(ty::UI, t::INK).line(1.4).medium();
    let head = Text::new(ty::CAPTION, t::INK2).line(1.4);
    let code = Text::new(ty::MONO, t::INK).face(Weight::Mono).line(1.45);
    let mut h = 0.0;
    for (i, l) in lines.iter().enumerate() {
        let st = if lines.len() < 3 || i + 1 == lines.len() {
            plain
        } else if i == 0 {
            head
        } else {
            code
        };
        h += if draw { app.canvas.para(l, x, y + h, w, st, 4) } else { app.canvas.para_h(l, w, st, 4) };
        if i + 1 < lines.len() {
            h += 4.0;
        }
    }
    h
}

/// How long a finger rests on a message before it is copied.
const HOLD_NS: u64 = 450_000_000;

/// What a long press on `it` copies: the words as written (markdown kept, so code stays code).
fn copy_words(it: &Item) -> Option<String> {
    match it {
        Item::You(s) | Item::Said(s) | Item::Thought(s) | Item::Error(s) | Item::Note(s) => Some(s.clone()),
        Item::Did { calls, .. } => {
            let lines: Vec<String> = calls.iter().map(|c| c.detail.as_ref().map_or_else(|| format!("{} {}", c.name, c.summary), |d| format!("{} {}\n{d}", c.name, c.summary))).collect();
            Some(lines.join("\n"))
        }
    }
}

/// The live line while it works, as the terminal shows it: the mark breathing, Claude Code's own
/// word for what it is doing, how long, and the tokens. Without the terminal's foot (an older PC),
/// how long since your last message.
fn live_status(app: &mut App, name: &str, status: Option<&crate::screen::Status>, y: f32) {
    let now = app.now_ns;
    let t_s = now as f32 / 1e9;
    // The mark turns and breathes, as Claude Code's spinner does at the PC.
    let pulse = 0.5 + 0.5 * (t_s * 3.2).sin();
    let turn = (t_s * 90.0) % 360.0;
    app.canvas.icon_turned("claude", t::PAD + 10.0, y + 20.0, 17.0, t::ACCENT.mix(t::INK, 0.3 * (1.0 - pulse)), turn);
    let (verb, details) = match status {
        Some(s) => (s.verb.clone(), s.details.join(" · ")),
        None => {
            // From when this turn began: your message, if it started it, else when the phone first
            // saw it working. Never a count from an older turn.
            let since = app.busy_since.get(name).copied();
            let said = app.st.chat.last_said_ms.into_iter().chain(app.sending.get(name).and_then(|s| s.waiting.first().map(|p| p.sent_ms))).max();
            let start = match (since, said) {
                (Some(b), Some(s)) if s + 120_000 >= b => Some(s.min(b)),
                (b, _) => b,
            };
            let secs = start.map(|f| rc_link::link::now_ms().saturating_sub(f) / 1000);
            ("Working…".to_string(), secs.map(|s| if s >= 60 { format!("{}m {}s", s / 60, s % 60) } else { format!("{s}s") }).unwrap_or_default())
        }
    };
    let vt = Text::new(ty::CAPTION, t::ACCENT.mix(t::INK, 0.35)).medium();
    let vw = app.canvas.text(&verb, t::PAD + 28.0, y + 11.0, vt);
    if !details.is_empty() {
        app.canvas.text_fit(&details, t::PAD + 36.0 + vw, y + 11.0, WIDTH - t::PAD - (t::PAD + 36.0 + vw), Text::new(ty::CAPTION, t::INK3));
    }
    // It moves while it works: about 30 frames a second, only on this page, only while working.
    app.wake_at(now + 33_000_000);
}

/// An edit's change: the file, then its `-` and `+` lines on red and green, the first dozen
/// shown and the rest a tap away. Returns its height.
/// Flip a call's fold from how it starts.
fn toggle(app: &mut App, key: (usize, usize)) {
    if !app.st.chat.toggled.remove(&key) {
        app.st.chat.toggled.insert(key);
    }
    app.st.chat.heights = Rc::default();
}

/// Lines of output shown before "show all": the tail, where a command says how it ended.
const TAIL: usize = 12;

/// One call, opened: what it ran or asked in full, then what it answered. Returns the height.
fn call(app: &mut App, c: &data::ToolCall, key: (usize, usize), y: f32, draw: bool) -> f32 {
    let (x, w) = (t::PAD + 24.0, WIDTH - 2.0 * t::PAD - 24.0);
    let mono = Text::new(ty::SMALL, t::INK2).face(Weight::Mono).line(1.45);
    let caption = Text::new(ty::CAPTION, t::INK2).line(1.45);
    let mut h = 0.0;
    let put = |app: &mut App, s: &str, st: Text, lines: usize, h: &mut f32| {
        *h += if draw { app.canvas.para(s, x, y + *h, w, st, lines) } else { app.canvas.para_h(s, w, st, lines) };
    };
    let detail = c.detail.as_deref().unwrap_or_default();
    if c.is_shell() {
        // The whole command (the summary is its first line), under what it said it is for.
        let whole = if detail.trim().is_empty() { c.summary.as_str() } else { detail };
        let (about, cmd) = match whole.strip_prefix("# ").and_then(|r| r.split_once('\n')) {
            Some((a, cmd)) => (Some(a), cmd),
            None => (None, whole),
        };
        if let Some(a) = about {
            put(app, a, caption.color(t::INK3), 2, &mut h);
            h += 2.0;
        }
        put(app, &format!("$ {cmd}"), mono.color(t::INK), 40, &mut h);
    } else if c.is_todos() {
        h += checklist(app, &data::todos(detail), x, y + h, w, draw);
    } else {
        put(app, &format!("{} {}", c.name, c.summary), mono, 3, &mut h);
        // Questions and their choices, a sub-agent's brief: the words it sent, in prose.
        if !detail.trim().is_empty() {
            h += 2.0;
            let lines = if c.name == "AskUserQuestion" { 40 } else { 6 };
            put(app, detail, caption, lines, &mut h);
        }
    }
    if let Some(out) = c.output.as_deref().filter(|o| !o.trim().is_empty()) {
        h += 6.0 + output(app, c, out, key, x, y + h + 6.0, w, draw);
    }
    h + 10.0
}

/// What a call answered, on code ground. A command's or a failure's tail shows ([`TAIL`] lines);
/// other answers (a file read, a search) fold to one line. A tap shows all or folds.
#[allow(clippy::too_many_arguments)]
fn output(app: &mut App, c: &data::ToolCall, out: &str, key: (usize, usize), x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let ink = if c.failed { t::DANGER } else { t::INK2 };
    let st = Text::new(ty::SMALL, ink).face(Weight::Mono).line(1.45);
    let mut rows: Vec<String> = Vec::new();
    for l in out.trim_end().lines() {
        rows.extend(app.canvas.wrap(l, w - 24.0, st, 4));
        if rows.len() >= 400 {
            break;
        }
    }
    let flipped = app.st.chat.toggled.contains(&key);
    // A failure shows its tail even when the call is folded, where its error is.
    let tail = if c.failed || c.is_shell() { TAIL } else { 0 };
    let shown = if flipped { rows.len() } else { rows.len().min(tail) };
    let hidden = rows.len() - shown;
    let note = if hidden > 0 {
        Some(if shown == 0 { format!("Output · {} lines", rows.len()) } else { format!("{hidden} earlier lines") })
    } else if flipped && rows.len() > tail {
        Some("Show less".to_string())
    } else {
        None
    };
    let note_h = if note.is_some() { 22.0 } else { 0.0 };
    let h = 8.0 + note_h + shown as f32 * st.lh() + if shown > 0 { 8.0 } else { 0.0 };
    if draw {
        app.canvas.rect(x, y, w, h, 8.0, t::CODE);
        let mut ly = y + 8.0;
        if let Some(n) = &note {
            app.canvas.text(n, x + 12.0, ly, Text::new(ty::SMALL, t::INK3).medium());
            ly += note_h;
        }
        for r in &rows[rows.len() - shown..] {
            app.canvas.text(r, x + 12.0, ly, st);
            ly += st.lh();
        }
        if note.is_some() {
            let mut cx = crate::cx!(app);
            if cx.tap(x, y, w, h) {
                toggle(app, key);
            }
        }
    }
    h
}

/// A to-do list: done ones ticked and dimmed, the one under way marked in the accent.
fn checklist(app: &mut App, list: &[(data::Todo, String)], x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let mut h = 0.0;
    for (state, words) in list {
        let (st, mark) = match state {
            data::Todo::Done => (Text::new(ty::CAPTION, t::INK3).line(1.45), 0),
            data::Todo::Doing => (Text::new(ty::CAPTION, t::INK).medium().line(1.45), 1),
            data::Todo::Open => (Text::new(ty::CAPTION, t::INK2).line(1.45), 2),
        };
        let lh = if draw { app.canvas.para(words, x + 22.0, y + h + 2.0, w - 22.0, st, 3) } else { app.canvas.para_h(words, w - 22.0, st, 3) };
        if draw {
            let (cx, cy) = (x + 7.0, y + h + 2.0 + st.lh() * 0.5);
            match mark {
                0 => app.canvas.icon("check", cx, cy, 12.0, t::OK),
                1 => app.canvas.circle(cx, cy, 4.0, t::ACCENT),
                _ => app.canvas.outline(cx - 5.0, cy - 5.0, 10.0, 10.0, 5.0, 1.25, t::INK3),
            }
        }
        h += lh + 4.0;
    }
    h
}

/// A plan put to you, as Markdown on its own card: open while it waits for your answer (it has no
/// answer until you give one), folded to its title once answered. A tap on its head flips that.
fn plan(app: &mut App, c: &data::ToolCall, key: (usize, usize), y: f32, draw: bool) -> f32 {
    let awaiting = c.output.is_none();
    let open = awaiting != app.st.chat.toggled.contains(&key);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let md = c.detail.as_deref().unwrap_or_default();
    let prose = Text::new(ty::BODY, t::INK).line(1.5);
    let head = 42.0;
    let body = if open { markdown(app, md, x + 16.0, 0.0, w - 32.0, prose, false) + 4.0 } else { 0.0 };
    let h = head + body;
    if draw {
        let top = y + 4.0;
        app.canvas.rect(x, top, w, h, 12.0, t::S1);
        if awaiting {
            app.canvas.outline(x, top, w, h, 12.0, 1.0, t::ACCENT.a(0.55));
        }
        app.canvas.icon("list", x + 22.0, top + 21.0, 14.0, t::INK2);
        let tw = app.canvas.text("Plan", x + 38.0, top + 12.0, Text::new(ty::UI, t::INK).semi());
        if awaiting {
            app.canvas.text("· waiting for your answer", x + 44.0 + tw, top + 12.0, Text::new(ty::UI, t::ACCENT));
        }
        let mut cx = crate::cx!(app);
        if cx.pressed(x, top, w, head) {
            cx.c.rect(x, top, w, head, 12.0, t::WASH);
        }
        cx.c.icon(if open { "chevron-up" } else { "chevron-down" }, x + w - 18.0, top + 21.0, 12.0, t::INK3);
        if cx.tap(x, top, w, head) {
            toggle(app, key);
        }
        if open {
            markdown(app, md, x + 16.0, top + head, w - 32.0, prose, true);
        }
    }
    h + 12.0
}

/// The pinned to-do list's height: one line folded, the whole list open.
fn pinned_h(app: &mut App) -> f32 {
    let Some(list) = app.st.chat.todos.clone() else { return 0.0 };
    if app.st.chat.todos_open {
        44.0 + checklist(app, &list, 0.0, 0.0, WIDTH - 2.0 * t::PAD - 28.0, false) + 6.0
    } else {
        44.0
    }
}

/// The newest to-do list, pinned above the message box while the agent works, as Claude's app
/// does: folded to the step under way and the count; a tap opens the whole list.
fn pinned(app: &mut App, y: f32, h: f32) {
    let Some(list) = app.st.chat.todos.clone() else { return };
    let (x, w) = (12.0, WIDTH - 24.0);
    let done = list.iter().filter(|(s, _)| *s == data::Todo::Done).count();
    let now = list.iter().find(|(s, _)| *s == data::Todo::Doing).or_else(|| list.iter().find(|(s, _)| *s == data::Todo::Open)).map_or("All done", |(_, w)| w.as_str());
    app.canvas.rect(x, y, w, h - 6.0, 14.0, t::S1);
    let count = format!("{done} of {}", list.len());
    let ct = Text::new(ty::SMALL, t::INK3).medium();
    let cw = app.canvas.measure(&count, ct);
    app.canvas.circle(x + 18.0, y + 19.0, 4.0, t::ACCENT);
    app.canvas.text_fit(now, x + 32.0, y + 10.0, w - 32.0 - cw - 44.0, Text::new(ty::CAPTION, t::INK).medium());
    app.canvas.text(&count, x + w - 36.0 - cw, y + 12.0, ct);
    let mut cx = crate::cx!(app);
    cx.c.icon(if app.st.chat.todos_open { "chevron-down" } else { "chevron-up" }, x + w - 18.0, y + 19.0, 12.0, t::INK3);
    if cx.tap(x, y, w, 38.0) {
        app.st.chat.todos_open = !app.st.chat.todos_open;
    }
    if app.st.chat.todos_open {
        checklist(app, &list, x + 16.0, y + 40.0, w - 28.0, true);
    }
}

fn diff(app: &mut App, c: &data::ToolCall, key: usize, y: f32, draw: bool) -> f32 {
    const SHOWN: usize = 14;
    let detail = c.detail.clone().unwrap_or_default();
    let st = Text::new(ty::SMALL, t::INK2).face(Weight::Mono).line(1.5);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    // Each line wrapped, never cut: the whole change shows, on its own red or green.
    let mut rows: Vec<(Option<crate::draw::Rgba>, crate::draw::Rgba, String)> = Vec::new();
    for l in detail.lines() {
        let (bg, ink) = match l.chars().next() {
            Some('+') => (Some(t::OK.a(0.12)), t::OK),
            Some('-') => (Some(t::DANGER.a(0.12)), t::DANGER),
            _ => (None, t::INK3),
        };
        for seg in app.canvas.wrap(l, w - 24.0, st, 8) {
            rows.push((bg, ink, seg));
        }
    }
    let open = app.st.chat.open_diffs.contains(&key);
    let n = if open { rows.len().min(600) } else { rows.len().min(SHOWN) };
    let head = 26.0;
    let more = rows.len() > SHOWN;
    let h = head + n as f32 * st.lh() + if more { 26.0 } else { 8.0 };
    if draw {
        app.canvas.rect(x, y + 2.0, w, h, 10.0, t::CODE);
        app.canvas.outline(x, y + 2.0, w, h, 10.0, 1.0, t::LINE);
        app.canvas.icon("edit", x + 14.0, y + 15.0, 12.0, t::INK3);
        app.canvas.text_fit(&c.summary, x + 26.0, y + 7.0, w - 40.0, Text::new(ty::SMALL, t::INK2).medium());
        let mut ly = y + 2.0 + head;
        for (bg, ink, seg) in &rows[..n] {
            if let Some(bg) = bg {
                app.canvas.rect(x + 1.0, ly, w - 2.0, st.lh(), 0.0, *bg);
            }
            app.canvas.text(seg, x + 12.0, ly, st.color(*ink));
            ly += st.lh();
        }
        if more {
            let words = if open { "Show less".to_string() } else { format!("{} more lines", rows.len() - SHOWN) };
            app.canvas.text(&words, x + 12.0, ly + 4.0, Text::new(ty::SMALL, t::INK3).medium());
            let mut cx = crate::cx!(app);
            if cx.tap(x, y, w, h) {
                if open {
                    app.st.chat.open_diffs.remove(&key);
                    app.st.chat.heights = Rc::default();
                } else {
                    app.st.chat.open_diffs.insert(key);
                    app.st.chat.heights = Rc::default();
                }
            }
        }
    }
    h + 8.0
}

/// Your words, right-aligned in a bubble. Returns its height.
fn bubble(app: &mut App, text: &str, y: f32, draw: bool) -> f32 {
    let st = Text::new(ty::BODY, t::INK).line(1.45);
    let lines = app.canvas.wrap(text, BUBBLE_W - 32.0, st, 30);
    let tw = lines.iter().map(|l| app.canvas.measure(l, st)).fold(0.0f32, f32::max);
    let (w, h) = (tw + 32.0, lines.len() as f32 * st.lh() + 24.0);
    if draw {
        let x = WIDTH - t::PAD - w;
        app.canvas.rect(x, y + 6.0, w, h, 18.0, t::S2);
        for (i, l) in lines.iter().enumerate() {
            app.canvas.text(l, x + 16.0, y + 6.0 + 12.0 + i as f32 * st.lh(), st);
        }
    }
    h + 12.0
}

/// Photos picked go to this agent's checkout; each path, once written, joins the message.
fn attachments(app: &mut App, name: &str) {
    for p in crate::attach::take_picked() {
        crate::attach::upload(app.link.clone(), name.to_string(), p);
        app.st.chat.uploading += 1;
    }
    for r in crate::attach::take_done() {
        app.st.chat.uploading = app.st.chat.uploading.saturating_sub(1);
        match r {
            Ok(path) => {
                let typed = if app.focus.is(FIELD) { app.focus.text.clone() } else { app.draft(name) };
                let joined = if typed.trim().is_empty() { path } else { format!("{}\n{path}", typed.trim_end()) };
                if app.focus.is(FIELD) {
                    app.focus.text = joined.clone();
                    app.focus.sel = None;
                    app.host.keyboard(true, &joined, Kb::Text);
                }
                app.set_draft(name, &joined);
            }
            Err(e) => app.toast(&e),
        }
    }
    if app.st.chat.uploading > 0 {
        app.toast("Sending the picture…");
    }
}

pub fn draw(app: &mut App, name: &str, dt: f32) -> bool {
    attachments(app, name);
    app.st.chat.dt = dt;
    let agent = app.agent(name).cloned();
    // A stopped or closed agent reads, with Resume in place of the message box (D1).
    let stopped = agent.as_ref().filter(|a| !crate::agents::live(a)).cloned();
    let working = app.busy(name);
    let asking = agent.as_ref().is_some_and(Agent::needs_you);

    // The composer's height first: the page ends above it.
    // What you started writing to this agent: its own draft, kept across agents and restarts.
    let typed = if app.focus.is(FIELD) { app.focus.text.clone() } else { app.draft(name) };
    let fw = WIDTH - 2.0 * 12.0 - 28.0;
    let tst = Text::new(ty::BODY, t::INK).line(1.4);
    let lines = if typed.is_empty() { 1 } else { app.canvas.wrap_spans(&typed, fw, tst).len().clamp(1, FIELD_LINES) };
    let box_h = 14.0 + lines as f32 * tst.lh() + 10.0 + 34.0 + 10.0;
    // A question it waits on, read off its terminal (only a real card: `screen::ask`).
    let scr = app.screens.get(name).cloned().unwrap_or_default();
    let question = scr.ask.clone().filter(|_| asking || scr.status.is_none());
    let ask_h = match &question {
        Some(q) => {
            let qh = question_words(app, &q.question, t::PAD + 14.0, 0.0, WIDTH - 2.0 * t::PAD - 28.0, false);
            let esc_row = if q.options.iter().any(|(_, l)| l.ends_with("(esc)")) { 14.0 } else { 52.0 };
            16.0 + 20.0 + qh + 10.0 + q.options.len() as f32 * 46.0 + esc_row
        }
        // Blocked with no card on its screen: it stopped and reported being blocked, or the PC
        // cannot show the phone its terminal. Only the second gets keys (Yes and No); the first
        // has nothing to answer, so it gets a line.
        None if asking && app.no_screen => 70.0,
        None if asking && app.screens.contains_key(name) => 30.0,
        None => 0.0,
    };
    // The newest to-do list, pinned while it works with nothing to ask (needs the list read: the
    // log is read into items below, so this frame uses the last frame's list).
    let pin_h = if working && question.is_none() && stopped.is_none() { pinned_h(app) } else { 0.0 };
    let foot_h = if stopped.is_some() { crate::agents::STOPPED_H } else { box_h + 20.0 + ask_h + pin_h };

    let page = app.page_begin(dt, TOP_H, foot_h);
    // The log is read into items and measured only when it or a fold changed: a long
    // conversation costs nothing per frame but what is on screen.
    let (from, found) = match app.convos.get(name) {
        Some(c) => (Some((name.to_string(), c.msgs.len(), c.cursor, c.msgs.last().map(|m| m.id.clone()).unwrap_or_default())), c.found || c.cursor.is_some()),
        None => (None, false),
    };
    if app.st.chat.shown_from != from {
        let msgs = app.convos.get(name).map(|c| c.msgs.as_slice()).unwrap_or_default();
        let mut items = data::items(msgs);
        app.st.chat.todos = items.iter().rev().find_map(|it| match it {
            Item::Did { calls, .. } => calls.iter().rev().find(|c| c.is_todos()).map(|c| data::todos(c.detail.as_deref().unwrap_or_default())),
            _ => None,
        });
        let base = items.len().saturating_sub(SHOWN);
        items.drain(..base);
        app.st.chat.last_said_ms = msgs.iter().rev().find(|m| m.role == "user").and_then(|m| data::parse_ms(&m.created_at));
        // What was already measured keeps its height: a new message measures itself alone.
        let (old, old_base, old_h) = (app.st.chat.items.clone(), app.st.chat.base, app.st.chat.heights.clone());
        let same_log = app.st.chat.shown_from.as_ref().is_some_and(|f| Some(&f.0) == from.as_ref().map(|f| &f.0));
        let mut hs = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let at = (base + i).checked_sub(old_base).filter(|&j| same_log && j < old.len() && j < old_h.len() && old[j] == *it);
            hs.push(match at {
                Some(j) => old_h[j],
                None => item(app, it, base + i, 0.0, false),
            });
        }
        app.st.chat.items = Rc::new(items);
        app.st.chat.base = base;
        app.st.chat.heights = Rc::new(hs);
        app.st.chat.shown_from = from;
    }
    let items = app.st.chat.items.clone();
    let base = app.st.chat.base;
    if app.st.chat.heights.len() != items.len() {
        let hs: Vec<f32> = items.iter().enumerate().map(|(i, it)| item(app, it, base + i, 0.0, false)).collect();
        app.st.chat.heights = Rc::new(hs);
    }
    let heights = app.st.chat.heights.clone();
    if let Some(s) = app.sending.get_mut(name) {
        s.expire_commands(rc_link::link::now_ms());
    }
    let waiting = app.sending.get(name).map(|s| s.waiting.clone()).unwrap_or_default();

    // Measure, so a short conversation sits at the bottom, as a chat does.
    let mut total: f32 = heights.iter().sum();
    for p in &waiting {
        total += bubble(app, &p.text, 0.0, false) + 18.0;
    }
    if working {
        total += 40.0;
    }
    if items.is_empty() && waiting.is_empty() {
        total += 120.0;
    }
    let view = page.bottom - page.top;
    let lead = (view - total - 8.0).max(0.0);
    // Kept at the newest until the person scrolls up; back at the bottom, it follows again.
    let max = total + 8.0 - view;
    let stick = !app.st.chat.unstuck;
    let sc = app.scroll();
    if stick && !page.moving {
        sc.off = max.max(0.0);
    }
    let off = sc.off;
    app.st.chat.unstuck = off < max - 24.0;
    app.canvas.dy = -off;

    let mut y = page.top + lead;
    if items.is_empty() && waiting.is_empty() {
        let words = if found { "Nothing said yet." } else { "The conversation shows once the PC sends it." };
        app.canvas.para(words, 40.0, y + 40.0, WIDTH - 80.0, Text::new(ty::UI, t::INK3).center().line(1.45), 3);
        y += 120.0;
    }
    let screen_top = page.top + off;
    let screen_bottom = page.bottom + off;
    // A finger held still on a message copies it, as Claude's app does.
    let held = app.ui.resting(crate::clock::now_ns()).filter(|(p, ns)| *ns >= HOLD_NS && !app.ui.hidden(*p) && p.1 > page.top && p.1 < page.bottom).map(|(p, _)| p.1 + off);
    let mut copy = None;
    for (i, it) in items.iter().enumerate() {
        let h = heights[i];
        // Only what is on screen is drawn.
        if y + h >= screen_top && y <= screen_bottom {
            item(app, it, base + i, y, true);
        }
        if held.is_some_and(|hy| hy >= y && hy < y + h) {
            copy = copy_words(it);
        }
        y += h;
    }
    if let Some(words) = copy {
        app.ui.swallow();
        app.host.haptic();
        app.host.copy_text(&words);
        app.toast("Copied");
    }
    let mut retry = None;
    for (i, p) in waiting.iter().enumerate() {
        y += bubble(app, &p.text, y, true);
        let online = app.link_state.online();
        let (words, ink) = if p.failed {
            ("Not sent · tap to send again", t::DANGER)
        } else if p.stage < Stage::Entered && !online {
            ("Waiting for your PC", t::INK3)
        } else if p.stage < Stage::Entered {
            ("Sending…", t::INK3)
        } else if working {
            ("Queued · it reads this after its current step", t::INK3)
        } else {
            ("Sent", t::INK3)
        };
        let st = Text::new(ty::SMALL, ink);
        app.canvas.text(words, WIDTH - t::PAD, y - 4.0, st.right());
        if p.failed && crate::cx!(app).tap(0.0, y - 70.0, WIDTH, 84.0) {
            retry = Some(i);
        }
        y += 18.0;
    }
    if working {
        live_status(app, name, scr.status.as_ref(), y);
        y += 40.0;
    }
    app.page_end(&page, y + 8.0);
    app.canvas.dy = 0.0;
    if let Some(i) = retry
        && let Some(s) = app.sending.get_mut(name)
    {
        s.retry(i);
        app.outbox_changed();
    }

    // Scrolled up: a key back to the newest, just above the message box.
    if app.st.chat.unstuck {
        let (kx, ky) = (WIDTH / 2.0, page.bottom - 30.0);
        kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, [kx - 20.0, ky - 20.0, 40.0, 40.0]);
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(kx - 20.0, ky - 20.0, 40.0, 40.0);
        cx.c.shadow(kx - 18.0, ky - 18.0, 36.0, 36.0, 18.0, 12.0, crate::draw::Rgba::hex(0x000000).a(0.4));
        cx.c.circle(kx, ky, 18.0, if pressed { t::S3 } else { t::S2 });
        cx.c.icon("arrow-down", kx, ky, 15.0, t::INK);
        if cx.tap(kx - 20.0, ky - 20.0, 40.0, 40.0) {
            app.st.chat.unstuck = false;
            // Even mid-coast: it stops and follows the newest again.
            app.scroll().stop();
        }
    }
    header(app, agent.as_ref(), name);
    let mut moving = page.moving || std::mem::take(&mut app.st.chat.moving);
    match &stopped {
        Some(a) => crate::agents::stopped_foot(app, a),
        None => moving |= composer(app, name, agent.as_ref(), foot_h, box_h, ask_h, question.as_ref(), scr.mode, &typed, lines, working),
    }
    if app.st.chat.model_sheet {
        model_picker(app, name, agent.as_ref());
    }
    moving
}

#[allow(clippy::too_many_arguments)]
fn composer(
    app: &mut App,
    name: &str,
    agent: Option<&Agent>,
    foot_h: f32,
    box_h: f32,
    ask_h: f32,
    question: Option<&crate::screen::Ask>,
    mode: Option<crate::screen::Mode>,
    typed: &str,
    lines: usize,
    working: bool,
) -> bool {
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - foot_h;
    app.canvas.dy = 0.0;
    // The message box is over the conversation: nothing under it takes a tap.
    let band = [-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0, 0.0, t::BG);
    let mut y = top + 6.0;
    // The pinned to-do list, when the foot was made taller for it.
    let pin_h = foot_h - (box_h + 20.0 + ask_h);
    if pin_h > 1.0 {
        pinned(app, y, pin_h);
        y += pin_h;
    }

    // A question read off its terminal: each answer a key, the first the accent (design 44, C4).
    if let Some(q) = question {
        let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
        let card_h = ask_h - 10.0;
        app.canvas.rect(x, y, w, card_h, 16.0, t::S1);
        app.canvas.outline(x, y, w, card_h, 16.0, 1.0, t::ACCENT.a(0.55));
        app.canvas.text("WAITING FOR YOUR ANSWER", x + 14.0, y + 14.0, Text::new(ty::SMALL, t::ACCENT).semi());
        let qh = question_words(app, &q.question, x + 14.0, y + 36.0, w - 28.0, true);
        let mut oy = y + 36.0 + qh + 10.0;
        let mut pick = None;
        for (i, (n, label)) in q.options.iter().enumerate() {
            let mut cx = crate::cx!(app);
            let pressed = cx.pressed(x + 10.0, oy, w - 20.0, 40.0);
            let fill = if i == 0 { t::ACCENT_DIM } else { t::S2 };
            cx.c.rect(x + 10.0, oy, w - 20.0, 40.0, t::KEY_R, if pressed { t::S3 } else { fill });
            if i == 0 {
                cx.c.outline(x + 10.0, oy, w - 20.0, 40.0, t::KEY_R, 1.0, t::ACCENT);
            }
            // "(esc)" is the Esc key's job here: the option is the Esc answer, so the words drop it.
            let words = label.strip_suffix("(esc)").map_or(label.as_str(), str::trim_end);
            cx.c.text_fit(words, x + 24.0, oy + 11.0, w - 48.0, Text::new(ty::UI, t::INK).medium());
            if cx.tap(x + 10.0, oy, w - 20.0, 40.0) {
                pick = Some(n.clone());
            }
            oy += 46.0;
        }
        // A separate Esc only when no answer already is one.
        if !q.options.iter().any(|(_, l)| l.ends_with("(esc)")) {
            let mut cx = crate::cx!(app);
            let ew = 92.0;
            if kit::plain(&mut cx, x + w - 10.0 - ew, oy, ew, 30.0, "Esc", None) {
                app.key(name, "\x1b");
            }
        }
        if let Some(n) = pick {
            app.key(name, &n);
        }
        y += ask_h;
    } else if ask_h > 0.0 && !app.no_screen {
        app.canvas.text("It stopped and reported it is blocked · reply to go on", t::PAD + 2.0, y + 2.0, Text::new(ty::CAPTION, t::WARN).medium());
        y += ask_h;
    } else if ask_h > 0.0 {
        // Held at a question this PC cannot show the phone: the two answers that always work.
        app.canvas.text("It is waiting on you in its terminal", t::PAD + 2.0, y + 2.0, Text::new(ty::CAPTION, t::WARN).medium());
        let kw = (WIDTH - 2.0 * t::PAD - 10.0) / 2.0;
        let mut cx = crate::cx!(app);
        let yes = kit::primary(&mut cx, t::PAD, y + 24.0, kw, 40.0, "Yes (Enter)", None);
        let no = kit::plain(&mut cx, t::PAD + kw + 10.0, y + 24.0, kw, 40.0, "No (Esc)", None);
        if yes {
            app.key(name, "\r");
        }
        if no {
            app.key(name, "\x1b");
        }
        y += ask_h;
    }

    // `/` and `!`: the commands, over the conversation.
    if app.focus.is(FIELD) && (typed.starts_with('/') || typed.starts_with('!')) && !typed.contains('\n') {
        // Fitted to the repo and this agent (commands.rs): built-ins, the repo's, its skills; for
        // `!`, what you ran here and what this kind of repo runs.
        let list = crate::commands::list(app, name, typed);
        if !list.is_empty() {
            let rh = 42.0;
            let h = list.len() as f32 * rh + 12.0;
            let px = 12.0;
            let py = y - h - 6.0;
            kit::above(&mut crate::cx!(app), crate::ui::layer::POPUP, [px, py, WIDTH - 24.0, h]);
            app.canvas.shadow(px, py, WIDTH - 24.0, h, 16.0, 18.0, crate::draw::Rgba::hex(0x000000).a(0.45));
            app.canvas.rect(px, py, WIDTH - 24.0, h, 16.0, t::PANEL);
            app.canvas.outline(px, py, WIDTH - 24.0, h, 16.0, 1.0, t::LINE);
            // The commands' column is as wide as the longest shown, up to half the card: a long
            // `!` command is cut, a description never is.
            let mono = Text::new(ty::MONO, t::INK).face(Weight::Mono);
            let col = list.iter().map(|c| app.canvas.measure(&c.text, mono)).fold(0.0, f32::max).min((WIDTH - 24.0) * 0.5);
            let mut pick = None;
            for (i, crate::commands::Command { text: cmd, what }) in list.iter().enumerate() {
                let ry = py + 6.0 + i as f32 * rh;
                let mut cx = crate::cx!(app);
                if cx.pressed(px, ry, WIDTH - 24.0, rh) {
                    cx.c.rect(px + 4.0, ry, WIDTH - 32.0, rh, 10.0, t::WASH);
                }
                cx.c.text_fit(cmd, px + 14.0, ry + 12.0, col, mono);
                cx.c.text_fit(what, px + 14.0 + col + 14.0, ry + 13.0, WIDTH - 24.0 - col - 42.0, Text::new(ty::CAPTION, t::INK3));
                if cx.tap(px, ry, WIDTH - 24.0, rh) {
                    pick = Some(cmd.to_string());
                }
            }
            if let Some(cmd) = pick {
                app.focus.text = format!("{cmd} ");
                app.focus.sel = None;
                app.host.keyboard(true, &app.focus.text.clone(), Kb::Text);
            }
        }
    }

    let (bx, bw) = (12.0, WIDTH - 24.0);
    app.st.chat.box_top = y;
    // Back on the message box's own layer after the command list.
    app.ui.layer = crate::ui::layer::CHROME;
    app.canvas.rect(bx, y, bw, box_h, 22.0, t::S1);
    app.canvas.outline(bx, y, bw, box_h, 22.0, 1.0, t::LINE);
    let tst = Text::new(ty::BODY, t::INK).line(1.4);
    let fw = bw - 28.0;
    let focused = app.focus.is(FIELD);
    if typed.is_empty() {
        let who = agent.map_or(name, |a| a.name.as_str());
        app.canvas.text_fit(&format!("Reply to {who}"), bx + 14.0, y + 14.0, fw, tst.color(t::INK3));
        if focused {
            app.canvas.rect(bx + 14.0, y + 14.0, 2.0, tst.lh(), 1.0, t::ACCENT);
        }
    } else {
        field_text(app, typed, bx + 14.0, y + 14.0, fw, tst, focused);
    }
    let field_h = 14.0 + lines as f32 * tst.lh();
    {
        let mut cx = crate::cx!(app);
        if let Some((tx, ty_)) = cx.tap_at(bx, y, bw, field_h + 6.0) {
            let text = typed.to_string();
            if !focused {
                cx.focus.id = Some(FIELD);
                cx.focus.text = text.clone();
                cx.focus.kind = Kb::Text;
                cx.focus.sel = None;
            }
            if !focused || !cx.focus.keyboard_up {
                cx.host.keyboard(true, &text, Kb::Text);
            }
            // A tap in the words puts the caret there, as any text box does.
            if focused && !text.is_empty() {
                let at = caret_at(cx.c, &text, cx.focus.selection().1, fw, tst, tx - (bx + 14.0), ty_ - (y + 14.0));
                cx.focus.sel = Some((at, at));
                cx.host.caret(&text, at, at);
            }
        }
    }
    if focused {
        let text = app.focus.text.clone();
        app.set_draft(name, &text);
    }
    // The keys under the words: the model chip, then send or stop.
    let ky = y + field_h + 10.0;
    let model = model_words(agent.and_then(|a| a.model.as_deref()));
    let mt = Text::new(ty::CAPTION, t::INK).medium();
    let mw = app.canvas.measure(&model, mt) + 40.0;
    // +: a photo or screenshot for the agent.
    {
        let mut cx = crate::cx!(app);
        let px = bx + 10.0;
        if cx.pressed(px, ky, 34.0, 34.0) {
            cx.c.circle(px + 17.0, ky + 17.0, 17.0, t::WASH);
        }
        cx.c.outline(px, ky, 34.0, 34.0, 17.0, 1.0, t::LINE);
        cx.c.icon("plus", px + 17.0, ky + 17.0, 15.0, t::INK2);
        if cx.tap(px - 4.0, ky - 6.0, 42.0, 46.0) {
            cx.host.pick_photo();
        }
    }
    let mx = bx + 10.0 + 42.0;
    {
        let mut cx = crate::cx!(app);
        if cx.pressed(mx, ky, mw, 34.0) {
            cx.c.rect(mx, ky, mw, 34.0, 17.0, t::WASH);
        }
        cx.c.outline(mx, ky, mw, 34.0, 17.0, 1.0, t::LINE);
        cx.c.text(&model, mx + 14.0, ky + (34.0 - mt.lh()) / 2.0, mt);
        cx.c.icon("chevron-down", mx + mw - 16.0, ky + 17.0, 11.0, t::INK3);
        if cx.tap(mx - 4.0, ky - 6.0, mw + 8.0, 46.0) {
            app.st.chat.model_sheet = true;
        }
    }
    // The mode, as its terminal's footer says it; a tap moves to the next (Shift-Tab, as at the PC).
    if let Some(m) = mode {
        let words = m.words();
        let ox = mx + mw + 8.0;
        let ow = app.canvas.measure(words, mt) + 28.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(ox, ky, ow, 34.0) {
            cx.c.rect(ox, ky, ow, 34.0, 17.0, t::WASH);
        }
        cx.c.outline(ox, ky, ow, 34.0, 17.0, 1.0, t::LINE);
        cx.c.text(words, ox + 14.0, ky + (34.0 - mt.lh()) / 2.0, mt);
        if cx.tap(ox - 4.0, ky - 6.0, ow + 8.0, 46.0) {
            app.key(name, "\x1b[Z");
        }
    }
    let sx = bx + bw - 10.0 - 34.0;
    let has_text = !typed.trim().is_empty();
    let mut cx = crate::cx!(app);
    let pressed = cx.pressed(sx - 6.0, ky - 6.0, 46.0, 46.0);
    if has_text || !working {
        let ink = if has_text { t::ACCENT } else { t::S2 };
        cx.c.circle(sx + 17.0, ky + 17.0, 17.0, if pressed { ink.mix(t::BG, 0.15) } else { ink });
        cx.c.icon("arrow-up", sx + 17.0, ky + 17.0, 16.0, if has_text { t::ON_ACCENT } else { t::INK3 });
    } else {
        cx.c.circle(sx + 17.0, ky + 17.0, 17.0, if pressed { t::INK2 } else { t::INK });
        cx.c.rect(sx + 12.0, ky + 12.0, 10.0, 10.0, 2.0, t::BG);
    }
    if cx.tap(sx - 6.0, ky - 6.0, 46.0, 46.0) {
        if has_text {
            let text = typed.trim_end().to_string();
            app.focus.text.clear();
            app.focus.sel = None;
            app.set_draft(name, "");
            app.host.keyboard(true, "", Kb::Text);
            if !app.focus.is(FIELD) {
                app.focus.id = None;
            }
            app.host.haptic();
            app.say(name, &text);
        } else if working {
            // Stop: Esc, as at the PC.
            app.key(name, "\x1b");
            app.toast("Asked it to stop");
        }
    }
    false
}

/// Most lines the message box grows to; past them it scrolls with the caret.
const FIELD_LINES: usize = 6;

/// The lines of the box shown: the last [`FIELD_LINES`] up to the caret's, so what you type is
/// always in view. (first line shown, the lines, the caret's line).
fn field_window(c: &mut crate::draw::Canvas, text: &str, caret: usize, w: f32, st: Text) -> (usize, Vec<(usize, usize)>, usize) {
    let spans = c.wrap_spans(text, w, st);
    // The caret's line: the last one that starts at or before it (a caret at a line's start
    // belongs to that line, not the end of the one before).
    let line = spans.iter().rposition(|(s, _)| *s <= caret).unwrap_or(0);
    let first = (line + 1).saturating_sub(FIELD_LINES).min(spans.len().saturating_sub(FIELD_LINES));
    (first, spans, line)
}

#[cfg(test)]
pub fn field_window_for_test(c: &mut crate::draw::Canvas, text: &str, caret: usize, w: f32, st: Text) -> (usize, Vec<(usize, usize)>, usize) {
    field_window(c, text, caret, w, st)
}

/// The words being typed, with the caret and the selection where the keyboard has them.
fn field_text(app: &mut App, text: &str, x: f32, y: f32, w: f32, st: Text, focused: bool) {
    let chars: Vec<char> = text.chars().collect();
    let (s0, s1) = if focused { app.focus.selection() } else { (chars.len(), chars.len()) };
    let (first, spans, caret_line) = field_window(&mut app.canvas, text, s1, w, st);
    let lh = st.lh();
    for (row, &(a, b)) in spans.iter().enumerate().skip(first).take(FIELD_LINES) {
        let ly = y + (row - first) as f32 * lh;
        let line: String = chars[a..b].iter().collect();
        let x_of = |c: &mut crate::draw::Canvas, i: usize| c.measure(&chars[a..i.clamp(a, b)].iter().collect::<String>(), st);
        // The selection behind its words.
        if focused && s0 < s1 && s0 <= b && s1 >= a {
            let (from, to) = (x_of(&mut app.canvas, s0.max(a)), x_of(&mut app.canvas, s1.min(b)));
            let to = if s1 > b { to + 6.0 } else { to };
            app.canvas.rect(x + from, ly, (to - from).max(2.0), lh, 2.0, t::ACCENT.a(0.3));
        }
        app.canvas.text(&line, x, ly, st);
        if focused && row == caret_line {
            let cx = x_of(&mut app.canvas, s1);
            app.canvas.rect(x + cx, ly, 2.0, lh, 1.0, t::ACCENT);
        }
    }
}

/// The character a tap at (`dx`, `dy`) from the box's first shown line lands nearest.
fn caret_at(c: &mut crate::draw::Canvas, text: &str, caret: usize, w: f32, st: Text, dx: f32, dy: f32) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (first, spans, _) = field_window(c, text, caret, w, st);
    let row = (first + (dy / st.lh()).max(0.0) as usize).min(spans.len().saturating_sub(1));
    let Some(&(a, b)) = spans.get(row) else { return chars.len() };
    let mut best = (a, f32::MAX);
    for i in a..=b {
        let x = c.measure(&chars[a..i].iter().collect::<String>(), st);
        let d = (x - dx).abs();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// The model picker (design 44, C2): a card over the model chip, as Claude's app has. A choice is
/// sent as `/model`, which the agent's log then shows; a tap outside closes it.
fn model_picker(app: &mut App, name: &str, agent: Option<&Agent>) {
    let screen = app.canvas.h;
    app.canvas.dy = 0.0;
    // The card takes every tap while it is open: a row, or outside to close it.
    kit::above(&mut crate::cx!(app), crate::ui::layer::POPUP, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    let (rh, w) = (58.0, WIDTH - 24.0);
    let h = 44.0 + MODELS.len() as f32 * rh + 8.0 + 30.0 + 44.0 + 48.0;
    let (x, y) = (12.0, (app.st.chat.box_top - 8.0 - h).max(app.canvas.inset_top + 8.0));
    app.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, screen, 0.0, t::SCRIM.a(0.35));
    app.canvas.shadow(x, y, w, h, 18.0, 24.0, crate::draw::Rgba::hex(0x000000).a(0.5));
    app.canvas.rect(x, y, w, h, 18.0, t::PANEL);
    app.canvas.outline(x, y, w, h, 18.0, 1.0, t::LINE);
    app.canvas.text("MODEL", x + 18.0, y + 18.0, Text::new(ty::SMALL, t::INK3).semi());
    let now = agent.and_then(|a| a.model.clone()).unwrap_or_default();
    let mut pick = None;
    let mut ry = y + 40.0;
    for (id, label, what) in MODELS {
        let on = now.contains(id);
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(x + 6.0, ry, w - 12.0, rh - 4.0);
        if on || pressed {
            cx.c.rect(x + 6.0, ry, w - 12.0, rh - 4.0, 12.0, if pressed { t::S2 } else { t::S1 });
        }
        cx.c.text(label, x + 18.0, ry + 8.0, Text::new(ty::BODY, t::INK).semi());
        cx.c.text_fit(what, x + 18.0, ry + 30.0, w - 70.0, Text::new(ty::CAPTION, t::INK3));
        if on {
            cx.c.icon("check", x + w - 28.0, ry + (rh - 4.0) / 2.0, 16.0, t::ACCENT);
        }
        if cx.tap(x + 6.0, ry, w - 12.0, rh - 4.0) {
            pick = Some((*id, on));
        }
        ry += rh;
    }
    // Effort: how hard it thinks, as `/effort` sets it (kept for that model at the PC).
    app.canvas.rect(x + 18.0, ry + 4.0, w - 36.0, 1.0, 0.0, t::LINE);
    app.canvas.text("EFFORT", x + 18.0, ry + 18.0, Text::new(ty::SMALL, t::INK3).semi());
    let ey = ry + 38.0;
    let sent = app.efforts.get(name).copied();
    let (gap, n) = (6.0, EFFORTS.len() as f32);
    let kw = (w - 36.0 - gap * (n - 1.0)) / n;
    let mut effort = None;
    for (i, (level, words)) in EFFORTS.iter().enumerate() {
        let kx = x + 18.0 + i as f32 * (kw + gap);
        let on = sent == Some(*level);
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(kx, ey, kw, 36.0);
        cx.c.rect(kx, ey, kw, 36.0, t::KEY_R, if pressed { t::S3 } else if on { t::ACCENT_DIM } else { t::S1 });
        if on {
            cx.c.outline(kx, ey, kw, 36.0, t::KEY_R, 1.0, t::ACCENT);
        }
        let st = Text::new(ty::CAPTION, if on { t::INK } else { t::INK2 }).medium().center();
        cx.c.text_fit(words, kx + kw / 2.0, ey + (36.0 - st.lh()) / 2.0, kw - 6.0, st);
        if cx.tap(kx, ey - 4.0, kw, 44.0) {
            effort = Some(*level);
        }
    }
    app.canvas.text_fit("Both apply from its next step", x + 18.0, ey + 36.0 + 16.0, w - 36.0, Text::new(ty::CAPTION, t::INK3));
    if let Some(level) = effort {
        app.st.chat.model_sheet = false;
        app.efforts.insert(name.to_string(), level);
        app.say(name, &format!("/effort {level}"));
        return;
    }
    match pick {
        Some((_, true)) => app.st.chat.model_sheet = false,
        Some((id, false)) => {
            app.st.chat.model_sheet = false;
            app.say(name, &format!("/model {id}"));
        }
        None => {
            if app.ui.tap_here().is_some() {
                app.ui.tap = None;
                app.st.chat.model_sheet = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host(std::path::PathBuf);
    impl crate::app::Host for Host {
        fn data_dir(&self) -> std::path::PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    #[test]
    fn a_draft_stays_with_its_agent_and_comes_back_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("relay-code-drafts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        let mut clock = 1_000_000_000u64;
        let mut frames = |a: &mut App, n: usize| {
            for _ in 0..n {
                clock += 16_666_666;
                a.frame(clock);
            }
        };
        a.open("bold-puffin");
        frames(&mut a, 2);
        a.focus.id = Some(FIELD);
        a.set_text_sel("half a thought", None);
        frames(&mut a, 2);
        // Another agent: its own empty box; the first keeps its words.
        a.open("swift-ocelot");
        frames(&mut a, 2);
        assert_eq!(a.draft("swift-ocelot"), "");
        assert_eq!(a.draft("bold-puffin"), "half a thought");
        a.open("bold-puffin");
        frames(&mut a, 2);
        assert_eq!(a.draft("bold-puffin"), "half a thought", "back on it, the words are there");
        // Away, then a new start: written off the UI thread, read back on launch.
        a.shot = false;
        a.paused();
        let file = dir.join("cache/outbox.json");
        for _ in 0..200 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let b = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        assert_eq!(b.draft("bold-puffin"), "half a thought");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn models_read_as_people_say_them() {
        assert_eq!(model_words(Some("claude-opus-5-5")), "Opus 5.5");
        assert_eq!(model_words(Some("sonnet")), "Sonnet");
        assert_eq!(model_words(Some("claude-haiku-5-5-20261001")), "Haiku 5.5");
        assert_eq!(model_words(None), "Default model");
    }
}
