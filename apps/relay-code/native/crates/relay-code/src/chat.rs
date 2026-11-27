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
    open: HashSet<String>,
    /// Diffs shown whole.
    open_diffs: HashSet<(String, usize)>,
    /// Calls whose fold (a command's output, a plan) was flipped from how it starts, by their
    /// item's place in the list and their own in it.
    toggled: HashSet<(String, usize)>,
    /// The newest to-do list in the conversation, pinned above the message box while it works.
    todos: Option<Vec<(data::Todo, String)>>,
    /// That pinned list opened whole.
    todos_open: bool,
    /// Code blocks shown whole, by their words.
    open_code: HashSet<(String, usize)>,
    /// Wide tables' sideways scroll, by their words.
    tables: std::collections::HashMap<u64, crate::ui::HScroll>,
    /// This frame's step (seconds), for a table coasting sideways.
    dt: f32,
    /// A table still coasts: draw again.
    moving: bool,
    /// The item being drawn (its place in the whole list), for a fold inside it.
    drawing: usize,
    /// When each item shown was written (ms; 0 unknown), for day names and the long press.
    times: Rc<Vec<u64>>,
    /// A long press's menu: the item (its place among those shown) and where the finger was.
    pub menu: Option<(usize, f32)>,
    /// Find in this conversation (A16), while its bar is open.
    pub find: Option<Find>,
    /// A long press on a message of yours still waiting: its place in the queue, where the finger was.
    pub queued_menu: Option<(usize, f32)>,
    /// Items whose fold changed: only they are measured again, on the next frame.
    refolded: HashSet<String>,
    /// Each item's key ([`data::items_timed`]): folds and heights follow it, not its place.
    keys: Rc<Vec<String>>,
    /// The question card last answered (its agent, the card, when): held until its terminal moves on.
    answered: Option<(String, crate::screen::Ask, u64)>,
    /// Items shown beyond the newest [`SHOWN`], once you scrolled up to the oldest shown.
    extra: usize,
    /// Items measured (not drawn) so far: what a new message costs, for tests.
    measured: usize,
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

    /// Measure every item again on the next frame (the text size changed): what is kept by
    /// content alone would keep its old height.
    pub fn relayout(&mut self) {
        self.shown_from = None;
        self.heights = Rc::default();
    }

    pub fn close_over(&mut self) -> bool {
        std::mem::take(&mut self.model_sheet) | self.menu.take().is_some() | self.queued_menu.take().is_some() | self.find.take().is_some()
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
    app.st.chat.drawing = index;
    // A new day starts here: its name above, as Claude's app does.
    let mut top = 0.0;
    if let Some(day) = day_break(app, index) {
        if draw {
            app.canvas.text(&day, WIDTH / 2.0, y + 12.0, Text::new(ty::SMALL, t::INK3).medium().center());
        }
        top = DAY_H;
    }
    top + item_body(app, it, index, y + top, draw)
}

/// The room a day's name takes above its first message.
const DAY_H: f32 = 40.0;

/// The day's name when item `index` is the first of a day: the first one there is, or one
/// written on another day than the one before it.
fn day_break(app: &App, index: usize) -> Option<String> {
    let times = &app.st.chat.times;
    let j = index.checked_sub(app.st.chat.base)?;
    let t = *times.get(j).filter(|t| **t > 0)?;
    let prev = times[..j].iter().rev().find(|t| **t > 0).copied();
    let first_ever = j == 0 && app.st.chat.base == 0;
    match prev {
        Some(p) if local(p).map(|l| (l.year, l.yday)) == local(t).map(|l| (l.year, l.yday)) => None,
        None if !first_ever => None,
        _ => Some(day_name(t, rc_link::link::now_ms())),
    }
}

/// A moment in the phone's own time zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local {
    pub year: i32,
    pub yday: i32,
    pub month: usize,
    pub mday: i32,
    pub wday: usize,
    pub hour: i32,
    pub min: i32,
}

/// `ms` since 1970 in the phone's time zone (the C library's, as Android sets it).
pub fn local(ms: u64) -> Option<Local> {
    let secs = (ms / 1000) as libc::time_t;
    // SAFETY: localtime_r writes only to the tm it is given and reads the zone the process has.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() };
    ok.then(|| Local { year: tm.tm_year + 1900, yday: tm.tm_yday, month: tm.tm_mon.clamp(0, 11) as usize, mday: tm.tm_mday, wday: tm.tm_wday.clamp(0, 6) as usize, hour: tm.tm_hour, min: tm.tm_min })
}

/// "Today", "Yesterday", "Mon 24 Nov", or with its year when it is another year's.
pub fn day_name(ms: u64, now_ms: u64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (Some(d), Some(now)) = (local(ms), local(now_ms)) else { return String::new() };
    let yesterday = local(now_ms.saturating_sub(86_400_000));
    if (d.year, d.yday) == (now.year, now.yday) {
        "Today".into()
    } else if yesterday.is_some_and(|y| (y.year, y.yday) == (d.year, d.yday)) {
        "Yesterday".into()
    } else if d.year == now.year {
        format!("{} {} {}", DAYS[d.wday], d.mday, MONTHS[d.month])
    } else {
        format!("{} {} {} {}", DAYS[d.wday], d.mday, MONTHS[d.month], d.year)
    }
}

fn item_body(app: &mut App, it: &Item, index: usize, y: f32, draw: bool) -> f32 {
    if !draw {
        app.st.chat.measured += 1;
    }
    let c_w = WIDTH - 2.0 * t::PAD;
    match it {
        Item::You(text) => bubble(app, text, y, draw),
        Item::Said(md) => {
            let st = Text::new(ty::READ, t::INK).face(Weight::Serif).line(1.55);
            6.0 + markdown(app, md, t::PAD, y + 6.0, c_w, st, draw)
        }
        Item::Did { line, failed, calls } => {
            let open = app.st.chat.open.contains(&ikey(app, index));
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
                        fold(app, index);
                    }
                }
            }
            // In the order it made them: every edit's change (the owner: "I don't see the code
            // change"), a plan, and, opened or failed, what each call ran and what it answered.
            for (k, c) in calls.iter().enumerate() {
                if c.is_edit() {
                    h += diff(app, c, (ikey(app, index), k), y + h, draw);
                } else if c.is_plan() {
                    h += plan(app, c, (ikey(app, index), k), y + h, draw);
                } else if open || c.failed {
                    h += call(app, c, (ikey(app, index), k), y + h, draw);
                }
            }
            h + 2.0
        }
        Item::Thought(text) => {
            // Shown, not folded away: Claude Code's thinking is often the summary of what it does.
            let open = app.st.chat.open.contains(&ikey(app, index));
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
                            app.st.chat.open.remove(&ikey(app, index));
                            refold(app);
                        } else {
                            app.st.chat.open.insert(ikey(app, index));
                            refold(app);
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
    let mut blocks = 0;
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
                h += code_block(app, &code, blocks, x, y + h, w, draw) + 12.0;
                blocks += 1;
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
fn code_block(app: &mut App, code: &str, n: usize, x: f32, y: f32, w: f32, draw: bool) -> f32 {
    let st = Text::new(ty::MONO, t::INK).face(Weight::Mono).line(1.5);
    // The words keep clear of the copy key at the top right.
    let cw = w - 28.0 - 26.0;
    // Its fold follows its message and its place there, not its words (two alike stay apart).
    let key = (ikey(app, app.st.chat.drawing), n);
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
                refold(app);
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
        match url.strip_prefix(data::FILE_LINK) {
            Some(path) => open_path(app, path),
            None => app.host.open_url(&url),
        }
    }
    h
}

/// The foot's height for a blocked agent with nothing to answer (a line) or a prompt the phone
/// cannot read (Yes and No keys).
const BLOCKED_LINE_H: f32 = 30.0;
const BLOCKED_KEYS_H: f32 = 70.0;

fn blocked_h(app: &App, agent: Option<&Agent>, name: &str) -> f32 {
    match agent.and_then(|a| a.waiting.as_deref()) {
        Some("report") => BLOCKED_LINE_H,
        Some("prompt") => BLOCKED_KEYS_H,
        _ if app.no_screen => BLOCKED_KEYS_H,
        _ if app.screens.contains_key(name) => BLOCKED_LINE_H,
        _ => 0.0,
    }
}

/// The effort the model picker ticks: what the phone sent, until the PC reports it (then that is
/// forgotten), else what the PC reports for the agent (`session.list`).
fn effort_tick(app: &mut App, name: &str, agent: Option<&Agent>) -> Option<&'static str> {
    let real = agent.and_then(|a| a.effort.as_deref()).and_then(|e| EFFORTS.iter().find(|(l, _)| l.eq_ignore_ascii_case(e.trim())).map(|(l, _)| *l));
    if real.is_some() && app.efforts.get(name).copied() == real {
        app.efforts.remove(name);
    }
    app.efforts.get(name).copied().or(real)
}

/// How long an answered card waits for its terminal to move on before it takes taps again.
const ANSWER_LOCK_NS: u64 = 5_000_000_000;

/// The card `q` for `name` was answered a moment ago and its terminal still shows it.
fn answered(app: &App, name: &str, q: &crate::screen::Ask) -> bool {
    app.st.chat.answered.as_ref().is_some_and(|(n, a, at)| n == name && a == q && app.now_ns.saturating_sub(*at) < ANSWER_LOCK_NS)
}

/// Answer card `q`: true when the answer may go (the first tap), and the card is held until the
/// terminal shows something else.
fn answer(app: &mut App, name: &str, q: &crate::screen::Ask) -> bool {
    if answered(app, name, q) {
        return false;
    }
    app.st.chat.answered = Some((name.to_string(), q.clone(), app.now_ns));
    true
}

/// How an answer's words are set on its key.
/// A question's option, at the chosen text size (a fn, as a const would not follow it).
fn option_style() -> Text {
    Text::new(ty::UI, t::INK).medium().line(1.35)
}

/// An answer's words as its key shows them: "(esc)" is the Esc key's job, so the words drop it.
fn option_words(label: &str) -> &str {
    label.strip_suffix("(esc)").map_or(label, str::trim_end)
}

/// An answer's key height: its words wrapped, up to four lines.
fn option_h(app: &mut App, label: &str) -> f32 {
    let w = WIDTH - 2.0 * t::PAD - 48.0;
    (app.canvas.para_h(option_words(label), w, option_style(), 4) + 22.0).max(40.0)
}

/// An answer that is followed by your own words ("No, and tell Claude what to do differently",
/// "Type something", "Other"): the message box takes them.
fn writes(label: &str) -> bool {
    let l = label.to_lowercase();
    l.contains("tell claude") || l.starts_with("type something") || l.starts_with("other") || l.contains("something else")
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

/// Where the long press's menu sits, from where the finger was.
pub fn menu_rect(app: &App) -> Option<[f32; 4]> {
    let (j, fy) = app.st.chat.menu?;
    let mine = matches!(app.st.chat.items.get(j), Some(Item::You(_)));
    let (w, h) = (220.0, MENU_H);
    let x = if mine { WIDTH - t::PAD - w } else { t::PAD };
    let lo = app.canvas.inset_top + TOP_H + 8.0;
    let hi = app.st.chat.box_top - 8.0 - h;
    Some([x, (fy - h - 12.0).max(lo).min(hi.max(lo)), w, h])
}

const MENU_H: f32 = 36.0 + 2.0 * 44.0 + 8.0;

/// A long press's menu: when the message was written, Copy (as written, Markdown kept) and Copy
/// as text (the words as read). It takes every tap while open: a row, or anywhere to close it.
fn message_menu(app: &mut App) {
    let Some([x, y, w, h]) = menu_rect(app) else { return };
    let Some((j, _)) = app.st.chat.menu else { return };
    let Some(it) = app.st.chat.items.get(j).cloned() else {
        app.st.chat.menu = None;
        return;
    };
    let screen = app.canvas.h;
    app.canvas.dy = 0.0;
    kit::above(&mut crate::cx!(app), crate::ui::layer::POPUP, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    app.canvas.shadow(x, y, w, h, 14.0, 20.0, crate::draw::Rgba::hex(0x000000).a(0.45));
    app.canvas.rect(x, y, w, h, 14.0, t::PANEL);
    app.canvas.outline(x, y, w, h, 14.0, 1.0, t::LINE);
    let when = app.st.chat.times.get(j).copied().filter(|t| *t > 0).and_then(|t| local(t).map(|l| format!("{}, {:02}:{:02}", day_name(t, rc_link::link::now_ms()), l.hour, l.min)));
    app.canvas.text_fit(when.as_deref().unwrap_or("When: not in the log"), x + 16.0, y + 11.0, w - 32.0, Text::new(ty::SMALL, t::INK3).medium());
    let mut pick = None;
    for (k, (words, glyph)) in [("Copy", "copy"), ("Copy as text", "file-text")].iter().enumerate() {
        let ry = y + 36.0 + k as f32 * 44.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(x + 4.0, ry, w - 8.0, 44.0) {
            cx.c.rect(x + 4.0, ry, w - 8.0, 44.0, 10.0, t::S2);
        }
        cx.c.icon(glyph, x + 24.0, ry + 22.0, 14.0, t::INK2);
        cx.c.text(words, x + 42.0, ry + (44.0 - Text::new(ty::UI, t::INK).lh()) / 2.0, Text::new(ty::UI, t::INK));
        if cx.tap(x + 4.0, ry, w - 8.0, 44.0) {
            pick = Some(k);
        }
    }
    if let Some(k) = pick {
        let words = if k == 0 { copy_words(&it) } else { plain_words(&it) };
        if let Some(words) = words {
            app.host.copy_text(&words);
            app.toast("Copied");
        }
        app.st.chat.menu = None;
    } else if app.ui.tap_here().is_some() {
        // Anywhere else closes it, and reaches nothing under it.
        app.ui.swallow();
        app.st.chat.menu = None;
    }
}

/// Find in a conversation: what is looked for, the messages that hold it (their keys and places
/// in the whole log, oldest first), which one is shown, and a pending move to it.
#[derive(Default, Clone, Debug)]
pub struct Find {
    pub query: String,
    hits: Vec<(String, usize)>,
    /// For which query and log the hits were found.
    for_: (String, usize, Option<String>),
    pub at: usize,
    jump: Option<String>,
}

/// The find field's id.
const FIND: &str = "find";
const FIND_H: f32 = 52.0;

/// Open the find bar (the chat menu's "Find in chat"), with the keyboard on its field.
pub fn find_open(app: &mut App) {
    app.st.chat.find = Some(Find::default());
    app.focus.id = Some(FIND);
    app.focus.text.clear();
    app.focus.kind = Kb::Text;
    app.focus.sel = None;
    app.host.keyboard(true, "", Kb::Text);
}

/// The words a message is found by: as read (Markdown's marks dropped), with a folded line's
/// calls (what they ran, read or asked).
fn find_text(it: &Item) -> String {
    match it {
        Item::Did { line, calls, .. } => {
            let mut t = line.clone();
            for c in calls {
                t.push('\n');
                t.push_str(&c.summary);
                if let Some(d) = &c.detail {
                    t.push('\n');
                    t.push_str(d);
                }
            }
            t
        }
        other => plain_words(other).unwrap_or_default(),
    }
}

/// The messages of `name`'s whole loaded log that hold `query` (any case): keys and places.
pub fn find_hits(app: &App, name: &str, query: &str) -> Vec<(String, usize)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let msgs = app.convos.get(name).map(|c| c.msgs.as_slice()).unwrap_or_default();
    let (items, _, keys) = data::items_timed(msgs);
    items.iter().zip(keys).enumerate().filter(|(_, (it, _))| find_text(it).to_lowercase().contains(&q)).map(|(i, (_, k))| (k, i)).collect()
}

/// The find bar under the top bar: the field, "3 of 12", older and newer, and close. Returns
/// whether it is open (the page starts under it).
fn find_bar(app: &mut App, name: &str) {
    let Some(mut f) = app.st.chat.find.clone() else { return };
    let y = app.canvas.inset_top + TOP_H;
    app.canvas.dy = 0.0;
    app.canvas.rect(-1000.0, y, WIDTH + 2000.0, FIND_H, 0.0, t::BG);
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, [-1000.0, y, WIDTH + 2000.0, FIND_H]);
    let (x, w) = (12.0, WIDTH - 24.0);
    let (fy, fh) = (y + 6.0, 40.0);
    app.canvas.rect(x, fy, w, fh, 10.0, t::S1);
    app.canvas.icon("search", x + 18.0, fy + fh / 2.0, 14.0, t::INK3);
    // The hits follow the words and the log.
    let log = app.convos.get(name).map(|c| (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()))).unwrap_or_default();
    let mut q = f.query.clone();
    let keys_w = 3.0 * 40.0;
    let count = if f.hits.is_empty() { if q.trim().is_empty() { String::new() } else { "None".into() } } else { format!("{} of {}", f.at + 1, f.hits.len()) };
    let ct = Text::new(ty::SMALL, t::INK3).medium();
    let cw = app.canvas.measure(&count, ct);
    {
        let mut cx = crate::cx!(app);
        kit::inline(&mut cx, FIND, x + 34.0, fy, w - 34.0 - keys_w - cw - 12.0, fh, "Find in this chat", &mut q, Kb::Text, Text::new(ty::UI, t::INK));
    }
    if (q.clone(), log.0, log.1.clone()) != f.for_ {
        let fresh = q != f.query;
        f.hits = find_hits(app, name, &q);
        f.for_ = (q.clone(), log.0, log.1);
        if fresh {
            // The newest first, as a chat is read from its end.
            f.at = f.hits.len().saturating_sub(1);
            f.jump = f.hits.last().map(|h| h.0.clone());
        } else {
            f.at = f.at.min(f.hits.len().saturating_sub(1));
        }
        f.query = q;
    }
    app.canvas.text(&count, x + w - keys_w - cw - 4.0, fy + (fh - ct.lh()) / 2.0, ct);
    let mut step: i32 = 0;
    for (k, glyph) in ["chevron-up", "chevron-down", "close"].iter().enumerate() {
        let kx = x + w - keys_w + k as f32 * 40.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(kx, fy, 40.0, fh) {
            cx.c.rect(kx + 2.0, fy + 2.0, 36.0, fh - 4.0, 8.0, t::WASH);
        }
        cx.c.icon(glyph, kx + 20.0, fy + fh / 2.0, 13.0, t::INK2);
        if cx.tap(kx, fy - 6.0, 40.0, fh + 12.0) {
            step = [-1, 1, 0][k];
            if k == 2 {
                app.st.chat.find = None;
                if app.focus.is(FIND) {
                    app.finish_typing();
                }
                return;
            }
        }
    }
    if step < 0 {
        if f.at > 0 {
            f.at -= 1;
        } else if !app.read_older(name) {
            app.toast("No earlier match");
        }
    } else if step > 0 && f.at + 1 < f.hits.len() {
        f.at += 1;
    }
    if step != 0 {
        f.jump = f.hits.get(f.at).map(|h| h.0.clone());
    }
    app.st.chat.find = Some(f);
}

/// Where the waiting message's menu sits.
pub fn queued_rect(app: &App) -> Option<[f32; 4]> {
    let (_, fy) = app.st.chat.queued_menu?;
    let (w, h) = (220.0, MENU_H);
    let lo = app.canvas.inset_top + TOP_H + 8.0;
    let hi = app.st.chat.box_top - 8.0 - h;
    Some([WIDTH - t::PAD - w, (fy - h - 12.0).max(lo).min(hi.max(lo)), w, h])
}

/// A long press on a message of yours still waiting: Edit (its words back in the box) and
/// Remove. One the PC already has cannot be unsent: it is struck through and the agent still
/// reads it, as the menu says.
fn queued_menu(app: &mut App, name: &str) {
    let (Some([x, y, w, h]), Some((i, _))) = (queued_rect(app), app.st.chat.queued_menu) else { return };
    let Some(p) = app.sending.get(name).and_then(|s| s.waiting.get(i)).cloned() else {
        app.st.chat.queued_menu = None;
        return;
    };
    let screen = app.canvas.h;
    app.canvas.dy = 0.0;
    kit::above(&mut crate::cx!(app), crate::ui::layer::POPUP, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    app.canvas.shadow(x, y, w, h, 14.0, 20.0, crate::draw::Rgba::hex(0x000000).a(0.45));
    app.canvas.rect(x, y, w, h, 14.0, t::PANEL);
    app.canvas.outline(x, y, w, h, 14.0, 1.0, t::LINE);
    let unsent = p.stage == Stage::Queued || p.failed;
    let head = if unsent { "Not sent yet" } else { "Sent · it will still read this" };
    app.canvas.text_fit(head, x + 16.0, y + 11.0, w - 32.0, Text::new(ty::SMALL, t::INK3).medium());
    let mut pick = None;
    for (k, (words, glyph)) in [("Edit", "edit"), ("Remove", "trash")].iter().enumerate() {
        let ry = y + 36.0 + k as f32 * 44.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(x + 4.0, ry, w - 8.0, 44.0) {
            cx.c.rect(x + 4.0, ry, w - 8.0, 44.0, 10.0, t::S2);
        }
        cx.c.icon(glyph, x + 24.0, ry + 22.0, 14.0, t::INK2);
        cx.c.text(words, x + 42.0, ry + (44.0 - Text::new(ty::UI, t::INK).lh()) / 2.0, Text::new(ty::UI, t::INK));
        if cx.tap(x + 4.0, ry, w - 8.0, 44.0) {
            pick = Some(k);
        }
    }
    if let Some(k) = pick {
        app.st.chat.queued_menu = None;
        if let Some(s) = app.sending.get_mut(name) {
            s.take_back(i);
        }
        app.outbox_changed();
        if k == 0 {
            // Its words back in the box, after what is there.
            let typed = app.draft(name);
            let text = if typed.trim().is_empty() { p.text.clone() } else { format!("{}\n{}", typed.trim_end(), p.text) };
            app.set_draft(name, &text);
            app.focus.id = Some(FIELD);
            app.focus.text = text.clone();
            app.focus.kind = Kb::Prose;
            app.focus.sel = None;
            app.host.keyboard(true, &text, Kb::Prose);
        }
    } else if app.ui.tap_here().is_some() {
        app.ui.swallow();
        app.st.chat.queued_menu = None;
    }
}

/// What "Copy as text" copies: the words as read, without Markdown's marks (a table's cells
/// apart by tabs, code as it is).
fn plain_words(it: &Item) -> Option<String> {
    let Item::Said(md) = it else { return copy_words(it) };
    let flat = |s: &str| data::spans(s).into_iter().map(|sp| sp.text).collect::<String>();
    let parts: Vec<String> = data::pieces(md)
        .into_iter()
        .map(|p| match p {
            Piece::Prose(s) | Piece::Heading(s) => flat(&s),
            Piece::Item { mark, text, depth } => format!("{}{mark} {}", "  ".repeat(depth as usize), flat(&text)),
            Piece::Code(c) => c,
            Piece::Table(rows) => rows.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n"),
        })
        .collect();
    Some(parts.join("\n\n"))
}

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
/// How often the live line is drawn again while the agent works.
const LIVE_NS: u64 = 83_000_000;

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
    // It moves while it works: 12 frames a second (the mark turns slowly; more is battery spent
    // on nothing a person sees), only on this page, only while working.
    app.wake_at(now + LIVE_NS);
}

/// An edit's change: the file, then its `-` and `+` lines on red and green, the first dozen
/// shown and the rest a tap away. Returns its height.
/// A fold in the item being drawn opened or closed: its height is measured again.
fn refold(app: &mut App) {
    let k = ikey(app, app.st.chat.drawing);
    app.st.chat.refolded.insert(k);
}

/// Item `index`'s key: what its folds and its kept height go by.
fn ikey(app: &App, index: usize) -> String {
    index.checked_sub(app.st.chat.base).and_then(|j| app.st.chat.keys.get(j)).cloned().unwrap_or_else(|| format!("#{index}"))
}

/// Open or close the folded line of item `index` (its calls), as a tap on it does.
fn fold(app: &mut App, index: usize) {
    let k = ikey(app, index);
    if !app.st.chat.open.remove(&k) {
        app.st.chat.open.insert(k);
    }
    app.st.chat.drawing = index;
    refold(app);
}

/// Flip a call's fold from how it starts.
fn toggle(app: &mut App, key: (String, usize)) {
    if !app.st.chat.toggled.remove(&key) {
        app.st.chat.toggled.insert(key);
    }
    refold(app);
}

/// Lines of output shown before "show all": the tail, where a command says how it ended.
const TAIL: usize = 12;

/// One call, opened: what it ran or asked in full, then what it answered. Returns the height.
fn call(app: &mut App, c: &data::ToolCall, key: (String, usize), y: f32, draw: bool) -> f32 {
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
        // A call on a file (a read, a search in one): its path opens the file.
        match data::file_path(c.summary.trim()).filter(|p| *p == c.summary.trim()) {
            Some(path) => {
                let line = format!("{} {}", c.name, c.summary);
                let lh = if draw { app.canvas.para(&line, x, y + h, w, mono.color(t::ACCENT.mix(t::INK, 0.35)), 3) } else { app.canvas.para_h(&line, w, mono, 3) };
                if draw && crate::cx!(app).tap(x - 4.0, y + h - 4.0, w + 8.0, lh + 8.0) {
                    open_path(app, path);
                }
                h += lh;
            }
            None => put(app, &format!("{} {}", c.name, c.summary), mono, 3, &mut h),
        }
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
fn output(app: &mut App, c: &data::ToolCall, out: &str, key: (String, usize), x: f32, y: f32, w: f32, draw: bool) -> f32 {
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
                toggle(app, key.clone());
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
fn plan(app: &mut App, c: &data::ToolCall, key: (String, usize), y: f32, draw: bool) -> f32 {
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

fn diff(app: &mut App, c: &data::ToolCall, key: (String, usize), y: f32, draw: bool) -> f32 {
    const SHOWN: usize = 14;
    let lines = data::diff_lines(c.detail.as_deref().unwrap_or_default());
    let lang = data::lang_of(&c.summary);
    let st = Text::new(ty::SMALL, t::INK).face(Weight::Mono).line(1.5);
    let lh = st.lh();
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    // The gutter: each line's number when the change says where it is, then its mark.
    let digits = lines.iter().filter_map(|l| l.num).max().map(|n| n.to_string().len());
    let num_w = digits.map_or(0.0, |d| app.canvas.measure(&"0".repeat(d), st.color(t::INK3)) + 8.0);
    let cx0 = x + 10.0 + num_w + 14.0;
    let cw = x + w - 10.0 - cx0;
    // Each line wrapped, never cut, as runs coloured by what they are.
    let mut laid: Vec<(usize, Vec<crate::draw::Run>, f32, usize)> = Vec::new();
    let mut rows = 0;
    for (i, l) in lines.iter().enumerate() {
        let n = match l.kind {
            data::DiffKind::Hunk | data::DiffKind::File => 1,
            _ => {
                let body = l.text.trim_start_matches(' ');
                let indent = app.canvas.measure(&l.text[..l.text.len() - body.len()], st);
                let ink = if l.kind == data::DiffKind::Del { t::INK2 } else { t::INK };
                let runs: Vec<crate::draw::Run> = match lang {
                    Some(lg) => data::tokens(body, lg).into_iter().map(|(k, text)| crate::draw::Run { text, style: st.color(tok_ink(k, ink)), chip: None, underline: false }).collect(),
                    None => vec![crate::draw::Run { text: body.to_string(), style: st.color(ink), chip: None, underline: false }],
                };
                let n = if body.is_empty() { 1 } else { app.canvas.rich_lines(&runs, (cw - indent).max(40.0)).len().max(1) };
                laid.push((i, runs, indent, n));
                n
            }
        };
        rows += n;
        if !draw && rows > 600 {
            break;
        }
    }
    let open = app.st.chat.open_diffs.contains(&key);
    // Folded, whole lines up to SHOWN rows.
    let mut shown_rows = 0;
    let mut shown_lines = 0;
    while shown_lines < lines.len() {
        let n = laid.iter().find(|(i, ..)| *i == shown_lines).map_or(1, |(.., n)| *n);
        if !open && shown_rows + n > SHOWN && shown_rows > 0 {
            break;
        }
        shown_rows += n;
        shown_lines += 1;
    }
    let more = shown_lines < lines.len() || open && rows > SHOWN;
    let head = 26.0;
    let h = head + shown_rows as f32 * lh + if more { 26.0 } else { 8.0 };
    if draw {
        app.canvas.rect(x, y + 2.0, w, h, 10.0, t::CODE);
        app.canvas.outline(x, y + 2.0, w, h, 10.0, 1.0, t::LINE);
        app.canvas.icon("edit", x + 14.0, y + 15.0, 12.0, t::INK3);
        app.canvas.text_fit(&c.summary, x + 26.0, y + 7.0, w - 40.0, Text::new(ty::SMALL, t::INK2).medium());
        // The file's name at the head opens it; a tap below folds or shows the rest.
        if data::file_path(c.summary.trim()).is_some() && crate::cx!(app).tap(x, y + 2.0, w, 26.0) {
            open_path(app, c.summary.trim());
            return h + 8.0;
        }
        let mut ly = y + 2.0 + head;
        for (i, l) in lines.iter().enumerate().take(shown_lines) {
            match l.kind {
                data::DiffKind::Hunk => {
                    // Lines left out between two hunks: a quiet break.
                    app.canvas.rect(x + 10.0, ly + lh / 2.0, w - 20.0, 1.0, 0.0, t::LINE);
                    ly += lh;
                }
                data::DiffKind::File => {
                    app.canvas.text_fit(&l.text, x + 12.0, ly, w - 24.0, Text::new(ty::SMALL, t::INK2).medium().line(1.5));
                    ly += lh;
                }
                kind => {
                    let Some((_, runs, indent, n)) = laid.iter().find(|(j, ..)| *j == i) else { continue };
                    let (bg, mark, mark_ink) = match kind {
                        data::DiffKind::Add => (Some(t::OK.a(0.10)), "+", t::OK),
                        data::DiffKind::Del => (Some(t::DANGER.a(0.10)), "-", t::DANGER),
                        _ => (None, "", t::INK3),
                    };
                    let bh = *n as f32 * lh;
                    if let Some(bg) = bg {
                        app.canvas.rect(x + 1.0, ly, w - 2.0, bh, 0.0, bg);
                    }
                    if let Some(num) = l.num {
                        app.canvas.text(&num.to_string(), x + 10.0 + num_w - 8.0, ly, st.color(t::INK3).right());
                    }
                    app.canvas.text(mark, x + 10.0 + num_w, ly, st.color(mark_ink));
                    app.canvas.rich(runs, cx0 + indent, ly, (cw - indent).max(40.0), lh, *n, true);
                    ly += bh;
                }
            }
        }
        if more {
            let hidden = rows.saturating_sub(shown_rows);
            let words = if open { "Show less".to_string() } else { format!("{hidden} more lines") };
            app.canvas.text(&words, x + 12.0, ly + 4.0, Text::new(ty::SMALL, t::INK3).medium());
            let mut cx = crate::cx!(app);
            if cx.tap(x, y, w, h) {
                if !app.st.chat.open_diffs.remove(&key) {
                    app.st.chat.open_diffs.insert(key);
                }
                refold(app);
            }
        }
    }
    h + 8.0
}

/// Open a file named in the chat in the file view: read from the open chat's agent's checkout
/// (its repo and worktree), or the repo shown when no chat is open (a task's words).
fn open_path(app: &mut App, path: &str) {
    let agent = app.open_chat().and_then(|n| app.agent(&n).cloned());
    let repo = agent.as_ref().map(|a| a.project_id).or(app.repo);
    let Some(repo) = repo else { return };
    crate::fileview::open(app, repo, agent.and_then(|a| a.worktree), path);
}

/// A piece of code's colour: few, quiet, and the line's own ink for the rest.
fn tok_ink(k: data::Tok, plain: crate::draw::Rgba) -> crate::draw::Rgba {
    match k {
        data::Tok::Plain => plain,
        data::Tok::Keyword => t::BADGES[1],
        data::Tok::Str => crate::draw::Rgba::hex(0xD9B77E),
        data::Tok::Number => t::BADGES[4],
        data::Tok::Comment => t::INK3,
    }
}

/// Your words, right-aligned in a bubble. Returns its height.
fn bubble(app: &mut App, text: &str, y: f32, draw: bool) -> f32 {
    bubble_ex(app, text, y, draw, false)
}

/// [`bubble`], struck through when you took it back after the PC had it.
fn bubble_ex(app: &mut App, text: &str, y: f32, draw: bool, struck: bool) -> f32 {
    // A picture you sent is named by its path on the PC: it shows as the picture (or, once the
    // app no longer has its copy, as its name), above your words.
    let (pics, words): (Vec<&str>, Vec<&str>) = text.lines().partition(|l| crate::attach::is_picture(l));
    let mut h = 0.0;
    for p in pics {
        h += picture(app, p.trim(), y + h, draw) + 6.0;
    }
    let words = words.join("\n");
    if words.trim().is_empty() {
        return h + 6.0;
    }
    let y = y + h;
    let st = Text::new(ty::BODY, t::INK).line(1.45);
    let lines = app.canvas.wrap(words.trim(), BUBBLE_W - 32.0, st, 30);
    let tw = lines.iter().map(|l| app.canvas.measure(l, st)).fold(0.0f32, f32::max);
    let (w, bh) = (tw + 32.0, lines.len() as f32 * st.lh() + 24.0);
    if draw {
        let x = WIDTH - t::PAD - w;
        app.canvas.rect(x, y + 6.0, w, bh, 18.0, t::S2);
        let st = if struck { st.color(t::INK3) } else { st };
        for (i, l) in lines.iter().enumerate() {
            let ly = y + 6.0 + 12.0 + i as f32 * st.lh();
            let lw = app.canvas.text(l, x + 16.0, ly, st);
            if struck {
                app.canvas.rect(x + 16.0, ly + st.lh() * 0.5, lw, 1.0, 0.0, t::INK3);
            }
        }
    }
    h + bh + 12.0
}

/// The most a sent picture takes in the chat, in units.
const PIC_MAX: (f32, f32) = (220.0, 240.0);

/// A picture you sent, right-aligned: the app's small copy of it, or its file name on a chip.
/// Returns the height.
fn picture(app: &mut App, path: &str, y: f32, draw: bool) -> f32 {
    let Some(th) = crate::attach::thumb(path) else {
        if draw {
            let name = path.rsplit('/').next().unwrap_or(path);
            let st = Text::new(ty::CAPTION, t::INK2);
            let w = (app.canvas.measure(name, st) + 46.0).min(BUBBLE_W);
            let x = WIDTH - t::PAD - w;
            app.canvas.rect(x, y + 6.0, w, 36.0, 12.0, t::S2);
            app.canvas.icon("file-image", x + 18.0, y + 24.0, 14.0, t::INK2);
            app.canvas.text_fit(name, x + 32.0, y + 6.0 + (36.0 - st.lh()) / 2.0, w - 42.0, st);
        }
        return 42.0;
    };
    let s = (PIC_MAX.0 / th.w as f32).min(PIC_MAX.1 / th.h as f32);
    let (w, h) = ((th.w as f32 * s).round(), (th.h as f32 * s).round());
    if draw {
        let key = format!("sent:{path}");
        if !app.canvas.has_image(&key) && !app.canvas.image_waiting(&key) {
            app.canvas.add_image(&key, th.w, th.h, th.pixels.clone());
        }
        let x = WIDTH - t::PAD - w;
        if !app.canvas.image_cover(&key, x, y + 6.0, w, h, 14.0, crate::draw::Rgba::WHITE) {
            app.canvas.rect(x, y + 6.0, w, h, 14.0, t::S2);
        }
    }
    h + 6.0
}

/// Photos picked go to this agent's checkout; each path, once written, joins the message.
fn attachments(app: &mut App, name: &str) {
    // A photo goes to the agent whose chat it was picked in, and its path into that agent's
    // draft only, wherever you are when it lands.
    for (agent, p) in crate::attach::take_picked() {
        crate::attach::upload(app.link.clone(), agent.unwrap_or_else(|| name.to_string()), p);
        app.st.chat.uploading += 1;
    }
    for (agent, r) in crate::attach::take_done() {
        app.st.chat.uploading = app.st.chat.uploading.saturating_sub(1);
        match r {
            Ok(path) => place_upload(app, name, &agent, path),
            Err(e) => app.toast(&e),
        }
    }
    if app.st.chat.uploading > 0 {
        app.toast("Sending the picture…");
    }
}

/// An uploaded picture's path joins `agent`'s draft: the box being typed in when that is the open
/// chat (`name`), else the draft kept for it.
fn place_upload(app: &mut App, name: &str, agent: &str, path: String) {
    let here = agent == name && app.focus.is(FIELD);
    let typed = if here { app.focus.text.clone() } else { app.draft(agent) };
    let joined = if typed.trim().is_empty() { path } else { format!("{}\n{path}", typed.trim_end()) };
    if here {
        app.focus.text = joined.clone();
        app.focus.sel = None;
        app.host.keyboard(true, &joined, Kb::Prose);
    }
    app.set_draft(agent, &joined);
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
            let opts: f32 = q.options.iter().map(|(_, l)| option_h(app, l) + 6.0).sum();
            16.0 + 20.0 + qh + 10.0 + opts + esc_row
        }
        // Blocked with no card read off its screen. The PC says why (`waiting`): it reported
        // being blocked (nothing to answer: a line), or a prompt is up that the phone could not
        // read (the two answers that always work: Yes and No). An older PC says neither: keys
        // when the phone cannot read the screen, else a line.
        None if asking => blocked_h(app, agent.as_ref(), name),
        None => 0.0,
    };
    // The newest to-do list, pinned while it works with nothing to ask (needs the list read: the
    // log is read into items below, so this frame uses the last frame's list).
    let pin_h = if working && question.is_none() && stopped.is_none() { pinned_h(app) } else { 0.0 };
    let foot_h = if stopped.is_some() { crate::agents::STOPPED_H } else { box_h + 20.0 + ask_h + pin_h + crate::keys::height(app) };

    let find_h = if app.st.chat.find.is_some() { FIND_H } else { 0.0 };
    let page = app.page_begin(dt, TOP_H + find_h, foot_h);
    // The log is read into items and measured only when it or a fold changed: a long
    // conversation costs nothing per frame but what is on screen.
    let (from, found) = match app.convos.get(name) {
        Some(c) => (Some((name.to_string(), c.msgs.len(), c.cursor, c.msgs.last().map(|m| m.id.clone()).unwrap_or_default())), c.found || c.cursor.is_some()),
        None => (None, false),
    };
    if app.st.chat.shown_from != from {
        let msgs = app.convos.get(name).map(|c| c.msgs.as_slice()).unwrap_or_default();
        let (mut items, mut times, mut keys) = data::items_timed(msgs);
        // The to-do list of this turn only: one from before your last message is done with.
        let turn = items.iter().rposition(|it| matches!(it, Item::You(_))).map_or(0, |i| i + 1);
        app.st.chat.todos = items[turn..].iter().rev().find_map(|it| match it {
            Item::Did { calls, .. } => calls.iter().rev().find(|c| c.is_todos()).map(|c| data::todos(c.detail.as_deref().unwrap_or_default())),
            _ => None,
        });
        let base = items.len().saturating_sub(SHOWN + app.st.chat.extra);
        items.drain(..base);
        times.drain(..base);
        keys.drain(..base);
        app.st.chat.times = Rc::new(times);
        app.st.chat.last_said_ms = msgs.iter().rev().find(|m| m.role == "user").and_then(|m| data::parse_ms(&m.created_at));
        // What was already measured keeps its height, found by its key: a new message measures
        // itself alone, however far the list moved (past 400 its oldest are cut).
        let (old, old_keys, old_h) = (app.st.chat.items.clone(), app.st.chat.keys.clone(), app.st.chat.heights.clone());
        let same_log = app.st.chat.shown_from.as_ref().is_some_and(|f| Some(&f.0) == from.as_ref().map(|f| &f.0));
        let old_at: std::collections::HashMap<&str, usize> = if same_log { old_keys.iter().enumerate().map(|(j, k)| (k.as_str(), j)).collect() } else { Default::default() };
        let keys = Rc::new(keys);
        // Measuring reads the new keys and place (day names, folds).
        app.st.chat.keys = keys.clone();
        app.st.chat.base = base;
        let mut hs = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let at = old_at.get(keys[i].as_str()).copied().filter(|&j| j < old.len() && j < old_h.len() && old[j] == *it && i > 0);
            hs.push(match at {
                Some(j) => old_h[j],
                None => item(app, it, base + i, 0.0, false),
            });
        }
        drop(old_at);
        // Older ones came in above what you were reading: the view stays on it.
        if let Some(p) = old_keys.first().and_then(|k0| keys.iter().position(|k| k == k0)).filter(|p| *p > 0 && same_log) {
            let above: f32 = hs[..p].iter().sum();
            app.scroll().off += above;
        }
        app.st.chat.items = Rc::new(items);
        app.st.chat.heights = Rc::new(hs);
        app.st.chat.shown_from = from;
    }
    let items = app.st.chat.items.clone();
    let base = app.st.chat.base;
    if app.st.chat.heights.len() != items.len() {
        let hs: Vec<f32> = items.iter().enumerate().map(|(i, it)| item(app, it, base + i, 0.0, false)).collect();
        app.st.chat.heights = Rc::new(hs);
    }
    // A fold opened or closed: that item alone is measured again (a whole long chat took ~15 ms).
    if !app.st.chat.refolded.is_empty() {
        let mut hs = (*app.st.chat.heights).clone();
        let refolded = std::mem::take(&mut app.st.chat.refolded);
        let keys = app.st.chat.keys.clone();
        for (j, k) in keys.iter().enumerate() {
            if refolded.contains(k) && j < items.len() && j < hs.len() {
                hs[j] = item(app, &items[j], base + j, 0.0, false);
            }
        }
        app.st.chat.heights = Rc::new(hs);
    }
    let heights = app.st.chat.heights.clone();
    if let Some(s) = app.sending.get_mut(name) {
        s.expire_commands(rc_link::link::now_ms());
    }
    let mut waiting = app.sending.get(name).map(|s| s.waiting.clone()).unwrap_or_default();
    // What the PC typed for a send it has not logged yet, from another device or from before a
    // restart: drawn once, as sent, unless it is one of these. A slash command the log never
    // shows is let go after a while.
    let now_ms = rc_link::link::now_ms();
    let extra: Vec<data::Pending> = app
        .convos
        .get(name)
        .map(|c| c.pending.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|p| !waiting.iter().any(|w| w.client_id == p.client_id))
        .map(|p| (p, data::parse_ms(&p.sent_at).unwrap_or(now_ms)))
        .filter(|(p, at)| !(p.text.starts_with('/') && now_ms.saturating_sub(*at) > data::COMMAND_WAIT_MS))
        .map(|(p, at)| data::Pending { text: p.text.clone(), sent_ms: at, failed: false, stage: Stage::Entered, not_before_ms: 0, client_id: p.client_id.clone(), via_chat: true, struck: false })
        .collect();
    waiting.extend(extra);

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
    // Find: to the match asked for, showing more of the log first when it is older than that.
    if let Some(key) = app.st.chat.find.as_ref().and_then(|f| f.jump.clone()) {
        match app.st.chat.keys.iter().position(|k| *k == key) {
            Some(j) => {
                let at: f32 = lead + heights[..j].iter().sum::<f32>() - 12.0;
                app.st.chat.unstuck = true;
                app.scroll().off = at.clamp(0.0, max.max(0.0));
                if let Some(f) = &mut app.st.chat.find {
                    f.jump = None;
                }
            }
            None => {
                let full = app.st.chat.find.as_ref().and_then(|f| f.hits.iter().find(|h| h.0 == key)).map(|h| h.1);
                if let Some(full) = full.filter(|full| *full < app.st.chat.base) {
                    app.st.chat.extra += app.st.chat.base - full + 4;
                    if let Some(f) = &mut app.st.chat.shown_from {
                        f.3.clear();
                    }
                    app.wake_at(app.now_ns);
                } else if let Some(f) = &mut app.st.chat.find {
                    f.jump = None;
                }
            }
        }
    }
    let stick = !app.st.chat.unstuck;
    let sc = app.scroll();
    if stick && !page.moving {
        sc.off = max.max(0.0);
    }
    let off = sc.off;
    app.st.chat.unstuck = off < max - 24.0;
    app.canvas.dy = -off;
    // Scrolled up near the oldest shown: show more of what the phone holds, then read the page
    // older than that from the PC.
    let mut loading = false;
    if app.st.chat.unstuck && off < 400.0 && !items.is_empty() {
        if app.st.chat.base > 0 {
            app.st.chat.extra += SHOWN;
            // Read into items again on the next frame, as the same log (so the view is kept).
            if let Some(f) = &mut app.st.chat.shown_from {
                f.3.clear();
            }
            app.wake_at(app.now_ns);
        } else {
            loading = app.read_older(name);
        }
    }

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
    let hit = app.st.chat.find.as_ref().and_then(|f| f.hits.get(f.at)).map(|h| h.0.clone());
    for (i, it) in items.iter().enumerate() {
        let h = heights[i];
        // Only what is on screen is drawn.
        if y + h >= screen_top && y <= screen_bottom {
            if hit.as_deref() == Some(app.st.chat.keys[i].as_str()) {
                app.canvas.rect(6.0, y, WIDTH - 12.0, h, 12.0, t::ACCENT.a(0.10));
            }
            item(app, it, base + i, y, true);
        }
        if let Some(hy) = held.filter(|hy| *hy >= y && *hy < y + h && copy_words(it).is_some()) {
            copy = Some((i, hy - off));
        }
        y += h;
    }
    // A finger held still on a message: its time, and Copy, in a small menu.
    if let Some(at) = copy {
        app.ui.swallow();
        app.host.haptic();
        app.st.chat.menu = Some(at);
    }
    let mut retry = None;
    // Your own (not another device's): a finger held on one offers Edit and Remove.
    let ours = app.sending.get(name).map_or(0, |s| s.waiting.len());
    let mut take = None;
    for (i, p) in waiting.iter().enumerate() {
        let top = y;
        y += bubble_ex(app, &p.text, y, true, p.struck);
        if i < ours
            && let Some(hy) = held.filter(|hy| *hy >= top && *hy < y)
        {
            take = Some((i, hy - off));
        }
        let online = app.link_state.online();
        let (words, ink) = if p.struck {
            ("Taken back · it will still read this", t::INK3)
        } else if p.failed {
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
    if let Some(at) = take {
        app.ui.swallow();
        app.host.haptic();
        app.st.chat.queued_menu = Some(at);
    }
    if let Some(i) = retry
        && let Some(s) = app.sending.get_mut(name)
    {
        s.retry(i);
        app.outbox_changed();
    }

    if loading {
        // Earlier messages on their way: a quiet line at the top of the page.
        let st = Text::new(ty::SMALL, t::INK3).medium();
        let words = "Loading earlier messages";
        let w = app.canvas.measure(words, st) + 28.0;
        let (lx, ly) = ((WIDTH - w) / 2.0, page.top + 8.0);
        app.canvas.rect(lx, ly, w, 26.0, 13.0, t::S1);
        app.canvas.text(words, WIDTH / 2.0, ly + (26.0 - st.lh()) / 2.0, st.center());
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
    find_bar(app, name);
    let mut moving = page.moving || std::mem::take(&mut app.st.chat.moving);
    match &stopped {
        Some(a) => crate::agents::stopped_foot(app, a),
        None => moving |= composer(app, name, agent.as_ref(), foot_h, box_h, ask_h, question.as_ref(), scr.mode, &typed, lines, working),
    }
    if app.st.chat.model_sheet {
        model_picker(app, name, agent.as_ref());
    }
    if app.st.chat.menu.is_some() {
        message_menu(app);
    }
    if app.st.chat.queued_menu.is_some() {
        queued_menu(app, name);
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
    let pin_h = foot_h - (box_h + 20.0 + ask_h + crate::keys::height(app));
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
        // Answered already: the card waits for the terminal to move on, and a second tap types
        // nothing into whatever comes next.
        let locked = answered(app, name, q);
        let head = if locked { "SENT · WAITING FOR IT" } else { "WAITING FOR YOUR ANSWER" };
        app.canvas.text(head, x + 14.0, y + 14.0, Text::new(ty::SMALL, if locked { t::INK3 } else { t::ACCENT }).semi());
        let qh = question_words(app, &q.question, x + 14.0, y + 36.0, w - 28.0, true);
        let mut oy = y + 36.0 + qh + 10.0;
        let mut pick = None;
        for (i, (n, label)) in q.options.iter().enumerate() {
            let oh = option_h(app, label);
            let mut cx = crate::cx!(app);
            let pressed = cx.pressed(x + 10.0, oy, w - 20.0, oh);
            let fill = if i == 0 { t::ACCENT_DIM } else { t::S2 };
            cx.c.rect(x + 10.0, oy, w - 20.0, oh, t::KEY_R, if pressed { t::S3 } else { fill });
            if i == 0 {
                cx.c.outline(x + 10.0, oy, w - 20.0, oh, t::KEY_R, 1.0, t::ACCENT);
            }
            // A long answer wraps, never cut: its key grows with it.
            cx.c.para(option_words(label), x + 24.0, oy + 11.0, w - 48.0, option_style(), 4);
            if cx.tap(x + 10.0, oy, w - 20.0, oh) {
                pick = Some((n.clone(), writes(label)));
            }
            oy += oh + 6.0;
        }
        // A separate Esc only when no answer already is one.
        if !q.options.iter().any(|(_, l)| l.ends_with("(esc)")) {
            let mut cx = crate::cx!(app);
            let ew = 92.0;
            if kit::plain(&mut cx, x + w - 10.0 - ew, oy, ew, 30.0, "Esc", None) && answer(app, name, q) {
                app.key(name, "\x1b");
            }
        }
        if let Some((n, write)) = pick.filter(|_| answer(app, name, q)) {
            app.key(name, &n);
            // "No, and tell Claude…", "Type something": your words go in the message box next.
            if write {
                let text = app.draft(name);
                app.focus.id = Some(FIELD);
                app.focus.text = text.clone();
                app.focus.kind = Kb::Prose;
                app.focus.sel = None;
                app.host.keyboard(true, &text, Kb::Prose);
            }
        }
        y += ask_h;
    } else if ask_h == BLOCKED_LINE_H {
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
                app.host.keyboard(true, &app.focus.text.clone(), Kb::Prose);
            }
        }
    }

    // The key strip (keys.rs), when it is up: just above the box, under the command list.
    y += crate::keys::strip(app, name, y);
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
                cx.focus.kind = Kb::Prose;
                cx.focus.sel = None;
            }
            if !focused || !cx.focus.keyboard_up {
                cx.host.keyboard(true, &text, Kb::Prose);
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
            crate::attach::pick_for(name);
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
    // Esc, Tab, arrows, Enter for the terminal's own menus (keys.rs): beside send.
    crate::keys::toggle(app, sx - 8.0 - 34.0, ky, 34.0);
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
            // Typing on: the field keeps the keys. Sent from a box not being typed in, the
            // keyboard stays down (raised with no field, it would drop what you type next).
            if app.focus.is(FIELD) {
                app.host.keyboard(true, "", Kb::Prose);
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
    let sent = effort_tick(app, name, agent);
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
    fn a_fold_measures_its_item_again_and_only_it() {
        let dir = std::env::temp_dir().join(format!("relay-code-refold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        crate::sample::tools(&mut a, false);
        a.open("swift-ocelot");
        let mut clock = 1_000_000_000u64;
        let mut frame = |a: &mut App| {
            clock += 16_666_666;
            a.frame(clock);
        };
        frame(&mut a);
        frame(&mut a);
        let j = a.st.chat.items.iter().rposition(|it| matches!(it, Item::Did { .. })).unwrap();
        let before = a.st.chat.heights[j];
        let at = a.st.chat.base + j;
        fold(&mut a, at);
        frame(&mut a);
        let (items, base) = (a.st.chat.items.clone(), a.st.chat.base);
        let fresh: Vec<f32> = items.iter().enumerate().map(|(i, it)| item(&mut a, it, base + i, 0.0, false)).collect();
        assert_eq!(*a.st.chat.heights, fresh, "the heights kept agree with measuring everything");
        assert!(a.st.chat.heights[j] > before, "the opened item grew");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opening a fold in a long conversation (400 messages, each with thinking, a 60-line diff
    /// and prose): the frame after the tap, release build on this machine's CPU:
    /// `cargo test --release fold_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fold_cost() {
        let dir = std::env::temp_dir().join(format!("relay-code-fold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        let long = "The phone matched its own copy to the log by the words, and a queued message is logged differently, so the match missed and both stayed. ".repeat(3);
        let diff = (0..30).map(|i| format!("-    let old_{i} = users.any {{ it.clientId == mine.id }}\n+    let new_{i} = users.any {{ it.id == mine.id }}")).collect::<Vec<_>>().join("\n");
        let msgs: Vec<serde_json::Value> = (0..400)
            .map(|i| {
                if i % 2 == 0 {
                    serde_json::json!({"id": i.to_string(), "role": "user", "created_at": "", "body": {"text": format!("Message {i}: push it when the tests pass.")}})
                } else {
                    serde_json::json!({"id": i.to_string(), "role": "assistant", "created_at": "", "body": {"blocks": [
                        {"type": "thinking", "text": long},
                        {"type": "tool_use", "id": format!("c{i}"), "name": "Bash", "summary": "cargo test"},
                        {"type": "tool_use", "id": format!("e{i}"), "name": "Edit", "summary": "data/SessionChat.kt", "detail": diff},
                        {"type": "text", "text": long},
                    ]}})
                }
            })
            .collect();
        let mut c = crate::data::Convo::default();
        c.take(&serde_json::json!({"cursor": 1, "reset": true, "found": true, "messages": msgs}));
        a.convos.insert("bold-puffin".into(), c);
        a.open("bold-puffin");
        let mut clock = 1_000_000_000u64;
        for _ in 0..5 {
            clock += 16_666_666;
            a.frame(clock);
        }
        let did = a.st.chat.base + a.st.chat.items.iter().rposition(|it| matches!(it, Item::Did { .. })).unwrap();
        let mut times = Vec::new();
        for _ in 0..41 {
            fold(&mut a, did);
            clock += 16_666_666;
            let t = std::time::Instant::now();
            a.frame(clock);
            times.push(t.elapsed().as_secs_f64() * 1e6);
        }
        times.sort_by(f64::total_cmp);
        println!("frame after a fold: p50 {:.0} µs, p95 {:.0} µs, max {:.0} µs ({} runs)", times[20], times[38], times[40], times.len());
        // Measuring every message shown, as opening the chat does.
        let (items, base) = (a.st.chat.items.clone(), a.st.chat.base);
        let mut all = Vec::new();
        for _ in 0..11 {
            let t = std::time::Instant::now();
            for (i, it) in items.iter().enumerate() {
                item(&mut a, it, base + i, 0.0, false);
            }
            all.push(t.elapsed().as_secs_f64() * 1e3);
        }
        all.sort_by(f64::total_cmp);
        println!("measuring all {} items: p50 {:.1} ms, max {:.1} ms (11 runs)", items.len(), all[5], all[10]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn answering_a_question_wraps_writes_and_knows_why_it_is_blocked() {
        let dir = std::env::temp_dir().join(format!("relay-code-answer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        // A long answer wraps: its key grows instead of cutting it.
        let long = "Yes, and don't ask again for git push commands in /home/anthony/dev3/RelayV4/.relay/worktrees/umber-zebra";
        assert!(option_h(&mut a, long) > option_h(&mut a, "Yes") + 10.0);
        // Answers your own words follow put the caret in the message box.
        assert!(writes("No, and tell Claude what to do differently (esc)"));
        assert!(writes("Type something."));
        assert!(!writes("Yes, and auto-accept edits"));
        // Blocked: a report gets a line, a prompt the phone cannot read gets Yes and No.
        let mut ag = a.agent("umber-zebra").cloned().unwrap();
        ag.waiting = Some("report".into());
        assert_eq!(blocked_h(&a, Some(&ag), "umber-zebra"), BLOCKED_LINE_H);
        ag.waiting = Some("prompt".into());
        assert_eq!(blocked_h(&a, Some(&ag), "umber-zebra"), BLOCKED_KEYS_H);
        // Effort: the phone's choice until the PC reports it, then the PC's.
        ag.effort = Some("medium".into());
        assert_eq!(effort_tick(&mut a, "umber-zebra", Some(&ag)), Some("medium"));
        a.efforts.insert("umber-zebra".into(), "high");
        assert_eq!(effort_tick(&mut a, "umber-zebra", Some(&ag)), Some("high"), "sent, not reported yet");
        ag.effort = Some("High".into());
        assert_eq!(effort_tick(&mut a, "umber-zebra", Some(&ag)), Some("high"));
        assert!(!a.efforts.contains_key("umber-zebra"), "reported: the PC's word from now on");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_composers_chips_take_a_tap_44_tall() {
        let dir = std::env::temp_dir().join(format!("relay-code-chips-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.open("bold-puffin");
        let clock = std::cell::Cell::new(1_000_000_000u64);
        let frames = |a: &mut App, n: usize| {
            for _ in 0..n {
                clock.set(clock.get() + 16_666_666);
                a.frame(clock.get());
            }
        };
        frames(&mut a, 3);
        // The chip is drawn 34 tall from `ky`; its tap reaches past it above and below.
        let lh = Text::new(ty::BODY, t::INK).line(1.4).lh();
        let ky = a.st.chat.box_top + 14.0 + lh + 10.0;
        // The words above take a tap down to 4 over it: 44 from there to 6 under it.
        for y in [ky - 3.0, ky + 34.0 + 5.0] {
            a.st.chat.model_sheet = false;
            let t0 = clock.get();
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Down, x: 100.0, y, t_ns: t0 });
            frames(&mut a, 1);
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Up, x: 100.0, y, t_ns: t0 + 40_000_000 });
            frames(&mut a, 2);
            assert!(a.st.chat.model_sheet, "a tap at {y} (chip drawn from {ky}) opened the model sheet");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn fresh_app(tag: &str) -> (App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("relay-code-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        (a, dir)
    }

    #[test]
    fn audit_reading_the_log() {
        let (mut a, dir) = fresh_app("audit-log");
        let msg = |i: usize| serde_json::json!({"id": format!("m{i}"), "role": "assistant", "created_at": "", "body": {"blocks": [{"type": "text", "text": "x"}]}});
        a.took_transcript("x", &serde_json::json!({"messages": [msg(0)], "cursor": 10, "reset": true, "found": true}));
        // `/clear`: what you entered before is in the reset log, so it stops waiting.
        let mut s = data::Sending::default();
        s.push("/clear", 0);
        s.next(0);
        s.answered(data::Answer::Ok, 0);
        s.next(data::ENTER_AFTER_MS);
        s.answered(data::Answer::Ok, data::ENTER_AFTER_MS);
        a.sending.insert("x".into(), s);
        a.took_transcript("x", &serde_json::json!({"messages": [msg(1)], "cursor": 20, "reset": true, "found": true}));
        assert!(a.sending["x"].waiting.is_empty(), "no second bubble after a reset");
        // Catching up: a full page is read on at once, a short one waits its turn.
        a.reread_at = 5_000_000_000;
        a.took_transcript("x", &serde_json::json!({"messages": [msg(2)], "cursor": 30, "reset": false, "found": true}));
        assert_eq!(a.reread_at, 5_000_000_000);
        let page: Vec<_> = (3..203).map(msg).collect();
        a.took_transcript("x", &serde_json::json!({"messages": page, "cursor": 40, "reset": false, "found": true}));
        assert_eq!(a.reread_at, 0, "a full page: the next one at once");
        // Back online, an older PC's missing foot is asked for again.
        a.no_screen = true;
        a.came_online();
        assert!(!a.no_screen);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_a_chats_copy_is_read_off_the_ui_thread() {
        let (mut a, dir) = fresh_app("audit-open");
        let c = data::Convo { msgs: vec![], cursor: Some(7), found: true, ..Default::default() };
        std::fs::create_dir_all(dir.join("cache")).unwrap();
        std::fs::write(dir.join("cache/chat-swift-ocelot.json"), serde_json::to_vec(&c).unwrap()).unwrap();
        a.open("swift-ocelot");
        assert!(!a.convos.contains_key("swift-ocelot"), "not read on the UI thread");
        for i in 0..200 {
            a.tick(2_000_000_000 + i * 10_000_000);
            if a.convos.contains_key("swift-ocelot") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(a.convos.get("swift-ocelot").and_then(|c| c.cursor), Some(7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_past_400_messages_folds_and_heights_follow_their_message() {
        let (mut a, dir) = fresh_app("audit-keys");
        a.sample();
        let msg = |i: usize| {
            if i.is_multiple_of(2) {
                serde_json::json!({"id": format!("m{i}"), "role": "user", "created_at": "", "body": {"text": format!("go {i}")}})
            } else {
                serde_json::json!({"id": format!("m{i}"), "role": "assistant", "created_at": "", "body": {"blocks": [
                    {"type": "tool_use", "id": format!("c{i}"), "name": "Bash", "summary": "cargo test"},
                    {"type": "text", "text": format!("done {i}\n```\nsame code\n```")}]}})
            }
        };
        let mut c = data::Convo::default();
        c.take(&serde_json::json!({"messages": (0..400).map(msg).collect::<Vec<_>>(), "cursor": 1, "reset": true, "found": true}));
        a.convos.insert("bold-puffin".into(), c);
        a.open("bold-puffin");
        let mut clock = 1_000_000_000u64;
        let mut frame = |a: &mut App| {
            clock += 16_666_666;
            a.frame(clock);
        };
        frame(&mut a);
        // Open the last folded line, and the second-last message's code block.
        let did = a.st.chat.items.iter().rposition(|it| matches!(it, Item::Did { .. })).unwrap();
        let did_key = a.st.chat.keys[did].clone();
        let at0 = a.st.chat.base + did;
        fold(&mut a, at0);
        frame(&mut a);
        // Two more messages: the 160 shown move along by their three items.
        let more = serde_json::json!({"messages": [msg(400), msg(401)], "cursor": 2, "reset": false, "found": true});
        a.convos.get_mut("bold-puffin").unwrap().take(&more);
        let before = a.st.chat.measured;
        frame(&mut a);
        let measured = a.st.chat.measured - before;
        assert!(measured <= 4, "only the three new items (and the first, for its day) are measured, not 160: {measured}");
        let at = a.st.chat.keys.iter().position(|k| *k == did_key).unwrap();
        assert!(a.st.chat.open.contains(&a.st.chat.keys[at]), "the fold stayed with its message");
        assert!(matches!(a.st.chat.items[at], Item::Did { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scrolled_to_the_top_earlier_messages_show_and_the_view_stays() {
        let (mut a, dir) = fresh_app("older");
        a.sample();
        let msg = |i: usize| serde_json::json!({"id": format!("m{i}"), "role": "user", "created_at": "", "body": {"text": format!("message {i}")}});
        let mut c = data::Convo::default();
        c.take(&serde_json::json!({"messages": (0..300).map(msg).collect::<Vec<_>>(), "cursor": 1, "before": 50, "older": true, "reset": true, "found": true}));
        a.convos.insert("bold-puffin".into(), c);
        a.open("bold-puffin");
        let mut clock = 1_000_000_000u64;
        let mut frame = |a: &mut App| {
            clock += 16_666_666;
            a.frame(clock);
        };
        frame(&mut a);
        let (base, first) = (a.st.chat.base, a.st.chat.keys[0].clone());
        assert!(base > 0, "the newest 160 of 300 are shown");
        // Up at the oldest shown: the next 160 the phone holds come in above it.
        a.st.chat.unstuck = true;
        a.scroll().off = 0.0;
        frame(&mut a);
        frame(&mut a);
        assert!(a.st.chat.base < base, "more shown");
        let at = a.st.chat.keys.iter().position(|k| *k == first).unwrap();
        let above: f32 = a.st.chat.heights[..at].iter().sum();
        assert!((a.scroll().off - above).abs() < 1.0, "the view stayed on what you were reading: {} vs {above}", a.scroll().off);
        // A page from the PC goes on top too (here as its answer would land).
        let page: Vec<_> = (300..340).map(|i| serde_json::json!({"id": format!("p{i}"), "role": "user", "created_at": "", "body": {"text": "older"}})).collect();
        a.convos.get_mut("bold-puffin").unwrap().take_older(&serde_json::json!({"messages": page, "before": 0, "older": false}));
        assert_eq!(a.convos["bold-puffin"].msgs[0].id, "p300");
        assert!(!a.convos["bold-puffin"].older);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_in_the_chat_opens_in_its_agents_checkout() {
        let (mut a, dir) = fresh_app("paths");
        a.sample();
        let mut ag = a.agent("swift-ocelot").cloned().unwrap();
        ag.worktree = Some("/wt/swift-ocelot".into());
        a.agents.retain(|x| x.name != "swift-ocelot");
        a.agents.push(ag);
        a.open("swift-ocelot");
        open_path(&mut a, "src/chat.rs:42");
        assert_eq!(a.route(), &crate::app::Route::File { repo: 1, worktree: Some("/wt/swift-ocelot".into()), path: "src/chat.rs:42".into() });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_waiting_message_held_can_be_edited_or_removed() {
        let (mut a, dir) = fresh_app("take-back");
        a.sample();
        a.open("bold-puffin");
        a.sending.insert("bold-puffin".into(), data::Sending::default());
        a.say("bold-puffin", "first thought");
        a.say("bold-puffin", "second thought");
        let mut clock = 1_000_000_000u64;
        let mut frame = |a: &mut App| {
            clock += 16_666_666;
            a.frame(clock);
        };
        frame(&mut a);
        let tap = |a: &mut App, x: f32, y: f32, clock: u64| {
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Down, x, y, t_ns: clock });
            a.frame(clock + 16_666_666);
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Up, x, y, t_ns: clock + 40_000_000 });
            a.frame(clock + 50_000_000);
            a.frame(clock + 70_000_000);
        };
        // Held on the second: Remove takes it out of the queue (in a shot nothing is sent).
        a.st.chat.queued_menu = Some((1, 500.0));
        frame(&mut a);
        let [x, y, w, _] = queued_rect(&a).unwrap();
        tap(&mut a, x + w / 2.0, y + 36.0 + 44.0 + 22.0, 3_000_000_000);
        assert_eq!(a.sending["bold-puffin"].waiting.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["first thought"]);
        // Held on the first: Edit puts its words in the box and takes it out.
        a.st.chat.queued_menu = Some((0, 500.0));
        a.frame(3_200_000_000);
        let [x, y, w, _] = queued_rect(&a).unwrap();
        tap(&mut a, x + w / 2.0, y + 36.0 + 22.0, 3_300_000_000);
        assert!(a.sending["bold-puffin"].waiting.is_empty());
        assert_eq!(a.draft("bold-puffin"), "first thought");
        assert!(a.focus.is(FIELD));
        // A tap beside the menu closes it and reaches nothing.
        a.say("bold-puffin", "third");
        a.st.chat.queued_menu = Some((0, 500.0));
        a.frame(3_500_000_000);
        tap(&mut a, 30.0, 300.0, 3_600_000_000);
        assert!(a.st.chat.queued_menu.is_none());
        assert_eq!(a.sending["bold-puffin"].waiting.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_steps_through_the_messages_that_hold_the_words() {
        let (mut a, dir) = fresh_app("find");
        a.sample();
        let msg = |i: usize| serde_json::json!({"id": format!("m{i}"), "role": "assistant", "created_at": "", "body": {"blocks": [{"type": "text", "text": if i % 50 == 7 { format!("the **needle** at {i}") } else { format!("hay {i}") }}]}});
        let mut c = data::Convo::default();
        c.take(&serde_json::json!({"messages": (0..300).map(msg).collect::<Vec<_>>(), "cursor": 1, "reset": true, "found": true}));
        a.convos.insert("bold-puffin".into(), c);
        a.open("bold-puffin");
        let mut clock = 1_000_000_000u64;
        let mut frame = |a: &mut App| {
            clock += 16_666_666;
            a.frame(clock);
        };
        frame(&mut a);
        find_open(&mut a);
        a.set_text("Needle");
        frame(&mut a);
        frame(&mut a);
        let f = a.st.chat.find.clone().unwrap();
        // Six messages hold it (7, 57, …, 257), the marks dropped; the newest is shown first.
        assert_eq!(f.hits.len(), 6);
        assert_eq!(f.at, 5);
        assert!(a.st.chat.unstuck, "scrolled to it");
        // Older, five times: the oldest (m7) is outside the 160 shown, so more is shown to reach it.
        let (x, w) = (12.0, WIDTH - 24.0);
        let up = (x + w - 120.0 + 20.0, a.canvas.inset_top + TOP_H + 6.0 + 20.0);
        for k in 0..5 {
            let t0 = 2_000_000_000 + k * 200_000_000;
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Down, x: up.0, y: up.1, t_ns: t0 });
            a.frame(t0 + 16_666_666);
            a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Up, x: up.0, y: up.1, t_ns: t0 + 40_000_000 });
            for n in 0..4 {
                a.frame(t0 + 50_000_000 + n * 16_666_666);
            }
        }
        let f = a.st.chat.find.clone().unwrap();
        assert_eq!(f.at, 0);
        assert!(a.st.chat.keys.iter().any(|k| *k == f.hits[0].0), "the oldest match is shown");
        assert!(f.jump.is_none(), "and scrolled to");
        // Back closes it.
        assert!(a.st.chat.close_over());
        assert!(a.st.chat.find.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_the_todo_list_pinned_is_this_turns() {
        let (mut a, dir) = fresh_app("audit-todos");
        a.sample();
        let todo = serde_json::json!({"type": "tool_use", "id": "t", "name": "TodoWrite", "summary": "", "detail": "[x] a\n[~] b"});
        let mut c = data::Convo::default();
        c.take(&serde_json::json!({"messages": [
            {"id": "1", "role": "assistant", "created_at": "", "body": {"blocks": [todo]}},
            {"id": "2", "role": "user", "created_at": "", "body": {"text": "something new"}},
        ], "cursor": 1, "reset": true, "found": true}));
        a.convos.insert("bold-puffin".into(), c);
        a.open("bold-puffin");
        a.frame(2_000_000_000);
        assert!(a.st.chat.todos.is_none(), "last turn's list is not pinned on this one");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_a_card_takes_one_answer_until_its_terminal_moves_on() {
        let (mut a, dir) = fresh_app("audit-card");
        let q = crate::screen::read("Proceed?\n❯ 1. Yes\n  2. No").ask.unwrap();
        assert!(answer(&mut a, "x", &q), "the first tap answers");
        assert!(!answer(&mut a, "x", &q), "a second tap on the same card types nothing");
        // The terminal moved on to another card: it takes its answer.
        let next = crate::screen::read("Then this?\n❯ 1. Yes\n  2. No").ask.unwrap();
        assert!(answer(&mut a, "x", &next));
        // Or the same card is still up a while later (the answer was lost): it may go again.
        a.now_ns += ANSWER_LOCK_NS + 1;
        assert!(answer(&mut a, "x", &next));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_an_upload_goes_to_its_own_agents_draft() {
        let (mut a, dir) = fresh_app("audit-upload");
        a.set_draft("b", "see this");
        a.focus.id = Some(FIELD);
        a.focus.text = "typing to a".into();
        // Picked in b's chat, landed while a's is open and being typed in.
        place_upload(&mut a, "a", "b", "/wt/.relay/uploads/x.png".into());
        assert_eq!(a.draft("b"), "see this\n/wt/.relay/uploads/x.png");
        assert_eq!(a.focus.text, "typing to a", "a's box is untouched");
        assert_eq!(a.draft("a"), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_send_with_the_keyboard_down_leaves_it_down() {
        struct KbHost(std::path::PathBuf, std::sync::Mutex<Vec<bool>>);
        impl crate::app::Host for KbHost {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _text: &str) {}
            fn keyboard(&self, show: bool, _text: &str, _kind: Kb) {
                self.1.lock().unwrap().push(show);
            }
        }
        let dir = std::env::temp_dir().join(format!("relay-code-audit-send-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let host = std::sync::Arc::new(KbHost(dir.clone(), Default::default()));
        let mut a = App::new(host.clone(), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.open("swift-ocelot");
        a.set_draft("swift-ocelot", "ship it");
        let mut clock = 1_000_000_000u64;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        host.1.lock().unwrap().clear();
        // The send key: at the box's right, on the row under the words.
        let lh = Text::new(ty::BODY, t::INK).line(1.4).lh();
        let ky = a.st.chat.box_top + 14.0 + lh + 10.0;
        let (x, y) = (12.0 + (WIDTH - 24.0) - 10.0 - 17.0, ky + 17.0);
        a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Down, x, y, t_ns: clock });
        clock += 16_666_666;
        a.frame(clock);
        a.touch(crate::ui::Touch { id: 1, phase: crate::ui::Phase::Up, x, y, t_ns: clock + 30_000_000 });
        for _ in 0..2 {
            clock += 16_666_666;
            a.frame(clock);
        }
        assert_eq!(a.sending["swift-ocelot"].waiting.last().map(|p| p.text.as_str()), Some("ship it"), "it was sent");
        assert!(!host.1.lock().unwrap().contains(&true), "the keyboard was not raised with no field to take its keys");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn days_are_named_as_people_say_them() {
        let now = 1_795_000_000_000u64;
        assert_eq!(day_name(now, now), "Today");
        assert_eq!(day_name(now - 86_400_000, now), "Yesterday");
        let n = day_name(now - 3 * 86_400_000, now);
        assert!(n.split(' ').count() == 3, "a weekday, a date and a month: {n}");
        assert!(day_name(now - 400 * 86_400_000, now).ends_with(&local(now - 400 * 86_400_000).unwrap().year.to_string()));
    }

    #[test]
    fn models_read_as_people_say_them() {
        assert_eq!(model_words(Some("claude-opus-5-5")), "Opus 5.5");
        assert_eq!(model_words(Some("sonnet")), "Sonnet");
        assert_eq!(model_words(Some("claude-haiku-5-5-20261001")), "Haiku 5.5");
        assert_eq!(model_words(None), "Default model");
    }
}
