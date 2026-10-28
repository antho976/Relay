//! The Unix-socket door (BUS.md §6.2): newline-delimited JSON, one `Request` per line,
//! responses correlated by id, `bus.subscribe` turns the connection into a subscriber.
//! One engine per instance, enforced with a lock file ([`InstanceLock`]).

use crate::engine::{Door, Engine};
use crate::paths::Instance;
use anyhow::{Context, Result};
use relay_bus::envelope::{Event, Frame, Response, ENVELOPE_V};
use relay_bus::error::BusError;
use relay_bus::ops::bus::{SubscribeIn, SubscribeOut, WaitIn, WaitOut};
use relay_bus::registry::Op;
use relay_bus::{Empty, Request};
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Why we could not become the engine for this instance.
#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("engine already running for instance {instance} (pid {pid:?}) at {socket}")]
    AlreadyRunning {
        instance: Instance,
        pid: Option<u32>,
        socket: PathBuf,
    },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A bound, listening socket door. Drop = stop accepting, end every connection, unlink the
/// socket, release the lock.
pub struct SocketServer {
    pub path: PathBuf,
    _lock: File,
    accept: Option<JoinHandle<()>>,
    conns: Arc<std::sync::Mutex<Conns>>,
}

/// The connections the door is serving. Closing the door ends them too, so nothing keeps
/// dispatching on a socket nobody can reach while the engine shuts down (RA-333).
#[derive(Default)]
struct Conns {
    closed: bool,
    set: tokio::task::JoinSet<()>,
}

/// The instance's lock, held: nobody else is or will become this instance's engine. Taken
/// before the store is opened, so a second `relay serve` is told the instance is taken rather
/// than failing on a locked store, or running recovery on another one first (RA-624).
pub struct InstanceLock {
    instance: Instance,
    lock: File,
    sock_path: PathBuf,
}

impl InstanceLock {
    /// In the instance's runtime dir (BUS.md §6.2).
    pub fn take(instance: Instance) -> Result<InstanceLock, BindError> {
        Self::take_in(instance, instance.runtime_dir())
    }

    /// Same, in an explicit directory (tests).
    pub fn take_in(inst: Instance, dir: PathBuf) -> Result<InstanceLock, BindError> {
        // The lock and the socket are only as safe as the directory holding them: one another
        // user owns lets them plant a symlink as the lock (truncated below) or swap the socket
        // for their own (RA-013). So it must be ours, private, and in a parent we trust.
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .with_context(|| format!("creating {}", dir.display()))?;
        crate::paths::check_parent_dir(&dir).with_context(|| format!("refusing runtime dir {}", dir.display()))?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod {}", dir.display()))?;
        crate::paths::check_private_dir(&dir).with_context(|| "refusing the runtime dir")?;
        let lock_path = dir.join(inst.lock_file());
        let sock_path = dir.join(inst.socket_file());

        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&lock_path)
            .with_context(|| format!("opening {}", lock_path.display()))?;
        {
            use std::os::unix::fs::MetadataExt;
            let meta = lock.metadata().with_context(|| format!("stat {}", lock_path.display()))?;
            if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } {
                return Err(BindError::Other(anyhow::anyhow!(
                    "{} is not a regular file of ours; refusing to use it as the lock",
                    lock_path.display()
                )));
            }
        }
        let rc = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            // Someone holds the lock, so someone is this instance's engine — answering or not.
            // A held lock with a dead socket (a hung engine, a crashed one's child) is refused,
            // not taken over: only a socket nobody holds the lock for is stale (BUS.md §6.2).
            let pid = std::fs::read_to_string(&lock_path)
                .ok()
                .and_then(|s| s.trim().parse().ok());
            return Err(BindError::AlreadyRunning {
                instance: inst,
                pid,
                socket: sock_path,
            });
        }
        {
            use std::io::Write;
            let mut l = &lock;
            let _ = l.set_len(0);
            let _ = write!(l, "{}", std::process::id());
        }
        Ok(InstanceLock { instance: inst, lock, sock_path })
    }

    /// Evict the stale socket, bind, and start accepting.
    pub async fn bind(self, engine: Arc<Engine>) -> Result<SocketServer, BindError> {
        let InstanceLock { instance: inst, lock, sock_path } = self;
        // We hold the lock: anything at the socket path is stale.
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        let listener = UnixListener::bind(&sock_path)
            .with_context(|| format!("binding {}", sock_path.display()))?;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| "chmod socket")?;
        *engine.socket_path.lock().unwrap() = Some(sock_path.display().to_string());
        tracing::info!(socket = %sock_path.display(), instance = %inst, "socket door open");

        let eng = engine.clone();
        let recent = Arc::new(Recent::new(&engine));
        let conns = Arc::new(std::sync::Mutex::new(Conns::default()));
        let served = conns.clone();
        let accept = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let mut conns = served.lock().unwrap_or_else(|poison| poison.into_inner());
                        if conns.closed {
                            break;
                        }
                        while conns.set.try_join_next().is_some() {}
                        let e = eng.clone();
                        let recent = recent.clone();
                        conns.set.spawn(async move {
                            if let Err(err) = handle_conn(e, recent, stream).await {
                                tracing::debug!(error = %err, "connection ended");
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        });
        Ok(SocketServer {
            path: sock_path,
            _lock: lock,
            accept: Some(accept),
            conns,
        })
    }
}

impl SocketServer {
    /// Take the instance lock, evict a stale socket, bind, and start accepting — in the
    /// instance's runtime dir (BUS.md §6.2).
    pub async fn start(engine: Arc<Engine>) -> Result<SocketServer, BindError> {
        InstanceLock::take(engine.instance)?.bind(engine).await
    }

    /// Same, in an explicit directory (tests).
    pub async fn start_in(engine: Arc<Engine>, dir: PathBuf) -> Result<SocketServer, BindError> {
        InstanceLock::take_in(engine.instance, dir)?.bind(engine).await
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        if let Some(h) = self.accept.take() {
            h.abort();
        }
        // Aborting a connection runs its drops — streams, mirrors, watch leases, borrowed
        // sizes — as it would had the client hung up.
        let mut conns = self.conns.lock().unwrap_or_else(|poison| poison.into_inner());
        conns.closed = true;
        conns.set.abort_all();
        drop(conns);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Is there a live engine behind this socket? (connect + `bus.ping`, 500 ms budget)
pub async fn probe(path: &Path) -> bool {
    let fut = async {
        let stream = UnixStream::connect(path).await.ok()?;
        let (r, mut w) = stream.into_split();
        let req = Request::new(relay_bus::Actor::User, "bus.ping", serde_json::json!({}));
        let mut line = serde_json::to_string(&req).ok()?;
        line.push('\n');
        w.write_all(line.as_bytes()).await.ok()?;
        let mut lines = BufReader::new(r).lines();
        let resp = lines.next_line().await.ok()??;
        let resp: Response = serde_json::from_str(&resp).ok()?;
        Some(resp.ok)
    };
    matches!(
        tokio::time::timeout(std::time::Duration::from_millis(500), fut).await,
        Ok(Some(true))
    )
}

/// How much queued output one `write_all` may carry. Large enough to swallow a burst from
/// every attached terminal, small enough that a slow reader never parks megabytes here.
const WRITE_BATCH_BYTES: usize = 256 * 1024;

/// The constant prefix of every `pty` frame on one attachment: the session name is escaped
/// once, here, rather than on every frame.
fn pty_frame_head(session: &str) -> String {
    let name = serde_json::to_string(session).unwrap_or_else(|_| String::from("\"\""));
    format!("{{\"v\":{ENVELOPE_V},\"stream\":\"pty\",\"session\":{name}")
}

/// One `pty` frame as its wire line, formatted in a single pass over its own bytes.
///
/// `serde_json` would first allocate a `String` for the base64 payload, wrap it in a `Value`,
/// then re-scan all of it looking for characters JSON must escape. Base64's alphabet contains
/// none of them and every other field here is a number or a fixed token, so the line can be
/// built directly — on the one path that carries every byte every agent ever prints.
fn pty_frame_line(head: &str, epoch: u64, seq: u64, data: &[u8]) -> String {
    use base64::Engine as _;
    use std::fmt::Write as _;
    let mut line = String::with_capacity(head.len() + 48 + data.len().div_ceil(3) * 4);
    line.push_str(head);
    let _ = write!(line, ",\"epoch\":{epoch},\"seq\":{seq},\"data\":\"");
    base64::engine::general_purpose::STANDARD.encode_string(data, &mut line);
    line.push_str("\"}");
    line
}

/// `bus.wait {matching}`: every top-level key given must be present in the event payload with
/// an equal value. Lets a waiter take *its* answer and not the first one of the same name.
fn payload_matches(payload: &serde_json::Value, matching: Option<&serde_json::Value>) -> bool {
    let Some(wanted) = matching.and_then(serde_json::Value::as_object) else { return true };
    wanted.iter().all(|(key, value)| payload.get(key) == Some(value))
}

/// The exception request a `bus.wait` is waiting on: `guardrail.request_resolved` with a
/// `request_id` to match. That answer is stored, so a wait begun after it can still see it.
fn exception_waited(filter: &Filter, matching: Option<&serde_json::Value>) -> Option<relay_bus::types::Id> {
    if filter.0.is_empty() || !filter.matches(crate::guardrail::grants::RESOLVED_EVENT) {
        return None;
    }
    matching?.get("request_id")?.as_i64()
}

/// How far back a `bus.wait` may look for an event it was too late to see live.
const REPLAY_WINDOW: Duration = Duration::from_secs(10 * 60);
const REPLAY_EVENTS: usize = 1024;

/// The events an agent's `bus.wait` can still be handed after they fired (RA-127).
///
/// A refusal names the event to wait for (`device.busy` → `device.lease.released`), but the
/// wait subscribes only when it arrives: a release in the seconds an agent spends between the
/// two was lost, and the agent slept to its timeout on a free phone. So a refused agent request
/// marks where the event stream stood when it arrived, and the agent's next wait first takes the
/// earliest matching event since that mark. Only a refusal marks, and the wait consumes the
/// mark, so an agent that waits for mail is never handed mail it was already woken for.
///
/// Recording is lazy: a receiver of its own is drained into the ring whenever an agent request
/// needs it, so nothing runs while no agent is talking to the engine.
struct Recent {
    inner: std::sync::Mutex<RecentInner>,
}

struct RecentInner {
    rx: tokio::sync::broadcast::Receiver<Event>,
    seq: u64,
    events: std::collections::VecDeque<(u64, std::time::Instant, Event)>,
    /// Agent name → (the stream position when its latest refused request arrived, when).
    refused: std::collections::HashMap<String, (u64, std::time::Instant)>,
}

impl RecentInner {
    fn drain(&mut self) -> u64 {
        use tokio::sync::broadcast::error::TryRecvError;
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    self.seq += 1;
                    self.events.push_back((self.seq, std::time::Instant::now(), event));
                }
                Err(TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
        while self.events.len() > REPLAY_EVENTS
            || self.events.front().is_some_and(|(_, at, _)| at.elapsed() > REPLAY_WINDOW)
        {
            self.events.pop_front();
        }
        self.seq
    }
}

impl Recent {
    fn new(engine: &Engine) -> Recent {
        Recent {
            inner: std::sync::Mutex::new(RecentInner {
                rx: engine.subscribe(),
                seq: 0,
                events: Default::default(),
                refused: Default::default(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RecentInner> {
        self.inner.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Where the stream stands as an agent's request arrives; `None` for anyone else.
    fn arrival(&self, req: &Request) -> Option<(String, u64)> {
        let relay_bus::Actor::Agent(name) = &req.actor else { return None };
        Some((name.clone(), self.lock().drain()))
    }

    /// That request was refused: the agent's next wait looks back to its arrival.
    fn refused(&self, arrival: Option<(String, u64)>, response: &Response) {
        let Some((name, at)) = arrival.filter(|_| !response.ok) else { return };
        let mut inner = self.lock();
        inner.refused.retain(|_, (_, when)| when.elapsed() < REPLAY_WINDOW);
        inner.refused.insert(name, (at, std::time::Instant::now()));
    }

    /// For an agent's wait: the earliest event since its last refusal that the wait would have
    /// taken live. The mark is spent either way; the wait now covers everything after it. What it
    /// may see is judged in the search, as in [`Recent::since`]: another session's event first
    /// must not spend the mark and hide its own.
    fn replay(&self, req: &Request, filter: &Filter, matching: Option<&serde_json::Value>, sees: impl Fn(&Event) -> bool) -> Option<Event> {
        let relay_bus::Actor::Agent(name) = &req.actor else { return None };
        let mut inner = self.lock();
        let (floor, _) = inner.refused.remove(name)?;
        inner.drain();
        inner.events.iter()
            .find(|(seq, _, event)| *seq > floor && filter.matches(&event.ev) && payload_matches(&event.payload, matching) && sees(event))
            .map(|(_, _, event)| event.clone())
    }

    /// For a wait given `since`: the earliest matching event the waiter may see that fired at or
    /// after it, of those still kept (the last ten minutes, at most 1024 events). What it may see
    /// is judged in the search: another project's event first must not hide its own.
    fn since(&self, since: jiff::Timestamp, filter: &Filter, matching: Option<&serde_json::Value>, sees: impl Fn(&Event) -> bool) -> Option<Event> {
        let mut inner = self.lock();
        inner.drain();
        inner.events.iter()
            .find(|(_, _, event)| {
                event.ts.parse::<jiff::Timestamp>().is_ok_and(|at| at >= since)
                    && filter.matches(&event.ev) && payload_matches(&event.payload, matching) && sees(event)
            })
            .map(|(_, _, event)| event.clone())
    }
}

/// Every task streaming output to one connection. Aborted on drop, so a connection that ends on
/// an error — any `?` in its loop — takes its streams with it instead of leaving a socket that
/// keeps writing terminal output and never reads another request (RA-126).
#[derive(Default)]
struct Streams {
    forwarder: Option<JoinHandle<()>>,
    /// pty streams attached on this connection, by session name
    attached: std::collections::HashMap<String, JoinHandle<()>>,
    /// device.run is a socket-capable op; its bounded log stream follows the response.
    runs: std::collections::HashMap<relay_bus::types::Id, JoinHandle<()>>,
}

impl Drop for Streams {
    fn drop(&mut self) {
        if let Some(h) = self.forwarder.take() {
            h.abort();
        }
        for (_, h) in self.attached.drain() {
            h.abort();
        }
        for (_, h) in self.runs.drain() {
            h.abort();
        }
    }
}

/// Event filter for `bus.subscribe {events}`: exact, `prefix.*`, or `*`. Empty = everything.
#[derive(Clone, Default)]
struct Filter(Vec<String>);

impl Filter {
    fn matches(&self, ev: &str) -> bool {
        self.0.is_empty()
            || self.0.iter().any(|p| {
                p == "*"
                    || p == ev
                    || p.strip_suffix(".*")
                        .map(|pre| ev.starts_with(pre) && ev[pre.len()..].starts_with('.'))
                        .unwrap_or(false)
            })
    }
}

// A native mirror belongs to its transport connection. Drop also runs on I/O error
// and task cancellation, so a vanished window cannot leave capture running.
struct MirrorAttachment {
    engine: Arc<Engine>,
    runtime: Arc<crate::device::MirrorRuntime>,
    task: JoinHandle<()>,
}
impl Drop for MirrorAttachment {
    fn drop(&mut self) {
        self.task.abort();
        // Out of the registry before the stop flag flips: anything that sees the mirror
        // stopped must also no longer find it.
        self.engine.mirrors.lock().unwrap().remove(&self.runtime.id);
        // First terminal state wins: a mirror already lost, failed or stopped was announced
        // as such, and closing its window must not relabel it "stopped" (RA-334).
        let stopped_here = self.runtime.finish(crate::device::MirrorState::Stopped, None, None);
        self.runtime.stop();
        if stopped_here {
            self.engine.emit_system(
                "mirror.changed",
                serde_json::json!({"mirror_id":self.runtime.id,"device":self.runtime.device,"state":"stopped"}),
            );
        }
    }
}

struct WatchLease {
    resources: bool,
    engine: Arc<Engine>,
    active: std::sync::Mutex<bool>,
}
impl Drop for WatchLease {
    fn drop(&mut self) {
        if !*self.active.lock().unwrap() {
            return;
        }
        if self.resources {
            let mut clients = self.engine.resource_watch_clients.lock().unwrap();
            *clients = clients.saturating_sub(1);
            if *clients == 0 {
                self.engine
                    .resource_watch
                    .store(false, std::sync::atomic::Ordering::SeqCst);
            }
            return;
        }
        // Stopped under the lock, so no one ever reads `clients == 0` with the watcher still
        // running. `stop()` only stores a flag and kills; nothing waits on the child here.
        let mut watch = self.engine.device_watch.lock().unwrap();
        watch.clients = watch.clients.saturating_sub(1);
        if watch.clients == 0 {
            if let Some(runtime) = watch.runtime.take() {
                runtime.stop();
            }
        }
    }
}

/// PTY sizes this connection borrowed with `session.resize {until_detach}`: a phone fits an
/// agent's terminal to its own screen while it looks at it. Each is handed back when the
/// connection detaches from the session or goes away, however it goes, unless someone resized
/// the PTY since: their size is the newer wish.
#[derive(Default)]
struct Borrowed(std::collections::HashMap<String, BorrowedSize>);
struct BorrowedSize {
    pty: Arc<crate::pty::Pty>,
    before: (u16, u16),
    set: (u16, u16),
}
impl Borrowed {
    /// After a successful resize: `prior` is the PTY and its size before it.
    fn resized(&mut self, p: &relay_bus::ops::session::ResizeIn, prior: Option<(Arc<crate::pty::Pty>, (u16, u16))>) {
        let Some((pty, before)) = prior else { return };
        if p.until_detach != Some(true) {
            // A size set for good ends the loan; there is nothing left to hand back.
            self.0.remove(&p.session);
            return;
        }
        let set = (p.cols.max(2), p.rows.max(2));
        match self.0.get_mut(&p.session) {
            // Still the same process: keep the size from before the first borrow.
            Some(held) if Arc::ptr_eq(&held.pty, &pty) => held.set = set,
            _ => {
                self.0.insert(p.session.clone(), BorrowedSize { pty, before, set });
            }
        }
    }
    fn release(&mut self, session: &str) {
        if let Some(held) = self.0.remove(session) {
            held.hand_back();
        }
    }
}
impl BorrowedSize {
    fn hand_back(self) {
        if !self.pty.exited() && self.pty.size() == self.set {
            let _ = self.pty.resize(self.before.0, self.before.1);
        }
    }
}
impl Drop for Borrowed {
    fn drop(&mut self) {
        for (_, held) in self.0.drain() {
            held.hand_back();
        }
    }
}

/// The longest request line the door reads. Above any payload the bus accepts — a 32 MiB
/// `file.read` window written back, JSON-escaped — and far below what would take the engine,
/// and every session in it, down with it (RA-335).
const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// What one read of the connection produced.
enum Incoming {
    Line(String),
    /// A line over [`MAX_LINE_BYTES`], read to its end and thrown away.
    TooLong,
}

/// Request lines with a ceiling. `Lines` buffers a line whole however long it is, so a client
/// that never wrote a newline grew the engine without bound. Cancel safe: a partial line stays
/// here across a cancelled [`LineReader::next`], so a `bus.wait` can listen for the client
/// leaving without losing what it sends.
struct LineReader {
    r: BufReader<tokio::net::unix::OwnedReadHalf>,
    buf: Vec<u8>,
    over: bool,
}

impl LineReader {
    fn new(r: tokio::net::unix::OwnedReadHalf) -> LineReader {
        LineReader { r: BufReader::new(r), buf: Vec::new(), over: false }
    }

    /// The next line without its `\n` / `\r\n`; `None` at end of stream. Not UTF-8 is an error,
    /// as it is for `Lines`.
    async fn next(&mut self) -> std::io::Result<Option<Incoming>> {
        loop {
            let chunk = self.r.fill_buf().await?;
            if chunk.is_empty() {
                // A last line without its newline is still a line.
                if std::mem::take(&mut self.over) {
                    return Ok(Some(Incoming::TooLong));
                }
                if self.buf.is_empty() {
                    return Ok(None);
                }
                return self.take().map(Some);
            }
            let end = chunk.iter().position(|b| *b == b'\n');
            let piece = &chunk[..end.unwrap_or(chunk.len())];
            if !self.over {
                if self.buf.len() + piece.len() > MAX_LINE_BYTES {
                    self.over = true;
                    self.buf = Vec::new();
                } else {
                    self.buf.extend_from_slice(piece);
                }
            }
            let used = end.map_or(chunk.len(), |at| at + 1);
            self.r.consume(used);
            if end.is_some() {
                if std::mem::take(&mut self.over) {
                    return Ok(Some(Incoming::TooLong));
                }
                return self.take().map(Some);
            }
        }
    }

    fn take(&mut self) -> std::io::Result<Incoming> {
        let mut bytes = std::mem::take(&mut self.buf);
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        String::from_utf8(bytes)
            .map(Incoming::Line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// What an agent's `bus.subscribe` / `bus.wait` may be handed (RA-337, D106): events of its own
/// project and those of none; of the mail, only what is addressed to it, broadcast to its
/// project, or its own. Devices are the one thing every project shares, so their events pass
/// whichever project's run or claim raised them — a lease held from another project is still
/// the one an agent waits on. This is consistency with the per-op checks, not confidentiality:
/// an agent can read the store file.
struct AgentScope {
    project_id: relay_bus::types::Id,
    session: String,
}

impl AgentScope {
    fn admits(&self, ev: &Event) -> bool {
        if ev.ev.starts_with("device.") {
            return true;
        }
        let project = ev.project_id.or_else(|| ev.payload.get("project_id").and_then(serde_json::Value::as_i64));
        if project.is_some_and(|project| project != self.project_id) {
            return false;
        }
        if ev.ev == "mailbox.new" {
            let mine = |key: &str| ev.payload.get(key).and_then(serde_json::Value::as_str) == Some(self.session.as_str());
            return mine("to") || mine("from") || ev.payload.get("to").and_then(serde_json::Value::as_str) == Some("*");
        }
        true
    }
}

fn visible(scope: Option<&AgentScope>, ev: &Event) -> bool {
    scope.is_none_or(|scope| scope.admits(ev))
}

/// The pipeline's checks for the ops the door answers itself (RA-336): envelope, actor and
/// token, schema, allowlist and role. They are dispatched like any request; their handlers
/// only say "answered by the door", so that answer means every check passed. `Err` is the
/// refusal to send; `Ok(Some)` is an agent, scoped to its own project and mail.
async fn door_preflight(engine: &Arc<Engine>, req: &Request) -> Result<Result<Option<AgentScope>, Response>> {
    let (e, r) = (engine.clone(), req.clone());
    let joined = tokio::task::spawn_blocking(move || {
        let id = r.id;
        let actor = r.actor.clone();
        let resp = e.dispatch(r, Door::Socket);
        if resp.error.as_ref().map(|error| error.code.as_str()) != Some(crate::handlers::bus::DOOR_ONLY) {
            return Err(resp);
        }
        let relay_bus::Actor::Agent(name) = actor else { return Ok(None) };
        // Authenticated a moment ago; a session gone since is refused, never left unscoped.
        let row = crate::sessions::by_name(&e.store.lock(), &name)
            .map_err(|_| Response::err(id, BusError::actor(format!("unknown session {name:?}"))))?;
        Ok(Some(AgentScope { project_id: row.session.project_id, session: name }))
    })
    .await;
    Ok(joined.unwrap_or_else(|error| Err(failed(req.id, &req.op, &error))))
}

/// `session.attach {}` / `session.detach {}` from an agent mean its own session, which the
/// pipeline fills in (D110). The door fills it first, so the stream it attaches or detaches is
/// the one the pipeline checked, not a session named "" (RA-340). The name is the same either
/// way: an agent's actor is its session's name.
fn own_session(mut req: Request) -> Request {
    if let (relay_bus::Actor::Agent(name), Some(object)) = (&req.actor, req.payload.as_object_mut()) {
        if !object.contains_key("session") {
            object.insert("session".into(), serde_json::Value::from(name.clone()));
        }
    }
    req
}

/// The pipeline on the blocking pool: it takes the store lock, which a runtime thread must
/// not wait on. A handler that panics is answered like an unlocked query ([`answered`]).
async fn dispatch_blocking(engine: &Arc<Engine>, req: Request) -> Result<Response> {
    let (e, id, op) = (engine.clone(), req.id, req.op.clone());
    Ok(answered(id, &op, tokio::task::spawn_blocking(move || e.dispatch(req, Door::Socket)).await))
}

/// The pipeline on this runtime thread, for a request that never waits ([`Engine::answers_from_memory`]).
/// A panic is answered here: unwound through the connection's loop, it would end the connection
/// with its streams and leave the keystroke unanswered.
fn dispatch_inline(engine: &Engine, req: Request) -> Response {
    let (id, op) = (req.id, req.op.clone());
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.dispatch(req, Door::Socket)))
        .unwrap_or_else(|_| failed(id, &op, &"it panicked"))
}

/// The answer to a request run on the blocking pool. One that panicked is still answered, with
/// an internal error: dropped, its caller waited for an answer forever, and the `?` that dropped
/// it ended the whole connection.
fn answered(id: uuid::Uuid, op: &str, joined: Result<Response, tokio::task::JoinError>) -> Response {
    joined.unwrap_or_else(|error| failed(id, op, &error))
}

/// The internal error a request that did not finish is answered with.
fn failed(id: uuid::Uuid, op: &str, why: &dyn std::fmt::Display) -> Response {
    tracing::error!(%op, %why, "a request did not finish");
    Response::err(id, BusError::internal(format!("{op} failed inside Relay")))
}

/// A `logcat` or `mirror` frame (§7); `pty` frames have their own writer.
fn stream_frame(stream: &str, run_id: Option<relay_bus::types::Id>, mirror_id: Option<relay_bus::types::Id>, seq: u64, data: serde_json::Value) -> Frame {
    Frame { v: ENVELOPE_V, stream: stream.into(), session: None, run_id, mirror_id, epoch: None, seq, data }
}

/// The event a subscriber gets in place of the ones it was too slow to take, so it knows its
/// picture is stale and refetches rather than carrying on with it (RA-338).
fn lagged_event(dropped: u64) -> Event {
    Event::new("bus.lagged", crate::time::now(), relay_bus::Actor::System, serde_json::json!({"dropped": dropped}))
}

/// How many unlocked queries one connection may have running at once.
const MAX_CONCURRENT_QUERIES: usize = 8;

async fn handle_conn(engine: Arc<Engine>, recent: Arc<Recent>, stream: UnixStream) -> Result<()> {
    // Who connected, once: a process never leaves the tree it was born in (RA-096, D165).
    let peer = crate::peer::identify(&stream, &engine);
    // A thread's agent proposes for its own thread alone (`thread.propose`): its requests carry
    // the thread to their handler, which nothing in a request can claim.
    let thread = match peer {
        crate::peer::Peer::Thread(id) | crate::peer::Peer::Assistant(id) | crate::peer::Peer::Aide(id) => Some(id),
        _ => None,
    };
    // The blocking dispatch keeps this Arc until its ownership update completes,
    // even if the socket task is cancelled while that dispatch is in flight.
    let device_watch = Arc::new(WatchLease {
        resources: false,
        engine: engine.clone(),
        active: std::sync::Mutex::new(false),
    });
    let resource_watch = Arc::new(WatchLease {
        resources: true,
        engine: engine.clone(),
        active: std::sync::Mutex::new(false),
    });
    let (r, mut w) = stream.into_split();
    let (out_tx, mut out_rx) = mpsc::channel::<String>(1024);
    let writer = tokio::spawn(async move {
        // Terminal output arrives as a burst of small lines, one per PTY read across every
        // attached session. Writing each of them separately is one syscall per frame; whatever
        // is already queued behind the first goes out with it instead.
        let mut batch: Vec<u8> = Vec::with_capacity(WRITE_BATCH_BYTES);
        // RELAY_MIRROR_TRACE: the mirror frames in this batch, logged once it is written.
        let trace = crate::mirror::trace_enabled();
        let mut traced: Vec<(i64, u64)> = Vec::new();
        while let Some(line) = out_rx.recv().await {
            batch.clear();
            if trace {
                traced.clear();
                traced.extend(crate::mirror::trace_frame_ids(&line));
            }
            batch.extend_from_slice(line.as_bytes());
            batch.push(b'\n');
            while batch.len() < WRITE_BATCH_BYTES {
                match out_rx.try_recv() {
                    Ok(next) => {
                        if trace {
                            traced.extend(crate::mirror::trace_frame_ids(&next));
                        }
                        batch.extend_from_slice(next.as_bytes());
                        batch.push(b'\n');
                    }
                    Err(_) => break,
                }
            }
            if w.write_all(&batch).await.is_err() {
                break;
            }
            if !traced.is_empty() {
                let mono_ns = crate::mirror::monotonic_ns();
                for &(mirror_id, seq) in &traced {
                    tracing::info!(target: "mirror_trace", stage = "flush", mono_ns, mirror_id, seq, batch_bytes = batch.len());
                }
            }
            // One oversized frame must not hold its buffer for the rest of the connection.
            if batch.capacity() > WRITE_BATCH_BYTES * 4 {
                batch = Vec::with_capacity(WRITE_BATCH_BYTES);
            }
        }
    });
    let mut reader = LineReader::new(r);
    let mut streams = Streams::default();

    let mut attached_mirrors: std::collections::HashMap<relay_bus::types::Id, MirrorAttachment> =
        std::collections::HashMap::new();
    // Dropped with the connection, which is when a borrowed size goes back.
    let mut borrowed = Borrowed::default();
    // This connection's offer to draw designs (`design.render.serve`), made the first time it
    // serves: an agent's connection never holds one. Dropped with the connection, which forgets
    // the offer and ends the renders it was drawing.
    let mut render_lease: Option<crate::design_render::RenderLease> = None;
    // Unlocked queries in flight on this connection. One client multiplexes all of its UI
    // traffic here, so a slow read (a PR listing, a diff, a search) used to hold up every
    // request queued behind it; D149's parallelism held only across connections (RA-015).
    let mut inflight = tokio::task::JoinSet::new();
    let concurrent = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_QUERIES));
    // A line that arrived while a `bus.wait` listened for the client leaving: next in line.
    let mut pending: Option<Incoming> = None;

    'conn: loop {
        let incoming = match pending.take() {
            Some(incoming) => incoming,
            None => match reader.next().await? {
                Some(incoming) => incoming,
                None => break,
            },
        };
        while inflight.try_join_next().is_some() {}
        let line = match incoming {
            Incoming::Line(line) => line,
            Incoming::TooLong => {
                let refused = BusError::invalid(
                    "bus.too_large",
                    format!("a request line may be at most {} MiB; this one was discarded", MAX_LINE_BYTES >> 20),
                );
                let _ = out_tx.send(serde_json::to_string(&Response::unparsed(refused))?).await;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let req = match Engine::parse(&line) {
            Ok(r) => r,
            Err(resp) => {
                let _ = out_tx.send(serde_json::to_string(&resp)?).await;
                continue;
            }
        };
        drop(line);
        // Once the engine is going down, the door takes nothing new: a session spawned now
        // would land after shutdown drained the PTYs, neither killed nor restorable (RA-333).
        if engine.is_quitting() {
            let refused = BusError::unavailable("app.quitting", "the engine is shutting down");
            let _ = out_tx.send(serde_json::to_string(&Response::err(req.id, refused))?).await;
            continue;
        }
        // `user` and `test` carry no token: what vouches for them is the process that connected.
        // A liveness ping claims nothing, so `socket::probe` works from anywhere.
        // A thread's agent may claim `user` for its own ops alone (`peer::Peer::Thread`).
        let admitted = match req.actor {
            relay_bus::Actor::User => peer.may_act_as_user_for(&req.op),
            _ => peer.may_act_as_user(),
        };
        if matches!(req.actor, relay_bus::Actor::User | relay_bus::Actor::Test)
            && !admitted
            && req.op != relay_bus::ops::bus::Ping::NAME
        {
            tracing::warn!(?peer, op = %req.op, actor = %req.actor, "refused a user claim from inside Relay's process tree");
            let refused = Response::err(req.id, peer.refusal(&req.actor.to_string()));
            let _ = out_tx.send(serde_json::to_string(&refused)?).await;
            continue;
        }
        // A keystroke is never refused into a wait, and must not queue on the replay lock.
        let arrival = if req.op == relay_bus::ops::bus::Wait::NAME || engine.answers_from_memory(&req) {
            None
        } else {
            recent.arrival(&req)
        };
        if engine.runs_unlocked(&req.op) {
            // Answered whenever it finishes; the client correlates by id. The permit bounds
            // how much of the blocking pool one connection can hold, and stops reading the
            // connection while it is spent.
            let permit = concurrent.clone().acquire_owned().await?;
            let (e, tx, recent) = (engine.clone(), out_tx.clone(), recent.clone());
            let (id, op) = (req.id, req.op.clone());
            inflight.spawn(async move {
                let resp = tokio::task::spawn_blocking(move || e.dispatch(req, Door::Socket)).await;
                drop(permit);
                let resp = answered(id, &op, resp);
                recent.refused(arrival, &resp);
                if let Ok(line) = serde_json::to_string(&resp) {
                    let _ = tx.send(line).await;
                }
            });
            continue;
        }
        // Everything else keeps its place in line: it waits for the queries sent before it,
        // so a client that reads then writes still has its read answered first. A keystroke
        // never touches the store and does not wait.
        if !engine.answers_from_memory(&req) {
            while inflight.join_next().await.is_some() {}
        }
        let resp = if matches!(req.op.as_str(), "device.watch" | "app.resources.watch") {
            let lease = if req.op == "device.watch" {
                device_watch.clone()
            } else {
                resource_watch.clone()
            };
            let (id, op) = (req.id, req.op.clone());
            let resp = tokio::task::spawn_blocking(move || {
                // A flag: a panic that poisoned it left it as true as it was.
                let mut active = lease.active.lock().unwrap_or_else(|poison| poison.into_inner());
                let on = req.payload["on"].as_bool();
                let response = lease.engine.dispatch_socket_watch(req, *active);
                if response.ok {
                    *active = on.expect("successful Watch validated its boolean payload");
                }
                response
            })
            .await;
            answered(id, &op, resp)
        } else if req.op == relay_bus::ops::bus::Subscribe::NAME {
            match door_preflight(&engine, &req).await? {
                Err(refused) => refused,
                Ok(scope) => match serde_json::from_value::<SubscribeIn>(req.payload.clone()) {
                    Err(e) => Response::err(req.id, BusError::schema(&req.op, e)),
                    Ok(p) => {
                        if let Some(h) = streams.forwarder.take() {
                            h.abort();
                        }
                        let filter = Filter(p.events.clone().unwrap_or_default());
                        let mut rx = engine.subscribe();
                        let tx = out_tx.clone();
                        let f = filter.clone();
                        streams.forwarder = Some(tokio::spawn(async move {
                            loop {
                                let line = match rx.recv().await {
                                    Ok(ev) if f.matches(&ev.ev) && visible(scope.as_ref(), &ev) => serde_json::to_string(&ev),
                                    Ok(_) => continue,
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                        tracing::warn!(lagged = n, "subscriber lagged; events dropped");
                                        serde_json::to_string(&lagged_event(n))
                                    }
                                    Err(_) => break,
                                };
                                if let Ok(s) = line {
                                    if tx.send(s).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }));
                        let subscribed = if filter.0.is_empty() {
                            vec!["*".to_string()]
                        } else {
                            filter.0.clone()
                        };
                        Response::ok(req.id, serde_json::to_value(SubscribeOut { subscribed })?)
                    }
                },
            }
        } else if req.op == relay_bus::ops::bus::Wait::NAME {
            // The wake-up an agent has instead of a poll loop. It blocks on the event
            // broadcast inside this connection's task — no timer anywhere, and the engine's
            // store lock is never held while waiting (SPEC §15, D115).
            match door_preflight(&engine, &req).await? {
                Err(refused) => refused,
                Ok(scope) => match serde_json::from_value::<WaitIn>(req.payload.clone()) {
                    Err(e) => Response::err(req.id, BusError::schema(&req.op, e)),
                    Ok(p) => {
                        let filter = Filter(p.events.clone().unwrap_or_default());
                        let timeout = Duration::from_millis(
                            p.timeout_ms.unwrap_or(60_000).clamp(1_000, 3_600_000),
                        );
                        let since = match p.since.as_deref().map(str::parse::<jiff::Timestamp>).transpose() {
                            Ok(since) => since,
                            Err(e) => {
                                let refused = BusError::invalid("bus.since", format!("since is an RFC 3339 moment, like an event's ts: {e}"));
                                let _ = out_tx.send(serde_json::to_string(&Response::err(req.id, refused))?).await;
                                continue;
                            }
                        };
                        let mut rx = engine.subscribe();
                        // Subscribed first, so an answer is either already stored or still to come.
                        let exception = exception_waited(&filter, p.matching.as_ref());
                        let answered = match exception {
                            Some(id) => stored_answer(&engine, id, p.matching.as_ref()).await?,
                            None => None,
                        };
                        // Then whatever fired between this agent's last refusal and now.
                        let answered = answered
                            .filter(|ev| visible(scope.as_ref(), ev))
                            .or_else(|| recent.replay(&req, &filter, p.matching.as_ref(), |ev| visible(scope.as_ref(), ev)))
                            .or_else(|| since.and_then(|since| recent.since(since, &filter, p.matching.as_ref(), |ev| visible(scope.as_ref(), ev))));
                        let wait = tokio::time::timeout(timeout, async {
                            if answered.is_some() {
                                return Ok::<_, anyhow::Error>(answered);
                            }
                            loop {
                                match rx.recv().await {
                                    Ok(ev) if filter.matches(&ev.ev) && payload_matches(&ev.payload, p.matching.as_ref()) && visible(scope.as_ref(), &ev) => return Ok(Some(ev)),
                                    Ok(_) => continue,
                                    // The answer may be among what was dropped. A stored one is
                                    // still there to read, rather than sleeping to the timeout (RA-338).
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                        tracing::warn!(lagged = n, "waiter lagged; events dropped");
                                        if let Some(id) = exception {
                                            if let Some(ev) = stored_answer(&engine, id, p.matching.as_ref()).await? {
                                                return Ok(Some(ev));
                                            }
                                        }
                                    }
                                    Err(_) => return Ok(None),
                                }
                            }
                        });
                        tokio::pin!(wait);
                        let waited = loop {
                            tokio::select! {
                                waited = &mut wait => break waited,
                                // A client that leaves mid-wait — a parked agent, a cancelled MCP
                                // call — ends it, rather than holding this task, its socket and its
                                // receiver for up to an hour (RA-339). A request sent meanwhile is
                                // read now and answered after the wait, in order.
                                next = reader.next(), if pending.is_none() => match next? {
                                    Some(incoming) => pending = Some(incoming),
                                    None => break 'conn,
                                },
                            }
                        };
                        let (event, timed_out) = match waited {
                            Ok(event) => (event?, false),
                            Err(_) => (None, true),
                        };
                        match serde_json::to_value(WaitOut { event, timed_out }) {
                            Ok(value) => Response::ok(req.id, value),
                            Err(e) => Response::err(req.id, BusError::internal(e.to_string())),
                        }
                    }
                },
            }
        } else if req.op == relay_bus::ops::design::RenderServe::NAME {
            // The desktop app offers to draw on this connection (render-SPEC-v2 §5.2). The engine
            // keeps only a weak sender to it, so the offer never keeps the writer alive.
            match door_preflight(&engine, &req).await? {
                Err(refused) => refused,
                Ok(_) => match serde_json::from_value::<relay_bus::ops::design::ServeIn>(req.payload.clone()) {
                    Err(e) => Response::err(req.id, BusError::schema(&req.op, e)),
                    Ok(p) => {
                        let lease = render_lease.get_or_insert_with(|| crate::design_render::RenderLease::new(&engine));
                        lease.serve(out_tx.downgrade(), p.fonts);
                        Response::ok(req.id, serde_json::to_value(Empty {})?)
                    }
                },
            }
        } else if req.op == relay_bus::ops::design::Render::NAME {
            // A picture of a design, drawn by the desktop app (render-SPEC-v2 §5.3). Checked against
            // the store and sent before the wait; the wait holds no lock, as bus.wait's.
            match door_preflight(&engine, &req).await? {
                Err(refused) => refused,
                Ok(scope) => {
                    let asker = scope.map(|scope| crate::design_render::Asker { project_id: scope.project_id, session: scope.session });
                    match crate::design_render::begin(&engine, req.payload.clone(), asker).await {
                        Err(error) => Response::err(req.id, error),
                        Ok(mut waiting) => {
                            let timer = tokio::time::sleep(waiting.timeout);
                            tokio::pin!(timer);
                            let outcome = loop {
                                tokio::select! {
                                    // An answer that lands with the timeout is still the answer.
                                    biased;
                                    answer = &mut waiting.reply => break Some(answer),
                                    () = &mut timer => break None,
                                    // As bus.wait: a client that leaves ends the render and frees
                                    // its place; a line sent meanwhile is answered after it.
                                    next = reader.next(), if pending.is_none() => match next? {
                                        Some(incoming) => pending = Some(incoming),
                                        None => break 'conn,
                                    },
                                }
                            };
                            match waiting.finish(outcome) {
                                Ok(out) => Response::ok(req.id, serde_json::to_value(out)?),
                                Err(error) => Response::err(req.id, error),
                            }
                        }
                    }
                }
            }
        } else if req.op == relay_bus::ops::bus::Unsubscribe::NAME {
            match door_preflight(&engine, &req).await? {
                Err(refused) => refused,
                Ok(_) => {
                    if let Some(h) = streams.forwarder.take() {
                        h.abort();
                    }
                    Response::ok(req.id, serde_json::to_value(Empty {})?)
                }
            }
        } else if req.op == relay_bus::ops::session::Attach::NAME {
            // control-plane validation first, then the data plane (BUS.md §7)
            let req = own_session(req);
            let parsed = serde_json::from_value::<relay_bus::ops::session::AttachIn>(req.payload.clone()).ok();
            let (e, r2, name) = (engine.clone(), req.clone(), parsed.as_ref().map(|p| p.session.clone()));
            // The PTY is found through the store, so on the blocking pool with the dispatch
            // rather than parking a runtime thread on the store lock (RA-341).
            let (resp, pty) = tokio::task::spawn_blocking(move || {
                let resp = e.dispatch(r2, Door::Socket);
                let pty = match (resp.ok, name) {
                    (true, Some(name)) => Some(crate::handlers::session::pty_by_name(&e, &name)),
                    _ => None,
                };
                (resp, pty)
            })
            .await
            .unwrap_or_else(|error| (failed(req.id, &req.op, &error), None));
            match (pty, parsed) {
                (Some(Ok((_, pty))), Some(p)) => {
                    if let Some(h) = streams.attached.remove(&p.session) {
                        h.abort();
                    }
                    let att = pty.attach(p.epoch, p.from_seq);
                    let tx = out_tx.clone();
                    let name = p.session.clone();
                    streams.attached.insert(p.session.clone(), tokio::spawn(async move {
                        let head = pty_frame_head(&name);
                        let mut rx = att.rx;
                        if !att.catch_up.is_empty() {
                            let line = pty_frame_line(&head, att.epoch, att.seq, &att.catch_up);
                            if tx.send(line).await.is_err() { return; }
                        }
                        loop {
                            match rx.recv().await {
                                Ok(fr) => {
                                    if fr.seq <= att.seq && fr.epoch == att.epoch { continue; } // already in catch-up
                                    let line = pty_frame_line(&head, fr.epoch, fr.seq, &fr.data);
                                    if tx.send(line).await.is_err() { break; }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!(lagged = n, session = %name, "pty subscriber lagged; frames dropped — client should re-attach from its last seq");
                                }
                                Err(_) => break,
                            }
                        }
                    }));
                }
                (Some(Err(e)), _) => {
                    let _ = out_tx
                        .send(serde_json::to_string(&Response::err(req.id, e))?)
                        .await;
                    continue;
                }
                _ => {}
            }
            resp
        } else if req.op == relay_bus::ops::session::Detach::NAME {
            let req = own_session(req);
            let name = req.payload.get("session").and_then(|v| v.as_str()).map(str::to_string);
            let resp = dispatch_blocking(&engine, req).await?;
            if let (true, Some(name)) = (resp.ok, name) {
                if let Some(h) = streams.attached.remove(&name) {
                    h.abort();
                }
                borrowed.release(&name);
            }
            resp
        } else if req.op == relay_bus::ops::session::Resize::NAME {
            let parsed = serde_json::from_value::<relay_bus::ops::session::ResizeIn>(req.payload.clone()).ok();
            let prior = parsed.as_ref().and_then(|p| engine.pty_named(&p.session)).map(|(_, pty)| {
                let size = pty.size();
                (pty, size)
            });
            let resp = if engine.answers_from_memory(&req) {
                dispatch_inline(&engine, req)
            } else {
                dispatch_blocking(&engine, req).await?
            };
            if let (true, Some(p)) = (resp.ok, parsed) {
                borrowed.resized(&p, prior);
            }
            resp
        } else if req.op == relay_bus::ops::device::MirrorStart::NAME {
            let response = dispatch_blocking(&engine, req).await?;
            if response.ok {
                if let Some(id) = response
                    .result
                    .as_ref()
                    .and_then(|v| v["mirror_id"].as_i64())
                {
                    // Frames on this connection are of two kinds, told apart by `data`: a base64
                    // string is one video packet; an object is the mirror's status (state, picture
                    // size, device name, and on the way out the typed code and message). The
                    // status goes first, then on every change, and a terminal one ends the
                    // stream — the window learns that the device went away from the stream it is
                    // already reading, without subscribing to anything (it used to wait forever).
                    let status_frame = move |status: &crate::device::MirrorStatus| {
                        serde_json::to_string(&stream_frame(
                            "mirror",
                            None,
                            Some(id),
                            0,
                            serde_json::to_value(status).unwrap_or_default(),
                        ))
                    };
                    match crate::handlers::device::mirror_by_id(&engine, id) {
                        Ok(runtime) => {
                            let mut rx = runtime.attach();
                            let mut status = runtime.watch_status();
                            let tx = out_tx.clone();
                            let task = tokio::spawn(async move {
                                use base64::Engine as _;
                                let frame = |item: crate::device::MirrorChunk| {
                                    // RELAY_MIRROR_TRACE: when the packet left the broadcast
                                    // queue, and what encoding it as a frame cost.
                                    let recv_ns = crate::mirror::trace_enabled().then(crate::mirror::monotonic_ns);
                                    let seq = item.seq;
                                    let line = serde_json::to_string(&stream_frame(
                                        "mirror",
                                        None,
                                        Some(id),
                                        item.seq,
                                        serde_json::Value::String(
                                            base64::engine::general_purpose::STANDARD
                                                .encode(&*item.data),
                                        ),
                                    ));
                                    if let Some(recv_ns) = recv_ns {
                                        tracing::info!(
                                            target: "mirror_trace",
                                            stage = "serialize",
                                            mono_ns = crate::mirror::monotonic_ns(),
                                            recv_ns,
                                            mirror_id = id,
                                            seq,
                                            bytes = line.as_ref().map_or(0, String::len),
                                        );
                                    }
                                    line
                                };
                                let first = status.borrow_and_update().clone();
                                let Ok(line) = status_frame(&first) else { return };
                                if tx.send(line).await.is_err() {
                                    return;
                                }
                                if first.state.is_terminal() {
                                    return;
                                }
                                loop {
                                    tokio::select! {
                                        biased;
                                        changed = status.changed() => {
                                            if changed.is_err() {
                                                break;
                                            }
                                            let now = status.borrow_and_update().clone();
                                            let Ok(line) = status_frame(&now) else { break };
                                            if tx.send(line).await.is_err() || now.state.is_terminal() {
                                                break;
                                            }
                                        }
                                        item = rx.recv() => match item {
                                            Ok(item) => {
                                                if let Ok(line) = frame(item) {
                                                    if tx.send(line).await.is_err() {
                                                        break;
                                                    }
                                                }
                                            }
                                            // A slow window skipped packets. The seq gap tells it so;
                                            // it asks the device for a fresh key frame and resumes —
                                            // no reason to end a mirror over one hiccup.
                                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                                tracing::warn!(lagged = n, mirror_id = id, "mirror subscriber lagged; packets dropped");
                                            }
                                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                                        },
                                    }
                                }
                            });
                            attached_mirrors.insert(
                                id,
                                MirrorAttachment {
                                    engine: engine.clone(),
                                    runtime,
                                    task,
                                },
                            );
                        }
                        // The worker already ended (a failure faster than this attach). Say so
                        // rather than leave the window on "Connecting".
                        Err(_) => {
                            let ended = crate::device::MirrorStatus {
                                state: crate::device::MirrorState::Failed,
                                code: Some("device.mirror_ended".into()),
                                message: Some("The mirror ended before it could start. Retry to see why.".into()),
                                ..Default::default()
                            };
                            let _ = out_tx.send(serde_json::to_string(&response)?).await;
                            if let Ok(line) = status_frame(&ended) {
                                let _ = out_tx.send(line).await;
                            }
                            continue;
                        }
                    }
                }
            }
            response
        } else if req.op == relay_bus::ops::device::RunOp::NAME
            || req.op == relay_bus::ops::device::Build::NAME
        {
            let req_id = req.id;
            let resp = dispatch_blocking(&engine, req).await?;
            if resp.ok {
                if let Some(id) = resp
                    .result
                    .as_ref()
                    .and_then(|value| value.get("id"))
                    .and_then(|value| value.as_i64())
                {
                    match crate::handlers::device::run_by_id(&engine, id) {
                        Ok(runtime) => {
                            if let Some(task) = streams.runs.remove(&id) {
                                task.abort();
                            }
                            let (cursor, history, mut rx) = runtime.attach();
                            let tx = out_tx.clone();
                            streams.runs.insert(id, tokio::spawn(async move {
                                let frame = |item: &crate::device::LogLine| {
                                    serde_json::to_string(&stream_frame(
                                        "logcat",
                                        Some(id),
                                        None,
                                        item.seq,
                                        serde_json::Value::String((*item.line).clone()),
                                    ))
                                };
                                for item in history {
                                    if let Ok(line) = frame(&item) {
                                        if tx.send(line).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                                loop {
                                    match rx.recv().await {
                                        Ok(item) if item.seq > cursor => {
                                            if let Ok(line) = frame(&item) {
                                                if tx.send(line).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                        Ok(_) => {}
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                            tracing::warn!(lagged = n, run_id = id, "device run subscriber lagged; log lines dropped");
                                        }
                                        Err(_) => break,
                                    }
                                }
                            }));
                        }
                        Err(error) => {
                            let _ = out_tx
                                .send(serde_json::to_string(&Response::err(req_id, error))?)
                                .await;
                            continue;
                        }
                    }
                }
            }
            resp
        } else if req.op == relay_bus::ops::device::RunStop::NAME {
            let run_id = req.payload.get("run_id").and_then(|value| value.as_i64());
            let resp = dispatch_blocking(&engine, req).await?;
            if resp.ok {
                if let Some(id) = run_id {
                    if let Some(task) = streams.runs.remove(&id) {
                        task.abort();
                    }
                }
            }
            resp
        } else if engine.answers_from_memory(&req) {
            // A keystroke is the one request whose latency a person can feel, and it is also the
            // one that never reaches the store. Answer it here rather than paying a trip to the
            // blocking pool and back for a map lookup and a write.
            dispatch_inline(&engine, req)
        } else if let Some(thread) = thread {
            // A thread's agent reaches only its own ops (`threads::AGENT_OPS`), none of which
            // the branches above answer.
            let (e, id, op) = (engine.clone(), req.id, req.op.clone());
            answered(id, &op, tokio::task::spawn_blocking(move || e.dispatch_from_thread(req, Door::Socket, thread)).await)
        } else {
            dispatch_blocking(&engine, req).await?
        };
        recent.refused(arrival, &resp);
        let _ = out_tx.send(serde_json::to_string(&resp)?).await;
    }
    while inflight.join_next().await.is_some() {}
    drop(streams);
    attached_mirrors.clear();
    // Before the writer is awaited: a cleanly closed connection's offer goes now, so the next
    // render is `no_renderer` at once rather than sent here.
    drop(render_lease);
    drop(device_watch);
    drop(resource_watch);
    drop(out_tx);
    let _ = writer.await;
    Ok(())
}

/// A stored answer to the exception request a `bus.wait` is waiting on, if it matches.
async fn stored_answer(engine: &Arc<Engine>, id: relay_bus::types::Id, matching: Option<&serde_json::Value>) -> Result<Option<Event>> {
    let e = engine.clone();
    let event = tokio::task::spawn_blocking(move || crate::guardrail::grants::resolved_event(&e.store.lock(), id)).await?;
    Ok(event.filter(|ev| payload_matches(&ev.payload, matching)))
}

/// A tiny client for tests and the CLI: one connection, request/response by line.
pub struct Client {
    lines: tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
    w: tokio::net::unix::OwnedWriteHalf,
}

/// Refuse an engine run by another user. Whoever listens at the socket path receives agent
/// tokens and answers guardrail gates, so a client checks who that is before saying anything
/// (RA-013).
pub fn same_user(stream: &UnixStream) -> Result<()> {
    let peer = stream.peer_cred().context("reading the engine's credentials")?.uid();
    let me = unsafe { libc::getuid() };
    anyhow::ensure!(peer == me, "the socket belongs to uid {peer}, not to this user ({me})");
    Ok(())
}

impl Client {
    pub async fn connect(path: &Path) -> Result<Client> {
        let stream = UnixStream::connect(path)
            .await
            .with_context(|| format!("connecting to {}", path.display()))?;
        same_user(&stream).with_context(|| format!("connecting to {}", path.display()))?;
        let (r, w) = stream.into_split();
        Ok(Client {
            lines: BufReader::new(r).lines(),
            w,
        })
    }
    pub async fn send_raw(&mut self, line: &str) -> Result<()> {
        self.w.write_all(line.as_bytes()).await?;
        self.w.write_all(b"\n").await?;
        Ok(())
    }
    /// Next line, parsed as either a Response or an Event.
    pub async fn next(&mut self) -> Result<Option<Line>> {
        let Some(l) = self.lines.next_line().await? else {
            return Ok(None);
        };
        let v: serde_json::Value = serde_json::from_str(&l)?;
        if v.get("ev").is_some() {
            Ok(Some(Line::Event(serde_json::from_value(v)?)))
        } else if v.get("stream").is_some() {
            Ok(Some(Line::Frame(serde_json::from_value(v)?)))
        } else {
            Ok(Some(Line::Response(serde_json::from_value(v)?)))
        }
    }
    /// Send a request and wait for its response (events that arrive meanwhile are returned
    /// via `on_event`).
    pub async fn call(
        &mut self,
        req: &Request,
        mut on_event: impl FnMut(Event),
    ) -> Result<Response> {
        self.send_raw(&serde_json::to_string(req)?).await?;
        loop {
            match self.next().await? {
                None => anyhow::bail!("connection closed"),
                Some(Line::Event(e)) => on_event(e),
                Some(Line::Frame(_)) => {}
                Some(Line::Response(r)) => {
                    if r.id == Some(req.id) || r.id.is_none() {
                        return Ok(r);
                    }
                }
            }
        }
    }
}

pub enum Line {
    Response(Response),
    Event(Event),
    Frame(Frame),
}

#[cfg(test)]
mod watch_tests {
    use super::*;
    use relay_bus::Actor;
    use serde_json::{json, Value};
    use std::os::unix::fs::PermissionsExt;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn resource_watch_leases_stop_on_disconnect_and_preserve_other_clients() {
        async fn watch(client: &mut Client, on: Value) -> Response {
            client
                .call(
                    &Request::new(Actor::User, "app.resources.watch", json!({"on":on})),
                    |_| {},
                )
                .await
                .unwrap()
        }
        async fn count(engine: &Engine, expected: usize) {
            tokio::time::timeout(Duration::from_secs(2), async {
                while *engine.resource_watch_clients.lock().unwrap() != expected {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let fixture = tempfile::tempdir().unwrap();
        let server = SocketServer::start_in(engine.clone(), fixture.path().join("socket"))
            .await
            .unwrap();
        let mut first = Client::connect(&server.path).await.unwrap();
        let mut second = Client::connect(&server.path).await.unwrap();
        assert!(watch(&mut first, json!(true)).await.ok);
        let epoch = engine
            .resource_watch_epoch
            .load(std::sync::atomic::Ordering::SeqCst);
        assert!(watch(&mut first, json!(true)).await.ok);
        assert!(!watch(&mut first, json!("yes")).await.ok);
        count(&engine, 1).await;
        assert!(watch(&mut second, json!(true)).await.ok);
        count(&engine, 2).await;
        drop(first);
        count(&engine, 1).await;
        assert!(engine
            .resource_watch
            .load(std::sync::atomic::Ordering::SeqCst));
        assert!(watch(&mut second, json!(false)).await.ok);
        assert!(watch(&mut second, json!(false)).await.ok);
        count(&engine, 0).await;
        assert!(!engine
            .resource_watch
            .load(std::sync::atomic::Ordering::SeqCst));
        assert!(watch(&mut second, json!(true)).await.ok);
        assert!(
            engine
                .resource_watch_epoch
                .load(std::sync::atomic::Ordering::SeqCst)
                > epoch
        );
        drop(second);
        count(&engine, 0).await;
        assert!(!engine
            .resource_watch
            .load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_device_watch_leases_are_owned_idempotent_and_validated() {
        async fn request(client: &mut Client, actor: Actor, payload: Value) -> Response {
            client
                .call(&Request::new(actor, "device.watch", payload), |_| {})
                .await
                .unwrap()
        }
        async fn until_clients(engine: &Engine, count: usize) {
            tokio::time::timeout(Duration::from_secs(2), async {
                while engine.device_watch.lock().unwrap().clients != count {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        let e = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let fixture = tempfile::tempdir().unwrap();
        let adb = fixture.path().join("adb");
        std::fs::write(
            &adb,
            "#!/bin/sh\nprintf 'fake-phone device\\n'\nexec sleep 20\n",
        )
        .unwrap();
        std::fs::set_permissions(&adb, std::fs::Permissions::from_mode(0o755)).unwrap();
        e.dispatch(
            Request::new(
                Actor::User,
                "settings.set",
                json!({"path":"device.adb_path","value":adb}),
            ),
            Door::InProcess,
        )
        .into_result()
        .unwrap();
        let server = SocketServer::start_in(e.clone(), fixture.path().join("socket"))
            .await
            .unwrap();
        let mut first = Client::connect(&server.path).await.unwrap();
        let mut second = Client::connect(&server.path).await.unwrap();
        assert!(
            request(&mut first, Actor::User, json!({"on":true}))
                .await
                .ok
        );
        let runtime = e.device_watch.lock().unwrap().runtime.clone().unwrap();
        assert!(
            request(&mut first, Actor::User, json!({"on":true}))
                .await
                .ok
        );
        assert_eq!(e.device_watch.lock().unwrap().clients, 1);
        assert!(
            request(&mut second, Actor::User, json!({"on":false}))
                .await
                .ok
        );
        assert_eq!(e.device_watch.lock().unwrap().clients, 1);
        assert!(!runtime.stopped());
        assert!(
            request(&mut second, Actor::User, json!({"on":true}))
                .await
                .ok
        );
        assert_eq!(e.device_watch.lock().unwrap().clients, 2);
        // Neither the duplicate path nor a lease release skips schema or actor validation.
        for on in [true, false] {
            let malformed = request(&mut first, Actor::User, json!({"on":on,"bogus":1})).await;
            assert_eq!(malformed.error.unwrap().code, "bus.schema");
            assert!(
                !request(&mut first, Actor::agent("unbound"), json!({"on":on}))
                    .await
                    .ok
            );
            assert_eq!(e.device_watch.lock().unwrap().clients, 2);
            assert!(!runtime.stopped());
        }
        assert!(
            !request(&mut first, Actor::User, json!({"on":"yes"}))
                .await
                .ok
        );
        assert!(
            request(&mut second, Actor::User, json!({"on":false}))
                .await
                .ok
        );
        assert!(
            request(&mut second, Actor::User, json!({"on":false}))
                .await
                .ok
        );
        drop(second);
        assert_eq!(e.device_watch.lock().unwrap().clients, 1);
        let mut third = Client::connect(&server.path).await.unwrap();
        assert!(
            request(&mut third, Actor::User, json!({"on":true}))
                .await
                .ok
        );
        drop(first);
        until_clients(&e, 1).await;
        assert!(!runtime.stopped(), "another window still owns the watcher");
        drop(third);
        until_clients(&e, 0).await;
        assert!(runtime.stopped());
        assert!(e.device_watch.lock().unwrap().runtime.is_none());
    }
}

#[cfg(test)]
mod conn_tests {
    use super::*;
    use relay_bus::Actor;
    use serde_json::json;

    async fn server() -> (Arc<Engine>, tempfile::TempDir, SocketServer) {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let fixture = tempfile::tempdir().unwrap();
        let server = SocketServer::start_in(engine.clone(), fixture.path().join("socket")).await.unwrap();
        (engine, fixture, server)
    }

    /// Two projects with one live agent each: `ghost` (token `tok-ghost`) in 1, `bystander`
    /// (token `tok-bystander`) in 2.
    fn agents(engine: &Engine) {
        engine.store.lock().execute_batch(
            "INSERT INTO workspaces(id, path, name, created_at, updated_at) VALUES (1, '/ws', 'ws', 'now', 'now');
             INSERT INTO projects(id, workspace_id, path, name, created_at, updated_at) VALUES
               (1, 1, '/ws/one', 'one', 'now', 'now'), (2, 1, '/ws/two', 'two', 'now', 'now');
             INSERT INTO sessions(name, project_id, provider, role, state, token, created_at, updated_at) VALUES
               ('ghost', 1, 'claude', 'builder', 'running', 'tok-ghost', 'now', 'now'),
               ('bystander', 2, 'claude', 'builder', 'running', 'tok-bystander', 'now', 'now');",
        ).unwrap();
    }

    fn agent(name: &str, op: &str, payload: serde_json::Value) -> Request {
        Request::new(Actor::agent(name), op, payload).with_token(format!("tok-{name}"))
    }

    async fn wait_as(client: &mut Client, req: Request) -> serde_json::Value {
        let resp = client.call(&req, |_| {}).await.unwrap();
        assert!(resp.ok, "{:?}", resp.error);
        resp.result.unwrap()
    }

    async fn wait(client: &mut Client, actor: Actor) -> serde_json::Value {
        let payload = json!({"events": ["device.lease.released"], "timeout_ms": 1000});
        let req = match &actor {
            Actor::Agent(name) => agent(name, "bus.wait", payload),
            _ => Request::new(actor, "bus.wait", payload),
        };
        wait_as(client, req).await
    }

    /// RA-127: a release between the refusal and the wait is still the agent's to wake on.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_wait_after_a_refusal_sees_what_fired_in_between() {
        let (engine, _fixture, server) = server().await;
        agents(&engine);
        let mut agent_conn = Client::connect(&server.path).await.unwrap();
        engine.emit_system("device.lease.released", json!({"device": "before"}));
        let refused = agent_conn.call(&agent("ghost", "session.get", json!({"session": "nope"})), |_| {}).await.unwrap();
        assert!(!refused.ok);
        engine.emit_system("device.lease.released", json!({"device": "between"}));
        let woken = wait(&mut agent_conn, Actor::agent("ghost")).await;
        assert_eq!(woken["timed_out"], false, "{woken}");
        assert_eq!(woken["event"]["payload"]["device"], "between", "only what fired after the refusal: {woken}");
        // The mark is spent: the next wait waits for a new event.
        assert_eq!(wait(&mut agent_conn, Actor::agent("ghost")).await["timed_out"], true);
        // Nobody else is handed history.
        let mut other = Client::connect(&server.path).await.unwrap();
        assert_eq!(wait(&mut other, Actor::agent("bystander")).await["timed_out"], true);
        assert_eq!(wait(&mut other, Actor::User).await["timed_out"], true);
    }

    /// Poisons one of the engine's mutexes, as a panic while it was held would.
    fn poison<T: Send>(lock: &std::sync::Mutex<T>) {
        std::thread::scope(|scope| {
            let _ = scope.spawn(|| {
                let _held = lock.lock().unwrap();
                panic!("poisoned for the test");
            }).join();
        });
        assert!(lock.is_poisoned());
    }

    /// A locked handler that panics is answered with an internal error under its id, and the
    /// connection goes on answering. A hot-path lock a panic poisoned still answers: its data
    /// stays whole, so the next request is not refused for it until a restart.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_panicking_locked_handler_is_answered_and_the_connection_lives() {
        let (engine, _fixture, server) = server().await;
        let mut conn = Client::connect(&server.path).await.unwrap();
        // `resource_watch_clients` is still read with `unwrap()`: poisoned, the watch panics.
        poison(&engine.resource_watch_clients);
        let req = Request::new(Actor::User, "app.resources.watch", json!({"on": true}));
        let resp = conn.call(&req, |_| {}).await.unwrap();
        assert_eq!(resp.id, Some(req.id));
        assert_eq!(resp.error.map(|e| e.kind), Some(relay_bus::ErrorKind::Internal));
        let ping = conn.call(&Request::new(Actor::User, "bus.ping", json!({})), |_| {}).await.unwrap();
        assert!(ping.ok, "{:?}", ping.error);

        poison(&engine.ui);
        let state = conn.call(&Request::new(Actor::User, "ui.state", json!({})), |_| {}).await.unwrap();
        assert!(state.ok, "{:?}", state.error);
    }

    /// Another project's event between the refusal and the wait neither wakes the agent nor
    /// spends the mark that its own event, fired after it, is handed by.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_wait_after_a_refusal_is_not_spent_on_an_event_it_may_not_see() {
        let (engine, _fixture, server) = server().await;
        agents(&engine);
        let mut conn = Client::connect(&server.path).await.unwrap();
        let refused = conn.call(&agent("ghost", "session.get", json!({"session": "nope"})), |_| {}).await.unwrap();
        assert!(!refused.ok);
        engine.emit_system("task.changed", json!({"id": 1, "project_id": 2}));
        engine.emit_system("task.changed", json!({"id": 2, "project_id": 1}));
        let woken = wait_as(&mut conn, agent("ghost", "bus.wait", json!({"events": ["task.changed"], "timeout_ms": 1000}))).await;
        assert_eq!((woken["timed_out"].as_bool(), woken["event"]["payload"]["id"].as_i64()), (Some(false), Some(2)), "{woken}");
    }

    /// A wait given `since` wakes on what fired after that moment even though it started later:
    /// a render that finished in the second between starting it and waiting for it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_wait_since_a_moment_takes_what_fired_after_it() {
        let (engine, _fixture, server) = server().await;
        agents(&engine);
        let mut conn = Client::connect(&server.path).await.unwrap();
        engine.emit_system("video.render.finished", json!({"id": 1}));
        let since = crate::time::now();
        engine.emit_system("video.render.finished", json!({"id": 2}));
        engine.emit_system("video.render.finished", json!({"id": 3}));
        let waited = |matching: serde_json::Value, since: Option<String>| {
            agent("ghost", "bus.wait", json!({"events": ["video.render.finished"], "matching": matching, "since": since, "timeout_ms": 1000}))
        };
        let woken = wait_as(&mut conn, waited(json!({"id": 3}), Some(since.clone()))).await;
        assert_eq!((woken["timed_out"].as_bool(), woken["event"]["payload"]["id"].as_i64()), (Some(false), Some(3)), "{woken}");
        // Before the moment is not after it; and without `since` nothing past is taken.
        assert_eq!(wait_as(&mut conn, waited(json!({"id": 1}), Some(since))).await["timed_out"], true);
        assert_eq!(wait_as(&mut conn, waited(json!({"id": 2}), None)).await["timed_out"], true);
        let bad = conn.call(&waited(json!({}), Some("yesterday".into())), |_| {}).await.unwrap();
        assert_eq!(bad.error.map(|e| e.code).as_deref(), Some("bus.since"));
        // Another project's event first does not hide this one's.
        let since = crate::time::now();
        engine.emit_system("video.render.finished", json!({"id": 7, "project_id": 2}));
        engine.emit_system("video.render.finished", json!({"id": 8, "project_id": 1}));
        let mine = wait_as(&mut conn, waited(json!({}), Some(since))).await;
        assert_eq!(mine["event"]["payload"]["id"], 8, "{mine}");
    }

    /// RA-336: the ops the door answers itself are refused exactly as the pipeline refuses.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn door_answered_ops_check_the_envelope_and_the_actor() {
        let (engine, _fixture, server) = server().await;
        agents(&engine);
        let mut client = Client::connect(&server.path).await.unwrap();
        for op in ["bus.subscribe", "bus.wait", "bus.unsubscribe"] {
            let code = |resp: Response| resp.error.map(|e| e.code).unwrap_or_default();
            let system = client.call(&Request::new(Actor::System, op, json!({})), |_| {}).await.unwrap();
            assert_eq!(code(system), "bus.actor", "{op} as system");
            let tokenless = client.call(&Request::new(Actor::agent("ghost"), op, json!({})), |_| {}).await.unwrap();
            assert_eq!(code(tokenless), "bus.actor", "{op} without a token");
            let forged = Request::new(Actor::agent("ghost"), op, json!({})).with_token("tok-bystander");
            assert_eq!(code(client.call(&forged, |_| {}).await.unwrap()), "bus.actor", "{op} with another's token");
            let mut old = Request::new(Actor::User, op, json!({}));
            old.v = 0;
            assert_eq!(code(client.call(&old, |_| {}).await.unwrap()), "bus.envelope", "{op} at v0");
            let bogus = client.call(&Request::new(Actor::User, op, json!({"bogus": 1})), |_| {}).await.unwrap();
            assert_eq!(code(bogus), "bus.schema", "{op} with a stray field");
        }
        let subscribed = client.call(&agent("ghost", "bus.subscribe", json!({})), |_| {}).await.unwrap();
        assert!(subscribed.ok, "{:?}", subscribed.error);
    }

    /// W68: the font families the desktop app reports on its render lease are what design.fonts
    /// reads, for as long as that connection lives; a lease refused by the pipeline is no lease.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_lease_keeps_its_apps_fonts_while_its_connection_lives() {
        let (engine, _fixture, server) = server().await;
        let mut app = Client::connect(&server.path).await.unwrap();
        let serve = |fonts: serde_json::Value| Request::new(Actor::User, "design.render.serve", json!({"fonts": fonts}));
        let served = app.call(&serve(json!(["Sora", "Geist", "Sora"])), |_| {}).await.unwrap();
        assert!(served.ok, "{:?}", served.error);
        assert_eq!(engine.renders.fonts().unwrap().as_slice(), ["Geist", "Sora"]);
        // Serving again on the connection replaces what it reported.
        assert!(app.call(&serve(json!(["Inter"])), |_| {}).await.unwrap().ok);
        assert_eq!(engine.renders.fonts().unwrap().as_slice(), ["Inter"]);

        let mut other = Client::connect(&server.path).await.unwrap();
        let bad = other.call(&serve(json!("Geist")), |_| {}).await.unwrap();
        assert_eq!(bad.error.unwrap().code, "bus.schema");
        assert_eq!(engine.renders.fonts().unwrap().as_slice(), ["Inter"], "the refused lease reported nothing");

        drop(app);
        tokio::time::timeout(Duration::from_secs(2), async {
            while engine.renders.fonts().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the lease went with its connection");
    }

    /// RA-337: an agent hears its own project, shared devices, and only its own mail.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_agent_is_handed_only_its_own_projects_events_and_mail() {
        let (engine, _fixture, server) = server().await;
        agents(&engine);
        let mut sub = Client::connect(&server.path).await.unwrap();
        assert!(sub.call(&agent("ghost", "bus.subscribe", json!({})), |_| {}).await.unwrap().ok);
        let mut user = Client::connect(&server.path).await.unwrap();
        assert!(user.call(&Request::new(Actor::User, "bus.subscribe", json!({})), |_| {}).await.unwrap().ok);
        // Tagged through the payload, as session.changed and the hints are.
        let emit = |ev: &str, project: Option<i64>, mut payload: serde_json::Value| {
            if let Some(project) = project {
                payload["project_id"] = json!(project);
            }
            engine.emit_system(ev, payload);
        };
        emit("task.changed", Some(2), json!({"id": 1, "mark": "theirs"}));
        emit("session.changed", Some(2), json!({"mark": "their session"}));
        emit("mailbox.new", Some(1), json!({"from": "user", "to": "someone-else", "mark": "not mine"}));
        emit("device.lease.released", Some(2), json!({"device": "phone", "mark": "shared device"}));
        emit("mailbox.new", Some(1), json!({"from": "user", "to": "ghost", "mark": "to me"}));
        emit("mailbox.new", Some(1), json!({"from": "someone-else", "to": "*", "mark": "broadcast"}));
        emit("task.changed", Some(1), json!({"id": 2, "mark": "mine"}));
        emit("settings.changed", None, json!({"mark": "global"}));
        async fn marks(client: &mut Client, count: usize) -> Vec<String> {
            let mut seen = Vec::new();
            while seen.len() < count {
                if let Line::Event(ev) = tokio::time::timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap() {
                    seen.push(ev.payload["mark"].as_str().unwrap().to_string());
                }
            }
            seen
        }
        assert_eq!(marks(&mut sub, 5).await, ["shared device", "to me", "broadcast", "mine", "global"]);
        assert_eq!(marks(&mut user, 8).await.len(), 8, "the user hears everything");
        // A waiter is scoped the same way: another project's event does not wake it.
        let mut waiter = Client::connect(&server.path).await.unwrap();
        let wake = agent("ghost", "bus.wait", json!({"events": ["task.changed"], "timeout_ms": 5000}));
        let waited = tokio::spawn(async move { wait_as(&mut waiter, wake).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        emit("task.changed", Some(2), json!({"mark": "theirs"}));
        emit("task.changed", Some(1), json!({"mark": "mine"}));
        let woken = tokio::time::timeout(Duration::from_secs(5), waited).await.unwrap().unwrap();
        assert_eq!(woken["event"]["payload"]["mark"], "mine", "{woken}");
    }

    /// RA-338: a subscriber that fell behind is told so, rather than left with a stale picture.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_lagging_subscriber_is_told_it_missed_events() {
        let (engine, _fixture, server) = server().await;
        let mut sub = Client::connect(&server.path).await.unwrap();
        assert!(sub.call(&Request::new(Actor::User, "bus.subscribe", json!({})), |_| {}).await.unwrap().ok);
        // Far more than the broadcast, the connection's queue and the socket buffer hold together.
        let filler = "x".repeat(512);
        for n in 0..40_000 {
            engine.emit_system("settings.changed", json!({"n": n, "filler": filler}));
        }
        let lagged = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(Line::Event(ev)) = sub.next().await.unwrap() {
                    if ev.ev == "bus.lagged" {
                        return ev.payload["dropped"].as_u64().unwrap();
                    }
                }
            }
        })
        .await
        .expect("no bus.lagged after dropping events");
        assert!(lagged > 0);
    }

    /// RA-339: a request sent while a wait is in progress is read, and answered after it; RA-335:
    /// a line past the ceiling is refused and the connection carries on.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_door_keeps_reading_during_a_wait_and_refuses_an_endless_line() {
        let (_engine, _fixture, server) = server().await;
        let mut client = Client::connect(&server.path).await.unwrap();
        let wait = Request::new(Actor::User, "bus.wait", json!({"events": ["nothing.happens"], "timeout_ms": 1000}));
        let ping = Request::new(Actor::User, "bus.ping", json!({}));
        client.send_raw(&serde_json::to_string(&wait).unwrap()).await.unwrap();
        client.send_raw(&serde_json::to_string(&ping).unwrap()).await.unwrap();
        let mut order = Vec::new();
        while order.len() < 2 {
            if let Some(Line::Response(r)) = client.next().await.unwrap() {
                order.push(r.id.unwrap());
            }
        }
        assert_eq!(order, [wait.id, ping.id]);

        let huge = vec![b'x'; MAX_LINE_BYTES + 1];
        client.w.write_all(&huge).await.unwrap();
        client.w.write_all(b"\n").await.unwrap();
        match client.next().await.unwrap().unwrap() {
            Line::Response(r) => assert_eq!(r.error.unwrap().code, "bus.too_large"),
            _ => panic!("expected a response"),
        }
        let r = client.call(&Request::new(Actor::User, "bus.ping", json!({})), |_| {}).await.unwrap();
        assert!(r.ok, "the connection survives an over-long line");
    }

    /// RA-333: once quitting, the door takes nothing new; closing it ends its connections.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn quitting_refuses_requests_and_closing_the_door_ends_connections() {
        let (engine, _fixture, server) = server().await;
        let mut client = Client::connect(&server.path).await.unwrap();
        assert!(client.call(&Request::new(Actor::User, "bus.subscribe", json!({})), |_| {}).await.unwrap().ok);
        engine.request_quit();
        let r = client.call(&Request::new(Actor::User, "bus.ping", json!({})), |_| {}).await.unwrap();
        assert_eq!(r.error.unwrap().code, "app.quitting");
        drop(server);
        let ended = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                engine.emit_system("settings.changed", json!({}));
                match client.next().await {
                    Ok(Some(_)) => tokio::time::sleep(Duration::from_millis(20)).await,
                    _ => return,
                }
            }
        })
        .await;
        assert!(ended.is_ok(), "a connection outlived the door");
    }

    /// RA-126: a connection that ends on an error stops streaming to its socket.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_connection_that_fails_closes_its_streams() {
        let (engine, _fixture, server) = server().await;
        let mut client = Client::connect(&server.path).await.unwrap();
        let subscribed = client.call(&Request::new(Actor::User, "bus.subscribe", json!({})), |_| {}).await.unwrap();
        assert!(subscribed.ok);
        // Not UTF-8: reading the line fails and the handler returns early.
        client.w.write_all(b"\xff\xfe\n").await.unwrap();
        let closed = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                engine.emit_system("settings.changed", json!({}));
                match client.next().await {
                    Ok(Some(_)) => tokio::time::sleep(Duration::from_millis(20)).await,
                    _ => return,
                }
            }
        })
        .await;
        assert!(closed.is_ok(), "the subscriber kept streaming after its connection failed");
    }
}

#[cfg(test)]
mod peer_tests {
    //! RA-096: the user actor is believed only from outside the engine's process tree.
    use super::*;
    use relay_bus::Actor;
    use serde_json::{json, Value};
    use std::process::Command;

    /// Connects, sends each `actor op` given on the command line, and writes the answers to
    /// `out` keyed the same way. With `detach`, it first leaves the test's process tree.
    const CLIENT: &str = r#"
import json, os, socket, sys, time, uuid
sock_path, out_path, mode = sys.argv[1], sys.argv[2], sys.argv[3]
if mode == "detach":
    parent = os.getpid()
    if os.fork() != 0:
        os._exit(0)
    while os.getppid() == parent:
        time.sleep(0.01)
s = socket.socket(socket.AF_UNIX)
s.connect(sock_path)
f = s.makefile("rw")
answers = {}
for call in sys.argv[4:]:
    actor, op = call.split(" ")
    f.write(json.dumps({"v": 1, "id": str(uuid.uuid4()), "actor": actor, "op": op, "payload": {}}) + "\n")
    f.flush()
    answers[call] = json.loads(f.readline())
with open(out_path + ".tmp", "w") as out:
    json.dump(answers, out)
os.replace(out_path + ".tmp", out_path)
"#;

    const CALLS: [&str; 5] = ["user project.list", "test project.list", "user bus.subscribe", "user bus.ping", "agent:calm-otter project.list"];

    fn python() -> bool {
        let found = Command::new("python3").args(["-c", ""]).status().is_ok_and(|status| status.success());
        if !found {
            eprintln!("skipping: no python3 to act as a foreign client");
        }
        found
    }

    struct Fixture {
        engine: Arc<Engine>,
        dir: tempfile::TempDir,
        server: SocketServer,
    }

    impl Fixture {
        async fn new() -> Fixture {
            let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("client.py"), CLIENT).unwrap();
            let server = SocketServer::start_in(engine.clone(), dir.path().join("run")).await.unwrap();
            Fixture { engine, dir, server }
        }

        fn args(&self, mode: &str) -> Vec<String> {
            let mut args = vec![
                self.dir.path().join("client.py").display().to_string(),
                self.server.path.display().to_string(),
                self.dir.path().join("out.json").display().to_string(),
                mode.to_string(),
            ];
            args.extend(CALLS.iter().map(|call| call.to_string()));
            args
        }

        async fn answers(&self) -> Value {
            let out = self.dir.path().join("out.json");
            for _ in 0..400 {
                if let Ok(text) = std::fs::read_to_string(&out) {
                    return serde_json::from_str(&text).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            panic!("the client never answered");
        }
    }

    fn code(answers: &Value, call: &str) -> String {
        answers[call]["error"]["code"].as_str().unwrap_or("ok").to_string()
    }

    /// The other answers do not depend on who asked.
    fn unchanged(answers: &Value) {
        assert_eq!(answers["user bus.ping"]["ok"], true, "a ping claims nothing: {answers}");
        assert_eq!(code(answers, "agent:calm-otter project.list"), "bus.actor", "an agent still needs its token: {answers}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_process_inside_a_session_cannot_claim_the_user() {
        if !python() {
            return;
        }
        let f = Fixture::new().await;
        // `sh` stands in for the session's PTY child; the client is its child, as an agent's
        // shell command would be. It waits until the pid is registered as the session's.
        let go = f.dir.path().join("go");
        let script = format!(
            "while [ ! -e '{}' ]; do sleep 0.02; done; python3 \"$@\"; sleep 30",
            go.display()
        );
        let mut args = vec!["-c".to_string(), script, "sh".to_string()];
        args.extend(f.args("attached"));
        let spec = crate::pty::SpawnSpec {
            cmd: "sh".into(),
            args,
            env: Vec::new(),
            env_remove: Vec::new(),
            cwd: f.dir.path().to_path_buf(),
            cols: 80,
            rows: 24,
            epoch: 1,
            initial_scrollback: Vec::new(),
        };
        let pty = crate::pty::Pty::spawn(spec, |_| {}).unwrap();
        f.engine.set_pty(1, "calm-otter", pty.clone());
        assert_eq!(f.engine.session_pids().get(&pty.pid()).map(String::as_str), Some("calm-otter"));
        std::fs::write(&go, "").unwrap();
        let answers = f.answers().await;
        pty.kill(Duration::from_millis(200));
        for call in ["user project.list", "test project.list", "user bus.subscribe"] {
            assert_eq!(code(&answers, call), "actor.peer", "{call}: {answers}");
            assert_eq!(answers[call]["error"]["kind"], "refused", "{answers}");
            assert_eq!(answers[call]["error"]["details"]["session"], "calm-otter", "{answers}");
        }
        let message = answers["user project.list"]["error"]["message"].as_str().unwrap();
        assert!(message.contains("the Relay app, the phone, or a terminal outside Relay's sessions"), "{message}");
        unchanged(&answers);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_process_the_engine_started_outside_every_session_cannot_claim_the_user() {
        if !python() {
            return;
        }
        let f = Fixture::new().await;
        // This test process is the engine; its child is a build, a hook, or a session's orphan.
        let status = Command::new("python3").args(f.args("attached")).status().unwrap();
        assert!(status.success());
        let answers = f.answers().await;
        for call in ["user project.list", "test project.list", "user bus.subscribe"] {
            assert_eq!(code(&answers, call), "actor.peer", "{call}: {answers}");
            assert_eq!(answers[call]["error"]["details"]["peer"], "engine_child", "{answers}");
        }
        unchanged(&answers);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_user_outside_the_engine_and_the_engine_itself_still_are_the_user() {
        if !python() {
            return;
        }
        let f = Fixture::new().await;
        // A process that left this tree before connecting: a terminal of the user's.
        let status = Command::new("python3").args(f.args("detach")).status().unwrap();
        assert!(status.success());
        let answers = f.answers().await;
        assert_eq!(code(&answers, "user project.list"), "ok", "{answers}");
        assert_eq!(code(&answers, "test project.list"), "ok", "{answers}");
        assert_eq!(code(&answers, "user bus.subscribe"), "ok", "{answers}");
        unchanged(&answers);
        // In-process (the phone bridge of `relay serve --remote` connects this way).
        let mut client = Client::connect(&f.server.path).await.unwrap();
        let listed = client.call(&Request::new(Actor::User, "project.list", json!({})), |_| {}).await.unwrap();
        assert!(listed.ok, "{:?}", listed.error);
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use base64::Engine as _;

    /// The hand-written encoder must be byte-for-byte what `serde_json` would have produced
    /// for the same `Frame`, or a client that trusts the schema is reading a different protocol.
    #[test]
    fn hand_written_pty_lines_match_serde_exactly() {
        let bodies: Vec<Vec<u8>> = vec![
            Vec::new(),
            b"hello".to_vec(),
            b"\x1b[32mgreen\x1b[0m \"quoted\" \\ backslash\n\t".to_vec(),
            (0u8..=255).collect(),
            vec![0xff; 64 * 1024],
        ];
        // Names that would break a formatter that forgot to escape.
        for session in ["brisk-otter", "quote\"name", "back\\slash", "new\nline", "unicode-é"] {
            let head = pty_frame_head(session);
            for (i, body) in bodies.iter().enumerate() {
                let epoch = i as u64 + 3;
                let seq = (i as u64 + 1) * 1_000_000;
                let expected = serde_json::to_string(&Frame {
                    v: ENVELOPE_V,
                    stream: "pty".into(),
                    session: Some(session.to_string()),
                    run_id: None,
                    mirror_id: None,
                    epoch: Some(epoch),
                    seq,
                    data: serde_json::Value::String(
                        base64::engine::general_purpose::STANDARD.encode(body),
                    ),
                })
                .unwrap();
                let actual = pty_frame_line(&head, epoch, seq, body);
                assert_eq!(actual, expected, "session {session:?}, body #{i}");
                // And it still parses back into the same frame.
                let parsed: Frame = serde_json::from_str(&actual).unwrap();
                assert_eq!(parsed.seq, seq);
                assert_eq!(parsed.epoch, Some(epoch));
                assert_eq!(parsed.session.as_deref(), Some(session));
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(parsed.data.as_str().unwrap())
                    .unwrap();
                assert_eq!(&decoded, body);
            }
        }
    }

    /// An unlocked query that panics on the blocking pool is answered with an internal error
    /// under its own id, not dropped.
    #[tokio::test]
    async fn a_panicked_unlocked_query_is_still_answered() {
        let id = uuid::Uuid::new_v4();
        let joined = tokio::task::spawn_blocking(|| -> Response { panic!("boom") }).await;
        let resp = answered(id, "gym.summary", joined);
        assert!(!resp.ok);
        assert_eq!(resp.id, Some(id));
        assert_eq!(resp.error.unwrap().kind, relay_bus::ErrorKind::Internal);
    }
}
