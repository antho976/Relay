//! The whole app: the link, what the PC last said, the pages open, input, one frame at a time.
//!
//! Screens are immediate (as Leaf and Tally RS): each draws itself and acts on taps as it draws,
//! returning true while something on it still moves. Nothing animates at rest: after an input the
//! app draws until scrolling settles, then sleeps until the next input or a wake from the link.
//!
//! Getting around is route A of design 44 (D4): one drawer, like Claude's app. The app opens on
//! the conversation you last had open (D3), drawn from the phone's own copy before the PC answers.
//! There is no Connect key anywhere: once paired, the link connects by itself (`rc_link::Link`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rc_link::{Call, FileStore, Link, LinkState};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{self, Agent, Convo, Project, Sending};
use crate::draw::{Canvas, WIDTH};
use crate::kit::{self, Focus, Kb};
use crate::ui::{Scroll, Touch, Ui};

/// What the platform does for the app.
pub trait Host: Send + Sync {
    /// Where the pairing, the settings and the copies of what the PC said live.
    fn data_dir(&self) -> PathBuf;
    fn log(&self, text: &str);
    /// Show the soft keyboard for a field holding `text`, or hide it.
    fn keyboard(&self, _show: bool, _text: &str, _kind: Kb) {}
    fn copy_text(&self, _text: &str) {}
    /// Tell the keyboard where the caret is (characters): after a tap in the field.
    fn caret(&self, _text: &str, _start: usize, _end: usize) {}
    /// Open a web address in the phone's browser (or the app that handles it).
    fn open_url(&self, _url: &str) {}
    /// Ask to post notifications (Android 13 and later).
    fn ask_notifications(&self) {}
    /// Keep the app alive in the background while a PC is paired (Android's `LinkService`), its
    /// ongoing notification saying `words`; off when none is.
    fn link_service(&self, _on: bool, _words: &str) {}
    /// A notification: an agent asks you something while the app is in the background.
    fn notify(&self, _id: i32, _title: &str, _body: &str) {}
    /// Ask the person for a photo or screenshot; it comes back through [`crate::attach::picked`].
    fn pick_photo(&self) {}
}

/// When the next frame is wanted.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Next {
    Now,
    OnInput,
    /// Sleep until input or this time on [`crate::clock::now_ns`], whichever comes first.
    At(u64),
}

pub const FONTS: [&[u8]; crate::draw::FACES] = [
    include_bytes!("../assets/fonts/geist_400.ttf"),
    include_bytes!("../assets/fonts/geist_500.ttf"),
    include_bytes!("../assets/fonts/geist_600.ttf"),
    include_bytes!("../assets/fonts/sora_600.ttf"),
    // Literata (OFL, assets/fonts/Literata-OFL.txt): the agent's words, as Claude's app sets them in a serif.
    include_bytes!("../assets/fonts/literata_400.ttf"),
    include_bytes!("../assets/fonts/geist_mono_400.ttf"),
    include_bytes!("../assets/fonts/literata_600.ttf"),
];

/// The most a field holds, in characters.
const MAX_TYPED: usize = 8000;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// No PC yet (or it forgot this phone): pair one.
    Pair,
    /// The repo's agents, when no conversation is open.
    Home,
    /// One agent's conversation.
    Chat(String),
    Settings,
    /// What changed in the repo (per agent's branch): diff, commit, push, pull request (E5).
    Changes,
    /// The repo's designs (E1), one design on its artboards (E3), one artboard full screen (E4).
    Designs,
    Design(i64),
    /// The repo's board, and one task.
    Board,
    Task(i64),
    /// What needs you: held commands, agents that ask, finished work.
    Inbox,
}

/// What a request was for, so its answer lands in the right place.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Projects,
    Agents,
    Transcript(String),
    /// Words typed into an agent's terminal (a message's words, from its send queue).
    Input(String),
    /// Enter after them, once the PC said it typed them.
    Enter(String),
    Create,
    Spawn(String),
    /// The rendered foot of an agent's terminal (`session.screen`).
    Screen(String),
}

#[derive(Default, Serialize, Deserialize)]
struct Config {
    repo: Option<i64>,
    last_chat: Option<String>,
    #[serde(default)]
    asked_notifications: bool,
}

#[derive(Default)]
pub struct States {
    pub pair: crate::pair::State,
    pub chat: crate::chat::State,
    pub drawer: crate::drawer::State,
    pub newagent: crate::drawer::NewAgent,
    pub settings: crate::settings::State,
    pub changes: crate::changes::State,
    pub board: crate::board::State,
    pub inbox: crate::inbox::State,
    pub designs: crate::designs::State,
}

pub struct App {
    pub canvas: Canvas,
    pub ui: Ui,
    pub host: Arc<dyn Host>,
    pub focus: Focus,
    /// The pages open, the current one last, each with its scroll.
    pub nav: Vec<(Route, Scroll)>,
    /// True on the first frame a page shows (opened, or uncovered by Back).
    pub fresh: bool,
    pub link: Link,
    pub link_state: LinkState,
    pub projects: Vec<Project>,
    pub agents: Vec<Agent>,
    /// The repo every list shows (design 44, D5): one switch, remembered.
    pub repo: Option<i64>,
    pub convos: HashMap<String, Convo>,
    pub sending: HashMap<String, Sending>,
    /// What each agent's terminal shows at its foot (status, mode, question), and when it was read.
    pub screens: HashMap<String, crate::screen::Screen>,
    /// The effort last sent to each agent from the model picker (`/effort`), to tick it there.
    pub efforts: HashMap<String, &'static str>,
    /// When the phone first saw each agent busy this turn (ms), for the live line's clock.
    pub busy_since: HashMap<String, u64>,
    /// When the agent list was last asked for while a chat is open (ns).
    agents_at: u64,
    /// When the agent list is next read while the app is in front (ns).
    list_at: u64,
    /// The PC's Relay is older than `session.screen`: the chat makes do without the terminal's foot.
    pub no_screen: bool,
    /// Held commands waiting on you (the inbox counts them with agents that ask).
    pub holds: usize,
    screen_at: u64,
    pub st: States,
    calls: Vec<(Ask, Call)>,
    /// The agents' list is read again once this passes (an event said it changed).
    agents_stale: bool,
    /// The open conversation is read again at this time.
    pub reread_at: u64,
    /// Kinds of thing the PC said changed since a page last looked (`git`, `design`, `task`,
    /// `guardrail`, `notify`, ...): the first part of each event's name. A page takes its own with
    /// [`App::take_changed`] and reads again.
    changed: std::collections::HashSet<String>,
    /// Agents that asked you something, already told about.
    told: std::collections::HashSet<String>,
    pub foreground: bool,
    pub data: PathBuf,
    config: Config,
    toast: Option<(String, u64)>,
    pub typed_done: Option<&'static str>,
    pub keyboard_up: bool,
    /// Screenshots: nothing is sent to the PC.
    pub shot: bool,
    size: (f32, f32),
    insets: (f32, f32),
    last_ns: u64,
    pub now_ns: u64,
    wake: Option<u64>,
}

fn read_json<T: for<'a> Deserialize<'a>>(p: PathBuf) -> Option<T> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Write a copy off the UI thread.
fn write_json<T: Serialize>(p: PathBuf, v: &T) {
    let Ok(bytes) = serde_json::to_vec(v) else { return };
    let _ = std::thread::Builder::new().name("rc-save".into()).spawn(move || {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    });
}

/// [`write_json`] for a value worth not serializing on the UI thread (a whole conversation).
fn write_owned<T: Serialize + Send + 'static>(p: PathBuf, v: T) {
    let _ = std::thread::Builder::new().name("rc-save".into()).spawn(move || {
        let Ok(bytes) = serde_json::to_vec(&v) else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    });
}

fn chat_file(data: &std::path::Path, name: &str) -> PathBuf {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    data.join("cache").join(format!("chat-{safe}.json"))
}

impl App {
    pub fn new(host: Arc<dyn Host>, now_ns: u64) -> App {
        let data = host.data_dir();
        let _ = std::fs::create_dir_all(&data);
        let config: Config = read_json(data.join("config.json")).unwrap_or_default();
        let projects: Vec<Project> = read_json(data.join("cache/projects.json")).unwrap_or_default();
        let agents: Vec<Agent> = read_json(data.join("cache/agents.json")).unwrap_or_default();
        // One link for the process: Android starts the app's loop again for each new activity
        // while LinkService keeps the process (and the link) alive between them.
        static LINK: std::sync::OnceLock<Link> = std::sync::OnceLock::new();
        let link = LINK.get_or_init(|| Link::start(Arc::new(FileStore(data.join("pc.json"))), Arc::new(crate::wake::wake))).clone();
        let link_state = link.state();
        let mut convos = HashMap::new();
        // The last conversation, from the phone's copy: drawn on the first frame (D3).
        if let Some(name) = &config.last_chat
            && let Some(c) = read_json::<Convo>(chat_file(&data, name))
        {
            convos.insert(name.clone(), c);
        }
        let mut nav = vec![(Route::Home, Scroll::default())];
        if link_state == LinkState::Unpaired {
            nav = vec![(Route::Pair, Scroll::default())];
        } else if let Some(name) = config.last_chat.clone().filter(|n| agents.iter().any(|a| &a.name == n && a.alive())) {
            nav.push((Route::Chat(name), Scroll::default()));
        }
        let repo = config.repo.or_else(|| projects.first().map(|p| p.id));
        let app = App {
            canvas: Canvas::new(FONTS),
            ui: Ui::default(),
            host,
            focus: Focus::default(),
            nav,
            fresh: true,
            link,
            link_state,
            projects,
            agents,
            repo,
            convos,
            sending: HashMap::new(),
            screens: HashMap::new(),
            efforts: HashMap::new(),
            busy_since: HashMap::new(),
            agents_at: 0,
            list_at: 0,
            no_screen: false,
            holds: 0,
            screen_at: 0,
            st: States::default(),
            calls: Vec::new(),
            agents_stale: true,
            reread_at: 0,
            told: Default::default(),
            changed: Default::default(),
            foreground: true,
            data,
            config,
            toast: None,
            typed_done: None,
            keyboard_up: false,
            shot: false,
            size: (1080.0, 2340.0),
            insets: (0.0, 0.0),
            last_ns: now_ns,
            now_ns,
            wake: None,
        };
        app.service();
        app
    }

    /// The background service follows the pairing: on while a PC is paired, saying where the link stands.
    fn service(&self) {
        if self.shot {
            return;
        }
        let pc = self.link.profile().map(|p| p.name).unwrap_or_default();
        match &self.link_state {
            LinkState::Unpaired | LinkState::Revoked { .. } => self.host.link_service(false, ""),
            LinkState::Online { host, .. } => self.host.link_service(true, &format!("Connected to {host}")),
            _ => self.host.link_service(true, &format!("Reconnecting to {}", if pc.is_empty() { "your PC" } else { &pc })),
        }
    }

    pub fn toast(&mut self, words: &str) {
        self.toast = Some((words.to_string(), self.now_ns + 2_500_000_000));
    }

    pub fn save_config(&self) {
        write_json(self.data.join("config.json"), &self.config);
    }

    // Asking the PC

    /// Ask the PC, once per kind of question at a time.
    pub fn ask(&mut self, ask: Ask, op: &str, payload: Value) {
        if self.shot || !self.link_state.online() {
            return;
        }
        if self.calls.iter().any(|(a, _)| *a == ask) {
            return;
        }
        let call = self.link.call(op, payload);
        self.calls.push((ask, call));
    }

    pub fn asking(&self, ask: &Ask) -> bool {
        self.calls.iter().any(|(a, _)| a == ask)
    }

    fn refresh(&mut self) {
        self.ask(Ask::Projects, "project.list", json!({}));
        self.ask(Ask::Agents, "session.list", json!({}));
        if let Some(name) = self.open_chat() {
            self.read_chat(&name);
        }
    }

    /// Whether the PC said something of `domain` changed (or the link came back) since this was
    /// last asked; asking clears it. Pages call it each frame and read again when true.
    pub fn take_changed(&mut self, domain: &str) -> bool {
        let hit = self.changed.remove(domain);
        hit || self.changed.contains("*")
    }

    /// Whether `name` is at work right now, as its terminal shows it. The terminal's own status
    /// line says so when the PC can send it (`session.screen`); otherwise the session must be
    /// running and have written to its terminal in the last few seconds: Claude Code redraws its
    /// spinner every second while it works and nothing while it waits at the prompt. Relay's
    /// "running" alone also covers an agent waiting at its prompt.
    pub fn busy(&self, name: &str) -> bool {
        let Some(a) = self.agents.iter().find(|a| a.name == name) else { return false };
        if a.needs_you() {
            return false;
        }
        if !self.no_screen
            && let Some(s) = self.screens.get(name)
        {
            return s.status.is_some();
        }
        a.working()
    }

    /// The link is up: pages may ask the PC.
    pub fn online(&self) -> bool {
        self.link_state.online() && !self.shot
    }

    /// The branch of the agent whose chat was open last: Changes starts on it.
    pub fn open_chat_agent_branch(&self) -> Option<String> {
        let name = self.open_chat().or_else(|| self.config.last_chat.clone())?;
        self.agents.iter().find(|a| a.name == name).map(|a| a.branch.clone())
    }

    pub fn open_chat(&self) -> Option<String> {
        match self.route() {
            Route::Chat(n) => Some(n.clone()),
            _ => None,
        }
    }

    /// Read what the agent wrote since the last read.
    pub fn read_chat(&mut self, name: &str) {
        let after = self.convos.get(name).and_then(|c| c.cursor);
        let mut p = json!({"session": name, "limit": 200});
        if let Some(a) = after {
            p["after"] = json!(a);
        }
        self.ask(Ask::Transcript(name.to_string()), "session.transcript", p);
    }

    /// Send a message to an agent: it joins the agent's send queue ([`data::Sending`]), which types
    /// the words and then Enter into its terminal, as the old app and the PC's own composer do,
    /// now or once the link is back. Several lines go as one paste.
    pub fn say(&mut self, name: &str, text: &str) {
        let text = text.trim_end();
        if text.is_empty() {
            return;
        }
        let now = rc_link::link::now_ms();
        self.sending.entry(name.to_string()).or_default().push(text, now);
        self.send_queued();
        self.reread_at = self.now_ns + 600_000_000;
    }

    /// Send what each agent's queue holds next, while the link is up. Enter goes only after the
    /// PC answered that it typed the words (and [`data::ENTER_AFTER_MS`] after them), never from a
    /// timer of its own, so it cannot land alone or ahead of the words.
    fn send_queued(&mut self) {
        if self.shot || !self.link_state.online() {
            return;
        }
        let now = rc_link::link::now_ms();
        let names: Vec<String> = self.sending.keys().cloned().collect();
        for name in names {
            if self.asking(&Ask::Input(name.clone())) || self.asking(&Ask::Enter(name.clone())) {
                continue;
            }
            let Some(step) = self.sending.get_mut(&name).and_then(|s| s.next(now)) else { continue };
            match step {
                data::Step::Type(text) => {
                    let mut p = json!({"session": name, "data": text});
                    if text.contains('\n') {
                        p["paste"] = json!(true);
                    }
                    let call = self.link.call("session.input", p);
                    self.calls.push((Ask::Input(name), call));
                }
                data::Step::Enter => {
                    let call = self.link.call("session.input", json!({"session": name, "data": "\r"}));
                    self.calls.push((Ask::Enter(name), call));
                }
                data::Step::Wait(ms) => self.wake_at(self.now_ns + ms * 1_000_000),
            }
        }
    }

    /// Raw keys (Esc, Shift-Tab, a menu's number), unanswered. The screen is read again soon
    /// after, so a mode or a question that changed shows at once.
    pub fn key(&mut self, name: &str, data: &str) {
        if !self.shot {
            self.link.post("session.input", json!({"session": name, "data": data}));
        }
        self.screen_at = self.now_ns + 250_000_000;
        self.reread_at = self.now_ns + 400_000_000;
    }

    fn take_answers(&mut self) {
        // The link's state: online reads everything again; the background service follows it.
        let state = if self.shot { self.link_state.clone() } else { self.link.state() };
        if state != self.link_state {
            let was_online = self.link_state.online();
            self.link_state = state.clone();
            self.service();
            match &state {
                LinkState::Online { .. } if !was_online => {
                    // Back online: every page reads again.
                    self.changed.insert("*".into());
                    self.refresh();
                }
                LinkState::Unpaired | LinkState::Revoked { .. } if self.route() != &Route::Pair => {
                    self.nav = vec![(Route::Pair, Scroll::default())];
                    self.shown();
                }
                _ => {}
            }
        }
        for e in self.link.take_events() {
            if let Some((domain, _)) = e.ev.split_once('.') {
                self.changed.insert(domain.to_string());
            }
            if e.ev.starts_with("session.") {
                self.agents_stale = true;
                // The open agent started or stopped: read its words and its foot now, not at the
                // next poll, so the phone keeps up with the PC.
                let open = self.open_chat();
                if open.is_some() && e.payload.get("name").and_then(Value::as_str) == open.as_deref() {
                    self.reread_at = 0;
                    self.screen_at = 0;
                }
            }
            if e.ev.starts_with("project.") {
                self.ask(Ask::Projects, "project.list", json!({}));
            }
        }
        if self.agents_stale && self.link_state.online() && !self.asking(&Ask::Agents) {
            self.agents_stale = false;
            self.ask(Ask::Agents, "session.list", json!({}));
        }
        let mut done = Vec::new();
        self.calls.retain(|(ask, call)| match call.poll() {
            Some(r) => {
                done.push((ask.clone(), r));
                false
            }
            None => true,
        });
        for (ask, r) in done {
            match (ask, r) {
                (Ask::Projects, Ok(v)) => {
                    self.projects = data::projects(&v);
                    if self.repo.is_none_or(|r| !self.projects.iter().any(|p| p.id == r)) {
                        self.repo = self.projects.first().map(|p| p.id);
                    }
                    write_json(self.data.join("cache/projects.json"), &self.projects);
                }
                (Ask::Agents, Ok(v)) => {
                    self.agents = data::agents(&v);
                    self.tell_new_questions();
                    write_json(self.data.join("cache/agents.json"), &self.agents);
                }
                (Ask::Transcript(name), Ok(v)) => {
                    let c = self.convos.entry(name.clone()).or_default();
                    let before = (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()), c.cursor);
                    let seen = c.take(&v);
                    let changed = before != (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()), c.cursor);
                    if let Some(s) = self.sending.get_mut(&name) {
                        s.seen(seen);
                    }
                    // Kept for the next start only when something came in; written (and turned
                    // into JSON) off the UI thread.
                    if changed {
                        write_owned(chat_file(&self.data, &name), self.convos[&name].clone());
                    }
                }
                (Ask::Input(name) | Ask::Enter(name), r) => {
                    let now = rc_link::link::now_ms();
                    if let Some(s) = self.sending.get_mut(&name) {
                        s.answered(r.is_ok(), now);
                    }
                    if let Err(e) = r {
                        self.toast(&format!("Not sent: {}", e.message));
                    }
                }
                (Ask::Create, Ok(v)) => {
                    let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    if name.is_empty() {
                        self.toast("The PC made no agent");
                    } else {
                        self.ask(Ask::Spawn(name.clone()), "session.spawn", json!({"session": name}));
                    }
                }
                (Ask::Spawn(name), Ok(_)) => {
                    self.agents_stale = true;
                    self.open(&name);
                }
                (Ask::Screen(name), Ok(v)) => {
                    let text = v.get("text").and_then(Value::as_str).unwrap_or_default();
                    self.screens.insert(name, crate::screen::read(text));
                }
                (Ask::Screen(_), Err(e)) => {
                    if e.code == "remote.op" || e.code.contains("unknown") {
                        self.no_screen = true;
                    }
                }
                (Ask::Create | Ask::Spawn(_), Err(e)) => self.toast(&e.message),
                (_, Err(e)) => self.host.log(&format!("{}: {}", e.code, e.message)),
            }
        }
        // What waited for the link, or for the write before it, goes now.
        self.send_queued();
        // The lists say who works and who waits by how fresh each agent's output is: read them
        // again every five seconds while the app is in front.
        if self.link_state.online() && self.foreground && self.now_ns >= self.list_at {
            self.agents_stale = true;
            self.list_at = self.now_ns + 5_000_000_000;
        }
        if self.link_state.online() && self.foreground {
            self.wake_at(self.list_at);
        }
        // The open conversation is read again: once a second while its agent works, every four
        // seconds otherwise.
        if let Some(name) = self.open_chat()
            && self.link_state.online()
            && self.foreground
        {
            let busy = self.busy(&name) || self.agents.iter().any(|a| a.name == name && a.needs_you()) || self.sending.get(&name).is_some_and(|s| !s.waiting.is_empty());
            if self.now_ns >= self.reread_at && !self.asking(&Ask::Transcript(name.clone())) {
                self.read_chat(&name);
                // Close behind the PC while it works; slower once it waits.
                self.reread_at = self.now_ns + if busy { 700_000_000 } else { 4_000_000_000 };
            }
            // The terminal's foot: the live status, the mode, a question.
            if !self.no_screen && self.now_ns >= self.screen_at && !self.asking(&Ask::Screen(name.clone())) {
                self.ask(Ask::Screen(name.clone()), "session.screen", json!({"session": name, "rows": 16}));
                self.screen_at = self.now_ns + if busy { 700_000_000 } else { 3_000_000_000 };
            }
            // Without the terminal's foot, how fresh its output is tells working from waiting:
            // the agent list (which carries it) is read every two seconds while a chat is open.
            if self.no_screen && self.now_ns >= self.agents_at {
                self.agents_stale = true;
                self.agents_at = self.now_ns + 2_000_000_000;
            }
            // When it started working, for the live line's clock; forgotten once it stops.
            let now_ms = rc_link::link::now_ms();
            if self.busy(&name) {
                self.busy_since.entry(name.clone()).or_insert(now_ms);
            } else {
                self.busy_since.remove(&name);
            }
            let mut at = self.reread_at.min(if self.no_screen { self.agents_at } else { self.screen_at });
            // A busy agent goes quiet without an event: look again when its output would be stale.
            if let Some(t) = self.agents.iter().find(|a| a.name == name).and_then(|a| a.last_output_at.as_deref()).and_then(crate::data::parse_ms) {
                let stale = (t + crate::data::BUSY_QUIET_MS).saturating_sub(now_ms);
                if stale > 0 && stale < 60_000 {
                    at = at.min(self.now_ns + stale * 1_000_000 + 50_000_000);
                }
            }
            self.wake_at(at);
        }
    }

    /// Agents that just started asking you something get a notification while the app is away.
    fn tell_new_questions(&mut self) {
        let asking: std::collections::HashSet<String> = self.agents.iter().filter(|a| a.needs_you()).map(|a| a.name.clone()).collect();
        for a in self.agents.iter().filter(|a| a.needs_you() && !self.told.contains(&a.name)) {
            if !self.foreground && !self.shot {
                let id = a.name.bytes().fold(7i32, |h, b| h.wrapping_mul(31).wrapping_add(b as i32));
                self.host.notify(id, &format!("{} asks you", a.name), a.title());
            }
        }
        self.told = asking;
    }

    // Repos

    pub fn repo_name(&self) -> String {
        self.repo.and_then(|r| self.projects.iter().find(|p| p.id == r)).map_or_else(|| "No repo".to_string(), |p| p.name.clone())
    }

    /// Switch repo and stay on the same kind of page (D5).
    pub fn set_repo(&mut self, id: i64) {
        self.repo = Some(id);
        self.config.repo = Some(id);
        self.save_config();
        if let Some(name) = self.open_chat()
            && self.agents.iter().find(|a| a.name == name).is_some_and(|a| a.project_id != id)
        {
            self.nav.truncate(1);
            self.shown();
        }
    }

    pub fn repo_agents(&self) -> Vec<&Agent> {
        self.agents.iter().filter(|a| Some(a.project_id) == self.repo && a.alive()).collect()
    }

    /// Open an agent's conversation over Home.
    pub fn open(&mut self, name: &str) {
        if !self.convos.contains_key(name)
            && let Some(c) = read_json::<Convo>(chat_file(&self.data, name))
        {
            self.convos.insert(name.to_string(), c);
        }
        if let Some(a) = self.agents.iter().find(|a| a.name == name)
            && Some(a.project_id) != self.repo
        {
            self.repo = Some(a.project_id);
            self.config.repo = self.repo;
        }
        self.nav.truncate(1);
        if self.nav[0].0 != Route::Home {
            self.nav = vec![(Route::Home, Scroll::default())];
        }
        self.nav.push((Route::Chat(name.to_string()), Scroll::default()));
        self.config.last_chat = Some(name.to_string());
        self.save_config();
        self.reread_at = 0;
        self.screen_at = 0;
        self.st.chat = Default::default();
        self.shown();
    }

    /// A PC was just paired: keep it, connect now, and go to its agents.
    pub fn paired(&mut self, p: &rc_link::Profile) {
        self.link.paired(p);
        self.link_state = self.link.state();
        self.service();
        if !self.config.asked_notifications {
            self.config.asked_notifications = true;
            self.save_config();
            self.host.ask_notifications();
        }
        self.reset_to(Route::Home);
    }

    pub fn unpair(&mut self) {
        self.link.forget();
        self.link_state = LinkState::Unpaired;
        self.service();
        self.agents.clear();
        self.projects.clear();
        self.convos.clear();
        self.config = Config::default();
        self.save_config();
        let _ = std::fs::remove_dir_all(self.data.join("cache"));
        self.reset_to(Route::Pair);
    }

    // The platform

    /// The app came to the front: check the link at once, read the open page again.
    pub fn resumed(&mut self) {
        self.foreground = true;
        self.fresh = true;
        self.link.kick();
        self.agents_stale = true;
        self.reread_at = 0;
    }

    pub fn paused(&mut self) {
        self.foreground = false;
    }

    /// In the background: take what the link brought (an agent that now asks you is a
    /// notification) without drawing.
    pub fn tick(&mut self, now_ns: u64) {
        self.now_ns = now_ns;
        self.take_answers();
    }

    pub fn tabbed(&self) -> bool {
        false
    }

    pub fn route(&self) -> &Route {
        &self.nav.last().expect("a page is always open").0
    }

    pub fn scroll(&mut self) -> &mut Scroll {
        &mut self.nav.last_mut().expect("a page is always open").1
    }

    /// The window changed size. Insets are the system bars (and the keyboard), in pixels.
    pub fn resize(&mut self, w: f32, h: f32, inset_top: f32, inset_bottom: f32) {
        self.size = (w, h);
        self.insets = (inset_top, inset_bottom);
    }

    pub fn touch(&mut self, t: Touch) {
        let u = self.canvas.u.max(0.01);
        let x0 = ((self.size.0 - WIDTH * u) * 0.5).max(0.0);
        self.ui.touch(t, (t.x - x0) / u, t.y / u);
    }

    /// The mouse wheel (desktop), in pixels down.
    pub fn wheel(&mut self, dy_px: f32) {
        let units = dy_px / self.canvas.u.max(0.01);
        let s = self.scroll();
        s.off = (s.off + units).clamp(0.0, s.max.max(0.0));
    }

    fn shown(&mut self) {
        self.fresh = true;
        self.ui.reset();
        if self.focus.id.take().is_some() {
            self.host.keyboard(false, "", Kb::Text);
        }
    }

    pub fn push(&mut self, route: Route) {
        if self.route() == &route {
            return;
        }
        self.nav.push((route, Scroll::default()));
        self.shown();
    }

    pub fn reset_to(&mut self, route: Route) {
        self.nav = vec![(route, Scroll::default())];
        self.st.drawer.open = false;
        self.shown();
    }

    pub fn pop(&mut self) -> bool {
        if self.nav.len() <= 1 {
            return false;
        }
        self.nav.pop();
        self.shown();
        true
    }

    /// Back: typing stops first, then a sheet or the drawer closes, then the page.
    pub fn back(&mut self) -> bool {
        if self.focus.id.is_some() {
            self.finish_typing();
            return true;
        }
        if self.st.drawer.close_over()
            || self.st.chat.close_over()
            || self.st.newagent.open
            || matches!(self.route(), Route::Design(_)) && self.st.designs.close_over()
        {
            self.st.newagent.open = false;
            self.ui.reset();
            return true;
        }
        self.pop()
    }

    // Typing

    /// Text the keyboard sent: the field's whole text (Android's input state).
    pub fn set_text(&mut self, text: &str) {
        self.set_text_sel(text, None);
    }

    /// The field's text and where the keyboard has its selection (characters), when it says.
    pub fn set_text_sel(&mut self, text: &str, sel: Option<(usize, usize)>) {
        if self.focus.id.is_some() {
            self.focus.text = text.chars().take(MAX_TYPED).collect();
            self.focus.sel = sel;
        }
    }

    pub fn finish_typing(&mut self) {
        if let Some(id) = self.focus.id.take() {
            self.focus.at = None;
            self.typed_done = Some(id);
            self.host.keyboard(false, "", Kb::Text);
        }
    }

    pub fn done_typing(&mut self) {
        self.finish_typing();
    }

    /// Characters typed on a hardware keyboard (the desktop).
    pub fn type_text(&mut self, s: &str) {
        if self.focus.id.is_some() && self.focus.text.chars().count() < MAX_TYPED {
            self.focus.text.push_str(s);
            self.focus.sel = None;
        }
    }

    pub fn backspace(&mut self) {
        if self.focus.id.is_some() {
            self.focus.text.pop();
            self.focus.sel = None;
        }
    }

    /// Draw again at `at` even with no input.
    pub fn wake_at(&mut self, at: u64) {
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
    }

    pub fn clear_colour(&self) -> [f32; 3] {
        crate::theme::BG.rgb_f32()
    }

    // Pages

    /// Start drawing a page's scrolling content under a fixed top of `top_h` units, above a fixed
    /// foot of `foot_h`: moves it with the finger and returns where its content starts and ends.
    pub fn page_begin(&mut self, dt: f32, top_h: f32, foot_h: f32) -> Page {
        let bottom = self.canvas.h - self.canvas.inset_bottom - foot_h;
        let top = self.canvas.inset_top + top_h;
        let viewport = [0.0, top, WIDTH, bottom - top];
        let ui = &self.ui;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        let moving = if ui.was_covered() { false } else { sc.update(ui, viewport, dt) };
        self.canvas.dy = -sc.off;
        Page { top, bottom, moving }
    }

    /// Finish the page: `content_bottom` is where its last thing ends (content units).
    pub fn page_end(&mut self, page: &Page, content_bottom: f32) {
        self.canvas.dy = 0.0;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        sc.max = content_bottom + 16.0 - page.bottom;
    }

    /// The ground behind a fixed top, so what scrolls under it never shows through.
    pub fn top_band(&mut self, h: f32) {
        let bottom = self.canvas.inset_top + h;
        self.canvas.dy = 0.0;
        self.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, bottom, 0.0, crate::theme::BG);
        // The top bar is over the page: what scrolls under it takes no tap there.
        kit::above(&mut crate::cx!(self), crate::ui::layer::CHROME, [-1000.0, 0.0, WIDTH + 2000.0, bottom]);
    }

    /// Draw one frame. Returns when the next is wanted.
    pub fn frame(&mut self, now_ns: u64) -> Next {
        let dt = ((now_ns.saturating_sub(self.last_ns)) as f32 / 1e9).clamp(0.0, 0.05);
        self.last_ns = now_ns;
        self.now_ns = now_ns;
        self.wake = None;
        self.take_answers();
        self.focus.keyboard_up = self.keyboard_up;
        let tapped = self.ui.tap.is_some();
        let (w, h) = self.size;
        self.canvas.begin(w, h, self.insets.0, self.insets.1);
        self.focus.at = None;
        kit::ground(&mut self.canvas);
        let route = self.route().clone();
        // Held commands are counted on every page, for the drawer's Inbox.
        if self.online() {
            crate::inbox::keep_up(self);
        }
        // Layers keep each tap on the surface it landed on (ui.rs): the page first, on the bottom.
        self.ui.layer = crate::ui::layer::CONTENT;
        let mut moving = match &route {
            Route::Pair => crate::pair::draw(self, dt),
            Route::Home => crate::home::draw(self, dt),
            Route::Chat(name) => crate::chat::draw(self, &name.clone(), dt),
            Route::Settings => crate::settings::draw(self, dt),
            Route::Changes => crate::changes::draw(self, dt),
            Route::Designs => crate::designs::draw(self, dt),
            Route::Design(id) => crate::designs::draw_design(self, *id, dt),
            Route::Board => crate::board::draw(self, dt),
            Route::Task(id) => crate::board::draw_task(self, *id, dt),
            Route::Inbox => crate::inbox::draw(self, dt),
        };
        // "*" (the link came back) has been seen by every page drawn this frame.
        self.changed.remove("*");
        if route != Route::Pair && route != Route::Settings {
            moving |= crate::drawer::draw(self, dt);
            // The drawer has no field: typing stops and the keyboard goes as it opens.
            if self.st.drawer.open && self.focus.id.is_some() {
                self.finish_typing();
            }
            moving |= crate::drawer::draw_new_agent(self);
        }
        if let Some((words, until)) = self.toast.clone() {
            if now_ns < until {
                let bottom = self.canvas.h - self.canvas.inset_bottom - 96.0;
                kit::toast(&mut self.canvas, &words, bottom);
                self.wake_at(until);
            } else {
                self.toast = None;
            }
        }
        if self.route() == &route {
            self.fresh = false;
        }
        self.typed_done = None;
        // With no waker (a test, a screenshot), frames keep coming while answers are out.
        moving |= !self.calls.is_empty() && !crate::wake::has();
        self.canvas.finish();
        self.ui.end_frame();
        if moving || tapped || self.ui.busy() || self.fresh {
            Next::Now
        } else if let Some(at) = self.wake {
            Next::At(at)
        } else {
            Next::OnInput
        }
    }

    // Screenshots

    /// Put sample data in place of the PC's, for `--shot` (no PC needed to look at a screen).
    pub fn sample(&mut self) {
        crate::sample::fill(self);
    }

    pub fn set_link_state(&mut self, s: LinkState) {
        self.link_state = s;
    }
}

/// A scrolling page's frame: where its content starts and ends on screen.
pub struct Page {
    pub top: f32,
    pub bottom: f32,
    pub moving: bool,
}
