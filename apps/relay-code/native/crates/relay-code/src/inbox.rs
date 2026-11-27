//! The inbox: what needs you, first. Agents that ask you something (a tap opens the chat, where
//! the question card answers it), commands and requests the guardrails hold for you (Allow once,
//! Deny: answered here), then work waiting in review.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::{App, Route};
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hold {
    pub id: i64,
    /// None for a hold that belongs to no repo: it shows under every one.
    pub project_id: Option<i64>,
    pub session: String,
    pub op: String,
    /// What it wants, in a line: the command, the path, the request.
    pub what: String,
    pub why: String,
    /// An agent's request for an exception (`guardrail.request`): it can be granted once or for
    /// the session, and says which it asked for.
    pub request: bool,
    pub asked_scope: Option<String>,
    /// What a destructive write would remove: "Removes 120 of 300 lines (40%), adds 4".
    pub removes: Option<String>,
}

/// How much of a file a held write would take away, from the destructive-write rule's details
/// (as they come, or under `original` when the hold is the person's own bypass).
fn removes(d: &Value) -> Option<String> {
    let d = d.get("original").unwrap_or(d);
    let n = |k: &str| d.get(k).and_then(Value::as_i64);
    let removed = n("removed_lines")?;
    let old = n("old_lines").map(|o| format!(" of {o}")).unwrap_or_default();
    let pct = d.get("removed_pct").and_then(Value::as_f64).map(|p| format!(" ({p:.0}%)")).unwrap_or_default();
    let added = n("added_lines").filter(|a| *a > 0).map(|a| format!(", adds {a}")).unwrap_or_default();
    Some(format!("Removes {removed}{old} line{}{pct}{added}", if removed == 1 && old.is_empty() { "" } else { "s" }))
}

pub fn holds(v: &Value) -> Vec<Hold> {
    v.get("holds")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|h| h.get("state").and_then(Value::as_str) == Some("open"))
                .filter_map(|h| {
                    let d = h.get("details").cloned().unwrap_or(Value::Null);
                    let s = |k: &str| d.get(k).and_then(Value::as_str).or_else(|| d.pointer(&format!("/original/{k}")).and_then(Value::as_str)).unwrap_or_default().to_string();
                    let op = h.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
                    let what = [s("command"), s("value"), s("path")].into_iter().find(|x| !x.is_empty()).unwrap_or_else(|| op.clone());
                    let kind = s("kind");
                    let scope = s("scope");
                    Some(Hold {
                        id: h.get("id")?.as_i64()?,
                        project_id: h.get("project_id").and_then(Value::as_i64),
                        session: h.get("session").and_then(Value::as_str).unwrap_or("an agent").to_string(),
                        request: op == "guardrail.request",
                        op: if kind.is_empty() { op } else { kind },
                        what,
                        why: s("reason"),
                        asked_scope: (!scope.is_empty()).then_some(scope),
                        removes: removes(&d),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
pub struct State {
    repo: Option<i64>,
    /// Every open hold; the page and the drawer's count show the repo's (and those of none).
    pub all: Vec<Hold>,
    pub holds: Vec<Hold>,
    loaded: bool,
    list: Option<Call>,
    act: Option<Call>,
    /// The repo's tasks waiting in review, read by the page itself.
    pub review: Vec<crate::board::Task>,
    review_call: Option<Call>,
    /// The hold being denied, and the reason being written for it.
    pub denying: Option<i64>,
    pub reason: String,
}

impl State {
    /// Back: the deny sheet closes. False when it was not open.
    pub fn close_over(&mut self) -> bool {
        self.denying.take().is_some()
    }
}

/// The holds that show under `repo`: its own, and those that belong to no repo.
pub fn for_repo(all: &[Hold], repo: Option<i64>) -> Vec<Hold> {
    all.iter().filter(|h| h.project_id.is_none() || repo.is_none() || h.project_id == repo).cloned().collect()
}

fn reload(app: &mut App) {
    if app.online() && app.st.inbox.list.is_none() {
        app.st.inbox.list = Some(app.link.call("guardrail.holds.list", json!({"open_only": true, "limit": 50})));
    }
}

fn take(app: &mut App) {
    if let Some(r) = app.st.inbox.list.as_ref().and_then(Call::poll) {
        app.st.inbox.list = None;
        if let Ok(v) = r {
            app.st.inbox.all = holds(&v);
            app.st.inbox.loaded = true;
        }
    }
    // The repo chip can change on any page: the count follows it.
    let st = &mut app.st.inbox;
    st.holds = for_repo(&st.all, app.repo);
    app.holds = st.holds.len();
    if let Some(r) = app.st.inbox.review_call.as_ref().and_then(Call::poll) {
        app.st.inbox.review_call = None;
        if let Ok(v) = r {
            app.st.inbox.review = crate::board::tasks(&v);
        }
    }
    if let Some(r) = app.st.inbox.act.as_ref().and_then(Call::poll) {
        app.st.inbox.act = None;
        if let Err(e) = r {
            app.toast(&e.message);
        }
        reload(app);
    }
}

/// Read the held commands in the background too, so the drawer's Inbox count is right.
pub fn keep_up(app: &mut App) {
    if app.take_changed("guardrail") {
        reload(app);
    }
    take(app);
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let moved = app.st.inbox.repo != app.repo;
    if moved {
        app.st.inbox.repo = app.repo;
        app.st.inbox.review.clear();
        app.st.inbox.review_call = None;
    }
    if app.fresh {
        reload(app);
    }
    // In review: the repo's own, read here, not whatever the board last read.
    let changed = app.take_changed("task");
    if (app.fresh || moved || changed)
        && app.online()
        && let Some(repo) = app.repo
        && app.st.inbox.review_call.is_none()
    {
        app.st.inbox.review_call = Some(app.link.call("task.list", json!({"project_id": repo, "column": "in_review", "summary": true})));
    }
    keep_up(app);
    let page = app.page_begin(dt, crate::home::TOP_H, 0.0);
    let mut y = page.top + 4.0;
    let now_ms = rc_link::link::now_ms();
    // Blocked at a prompt asks you; blocked because it reported so has stopped, with nothing to
    // answer: a quieter list, last.
    let (asking, stopped) = split_blocked(&app.agents);
    let holds = app.st.inbox.holds.clone();
    let review = app.st.inbox.review.clone();
    if asking.is_empty() && holds.is_empty() && review.is_empty() && stopped.is_empty() {
        let (title, body) = if app.online() { ("Nothing needs you", "Agents that ask you something and commands held for you show here.") } else { ("Waiting for your PC", "What needs you shows here as soon as the PC answers.") };
        let (h, _) = kit::empty(&mut crate::cx!(app), y, title, Some(body), None);
        y += h;
    }
    let mut open = None;
    if !asking.is_empty() {
        app.canvas.text("ASKS YOU", t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for a in &asking {
            if crate::drawer::agent_row(app, a, 0.0, y, WIDTH, now_ms) {
                open = Some(a.name.clone());
            }
            y += 60.0;
        }
        y += 8.0;
    }
    let mut answer: Option<Answer> = None;
    if !holds.is_empty() {
        app.canvas.text("HELD FOR YOU", t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for h in &holds {
            let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
            let mono = Text::new(ty::MONO, t::INK).face(crate::draw::Weight::Mono).line(1.45);
            let why = Text::new(ty::CAPTION, t::INK2).line(1.45);
            let gone = Text::new(ty::CAPTION, t::DANGER).medium();
            let wh = app.canvas.para_h(&h.what, w - 28.0, mono, 4);
            let gh = if h.removes.is_some() { gone.lh() + 6.0 } else { 0.0 };
            let rh = if h.why.is_empty() { 0.0 } else { app.canvas.para_h(&h.why, w - 28.0, why, 4) + 6.0 };
            // An exception request can also be granted for the rest of the agent's session.
            let sh = if h.request { 40.0 } else { 0.0 };
            let card = 14.0 + 18.0 + 8.0 + wh + 8.0 + gh + rh + 8.0 + 40.0 + sh + 14.0;
            app.canvas.rect(x, y, w, card, 14.0, t::S1);
            let asked = match h.asked_scope.as_deref() {
                Some("session") => " · asks for the session",
                _ => "",
            };
            app.canvas.text_fit(&format!("{} · {}{asked}", h.session, h.op), x + 14.0, y + 14.0, w - 28.0, Text::new(ty::CAPTION, t::INK3).medium());
            let mut cy = y + 40.0;
            cy += app.canvas.para(&h.what, x + 14.0, cy, w - 28.0, mono, 4) + 8.0;
            if let Some(r) = &h.removes {
                app.canvas.text_fit(r, x + 14.0, cy, w - 28.0, gone);
                cy += gh;
            }
            if !h.why.is_empty() {
                cy += app.canvas.para(&h.why, x + 14.0, cy, w - 28.0, why, 4) + 6.0;
            }
            cy += 8.0;
            let kw = (w - 28.0 - 10.0) / 2.0;
            let mut cx = crate::cx!(app);
            let busy = app.st.inbox.act.is_some();
            if kit::primary(&mut cx, x + 14.0, cy, kw, 40.0, "Allow once", None) && !busy {
                answer = Some(Answer::Allow(h.id, "once"));
            }
            if kit::plain(&mut cx, x + 24.0 + kw, cy, kw, 40.0, "Deny", None) && !busy {
                answer = Some(Answer::Deny(h.id));
            }
            if h.request {
                let sy = cy + 40.0 + 4.0;
                let label = "Allow for this session";
                let small = Text::new(ty::SMALL, t::INK2).medium();
                let lw = cx.c.measure(label, small) + 24.0;
                if cx.pressed(x + 14.0, sy, lw, 36.0) {
                    cx.c.rect(x + 14.0, sy, lw, 36.0, 8.0, t::WASH);
                }
                cx.c.text(label, x + 26.0, sy + 9.0, small);
                if cx.tap(x + 14.0, sy - 4.0, lw, 44.0) && !busy {
                    answer = Some(Answer::Allow(h.id, "session"));
                }
            }
            y += card + 10.0;
        }
    }
    let mut task = None;
    if !review.is_empty() {
        app.canvas.text("IN REVIEW", t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for tk in &review {
            let mut cx = crate::cx!(app);
            if cx.pressed(0.0, y, WIDTH, 48.0) {
                cx.c.rect(8.0, y, WIDTH - 16.0, 48.0, 10.0, t::WASH);
            }
            cx.c.icon("board", t::PAD + 10.0, y + 24.0, 16.0, t::INK2);
            cx.c.text_fit(&tk.title, t::PAD + 32.0, y + 14.0, WIDTH - t::PAD * 2.0 - 40.0, Text::new(ty::BODY, t::INK));
            if cx.tap(0.0, y, WIDTH, 48.0) {
                task = Some(tk.id);
            }
            y += 48.0;
        }
    }
    if !stopped.is_empty() {
        app.canvas.text("STOPPED", t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for a in &stopped {
            if crate::drawer::agent_row(app, a, 0.0, y, WIDTH, now_ms) {
                open = Some(a.name.clone());
            }
            y += 60.0;
        }
    }
    app.page_end(&page, y);
    crate::home::top(app, "Inbox");
    match answer {
        Some(Answer::Allow(id, scope)) => {
            let request = app.st.inbox.holds.iter().any(|h| h.id == id && h.request);
            app.st.inbox.act = Some(app.link.call("guardrail.confirm", confirm_payload(id, request, scope)));
            answered(app, id);
            app.toast(if scope == "session" { "Allowed for this session" } else { "Allowed once" });
        }
        Some(Answer::Deny(id)) => {
            app.st.inbox.denying = Some(id);
            app.st.inbox.reason.clear();
        }
        None => {}
    }
    if app.st.inbox.denying.is_some() {
        deny_sheet(app);
    }
    if let Some(n) = open {
        app.open(&n);
    } else if let Some(id) = task {
        app.push(Route::Task(id));
    }
    page.moving
}

/// Blocked agents: those at a prompt (asking you), and those that stopped and reported blocked.
/// No `waiting` is an older PC, which only blocked at prompts.
pub fn split_blocked(agents: &[crate::data::Agent]) -> (Vec<crate::data::Agent>, Vec<crate::data::Agent>) {
    agents.iter().filter(|a| a.needs_you()).cloned().partition(|a| a.waiting.as_deref() != Some("report"))
}

enum Answer {
    Allow(i64, &'static str),
    Deny(i64),
}

/// Allow: an exception request says its scope outright, since the schema's default is the scope
/// the agent asked for. Other holds have none.
pub fn confirm_payload(id: i64, request: bool, scope: &str) -> Value {
    if request { json!({"hold_id": id, "scope": scope}) } else { json!({"hold_id": id}) }
}

pub fn reject_payload(id: i64, reason: &str) -> Value {
    let r = reason.trim();
    if r.is_empty() { json!({"hold_id": id}) } else { json!({"hold_id": id, "reason": r}) }
}

/// The hold is answered: it leaves the list and the count at once.
fn answered(app: &mut App, id: i64) {
    let st = &mut app.st.inbox;
    st.all.retain(|h| h.id != id);
    st.holds.retain(|h| h.id != id);
    app.holds = st.holds.len();
}

/// Deny, with a reason the agent reads (optional).
fn deny_sheet(app: &mut App) {
    let Some(id) = app.st.inbox.denying else { return };
    let what = app.st.inbox.holds.iter().find(|h| h.id == id).map(|h| h.what.clone()).unwrap_or_default();
    let h = 250.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside {
        app.finish_typing();
        app.st.inbox.denying = None;
        return;
    }
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    app.canvas.text("Deny", x + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&what, x + 4.0, top + 58.0, w - 8.0, Text::new(ty::SMALL, t::INK3).face(crate::draw::Weight::Mono));
    let mut reason = app.st.inbox.reason.clone();
    kit::field(&mut crate::cx!(app), "deny-reason", x, top + 88.0, w, "Why (the agent reads it)", "Optional", &mut reason, kit::Kb::Text);
    app.st.inbox.reason = reason.clone();
    if kit::primary(&mut crate::cx!(app), x, top + h - 62.0, w, 46.0, "Deny", None) {
        app.finish_typing();
        app.st.inbox.act = Some(app.link.call("guardrail.reject", reject_payload(id, &reason)));
        app.st.inbox.denying = None;
        answered(app, id);
        app.toast("Denied");
    }
}

/// Sample holds for screenshots.
pub fn sample(app: &mut App) {
    let v = json!({"holds": [
        {"id": 201, "project_id": 1, "session": "umber-zebra", "op": "guardrail.request", "state": "open", "details": {"kind": "command", "value": "git push --force-with-lease origin relay/umber-zebra", "reason": "The branch was rebased on main after review; the PR needs the rewritten history.", "scope": "once"}},
        {"id": 202, "project_id": 1, "session": "bold-puffin", "op": "file.write", "state": "open", "policy": "destructive_write", "details": {"path": "apps/relay-code/native/crates/relay-code/src/chat.rs", "removed_lines": 412, "added_lines": 38, "old_lines": 1083, "removed_pct": 38.04}},
        {"id": 203, "project_id": 2, "session": "calm-heron", "op": "guardrail.request", "state": "open", "details": {"kind": "path", "value": ".github/workflows/ci.yml"}}]});
    let all = holds(&v);
    let shown = for_repo(&all, app.repo);
    app.holds = shown.len();
    let review = vec![crate::board::Task { id: 88, title: "Leaf: bookmark marked on the strip".into(), column: "in_review".into(), ..crate::board::Task::default() }];
    app.st.inbox = State { repo: app.repo, all, holds: shown, loaded: true, review, ..State::default() };
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

    /// The Inbox with 30 items (holds and tasks in review), at rest:
    /// `cargo test --release inbox_frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn inbox_frame_cost() {
        let mut a = bench_app("inbox");
        let holds: Vec<Hold> = (0..20)
            .map(|i| Hold { id: i, project_id: Some(1), session: "bold-puffin".into(), op: "command".into(), what: format!("git push --force-with-lease origin relay/branch-{i}"), why: "The branch was rebased on main after review; the PR needs the rewritten history.".into(), request: true, ..Hold::default() })
            .collect();
        a.st.inbox.all = holds.clone();
        a.st.inbox.holds = holds;
        a.st.inbox.review = (0..10).map(|i| crate::board::Task { id: i, title: format!("Review {i}: the change to the drawer"), column: "in_review".into(), ..crate::board::Task::default() }).collect();
        a.push(crate::app::Route::Inbox);
        println!("inbox frame, 30 items: {:.0} µs", frame_cost(&mut a, 10, 200));
    }

    #[test]
    fn open_holds_read_with_what_they_want() {
        let v = json!({"holds": [
            {"id": 1, "session": "a-b", "op": "guardrail.request", "state": "open", "details": {"kind": "path", "value": ".github/workflows/x.yml", "reason": "CI for x"}},
            {"id": 2, "session": "c-d", "op": "exec", "state": "confirmed", "details": {"command": "rm -rf"}},
        ]});
        let h = holds(&v);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].what.as_str(), h[0].op.as_str(), h[0].why.as_str()), (".github/workflows/x.yml", "path", "CI for x"));
        assert!(h[0].request);
    }

    struct NoHost(std::path::PathBuf);

    impl crate::app::Host for NoHost {
        fn data_dir(&self) -> std::path::PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    fn press(a: &mut App, clock: &mut u64, x: f32, y: f32) {
        use crate::ui::{Phase, Touch};
        for phase in [Phase::Down, Phase::Up] {
            *clock += 30_000_000;
            a.touch(Touch { id: 1, phase, x, y, t_ns: *clock });
            a.frame(*clock);
        }
        for _ in 0..20 {
            *clock += 16_666_666;
            a.frame(*clock);
        }
    }

    #[test]
    fn deny_opens_a_sheet_whose_taps_reach_nothing_under_it() {
        let dir = std::env::temp_dir().join(format!("relay-code-inbox-deny-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Inbox);
        let mut clock = 2_000_000_000;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        a.st.inbox.denying = Some(201);
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        // On the sheet's title, over the cards' keys and the review list.
        let top = a.canvas.h - 250.0;
        press(&mut a, &mut clock, 200.0, top + 30.0);
        assert_eq!(a.st.inbox.denying, Some(201), "the sheet stays");
        assert!(a.st.inbox.act.is_none() && a.st.inbox.holds.len() == 2, "nothing under it was answered");
        assert_eq!(*a.route(), Route::Inbox, "no review row under it opened");
        // A tap above it closes it, and answers nothing.
        press(&mut a, &mut clock, 200.0, 150.0);
        assert_eq!(a.st.inbox.denying, None);
        assert!(a.st.inbox.act.is_none() && a.st.inbox.holds.len() == 2);
    }

    #[test]
    fn agents_that_reported_blocked_are_stopped_not_asking() {
        let ag = |n: &str, state: &str, w: Option<&str>| crate::data::Agent { name: n.into(), state: state.into(), waiting: w.map(str::to_string), ..crate::data::Agent::default() };
        let list = [ag("a", "blocked", Some("prompt")), ag("b", "blocked", Some("report")), ag("c", "blocked", None), ag("d", "running", None)];
        let (asking, stopped) = split_blocked(&list);
        assert_eq!(asking.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!(stopped.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn allow_once_says_once_and_deny_carries_its_reason() {
        assert_eq!(confirm_payload(7, true, "once"), json!({"hold_id": 7, "scope": "once"}));
        assert_eq!(confirm_payload(7, true, "session"), json!({"hold_id": 7, "scope": "session"}));
        // Not an exception request: no scope to give.
        assert_eq!(confirm_payload(7, false, "once"), json!({"hold_id": 7}));
        assert_eq!(reject_payload(7, "  "), json!({"hold_id": 7}));
        assert_eq!(reject_payload(7, " not on main "), json!({"hold_id": 7, "reason": "not on main"}));
    }

    #[test]
    fn a_destructive_write_says_what_it_removes_and_holds_follow_the_repo() {
        let v = json!({"holds": [
            {"id": 1, "project_id": 1, "session": "a", "op": "file.write", "state": "open", "details": {"path": "src/a.rs", "removed_lines": 412, "added_lines": 38, "old_lines": 1083, "removed_pct": 38.04}},
            {"id": 2, "project_id": 2, "session": "b", "op": "file.write", "state": "open", "details": {"policy": "destructive_write", "original": {"path": "src/b.rs", "removed_lines": 900, "added_lines": 0}}},
            {"id": 3, "session": "c", "op": "guardrail.request", "state": "open", "details": {"kind": "command", "value": "git push -f", "scope": "session"}},
        ]});
        let h = holds(&v);
        assert_eq!(h[0].removes.as_deref(), Some("Removes 412 of 1083 lines (38%), adds 38"));
        assert_eq!((h[1].what.as_str(), h[1].removes.as_deref()), ("src/b.rs", Some("Removes 900 lines")));
        assert_eq!((h[2].removes.as_deref(), h[2].asked_scope.as_deref()), (None, Some("session")));
        let ids = |r| for_repo(&h, r).iter().map(|h| h.id).collect::<Vec<_>>();
        assert_eq!(ids(Some(1)), [1, 3]);
        assert_eq!(ids(Some(2)), [2, 3]);
    }
}
