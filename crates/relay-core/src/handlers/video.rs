//! `video.*` (BUS.md §10.19, docs/VIDEO.md): videos written as code. The rows and folders are
//! here; the worker, the kit, versions, stills and renders are `crate::video`.
//!
//! What reads the folder through the worker (check, still) runs unlocked, and what changes rows
//! after slow file work (render, asset.add, version.restore) is staged (D149): no store lock is
//! ever held while Node bundles, Chrome draws or a file is copied.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::handlers::notes::{actor_name, assert_actor_project, check_project};
use crate::video::stills::{self, Caller};
use crate::video::{self, assets, get_render, get_video, jobs, kit, live_video, quality_name, render_row, select_videos, versions, video_row, RENDER_COLUMNS};
use relay_bus::design::{plain_label, MAX_NAME};
use relay_bus::error::BusError;
use relay_bus::ops::video::{
    AssetAdd, AssetAddOut, AssetInspect, Preview, PreviewOut, CaptureAndroid, CaptureOut, ElevenKeyRemove, ElevenKeySet, ElevenStatusOp, MadeOut, Music, Sfx, Transcribe, TranscribeOut, VoiceInfo, Voices,
    VoicesOut, VoiceOut, AssetList, AssetListOut, AudioBeats, BeatBar, BeatGrid, BeatSection, Check, Composition, Create, CreateOut, Delete, Format, Get, GetOut, KitInstall,
    KitState, KitStatusOp, List, ListOut, Quality, Render, RenderBatch, RenderCancel, RenderGet, RenderJob, RenderState, RenderWait,
    RendersOut, Restore, Scan, Still, Timeline, Update, Version, VersionDiff, VersionDiffOut, VersionLabel, VersionRestore, VersionRestoreOut, VideoRender,
    MAX_ASSETS, MAX_SECONDS, MAX_SIDE, MAX_VERSION_LABEL,
};
use relay_bus::types::Id;
use relay_bus::Empty;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_TITLE: &str = "Untitled video";

fn title(raw: Option<&str>) -> Result<String, BusError> {
    let title = raw.map_or(DEFAULT_TITLE, str::trim);
    let n = title.chars().count();
    if n == 0 || n > MAX_NAME {
        return Err(BusError::invalid("video.title", format!("a video title is 1 to {MAX_NAME} characters")));
    }
    if !plain_label(title) {
        return Err(BusError::invalid("video.title", "a video title has no control characters"));
    }
    Ok(title.to_string())
}

/// The terminal a video is tied to, as a design's (`design.create`): an agent's own; the
/// person's any session the project has had.
fn check_session(ctx: &Ctx, project_id: Id, session: &str) -> Result<(), BusError> {
    if let Some(own) = ctx.actor.session_name() {
        return if own == session { Ok(()) } else { Err(BusError::not_own("session")) };
    }
    let known: bool = ctx.tx()
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM sessions WHERE project_id = ?1 AND name = ?2)").bus()?
        .query_row(params![project_id, session], |r| r.get(0)).bus()?;
    if !known {
        return Err(BusError::invalid("video.session", format!("project {project_id} has no session {session:?}"))
            .with_hint("name one of the project's sessions (session.list), or none"));
    }
    Ok(())
}

fn emit(ctx: &mut Ctx, id: Id) -> Result<relay_bus::ops::video::Video, BusError> {
    let video = get_video(ctx.tx(), id)?;
    ctx.set_project(video.project_id);
    ctx.emit("video.changed", serde_json::to_value(&video).bus()?);
    Ok(video)
}

/// Video `id` for a request that reads it with no lock held: one short read, checked against
/// the asker's project.
fn read_video(ctx: &mut Unlocked, id: Id, live: bool) -> Result<relay_bus::ops::video::Video, BusError> {
    let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
    ctx.read(|conn| {
        let video = if live { live_video(conn, id)? } else { get_video(conn, id)? };
        check_project(conn, video.project_id, session_id, agent)?;
        Ok(video)
    })
}

fn caller_name(ctx: &Unlocked) -> String {
    ctx.actor.session_name().map_or_else(|| "user".to_string(), str::to_string)
}

fn compositions(conn: &Connection, id: Id) -> Result<Vec<Composition>, BusError> {
    let text: String = conn.prepare_cached("SELECT compositions FROM videos WHERE id = ?1").bus()?.query_row([id], |r| r.get(0)).bus()?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

/// `props` checked against what the composition's schema says of each: its type, an `enum`'s
/// values, and no name it does not have when it allows none.
fn check_props(schema: Option<&Value>, composition: &str, props: &Map<String, Value>) -> Result<(), BusError> {
    let Some(schema) = schema else { return Ok(()) };
    let fields = schema.get("properties").and_then(Value::as_object);
    let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
    for (name, value) in props {
        let Some(field) = fields.and_then(|f| f.get(name)) else {
            if closed {
                let known: Vec<&String> = fields.map(|f| f.keys().collect()).unwrap_or_default();
                return Err(BusError::invalid("video.props", format!("{composition} has no prop {name:?}"))
                    .with_details(json!({"props": known})));
            }
            continue;
        };
        let fits = match field.get("type").and_then(Value::as_str) {
            Some("string") => value.is_string(),
            Some("number") => value.is_number(),
            Some("integer") => value.is_i64() || value.is_u64(),
            Some("boolean") => value.is_boolean(),
            Some("array") => value.is_array(),
            Some("object") => value.is_object(),
            _ => true,
        };
        let listed = field.get("enum").and_then(Value::as_array).is_none_or(|values| values.contains(value));
        if !fits || !listed {
            return Err(BusError::invalid("video.props", format!("{composition}.{name}: {value} does not fit its schema {field}")));
        }
    }
    Ok(())
}

/// Write the starter into `dir`: its files with the video's size, rate, length and title, and an
/// empty `public/`. A file already there is left as it is.
fn write_starter(dir: &Path, title: &str, w: u32, h: u32, fps: f64, frames: u32) -> Result<Vec<String>, String> {
    let fps_text = if fps.fract() == 0.0 { format!("{}", fps as u32) } else { format!("{fps}") };
    let mut written = Vec::new();
    for (rel, bytes) in kit::starter() {
        let path = dir.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let text = String::from_utf8_lossy(bytes)
            .replace("__TITLE__", &serde_json::to_string(title).unwrap_or_default())
            .replace("__WIDTH__", &w.to_string())
            .replace("__HEIGHT__", &h.to_string())
            .replace("__FPS__", &fps_text)
            .replace("__FRAMES__", &frames.to_string());
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(rel.to_string());
    }
    fs::create_dir_all(dir.join("public")).map_err(|e| format!("public/: {e}"))?;
    written.sort();
    Ok(written)
}

pub struct StagedAsset {
    out: AssetAddOut,
}

pub struct Staged<T>(T);

/// One render made ready with nothing locked: what its row is inserted with.
pub struct Job {
    composition: String,
    props: Value,
    range: Option<[u32; 2]>,
    format: Format,
    quality: Quality,
    name: Option<String>,
    loudness: Option<f64>,
    peak: Option<f64>,
}

pub struct StagedRender {
    video: relay_bus::ops::video::Video,
    scan: versions::Scan,
    jobs: Vec<Job>,
}

pub struct StagedRestore {
    before: versions::Scan,
    after: versions::Scan,
    only: Option<Vec<String>>,
    missing: Vec<String>,
    left: Vec<String>,
}

/// A staged op's commit: video `id` still there and the caller's to change, and the audit row
/// under its project.
fn commit_to(ctx: &mut Ctx, id: Id) -> Result<(), BusError> {
    let video = live_video(ctx.tx(), id)?;
    assert_actor_project(ctx, video.project_id)?;
    ctx.set_project(video.project_id);
    Ok(())
}

/// `paths` an agent or the person gave, made a version's own, each naming a file (or a folder of
/// files) that one of `sides` has: a name that matches nothing is a mistake, not a no-op.
fn only_paths(raw: &[String], sides: &[&[versions::Entry]], what: &str) -> Result<Vec<String>, BusError> {
    if raw.is_empty() || raw.len() > 100 {
        return Err(BusError::invalid("video.version", "name 1 to 100 files or folders"));
    }
    let only = versions::clean_paths(raw)
        .ok_or_else(|| BusError::invalid("video.version", "paths are relative to the video's folder (src/Open.tsx), and stay in it"))?;
    for path in &only {
        let one = std::slice::from_ref(path);
        if !sides.iter().any(|side| side.iter().any(|e| versions::within(&e.path, Some(one)))) {
            return Err(BusError::invalid("video.version", format!("no file or folder {path} in {what}"))
                .with_hint("video.version.diff lists the files that changed; a version holds src/, public/ but its assets, and the folder's own files"));
        }
    }
    Ok(only)
}

/// A job's fields checked against the video, with nothing locked: its composition, props, frames,
/// file name and loudness.
fn prepare(ctx: &mut Unlocked, video: &relay_bus::ops::video::Video, job: RenderJob) -> Result<Job, BusError> {
    let composition = job.composition.clone().unwrap_or_else(|| video.composition.clone());
    let props = stills::props_for(ctx.engine(), video.id, &composition, job.props.as_ref())?;
    // The range is read against the composition as the last check found it.
    let known = ctx.read(|conn| compositions(conn, video.id))?;
    let (fps, frames) = known.iter().find(|c| c.id == composition)
        .map_or((video.fps, video.duration_frames), |c| (c.fps, c.duration_frames));
    let at = |m: &Option<relay_bus::ops::video::Moment>, default: u32| -> Result<u32, BusError> {
        m.as_ref().map_or(Ok(default), |m| m.frame(fps, frames).map_err(|e| BusError::invalid("video.moment", e)))
    };
    let range = if job.from.is_some() || job.to.is_some() {
        let (a, b) = (at(&job.from, 0)?, at(&job.to, frames.saturating_sub(1))?);
        if b < a {
            return Err(BusError::invalid("video.moment", "`to` is before `from`"));
        }
        Some([a, b])
    } else {
        None
    };
    let format = job.format.unwrap_or(Format::Mp4);
    let name = match job.name.as_deref() {
        Some(raw) => Some(jobs::safe_name(raw).ok_or_else(|| BusError::invalid("video.name", "a render's name keeps letters, digits, dots, dashes or underscores"))?),
        None => None,
    };
    let (loudness, peak) = match (job.preset, job.loudness, job.peak) {
        (Some(preset), _, peak) => {
            let (l, p) = preset.target();
            (Some(l), Some(peak.unwrap_or(p)))
        }
        (None, Some(l), peak) => (Some(l), Some(peak.unwrap_or(-1.0))),
        (None, None, peak) => (None, peak),
    };
    if loudness.is_some_and(|l| !(-40.0..=-5.0).contains(&l)) || peak.is_some_and(|p| !(-9.0..=0.0).contains(&p)) {
        return Err(BusError::invalid("video.loudness", "loudness is -40 to -5 LUFS, peak -9 to 0 dBTP"));
    }
    if format == Format::Gif && (loudness.is_some() || peak.is_some()) {
        return Err(BusError::invalid("video.loudness", "a GIF has no sound to bring to a loudness"));
    }
    Ok(Job { composition, props, range, format, quality: job.quality.unwrap_or(Quality::Standard), name, loudness, peak })
}

/// Queue `jobs` of `video` in the transaction: the code's version recorded, a row each, the queue
/// started once it commits; the rows in order.
fn queue(ctx: &mut Ctx, video_id: Id, scan: &versions::Scan, jobs: &[Job]) -> Result<Vec<VideoRender>, BusError> {
    let video = live_video(ctx.tx(), video_id)?;
    assert_actor_project(ctx, video.project_id)?;
    let by = actor_name(ctx)?;
    let (version, new) = versions::record(ctx.tx(), video.id, scan, &by, &ctx.now)?;
    let mut renders = Vec::with_capacity(jobs.len());
    for job in jobs {
        ctx.tx().execute(
            "INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, props, range_from, range_to,
                name, loudness, peak, actor, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![video.id, video.project_id, job.composition, job.format.name(), quality_name(job.quality), version,
                job.props.to_string(), job.range.map(|r| r[0]), job.range.map(|r| r[1]), job.name, job.loudness, job.peak, by, ctx.now],
        ).bus()?;
        renders.push(get_render(ctx.tx(), ctx.tx().last_insert_rowid())?);
    }
    ctx.set_project(video.project_id);
    if new {
        ctx.emit("video.changed", serde_json::to_value(get_video(ctx.tx(), video.id)?).bus()?);
    }
    for render in &renders {
        for (ev, payload) in jobs::events(render) {
            ctx.emit(&ev, payload);
        }
    }
    ctx.after_commit(|engine| jobs::enqueue(&engine));
    Ok(renders)
}

/// A file an agent names: an absolute path as it is, a relative one in its own checkout.
fn source_path(ctx: &mut Unlocked, project_id: Id, raw: &str) -> Result<PathBuf, BusError> {
    let raw = Path::new(raw);
    if raw.is_absolute() {
        return Ok(raw.to_path_buf());
    }
    let session_id = ctx.actor_session_id().ok_or_else(|| BusError::invalid("video.asset", "give the file's absolute path"))?;
    let (project, chosen) = ctx.read(|conn| crate::handlers::file::root_choice(conn, Some(session_id), project_id, None))?;
    Ok(crate::handlers::file::root_verify(&project, chosen, "video.asset")?.join(raw))
}

/// A file a measuring op names: an asset by its `staticFile()` name, or an absolute path.
fn asset_path(dir: &Path, raw: &str) -> Result<PathBuf, BusError> {
    let raw_path = Path::new(raw.trim());
    let path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        if raw_path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            return Err(BusError::invalid("video.asset", format!("{raw:?} is not a file under public/")));
        }
        dir.join("public").join(raw_path)
    };
    if !path.is_file() {
        return Err(BusError::invalid("video.asset", format!("{} is not a file", path.display()))
            .with_hint("video.asset.list names the video's files"));
    }
    Ok(path)
}

/// A file's length in seconds, by ffprobe; 0 when it cannot be read.
fn length_of(engine: &Engine, path: &Path) -> f64 {
    video::media::probe(engine, path).ok()
        .and_then(|info| info["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()))
        .map_or(0.0, |s| (s * 1000.0).round() / 1000.0)
}

/// The grid as the bus says it, in the video's frames at `fps`.
fn beat_grid(grid: video::beats::Grid, fps: f64) -> BeatGrid {
    let r3 = |v: f64| (v * 1000.0).round() / 1000.0;
    let frame = |s: f64| (s * fps).round().max(0.0) as u32;
    BeatGrid {
        bpm: (grid.bpm * 1000.0).round() / 1000.0,
        beat_s: (grid.beat_s * 100000.0).round() / 100000.0,
        bar_s: (grid.beat_s * grid.beats_per_bar as f64 * 100000.0).round() / 100000.0,
        beats_per_bar: grid.beats_per_bar,
        first_beat_s: r3(grid.first_beat_s),
        first_downbeat_s: r3(grid.first_downbeat_s),
        fps,
        frames_per_beat: (grid.beat_s * fps * 1000.0).round() / 1000.0,
        first_downbeat_frame: (grid.first_downbeat_s * fps * 100.0).round() / 100.0,
        beats_found: grid.beats_found,
        fit_rms_ms: (grid.fit_rms_ms * 10.0).round() / 10.0,
        beat_confidence: (grid.beat_confidence * 100.0).round() / 100.0,
        downbeat_confidence: (grid.downbeat_confidence * 100.0).round() / 100.0,
        seconds: r3(grid.seconds),
        kit: format!("useBeatGrid({{ bpm: {:.3}, firstBeat: {:.3}, beatsPerBar: {} }})", grid.bpm, grid.first_downbeat_s, grid.beats_per_bar),
        sections: grid.sections.iter().map(|s| BeatSection { bar: s.bar, start_s: r3(s.start_s), frame: frame(s.start_s), kind: s.kind.into(), change_db: s.change_db }).collect(),
        bars: grid.bars.iter().map(|b| BeatBar { n: b.n, start_s: r3(b.start_s), frame: frame(b.start_s), loudness_db: (b.loudness_db * 10.0).round() / 10.0 }).collect(),
    }
}

/// `source` as an H.264 clip at a constant `fps` in a folder of its own under `public/`, for
/// `assets::add` to take in its place.
fn constant_rate(engine: &Engine, dir: &Path, source: &Path, fps: f64) -> Result<PathBuf, BusError> {
    let stem = source.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "clip".into());
    let tmp = dir.join("public").join(format!(".{}-cfr", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp).map_err(|e| BusError::unavailable("video.folder", format!("{}: {e}", tmp.display())))?;
    let out = tmp.join(format!("{stem}.mp4"));
    // A failed or timed-out conversion leaves nothing behind (a partial clip can be GBs, and the
    // worker would link it into every bundle).
    if let Err(e) = video::media::constant_rate(engine, source, &out, fps) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    Ok(out)
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<List>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let rest = if p.include_deleted.unwrap_or(false) {
            "WHERE v.project_id = ?1 AND (?2 IS NULL OR v.session = ?2) ORDER BY v.updated_at DESC, v.id DESC"
        } else {
            "WHERE v.project_id = ?1 AND v.deleted_at IS NULL AND (?2 IS NULL OR v.session = ?2) ORDER BY v.updated_at DESC, v.id DESC"
        };
        let videos = ctx.read(|conn| {
            check_project(conn, p.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(&select_videos(rest)).bus()?;
            let videos = stmt.query_map(params![p.project_id, p.session], video_row).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(videos)
        })?;
        Ok(ListOut { videos })
    });

    e.register_unlocked::<Get>(|ctx, p| {
        let video = read_video(ctx, p.video_id, false)?;
        // Only the rows are read with the store held: the manifests (up to thousands of entries
        // each) are parsed and compared after.
        let (compositions, props, recent, named, renders) = ctx.read(|conn| {
            let props: String = conn.prepare_cached("SELECT props FROM videos WHERE id = ?1").bus()?.query_row([video.id], |r| r.get(0)).bus()?;
            // The newest 20 and the one before them, each told by what it changed; then every
            // older one with a name, told against the one before it.
            let row = |r: &rusqlite::Row| Ok((Version { n: r.get(0)?, actor: r.get(1)?, files: r.get(2)?, created_at: r.get(3)?, changed: Vec::new(), label: r.get(5)? }, r.get::<_, String>(4)?));
            let recent = conn.prepare_cached("SELECT n, actor, files, created_at, manifest, label FROM video_versions WHERE video_id = ?1 ORDER BY n DESC LIMIT 21").bus()?
                .query_map([video.id], row).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            let mut named = Vec::new();
            if let Some((oldest, _)) = recent.get(19).filter(|_| recent.len() > 20) {
                let rows = conn.prepare_cached("SELECT n, actor, files, created_at, manifest, label FROM video_versions
                    WHERE video_id = ?1 AND n < ?2 AND label IS NOT NULL ORDER BY n DESC").bus()?
                    .query_map(params![video.id, oldest.n], row).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
                for (v, manifest) in rows {
                    let before: Option<String> = conn.prepare_cached("SELECT manifest FROM video_versions WHERE video_id = ?1 AND n < ?2 ORDER BY n DESC LIMIT 1").bus()?
                        .query_row(params![video.id, v.n], |r| r.get(0)).optional().bus()?;
                    named.push((v, manifest, before));
                }
            }
            let renders = conn.prepare_cached(&format!("SELECT {RENDER_COLUMNS} FROM video_renders WHERE video_id = ?1 ORDER BY id DESC LIMIT 20")).bus()?
                .query_map([video.id], render_row).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok((compositions(conn, video.id)?, serde_json::from_str(&props).unwrap_or(json!({})), recent, named, renders))
        })?;
        let parse = |m: &str| -> Vec<versions::Entry> { serde_json::from_str(m).unwrap_or_default() };
        let changed = |n: i64, before: Option<&Vec<versions::Entry>>, after: &[versions::Entry]| match before {
            Some(before) => versions::changes(before, after, 12),
            None if n == 1 => vec!["the starter".to_string()],
            None => Vec::new(),
        };
        let manifests: Vec<Vec<versions::Entry>> = recent.iter().map(|(_, m)| parse(m)).collect();
        let mut versions = recent.into_iter().take(20).enumerate().map(|(i, (v, _))| {
            Version { changed: changed(v.n, manifests.get(i + 1), &manifests[i]), ..v }
        }).collect::<Vec<_>>();
        for (v, manifest, before) in named {
            let before = before.as_deref().map(parse);
            versions.push(Version { changed: changed(v.n, before.as_ref(), &parse(&manifest)), ..v });
        }
        let entry = Path::new(&video.dir).join("src/Video.tsx").display().to_string();
        Ok(GetOut { video, entry, compositions, props, versions, renders })
    });

    e.register::<Create>(|ctx: &mut Ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let by = actor_name(ctx)?;
        let title = title(p.title.as_deref())?;
        let session = p.session.or_else(|| ctx.actor.session_name().map(str::to_string));
        if let Some(session) = &session {
            check_session(ctx, p.project_id, session)?;
        }
        let (w, h) = match (p.width, p.height) {
            (Some(w), Some(h)) => (w, h),
            (None, None) => p.shape.unwrap_or(relay_bus::ops::video::Shape::Vertical).size(),
            _ => return Err(BusError::invalid("video.size", "give both width and height, or a shape")),
        };
        if !(16..=MAX_SIDE).contains(&w) || !(16..=MAX_SIDE).contains(&h) || w % 2 != 0 || h % 2 != 0 {
            return Err(BusError::invalid("video.size", format!("a video is 16 to {MAX_SIDE} px a side, and even (H.264 needs it): not {w} × {h}")));
        }
        let fps = p.fps.unwrap_or(30.0);
        if !(1.0..=120.0).contains(&fps) {
            return Err(BusError::invalid("video.fps", "1 to 120 frames a second"));
        }
        let seconds = p.seconds.unwrap_or(10.0);
        if !(1.0..=MAX_SECONDS).contains(&seconds) {
            return Err(BusError::invalid("video.seconds", format!("1 to {MAX_SECONDS} seconds to start with; the code may make it longer")));
        }
        let frames = (seconds * fps).round().max(1.0) as u32;
        ctx.tx().execute(
            "INSERT INTO videos(project_id, session, title, dir, width, height, fps, duration_frames, created_by, updated_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?9)",
            params![p.project_id, session, title, w, h, fps, frames, by, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        let dir: PathBuf = video::project_dir(&ctx.engine().store.videos_dir(), p.project_id).join(format!("{id}-{}", video::slug(&title)));
        ctx.tx().execute("UPDATE videos SET dir = ?2 WHERE id = ?1", params![id, dir.display().to_string()]).bus()?;
        // A handful of small files: written in the transaction, so a video row always has its folder.
        let files = write_starter(&dir, &title, w, h, fps, frames)
            .map_err(|e| BusError::unavailable("video.folder", format!("could not write the video's folder: {e}")))?;
        // The starter is version 1: replacing it whole is then a rewrite a version undoes.
        let starter = versions::scan(&dir, &video::blobs_dir(ctx.engine()))
            .map_err(|e| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}")))?;
        versions::record(ctx.tx(), id, &starter, &by, &ctx.now)?;
        ctx.set_undo("video.delete", json!({"video_id": id}), None);
        // While the agent writes its code, the worker starts and builds the starter: its first
        // check then finds the browser open and the bundler's and TypeScript's caches warm
        // (docs/perf/VIDEO-2026-10-21.md). Nothing is recorded; a kit not set up skips it.
        let (warm_dir, warm_bundle) = (dir.display().to_string(), video::bundle_dir(ctx.engine(), id));
        ctx.after_commit(move |engine| {
            let _ = std::thread::Builder::new().name("video-warm".into()).spawn(move || {
                if kit::ready(&engine).is_ok() {
                    let _ = video::worker::call(&engine, "check", json!({"dir": warm_dir, "bundle_dir": warm_bundle}), std::time::Duration::from_secs(180));
                }
            });
        });
        let video = emit(ctx, id)?;
        let kit = kit::status(ctx.engine());
        let next = match kit.state {
            KitState::Ready => "Edit src/Video.tsx (and src/Root.tsx for its props and length) with your file tools, then video.check, then video.still to look at it.".to_string(),
            _ => "Edit src/Video.tsx (and src/Root.tsx) with your file tools. To look at it, the kit must be set up first: call video.kit.install.".to_string(),
        };
        let entry = dir.join("src/Video.tsx").display().to_string();
        Ok(CreateOut { video, entry, files, kit, next })
    });

    e.register::<Update>(|ctx: &mut Ctx, p| {
        let before = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, before.project_id)?;
        let by = actor_name(ctx)?;
        let title = match p.title.as_deref() {
            Some(raw) => title(Some(raw))?,
            None => before.title.clone(),
        };
        let session = match p.session {
            None => before.session.clone(),
            Some(None) => None,
            Some(Some(session)) if before.session.as_deref() == Some(session.as_str()) => Some(session),
            Some(Some(session)) => {
                check_session(ctx, before.project_id, &session)?;
                Some(session)
            }
        };
        let known = compositions(ctx.tx(), before.id)?;
        let composition = match p.composition {
            Some(c) if c != before.composition => {
                if !known.is_empty() && !known.iter().any(|k| k.id == c) {
                    return Err(BusError::invalid("video.composition", format!("no composition {c:?}"))
                        .with_details(json!({"compositions": known.iter().map(|k| &k.id).collect::<Vec<_>>()})));
                }
                c
            }
            _ => before.composition.clone(),
        };
        let stored: String = ctx.tx().prepare_cached("SELECT props FROM videos WHERE id = ?1").bus()?.query_row([before.id], |r| r.get(0)).bus()?;
        let mut props: Map<String, Value> = serde_json::from_str(&stored).unwrap_or_default();
        let old_props = props.clone();
        for (comp, value) in p.props.unwrap_or_default() {
            match value {
                Value::Null => {
                    props.remove(&comp);
                }
                Value::Object(set) => {
                    check_props(known.iter().find(|k| k.id == comp).and_then(|k| k.props_schema.as_ref()), &comp, &set)?;
                    props.insert(comp, Value::Object(set));
                }
                _ => return Err(BusError::invalid("video.props", format!("props for {comp:?} are an object, or null to drop them"))),
            }
        }
        if title == before.title && session == before.session && composition == before.composition && props == old_props {
            return Ok(before);
        }
        // The undo puts each composition's props back as they were, and null for one that had none.
        let mut undo_props = Map::new();
        for comp in old_props.keys().chain(props.keys()) {
            undo_props.insert(comp.clone(), old_props.get(comp).cloned().unwrap_or(Value::Null));
        }
        ctx.tx().execute(
            "UPDATE videos SET title = ?2, session = ?3, composition = ?4, props = ?5, updated_by = ?6, updated_at = ?7 WHERE id = ?1",
            params![before.id, title, session, composition, Value::Object(props).to_string(), by, ctx.now],
        ).bus()?;
        let video = get_video(ctx.tx(), before.id)?;
        ctx.set_undo("video.update", json!({"video_id": before.id, "title": before.title, "session": before.session,
            "composition": before.composition, "props": undo_props}), None);
        ctx.set_project(video.project_id);
        ctx.emit("video.changed", serde_json::to_value(&video).bus()?);
        Ok(video)
    });

    e.register::<Delete>(|ctx: &mut Ctx, p| {
        let video = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        let by = actor_name(ctx)?;
        ctx.tx().execute("UPDATE videos SET deleted_at = ?2, updated_at = ?2, updated_by = ?3 WHERE id = ?1", params![video.id, ctx.now, by]).bus()?;
        ctx.set_undo("video.restore", json!({"video_id": video.id}), None);
        let dir = video.dir.clone();
        ctx.after_commit(move |engine| video::worker::forget(&engine, &dir));
        emit(ctx, video.id)?;
        Ok(Empty {})
    });

    e.register::<Restore>(|ctx: &mut Ctx, p| {
        let video = get_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        if video.deleted_at.is_none() {
            return Ok(video);
        }
        let by = actor_name(ctx)?;
        ctx.tx().execute("UPDATE videos SET deleted_at = NULL, updated_at = ?2, updated_by = ?3 WHERE id = ?1", params![video.id, ctx.now, by]).bus()?;
        ctx.set_undo("video.delete", json!({"video_id": video.id}), None);
        emit(ctx, video.id)
    });

    // Unlocked (D149): Node bundles and typechecks with nothing held; the version and the
    // compositions it finds are system writes of their own.
    e.register_unlocked::<Check>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let actor = caller_name(ctx);
        stills::check(ctx.engine(), &video, &Caller { actor: &actor, session_id: ctx.actor_session_id() })
    });

    e.register_unlocked::<Still>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let actor = caller_name(ctx);
        stills::still(ctx.engine(), &video, &p, &Caller { actor: &actor, session_id: ctx.actor_session_id() })
    });

    e.register_unlocked::<Scan>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let actor = caller_name(ctx);
        stills::scan(ctx.engine(), &video, &p, &Caller { actor: &actor, session_id: ctx.actor_session_id() })
    });

    // Unlocked: a pass of every frame without pictures, and the mix, with nothing held.
    e.register_unlocked::<Timeline>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let actor = caller_name(ctx);
        stills::timeline_of(ctx.engine(), &video, &p, &Caller { actor: &actor, session_id: ctx.actor_session_id() })
    });

    // Staged: the folder is read (and its new files kept) with the lock released; the
    // transaction records the version and queues the render. With `wait`, the answer is finished
    // with nothing locked once the render is (`jobs::wait`, on the row: never missed).
    e.register_staged::<Render, StagedRender>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        kit::ready(ctx.engine())?;
        let scan = versions::scan(Path::new(&video.dir), &video::blobs_dir(ctx.engine()))
            .map_err(|e| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}")))?;
        let job = prepare(ctx, &video, RenderJob {
            composition: p.composition.clone(), format: p.format, quality: p.quality, from: p.from.clone(), to: p.to.clone(),
            props: p.props.clone(), name: p.name.clone(), loudness: p.loudness, peak: p.peak, preset: p.preset,
        })?;
        Ok(StagedRender { video, scan, jobs: vec![job] })
    }, |ctx: &mut Ctx, p, staged| {
        let render = queue(ctx, staged.video.id, &staged.scan, &staged.jobs)?.remove(0);
        if p.wait == Some(true) {
            let (id, within) = (render.id, jobs::wait_for(p.timeout_ms));
            ctx.finish_reply(move |engine, reply| {
                if let Ok((renders, _)) = jobs::wait(engine, &[id], within) {
                    if let Some(value) = renders.into_iter().next().and_then(|r| serde_json::to_value(r).ok()) {
                        *reply = value;
                    }
                }
            });
        }
        Ok(render)
    });

    e.register_staged::<RenderBatch, StagedRender>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        kit::ready(ctx.engine())?;
        if p.jobs.is_empty() || p.jobs.len() > relay_bus::ops::video::MAX_BATCH {
            return Err(BusError::invalid("video.batch", format!("1 to {} renders in one batch", relay_bus::ops::video::MAX_BATCH)));
        }
        let scan = versions::scan(Path::new(&video.dir), &video::blobs_dir(ctx.engine()))
            .map_err(|e| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}")))?;
        let jobs = p.jobs.iter().cloned().map(|job| prepare(ctx, &video, job)).collect::<Result<Vec<_>, _>>()?;
        Ok(StagedRender { video, scan, jobs })
    }, |ctx: &mut Ctx, p, staged| {
        let renders = queue(ctx, staged.video.id, &staged.scan, &staged.jobs)?;
        if p.wait == Some(true) {
            let (ids, within) = (renders.iter().map(|r| r.id).collect::<Vec<_>>(), jobs::wait_for(p.timeout_ms));
            ctx.finish_reply(move |engine, reply| {
                if let Ok((renders, timed_out)) = jobs::wait(engine, &ids, within) {
                    if let Ok(value) = serde_json::to_value(RendersOut { renders, timed_out }) {
                        *reply = value;
                    }
                }
            });
        }
        Ok(RendersOut { renders, timed_out: false })
    });

    // Unlocked: it sleeps on the queue's changes, reading the rows each time one changes.
    e.register_unlocked::<RenderWait>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        if p.render_ids.is_empty() || p.render_ids.len() > 50 {
            return Err(BusError::invalid("video.render", "1 to 50 renders to wait for"));
        }
        ctx.read(|conn| {
            for id in &p.render_ids {
                check_project(conn, get_render(conn, *id)?.project_id, session_id, agent)?;
            }
            Ok(())
        })?;
        let (renders, timed_out) = jobs::wait(ctx.engine(), &p.render_ids, jobs::wait_for(p.timeout_ms))?;
        Ok(RendersOut { renders, timed_out })
    });

    e.register_unlocked::<RenderGet>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        ctx.read(|conn| {
            let render = get_render(conn, p.render_id)?;
            check_project(conn, render.project_id, session_id, agent)?;
            Ok(render)
        })
    });

    e.register::<RenderCancel>(|ctx: &mut Ctx, p| {
        let render = get_render(ctx.tx(), p.render_id)?;
        assert_actor_project(ctx, render.project_id)?;
        match render.state {
            RenderState::Queued => {
                ctx.tx().execute("UPDATE video_renders SET state = 'cancelled', finished_at = ?2 WHERE id = ?1", params![render.id, ctx.now]).bus()?;
                let render = get_render(ctx.tx(), render.id)?;
                ctx.after_commit(|engine| jobs::notify(&engine));
                ctx.set_project(render.project_id);
                for (ev, payload) in jobs::events(&render) {
                    ctx.emit(&ev, payload);
                }
                Ok(render)
            }
            RenderState::Rendering => {
                let running = render.clone();
                ctx.after_commit(move |engine| {
                    let _ = std::thread::Builder::new().name("video-render-cancel".into()).spawn(move || jobs::cancel(&engine, &running));
                });
                ctx.set_project(render.project_id);
                ctx.emit("video.render.changed", serde_json::to_value(&render).bus()?);
                Ok(render)
            }
            _ => Ok(render),
        }
    });

    // Staged: a clip may be gigabytes, copied with nothing locked. Like design.asset.add it changes
    // no row, so it emits nothing (BUS.md §3.3's exemption).
    e.register_staged::<AssetAdd, StagedAsset>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let dir = PathBuf::from(&video.dir);
        let (overwrite, link) = (p.overwrite.unwrap_or(false), p.link.unwrap_or(false));
        let given = [p.path.is_some(), p.paths.is_some(), p.font.is_some()].iter().filter(|g| **g).count();
        if given != 1 {
            return Err(BusError::invalid("video.asset", "give one of path, paths (several files) or font (an installed family)"));
        }
        if let Some(family) = p.font.as_deref() {
            let weights = p.weights.clone().unwrap_or_else(|| vec![400, 700]);
            if weights.is_empty() || weights.iter().any(|w| !(100..=900).contains(w)) {
                return Err(BusError::invalid("video.font", "weights are 100 to 900"));
            }
            let faces = assets::faces(family.trim())?;
            if faces.is_empty() {
                return Err(BusError::invalid("video.font", format!("no font family {family:?} is installed"))
                    .with_hint("fc-list : family lists the installed families; or add the font's files by path"));
            }
            let (chosen, missing) = assets::pick(&faces, &weights, p.italic.unwrap_or(false));
            if chosen.is_empty() {
                return Err(BusError::invalid("video.font", format!("{family} has none of those weights: {}", missing.join(", "))));
            }
            let mut added = Vec::new();
            let mut files = Vec::new();
            for (face, _) in &chosen {
                let asset = assets::add(&dir, &assets::Add { source: &face.file, folder: "assets/fonts", name: None, overwrite, link })?;
                files.push((asset.static_file.clone(), face));
                added.push(asset);
            }
            let font = assets::font_added(family.trim(), &files, missing);
            return Ok(StagedAsset { out: AssetAddOut { assets: added, font: Some(font) } });
        }
        let raws: Vec<String> = match (&p.path, &p.paths) {
            (Some(path), _) => vec![path.clone()],
            (_, Some(paths)) => paths.clone(),
            _ => Vec::new(),
        };
        if raws.is_empty() || raws.len() > MAX_ASSETS {
            return Err(BusError::invalid("video.asset", format!("1 to {MAX_ASSETS} files in one call")));
        }
        if p.name.is_some() && raws.len() > 1 {
            return Err(BusError::invalid("video.asset", "name is for one file; several keep their own names"));
        }
        if p.fps.is_some_and(|f| !(1.0..=120.0).contains(&f)) {
            return Err(BusError::invalid("video.asset", "fps is 1 to 120"));
        }
        let mut added = Vec::with_capacity(raws.len());
        for raw in &raws {
            let source = source_path(ctx, video.project_id, raw)?;
            let Some(fps) = p.fps.filter(|_| assets::kind(&source.to_string_lossy()) == "video") else {
                added.push(assets::add(&dir, &assets::Add { source: &source, folder: "assets", name: p.name.as_deref(), overwrite, link })?);
                continue;
            };
            // Converted: a new file, so copied whatever `link` says.
            let converted = constant_rate(ctx.engine(), &dir, &source, fps)?;
            let name = p.name.clone().map(|n| assets::target_name(&converted, Some(&n)));
            let result = assets::add(&dir, &assets::Add { source: &converted, folder: "assets", name: name.as_deref(), overwrite, link: false });
            if let Some(tmp) = converted.parent() {
                let _ = fs::remove_dir_all(tmp);
            }
            added.push(result?);
        }
        Ok(StagedAsset { out: AssetAddOut { assets: added, font: None } })
    }, |ctx: &mut Ctx, p, staged| {
        commit_to(ctx, p.video_id)?;
        Ok(staged.out)
    });

    // Unlocked: ffprobe and ffmpeg read the file with nothing held.
    e.register_unlocked::<AssetInspect>(|ctx, p| {
        let video = read_video(ctx, p.video_id, false)?;
        let dir = PathBuf::from(&video.dir);
        let path = asset_path(&dir, &p.asset)?;
        video::inspect::inspect(ctx.engine(), &dir, &path, p.motion)
    });

    e.register_unlocked::<AudioBeats>(|ctx, p| {
        let video = read_video(ctx, p.video_id, false)?;
        let path = asset_path(Path::new(&video.dir), &p.asset)?;
        let pcm = video::media::decode(ctx.engine(), &path, video::beats::RATE, 1)?
            .ok_or_else(|| BusError::invalid("video.audio", format!("{} has no sound", p.asset)))?;
        let grid = video::beats::analyse(&pcm, p.beats_per_bar.unwrap_or(4).clamp(1, 12), p.bpm)
            .ok_or_else(|| BusError::invalid("video.audio.no_beat", format!("{} has no steady beat to find", p.asset))
                .with_hint("speech, a drone or a free-time piece has none; give bpm if you know it"))?;
        Ok(beat_grid(grid, video.fps))
    });

    // ElevenLabs: the network and the file with nothing locked; the transaction only checks the
    // video is still there and the caller's (as asset.add, nothing on a row changes).
    e.register_staged::<relay_bus::ops::video::Voice, Staged<VoiceOut>>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let text = p.text.trim();
        if text.is_empty() {
            return Err(BusError::invalid("video.voice", "say something"));
        }
        let unit = |v: Option<f64>, lo: f64, hi: f64, name: &str| -> Result<Option<f64>, BusError> {
            match v {
                Some(v) if !(lo..=hi).contains(&v) => Err(BusError::invalid("video.voice", format!("{name} is {lo} to {hi}"))),
                v => Ok(v),
            }
        };
        let settings = json!({
            "stability": unit(p.stability, 0.0, 1.0, "stability")?.unwrap_or(0.5),
            "similarity_boost": unit(p.similarity, 0.0, 1.0, "similarity")?.unwrap_or(0.75),
            "style": unit(p.style, 0.0, 1.0, "style")?.unwrap_or(0.0),
            "speed": unit(p.speed, 0.7, 1.2, "speed")?.unwrap_or(1.0),
            "use_speaker_boost": true,
        });
        let engine = ctx.engine();
        let (voice_id, voice) = video::eleven::voice_id(engine, p.voice.trim())?;
        let say = p.say.clone().unwrap_or_default();
        let (said, counts) = video::eleven::spoken(text, &say);
        let model = p.model.clone().unwrap_or_else(|| video::eleven::TTS_MODEL.into());
        let (audio, alignment) = video::eleven::speak(engine, &voice_id, &said, &model, settings.clone())?;
        let words = video::eleven::written(text, &counts, video::eleven::words_of(&alignment));
        let seconds = words.last().map_or(0.0, |w| w.end);
        let captions = video::eleven::captions(&words);
        let name = p.name.clone().unwrap_or_else(|| video::eleven::name_from("vo", text));
        let made = json!({"made_by": "video.voice", "provider": "elevenlabs", "text": text, "said": said, "voice": voice, "voice_id": voice_id,
            "model": model, "settings": settings, "words": words, "captions": captions, "seconds": seconds, "made_at": crate::time::now()});
        let asset = video::eleven::keep(Path::new(&video.dir), &audio, &name, "mp3", p.overwrite.unwrap_or(false), made)?;
        Ok(Staged(VoiceOut { asset, seconds, words, captions, voice_id, voice, model }))
    }, |ctx: &mut Ctx, p, staged| {
        commit_to(ctx, p.video_id)?;
        Ok(staged.0)
    });

    // Unlocked: Studio opens (its first bundle) with nothing held.
    e.register_unlocked::<Preview>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        if p.stop == Some(true) {
            if video::preview::running(ctx.engine(), video.id).is_some() {
                video::preview::stop(ctx.engine());
            }
            return Ok(PreviewOut { url: None, running: false, started: false });
        }
        let composition = p.composition.clone().unwrap_or_else(|| video.composition.clone());
        let (url, started) = video::preview::open(ctx.engine(), &video, &composition)?;
        Ok(PreviewOut { url: Some(url), running: true, started })
    });

    // Staged: the device is recorded with nothing locked (it holds the device's own lease).
    e.register_staged::<CaptureAndroid, Staged<CaptureOut>>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let fps = p.fps.unwrap_or(30.0);
        let tail = p.tail.unwrap_or(1.0);
        if !(1.0..=60.0).contains(&fps) || !(0.0..=30.0).contains(&tail) {
            return Err(BusError::invalid("video.capture", "fps is 1 to 60, tail 0 to 30 seconds"));
        }
        let adb = ctx.read(crate::handlers::device::adb_path)?;
        let holder = match (ctx.actor_session_id(), ctx.actor.session_name()) {
            (Some(id), Some(name)) => crate::device_lease::Holder::Session { id, name: name.to_string() },
            _ => crate::device_lease::Holder::User,
        };
        let plan = video::capture::Plan {
            device: p.device.as_deref(), launch: p.launch.as_deref(), steps: &p.steps, tail, fps, demo: p.demo.unwrap_or(true),
            name: p.name.as_deref(), overwrite: p.overwrite.unwrap_or(false),
        };
        let c = video::capture::capture(ctx, &adb, Path::new(&video.dir), holder, &plan)?;
        Ok(Staged(CaptureOut { asset: c.asset, seconds: c.seconds, fps, frames: c.frames, steps: c.marks, motion: c.motion, device: c.device }))
    }, |ctx: &mut Ctx, p, staged| {
        commit_to(ctx, p.video_id)?;
        Ok(staged.0)
    });

    e.register_unlocked::<Voices>(|ctx, p| {
        let mut voices: Vec<VoiceInfo> = video::eleven::voices(ctx.engine())?;
        if let Some(search) = p.search.as_deref().map(str::to_lowercase).filter(|s| !s.is_empty()) {
            voices.retain(|v| v.name.to_lowercase().contains(&search) || v.description.as_deref().is_some_and(|d| d.to_lowercase().contains(&search)));
        }
        Ok(VoicesOut { voices })
    });

    e.register_staged::<Sfx, Staged<MadeOut>>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        if p.seconds.is_some_and(|s| !(0.5..=30.0).contains(&s)) || p.influence.is_some_and(|i| !(0.0..=1.0).contains(&i)) {
            return Err(BusError::invalid("video.sfx", "seconds is 0.5 to 30, influence 0 to 1"));
        }
        let audio = video::eleven::sound_effect(ctx.engine(), p.prompt.trim(), p.seconds, p.influence, p.looped.unwrap_or(false))?;
        let name = p.name.clone().unwrap_or_else(|| video::eleven::name_from("sfx", &p.prompt));
        let made = json!({"made_by": "video.sfx", "provider": "elevenlabs", "prompt": p.prompt.trim(), "seconds": p.seconds,
            "influence": p.influence, "loop": p.looped.unwrap_or(false), "made_at": crate::time::now()});
        let asset = video::eleven::keep(Path::new(&video.dir), &audio, &name, "mp3", p.overwrite.unwrap_or(false), made)?;
        let seconds = length_of(ctx.engine(), Path::new(&asset.path));
        Ok(Staged(MadeOut { asset, seconds }))
    }, |ctx: &mut Ctx, p, staged| {
        commit_to(ctx, p.video_id)?;
        Ok(staged.0)
    });

    e.register_staged::<Music, Staged<MadeOut>>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        if !(3.0..=600.0).contains(&p.seconds) {
            return Err(BusError::invalid("video.music", "music is 3 to 600 seconds"));
        }
        let model = p.model.clone().unwrap_or_else(|| video::eleven::MUSIC_MODEL.into());
        let instrumental = p.instrumental.unwrap_or(true);
        let audio = video::eleven::compose(ctx.engine(), p.prompt.trim(), p.seconds, instrumental, &model)?;
        let name = p.name.clone().unwrap_or_else(|| video::eleven::name_from("music", &p.prompt));
        let made = json!({"made_by": "video.music", "provider": "elevenlabs", "prompt": p.prompt.trim(), "seconds": p.seconds,
            "instrumental": instrumental, "model": model, "made_at": crate::time::now()});
        let asset = video::eleven::keep(Path::new(&video.dir), &audio, &name, "mp3", p.overwrite.unwrap_or(false), made)?;
        let seconds = length_of(ctx.engine(), Path::new(&asset.path));
        Ok(Staged(MadeOut { asset, seconds }))
    }, |ctx: &mut Ctx, p, staged| {
        commit_to(ctx, p.video_id)?;
        Ok(staged.0)
    });

    e.register_unlocked::<Transcribe>(|ctx, p| {
        let video = read_video(ctx, p.video_id, false)?;
        let dir = PathBuf::from(&video.dir);
        let path = asset_path(&dir, &p.asset).or_else(|e| {
            // A render, by its name in out/: one plain name, so `../` cannot leave the folder.
            let name = Path::new(p.asset.trim());
            let plain = matches!(name.components().collect::<Vec<_>>()[..], [std::path::Component::Normal(_)]);
            let out = dir.join("out").join(name);
            if plain && out.is_file() { Ok(out) } else { Err(e) }
        })?;
        let (text, language, words) = video::eleven::hear(ctx.engine(), &path, p.language.as_deref())?;
        let misheard = p.expect.as_deref().map(|e| video::eleven::misheard(e, &words)).unwrap_or_default();
        Ok(TranscribeOut { text, language, words, misheard })
    });

    e.register_unlocked::<ElevenStatusOp>(|ctx, _| video::eleven::status(ctx.engine()));

    // Staged: the key is checked with ElevenLabs before it is kept, with nothing locked.
    e.register_staged::<ElevenKeySet, Staged<relay_bus::ops::video::ElevenStatus>>(|ctx, p| {
        let key = video::eleven::check_format(&p.key)?;
        Ok(Staged(video::eleven::save(ctx.engine(), key)?))
    }, |_ctx: &mut Ctx, _p, staged| Ok(staged.0));

    e.register_staged::<ElevenKeyRemove, Staged<relay_bus::ops::video::ElevenStatus>>(|ctx, _| Ok(Staged(video::eleven::remove(ctx.engine())?)),
        |_ctx: &mut Ctx, _p, staged| Ok(staged.0));

    e.register_unlocked::<AssetList>(|ctx, p| {
        let video = read_video(ctx, p.video_id, false)?;
        Ok(AssetListOut { assets: assets::list(Path::new(&video.dir)) })
    });

    // Staged: the folder is put back with nothing locked, the code as it was recorded first, so
    // a restore of that version undoes this one.
    e.register_staged::<VersionRestore, StagedRestore>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let entries = ctx.read(|conn| versions::manifest(conn, video.id, p.version))?;
        let (dir, blobs) = (PathBuf::from(&video.dir), video::blobs_dir(ctx.engine()));
        let folder = |e: String| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}"));
        let before = versions::scan(&dir, &blobs).map_err(folder)?;
        let only = match p.paths.as_deref() {
            Some(raw) => Some(only_paths(raw, &[&entries, &before.entries], &format!("v{} or the code now", p.version))?),
            None => None,
        };
        // The code as it is now is a version before anything is written, so a restore that fails
        // halfway can still be undone from it. Inside a transaction already (an undo's replay)
        // the commit below records it.
        if !ctx.holds_store() {
            let actor = caller_name(ctx);
            stills::snapshot(ctx.engine(), &video, &Caller { actor: &actor, session_id: ctx.actor_session_id() })?;
        }
        // One that fails partway puts the folder back as it was: in an undo's replay nothing
        // above recorded it, and the transaction's rollback would lose a record made here.
        let versions::Restored { missing, left, .. } = versions::restore_or_back(&dir, &blobs, &entries, &before.entries, only.as_deref()).map_err(folder)?;
        let after = versions::scan(&dir, &blobs).map_err(folder)?;
        Ok(StagedRestore { before, after, only, missing, left })
    }, |ctx: &mut Ctx, p, staged| {
        let video = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        let by = actor_name(ctx)?;
        let (previous, _) = versions::record(ctx.tx(), video.id, &staged.before, &by, &ctx.now)?;
        versions::record(ctx.tx(), video.id, &staged.after, &by, &ctx.now)?;
        ctx.tx().execute("UPDATE videos SET updated_at = ?2, updated_by = ?3 WHERE id = ?1", params![video.id, ctx.now, by]).bus()?;
        // A restore of some paths is undone for those paths only.
        let mut undo = json!({"video_id": video.id, "version": previous});
        if let Some(only) = &staged.only {
            undo["paths"] = json!(only);
        }
        ctx.set_undo("video.version.restore", undo, None);
        let video = emit(ctx, video.id)?;
        let changed = versions::changes(&staged.before.entries, &staged.after.entries, 100);
        Ok(VersionRestoreOut { video, previous, changed, missing: staged.missing, left: staged.left })
    });

    e.register::<VersionLabel>(|ctx: &mut Ctx, p| {
        let video = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        let label = match p.label.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(raw) if raw.chars().count() > MAX_VERSION_LABEL || !plain_label(raw) => {
                return Err(BusError::invalid("video.version", format!("a version's name is 1 to {MAX_VERSION_LABEL} characters, without control characters")));
            }
            Some(raw) => Some(raw.to_string()),
        };
        let before: Option<String> = ctx.tx().prepare_cached("SELECT label FROM video_versions WHERE video_id = ?1 AND n = ?2").bus()?
            .query_row(params![video.id, p.version], |r| r.get(0)).optional().bus()?
            .ok_or_else(|| BusError::not_found("video.version", format!("video {} has no version {}", video.id, p.version))
                .with_hint("video.get lists its versions"))?;
        if before != label {
            ctx.tx().prepare_cached("UPDATE video_versions SET label = ?3 WHERE video_id = ?1 AND n = ?2").bus()?
                .execute(params![video.id, p.version, label]).bus()?;
            ctx.set_undo("video.version.label", json!({"video_id": video.id, "version": p.version, "label": before}), None);
            emit(ctx, video.id)?;
        }
        ctx.set_project(video.project_id);
        let (actor, files, created_at): (String, u32, String) = ctx.tx()
            .prepare_cached("SELECT actor, files, created_at FROM video_versions WHERE video_id = ?1 AND n = ?2").bus()?
            .query_row(params![video.id, p.version], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?;
        Ok(Version { n: p.version, actor, files, created_at, changed: Vec::new(), label })
    });

    // Unlocked: the folder is read (and recorded, when it changed) with nothing held; the blobs are
    // files of their own.
    e.register_unlocked::<VersionDiff>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let to = match p.to {
            Some(n) => n,
            None => {
                let actor = caller_name(ctx);
                stills::snapshot(ctx.engine(), &video, &Caller { actor: &actor, session_id: ctx.actor_session_id() })?
            }
        };
        let (before, after) = ctx.read(|conn| Ok((versions::manifest(conn, video.id, p.from)?, versions::manifest(conn, video.id, to)?)))?;
        let only = match p.paths.as_deref() {
            Some(raw) => Some(only_paths(raw, &[&before, &after], &format!("v{} or v{to}", p.from))?),
            None => None,
        };
        let files = versions::diff(&video::blobs_dir(ctx.engine()), &before, &after, only.as_deref());
        Ok(VersionDiffOut { from: p.from, to, files })
    });

    e.register_unlocked::<KitStatusOp>(|ctx, _| Ok(kit::status(ctx.engine())));

    // The answer is the kit's real state: ready stays ready unless `force`, installing says how far
    // it has come, and only what needs setting up is set up.
    e.register::<KitInstall>(|ctx: &mut Ctx, p| {
        let mut status = kit::status(ctx.engine());
        // Setting it up again replaces the packages a render is using and stops the worker,
        // which would end a render as if someone had stopped it.
        if status.state == KitState::Ready && p.force == Some(true) {
            let busy: bool = ctx.tx().prepare_cached("SELECT EXISTS(SELECT 1 FROM video_renders WHERE state IN ('queued', 'rendering'))").bus()?
                .query_row([], |r| r.get(0)).bus()?;
            if busy {
                return Err(BusError::conflict("video.kit.busy", "renders are queued or running; setting the kit up again would stop them")
                    .with_hint("wait for them (video.render.wait) or stop them, then try again"));
            }
        }
        let wanted = match status.state {
            KitState::Ready => p.force == Some(true),
            KitState::NoNode | KitState::Installing => false,
            KitState::Missing | KitState::Outdated | KitState::Failed => true,
        };
        if wanted {
            ctx.after_commit(|engine| {
                kit::install(&engine);
            });
            status.state = KitState::Installing;
            status.error = None;
            status.progress = Some(kit::starting());
        }
        Ok(status)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn props_are_held_to_their_schema() {
        let schema = json!({"type": "object", "properties": {"title": {"type": "string"}, "count": {"type": "number"},
            "mode": {"type": "string", "enum": ["light", "dark"]}}, "additionalProperties": false});
        let ok = |v: Value| check_props(Some(&schema), "main", v.as_object().unwrap());
        assert!(ok(json!({"title": "Hi", "count": 3, "mode": "dark"})).is_ok());
        assert_eq!(ok(json!({"title": 3})).unwrap_err().code, "video.props");
        assert_eq!(ok(json!({"mode": "blue"})).unwrap_err().code, "video.props");
        assert_eq!(ok(json!({"other": 1})).unwrap_err().code, "video.props");
        // No schema: anything goes.
        assert!(check_props(None, "main", json!({"x": 1}).as_object().unwrap()).is_ok());
    }
}
