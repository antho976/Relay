//! `video.*` (BUS.md §10.19, docs/VIDEO.md): videos written as code through the bus — the folder
//! a video is made with, the plugin switch and the write access it gives, the worker (a fake one,
//! `fixtures/video/fake_worker.py`, speaking its protocol), checks, stills and the issues found in
//! them, versions, renders and their queue, and assets.

mod common;

use common::{call, call_as, code, committed_repo, engine_with_project, ok, ok_as, refused, wait_until};
use relay_bus::{Actor, Event};
use relay_core::engine::Engine;
use rusqlite::params;
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
    // The worker's log holds its words, not the page's probe lines.
    let log_path = e.store.videos_dir().join(".relay/worker.log");
    wait_until("the log to hold the check", || fs::read_to_string(&log_path).is_ok_and(|l| l.contains("checking ")));
    assert!(!fs::read_to_string(&log_path).unwrap().contains("__relay_probe__"));
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

/// A still of many frames at full scale is drawn smaller, within the pixels a still may hold:
/// 24 frames of a 1080 x 1920 composition come to 50 MP at full scale.
#[test]
fn a_still_of_many_full_frames_is_held_to_its_pixels() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, _) = create(&e);
    let at: Vec<String> = (0..24).map(|i| format!("f{}", i * 3)).collect();
    let still = ok(&e, "video.still", json!({"video_id": id, "at": at, "scale": 1.0}));
    let (w, h) = png_size(Path::new(still["png"].as_str().unwrap()));
    let pixels = w as f64 * h as f64;
    assert!(pixels < 50_500_000.0, "{w} x {h}");
    assert!(pixels > 40_000_000.0, "only as much smaller as it needs: {w} x {h}");
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

/// A stop that lands while the worker still bundles (it does not know the render yet, and
/// answers that nothing was stopped) is kept, and the render ends stopped, not done.
#[test]
fn a_render_stopped_while_it_bundles_is_stopped() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/BUNDLING"), "").unwrap();
    let render = ok(&e, "video.render", json!({"video_id": id}))["id"].as_i64().unwrap();
    wait_until("it to start", || ok(&e, "video.render.get", json!({"render_id": render}))["state"] == "rendering");
    ok(&e, "video.render.cancel", json!({"render_id": render}));
    wait_until("it to end", || ok(&e, "video.render.get", json!({"render_id": render}))["state"] != "rendering");
    let row = ok(&e, "video.render.get", json!({"render_id": render}));
    assert_eq!(row["state"], "cancelled", "{row}");
    assert!(!dir.join("out/main-1.mp4").exists());
}

#[test]
fn renders_left_running_by_a_stopped_relay_are_failed_at_start() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    for state in ["rendering", "queued"] {
        e.store.lock().execute(
            "INSERT INTO video_renders(video_id, project_id, composition, format, quality, state, version, actor, created_at) VALUES (?1, 1, 'main', 'mp4', 'standard', ?2, 0, 'user', '2026-10-21T00:00:00Z')",
            params![id, state],
        ).unwrap();
    }
    fs::create_dir_all(dir.join("out")).unwrap();
    fs::write(dir.join("out/.1.partial.main-1.mp4"), "half").unwrap();
    relay_core::video::jobs::reconcile(&e);
    let render = ok(&e, "video.render.get", json!({"render_id": 1}));
    assert_eq!((render["state"].as_str(), render["error"].as_str()), (Some("failed"), Some("Relay stopped before it finished")));
    assert!(!dir.join("out/.1.partial.main-1.mp4").exists(), "what it was drawing is removed");
    // The one queued never started: it runs now.
    wait_until("the queued one to run", || ok(&e, "video.render.get", json!({"render_id": 2}))["state"] == "done");
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
fn assets_are_copied_in_and_never_written_over_unasked() {
    let (e, root) = project();
    let (id, dir) = create(&e);
    let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0, 0, 0];
    let first = root.path().join("Logo Final.png");
    fs::write(&first, png).unwrap();
    let added = ok(&e, "video.asset.add", json!({"video_id": id, "path": first}))["assets"][0].clone();
    assert_eq!((added["static_file"].as_str(), added["kind"].as_str()), (Some("assets/Logo-Final.png"), Some("image")));
    assert_eq!((added["w"].as_u64(), added["h"].as_u64()), (Some(2), Some(3)));
    assert!(dir.join("public/assets/Logo-Final.png").is_file());
    // The same file again is the same asset, and says nothing was written.
    let again = ok(&e, "video.asset.add", json!({"video_id": id, "path": first}))["assets"][0].clone();
    assert_eq!((again["static_file"].as_str(), again["unchanged"].as_bool()), (Some("assets/Logo-Final.png"), Some(true)));
    // Another under the same name gets a number, and the answer says it was renamed.
    let other = root.path().join("other.png");
    fs::write(&other, [png, b"more"].concat()).unwrap();
    let renamed = ok(&e, "video.asset.add", json!({"video_id": id, "path": other, "name": "Logo Final.png"}))["assets"][0].clone();
    assert_eq!(renamed["static_file"], "assets/Logo-Final-2.png");
    assert_eq!(renamed["renamed_from"], "assets/Logo-Final.png");
    // overwrite replaces it instead: a clip recorded again keeps its name.
    let replaced = ok(&e, "video.asset.add", json!({"video_id": id, "path": other, "name": "Logo Final.png", "overwrite": true}))["assets"][0].clone();
    assert_eq!((replaced["static_file"].as_str(), replaced["replaced"].as_bool()), (Some("assets/Logo-Final.png"), Some(true)));
    assert_eq!(fs::read(dir.join("public/assets/Logo-Final.png")).unwrap(), [png, b"more"].concat());
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "path": "relative.png"}))), "video.asset");
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "path": root.path().join("missing.mp3")}))), "video.asset");
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id}))), "video.asset");
}

#[test]
fn many_assets_go_in_one_call_and_a_large_one_can_be_linked() {
    let (e, root) = project();
    let (id, dir) = create(&e);
    let clips = root.path().join("clips");
    fs::create_dir_all(&clips).unwrap();
    let names = ["tap.wav", "pop.wav", "intro.mp4"];
    for (i, name) in names.iter().enumerate() {
        fs::write(clips.join(name), vec![i as u8; 64]).unwrap();
    }
    let paths: Vec<PathBuf> = names.iter().map(|n| clips.join(n)).collect();
    let added = ok(&e, "video.asset.add", json!({"video_id": id, "paths": paths}));
    let files: Vec<&str> = added["assets"].as_array().unwrap().iter().map(|a| a["static_file"].as_str().unwrap()).collect();
    assert_eq!(files, ["assets/tap.wav", "assets/pop.wav", "assets/intro.mp4"], "in the order asked");
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "paths": paths, "name": "x"}))), "video.asset");
    // A link is no copy: it points at the file where it is.
    let big = root.path().join("recording.mp4");
    fs::write(&big, vec![7u8; 4096]).unwrap();
    let linked = ok(&e, "video.asset.add", json!({"video_id": id, "path": big, "link": true}))["assets"][0].clone();
    assert_eq!(linked["static_file"], "assets/recording.mp4");
    assert_eq!(linked["linked"].as_str().map(PathBuf::from), Some(fs::canonicalize(&big).unwrap()));
    assert!(fs::symlink_metadata(dir.join("public/assets/recording.mp4")).unwrap().file_type().is_symlink());
    let listed = ok(&e, "video.asset.list", json!({"video_id": id}));
    let listed: Vec<(&str, &str)> = listed["assets"].as_array().unwrap().iter().map(|a| (a["static_file"].as_str().unwrap(), a["kind"].as_str().unwrap())).collect();
    assert_eq!(listed, [("assets/intro.mp4", "video"), ("assets/pop.wav", "audio"), ("assets/recording.mp4", "video"), ("assets/tap.wav", "audio")]);
}

#[test]
fn a_render_can_be_named_and_waited_for_in_the_same_call() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));
    let done = ok(&e, "video.render", json!({"video_id": id, "name": "avex 1.0 voiceover", "wait": true}));
    assert_eq!(done["state"], "done", "wait answers once it has finished: {done}");
    assert_eq!(done["name"], "avex-1.0-voiceover");
    assert_eq!(PathBuf::from(done["path"].as_str().unwrap()), dir.join("out/avex-1.0-voiceover.mp4"));
    // Nothing but the file is left in out/: the partial it was drawn to is gone.
    let out: Vec<String> = fs::read_dir(dir.join("out")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(out, ["avex-1.0-voiceover.mp4"]);
    // The same name renders over the file.
    let again = ok(&e, "video.render", json!({"video_id": id, "name": "avex-1.0-voiceover", "props": {"title": "Second"}, "wait": true}));
    assert!(fs::read_to_string(again["path"].as_str().unwrap()).unwrap().contains("Second"));
    assert_eq!(code(call(&e, "video.render", json!({"video_id": id, "name": "///"}))), "video.name");
    assert_eq!(code(call(&e, "video.render", json!({"video_id": id, "format": "gif", "preset": "youtube"}))), "video.loudness");
}

#[test]
fn a_wait_on_a_render_that_already_finished_answers_at_once() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, _) = create(&e);
    let render_id = ok(&e, "video.render", json!({"video_id": id}))["id"].as_i64().unwrap();
    wait_until("the render to finish", || ok(&e, "video.render.get", json!({"render_id": render_id}))["state"] == "done");
    // The race the first test hit: the render finished before anyone waited. The wait reads the
    // row, so it answers now rather than at its timeout.
    let started = std::time::Instant::now();
    let waited = ok(&e, "video.render.wait", json!({"render_ids": [render_id], "timeout_ms": 60000}));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
    assert_eq!((waited["renders"][0]["state"].as_str(), waited.get("timed_out")), (Some("done"), None));
}

#[test]
fn a_batch_queues_every_render_in_order_and_waits_for_them_all() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));
    let batch = ok(&e, "video.render.batch", json!({"video_id": id, "wait": true, "jobs": [
        {"name": "promo"}, {"name": "promo", "format": "wav"}, {"composition": "wide", "name": "promo-wide"},
    ]}));
    let renders = batch["renders"].as_array().unwrap();
    assert_eq!(renders.len(), 3);
    assert!(renders.iter().all(|r| r["state"] == "done"), "{batch}");
    let paths: Vec<PathBuf> = renders.iter().map(|r| PathBuf::from(r["path"].as_str().unwrap())).collect();
    assert_eq!(paths, [dir.join("out/promo.mp4"), dir.join("out/promo.wav"), dir.join("out/promo-wide.mp4")]);
    assert!(renders[0]["id"].as_i64() < renders[1]["id"].as_i64(), "in the order asked");
    assert_eq!(code(call(&e, "video.render.batch", json!({"video_id": id, "jobs": []}))), "video.batch");
}

#[test]
fn setting_up_a_kit_that_is_ready_answers_ready() {
    let (e, _root) = project();
    with_worker(&e);
    let status = ok(&e, "video.kit.install", json!({}));
    assert_eq!(status["state"], "ready", "nothing to set up, and the answer says so: {status}");
    // Set up again by force while a render runs: refused, as it would stop the render.
    let (id, dir) = create(&e);
    fs::write(dir.join("src/SLOW"), "").unwrap();
    let render = ok(&e, "video.render", json!({"video_id": id}))["id"].as_i64().unwrap();
    assert_eq!(code(call(&e, "video.kit.install", json!({"force": true}))), "video.kit.busy");
    ok(&e, "video.render.cancel", json!({"render_id": render}));
    wait_until("it to stop", || ok(&e, "video.render.get", json!({"render_id": render}))["state"] == "cancelled");
}

#[test]
fn a_deleted_video_is_listed_only_when_asked_and_comes_back() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    ok(&e, "video.check", json!({"video_id": id}));
    ok(&e, "video.delete", json!({"video_id": id}));
    // The running worker lets go of what it kept for it.
    let log_path = e.store.videos_dir().join(".relay/worker.log");
    wait_until("the worker to forget it", || fs::read_to_string(&log_path).is_ok_and(|l| l.contains(&format!("forgot {}", dir.display()))));
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

#[test]
fn the_timeline_lists_scenes_and_sounds_on_the_grid_and_what_the_sound_gets_wrong() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/SOUND"), "").unwrap();
    let t = ok(&e, "video.timeline", json!({"video_id": id}));
    let scenes: Vec<(&str, u64, u64, u64)> = t["scenes"].as_array().unwrap().iter()
        .map(|s| (s["name"].as_str().unwrap(), s["start"].as_u64().unwrap(), s["frames"].as_u64().unwrap(), s["depth"].as_u64().unwrap())).collect();
    assert_eq!(scenes, [("Hook", 0, 45, 0), ("Feature", 45, 45, 0), ("Card", 55, 20, 1)], "{t}");
    // The cut to Feature is on beat 3 of bar 1 (15 frames a beat at 120 BPM and 30 fps).
    assert_eq!((t["scenes"][1]["on_beat"].as_bool(), t["scenes"][1]["beat"].as_str()), (Some(true), Some("bar 1 · beat 4")));
    let sounds: Vec<(&str, &str, u64, u64)> = t["sounds"].as_array().unwrap().iter()
        .map(|s| (s["kind"].as_str().unwrap(), s["name"].as_str().unwrap(), s["start"].as_u64().unwrap(), s["end"].as_u64().unwrap())).collect();
    assert_eq!(sounds, [("music", "music", 0, 90), ("sfx", "tap", 20, 26), ("voice", "vo-1", 30, 60)], "{t}");
    assert_eq!(t["sounds"][1]["file"], "assets/sfx-tap.wav");
    assert_eq!((t["grid"]["bpm"].as_f64(), t["grid"]["from"].as_str()), (Some(120.0), Some("code")));
    let kinds: Vec<&str> = t["issues"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"off_beat") && kinds.contains(&"no_duck"), "{}", t["issues"]);
    let off = t["issues"].as_array().unwrap().iter().find(|i| i["kind"] == "off_beat").unwrap();
    assert_eq!((off["id"].as_str(), off["frames"].clone()), (Some("tap"), json!([20])));
    let png = PathBuf::from(t["png"].as_str().unwrap());
    assert!(png.is_file());
    // A grid given by the call is checked instead of the code's.
    let given = ok(&e, "video.timeline", json!({"video_id": id, "bpm": 90, "picture": false}));
    assert_eq!((given["grid"]["from"].as_str(), given.get("png")), (Some("given"), None));
    // Where ffmpeg can read the mix, it is measured.
    if which::which("ffmpeg").is_ok() {
        let lufs = t["mix"]["lufs"].as_f64().unwrap_or_else(|| panic!("{t}"));
        assert!((-15.0..-8.0).contains(&lufs), "a 0.3 sine: {lufs}");
    }
}

#[test]
fn a_scan_checks_the_sound_too() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/SOUND"), "").unwrap();
    let scanned = ok(&e, "video.scan", json!({"video_id": id}));
    let kinds: Vec<&str> = scanned["issues"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"off_beat") && kinds.contains(&"no_duck"), "{kinds:?}");
}

#[test]
fn a_still_can_show_its_sound_under_it() {
    let (e, _root) = project();
    with_worker(&e);
    let (id, dir) = create(&e);
    fs::write(dir.join("src/SOUND"), "").unwrap();
    let height = |still: &Value| {
        let decoded = png_size(Path::new(still["png"].as_str().unwrap()));
        decoded.1
    };
    let plain = ok(&e, "video.still", json!({"video_id": id, "count": 3}));
    let heard = ok(&e, "video.still", json!({"video_id": id, "count": 3, "sound": true}));
    assert_eq!(height(&heard), height(&plain) + 150, "the strip goes under the sheet");
    let one = ok(&e, "video.still", json!({"video_id": id, "at": ["1s"], "sound": true}));
    assert!(height(&one) > 150);
}

/// A PNG's width and height, from its header.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap();
    (u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap()))
}

/// A stand-in for ElevenLabs: two voices, a line said with timings, a sound, music and a
/// transcript that hears a name wrong; every call recorded.
fn with_elevenlabs(e: &Engine) -> Arc<std::sync::Mutex<Vec<relay_core::video::eleven::Call>>> {
    use relay_core::video::eleven::{Answer, Body, Call};
    let calls: Arc<std::sync::Mutex<Vec<Call>>> = Arc::default();
    let seen = calls.clone();
    e.video.use_elevenlabs(Arc::new(move |call: &Call| {
        seen.lock().unwrap().push(call.clone());
        let ok = |body: Vec<u8>| Answer { status: 200, body };
        let mp3 = b"ID3fake-mp3".to_vec();
        match (call.method, call.path.as_str()) {
            ("GET", "/v1/user/subscription") => ok(json!({"tier": "creator", "character_count": 1000, "character_limit": 100000}).to_string().into_bytes()),
            ("GET", "/v1/voices") => ok(json!({"voices": [
                {"voice_id": "iP95p4xoKVk53GoZ742B", "name": "Chris", "category": "premade", "labels": {"accent": "american"}, "description": "Natural and real"},
                {"voice_id": "bIHbv24MWmeRgasZH58o", "name": "Will", "category": "premade"},
            ]}).to_string().into_bytes()),
            ("POST", path) if path.ends_with("/with-timestamps") => {
                let Body::Json(body) = &call.body else { panic!("{call:?}") };
                let text = body["text"].as_str().unwrap();
                let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
                let starts: Vec<f64> = (0..chars.len()).map(|i| i as f64 * 0.05).collect();
                let ends: Vec<f64> = starts.iter().map(|s| s + 0.05).collect();
                use base64::Engine as _;
                ok(json!({"audio_base64": base64::engine::general_purpose::STANDARD.encode(&mp3),
                    "alignment": {"characters": chars, "character_start_times_seconds": starts, "character_end_times_seconds": ends}}).to_string().into_bytes())
            }
            ("POST", "/v1/sound-generation") | ("POST", "/v1/music") => ok(mp3),
            ("POST", "/v1/speech-to-text") => ok(json!({"language_code": "en", "language_probability": 0.99, "text": "Meet Avox.",
                "words": [{"text": "Meet", "type": "word", "start": 0.0, "end": 0.3}, {"text": " ", "type": "spacing", "start": 0.3, "end": 0.35},
                          {"text": "Avox.", "type": "word", "start": 0.35, "end": 0.9}]}).to_string().into_bytes()),
            _ => Answer { status: 404, body: br#"{"detail":"not found"}"#.to_vec() },
        }
    }));
    calls
}

#[test]
fn voice_lines_sounds_and_music_come_from_elevenlabs_with_how_they_were_made() {
    let (e, _root) = project();
    let calls = with_elevenlabs(&e);
    let (id, dir) = create(&e);
    // No key yet: refused, saying to ask the person, never for the key itself.
    let none = call(&e, "video.voice", json!({"video_id": id, "text": "Hi", "voice": "Chris"}));
    assert_eq!(code(none), "video.elevenlabs.no_key");
    assert_eq!(ok(&e, "video.elevenlabs.status", json!({}))["saved"], false);
    // The person saves it: checked, kept beside the test store, never answered back.
    let saved = ok(&e, "video.elevenlabs.key_set", json!({"key": "sk_test_123"}));
    assert_eq!((saved["saved"].as_bool(), saved["tier"].as_str(), saved["characters_left"].as_i64()), (Some(true), Some("creator"), Some(99000)));
    assert!(!saved.to_string().contains("sk_test_123"));
    let (agent_name, agent_actor) = agent(&e);
    let _ = agent_name;
    assert_eq!(code(call_as(&e, agent_actor.clone(), "video.elevenlabs.key_set", json!({"key": "x"}))), "actor.allowlist", "only the person saves a key");

    let line = ok(&e, "video.voice", json!({"video_id": id, "text": "Meet Avex.", "voice": "chris", "say": {"Avex": "Ay-vex"}}));
    assert_eq!((line["voice"].as_str(), line["voice_id"].as_str()), (Some("Chris"), Some("iP95p4xoKVk53GoZ742B")));
    assert_eq!(line["asset"]["static_file"], "assets/vo-meet-avex.mp3");
    let words: Vec<&str> = line["words"].as_array().unwrap().iter().map(|w| w["text"].as_str().unwrap()).collect();
    assert_eq!(words, ["Meet", "Avex."], "timed as written, said as Ay-vex");
    assert_eq!(line["captions"][1]["text"], " Avex.");
    let sent = calls.lock().unwrap().iter().find(|c| c.path.ends_with("/with-timestamps")).cloned().unwrap();
    let relay_core::video::eleven::Body::Json(body) = &sent.body else { panic!() };
    assert_eq!(body["text"], "Meet Ay-vex.");
    // How it was made is kept beside it, and listed with it.
    let made: Value = serde_json::from_slice(&fs::read(dir.join("public/assets/vo-meet-avex.mp3.json")).unwrap()).unwrap();
    assert_eq!((made["voice"].as_str(), made["text"].as_str(), made["said"].as_str()), (Some("Chris"), Some("Meet Avex."), Some("Meet Ay-vex.")));
    let listed = ok(&e, "video.asset.list", json!({"video_id": id}));
    assert_eq!(listed["assets"][0]["made"]["made_by"], "video.voice");

    let sfx = ok(&e, "video.sfx", json!({"video_id": id, "prompt": "a soft UI tap", "seconds": 0.5}));
    assert_eq!(sfx["asset"]["static_file"], "assets/sfx-a-soft-ui-tap.mp3");
    let music = ok(&e, "video.music", json!({"video_id": id, "prompt": "warm lo-fi, 120 BPM", "seconds": 30}));
    assert_eq!(music["asset"]["made"]["prompt"], "warm lo-fi, 120 BPM");
    assert_eq!(code(call(&e, "video.music", json!({"video_id": id, "prompt": "x", "seconds": 1}))), "video.music");

    // Hearing it back finds the name said wrong.
    let heard = ok(&e, "video.transcribe", json!({"video_id": id, "asset": "assets/vo-meet-avex.mp3", "expect": "Meet Avex."}));
    assert_eq!(heard["misheard"], json!([{"expected": "Avex.", "heard": "Avox.", "at": 0.35}]));
    let voices = ok(&e, "video.voices", json!({"search": "american"}));
    assert_eq!(voices["voices"].as_array().unwrap().len(), 1);
    assert_eq!(ok(&e, "video.elevenlabs.key_remove", json!({}))["saved"], false);
    assert_eq!(code(call(&e, "video.sfx", json!({"video_id": id, "prompt": "boing"}))), "video.elevenlabs.no_key");
}

/// A transcription sends the sound alone, small: ten seconds of stereo WAV go as a mono MP3.
/// The keyring's "no key" is its answer for a minute, so every agent call does not ask it again
/// (a locked one raises a prompt each time); saving a key asks again at once.
#[test]
fn a_missing_elevenlabs_key_is_not_looked_up_on_every_call() {
    let (e, root) = project();
    let (id, _) = create(&e);
    with_elevenlabs(&e);
    assert_eq!(code(call(&e, "video.sfx", json!({"video_id": id, "prompt": "tap"}))), "video.elevenlabs.no_key");
    // Written behind Relay's back: not seen until the minute is up.
    fs::write(root.path().join("store").join(relay_core::video::eleven::TEST_FILE), "sk_behind").unwrap();
    assert_eq!(code(call(&e, "video.sfx", json!({"video_id": id, "prompt": "tap"}))), "video.elevenlabs.no_key");
    ok(&e, "video.elevenlabs.key_set", json!({"key": "sk_test_123"}));
    ok(&e, "video.sfx", json!({"video_id": id, "prompt": "tap"}));
}

#[test]
fn a_transcription_sends_only_the_sound() {
    if which::which("ffmpeg").is_err() {
        eprintln!("no ffmpeg: skipped");
        return;
    }
    let (e, _root) = project();
    let (id, dir) = create(&e);
    let calls = with_elevenlabs(&e);
    ok(&e, "video.elevenlabs.key_set", json!({"key": "sk_test_123"}));
    fs::create_dir_all(dir.join("public/assets")).unwrap();
    let wav = dir.join("public/assets/talk.wav");
    let made = std::process::Command::new("ffmpeg").args(["-v", "error", "-f", "lavfi", "-i", "sine=frequency=300:duration=10", "-ac", "2", "-ar", "48000"])
        .arg(&wav).status().unwrap();
    assert!(made.success());
    ok(&e, "video.transcribe", json!({"video_id": id, "asset": "assets/talk.wav"}));
    let sent = calls.lock().unwrap().iter().find(|c| c.path == "/v1/speech-to-text").cloned().unwrap();
    let relay_core::video::eleven::Body::Form { file, .. } = &sent.body else { panic!("{sent:?}") };
    assert_eq!(file.0, "talk.mp3");
    assert!((file.1.len() as u64) < fs::metadata(&wav).unwrap().len() / 10, "{} bytes sent", file.1.len());
}

#[test]
fn an_android_capture_holds_the_device_records_the_steps_and_adds_an_even_clip() {
    if which::which("ffmpeg").is_err() {
        eprintln!("no ffmpeg: skipped");
        return;
    }
    let (e, root) = project();
    let (id, _) = create(&e);
    // A clip the fake device hands back: two seconds that change at one second.
    let clip = root.path().join("screen.mp4");
    let made = std::process::Command::new("ffmpeg").args(["-v", "error", "-f", "lavfi", "-i", "color=c=gray:s=180x400:d=1:r=30", "-f", "lavfi", "-i",
        "color=c=white:s=180x400:d=1:r=30", "-filter_complex", "[0][1]concat=n=2:v=1", "-c:v", "libx264", "-pix_fmt", "yuv420p"]).arg(&clip).status().unwrap();
    assert!(made.success());
    let log = root.path().join("adb.log");
    std::env::set_var("FAKE_ADB_LOG", &log);
    std::env::set_var("FAKE_ADB_CLIP", &clip);
    let adb = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/video/fake_adb.py");
    e.store.lock().execute("INSERT INTO settings(path, value, updated_at) VALUES ('device.adb_path', ?1, 'now')", [json!(adb.display().to_string()).to_string()]).unwrap();
    let mut rx = e.subscribe();
    let captured = ok(&e, "video.capture.android", json!({"video_id": id, "launch": "com.quietsoftware.avex/.MainActivity", "name": "home",
        "steps": [{"tap": [540, 1200]}, {"wait": 0.2}, {"swipe": [540, 1800, 540, 600], "ms": 250}, {"key": "BACK"}], "tail": 0.2}));
    assert_eq!((captured["asset"]["static_file"].as_str(), captured["device"].as_str(), captured["fps"].as_f64()), (Some("assets/home.mp4"), Some("emulator-5554"), Some(30.0)));
    let did: Vec<&str> = captured["steps"].as_array().unwrap().iter().map(|s| s["did"].as_str().unwrap()).collect();
    assert_eq!(did, ["tap 540 1200", "wait 0.2 s", "swipe 540 1800 to 540 600", "key BACK"]);
    assert!(captured["motion"].as_array().unwrap().iter().any(|m| m["kind"] == "cut" && m["frame"] == 30), "{}", captured["motion"]);
    let sent = fs::read_to_string(&log).unwrap();
    for wanted in ["shell am broadcast -a com.android.systemui.demo -e command enter", "shell am start -n com.quietsoftware.avex/.MainActivity",
        "shell input tap 540 1200", "shell input swipe 540 1800 540 600 250", "shell input keyevent 4", "shell pkill -INT screenrecord",
        "pull /sdcard/relay-capture.mp4", "-e command exit"] {
        assert!(sent.contains(wanted), "{wanted} not sent:\n{sent}");
    }
    // The device was held for the capture, and let go.
    let events: Vec<String> = drain(&mut rx).into_iter().map(|ev| ev.ev).collect();
    assert!(events.contains(&"device.lease.acquired".to_string()) && events.contains(&"device.lease.released".to_string()), "{events:?}");
    // A claim the person holds already is theirs before and after: the capture rides on it.
    ok(&e, "device.claim", json!({"device": "emulator-5554", "action": "testing the release", "minutes": 30}));
    std::env::set_var("FAKE_ADB_LOG", root.path().join("adb2.log"));
    ok(&e, "video.capture.android", json!({"video_id": id, "steps": [{"wait": 0.1}], "tail": 0.1, "name": "again", "demo": false}));
    let held = ok(&e, "device.leases", json!({}));
    assert!(held.to_string().contains("testing the release"), "the claim was kept as it was: {held}");
    let inspected = ok(&e, "video.asset.inspect", json!({"video_id": id, "asset": "assets/home.mp4"}));
    assert_eq!((inspected["fps"].as_f64(), inspected["frames"].as_u64(), inspected.get("variable_rate")), (Some(30.0), Some(60), None), "{inspected}");
}

/// Sound longer than a pipe's 64 MiB (5 min 50 s of stereo at 48 kHz) is read whole: a render's
/// loudness preset, a timeline's mix and an asset's inspection all decode through it. The store is
/// on disk under the target folder, the decode being about 70 MB.
#[test]
fn sound_past_six_minutes_is_read_whole() {
    if which::which("ffmpeg").is_err() {
        eprintln!("no ffmpeg: skipped");
        return;
    }
    let root = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let (ws, repo) = (root.path().join("ws"), root.path().join("ws/app"));
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let e = engine_with_project(root.path(), &ws, &repo);
    let (id, dir) = create(&e);
    fs::create_dir_all(dir.join("public/assets")).unwrap();
    let made = std::process::Command::new("ffmpeg").args(["-v", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=400", "-c:a", "libopus", "-b:a", "24k"])
        .arg(dir.join("public/assets/long.ogg")).status().unwrap();
    assert!(made.success());
    let inspected = ok(&e, "video.asset.inspect", json!({"video_id": id, "asset": "assets/long.ogg"}));
    let seconds = inspected["audio"]["seconds"].as_f64().unwrap_or_default();
    assert!((seconds - 400.0).abs() < 1.0, "read {seconds} s of 400: {inspected}");
    let scratch = fs::read_dir(e.store.videos_dir().join(".relay/sound")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(scratch, 0, "the decode's file is removed");
}

#[test]
fn a_preview_runs_one_at_a_time_and_answers_its_link() {
    let (e, _root) = project();
    // A plain server stands in for Studio.
    e.video.preview.use_program(vec!["python3".into(), "-m".into(), "http.server".into(), "--bind".into(), "127.0.0.1".into(), "{port}".into()]);
    let (first, _) = create(&e);
    let opened = ok(&e, "video.preview", json!({"video_id": first}));
    let url = opened["url"].as_str().unwrap().to_string();
    assert!(url.starts_with("http://127.0.0.1:") && url.ends_with("/main"), "{url}");
    assert_eq!((opened["running"].as_bool(), opened["started"].as_bool()), (Some(true), Some(true)));
    // Asked again, the same one.
    let again = ok(&e, "video.preview", json!({"video_id": first, "composition": "wide"}));
    assert_eq!((again.get("started"), again["url"].as_str().map(|u| u.ends_with("/wide"))), (None, Some(true)));
    // Another video's preview takes its place.
    let second = ok(&e, "video.create", json!({"project_id": 1, "title": "Second"}))["video"]["id"].as_i64().unwrap();
    let other = ok(&e, "video.preview", json!({"video_id": second}));
    assert_ne!(other["url"], opened["url"]);
    assert_eq!(relay_core::video::preview::running(&e, first), None);
    assert_eq!(ok(&e, "video.preview", json!({"video_id": second, "stop": true}))["running"], false);
    assert_eq!(relay_core::video::preview::running(&e, second), None);
}

#[test]
fn an_installed_font_is_added_by_its_family() {
    let Ok(listed) = std::process::Command::new("fc-list").args([":", "family"]).output() else {
        eprintln!("no fontconfig: skipped");
        return;
    };
    let families = String::from_utf8_lossy(&listed.stdout).to_string();
    let Some(family) = ["DejaVu Sans", "Noto Serif", "Liberation Serif"].into_iter().find(|f| families.lines().any(|l| l.split(',').any(|n| n == *f))) else {
        eprintln!("none of the families looked for is installed: skipped");
        return;
    };
    let (e, _root) = project();
    let (id, dir) = create(&e);
    let added = ok(&e, "video.asset.add", json!({"video_id": id, "font": family, "weights": [400, 700]}));
    let font = &added["font"];
    assert_eq!(font["family"], family);
    assert!(!added["assets"].as_array().unwrap().is_empty(), "{added}");
    for asset in added["assets"].as_array().unwrap() {
        let file = asset["static_file"].as_str().unwrap();
        assert!(file.starts_with("assets/fonts/") && dir.join("public").join(file).is_file(), "{asset}");
        assert!(font["load"].as_str().unwrap().contains(file), "the loadFonts call names {file}: {}", font["load"]);
    }
    assert_eq!(code(call(&e, "video.asset.add", json!({"video_id": id, "font": "No Such Family Anywhere"}))), "video.font");
}
