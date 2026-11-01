//! Posting to the person's YouTube channel: the sign-in (OAuth for an installed app, with PKCE and
//! a one-shot listener on 127.0.0.1 for Google's answer), the uploads (YouTube Data API v3,
//! resumable), and the poster thread that sends approved clips out at the person's slots.
//!
//! Relay has no Google client of its own: the person makes a "Desktop app" OAuth client in their
//! own Google Cloud project (YouTube Data API v3 on) and pastes its ID and secret. Those and the
//! sign-in's refresh token are kept in the desktop keyring (`secret-tool`), a 0600 file on the
//! test instance, nowhere for a store in memory; never in `clips.db`, never answered or logged.
//!
//! Requests go to Google's hosts only, over https, never following a redirect, each with a
//! deadline. Until a project passes Google's audit, YouTube keeps what it uploads private: the
//! space says so.

use super::{changed, lock, with_db};
use crate::engine::Engine;
use crate::Instance;
use relay_bus::ops::clips::*;
use relay_bus::BusError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REVOKE: &str = "https://oauth2.googleapis.com/revoke";
const CHANNELS: &str = "https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true";
const UPLOAD: &str = "https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status";
const SCOPES: &str = "https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/youtube.readonly";
/// How long a sign-in waits for the browser.
const SIGN_IN_WITHIN: Duration = Duration::from_secs(10 * 60);
const KEYRING_WITHIN: Duration = Duration::from_secs(20);
/// The largest clip Relay uploads.
const MOST_UPLOAD: u64 = 2 << 30;

/// A sign-in waiting for Google to send the browser back.
pub struct Pending {
    pub port: u16,
    pub state: String,
    pub started: Instant,
}

/// What came back from a request.
#[derive(Debug, Clone, Default)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
    pub location: Option<String>,
    /// A resumable upload's `Range` (`bytes=0-<last byte Google has>`), on its 308.
    pub range: Option<String>,
}

/// A request, as the tests' stand-in sees it.
#[derive(Debug, Clone)]
pub struct Call {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub type Fake = Arc<dyn Fn(&Call) -> Answer + Send + Sync>;

static FAKE: std::sync::Mutex<Option<Fake>> = std::sync::Mutex::new(None);

/// Answer every Google request with `fake` instead of the network: what the engine tests use.
#[doc(hidden)]
pub fn use_fake(fake: Option<Fake>) {
    *lock(&FAKE) = fake;
}

// ---------------------------------------------------------------- secrets

/// The client and the sign-in, as kept.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Secrets {
    client_id: String,
    client_secret: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn secrets_file(engine: &Engine) -> Option<Result<PathBuf, ()>> {
    let store = engine.store.path();
    if store == Path::new(":memory:") {
        None
    } else if engine.instance == Instance::Test {
        Some(Ok(store.with_file_name("clips-youtube.json")))
    } else {
        Some(Err(()))
    }
}

const ATTRIBUTES: [&str; 4] = ["application", "Relay", "purpose", "clips-youtube"];

fn load(engine: &Engine) -> Result<Option<Secrets>, BusError> {
    match secrets_file(engine) {
        None => Ok(None),
        Some(Ok(file)) => Ok(std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok())),
        Some(Err(())) => {
            let out = crate::proc::output_with_timeout(Command::new("secret-tool").arg("lookup").args(ATTRIBUTES), KEYRING_WITHIN)
                .map_err(|e| BusError::unavailable("clips.keyring", format!("secret-tool: {e}")).with_hint("install libsecret (secret-tool)"))?
                .ok_or_else(|| BusError::unavailable("clips.keyring", "the desktop keyring did not answer").with_hint("unlock the desktop keyring"))?;
            if out.status.success() {
                return Ok(serde_json::from_slice(&out.stdout).ok());
            }
            Ok(None)
        }
    }
}

/// What the keyring holds of the sign-in, as [`status`] tells it: kept in memory
/// (`Shelf::keyring`) until the secrets are next written, so the space's refreshes and the
/// poster's ticks do not each start the keyring's tool.
#[derive(Debug, Clone, Default)]
pub(crate) struct Kept {
    configured: bool,
    connected: bool,
    /// Why the keyring could not be read: asked again after [`KEYRING_RETRY`].
    note: Option<String>,
}

/// How long a keyring that did not answer (locked, or its tool missing) is taken at its word.
const KEYRING_RETRY: Duration = Duration::from_secs(30);

fn kept(engine: &Engine) -> Kept {
    let generation = {
        let slot = lock(&engine.clips.keyring);
        if let Some((k, _)) = slot.1.as_ref().filter(|(k, at)| k.note.is_none() || at.elapsed() < KEYRING_RETRY) {
            return k.clone();
        }
        slot.0
    };
    let k = match load(engine) {
        Ok(s) => Kept {
            configured: s.as_ref().is_some_and(|s| !s.client_id.is_empty()),
            connected: s.as_ref().is_some_and(|s| s.refresh_token.is_some()),
            note: None,
        },
        Err(e) => Kept { note: Some(e.message), ..Default::default() },
    };
    let mut slot = lock(&engine.clips.keyring);
    if slot.0 == generation {
        slot.1 = Some((k.clone(), Instant::now()));
    }
    k
}

/// The secrets were written: what was kept of them is read again next time.
fn forget_kept(engine: &Engine) {
    let mut slot = lock(&engine.clips.keyring);
    slot.0 += 1;
    slot.1 = None;
}

fn save(engine: &Engine, s: &Secrets) -> Result<(), BusError> {
    let saved = save_secrets(engine, s);
    forget_kept(engine);
    saved
}

fn save_secrets(engine: &Engine, s: &Secrets) -> Result<(), BusError> {
    let text = serde_json::to_string(s).map_err(|e| BusError::internal(e.to_string()))?;
    match secrets_file(engine) {
        None => Err(BusError::unavailable("clips.keyring", "this store keeps no keys")),
        Some(Ok(file)) => crate::private::write_private_replace(&file, text.as_bytes()).map_err(|e| BusError::unavailable("clips.keyring", format!("{}: {e}", file.display()))),
        Some(Err(())) => {
            let mut cmd = Command::new("secret-tool");
            cmd.arg("store").arg("--label=Relay Clips: YouTube").args(ATTRIBUTES);
            let out = crate::proc::output_with_input(&mut cmd, text.as_bytes(), KEYRING_WITHIN)
                .map_err(|e| BusError::unavailable("clips.keyring", format!("secret-tool: {e}")))?
                .ok_or_else(|| BusError::unavailable("clips.keyring", "the desktop keyring did not answer"))?;
            if !out.status.success() {
                return Err(BusError::unavailable("clips.keyring", "the desktop keyring refused to keep the sign-in").with_hint("unlock the desktop keyring"));
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- the network

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_connect(Some(Duration::from_secs(10)))
            .user_agent("relay-clips")
            .build()
            .into()
    })
}

fn google(url: &str) -> bool {
    url.parse::<ureq::http::Uri>().ok().and_then(|u| u.host().map(str::to_string))
        .is_some_and(|h| h == "accounts.google.com" || h == "oauth2.googleapis.com" || h == "www.googleapis.com")
}

fn send(call: Call, deadline: Duration) -> Result<Answer, BusError> {
    if !google(&call.url) {
        return Err(BusError::invalid("clips.youtube", "Relay talks to Google's own hosts only"));
    }
    if let Some(fake) = lock(&FAKE).clone() {
        return Ok(fake(&call));
    }
    let lost = |e: ureq::Error| match e {
        ureq::Error::Timeout(_) => BusError::unavailable("clips.youtube.timeout", format!("Google did not answer within {} s", deadline.as_secs())),
        e => {
            tracing::debug!(error = %e, "clips: a Google request got no answer");
            BusError::unavailable("clips.youtube.offline", "Google could not be reached").with_hint("check the network, then try again")
        }
    };
    let mut response = match call.method {
        "GET" => {
            let mut b = agent().get(&call.url);
            for (k, v) in &call.headers {
                b = b.header(k.as_str(), v.as_str());
            }
            b.config().timeout_global(Some(deadline)).build().call().map_err(lost)?
        }
        method => {
            let mut b = if method == "PUT" { agent().put(&call.url) } else { agent().post(&call.url) };
            for (k, v) in &call.headers {
                b = b.header(k.as_str(), v.as_str());
            }
            b.config().timeout_global(Some(deadline)).build().send(&call.body[..]).map_err(lost)?
        }
    };
    let status = response.status().as_u16();
    let location = response.headers().get("location").and_then(|v| v.to_str().ok()).map(str::to_string);
    let range = response.headers().get("range").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = response.body_mut().with_config().limit(8 << 20).read_to_vec().map_err(lost)?;
    Ok(Answer { status, body, location, range })
}

fn form(fields: &[(&str, &str)]) -> Vec<u8> {
    fields.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&").into_bytes()
}

/// Percent-encoding for a query or a form.
pub fn enc(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn google_error(a: &Answer) -> String {
    let v: Value = serde_json::from_slice(&a.body).unwrap_or_default();
    let msg = v["error"]["message"].as_str().or(v["error_description"].as_str()).or(v["error"].as_str()).map(str::to_string)
        .unwrap_or_else(|| String::from_utf8_lossy(&a.body).chars().take(200).collect());
    format!("Google answered {}: {msg}", a.status)
}

fn b64url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random(n: usize) -> String {
    use rand::RngCore;
    let mut bytes = vec![0u8; n];
    rand::rng().fill_bytes(&mut bytes);
    b64url(&bytes)
}

// ---------------------------------------------------------------- status and sign-in

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Channel {
    channel: Option<String>,
    channel_id: Option<String>,
    note: Option<String>,
}

fn channel(engine: &Engine) -> Channel {
    with_db(engine, |db| db.meta("youtube")).ok().flatten().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default()
}

fn set_channel(engine: &Engine, c: &Channel) {
    if let Ok(raw) = serde_json::to_string(c) {
        let _ = with_db(engine, |db| db.set_meta("youtube", &raw));
    }
}

/// The channel's sign-in: the keyring as last read ([`Kept`]), the channel and the sign-in
/// under way as they are now.
pub fn status(engine: &Engine) -> YoutubeStatus {
    let k = kept(engine);
    let c = channel(engine);
    let pending = lock(&engine.clips.oauth).as_ref().is_some_and(|p| p.started.elapsed() < SIGN_IN_WITHIN);
    match k.note {
        None => YoutubeStatus { configured: k.configured, connected: k.connected, channel: c.channel, channel_id: c.channel_id, pending, note: c.note },
        Some(why) => YoutubeStatus { pending, note: Some(why), ..Default::default() },
    }
}

pub fn setup(engine: &Engine, client_id: &str, client_secret: &str) -> Result<YoutubeStatus, BusError> {
    let id = client_id.trim();
    let secret = client_secret.trim();
    if !id.ends_with(".apps.googleusercontent.com") || id.len() > 200 || id.contains(char::is_whitespace) {
        return Err(BusError::invalid("clips.youtube.client", "that is not a Google OAuth client ID (it ends in .apps.googleusercontent.com)"));
    }
    if secret.is_empty() || secret.len() > 200 || secret.contains(char::is_whitespace) {
        return Err(BusError::invalid("clips.youtube.client", "paste the client's secret too"));
    }
    let old = load(engine)?.unwrap_or_default();
    // A new client's sign-in is not the old one's.
    let refresh_token = if old.client_id == id { old.refresh_token } else { None };
    save(engine, &Secrets { client_id: id.into(), client_secret: secret.into(), refresh_token })?;
    *lock(&engine.clips.access) = None;
    Ok(status(engine))
}

/// Start a sign-in: the page to open, and a thread waiting for Google to send the browser back.
pub fn connect(engine: &Arc<Engine>) -> Result<String, BusError> {
    let s = load(engine)?.filter(|s| !s.client_id.is_empty())
        .ok_or_else(|| BusError::unavailable("clips.youtube.no_client", "save your Google OAuth client first").with_hint("Clips → Settings → YouTube"))?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| BusError::unavailable("clips.youtube", format!("could not listen for Google's answer: {e}")))?;
    let port = listener.local_addr().map_err(|e| BusError::internal(e.to_string()))?.port();
    let state = random(18);
    let verifier = random(48);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let redirect = format!("http://127.0.0.1:{port}");
    let url = format!(
        "{AUTH}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&include_granted_scopes=true&state={}&code_challenge={challenge}&code_challenge_method=S256",
        enc(&s.client_id), enc(&redirect), enc(SCOPES), enc(&state)
    );
    // A newer sign-in takes over: the one before stops listening (its port closes) and says
    // nothing more, so its timeout cannot clear this one's flag or write over how it ended.
    *lock(&engine.clips.oauth) = Some(Pending { port, state: state.clone(), started: Instant::now() });
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new().name("clips-sign-in".into()).spawn(move || {
        let mine = || weak.upgrade().is_some_and(|e| lock(&e.clips.oauth).as_ref().is_some_and(|p| p.state == state));
        let outcome = wait_for_code(&listener, &state, &mine);
        drop(listener);
        let Some(engine) = weak.upgrade() else { return };
        if !mine() {
            return;
        }
        let finished = outcome.and_then(|code| exchange(&engine, &code, &verifier, &redirect));
        let mut c = channel(&engine);
        match finished {
            Ok(()) => {
                c.note = None;
                if let Ok((title, id)) = fetch_channel(&engine) {
                    c.channel = Some(title);
                    c.channel_id = Some(id);
                }
            }
            Err(e) => c.note = Some(e.message),
        }
        {
            let mut pending = lock(&engine.clips.oauth);
            if !pending.as_ref().is_some_and(|p| p.state == state) {
                return;
            }
            set_channel(&engine, &c);
            *pending = None;
        }
        changed(&engine, "youtube", None, None);
        super::poke_poster(&engine);
    }).map_err(|e| BusError::internal(e.to_string()))?;
    Ok(url)
}

/// Wait for the browser to come back with `?code=…&state=…`; thank it either way. Gives up when
/// `mine` says a newer sign-in took over.
fn wait_for_code(listener: &TcpListener, state: &str, mine: &dyn Fn() -> bool) -> Result<String, BusError> {
    listener.set_nonblocking(true).map_err(|e| BusError::internal(e.to_string()))?;
    let deadline = Instant::now() + SIGN_IN_WITHIN;
    loop {
        if !mine() {
            return Err(BusError::unavailable("clips.youtube.superseded", "a newer sign-in took over"));
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut line = String::new();
                let _ = BufReader::new(&mut stream).read_line(&mut line);
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let query = target.split_once('?').map_or("", |(_, q)| q);
                let param = |k: &str| query.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| super::query_value(v)));
                if param("state").is_none() && param("error").is_none() {
                    // A favicon or a stray request: not Google's answer.
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                }
                let ok = param("state").as_deref() == Some(state) && param("code").is_some();
                let page = if ok {
                    "<h2 style='font-family:sans-serif'>Relay is connected to YouTube.</h2><p style='font-family:sans-serif'>You can close this tab.</p>"
                } else {
                    "<h2 style='font-family:sans-serif'>Relay could not connect to YouTube.</h2><p style='font-family:sans-serif'>Go back to Relay and try again.</p>"
                };
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
                if let Some(err) = param("error") {
                    return Err(BusError::unavailable("clips.youtube.denied", format!("Google said: {err}")));
                }
                if !ok {
                    return Err(BusError::unavailable("clips.youtube.state", "the answer did not match this sign-in"));
                }
                return Ok(param("code").unwrap_or_default());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err(BusError::unavailable("clips.youtube.timeout", "the sign-in was not finished in the browser within 10 minutes"));
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(BusError::unavailable("clips.youtube", format!("listening for Google: {e}"))),
        }
    }
}

fn exchange(engine: &Engine, code: &str, verifier: &str, redirect: &str) -> Result<(), BusError> {
    let mut s = load(engine)?.ok_or_else(|| BusError::unavailable("clips.youtube.no_client", "the OAuth client was removed"))?;
    let a = send(Call {
        method: "POST", url: TOKEN.into(), headers: vec![("content-type".into(), "application/x-www-form-urlencoded".into())],
        body: form(&[("code", code), ("client_id", &s.client_id), ("client_secret", &s.client_secret), ("redirect_uri", redirect),
            ("grant_type", "authorization_code"), ("code_verifier", verifier)]),
    }, Duration::from_secs(30))?;
    if a.status != 200 {
        return Err(BusError::unavailable("clips.youtube.token", google_error(&a)));
    }
    let v: Value = serde_json::from_slice(&a.body).unwrap_or_default();
    let refresh = v["refresh_token"].as_str().ok_or_else(|| BusError::unavailable("clips.youtube.token", "Google sent no refresh token: sign in again"))?;
    s.refresh_token = Some(refresh.into());
    save(engine, &s)?;
    if let (Some(at), Some(secs)) = (v["access_token"].as_str(), v["expires_in"].as_u64()) {
        *lock(&engine.clips.access) = Some((at.into(), Instant::now() + Duration::from_secs(secs.saturating_sub(60))));
    }
    Ok(())
}

/// Sign out: the sign-in is revoked at Google (so a copy of it elsewhere is worth nothing; best
/// effort, offline it is only forgotten) and forgotten here, the client kept.
pub fn disconnect(engine: &Engine) -> Result<YoutubeStatus, BusError> {
    if let Some(mut s) = load(engine)? {
        if let Some(token) = s.refresh_token.take() {
            let revoked = send(Call {
                method: "POST", url: REVOKE.into(), headers: vec![("content-type".into(), "application/x-www-form-urlencoded".into())],
                body: form(&[("token", &token)]),
            }, Duration::from_secs(10));
            if let Err(e) = revoked.as_ref().map_err(|e| e.message.clone()).and_then(|a| if a.status == 200 { Ok(()) } else { Err(google_error(a)) }) {
                tracing::info!(error = %e, "clips: Google did not take the sign-out; the sign-in is forgotten here all the same");
            }
        }
        save(engine, &s)?;
    }
    *lock(&engine.clips.access) = None;
    set_channel(engine, &Channel::default());
    Ok(status(engine))
}

/// A current access token, refreshed when it ran out.
fn access(engine: &Engine) -> Result<String, BusError> {
    if let Some((t, until)) = lock(&engine.clips.access).clone() {
        if Instant::now() < until {
            return Ok(t);
        }
    }
    let s = load(engine)?.ok_or_else(|| BusError::unavailable("clips.youtube.no_client", "no YouTube channel is connected"))?;
    let refresh = s.refresh_token.clone().ok_or_else(|| BusError::unavailable("clips.youtube.signed_out", "no YouTube channel is connected").with_hint("Clips → Settings → YouTube → Connect"))?;
    let a = send(Call {
        method: "POST", url: TOKEN.into(), headers: vec![("content-type".into(), "application/x-www-form-urlencoded".into())],
        body: form(&[("client_id", &s.client_id), ("client_secret", &s.client_secret), ("refresh_token", &refresh), ("grant_type", "refresh_token")]),
    }, Duration::from_secs(30))?;
    if a.status != 200 {
        let why = google_error(&a);
        if a.status == 400 || a.status == 401 {
            // Revoked, or a project in testing whose tokens last seven days: sign in again.
            let mut c = channel(engine);
            c.note = Some(format!("{why}. Connect again."));
            set_channel(engine, &c);
            let mut s = s;
            s.refresh_token = None;
            let _ = save(engine, &s);
        }
        return Err(BusError::unavailable("clips.youtube.signed_out", why));
    }
    let v: Value = serde_json::from_slice(&a.body).unwrap_or_default();
    let token = v["access_token"].as_str().ok_or_else(|| BusError::unavailable("clips.youtube.token", "Google sent no access token"))?.to_string();
    let secs = v["expires_in"].as_u64().unwrap_or(3600);
    *lock(&engine.clips.access) = Some((token.clone(), Instant::now() + Duration::from_secs(secs.saturating_sub(60))));
    Ok(token)
}

fn fetch_channel(engine: &Engine) -> Result<(String, String), BusError> {
    let token = access(engine)?;
    let a = send(Call { method: "GET", url: CHANNELS.into(), headers: vec![("authorization".into(), format!("Bearer {token}"))], body: vec![] }, Duration::from_secs(20))?;
    if a.status != 200 {
        return Err(BusError::unavailable("clips.youtube", google_error(&a)));
    }
    let v: Value = serde_json::from_slice(&a.body).unwrap_or_default();
    let item = &v["items"][0];
    Ok((item["snippet"]["title"].as_str().unwrap_or("Your channel").to_string(), item["id"].as_str().unwrap_or("").to_string()))
}

// ---------------------------------------------------------------- uploads

/// The description a clip goes out with: its line, the streamer's credit, the footer, #shorts.
pub fn description(c: &Clip, s: &Settings) -> String {
    let mut out = c.description.trim().to_string();
    let who = if s.streamer.trim().is_empty() { s.handle.trim() } else { s.streamer.trim() };
    if !who.is_empty() {
        let link = match (s.platform.trim().to_ascii_lowercase().as_str(), s.handle.trim()) {
            (_, "") => String::new(),
            ("twitch", h) => format!(": https://twitch.tv/{}", h.to_ascii_lowercase()),
            ("kick", h) => format!(": https://kick.com/{}", h.to_ascii_lowercase()),
            ("youtube", h) => format!(": https://youtube.com/@{h}"),
            (_, h) => format!(" (@{h})"),
        };
        out.push_str(&format!("\n\nFrom {who}'s stream{link}"));
    }
    if !s.posting.footer.trim().is_empty() {
        out.push_str(&format!("\n\n{}", s.posting.footer.trim()));
    }
    // YouTube takes 5000 bytes: the body is cut first, so #shorts is never what is lost.
    const MOST: usize = 4900;
    const TAG: &str = "\n\n#shorts";
    if out.len() <= MOST && out.to_ascii_lowercase().contains("#shorts") {
        return out;
    }
    let mut end = out.len().min(MOST - TAG.len());
    while !out.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{TAG}", out[..end].trim_end())
}

/// Upload clip `c`'s file: its YouTube id.
pub fn upload(engine: &Engine, c: &Clip, s: &Settings, privacy: Privacy) -> Result<String, BusError> {
    let file = c.file.as_deref().ok_or_else(|| BusError::unavailable("clips.unrendered", "the clip has not been drawn yet"))?;
    let bytes = std::fs::read(file).map_err(|e| BusError::unavailable("clips.file", format!("{file}: {e}")))?;
    if bytes.len() as u64 > MOST_UPLOAD {
        return Err(BusError::invalid("clips.file", "the clip's file is over 2 GB"));
    }
    // What its render drew, and nothing written over it since (another render to its path).
    if let Some(sha) = with_db(engine, |db| db.file_sha(c.id))? {
        if super::sha_hex(&bytes) != sha {
            return Err(BusError::conflict("clips.file", "the clip's file changed since it was drawn: draw it again"));
        }
    }
    let total = bytes.len() as u64;
    // An upload opened before (a try that failed, or an engine stopped in the middle) is asked
    // after first: finished, it is the post; part sent, it goes on from there. Never a second copy.
    let opened = match with_db(engine, |db| db.upload(c.id))? {
        Some((place, bytes_then)) if bytes_then == total => match session(&place, total) {
            Ok(Session::Done(id)) => return Ok(id),
            Ok(Session::At(next)) => Some((place, next)),
            Ok(Session::Gone) => None,
            Err(e) => return Err(may_have_arrived(&e.message)),
        },
        _ => None,
    };
    let (place, from) = match opened {
        Some(p) => p,
        None => (open_upload(engine, c, s, privacy, total)?, 0),
    };
    send_bytes(c.id, &place, &bytes, from)
}

fn may_have_arrived(why: &str) -> BusError {
    BusError::unavailable("clips.youtube.upload", format!("{why}; it may have reached YouTube: look on your channel before trying again"))
}

/// Open a resumable upload for clip `c`, `total` bytes: its address, kept before a byte is sent.
fn open_upload(engine: &Engine, c: &Clip, s: &Settings, privacy: Privacy, total: u64) -> Result<String, BusError> {
    let token = access(engine)?;
    let tags: Vec<&str> = {
        // YouTube takes 500 characters of tags in all.
        let mut used = 0;
        c.tags.iter().map(String::as_str).filter(|t| { used += t.len() + 3; used <= 480 }).collect()
    };
    let meta = json!({
        "snippet": {"title": super::cut(c.title.trim(), 100), "description": description(c, s), "tags": tags, "categoryId": "20"},
        "status": {"privacyStatus": privacy.name(), "selfDeclaredMadeForKids": false},
    });
    let start = send(Call {
        method: "POST", url: UPLOAD.into(),
        headers: vec![
            ("authorization".into(), format!("Bearer {token}")),
            ("content-type".into(), "application/json; charset=UTF-8".into()),
            ("x-upload-content-type".into(), "video/mp4".into()),
            ("x-upload-content-length".into(), total.to_string()),
        ],
        body: meta.to_string().into_bytes(),
    }, Duration::from_secs(30))?;
    if start.status != 200 {
        return Err(BusError::unavailable("clips.youtube.upload", google_error(&start)));
    }
    let place = start.location.ok_or_else(|| BusError::unavailable("clips.youtube.upload", "Google gave no upload address"))?;
    // Kept before a byte is sent: an engine stopped from here on, or a try that fails, asks
    // Google how the upload ended ([`recover`], [`upload`]) instead of sending the clip a second
    // time. The address is good for this one upload only; `clips.db` is the person's alone (0600).
    with_db(engine, |db| db.set_upload(c.id, Some((&place, total))))?;
    Ok(place)
}

/// How many times an upload goes on from where Google has it before it gives up.
const RESUMES: u32 = 8;

/// Send `bytes` from byte `from` to the upload at `place`, going on from what Google has after a
/// slow line's deadline, a lost answer or Google's own error: the video's id. Any doubt after a
/// byte was sent is put to Google ([`session`]), never settled by sending again.
fn send_bytes(clip: i64, place: &str, bytes: &[u8], mut from: u64) -> Result<String, BusError> {
    let total = bytes.len() as u64;
    for _ in 0..RESUMES {
        let mut headers = vec![("content-type".to_string(), "video/mp4".to_string())];
        if from > 0 {
            headers.push(("content-range".into(), format!("bytes {from}-{}/{total}", total - 1)));
        }
        // A deadline for this stretch at a slow line's pace (64 KB/s); past it, Google is asked
        // how far it got and the upload goes on from there.
        let secs = ((total - from) / (64 * 1024)).clamp(120, 3600);
        let answer = send(Call { method: "PUT", url: place.into(), headers, body: bytes[from as usize..].to_vec() }, Duration::from_secs(secs));
        let why = match answer {
            Ok(a) if (200..300).contains(&a.status) => match video_id(&a) {
                Ok(id) => return Ok(id),
                Err(e) => e.message,
            },
            // Refused outright (not a timeout or a throttle): nothing more of this stretch went.
            Ok(a) if (400..500).contains(&a.status) && ![408, 429].contains(&a.status) => return Err(BusError::unavailable("clips.youtube.upload", google_error(&a))),
            Ok(a) => google_error(&a),
            Err(e) => e.message,
        };
        match session(place, total) {
            Ok(Session::Done(id)) => return Ok(id),
            Ok(Session::At(next)) => {
                tracing::info!(clip, from, next, %why, "clips: an upload goes on from where Google has it");
                from = next.min(total);
            }
            Ok(Session::Gone) | Err(_) => return Err(may_have_arrived(&why)),
        }
    }
    Err(BusError::unavailable("clips.youtube.upload", format!("the upload did not finish in {RESUMES} tries; it goes on from where it stopped next time")))
}

fn video_id(a: &Answer) -> Result<String, BusError> {
    let v: Value = serde_json::from_slice(&a.body).unwrap_or_default();
    v["id"].as_str().map(str::to_string).ok_or_else(|| BusError::unavailable("clips.youtube.upload", "YouTube answered no video id"))
}

/// Where a resumable upload stands at Google.
#[derive(Debug, Clone, PartialEq)]
pub enum Session {
    /// It all arrived: the video's id.
    Done(String),
    /// Not all of it: the next byte to send (YouTube makes no video of an unfinished upload).
    At(u64),
    /// Google no longer knows the address (it ran out).
    Gone,
}

/// Where the resumable upload at `place` (of `total` bytes) stands, asked of Google with an
/// empty PUT; an error when Google cannot say (no network, its own error).
pub fn session(place: &str, total: u64) -> Result<Session, BusError> {
    let a = send(Call { method: "PUT", url: place.into(), headers: vec![("content-range".into(), format!("bytes */{total}"))], body: vec![] }, Duration::from_secs(30))?;
    match a.status {
        200 | 201 => video_id(&a).map(Session::Done),
        // `Range: bytes=0-<last>`, or none when nothing arrived.
        308 => Ok(Session::At(a.range.as_deref().and_then(|r| r.rsplit_once('-')).and_then(|(_, last)| last.trim().parse::<u64>().ok()).map_or(0, |l| l + 1))),
        404 | 410 => Ok(Session::Gone),
        _ => Err(BusError::unavailable("clips.youtube.upload", google_error(&a))),
    }
}

/// Clip `c`, left `posting` by an engine that stopped: posted when its upload arrived (Google
/// says so), back to its slot when it did not (the next try goes on from what Google has), else
/// failed with a word to look on the channel first: never sent twice on a guess.
pub fn recover(engine: &Engine, c: &Clip) {
    let t = crate::time::now();
    let back = if c.publish_at.is_some() { ClipState::Scheduled } else { ClipState::Approved };
    let upload = with_db(engine, |db| db.upload(c.id)).ok().flatten();
    let posting = [ClipState::Posting];
    let _ = match upload {
        // No upload was opened: nothing was sent.
        None => with_db(engine, |db| db.move_clip(c.id, &posting, back, Some("Relay stopped before the upload began; it will try again"), &t)),
        Some((place, total)) => match session(&place, total) {
            Ok(Session::Done(id)) => with_db(engine, |db| db.set_posted(c.id, &id, &t).map(|_| true)),
            Ok(Session::At(_)) => with_db(engine, |db| db.move_clip(c.id, &posting, back, Some("Relay stopped in the middle of the upload; it goes on from there"), &t)),
            Ok(Session::Gone) | Err(_) => {
                tracing::warn!(clip = c.id, "clips: an interrupted upload could not be asked after");
                with_db(engine, |db| db.move_clip(c.id, &posting, ClipState::Failed, Some(
                    "Upload interrupted: Relay stopped while it was uploading, and Google could not say whether it arrived. Look on your channel before trying again."), &t))
            }
        },
    };
    changed(engine, "clip", Some(c.vod_id), Some(c.id));
}

// ---------------------------------------------------------------- the schedule

/// The next free posting time at or after `after`: one of the day's slots (local time) no other
/// clip is scheduled at; `after` itself when there are no slots.
pub fn next_slot(slots: &[String], taken: &[jiff::Timestamp], after: jiff::Timestamp, tz: &jiff::tz::TimeZone) -> jiff::Timestamp {
    if slots.is_empty() {
        return after;
    }
    let day = after.to_zoned(tz.clone()).date();
    for d in 0..60 {
        let Ok(date) = day.checked_add(jiff::Span::new().days(d)) else { break };
        for slot in slots {
            let Some((h, m)) = slot.split_once(':').and_then(|(h, m)| Some((h.parse::<i8>().ok()?, m.parse::<i8>().ok()?))) else { continue };
            let Ok(at) = date.at(h, m, 0, 0).to_zoned(tz.clone()) else { continue };
            let at = at.timestamp();
            if at >= after && !taken.iter().any(|t| (t.as_second() - at.as_second()).abs() < 60) {
                return at;
            }
        }
    }
    after
}

/// The poster: approved clips get a slot, clips whose time came are uploaded, one at a time.
pub fn post_loop(weak: Weak<Engine>) {
    crate::background_priority();
    loop {
        let Some(engine) = weak.upgrade() else { return };
        if engine.is_quitting() {
            return;
        }
        let wake = engine.clips.post_wake.clone();
        let seen = super::pokes(&wake);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tick(&engine))) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::debug!(error = %e.message, "clips: the poster could not look"),
            Err(p) => tracing::error!(why = %super::panic_text(&*p), "clips: the poster panicked"),
        }
        drop(engine);
        super::sleep_on(&wake, seen, Duration::from_secs(60));
    }
}

fn tick(engine: &Engine) -> Result<(), BusError> {
    let s = super::settings(engine)?;
    let now = jiff::Timestamp::now();
    let t = crate::time::stamp(now);
    // On its own: clips that score enough are approved as the person would, but only those
    // nobody has decided on (`auto_ok`): never one the person took back, restored or rejected,
    // one an agent changed, or one whose upload failed (it may be on YouTube already).
    if s.posting.mode == PostingMode::Auto && status(engine).connected {
        for c in with_db(engine, |db| db.undecided(s.posting.min_score.max(1)))? {
            if with_db(engine, |db| db.auto_approve(c.id, &t))? {
                changed(engine, "clip", Some(c.vod_id), Some(c.id));
            }
        }
    }
    // Approved clips take the next free slots, best first.
    let mut approved = with_db(engine, |db| db.clips_in(&[ClipState::Approved]))?;
    approved.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
    let mut taken: Vec<jiff::Timestamp> = with_db(engine, |db| db.clips_in(&[ClipState::Scheduled, ClipState::Posting, ClipState::Posted]))?
        .iter().filter_map(|c| c.publish_at.as_deref()).filter_map(|p| p.parse().ok()).collect();
    let tz = jiff::tz::TimeZone::system();
    for c in approved {
        let at = next_slot(&s.posting.slots, &taken, now, &tz);
        let privacy = c.privacy.unwrap_or(s.posting.privacy);
        // Still approved: drawn again or taken back since this look, it is left as it is.
        if with_db(engine, |db| db.schedule(c.id, ClipState::Approved, Some(&crate::time::stamp(at)), Some(privacy), ClipState::Scheduled, &t))? {
            taken.push(at);
            changed(engine, "clip", Some(c.vod_id), Some(c.id));
        }
    }
    // Upload what is due, when signed in.
    if !status(engine).connected {
        return Ok(());
    }
    let due: Vec<Clip> = with_db(engine, |db| db.clips_in(&[ClipState::Scheduled]))?.into_iter()
        .filter(|c| c.publish_at.as_deref().and_then(|p| p.parse::<jiff::Timestamp>().ok()).is_none_or(|p| p <= now))
        .collect();
    for c in due {
        if engine.is_quitting() {
            break;
        }
        post_now(engine, &c, &s)?;
    }
    Ok(())
}

/// Upload clip `c` now, its row following: posting, then posted or failed.
pub fn post_now(engine: &Engine, c: &Clip, s: &Settings) -> Result<(), BusError> {
    if c.file.is_none() || c.state == ClipState::Rendering {
        return Ok(());
    }
    // A render still drawing to it: what it posts would be half of one picture or the other.
    if c.render_id.and_then(|rid| engine.read_store("clips.render", |conn| crate::video::get_render(conn, rid)).ok()).is_some_and(|r| !r.state.is_final()) {
        return Ok(());
    }
    let t = crate::time::now();
    // Only while it is still scheduled: drawn again or taken back since the poster looked, it
    // stays as the person left it.
    if !with_db(engine, |db| db.start_posting(c.id, &t))? {
        return Ok(());
    }
    changed(engine, "clip", Some(c.vod_id), Some(c.id));
    let c = &super::clip(engine, c.id)?;
    let privacy = c.privacy.unwrap_or(s.posting.privacy);
    let result = upload(engine, c, s, privacy);
    let t = crate::time::now();
    match result {
        Ok(id) => with_db(engine, |db| db.set_posted(c.id, &id, &t))?,
        Err(e) => {
            // Its upload's address is kept: the next try asks Google about it first.
            tracing::warn!(clip = c.id, error = %e.message, "clips: an upload failed");
            with_db(engine, |db| db.move_clip(c.id, &[ClipState::Posting], ClipState::Failed, Some(&format!("Upload failed: {}", e.message)), &t))?;
        }
    }
    changed(engine, "clip", Some(c.vod_id), Some(c.id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_skip_taken_and_past_times() {
        let tz = jiff::tz::TimeZone::UTC;
        let slots = vec!["09:00".to_string(), "17:00".to_string()];
        let at = |s: &str| s.parse::<jiff::Timestamp>().unwrap();
        let now = at("2026-11-01T12:00:00Z");
        assert_eq!(next_slot(&slots, &[], now, &tz), at("2026-11-01T17:00:00Z"));
        assert_eq!(next_slot(&slots, &[at("2026-11-01T17:00:00Z")], now, &tz), at("2026-11-02T09:00:00Z"));
        assert_eq!(next_slot(&[], &[], now, &tz), now);
    }

    #[test]
    fn descriptions_credit_the_streamer() {
        let mut s = super::super::default_settings();
        s.streamer = "Nephilheim".into();
        s.handle = "Nephilheim".into();
        let c = Clip {
            id: 1, vod_id: 1, kind: Kind::Funny, title: "t".into(), description: "He burned it.".into(), tags: vec![], score: 8, why: String::new(),
            start_s: 0.0, end_s: 1.0, length_s: 1.0, edit: ClipEdit {
                keep: vec![[0.0, 1.0]], tighten_ms: None, layout: Layout::Fit, view: None, reframe: Default::default(), hook: String::new(),
                captions: CaptionStyle::Pixel, caption_y: None, emphasis: vec![], fixes: Default::default(), punch_at: vec![], end_card: true, scene: None,
            },
            state: ClipState::Ready, error: None, file: None, poster: None, render_id: None, publish_at: None, privacy: None, youtube_id: None,
            youtube_url: None, posted_at: None, created_at: String::new(), updated_at: String::new(),
        };
        let d = description(&c, &s);
        assert!(d.starts_with("He burned it.\n\nFrom Nephilheim's stream: https://twitch.tv/nephilheim"), "{d}");
        assert!(d.ends_with("#shorts"));
        // A long footer is cut, not #shorts; and the whole stays inside YouTube's 5000 bytes.
        s.posting.footer = "é".repeat(4000);
        let d = description(&c, &s);
        assert!(d.ends_with("\n\n#shorts") && d.len() <= 4900, "{} bytes", d.len());
        assert!(d.starts_with("He burned it."));
        // Already tagged and short: as written.
        let mut tagged = c.clone();
        tagged.description = "#Shorts first".into();
        s.posting.footer.clear();
        assert!(!description(&tagged, &s).ends_with("#shorts"));
    }

    /// The tests that stand Google in (the stand-in is one for the whole process), one at a time.
    static GOOGLE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A clip drawn to `file` and gone to `posting`, as the poster leaves it before the upload.
    fn sample_clip(db: &super::super::store::ClipsDb, vod: i64, file: &str) -> Clip {
        drawn_clip(db, vod, file, None)
    }

    /// [`sample_clip`] whose render's file hashed to `sha`.
    fn drawn_clip(db: &super::super::store::ClipsDb, vod: i64, file: &str, sha: Option<&str>) -> Clip {
        let edit = ClipEdit {
            keep: vec![[0.0, 1.0]], tighten_ms: None, layout: Layout::Fit, view: None, reframe: Default::default(), hook: String::new(),
            captions: CaptionStyle::Pixel, caption_y: None, emphasis: vec![], fixes: Default::default(), punch_at: vec![], end_card: true, scene: None,
        };
        let c = db.add_clip(vod, &super::super::store::NewClip {
            kind: Kind::Funny, title: "t".into(), description: String::new(), tags: vec![], score: 8, why: String::new(), start_s: 0.0, end_s: 1.0, length_s: 1.0, edit,
        }, "t0").unwrap();
        db.set_render_id(c.id, 1, "t0").unwrap();
        db.finish_render(c.id, 1, file, sha, None, None, "t0").unwrap();
        db.schedule(c.id, ClipState::Ready, Some("2026-11-01T17:00:00Z"), None, ClipState::Scheduled, "t0").unwrap();
        db.start_posting(c.id, "t1").unwrap();
        db.clip(c.id).unwrap().unwrap()
    }

    /// Google as one upload sees it: `have` bytes kept so far, done once all arrived; a PUT of
    /// bytes keeps at most `take` of them and then the answer is lost (a slow line's deadline).
    struct Upload {
        have: u64,
        total: u64,
        take: u64,
        status_answer: Option<u16>,
        puts: Vec<(Option<String>, usize)>,
    }

    fn stand_in(up: Arc<std::sync::Mutex<Upload>>) -> Fake {
        Arc::new(move |call: &Call| {
            let mut u = up.lock().unwrap();
            let range = call.headers.iter().find(|(k, _)| k == "content-range").map(|(_, v)| v.clone());
            let answer = |status: u16, body: &str, range: Option<String>| Answer { status, body: body.as_bytes().to_vec(), location: None, range };
            if call.method == "PUT" && call.body.is_empty() {
                if let Some(s) = u.status_answer {
                    return answer(s, "{}", None);
                }
                if u.have >= u.total {
                    return answer(200, "{\"id\": \"v9\"}", None);
                }
                return answer(308, "", (u.have > 0).then(|| format!("bytes=0-{}", u.have - 1)));
            }
            if call.method == "PUT" {
                u.puts.push((range, call.body.len()));
                let took = (call.body.len() as u64).min(u.take);
                u.have += took;
                if u.have >= u.total {
                    return answer(200, "{\"id\": \"v9\"}", None);
                }
                // The line was too slow: Google kept what came, the answer never did.
                return answer(503, "{\"error\": {\"message\": \"backend error\"}}", None);
            }
            answer(404, "{}", None)
        })
    }

    /// An engine stopped mid-upload: Google is asked how the upload ended, and the clip is never
    /// sent again on a guess.
    #[test]
    fn an_interrupted_upload_is_asked_after_not_sent_twice() {
        let _one = GOOGLE.lock().unwrap_or_else(|p| p.into_inner());
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let up = Arc::new(std::sync::Mutex::new(Upload { have: 1234, total: 1234, take: 0, status_answer: None, puts: vec![] }));
        use_fake(Some(stand_in(up.clone())));
        let vod = with_db(&engine, |db| db.add_vod(1, "v", "/v.mp4", "", "", &[], |id| format!("/vods/{id}"), "t0")).unwrap().id;
        let place = "https://www.googleapis.com/upload/youtube/v3/videos?upload_id=u1";
        let state = |id: i64| super::super::clip(&engine, id).unwrap();
        // It all arrived: posted, with the id Google gives.
        let c = with_db(&engine, |db| Ok(sample_clip(db, vod, "/c.mp4"))).unwrap();
        with_db(&engine, |db| db.set_upload(c.id, Some((place, 1234)))).unwrap();
        recover(&engine, &c);
        assert_eq!((state(c.id).state, state(c.id).youtube_id), (ClipState::Posted, Some("v9".to_string())));
        // Part of it: back to its slot, its address kept so the next try goes on from there.
        up.lock().unwrap().have = 100;
        let c = with_db(&engine, |db| Ok(sample_clip(db, vod, "/c.mp4"))).unwrap();
        with_db(&engine, |db| db.set_upload(c.id, Some((place, 1234)))).unwrap();
        recover(&engine, &c);
        assert_eq!(state(c.id).state, ClipState::Scheduled);
        assert_eq!(with_db(&engine, |db| db.upload(c.id)).unwrap(), Some((place.to_string(), 1234)));
        // Google cannot say (or no longer knows the address): failed, with a word to look first.
        for s in [404, 500] {
            up.lock().unwrap().status_answer = Some(s);
            let c = with_db(&engine, |db| Ok(sample_clip(db, vod, "/c.mp4"))).unwrap();
            with_db(&engine, |db| db.set_upload(c.id, Some((place, 1234)))).unwrap();
            recover(&engine, &c);
            let got = state(c.id);
            assert_eq!(got.state, ClipState::Failed);
            assert!(got.error.as_deref().is_some_and(|e| e.starts_with("Upload interrupted") && e.contains("Look on your channel")), "{:?}", got.error);
        }
        // No upload opened: nothing reached Google, back to its slot.
        let c = with_db(&engine, |db| Ok(sample_clip(db, vod, "/c.mp4"))).unwrap();
        recover(&engine, &c);
        assert_eq!(state(c.id).state, ClipState::Scheduled);
        use_fake(None);
    }

    /// A slow line, or Google's own error after the bytes went: the upload goes on from what
    /// Google has (its `Range`), never starts over; a file written over since it was drawn is not
    /// sent; and a try after a failure asks about the upload it opened before opening another.
    #[test]
    fn an_upload_goes_on_from_where_google_has_it() {
        let _one = GOOGLE.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mp4");
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&file, &bytes).unwrap();
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let up = Arc::new(std::sync::Mutex::new(Upload { have: 0, total: 1000, take: 400, status_answer: None, puts: vec![] }));
        use_fake(Some(stand_in(up.clone())));
        let vod = with_db(&engine, |db| db.add_vod(1, "v", "/v.mp4", "", "", &[], |id| format!("/vods/{id}"), "t0")).unwrap().id;
        let path = file.display().to_string();
        let c = with_db(&engine, |db| Ok(drawn_clip(db, vod, &path, Some(&super::super::sha_hex(&bytes))))).unwrap();
        let place = "https://www.googleapis.com/upload/youtube/v3/videos?upload_id=u2";
        // The upload was opened by an earlier try (opening one needs the sign-in).
        with_db(&engine, |db| db.set_upload(c.id, Some((place, 1000)))).unwrap();
        let s = super::super::default_settings();
        assert_eq!(upload(&engine, &c, &s, Privacy::Private).unwrap(), "v9");
        let puts = up.lock().unwrap().puts.clone();
        assert_eq!(puts, vec![(None, 1000), (Some("bytes 400-999/1000".into()), 600), (Some("bytes 800-999/1000".into()), 200)]);
        // Its file written over since it was drawn: not sent.
        let c2 = with_db(&engine, |db| Ok(drawn_clip(db, vod, &path, Some("not-this-file")))).unwrap();
        let refused = upload(&engine, &c2, &s, Privacy::Private).unwrap_err();
        assert!(refused.message.contains("changed since it was drawn"), "{}", refused.message);
        // Google cannot say how the last try ended: refused, not sent again.
        up.lock().unwrap().status_answer = Some(500);
        let c3 = with_db(&engine, |db| Ok(sample_clip(db, vod, &path))).unwrap();
        with_db(&engine, |db| db.set_upload(c3.id, Some((place, 1000)))).unwrap();
        let before = up.lock().unwrap().puts.len();
        let refused = upload(&engine, &c3, &s, Privacy::Private).unwrap_err();
        assert!(refused.message.contains("may have reached YouTube"), "{}", refused.message);
        assert_eq!(up.lock().unwrap().puts.len(), before, "no byte sent");
        use_fake(None);
    }

    /// Sign-out revokes the sign-in at Google, and forgets it here even when Google says no.
    #[test]
    fn signing_out_revokes_at_google() {
        let _one = GOOGLE.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let store = crate::Store::open(&dir.path().join("relay.db"), false).unwrap();
        let engine = Engine::new(crate::Instance::Test, store);
        save(&engine, &Secrets { client_id: "c.apps.googleusercontent.com".into(), client_secret: "s".into(), refresh_token: Some("rt-9".into()) }).unwrap();
        let calls = Arc::new(std::sync::Mutex::new(Vec::<Call>::new()));
        let seen = calls.clone();
        use_fake(Some(Arc::new(move |call: &Call| {
            seen.lock().unwrap().push(call.clone());
            Answer { status: 400, body: b"{\"error\": \"invalid_token\"}".to_vec(), ..Default::default() }
        })));
        let out = disconnect(&engine).unwrap();
        assert!(out.configured && !out.connected);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!((calls[0].method, calls[0].url.as_str()), ("POST", REVOKE));
        assert_eq!(String::from_utf8_lossy(&calls[0].body), "token=rt-9");
        use_fake(None);
    }

    #[test]
    fn only_google_is_called() {
        assert!(google("https://oauth2.googleapis.com/token"));
        assert!(google(UPLOAD));
        assert!(!google("https://example.com/upload"));
        assert!(!google("http://www.googleapis.com.evil.test/"));
    }
}
