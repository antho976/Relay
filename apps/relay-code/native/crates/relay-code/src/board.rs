//! The board, light (design 44, Q3: kept): the repo's tasks one column at a time under pill tabs,
//! each card saying who is on it. A task opens to its words and its agents, moves to the next
//! column, takes a comment, or gets an agent started on it.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::{App, Ask, Route};
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Kb};
use crate::theme::{self as t, ty};

pub const COLUMNS: [(&str, &str); 5] = [("backlog", "Backlog"), ("ready", "Ready"), ("active", "Active"), ("in_review", "In review"), ("done", "Done")];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub column: String,
    pub sessions: Vec<String>,
    pub labels: Vec<String>,
    pub priority: String,
}

pub fn task(v: &Value) -> Option<Task> {
    let strs = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string).or_else(|| x.get("name").and_then(Value::as_str).map(str::to_string))).collect())
            .unwrap_or_default()
    };
    Some(Task {
        id: v.get("id")?.as_i64()?,
        title: v.get("title")?.as_str()?.to_string(),
        body: v.get("body").and_then(Value::as_str).unwrap_or_default().to_string(),
        column: v.get("column").and_then(Value::as_str).unwrap_or("backlog").to_string(),
        sessions: strs("sessions"),
        labels: strs("labels"),
        priority: v.get("priority").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

pub fn tasks(v: &Value) -> Vec<Task> {
    v.get("tasks").and_then(Value::as_array).map(|a| a.iter().filter_map(task).collect()).unwrap_or_default()
}

/// The column a task moves to next, and the key's words.
pub fn next(column: &str) -> Option<(&'static str, &'static str)> {
    match column {
        "backlog" => Some(("ready", "Move to Ready")),
        "ready" => Some(("active", "Start")),
        "active" => Some(("in_review", "Send to review")),
        "in_review" => Some(("done", "Approve")),
        _ => None,
    }
}

/// One line of a task's activity: a comment, or something done to it.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Comment { author: String, body: String, at: String },
    Did { who: String, what: String, at: String },
}

impl Event {
    fn at(&self) -> &str {
        match self {
            Event::Comment { at, .. } | Event::Did { at, .. } => at,
        }
    }
}

/// Who did it, as a person reads it.
fn who(actor: &str) -> String {
    match actor {
        "user" => "You".into(),
        "system" => "Relay".into(),
        a => a.strip_prefix("agent:").unwrap_or(a).to_string(),
    }
}

fn column_words(key: &str) -> &str {
    COLUMNS.iter().find(|(k, _)| *k == key).map_or(key, |(_, w)| w)
}

/// A task's comments and what was done to it (`task.activity`), oldest first.
pub fn activity(v: &Value) -> Vec<Event> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut out: Vec<Event> = Vec::new();
    for c in v.get("comments").and_then(Value::as_array).into_iter().flatten() {
        out.push(Event::Comment { author: who(&s(c, "author")), body: s(c, "body"), at: s(c, "created_at") });
    }
    for h in v.get("history").and_then(Value::as_array).into_iter().flatten() {
        if h.get("kind").and_then(Value::as_str) != Some("ok") {
            continue;
        }
        let p = h.get("payload").cloned().unwrap_or(Value::Null);
        let what = match s(h, "op").as_str() {
            "task.move" => format!("moved it to {}", column_words(&s(&p, "column"))),
            "task.approve" => "approved it".into(),
            "task.unapprove" => "reopened it".into(),
            "task.dispatch" => "started an agent on it".into(),
            "task.create" => "created it".into(),
            "task.update" => "edited it".into(),
            "task.label.add" => format!("labelled it {}", s(&p, "label")).trim_end().to_string(),
            "task.label.remove" => "took a label off".into(),
            "task.link_commit" => format!("linked {}", s(&p, "sha").chars().take(7).collect::<String>()),
            "task.changelog.write" => "wrote its changelog".into(),
            "session.done" => format!("reported it {}", s(&p, "status")).trim_end().to_string(),
            // Comments come whole from `comments`.
            "task.comment" => continue,
            op => op.to_string(),
        };
        out.push(Event::Did { who: who(&s(h, "actor")), what, at: s(h, "ts") });
    }
    // RFC 3339 times of one clock sort as text.
    out.sort_by(|a, b| a.at().cmp(b.at()));
    out
}

/// The open task: read on its own (`task.get`), not from the board's list, which leaves bodies
/// out, with its activity.
#[derive(Default)]
pub struct Opened {
    pub id: i64,
    pub task: Option<Task>,
    pub error: Option<String>,
    pub events: Vec<Event>,
    get: Option<Call>,
    activity: Option<Call>,
    /// A comment on its way: the box keeps its words until the PC says it has it.
    sending: Option<Call>,
}

#[derive(Default)]
pub struct State {
    repo: Option<i64>,
    pub tasks: Vec<Task>,
    loaded: bool,
    pub column: usize,
    tabs: crate::ui::HScroll,
    list: Option<Call>,
    act: Option<Call>,
    pub comment: String,
    pub open: Opened,
}

fn reload(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if app.online() && app.st.board.list.is_none() {
        // Titles, not essays: the open task reads its own body.
        app.st.board.list = Some(app.link.call("task.list", json!({"project_id": repo, "limit": 500, "summary": true})));
    }
}

/// Read the open task and its activity again.
fn reload_task(app: &mut App, id: i64) {
    if app.st.board.open.id != id {
        app.st.board.open = Opened { id, ..Opened::default() };
        app.st.board.comment.clear();
    }
    if !app.online() {
        return;
    }
    let o = &mut app.st.board.open;
    if o.get.is_none() {
        o.get = Some(app.link.call("task.get", json!({"task_id": id})));
    }
    if o.activity.is_none() {
        o.activity = Some(app.link.call("task.activity", json!({"task_id": id, "limit": 40})));
    }
}

fn take(app: &mut App) {
    if let Some(r) = app.st.board.list.as_ref().and_then(Call::poll) {
        app.st.board.list = None;
        match r {
            Ok(v) => {
                app.st.board.tasks = tasks(&v);
                app.st.board.loaded = true;
            }
            Err(e) => app.toast(&e.message),
        }
    }
    if let Some(r) = app.st.board.act.as_ref().and_then(Call::poll) {
        app.st.board.act = None;
        match r {
            Ok(v) => {
                if let Some(tk) = task(&v) {
                    if let Some(old) = app.st.board.tasks.iter_mut().find(|x| x.id == tk.id) {
                        // The list's copy has no body: keep it so.
                        *old = Task { body: String::new(), ..tk.clone() };
                    }
                    if app.st.board.open.id == tk.id {
                        app.st.board.open.task = Some(tk);
                    }
                }
                reload(app);
            }
            Err(e) => app.toast(&e.message),
        }
    }
    let o = &mut app.st.board.open;
    if let Some(r) = o.get.as_ref().and_then(Call::poll) {
        o.get = None;
        match r {
            Ok(v) => {
                o.task = task(&v);
                o.error = None;
            }
            // Said where the task would be: never "Reading the task…" for ever.
            Err(e) => o.error = Some(e.message),
        }
    }
    if let Some(r) = o.activity.as_ref().and_then(Call::poll) {
        o.activity = None;
        if let Ok(v) = r {
            o.events = activity(&v);
        }
    }
    if let Some(r) = app.st.board.open.sending.as_ref().and_then(Call::poll) {
        app.st.board.open.sending = None;
        match r {
            Ok(v) => {
                app.st.board.comment.clear();
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                app.st.board.open.events.push(Event::Comment { author: who(&s("author")), body: s("body"), at: s("created_at") });
                app.toast("Commented");
            }
            // The words stay in the box to send again.
            Err(e) => app.toast(&e.message),
        }
    }
}

fn prepare(app: &mut App) {
    if app.st.board.repo != app.repo {
        app.st.board = State { repo: app.repo, column: 2, ..State::default() };
    }
    if app.fresh || app.take_changed("task") {
        reload(app);
        if let Route::Task(id) = *app.route() {
            reload_task(app, id);
        }
    }
    take(app);
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    prepare(app);
    let tabs_h = 48.0;
    let page = app.page_begin(dt, crate::home::TOP_H + tabs_h, 0.0);
    let col = COLUMNS[app.st.board.column.min(4)].0;
    let list: Vec<Task> = app.st.board.tasks.iter().filter(|x| x.column == col).cloned().collect();
    let mut y = page.top + 6.0;
    let mut open = None;
    if !app.st.board.loaded {
        let words = if app.online() { "Reading the board…" } else { "The board shows here once the PC answers." };
        app.canvas.text(words, t::PAD, y + 10.0, Text::new(ty::UI, t::INK3));
        y += 40.0;
    } else if list.is_empty() {
        app.canvas.text(&format!("Nothing in {}.", COLUMNS[app.st.board.column].1), t::PAD, y + 10.0, Text::new(ty::UI, t::INK3));
        y += 40.0;
    }
    for tk in &list {
        let tt = Text::new(ty::BODY, t::INK).semi().line(1.35);
        let th = app.canvas.para_h(&tk.title, WIDTH - 2.0 * t::PAD - 28.0, tt, 3);
        let h = 14.0 + th + 6.0 + 18.0 + 14.0;
        let mut cx = crate::cx!(app);
        let x = t::PAD;
        let w = WIDTH - 2.0 * t::PAD;
        cx.c.rect(x, y, w, h, 14.0, if cx.pressed(x, y, w, h) { t::S2 } else { t::S1 });
        cx.c.para(&tk.title, x + 14.0, y + 14.0, w - 28.0, tt, 3);
        let who = if tk.sessions.is_empty() { "Nobody on it".to_string() } else { format!("{} on it", tk.sessions.join(", ")) };
        let meta = if tk.labels.is_empty() { format!("#{} · {who}", tk.id) } else { format!("#{} · {} · {who}", tk.id, tk.labels.join(", ")) };
        cx.c.text_fit(&meta, x + 14.0, y + 14.0 + th + 6.0, w - 28.0, Text::new(ty::CAPTION, t::INK3));
        if cx.tap(x, y, w, h) {
            open = Some(tk.id);
        }
        y += h + 10.0;
    }
    app.page_end(&page, y);
    crate::home::top(app, "Board");
    // The column tabs, under the top bar, on its layer.
    let ty_ = app.canvas.inset_top + crate::home::TOP_H;
    app.canvas.rect(-1000.0, ty_, WIDTH + 2000.0, tabs_h, 0.0, t::BG);
    let band = [-1000.0, ty_, WIDTH + 2000.0, tabs_h];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    let items: Vec<(String, bool, Option<crate::draw::Rgba>)> = COLUMNS
        .iter()
        .enumerate()
        .map(|(i, (key, words))| {
            let n = app.st.board.tasks.iter().filter(|x| x.column == *key).count();
            (format!("{words} {n}"), i == app.st.board.column, None)
        })
        .collect();
    let mut hs = app.st.board.tabs;
    let hit = kit::chip_row(&mut crate::cx!(app), &mut hs, ty_ + 6.0, &items, dt);
    app.st.board.tabs = hs;
    if let Some(i) = hit {
        app.st.board.column = i;
        app.scroll().off = 0.0;
    }
    if let Some(id) = open {
        app.push(Route::Task(id));
    }
    page.moving
}

/// One task: its words, who is on it, the next step, a comment.
pub fn draw_task(app: &mut App, id: i64, dt: f32) -> bool {
    if app.st.board.open.id != id {
        reload_task(app, id);
    }
    prepare(app);
    // Its own read when it has come, else the board's line for it (a title, no body) meanwhile.
    let read = app.st.board.open.task.clone().filter(|x| x.id == id);
    let Some(tk) = read.clone().or_else(|| app.st.board.tasks.iter().find(|x| x.id == id).cloned()) else {
        let page = app.page_begin(dt, 56.0, 0.0);
        let words = match &app.st.board.open.error {
            Some(e) => e.clone(),
            None if app.online() => "Reading the task…".to_string(),
            None => "The task shows here once the PC answers.".to_string(),
        };
        app.canvas.para(&words, t::PAD, page.top + 16.0, WIDTH - 2.0 * t::PAD, Text::new(ty::UI, t::INK3).line(1.45), 4);
        app.page_end(&page, page.top + 40.0);
        back_bar(app, "");
        return page.moving;
    };
    let foot_h = 124.0;
    let page = app.page_begin(dt, 56.0, foot_h);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let mut y = page.top + 8.0;
    let col_words = COLUMNS.iter().find(|(k, _)| *k == tk.column).map_or(tk.column.as_str(), |(_, w)| w);
    app.canvas.text(&format!("#{} · {col_words}", tk.id), x, y, Text::new(ty::CAPTION, t::INK3).medium());
    y += 24.0;
    y += app.canvas.para(&tk.title, x, y, w, Text::new(ty::TITLE - 2.0, t::INK).semi().line(1.25), 6) + 14.0;
    if !tk.sessions.is_empty() {
        app.canvas.text("ON IT", x, y, Text::new(ty::SMALL, t::INK3).semi());
        y += 22.0;
        let now_ms = rc_link::link::now_ms();
        let names = tk.sessions.clone();
        let mut open = None;
        for n in &names {
            if let Some(a) = app.agents.iter().find(|a| &a.name == n).cloned() {
                if crate::drawer::agent_row(app, &a, 0.0, y, WIDTH, now_ms) {
                    open = Some(a.name.clone());
                }
                y += 60.0;
            } else {
                app.canvas.text(n, x, y + 8.0, Text::new(ty::UI, t::INK2));
                y += 32.0;
            }
        }
        if let Some(n) = open {
            app.open(&n);
            return false;
        }
        y += 8.0;
    }
    if !tk.body.trim().is_empty() {
        y += crate::chat::markdown(app, &tk.body, x, y, w, Text::new(ty::BODY, t::INK2).line(1.5), true);
    } else if read.is_none() && app.st.board.open.error.is_none() && app.online() {
        app.canvas.text("Reading the task…", x, y, Text::new(ty::SMALL, t::INK3));
        y += 24.0;
    }
    y = events(app, x, y + 16.0, w);
    app.page_end(&page, y + 12.0);
    back_bar(app, "");

    // The foot: a comment, then the next step (the one filled key) and Start an agent.
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - foot_h;
    let band = [-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0, 0.0, t::BG);
    app.canvas.rect(-1000.0, top, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
    let my = top + 12.0;
    app.canvas.rect(x, my, w, 44.0, 12.0, t::S1);
    let mut comment = app.st.board.comment.clone();
    kit::inline(&mut crate::cx!(app), "task-comment", x + 14.0, my, w - 60.0, 44.0, "Comment", &mut comment, Kb::Text, Text::new(ty::UI, t::INK));
    app.st.board.comment = comment.clone();
    let sending = app.st.board.open.sending.is_some();
    let send = {
        let mut cx = crate::cx!(app);
        !comment.trim().is_empty() && kit::icon_key_ink(&mut cx, x + w - 44.0, my, "send", if sending { t::INK3 } else { t::ACCENT })
    };
    if send && !sending && app.online() {
        app.finish_typing();
        app.st.board.open.sending = Some(app.link.call("task.comment", json!({"task_id": id, "body": comment.trim()})));
    }
    let ky = my + 44.0 + 12.0;
    let busy = app.st.board.act.is_some();
    let next = next(&tk.column);
    let agent_w = if tk.sessions.is_empty() && tk.column != "done" { 150.0 } else { 0.0 };
    let mut cx = crate::cx!(app);
    let step = match next {
        Some((_, words)) if !busy => kit::primary(&mut cx, x, ky, w - agent_w - if agent_w > 0.0 { 10.0 } else { 0.0 }, 44.0, words, None),
        Some((_, words)) => kit::plain(&mut cx, x, ky, w - agent_w - 10.0, 44.0, words, None) && false,
        None => false,
    };
    let start = agent_w > 0.0 && kit::plain(&mut cx, x + w - agent_w, ky, agent_w, 44.0, "Start an agent", None);
    if step && let Some((col, _)) = next {
        // Approving is its own op: it closes the task and links its commit.
        let call = if tk.column == "in_review" { app.link.call("task.approve", json!({"task_id": id})) } else { app.link.call("task.move", json!({"task_id": id, "column": col})) };
        app.st.board.act = Some(call);
    }
    if start && let Some(repo) = app.repo {
        let prompt = format!("Task #{}: {}\n\n{}", tk.id, tk.title, tk.body);
        app.ask(Ask::Create, "session.create", json!({"project_id": repo, "provider": "claude", "role": "builder", "task_id": id, "prompt": prompt.trim()}));
        app.toast("Starting an agent on it…");
    }
    page.moving
}

/// The task's comments and what was done to it, oldest first. Returns where it ends.
fn events(app: &mut App, x: f32, mut y: f32, w: f32) -> f32 {
    let list = app.st.board.open.events.clone();
    if list.is_empty() {
        return y;
    }
    app.canvas.text("ACTIVITY", x, y, Text::new(ty::SMALL, t::INK3).semi());
    y += 26.0;
    let now = rc_link::link::now_ms();
    let small = Text::new(ty::SMALL, t::INK3);
    for e in &list {
        let when = crate::data::ago(e.at(), now);
        match e {
            Event::Comment { author, body, .. } => {
                let aw = app.canvas.text(author, x, y, Text::new(ty::CAPTION, t::INK).semi());
                if !when.is_empty() {
                    app.canvas.text(&format!(" · {when}"), x + aw, y, Text::new(ty::CAPTION, t::INK3));
                }
                y += 22.0;
                y += app.canvas.para(body, x, y, w, Text::new(ty::UI, t::INK2).line(1.45), 12) + 14.0;
            }
            Event::Did { who, what, .. } => {
                let line = if when.is_empty() { format!("{who} {what}") } else { format!("{who} {what} · {when}") };
                app.canvas.text_fit(&line, x, y, w, small);
                y += 24.0;
            }
        }
    }
    y
}

/// A pushed page's top: Back and a title.
pub fn back_bar(app: &mut App, title: &str) {
    app.top_band(56.0);
    let y = app.canvas.inset_top + 6.0;
    let mut cx = crate::cx!(app);
    let (back, _, _) = kit::top_bar(&mut cx, y, title, &[]);
    if back {
        app.pop();
    }
}

/// Sample tasks for screenshots.
pub fn sample(app: &mut App) {
    let v = json!({"tasks": [
        {"id": 41, "title": "Relay Code: Designs open on artboards", "column": "active", "sessions": ["bold-puffin"], "labels": ["phone"]},
        {"id": 42, "title": "PC-side chat.send with ids in the transcript (D6)", "column": "active", "sessions": [], "labels": ["engine"]},
        {"id": 38, "title": "Leaf comics autopilot: source switching", "column": "in_review", "sessions": ["swift-ocelot"]},
        {"id": 35, "title": "Board drag drops on Done", "column": "backlog"},
    ]});
    let list = tasks(&v);
    // Task 41 open, as task.get and task.activity read it.
    let mut t41 = list[0].clone();
    t41.body = "Designs open on their artboards, pinch to zoom, and a tap on a layer names it.\n\n- Pages as tabs\n- The canvas fits the phone's width first".into();
    let now = rc_link::link::now_ms();
    let at = |mins: u64| iso(now.saturating_sub(mins * 60_000));
    let events = vec![
        Event::Did { who: "You".into(), what: "created it".into(), at: at(600) },
        Event::Did { who: "You".into(), what: "started an agent on it".into(), at: at(300) },
        Event::Comment { author: "bold-puffin".into(), body: "Pinch zoom works; the pages bar still overlaps the notch on the Pixel.".into(), at: at(40) },
        Event::Comment { author: "You".into(), body: "Leave the notch for now, ship the tabs.".into(), at: at(12) },
    ];
    let open = Opened { id: 41, task: Some(t41), events, ..Opened::default() };
    app.st.board = State { repo: app.repo, tasks: list, loaded: true, column: 2, open, ..State::default() };
}

/// An RFC 3339 time from milliseconds since 1970 (samples only).
fn iso(ms: u64) -> String {
    let s = ms / 1000;
    let (d, r) = ((s / 86_400) as i64 + 719_468, s % 86_400);
    let era = d.div_euclid(146_097);
    let doe = d - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", r / 3600, r % 3600 / 60, r % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_read_and_move_along() {
        let v = json!({"tasks": [{"id": 7, "title": "Fix the drawer", "column": "active", "sessions": ["bold-puffin"], "labels": [{"name": "ui"}]}]});
        let t = tasks(&v);
        assert_eq!(t[0].sessions, ["bold-puffin"]);
        assert_eq!(t[0].labels, ["ui"]);
        assert_eq!(next("active"), Some(("in_review", "Send to review")));
        assert_eq!(next("done"), None);
    }

    #[test]
    fn activity_reads_comments_and_what_was_done_oldest_first() {
        let v = json!({
            "comments": [{"id": 1, "task_id": 7, "author": "user", "body": "Ship it.", "created_at": "2026-11-26T10:05:00Z"}],
            "history": [
                {"id": 9, "ts": "2026-11-26T10:09:00Z", "actor": "user", "op": "task.approve", "kind": "ok", "payload": {"task_id": 7}},
                {"id": 8, "ts": "2026-11-26T10:01:00Z", "actor": "agent:bold-puffin", "op": "task.move", "kind": "ok", "payload": {"column": "in_review"}},
                {"id": 7, "ts": "2026-11-26T10:00:30Z", "actor": "agent:bold-puffin", "op": "task.move", "kind": "refused", "payload": {}},
                {"id": 6, "ts": "2026-11-26T10:05:00Z", "actor": "user", "op": "task.comment", "kind": "ok", "payload": {}},
            ],
            "messages": []});
        let e = activity(&v);
        assert_eq!(e, [
            Event::Did { who: "bold-puffin".into(), what: "moved it to In review".into(), at: "2026-11-26T10:01:00Z".into() },
            Event::Comment { author: "You".into(), body: "Ship it.".into(), at: "2026-11-26T10:05:00Z".into() },
            Event::Did { who: "You".into(), what: "approved it".into(), at: "2026-11-26T10:09:00Z".into() },
        ]);
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_764_151_500_000), "2025-11-26T10:05:00Z");
    }

    struct NoHost(std::path::PathBuf);

    impl crate::app::Host for NoHost {
        fn data_dir(&self) -> std::path::PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    #[test]
    fn a_column_chip_takes_a_tap_44_tall() {
        use crate::ui::{Phase, Touch};
        let dir = std::env::temp_dir().join(format!("relay-code-board-chips-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Board);
        let mut clock = 2_000_000_000u64;
        let mut tap = |a: &mut App, x: f32, y: f32| {
            for phase in [Phase::Down, Phase::Up] {
                clock += 30_000_000;
                a.touch(Touch { id: 1, phase, x, y, t_ns: clock });
                a.frame(clock);
            }
        };
        tap(&mut a, 1.0, 1.0);
        // The chips' row: 32 tall, drawn 6 under the top bar.
        let chip_top = a.canvas.inset_top + crate::home::TOP_H + 6.0;
        // 5 above the Backlog chip's top, then 5 under its foot: both are the chip's.
        a.st.board.column = 2;
        tap(&mut a, t::PAD + 12.0, chip_top - 5.0);
        assert_eq!(a.st.board.column, 0, "a tap just above the chip");
        a.st.board.column = 2;
        tap(&mut a, t::PAD + 12.0, chip_top + 32.0 + 5.0);
        assert_eq!(a.st.board.column, 0, "a tap just under the chip");
    }

    #[test]
    fn a_task_that_cannot_be_read_says_why_and_a_comment_stays_until_ok() {
        let dir = std::env::temp_dir().join(format!("relay-code-board-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.repo = Some(1);
        a.st.board.repo = Some(1);
        // Not connected: task.get fails at once, as a refusal would.
        a.st.board.open = Opened { id: 5, get: Some(a.link.call("task.get", json!({"task_id": 5}))), ..Opened::default() };
        take(&mut a);
        assert!(a.st.board.open.error.is_some(), "an answer, not \"Reading the task…\" for ever");
        a.st.board.comment = "Looks good".into();
        a.st.board.open.sending = Some(a.link.call("task.comment", json!({"task_id": 5, "body": "Looks good"})));
        take(&mut a);
        assert_eq!(a.st.board.comment, "Looks good", "a failed comment keeps its words");
        assert!(a.st.board.open.sending.is_none());
    }
}
