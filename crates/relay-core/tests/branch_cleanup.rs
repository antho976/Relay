//! Branch cleanup: a closed session's branch (and worktree, and merged PR's remote branch) goes
//! once its work is merged, and never before.

mod common;

use common::{committed_repo, engine_with_project, git, git_command, ok};
use relay_core::branch_cleanup::{self, Options};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn has_ref(repo: &Path, reference: &str) -> bool {
    git_command(repo).args(["show-ref", "--verify", "--quiet", reference]).status().unwrap().success()
}

struct Fixture {
    _root: tempfile::TempDir,
    base: PathBuf,
    repo: PathBuf,
    origin: PathBuf,
    engine: Arc<Engine>,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let ws = base.join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "hi\n")]);
    let origin = base.join("origin.git");
    git(&base, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    let engine = engine_with_project(&base, &ws, &repo);
    Fixture { _root: root, base, repo, origin, engine }
}

struct Work { name: String, branch: String, worktree: PathBuf }

/// A session with one commit of its own on its branch.
fn session_with_commit(f: &Fixture, file: &str) -> Work {
    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "codex"}));
    let worktree = PathBuf::from(created["worktree"].as_str().unwrap());
    git(&worktree, &["config", "user.email", "t@t"]);
    git(&worktree, &["config", "user.name", "t"]);
    std::fs::write(worktree.join(file), format!("{file}\n")).unwrap();
    git(&worktree, &["add", file]);
    git(&worktree, &["commit", "--no-verify", "-q", "-m", &format!("add {file}")]);
    Work {
        name: created["name"].as_str().unwrap().to_string(),
        branch: created["branch"].as_str().unwrap().to_string(),
        worktree,
    }
}

/// A stand-in `gh` that reports one merged PR with the given head.
fn fake_gh(f: &Fixture, number: u64, head: &str) -> PathBuf {
    let path = f.base.join(format!("gh-{number}"));
    std::fs::write(&path, format!("#!/bin/sh\n[ \"$1 $2\" = \"pr list\" ] || exit 9\nprintf '[{{\"number\":{number},\"headRefOid\":\"{head}\"}}]'\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn cleanup(f: &Fixture, branch: &str, gh: Option<PathBuf>) -> relay_bus::ops::git::BranchCleanupRow {
    let options = Options { gh, audit_kept: true, ..Options::default() };
    let mut rows = branch_cleanup::run(&f.engine, Some(1), Some(&[branch.to_string()]), &options).unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows.remove(0)
}

fn audited(f: &Fixture, branch: &str) -> Vec<Value> {
    let audit = ok(&f.engine, "audit.list", json!({"op_prefix": "git.branch.cleanup"}));
    audit["rows"].as_array().cloned().unwrap_or_default().into_iter()
        .filter(|row| row.to_string().contains(branch)).collect()
}

#[test]
fn a_merged_branch_goes_and_an_unmerged_one_stays_with_its_reason() {
    let f = fixture();
    let merged = session_with_commit(&f, "merged.txt");
    let open = session_with_commit(&f, "open.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &merged.branch]);
    ok(&f.engine, "session.close", json!({"session": merged.name}));
    ok(&f.engine, "session.close", json!({"session": open.name}));
    // In production session.close starts this same cleanup on a background thread at once;
    // the test instance skips that so `cleanup` below can drive it synchronously.
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", merged.branch)), "the test instance defers after-close cleanup to the explicit run");

    let row = cleanup(&f, &merged.branch, None);
    assert_eq!(row.outcome, "deleted", "{row:?}");
    assert!(row.reason.contains("merged into main"), "{row:?}");
    assert_eq!(row.session.as_deref(), Some(merged.name.as_str()));
    assert!(!has_ref(&f.repo, &format!("refs/heads/{}", merged.branch)));
    assert!(!audited(&f, &merged.branch).is_empty(), "the deletion is on the audit record");

    let row = cleanup(&f, &open.branch, None);
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("1 commit(s) not in main"), "{row:?}");
    assert!(row.reason.contains("never pushed"), "{row:?}");
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", open.branch)), "unmerged work is never deleted");
    assert!(!audited(&f, &open.branch).is_empty(), "keeping it after a close is on the record, with why");
}

#[test]
fn a_squash_merged_pr_removes_worktree_branch_and_remote_branch() {
    let f = fixture();
    let work = session_with_commit(&f, "feature.txt");
    // Two commits: squashed into one, no patch id matches, so only GitHub can prove the merge.
    std::fs::write(work.worktree.join("feature.txt"), "feature, revised\n").unwrap();
    git(&work.worktree, &["commit", "--no-verify", "-q", "-am", "revise feature"]);
    git(&work.worktree, &["push", "-q", "-u", "origin", &work.branch]);
    let tip = git(&work.worktree, &["rev-parse", "HEAD"]);
    // GitHub squash-merges: main gets one new commit, not the branch's.
    git(&f.repo, &["merge", "-q", "--squash", &work.branch]);
    git(&f.repo, &["commit", "--no-verify", "-q", "-m", "feature (#42)"]);
    // Closed with its checkout kept, the PR merged later.
    ok(&f.engine, "session.close", json!({"session": work.name, "remove_worktree": false}));
    assert!(work.worktree.exists());

    // Without GitHub, nothing proves a squash merge: kept.
    let row = cleanup(&f, &work.branch, None);
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(work.worktree.exists());

    let row = cleanup(&f, &work.branch, Some(fake_gh(&f, 42, &tip)));
    assert_eq!(row.outcome, "deleted", "{row:?}");
    assert_eq!(row.pr, Some(42));
    assert!(row.removed_worktree && !work.worktree.exists(), "{row:?}");
    assert!(row.deleted_remote, "{row:?}");
    assert!(!has_ref(&f.repo, &format!("refs/heads/{}", work.branch)));
    assert!(!has_ref(&f.origin, &format!("refs/heads/{}", work.branch)), "the merged PR's branch is gone from the remote");
}

#[test]
fn work_after_the_pr_or_uncommitted_changes_keep_the_branch() {
    let f = fixture();
    // Commits after the PR head was merged.
    let late = session_with_commit(&f, "late.txt");
    git(&late.worktree, &["push", "-q", "-u", "origin", &late.branch]);
    let pr_head = git(&late.worktree, &["rev-parse", "HEAD"]);
    std::fs::write(late.worktree.join("after.txt"), "more\n").unwrap();
    git(&late.worktree, &["add", "after.txt"]);
    git(&late.worktree, &["commit", "--no-verify", "-q", "-m", "after the PR"]);
    ok(&f.engine, "session.close", json!({"session": late.name}));
    let row = cleanup(&f, &late.branch, Some(fake_gh(&f, 7, &pr_head)));
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("no merged PR contains its tip"), "{row:?}");
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", late.branch)));
    assert!(has_ref(&f.origin, &format!("refs/heads/{}", late.branch)));

    // Merged, but its kept checkout has uncommitted changes.
    let dirty = session_with_commit(&f, "dirty.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &dirty.branch]);
    ok(&f.engine, "session.close", json!({"session": dirty.name, "remove_worktree": false}));
    std::fs::write(dirty.worktree.join("scratch.txt"), "not committed\n").unwrap();
    let row = cleanup(&f, &dirty.branch, None);
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("uncommitted"), "{row:?}");
    assert!(dirty.worktree.join("scratch.txt").exists());
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", dirty.branch)));
}

#[test]
fn open_sessions_are_never_candidates_and_the_op_dry_runs() {
    let f = fixture();
    let live = session_with_commit(&f, "live.txt");
    let done = session_with_commit(&f, "done.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge live", &live.branch]);
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge done", &done.branch]);
    ok(&f.engine, "session.close", json!({"session": done.name}));

    let dry = ok(&f.engine, "git.branch.cleanup", json!({"project_id": 1, "dry_run": true}));
    let rows = dry["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "only the closed session's branch is considered: {dry}");
    assert_eq!(rows[0]["branch"], done.branch.as_str());
    assert_eq!(rows[0]["outcome"], "would_delete");
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", done.branch)), "a dry run deletes nothing");

    let real = ok(&f.engine, "git.branch.cleanup", json!({"project_id": 1}));
    assert_eq!(real["branches"][0]["outcome"], "deleted");
    assert!(!has_ref(&f.repo, &format!("refs/heads/{}", done.branch)));
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", live.branch)), "a live session's branch is untouched");
    assert!(live.worktree.exists());
}

/// RA-086: a session on a branch Relay did not name is not Relay's to clean up after.
#[test]
fn a_users_own_branch_is_never_a_candidate() {
    let f = fixture();
    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "codex", "branch": "feature/mine"}));
    let name = created["name"].as_str().unwrap();
    let worktree = PathBuf::from(created["worktree"].as_str().unwrap());
    git(&worktree, &["config", "user.email", "t@t"]);
    git(&worktree, &["config", "user.name", "t"]);
    std::fs::write(worktree.join("mine.txt"), "mine\n").unwrap();
    git(&worktree, &["add", "mine.txt"]);
    git(&worktree, &["commit", "--no-verify", "-q", "-m", "mine"]);
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge mine", "feature/mine"]);
    ok(&f.engine, "session.close", json!({"session": name}));
    let rows = branch_cleanup::run(&f.engine, Some(1), None, &Options::default()).unwrap();
    assert!(rows.is_empty(), "{rows:?}");
    assert!(has_ref(&f.repo, "refs/heads/feature/mine"), "a merged user branch is still the user's");
}

/// RA-087: a checkout kept at close stays, and a later cleanup never deletes ignored files.
#[test]
fn a_worktree_kept_at_close_and_its_ignored_files_survive_cleanup() {
    let f = fixture();
    let work = session_with_commit(&f, "kept.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &work.branch]);
    ok(&f.engine, "session.close", json!({"session": work.name, "remove_worktree": false}));

    branch_cleanup::after_close(f.engine.clone(), 1, work.branch.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while audited(&f, &work.branch).is_empty() {
        assert!(std::time::Instant::now() < deadline, "the after-close cleanup never reported");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let audit = audited(&f, &work.branch);
    assert!(audit[0].to_string().contains("kept at close"), "{audit:?}");
    assert!(work.worktree.exists(), "the checkout the user kept is gone");
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", work.branch)));

    // Later, a cleanup may take the checkout — but not with a git-ignored `.env` in it.
    let exclude = f.repo.join(".git/info/exclude");
    let mut rules = std::fs::read_to_string(&exclude).unwrap_or_default();
    rules.push_str(".env\n");
    std::fs::write(&exclude, rules).unwrap();
    std::fs::write(work.worktree.join(".env"), "TOKEN=secret\n").unwrap();
    let row = cleanup(&f, &work.branch, None);
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("git-ignored") && row.reason.contains(".env"), "{row:?}");
    assert!(work.worktree.join(".env").exists());

    std::fs::remove_file(work.worktree.join(".env")).unwrap();
    // A shell working inside the checkout keeps it, however the sweep judged the branch.
    let mut shell = std::process::Command::new("sleep").arg("30").current_dir(&work.worktree).spawn().unwrap();
    let row = cleanup(&f, &work.branch, None);
    let _ = shell.kill();
    let _ = shell.wait();
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("in use now"), "{row:?}");
    assert!(work.worktree.exists());
    let row = cleanup(&f, &work.branch, None);
    assert_eq!(row.outcome, "deleted", "Relay's own hook files are not the user's: {row:?}");
    assert!(row.removed_worktree && !work.worktree.exists());
}

/// RA-088: the periodic sweep does not re-judge a kept branch whose refs have not moved.
#[test]
fn the_sweep_skips_a_kept_branch_until_its_refs_move() {
    let f = fixture();
    let open = session_with_commit(&f, "unmerged.txt");
    ok(&f.engine, "session.close", json!({"session": open.name}));
    let sweep = Options { use_gh_cache: true, ..Options::default() };
    let only = [open.branch.clone()];
    let first = branch_cleanup::run(&f.engine, Some(1), Some(&only), &sweep).unwrap();
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].outcome, "kept");
    assert!(branch_cleanup::run(&f.engine, Some(1), Some(&only), &sweep).unwrap().is_empty(), "judged again with nothing changed");
    // Asked for directly, it is still answered.
    assert_eq!(cleanup(&f, &open.branch, None).outcome, "kept");
    // Its base moving is a reason to look again — here, because it is now merged.
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &open.branch]);
    let again = branch_cleanup::run(&f.engine, Some(1), Some(&only), &sweep).unwrap();
    assert_eq!(again.len(), 1, "{again:?}");
    assert_eq!(again[0].outcome, "deleted", "{again:?}");
}

/// RA-309: `git worktree list` shows a checkout mid-rebase as detached, and `update-ref -d` does
/// not refuse the branch it is rebasing; cleanup looks for one just before it deletes.
#[test]
fn a_branch_being_rebased_or_bisected_is_kept() {
    let f = fixture();
    let work = session_with_commit(&f, "rebased.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &work.branch]);
    ok(&f.engine, "session.close", json!({"session": work.name}));
    let rebase = f.repo.join(".git/rebase-merge");
    std::fs::create_dir_all(&rebase).unwrap();
    std::fs::write(rebase.join("head-name"), format!("refs/heads/{}\n", work.branch)).unwrap();
    let row = cleanup(&f, &work.branch, None);
    assert_eq!(row.outcome, "kept", "{row:?}");
    assert!(row.reason.contains("rebase or bisect"), "{row:?}");
    assert!(has_ref(&f.repo, &format!("refs/heads/{}", work.branch)));
    std::fs::remove_dir_all(&rebase).unwrap();
    std::fs::write(f.repo.join(".git/BISECT_START"), format!("{}\n", work.branch)).unwrap();
    assert_eq!(cleanup(&f, &work.branch, None).outcome, "kept");
    std::fs::remove_file(f.repo.join(".git/BISECT_START")).unwrap();
    assert_eq!(cleanup(&f, &work.branch, None).outcome, "deleted");
}

/// A merged branch whose pooled checkout was deleted by hand goes: git's record of the folder
/// is pruned, where `status` in the missing folder kept the branch at every sweep for good.
#[test]
fn a_merged_branch_whose_checkout_was_deleted_by_hand_goes() {
    let f = fixture();
    let work = session_with_commit(&f, "gone.txt");
    git(&f.repo, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", &work.branch]);
    ok(&f.engine, "session.close", json!({"session": work.name, "remove_worktree": false}));
    std::fs::remove_dir_all(&work.worktree).unwrap();
    let row = cleanup(&f, &work.branch, None);
    assert_eq!(row.outcome, "deleted", "{row:?}");
    assert!(row.removed_worktree, "{row:?}");
    assert!(!has_ref(&f.repo, &format!("refs/heads/{}", work.branch)));
}
