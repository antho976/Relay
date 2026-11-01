//! `clips.db` beside the store: the Clips space's own SQLite file, as Aide's `aide.db` is. A VOD
//! row says where its work stands; a clip row holds its edit (JSON, as the bus has it), its file
//! and where it was posted. The engine keeps the file behind one mutex of its own, never taken
//! while the store's lock is held.

use relay_bus::ops::clips::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

pub type Result<T> = rusqlite::Result<T>;

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE vod (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    source TEXT NOT NULL,
    streamer TEXT NOT NULL,
    duration_s REAL NOT NULL DEFAULT 0,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    fps REAL NOT NULL DEFAULT 0,
    request TEXT NOT NULL,
    kinds TEXT NOT NULL,
    state TEXT NOT NULL,
    stage TEXT,
    progress REAL NOT NULL DEFAULT 0,
    detail TEXT,
    error TEXT,
    layout TEXT,
    video_id INTEGER,
    dir TEXT NOT NULL,
    poster TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX vod_state ON vod(state);
CREATE TABLE clip (
    id INTEGER PRIMARY KEY,
    vod_id INTEGER NOT NULL REFERENCES vod(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    tags TEXT NOT NULL,
    score INTEGER NOT NULL,
    why TEXT NOT NULL,
    start_s REAL NOT NULL,
    end_s REAL NOT NULL,
    length_s REAL NOT NULL,
    edit TEXT NOT NULL,
    state TEXT NOT NULL,
    error TEXT,
    section TEXT,
    file TEXT,
    poster TEXT,
    render_id INTEGER,
    publish_at TEXT,
    privacy TEXT,
    youtube_id TEXT,
    posted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX clip_vod ON clip(vod_id, score DESC);
CREATE INDEX clip_state ON clip(state);
CREATE INDEX clip_render ON clip(render_id);
"#];

/// The footage a clip is drawn from: a part of the VOD cut out with handles, in the video's
/// `public/`, and where in the VOD it starts.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Section {
    /// Its `staticFile()` name: `assets/clip-<id>.mp4`.
    pub file: String,
    pub start_s: f64,
    pub end_s: f64,
}

pub struct ClipsDb {
    conn: Connection,
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn parse<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Option<T> {
    raw.and_then(|r| serde_json::from_str(&r).ok())
}

fn bad(e: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(e.to_string().into())
}

/// A VOD row and what it needs to be one: the counts are filled by the caller's query.
const VOD_COLS: &str = "v.id, v.project_id, v.title, v.source, v.streamer, v.duration_s, v.width, v.height, v.fps, v.request, v.kinds,
    v.state, v.stage, v.progress, v.detail, v.error, v.layout, v.video_id, v.dir, v.poster, v.created_at, v.updated_at,
    (SELECT COUNT(*) FROM clip c WHERE c.vod_id = v.id AND c.state != 'rejected'),
    (SELECT COUNT(*) FROM clip c WHERE c.vod_id = v.id AND c.state = 'ready'),
    (SELECT COUNT(*) FROM clip c WHERE c.vod_id = v.id AND c.state IN ('approved', 'scheduled', 'posting')),
    (SELECT COUNT(*) FROM clip c WHERE c.vod_id = v.id AND c.state = 'posted')";

fn vod_row(r: &Row) -> Result<Vod> {
    Ok(Vod {
        id: r.get(0)?,
        project_id: r.get(1)?,
        title: r.get(2)?,
        source: r.get(3)?,
        streamer: r.get(4)?,
        duration_s: r.get(5)?,
        width: r.get(6)?,
        height: r.get(7)?,
        fps: r.get(8)?,
        request: r.get(9)?,
        kinds: parse(r.get(10)?).unwrap_or_default(),
        state: VodState::parse(&r.get::<_, String>(11)?),
        stage: r.get::<_, Option<String>>(12)?.as_deref().and_then(Stage::parse),
        progress: r.get(13)?,
        detail: r.get(14)?,
        error: r.get(15)?,
        layout: parse(r.get(16)?),
        video_id: r.get(17)?,
        dir: r.get(18)?,
        poster: r.get(19)?,
        created_at: r.get(20)?,
        updated_at: r.get(21)?,
        clips: r.get(22)?,
        clips_ready: r.get(23)?,
        clips_approved: r.get(24)?,
        clips_posted: r.get(25)?,
    })
}

const CLIP_COLS: &str = "id, vod_id, kind, title, description, tags, score, why, start_s, end_s, length_s, edit, state, error, file, poster,
    render_id, publish_at, privacy, youtube_id, posted_at, created_at, updated_at";

fn clip_row(r: &Row) -> Result<Clip> {
    let edit: ClipEdit = serde_json::from_str(&r.get::<_, String>(11)?).map_err(bad)?;
    let youtube_id: Option<String> = r.get(19)?;
    Ok(Clip {
        id: r.get(0)?,
        vod_id: r.get(1)?,
        kind: Kind::parse(&r.get::<_, String>(2)?).unwrap_or(Kind::Other),
        title: r.get(3)?,
        description: r.get(4)?,
        tags: parse(r.get(5)?).unwrap_or_default(),
        score: r.get(6)?,
        why: r.get(7)?,
        start_s: r.get(8)?,
        end_s: r.get(9)?,
        length_s: r.get(10)?,
        edit,
        state: ClipState::parse(&r.get::<_, String>(12)?),
        error: r.get(13)?,
        file: r.get(14)?,
        poster: r.get(15)?,
        render_id: r.get(16)?,
        publish_at: r.get(17)?,
        privacy: r.get::<_, Option<String>>(18)?.as_deref().and_then(Privacy::parse),
        youtube_url: youtube_id.as_ref().map(|id| format!("https://youtube.com/shorts/{id}")),
        youtube_id,
        posted_at: r.get(20)?,
        created_at: r.get(21)?,
        updated_at: r.get(22)?,
    })
}

/// What a new clip starts with.
pub struct NewClip {
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub score: u32,
    pub why: String,
    pub start_s: f64,
    pub end_s: f64,
    pub length_s: f64,
    pub edit: ClipEdit,
}

impl ClipsDb {
    pub fn open(path: &Path) -> Result<ClipsDb> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<ClipsDb> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<ClipsDb> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))?;
        }
        Ok(ClipsDb { conn })
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        self.conn.prepare_cached("SELECT value FROM meta WHERE key = ?1")?.query_row([key], |r| r.get(0)).optional()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.prepare_cached("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?
            .execute(params![key, value])?;
        Ok(())
    }

    pub fn delete_meta(&self, key: &str) -> Result<()> {
        self.conn.prepare_cached("DELETE FROM meta WHERE key = ?1")?.execute([key])?;
        Ok(())
    }

    // ---- VODs ----

    #[allow(clippy::too_many_arguments)]
    pub fn add_vod(&self, project_id: i64, title: &str, source: &str, streamer: &str, request: &str, kinds: &[Kind], dir_of: impl FnOnce(i64) -> String, now: &str) -> Result<Vod> {
        self.conn.prepare_cached(
            "INSERT INTO vod(project_id, title, source, streamer, request, kinds, state, dir, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', '', ?7, ?7)",
        )?.execute(params![project_id, title, source, streamer, request, json(&kinds), now])?;
        let id = self.conn.last_insert_rowid();
        self.conn.prepare_cached("UPDATE vod SET dir = ?2 WHERE id = ?1")?.execute(params![id, dir_of(id)])?;
        self.vod(id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn vod(&self, id: i64) -> Result<Option<Vod>> {
        self.conn.prepare_cached(&format!("SELECT {VOD_COLS} FROM vod v WHERE v.id = ?1"))?.query_row([id], vod_row).optional()
    }

    pub fn vods(&self) -> Result<Vec<Vod>> {
        self.conn.prepare_cached(&format!("SELECT {VOD_COLS} FROM vod v ORDER BY v.id DESC"))?.query_map([], vod_row)?.collect()
    }

    /// The oldest VOD waiting its turn.
    pub fn next_queued(&self) -> Result<Option<Vod>> {
        self.conn.prepare_cached(&format!("SELECT {VOD_COLS} FROM vod v WHERE v.state IN ('queued', 'working') ORDER BY v.state = 'working' DESC, v.id LIMIT 1"))?
            .query_row([], vod_row).optional()
    }

    pub fn set_vod_state(&self, id: i64, state: VodState, error: Option<&str>, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET state = ?2, error = ?3, updated_at = ?4 WHERE id = ?1")?
            .execute(params![id, state.name(), error, now])?;
        Ok(())
    }

    pub fn set_stage(&self, id: i64, stage: Option<Stage>, progress: f64, detail: Option<&str>, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET stage = ?2, progress = ?3, detail = ?4, updated_at = ?5 WHERE id = ?1")?
            .execute(params![id, stage.map(Stage::name), progress.clamp(0.0, 1.0), detail, now])?;
        Ok(())
    }

    pub fn set_progress(&self, id: i64, progress: f64, detail: Option<&str>) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET progress = ?2, detail = COALESCE(?3, detail) WHERE id = ?1")?
            .execute(params![id, progress.clamp(0.0, 1.0), detail])?;
        Ok(())
    }

    pub fn set_vod_media(&self, id: i64, duration_s: f64, width: u32, height: u32, fps: f64, title: Option<&str>) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET duration_s = ?2, width = ?3, height = ?4, fps = ?5, title = COALESCE(?6, title) WHERE id = ?1")?
            .execute(params![id, duration_s, width, height, fps, title])?;
        Ok(())
    }

    pub fn set_vod_layout(&self, id: i64, layout: &SourceLayout, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET layout = ?2, updated_at = ?3 WHERE id = ?1")?.execute(params![id, json(layout), now])?;
        Ok(())
    }

    pub fn set_vod_video(&self, id: i64, video_id: i64) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET video_id = ?2 WHERE id = ?1")?.execute(params![id, video_id])?;
        Ok(())
    }

    pub fn set_vod_poster(&self, id: i64, poster: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET poster = ?2 WHERE id = ?1")?.execute(params![id, poster])?;
        Ok(())
    }

    pub fn set_vod_request(&self, id: i64, request: &str, kinds: &[Kind], now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE vod SET request = ?2, kinds = ?3, updated_at = ?4 WHERE id = ?1")?
            .execute(params![id, request, json(&kinds), now])?;
        Ok(())
    }

    pub fn delete_vod(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("DELETE FROM vod WHERE id = ?1")?.execute([id])? > 0)
    }

    // ---- Clips ----

    pub fn add_clip(&self, vod_id: i64, c: &NewClip, now: &str) -> Result<Clip> {
        self.conn.prepare_cached(
            "INSERT INTO clip(vod_id, kind, title, description, tags, score, why, start_s, end_s, length_s, edit, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'draft', ?12, ?12)",
        )?.execute(params![vod_id, c.kind.name(), c.title, c.description, json(&c.tags), c.score, c.why, c.start_s, c.end_s, c.length_s, json(&c.edit), now])?;
        self.clip(self.conn.last_insert_rowid())?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn clip(&self, id: i64) -> Result<Option<Clip>> {
        self.conn.prepare_cached(&format!("SELECT {CLIP_COLS} FROM clip WHERE id = ?1"))?.query_row([id], clip_row).optional()
    }

    pub fn clips_of(&self, vod_id: i64) -> Result<Vec<Clip>> {
        self.conn.prepare_cached(&format!("SELECT {CLIP_COLS} FROM clip WHERE vod_id = ?1 ORDER BY score DESC, start_s"))?
            .query_map([vod_id], clip_row)?.collect()
    }

    /// Every clip: the newest VOD's first, best first within each.
    pub fn clips(&self) -> Result<Vec<Clip>> {
        self.conn.prepare_cached(&format!("SELECT {CLIP_COLS} FROM clip ORDER BY vod_id DESC, score DESC, start_s"))?
            .query_map([], clip_row)?.collect()
    }

    pub fn clips_in(&self, states: &[ClipState]) -> Result<Vec<Clip>> {
        Ok(self.clips()?.into_iter().filter(|c| states.contains(&c.state)).collect())
    }

    pub fn clip_by_render(&self, render_id: i64) -> Result<Option<Clip>> {
        self.conn.prepare_cached(&format!("SELECT {CLIP_COLS} FROM clip WHERE render_id = ?1"))?.query_row([render_id], clip_row).optional()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_clip(&self, id: i64, title: &str, description: &str, tags: &[String], kind: Kind, edit: &ClipEdit, start_s: f64, end_s: f64, length_s: f64, now: &str) -> Result<()> {
        self.conn.prepare_cached(
            "UPDATE clip SET title = ?2, description = ?3, tags = ?4, kind = ?5, edit = ?6, start_s = ?7, end_s = ?8, length_s = ?9, updated_at = ?10 WHERE id = ?1",
        )?.execute(params![id, title, description, json(&tags), kind.name(), json(edit), start_s, end_s, length_s, now])?;
        Ok(())
    }

    pub fn set_clip_state(&self, id: i64, state: ClipState, error: Option<&str>, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET state = ?2, error = ?3, updated_at = ?4 WHERE id = ?1")?
            .execute(params![id, state.name(), error, now])?;
        Ok(())
    }

    pub fn set_clip_render(&self, id: i64, render_id: Option<i64>, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET render_id = ?2, state = 'rendering', error = NULL, updated_at = ?3 WHERE id = ?1")?
            .execute(params![id, render_id, now])?;
        Ok(())
    }

    pub fn set_clip_file(&self, id: i64, file: &str, poster: Option<&str>, length_s: Option<f64>, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET file = ?2, poster = COALESCE(?3, poster), length_s = COALESCE(?4, length_s), updated_at = ?5 WHERE id = ?1")?
            .execute(params![id, file, poster, length_s, now])?;
        Ok(())
    }

    pub fn section(&self, id: i64) -> Result<Option<Section>> {
        let raw: Option<String> = self.conn.prepare_cached("SELECT section FROM clip WHERE id = ?1")?.query_row([id], |r| r.get(0)).optional()?.flatten();
        Ok(parse(raw))
    }

    pub fn set_section(&self, id: i64, section: &Section) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET section = ?2 WHERE id = ?1")?.execute(params![id, json(section)])?;
        Ok(())
    }

    pub fn schedule(&self, id: i64, at: Option<&str>, privacy: Option<Privacy>, state: ClipState, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET publish_at = ?2, privacy = ?3, state = ?4, error = NULL, updated_at = ?5 WHERE id = ?1")?
            .execute(params![id, at, privacy.map(Privacy::name), state.name(), now])?;
        Ok(())
    }

    pub fn set_posted(&self, id: i64, youtube_id: &str, now: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE clip SET youtube_id = ?2, posted_at = ?3, state = 'posted', error = NULL, updated_at = ?3 WHERE id = ?1")?
            .execute(params![id, youtube_id, now])?;
        Ok(())
    }

    pub fn delete_clip(&self, id: i64) -> Result<bool> {
        Ok(self.conn.prepare_cached("DELETE FROM clip WHERE id = ?1")?.execute([id])? > 0)
    }

    /// Clips left `rendering` or `posting` by an engine that stopped: drawn again or put back.
    pub fn interrupted(&self) -> Result<Vec<Clip>> {
        self.clips_in(&[ClipState::Rendering, ClipState::Posting])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn edit() -> ClipEdit {
        ClipEdit {
            keep: vec![[10.0, 30.0]], tighten_ms: Some(650), layout: Layout::Split, view: None, reframe: BTreeMap::new(),
            hook: "He did it".into(), captions: CaptionStyle::Pixel, caption_y: None, emphasis: vec![], fixes: BTreeMap::new(),
            punch_at: vec![], end_card: true, scene: None,
        }
    }

    #[test]
    fn vods_and_clips_round_trip() {
        let db = ClipsDb::open_in_memory().unwrap();
        let vod = db.add_vod(1, "A stream", "/v.mp4", "Neph", "funny", &[Kind::Funny], |id| format!("/vods/{id}"), "t0").unwrap();
        assert_eq!(vod.dir, format!("/vods/{}", vod.id));
        assert_eq!(vod.kinds, vec![Kind::Funny]);
        assert_eq!(vod.state, VodState::Queued);
        let clip = db.add_clip(vod.id, &NewClip {
            kind: Kind::Fail, title: "t".into(), description: "d".into(), tags: vec!["a".into()], score: 7, why: "w".into(),
            start_s: 10.0, end_s: 30.0, length_s: 20.0, edit: edit(),
        }, "t1").unwrap();
        assert_eq!(clip.state, ClipState::Draft);
        assert_eq!(clip.edit, edit());
        db.set_clip_state(clip.id, ClipState::Ready, None, "t2").unwrap();
        let vod = db.vod(vod.id).unwrap().unwrap();
        assert_eq!((vod.clips, vod.clips_ready), (1, 1));
        db.set_posted(clip.id, "abc", "t3").unwrap();
        let clip = db.clip(clip.id).unwrap().unwrap();
        assert_eq!(clip.youtube_url.as_deref(), Some("https://youtube.com/shorts/abc"));
        assert!(db.delete_vod(vod.id).unwrap());
        assert!(db.clip(clip.id).unwrap().is_none(), "a VOD's clips go with it");
    }
}
