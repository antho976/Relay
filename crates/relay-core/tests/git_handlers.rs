//! `git.*` over the bus where a repository is less tidy than a fixture: merges in history,
//! bracketed file names, submodules, symlinks, renames out of protected paths.

mod common;

use common::{call, engine, err, git, git_command, init_repo as init, ok};
use relay_bus::ErrorKind;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::Path;
/// A repository with one commit on `main`, registered as project 1.
fn project(e: &Engine) -> (tempfile::TempDir, String) {
    let ws = tempfile::tempdir().unwrap();
    let repo = ws.path().join("repo");
    init(&repo);
    std::fs::write(repo.join("README.md"), "# Relay\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-q", "-m", "Initial"]);
    let path = std::fs::canonicalize(&repo).unwrap().display().to_string();
    ok(e, "workspace.create", json!({"path":std::fs::canonicalize(ws.path()).unwrap()}));
    ok(e, "project.add", json!({"workspace_id":1,"path":path}));
    (ws, path)
}
fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["commit", "-q", "--allow-empty", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}
fn staged(repo: &Path) -> Vec<String> {
    let names = git(repo, &["diff", "--cached", "--name-only"]);
    names.lines().map(str::to_owned).collect()
}

/// RA-150: a parent is never listed before one of its children, even when a breadth-first
/// walk would reach it first through the shorter side of a merge.
#[test]
fn log_lists_every_commit_before_its_parents() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["checkout", "-q", "-b", "side"]);
    for n in 1..=3 {
        commit(root, &format!("side {n}"));
    }
    git(root, &["checkout", "-q", "main"]);
    commit(root, "main 1");
    git(root, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let log = ok(&e, "git.log", json!({"project_id":1,"limit":50}));
    let commits = log["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 6);
    assert_eq!(commits[0]["subject"], "Merge side");
    let position = |sha: &Value| commits.iter().position(|c| c["sha"] == *sha);
    for (at, c) in commits.iter().enumerate() {
        for parent in c["parents"].as_array().unwrap() {
            let parent_at = position(parent).expect("every parent is in the listing");
            assert!(parent_at > at, "{} is listed before its child {}", parent, c["sha"]);
        }
    }
}

/// RA-151: `git.branches` decides "merged" with the same single walk as branch.delete.
#[test]
fn branches_mark_merged_and_unmerged_tips() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["branch", "landed"]);
    git(root, &["checkout", "-q", "-b", "open"]);
    commit(root, "not on main");
    git(root, &["checkout", "-q", "main"]);
    commit(root, "main moves on");
    let out = ok(&e, "git.branches", json!({"project_id":1}));
    let merged = |name: &str| {
        out["branches"].as_array().unwrap().iter().find(|b| b["name"] == name).unwrap()["merged"].clone()
    };
    assert_eq!(out["current"], "main");
    assert_eq!(out["branches"][0]["name"], "main", "the current branch is listed first");
    assert_eq!(merged("landed"), true);
    assert_eq!(merged("open"), false);
    assert_eq!(merged("main"), true);
}

/// RA-152: creating a branch with checkout moves a live session's checkout no more than
/// git.branch.switch may; creating it without checkout is still fine.
#[test]
fn branch_create_does_not_check_out_in_a_session_owned_checkout() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let session = ok(&e, "session.create", json!({"project_id":1,"provider":"codex","worktree":"primary"}));
    assert_eq!(session["worktree"], repo);
    let refused = call(&e, "git.branch.create", json!({"project_id":1,"name":"feature/taken"}));
    assert_eq!(err(&refused).code, "git.checkout_session_owned");
    assert_eq!(git(Path::new(&repo), &["branch", "--show-current"]), "main");
    assert!(git(Path::new(&repo), &["branch", "--list", "feature/taken"]).is_empty(), "nothing was created");
    ok(&e, "git.branch.create", json!({"project_id":1,"name":"feature/taken","checkout":false}));
    assert_eq!(git(Path::new(&repo), &["branch", "--show-current"]), "main");
}

/// RA-153: a path is a name, never a glob.
#[test]
fn stage_and_unstage_take_paths_literally() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::write(root.join("a[1].txt"), "bracketed\n").unwrap();
    std::fs::write(root.join("a1.txt"), "plain\n").unwrap();
    ok(&e, "git.stage", json!({"project_id":1,"paths":["a[1].txt"]}));
    assert_eq!(staged(root), ["a[1].txt"]);
    git(root, &["add", "a1.txt"]);
    ok(&e, "git.unstage", json!({"project_id":1,"paths":["a[1].txt"]}));
    assert_eq!(staged(root), ["a1.txt"]);
}

/// RA-155: a submodule bump and an untracked nested repository are diffed as git does — one
/// `Subproject commit` line — and committed, instead of failing the whole op.
#[test]
fn submodules_and_nested_repositories_diff_and_commit() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    let sub = root.join("sub");
    init(&sub);
    commit(&sub, "sub one");
    git(root, &["-c", "advice.addEmbeddedRepo=false", "add", "sub"]);
    git(root, &["commit", "-q", "-m", "Add sub"]);
    commit(&sub, "sub two");
    init(&root.join("nested"));
    commit(&root.join("nested"), "nested one");

    let unstaged = ok(&e, "git.diff", json!({"project_id":1}));
    let files = unstaged["files"].as_array().unwrap();
    let sub_row = files.iter().find(|f| f["path"] == "sub").expect("the bumped submodule is listed");
    assert_eq!((sub_row["added"].as_i64(), sub_row["removed"].as_i64()), (Some(1), Some(1)));
    assert!(files.iter().any(|f| f["path"].as_str().unwrap().starts_with("nested")));

    ok(&e, "git.stage", json!({"project_id":1,"paths":["sub"]}));
    let staged_diff = ok(&e, "git.diff", json!({"project_id":1,"staged":true}));
    assert_eq!(staged_diff["files"][0]["path"], "sub");
    assert_eq!(staged_diff["files"][0]["added"], 1);
    let out = ok(&e, "git.commit", json!({"project_id":1,"message":"Bump sub"}));
    let shown = ok(&e, "git.show", json!({"project_id":1,"sha":out["sha"]}));
    assert_eq!(shown["files"][0]["path"], "sub");
    assert_eq!(shown["files"][0]["status"], "M");
}

/// RA-156: a symlink is diffed as the name it holds, as git stores it, and a path through a
/// symlinked directory is not read at all — even when it points at something endless.
#[test]
fn diffs_never_follow_symlinks_out_of_the_checkout() {
    let e = engine();
    let (ws, repo) = project(&e);
    let root = Path::new(&repo);
    let outside = ws.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "not in the checkout\n").unwrap();
    std::os::unix::fs::symlink("/dev/zero", root.join("zero")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("out")).unwrap();

    let diff = ok(&e, "git.diff", json!({"project_id":1}));
    let zero = diff["files"].as_array().unwrap().iter().find(|f| f["path"] == "zero").unwrap();
    assert_eq!(zero["added"], 1);
    assert_eq!(zero["binary"], false);
    let file = ok(&e, "git.diff.file", json!({"project_id":1,"path":"zero"}));
    assert_eq!(file["new"], "/dev/zero");
    let through = ok(&e, "git.diff.file", json!({"project_id":1,"path":"out/secret.txt"}));
    assert_eq!(through["new"], "");
}

/// RA-158: moving a protected file out is a change to the protected path, and a pure rename
/// counts no lines.
#[test]
fn a_staged_rename_out_of_a_protected_path_is_gated_on_its_source() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::create_dir_all(root.join("secret")).unwrap();
    std::fs::write(root.join("secret/key.txt"), "one\ntwo\nthree\n").unwrap();
    git(root, &["add", "secret/key.txt"]);
    git(root, &["commit", "-q", "-m", "Add key"]);
    ok(&e, "guardrail.config.set", json!({"project_id":1,"patch":{"protected_paths":["secret/**"]}}));
    std::fs::create_dir_all(root.join("public")).unwrap();
    git(root, &["mv", "secret/key.txt", "public/key.txt"]);

    let diff = ok(&e, "git.diff", json!({"project_id":1,"staged":true}));
    let moved = &diff["files"][0];
    assert_eq!((moved["path"].as_str(), moved["old_path"].as_str()), (Some("public/key.txt"), Some("secret/key.txt")));
    assert_eq!((moved["added"].as_i64(), moved["removed"].as_i64()), (Some(0), Some(0)));

    let held = call(&e, "git.commit", json!({"project_id":1,"message":"Move the key"}));
    assert_eq!(err(&held).kind, ErrorKind::Held, "{:?}", err(&held));
    assert!(err(&held).message.contains("secret/key.txt"), "{}", err(&held).message);
    assert_eq!(git(root, &["log", "-1", "--format=%s"]), "Add key", "nothing was committed");
}

/// RA-159: a commit that adds a file in a new directory lists the file, not the directory.
#[test]
fn show_and_base_diff_list_files_not_directories() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::write(root.join("src/deep/lib.rs"), "fn main() {}\n").unwrap();
    git(root, &["add", "src"]);
    let sha = {
        git(root, &["commit", "-q", "-m", "Add lib"]);
        git(root, &["rev-parse", "HEAD"])
    };
    let shown = ok(&e, "git.show", json!({"project_id":1,"sha":sha}));
    let paths: Vec<_> = shown["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_owned()).collect();
    assert_eq!(paths, ["src/deep/lib.rs"]);
    let base = ok(&e, "git.diff", json!({"project_id":1,"base":"HEAD~1"}));
    let paths: Vec<_> = base["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_owned()).collect();
    assert_eq!(paths, ["src/deep/lib.rs"]);
    assert_eq!(base["files"][0]["added"], 1);
}

/// RA-157: a big or binary file is flagged rather than diffed line by line.
#[test]
fn big_and_binary_files_are_flagged_without_counts() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::write(root.join("blob.bin"), b"head\0tail").unwrap();
    std::fs::write(root.join("big.txt"), "line\n".repeat(2 * 1024 * 1024)).unwrap();
    let diff = ok(&e, "git.diff", json!({"project_id":1}));
    for name in ["blob.bin", "big.txt"] {
        let row = diff["files"].as_array().unwrap().iter().find(|f| f["path"] == name).unwrap();
        assert_eq!(row["binary"], true, "{name}");
        assert_eq!(row["added"], 0, "{name}");
    }
}

/// RA-206: a staged row diffs HEAD against the index, and a rename against its source.
#[test]
fn diff_file_shows_the_staged_side_and_a_rename_source() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
    git(root, &["add", "a.txt"]);
    git(root, &["commit", "-q", "-m", "Add a"]);
    std::fs::write(root.join("a.txt"), "one\nstaged\n").unwrap();
    git(root, &["add", "a.txt"]);
    std::fs::write(root.join("a.txt"), "one\nstaged\nlater\n").unwrap();

    let staged = ok(&e, "git.diff.file", json!({"project_id":1,"path":"a.txt","staged":true}));
    assert_eq!((staged["old"].as_str(), staged["new"].as_str()), (Some("one\ntwo\n"), Some("one\nstaged\n")));
    // Without `staged`, as before: HEAD against the working tree.
    let work = ok(&e, "git.diff.file", json!({"project_id":1,"path":"a.txt"}));
    assert_eq!(work["new"], "one\nstaged\nlater\n");

    git(root, &["commit", "-q", "-am", "Edit a"]);
    git(root, &["mv", "a.txt", "b.txt"]);
    std::fs::write(root.join("b.txt"), "one\nstaged\nlater\nrenamed\n").unwrap();
    git(root, &["add", "b.txt"]);
    let status = ok(&e, "git.status", json!({"project_id":1}));
    assert_eq!(status["files"][0]["renamed_from"], "a.txt", "{status}");
    let renamed = ok(&e, "git.diff.file", json!({"project_id":1,"path":"b.txt","staged":true,"old_path":"a.txt"}));
    assert_eq!(renamed["old"], "one\nstaged\nlater\n");
    assert_eq!(renamed["new"], "one\nstaged\nlater\nrenamed\n");
    let text = renamed["hunks"][0]["text"].as_str().unwrap();
    assert!(text.contains("+renamed") && !text.contains("-one"), "{text}");
    let escape = call(&e, "git.diff.file", json!({"project_id":1,"path":"b.txt","old_path":"../a.txt"}));
    assert_eq!(err(&escape).kind, ErrorKind::Invalid);
}

/// RA-204: no commit, `all` or not, while a conflict is unresolved in the index.
#[test]
fn commit_is_refused_while_the_index_is_unmerged() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("README.md"), "# Side\n").unwrap();
    git(root, &["commit", "-q", "-am", "Side"]);
    git(root, &["switch", "-q", "main"]);
    std::fs::write(root.join("README.md"), "# Main\n").unwrap();
    git(root, &["commit", "-q", "-am", "Main"]);
    let merge = git_command(root).args(["merge", "-q", "side"]).output().unwrap();
    assert!(!merge.status.success(), "the fixture must conflict");
    let head = git(root, &["rev-parse", "HEAD"]);

    for all in [true, false] {
        let refused = call(&e, "git.commit", json!({"project_id":1,"message":"Merge","all":all}));
        let error = err(&refused);
        assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Conflict, "git.unmerged"), "{error:?}");
        assert!(error.message.contains("README.md"), "{}", error.message);
    }
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert!(!git(root, &["diff", "--name-only", "--diff-filter=U"]).is_empty(), "the conflict is still unresolved");

    std::fs::write(root.join("README.md"), "# Both\n").unwrap();
    ok(&e, "git.stage", json!({"project_id":1,"paths":["README.md"]}));
    ok(&e, "git.commit", json!({"project_id":1,"message":"Merge side"}));
    assert_eq!(git(root, &["log", "-1", "--format=%p"]).split(' ').count(), 2, "the merge was concluded");
}

/// RA-205: an untracked directory is one status entry, and the dirty check still sees it.
#[test]
fn status_lists_an_untracked_directory_once() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::create_dir_all(root.join("venv/lib")).unwrap();
    for n in 0..50 {
        std::fs::write(root.join(format!("venv/lib/m{n}.py")), "x\n").unwrap();
    }
    let status = ok(&e, "git.status", json!({"project_id":1}));
    let paths: Vec<&str> = status["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["venv/"]);
    assert_eq!((status["truncated"].as_bool(), status["total"].as_u64()), (Some(false), Some(1)));
    let listed = ok(&e, "worktree.list", json!({"project_id":1}));
    assert_eq!(listed["worktrees"][0]["dirty"], true, "{listed}");
}

/// RA-371: a branch created without checkout reports its own tip, not the checkout's HEAD.
#[test]
fn branch_create_without_checkout_reports_the_new_branch_tip() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    let first = git(root, &["rev-parse", "HEAD"]);
    commit(root, "second");
    let out = ok(&e, "git.branch.create", json!({"project_id":1,"name":"from-first","start_point":first,"checkout":false}));
    assert_eq!(out["head"], first.as_str());
    assert_eq!(git(root, &["branch", "--show-current"]), "main");
}

/// RA-374: a commit dated past what jiff can represent is listed with no date, not an error.
#[test]
fn log_and_show_survive_an_out_of_range_author_date() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    let parent = git(root, &["rev-parse", "HEAD"]);
    let tree = git(root, &["rev-parse", "HEAD^{tree}"]);
    let raw = format!(
        "tree {tree}\nparent {parent}\nauthor Far Future <far@example.test> 999999999999 +0000\ncommitter Far Future <far@example.test> 999999999999 +0000\n\nfrom the far future\n"
    );
    let mut child = git_command(root).args(["hash-object", "-t", "commit", "-w", "--literally", "--stdin"])
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), raw.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    git(root, &["update-ref", "refs/heads/main", &sha]);
    let log = ok(&e, "git.log", json!({"project_id":1,"limit":10}));
    assert_eq!(log["commits"][0]["subject"], "from the far future");
    assert_eq!(log["commits"][0]["at"], "");
    assert_ne!(log["commits"][1]["at"], "", "a normal date is still given");
    ok(&e, "git.show", json!({"project_id":1,"sha":sha}));
}

/// RA-373: `git.branch.clean_merged` (an alias of `git.branch.cleanup`) deletes every closed
/// session's merged `relay/*` branch it can, and names the merged ones it could not delete with
/// why, apart from the unmerged work it keeps on purpose.
#[test]
fn clean_merged_deletes_every_branch_it_can() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    let session = |file: &str| {
        let created = ok(&e, "session.create", json!({"project_id":1,"provider":"codex"}));
        let worktree = Path::new(created["worktree"].as_str().unwrap()).to_path_buf();
        std::fs::write(worktree.join(file), "work\n").unwrap();
        git(&worktree, &["add", file]);
        git(&worktree, &["commit", "--no-verify", "-q", "-m", file]);
        (created["name"].as_str().unwrap().to_string(), created["branch"].as_str().unwrap().to_string())
    };
    let (done, done_branch) = session("done.txt");
    let (held, held_branch) = session("held.txt");
    let (open, open_branch) = session("open.txt");
    for branch in [&done_branch, &held_branch] {
        git(root, &["merge", "--no-verify", "-q", "--no-ff", "-m", "merge", branch]);
    }
    for name in [&done, &held, &open] {
        ok(&e, "session.close", json!({"session": name}));
    }
    // A rebase in progress on a merged branch: deleting it would pull it out from under that.
    let rebase = root.join(".git/rebase-merge");
    std::fs::create_dir_all(&rebase).unwrap();
    std::fs::write(rebase.join("head-name"), format!("refs/heads/{held_branch}\n")).unwrap();

    let out = ok(&e, "git.branch.clean_merged", json!({"project_id":1}));
    assert_eq!(out["deleted"], json!([done_branch]), "{out}");
    let failed = out["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1, "unmerged work is kept, not failed: {out}");
    assert_eq!(failed[0]["branch"], held_branch.as_str());
    assert!(failed[0]["reason"].as_str().unwrap().contains("rebase or bisect"), "{out}");
    assert!(git(root, &["branch", "--list", &done_branch]).is_empty());
    assert!(!git(root, &["branch", "--list", &held_branch]).is_empty());
    assert!(!git(root, &["branch", "--list", &open_branch]).is_empty(), "unmerged work stays");

    // The rebase over, the held branch goes too.
    std::fs::remove_dir_all(&rebase).unwrap();
    let out = ok(&e, "git.branch.clean_merged", json!({"project_id":1}));
    assert_eq!(out["deleted"], json!([held_branch]), "{out}");
    assert_eq!(out["failed"], json!([]), "{out}");
}

/// RA-422: project.update's legacy guardrail columns are checked like guardrail.config.set's.
#[test]
fn project_update_refuses_a_guardrail_pattern_the_config_would_reject() {
    let e = engine();
    let (_ws, _repo) = project(&e);
    for bad in [json!({"protected_paths":["/home/me/secrets"]}), json!({"critical_files":["../outside"]})] {
        let mut payload = bad.clone();
        payload["project_id"] = json!(1);
        let refused = call(&e, "project.update", payload);
        assert_eq!(err(&refused).code, "guardrail.config", "{bad}");
    }
    ok(&e, "project.update", json!({"project_id":1,"protected_paths":["secrets/"]}));
    let config = ok(&e, "guardrail.config.get", json!({"project_id":1}));
    assert!(config["protected_paths"].as_array().unwrap().iter().any(|p| p == "secrets/"), "{config}");
}

/// RA-110: a bus commit runs the user's prepare-commit-msg hook with git's arguments, before
/// commit-msg, and keeps what it adds. Concluding a merge it runs exactly once too, and so does
/// commit-msg.
#[test]
fn prepare_commit_msg_hook_runs_once_on_every_bus_commit() {
    use std::os::unix::fs::PermissionsExt;
    let e = engine();
    let (ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("side.txt"), "side\n").unwrap();
    git(root, &["add", "side.txt"]);
    git(root, &["commit", "-q", "-m", "Side"]);
    git(root, &["switch", "-q", "main"]);
    let log = ws.path().join("hooks.log");
    let hook = |name: &str, body: String| {
        let path = root.join(".git/hooks").join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    hook("prepare-commit-msg", format!(
        "#!/bin/sh\necho \"prepare-commit-msg $2 $3\" >> '{}'\nprintf '\\nPrepared-by: hook\\n' >> \"$1\"\n", log.display()));
    hook("commit-msg", format!("#!/bin/sh\necho commit-msg >> '{}'\n", log.display()));
    let ran = || std::fs::read_to_string(&log).unwrap_or_default();

    std::fs::write(root.join("notes.txt"), "notes\n").unwrap();
    ok(&e, "git.commit", json!({"project_id":1,"message":"Add notes","all":true}));
    assert_eq!(ran(), "prepare-commit-msg message \ncommit-msg\n");
    assert_eq!(git(root, &["log", "-1", "--format=%B"]), "Add notes\n\nPrepared-by: hook");

    std::fs::remove_file(&log).unwrap();
    git(root, &["merge", "-q", "--no-commit", "--no-ff", "side"]);
    ok(&e, "git.commit", json!({"project_id":1,"message":"Merge side"}));
    assert_eq!(git(root, &["log", "-1", "--format=%p"]).split(' ').count(), 2, "the merge was concluded");
    let ran = ran();
    assert_eq!(ran.lines().filter(|line| line.starts_with("prepare-commit-msg message")).count(), 1, "{ran}");
    assert_eq!(ran.lines().filter(|line| *line == "commit-msg").count(), 1, "{ran}");
    assert_eq!(git(root, &["log", "-1", "--format=%B"]).matches("Prepared-by: hook").count(), 1);
}

/// Concluding a merge is staged like any commit: the signature is made before the store is
/// locked, so a signer waiting on its pinentry holds up nothing else, and the transaction only
/// publishes the object and clears the merge's state as `git commit` would.
#[test]
fn concluding_a_merge_signs_with_the_store_free() {
    use std::os::unix::fs::PermissionsExt;
    let e = engine();
    let (ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("side.txt"), "side\n").unwrap();
    git(root, &["add", "side.txt"]);
    git(root, &["commit", "-q", "-m", "Side"]);
    git(root, &["switch", "-q", "main"]);
    let main = commit(root, "Main");
    git(root, &["merge", "-q", "--no-commit", "--no-ff", "side"]);
    // A signer that waits, as one on a pinentry does, until it is let go — or gives up and says so.
    let dir = ws.path();
    let signer = dir.join("signer.sh");
    std::fs::write(&signer, format!(
        "#!/bin/sh\ncat > /dev/null\ntouch '{d}/started'\ni=0\nwhile [ ! -e '{d}/release' ]; do\n  i=$((i+1)); if [ $i -gt 200 ]; then touch '{d}/timed-out'; break; fi\n  sleep 0.05\ndone\necho '[GNUPG:] SIG_CREATED ' >&2\nprintf -- '-----BEGIN PGP SIGNATURE-----\\n\\nfake\\n-----END PGP SIGNATURE-----\\n'\n",
        d = dir.display(),
    )).unwrap();
    std::fs::set_permissions(&signer, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(root, &["config", "commit.gpgsign", "true"]);
    git(root, &["config", "gpg.program", signer.to_str().unwrap()]);

    let committing = {
        let e = e.clone();
        std::thread::spawn(move || call(&e, "git.commit", json!({"project_id":1,"message":"Merge side"})))
    };
    common::wait_until("the signer to start", || dir.join("started").exists());
    ok(&e, "project.list", json!({}));
    std::fs::write(dir.join("release"), "").unwrap();
    let out = committing.join().unwrap().into_result().unwrap();
    assert!(!dir.join("timed-out").exists(), "the store waited on the signer");

    assert_eq!(out["sha"], git(root, &["rev-parse", "HEAD"]).as_str());
    assert_eq!(git(root, &["rev-parse", "HEAD^1"]), main);
    assert_eq!(git(root, &["rev-parse", "HEAD^2"]), git(root, &["rev-parse", "side"]));
    assert!(git(root, &["cat-file", "commit", "HEAD"]).contains("gpgsig"), "the merge was signed");
    assert_eq!(git(root, &["reflog", "-1", "--format=%gs"]), "commit (merge): Merge side");
    for state in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE"] {
        assert!(!root.join(".git").join(state).exists(), "{state} was left behind");
    }
    assert!(git(root, &["status", "--porcelain"]).is_empty());
}

/// Concluding a cherry-pick keeps the picked commit's author, as `git commit` does, and clears
/// the pick.
#[test]
fn concluding_a_cherry_pick_keeps_its_author() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("README.md"), "# Side\n").unwrap();
    let out = git_command(root)
        .env("GIT_AUTHOR_NAME", "Pat Picked")
        .env("GIT_AUTHOR_EMAIL", "pat@example.test")
        .env("GIT_AUTHOR_DATE", "1600000000 +0200")
        .args(["commit", "-q", "-am", "Side"])
        .output()
        .unwrap();
    assert!(out.status.success());
    git(root, &["switch", "-q", "main"]);
    std::fs::write(root.join("README.md"), "# Main\n").unwrap();
    git(root, &["commit", "-q", "-am", "Main"]);
    let main = git(root, &["rev-parse", "HEAD"]);
    assert!(!git_command(root).args(["cherry-pick", "side"]).output().unwrap().status.success(), "the fixture must conflict");
    std::fs::write(root.join("README.md"), "# Both\n").unwrap();
    ok(&e, "git.stage", json!({"project_id":1,"paths":["README.md"]}));
    ok(&e, "git.commit", json!({"project_id":1,"message":"Side, picked"}));

    assert_eq!(git(root, &["log", "-1", "--format=%an|%ae|%at|%s"]), "Pat Picked|pat@example.test|1600000000|Side, picked");
    assert_eq!(git(root, &["rev-parse", "HEAD^"]), main);
    assert_eq!(git(root, &["reflog", "-1", "--format=%gs"]), "commit (cherry-pick): Side, picked");
    assert!(!root.join(".git/CHERRY_PICK_HEAD").exists());
}

/// A repository like [`project`]'s, its refs stored in `format` (`files` or `reftable`).
fn project_in(e: &Engine, format: &str) -> (tempfile::TempDir, String) {
    let ws = tempfile::tempdir().unwrap();
    let repo = ws.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main", &format!("--ref-format={format}")]);
    for (key, value) in [("user.name", common::GIT_NAME), ("user.email", common::GIT_EMAIL), ("commit.gpgsign", "false")] {
        git(&repo, &["config", key, value]);
    }
    std::fs::write(repo.join("README.md"), "# Relay\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-q", "-m", "Initial"]);
    let path = std::fs::canonicalize(&repo).unwrap().display().to_string();
    ok(e, "workspace.create", json!({"path":std::fs::canonicalize(ws.path()).unwrap()}));
    ok(e, "project.add", json!({"workspace_id":1,"path":path}));
    (ws, path)
}

/// A `gpg.program` that signs anything and calls every signature good, and says so on stdout
/// when verifying, as gpg does with log.showSignature.
fn fake_signer(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let signer = dir.join("signer.sh");
    std::fs::write(&signer, "#!/bin/sh\ncat > /dev/null\ncase \"$*\" in\n  *--verify*) echo '[GNUPG:] GOODSIG 0 Signer <s@x>'; echo 'gpg: Good signature from Signer <s@x>' >&2 ;;\n  *) echo '[GNUPG:] SIG_CREATED ' >&2; printf -- '-----BEGIN PGP SIGNATURE-----\\n\\nfake\\n-----END PGP SIGNATURE-----\\n' ;;\nesac\n").unwrap();
    std::fs::set_permissions(&signer, std::fs::Permissions::from_mode(0o755)).unwrap();
    signer
}

/// Concluding the first, conflicted pick of `cherry-pick A B` through the bus, in either ref
/// storage: A's author is kept even when log.showSignature would print a signature check into a
/// log format, CHERRY_PICK_HEAD — a ref, which reftable keeps in its tables — is cleared with
/// the commit, and `cherry-pick --continue` goes on to B.
#[test]
fn concluding_a_pick_of_a_sequence_clears_it_in_either_ref_storage() {
    for format in ["files", "reftable"] {
        let e = engine();
        let (ws, repo) = project_in(&e, format);
        let root = Path::new(&repo);
        git(root, &["config", "gpg.program", fake_signer(ws.path()).to_str().unwrap()]);
        git(root, &["switch", "-q", "-c", "side"]);
        std::fs::write(root.join("README.md"), "# Side\n").unwrap();
        let out = git_command(root)
            .env("GIT_AUTHOR_NAME", "Pat Picked")
            .env("GIT_AUTHOR_EMAIL", "pat@example.test")
            .env("GIT_AUTHOR_DATE", "1600000000 +0200")
            .args(["commit", "-q", "-S", "-am", "Side A"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        std::fs::write(root.join("b.txt"), "b\n").unwrap();
        git(root, &["add", "b.txt"]);
        git(root, &["commit", "-q", "-m", "Side B"]);
        git(root, &["switch", "-q", "main"]);
        std::fs::write(root.join("README.md"), "# Main\n").unwrap();
        git(root, &["commit", "-q", "-am", "Main"]);
        git(root, &["config", "log.showSignature", "true"]);
        let picking = git_command(root).args(["cherry-pick", "side~1", "side"]).output().unwrap();
        assert!(!picking.status.success(), "{format}: the first pick must conflict");
        std::fs::write(root.join("README.md"), "# Both\n").unwrap();
        ok(&e, "git.stage", json!({"project_id":1,"paths":["README.md"]}));
        ok(&e, "git.commit", json!({"project_id":1,"message":"Side A, picked"}));

        assert_eq!(git(root, &["log", "-1", "--no-show-signature", "--format=%an|%ae|%at"]), "Pat Picked|pat@example.test|1600000000", "{format}");
        assert_eq!(git(root, &["reflog", "-1", "--format=%gs"]), "commit (cherry-pick): Side A, picked", "{format}");
        let left = git_command(root).args(["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"]).output().unwrap();
        assert!(!left.status.success(), "{format}: CHERRY_PICK_HEAD was left behind");
        assert!(root.join(".git/sequencer").is_dir(), "{format}: B is still to come");
        git(root, &["cherry-pick", "--continue"]);
        assert_eq!(git(root, &["log", "-2", "--no-show-signature", "--format=%s"]), "Side B\nSide A, picked", "{format}");
        assert!(!root.join(".git/sequencer").exists(), "{format}");
    }
}

/// `git commit -m` keeps a `# Conflicts:` block unless commit.cleanup is `strip`, and so does a
/// bus commit concluding a merge.
#[test]
fn commit_cleanup_strips_comments_only_when_configured() {
    for (cleanup, kept) in [(None, true), (Some("strip"), false)] {
        let e = engine();
        let (_ws, repo) = project(&e);
        let root = Path::new(&repo);
        if let Some(cleanup) = cleanup {
            git(root, &["config", "commit.cleanup", cleanup]);
        }
        git(root, &["switch", "-q", "-c", "side"]);
        std::fs::write(root.join("side.txt"), "side\n").unwrap();
        git(root, &["add", "side.txt"]);
        git(root, &["commit", "-q", "-m", "Side"]);
        git(root, &["switch", "-q", "main"]);
        commit(root, "Main");
        git(root, &["merge", "-q", "--no-commit", "--no-ff", "side"]);
        ok(&e, "git.commit", json!({"project_id":1,"message":"Merge side\n\n# Conflicts:\n#\tREADME.md\n"}));
        let message = git(root, &["log", "-1", "--format=%B"]);
        assert_eq!(message.contains("# Conflicts:"), kept, "{cleanup:?}: {message}");
        assert!(message.starts_with("Merge side"), "{message}");
    }
}

/// rerere records a resolution concluded through the bus even when no merge or pick head says a
/// conflict was being resolved: a rebase stopped on one writes neither.
#[test]
fn rerere_records_a_rebase_resolution_committed_through_the_bus() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    git(root, &["config", "rerere.enabled", "true"]);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("README.md"), "# Side\n").unwrap();
    git(root, &["commit", "-q", "-am", "Side"]);
    git(root, &["switch", "-q", "main"]);
    std::fs::write(root.join("README.md"), "# Main\n").unwrap();
    git(root, &["commit", "-q", "-am", "Main"]);
    git(root, &["switch", "-q", "side"]);
    assert!(!git_command(root).args(["rebase", "main"]).output().unwrap().status.success(), "the fixture must conflict");
    assert!(git_command(root).args(["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"]).output().unwrap().stdout.is_empty());
    std::fs::write(root.join("README.md"), "# Both\n").unwrap();
    ok(&e, "git.stage", json!({"project_id":1,"paths":["README.md"]}));
    ok(&e, "git.commit", json!({"project_id":1,"message":"Side, rebased"}));
    let postimage = || {
        std::fs::read_dir(root.join(".git/rr-cache")).into_iter().flatten().flatten()
            .any(|entry| entry.path().join("postimage").exists())
    };
    common::wait_until("rerere to record the resolution", postimage);
}

/// The stash `merge --squash --autostash` set aside — left for the commit when the squash
/// conflicts — comes back once that is committed, though nothing was pending as a merge.
#[test]
fn a_squash_autostash_comes_back_after_the_commit() {
    let e = engine();
    let (_ws, repo) = project(&e);
    let root = Path::new(&repo);
    std::fs::write(root.join("notes.txt"), "notes\n").unwrap();
    git(root, &["add", "notes.txt"]);
    git(root, &["commit", "-q", "-m", "Notes"]);
    git(root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("README.md"), "# Side\n").unwrap();
    git(root, &["commit", "-q", "-am", "Side"]);
    git(root, &["switch", "-q", "main"]);
    std::fs::write(root.join("README.md"), "# Main\n").unwrap();
    git(root, &["commit", "-q", "-am", "Main"]);
    std::fs::write(root.join("notes.txt"), "notes, being edited\n").unwrap();
    assert!(!git_command(root).args(["merge", "-q", "--squash", "--autostash", "side"]).output().unwrap().status.success(), "the fixture must conflict");
    assert!(!root.join(".git/MERGE_HEAD").exists());
    assert_eq!(std::fs::read_to_string(root.join("notes.txt")).unwrap(), "notes\n", "the edit was stashed");
    std::fs::write(root.join("README.md"), "# Both\n").unwrap();
    ok(&e, "git.stage", json!({"project_id":1,"paths":["README.md"]}));
    ok(&e, "git.commit", json!({"project_id":1,"message":"Squash side"}));
    common::wait_until("the autostash to come back", || {
        std::fs::read_to_string(root.join("notes.txt")).unwrap() == "notes, being edited\n"
    });
    common::wait_until("MERGE_AUTOSTASH to go", || {
        !git_command(root).args(["rev-parse", "-q", "--verify", "MERGE_AUTOSTASH"]).output().unwrap().status.success()
    });
    assert!(!root.join(".git/SQUASH_MSG").exists());
    assert_eq!(git(root, &["log", "-1", "--format=%p"]).split(' ').count(), 1, "a squash is not a merge");
}
