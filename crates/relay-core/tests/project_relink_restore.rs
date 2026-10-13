//! A project whose repository moved is relinked rather than removed and re-added, and a removed
//! project comes back from the backup its removal took, with its original ids (RA-195).

mod common;

use relay_bus::{Actor, BusError, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn call(e: &Engine, op: &str, payload: Value) -> Response {
    e.dispatch(Request::new(Actor::User, op, payload), Door::InProcess)
}
fn ok(e: &Engine, op: &str, payload: Value) -> Value {
    match call(e, op, payload).into_result() {
        Ok(v) => v,
        Err(err) => panic!("{op} failed: {} {}", err.code, err.message),
    }
}
fn refused(r: Response) -> BusError {
    r.error.expect("expected an error")
}

fn git(repo: &Path, args: &[&str]) -> String {
    let st = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(st.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&st.stderr));
    String::from_utf8_lossy(&st.stdout).trim().to_string()
}

fn repo_in(ws: &Path, name: &str) -> PathBuf {
    let repo = ws.join(name);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@t"]);
    git(&repo, &["config", "user.name", "t"]);
    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    repo
}

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    ws: PathBuf,
    repo: PathBuf,
    engine: Arc<Engine>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let ws = root.join("ws");
    let repo = repo_in(&ws, "app");
    let store = Store::open(&root.join("store").join("store.db"), false).unwrap();
    let engine = Engine::new(Instance::Test, store);
    ok(&engine, "workspace.create", json!({"path": ws}));
    ok(&engine, "project.add", json!({"workspace_id": 1, "path": repo}));
    Fixture { _tmp: tmp, root, ws, repo, engine }
}

fn count(e: &Engine, sql: &str) -> i64 {
    e.store.lock().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn ids(e: &Engine, sql: &str) -> Vec<i64> {
    let conn = e.store.lock();
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<Vec<i64>>>().unwrap();
    rows
}

#[test]
fn relink_follows_a_moved_repository_and_its_closed_agents_worktrees() {
    let f = fixture();
    let e = &f.engine;
    let session = ok(e, "session.create", json!({"project_id": 1, "provider": "codex", "role": "builder"}));
    let name = session["name"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(session["worktree"].as_str().unwrap());
    assert!(worktree.starts_with(&f.repo), "{}", worktree.display());
    ok(e, "task.create", json!({"project_id": 1, "title": "survives the move"}));
    ok(e, "session.close", json!({"session": name, "remove_worktree": false}));

    let moved = f.ws.join("renamed");
    std::fs::rename(&f.repo, &moved).unwrap();
    let out = ok(e, "project.relink", json!({"project_id": 1, "path": moved}));
    assert_eq!(out["path"], moved.display().to_string());
    assert_eq!(out["id"], 1);
    assert_eq!(count(e, "SELECT COUNT(*) FROM tasks WHERE project_id = 1"), 1);
    let stored: String = e.store.lock().query_row("SELECT worktree FROM sessions WHERE name = ?1", [&name], |r| r.get(0)).unwrap();
    let relocated = moved.join(worktree.strip_prefix(&f.repo).unwrap());
    assert_eq!(PathBuf::from(&stored), relocated);
    // `git worktree repair` ran once the store unlocked: the checkout works from its new place.
    assert_eq!(git(&relocated, &["rev-parse", "--show-toplevel"]), relocated.display().to_string());

    // Undoable while the old location is a repository again.
    std::fs::rename(&moved, &f.repo).unwrap();
    let audit = ok(e, "audit.list", json!({"op_prefix": "project.relink"}));
    let id = audit["rows"][0]["id"].as_i64().unwrap();
    ok(e, "audit.undo", json!({"audit_id": id}));
    assert_eq!(ok(e, "project.get", json!({"project_id": 1}))["path"], f.repo.display().to_string());
}

#[test]
fn relink_refuses_while_agents_are_open_and_for_a_directory_that_is_not_a_repository() {
    let f = fixture();
    let e = &f.engine;
    let plain = f.ws.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(refused(call(e, "project.relink", json!({"project_id": 1, "path": plain}))).code, "project.path");

    let other = repo_in(&f.ws, "other");
    ok(e, "project.add", json!({"workspace_id": 1, "path": other}));
    assert_eq!(refused(call(e, "project.relink", json!({"project_id": 1, "path": other}))).code, "project.exists");

    let outside = repo_in(&f.root, "elsewhere");
    assert_eq!(refused(call(e, "project.relink", json!({"project_id": 1, "path": outside}))).code, "project.outside_workspace");

    ok(e, "session.create", json!({"project_id": 1, "provider": "codex", "role": "builder"}));
    let fresh = repo_in(&f.ws, "fresh");
    assert_eq!(refused(call(e, "project.relink", json!({"project_id": 1, "path": fresh}))).code, "project.sessions_live");
    assert_eq!(ok(e, "project.get", json!({"project_id": 1}))["path"], f.repo.display().to_string());
}

#[test]
fn a_removed_project_comes_back_from_its_backup_with_its_original_ids() {
    let f = fixture();
    let e = &f.engine;
    // Something in the store before it, so the restored ids are not simply 1, 2, 3.
    let other = repo_in(&f.ws, "other");
    ok(e, "project.add", json!({"workspace_id": 1, "path": other}));
    ok(e, "task.create", json!({"project_id": 2, "title": "elsewhere"}));
    let module = ok(e, "module.create", json!({"project_id": 1, "name": "Core"}));
    let parent = ok(e, "task.create", json!({"project_id": 1, "title": "parent", "module_id": module["id"], "labels": ["ui"]}));
    let child = ok(e, "task.create", json!({"project_id": 1, "title": "child", "parent_id": parent["id"]}));
    ok(e, "task.relate", json!({"task_id": child["id"], "relation": "blocked_by", "other_id": parent["id"]}));
    ok(e, "notes.create", json!({"project_id": 1, "title": "plan", "body": "precious"}));
    ok(e, "notes.create", json!({"project_id": 1, "title": "more", "body": "also"}));
    ok(e, "settings.set", json!({"path": "guardrails.projects.1.caps", "value": {"files": 3, "lines": 30}}));
    let preview = ok(e, "project.remove.preview", json!({"project_id": 1}));
    assert_eq!((preview["tasks"].as_i64(), preview["notes"].as_i64(), preview["modules"].as_i64()), (Some(2), Some(2), Some(1)));

    let task_ids = ids(e, "SELECT id FROM tasks WHERE project_id = 1 ORDER BY id");
    let note_ids = ids(e, "SELECT id FROM notes WHERE project_id = 1 ORDER BY id");
    let module_ids = ids(e, "SELECT id FROM modules WHERE project_id = 1 ORDER BY id");
    let labels = count(e, "SELECT COUNT(*) FROM task_labels");
    ok(e, "project.remove", json!({"project_id": 1}));
    assert_eq!(count(e, "SELECT COUNT(*) FROM tasks WHERE project_id = 1"), 0);
    // Something made since the removal keeps its own ids.
    ok(e, "task.create", json!({"project_id": 2, "title": "made after"}));

    let listed = ok(e, "project.removed.list", json!({}));
    let removed = listed["removed"].as_array().unwrap();
    assert_eq!(removed.len(), 1, "{listed}");
    assert_eq!(removed[0]["project_id"], 1);
    assert_eq!(removed[0]["path"], f.repo.display().to_string());
    let backup = removed[0]["backup_path"].as_str().unwrap().to_string();

    let out = ok(e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    assert_eq!((out["tasks"].as_i64(), out["notes"].as_i64(), out["modules"].as_i64()), (Some(2), Some(2), Some(1)));
    assert_eq!(out["project"]["id"], 1);
    assert_eq!(out["workspace_restored"], false);
    assert_eq!(ids(e, "SELECT id FROM tasks WHERE project_id = 1 ORDER BY id"), task_ids);
    assert_eq!(ids(e, "SELECT id FROM notes WHERE project_id = 1 ORDER BY id"), note_ids);
    assert_eq!(ids(e, "SELECT id FROM modules WHERE project_id = 1 ORDER BY id"), module_ids);
    assert_eq!(count(e, "SELECT COUNT(*) FROM task_labels"), labels);
    assert_eq!(count(e, "SELECT COUNT(*) FROM task_relations"), 1);
    assert_eq!(count(e, "SELECT COUNT(*) FROM settings WHERE path LIKE 'guardrails.projects.1.%'"), 2);
    let restored_child = ok(e, "task.get", json!({"task_id": child["id"]}));
    assert_eq!(restored_child["parent_id"], parent["id"]);
    assert_eq!(ok(e, "task.get", json!({"task_id": parent["id"]}))["module_id"], module["id"]);
    assert!(ok(e, "project.removed.list", json!({}))["removed"].as_array().unwrap().is_empty());

    // Already back: nothing to restore over.
    assert_eq!(refused(call(e, "project.restore", json!({"backup_path": backup, "project_id": 1}))).code, "project.exists");
    // A next project still gets a fresh id.
    let third = repo_in(&f.ws, "third");
    assert_eq!(ok(e, "project.add", json!({"workspace_id": 1, "path": third}))["id"], 3);
}

#[test]
fn restore_reads_only_the_backups_directory_and_refuses_a_taken_path() {
    let f = fixture();
    let e = &f.engine;
    ok(e, "notes.create", json!({"project_id": 1, "title": "plan", "body": "precious"}));
    ok(e, "project.remove", json!({"project_id": 1}));
    let backup = ok(e, "project.removed.list", json!({}))["removed"][0]["backup_path"].as_str().unwrap().to_string();

    let outside = f.root.join("store-copy.db");
    std::fs::copy(&backup, &outside).unwrap();
    assert_eq!(refused(call(e, "project.restore", json!({"backup_path": outside, "project_id": 1}))).code, "project.backup_path");
    let sneaky = format!("{}/../store-copy.db", Path::new(&backup).parent().unwrap().display());
    assert_eq!(refused(call(e, "project.restore", json!({"backup_path": sneaky, "project_id": 1}))).code, "project.backup_path");
    assert_eq!(refused(call(e, "project.restore", json!({"backup_path": backup, "project_id": 9}))).code, "project.not_in_backup");

    // The repository was added again in the meantime: restoring would make two projects of it.
    ok(e, "project.add", json!({"workspace_id": 1, "path": f.repo}));
    assert_eq!(refused(call(e, "project.restore", json!({"backup_path": backup, "project_id": 1}))).code, "project.exists");
    assert_eq!(count(e, "SELECT COUNT(*) FROM notes"), 0);
}

#[test]
fn a_project_removed_with_its_workspace_brings_the_workspace_back() {
    let f = fixture();
    let e = &f.engine;
    ok(e, "task.create", json!({"project_id": 1, "title": "kept"}));
    ok(e, "workspace.remove", json!({"workspace_id": 1, "force": true}));
    let listed = ok(e, "project.removed.list", json!({}));
    assert_eq!(listed["removed"][0]["reason"], "workspace-remove");
    let backup = listed["removed"][0]["backup_path"].clone();
    let out = ok(e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    assert_eq!(out["workspace_restored"], true);
    assert_eq!(out["tasks"], 1);
    assert_eq!(ok(e, "workspace.list", json!({}))["workspaces"][0]["id"], 1);
    assert_eq!(ok(e, "project.get", json!({"project_id": 1}))["workspace_id"], 1);
}

/// A `project-remove` backup is the only copy of the project it was taken for. Pruned by count,
/// five were kept, and the sixth removal silently dropped the first project from
/// `project.removed.list` (and let recovery sweep its attachment files). One holding the newest
/// copy of a project still removed now stays; one whose projects are back, or held by a newer
/// backup, still goes once past the five.
#[test]
fn removal_backups_outlive_the_count_while_they_hold_a_removed_project() {
    let f = fixture();
    let e = &f.engine;
    for id in 2..=8 {
        let repo = repo_in(&f.ws, &format!("p{id}"));
        assert_eq!(ok(e, "project.add", json!({"workspace_id": 1, "path": repo}))["id"], id);
    }
    let removal_backups = || e.store.list_backups().unwrap().into_iter().filter(|b| b.reason == "project-remove").count();
    let removed = || {
        let listed = ok(e, "project.removed.list", json!({}));
        let mut ids: Vec<i64> = listed["removed"].as_array().unwrap().iter().map(|r| r["project_id"].as_i64().unwrap()).collect();
        ids.sort();
        ids
    };
    for id in 1..=7 {
        ok(e, "project.remove", json!({"project_id": id}));
    }
    assert_eq!(removal_backups(), 7);
    assert_eq!(removed(), (1..=7).collect::<Vec<i64>>());

    // Project 1 back, its backup holds only projects a newer backup holds too: past the five, it goes.
    let listed = ok(e, "project.removed.list", json!({}));
    let first = listed["removed"].as_array().unwrap().iter().find(|r| r["project_id"] == 1).unwrap()["backup_path"].as_str().unwrap().to_string();
    ok(e, "project.restore", json!({"backup_path": first, "project_id": 1}));
    ok(e, "project.remove", json!({"project_id": 8}));
    assert!(!Path::new(&first).exists(), "a backup no removed project needs was kept");
    assert_eq!(removal_backups(), 7);
    assert_eq!(removed(), (2..=8).collect::<Vec<i64>>());
}

/// The base branch reaches git's command line: one that would read as an option is refused.
#[test]
fn project_update_refuses_a_base_branch_git_would_read_as_an_option() {
    let f = fixture();
    let e = &f.engine;
    for bad in ["--output=/tmp/owned", "-b", "a..b", "has space"] {
        let error = refused(call(e, "project.update", json!({"project_id": 1, "base_branch": bad})));
        assert_eq!(error.code, "project.base_branch", "{bad}: {}", error.message);
    }
    assert_eq!(ok(e, "project.get", json!({"project_id": 1}))["base_branch"], "main");
    assert_eq!(ok(e, "project.update", json!({"project_id": 1, "base_branch": "release/2.0"}))["base_branch"], "release/2.0");
}
