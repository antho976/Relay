//! The video kit (D167): Node.js packages — Remotion, React, TypeScript, the kit's fonts — and
//! Remotion's headless browser, set up once per instance in `videos/` beside the store, with the
//! files Relay itself writes there from the Video plugin's `runtime/` folder: the worker, the
//! `@relay/video` kit and the TypeScript settings every video extends.
//!
//! Setting up (`video.kit.install`) downloads about 600 MB from npm and Google's browser
//! storage, so it runs only when asked, on a thread of its own, and says how it went with
//! `video.kit.changed`. Relay's own files are written again whenever a build changes them; the
//! packages only when `runtime/package.json` (the pinned versions) changes, which marks the kit
//! outdated until it is set up again.

use super::Hub;
use crate::engine::Engine;
use relay_bus::error::BusError;
use relay_bus::ops::video::{KitProgress, KitState, KitStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// The Remotion the kit pins (`runtime/package.json`).
pub const REMOTION: &str = "4.0.534";
/// The oldest Node.js the kit runs on.
const NODE_MAJOR: u32 = 20;
const NPM_WITHIN: Duration = Duration::from_secs(15 * 60);
const BROWSER_WITHIN: Duration = Duration::from_secs(10 * 60);
/// How long "no Node.js" is believed before looking again.
const NO_NODE_FOR: Duration = Duration::from_secs(30);

const ABOUT: &str = "Setting up downloads Remotion, React, TypeScript and four open-licensed font families from npm \
    (about 350 MB) and Remotion's headless Chrome from Google (about 240 MB) into Relay's data folder, once. \
    Nothing is installed system-wide, and nothing is sent anywhere while videos render.";

/// About what each stage puts on disk: npm's packages, then the browser. Progress is read as how
/// much of it has landed, which is as near as npm lets anyone tell.
const PACKAGES_BYTES: u64 = 360 * 1024 * 1024;
const BROWSER_BYTES: u64 = 240 * 1024 * 1024;
/// How the setup's progress divides between the stages.
const PACKAGES_SHARE: f64 = 0.6;

/// The last look for Node: when, the version found (20 or newer), and the binary on the PATH
/// it asked (its real path, and when it last changed).
type NodeLook = (Instant, Option<String>, Option<(PathBuf, SystemTime)>);

/// What the setup thread and the status share.
#[derive(Default)]
pub struct State {
    pub(super) installing: std::sync::atomic::AtomicBool,
    error: Mutex<Option<String>>,
    /// When Node was last looked for, what was found, and the binary it asked (its path and
    /// when it changed), so an old one is not asked again until it is another.
    node: Mutex<Option<NodeLook>>,
    /// While installing: the stage, its start, and the latest progress reading.
    progress: Mutex<Option<(Instant, KitProgress)>>,
}

/// What an install that is just starting reports.
pub fn starting() -> KitProgress {
    KitProgress { stage: "packages".into(), progress: 0.0, detail: "starting: npm installs Remotion and its packages".into(), elapsed_s: 0 }
}

/// Bytes under `dir`, following nothing: how much of a stage has landed.
fn bytes_under(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = fs::read_dir(&at) else { continue };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

/// `videos/.relay/kit.json`: what was set up, written last.
#[derive(Debug, Serialize, Deserialize)]
struct Installed {
    digest: String,
    remotion: String,
    installed_at: String,
}

fn runtime_files() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    crate::plugins::get(super::PLUGIN)
        .map(|p| p.bundle.files)
        .unwrap_or(&[])
        .iter()
        .filter_map(|(path, bytes)| path.strip_prefix("runtime/").map(|rel| (rel, *bytes)))
}

/// The starter a new video is made from: `(path in the video's folder, contents)`.
pub fn starter() -> Vec<(&'static str, &'static [u8])> {
    runtime_files().filter_map(|(rel, bytes)| rel.strip_prefix("starter/").map(|r| (r, bytes))).collect()
}

/// Where a runtime file goes under the kit's root; `None` for one Relay does not write there.
fn target(rel: &str) -> Option<String> {
    if rel == "tsconfig.base.json" {
        return Some(rel.into());
    }
    if rel == "worker.mjs" || rel == "loopback.cjs" || rel.starts_with("clips/") {
        return Some(format!(".relay/{rel}"));
    }
    rel.strip_prefix("kit/").map(|kit| format!(".relay/kit/{kit}"))
}

/// The pinned packages' digest: the kit is outdated when the installed one differs.
pub fn deps_digest() -> String {
    let package = runtime_files().find(|(rel, _)| *rel == "package.json").map_or(&[][..], |(_, b)| b);
    Sha256::digest(package).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Write Relay's own files under `root` where they differ from this build's: the worker, the kit
/// and the shared TypeScript settings (never `package.json`, which only setting up writes).
pub fn sync(root: &Path) -> Result<(), String> {
    for (rel, bytes) in runtime_files() {
        let Some(target) = target(rel) else { continue };
        let path = root.join(target);
        if fs::read(&path).ok().as_deref() == Some(bytes) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn installed(root: &Path) -> Option<Installed> {
    serde_json::from_slice(&fs::read(root.join(".relay/kit.json")).ok()?).ok()
}

/// `node --version` when it is 20 or newer, looked up again after a while when there was none.
/// A Node too old (or one that would not answer) is asked again only once the binary on the
/// PATH is another or has changed: callers hold the store, and asking runs a process.
pub fn node_version(hub: &Hub) -> Option<String> {
    let mut cached = hub.kit.node.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, found, _)) = cached.as_ref() {
        if found.is_some() || at.elapsed() < NO_NODE_FOR {
            return found.clone();
        }
    }
    let binary = which::which("node").ok().and_then(|node| {
        let node = fs::canonicalize(&node).unwrap_or(node);
        let changed = fs::metadata(&node).and_then(|m| m.modified()).ok()?;
        Some((node, changed))
    });
    if let Some((at, None, asked)) = cached.as_mut() {
        if binary.is_some() && *asked == binary {
            *at = Instant::now();
            return None;
        }
    }
    let found = binary.as_ref().and_then(|(node, _)| {
        let out = crate::proc::output_with_timeout(Command::new(node).arg("--version"), Duration::from_secs(5)).ok()??;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let major: u32 = text.trim_start_matches('v').split('.').next()?.parse().ok()?;
        (major >= NODE_MAJOR).then_some(text)
    });
    *cached = Some((Instant::now(), found.clone(), binary));
    found
}

/// Find Node once at start, off every lock: `video.create` and `video.kit.install` read the kit's
/// status with the store held, and the first look would otherwise run `node --version` there.
pub fn warm(engine: &std::sync::Arc<Engine>) {
    let weak = std::sync::Arc::downgrade(engine);
    let _ = std::thread::Builder::new().name("video-kit-warm".into()).spawn(move || {
        if let Some(engine) = weak.upgrade() {
            node_version(&engine.video);
        }
    });
}

/// Whether the kit is set up, and what setting it up would do.
pub fn status(engine: &Engine) -> KitStatus {
    let root = engine.store.videos_dir();
    let hub = &engine.video;
    let mut out = KitStatus {
        state: KitState::Missing,
        progress: None,
        root: root.display().to_string(),
        kit_dir: root.join(".relay/kit").display().to_string(),
        node: None,
        remotion: REMOTION.into(),
        error: None,
        about: ABOUT.into(),
    };
    if hub.program().is_some() {
        out.state = KitState::Ready;
        return out;
    }
    if hub.kit.installing.load(Ordering::SeqCst) {
        out.state = KitState::Installing;
        out.progress = Some(hub.kit.progress.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            .map(|(started, p)| KitProgress { elapsed_s: started.elapsed().as_secs(), ..p.clone() })
            .unwrap_or_else(starting));
        return out;
    }
    out.node = node_version(hub);
    if out.node.is_none() {
        out.state = KitState::NoNode;
        return out;
    }
    out.error = hub.kit.error.lock().unwrap_or_else(|e| e.into_inner()).clone();
    out.state = match installed(&root) {
        Some(_) if !root.join("node_modules/remotion/package.json").is_file() => KitState::Missing,
        Some(kit) if kit.digest != deps_digest() => KitState::Outdated,
        Some(_) => KitState::Ready,
        None if out.error.is_some() => KitState::Failed,
        None => KitState::Missing,
    };
    out
}

/// Ok when the worker can run; otherwise the refusal that says what to do.
pub fn ready(engine: &Engine) -> Result<(), BusError> {
    let status = status(engine);
    let wait = "bus.wait {\"events\": [\"video.kit.changed\"]} until its state is ready";
    Err(match status.state {
        KitState::Ready => return Ok(()),
        KitState::Installing => BusError::unavailable("video.kit.installing", "the video kit is being set up")
            .with_hint(format!("{wait}, then try again")),
        KitState::Outdated => BusError::unavailable("video.kit.outdated", "the video kit was set up for an older Relay")
            .with_hint(format!("call video.kit.install to bring it up to date, then {wait}")),
        KitState::NoNode => BusError::unavailable("video.kit.no_node", format!("Node.js {NODE_MAJOR} or newer is not installed"))
            .with_hint("ask the person to install Node.js and npm (their package manager's nodejs and npm), then try again"),
        KitState::Missing | KitState::Failed => {
            let why = status.error.map(|e| format!(" (the last setup failed: {e})")).unwrap_or_default();
            BusError::unavailable("video.kit.missing", format!("this machine is not set up to render videos{why}"))
                .with_hint(format!("call video.kit.install (it downloads about 600 MB once), then {wait}"))
        }
    }
    .with_details(serde_json::json!({"state": status.state})))
}

/// Set the kit up on a thread of its own, unless it is being set up already; the status now.
pub fn install(engine: &Arc<Engine>) -> KitStatus {
    let hub = &engine.video;
    if hub.kit.installing.swap(true, Ordering::SeqCst) {
        return status(engine);
    }
    *hub.kit.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *hub.kit.progress.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), starting()));
    let weak = Arc::downgrade(engine);
    // The ticker: every few seconds, how much of the stage has landed, as `video.kit.progress`.
    let ticking = weak.clone();
    let _ = std::thread::Builder::new().name("video-kit-progress".into()).spawn(move || {
        let mut said = -1.0;
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let Some(engine) = ticking.upgrade() else { return };
            if !engine.video.kit.installing.load(Ordering::SeqCst) {
                return;
            }
            let root = engine.store.videos_dir();
            let reading = {
                let guard = engine.video.kit.progress.lock().unwrap_or_else(|e| e.into_inner());
                let Some((started, p)) = guard.as_ref() else { return };
                let (stage, share) = match p.stage.as_str() {
                    "browser" => {
                        let landed = bytes_under(&root.join("node_modules/.remotion")) as f64 / BROWSER_BYTES as f64;
                        ("browser", PACKAGES_SHARE + (1.0 - PACKAGES_SHARE) * landed.min(0.98))
                    }
                    _ => {
                        let landed = bytes_under(&root.join("node_modules")) as f64 / PACKAGES_BYTES as f64;
                        ("packages", PACKAGES_SHARE * landed.min(0.98))
                    }
                };
                KitProgress { stage: stage.into(), progress: (share * 100.0).round() / 100.0, detail: p.detail.clone(), elapsed_s: started.elapsed().as_secs() }
            };
            if let Some((_, p)) = engine.video.kit.progress.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                p.progress = p.progress.max(reading.progress);
            }
            if reading.progress - said >= 0.02 {
                said = reading.progress;
                engine.emit_system("video.kit.progress", serde_json::to_value(&reading).unwrap_or_default());
            }
        }
    });
    let started = std::thread::Builder::new().name("video-kit".into()).spawn(move || {
        let Some(engine) = weak.upgrade() else { return };
        // However the setup ends, a panic included, it is over when this goes.
        let _settle = Settle(engine.clone());
        let root = engine.store.videos_dir();
        let stage = |name: &str, detail: &str| {
            if let Some((_, p)) = engine.video.kit.progress.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                p.stage = name.into();
                p.detail = detail.into();
                if name == "browser" {
                    p.progress = p.progress.max(PACKAGES_SHARE);
                }
            }
        };
        // Nothing runs on the packages while they are replaced.
        super::worker::stop(&engine);
        if let Err(e) = set_up(&root, &stage) {
            tracing::warn!(error = %e, "video kit setup failed");
            *engine.video.kit.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
        }
    });
    if started.is_err() {
        hub.kit.installing.store(false, Ordering::SeqCst);
    }
    let status = status(engine);
    engine.emit_system("video.kit.changed", serde_json::to_value(&status).unwrap_or_default());
    status
}

/// The end of a setup, run when its thread lets go of it, however it ends: the worker stopped
/// (one started on the old kit would keep it), the kit no longer setting up, its state said, and
/// the renders queued meanwhile started. A setup that panicked is a failed one.
struct Settle(Arc<Engine>);

impl Drop for Settle {
    fn drop(&mut self) {
        let engine = &self.0;
        let hub = &engine.video;
        if std::thread::panicking() {
            *hub.kit.error.lock().unwrap_or_else(|e| e.into_inner()) = Some("the setup stopped on an error in Relay".into());
        }
        *hub.kit.progress.lock().unwrap_or_else(|e| e.into_inner()) = None;
        super::worker::stop(engine);
        hub.kit.installing.store(false, Ordering::SeqCst);
        let status = status(engine);
        engine.emit_system("video.kit.changed", serde_json::to_value(&status).unwrap_or_default());
        super::jobs::enqueue(engine);
    }
}

fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(6)..].join(" | ").chars().take(800).collect()
}

/// The setup itself: Relay's files and the pinned `package.json`, `npm install`, then Remotion's
/// browser; `kit.json` last, so a setup cut short reads as not done.
fn set_up(root: &Path, stage: &dyn Fn(&str, &str)) -> Result<(), String> {
    fs::create_dir_all(root.join(".relay")).map_err(|e| format!("{}: {e}", root.display()))?;
    stage("packages", "npm installs Remotion, React, TypeScript and the fonts (about 350 MB)");
    sync(root)?;
    let package = runtime_files().find(|(rel, _)| *rel == "package.json").map(|(_, b)| b).ok_or("the Video plugin has no runtime/package.json")?;
    fs::write(root.join("package.json"), package).map_err(|e| format!("package.json: {e}"))?;
    let npm: PathBuf = which::which("npm").map_err(|_| "npm is not installed (it comes with Node.js)".to_string())?;
    let out = crate::proc::output_with_timeout(
        Command::new(npm)
            .args(["install", "--no-audit", "--no-fund", "--loglevel=error"])
            .env("npm_config_update_notifier", "false")
            .current_dir(root),
        NPM_WITHIN,
    ).map_err(|e| format!("npm install: {e}"))?.ok_or("npm install took over 15 minutes")?;
    if !out.status.success() {
        return Err(format!("npm install failed: {}", tail(&out.stderr)));
    }
    let node = which::which("node").map_err(|_| "node is not installed".to_string())?;
    stage("browser", "fetching Remotion's headless Chrome (about 240 MB)");
    let out = crate::proc::output_with_timeout(
        Command::new(node).arg(root.join(".relay/worker.mjs")).arg("ensure-browser").current_dir(root),
        BROWSER_WITHIN,
    ).map_err(|e| format!("fetching Remotion's browser: {e}"))?.ok_or("fetching Remotion's browser took over 10 minutes")?;
    if !out.status.success() {
        return Err(format!("fetching Remotion's browser failed: {} {}", tail(&out.stdout), tail(&out.stderr)));
    }
    let kit = Installed { digest: deps_digest(), remotion: REMOTION.into(), installed_at: crate::time::now() };
    fs::write(root.join(".relay/kit.json"), serde_json::to_vec_pretty(&kit).unwrap_or_default()).map_err(|e| format!("kit.json: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A setup whose thread panics still ends: the kit is no longer setting up (so the renders
    /// waiting on it start), and it reads as failed.
    #[test]
    fn a_setup_that_panics_still_ends() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        engine.video.kit.installing.store(true, Ordering::SeqCst);
        let held = engine.clone();
        let ended = std::thread::spawn(move || {
            let _settle = Settle(held);
            panic!("set_up broke");
        }).join();
        assert!(ended.is_err());
        assert!(!engine.video.kit.installing.load(Ordering::SeqCst));
        assert_eq!(engine.video.kit.error.lock().unwrap().as_deref(), Some("the setup stopped on an error in Relay"));
    }

    #[test]
    fn the_runtime_comes_with_the_plugin_and_lands_where_videos_resolve_it() {
        let files: Vec<&str> = runtime_files().map(|(rel, _)| rel).collect();
        for needed in ["package.json", "worker.mjs", "tsconfig.base.json", "kit/index.ts", "kit/probe.ts", "starter/src/index.ts", "starter/src/Root.tsx"] {
            assert!(files.contains(&needed), "{needed} missing from {files:?}");
        }
        assert_eq!(target("worker.mjs").as_deref(), Some(".relay/worker.mjs"));
        assert_eq!(target("kit/text.tsx").as_deref(), Some(".relay/kit/text.tsx"));
        assert_eq!(target("clips/streamclips.py").as_deref(), Some(".relay/clips/streamclips.py"));
        assert!(files.contains(&"kit/clips.tsx") && files.contains(&"clips/streamclips.py"));
        assert_eq!(target("starter/src/Video.tsx"), None);
        assert_eq!(target("package.json"), None);
        let package: serde_json::Value = serde_json::from_slice(runtime_files().find(|(r, _)| *r == "package.json").unwrap().1).unwrap();
        assert_eq!(package["dependencies"]["remotion"], REMOTION, "REMOTION names the pinned version");
        assert_eq!(package["dependencies"]["@relay/video"], "file:./.relay/kit");
    }

    #[test]
    fn sync_writes_relays_files_and_leaves_them_when_unchanged() {
        let root = tempfile::tempdir().unwrap();
        sync(root.path()).unwrap();
        let worker = root.path().join(".relay/worker.mjs");
        assert!(worker.is_file() && root.path().join(".relay/kit/index.ts").is_file());
        assert!(!root.path().join("package.json").exists(), "only setting up writes package.json");
        fs::write(&worker, "changed").unwrap();
        sync(root.path()).unwrap();
        assert_ne!(fs::read_to_string(&worker).unwrap(), "changed");
    }
}
