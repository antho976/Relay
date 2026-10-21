//! `video.*` (BUS.md §10.19, docs/VIDEO.md): videos written as code through the bus — the folder
//! a video is made with, the plugin switch and the write access it gives, the worker (a fake one,
//! `fixtures/video/fake_worker.py`, speaking its protocol), checks, stills and the issues found in
//! them, versions, renders and their queue, and assets.

mod common;

use common::{call, call_as, code, committed_repo, engine_with_project, ok, ok_as, refused, wait_until};
use relay_bus::{Actor, Event};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A store on disk (videos live beside it) with project 1.
fn project() -> (Arc<Engine>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    (engine, root)
}

/// The fake worker stands in for Node and Remotion, and the kit counts as set up.
fn with_worker(e: &Engine) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/video/fake_worker.py");
    e.video.use_worker(vec!["python3".into(), script.display().to_string()]);
}

fn create(e: &Engine) -> (i64, PathBuf) {
    let made = ok(e, "video.create", json!({"project_id": 1, "title": "Tally launch"}));
    (made["video"]["id"].as_i64().unwrap(), PathBuf::from(made["video"]["dir"].as_str().unwrap()))
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn agent(e: &Engine) -> (String, Actor) {
    let name = ok(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let actor = Actor::agent(&name);
    (name, actor)
}

#[test]
fn a_video_is_a_folder_made_from_the_starter() {
    let (e, root) = project();
    let made = ok(&e, "video.create", json!({"project_id": 1, "title": "Tally — launch", "shape": "landscape", "seconds": 4}));
    let video = &made["video"];
    let dir = PathBuf::from(video["dir"].as_str().unwrap());
    assert_eq!(dir, root.path().join("store/videos/p1/1-tally-launch"));
    assert_eq!((video["width"].as_u64(), video["height"].as_u64(), video["duration_frames"].as_u64()), (Some(1920), Some(1080), Some(120)));
    let files: Vec<&str> = made["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, ["src/Root.tsx", "src/Video.tsx", "src/index.ts", "tsconfig.json"]);
    assert!(dir.join("public").is_dir());
    let root_tsx = fs::read_to_string(dir.join("src/Root.tsx")).unwrap();
    for wanted in ["width={1920}", "height={1080}", "fps={30}", "durationInFrames={120}", "title: \"Tally — launch\""] {
        assert!(root_tsx.contains(wanted), "{wanted} not in Root.tsx:\n{root_tsx}");
    }
    assert!(!root_tsx.contains("__"), "a placeholder was left: {root_tsx}");
    assert!(fs::read_to_string(dir.join("src/index.ts")).unwrap().contains("registerRoot(relayRoot(Root))"));
    assert!(made["entry"].as_str().unwrap().ends_with("src/Video.tsx"));
    // Without a kit, the answer says how to get one.
    assert!(made["next"].as_str().unwrap().contains("video.kit.install") || made["kit"]["state"] == "ready");

    let listed = ok(&e, "video.list", json!({"project_id": 1}));
    assert_eq!(listed["videos"].as_array().unwrap().len(), 1);
    let got = ok(&e, "video.get", json!({"video_id": 1}));
    assert_eq!(got["compositions"], json!([]));
    assert_eq!(got["props"], json!({}));

    assert_eq!(code(call(&e, "video.create", json!({"project_id": 1, "width": 1081, "height": 1920}))), "video.size");
    assert_eq!(code(call(&e, "video.create", json!({"project_id": 1, "width": 1080}))), "video.size");
    assert_eq!(code(call(&e, "video.create", json!({"project_id": 1, "seconds": 0.5}))), "video.seconds");
    assert_eq!(code(call(&e, "video.create", json!({"project_id": 1, "title": "  "}))), "video.title");
    assert_eq!(code(call(&e, "video.get", json!({"video_id": 99}))), "video.not_found");
}

#[test]
fn agents_have_videos_only_where_the_plugin_is_on_and_may_write_their_folders() {
    let (e, root) = project();
    let (name, actor) = agent(&e);
    assert_eq!(code(call_as(&e, actor.clone(), "video.create", json!({"project_id": 1}))), "plugin.off");
    // Whether the machine could render is not the plugin's to hide.
    ok_as(&e, actor.clone(), "video.kit.status", json!({}));

    let videos = root.path().join("store/videos/p1");
    let roots = |e: &Engine| {
        let conn = e.store.lock();
        relay_core::guardrail::write_roots(&relay_core::guardrail::config(&conn, Some(1)).unwrap(), Path::new("/tmp/checkout"))
    };
    assert!(!roots(&e).contains(&videos));
    ok(&e, "plugin.enable", json!({"plugin_id": "video", "project_id": 1, "enabled": true}));
    assert!(roots(&e).contains(&videos), "{:?}", roots(&e));

    let made = ok_as(&e, actor.clone(), "video.create", json!({"project_id": 1, "title": "Promo"}));
    assert_eq!(made["video"]["session"], name.as_str());
    assert!(Path::new(made["video"]["dir"].as_str().unwrap()).starts_with(&videos));
    // An agent ties a video to its own terminal only.
    let (other, _) = agent(&e);
    assert_eq!(code(call_as(&e, actor, "video.update", json!({"video_id": 1, "session": other}))), "actor.scope");
}

#[test]
fn without_a_kit_the_worker_is_not_started_and_the_refusal_says_what_to_do() {
    let (e, _root) = project();
    let (id, _) = create(&e);
    let refusal = refused(call(&e, "video.check", json!({"video_id": id})));
    assert!(refusal.code.starts_with("video.kit."), "{}", refusal.code);
    assert!(refusal.hint.is_some());
}

#[test]
fn a_check_builds_and_keeps_the_compositions() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    let checked = ok(&e, "video.check", json!({"video_id": id}));
    assert_eq!(checked["ok"], true);
    assert_eq!(checked["errors"], json!([]));
    assert_eq!(checked["version"], 1);
    let ids: Vec<&str> = checked["compositions"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["main", "wide"]);
    // The row takes the default composition's size and length from the code.
    let video = &ok(&e, "video.get", json!({"video_id": id}))["video"];
    assert_eq!((video["duration_frames"].as_u64(), video["version"].as_i64()), (Some(90), Some(1)));

    fs::write(dir.join("src/TYPED"), "").unwrap();
    let typed = ok(&e, "video.check", json!({"video_id": id}));
    assert_eq!((typed["ok"].as_bool(), typed["errors"][0]["kind"].as_str()), (Some(true), Some("type")));
    fs::remove_file(dir.join("src/TYPED")).unwrap();
    fs::write(dir.join("src/BROKEN"), "").unwrap();
    let broken = ok(&e, "video.check", json!({"video_id": id}));
    assert_eq!(broken["ok"], false);
    assert_eq!((broken["errors"][0]["file"].as_str(), broken["errors"][0]["line"].as_u64()), (Some("src/Video.tsx"), Some(3)));
    // A still of code that does not build is refused with the first error, at its place.
    let refusal = refused(call(&e, "video.still", json!({"video_id": id})));
    assert_eq!(refusal.code, "video.build");
    assert!(refusal.message.starts_with("src/Video.tsx:3: "), "{}", refusal.message);
}

#[test]
fn a_still_answers_a_picture_and_the_issues_across_its_frames() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));

    let sheet = ok(&e, "video.still", json!({"video_id": id, "count": 3}));
    let frames: Vec<u64> = sheet["frames"].as_array().unwrap().iter().map(|f| f["frame"].as_u64().unwrap()).collect();
    assert_eq!(frames, [0, 45, 89]);
    assert_eq!(sheet["frames"][1]["label"], "1.5s · f45");
    let png = PathBuf::from(sheet["png"].as_str().unwrap());
    assert_eq!(png.parent().unwrap().file_name().unwrap(), "video-stills");
    assert_eq!(&fs::read(&png).unwrap()[..4], b"\x89PNG");
    // The fake's title runs into the right edge's safe margin on every frame: one issue, three frames.
    let issues = sheet["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!((issues[0]["kind"].as_str(), issues[0]["id"].as_str()), (Some("unsafe"), Some("title")));
    assert_eq!(issues[0]["frames"], json!([0, 45, 89]));
    assert!(sheet.get("boxes").is_none());
    // The tiles a sheet is made of are not left behind.
    let names: Vec<String> = fs::read_dir(png.parent().unwrap()).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(names.iter().all(|n| !n.starts_with("t-")), "{names:?}");

    let one = ok(&e, "video.still", json!({"video_id": id, "at": ["1s"], "boxes": true}));
    assert_eq!(one["frames"][0]["frame"], 30);
    assert_eq!(one["boxes"][0]["id"], "title");
    // A frame drawn becomes the video's poster.
    let poster = ok(&e, "video.get", json!({"video_id": id}))["video"]["poster"].as_str().unwrap().to_string();
    assert!(Path::new(&poster).is_file());

    assert_eq!(code(call(&e, "video.still", json!({"video_id": id, "composition": "nope"}))), "video.composition");
    assert_eq!(code(call(&e, "video.still", json!({"video_id": id, "at": ["soon"]}))), "video.moment");
    fs::write(dir.join("src/THROWS"), "").unwrap();
    let refusal = refused(call(&e, "video.still", json!({"video_id": id, "at": [0.5]})));
    assert_eq!(refusal.code, "video.runtime");
    assert!(refusal.message.starts_with("src/Video.tsx:12: boom"), "{}", refusal.message);
}

#[test]
fn each_read_records_a_version_when_the_code_changed_and_a_restore_puts_it_back() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    let original = fs::read_to_string(dir.join("src/Video.tsx")).unwrap();
    let mut rx = e.subscribe();
    assert_eq!(ok(&e, "video.check", json!({"video_id": id}))["version"], 1);
    assert_eq!(ok(&e, "video.check", json!({"video_id": id}))["version"], 1, "nothing changed");
    fs::write(dir.join("src/Video.tsx"), "export const Video = () => null;\n").unwrap();
    fs::write(dir.join("src/Extra.tsx"), "export {};\n").unwrap();
    assert_eq!(ok(&e, "video.still", json!({"video_id": id, "at": [0]}))["version"], 2);
    let changed = drain(&mut rx).into_iter().filter(|ev| ev.ev == "video.changed").count();
    assert!(changed >= 2, "a new version is announced");

    let versions = ok(&e, "video.get", json!({"video_id": id}))["versions"].clone();
    assert_eq!(versions.as_array().unwrap().iter().map(|v| v["n"].as_i64().unwrap()).collect::<Vec<_>>(), [2, 1]);

    // Renders and copied assets are not the code's, and a restore leaves them.
    fs::create_dir_all(dir.join("out")).unwrap();
    fs::write(dir.join("out/main-1.mp4"), "render").unwrap();
    let restored = ok(&e, "video.version.restore", json!({"video_id": id, "version": 1}));
    assert_eq!(restored["previous"], 2);
    assert_eq!(restored["video"]["version"], 3);
    assert_eq!(fs::read_to_string(dir.join("src/Video.tsx")).unwrap(), original);
    assert!(!dir.join("src/Extra.tsx").exists());
    assert!(dir.join("out/main-1.mp4").exists());
    // Its undo is a restore of the version it recorded first.
    ok(&e, "video.version.restore", json!({"video_id": id, "version": 2}));
    assert!(dir.join("src/Extra.tsx").exists());
    assert_eq!(code(call(&e, "video.version.restore", json!({"video_id": id, "version": 40}))), "video.version");
}

#[test]
fn a_render_runs_on_the_queue_reports_its_tenths_and_then_its_end() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));
    let mut rx = e.subscribe();
    let queued = ok(&e, "video.render", json!({"video_id": id, "props": {"title": "Bye"}, "from": "1s", "to": "2s"}));
    assert_eq!((queued["state"].as_str(), queued["range"].clone()), (Some("queued"), json!([30, 60])));
    let render_id = queued["id"].as_i64().unwrap();
    wait_until("the render to finish", || ok(&e, "video.render.get", json!({"render_id": render_id}))["state"] == "done");
    let done = ok(&e, "video.render.get", json!({"render_id": render_id}));
    let path = PathBuf::from(done["path"].as_str().unwrap());
    assert_eq!(path, dir.join("out/main-1.mp4"));
    assert!(fs::read_to_string(&path).unwrap().contains("Bye"), "the render drew with the props it was given");
    assert_eq!((done["progress"].as_f64(), done["bytes"].as_i64().unwrap() > 0), (Some(1.0), true));

    let events = drain(&mut rx);
    let progress: Vec<f64> = events.iter().filter(|ev| ev.ev == "video.render.changed" && ev.payload["state"] == "rendering")
        .map(|ev| ev.payload["progress"].as_f64().unwrap()).collect();
    assert_eq!(progress, [0.0, 0.1, 0.3, 0.5, 0.7, 0.9], "a tenth at a time, never every report");
    let finished: Vec<&Event> = events.iter().filter(|ev| ev.ev == "video.render.finished").collect();
    assert_eq!(finished.len(), 1);
    assert_eq!((finished[0].payload["id"].as_i64(), finished[0].project_id), (Some(render_id), Some(1)));
    // The list shows its newest render.
    let listed = ok(&e, "video.list", json!({"project_id": 1}));
    assert_eq!(listed["videos"][0]["last_render"]["state"], "done");
}

#[test]
fn a_render_is_cancelled_queued_or_running() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/SLOW"), "").unwrap();
    let running = ok(&e, "video.render", json!({"video_id": id}))["id"].as_i64().unwrap();
    let queued = ok(&e, "video.render", json!({"video_id": id, "format": "webm"}))["id"].as_i64().unwrap();
    wait_until("the first to start", || ok(&e, "video.render.get", json!({"render_id": running}))["state"] == "rendering");
    assert_eq!(ok(&e, "video.render.cancel", json!({"render_id": queued}))["state"], "cancelled");
    ok(&e, "video.render.cancel", json!({"render_id": running}));
    wait_until("the running one to stop", || ok(&e, "video.render.get", json!({"render_id": running}))["state"] == "cancelled");
    // The queued one never ran.
    assert_eq!(ok(&e, "video.render.get", json!({"render_id": queued}))["started_at"], Value::Null);
}

#[test]
fn renders_left_running_by_a_stopped_relay_are_failed_at_start() {
    let (e, _root) = project();
    let (id, _) = create(&e);
    e.store.lock().execute(
        "INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, actor, created_at) VALUES (?1, 1, 'main', 'mp4', 'standard', 'rendering', 0, 'user', '2026-10-21T00:00:00Z')",
        [id],
    ).unwrap();
    relay_core::video::jobs::reconcile(&e);
    let render = ok(&e, "video.render.get", json!({"render_id": 1}));
    assert_eq!((render["state"].as_str(), render["error"].as_str()), (Some("failed"), Some("Relay stopped before it finished")));
}

#[test]
fn props_the_person_sets_are_held_to_the_schema_and_drawn_with() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, _) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));
    ok(&e, "video.update", json!({"video_id": id, "props": {"main": {"title": "Hi there"}}}));
    assert_eq!(ok(&e, "video.get", json!({"video_id": id}))["props"], json!({"main": {"title": "Hi there"}}));
    assert_eq!(code(call(&e, "video.update", json!({"video_id": id, "props": {"main": {"title": 3}}}))), "video.props");
    assert_eq!(code(call(&e, "video.update", json!({"video_id": id, "props": {"main": {"colour": "red"}}}))), "video.props");
    let still = ok(&e, "video.still", json!({"video_id": id, "at": [0]}));
    assert_eq!(still["issues"][0]["text"], "Hi there", "stills draw with the person's props");
    let over = ok(&e, "video.still", json!({"video_id": id, "at": [0], "props": {"title": "Asked"}}));
    assert_eq!(over["issues"][0]["text"], "Asked", "a still's own props go over them");
    ok(&e, "video.update", json!({"video_id": id, "props": {"main": null}}));
    assert_eq!(ok(&e, "video.get", json!({"video_id": id}))["props"], json!({}));
    assert_eq!(code(call(&e, "video.update", json!({"video_id": id, "composition": "nope"}))), "video.composition");
    assert_eq!(ok(&e, "video.update", json!({"video_id": id, "composition": "wide"}))["composition"], "wide");
}

#[test]
fn assets_are_copied_in_and_never_written_over() {
    let (e, root) = project();
    let (id, dir) = create(&e);
    let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0, 0, 0];
    let first = root.path().join("Logo Final.png");
    fs::write(&first, png).unwrap();
    let added = ok(&e, "video.asset.add", json!({"video_id": id, "path": first}));
    assert_eq!((added["static_file"].as_str(), added["kind"].as_str()), (Some("assets/Logo-Final.png"), Some("image")));
    assert_eq!((added["w"].as_u64(), added["h"].as_u64()), (Some(2), Some(3)));
    assert!(dir.join("public/assets/Logo-Final.png").is_file());
    // The same file again is the same asset; another under the same name gets a number.
    assert_eq!(ok(&e, "video.asset.add", json!({"video_id": id, "path": first}))["static_file"], "assets/Logo-Final.png");
    let other = root.path().join("other.png");
    fs::write(&other, [png, b"more"].concat()).unwrap();
    assert_eq!(ok(&e, "video.asset.add", json!({"video_id": id, "path": other, "name": "Logo Final.png"}))["static_file"], "assets/Logo-Final-2.png");
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "path": "relative.png"}))), "video.asset");
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "path": root.path().join("missing.mp3")}))), "video.asset");
}

#[test]
fn a_deleted_video_is_listed_only_when_asked_and_comes_back() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.delete", json!({"video_id": id}));
    assert_eq!(ok(&e, "video.list", json!({"project_id": 1}))["videos"], json!([]));
    assert_eq!(ok(&e, "video.list", json!({"project_id": 1, "include_deleted": true}))["videos"].as_array().unwrap().len(), 1);
    assert_eq!(code(call(&e, "video.still", json!({"video_id": id}))), "video.deleted");
    assert!(dir.join("src/Video.tsx").exists(), "deleting a video leaves its code");
    assert_eq!(ok(&e, "video.restore", json!({"video_id": id}))["deleted_at"], Value::Null);
}

#[test]
fn a_scan_checks_every_few_frames_where_words_rest_and_how_long_they_stay() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, _) = create(&e);
    let scanned = ok(&e, "video.scan", json!({"video_id": id}));
    assert_eq!((scanned["every"].as_u64(), scanned["frames"].as_u64()), (Some(3), Some(30)));
    let kinds: Vec<&str> = scanned["issues"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["unsafe", "brief_text"], "{}", scanned["issues"]);
    // The title is judged where it rests, not on its way up: from frame 12, the first it was still.
    assert_eq!(scanned["issues"][0]["frames"][0], 12);
    assert_eq!(scanned["issues"][1]["id"], "flash");
    // Answers stay short: no schema or default props.
    assert!(scanned["composition"].get("props_schema").is_none() && scanned["composition"].get("default_props").is_none());
    let part = ok(&e, "video.scan", json!({"video_id": id, "every": 10, "from": "1s", "to": "2s"}));
    assert_eq!(part["frames"], 4);
}

#[test]
fn versions_say_what_they_changed() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/Hook.tsx"), "export const Hook = () => null;\n").unwrap();
    ok(&e, "video.check", json!({"video_id": id}));
    let versions = ok(&e, "video.get", json!({"video_id": id}))["versions"].clone();
    assert_eq!(versions[0]["changed"], json!(["+src/Hook.tsx"]));
    assert_eq!(versions[1]["changed"], json!(["the starter"]));
}
