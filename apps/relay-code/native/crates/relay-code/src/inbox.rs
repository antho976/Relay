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
    pub session: String,
    pub op: String,
    /// What it wants, in a line: the command, the path, the request.
    pub what: String,
    pub why: String,
}

pub fn holds(v: &Value) -> Vec<Hold> {
    v.get("holds")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|h| h.get("state").and_then(Value::as_str) == Some("open"))
                .filter_map(|h| {
                    let d = h.get("details").cloned().unwrap_or(Value::Null);
                    let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    let what = [s("command"), s("value"), s("path")].into_iter().find(|x| !x.is_empty()).unwrap_or_else(|| h.get("op").and_then(Value::as_str).unwrap_or_default().to_string());
                    let kind = s("kind");
                    Some(Hold {
                        id: h.get("id")?.as_i64()?,
                        session: h.get("session").and_then(Value::as_str).unwrap_or("an agent").to_string(),
                        op: if kind.is_empty() { h.get("op").and_then(Value::as_str).unwrap_or_default().to_string() } else { kind },
                        what,
                        why: s("reason"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default)]
pub struct State {
    repo: Option<i64>,
    pub holds: Vec<Hold>,
    loaded: bool,
    list: Option<Call>,
    act: Option<Call>,
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
            app.st.inbox.holds = holds(&v);
            app.st.inbox.loaded = true;
            app.holds = app.st.inbox.holds.len();
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
    if app.st.inbox.repo != app.repo {
        app.st.inbox.repo = app.repo;
    }
    if app.fresh {
        reload(app);
    }
    keep_up(app);
    let page = app.page_begin(dt, crate::home::TOP_H, 0.0);
    let mut y = page.top + 4.0;
    let now_ms = rc_link::link::now_ms();
    let asking: Vec<crate::data::Agent> = app.agents.iter().filter(|a| a.needs_you()).cloned().collect();
    let holds = app.st.inbox.holds.clone();
    let review: Vec<crate::board::Task> = app.st.board.tasks.iter().filter(|x| x.column == "in_review").cloned().collect();
    if asking.is_empty() && holds.is_empty() && review.is_empty() {
        let (title, body) = if app.online() { ("Nothing needs you", "Agents that ask you something and commands held for you show here.") } else { ("Waiting for your PC", "What needs you shows here as soon as the PC answers.") };
        let (h, _) = kit::notice(&mut crate::cx!(app), 32.0, y + 120.0, WIDTH - 64.0, title, Some(body), None);
        y += 120.0 + h;
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
    let mut answer: Option<(i64, bool)> = None;
    if !holds.is_empty() {
        app.canvas.text("HELD FOR YOU", t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for h in &holds {
            let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
            let mono = Text::new(ty::MONO, t::INK).face(crate::draw::Weight::Mono).line(1.45);
            let why = Text::new(ty::CAPTION, t::INK2).line(1.45);
            let wh = app.canvas.para_h(&h.what, w - 28.0, mono, 4);
            let rh = if h.why.is_empty() { 0.0 } else { app.canvas.para_h(&h.why, w - 28.0, why, 4) + 6.0 };
            let card = 14.0 + 18.0 + 8.0 + wh + 8.0 + rh + 8.0 + 40.0 + 14.0;
            app.canvas.rect(x, y, w, card, 14.0, t::S1);
            app.canvas.text_fit(&format!("{} · {}", h.session, h.op), x + 14.0, y + 14.0, w - 28.0, Text::new(ty::CAPTION, t::INK3).medium());
            let mut cy = y + 40.0;
            cy += app.canvas.para(&h.what, x + 14.0, cy, w - 28.0, mono, 4) + 8.0;
            if !h.why.is_empty() {
                cy += app.canvas.para(&h.why, x + 14.0, cy, w - 28.0, why, 4) + 6.0;
            }
            cy += 8.0;
            let kw = (w - 28.0 - 10.0) / 2.0;
            let mut cx = crate::cx!(app);
            let busy = app.st.inbox.act.is_some();
            if kit::primary(&mut cx, x + 14.0, cy, kw, 40.0, "Allow once", None) && !busy {
                answer = Some((h.id, true));
            }
            if kit::plain(&mut cx, x + 24.0 + kw, cy, kw, 40.0, "Deny", None) && !busy {
                answer = Some((h.id, false));
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
    app.page_end(&page, y);
    crate::home::top(app, "Inbox");
    if let Some((id, allow)) = answer {
        let call = if allow { app.link.call("guardrail.confirm", json!({"hold_id": id})) } else { app.link.call("guardrail.reject", json!({"hold_id": id})) };
        app.st.inbox.act = Some(call);
        app.st.inbox.holds.retain(|h| h.id != id);
        app.holds = app.st.inbox.holds.len();
        app.toast(if allow { "Allowed once" } else { "Denied" });
    }
    if let Some(n) = open {
        app.open(&n);
    } else if let Some(id) = task {
        app.push(Route::Task(id));
    }
    page.moving
}

/// Sample holds for screenshots.
pub fn sample(app: &mut App) {
    let v = json!({"holds": [{"id": 201, "session": "umber-zebra", "op": "guardrail.request", "state": "open", "details": {"kind": "command", "value": "git push --force-with-lease origin relay/umber-zebra", "reason": "The branch was rebased on main after review; the PR needs the rewritten history."}}]});
    app.st.inbox = State { repo: app.repo, holds: holds(&v), loaded: true, ..State::default() };
    app.holds = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_holds_read_with_what_they_want() {
        let v = json!({"holds": [
            {"id": 1, "session": "a-b", "op": "guardrail.request", "state": "open", "details": {"kind": "path", "value": ".github/workflows/x.yml", "reason": "CI for x"}},
            {"id": 2, "session": "c-d", "op": "exec", "state": "confirmed", "details": {"command": "rm -rf"}},
        ]});
        let h = holds(&v);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].what.as_str(), h[0].op.as_str(), h[0].why.as_str()), (".github/workflows/x.yml", "path", "CI for x"));
    }
}
