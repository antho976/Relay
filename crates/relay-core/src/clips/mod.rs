//! The Clips space in the engine (docs/CLIPS.md): VODs in, Shorts out. `clips.db` holds the VODs
//! and their clips; three threads of its own do the work, none of them inside a handler or with
//! the store's lock held while it waits:
//!
//! - the **pipeline** ([`pipeline`]) takes one VOD at a time through its stages: its sound, the
//!   words (whisper.cpp on the GPU), loudness, where the facecam is ([`frame`]), the moments the
//!   person asked for (Claude, through their own account: [`find`]), each moment cut and framed
//!   ([`edit`]), then drawn by the Video plugin's renderer;
//! - the **watcher** reads the engine's events and marks a clip drawn when its render finishes,
//!   whoever asked for it;
//! - the **poster** ([`youtube`]) sends approved clips to the person's YouTube channel at their
//!   slots, and holds the sign-in.
//!
//! Every stage keeps its files in the VOD's folder (`videos/p<project>/vods/<id>-<slug>/`), so a
//! VOD stopped, failed or cut off by a restart starts again at the stage it was in.

pub mod edit;
pub mod find;
pub mod frame;
pub mod pipeline;
pub mod store;
pub mod youtube;

use crate::engine::Engine;
use relay_bus::ops::clips::*;
use relay_bus::BusError;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use store::ClipsDb;

/// The engine's Clips: `clips.db`, opened on first use, and what its threads share.
#[derive(Default)]
pub struct Shelf {
    db: Mutex<Option<ClipsDb>>,
    /// Bumped to wake the pipeline: a VOD was added, retried or asked to look again.
    wake: Arc<(Mutex<u64>, Condvar)>,
    /// Bumped to wake the poster: a clip was approved or scheduled, or the account changed.
    post_wake: Arc<(Mutex<u64>, Condvar)>,
    started: OnceLock<()>,
    /// VODs the person asked to stop: the pipeline drops them at its next look.
    stopping: Mutex<HashSet<i64>>,
    /// The YouTube sign-in waiting for the browser.
    pub(crate) oauth: Mutex<Option<youtube::Pending>>,
    /// The access token and when it runs out.
    pub(crate) access: Mutex<Option<(String, Instant)>>,
    /// Tests only: every tool counted as installed.
    tools_present: std::sync::atomic::AtomicBool,
}

/// Count every tool Clips needs as installed: what the engine tests use, on a machine (CI)
/// without whisper.cpp or ffmpeg. Nothing runs them.
#[doc(hidden)]
pub fn assume_tools(engine: &Engine) {
    engine.clips.tools_present.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn sql(e: rusqlite::Error) -> BusError {
    BusError::internal(format!("clips.db: {e}"))
}

pub(crate) fn invalid(m: impl Into<String>) -> BusError {
    BusError::invalid("clips.invalid", m)
}

pub(crate) fn not_found(what: &str, id: i64) -> BusError {
    BusError::not_found("clips.not_found", format!("No {what} {id}"))
}

/// Runs `f` on `clips.db`, opening it beside the store on first use (in memory when the store
/// is, as in tests).
pub fn with_db<T>(engine: &Engine, f: impl FnOnce(&ClipsDb) -> rusqlite::Result<T>) -> Result<T, BusError> {
    let mut slot = lock(&engine.clips.db);
    if slot.is_none() {
        let store = engine.store.path();
        let opened = if store == Path::new(":memory:") {
            ClipsDb::open_in_memory()
        } else {
            let path = store.with_file_name("clips.db");
            ClipsDb::open(&path).inspect(|_| crate::private::private_db(&path))
        };
        *slot = Some(opened.map_err(sql)?);
    }
    f(slot.as_ref().expect("opened above")).map_err(sql)
}

pub fn vod(engine: &Engine, id: i64) -> Result<Vod, BusError> {
    with_db(engine, |db| db.vod(id))?.ok_or_else(|| not_found("VOD", id))
}

pub fn clip(engine: &Engine, id: i64) -> Result<Clip, BusError> {
    with_db(engine, |db| db.clip(id))?.ok_or_else(|| not_found("clip", id))
}

/// Say that something of Clips changed: the space reads again.
pub fn changed(engine: &Engine, what: &str, vod_id: Option<i64>, clip_id: Option<i64>) {
    engine.emit_system("clips.changed", json!({"what": what, "vod_id": vod_id, "clip_id": clip_id}));
}

// ---------------------------------------------------------------- settings

pub fn default_settings() -> Settings {
    Settings {
        streamer: String::new(),
        platform: "Twitch".into(),
        handle: String::new(),
        terms: String::new(),
        notes: String::new(),
        captions: CaptionStyle::Pixel,
        highlight: "#FFE14D".into(),
        end_card: true,
        min_s: 15,
        max_s: 58,
        max_clips: 10,
        model: "opus".into(),
        fps: 30,
        posting: Posting { mode: PostingMode::Ask, privacy: Privacy::Public, slots: vec!["17:00".into()], min_score: 8, footer: String::new() },
    }
}

pub fn settings(engine: &Engine) -> Result<Settings, BusError> {
    let raw = with_db(engine, |db| db.meta("settings"))?;
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_else(default_settings))
}

/// The settings as given, checked and tidied.
pub fn checked(mut s: Settings) -> Result<Settings, BusError> {
    let short = |field: &str, v: &str, max: usize| -> Result<String, BusError> {
        let t = v.trim();
        if t.chars().count() > max {
            return Err(invalid(format!("{field} is at most {max} characters")));
        }
        Ok(t.to_string())
    };
    s.streamer = short("streamer", &s.streamer, 60)?;
    s.platform = short("platform", &s.platform, 30)?;
    s.handle = short("handle", s.handle.trim_start_matches('@'), 60)?;
    s.terms = short("terms", &s.terms, 600)?;
    s.notes = short("notes", &s.notes, 2000)?;
    s.posting.footer = short("footer", &s.posting.footer, 1000)?;
    let hex = s.highlight.trim();
    if !(hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|c| c.is_ascii_hexdigit())) {
        return Err(invalid("highlight is a colour like #FFE14D"));
    }
    s.highlight = hex.to_ascii_uppercase();
    if !(5..=120).contains(&s.min_s) || !(s.min_s + 5..=MAX_CLIP_S as u32).contains(&s.max_s) {
        return Err(invalid(format!("a clip is 5 to {MAX_CLIP_S} seconds, the longest at least 5 more than the shortest")));
    }
    if !(1..=MAX_FIND).contains(&s.max_clips) {
        return Err(invalid(format!("1 to {MAX_FIND} clips a find")));
    }
    if !["opus", "sonnet", "haiku"].contains(&s.model.as_str()) && !s.model.starts_with("claude-") {
        return Err(invalid("the finder's model is opus, sonnet or haiku"));
    }
    if s.fps != 30 && s.fps != 60 {
        return Err(invalid("clips are drawn at 30 or 60 frames a second"));
    }
    if s.posting.min_score > 10 {
        return Err(invalid("a score is 1 to 10"));
    }
    let mut slots = Vec::new();
    for slot in &s.posting.slots {
        let t = slot.trim();
        let ok = t.split_once(':').is_some_and(|(h, m)| h.parse::<u8>().is_ok_and(|h| h < 24) && m.len() == 2 && m.parse::<u8>().is_ok_and(|m| m < 60));
        if !ok {
            return Err(invalid(format!("{t:?} is not a time of day like 17:00")));
        }
        let (h, m) = t.split_once(':').unwrap();
        slots.push(format!("{:02}:{m}", h.parse::<u8>().unwrap()));
    }
    slots.sort();
    slots.dedup();
    if slots.len() > 12 {
        return Err(invalid("at most 12 posting times a day"));
    }
    s.posting.slots = slots;
    Ok(s)
}

pub fn save_settings(engine: &Engine, s: &Settings) -> Result<(), BusError> {
    let raw = serde_json::to_string(s).map_err(|e| BusError::internal(e.to_string()))?;
    with_db(engine, |db| db.set_meta("settings", &raw))
}

// ---------------------------------------------------------------- places and tools

/// Where Clips keeps a project's VOD folders.
pub fn vods_dir(engine: &Engine, project_id: i64) -> PathBuf {
    crate::video::project_dir(&engine.store.videos_dir(), project_id).join("vods")
}

/// The stream clips script the kit carries (`plugins/video/runtime/clips/`), written where it
/// runs from if this build's copy differs.
pub fn script(engine: &Engine) -> Result<PathBuf, String> {
    let root = engine.store.videos_dir();
    crate::video::kit::sync(&root)?;
    Ok(root.join(".relay/clips/streamclips.py"))
}

/// Where the template a VOD's video is drawn with lives once synced.
pub fn template_dir(engine: &Engine) -> PathBuf {
    engine.store.videos_dir().join(".relay/clips/short")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn found(name: &str, env: &str) -> Option<PathBuf> {
    std::env::var_os(env).map(PathBuf::from).filter(|p| p.is_file()).or_else(|| which::which(name).ok())
}

pub fn whisper() -> Option<PathBuf> {
    found("whisper-cli", "RELAY_WHISPER").or_else(|| {
        [".local/share/whisper.cpp/bin/whisper-cli", ".local/share/whisper.cpp/build/bin/whisper-cli"].iter().map(|p| home().join(p)).find(|p| p.is_file())
    })
}

pub fn whisper_model() -> Option<PathBuf> {
    std::env::var_os("RELAY_WHISPER_MODEL").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| {
        ["models/ggml-large-v3-turbo.bin", "models/ggml-large-v3-turbo-q8_0.bin"].iter().map(|p| home().join(p)).find(|p| p.is_file())
    })
}

pub fn ffmpeg() -> PathBuf {
    found("ffmpeg", "RELAY_FFMPEG").unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

pub fn ffprobe() -> PathBuf {
    found("ffprobe", "RELAY_FFPROBE").unwrap_or_else(|| PathBuf::from("ffprobe"))
}

pub fn ytdlp() -> Option<PathBuf> {
    found("yt-dlp", "RELAY_YTDLP")
}

pub fn claude(engine: &Engine) -> Result<PathBuf, BusError> {
    if let Some(p) = std::env::var_os("RELAY_CLIPS_CLAUDE").map(PathBuf::from) {
        return Ok(p);
    }
    let conn = engine.store.lock();
    crate::providers::executable(&conn, relay_bus::types::Provider::Claude)
}

pub fn tools(engine: &Engine) -> Vec<Tool> {
    if engine.clips.tools_present.load(std::sync::atomic::Ordering::SeqCst) {
        return ["ffmpeg", "python3", "whisper.cpp", "whisper model", "Claude", "Video kit", "yt-dlp"].into_iter()
            .map(|n| Tool { name: n.into(), found: true, path: None, purpose: String::new(), install: None }).collect();
    }
    let tool = |name: &str, path: Option<PathBuf>, purpose: &str, install: &str| Tool {
        name: name.into(),
        found: path.is_some(),
        path: path.map(|p| p.display().to_string()),
        purpose: purpose.into(),
        install: Some(install.into()),
    };
    let kit = crate::video::kit::status(engine);
    vec![
        tool("ffmpeg", which::which("ffmpeg").ok().or_else(|| std::env::var_os("RELAY_FFMPEG").map(PathBuf::from)), "Reads and cuts the VOD", "sudo pacman -S ffmpeg"),
        tool("python3", which::which("python3").ok(), "Runs the transcript and loudness steps", "sudo pacman -S python"),
        tool("whisper.cpp", whisper(), "Transcribes the VOD on the GPU, every word with its time",
            "git clone --depth 1 https://github.com/ggml-org/whisper.cpp ~/.local/share/whisper.cpp && cd ~/.local/share/whisper.cpp && cmake -B build -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=native -DCMAKE_BUILD_TYPE=Release && cmake --build build -j 4 --config Release --target whisper-cli"),
        tool("whisper model", whisper_model(), "large-v3-turbo, the model whisper.cpp transcribes with",
            "mkdir -p ~/models && curl -L -o ~/models/ggml-large-v3-turbo.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"),
        tool("Claude", claude(engine).ok(), "Finds the moments and the facecam, through your own Claude account", "Install Claude Code and sign in"),
        Tool {
            name: "Video kit".into(),
            found: kit.state == relay_bus::ops::video::KitState::Ready,
            path: Some(kit.root.clone()),
            purpose: "Remotion, which draws each clip".into(),
            install: Some("Set it up in Design → Videos (about 600 MB, once)".into()),
        },
        Tool { install: Some("curl -L -o ~/.local/bin/yt-dlp https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp && chmod +x ~/.local/bin/yt-dlp".into()),
            ..tool("yt-dlp", ytdlp(), "Fetches a VOD from a link (only for links)", "") },
    ]
}

/// The tools a VOD from `source` cannot do without, missing.
pub fn missing(engine: &Engine, link: bool) -> Vec<Tool> {
    tools(engine).into_iter().filter(|t| !t.found && (link || t.name != "yt-dlp")).collect()
}

pub fn is_link(source: &str) -> bool {
    let s = source.trim();
    s.starts_with("https://") || s.starts_with("http://")
}

/// A `file://` URL (as a file manager copies it) as a path.
pub fn file_path(source: &str) -> String {
    let s = source.trim();
    match s.strip_prefix("file://") {
        Some(rest) => percent(rest),
        None => s.to_string(),
    }
}

/// `%XX` escapes decoded (a file URL, a query's value; `+` in a query is a space).
pub(crate) fn percent(s: &str) -> String {
    let s = &s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------- running tools

/// What stopped a step: the person, the engine quitting, or a failure.
#[derive(Debug)]
pub enum Halt {
    Stopped,
    Failed(String),
}

impl From<String> for Halt {
    fn from(s: String) -> Halt {
        Halt::Failed(s)
    }
}

impl From<BusError> for Halt {
    fn from(e: BusError) -> Halt {
        Halt::Failed(e.message)
    }
}

pub type Step<T> = Result<T, Halt>;

/// Whether VOD `id`'s work should stop now.
pub fn halted(engine: &Engine, id: i64) -> bool {
    engine.is_quitting() || lock(&engine.clips.stopping).contains(&id)
}

pub fn ask_stop(engine: &Engine, id: i64) {
    lock(&engine.clips.stopping).insert(id);
}

pub fn clear_stop(engine: &Engine, id: i64) {
    lock(&engine.clips.stopping).remove(&id);
}

/// The environment a tool Clips starts runs in: nothing of an agent session the engine may run
/// inside leaks into it.
pub fn clean_env(cmd: &mut Command) {
    for var in ["RELAY_SESSION", "RELAY_TOKEN", "RELAY_PROJECT_ID", "RELAY_WORKTREE", "RELAY_BRIEF", "CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        cmd.env_remove(var);
    }
}

/// Run `cmd` for VOD `vod` to the end: its stdout, or why it failed. Each line its stdout writes
/// that starts with `@progress ` moves the stage's bar (`on_progress`, 0 to 1); the run is killed
/// when the VOD is stopped or `within` passes.
pub fn run(engine: &Engine, vod: i64, cmd: &mut Command, within: Duration, on_progress: impl FnMut(f64)) -> Step<String> {
    run_with(engine, vod, cmd, Stdio::null(), within, on_progress)
}

/// [`run`] with `stdin` for the tool to read (a prompt in a file).
pub fn run_with(engine: &Engine, vod: i64, cmd: &mut Command, stdin: Stdio, within: Duration, mut on_progress: impl FnMut(f64)) -> Step<String> {
    clean_env(cmd);
    cmd.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let program = cmd.get_program().to_string_lossy().into_owned();
    let name = Path::new(&program).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(program);
    let mut child = cmd.spawn().map_err(|e| Halt::Failed(format!("{name} could not start: {e}")))?;
    let pid = child.id();
    let stderr = child.stderr.take();
    let err_tail = std::thread::spawn(move || {
        let mut tail: Vec<String> = Vec::new();
        if let Some(e) = stderr {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                tail.push(line);
                if tail.len() > 40 {
                    tail.remove(0);
                }
            }
        }
        tail
    });
    let stdout = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        if let Some(o) = stdout {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }
    });
    let deadline = Instant::now() + within;
    let mut out = String::new();
    let kill = |child: &mut std::process::Child| {
        crate::proc::terminate_group(pid, crate::proc::TERM_GRACE, || false);
        let _ = child.kill();
        let _ = child.wait();
    };
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if let Some(p) = line.strip_prefix("@progress ") {
                    if let Ok(v) = p.trim().parse::<f64>() {
                        on_progress((v / 100.0).clamp(0.0, 1.0));
                    }
                } else if let Some(v) = download_percent(&line) {
                    on_progress(v);
                } else {
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if halted(engine, vod) {
            kill(&mut child);
            return Err(Halt::Stopped);
        }
        if Instant::now() > deadline {
            kill(&mut child);
            return Err(Halt::Failed(format!("{name} took longer than {} min", within.as_secs() / 60)));
        }
    }
    let _ = reader.join();
    let status = child.wait().map_err(|e| Halt::Failed(format!("{name}: {e}")))?;
    let tail = err_tail.join().unwrap_or_default();
    if !status.success() {
        if halted(engine, vod) {
            return Err(Halt::Stopped);
        }
        let why = tail.iter().rev().take(6).rev().cloned().collect::<Vec<_>>().join("\n");
        return Err(Halt::Failed(format!("{name} failed: {}", if why.trim().is_empty() { out.lines().last().unwrap_or("").to_string() } else { why })));
    }
    Ok(out)
}

/// yt-dlp's `[download]  45.3% of …` as 0.453.
fn download_percent(line: &str) -> Option<f64> {
    let rest = line.strip_prefix("[download]")?;
    let pct = rest.split_whitespace().next()?.strip_suffix('%')?;
    pct.parse::<f64>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0))
}

/// The JSON a tool printed last (streamclips.py prints its result as one indented object).
pub fn last_json(out: &str) -> Option<Value> {
    let start = out.find(['{', '['])?;
    serde_json::from_str(&out[start..]).ok().or_else(|| {
        // Lines printed before the result: take the last top-level object.
        out.rmatch_indices("\n{").find_map(|(i, _)| serde_json::from_str(&out[i + 1..]).ok())
    })
}

// ---------------------------------------------------------------- threads

/// Start the pipeline, the watcher and the poster, once.
pub fn spawn(engine: &Arc<Engine>) {
    if engine.clips.started.set(()).is_err() {
        return;
    }
    pipeline::reconcile(engine);
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new().name("clips".into()).spawn(move || pipeline::run_loop(weak)).ok();
    let weak = Arc::downgrade(engine);
    let events = engine.subscribe();
    std::thread::Builder::new().name("clips-watch".into()).spawn(move || pipeline::watch(weak, events)).ok();
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new().name("clips-post".into()).spawn(move || youtube::post_loop(weak)).ok();
}

/// Wake the pipeline: there may be a VOD to work on.
pub fn poke(engine: &Engine) {
    let (m, c) = &*engine.clips.wake;
    *lock(m) += 1;
    c.notify_all();
}

/// Wake the poster.
pub fn poke_poster(engine: &Engine) {
    let (m, c) = &*engine.clips.post_wake;
    *lock(m) += 1;
    c.notify_all();
}

/// Sleep until poked or `max` passes.
pub(crate) fn sleep_on(wake: &Arc<(Mutex<u64>, Condvar)>, max: Duration) {
    let (m, c) = &**wake;
    let guard = lock(m);
    let seen = *guard;
    let _ = c.wait_timeout_while(guard, max, |n| *n == seen);
}

pub fn shutdown(engine: &Engine) {
    poke(engine);
    poke_poster(engine);
}

/// `text` cut to about `max` characters, on a character boundary.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// `h:mm:ss` for seconds.
pub fn clock(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// Seconds from `75`, `75.5`, `1:15`, `0:01:15.5`.
pub fn seconds(raw: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in raw.trim().split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    total.is_finite().then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_and_print() {
        assert_eq!(seconds("1:02:03.5"), Some(3723.5));
        assert_eq!(seconds("75"), Some(75.0));
        assert_eq!(seconds("x"), None);
        assert_eq!(clock(3723.4), "1:02:03");
        assert_eq!(clock(59.6), "1:00");
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(file_path("file:///home/a/My%20VOD.mp4"), "/home/a/My VOD.mp4");
        assert_eq!(file_path("/home/a/v.mp4"), "/home/a/v.mp4");
        assert!(is_link("https://www.twitch.tv/videos/1"));
        assert!(!is_link("/home/a/v.mp4"));
    }

    #[test]
    fn settings_are_checked() {
        let mut s = default_settings();
        s.posting.slots = vec!["9:30".into(), "17:00".into(), "09:30".into()];
        s.handle = "@Neph".into();
        let s = checked(s).unwrap();
        assert_eq!(s.posting.slots, vec!["09:30", "17:00"]);
        assert_eq!(s.handle, "Neph");
        let mut bad = default_settings();
        bad.highlight = "yellow".into();
        assert!(checked(bad).is_err());
        let mut bad = default_settings();
        bad.posting.slots = vec!["25:00".into()];
        assert!(checked(bad).is_err());
    }

    #[test]
    fn last_json_skips_what_came_before() {
        assert_eq!(last_json("loading\n{\n \"a\": 1\n}\n").unwrap()["a"], 1);
        assert!(last_json("nothing").is_none());
    }
}
