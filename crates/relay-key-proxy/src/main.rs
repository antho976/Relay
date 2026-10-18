//! `relay-key-proxy` — holds the Anthropic API key for Relay's API-key sessions (docs/KEY-PROXY.md).
//!
//! It runs as its own throwaway system user (`relay-key-proxy.service`, `DynamicUser=yes`), and
//! systemd hands it the key from a credential encrypted with the host's root-only key. Nothing that
//! runs as the person — an agent, a repository's hooks, a line injected over the phone link — can
//! read another user's files or memory, so none of them can learn the key.
//!
//! A session's Claude talks to it on loopback with a local pass token in `x-api-key`. One request
//! per connection: a request that carries exactly that token, to one of the few endpoints Claude
//! Code uses, with a body of a stated length, is sent on to `api.anthropic.com` alone, over TLS
//! checked against roots built into this binary, with only an allowlisted set of headers and the
//! token swapped for the key. Anything else is answered here and goes nowhere. The token lets a
//! program running as the person spend through the proxy; it never lets one see the key.
//!
//! Deliberately small, with six direct dependencies, so root can build it from a clean clone and
//! read what it runs (scripts/key-proxy/install.sh).

use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Where Relay sends an API-key session (`relay_core::hooks::KEY_PROXY_PORT`).
const PORT: u16 = 7445;
/// The systemd credential names (`LoadCredentialEncrypted=` in the unit).
const KEY_CREDENTIAL: &str = "relay-anthropic-key";
const TOKEN_CREDENTIAL: &str = "relay-proxy-token";
/// Where every forwarded request goes. Fixed: nothing a caller sends can name another host.
const UPSTREAM: &str = "api.anthropic.com";
const MAX_HEAD: usize = 64 * 1024;
const MAX_HEADERS: usize = 64;
/// Anthropic's responses carry more headers than Claude Code's requests.
const MAX_RESPONSE_HEADERS: usize = 256;
/// A long conversation with pictures is a few megabytes; this is far above that.
const MAX_BODY: u64 = 64 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 256;
/// Connections over the limit being told so; beyond these, closed unanswered.
const MAX_REFUSALS: usize = 16;
const HEAD_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Longest silence in either direction once a request is under way. Anthropic's streams send a
/// ping well within it while a model thinks.
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);

type Refusal = (u16, &'static str);

struct Secrets {
    key: String,
    token: String,
}

/// Where requests go: Anthropic, or in a test a plain listener standing in for it. The test arm
/// is compiled only into tests, so no build of this binary can be pointed anywhere else.
enum Upstream {
    Anthropic(tokio_rustls::TlsConnector),
    #[cfg(test)]
    Plain(SocketAddr),
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

impl Upstream {
    fn anthropic() -> Result<Upstream, String> {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Upstream::Anthropic(tokio_rustls::TlsConnector::from(Arc::new(config))))
    }

    async fn connect(&self) -> std::io::Result<Box<dyn Stream>> {
        let timed_out = || std::io::Error::new(std::io::ErrorKind::TimedOut, "connecting to api.anthropic.com timed out");
        match self {
            Upstream::Anthropic(tls) => {
                let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((UPSTREAM, 443))).await.map_err(|_| timed_out())??;
                tcp.set_nodelay(true)?;
                let name = rustls::pki_types::ServerName::try_from(UPSTREAM).expect("a valid DNS name");
                Ok(Box::new(tokio::time::timeout(CONNECT_TIMEOUT, tls.connect(name, tcp)).await.map_err(|_| timed_out())??))
            }
            #[cfg(test)]
            Upstream::Plain(addr) => Ok(Box::new(TcpStream::connect(addr).await?)),
        }
    }
}

/// Equal strings, in time that depends only on their lengths.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// An ordinary API key. An admin key (`sk-ant-admin…`) manages the organisation, its members and
/// its keys, and is refused: the pass token must never be worth more than API calls.
fn key_shaped(key: &str) -> bool {
    key.len() > 30
        && key.starts_with("sk-ant-api")
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Read one systemd credential, trimmed. An error never carries the content.
fn credential(dir: &Path, name: &str) -> Result<String, String> {
    std::fs::read_to_string(dir.join(name)).map(|raw| raw.trim().to_string()).map_err(|e| format!("reading credential {name}: {e}"))
}

fn main() -> ExitCode {
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("relay-key-proxy: {error}");
            ExitCode::FAILURE
        }
    }
}

fn start() -> Result<(), String> {
    // No core dump and no ptrace by another process of this user id, whatever the unit says.
    // SAFETY: prctl with PR_SET_DUMPABLE takes plain integers and touches no memory of ours.
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(format!("prctl(PR_SET_DUMPABLE): {}", std::io::Error::last_os_error()));
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let listen = match args.as_slice() {
        [] => format!("127.0.0.1:{PORT}"),
        [flag, addr] if flag == "--listen" => addr.clone(),
        _ => return Err("usage: relay-key-proxy [--listen 127.0.0.1:PORT]".into()),
    };
    let addr: SocketAddr = listen.parse().map_err(|_| format!("--listen {listen:?} is not an address"))?;
    if !addr.ip().is_loopback() {
        return Err(format!("--listen {addr} is not a loopback address: the proxy answers this machine only"));
    }
    let dir = std::env::var_os("CREDENTIALS_DIRECTORY")
        .ok_or("no credentials: run this as relay-key-proxy.service (scripts/key-proxy/install.sh)")?;
    let key = credential(Path::new(&dir), KEY_CREDENTIAL)?;
    if !key_shaped(&key) {
        return Err(format!("credential {KEY_CREDENTIAL} is not an Anthropic API key (sk-ant-api…); admin keys are refused"));
    }
    let token = credential(Path::new(&dir), TOKEN_CREDENTIAL)?;
    if token.len() < 32 {
        return Err(format!("credential {TOKEN_CREDENTIAL} is shorter than 32 characters"));
    }
    let upstream = Upstream::anthropic()?;
    // Under relay-key-proxy.socket, PID 1 holds the port and hands it over, so no other program
    // can take it while this service is stopped or restarting.
    let inherited = inherited_listener().map_err(|e| format!("the socket systemd passed: {e}"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let listener = match inherited {
            Some(listener) => TcpListener::from_std(listener).map_err(|e| e.to_string())?,
            None => TcpListener::bind(addr).await.map_err(|e| format!("listening on {addr}: {e}"))?,
        };
        let local = listener.local_addr().map_err(|e| e.to_string())?;
        if !local.ip().is_loopback() {
            return Err(format!("{local} is not a loopback address: the proxy answers this machine only"));
        }
        eprintln!("relay-key-proxy: listening on {local}, forwarding to https://{UPSTREAM}");
        serve(listener, Arc::new(Secrets { key, token }), Arc::new(upstream), Arc::new(host_names(local.port()))).await;
        Ok(())
    })
}

/// The listening socket systemd passed (`sd_listen_fds`: fd 3, named by `LISTEN_PID` and
/// `LISTEN_FDS`), if it passed one to this process.
fn inherited_listener() -> std::io::Result<Option<std::net::TcpListener>> {
    let ours = std::env::var("LISTEN_PID").ok().and_then(|pid| pid.parse::<u32>().ok()) == Some(std::process::id());
    if !ours || std::env::var("LISTEN_FDS").ok().as_deref() != Some("1") {
        return Ok(None);
    }
    use std::os::fd::FromRawFd;
    // SAFETY: systemd passed this process exactly one socket, as fd 3, and nothing else owns it.
    let listener = unsafe { std::net::TcpListener::from_raw_fd(3) };
    listener.set_nonblocking(true)?;
    Ok(Some(listener))
}

/// The `Host` a request to this proxy carries: anything else reached it by another name, as a
/// web page using DNS rebinding would.
fn host_names(port: u16) -> Vec<String> {
    vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")]
}

async fn serve(listener: TcpListener, secrets: Arc<Secrets>, upstream: Arc<Upstream>, hosts: Arc<Vec<String>>) {
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    let refusals = Arc::new(tokio::sync::Semaphore::new(MAX_REFUSALS));
    loop {
        let client = match listener.accept().await {
            Ok((client, _)) => client,
            Err(_) => {
                // Out of descriptors, most likely: give the open connections time to end.
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        // Over the limit a few are told so; the rest are closed unanswered, as answering each
        // would cost a task.
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            if let Ok(refusal) = refusals.clone().try_acquire_owned() {
                let mut client = client;
                tokio::spawn(async move {
                    let _refusal = refusal;
                    let _ = answer(&mut client, 503, "the key proxy is busy; try again").await;
                });
            }
            continue;
        };
        let (secrets, upstream, hosts) = (secrets.clone(), upstream.clone(), hosts.clone());
        tokio::spawn(async move {
            let _slot = slot;
            if let Err(error) = handle(client, &secrets, &upstream, &hosts).await {
                // Never the request: its headers carried the token.
                eprintln!("relay-key-proxy: {error}");
            }
        });
    }
}

/// An answer from the proxy itself, in the shape Anthropic's API errors take so Claude shows it.
async fn answer(client: &mut TcpStream, status: u16, message: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        411 => "Length Required",
        413 => "Payload Too Large",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        _ => "Service Unavailable",
    };
    let body = if status == 200 {
        String::new()
    } else {
        format!(r#"{{"type":"error","error":{{"type":"relay_key_proxy","message":"{message}"}}}}"#)
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    client.write_all(response.as_bytes()).await?;
    let _ = client.shutdown().await;
    // Closed with the caller's bytes unread, the socket resets and the answer can be lost: take
    // what is still coming for a moment first, up to a limit.
    let mut sink = [0u8; 8192];
    let drain = async {
        let mut left = 1usize << 20;
        while left > 0 {
            match client.read(&mut sink).await {
                Ok(n) if n > 0 => left = left.saturating_sub(n),
                _ => break,
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(1), drain).await;
    Ok(())
}

/// Read the request head. Returns its length as the parser counted it, so a head that ends its
/// lines with a bare LF is measured the same way it is read; bytes after it are the body's start.
async fn read_head(client: &mut TcpStream, buf: &mut Vec<u8>) -> Result<usize, Refusal> {
    let mut chunk = [0u8; 8192];
    loop {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        match httparse::Request::new(&mut headers).parse(buf) {
            Ok(httparse::Status::Complete(len)) if len <= MAX_HEAD => return Ok(len),
            Ok(httparse::Status::Complete(_)) => return Err((431, "request head too large")),
            Ok(httparse::Status::Partial) => {}
            Err(httparse::Error::TooManyHeaders) => return Err((431, "too many headers")),
            Err(_) => return Err((400, "malformed request")),
        }
        if buf.len() > MAX_HEAD {
            return Err((431, "request head too large"));
        }
        match client.read(&mut chunk).await {
            Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
            _ => return Err((400, "connection closed before the request head ended")),
        }
    }
}

/// The endpoints Claude Code calls with an API key: the conversation, its token count and the
/// model list. Every other path, Anthropic's admin API among them, is answered here.
fn allowed(method: &str, path: &str) -> bool {
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let plain = |s: &str, extra: &[u8]| s.bytes().all(|b| b.is_ascii_alphanumeric() || extra.contains(&b));
    if !plain(query, b"=&_.-") {
        return false;
    }
    match method {
        "POST" => route == "/v1/messages" || route == "/v1/messages/count_tokens",
        "GET" => {
            route == "/v1/models"
                || route.strip_prefix("/v1/models/").is_some_and(|id| !id.is_empty() && plain(id, b"_.-") && !id.contains(".."))
        }
        _ => false,
    }
}

/// Headers sent on: what Claude Code's requests carry, by name. The caller's credentials, hop by
/// hop headers and anything unknown stay here.
fn forwarded(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["accept", "accept-encoding", "accept-language", "content-type", "user-agent", "x-app"].contains(&name.as_str())
        || ["anthropic-", "x-stainless-", "x-claude-code-"].iter().any(|prefix| name.starts_with(prefix))
}

/// What became of a request head: answered here, or sent on with a body of this length.
#[derive(Debug, PartialEq)]
enum Verdict {
    Answer(u16, &'static str),
    Forward { head: Vec<u8>, body: u64 },
}

fn judge(head: &[u8], secrets: &Secrets, hosts: &[String]) -> Verdict {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    if !matches!(request.parse(head), Ok(httparse::Status::Complete(_))) {
        return Verdict::Answer(400, "malformed request");
    }
    let (Some(method), Some(path)) = (request.method, request.path) else { return Verdict::Answer(400, "malformed request") };
    let named = |name: &str| request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value).collect::<Vec<_>>();
    let host = named("host");
    if !(host.len() == 1 && hosts.iter().any(|ok| same(host[0], ok.as_bytes()))) {
        return Verdict::Answer(421, "the key proxy answers only as 127.0.0.1");
    }
    // Claude Code's check that the service is up, before it has a key to send.
    if matches!(method, "HEAD" | "GET") && path == "/api/hello" {
        return Verdict::Answer(200, "");
    }
    let token = named("x-api-key");
    if !(token.len() == 1 && same(token[0], secrets.token.as_bytes())) {
        return Verdict::Answer(401, "not authorized by the Relay key proxy");
    }
    if !allowed(method, path) {
        return Verdict::Answer(404, "the key proxy forwards only the endpoints Claude Code uses");
    }
    // One framing: a stated length. A chunked body or two lengths is how a request is smuggled
    // past a proxy, and Claude Code sends neither.
    if !named("transfer-encoding").is_empty() {
        return Verdict::Answer(411, "send a content-length, not a chunked body");
    }
    let lengths: Vec<Option<u64>> = named("content-length").iter().map(|v| std::str::from_utf8(v).ok().and_then(|v| v.parse().ok())).collect();
    let body = match lengths.as_slice() {
        [] => 0,
        [Some(len)] if *len <= MAX_BODY => *len,
        [Some(_)] => return Verdict::Answer(413, "request body too large"),
        _ => return Verdict::Answer(400, "unreadable content-length"),
    };
    let mut out = Vec::with_capacity(head.len() + secrets.key.len() + 128);
    out.extend_from_slice(format!("{method} {path} HTTP/1.1\r\nhost: {UPSTREAM}\r\nx-api-key: ").as_bytes());
    out.extend_from_slice(secrets.key.as_bytes());
    out.extend_from_slice(format!("\r\ncontent-length: {body}\r\nconnection: close\r\n").as_bytes());
    for header in request.headers.iter().filter(|h| forwarded(h.name)) {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    Verdict::Forward { head: out, body }
}

/// Copy until `from` ends, failing on a silence longer than [`IDLE_TIMEOUT`]; exactly `limit`
/// bytes when one is given.
async fn pump<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(from: &mut R, to: &mut W, limit: Option<u64>) -> std::io::Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    let mut left = limit;
    while left != Some(0) {
        let want = left.map_or(buf.len(), |left| buf.len().min(left as usize));
        let n = tokio::time::timeout(IDLE_TIMEOUT, from.read(&mut buf[..want]))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "idle"))??;
        if n == 0 {
            if left.is_some() {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the request body ended early"));
            }
            break;
        }
        to.write_all(&buf[..n]).await?;
        left = left.map(|left| left - n as u64);
    }
    to.flush().await
}

async fn handle(mut client: TcpStream, secrets: &Secrets, upstream: &Upstream, hosts: &[String]) -> std::io::Result<()> {
    client.set_nodelay(true)?;
    let mut buf = Vec::with_capacity(8192);
    let head_len = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut client, &mut buf)).await {
        Ok(Ok(len)) => len,
        Ok(Err((status, message))) => return answer(&mut client, status, message).await,
        Err(_) => return answer(&mut client, 400, "request head timed out").await,
    };
    let (head, body) = match judge(&buf[..head_len], secrets, hosts) {
        Verdict::Forward { head, body } => (head, body),
        Verdict::Answer(status, message) => return answer(&mut client, status, message).await,
    };
    let mut server = match upstream.connect().await {
        Ok(server) => server,
        Err(error) => {
            answer(&mut client, 502, "the key proxy could not reach api.anthropic.com").await?;
            return Err(error);
        }
    };
    server.write_all(&head).await?;
    drop(head);
    // The body is exactly its stated length; whatever follows it on this connection (a second,
    // pipelined request) is never read, let alone sent on.
    let early = &buf[head_len..];
    let early = &early[..early.len().min(body as usize)];
    server.write_all(early).await?;
    pump(&mut client, &mut server, Some(body - early.len() as u64)).await?;
    // One request per connection (`connection: close` upstream): the response ends when Anthropic
    // closes, and the caller's connection with it. Its head says so whatever Anthropic's did, so
    // the caller never pools a connection this proxy is about to close.
    let mut response = Vec::with_capacity(8192);
    let head_len = read_response_head(&mut server, &mut response).await?;
    client.write_all(&closing(&response[..head_len])?).await?;
    client.write_all(&response[head_len..]).await?;
    pump(&mut server, &mut client, None).await?;
    let _ = client.shutdown().await;
    Ok(())
}

async fn read_response_head<R: AsyncRead + Unpin>(server: &mut R, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    let bad = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Anthropic's response: {why}"));
    let mut chunk = [0u8; 8192];
    loop {
        let mut headers = [httparse::EMPTY_HEADER; MAX_RESPONSE_HEADERS];
        match httparse::Response::new(&mut headers).parse(buf) {
            Ok(httparse::Status::Complete(len)) if len <= MAX_HEAD => return Ok(len),
            Ok(httparse::Status::Partial) if buf.len() <= MAX_HEAD => {}
            Ok(_) => return Err(bad("head too large")),
            Err(_) => return Err(bad("malformed head")),
        }
        let n = tokio::time::timeout(IDLE_TIMEOUT, server.read(&mut chunk)).await.map_err(|_| bad("timed out"))??;
        if n == 0 {
            return Err(bad("closed before its head ended"));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// A response head, with `connection: close` in place of whatever it said about the connection.
fn closing(head: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_RESPONSE_HEADERS];
    let mut response = httparse::Response::new(&mut headers);
    response.parse(head).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let code = response.code.unwrap_or(502);
    let mut out = format!("HTTP/1.1 {code} {}\r\n", response.reason.unwrap_or("")).into_bytes();
    for header in response.headers.iter().filter(|h| !["connection", "keep-alive"].iter().any(|d| h.name.eq_ignore_ascii_case(d))) {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"connection: close\r\n\r\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-ant-api03-REALKEY_never-shown-0123456789";
    const TOKEN: &str = "local-pass-token-0123456789abcdefghij";

    fn secrets() -> Secrets {
        Secrets { key: KEY.into(), token: TOKEN.into() }
    }

    /// A stand-in for Anthropic: records everything it receives on each connection until it has
    /// the whole stated body (and anything sent after it), then answers with a streamed body.
    async fn fake_upstream() -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 65536];
                    loop {
                        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                            let len: usize = head.lines().find_map(|l| l.strip_prefix("content-length: ")).and_then(|v| v.parse().ok()).unwrap_or(0);
                            if buf.len() >= end + 4 + len {
                                break;
                            }
                        }
                        match tokio::time::timeout(Duration::from_millis(500), socket.read(&mut chunk)).await {
                            Ok(Ok(n)) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                            _ => break,
                        }
                    }
                    // Anything more the proxy sends would show here.
                    if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_millis(100), socket.read(&mut chunk)).await {
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    tx.send(buf).unwrap();
                    socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: keep-alive\r\nkeep-alive: timeout=5\r\n\r\n").await.unwrap();
                    for part in ["event: a\n\n", "event: b\n\n"] {
                        socket.write_all(part.as_bytes()).await.unwrap();
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                });
            }
        });
        (addr, rx)
    }

    async fn proxy(upstream: SocketAddr) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hosts = Arc::new(host_names(addr.port()));
        tokio::spawn(serve(listener, Arc::new(secrets()), Arc::new(Upstream::Plain(upstream)), hosts));
        addr
    }

    async fn send(proxy: SocketAddr, request: &[u8]) -> String {
        let mut socket = TcpStream::connect(proxy).await.unwrap();
        // The proxy may answer and close before a large request is all written.
        let _ = socket.write_all(request).await;
        let mut out = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut out)).await.unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    fn request(port: u16, extra: &str, body: &str) -> String {
        format!(
            "POST /v1/messages?beta=true HTTP/1.1\r\nhost: 127.0.0.1:{port}\r\n{extra}content-type: application/json\r\n\
             content-length: {}\r\nconnection: keep-alive\r\nanthropic-version: 2023-06-01\r\nx-stainless-os: Linux\r\n\
             x-claude-code-session-id: s1\r\ncookie: c=1\r\nx-forwarded-for: 1.2.3.4\r\nx_api_key: nope\r\n\r\n{body}",
            body.len()
        )
    }

    /// The token is swapped for the key on the way to Anthropic; only allowlisted headers go
    /// with it; the body goes up and the streamed answer comes back whole; the caller never sees
    /// the key.
    #[tokio::test]
    async fn an_authorized_request_reaches_anthropic_with_the_key_and_only_known_headers() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let auth = format!("x-api-key: {TOKEN}\r\nauthorization: Bearer {TOKEN}\r\n");
        let answer = send(proxy, request(proxy.port(), &auth, r#"{"hello":"x"}"#).as_bytes()).await;
        assert!(answer.starts_with("HTTP/1.1 200 OK") && answer.ends_with("event: a\n\nevent: b\n\n"), "{answer:?}");
        assert!(!answer.contains(KEY));
        // Whatever Anthropic said of the connection, the caller is told it closes.
        let (response_head, _) = answer.split_once("\r\n\r\n").unwrap();
        assert_eq!(response_head, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close");
        let sent = String::from_utf8(seen.recv().await.unwrap()).unwrap();
        let (head, body) = sent.split_once("\r\n\r\n").unwrap();
        assert_eq!(body, r#"{"hello":"x"}"#);
        let lines: Vec<&str> = head.lines().collect();
        assert_eq!(lines, [
            "POST /v1/messages?beta=true HTTP/1.1", "host: api.anthropic.com", &format!("x-api-key: {KEY}"),
            "content-length: 13", "connection: close", "content-type: application/json",
            "anthropic-version: 2023-06-01", "x-stainless-os: Linux", "x-claude-code-session-id: s1",
        ]);
        assert!(!sent.contains(TOKEN));
    }

    /// A second request pipelined on the same connection, or bytes past the stated body, are never
    /// sent on: only the first request's exact body follows its head.
    #[tokio::test]
    async fn nothing_after_the_stated_body_is_forwarded() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let auth = format!("x-api-key: {TOKEN}\r\n");
        let second = format!("GET /v1/organizations/users HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n{auth}\r\n", proxy.port());
        let both = format!("{}{second}", request(proxy.port(), &auth, "{}"));
        let answer = send(proxy, both.as_bytes()).await;
        assert!(answer.starts_with("HTTP/1.1 200 OK"), "{answer:?}");
        let sent = String::from_utf8(seen.recv().await.unwrap()).unwrap();
        assert!(sent.ends_with("\r\n\r\n{}"), "{sent:?}");
        assert!(!sent.contains("organizations"), "{sent:?}");
        assert!(seen.try_recv().is_err(), "a second request reached the upstream");
    }

    /// A body bigger than one read is sent whole and exactly.
    #[tokio::test]
    async fn a_large_body_is_forwarded_exactly() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let body = "x".repeat(3 * 1024 * 1024 + 7);
        let answer = send(proxy, request(proxy.port(), &format!("x-api-key: {TOKEN}\r\n"), &body).as_bytes()).await;
        assert!(answer.starts_with("HTTP/1.1 200 OK"), "{}", &answer[..answer.len().min(200)]);
        let sent = seen.recv().await.unwrap();
        assert!(sent.ends_with(body.as_bytes()) && sent.len() < body.len() + 1024);
    }

    /// Without the token, with a wrong one or two of them, to another endpoint or by another
    /// name, with framing two parsers could read apart, or malformed: answered here, and nothing
    /// is forwarded.
    #[tokio::test]
    async fn anything_else_is_answered_here_and_never_forwarded() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let port = proxy.port();
        let auth = format!("x-api-key: {TOKEN}\r\n");
        let get = |path: &str, host: &str, extra: &str| format!("GET {path} HTTP/1.1\r\nhost: {host}\r\n{extra}\r\n");
        let here = format!("127.0.0.1:{port}");
        let cases: Vec<(String, &str)> = vec![
            (request(port, "", "{}"), "401"),
            (request(port, "x-api-key: wrong-token-0123456789abcdefghijkl\r\n", "{}"), "401"),
            (request(port, &format!("{auth}x-api-key: other\r\n"), "{}"), "401"),
            (request(port, &format!("{auth}transfer-encoding: chunked\r\n"), "{}"), "411"),
            (request(port, &format!("{auth}content-length: 2\r\n"), "{}"), "400"),
            (get("/v1/organizations/users", &here, &auth), "404"),
            (get("/v1/organizations/api_keys", &here, &auth), "404"),
            (get("/v1/models/../organizations", &here, &auth), "404"),
            (get("/v1/messages", &here, &auth), "404"),
            (format!("DELETE /v1/messages HTTP/1.1\r\nhost: {here}\r\n{auth}\r\n"), "404"),
            (get("/v1/models", "evil.example", &auth), "421"),
            (get("/v1/models", &format!("127.0.0.1:{}", port.wrapping_add(1)), &auth), "421"),
            (format!("GET /v1/models HTTP/1.1\r\nhost: {here}\r\nhost: evil.example\r\n{auth}\r\n"), "421"),
            (format!("GET http://evil.example/v1/models HTTP/1.1\r\nhost: {here}\r\n{auth}\r\n"), "404"),
            (format!("CONNECT evil.example:443 HTTP/1.1\r\nhost: {here}\r\n{auth}\r\n"), "404"),
            ("garbage\r\n\r\n".to_string(), "400"),
            (format!("GET /v1/models HTTP/1.1\r\nhost: {here}\r\n{auth}x-pad: {}\r\n\r\n", "a".repeat(MAX_HEAD + 10)), "431"),
        ];
        for (request, status) in &cases {
            let answer = send(proxy, request.as_bytes()).await;
            assert!(answer.starts_with(&format!("HTTP/1.1 {status}")), "{status} for {:?}: {answer:?}", &request[..request.len().min(80)]);
            assert!(!answer.contains(KEY));
        }
        assert!(seen.try_recv().is_err(), "a refused request reached the upstream");
        // Claude Code's liveness check is answered here, with no token.
        assert!(send(proxy, format!("HEAD /api/hello HTTP/1.1\r\nhost: {here}\r\n\r\n").as_bytes()).await.starts_with("HTTP/1.1 200"));
        assert!(seen.try_recv().is_err());
    }

    /// A head whose lines end in a bare LF is measured as it is parsed: what follows is body.
    #[test]
    fn a_bare_lf_head_is_judged_on_the_bytes_the_parser_read() {
        let head = format!("POST /v1/messages HTTP/1.1\nhost: 127.0.0.1:{PORT}\nx-api-key: {TOKEN}\ncontent-length: 2\n\n");
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let parsed = httparse::Request::new(&mut headers).parse(format!("{head}{{}}").as_bytes()).unwrap();
        assert_eq!(parsed, httparse::Status::Complete(head.len()));
        let Verdict::Forward { head: out, body } = judge(head.as_bytes(), &secrets(), &host_names(PORT)) else { panic!("refused") };
        assert_eq!(body, 2);
        assert!(String::from_utf8(out).unwrap().ends_with("connection: close\r\n\r\n"));
    }

    #[test]
    fn only_the_endpoints_claude_code_uses_are_allowed() {
        for (method, path) in [("POST", "/v1/messages"), ("POST", "/v1/messages?beta=true"), ("POST", "/v1/messages/count_tokens?beta=true"),
            ("GET", "/v1/models"), ("GET", "/v1/models/claude-opus-5-5")] {
            assert!(allowed(method, path), "{method} {path}");
        }
        for (method, path) in [("GET", "/v1/messages"), ("POST", "/v1/messages/batches"), ("GET", "/v1/organizations/me"),
            ("POST", "/v1/messages?x=%2f"), ("GET", "/v1/models/"), ("GET", "/v1/models/a/b"), ("GET", "/v1/models/.."),
            ("POST", "/v1/files"), ("PUT", "/v1/messages")] {
            assert!(!allowed(method, path), "{method} {path}");
        }
        assert!(forwarded("Anthropic-Beta") && forwarded("x-stainless-retry-count") && forwarded("User-Agent"));
        for name in ["authorization", "x-api-key", "x_api_key", "cookie", "x-forwarded-for", "host", "connection", "upgrade", "te", "content-length"] {
            assert!(!forwarded(name), "{name}");
        }
    }

    #[test]
    fn only_an_ordinary_api_key_is_taken() {
        assert!(key_shaped(KEY));
        for bad in ["", "sk-ant-", "sk-ant-admin01-abcdefghijklmnopqrstuvwxyz", "sk-ant-api03-a b c d e f g h i j k",
            "sk-proj-abcdefghijklmnopqrstuvwxyz0123", "sk-ant-api03-abcdefghijklmnop\nInjected: 1"] {
            assert!(!key_shaped(bad), "{bad:?}");
        }
        assert!(same(b"abc", b"abc") && !same(b"abc", b"abd") && !same(b"abc", b"ab"));
    }

    /// The units that run the proxy: PID 1 holds the port sessions are sent to, and the service
    /// loads both credentials and keeps every line of its isolation: dropping one fails here.
    #[test]
    fn the_units_keep_the_port_credentials_and_isolation() {
        let socket: Vec<&str> = include_str!("../../../scripts/key-proxy/relay-key-proxy.socket").lines().collect();
        assert!(socket.contains(&format!("ListenStream=127.0.0.1:{PORT}").as_str()) && socket.contains(&"Accept=no"));
        assert_eq!(socket.iter().filter(|line| line.starts_with("Listen")).count(), 1, "one loopback listener only");
        let unit = include_str!("../../../scripts/key-proxy/relay-key-proxy.service");
        let lines: Vec<&str> = unit.lines().collect();
        assert!(lines.contains(&"ExecStart=/usr/local/libexec/relay/relay-key-proxy"));
        for line in [
            "Requires=relay-key-proxy.socket", "DynamicUser=yes", "LoadCredentialEncrypted=relay-anthropic-key", "LoadCredentialEncrypted=relay-proxy-token",
            "NoNewPrivileges=yes", "CapabilityBoundingSet=", "AmbientCapabilities=", "ProtectSystem=strict", "ProtectHome=yes",
            "PrivateTmp=yes", "PrivateDevices=yes", "ProtectKernelTunables=yes", "ProtectKernelModules=yes",
            "RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX", "RestrictNamespaces=yes", "RestrictSUIDSGID=yes",
            "MemoryDenyWriteExecute=yes", "SystemCallArchitectures=native", "SystemCallFilter=@system-service",
            "LimitCORE=0", "UMask=0077", "SocketBindDeny=any",
        ] {
            assert!(lines.contains(&line), "the unit lost {line:?}");
        }
        // Nothing that would run it as someone else, add a command with more power, or widen it.
        for prefix in ["User=", "Group=", "ExecStartPre=", "ExecStartPost=", "ExecReload=", "ExecStop", "Environment", "SupplementaryGroups=",
            "ReadWritePaths=", "BindPaths=", "Delegate=", "PermissionsStartOnly="] {
            assert!(!lines.iter().any(|line| line.starts_with(prefix)), "the unit must not set {prefix}");
        }
        assert_eq!(lines.iter().filter(|line| line.starts_with("ExecStart=")).count(), 1);
        assert!(!unit.contains("ExecStart=+") && !unit.contains("ExecStart=!"));
    }
}
