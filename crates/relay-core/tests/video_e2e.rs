//! `video.*` end to end with nothing faked: the kit set up for real (npm, Remotion's browser), the
//! real worker, the starter checked, looked at and rendered. It needs Node.js 20+ and the network
//! the first time, so it is ignored unless asked for:
//!
//! `cargo test -p relay-core --test all video_e2e -- --ignored --nocapture`
//!
//! The store and the kit are kept under the workspace's `target/video-e2e/` (on disk, about
//! 600 MB, set up once), so a second run takes seconds.

mod common;

use common::{call, code, committed_repo, ok, refused};
use relay_core::engine::Engine;
use relay_core::store::Store;
use relay_core::Instance;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/video-e2e")
}

/// An engine on the kept store, with project 1 and the Video plugin on.
fn engine() -> Arc<Engine> {
    let root = root();
    let repo = root.join("ws/app");
    if !repo.join(".git").exists() {
        committed_repo(&repo, &[("README.md", "# app\n")]);
    }
    let engine = Engine::new(Instance::Test, Store::open(&root.join("store/store.db"), false).unwrap());
    if ok(&engine, "project.list", json!({}))["projects"].as_array().is_none_or(|p| p.is_empty()) {
        ok(&engine, "workspace.create", json!({"path": root.join("ws")}));
        ok(&engine, "project.add", json!({"workspace_id": 1, "path": repo}));
    }
    ok(&engine, "plugin.enable", json!({"plugin_id": "video", "project_id": 1, "enabled": true}));
    engine
}

fn wait(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + within;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn timed(what: &str, f: impl FnOnce() -> Value) -> Value {
    let started = Instant::now();
    let out = f();
    println!("{what}: {} ms", started.elapsed().as_millis());
    out
}

#[test]
#[ignore = "needs Node.js 20+ and, the first time, the network: run with --ignored"]
fn video_e2e_the_starter_checks_looks_and_renders_for_real() {
    let e = engine();
    let status = ok(&e, "video.kit.status", json!({}));
    println!("kit: {status}");
    if status["state"] != "ready" {
        ok(&e, "video.kit.install", json!({}));
        wait("the kit", Duration::from_secs(900), || ok(&e, "video.kit.status", json!({}))["state"] != "installing");
        let status = ok(&e, "video.kit.status", json!({}));
        assert_eq!(status["state"], "ready", "{status}");
    }

    let made = ok(&e, "video.create", json!({"project_id": 1, "title": "Launch promo", "seconds": 4}));
    let (id, dir) = (made["video"]["id"].as_i64().unwrap(), PathBuf::from(made["video"]["dir"].as_str().unwrap()));
    println!("video {id} in {}", dir.display());

    let checked = timed("first check", || ok(&e, "video.check", json!({"video_id": id})));
    assert_eq!(checked["ok"], true, "{checked}");
    assert_eq!(checked["errors"], json!([]), "the starter typechecks: {checked}");
    let main = &checked["compositions"][0];
    assert_eq!((main["id"].as_str(), main["duration_frames"].as_u64()), (Some("main"), Some(120)));
    assert_eq!(main["props_schema"]["properties"]["title"]["description"], "Headline");
    timed("second check", || ok(&e, "video.check", json!({"video_id": id})));

    let sheet = timed("sheet of 6", || ok(&e, "video.still", json!({"video_id": id, "count": 6})));
    let png = PathBuf::from(sheet["png"].as_str().unwrap());
    assert!(png.is_file());
    println!("sheet: {} ({} bytes)", png.display(), fs::metadata(&png).unwrap().len());
    println!("issues: {}", sheet["issues"]);
    // The starter is laid out inside the safe area, in fonts that are there.
    for issue in sheet["issues"].as_array().unwrap() {
        assert!(!["unsafe", "font_fallback", "clipped", "broken_image", "console"].contains(&issue["kind"].as_str().unwrap()), "{issue}");
    }
    let one = timed("one frame with boxes", || ok(&e, "video.still", json!({"video_id": id, "at": ["1.5s"], "boxes": true})));
    let ids: Vec<&str> = one["boxes"].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"title") && ids.contains(&"subtitle"), "{ids:?}");

    // A text in a font that is not there, pushed under the platform's buttons, is found.
    let video = dir.join("src/Video.tsx");
    let original = fs::read_to_string(&video).unwrap();
    fs::write(&video, original.replace(
        "        <div data-id=\"subtitle\" style={{",
        "        <div data-id=\"legal\" style={{ position: 'absolute', bottom: 40, left: 40, fontFamily: 'No Such Font', fontSize: 18, color: '#20242c' }}>Terms apply</div>\n        <div data-id=\"subtitle\" style={{",
    )).unwrap();
    let found = timed("still after an edit", || ok(&e, "video.still", json!({"video_id": id, "at": ["1.5s"]})));
    let kinds: Vec<&str> = found["issues"].as_array().unwrap().iter().filter(|i| i["id"] == "legal").map(|i| i["kind"].as_str().unwrap()).collect();
    println!("legal: {kinds:?}");
    for kind in ["unsafe", "small_text", "font_fallback", "contrast"] {
        assert!(kinds.contains(&kind), "{kind} not found in {}", found["issues"]);
    }
    assert_eq!(found["version"], 2, "the edit was recorded");

    // A typo is answered at its line, and a frame that throws at its file.
    fs::write(&video, original.replace("{subtitle}\n", "{subtitle\n")).unwrap();
    let broken = ok(&e, "video.check", json!({"video_id": id}));
    assert_eq!(broken["ok"], false);
    println!("broken: {}", broken["errors"]);
    assert_eq!(broken["errors"][0]["file"], "src/Video.tsx");
    assert_eq!(code(call(&e, "video.still", json!({"video_id": id}))), "video.build");
    fs::write(&video, original.replace("const sub = useSpring(16);", "const sub = useSpring(16);\n  if (useCurrentFrame() > 30) throw new Error('scene two broke');")
        .replace("import { AbsoluteFill, useVideoConfig } from 'remotion';", "import { AbsoluteFill, useCurrentFrame, useVideoConfig } from 'remotion';")).unwrap();
    let thrown = refused(call(&e, "video.still", json!({"video_id": id, "at": ["1.5s"]})));
    println!("thrown: {} {}", thrown.code, thrown.message);
    assert_eq!(thrown.code, "video.runtime");
    assert!(thrown.message.starts_with("src/Video.tsx:"), "{}", thrown.message);
    fs::write(&video, &original).unwrap();

    // Props the person set are drawn with.
    ok(&e, "video.update", json!({"video_id": id, "props": {"main": {"title": "Every euro in its place", "accent": "#F5A524"}}}));
    let rendered = ok(&e, "video.render", json!({"video_id": id, "quality": "preview"}));
    let render_id = rendered["id"].as_i64().unwrap();
    let started = Instant::now();
    wait("the render", Duration::from_secs(300), || {
        let r = ok(&e, "video.render.get", json!({"render_id": render_id}));
        ["done", "failed", "cancelled"].contains(&r["state"].as_str().unwrap())
    });
    let done = ok(&e, "video.render.get", json!({"render_id": render_id}));
    println!("render: {} ms, {done}", started.elapsed().as_millis());
    assert_eq!(done["state"], "done", "{done}");
    let file = PathBuf::from(done["path"].as_str().unwrap());
    assert!(file.starts_with(dir.join("out")) && fs::metadata(&file).unwrap().len() > 10_000);

    // A picture copied in is drawn from public/.
    let logo = root().join("logo.png");
    fs::copy(&png, &logo).unwrap();
    let asset = ok(&e, "video.asset.add", json!({"video_id": id, "path": logo}));
    assert_eq!(asset["kind"], "image");
    println!("versions: {}", ok(&e, "video.get", json!({"video_id": id}))["versions"]);
    ok(&e, "video.delete", json!({"video_id": id}));
}

/// The first check of a session as an agent meets it: the video made, some seconds spent writing
/// its code, an edit, then the check (docs/perf/VIDEO-2026-10-21.md). A fresh engine, so no worker
/// runs yet.
#[test]
#[ignore = "needs Node.js 20+ and the kit video_e2e sets up: run with --ignored"]
fn video_e2e_first_check_after_writing() {
    let e = engine();
    assert_eq!(ok(&e, "video.kit.status", json!({}))["state"], "ready", "run video_e2e_the_starter_checks_looks_and_renders_for_real first");
    let made = ok(&e, "video.create", json!({"project_id": 1, "title": "First check"}));
    let (id, dir) = (made["video"]["id"].as_i64().unwrap(), PathBuf::from(made["video"]["dir"].as_str().unwrap()));
    std::thread::sleep(Duration::from_secs(8));
    let video = dir.join("src/Video.tsx");
    fs::write(&video, fs::read_to_string(&video).unwrap().replace("gap: 36", "gap: 40")).unwrap();
    let checked = timed("first check after writing", || ok(&e, "video.check", json!({"video_id": id})));
    assert_eq!(checked["ok"], true, "{checked}");
    ok(&e, "video.delete", json!({"video_id": id}));
}
