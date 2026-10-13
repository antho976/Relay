//! The server a person hosts themselves so a phone away from home can still reach the engine:
//! the PC dials out and keeps one WebSocket open; each phone that joins the PC's room gets a
//! lane on it. The server copies lines between the two and interprets none of them — every
//! credential check still happens on the PC, in `bridge.rs`. It can read them, though, and
//! could write into a lane after the check: whoever runs it is trusted (docs/MOBILE.md §6).
//!
//! The server forwards between many sockets in one loop per host, so it never waits on any
//! one phone: each lane has a byte budget, and a phone that lets it fill is cut off and told to
//! reconnect. Every connection also gets a deadline to finish its upgrade, and the number of
//! connections, rooms and lanes per room is capped (`Limits`).
//!
//! Joining needs no credential, so a phone's limits are the ones a stranger who learned a room
//! id gets: its messages are capped well below what the host socket carries, so nothing a phone
//! sends can grow, wrapped in a lane line, into a frame that breaks the host's connection — an
//! oversized message drops that phone, never the tunnel. Both sides are pinged, and a host or a
//! phone silent for too long is dropped, so a PC that vanished stops being listed as the host.
//!
//! A host proves a room by presenting the secret whose digest names it (`registry::room_for`),
//! so anyone can open a room by making up a secret. An operator who lists the rooms it serves
//! (`Limits::rooms`) shuts strangers out entirely. Without a list, a room hosted before is
//! remembered and always let back in, while rooms the server has not seen are capped on their
//! own, so strangers holding made-up rooms cannot keep a PC whose link blinked from coming back.
//! What is remembered stays in memory unless `Limits::state` names a file to keep it in.
//!
//! Joining needs only a room id, so what one address may hold before it has said anything is
//! capped too, and a phone that joins and never sends its hello is dropped. Behind a proxy every
//! connection comes from the proxy: the caps then count nothing (loopback is never counted) unless
//! the operator says how many proxies to trust in `X-Forwarded-For` (`Limits::trust_proxy`).
//!
//! Paths: `GET /host/<room>` with `Authorization: Bearer <secret>` for the PC, `GET /join/<room>`
//! for a phone, `GET /health` for whoever runs it. The secret goes in a header, not the URL, so
//! a proxy's access log never records it; `?secret=` is still read from a PC that predates that.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::{AbortHandle, JoinHandle};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// One multiplexed message on the host's socket. Lanes are opened by the server when a phone
/// joins and closed by whichever side goes first.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Lane {
    Open { c: String },
    Data { c: String, l: String },
    Close { c: String },
}

impl Lane {
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The largest lane line either side puts on the host socket. Below tungstenite's default
/// 16 MiB frame limit, which a PC's tunnel (this one, or an older one) reads with: a line over
/// it would break the whole tunnel, so the server drops the phone that caused it instead, and
/// the PC closes the lane whose engine line would not fit (`tunnel.rs`).
pub const MAX_LANE_LINE: usize = 15 << 20;

/// What the host socket accepts in one message or frame: a lane line and its envelope.
const HOST_MESSAGE: usize = 16 << 20;

/// What a rendezvous server allows. `Default` is what `relay remote rendezvous` runs with;
/// tests shorten the clocks.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Connections held at once, hosts and phones together, upgraded or not.
    pub max_connections: usize,
    /// Rooms the server remembers as hosted before. One it remembers is always let back in, so
    /// rooms strangers made up meanwhile never keep a returning PC out.
    pub max_rooms: usize,
    /// Rooms hosted at once that the server does not remember: what strangers who make up
    /// secrets can hold. Once `max_rooms` are remembered, every further room counts here.
    pub max_new_rooms: usize,
    /// Connections from one address that have not finished their upgrade. Loopback is never
    /// counted: there it is a proxy on the same machine, or this server's own engine.
    pub pending_per_ip: usize,
    /// Phones from one address that joined a room and have not sent anything yet.
    pub quiet_lanes_per_ip: usize,
    /// How long a phone that joined has to send its first line, its hello. The app waits 10 s
    /// for the greeting and answers it at once.
    pub lane_hello: Duration,
    /// The only rooms that may be hosted or joined, when set (`relay remote rendezvous --room`).
    pub rooms: Option<HashSet<String>>,
    /// The file the rooms hosted before are kept in across restarts. Without it they are
    /// remembered until the server exits.
    pub state: Option<PathBuf>,
    /// How many proxies in front append to `X-Forwarded-For`; the address the outermost of them
    /// saw is the phone's. 0 reads no header, since anyone can send one.
    pub trust_proxy: usize,
    /// Phones one room carries at once. Joining needs no credential, so this bounds what a
    /// stranger who knows a room id can make the PC do.
    pub max_lanes_per_room: usize,
    /// The largest message a phone may send. Bus requests are small; a file write is the
    /// biggest. Escaped into a lane line it must still fit `MAX_LANE_LINE`.
    pub phone_message: usize,
    /// Bytes queued toward one phone before it is cut off. Large enough for a burst of terminal
    /// output or a re-attach catch-up on a slow link, and for the largest single lane line; a
    /// phone that drops off without a FIN fills it and is closed instead of stalling the room.
    pub lane_budget: usize,
    /// Bytes queued toward every phone on the server together; past it, the lane that would
    /// overflow it is cut.
    pub queued_total: usize,
    /// Bytes queued toward one host. Phones wait for room here rather than being cut: this is
    /// the PC reading slowly, not one phone misbehaving.
    pub host_budget: usize,
    /// How long a connection has to finish its WebSocket upgrade.
    pub handshake: Duration,
    /// The PC pings every 30 s (`tunnel.rs`); a host silent this long has gone, and leaves the room.
    pub host_silence: Duration,
    /// A live phone sends `bus.ping` every 25 s and answers pings; one silent this long has gone.
    pub phone_silence: Duration,
    /// How often the server pings each host and phone.
    pub ping_every: Duration,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_connections: 1024,
            max_rooms: 64,
            max_new_rooms: 16,
            pending_per_ip: 16,
            quiet_lanes_per_ip: 8,
            lane_hello: Duration::from_secs(15),
            rooms: None,
            state: None,
            trust_proxy: 0,
            max_lanes_per_room: 32,
            phone_message: 8 << 20,
            lane_budget: 2 * MAX_LANE_LINE,
            queued_total: 512 << 20,
            host_budget: 2 * MAX_LANE_LINE,
            handshake: Duration::from_secs(10),
            host_silence: Duration::from_secs(90),
            phone_silence: Duration::from_secs(90),
            ping_every: Duration::from_secs(30),
        }
    }
}

/// Bytes a queued line holds against its lane's and the server's budgets, given back when the
/// line is written — or dropped with its queue when the lane is cut.
struct Charge {
    n: usize,
    lane: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

impl Drop for Charge {
    fn drop(&mut self) {
        self.lane.fetch_sub(self.n, Ordering::AcqRel);
        self.total.fetch_sub(self.n, Ordering::AcqRel);
    }
}

/// The server's side of one joined phone.
struct LaneTx {
    lines: mpsc::UnboundedSender<(String, Charge)>,
    /// Bytes in `lines` not yet written to the phone's socket.
    queued: Arc<AtomicUsize>,
    /// The same, for every lane on the server.
    total: Arc<AtomicUsize>,
    budget: usize,
    total_budget: usize,
    /// The task writing to the phone. Aborting it drops the socket even while a write is stuck.
    writer: AbortHandle,
}

impl LaneTx {
    /// Queue a line without waiting. `false` means the phone is over budget or gone; the caller
    /// drops the lane.
    fn offer(&self, line: String) -> bool {
        let n = line.len();
        let lane = self.queued.fetch_add(n, Ordering::AcqRel) + n;
        let total = self.total.fetch_add(n, Ordering::AcqRel) + n;
        let charge = Charge { n, lane: self.queued.clone(), total: self.total.clone() };
        if lane > self.budget || total > self.total_budget {
            return false;
        }
        self.lines.send((line, charge)).is_ok()
    }

    fn cut(self) {
        self.writer.abort();
    }
}

/// The server's side of one host: its lines, and the byte budget phones wait on to add to them.
#[derive(Clone)]
struct HostTx {
    lines: mpsc::Sender<(String, Option<OwnedSemaphorePermit>)>,
    budget: Arc<Semaphore>,
    budget_bytes: usize,
}

impl HostTx {
    /// A phone's line, once the host's budget has room for it. `false`: the host is gone.
    async fn send_data(&self, line: String) -> bool {
        let n = line.len().clamp(1, self.budget_bytes.max(1));
        let Ok(permit) = self.budget.clone().acquire_many_owned(u32::try_from(n).unwrap_or(u32::MAX)).await else {
            return false;
        };
        self.lines.send((line, Some(permit))).await.is_ok()
    }

    /// One of the server's own small lines (`open`, `close`).
    async fn send_control(&self, line: String) -> bool {
        self.lines.send((line, None)).await.is_ok()
    }

    fn same(&self, other: &HostTx) -> bool {
        self.lines.same_channel(&other.lines)
    }
}

#[derive(Default)]
struct Room {
    /// Lines for the host's socket.
    host: Option<HostTx>,
    /// Lines for each joined phone, by lane id.
    lanes: HashMap<String, LaneTx>,
}

struct Shared {
    rooms: Mutex<HashMap<String, Room>>,
    /// Rooms hosted before (`Limits::max_rooms`). Locked after `rooms` when both are.
    known: Mutex<HashSet<String>>,
    /// One writer of `Limits::state` at a time.
    saving: Mutex<()>,
    limits: Limits,
    /// Bytes queued toward every phone.
    queued: Arc<AtomicUsize>,
    /// Connections still upgrading, by address (`Limits::pending_per_ip`).
    pending: Arc<PerIp>,
    /// Joined phones that have not said anything yet, by address (`Limits::quiet_lanes_per_ip`).
    quiet: Arc<PerIp>,
}

/// How many of something each address holds; each one is a `Held` that gives it back when dropped.
#[derive(Default)]
struct PerIp(Mutex<HashMap<IpAddr, usize>>);

struct Held {
    counts: Arc<PerIp>,
    ip: IpAddr,
}

impl PerIp {
    /// One more for `ip`, unless it already holds `cap`.
    fn take(self: &Arc<Self>, ip: IpAddr, cap: usize) -> Option<Held> {
        let mut counts = self.0.lock().unwrap();
        let held = counts.get(&ip).copied().unwrap_or(0);
        if held >= cap {
            return None;
        }
        counts.insert(ip, held + 1);
        Some(Held { counts: self.clone(), ip })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut counts = self.counts.0.lock().unwrap();
        if let Some(held) = counts.get_mut(&self.ip) {
            *held -= 1;
            if *held == 0 {
                counts.remove(&self.ip);
            }
        }
    }
}

/// The address the per-address caps count, or `None` for loopback, which they never count: a
/// proxy on this machine (every phone at once, as far as this server can tell) or a local engine.
fn counted(ip: IpAddr) -> Option<IpAddr> {
    let ip = ip.to_canonical();
    (!ip.is_loopback()).then_some(ip)
}

/// Where a request came from: the TCP peer, or with `trust` proxies in front, the address in
/// `X-Forwarded-For` that the outermost of them wrote (the `trust`-th from the right; the
/// leftmost when there are fewer). Entries further left are the phone's own say, never read.
fn client_ip(peer: IpAddr, forwarded: &str, trust: usize) -> IpAddr {
    if trust == 0 {
        return peer;
    }
    let hops: Vec<&str> = forwarded.split(',').map(str::trim).filter(|h| !h.is_empty()).collect();
    let Some(hop) = hops.len().checked_sub(trust).map(|i| hops[i]).or(hops.first().copied()) else {
        return peer;
    };
    hop.parse::<IpAddr>()
        .or_else(|_| hop.parse::<SocketAddr>().map(|a| a.ip()))
        .unwrap_or(peer)
}

/// What `Limits::state` holds.
#[derive(Serialize, Deserialize, Default)]
struct Known {
    rooms: Vec<String>,
}

fn load_known(path: &Path, max: usize) -> HashSet<String> {
    let known = match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Known>(&text) {
            Ok(k) => k.rooms,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "rendezvous state unreadable; starting with no rooms remembered");
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "rendezvous state unreadable; starting with no rooms remembered");
            Vec::new()
        }
    };
    known.into_iter().take(max).collect()
}

/// Write the rooms remembered to `Limits::state`, if set. Best effort: a failure costs only
/// what a restart would forget.
fn save_known(shared: &Shared) {
    let Some(path) = &shared.limits.state else { return };
    let _one = shared.saving.lock().unwrap();
    let mut rooms: Vec<String> = shared.known.lock().unwrap().iter().cloned().collect();
    rooms.sort();
    let body = serde_json::to_string(&Known { rooms }).unwrap_or_default();
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, path)) {
        tracing::warn!(path = %path.display(), error = %e, "rendezvous state not saved");
    }
}

type Rooms = Arc<Shared>;

pub struct RendezvousServer {
    pub local_addr: SocketAddr,
    accept: JoinHandle<()>,
}

impl Drop for RendezvousServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

impl RendezvousServer {
    pub async fn bind(addr: SocketAddr) -> Result<RendezvousServer> {
        RendezvousServer::bind_with(addr, Limits::default()).await
    }

    pub async fn bind_with(addr: SocketAddr, limits: Limits) -> Result<RendezvousServer> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding {addr}"))?;
        let local_addr = listener.local_addr()?;
        tracing::info!(addr = %local_addr, "rendezvous open");
        match &limits.rooms {
            Some(rooms) => tracing::info!(rooms = rooms.len(), "rendezvous hosts only its listed rooms"),
            None => tracing::warn!(
                new_rooms = limits.max_new_rooms,
                "no room list: anyone who makes up a secret can host a room here, up to the cap on new rooms; \
                 list yours with --room <id> or RELAY_RENDEZVOUS_ROOMS (docs/MOBILE.md §4)"
            ),
        }
        let known = limits.state.as_deref().map(|p| load_known(p, limits.max_rooms)).unwrap_or_default();
        let slots = Arc::new(Semaphore::new(limits.max_connections));
        let rooms: Rooms = Arc::new(Shared {
            rooms: Mutex::default(),
            known: Mutex::new(known),
            saving: Mutex::default(),
            limits,
            queued: Arc::default(),
            pending: Arc::default(),
            quiet: Arc::default(),
        });
        let accept = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        // Behind a trusted proxy the peer is the proxy; the phone's address is
                        // known only once the request is read, by which time it is not pending.
                        let key = if rooms.limits.trust_proxy == 0 { counted(peer.ip()) } else { None };
                        let pending = match key {
                            Some(ip) => match rooms.pending.take(ip, rooms.limits.pending_per_ip) {
                                Some(held) => Some(held),
                                None => {
                                    tracing::debug!(peer = %peer, "too many connections from one address still upgrading; refused");
                                    continue;
                                }
                            },
                            None => None,
                        };
                        let Ok(slot) = slots.clone().try_acquire_owned() else {
                            tracing::debug!(peer = %peer, "rendezvous full; connection refused");
                            continue;
                        };
                        let rooms = rooms.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle(rooms, stream, peer, pending).await {
                                tracing::debug!(peer = %peer, error = %e, "rendezvous connection ended");
                            }
                            drop(slot);
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        });
        Ok(RendezvousServer { local_addr, accept })
    }
}

enum Role {
    Host { room: String },
    /// `quiet` is this phone's place under `Limits::quiet_lanes_per_ip` until it says something.
    Join { room: String, quiet: Option<Held> },
}

/// Parse the request path into a role, refusing what is not for us before the upgrade. `bearer`
/// is the secret from the `Authorization` header; without one, a `secret=` query is read.
/// `listed` is the operator's room list, if there is one.
fn classify(path: &str, bearer: Option<&str>, listed: Option<&HashSet<String>>) -> std::result::Result<Role, (u16, &'static str)> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let secret = bearer
        .or_else(|| query.split('&').find_map(|kv| kv.strip_prefix("secret=")))
        .unwrap_or("");
    if let Some(room) = path.strip_prefix("/host/") {
        if room.is_empty() || crate::registry::room_for(secret) != room {
            return Err((403, "room secret does not open this room"));
        }
        // Worded unlike the refusal above, which a PC answers by retrying with `?secret=`.
        if listed.is_some_and(|l| !l.contains(room)) {
            return Err((403, "this rendezvous does not host that room"));
        }
        return Ok(Role::Host { room: room.to_string() });
    }
    if let Some(room) = path.strip_prefix("/join/") {
        if room.is_empty() || listed.is_some_and(|l| !l.contains(room)) {
            return Err((404, "no room"));
        }
        return Ok(Role::Join { room: room.to_string(), quiet: None });
    }
    Err((404, "relay rendezvous: /host/<room> or /join/<room>"))
}

fn refuse((status, body): (u16, &str)) -> ErrorResponse {
    let mut r = ErrorResponse::new(Some(body.to_string()));
    *r.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::from_u16(status)
        .unwrap_or(tokio_tungstenite::tungstenite::http::StatusCode::NOT_FOUND);
    r
}

async fn handle(rooms: Rooms, stream: TcpStream, peer: SocketAddr, pending: Option<Held>) -> Result<()> {
    let handshake = rooms.limits.handshake;
    let upgraded = tokio::time::timeout(handshake, upgrade(&rooms, stream, peer)).await;
    drop(pending);
    let (role, ws) = match upgraded {
        Err(_) => anyhow::bail!("no handshake within {}s", handshake.as_secs()),
        Ok(Err(e)) => return Err(e),
        Ok(Ok(None)) => return Ok(()),
        Ok(Ok(Some(upgraded))) => upgraded,
    };
    match role {
        Role::Host { room } => host(rooms, room, ws).await,
        Role::Join { room, quiet } => join(rooms, room, ws, quiet).await,
    }
}

/// The start of the request line, without consuming it: enough to tell `/health`, a host and
/// a phone apart before the upgrade, so each gets its own message limits from the first byte.
async fn peek_head(stream: &TcpStream) -> Result<Vec<u8>> {
    let mut head = [0u8; 16];
    loop {
        let n = stream.peek(&mut head).await?;
        if n == 0 || n == head.len() || head[..n].contains(&b'\n') {
            return Ok(head[..n].to_vec());
        }
        // Part of a request line: wait for the rest (the handshake deadline bounds this).
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Answer `/health` (`None`), or finish the WebSocket upgrade and say which side this is.
async fn upgrade(rooms: &Rooms, mut stream: TcpStream, peer: SocketAddr) -> Result<Option<(Role, Ws)>> {
    let head = peek_head(&stream).await?;
    if head.starts_with(b"GET /health") {
        use tokio::io::AsyncWriteExt as _;
        let body = {
            let rooms = rooms.rooms.lock().unwrap();
            let hosts = rooms.values().filter(|r| r.host.is_some()).count();
            let lanes: usize = rooms.values().map(|r| r.lanes.len()).sum();
            serde_json::json!({"relay":"rendezvous","hosts":hosts,"lanes":lanes}).to_string()
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;
        return Ok(None);
    }

    let mut role: Option<Role> = None;
    // The callback's error type is tungstenite's, an `http::Response` it will write verbatim.
    #[allow(clippy::result_large_err)]
    let on_request = |req: &Request, resp: Response| {
        let path = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/");
        let bearer = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let mut r = classify(path, bearer, rooms.limits.rooms.as_ref()).map_err(refuse)?;
        if let Role::Join { quiet, .. } = &mut r {
            let forwarded: Vec<&str> = req.headers().get_all("x-forwarded-for").iter().filter_map(|v| v.to_str().ok()).collect();
            if let Some(ip) = counted(client_ip(peer.ip(), &forwarded.join(","), rooms.limits.trust_proxy)) {
                *quiet = Some(
                    rooms
                        .quiet
                        .take(ip, rooms.limits.quiet_lanes_per_ip)
                        .ok_or_else(|| refuse((429, "too many phones from this address have not said hello yet")))?,
                );
            }
        }
        role = Some(r);
        Ok(resp)
    };
    // A host carries every lane's lines; a phone only its own requests. Which one this is
    // decides the limit before tungstenite reads a byte of a message.
    let max = if head.starts_with(b"GET /host/") { HOST_MESSAGE } else { rooms.limits.phone_message };
    // Lines are flushed one message at a time; Nagle would hold the second of two behind an ACK.
    let _ = stream.set_nodelay(true);
    let config = WebSocketConfig::default().max_message_size(Some(max)).max_frame_size(Some(max));
    let ws = tokio_tungstenite::accept_hdr_async_with_config(stream, on_request, Some(config))
        .await
        .context("websocket handshake")?;
    let role = role.expect("callback ran on success");
    Ok(Some((role, ws)))
}

type Ws = tokio_tungstenite::WebSocketStream<TcpStream>;

async fn host(rooms: Rooms, room: String, ws: Ws) -> Result<()> {
    let (mut sink, mut source) = ws.split();
    let (lines, mut rx) = mpsc::channel::<(String, Option<OwnedSemaphorePermit>)>(1024);
    let budget_bytes = rooms.limits.host_budget;
    let tx = HostTx { lines, budget: Arc::new(Semaphore::new(budget_bytes)), budget_bytes };
    let (admitted, remembered) = {
        let mut all = rooms.rooms.lock().unwrap();
        let mut known = rooms.known.lock().unwrap();
        let limits = &rooms.limits;
        // A listed room (the list already turned the rest away), one hosted before, or one held
        // now (a PC replacing its own socket) comes in whatever else is here.
        let (admitted, remembered) = if limits.rooms.is_some() || known.contains(&room) || all.contains_key(&room) {
            (true, false)
        } else if all.keys().filter(|r| !known.contains(*r)).count() >= limits.max_new_rooms {
            (false, false)
        } else if known.len() < limits.max_rooms {
            known.insert(room.clone());
            (true, true)
        } else {
            (true, false)
        };
        drop(known);
        if admitted {
            let entry = all.entry(room.clone()).or_default();
            // A second host for the same room replaces the first: a PC that restarted is the
            // common case, and its old socket is about to time out anyway.
            entry.host = Some(tx.clone());
            // Close the old host's phones so they reconnect against this one.
            for (_, lane) in entry.lanes.drain() {
                lane.cut();
            }
        }
        (admitted, remembered)
    };
    if !admitted {
        tracing::warn!(room = %room, "rendezvous holds its most new rooms; host of a room it has not seen refused");
        let _ = sink.close().await;
        return Ok(());
    }
    if remembered && rooms.limits.state.is_some() {
        let shared = rooms.clone();
        tokio::task::spawn_blocking(move || save_known(&shared));
    }
    tracing::info!(room = %room, "host joined");
    let ping_every = rooms.limits.ping_every;
    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + ping_every, ping_every);
        loop {
            let (msg, permit) = tokio::select! {
                next = rx.recv() => match next {
                    Some((line, permit)) => (Message::text(line), permit),
                    None => break,
                },
                _ = ping.tick() => (Message::Ping(Vec::new().into()), None),
            };
            let sent = sink.send(msg).await;
            drop(permit);
            if sent.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let silence = rooms.limits.host_silence;
    loop {
        // Anything from the PC counts, its pings included; silence this long is a PC gone
        // without a FIN, and the room must stop listing it.
        let msg = match tokio::time::timeout(silence, source.next()).await {
            Ok(Some(msg)) => msg,
            Ok(None) => break,
            Err(_) => {
                tracing::info!(room = %room, silent_s = silence.as_secs(), "host went silent; dropping it");
                break;
            }
        };
        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        let Ok(lane) = serde_json::from_str::<Lane>(&text) else {
            continue;
        };
        match lane {
            Lane::Data { c, l } => {
                // Never wait on one phone here: this loop carries every lane in the room.
                let mut all = rooms.rooms.lock().unwrap();
                let Some(r) = all.get_mut(&room) else { continue };
                let delivered = r.lanes.get(&c).map(|lane| lane.offer(l));
                if delivered == Some(false) {
                    tracing::info!(room = %room, lane = %c, "phone fell behind; cutting its lane");
                    if let Some(lane) = r.lanes.remove(&c) {
                        lane.cut();
                    }
                    let _ = tx.lines.try_send((Lane::Close { c }.to_line(), None));
                }
            }
            Lane::Close { c } => {
                let target = rooms.rooms.lock().unwrap().get_mut(&room).and_then(|r| r.lanes.remove(&c));
                if let Some(lane) = target {
                    lane.cut();
                }
            }
            Lane::Open { .. } => {}
        }
    }
    {
        let mut rooms = rooms.rooms.lock().unwrap();
        // Only if this host is still the room's: a replacement already took the room over, and
        // the lanes left are the replacement's.
        if let Some(r) = rooms.get_mut(&room).filter(|r| r.host.as_ref().is_some_and(|h| h.same(&tx))) {
            r.host = None;
            for (_, lane) in r.lanes.drain() {
                lane.cut();
            }
        }
        rooms.retain(|_, r| r.host.is_some() || !r.lanes.is_empty());
    }
    writer.abort();
    tracing::info!(room = %room, "host left");
    Ok(())
}

async fn join(shared: Rooms, room: String, ws: Ws, mut quiet: Option<Held>) -> Result<()> {
    let lane_id = crate::registry::random_hex(8);
    let (mut sink, mut source) = ws.split();
    let refuse = |error: &str| Message::text(serde_json::json!({"v":1,"ok":false,"error":error}).to_string());
    let limits = shared.limits.clone();
    let rooms = &shared.rooms;
    let host = {
        let rooms = rooms.lock().unwrap();
        match rooms.get(&room) {
            Some(r) if r.lanes.len() >= limits.max_lanes_per_room => Err("room.full"),
            Some(r) => r.host.clone().ok_or("host.offline"),
            None => Err("host.offline"),
        }
    };
    let host = match host {
        Ok(host) => host,
        Err(error) => {
            let _ = sink.send(refuse(error)).await;
            let _ = sink.close().await;
            return Ok(());
        }
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<(String, Charge)>();
    let queued = Arc::new(AtomicUsize::new(0));
    let writer = {
        let ping_every = limits.ping_every;
        tokio::spawn(async move {
            let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + ping_every, ping_every);
            loop {
                let (msg, charge) = tokio::select! {
                    next = rx.recv() => match next {
                        Some((line, charge)) => (Message::text(line), Some(charge)),
                        None => break,
                    },
                    _ = ping.tick() => (Message::Ping(Vec::new().into()), None),
                };
                let sent = sink.send(msg).await;
                drop(charge);
                if sent.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        })
    };
    // The writer ending, for any reason, ends this lane.
    let gone = tx.clone();
    {
        let mut rooms = rooms.lock().unwrap();
        // The host may have left or been replaced since it was looked up; the phone reconnects.
        match rooms.get_mut(&room) {
            Some(r) if r.host.as_ref().is_some_and(|h| h.same(&host)) => {
                let lane = LaneTx {
                    lines: tx,
                    queued,
                    total: shared.queued.clone(),
                    budget: limits.lane_budget,
                    total_budget: limits.queued_total,
                    writer: writer.abort_handle(),
                };
                r.lanes.insert(lane_id.clone(), lane);
            }
            _ => {
                writer.abort();
                return Ok(());
            }
        }
    }
    if host.send_control(Lane::Open { c: lane_id.clone() }.to_line()).await {
        // Joining needs no credential, so a phone that never says hello is not left holding a lane.
        let hello_by = tokio::time::Instant::now() + limits.lane_hello;
        let mut said = false;
        loop {
            let msg = tokio::select! {
                _ = gone.closed() => break,
                next = tokio::time::timeout(limits.phone_silence, source.next()) => match next {
                    Ok(Some(msg)) => msg,
                    Ok(None) | Err(_) => break,
                },
                _ = tokio::time::sleep_until(hello_by), if !said => {
                    tracing::info!(room = %room, lane = %lane_id, "phone sent nothing after joining; dropping it");
                    break;
                }
            };
            let text = match msg {
                Ok(Message::Text(t)) => t.to_string(),
                // As the direct door takes it: the bus is text, and a binary frame is read as text.
                Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
                // A message over `phone_message` is an error here: this phone goes, the host stays.
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => continue,
            };
            if !said {
                said = true;
                drop(quiet.take());
            }
            let line = Lane::Data { c: lane_id.clone(), l: text }.to_line();
            if line.len() > MAX_LANE_LINE {
                // Escaping grew it past what the host socket carries.
                tracing::info!(room = %room, lane = %lane_id, bytes = line.len(), "phone sent a line too large to forward; dropping it");
                break;
            }
            if !host.send_data(line).await {
                break;
            }
        }
    }
    let _ = host.send_control(Lane::Close { c: lane_id.clone() }.to_line()).await;
    if let Some(r) = rooms.lock().unwrap().get_mut(&room) {
        r.lanes.remove(&lane_id);
    }
    writer.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_open_the_right_role_and_nothing_else() {
        let secret = "s3cret";
        let room = crate::registry::room_for(secret);
        let host = |path: &str, bearer| matches!(classify(path, bearer, None), Ok(Role::Host { .. }));
        assert!(host(&format!("/host/{room}"), Some(secret)));
        assert!(classify(&format!("/host/{room}"), Some("wrong"), None).is_err());
        assert!(classify(&format!("/host/{room}"), None, None).is_err());
        // A PC from before the header still opens its room.
        assert!(host(&format!("/host/{room}?secret={secret}"), None));
        assert!(classify(&format!("/host/{room}?secret={secret}"), Some("wrong"), None).is_err());
        assert!(matches!(classify(&format!("/join/{room}"), None, None), Ok(Role::Join { .. })));
        assert!(classify("/join/", None, None).is_err());
        assert!(classify("/", None, None).is_err());

        // With a room list, a room off it is neither hosted, even with its own secret, nor joined.
        let other = "another-secret";
        let other_room = crate::registry::room_for(other);
        let listed: HashSet<String> = [room.clone()].into();
        assert!(matches!(classify(&format!("/host/{room}"), Some(secret), Some(&listed)), Ok(Role::Host { .. })));
        assert!(matches!(classify(&format!("/join/{room}"), None, Some(&listed)), Ok(Role::Join { .. })));
        let refused = classify(&format!("/host/{other_room}"), Some(other), Some(&listed)).err().unwrap();
        assert_eq!(refused.0, 403);
        assert_ne!(refused.1, "room secret does not open this room", "a PC would retry that with ?secret=");
        assert_eq!(classify(&format!("/join/{other_room}"), None, Some(&listed)).err().unwrap().0, 404);
    }

    #[test]
    fn forwarded_addresses_are_read_only_as_far_as_the_proxies_are_trusted() {
        let peer: IpAddr = "127.0.0.1".parse().unwrap();
        let phone: IpAddr = "203.0.113.7".parse().unwrap();
        // Trusting nothing, the header is the sender's say and is ignored.
        assert_eq!(client_ip(peer, "203.0.113.7", 0), peer);
        // One proxy: the entry it appended, never the one the phone wrote before it.
        assert_eq!(client_ip(peer, "198.51.100.1, 203.0.113.7", 1), phone);
        // Two: the outer proxy's entry, second from the right.
        assert_eq!(client_ip(peer, "198.51.100.1, 203.0.113.7, 10.0.0.2", 2), phone);
        // Fewer entries than proxies: the leftmost there is.
        assert_eq!(client_ip(peer, "203.0.113.7", 2), phone);
        assert_eq!(client_ip(peer, "", 1), peer);
        assert_eq!(client_ip(peer, "not-an-address", 1), peer);
        assert_eq!(client_ip(peer, "203.0.113.7:4431", 1), phone);
        // Loopback is never counted, however it is written.
        assert_eq!(counted(peer), None);
        assert_eq!(counted("::ffff:127.0.0.1".parse().unwrap()), None);
        assert_eq!(counted(phone), Some(phone));
    }

    #[test]
    fn an_address_holds_only_its_share_and_gets_it_back() {
        let counts = Arc::new(PerIp::default());
        let (a, b): (IpAddr, IpAddr) = ("203.0.113.7".parse().unwrap(), "203.0.113.8".parse().unwrap());
        let first = counts.take(a, 2).unwrap();
        let _second = counts.take(a, 2).unwrap();
        assert!(counts.take(a, 2).is_none(), "a third over the cap");
        assert!(counts.take(b, 2).is_some(), "another address has its own share");
        drop(first);
        assert!(counts.take(a, 2).is_some(), "a place given back is free again");
    }

    #[test]
    fn remembered_rooms_survive_a_restart_in_the_state_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rendezvous.json");
        assert!(load_known(&path, 4).is_empty(), "no file yet is no rooms");
        let limits = Limits { state: Some(path.clone()), ..Default::default() };
        let shared = Shared {
            rooms: Mutex::default(),
            known: Mutex::new(["b".to_string(), "a".to_string()].into()),
            saving: Mutex::default(),
            limits,
            queued: Arc::default(),
            pending: Arc::default(),
            quiet: Arc::default(),
        };
        save_known(&shared);
        assert_eq!(load_known(&path, 4), ["a".to_string(), "b".to_string()].into());
        assert_eq!(load_known(&path, 1).len(), 1, "never more than the server remembers");
        std::fs::write(&path, "garbage").unwrap();
        assert!(load_known(&path, 4).is_empty());
    }

    #[tokio::test]
    async fn a_lane_over_its_budget_is_refused_not_waited_on() {
        let (lines, mut rx) = mpsc::unbounded_channel();
        let writer = tokio::spawn(std::future::pending::<()>());
        let total = Arc::new(AtomicUsize::new(0));
        let lane = LaneTx { lines, queued: Arc::default(), total: total.clone(), budget: 400, total_budget: 1000, writer: writer.abort_handle() };
        let chunk = "x".repeat(100);
        for _ in 0..4 {
            assert!(lane.offer(chunk.clone()));
        }
        assert!(!lane.offer("y".into()), "the budget is a ceiling");
        assert_eq!(total.load(Ordering::Acquire), 400, "a refused line holds nothing");
        // What the writer sends frees budget again.
        drop(rx.recv().await.unwrap());
        assert!(lane.offer("y".into()));
        assert_eq!(total.load(Ordering::Acquire), 301);
        // Lines dropped with a cut lane's queue give their bytes back to the server.
        lane.cut();
        drop(rx);
        assert!(writer.await.unwrap_err().is_cancelled());
        assert_eq!(total.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn every_lane_shares_one_server_budget() {
        let total = Arc::new(AtomicUsize::new(0));
        let writer = tokio::spawn(std::future::pending::<()>());
        let mk = |rx: &mut Vec<_>| {
            let (lines, r) = mpsc::unbounded_channel();
            rx.push(r);
            LaneTx { lines, queued: Arc::default(), total: total.clone(), budget: 400, total_budget: 500, writer: writer.abort_handle() }
        };
        let mut queues = Vec::new();
        let (a, b) = (mk(&mut queues), mk(&mut queues));
        assert!(a.offer("x".repeat(300)));
        assert!(!b.offer("x".repeat(300)), "under its own budget, over the server's");
        assert!(b.offer("x".repeat(200)));
        writer.abort();
    }

    #[test]
    fn lanes_serialize_compactly() {
        assert_eq!(Lane::Open { c: "a".into() }.to_line(), r#"{"t":"open","c":"a"}"#);
        let back: Lane = serde_json::from_str(r#"{"t":"data","c":"a","l":"{}"}"#).unwrap();
        assert_eq!(back, Lane::Data { c: "a".into(), l: "{}".into() });
    }
}
