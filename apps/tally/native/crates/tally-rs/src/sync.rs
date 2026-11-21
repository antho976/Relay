//! Pairing with the owner's server and syncing the ledger through it (Kotlin: link/, data/sync,
//! and the Sync page of ui/settings).
//!
//! The contract the other screens use: [`State::request`] sends one op to the server's Relay off
//! the UI thread and [`State::reply`] is asked each frame until it answers.
//!
//! How it runs: every network call is on a thread of its own ([`crate::sync_net`] blocks), and
//! hands its result back over a channel that [`tick`] reads at the top of each frame. The ledger is
//! only ever touched here, on the UI thread: the rows to send are read when a sync starts, and the
//! server's are applied when its answer arrives ([`crate::sync_ledger`]).
//!
//! When it syncs: on start, a few seconds after a change here (a burst of edits is one sync),
//! every few minutes while the app is open, and from Sync now. The first sync takes the server's
//! ledger in place of this phone's and sends nothing; it never replaces the server's
//! (`sync_ledger` says how that is made impossible).
//!
//! Unlike the Kotlin app, Tally RS never asks the server to let it in by itself: asking DMs the
//! owner, so it waits for the person to tap Connect (here or on the first-run page). A request
//! already in flight when the app closed is resumed, which sends nothing new.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, KeyKind, PAD};
use crate::model::Store;
use crate::sync_ledger::{self as ledger, Answer, Ask, Mark};
use crate::sync_net::{self as net, Creds, Enrollment, Net, Requester, code};
use crate::settings_kit as sk;
use crate::sync_wire as wire;
use crate::widgets;

/// Why a request to the server failed: Relay's error code ("bus.unknown_op", "remote.op") and a
/// message to show as is.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub code: String,
    pub message: String,
}

impl Failure {
    pub fn not_paired() -> Failure {
        Failure { code: NOT_PAIRED.into(), message: "This phone is not connected to your server yet. Connect it in Settings, Sync.".into() }
    }
}

/// The code of a request made before this phone is paired.
pub const NOT_PAIRED: &str = "tally.not_paired";

/// Where a request stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Pending,
    Done(Result<Value, Failure>),
    /// No request has that ticket (already collected, or never made).
    Unknown,
}

/// A burst of edits settles into one sync.
const DEBOUNCE_NS: u64 = 3_000_000_000;
/// While the app is open, a sync this often.
const PERIOD_NS: u64 = 5 * 60 * 1_000_000_000;
/// The same, by the wall clock: the monotonic clock stops while the phone sleeps, so a return
/// after a night asleep would otherwise wait five more awake minutes for its sync.
const PERIOD_MS: i64 = 5 * 60 * 1_000;
/// A screen's request waits this long for its answer.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Ops no screen may send through [`State::request`]: the ones that replace or erase the server's
/// ledger. Sync's own `money.sync` goes only through `sync_ledger`, which cannot ask a replace.
const REFUSED_OPS: &[&str] = &["money.sync", "money.reset", "money.import", "money.sample"];

/// Where connecting this phone to the owner's server stands (Kotlin: `Connect`).
#[derive(Clone, Debug, PartialEq)]
pub enum Connect {
    /// Not connected, and not asking.
    Idle,
    Reaching,
    /// Asked; waiting for the owner's Allow on Discord (or the dashboard when not `notified`).
    Waiting { code: String, notified: bool, expires_at: i64, server: String },
    Pairing,
    Connected,
    Denied,
    /// The request ran out before anyone answered.
    Expired,
    Trouble { reason: String, cloud_off: bool },
}

/// How a status reads: Relay's lamps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Live,
    Waiting,
    Held,
    Off,
}

impl Tone {
    pub fn colour(self, p: &crate::theme::Palette) -> Rgba {
        match self {
            Tone::Live => p.live,
            Tone::Waiting => p.waiting,
            Tone::Held => p.held,
            Tone::Off => p.ink3,
        }
    }
}

/// One line about sync, as the Sync page and the first-run page say it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
    pub detail: Option<String>,
}

fn line(text: impl Into<String>, tone: Tone, detail: Option<&str>) -> Line {
    Line { text: text.into(), tone, detail: detail.map(str::to_string) }
}

/// What the threads hand back. Each but a screen's reply carries the attempt it belongs to (a
/// connect thread's [`State::attempt`], a sync's number): one that has been cancelled, or that a
/// disconnect, a new address or a ledger replaced here has left behind, is dropped unread.
enum Event {
    Connect(u64, Connect),
    Pending(u64, Option<Enrollment>),
    /// The key, and the server address it was asked of.
    Paired(u64, Creds, String),
    Synced(u64, Result<Answer, Failure>, Mark),
    Replied(u64, Result<Value, Failure>),
}

/// What the Sync page asks before it acts.
#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    Take,
    Disconnect,
    /// The first pull would replace what this phone holds.
    First,
}

/// The question a first pull asks when this phone already holds a ledger (the Sync page and the
/// first-run page put it).
pub const FIRST_TITLE: &str = "Replace this phone's ledger?";
pub const FIRST_BODY: &str = "Its first sync puts your server's ledger in place of what is here; Tally keeps a copy of this phone's first. Nothing here is sent to your server. Cancel turns sync off and keeps this phone as it is.";
pub const FIRST_YES: &str = "Take it";
/// The same question when entries were made here while the server's ledger was on its way.
pub const CHANGED_BODY: &str = "Entries were made on this phone while your server's ledger was on its way. Taking it puts your server's ledger in place of what is here, those entries included; Tally keeps a copy of this phone's first. Nothing here is sent to your server. Cancel turns sync off and keeps this phone as it is.";
/// The same question when the server's ledger was replaced and changes here never went to it.
pub const STALE_BODY: &str = "Your server's ledger was replaced since this phone last synced, and changes made here never went to it. Taking it puts your server's ledger in place of what is here, those changes included; Tally keeps a copy of this phone's first. Nothing here is sent to your server. Cancel turns sync off and keeps this phone as it is.";
/// The same question when the server's ledger, replaced since, holds far fewer accounts and
/// entries than this phone (`there` against `here`): it may have been lost there.
pub fn fewer_body(there: usize, here: usize) -> String {
    format!(
        "Your server's ledger has far fewer accounts and entries than this phone ({there} against {here}); it may have been lost or erased there. Taking it puts your server's ledger in place of what is here; Tally keeps a copy of this phone's first. Nothing here is sent to your server. Cancel turns sync off and keeps this phone as it is."
    )
}
/// What "Take the server's ledger" asks, with sync on and with it off.
const TAKE_BODY: &str = "This phone's ledger becomes exactly the server's copy. Anything only on this phone goes.";
const TAKE_BODY_OFF: &str = "Sync turns on, and this phone's ledger becomes exactly the server's copy. Anything only on this phone goes.";

/// This phone's key to the server, apart from the prefs: its own file, readable by this app
/// only, never logged.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Secret {
    creds: Option<Creds>,
    pending: Option<Enrollment>,
    /// A key a connect thread paired after its App went away, and the server it asked: the next
    /// [`State::start`] takes it as that pairing's news ([`Handoff`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handed_on: Option<(Creds, String)>,
}

/// What a connect thread whose App went away (Android destroyed the activity) leaves to the next
/// App: it may already be pairing, and pairing spends the link and makes a device on the server,
/// so its key must not be thrown away. Process-wide, as the next App in this process starts while
/// the old thread runs; kept by data folder (tests run side by side, each with its own).
struct Handoff {
    /// The folders a connect thread is pairing for right now: another for the same folder waits
    /// rather than make a second device.
    pairing: Vec<Option<PathBuf>>,
    /// Keys paired after their App went away, and the server asked, until the next State with that
    /// folder takes them.
    paired: Vec<(Option<PathBuf>, Creds, String)>,
}

static HANDOFF: std::sync::Mutex<Handoff> = std::sync::Mutex::new(Handoff { pairing: Vec::new(), paired: Vec::new() });

fn handoff() -> std::sync::MutexGuard<'static, Handoff> {
    HANDOFF.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps a key paired after its App went away, for the next App: in [`HANDOFF`] for one in this
/// process, and in the secret file for one in the next (the process may be stopped first).
fn hand_on(h: &mut Handoff, dir: &Option<PathBuf>, creds: Creds, server: String) {
    h.paired.retain(|(d, ..)| d != dir);
    h.paired.push((dir.clone(), creds.clone(), server.clone()));
    let Some(dir) = dir else { return };
    let path = dir.join(SECRET_FILE);
    let mut secret: Secret = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // The request it answered is spent.
    secret.pending = None;
    secret.handed_on = Some((creds, server));
    let _ = write_private(&path, &serde_json::to_string(&secret).unwrap_or_default());
}

/// A connect thread pairing for a folder ([`Handoff::pairing`]), until it drops.
struct Pairing(Option<PathBuf>);

impl Pairing {
    /// None while another connect thread pairs for the same folder, or has paired and handed its
    /// key on: the key comes to this App ([`State::pump`]), and pairing here too would make a
    /// second device on the server.
    fn begin(dir: &Option<PathBuf>) -> Option<Pairing> {
        let mut h = handoff();
        if h.pairing.contains(dir) || h.paired.iter().any(|(d, ..)| d == dir) {
            return None;
        }
        h.pairing.push(dir.clone());
        Some(Pairing(dir.clone()))
    }
}

impl Drop for Pairing {
    fn drop(&mut self) {
        let mut h = handoff();
        if let Some(i) = h.pairing.iter().position(|d| *d == self.0) {
            h.pairing.remove(i);
        }
    }
}

/// How a connect thread is told to stop: `cancel` by a disconnect or a new address (what it
/// still learns is dropped), and `gone` too when its App went away (a key it pairs is handed on).
#[derive(Default)]
struct Stop {
    cancel: AtomicBool,
    gone: AtomicBool,
}

const SECRET_FILE: &str = "sync-secret.json";

pub mod pref {
    pub const SERVER: &str = "sync.server";
    pub const PHONE_ONLY: &str = "sync.phone_only";
    pub const UNPAIRED: &str = "sync.unpaired";
    pub const LAST_ATTEMPT: &str = "sync.last_attempt";
    pub const LAST_SUCCESS: &str = "sync.last_success";
    pub const LAST_ERROR: &str = "sync.last_error";
    pub const SENT: &str = "sync.sent";
    pub const RECEIVED: &str = "sync.received";
    pub const NOTE: &str = "sync.note";
    /// The address of the server where sync stands (`sync_ledger`'s prefs) were made with: kept
    /// through a disconnect, so pairing with it again goes on from there.
    pub const PAIRED_SERVER: &str = "sync.paired_server";
}

pub struct State {
    net: Net,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    started: bool,
    dir: Option<PathBuf>,
    creds: Option<Creds>,
    pending: Option<Enrollment>,
    pub connect: Connect,
    cancel: Option<Arc<Stop>>,
    /// The connect thread whose news is taken; any other's is dropped.
    attempt: u64,
    /// A sync is running: its number.
    in_flight: Option<u64>,
    syncs: u64,
    /// A pull of the server's whole ledger waits on the person's yes: it would replace what this
    /// phone holds.
    ask_first: bool,
    /// Why: [`FIRST_BODY`], [`CHANGED_BODY`], [`STALE_BODY`] or [`fewer_body`].
    ask_body: String,
    /// The person asked for the server's whole ledger.
    take_whole: bool,
    /// A whole ledger was taken here since the app last asked ([`State::took_ledger`]).
    took_ledger: bool,
    /// A sync brought rows from the server since the app last asked ([`State::received_rows`]).
    received_rows: bool,
    /// The sync running is the one they asked for.
    whole_asked: bool,
    /// A yes in `take_whole` that outlived a sync that failed, and where this phone's ledger stood
    /// when it was first acted on: it waits for the next sync due by itself (or Sync now), not
    /// the next frame, and a ledger changed here since is asked about again.
    whole_mark: Option<Mark>,
    sync_asked: bool,
    dirty_at: Option<u64>,
    last_try_ns: Option<u64>,
    /// When the last sync was tried, by the wall clock ([`PERIOD_MS`]).
    last_try_ms: Option<i64>,
    /// The ledger writes being made are the server's, not edits here.
    applying: bool,
    /// Paired and sync not turned off, as the last [`tick`] found it: a sync may start by itself.
    on: bool,
    inbox: Vec<Event>,
    next_ticket: u64,
    answered: HashMap<u64, Result<Value, Failure>>,
    waiting: HashSet<u64>,
    // The Sync page
    address: Option<String>,
    confirm: Option<Confirm>,
}

impl Default for State {
    fn default() -> State {
        State::with_net(Net::real())
    }
}

/// The app going away (Android destroys the activity, and starts it again with an App of its
/// own) cancels a connect thread still waiting on the owner's Allow: left running, it would pair,
/// spending the link, and its key would go to a channel nobody reads, so the new App's resumed
/// request ([`State::start`] reads the pending one from the secret file) would find the link
/// spent. The new App's own connect thread pairs instead. One already pairing has spent the link:
/// its key is handed on to the next App ([`hand_on`]), as is one it sent that no step took yet.
impl Drop for State {
    fn drop(&mut self) {
        let Some(c) = &self.cancel else { return };
        // Under the hand-off's lock, as the thread decides where its key goes: it finds the App
        // gone, or its key is in the channel here (never neither).
        let mut h = handoff();
        c.gone.store(true, Ordering::SeqCst);
        c.cancel.store(true, Ordering::SeqCst);
        let mut sent: Vec<Event> = std::mem::take(&mut self.inbox);
        sent.extend(self.rx.try_iter());
        for e in sent {
            if let Event::Paired(a, creds, server) = e
                && a == self.attempt
            {
                hand_on(&mut h, &self.dir, creds, server);
            }
        }
    }
}

impl State {
    pub fn with_net(net: Net) -> State {
        let (tx, rx) = channel();
        State {
            net,
            tx,
            rx,
            started: false,
            dir: None,
            creds: None,
            pending: None,
            connect: Connect::Idle,
            cancel: None,
            attempt: 0,
            in_flight: None,
            syncs: 0,
            ask_first: false,
            ask_body: FIRST_BODY.to_string(),
            take_whole: false,
            took_ledger: false,
            received_rows: false,
            whole_asked: false,
            whole_mark: None,
            sync_asked: false,
            dirty_at: None,
            last_try_ns: None,
            last_try_ms: None,
            applying: false,
            on: false,
            inbox: Vec::new(),
            next_ticket: 0,
            answered: HashMap::new(),
            waiting: HashSet::new(),
            address: None,
            confirm: None,
        }
    }

    /// True once this phone is paired with the owner's server.
    pub fn paired(&self, store: &Store) -> bool {
        self.creds.is_some() && store.pref(pref::UNPAIRED).is_none()
    }

    /// Send `op` with `payload` to the server's Relay. Returns a ticket for [`Self::reply`].
    /// Never blocks: the network runs off the UI thread.
    pub fn request(&mut self, store: &Store, op: &str, payload: Value) -> u64 {
        self.next_ticket += 1;
        let ticket = self.next_ticket;
        if refused(op, &payload) {
            self.answered.insert(ticket, Err(net::fail(code::REFUSED, "Tally RS does not send that to your server")));
            return ticket;
        }
        let Some(creds) = self.creds.clone().filter(|_| self.paired(store)) else {
            self.answered.insert(ticket, Err(Failure::not_paired()));
            return ticket;
        };
        self.waiting.insert(ticket);
        let (dial, tx, op) = (self.net.dial.clone(), self.tx.clone(), op.to_string());
        std::thread::spawn(move || {
            let result = net::open(dial.as_ref(), &creds).and_then(|mut s| s.request(&op, &payload, CALL_TIMEOUT));
            let _ = tx.send(Event::Replied(ticket, result));
        });
        ticket
    }

    /// The answer to a ticket, once; `Pending` until it has come. Ask each frame while waiting
    /// (a screen waiting on a reply returns true from `draw` so frames keep coming).
    pub fn reply(&mut self, ticket: u64) -> Reply {
        self.pump();
        match self.answered.remove(&ticket) {
            Some(r) => Reply::Done(r),
            None if self.waiting.contains(&ticket) => Reply::Pending,
            None => Reply::Unknown,
        }
    }

    /// A pairing or a sync is talking to the server.
    pub fn busy(&self) -> bool {
        self.in_flight.is_some() || matches!(self.connect, Connect::Reaching | Connect::Pairing)
    }

    /// Something is on its way back: keep frames coming to collect it.
    pub fn working(&self) -> bool {
        self.busy() || matches!(self.connect, Connect::Waiting { .. }) || !self.waiting.is_empty()
    }

    /// When the app should next draw for sync's sake with no input (`Next::At`): soon while
    /// something runs in the background, so its answer is applied; then when the debounce after a
    /// change ends, and when the periodic sync is due. None when no sync can start by itself: not
    /// connected, sync turned off, or a pull waiting on the person's yes. The platform loops read
    /// a time already past as draw now, so one that no sync would follow would redraw for good.
    pub fn wake_at(&self, now_ns: u64) -> Option<u64> {
        if self.working() {
            return Some(now_ns + 250_000_000);
        }
        if !self.on || self.ask_first {
            return None;
        }
        if self.sync_asked || (self.take_whole && self.whole_mark.is_none()) || self.last_try_ns.is_none() {
            // Due at once: the next frame's tick starts it.
            return Some(now_ns);
        }
        let debounce = self.dirty_at.map(|d| d + DEBOUNCE_NS);
        // Whichever clock says the period ends first: the wall clock went on while the phone slept.
        let wall = self.last_try_ms.map(|t| now_ns + (PERIOD_MS - (now_ms() - t)).max(0) as u64 * 1_000_000);
        let period = match (self.last_try_ns.map(|t| t + PERIOD_NS), wall) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let at = match (debounce, period) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b)?,
        };
        // Past already only when tick has not seen it yet: one frame starts it.
        Some(at.max(now_ns))
    }

    /// Sync now (the button), or connect when not connected yet.
    pub fn sync_now(&mut self) {
        self.sync_asked = true;
    }

    /// The server this phone talks to: the person's address, or the one this build was made for.
    pub fn server(store: &Store) -> String {
        wire::normalize(&store.pref(pref::SERVER).unwrap_or_else(|| wire::DEFAULT_SERVER.to_string()))
    }

    /// Ask the server to let this phone in: the owner gets a DM with Allow. Only ever from a tap.
    pub fn connect_now(&mut self, store: &mut Store) {
        if self.cancel.is_some() && matches!(self.connect, Connect::Reaching | Connect::Waiting { .. } | Connect::Pairing) {
            return;
        }
        let _ = store.clear_pref(pref::PHONE_ONLY);
        let server = State::server(store);
        let pending = self.pending.clone().filter(|e| wire::normalize(&e.server) == server && e.expires_at > now_ms());
        self.start_connect(server, pending);
    }

    fn start_connect(&mut self, server: String, pending: Option<Enrollment>) {
        self.stop_connect();
        let stop = Arc::new(Stop::default());
        self.cancel = Some(stop.clone());
        self.connect = Connect::Reaching;
        let (net, tx, attempt, dir) = (self.net.clone(), self.tx.clone(), self.attempt, self.dir.clone());
        std::thread::spawn(move || {
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_connect(&net, &tx, &stop, &dir, attempt, &server, pending, &device_name())));
            // A fault in there must not leave the page waiting for good.
            if ran.is_err() && !stop.cancel.load(Ordering::Relaxed) {
                let _ = tx.send(Event::Connect(attempt, Connect::Trouble { reason: "Connecting stopped on a fault. Ask again.".into(), cloud_off: false }));
            }
        });
    }

    /// Cancels the connect thread, if one runs; whatever it still sends is dropped.
    fn stop_connect(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.cancel.store(true, Ordering::SeqCst);
        }
        self.attempt += 1;
    }

    /// Drops the sync running, if one is: its answer is not applied when it comes.
    fn drop_sync(&mut self) {
        self.in_flight = None;
        self.whole_asked = false;
    }

    /// This phone's ledger was replaced here (Erase everything, a restore, the sample). The next
    /// sync takes the server's ledger whole: a replaced ledger holds a tombstone for every row it
    /// had, and sending those would erase the server's. A sync already running is dropped.
    pub fn ledger_replaced(&mut self, store: &mut Store) {
        self.drop_sync();
        self.take_whole = false;
        self.ask_first = false;
        ledger::forget(store);
    }

    /// The sample is in: it stays on this phone, so sync turns off (connecting again turns it on,
    /// and the first sync then asks before it replaces the sample).
    pub fn keep_on_phone(&mut self, store: &mut Store) {
        if !self.paired(store) {
            self.stop_connect();
            self.connect = Connect::Idle;
        }
        self.ledger_replaced(store);
        let _ = store.set_pref(pref::PHONE_ONLY, "1");
        self.on = false;
    }

    /// A pull of the server's whole ledger is waiting on the person's yes ([`FIRST_BODY`]).
    pub fn awaiting_yes(&self) -> bool {
        self.ask_first
    }

    /// What the question while [`awaiting_yes`](Self::awaiting_yes) says.
    pub fn question(&self) -> &str {
        &self.ask_body
    }

    fn ask(&mut self, body: impl Into<String>) {
        self.ask_first = true;
        self.ask_body = body.into();
    }

    /// Whether a sync took a whole ledger in place of this phone's since the last call: the bills
    /// it brings that are due today have not been posted here yet (`App::frame`).
    pub fn took_ledger(&mut self) -> bool {
        std::mem::take(&mut self.took_ledger)
    }

    /// Whether a sync brought any rows since the last call: a bill among them can be due today
    /// (new, resumed or moved), and has not been posted here yet (`App::frame`).
    pub fn received_rows(&mut self) -> bool {
        std::mem::take(&mut self.received_rows)
    }

    /// The person's answer to [`FIRST_BODY`]: take the server's ledger, or turn sync off and keep
    /// this phone's.
    pub fn answer_first(&mut self, store: &mut Store, take: bool) {
        self.ask_first = false;
        if take {
            self.take_whole = true;
            self.whole_mark = None;
        } else {
            let _ = store.set_pref(pref::PHONE_ONLY, "1");
            self.on = false;
            self.take_whole = false;
        }
    }

    /// The page's "Sync with my server". Turned off, a sync running is dropped and a pull of the
    /// server's whole ledger the person said yes to is dropped with it: their yes was for this
    /// phone's ledger as it stood then, and entries made while sync is off would go with it,
    /// unasked, once it is on again. Turned on, it syncs (connecting asks nothing by itself).
    pub(crate) fn set_sync(&mut self, store: &mut Store, on: bool) {
        if on {
            let _ = store.clear_pref(pref::PHONE_ONLY);
            self.sync_now();
        } else {
            self.stop_connect();
            // A sync running now is not applied when it answers: sync is off.
            self.drop_sync();
            self.take_whole = false;
            self.connect = Connect::Idle;
            let _ = store.set_pref(pref::PHONE_ONLY, "1");
        }
    }

    /// The page's "Take the server's ledger", said yes to. Sync turns on first if it was off: off,
    /// the next frame would drop the yes ([`step_held`]), and taking the server's ledger is
    /// asking for sync.
    fn take(&mut self, store: &mut Store) {
        if store.pref(pref::PHONE_ONLY).is_some() {
            self.set_sync(store, true);
        }
        self.take_whole = true;
        self.whole_mark = None;
    }

    /// A sync they asked to take the server's ledger failed (offline, a timeout, the server
    /// down): their yes stands, or the next sync would be an ordinary merge that sends what they
    /// chose to drop. It goes with the next sync due by itself, from `mark`, so a phone offline
    /// does not try again every frame. Sync turned off, a disconnect, or a No drops it.
    fn keep_yes(&mut self, mark: Mark) {
        self.take_whole = true;
        self.whole_mark = Some(mark);
    }

    /// Whether a thread sent something [`tick`] has not taken yet ([`has_news`]).
    fn news(&mut self) -> bool {
        self.pump();
        !self.inbox.is_empty()
    }

    /// Takes what the threads sent: screens' answers at once, the rest for [`tick`]. A key an
    /// earlier App's connect thread paired after it went away comes as this App's pairing.
    fn pump(&mut self) {
        if self.started
            && let Some((creds, server)) = take_handed_on(&self.dir)
        {
            // This App's own connect, resuming the same request, would find it spent.
            self.stop_connect();
            self.pending = None;
            self.inbox.push(Event::Paired(self.attempt, creds, server));
        }
        while let Ok(e) = self.rx.try_recv() {
            match e {
                Event::Replied(ticket, r) => {
                    if self.waiting.remove(&ticket) {
                        self.answered.insert(ticket, r);
                    }
                }
                other => self.inbox.push(other),
            }
        }
    }

    fn save_secret(&self, app_log: &dyn Fn(&str)) {
        let Some(dir) = &self.dir else { return };
        // Under the hand-off's lock: a key handed on and not taken yet stays in the file, and a
        // gone App's thread does not write it at the same time.
        let h = handoff();
        let handed_on = h.paired.iter().find(|(d, ..)| d.as_ref() == Some(dir)).map(|(_, c, s)| (c.clone(), s.clone()));
        let s = Secret { creds: self.creds.clone(), pending: self.pending.clone(), handed_on };
        if let Err(e) = write_private(&dir.join(SECRET_FILE), &serde_json::to_string(&s).unwrap_or_default()) {
            app_log(&format!("keeping the sync key: {e}"));
        }
    }

    fn start(&mut self, dir: PathBuf, store: &mut Store) {
        self.started = true;
        let secret: Secret = std::fs::read_to_string(dir.join(SECRET_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        // A key an earlier App's connect thread paired after it went away: this process's, or one
        // kept in the file by a process stopped since.
        let handed_on = take_handed_on(&Some(dir.clone())).or(secret.handed_on);
        self.dir = Some(dir);
        self.creds = secret.creds;
        // The request that key answered is spent: not resumed.
        self.pending = secret.pending.filter(|_| handed_on.is_none());
        if let Some((creds, server)) = handed_on {
            self.inbox.push(Event::Paired(self.attempt, creds, server));
        }
        demo(self, store);
        if self.creds.is_some() {
            self.connect = Connect::Connected;
            // A pairing made before the address was kept: it is the address set now (a new one
            // disconnects first).
            if store.pref(pref::PAIRED_SERVER).is_none() {
                let _ = store.set_pref(pref::PAIRED_SERVER, &State::server(store));
            }
        }
        // Not paired, or paired with a key the server has since forgotten (connecting again):
        // the owner may have tapped Allow while the app was closed, so collect it.
        if !self.paired(store)
            && let Some(e) = self.pending.clone().filter(|e| e.expires_at > now_ms() && store.pref(pref::PHONE_ONLY).is_none())
        {
            self.start_connect(e.server.clone(), Some(e));
        }
    }

    fn start_sync(&mut self, store: &mut Store, now_ns: u64) {
        let Some(creds) = self.creds.clone() else { return };
        self.last_try_ns = Some(now_ns);
        self.last_try_ms = Some(now_ms());
        self.dirty_at = None;
        self.sync_asked = false;
        self.whole_asked = std::mem::take(&mut self.take_whole);
        let kept = self.whole_mark.take();
        // Where this phone's ledger stands as the sync starts (or stood when a yes kept through a
        // failed sync was first acted on): a whole answer is not taken over entries made after it.
        let asked = match (self.whole_asked, kept) {
            (true, Some(m)) => Ok((Ask::Whole, m)),
            (true, None) => Mark::now(store).map(|m| (Ask::Whole, m)),
            (false, _) => ledger::next_ask(store),
        };
        let (ask, at) = match asked {
            Ok(a) => a,
            Err(f) => {
                record_failure(store, &f.message);
                return;
            }
        };
        // A pull nobody asked for (a first pairing, a ledger replaced here) replaces this phone's
        // ledger: when it holds anything, the person says so first.
        // Categories the person changed count, as relay-money's own check leaves them out.
        if matches!(ask, Ask::Whole) && !self.whole_asked && ledger::holds_own(store) {
            self.ask(FIRST_BODY);
            return;
        }
        let _ = store.set_pref(pref::LAST_ATTEMPT, &now_ms().to_string());
        self.syncs += 1;
        let id = self.syncs;
        self.in_flight = Some(id);
        let (dial, tx) = (self.net.dial.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| net::open(dial.as_ref(), &creds).and_then(|mut s| ledger::exchange(&mut s, &device_name(), ask))))
                .unwrap_or_else(|_| Err(net::fail(code::ANSWER, "The sync stopped on a fault. It goes again by itself.")));
            let _ = tx.send(Event::Synced(id, result, at));
            // A loop asleep in the background (Android after Pause) takes the answer now, so a
            // follow-up for edits made while this ran can start before the process is stopped.
            wake();
        });
    }

    /// Forget the server's key. This phone's ledger stays as it is, and so does where sync stood
    /// with that server ([`pref::PAIRED_SERVER`]): connecting to it again goes on from there, and
    /// connecting to another starts from nothing. A connect or a sync still running is dropped.
    fn disconnect(&mut self, store: &mut Store) {
        self.stop_connect();
        self.drop_sync();
        self.take_whole = false;
        self.ask_first = false;
        self.creds = None;
        self.pending = None;
        self.connect = Connect::Idle;
        self.on = false;
        store.holding_prefs(|store| {
            for k in [pref::UNPAIRED, pref::LAST_ATTEMPT, pref::LAST_SUCCESS, pref::LAST_ERROR, pref::SENT, pref::RECEIVED, pref::NOTE] {
                let _ = store.clear_pref(k);
            }
        });
    }

    /// Where this phone stands with the owner's server, in the fewest words (Kotlin:
    /// shell/SyncText.kt `syncLine`). A sync that failed after earlier ones worked reads as
    /// waiting, not broken: it goes again by itself.
    pub fn line(&self, store: &Store, now: i64) -> Line {
        if store.pref(pref::PHONE_ONLY).is_some() {
            return line("On this phone only", Tone::Off, Some("Sync is off. Turn it on in Settings, Sync."));
        }
        if !self.paired(store) {
            // Not paired with a key kept: the server forgot this phone. Read from the key, not
            // from how connecting stands, which sync turned off and on again leaves Idle; a
            // connect running again says its own news.
            if self.creds.is_some() && matches!(self.connect, Connect::Connected | Connect::Idle) {
                return line("Your server no longer knows this phone", Tone::Held, Some("Connect it again below."));
            }
            return match &self.connect {
                Connect::Waiting { code, notified: true, .. } => line(format!("Allow on Discord · {code}"), Tone::Waiting, Some(&format!("Your bot sent you a message. Tap Allow if it shows {code}."))),
                Connect::Waiting { code, .. } => line(
                    format!("Allow on your dashboard · {code}"),
                    Tone::Waiting,
                    Some(&format!("Open your bot's dashboard, Tally sync, and allow the request that shows {code}.")),
                ),
                Connect::Reaching => line("Reaching your server", Tone::Waiting, None),
                Connect::Pairing => line("Connecting", Tone::Waiting, None),
                Connect::Denied => line("Your server declined this phone", Tone::Held, Some("Ask again from Settings, Sync.")),
                Connect::Expired => line("Request ran out · ask again", Tone::Held, Some("Nobody answered in 15 minutes. Ask again from Settings, Sync.")),
                Connect::Trouble { reason, cloud_off } => line(reason.clone(), if *cloud_off { Tone::Held } else { Tone::Waiting }, None),
                Connect::Connected | Connect::Idle => line("Not connected yet", Tone::Off, None),
            };
        }
        if self.in_flight.is_some() {
            return line("Syncing", Tone::Waiting, None);
        }
        if self.ask_first {
            let what = if self.ask_body == FIRST_BODY { "The first sync replaces" } else { "Taking your server's ledger replaces" };
            return line("Waiting for your yes", Tone::Waiting, Some(&format!("{what} this phone's ledger with your server's. Answer on the Your server page.")));
        }
        let num = |k: &str| store.pref(k).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let (last, attempt) = (num(pref::LAST_SUCCESS), num(pref::LAST_ATTEMPT));
        match store.pref(pref::LAST_ERROR) {
            Some(e) if attempt >= last => {
                let text = if last > 0 { format!("Offline · synced {}", ago(last, now)) } else { "Waiting for your server".into() };
                line(text, Tone::Waiting, Some(&e))
            }
            _ if last > 0 => line(format!("Synced · {}", ago(last, now)), Tone::Live, store.pref(pref::NOTE).as_deref()),
            _ => line("Connected", Tone::Live, None),
        }
    }
}

/// Whether a screen's request is one Tally RS never sends: an op that replaces or erases the
/// server's ledger, or any payload asking a replace.
fn refused(op: &str, payload: &Value) -> bool {
    REFUSED_OPS.contains(&op.trim()) || payload.get("replace").is_some_and(|v| v != &Value::Bool(false) && !v.is_null())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// "just now", "4m ago", "2h ago", "3d ago".
pub fn ago(at_ms: i64, now: i64) -> String {
    let s = (now - at_ms) / 1000;
    match s {
        s if s < 45 => "just now".into(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86400 => format!("{}h ago", s / 3600),
        s => format!("{}d ago", s / 86400),
    }
}

/// The name the server lists this phone under, and records its syncs for (1 to 64 characters).
pub fn device_name() -> String {
    #[cfg(target_os = "android")]
    let place = "a phone".to_string();
    #[cfg(not(target_os = "android"))]
    let place = std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).unwrap_or_else(|| "this computer".into());
    let name = format!("Tally RS on {place}");
    name.chars().filter(|c| !c.is_control()).take(64).collect()
}

/// Writes `text` to `path` readable by this app only (0600), whole or not at all.
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        let mut f = o.open(&tmp)?;
        #[cfg(unix)]
        std::fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        std::io::Write::write_all(&mut f, text.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

fn record_failure(store: &mut Store, reason: &str) {
    let _ = store.set_pref(pref::LAST_ATTEMPT, &now_ms().to_string());
    let _ = store.set_pref(pref::LAST_ERROR, reason);
}

/// Asks the server to let this phone in and waits for the owner's answer (Kotlin: CloudConnect
/// `connect`, `ask`, `await`). Polls every two seconds for the first minute, while they are likely
/// looking at Discord, then every six; once allowed, pairs, collecting a fresh link up to three
/// times.
///
/// Once cancelled it sends nothing more, checked before every send; what slips through between the
/// check and the send carries `attempt`, which the UI no longer takes.
#[allow(clippy::too_many_arguments)]
fn run_connect(net: &Net, tx: &Sender<Event>, stop: &Stop, dir: &Option<PathBuf>, attempt: u64, server: &str, pending: Option<Enrollment>, device: &str) {
    let cancel = &stop.cancel;
    let send = |e: Event| {
        if !cancel.load(Ordering::Relaxed) {
            let _ = tx.send(e);
        }
    };
    let say = |c: Connect| send(Event::Connect(attempt, c));
    // What is kept in the secret file changed: a loop asleep in the background (the person is in
    // Discord tapping Allow) writes it now, before Android may stop the process.
    let pending_is = |e: Option<Enrollment>| {
        send(Event::Pending(attempt, e));
        wake();
    };
    let trouble = |f: Failure| say(Connect::Trouble { cloud_off: f.code == code::CLOUD_OFF, reason: f.message });
    let e = match pending {
        Some(e) => e,
        None => {
            say(Connect::Reaching);
            match net::hello(net.http.as_ref(), server) {
                Ok(true) => {}
                Ok(false) => return say(Connect::Trouble { reason: "Relay is not running on your server yet".into(), cloud_off: true }),
                Err(f) => return trouble(f),
            }
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            match net::enroll(net.http.as_ref(), server, device, now_ms()) {
                Ok(e) => {
                    pending_is(Some(e.clone()));
                    e
                }
                Err(f) => return trouble(f),
            }
        }
    };
    let started = std::time::Instant::now();
    let (mut failures, mut pairings) = (0, 0);
    while now_ms() < e.expires_at {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        say(Connect::Waiting { code: e.code.clone(), notified: e.notified, expires_at: e.expires_at, server: e.server.clone() });
        match net::claim(net.http.as_ref(), &e) {
            net::Claim::Waiting => failures = 0,
            net::Claim::Ready(link) => {
                // Pairing spends the link: a cancelled thread leaves it to the one after it.
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                // Another thread (a gone App's) is pairing for this phone, or has paired: its key
                // comes to this App. Ask again after the wait.
                let Some(_pairing) = Pairing::begin(dir) else {
                    sleep_unless(cancel, Duration::from_secs(2));
                    continue;
                };
                say(Connect::Pairing);
                match net::pair(net.dial.as_ref(), &link, device, now_ms()) {
                    Ok(creds) => {
                        {
                            // Under the hand-off's lock, as [`State`]'s Drop looks in the channel.
                            let mut h = handoff();
                            if stop.gone.load(Ordering::SeqCst) {
                                // The App went away while this paired: the link is spent and the
                                // server knows this device, so the key goes to the next App.
                                hand_on(&mut h, dir, creds, e.server.clone());
                            } else {
                                send(Event::Pending(attempt, None));
                                send(Event::Paired(attempt, creds, e.server.clone()));
                            }
                        }
                        // The new key: the link that made it is spent, so it must not wait for
                        // the app to come back to be kept.
                        wake();
                        return;
                    }
                    Err(f) => {
                        pairings += 1;
                        if pairings >= 3 {
                            pending_is(None);
                            return trouble(f);
                        }
                    }
                }
            }
            net::Claim::Denied => {
                pending_is(None);
                return say(Connect::Denied);
            }
            net::Claim::Expired | net::Claim::Spent => {
                pending_is(None);
                return say(Connect::Expired);
            }
            net::Claim::Trouble(reason) => {
                failures += 1;
                if failures >= 10 {
                    return say(Connect::Trouble { reason, cloud_off: false });
                }
            }
        }
        let wait = if started.elapsed() < Duration::from_secs(60) { 2_000 } else { 6_000 };
        if !sleep_unless(cancel, Duration::from_millis(wait)) {
            return;
        }
    }
    pending_is(None);
    say(Connect::Expired);
}

/// Sleeps `wait`, a tenth of a second at a time, unless cancelled: false once it is.
fn sleep_unless(cancel: &AtomicBool, wait: Duration) -> bool {
    for _ in 0..wait.as_millis() / 100 {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !cancel.load(Ordering::Relaxed)
}

/// A key handed on for the data folder `dir` ([`hand_on`]), once.
fn take_handed_on(dir: &Option<PathBuf>) -> Option<(Creds, String)> {
    let mut h = handoff();
    let i = h.paired.iter().position(|(d, ..)| d == dir)?;
    let (_, creds, server) = h.paired.remove(i);
    Some((creds, server))
}

type Waker = Box<dyn Fn() + Send + Sync>;

/// Called from the sync thread when its answer is in, so a sleeping loop takes it at once (as
/// [`crate::pictures::set_waker`]). Replaceable: Android starts its loop again for each activity.
static WAKE: std::sync::Mutex<Option<Waker>> = std::sync::Mutex::new(None);

/// How the platform wakes the app's loop from the sync thread. A later call replaces the earlier.
pub fn set_waker(f: Waker) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

fn wake() {
    if let Some(w) = WAKE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w();
    }
}

/// Something came back from a sync or a connect thread that no frame has taken yet. A loop asleep
/// in the background ([`set_waker`]) [`flush`]es when it is woken with news, so a pairing finished
/// while the app was away keeps its key at once.
pub fn has_news(app: &mut App) -> bool {
    app.st.sync.started && app.st.sync.news()
}

/// The ledger changed here: sync a few seconds after the last change (`App::invalidate`).
pub fn changed(app: &mut App) {
    let st = &mut app.st.sync;
    if !st.applying {
        st.dirty_at = Some(app.now_ns);
    }
}

/// Once a frame, before the page draws: collect what came back, apply it, and start what is due.
pub fn tick(app: &mut App, now_ns: u64) {
    if !app.st.sync.started {
        let dir = app.host.data_dir();
        app.st.sync.start(dir, &mut app.store);
        // No sync starts on the first frame: the bills that fell due while the app was closed
        // post this frame (`App::frame`, before this tick), and a sync started before them would
        // find this ledger moved when its answer came, which for a whole answer asks the person
        // about entries nobody made. The next frame starts it ([`State::wake_at`]).
        let st = &mut app.st.sync;
        st.on = st.paired(&app.store) && app.store.pref(pref::PHONE_ONLY).is_none();
        return;
    }
    let host = app.host.clone();
    if step(&mut app.st.sync, &mut app.store, now_ns, &|s| host.log(s)) {
        app.st.sync.applying = true;
        app.invalidate();
        app.st.sync.applying = false;
    }
}

/// The app leaves the screen (Android's Pause, the desktop window closing): a change still in its
/// debounce syncs now, as no frame comes to start it until the app is open again. The sync runs
/// on a thread of its own, which needs no frames; its answer waits in the channel and the first
/// [`tick`] after the app comes back applies it (a sync that took longer than the person was away
/// is applied the same way, its mark guarding entries made meanwhile).
///
/// A change made while a sync runs cannot go until that one answers: true while a sync is still
/// running after this, so the platform flushes again when its answer is in (the sync thread wakes
/// the loop, [`set_waker`]), which applies it and starts the follow-up at once.
pub fn flush(app: &mut App, now_ns: u64) -> bool {
    if !app.st.sync.started || app.ledger_error.is_some() {
        return false;
    }
    let host = app.host.clone();
    if step_with(&mut app.st.sync, &mut app.store, now_ns, &|s| host.log(s), true) {
        app.st.sync.applying = true;
        app.invalidate();
        app.st.sync.applying = false;
    }
    app.st.sync.in_flight.is_some()
}

/// The process is about to end (the desktop window closed): waits up to `limit` for the sync
/// running to answer, and applies it, as its thread ends with the process. An edit made while it
/// ran is then sent too ([`flush`] starts that follow-up as the answer lands), and waited on
/// within the same limit. Never on Android: the process lives on after Pause there, and its loop
/// must not stall.
pub fn wait_out(app: &mut App, limit: Duration) {
    let start = std::time::Instant::now();
    while app.st.sync.started && app.st.sync.in_flight.is_some() && start.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(50));
        // The time of the last frame: nothing new falls due by the clock while waiting, only
        // what flush takes as due (a change still in its debounce).
        let now_ns = app.now_ns;
        flush(app, now_ns);
    }
}

/// [`tick`]'s work, apart from the App. True when the ledger changed. The prefs it sets are
/// written once, at its end.
fn step(st: &mut State, store: &mut Store, now_ns: u64, log: &dyn Fn(&str)) -> bool {
    step_with(st, store, now_ns, log, false)
}

/// [`step`], with `flush` taking a change still in its debounce as due ([`flush`]).
fn step_with(st: &mut State, store: &mut Store, now_ns: u64, log: &dyn Fn(&str), flush: bool) -> bool {
    store.holding_prefs(|store| step_held(st, store, now_ns, log, flush))
}

fn step_held(st: &mut State, store: &mut Store, now_ns: u64, log: &dyn Fn(&str), flush: bool) -> bool {
    st.pump();
    let mut ledger_changed = false;
    for e in std::mem::take(&mut st.inbox) {
        match e {
            // A cancelled connect's news, or a dropped sync's answer: not this phone's any more.
            Event::Connect(a, _) | Event::Pending(a, _) | Event::Paired(a, ..) if a != st.attempt => {}
            Event::Synced(id, ..) if st.in_flight != Some(id) => {}
            Event::Connect(_, c) => {
                // A paired phone's status is not a connect's news; one the server forgot (its
                // stale key still kept) is connecting again, and shows the code to allow.
                if !st.paired(store) || !matches!(c, Connect::Waiting { .. } | Connect::Reaching) {
                    st.connect = c;
                }
            }
            Event::Pending(_, p) => {
                st.pending = p;
                st.save_secret(log);
            }
            Event::Paired(_, creds, server) => {
                // Pairing with another server starts from nothing: the next sync takes its ledger
                // whole (after the person's yes when this phone holds anything). Pairing again with
                // the one this phone last synced with (it forgot this phone's key, or a disconnect)
                // keeps where sync stood, so what was made here since goes up as a merge instead
                // of being replaced. That never sends a ledger replaced here since (`collect`
                // refuses one, and the next sync takes the server's whole), and the server refuses
                // a merge into a ledger of its own replaced since (the stale answer, which asks
                // before dropping changes never sent).
                st.drop_sync();
                st.ask_first = false;
                st.take_whole = false;
                // Compared normalized: an address kept by an older build may differ in case only.
                let server = wire::normalize(&server);
                if store.pref(pref::PAIRED_SERVER).map(|p| wire::normalize(&p)).as_deref() != Some(server.as_str()) {
                    ledger::forget(store);
                }
                let _ = store.set_pref(pref::PAIRED_SERVER, &server);
                for k in [pref::UNPAIRED, pref::LAST_ERROR, pref::NOTE, pref::LAST_SUCCESS] {
                    let _ = store.clear_pref(k);
                }
                st.creds = Some(creds);
                st.cancel = None;
                st.connect = Connect::Connected;
                st.save_secret(log);
                st.sync_asked = true;
            }
            Event::Synced(_, result, at) => {
                st.in_flight = None;
                let asked = std::mem::take(&mut st.whole_asked);
                match result {
                    Ok(answer) => {
                        st.applying = true;
                        let applied = ledger::apply(store, &answer, &at, asked);
                        st.applying = false;
                        match applied {
                            Ok(a) if a.ask_first => match a.fewer {
                                Some((there, here)) => st.ask(fewer_body(there, here)),
                                None => st.ask(CHANGED_BODY),
                            },
                            Ok(a) => {
                                ledger_changed |= a.received > 0 || answer.whole;
                                st.took_ledger |= answer.whole;
                                st.received_rows |= a.received > 0;
                                record_success(store, &answer, a.received);
                                if a.ahead > 0 {
                                    let _ = store.set_pref(pref::NOTE, &ledger::ahead_note(a.ahead));
                                } else if asked && answer.note.is_none() {
                                    let _ = store.set_pref(pref::NOTE, ledger::TOOK_SERVER_LEDGER_ASKED);
                                }
                                if a.again {
                                    st.dirty_at = Some(now_ns);
                                }
                            }
                            Err(f) => {
                                if asked && answer.whole {
                                    st.keep_yes(at);
                                }
                                record_failure(store, &f.message);
                            }
                        }
                    }
                    // The server's ledger was replaced and changes here never went: taking it is
                    // the person's call.
                    Err(f) if f.code == ledger::STALE_UNSENT => st.ask(STALE_BODY),
                    Err(f) => {
                        if f.code == code::UNPAIRED {
                            let _ = store.set_pref(pref::UNPAIRED, "1");
                        }
                        // Unpaired, sync is off below, which drops the yes again.
                        if asked {
                            st.keep_yes(at);
                        }
                        record_failure(store, &f.message);
                    }
                }
            }
            Event::Replied(..) => {}
        }
    }
    let on = st.paired(store) && store.pref(pref::PHONE_ONLY).is_none();
    st.on = on;
    if !on {
        // However sync went off (the toggle, the first-run page, the server forgetting this
        // phone), a yes to take the server's ledger is not carried to when it is on again: the
        // entries made meanwhile would be replaced without a word.
        st.ask_first = false;
        st.take_whole = false;
    }
    if on && st.in_flight.is_none() && !st.ask_first {
        let due = st.sync_asked
            || (st.take_whole && st.whole_mark.is_none())
            || st.last_try_ns.is_none()
            || st.dirty_at.is_some_and(|d| flush || now_ns.saturating_sub(d) >= DEBOUNCE_NS)
            || st.last_try_ns.is_some_and(|t| now_ns.saturating_sub(t) >= PERIOD_NS)
            // A phone that slept: its monotonic clock stood still, the wall clock did not. One
            // sync, as it sets both clocks' last try again.
            || st.last_try_ms.is_some_and(|t| now_ms().saturating_sub(t) >= PERIOD_MS);
        if due {
            st.start_sync(store, now_ns);
        }
    } else if !on {
        st.sync_asked = false;
    }
    ledger_changed
}

fn record_success(store: &mut Store, answer: &Answer, received: usize) {
    let now = now_ms().to_string();
    let _ = store.set_pref(pref::LAST_ATTEMPT, &now);
    let _ = store.set_pref(pref::LAST_SUCCESS, &now);
    let _ = store.clear_pref(pref::LAST_ERROR);
    let _ = store.set_pref(pref::SENT, &answer.sent.to_string());
    let _ = store.set_pref(pref::RECEIVED, &received.to_string());
    match &answer.note {
        Some(n) => {
            let _ = store.set_pref(pref::NOTE, n);
        }
        None if answer.sent + received > 0 => {
            let _ = store.clear_pref(pref::NOTE);
        }
        None => {}
    }
}

/// For screenshots of the page's states on the desktop: `TALLY_SYNC_DEMO=connected|waiting`
/// pretends, with no network at all.
#[cfg(all(debug_assertions, not(target_os = "android")))]
fn demo(st: &mut State, store: &mut Store) {
    let Ok(which) = std::env::var("TALLY_SYNC_DEMO") else { return };
    st.net = Net { dial: Arc::new(net::Offline), http: Arc::new(net::Offline) };
    st.dir = None;
    let now = now_ms();
    match which.as_str() {
        "connected" => {
            st.creds = Some(Creds { host: "Cloud".into(), routes: vec![], device: "demo".into(), token: String::new(), paired_at: now });
            let _ = store.set_pref(ledger::PULLED, "1");
            for (k, v) in [(pref::LAST_SUCCESS, (now - 65_000).to_string()), (pref::LAST_ATTEMPT, (now - 65_000).to_string()), (pref::SENT, "3".into()), (pref::RECEIVED, "12".into())] {
                let _ = store.set_pref(k, &v);
            }
            st.last_try_ns = Some(0);
        }
        "waiting" => {
            st.connect = Connect::Waiting { code: "7F3A".into(), notified: true, expires_at: now + 600_000, server: State::server(store) };
        }
        _ => {}
    }
}

#[cfg(not(all(debug_assertions, not(target_os = "android"))))]
fn demo(_st: &mut State, _store: &mut Store) {}

// The Sync page

const HOW: &str = "Tally syncs with Relay's copy on your server, wherever the phone is: home Wi-Fi or not. Your PC's Relay syncs with the same copy, so all three hold one ledger. Entries made offline go when the network comes back; the newest edit wins.";
const FIRST: &str = "The first sync brings your server's ledger to this phone, in place of what is here. Tally RS never sends a ledger to replace the server's.";

enum Act {
    Back,
    SyncNow,
    Connect,
    Dashboard(String),
    UseAddress(String),
    DefaultAddress,
    PhoneOnly(bool),
    Ask(Confirm),
    Confirmed(Confirm),
    Cancel,
}

/// A quiet key: a glyph and words, no box.
fn quiet_key(cx: &mut Ctx, x: f32, y: f32, words: &str, glyph: &'static str) -> (f32, bool) {
    let p = cx.p;
    let t = Text::new(14.0, p.ink2).medium();
    let w = cx.c.measure(words, t) + 26.0 + 24.0;
    if cx.pressed(x, y, w, 40.0) {
        cx.c.rect(x, y, w, 40.0, 20.0, p.wash);
    }
    cx.c.icon(glyph, x + 12.0 + 8.0, y + 20.0, 15.0, p.ink2);
    cx.c.text(words, x + 12.0 + 26.0, y + (40.0 - t.lh()) / 2.0, t);
    (w, cx.tap(x, y, w, 40.0))
}

/// Back closes the open question as its Cancel does.
pub fn back(app: &mut App) -> bool {
    match app.st.sync.confirm.take() {
        Some(Confirm::First) => {
            app.st.sync.answer_first(&mut app.store, false);
            app.say("Sync is off. This phone keeps its ledger.", None);
            true
        }
        Some(_) => true,
        None => false,
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let now = now_ms();
    let server = State::server(&app.store);
    if app.fresh {
        app.st.sync.address = Some(app.store.pref(pref::SERVER).unwrap_or_else(|| server.clone()));
    }
    let paired = app.st.sync.paired(&app.store);
    let phone_only = app.store.pref(pref::PHONE_ONLY).is_some();
    let pulled = app.store.pref(ledger::PULLED).is_some();
    let l = app.st.sync.line(&app.store, now);
    let connect = app.st.sync.connect.clone();
    let syncing = app.st.sync.in_flight.is_some();
    let num = |k: &str| app.store.pref(k).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    let (last, sent, received) = (num(pref::LAST_SUCCESS), num(pref::SENT), num(pref::RECEIVED));
    let (problem, note) = (app.store.pref(pref::LAST_ERROR), app.store.pref(pref::NOTE));
    if app.st.sync.confirm == Some(Confirm::First) && !app.st.sync.ask_first {
        app.st.sync.confirm = None;
    }
    if app.st.sync.ask_first && app.st.sync.confirm.is_none() {
        app.st.sync.confirm = Some(Confirm::First);
    }
    let confirm = app.st.sync.confirm;
    let question = app.st.sync.question().to_string();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let held = if confirm.is_some() { widgets::hold(&mut cx) } else { None };
        let p = cx.p;
        let mut y = pg.top;
        let (back, _, hh) = widgets::page_header(&mut cx, y, "Your server", None);
        if back {
            act = Some(Act::Back);
        }
        y += hh + 2.0;
        let host = server.trim_start_matches("https://").to_string();
        y += sk::note(&mut cx, y, Some("globe"), &host) + 16.0;

        // Where it stands
        let (x, w) = (PAD, WIDTH - PAD * 2.0);
        let inner = w - 44.0;
        let title_t = Text::new(20.0, p.ink).semi().center().line(1.25);
        let detail_t = Text::new(14.0, p.ink2).center().line(1.45);
        let caption_t = Text::new(13.0, p.ink3).line(1.45);
        let waiting = match (&connect, paired) {
            (Connect::Waiting { code, notified, server, .. }, false) => Some((code.clone(), *notified, server.clone())),
            _ => None,
        };
        let how_wait = "Tap Allow on the message from your bot on Discord. This phone connects by itself the moment you do, and syncs from then on with nothing to set up.";
        let key: Option<(&str, &'static str)> = if paired && !phone_only {
            Some((if syncing { "Syncing" } else { "Sync now" }, "sync"))
        } else if matches!(connect, Connect::Denied | Connect::Expired | Connect::Trouble { .. }) {
            Some(("Ask again", "refresh"))
        } else if !paired && !phone_only && matches!(connect, Connect::Idle | Connect::Connected) {
            Some(("Connect to your server", "link"))
        } else {
            None
        };
        let th = cx.c.para_h(&l.text, inner, title_t, 2);
        let dh = l.detail.as_ref().map_or(0.0, |d| cx.c.para_h(d, inner, detail_t, 4) + 6.0);
        let wh = if waiting.is_some() { 14.0 + 76.0 + 10.0 + cx.c.para_h(how_wait, inner, caption_t, 4) + 8.0 + 40.0 } else { 0.0 };
        let kh = if key.is_some() { 18.0 + 52.0 } else { 0.0 };
        let h = 24.0 + 60.0 + 14.0 + th + dh + wh + kh + 22.0;
        kit::card(&mut cx, x, y, w, h);
        let mut iy = y + 24.0;
        let tone = l.tone.colour(&p);
        cx.c.circle(WIDTH / 2.0, iy + 30.0, 30.0, crate::theme::soft_of(tone));
        let ink = match l.tone {
            Tone::Live => p.live_text,
            Tone::Waiting => p.waiting_text,
            Tone::Held => p.held_text,
            Tone::Off => p.ink2,
        };
        cx.c.icon(if l.tone == Tone::Live { "check" } else { "cloud" }, WIDTH / 2.0, iy + 30.0, 26.0, ink);
        iy += 60.0 + 14.0;
        iy += cx.c.para(&l.text, x + 22.0, iy, inner, title_t, 2);
        if let Some(d) = &l.detail {
            iy += 6.0 + cx.c.para(d, x + 22.0, iy + 6.0, inner, detail_t, 4);
        }
        if let Some((code_, notified, srv)) = &waiting {
            iy += 14.0;
            cx.c.rect(x + 22.0, iy, inner, 76.0, kit::KEY_R, p.card2);
            cx.c.text("THE CODE YOUR BOT SHOWS", WIDTH / 2.0, iy + 12.0, Text::new(11.0, p.ink3).semi().center());
            let spaced: String = code_.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
            cx.c.text(&spaced, WIDTH / 2.0, iy + 32.0, Text::new(28.0, p.ink).medium().center().line(1.2));
            iy += 76.0 + 10.0;
            iy += cx.c.para(how_wait, x + 22.0, iy, inner, caption_t, 4) + 8.0;
            let label = if *notified { "Open the dashboard instead" } else { "Open the dashboard" };
            if quiet_key(&mut cx, x + 12.0, iy, label, "external").1 {
                act = Some(Act::Dashboard(format!("{srv}/dashboard/tally")));
            }
            iy += 40.0;
        }
        if let Some((words, glyph)) = key {
            iy += 18.0;
            if kit::key(&mut cx, x + 22.0, iy, inner, 52.0, words, Some(glyph), KeyKind::Primary) && !syncing {
                act = Some(if paired { Act::SyncNow } else { Act::Connect });
            }
        }
        y += h + sk::GROUP_GAP;

        if paired {
            let when = if last > 0 { ago(last, now) } else { "Not yet".into() };
            let traffic = format!("{sent} · {received}");
            let mut rows = vec![sk::Row::new("When").value(&when), sk::Row::new("Sent and received").value(&traffic)];
            if let Some(e) = &problem {
                rows.push(sk::Row::new("Problem").line(e).line_color(p.waiting_text));
            }
            if let Some(n) = &note {
                rows.push(sk::Row::new("News").line(n));
            }
            y += sk::group(&mut cx, y, Some("Last sync"), &rows).0 + sk::GROUP_GAP;
        }

        y += sk::eyebrow(&mut cx, y, "How it works");
        let pt = Text::new(14.0, p.ink2).line(1.5);
        let how = if pulled { HOW.to_string() } else { format!("{HOW} {FIRST}") };
        let ph = cx.c.para_h(&how, w - 40.0, pt, 10);
        kit::card(&mut cx, PAD, y, w, ph + 40.0);
        cx.c.para(&how, PAD + 20.0, y + 20.0, w - 40.0, pt, 10);
        y += ph + 40.0 + sk::GROUP_GAP;

        y += sk::eyebrow(&mut cx, y, "Address");
        let sh = 14.0 + 52.0 + 12.0 + 40.0 + 14.0;
        kit::card(&mut cx, PAD, y, w, sh);
        let (fx, fy, fw) = (PAD + 14.0, y + 14.0, w - 28.0);
        let focused = cx.focus.is("sync.server");
        cx.c.rect(fx, fy, fw, 52.0, kit::KEY_R, p.card2);
        if focused {
            cx.c.outline(fx, fy, fw, 52.0, kit::KEY_R, 1.5, p.signal);
        }
        cx.c.icon("globe", fx + 22.0, fy + 26.0, 18.0, p.ink2);
        let st = &mut app.st.sync;
        let address = st.address.get_or_insert_with(|| server.clone());
        widgets::inline_kb(&mut cx, "sync.server", fx + 44.0, fy, fw - 58.0, 52.0, wire::DEFAULT_SERVER, address, widgets::Kb::Url, Text::new(15.0, p.ink));
        let typed = address.trim().to_string();
        let ky = fy + 52.0 + 12.0;
        let use_t = Text::new(14.0, p.ink).semi();
        let uw = cx.c.measure("Use this address", use_t) + 32.0;
        let can_use = !typed.is_empty() && wire::normalize(&typed) != server;
        if sk::tall_key(&mut cx, fx, ky, uw, 40.0, "Use this address", None, can_use, can_use) {
            act = Some(Act::UseAddress(typed));
        }
        if quiet_key(&mut cx, fx + uw + 8.0, ky, "Default", "undo").1 {
            act = Some(Act::DefaultAddress);
        }
        y += sh + sk::GROUP_GAP;

        let sub = if phone_only { "Off: the ledger stays on this phone" } else { "On: it syncs by itself" };
        let mut rows = vec![sk::Row::new("Sync with my server").icon("sync").line(sub).toggle(!phone_only)];
        if paired {
            rows.push(sk::Row::new("Take the server's ledger").icon("download").line("Replace this phone's with the server's copy").opens());
            rows.push(sk::Row::new("Disconnect").glyph("close", p.held).color(p.held_text).line("Stop syncing; both copies stay as they are").opens());
        }
        let (h, tapped) = sk::group(&mut cx, y, Some("This phone"), &rows);
        match tapped {
            Some(0) => act = Some(Act::PhoneOnly(!phone_only)),
            Some(1) => act = Some(Act::Ask(Confirm::Take)),
            Some(_) => act = Some(Act::Ask(Confirm::Disconnect)),
            None => {}
        }
        y += h;
        bottom = y;

        if let Some(c) = confirm {
            widgets::release(&mut cx, held);
            let answer = match c {
                Confirm::Take => widgets::confirm(&mut cx, "Take the server's ledger?", if phone_only { TAKE_BODY_OFF } else { TAKE_BODY }, "Take it", true),
                Confirm::Disconnect => widgets::confirm(
                    &mut cx,
                    "Disconnect from your server?",
                    "This phone stops syncing; both copies stay as they are. Connecting again asks your bot to let it in.",
                    "Disconnect",
                    true,
                ),
                Confirm::First => widgets::confirm(&mut cx, FIRST_TITLE, &question, FIRST_YES, true),
            };
            match answer {
                Some(true) => act = Some(Act::Confirmed(c)),
                Some(false) => act = Some(Act::Cancel),
                None => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    match act {
        Some(Act::Back) => {
            app.pop();
        }
        Some(Act::SyncNow) => app.st.sync.sync_now(),
        Some(Act::Connect) => {
            app.finish_typing();
            app.st.sync.connect_now(&mut app.store);
        }
        Some(Act::Dashboard(url)) => app.host.open_url(&url),
        Some(Act::UseAddress(a)) => {
            app.finish_typing();
            set_server(app, Some(&a));
        }
        Some(Act::DefaultAddress) => {
            app.finish_typing();
            app.st.sync.address = Some(wire::DEFAULT_SERVER.to_string());
            set_server(app, None);
        }
        Some(Act::PhoneOnly(off)) => app.st.sync.set_sync(&mut app.store, !off),
        Some(Act::Ask(c)) => app.st.sync.confirm = Some(c),
        Some(Act::Cancel) => {
            if app.st.sync.confirm.take() == Some(Confirm::First) {
                app.st.sync.answer_first(&mut app.store, false);
                app.say("Sync is off. This phone keeps its ledger.", None);
            }
        }
        Some(Act::Confirmed(c)) => {
            app.st.sync.confirm = None;
            match c {
                Confirm::Take => {
                    let off = app.store.pref(pref::PHONE_ONLY).is_some();
                    app.st.sync.take(&mut app.store);
                    app.say(if off { "Sync is on. Taking your server's ledger" } else { "Taking your server's ledger" }, None);
                }
                Confirm::First => {
                    app.st.sync.answer_first(&mut app.store, true);
                    app.say("Taking your server's ledger", None);
                }
                Confirm::Disconnect => {
                    let st = &mut app.st.sync;
                    st.disconnect(&mut app.store);
                    let host = app.host.clone();
                    st.save_secret(&|s| host.log(s));
                    app.say("Disconnected. This phone's ledger stays as it is.", None);
                }
            }
        }
        None => {}
    }
    pg.moving || app.st.sync.working() || app.st.sync.confirm.is_some()
}

/// A new address: the old request and pairing belong to the old server.
fn set_server(app: &mut App, address: Option<&str>) {
    let next = address.map(wire::normalize);
    let target = next.clone().unwrap_or_else(|| wire::normalize(wire::DEFAULT_SERVER));
    if target == State::server(&app.store) {
        return;
    }
    let st = &mut app.st.sync;
    st.disconnect(&mut app.store);
    let host = app.host.clone();
    st.save_secret(&|s| host.log(s));
    let _ = match &next {
        Some(a) if a != &wire::normalize(wire::DEFAULT_SERVER) => app.store.set_pref(pref::SERVER, a),
        _ => app.store.clear_pref(pref::SERVER),
    };
    app.say("Your server's address is changed. Connect to it when you are ready.", None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_ledger::tests::{FakeServer, all_uids};
    use crate::sync_net::{Dial, Http, Lines};
    use serde_json::json;
    use std::sync::Mutex;

    /// The owner's server, faked: its `/api/tally` and Relay's door in front of a ledger.
    #[derive(Clone)]
    struct Fake(Arc<Mutex<Inner>>);

    struct Inner {
        server: FakeServer,
        claims: usize,
        allow_after: usize,
        enrolls: usize,
        tokens: Vec<(String, String)>,
        requests: Vec<String>,
        /// The server is out of reach: every connection fails.
        down: bool,
        /// How long a pairing takes to answer.
        pair_delay_ms: u64,
    }

    impl Fake {
        fn new() -> Fake {
            Fake(Arc::new(Mutex::new(Inner { server: FakeServer::with_owners_ledger(), claims: 0, allow_after: 1, enrolls: 0, tokens: vec![], requests: vec![], down: false, pair_delay_ms: 0 })))
        }
        fn net(&self) -> Net {
            Net { dial: Arc::new(self.clone()), http: Arc::new(self.clone()) }
        }
    }

    impl Http for Fake {
        fn get(&self, url: &str) -> Result<(u16, Option<Value>), Failure> {
            assert!(url.ends_with("/api/tally"));
            Ok((200, Some(json!({"service": "tally-cloud", "cloud": true}))))
        }
        fn post(&self, url: &str, body: &Value) -> Result<(u16, Option<Value>), Failure> {
            let mut i = self.0.lock().unwrap();
            if url.ends_with("/enroll") {
                i.enrolls += 1;
                assert_eq!(body["challenge"].as_str().unwrap().len(), 64);
                let code = wire::code_of(body["challenge"].as_str().unwrap());
                return Ok((201, Some(json!({"id": "e1", "code": code, "expiresAt": now_ms() + 60_000, "notified": true}))));
            }
            i.claims += 1;
            if i.claims <= i.allow_after {
                return Ok((200, Some(json!({"status": "waiting"}))));
            }
            Ok((200, Some(json!({"status": "ready", "link": "relay://pair?v=1&code=PAIRME&host=Cloud&via=wss://fake/join/r"}))))
        }
    }

    struct FakeLines {
        fake: Fake,
        out: std::collections::VecDeque<String>,
        challenge: String,
        admitted: bool,
    }

    impl Dial for Fake {
        fn open(&self, route: &str) -> Result<Box<dyn Lines>, Failure> {
            assert_eq!(route, "wss://fake/join/r");
            if self.0.lock().unwrap().down {
                return Err(net::fail(code::UNREACHABLE, "offline"));
            }
            let challenge = uuid::Uuid::new_v4().to_string();
            let greet = json!({"v": 1, "relay": "remote", "host": "", "host_id": "", "instance": "cloud", "challenge": challenge}).to_string();
            Ok(Box::new(FakeLines { fake: self.clone(), out: [greet].into(), challenge, admitted: false }))
        }
    }

    impl Lines for FakeLines {
        fn send(&mut self, line: &str) -> Result<(), Failure> {
            let v: Value = serde_json::from_str(line).unwrap();
            if !self.admitted && v.get("pair").is_some() {
                let delay = self.fake.0.lock().unwrap().pair_delay_ms;
                std::thread::sleep(Duration::from_millis(delay));
            }
            let mut i = self.fake.0.lock().unwrap();
            if !self.admitted {
                if v.get("pair").is_some() {
                    assert_eq!(v["pair"], "PAIRME");
                    i.tokens.push(("dev1".into(), "tok1".into()));
                    self.out.push_back(json!({"v": 1, "ok": true, "device": "dev1", "token": "tok1", "host": "Cloud"}).to_string());
                } else {
                    let ok = i.tokens.iter().any(|(d, t)| v["device"] == *d && v["proof"] == wire::proof(&self.challenge, t));
                    self.out.push_back(if ok { json!({"v": 1, "ok": true, "device": "dev1"}) } else { json!({"v": 1, "ok": false, "error": "auth.bad_proof"}) }.to_string());
                }
                self.admitted = true;
                return Ok(());
            }
            let op = v["op"].as_str().unwrap().to_string();
            i.requests.push(op.clone());
            let id = v["id"].clone();
            let answer = if op == "money.sync" {
                i.server.handle(&op, &v["payload"])
            } else {
                Ok(json!({"echo": op}))
            };
            self.out.push_back(match answer {
                Ok(r) => json!({"id": id, "ok": true, "result": r}),
                Err(f) => json!({"id": id, "ok": false, "error": {"code": f.code, "message": f.message}}),
            }.to_string());
            Ok(())
        }
        fn next(&mut self, _wait: Duration) -> Result<String, Failure> {
            self.out.pop_front().ok_or_else(|| net::fail(code::UNREACHABLE, "closed"))
        }
    }

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("tally-sync-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Runs the parts of `tick` that need no App, until `until` holds or a few seconds pass.
    fn run(st: &mut State, store: &mut Store, until: impl Fn(&State, &Store) -> bool) {
        let start = std::time::Instant::now();
        let mut now_ns = 1u64;
        while !until(st, store) {
            assert!(start.elapsed() < Duration::from_secs(20), "timed out at {:?}", st.connect);
            step(st, store, now_ns, &|_| {});
            now_ns += 50_000_000;
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn connecting_pairs_pulls_the_servers_ledger_and_never_replaces_it() {
        let fake = Fake::new();
        let before = fake.0.lock().unwrap().server.entries();
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        store.load_sample(crate::model::today()).unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        assert!(!st.paired(&store));
        assert_eq!(fake.0.lock().unwrap().enrolls, 0, "nothing is asked until the person taps Connect");
        st.connect_now(&mut store);
        // Paired, the first pull waits on a yes: it would replace the sample here.
        run(&mut st, &mut store, |st, _| st.awaiting_yes());
        assert!(st.paired(&store));
        assert!(!fake.0.lock().unwrap().requests.iter().any(|o| o == "money.sync"), "nothing pulled before the yes");
        assert_eq!(st.line(&store, now_ms()).text, "Waiting for your yes");
        st.answer_first(&mut store, true);
        run(&mut st, &mut store, |st, store| st.paired(store) && store.pref(pref::LAST_SUCCESS).is_some());
        let i = fake.0.lock().unwrap();
        assert_eq!(i.enrolls, 1);
        assert_eq!(i.server.replaces, 0);
        assert_eq!(i.server.entries(), before, "the server's ledger is as it was");
        assert_eq!(all_uids(&store.ledger), all_uids(&i.server.ledger), "this phone holds the server's ledger");
        drop(i);
        // The key is kept in a file of its own, readable by this app only, and read back.
        let secret = d.join(SECRET_FILE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&secret).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert!(std::fs::read_to_string(&secret).unwrap().contains("tok1"));
        let mut again = State::with_net(fake.net());
        again.start(d.clone(), &mut store);
        assert!(again.paired(&store));

        // An entry here goes up on the next sync.
        let account = store.ledger.accounts().unwrap()[0].id;
        let mine = store
            .add(&relay_money::views::TxInput { r#type: relay_money::model::TxType::Expense, amount: 2_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None })
            .unwrap();
        again.sync_asked = true;
        run(&mut again, &mut store, |st, _| st.in_flight.is_none() && !st.sync_asked);
        let i = fake.0.lock().unwrap();
        assert!(i.server.entries().contains(&mine.uid));
        assert_eq!(i.server.replaces, 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A State paired with `fake` and synced once, over an empty phone ledger.
    fn paired(fake: &Fake) -> (State, Store, PathBuf) {
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, store| st.paired(store) && store.pref(pref::LAST_SUCCESS).is_some() && st.in_flight.is_none());
        (st, store, d)
    }

    /// Every row the phone sent the fake server, and how many of them were deletions.
    fn sent(fake: &Fake) -> (usize, usize) {
        let i = fake.0.lock().unwrap();
        let all: Vec<Value> = i.server.requests.iter().flat_map(|r| r["changes"].as_array().unwrap().clone()).collect();
        (all.len(), all.iter().filter(|c| c["deleted"] == Value::Bool(true)).count())
    }

    #[test]
    fn a_first_pull_into_an_empty_phone_asks_nothing() {
        let fake = Fake::new();
        let (st, store, d) = paired(&fake);
        assert!(!st.awaiting_yes());
        assert_eq!(all_uids(&store.ledger), fake.0.lock().unwrap().server.entries());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn erase_and_restore_on_a_paired_phone_send_no_tombstones() {
        let fake = Fake::new();
        let before = fake.0.lock().unwrap().server.entries();
        let (mut st, mut store, d) = paired(&fake);
        // Erase everything, as Settings does it.
        store.ledger.reset().unwrap();
        store.ledger.ensure_categories().unwrap();
        st.ledger_replaced(&mut store);
        st.sync_now();
        run(&mut st, &mut store, |st, store| store.pref(ledger::PULLED).is_some() && st.in_flight.is_none());
        assert_eq!(sent(&fake), (0, 0), "nothing went up, not one tombstone");
        assert_eq!(fake.0.lock().unwrap().server.entries(), before);
        assert_eq!(all_uids(&store.ledger), before, "the server's copy came back on the next sync");

        // A restore (Settings refuses one while paired; this is the ledger's own guard).
        let mut other = Store::in_memory().unwrap();
        other.load_sample(crate::model::today()).unwrap();
        store.ledger.import_backup(&other.ledger.export_backup("t").unwrap()).unwrap();
        st.sync_now();
        run(&mut st, &mut store, |st, _| st.awaiting_yes() || (st.in_flight.is_none() && !st.sync_asked));
        assert!(st.awaiting_yes(), "the restored ledger is not replaced without a yes");
        st.answer_first(&mut store, true);
        run(&mut st, &mut store, |st, _| st.in_flight.is_none() && !st.take_whole);
        assert_eq!(sent(&fake), (0, 0));
        let i = fake.0.lock().unwrap();
        assert_eq!(i.server.entries(), before);
        assert_eq!(i.server.replaces, 0);
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_sync_in_flight_at_a_disconnect_is_not_applied() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        let account = store.ledger.accounts().unwrap()[0].id;
        store.add(&relay_money::views::TxInput { r#type: relay_money::model::TxType::Expense, amount: 1_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
        let kept = [ledger::PULLED, ledger::GENERATION, ledger::CURSOR, ledger::PUSHED].map(|k| store.pref(k));
        st.sync_now();
        step(&mut st, &mut store, 1, &|_| {});
        assert!(st.in_flight.is_some());
        st.disconnect(&mut store);
        // The thread answers; the answer is dropped.
        std::thread::sleep(Duration::from_millis(300));
        step(&mut st, &mut store, 2, &|_| {});
        assert!(st.in_flight.is_none());
        assert!(store.pref(pref::LAST_SUCCESS).is_none());
        assert_eq!([ledger::PULLED, ledger::GENERATION, ledger::CURSOR, ledger::PUSHED].map(|k| store.pref(k)), kept, "the answer moved nothing");
        let _ = std::fs::remove_dir_all(d);
    }

    /// Connects again from the Sync page and waits for the sync that follows (or its question).
    fn connect_again(st: &mut State, store: &mut Store) {
        store.clear_pref(pref::LAST_SUCCESS).unwrap();
        st.connect_now(store);
        run(st, store, |st, store| st.awaiting_yes() || (st.paired(store) && store.pref(pref::LAST_SUCCESS).is_some() && st.in_flight.is_none()));
    }

    #[test]
    fn pairing_again_with_the_same_server_sends_what_was_made_here_meanwhile() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        // The server forgot this phone's key; an entry is made here before it is let in again.
        store.set_pref(pref::UNPAIRED, "1").unwrap();
        let mine = entry_here(&mut store);
        connect_again(&mut st, &mut store);
        assert!(!st.awaiting_yes(), "no question: nothing here is replaced");
        assert!(fake.0.lock().unwrap().server.entries().contains(&mine), "the entry went up");
        // And after a disconnect, the same.
        st.disconnect(&mut store);
        let mine = entry_here(&mut store);
        connect_again(&mut st, &mut store);
        assert!(!st.awaiting_yes());
        let i = fake.0.lock().unwrap();
        assert!(i.server.entries().contains(&mine));
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        assert_eq!(i.server.replaces, 0);
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn pairing_with_another_server_starts_from_nothing() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        st.disconnect(&mut store);
        // Where sync stood was made with another server than the one connected to now.
        store.set_pref(pref::PAIRED_SERVER, "https://elsewhere.example").unwrap();
        let mine = entry_here(&mut store);
        connect_again(&mut st, &mut store);
        assert!(st.awaiting_yes(), "the first pull asks before it replaces this phone's ledger");
        assert_eq!(st.question(), FIRST_BODY);
        assert!(!fake.0.lock().unwrap().server.entries().contains(&mine), "nothing went up");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn pairing_again_after_a_ledger_replaced_here_sends_nothing() {
        let fake = Fake::new();
        let before = fake.0.lock().unwrap().server.entries();
        let (mut st, mut store, d) = paired(&fake);
        st.disconnect(&mut store);
        // A restore while disconnected, without the page's own forgetting: the ledger's guard.
        let mut other = Store::in_memory().unwrap();
        other.load_sample(crate::model::today()).unwrap();
        store.ledger.import_backup(&other.ledger.export_backup("t").unwrap()).unwrap();
        connect_again(&mut st, &mut store);
        assert!(st.awaiting_yes(), "the restored ledger is not replaced without a yes");
        assert_eq!(sent(&fake), (0, 0), "not one row, not one tombstone");
        assert_eq!(fake.0.lock().unwrap().server.entries(), before);
        let _ = std::fs::remove_dir_all(d);
    }

    /// An entry made here, on an account of its own.
    fn entry_here(store: &mut Store) -> String {
        let account = store.ledger.account_add("Cash", relay_money::model::AccountType::Cash, 0, None, "").unwrap().id;
        let tx = relay_money::views::TxInput { r#type: relay_money::model::TxType::Expense, amount: 4_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some("Made during the pull".into()) };
        store.add(&tx).unwrap().uid
    }

    #[test]
    fn entries_made_while_the_first_pull_runs_are_not_erased_unasked() {
        let fake = Fake::new();
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        // Paired with an empty ledger: the first pull starts with no question asked.
        run(&mut st, &mut store, |st, _| st.in_flight.is_some());
        assert!(!st.awaiting_yes());
        // The person adds an account and an entry while it runs.
        let mine = entry_here(&mut store);
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(st.awaiting_yes(), "the answer is not taken over the new entry without a yes");
        assert_eq!(st.question(), CHANGED_BODY);
        assert!(all_uids(&store.ledger).contains(&mine), "the entry is still here");
        assert!(store.pref(ledger::PULLED).is_none());
        // Their yes, with the entry in view, takes the server's ledger.
        st.answer_first(&mut store, true);
        run(&mut st, &mut store, |st, store| st.in_flight.is_none() && store.pref(ledger::PULLED).is_some());
        let i = fake.0.lock().unwrap();
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        assert_eq!(i.server.replaces, 0);
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn entries_made_while_a_pull_they_asked_for_runs_are_asked_about_again() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        st.take_whole = true;
        run(&mut st, &mut store, |st, _| st.in_flight.is_some());
        let mine = entry_here(&mut store);
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(st.awaiting_yes());
        assert!(all_uids(&store.ledger).contains(&mine));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_yes_to_take_the_servers_ledger_is_not_kept_through_sync_turned_off() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        // Take the server's ledger, said yes to while a sync runs: it waits for that one.
        st.sync_now();
        step(&mut st, &mut store, 1, &|_| {});
        assert!(st.in_flight.is_some());
        st.take_whole = true;
        // Sync turned off from the page, an entry made, sync on again.
        st.set_sync(&mut store, false);
        std::thread::sleep(Duration::from_millis(300));
        step(&mut st, &mut store, 2, &|_| {});
        let mine = entry_here(&mut store);
        st.set_sync(&mut store, true);
        run(&mut st, &mut store, |st, _| st.awaiting_yes() || (st.in_flight.is_none() && !st.sync_asked));
        assert!(!st.awaiting_yes());
        assert!(all_uids(&store.ledger).contains(&mine), "the entry made while sync was off is still here");
        assert!(fake.0.lock().unwrap().server.entries().contains(&mine), "and went up as a merge");
        // However sync goes off (here the pref, as the first-run page sets it), the yes goes too.
        st.take_whole = true;
        store.set_pref(pref::PHONE_ONLY, "1").unwrap();
        step(&mut st, &mut store, 3, &|_| {});
        assert!(!st.take_whole);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn leaving_the_screen_syncs_a_change_still_in_its_debounce() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        let mine = entry_here(&mut store);
        let now = st.last_try_ns.unwrap() + 1;
        st.dirty_at = Some(now);
        // A frame a moment after the change waits for the debounce; leaving the screen does not.
        step(&mut st, &mut store, now + 1, &|_| {});
        assert!(st.in_flight.is_none());
        step_with(&mut st, &mut store, now + 1, &|_| {}, true);
        assert!(st.in_flight.is_some());
        // Its answer is applied by the frames after the app comes back.
        std::thread::sleep(Duration::from_millis(300));
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(fake.0.lock().unwrap().server.entries().contains(&mine));
        assert!(store.pref(pref::LAST_ERROR).is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_same_server_in_another_case_goes_on_from_where_sync_stood() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        st.disconnect(&mut store);
        // Kept by an older build, in the case it was typed in.
        store.set_pref(pref::PAIRED_SERVER, &State::server(&store).to_ascii_uppercase()).unwrap();
        let mine = entry_here(&mut store);
        connect_again(&mut st, &mut store);
        assert!(!st.awaiting_yes(), "no question: it is the same server");
        assert!(fake.0.lock().unwrap().server.entries().contains(&mine));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_replaced_server_ledger_asks_before_it_drops_changes_never_sent() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        {
            // Another device replaces the server's ledger.
            let mut i = fake.0.lock().unwrap();
            let mut other = relay_money::ledger::Ledger::open_in_memory().unwrap();
            other.ensure_categories().unwrap();
            other.load_sample(crate::model::today(), "USD").unwrap();
            let rows = other.sync("x", false, 0, None, &[], "t").unwrap().changes;
            i.server.ledger.sync("Kotlin", true, 0, None, &rows, "t").unwrap();
        }
        let mine = entry_here(&mut store);
        st.sync_now();
        run(&mut st, &mut store, |st, _| st.awaiting_yes());
        assert_eq!(st.question(), STALE_BODY);
        assert!(all_uids(&store.ledger).contains(&mine), "nothing here was replaced by itself");
        st.answer_first(&mut store, true);
        run(&mut st, &mut store, |st, _| st.in_flight.is_none() && !st.take_whole && !st.awaiting_yes());
        let i = fake.0.lock().unwrap();
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        assert!(!i.server.entries().contains(&mine));
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_server_ledger_made_again_empty_asks_before_this_phone_takes_it() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        let held = all_uids(&store.ledger);
        assert!(!held.is_empty());
        {
            // The server restarts on a disk that did not last: its ledger is new and empty.
            let mut i = fake.0.lock().unwrap();
            let mut fresh = relay_money::ledger::Ledger::open_in_memory().unwrap();
            fresh.ensure_categories().unwrap();
            i.server.ledger = fresh;
        }
        st.sync_now();
        run(&mut st, &mut store, |st, _| st.awaiting_yes());
        assert!(st.question().starts_with("Your server's ledger has far fewer accounts and entries than this phone (0 against "), "{}", st.question());
        assert_eq!(all_uids(&store.ledger), held, "nothing here was replaced by itself");
        // Cancel keeps this phone as it is, sync off.
        st.answer_first(&mut store, false);
        step(&mut st, &mut store, 1, &|_| {});
        assert_eq!(all_uids(&store.ledger), held);
        assert!(store.pref(pref::PHONE_ONLY).is_some());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_first_pull_asks_over_categories_changed_here() {
        let fake = Fake::new();
        let (dir_, path) = {
            let d = dir();
            let p = d.join("tally.db");
            (d, p)
        };
        let mut store = Store::open(&path).unwrap();
        let id = store.ledger.categories().unwrap()[0].id;
        store.sql().unwrap().execute("UPDATE categories SET name = 'Mine' WHERE id = ?1", [id]).unwrap();
        let mut st = State::with_net(fake.net());
        st.start(dir_.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| st.awaiting_yes());
        assert_eq!(st.question(), FIRST_BODY);
        assert!(store.ledger.categories().unwrap().iter().any(|c| c.name == "Mine"), "the renamed category is still here");
        drop(store);
        let _ = std::fs::remove_dir_all(dir_);
    }

    #[test]
    fn rows_an_ordinary_sync_brings_are_said_once() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        // What the pairing's pull said.
        let _ = (st.took_ledger(), st.received_rows());
        // Nothing new on the server: nothing to say.
        st.sync_now();
        run(&mut st, &mut store, |st, _| st.in_flight.is_none() && !st.sync_asked);
        assert!(!st.received_rows());
        // Another device adds an account: the next sync brings it (a bill due today could come
        // the same way, for `App::frame` to post).
        fake.0.lock().unwrap().server.ledger.account_add("Theirs", relay_money::model::AccountType::Cash, 0, None, "").unwrap();
        st.sync_now();
        run(&mut st, &mut store, |st, _| st.in_flight.is_none() && !st.sync_asked);
        assert!(store.ledger.accounts().unwrap().iter().any(|a| a.name == "Theirs"), "it came");
        assert!(!st.took_ledger(), "an ordinary sync, not a whole ledger");
        assert!(st.received_rows());
        assert!(!st.received_rows(), "said once");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn taking_the_servers_ledger_with_sync_off_turns_it_on_and_takes_it() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        st.set_sync(&mut store, false);
        step(&mut st, &mut store, 1, &|_| {});
        let mine = entry_here(&mut store);
        // The page's yes to "Take the server's ledger".
        st.take(&mut store);
        assert!(store.pref(pref::PHONE_ONLY).is_none(), "sync is on");
        step(&mut st, &mut store, 2, &|_| {});
        assert!(st.in_flight.is_some() && st.whole_asked, "the pull asked for started, not dropped");
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(st.took_ledger());
        assert!(!st.took_ledger(), "said once");
        let i = fake.0.lock().unwrap();
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        assert!(!all_uids(&store.ledger).contains(&mine));
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn connecting_again_after_the_server_forgot_this_phone_shows_the_code() {
        let fake = Fake::new();
        fake.0.lock().unwrap().allow_after = 1_000;
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        // Paired once; the server has since forgotten this phone. Its old key is still kept.
        st.creds = Some(Creds { host: "Cloud".into(), routes: vec!["wss://fake/join/r".into()], device: "old".into(), token: "old".into(), paired_at: 0 });
        st.connect = Connect::Connected;
        store.set_pref(pref::UNPAIRED, "1").unwrap();
        assert_eq!(st.line(&store, now_ms()).text, "Your server no longer knows this phone");
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| matches!(st.connect, Connect::Waiting { .. }));
        assert!(st.line(&store, now_ms()).text.starts_with("Allow on Discord · "));
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_cancelled_connect_says_nothing_more() {
        let fake = Fake::new();
        fake.0.lock().unwrap().allow_after = 1_000;
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| matches!(st.connect, Connect::Waiting { .. }));
        st.disconnect(&mut store);
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_millis(2_500) {
            step(&mut st, &mut store, 1, &|_| {});
            assert_eq!(st.connect, Connect::Idle, "nothing from the old attempt is taken");
            assert!(st.pending.is_none());
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(fake.0.lock().unwrap().enrolls, 1, "and nothing asked again");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_screens_request_is_answered_and_the_ones_that_erase_are_refused() {
        let fake = Fake::new();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        let t = st.request(&store, "plaid.status", json!({}));
        assert_eq!(st.reply(t), Reply::Done(Err(Failure::not_paired())));
        assert_eq!(st.reply(t), Reply::Unknown);
        fake.0.lock().unwrap().tokens.push(("dev1".into(), "tok1".into()));
        st.creds = Some(Creds { host: "Cloud".into(), routes: vec!["wss://fake/join/r".into()], device: "dev1".into(), token: "tok1".into(), paired_at: 0 });
        let t = st.request(&store, "plaid.status", json!({}));
        let start = std::time::Instant::now();
        let r = loop {
            match st.reply(t) {
                Reply::Pending => std::thread::sleep(Duration::from_millis(10)),
                r => break r,
            }
            assert!(start.elapsed() < Duration::from_secs(10));
        };
        assert_eq!(r, Reply::Done(Ok(json!({"echo": "plaid.status"}))));
        for (op, payload) in [("money.sync", json!({"replace": true})), ("money.reset", json!({})), ("money.import", json!({})), ("x.y", json!({"replace": true}))] {
            let t = st.request(&store, op, payload);
            assert!(matches!(st.reply(t), Reply::Done(Err(f)) if f.code == code::REFUSED), "{op}");
        }
        assert!(!fake.0.lock().unwrap().requests.iter().any(|o| o == "money.sync"));
        // A phone the server forgot reads as not paired.
        let _ = store.set_pref(pref::UNPAIRED, "1");
        assert!(!st.paired(&store));
    }

    #[test]
    fn the_status_reads_as_the_kotlin_app_says_it() {
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(Net { dial: Arc::new(net::Offline), http: Arc::new(net::Offline) });
        assert_eq!(st.line(&store, 0).text, "Not connected yet");
        st.connect = Connect::Waiting { code: "7F3A".into(), notified: true, expires_at: 0, server: "s".into() };
        assert_eq!(st.line(&store, 0).text, "Allow on Discord · 7F3A");
        st.creds = Some(Creds { host: String::new(), routes: vec![], device: "d".into(), token: "t".into(), paired_at: 0 });
        st.connect = Connect::Connected;
        assert_eq!(st.line(&store, 0).text, "Connected");
        store.set_pref(pref::LAST_SUCCESS, "1000").unwrap();
        store.set_pref(pref::LAST_ATTEMPT, "1000").unwrap();
        assert_eq!(st.line(&store, 61_000 + 1000).text, "Synced · 1m ago");
        store.set_pref(pref::LAST_ERROR, "Your server could not be reached").unwrap();
        store.set_pref(pref::LAST_ATTEMPT, "2000").unwrap();
        let l = st.line(&store, 3_000);
        assert_eq!((l.text.as_str(), l.tone), ("Offline · synced just now", Tone::Waiting));
        store.set_pref(pref::PHONE_ONLY, "1").unwrap();
        assert_eq!(st.line(&store, 0).tone, Tone::Off);
        assert!(device_name().starts_with("Tally RS on ") && device_name().chars().count() <= 64);
    }

    #[test]
    fn a_phone_that_slept_syncs_once_on_its_first_frame_back() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        let now = st.last_try_ns.unwrap() + 1;
        step(&mut st, &mut store, now, &|_| {});
        assert!(st.in_flight.is_none(), "not due yet by either clock");
        // A night asleep: the monotonic clock stood still, the wall clock went on.
        st.last_try_ms = Some(now_ms() - PERIOD_MS - 1);
        assert_eq!(st.wake_at(now + 1), Some(now + 1), "due at once");
        step(&mut st, &mut store, now + 1, &|_| {});
        assert!(st.in_flight.is_some(), "the first frame back syncs");
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        // One sync, not one a frame: the try it made set the wall clock's too.
        let later = st.last_try_ns.unwrap() + 1;
        step(&mut st, &mut store, later, &|_| {});
        assert!(st.in_flight.is_none());
        assert!(st.wake_at(later).is_some_and(|at| at > later + PERIOD_NS / 2));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_pairing_finished_in_the_background_wakes_the_loop_and_keeps_its_key() {
        let (wake_tx, woken) = channel::<()>();
        let wake_tx = std::sync::Mutex::new(wake_tx);
        set_waker(Box::new(move || {
            let _ = wake_tx.lock().unwrap().send(());
        }));
        let fake = Fake::new();
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        // No frames: only a wake makes the loop look, as Android's after Pause does.
        let start = std::time::Instant::now();
        let kept = |d: &Path| std::fs::read_to_string(d.join(SECRET_FILE)).ok().and_then(|t| serde_json::from_str::<Secret>(&t).ok()).is_some_and(|s| s.creds.is_some());
        while !kept(&d) {
            assert!(start.elapsed() < Duration::from_secs(20), "the key was never kept");
            if woken.recv_timeout(Duration::from_secs(1)).is_ok() && st.news() {
                step_with(&mut st, &mut store, 1, &|_| {}, true);
            }
        }
        assert!(st.paired(&store));
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_request_in_flight_with_a_forgotten_key_is_collected_on_start() {
        let fake = Fake::new();
        fake.0.lock().unwrap().allow_after = 1_000;
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        // The server forgot this phone; connecting again was asked, and the app closed waiting.
        st.creds = Some(Creds { host: "Cloud".into(), routes: vec!["wss://fake/join/r".into()], device: "old".into(), token: "old".into(), paired_at: 0 });
        store.set_pref(pref::UNPAIRED, "1").unwrap();
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| matches!(st.connect, Connect::Waiting { .. }) && st.pending.is_some());
        st.stop_connect();
        drop(st);
        // The owner taps Allow while it is closed; opened again, the request is collected.
        fake.0.lock().unwrap().allow_after = 0;
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        assert_ne!(st.connect, Connect::Connected, "connecting again, not sitting on the old key");
        run(&mut st, &mut store, |st, store| st.paired(store));
        assert_eq!(fake.0.lock().unwrap().enrolls, 1, "nothing asked again");
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_destroyed_app_leaves_the_pairing_to_the_next_one() {
        let fake = Fake::new();
        fake.0.lock().unwrap().allow_after = 1_000;
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| matches!(st.connect, Connect::Waiting { .. }) && st.pending.is_some());
        // Android destroys the activity while the connect thread waits on Allow: no stop_connect,
        // the App (and its State) simply goes.
        drop(st);
        fake.0.lock().unwrap().allow_after = 0;
        // The next activity resumes the saved request and pairs.
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        run(&mut st, &mut store, |st, store| st.paired(store));
        // Long enough for the old thread's next claim, had it gone on.
        std::thread::sleep(Duration::from_millis(2_500));
        let i = fake.0.lock().unwrap();
        assert_eq!(i.tokens.len(), 1, "only the new App's thread paired");
        assert_eq!(i.enrolls, 1, "nothing asked again");
        drop(i);
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    /// A State whose connect thread is pairing (it takes `delay_ms`), as its App is destroyed.
    fn destroyed_while_pairing(fake: &Fake, delay_ms: u64) -> (Store, PathBuf) {
        {
            let mut i = fake.0.lock().unwrap();
            i.allow_after = 0;
            i.pair_delay_ms = delay_ms;
        }
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| st.connect == Connect::Pairing && st.pending.is_some());
        // Android destroys the activity: no stop_connect, the App (and its State) simply goes.
        drop(st);
        (store, d)
    }

    #[test]
    fn an_app_destroyed_while_pairing_hands_its_key_to_the_next_one() {
        let fake = Fake::new();
        let (mut store, d) = destroyed_while_pairing(&fake, 1_000);
        // The next activity starts at once, while the old thread still pairs: it resumes the
        // request, but waits for that pairing rather than make a second device.
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        run(&mut st, &mut store, |st, store| st.paired(store));
        // Long enough for the new thread's next claim, had it gone on.
        std::thread::sleep(Duration::from_millis(2_500));
        step(&mut st, &mut store, 1, &|_| {});
        let i = fake.0.lock().unwrap();
        assert_eq!(i.tokens.len(), 1, "one device paired, and its key kept");
        assert_eq!(i.enrolls, 1, "nothing asked again");
        drop(i);
        assert_eq!(st.creds.as_ref().map(|c| c.token.as_str()), Some("tok1"));
        let secret = std::fs::read_to_string(d.join(SECRET_FILE)).unwrap();
        assert!(secret.contains("tok1") && !secret.contains("handed_on"), "kept as this App's key");
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_key_paired_as_the_app_went_away_outlives_the_process() {
        let fake = Fake::new();
        let (mut store, d) = destroyed_while_pairing(&fake, 300);
        let dir_ = Some(d.clone());
        let started = std::time::Instant::now();
        while !handoff().paired.iter().any(|(k, ..)| *k == dir_) {
            assert!(started.elapsed() < Duration::from_secs(10), "the old thread paired");
            std::thread::sleep(Duration::from_millis(20));
        }
        // The process is stopped before another App starts: only the secret file holds the key.
        assert!(take_handed_on(&dir_).is_some());
        assert!(std::fs::read_to_string(d.join(SECRET_FILE)).unwrap().contains("handed_on"));
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        run(&mut st, &mut store, |st, store| st.paired(store));
        let i = fake.0.lock().unwrap();
        assert_eq!(i.tokens.len(), 1);
        assert_eq!(i.enrolls, 1);
        drop(i);
        assert_eq!(store.pref(pref::PAIRED_SERVER).as_deref(), Some(State::server(&store).as_str()));
        st.disconnect(&mut store);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_yes_to_take_the_servers_ledger_outlives_a_sync_that_failed() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        let mine = entry_here(&mut store);
        // "Take the server's ledger", said yes to, with the server out of reach.
        fake.0.lock().unwrap().down = true;
        st.take(&mut store);
        let mut now = st.last_try_ns.unwrap() + 1;
        step(&mut st, &mut store, now, &|_| {});
        assert!(st.whole_asked);
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(store.pref(pref::LAST_ERROR).is_some(), "the sync failed");
        assert!(st.take_whole, "the yes stands");
        // Not again each frame: it waits for the next sync due by itself.
        now += 1_000_000;
        step(&mut st, &mut store, now, &|_| {});
        assert!(st.in_flight.is_none());
        assert!(st.wake_at(now).is_some_and(|at| at > now + PERIOD_NS / 2));
        // Back online, the next sync takes the server's ledger: the entry made here is not sent.
        fake.0.lock().unwrap().down = false;
        st.sync_now();
        run(&mut st, &mut store, |st, store| st.in_flight.is_none() && !st.sync_asked && store.pref(pref::LAST_ERROR).is_none());
        assert!(!st.awaiting_yes() && !st.take_whole);
        assert!(st.took_ledger());
        let i = fake.0.lock().unwrap();
        assert!(!i.server.entries().contains(&mine), "what they chose to drop never went up");
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        assert_eq!(i.server.replaces, 0);
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_yes_to_the_first_pull_is_not_asked_again_after_it_failed() {
        let fake = Fake::new();
        let d = dir();
        let mut store = Store::in_memory().unwrap();
        store.load_sample(crate::model::today()).unwrap();
        let mut st = State::with_net(fake.net());
        st.start(d.clone(), &mut store);
        st.connect_now(&mut store);
        run(&mut st, &mut store, |st, _| st.awaiting_yes());
        assert_eq!(st.question(), FIRST_BODY);
        // Their yes, and the first pull fails.
        fake.0.lock().unwrap().down = true;
        st.answer_first(&mut store, true);
        run(&mut st, &mut store, |st, store| st.in_flight.is_none() && store.pref(pref::LAST_ERROR).is_some());
        assert!(!st.awaiting_yes());
        fake.0.lock().unwrap().down = false;
        st.sync_now();
        run(&mut st, &mut store, |st, store| st.awaiting_yes() || store.pref(ledger::PULLED).is_some());
        assert!(!st.awaiting_yes(), "the yes given is not asked for again");
        let i = fake.0.lock().unwrap();
        assert_eq!(all_uids(&store.ledger), i.server.entries());
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_yes_kept_through_a_failed_sync_goes_with_sync_turned_off() {
        let fake = Fake::new();
        let (mut st, mut store, d) = paired(&fake);
        fake.0.lock().unwrap().down = true;
        st.take(&mut store);
        let now = st.last_try_ns.unwrap() + 1;
        step(&mut st, &mut store, now, &|_| {});
        run(&mut st, &mut store, |st, _| st.in_flight.is_none());
        assert!(st.take_whole);
        st.set_sync(&mut store, false);
        step(&mut st, &mut store, now + 1, &|_| {});
        assert!(!st.take_whole);
        fake.0.lock().unwrap().down = false;
        let mine = entry_here(&mut store);
        st.set_sync(&mut store, true);
        run(&mut st, &mut store, |st, store| st.in_flight.is_none() && !st.sync_asked && store.pref(pref::LAST_ERROR).is_none());
        assert!(fake.0.lock().unwrap().server.entries().contains(&mine), "an ordinary merge, the yes gone");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_forgotten_phone_says_so_after_sync_goes_off_and_on() {
        let mut store = Store::in_memory().unwrap();
        let mut st = State::with_net(Net { dial: Arc::new(net::Offline), http: Arc::new(net::Offline) });
        st.creds = Some(Creds { host: String::new(), routes: vec![], device: "d".into(), token: "t".into(), paired_at: 0 });
        st.connect = Connect::Connected;
        store.set_pref(pref::UNPAIRED, "1").unwrap();
        st.set_sync(&mut store, false);
        st.set_sync(&mut store, true);
        assert_eq!(st.line(&store, 0).text, "Your server no longer knows this phone");
    }

    struct TestHost(PathBuf);

    impl crate::app::Host for TestHost {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn log(&self, _: &str) {}
    }

    #[test]
    fn an_edit_made_while_a_sync_runs_goes_before_the_window_closes() {
        let fake = Fake::new();
        let (st, store, d) = paired(&fake);
        let mut app = App::new(Arc::new(TestHost(d.clone())), 1_000_000_000);
        app.store = store;
        app.st.sync = st;
        let first = entry_here(&mut app.store);
        app.invalidate();
        // The window closes: the change still in its debounce syncs now...
        let now = app.now_ns;
        assert!(flush(&mut app, now), "a sync runs");
        // ...and another change is made while it runs, which it does not carry.
        let account = app.store.ledger.accounts().unwrap()[0].id;
        let tx = relay_money::views::TxInput { r#type: relay_money::model::TxType::Expense, amount: 7_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some("Made while it ran".into()) };
        let second = app.store.add(&tx).unwrap().uid;
        app.invalidate();
        // The clock stands still while waiting: the follow-up starts as the first answers.
        wait_out(&mut app, Duration::from_secs(10));
        assert!(app.st.sync.in_flight.is_none(), "both answered");
        assert!(app.st.sync.dirty_at.is_none(), "nothing left unsent");
        let i = fake.0.lock().unwrap();
        assert!(i.server.entries().contains(&first));
        assert!(i.server.entries().contains(&second), "the change made during the sync went too");
        drop(i);
        let _ = std::fs::remove_dir_all(d);
    }
}
