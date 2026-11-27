//! Agents you manage (backlog D1): the ones that stopped or finished ("Earlier", from
//! `session.list` with closed ones and `session.restorable`), each opening its conversation
//! read-only with Resume; Pause and Close from a chat's menu; search by name and intent in the
//! drawer.
//!
//! Live agents are the ones with a terminal running or starting. Parked, restorable and exited
//! ones keep their checkout and come back (Wake, Resume); closed ones are gone but for their
//! branch and their conversation, which still reads.

use serde_json::json;

use crate::app::{App, Ask};
use crate::data::Agent;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

/// The read-only foot of a stopped agent's chat, in place of the message box.
pub const STOPPED_H: f32 = 76.0;
/// Earlier agents shown before "Show all".
pub const FEW: usize = 5;
/// The search field in the drawer.
pub const SEARCH: &str = "drawer-search";
/// The earlier list is read again at most this often while it shows (ns), events aside.
const EVERY_NS: u64 = 60_000_000_000;
/// ... and at most this often after events (ns).
const SOON_NS: u64 = 5_000_000_000;

#[derive(Default)]
pub struct State {
    /// The agent whose chat menu is open.
    pub menu: Option<String>,
    /// A confirm up for this agent: close it, or close it and drop its uncommitted changes.
    pub confirm: Option<(String, Confirm)>,
    /// What the drawer's search holds.
    pub query: String,
    /// "Show all" was tapped: every earlier agent shows, not the newest few.
    pub all: bool,
    /// The repo the earlier list was read for, and when (ns).
    asked: Option<(i64, u64)>,
    /// A session event came: read the earlier list again next time it shows.
    pub stale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Confirm {
    Close,
    Discard,
}

impl State {
    /// Close the menu or the dialog; false when neither was up.
    pub fn close_over(&mut self) -> bool {
        self.menu.take().is_some() || self.confirm.take().is_some()
    }
}

/// A terminal runs for it, or is starting: it takes messages.
pub fn live(a: &Agent) -> bool {
    matches!(a.state.as_str(), "created" | "spawning" | "running" | "idle" | "blocked")
}

/// It stopped with its checkout kept: Resume or Wake brings it back.
pub fn resumable(a: &Agent) -> bool {
    matches!(a.state.as_str(), "restorable" | "exited" | "parked")
}

/// The words a search looks in: name and intent, ignoring case.
pub fn matches(a: &Agent, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || a.name.to_lowercase().contains(&q) || a.intent.as_deref().is_some_and(|i| i.to_lowercase().contains(&q))
}

/// The repo's agents that are not live, newest first: the stopped ones from the live list and the
/// closed ones read for Earlier, each once.
pub fn earlier(app: &App) -> Vec<Agent> {
    let mut out: Vec<Agent> = app.agents.iter().filter(|a| Some(a.project_id) == app.repo && !live(a)).cloned().collect();
    for a in app.earlier.iter().filter(|a| Some(a.project_id) == app.repo && !live(a)) {
        if !out.iter().any(|b| b.name == a.name) && !app.agents.iter().any(|b| b.name == a.name) {
            out.push(a.clone());
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

/// Read the earlier list and the restorable reasons when they show and are stale: once per repo,
/// after a session event, and at most once a minute otherwise.
pub fn keep_up(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if !app.online() {
        return;
    }
    let s = &app.st.agents;
    let due = match s.asked {
        Some((r, at)) => r != repo || (s.stale && app.now_ns >= at + SOON_NS) || app.now_ns >= at + EVERY_NS,
        None => true,
    };
    if !due {
        return;
    }
    app.st.agents.asked = Some((repo, app.now_ns));
    app.st.agents.stale = false;
    app.ask(Ask::Earlier, "session.list", json!({"project_id": repo, "include_closed": true}));
    app.ask(Ask::Restorable, "session.restorable", json!({"project_id": repo}));
}

/// Its state in a few words for a stopped agent's foot.
fn stopped_words(app: &App, a: &Agent) -> String {
    match a.state.as_str() {
        "parked" => "Paused. Wake it to go on where it left off.".into(),
        "restorable" => match app.restorable.get(&a.name).map(|(r, _)| r.as_str()) {
            Some("crash") => "Stopped when Relay crashed. Resume it to go on.".into(),
            Some("app_restart") => "Stopped when Relay closed. Resume it to go on.".into(),
            _ => "Stopped. Resume it to go on.".into(),
        },
        "exited" => "Its terminal quit. Resume it to go on.".into(),
        _ => format!("Closed. Its branch {} is kept.", a.branch),
    }
}

/// The foot of a stopped agent's chat: what happened, and Resume or Wake when it can come back.
/// Drawn by the chat in place of the message box, over the page.
pub fn stopped_foot(app: &mut App, a: &Agent) {
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - STOPPED_H;
    let band = [-1000.0, top, WIDTH + 2000.0, STOPPED_H + app.canvas.inset_bottom + 2.0];
    app.canvas.dy = 0.0;
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(band[0], band[1], band[2], band[3], 0.0, t::BG);
    app.canvas.rect(0.0, top, WIDTH, 1.0, 0.0, t::LINE);
    let back = resumable(a);
    let waking = app.asking(&Ask::Resume(a.name.clone()));
    let kw = if back { 112.0 } else { 0.0 };
    let words = stopped_words(app, a);
    let tt = Text::new(ty::UI, t::INK2).line(1.4);
    let tw = WIDTH - 2.0 * t::PAD - if back { kw + 12.0 } else { 0.0 };
    let th = app.canvas.para_h(&words, tw, tt, 2);
    app.canvas.para(&words, t::PAD, top + (STOPPED_H - th) / 2.0, tw, tt, 2);
    if !back {
        return;
    }
    let (kx, ky) = (WIDTH - t::PAD - kw, top + (STOPPED_H - 44.0) / 2.0);
    let label = if waking { "Starting…" } else if a.state == "parked" { "Wake" } else { "Resume" };
    let mut cx = crate::cx!(app);
    let tapped = if waking { kit::plain(&mut cx, kx, ky, kw, 44.0, label, None) && false } else { kit::primary(&mut cx, kx, ky, kw, 44.0, label, Some("play")) };
    if tapped {
        resume(app, a);
    }
}

/// Bring a stopped agent back: Wake for a parked one, Resume for the others.
pub fn resume(app: &mut App, a: &Agent) {
    let op = if a.state == "parked" { "session.wake" } else { "session.resume" };
    app.ask(Ask::Resume(a.name.clone()), op, json!({"session": a.name}));
}

/// The chat header's menu key, at the top right. Not for a closed agent: there is nothing left to do.
pub fn more_key(app: &mut App, a: &Agent, x: f32, y: f32) {
    if a.state == "closed" {
        return;
    }
    let mut cx = crate::cx!(app);
    if kit::icon_key_ink(&mut cx, x, y, "more", t::INK2) {
        app.st.agents.menu = Some(a.name.clone());
    }
}

/// The chat menu and its confirms, over everything. Call after the page draws.
pub fn draw_over(app: &mut App) {
    if let Some(name) = app.st.agents.menu.clone() {
        let Some(a) = app.agent(&name).cloned() else {
            app.st.agents.menu = None;
            return;
        };
        let items: &[&str] = if live(&a) { &["Pause agent", "Close agent"] } else { &["Close agent"] };
        let top = app.canvas.inset_top + 52.0;
        match kit::menu(&mut crate::cx!(app), top, items) {
            Some(i) if items.get(i) == Some(&"Pause agent") => {
                app.st.agents.menu = None;
                app.ask(Ask::Park(name.clone()), "session.park", json!({"session": name}));
                app.toast("Pausing the agent…");
            }
            Some(i) if items.get(i) == Some(&"Close agent") => {
                app.st.agents.menu = None;
                app.st.agents.confirm = Some((name, Confirm::Close));
            }
            Some(_) => app.st.agents.menu = None,
            None => {}
        }
        return;
    }
    let Some((name, kind)) = app.st.agents.confirm.clone() else { return };
    let branch = app.agent(&name).map(|a| a.branch.clone()).unwrap_or_default();
    let (title, body, act) = match kind {
        Confirm::Close => (
            format!("Close {name}?"),
            format!("Its terminal stops and its checkout is removed. The branch {branch} and the conversation are kept."),
            "Close",
        ),
        Confirm::Discard => (
            format!("{name} has uncommitted changes"),
            "Closing it now deletes them. Commit them from Changes first, or close it anyway.".to_string(),
            "Delete and close",
        ),
    };
    match kit::confirm(&mut crate::cx!(app), &title, &body, act) {
        Some(true) => {
            app.st.agents.confirm = None;
            let mut p = json!({"session": name});
            if kind == Confirm::Discard {
                p["discard_changes"] = json!(true);
            }
            app.ask(Ask::Close(name), "session.close", p);
        }
        Some(false) => app.st.agents.confirm = None,
        None => {}
    }
}

/// A section's label and its rows. The agent tapped, by name; rows outside `window` (screen
/// units, for a list that scrolls in a clip) take no tap.
pub fn section(app: &mut App, label: &str, list: &[Agent], x: f32, y: &mut f32, w: f32, window: Option<(f32, f32)>) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    let now_ms = rc_link::link::now_ms();
    app.canvas.text(label, x + t::PAD, *y + 10.0, Text::new(ty::SMALL, t::INK3).semi());
    *y += 32.0;
    let mut open = None;
    for a in list {
        // Rows outside the list's window take no tap.
        let visible = window.is_none_or(|(top, bottom)| *y + crate::drawer::AGENT_ROW_H > top && *y < bottom);
        if crate::drawer::agent_row(app, a, x, *y, w, now_ms) && visible {
            open = Some(a.name.clone());
        }
        *y += crate::drawer::AGENT_ROW_H;
    }
    *y += 4.0;
    open
}

/// Earlier, the newest few and a row to show the rest. The agent tapped, by name.
pub fn earlier_section(app: &mut App, x: f32, y: &mut f32, w: f32, window: Option<(f32, f32)>) -> Option<String> {
    let all = earlier(app);
    if all.is_empty() {
        return None;
    }
    let shown: Vec<Agent> = if app.st.agents.all { all.clone() } else { all.iter().take(FEW).cloned().collect() };
    let open = section(app, "EARLIER", &shown, x, y, w, window);
    if all.len() > FEW {
        let words = if app.st.agents.all { "Show fewer".to_string() } else { format!("Show all {}", all.len()) };
        let h = 44.0;
        let visible = window.is_none_or(|(top, bottom)| *y + h > top && *y < bottom);
        let mut cx = crate::cx!(app);
        if cx.pressed(x + 8.0, *y, w - 16.0, h) {
            cx.c.rect(x + 8.0, *y, w - 16.0, h, 10.0, t::WASH);
        }
        cx.c.text(&words, x + t::PAD, *y + (h - 18.0) / 2.0, Text::new(ty::UI, t::INK2).medium());
        if cx.tap(x, *y, w, h) && visible {
            app.st.agents.all = !app.st.agents.all;
        }
        *y += h + 4.0;
    }
    open
}

/// Live and earlier agents whose name or intent holds the query.
pub fn found(app: &App, query: &str) -> Vec<Agent> {
    let mut out: Vec<Agent> = app.agents.iter().filter(|a| Some(a.project_id) == app.repo && live(a) && matches(a, query)).cloned().collect();
    out.extend(earlier(app).into_iter().filter(|a| matches(a, query)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str, state: &str, intent: Option<&str>) -> Agent {
        crate::data::agents(&json!({"sessions": [{"name": name, "project_id": 1, "provider": "claude", "state": state, "intent": intent}]})).remove(0)
    }

    #[test]
    fn live_and_resumable_split_the_states() {
        for s in ["created", "spawning", "running", "idle", "blocked"] {
            assert!(live(&agent("a-b", s, None)), "{s}");
        }
        for s in ["restorable", "exited", "parked"] {
            assert!(!live(&agent("a-b", s, None)) && resumable(&agent("a-b", s, None)), "{s}");
        }
        assert!(!live(&agent("a-b", "closed", None)) && !resumable(&agent("a-b", "closed", None)));
    }

    #[test]
    fn blocked_asks_you_only_at_a_prompt() {
        let blocked = |waiting: Option<&str>| {
            crate::data::agents(&json!({"sessions": [{"name": "a-b", "project_id": 1, "provider": "claude", "state": "blocked", "waiting": waiting}]})).remove(0)
        };
        assert_eq!(blocked(Some("prompt")).state_words(), "asks you");
        assert_eq!(blocked(Some("report")).state_words(), "stopped · blocked");
        // An older PC says nothing of why: as before.
        assert_eq!(blocked(None).state_words(), "asks you");
        assert!(blocked(Some("report")).needs_you(), "it still waits on you, under Needs you");
    }

    #[test]
    fn search_looks_in_the_name_and_the_intent_ignoring_case() {
        let a = agent("umber-zebra", "running", Some("Fix the Leaf reader"));
        assert!(matches(&a, "ZEBRA"));
        assert!(matches(&a, " leaf "));
        assert!(matches(&a, ""));
        assert!(!matches(&a, "tally"));
        assert!(!matches(&agent("calm-otter", "closed", None), "leaf"));
    }
}
