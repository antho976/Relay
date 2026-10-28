//! ElevenLabs for videos (`video.voice`, `video.sfx`, `video.music`, `video.transcribe`,
//! `video.voices`): voice lines with each word's timing, sound effects and music from a
//! description, and speech-to-text to hear whether a name was said right. What Relay makes goes
//! into the video's `public/assets/` with how it was made beside it (`<file>.json`: the text or
//! prompt, the voice, the model, the words' timings).
//!
//! The key is the person's, saved from the Design view (`video.elevenlabs.key_set`, never audited)
//! in the desktop keyring — a private file beside the store on the test instance — and read when a
//! call needs it; no answer, event, log or audit row carries it. Requests go to one host over
//! https, never follow a redirect, and each has a deadline and a cap on its answer. The engine
//! tests stand a fake in for the network ([`super::Hub::use_elevenlabs`]).

use super::assets;
use crate::engine::Engine;
use crate::pictures::SecretKey;
use crate::Instance;
use relay_bus::error::BusError;
use relay_bus::ops::video::{ElevenStatus, Misheard, VideoAsset, VoiceInfo, VoiceWord};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const HOST: &str = "api.elevenlabs.io";
const BASE: &str = "https://api.elevenlabs.io";
/// The test instance's key file, beside its store.
pub const TEST_FILE: &str = "video-elevenlabs.key";
const LABEL: &str = "--label=Relay Video: ElevenLabs key";
const ATTRIBUTES: [&str; 4] = ["application", "Relay", "purpose", "video-elevenlabs"];
const KEYRING_WITHIN: Duration = Duration::from_secs(20);
/// The most a sound may weigh, and a JSON answer (a line's audio comes base64 in one).
const MOST_AUDIO: u64 = 100 << 20;
const MOST_JSON: u64 = 40 << 20;
pub const TTS_MODEL: &str = "eleven_multilingual_v2";
pub const MUSIC_MODEL: &str = "music_v1";
const STT_MODELS: [&str; 2] = ["scribe_v2", "scribe_v1"];
/// Most a transcription sends: its sound alone, mono at 16 kHz, is about 22 MB an hour.
const MOST_UPLOAD: u64 = 200 << 20;

/// What is sent.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    None,
    Json(Value),
    /// Form fields and one file, as multipart/form-data.
    Form { fields: Vec<(String, String)>, file: (String, Arc<Vec<u8>>) },
}

/// One request to ElevenLabs.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub method: &'static str,
    /// From the host on: `/v1/sound-generation`.
    pub path: String,
    pub query: Vec<(&'static str, String)>,
    pub body: Body,
    pub deadline: Duration,
    pub cap: u64,
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// A stand-in for the network: the engine tests answer calls with it.
pub type Fake = Arc<dyn Fn(&Call) -> Answer + Send + Sync>;

#[derive(Default)]
pub struct State {
    key: Mutex<Option<SecretKey>>,
    /// Counts the keys saved and removed: a lookup that started before one keeps nothing it
    /// read, or a removed key would come back until restart.
    generation: std::sync::atomic::AtomicU64,
    /// When the keyring last said there is no key, or could not be read, and what it said: asked
    /// again only after [`ASK_AGAIN`], so a locked keyring is not asked (and an unlock prompt not
    /// raised) by every call.
    missed: Mutex<Option<(std::time::Instant, BusError)>>,
    pub(super) fake: Mutex<Option<Fake>>,
}

/// How long a keyring's "no key" or failure is taken as its answer.
const ASK_AGAIN: Duration = Duration::from_secs(60);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------- the key

/// Where an engine keeps the key: nowhere for a store in memory, a 0600 file on the test
/// instance, else the desktop keyring through `secret-tool`.
fn keyring(engine: &Engine) -> Option<Result<PathBuf, ()>> {
    let store = engine.store.path();
    if store == Path::new(":memory:") {
        None
    } else if engine.instance == Instance::Test {
        Some(Ok(store.with_file_name(TEST_FILE)))
    } else {
        Some(Err(()))
    }
}

fn load(engine: &Engine) -> Result<Option<SecretKey>, BusError> {
    match keyring(engine) {
        None => Ok(None),
        Some(Ok(file)) => Ok(std::fs::read_to_string(file).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(SecretKey::new)),
        Some(Err(())) => {
            let out = crate::proc::output_with_timeout(Command::new("secret-tool").arg("lookup").args(ATTRIBUTES), KEYRING_WITHIN)
                .map_err(|e| BusError::unavailable("video.elevenlabs.keyring", format!("secret-tool: {e}")).with_hint("install libsecret (secret-tool)"))?
                .ok_or_else(|| BusError::unavailable("video.elevenlabs.keyring", "the desktop keyring did not answer").with_hint("unlock the desktop keyring"))?;
            if out.status.success() {
                let key = String::from_utf8_lossy(&out.stdout).trim().to_string();
                return Ok((!key.is_empty()).then(|| SecretKey::new(key)));
            }
            if out.stderr.iter().all(u8::is_ascii_whitespace) {
                return Ok(None);
            }
            Err(BusError::unavailable("video.elevenlabs.keyring", "the desktop keyring could not be read").with_hint("unlock the desktop keyring"))
        }
    }
}

/// The saved key, read once and kept; refused when none is saved.
fn key(engine: &Engine) -> Result<SecretKey, BusError> {
    use std::sync::atomic::Ordering;
    let state = &engine.video.eleven;
    if let Some(key) = lock(&state.key).clone() {
        return Ok(key);
    }
    if let Some((at, why)) = lock(&state.missed).as_ref() {
        if at.elapsed() < ASK_AGAIN {
            return Err(why.clone());
        }
    }
    let generation = state.generation.load(Ordering::SeqCst);
    let found = load(engine).and_then(|key| key.ok_or_else(|| BusError::unavailable("video.elevenlabs.no_key", "no ElevenLabs key is saved")
        .with_hint("ask the person to save their ElevenLabs API key in the Design view (Videos, ElevenLabs); never ask for the key itself")));
    // Saved or removed while it looked: what it read is already out of date.
    if state.generation.load(Ordering::SeqCst) != generation {
        return lock(&state.key).clone().ok_or_else(|| BusError::unavailable("video.elevenlabs.no_key", "no ElevenLabs key is saved"));
    }
    match found {
        Ok(key) => {
            *lock(&state.key) = Some(key.clone());
            Ok(key)
        }
        Err(why) => {
            *lock(&state.missed) = Some((std::time::Instant::now(), why.clone()));
            Err(why)
        }
    }
}

/// A key was saved or removed: lookups already running keep nothing, and the keyring is asked
/// again.
fn changed(engine: &Engine, key: Option<SecretKey>) {
    let state = &engine.video.eleven;
    state.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    *lock(&state.missed) = None;
    *lock(&state.key) = key;
}

/// The key as pasted, checked for shape only: printable ASCII with no spaces.
pub fn check_format(raw: &str) -> Result<SecretKey, BusError> {
    let key = raw.trim();
    let bad = |why: &str| BusError::invalid("video.elevenlabs.key_format", format!("that key {why}"));
    if key.is_empty() {
        return Err(bad("is empty"));
    }
    if key.len() > 200 || !key.is_ascii() || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad("is not an ElevenLabs key (printable letters and digits, no spaces)"));
    }
    Ok(SecretKey::new(key))
}

/// Check `key` with ElevenLabs (its subscription) and keep it: the status it gives.
pub fn save(engine: &Engine, key: SecretKey) -> Result<ElevenStatus, BusError> {
    let status = subscription(engine, &key)?;
    match keyring(engine) {
        None => return Err(BusError::unavailable("video.elevenlabs.keyring", "this store keeps no keys")),
        Some(Ok(file)) => {
            write_private(&file, key.expose()).map_err(|e| BusError::unavailable("video.elevenlabs.keyring", format!("{}: {e}", file.display())))?;
        }
        Some(Err(())) => {
            let mut cmd = Command::new("secret-tool");
            cmd.arg("store").arg(LABEL).args(ATTRIBUTES);
            let out = crate::proc::output_with_input(&mut cmd, key.expose().as_bytes(), KEYRING_WITHIN)
                .map_err(|e| BusError::unavailable("video.elevenlabs.keyring", format!("secret-tool: {e}")))?
                .ok_or_else(|| BusError::unavailable("video.elevenlabs.keyring", "the desktop keyring did not answer"))?;
            if !out.status.success() {
                return Err(BusError::unavailable("video.elevenlabs.keyring", "the desktop keyring refused to keep the key").with_hint("unlock the desktop keyring"));
            }
        }
    }
    changed(engine, Some(key));
    Ok(status)
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(text.as_bytes())
}

/// Forget the key.
pub fn remove(engine: &Engine) -> Result<ElevenStatus, BusError> {
    changed(engine, None);
    match keyring(engine) {
        None => {}
        Some(Ok(file)) => {
            let _ = std::fs::remove_file(file);
        }
        Some(Err(())) => {
            let out = crate::proc::output_with_timeout(Command::new("secret-tool").arg("clear").args(ATTRIBUTES), KEYRING_WITHIN)
                .map_err(|e| BusError::unavailable("video.elevenlabs.keyring", format!("secret-tool: {e}")))?;
            if out.is_none() {
                return Err(BusError::unavailable("video.elevenlabs.keyring", "the desktop keyring did not answer"));
            }
        }
    }
    // Again once it is gone: a lookup that began while it was being cleared read it still.
    changed(engine, None);
    Ok(ElevenStatus { saved: false, tier: None, characters_left: None })
}

/// Whether a key is saved, and what it has left when one is (asked of ElevenLabs).
pub fn status(engine: &Engine) -> Result<ElevenStatus, BusError> {
    match key(engine) {
        Ok(key) => subscription(engine, &key).or(Ok(ElevenStatus { saved: true, tier: None, characters_left: None })),
        Err(e) if e.code == "video.elevenlabs.no_key" => Ok(ElevenStatus { saved: false, tier: None, characters_left: None }),
        Err(e) => Err(e),
    }
}

fn subscription(engine: &Engine, key: &SecretKey) -> Result<ElevenStatus, BusError> {
    let answer = send_with(engine, key, &Call {
        method: "GET", path: "/v1/user/subscription".into(), query: Vec::new(), body: Body::None, deadline: Duration::from_secs(15), cap: 1 << 20,
    })?;
    let v: Value = serde_json::from_slice(&answer.body).unwrap_or_default();
    let left = match (v["character_limit"].as_i64(), v["character_count"].as_i64()) {
        (Some(limit), Some(used)) => Some(limit - used),
        _ => None,
    };
    Ok(ElevenStatus { saved: true, tier: v["tier"].as_str().map(str::to_string), characters_left: left })
}

// ---------------------------------------------------------------- the network

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_connect(Some(Duration::from_secs(6)))
            .user_agent("relay-video")
            .build()
            .into()
    })
}

fn send(engine: &Engine, call: &Call) -> Result<Answer, BusError> {
    let key = key(engine)?;
    send_with(engine, &key, call)
}

fn send_with(engine: &Engine, key: &SecretKey, call: &Call) -> Result<Answer, BusError> {
    let fake = lock(&engine.video.eleven.fake).clone();
    let answer = match fake {
        Some(fake) => fake(call),
        None => real(key, call)?,
    };
    match answer.status {
        200..=299 => Ok(answer),
        401 => Err(BusError::unavailable("video.elevenlabs.key", "ElevenLabs refused the saved key")
            .with_hint("ask the person to save a working key in the Design view (Videos, ElevenLabs)")),
        402 => Err(BusError::unavailable("video.elevenlabs.quota", format!("ElevenLabs: {}", detail(&answer.body)))
            .with_hint("the account is out of credits or its plan lacks this: tell the person")),
        429 => Err(BusError::unavailable("video.elevenlabs.busy", format!("ElevenLabs is limiting requests: {}", detail(&answer.body)))
            .with_hint("wait a minute and try again, one request at a time")),
        400 | 404 | 422 => {
            let why = detail(&answer.body);
            let code = if why.contains("quota") { "video.elevenlabs.quota" } else { "video.elevenlabs.invalid" };
            Err(BusError::invalid(code, format!("ElevenLabs: {why}")))
        }
        status => Err(BusError::unavailable("video.elevenlabs", format!("ElevenLabs answered {status}: {}", detail(&answer.body)))),
    }
}

/// What a refusal says: `detail` (a message, or a list of them), else the body's start.
fn detail(body: &[u8]) -> String {
    let v: Value = serde_json::from_slice(body).unwrap_or_default();
    let d = &v["detail"];
    let text = if let Some(s) = d.as_str() {
        s.to_string()
    } else if let Some(m) = d["message"].as_str() {
        m.to_string()
    } else if let Some(list) = d.as_array() {
        list.iter().filter_map(|e| e["msg"].as_str().or(e["message"].as_str())).collect::<Vec<_>>().join("; ")
    } else {
        String::from_utf8_lossy(body).chars().take(200).collect()
    };
    text.chars().take(400).collect()
}

fn real(key: &SecretKey, call: &Call) -> Result<Answer, BusError> {
    let url = format!("{BASE}{}", call.path);
    let lost = |e: ureq::Error| match e {
        ureq::Error::Timeout(_) => BusError::unavailable("video.elevenlabs.timeout", format!("ElevenLabs did not answer within {} s", call.deadline.as_secs())),
        ureq::Error::BodyExceedsLimit(_) => BusError::unavailable("video.elevenlabs", "ElevenLabs answered with more than Relay takes"),
        e => {
            tracing::debug!(error = %e, "elevenlabs: a request got no answer");
            BusError::unavailable("video.elevenlabs.offline", "ElevenLabs could not be reached").with_hint("check the network, then try again")
        }
    };
    let query = |mut b: ureq::RequestBuilder<ureq::typestate::WithBody>| {
        for (k, v) in &call.query {
            b = b.query(*k, v);
        }
        b.header("xi-api-key", key.expose()).config().timeout_global(Some(call.deadline)).build()
    };
    let mut response = match &call.body {
        Body::None => {
            let mut b = agent().get(&url);
            for (k, v) in &call.query {
                b = b.query(*k, v);
            }
            b.header("xi-api-key", key.expose()).config().timeout_global(Some(call.deadline)).build().call().map_err(lost)?
        }
        Body::Json(v) => query(agent().post(&url)).header("content-type", "application/json").send(v.to_string().as_bytes()).map_err(lost)?,
        Body::Form { fields, file } => {
            let boundary = format!("relay-{}", uuid::Uuid::new_v4().simple());
            let mut body = Vec::new();
            for (name, value) in fields {
                body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
            }
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n", file.0).as_bytes());
            body.extend_from_slice(&file.1);
            body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            query(agent().post(&url)).header("content-type", format!("multipart/form-data; boundary={boundary}")).send(&body[..]).map_err(lost)?
        }
    };
    let status = response.status().as_u16();
    let body = response.body_mut().with_config().limit(call.cap.saturating_add(1)).read_to_vec().map_err(lost)?;
    if body.len() as u64 > call.cap {
        return Err(BusError::unavailable("video.elevenlabs", "ElevenLabs answered with more than Relay takes"));
    }
    Ok(Answer { status, body })
}

// ---------------------------------------------------------------- what it makes

/// A file name from words: `vo-every-day-you`, at most four words.
pub fn name_from(prefix: &str, words: &str) -> String {
    let slug: Vec<String> = words.split_whitespace().take(4)
        .map(|w| w.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase())
        .filter(|w| !w.is_empty()).collect();
    if slug.is_empty() { prefix.to_string() } else { format!("{prefix}-{}", slug.join("-")) }
}

/// `bytes` into the video at `dir` as `assets/<name>.<ext>`, with how it was made beside it.
pub fn keep(dir: &Path, bytes: &[u8], name: &str, ext: &str, overwrite: bool, made: Value) -> Result<VideoAsset, BusError> {
    let tmp = dir.join("public").join(format!(".made-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp).map_err(|e| BusError::unavailable("video.folder", format!("{}: {e}", tmp.display())))?;
    let file = tmp.join(format!("{}.{ext}", assets::safe_name(name).trim_end_matches(&format!(".{ext}"))));
    let added = std::fs::write(&file, bytes).map_err(|e| BusError::unavailable("video.asset", format!("{}: {e}", file.display())))
        .and_then(|_| assets::add(dir, &assets::Add { source: &file, folder: "assets", name: None, overwrite, link: false }));
    let _ = std::fs::remove_dir_all(&tmp);
    let asset = added?;
    let at = PathBuf::from(&asset.path);
    let _ = std::fs::write(assets::sidecar(&at), serde_json::to_vec_pretty(&made).unwrap_or_default());
    Ok(assets::describe(dir, &at, asset.renamed_from, asset.replaced, false))
}

/// `text` as it is sent to be said, each word in `say` (`{"Avex": "Ay-vex"}`) replaced, and for
/// each written word how many spoken words stand for it.
pub fn spoken(text: &str, say: &Map<String, Value>) -> (String, Vec<usize>) {
    let mut out = Vec::new();
    let mut counts = Vec::new();
    for word in text.split_whitespace() {
        let start = word.find(|c: char| c.is_alphanumeric()).unwrap_or(word.len());
        let end = word.rfind(|c: char| c.is_alphanumeric()).map_or(start, |i| i + word[i..].chars().next().map_or(1, char::len_utf8));
        let core = &word[start..end.max(start)];
        let alias = say.iter().find(|(k, _)| k.eq_ignore_ascii_case(core)).and_then(|(_, v)| v.as_str());
        match alias {
            Some(alias) => {
                let said = format!("{}{alias}{}", &word[..start], &word[end.max(start)..]);
                counts.push(said.split_whitespace().count().max(1));
                out.push(said);
            }
            None => {
                counts.push(1);
                out.push(word.to_string());
            }
        }
    }
    (out.join(" "), counts)
}

/// Words from ElevenLabs' character alignment: each run of non-space characters, from its first
/// character's start to its last's end.
pub fn words_of(alignment: &Value) -> Vec<VoiceWord> {
    let chars: Vec<&str> = alignment["characters"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let starts: Vec<f64> = alignment["character_start_times_seconds"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let ends: Vec<f64> = alignment["character_end_times_seconds"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let mut words = Vec::new();
    let mut current: Option<VoiceWord> = None;
    for (i, c) in chars.iter().enumerate() {
        let (s, e) = (starts.get(i).copied().unwrap_or(0.0), ends.get(i).copied().unwrap_or(0.0));
        if c.trim().is_empty() {
            if let Some(w) = current.take() {
                words.push(w);
            }
            continue;
        }
        match current.as_mut() {
            Some(w) => {
                w.text.push_str(c);
                w.end = e;
            }
            None => current = Some(VoiceWord { text: c.to_string(), start: s, end: e }),
        }
    }
    words.extend(current);
    for w in &mut words {
        w.start = (w.start * 1000.0).round() / 1000.0;
        w.end = (w.end * 1000.0).round() / 1000.0;
    }
    words
}

/// The written words with the timings of the spoken ones that stand for them; the spoken words
/// as they are when they do not line up.
pub fn written(text: &str, counts: &[usize], spoken: Vec<VoiceWord>) -> Vec<VoiceWord> {
    let original: Vec<&str> = text.split_whitespace().collect();
    if counts.iter().sum::<usize>() != spoken.len() || original.len() != counts.len() {
        return spoken;
    }
    let mut out = Vec::with_capacity(original.len());
    let mut at = 0;
    for (word, n) in original.iter().zip(counts) {
        let (first, last) = (&spoken[at], &spoken[at + n - 1]);
        out.push(VoiceWord { text: word.to_string(), start: first.start, end: last.end });
        at += n;
    }
    out
}

/// Remotion's `Caption[]` for words, ms from the line's start.
pub fn captions(words: &[VoiceWord]) -> Value {
    Value::Array(words.iter().enumerate().map(|(i, w)| json!({
        "text": if i == 0 { w.text.clone() } else { format!(" {}", w.text) },
        "startMs": (w.start * 1000.0).round(),
        "endMs": (w.end * 1000.0).round(),
        "timestampMs": ((w.start + w.end) * 500.0).round(),
        "confidence": null,
    })).collect())
}

fn norm(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric() || *c == '\'').collect::<String>().to_lowercase()
}

/// Where `heard` differs from `expected`, word by word (an edit distance's alignment): each
/// written word heard as another, or not heard.
pub fn misheard(expected: &str, heard: &[VoiceWord]) -> Vec<Misheard> {
    let want: Vec<String> = expected.split_whitespace().map(norm).filter(|w| !w.is_empty()).collect();
    let shown: Vec<&str> = expected.split_whitespace().filter(|w| !norm(w).is_empty()).collect();
    let got: Vec<String> = heard.iter().map(|w| norm(&w.text)).collect();
    let (n, m) = (want.len(), got.len());
    let mut cost = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in cost.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, c) in cost[0].iter_mut().enumerate() {
        *c = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let same = usize::from(want[i - 1] != got[j - 1]);
            cost[i][j] = (cost[i - 1][j - 1] + same).min(cost[i - 1][j] + 1).min(cost[i][j - 1] + 1);
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && cost[i][j] == cost[i - 1][j - 1] + usize::from(want[i - 1] != got[j - 1]) {
            if want[i - 1] != got[j - 1] {
                out.push(Misheard { expected: shown[i - 1].to_string(), heard: Some(heard[j - 1].text.clone()), at: Some(heard[j - 1].start) });
            }
            i -= 1;
            j -= 1;
        } else if i > 0 && cost[i][j] == cost[i - 1][j] + 1 {
            out.push(Misheard { expected: shown[i - 1].to_string(), heard: None, at: (j > 0).then(|| heard[j - 1].end) });
            i -= 1;
        } else {
            j -= 1;
        }
    }
    out.reverse();
    out
}

/// Text to speech with each character's timing: the audio (MP3) and the alignment.
pub fn speak(engine: &Engine, voice_id: &str, text: &str, model: &str, settings: Value) -> Result<(Vec<u8>, Value), BusError> {
    let answer = send(engine, &Call {
        method: "POST", path: format!("/v1/text-to-speech/{voice_id}/with-timestamps"), query: vec![("output_format", "mp3_44100_128".into())],
        body: Body::Json(json!({"text": text, "model_id": model, "voice_settings": settings})), deadline: Duration::from_secs(120), cap: MOST_JSON,
    })?;
    let v: Value = serde_json::from_slice(&answer.body).map_err(|e| BusError::unavailable("video.elevenlabs", format!("ElevenLabs' answer: {e}")))?;
    use base64::Engine as _;
    let audio = base64::engine::general_purpose::STANDARD.decode(v["audio_base64"].as_str().unwrap_or_default())
        .map_err(|e| BusError::unavailable("video.elevenlabs", format!("ElevenLabs' audio: {e}")))?;
    let alignment = if v["alignment"].is_object() { v["alignment"].clone() } else { v["normalized_alignment"].clone() };
    Ok((audio, alignment))
}

/// The voices the key can use: id, name, category and a line of description.
pub fn voices(engine: &Engine) -> Result<Vec<VoiceInfo>, BusError> {
    let answer = send(engine, &Call { method: "GET", path: "/v1/voices".into(), query: Vec::new(), body: Body::None, deadline: Duration::from_secs(20), cap: 8 << 20 })?;
    let v: Value = serde_json::from_slice(&answer.body).unwrap_or_default();
    Ok(v["voices"].as_array().into_iter().flatten().filter_map(|voice| {
        let labels = voice["labels"].as_object().map(|l| l.values().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty());
        let description = match (voice["description"].as_str().filter(|d| !d.is_empty()), labels) {
            (Some(d), Some(l)) => Some(format!("{l}. {d}")),
            (Some(d), None) => Some(d.to_string()),
            (None, l) => l,
        };
        Some(VoiceInfo { voice_id: voice["voice_id"].as_str()?.to_string(), name: voice["name"].as_str()?.to_string(),
            category: voice["category"].as_str().map(str::to_string), description })
    }).collect())
}

/// A voice by id, or by name among the key's voices.
pub fn voice_id(engine: &Engine, voice: &str) -> Result<(String, String), BusError> {
    let all = voices(engine)?;
    if let Some(found) = all.iter().find(|v| v.voice_id == voice || v.name.eq_ignore_ascii_case(voice)) {
        return Ok((found.voice_id.clone(), found.name.clone()));
    }
    let named: Vec<&str> = all.iter().map(|v| v.name.as_str()).take(30).collect();
    Err(BusError::invalid("video.elevenlabs.voice", format!("no voice {voice:?}")).with_details(json!({"voices": named}))
        .with_hint("video.voices lists them"))
}

/// A sound effect from a description (MP3).
pub fn sound_effect(engine: &Engine, prompt: &str, seconds: Option<f64>, influence: Option<f64>, looped: bool) -> Result<Vec<u8>, BusError> {
    let mut body = json!({"text": prompt, "loop": looped});
    if let Some(s) = seconds {
        body["duration_seconds"] = json!(s);
    }
    if let Some(i) = influence {
        body["prompt_influence"] = json!(i);
    }
    Ok(send(engine, &Call {
        method: "POST", path: "/v1/sound-generation".into(), query: vec![("output_format", "mp3_44100_128".into())], body: Body::Json(body),
        deadline: Duration::from_secs(120), cap: MOST_AUDIO,
    })?.body)
}

/// Music from a description, `seconds` long (MP3).
pub fn compose(engine: &Engine, prompt: &str, seconds: f64, instrumental: bool, model: &str) -> Result<Vec<u8>, BusError> {
    Ok(send(engine, &Call {
        method: "POST", path: "/v1/music".into(), query: Vec::new(),
        body: Body::Json(json!({"prompt": prompt, "music_length_ms": (seconds * 1000.0).round() as i64, "force_instrumental": instrumental, "model_id": model})),
        deadline: Duration::from_secs(600), cap: MOST_AUDIO,
    })?.body)
}

/// Speech to text with each word's timing: the text, its language, and the words.
pub fn hear(engine: &Engine, file: &Path, language: Option<&str>) -> Result<(String, String, Vec<VoiceWord>), BusError> {
    let (name, bytes) = for_hearing(engine, file)?;
    let mut last = None;
    for model in STT_MODELS {
        let mut fields = vec![("model_id".to_string(), model.to_string()), ("timestamps_granularity".to_string(), "word".to_string())];
        if let Some(l) = language {
            fields.push(("language_code".into(), l.into()));
        }
        let call = Call { method: "POST", path: "/v1/speech-to-text".into(), query: Vec::new(),
            body: Body::Form { fields, file: (name.clone(), bytes.clone()) }, deadline: Duration::from_secs(300), cap: MOST_JSON };
        match send(engine, &call) {
            Ok(answer) => {
                let v: Value = serde_json::from_slice(&answer.body).map_err(|e| BusError::unavailable("video.elevenlabs", format!("ElevenLabs' answer: {e}")))?;
                let words = v["words"].as_array().into_iter().flatten().filter(|w| w["type"].as_str().unwrap_or("word") == "word")
                    .map(|w| VoiceWord { text: w["text"].as_str().unwrap_or_default().to_string(), start: w["start"].as_f64().unwrap_or(0.0), end: w["end"].as_f64().unwrap_or(0.0) })
                    .collect();
                return Ok((v["text"].as_str().unwrap_or_default().to_string(), v["language_code"].as_str().unwrap_or_default().to_string(), words));
            }
            // A model the account has no use of: the older one.
            Err(e) if e.code == "video.elevenlabs.invalid" => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| BusError::unavailable("video.elevenlabs", "no speech-to-text model answered")))
}

/// What a transcription of `file` sends: its sound alone, mono MP3 at 16 kHz (a render or a clip
/// of gigabytes is a few MB of it), written to a scratch file and read once; the file itself
/// when ffmpeg cannot read it so (none set up, or not media it knows), it is small enough, and
/// its name says sound or video. Anything else is refused, never uploaded: a key file or a
/// document ffmpeg cannot read is not what speech-to-text is for.
fn for_hearing(engine: &Engine, file: &Path) -> Result<(String, Arc<Vec<u8>>), BusError> {
    let stem = file.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "audio".into());
    let dir = engine.store.videos_dir().join(".relay/sound");
    let scratch = dir.join(format!(".hear-{}.mp3", uuid::Uuid::new_v4()));
    let made = std::fs::create_dir_all(&dir).is_ok() && super::media::ffmpeg(engine, &[std::ffi::OsStr::new("-i"), file.as_os_str(),
        "-vn".as_ref(), "-sn".as_ref(), "-dn".as_ref(), "-ac".as_ref(), "1".as_ref(), "-ar".as_ref(), "16000".as_ref(),
        "-c:a".as_ref(), "libmp3lame".as_ref(), "-b:a".as_ref(), "48k".as_ref(), scratch.as_os_str()], "taking the sound out").is_ok();
    let (path, name) = if made { (scratch.as_path(), format!("{stem}.mp3")) } else {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !matches!(assets::kind(&name), "audio" | "video") {
            let _ = std::fs::remove_file(&scratch);
            return Err(BusError::invalid("video.asset", format!("{} is not sound or video that could be read; nothing was sent", file.display()))
                .with_hint("transcribe an audio or video file, or a render"));
        }
        (file, name)
    };
    let read = (|| {
        let size = std::fs::metadata(path).map_err(|e| BusError::invalid("video.asset", format!("{}: {e}", file.display())))?.len();
        if size > MOST_UPLOAD {
            return Err(BusError::invalid("video.asset", format!("{} holds {} MB of sound to send; at most {} MB", file.display(), size >> 20, MOST_UPLOAD >> 20))
                .with_hint("transcribe a shorter clip, or a render of part of it"));
        }
        std::fs::read(path).map_err(|e| BusError::invalid("video.asset", format!("{}: {e}", file.display())))
    })();
    let _ = std::fs::remove_file(&scratch);
    Ok((name, Arc::new(read?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file ffmpeg cannot read is sent as it is only when its name says sound or video: a key
    /// file named as an asset never leaves the machine.
    #[test]
    fn only_sound_or_video_is_sent_when_ffmpeg_cannot_read_it() {
        let engine = Engine::new(Instance::Test, crate::Store::open_memory().unwrap());
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("id_ed25519");
        std::fs::write(&key, "-----BEGIN OPENSSH PRIVATE KEY-----\nnot sound\n").unwrap();
        let refused = for_hearing(&engine, &key).map(|_| ()).unwrap_err();
        assert_eq!(refused.code, "video.asset");
        assert!(refused.message.contains("nothing was sent"), "{}", refused.message);
        // A recording ffmpeg cannot open (or no ffmpeg at all) still goes as it is.
        let clip = dir.path().join("memo.m4a");
        std::fs::write(&clip, b"not media ffmpeg knows").unwrap();
        let (name, bytes) = for_hearing(&engine, &clip).unwrap();
        assert_eq!((name.as_str(), bytes.as_slice()), ("memo.m4a", &b"not media ffmpeg knows"[..]));
    }

    #[test]
    fn words_come_from_the_character_alignment() {
        let alignment = json!({
            "characters": ["H", "i", " ", "A", "y", "-", "v", "e", "x", "."],
            "character_start_times_seconds": [0.0, 0.1, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55],
            "character_end_times_seconds": [0.1, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6],
        });
        let words = words_of(&alignment);
        assert_eq!(words, [VoiceWord { text: "Hi".into(), start: 0.0, end: 0.2 }, VoiceWord { text: "Ay-vex.".into(), start: 0.25, end: 0.6 }]);
        // Said as "Ay-vex", written and timed as "Avex".
        let say: Map<String, Value> = serde_json::from_value(json!({"avex": "Ay-vex"})).unwrap();
        let (said, counts) = spoken("Hi Avex.", &say);
        assert_eq!((said.as_str(), counts.as_slice()), ("Hi Ay-vex.", &[1, 1][..]));
        let back = written("Hi Avex.", &counts, words);
        assert_eq!(back[1], VoiceWord { text: "Avex.".into(), start: 0.25, end: 0.6 });
        let caps = captions(&back);
        assert_eq!(caps[1]["text"], " Avex.");
        assert_eq!((caps[1]["startMs"].as_f64(), caps[1]["endMs"].as_f64(), caps[1]["timestampMs"].as_f64()), (Some(250.0), Some(600.0), Some(425.0)));
    }

    #[test]
    fn an_alias_of_two_words_times_the_one_written() {
        let say: Map<String, Value> = serde_json::from_value(json!({"GIF": "jiff file"})).unwrap();
        let (said, counts) = spoken("Save the GIF now", &say);
        assert_eq!(said, "Save the jiff file now");
        let spoken_words: Vec<VoiceWord> = ["Save", "the", "jiff", "file", "now"].iter().enumerate()
            .map(|(i, w)| VoiceWord { text: (*w).into(), start: i as f64, end: i as f64 + 0.5 }).collect();
        let back = written("Save the GIF now", &counts, spoken_words);
        assert_eq!(back[2], VoiceWord { text: "GIF".into(), start: 2.0, end: 3.5 });
    }

    #[test]
    fn a_name_heard_wrong_is_found() {
        let heard: Vec<VoiceWord> = ["Meet", "Avox", "one", "point", "oh"].iter().enumerate()
            .map(|(i, w)| VoiceWord { text: (*w).into(), start: i as f64 * 0.4, end: i as f64 * 0.4 + 0.3 }).collect();
        let found = misheard("Meet Avex one point oh.", &heard);
        assert_eq!(found, [Misheard { expected: "Avex".into(), heard: Some("Avox".into()), at: Some(0.4) }]);
        let dropped = misheard("Meet the new Avex", &heard[..2]);
        assert!(dropped.iter().any(|m| m.heard.is_none()), "{dropped:?}");
    }

    #[test]
    fn names_come_from_the_words() {
        assert_eq!(name_from("vo", "Every day you trained, at a glance."), "vo-every-day-you-trained");
        assert_eq!(name_from("sfx", "!!!"), "sfx");
        assert!(check_format("  sk_abc123  ").is_ok());
        assert_eq!(check_format("a key with spaces").unwrap_err().code, "video.elevenlabs.key_format");
        assert_eq!(detail(br#"{"detail":{"status":"quota_exceeded","message":"You have 0 credits"}}"#), "You have 0 credits");
    }
}
