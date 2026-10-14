//! `session.transcript`: an agent's conversation read from its provider's log, as the phone
//! draws a terminal agent. One test in its own binary: it points `CLAUDE_CONFIG_DIR` at a
//! temporary directory, which no other test in this process may see.

use relay_bus::{Actor, Request};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

fn ok(e: &Engine, actor: Actor, op: &str, payload: Value) -> Value {
    match e.dispatch(Request::new(actor, op, payload), Door::InProcess).into_result() {
        Ok(v) => v,
        Err(err) => panic!("{op} failed: {} {}", err.code, err.message),
    }
}

fn git(repo: &Path, args: &[&str]) {
    assert!(Command::new("git").arg("-C").arg(repo).args(args).status().unwrap().success(), "git {args:?}");
}

#[test]
fn a_claude_agent_reads_as_a_conversation_from_its_cursor_on() {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let config = root.join("claude");
    std::env::set_var("CLAUDE_CONFIG_DIR", &config);
    let repo = root.join("ws").join("app");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    let e = Engine::new(Instance::Test, Store::open(&root.join("store").join("store.db"), false).unwrap());
    ok(&e, Actor::User, "workspace.create", json!({"path": root.join("ws")}));
    ok(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": repo}));
    let s = ok(&e, Actor::User, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    let worktree = s["worktree"].as_str().unwrap().to_string();

    // Before Claude Code's first hook names its conversation there is nothing to read.
    let none = ok(&e, Actor::User, "session.transcript", json!({"session": name}));
    assert_eq!(none["found"], false);
    assert_eq!(none["messages"], json!([]));

    ok(&e, Actor::agent(&name), "session.report", json!({"session": name, "kind": "session_start", "data": {"session_id": "c0ffee-1"}}));
    let slug: String = worktree.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let dir = config.join("projects").join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("c0ffee-1.jsonl");
    let line = |v: Value| format!("{v}\n");
    std::fs::write(&log, [
        line(json!({"type": "user", "uuid": "u1", "timestamp": "2026-10-13T10:00:00Z", "message": {"role": "user", "content": "run the tests"}})),
        line(json!({"type": "assistant", "uuid": "a1", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "cargo test"}}]}})),
    ].concat()).unwrap();

    let first = ok(&e, Actor::User, "session.transcript", json!({"session": name}));
    assert_eq!(first["found"], true);
    assert_eq!(first["messages"].as_array().unwrap().len(), 2);
    assert_eq!(first["messages"][1]["body"]["blocks"][0]["summary"], "cargo test");

    use std::io::Write;
    std::fs::OpenOptions::new().append(true).open(&log).unwrap().write_all(line(json!({
        "type": "user", "uuid": "r1", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "test result: ok"}]},
    })).as_bytes()).unwrap();
    let next = ok(&e, Actor::User, "session.transcript", json!({"session": name, "after": first["cursor"]}));
    assert_eq!(next["reset"], false);
    let messages = next["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[0]["body"]["text"], "test result: ok");

    // Another agent may not read it.
    let other = ok(&e, Actor::User, "session.create", json!({"project_id": 1, "provider": "codex"}))["name"].as_str().unwrap().to_string();
    let refused = e.dispatch(Request::new(Actor::agent(&other), "session.transcript", json!({"session": name})), Door::InProcess).into_result();
    assert!(refused.is_err(), "{refused:?}");
}
