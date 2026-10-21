//! Renders (`video.render`): queued rows run one at a time, oldest first, on a thread of their
//! own, each through the worker. A render's row says where it is; `video.render.changed` carries
//! it at each change of state and each tenth of the way, and `video.render.finished` once it is
//! done, failed or stopped — what an agent waits on.

use super::{bundle_dir, get_render, get_video, worker, Video};
use crate::engine::{Engine, IntoBus};
use relay_bus::error::BusError;
use relay_bus::ops::video::{Format, Quality, RenderState, VideoRender};
use relay_bus::types::Id;
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// The longest a render may run.
const RENDER_WITHIN: Duration = Duration::from_secs(6 * 60 * 60);

/// Whether a thread is running the queue.
#[derive(Default)]
pub struct Queue {
    running: Mutex<bool>,
}

/// The job name the worker knows render `id` by.
fn job(id: Id) -> String {
    format!("render-{id}")
}

/// Where render `id` of `video` is written: `out/<composition>-<id>.<ext>`.
pub fn output(video: &Video, composition: &str, id: Id, format: Format) -> String {
    let safe: String = composition.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    std::path::Path::new(&video.dir).join("out").join(format!("{safe}-{id}.{}", format.ext())).display().to_string()
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
    engine.system_write("video.render.advance", None, Some(project_id), None, json!({"render_id": id}), |tx, now| {
        f(tx, now)?;
        let render = get_render(tx, id)?;
        Ok((render.clone(), events(&render)))
    })
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
        _ => return,
    };
    let outcome = render_with_worker(engine, &render);
    let _ = advance(engine, id, project_id, |tx, now| {
        match &outcome {
            Ok((path, bytes)) => tx.prepare_cached("UPDATE video_renders SET state = 'done', progress = 1, path = ?2, bytes = ?3, finished_at = ?4 WHERE id = ?1").bus()?
                .execute(params![id, path, bytes, now]).bus()?,
            Err(e) => {
                let state = if e.code == "video.render.cancelled" { "cancelled" } else { "failed" };
                tx.prepare_cached("UPDATE video_renders SET state = ?2, error = ?3, finished_at = ?4 WHERE id = ?1").bus()?
                    .execute(params![id, state, e.message, now]).bus()?
            }
        };
        Ok(())
    });
}

fn render_with_worker(engine: &Arc<Engine>, render: &VideoRender) -> Result<(String, i64), BusError> {
    let (video, props): (Video, String) = engine.read_store("video.render.read", |conn| {
        let props: String = conn.prepare_cached("SELECT props FROM video_renders WHERE id = ?1").bus()?.query_row([render.id], |r| r.get(0)).bus()?;
        Ok((get_video(conn, render.video_id)?, props))
    })?;
    let path = output(&video, &render.composition, render.id, render.format);
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(&video.dir)))
        .map_err(|e| BusError::internal(format!("out/: {e}")))?;
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
        "composition": render.composition, "format": render.format.name(), "out": path, "scale": scale,
        "crf": crf, "props": serde_json::from_str::<Value>(&props).unwrap_or(json!({})), "range": range,
    }), RENDER_WITHIN);
    worker::unwatch(engine, &job(render.id));
    let done = result?;
    Ok((done["path"].as_str().unwrap_or(&path).to_string(), done["bytes"].as_i64().unwrap_or(0)))
}

/// Stop render `id`: a queued one at once; a running one through the worker, its thread then
/// marking it stopped.
pub fn cancel(engine: &Engine, render: &VideoRender) {
    if render.state == RenderState::Rendering {
        let _ = worker::call(engine, "cancel", json!({"job": job(render.id)}), Duration::from_secs(10));
    }
}

/// Renders still queued or running when Relay last stopped did not finish: they are failed, so a
/// row never says rendering with nothing behind it.
pub fn reconcile(engine: &Engine) {
    let ids: Vec<(Id, Id)> = engine.read_store("video.render.reconcile", |conn| {
        conn.prepare_cached("SELECT id, project_id FROM video_renders WHERE state IN ('queued', 'rendering')").bus()?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()
    }).unwrap_or_default();
    for (id, project_id) in ids {
        let _ = advance(engine, id, project_id, |tx, now| {
            tx.prepare_cached("UPDATE video_renders SET state = 'failed', error = 'Relay stopped before it finished', finished_at = ?2
                WHERE id = ?1 AND state IN ('queued', 'rendering')").bus()?.execute(params![id, now]).bus()?;
            Ok(())
        });
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
        assert_eq!(output(&video, "main", 12, Format::Mp4), "/v/p1/3-t/out/main-12.mp4");
        assert_eq!(output(&video, "wide/../x", 13, Format::Prores), "/v/p1/3-t/out/wide----x-13.mov");
    }
}
