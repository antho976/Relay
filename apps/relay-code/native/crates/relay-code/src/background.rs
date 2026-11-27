//! What runs for the process, with or without the activity: the one link to the PC, and the
//! notifications that reach you while the app is away.
//!
//! Android keeps the process alive through `LinkService` after the app is swiped away, and can
//! start the service again by itself in a fresh process with no activity at all. So neither the
//! link nor the notifications may live in the activity's loop: the service starts [`start`], which
//! makes the link if no activity has, and runs one thread that hears the link's events beside the
//! app ([`rc_link::Link::listen`]), reads `session.list` when an agent's state changes, and posts
//! through the [`Notifier`] the service gave.
//!
//! What notifies ([`Watch`], tested): an agent asks you (a live prompt), an agent finished its
//! turn (working to waiting at its prompt, or it reported that it stopped), a command is held for
//! you. Never for the agent whose chat is open on the screen, and only what Settings leaves on.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rc_link::{FileStore, Link, LinkState, Note};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::Notify;

/// What a notification lets you do from the shade (D18), besides a tap opening the chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actions {
    /// Open only.
    Open,
    /// Reply (words typed in the shade, sent with `chat.send`) and Open: an agent asks you.
    Reply,
    /// Allow once and Deny for this hold: a command is held for you.
    Hold(i64),
    /// None, and no sound: the notification saying what was done, before it clears.
    Quiet,
}

/// How the platform posts what the loop decides.
pub trait Notifier: Send + Sync {
    /// A notification about `agent`; a tap opens its chat. One per agent: a newer one replaces it.
    fn alert(&self, id: i32, title: &str, body: &str, agent: &str, actions: Actions);
    /// Take a notification away (after what it asked was done).
    fn clear(&self, _id: i32) {}
    /// The background service's ongoing line: where the link stands.
    fn line(&self, words: &str);
}

/// The one link for the process: Android starts the app's loop again for each new activity, and
/// the service may run with none, so whichever comes first makes it.
pub fn link(data: &Path) -> Link {
    static LINK: OnceLock<Link> = OnceLock::new();
    LINK.get_or_init(|| Link::start(Arc::new(FileStore(data.join("pc.json"))), Arc::new(crate::wake::wake))).clone()
}

/// What is on the screen, so an agent you are looking at does not notify.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Front {
    pub foreground: bool,
    /// The agent whose chat is open.
    pub chat: Option<String>,
}

static FRONT: Mutex<Front> = Mutex::new(Front { foreground: false, chat: None });

/// The app says what it shows: each frame, and when it goes to the background.
pub fn front(foreground: bool, chat: Option<String>) {
    let mut f = FRONT.lock().unwrap_or_else(|e| e.into_inner());
    if f.foreground != foreground || f.chat != chat {
        *f = Front { foreground, chat };
    }
}

fn front_now() -> Front {
    FRONT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// An agent that a notification tap asked to open, for the app's loop to take.
static OPENED: Mutex<Option<String>> = Mutex::new(None);

/// A notification about `agent` was tapped (Android's intent extra `agent`).
pub fn opened(agent: String) {
    *OPENED.lock().unwrap_or_else(|e| e.into_inner()) = Some(agent);
    crate::wake::wake();
}

/// The agent a tapped notification names, once.
pub fn take_opened() -> Option<String> {
    OPENED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// One agent as `session.list` gives it: only what the rules read.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Row {
    pub name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub state: String,
    /// Why a blocked agent waits: "prompt" or "report" (engine v37); absent from an older PC.
    #[serde(default)]
    pub waiting: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub last_output_at: Option<String>,
}

impl Row {
    fn title(&self) -> &str {
        self.intent.as_deref().filter(|i| !i.trim().is_empty()).unwrap_or(&self.name)
    }

    /// Waiting at a live prompt. A blocked agent from a PC that does not say why counts as one.
    fn asks(&self) -> bool {
        self.state == "blocked" && self.waiting.as_deref() != Some("report")
    }

    /// It reported blocked or partial: it stopped, and there is nothing to answer.
    fn stopped(&self) -> bool {
        self.state == "blocked" && self.waiting.as_deref() == Some("report")
    }

    fn is_agent(&self) -> bool {
        self.provider != "shell"
    }

    /// Claude Code says when a turn starts and ends (its hooks: running, then idle). Other
    /// providers stay "running" at their prompt, so their terminal going quiet says it instead.
    fn has_hooks(&self) -> bool {
        self.provider == "claude"
    }

    fn working(&self, now_ms: u64) -> bool {
        if self.state != "running" {
            return false;
        }
        self.has_hooks() || self.last_output_at.as_deref().and_then(crate::data::parse_ms).is_none_or(|t| now_ms.saturating_sub(t) < crate::data::BUSY_QUIET_MS)
    }
}

/// A notification to post, if Settings leaves its kind on.
#[derive(Clone, Debug, PartialEq)]
pub struct Tell {
    pub kind: Notify,
    pub agent: String,
    pub title: String,
    pub body: String,
    /// The hold a held command's notification allows or denies.
    pub hold: Option<i64>,
}

impl Tell {
    /// What it lets you do from the shade.
    pub fn actions(&self) -> Actions {
        match (self.kind, self.hold) {
            (Notify::Asks, _) => Actions::Reply,
            (Notify::Holds, Some(h)) => Actions::Hold(h),
            _ => Actions::Open,
        }
    }
}

/// What was done from the shade (Android hands it over: `LinkService.nativeAct`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Reply { agent: String, text: String },
    Allow { agent: String, hold: i64 },
    Deny { agent: String, hold: i64 },
}

impl Act {
    /// From the platform's words: `reply`, `allow` or `deny`; none for anything else or a reply
    /// with nothing in it.
    pub fn read(kind: &str, agent: &str, hold: i64, text: &str) -> Option<Act> {
        let agent = agent.to_string();
        match kind {
            "reply" if !text.trim().is_empty() => Some(Act::Reply { agent, text: text.trim().to_string() }),
            "allow" if hold > 0 => Some(Act::Allow { agent, hold }),
            "deny" if hold > 0 => Some(Act::Deny { agent, hold }),
            _ => None,
        }
    }

    pub fn agent(&self) -> &str {
        match self {
            Act::Reply { agent, .. } | Act::Allow { agent, .. } | Act::Deny { agent, .. } => agent,
        }
    }

    /// The request that does it: a reply is a message (`chat.send`, under an id of its own), a
    /// hold is allowed once or denied.
    pub fn request(&self, now_ms: u64) -> (&'static str, Value) {
        match self {
            Act::Reply { agent, text } => ("chat.send", json!({"session": agent, "text": text, "client_id": format!("shade-{now_ms}")})),
            Act::Allow { hold, .. } => ("guardrail.confirm", json!({"hold_id": hold, "scope": "once"})),
            Act::Deny { hold, .. } => ("guardrail.reject", json!({"hold_id": hold, "reason": "Denied from the phone"})),
        }
    }

    /// What the notification says once the PC answered: its title and line.
    pub fn after(&self, outcome: Result<(), String>) -> (String, String) {
        let agent = self.agent();
        match (self, outcome) {
            (Act::Reply { text, .. }, Ok(())) => (format!("Sent to {agent}"), text.clone()),
            (Act::Allow { .. }, Ok(())) => (format!("Allowed for {agent}"), "The command runs now".into()),
            (Act::Deny { .. }, Ok(())) => (format!("Denied for {agent}"), "The command does not run".into()),
            (Act::Reply { .. }, Err(e)) => (format!("Not sent to {agent}"), format!("{e}. Open the chat to send it again")),
            (_, Err(e)) => (format!("Not done for {agent}"), format!("{e}. Open the chat to answer there")),
        }
    }
}

/// Something was done from the shade: the loop asks the PC, and the notification says how it went.
pub fn act(a: Act) {
    tell_loop(Msg::Act(a));
}

/// How long a "Sent" or "Allowed" stays before it clears.
const DONE_SHOWN: Duration = Duration::from_millis(2_500);

/// What the loop has seen, so each change notifies once.
#[derive(Default)]
pub struct Watch {
    /// A list has been read since the process started.
    known: bool,
    asking: HashSet<String>,
    stopped: HashSet<String>,
    working: HashSet<String>,
    holds: HashSet<i64>,
}

impl Watch {
    /// The agents now: what changed since the last list, for those not on the screen.
    pub fn list(&mut self, rows: &[Row], now_ms: u64, front: &Front) -> Vec<Tell> {
        let first = !self.known;
        self.known = true;
        let mut out = Vec::new();
        let (mut asking, mut stopped, mut working) = (HashSet::new(), HashSet::new(), HashSet::new());
        // A shell is the person's own terminal on the PC: nothing it does is an agent's news.
        for r in rows.iter().filter(|r| r.is_agent()) {
            let name = &r.name;
            let looking = front.foreground && front.chat.as_deref() == Some(name.as_str());
            let tell = |kind: Notify, title: String| Tell { kind, agent: name.clone(), title, body: r.title().to_string(), hold: None };
            if r.asks() {
                asking.insert(name.clone());
                // A question already waiting when the process started is told too, unless the
                // app is open: Android may have restarted the service after it was missed.
                if !self.asking.contains(name) && !looking && !(first && front.foreground) {
                    out.push(tell(Notify::Asks, format!("{name} asks you")));
                }
            } else if r.stopped() {
                stopped.insert(name.clone());
                if !self.stopped.contains(name) && !first && !looking {
                    out.push(tell(Notify::Done, format!("{name} stopped: blocked")));
                }
            } else if r.working(now_ms) {
                working.insert(name.clone());
            } else if self.working.contains(name) && matches!(r.state.as_str(), "idle" | "running") && !looking {
                out.push(tell(Notify::Done, format!("{name} finished")));
            }
        }
        (self.asking, self.stopped, self.working) = (asking, stopped, working);
        out
    }

    /// A command held for you (`guardrail.held`), once per hold.
    pub fn held(&mut self, hold_id: i64, agent: &str, front: &Front) -> Option<Tell> {
        if !self.holds.insert(hold_id) || (front.foreground && front.chat.as_deref() == Some(agent)) {
            return None;
        }
        Some(Tell { kind: Notify::Holds, agent: agent.to_string(), title: format!("{agent} needs your OK"), body: "A command is held until you allow it".into(), hold: Some(hold_id) })
    }

    /// When to read the list again by itself: once a working agent without hooks may have
    /// fallen quiet, and no sooner than [`LOOK_FLOOR_MS`]. Only one at work can still change its
    /// answer: Claude Code says when its turn ends (an event), a quiet one stays quiet until an
    /// event, and a shell is no agent.
    pub fn next_look(rows: &[Row], now_ms: u64) -> Option<u64> {
        rows.iter()
            .filter(|r| r.is_agent() && !r.has_hooks() && r.working(now_ms))
            .filter_map(|r| r.last_output_at.as_deref().and_then(crate::data::parse_ms))
            .map(|t| (t + crate::data::BUSY_QUIET_MS + 500).max(now_ms + LOOK_FLOOR_MS))
            .min()
    }
}

/// The ongoing line for a link state; none once no PC is paired, when the app stops the service
/// (a line posted then would outlive it).
pub fn line(state: &LinkState, pc: &str) -> Option<String> {
    let pc = if pc.is_empty() { "your PC" } else { pc };
    match state {
        LinkState::Online { host, .. } => Some(format!("Connected to {host}")),
        LinkState::Unpaired | LinkState::Revoked { .. } => None,
        _ => Some(format!("Reconnecting to {pc}")),
    }
}

/// The same agent always gets the same notification, so a newer one replaces the older.
pub fn alert_id(agent: &str) -> i32 {
    // Away from 1, the service's own ongoing notification.
    agent.bytes().fold(7i32, |h, b| h.wrapping_mul(31).wrapping_add(b as i32)) | 0x100
}

/// What the loop hears: the link's news, the app's own list, or a reason to look.
enum Msg {
    Link(Note),
    /// The app read `session.list` (it reads it itself while it is in front).
    Listed(Value),
    /// Look now: the app went to the background, or the screen came on.
    Look,
    /// Something was done from the shade.
    Act(Act),
}

static TO_LOOP: Mutex<Option<std::sync::mpsc::Sender<Msg>>> = Mutex::new(None);

fn tell_loop(m: Msg) {
    if let Some(tx) = TO_LOOP.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(m);
    }
}

/// The app's own `session.list` answer: while it is in front the loop reads none itself, so the
/// PC is asked once.
pub fn listed(v: &Value) {
    tell_loop(Msg::Listed(v.clone()));
}

static SCREEN_ON: AtomicBool = AtomicBool::new(true);

/// The screen went on or off (LinkService hears it). Off, nothing is read on a timer.
pub fn screen(on: bool) {
    if SCREEN_ON.swap(on, Ordering::SeqCst) != on && on {
        tell_loop(Msg::Look);
    }
}

/// Start the notifications for the process, once; later calls do nothing.
pub fn start(data: PathBuf, notifier: Arc<dyn Notifier>) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let link = link(&data);
    let heard = link.listen();
    let (tx, rx) = std::sync::mpsc::channel();
    *TO_LOOP.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx.clone());
    // The link's news into the loop's one queue.
    let _ = std::thread::Builder::new().name("rc-alerts-in".into()).spawn(move || {
        while let Ok(n) = heard.recv() {
            if tx.send(Msg::Link(n)).is_err() {
                return;
            }
        }
    });
    let _ = std::thread::Builder::new().name("rc-alerts".into()).spawn(move || run(link, rx, data, notifier));
}

/// How long after an agent's change to read the list: several events come together.
const SETTLE: Duration = Duration::from_millis(300);
const LIST_TIMEOUT: Duration = Duration::from_secs(20);
/// The soonest the list is read again on a timer.
pub const LOOK_FLOOR_MS: u64 = 15_000;

fn run(link: Link, heard: std::sync::mpsc::Receiver<Msg>, data: PathBuf, notifier: Arc<dyn Notifier>) {
    let mut watch = Watch::default();
    let mut due: Option<u64> = link.state().online().then(rc_link::link::now_ms);
    // The next look on a timer, kept apart from one an event asked for.
    let mut timer: Option<u64> = None;
    let pc = || link.profile().map(|p| p.name).unwrap_or_default();
    if let Some(l) = line(&link.state(), &pc()) {
        notifier.line(&l);
    }
    let mut was_front = false;
    loop {
        let now = rc_link::link::now_ms();
        // With the screen off nothing is read on a timer: only an event brings a look.
        let next = if SCREEN_ON.load(Ordering::SeqCst) { due.min(timer).or(due).or(timer) } else { due };
        let msg = match next {
            Some(at) if at <= now => None,
            Some(at) => match heard.recv_timeout(Duration::from_millis(at - now)) {
                Ok(m) => Some(m),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => return,
            },
            None => match heard.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            },
        };
        let soon = || Some(rc_link::link::now_ms() + SETTLE.as_millis() as u64);
        let front = front_now();
        // Leaving the front: the app's reads stop, so the loop takes over with a look of its own.
        if was_front && !front.foreground {
            due = due.min(soon()).or(soon());
        }
        was_front = front.foreground;
        let rows_of = |v: &Value| -> Vec<Row> { v.get("sessions").cloned().and_then(|s| serde_json::from_value(s).ok()).unwrap_or_default() };
        match msg {
            Some(Msg::Look) => due = due.min(soon()).or(soon()),
            // Done on a thread of its own: the PC's answer may take a while, and the loop goes on.
            Some(Msg::Act(a)) => {
                let (link, notifier) = (link.clone(), notifier.clone());
                let _ = std::thread::Builder::new().name("rc-act".into()).spawn(move || {
                    let (op, payload) = a.request(rc_link::link::now_ms());
                    let outcome = link.call_quiet(op, payload, LIST_TIMEOUT).wait(LIST_TIMEOUT).map(|_| ()).map_err(|e| e.message);
                    let ok = outcome.is_ok();
                    let id = alert_id(a.agent());
                    let (title, body) = a.after(outcome);
                    notifier.alert(id, &title, &body, a.agent(), Actions::Quiet);
                    if ok {
                        std::thread::sleep(DONE_SHOWN);
                        notifier.clear(id);
                    }
                });
            }
            Some(Msg::Listed(v)) => {
                let rows = rows_of(&v);
                let now = rc_link::link::now_ms();
                for t in watch.list(&rows, now, &front) {
                    post(&*notifier, &data, &t);
                }
            }
            Some(Msg::Link(Note::State(s))) => {
                if let Some(l) = line(&s, &pc()) {
                    notifier.line(&l);
                }
                if s.online() {
                    due = soon();
                }
            }
            Some(Msg::Link(Note::Event(e))) => match e.ev.as_str() {
                "guardrail.held" => {
                    let agent = e.payload.get("session").and_then(Value::as_str).unwrap_or_default();
                    let hold = e.payload.get("hold_id").and_then(Value::as_i64).unwrap_or_default();
                    if !agent.is_empty()
                        && let Some(t) = watch.held(hold, agent, &front)
                    {
                        post(&*notifier, &data, &t);
                    }
                }
                ev if ev.starts_with("session.") => due = due.min(soon()).or(soon()),
                _ => {}
            },
            None => {
                (due, timer) = (None, None);
                // In front, the app reads the list on the same events and hands it over.
                if front.foreground || !link.state().online() {
                    continue;
                }
                // Quiet: the answer does not wake the activity asleep in the background.
                let Ok(v) = link.call_quiet("session.list", json!({}), LIST_TIMEOUT).wait(LIST_TIMEOUT) else { continue };
                let rows = rows_of(&v);
                let now = rc_link::link::now_ms();
                for t in watch.list(&rows, now, &front) {
                    post(&*notifier, &data, &t);
                }
                timer = Watch::next_look(&rows, now);
            }
        }
    }
}

fn post(notifier: &dyn Notifier, data: &Path, t: &Tell) {
    if crate::app::notifies_in(data, t.kind) {
        notifier.alert(alert_id(&t.agent), &t.title, &t.body, &t.agent, t.actions());
    }
}

/// Rows by name, for tests.
#[cfg(test)]
fn rows(list: &[(&str, &str, &str, Option<&str>)]) -> Vec<Row> {
    list.iter()
        .map(|(name, provider, state, waiting)| Row { name: name.to_string(), provider: provider.to_string(), state: state.to_string(), waiting: waiting.map(str::to_string), ..Default::default() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000_000;
    fn away() -> Front {
        Front::default()
    }
    fn kinds(t: &[Tell]) -> Vec<(Notify, &str, &str)> {
        t.iter().map(|t| (t.kind, t.agent.as_str(), t.title.as_str())).collect()
    }

    #[test]
    fn a_prompt_asks_once_and_a_report_stops_never_asks() {
        let mut w = Watch::default();
        assert!(w.list(&rows(&[("a", "claude", "running", None), ("b", "claude", "running", None)]), NOW, &away()).is_empty());
        let t = w.list(&rows(&[("a", "claude", "blocked", Some("prompt")), ("b", "claude", "blocked", Some("report"))]), NOW, &away());
        assert_eq!(kinds(&t), [(Notify::Asks, "a", "a asks you"), (Notify::Done, "b", "b stopped: blocked")]);
        // Still the same: nothing new.
        assert!(w.list(&rows(&[("a", "claude", "blocked", Some("prompt")), ("b", "claude", "blocked", Some("report"))]), NOW, &away()).is_empty());
        // An older PC that does not say why: a blocked agent asks.
        let mut old = Watch::default();
        old.list(&[], NOW, &away());
        assert_eq!(kinds(&old.list(&rows(&[("c", "claude", "blocked", None)]), NOW, &away())), [(Notify::Asks, "c", "c asks you")]);
    }

    #[test]
    fn a_turn_that_ends_is_told_finished() {
        let mut w = Watch::default();
        w.list(&rows(&[("a", "claude", "running", None)]), NOW, &away());
        assert_eq!(kinds(&w.list(&rows(&[("a", "claude", "idle", None)]), NOW, &away())), [(Notify::Done, "a", "a finished")]);
        // Idle and idle again: once.
        assert!(w.list(&rows(&[("a", "claude", "idle", None)]), NOW, &away()).is_empty());
        // An agent first seen idle did not finish anything you saw start.
        assert!(w.list(&rows(&[("a", "claude", "idle", None), ("z", "claude", "idle", None)]), NOW, &away()).is_empty());
    }

    #[test]
    fn an_agent_without_hooks_finishes_when_its_terminal_goes_quiet() {
        let at = |ms_ago: u64| {
            let t = chrono_like(NOW - ms_ago);
            vec![Row { name: "x".into(), provider: "codex".into(), state: "running".into(), last_output_at: Some(t), ..Default::default() }]
        };
        let mut w = Watch::default();
        assert!(w.list(&at(1_000), NOW, &away()).is_empty(), "writing: working");
        assert_eq!(Watch::next_look(&at(1_000), NOW), Some(NOW + LOOK_FLOOR_MS), "looks again once it may be quiet, not sooner than the floor");
        assert_eq!(Watch::next_look(&at(20_000), NOW), None, "quiet already: nothing changes until an event");
        assert_eq!(kinds(&w.list(&at(20_000), NOW, &away())), [(Notify::Done, "x", "x finished")]);
        // Claude says it itself: no look on a timer.
        assert_eq!(Watch::next_look(&rows(&[("a", "claude", "running", None)]), NOW), None);
    }

    #[test]
    fn a_shell_never_notifies_and_is_never_polled() {
        let shell = |state: &str| vec![Row { name: "sh".into(), provider: "shell".into(), state: state.into(), last_output_at: Some(chrono_like(NOW - 1_000)), ..Default::default() }];
        let mut w = Watch::default();
        assert!(w.list(&shell("running"), NOW, &away()).is_empty());
        assert!(w.list(&shell("running"), NOW + 60_000, &away()).is_empty(), "its terminal went quiet: no agent finished");
        assert!(w.list(&shell("blocked"), NOW, &away()).is_empty());
        assert_eq!(Watch::next_look(&shell("running"), NOW), None);
    }

    #[test]
    fn the_chat_on_the_screen_does_not_notify_and_others_do() {
        let mut w = Watch::default();
        let looking = Front { foreground: true, chat: Some("a".into()) };
        w.list(&rows(&[("a", "claude", "running", None), ("b", "claude", "running", None)]), NOW, &looking);
        let t = w.list(&rows(&[("a", "claude", "blocked", Some("prompt")), ("b", "claude", "blocked", Some("prompt"))]), NOW, &looking);
        assert_eq!(kinds(&t), [(Notify::Asks, "b", "b asks you")]);
        assert_eq!(w.held(7, "a", &looking), None, "held for the chat on the screen");
        assert_eq!(w.held(8, "b", &looking).map(|t| t.title), Some("b needs your OK".into()));
        assert_eq!(w.held(8, "b", &looking), None, "once per hold");
    }

    #[test]
    fn what_waited_before_the_process_started_asks_only_while_away() {
        let blocked = rows(&[("a", "claude", "blocked", Some("prompt")), ("b", "claude", "blocked", Some("report"))]);
        // The service came back by itself, no activity: a question may have been missed.
        assert_eq!(kinds(&Watch::default().list(&blocked, NOW, &away())), [(Notify::Asks, "a", "a asks you")]);
        // The app is open: its lists show it already.
        assert!(Watch::default().list(&blocked, NOW, &Front { foreground: true, chat: None }).is_empty());
    }

    #[test]
    fn rows_read_from_session_list_and_ids_stay_off_the_service() {
        let v = json!([{"name": "a", "provider": "claude", "state": "blocked", "waiting": "report", "intent": "Fix it", "project_id": 1}]);
        let r: Vec<Row> = serde_json::from_value(v).unwrap();
        assert!(r[0].stopped() && !r[0].asks());
        assert_eq!(r[0].title(), "Fix it");
        assert_ne!(alert_id("a"), 1);
        assert_eq!(line(&LinkState::Connecting { attempt: 1 }, "").as_deref(), Some("Reconnecting to your PC"));
        assert_eq!(line(&LinkState::Unpaired, "desk"), None, "the service is stopping");
    }

    /// An RFC 3339 time for `ms`, as the engine writes it.
    fn chrono_like(ms: u64) -> String {
        let secs = ms / 1000;
        let (days, rest) = (secs / 86_400, secs % 86_400);
        let (y, m, d) = civil(days as i64);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rest / 3600, rest % 3600 / 60, rest % 60, ms % 1000)
    }

    fn civil(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
    }

    #[test]
    fn asks_reply_holds_allow_or_deny_and_the_rest_only_open() {
        let tell = |kind, hold| Tell { kind, agent: "a-b".into(), title: String::new(), body: String::new(), hold };
        assert_eq!(tell(Notify::Asks, None).actions(), Actions::Reply);
        assert_eq!(tell(Notify::Holds, Some(7)).actions(), Actions::Hold(7));
        assert_eq!(tell(Notify::Done, None).actions(), Actions::Open);
        // A hold the PC gave no id cannot be answered from the shade.
        assert_eq!(tell(Notify::Holds, None).actions(), Actions::Open);
    }

    #[test]
    fn an_act_from_the_shade_reads_strictly_and_asks_the_right_op() {
        assert_eq!(Act::read("reply", "a-b", 0, "  yes, push it "), Some(Act::Reply { agent: "a-b".into(), text: "yes, push it".into() }));
        assert_eq!(Act::read("reply", "a-b", 0, "   "), None, "an empty reply sends nothing");
        assert_eq!(Act::read("allow", "a-b", 0, ""), None, "no hold, nothing to allow");
        assert_eq!(Act::read("shrug", "a-b", 7, "x"), None);
        let (op, p) = Act::read("reply", "a-b", 0, "go").unwrap().request(1_700_000_000_000);
        assert_eq!((op, p["session"].as_str(), p["text"].as_str(), p["client_id"].as_str()), ("chat.send", Some("a-b"), Some("go"), Some("shade-1700000000000")));
        let (op, p) = Act::read("allow", "a-b", 7, "").unwrap().request(0);
        assert_eq!((op, p["hold_id"].as_i64(), p["scope"].as_str()), ("guardrail.confirm", Some(7), Some("once")));
        let (op, p) = Act::read("deny", "a-b", 7, "").unwrap().request(0);
        assert_eq!((op, p["hold_id"].as_i64()), ("guardrail.reject", Some(7)));
    }

    #[test]
    fn the_notification_says_how_it_went() {
        let reply = Act::Reply { agent: "a-b".into(), text: "go".into() };
        assert_eq!(reply.after(Ok(())), ("Sent to a-b".to_string(), "go".to_string()));
        assert_eq!(reply.after(Err("The PC did not answer in time".into())).0, "Not sent to a-b");
        let allow = Act::Allow { agent: "a-b".into(), hold: 7 };
        assert_eq!(allow.after(Ok(())).0, "Allowed for a-b");
        assert!(allow.after(Err("gone".into())).1.contains("Open the chat"));
    }
}
