//! Bounded, concurrent socket transport. Each visible terminal gets its own connection,
//! so terminal backpressure cannot starve the application's control plane.
use relay_bus::envelope::{Event, Frame, Request, Response, ENVELOPE_V};
use relay_bus::{Actor, BusError};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;
use uuid::Uuid;

/// Replies awaited on one connection. `None` once the connection is gone: taking the map and
/// refusing new requests happen under the same lock, so no request slips in after the drain.
type Pending = Arc<Mutex<Option<HashMap<Uuid, oneshot::Sender<Result<Value, Error>>>>>>;

/// Which of the three line shapes a socket line carries.
enum Shape {
    Event,
    Frame,
    Response,
}

/// The shape of `line`, read from its leading bytes.
///
/// Every PTY frame the engine sends lands here, and a frame's payload is the bulk of the line,
/// so the cost that matters is how many times the line is scanned. Each envelope serializes its
/// fields in declaration order, which puts a distinguishing key first and lets the common case
/// be exactly one typed parse. `None` means "not recognizable from the prefix", and the caller
/// falls back to a `serde_json::Value` to decide — so a reordered field costs a little speed
/// and never correctness. A matched prefix has also proven the envelope version.
fn shape(line: &[u8]) -> Option<Shape> {
    let rest = line.strip_prefix(br#"{"v":1,""#)?;
    if rest.starts_with(br#"ev""#) {
        Some(Shape::Event)
    } else if rest.starts_with(br#"stream""#) {
        Some(Shape::Frame)
    } else if rest.starts_with(br#"id""#) {
        Some(Shape::Response)
    } else {
        None
    }
}

const QUEUE: usize = 64;
const MAX_LINE: usize = 2 * 1024 * 1024;
/// How much of an oversized line is kept: enough for the envelope's first key and its value.
const OVERSIZED_HEAD: usize = 256;
/// Synthesized in place of an event too large to read: the listener refreshes instead.
pub const DROPPED_EVENT: &str = "client.event_dropped";

/// The string value that directly follows `prefix` at the start of `head`.
fn head_string(head: &[u8], prefix: &[u8]) -> Option<String> {
    let rest = head.strip_prefix(prefix)?;
    let end = rest.iter().position(|b| *b == b'"')?;
    String::from_utf8(rest[..end].to_vec()).ok()
}

pub fn is_lifecycle_request(op: &str) -> bool {
    matches!(
        op,
        "session.create"
            | "session.close"
            | "session.park"
            | "session.wake"
            | "session.resume"
            | "session.spawn"
            | "session.clear_restorable"
            | "session.discard_restorable"
            | "task.dispatch"
            // A clone may take many minutes; on the main connection every other request
            // would wait behind it.
            | "project.clone"
    )
}

fn request_timeout(op: &str) -> Duration {
    match op {
        "project.clone" => Duration::from_secs(1800),
        // A new checkout may hydrate large LFS assets after the bounded fetch.
        "session.create" | "worktree.create" | "task.dispatch" => Duration::from_secs(180),
        // A backtest reads up to two years of candles from the exchange, 350 at a time.
        "arbiter.backtest" => Duration::from_secs(180),
        // These wait on the exchange: a key's check, a market sell, cancelling every order.
        "arbiter.key.set" | "arbiter.flatten" | "arbiter.halt" | "arbiter.refresh" => Duration::from_secs(60),
        // Years of training: a file of tens of megabytes, read and written whole.
        "gym.import" => Duration::from_secs(120),
        _ => Duration::from_secs(30),
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Bus(Box<BusError>),
    #[error("Connection closed. Reconnect to the engine.")]
    Disconnected,
    #[error("Request timed out. Its outcome is unknown; refresh before retrying.")]
    Timeout,
    #[error("Invalid engine response: {0}")]
    Protocol(String),
    #[error("The engine's reply was larger than {} MiB and was dropped; the connection is still open.", .0 / 1024 / 1024)]
    TooLarge(usize),
    #[error("{0}")]
    Io(String),
}

impl From<BusError> for Error {
    fn from(error: BusError) -> Self {
        Self::Bus(Box::new(error))
    }
}

#[derive(Debug)]
pub enum Notice {
    Event(Event),
    Frame(Frame),
    Disconnected(Error),
}

struct Connection {
    tx: mpsc::Sender<Request>,
    pending: Pending,
    tasks: Vec<AbortHandle>,
    notices: async_channel::Sender<Notice>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        self.pending.lock().unwrap().take();
        self.notices.close();
    }
}

#[derive(Clone)]
pub struct Client(Arc<Connection>);

impl Client {
    /// Binary file responses use a separate bounded connection. Keep the normal
    /// control/event channel's 2 MiB limit and responsiveness unchanged.
    pub async fn image_read(rt: &Handle, path: PathBuf, payload: Value) -> Result<Value, Error> {
        Self::bulk_request(rt, path, "file.read", payload).await
    }
    /// A request that carries a picture either way (`file.read`, `thread.upload`, `thread.image`):
    /// on a connection of its own whose lines may reach 24 MiB, so a picture neither fails the
    /// control connection's 2 MiB limit nor holds the keystrokes queued behind it.
    pub async fn bulk_request(rt: &Handle, path: PathBuf, op: &str, payload: Value) -> Result<Value, Error> {
        let (client, _notices) = rt
            .spawn(async move { Self::open_with_limit(path, 24 * 1024 * 1024).await })
            .await
            .map_err(|e| Error::Io(e.to_string()))??;
        client.request(rt, op, payload).await
    }
    /// Session lifecycle actions must not sit behind slow reads on the UI socket.
    /// This connection has no subscriptions and exists only for this one action.
    pub async fn lifecycle_request(
        rt: &Handle,
        path: PathBuf,
        op: &str,
        payload: Value,
    ) -> Result<Value, Error> {
        let (client, _notices) = Self::connect(rt, path).await?;
        client.request(rt, op, payload).await
    }

    pub async fn connect(
        rt: &Handle,
        path: PathBuf,
    ) -> Result<(Self, async_channel::Receiver<Notice>), Error> {
        rt.spawn(async move { Self::open(path).await })
            .await
            .map_err(|e| Error::Io(e.to_string()))?
    }

    /// A connection whose frames may be larger than the control channel's 2 MiB: the device
    /// mirror's video stream, where one key frame at a high capture rung arrives as a single
    /// base64 line. Nothing else should need it.
    pub async fn connect_with_limit(
        rt: &Handle,
        path: PathBuf,
        max_line: usize,
    ) -> Result<(Self, async_channel::Receiver<Notice>), Error> {
        rt.spawn(async move { Self::open_with_limit(path, max_line).await })
            .await
            .map_err(|e| Error::Io(e.to_string()))?
    }

    async fn open(path: PathBuf) -> Result<(Self, async_channel::Receiver<Notice>), Error> {
        Self::open_with_limit(path, MAX_LINE).await
    }

    async fn open_with_limit(
        path: PathBuf,
        max_line: usize,
    ) -> Result<(Self, async_channel::Receiver<Notice>), Error> {
        let socket = UnixStream::connect(&path)
            .await
            .map_err(|e| Error::Io(format!("Cannot connect to {}: {e}", path.display())))?;
        let (read, mut write) = socket.into_split();
        let pending: Pending = Arc::new(Mutex::new(Some(HashMap::new())));
        let (tx, mut rx) = mpsc::channel::<Request>(QUEUE);
        let (notices, receiver) = async_channel::bounded(QUEUE);
        let waiting = pending.clone();
        let reader_notices = notices.clone();
        let reader = tokio::spawn(async move {
            // A connection allowed lines past the 2 MiB control limit carries video: a picture's
            // line is tens to hundreds of KiB, which an 8 KiB buffer took a read call per 8 KiB of.
            let capacity = if max_line > MAX_LINE { 256 * 1024 } else { 8 * 1024 };
            let mut reader = BufReader::with_capacity(capacity, read);
            let outcome: Result<(), Error> = async {
                loop {
                    // read_until alone has no size limit. Bound the frame before allocation.
                    let mut line = Vec::new();
                    let mut oversized = false;
                    loop {
                        let available = reader
                            .fill_buf()
                            .await
                            .map_err(|e| Error::Io(e.to_string()))?;
                        if available.is_empty() {
                            return Err(Error::Disconnected);
                        }
                        let n = available
                            .iter()
                            .position(|b| *b == b'\n')
                            .map_or(available.len(), |n| n + 1);
                        let complete = available[n - 1] == b'\n';
                        if !oversized && line.len() + n > max_line {
                            // Keep only the head, which says what the line was and whose reply
                            // it is, and skip the rest without buffering it.
                            oversized = true;
                            let keep = OVERSIZED_HEAD.saturating_sub(line.len()).min(n);
                            line.extend_from_slice(&available[..keep]);
                            line.truncate(OVERSIZED_HEAD);
                        } else if !oversized {
                            line.extend_from_slice(&available[..n]);
                        }
                        reader.consume(n);
                        if complete {
                            break;
                        }
                    }
                    // RELAY_MIRROR_TRACE only (0 otherwise): when a video packet's line was complete.
                    let line_done = crate::mirror::trace::now();
                    if oversized {
                        // One reply or event too large for this connection fails on its own;
                        // the connection, its subscriptions and every other request stay up.
                        match shape(&line) {
                            Some(Shape::Response) => {
                                if let Some(id) = head_string(&line, br#"{"v":1,"id":""#)
                                    .and_then(|id| Uuid::parse_str(&id).ok())
                                {
                                    let reply = waiting.lock().unwrap().as_mut().and_then(|w| w.remove(&id));
                                    if let Some(reply) = reply {
                                        let _ = reply.send(Err(Error::TooLarge(max_line)));
                                    }
                                    continue;
                                }
                            }
                            Some(Shape::Event) => {
                                let ev = head_string(&line, br#"{"v":1,"ev":""#).unwrap_or_default();
                                let dropped = Event::new(
                                    DROPPED_EVENT,
                                    String::new(),
                                    Actor::System,
                                    serde_json::json!({ "ev": ev }),
                                );
                                reader_notices
                                    .send(Notice::Event(dropped))
                                    .await
                                    .map_err(|_| Error::Disconnected)?;
                                continue;
                            }
                            _ => {}
                        }
                        return Err(Error::Protocol(format!(
                            "frame exceeds {} MiB",
                            max_line / 1024 / 1024
                        )));
                    }
                    let shape = match shape(&line) {
                        Some(shape) => shape,
                        None => {
                            let value: Value = serde_json::from_slice(&line)
                                .map_err(|e| Error::Protocol(e.to_string()))?;
                            if value["v"].as_u64() != Some(ENVELOPE_V.into()) {
                                return Err(Error::Protocol(
                                    "unsupported envelope version".into(),
                                ));
                            }
                            if value.get("ev").is_some() {
                                Shape::Event
                            } else if value.get("stream").is_some() {
                                Shape::Frame
                            } else {
                                Shape::Response
                            }
                        }
                    };
                    if matches!(shape, Shape::Event) {
                        let event = serde_json::from_slice(&line)
                            .map_err(|e| Error::Protocol(e.to_string()))?;
                        reader_notices
                            .send(Notice::Event(event))
                            .await
                            .map_err(|_| Error::Disconnected)?;
                    } else if matches!(shape, Shape::Frame) {
                        let frame: Frame = serde_json::from_slice(&line)
                            .map_err(|e| Error::Protocol(e.to_string()))?;
                        if line_done != 0 && frame.stream == "mirror" && frame.data.is_string() {
                            crate::mirror::trace::trace!(stage = "recv", mono_ns = crate::mirror::trace::monotonic(), mirror_id = frame.mirror_id.unwrap_or(0), seq = frame.seq, line_ns = line_done, bytes = line.len());
                        }
                        reader_notices
                            .send(Notice::Frame(frame))
                            .await
                            .map_err(|_| Error::Disconnected)?;
                    } else {
                        let response: Response = serde_json::from_slice(&line)
                            .map_err(|e| Error::Protocol(e.to_string()))?;
                        let id = response
                            .id
                            .ok_or_else(|| Error::Protocol("response has no request id".into()))?;
                        let reply = waiting.lock().unwrap().as_mut().and_then(|w| w.remove(&id));
                        if let Some(reply) = reply {
                            let _ = reply.send(response.into_result().map_err(Error::from));
                        }
                    }
                }
            }
            .await;
            let error = outcome.unwrap_err();
            // Taking the map refuses new requests in the same step, so none is written after the
            // drain only to wait out its timeout. Notify, then close so drains cannot wait forever.
            let waiting = waiting.lock().unwrap().take();
            for (_, reply) in waiting.into_iter().flatten() {
                let _ = reply.send(Err(error.clone()));
            }
            let _ = reader_notices.send(Notice::Disconnected(error)).await;
            reader_notices.close();
        });
        let waiting = pending.clone();
        let writer_notices = notices.clone();
        let reader_abort = reader.abort_handle();
        let writer = tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                let mut bytes = serde_json::to_vec(&request).expect("request serialization");
                bytes.push(b'\n');
                if let Err(e) = write.write_all(&bytes).await {
                    reader_abort.abort();
                    let waiting = waiting.lock().unwrap().take();
                    for (_, reply) in waiting.into_iter().flatten() {
                        let _ = reply.send(Err(Error::Disconnected));
                    }
                    let _ = writer_notices
                        .send(Notice::Disconnected(Error::Io(e.to_string())))
                        .await;
                    writer_notices.close();
                    break;
                }
            }
        });
        Ok((
            Self(Arc::new(Connection {
                tx,
                pending,
                tasks: vec![reader.abort_handle(), writer.abort_handle()],
                notices,
            })),
            receiver,
        ))
    }

    /// Write `op` behind everything sent before it from the same caller, without waiting for its
    /// answer, which the reader then drops. For a stream of small requests whose order matters and
    /// whose answers do not — mirror input, where waiting out each round trip before sending the
    /// next touch was latency added to every move. `request` cannot be used back to back for
    /// that: each one is queued from its own task, so two of them may reach the socket swapped.
    /// Waits only while the writer's queue is full.
    pub async fn send_unanswered(&self, op: &str, payload: Value) -> Result<(), Error> {
        let request = Request::new(Actor::User, op, payload);
        self.0.tx.send(request).await.map_err(|_| Error::Disconnected)
    }

    pub async fn request(&self, rt: &Handle, op: &str, payload: Value) -> Result<Value, Error> {
        self.request_with_id(rt, op, payload, Uuid::new_v4()).await
    }

    pub async fn request_with_id(
        &self,
        rt: &Handle,
        op: &str,
        payload: Value,
        id: Uuid,
    ) -> Result<Value, Error> {
        let mut request = Request::new(Actor::User, op, payload);
        request.id = id;
        let (send, reply) = oneshot::channel();
        match self.0.pending.lock().unwrap().as_mut() {
            Some(pending) => pending.insert(id, send),
            None => return Err(Error::Disconnected),
        };
        // Clean up even if the GTK future is cancelled while awaiting an answer.
        struct Remove(Pending, Uuid);
        impl Drop for Remove {
            fn drop(&mut self) {
                if let Some(pending) = self.0.lock().unwrap().as_mut() {
                    pending.remove(&self.1);
                }
            }
        }
        let _remove = Remove(self.0.pending.clone(), id);
        let tx = self.0.tx.clone();
        let timeout = request_timeout(op);
        rt.spawn(async move {
            tokio::time::timeout(timeout, async {
                tx.send(request).await.map_err(|_| Error::Disconnected)?;
                reply.await.map_err(|_| Error::Disconnected)?
            })
            .await
            .map_err(|_| Error::Timeout)?
        })
        .await
        .map_err(|_| Error::Disconnected)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn an_oversized_reply_fails_only_its_own_request() {
        let path = std::env::temp_dir().join(format!("relay-oversized-{}.sock", Uuid::new_v4()));
        let server = UnixListener::bind(&path).unwrap();
        let rt = Handle::current();
        let (client, notices) = Client::connect(&rt, path.clone()).await.unwrap();
        let fixture = tokio::spawn(async move {
            let (socket, _) = server.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            let big: Request = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let body = "x".repeat(MAX_LINE + 1024);
            let reply = Response::ok(big.id, serde_json::json!({ "body": body }));
            write.write_all(format!("{}\n", serde_json::to_string(&reply).unwrap()).as_bytes()).await.unwrap();
            let event = Event::new("notes.changed", String::new(), Actor::User, serde_json::json!({ "body": body }));
            write.write_all(format!("{}\n", serde_json::to_string(&event).unwrap()).as_bytes()).await.unwrap();
            let small: Request = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let reply = Response::ok(small.id, serde_json::json!({ "ok": true }));
            write.write_all(format!("{}\n", serde_json::to_string(&reply).unwrap()).as_bytes()).await.unwrap();
            // Hold the socket open until the client is done with it.
            let _ = lines.next_line().await;
        });
        let big = client.request(&rt, "git.diff.file", serde_json::json!({})).await;
        assert!(matches!(big, Err(Error::TooLarge(_))), "{big:?}");
        match notices.recv().await.unwrap() {
            Notice::Event(event) => {
                assert_eq!(event.ev, DROPPED_EVENT);
                assert_eq!(event.payload["ev"], "notes.changed");
            }
            other => panic!("expected the dropped-event notice, got {other:?}"),
        }
        let small = client.request(&rt, "git.status", serde_json::json!({})).await.unwrap();
        assert_eq!(small["ok"], true);
        drop(client);
        fixture.await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn lifecycle_actions_bypass_a_stalled_status_request() {
        for op in ["session.close", "session.create", "task.dispatch"] {
            lifecycle_bypasses_a_stalled_status_request(op).await;
        }
    }

    async fn lifecycle_bypasses_a_stalled_status_request(op: &'static str) {
        let path = std::env::temp_dir().join(format!("relay-lifecycle-{}.sock", Uuid::new_v4()));
        let server = UnixListener::bind(&path).unwrap();
        let rt = Handle::current();
        let (client, _notices) = Client::connect(&rt, path.clone()).await.unwrap();
        let (started, blocked) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let fixture = tokio::spawn(async move {
            let (socket, _) = server.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            let slow: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(slow.op, "git.status");
            started.send(()).unwrap();
            let (action, _) = server.accept().await.unwrap();
            let (read, mut action_write) = action.into_split();
            let mut lines = BufReader::new(read).lines();
            let close: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(close.op, op);
            action_write
                .write_all(
                    format!(
                        "{}\n",
                        serde_json::to_string(&Response::ok(close.id, serde_json::json!({})))
                            .unwrap()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            released.await.unwrap();
            write
                .write_all(
                    format!(
                        "{}\n",
                        serde_json::to_string(&Response::ok(slow.id, serde_json::json!({})))
                            .unwrap()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let handle = rt.clone();
        let action_client = client.clone();
        let slow = tokio::spawn(async move {
            client
                .request(&handle, "git.status", serde_json::json!({}))
                .await
        });
        blocked.await.unwrap();
        tokio::time::timeout(Duration::from_millis(500), async {
            if is_lifecycle_request(op) {
                Client::lifecycle_request(
                    &rt,
                    path.clone(),
                    op,
                    serde_json::json!({"session":"fixture"}),
                )
                .await
            } else {
                action_client
                    .request(&rt, op, serde_json::json!({"session":"fixture"}))
                    .await
            }
        })
        .await
        .expect("Lifecycle action queued behind slow Git read")
        .unwrap();
        assert!(
            !slow.is_finished(),
            "The status request must still be blocked"
        );
        release.send(()).unwrap();
        slow.await.unwrap().unwrap();
        fixture.await.unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn correlates_out_of_order_replies_events_and_disconnect() {
        let path = std::env::temp_dir().join(format!("relay-native-{}.sock", Uuid::new_v4()));
        let server = UnixListener::bind(&path).unwrap();
        let rt = Handle::current();
        let (client, notices) = Client::connect(&rt, path.clone()).await.unwrap();
        let request_id = Uuid::new_v4();
        let fixture = tokio::spawn(async move {
            let (socket, _) = server.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            let a: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let b: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let tagged = if a.op == "a" { &a } else { &b };
            assert_eq!(tagged.id, request_id);
            assert_eq!(a.actor, Actor::User);
            assert!(a.token.is_none());
            let event = serde_json::json!({"v":1,"ev":"task.changed","ts":"2026-09-04T00:00:00Z","actor":"user","payload":{}});
            for line in [
                event,
                serde_json::to_value(Response::ok(b.id, serde_json::json!(b.op))).unwrap(),
                serde_json::to_value(Response::ok(a.id, serde_json::json!(a.op))).unwrap(),
            ] {
                write
                    .write_all(format!("{line}\n").as_bytes())
                    .await
                    .unwrap();
            }
            let _: Request =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        });
        let (a, b) = tokio::join!(
            client.request_with_id(&rt, "a", Value::Null, request_id),
            client.request(&rt, "b", Value::Null)
        );
        assert_eq!(a.unwrap(), "a");
        assert_eq!(b.unwrap(), "b");
        assert!(matches!(notices.recv().await.unwrap(), Notice::Event(_)));
        assert!(client.request(&rt, "c", Value::Null).await.is_err());
        fixture.await.unwrap();
        assert!(client.0.pending.lock().unwrap().is_none());
        std::fs::remove_file(path).unwrap();
    }
}
