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

use rc_link::{Call, Link, LinkState};
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
    /// Hand words to another app (Android's share sheet).
    fn share(&self, _text: &str) {}
    /// Whether [`Host::share`] reaches anything here: the menu offers Share only then.
    fn can_share(&self) -> bool {
        false
    }
    /// Tell the keyboard where the caret is (characters): after a tap in the field.
    fn caret(&self, _text: &str, _start: usize, _end: usize) {}
    /// Open a web address in the phone's browser (or the app that handles it).
    fn open_url(&self, _url: &str) {}
    /// Ask to post notifications (Android 13 and later).
    fn ask_notifications(&self) {}
    /// Keep the app alive in the background while a PC is paired (Android's `LinkService`), its
    /// ongoing notification saying `words`; off when none is.
    fn link_service(&self, _on: bool, _words: &str) {}
    /// Ask the person for a photo or screenshot; it comes back through [`crate::attach::picked`].
    fn pick_photo(&self) {}
    /// A light tick under the finger (Android's `performHapticFeedback`): on send, on a long press
    /// that copies, on a switch.
    fn haptic(&self) {}
    /// The screen's pixels per density-independent unit (Android's density / 160); 0 when unknown.
    /// The canvas never scales past a phone's look for it, so a tablet keeps a phone-sized column.
    fn density(&self) -> f32 {
        0.0
    }
    /// Whether the platform lets this app post notifications; none when it cannot say.
    fn can_notify(&self) -> Option<bool> {
        None
    }
    /// Open the platform's notification settings for this app.
    fn open_notification_settings(&self) {}
    /// The phone's own text size setting (Android's font scale; 1.0 is its default), which the
    /// app's Default text size follows (D20). Read when the app comes to the front.
    fn font_scale(&self) -> f32 {
        1.0
    }
    /// This app may hand an APK to the system's installer (Android; never the desktop).
    fn can_install(&self) -> bool {
        false
    }
    /// Hand a downloaded and checked APK to the system's installer (`update.rs`).
    fn install_apk(&self, _path: &std::path::Path) -> crate::update::Install {
        crate::update::Install::Failed
    }
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
    // Literata italic (the same OFL family, as Leaf ships it): emphasis in the agent's words.
    include_bytes!("../assets/fonts/literata_400i.ttf"),
];

/// A typical phone's width in Android's dp: its 390-unit column is drawn at density × 411 / 390
/// pixels per unit, which the canvas never goes past (with a tenth to spare for wider phones).
const WIDTH_DP: f32 = 411.0;

/// How long a terminal's foot read for a chat no longer open still says whether it works.
const SCREEN_FRESH_NS: u64 = 10_000_000_000;

/// Messages asked for in one transcript read.
const PAGE: usize = 200;

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
    /// A file, read only (`fileview`): in `worktree` (the repo's own checkout when None), `path`
    /// as given (relative or absolute, maybe ending in `:line`).
    File { repo: i64, worktree: Option<String>, path: String },
    /// What needs you: held commands, agents that ask, finished work.
    Inbox,
    /// One agent's whole terminal screen, read-only, with the key strip (D13).
    Terminal(String),
}

/// What a Task call's helper did, as last read.
#[derive(Clone, Debug, Default)]
pub struct Helper {
    pub items: Vec<crate::data::Item>,
    /// The PC keeps a log for it.
    pub found: bool,
    /// When it was read (ns).
    pub at: u64,
}

/// What a request was for, so its answer lands in the right place.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Projects,
    Agents,
    Transcript(String),
    /// A page older than what the phone holds of a conversation (scrolled to its top).
    Older(String),
    /// What a Task call's helper did: the agent and the call.
    Helper(String, String),
    /// Words typed into an agent's terminal (a message's words, from its send queue).
    Input(String),
    /// Enter after them, once the PC said it typed them.
    Enter(String),
    Create,
    Spawn(String),
    /// The repo's tasks, to link a new agent to one.
    Tasks,
    /// The rendered foot of an agent's terminal (`session.screen`).
    Screen(String),
    /// The repo's agents with closed ones, for Earlier (D1).
    Earlier,
    /// Why each stopped agent stopped (`session.restorable`).
    Restorable,
    /// Resume or Wake a stopped agent; Pause (park) or Close a live one.
    Resume(String),
    Park(String),
    Close(String),
}

#[derive(Default, Serialize, Deserialize)]
struct Config {
    repo: Option<i64>,
    last_chat: Option<String>,
    #[serde(default)]
    asked_notifications: bool,
    /// The model and effort New agent last started one with (None: Claude Code's default).
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    /// Text size (D9): "small" or "large"; Default when none.
    #[serde(default)]
    text_size: Option<String>,
    /// "Later" on an update: that build stays hidden until then (ms).
    #[serde(default)]
    update_later: Option<(u32, u64)>,
    /// Notifications switched off in Settings (D6): each kind is on until then.
    #[serde(default)]
    quiet: Vec<String>,
}

fn quiet_key(n: crate::settings::Notify) -> &'static str {
    match n {
        crate::settings::Notify::Asks => "asks",
        crate::settings::Notify::Done => "done",
        crate::settings::Notify::Holds => "holds",
    }
}

/// Whether this kind of notification is on, read from the app's settings in `data` (the host's
/// data dir): for code that posts notifications without an [`App`], as the background loop does.
/// How many answers in a row each poll got with nothing new in them (B14).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rest {
    pub list: u32,
    pub chat: u32,
    pub screen: u32,
}

/// A poll's wait after `level` answers with nothing new: `base`, doubled for each, at most `cap`.
pub fn rested(base: u64, level: u32, cap: u64) -> u64 {
    base.saturating_mul(1 << level.min(8)).min(cap)
}

const LIST_EVERY: u64 = 5_000_000_000;
const LIST_REST: u64 = 60_000_000_000;
const CHAT_EVERY: u64 = 4_000_000_000;
const CHAT_REST: u64 = 30_000_000_000;
const SCREEN_EVERY: u64 = 3_000_000_000;
const SCREEN_REST: u64 = 20_000_000_000;

pub fn notifies_in(data: &std::path::Path, n: crate::settings::Notify) -> bool {
    read_json::<Config>(data.join("config.json")).is_none_or(|c| !c.quiet.iter().any(|q| q == quiet_key(n)))
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
    pub fileview: crate::fileview::State,
    pub inbox: crate::inbox::State,
    pub designs: crate::designs::State,
    pub agents: crate::agents::State,
    pub commands: crate::commands::State,
    pub update: crate::update::State,
    pub news: crate::news::State,
    pub keys: crate::keys::State,
    pub terminal: crate::terminal::State,
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
    /// The repo's stopped and closed agents, as last read for Earlier (D1).
    pub earlier: Vec<Agent>,
    /// Why each restorable agent stopped (`crash`, `app_restart`) and whether its checkout has
    /// uncommitted changes, by name.
    pub restorable: HashMap<String, (String, bool)>,
    /// The repo every list shows (design 44, D5): one switch, remembered.
    pub repo: Option<i64>,
    pub convos: HashMap<String, Convo>,
    pub sending: HashMap<String, Sending>,
    /// What each Task call's helper did, by agent and call, and when it was read.
    pub helpers: HashMap<(String, String), Helper>,
    /// Why the PC could not read a conversation, until it next can.
    pub chat_errors: HashMap<String, String>,
    /// When each terminal's foot was read (ns).
    screens_at: HashMap<String, u64>,
    /// Chats' copies being read off the UI thread ([`App::open`]).
    opening: Vec<(String, std::sync::mpsc::Receiver<Option<Convo>>)>,
    /// What you started writing to each agent, kept across agents and restarts.
    pub drafts: HashMap<String, String>,
    /// The drafts or a send queue changed at this time (ns): written to the phone a second later.
    outbox_at: Option<u64>,
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
    /// The PC's Relay is older than `chat.send`: messages go the old way (`session.input`).
    pub no_chat_send: bool,
    /// Held commands waiting on you (the inbox counts them with agents that ask).
    pub holds: usize,
    screen_at: u64,
    pub st: States,
    calls: Vec<(Ask, Call)>,
    /// The agents' list is read again once this passes (an event said it changed).
    agents_stale: bool,
    /// Acts that change an agent's state from here (close, pause, resume, create), counted; and
    /// the count when the list now on its way was asked for. A list asked before the latest act
    /// is dropped and asked again.
    acts: u64,
    agents_asked: u64,
    /// The open conversation is read again at this time.
    pub reread_at: u64,
    /// How long each poll has found nothing new: each answer the same as the last doubles its
    /// wait, up to a cap; anything new, an event or a touch brings it back to fast.
    pub rest: Rest,
    /// Kinds of thing the PC said changed since a page last looked (`git`, `design`, `task`,
    /// `guardrail`, `notify`, ...): the first part of each event's name. A page takes its own with
    /// [`App::take_changed`] and reads again.
    changed: std::collections::HashSet<String>,
    pub foreground: bool,
    pub data: PathBuf,
    config: Config,
    toast: Option<(String, u64)>,
    pub typed_done: Option<&'static str>,
    pub keyboard_up: bool,
    /// Android's font scale when the app last came to the front (D20).
    pub font_scale: f32,
    /// Whether Android lets this app post notifications, read when it comes to the front (D22).
    pub can_notify: Option<bool>,
    /// Screenshots: nothing is sent to the PC.
    pub shot: bool,
    size: (f32, f32),
    insets: (f32, f32),
    last_ns: u64,
    pub now_ns: u64,
    wake: Option<u64>,
}

pub(crate) fn read_json<T: for<'a> Deserialize<'a>>(p: PathBuf) -> Option<T> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Write a copy off the UI thread.
pub(crate) fn write_json<T: Serialize>(p: PathBuf, v: &T) {
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

pub(crate) fn chat_file(data: &std::path::Path, name: &str) -> PathBuf {
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
        // One link for the process, which LinkService may have made already (background.rs).
        let link = crate::background::link(&data);
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
        let outbox: data::Outbox = read_json(data.join("cache/outbox.json")).unwrap_or_default();
        let drafts = outbox.drafts.clone();
        let sending = outbox.sending();
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
            earlier: Vec::new(),
            restorable: HashMap::new(),
            repo,
            convos,
            sending,
            drafts,
            outbox_at: None,
            opening: Vec::new(),
            screens_at: HashMap::new(),
            chat_errors: HashMap::new(),
            helpers: HashMap::new(),
            screens: HashMap::new(),
            efforts: HashMap::new(),
            busy_since: HashMap::new(),
            agents_at: 0,
            list_at: 0,
            no_screen: false,
            no_chat_send: false,
            holds: 0,
            screen_at: 0,
            st: States::default(),
            calls: Vec::new(),
            agents_stale: true,
            acts: 0,
            agents_asked: 0,
            reread_at: 0,
            rest: Rest::default(),
            changed: Default::default(),
            foreground: true,
            data,
            config,
            toast: None,
            typed_done: None,
            keyboard_up: false,
            font_scale: 1.0,
            can_notify: None,
            shot: false,
            size: (1080.0, 2340.0),
            insets: (0.0, 0.0),
            last_ns: now_ns,
            now_ns,
            wake: None,
        };
        app.service();
        let mut app = app;
        app.font_scale = app.host.font_scale();
        app.can_notify = app.host.can_notify();
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

    /// The toast up now: tests.
    #[cfg(test)]
    pub fn toast_words(&self) -> Option<&str> {
        self.toast.as_ref().map(|t| t.0.as_str())
    }

    pub fn toast(&mut self, words: &str) {
        self.toast = Some((words.to_string(), self.now_ns + 2_500_000_000));
    }

    /// The PC made the agent New agent asked for: its chat opens now, starting, while the PC
    /// starts it.
    pub fn created(&mut self, v: &Value) {
        let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        if name.is_empty() {
            self.toast("The PC made no agent");
        } else {
            // Its chat opens now, starting, while the PC starts it.
            self.acts += 1;
        if let Some(mut a) = data::agents(&json!({"sessions": [v.clone()]})).pop() {
                a.state = "spawning".into();
                self.agents.retain(|b| b.name != name);
                self.agents.push(a);
            }
            self.st.newagent.open = false;
            self.st.newagent.task.clear();
            self.st.newagent.linked = None;
            self.ask(Ask::Spawn(name.clone()), "session.spawn", json!({"session": name}));
            self.open(&name);
        }
    }

    /// The text size chosen in Settings: 0 Small, 1 Default, 2 Large.
    pub fn text_size(&self) -> usize {
        match self.config.text_size.as_deref() {
            Some("small") => 0,
            Some("large") => 2,
            _ => 1,
        }
    }

    pub fn set_text_size(&mut self, i: usize) {
        self.config.text_size = match i {
            0 => Some("small".into()),
            2 => Some("large".into()),
            _ => None,
        };
        self.save_config();
    }

    /// The build "Later" put off, and until when (ms).
    pub fn update_later(&self) -> Option<(u32, u64)> {
        self.config.update_later
    }

    pub fn clear_update_later(&mut self) {
        if self.config.update_later.take().is_some() {
            self.save_config();
        }
    }

    pub fn set_update_later(&mut self, build: u32, until_ms: u64) {
        self.config.update_later = Some((build, until_ms));
        self.save_config();
    }

    /// Whether this kind of notification is on (Settings).
    pub fn notifies(&self, n: crate::settings::Notify) -> bool {
        !self.config.quiet.iter().any(|q| q == quiet_key(n))
    }

    pub fn set_notifies(&mut self, n: crate::settings::Notify, on: bool) {
        self.config.quiet.retain(|q| q != quiet_key(n));
        if !on {
            self.config.quiet.push(quiet_key(n).to_string());
        }
        self.save_config();
    }

    /// The model and effort New agent offers first: the last ones used.
    pub fn last_choice(&self) -> (Option<String>, Option<String>) {
        (self.config.model.clone(), self.config.effort.clone())
    }

    pub fn set_choice(&mut self, model: Option<&str>, effort: Option<&str>) {
        self.config.model = model.map(str::to_string);
        self.config.effort = effort.map(str::to_string);
        self.save_config();
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
        if ask == Ask::Agents {
            self.agents_asked = self.acts;
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
        // The terminal's foot is read only for the open chat: trusted there, or for a few seconds
        // after a read elsewhere. Past that it is stale (an agent last seen working would show
        // working for ever).
        let fresh = self.open_chat().as_deref() == Some(name) || self.screens_at.get(name).is_some_and(|t| self.now_ns.saturating_sub(*t) < SCREEN_FRESH_NS);
        // Out of reach, the last foot read is old news: no live line from it.
        if !self.link_state.online() {
            return false;
        }
        if !self.no_screen
            && fresh
            && let Some(s) = self.screens.get(name)
        {
            return s.status.is_some();
        }
        a.working()
    }

    /// An agent's state in words, as its chat header says it: "working" while its terminal says
    /// so ([`App::busy`]), which the list's freshness can lag.
    pub fn agent_words(&self, a: &Agent) -> &'static str {
        if self.link_state.online() && a.alive() && !a.needs_you() && self.busy(&a.name) { "working" } else { a.state_words() }
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
        let mut p = json!({"session": name, "limit": PAGE});
        if let Some(a) = after {
            p["after"] = json!(a);
        }
        self.ask(Ask::Transcript(name.to_string()), "session.transcript", p);
    }

    /// Read what the helper of `name`'s Task call `call` did (its own log on the PC).
    pub fn read_helper(&mut self, name: &str, call: &str) {
        if self.online() {
            self.ask(Ask::Helper(name.to_string(), call.to_string()), "session.transcript", json!({"session": name, "task": call, "limit": PAGE}));
        }
    }

    /// Read the page older than what the phone holds of `name`'s conversation, when there is one.
    /// True while it is being read (or was just asked for).
    pub fn read_older(&mut self, name: &str) -> bool {
        let ask = Ask::Older(name.to_string());
        if self.asking(&ask) {
            return true;
        }
        let Some(before) = self.convos.get(name).filter(|c| c.older).and_then(|c| c.before) else { return false };
        if !self.online() {
            return false;
        }
        self.ask(ask, "session.transcript", json!({"session": name, "before": before, "limit": PAGE}));
        true
    }

    /// Send a message to an agent: it joins the agent's send queue ([`data::Sending`]), which types
    /// the words and then Enter into its terminal, as the old app and the PC's own composer do,
    /// now or once the link is back. Several lines go as one paste.
    pub fn say(&mut self, name: &str, text: &str) {
        let text = text.trim_end();
        if text.is_empty() {
            return;
        }
        // A shell command heads this agent's `!` list next time (D3).
        crate::commands::remember(self, name, text);
        let now = rc_link::link::now_ms();
        self.sending.entry(name.to_string()).or_default().push(text, now);
        self.outbox_changed();
        self.send_queued();
        self.reread_at = self.now_ns + 600_000_000;
    }

    /// An answer of `session.transcript` for `name`: what came in joins its conversation, and what
    /// you sent that it now holds stops waiting.
    pub fn took_transcript(&mut self, name: &str, v: &Value) {
        self.chat_errors.remove(name);
        let c = self.convos.entry(name.to_string()).or_default();
        let before = (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()), c.cursor);
        // A reset (a first read, `/clear`, a new log) holds every message of yours already
        // entered: none of them may wait on to be drawn a second time.
        let reset = v.get("reset").and_then(Value::as_bool) == Some(true) || before.2.is_none();
        let full = v.get("messages").and_then(Value::as_array).is_some_and(|m| m.len() >= PAGE);
        let (seen, ids) = c.take_sent(v);
        let changed = before != (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()), c.cursor);
        self.rest.chat = if changed { 0 } else { self.rest.chat.saturating_add(1) };
        // Your messages come back by their ids (chat.send); one with none (an older PC, or typed
        // at the PC) takes the oldest entered bubble, never both for one message.
        if let Some(s) = self.sending.get_mut(name) {
            s.seen_ids(&ids);
            if reset {
                s.reset();
            }
            s.seen(seen);
        }
        if !ids.is_empty() {
            self.outbox_changed();
        }
        if reset {
            self.outbox_changed();
        }
        // A full page means more came after it (away a while): read on at once.
        if full && !reset {
            self.reread_at = 0;
        }
        if seen > 0 {
            self.outbox_changed();
        }
        // Kept for the next start only when something came in; written (and turned
        // into JSON) off the UI thread.
        if changed {
            write_owned(chat_file(&self.data, name), self.convos[name].clone());
        }
    }

    /// The link came back: every page reads again, and a PC that could not send the terminal's
    /// foot (an older one) is asked again, since it may have been updated.
    pub(crate) fn came_online(&mut self) {
        self.no_screen = false;
        self.no_chat_send = false;
        self.changed.insert("*".into());
        self.refresh();
    }

    /// How long ago `name`'s terminal foot was read (ms), when it was.
    pub fn screen_age_ms(&self, name: &str) -> Option<u64> {
        self.screens_at.get(name).map(|t| self.now_ns.saturating_sub(*t) / 1_000_000)
    }

    /// What `name`'s terminal shows at its foot, read now.
    pub fn put_screen(&mut self, name: &str, s: crate::screen::Screen) {
        self.screens.insert(name.to_string(), s);
        self.screens_at.insert(name.to_string(), self.now_ns);
    }

    /// What you started writing to `name`.
    pub fn draft(&self, name: &str) -> String {
        self.drafts.get(name).cloned().unwrap_or_default()
    }

    pub fn set_draft(&mut self, name: &str, text: &str) {
        if self.drafts.get(name).map_or(text.is_empty(), |d| d == text) {
            return;
        }
        self.drafts.insert(name.to_string(), text.to_string());
        self.outbox_changed();
    }

    /// A draft or a send queue changed: keep it on the phone soon.
    pub fn outbox_changed(&mut self) {
        self.outbox_at.get_or_insert(self.now_ns);
    }

    /// Write the drafts and the send queues, off the UI thread.
    fn save_outbox(&mut self) {
        if self.outbox_at.take().is_some() && !self.shot {
            write_owned(self.data.join("cache/outbox.json"), data::Outbox::of(&self.drafts, &self.sending));
        }
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
            let chat = !self.no_chat_send;
            let Some(step) = self.sending.get_mut(&name).and_then(|s| s.next_by(now, chat)) else { continue };
            if !matches!(step, data::Step::Wait(_)) {
                self.outbox_changed();
            }
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
                data::Step::Send(text, client_id) => {
                    // The PC types the words and Enter, once per id however often it is sent.
                    let call = self.link.call("chat.send", json!({"session": name, "text": text, "client_id": client_id}));
                    self.calls.push((Ask::Input(name), call));
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
                LinkState::Online { .. } if !was_online => self.came_online(),
                LinkState::Unpaired | LinkState::Revoked { .. } if self.route() != &Route::Pair => {
                    self.nav = vec![(Route::Pair, Scroll::default())];
                    self.shown();
                }
                _ => {}
            }
        }
        let events = self.link.take_events();
        if !events.is_empty() {
            self.stir();
        }
        for e in events {
            if let Some((domain, _)) = e.ev.split_once('.') {
                self.changed.insert(domain.to_string());
            }
            if e.ev.starts_with("session.") {
                self.agents_stale = true;
                // Earlier is read again only when an agent stops, closes or comes back: a live
                // agent's intent and state change all the time, and Earlier is a big read.
                let name = e.payload.get("name").and_then(Value::as_str);
                let stopped = e.payload.get("state").and_then(Value::as_str).is_some_and(|s| !matches!(s, "created" | "spawning" | "running" | "idle" | "blocked"));
                if stopped || name.is_some_and(|n| self.earlier.iter().any(|a| a.name == n) || self.agents.iter().any(|a| a.name == n && !crate::agents::live(a))) {
                    self.st.agents.stale = true;
                }
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
        // Away, the notifications' loop reads the list (background.rs): asked once, not twice.
        if self.agents_stale && self.foreground && self.link_state.online() && !self.asking(&Ask::Agents) {
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
                // A list asked before the latest act (a close, a pause, a resume, a new agent) may
                // still hold the agent as it was: it would undo what the phone just showed. Asked
                // again instead (D22).
                (Ask::Agents, Ok(_)) if self.agents_asked < self.acts => {
                    self.agents_stale = true;
                }
                (Ask::Agents, Ok(v)) => {
                    crate::background::listed(&v);
                    let agents = data::agents(&v);
                    if agents != self.agents {
                        self.agents = agents;
                        write_json(self.data.join("cache/agents.json"), &self.agents);
                        self.rest.list = 0;
                    } else {
                        self.rest.list = self.rest.list.saturating_add(1);
                    }
                }
                (Ask::Transcript(name), Ok(v)) => self.took_transcript(&name, &v),
                (Ask::Helper(name, call), Ok(v)) => {
                    let found = v.get("found").and_then(Value::as_bool).unwrap_or(false);
                    let items = data::items(&data::messages(&v));
                    let changed = self.helpers.get(&(name.clone(), call.clone())).is_none_or(|h| h.items != items || h.found != found);
                    self.helpers.insert((name, call.clone()), Helper { items, found, at: self.now_ns });
                    if changed {
                        crate::chat::helper_came(self, &call);
                    }
                }
                (Ask::Older(name), Ok(v)) => {
                    if let Some(c) = self.convos.get_mut(&name) {
                        c.take_older(&v);
                        write_owned(chat_file(&self.data, &name), c.clone());
                    }
                }
                (Ask::Input(name) | Ask::Enter(name), r) => {
                    let now = rc_link::link::now_ms();
                    let answer = match &r {
                        Ok(_) => data::Answer::Ok,
                        Err(e) if e.transient() => data::Answer::Dropped,
                        // An older Relay has no chat.send: from now on the old way.
                        Err(e) if !self.no_chat_send && (e.code == "remote.op" || e.code.contains("unknown")) => {
                            self.no_chat_send = true;
                            data::Answer::Unsupported
                        }
                        Err(_) => data::Answer::Refused,
                    };
                    if let Some(s) = self.sending.get_mut(&name) {
                        s.answered(answer, now);
                    }
                    self.outbox_changed();
                    // A drop is sent again by itself; only a refusal is the person's to retry.
                    if let Err(e) = r
                        && answer == data::Answer::Refused
                    {
                        self.toast(&format!("Not sent. {}", crate::errors::words(&e)));
                    }
                }
                (Ask::Create, Ok(v)) => self.created(&v),
                (Ask::Spawn(_), Ok(_)) => {
                    self.agents_stale = true;
                }
                (Ask::Tasks, Ok(v)) => {
                    self.st.newagent.tasks = crate::board::tasks(&v);
                }
                (Ask::Screen(name), Ok(v)) => {
                    let text = v.get("text").and_then(Value::as_str).unwrap_or_default();
                    let scr = crate::screen::read(text);
                    self.rest.screen = if self.screens.get(&name) == Some(&scr) { self.rest.screen.saturating_add(1) } else { 0 };
                    self.put_screen(&name, scr);
                }
                (Ask::Screen(_), Err(e)) => {
                    if e.code == "remote.op" || e.code.contains("unknown") {
                        self.no_screen = true;
                    }
                }
                (Ask::Earlier, Ok(v)) => {
                    let mut all = data::agents(&v);
                    all.retain(|a| !crate::agents::live(a));
                    all.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                    all.truncate(200);
                    self.earlier = all;
                }
                (Ask::Restorable, Ok(v)) => {
                    self.restorable = v
                        .get("sessions")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|r| {
                                    let name = r.pointer("/session/name")?.as_str()?.to_string();
                                    let reason = r.get("reason").and_then(Value::as_str).unwrap_or_default().to_string();
                                    Some((name, (reason, r.get("worktree_dirty").and_then(Value::as_bool).unwrap_or(false))))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                }
                (Ask::Resume(name), Ok(_)) => {
                    self.acts += 1;
                    self.agents_stale = true;
                    self.st.agents.stale = true;
                    if self.open_chat().as_deref() == Some(name.as_str()) {
                        self.reread_at = 0;
                        self.screen_at = 0;
                    }
                }
                (Ask::Park(name), Ok(_)) => {
                    self.acts += 1;
                    self.agents_stale = true;
                    self.st.agents.stale = true;
                    self.toast(&format!("{name} is paused"));
                }
                (Ask::Close(name), Ok(_)) => {
                    self.acts += 1;
                    self.agents_stale = true;
                    self.st.agents.stale = true;
                    // Kept as closed until the lists say so, so its chat turns read-only at once.
                    if let Some(a) = self.agents.iter().find(|a| a.name == name).cloned() {
                        self.agents.retain(|b| b.name != name);
                        self.earlier.insert(0, Agent { state: "closed".into(), ..a });
                    }
                    self.toast(&format!("{name} is closed"));
                }
                (Ask::Close(name), Err(e)) if e.code == "worktree.dirty" => {
                    self.st.agents.confirm = Some((name, crate::agents::Confirm::Discard));
                }
                (Ask::Create | Ask::Spawn(_) | Ask::Resume(_) | Ask::Park(_) | Ask::Close(_), Err(e)) => self.toast(&crate::errors::words(&e)),
                // A conversation the PC could not read (not a dropped link): the chat says so.
                (Ask::Transcript(name), Err(e)) if !e.transient() => {
                    self.chat_errors.insert(name, crate::errors::words(&e));
                }
                (_, Err(e)) => self.host.log(&format!("{}: {}", e.code, e.message)),
            }
        }
        // Chats' copies read off the UI thread: kept unless the PC's answer came first.
        let convos = &mut self.convos;
        self.opening.retain(|(name, rx)| match rx.try_recv() {
            Ok(c) => {
                if let Some(c) = c
                    && !convos.contains_key(name)
                {
                    convos.insert(name.clone(), c);
                }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        // What waited for the link, or for the write before it, goes now.
        self.send_queued();
        // Drafts and queues are written a second after they last changed, not on each key.
        if let Some(t) = self.outbox_at {
            let due = t + 1_000_000_000;
            if self.now_ns >= due {
                self.save_outbox();
            } else {
                self.wake_at(due);
            }
        }
        // The lists say who works and who waits by how fresh each agent's output is: read them
        // again every five seconds while the app is in front.
        if self.link_state.online() && self.foreground && self.now_ns >= self.list_at {
            self.agents_stale = true;
            self.list_at = self.now_ns + rested(LIST_EVERY, self.rest.list, LIST_REST);
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
                self.reread_at = self.now_ns + if busy { 700_000_000 } else { rested(CHAT_EVERY, self.rest.chat, CHAT_REST) };
            }
            // The terminal's foot: the live status, the mode, a question.
            if !self.no_screen && self.now_ns >= self.screen_at && !self.asking(&Ask::Screen(name.clone())) && self.live(&name) {
                self.ask(Ask::Screen(name.clone()), "session.screen", json!({"session": name, "rows": 16}));
                self.screen_at = self.now_ns + if busy { 700_000_000 } else { rested(SCREEN_EVERY, self.rest.screen, SCREEN_REST) };
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

    // Repos

    pub fn repo_name(&self) -> String {
        self.repo.and_then(|r| self.projects.iter().find(|p| p.id == r)).map_or_else(|| "No repo".to_string(), |p| p.name.clone())
    }

    /// Switch repo and stay on the same kind of page (D5).
    pub fn set_repo(&mut self, id: i64) {
        self.repo = Some(id);
        self.config.repo = Some(id);
        self.save_config();
        // An agent's chat or Terminal of another repo, open or under the page, goes: back to Home,
        // as Back from a chat goes (D22: the Terminal's keys would type into the old repo's agent).
        let other = |r: &Route| match r {
            Route::Chat(n) | Route::Terminal(n) => self.agent(n).is_some_and(|a| a.project_id != id),
            _ => false,
        };
        if self.nav.iter().any(|(r, _)| other(r)) {
            self.nav.truncate(1);
            self.shown();
        }
    }

    /// The repo's live agents: stopped and closed ones are under Earlier.
    pub fn repo_agents(&self) -> Vec<&Agent> {
        self.agents.iter().filter(|a| Some(a.project_id) == self.repo && crate::agents::live(a)).collect()
    }

    /// An agent by name, live or earlier.
    pub fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.name == name).or_else(|| self.earlier.iter().find(|a| a.name == name))
    }

    /// The agent has a terminal running (or starting), so it takes messages.
    pub fn live(&self, name: &str) -> bool {
        self.agents.iter().find(|a| a.name == name).is_some_and(crate::agents::live)
    }

    /// Open an agent's conversation over Home.
    pub fn open(&mut self, name: &str) {
        // The phone's copy is read and parsed off the UI thread (a long chat is megabytes of JSON);
        // it shows as soon as it is in, unless the PC answered first.
        if !self.convos.contains_key(name) && !self.shot {
            let (tx, rx) = std::sync::mpsc::channel();
            let (file, n) = (chat_file(&self.data, name), name.to_string());
            let _ = std::thread::Builder::new().name("rc-open".into()).spawn(move || {
                let _ = tx.send(read_json::<Convo>(file));
                crate::wake::wake();
            });
            self.opening.push((n, rx));
        }
        if let Some(a) = self.agent(name)
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

    /// A notification about `name` was tapped: its chat, with nothing left over it (no sheet, menu,
    /// drawer or keyboard). Not while pairing: there is no PC to read it from.
    pub fn open_notified(&mut self, name: &str) {
        if self.route() == &Route::Pair || name.is_empty() {
            return;
        }
        self.finish_typing();
        self.st.drawer.open = false;
        self.st.drawer.repos = false;
        self.st.newagent.open = false;
        self.st.newagent.picking = false;
        self.st.agents.menu = None;
        self.st.agents.confirm = None;
        self.st.update.open = false;
        self.st.chat.model_sheet = false;
        self.open(name);
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
        // Everything the old PC said goes, so a new pairing starts clean (D22): repo ids start at 1
        // on every PC, so a kept page would show the old PC's branch, files, tasks or holds, and a
        // kept update offer would install its build.
        self.agents.clear();
        self.earlier.clear();
        self.restorable.clear();
        self.projects.clear();
        self.repo = None;
        self.convos.clear();
        self.helpers.clear();
        self.chat_errors.clear();
        self.screens.clear();
        self.screens_at.clear();
        self.opening.clear();
        self.efforts.clear();
        self.busy_since.clear();
        self.drafts.clear();
        self.sending.clear();
        self.outbox_at = None;
        self.holds = 0;
        self.no_screen = false;
        self.no_chat_send = false;
        self.changed.clear();
        // The old PC's answers still on their way land nowhere.
        self.calls.clear();
        // Every page's state, but the phone's own choices: the terminal's text size.
        let terminal_size = self.st.terminal.size;
        self.st = States::default();
        self.st.terminal.size = terminal_size;
        // What was the PC's goes; what is the phone's (text size, notification switches, the
        // one-time ask for notifications) stays.
        self.config = Config {
            text_size: self.config.text_size.take(),
            quiet: std::mem::take(&mut self.config.quiet),
            asked_notifications: self.config.asked_notifications,
            ..Config::default()
        };
        self.save_config();
        let _ = std::fs::remove_dir_all(self.data.join("cache"));
        let _ = std::fs::remove_dir_all(self.data.join("updates"));
        self.reset_to(Route::Pair);
    }

    // The platform

    /// The app came to the front: check the link at once, read the open page again.
    pub fn resumed(&mut self) {
        // Android's text size may have changed while away (it is set in its own Settings).
        self.font_scale = self.host.font_scale();
        self.can_notify = self.host.can_notify();
        self.foreground = true;
        self.fresh = true;
        self.link.kick();
        self.agents_stale = true;
        self.reread_at = 0;
    }

    pub fn paused(&mut self) {
        self.foreground = false;
        // Android may stop the process once it is away: what you wrote is kept now.
        self.save_outbox();
        crate::background::front(false, None);
    }

    /// In the background: take what the link brought without drawing (notifications are
    /// background.rs's, which runs with no activity too).
    ///
    /// A commit's push still goes out, and a message's Enter its moment after the words: the
    /// answer is when to tick again, if anything is due.
    pub fn tick(&mut self, now_ns: u64) -> Option<u64> {
        self.now_ns = now_ns;
        self.wake = None;
        self.take_answers();
        crate::changes::keep_up(self);
        self.wake
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

    /// Something happened (an event, a touch): the polls that had slowed down go back to fast,
    /// from now.
    pub fn stir(&mut self) {
        if self.rest == Rest::default() {
            return;
        }
        self.rest = Rest::default();
        self.list_at = self.list_at.min(self.now_ns + LIST_EVERY);
        self.reread_at = self.reread_at.min(self.now_ns + CHAT_EVERY);
        self.screen_at = self.screen_at.min(self.now_ns + SCREEN_EVERY);
    }

    pub fn touch(&mut self, t: Touch) {
        self.stir();
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
        // A page is shown clean (D11): what was open over the last one, a confirm, a sheet or a
        // menu, does not wait for the next visit ("Discard changes to X?" on opening Changes).
        while self.st.changes.close_over() {}
        while self.st.inbox.close_over() {}
        // The Send back sheet (Task, Inbox): kept while it sends, when take_answers closes it.
        self.st.board.close_over();
        self.st.settings.close_over();
        while self.st.agents.close_over() {}
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
        // The task list goes back to New agent, not past it.
        if std::mem::take(&mut self.st.newagent.picking) {
            self.ui.reset();
            return true;
        }
        // What is over the page closes first, the top one first; a page's own overlays only on
        // that page, so a flag left elsewhere never takes a Back (D11).
        let route = self.route().clone();
        if self.st.update.close_over()
            || self.st.agents.close_over()
            || self.st.drawer.close_over()
            || self.st.chat.close_over()
            || self.st.newagent.open
            || matches!(route, Route::Design(_)) && self.st.designs.close_over()
            || route == Route::Changes && self.st.changes.close_over()
            || route == Route::Inbox && self.st.inbox.close_over()
            || matches!(route, Route::Task(_) | Route::Inbox) && self.st.board.close_over()
            || route == Route::Settings && self.st.settings.close_over()
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
        // Out of reach, the top says when it tries again, to the second: drawn again as it ticks.
        if let LinkState::Offline { retry_at_ms, .. } = self.link_state {
            let now_ms = rc_link::link::now_ms();
            if retry_at_ms > now_ms {
                self.wake_at(now_ns + ((retry_at_ms - now_ms) % 1000).max(1) * 1_000_000);
            }
        }
        // The text size chosen in Settings (D9): every Text made this frame takes it, and the
        // chat measures again when it changed.
        let scale = crate::settings::text_scale(self.text_size(), self.font_scale);
        if scale != crate::draw::text_scale() {
            crate::draw::set_text_scale(scale);
            self.st.chat.relayout();
        }
        // A phone's scale for this screen's density at most: a tablet keeps the phone's column.
        let d = self.host.density();
        self.canvas.max_u = if d > 0.0 { d * (WIDTH_DP / WIDTH) * 1.1 } else { f32::INFINITY };
        // A tapped notification opens its agent's chat (D4), over whatever was open.
        if let Some(name) = crate::background::take_opened() {
            self.open_notified(&name);
        }
        // What is on the screen: the chat open here does not notify (background.rs).
        crate::background::front(self.foreground, self.open_chat());
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
        // A send-back (or an approval) already on its way finishes on any page, online or not: a
        // call the link dropped fails at once and frees its key (C18; it starts nothing new).
        crate::board::take_answers(self);
        // A commit's push goes out from here, whatever page is open.
        crate::changes::keep_up(self);
        // Designs' finished paints are taken (never left to pile up), and leaving drops its queue.
        crate::designs::keep_up(self);
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
            Route::File { repo, worktree, path } => crate::fileview::draw(self, *repo, worktree.clone(), path, dt),
            Route::Inbox => crate::inbox::draw(self, dt),
            Route::Terminal(name) => crate::terminal::draw(self, &name.clone(), dt),
        };
        // "*" (the link came back) has been seen by every page drawn this frame.
        self.changed.remove("*");
        if route != Route::Pair && route != Route::Settings {
            moving |= crate::drawer::draw(self, dt);
            // The drawer's one field is its search: anything else typed stops as it opens.
            if self.st.drawer.open && self.focus.id.is_some_and(|f| f != crate::agents::SEARCH) {
                self.finish_typing();
            }
            moving |= crate::drawer::draw_new_agent(self);
            // Earlier (D1) is read while the drawer or Home shows it.
            if self.st.drawer.open || route == Route::Home {
                crate::agents::keep_up(self);
            }
            crate::agents::draw_over(self);
        }
        // What you have seen of each agent, and its rows' previews (D12).
        if route != Route::Pair {
            crate::news::keep_up(self);
        }
        // App updates from the PC (D10): asked for while in front, offered over any page but pairing.
        if route != Route::Pair && !self.shot {
            crate::update::keep_up(self);
        }
        moving |= crate::update::draw(self);
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
