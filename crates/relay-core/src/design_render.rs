//! The state `design.render` waits in (render-SPEC-v2 §5): the desktop apps that offered to draw,
//! the renders waiting for their answer, the checks an answer passes, and the pictures written
//! for agents.
//!
//! A render crosses three requests. The agent's `design.render` reaches the socket door, which
//! checks it against the store and calls [`Renders::start`]: that queues one
//! `design.render.request` line on the newest leased connection's own queue (never a broadcast,
//! never in the `Recent` ring) and leaves a [`Wait`] under a fresh render id. The desktop app
//! draws and sends `design.render.answer` with that id on the same connection; [`answer`] takes
//! the wait, checks what was drawn, writes the PNG and hands the result to the door, which
//! answers the agent.
//!
//! Everything here sits behind one `std::sync::Mutex`, held for microseconds and never across an
//! await, and nothing here touches the store: the answer is an unlocked query (D149).

use crate::engine::Engine;
use crate::picture;
use relay_bus::design::motion::MAX_MS;
use relay_bus::design::MAX_NAME;
use relay_bus::error::BusError;
use relay_bus::ops::design::{
    AnswerIn, AnswerOut, RenderDrawn, RenderFailure, RenderIssue, RenderOut, RenderPng, RenderRequest,
    RENDER_AREA_PX_MAX, RENDER_BOX_FIELDS, RENDER_SCALE_MAX, SERVE_FONTS_MAX,
};
use relay_bus::types::Id;
use relay_bus::{Actor, Event};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Renders waiting for the desktop app at once, engine-wide; one more is `design.render.busy`.
/// Only one app draws at a time and it draws one job after another, so a fifth would only wait.
pub(crate) const MAX_WAITS: usize = 4;
/// Pictures the render folder keeps, the one just written among them, and the age past which
/// one goes once a newer one is written. There is no timer: with no renders nothing arrives, and
/// the folder stays as it is.
const KEEP_FILES: usize = 32;
const KEEP_FOR: Duration = Duration::from_secs(60 * 60);
/// How much sooner than the door gives up the request line tells the app to stop, so an app
/// that wakes late skips work nobody waits for.
const DEADLINE_MARGIN_MS: i64 = 500;
/// The app's own failure text, and each issue's and warning's, cut to this many characters.
const FAILURE_CHARS: usize = 500;
const NOTE_CHARS: usize = 300;
/// Issues and warnings an answer lists, and the ids one issue names; past these an answer is cut,
/// never refused.
const MAX_ISSUES: usize = 100;
const MAX_WARNINGS: usize = 20;
const ISSUE_IDS: usize = 8;
/// The failures an app reports under their own code; any other is `design.render.failed`.
const APP_CODES: [&str; 5] = [
    "design.render.no_surface",
    "design.render.failed",
    "design.not_found",
    "design.page_not_found",
    "design.node_not_found",
];

/// What the door hands the agent: the render, or why there is none.
pub(crate) type Reply = Result<RenderOut, BusError>;

/// The render state: one per engine (`Engine::renders`).
#[derive(Default)]
pub(crate) struct Renders {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Leased connections, oldest first; a request goes to the newest that still takes lines.
    renderers: Vec<Renderer>,
    waits: HashMap<Uuid, Wait>,
    next_lease: u64,
}

/// A connection that offered to draw. Its sender is weak, so a lease can never keep the
/// connection's writer alive: when the connection drops its own sender the writer ends, and
/// [`Renders::start`] passes over it.
struct Renderer {
    lease: u64,
    tx: mpsc::WeakSender<String>,
    /// The font families its app reported: sorted, each once.
    fonts: Option<Arc<Vec<String>>>,
}

/// A render the door is waiting on: what its answer is checked against, and where it goes.
pub(crate) struct Wait {
    lease: u64,
    design_id: Id,
    page: String,
    base_rev: Option<i64>,
    png: bool,
    max_side: u32,
    boxes_limit: u32,
    frames: Option<u8>,
    started: Instant,
    reply: oneshot::Sender<Reply>,
}

/// What the door resolved against the store: the design, the page's id and the nodes a render
/// narrows to, all checked.
// Built by the socket door (render-door); until it is wired, only the tests build one.
#[allow(dead_code)]
pub(crate) struct Target {
    pub design_id: Id,
    pub page: String,
    pub nodes: Option<Vec<String>>,
}

/// A render's options after the door's range checks, defaults filled in.
// Built by the socket door (render-door); until it is wired, only the tests build one.
#[allow(dead_code)]
pub(crate) struct Options {
    pub png: bool,
    pub scale: f64,
    pub max_side: u32,
    pub boxes_limit: u32,
    pub base_rev: Option<i64>,
    pub timeout: Duration,
    pub at_ms: Option<f64>,
    pub frames: Option<u8>,
    /// The asking agent's session name, for the note an open board shows; none for the person.
    pub by: Option<String>,
}

// The lease, the start, the cancel and the fonts are the socket door's and design.fonts' (the
// render-door and engine-find-fonts streams); until they are wired, only the tests call them.
#[allow(dead_code)]
impl Renders {
    /// A number for a connection's lease, never given twice.
    pub(crate) fn new_lease(&self) -> u64 {
        let mut inner = self.inner.lock().unwrap();
        inner.next_lease += 1;
        inner.next_lease
    }

    /// `design.render.serve`: the connection holding `lease` offers to draw, from now on the
    /// newest, with the font families its app reported. Serving again on one lease replaces its
    /// entry and makes it the newest.
    pub(crate) fn serve(&self, lease: u64, tx: mpsc::WeakSender<String>, fonts: Option<Vec<String>>) {
        let fonts = fonts.map(|fonts| Arc::new(families(fonts)));
        let mut inner = self.inner.lock().unwrap();
        inner.renderers.retain(|renderer| renderer.lease != lease);
        inner.renderers.push(Renderer { lease, tx, fonts });
    }

    /// The connection holding `lease` ended: forget it, and end its waits. Their senders drop, so
    /// each door waiting on one answers `design.render.renderer_gone` at once.
    pub(crate) fn release(&self, lease: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.renderers.retain(|renderer| renderer.lease != lease);
        inner.waits.retain(|_, wait| wait.lease != lease);
    }

    /// Send a render to the newest app that still takes lines and leave its wait. The cap, the
    /// choice of app, the line and the wait all happen under one hold of the lock, so an answer
    /// can never arrive before its wait exists. A lease whose connection is gone (it dropped its
    /// sender, or its writer ended) is forgotten and the next older one tried; a full queue is
    /// `busy`. Nothing is sent when this refuses.
    pub(crate) fn start(&self, target: &Target, options: &Options) -> Result<(Uuid, oneshot::Receiver<Reply>), BusError> {
        let mut guard = self.inner.lock().unwrap();
        let inner = &mut *guard;
        if inner.waits.len() >= MAX_WAITS {
            return Err(busy(format!("{MAX_WAITS} renders are already waiting for the desktop app")));
        }
        let id = Uuid::new_v4();
        let mut line = request_line(id, target, options, unix_ms(SystemTime::now()))?;
        while let Some(renderer) = inner.renderers.last() {
            let lease = renderer.lease;
            line = match renderer.tx.upgrade() {
                // The connection dropped its own sender: it is ending.
                None => line,
                Some(tx) => match tx.try_send(line) {
                    Ok(()) => {
                        let (reply, rx) = oneshot::channel();
                        inner.waits.insert(id, Wait {
                            lease,
                            design_id: target.design_id,
                            page: target.page.clone(),
                            base_rev: options.base_rev,
                            png: options.png,
                            max_side: options.max_side,
                            boxes_limit: options.boxes_limit,
                            frames: options.frames,
                            started: Instant::now(),
                            reply,
                        });
                        return Ok((id, rx));
                    }
                    Err(TrySendError::Full(_)) => return Err(busy("the desktop app's queue is full")),
                    // Its writer ended: the socket broke.
                    Err(TrySendError::Closed(line)) => line,
                },
            };
            inner.renderers.pop();
        }
        Err(no_renderer())
    }

    /// The door gave up on render `id` (its timeout, its agent left, an error): forget the wait
    /// if an answer has not taken it.
    pub(crate) fn cancel(&self, id: Uuid) {
        self.inner.lock().unwrap().waits.remove(&id);
    }

    /// The answer's half: take render `id`'s wait, so a timeout and an answer can never both act
    /// on it.
    pub(crate) fn take(&self, id: Uuid) -> Option<Wait> {
        self.inner.lock().unwrap().waits.remove(&id)
    }

    /// The font families the newest connected app that reported any can draw (sorted, each
    /// once): what design.fonts lists and design.apply's font warnings read. None when no
    /// connected app has reported them.
    pub(crate) fn fonts(&self) -> Option<Arc<Vec<String>>> {
        let inner = self.inner.lock().unwrap();
        inner.renderers.iter().rev().filter(|renderer| renderer.tx.strong_count() > 0).find_map(|renderer| renderer.fonts.clone())
    }
}

/// The families an app reported, as design.fonts gives them: sorted, each once, only names of 1 to
/// 200 characters with no control character, at most [`SERVE_FONTS_MAX`]. A list past the cap is
/// cut rather than refused, so a machine with many fonts still draws for agents.
fn families(fonts: Vec<String>) -> Vec<String> {
    let mut fonts: Vec<String> = fonts
        .into_iter()
        .filter(|font| !font.trim().is_empty() && font.chars().count() <= MAX_NAME && !font.chars().any(char::is_control))
        .collect();
    fonts.sort_unstable();
    fonts.dedup();
    if fonts.len() > SERVE_FONTS_MAX {
        tracing::warn!(families = fonts.len(), "the desktop app reported more font families than design.fonts keeps; the rest are left out");
        fonts.truncate(SERVE_FONTS_MAX);
    }
    fonts
}

/// The line the leased connection reads: shaped as an event, so the app's reader already takes
/// it, but written to that one connection only.
fn request_line(id: Uuid, target: &Target, options: &Options, now_ms: i64) -> Result<String, BusError> {
    let timeout_ms = i64::try_from(options.timeout.as_millis()).unwrap_or(i64::MAX);
    let request = RenderRequest {
        render_id: id.to_string(),
        design_id: target.design_id,
        page: target.page.clone(),
        nodes: target.nodes.clone(),
        png: options.png,
        scale: options.scale,
        max_side: options.max_side,
        boxes_limit: options.boxes_limit,
        deadline_unix_ms: now_ms.saturating_add(timeout_ms).saturating_sub(DEADLINE_MARGIN_MS),
        at_ms: options.at_ms,
        frames: options.frames,
        by: options.by.clone(),
    };
    let payload = serde_json::to_value(request).map_err(crate::engine::internal)?;
    serde_json::to_string(&Event::new("design.render.request", crate::time::now(), Actor::System, payload))
        .map_err(crate::engine::internal)
}

fn unix_ms(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH).map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

/// `design.render.answer`: the desktop app's answer to the render `render_id` names. `accepted`
/// says a render was still waiting for it and got it, drawn or refused: every path after the wait
/// is taken sends exactly once, so an answer that arrived never reaches its agent as
/// `renderer_gone`.
pub(crate) fn answer(engine: &Engine, p: AnswerIn) -> AnswerOut {
    answer_into(&engine.renders, &engine.store.design_renders_dir(), p)
}

fn answer_into(renders: &Renders, dir: &Path, p: AnswerIn) -> AnswerOut {
    let Some((id, wait)) = Uuid::parse_str(&p.render_id).ok().and_then(|id| Some((id, renders.take(id)?))) else {
        return AnswerOut { accepted: false };
    };
    let result = checked(dir, id, &wait, p);
    AnswerOut { accepted: wait.reply.send(result).is_ok() }
}

/// What the agent gets for this answer: the render, its picture written to `dir`, or the reason
/// there is none. Long lists and texts are cut, never refused. When the door no longer waits,
/// nothing is written.
fn checked(dir: &Path, id: Uuid, wait: &Wait, p: AnswerIn) -> Reply {
    let drawn = match (p.drawn, p.error) {
        (Some(drawn), None) => drawn,
        (None, Some(error)) => return Err(app_failure(error)),
        (Some(_), Some(_)) => return Err(failed("an answer gives exactly one of drawn and error, and this one gave both")),
        (None, None) => return Err(failed("an answer gives exactly one of drawn and error, and this one gave neither")),
    };
    if let Some(base_rev) = wait.base_rev.filter(|base_rev| *base_rev != drawn.rev) {
        return Err(stale(wait.design_id, drawn.rev, base_rev));
    }
    let picture = check_drawn(wait, &drawn).map_err(failed)?;
    let RenderDrawn { rev, area, scale, boxes, issues, issue_count, warnings, timings_ms, times_ms, .. } = drawn;
    let issue_count = issue_count.max(u32::try_from(issues.len()).unwrap_or(u32::MAX));
    let issues = issues
        .into_iter()
        .take(MAX_ISSUES)
        .map(|issue| RenderIssue {
            code: issue.code,
            ids: issue.ids.into_iter().take(ISSUE_IDS).collect(),
            message: cut(issue.message, NOTE_CHARS),
        })
        .collect();
    let warnings = warnings.into_iter().take(MAX_WARNINGS).map(|warning| cut(warning, NOTE_CHARS)).collect();
    if wait.reply.is_closed() {
        // The door timed out or its agent left: nobody would read the file.
        return Err(timeout(wait.started.elapsed()));
    }
    let png = match picture {
        None => None,
        Some((bytes, w, h)) => Some(RenderPng { path: write_png(dir, id, &bytes)?.to_string_lossy().into_owned(), w, h }),
    };
    Ok(RenderOut {
        design_id: wait.design_id,
        rev,
        page: wait.page.clone(),
        area,
        scale,
        png,
        issues,
        issue_count,
        warnings,
        boxes,
        timings_ms,
        times_ms,
    })
}

/// What was drawn, checked against what was asked: the picture decoded with its size, none when
/// there is none or none was asked for, or why the answer cannot be used.
fn check_drawn(wait: &Wait, drawn: &RenderDrawn) -> Result<Option<(Vec<u8>, u32, u32)>, String> {
    if !(drawn.scale.is_finite() && drawn.scale > 0.0 && drawn.scale <= RENDER_SCALE_MAX) {
        return Err(format!("scale {} is outside 0 to {RENDER_SCALE_MAX}", drawn.scale));
    }
    let area = &drawn.area;
    if ![area.x, area.y, area.w, area.h].iter().all(|v| v.is_finite()) || area.w < 0.0 || area.h < 0.0 {
        return Err(format!("area {} {} {} {} is not finite, or its size is negative", area.x, area.y, area.w, area.h));
    }
    if !drawn.boxes.fields.iter().map(String::as_str).eq(RENDER_BOX_FIELDS) {
        return Err(format!("boxes.fields is {:?}, not {RENDER_BOX_FIELDS:?}", drawn.boxes.fields));
    }
    let rows = &drawn.boxes.rows;
    if rows.len() > wait.boxes_limit as usize {
        return Err(format!("{} box rows, over boxes_limit {}", rows.len(), wait.boxes_limit));
    }
    if let Some((at, row)) = rows.iter().enumerate().find(|(_, row)| row.len() != RENDER_BOX_FIELDS.len()) {
        return Err(format!("box row {at} has {} values, not {}", row.len(), RENDER_BOX_FIELDS.len()));
    }
    let frames = usize::from(wait.frames.unwrap_or(0));
    if drawn.times_ms.len() > frames {
        return Err(match frames {
            0 => "times_ms is for a contact sheet, and none was asked for".to_string(),
            _ => format!("times_ms has {} times for {frames} frames", drawn.times_ms.len()),
        });
    }
    if let Some(at) = drawn.times_ms.iter().find(|at| !(at.is_finite() && (0.0..=MAX_MS).contains(*at))) {
        return Err(format!("times_ms {at} is outside 0 to {MAX_MS}"));
    }
    let Some(encoded) = drawn.png_b64.as_deref().filter(|_| wait.png) else { return Ok(None) };
    // decode_upload's own text names bytes_b64, which this answer does not have.
    let bytes = picture::decode_upload(encoded, "design.render.failed")
        .map_err(|_| format!("png_b64 is not base64 or over {} MiB", picture::UPLOAD_MAX >> 20))?;
    let Some(png) = picture::raster(&bytes).filter(|picture| picture.ext == "png") else {
        return Err("png_b64 is not a PNG whose size can be read".to_string());
    };
    if png.w.max(png.h) > wait.max_side {
        return Err(format!("the picture is {}x{} px, over max_side {}", png.w, png.h, wait.max_side));
    }
    if u64::from(png.w) * u64::from(png.h) > RENDER_AREA_PX_MAX {
        return Err(format!("the picture is {}x{} px, over {RENDER_AREA_PX_MAX} pixels", png.w, png.h));
    }
    Ok(Some((bytes, png.w, png.h)))
}

/// The app's own failure, under its code when it is one of [`APP_CODES`].
fn app_failure(error: RenderFailure) -> BusError {
    let message = cut(error.message, FAILURE_CHARS);
    match error.code.as_str() {
        "design.render.no_surface" => BusError::unavailable(error.code, message)
            .with_hint("call again with png: false for the boxes and issues alone, or ask the person to show Relay's window"),
        "design.render.failed" => failed(message),
        code if APP_CODES.contains(&code) => BusError::not_found(error.code, message),
        _ => {
            let app_code = cut(error.code, 100);
            failed(message.clone()).with_details(json!({"reason": message, "app_code": app_code}))
        }
    }
}

/// `text` cut to `most` characters, the last of them an ellipsis when it was longer.
fn cut(text: String, most: usize) -> String {
    match text.char_indices().nth(most) {
        None => text,
        Some(_) => text.chars().take(most.saturating_sub(1)).chain(['…']).collect(),
    }
}

/// Write render `id`'s picture as `<id>.png` in the render folder, then sweep the folder.
fn write_png(dir: &Path, id: Uuid, bytes: &[u8]) -> Result<PathBuf, BusError> {
    private_dir(dir)?;
    let path = dir.join(format!("{id}.png"));
    let writing = |e: std::io::Error| failed(format!("writing the picture: {e}"));
    // 0600, and in place only once whole.
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(writing)?;
    file.write_all(bytes).map_err(writing)?;
    file.persist(&path).map_err(|e| writing(e.error))?;
    sweep(dir, SystemTime::now(), &path);
    Ok(path)
}

/// The render folder, made 0700 when missing, and refused unless it is a directory only this
/// user can reach: its pictures show the person's designs.
fn private_dir(dir: &Path) -> Result<(), BusError> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| failed(format!("making the render folder: {e}")))?;
    crate::paths::check_private_dir(dir).map_err(|e| failed(format!("the render folder is not private: {e}")))
}

/// Remove the render pictures past keeping: every one over [`KEEP_FOR`] old, then all but the
/// newest [`KEEP_FILES`], the picture just `written` always kept and counted. Only regular files
/// named `<uuid>.png` are looked at: never a symlink, another name or a folder.
fn sweep(dir: &Path, now: SystemTime, written: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut pngs: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path == written || !is_render_file_name(&path) {
                return None;
            }
            let meta = std::fs::symlink_metadata(&path).ok()?;
            meta.file_type().is_file().then_some(())?;
            Some((meta.modified().ok()?, path))
        })
        .collect();
    pngs.sort_unstable_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (newer, (modified, path)) in pngs.iter().enumerate() {
        let old = now.duration_since(*modified).is_ok_and(|age| age > KEEP_FOR);
        // `newer` others are newer than this one, and `written` is kept besides them.
        if old || newer + 1 >= KEEP_FILES {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// `<uuid>.png`, the uuid hyphenated as a render id is written.
fn is_render_file_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".png"))
        .is_some_and(|stem| stem.len() == 36 && Uuid::parse_str(stem).is_ok())
}

// ---- refusals, as render-SPEC-v2 §5.7 words them ----

const FAILED_HINT: &str = "call design.render once more; if it fails again, report details.reason";

/// `design.render.no_renderer`: no desktop app takes requests.
pub(crate) fn no_renderer() -> BusError {
    BusError::unavailable("design.render.no_renderer", "no desktop app is connected to draw the design")
        .with_hint("No desktop app is connected to this engine, so there is no picture and no measured size. Tell the person; until then design.get still reads the nodes, but hugged text reads w 0, h 0 there: do not guess sizes.")
}

/// `design.render.busy`: four renders already wait, or the app's queue is full.
pub(crate) fn busy(why: impl Into<String>) -> BusError {
    BusError::unavailable("design.render.busy", why).with_hint("call design.render again in a few seconds")
}

/// `design.render.timeout`: no answer within the wait.
pub(crate) fn timeout(waited: Duration) -> BusError {
    let waited_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
    BusError::unavailable("design.render.timeout", format!("the desktop app did not answer within {waited_ms} ms"))
        .with_details(json!({"waited_ms": waited_ms}))
        .with_hint("the desktop app may be busy: call design.render once more; if it times out again, carry on without the picture and say so")
}

/// `design.render.renderer_gone`: the app's connection ended while it drew.
// The socket door's (render-door), when a wait's sender drops.
#[allow(dead_code)]
pub(crate) fn renderer_gone() -> BusError {
    BusError::unavailable("design.render.renderer_gone", "the desktop app's connection closed while it drew the design")
        .with_hint("the desktop app closed: call design.render again once it is open")
}

/// `design.render.failed`, with `details.reason`.
pub(crate) fn failed(reason: impl Into<String>) -> BusError {
    let reason = reason.into();
    BusError::unavailable("design.render.failed", format!("the desktop app's render failed: {reason}"))
        .with_details(json!({"reason": reason}))
        .with_hint(FAILED_HINT)
}

/// `design.stale`: the design is not at the `base_rev` the agent gave, with `details.rev`, as
/// design.apply answers it.
pub(crate) fn stale(design_id: Id, rev: i64, base_rev: i64) -> BusError {
    BusError::conflict("design.stale", format!("design {design_id} is at rev {rev}, not {base_rev}"))
        .with_details(json!({"rev": rev}))
        .with_hint("the design moved on: read it again with design.get, then render without base_rev or with the new one")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Door;
    use base64::Engine as _;
    use relay_bus::error::ErrorKind;
    use relay_bus::ops::design::{RenderAnswer, RenderBoxes, RenderRect, RenderTimings};
    use relay_bus::registry::Op;
    use relay_bus::Request;
    use serde_json::Value;
    use std::os::unix::fs::PermissionsExt;

    /// A PNG's signature and header, which is all `picture::raster` reads: `w` by `h`.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend(w.to_be_bytes());
        bytes.extend(h.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        bytes
    }
    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn target() -> Target {
        Target { design_id: 12, page: "page-1".into(), nodes: None }
    }
    fn options() -> Options {
        Options {
            png: true,
            scale: 1.0,
            max_side: 1568,
            boxes_limit: 200,
            base_rev: None,
            timeout: Duration::from_secs(20),
            at_ms: None,
            frames: None,
            by: None,
        }
    }

    /// A desktop app on a render connection of its own: its lease, the queue the engine writes
    /// its lines to, and the connection's own sender, which keeps the lease's alive.
    struct App {
        lease: u64,
        lines: mpsc::Receiver<String>,
        tx: mpsc::Sender<String>,
    }
    fn app(renders: &Renders, fonts: Option<Vec<&str>>) -> App {
        let (tx, lines) = mpsc::channel(1024);
        let lease = renders.new_lease();
        renders.serve(lease, tx.downgrade(), fonts.map(|fonts| fonts.into_iter().map(String::from).collect()));
        App { lease, lines, tx }
    }

    fn row(id: &str) -> Vec<Value> {
        vec![json!(id), json!("rect"), json!("page-1"), json!(0), json!(0), json!(10), json!(10), json!(0), json!(0), json!(0)]
    }
    fn drawn(rev: i64) -> RenderDrawn {
        RenderDrawn {
            rev,
            area: RenderRect { x: -32.0, y: -32.0, w: 1.0, h: 1.0 },
            scale: 1.0,
            png_b64: Some(b64(&png(1, 1))),
            boxes: RenderBoxes { fields: RENDER_BOX_FIELDS.map(String::from).to_vec(), rows: vec![row("a"), row("b")], total: 2, hidden: 0 },
            issues: Vec::new(),
            issue_count: 0,
            warnings: Vec::new(),
            timings_ms: RenderTimings { read: 1.0, layout: 1.0, paint: 1.0, encode: 1.0 },
            times_ms: Vec::new(),
        }
    }
    fn drawn_answer(id: Uuid, drawn: RenderDrawn) -> AnswerIn {
        AnswerIn { render_id: id.to_string(), drawn: Some(drawn), error: None }
    }
    fn error_answer(id: Uuid, code: &str, message: &str) -> AnswerIn {
        AnswerIn { render_id: id.to_string(), drawn: None, error: Some(RenderFailure { code: code.into(), message: message.into() }) }
    }

    /// One render waiting with `options`, answered with what `answer` makes of its id: whether
    /// the answer was accepted, and what the door got.
    fn answered(dir: &Path, options: Options, answer: impl FnOnce(Uuid) -> AnswerIn) -> (bool, Reply) {
        let renders = Renders::default();
        let _app = app(&renders, None);
        let (id, mut rx) = renders.start(&target(), &options).unwrap();
        let out = answer_into(&renders, dir, answer(id));
        (out.accepted, rx.try_recv().expect("the door got an answer"))
    }

    /// Why an answer to a waiting render failed: accepted, `design.render.failed`, its reason.
    fn reason(dir: &Path, options: Options, answer: impl FnOnce(Uuid) -> AnswerIn) -> String {
        let (accepted, reply) = answered(dir, options, answer);
        assert!(accepted, "a render was waiting, so the answer is accepted");
        let error = reply.expect_err("the answer fails");
        assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Unavailable, "design.render.failed"), "{error:?}");
        assert_eq!(error.hint.as_deref(), Some(FAILED_HINT));
        error.details.unwrap()["reason"].as_str().unwrap().to_string()
    }

    fn renders_dir() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("design-renders");
        (root, dir)
    }

    #[test]
    fn an_answer_without_a_wait_is_not_accepted() {
        let (_root, dir) = renders_dir();
        let renders = Renders::default();
        for id in [Uuid::new_v4().to_string(), "not a render id".to_string()] {
            let out = answer_into(&renders, &dir, AnswerIn { render_id: id, drawn: Some(drawn(1)), error: None });
            assert!(!out.accepted);
        }
        // An answer takes its wait, so a second answer to the same render finds none.
        let _app = app(&renders, None);
        let (id, _rx) = renders.start(&target(), &options()).unwrap();
        assert!(answer_into(&renders, &dir, drawn_answer(id, drawn(1))).accepted);
        assert!(!answer_into(&renders, &dir, drawn_answer(id, drawn(1))).accepted);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "only the first answer wrote a picture");
    }

    #[test]
    fn an_answer_completes_its_wait_with_the_file_and_rows() {
        let (_root, dir) = renders_dir();
        let (accepted, reply) = answered(&dir, Options { base_rev: Some(15), ..options() }, |id| drawn_answer(id, drawn(15)));
        assert!(accepted);
        let out = reply.unwrap();
        assert_eq!((out.design_id, out.rev, out.page.as_str(), out.scale), (12, 15, "page-1", 1.0));
        assert_eq!(out.area, RenderRect { x: -32.0, y: -32.0, w: 1.0, h: 1.0 });
        let picture = out.png.unwrap();
        assert_eq!((picture.w, picture.h), (1, 1));
        assert_eq!(Path::new(&picture.path).parent(), Some(dir.as_path()));
        assert_eq!(std::fs::read(&picture.path).unwrap(), png(1, 1));
        assert_eq!(out.boxes.rows, vec![row("a"), row("b")]);
        assert_eq!((out.boxes.total, out.boxes.hidden), (2, 0));
        assert!(out.times_ms.is_empty());
    }

    #[test]
    fn a_picture_not_asked_for_is_not_written() {
        let (_root, dir) = renders_dir();
        let (accepted, reply) = answered(&dir, Options { png: false, ..options() }, |id| drawn_answer(id, drawn(1)));
        assert!(accepted);
        let out = reply.unwrap();
        assert_eq!(out.png, None);
        assert_eq!(out.boxes.rows.len(), 2);
        assert!(!dir.exists(), "nothing was written");
        // And a picture asked for but not drawn (nothing in scope is visible) is no failure.
        let (_, reply) = answered(&dir, options(), |id| drawn_answer(id, RenderDrawn { png_b64: None, ..drawn(1) }));
        assert_eq!(reply.unwrap().png, None);
    }

    #[test]
    fn accepted_means_a_render_was_waiting() {
        let (_root, dir) = renders_dir();
        let (accepted, reply) = answered(&dir, options(), |id| error_answer(id, "design.render.no_surface", "the window has no renderer"));
        assert!(accepted, "a refusal reaches the waiting render too");
        let error = reply.unwrap_err();
        assert_eq!((error.kind, error.code.as_str(), error.message.as_str()), (ErrorKind::Unavailable, "design.render.no_surface", "the window has no renderer"));
        assert!(error.hint.unwrap().contains("png: false"));
    }

    #[test]
    fn each_bad_answer_is_failed_with_its_reason() {
        let (_root, dir) = renders_dir();
        let with = |change: fn(&mut RenderDrawn)| {
            move |id| {
                let mut drawn = drawn(1);
                change(&mut drawn);
                drawn_answer(id, drawn)
            }
        };
        let both = |id| AnswerIn { error: Some(RenderFailure { code: "design.render.failed".into(), message: "no".into() }), ..drawn_answer(id, drawn(1)) };
        assert!(reason(&dir, options(), both).contains("gave both"));
        let neither = |id: Uuid| AnswerIn { render_id: id.to_string(), drawn: None, error: None };
        assert!(reason(&dir, options(), neither).contains("gave neither"));

        assert!(reason(&dir, options(), with(|d| d.png_b64 = Some("@@ not base64 @@".into()))).contains("png_b64 is not base64"));
        assert!(reason(&dir, options(), with(|d| d.png_b64 = Some(b64(&vec![0; picture::UPLOAD_MAX + 1])))).contains("over 4 MiB"));
        assert!(reason(&dir, options(), with(|d| d.png_b64 = Some(b64(b"GIF89a\x01\0\x01\0\0\0\0")))).contains("not a PNG"));
        assert!(reason(&dir, options(), with(|d| d.png_b64 = Some(b64(b"hello")))).contains("not a PNG"));
        let small = Options { max_side: 64, ..options() };
        assert!(reason(&dir, small, with(|d| d.png_b64 = Some(b64(&png(65, 10))))).contains("over max_side 64"));
        let big = Options { max_side: 2560, ..options() };
        assert!(reason(&dir, big, with(|d| d.png_b64 = Some(b64(&png(2560, 1563))))).contains("over 4000000 pixels"));

        let one_row = Options { boxes_limit: 1, ..options() };
        assert!(reason(&dir, one_row, with(|_| {})).contains("2 box rows, over boxes_limit 1"));
        assert!(reason(&dir, options(), with(|d| { d.boxes.rows[1].pop(); })).contains("box row 1 has 9 values, not 10"));
        assert!(reason(&dir, options(), with(|d| d.boxes.fields.swap(0, 1))).contains("boxes.fields"));
        for scale in [f64::NAN, f64::INFINITY, 0.0, -1.0, 2.5] {
            let wrong = move |id| drawn_answer(id, RenderDrawn { scale, ..drawn(1) });
            assert!(reason(&dir, options(), wrong).starts_with("scale"), "{scale}");
        }
        assert!(reason(&dir, options(), with(|d| d.area.x = f64::NAN)).starts_with("area"));
        assert!(reason(&dir, options(), with(|d| d.area.w = -1.0)).starts_with("area"));

        assert!(reason(&dir, options(), with(|d| d.times_ms = vec![0.0])).contains("none was asked for"));
        let sheet = || Options { frames: Some(2), ..options() };
        assert!(reason(&dir, sheet(), with(|d| d.times_ms = vec![0.0, 1.0, 2.0])).contains("3 times for 2 frames"));
        assert!(reason(&dir, sheet(), with(|d| d.times_ms = vec![0.0, -1.0])).contains("times_ms -1"));
        assert!(reason(&dir, sheet(), with(|d| d.times_ms = vec![0.0, MAX_MS + 1.0])).contains("outside 0 to 600000"));
        let (_, reply) = answered(&dir, sheet(), with(|d| d.times_ms = vec![0.0, 600.0]));
        assert_eq!(reply.unwrap().times_ms, vec![0.0, 600.0], "a contact sheet keeps its times");

        let pictures = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(pictures, 1, "only the good contact sheet wrote a picture");
    }

    #[test]
    fn a_rev_that_moved_is_stale() {
        let (_root, dir) = renders_dir();
        let (accepted, reply) = answered(&dir, Options { base_rev: Some(15), ..options() }, |id| drawn_answer(id, drawn(16)));
        assert!(accepted);
        let error = reply.unwrap_err();
        assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Conflict, "design.stale"));
        assert_eq!(error.details, Some(json!({"rev": 16})));
        assert!(error.hint.unwrap().contains("design.get"));
        assert!(!dir.exists(), "a stale picture is not written");
        // Without base_rev, whatever rev was drawn is the answer's.
        let (_, reply) = answered(&dir, options(), |id| drawn_answer(id, drawn(16)));
        assert_eq!(reply.unwrap().rev, 16);
    }

    #[test]
    fn an_app_code_outside_the_list_is_failed_with_app_code() {
        let (_root, dir) = renders_dir();
        let (_, reply) = answered(&dir, options(), |id| error_answer(id, "gtk.cairo", "cairo ran out of memory"));
        let error = reply.unwrap_err();
        assert_eq!(error.code, "design.render.failed");
        assert_eq!(error.details, Some(json!({"reason": "cairo ran out of memory", "app_code": "gtk.cairo"})));

        let (_, reply) = answered(&dir, options(), |id| error_answer(id, "design.render.failed", "the texture did not download"));
        let error = reply.unwrap_err();
        assert_eq!(error.details, Some(json!({"reason": "the texture did not download"})));

        for code in ["design.not_found", "design.page_not_found", "design.node_not_found"] {
            let (_, reply) = answered(&dir, options(), |id| error_answer(id, code, "it went"));
            let error = reply.unwrap_err();
            assert_eq!((error.kind, error.code.as_str(), error.message.as_str()), (ErrorKind::NotFound, code, "it went"));
        }
    }

    #[test]
    fn long_texts_are_cut_not_refused() {
        let (_root, dir) = renders_dir();
        let long = |id| {
            let mut drawn = drawn(1);
            drawn.issues = (0..150)
                .map(|at| RenderIssue { code: "overlap".into(), ids: (0..20).map(|n| format!("n{at}-{n}")).collect(), message: "é".repeat(1000) })
                .collect();
            drawn.issue_count = 120; // fewer than it lists: the count is at least what was found
            drawn.warnings = (0..30).map(|_| "w".repeat(400)).collect();
            drawn_answer(id, drawn)
        };
        let out = answered(&dir, options(), long).1.unwrap();
        assert_eq!((out.issues.len(), out.issue_count), (100, 150));
        assert!(out.issues.iter().all(|issue| issue.ids.len() == 8 && issue.message.chars().count() == 300 && issue.message.ends_with('…')));
        assert_eq!(out.issues[0].ids[7], "n0-7");
        assert_eq!(out.warnings.len(), 20);
        assert!(out.warnings.iter().all(|warning| warning.chars().count() == 300));
        assert!(out.png.is_some());

        let (_, reply) = answered(&dir, options(), |id| error_answer(id, "design.render.failed", &"x".repeat(2000)));
        let reason = reply.unwrap_err().details.unwrap()["reason"].as_str().unwrap().to_string();
        assert_eq!(reason.chars().count(), 500);
        // Short texts stay as they are.
        assert_eq!(cut("short".into(), 300), "short");
        assert_eq!(cut("exactly".into(), 7), "exactly");
        assert_eq!(cut("exactly!".into(), 7), "exactl…");
    }

    #[test]
    fn an_answer_after_the_door_left_writes_no_file() {
        let (_root, dir) = renders_dir();
        let renders = Renders::default();
        let _app = app(&renders, None);
        let (id, rx) = renders.start(&target(), &options()).unwrap();
        // The door timed out or its agent left, and has not yet forgotten the wait.
        drop(rx);
        assert!(!answer_into(&renders, &dir, drawn_answer(id, drawn(1))).accepted);
        assert!(!dir.exists());
        assert!(renders.inner.lock().unwrap().waits.is_empty(), "the answer took the wait");
    }

    #[test]
    fn the_file_is_0600_in_a_0700_folder_named_by_the_render_id() {
        let (_root, dir) = renders_dir();
        let renders = Renders::default();
        let _app = app(&renders, None);
        let (id, mut rx) = renders.start(&target(), &options()).unwrap();
        assert!(answer_into(&renders, &dir, drawn_answer(id, drawn(1))).accepted);
        let path = PathBuf::from(rx.try_recv().unwrap().unwrap().png.unwrap().path);
        assert_eq!(path, dir.join(format!("{id}.png")));
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file is left");
    }

    #[test]
    fn a_folder_open_to_others_is_refused() {
        let (root, dir) = renders_dir();
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let why = reason(&dir, options(), |id| drawn_answer(id, drawn(1)));
        assert!(why.contains("the render folder is not private"), "{why}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);

        // Nor a link to a private folder elsewhere.
        let elsewhere = root.path().join("elsewhere");
        std::fs::DirBuilder::new().mode(0o700).create(&elsewhere).unwrap();
        let linked = root.path().join("linked");
        std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
        let why = reason(&linked, options(), |id| drawn_answer(id, drawn(1)));
        assert!(why.contains("not private"), "{why}");
        assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
    }

    #[test]
    fn sweep_removes_old_and_extra_pngs_only() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        let now = SystemTime::now();
        let made = |name: String, age: Duration| {
            let path = dir.join(name);
            std::fs::write(&path, b"png").unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(now - age).unwrap();
            path
        };
        let minutes = |n: u64| Duration::from_secs(60 * n);
        // 40 renders a minute apart, newest first; and the one just written, dated long ago by a
        // clock that moved, which is kept all the same.
        let renders: Vec<PathBuf> = (1..=40).map(|n| made(format!("{}.png", Uuid::new_v4()), minutes(n))).collect();
        let written = made(format!("{}.png", Uuid::new_v4()), minutes(600));
        // Never looked at: other names, an old one among them, a link, a folder.
        let others = [made("notes.txt".into(), minutes(600)), made("render.png".into(), minutes(600)), made(format!("{}.PNG", Uuid::new_v4()), minutes(600))];
        let outside = root.path().join("outside.png");
        let target_dir = tempfile::tempdir().unwrap();
        std::fs::write(target_dir.path().join("keep.png"), b"png").unwrap();
        let link = dir.join(format!("{}.png", Uuid::new_v4()));
        std::os::unix::fs::symlink(target_dir.path().join("keep.png"), &link).unwrap();
        let folder = dir.join(format!("{}.png", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();

        sweep(dir, now, &written);

        assert!(written.exists(), "the picture just written stays, whatever its date");
        let kept: Vec<&PathBuf> = renders.iter().filter(|path| path.exists()).collect();
        assert_eq!(kept.len(), KEEP_FILES - 1, "with the one just written, the newest 32");
        assert_eq!(kept, renders.iter().take(KEEP_FILES - 1).collect::<Vec<_>>(), "the newest are the ones kept");
        assert!(others.iter().all(|path| path.exists()), "other names are left");
        assert!(std::fs::symlink_metadata(&link).is_ok() && target_dir.path().join("keep.png").exists(), "a link is left, and what it points at");
        assert!(folder.is_dir());
        assert!(!outside.exists());

        // An hour's age takes a render however few there are.
        let few = tempfile::tempdir().unwrap();
        let in_few = |name: String, age: Duration| {
            let path = few.path().join(name);
            std::fs::write(&path, b"png").unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(now - age).unwrap();
            path
        };
        let old = in_few(format!("{}.png", Uuid::new_v4()), minutes(61));
        let fresh = in_few(format!("{}.png", Uuid::new_v4()), minutes(59));
        sweep(few.path(), now, &few.path().join("just-written.png"));
        assert!(!old.exists() && fresh.exists());
        // A missing folder is nothing to sweep.
        sweep(&dir.join("missing"), now, &written);
    }

    #[test]
    fn the_request_line_is_an_event_for_its_connection_only() {
        let renders = Renders::default();
        let mut older = app(&renders, None);
        let mut newest = app(&renders, None);
        let options = Options { at_ms: Some(500.0), by: Some("brisk-otter".into()), ..options() };
        let target = Target { nodes: Some(vec!["hero".into()]), ..target() };
        let before = unix_ms(SystemTime::now());
        let (id, _rx) = renders.start(&target, &options).unwrap();
        let after = unix_ms(SystemTime::now());
        let line: Value = serde_json::from_str(&newest.lines.try_recv().unwrap()).unwrap();
        assert!(older.lines.try_recv().is_err(), "only the newest lease hears it");
        assert_eq!((line["v"].as_u64(), line["ev"].as_str(), line["actor"].as_str()), (Some(1), Some("design.render.request"), Some("system")));
        let request: RenderRequest = serde_json::from_value(line["payload"].clone()).unwrap();
        let deadline = request.deadline_unix_ms;
        assert!((before + 19_500..=after + 19_500).contains(&deadline), "{deadline} is the timeout less half a second from now");
        assert_eq!(
            request,
            RenderRequest {
                render_id: id.to_string(),
                design_id: 12,
                page: "page-1".into(),
                nodes: Some(vec!["hero".into()]),
                png: true,
                scale: 1.0,
                max_side: 1568,
                boxes_limit: 200,
                deadline_unix_ms: deadline,
                at_ms: Some(500.0),
                frames: None,
                by: Some("brisk-otter".into()),
            }
        );
        assert!(line["payload"].get("frames").is_none(), "what was not asked is left out");
    }

    #[test]
    fn serving_again_makes_a_lease_the_newest() {
        let renders = Renders::default();
        let mut first = app(&renders, None);
        let mut second = app(&renders, None);
        renders.serve(first.lease, first.tx.downgrade(), None);
        renders.start(&target(), &options()).unwrap();
        assert!(first.lines.try_recv().is_ok() && second.lines.try_recv().is_err());
        assert_eq!(renders.inner.lock().unwrap().renderers.len(), 2, "one entry per lease");
        assert_ne!(renders.new_lease(), renders.new_lease());
    }

    #[test]
    fn release_ends_its_waits() {
        let renders = Renders::default();
        let mut older = app(&renders, None);
        let newest = app(&renders, None);
        let (_, mut on_newest) = renders.start(&target(), &options()).unwrap();
        renders.release(newest.lease);
        assert!(matches!(on_newest.try_recv(), Err(oneshot::error::TryRecvError::Closed)), "the door sees its sender drop: renderer_gone");
        assert!(renders.inner.lock().unwrap().waits.is_empty());
        // The older lease takes the next render, and keeps its own waits when another goes.
        let (_, mut on_older) = renders.start(&target(), &options()).unwrap();
        assert!(older.lines.try_recv().is_ok());
        renders.release(newest.lease);
        assert!(matches!(on_older.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
        renders.release(older.lease);
        assert_eq!(renders.start(&target(), &options()).unwrap_err().code, "design.render.no_renderer");
    }

    #[test]
    fn a_closed_or_dropped_renderer_is_skipped_for_the_older_one() {
        let renders = Renders::default();
        let mut oldest = app(&renders, None);
        // Its writer ended (the socket broke): the queue is closed.
        let broken = app(&renders, None);
        drop(broken.lines);
        // Its connection dropped its own sender and has not yet released its lease.
        let ending = app(&renders, None);
        drop(ending.tx);
        let (_, _rx) = renders.start(&target(), &options()).unwrap();
        assert!(oldest.lines.try_recv().is_ok(), "the oldest live lease draws");
        assert_eq!(renders.inner.lock().unwrap().renderers.iter().map(|r| r.lease).collect::<Vec<_>>(), vec![oldest.lease], "the gone ones are forgotten");
        let waits: Vec<u64> = renders.inner.lock().unwrap().waits.values().map(|wait| wait.lease).collect();
        assert_eq!(waits, vec![oldest.lease]);

        drop(oldest.lines);
        let error = renders.start(&target(), &options()).unwrap_err();
        assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Unavailable, "design.render.no_renderer"));
        assert!(error.hint.unwrap().contains("do not guess sizes"));
        assert_eq!(renders.inner.lock().unwrap().waits.len(), 1, "a refusal leaves no wait");
        // With no app at all, the same.
        assert_eq!(Renders::default().start(&target(), &options()).unwrap_err().code, "design.render.no_renderer");
    }

    #[test]
    fn a_full_queue_is_busy() {
        let renders = Renders::default();
        let mut older = app(&renders, None);
        let (tx, _lines) = mpsc::channel::<String>(1);
        tx.try_send("a line the writer has not sent yet".into()).unwrap();
        let lease = renders.new_lease();
        renders.serve(lease, tx.downgrade(), None);
        let error = renders.start(&target(), &options()).unwrap_err();
        assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Unavailable, "design.render.busy"));
        assert!(error.message.contains("queue is full"));
        assert!(older.lines.try_recv().is_err(), "a full queue is no reason to ask another app");
        assert!(renders.inner.lock().unwrap().waits.is_empty());
        assert_eq!(renders.inner.lock().unwrap().renderers.len(), 2, "a busy app keeps its lease");
    }

    #[test]
    fn a_fifth_wait_is_busy() {
        let renders = Renders::default();
        let mut app = app(&renders, None);
        let ids: Vec<Uuid> = (0..MAX_WAITS).map(|_| renders.start(&target(), &options()).unwrap().0).collect();
        let error = renders.start(&target(), &options()).unwrap_err();
        assert_eq!(error.code, "design.render.busy");
        assert!(error.hint.unwrap().contains("again in a few seconds"));
        let mut lines = 0;
        while app.lines.try_recv().is_ok() {
            lines += 1;
        }
        assert_eq!(lines, MAX_WAITS, "the busy render sent nothing");
        renders.cancel(ids[0]);
        assert!(renders.start(&target(), &options()).is_ok(), "a cancelled wait frees its place");
    }

    #[test]
    fn fonts_are_the_newest_connected_apps() {
        let renders = Renders::default();
        assert_eq!(renders.fonts(), None);
        let long = "x".repeat(MAX_NAME + 1);
        let first = app(&renders, Some(vec!["Sora", "Geist", "Geist", "  ", "", &long, "Bell\u{7}", "Geist Mono"]));
        let names = |fonts: Option<Arc<Vec<String>>>| fonts.map(|fonts| fonts.as_ref().clone());
        assert_eq!(names(renders.fonts()), Some(vec!["Geist".to_string(), "Geist Mono".into(), "Sora".into()]), "sorted, each once, the bad names left out");
        let _quiet = app(&renders, None);
        assert_eq!(names(renders.fonts()).unwrap().len(), 3, "an app that reported none passes to the next");
        let newest = app(&renders, Some(vec!["Inter"]));
        assert_eq!(names(renders.fonts()), Some(vec!["Inter".to_string()]));
        drop(newest.tx);
        assert_eq!(names(renders.fonts()).unwrap().len(), 3, "a connection that let go of its sender is not asked");
        renders.release(first.lease);
        assert_eq!(renders.fonts(), None);

        let many: Vec<String> = (0..SERVE_FONTS_MAX + 5).map(|n| format!("Family {n:05}")).collect();
        let lease = renders.new_lease();
        let (tx, _lines) = mpsc::channel::<String>(1);
        renders.serve(lease, tx.downgrade(), Some(many));
        assert_eq!(renders.fonts().unwrap().len(), SERVE_FONTS_MAX, "a long list is cut, and the app still draws");
    }

    #[test]
    fn the_answer_is_registered_unlocked_and_needs_no_store_lock() {
        let root = tempfile::tempdir().unwrap();
        let engine = Engine::new(crate::Instance::Test, crate::Store::open(&root.path().join("relay.db"), false).unwrap());
        assert!(engine.runs_unlocked(RenderAnswer::NAME));
        let _app = app(&engine.renders, None);
        let (id, mut rx) = engine.renders.start(&target(), &options()).unwrap();
        let request = Request::new(Actor::User, RenderAnswer::NAME, serde_json::to_value(drawn_answer(id, drawn(1))).unwrap());
        // The store stays locked throughout: an answer must never wait on it.
        let held = engine.store.lock();
        let (done, answered) = std::sync::mpsc::channel();
        let e = engine.clone();
        std::thread::spawn(move || done.send(e.dispatch(request, Door::Socket)).unwrap());
        let response = answered.recv_timeout(Duration::from_secs(10)).expect("answered with the store locked");
        drop(held);
        assert!(response.ok, "{response:?}");
        assert_eq!(response.result, Some(json!({"accepted": true})));
        let out = rx.try_recv().unwrap().unwrap();
        assert_eq!(PathBuf::from(out.png.unwrap().path), root.path().join("design-renders").join(format!("{id}.png")));

        let unknown = Request::new(Actor::User, RenderAnswer::NAME, json!({"render_id": Uuid::new_v4().to_string(), "drawn": drawn(1)}));
        assert_eq!(engine.dispatch(unknown, Door::Socket).result, Some(json!({"accepted": false})));
    }
}
