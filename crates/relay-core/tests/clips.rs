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
}

/// Google, as the sign-in and the upload see it: the token exchange, the channel, the resumable
/// upload's start and its bytes. Each call is kept.
fn fake_google(calls: Arc<Mutex<Vec<youtube::Call>>>) -> youtube::Fake {
    Arc::new(move |call: &youtube::Call| {
        calls.lock().unwrap().push(call.clone());
        let json = |v: Value| youtube::Answer { status: 200, body: v.to_string().into_bytes(), location: None };
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
            return youtube::Answer { status: 200, body: vec![], location: Some("https://www.googleapis.com/upload/youtube/v3/videos?upload_id=u1".into()) };
        }
        youtube::Answer { status: 404, body: b"{}".to_vec(), location: None }
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
