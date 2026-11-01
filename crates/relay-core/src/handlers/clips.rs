//! `clips.*` — the Clips space (docs/CLIPS.md). `clips.db` is its own file with its own mutex, so
//! reads are `register_unlocked`; changes are staged: the work on `clips.db` (and anything slow:
//! the keyring, a file's check) is done with the store's lock released, and the transaction only
//! audits it. Nothing a handler does waits on a VOD's stages or a render: those run on the
//! Clips threads (`clips::pipeline`), woken once the request is answered.

use crate::clips::{self, edit, pipeline, with_db, youtube};
use crate::engine::{Engine, Unlocked};
use relay_bus::ops::clips::*;
use relay_bus::{BusError, Empty};
use serde_json::json;
use std::path::Path;

fn now() -> String {
    crate::time::now()
}

fn emit(ctx: &mut Unlocked, what: &str, vod: Option<i64>, clip: Option<i64>) {
    ctx.emit("clips.changed", json!({"what": what, "vod_id": vod, "clip_id": clip}));
}

/// The project a VOD's video is made in: the one asked for, else the first one.
fn project(ctx: &mut Unlocked, asked: Option<i64>) -> Result<i64, BusError> {
    let ids: Vec<i64> = ctx.read(|conn| {
        let mut st = conn.prepare_cached("SELECT id FROM projects ORDER BY id").map_err(crate::engine::internal)?;
        let rows = st.query_map([], |r| r.get(0)).map_err(crate::engine::internal)?;
        rows.collect::<Result<Vec<i64>, _>>().map_err(crate::engine::internal)
    })?;
    match asked {
        Some(id) if ids.contains(&id) => Ok(id),
        Some(id) => Err(BusError::not_found("clips.project", format!("No project {id}"))),
        None => ids.first().copied().ok_or_else(|| BusError::unavailable("clips.project", "add a project first: a VOD's clips are a video of one")),
    }
}

fn kinds(k: Option<Vec<Kind>>) -> Vec<Kind> {
    let mut k = k.unwrap_or_default();
    k.sort();
    k.dedup();
    if k.len() == Kind::ALL.len() {
        k.clear();
    }
    k
}

/// Draw clip `id` again on a thread of its own: its footage may need cutting again, and the
/// render's queue takes the store's lock.
fn render_soon(ctx: &mut Unlocked, id: i64) {
    let engine = ctx.engine().arc();
    let _ = with_db(ctx.engine(), |db| db.set_clip_render(id, None, &now()));
    if let Some(engine) = engine {
        std::thread::Builder::new().name("clips-render".into()).spawn(move || {
            if let Err(e) = pipeline::queue_render(&engine, id) {
                let _ = with_db(&engine, |db| db.set_clip_state(id, ClipState::Failed, Some(&e.message), &now()));
            }
            if let Ok(c) = clips::clip(&engine, id) {
                clips::changed(&engine, "clip", Some(c.vod_id), Some(id));
            }
        }).ok();
    }
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<Summary>(|ctx, _: Empty| {
        let engine = ctx.engine();
        let vods = with_db(engine, |db| db.vods())?;
        let all = with_db(engine, |db| db.clips())?;
        let count = |s: ClipState| all.iter().filter(|c| c.state == s).count() as u32;
        let next_post = all.iter().filter(|c| c.state == ClipState::Scheduled).min_by(|a, b| a.publish_at.cmp(&b.publish_at)).cloned();
        let link = vods.iter().any(|v| clips::is_link(&v.source));
        Ok(SummaryOut {
            vods: vods.len() as u32,
            working: vods.iter().find(|v| v.state == VodState::Working).cloned(),
            queued: vods.iter().filter(|v| v.state == VodState::Queued).count() as u32,
            to_review: count(ClipState::Ready),
            approved: count(ClipState::Approved),
            scheduled: count(ClipState::Scheduled),
            posted: count(ClipState::Posted),
            next_post,
            youtube: youtube::status(engine),
            missing: clips::missing(engine, link),
        })
    });

    e.register_unlocked::<Tools>(|ctx, _: Empty| Ok(ToolsOut { tools: clips::tools(ctx.engine()) }));

    e.register_unlocked::<SettingsGet>(|ctx, _: Empty| clips::settings(ctx.engine()));
    e.register_staged::<SettingsSet, Settings>(|ctx, p| {
        let s = clips::checked(p.clone()).map_err(|e| BusError::invalid("clips.invalid", e.message))?;
        clips::save_settings(ctx.engine(), &s)?;
        emit(ctx, "settings", None, None);
        ctx.after_commit(|engine| clips::poke_poster(&engine));
        Ok(s)
    }, |_, _, s| Ok(s));

    e.register_staged::<Import, Vod>(|ctx, p| {
        let source = p.source.trim();
        if source.is_empty() {
            return Err(clips::invalid("give a video file or a link"));
        }
        let link = clips::is_link(source);
        let source = if link { source.to_string() } else { clips::file_path(source) };
        if !link {
            let path = Path::new(&source);
            if !path.is_absolute() {
                return Err(clips::invalid("give the file's full path"));
            }
            if !path.is_file() {
                return Err(BusError::not_found("clips.file", format!("{source} is not a file")));
            }
        }
        let missing: Vec<String> = clips::missing(ctx.engine(), link).into_iter().filter(|t| t.name != "Video kit").map(|t| t.name).collect();
        if !missing.is_empty() {
            return Err(BusError::unavailable("clips.tools", format!("Clips needs {} on this PC", missing.join(", ")))
                .with_hint("clips.tools says how to install each"));
        }
        let project_id = project(ctx, p.project_id)?;
        let s = clips::settings(ctx.engine())?;
        let title = p.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| clips::cut(t, 120)).unwrap_or_else(|| {
            if link {
                source.clone()
            } else {
                title_from_file(&source)
            }
        });
        let request = p.request.as_deref().unwrap_or("").trim().to_string();
        let kinds = kinds(p.kinds.clone());
        let vods = clips::vods_dir(ctx.engine(), project_id);
        let slug = crate::video::slug(&title);
        let t = now();
        let vod = with_db(ctx.engine(), |db| db.add_vod(project_id, &title, &source, &s.streamer, &request, &kinds,
            |id| vods.join(format!("{id}-{slug}")).display().to_string(), &t))?;
        pipeline::set_ask(ctx.engine(), vod.id, &pipeline::Ask { request, kinds, max: s.max_clips })?;
        emit(ctx, "vod", Some(vod.id), None);
        ctx.after_commit(|engine| clips::poke(&engine));
        Ok(vod)
    }, |_, _, s| Ok(s));

    e.register_unlocked::<VodList>(|ctx, _: Empty| Ok(VodsOut { vods: with_db(ctx.engine(), |db| db.vods())? }));
    e.register_unlocked::<VodGet>(|ctx, p| {
        let vod = clips::vod(ctx.engine(), p.id)?;
        Ok(VodOut { clips: with_db(ctx.engine(), |db| db.clips_of(p.id))?, vod })
    });

    e.register_staged::<Find, Vod>(|ctx, p| {
        let vod = clips::vod(ctx.engine(), p.id)?;
        if matches!(vod.state, VodState::Working | VodState::Queued) {
            return Err(BusError::conflict("clips.busy", "this VOD is still being worked on").with_hint("look again once it is ready"));
        }
        let request = p.request.trim();
        if request.chars().count() > 500 {
            return Err(clips::invalid("say what to look for in at most 500 characters"));
        }
        let s = clips::settings(ctx.engine())?;
        let kinds = kinds(p.kinds.clone());
        let max = p.max.unwrap_or(s.max_clips).clamp(1, MAX_FIND);
        pipeline::set_ask(ctx.engine(), vod.id, &pipeline::Ask { request: request.into(), kinds: kinds.clone(), max })?;
        let t = now();
        // From the find stage on: what came before it is kept.
        let from = if vod.stage.is_some_and(|s| s < Stage::Find) { vod.stage } else { Some(Stage::Find) };
        with_db(ctx.engine(), |db| {
            db.set_vod_request(vod.id, request, &kinds, &t)?;
            db.set_stage(vod.id, from, 0.0, None, &t)?;
            db.set_vod_state(vod.id, VodState::Queued, None, &t)
        })?;
        emit(ctx, "vod", Some(vod.id), None);
        ctx.after_commit(|engine| clips::poke(&engine));
        clips::vod(ctx.engine(), vod.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<VodStop, Vod>(|ctx, p| {
        pipeline::stop(ctx.engine(), p.id)?;
        emit(ctx, "vod", Some(p.id), None);
        clips::vod(ctx.engine(), p.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<VodRetry, Vod>(|ctx, p| {
        let vod = clips::vod(ctx.engine(), p.id)?;
        if !matches!(vod.state, VodState::Failed | VodState::Stopped) {
            return Err(BusError::conflict("clips.state", "only a failed or stopped VOD is started again"));
        }
        clips::clear_stop(ctx.engine(), p.id);
        with_db(ctx.engine(), |db| db.set_vod_state(p.id, VodState::Queued, None, &now()))?;
        emit(ctx, "vod", Some(p.id), None);
        ctx.after_commit(|engine| clips::poke(&engine));
        clips::vod(ctx.engine(), p.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<VodDelete, Empty>(|ctx, p| {
        let vod = clips::vod(ctx.engine(), p.id)?;
        if vod.state == VodState::Working {
            clips::ask_stop(ctx.engine(), p.id);
        }
        // Its working folder only: never the file it came from.
        let dir = Path::new(&vod.dir);
        let under = clips::vods_dir(ctx.engine(), vod.project_id);
        if dir.starts_with(&under) && dir != under && dir.is_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
        for c in with_db(ctx.engine(), |db| db.clips_of(p.id))? {
            if let Some(f) = c.file.as_deref() {
                let _ = std::fs::remove_file(f);
            }
        }
        with_db(ctx.engine(), |db| db.delete_vod(p.id))?;
        emit(ctx, "vod", Some(p.id), None);
        Ok(Empty {})
    }, |_, _, s| Ok(s));

    e.register_staged::<LayoutSet, Vod>(|ctx, p| {
        let vod = clips::vod(ctx.engine(), p.id)?;
        let mut l = p.layout.clone();
        if !["facecam", "camera", "screen"].contains(&l.kind.as_str()) {
            return Err(clips::invalid("a layout is facecam, camera or screen"));
        }
        if l.kind == "facecam" && l.cam.is_none() {
            return Err(clips::invalid("a facecam layout needs the cam's box"));
        }
        let (w, h) = (vod.width.max(16) as f64, vod.height.max(16) as f64);
        for r in [&mut l.cam, &mut l.game, &mut l.subject].into_iter().flatten() {
            r.w = r.w.clamp(16.0, w);
            r.h = r.h.clamp(16.0, h);
            r.x = r.x.clamp(0.0, w - r.w);
            r.y = r.y.clamp(0.0, h - r.h);
        }
        l.by = "person".into();
        if l.frame.is_none() {
            l.frame = vod.layout.as_ref().and_then(|o| o.frame.clone());
        }
        with_db(ctx.engine(), |db| db.set_vod_layout(vod.id, &l, &now()))?;
        if p.rerender == Some(true) {
            for c in with_db(ctx.engine(), |db| db.clips_of(vod.id))? {
                if matches!(c.state, ClipState::Ready | ClipState::Failed | ClipState::Approved) {
                    render_soon(ctx, c.id);
                }
            }
        }
        emit(ctx, "vod", Some(vod.id), None);
        clips::vod(ctx.engine(), vod.id)
    }, |_, _, s| Ok(s));

    e.register_unlocked::<ClipGet>(|ctx, p| {
        let clip = clips::clip(ctx.engine(), p.id)?;
        let vod = clips::vod(ctx.engine(), clip.vod_id)?;
        let words = edit::load_words(&Path::new(&vod.dir).join("words.json")).unwrap_or_default();
        let around = edit::between(&words, (clip.start_s - 20.0).max(0.0), clip.end_s + 20.0).iter().map(|w| Word {
            text: w.text.trim().to_string(), start_ms: w.start_ms.max(0.0) as u64, end_ms: w.end_ms.max(0.0) as u64,
        }).collect();
        Ok(ClipOut { clip, vod, words: around })
    });

    e.register_unlocked::<ClipList>(|ctx, p| {
        let mut list = match p.vod_id {
            Some(id) => with_db(ctx.engine(), |db| db.clips_of(id))?,
            None => with_db(ctx.engine(), |db| db.clips())?,
        };
        if let Some(states) = p.states.as_ref().filter(|s| !s.is_empty()) {
            list.retain(|c| states.contains(&c.state));
        }
        Ok(ClipsOut { clips: list })
    });

    e.register_staged::<ClipUpdate, Clip>(|ctx, p| {
        let c = clips::clip(ctx.engine(), p.id)?;
        if matches!(c.state, ClipState::Posting | ClipState::Posted) && p.edit.is_some() {
            return Err(BusError::conflict("clips.posted", "a posted clip's picture cannot change"));
        }
        let vod = clips::vod(ctx.engine(), c.vod_id)?;
        let title = match p.title.as_deref() {
            Some(t) if t.trim().is_empty() || t.trim().chars().count() > 100 => return Err(clips::invalid("a title is 1 to 100 characters")),
            Some(t) => t.trim().to_string(),
            None => c.title.clone(),
        };
        let description = match p.description.as_deref() {
            Some(d) if d.chars().count() > 4000 => return Err(clips::invalid("a description is at most 4000 characters")),
            Some(d) => d.trim().to_string(),
            None => c.description.clone(),
        };
        let tags = p.tags.clone().map(|t| t.into_iter().map(|t| t.trim().trim_start_matches('#').to_lowercase()).filter(|t| !t.is_empty()).take(15).collect()).unwrap_or(c.tags.clone());
        let new_edit = match p.edit.clone() {
            Some(e) => Some(edit::checked(e, vod.duration_s, vod.width, vod.height).map_err(clips::invalid)?),
            None => None,
        };
        let changed_edit = new_edit.as_ref().is_some_and(|e| *e != c.edit);
        let e = new_edit.unwrap_or(c.edit.clone());
        let words = edit::load_words(&Path::new(&vod.dir).join("words.json")).unwrap_or_default();
        let (start, end) = edit::bounds(&e);
        let length = if changed_edit { edit::length(&words, &e) } else { c.length_s };
        with_db(ctx.engine(), |db| db.update_clip(c.id, &title, &description, &tags, p.kind.unwrap_or(c.kind), &e, start, end, length, &now()))?;
        if p.render.unwrap_or(changed_edit) && !matches!(c.state, ClipState::Posting | ClipState::Posted) {
            render_soon(ctx, c.id);
        }
        emit(ctx, "clip", Some(c.vod_id), Some(c.id));
        clips::clip(ctx.engine(), c.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<ClipRender, Clip>(|ctx, p| {
        let c = clips::clip(ctx.engine(), p.id)?;
        if matches!(c.state, ClipState::Posting | ClipState::Posted) {
            return Err(BusError::conflict("clips.posted", "a posted clip is not drawn again"));
        }
        render_soon(ctx, c.id);
        emit(ctx, "clip", Some(c.vod_id), Some(c.id));
        clips::clip(ctx.engine(), c.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<ClipSet, Clip>(|ctx, p| {
        let c = clips::clip(ctx.engine(), p.id)?;
        if matches!(c.state, ClipState::Posting | ClipState::Posted) {
            return Err(BusError::conflict("clips.posted", "this clip is already on YouTube"));
        }
        let t = now();
        match p.state {
            ClipState::Approved => {
                if c.file.is_none() {
                    return Err(BusError::conflict("clips.unrendered", "approve a clip once it is drawn"));
                }
                with_db(ctx.engine(), |db| db.schedule(c.id, None, c.privacy, ClipState::Approved, &t))?;
            }
            ClipState::Rejected => with_db(ctx.engine(), |db| db.schedule(c.id, None, None, ClipState::Rejected, &t))?,
            ClipState::Ready => {
                let back = if c.file.is_some() { ClipState::Ready } else { ClipState::Draft };
                with_db(ctx.engine(), |db| db.schedule(c.id, None, None, back, &t))?;
            }
            other => return Err(clips::invalid(format!("a clip is set approved, rejected or ready, not {}", other.name()))),
        }
        emit(ctx, "clip", Some(c.vod_id), Some(c.id));
        ctx.after_commit(|engine| clips::poke_poster(&engine));
        clips::clip(ctx.engine(), c.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<Post, Clip>(|ctx, p| {
        let c = clips::clip(ctx.engine(), p.id)?;
        if matches!(c.state, ClipState::Posting | ClipState::Posted) {
            return Err(BusError::conflict("clips.posted", "this clip is already on YouTube"));
        }
        if c.file.is_none() || c.state == ClipState::Rendering {
            return Err(BusError::conflict("clips.unrendered", "post a clip once it is drawn"));
        }
        let yt = youtube::status(ctx.engine());
        if !yt.connected {
            return Err(BusError::unavailable("clips.youtube.signed_out", "no YouTube channel is connected").with_hint("Clips → Settings → YouTube"));
        }
        let s = clips::settings(ctx.engine())?;
        let at = match p.at.as_deref() {
            Some(raw) => {
                let ts: jiff::Timestamp = raw.parse().map_err(|_| clips::invalid("`at` is a time like 2026-11-02T17:00:00Z"))?;
                crate::time::stamp(ts.max(jiff::Timestamp::now()))
            }
            None => now(),
        };
        with_db(ctx.engine(), |db| db.schedule(c.id, Some(&at), Some(p.privacy.unwrap_or(s.posting.privacy)), ClipState::Scheduled, &now()))?;
        emit(ctx, "clip", Some(c.vod_id), Some(c.id));
        ctx.after_commit(|engine| clips::poke_poster(&engine));
        clips::clip(ctx.engine(), c.id)
    }, |_, _, s| Ok(s));

    e.register_staged::<ClipDelete, Empty>(|ctx, p| {
        let c = clips::clip(ctx.engine(), p.id)?;
        if c.state == ClipState::Posting {
            return Err(BusError::conflict("clips.posting", "this clip is uploading now"));
        }
        if let Some(f) = c.file.as_deref() {
            let _ = std::fs::remove_file(f);
        }
        with_db(ctx.engine(), |db| db.delete_clip(c.id))?;
        emit(ctx, "clip", Some(c.vod_id), Some(c.id));
        Ok(Empty {})
    }, |_, _, s| Ok(s));

    e.register_staged::<YoutubeSetup, YoutubeStatus>(|ctx, p| {
        let s = youtube::setup(ctx.engine(), &p.client_id, &p.client_secret)?;
        emit(ctx, "youtube", None, None);
        Ok(s)
    }, |_, _, s| Ok(s));
    e.register_unlocked::<YoutubeStatusGet>(|ctx, _: Empty| Ok(youtube::status(ctx.engine())));
    e.register_staged::<YoutubeConnect, YoutubeConnectOut>(|ctx, _| {
        let engine = ctx.engine().arc().ok_or_else(|| BusError::internal("the engine is stopping"))?;
        let url = youtube::connect(&engine)?;
        emit(ctx, "youtube", None, None);
        Ok(YoutubeConnectOut { url })
    }, |_, _, s| Ok(s));
    e.register_staged::<YoutubeDisconnect, YoutubeStatus>(|ctx, _| {
        let s = youtube::disconnect(ctx.engine())?;
        emit(ctx, "youtube", None, None);
        Ok(s)
    }, |_, _, s| Ok(s));
}

/// A VOD's title from its file's name: `YTDown.com_YouTube_Media_<id>_<Title-Words>_001_1080p`
/// reads as `Title Words`.
pub fn title_from_file(path: &str) -> String {
    let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = stem.as_str();
    for prefix in ["YTDown.com_YouTube_Media_", "YTDown.com_"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            // The video's id comes next: eleven characters and an underscore.
            s = rest.get(12..).filter(|_| rest.as_bytes().get(11) == Some(&b'_')).unwrap_or(rest);
        }
    }
    let mut words: Vec<&str> = s.split(['_', '-', ' ', '.']).filter(|w| !w.is_empty()).collect();
    while words.last().is_some_and(|w| w.ends_with('p') && w[..w.len() - 1].parse::<u32>().is_ok() || w.parse::<u32>().is_ok()) {
        words.pop();
    }
    let title = words.join(" ");
    let title = if title.is_empty() { stem } else { title };
    clips::cut(&title, 120)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_file_names() {
        assert_eq!(title_from_file("/d/YTDown.com_YouTube_Media_BPo2vYmNFZo_2-Days-One-T54-Sushi-The-grind-is-over-IdleOn-VOD_001_1080p.mp4"),
            "2 Days One T54 Sushi The grind is over IdleOn VOD");
        assert_eq!(title_from_file("/d/my stream.mkv"), "my stream");
    }
}
