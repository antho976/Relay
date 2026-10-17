//! `guardrails.check_on_push`: a project added with a build tool at its root starts with that
//! tool's tests as its check, and the switch is on for every project until a person turns it off.

use relay_bus::{Actor, Request};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(repo).args(args).status().unwrap();
    assert!(status.success(), "git {args:?}");
}

fn ok(engine: &Engine, op: &str, payload: Value) -> Value {
    engine.dispatch(Request::new(Actor::User, op, payload), Door::InProcess)
        .into_result()
        .unwrap_or_else(|error| panic!("{op} failed: {} {}", error.code, error.message))
}

#[test]
fn a_new_project_starts_with_its_build_tools_tests_and_the_check_on() {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let engine = Engine::new(Instance::Test, Store::open(&root.path().join("store/store.db"), false).unwrap());
    for (name, file) in [("crate", "Cargo.toml"), ("notes", "README.md")] {
        let repo = ws.join(name);
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join(file), "").unwrap();
    }
    ok(&engine, "workspace.create", json!({"path": ws}));
    let rust = ok(&engine, "project.add", json!({"workspace_id": 1, "path": ws.join("crate")}));
    assert_eq!(rust["build_cmd"], "cargo test");
    let notes = ok(&engine, "project.add", json!({"workspace_id": 1, "path": ws.join("notes")}));
    assert!(notes["build_cmd"].is_null(), "a checkout with no build tool was given a check: {notes}");

    let id = rust["id"].clone();
    assert_eq!(ok(&engine, "guardrail.config.get", json!({"project_id": id}))["check_on_push"], true);
    ok(&engine, "guardrail.config.set", json!({"project_id": id, "patch": {"check_on_push": false}}));
    assert_eq!(ok(&engine, "guardrail.config.get", json!({"project_id": id}))["check_on_push"], false);
    assert_eq!(ok(&engine, "guardrail.config.get", json!({"project_id": notes["id"]}))["check_on_push"], true,
        "one project's switch reached another");
}
