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
}

/// An agent's badge: two letters on its own colour, and its lamp.
pub fn badge(c: &mut Canvas, a: &Agent, x: f32, y: f32, size: f32) {
    let col = t::BADGES[a.colour()];
    c.rect(x, y, size, size, size * 0.3, col.a(0.2));
    let tt = Text::new(size * 0.38, col).semi().center();
    c.text(&a.initials(), x + size / 2.0, y + (size - tt.lh()) / 2.0, tt);
    let lamp = if a.needs_you() {
        Some(t::WARN)
    } else if a.working() {
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
    let mut cx = crate::cx!(app);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x + 6.0, y + 2.0, w - 12.0, h - 4.0, 10.0, t::WASH);
    }
    badge(cx.c, a, x + t::PAD, y + 14.0, 32.0);
    let tx = x + t::PAD + 32.0 + 12.0;
    let when = a.last_output_at.as_deref().map(|s| crate::data::ago(s, now_ms)).unwrap_or_default();
    let wt = Text::new(ty::SMALL, t::INK3);
    let ww = if when.is_empty() { 0.0 } else { cx.c.measure(&when, wt) + 8.0 };
    cx.c.text_fit(a.title(), tx, y + 11.0, x + w - tx - t::PAD - ww, Text::new(ty::BODY, t::INK).semi());
    let sub = format!("{} · {}", a.name, a.state_words());
    cx.c.text_fit(&sub, tx, y + 32.0, x + w - tx - t::PAD, Text::new(ty::CAPTION, if a.needs_you() { t::WARN } else { t::INK3 }));
    if !when.is_empty() {
        cx.c.text(&when, x + w - t::PAD, y + 13.0, wt.right());
    }
    cx.tap(x, y, w, h)
}

/// The repo, as the top bar shows it: a chip that opens the repo sheet. Returns its width.
pub fn repo_chip(app: &mut App, right: f32, y: f32) -> f32 {
    let name = app.repo_name();
    let mut cx = crate::cx!(app);
    let tt = Text::new(ty::UI, t::INK).semi();
    let w = cx.c.measure(&name, tt).min(170.0) + 58.0;
    let x = right - w;
    let h = 34.0;
    cx.c.rect(x, y, w, h, 10.0, if cx.pressed(x, y, w, h) { t::S2 } else { t::S1 });
    cx.c.icon("folder", x + 18.0, y + h / 2.0, 15.0, t::INK2);
    cx.c.text_fit(&name, x + 32.0, y + (h - tt.lh()) / 2.0, w - 58.0, tt);
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
    app.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, screen, 0.0, t::SCRIM.a(pos));
    // A tap beside the drawer closes it.
    if app.ui.tap_here().is_some_and(|(x, _)| x > x0 + W) {
        app.ui.tap = None;
        app.st.drawer.open = false;
    }
    app.canvas.rect(x0 - 40.0, 0.0, W + 40.0, screen, 24.0, t::PANEL);
    let top = app.canvas.inset_top + 8.0;
    let now_ms = rc_link::link::now_ms();

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

    // The agents, scrolling between the keys and the foot.
    let foot_h = 58.0 + app.canvas.inset_bottom;
    let links_h = 4.0 * 44.0 + 12.0;
    let list_top = ky + 46.0 + 12.0;
    let list_bottom = screen - foot_h - links_h;
    let dt = 0.016;
    let viewport = [x0, list_top, W, list_bottom - list_top];
    let moving = app.st.drawer.scroll.update(&app.ui, viewport, dt);
    app.canvas.push_clip(x0, list_top, W, list_bottom - list_top);
    let mut y = list_top - app.st.drawer.scroll.off;
    let mut open: Option<String> = None;
    let (needs, rest): (Vec<&Agent>, Vec<&Agent>) = agents.iter().partition(|a| a.needs_you());
    for (label, list) in [("NEEDS YOU", needs), ("AGENTS", rest)] {
        if list.is_empty() {
            continue;
        }
        app.canvas.text(label, x0 + t::PAD, y + 10.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 32.0;
        for a in list {
            // Rows outside the list's window take no tap.
            let visible = y + 60.0 > list_top && y < list_bottom;
            if agent_row(app, a, x0, y, W, now_ms) && visible {
                open = Some(a.name.clone());
            }
            y += 60.0;
        }
        y += 4.0;
    }
    if agents.is_empty() {
        let tt = Text::new(ty::UI, t::INK3).line(1.45);
        app.canvas.para("No agents in this repo yet. New agent starts one.", x0 + t::PAD, y + 12.0, W - 2.0 * t::PAD, tt, 3);
        y += 60.0;
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

/// New agent: what it should do, in this repo, on Claude. Start makes and starts it, then opens
/// its conversation. True while something moves.
pub fn draw_new_agent(app: &mut App) -> bool {
    if !app.st.newagent.open {
        return false;
    }
    let creating = app.calls_for_new();
    let h = 252.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside && app.focus.id.is_none() {
        app.st.newagent.open = false;
        return false;
    }
    let repo = app.repo_name();
    app.canvas.text("New agent", t::PAD + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&format!("In {repo}, on Claude Code"), t::PAD + 4.0, top + 52.0, WIDTH - 40.0, Text::new(ty::CAPTION, t::INK3));
    let fy = top + 82.0;
    let fw = WIDTH - 2.0 * t::PAD;
    app.canvas.rect(t::PAD, fy, fw, 96.0, 14.0, t::S1);
    let mut task = app.st.newagent.task.clone();
    let focused = app.focus.is("new-agent");
    if focused {
        task = app.focus.text.clone();
        app.focus.at = Some((fy, 96.0));
    }
    let style = Text::new(ty::BODY, if task.is_empty() { t::INK3 } else { t::INK }).line(1.4);
    let shown = if task.is_empty() { "What should it do?" } else { task.as_str() };
    app.canvas.para(shown, t::PAD + 14.0, fy + 12.0, fw - 28.0, style, 3);
    if focused && task.is_empty() {
        app.canvas.rect(t::PAD + 14.0, fy + 12.0, 2.0, style.lh(), 1.0, t::ACCENT);
    }
    {
        let mut cx = crate::cx!(app);
        if cx.tap(t::PAD, fy, fw, 96.0) && !focused {
            cx.focus.id = Some("new-agent");
            cx.focus.text = task.clone();
            cx.focus.kind = Kb::Text;
            cx.host.keyboard(true, &task, Kb::Text);
        }
    }
    app.st.newagent.task = task.clone();
    let by = fy + 96.0 + 16.0;
    let ready = !task.trim().is_empty() && app.repo.is_some() && !creating;
    let words = if creating { "Starting…" } else { "Start agent" };
    let mut cx = crate::cx!(app);
    let tapped = if ready { kit::primary(&mut cx, t::PAD, by, fw, 48.0, words, None) } else { kit::plain(&mut cx, t::PAD, by, fw, 48.0, words, None) && false };
    if tapped && let Some(repo) = app.repo {
        let prompt = task.trim().to_string();
        app.finish_typing();
        app.ask(Ask::Create, "session.create", json!({"project_id": repo, "provider": "claude", "role": "builder", "prompt": prompt}));
        app.st.newagent.task.clear();
        app.st.newagent.open = false;
        app.toast("Starting the agent…");
    }
    false
}

impl App {
    fn calls_for_new(&self) -> bool {
        self.asking(&Ask::Create)
    }
}
