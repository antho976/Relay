//! `relay key-proxy` — holds the Anthropic API key for Relay's API-key sessions (docs/KEY-PROXY.md).
//!
//! It runs as its own throwaway system user (`relay-key-proxy.service`, `DynamicUser=yes`), and
//! systemd hands it the key from a credential encrypted with the host's root-only key. Nothing that
//! runs as the person — an agent, a repository's hooks, a line injected over the phone link — can
//! read a file or the memory of another user, so none of them can ever learn the key.
//!
//! A session's Claude talks to it on loopback with a local pass token in `x-api-key`. A request
//! that carries the token is sent on to `api.anthropic.com` alone, over TLS checked against the
//! roots built into this binary, with the token swapped for the key. The token lets a program
//! running as the person spend through the proxy; it never lets one see the key.

use anyhow::{anyhow, bail, Context, Result};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The systemd credential names (`LoadCredentialEncrypted=` in the unit).
const KEY_CREDENTIAL: &str = "relay-anthropic-key";
const TOKEN_CREDENTIAL: &str = "relay-proxy-token";
/// Where every request goes. Fixed: nothing a caller sends can name another host.
const UPSTREAM: &str = "api.anthropic.com";
const MAX_HEAD: usize = 64 * 1024;
const MAX_HEADERS: usize = 128;
const MAX_CONNECTIONS: usize = 64;
const HEAD_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Never forwarded: the caller's credentials, and hop-by-hop headers this proxy sets itself.
const DROPPED: [&str; 9] =
    ["host", "x-api-key", "authorization", "proxy-authorization", "connection", "proxy-connection", "keep-alive", "upgrade", "te"];

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
    fn anthropic() -> Result<Upstream> {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Upstream::Anthropic(tokio_rustls::TlsConnector::from(Arc::new(config))))
    }

    async fn connect(&self) -> Result<Box<dyn Stream>> {
        match self {
            Upstream::Anthropic(tls) => {
                let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((UPSTREAM, 443))).await??;
                tcp.set_nodelay(true)?;
                let name = rustls::pki_types::ServerName::try_from(UPSTREAM)?;
                Ok(Box::new(tokio::time::timeout(CONNECT_TIMEOUT, tls.connect(name, tcp)).await??))
            }
            #[cfg(test)]
            Upstream::Plain(addr) => Ok(Box::new(TcpStream::connect(addr).await?)),
        }
    }
}

/// Read one systemd credential, trimmed. The error never carries the content.
fn credential(dir: &Path, name: &str) -> Result<String> {
    let raw = std::fs::read_to_string(dir.join(name)).with_context(|| format!("reading credential {name}"))?;
    Ok(raw.trim().to_string())
}

fn key_shaped(key: &str) -> bool {
    key.len() > 20 && key.starts_with("sk-ant-") && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub async fn run(listen: &str) -> Result<u8> {
    let dir = std::env::var_os("CREDENTIALS_DIRECTORY")
        .ok_or_else(|| anyhow!("no credentials: run this as relay-key-proxy.service (scripts/key-proxy/install.sh)"))?;
    let key = credential(Path::new(&dir), KEY_CREDENTIAL)?;
    if !key_shaped(&key) {
        bail!("credential {KEY_CREDENTIAL} is not an Anthropic API key (sk-ant-…)");
    }
    let token = credential(Path::new(&dir), TOKEN_CREDENTIAL)?;
    if token.len() < 32 {
        bail!("credential {TOKEN_CREDENTIAL} is shorter than 32 characters");
    }
    let addr: SocketAddr = listen.parse().with_context(|| format!("--listen {listen:?} is not an address"))?;
    if !addr.ip().is_loopback() {
        bail!("--listen {addr} is not a loopback address: the proxy answers this machine only");
    }
    let listener = TcpListener::bind(addr).await.with_context(|| format!("listening on {addr}"))?;
    eprintln!("relay key-proxy: listening on {addr}, forwarding to https://{UPSTREAM}");
    serve(listener, Arc::new(Secrets { key, token }), Arc::new(Upstream::anthropic()?)).await;
    Ok(0)
}

async fn serve(listener: TcpListener, secrets: Arc<Secrets>, upstream: Arc<Upstream>) {
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    loop {
        let Ok((mut client, _)) = listener.accept().await else { continue };
        let (secrets, upstream) = (secrets.clone(), upstream.clone());
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            tokio::spawn(async move { let _ = refuse(&mut client, 503, "too many connections to the key proxy").await; });
            continue;
        };
        tokio::spawn(async move {
            let _slot = slot;
            if let Err(error) = handle(client, &secrets, &upstream).await {
                // Never the request: its headers carried the token.
                eprintln!("relay key-proxy: {error:#}");
            }
        });
    }
}

/// An error in the shape Anthropic's API answers with, so Claude shows the message.
async fn refuse(client: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    let reason = match status { 400 => "Bad Request", 401 => "Unauthorized", 431 => "Request Header Fields Too Large", 502 => "Bad Gateway", _ => "Service Unavailable" };
    let body = serde_json::json!({"type": "error", "error": {"type": "relay_key_proxy", "message": message}}).to_string();
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    client.write_all(response.as_bytes()).await?;
    let _ = client.shutdown().await;
    // Closed with the caller's bytes unread, the socket resets and the answer can be lost: read
    // what is still coming for a moment first.
    let mut sink = [0u8; 8192];
    let drain = async { while client.read(&mut sink).await.is_ok_and(|n| n > 0) {} };
    let _ = tokio::time::timeout(Duration::from_secs(1), drain).await;
    Ok(())
}

/// Read up to the end of the request head; returns its length. Bytes after it (the start of the
/// body) stay in `buf`.
async fn read_head(client: &mut TcpStream, buf: &mut Vec<u8>) -> Result<Option<usize>> {
    let mut chunk = [0u8; 8192];
    loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            return Ok((at + 4 <= MAX_HEAD).then_some(at + 4));
        }
        if buf.len() > MAX_HEAD {
            return Ok(None);
        }
        let n = client.read(&mut chunk).await?;
        if n == 0 {
            bail!("connection closed before the request head ended");
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// The head sent to Anthropic, or why the request is refused.
fn rewrite(head: &[u8], secrets: &Secrets) -> std::result::Result<Vec<u8>, (u16, &'static str)> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    match request.parse(head) {
        Ok(httparse::Status::Complete(_)) => {}
        _ => return Err((400, "malformed request")),
    }
    let (Some(method), Some(path)) = (request.method, request.path) else { return Err((400, "malformed request")) };
    // Origin form only: a target naming a host is for a forward proxy, which this is not.
    if !path.starts_with('/') {
        return Err((400, "the request target must be a path"));
    }
    let count = |name: &str| request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(name)).count();
    let authorized = count("x-api-key") == 1
        && request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("x-api-key"))
            .all(|h| std::str::from_utf8(h.value).is_ok_and(|v| relay_remote::wire::digest_eq(v, &secrets.token)));
    if !authorized {
        return Err((401, "not authorized by the Relay key proxy"));
    }
    // One framing, stated once: anything else is how a request is smuggled past a proxy.
    let lengths = count("content-length");
    let chunked = count("transfer-encoding");
    if lengths > 1 || chunked > 1 || (lengths == 1 && chunked == 1) {
        return Err((400, "ambiguous request framing"));
    }
    let mut out = Vec::with_capacity(head.len() + secrets.key.len() + 64);
    out.extend_from_slice(format!("{method} {path} HTTP/1.1\r\nhost: {UPSTREAM}\r\nx-api-key: ").as_bytes());
    out.extend_from_slice(secrets.key.as_bytes());
    out.extend_from_slice(b"\r\nconnection: close\r\n");
    for header in request.headers.iter().filter(|h| !DROPPED.iter().any(|d| h.name.eq_ignore_ascii_case(d))) {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    Ok(out)
}

async fn handle(mut client: TcpStream, secrets: &Secrets, upstream: &Upstream) -> Result<()> {
    client.set_nodelay(true)?;
    let mut buf = Vec::with_capacity(8192);
    let head_len = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut client, &mut buf)).await {
        Ok(Ok(Some(len))) => len,
        Ok(Ok(None)) => return refuse(&mut client, 431, "request head too large").await,
        Ok(Err(error)) => return Err(error),
        Err(_) => return refuse(&mut client, 400, "request head timed out").await,
    };
    let head = match rewrite(&buf[..head_len], secrets) {
        Ok(head) => head,
        Err((status, message)) => return refuse(&mut client, status, message).await,
    };
    let mut server = match upstream.connect().await {
        Ok(server) => server,
        Err(error) => {
            refuse(&mut client, 502, "the key proxy could not reach api.anthropic.com").await?;
            return Err(error.context("connecting to api.anthropic.com"));
        }
    };
    server.write_all(&head).await?;
    drop(head);
    server.write_all(&buf[head_len..]).await?;
    server.flush().await?;
    // One request per connection (`connection: close` upstream): the response ends when Anthropic
    // closes, and the caller's connection with it. The body streams both ways meanwhile.
    let (mut client_read, mut client_write) = client.into_split();
    let (mut server_read, mut server_write) = tokio::io::split(server);
    let up = async {
        let _ = tokio::io::copy(&mut client_read, &mut server_write).await;
        let _ = server_write.flush().await;
    };
    {
        let down = tokio::io::copy(&mut server_read, &mut client_write);
        tokio::pin!(up, down);
        let mut up_done = false;
        loop {
            tokio::select! {
                _ = &mut up, if !up_done => up_done = true,
                result = &mut down => {
                    result?;
                    break;
                }
            }
        }
    }
    let _ = client_write.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-ant-api03-REALKEY_never-shown";
    const TOKEN: &str = "local-pass-token-0123456789abcdefghij";

    /// A stand-in for Anthropic: records the head it receives and answers with a streamed body.
    async fn fake_upstream() -> (SocketAddr, tokio::sync::mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    // Head plus the 13-byte body the tests send.
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") || !buf.ends_with(b"{\"hello\":\"x\"}") {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 { break; }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    tx.send(buf).await.unwrap();
                    socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n").await.unwrap();
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
        let secrets = Arc::new(Secrets { key: KEY.into(), token: TOKEN.into() });
        tokio::spawn(serve(listener, secrets, Arc::new(Upstream::Plain(upstream))));
        addr
    }

    async fn send(proxy: SocketAddr, request: &str) -> String {
        let mut socket = TcpStream::connect(proxy).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut out = String::new();
        tokio::time::timeout(Duration::from_secs(5), socket.read_to_string(&mut out)).await.unwrap().unwrap();
        out
    }

    fn request(extra: &str) -> String {
        format!("POST /v1/messages HTTP/1.1\r\nhost: 127.0.0.1\r\n{extra}content-type: application/json\r\ncontent-length: 13\r\nconnection: keep-alive\r\n\r\n{{\"hello\":\"x\"}}")
    }

    /// The token is swapped for the key on the way to Anthropic and is not forwarded; the body
    /// goes up and the streamed answer comes back whole; the caller never sees the key.
    #[tokio::test]
    async fn an_authorized_request_reaches_anthropic_with_the_key_in_place_of_the_token() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let answer = send(proxy, &request(&format!("x-api-key: {TOKEN}\r\nauthorization: Bearer junk\r\n"))).await;
        assert!(answer.starts_with("HTTP/1.1 200 OK") && answer.ends_with("event: a\n\nevent: b\n\n"), "{answer:?}");
        assert!(!answer.contains(KEY));
        let head = String::from_utf8(seen.recv().await.unwrap()).unwrap();
        let lower = head.to_ascii_lowercase();
        assert!(head.starts_with("POST /v1/messages HTTP/1.1\r\nhost: api.anthropic.com\r\n"), "{head:?}");
        assert!(head.contains(&format!("x-api-key: {KEY}\r\n")), "{head:?}");
        assert!(!head.contains(TOKEN) && !lower.contains("authorization") && !lower.contains("keep-alive"), "{head:?}");
        assert_eq!(lower.matches("host:").count(), 1, "{head:?}");
        assert!(lower.contains("connection: close\r\n") && head.ends_with("\r\n\r\n{\"hello\":\"x\"}"), "{head:?}");
    }

    /// Without the token, with a wrong one or two of them, with a target naming another host, or
    /// with framing that two parsers could read apart, nothing is forwarded.
    #[tokio::test]
    async fn anything_else_is_refused_without_reaching_anthropic() {
        let (upstream, mut seen) = fake_upstream().await;
        let proxy = proxy(upstream).await;
        let cases = [
            (request(""), "401"),
            (request("x-api-key: wrong-token-0123456789abcdefghijkl\r\n"), "401"),
            (request(&format!("x-api-key: {TOKEN}\r\nx-api-key: other\r\n")), "401"),
            (request(&format!("x-api-key: {TOKEN}\r\ntransfer-encoding: chunked\r\n")), "400"),
            (format!("POST http://evil.example/v1/messages HTTP/1.1\r\nx-api-key: {TOKEN}\r\ncontent-length: 0\r\n\r\n"), "400"),
            ("garbage\r\n\r\n".to_string(), "400"),
            (format!("GET / HTTP/1.1\r\nx-api-key: {TOKEN}\r\nx-pad: {}\r\n\r\n", "a".repeat(MAX_HEAD + 10)), "431"),
        ];
        for (request, status) in cases {
            let answer = send(proxy, &request).await;
            assert!(answer.starts_with(&format!("HTTP/1.1 {status}")), "{status}: {answer:?}");
            assert!(!answer.contains(KEY));
        }
        assert!(seen.try_recv().is_err(), "a refused request reached the upstream");
    }

    /// The unit that runs the proxy listens where sessions are sent.
    #[test]
    fn the_unit_listens_on_the_port_sessions_use() {
        let unit = include_str!("../../../scripts/key-proxy/relay-key-proxy.service");
        let listen = format!("key-proxy --listen 127.0.0.1:{}\n", relay_core::hooks::KEY_PROXY_PORT);
        assert!(unit.contains(&listen), "the unit's ExecStart must end with {listen:?}");
        for credential in [KEY_CREDENTIAL, TOKEN_CREDENTIAL] {
            assert!(unit.contains(&format!("LoadCredentialEncrypted={credential}\n")), "{credential}");
        }
        assert!(unit.contains("DynamicUser=yes\n"));
    }

    #[test]
    fn only_a_key_shaped_credential_is_taken() {
        assert!(key_shaped(KEY));
        for bad in ["", "sk-ant-", "sk-ant-api03-a b c d e f", "sk-proj-xyzxyzxyzxyzxyz", "sk-ant-api03-x\nInjected: 1"] {
            assert!(!key_shaped(bad), "{bad:?}");
        }
    }
}
