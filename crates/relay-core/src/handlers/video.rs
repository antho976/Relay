//! `video.*` (BUS.md §10.19, docs/VIDEO.md): videos written as code. The rows and folders are
//! here; the worker, the kit, versions, stills and renders are `crate::video`.
//!
//! What reads the folder through the worker (check, still) runs unlocked, and what changes rows
//! after slow file work (render, asset.add, version.restore) is staged (D149): no store lock is
//! ever held while Node bundles, Chrome draws or a file is copied.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::handlers::notes::{actor_name, assert_actor_project, check_project};
use crate::video::stills::{self, Caller};
use crate::video::{self, get_render, get_video, jobs, kit, live_video, quality_name, render_row, select_videos, versions, video_row, RENDER_COLUMNS};
use relay_bus::design::{plain_label, MAX_NAME};
use relay_bus::error::BusError;
use relay_bus::ops::video::{
    AssetAdd, AssetAddOut, Check, Composition, Create, CreateOut, Delete, Format, Get, GetOut, KitInstall, KitState, KitStatusOp,
    List, ListOut, Quality, Render, RenderCancel, RenderGet, RenderState, Restore, Still, Update, Version, VersionRestore,
    VersionRestoreOut, MAX_SECONDS, MAX_SIDE,
};
use relay_bus::types::Id;
use relay_bus::Empty;
use rusqlite::{params, Connection};
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

/// What a file copied in is, by its extension.
fn asset_kind(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" => "image",
        "mp4" | "webm" | "mov" | "mkv" | "m4v" => "video",
        "mp3" | "wav" | "ogg" | "m4a" | "aac" | "flac" | "opus" => "audio",
        "woff2" | "woff" | "ttf" | "otf" => "font",
        _ => "other",
    }
}

/// A file name safe in a URL and on any disk: letters, digits, dots, dashes and underscores.
fn safe_name(raw: &str) -> String {
    let name: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let name = name.trim_matches(|c| c == '.' || c == '-').chars().take(120).collect::<String>();
    if name.is_empty() { "asset".into() } else { name }
}

/// Most bytes `video.asset.add` copies (a long clip).
const MOST_ASSET: u64 = 4 * 1024 * 1024 * 1024;

pub struct StagedAsset {
    out: AssetAddOut,
}

pub struct StagedRender {
    video: relay_bus::ops::video::Video,
    scan: versions::Scan,
    composition: String,
    props: Value,
    range: Option<[u32; 2]>,
}

pub struct StagedRestore {
    before: versions::Scan,
    after: versions::Scan,
    missing: Vec<String>,
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
        let (compositions, props, versions, renders) = ctx.read(|conn| {
            let props: String = conn.prepare_cached("SELECT props FROM videos WHERE id = ?1").bus()?.query_row([video.id], |r| r.get(0)).bus()?;
            let versions = conn.prepare_cached("SELECT n, actor, files, created_at FROM video_versions WHERE video_id = ?1 ORDER BY n DESC LIMIT 20").bus()?
                .query_map([video.id], |r| Ok(Version { n: r.get(0)?, actor: r.get(1)?, files: r.get(2)?, created_at: r.get(3)? })).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            let renders = conn.prepare_cached(&format!("SELECT {RENDER_COLUMNS} FROM video_renders WHERE video_id = ?1 ORDER BY id DESC LIMIT 20")).bus()?
                .query_map([video.id], render_row).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok((compositions(conn, video.id)?, serde_json::from_str(&props).unwrap_or(json!({})), versions, renders))
        })?;
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
        ctx.set_undo("video.delete", json!({"video_id": id}), None);
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

    // Staged: the folder is read (and its new files kept) with the lock released; the
    // transaction records the version and queues the render.
    e.register_staged::<Render, StagedRender>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        kit::ready(ctx.engine())?;
        let scan = versions::scan(Path::new(&video.dir), &video::blobs_dir(ctx.engine()))
            .map_err(|e| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}")))?;
        let composition = p.composition.clone().unwrap_or_else(|| video.composition.clone());
        let props = stills::props_for(ctx.engine(), video.id, &composition, p.props.as_ref())?;
        // The range is read against the composition as the last check found it.
        let known = ctx.read(|conn| compositions(conn, video.id))?;
        let (fps, frames) = known.iter().find(|c| c.id == composition)
            .map_or((video.fps, video.duration_frames), |c| (c.fps, c.duration_frames));
        let at = |m: &Option<relay_bus::ops::video::Moment>, default: u32| -> Result<u32, BusError> {
            m.as_ref().map_or(Ok(default), |m| m.frame(fps, frames).map_err(|e| BusError::invalid("video.moment", e)))
        };
        let range = if p.from.is_some() || p.to.is_some() {
            let (a, b) = (at(&p.from, 0)?, at(&p.to, frames.saturating_sub(1))?);
            if b < a {
                return Err(BusError::invalid("video.moment", "`to` is before `from`"));
            }
            Some([a, b])
        } else {
            None
        };
        Ok(StagedRender { video, scan, composition, props, range })
    }, |ctx: &mut Ctx, p, staged| {
        let video = live_video(ctx.tx(), staged.video.id)?;
        assert_actor_project(ctx, video.project_id)?;
        let by = actor_name(ctx)?;
        let (version, new) = versions::record(ctx.tx(), video.id, &staged.scan, &by, &ctx.now)?;
        let format = p.format.unwrap_or(Format::Mp4);
        let quality = p.quality.unwrap_or(Quality::Standard);
        ctx.tx().execute(
            "INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, props, range_from, range_to, actor, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?10, ?11)",
            params![video.id, video.project_id, staged.composition, format.name(), quality_name(quality), version,
                staged.props.to_string(), staged.range.map(|r| r[0]), staged.range.map(|r| r[1]), by, ctx.now],
        ).bus()?;
        let render = get_render(ctx.tx(), ctx.tx().last_insert_rowid())?;
        ctx.set_project(video.project_id);
        if new {
            ctx.emit("video.changed", serde_json::to_value(get_video(ctx.tx(), video.id)?).bus()?);
        }
        for (ev, payload) in jobs::events(&render) {
            ctx.emit(&ev, payload);
        }
        ctx.after_commit(|engine| jobs::enqueue(&engine));
        Ok(render)
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
        let raw = Path::new(&p.path);
        let source = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            let session_id = ctx.actor_session_id().ok_or_else(|| BusError::invalid("video.asset", "give the file's absolute path"))?;
            let (project, chosen) = ctx.read(|conn| crate::handlers::file::root_choice(conn, Some(session_id), video.project_id, None))?;
            crate::handlers::file::root_verify(&project, chosen, "video.asset")?.join(raw)
        };
        let meta = fs::metadata(&source).map_err(|e| BusError::invalid("video.asset", format!("{}: {e}", source.display())))?;
        if !meta.is_file() {
            return Err(BusError::invalid("video.asset", format!("{} is not a file", source.display())));
        }
        if meta.len() > MOST_ASSET {
            return Err(BusError::invalid("video.asset", "a file of at most 4 GiB"));
        }
        let original = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut name = safe_name(p.name.as_deref().unwrap_or(&original));
        if !name.contains('.') {
            if let Some((_, ext)) = original.rsplit_once('.') {
                name = format!("{name}.{}", safe_name(ext));
            }
        }
        let folder = Path::new(&video.dir).join("public/assets");
        fs::create_dir_all(&folder).map_err(|e| BusError::unavailable("video.folder", format!("{}: {e}", folder.display())))?;
        let same = |a: &Path, b: &Path| -> bool {
            fs::metadata(a).ok().map(|m| m.len()) == fs::metadata(b).ok().map(|m| m.len()) && fs::read(a).ok() == fs::read(b).ok()
        };
        let (stem, ext) = name.rsplit_once('.').map_or((name.clone(), String::new()), |(s, e)| (s.to_string(), format!(".{e}")));
        let mut target = folder.join(&name);
        let mut n = 2;
        // Never written over: a name taken by other content gets a number.
        while target.exists() && !same(&target, &source) {
            target = folder.join(format!("{stem}-{n}{ext}"));
            n += 1;
        }
        if !target.exists() {
            let part = target.with_extension("part");
            fs::copy(&source, &part).and_then(|_| fs::rename(&part, &target))
                .map_err(|e| BusError::unavailable("video.asset", format!("could not copy {}: {e}", source.display())))?;
        }
        let file_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let kind = asset_kind(&file_name);
        let (w, h) = if kind == "image" && meta.len() <= 64 * 1024 * 1024 {
            fs::read(&target).ok().and_then(|b| crate::picture::sniff(&b)).map_or((None, None), |p| (Some(p.w), Some(p.h)))
        } else {
            (None, None)
        };
        Ok(StagedAsset { out: AssetAddOut {
            static_file: format!("assets/{file_name}"), path: target.display().to_string(), bytes: meta.len(), kind: kind.into(), w, h,
        } })
    }, |ctx: &mut Ctx, p, staged| {
        let video = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        ctx.set_project(video.project_id);
        Ok(staged.out)
    });

    // Staged: the folder is put back with nothing locked, the code as it was recorded first, so
    // a restore of that version undoes this one.
    e.register_staged::<VersionRestore, StagedRestore>(|ctx, p| {
        let video = read_video(ctx, p.video_id, true)?;
        let entries = ctx.read(|conn| versions::manifest(conn, video.id, p.version))?;
        let (dir, blobs) = (PathBuf::from(&video.dir), video::blobs_dir(ctx.engine()));
        let folder = |e: String| BusError::unavailable("video.folder", format!("could not read the video's folder: {e}"));
        let before = versions::scan(&dir, &blobs).map_err(folder)?;
        let (_, _, missing) = versions::restore(&dir, &blobs, &entries).map_err(folder)?;
        let after = versions::scan(&dir, &blobs).map_err(folder)?;
        Ok(StagedRestore { before, after, missing })
    }, |ctx: &mut Ctx, p, staged| {
        let video = live_video(ctx.tx(), p.video_id)?;
        assert_actor_project(ctx, video.project_id)?;
        let by = actor_name(ctx)?;
        let (previous, _) = versions::record(ctx.tx(), video.id, &staged.before, &by, &ctx.now)?;
        versions::record(ctx.tx(), video.id, &staged.after, &by, &ctx.now)?;
        ctx.tx().execute("UPDATE videos SET updated_at = ?2, updated_by = ?3 WHERE id = ?1", params![video.id, ctx.now, by]).bus()?;
        ctx.set_undo("video.version.restore", json!({"video_id": video.id, "version": previous}), None);
        let video = emit(ctx, video.id)?;
        if !staged.missing.is_empty() {
            tracing::warn!(video = video.id, missing = ?staged.missing, "video version restore: files whose contents were not kept");
        }
        Ok(VersionRestoreOut { video, previous })
    });

    e.register_unlocked::<KitStatusOp>(|ctx, _| Ok(kit::status(ctx.engine())));

    e.register::<KitInstall>(|ctx: &mut Ctx, _| {
        let mut status = kit::status(ctx.engine());
        if !matches!(status.state, KitState::NoNode | KitState::Installing) {
            ctx.after_commit(|engine| {
                kit::install(&engine);
            });
            status.state = KitState::Installing;
            status.error = None;
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

    #[test]
    fn copied_files_get_safe_names_and_kinds() {
        assert_eq!(safe_name("My Logo (final).PNG"), "My-Logo--final-.PNG");
        assert_eq!(safe_name("../../etc/passwd"), "etc-passwd");
        assert_eq!(asset_kind("clip.MOV"), "video");
        assert_eq!(asset_kind("Inter.woff2"), "font");
        assert_eq!(asset_kind("song.mp3"), "audio");
        assert_eq!(asset_kind("notes.txt"), "other");
    }
}
