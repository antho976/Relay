//! One paired conversation: greet, verify, then carry bus lines between a phone and the
//! engine's socket door. The transport underneath is either a WebSocket the phone opened
//! directly (`direct.rs`) or one lane of the rendezvous tunnel (`tunnel.rs`); both hand this
//! module a pair of channels and nothing else.

use crate::registry::{Outcome, Presented, Registry};
use crate::wire::{self, Gate, Greeting, Hello, Welcome, WIRE_V};
use anyhow::{Context, Result};
use relay_core::Instance;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// What every bridge needs to know about the engine it fronts.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub instance: Instance,
    pub registry_path: PathBuf,
    pub socket_path: PathBuf,
    pub version: String,
    /// The `relay` binary this door starts the engine with when an admitted phone finds none
    /// answering (`relay remote serve --start-engine`). `None` for a door that lives inside its
    /// engine (`relay serve --remote`), or one that only fronts an engine someone else runs.
    pub start_engine: Option<PathBuf>,
    /// Which ops a paired device may call through this door (`wire::OpProfile`).
    pub ops: wire::OpProfile,
}

impl Ctx {
    pub fn for_instance(instance: Instance) -> Ctx {
        Ctx {
            instance,
            registry_path: Registry::path_for(instance),
            socket_path: instance.socket_path(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            start_engine: None,
            ops: wire::OpProfile::Phone,
        }
    }
}

/// How long a phone has to answer the greeting. The transport bounds the time before the
/// greeting (`direct.rs`'s handshake deadline); this bounds the time after it.
const HELLO_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a phone presenting a code that needs the PC's approval waits for it. The phone
/// waits 90 s for the welcome to a pairing (`Handshake.PAIR_WELCOME_TIMEOUT_MS` in apps/relay-android), so this
/// stays under that and the phone hears `pair.unconfirmed` rather than its own timeout; a person
/// who is slower runs `relay remote pair` again. Both doors ping during the wait, so neither
/// drops the quiet phone first. A phone that leaves sooner (an older app gives up after 20 s)
/// withdraws its request: see `Withdraw`.
const PAIR_CONFIRM_WAIT: Duration = Duration::from_secs(75);
const PAIR_POLL: Duration = Duration::from_millis(200);

/// How often an admitted connection checks that its device is still paired. A revoke from
/// another process only edits `remote.json`; this is what cuts a connected phone off. The check
/// is a `stat` until the file changes.
const RECHECK_EVERY: Duration = Duration::from_secs(1);

/// Lines queued toward the phone before the transport applies backpressure. Terminal output
/// arrives in bursts; a phone on a poor link must not park unbounded memory here.
pub const OUTBOUND_QUEUE: usize = 256;

/// Aborts a task when dropped, so a conversation that is itself aborted takes its helpers with it.
pub struct AbortOnDrop(pub AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Why a connection ended before or during the bus phase. Logged, never fatal.
#[derive(Debug, thiserror::Error)]
pub enum BridgeEnd {
    #[error("the phone did not answer the greeting in time")]
    HelloTimeout,
    #[error("the phone closed before saying hello")]
    ClosedEarly,
    #[error("hello was not understood: {0}")]
    BadHello(String),
    #[error("denied: {0}")]
    Denied(&'static str),
    #[error("the device was revoked while connected")]
    Revoked,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// The outcome of a handshake, with the registry already written back.
pub struct Admitted {
    pub device_id: String,
    pub welcome: Welcome,
    /// The credential this connection was admitted with; it stays admitted while the registry
    /// still holds exactly this device and token.
    token: String,
}

/// Verify a `Hello` against the registry on disk and record the result. Pure enough to test
/// without a transport: the transport-side `run` calls exactly this. `origin` is how the
/// connection arrived, shown to the person asked to approve a pairing.
pub async fn admit(ctx: &Ctx, challenge: &str, hello: &Hello, origin: &str) -> std::result::Result<Admitted, BridgeEnd> {
    if hello.v != WIRE_V {
        return Err(BridgeEnd::BadHello(format!("wire v{} is not v{WIRE_V}", hello.v)));
    }
    let path = &ctx.registry_path;
    if let Some(code) = hello.pair.as_deref() {
        let name = hello.device_name.as_deref().unwrap_or("");
        let presented = Registry::update(path, |r| Ok::<_, BridgeEnd>(r.present(code, name, origin)))?;
        let device = match presented {
            Presented::Invalid => return Err(BridgeEnd::Denied("pair.invalid")),
            Presented::Paired(device) => device,
            Presented::Waiting => {
                tracing::warn!(device_name = %name, origin = %origin, instance = %ctx.instance,
                    "a phone presented the pairing code; waiting for `relay remote pair` to approve it");
                let mut withdraw = Withdraw { path, code: Some(code) };
                let deadline = tokio::time::Instant::now() + PAIR_CONFIRM_WAIT;
                loop {
                    match Registry::update(path, |r| Ok::<_, BridgeEnd>(r.outcome(code)))? {
                        Outcome::Approved(device) => {
                            withdraw.code = None;
                            break device;
                        }
                        Outcome::Declined => {
                            withdraw.code = None;
                            return Err(BridgeEnd::Denied("pair.declined"));
                        }
                        // Unanswered is not yes: `withdraw` spends the code, and the person pairs again.
                        Outcome::Waiting if tokio::time::Instant::now() >= deadline => {
                            return Err(BridgeEnd::Denied("pair.unconfirmed"));
                        }
                        Outcome::Waiting => tokio::time::sleep(PAIR_POLL).await,
                    }
                }
            }
        };
        // Every pairing is announced where the PC's owner looks: the engine log, at warn, and
        // the desktop app, if one is open.
        tracing::warn!(device = %device.id, device_name = %device.name, origin = %origin, instance = %ctx.instance,
            "a new phone paired with this PC; `relay remote devices` lists it, `relay remote revoke {}` removes it", device.id);
        tokio::spawn(announce_pairing(ctx.socket_path.clone(), device.name.clone(), device.id.clone(), origin.to_string()));
        return Ok(Admitted {
            device_id: device.id.clone(),
            welcome: Welcome::paired(&device.id, &device.token),
            token: device.token,
        });
    }
    let (Some(id), Some(proof)) = (hello.device.as_deref(), hello.proof.as_deref()) else {
        return Err(BridgeEnd::BadHello("expected `pair` or `device`+`proof`".into()));
    };
    Registry::update(path, |registry| {
        let device = registry.device(id).ok_or(BridgeEnd::Denied("auth.unknown_device"))?;
        if !wire::digest_eq(proof, &wire::proof(challenge, &device.token)) {
            return Err(BridgeEnd::Denied("auth.bad_proof"));
        }
        let token = device.token.clone();
        registry.touch(id);
        Ok(Admitted { device_id: id.to_string(), welcome: Welcome::admitted(id), token })
    })
}

/// How long announcing a pairing to the desktop may take before it is given up.
const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(5);

/// Tell whoever is at the desktop that a phone just paired: `ui.toast`, which the native client
/// shows in every window it has open. Best effort — with no desktop open, or no engine, the warn
/// line in the log is the record. A lasting entry in the notification centre needs an op that
/// can write one (none exists for a client today; see docs/MOBILE.md §6).
async fn announce_pairing(socket_path: PathBuf, name: String, id: String, origin: String) {
    let text = format!("A phone paired with this PC: \"{name}\" ({origin}). Not yours? Run `relay remote revoke {id}`.");
    toast(socket_path, text, "warn", 20_000).await;
}

/// A `ui.toast` on every window the native client has open. Best effort, as above.
pub(crate) async fn toast(socket_path: PathBuf, text: String, level: &str, ttl_ms: u64) {
    use relay_bus::envelope::{Actor, Request};
    let request = Request::new(Actor::User, "ui.toast", serde_json::json!({"text": text, "level": level, "ttl_ms": ttl_ms}));
    let attempt = async {
        let stream = UnixStream::connect(&socket_path).await?;
        relay_core::socket::same_user(&stream)?;
        let (reader, mut writer) = stream.into_split();
        writer.write_all(serde_json::to_string(&request)?.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        // Wait for the answer, so closing the socket cannot cut the request off.
        let id = request.id.to_string();
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await? {
            if line.contains(&id) {
                break;
            }
        }
        anyhow::Ok(())
    };
    match tokio::time::timeout(ANNOUNCE_TIMEOUT, attempt).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::debug!(error = %format!("{e:#}"), "toast not shown on the desktop"),
        Err(_) => tracing::debug!("toast not shown on the desktop: the engine did not answer"),
    }
}

/// Closes the window of a pairing that stopped waiting for its answer — timed out, or dropped
/// because the phone left — so a late "yes" at the PC mints no credential nobody holds.
struct Withdraw<'a> {
    path: &'a std::path::Path,
    code: Option<&'a str>,
}

impl Drop for Withdraw<'_> {
    fn drop(&mut self) {
        if let Some(code) = self.code {
            let _ = Registry::update(self.path, |r| {
                r.abandon(code);
                Ok::<_, anyhow::Error>(())
            });
        }
    }
}

/// Resolves once the phone has gone: its inbound channel closed. Before the welcome a phone
/// has nothing to say, so whatever it sends while it waits is dropped.
async fn gone(inbound: &mut mpsc::Receiver<String>) {
    while inbound.recv().await.is_some() {}
}

/// What `still_admitted` compares to notice that `remote.json` changed without reading it.
fn stamp(path: &std::path::Path) -> Option<(std::time::SystemTime, u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.ino(), meta.len()))
}

/// Whether the registry still holds this device with this token. A registry that cannot be
/// read admits no one: it is the only record of who may be here.
fn still_admitted(ctx: &Ctx, device_id: &str, token: &str) -> bool {
    Registry::load(&ctx.registry_path).is_ok_and(|r| r.device(device_id).is_some_and(|d| d.token == token))
}

/// What the door says before anything is proved: through a rendezvous, anyone who knows the room
/// id reads it. Who this PC is goes in the welcome (`welcome_for`).
pub fn greeting(ctx: &Ctx, challenge: &str, engine_up: bool) -> Greeting {
    Greeting {
        v: WIRE_V,
        relay: "remote".into(),
        host: String::new(),
        host_id: String::new(),
        instance: ctx.instance.as_str().into(),
        version: String::new(),
        challenge: challenge.into(),
        // Only a door that can start the engine says it is stopped: the phone then waits for it.
        engine: Some(if engine_up || ctx.start_engine.is_none() { "running" } else { "stopped" }.into()),
    }
}

/// An admitted phone's welcome, with who it reached: the PC's name and id, the Relay version, the
/// network cards it can wake from the same network, and which ops this door opens (a PC's or a
/// cloud copy's), so the phone never takes one for the other.
pub fn welcome_for(ctx: &Ctx, registry: &Registry, welcome: Welcome) -> Welcome {
    let welcome = welcome.with_host(&registry.host_name, &registry.host_id, &ctx.version, crate::wake::targets());
    Welcome { ops: Some(ctx.ops.as_str().into()), ..welcome }
}

/// How long a door that started the engine waits for its socket to answer.
const ENGINE_START_WAIT: Duration = Duration::from_secs(30);

/// Connect to the engine's socket door, starting the engine first when this door may and none
/// answers. Several phones arriving at once start it once: a second `relay serve` finds the
/// instance lock held and exits, and every waiter connects to whichever one won.
async fn engine(ctx: &Ctx) -> Result<UnixStream> {
    if let Ok(stream) = UnixStream::connect(&ctx.socket_path).await {
        return Ok(stream);
    }
    let Some(relay) = &ctx.start_engine else {
        anyhow::bail!("no engine at {}", ctx.socket_path.display());
    };
    static STARTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = STARTING.lock().await;
    if !relay_core::socket::probe(&ctx.socket_path).await {
        start_engine(relay, ctx)?;
        let deadline = tokio::time::Instant::now() + ENGINE_START_WAIT;
        while !relay_core::socket::probe(&ctx.socket_path).await {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("the engine did not answer within {}s of starting; see {}", ENGINE_START_WAIT.as_secs(), engine_log(ctx).display());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    UnixStream::connect(&ctx.socket_path)
        .await
        .with_context(|| format!("no engine at {}", ctx.socket_path.display()))
}

/// Beside `remote.json`, in the instance's data directory.
fn engine_log(ctx: &Ctx) -> PathBuf {
    ctx.registry_path.with_file_name("engine.log")
}

/// `relay --instance <i> serve` in its own process group, its output in the instance's
/// `engine.log`. It outlives this door, as an engine started by `./run.sh` outlives its
/// terminal: closing the door must not end the agents the engine holds.
fn start_engine(relay: &std::path::Path, ctx: &Ctx) -> Result<()> {
    use std::os::unix::process::CommandExt as _;
    let instance = ctx.instance;
    let log_path = engine_log(ctx);
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path).with_context(|| format!("opening {}", log_path.display()))?;
    let mut child = std::process::Command::new(relay)
        .args(["--instance", instance.as_str(), "serve"])
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .with_context(|| format!("starting {} serve", relay.display()))?;
    tracing::warn!(pid = child.id(), instance = %instance, "a paired phone found no engine; started one");
    // Reaped when it exits, so an engine that stops before this door leaves no zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Run one conversation to its end. `inbound` carries text frames from the phone; `outbound`
/// carries lines to it. Returns when either side goes away, or when the device is revoked.
/// `unproven` is whatever the transport holds for a connection that has not yet proved itself;
/// it is dropped once the device is admitted. `origin` says where the connection came from.
pub async fn run(
    ctx: Arc<Ctx>,
    mut inbound: mpsc::Receiver<String>,
    outbound: mpsc::Sender<String>,
    unproven: impl Send,
    origin: String,
) -> std::result::Result<(), BridgeEnd> {
    let registry = Registry::load(&ctx.registry_path).map_err(BridgeEnd::Other)?;
    let challenge = crate::registry::random_hex(16);
    let engine_up = ctx.start_engine.is_none() || relay_core::socket::probe(&ctx.socket_path).await;
    let greet = greeting(&ctx, &challenge, engine_up);
    outbound
        .send(serde_json::to_string(&greet).context("greeting")?)
        .await
        .map_err(|_| BridgeEnd::ClosedEarly)?;

    let first = match tokio::time::timeout(HELLO_TIMEOUT, inbound.recv()).await {
        Err(_) => return Err(BridgeEnd::HelloTimeout),
        Ok(None) => return Err(BridgeEnd::ClosedEarly),
        Ok(Some(line)) => line,
    };
    let hello: Hello = match serde_json::from_str(&first) {
        Ok(h) => h,
        Err(e) => {
            let _ = outbound.send(serde_json::to_string(&Welcome::denied("hello.parse")).unwrap_or_default()).await;
            return Err(BridgeEnd::BadHello(e.to_string()));
        }
    };
    // Taken before the registry is read to admit: a revoke at any point after that — while the
    // engine starts, which can take 30 s — changes the file from this, and the first recheck
    // (an interval ticks at once) sees it. Admitting writes the file too; that costs one load.
    let mut seen = stamp(&ctx.registry_path);
    // A pairing can wait on a person at the PC; a phone that leaves meanwhile ends it.
    let admission = tokio::select! {
        admission = admit(&ctx, &challenge, &hello, &origin) => admission,
        _ = gone(&mut inbound) => return Err(BridgeEnd::ClosedEarly),
    };
    let admitted = match admission {
        Ok(a) => a,
        Err(end) => {
            let code = match &end {
                BridgeEnd::Denied(code) => code,
                BridgeEnd::BadHello(_) => "hello.shape",
                _ => "internal",
            };
            let _ = outbound.send(serde_json::to_string(&Welcome::denied(code)).unwrap_or_default()).await;
            return Err(end);
        }
    };
    let welcome = welcome_for(&ctx, &registry, admitted.welcome.clone());
    outbound
        .send(serde_json::to_string(&welcome).context("welcome")?)
        .await
        .map_err(|_| BridgeEnd::ClosedEarly)?;
    tracing::info!(device = %admitted.device_id, instance = %ctx.instance, "remote device admitted");
    drop(unproven);

    // Only now does the engine hear about this connection; a door that may start it does so here.
    let stream = engine(&ctx).await?;
    relay_core::socket::same_user(&stream)?;
    let (reader, mut writer) = stream.into_split();

    let to_phone = outbound.clone();
    let mut pump = tokio::spawn(async move {
        let mut lines = BufReader::with_capacity(256 * 1024, reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if to_phone.send(line).await.is_err() {
                break;
            }
        }
    });
    // A transport that aborts this conversation must not leave the engine connection open.
    let pump_guard = AbortOnDrop(pump.abort_handle());

    let (instance, profile) = (ctx.instance, ctx.ops);
    let mut recheck = tokio::time::interval(RECHECK_EVERY);
    recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut revoked = false;
    let result: Result<()> = async {
        loop {
            let line = tokio::select! {
                line = inbound.recv() => match line {
                    Some(line) => line,
                    None => break,
                },
                // The engine went away (restarted, crashed): end the conversation, so the
                // transport closes and the phone reconnects instead of talking to nothing.
                _ = &mut pump => {
                    tracing::info!(device = %admitted.device_id, "the engine closed its side; closing the phone's");
                    break;
                }
                _ = recheck.tick() => {
                    let now = stamp(&ctx.registry_path);
                    if now != seen {
                        seen = now;
                        if !still_admitted(&ctx, &admitted.device_id, &admitted.token) {
                            revoked = true;
                            break;
                        }
                    }
                    continue;
                }
            };
            // The engine reads its door line by line, so a frame holding several lines is
            // several requests, and each one is gated on its own.
            for line in line.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match wire::gate_for(line, instance, profile) {
                    Gate::Forward => {
                        writer.write_all(line.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Gate::Reject(resp) => {
                        if outbound.send(serde_json::to_string(&resp)?).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    .await;
    drop(pump_guard);
    if revoked {
        tracing::warn!(device = %admitted.device_id, "remote device revoked; its connection is closed");
        return Err(BridgeEnd::Revoked);
    }
    tracing::info!(device = %admitted.device_id, "remote device left");
    result.map_err(BridgeEnd::Other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(dir: &tempfile::TempDir) -> Ctx {
        Ctx {
            instance: Instance::Test,
            registry_path: dir.path().join("remote.json"),
            socket_path: dir.path().join("none.sock"),
            version: "test".into(),
            start_engine: None,
            ops: wire::OpProfile::Phone,
        }
    }

    #[tokio::test]
    async fn pairing_mints_a_token_that_then_proves_itself() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(&dir);
        let mut reg = Registry::fresh();
        let code = reg.begin_pair(false).code;
        reg.save(&ctx.registry_path).unwrap();

        let hello = Hello { v: 1, pair: Some(code.clone()), device_name: Some("Pixel".into()), ..Default::default() };
        let paired = admit(&ctx, "c0", &hello, "test").await.unwrap();
        let token = paired.welcome.token.clone().unwrap();

        // The code is spent.
        assert!(matches!(admit(&ctx, "c0", &hello, "test").await, Err(BridgeEnd::Denied("pair.invalid"))));

        // The token proves itself against a fresh challenge, and only that challenge.
        let ok = Hello { v: 1, device: Some(paired.device_id.clone()), proof: Some(wire::proof("c1", &token)), ..Default::default() };
        let admitted = admit(&ctx, "c1", &ok, "test").await.unwrap();
        assert!(admitted.welcome.token.is_none(), "the token is never sent twice");
        assert!(matches!(admit(&ctx, "c2", &ok, "test").await, Err(BridgeEnd::Denied("auth.bad_proof"))));

        let stranger = Hello { v: 1, device: Some("nobody".into()), proof: Some("00".into()), ..Default::default() };
        assert!(matches!(admit(&ctx, "c1", &stranger, "test").await, Err(BridgeEnd::Denied("auth.unknown_device"))));

        let reg = Registry::load(&ctx.registry_path).unwrap();
        assert!(reg.device(&paired.device_id).unwrap().last_seen.is_some());
        assert!(reg.pending.is_empty());
    }

    #[tokio::test]
    async fn a_hello_with_neither_shape_or_the_wrong_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(&dir);
        assert!(matches!(admit(&ctx, "c", &Hello { v: 1, ..Default::default() }, "test").await, Err(BridgeEnd::BadHello(_))));
        assert!(matches!(admit(&ctx, "c", &Hello { v: 2, pair: Some("x".into()), ..Default::default() }, "test").await, Err(BridgeEnd::BadHello(_))));
    }

    #[tokio::test]
    async fn a_confirmed_code_pairs_only_when_the_pc_says_yes() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Arc::new(ctx(&dir));
        let code = Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.begin_pair(true).code)).unwrap();
        let hello = Hello { v: 1, pair: Some(code.clone()), device_name: Some("Pixel".into()), ..Default::default() };

        // The PC approves while the phone waits.
        let waiting = {
            let (ctx, hello) = (ctx.clone(), hello.clone());
            tokio::spawn(async move { admit(&ctx, "c", &hello, "direct 192.0.2.7").await })
        };
        let approved = loop {
            let decided = Registry::update(&ctx.registry_path, |r| {
                let asked = r.pending.first().and_then(|p| p.request.clone());
                Ok::<_, anyhow::Error>(asked.filter(|q| q.origin == "direct 192.0.2.7").map(|_| r.decide(&code, true)))
            })
            .unwrap();
            if let Some(decided) = decided {
                break decided;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert!(approved);
        let paired = waiting.await.unwrap().unwrap();
        assert!(paired.welcome.token.is_some());

        // A declined phone gets nothing, and the code is spent.
        let code = Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.begin_pair(true).code)).unwrap();
        let hello = Hello { v: 1, pair: Some(code.clone()), device_name: Some("Stranger".into()), ..Default::default() };
        let waiting = {
            let ctx = ctx.clone();
            tokio::spawn(async move { admit(&ctx, "c", &hello, "rendezvous").await })
        };
        while !Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.decide(&code, false))).unwrap() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(matches!(waiting.await.unwrap(), Err(BridgeEnd::Denied("pair.declined"))));
        let reg = Registry::load(&ctx.registry_path).unwrap();
        assert_eq!(reg.devices.len(), 1);
        assert!(reg.pending.is_empty());

        // A phone that leaves while it waits withdraws its request: a late "yes" mints nothing.
        let code = Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.begin_pair(true).code)).unwrap();
        let hello = Hello { v: 1, pair: Some(code.clone()), device_name: Some("Impatient".into()), ..Default::default() };
        let waiting = {
            let ctx = ctx.clone();
            tokio::spawn(async move { admit(&ctx, "c", &hello, "direct 192.0.2.8").await })
        };
        while Registry::load(&ctx.registry_path).unwrap().pending.first().and_then(|p| p.request.as_ref()).is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        waiting.abort();
        assert!(waiting.await.is_err_and(|e| e.is_cancelled()));
        assert!(!Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.decide(&code, true))).unwrap());
        let reg = Registry::load(&ctx.registry_path).unwrap();
        assert!(reg.pending.is_empty());
        assert_eq!(reg.devices.len(), 1);
    }

    #[tokio::test]
    async fn a_revoke_while_the_engine_starts_still_closes_the_connection() {
        let dir = tempfile::tempdir().unwrap();
        // "Starting" the engine does nothing; the test brings its socket up itself, late.
        let ctx = Arc::new(Ctx { start_engine: Some("/bin/true".into()), ..ctx(&dir) });
        let device = Registry::update(&ctx.registry_path, |r| {
            let code = r.begin_pair(false).code;
            match r.present(&code, "Pixel", "test") {
                Presented::Paired(device) => Ok::<_, anyhow::Error>(device),
                other => panic!("a first-come code pairs: {other:?}"),
            }
        })
        .unwrap();
        let (to_bridge, inbound) = mpsc::channel(8);
        let (outbound, mut from_bridge) = mpsc::channel(8);
        let bridge = tokio::spawn(run(ctx.clone(), inbound, outbound, (), "test".into()));
        let greeting: Greeting = serde_json::from_str(&from_bridge.recv().await.unwrap()).unwrap();
        let hello = Hello { v: 1, device: Some(device.id.clone()), proof: Some(wire::proof(&greeting.challenge, &device.token)), ..Default::default() };
        to_bridge.send(serde_json::to_string(&hello).unwrap()).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&from_bridge.recv().await.unwrap()).unwrap();
        assert!(welcome.ok);

        // Admitted, and waiting for an engine: the phone is revoked before one answers.
        assert!(Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.revoke(&device.id))).unwrap());
        let listener = tokio::net::UnixListener::bind(&ctx.socket_path).unwrap();
        let _engine = AbortOnDrop(
            tokio::spawn(async move {
                // Answers every request ok, which is all `probe` asks.
                while let Ok((stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let request: relay_bus::envelope::Request = serde_json::from_str(&line).unwrap();
                            let response = relay_bus::envelope::Response::ok(request.id, serde_json::json!({}));
                            let _ = writer.write_all(format!("{}\n", serde_json::to_string(&response).unwrap()).as_bytes()).await;
                        }
                    });
                }
            })
            .abort_handle(),
        );
        let end = tokio::time::timeout(Duration::from_secs(10), bridge).await.expect("a revoked phone is cut off").unwrap();
        assert!(matches!(end, Err(BridgeEnd::Revoked)), "{end:?}");
    }
}
