//! Renders (`video.render`): queued rows run one at a time, oldest first, on a thread of their
//! own, each through the worker. A render's row says where it is; `video.render.changed` carries
//! it at each change of state and each tenth of the way, and `video.render.finished` once it is
//! done, failed or stopped. [`wait`] blocks on the row itself, so a render that finished before
//! anyone started waiting is never missed (`video.render.wait`, `video.render {wait}`).
//!
//! The file is drawn to a partial name beside its own and renamed once its sound is measured (and
//! brought to the loudness asked for, `audio`): a render that fails leaves the file it would have
//! replaced as it was.

use super::{bundle_dir, get_render, get_video, worker, Video};
use crate::engine::{Engine, IntoBus};
use relay_bus::error::BusError;
use relay_bus::ops::video::{AudioReport, Format, Quality, RenderState, VideoRender};
use relay_bus::types::Id;
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

/// The longest a render may run.
const RENDER_WITHIN: Duration = Duration::from_secs(6 * 60 * 60);
/// A wait looks at the rows again at least this often, whatever it was told.
const WAIT_SLICE: Duration = Duration::from_secs(5);

/// Whether a thread is running the queue, and a count of the changes to renders' rows that a
/// [`wait`] sleeps on.
#[derive(Default)]
pub struct Queue {
    running: Mutex<bool>,
    changes: Mutex<u64>,
    changed: Condvar,
    /// The renders asked to stop while running: the worker knows a render only once it has
    /// bundled, and drops it before its sound is measured, so a stop that lands outside that
    /// window is kept here and honoured when the worker answers.
    stopping: Mutex<std::collections::HashSet<Id>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Wake every [`wait`]: a render's row changed.
pub fn notify(engine: &Engine) {
    *lock(&engine.video.jobs.changes) += 1;
    engine.video.jobs.changed.notify_all();
}

/// Block until every render of `ids` has finished (done, failed or cancelled), or `within` has
/// passed: their rows as they stand, and whether it ran out. A render that finished before the
/// call counts at once.
pub fn wait(engine: &Engine, ids: &[Id], within: Duration) -> Result<(Vec<VideoRender>, bool), BusError> {
    let deadline = Instant::now() + within;
    let queue = &engine.video.jobs;
    loop {
        let seen = *lock(&queue.changes);
        let renders = engine.read_store("video.render.wait", |conn| ids.iter().map(|id| get_render(conn, *id)).collect::<Result<Vec<_>, _>>())?;
        let now = Instant::now();
        if renders.iter().all(|r| r.state.is_final()) {
            return Ok((renders, false));
        }
        if now >= deadline || engine.is_quitting() {
            return Ok((renders, true));
        }
        let guard = lock(&queue.changes);
        let slice = (deadline - now).min(WAIT_SLICE);
        let _ = queue.changed.wait_timeout_while(guard, slice, |changes| *changes == seen);
    }
}

/// How long a `wait` waits: `timeout_ms`, held to an hour; 30 minutes when none is given.
pub fn wait_for(timeout_ms: Option<u64>) -> Duration {
    Duration::from_millis(timeout_ms.unwrap_or(30 * 60 * 1000).clamp(1_000, relay_bus::ops::video::MAX_WAIT_MS))
}

/// The job name the worker knows render `id` by.
fn job(id: Id) -> String {
    format!("render-{id}")
}

/// A render's name as asked for, made safe: letters, digits, dots, dashes and underscores, not
/// starting with a dot, at most 80; `None` for one that keeps nothing.
pub fn safe_name(raw: &str) -> Option<String> {
    let name: String = raw.trim().chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let name = name.trim_start_matches(['.', '-']).trim_end_matches(['.', '-']).chars().take(80).collect::<String>();
    (!name.is_empty()).then_some(name)
}

/// Where render `id` of `video` is written: `out/<name>.<ext>`, or `out/<composition>-<id>.<ext>`.
pub fn output(video: &Video, composition: &str, id: Id, format: Format, name: Option<&str>) -> String {
    let stem = match name.and_then(safe_name) {
        Some(name) => name.strip_suffix(&format!(".{}", format.ext())).map(str::to_string).unwrap_or(name),
        None => {
            let safe: String = composition.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
            format!("{safe}-{id}")
        }
    };
    std::path::Path::new(&video.dir).join("out").join(format!("{stem}.{}", format.ext())).display().to_string()
}

/// The partial file a render draws to before it is renamed to `path`: hidden, beside it, with the
/// same extension (Remotion reads the codec's container from it).
fn partial(path: &str, id: Id) -> String {
    let path = std::path::Path::new(path);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{id}.partial.{name}")).display().to_string()
}

/// The events a render's new row is announced with: always `video.render.changed`, and
/// `video.render.finished` once it will not change again.
pub fn events(render: &VideoRender) -> Vec<(String, Value)> {
    let row = serde_json::to_value(render).unwrap_or_default();
    let mut out = vec![("video.render.changed".to_string(), row.clone())];
    if render.state.is_final() {
        out.push(("video.render.finished".to_string(), row));
    }
    out
}

fn advance(engine: &Engine, id: Id, project_id: Id, f: impl FnOnce(&Transaction, &str) -> Result<(), BusError>) -> Result<VideoRender, BusError> {
    let render = engine.system_write("video.render.advance", None, Some(project_id), None, json!({"render_id": id}), |tx, now| {
        f(tx, now)?;
        let render = get_render(tx, id)?;
        Ok((render.clone(), events(&render)))
    });
    notify(engine);
    render
}

/// Start the queue's thread unless it runs; it takes every queued render, oldest first.
pub fn enqueue(engine: &Arc<Engine>) {
    let mut running = engine.video.jobs.running.lock().unwrap_or_else(|e| e.into_inner());
    if *running {
        return;
    }
    let weak = Arc::downgrade(engine);
    if std::thread::Builder::new().name("video-render".into()).spawn(move || run(weak)).is_ok() {
        *running = true;
    }
}

fn run(weak: Weak<Engine>) {
    loop {
        let Some(engine) = weak.upgrade() else { return };
        // Taken and released with the flag held, so a render queued as this one finds nothing
        // still finds the thread gone and starts another.
        let next = {
            let mut running = engine.video.jobs.running.lock().unwrap_or_else(|e| e.into_inner());
            let next: Option<(Id, Id)> = engine.read_store("video.render.next", |conn| {
                conn.prepare_cached("SELECT id, project_id FROM video_renders WHERE state = 'queued' ORDER BY id LIMIT 1").bus()?
                    .query_row([], |r| Ok((r.get(0)?, r.get(1)?))).optional().bus()
            }).unwrap_or(None);
            if next.is_none() || engine.is_quitting() {
                *running = false;
                return;
            }
            next
        };
        if let Some((id, project_id)) = next {
            render_one(&engine, id, project_id);
        }
    }
}

fn render_one(engine: &Arc<Engine>, id: Id, project_id: Id) {
    let started = advance(engine, id, project_id, |tx, now| {
        tx.prepare_cached("UPDATE video_renders SET state = 'rendering', started_at = ?2 WHERE id = ?1 AND state = 'queued'").bus()?
            .execute(params![id, now]).bus()?;
        Ok(())
    });
    let render = match started {
        Ok(render) if render.state == RenderState::Rendering => render,
        Ok(_) => return,
        Err(e) => {
            // Its row cannot be written (a full disk, an I/O error): the queue would take it
            // again at once, forever. A pause, then once more; then it is failed if it can be.
            tracing::warn!(render = id, error = %e.message, "video render: could not start it");
            std::thread::sleep(Duration::from_secs(2));
            if engine.is_quitting() {
                return;
            }
            let _ = advance(engine, id, project_id, |tx, now| {
                tx.prepare_cached("UPDATE video_renders SET state = 'failed', error = ?2, finished_at = ?3 WHERE id = ?1 AND state = 'queued'").bus()?
                    .execute(params![id, format!("could not be started: {}", e.message), now]).bus()?;
                Ok(())
            });
            return;
        }
    };
    let outcome = render_with_worker(engine, &render);
    lock(&engine.video.jobs.stopping).remove(&id);
    let _ = advance(engine, id, project_id, |tx, now| {
        match &outcome {
            Ok((path, bytes, audio)) => {
                // A render of the same name before it no longer has its file: this one does.
                tx.prepare_cached("UPDATE video_renders SET replaced_by = ?1 WHERE video_id = ?2 AND path = ?3 AND id <> ?1 AND state = 'done'").bus()?
                    .execute(params![id, render.video_id, path]).bus()?;
                tx.prepare_cached("UPDATE video_renders SET state = 'done', progress = 1, path = ?2, bytes = ?3, audio = ?4, finished_at = ?5 WHERE id = ?1").bus()?
                    .execute(params![id, path, bytes, audio.as_ref().and_then(|a| serde_json::to_string(a).ok()), now]).bus()?
            }
            Err(e) => {
                let state = if e.code == "video.render.cancelled" { "cancelled" } else { "failed" };
                tx.prepare_cached("UPDATE video_renders SET state = ?2, error = ?3, finished_at = ?4 WHERE id = ?1").bus()?
                    .execute(params![id, state, e.message, now]).bus()?
            }
        };
        Ok(())
    });
}

fn render_with_worker(engine: &Arc<Engine>, render: &VideoRender) -> Result<(String, i64, Option<AudioReport>), BusError> {
    let (video, props): (Video, String) = engine.read_store("video.render.read", |conn| {
        let props: String = conn.prepare_cached("SELECT props FROM video_renders WHERE id = ?1").bus()?.query_row([render.id], |r| r.get(0)).bus()?;
        Ok((get_video(conn, render.video_id)?, props))
    })?;
    let path = output(&video, &render.composition, render.id, render.format, render.name.as_deref());
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(&video.dir)))
        .map_err(|e| BusError::internal(format!("out/: {e}")))?;
    let drawn = partial(&path, render.id);
    let (scale, crf) = match render.quality {
        Quality::Preview => (0.5, Some(28)),
        Quality::Standard => (1.0, None),
        Quality::High => (1.0, Some(14)),
    };
    // Progress lands on the row a tenth at a time: the person's bar moves, the bus stays quiet.
    let step = Arc::new(AtomicU32::new(0));
    let (weak, id, project_id) = (Arc::downgrade(engine), render.id, render.project_id);
    worker::watch(engine, &job(render.id), move |event| {
        let progress = event["progress"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
        let tenth = (progress * 10.0).floor() as u32;
        if tenth == 0 || tenth >= 10 || step.fetch_max(tenth, Ordering::SeqCst) >= tenth {
            return;
        }
        if let Some(engine) = weak.upgrade() {
            let _ = advance(&engine, id, project_id, |tx, _| {
                tx.prepare_cached("UPDATE video_renders SET progress = ?2 WHERE id = ?1 AND state = 'rendering'").bus()?
                    .execute(params![id, tenth as f64 / 10.0]).bus()?;
                Ok(())
            });
        }
    })?;
    let range = render.range.map(|[a, b]| json!([a, b]));
    let result = worker::call(engine, "render", json!({
        "job": job(render.id), "dir": video.dir, "bundle_dir": bundle_dir(engine, video.id),
        "composition": render.composition, "format": render.format.name(), "out": drawn, "scale": scale,
        "crf": crf, "props": serde_json::from_str::<Value>(&props).unwrap_or(json!({})), "range": range,
    }), RENDER_WITHIN);
    worker::unwatch(engine, &job(render.id));
    let stopped = || if lock(&engine.video.jobs.stopping).contains(&render.id) { Err(BusError::new(relay_bus::ErrorKind::Conflict, "video.render.cancelled", "the render was stopped")) } else { Ok(()) };
    let finished = result.and_then(|_| stopped())
        .and_then(|_| super::audio::finish(engine, render, std::path::Path::new(&drawn)))
        .and_then(|audio| stopped().map(|_| audio));
    let audio = match finished {
        Ok(audio) => audio,
        Err(e) => {
            let _ = std::fs::remove_file(&drawn);
            return Err(e);
        }
    };
    std::fs::rename(&drawn, &path).map_err(|e| BusError::internal(format!("{path}: {e}")))?;
    let bytes = std::fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0);
    Ok((path, bytes, audio))
}

/// Stop render `id`: a queued one at once; a running one through the worker, its thread then
/// marking it stopped.
pub fn cancel(engine: &Engine, render: &VideoRender) {
    if render.state == RenderState::Rendering {
        lock(&engine.video.jobs.stopping).insert(render.id);
        let _ = worker::call(engine, "cancel", json!({"job": job(render.id)}), Duration::from_secs(10));
    }
}

/// Renders still running when Relay last stopped did not finish: they are failed, so a row never
/// says rendering with nothing behind it, and the partial files they were drawing are removed.
/// Those still queued never started: the queue takes them now.
pub fn reconcile(engine: &Arc<Engine>) {
    let rows: Vec<(Id, Id, String)> = engine.read_store("video.render.reconcile", |conn| {
        conn.prepare_cached("SELECT r.id, r.project_id, v.dir FROM video_renders r JOIN videos v ON v.id = r.video_id WHERE r.state = 'rendering'").bus()?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()
    }).unwrap_or_default();
    for (id, project_id, dir) in rows {
        let _ = advance(engine, id, project_id, |tx, now| {
            tx.prepare_cached("UPDATE video_renders SET state = 'failed', error = 'Relay stopped before it finished', finished_at = ?2
                WHERE id = ?1 AND state = 'rendering'").bus()?.execute(params![id, now]).bus()?;
            Ok(())
        });
        let out = std::path::Path::new(&dir).join("out");
        let prefix = format!(".{id}.partial.");
        for entry in std::fs::read_dir(&out).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let queued = engine.read_store("video.render.reconcile", |conn| {
        conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM video_renders WHERE state = 'queued')").bus()?.query_row([], |r| r.get::<_, bool>(0)).bus()
    }).unwrap_or(false);
    if queued {
        enqueue(engine);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_is_written_to_out_by_composition_and_id() {
        let video = Video {
            id: 3, project_id: 1, session: None, title: "T".into(), dir: "/v/p1/3-t".into(), composition: "main".into(),
            width: 1080, height: 1920, fps: 30.0, duration_frames: 300, version: 1, poster: None, last_render: None,
            created_by: "user".into(), updated_by: "user".into(), created_at: String::new(), updated_at: String::new(), deleted_at: None,
        };
        assert_eq!(output(&video, "main", 12, Format::Mp4, None), "/v/p1/3-t/out/main-12.mp4");
        assert_eq!(output(&video, "wide/../x", 13, Format::Prores, None), "/v/p1/3-t/out/wide----x-13.mov");
        // A name asked for is the file's, made safe; its own extension is not doubled.
        assert_eq!(output(&video, "main", 14, Format::Mp4, Some("avex-1.0 voiceover")), "/v/p1/3-t/out/avex-1.0-voiceover.mp4");
        assert_eq!(output(&video, "main", 15, Format::Wav, Some("mix.wav")), "/v/p1/3-t/out/mix.wav");
        assert_eq!(output(&video, "main", 16, Format::Mp4, Some("../../etc")), "/v/p1/3-t/out/etc.mp4");
        assert_eq!(output(&video, "main", 17, Format::Mp4, Some("...")), "/v/p1/3-t/out/main-17.mp4");
        assert_eq!(partial("/v/p1/3-t/out/mix.wav", 15), "/v/p1/3-t/out/.15.partial.mix.wav");
    }
}
