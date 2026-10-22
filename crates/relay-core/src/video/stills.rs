//! `video.check` and `video.still`: reading a video's code through the worker, with no store lock
//! held. Each records a version of the code first when it changed (a system write, as a
//! background job's), so what an agent looked at can be told apart from what came after.

use super::probe::{self, Drawn, Probe};
use super::sheet::{self, Rgba};
use super::{blobs_dir, bundle_dir, get_video, poster_path, versions, worker, Video};
use crate::engine::{Engine, IntoBus};
use relay_bus::error::BusError;
use relay_bus::ops::video::{frame_label, CheckOut, CodeError, Composition, ErrorKind, ScanIn, ScanOut, StillFrame, StillIn, StillOut, MAX_STILLS};
use relay_bus::types::Id;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHECK_WITHIN: Duration = Duration::from_secs(180);
const META_WITHIN: Duration = Duration::from_secs(120);
/// Most still pictures kept: the oldest go as new ones come.
const KEEP_STILLS: usize = 64;
/// Most boxes one still answers with.
const MOST_BOXES: usize = 400;

/// Who a system write is on behalf of: the request's actor and its session.
pub struct Caller<'a> {
    pub actor: &'a str,
    pub session_id: Option<Id>,
}

/// Record the folder's code as video `video`'s next version when it changed; the version it is
/// at. Emits `video.changed` for a new one.
pub fn snapshot(engine: &Engine, video: &Video, by: &Caller) -> Result<i64, BusError> {
    let scan = versions::scan(Path::new(&video.dir), &blobs_dir(engine))
        .map_err(|e| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}")))?;
    let id = video.id;
    engine.system_write("video.version", None, Some(video.project_id), by.session_id, json!({"video_id": id}), |tx, now| {
        let (n, new) = versions::record(tx, id, &scan, by.actor, now)?;
        let mut events = Vec::new();
        if new {
            tx.prepare_cached("UPDATE videos SET updated_at = ?2, updated_by = ?3 WHERE id = ?1").bus()?
                .execute(rusqlite::params![id, now, by.actor]).bus()?;
            events.push(("video.changed".to_string(), serde_json::to_value(get_video(tx, id)?).bus()?));
        }
        Ok((n, events))
    })
}

#[derive(Deserialize)]
struct WorkerCheck {
    #[serde(default)]
    errors: Vec<CodeError>,
    #[serde(default)]
    compositions: Vec<Composition>,
}

/// Build, typecheck and list the compositions; keeps them on the row.
pub fn check(engine: &Engine, video: &Video, by: &Caller) -> Result<CheckOut, BusError> {
    super::kit::ready(engine)?;
    let version = snapshot(engine, video, by)?;
    let args = json!({"dir": video.dir, "bundle_dir": bundle_dir(engine, video.id)});
    let found: WorkerCheck = serde_json::from_value(worker::call(engine, "check", args, CHECK_WITHIN)?).bus()?;
    let ok = !found.errors.iter().any(|e| matches!(e.kind, ErrorKind::Syntax | ErrorKind::Bundle | ErrorKind::Runtime));
    if !found.compositions.is_empty() {
        keep_compositions(engine, video, &found.compositions, by)?;
    }
    // Their props and schemas are kept for video.get and the props panel; a check answers what
    // the agent acts on.
    let compositions = found.compositions.into_iter().map(summary).collect();
    Ok(CheckOut { ok, errors: found.errors, compositions, version })
}

/// A composition without its props and schema: what a check, a still or a scan answers.
pub fn summary(mut c: Composition) -> Composition {
    c.default_props = Value::Null;
    c.props_schema = None;
    c
}

/// Keep what a check found on the row: the compositions, and the default one's size, rate and
/// length (the first, when the default is gone).
fn keep_compositions(engine: &Engine, video: &Video, compositions: &[Composition], by: &Caller) -> Result<(), BusError> {
    let main = compositions.iter().find(|c| c.id == video.composition).unwrap_or(&compositions[0]);
    let text = serde_json::to_string(compositions).bus()?;
    let id = video.id;
    engine.system_write("video.check", None, Some(video.project_id), by.session_id, json!({"video_id": id}), |tx, now| {
        let before: String = tx.prepare_cached("SELECT compositions FROM videos WHERE id = ?1").bus()?.query_row([id], |r| r.get(0)).bus()?;
        let same = before == text && main.id == video.composition && main.width == video.width && main.height == video.height
            && main.fps == video.fps && main.duration_frames == video.duration_frames;
        if same {
            return Ok(((), Vec::new()));
        }
        tx.prepare_cached("UPDATE videos SET compositions = ?2, composition = ?3, width = ?4, height = ?5, fps = ?6, duration_frames = ?7,
            updated_at = ?8, updated_by = ?9 WHERE id = ?1").bus()?
            .execute(rusqlite::params![id, text, main.id, main.width, main.height, main.fps, main.duration_frames, now, by.actor]).bus()?;
        Ok(((), vec![("video.changed".to_string(), serde_json::to_value(get_video(tx, id)?).bus()?)]))
    })
}

/// The errors as one refusal, when nothing could be drawn: the first in full, a count of the rest.
pub fn build_refusal(errors: &[CodeError]) -> BusError {
    let first = errors.first();
    let at = first.map(|e| match (&e.file, e.line) {
        (Some(f), Some(l)) => format!("{f}:{l}: "),
        (Some(f), None) => format!("{f}: "),
        _ => String::new(),
    }).unwrap_or_default();
    let more = if errors.len() > 1 { format!(" ({} more)", errors.len() - 1) } else { String::new() };
    let kind = first.map_or(ErrorKind::Bundle, |e| e.kind);
    let code = match kind {
        ErrorKind::Runtime => "video.runtime",
        _ => "video.build",
    };
    BusError::invalid(code, format!("{at}{}{more}", first.map_or("the video did not build", |e| e.message.as_str())))
        .with_details(json!({"errors": errors}))
        .with_hint("fix the code at that line, then video.check")
}

#[derive(Deserialize)]
struct WorkerMeta {
    composition: Option<Composition>,
    #[serde(default)]
    errors: Vec<CodeError>,
}

#[derive(Deserialize)]
struct WorkerStill {
    frame: u32,
    path: Option<String>,
    probe: Option<Probe>,
    #[serde(default)]
    console: Vec<String>,
}

#[derive(Deserialize)]
struct WorkerStills {
    #[serde(default)]
    stills: Vec<WorkerStill>,
    #[serde(default)]
    errors: Vec<CodeError>,
}

/// The frames a still draws: each of `at`, or `count` evenly from `from` to `to`; default six
/// across the whole composition.
pub fn frames_of(p: &StillIn, fps: f64, duration: u32) -> Result<Vec<u32>, BusError> {
    let bad = |e: String| BusError::invalid("video.moment", e);
    let mut frames: Vec<u32> = match &p.at {
        Some(at) if !at.is_empty() => {
            if at.len() > MAX_STILLS {
                return Err(BusError::invalid("video.moment", format!("at most {MAX_STILLS} moments in one still")));
            }
            at.iter().map(|m| m.frame(fps, duration)).collect::<Result<_, _>>().map_err(bad)?
        }
        _ => {
            let n = p.count.unwrap_or(6).clamp(2, MAX_STILLS as u32);
            let from = p.from.as_ref().map(|m| m.frame(fps, duration)).transpose().map_err(bad)?.unwrap_or(0);
            let to = p.to.as_ref().map(|m| m.frame(fps, duration)).transpose().map_err(bad)?.unwrap_or(duration.saturating_sub(1));
            if to < from {
                return Err(BusError::invalid("video.moment", "`to` is before `from`"));
            }
            (0..n).map(|i| from + ((to - from) as f64 * i as f64 / (n - 1) as f64).round() as u32).collect()
        }
    };
    frames.dedup();
    Ok(frames)
}

/// Props to draw with: the person's for the composition, with `extra` over them.
pub fn props_for(engine: &Engine, video_id: Id, composition: &str, extra: Option<&Value>) -> Result<Value, BusError> {
    let stored: String = engine.read_store("video.props", |conn| {
        conn.prepare_cached("SELECT props FROM videos WHERE id = ?1").bus()?.query_row([video_id], |r| r.get(0)).bus()
    })?;
    let mut props = serde_json::from_str::<Value>(&stored).ok().and_then(|v| v.get(composition).cloned()).unwrap_or(json!({}));
    if let (Some(Value::Object(extra)), Value::Object(base)) = (extra, &mut props) {
        for (k, v) in extra {
            base.insert(k.clone(), v.clone());
        }
    } else if let Some(extra) = extra {
        if !extra.is_object() {
            return Err(BusError::invalid("video.props", "props are an object of the composition's props"));
        }
    }
    Ok(props)
}

/// Draw frames of a composition, check them, and answer the picture.
pub fn still(engine: &Engine, video: &Video, p: &StillIn, by: &Caller) -> Result<StillOut, BusError> {
    super::kit::ready(engine)?;
    let version = snapshot(engine, video, by)?;
    let composition = p.composition.clone().unwrap_or_else(|| video.composition.clone());
    let props = props_for(engine, video.id, &composition, p.props.as_ref())?;
    let bundle = bundle_dir(engine, video.id);
    let meta: WorkerMeta = serde_json::from_value(worker::call(engine, "meta",
        json!({"dir": video.dir, "bundle_dir": bundle, "composition": composition, "props": props}), META_WITHIN)?).bus()?;
    let Some(comp) = meta.composition.filter(|_| meta.errors.is_empty()) else {
        return Err(build_refusal(&meta.errors));
    };
    let frames = frames_of(p, comp.fps, comp.duration_frames)?;
    let sheet_of = frames.len() > 1;
    let scale = p.scale.unwrap_or_else(|| {
        let long = comp.width.max(comp.height).max(1) as f64;
        (if sheet_of { 480.0 } else { 1280.0 } / long).min(1.0)
    }).clamp(0.05, 1.0);

    let dir = engine.store.video_stills_dir();
    fs::create_dir_all(&dir).map_err(|e| BusError::internal(format!("{}: {e}", dir.display())))?;
    let name = uuid::Uuid::new_v4();
    let tiles: Vec<PathBuf> = (0..frames.len()).map(|i| dir.join(format!("t-{name}-{i}.png"))).collect();
    let within = Duration::from_secs(60 + 10 * frames.len() as u64);
    let drawn: WorkerStills = serde_json::from_value(worker::call(engine, "still", json!({
        "dir": video.dir, "bundle_dir": bundle, "composition": composition, "frames": frames,
        "outputs": tiles, "scale": scale, "props": props,
    }), within)?).bus()?;
    if drawn.stills.iter().all(|s| s.path.is_none()) {
        return Err(build_refusal(&drawn.errors));
    }

    let pictures: Vec<Option<Rgba>> = drawn.stills.iter().map(|s| s.path.as_ref().and_then(|p| sheet::decode(Path::new(p)).ok())).collect();
    let checked: Vec<Drawn> = drawn.stills.iter().zip(&pictures).filter_map(|(s, picture)| {
        s.probe.as_ref().map(|probe| Drawn { probe, picture: picture.as_ref(), scale, console: &s.console, moving: &[] })
    }).collect();
    let issues = probe::issues(&checked, comp.fps);
    let boxes = if p.boxes.unwrap_or(false) {
        probe::boxes(&drawn.stills.iter().filter_map(|s| s.probe.as_ref()).collect::<Vec<_>>(), MOST_BOXES)
    } else {
        Vec::new()
    };
    let mut errors = drawn.errors;
    if drawn.stills.iter().any(|s| s.path.is_some() && s.probe.is_none()) {
        errors.push(CodeError { kind: ErrorKind::Probe, file: Some("src/index.ts".into()), line: None, col: None, frame: None,
            message: "the frames were drawn but not measured: src/index.ts must register relayRoot(Root) for the checks".into() });
    }

    let labels: Vec<String> = drawn.stills.iter().map(|s| frame_label(s.frame, comp.fps)).collect();
    let png = dir.join(format!("{name}.png"));
    if sheet_of {
        let blank = Rgba::new((comp.width as f64 * scale).round().max(1.0) as u32, (comp.height as f64 * scale).round().max(1.0) as u32, [60, 20, 24, 255]);
        let tiles_drawn: Vec<Rgba> = pictures.iter().map(|p| p.clone().unwrap_or_else(|| blank.clone())).collect();
        sheet::encode(&png, &sheet::sheet(&tiles_drawn, &labels)).map_err(BusError::internal)?;
    } else if let Some(path) = drawn.stills[0].path.as_ref() {
        fs::rename(path, &png).map_err(|e| BusError::internal(format!("{}: {e}", png.display())))?;
    }

    // The poster: the frame drawn nearest two fifths in, of the video's own composition.
    if composition == video.composition {
        let target = comp.duration_frames as f64 * 0.4;
        let best = drawn.stills.iter().enumerate().filter(|(_, s)| s.path.is_some())
            .min_by(|(_, a), (_, b)| (a.frame as f64 - target).abs().total_cmp(&(b.frame as f64 - target).abs()))
            .map(|(i, _)| i);
        if let Some(i) = best {
            let from = if sheet_of { tiles[i].clone() } else { png.clone() };
            keep_poster(engine, video, &from, by);
        }
    }
    for tile in &tiles {
        let _ = fs::remove_file(tile);
    }
    prune(&dir);

    let frames = drawn.stills.iter().map(|s| StillFrame { frame: s.frame, at_ms: s.frame as f64 * 1000.0 / comp.fps, label: frame_label(s.frame, comp.fps) }).collect();
    Ok(StillOut { png: png.display().to_string(), composition: summary(comp), frames, boxes, issues, errors, version })
}

#[derive(Deserialize)]
struct WorkerScan {
    #[serde(default)]
    probes: Vec<Probe>,
    #[serde(default)]
    console: Vec<String>,
    #[serde(default)]
    errors: Vec<CodeError>,
}

/// Measure every `every` frames of a composition, with no pictures, and check them all.
pub fn scan(engine: &Engine, video: &Video, p: &ScanIn, by: &Caller) -> Result<ScanOut, BusError> {
    super::kit::ready(engine)?;
    let version = snapshot(engine, video, by)?;
    let composition = p.composition.clone().unwrap_or_else(|| video.composition.clone());
    let props = props_for(engine, video.id, &composition, p.props.as_ref())?;
    let bundle = bundle_dir(engine, video.id);
    let meta: WorkerMeta = serde_json::from_value(worker::call(engine, "meta",
        json!({"dir": video.dir, "bundle_dir": bundle, "composition": composition, "props": props}), META_WITHIN)?).bus()?;
    let Some(comp) = meta.composition.filter(|_| meta.errors.is_empty()) else {
        return Err(build_refusal(&meta.errors));
    };
    let every = p.every.unwrap_or(3).clamp(1, 30);
    let bad = |e: String| BusError::invalid("video.moment", e);
    let last = comp.duration_frames.saturating_sub(1);
    let from = p.from.as_ref().map(|m| m.frame(comp.fps, comp.duration_frames)).transpose().map_err(bad)?.unwrap_or(0);
    let to = p.to.as_ref().map(|m| m.frame(comp.fps, comp.duration_frames)).transpose().map_err(bad)?.unwrap_or(last);
    if to < from {
        return Err(BusError::invalid("video.moment", "`to` is before `from`"));
    }
    // About a fifth of a second a frame measured, on a loaded machine.
    let within = Duration::from_secs(60 + u64::from((to - from) / every) / 5);
    let found: WorkerScan = serde_json::from_value(worker::call(engine, "scan", json!({
        "dir": video.dir, "bundle_dir": bundle, "composition": composition, "props": props, "every": every, "range": [from, to],
    }), within)?).bus()?;
    let mut probes = found.probes;
    probes.sort_by_key(|p| p.frame);
    probes.dedup_by_key(|p| p.frame);
    if probes.is_empty() && !found.errors.is_empty() {
        return Err(build_refusal(&found.errors));
    }
    let issues = probe::scan(&probes, &found.console, comp.fps, every);
    Ok(ScanOut { composition: summary(comp), frames: probes.len() as u32, every, issues, errors: found.errors, version })
}

/// Copy `from` to the video's poster, and name it on the row the first time.
fn keep_poster(engine: &Engine, video: &Video, from: &Path, by: &Caller) {
    let to = poster_path(engine, video.id);
    if let Some(parent) = to.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let part = to.with_extension("part");
    if fs::copy(from, &part).and_then(|_| fs::rename(&part, &to)).is_err() {
        return;
    }
    let path = to.display().to_string();
    if video.poster.as_deref() == Some(path.as_str()) {
        return;
    }
    let id = video.id;
    let _ = engine.system_write("video.poster", None, Some(video.project_id), by.session_id, json!({"video_id": id}), |tx, _| {
        tx.prepare_cached("UPDATE videos SET poster = ?2 WHERE id = ?1").bus()?.execute(rusqlite::params![id, path]).bus()?;
        Ok(((), vec![("video.changed".to_string(), serde_json::to_value(get_video(tx, id)?).bus()?)]))
    });
}

/// Keep the newest [`KEEP_STILLS`] pictures.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries.flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(_, p)| p.extension().is_some_and(|x| x == "png"))
        .collect();
    if files.len() <= KEEP_STILLS {
        return;
    }
    files.sort();
    for (_, path) in &files[..files.len() - KEEP_STILLS] {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::ops::video::Moment;

    fn input(at: Option<Vec<Moment>>, count: Option<u32>) -> StillIn {
        StillIn { video_id: 1, composition: None, at, count, from: None, to: None, scale: None, props: None, boxes: None }
    }

    #[test]
    fn a_still_draws_its_moments_or_an_even_spread() {
        assert_eq!(frames_of(&input(Some(vec![Moment::Seconds(1.0), Moment::Text("end".into())]), None), 30.0, 300).unwrap(), [30, 299]);
        assert_eq!(frames_of(&input(None, None), 30.0, 300).unwrap(), [0, 60, 120, 179, 239, 299]);
        assert_eq!(frames_of(&input(None, Some(3)), 30.0, 301).unwrap(), [0, 150, 300]);
        let mut ranged = input(None, Some(2));
        ranged.from = Some(Moment::Seconds(1.0));
        ranged.to = Some(Moment::Seconds(2.0));
        assert_eq!(frames_of(&ranged, 30.0, 300).unwrap(), [30, 60]);
        ranged.to = Some(Moment::Seconds(0.5));
        assert_eq!(frames_of(&ranged, 30.0, 300).unwrap_err().code, "video.moment");
    }

    #[test]
    fn a_build_that_failed_reads_as_its_first_error() {
        let errors = vec![
            CodeError { kind: ErrorKind::Syntax, file: Some("src/Video.tsx".into()), line: Some(12), col: Some(4), message: "Expected \"}\"".into(), frame: None },
            CodeError { kind: ErrorKind::Type, file: Some("src/Video.tsx".into()), line: Some(3), col: None, message: "TS2322".into(), frame: None },
        ];
        let e = build_refusal(&errors);
        assert_eq!(e.code, "video.build");
        assert_eq!(e.message, "src/Video.tsx:12: Expected \"}\" (1 more)");
    }
}
