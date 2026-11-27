//! The board, light (design 44, Q3: kept): the repo's tasks one column at a time under pill tabs,
//! each card saying who is on it. A task opens to its words and its agents, moves to the next
//! column, takes a comment, or gets an agent started on it.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::{App, Route};
use crate::draw::{Text, WIDTH};
use crate::kit;
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
    /// Why the board could not be read, before it was.
    pub error: Option<String>,
    list: Option<Call>,
    /// The list was asked for again while one was out: its answer predates a move made since.
    stale: bool,
    act: Option<Call>,
    /// What to say once `act` answers well ("Approved").
    act_done: Option<&'static str>,
    pub comment: String,
    pub open: Opened,
    /// Sending a task in review back to Active: the task (id, title), what to change, and the
    /// comment then the move on their way (`moved` once the comment is in).
    pub back: Option<(i64, String)>,
    /// A task being written on the phone, and its create on its way.
    pub new_task: Option<NewTask>,
    creating: Option<Call>,
    pub back_words: String,
    back_call: Option<(i64, Call, bool)>,
    /// Each card's title height, measured once per title: wrapping 80 titles every frame was
    /// most of the Board's frame.
    heights: std::collections::HashMap<i64, (String, f32)>,
}

fn reload(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if !app.online() {
        return;
    }
    if app.st.board.list.is_some() {
        app.st.board.stale = true;
        return;
    }
    // Titles, not essays: the open task reads its own body.
    app.st.board.list = Some(app.link.call("task.list", json!({"project_id": repo, "limit": 500, "summary": true})));
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
        // Asked again since (a move, an approval): this older list could move a task back.
        let stale = std::mem::take(&mut app.st.board.stale);
        if stale {
            reload(app);
        }
        match r {
            _ if stale => {}
            Ok(v) => {
                app.st.board.tasks = tasks(&v);
                app.st.board.loaded = true;
                app.st.board.error = None;
            }
            Err(e) if !app.st.board.loaded => app.st.board.error = Some(crate::errors::words(&e)),
            Err(e) => app.toast(&crate::errors::words(&e)),
        }
    }
    take_answers(app);
    let o = &mut app.st.board.open;
    if let Some(r) = o.get.as_ref().and_then(Call::poll) {
        o.get = None;
        match r {
            Ok(v) => {
                o.task = task(&v);
                o.error = None;
            }
            // Said where the task would be: never "Reading the task…" for ever.
            Err(e) => o.error = Some(crate::errors::words(&e)),
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
            Err(e) => app.toast(&crate::errors::words(&e)),
        }
    }
}

/// A task written on the phone: its title, what it is, and the column it starts in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewTask {
    pub title: String,
    pub body: String,
    /// Into [`NEW_COLUMNS`].
    pub column: usize,
}

/// The columns a task may start in from the phone.
pub const NEW_COLUMNS: [(&str, &str); 3] = [("backlog", "Backlog"), ("ready", "Ready"), ("active", "Active")];

/// What `task.create` is sent for a task written on the phone.
pub fn create_payload(repo: i64, t: &NewTask) -> Value {
    let mut p = json!({"project_id": repo, "title": t.title.trim(), "column": NEW_COLUMNS[t.column.min(2)].0});
    if !t.body.trim().is_empty() {
        p["body"] = json!(t.body.trim());
    }
    p
}

/// Open the New task sheet, starting in the column the Board shows (Backlog past Active).
pub fn new_task(app: &mut App) {
    let shown = COLUMNS[app.st.board.column.min(4)].0;
    let column = NEW_COLUMNS.iter().position(|(k, _)| *k == shown).unwrap_or(0);
    app.st.board.new_task = Some(NewTask { column, ..NewTask::default() });
}

/// The New task sheet: a title, what it is (optional), the column; Create makes it and opens it.
fn draw_new_task(app: &mut App) {
    let Some(mut nt) = app.st.board.new_task.clone() else { return };
    let creating = app.st.board.creating.is_some();
    let body_w = WIDTH - 2.0 * t::PAD - 28.0;
    let extra = crate::changes::prose_extra(app, "new-task-body", &nt.body, body_w);
    let h = 380.0 + extra;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside && !creating {
        app.finish_typing();
        app.st.board.new_task = None;
        return;
    }
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    app.canvas.text("New task", x + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&format!("On the board of {}", app.repo_name()), x + 4.0, top + 52.0, w - 8.0, Text::new(ty::CAPTION, t::INK3));
    let mut y = top + 84.0;
    y += kit::field(&mut crate::cx!(app), "new-task-title", x, y, w, "Title", "What needs doing", &mut nt.title, kit::Kb::Text) + 14.0;
    let lt = Text::new(ty::CAPTION, t::INK2).medium();
    app.canvas.text("What it is (optional)", x + 2.0, y, lt);
    let by = y + lt.lh() + 6.0;
    let bh = 48.0 + extra;
    let focused = app.focus.is("new-task-body");
    app.canvas.rect(x, by, w, bh, t::KEY_R, t::S1);
    app.canvas.outline(x, by, w, bh, t::KEY_R, 1.0, if focused { t::INK3 } else { t::LINE });
    crate::changes::prose_box(app, "new-task-body", "The details an agent needs", x + 14.0, by + 1.0, body_w, bh, &mut nt.body);
    y = by + bh + 16.0;
    let names: Vec<&str> = NEW_COLUMNS.iter().map(|(_, n)| *n).collect();
    if let Some(i) = kit::segmented_on(&mut crate::cx!(app), x, y, w, &names, nt.column, t::S2) {
        nt.column = i;
    }
    let ready = !nt.title.trim().is_empty() && !creating && app.online();
    let label = if creating { "Creating…" } else { "Create task" };
    let ky = top + h - 62.0;
    let mut cx = crate::cx!(app);
    let go = if ready { kit::primary(&mut cx, x, ky, w, 46.0, label, None) } else { kit::plain(&mut cx, x, ky, w, 46.0, label, None) && false };
    if go && let Some(repo) = app.repo {
        app.finish_typing();
        app.st.board.creating = Some(app.link.call("task.create", create_payload(repo, &nt)));
    }
    // Only while the sheet is still the one being written (a create's answer may have closed it).
    if let Some(cur) = app.st.board.new_task.as_mut() {
        *cur = nt;
    }
}

/// A create's answer: the task joins the board and opens; a refusal keeps the sheet's words.
fn take_created(app: &mut App) {
    let Some(r) = app.st.board.creating.as_ref().and_then(Call::poll) else { return };
    app.st.board.creating = None;
    match r {
        Ok(v) => {
            app.st.board.new_task = None;
            if let Some(tk) = task(&v) {
                let id = tk.id;
                app.st.board.tasks.push(Task { body: String::new(), ..tk.clone() });
                app.st.board.open = Opened { id, task: Some(tk), ..Opened::default() };
                app.toast("Task created");
                app.push(Route::Task(id));
            }
            reload_shown(app);
        }
        Err(e) => app.toast(&crate::errors::words(&e)),
    }
}

/// Approve a task in review: done, its commit linked (`task.approve`).
pub fn approve(app: &mut App, id: i64) {
    if app.online() && app.st.board.act.is_none() {
        app.st.board.act = Some(app.link.call("task.approve", json!({"task_id": id})));
        app.st.board.act_done = Some("Approved");
    }
}

/// Open the sheet that sends a task in review back to Active with a comment.
pub fn send_back(app: &mut App, id: i64, title: &str) {
    app.st.board.back = Some((id, title.to_string()));
}

/// Take what the Board's actions answered (an approval, a move, a send-back), on any page: it only
/// polls what is already on its way. The one request it may start is a send-back's second step,
/// the move to Active once its comment is in, which finishes that same action. The board is read
/// again only while the Board or a task shows; elsewhere the PC's task event refreshes the lists
/// when they next show.
pub fn take_answers(app: &mut App) {
    if let Some(r) = app.st.board.act.as_ref().and_then(Call::poll) {
        app.st.board.act = None;
        let done = app.st.board.act_done.take();
        match r {
            Ok(v) => {
                if let Some(words) = done {
                    app.toast(words);
                }
                if let Some(tk) = task(&v) {
                    if let Some(old) = app.st.board.tasks.iter_mut().find(|x| x.id == tk.id) {
                        *old = Task { body: String::new(), ..tk.clone() };
                    }
                    if app.st.board.open.id == tk.id {
                        app.st.board.open.task = Some(tk);
                    }
                }
                reload_shown(app);
            }
            Err(e) => app.toast(&crate::errors::words(&e)),
        }
    }
    let Some((id, call, moved)) = app.st.board.back_call.take() else { return };
    let Some(r) = call.poll() else {
        app.st.board.back_call = Some((id, call, moved));
        return;
    };
    match (moved, r) {
        // The comment is in: now the move, so the agent finds the task Active with its reason.
        (false, Ok(_)) => {
            let call = app.link.call("task.move", json!({"task_id": id, "column": "active"}));
            app.st.board.back_call = Some((id, call, true));
        }
        (true, Ok(v)) => {
            app.toast("Sent back to Active");
            app.st.board.back = None;
            app.st.board.back_words.clear();
            if let Some(tk) = task(&v) {
                if let Some(old) = app.st.board.tasks.iter_mut().find(|x| x.id == tk.id) {
                    *old = Task { body: String::new(), ..tk.clone() };
                }
                if app.st.board.open.id == tk.id {
                    app.st.board.open.task = Some(tk);
                }
            }
            reload_shown(app);
        }
        // The words stay in the sheet to send again.
        (_, Err(e)) => app.toast(&crate::errors::words(&e)),
    }
}

/// Read the board again, only while a page of it shows.
fn reload_shown(app: &mut App) {
    if matches!(app.route(), Route::Board | Route::Task(_)) {
        reload(app);
    }
}

/// The send-back sheet: what to change, required, then Send back. Over any page that opens it.
pub fn draw_send_back(app: &mut App) {
    let Some((id, title)) = app.st.board.back.clone() else { return };
    let extra = crate::changes::prose_extra(app, "send-back", &app.st.board.back_words.clone(), WIDTH - 2.0 * t::PAD - 28.0);
    let h = 300.0 + extra;
    let sending = app.st.board.back_call.is_some();
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside && !sending {
        app.finish_typing();
        app.st.board.back = None;
        return;
    }
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    app.canvas.text(&format!("Send back #{id}"), x + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&title, x + 4.0, top + 58.0, w - 8.0, Text::new(ty::SMALL, t::INK3));
    let lt = Text::new(ty::CAPTION, t::INK2).medium();
    let fy = top + 88.0;
    app.canvas.text("What to change (the agent reads it)", x + 2.0, fy, lt);
    let by = fy + lt.lh() + 6.0;
    let bh = 48.0 + extra;
    let focused = app.focus.is("send-back");
    app.canvas.rect(x, by, w, bh, t::KEY_R, t::S1);
    app.canvas.outline(x, by, w, bh, t::KEY_R, 1.0, if focused { t::INK3 } else { t::LINE });
    let mut words = app.st.board.back_words.clone();
    crate::changes::prose_box(app, "send-back", "The tests fail on…", x + 14.0, by + 1.0, w - 28.0, bh, &mut words);
    app.st.board.back_words = words.clone();
    let ready = !words.trim().is_empty() && !sending && app.online();
    let label = if sending { "Sending back…" } else { "Send back to Active" };
    let ky = top + h - 62.0;
    let mut cx = crate::cx!(app);
    let go = if ready { kit::primary(&mut cx, x, ky, w, 46.0, label, None) } else { kit::plain(&mut cx, x, ky, w, 46.0, label, None) && false };
    if go {
        app.finish_typing();
        let call = app.link.call("task.comment", json!({"task_id": id, "body": words.trim()}));
        app.st.board.back_call = Some((id, call, false));
    }
}

impl State {
    /// Back: the send-back sheet closes (unless it is sending). False when it was not open.
    pub fn close_over(&mut self) -> bool {
        if self.creating.is_none() && self.new_task.take().is_some() {
            return true;
        }
        self.back_call.is_none() && self.back.take().is_some()
    }
}

fn prepare(app: &mut App) {
    if app.st.board.repo != app.repo {
        app.st.board = State { repo: app.repo, column: 2, ..State::default() };
    }
    let changed = app.take_changed("task");
    if app.fresh || changed {
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
    if !app.st.board.loaded || list.is_empty() {
        let error = app.st.board.error.clone();
        let column = COLUMNS[app.st.board.column.min(4)].1;
        let empty_body = format!("Tasks moved to {column} show here.");
        let (h, retry) = match (app.st.board.loaded, &error, app.online()) {
            (true, _, _) => {
                let (h, add) = kit::empty(&mut crate::cx!(app), y, &format!("Nothing in {column}"), Some(&empty_body), Some("New task"));
                if add {
                    new_task(app);
                }
                (h, false)
            }
            (false, Some(e), online) => kit::problem(&mut crate::cx!(app), y, "The board didn't load", e, online),
            (false, None, true) => kit::empty(&mut crate::cx!(app), y, "Reading the board…", None, None),
            (false, None, false) => kit::empty(&mut crate::cx!(app), y, "Waiting for your PC", Some("The board shows here once the PC answers."), None),
        };
        if retry {
            app.st.board.error = None;
            reload(app);
        }
        y += h;
    }
    let tt = Text::new(ty::BODY, t::INK).semi().line(1.35);
    for tk in &list {
        let th = match app.st.board.heights.get(&tk.id) {
            Some((title, h)) if *title == tk.title => *h,
            _ => {
                let h = app.canvas.para_h(&tk.title, WIDTH - 2.0 * t::PAD - 28.0, tt, 3);
                app.st.board.heights.insert(tk.id, (tk.title.clone(), h));
                h
            }
        };
        let h = 14.0 + th + 6.0 + 18.0 + 14.0;
        // Off screen: its place only.
        let on_screen = y + app.canvas.dy;
        if on_screen > page.bottom || on_screen + h < page.top {
            y += h + 10.0;
            continue;
        }
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
    // At the list's foot: a new task, starting in this column.
    if app.st.board.loaded && !list.is_empty() {
        let label = "New task";
        let small = Text::new(ty::UI, t::INK2).medium();
        let lw = app.canvas.measure(label, small) + 52.0;
        let mut cx = crate::cx!(app);
        let x = t::PAD;
        if cx.pressed(x, y, lw, 44.0) {
            cx.c.rect(x, y, lw, 44.0, 10.0, t::WASH);
        }
        cx.c.icon("plus", x + 20.0, y + 22.0, 14.0, t::INK2);
        cx.c.text(label, x + 38.0, y + (44.0 - small.lh()) / 2.0, small);
        if cx.tap(x, y, lw, 44.0) {
            new_task(app);
        }
        y += 54.0;
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
    take_created(app);
    draw_new_task(app);
    page.moving || app.st.board.creating.is_some() && !crate::wake::has()
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
        let error = app.st.board.open.error.clone();
        let (h, retry) = match (&error, app.online()) {
            (Some(e), online) => kit::problem(&mut crate::cx!(app), page.top, "The task didn't load", e, online),
            (None, true) => kit::empty(&mut crate::cx!(app), page.top, "Reading the task…", None, None),
            (None, false) => kit::empty(&mut crate::cx!(app), page.top, "Waiting for your PC", Some("The task shows here once the PC answers."), None),
        };
        if retry {
            app.st.board.open.error = None;
            reload_task(app, id);
        }
        app.page_end(&page, page.top + h);
        back_bar(app, "");
        return page.moving;
    };
    // The comment's box grows with what is written, up to five lines.
    let comment_w = WIDTH - 2.0 * t::PAD - 60.0;
    let draft = app.st.board.comment.clone();
    let extra = crate::changes::prose_extra(app, "task-comment", &draft, comment_w);
    let foot_h = 124.0 + extra;
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
    let box_h = 44.0 + extra;
    app.canvas.rect(x, my, w, box_h, 12.0, t::S1);
    let mut comment = app.st.board.comment.clone();
    crate::changes::prose_box(app, "task-comment", "Comment", x + 14.0, my - 1.0, comment_w, box_h, &mut comment);
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
    let ky = my + box_h + 12.0;
    let busy = app.st.board.act.is_some();
    let next = next(&tk.column);
    let review = tk.column == "in_review";
    // An agent of today on it: its row shows above, and there is no second one to start.
    let live = tk.sessions.iter().any(|n| app.agents.iter().any(|a| &a.name == n));
    let side_w = if review { 130.0 } else if !live && tk.column != "done" { 150.0 } else { 0.0 };
    let gap = if side_w > 0.0 { 10.0 } else { 0.0 };
    let mut cx = crate::cx!(app);
    let step = match next {
        Some((_, words)) if !busy => kit::primary(&mut cx, x, ky, w - side_w - gap, 44.0, words, None),
        Some((_, words)) => kit::plain(&mut cx, x, ky, w - side_w - gap, 44.0, words, None) && false,
        None => false,
    };
    let side = side_w > 0.0 && kit::plain(&mut cx, x + w - side_w, ky, side_w, 44.0, if review { "Send back" } else { "Start an agent" }, None);
    if step && let Some((col, _)) = next {
        if review {
            // Approving is its own op: it closes the task and links its commit.
            approve(app, id);
        } else {
            app.st.board.act = Some(app.link.call("task.move", json!({"task_id": id, "column": col})));
        }
    }
    if side && review {
        send_back(app, id, &tk.title);
    } else if side {
        // D2's New agent, the task linked and its words written in: model and effort are chosen
        // there, and the new agent's chat opens once the PC has made it.
        app.finish_typing();
        let na = &mut app.st.newagent;
        na.linked = Some((id, tk.title.clone()));
        na.task = format!("Task #{}: {}\n\n{}", tk.id, tk.title, tk.body).trim().to_string();
        na.open = true;
    }
    draw_send_back(app);
    page.moving || app.st.board.back_call.is_some() && !crate::wake::has()
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

/// Errors on the Board and a task for the shot tool (`--state error`).
pub fn sample_error(app: &mut App) {
    let st = &mut app.st.board;
    st.loaded = false;
    st.tasks.clear();
    st.error = Some(crate::errors::words(&rc_link::BusError { kind: "unavailable".into(), code: "link.timeout".into(), message: "The PC did not answer in time".into(), hint: None }));
    st.open = Opened { id: 41, error: Some(crate::errors::words(&rc_link::BusError { kind: "unavailable".into(), code: "task.not_found".into(), message: "task 41 not found".into(), hint: None })), ..Opened::default() };
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

    /// Frames drawn at rest on `route`, after `warm` to settle: µs a frame (release, this machine).
    fn frame_cost(a: &mut App, warm: usize, n: usize) -> f64 {
        let mut clock = 2_000_000_000u64;
        for _ in 0..warm {
            clock += 16_666_666;
            a.frame(clock);
        }
        let t = std::time::Instant::now();
        for _ in 0..n {
            clock += 16_666_666;
            a.frame(clock);
        }
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    }

    fn bench_app(name: &str) -> App {
        struct NoHost(std::path::PathBuf);
        impl crate::app::Host for NoHost {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _text: &str) {}
        }
        let dir = std::env::temp_dir().join(format!("relay-code-bench-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a
    }

    /// The Board with 200 tasks (80 in the column shown), at rest:
    /// `cargo test --release board_frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn board_frame_cost() {
        let mut a = bench_app("board");
        let cols = ["backlog", "ready", "active", "in_review", "done"];
        a.st.board.tasks = (0..200)
            .map(|i| Task { id: i, title: format!("Task {i}: make the thing on the phone behave as the desktop's does"), column: (if i < 80 { "active" } else { cols[i as usize % 5] }).into(), sessions: vec!["bold-puffin".into()], labels: vec!["phone".into()], ..Task::default() })
            .collect();
        a.st.board.column = 2;
        a.push(crate::app::Route::Board);
        println!("board frame, 200 tasks: {:.0} µs", frame_cost(&mut a, 10, 200));
    }

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

    fn on_task(name: &str, column: &str) -> (App, u64) {
        let dir = std::env::temp_dir().join(format!("relay-code-board-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        if let Some(t) = a.st.board.open.task.as_mut() {
            t.column = column.into();
            t.sessions.clear();
        }
        a.push(Route::Task(41));
        let mut clock = 2_000_000_000u64;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        (a, clock)
    }

    fn lift(a: &mut App, clock: &mut u64, x: f32, y: f32) {
        use crate::ui::{Phase, Touch};
        for phase in [Phase::Down, Phase::Up] {
            *clock += 30_000_000;
            a.touch(Touch { id: 1, phase, x, y, t_ns: *clock });
            a.frame(*clock);
        }
    }

    #[test]
    fn start_an_agent_opens_new_agent_with_the_task_linked() {
        let (mut a, mut clock) = on_task("start", "active");
        // The foot's right key, on its last row.
        lift(&mut a, &mut clock, 390.0 - 20.0 - 75.0, 844.0 - 124.0 + 12.0 + 44.0 + 12.0 + 22.0);
        let na = &a.st.newagent;
        assert!(na.open, "New agent opens");
        assert_eq!(na.linked.as_ref().map(|l| l.0), Some(41));
        assert!(na.task.starts_with("Task #41: "), "{}", na.task);
    }

    #[test]
    fn send_back_wants_words_keeps_them_when_it_fails_and_back_closes_it() {
        let (mut a, mut clock) = on_task("back", "in_review");
        lift(&mut a, &mut clock, 390.0 - 20.0 - 65.0, 844.0 - 124.0 + 12.0 + 44.0 + 12.0 + 22.0);
        assert_eq!(a.st.board.back.as_ref().map(|b| b.0), Some(41), "Send back opens its sheet");
        // No words: its key does nothing.
        let key_y = 844.0 - 300.0 + 300.0 - 62.0 + 20.0;
        lift(&mut a, &mut clock, 195.0, key_y);
        assert!(a.st.board.back_call.is_none());
        // With words, offline: the comment fails at once, the words and the sheet stay.
        a.st.board.back_words = "The drawer test fails on a tablet.".into();
        a.st.board.back_call = Some((41, a.link.call("task.comment", json!({"task_id": 41, "body": "x"})), false));
        take_answers(&mut a);
        assert!(a.st.board.back_call.is_none() && a.st.board.back.is_some());
        assert_eq!(a.st.board.back_words, "The drawer test fails on a tablet.");
        // While it sends, Back leaves it; after, Back closes it.
        a.st.board.back_call = Some((41, rc_link::Call::never(), false));
        assert!(!a.st.board.close_over() && a.st.board.back.is_some());
        a.st.board.back_call = None;
        assert!(a.st.board.close_over() && a.st.board.back.is_none());
        clock += 1;
        let _ = clock;
    }

    #[test]
    fn approved_is_said_once_it_is_and_an_older_board_never_moves_a_task_back() {
        let (mut a, _) = on_task("approved", "in_review");
        // Refused: no "Approved".
        a.st.board.act = Some(a.link.call("task.approve", json!({})));
        a.st.board.act_done = Some("Approved");
        take_answers(&mut a);
        assert!(a.st.board.act.is_none() && a.st.board.act_done.is_none());
        // An older list out, asked again: dropped, the cards stay as they are.
        let tasks = a.st.board.tasks.clone();
        a.st.board.list = Some(a.link.call("task.list", json!({})));
        a.st.board.stale = true;
        take(&mut a);
        assert_eq!(a.st.board.tasks, tasks);
        assert!(!a.st.board.stale);
    }

    #[test]
    fn approve_is_one_call_and_a_second_tap_waits_for_it() {
        let (mut a, _) = on_task("approve", "in_review");
        a.st.board.act = Some(rc_link::Call::never());
        approve(&mut a, 41);
        assert!(a.st.board.act.is_some());
    }

    #[test]
    fn a_new_task_is_sent_with_its_title_column_and_body_when_there_is_one() {
        let t = NewTask { title: "  Pinch zoom jumps  ".into(), body: " ".into(), column: 2 };
        assert_eq!(create_payload(7, &t), json!({"project_id": 7, "title": "Pinch zoom jumps", "column": "active"}));
        let t = NewTask { title: "A".into(), body: "Why\nand how".into(), column: 0 };
        assert_eq!(create_payload(7, &t)["body"], "Why\nand how");
    }

    #[test]
    fn the_new_task_sheet_starts_on_the_column_shown_and_keeps_its_taps() {
        let dir = std::env::temp_dir().join(format!("relay-code-board-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Board);
        a.st.board.column = 2;
        new_task(&mut a);
        assert_eq!(a.st.board.new_task.as_ref().map(|t| t.column), Some(2), "Active, as shown");
        a.st.board.column = 3;
        new_task(&mut a);
        assert_eq!(a.st.board.new_task.as_ref().map(|t| t.column), Some(0), "In review is no place to start: Backlog");
        a.st.board.column = 2;
        let mut clock = 2_000_000_000u64;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        // On the sheet's title, over the board's cards: the sheet stays, no card opens.
        let top = 844.0 - 380.0;
        lift(&mut a, &mut clock, 200.0, top + 30.0);
        assert!(a.st.board.new_task.is_some() && *a.route() == Route::Board);
        // A failed create keeps the words; Back then closes the sheet.
        a.st.board.new_task.as_mut().unwrap().title = "Pinch zoom jumps".into();
        a.st.board.creating = Some(a.link.call("task.create", json!({})));
        take_created(&mut a);
        assert_eq!(a.st.board.new_task.as_ref().map(|t| t.title.as_str()), Some("Pinch zoom jumps"));
        assert!(a.st.board.close_over() && a.st.board.new_task.is_none());
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
