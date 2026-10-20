//! The desktop renderer for `design.render` (native-render; render-SPEC-v2 §6): the lease on a
//! connection of its own, and the job that reads, lays out, paints and answers.
//!
//! The app offers to draw with `design.render.serve` on a connection that carries nothing else,
//! so a request is never read behind the main connection's notices and an answer never waits
//! behind its locked requests (§6.2). The engine writes each request to that connection alone.
//! Jobs run one at a time in arrival order, at idle priority: every main-thread step (layout,
//! the render node, `render_texture` and its download) follows a deadline check and a yield, so
//! a frame can paint between them; decoding, the PNG and base64 run off the main thread (§6.3).
//! The job always reads the design afresh and draws with a painter of its own, never a board's
//! (§6.4). An agent's render says so on every open board showing that design (decision 3).

use super::export;
use super::paint::Painter;
use crate::app::Ui;
use crate::client::{Client, Error, Notice};
use base64::Engine as _;
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4 as gtk;
use relay_bus::design::{NodeKind, Styles, MAX_NAME};
use relay_bus::envelope::Event;
use relay_bus::ops::design::{AnswerIn, GetOut, RenderBoxes, RenderDrawn, RenderFailure, RenderIssue, RenderRect, RenderRequest, RenderTimings, SERVE_FONTS_MAX};
use relay_client::design::{boxes, layout, Scene};
use serde_json::json;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The line the engine writes to the leased connection.
const REQUEST: &str = "design.render.request";
/// Most bytes of a PNG the engine takes (its `picture::UPLOAD_MAX`).
const PNG_MAX: usize = 4 << 20;
/// How long a job waits for its pictures to load, never past its deadline (R3).
const PICTURE_WAIT: Duration = Duration::from_secs(5);
/// The longest line the render connection reads: a whole design's `design.get` (1.5 MiB stored)
/// with room to spare.
const MAX_LINE: usize = 8 << 20;
/// The wait before the lease is offered again after its connection failed, doubling to the
/// most.
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_MOST: Duration = Duration::from_secs(30);
/// What an answer lists, as the engine keeps them; cutting is not a failure.
const MAX_WARNINGS: usize = 20;
const ISSUE_IDS: usize = 8;
const NOTE_CHARS: usize = 300;
/// Why a request for a time of the motion, or a contact sheet, has no picture yet.
const NO_MOTION: &str = "this desktop app cannot play motion yet";

thread_local! {
    /// The running lease; a new one replaces it.
    static LEASE: RefCell<Option<glib::JoinHandle<()>>> = const { RefCell::new(None) };
    /// The render painter, made on first use: one per process, kept warm across an agent's
    /// apply-render loop and pruned to the last design after each job (§6.3 R10).
    static PAINTER: RefCell<Option<Rc<Painter>>> = const { RefCell::new(None) };
}

/// Offer to draw designs for agents on a connection of its own, replacing any lease this app
/// held. The connection reconnects by itself when it drops; the main connection calls this again
/// after each reconnect of its own (§6.2).
pub(crate) fn lease(ui: &Rc<Ui>) {
    let weak = Rc::downgrade(ui);
    let run = async move {
        let mut wait = RETRY_FIRST;
        loop {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let (rt, path, fonts) = (ui.rt.clone(), ui.path.clone(), serve_fonts(families(&ui)));
            drop(ui);
            match Client::connect_with_limit(&rt, path, MAX_LINE).await {
                Ok((client, notices)) => match client.request(&rt, "design.render.serve", json!({"fonts": fonts})).await {
                    Ok(_) => {
                        wait = RETRY_FIRST;
                        serve(&weak, &client, &notices).await;
                    }
                    Err(error) => tracing::warn!(%error, "the desktop app could not offer to draw designs for agents"),
                },
                Err(error) => tracing::debug!(%error, "the render connection could not open"),
            }
            glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, wait).await;
            wait = next_wait(wait);
        }
    };
    let handle = glib::MainContext::default().spawn_local_with_priority(glib::Priority::DEFAULT_IDLE, run);
    if let Some(old) = LEASE.with(|l| l.replace(Some(handle))) {
        // Its `Client` drops with it, which closes its socket: the engine forgets that lease.
        old.abort();
    }
}

/// The wait before the next try, after `wait`.
fn next_wait(wait: Duration) -> Duration {
    (wait * 2).min(RETRY_MOST)
}

/// The font families this app draws, as the window's Pango context lists them (Relay's bundled
/// faces included, `fonts::install`).
fn families(ui: &Ui) -> Vec<String> {
    ui.window.pango_context().list_families().iter().map(|f| f.name().to_string()).collect()
}

/// The families `design.render.serve` reports (W68): each 1 to 200 characters with no control
/// character, sorted, each once, at most `SERVE_FONTS_MAX`, so one odd name never costs the
/// lease.
fn serve_fonts(names: Vec<String>) -> Vec<String> {
    let mut names: Vec<String> = names
        .into_iter()
        .filter(|n| !n.trim().is_empty() && n.chars().count() <= MAX_NAME && !n.chars().any(char::is_control))
        .collect();
    names.sort_unstable();
    names.dedup();
    names.truncate(SERVE_FONTS_MAX);
    names
}

/// Take the requests on the leased connection one at a time, until it closes. Awaiting a
/// reply inside this loop is safe (RA-554): the only notices are requests, the engine holds at
/// most four at once, and one past its deadline is dropped without a call.
async fn serve(ui: &Weak<Ui>, client: &Client, notices: &async_channel::Receiver<Notice>) {
    while let Ok(notice) = notices.recv().await {
        let Notice::Event(ev) = notice else {
            // Disconnected: the lease went with the connection.
            break;
        };
        let Some(req) = request_of(&ev) else {
            continue;
        };
        if expired(&req, now_ms()) {
            // The engine has already answered the agent.
            continue;
        }
        let Some(ui) = ui.upgrade() else {
            break;
        };
        let drawn = job(&ui, client, &req).await;
        let Some(drawn) = drawn.filter(|_| !expired(&req, now_ms())) else {
            continue;
        };
        let looked = drawn.is_ok();
        let answer = answer_of(&req.render_id, drawn);
        // The picture's base64 is the bulk of the answer: it is put in a `Value` off the main
        // thread, and written by the connection's own writer task.
        let payload = ui.rt.spawn_blocking(move || serde_json::to_value(answer)).await;
        let Ok(Ok(payload)) = payload else {
            continue;
        };
        if let Err(error) = client.request(&ui.rt, "design.render.answer", payload).await {
            tracing::warn!(%error, "the desktop app's render answer did not reach the engine");
        }
        if looked {
            if let Some(note) = req.by.as_deref().and_then(looked_note) {
                for b in super::boards_showing(req.design_id) {
                    b.toast(&note);
                }
            }
        }
    }
}

/// A request line's payload, if `ev` is one.
fn request_of(ev: &Event) -> Option<RenderRequest> {
    (ev.ev == REQUEST).then(|| serde_json::from_value(ev.payload.clone()).ok()).flatten()
}

/// Whether the engine has stopped waiting for `req`: then nobody reads what is drawn.
fn expired(req: &RenderRequest, now_ms: i64) -> bool {
    now_ms >= req.deadline_unix_ms
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// What an open board says when agent session `by` rendered its design (decision 3); nothing
/// for a blank name. The person's own renders carry no `by`.
fn looked_note(by: &str) -> Option<String> {
    let by = by.trim();
    (!by.is_empty()).then(|| format!("{by} looked at this design"))
}

/// The render painter, made on first use from the main window, so text measures as the board
/// measures it.
fn painter(ui: &Ui) -> Rc<Painter> {
    PAINTER.with(|p| p.borrow_mut().get_or_insert_with(|| Painter::new(&ui.window)).clone())
}

/// After the main loop's pending input, layout and frame work.
async fn yield_now() {
    glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, Duration::ZERO).await;
}

/// `RELAY_DESIGN_TRACE` set: each main-thread step prints `DESIGN_TRACE render-<step> <ms> ms`,
/// the format the smoke reads.
fn trace(step: &str, since: Instant) {
    thread_local! {
        static ON: bool = std::env::var_os("RELAY_DESIGN_TRACE").is_some();
    }
    if ON.with(|on| *on) {
        println!("DESIGN_TRACE render-{step} {:.3} ms", ms(since));
    }
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e3
}

/// One request: what was drawn, or why not; `None` when its deadline passed on the way.
async fn job(ui: &Rc<Ui>, client: &Client, req: &RenderRequest) -> Option<Result<RenderDrawn, RenderFailure>> {
    if let Some(refused) = motion_refused(req) {
        return Some(Err(refused));
    }
    // R1, R2: a fresh read, decoded and built off the main thread.
    let started = Instant::now();
    let got = match client.request(&ui.rt, "design.get", json!({"design_id": req.design_id, "page": req.page})).await {
        Ok(got) => got,
        Err(e) => return Some(Err(failure_of(&e))),
    };
    let built = ui.rt.spawn_blocking(move || -> Result<(i64, String, Scene), String> {
        let got: GetOut = serde_json::from_value(got).map_err(|e| e.to_string())?;
        let scene = Scene::new(got.nodes).with_styles(Styles { colors: got.colors, styles: got.styles });
        Ok((got.design.rev, got.assets_dir, scene))
    });
    let (rev, assets_dir, scene) = match built.await.map_err(|e| e.to_string()).and_then(|r| r) {
        Ok(built) => built,
        Err(e) => return Some(Err(failed(format!("the design could not be read: {e}")))),
    };
    let read = ms(started);
    let painter = painter(ui);
    let drawn = match missing_node(&scene, req.nodes.as_deref().unwrap_or_default()) {
        Some(missing) => Some(Err(missing)),
        None => draw(ui, &painter, req, &scene, rev, &assets_dir, read).await,
    };
    // R10: the painter holds one design's layouts, paths and pictures, answered or not; a
    // picture that failed is tried again next time.
    painter.retain(&scene);
    painter.retain_pictures(&scene, true);
    drawn
}

/// R3 to R8 on a fresh `scene` at `rev`.
async fn draw(ui: &Rc<Ui>, painter: &Rc<Painter>, req: &RenderRequest, scene: &Scene, rev: i64, assets_dir: &str, read: f64) -> Option<Result<RenderDrawn, RenderFailure>> {
    let page = req.page.as_str();
    let ids = req.nodes.clone().unwrap_or_default();
    let mut warnings = Vec::new();
    // R3: pictures decode off the main thread; wait for them as export does, so the agent does
    // not see grey placeholders.
    *painter.assets_dir.borrow_mut() = assets_dir.to_string();
    if req.png {
        let roots = if ids.is_empty() { vec![page.to_string()] } else { ids.clone() };
        let waited = Instant::now();
        while roots.iter().any(|r| painter.loading_under(scene, r)) && waited.elapsed() < PICTURE_WAIT && !expired(req, now_ms()) {
            glib::timeout_future_with_priority(glib::Priority::DEFAULT_IDLE, Duration::from_millis(50)).await;
        }
        warnings.extend(placeholders_note(unpictured(painter, scene, &roots, assets_dir)));
    }
    // R4: the page laid out as a still shows it, every scrolling frame at its start.
    if expired(req, now_ms()) {
        return None;
    }
    yield_now().await;
    let t = Instant::now();
    let placed = layout::place(scene, page, &**painter);
    trace("layout", t);
    let layout_ms = ms(t);
    // R5: boxes, issues and the area; the area and the issues measure, so they stay here.
    yield_now().await;
    let t = Instant::now();
    let area = export::area_of(painter, scene, &placed, page, &ids);
    let boxes = boxes::boxes(scene, &placed, page, &ids, req.boxes_limit as usize);
    let (issues, issue_count) = boxes::issues(scene, &placed, &**painter, page, &ids);
    trace("boxes", t);
    warnings.extend(boxes::more_boxes(&boxes));
    let Some(area) = area else {
        warnings.push(nothing_visible(&ids).to_string());
        let timings = RenderTimings { read, layout: layout_ms, paint: 0.0, encode: 0.0 };
        return Some(Ok(shaped(rev, RenderRect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }, req.scale, None, boxes, issues, issue_count, warnings, timings)));
    };
    let mut scale = boxes::fit_scale(&area, req.scale, req.max_side, boxes::MAX_AREA_PX);
    if scale < req.scale - 1e-9 {
        warnings.push(format!("drawn at {scale:.2}×, to fit max_side {} and 4 megapixels", req.max_side));
    }
    let mut timings = RenderTimings { read, layout: layout_ms, paint: 0.0, encode: 0.0 };
    if !req.png {
        return Some(Ok(shaped(rev, boxes::render_rect(&area), scale, None, boxes, issues, issue_count, warnings, timings)));
    }
    let mut png_b64 = None;
    for retry in [false, true] {
        // R6: the render node.
        if expired(req, now_ms()) {
            return None;
        }
        yield_now().await;
        let t = Instant::now();
        let Some((node, area)) = export::picture_of(painter, scene, &placed, page, &ids, true, scale) else {
            warnings.push(nothing_visible(&ids).to_string());
            break;
        };
        trace("content", t);
        timings.paint += ms(t);
        // R7: drawn by the window's renderer, and downloaded here, so the worker does CPU work
        // only, whatever the renderer.
        if expired(req, now_ms()) {
            return None;
        }
        yield_now().await;
        let t = Instant::now();
        let Some(renderer) = ui.window.renderer() else {
            return Some(Err(RenderFailure { code: "design.render.no_surface".into(), message: "Relay's window has no renderer: it is not on screen".into() }));
        };
        let (w, h) = boxes::picture_size(&area, scale);
        let texture = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w as f32, h as f32)));
        let downloader = gdk::TextureDownloader::new(&texture);
        let format = downloader.format();
        let (bytes, stride) = downloader.download_bytes();
        trace("paint", t);
        timings.paint += ms(t);
        // R8: the PNG and its base64 off the main thread; only one that fits is encoded.
        let t = Instant::now();
        let encoded = ui.rt.spawn_blocking(move || {
            let png = gdk::MemoryTexture::new(w as i32, h as i32, format, &bytes, stride).save_to_png_bytes();
            let size = png.len();
            (size, (size <= PNG_MAX).then(|| base64::engine::general_purpose::STANDARD.encode(&png)))
        });
        let Ok((size, b64)) = encoded.await else {
            return Some(Err(failed("the picture could not be encoded".into())));
        };
        timings.encode += ms(t);
        if let Some(b64) = b64 {
            png_b64 = Some(b64);
            break;
        }
        if retry {
            return Some(Err(failed(format!("the picture is {size} bytes as a PNG even at {scale:.2}×, over {} MiB", PNG_MAX >> 20))));
        }
        scale = retry_scale(scale, size);
        warnings.push(format!("drawn at {scale:.2}×, as a PNG at most {} MiB", PNG_MAX >> 20));
    }
    Some(Ok(shaped(rev, boxes::render_rect(&area), scale, png_b64, boxes, issues, issue_count, warnings, timings)))
}

/// A time of the motion or a contact sheet: refused until this app can pose a design
/// (native-motion-export).
fn motion_refused(req: &RenderRequest) -> Option<RenderFailure> {
    (req.at_ms.is_some() || req.frames.is_some()).then(|| failed(NO_MOTION.into()))
}

/// The first of `nodes` the fresh read does not hold: removed after the engine's check.
fn missing_node(scene: &Scene, nodes: &[String]) -> Option<RenderFailure> {
    let id = nodes.iter().find(|id| scene.node(id).is_none())?;
    Some(RenderFailure { code: "design.node_not_found".into(), message: format!("node {id} is not in the design any more") })
}

/// The failure an answer carries for a call that failed: the engine's own code when the design
/// or its page went away after its check, else `design.render.failed`.
fn failure_of(error: &Error) -> RenderFailure {
    match error {
        Error::Bus(e) if matches!(e.code.as_str(), "design.not_found" | "design.page_not_found" | "design.node_not_found") => {
            RenderFailure { code: e.code.clone(), message: e.message.clone() }
        }
        e => failed(format!("the design could not be read: {e}")),
    }
}

fn failed(message: String) -> RenderFailure {
    RenderFailure { code: "design.render.failed".into(), message }
}

/// The scale to draw at once more when a picture at `scale` was `size` bytes as a PNG, over
/// the cap: its pixels cut in proportion, with a tenth to spare (§6.3 R8).
fn retry_scale(scale: f64, size: usize) -> f64 {
    scale * (PNG_MAX as f64 / size.max(1) as f64).sqrt().min(1.0) * 0.9
}

/// The image nodes under `roots` that are drawn as placeholders: still loading, or with no file
/// to read.
fn unpictured(painter: &Painter, scene: &Scene, roots: &[String], assets_dir: &str) -> usize {
    let dir = std::path::Path::new(assets_dir);
    let unread = |id: &str, src: Option<&str>| src.is_none_or(|s| s.is_empty() || !dir.join(s).is_file()) || painter.loading_under(scene, id);
    roots
        .iter()
        .flat_map(|r| scene.subtree(r))
        .filter(|id| scene.node(id).is_some_and(|n| n.kind == NodeKind::Image && !n.is_hidden() && unread(id, n.src.as_deref())))
        .count()
}

/// The warning for `count` pictures drawn as placeholders, if any.
fn placeholders_note(count: usize) -> Option<String> {
    let (what, are) = if count == 1 { ("picture", "is") } else { ("pictures", "are") };
    (count > 0).then(|| format!("{count} {what} {are} drawn as a grey placeholder: still loading or not on this machine; render again to try once more"))
}

/// The warning when nothing in scope is visible, so there is no picture.
fn nothing_visible(ids: &[String]) -> &'static str {
    if ids.is_empty() {
        "the page has no visible layers"
    } else {
        "nothing in nodes is visible: they are hidden, wholly clear or empty"
    }
}

/// What was drawn, as the engine takes it: [`boxes::FIELDS`] rows, issues and warnings cut to
/// what it lists.
#[allow(clippy::too_many_arguments)]
fn shaped(rev: i64, area: RenderRect, scale: f64, png_b64: Option<String>, boxes: RenderBoxes, issues: Vec<RenderIssue>, issue_count: u32, warnings: Vec<String>, timings_ms: RenderTimings) -> RenderDrawn {
    let issues = issues
        .into_iter()
        .take(boxes::MAX_ISSUES)
        .map(|i| RenderIssue { code: i.code, ids: i.ids.into_iter().take(ISSUE_IDS).collect(), message: cut(i.message) })
        .collect();
    let warnings = warnings.into_iter().take(MAX_WARNINGS).map(cut).collect();
    RenderDrawn { rev, area, scale, png_b64, boxes, issues, issue_count, warnings, timings_ms, times_ms: Vec::new() }
}

/// `text` within `NOTE_CHARS` characters, an ellipsis last when it was longer.
fn cut(text: String) -> String {
    if text.chars().count() <= NOTE_CHARS {
        return text;
    }
    text.chars().take(NOTE_CHARS - 1).chain(['…']).collect()
}

/// The answer to render `render_id`: exactly one of what was drawn and why not.
fn answer_of(render_id: &str, drawn: Result<RenderDrawn, RenderFailure>) -> AnswerIn {
    let (drawn, error) = match drawn {
        Ok(drawn) => (Some(drawn), None),
        Err(error) => (None, Some(error)),
    };
    AnswerIn { render_id: render_id.to_string(), drawn, error }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::design::Node;
    use relay_bus::ops::design::RENDER_BOX_FIELDS;

    fn request() -> RenderRequest {
        RenderRequest {
            render_id: "6f0c1f9e-2d5c-4a57-9a43-0e6f3b8c1d22".into(),
            design_id: 12,
            page: "page-1".into(),
            nodes: None,
            png: true,
            scale: 1.0,
            max_side: 1568,
            boxes_limit: 200,
            deadline_unix_ms: 1_792_368_000_000,
            at_ms: None,
            frames: None,
            by: None,
        }
    }

    fn node(id: &str, parent: Option<&str>, kind: NodeKind) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        n
    }

    /// The line the engine writes to the leased connection is read as an event (the client's
    /// reader takes any line whose second key is `ev`) and its payload as the request.
    #[test]
    fn a_request_line_parses() {
        let line = r#"{"v":1,"ev":"design.render.request","ts":"2026-10-20T12:00:00.000Z","actor":"system","payload":{
            "render_id":"6f0c1f9e-2d5c-4a57-9a43-0e6f3b8c1d22","design_id":12,"page":"page-1","nodes":null,"png":true,
            "scale":1.0,"max_side":1568,"boxes_limit":200,"deadline_unix_ms":1792368000000,"by":"brisk-otter"}}"#;
        let ev: Event = serde_json::from_str(line).unwrap();
        assert_eq!(request_of(&ev), Some(RenderRequest { by: Some("brisk-otter".into()), ..request() }));
        // Motion fields ride along when asked for.
        let posed = line.replace(r#""by":"brisk-otter""#, r#""at_ms":500.0"#);
        let ev: Event = serde_json::from_str(&posed).unwrap();
        assert_eq!(request_of(&ev).and_then(|r| r.at_ms), Some(500.0));
        // Any other line on the connection is not a request.
        let other: Event = serde_json::from_str(&line.replace("design.render.request", "design.changed")).unwrap();
        assert_eq!(request_of(&other), None);
        let broken: Event = serde_json::from_str(&line.replace(r#""design_id":12"#, r#""design_id":"twelve""#)).unwrap();
        assert_eq!(request_of(&broken), None);
    }

    /// A job whose deadline has passed is skipped: the engine already answered the agent.
    #[test]
    fn an_expired_job_is_dropped() {
        let req = request();
        assert!(!expired(&req, req.deadline_unix_ms - 1));
        assert!(expired(&req, req.deadline_unix_ms));
        assert!(expired(&req, req.deadline_unix_ms + 60_000));
    }

    /// The answer is the op's own payload: one of drawn and error, the rows' columns the
    /// engine checks, and issues and warnings cut to what it lists.
    #[test]
    fn an_answer_is_shaped_as_answer_in() {
        assert_eq!(boxes::FIELDS, RENDER_BOX_FIELDS, "the rows' columns are the ones the engine holds an answer to");
        let boxes = RenderBoxes { fields: boxes::FIELDS.map(String::from).to_vec(), rows: vec![], total: 0, hidden: 0 };
        let issue = |i: usize| RenderIssue { code: "overlap".into(), ids: (0..12).map(|k| format!("n{i}-{k}")).collect(), message: "x".repeat(400) };
        let issues: Vec<RenderIssue> = (0..130).map(issue).collect();
        let warnings: Vec<String> = (0..25).map(|i| format!("warning {i} {}", "y".repeat(320))).collect();
        let timings = RenderTimings { read: 1.0, layout: 2.0, paint: 3.0, encode: 4.0 };
        let area = RenderRect { x: -32.0, y: -32.0, w: 864.0, h: 664.0 };
        let drawn = shaped(7, area, 1.0, Some("iVBORw0KGgo=".into()), boxes, issues, 130, warnings, timings);
        assert_eq!(drawn.issues.len(), boxes::MAX_ISSUES);
        assert_eq!(drawn.issue_count, 130, "the count covers every issue found");
        assert!(drawn.issues.iter().all(|i| i.ids.len() == ISSUE_IDS && i.message.chars().count() == NOTE_CHARS));
        assert_eq!(drawn.warnings.len(), MAX_WARNINGS);
        assert!(drawn.warnings.iter().all(|w| w.chars().count() <= NOTE_CHARS && w.ends_with('…')));
        let answer = answer_of("6f0c", Ok(drawn));
        let value = serde_json::to_value(&answer).unwrap();
        assert_eq!(value["render_id"], "6f0c");
        assert!(value["error"].is_null());
        assert_eq!(value["drawn"]["boxes"]["fields"], json!(RENDER_BOX_FIELDS));
        assert_eq!(value["drawn"]["rev"], 7);
        assert!(value["drawn"].get("times_ms").is_none(), "one picture carries no frame times");
        // What the engine reads back is what was sent.
        assert_eq!(serde_json::from_value::<AnswerIn>(value).unwrap(), answer);
        let refused = serde_json::to_value(answer_of("6f0c", Err(failed("no".into())))).unwrap();
        assert!(refused["drawn"].is_null());
        assert_eq!(refused["error"], json!({"code": "design.render.failed", "message": "no"}));
    }

    /// The app reports a failure under the codes the engine keeps (§6.6): a design or page gone
    /// after the engine's check under its own, a node gone as node_not_found, motion and
    /// anything else as design.render.failed.
    #[test]
    fn app_failures_use_the_listed_codes() {
        for code in ["design.not_found", "design.page_not_found"] {
            let e = Error::Bus(Box::new(relay_bus::BusError::not_found(code, "gone")));
            assert_eq!(failure_of(&e).code, code);
        }
        let refused = Error::Bus(Box::new(relay_bus::BusError::invalid("design.get", "bad")));
        assert_eq!(failure_of(&refused).code, "design.render.failed");
        assert_eq!(failure_of(&Error::Disconnected).code, "design.render.failed");
        assert_eq!(failure_of(&Error::Timeout).code, "design.render.failed");
        let scene = Scene::new(vec![node("p", None, NodeKind::Page), node("a", Some("p"), NodeKind::Rect)]);
        assert_eq!(missing_node(&scene, &["a".into()]), None);
        assert_eq!(missing_node(&scene, &["a".into(), "b".into()]).map(|f| f.code), Some("design.node_not_found".into()));
        assert_eq!(motion_refused(&request()), None);
        for posed in [RenderRequest { at_ms: Some(250.0), ..request() }, RenderRequest { frames: Some(4), ..request() }] {
            assert_eq!(motion_refused(&posed), Some(failed(NO_MOTION.into())));
        }
        let listed = ["design.render.no_surface", "design.render.failed", "design.not_found", "design.page_not_found", "design.node_not_found"];
        assert!(listed.contains(&failed(String::new()).code.as_str()));
    }

    /// A PNG over the cap is drawn once more at a scale whose pixels, and so its bytes, fit
    /// with room; a smaller one never grows.
    #[test]
    fn the_retry_scale_fits_the_cap() {
        for (scale, size) in [(1.0, PNG_MAX + 1), (2.0, 3 * PNG_MAX), (0.5, 16 * PNG_MAX)] {
            let next = retry_scale(scale, size);
            assert!(next < scale && next > 0.0);
            // Bytes grow with pixels, pixels with the square of the scale.
            let expected = size as f64 * (next / scale).powi(2);
            assert!(expected <= PNG_MAX as f64 * 0.81 + 1.0, "{scale} at {size} bytes: {next} gives {expected}");
        }
        assert!(retry_scale(1.0, PNG_MAX / 2) <= 1.0);
        assert!(retry_scale(1.0, 0) <= 1.0);
    }

    /// Decision 3: an agent's render says so on the boards showing that design; a blank name
    /// says nothing.
    #[test]
    fn a_board_hears_who_looked() {
        assert_eq!(looked_note("brisk-otter").as_deref(), Some("brisk-otter looked at this design"));
        assert_eq!(looked_note("  "), None);
    }

    /// W68: the lease reports each family once, sorted, and leaves out a name the op would
    /// refuse rather than losing the lease over it.
    #[test]
    fn the_lease_reports_fonts_the_engine_takes() {
        let names = vec!["Sora".to_string(), "Geist".into(), "Geist".into(), String::new(), " ".into(), "x".repeat(201), "Bad\u{7}".into(), "x".repeat(200)];
        assert_eq!(serve_fonts(names), ["Geist", "Sora", &"x".repeat(200)]);
        let many: Vec<String> = (0..SERVE_FONTS_MAX + 5).map(|i| format!("Family {i:05}")).collect();
        assert_eq!(serve_fonts(many).len(), SERVE_FONTS_MAX);
    }

    /// A dropped render connection is offered again soon, then less often, never past the most.
    #[test]
    fn the_lease_retries_with_a_growing_wait() {
        let mut wait = RETRY_FIRST;
        let mut seen = vec![wait];
        for _ in 0..8 {
            wait = next_wait(wait);
            seen.push(wait);
        }
        assert_eq!(seen[1], Duration::from_secs(2));
        assert!(seen.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(*seen.last().unwrap(), RETRY_MOST);
    }

    #[test]
    fn warnings_name_what_is_missing() {
        assert_eq!(placeholders_note(0), None);
        assert!(placeholders_note(1).unwrap().starts_with("1 picture is drawn"));
        assert!(placeholders_note(3).unwrap().starts_with("3 pictures are drawn"));
        assert_eq!(nothing_visible(&[]), "the page has no visible layers");
        assert!(nothing_visible(&["a".into()]).starts_with("nothing in nodes"));
    }
}
