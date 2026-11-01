//! The Clips space across the bus door (docs/CLIPS.md): settings, importing a VOD, a clip's edit
//! checked and saved, approving and posting, and the YouTube sign-in and upload against a stand-in
//! for Google. The pipeline's stages (whisper, Claude, Remotion) are not run here: their parts have
//! unit tests in `clips::*`, and a real VOD is the end-to-end check (docs/CLIPS.md, "Checking it").

mod common;

use common::{code, committed_repo, engine_with_project, ok, wait_until};
use relay_bus::ops::clips::{CaptionStyle, ClipEdit, Kind, Layout};
use relay_core::clips::{self, store::NewClip, youtube};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn setup(root: &Path) -> Arc<Engine> {
    let ws = root.join("ws");
    let repo = ws.join("repo");
    committed_repo(&repo, &[("README.md", "hi\n")]);
    let engine = engine_with_project(root, &ws, &repo);
    clips::assume_tools(&engine);
    engine
}

fn edit(keep: [f64; 2]) -> ClipEdit {
    ClipEdit {
        keep: vec![keep], tighten_ms: Some(700), layout: Layout::Split, view: None, reframe: BTreeMap::new(), hook: "He did it".into(),
        captions: CaptionStyle::Pixel, caption_y: None, emphasis: vec!["did".into()], fixes: BTreeMap::new(), punch_at: vec![], end_card: true, scene: None,
    }
}

/// A VOD imported from `file` and a clip of it, as the find stage would leave it.
fn vod_with_clip(engine: &Engine, file: &Path) -> (i64, i64) {
    let vod = ok(engine, "clips.vod.import", json!({"source": file, "request": "the fails", "kinds": ["fail"]}));
    let id = vod["id"].as_i64().unwrap();
    clips::with_db(engine, |db| db.set_vod_media(id, 3600.0, 1920, 1080, 60.0, None)).unwrap();
    let clip = clips::with_db(engine, |db| db.add_clip(id, &NewClip {
        kind: Kind::Fail, title: "He burned the marshmallow".into(), description: "One job.".into(), tags: vec!["cooking".into()], score: 8,
        why: "A clean fail".into(), start_s: 100.0, end_s: 130.0, length_s: 30.0, edit: edit([100.0, 130.0]),
    }, "2026-10-31T00:00:00Z")).unwrap();
    (id, clip.id)
}

#[test]
fn settings_are_kept_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let mut s = ok(&engine, "clips.settings.get", json!({}));
    assert_eq!(s["captions"], "pixel");
    assert_eq!(s["posting"]["mode"], "ask", "nothing goes out on its own until the person says so");
    s["streamer"] = json!("Nephilheim");
    s["handle"] = json!("@Nephilheim");
    s["posting"]["slots"] = json!(["21:00", "9:30"]);
    let saved = ok(&engine, "clips.settings.set", s.clone());
    assert_eq!(saved["handle"], "Nephilheim");
    assert_eq!(saved["posting"]["slots"], json!(["09:30", "21:00"]));
    assert_eq!(ok(&engine, "clips.settings.get", json!({}))["streamer"], "Nephilheim");
    s["highlight"] = json!("yellow");
    assert_eq!(code(common::call(&engine, "clips.settings.set", s)), "clips.invalid");
}

#[test]
fn a_vod_is_imported_and_its_ask_kept() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    assert_eq!(code(common::call(&engine, "clips.vod.import", json!({"source": "/nowhere/vod.mp4"}))), "clips.file");
    assert_eq!(code(common::call(&engine, "clips.vod.import", json!({"source": "vod.mp4"}))), "clips.invalid");
    let file = root.path().join("YTDown.com_YouTube_Media_Y1STa6HHaGg_Turns-out-I-m-Better-at-Cooking_001_1080p.mp4");
    std::fs::write(&file, b"not really a video").unwrap();
    let url = format!("file://{}", file.display());
    let vod = ok(&engine, "clips.vod.import", json!({"source": url, "request": "funny moments and his dog", "kinds": ["funny", "story"]}));
    assert_eq!(vod["state"], "queued");
    assert_eq!(vod["source"], file.display().to_string(), "a file:// link is read as its path");
    assert_eq!(vod["title"], "Turns out I m Better at Cooking");
    assert_eq!(vod["kinds"], json!(["funny", "story"]));
    assert_eq!(vod["request"], "funny moments and his dog");
    let listed = ok(&engine, "clips.vod.list", json!({}));
    assert_eq!(listed["vods"].as_array().unwrap().len(), 1);
    // A VOD still being worked on is not asked to look again.
    let id = vod["id"].as_i64().unwrap();
    assert_eq!(code(common::call(&engine, "clips.vod.find", json!({"id": id, "request": "more"}))), "clips.busy");
    let summary = ok(&engine, "clips.summary", json!({}));
    assert_eq!(summary["queued"], 1);
    // Deleting it never touches the file it came from.
    ok(&engine, "clips.vod.delete", json!({"id": id}));
    assert!(file.is_file());
    assert!(ok(&engine, "clips.vod.list", json!({}))["vods"].as_array().unwrap().is_empty());
}

#[test]
fn a_clip_edit_is_checked_and_saved() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let file = root.path().join("vod.mp4");
    std::fs::write(&file, b"x").unwrap();
    let (_, clip) = vod_with_clip(&engine, &file);
    let got = ok(&engine, "clips.clip.get", json!({"id": clip}));
    assert_eq!(got["clip"]["title"], "He burned the marshmallow");
    // The words around it: as lines by default (an agent's read), one by one when asked.
    let dir = got["vod"]["dir"].as_str().unwrap().to_string();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(Path::new(&dir).join("words.json"), serde_json::to_vec(&json!([
        {"text": " It", "startMs": 101000, "endMs": 101200}, {"text": " burned.", "startMs": 101200, "endMs": 101700},
    ])).unwrap()).unwrap();
    let lines = ok(&engine, "clips.clip.get", json!({"id": clip}));
    assert_eq!(lines["transcript"], "[101.0] It burned.\n");
    assert_eq!(lines["words"], json!([]));
    assert!(lines["drawing"].is_null(), "not being drawn");
    let each = ok(&engine, "clips.clip.get", json!({"id": clip, "words": true}));
    assert_eq!(each["words"][1], json!({"text": "burned.", "start_ms": 101200, "end_ms": 101700}));
    assert!(got["vod"]["eta_s"].is_number(), "a VOD waiting its turn says about how long it takes: {}", got["vod"]);
    // A span past the VOD's end, or a title too long, is refused.
    let mut far = serde_json::to_value(edit([3590.0, 3700.0])).unwrap();
    assert_eq!(code(common::call(&engine, "clips.clip.update", json!({"id": clip, "edit": far.take(), "render": false}))), "clips.invalid");
    assert_eq!(code(common::call(&engine, "clips.clip.update", json!({"id": clip, "title": "x".repeat(101)}))), "clips.invalid");
    // A cut in the middle and a fix to what was misheard.
    let mut e = edit([100.0, 112.0]);
    e.keep.push([115.0, 131.5]);
    e.fixes.insert("Neffleheim".into(), "Nephilheim".into());
    e.layout = Layout::Fit;
    let saved = ok(&engine, "clips.clip.update", json!({"id": clip, "edit": e, "title": "He Burned The Burned Marshmallow", "tags": ["#Cooking", "fail"], "render": false}));
    assert_eq!(saved["edit"]["keep"], json!([[100.0, 112.0], [115.0, 131.5]]));
    assert_eq!(saved["start_s"], 100.0);
    assert_eq!(saved["end_s"], 131.5);
    assert_eq!(saved["tags"], json!(["cooking", "fail"]));
    assert_eq!(saved["edit"]["fixes"]["Neffleheim"], "Nephilheim");
}

#[test]
fn a_clip_is_approved_only_once_drawn_and_posted_only_when_signed_in() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let file = root.path().join("vod.mp4");
    std::fs::write(&file, b"x").unwrap();
    let (_, clip) = vod_with_clip(&engine, &file);
    assert_eq!(code(common::call(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}))), "clips.unrendered");
    let drawn = root.path().join("clip-1.mp4");
    std::fs::write(&drawn, b"drawn").unwrap();
    clips::with_db(&engine, |db| {
        db.set_clip_file(clip, &drawn.display().to_string(), None, Some(29.5), "t")?;
        db.set_clip_state(clip, relay_bus::ops::clips::ClipState::Ready, None, "t")
    }).unwrap();
    let approved = ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}));
    assert_eq!(approved["state"], "approved");
    assert_eq!(code(common::call(&engine, "clips.clip.post", json!({"id": clip}))), "clips.youtube.signed_out");
    let back = ok(&engine, "clips.clip.set", json!({"id": clip, "state": "ready"}));
    assert_eq!(back["state"], "ready");
    assert_eq!(ok(&engine, "clips.clip.list", json!({"states": ["ready"]}))["clips"].as_array().unwrap().len(), 1);
    assert_eq!(ok(&engine, "clips.summary", json!({}))["to_review"], 1);
}

/// Drawn again, a clip's file is the old picture until the render ends: it is not approved (the
/// poster would send the old one), an approved one keeps its place, and taken back meanwhile it
/// comes back to the person.
#[test]
fn a_clip_being_drawn_again_is_not_approved_and_keeps_its_place() {
    use relay_bus::ops::clips::ClipState;
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let file = root.path().join("vod.mp4");
    std::fs::write(&file, b"x").unwrap();
    let (_, clip) = vod_with_clip(&engine, &file);
    let drawn = root.path().join("clip-1.mp4");
    std::fs::write(&drawn, b"drawn").unwrap();
    clips::with_db(&engine, |db| {
        db.set_clip_file(clip, &drawn.display().to_string(), None, Some(29.5), "t")?;
        db.set_clip_state(clip, ClipState::Ready, None, "t")
    }).unwrap();
    ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}));
    let summary = ok(&engine, "clips.summary", json!({}));
    assert_eq!((summary["approved"].as_u64(), summary["to_review"].as_u64()), (Some(1), Some(0)));
    // Drawn again: waiting for the drawer, and not approved meanwhile.
    assert_eq!(ok(&engine, "clips.clip.render", json!({"id": clip}))["state"], "rendering");
    assert_eq!(code(common::call(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}))), "clips.unrendered");
    assert_eq!(code(common::call(&engine, "clips.clip.post", json!({"id": clip}))), "clips.unrendered");
    // Its render ends: approved again, with the new file.
    clips::with_db(&engine, |db| {
        db.set_render_id(clip, 41, "t")?;
        db.finish_render(clip, 41, "/new.mp4", None, None, None, "t")
    }).unwrap();
    let c = ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"].clone();
    assert_eq!((c["state"].as_str(), c["file"].as_str()), (Some("approved"), Some("/new.mp4")));
    // Drawn again and taken back meanwhile: back for review once drawn.
    ok(&engine, "clips.clip.render", json!({"id": clip}));
    assert_eq!(ok(&engine, "clips.clip.set", json!({"id": clip, "state": "ready"}))["state"], "rendering");
    clips::with_db(&engine, |db| {
        db.set_render_id(clip, 42, "t")?;
        db.finish_render(clip, 42, "/newer.mp4", None, None, None, "t")
    }).unwrap();
    assert_eq!(ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"]["state"], "ready");
}

/// A Clips notification keeps its category across the bus: the space's click route needs it.
#[test]
fn clips_notifications_keep_their_category() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    engine.store.lock().execute(
        "INSERT INTO notifications(project_id, category, title, body, link, read, created_at) VALUES (NULL, 'clips', 'Clips ready', '3 clips wait for you', NULL, 0, '2026-11-01T00:00:00Z')",
        [],
    ).unwrap();
    let list = ok(&engine, "notify.list", json!({"category": "clips"}));
    let n = list["notifications"].as_array().unwrap();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0]["category"], "clips");
    assert_eq!(list["unread"], 1);
    assert_eq!(ok(&engine, "notify.list", json!({}))["notifications"][0]["category"], "clips");
}

/// The sign-in is read from the keyring once and kept: each refresh of the space does not start
/// the keyring's tool. Writing the secrets forgets what was kept.
#[test]
fn the_sign_in_is_read_once_and_read_again_after_a_change() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let status = ok(&engine, "clips.youtube.setup", json!({"client_id": "123-abc.apps.googleusercontent.com", "client_secret": "sec-ret"}));
    assert_eq!(status["connected"], false);
    let kept = engine.store.path().with_file_name("clips-youtube.json");
    assert!(kept.is_file(), "the test instance keeps its secrets in a file");
    // Changed behind Relay's back: still what was read.
    std::fs::write(&kept, json!({"client_id": "123-abc.apps.googleusercontent.com", "client_secret": "sec-ret", "refresh_token": "rt"}).to_string()).unwrap();
    assert_eq!(ok(&engine, "clips.youtube.status", json!({}))["connected"], false);
    assert_eq!(ok(&engine, "clips.summary", json!({}))["youtube"]["connected"], false);
    // Written by Relay: read again.
    let status = ok(&engine, "clips.youtube.setup", json!({"client_id": "123-abc.apps.googleusercontent.com", "client_secret": "sec-ret"}));
    assert_eq!(status["connected"], true, "the same client keeps its sign-in");
    assert_eq!(ok(&engine, "clips.summary", json!({}))["youtube"]["connected"], true);
    let out = ok(&engine, "clips.youtube.disconnect", json!({}));
    assert_eq!(out["connected"], false);
}

/// Google, as the sign-in and the upload see it: the token exchange, the channel, the resumable
/// upload's start and its bytes. Each call is kept.
fn fake_google(calls: Arc<Mutex<Vec<youtube::Call>>>) -> youtube::Fake {
    Arc::new(move |call: &youtube::Call| {
        calls.lock().unwrap().push(call.clone());
        let json = |v: Value| youtube::Answer { status: 200, body: v.to_string().into_bytes(), location: None, range: None };
        if call.url.starts_with("https://oauth2.googleapis.com/token") {
            return json(json!({"access_token": "at-1", "expires_in": 3600, "refresh_token": "rt-1"}));
        }
        if call.url.starts_with("https://www.googleapis.com/youtube/v3/channels") {
            return json(json!({"items": [{"id": "UC123", "snippet": {"title": "Neph Clips"}}]}));
        }
        if call.method == "PUT" {
            return json(json!({"id": "vid123"}));
        }
        if call.url.starts_with("https://www.googleapis.com/upload/youtube/v3/videos") {
            return youtube::Answer { status: 200, body: vec![], location: Some("https://www.googleapis.com/upload/youtube/v3/videos?upload_id=u1".into()), range: None };
        }
        youtube::Answer { status: 404, body: b"{}".to_vec(), location: None, range: None }
    })
}

#[test]
fn youtube_signs_in_and_uploads_through_google_alone() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    youtube::use_fake(Some(fake_google(calls.clone())));
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    assert_eq!(code(common::call(&engine, "clips.youtube.setup", json!({"client_id": "nope", "client_secret": "s"}))), "clips.youtube.client");
    let status = ok(&engine, "clips.youtube.setup", json!({"client_id": "123-abc.apps.googleusercontent.com", "client_secret": "sec-ret"}));
    assert_eq!((status["configured"].as_bool(), status["connected"].as_bool()), (Some(true), Some(false)));
    // Connecting answers Google's page, with PKCE and a state, and listens on this PC for the
    // browser to come back.
    let url = ok(&engine, "clips.youtube.connect", json!({}))["url"].as_str().unwrap().to_string();
    assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=123-abc.apps.googleusercontent.com"));
    assert!(url.contains("code_challenge_method=S256") && url.contains("access_type=offline"));
    let param = |k: &str| url.split(['?', '&']).find_map(|kv| kv.strip_prefix(&format!("{k}="))).unwrap().to_string();
    let redirect = param("redirect_uri").replace("%3A", ":").replace("%2F", "/");
    let port: u16 = redirect.rsplit(':').next().unwrap().parse().unwrap();
    let state = param("state");
    // A wrong state is not taken as the answer; then the browser comes back with the right one.
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut page = String::new();
    let _ = s.read_to_string(&mut page);
    assert!(page.starts_with("HTTP/1.1 404"));
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET /?state={state}&code=the-code&scope=x HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut page = String::new();
    let _ = s.read_to_string(&mut page);
    assert!(page.contains("Relay is connected to YouTube"), "{page}");
    wait_until("the sign-in to finish", || ok(&engine, "clips.youtube.status", json!({}))["connected"] == true);
    let status = ok(&engine, "clips.youtube.status", json!({}));
    assert_eq!(status["channel"], "Neph Clips");
    {
        let calls = calls.lock().unwrap();
        let exchange = calls.iter().find(|c| c.url.starts_with("https://oauth2.googleapis.com/token")).unwrap();
        let body = String::from_utf8_lossy(&exchange.body);
        assert!(body.contains("code=the-code") && body.contains("code_verifier=") && body.contains("grant_type=authorization_code"), "{body}");
    }

    // A drawn clip goes out with the streamer credited and #shorts.
    let mut settings = ok(&engine, "clips.settings.get", json!({}));
    settings["streamer"] = json!("Nephilheim");
    settings["handle"] = json!("Nephilheim");
    ok(&engine, "clips.settings.set", settings);
    let file = root.path().join("vod.mp4");
    std::fs::write(&file, b"x").unwrap();
    let (_, clip) = vod_with_clip(&engine, &file);
    let drawn = root.path().join("clip.mp4");
    std::fs::write(&drawn, b"the clip's bytes").unwrap();
    clips::with_db(&engine, |db| {
        db.set_clip_file(clip, &drawn.display().to_string(), None, Some(29.5), "t")?;
        db.set_clip_state(clip, relay_bus::ops::clips::ClipState::Ready, None, "t")
    }).unwrap();
    let scheduled = ok(&engine, "clips.clip.post", json!({"id": clip, "privacy": "unlisted"}));
    assert_eq!(scheduled["state"], "scheduled");
    let c: relay_bus::ops::clips::Clip = serde_json::from_value(ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"].clone()).unwrap();
    let s: relay_bus::ops::clips::Settings = serde_json::from_value(ok(&engine, "clips.settings.get", json!({}))).unwrap();
    youtube::post_now(&engine, &c, &s).unwrap();
    let posted = ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"].clone();
    assert_eq!(posted["state"], "posted");
    assert_eq!(posted["youtube_url"], "https://youtube.com/shorts/vid123");
    let calls = calls.lock().unwrap();
    let start = calls.iter().find(|c| c.url.starts_with("https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable")).unwrap();
    let meta: Value = serde_json::from_slice(&start.body).unwrap();
    assert_eq!(meta["status"]["privacyStatus"], "unlisted");
    assert_eq!(meta["snippet"]["title"], "He burned the marshmallow");
    let description = meta["snippet"]["description"].as_str().unwrap();
    assert!(description.contains("From Nephilheim's stream: https://twitch.tv/nephilheim") && description.ends_with("#shorts"), "{description}");
    assert!(start.headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer at-1"));
    let put = calls.iter().find(|c| c.method == "PUT").unwrap();
    assert_eq!(put.body, b"the clip's bytes");
    drop(calls);
    // Signed out: the sign-in is forgotten, the client kept.
    let out = ok(&engine, "clips.youtube.disconnect", json!({}));
    assert_eq!((out["configured"].as_bool(), out["connected"].as_bool()), (Some(true), Some(false)));
    youtube::use_fake(None);
}

/// A clip drawn to a file in `root` and set ready, as the pipeline leaves it.
fn ready_clip(engine: &Engine, root: &Path) -> (i64, i64) {
    let file = root.join("vod.mp4");
    std::fs::write(&file, b"x").unwrap();
    let (vod, clip) = vod_with_clip(engine, &file);
    let drawn = root.join(format!("clip-{clip}.mp4"));
    std::fs::write(&drawn, b"drawn").unwrap();
    clips::with_db(engine, |db| {
        db.set_clip_file(clip, &drawn.display().to_string(), None, Some(29.5), "t")?;
        db.set_clip_state(clip, relay_bus::ops::clips::ClipState::Ready, None, "t")
    }).unwrap();
    (vod, clip)
}

fn agent() -> relay_bus::Actor {
    relay_bus::Actor::Agent("merry-badger".into())
}

/// An agent may edit and draw a clip, never post one: what it changes on an approved clip goes
/// back to the person, with a word on why, instead of out at the next slot.
#[test]
fn an_agents_change_to_an_approved_clip_goes_back_to_the_person() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let (_, clip) = ready_clip(&engine, root.path());
    ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}));
    let changed = common::call_as(&engine, agent(), "clips.clip.update", json!({"id": clip, "title": "Something else entirely"}));
    let c = changed.into_result().unwrap_or_else(|e| panic!("{} {}", e.code, e.message));
    assert_eq!(c["state"], "ready", "{c}");
    assert!(c["error"].as_str().unwrap().contains("An agent changed this clip"));
    // Drawn again at an agent's ask: back to the person once drawn, not to the schedule.
    ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}));
    let c = common::call_as(&engine, agent(), "clips.clip.render", json!({"id": clip})).into_result().unwrap();
    assert_eq!(c["state"], "rendering");
    clips::with_db(&engine, |db| {
        db.set_render_id(clip, 51, "t")?;
        db.finish_render(clip, 51, "/new.mp4", None, None, None, "t")
    }).unwrap();
    assert_eq!(ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"]["state"], "ready");
    // The person's own redraw keeps its place.
    ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}));
    ok(&engine, "clips.clip.render", json!({"id": clip}));
    clips::with_db(&engine, |db| {
        db.set_render_id(clip, 52, "t")?;
        db.finish_render(clip, 52, "/newer.mp4", None, None, None, "t")
    }).unwrap();
    assert_eq!(ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"]["state"], "approved");
}

/// A clip whose render is still drawing (rejected meanwhile, say) is not approved or posted: the
/// render would write over the file that goes out.
#[test]
fn a_clip_is_not_approved_while_a_render_draws_to_it() {
    use relay_bus::ops::clips::ClipState;
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let (_, clip) = ready_clip(&engine, root.path());
    let render: i64 = {
        let conn = engine.store.lock();
        conn.execute("INSERT INTO videos(project_id, session, title, dir, width, height, fps, duration_frames, created_by, updated_by, created_at, updated_at)
            VALUES (1, NULL, 'v', '/nowhere', 1080, 1920, 30, 900, 'user', 'user', 't', 't')", []).unwrap();
        let video = conn.last_insert_rowid();
        conn.execute("INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, actor, created_at)
            VALUES (?1, 1, 'short', 'mp4', 'standard', 'rendering', 1, 'user', 't')", [video]).unwrap();
        conn.last_insert_rowid()
    };
    // Drawn again, then rejected while it draws: its old file and a render still writing.
    ok(&engine, "clips.clip.render", json!({"id": clip}));
    clips::with_db(&engine, |db| db.set_render_id(clip, render, "t")).unwrap();
    ok(&engine, "clips.clip.set", json!({"id": clip, "state": "rejected"}));
    assert_eq!(clips::clip(&engine, clip).unwrap().state, ClipState::Rejected);
    assert_eq!(code(common::call(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}))), "clips.unrendered");
    // Its render done: approved as asked.
    engine.store.lock().execute("UPDATE video_renders SET state = 'done' WHERE id = ?1", [render]).unwrap();
    assert_eq!(ok(&engine, "clips.clip.set", json!({"id": clip, "state": "approved"}))["state"], "approved");
}

/// A VOD's framing changed for all its clips redraws the scheduled ones and those being drawn
/// too: none goes out with the old framing.
#[test]
fn a_new_framing_redraws_scheduled_clips_too() {
    use relay_bus::ops::clips::ClipState;
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let (vod, clip) = ready_clip(&engine, root.path());
    clips::with_db(&engine, |db| db.schedule(clip, ClipState::Ready, Some("2026-11-02T17:00:00Z"), None, ClipState::Scheduled, "t")).unwrap();
    let layout = json!({"kind": "camera", "subject": {"x": 400, "y": 0, "w": 800, "h": 1080}, "by": "person"});
    ok(&engine, "clips.vod.layout", json!({"id": vod, "layout": layout, "rerender": true}));
    assert_eq!(ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"]["state"], "rendering");
    clips::with_db(&engine, |db| {
        db.set_render_id(clip, 61, "t")?;
        db.finish_render(clip, 61, "/new.mp4", None, None, None, "t")
    }).unwrap();
    let c = ok(&engine, "clips.clip.get", json!({"id": clip}))["clip"].clone();
    assert_eq!((c["state"].as_str(), c["publish_at"].as_str()), (Some("scheduled"), Some("2026-11-02T17:00:00Z")));
}

/// Deleting a clip, or its VOD, takes its cut footage and drawing off the disk; a VOD with a clip
/// uploading is not deleted under it.
#[test]
fn deleting_takes_the_footage_and_waits_for_an_upload() {
    use relay_bus::ops::clips::ClipState;
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let (vod, clip) = ready_clip(&engine, root.path());
    let video_dir = root.path().join("video");
    std::fs::create_dir_all(video_dir.join("public/assets")).unwrap();
    let video_id: i64 = {
        let conn = engine.store.lock();
        conn.execute("INSERT INTO videos(project_id, session, title, dir, width, height, fps, duration_frames, created_by, updated_by, created_at, updated_at)
            VALUES (1, NULL, 'v', ?1, 1080, 1920, 30, 900, 'user', 'user', 't', 't')", [video_dir.display().to_string()]).unwrap();
        conn.last_insert_rowid()
    };
    let footage = video_dir.join(format!("public/assets/clip-{clip}.mp4"));
    std::fs::write(&footage, b"cut").unwrap();
    clips::with_db(&engine, |db| {
        db.set_vod_video(vod, video_id)?;
        db.set_section(clip, &clips::store::Section { file: format!("assets/clip-{clip}.mp4"), start_s: 98.0, end_s: 135.0 })
    }).unwrap();
    let drawn = clips::clip(&engine, clip).unwrap().file.unwrap();
    // Uploading: the VOD stays.
    clips::with_db(&engine, |db| db.set_clip_state(clip, ClipState::Posting, None, "t")).unwrap();
    assert_eq!(code(common::call(&engine, "clips.vod.delete", json!({"id": vod}))), "clips.posting");
    clips::with_db(&engine, |db| db.set_clip_state(clip, ClipState::Ready, None, "t")).unwrap();
    ok(&engine, "clips.clip.delete", json!({"id": clip}));
    assert!(!footage.exists() && !Path::new(&drawn).exists(), "the clip's footage and drawing go with it");
    // The VOD's other clips' files go with the VOD.
    let (vod2, clip2) = ready_clip(&engine, root.path());
    let drawn2 = clips::clip(&engine, clip2).unwrap().file.unwrap();
    ok(&engine, "clips.vod.delete", json!({"id": vod2}));
    assert!(!Path::new(&drawn2).exists());
}

/// yt-dlp fetches any URL: an agent's link to this PC or the local network is refused.
#[test]
fn an_agent_imports_internet_links_only() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    for url in ["http://127.0.0.1:8080/vod.mp4", "http://192.168.1.5/vod.mp4", "http://169.254.169.254/latest/meta-data", "http://localhost/x", "http://[::1]/x", "http://nas/x"] {
        let r = common::call_as(&engine, agent(), "clips.vod.import", json!({"source": url}));
        assert_eq!(code(r), "clips.link", "{url}");
    }
    assert!(relay_core::handlers::clips::private_host("http://10.0.0.1/x"));
    assert!(!relay_core::handlers::clips::private_host("https://93.184.216.34/videos/1"));
}

/// A second Connect takes over: the first stops listening, and its end says nothing over the new
/// sign-in's.
#[test]
fn a_second_sign_in_takes_over_from_the_first() {
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    ok(&engine, "clips.youtube.setup", json!({"client_id": "123-abc.apps.googleusercontent.com", "client_secret": "sec-ret"}));
    let port = |url: &str| -> u16 {
        let redirect = url.split(['?', '&']).find_map(|kv| kv.strip_prefix("redirect_uri=")).unwrap().replace("%3A", ":").replace("%2F", "/");
        redirect.rsplit(':').next().unwrap().parse().unwrap()
    };
    let first = port(ok(&engine, "clips.youtube.connect", json!({}))["url"].as_str().unwrap());
    let second = port(ok(&engine, "clips.youtube.connect", json!({}))["url"].as_str().unwrap());
    wait_until("the first sign-in to stop listening", || std::net::TcpStream::connect(("127.0.0.1", first)).is_err());
    let status = ok(&engine, "clips.youtube.status", json!({}));
    assert_eq!(status["pending"], true, "the newer sign-in still waits");
    assert!(status["note"].is_null(), "{status}");
    assert!(std::net::TcpStream::connect(("127.0.0.1", second)).is_ok());
}

/// The watcher fell behind the engine's events (a finish may be among those lost): every clip
/// still drawing is brought up to its render.
#[test]
fn a_watcher_that_fell_behind_looks_at_every_clip_drawing() {
    use relay_bus::ops::clips::ClipState;
    let root = tempfile::tempdir().unwrap();
    let engine = setup(root.path());
    let (_, clip) = ready_clip(&engine, root.path());
    let render: i64 = {
        let conn = engine.store.lock();
        conn.execute("INSERT INTO videos(project_id, session, title, dir, width, height, fps, duration_frames, created_by, updated_by, created_at, updated_at)
            VALUES (1, NULL, 'v', '/nowhere', 1080, 1920, 30, 900, 'user', 'user', 't', 't')", []).unwrap();
        let video = conn.last_insert_rowid();
        conn.execute("INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, actor, created_at, error)
            VALUES (?1, 1, 'short', 'mp4', 'standard', 'failed', 1, 'user', 't', 'the GPU ran out of memory')", [video]).unwrap();
        conn.last_insert_rowid()
    };
    clips::with_db(&engine, |db| {
        db.ask_render(clip, "t")?;
        db.set_render_id(clip, render, "t")
    }).unwrap();
    // A channel of one: three events, and the watcher's first look is `Lagged`.
    let (tx, rx) = tokio::sync::broadcast::channel(1);
    for _ in 0..3 {
        let _ = tx.send(relay_bus::envelope::Event::new("session.output", "t".into(), relay_bus::Actor::System, json!({})));
    }
    drop(tx);
    let weak = Arc::downgrade(&engine);
    let watcher = std::thread::spawn(move || clips::pipeline::watch(weak, rx));
    wait_until("the clip to take its render's end", || clips::clip(&engine, clip).unwrap().state == ClipState::Failed);
    assert!(clips::clip(&engine, clip).unwrap().error.unwrap().contains("ran out of memory"));
    watcher.join().unwrap();
}
