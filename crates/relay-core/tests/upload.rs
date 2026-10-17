//! `session.upload`: a picture from the phone for an agent, put in its checkout where git never
//! sees it, and only by the person.

use relay_bus::{Actor, Request};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// A 1×1 PNG.
const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn ok(e: &Engine, actor: Actor, op: &str, payload: Value) -> Value {
    match e.dispatch(Request::new(actor, op, payload), Door::InProcess).into_result() {
        Ok(v) => v,
        Err(err) => panic!("{op} failed: {} {}", err.code, err.message),
    }
}

/// The refusal's code, for a request that must be refused.
fn refused(e: &Engine, actor: Actor, op: &str, payload: Value) -> String {
    match e.dispatch(Request::new(actor, op, payload), Door::InProcess).into_result() {
        Ok(v) => panic!("{op} was taken: {v}"),
        Err(err) => err.code.to_string(),
    }
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn a_picture_lands_in_the_checkout_out_of_git_and_only_the_person_sends_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let repo = root.join("ws").join("app");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    let e = Engine::new(Instance::Test, Store::open(&root.join("store").join("store.db"), false).unwrap());
    ok(&e, Actor::User, "workspace.create", json!({"path": root.join("ws")}));
    ok(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": repo}));
    let s = ok(&e, Actor::User, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    let worktree = Path::new(s["worktree"].as_str().unwrap()).to_path_buf();

    let out = ok(&e, Actor::User, "session.upload", json!({"session": name, "bytes_b64": PIXEL}));
    let path = out["path"].as_str().unwrap();
    assert!(path.starts_with(".relay/attachments/phone-") && path.ends_with(".png"), "{path}");
    assert_eq!((out["w"].as_u64(), out["h"].as_u64()), (Some(1), Some(1)));
    assert_eq!(Path::new(out["absolute"].as_str().unwrap()), worktree.join(path));
    assert!(worktree.join(path).is_file());
    // Git never sees it, so it is never committed with the agent's work.
    assert_eq!(git(&worktree, &["status", "--porcelain", "--untracked-files=all"]).trim(), "");
    // The same picture again is the same file.
    assert_eq!(ok(&e, Actor::User, "session.upload", json!({"session": name, "bytes_b64": PIXEL}))["path"], out["path"]);

    assert_eq!(refused(&e, Actor::User, "session.upload", json!({"session": name, "bytes_b64": "aGVsbG8="})), "session.upload");
    // An agent may not send itself a picture.
    refused(&e, Actor::agent(&name), "session.upload", json!({"session": name, "bytes_b64": PIXEL}));

    // A link the agent planted where the pictures go is not written through.
    let elsewhere = root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::remove_dir_all(worktree.join(".relay/attachments")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, worktree.join(".relay/attachments")).unwrap();
    assert_eq!(refused(&e, Actor::User, "session.upload", json!({"session": name, "bytes_b64": PIXEL})), "session.upload_path");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}
