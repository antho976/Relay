//! Build seeds: a new session's worktree starts with the project's third-party Rust
//! dependencies already compiled, so its first `cargo test` compiles only the project's own
//! crates (148 s from nothing, 72 s seeded, on the reference machine: docs/perf/FIXES-2026-10-16.md).
//!
//! Relay keeps one seed per repository under `.relay/build-seed/`. `staging/` is a target
//! directory built in the background from a clean export of a commit; after each build step it
//! is published as an immutable generation, `gens/<n>/target`, with every local package's
//! output taken out, and `current` points at the newest. A new worktree gets a copy-on-write
//! copy of the current generation (`cp --reflink=always`): seconds, and no disk until one side
//! rebuilds a file. Cargo's own fingerprints judge the copy, as they judge any target directory:
//! a dependency whose version, features, profile or compiler changed is rebuilt, so a stale seed
//! only makes a build slower, never wrong. Cargo also rewrites a copied build script's output
//! directory to the new target's, so nothing points back at `staging/`.
//!
//! What could go wrong, and what stops it:
//! - The project's own crates judged fresh by timestamps: their output is never copied, so they
//!   always build from the worktree's own source (it costs nothing — they rebuild anyway).
//! - A copy taken while a build writes: copies come only from published generations, which are
//!   never written again, and a generation is not swept while a copy reads it.
//! - The agent's own `cargo` writing into `target/` as the copy lands: the gate holds a cargo
//!   command until the copy is in, and the copy is renamed in only if no `target/` exists.
//! - A filesystem that cannot share extents: nothing is copied (a real copy would be ~28 GB).
//! - A burst of sessions before any seed exists: one build; their cargo commands are refused
//!   with `build.seeding` until `build.seed.ready`, rather than N builds of the same crates.
//!
//! The seed is built in the engine's environment, which is the one its agents inherit: a
//! variable a crate's build reads (`RUSTFLAGS`, or any `env!`) is the same on both sides, as
//! Cargo's fingerprints require for the copy to count as fresh.
//!
//! `RELAY_BUILD_SEED=0` in the engine's environment turns it off.

use crate::Engine;
use anyhow::{anyhow, Context, Result};
use relay_bus::types::Id;
use relay_bus::BusError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

pub const READY_EVENT: &str = "build.seed.ready";

/// How long the gate holds a cargo command for a copy in flight. A copy takes about a second; past
/// this something is wrong, and the agent builds on its own rather than wait longer. Well inside
/// the hook's own 20 s deadline for the whole gate (`GATE_DEADLINE`, relay-cli), past which the
/// hook blocks the command instead.
const COPY_HOLD: Duration = Duration::from_secs(10);
/// How long a worktree with nothing to copy waits on the first build before its cargo commands
/// go through anyway.
const BUILD_HOLD: Duration = Duration::from_secs(30 * 60);
/// The most one build step may run.
const STEP_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// The most one copy may run; a reflink copy of 54 GB takes 2.5 s.
const COPY_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How much of a failed step's stderr the log keeps.
const LOG_TAIL: usize = 4096;

pub fn seed_dir(repo: &Path) -> PathBuf {
    repo.join(".relay").join("build-seed")
}

fn enabled(instance: crate::Instance) -> bool {
    instance != crate::Instance::Test
        && !matches!(std::env::var("RELAY_BUILD_SEED").as_deref(), Ok("0" | "off" | "false"))
}

/// Every repository's seed state, in process. Its own lock; never the store's.
#[derive(Clone, Default)]
pub struct Seeds(Arc<Shared>);

#[derive(Default)]
struct Shared {
    state: Mutex<HashMap<PathBuf, Repo>>,
    changed: Condvar,
}

#[derive(Default)]
struct Repo {
    project_id: Id,
    /// Whether this repository's filesystem was probed, and could not share extents.
    probed: bool,
    unsupported: bool,
    /// When the running build began.
    building: Option<Instant>,
    /// A newer commit a running build should be followed by.
    again: Option<String>,
    /// Worktrees with no generation to copy, served when the running build publishes one.
    waiting: HashSet<PathBuf>,
    /// Worktrees a copy is being made for → the generation it reads (`None`: not chosen yet).
    copying: HashMap<PathBuf, Option<PathBuf>>,
}

/// What a generation was built from, beside it as `seed.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    pub generation: u64,
    pub commit: String,
    /// SHA-256 of the `Cargo.lock` it was built from.
    pub lock: String,
    /// `rustc -vV`.
    pub rustc: String,
    pub built_at: String,
    /// The step this generation is the result of, and whether it finished cleanly. A failed
    /// step's generation still holds every crate that did build: Cargo writes a crate's
    /// fingerprint only once it succeeds.
    pub step: String,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct Generation {
    pub dir: PathBuf,
    pub meta: Meta,
}

impl Generation {
    pub fn target(&self) -> PathBuf {
        self.dir.join("target")
    }
    /// Whether `worktree` would build against different dependencies or another compiler.
    fn stale_for(&self, worktree: &Path) -> bool {
        lock_hash(worktree).ok().as_deref() != Some(self.meta.lock.as_str())
            || rustc_version(worktree).ok().as_deref() != Some(self.meta.rustc.as_str())
    }
}

/// A new session's worktree was just checked out: copy the current generation into it, on a
/// thread of its own, and build or refresh the seed when it has none or an outdated one. The
/// worktree is marked before this returns, so its first cargo command is held for the copy.
pub fn seed_new_worktree(engine: &Arc<Engine>, instance: crate::Instance, project_id: Id, repo: &Path, worktree: &Path) {
    if !enabled(instance) || !worktree.join("Cargo.lock").is_file() || worktree.join("target").symlink_metadata().is_ok() {
        return;
    }
    let seeds = engine.build_seeds.clone();
    {
        let mut state = seeds.0.state.lock().unwrap();
        let entry = state.entry(repo.to_path_buf()).or_default();
        if entry.unsupported {
            return;
        }
        entry.project_id = project_id;
        entry.copying.insert(worktree.to_path_buf(), None);
    }
    let weak = Arc::downgrade(engine);
    let (repo, worktree) = (repo.to_path_buf(), worktree.to_path_buf());
    let thread = {
        let seeds = seeds.clone();
        let (repo, worktree) = (repo.clone(), worktree.clone());
        std::thread::Builder::new().name("build-seed".into()).spawn(move || {
            // Children inherit the thread's nice value: the seed's compile yields to the person.
            crate::background_priority();
            seeds.serve(&weak, &repo, &worktree);
        })
    };
    if thread.is_err() {
        seeds.settle(&repo, &worktree, false);
    }
}

/// The guardrail gate saw `session`'s agent about to run `command` (D149: the gate's unlocked
/// phase). A cargo command in a worktree whose copy is landing waits for it; one in a worktree
/// waiting on the first build is refused with `build.seeding`. Everything else passes at once.
pub(crate) fn gate(ctx: &crate::engine::Unlocked, session: &str, command: &str) -> Result<(), BusError> {
    let seeds = &ctx.engine().build_seeds;
    if !seeds.tracking() || !invokes_cargo(command) {
        return Ok(());
    }
    let Ok(worktree) = ctx.read(|conn| Ok(crate::sessions::by_name(conn, session)?.session.worktree)) else {
        return Ok(());
    };
    seeds.hold(Path::new(&worktree), COPY_HOLD)
}

/// Cargo subcommands that compile into `target/`; `cargo fmt`, `metadata` or `tree` never wait.
const BUILDING: &[&str] = &[
    "build", "b", "check", "c", "clippy", "test", "t", "run", "r", "bench", "doc", "d", "rustc", "rustdoc",
    "fix", "nextest", "llvm-cov", "miri",
];

/// Whether `line` runs a cargo command that builds, past the assignments and wrappers in front of
/// it and cargo's own flags (`+toolchain`, `-q`).
pub(crate) fn invokes_cargo(line: &str) -> bool {
    crate::shell::commands(line).iter().any(|segment| {
        let mut words = segment.iter().map(|word| word.text.as_str()).skip_while(|word| {
            is_assignment(word) || matches!(*word, "env" | "time" | "nice" | "nohup" | "command" | "exec")
        });
        words.next().is_some_and(|program| program.rsplit('/').next() == Some("cargo"))
            && words.find(|word| !word.starts_with(['-', '+'])).is_some_and(|sub| BUILDING.contains(&sub))
    })
}

fn is_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

impl Seeds {
    /// Whether any worktree is waiting on a copy or a build: the gate's check for every other
    /// command is this one lock.
    fn tracking(&self) -> bool {
        self.0.state.lock().unwrap().values().any(|repo| !repo.copying.is_empty() || !repo.waiting.is_empty())
    }

    fn hold(&self, worktree: &Path, copy_hold: Duration) -> Result<(), BusError> {
        let deadline = Instant::now() + copy_hold;
        let mut state = self.0.state.lock().unwrap();
        loop {
            let Some(repo) = state.values().find(|repo| repo.copying.contains_key(worktree) || repo.waiting.contains(worktree)) else {
                return Ok(());
            };
            if repo.waiting.contains(worktree) {
                return match repo.building {
                    Some(started) if started.elapsed() < BUILD_HOLD => Err(seeding(started.elapsed())),
                    _ => Ok(()),
                };
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            state = self.0.changed.wait_timeout(state, deadline - now).unwrap().0;
        }
    }

    /// The copy for `worktree` is over: it is no longer held, or, with `wait`, it waits on the
    /// running build instead.
    fn settle(&self, repo: &Path, worktree: &Path, wait: bool) {
        let mut state = self.0.state.lock().unwrap();
        if let Some(entry) = state.get_mut(repo) {
            entry.copying.remove(worktree);
            if wait {
                entry.waiting.insert(worktree.to_path_buf());
            }
        }
        self.0.changed.notify_all();
    }

    fn supported(&self, repo: &Path, dir: &Path) -> bool {
        {
            let state = self.0.state.lock().unwrap();
            if let Some(entry) = state.get(repo).filter(|entry| entry.probed) {
                return !entry.unsupported;
            }
        }
        let ok = probe_reflink(dir);
        if !ok {
            tracing::info!(repo = %repo.display(), "build seed off: this filesystem cannot share extents (cp --reflink)");
        }
        let mut state = self.0.state.lock().unwrap();
        let entry = state.entry(repo.to_path_buf()).or_default();
        entry.probed = true;
        entry.unsupported = !ok;
        ok
    }

    fn serve(&self, engine: &Weak<Engine>, repo: &Path, worktree: &Path) {
        let dir = seed_dir(repo);
        if !self.supported(repo, &dir) {
            self.settle(repo, worktree, false);
            return;
        }
        let current = current(&dir);
        match &current {
            Some(generation) => {
                self.reading(repo, worktree, &generation.dir);
                match copy_into(&generation.target(), worktree, &dir) {
                    Ok(true) => tracing::info!(worktree = %worktree.display(), generation = generation.meta.generation, "seeded target/"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(worktree = %worktree.display(), %error, "copying the build seed"),
                }
                self.settle(repo, worktree, false);
            }
            None => self.settle(repo, worktree, true),
        }
        if current.as_ref().is_none_or(|generation| generation.stale_for(worktree)) {
            match head(worktree) {
                Ok(commit) => self.build(engine, repo, &commit),
                Err(error) => {
                    tracing::warn!(worktree = %worktree.display(), %error, "reading the commit to seed from");
                    // Not served by this thread; a build already running may still serve it.
                    if self.0.state.lock().unwrap().get(repo).is_some_and(|entry| entry.building.is_none()) {
                        self.unwait(repo, worktree);
                    }
                }
            }
        }
    }

    fn reading(&self, repo: &Path, worktree: &Path, generation: &Path) {
        if let Some(entry) = self.0.state.lock().unwrap().get_mut(repo) {
            entry.copying.insert(worktree.to_path_buf(), Some(generation.to_path_buf()));
        }
    }

    /// Build the seed from `commit`, unless a build is already running: then it is followed by
    /// one from `commit`.
    fn build(&self, engine: &Weak<Engine>, repo: &Path, commit: &str) {
        {
            let mut state = self.0.state.lock().unwrap();
            let entry = state.entry(repo.to_path_buf()).or_default();
            if entry.building.is_some() {
                entry.again = Some(commit.to_string());
                return;
            }
            entry.building = Some(Instant::now());
        }
        let mut commit = commit.to_string();
        loop {
            // A panic in the build must still end it below, or the repository's seed would stay
            // "building" with its waiting worktrees refused until the hold ran out.
            let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_seed(repo, &commit, |generation, step, steps| self.published(engine, repo, generation, step, steps))
            }))
            .unwrap_or_else(|_| Err(anyhow!("the build panicked")));
            if let Err(error) = &built {
                tracing::warn!(repo = %repo.display(), %error, "building the build seed");
            }
            // Taking the follow-up and ending the build are one step: a request that arrives
            // after it sees no build running and starts its own.
            let mut state = self.0.state.lock().unwrap();
            let entry = state.entry(repo.to_path_buf()).or_default();
            match entry.again.take() {
                Some(next) if next != commit => commit = next,
                _ => {
                    entry.building = None;
                    drop(state);
                    break self.finish(engine, repo, built.is_ok());
                }
            }
        }
    }

    /// The build is over. A worktree that started waiting after the last generation was
    /// published is served from the current one; with none, it builds on its own.
    fn finish(&self, engine: &Weak<Engine>, repo: &Path, ok: bool) {
        if let Some(generation) = current(&seed_dir(repo)) {
            let pending = self.0.state.lock().unwrap().get(repo).is_some_and(|entry| !entry.waiting.is_empty());
            if pending {
                self.published(engine, repo, &generation, 0, 0);
            }
            return;
        }
        let (project_id, unserved) = {
            let mut state = self.0.state.lock().unwrap();
            let Some(entry) = state.get_mut(repo) else { return };
            (entry.project_id, std::mem::take(&mut entry.waiting))
        };
        self.0.changed.notify_all();
        if !unserved.is_empty() {
            if let Some(engine) = engine.upgrade() {
                engine.emit_system(READY_EVENT, json!({
                    "project_id": project_id, "ok": false, "generation": null,
                    "served": unserved.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                    "note": if ok { "the build published nothing to copy" } else { "the build failed; cargo builds on its own" },
                }));
            }
        }
    }

    fn unwait(&self, repo: &Path, worktree: &Path) {
        if let Some(entry) = self.0.state.lock().unwrap().get_mut(repo) {
            entry.waiting.remove(worktree);
        }
        self.0.changed.notify_all();
    }

    /// A generation was published: copy it to every worktree waiting for one and announce it.
    /// Answers with the generations a copy is still reading.
    fn published(&self, engine: &Weak<Engine>, repo: &Path, generation: &Generation, step: usize, steps: usize) -> HashSet<PathBuf> {
        let dir = seed_dir(repo);
        let (project_id, waiting) = {
            let mut state = self.0.state.lock().unwrap();
            let entry = state.entry(repo.to_path_buf()).or_default();
            let waiting: Vec<PathBuf> = entry.waiting.drain().collect();
            for worktree in &waiting {
                entry.copying.insert(worktree.clone(), Some(generation.dir.clone()));
            }
            (entry.project_id, waiting)
        };
        for worktree in &waiting {
            if let Err(error) = copy_into(&generation.target(), worktree, &dir) {
                tracing::warn!(worktree = %worktree.display(), %error, "copying the build seed");
            }
            self.settle(repo, worktree, false);
        }
        if let Some(engine) = engine.upgrade() {
            engine.emit_system(READY_EVENT, json!({
                "project_id": project_id, "ok": generation.meta.ok, "generation": generation.meta.generation,
                "step": step + 1, "steps": steps, "command": generation.meta.step, "commit": generation.meta.commit,
                "served": waiting.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
            }));
        }
        self.0.state.lock().unwrap().get(repo)
            .map(|entry| entry.copying.values().flatten().cloned().collect())
            .unwrap_or_default()
    }
}

fn seeding(elapsed: Duration) -> BusError {
    BusError::conflict(
        "build.seeding",
        format!(
            "Relay is compiling this project's dependencies once for every new session (started {} ago); this \
             worktree gets a copy the moment the first step lands, and cargo here then compiles only the project's own crates",
            minutes(elapsed),
        ),
    )
    .with_hint("wait for it with bus.wait {\"events\":[\"build.seed.ready\"]} and retry; reading and editing need not wait")
}

fn minutes(elapsed: Duration) -> String {
    match elapsed.as_secs() {
        s if s < 60 => format!("{s} s"),
        s => format!("{} min", s / 60),
    }
}

/// The generation `current` points at, if it is whole.
pub fn current(dir: &Path) -> Option<Generation> {
    let link = std::fs::read_link(dir.join("current")).ok()?;
    let generation = dir.join(link);
    let meta = serde_json::from_slice(&std::fs::read(generation.join("seed.json")).ok()?).ok()?;
    generation.join("target").is_dir().then_some(Generation { dir: generation, meta })
}

/// Build the seed for `repo` from `commit`, publishing a generation after each step. Another
/// process building the same seed (a second Relay instance) is left to it.
/// `published` is told of each generation and answers with the generations a copy is still
/// reading, which the sweep after it keeps.
pub fn build_seed(repo: &Path, commit: &str, mut published: impl FnMut(&Generation, usize, usize) -> HashSet<PathBuf>) -> Result<usize> {
    let dir = seed_dir(repo);
    std::fs::create_dir_all(&dir)?;
    let Some(_lock) = try_lock(&dir.join("lock"))? else {
        tracing::info!(repo = %repo.display(), "build seed: another process is building it");
        return Ok(0);
    };
    let started = Instant::now();
    clear_leftovers(&dir);
    let src = dir.join("src");
    export(repo, commit, &dir, &src)?;
    let built = build_steps(&dir, &src, commit, &mut published);
    let _ = std::fs::remove_dir_all(&src);
    let count = built?;
    tracing::info!(repo = %repo.display(), commit, generations = count, seconds = started.elapsed().as_secs(), "build seed built");
    Ok(count)
}

fn build_steps(dir: &Path, src: &Path, commit: &str, published: &mut impl FnMut(&Generation, usize, usize) -> HashSet<PathBuf>) -> Result<usize> {
    let rustc = rustc_version(src)?;
    let lock = lock_hash(src)?;
    let staging = dir.join("staging");
    // A new compiler leaves nothing reusable; start over rather than keep every old artifact.
    let stamp = dir.join("staging.rustc");
    if std::fs::read_to_string(&stamp).ok().as_deref() != Some(rustc.as_str()) {
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::write(&stamp, &rustc)?;
    }
    let workspace = metadata(src)?;
    let steps = steps(&workspace);
    let mut log = String::new();
    let mut count = 0;
    for (index, step) in steps.iter().enumerate() {
        let started = Instant::now();
        let mut command = Command::new("cargo");
        // Incremental state is kept only for local packages, which no copy keeps: it would be
        // disk spent on nothing. Dependencies' fingerprints do not depend on it.
        command.args(step).current_dir(src).env("CARGO_TARGET_DIR", &staging).env("CARGO_INCREMENTAL", "0");
        let output = crate::proc::output_with_timeout(&mut command, STEP_TIMEOUT);
        let line = format!("cargo {}", step.join(" "));
        let ok = matches!(&output, Ok(Some(out)) if out.status.success());
        log.push_str(&format!("{line}: {} in {} s\n", if ok { "ok" } else { "failed" }, started.elapsed().as_secs()));
        match &output {
            Ok(Some(out)) if !ok => {
                log.push_str(tail(&String::from_utf8_lossy(&out.stderr), LOG_TAIL));
                log.push('\n');
            }
            Ok(None) => log.push_str("timed out\n"),
            Err(error) => log.push_str(&format!("{error}\n")),
            Ok(Some(_)) => {}
        }
        let _ = std::fs::write(dir.join("last-build.log"), &log);
        if !staging.join("debug").is_dir() {
            continue;
        }
        let meta = Meta {
            generation: 0, commit: commit.to_string(), lock: lock.clone(), rustc: rustc.clone(),
            built_at: crate::time::now(), step: line, ok,
        };
        let generation = publish(dir, &staging, &workspace.local, meta)?;
        count += 1;
        let reading = published(&generation, index, steps.len());
        sweep(dir, &reading);
    }
    Ok(count)
}

/// The last `max` bytes of `text`, or a little less, cut on a character boundary.
fn tail(text: &str, max: usize) -> &str {
    let mut start = text.len().saturating_sub(max);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// The workspace as the seed needs it: which packages are local (their output is never copied)
/// and which members `cargo test` does not build by default.
#[derive(Debug, Default)]
pub struct Workspace {
    pub local: Vec<String>,
    pub extra_members: Vec<String>,
}

fn metadata(src: &Path) -> Result<Workspace> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--format-version", "1"]).current_dir(src);
    let out = crate::proc::output_with_timeout(&mut command, Duration::from_secs(600))?
        .ok_or_else(|| anyhow!("cargo metadata timed out"))?;
    if !out.status.success() {
        return Err(anyhow!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    parse_metadata(&out.stdout)
}

pub fn parse_metadata(json: &[u8]) -> Result<Workspace> {
    let value: serde_json::Value = serde_json::from_slice(json)?;
    let packages = value["packages"].as_array().cloned().unwrap_or_default();
    let name_of = |id: &serde_json::Value| {
        packages.iter().find(|package| package["id"] == *id).and_then(|package| package["name"].as_str()).map(str::to_string)
    };
    let local = packages.iter()
        .filter(|package| package["source"].is_null())
        .filter_map(|package| package["name"].as_str().map(str::to_string))
        .collect();
    let members = value["workspace_members"].as_array().cloned().unwrap_or_default();
    let defaults = value["workspace_default_members"].as_array().cloned().unwrap_or_else(|| members.clone());
    let extra_members = members.iter().filter(|id| !defaults.contains(id)).filter_map(name_of).collect();
    Ok(Workspace { local, extra_members })
}

/// What the seed builds: what agents run. `cargo test` and `cargo build` share their
/// dependencies' output; clippy checks them separately; a member outside the default set (a
/// GTK client) is built on its own, as it is built in use. A step that cannot build here
/// (no GTK) still leaves every dependency it did build.
pub fn steps(workspace: &Workspace) -> Vec<Vec<String>> {
    let mut steps = vec![
        vec!["test".to_string(), "--no-run".to_string()],
        vec!["clippy".to_string(), "--all-targets".to_string()],
    ];
    for member in &workspace.extra_members {
        steps.push(vec!["build".to_string(), "-p".to_string(), member.clone()]);
    }
    steps
}

/// Snapshot `staging` as the next generation, without the local packages, and point `current`
/// at it. The rename makes it whole before anything can read it.
fn publish(dir: &Path, staging: &Path, local: &[String], mut meta: Meta) -> Result<Generation> {
    let gens = dir.join("gens");
    std::fs::create_dir_all(&gens)?;
    let number = numbered(&gens).into_iter().max().unwrap_or(0) + 1;
    meta.generation = number;
    let next = gens.join(number.to_string());
    let partial = gens.join(format!("{number}.partial"));
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial)?;
    reflink_copy(staging, &partial.join("target"))?;
    prune(&partial.join("target"), local)?;
    std::fs::write(partial.join("seed.json"), serde_json::to_vec_pretty(&meta)?)?;
    std::fs::rename(&partial, &next)?;
    let link = dir.join("current.next");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(Path::new("gens").join(number.to_string()), &link)?;
    std::fs::rename(&link, dir.join("current"))?;
    Ok(Generation { dir: next, meta })
}

fn numbered(gens: &Path) -> Vec<u64> {
    std::fs::read_dir(gens).map(|entries| {
        entries.flatten().filter_map(|entry| entry.file_name().to_str()?.parse().ok()).collect()
    }).unwrap_or_default()
}

/// Keep the two newest generations and any a copy is reading; remove the rest.
fn sweep(dir: &Path, reading: &HashSet<PathBuf>) {
    let gens = dir.join("gens");
    let mut numbers = numbered(&gens);
    numbers.sort_unstable();
    let keep = numbers.len().saturating_sub(2);
    for number in &numbers[..keep] {
        let generation = gens.join(number.to_string());
        if !reading.contains(&generation) {
            let _ = std::fs::remove_dir_all(&generation);
        }
    }
}

/// A crashed build's half-made generation and copies. Called under the build lock.
fn clear_leftovers(dir: &Path) {
    let gens = dir.join("gens");
    if let Ok(entries) = std::fs::read_dir(&gens) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(".partial") {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("copies")) {
        let old = |entry: &std::fs::DirEntry| entry.metadata().and_then(|m| m.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age > Duration::from_secs(3600)));
        for entry in entries.flatten().filter(old) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Take every local package's output out of a target directory, so the worktree that copies it
/// compiles them from its own source: their fingerprints, build-script runs and incremental
/// state, and the binaries and libraries uplifted beside `deps/`. What stays in `deps/` under
/// their names is overwritten by that rebuild.
pub fn prune(target: &Path, local: &[String]) -> Result<()> {
    for profile in profile_dirs(target)? {
        for kind in [".fingerprint", "build"] {
            let Ok(entries) = std::fs::read_dir(profile.join(kind)) else { continue };
            for entry in entries.flatten() {
                if is_local_unit(&entry.file_name().to_string_lossy(), local) {
                    std::fs::remove_dir_all(entry.path()).with_context(|| format!("pruning {}", entry.path().display()))?;
                }
            }
        }
        for dir in ["incremental", "examples"] {
            let _ = std::fs::remove_dir_all(profile.join(dir));
        }
        for entry in std::fs::read_dir(&profile)?.flatten() {
            if entry.file_type().is_ok_and(|kind| !kind.is_dir()) {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

/// Every directory holding a `.fingerprint/`: `target/<profile>`, and `target/<triple>/<profile>`
/// for a build with `--target` or `build.target`. A local package's units under either must go.
fn profile_dirs(target: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for child in std::fs::read_dir(target)?.flatten() {
        let child = child.path();
        if !child.is_dir() {
            continue;
        }
        if child.join(".fingerprint").is_dir() {
            found.push(child);
            continue;
        }
        for grandchild in std::fs::read_dir(&child).into_iter().flatten().flatten() {
            if grandchild.path().join(".fingerprint").is_dir() {
                found.push(grandchild.path());
            }
        }
    }
    Ok(found)
}

/// `<package>-<16 hex>`: a unit directory of one of `local`.
fn is_local_unit(name: &str, local: &[String]) -> bool {
    local.iter().any(|package| {
        name.strip_prefix(package.as_str())
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|hash| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
    })
}

/// Copy `target` into `worktree/target`, unless the worktree has one by then. Ok(true) when it
/// was copied.
pub fn copy_into(target: &Path, worktree: &Path, dir: &Path) -> Result<bool> {
    let dest = worktree.join("target");
    if !worktree.is_dir() || dest.symlink_metadata().is_ok() {
        return Ok(false);
    }
    let copies = dir.join("copies");
    std::fs::create_dir_all(&copies)?;
    let name = worktree.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let staged = copies.join(format!("{name}-{}-{nanos}", std::process::id()));
    if let Err(error) = reflink_copy(target, &staged) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(error);
    }
    match rename_noreplace(&staged, &dest) {
        Ok(()) => Ok(true),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staged);
            // The agent's own cargo made one first: it builds without the seed.
            if matches!(error.raw_os_error(), Some(libc::EEXIST) | Some(libc::ENOTEMPTY)) {
                Ok(false)
            } else {
                Err(error).context("moving the copy into place")
            }
        }
    }
}

fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both are NUL-terminated paths that outlive the call.
    let result = unsafe { libc::renameat2(libc::AT_FDCWD, from.as_ptr(), libc::AT_FDCWD, to.as_ptr(), libc::RENAME_NOREPLACE) };
    if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

/// `cp -a --reflink=always`: shares every extent, or fails. Never a byte-for-byte copy.
fn reflink_copy(from: &Path, to: &Path) -> Result<()> {
    let mut command = Command::new("cp");
    command.args(["-a", "--reflink=always", "--no-target-directory"]).arg(from).arg(to);
    let out = crate::proc::output_with_timeout(&mut command, COPY_TIMEOUT)?
        .ok_or_else(|| anyhow!("cp --reflink timed out after {} s", COPY_TIMEOUT.as_secs()))?;
    if !out.status.success() {
        return Err(anyhow!("cp --reflink failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// Whether files under `dir` can share extents: one small reflink copy.
pub fn probe_reflink(dir: &Path) -> bool {
    // Its own names: two sessions probing at once must not delete each other's file.
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let probe = dir.join(format!("probe-{}-{nanos}", std::process::id()));
    let copy = probe.with_extension("copy");
    let ok = std::fs::create_dir_all(dir).is_ok()
        && std::fs::write(&probe, b"relay build seed probe").is_ok()
        && reflink_copy(&probe, &copy).is_ok();
    let _ = std::fs::remove_file(&probe);
    let _ = std::fs::remove_file(&copy);
    ok
}

/// A clean tree of `commit` at `src`: never a checkout someone is editing. Files get the time of
/// extraction, not the commit's.
fn export(repo: &Path, commit: &str, dir: &Path, src: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(src);
    std::fs::create_dir_all(src)?;
    let tar = dir.join("src.tar");
    let tar_s = tar.display().to_string();
    crate::worktree::git_mutate(repo, &["archive", "--format=tar", "-o", &tar_s, commit])?;
    let mut command = Command::new("tar");
    command.arg("-xmf").arg(&tar).arg("-C").arg(src);
    let out = crate::proc::output_with_timeout(&mut command, Duration::from_secs(600))?
        .ok_or_else(|| anyhow!("tar timed out"))?;
    let _ = std::fs::remove_file(&tar);
    if !out.status.success() {
        return Err(anyhow!("tar failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

fn head(worktree: &Path) -> Result<String> {
    Ok(crate::worktree::git_mutate(worktree, &["rev-parse", "HEAD"])?.trim().to_string())
}

fn lock_hash(root: &Path) -> Result<String> {
    use sha2::Digest;
    let bytes = std::fs::read(root.join("Cargo.lock"))?;
    Ok(crate::hex(&sha2::Sha256::digest(&bytes)))
}

/// `rustc -vV` as `root` would run it: a `rust-toolchain` file there picks the compiler.
fn rustc_version(root: &Path) -> Result<String> {
    let mut command = Command::new("rustc");
    command.arg("-vV").current_dir(root);
    let out = crate::proc::output_with_timeout(&mut command, Duration::from_secs(60))?
        .ok_or_else(|| anyhow!("rustc -vV timed out"))?;
    if !out.status.success() {
        return Err(anyhow!("rustc -vV failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// An exclusive `flock` on `path`, if no one else holds it.
fn try_lock(path: &Path) -> Result<Option<std::fs::File>> {
    use std::os::fd::AsRawFd;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: the descriptor is open for the duration of the call.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        Ok(Some(file))
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EWOULDBLOCK) { Ok(None) } else { Err(error.into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_is_found_past_wrappers_and_inside_compound_lines() {
        for line in [
            "cargo test",
            "cd crates/relay-core && cargo clippy --all-targets -- -D warnings",
            "RUSTFLAGS=-g env nice cargo build -p relay-native",
            "~/.cargo/bin/cargo check",
            "git status; (cargo test -q 2>&1 | tail -3)",
            "bash -c 'cargo build'",
        ] {
            assert!(invokes_cargo(line), "{line}");
        }
        assert!(invokes_cargo("cargo +nightly -q build"));
        for line in [
            "git commit -m 'cargo test passes'", "echo cargo", "ls target", "cat Cargo.toml", "cargo-watch",
            "cargo fmt", "cargo metadata --format-version 1", "cargo tree -p relay-core", "cargo",
        ] {
            assert!(!invokes_cargo(line), "{line}");
        }
    }

    #[test]
    fn a_log_tail_never_cuts_a_character() {
        let text = format!("x{}", "—".repeat(2000));
        for max in 0..8 {
            assert!(tail(&text, max).len() <= max);
        }
        assert_eq!(tail("abc", 10), "abc");
    }

    #[test]
    fn units_under_a_target_triple_are_pruned_too() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let nested = target.join("x86_64-unknown-linux-gnu/debug");
        std::fs::create_dir_all(nested.join(".fingerprint/app-0123456789abcdef")).unwrap();
        std::fs::create_dir_all(nested.join(".fingerprint/serde-0123456789abcdef")).unwrap();
        std::fs::create_dir_all(target.join("debug/.fingerprint/app-0123456789abcdef")).unwrap();
        prune(&target, &["app".to_string()]).unwrap();
        assert!(!nested.join(".fingerprint/app-0123456789abcdef").exists());
        assert!(nested.join(".fingerprint/serde-0123456789abcdef").exists());
        assert!(!target.join("debug/.fingerprint/app-0123456789abcdef").exists());
    }

    #[test]
    fn only_local_packages_lose_their_units() {
        let local = vec!["relay-core".to_string(), "relay".to_string()];
        assert!(is_local_unit("relay-core-0123456789abcdef", &local));
        assert!(is_local_unit("relay-0123456789abcdef", &local));
        // A dependency whose name extends a local one keeps its output.
        assert!(!is_local_unit("relay-core-extra-0123456789abcdef", &local));
        assert!(!is_local_unit("serde-0123456789abcdef", &local));
        assert!(!is_local_unit("relay-core-0123", &local));
    }

    #[test]
    fn pruning_keeps_dependencies_and_drops_the_projects_own_output() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let debug = target.join("debug");
        for dir in [
            ".fingerprint/relay-core-0123456789abcdef", ".fingerprint/serde-fedcba9876543210",
            "build/relay-core-1111111111111111", "build/ring-2222222222222222/out",
            "deps", "incremental/relay_core-3333", "examples",
        ] {
            std::fs::create_dir_all(debug.join(dir)).unwrap();
        }
        for file in ["relay", "relay.d", ".cargo-lock", "deps/libserde-fedcba9876543210.rlib"] {
            std::fs::write(debug.join(file), b"x").unwrap();
        }
        std::fs::write(target.join("CACHEDIR.TAG"), b"x").unwrap();
        prune(&target, &["relay-core".to_string(), "relay".to_string()]).unwrap();
        assert!(debug.join(".fingerprint/serde-fedcba9876543210").is_dir());
        assert!(debug.join("build/ring-2222222222222222/out").is_dir());
        assert!(debug.join("deps/libserde-fedcba9876543210.rlib").is_file());
        assert!(target.join("CACHEDIR.TAG").is_file());
        for gone in [".fingerprint/relay-core-0123456789abcdef", "build/relay-core-1111111111111111", "incremental", "examples", "relay", "relay.d", ".cargo-lock"] {
            assert!(!debug.join(gone).exists(), "{gone}");
        }
    }

    #[test]
    fn metadata_names_local_packages_and_members_outside_the_default_set() {
        let json = br#"{
            "packages": [
                {"name": "app", "id": "path+file:///r/app#0.1.0", "source": null},
                {"name": "gui", "id": "path+file:///r/gui#0.1.0", "source": null},
                {"name": "vendored", "id": "path+file:///r/vendor/v#0.1.0", "source": null},
                {"name": "serde", "id": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0", "source": "registry+https://github.com/rust-lang/crates.io-index"}
            ],
            "workspace_members": ["path+file:///r/app#0.1.0", "path+file:///r/gui#0.1.0"],
            "workspace_default_members": ["path+file:///r/app#0.1.0"]
        }"#;
        let workspace = parse_metadata(json).unwrap();
        assert_eq!(workspace.local, ["app", "gui", "vendored"]);
        assert_eq!(workspace.extra_members, ["gui"]);
        assert_eq!(steps(&workspace).last().unwrap(), &["build", "-p", "gui"]);
    }

    fn seeds_with(repo: &Path, worktree: &Path, waiting: bool, building: Option<Instant>) -> Seeds {
        let seeds = Seeds::default();
        let mut state = seeds.0.state.lock().unwrap();
        let entry = state.entry(repo.to_path_buf()).or_default();
        entry.building = building;
        if waiting {
            entry.waiting.insert(worktree.to_path_buf());
        } else {
            entry.copying.insert(worktree.to_path_buf(), None);
        }
        drop(state);
        seeds
    }

    #[test]
    fn a_worktree_waiting_on_the_first_build_is_refused_with_the_event_to_wait_for() {
        let (repo, worktree) = (Path::new("/r"), Path::new("/r/.relay/worktrees/a"));
        let seeds = seeds_with(repo, worktree, true, Some(Instant::now()));
        let refused = seeds.hold(worktree, COPY_HOLD).unwrap_err();
        assert_eq!(refused.code, "build.seeding");
        assert!(refused.hint.as_deref().unwrap_or_default().contains(READY_EVENT));
        // Another worktree, and the same one once its build is no longer running, pass.
        assert!(seeds.hold(Path::new("/r/.relay/worktrees/b"), COPY_HOLD).is_ok());
        seeds.0.state.lock().unwrap().get_mut(repo).unwrap().building = None;
        assert!(seeds.hold(worktree, COPY_HOLD).is_ok());
    }

    #[test]
    fn a_build_running_past_its_hold_lets_cargo_through() {
        let (repo, worktree) = (Path::new("/r"), Path::new("/r/.relay/worktrees/a"));
        let long_ago = Instant::now().checked_sub(BUILD_HOLD + Duration::from_secs(1));
        let Some(long_ago) = long_ago else { return };
        assert!(seeds_with(repo, worktree, true, Some(long_ago)).hold(worktree, COPY_HOLD).is_ok());
    }

    #[test]
    fn a_cargo_command_waits_for_the_copy_and_no_longer() {
        let (repo, worktree) = (PathBuf::from("/r"), PathBuf::from("/r/.relay/worktrees/a"));
        let seeds = seeds_with(&repo, &worktree, false, None);
        let landing = {
            let (seeds, repo, worktree) = (seeds.clone(), repo.clone(), worktree.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                seeds.settle(&repo, &worktree, false);
            })
        };
        let started = Instant::now();
        seeds.hold(&worktree, Duration::from_secs(10)).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(90), "{:?}", started.elapsed());
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
        landing.join().unwrap();
        // A copy that never lands holds a command only for the hold, then lets it through.
        let stuck = seeds_with(&repo, &worktree, false, None);
        let started = Instant::now();
        stuck.hold(&worktree, Duration::from_millis(50)).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(45));
        assert!(!seeds.tracking() && stuck.tracking());
    }
}
