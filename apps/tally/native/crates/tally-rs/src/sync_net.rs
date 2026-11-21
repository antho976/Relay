//! The network under sync: Relay's door over a WebSocket (Kotlin: link/RelayClient.kt) and the
//! request to connect over HTTPS (link/Cloud.kt). Everything here blocks, so it only ever runs on
//! sync's own threads, never the UI's.
//!
//! The two ways out, [`Dial`] (a door's lines) and [`Http`] (the server's API), are traits, so the
//! tests put a fake server behind them and never reach a real one.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::sync::Failure;
use crate::sync_wire::{self as wire, Greeting, PairLink, Welcome};

/// Failure codes of Tally's own (the bus's pass through as they are).
pub mod code {
    /// The server, or the way to it, could not be reached or did not answer.
    pub const UNREACHABLE: &str = "tally.unreachable";
    /// The door no longer knows this phone: it has to connect again.
    pub const UNPAIRED: &str = "tally.unpaired";
    /// The door refused a pairing or a connection.
    pub const DENIED: &str = "tally.denied";
    /// The server has no Relay, or turned Tally's sync off: asking again will not help.
    pub const CLOUD_OFF: &str = "tally.cloud_off";
    /// The server answered something that does not read.
    pub const ANSWER: &str = "tally.answer";
    /// An op Tally RS never sends from a screen (one that replaces or erases the server's ledger).
    pub const REFUSED: &str = "tally.refused";
    /// An address Tally RS will not use: unencrypted, and not on this phone's own network.
    pub const INSECURE: &str = "tally.insecure";
}

pub fn fail(code: &str, message: impl Into<String>) -> Failure {
    Failure { code: code.into(), message: message.into() }
}

fn unreachable_() -> Failure {
    fail(code::UNREACHABLE, "Your server could not be reached")
}

fn too_slow() -> Failure {
    fail(code::UNREACHABLE, "Your server did not answer in time")
}

fn closed() -> Failure {
    fail(code::UNREACHABLE, "Your server closed the connection")
}

fn insecure() -> Failure {
    fail(code::INSECURE, "Tally RS reaches a server outside your own network over https only. Use its https:// address.")
}

/// [`insecure`] unless [`wire::may_reach`] takes `url`.
fn reachable(url: &str) -> Result<(), Failure> {
    if wire::may_reach(url) { Ok(()) } else { Err(insecure()) }
}

/// One connection to a door, as lines.
pub trait Lines: Send {
    fn send(&mut self, line: &str) -> Result<(), Failure>;
    /// The next line, waiting at most `wait` for it.
    fn next(&mut self, wait: Duration) -> Result<String, Failure>;
}

/// Opens connections to a door's routes (`ws://…`, `wss://…`).
pub trait Dial: Send + Sync {
    fn open(&self, route: &str) -> Result<Box<dyn Lines>, Failure>;
}

/// The server's JSON API: the status and the object it answered with (None when it was not one).
pub trait Http: Send + Sync {
    fn get(&self, url: &str) -> Result<(u16, Option<Value>), Failure>;
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Option<Value>), Failure>;
}

/// The way out: the real network, or a test's fake.
#[derive(Clone)]
pub struct Net {
    pub dial: Arc<dyn Dial>,
    pub http: Arc<dyn Http>,
}

impl Net {
    /// The real network. In tests, a network that refuses everything, so no test can reach the
    /// owner's server (a request to connect sends them a DM).
    pub fn real() -> Net {
        #[cfg(not(test))]
        {
            Net { dial: Arc::new(WsDial), http: Arc::new(UreqHttp::new()) }
        }
        #[cfg(test)]
        {
            Net { dial: Arc::new(Offline), http: Arc::new(Offline) }
        }
    }
}

/// No network at all.
pub struct Offline;

impl Dial for Offline {
    fn open(&self, _route: &str) -> Result<Box<dyn Lines>, Failure> {
        Err(unreachable_())
    }
}

impl Http for Offline {
    fn get(&self, _url: &str) -> Result<(u16, Option<Value>), Failure> {
        Err(unreachable_())
    }
    fn post(&self, _url: &str, _body: &Value) -> Result<(u16, Option<Value>), Failure> {
        Err(unreachable_())
    }
}

// The door

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const GREETING_TIMEOUT: Duration = Duration::from_secs(15);
pub const WELCOME_TIMEOUT: Duration = Duration::from_secs(20);
/// A pairing may wait on a person at the other end: the door gives them 75 s.
pub const PAIR_WELCOME_TIMEOUT: Duration = Duration::from_secs(90);
/// A large first sync page is one request; Relay answers it in one transaction.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// One admitted connection.
pub struct Session {
    lines: Box<dyn Lines>,
    pub greeting: Greeting,
    pub welcome: Welcome,
}

/// Sends one bus request and waits for its answer: the result, or the bus error.
pub trait Requester {
    fn request(&mut self, op: &str, payload: &Value, wait: Duration) -> Result<Value, Failure>;
}

impl Requester for Session {
    fn request(&mut self, op: &str, payload: &Value, wait: Duration) -> Result<Value, Failure> {
        let id = uuid::Uuid::new_v4().to_string();
        self.lines.send(&wire::request(op, payload, &id))?;
        let until = Instant::now() + wait;
        let response = loop {
            let left = until.checked_duration_since(Instant::now()).ok_or_else(too_slow)?;
            if let Some(r) = wire::response(&self.lines.next(left)?, &id) {
                break r;
            }
        };
        if response.ok {
            return Ok(response.result);
        }
        let code = response.code.unwrap_or_else(|| "bus.error".into());
        let message = match code.as_str() {
            "remote.op" => "Your server's Relay does not take this from Tally. Update Relay on the server.".to_string(),
            "bus.unknown_op" => "Your server's Relay does not know this. Update Relay on the server.".to_string(),
            _ => response.message.unwrap_or_else(|| "Your server refused the request".into()),
        };
        Err(Failure { code, message })
    }
}

/// The door's refusal codes (relay-remote bridge.rs), in plain words.
pub fn denial(code_: Option<&str>) -> Failure {
    match code_ {
        Some("pair.invalid") => fail(code::DENIED, "The pairing code was wrong or has expired. Connect again."),
        Some("pair.declined") => fail(code::DENIED, "Your server declined this phone."),
        Some("pair.unconfirmed") => fail(code::DENIED, "Nobody approved this phone in time. Connect again."),
        Some("auth.unknown_device") => fail(code::UNPAIRED, "Your server no longer knows this phone. Connect it again."),
        Some("auth.bad_proof") => fail(code::UNPAIRED, "Your server no longer accepts this phone's key. Connect it again."),
        None => fail(code::DENIED, "Something answered at that address, but not Relay"),
        Some(c) => fail(code::DENIED, format!("Your server refused the connection ({c})")),
    }
}

/// Tries each route until a door greets, answers with `hello`, and hands back the admitted
/// connection: [`wire::ordered_routes`] of them (a pairing kept from before that order is tried
/// in it too). A refusal after the hello is final (a spent code, a revoked phone); a route that
/// cannot be reached moves on to the next. `one_shot`: pairing, where the code may be spent once
/// sent, so no other route is tried after it went.
///
/// A door at a clear address on the network proves nothing about itself, so its saying it does
/// not know this phone is not believed: that reads as the server not reached, never as unpaired.
pub fn connect(dial: &dyn Dial, routes: &[String], hello: &dyn Fn(&Greeting) -> String, welcome_wait: Duration, one_shot: bool) -> Result<Session, Failure> {
    if routes.is_empty() {
        return Err(fail(code::UNREACHABLE, "There is no address to reach your server at. Connect again."));
    }
    let routes = wire::ordered_routes(routes);
    if routes.is_empty() {
        return Err(fail(code::INSECURE, "Your server offers this phone only unencrypted addresses outside your own network, and Tally RS sends its key over none of them."));
    }
    let mut last = unreachable_();
    for route in &routes {
        let mut lines = match dial.open(route) {
            Ok(l) => l,
            Err(f) => {
                last = f;
                continue;
            }
        };
        let greeting = match lines.next(GREETING_TIMEOUT) {
            Ok(line) => wire::greeting(&line),
            Err(f) => {
                last = f;
                continue;
            }
        };
        let greeting = match greeting {
            None => {
                last = fail(code::UNREACHABLE, "Something answered at that address, but not Relay");
                continue;
            }
            Some(g) if g.refusal.is_some() => {
                last = match g.refusal.as_deref() {
                    Some("host.offline") => fail(code::UNREACHABLE, "Relay is not running on your server right now"),
                    r => fail(code::UNREACHABLE, format!("Your server refused the connection ({})", r.unwrap_or("?"))),
                };
                continue;
            }
            Some(g) => g,
        };
        if let Err(f) = lines.send(&hello(&greeting)) {
            last = f;
            if one_shot {
                break;
            }
            continue;
        }
        let welcome = match lines.next(welcome_wait) {
            Ok(line) => wire::welcome(&line),
            Err(f) => {
                last = f;
                if one_shot {
                    break;
                }
                continue;
            }
        };
        match welcome {
            Some(w) if w.ok => return Ok(Session { lines, greeting, welcome: w }),
            w => {
                let f = denial(w.and_then(|w| w.error).as_deref());
                if f.code == code::UNPAIRED && !wire::is_secure(route) && !wire::is_loopback(route) {
                    last = fail(code::UNREACHABLE, "Something at your server's unencrypted address refused this phone's key. Tally RS does not take that as your server forgetting this phone; if it did, disconnect and connect again.");
                    if one_shot {
                        break;
                    }
                    continue;
                }
                return Err(f);
            }
        }
    }
    Err(last)
}

/// Where this phone is paired: the door's routes and the key it gave. The token never leaves
/// this phone again; each connection sends a proof made from it.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Creds {
    pub host: String,
    pub routes: Vec<String>,
    pub device: String,
    pub token: String,
    pub paired_at: i64,
}

impl std::fmt::Debug for Creds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the token, whatever prints this.
        f.debug_struct("Creds").field("host", &self.host).field("routes", &self.routes).field("device", &self.device).finish_non_exhaustive()
    }
}

/// A connection proving this phone's key.
pub fn open(dial: &dyn Dial, c: &Creds) -> Result<Session, Failure> {
    connect(dial, &c.routes, &|g| wire::proof_hello(&c.device, &g.challenge, &c.token), WELCOME_TIMEOUT, false)
}

/// Pairs with the door behind `link`: its key, kept by the caller.
pub fn pair(dial: &dyn Dial, link: &PairLink, device_name: &str, now_ms: i64) -> Result<Creds, Failure> {
    let routes = link.routes();
    let name: String = device_name.chars().take(64).collect();
    let session = connect(dial, &routes, &|_| wire::pair_hello(&link.code, &name), PAIR_WELCOME_TIMEOUT, true)?;
    let (Some(device), Some(token)) = (session.welcome.device.clone().filter(|d| !d.is_empty()), session.welcome.token.clone().filter(|t| !t.is_empty())) else {
        return Err(fail(code::DENIED, "Your server let this phone in without a key. Connect again."));
    };
    let host = [session.welcome.host.clone().unwrap_or_default(), session.greeting.host.clone(), link.host.clone()].into_iter().find(|h| !h.trim().is_empty()).unwrap_or_default();
    Ok(Creds { host, routes, device, token, paired_at: now_ms })
}

// The request to connect

/// A request in flight: kept until it is answered, so a restart resumes it.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Enrollment {
    pub server: String,
    pub id: String,
    pub verifier: String,
    pub code: String,
    pub expires_at: i64,
    /// The bot reached the owner on Discord; false sends them to the dashboard instead.
    pub notified: bool,
}

impl std::fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enrollment").field("server", &self.server).field("code", &self.code).field("expires_at", &self.expires_at).finish_non_exhaustive()
    }
}

/// What the server said about this phone's request.
#[derive(Clone, Debug, PartialEq)]
pub enum Claim {
    Waiting,
    Denied,
    Expired,
    /// Allowed, and its links already used up: ask again.
    Spent,
    Ready(PairLink),
    Trouble(String),
}

fn str_of(o: &Option<Value>, key: &str) -> Option<String> {
    o.as_ref()?.get(key)?.as_str().map(str::to_string)
}

/// Whether the server carries Relay's cloud copy and takes requests (`GET /api/tally`).
pub fn hello(http: &dyn Http, server: &str) -> Result<bool, Failure> {
    reachable(&wire::api(server))?;
    let (_, o) = http.get(&wire::api(server))?;
    if str_of(&o, "service").as_deref() != Some("tally-cloud") {
        return Err(fail(code::ANSWER, "That address does not answer as your server"));
    }
    Ok(o.and_then(|o| o.get("cloud").and_then(Value::as_bool)).unwrap_or(false))
}

/// How long the server keeps a request to connect open (its `TTL_MS`).
pub const ENROLL_TTL_MS: i64 = 15 * 60_000;

/// Asks to sync. The owner gets a DM; the answer carries what to poll [`claim`] with.
pub fn enroll(http: &dyn Http, server: &str, device_name: &str, now_ms: i64) -> Result<Enrollment, Failure> {
    reachable(&wire::api(server))?;
    let verifier = wire::new_verifier();
    let challenge = wire::challenge_of(&verifier);
    let name: String = device_name.chars().take(100).collect();
    let body = json!({"kind": "tally", "device": name, "challenge": challenge});
    let (status, o) = http.post(&(wire::api(server) + "/enroll"), &body)?;
    if status == 201 && o.is_some() {
        let id = str_of(&o, "id").ok_or_else(|| fail(code::ANSWER, "Your server's answer did not read"))?;
        let get = |k: &str| o.as_ref().and_then(|o| o.get(k).cloned());
        return Ok(Enrollment {
            server: wire::normalize(server),
            id,
            verifier,
            code: str_of(&o, "code").unwrap_or_else(|| wire::code_of(&challenge)),
            // The server's `expiresAt` is on its clock, which this phone's may be minutes off:
            // a phone running fast would read a fresh request as long over. The deadline kept is
            // on this phone's clock instead, the server's window from now; the server still says
            // when the request ran out (`claim`'s Expired), which ends the wait first if sooner.
            expires_at: now_ms + ENROLL_TTL_MS,
            notified: get("notified").and_then(|v| v.as_bool()).unwrap_or(false),
        });
    }
    Err(match str_of(&o, "error").as_deref() {
        Some("cloud_off") => fail(code::CLOUD_OFF, "Relay is not running on your server yet"),
        Some("off") => fail(code::CLOUD_OFF, "Your server has Tally sync turned off"),
        Some("busy") => fail(code::DENIED, "Your server has requests waiting already. Answer those, then try again."),
        Some("slow_down") => fail(code::DENIED, "Asked a moment ago. Try again in half a minute."),
        _ => fail(code::DENIED, format!("Your server could not take the request ({status})")),
    })
}

pub fn claim(http: &dyn Http, e: &Enrollment) -> Claim {
    if let Err(f) = reachable(&wire::api(&e.server)) {
        return Claim::Trouble(f.message);
    }
    let body = json!({"id": e.id, "verifier": e.verifier});
    let (status, o) = match http.post(&(wire::api(&e.server) + "/claim"), &body) {
        Ok(r) => r,
        Err(f) => return Claim::Trouble(f.message),
    };
    if o.is_none() {
        return Claim::Trouble(format!("Your server's answer did not read ({status})"));
    }
    match str_of(&o, "status").as_deref() {
        Some("waiting") => Claim::Waiting,
        Some("denied") => Claim::Denied,
        Some("expired") => Claim::Expired,
        Some("spent") => Claim::Spent,
        Some("ready") => match str_of(&o, "link").and_then(|l| PairLink::parse(&l)) {
            Some(link) => Claim::Ready(link),
            None => Claim::Trouble("Your server sent a link that does not read".into()),
        },
        _ => Claim::Trouble(str_of(&o, "reason").unwrap_or_else(|| "Your server could not open a pairing window".into())),
    }
}

// The real network

enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

/// TLS as a browser does it: Mozilla's roots, ring's crypto.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("ring supports the default TLS versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// The scheme, host, port of a `ws(s)://` or `http(s)://` address.
fn endpoint(url: &str) -> Option<(bool, String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let tls = match scheme.to_ascii_lowercase().as_str() {
        "wss" | "https" => true,
        "ws" | "http" => false,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (host, port) = if let Some(h) = authority.strip_prefix('[') {
        let (h, after) = h.split_once(']')?;
        (h.to_string(), after.strip_prefix(':').and_then(|p| p.parse().ok()))
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p.parse().ok()?)),
            None => (authority.to_string(), None),
        }
    };
    if host.is_empty() {
        return None;
    }
    Some((tls, host, port.unwrap_or(if tls { 443 } else { 80 })))
}

/// WebSockets over a TCP connection of our own (TLS by rustls), so the timeouts are ours.
pub struct WsDial;

struct Ws {
    ws: tungstenite::WebSocket<Stream>,
    queue: VecDeque<String>,
}

impl Dial for WsDial {
    fn open(&self, route: &str) -> Result<Box<dyn Lines>, Failure> {
        let bad = || fail(code::UNREACHABLE, "That address does not read as a server's");
        reachable(route)?;
        let (tls, host, port) = endpoint(route).ok_or_else(bad)?;
        let addrs: Vec<_> = (host.as_str(), port).to_socket_addrs().map_err(|_| unreachable_())?.collect();
        let tcp = addrs.iter().find_map(|a| TcpStream::connect_timeout(a, CONNECT_TIMEOUT).ok()).ok_or_else(unreachable_)?;
        let _ = tcp.set_nodelay(true);
        tcp.set_read_timeout(Some(GREETING_TIMEOUT)).map_err(|_| unreachable_())?;
        tcp.set_write_timeout(Some(REQUEST_TIMEOUT)).map_err(|_| unreachable_())?;
        let stream = if tls {
            let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| bad())?;
            let conn = rustls::ClientConnection::new(tls_config(), name).map_err(|_| unreachable_())?;
            Stream::Tls(Box::new(rustls::StreamOwned::new(conn, tcp)))
        } else {
            Stream::Plain(tcp)
        };
        let (ws, _) = tungstenite::client::client(route, stream).map_err(|_| unreachable_())?;
        Ok(Box::new(Ws { ws, queue: VecDeque::new() }))
    }
}

impl Lines for Ws {
    fn send(&mut self, line: &str) -> Result<(), Failure> {
        self.ws.send(tungstenite::Message::text(line)).map_err(|_| closed())
    }

    fn next(&mut self, wait: Duration) -> Result<String, Failure> {
        let until = Instant::now() + wait;
        loop {
            if let Some(l) = self.queue.pop_front() {
                return Ok(l);
            }
            let left = until.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()).ok_or_else(too_slow)?;
            let _ = self.ws.get_ref().tcp().set_read_timeout(Some(left));
            match self.ws.read() {
                // One line per message, but a frame holding several is several lines.
                Ok(tungstenite::Message::Text(t)) => self.queue.extend(t.as_str().split('\n').filter(|l| !l.trim().is_empty()).map(str::to_string)),
                Ok(tungstenite::Message::Close(_)) => return Err(closed()),
                // Pings are answered by tungstenite; nothing else carries lines.
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return Err(too_slow()),
                Err(_) => return Err(closed()),
            }
        }
    }
}

impl Drop for Ws {
    fn drop(&mut self) {
        let _ = self.ws.close(None);
        let _ = self.ws.flush();
    }
}

/// HTTPS through ureq (rustls and ring, Mozilla's roots). Plain HTTP only to an address
/// [`wire::may_reach`] allows.
pub struct UreqHttp {
    /// For https: never follows a redirect to http.
    secure: ureq::Agent,
    /// For http on this phone's own network: follows no redirect at all.
    local: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> UreqHttp {
        let agent = |https_only: bool| -> ureq::Agent {
            ureq::Agent::config_builder()
                .timeout_connect(Some(Duration::from_secs(15)))
                // Render wakes a sleeping service on the first request; give it the time it takes.
                .timeout_global(Some(Duration::from_secs(75)))
                .http_status_as_error(false)
                .https_only(https_only)
                .max_redirects(if https_only { 10 } else { 0 })
                .build()
                .into()
        };
        UreqHttp { secure: agent(true), local: agent(false) }
    }

    fn agent(&self, url: &str) -> Result<&ureq::Agent, Failure> {
        reachable(url)?;
        Ok(if wire::is_secure(url) { &self.secure } else { &self.local })
    }

    fn read(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<(u16, Option<Value>), Failure> {
        let mut r = r.map_err(|e| match e {
            ureq::Error::BadUri(_) | ureq::Error::Http(_) => fail(code::UNREACHABLE, "That address does not read as a server's"),
            _ => unreachable_(),
        })?;
        let status = r.status().as_u16();
        let text = r.body_mut().read_to_string().unwrap_or_default();
        Ok((status, serde_json::from_str::<Value>(&text).ok().filter(Value::is_object)))
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        UreqHttp::new()
    }
}

impl Http for UreqHttp {
    fn get(&self, url: &str) -> Result<(u16, Option<Value>), Failure> {
        UreqHttp::read(self.agent(url)?.get(url).call())
    }
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Option<Value>), Failure> {
        UreqHttp::read(self.agent(url)?.post(url).header("content-type", "application/json").send(body.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_splits_into_where_to_connect() {
        assert_eq!(endpoint("wss://s.example/join/r1"), Some((true, "s.example".into(), 443)));
        assert_eq!(endpoint("ws://10.0.0.2:7420"), Some((false, "10.0.0.2".into(), 7420)));
        assert_eq!(endpoint("ws://[::1]:9/x"), Some((false, "::1".into(), 9)));
        assert_eq!(endpoint("ftp://x"), None);
        assert_eq!(endpoint("wss://"), None);
    }

    /// Records every route it is asked to open, and opens none.
    struct Routes(std::sync::Mutex<Vec<String>>);

    impl Dial for Routes {
        fn open(&self, route: &str) -> Result<Box<dyn Lines>, Failure> {
            self.0.lock().unwrap().push(route.to_string());
            Err(unreachable_())
        }
    }

    struct NoHttp;

    impl Http for NoHttp {
        fn get(&self, url: &str) -> Result<(u16, Option<Value>), Failure> {
            panic!("asked {url}")
        }
        fn post(&self, url: &str, _body: &Value) -> Result<(u16, Option<Value>), Failure> {
            panic!("asked {url}")
        }
    }

    #[test]
    fn encrypted_routes_go_first_and_public_clear_ones_never() {
        let dial = Routes(Default::default());
        let c = Creds { host: "h".into(), routes: ["ws://8.8.8.8:7420", "ws://192.168.1.20:7420", "wss://s.example/join/r"].map(String::from).to_vec(), device: "d".into(), token: "t".into(), paired_at: 0 };
        assert!(open(&dial, &c).is_err());
        assert_eq!(*dial.0.lock().unwrap(), vec!["wss://s.example/join/r".to_string()], "no clear fallback beside an encrypted route");
        let only_public = Creds { routes: vec!["ws://8.8.8.8:7420".into()], ..c };
        assert_eq!(open(&dial, &only_public).err().map(|f| f.code), Some(code::INSECURE.to_string()));
        // A server typed as http:// on the internet is never asked anything.
        assert_eq!(hello(&NoHttp, "http://anthobot.bid").err().map(|f| f.code), Some(code::INSECURE.to_string()));
        assert_eq!(enroll(&NoHttp, "http://anthobot.bid", "d", 0).err().map(|f| f.code), Some(code::INSECURE.to_string()));
        let e = Enrollment { server: "http://anthobot.bid".into(), id: "i".into(), verifier: "v".into(), code: "AB".into(), expires_at: 0, notified: true };
        assert!(matches!(claim(&NoHttp, &e), Claim::Trouble(_)));
        assert!(WsDial.open("ws://8.8.8.8:7420").is_err());
        assert!(UreqHttp::new().get("http://anthobot.bid/api/tally").is_err());
    }

    /// A door that greets and refuses every proof with `error`.
    struct Refuses(&'static str, std::sync::Mutex<Vec<String>>);

    struct RefusingLines(VecDeque<String>, &'static str);

    impl Lines for RefusingLines {
        fn send(&mut self, _line: &str) -> Result<(), Failure> {
            self.0.push_back(json!({"v": 1, "ok": false, "error": self.1}).to_string());
            Ok(())
        }
        fn next(&mut self, _wait: Duration) -> Result<String, Failure> {
            self.0.pop_front().ok_or_else(closed)
        }
    }

    impl Dial for Refuses {
        fn open(&self, route: &str) -> Result<Box<dyn Lines>, Failure> {
            self.1.lock().unwrap().push(route.to_string());
            let greet = json!({"v": 1, "relay": "remote", "host": "", "host_id": "", "instance": "cloud", "challenge": "c"}).to_string();
            Ok(Box::new(RefusingLines([greet].into(), self.0)))
        }
    }

    #[test]
    fn a_clear_door_saying_it_forgot_this_phone_is_not_believed() {
        let c = |routes: &[&str]| Creds { host: "h".into(), routes: routes.iter().map(|r| r.to_string()).collect(), device: "d".into(), token: "t".into(), paired_at: 0 };
        let door = Refuses("auth.unknown_device", Default::default());
        // A gateway answering on the LAN address of a phone paired over wss is never asked.
        assert_eq!(open(&door, &c(&["ws://192.168.1.20:7420", "wss://s.example/join/r"])).err().map(|f| f.code), Some(code::UNPAIRED.to_string()));
        assert_eq!(*door.1.lock().unwrap(), vec!["wss://s.example/join/r".to_string()]);
        // A door with only clear addresses: its refusal is no proof the server forgot this phone.
        assert_eq!(open(&door, &c(&["ws://192.168.1.20:7420"])).err().map(|f| f.code), Some(code::UNREACHABLE.to_string()));
        // This machine's own door is believed.
        assert_eq!(open(&door, &c(&["ws://127.0.0.1:7430"])).err().map(|f| f.code), Some(code::UNPAIRED.to_string()));
    }

    /// A server whose clock is `behind_ms` behind this phone's.
    struct SlowClock(i64);

    impl Http for SlowClock {
        fn get(&self, url: &str) -> Result<(u16, Option<Value>), Failure> {
            panic!("asked {url}")
        }
        fn post(&self, _url: &str, _body: &Value) -> Result<(u16, Option<Value>), Failure> {
            let server_now = 1_000_000_000 - self.0;
            Ok((201, Some(json!({"id": "e1", "code": "AB12", "expiresAt": server_now + ENROLL_TTL_MS, "notified": true}))))
        }
    }

    #[test]
    fn a_request_lasts_its_window_on_this_phones_clock() {
        // This phone runs 20 minutes fast: the server's own deadline is already past here.
        let e = enroll(&SlowClock(20 * 60_000), "https://anthobot.bid", "d", 1_000_000_000).unwrap();
        assert_eq!(e.expires_at, 1_000_000_000 + ENROLL_TTL_MS);
        let e = enroll(&SlowClock(-20 * 60_000), "https://anthobot.bid", "d", 1_000_000_000).unwrap();
        assert_eq!(e.expires_at, 1_000_000_000 + ENROLL_TTL_MS);
    }

    #[test]
    fn the_key_never_prints() {
        let c = Creds { host: "Cloud".into(), routes: vec![], device: "d".into(), token: "secret-token".into(), paired_at: 0 };
        assert!(!format!("{c:?}").contains("secret-token"));
        let e = Enrollment { server: "s".into(), id: "i".into(), verifier: "secret-verifier".into(), code: "AB".into(), expires_at: 0, notified: true };
        assert!(!format!("{e:?}").contains("secret-verifier"));
    }

    #[test]
    fn tests_cannot_reach_any_server() {
        let net = Net::real();
        assert!(net.dial.open("wss://example.com").is_err());
        assert!(net.http.get("https://example.com").is_err());
    }
}
