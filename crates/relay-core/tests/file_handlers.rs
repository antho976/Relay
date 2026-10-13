//! `file.*` handler fixes from the audit: restore is gated (RA-362), restore_head restores
//! from HEAD (RA-364), deletes never reuse a trash slot (RA-361), a tree survives an unreadable
//! folder (RA-368), and a missing path is `not_found` (RA-369).

use relay_bus::{Actor, BusError, ErrorKind, Request, Response};
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
    call(e, op, payload).into_result().unwrap_or_else(|error| panic!("{op}: {error:?}"))
}
fn err(r: &Response) -> &BusError {
    r.error.as_ref().expect("expected an error response")
}
fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
}
fn project() -> (Arc<Engine>, tempfile::TempDir, PathBuf) {
    let e = Engine::new(Instance::Test, Store::open_memory().unwrap());
    let ws = tempfile::tempdir().unwrap();
    let repo = ws.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Relay Test"]);
    git(&repo, &["config", "user.email", "relay@example.test"]);
    std::fs::write(repo.join("README.md"), "# Relay\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-m", "Initial"]);
    let repo = std::fs::canonicalize(&repo).unwrap();
    ok(&e, "workspace.create", json!({"path": std::fs::canonicalize(ws.path()).unwrap()}));
    ok(&e, "project.add", json!({"workspace_id": 1, "path": repo}));
    (e, ws, repo)
}
fn confirm(e: &Engine, held: &Response) {
    assert_eq!(err(held).kind, ErrorKind::Held, "{held:?}");
    let hold_id = err(held).confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();
    assert_eq!(ok(e, "guardrail.confirm", json!({"hold_id": hold_id}))["outcome"]["ok"], true);
}

/// RA-362: putting a protected file back is gated like deleting it was.
#[test]
fn restoring_a_protected_path_is_held_like_any_other_write() {
    let (e, _ws, repo) = project();
    std::fs::write(repo.join("prod.env"), "SECRET=1\n").unwrap();
    ok(&e, "project.update", json!({"project_id": 1, "protected_paths": ["prod.env"]}));
    confirm(&e, &call(&e, "file.delete", json!({"project_id": 1, "path": "prod.env"})));
    assert!(!repo.join("prod.env").exists());
    let trash_id = ok(&e, "file.trash.list", json!({"project_id": 1}))["entries"][0]["id"].clone();
    let held = call(&e, "file.restore", json!({"project_id": 1, "trash_id": trash_id}));
    assert!(!repo.join("prod.env").exists(), "nothing moves before the hold is confirmed");
    confirm(&e, &held);
    assert_eq!(std::fs::read_to_string(repo.join("prod.env")).unwrap(), "SECRET=1\n");
    let again = call(&e, "file.restore", json!({"project_id": 1, "trash_id": trash_id}));
    assert_eq!(err(&again).code, "file.trash_not_found");
}

/// RA-361: trash directories the store does not know of (a fresh store over an old checkout)
/// are skipped, never moved onto.
#[test]
fn a_delete_skips_trash_slots_already_on_disk() {
    let (e, _ws, repo) = project();
    for id in 1..=3 {
        let slot = repo.join(".relay/trash").join(id.to_string());
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::write(slot.join("payload"), format!("orphan {id}")).unwrap();
    }
    std::fs::write(repo.join("gone.txt"), "gone\n").unwrap();
    let trash_id = ok(&e, "file.delete", json!({"project_id": 1, "path": "gone.txt"}))["trash_id"].as_i64().unwrap();
    assert_eq!(trash_id, 4);
    for id in 1..=3 {
        let payload = repo.join(".relay/trash").join(id.to_string()).join("payload");
        assert_eq!(std::fs::read_to_string(payload).unwrap(), format!("orphan {id}"));
    }
    ok(&e, "file.restore", json!({"project_id": 1, "trash_id": trash_id}));
    assert_eq!(std::fs::read_to_string(repo.join("gone.txt")).unwrap(), "gone\n");
}

/// RA-364: a staged change does not survive restore_head, and `.` is refused.
#[test]
fn restore_head_restores_from_head_not_the_index() {
    let (e, _ws, repo) = project();
    std::fs::write(repo.join("README.md"), "staged edit\n").unwrap();
    git(&repo, &["add", "README.md"]);
    std::fs::write(repo.join("README.md"), "unstaged edit\n").unwrap();
    ok(&e, "file.restore_head", json!({"project_id": 1, "path": "README.md"}));
    assert_eq!(std::fs::read_to_string(repo.join("README.md")).unwrap(), "# Relay\n");
    let staged = Command::new("git").arg("-C").arg(&repo).args(["diff", "--cached", "--name-only"]).output().unwrap();
    assert!(staged.stdout.is_empty(), "{}", String::from_utf8_lossy(&staged.stdout));
    for path in [".", "./"] {
        let refused = call(&e, "file.restore_head", json!({"project_id": 1, "path": path}));
        assert_eq!(err(&refused).code, "file.path", "{path}");
    }
}

/// RA-368 and RA-369: an unreadable folder is listed without children instead of failing the
/// tree, and reading a missing file is the caller's `not_found`.
#[test]
fn a_tree_lists_an_unreadable_folder_and_a_missing_file_is_not_found() {
    use std::os::unix::fs::PermissionsExt;
    let (e, _ws, repo) = project();
    let locked = repo.join("locked");
    std::fs::create_dir_all(locked.join("inner")).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root ignores directory permissions; the case is only meaningful without it.
    let readable = std::fs::read_dir(&locked).is_ok();
    let tree = call(&e, "file.tree", json!({"project_id": 1, "depth": 3, "git_badges": false}));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let tree = tree.into_result().unwrap();
    let entry = tree["entries"].as_array().unwrap().iter().find(|e| e["path"] == "locked").unwrap().clone();
    if !readable {
        assert!(entry.get("children").is_none_or(Value::is_null), "{entry}");
    }
    let missing = call(&e, "file.read", json!({"project_id": 1, "path": "nope.txt"}));
    assert_eq!(err(&missing).kind, ErrorKind::NotFound);
    assert_eq!(err(&missing).code, "file.not_found");
}

/// `file.write` reads the file it replaces under the store lock, so it reads only a regular file:
/// `fs::read_to_string` on a FIFO there never returned, and every request queued behind it.
#[test]
fn a_write_over_a_pipe_answers_instead_of_blocking_the_bus() {
    let (e, _ws, repo) = project();
    assert!(Command::new("mkfifo").arg(repo.join("pipe.txt")).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let answering = e.clone();
    std::thread::spawn(move || {
        let _ = tx.send(call(&answering, "file.write", json!({
            "project_id": 1, "path": "pipe.txt", "text": "draft\n",
            "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        })));
    });
    let answered = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("file.write blocked on the pipe");
    assert_eq!(err(&answered).code, "file.edit_conflict");
    assert_eq!(ok(&e, "file.read", json!({"project_id": 1, "path": "README.md"}))["text"], "# Relay\n");
}
