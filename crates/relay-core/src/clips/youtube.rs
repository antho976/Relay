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

fn save(engine: &Engine, s: &Secrets) -> Result<(), BusError> {
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
    let body = response.body_mut().with_config().limit(8 << 20).read_to_vec().map_err(lost)?;
    Ok(Answer { status, body, location })
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

pub fn status(engine: &Engine) -> YoutubeStatus {
    let secrets = load(engine);
    let c = channel(engine);
    let pending = lock(&engine.clips.oauth).as_ref().is_some_and(|p| p.started.elapsed() < SIGN_IN_WITHIN);
    match secrets {
        Ok(s) => YoutubeStatus {
            configured: s.as_ref().is_some_and(|s| !s.client_id.is_empty()),
            connected: s.as_ref().is_some_and(|s| s.refresh_token.is_some()),
            channel: c.channel,
            channel_id: c.channel_id,
            pending,
            note: c.note,
        },
        Err(e) => YoutubeStatus { pending, note: Some(e.message), ..Default::default() },
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
    *lock(&engine.clips.oauth) = Some(Pending { port, state: state.clone(), started: Instant::now() });
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new().name("clips-sign-in".into()).spawn(move || {
        let outcome = wait_for_code(&listener, &state);
        let Some(engine) = weak.upgrade() else { return };
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
        set_channel(&engine, &c);
        *lock(&engine.clips.oauth) = None;
        changed(&engine, "youtube", None, None);
        super::poke_poster(&engine);
    }).map_err(|e| BusError::internal(e.to_string()))?;
    Ok(url)
}

/// Wait for the browser to come back with `?code=…&state=…`; thank it either way.
fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String, BusError> {
    listener.set_nonblocking(true).map_err(|e| BusError::internal(e.to_string()))?;
    let deadline = Instant::now() + SIGN_IN_WITHIN;
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut line = String::new();
                let _ = BufReader::new(&mut stream).read_line(&mut line);
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let query = target.split_once('?').map_or("", |(_, q)| q);
                let param = |k: &str| query.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| super::percent(v)));
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

pub fn disconnect(engine: &Engine) -> Result<YoutubeStatus, BusError> {
    if let Some(mut s) = load(engine)? {
        s.refresh_token = None;
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
    if !out.to_ascii_lowercase().contains("#shorts") {
        out.push_str("\n\n#shorts");
    }
    out.chars().take(4900).collect()
}

/// Upload clip `c`'s file: its YouTube id.
pub fn upload(engine: &Engine, c: &Clip, s: &Settings, privacy: Privacy) -> Result<String, BusError> {
    let file = c.file.as_deref().ok_or_else(|| BusError::unavailable("clips.unrendered", "the clip has not been drawn yet"))?;
    let bytes = std::fs::read(file).map_err(|e| BusError::unavailable("clips.file", format!("{file}: {e}")))?;
    if bytes.len() as u64 > MOST_UPLOAD {
        return Err(BusError::invalid("clips.file", "the clip's file is over 2 GB"));
    }
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
            ("x-upload-content-length".into(), bytes.len().to_string()),
        ],
        body: meta.to_string().into_bytes(),
    }, Duration::from_secs(30))?;
    if start.status != 200 {
        return Err(BusError::unavailable("clips.youtube.upload", google_error(&start)));
    }
    let place = start.location.ok_or_else(|| BusError::unavailable("clips.youtube.upload", "Google gave no upload address"))?;
    let secs = (bytes.len() as u64 / (256 * 1024)).clamp(60, 3600);
    let done = send(Call { method: "PUT", url: place, headers: vec![("content-type".into(), "video/mp4".into())], body: bytes }, Duration::from_secs(secs))?;
    if !(200..300).contains(&done.status) {
        return Err(BusError::unavailable("clips.youtube.upload", google_error(&done)));
    }
    let v: Value = serde_json::from_slice(&done.body).unwrap_or_default();
    v["id"].as_str().map(str::to_string).ok_or_else(|| BusError::unavailable("clips.youtube.upload", "YouTube answered no video id"))
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
        if let Err(e) = tick(&engine) {
            tracing::debug!(error = %e.message, "clips: the poster could not look");
        }
        let wake = engine.clips.post_wake.clone();
        drop(engine);
        super::sleep_on(&wake, Duration::from_secs(60));
    }
}

fn tick(engine: &Engine) -> Result<(), BusError> {
    let s = super::settings(engine)?;
    let now = jiff::Timestamp::now();
    let t = crate::time::stamp(now);
    // On its own: clips that score enough are approved as the person would.
    if s.posting.mode == PostingMode::Auto && status(engine).connected {
        for c in with_db(engine, |db| db.clips_in(&[ClipState::Ready]))? {
            if c.score >= s.posting.min_score.max(1) {
                with_db(engine, |db| db.set_clip_state(c.id, ClipState::Approved, None, &t))?;
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
        taken.push(at);
        let privacy = c.privacy.unwrap_or(s.posting.privacy);
        with_db(engine, |db| db.schedule(c.id, Some(&crate::time::stamp(at)), Some(privacy), ClipState::Scheduled, &t))?;
        changed(engine, "clip", Some(c.vod_id), Some(c.id));
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
    let t = crate::time::now();
    with_db(engine, |db| db.set_clip_state(c.id, ClipState::Posting, None, &t))?;
    changed(engine, "clip", Some(c.vod_id), Some(c.id));
    let privacy = c.privacy.unwrap_or(s.posting.privacy);
    let result = upload(engine, c, s, privacy);
    let t = crate::time::now();
    match result {
        Ok(id) => with_db(engine, |db| db.set_posted(c.id, &id, &t))?,
        Err(e) => {
            tracing::warn!(clip = c.id, error = %e.message, "clips: an upload failed");
            with_db(engine, |db| db.set_clip_state(c.id, ClipState::Failed, Some(&format!("Upload failed: {}", e.message)), &t))?;
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
    }

    #[test]
    fn only_google_is_called() {
        assert!(google("https://oauth2.googleapis.com/token"));
        assert!(google(UPLOAD));
        assert!(!google("https://example.com/upload"));
        assert!(!google("http://www.googleapis.com.evil.test/"));
    }
}
