//! The PC's end of the rendezvous: dial the server, hold the socket open, and run one bridge
//! per lane the server opens. This is the Claude-remote-control shape — the machine at home
//! reaches out, nothing on it listens to the internet — with a server the person hosts.

use crate::bridge::{self, AbortOnDrop, Ctx};
use crate::registry::Rendezvous;
use crate::rendezvous::{Lane, MAX_LANE_LINE};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// The host URL for a rendezvous config. The room secret is not in it: `dial` sends it in a
/// header, so a proxy in front of the server never writes it to its access log.
pub fn host_url(r: &Rendezvous) -> String {
    format!("{}/host/{}", r.url, r.room)
}

/// The URL a phone uses to reach this room: through the public address when one is set.
pub fn join_url(r: &Rendezvous) -> String {
    format!("{}/join/{}", r.public.as_deref().unwrap_or(&r.url), r.room)
}

/// Keep the tunnel up until the task is dropped. Reconnects with backoff; every attempt and
/// every drop is logged so a person can see why a phone was told "host offline".
pub fn spawn(ctx: Arc<Ctx>, rendezvous: Rendezvous) -> JoinHandle<()> {
    ensure_crypto_provider();
    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            // Each attempt is its own task, so a panic inside a dial is a logged, retried failure
            // rather than the silent end of the tunnel; the guard takes it down with this one.
            let connected = Arc::new(AtomicBool::new(false));
            let attempt = {
                let (ctx, rendezvous, connected) = (ctx.clone(), rendezvous.clone(), connected.clone());
                tokio::spawn(async move { connect_once(ctx, &rendezvous, &connected).await })
            };
            let _guard = AbortOnDrop(attempt.abort_handle());
            let outcome = attempt.await;
            // A connection that came up was a success however it ended (most end in an error:
            // a dropped socket, the silence limit); the next dial starts from the shortest wait.
            if connected.load(Ordering::Acquire) {
                backoff = Duration::from_secs(1);
            }
            match outcome {
                Ok(Ok(())) => {
                    tracing::info!(url = %rendezvous.url, "rendezvous closed; reconnecting");
                }
                Ok(Err(e)) => {
                    tracing::warn!(url = %rendezvous.url, error = %e, retry_s = backoff.as_secs(), "rendezvous unreachable");
                }
                Err(e) => {
                    tracing::error!(url = %rendezvous.url, error = %e, retry_s = backoff.as_secs(), "rendezvous attempt panicked");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    })
}

/// Make rustls's process-wide crypto provider explicit before the first `wss://` dial.
/// tokio-tungstenite builds its client config with `ClientConfig::builder()`, which panics when
/// it cannot pick a provider; naming one here keeps that true even if a second provider feature
/// is ever unified into the build. Losing the race to another installer is fine: one is set.
pub fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

/// How often the host pings the server, and how long silence may last before the socket is
/// declared dead. A NAT or a mobile hop that drops the connection without a FIN would
/// otherwise leave the host believing it is reachable while every phone is told "offline".
const PING_EVERY: Duration = Duration::from_secs(30);
const SILENCE_LIMIT: Duration = Duration::from_secs(90);

/// How long a dial may take, TCP, TLS and the upgrade together. A peer that accepts and never
/// answers would otherwise hold the tunnel down, unlogged, until TCP gives up.
const DIAL_TIMEOUT: Duration = Duration::from_secs(15);

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// What `relay remote rendezvous` answers a host whose secret does not open its room
/// (`rendezvous.rs`, `classify`), the same before the header and since.
const WRONG_SECRET: &str = "room secret does not open this room";

/// Whether a refused dial was a rendezvous from before the `Authorization` header, which reads
/// the secret only from the URL. It says so the one way it can: refusing a secret that does open
/// the room, with its own words. Any other 403 — a proxy, a WAF, a login page — is not it, and
/// gets no retry that would put the secret in a URL it logs.
fn predates_the_header(r: &Rendezvous, status: u16, body: Option<&[u8]>) -> bool {
    status == 403
        && crate::registry::room_for(&r.secret) == r.room
        && body.is_some_and(|b| String::from_utf8_lossy(b).contains(WRONG_SECRET))
}

/// Open the host socket, with the room secret in an `Authorization` header. A server from
/// before the header refuses that; it is dialed again the old way, secret in the query.
async fn dial(r: &Rendezvous) -> Result<Ws> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
    use tokio_tungstenite::tungstenite::{http, Error};
    // Nagle off: every line is its own message, flushed at once.
    let connect = |request| tokio_tungstenite::connect_async_with_config(request, None, true);
    let mut request = host_url(r).into_client_request()?;
    request
        .headers_mut()
        .insert(http::header::AUTHORIZATION, http::HeaderValue::from_str(&format!("Bearer {}", r.secret))?);
    match connect(request).await {
        Ok((ws, _)) => Ok(ws),
        Err(Error::Http(resp)) if predates_the_header(r, resp.status().as_u16(), resp.body().as_deref()) => {
            tracing::warn!(url = %r.url, "the rendezvous only reads the room secret from the URL, where its proxy logs record it; update `relay remote rendezvous` there");
            let legacy = format!("{}?secret={}", host_url(r), r.secret).into_client_request()?;
            Ok(connect(legacy).await?.0)
        }
        Err(e) => Err(e.into()),
    }
}

/// Every lane's task, aborted however `connect_once` ends, with its queue and the bytes it may have
/// waiting.
#[derive(Default)]
struct Lanes(HashMap<String, OpenLane>);

/// A lane's queue, its task, and its share of the bytes waiting.
type OpenLane = (mpsc::Sender<(String, Waiting)>, JoinHandle<()>, Arc<Semaphore>);

impl Drop for Lanes {
    fn drop(&mut self) {
        for (_, (_, task, _)) in self.0.drain() {
            task.abort();
        }
    }
}

/// Requests from one phone waiting for its bridge. The shared loop never waits on a lane: a lane
/// that lets this fill is closed, and the phone reconnects.
const LANE_INBOUND: usize = 256;
/// The bytes of them one lane, and every lane together, may have waiting. The count alone let a
/// lane hold 256 lines of up to [`MAX_LANE_LINE`] each, gigabytes, while its bridge sat behind a
/// slow request (a `money.sync` under the store's lock), and a lane is open to whoever joins the
/// room, before any handshake: a lane past either is closed the same way.
const LANE_INBOUND_BYTES: usize = 2 * MAX_LANE_LINE;
const INBOUND_BYTES: usize = 4 * MAX_LANE_LINE;

/// A waiting line's bytes, held against its lane's share and the total until its bridge takes it.
type Waiting = (OwnedSemaphorePermit, OwnedSemaphorePermit);

/// Room for a line of `bytes` in `lane`'s share and in `all`, or none.
fn waiting(lane: &Arc<Semaphore>, all: &Arc<Semaphore>, bytes: usize) -> Option<Waiting> {
    let n = u32::try_from(bytes.max(1)).ok()?;
    Some((lane.clone().try_acquire_many_owned(n).ok()?, all.clone().try_acquire_many_owned(n).ok()?))
}

/// One connection's lifetime. `Ok` means the server closed cleanly; `connected` is set once the
/// dial succeeded, whatever happens after.
pub async fn connect_once(ctx: Arc<Ctx>, rendezvous: &Rendezvous, connected: &AtomicBool) -> Result<()> {
    let ws = match tokio::time::timeout(DIAL_TIMEOUT, dial(rendezvous)).await {
        Ok(ws) => ws.with_context(|| format!("dialing {}", rendezvous.url))?,
        Err(_) => anyhow::bail!("dialing {}: no answer within {}s", rendezvous.url, DIAL_TIMEOUT.as_secs()),
    };
    connected.store(true, Ordering::Release);
    tracing::info!(url = %rendezvous.url, room = %rendezvous.room, "rendezvous connected");
    let (mut sink, mut source) = ws.split();

    // Every lane's outbound lines funnel through one sender to the socket, pings included.
    let (to_server, mut from_lanes) = mpsc::channel::<Message>(1024);
    let writer = tokio::spawn(async move {
        while let Some(msg) = from_lanes.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });
    // However this ends — an error, the silence limit, a close — the writer and every lane go.
    let _writer = AbortOnDrop(writer.abort_handle());
    let mut lanes = Lanes::default();
    let inbound = Arc::new(Semaphore::new(INBOUND_BYTES));
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_heard = tokio::time::Instant::now();
    loop {
        let msg = tokio::select! {
            next = source.next() => next,
            _ = ping.tick() => {
                if last_heard.elapsed() > SILENCE_LIMIT {
                    anyhow::bail!("no traffic from the rendezvous for {}s", SILENCE_LIMIT.as_secs());
                }
                // Never wait here: with the writer backed up, the silence check above is the
                // only thing that notices a dead socket.
                if let Err(mpsc::error::TrySendError::Closed(_)) = to_server.try_send(Message::Ping(Vec::new().into())) {
                    break;
                }
                continue;
            }
        };
        let Some(msg) = msg else { break };
        last_heard = tokio::time::Instant::now();
        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) => break,
            Err(e) => return Err(e).context("rendezvous socket"),
            Ok(_) => continue,
        };
        let Ok(lane) = serde_json::from_str::<Lane>(&text) else {
            continue;
        };
        match lane {
            Lane::Open { c } => {
                let (in_tx, mut queued) = mpsc::channel::<(String, Waiting)>(LANE_INBOUND);
                // The bridge takes one line at a time; a line's bytes count until it does.
                let (to_bridge, in_rx) = mpsc::channel::<String>(1);
                let hand_on = tokio::spawn(async move {
                    while let Some((line, _waiting)) = queued.recv().await {
                        if to_bridge.send(line).await.is_err() {
                            break;
                        }
                    }
                });
                let (out_tx, mut out_rx) = mpsc::channel::<String>(bridge::OUTBOUND_QUEUE);
                let ctx = ctx.clone();
                let to_server = to_server.clone();
                let id = c.clone();
                let task = tokio::spawn(async move {
                    let forward = {
                        let to_server = to_server.clone();
                        let id = id.clone();
                        tokio::spawn(async move {
                            while let Some(l) = out_rx.recv().await {
                                let line = Lane::Data { c: id.clone(), l }.to_line();
                                if line.len() > MAX_LANE_LINE {
                                    // Sent, it would break the socket every lane shares; closing
                                    // this lane costs one phone a reconnect instead.
                                    tracing::warn!(lane = %id, bytes = line.len(), "engine line too large for the rendezvous; closing the lane");
                                    let _ = to_server.send(Message::text(Lane::Close { c: id.clone() }.to_line())).await;
                                    break;
                                }
                                if to_server.send(Message::text(line)).await.is_err() {
                                    break;
                                }
                            }
                        })
                    };
                    // Aborting this lane must not leave the forwarder, or what hands lines on, behind.
                    let _forward_guard = AbortOnDrop(forward.abort_handle());
                    let _hand_on_guard = AbortOnDrop(hand_on.abort_handle());
                    if let Err(e) = bridge::run(ctx, in_rx, out_tx, (), "rendezvous".to_string()).await {
                        tracing::debug!(lane = %id, error = %e, "lane ended");
                    }
                    let _ = forward.await;
                    let _ = to_server.send(Message::text(Lane::Close { c: id }.to_line())).await;
                });
                lanes.0.insert(c, (in_tx, task, Arc::new(Semaphore::new(LANE_INBOUND_BYTES))));
            }
            Lane::Data { c, l } => {
                let Some((tx, _, share)) = lanes.0.get(&c) else { continue };
                let sent = match waiting(share, &inbound, l.len()) {
                    Some(held) => tx.try_send((l, held)).map_err(|e| matches!(e, mpsc::error::TrySendError::Closed(_))),
                    None => Err(false),
                };
                match sent {
                    Ok(()) => {}
                    Err(false) => {
                        // A lane whose bridge cannot keep up is closed rather than waited on:
                        // every other phone's lines go through this loop.
                        tracing::warn!(lane = %c, "lane fell behind; closing it");
                        if let Some((_, task, _)) = lanes.0.remove(&c) {
                            task.abort();
                        }
                        let _ = to_server.try_send(Message::text(Lane::Close { c }.to_line()));
                    }
                    Err(true) => {
                        lanes.0.remove(&c);
                    }
                }
            }
            Lane::Close { c } => {
                // The phone is gone; nothing it sent is still worth delivering.
                if let Some((_, task, _)) = lanes.0.remove(&c) {
                    task.abort();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A lane's waiting lines are bounded in bytes, its own and all lanes' together, and a line's
    /// bytes come back once its bridge has taken it.
    #[test]
    fn waiting_lines_are_bounded_in_bytes_per_lane_and_in_all() {
        let all = Arc::new(Semaphore::new(INBOUND_BYTES));
        let (a, b) = (Arc::new(Semaphore::new(LANE_INBOUND_BYTES)), Arc::new(Semaphore::new(LANE_INBOUND_BYTES)));
        let first = waiting(&a, &all, MAX_LANE_LINE).unwrap();
        let second = waiting(&a, &all, MAX_LANE_LINE).unwrap();
        assert!(waiting(&a, &all, 1).is_none(), "a lane holds two of the largest lines at most");
        let others: Vec<Waiting> = (0..2).map(|_| waiting(&b, &all, MAX_LANE_LINE).unwrap()).collect();
        let c = Arc::new(Semaphore::new(LANE_INBOUND_BYTES));
        assert!(waiting(&c, &all, 1).is_none(), "every lane together holds four");
        drop(first);
        assert!(waiting(&a, &all, MAX_LANE_LINE).is_some(), "taken by its bridge, a line's bytes come back");
        drop((second, others));
        assert!(waiting(&c, &all, 0).is_some(), "an empty line still counts one byte");
        assert!(waiting(&c, &all, usize::MAX).is_none());
    }

    fn rendezvous(addr: std::net::SocketAddr) -> Rendezvous {
        let secret = "s3cret".to_string();
        Rendezvous { url: format!("ws://{addr}"), room: crate::registry::room_for(&secret), secret, public: None }
    }

    /// Answers each connection's request head with `answer`, and records the request line.
    async fn refusing(answer: &'static str) -> (std::net::SocketAddr, mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (seen, heard) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = seen.send(String::from_utf8_lossy(&head).lines().next().unwrap_or_default().to_string());
                let _ = stream.write_all(answer.as_bytes()).await;
            }
        });
        (addr, heard)
    }

    #[test]
    fn only_the_old_rendezvous_own_refusal_reads_as_old() {
        let r = rendezvous("127.0.0.1:1".parse().unwrap());
        let old = WRONG_SECRET;
        assert!(predates_the_header(&r, 403, Some(old.as_bytes())));
        assert!(predates_the_header(&r, 403, Some(format!("23\r\n{old}\r\n0\r\n\r\n").as_bytes())), "chunked by a proxy");
        assert!(!predates_the_header(&r, 403, Some(b"<html>Access denied</html>")), "a proxy's 403");
        assert!(!predates_the_header(&r, 403, None));
        assert!(!predates_the_header(&r, 401, Some(old.as_bytes())));
        let mismatched = Rendezvous { room: "0".repeat(32), ..r };
        assert!(!predates_the_header(&mismatched, 403, Some(old.as_bytes())), "the URL would not open it either");
    }

    #[tokio::test]
    async fn a_403_from_anything_else_never_puts_the_secret_in_a_url() {
        let (addr, mut heard) = refusing("HTTP/1.1 403 Forbidden\r\nContent-Length: 13\r\nConnection: close\r\n\r\nAccess denied").await;
        let r = rendezvous(addr);
        assert!(dial(&r).await.is_err());
        let first = heard.recv().await.unwrap();
        assert!(first.starts_with(&format!("GET /host/{} ", r.room)), "{first}");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(heard.try_recv().is_err(), "dialed once, and never with the secret in the URL");
    }

    #[tokio::test]
    async fn the_old_rendezvous_is_dialed_again_the_old_way() {
        let refusal = "HTTP/1.1 403 Forbidden\r\nContent-Length: 35\r\nConnection: close\r\n\r\nroom secret does not open this room";
        let (addr, mut heard) = refusing(refusal).await;
        let r = rendezvous(addr);
        // The fake refuses the retry too; what matters is that it came, and how.
        assert!(dial(&r).await.is_err());
        assert!(!heard.recv().await.unwrap().contains(&r.secret));
        let retry = heard.recv().await.unwrap();
        assert!(retry.starts_with(&format!("GET /host/{}?secret={} ", r.room, r.secret)), "{retry}");
    }
}
