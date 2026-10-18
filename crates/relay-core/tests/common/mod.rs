//! Helpers the engine's integration tests share (RA-671). Every file under `tests/` is its own
//! crate and pulls these in with `mod common;`.
//!
//! Git is hermetic (RA-672): neither fixture git nor the in-process engine's reads global or
//! system config, so a developer's `core.hooksPath`, `commit.gpgsign` or `init.templateDir`
//! never reaches a test. A test file that uses none of these helpers still needs `mod common;`
//! for that.

// Each test crate uses a different subset of these.
#![allow(dead_code)]

use relay_bus::{Actor, BusError, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Before `main`, while this test process has one thread: its git (fixtures and the engine's own
/// calls alike) reads no global or system config (RA-672), so a developer's `core.hooksPath`,
/// signing or templates never reach a test. The real engine keeps reading the user's config;
/// only test binaries carry this. Nor does the session of an agent running the tests: a
/// fixture's push under its `RELAY_SESSION` would run that agent's push check, `cargo test`
/// inside `cargo test`, where CI runs none.
#[ctor::ctor(unsafe)]
fn hermetic_git() {
    // SAFETY: runs before main, so no other thread can be reading the environment.
    unsafe {
        libc::setenv(c"GIT_CONFIG_GLOBAL".as_ptr(), c"/dev/null".as_ptr(), 1);
        libc::setenv(c"GIT_CONFIG_NOSYSTEM".as_ptr(), c"1".as_ptr(), 1);
        libc::unsetenv(c"RELAY_SESSION".as_ptr());
    }
}

pub const GIT_NAME: &str = "Relay Test";
pub const GIT_EMAIL: &str = "relay@example.test";

/// `git -C repo` with no global or system config and a fixed identity.
pub fn git_command(repo: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", GIT_NAME)
        .env("GIT_AUTHOR_EMAIL", GIT_EMAIL)
        .env("GIT_COMMITTER_NAME", GIT_NAME)
        .env("GIT_COMMITTER_EMAIL", GIT_EMAIL);
    command
}

/// Runs git in `repo` and returns its trimmed stdout; a failure names the arguments and
/// carries git's stderr instead of leaving it interleaved with the other tests' output.
pub fn git(repo: &Path, args: &[&str]) -> String {
    let out = git_command(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A new repository on `main`. Its local config pins what the engine's git, which does read
/// the developer's config, would otherwise take from it: the identity and signing.
pub fn init_repo(repo: &Path) {
    std::fs::create_dir_all(repo).unwrap();
    git(repo, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", GIT_NAME),
        ("user.email", GIT_EMAIL),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
    ] {
        git(repo, &["config", key, value]);
    }
}

/// `init_repo` with `files` (path, text) committed as its first commit.
pub fn committed_repo(repo: &Path, files: &[(&str, &str)]) {
    init_repo(repo);
    for (path, text) in files {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "init"]);
}

/// An engine on an in-memory store.
pub fn engine() -> Arc<Engine> {
    Engine::new(Instance::Test, Store::open_memory().unwrap())
}

/// An engine on a store under `root/store`, with `ws` as workspace 1 and `repo` as project 1.
pub fn engine_with_project(root: &Path, ws: &Path, repo: &Path) -> Arc<Engine> {
    let store = Store::open(&root.join("store/store.db"), false).unwrap();
    let engine = Engine::new(Instance::Test, store);
    ok(&engine, "workspace.create", json!({"path": ws}));
    ok(&engine, "project.add", json!({"workspace_id": 1, "path": repo}));
    engine
}

pub fn call_as(engine: &Engine, actor: Actor, op: &str, payload: Value) -> Response {
    engine.dispatch(Request::new(actor, op, payload), Door::InProcess)
}

/// `call_as` the user.
pub fn call(engine: &Engine, op: &str, payload: Value) -> Response {
    call_as(engine, Actor::User, op, payload)
}

pub fn ok_as(engine: &Engine, actor: Actor, op: &str, payload: Value) -> Value {
    call_as(engine, actor, op, payload)
        .into_result()
        .unwrap_or_else(|error| panic!("{op} failed: {} {}", error.code, error.message))
}

/// `ok_as` the user.
pub fn ok(engine: &Engine, op: &str, payload: Value) -> Value {
    ok_as(engine, Actor::User, op, payload)
}

pub fn err(response: &Response) -> &BusError {
    response.error.as_ref().expect("expected an error response")
}

pub fn refused(response: Response) -> BusError {
    response.error.expect("expected an error response")
}

pub fn code(response: Response) -> String {
    refused(response).code
}

pub fn wait_until(what: &str, f: impl FnMut() -> bool) {
    wait_until_within(what, Duration::from_secs(10), f)
}

/// [`wait_until`] for something that runs a real host tool, which a cold CI runner can take
/// far longer to start than the work takes.
pub fn wait_until_within(what: &str, within: Duration, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + within;
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
