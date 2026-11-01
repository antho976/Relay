//! A VOD's work, stage by stage, on the `clips` thread: one VOD at a time, oldest first. Each
//! stage leaves its files in the VOD's folder and is skipped when they are there, so a VOD that
//! was stopped, failed or cut off by a restart goes on from the stage it was in.
//!
//! | Stage | Leaves |
//! | --- | --- |
//! | probe | the VOD's length and size (a link is downloaded first, `vod.mp4`) |
//! | audio | `audio.m4a` |
//! | transcribe | `words.json`, `transcript.txt` (whisper.cpp, `streamclips.py transcribe`) |
//! | signals | `signals.json`, `candidates.json` (loudness, and chat for a VOD with its replay) |
//! | layout | the VOD's layout (Claude reading frames: `frame`) |
//! | find | the clips, as drafts (Claude reading the transcript: `find`) |
//! | cut | each draft's footage in the video's `public/assets/`, its framing and its cut edges checked |
//! | render | each draft drawn by the Video plugin (`video.render`, composition `short`) |

use super::edit::{self, W};
use super::find::{self, Moment};
use super::store::{NewClip, Section};
use super::{changed, frame, halted, with_db, Halt, Step};
use crate::engine::{Door, Engine};
use relay_bus::ops::clips::*;
use relay_bus::{Actor, BusError, Request};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// What `clips.vod.find` (or an import) asked the find stage for.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ask {
    pub request: String,
    pub kinds: Vec<Kind>,
    pub max: u32,
}

fn ask_key(vod: i64) -> String {
    format!("find:{vod}")
}

pub fn set_ask(engine: &Engine, vod: i64, ask: &Ask) -> Result<(), BusError> {
    let raw = serde_json::to_string(ask).map_err(|e| BusError::internal(e.to_string()))?;
    with_db(engine, |db| db.set_meta(&ask_key(vod), &raw))
}

fn take_ask(engine: &Engine, vod: i64) -> Option<Ask> {
    with_db(engine, |db| db.meta(&ask_key(vod))).ok().flatten().and_then(|r| serde_json::from_str(&r).ok())
}

/// The file the VOD's picture and sound are read from: the file it was given, or the one a link
/// was downloaded to.
pub fn media(vod: &Vod) -> PathBuf {
    if super::is_link(&vod.source) {
        Path::new(&vod.dir).join("vod.mp4")
    } else {
        PathBuf::from(super::file_path(&vod.source))
    }
}

fn now() -> String {
    crate::time::now()
}

fn stage(engine: &Engine, vod: i64, s: Stage, detail: Option<&str>) {
    let _ = with_db(engine, |db| db.set_stage(vod, Some(s), 0.0, detail, &now()));
    changed(engine, "stage", Some(vod), None);
}

/// Move the stage's bar, telling the space at most every half second.
struct Bar<'a> {
    engine: &'a Engine,
    vod: i64,
    last: std::cell::Cell<Instant>,
}

impl<'a> Bar<'a> {
    fn new(engine: &'a Engine, vod: i64) -> Bar<'a> {
        Bar { engine, vod, last: std::cell::Cell::new(Instant::now() - Duration::from_secs(1)) }
    }
    fn set(&self, p: f64, detail: Option<&str>) {
        let _ = with_db(self.engine, |db| db.set_progress(self.vod, p, detail));
        if self.last.get().elapsed() >= Duration::from_millis(500) {
            self.last.set(Instant::now());
            changed(self.engine, "progress", Some(self.vod), None);
        }
    }
}

// ---------------------------------------------------------------- the loop

/// On start: clips an earlier engine left drawing are matched with their renders again, and those
/// it left posting go back to waiting for their slot.
pub fn reconcile(engine: &Arc<Engine>) {
    let Ok(clips) = with_db(engine, |db| db.interrupted()) else { return };
    for c in clips {
        match c.state {
            ClipState::Rendering => sync_render(engine, &c),
            ClipState::Posting => {
                let back = if c.publish_at.is_some() { ClipState::Scheduled } else { ClipState::Approved };
                let _ = with_db(engine, |db| db.set_clip_state(c.id, back, Some("Relay stopped while it was uploading; it will try again"), &now()));
            }
            _ => {}
        }
    }
}

pub fn run_loop(weak: Weak<Engine>) {
    crate::background_priority();
    loop {
        let Some(engine) = weak.upgrade() else { return };
        if engine.is_quitting() {
            return;
        }
        let next = with_db(&engine, |db| db.next_queued()).ok().flatten();
        match next {
            Some(vod) => work(&engine, vod),
            None => {
                let wake = engine.clips.wake.clone();
                drop(engine);
                super::sleep_on(&wake, Duration::from_secs(120));
            }
        }
    }
}

/// Take VOD `vod` from the stage it is at to the end, or until it is stopped or fails.
fn work(engine: &Arc<Engine>, vod: Vod) {
    let id = vod.id;
    super::clear_stop(engine, id);
    let _ = with_db(engine, |db| db.set_vod_state(id, VodState::Working, None, &now()));
    changed(engine, "vod", Some(id), None);
    let from = vod.stage.unwrap_or(Stage::Probe);
    let result = (|| -> Step<()> {
        for s in Stage::ALL.into_iter().filter(|s| *s >= from) {
            if halted(engine, id) {
                return Err(Halt::Stopped);
            }
            stage(engine, id, s, None);
            let vod = super::vod(engine, id)?;
            let began = Instant::now();
            let _done = Logged { vod: id, stage: s, began };
            match s {
                Stage::Probe => probe(engine, &vod)?,
                Stage::Audio => audio(engine, &vod)?,
                Stage::Transcribe => transcribe(engine, &vod)?,
                Stage::Signals => signals(engine, &vod)?,
                Stage::Layout => layout(engine, &vod)?,
                Stage::Find => find_stage(engine, &vod)?,
                Stage::Cut => cut_stage(engine, &vod)?,
                Stage::Render => render_stage(engine, &vod)?,
            }
        }
        Ok(())
    })();
    let t = now();
    match result {
        Ok(()) => {
            let _ = with_db(engine, |db| {
                db.set_stage(id, None, 1.0, None, &t)?;
                db.set_vod_state(id, VodState::Ready, None, &t)
            });
            notify_ready(engine, id);
            super::poke_poster(engine);
        }
        Err(Halt::Stopped) if engine.is_quitting() => {
            // Left `working`: the next engine goes on from this stage.
        }
        Err(Halt::Stopped) => {
            let _ = with_db(engine, |db| db.set_vod_state(id, VodState::Stopped, None, &t));
        }
        Err(Halt::Failed(why)) => {
            tracing::warn!(vod = id, %why, "clips: a VOD's stage failed");
            let _ = with_db(engine, |db| db.set_vod_state(id, VodState::Failed, Some(&super::cut(&why, 900)), &t));
        }
    }
    super::clear_stop(engine, id);
    changed(engine, "vod", Some(id), None);
}

/// Logs how long a stage took when it ends, however it ends: the readings docs/CLIPS.md keeps.
struct Logged {
    vod: i64,
    stage: Stage,
    began: Instant,
}

impl Drop for Logged {
    fn drop(&mut self) {
        tracing::info!(vod = self.vod, stage = self.stage.name(), secs = self.began.elapsed().as_secs_f64(), "clips: stage over");
    }
}

/// Tell the person a VOD's clips are ready, as a notification.
fn notify_ready(engine: &Engine, vod_id: i64) {
    let Ok(vod) = super::vod(engine, vod_id) else { return };
    if vod.clips_ready == 0 {
        return;
    }
    let text = format!("{} clip{} from {} wait for you", vod.clips_ready, if vod.clips_ready == 1 { "" } else { "s" }, super::cut(&vod.title, 60));
    let at = now();
    let written = {
        let conn = engine.store.lock();
        conn.prepare_cached(
            "INSERT INTO notifications(project_id, category, title, body, link, read, created_at) VALUES (NULL, 'clips', 'Clips ready', ?1, NULL, 0, ?2)",
        ).and_then(|mut st| st.execute(rusqlite::params![text, at])).map(|_| conn.last_insert_rowid())
    };
    if let Ok(id) = written {
        engine.emit_system("notify.new", json!({"category": "clips", "notification_id": id}));
    }
}

// ---------------------------------------------------------------- stages

fn probe(engine: &Engine, vod: &Vod) -> Step<()> {
    let dir = Path::new(&vod.dir);
    std::fs::create_dir_all(dir).map_err(|e| Halt::Failed(format!("{}: {e}", dir.display())))?;
    let media = media(vod);
    let mut title = None;
    if super::is_link(&vod.source) && !media.is_file() {
        let ytdlp = super::ytdlp().ok_or_else(|| Halt::Failed("yt-dlp is not installed: it fetches VODs from links (see Clips → Settings → Tools)".into()))?;
        let bar = Bar::new(engine, vod.id);
        bar.set(0.0, Some("Downloading"));
        let mut cmd = Command::new(ytdlp);
        cmd.args(["--no-playlist", "--newline", "-f", "bv*[height<=1080]+ba/b[height<=1080]/b", "--merge-output-format", "mp4",
            "--write-info-json", "--write-subs", "--sub-langs", "rechat,live_chat", "-o"])
            .arg(dir.join("vod.%(ext)s")).arg(vod.source.trim());
        super::run(engine, vod.id, &mut cmd, Duration::from_secs(6 * 3600), |p| bar.set(p, Some("Downloading")))?;
        if !media.is_file() {
            return Err(Halt::Failed("yt-dlp finished but left no vod.mp4".into()));
        }
        title = std::fs::read(dir.join("vod.info.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["title"].as_str().map(str::to_string));
    }
    if !media.is_file() {
        return Err(Halt::Failed(format!("{} is not there any more", media.display())));
    }
    let info = probe_file(&media)?;
    let _ = with_db(engine, |db| db.set_vod_media(vod.id, info.0, info.1, info.2, info.3, title.as_deref().filter(|_| vod.title == vod.source)));
    if let Some(p) = frame::poster(&media, dir, info.0) {
        let _ = with_db(engine, |db| db.set_vod_poster(vod.id, &p.display().to_string()));
    }
    Ok(())
}

/// A file's length, width, height and frame rate (ffprobe).
pub fn probe_file(path: &Path) -> Step<(f64, u32, u32, f64)> {
    let o = crate::proc::output_with_timeout(
        Command::new(super::ffprobe()).args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"]).arg(path),
        Duration::from_secs(60),
    ).map_err(|e| Halt::Failed(format!("ffprobe: {e}")))?.ok_or_else(|| Halt::Failed("ffprobe took too long".into()))?;
    if !o.status.success() {
        return Err(Halt::Failed(format!("ffprobe could not read {}: is it a video?", path.display())));
    }
    let v: Value = serde_json::from_slice(&o.stdout).map_err(|e| Halt::Failed(format!("ffprobe: {e}")))?;
    let video = v["streams"].as_array().and_then(|s| s.iter().find(|s| s["codec_type"] == "video")).cloned()
        .ok_or_else(|| Halt::Failed(format!("{} has no picture", path.display())))?;
    if !v["streams"].as_array().is_some_and(|s| s.iter().any(|s| s["codec_type"] == "audio")) {
        return Err(Halt::Failed(format!("{} has no sound: Clips finds moments in what is said", path.display())));
    }
    let rate = video["avg_frame_rate"].as_str().unwrap_or("30/1");
    let fps = rate.split_once('/').and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok().filter(|d| *d > 0.0)?)).unwrap_or(30.0);
    let duration = v["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).unwrap_or(0.0);
    Ok((duration, video["width"].as_u64().unwrap_or(0) as u32, video["height"].as_u64().unwrap_or(0) as u32, (fps * 1000.0).round() / 1000.0))
}

fn audio(engine: &Engine, vod: &Vod) -> Step<()> {
    let out = Path::new(&vod.dir).join("audio.m4a");
    if out.is_file() {
        return Ok(());
    }
    let part = Path::new(&vod.dir).join(".audio.m4a");
    let media = media(vod);
    let copy = super::run(engine, vod.id, Command::new(super::ffmpeg()).args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(&media)
        .args(["-vn", "-map", "0:a:0", "-c:a", "copy"]).arg(&part), Duration::from_secs(30 * 60), |_| {});
    if copy.is_err() {
        if let Err(Halt::Stopped) = copy {
            return Err(Halt::Stopped);
        }
        super::run(engine, vod.id, Command::new(super::ffmpeg()).args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(&media)
            .args(["-vn", "-map", "0:a:0", "-c:a", "aac", "-b:a", "160k"]).arg(&part), Duration::from_secs(60 * 60), |_| {})?;
    }
    std::fs::rename(&part, &out).map_err(|e| Halt::Failed(format!("{}: {e}", out.display())))?;
    Ok(())
}

fn python() -> PathBuf {
    which::which("python3").unwrap_or_else(|_| PathBuf::from("python3"))
}

/// The names whisper is told to spell right.
fn whisper_prompt(s: &Settings) -> String {
    let mut parts: Vec<String> = Vec::new();
    for p in [s.streamer.trim(), s.handle.trim()] {
        if !p.is_empty() && !parts.iter().any(|x| x.eq_ignore_ascii_case(p)) {
            parts.push(p.to_string());
        }
    }
    if !s.terms.trim().is_empty() {
        parts.push(s.terms.trim().to_string());
    }
    parts.join(", ")
}

fn transcribe(engine: &Engine, vod: &Vod) -> Step<()> {
    let dir = Path::new(&vod.dir);
    if dir.join("words.json").is_file() {
        return Ok(());
    }
    if super::whisper().is_none() || super::whisper_model().is_none() {
        return Err(Halt::Failed("whisper.cpp or its model is not installed (Clips → Settings → Tools says how)".into()));
    }
    let s = super::settings(engine)?;
    let script = super::script(engine)?;
    let bar = Bar::new(engine, vod.id);
    let mut cmd = Command::new(python());
    cmd.arg(&script).args(["transcribe"]).arg(dir.join("audio.m4a")).arg("--out").arg(dir).arg("--progress");
    let prompt = whisper_prompt(&s);
    if !prompt.is_empty() {
        cmd.args(["--prompt", &prompt]);
    }
    if let Some(w) = super::whisper() {
        cmd.env("RELAY_WHISPER", w);
    }
    if let Some(m) = super::whisper_model() {
        cmd.env("RELAY_WHISPER_MODEL", m);
    }
    let minutes = (vod.duration_s / 60.0).round();
    let detail = format!("{minutes:.0} min of speech, on the GPU");
    bar.set(0.0, Some(&detail));
    super::run(engine, vod.id, &mut cmd, Duration::from_secs(4 * 3600), |p| bar.set(p, None))?;
    if !dir.join("words.json").is_file() {
        return Err(Halt::Failed("whisper finished but wrote no words".into()));
    }
    Ok(())
}

fn signals(engine: &Engine, vod: &Vod) -> Step<()> {
    let dir = Path::new(&vod.dir);
    let script = super::script(engine)?;
    if !dir.join("signals.json").is_file() {
        let mut cmd = Command::new(python());
        cmd.arg(&script).arg("signals").arg(dir.join("audio.m4a")).arg("--out").arg(dir.join(".signals.json"));
        if let Some(chat) = chat_file(dir) {
            cmd.arg("--chat").arg(chat);
        }
        super::run(engine, vod.id, &mut cmd, Duration::from_secs(60 * 60), |_| {})?;
        std::fs::rename(dir.join(".signals.json"), dir.join("signals.json")).map_err(|e| Halt::Failed(e.to_string()))?;
    }
    if !dir.join("candidates.json").is_file() {
        let mut cmd = Command::new(python());
        cmd.arg(&script).arg("candidates").arg("--transcript").arg(dir.join("words.json")).arg("--signals").arg(dir.join("signals.json"))
            .arg("--out").arg(dir.join("candidates.json")).args(["--top", "90", "--spacing", "35"]);
        super::run(engine, vod.id, &mut cmd, Duration::from_secs(10 * 60), |_| {})?;
    }
    Ok(())
}

/// A chat replay yt-dlp left beside a downloaded VOD.
fn chat_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| { let n = n.to_string_lossy(); n.contains("rechat") || n.contains("live_chat") }))
}

fn layout(engine: &Engine, vod: &Vod) -> Step<()> {
    if vod.layout.is_some() {
        return Ok(());
    }
    let l = frame::detect(engine, vod, &media(vod), Path::new(&vod.dir))?;
    let _ = with_db(engine, |db| db.set_vod_layout(vod.id, &l, &now()));
    Ok(())
}

fn words_of(vod: &Vod) -> Step<Vec<W>> {
    edit::load_words(&Path::new(&vod.dir).join("words.json")).map_err(Halt::Failed)
}

fn find_stage(engine: &Engine, vod: &Vod) -> Step<()> {
    let Some(ask) = take_ask(engine, vod.id) else { return Ok(()) };
    let dir = Path::new(&vod.dir);
    let words = words_of(vod)?;
    let candidates: Value = std::fs::read(dir.join("candidates.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    let existing = with_db(engine, |db| db.clips_of(vod.id))?;
    let taken: Vec<(f64, f64)> = existing.iter().map(|c| (c.start_s, c.end_s)).collect();
    let bar = Bar::new(engine, vod.id);
    bar.set(0.0, Some("Reading the transcript"));
    let progress = |done: usize, of: usize| bar_set_sync(engine, vod.id, done as f64 / of.max(1) as f64, &format!("Read {done} of {of} parts"));
    let moments = find::find(engine, vod, dir, &words, &candidates, &ask.request, &ask.kinds, &taken, ask.max, &progress)?;
    let s = super::settings(engine)?;
    let layout = vod.layout.clone().unwrap_or_else(|| frame::fallback(vod.width, vod.height, None));
    let t = now();
    for m in &moments {
        let e = edit_for(m, &words, &layout, &s, vod);
        let length = edit::length(&words, &e);
        let (start, end) = edit::bounds(&e);
        let mut tags = m.tags.clone();
        let credit = s.handle.trim().to_lowercase();
        if !credit.is_empty() && !tags.contains(&credit) {
            tags.push(credit);
        }
        with_db(engine, |db| db.add_clip(vod.id, &NewClip {
            kind: m.kind, title: m.title.clone(), description: m.description.clone(), tags, score: m.score, why: m.why.clone(),
            start_s: start, end_s: end, length_s: length, edit: e.clone(),
        }, &t))?;
    }
    with_db(engine, |db| db.delete_meta(&ask_key(vod.id)))?;
    changed(engine, "clips", Some(vod.id), None);
    if moments.is_empty() {
        tracing::info!(vod = vod.id, "clips: the finder found nothing new");
    }
    Ok(())
}

/// [`Bar::set`] for a closure shared across threads: a write and an event, no rate limit (a
/// find has a handful of parts).
fn bar_set_sync(engine: &Engine, vod: i64, p: f64, detail: &str) {
    let _ = with_db(engine, |db| db.set_progress(vod, p, Some(detail)));
    changed(engine, "progress", Some(vod), None);
}

/// A moment's first edit: its spans on the words, what was asked to be cut taken out, the teaser
/// first; its framing from the VOD's layout and what the finder suggested.
pub fn edit_for(m: &Moment, words: &[W], layout: &SourceLayout, s: &Settings, vod: &Vod) -> ClipEdit {
    let (mut a, mut b) = edit::snap(words, m.start, m.end, 3.0);
    // Held to the longest clip: ended at a sentence before it when the moment ran long.
    let probe = |a: f64, b: f64| edit::length(words, &ClipEdit { keep: vec![[a, b]], ..base(s) });
    if probe(a, b) > s.max_s as f64 + 4.0 {
        let (_, e) = edit::snap(words, a, a + s.max_s as f64 - 2.0, 0.0);
        b = e.max(a + s.min_s as f64);
    }
    if b - a < 1.0 {
        (a, b) = (m.start, m.end);
    }
    let mut keep = vec![[a, b]];
    for &[ca, cb] in &m.cut {
        // A cut from the end of the word before to the start of the word after.
        let (ca, cb) = (edit::between(words, ca - 0.6, ca).last().map_or(ca, |w| w.end_ms / 1000.0 + 0.08), edit::between(words, cb, cb + 0.6).first().map_or(cb, |w| w.start_ms / 1000.0 - 0.08));
        if cb <= ca {
            continue;
        }
        keep = keep.into_iter().flat_map(|[x, y]| {
            if ca >= y || cb <= x {
                vec![[x, y]]
            } else {
                [[x, ca], [cb, y]].into_iter().filter(|[p, q]| q - p >= 0.4).collect()
            }
        }).collect();
    }
    if let Some([ta, tb]) = m.teaser {
        let inner = edit::between(words, ta, tb);
        if let (Some(f), Some(l)) = (inner.first(), inner.last()) {
            keep.insert(0, [(f.start_ms / 1000.0 - 0.08).max(0.0), l.end_ms / 1000.0 + 0.15]);
        }
    }
    // A facecam fills the frame only when it is big enough not to blur: most are a corner box.
    let big_cam = layout.cam.is_some_and(|c| c.w >= vod.width as f64 * 0.35);
    let lay = match layout.kind.as_str() {
        "facecam" => match m.layout {
            Some(Layout::Fit) => Layout::Fit,
            Some(Layout::Cam) if big_cam => Layout::Cam,
            Some(Layout::Game) => Layout::Game,
            _ if m.kind == Kind::Guide => Layout::Fit,
            _ => Layout::Split,
        },
        "screen" => if m.layout == Some(Layout::Game) { Layout::Game } else { Layout::Fit },
        _ => if m.layout == Some(Layout::Fit) { Layout::Fit } else { Layout::Crop },
    };
    let view = match lay {
        Layout::Game => layout.game.map(frame::game_crop),
        Layout::Cam => layout.cam,
        // The game's window, without the stream's empty borders.
        Layout::Fit if layout.kind != "camera" => layout.game,
        _ => None,
    };
    let e = ClipEdit {
        keep,
        layout: lay,
        view,
        hook: if m.hook.trim().is_empty() { m.title.clone() } else { m.hook.clone() },
        emphasis: m.emphasis.clone(),
        punch_at: if lay == Layout::Fit { vec![] } else { m.punch.into_iter().collect() },
        ..base(s)
    };
    edit::checked(e.clone(), vod.duration_s, vod.width, vod.height).unwrap_or(e)
}

fn base(s: &Settings) -> ClipEdit {
    ClipEdit {
        keep: vec![], tighten_ms: Some(700), layout: Layout::Fit, view: None, reframe: Default::default(), hook: String::new(),
        captions: s.captions, caption_y: None, emphasis: vec![], fixes: Default::default(), punch_at: vec![], end_card: s.end_card, scene: None,
    }
}

fn cut_stage(engine: &Engine, vod: &Vod) -> Step<()> {
    let drafts: Vec<Clip> = with_db(engine, |db| db.clips_of(vod.id))?.into_iter().filter(|c| c.state == ClipState::Draft).collect();
    if drafts.is_empty() {
        return Ok(());
    }
    let words = words_of(vod)?;
    ensure_video(engine, vod)?;
    let bar = Bar::new(engine, vod.id);
    // Each clip's footage first, then one look at every clip's scene, then each one's framing.
    let n = drafts.len();
    let mut cut = Vec::new();
    for (i, c) in drafts.iter().enumerate() {
        if halted(engine, vod.id) {
            return Err(Halt::Stopped);
        }
        bar.set(i as f64 / n as f64 * 0.7, Some(&format!("Cutting clip {} of {n}", i + 1)));
        let section = prepare(engine, vod.id, c.id)?;
        cut.push((c.id, section));
    }
    bar.set(0.72, Some("Looking at each clip's scene"));
    let video_dir = video_dir(engine, vod)?;
    let items: Vec<(i64, PathBuf, f64, f64)> = cut.iter().filter_map(|(id, section)| {
        let c = drafts.iter().find(|c| c.id == *id)?;
        let [a, b] = c.edit.keep.iter().copied().max_by(|x, y| (x[1] - x[0]).total_cmp(&(y[1] - y[0])))?;
        let local = |t: f64| (t - section.start_s).max(0.0);
        Some((*id, video_dir.join("public").join(&section.file), local(a + (b - a) * 0.3), local(a + (b - a) * 0.75)))
    }).collect();
    let scenes = frame::scenes(engine, vod, Path::new(&vod.dir), &items)?;
    for (i, (id, section)) in cut.iter().enumerate() {
        if halted(engine, vod.id) {
            return Err(Halt::Stopped);
        }
        bar.set(0.75 + i as f64 / n as f64 * 0.25, Some(&format!("Framing clip {} of {n}", i + 1)));
        settle(engine, vod, *id, &words, section, scenes.get(id))?;
    }
    Ok(())
}

/// A clip's first framing, once its footage is cut: its scene (when the stream switched to
/// another for it), the framing that suits it, a crop that follows the person, and its cut
/// edges checked against the sound.
fn settle(engine: &Engine, vod: &Vod, clip_id: i64, words: &[W], section: &Section, scene: Option<&frame::Scene>) -> Step<()> {
    let c = super::clip(engine, clip_id)?;
    let mut e = c.edit.clone();
    if let Some(kind) = scene.map(|s| s.kind.as_str()) {
        e.scene = frame::scene_layout(vod.layout.as_ref(), kind, vod.width, vod.height);
        let suited = frame::suited(e.layout, kind);
        if suited != e.layout {
            e.layout = suited;
            e.view = None;
            e.reframe.clear();
        }
        // Nobody to follow: the streamer is off camera for it (at the stove, out of the room).
        // The whole picture says more than a crop of an empty corner.
        if scene.is_some_and(|s| s.absent) && e.layout == Layout::Crop {
            e.layout = Layout::Fit;
            e.view = None;
            e.reframe.clear();
        }
        if e.layout == Layout::Fit {
            e.punch_at.clear();
        }
    }
    let layout = e.scene.as_ref().or(vod.layout.as_ref());
    let path = video_dir(engine, vod)?.join("public").join(&section.file);
    match e.layout {
        Layout::Crop if e.view.is_none() => {
            let local: Vec<[f64; 2]> = e.keep.iter().map(|[a, b]| [a - section.start_s, b - section.start_s]).collect();
            let (view, reframes) = frame::follow(&path, &local, vod.width, vod.height, layout.and_then(|l| l.subject), scene.and_then(|s| s.streamer));
            e.view = Some(view);
            e.reframe = reframes.into_iter().map(|(i, r)| (i.to_string(), r)).collect();
        }
        Layout::Game if e.view.is_none() => e.view = layout.and_then(|l| l.game).map(frame::game_crop),
        Layout::Fit if e.view.is_none() && layout.is_some_and(|l| l.kind != "camera") => e.view = layout.and_then(|l| l.game),
        Layout::Cam if e.view.is_none() => e.view = layout.and_then(|l| l.cam),
        _ => {}
    }
    if let Some(fixed) = check_edges(engine, vod, &path, section, words, &e) {
        e.keep = fixed;
    }
    let e = edit::checked(e, vod.duration_s, vod.width, vod.height).map_err(Halt::Failed)?;
    let (start, end) = edit::bounds(&e);
    let length = edit::length(words, &e);
    with_db(engine, |db| db.update_clip(clip_id, &c.title, &c.description, &c.tags, c.kind, &e, start, end, length, &now()))?;
    Ok(())
}

/// Make clip `clip_id` ready to draw: its footage cut, again when its spans moved outside it.
pub fn prepare(engine: &Engine, vod_id: i64, clip_id: i64) -> Step<Section> {
    let vod = super::vod(engine, vod_id)?;
    let c = super::clip(engine, clip_id)?;
    let video_dir = video_dir(engine, &vod)?;
    let current = with_db(engine, |db| db.section(clip_id))?;
    if let Some(s) = current.filter(|s| edit::covers(s, &c.edit) && video_dir.join("public").join(&s.file).is_file()) {
        return Ok(s);
    }
    let (a, b) = edit::section_for(&c.edit, vod.duration_s);
    let file = format!("assets/clip-{clip_id}.mp4");
    let s = super::settings(engine)?;
    cut_footage(engine, vod_id, &media(&vod), a, b, s.fps, &video_dir.join("public").join(&file))?;
    let section = Section { file, start_s: a, end_s: b };
    with_db(engine, |db| db.set_section(clip_id, &section))?;
    Ok(section)
}

/// The kept spans with their edges moved off words and loud sounds (`streamclips.py cuts`), or
/// `None` when they were fine or could not be checked.
fn check_edges(engine: &Engine, vod: &Vod, footage: &Path, section: &Section, words: &[W], e: &ClipEdit) -> Option<Vec<[f64; 2]>> {
    let script = super::script(engine).ok()?;
    let local: Vec<Value> = edit::between(words, section.start_s, section.end_s).iter().map(|w| json!({
        "text": w.text, "startMs": (w.start_ms - section.start_s * 1000.0).max(0.0), "endMs": (w.end_ms - section.start_s * 1000.0).max(0.0),
    })).collect();
    let words_file = Path::new(&vod.dir).join(format!(".edge-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&words_file, serde_json::to_vec(&local).ok()?).ok()?;
    let keep: Vec<[f64; 2]> = e.keep.iter().map(|[a, b]| [a - section.start_s, b - section.start_s]).collect();
    let mut cmd = Command::new(python());
    cmd.arg(&script).arg("cuts").arg(footage).arg("--keep").arg(serde_json::to_string(&keep).ok()?).arg("--words").arg(&words_file);
    if e.end_card {
        cmd.arg("--runs-on");
    }
    let out = super::run(engine, vod.id, &mut cmd, Duration::from_secs(120), |_| {});
    let _ = std::fs::remove_file(&words_file);
    let v = super::last_json(&out.ok()?)?;
    if v["ok"].as_bool() == Some(true) {
        return None;
    }
    let fixed: Vec<[f64; 2]> = v["keep"].as_array()?.iter().filter_map(|p| Some([p.get(0)?.as_f64()? + section.start_s, p.get(1)?.as_f64()? + section.start_s])).collect();
    // Only small moves: an edge that would travel far is left where the finder put it.
    let close = fixed.len() == e.keep.len() && fixed.iter().zip(&e.keep).all(|(f, k)| (f[0] - k[0]).abs() <= 1.1 && (f[1] - k[1]).abs() <= 1.1 && f[1] > f[0]);
    close.then_some(fixed)
}

/// Whether ffmpeg here can encode on the GPU.
fn nvenc() -> bool {
    static HAS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HAS.get_or_init(|| {
        crate::proc::output_with_timeout(Command::new(super::ffmpeg()).args(["-hide_banner", "-encoders"]), Duration::from_secs(20))
            .ok().flatten().is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("h264_nvenc"))
    })
}

/// Cut `[a, b]` of `media` to `out`, re-encoded from the exact frame at a constant `fps` (a copy
/// would start at the keyframe before), on the GPU when it can.
fn cut_footage(engine: &Engine, vod: i64, media: &Path, a: f64, b: f64, fps: u32, out: &Path) -> Step<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Halt::Failed(format!("{}: {e}", parent.display())))?;
    }
    let part = out.with_file_name(format!(".{}", out.file_name().unwrap_or_default().to_string_lossy()));
    let encode = |gpu: bool| {
        let mut cmd = Command::new(super::ffmpeg());
        cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-ss", &format!("{a:.3}"), "-i"]).arg(media)
            .args(["-t", &format!("{:.3}", b - a), "-map", "0:v:0", "-map", "0:a:0?", "-vf", &format!("fps={fps}"), "-pix_fmt", "yuv420p"]);
        if gpu {
            cmd.args(["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "19", "-b:v", "0"]);
        } else {
            cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18"]);
        }
        cmd.args(["-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-movflags", "+faststart", "-f", "mp4"]).arg(&part);
        super::run(engine, vod, &mut cmd, Duration::from_secs(15 * 60), |_| {})
    };
    let done = if nvenc() { encode(true).or_else(|e| if matches!(e, Halt::Stopped) { Err(e) } else { encode(false) }) } else { encode(false) };
    done?;
    std::fs::rename(&part, out).map_err(|e| Halt::Failed(format!("{}: {e}", out.display())))?;
    Ok(())
}

fn render_stage(engine: &Engine, vod: &Vod) -> Step<()> {
    let drafts: Vec<Clip> = with_db(engine, |db| db.clips_of(vod.id))?.into_iter().filter(|c| c.state == ClipState::Draft).collect();
    let mut ids = Vec::new();
    for c in &drafts {
        if halted(engine, vod.id) {
            return Err(Halt::Stopped);
        }
        match queue_render(engine, c.id) {
            Ok(id) => ids.push(id),
            Err(e) => {
                let _ = with_db(engine, |db| db.set_clip_state(c.id, ClipState::Failed, Some(&e.message), &now()));
            }
        }
    }
    // Those already drawing from before a restart are waited for too.
    ids.extend(with_db(engine, |db| db.clips_of(vod.id))?.iter().filter(|c| c.state == ClipState::Rendering).filter_map(|c| c.render_id));
    ids.sort_unstable();
    ids.dedup();
    let bar = Bar::new(engine, vod.id);
    let total = ids.len();
    loop {
        if halted(engine, vod.id) {
            return Err(Halt::Stopped);
        }
        let (renders, _) = crate::video::jobs::wait(engine, &ids, Duration::from_secs(5))?;
        let done = renders.iter().filter(|r| r.state.is_final()).count();
        let partial: f64 = renders.iter().map(|r| if r.state.is_final() { 1.0 } else { r.progress }).sum();
        bar.set(partial / total.max(1) as f64, Some(&format!("{done} of {total} drawn")));
        if done == total {
            break;
        }
    }
    for c in with_db(engine, |db| db.clips_of(vod.id))? {
        if c.state == ClipState::Rendering {
            sync_render(engine, &c);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the video

/// The folder of VOD `vod`'s video, made (with the clips template) on first use.
fn video_dir(engine: &Engine, vod: &Vod) -> Step<PathBuf> {
    let id = ensure_video(engine, vod)?;
    let v = engine.read_store("clips.video", |conn| crate::video::get_video(conn, id))?;
    Ok(PathBuf::from(v.dir))
}

/// The video VOD `vod`'s clips are compositions of: made with `video.create` the first time,
/// again if it was deleted, and its template brought up to this build's.
pub fn ensure_video(engine: &Engine, vod: &Vod) -> Step<i64> {
    // The row as it is now: the `vod` a stage read at its start does not know a video made since.
    let vod = &super::vod(engine, vod.id)?;
    let existing = vod.video_id.and_then(|id| engine.read_store("clips.video", |conn| crate::video::get_video(conn, id)).ok()).filter(|v| v.deleted_at.is_none());
    let video = match existing {
        Some(v) => v,
        None => {
            let s = super::settings(engine)?;
            let who = if s.streamer.trim().is_empty() { String::new() } else { format!("{} · ", s.streamer.trim()) };
            let title = super::cut(&format!("Clips · {who}{}", vod.title), 80);
            let out = engine.dispatch(Request::new(Actor::User, "video.create", json!({
                "project_id": vod.project_id, "title": title, "shape": "vertical", "fps": s.fps as f64, "seconds": 30.0,
            })), Door::InProcess).into_result()?;
            let v: relay_bus::ops::video::Video = serde_json::from_value(out["video"].clone()).map_err(|e| Halt::Failed(e.to_string()))?;
            with_db(engine, |db| db.set_vod_video(vod.id, v.id))?;
            v
        }
    };
    install_template(engine, Path::new(&video.dir))?;
    Ok(video.id)
}

/// Write the clips template into a video's folder where it differs: `src/Root.tsx`,
/// `src/Short.tsx`, and the caption font in `public/assets/`.
pub fn install_template(engine: &Engine, dir: &Path) -> Step<()> {
    super::script(engine)?; // syncs the kit's runtime files, the template among them
    let from = super::template_dir(engine);
    let files = [("Root.tsx", "src/Root.tsx"), ("Short.tsx", "src/Short.tsx"), ("Video.tsx", "src/Video.tsx"), ("PixelifySans.ttf", "public/assets/PixelifySans.ttf"), ("OFL.txt", "public/assets/OFL.txt")];
    for (src, dst) in files {
        let bytes = std::fs::read(from.join(src)).map_err(|e| Halt::Failed(format!("the clips template's {src}: {e}")))?;
        let to = dir.join(dst);
        if std::fs::read(&to).ok().as_deref() == Some(&bytes[..]) {
            continue;
        }
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p).map_err(|e| Halt::Failed(e.to_string()))?;
        }
        std::fs::write(&to, bytes).map_err(|e| Halt::Failed(format!("{}: {e}", to.display())))?;
    }
    Ok(())
}

/// Queue clip `clip_id`'s render: its footage made ready, its props from its edit; the render's
/// id. The clip is `rendering` until the watcher sees the render finish.
pub fn queue_render(engine: &Engine, clip_id: i64) -> Result<i64, BusError> {
    let c = super::clip(engine, clip_id)?;
    let vod = super::vod(engine, c.vod_id)?;
    let words = words_of(&vod).map_err(halt_bus)?;
    let section = prepare(engine, vod.id, clip_id).map_err(halt_bus)?;
    let video_id = ensure_video(engine, &vod).map_err(halt_bus)?;
    let s = super::settings(engine)?;
    let c = super::clip(engine, clip_id)?;
    let props = edit::props(&c.edit, &section, &words, &vod, vod.layout.as_ref(), &s);
    let out = engine.dispatch(Request::new(Actor::User, "video.render", json!({
        "video_id": video_id, "composition": "short", "props": props, "name": format!("clip-{clip_id}"), "preset": "shorts",
    })), Door::InProcess).into_result()?;
    let render_id = out["id"].as_i64().ok_or_else(|| BusError::internal("video.render answered no render id"))?;
    with_db(engine, |db| db.set_clip_render(clip_id, Some(render_id), &now()))?;
    changed(engine, "clip", Some(vod.id), Some(clip_id));
    Ok(render_id)
}

fn halt_bus(h: Halt) -> BusError {
    match h {
        Halt::Stopped => BusError::unavailable("clips.stopped", "stopped"),
        Halt::Failed(why) => BusError::unavailable("clips.failed", why),
    }
}

/// Bring clip `c`'s row up to its render's: drawn (with its poster), failed, or still going.
pub fn sync_render(engine: &Engine, c: &Clip) {
    let Some(rid) = c.render_id else { return };
    let Ok(r) = engine.read_store("clips.render", |conn| crate::video::get_render(conn, rid)) else { return };
    use relay_bus::ops::video::RenderState;
    let t = now();
    match r.state {
        RenderState::Done => {
            let Some(path) = r.path.clone() else { return };
            let vod = super::vod(engine, c.vod_id).ok();
            let poster = vod.as_ref().and_then(|v| {
                let dir = Path::new(&v.dir).join("posters");
                let _ = std::fs::create_dir_all(&dir);
                let out = dir.join(format!("clip-{}.jpg", c.id));
                frame::grab(Path::new(&path), 0.6, &out, 360, "").ok().map(|_| out.display().to_string())
            });
            let length = probe_file(Path::new(&path)).ok().map(|i| (i.0 * 10.0).round() / 10.0);
            let _ = with_db(engine, |db| {
                db.set_clip_file(c.id, &path, poster.as_deref(), length, &t)?;
                // An approved clip drawn again stays approved; any other goes to the person.
                let next = if matches!(c.state, ClipState::Approved | ClipState::Scheduled) { c.state } else { ClipState::Ready };
                db.set_clip_state(c.id, next, None, &t)
            });
        }
        RenderState::Failed | RenderState::Cancelled => {
            let why = r.error.clone().unwrap_or_else(|| "the render was stopped".into());
            let _ = with_db(engine, |db| db.set_clip_state(c.id, ClipState::Failed, Some(&super::cut(&why, 900)), &t));
        }
        _ => return,
    }
    changed(engine, "clip", Some(c.vod_id), Some(c.id));
    super::poke_poster(engine);
}

/// The watcher: every finished render of a clip brings the clip's row up to it.
pub fn watch(weak: Weak<Engine>, mut events: tokio::sync::broadcast::Receiver<relay_bus::envelope::Event>) {
    loop {
        let event = match events.blocking_recv() {
            Ok(e) => e,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => return,
        };
        if event.ev != "video.render.finished" {
            continue;
        }
        let Some(engine) = weak.upgrade() else { return };
        if engine.is_quitting() {
            return;
        }
        let Some(rid) = event.payload["id"].as_i64() else { continue };
        if let Ok(Some(c)) = with_db(&engine, |db| db.clip_by_render(rid)) {
            sync_render(&engine, &c);
        }
    }
}

/// Stop VOD `id`'s work: at once when it is the one running, else it simply will not start.
pub fn stop(engine: &Engine, id: i64) -> Result<(), BusError> {
    let vod = super::vod(engine, id)?;
    if vod.state == VodState::Working {
        super::ask_stop(engine, id);
    } else if vod.state == VodState::Queued {
        with_db(engine, |db| db.set_vod_state(id, VodState::Stopped, None, &now()))?;
    }
    Ok(())
}
