//! Audit findings RA-145 … RA-149 (`file.*`) and RA-188 … RA-191 (`task.*`), each pinned
//! through the bus door the way a client or an agent reaches it.

mod common;

use common::{call_as as call, committed_repo, engine_with_project, git, ok};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// Unlike `common::refused`, takes the request and names the op that should have refused.
fn refused(engine: &Engine, actor: Actor, op: &str, payload: Value) -> relay_bus::BusError {
    call(engine, actor, op, payload)
        .into_result()
        .expect_err(&format!("{op} was expected to refuse"))
}

struct Fixture {
    root: tempfile::TempDir,
    engine: Arc<Engine>,
    repo: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        let repo = ws.join("first");
        committed_repo(&repo, &[("README.md", "audit\n")]);
        committed_repo(&ws.join("second"), &[("README.md", "audit\n")]);
        let engine = engine_with_project(root.path(), &ws, &repo);
        ok(&engine, "project.add", json!({"workspace_id": 1, "path": ws.join("second")}));
        let repo = std::fs::canonicalize(&repo).unwrap();
        Self { root, engine, repo }
    }

    fn task(&self, title: &str, extra: Value) -> i64 {
        let mut payload = json!({"project_id": 1, "title": title});
        payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        ok(&self.engine, "task.create", payload)["id"].as_i64().unwrap()
    }
}

/// RA-149: a path whose parent does not exist yet used to fail `safe_join`, so nested
/// creates, writes and restores all failed and their `create_dir_all` calls never ran.
#[test]
fn nested_creates_writes_and_restores_make_their_parents() {
    let f = Fixture::new();
    let e = &f.engine;
    ok(e, "file.create", json!({"project_id": 1, "path": "a/b/c.txt", "kind": "file", "text": "c\n"}));
    assert_eq!(std::fs::read_to_string(f.repo.join("a/b/c.txt")).unwrap(), "c\n");
    ok(e, "file.write", json!({"project_id": 1, "path": "x/y/z.txt", "text": "z\n"}));
    assert_eq!(std::fs::read_to_string(f.repo.join("x/y/z.txt")).unwrap(), "z\n");

    let trash = ok(e, "file.delete", json!({"project_id": 1, "path": "a/b/c.txt"}))["trash_id"].clone();
    std::fs::remove_dir_all(f.repo.join("a")).unwrap();
    ok(e, "file.restore", json!({"project_id": 1, "trash_id": trash}));
    assert_eq!(std::fs::read_to_string(f.repo.join("a/b/c.txt")).unwrap(), "c\n");

    // The traversal guard still holds for a path that does not exist yet.
    let outside = f.root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, f.repo.join("escape")).unwrap();
    let error = refused(e, Actor::User, "file.create",
        json!({"project_id": 1, "path": "escape/new/file.txt", "kind": "file", "text": "x"}));
    assert_eq!(error.code, "file.path");
    assert!(!outside.join("new").exists());
}

/// RA-146: a write through a symlink updates its target and leaves the link a link; a create
/// or import onto a dangling link no longer creates its target outside the worktree.
#[test]
fn symlinks_are_written_through_and_dangling_ones_are_not_followed() {
    let f = Fixture::new();
    let e = &f.engine;
    std::fs::write(f.repo.join("target.txt"), "old\n").unwrap();
    std::os::unix::fs::symlink("target.txt", f.repo.join("link.txt")).unwrap();
    ok(e, "file.write", json!({"project_id": 1, "path": "link.txt", "text": "new\n"}));
    assert!(std::fs::symlink_metadata(f.repo.join("link.txt")).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(f.repo.join("target.txt")).unwrap(), "new\n");

    let outside = f.root.path().join("planted.txt");
    std::os::unix::fs::symlink(&outside, f.repo.join("dangling.txt")).unwrap();
    let error = refused(e, Actor::User, "file.create",
        json!({"project_id": 1, "path": "dangling.txt", "kind": "file", "text": "x"}));
    assert_eq!(error.code, "file.path");
    let error = refused(e, Actor::User, "file.write",
        json!({"project_id": 1, "path": "dangling.txt", "text": "x"}));
    assert_eq!(error.code, "file.path");

    let source_dir = f.root.path().join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("dangling.txt"), "imported\n").unwrap();
    let error = refused(e, Actor::User, "file.import",
        json!({"project_id": 1, "sources": [source_dir.join("dangling.txt")], "into": ""}));
    assert_eq!(error.code, "file.exists");
    assert!(!outside.exists(), "a dangling symlink was followed out of the worktree");
}

/// RA-147: a directory is gated by the protected paths inside it, not only by its own path.
#[test]
fn directory_mutations_are_gated_by_the_protected_paths_inside_them() {
    let f = Fixture::new();
    let e = &f.engine;
    ok(e, "guardrail.config.set", json!({"project_id": 1, "patch": {"protected_paths": ["config/prod.env"]}}));
    std::fs::create_dir_all(f.repo.join("config")).unwrap();
    std::fs::write(f.repo.join("config/prod.env"), "KEY=1\n").unwrap();
    std::fs::create_dir_all(f.repo.join("elsewhere")).unwrap();

    for (op, payload) in [
        ("file.delete", json!({"project_id": 1, "path": "config"})),
        ("file.rename", json!({"project_id": 1, "path": "config", "new_name": "settings"})),
        ("file.move", json!({"project_id": 1, "path": "config", "into": "elsewhere"})),
    ] {
        let error = refused(e, Actor::User, op, payload);
        assert!(error.code.starts_with("guardrail."), "{op} answered {}", error.code);
        assert!(f.repo.join("config/prod.env").exists(), "{op} went through");
    }

    // An unprotected directory still moves, and the walk finds nothing to hold it on.
    std::fs::create_dir_all(f.repo.join("docs/deep")).unwrap();
    std::fs::write(f.repo.join("docs/deep/a.md"), "a\n").unwrap();
    ok(e, "file.move", json!({"project_id": 1, "path": "docs", "into": "elsewhere"}));
    assert!(f.repo.join("elsewhere/docs/deep/a.md").exists());
}

/// RA-148: an agent's file mutations are confined to its own project and its own checkout.
#[test]
fn agents_mutate_only_their_own_checkout() {
    let f = Fixture::new();
    let e = &f.engine;
    let builder = ok(e, "session.create",
        json!({"project_id": 1, "provider": "claude", "role": "builder", "bus_writes": true}));
    let agent = || Actor::agent(builder["name"].as_str().unwrap());
    let worktree = PathBuf::from(builder["worktree"].as_str().unwrap());
    assert_ne!(std::fs::canonicalize(&worktree).unwrap(), f.repo);

    let error = refused(e, agent(), "file.write", json!({"project_id": 2, "path": "x.txt", "text": "x"}));
    assert_eq!(error.code, "actor.scope");
    let error = refused(e, agent(), "file.create",
        json!({"project_id": 1, "worktree": "@project", "path": "x.txt", "kind": "file", "text": "x"}));
    assert_eq!(error.code, "actor.scope");
    let error = refused(e, agent(), "file.delete",
        json!({"project_id": 1, "worktree": f.repo, "path": "README.md"}));
    assert_eq!(error.code, "actor.scope");
    assert!(f.repo.join("README.md").exists());
    assert!(!f.repo.join("x.txt").exists());

    let wrote = call(e, agent(), "file.create", json!({"project_id": 1, "path": "mine.txt", "kind": "file", "text": "x"}));
    assert!(wrote.ok, "{:?}", wrote.error);
    assert!(worktree.join("mine.txt").exists());
    // The user still reaches any worktree.
    ok(e, "file.create", json!({"project_id": 1, "worktree": worktree, "path": "user.txt", "kind": "file"}));
}

/// RA-190: `task.attach` from a path copies with the lock released and leaves no staging behind.
#[test]
fn attach_from_a_path_lands_the_copy_and_cleans_up() {
    let f = Fixture::new();
    let e = &f.engine;
    let id = f.task("Attach", json!({}));
    let source = f.root.path().join("notes.bin");
    std::fs::write(&source, vec![7u8; 300_000]).unwrap();
    let attachment = ok(e, "task.attach", json!({"task_id": id, "path": source}));
    assert_eq!(attachment["bytes"], 300_000);
    assert_eq!(std::fs::read(attachment["path"].as_str().unwrap()).unwrap().len(), 300_000);
    let staging = f.root.path().join("store/attachments/.staging");
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0, "a staged copy was left behind");

    let empty = f.root.path().join("empty.bin");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(refused(e, Actor::User, "task.attach", json!({"task_id": id, "path": empty})).code, "task.attachment_empty");
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0, "a refused attach left its copy");
}

/// RA-189: a move out of Done can be undone, so the board's Ctrl+Z is not jammed by it. A task
/// that never had a commit linked still goes to Done only through `task.approve`.
#[test]
fn a_move_out_of_done_can_be_undone() {
    let f = Fixture::new();
    let e = &f.engine;
    let id = f.task("Ship it", json!({"column": "ready"}));
    ok(e, "task.approve", json!({"task_id": id, "sha": "abc123"}));
    ok(e, "task.move", json!({"task_id": id, "column": "in_review"}));
    let row = ok(e, "audit.list", json!({"op_prefix": "task.move", "limit": 1}))["rows"][0].clone();
    ok(e, "audit.undo", json!({"audit_id": row["id"]}));
    let task = ok(e, "task.get", json!({"task_id": id}));
    assert_eq!(task["column"], "done");
    assert_eq!(task["state"], "none");

    let fresh = f.task("Unlinked", json!({}));
    let error = refused(e, Actor::User, "task.move", json!({"task_id": fresh, "column": "done"}));
    assert_eq!(error.code, "task.column_transition");
}

/// RA-191: deleting a parent takes its sub-tasks with it and restoring brings them back; a
/// sub-task restored under a parent that can no longer take it comes back top-level.
#[test]
fn sub_tasks_are_deleted_and_restored_with_their_parent() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Parent", json!({}));
    let child = f.task("Child", json!({"parent_id": parent}));
    let grandchild = f.task("Grandchild", json!({"parent_id": child}));
    ok(e, "task.delete", json!({"task_id": parent}));
    for id in [child, grandchild] {
        assert_eq!(refused(e, Actor::User, "task.get", json!({"task_id": id})).code, "task.not_found");
    }
    ok(e, "task.restore", json!({"task_id": parent}));
    assert_eq!(ok(e, "task.get", json!({"task_id": grandchild}))["parent_id"], child);
    assert_eq!(ok(e, "task.get", json!({"task_id": parent}))["rollup"]["total"], 2);

    // Delete the child (and grandchild) alone, then deepen the parent: the hidden subtree no
    // longer fits, and restoring it must not break the depth cap.
    ok(e, "task.delete", json!({"task_id": child}));
    let top = f.task("Top", json!({}));
    ok(e, "task.parent.set", json!({"task_id": parent, "parent_id": top}));
    let restored = ok(e, "task.restore", json!({"task_id": child}));
    assert!(restored["parent_id"].is_null(), "restored past the depth cap: {restored}");
    assert_eq!(ok(e, "task.get", json!({"task_id": grandchild}))["depth"], 1);
}

/// RA-188: the narrowed activity query still finds creation, edits and dispatch-free moves,
/// and only for its own task.
#[test]
fn task_activity_still_finds_the_tasks_own_history() {
    let f = Fixture::new();
    let e = &f.engine;
    let id = f.task("Watched", json!({}));
    let other = f.task("Other", json!({}));
    ok(e, "task.update", json!({"task_id": id, "body": "more"}));
    ok(e, "task.move", json!({"task_id": id, "column": "ready"}));
    ok(e, "task.move", json!({"task_id": other, "column": "ready"}));
    let history = ok(e, "task.activity", json!({"task_id": id}))["history"].clone();
    let ops: Vec<&str> = history.as_array().unwrap().iter().map(|row| row["op"].as_str().unwrap()).collect();
    assert_eq!(ops, ["task.move", "task.update", "task.create"]);
}

/// RA-153: `file.restore_head` takes its path literally. As a pathspec, `[ab].txt` also
/// matched `a.txt` and `b.txt`, and discarded their uncommitted edits along with it.
#[test]
fn restore_head_takes_a_bracketed_name_literally() {
    let f = Fixture::new();
    let e = &f.engine;
    for name in ["[ab].txt", "a.txt", "b.txt"] {
        std::fs::write(f.repo.join(name), "committed\n").unwrap();
    }
    git(&f.repo, &["add", "--", "."]);
    git(&f.repo, &["commit", "-q", "-m", "files"]);
    for name in ["[ab].txt", "a.txt", "b.txt"] {
        std::fs::write(f.repo.join(name), "edited\n").unwrap();
    }
    ok(e, "file.restore_head", json!({"project_id": 1, "path": "[ab].txt"}));
    assert_eq!(std::fs::read_to_string(f.repo.join("[ab].txt")).unwrap(), "committed\n");
    for name in ["a.txt", "b.txt"] {
        assert_eq!(std::fs::read_to_string(f.repo.join(name)).unwrap(), "edited\n", "{name} was discarded");
    }
}

/// `audit.undo` of a directory delete restores it through `file.restore`'s staged path — the
/// checkout check and the walk of the trashed tree before the transaction — and a second undo of
/// the same row is refused.
#[test]
fn undoing_a_directory_delete_restores_it_once() {
    let f = Fixture::new();
    let e = &f.engine;
    ok(e, "file.create", json!({"project_id": 1, "path": "docs/a.txt", "kind": "file", "text": "a\n"}));
    ok(e, "file.create", json!({"project_id": 1, "path": "docs/inner/b.txt", "kind": "file", "text": "b\n"}));
    ok(e, "file.delete", json!({"project_id": 1, "path": "docs"}));
    assert!(!f.repo.join("docs").exists());
    let row = ok(e, "audit.list", json!({"op_prefix": "file.delete", "limit": 1}))["rows"][0].clone();
    let undone = ok(e, "audit.undo", json!({"audit_id": row["id"]}));
    assert_eq!(undone["undone"], row["id"]);
    assert_eq!(std::fs::read_to_string(f.repo.join("docs/a.txt")).unwrap(), "a\n");
    assert_eq!(std::fs::read_to_string(f.repo.join("docs/inner/b.txt")).unwrap(), "b\n");
    assert_eq!(refused(e, Actor::User, "audit.undo", json!({"audit_id": row["id"]})).code, "audit.not_undoable");
}

/// An inverse's read/external phase runs only for an undo that will happen: undoing a branch
/// rename whose branch was renamed again since is stale, and renames nothing.
#[test]
fn a_stale_undo_runs_nothing_of_its_inverse() {
    let f = Fixture::new();
    let e = &f.engine;
    let session = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = session["name"].as_str().unwrap();
    let worktree = PathBuf::from(session["worktree"].as_str().unwrap());
    ok(e, "session.update", json!({"session": name, "branch": "relay/first"}));
    let row = ok(e, "audit.list", json!({"op_prefix": "session.update", "limit": 1}))["rows"][0].clone();
    ok(e, "session.update", json!({"session": name, "branch": "relay/second"}));
    assert_eq!(refused(e, Actor::User, "audit.undo", json!({"audit_id": row["id"]})).code, "audit.stale");
    assert_eq!(git(&worktree, &["branch", "--show-current"]), "relay/second");
    assert_eq!(ok(e, "session.get", json!({"session": name}))["branch"], "relay/second");
}

/// RA-190 for `task.create`: inline attachments are decoded and staged before the transaction,
/// which only renames them into place; a create refused inside it leaves no copy behind.
#[test]
fn create_with_inline_attachments_stages_them_and_cleans_up() {
    use base64::Engine as _;
    let f = Fixture::new();
    let e = &f.engine;
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let attachments = f.root.path().join("store/attachments");
    let staging = attachments.join(".staging");
    let task = ok(e, "task.create", json!({"project_id": 1, "title": "With files", "attachments": [
        {"name": "one.txt", "mime": "text/plain", "bytes_b64": b64(b"one")},
        {"name": "dir/two.bin", "mime": "application/octet-stream", "bytes_b64": b64(&[7u8; 4096])},
    ]}));
    let stored = task["attachments"].as_array().unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!((stored[0]["name"].as_str(), stored[0]["bytes"].as_u64()), (Some("one.txt"), Some(3)));
    assert_eq!((stored[1]["name"].as_str(), stored[1]["bytes"].as_u64()), (Some("two.bin"), Some(4096)));
    assert_eq!(std::fs::read(stored[0]["path"].as_str().unwrap()).unwrap(), b"one");
    assert_eq!(std::fs::read(stored[1]["path"].as_str().unwrap()).unwrap(), vec![7u8; 4096]);
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0, "a staged copy was left behind");

    // Refused in the transaction, after its attachments were staged.
    let error = refused(e, Actor::User, "task.create", json!({"project_id": 1, "title": "No module", "module_id": 999,
        "attachments": [{"name": "three.txt", "mime": "text/plain", "bytes_b64": b64(b"three")}]}));
    assert_eq!(error.kind, relay_bus::ErrorKind::NotFound, "{error:?}");
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0, "a refused create left its copy");
    let dirs: Vec<_> = std::fs::read_dir(&attachments).unwrap().flatten().map(|d| d.file_name()).collect();
    assert_eq!(dirs.len(), 2, "only the first task's directory and the staging area: {dirs:?}");
    // Refused before anything is staged: an empty one, and bytes that are not base64.
    for (name, bytes) in [("empty.txt", String::new()), ("bad.txt", "not base64!".to_string())] {
        refused(e, Actor::User, "task.create", json!({"project_id": 1, "title": "Bad",
            "attachments": [{"name": name, "mime": "text/plain", "bytes_b64": bytes}]}));
    }
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
}
