//! The link to the paired PC: dials every route (direct first, the server last), proves the
//! token, subscribes, keeps the session alive with `bus.ping`, and dials again with backoff when it
//! drops. A port of the Kotlin app's `Handshake.kt`, `BusSession.kt` and `Link.kt`, with one
//! change the owner asked for: **there is no Connect and no Disconnect.** Once a phone is paired the
//! link is always wanted: it connects when the app starts, right after pairing, and again by itself
//! after any drop. Only a PC that forgot this phone stops it (pair again).
//!
//! It runs on its own thread (a current-thread tokio runtime); the app calls it from the UI thread
//! without blocking: [`Link::call`] answers through a [`Call`] polled each frame, and every change
//! (state, an answer, an event) runs the waker the app gave, so a sleeping app draws at once.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::pairlink::{PairLink, Route, order_routes};
use crate::wire::{self, BusError, Event, Greeting, Incoming};

pub const DIAL_TIMEOUT: Duration = Duration::from_secs(8);
pub const GREETING_TIMEOUT: Duration = Duration::from_secs(10);
pub const WELCOME_TIMEOUT: Duration = Duration::from_secs(20);
/// The PC may ask a person to approve the phone first, which can take up to 75 seconds.
pub const PAIR_WELCOME_TIMEOUT: Duration = Duration::from_secs(90);
/// How long to give one route before also trying the next ("happy eyeballs").
pub const STAGGER: Duration = Duration::from_millis(400);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);
pub const KEEPALIVE: Duration = Duration::from_secs(25);
pub const PING_TIMEOUT: Duration = Duration::from_secs(10);
pub const ENGINE_START_TIMEOUT: Duration = Duration::from_secs(45);
pub const RECONNECT_MIN: Duration = Duration::from_secs(1);
pub const RECONNECT_MAX: Duration = Duration::from_secs(30);
/// A session shorter than this does not reset the backoff.
pub const STABLE: Duration = Duration::from_secs(5);

/// What the app hears about: coding only (design 44, D2).
pub const SUBSCRIBE: &[&str] = &["session.*", "guardrail.*", "notify.*", "task.*", "project.*", "git.*", "worktree.*", "design.*", "mailbox.*"];

/// A PC this phone paired with, and everything needed to reach it again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub host_id: String,
    pub name: String,
    pub instance: String,
    pub device: String,
    pub token: String,
    pub routes: Vec<Route>,
    #[serde(default)]
    pub last_route: Option<String>,
    #[serde(default)]
    pub paired_at_ms: u64,
    #[serde(default)]
    pub version: String,
}

/// Where the paired PC's profile lives.
pub trait Store: Send + Sync {
    fn load(&self) -> Option<Profile>;
    fn save(&self, p: &Profile);
    fn forget(&self);
}

/// The profile as a file in the app's private folder (`pc.json`). On Android that folder is the
/// app's own; the token is the phone's credential, so the file is written owner-only.
pub struct FileStore(pub std::path::PathBuf);

impl Store for FileStore {
    fn load(&self) -> Option<Profile> {
        serde_json::from_slice(&std::fs::read(&self.0).ok()?).ok()
    }

    fn save(&self, p: &Profile) {
        let Ok(bytes) = serde_json::to_vec_pretty(p) else { return };
        let tmp = self.0.with_extension("json.tmp");
        if let Some(dir) = self.0.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, bytes).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            let _ = std::fs::rename(&tmp, &self.0);
        }
    }

    fn forget(&self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Why a connection attempt ended without a session.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// The PC no longer knows this phone: pair again.
    Unpaired,
    /// Every route was tried and none answered.
    Unreachable,
    /// The server answered, but the PC is not connected to it.
    HostOffline,
    /// That door is the server's copy, not a PC.
    WrongDoor,
    Denied { code: String, text: String },
}

impl Refusal {
    pub fn of(error: Option<&str>) -> Refusal {
        match error {
            Some("auth.unknown_device") | Some("auth.bad_proof") => Refusal::Unpaired,
            Some("host.offline") => Refusal::HostOffline,
            Some(code) => Refusal::Denied { code: code.into(), text: denied_text(code) },
            None => Refusal::Denied { code: "internal".into(), text: "The PC refused the connection.".into() },
        }
    }

    pub fn text(&self) -> String {
        match self {
            Refusal::Unpaired => "This PC no longer knows this phone. Pair it again.".into(),
            Refusal::Unreachable => "The PC did not answer".into(),
            Refusal::HostOffline => "Your server answered, but the PC is not connected to it".into(),
            Refusal::WrongDoor => "That is your server's copy, not a PC".into(),
            Refusal::Denied { text, .. } => text.clone(),
        }
    }
}

fn denied_text(code: &str) -> String {
    match code {
        "pair.invalid" => "That pairing code is not valid any more. Ask for a new one.".into(),
        "pair.declined" => "The PC declined this phone.".into(),
        "pair.unconfirmed" => "Nobody approved this phone on the PC in time. Try again.".into(),
        other => format!("The PC refused the connection ({other})."),
    }
}

// One WebSocket, as lines

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A WebSocket read as lines: a text message may carry several.
pub struct Conn {
    ws: Ws,
    buf: VecDeque<String>,
}

fn install_crypto() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

impl Conn {
    pub async fn dial(url: &str) -> Result<Conn, String> {
        install_crypto();
        match tokio::time::timeout(DIAL_TIMEOUT, tokio_tungstenite::connect_async(url)).await {
            Ok(Ok((ws, _))) => Ok(Conn { ws, buf: VecDeque::new() }),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("timed out".into()),
        }
    }

    /// The next line, or None when the socket closed or `wait` passed.
    pub async fn line(&mut self, wait: Duration) -> Option<String> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            if let Some(l) = self.buf.pop_front() {
                return Some(l);
            }
            let msg = tokio::time::timeout_at(deadline, self.ws.next()).await.ok()??.ok()?;
            match msg {
                Message::Text(t) => self.buf.extend(t.as_str().split('\n').filter(|l| !l.trim().is_empty()).map(str::to_string)),
                Message::Close(_) => return None,
                _ => {}
            }
        }
    }

    pub async fn send(&mut self, line: String) -> bool {
        self.ws.send(Message::Text(line.into())).await.is_ok()
    }

    pub async fn close(mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(1), self.ws.close(None)).await;
    }
}

/// A socket that greeted, and which route it came by.
pub struct Greeted {
    pub conn: Conn,
    pub route: Route,
    pub greeting: Greeting,
}

/// Dial `routes` in order, each [`STAGGER`] after the one before unless it already failed, and keep
/// the first that greets. The others are dropped unanswered: the door drops a connection that
/// never says hello.
pub async fn greet(routes: &[Route]) -> Result<Greeted, Refusal> {
    if routes.is_empty() {
        return Err(Refusal::Unreachable);
    }
    let (tx, mut rx) = mpsc::channel::<Result<Greeted, Option<Refusal>>>(routes.len());
    let mut set = tokio::task::JoinSet::new();
    let mut started = 0;
    let mut failures = 0;
    let mut refused: Option<Refusal> = None;
    loop {
        if started < routes.len() {
            let route = routes[started].clone();
            let tx = tx.clone();
            set.spawn(async move {
                let out = attempt(route).await;
                let _ = tx.send(out).await;
            });
            started += 1;
        }
        let stagger = tokio::time::sleep(if started < routes.len() { STAGGER } else { Duration::from_secs(3600) });
        tokio::select! {
            got = rx.recv() => match got {
                Some(Ok(g)) => return Ok(g),
                Some(Err(r)) => {
                    failures += 1;
                    if r.is_some() { refused = r; }
                    if failures == routes.len() { return Err(refused.unwrap_or(Refusal::Unreachable)); }
                }
                None => return Err(refused.unwrap_or(Refusal::Unreachable)),
            },
            _ = stagger => {}
        }
    }
}

async fn attempt(route: Route) -> Result<Greeted, Option<Refusal>> {
    let mut conn = Conn::dial(&route.url).await.map_err(|_| None)?;
    let first = conn.line(GREETING_TIMEOUT).await.ok_or(None)?;
    match wire::greeting(&first) {
        Some(greeting) => Ok(Greeted { conn, route, greeting }),
        None => Err(wire::offline(&first).map(|e| Refusal::of(Some(&e)))),
    }
}

/// Prove this phone holds its token on a socket that greeted.
pub async fn prove(g: &mut Greeted, device: &str, token: &str) -> Result<wire::Welcome, Refusal> {
    if !g.conn.send(wire::proof_hello(device, &g.greeting.challenge, token)).await {
        return Err(Refusal::Unreachable);
    }
    let line = g.conn.line(WELCOME_TIMEOUT).await.ok_or(Refusal::Unreachable)?;
    let w = wire::welcome(&line).ok_or(Refusal::Unreachable)?;
    if !w.ok {
        return Err(Refusal::of(w.error.as_deref()));
    }
    if w.ops.as_deref().is_some_and(|o| o != wire::OPS_PC) {
        return Err(Refusal::WrongDoor);
    }
    Ok(w)
}

/// Present a pairing code. The code is spent the moment it is sent, so once one route has greeted
/// no other is tried.
pub async fn pair_async(link: &PairLink, device_name: &str) -> Result<Profile, Refusal> {
    let mut g = greet(&link.routes()).await?;
    let out = async {
        if !g.conn.send(wire::pair_hello(&link.code, device_name)).await {
            return Err(Refusal::Unreachable);
        }
        let line = g.conn.line(PAIR_WELCOME_TIMEOUT).await.ok_or(Refusal::of(Some("pair.unconfirmed")))?;
        let w = wire::welcome(&line).ok_or(Refusal::of(Some("pair.unconfirmed")))?;
        if !w.ok {
            return Err(Refusal::of(w.error.as_deref()));
        }
        if w.ops.as_deref().is_some_and(|o| o != wire::OPS_PC) {
            return Err(Refusal::WrongDoor);
        }
        let (Some(device), Some(token)) = (w.device.clone(), w.token.clone()) else {
            return Err(Refusal::of(Some("hello.shape")));
        };
        let gr = g.greeting.admitted_by(&w);
        let or = |a: String, b: &str| if a.is_empty() { b.to_string() } else { a };
        Ok(Profile {
            host_id: or(gr.host_id, &link.host_id),
            name: or(gr.host, &link.host),
            instance: or(gr.instance, &link.instance),
            device,
            token,
            routes: link.routes(),
            last_route: Some(g.route.url.clone()),
            paired_at_ms: now_ms(),
            version: gr.version,
        })
    }
    .await;
    g.conn.close().await;
    out
}

/// [`pair_async`] for a plain thread.
pub fn pair(link: &PairLink, device_name: &str) -> Result<Profile, Refusal> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_| Refusal::Unreachable)?;
    rt.block_on(pair_async(link, device_name))
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

// The session

type Answer = std::sync::mpsc::Sender<Result<Value, BusError>>;

struct Session {
    tx: mpsc::UnboundedSender<String>,
    pending: Mutex<HashMap<String, Answer>>,
    open: AtomicBool,
    ended: Notify,
    /// Tells the writer to close the socket.
    stop: Notify,
}

impl Session {
    fn shut(&self) {
        if self.open.swap(false, Ordering::SeqCst) {
            let down = BusError::link("link.down", "The link to the PC closed");
            for (_, a) in self.pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
                let _ = a.send(Err(down.clone()));
            }
            self.ended.notify_waiters();
            self.ended.notify_one();
            // A permit, kept if the writer is not waiting yet: the socket is always closed.
            self.stop.notify_one();
        }
    }

    fn send(&self, op: &str, payload: &Value, id: &str, answer: Option<Answer>) -> bool {
        if !self.open.load(Ordering::SeqCst) {
            return false;
        }
        if let Some(a) = answer {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), a);
        }
        if self.tx.send(wire::request(op, payload, id)).is_err() {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
            return false;
        }
        true
    }
}

/// An answer on its way. Poll it each frame.
pub struct Call {
    rx: std::sync::mpsc::Receiver<Result<Value, BusError>>,
}

impl Call {
    /// The answer once it has come; None while it is still on its way.
    pub fn poll(&self) -> Option<Result<Value, BusError>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(BusError::link("link.down", "The link to the PC closed"))),
        }
    }

    /// Wait for the answer (tests and worker threads only, never the UI thread).
    pub fn wait(self, timeout: Duration) -> Result<Value, BusError> {
        self.rx.recv_timeout(timeout).unwrap_or_else(|_| Err(BusError::link("link.timeout", "The PC did not answer in time")))
    }

    fn failed(e: BusError) -> Call {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err(e));
        Call { rx }
    }
}

/// Where the link stands, for the drawer's foot and the first screen.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkState {
    /// No PC is paired: the first screen pairs one.
    Unpaired,
    Connecting { attempt: u32 },
    /// Admitted, waiting on the PC's engine (the door is starting it).
    Starting { host: String },
    Online { host: String, route: Route },
    /// Unreachable for now; dials again by itself at `retry_at_ms` (or sooner on [`Link::kick`]).
    Offline { reason: String, retry_at_ms: u64 },
    /// The PC refused this phone's credential: pair again.
    Revoked { reason: String },
}

impl LinkState {
    pub fn online(&self) -> bool {
        matches!(self, LinkState::Online { .. })
    }
}

type Waker = Arc<dyn Fn() + Send + Sync>;

/// What a listener beside the app hears ([`Link::listen`]): every event and every change of
/// state, as they come. The app's own [`Link::take_events`] still gets every event.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    Event(Event),
    State(LinkState),
}

struct Inner {
    store: Arc<dyn Store>,
    state: Mutex<LinkState>,
    session: Mutex<Option<Arc<Session>>>,
    events: Mutex<Vec<Event>>,
    kick: Notify,
    waker: Waker,
    /// Bumped by [`Link::forget`]: a loop of an older generation stops.
    generation: Mutex<u64>,
    listeners: Mutex<Vec<std::sync::mpsc::Sender<Note>>>,
}

impl Inner {
    fn show(&self, s: LinkState) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *st != s {
            *st = s.clone();
            drop(st);
            self.tell(Note::State(s));
            (self.waker)();
        }
    }

    /// Every listener hears it; one that went away is dropped.
    fn tell(&self, n: Note) {
        let mut ls = self.listeners.lock().unwrap_or_else(|e| e.into_inner());
        if !ls.is_empty() {
            ls.retain(|tx| tx.send(n.clone()).is_ok());
        }
    }
}

/// The link. Cheap to clone; one per app.
#[derive(Clone)]
pub struct Link {
    inner: Arc<Inner>,
    handle: tokio::runtime::Handle,
}

impl Link {
    /// Start the link on its own thread. It connects at once when a PC is paired, and waits for
    /// one (a [`Link::kick`] after pairing) when none is.
    pub fn start(store: Arc<dyn Store>, waker: Waker) -> Link {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
        let handle = rt.handle().clone();
        let state = if store.load().is_some() { LinkState::Connecting { attempt: 0 } } else { LinkState::Unpaired };
        let inner = Arc::new(Inner {
            store,
            state: Mutex::new(state),
            session: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            kick: Notify::new(),
            waker,
            generation: Mutex::new(0),
            listeners: Mutex::new(Vec::new()),
        });
        let i2 = inner.clone();
        std::thread::Builder::new()
            .name("rc-link".into())
            .spawn(move || rt.block_on(run(i2)))
            .expect("the link's thread");
        Link { inner, handle }
    }

    pub fn state(&self) -> LinkState {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn profile(&self) -> Option<Profile> {
        self.inner.store.load()
    }

    /// Dial now instead of waiting out the backoff: the app came to the foreground, the network
    /// changed, a PC was just paired.
    pub fn kick(&self) {
        self.inner.kick.notify_one();
    }

    /// A PC was paired: keep it and connect to it now.
    pub fn paired(&self, p: &Profile) {
        self.inner.store.save(p);
        if let Some(s) = self.inner.session.lock().unwrap_or_else(|e| e.into_inner()).take() {
            s.shut();
        }
        self.inner.show(LinkState::Connecting { attempt: 0 });
        self.kick();
    }

    /// Forget the PC (Settings › Unpair): the link waits for the next pairing.
    pub fn forget(&self) {
        *self.inner.generation.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.inner.store.forget();
        if let Some(s) = self.inner.session.lock().unwrap_or_else(|e| e.into_inner()).take() {
            s.shut();
        }
        self.inner.show(LinkState::Unpaired);
        self.kick();
    }

    /// Ask the PC. The answer comes through the [`Call`]; a link that is down answers at once
    /// with `link.down`.
    pub fn call(&self, op: &str, payload: Value) -> Call {
        self.call_with(op, payload, CALL_TIMEOUT)
    }

    pub fn call_with(&self, op: &str, payload: Value, timeout: Duration) -> Call {
        let Some(s) = self.session() else {
            return Call::failed(BusError::link("link.down", "Not connected to the PC"));
        };
        let id = wire::uuid_v4();
        let (tx, rx) = std::sync::mpsc::channel();
        if !s.send(op, &payload, &id, Some(tx)) {
            return Call::failed(BusError::link("link.down", "Not connected to the PC"));
        }
        let (s2, waker, op) = (s.clone(), self.inner.waker.clone(), op.to_string());
        self.handle.spawn(async move {
            tokio::time::sleep(timeout).await;
            let gone = s2.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            if let Some(a) = gone {
                let _ = a.send(Err(BusError::link("link.timeout", &format!("The PC did not answer {op} in time"))));
                waker();
            }
        });
        Call { rx }
    }

    /// Fire and forget, for keys: the answer is read and dropped.
    pub fn post(&self, op: &str, payload: Value) -> bool {
        self.session().is_some_and(|s| s.send(op, &payload, &wire::uuid_v4(), None))
    }

    /// Hear every event and change of state from now on, on another thread than the app's
    /// (the notifications that work with no activity). Dropping the receiver stops it.
    pub fn listen(&self) -> std::sync::mpsc::Receiver<Note> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.listeners.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }

    /// Events since the last take, oldest first.
    pub fn take_events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.inner.events.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn session(&self) -> Option<Arc<Session>> {
        self.inner.session.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|s| s.open.load(Ordering::SeqCst))
    }
}

async fn run(inner: Arc<Inner>) {
    let mut backoff = RECONNECT_MIN;
    let mut attempt = 0u32;
    loop {
        let generation = *inner.generation.lock().unwrap_or_else(|e| e.into_inner());
        let Some(profile) = inner.store.load() else {
            inner.show(LinkState::Unpaired);
            inner.kick.notified().await;
            continue;
        };
        if matches!(&*inner.state.lock().unwrap_or_else(|e| e.into_inner()), LinkState::Revoked { .. }) {
            // A forgotten credential does not get better by dialing again: wait for a new pairing.
            inner.kick.notified().await;
            if inner.store.load().as_ref().map(|p| &p.token) == Some(&profile.token) {
                continue;
            }
        }
        attempt += 1;
        inner.show(LinkState::Connecting { attempt });
        let routes = order_routes(&profile.routes, profile.last_route.as_deref());
        let mut g = match greet(&routes).await {
            Ok(g) => g,
            Err(r) => {
                backoff = wait_out(&inner, &r.text(), backoff).await;
                continue;
            }
        };
        let welcome = match prove(&mut g, &profile.device, &profile.token).await {
            Ok(w) => w,
            Err(r @ (Refusal::Unpaired | Refusal::WrongDoor)) => {
                inner.show(LinkState::Revoked { reason: r.text() });
                continue;
            }
            Err(r) => {
                backoff = wait_out(&inner, &r.text(), backoff).await;
                continue;
            }
        };
        if *inner.generation.lock().unwrap_or_else(|e| e.into_inner()) != generation {
            g.conn.close().await;
            continue;
        }
        let greeting = g.greeting.admitted_by(&welcome);
        let host = if greeting.host.is_empty() { profile.name.clone() } else { greeting.host.clone() };
        // Remember what worked: the route leads next time, and the PC's name.
        let mut p = profile.clone();
        p.last_route = Some(g.route.url.clone());
        if !greeting.host.is_empty() {
            p.name = greeting.host.clone();
        }
        p.version = greeting.version.clone();
        if p != profile {
            inner.store.save(&p);
        }
        let route = g.route.clone();
        let session = open(&inner, g.conn);
        let starting = matches!(greeting.engine.as_deref(), Some("stopped" | "starting"));
        if starting {
            inner.show(LinkState::Starting { host: host.clone() });
        }
        // The first request waits for the engine when the door had to start it.
        let ping = ask(&session, "bus.ping", json!({}), if starting { ENGINE_START_TIMEOUT } else { CALL_TIMEOUT }).await;
        if ping.is_err() {
            session.shut();
            backoff = wait_out(&inner, "The PC's engine did not answer", backoff).await;
            continue;
        }
        let events: Vec<Value> = SUBSCRIBE.iter().map(|e| json!(e)).collect();
        let _ = ask(&session, "bus.subscribe", json!({ "events": events }), CALL_TIMEOUT).await;
        *inner.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session.clone());
        inner.show(LinkState::Online { host, route });
        (inner.waker)();
        let began = tokio::time::Instant::now();
        keep_alive(&inner, &session).await;
        session.shut();
        inner.session.lock().unwrap_or_else(|e| e.into_inner()).take();
        if began.elapsed() < STABLE {
            backoff = wait_out(&inner, "Connection lost, reconnecting", backoff).await;
        } else {
            backoff = RECONNECT_MIN;
            attempt = 0;
        }
    }
}

/// Split the socket: a writer task for the lines out, a reader task that answers calls and
/// keeps events. Either ending shuts the session.
fn open(inner: &Arc<Inner>, conn: Conn) -> Arc<Session> {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let session = Arc::new(Session { tx, pending: Mutex::new(HashMap::new()), open: AtomicBool::new(true), ended: Notify::new(), stop: Notify::new() });
    let Conn { ws, buf } = conn;
    let (mut sink, mut stream) = ws.split();
    let s = session.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(line) => if sink.send(Message::Text(line.into())).await.is_err() { break },
                    None => break,
                },
                _ = s.stop.notified() => break,
            }
        }
        s.shut();
        // Closed for good: the door sees the phone leave instead of holding a dead connection.
        let _ = tokio::time::timeout(Duration::from_secs(2), sink.close()).await;
    });
    let (s, inner) = (session.clone(), inner.clone());
    tokio::spawn(async move {
        for l in buf {
            dispatch(&inner, &s, &l);
        }
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(t) => {
                    for l in t.as_str().split('\n').filter(|l| !l.trim().is_empty()) {
                        dispatch(&inner, &s, l);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
            if !s.open.load(Ordering::SeqCst) {
                break;
            }
        }
        s.shut();
        (inner.waker)();
    });
    session
}

fn dispatch(inner: &Inner, s: &Session, line: &str) {
    match wire::classify(line) {
        Some(Incoming::Res(r)) => {
            let Some(id) = r.id else { return };
            let a = s.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            if let Some(a) = a {
                let out = if r.ok {
                    Ok(r.result)
                } else {
                    Err(r.error.unwrap_or_else(|| BusError { kind: "internal".into(), code: "bus.no_error".into(), message: "The PC refused without saying why".into(), hint: None }))
                };
                let _ = a.send(out);
                (inner.waker)();
            }
        }
        Some(Incoming::Ev(e)) => {
            let mut ev = inner.events.lock().unwrap_or_else(|e| e.into_inner());
            // A phone that stopped drawing (asleep) keeps the newest 2000.
            if ev.len() >= 2000 {
                ev.remove(0);
            }
            ev.push(e.clone());
            drop(ev);
            inner.tell(Note::Event(e));
            (inner.waker)();
        }
        _ => {}
    }
}

/// A call from inside the link's own thread.
async fn ask(s: &Arc<Session>, op: &str, payload: Value, timeout: Duration) -> Result<Value, BusError> {
    let id = wire::uuid_v4();
    let (tx, rx) = std::sync::mpsc::channel();
    if !s.send(op, &payload, &id, Some(tx)) {
        return Err(BusError::link("link.down", "Not connected to the PC"));
    }
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match rx.try_recv() {
            Ok(r) => return r,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(BusError::link("link.down", "The link to the PC closed")),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if tokio::time::Instant::now() >= deadline || !s.open.load(Ordering::SeqCst) {
            s.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(BusError::link("link.timeout", &format!("The PC did not answer {op} in time")));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Ping until the session ends; two missed pings in a row mean the path is dead. A kick (the app
/// came back, the network changed) pings at once, so a dead path is found in seconds.
async fn keep_alive(inner: &Inner, s: &Arc<Session>) {
    let mut missed = 0;
    while s.open.load(Ordering::SeqCst) {
        tokio::select! {
            _ = s.ended.notified() => break,
            _ = inner.kick.notified() => {}
            _ = tokio::time::sleep(KEEPALIVE) => {}
        }
        if !s.open.load(Ordering::SeqCst) {
            break;
        }
        missed = if ask(s, "bus.ping", json!({}), PING_TIMEOUT).await.is_ok() { 0 } else { missed + 1 };
        if missed >= 2 {
            break;
        }
    }
}

async fn wait_out(inner: &Inner, reason: &str, backoff: Duration) -> Duration {
    inner.show(LinkState::Offline { reason: reason.to_string(), retry_at_ms: now_ms() + backoff.as_millis() as u64 });
    tokio::select! {
        _ = inner.kick.notified() => {}
        _ = tokio::time::sleep(backoff) => {}
    }
    (backoff * 2).min(RECONNECT_MAX)
}
