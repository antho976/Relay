//! RA-109: an agent's own commits can skip Relay's pre-commit gate, so `guardrail.gate` refuses
//! the ways of switching the hook off, and `session.done` judges the branch's commits again.

mod common;

use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn call_as(e: &Engine, actor: Actor, op: &str, payload: Value) -> Response {
    e.dispatch(Request::new(actor, op, payload), Door::InProcess)
}
fn ok_as(e: &Engine, actor: Actor, op: &str, payload: Value) -> Value {
    match call_as(e, actor, op, payload).into_result() {
        Ok(v) => v,
        Err(err) => panic!("{op} failed: {} {}", err.code, err.message),
    }
}
fn ok(e: &Engine, op: &str, payload: Value) -> Value {
    ok_as(e, Actor::User, op, payload)
}

fn git(repo: &Path, args: &[&str]) {
    let st = Command::new("git").arg("-C").arg(repo).args(args).status().unwrap();
    assert!(st.success(), "git {args:?}");
}

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
    name: String,
    task: i64,
    worktree: PathBuf,
}

impl Fixture {
    /// A builder session on its own checkout of a one-commit repository, working on a task,
    /// with `secret/**` protected and `manifest.json` shape-gated.
    fn new() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let ws = root.join("ws");
        let repo = ws.join("app");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "t@t"]);
        git(&repo, &["config", "user.name", "t"]);
        std::fs::write(repo.join("README.md"), "hi\n").unwrap();
        std::fs::write(repo.join("manifest.json"), "{\"name\": \"app\"}\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let engine = Engine::new(Instance::Test, Store::open(&root.join("store").join("store.db"), false).unwrap());
        ok(&engine, "workspace.create", json!({"path": ws}));
        ok(&engine, "project.add", json!({"workspace_id": 1, "path": repo}));
        ok(&engine, "guardrail.config.set", json!({"project_id": 1, "patch": {
            "protected_paths": ["secret/**"],
            "shape_gates": [{"path": "manifest.json", "validator": "json_non_empty_object"}],
        }}));
        let task = ok(&engine, "task.create", json!({"project_id": 1, "title": "work"}))["id"].as_i64().unwrap();
        let s = ok(&engine, "session.create", json!({"project_id": 1, "provider": "claude", "task_id": task}));
        let name = s["name"].as_str().unwrap().to_string();
        engine.store.lock().execute("UPDATE tasks SET col='active' WHERE id=?1", [task]).unwrap();
        let worktree = PathBuf::from(s["worktree"].as_str().unwrap());
        ok_as(&engine, Actor::agent(&name), "session.report", json!({"session": name, "kind": "session_start"}));
        Fixture { _root: tmp, engine, name, task, worktree }
    }

    fn agent(&self) -> Actor {
        Actor::agent(&self.name)
    }

    /// Commit `files` (path, text; `None` deletes) past the hook, as an agent would with `-n`.
    fn commit(&self, files: &[(&str, Option<&str>)], message: &str) {
        for (path, text) in files {
            let at = self.worktree.join(path);
            match text {
                Some(text) => {
                    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                    std::fs::write(&at, text).unwrap();
                }
                None => std::fs::remove_file(&at).unwrap(),
            }
        }
        git(&self.worktree, &["add", "-A"]);
        git(&self.worktree, &["commit", "-q", "--no-verify", "-m", message]);
    }

    fn done(&self) -> Response {
        call_as(&self.engine, self.agent(), "session.done", json!({"session": self.name, "summary": "done"}))
    }

    fn column(&self) -> String {
        ok(&self.engine, "task.get", json!({"task_id": self.task}))["column"].as_str().unwrap().to_string()
    }
}

#[test]
fn the_exec_gate_refuses_an_agent_switching_the_commit_hook_off() {
    let f = Fixture::new();
    for command in ["git commit -anm wip", "git commit --no-verify -m wip", "git -c core.hooksPath=/dev/null commit -m wip",
        "git config --worktree --unset core.hooksPath"]
    {
        let refused = call_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": command}))
            .error.unwrap_or_else(|| panic!("{command:?} must be refused"));
        assert_eq!(refused.code, "guardrail.hook_bypass", "{command:?}");
        assert!(refused.hint.unwrap().contains("session.done"), "the hint says why it does not help");
    }
    // The pre-push check, and a hook switched off through an alias.
    for command in ["git push --no-verify origin relay/x", "git -c alias.ci='commit --no-verify' ci -m wip"] {
        let refused = call_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": command}))
            .error.unwrap_or_else(|| panic!("{command:?} must be refused"));
        assert_eq!(refused.code, "guardrail.hook_bypass", "{command:?}");
    }
    ok_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": "git push origin relay/x"}));
    ok_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": "git commit -m 'wip -n'"}));
    // A person's own commit is theirs to make.
    ok(&f.engine, "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": "git commit -n -m wip"}));
}

/// An agent never uploads to Google Play, whatever denied-command list the user stored; a
/// release build that does not publish, and the user's own publish, pass.
#[test]
fn the_exec_gate_refuses_an_agent_publishing_to_play() {
    let f = Fixture::new();
    ok(&f.engine, "guardrail.config.set", json!({"patch": {"denied_commands": ["rm -rf"]}}));
    for command in ["./gradlew publishReleaseBundle", "./gradlew bundleRelease :app:promoteReleaseArtifact", "cd android && ./gradlew pRB"] {
        let refused = call_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": command}))
            .error.unwrap_or_else(|| panic!("{command:?} must be refused"));
        assert_eq!(refused.code, "guardrail.play_publish", "{command:?}");
    }
    ok_as(&f.engine, f.agent(), "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": "./gradlew bundleRelease"}));
    ok(&f.engine, "guardrail.gate", json!({"session": f.name, "kind": "exec", "command": "./gradlew publishReleaseBundle"}));
}

#[test]
fn done_judges_every_commit_against_the_protected_paths() {
    let f = Fixture::new();
    // Added in one commit and gone again in the next: the net diff never shows it.
    f.commit(&[("secret/key", Some("hunter2\n")), ("src/a.txt", Some("a\n"))], "add a key");
    f.commit(&[("secret/key", None)], "drop the key");
    let refused = f.done().error.expect("a commit touching a protected path refuses the done");
    assert_eq!(refused.code, "guardrail.protected_path");
    let details = refused.details.unwrap();
    assert_eq!(details["path"], "secret/key");
    assert_eq!(details["commit"].as_str().unwrap().len(), 40, "{details}");
    assert!(refused.message.starts_with("commit "), "{}", refused.message);
    assert_eq!(f.column(), "active");
    // Saying it is stuck is never refused.
    ok_as(&f.engine, f.agent(), "session.done",
        json!({"session": f.name, "status": "partial", "blockers": ["a key slipped into history"]}));
}

/// A `blocked` done leaves the turn's done pending; a `completed` one after it, in the same turn,
/// skipped the branch re-check and took a protected path to review.
#[test]
fn reporting_blocked_first_does_not_skip_the_recheck() {
    let f = Fixture::new();
    f.commit(&[("secret/key", Some("hunter2\n"))], "add a key");
    ok_as(&f.engine, f.agent(), "session.done",
        json!({"session": f.name, "status": "blocked", "blockers": ["waiting on a review"]}));
    let refused = f.done().error.expect("the completed done after a blocked one is still re-checked");
    assert_eq!(refused.code, "guardrail.protected_path");
    assert_ne!(f.column(), "review");
}

/// The same sequence on a clean branch completes, as a plain completed done does.
#[test]
fn a_completed_done_after_a_blocked_one_on_clean_work_completes() {
    let f = Fixture::new();
    f.commit(&[("src/a.txt", Some("a\n"))], "work");
    ok_as(&f.engine, f.agent(), "session.done",
        json!({"session": f.name, "status": "blocked", "blockers": ["waiting on a review"]}));
    assert!(f.done().ok, "a clean branch completes after a blocked report");
    let plain = Fixture::new();
    plain.commit(&[("src/a.txt", Some("a\n"))], "work");
    assert!(plain.done().ok);
    assert_eq!(plain.column(), f.column());
}

#[test]
fn a_granted_path_passes_and_the_done_does_not_spend_the_grant() {
    let f = Fixture::new();
    f.commit(&[("secret/key", Some("rotated\n"))], "rotate the key");
    assert_eq!(f.done().error.unwrap().code, "guardrail.protected_path");
    let id = ok_as(&f.engine, f.agent(), "guardrail.request", json!({
        "session": f.name, "kind": "path", "value": "secret/key", "reason": "the task is rotating it",
    }))["request"]["id"].as_i64().unwrap();
    ok(&f.engine, "guardrail.confirm", json!({"hold_id": id}));
    f.done().into_result().expect("the person let this path through");
    assert_eq!(f.column(), "in_review");
    let grant = ok(&f.engine, "guardrail.request.get", json!({"request_id": id}));
    assert_eq!((grant["uses"].as_u64(), grant["active"].as_bool()), (Some(0), Some(true)), "{grant}");
}

#[test]
fn a_once_grant_the_commit_gate_used_still_covers_that_commit_at_done() {
    let f = Fixture::new();
    let id = ok_as(&f.engine, f.agent(), "guardrail.request", json!({
        "session": f.name, "kind": "path", "value": "secret/key", "reason": "rotating it", "scope": "once",
    }))["request"]["id"].as_i64().unwrap();
    ok(&f.engine, "guardrail.confirm", json!({"hold_id": id}));
    // The hook's gate lets the commit through on the grant, and spends it.
    ok_as(&f.engine, f.agent(), "guardrail.gate",
        json!({"session": f.name, "kind": "commit", "diff": "1\t0\tsecret/key\n"}));
    assert_eq!(ok(&f.engine, "guardrail.request.get", json!({"request_id": id}))["active"], false);
    f.commit(&[("secret/key", Some("rotated\n"))], "rotate the key");
    f.done().into_result().expect("the commit the grant was used on is not refused again");
}

#[test]
fn done_judges_shape_gated_files_as_the_branch_leaves_them() {
    let f = Fixture::new();
    f.commit(&[("manifest.json", Some("{}\n"))], "empty the manifest");
    let refused = f.done().error.expect("a manifest that fails its validator refuses the done");
    assert_eq!(refused.code, "guardrail.shape_gate");
    assert_eq!(refused.details.unwrap()["path"], "manifest.json");
    f.commit(&[("manifest.json", Some("{\"name\": \"app\", \"v\": 2}\n"))], "restore it");
    f.done().into_result().expect("the branch leaves it valid");
}

/// Work git cannot measure is not taken as within limits: an agent can break the measurement
/// on purpose, so the done is refused until it can be taken, and blocked stays sayable.
#[test]
fn done_refuses_work_git_cannot_measure() {
    let f = Fixture::new();
    f.commit(&[("secret/key", Some("x\n"))], "touch the key");
    // A base that no longer exists: nothing to measure against.
    f.engine.store.lock().execute("UPDATE projects SET base_branch='gone' WHERE id=1", []).unwrap();
    let refused = f.done().error.expect("a branch Relay cannot measure is not handed to review");
    assert_eq!(refused.code, "guardrail.unmeasured", "{}", refused.message);
    assert!(refused.hint.unwrap().contains("blocked"));
    assert_eq!(f.column(), "active");
    ok_as(&f.engine, f.agent(), "session.done",
        json!({"session": f.name, "status": "blocked", "blockers": ["the base branch is gone"]}));
}

#[test]
fn a_config_git_rejects_or_an_orphan_branch_does_not_wave_the_done_through() {
    let f = Fixture::new();
    f.commit(&[("secret/key", Some("x\n"))], "touch the key");
    git(&f.worktree, &["config", "core.quotePath", "notabool"]);
    assert_eq!(f.done().error.expect("a broken config refuses").code, "guardrail.unmeasured");
    git(&f.worktree, &["config", "--unset", "core.quotePath"]);
    assert_eq!(f.done().error.expect("measured again").code, "guardrail.protected_path");

    let g = Fixture::new();
    git(&g.worktree, &["checkout", "-q", "--orphan", "fresh"]);
    g.commit(&[("secret/key", Some("x\n"))], "no history in common with main");
    assert_eq!(g.done().error.expect("an orphan branch refuses").code, "guardrail.unmeasured");
    assert_eq!(g.column(), "active");
}

/// Settings the checkout may hold that change what git prints do not change the measurement.
#[test]
fn the_measurement_pins_the_config_it_reads_by() {
    let f = Fixture::new();
    f.commit(&[("secret/kéy", Some("x\n"))], "a key with an accent");
    git(&f.worktree, &["config", "core.quotePath", "false"]);
    git(&f.worktree, &["config", "diff.noprefix", "true"]);
    git(&f.worktree, &["config", "color.ui", "always"]);
    let refused = f.done().error.expect("still seen");
    assert_eq!(refused.code, "guardrail.protected_path");
    assert_eq!(refused.details.unwrap()["path"], "secret/kéy");
}

/// An agent hands its task to review only through session.done, which judges the branch: a
/// task.move straight to in_review skipped the check, the other builders and the reviewer.
#[test]
fn an_agent_cannot_move_its_task_to_review_past_the_done() {
    let f = Fixture::new();
    f.commit(&[("secret/key", Some("x\n"))], "touch the key");
    let moved = call_as(&f.engine, f.agent(), "task.move", json!({"task_id": f.task, "column": "in_review"}))
        .error.expect("an agent's move is refused");
    // The default builder allowlist no longer lists it; a config that still does meets the handler.
    assert_eq!(moved.code, "actor.allowlist", "{}", moved.message);
    ok(&f.engine, "guardrail.config.set", json!({"project_id": 1, "patch": {"roles": {"builder": ["task.move", "session.done"]}}}));
    let moved = call_as(&f.engine, f.agent(), "task.move", json!({"task_id": f.task, "column": "in_review"}))
        .error.expect("still refused with the op allowed");
    assert_eq!(moved.code, "task.column_transition");
    assert!(moved.hint.unwrap().contains("session.done"));
    assert_eq!(f.column(), "active");
    assert_eq!(f.done().error.unwrap().code, "guardrail.protected_path");
    // The person still moves it as they like.
    ok(&f.engine, "task.move", json!({"task_id": f.task, "column": "in_review"}));
    assert_eq!(f.column(), "in_review");
}

/// Relay fetches but never moves a local base, so `main` can sit far behind `origin/main`. A
/// session cut from the remote tip is measured against that tip at done: other people's merged
/// work (here a protected path and more lines than the cap) is not the task's.
#[test]
fn done_measures_against_the_remote_base_when_the_local_one_is_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let upstream = root.join("upstream");
    std::fs::create_dir_all(&upstream).unwrap();
    git(&upstream, &["init", "-q", "-b", "main"]);
    git(&upstream, &["config", "user.email", "t@t"]);
    git(&upstream, &["config", "user.name", "t"]);
    std::fs::write(upstream.join("README.md"), "hi\n").unwrap();
    git(&upstream, &["add", "."]);
    git(&upstream, &["commit", "-q", "-m", "init"]);
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let repo = ws.join("app");
    git(&ws, &["clone", "-q", upstream.to_str().unwrap(), repo.to_str().unwrap()]);
    git(&repo, &["config", "user.email", "t@t"]);
    git(&repo, &["config", "user.name", "t"]);
    // Merged by someone else after the clone, and fetched: local main stays where it was.
    std::fs::create_dir_all(upstream.join("secret")).unwrap();
    std::fs::write(upstream.join("secret/key"), "theirs\n").unwrap();
    std::fs::write(upstream.join("big.txt"), "line\n".repeat(2500)).unwrap();
    git(&upstream, &["add", "."]);
    git(&upstream, &["commit", "-q", "-m", "someone else's work"]);
    git(&repo, &["fetch", "-q", "origin"]);

    let engine = Engine::new(Instance::Test, Store::open(&root.join("store").join("store.db"), false).unwrap());
    ok(&engine, "workspace.create", json!({"path": ws}));
    ok(&engine, "project.add", json!({"workspace_id": 1, "path": repo}));
    ok(&engine, "guardrail.config.set", json!({"project_id": 1, "patch": {"protected_paths": ["secret/**"]}}));
    let task = ok(&engine, "task.create", json!({"project_id": 1, "title": "work"}))["id"].as_i64().unwrap();
    let s = ok(&engine, "session.create", json!({"project_id": 1, "provider": "claude", "task_id": task}));
    let name = s["name"].as_str().unwrap().to_string();
    engine.store.lock().execute("UPDATE tasks SET col='active' WHERE id=?1", [task]).unwrap();
    let worktree = PathBuf::from(s["worktree"].as_str().unwrap());
    assert!(worktree.join("secret/key").exists(), "the session starts from the remote tip");
    ok_as(&engine, Actor::agent(&name), "session.report", json!({"session": name, "kind": "session_start"}));
    std::fs::write(worktree.join("mine.txt"), "small\n").unwrap();
    git(&worktree, &["add", "-A"]);
    git(&worktree, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--no-verify", "-m", "my work"]);

    ok_as(&engine, Actor::agent(&name), "session.done", json!({"session": name, "summary": "done"}));
    assert_eq!(ok(&engine, "task.get", json!({"task_id": task}))["column"], "in_review");
}
