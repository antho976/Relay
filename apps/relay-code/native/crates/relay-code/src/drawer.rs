//! The drawer: route A of design 44 (D4), like Claude's app. The repo on top (one tap switches it,
//! D5), New agent, what needs you, this repo's agents, and the PC at the foot. Opened by the menu
//! key or a swipe from the left edge. Also the repo sheet and the New agent sheet.

use rc_link::LinkState;
use serde_json::json;

use crate::app::{App, Ask, Route};
use crate::data::Agent;
use crate::draw::{Canvas, Text, WIDTH, Weight};
use crate::kit::{self, Kb};
use crate::theme::{self as t, ty};

const W: f32 = 330.0;
/// A swipe that starts this close to the left edge opens the drawer.
const EDGE: f32 = 24.0;

#[derive(Default)]
pub struct State {
    pub open: bool,
    /// How far out it is, 0 to 1, eased toward `open`.
    pos: f32,
    /// The repo sheet is up.
    pub repos: bool,
    swiping: bool,
    pub scroll: crate::ui::Scroll,
}

impl State {
    pub fn dragging(&self) -> bool {
        self.swiping
    }

    /// Close whatever of this is open; false when nothing was.
    pub fn close_over(&mut self) -> bool {
        if self.repos {
            self.repos = false;
            return true;
        }
        if self.open {
            self.open = false;
            return true;
        }
        false
    }
}

#[derive(Default)]
pub struct NewAgent {
    pub open: bool,
    pub task: String,
    /// The board task it is for, by id and title.
    pub linked: Option<(i64, String)>,
    /// The task list is up in place of the sheet.
    pub picking: bool,
    /// The repo's tasks, as last read for the list.
    pub tasks: Vec<crate::board::Task>,
    pub scroll: crate::ui::Scroll,
}

/// An agent's badge: two letters on its own colour, and its lamp.
pub fn badge(c: &mut Canvas, a: &Agent, x: f32, y: f32, size: f32) {
    badge_lit(c, a, x, y, size, a.working());
}

/// [`badge`], its green lamp lit when `working` says so (the rows pass [`App::busy`]).
pub fn badge_lit(c: &mut Canvas, a: &Agent, x: f32, y: f32, size: f32, working: bool) {
    let col = t::BADGES[a.colour()];
    c.rect(x, y, size, size, size * 0.3, col.a(0.2));
    let tt = Text::new(size * 0.38, col).semi().center();
    c.text(&a.initials(), x + size / 2.0, y + (size - tt.lh()) / 2.0, tt);
    let lamp = if a.needs_you() {
        Some(t::WARN)
    } else if working {
        Some(t::OK)
    } else {
        None
    };
    if let Some(l) = lamp {
        c.circle(x + size - 2.0, y + size - 2.0, 5.5, t::BG);
        c.circle(x + size - 2.0, y + size - 2.0, 3.8, l);
    }
}

/// An agent as a row: badge, what it is doing, its name and state, and when it last spoke. True
/// when tapped.
pub fn agent_row(app: &mut App, a: &Agent, x: f32, y: f32, w: f32, now_ms: u64) -> bool {
    let h = 60.0;
    let working = app.busy(&a.name);
    let words = app.agent_words(a);
    let mut cx = crate::cx!(app);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x + 6.0, y + 2.0, w - 12.0, h - 4.0, 10.0, t::WASH);
    }
    badge_lit(cx.c, a, x + t::PAD, y + 14.0, 32.0, working);
    let tx = x + t::PAD + 32.0 + 12.0;
    // "asks you" only for a question in its terminal; an agent that stopped blocked says so
    // (state_words reads session.list's `waiting`).
    let when = a.last_output_at.as_deref().or(Some(a.updated_at.as_str()).filter(|s| !s.is_empty())).map(|s| crate::data::ago(s, now_ms)).unwrap_or_default();
    let wt = Text::new(ty::SMALL, t::INK3);
    let ww = if when.is_empty() { 0.0 } else { cx.c.measure(&when, wt) + 8.0 };
    // The two lines centred on the row by their own heights, so a larger text size keeps them apart.
    let (tt, st) = (Text::new(ty::BODY, t::INK).semi(), Text::new(ty::CAPTION, if a.needs_you() { t::WARN } else { t::INK3 }));
    let ty0 = y + (h - (tt.lh() + 1.5 + st.lh())) / 2.0;
    cx.c.text_fit(a.title(), tx, ty0, x + w - tx - t::PAD - ww, tt);
    let sub = format!("{} · {words}", a.name);
    cx.c.text_fit(&sub, tx, ty0 + tt.lh() + 1.5, x + w - tx - t::PAD, st);
    if !when.is_empty() {
        cx.c.text(&when, x + w - t::PAD, ty0 + (tt.lh() - wt.lh()) / 2.0, wt.right());
    }
    cx.tap(x, y, w, h)
}

/// The repo, as the top bar shows it: a chip that opens the repo sheet. Returns its width.
pub fn repo_chip(app: &mut App, right: f32, y: f32) -> f32 {
    let name = app.repo_name();
    let mut cx = crate::cx!(app);
    let tt = Text::new(ty::UI, t::INK).semi();
    // Folder, words, chevron: the words get their own width and a little more, so a name that
    // fits is never cut by rounding (D5: "Relay…" with room to spare).
    let tw = cx.c.measure(&name, tt).min(170.0) + 2.0;
    let w = tw + 32.0 + 30.0;
    let x = right - w;
    let h = 34.0;
    cx.c.rect(x, y, w, h, 10.0, if cx.pressed(x, y, w, h) { t::S2 } else { t::S1 });
    cx.c.icon("folder", x + 18.0, y + h / 2.0, 15.0, t::INK2);
    cx.c.text_fit(&name, x + 32.0, y + (h - tt.lh()) / 2.0, tw, tt);
    cx.c.icon("chevron-down", x + w - 16.0, y + h / 2.0, 12.0, t::INK3);
    if cx.tap(x - 4.0, y - 5.0, w + 8.0, h + 10.0) {
        app.st.drawer.repos = true;
    }
    w
}

/// The drawer and the repo sheet, over the page. True while moving.
pub fn draw(app: &mut App, dt: f32) -> bool {
    // A swipe from the left edge opens it; one to the left closes it.
    if app.ui.horizontal
        && let Some((fx, _)) = app.ui.drag_from
    {
        if !app.st.drawer.open && fx < EDGE && app.ui.drag_dx > 0.0 {
            app.st.drawer.swiping = true;
            app.st.drawer.open = true;
        } else if app.st.drawer.open && app.ui.drag_dx < -2.0 {
            app.st.drawer.open = false;
        }
    }
    if !app.ui.busy() {
        app.st.drawer.swiping = false;
    }
    let target = if app.st.drawer.open { 1.0 } else { 0.0 };
    let d = &mut app.st.drawer;
    let moving = (d.pos - target).abs() > 0.002;
    d.pos += (target - d.pos) * (1.0 - (-dt * 16.0).exp());
    if !moving {
        d.pos = target;
    }
    let pos = d.pos;
    let mut out = moving;
    if pos > 0.0 {
        out |= panel(app, pos);
    }
    if app.st.drawer.repos {
        repos(app);
    }
    out
}

fn panel(app: &mut App, pos: f32) -> bool {
    let saved = app.canvas.dy;
    app.canvas.dy = 0.0;
    app.ui.cover();
    let screen = app.canvas.h;
    let x0 = -W * (1.0 - crate::draw::ease_out(pos));
    // The drawer and its veil cover the whole page: no tap reaches it while the drawer is out.
    kit::above(&mut crate::cx!(app), crate::ui::layer::DRAWER, [-1000.0, 0.0, WIDTH + 2000.0, screen]);
    app.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, screen, 0.0, t::VEIL.a(pos));
    // A tap beside the drawer closes it.
    if app.ui.tap_here().is_some_and(|(x, _)| x > x0 + W) {
        app.ui.tap = None;
        app.st.drawer.open = false;
    }
    app.canvas.rect(x0 - 40.0, 0.0, W + 40.0, screen, 24.0, t::PANEL);
    let top = app.canvas.inset_top + 8.0;

    // The repo
    let (rx, ry, rw, rh) = (x0 + 10.0, top, W - 20.0, 56.0);
    let name = app.repo_name();
    let agents: Vec<Agent> = app.repo_agents().into_iter().cloned().collect();
    let asking = agents.iter().filter(|a| a.needs_you()).count();
    let branch = app.repo.and_then(|r| app.projects.iter().find(|p| p.id == r)).map(|p| p.base_branch.clone()).unwrap_or_default();
    {
        let mut cx = crate::cx!(app);
        if cx.pressed(rx, ry, rw, rh) {
            cx.c.rect(rx, ry, rw, rh, 12.0, t::WASH);
        }
        cx.c.icon("folder", rx + 18.0, ry + rh / 2.0, 17.0, t::INK2);
        cx.c.text_fit(&name, rx + 38.0, ry + 8.0, rw - 70.0, Text::new(ty::SECTION, t::INK).semi());
        let mut sub = format!("{branch} · {} agent{}", agents.len(), if agents.len() == 1 { "" } else { "s" });
        if asking > 0 {
            sub.push_str(&format!(" · {asking} need{} you", if asking == 1 { "s" } else { "" }));
        }
        cx.c.text_fit(&sub, rx + 38.0, ry + 31.0, rw - 70.0, Text::new(ty::CAPTION, t::INK3));
        cx.c.icon("chevron-down", rx + rw - 18.0, ry + rh / 2.0, 13.0, t::INK3);
        if cx.tap(rx, ry, rw, rh) {
            app.st.drawer.repos = true;
        }
    }
    // New agent: the drawer's one filled key.
    let ky = ry + rh + 10.0;
    {
        let mut cx = crate::cx!(app);
        if kit::primary(&mut cx, x0 + 16.0, ky, W - 32.0, 46.0, "New agent", Some("plus")) {
            app.st.newagent.open = true;
            app.st.drawer.open = false;
        }
    }

    // Search, by name and intent, live and earlier (D1).
    let sy = ky + 46.0 + 10.0;
    search(app, x0 + 16.0, sy, W - 32.0);

    // The agents, scrolling between the keys and the foot.
    let foot_h = 58.0 + app.canvas.inset_bottom;
    let links_h = 4.0 * 44.0 + 12.0;
    let list_top = sy + SEARCH_H + 8.0;
    let list_bottom = screen - foot_h - links_h;
    let dt = 0.016;
    let viewport = [x0, list_top, W, list_bottom - list_top];
    let moving = app.st.drawer.scroll.update(&app.ui, viewport, dt);
    app.canvas.push_clip(x0, list_top, W, list_bottom - list_top);
    let mut y = list_top - app.st.drawer.scroll.off;
    let mut open: Option<String> = None;
    let window = Some((list_top, list_bottom));
    let query = if app.focus.is(crate::agents::SEARCH) { app.focus.text.clone() } else { app.st.agents.query.clone() };
    if !query.trim().is_empty() {
        let found = crate::agents::found(app, &query);
        open = crate::agents::section(app, "FOUND", &found, x0, &mut y, W, window);
        if found.is_empty() {
            let tt = Text::new(ty::UI, t::INK3).line(1.45);
            app.canvas.para(&format!("No agent here is called or doing \"{}\".", query.trim()), x0 + t::PAD, y + 12.0, W - 2.0 * t::PAD, tt, 3);
            y += 60.0;
        }
    } else {
        // Nothing live: said first, above what stopped earlier.
        if agents.is_empty() {
            let tt = Text::new(ty::UI, t::INK3).line(1.45);
            app.canvas.para("No agents running in this repo. New agent starts one.", x0 + t::PAD, y + 12.0, W - 2.0 * t::PAD, tt, 3);
            y += 56.0;
        }
        let (needs, rest): (Vec<Agent>, Vec<Agent>) = agents.iter().cloned().partition(|a| a.needs_you());
        for (label, list) in [("NEEDS YOU", needs), ("AGENTS", rest)] {
            open = open.or(crate::agents::section(app, label, &list, x0, &mut y, W, window));
        }
        open = open.or(crate::agents::earlier_section(app, x0, &mut y, W, window));
    }
    // More below: the list fades into the panel instead of cutting a row in half (D5).
    let more = y + 12.0 > list_bottom + 1.0;
    if more {
        app.canvas.rect_grad(x0, list_bottom - 32.0, W, 32.0, 0.0, t::PANEL.a(0.0), t::PANEL);
    }
    app.canvas.pop_clip();
    app.st.drawer.scroll.max = y + app.st.drawer.scroll.off - list_bottom + 12.0;

    // The rest of the repo, one tap each (route A).
    let mut ly = list_bottom + 6.0;
    app.canvas.rect(x0 + 16.0, list_bottom, W - 32.0, 1.0, 0.0, t::LINE);
    let inbox_n = app.agents.iter().filter(|a| a.needs_you()).count() + app.holds;
    let links: [(&'static str, &str, Route, String); 4] = [
        ("branch", "Changes", Route::Changes, String::new()),
        ("palette", "Designs", Route::Designs, String::new()),
        ("board", "Board", Route::Board, String::new()),
        ("bell", "Inbox", Route::Inbox, if inbox_n > 0 { inbox_n.to_string() } else { String::new() }),
    ];
    let mut go: Option<Route> = None;
    for (glyph, words, route, count) in links {
        let mut cx = crate::cx!(app);
        if cx.pressed(x0 + 8.0, ly, W - 16.0, 44.0) {
            cx.c.rect(x0 + 8.0, ly, W - 16.0, 44.0, 10.0, t::WASH);
        }
        cx.c.icon(glyph, x0 + t::PAD + 10.0, ly + 22.0, 17.0, t::INK2);
        cx.c.text(words, x0 + t::PAD + 32.0, ly + 12.0, Text::new(ty::BODY, t::INK));
        if !count.is_empty() {
            let ct = Text::new(ty::SMALL, t::ON_ACCENT).semi().center();
            let cw = cx.c.measure(&count, ct).max(10.0) + 12.0;
            cx.c.rect(x0 + W - t::PAD - cw, ly + 12.0, cw, 20.0, 10.0, t::ACCENT);
            cx.c.text(&count, x0 + W - t::PAD - cw / 2.0, ly + 12.0 + (20.0 - ct.lh()) / 2.0, ct);
        }
        if cx.tap(x0 + 8.0, ly, W - 16.0, 44.0) {
            go = Some(route);
        }
        ly += 44.0;
    }
    if let Some(r) = go {
        app.st.drawer.open = false;
        // Each is a page over Home; Back comes to the chat or Home it was opened from.
        if app.route() != &r {
            app.push(r);
        }
    }

    // The PC at the foot: where the link stands, and Settings.
    let fy = screen - foot_h;
    app.canvas.rect(x0, fy, W, 1.0, 0.0, t::LINE);
    let (dot, words) = link_words(&app.link_state);
    app.canvas.circle(x0 + t::PAD + 4.0, fy + 29.0, 4.0, dot);
    app.canvas.text_fit(&words, x0 + t::PAD + 16.0, fy + 20.0, W - 100.0, Text::new(ty::CAPTION, t::INK2).face(Weight::Medium));
    let mut cx = crate::cx!(app);
    if kit::icon_key_ink(&mut cx, x0 + W - 54.0, fy + 7.0, "settings", t::INK2) {
        app.st.drawer.open = false;
        app.push(Route::Settings);
    }
    if let Some(name) = open {
        app.st.drawer.open = false;
        app.open(&name);
    }
    app.canvas.dy = saved;
    moving
}

/// The search field's height.
const SEARCH_H: f32 = 40.0;

/// The drawer's search: a quiet field with a glass, and a key to clear it.
fn search(app: &mut App, x: f32, y: f32, w: f32) {
    let focused = app.focus.is(crate::agents::SEARCH);
    app.canvas.rect(x, y, w, SEARCH_H, 10.0, t::S1);
    if focused {
        app.canvas.outline(x, y, w, SEARCH_H, 10.0, 1.0, t::INK3);
    }
    app.canvas.icon("search", x + 18.0, y + SEARCH_H / 2.0, 15.0, t::INK3);
    let mut q = app.st.agents.query.clone();
    let has = !(if focused { app.focus.text.is_empty() } else { q.is_empty() });
    let fw = w - 36.0 - if has { 40.0 } else { 12.0 };
    let mut cx = crate::cx!(app);
    kit::inline(&mut cx, crate::agents::SEARCH, x + 36.0, y, fw, SEARCH_H, "Search agents", &mut q, Kb::Text, Text::new(ty::UI, t::INK));
    if has {
        let (kx, ky) = (x + w - 44.0, y - 2.0);
        if cx.pressed(kx, ky, 44.0, 44.0) {
            cx.c.circle(kx + 22.0, ky + 22.0, 16.0, t::WASH);
        }
        cx.c.icon("close", kx + 22.0, ky + 22.0, 13.0, t::INK3);
        if cx.tap(kx, ky, 44.0, 44.0) {
            q.clear();
            if focused {
                cx.focus.text.clear();
                cx.focus.sel = None;
                cx.host.keyboard(true, "", Kb::Text);
            }
        }
    }
    app.st.agents.query = q;
}

/// The link in a few words, and its dot.
pub fn link_words(s: &LinkState) -> (crate::draw::Rgba, String) {
    match s {
        LinkState::Online { host, .. } => (t::OK, host.clone()),
        LinkState::Starting { host } => (t::WARN, format!("Starting Relay on {host}")),
        LinkState::Connecting { .. } => (t::WARN, "Connecting…".into()),
        LinkState::Offline { .. } => (t::INK3, "PC away · trying again".into()),
        LinkState::Revoked { .. } => (t::DANGER, "Pair again".into()),
        LinkState::Unpaired => (t::INK3, "No PC paired".into()),
    }
}

/// The repo sheet: every project, the one shown ticked. Picking one keeps you on the same kind of page.
fn repos(app: &mut App) {
    let projects = app.projects.clone();
    let rows = projects.len().max(1) as f32;
    let h = 70.0 + rows * 56.0 + 12.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside {
        app.st.drawer.repos = false;
        return;
    }
    app.canvas.text("Repo", t::PAD + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    let mut y = top + 64.0;
    if projects.is_empty() {
        app.canvas.text("The PC has not said which repos it has yet.", t::PAD + 4.0, y + 16.0, Text::new(ty::UI, t::INK3));
    }
    for p in projects {
        let n = app.agents.iter().filter(|a| a.project_id == p.id && a.alive()).count();
        let ask = app.agents.iter().filter(|a| a.project_id == p.id && a.needs_you()).count();
        let mut cx = crate::cx!(app);
        if cx.pressed(0.0, y, WIDTH, 56.0) {
            cx.c.rect(8.0, y, WIDTH - 16.0, 56.0, 10.0, t::WASH);
        }
        cx.c.icon("folder", t::PAD + 14.0, y + 28.0, 17.0, t::INK2);
        cx.c.text_fit(&p.name, t::PAD + 36.0, y + 9.0, WIDTH - 110.0, Text::new(ty::BODY, t::INK).semi());
        let mut sub = format!("{} · {n} agent{}", p.base_branch, if n == 1 { "" } else { "s" });
        if ask > 0 {
            sub.push_str(&format!(" · {ask} need{} you", if ask == 1 { "s" } else { "" }));
        }
        cx.c.text_fit(&sub, t::PAD + 36.0, y + 31.0, WIDTH - 110.0, Text::new(ty::CAPTION, t::INK3));
        if Some(p.id) == app.repo {
            cx.c.icon("check", WIDTH - t::PAD - 14.0, y + 28.0, 18.0, t::ACCENT);
        }
        if cx.tap(0.0, y, WIDTH, 56.0) {
            app.st.drawer.repos = false;
            app.set_repo(p.id);
        }
        y += 56.0;
    }
}

/// The models New agent offers, as `claude --model` takes them; None is Claude Code's own default.
pub const MODELS: [(Option<&str>, &str); 4] = [(None, "Auto"), (Some("opus"), "Opus"), (Some("sonnet"), "Sonnet"), (Some("haiku"), "Haiku")];
/// The efforts, as `claude --effort` takes them.
pub const EFFORTS: [(Option<&str>, &str); 6] = [(None, "Auto"), (Some("low"), "Low"), (Some("medium"), "Med"), (Some("high"), "High"), (Some("xhigh"), "Extra"), (Some("max"), "Max")];

const FIELD_H: f32 = 96.0;
const ROW_H: f32 = 36.0;

/// The sheet's height: its head, the field, model and effort, the task, Start.
fn new_agent_h() -> f32 {
    82.0 + FIELD_H + 16.0 + 2.0 * (ROW_H + 10.0) + 6.0 + 48.0 + 14.0 + 48.0 + 16.0
}

/// New agent: what it should do, in this repo, on which model and effort (remembered), and the
/// board task it is for. Start makes and starts it; its chat opens as soon as the PC has made it.
pub fn draw_new_agent(app: &mut App) -> bool {
    if !app.st.newagent.open {
        return false;
    }
    if app.st.newagent.picking {
        tasks(app);
        return false;
    }
    let creating = app.calls_for_new();
    let (top, outside) = kit::sheet(&mut crate::cx!(app), new_agent_h());
    if outside && app.focus.id.is_none() && !creating {
        app.st.newagent.open = false;
        return false;
    }
    let repo = app.repo_name();
    app.canvas.text("New agent", t::PAD + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&format!("In {repo}, on Claude Code"), t::PAD + 4.0, top + 52.0, WIDTH - 40.0, Text::new(ty::CAPTION, t::INK3));
    let fy = top + 82.0;
    let fw = WIDTH - 2.0 * t::PAD;
    app.canvas.rect(t::PAD, fy, fw, FIELD_H, 14.0, t::S2);
    let mut task = app.st.newagent.task.clone();
    let focused = app.focus.is("new-agent");
    if focused {
        task = app.focus.text.clone();
        app.focus.at = Some((fy, FIELD_H));
    }
    let style = Text::new(ty::BODY, if task.is_empty() { t::INK3 } else { t::INK }).line(1.4);
    let shown = if task.is_empty() { "What should it do?" } else { task.as_str() };
    app.canvas.para(shown, t::PAD + 14.0, fy + 12.0, fw - 28.0, style, 3);
    if focused && task.is_empty() {
        app.canvas.rect(t::PAD + 14.0, fy + 12.0, 2.0, style.lh(), 1.0, t::ACCENT);
    }
    {
        let mut cx = crate::cx!(app);
        if cx.tap(t::PAD, fy, fw, FIELD_H) && !focused && !creating {
            cx.focus.id = Some("new-agent");
            cx.focus.text = task.clone();
            cx.focus.kind = Kb::Text;
            cx.host.keyboard(true, &task, Kb::Text);
        }
    }
    app.st.newagent.task = task.clone();

    // Model and effort, each a row of choices, remembered for the next agent.
    let (model, effort) = app.last_choice();
    let lw = 64.0;
    let mut y = fy + FIELD_H + 16.0;
    let lt = Text::new(ty::CAPTION, t::INK2).medium();
    app.canvas.text("Model", t::PAD + 2.0, y + (ROW_H - lt.lh()) / 2.0, lt);
    let on = MODELS.iter().position(|(m, _)| *m == model.as_deref()).unwrap_or(0);
    let words: Vec<&str> = MODELS.iter().map(|(_, w)| *w).collect();
    if let Some(i) = kit::segmented_on(&mut crate::cx!(app), t::PAD + lw, y, fw - lw, &words, on, t::S2)
        && !creating
    {
        app.set_choice(MODELS[i].0, effort.as_deref());
    }
    y += ROW_H + 10.0;
    app.canvas.text("Effort", t::PAD + 2.0, y + (ROW_H - lt.lh()) / 2.0, lt);
    let (model, effort) = app.last_choice();
    let on = EFFORTS.iter().position(|(e, _)| *e == effort.as_deref()).unwrap_or(0);
    let words: Vec<&str> = EFFORTS.iter().map(|(_, w)| *w).collect();
    if let Some(i) = kit::segmented_on(&mut crate::cx!(app), t::PAD + lw, y, fw - lw, &words, on, t::S2)
        && !creating
    {
        app.set_choice(model.as_deref(), EFFORTS[i].0);
    }
    y += ROW_H + 10.0 + 6.0;

    // The board task it is for: none, or one picked from the repo's open tasks.
    let linked = app.st.newagent.linked.clone();
    {
        let h = 48.0;
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(t::PAD, y, fw, h);
        cx.c.rect(t::PAD, y, fw, h, t::KEY_R, if pressed { t::S3 } else { t::S2 });
        cx.c.icon("board", t::PAD + 20.0, y + h / 2.0, 16.0, t::INK2);
        let (words, ink) = match &linked {
            Some((id, title)) => (format!("#{id} {title}"), t::INK),
            None => ("Link a board task".to_string(), t::INK2),
        };
        cx.c.text_fit(&words, t::PAD + 42.0, y + (h - 20.0) / 2.0, fw - 42.0 - 48.0, Text::new(ty::UI, ink).medium());
        let clear = linked.is_some();
        cx.c.icon(if clear { "close" } else { "chevron-right" }, t::PAD + fw - 22.0, y + h / 2.0, 13.0, t::INK3);
        if clear && cx.tap(t::PAD + fw - 48.0, y, 48.0, h) {
            app.st.newagent.linked = None;
        } else if cx.tap(t::PAD, y, fw, h) && !creating {
            app.st.newagent.picking = true;
            app.finish_typing();
            let Some(repo) = app.repo else { return false };
            app.ask(Ask::Tasks, "task.list", json!({"project_id": repo, "limit": 200}));
        }
        y += h + 14.0;
    }

    let ready = !task.trim().is_empty() && app.repo.is_some() && !creating;
    let words = if creating { "Starting…" } else { "Start agent" };
    let mut cx = crate::cx!(app);
    let tapped = if ready { kit::primary(&mut cx, t::PAD, y, fw, 48.0, words, None) } else { kit::plain(&mut cx, t::PAD, y, fw, 48.0, words, None) && false };
    if tapped && let Some(repo) = app.repo {
        let prompt = task.trim().to_string();
        app.finish_typing();
        let (model, effort) = app.last_choice();
        let mut p = json!({"project_id": repo, "provider": "claude", "role": "builder", "prompt": prompt});
        if let Some(m) = model {
            p["model"] = json!(m);
        }
        if let Some(e) = effort {
            p["effort"] = json!(e);
        }
        if let Some((id, _)) = linked {
            p["task_id"] = json!(id);
        }
        // The sheet stays, saying Starting…, until the PC has made the agent; then its chat opens.
        app.ask(Ask::Create, "session.create", p);
    }
    creating
}

/// The repo's open tasks to link a new agent to: what is not done yet, Active first.
fn tasks(app: &mut App) {
    let loading = app.asking(&Ask::Tasks);
    let order = |c: &str| match c {
        "active" => 0,
        "ready" => 1,
        "in_review" => 2,
        _ => 3,
    };
    let mut list: Vec<crate::board::Task> = app.st.newagent.tasks.iter().filter(|t| t.column != "done").cloned().collect();
    list.sort_by_key(|t| order(&t.column));
    list.truncate(40);
    let rows = list.len().clamp(1, 8) as f32;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), 70.0 + rows * 56.0 + 12.0);
    if outside {
        app.st.newagent.picking = false;
        return;
    }
    {
        let mut cx = crate::cx!(app);
        if kit::icon_key(&mut cx, 6.0, top + 17.0, "chevron-left") {
            app.st.newagent.picking = false;
            return;
        }
    }
    app.canvas.text("Board task", 56.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    let list_top = top + 64.0;
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    if list.is_empty() {
        let words = if loading { "Reading the board…" } else { "No open tasks in this repo." };
        app.canvas.text(words, t::PAD + 4.0, list_top + 16.0, Text::new(ty::UI, t::INK3));
        return;
    }
    let dt = 0.016;
    let viewport = [0.0, list_top, WIDTH, bottom - list_top];
    app.st.newagent.scroll.update(&app.ui, viewport, dt);
    app.canvas.push_clip(0.0, list_top, WIDTH, bottom - list_top);
    let mut y = list_top - app.st.newagent.scroll.off;
    let mut pick = None;
    for task in &list {
        let visible = y + 56.0 > list_top && y < bottom;
        let mut cx = crate::cx!(app);
        if visible && cx.pressed(0.0, y, WIDTH, 56.0) {
            cx.c.rect(8.0, y, WIDTH - 16.0, 56.0, 10.0, t::WASH);
        }
        cx.c.text_fit(&task.title, t::PAD + 4.0, y + 9.0, WIDTH - 2.0 * t::PAD - 8.0, Text::new(ty::BODY, t::INK).semi());
        let column = crate::board::COLUMNS.iter().find(|(k, _)| *k == task.column).map_or(task.column.as_str(), |(_, w)| *w);
        cx.c.text(&format!("#{} · {column}", task.id), t::PAD + 4.0, y + 31.0, Text::new(ty::CAPTION, t::INK3));
        if cx.tap(0.0, y, WIDTH, 56.0) && visible {
            pick = Some((task.id, task.title.clone()));
        }
        y += 56.0;
    }
    app.canvas.pop_clip();
    app.st.newagent.scroll.max = y + app.st.newagent.scroll.off - bottom + 12.0;
    if let Some(p) = pick {
        app.st.newagent.linked = Some(p);
        app.st.newagent.picking = false;
    }
}

impl App {
    fn calls_for_new(&self) -> bool {
        self.asking(&Ask::Create)
    }
}
