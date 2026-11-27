//! The WiFi door: a WebSocket listener on the LAN. A phone that scanned the pairing code
//! connects here first, because a link that never leaves the room is the shortest one.
//!
//! `GET /info` answers plain HTTP with the host's name so a phone can tell "wrong network"
//! from "wrong machine" before it starts a handshake.
//!
//! The door runs inside the engine (`relay serve --remote`), so a stranger on the network must
//! not be able to spend the engine's resources: until this machine has a paired phone or an
//! open pairing window it answers nothing, every connection gets a deadline to say what it is,
//! and the number of connections — and of those not yet proven — is capped.

use crate::bridge::{self, Ctx, OUTBOUND_QUEUE};
use crate::registry::Registry;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// How long a connection has to say what it is: the `/info` probe or a finished WebSocket
/// upgrade. After the upgrade the bridge's own hello deadline takes over.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Connections the door holds at once. A phone keeps one or two; the cap is far below the
/// engine's descriptor limit so the desktop, the CLI and agent hooks always find room.
const MAX_CONNECTIONS: usize = 64;

/// Of those, how many may still be unproven, in all and from one address. A stranger can only
/// occupy unproven slots, and only a few of them, so a paired phone is not locked out by
/// connections that will never say hello.
const MAX_UNPROVEN: usize = 16;
const MAX_UNPROVEN_PER_PEER: usize = 4;

/// The largest message a phone may send. Bus requests are small; a file write is the biggest.
const MAX_MESSAGE: usize = 8 << 20;

/// A live phone sends `bus.ping` every 25 s, and answers the door's own pings even when its
/// app is asleep. A connection silent for this long is a phone that dropped off without a FIN;
/// closing it frees its slot and its engine connection instead of waiting out TCP retries. A
/// phone that told its keep-alive ([`Pace`]) gets longer.
const PHONE_SILENCE: Duration = Duration::from_secs(90);

/// How often the door pings the phone, so a dead link is noticed from this side too.
pub(crate) const PING_EVERY: Duration = Duration::from_secs(30);

/// The longest silence a told door waits out (a phone that pings every few minutes with its
/// screen off): past it, the phone is gone.
const TOLD_SILENCE_MAX: Duration = Duration::from_secs(600);

/// The op a phone tells its keep-alive with (`{"ms": n}`), answered by the door itself and never
/// sent on to the engine: Relay Code sends it after subscribing and whenever its pace changes.
pub const KEEPALIVE_OP: &str = "door.keepalive";

/// How the door paces one phone's connection. A phone that says nothing (the Kotlin app, older
/// builds) gets the door's ping every 30 s and is dropped after 90 s of silence, as always. A phone
/// that told its keep-alive is not pinged while it is heard within that interval, which spares
/// its radio, and is dropped after three intervals of silence (90 s at least, 10 min at most).
#[derive(Debug, Default)]
pub struct Pace {
    /// The phone's keep-alive, in ms; 0 until it says.
    told_ms: std::sync::atomic::AtomicU64,
    /// When the phone was last heard, in ms on [`Pace::clock`].
    heard_ms: std::sync::atomic::AtomicU64,
    start: std::sync::OnceLock<std::time::Instant>,
}

impl Pace {
    /// Ms since this connection began: the pace's own clock.
    pub fn clock(&self) -> u64 {
        self.start.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
    }

    pub fn told(&self, ms: u64) {
        self.told_ms.store(ms, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn heard(&self, now_ms: u64) {
        self.heard_ms.store(now_ms, std::sync::atomic::Ordering::SeqCst);
    }

    /// How long the phone may be silent before the door lets it go.
    pub fn silence(&self) -> Duration {
        match self.told_ms.load(std::sync::atomic::Ordering::SeqCst) {
            0 => PHONE_SILENCE,
            ms => (Duration::from_millis(ms) * 3).clamp(PHONE_SILENCE, TOLD_SILENCE_MAX),
        }
    }

    /// Whether the door's own ping goes out at this tick.
    pub fn ping_due(&self, now_ms: u64) -> bool {
        match self.told_ms.load(std::sync::atomic::Ordering::SeqCst) {
            0 => true,
            ms => now_ms.saturating_sub(self.heard_ms.load(std::sync::atomic::Ordering::SeqCst)) >= ms,
        }
    }

    /// The phone's line, if it is the door's own keep-alive op: the answer to send it back.
    pub fn take_told(&self, line: &str) -> Option<String> {
        if !line.contains(KEEPALIVE_OP) {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        if value.get("op").and_then(|o| o.as_str()) != Some(KEEPALIVE_OP) {
            return None;
        }
        let ms = value.pointer("/payload/ms").and_then(|m| m.as_u64()).unwrap_or(0);
        self.told(ms.min(TOLD_SILENCE_MAX.as_millis() as u64));
        let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
        Some(serde_json::json!({"v": 1, "id": id, "ok": true, "result": {}}).to_string())
    }
}

pub struct DirectServer {
    pub local_addr: SocketAddr,
    accept: JoinHandle<()>,
}

impl Drop for DirectServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

impl DirectServer {
    pub async fn bind(ctx: Arc<Ctx>, addr: SocketAddr) -> Result<DirectServer> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding {addr}"))?;
        let local_addr = listener.local_addr()?;
        tracing::info!(addr = %local_addr, "remote door open (direct)");
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let unproven = Arc::new(Unproven::default());
        let accept = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        // Refuse at once rather than queue: a socket waiting for a slot still
                        // holds a descriptor.
                        let (Ok(slot), Some(unproven)) =
                            (slots.clone().try_acquire_owned(), unproven.enter(peer.ip()))
                        else {
                            tracing::debug!(peer = %peer, "direct door full; connection refused");
                            continue;
                        };
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle(ctx, stream, unproven, peer).await {
                                tracing::debug!(peer = %peer, error = %e, "direct connection ended");
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
        Ok(DirectServer { local_addr, accept })
    }
}

/// The unproven connections, counted in all and per address.
struct Unproven {
    total: Arc<Semaphore>,
    per_peer: Mutex<HashMap<IpAddr, usize>>,
}

impl Default for Unproven {
    fn default() -> Unproven {
        Unproven { total: Arc::new(Semaphore::new(MAX_UNPROVEN)), per_peer: Mutex::default() }
    }
}

impl Unproven {
    fn enter(self: &Arc<Self>, ip: IpAddr) -> Option<UnprovenSlot> {
        let permit = self.total.clone().try_acquire_owned().ok()?;
        let mut per_peer = self.per_peer.lock().unwrap();
        let held = per_peer.entry(ip).or_default();
        if *held >= MAX_UNPROVEN_PER_PEER {
            return None;
        }
        *held += 1;
        Some(UnprovenSlot { unproven: self.clone(), ip, _permit: permit })
    }
}

/// One unproven connection's place; the bridge drops it once the device is admitted.
struct UnprovenSlot {
    unproven: Arc<Unproven>,
    ip: IpAddr,
    _permit: OwnedSemaphorePermit,
}

impl Drop for UnprovenSlot {
    fn drop(&mut self) {
        let mut per_peer = self.unproven.per_peer.lock().unwrap();
        if let Some(held) = per_peer.get_mut(&self.ip) {
            *held -= 1;
            if *held == 0 {
                per_peer.remove(&self.ip);
            }
        }
    }
}

/// The body of `GET /info`.
pub fn info_json(ctx: &Ctx) -> String {
    let registry = Registry::load(&ctx.registry_path).unwrap_or_else(|_| Registry::fresh());
    serde_json::json!({
        "relay": "remote",
        "host": registry.host_name,
        "host_id": registry.host_id,
        "instance": ctx.instance.as_str(),
        "version": ctx.version,
        "pairing_open": !registry.pending.is_empty(),
    })
    .to_string()
}

/// Whether the door has anyone to talk to: a paired phone, or an open pairing window. Until
/// then a connection is closed unanswered, so a machine that never paired a phone shows the
/// network nothing. `relay remote pair` only writes `remote.json`, so this reads it each time.
fn door_in_use(ctx: &Ctx) -> bool {
    Registry::load(&ctx.registry_path).is_ok_and(|r| !r.devices.is_empty() || !r.pending.is_empty())
}

/// A browser always sends `Origin`; a page on another site must not be able to open this door
/// from the person's own browser. React Native sends the URL's own origin, so the host must match.
#[allow(clippy::result_large_err)]
fn check_origin(req: &Request, resp: Response) -> std::result::Result<Response, ErrorResponse> {
    let Some(origin) = req.headers().get("origin") else {
        return Ok(resp);
    };
    let host_of = |authority: &str| -> String {
        let authority = authority.trim_end_matches('/');
        let host = match authority.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or(""),
            None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
        };
        host.to_ascii_lowercase()
    };
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|o| o.split_once("://"))
        .map(|(_, rest)| host_of(rest));
    let host = req.headers().get("host").and_then(|h| h.to_str().ok()).map(host_of);
    if origin_host.is_some() && origin_host == host {
        return Ok(resp);
    }
    let mut refusal = ErrorResponse::new(Some("cross-origin requests are not accepted".into()));
    *refusal.status_mut() = StatusCode::FORBIDDEN;
    Err(refusal)
}

async fn handle(ctx: Arc<Ctx>, stream: TcpStream, unproven: UnprovenSlot, peer: SocketAddr) -> Result<()> {
    if !door_in_use(&ctx) {
        return Ok(());
    }
    let ws = match tokio::time::timeout(HANDSHAKE_TIMEOUT, open(&ctx, stream)).await {
        Err(_) => anyhow::bail!("no handshake within {}s", HANDSHAKE_TIMEOUT.as_secs()),
        Ok(Err(e)) => return Err(e),
        Ok(Ok(None)) => return Ok(()),
        Ok(Ok(Some(ws))) => ws,
    };
    let (mut sink, mut source) = ws.split();
    let (in_tx, in_rx) = mpsc::channel::<String>(64);
    let (out_tx, mut out_rx) = mpsc::channel::<String>(OUTBOUND_QUEUE);
    let pace = Arc::new(Pace::default());
    pace.heard(pace.clock());

    let ping_pace = pace.clone();
    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
        loop {
            let msg = tokio::select! {
                line = out_rx.recv() => match line {
                    Some(line) => Message::text(line),
                    None => break,
                },
                _ = ping.tick() => {
                    if !ping_pace.ping_due(ping_pace.clock()) {
                        continue;
                    }
                    Message::Ping(Vec::new().into())
                }
            };
            if sink.send(msg).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let answers = out_tx.clone();
    let reader = tokio::spawn(async move {
        while let Ok(Some(msg)) = tokio::time::timeout(pace.silence(), source.next()).await {
            pace.heard(pace.clock());
            match msg {
                Ok(Message::Text(text)) => {
                    // The phone's keep-alive is the door's to hear: answered here, not sent on.
                    if let Some(answer) = pace.take_told(&text) {
                        if answers.send(answer).await.is_err() {
                            break;
                        }
                        continue;
                    }
                    if in_tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    if in_tx.send(String::from_utf8_lossy(&bytes).into_owned()).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    let result = bridge::run(ctx, in_rx, out_tx, unproven, format!("direct {}", peer.ip())).await;
    reader.abort();
    // Let queued lines drain, then the writer closes the socket.
    let _ = writer.await;
    result.map_err(|e| anyhow::anyhow!(e))
}

/// Answer a `/info` probe (`None`), or finish the WebSocket upgrade.
async fn open(ctx: &Ctx, mut stream: TcpStream) -> Result<Option<tokio_tungstenite::WebSocketStream<TcpStream>>> {
    // A plain HTTP probe never upgrades; answer it by hand and get out of the way.
    let mut head = [0u8; 16];
    let n = stream.peek(&mut head).await?;
    if head[..n].starts_with(b"GET /info") {
        let body = info_json(ctx);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;
        return Ok(None);
    }
    // Every bus line is its own message, flushed at once; with Nagle on, a line that follows
    // another waits for the phone's delayed ACK.
    let _ = stream.set_nodelay(true);
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let ws = tokio_tungstenite::accept_hdr_async_with_config(stream, check_origin, Some(config))
        .await
        .context("websocket handshake")?;
    Ok(Some(ws))
}

/// A Tailscale address: tailnets hand out 100.64.0.0/10. A phone on the same tailnet reaches
/// the direct door there from anywhere, with no rendezvous and nothing listening publicly.
pub fn is_tailnet(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (b & 0xc0) == 64
}

/// Every address a phone on the same network could reach this machine at.
pub fn lan_addresses() -> Vec<std::net::Ipv4Addr> {
    let mut out: Vec<std::net::Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
        .filter(|v4| !v4.is_link_local())
        .collect();
    out.sort_by_key(|a| (!a.is_private(), a.octets()));
    out.dedup();
    out
}

#[cfg(test)]
mod tailnet_tests {
    use super::is_tailnet;

    #[test]
    fn tailnet_is_the_cgnat_block_only() {
        assert!(is_tailnet("100.64.0.1".parse().unwrap()));
        assert!(is_tailnet("100.101.102.103".parse().unwrap()));
        assert!(is_tailnet("100.127.255.254".parse().unwrap()));
        assert!(!is_tailnet("100.63.0.1".parse().unwrap()));
        assert!(!is_tailnet("100.128.0.1".parse().unwrap()));
        assert!(!is_tailnet("192.168.1.20".parse().unwrap()));
    }
}

#[cfg(test)]
mod pace_tests {
    use super::*;

    /// An hour of the writer's 30 s ticks, with the phone heard every `every` ms: the pings sent.
    fn pings_in_an_hour(pace: &Pace, every: u64) -> usize {
        let tick = PING_EVERY.as_millis() as u64;
        let mut sent = 0;
        for t in (tick..=3_600_000).step_by(tick as usize) {
            if let Some(n) = t.checked_div(every) {
                pace.heard(n * every);
            }
            if pace.ping_due(t) {
                sent += 1;
            }
        }
        sent
    }

    #[test]
    fn a_phone_that_says_nothing_is_paced_as_always() {
        let pace = Pace::default();
        assert_eq!(pace.silence(), PHONE_SILENCE);
        assert_eq!(pings_in_an_hour(&pace, 25_000), 120, "the door's 30 s ping, every tick");
        assert_eq!(pace.take_told(r#"{"v":1,"id":"a","op":"bus.ping","payload":{}}"#), None, "other lines go on");
    }

    #[test]
    fn a_told_phone_is_pinged_only_when_it_goes_quiet() {
        let pace = Pace::default();
        let answer = pace.take_told(r#"{"v":1,"id":"k1","op":"door.keepalive","payload":{"ms":180000}}"#).expect("answered by the door");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&answer).unwrap(), serde_json::json!({"v": 1, "id": "k1", "ok": true, "result": {}}));
        assert_eq!(pace.silence(), Duration::from_secs(540));
        assert_eq!(pings_in_an_hour(&pace, 180_000), 0, "heard every 3 min: never pinged");
        // Never heard again: pinged on every tick once its interval has passed (a live phone
        // answers, and so is pinged at most once an interval).
        pace.heard(0);
        assert_eq!(pings_in_an_hour(&pace, 0), 115, "quiet: pinged again once 3 min passed");
        // In front: 25 s, still 90 s of silence at least; a long interval is capped.
        pace.told(25_000);
        assert_eq!(pace.silence(), PHONE_SILENCE);
        pace.told(3_600_000);
        assert_eq!(pace.silence(), TOLD_SILENCE_MAX);
    }
}
