//! Videos written as code (docs/VIDEO.md, D167). A video is a folder of React components that
//! Remotion renders — `videos/p<project>/<id>-<slug>/` beside the store — which agents edit with
//! their own file tools, and a `videos` row with what Relay knows of it. The ops (`handlers::video`)
//! make one, check it, look at frames of it and render it through the Node worker (`worker`),
//! once the kit (`kit`) is set up. Each read of the folder records a version of its code when it
//! changed (`versions`); stills come back as pictures with the problems found in them (`probe`,
//! `sheet`), and renders run one at a time on a thread of their own (`jobs`).

pub mod assets;
pub mod audio;
pub mod beats;
pub mod capture;
pub mod eleven;
pub mod inspect;
pub mod jobs;
pub mod kit;
pub mod media;
pub mod preview;
pub mod probe;
pub mod sheet;
pub mod stills;
pub mod timeline;
pub mod versions;
pub mod worker;

use crate::engine::{Engine, IntoBus};
use relay_bus::error::BusError;
use relay_bus::ops::video::{Format, Quality, RenderInfo, RenderState, Video, VideoRender};
use relay_bus::types::Id;
use rusqlite::{Connection, OptionalExtension, Row};
use std::path::PathBuf;
use std::sync::Mutex;

/// The plugin the videos come with.
pub const PLUGIN: &str = relay_bus::ops::video::PLUGIN;

/// What the engine keeps for videos between requests: the worker, the kit's setup and the render
/// queue. Its own locks; never the store's.
#[derive(Default)]
pub struct Hub {
    pub(crate) worker: worker::Slot,
    pub(crate) kit: kit::State,
    pub(crate) jobs: jobs::Queue,
    /// The newest timelines measured (`video.timeline`, a still's sound), by video, version,
    /// composition and props: a still after a timeline reuses its mix.
    pub(crate) timelines: Mutex<Vec<(String, std::sync::Arc<stills::Measured>)>>,
    /// The live preview (Remotion Studio), one at a time.
    pub preview: preview::Slot,
    /// ElevenLabs: the saved key once read, and the tests' stand-in for the network.
    pub(crate) eleven: eleven::State,
    /// Files' lengths in seconds, by path and change time (ffprobe).
    pub(crate) lengths: Mutex<std::collections::HashMap<PathBuf, (std::time::SystemTime, Option<f64>)>>,
    /// Tests only: the worker's command line instead of `node videos/.relay/worker.mjs`, with
    /// the kit counted as set up.
    program: Mutex<Option<Vec<String>>>,
}

impl Hub {
    pub(crate) fn program(&self) -> Option<Vec<String>> {
        self.program.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Answer ElevenLabs calls with `fake` instead of the network: what the engine tests use.
    #[doc(hidden)]
    pub fn use_elevenlabs(&self, fake: eleven::Fake) {
        *self.eleven.fake.lock().unwrap_or_else(|e| e.into_inner()) = Some(fake);
    }

    /// Run `program` as the worker instead of Relay's, the kit counted as set up: what the engine
    /// tests drive a fake worker with.
    #[doc(hidden)]
    pub fn use_worker(&self, program: Vec<String>) {
        *self.program.lock().unwrap_or_else(|e| e.into_inner()) = Some(program);
    }
}

/// Stop the worker and what it runs: Relay is going.
pub fn shutdown(engine: &Engine) {
    worker::stop(engine);
    preview::stop(engine);
}

/// The folder a project's videos are made in: what every session of the project may write to
/// while the Video plugin is on (`guardrail::config`).
pub fn project_dir(videos: &std::path::Path, project_id: Id) -> PathBuf {
    videos.join(format!("p{project_id}"))
}

/// Where the worker keeps video `id`'s bundles.
pub fn bundle_dir(engine: &Engine, id: Id) -> PathBuf {
    engine.store.videos_dir().join(".relay/bundles").join(id.to_string())
}

/// Where versions keep file contents, by hash.
pub fn blobs_dir(engine: &Engine) -> PathBuf {
    engine.store.videos_dir().join(".relay/blobs")
}

/// Where a video's poster (its latest still) is kept.
pub fn poster_path(engine: &Engine, id: Id) -> PathBuf {
    engine.store.videos_dir().join(".relay/posters").join(format!("{id}.png"))
}

/// A folder name from a title: lower-case letters, digits and dashes, at most 40.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "video".into() } else { out }
}

const VIDEO_COLUMNS: &str = "v.id, v.project_id, v.session, v.title, v.dir, v.composition, v.width, v.height, v.fps, v.duration_frames, \
    v.version, v.poster, v.created_by, v.updated_by, v.created_at, v.updated_at, v.deleted_at, \
    r.id, r.state, r.format, r.path, r.quality, r.version, r.finished_at";

/// `SELECT` of videos with their newest render, for a `WHERE` that names the video as `v`.
pub fn select_videos(rest: &str) -> String {
    format!("SELECT {VIDEO_COLUMNS} FROM videos v LEFT JOIN video_renders r ON r.id = (SELECT MAX(id) FROM video_renders WHERE video_id = v.id) {rest}")
}

pub fn video_row(row: &Row) -> rusqlite::Result<Video> {
    let last_render = match row.get::<_, Option<Id>>(17)? {
        Some(id) => Some(RenderInfo {
            id,
            state: RenderState::from_name(&row.get::<_, String>(18)?).unwrap_or(RenderState::Failed),
            format: Format::from_name(&row.get::<_, String>(19)?).unwrap_or(Format::Mp4),
            path: row.get(20)?,
            preview: row.get::<_, String>(21)? == "preview",
            version: row.get(22)?,
            finished_at: row.get(23)?,
        }),
        None => None,
    };
    Ok(Video {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session: row.get(2)?,
        title: row.get(3)?,
        dir: row.get(4)?,
        composition: row.get(5)?,
        width: row.get(6)?,
        height: row.get(7)?,
        fps: row.get(8)?,
        duration_frames: row.get(9)?,
        version: row.get(10)?,
        poster: row.get(11)?,
        created_by: row.get(12)?,
        updated_by: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        deleted_at: row.get(16)?,
        last_render,
    })
}

/// Video `id`, deleted or not.
pub fn get_video(conn: &Connection, id: Id) -> Result<Video, BusError> {
    conn.prepare_cached(&select_videos("WHERE v.id = ?1")).bus()?
        .query_row([id], video_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("video.not_found", format!("no video {id}")))
}

/// Video `id` if it is not deleted: what every change to one needs.
pub fn live_video(conn: &Connection, id: Id) -> Result<Video, BusError> {
    let video = get_video(conn, id)?;
    if video.deleted_at.is_some() {
        return Err(BusError::conflict("video.deleted", format!("video {id} is deleted")).with_hint("video.restore brings it back"));
    }
    Ok(video)
}

pub const RENDER_COLUMNS: &str = "id, video_id, project_id, composition, format, quality, state, progress, version, range_from, range_to, \
    path, bytes, error, actor, created_at, started_at, finished_at, name, audio, loudness, peak";

pub fn render_row(row: &Row) -> rusqlite::Result<VideoRender> {
    let range = match (row.get::<_, Option<u32>>(9)?, row.get::<_, Option<u32>>(10)?) {
        (Some(a), Some(b)) => Some([a, b]),
        _ => None,
    };
    Ok(VideoRender {
        id: row.get(0)?,
        video_id: row.get(1)?,
        project_id: row.get(2)?,
        composition: row.get(3)?,
        format: Format::from_name(&row.get::<_, String>(4)?).unwrap_or(Format::Mp4),
        quality: match row.get::<_, String>(5)?.as_str() {
            "preview" => Quality::Preview,
            "high" => Quality::High,
            _ => Quality::Standard,
        },
        state: RenderState::from_name(&row.get::<_, String>(6)?).unwrap_or(RenderState::Failed),
        progress: row.get(7)?,
        version: row.get(8)?,
        range,
        path: row.get(11)?,
        bytes: row.get(12)?,
        error: row.get(13)?,
        actor: row.get(14)?,
        created_at: row.get(15)?,
        started_at: row.get(16)?,
        finished_at: row.get(17)?,
        name: row.get(18)?,
        audio: row.get::<_, Option<String>>(19)?.and_then(|text| serde_json::from_str(&text).ok()),
        loudness: row.get(20)?,
        peak: row.get(21)?,
    })
}

pub fn get_render(conn: &Connection, id: Id) -> Result<VideoRender, BusError> {
    conn.prepare_cached(&format!("SELECT {RENDER_COLUMNS} FROM video_renders WHERE id = ?1")).bus()?
        .query_row([id], render_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("video.render.not_found", format!("no render {id}")))
}

pub fn quality_name(q: Quality) -> &'static str {
    match q {
        Quality::Preview => "preview",
        Quality::Standard => "standard",
        Quality::High => "high",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_named_from_its_title() {
        assert_eq!(slug("Tally — launch promo!"), "tally-launch-promo");
        assert_eq!(slug("   "), "video");
        assert_eq!(slug("Été 2026"), "t-2026");
        assert!(slug(&"long ".repeat(30)).len() <= 40);
    }
}
