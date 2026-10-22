//! `video.*` end to end with nothing faked: the kit set up for real (npm, Remotion's browser), the
//! real worker, the starter checked, looked at and rendered. It needs Node.js 20+ and the network
//! the first time, so it is ignored unless asked for:
//!
//! `cargo test -p relay-core --test all video_e2e -- --ignored --nocapture --test-threads=1`
//!
//! One at a time: they share one store.
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
    // The schema is kept for video.get and the props panel; a check's answer leaves it out.
    assert!(main.get("props_schema").is_none());
    let kept = &ok(&e, "video.get", json!({"video_id": id}))["compositions"][0];
    assert_eq!(kept["props_schema"]["properties"]["title"]["description"], "Headline");
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
    let scanned = timed("scan every 3", || ok(&e, "video.scan", json!({"video_id": id})));
    println!("scan: {} frames, issues {}", scanned["frames"], scanned["issues"]);
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
    fs::write(&video, original.replace("const sub = useSpring(4);", "const sub = useSpring(4);\n  if (useCurrentFrame() > 30) throw new Error('scene two broke');")
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
    relay_core::video::shutdown(&e);
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
    relay_core::video::shutdown(&e);
}

/// The kit's screens and charts drawn for real: a phone laid out at iPhone points with its status
/// bar and home indicator, a donut, bars, a tap and a counter. Typechecks clean and draws without
/// issues; the sheet is printed to look at.
#[test]
#[ignore = "needs Node.js 20+ and the kit video_e2e sets up: run with --ignored"]
fn video_e2e_the_kit_draws_screens_and_charts() {
    let e = engine();
    assert_eq!(ok(&e, "video.kit.status", json!({}))["state"], "ready", "run video_e2e_the_starter_checks_looks_and_renders_for_real first");
    let made = ok(&e, "video.create", json!({"project_id": 1, "title": "Kit", "seconds": 4}));
    let (id, dir) = (made["video"]["id"].as_i64().unwrap(), PathBuf::from(made["video"]["dir"].as_str().unwrap()));
    fs::write(dir.join("src/Video.tsx"), r##"import { Bars, Counter, Donut, Glow, Gradient, Phone, Tap, theme, Title, useSafePadding } from '@relay/video';
import { AbsoluteFill } from 'remotion';
import type { Props } from './Root';

const App: React.FC = () => (
  <AbsoluteFill style={{ background: '#F6F7F9', padding: '64px 20px 0', fontFamily: 'Inter', gap: 16 }}>
    <div data-id="app-title" style={{ fontSize: 28, fontWeight: 800, color: '#0B0D12' }}>October</div>
    <Donut id="donut" size={220} thickness={30} track="#E6E9EE" highlight={0}
      segments={[{ value: 32, color: '#34D399' }, { value: 24, color: '#60A5FA' }, { value: 18, color: '#F59E0B' }, { value: 26, color: '#A78BFA' }]}>
      <div data-id="donut-total" style={{ fontSize: 24, fontWeight: 800, color: '#0B0D12' }}>$2,486</div>
    </Donut>
    <Bars id="bars" values={[320, 410, 280, 520, 390]} labels={['M', 'T', 'W', 'T', 'F']} colors="#34D399" width={340} height={180} highlight={3} labelColor="#5B6472" />
  </AbsoluteFill>
);

export const Video: React.FC<Props> = ({ accent }) => {
  const padding = useSafePadding();
  return (
    <AbsoluteFill>
      <Gradient />
      <Glow color={accent} />
      <AbsoluteFill style={{ ...padding, alignItems: 'center', gap: 40 }}>
        <Title id="headline" size={84}>Every euro, seen</Title>
        <div style={{ position: 'relative' }}>
          <Phone id="phone" width={460} designWidth={390} statusBar="dark" homeIndicator><App /></Phone>
          <Tap x={230} y={520} at={50} />
        </div>
        <Counter id="saved" to={1240} prefix="$" size={theme.type.title} delay={10} />
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
"##).unwrap();
    let checked = ok(&e, "video.check", json!({"video_id": id}));
    assert_eq!((checked["ok"].as_bool(), checked["errors"].clone()), (Some(true), json!([])), "{checked}");
    assert!(checked["compositions"][0].get("props_schema").is_none(), "a check answers no schema");
    let sheet = timed("kit sheet", || ok(&e, "video.still", json!({"video_id": id, "at": ["0.5s", "1.8s", "3s"], "boxes": true})));
    println!("kit sheet: {}", sheet["png"]);
    println!("kit issues: {}", sheet["issues"]);
    for issue in sheet["issues"].as_array().unwrap() {
        assert!(!["font_fallback", "clipped", "broken_image", "console", "offscreen"].contains(&issue["kind"].as_str().unwrap()), "{issue}");
    }
    let ids: Vec<&str> = sheet["boxes"].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect();
    for wanted in ["phone", "donut", "bars", "app-title", "donut-total", "saved"] {
        assert!(ids.contains(&wanted), "{wanted} not in {ids:?}");
    }
    let scanned = timed("kit scan", || ok(&e, "video.scan", json!({"video_id": id})));
    println!("kit scan: {} frames, issues {}", scanned["frames"], scanned["issues"]);
    ok(&e, "video.delete", json!({"video_id": id}));
    relay_core::video::shutdown(&e);
}
