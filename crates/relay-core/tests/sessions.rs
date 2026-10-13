//! Phase 3: worktrees, PTY sessions, teardown (SPEC §16 "spawn/kill N sessions, assert zero
//! orphans"), token binding, crash recovery. All through the bus.

mod common;

use common::{call, code, committed_repo, engine_with_project, git, git_command, ok, wait_until};
use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::socket::{Client, Line, SocketServer};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The TERM grace `session.close` gives a child before SIGKILL. It is a literal in
/// handlers/session.rs (`pty.kill(Duration::from_millis(150))`, in both `retire_session` and
/// close's unlocked phase); keep the two in step.
const CLOSE_GRACE: Duration = Duration::from_millis(150);

/// A workspace with one real git repo (one commit on `main`) and a store beside it.
struct Fixture {
    _root: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    engine: Arc<Engine>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let ws = root.join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "hi\n")]);
    let engine = engine_with_project(&root, &ws, &repo);
    Fixture { _root: tmp, root, repo, engine }
}

fn head(repo: &Path, reference: &str) -> String {
    git(repo, &["rev-parse", reference])
}

fn advancing_remote(f: &Fixture, tracking: bool) -> PathBuf {
    let origin = f.root.join("remote");
    git(&f.root, &["clone", f.repo.to_str().unwrap(), origin.to_str().unwrap()]);
    git(&f.repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&f.repo, &["fetch", "origin"]);
    if tracking {
        git(&f.repo, &["branch", "--set-upstream-to=origin/main", "main"]);
    }
    std::fs::write(origin.join("latest.txt"), "remote change\n").unwrap();
    git(&origin, &["add", "latest.txt"]);
    git(&origin, &["commit", "-qm", "remote advance"]);
    origin
}

#[test]
fn new_sessions_fetch_remote_commits_without_touching_primary() {
    for tracking in [true, false] {
        let f = fixture();
        let origin = advancing_remote(&f, tracking);
        let original = head(&f.repo, "HEAD");
        std::fs::write(f.repo.join("README.md"), "uncommitted primary work\n").unwrap();
        for round in 0..2 {
            git(&origin, &["commit", "--allow-empty", "-qm", &format!("advance {round}")]);
            let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}));
            let checkout = Path::new(session["worktree"].as_str().unwrap());
            assert_eq!(head(checkout, "HEAD"), head(&origin, "HEAD"));
            assert!(checkout.join("latest.txt").exists());
            assert_eq!(head(&f.repo, "HEAD"), original);
            assert_eq!(std::fs::read_to_string(f.repo.join("README.md")).unwrap(), "uncommitted primary work\n");
        }
    }
}

#[test]
fn new_worktree_defaults_fetch_but_explicit_revisions_and_existing_branches_are_preserved() {
    let f = fixture();
    let origin = advancing_remote(&f, true);
    let original = head(&f.repo, "HEAD");
    let worktree = ok(&f.engine, "worktree.create", json!({"project_id":1,"branch":"relay/fresh"}));
    assert_eq!(head(Path::new(worktree["path"].as_str().unwrap()), "HEAD"), head(&origin, "HEAD"));
    git(&f.repo, &["remote", "set-url", "origin", f.root.join("missing").to_str().unwrap()]);
    let pinned = ok(&f.engine, "worktree.create", json!({"project_id":1,"branch":"relay/pinned","from":original}));
    assert_eq!(head(Path::new(pinned["path"].as_str().unwrap()), "HEAD"), original);
    git(&f.repo, &["branch", "relay/existing", &original]);
    let existing = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"codex","branch":"relay/existing"}));
    assert_eq!(head(Path::new(existing["worktree"].as_str().unwrap()), "HEAD"), original);
    let pair = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude","role":"reviewer","pair_with":existing["name"]}));
    assert_eq!(pair["worktree"], existing["worktree"]);
}

#[test]
fn new_sessions_refuse_failed_fetch_and_divergence_without_stale_fallback() {
    let f = fixture();
    advancing_remote(&f, true);
    git(&f.repo, &["commit", "--allow-empty", "-qm", "local advance"]);
    let original = head(&f.repo, "HEAD");
    assert_eq!(code(call(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}))), "git.base_diverged");
    git(&f.repo, &["remote", "set-url", "origin", f.root.join("missing").to_str().unwrap()]);
    assert_eq!(code(call(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}))), "git.fetch_failed");
    assert_eq!(head(&f.repo, "HEAD"), original);
    assert_eq!(ok(&f.engine, "worktree.list", json!({"project_id":1}))["worktrees"].as_array().unwrap().len(), 1);
    assert!(ok(&f.engine, "session.list", json!({"project_id":1}))["sessions"].as_array().unwrap().is_empty());
}

#[test]
fn new_sessions_preserve_local_ahead_base_and_reject_missing_base() {
    let f = fixture();
    let origin = advancing_remote(&f, true);
    git(&f.repo, &["fetch", "origin"]);
    git(&f.repo, &["merge", "--ff-only", "origin/main"]);
    git(&f.repo, &["commit", "--allow-empty", "-qm", "local only"]);
    let local = head(&f.repo, "HEAD");
    assert_ne!(local, head(&origin, "HEAD"));
    let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}));
    assert_eq!(head(Path::new(session["worktree"].as_str().unwrap()), "HEAD"), local);
    ok(&f.engine, "project.update", json!({"project_id":1,"base_branch":"missing"}));
    assert_eq!(code(call(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}))), "git.base_missing");
}

/// A stand-in provider CLI: greets, echoes lines, exits 3 on "exit". Its `sleep` child is
/// what the teardown test looks for.
fn fake_provider(dir: &Path, with_child: bool) -> PathBuf {
    let p = dir.join("fake-claude.sh");
    let body = if with_child {
        "#!/bin/sh\necho hello-from-pty\nsleep 300 &\necho $! > \"$PWD/child.pid\"\nwhile IFS= read -r line; do echo \"echo:$line\"; [ \"$line\" = exit ] && exit 3; done\n"
    } else {
        "#!/bin/sh\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; [ \"$line\" = exit ] && exit 3; done\n"
    };
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn fake_discovery_provider(dir: &Path, binary: &str, version: &str) -> PathBuf {
    let path = dir.join(binary);
    let auth = if binary == "claude" {
        "echo '{\"loggedIn\":true,\"email\":\"agent@example.test\"}'"
    } else {
        "echo 'Logged in using ChatGPT'"
    };
    let body = format!(
        "#!/bin/sh\ncase \"${{1:-}}\" in\n  --version) echo '{version}'; exit 0;;\n  auth|login) {auth}; exit 0;;\nesac\nprintf '%s\\n' \"$@\" > \"$PWD/.relay/provider-args.txt\"\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; done\n"
    );
    std::fs::write(&path, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn alive(pid: i64) -> bool {
    relay_core::pty::pid_alive(pid as u32) && std::fs::read_to_string(format!("/proc/{pid}/stat")).map(|s| !s.contains(") Z ")).unwrap_or(false)
}

fn wait_for_pid(path: &Path) -> i64 {
    let mut pid = None;
    wait_until("child pid written", || {
        pid = std::fs::read_to_string(path).ok().and_then(|value| value.trim().parse().ok());
        pid.is_some()
    });
    pid.expect("wait completed with a parsed pid")
}

#[test]
fn worktree_ops() {
    let f = fixture();
    let e = &f.engine;
    let l = ok(e, "worktree.list", json!({"project_id": 1}));
    let wts = l["worktrees"].as_array().unwrap();
    assert_eq!(wts.len(), 1);
    assert_eq!(wts[0]["branch"], "main");
    assert_eq!(wts[0]["path"], f.repo.display().to_string());
    assert_eq!(wts[0]["dirty"], false);
    // create
    let w = ok(e, "worktree.create", json!({"project_id": 1, "branch": "relay/feature-x"}));
    assert_eq!(w["branch"], "relay/feature-x");
    let path = PathBuf::from(w["path"].as_str().unwrap());
    assert!(path.starts_with(f.repo.join(".relay/worktrees")));
    assert!(path.join("README.md").is_file());
    let excl = std::fs::read_to_string(f.repo.join(".git/info/exclude")).unwrap();
    assert!(excl.lines().any(|l| l == ".relay/"));
    assert_eq!(code(call(e, "worktree.create", json!({"project_id": 1, "branch": "relay/feature-x"}))), "worktree.exists");
    assert_eq!(ok(e, "worktree.list", json!({"project_id": 1}))["worktrees"].as_array().unwrap().len(), 2);
    // dirty detection through gix, no spawn
    std::fs::write(path.join("scratch.txt"), "x").unwrap();
    let l = ok(e, "worktree.list", json!({"project_id": 1}));
    let mine = l["worktrees"].as_array().unwrap().iter().find(|w| w["path"] == path.display().to_string()).unwrap();
    assert_eq!(mine["dirty"], true);
    // build output shows in disk, is purged on remove
    std::fs::create_dir_all(path.join("build")).unwrap();
    std::fs::write(path.join("build/big.bin"), vec![0u8; 512 * 1024]).unwrap();
    let d = ok(e, "worktree.disk", json!({"project_id": 1}));
    let mine = d["worktrees"].as_array().unwrap().iter().find(|w| w["path"] == path.display().to_string()).unwrap();
    assert!(mine["build_mb"].as_f64().unwrap() > 0.4, "{mine}");
    // scratch.txt is uncommitted work, and `git worktree remove --force` would delete it: this
    // used to go without a word. It is refused as `session.close` refuses it, until the caller
    // says to discard it.
    assert_eq!(code(call(e, "worktree.remove", json!({"project_id": 1, "path": path}))), "worktree.dirty");
    assert!(path.join("scratch.txt").is_file());
    let r = ok(e, "worktree.remove", json!({"project_id": 1, "path": path, "discard_changes": true}));
    assert!(r["freed_mb"].as_f64().unwrap() > 0.4);
    assert!(!path.exists());
    assert_eq!(ok(e, "worktree.list", json!({"project_id": 1}))["worktrees"].as_array().unwrap().len(), 1);
    // the branch survives removal (SPEC §8)
    assert!(git(&f.repo, &["branch", "--list", "relay/feature-x"]).contains("relay/feature-x"));
    // agents may not create/remove worktrees
    let r = e.dispatch(Request::new(Actor::agent("x"), "worktree.create", json!({"project_id": 1, "branch": "relay/y"})), Door::InProcess);
    assert_eq!(code(r), "actor.allowlist");
}

/// RA-017: `worktree.remove` purged build directories under, then `rm -rf`'d, any path it was
/// handed once git said "not a working tree". Only a linked worktree of the project goes now.
#[test]
fn worktree_remove_refuses_anything_that_is_not_a_worktree_of_the_project() {
    let f = fixture();
    let e = &f.engine;
    let assert_kept = |path: &Path, why: &str| {
        let r = call(e, "worktree.remove", json!({"project_id": 1, "path": path}));
        assert!(r.clone().into_result().is_err(), "{why}: {} was removed", path.display());
        assert!(path.join("keep.txt").is_file(), "{why}: contents were touched");
        assert!(path.join("build/out.bin").is_file(), "{why}: build output was purged");
    };
    let plain = f.root.join("notes");
    let other = f.root.join("other-repo");
    for dir in [&plain, &other] {
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::write(dir.join("keep.txt"), "mine").unwrap();
        std::fs::write(dir.join("build/out.bin"), "x").unwrap();
    }
    git(&other, &["init", "-q", "-b", "main"]);
    assert_kept(&plain, "a plain directory");
    assert_kept(&other, "another repository");

    // The pool directory itself is not a worktree, though every pooled one is inside it.
    let w = ok(e, "worktree.create", json!({"project_id": 1, "branch": "relay/keep"}));
    let pooled = PathBuf::from(w["path"].as_str().unwrap());
    let pool = f.repo.join(".relay/worktrees");
    std::fs::create_dir_all(pool.join("build")).unwrap();
    std::fs::write(pool.join("keep.txt"), "mine").unwrap();
    std::fs::write(pool.join("build/out.bin"), "x").unwrap();
    assert_kept(&pool, "the pool directory");
    assert!(pooled.join("README.md").is_file());

    // A locked worktree is one git refuses to remove; that refusal now stands.
    let locked = f.root.join("locked-wt");
    git(&f.repo, &["worktree", "add", "-q", "-b", "locked", locked.to_str().unwrap()]);
    git(&f.repo, &["worktree", "lock", locked.to_str().unwrap()]);
    std::fs::create_dir_all(locked.join("build")).unwrap();
    std::fs::write(locked.join("keep.txt"), "mine").unwrap();
    std::fs::write(locked.join("build/out.bin"), "x").unwrap();
    assert_kept(&locked, "a locked worktree");

    assert_eq!(code(call(e, "worktree.remove", json!({"project_id": 1, "path": "."}))), "worktree.path");
    // And a real pooled worktree still goes.
    ok(e, "worktree.remove", json!({"project_id": 1, "path": pooled}));
    assert!(!pooled.exists());
}

#[test]
fn launch_passes_configured_and_memory_write_roots_to_both_providers() {
    for provider in ["claude", "codex"] {
        let f = fixture();
        let binary = fake_discovery_provider(&f.root, provider, "fixture 1.0");
        ok(&f.engine, "settings.set", json!({"path":format!("providers.{provider}.path"),"value":binary}));
        // Outside the temp dir, which is always a write root, so only the setting can let a
        // write land here; the sibling is the same kind of directory, not configured (RA-675).
        let outside = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        let outside = std::fs::canonicalize(outside.path()).unwrap();
        let extra = outside.join("agent notes");
        let sibling = outside.join("not listed");
        std::fs::create_dir_all(&extra).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        ok(&f.engine, "settings.set", json!({"path":"guardrails.allowed_write_roots","value":[extra]}));
        let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":provider}));
        let worktree = PathBuf::from(session["worktree"].as_str().unwrap());
        ok(&f.engine, "session.spawn", json!({"session":session["name"]}));
        let captured = worktree.join(".relay/provider-args.txt");
        wait_until("provider argv", || captured.is_file());
        let args = std::fs::read_to_string(captured).unwrap();
        let args: Vec<_> = args.lines().collect();
        // The roots written out from their documented locations, not asked of the code that
        // builds them: each provider's memory folder, the configured root, and the temp dir.
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
        let claude = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude"));
        let project_key = |path: &Path| -> String {
            path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
        };
        let expected = [
            codex.join("memories"),
            claude.join("projects").join(project_key(&worktree)).join("memory"),
            claude.join("projects").join(project_key(&f.repo)).join("memory"),
            extra.clone(),
            std::env::temp_dir(),
        ];
        let granted: Vec<PathBuf> = args.windows(2).filter(|pair| pair[0] == "--add-dir").map(|pair| PathBuf::from(pair[1])).collect();
        assert_eq!(granted, expected, "{provider}: --add-dir grants");
        let check = |path: PathBuf| f.engine.dispatch(Request::new(Actor::agent(session["name"].as_str().unwrap()), "guardrail.check", json!({
            "project_id":1,"kind":"write","path":path,"new_text":"note"
        })), Door::InProcess).into_result().unwrap();
        for root in &expected {
            assert_eq!(check(root.join("note.md"))["verdict"], "allow", "{provider}: {root:?}");
        }
        let refused = check(sibling.join("note.md"));
        assert_ne!(refused["verdict"], "allow", "{provider}: an unlisted directory is not a write root");
        assert_eq!(refused["error"]["code"], "guardrail.write_root", "{refused}");
        ok(&f.engine, "session.close", json!({"session":session["name"]}));
    }
}

#[test]
fn closing_a_provider_that_ignores_term_is_bounded_and_preserves_its_worktree() {
    let f = fixture();
    let provider = fake_provider(&f.root, false);
    std::fs::write(&provider, "#!/bin/sh\ntrap '' TERM\necho ready\nwhile IFS= read -r line; do :; done\n").unwrap();
    ok(&f.engine, "settings.set", json!({"path":"providers.claude.path","value":provider}));
    let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude"}));
    let spawned = ok(&f.engine, "session.spawn", json!({"session":session["name"]}));
    wait_until("ignoring TERM", || ok(&f.engine,"session.scrollback",json!({"session":session["name"]}))["text"].as_str().unwrap().contains("ready"));
    let pid = spawned["pid"].as_i64().unwrap();
    let start = Instant::now();
    ok(&f.engine, "session.close", json!({"session":session["name"],"remove_worktree":false}));
    // The close answers without waiting out the TERM grace; the kill finishes on its own thread
    // (RA-742). The child ignores TERM, so it can only be dead already if close waited the grace
    // out itself — and a close quicker than the grace cannot have waited it out.
    let took = start.elapsed();
    assert!(alive(pid) || took < CLOSE_GRACE, "close waited out the TERM grace: {took:?}, and the child is gone");
    wait_until("the TERM-ignoring child to be killed", || !alive(pid));
    assert!(start.elapsed() < Duration::from_secs(3), "kill took {:?}", start.elapsed());
    assert_eq!(ok(&f.engine, "session.list", json!({"include_closed":true}))["sessions"][0]["state"], "closed");
    assert!(Path::new(session["worktree"].as_str().unwrap()).join("README.md").is_file());
}

#[test]
fn session_create_and_worktree_ownership() {
    let f = fixture();
    let e = &f.engine;
    let s = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    assert!(name.split('-').count() == 2, "{name}");
    assert_eq!(s["state"], "created");
    assert_eq!(s["branch"], format!("relay/{name}"));
    let wt = PathBuf::from(s["worktree"].as_str().unwrap());
    assert!(wt.join("README.md").is_file());
    assert_eq!(s["role"], "builder");
    // the worktree is listed as owned; cannot be removed while owned
    let l = ok(e, "worktree.list", json!({"project_id": 1}));
    let mine = l["worktrees"].as_array().unwrap().iter().find(|w| w["path"] == wt.display().to_string()).unwrap();
    assert_eq!(mine["session"], name);
    assert_eq!(code(call(e, "worktree.remove", json!({"project_id": 1, "path": wt}))), "worktree.owned");
    // primary checkout
    let s2 = ok(e, "session.create", json!({"project_id": 1, "provider": "codex", "worktree": "primary", "role": "reviewer"}));
    assert_eq!(s2["worktree"], f.repo.display().to_string());
    assert_eq!(s2["branch"], "main");
    assert_ne!(s2["name"], name);
    // list / get
    assert_eq!(ok(e, "session.list", json!({})).as_object().unwrap()["sessions"].as_array().unwrap().len(), 2);
    assert_eq!(ok(e, "session.get", json!({"session": name}))["id"], 1);
    assert_eq!(code(call(e, "session.get", json!({"session": "nope-nope"}))), "session.not_found");
    // close: worktree gone, state closed, name free again for listing purposes
    let r = ok(e, "session.close", json!({"session": name}));
    assert!(r["freed_mb"].as_f64().unwrap() >= 0.0);
    assert!(!wt.exists());
    assert_eq!(code(call(e, "session.get", json!({"session": name}))), "session.not_found");
    let all = ok(e, "session.list", json!({"include_closed": true}));
    assert_eq!(all["sessions"].as_array().unwrap().len(), 2);
    // closing the primary-checkout session never removes the repo
    ok(e, "session.close", json!({"session": s2["name"]}));
    assert!(f.repo.join("README.md").is_file());
    // spawn without a provider on PATH / configured is a typed unavailable
    let s3 = ok(e, "session.create", json!({"project_id": 1, "provider": "codex"}));
    ok(e, "settings.set", json!({"path": "providers.codex.path", "value": "/nonexistent/codex"}));
    assert_eq!(code(call(e, "session.spawn", json!({"session": s3["name"]}))), "provider.not_installed");
}

/// Read an attached client's frames into `seen` until it holds `needle`, handing each frame to
/// `frame` first. A stream that closes or sends a line that does not decode fails at once,
/// saying so, rather than spinning until the deadline as if the PTY had gone quiet (RA-676).
async fn read_until(term: &mut Client, needle: &str, seen: &mut Vec<u8>, mut frame: impl FnMut(&relay_bus::envelope::Frame)) {
    use base64::Engine as _;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !String::from_utf8_lossy(seen).contains(needle) {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "no {needle:?}; got {:?}", String::from_utf8_lossy(seen));
        match tokio::time::timeout(left, term.next()).await {
            Ok(Ok(Some(Line::Frame(fr)))) => {
                frame(&fr);
                seen.extend(base64::engine::general_purpose::STANDARD.decode(fr.data.as_str().unwrap()).unwrap());
            }
            Ok(Ok(Some(_))) | Err(_) => {}
            Ok(Ok(None)) => panic!("attach stream closed before {needle:?}; got {:?}", String::from_utf8_lossy(seen)),
            Ok(Err(error)) => panic!("attach stream error before {needle:?}: {error:#}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_lifecycle_over_socket() {
    let f = fixture();
    let e = f.engine.clone();
    let prov = fake_provider(&f.root, false);
    ok(&e, "settings.set", json!({"path": "providers.claude.path", "value": prov}));
    let sockdir = f.root.join("run");
    let server = SocketServer::start_in(e.clone(), sockdir.clone()).await.unwrap();

    let s = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    // events subscriber
    let mut sub = Client::connect(&server.path).await.unwrap();
    sub.call(&Request::new(Actor::User, "bus.subscribe", json!({"events": ["session.*"]})), |_| {}).await.unwrap();

    let spawn_req = Request::new(Actor::User, "session.spawn", json!({"session": name}));
    let spawn_id = spawn_req.id;
    let s = e.dispatch(spawn_req, Door::InProcess).into_result().unwrap();
    assert_eq!(s["state"], "running");
    let pid = s["pid"].as_i64().unwrap();
    assert!(alive(pid));
    assert_eq!(code(call(&e, "session.spawn", json!({"session": name}))), "session.already_spawned");

    // token binding on the socket door
    let token: String = e.store.lock().query_row("SELECT token FROM sessions WHERE name = ?1", [&name], |r| r.get(0)).unwrap();
    let mut c = Client::connect(&server.path).await.unwrap();
    let r = c.call(&Request::new(Actor::agent(&name), "session.get", json!({"session": name})).with_token("wrong"), |_| {}).await.unwrap();
    assert_eq!(code(r), "bus.actor");
    let r = c.call(&Request::new(Actor::agent(&name), "session.get", json!({"session": name})).with_token(&token), |_| {}).await.unwrap();
    assert!(r.ok, "{:?}", r.error);
    // an agent sees the metadata of a peer in its own project (D106) but not its private
    // surfaces, and nothing at all outside the project
    let other = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let r = c.call(&Request::new(Actor::agent(&name), "session.get", json!({"session": other["name"]})).with_token(&token), |_| {}).await.unwrap();
    assert!(r.ok, "{:?}", r.error);
    let r = c.call(&Request::new(Actor::agent(&name), "session.scrollback", json!({"session": other["name"]})).with_token(&token), |_| {}).await.unwrap();
    assert_eq!(code(r), "actor.scope", "a peer's scrollback stays private");
    let r = c.call(&Request::new(Actor::agent(&name), "session.brief", json!({"session": other["name"]})).with_token(&token), |_| {}).await.unwrap();
    assert_eq!(code(r), "actor.scope", "a peer's brief stays private");
    // ...nor close one (user-only)
    let r = c.call(&Request::new(Actor::agent(&name), "session.close", json!({"session": name})).with_token(&token), |_| {}).await.unwrap();
    assert_eq!(code(r), "actor.allowlist");

    // attach: catch-up + live frames
    let mut term = Client::connect(&server.path).await.unwrap();
    let r = term.call(&Request::new(Actor::User, "session.attach", json!({"session": name})), |_| {}).await.unwrap();
    assert!(r.ok, "{:?}", r.error);
    let mut seen = Vec::new();
    let mut last_seq = 0u64;
    let mut epoch = 0u64;
    read_until(&mut term, "hello-from-pty", &mut seen, |fr| {
        assert_eq!(fr.stream, "pty");
        assert_eq!(fr.session.as_deref(), Some(name.as_str()));
        epoch = fr.epoch.unwrap();
        assert!(fr.seq >= last_seq);
        last_seq = fr.seq;
    }).await;
    assert_eq!(epoch, 1);
    // input → echo
    ok(&e, "session.input", json!({"session": name, "data": "ping\n"}));
    read_until(&mut term, "echo:ping", &mut seen, |fr| last_seq = fr.seq).await;
    // resize + scrollback
    assert_eq!(ok(&e, "session.scrollback", json!({"session": name}))["cols"].as_u64(), Some(120));
    ok(&e, "session.resize", json!({"session": name, "cols": 200, "rows": 50}));
    let sb = ok(&e, "session.scrollback", json!({"session": name}));
    assert_eq!((sb["cols"].as_u64(), sb["rows"].as_u64()), (Some(200), Some(50)), "the size a client hands back");
    assert!(sb["text"].as_str().unwrap().contains("hello-from-pty"));
    assert!(sb["text"].as_str().unwrap().contains("echo:ping"));
    assert_eq!(sb["epoch"], 1);
    assert!(sb["seq"].as_u64().unwrap() >= last_seq);
    // a second attach from (epoch, seq) replays only what came after
    let mut term2 = Client::connect(&server.path).await.unwrap();
    term2.call(&Request::new(Actor::User, "session.attach", json!({"session": name, "epoch": epoch, "from_seq": sb["seq"]})), |_| {}).await.unwrap();
    ok(&e, "session.input", json!({"session": name, "data": "again\n"}));
    let mut seen2 = Vec::new();
    read_until(&mut term2, "echo:again", &mut seen2, |_| {}).await;
    assert!(!String::from_utf8_lossy(&seen2).contains("hello-from-pty"), "catch-up replayed history it should have skipped: {:?}", String::from_utf8_lossy(&seen2));
    // detach stops frames on term2, and only there: output that still reaches `term` sends
    // term2 nothing (RA-677). Frames already in flight arrive before the detach answer.
    let detached = term2.call(&Request::new(Actor::User, "session.detach", json!({"session": name})), |_| {}).await.unwrap();
    assert!(detached.ok, "{:?}", detached.error);
    ok(&e, "session.input", json!({"session": name, "data": "quiet\n"}));
    read_until(&mut term, "echo:quiet", &mut seen, |_| {}).await;
    let quiet_until = Instant::now() + Duration::from_millis(500);
    while let Some(left) = quiet_until.checked_duration_since(Instant::now()) {
        match tokio::time::timeout(left, term2.next()).await {
            Err(_) => break,
            Ok(Ok(Some(Line::Frame(fr)))) => panic!("a detached client still gets frames: {fr:?}"),
            Ok(Ok(Some(_))) => {}
            Ok(Ok(None)) => panic!("detach closed the connection"),
            Ok(Err(error)) => panic!("term2 stream error after detach: {error:#}"),
        }
    }

    // process exit → session.changed(exited, 3) event + completion audit row
    ok(&e, "session.input", json!({"session": name, "data": "exit\n"}));
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut exited = false;
    while !exited {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "no exit event");
        match tokio::time::timeout(left, sub.next()).await {
            Ok(Ok(Some(Line::Event(ev)))) => {
                if ev.ev == "session.changed" && ev.payload["name"] == name && ev.payload["state"] == "exited" {
                    assert_eq!(ev.payload["exit_code"], 3);
                    assert_eq!(ev.actor, Actor::System);
                    exited = true;
                }
            }
            Ok(Ok(Some(_))) | Err(_) => {}
            Ok(Ok(None)) => panic!("event stream closed before the exit event"),
            Ok(Err(error)) => panic!("event stream error before the exit event: {error:#}"),
        }
    }
    wait_until("pid gone", || !alive(pid));
    let rows = ok(&e, "audit.list", json!({"op_prefix": "session.spawn.completed"}));
    let row = &rows["rows"][0];
    assert_eq!(row["parent_req"], spawn_id.to_string());
    assert_eq!(row["actor"], "system");
    assert_eq!(row["result_summary"]["exit_code"], 3);
    assert_eq!(code(call(&e, "session.input", json!({"session": name, "data": "x"}))), "session.exited");
    // scrollback survives exit; close tears it all down
    assert!(ok(&e, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("echo:again"));
    // An exited session relaunches in place, carrying the dead PTY's history into the new one.
    let resumed = ok(&e, "session.resume", json!({"session": name}));
    assert_eq!(resumed["state"], "running");
    assert_ne!(resumed["pid"], pid);
    assert!(ok(&e, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("echo:again"));
    let wt = PathBuf::from(s["worktree"].as_str().unwrap());
    ok(&e, "session.close", json!({"session": name}));
    assert!(!wt.exists());
    let r = c.call(&Request::new(Actor::agent(&name), "session.get", json!({"session": name})).with_token(&token), |_| {}).await.unwrap();
    assert_eq!(code(r), "bus.actor", "token revoked with the session");
    drop(server);
}

/// Run `held` while another thread holds the store's one connection, as a slow handler would.
/// The lock is let go when `held` returns or panics.
fn with_store_held<T>(e: &Arc<Engine>, held: impl FnOnce() -> T) -> T {
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let engine = e.clone();
    let holder = std::thread::spawn(move || {
        let _conn = engine.store.lock();
        locked_tx.send(()).unwrap();
        let _ = release_rx.recv();
    });
    locked_rx.recv().unwrap();
    let out = held();
    drop(release_tx);
    holder.join().unwrap();
    out
}

/// Dispatch on a thread of its own; `None` when no answer came within `within`. The thread is
/// left behind, to finish once whatever it waits on is let go.
fn answer_within(e: &Arc<Engine>, op: &str, payload: Value, within: Duration) -> Option<Response> {
    let (tx, rx) = std::sync::mpsc::channel();
    let engine = e.clone();
    let req = Request::new(Actor::User, op, payload);
    std::thread::spawn(move || {
        let _ = tx.send(engine.dispatch(req, Door::InProcess));
    });
    rx.recv_timeout(within).ok()
}

/// D148: input reaches the PTY without a transaction, and still carries the one state change
/// that a keystroke owes the store — the idle→running edge — on a worker.
#[test]
fn input_bypasses_the_store_but_still_wakes_an_idle_session() {
    let f = fixture();
    let e = &f.engine;
    let prov = fake_provider(&f.root, false);
    ok(e, "settings.set", json!({"path": "providers.claude.path", "value": prov}));
    let s = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();

    // An unspawned session has no PTY to resolve from memory, so the request must still fall
    // through to the handler and get the exact typed refusal.
    assert_eq!(
        code(call(e, "session.input", json!({"session": name, "data": "x"}))),
        "session.not_spawned"
    );
    assert_eq!(
        code(call(e, "session.input", json!({"session": "no-such-session", "data": "x"}))),
        "session.not_found"
    );

    ok(e, "session.spawn", json!({"session": &name}));
    let session_id: i64 = e.store.lock()
        .query_row("SELECT id FROM sessions WHERE name = ?1", [&name], |r| r.get(0))
        .unwrap();

    let state = || -> String {
        e.store.lock()
            .query_row("SELECT state FROM sessions WHERE id = ?1", [session_id], |r| r.get(0))
            .unwrap()
    };
    // The provider says it went idle; the live PTY learns it too.
    e.store.lock()
        .execute("UPDATE sessions SET state='idle' WHERE id=?1", [session_id])
        .unwrap();
    e.pty(session_id).expect("spawned session has a PTY").set_idle(true);

    // The store is held, as a git scan or `adb devices` would hold it, while the keystroke
    // arrives: it must still be answered, and only the idle edge waits for the lock.
    with_store_held(e, || {
        // The control: an op that does read the store queues behind the holder. If it did
        // not, this test could not tell a keystroke that skips the store from one that waits.
        assert!(
            answer_within(e, "session.get", json!({"session": &name}), Duration::from_millis(300)).is_none(),
            "session.get answered with the store held, so the hold below proves nothing",
        );
        let answer = answer_within(e, "session.input", json!({"session": &name, "data": "wake\n"}), Duration::from_secs(5))
            .expect("session.input waited on the store lock (D148)");
        assert!(answer.ok, "{:?}", answer.error);
    });
    wait_until("idle session back to running", || state() == "running");

    // Every keystroke after the edge is pure memory: no further writes, and the echo still lands.
    ok(e, "session.resize", json!({"session": &name, "cols": 120, "rows": 40}));
    ok(e, "session.input", json!({"session": &name, "data": "again\n"}));
    wait_until("echo reaches the scrollback", || {
        ok(e, "session.scrollback", json!({"session": &name}))["text"]
            .as_str()
            .unwrap()
            .contains("echo:again")
    });
    assert_eq!(state(), "running");
    ok(e, "session.close", json!({"session": &name}));
}

/// A phone fits an agent's terminal to its own screen with `until_detach`. The size goes back
/// when it detaches or its connection goes, and never over a size someone set since.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_borrowed_terminal_size_goes_back() {
    let f = fixture();
    let e = f.engine.clone();
    let prov = fake_provider(&f.root, false);
    ok(&e, "settings.set", json!({"path": "providers.claude.path", "value": prov}));
    let server = SocketServer::start_in(e.clone(), f.root.join("run")).await.unwrap();
    let s = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    ok(&e, "session.spawn", json!({"session": &name}));
    let size = || {
        let sb = ok(&e, "session.scrollback", json!({"session": &name}));
        (sb["cols"].as_u64().unwrap(), sb["rows"].as_u64().unwrap())
    };
    let user = |op: &str, payload: Value| Request::new(Actor::User, op, payload);
    let fit = |cols: u16, rows: u16| user("session.resize", json!({"session": &name, "cols": cols, "rows": rows, "until_detach": true}));
    let set = |cols: u16, rows: u16| user("session.resize", json!({"session": &name, "cols": cols, "rows": rows}));
    let detach = || user("session.detach", json!({"session": &name}));
    async fn sent(c: &mut Client, req: Request) {
        let r = c.call(&req, |_| {}).await.unwrap();
        assert!(r.ok, "{} failed: {:?}", req.op, r.error);
    }
    assert_eq!(size(), (120, 40));

    // Detaching hands it back, to the size from before the first fit, not the last.
    let mut phone = Client::connect(&server.path).await.unwrap();
    sent(&mut phone, user("session.attach", json!({"session": &name}))).await;
    sent(&mut phone, fit(52, 38)).await;
    assert_eq!(size(), (52, 38));
    sent(&mut phone, fit(96, 20)).await;
    assert_eq!(size(), (96, 20));
    sent(&mut phone, detach()).await;
    assert_eq!(size(), (120, 40));

    // So does the connection going away, however it goes.
    sent(&mut phone, fit(52, 38)).await;
    drop(phone);
    wait_until("a dropped phone hands the size back", || size() == (120, 40));

    // The desktop resizing since is the newer wish: the hand-back leaves it alone.
    let mut phone = Client::connect(&server.path).await.unwrap();
    let mut desk = Client::connect(&server.path).await.unwrap();
    sent(&mut phone, fit(52, 38)).await;
    sent(&mut desk, set(160, 48)).await;
    sent(&mut phone, detach()).await;
    assert_eq!(size(), (160, 48));

    // A size the phone sets for good ends the loan: nothing to hand back.
    sent(&mut phone, fit(52, 38)).await;
    sent(&mut phone, set(100, 30)).await;
    sent(&mut phone, detach()).await;
    assert_eq!(size(), (100, 30));

    ok(&e, "session.close", json!({"session": &name}));
    drop(server);
}

/// SPEC §16: spawn/kill N sessions, assert zero orphans — including the children's children.
#[test]
fn teardown_leaves_no_orphans() {
    let f = fixture();
    let e = &f.engine;
    let prov = fake_provider(&f.root, true);
    ok(e, "settings.set", json!({"path": "providers.claude.path", "value": prov}));
    let mut pids = Vec::new();
    let mut names = Vec::new();
    for _ in 0..5 {
        let s = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
        let name = s["name"].as_str().unwrap().to_string();
        let s = ok(e, "session.spawn", json!({"session": name}));
        let pid = s["pid"].as_i64().unwrap();
        let wt = PathBuf::from(s["worktree"].as_str().unwrap());
        let child = wait_for_pid(&wt.join("child.pid"));
        assert!(alive(pid) && alive(child));
        pids.push((pid, child));
        names.push(name);
    }
    assert_eq!(e.live_pty_count(), 5);
    // close three through the bus, then shut the engine down for the rest; the fake provider
    // leaves its child.pid in each checkout, an untracked file (RA-405)
    for name in &names[..3] {
        ok(e, "session.close", json!({"session": name, "discard_changes": true}));
    }
    for (pid, child) in &pids[..3] {
        wait_until("closed session's processes gone", || !alive(*pid) && !alive(*child));
    }
    assert_eq!(e.live_pty_count(), 2);
    e.shutdown();
    for (pid, child) in &pids[3..] {
        wait_until("shut-down session's processes gone", || !alive(*pid) && !alive(*child));
    }
    assert_eq!(e.live_pty_count(), 0);
    let store = e.store.path().display().to_string();
    assert!(relay_core::pty::relay_children("test", &store).is_empty(), "orphans left behind");
    // the two shut-down sessions are restorable, the closed three closed
    let l = ok(e, "session.list", json!({"include_closed": true}));
    let mut states: Vec<String> = l["sessions"].as_array().unwrap().iter().map(|s| s["state"].as_str().unwrap().to_string()).collect();
    states.sort();
    assert_eq!(states, vec!["closed", "closed", "closed", "restorable", "restorable"]);
}

#[test]
fn recovery_reaps_orphans_and_fscks() {
    let f = fixture();
    let prov = fake_provider(&f.root, true);
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": prov}));
    let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    let s = ok(&f.engine, "session.spawn", json!({"session": name}));
    let pid = s["pid"].as_i64().unwrap();
    let wt = PathBuf::from(s["worktree"].as_str().unwrap());
    let child = wait_for_pid(&wt.join("child.pid"));
    // a session row that claims to be running with a dead pid, and a task stuck in active
    f.engine.store.lock().execute_batch(
        "INSERT INTO sessions(name, project_id, provider, role, branch, worktree, state, pid, token, epoch, created_at, updated_at)
         VALUES ('ghost-heron', 1, 'claude', 'builder', 'x', '/tmp', 'running', 999999, 't', 1, 'now', 'now');
         INSERT INTO tasks(project_id, title, col, created_at, updated_at) VALUES (1, 'stuck', 'active', 'now', 'now');").unwrap();
    // "crash": leak the PTY handle so the child outlives the engine, then drop the engine
    let leaked = f.engine.take_pty(1).unwrap();
    std::mem::forget(leaked);
    let store_path = f.engine.store.path().to_path_buf();
    drop(f.engine);
    assert!(alive(pid) && alive(child), "the orphan should have survived the engine");

    // a new engine on the same store
    let e2 = Engine::new(Instance::Test, Store::open(&store_path, false).unwrap());
    let report = relay_core::recovery::run(&e2).unwrap();
    assert!(report.reaped_pids.contains(&pid), "{report:?}");
    wait_until("orphan and its child gone", || !alive(pid) && !alive(child));
    assert!(report.fsck_fixes.iter().any(|x| x.contains(&name) && x.contains("restorable")), "{report:?}");
    assert!(report.fsck_fixes.iter().any(|x| x.contains("ghost-heron") && x.contains("dead")), "{report:?}");
    assert_eq!(report.tasks_reset_offered, vec![1]);
    let s = ok(&e2, "session.get", json!({"session": name}));
    assert_eq!(s["state"], "restorable");
    assert!(s["pid"].is_null());
    let last = ok(&e2, "app.recovery.last", json!({}));
    assert_eq!(last["reaped_pids"], json!(report.reaped_pids));
    // dirty pooled worktree gets flagged (child.pid is untracked)
    assert!(report.dirty_worktrees.iter().any(|d| d == &wt.display().to_string()), "{report:?}");
}

#[test]
fn recovery_leaves_alone_a_process_that_inherited_a_stale_pid() {
    let f = fixture();
    // An unrelated process that now holds the pid a crashed session recorded.
    let mut bystander = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = bystander.id() as i64;
    f.engine.store.lock().execute(
        "INSERT INTO sessions(name, project_id, provider, role, branch, worktree, state, pid, token, epoch, created_at, updated_at)
         VALUES ('stale-heron', 1, 'claude', 'builder', 'x', '/tmp', 'running', ?1, 't', 1, 'now', 'now')",
        [pid],
    ).unwrap();
    let report = relay_core::recovery::run(&f.engine).unwrap();
    assert!(alive(pid), "recovery killed a process that was not Relay's: {report:?}");
    assert!(!report.reaped_pids.contains(&pid), "{report:?}");
    assert!(report.fsck_fixes.iter().any(|x| x.contains("stale-heron") && x.contains("another process")), "{report:?}");
    let _ = bystander.kill();
    let _ = bystander.wait();
}

#[test]
fn recovery_releases_legacy_claims_but_preserves_new_work() {
    let f = fixture();
    let create = || ok(&f.engine, "session.create", json!({"project_id":1,"provider":"codex"}));
    let done = create();
    let closed = create();
    let working = create();
    let archived = create();
    f.engine.dispatch(Request::new(Actor::agent(done["name"].as_str().unwrap()),
        "session.done", json!({"summary":"Finished before upgrade"})), Door::InProcess).into_result().unwrap();
    ok(&f.engine, "session.close", json!({"session":closed["name"],"remove_worktree":false}));
    let task = ok(&f.engine,"task.create",json!({"project_id":1,"title":"Completed before audit retention"}));
    ok(&f.engine,"task.dispatch",json!({"task_id":task["id"],"session":archived["name"],"start":false}));
    f.engine.dispatch(Request::new(Actor::agent(archived["name"].as_str().unwrap()),
        "session.done", json!({"summary":"Archived completion"})), Door::InProcess).into_result().unwrap();
    {
        let conn = f.engine.store.lock();
        conn.execute("DELETE FROM audit WHERE op='session.done' AND session_id=?1", [archived["id"].as_i64().unwrap()]).unwrap();
        // Reconstruct rows the old build failed to delete, plus a renewed claim
        // and unfinished work whose claims must remain reserved.
        for (session, path, updated) in [
            (&done,"old.rs","2000-01-01T00:00:00Z"),
            (&done,"renewed.rs","2999-01-01T00:00:00Z"),
            (&closed,"closed.rs","2000-01-01T00:00:00Z"),
            (&working,"working.rs","2000-01-01T00:00:00Z"),
            (&archived,"archived.rs","2000-01-01T00:00:00Z"),
        ] {
            conn.execute("INSERT INTO claims(project_id,session_id,session,path,created_at,updated_at) VALUES(1,?1,?2,?3,'2000-01-01T00:00:00Z',?4)",
                rusqlite::params![session["id"].as_i64().unwrap(),session["name"].as_str().unwrap(),path,updated]).unwrap();
            conn.execute("INSERT INTO overlaps(project_id,fingerprint,sessions,path,kind,first_seen,last_seen) VALUES(1,?1,?2,?1,'claim','2000-01-01T00:00:00Z','2000-01-01T00:00:00Z')",
                rusqlite::params![path,json!([session["name"]]).to_string()]).unwrap();
        }
    }
    let report = relay_core::recovery::run(&f.engine).unwrap();
    assert!(report.fsck_fixes.iter().any(|fix| fix == "released 3 stale file claim(s)"), "{report:?}");
    {
        let conn = f.engine.store.lock();
        let remaining: Vec<String> = conn.prepare("SELECT path FROM claims ORDER BY path").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(remaining, ["renewed.rs", "working.rs"]);
        let overlaps: Vec<String> = conn.prepare("SELECT path FROM overlaps WHERE active=1 ORDER BY path").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(overlaps, remaining);
    }
    let again = relay_core::recovery::run(&f.engine).unwrap();
    assert!(!again.fsck_fixes.iter().any(|fix| fix.contains("stale file claim")));
}

#[test]
fn provider_discovery_is_explicit_cached_and_reports_version_changes() {
    let f = fixture();
    let claude = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    let codex = fake_discovery_provider(&f.root, "codex", "codex-cli 2.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": claude}));
    ok(&f.engine, "settings.set", json!({"path": "providers.codex.path", "value": codex}));

    let before = ok(&f.engine, "provider.list", json!({}));
    assert!(before["providers"].as_array().unwrap().iter().all(|provider| provider["installed"] == true && provider["version"].is_null()));
    let refreshed = ok(&f.engine, "provider.refresh", json!({}));
    let providers = refreshed["providers"].as_array().unwrap();
    assert_eq!(providers[0]["version"], "claude 1.0.0");
    assert_eq!(providers[0]["signed_in_as"], "agent@example.test");
    assert_eq!(providers[1]["version"], "codex-cli 2.0.0");
    assert_eq!(providers[1]["signed_in_as"], "ChatGPT");
    assert!(providers[0]["spawn_profile"]["fresh"].is_array());

    fake_discovery_provider(&f.root, "claude", "claude 1.1.0");
    let changed = ok(&f.engine, "provider.refresh", json!({}));
    let claude = &changed["providers"][0];
    assert_eq!(claude["version"], "claude 1.1.0");
    assert_eq!(claude["last_seen_version"], "claude 1.0.0");
    let notifications: i64 = f.engine.store.lock().query_row(
        "SELECT COUNT(*) FROM notifications WHERE category='provider' AND title='claude version changed'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(notifications, 1);
    assert_eq!(ok(&f.engine, "app.status", json!({}))["providers"][0]["version"], "claude 1.1.0");
}

#[test]
fn codex_launch_uses_reviewer_instructions_and_types_the_private_start_prompt() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "codex", "codex-cli 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.codex.path", "value": provider}));
    let created = ok(&f.engine, "session.create", json!({
        "project_id": 1, "provider": "codex", "role": "reviewer"
    }));
    let name = created["name"].as_str().unwrap().to_string();
    let running = ok(&f.engine, "session.spawn", json!({
        "session": name, "prompt": "review the private assignment"
    }));
    wait_until("provider greeting", || ok(&f.engine, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("hello-from-pty"));
    wait_until("provider start prompt", || ok(&f.engine, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("echo:Call session.bootstrap first, then begin this assignment: review the private assignment"));
    let worktree = PathBuf::from(running["worktree"].as_str().unwrap());
    let args = std::fs::read_to_string(worktree.join(".relay/provider-args.txt")).unwrap();
    assert!(args.contains("developer_instructions=\"You are this Relay session's reviewer."));
    assert!(args.contains("session.bootstrap"));
    assert!(args.contains("notify=[\""));
    assert!(args.contains("\",\"hook\",\"codex-notify\"]"));
    // F7: Codex gets no MCP config and files no lifecycle report, so the one channel it does
    // have has to carry the brief and the shell path to the bus…
    assert!(args.contains("$RELAY_BIN q"), "codex is never told how to reach the bus");
    assert!(args.contains("Live peers"), "codex never receives the peer table");
    // …and Relay still sees it working, because the PTY is the signal (D108).
    let observed = ok(&f.engine, "session.get", json!({"session": name}));
    assert!(
        observed["last_output_at"].is_string(),
        "an unhooked provider must not look like a session that has never done anything",
    );
    assert!(!args.contains("review the private assignment"));
    assert!(!args.contains("Start by reading"));
    let bootstrap = f.engine.dispatch(
        Request::new(Actor::agent(&name), "session.bootstrap", json!({})),
        Door::InProcess,
    ).into_result().unwrap();
    assert_eq!(bootstrap["assignment"], "review the private assignment");
    ok(&f.engine, "session.close", json!({"session": name}));
}

#[test]
fn skills_are_app_wide_folders_in_every_checkout() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    // A skill the repository checks in itself is the project's, not Relay's: same name, and
    // the checked-in copy still wins in the folder it occupies.
    let owned = f.repo.join(".claude/skills/impeccable");
    std::fs::create_dir_all(&owned).unwrap();
    std::fs::write(owned.join("SKILL.md"), "checked in").unwrap();
    git(&f.repo, &["add", "."]);
    git(&f.repo, &["commit", "-q", "-m", "vendor a skill"]);

    let body = "---\nname: Impeccable\n---\nDesign well.\n";
    let skill = ok(&f.engine, "skill.create", json!({"name": "Impeccable", "body": body}));
    // Installing once covers the project that already existed (D147).
    assert_eq!(skill["enabled_in"], json!([1]));

    // What `skill.install` stores beside the store: the folder, not just the SKILL.md body.
    let id = skill["id"].as_i64().unwrap();
    let library = f.root.join("store/skills").join(id.to_string());
    std::fs::create_dir_all(library.join("reference")).unwrap();
    std::fs::write(library.join("SKILL.md"), body).unwrap();
    std::fs::write(library.join("reference/polish.md"), "polish pass").unwrap();

    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = created["name"].as_str().unwrap().to_string();
    let running = ok(&f.engine, "session.spawn", json!({"session": name}));
    let worktree = PathBuf::from(running["worktree"].as_str().unwrap());
    let relay_owned = worktree.join(".agents/skills/impeccable");
    assert_eq!(
        std::fs::read_to_string(relay_owned.join("reference/polish.md")).unwrap(),
        "polish pass",
        "the reference documents a SKILL.md points at never travelled with it",
    );
    assert_eq!(
        std::fs::read_to_string(relay_owned.join("SKILL.md")).unwrap(),
        body,
        "the worktree never received the skill as a real provider folder",
    );
    assert!(relay_owned.join(relay_core::skills::MARKER).is_file(), "the folder is unmarked");
    assert_eq!(
        std::fs::read_to_string(worktree.join(".claude/skills/impeccable/SKILL.md")).unwrap(),
        "checked in",
        "Relay overwrote a skill the repository checks in",
    );
    let status = git(&worktree, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "materialized skills dirty the worktree: {status}");

    // Disabling removes the folders Relay owns again on the next launch.
    ok(&f.engine, "skill.enable", json!({"skill_id": id, "project_id": 1, "enabled": false}));
    ok(&f.engine, "session.park", json!({"session": name}));
    ok(&f.engine, "session.wake", json!({"session": name}));
    assert!(!relay_owned.exists(), "the worktree kept a skill nobody enabled");
    assert!(worktree.join(".claude/skills/impeccable/SKILL.md").is_file(), "pruning reached a checked-in skill");
    ok(&f.engine, "session.close", json!({"session": name}));
}

#[test]
fn a_plugin_that_is_on_reaches_every_agent_of_its_project() {
    let f = fixture();
    let claude = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    let codex = fake_discovery_provider(&f.root, "codex", "codex-cli 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": claude}));
    ok(&f.engine, "settings.set", json!({"path": "providers.codex.path", "value": codex}));
    std::fs::write(f.repo.join("Arena.uproject"), "{\"FileVersion\":3}").unwrap();
    git(&f.repo, &["add", "."]);
    git(&f.repo, &["commit", "-q", "-m", "an unreal project"]);

    // Off by default, and suggested because the root holds a .uproject.
    let listed = ok(&f.engine, "plugin.list", json!({}));
    let unreal = listed["plugins"].as_array().unwrap().iter().find(|p| p["id"] == "unreal-engine").unwrap();
    assert_eq!(unreal["enabled_in"], json!([]));
    assert_eq!(unreal["suggested_for"], json!([1]));
    assert!(unreal["skills"].as_array().unwrap().iter().any(|s| s["name"] == "unreal-fundamentals"));
    assert_eq!(unreal["mcp_servers"][0]["name"], "unreal");

    let off = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let off_name = off["name"].as_str().unwrap().to_string();
    let running = ok(&f.engine, "session.spawn", json!({"session": off_name}));
    let off_tree = PathBuf::from(running["worktree"].as_str().unwrap());
    assert!(!off_tree.join(".claude/skills/unreal-fundamentals").exists(), "a plugin that is off wrote skills");
    let mcp = std::fs::read_to_string(off_tree.join(".relay/relay.mcp.json")).unwrap();
    assert!(!mcp.contains("unreal-mcp"), "a plugin that is off registered its MCP server");

    let on = ok(&f.engine, "plugin.enable", json!({"plugin_id": "unreal-engine", "project_id": 1, "enabled": true}));
    assert_eq!(on["enabled_in"], json!([1]));
    assert_eq!(ok(&f.engine, "plugin.list", json!({}))["plugins"][0]["suggested_for"], json!([]));
    // The refresh runs after commit on its own thread; the running agent's checkout gets the
    // skill folders without a relaunch.
    // Each folder's marker is written last, and `.agents/` after `.claude/`, so wait on markers.
    let marker = relay_core::skills::MARKER;
    let skill = off_tree.join(".claude/skills/unreal-fundamentals");
    wait_until("plugin skills in a running worktree", || skill.join(marker).is_file());
    assert!(std::fs::read_to_string(skill.join("SKILL.md")).unwrap().starts_with("---\nname: unreal-fundamentals"));
    wait_until("plugin skills in .agents", || off_tree.join(".agents/skills/unreal-editor-automation").join(marker).is_file());
    wait_until("plugin skills in the project root", || f.repo.join(".claude/skills/unreal-cpp").join(marker).is_file());
    ok(&f.engine, "session.close", json!({"session": off_name}));

    // The editor has one project open, so the plugin starts new agents in the main checkout
    // (D160); an explicit worktree request still gets one.
    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = created["name"].as_str().unwrap().to_string();
    assert_eq!(PathBuf::from(created["worktree"].as_str().unwrap()), f.repo, "an Unreal agent did not start in the main checkout");
    let own = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude", "worktree": "new"}));
    assert_ne!(PathBuf::from(own["worktree"].as_str().unwrap()), f.repo);
    ok(&f.engine, "session.close", json!({"session": own["name"]}));
    let running = ok(&f.engine, "session.spawn", json!({"session": name}));
    let worktree = PathBuf::from(running["worktree"].as_str().unwrap());
    let mcp: Value = serde_json::from_str(&std::fs::read_to_string(worktree.join(".relay/relay.mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["unreal"]["args"], json!(["unreal-mcp"]));
    assert_eq!(mcp["mcpServers"]["unreal"]["command"], mcp["mcpServers"]["relay"]["command"], "`relay` is this binary");
    assert!(mcp["mcpServers"]["relay"]["args"].as_array().unwrap().iter().any(|a| a == "mcp"), "the bus server was replaced");
    let role = std::fs::read_to_string(worktree.join(format!(".relay/sessions/{name}/role-instructions.md"))).unwrap();
    assert!(role.contains("## Enabled plugins"), "the brief never names the plugin");
    assert!(role.contains("Start every task with `ue_project_info`"), "the plugin's rules are not in the injected brief");
    assert!(role.contains("- unreal-animation — "), "plugin skills are not listed with their descriptions");
    let status = git(&worktree, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "plugin skills dirty the worktree: {status}");
    ok(&f.engine, "session.close", json!({"session": name}));

    // Codex has no MCP config file; the server travels as --config overrides.
    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "codex"}));
    let codex_name = created["name"].as_str().unwrap().to_string();
    // It shares the main checkout with the Claude agent above; drop that agent's recorded args.
    let _ = std::fs::remove_file(f.repo.join(".relay/provider-args.txt"));
    let running = ok(&f.engine, "session.spawn", json!({"session": codex_name}));
    let codex_tree = PathBuf::from(running["worktree"].as_str().unwrap());
    wait_until("codex args", || codex_tree.join(".relay/provider-args.txt").is_file());
    let args = std::fs::read_to_string(codex_tree.join(".relay/provider-args.txt")).unwrap();
    assert!(args.contains("mcp_servers.unreal.args=[\"unreal-mcp\"]"), "{args}");
    assert!(args.contains("mcp_servers.unreal.command=\""));
    ok(&f.engine, "session.close", json!({"session": codex_name}));

    // Switching it off takes the folders out of the project root again; undo brings it back.
    ok(&f.engine, "plugin.enable", json!({"plugin_id": "unreal-engine", "project_id": 1, "enabled": false}));
    wait_until("plugin skills pruned", || !f.repo.join(".claude/skills/unreal-cpp").exists());
    let audit = ok(&f.engine, "audit.list", json!({"op_prefix": "plugin.enable", "limit": 1}));
    ok(&f.engine, "audit.undo", json!({"audit_id": audit["rows"][0]["id"]}));
    assert_eq!(ok(&f.engine, "plugin.get", json!({"plugin_id": "unreal-engine"}))["plugin"]["enabled_in"], json!([1]));

    let detail = ok(&f.engine, "plugin.get", json!({"plugin_id": "unreal-engine", "skill": "unreal-gas"}));
    assert!(detail["instructions"].as_str().unwrap().contains("ue_project_info"));
    assert!(detail["docs"].as_array().unwrap().iter().any(|d| d["path"] == "docs/setup.md"));
    assert!(detail["skill"]["body"].as_str().unwrap().contains("name: unreal-gas"));
    assert_eq!(code(call(&f.engine, "plugin.get", json!({"plugin_id": "nope"}))), "plugin.not_found");
    assert_eq!(code(call(&f.engine, "plugin.enable", json!({"plugin_id": "unreal-engine", "project_id": 9, "enabled": true}))), "project.not_found");
}

#[test]
fn multi_task_launch_stages_the_queue_and_prompts_each_current_task() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let first = ok(&f.engine, "task.create", json!({"project_id":1,"title":"First queued task"}));
    let second = ok(&f.engine, "task.create", json!({"project_id":1,"title":"Second current task"}));
    let created = ok(&f.engine, "session.create", json!({
        "project_id":1,"provider":"claude","role":"builder"
    }));
    let name = created["name"].as_str().unwrap().to_string();

    let staged_first = ok(&f.engine, "task.dispatch", json!({
        "task_id":first["id"],"session":name,"start":false
    }));
    let staged_second = ok(&f.engine, "task.dispatch", json!({
        "task_id":second["id"],"session":name,"start":false
    }));
    assert_eq!(staged_first["session"]["state"], "created");
    assert_eq!(staged_second["session"]["state"], "created");
    assert_eq!(staged_second["session"]["task_id"], first["id"]);

    ok(&f.engine, "session.spawn", json!({"session":name}));
    wait_until("current-task start prompt", || {
        ok(&f.engine, "session.scrollback", json!({"session":name}))["text"]
            .as_str().unwrap().contains(&format!("echo:Call session.bootstrap first, then begin current Task #{}", first["id"]))
    });
    let brief = ok(&f.engine, "session.brief", json!({"session":name}));
    assert!(brief["text"].as_str().unwrap().contains("assigned_tasks: 2"));
    assert!(brief["text"].as_str().unwrap().contains(&format!("Task #{} [CURRENT]", first["id"])));
    assert!(brief["text"].as_str().unwrap().contains(&format!("Task #{} [QUEUED]", second["id"])));

    let advanced = f.engine.dispatch(
        Request::new(Actor::agent(&name), "session.done", json!({
            "session":name,"status":"completed","summary":"current complete"
        })),
        Door::InProcess,
    ).into_result().unwrap();
    assert_eq!(advanced["task_id"], second["id"]);
    assert_eq!(ok(&f.engine, "task.get", json!({"task_id":first["id"]}))["column"], "in_review");
    assert_eq!(ok(&f.engine, "task.get", json!({"task_id":second["id"]}))["column"], "active");
    let next_prompt = format!("echo:Your current assignment is Task #{}.", second["id"]);
    let scrollback = || ok(&f.engine, "session.scrollback", json!({"session":name}))["text"].as_str().unwrap().to_string();
    // An injected prompt shows only once the provider has read and echoed it, so a check made
    // straight after Done proves nothing. A line typed now queues behind anything Done wrote
    // to the PTY: once its echo is back, an injection would be too (RA-677).
    let fence = |line: &str| {
        ok(&f.engine, "session.input", json!({"session":name,"data":format!("{line}\n")}));
        wait_until("the fence line's echo", || scrollback().contains(&format!("echo:{line}")));
    };
    fence("fence-after-done");
    assert!(!scrollback().contains(&next_prompt), "Done must not inject into its own unfinished provider turn");
    // Providers can emit another tool event while finishing the Done turn. It must
    // not clear the marker or make that turn's Stop complete the next assignment.
    ok(&f.engine, "session.report", json!({"session":name,"kind":"tool_use"}));
    ok(&f.engine, "session.report", json!({"session":name,"kind":"stop"}));
    assert_eq!(f.engine.store.lock().query_row(
        "SELECT COUNT(*) FROM notifications WHERE category='agent_done'", [], |row| row.get::<_, i64>(0),
    ).unwrap(), 1, "the trailing Stop belongs to the first task");
    assert_eq!(ok(&f.engine, "session.get", json!({"session":name}))["state"], "running");
    wait_until("next-task prompt", || scrollback().contains(&next_prompt));
    fence("fence-after-stop");
    assert_eq!(scrollback().matches(&next_prompt).count(), 1, "the next assignment is typed exactly once");
    let mail_count: i64 = f.engine.store.lock().query_row(
        "SELECT COUNT(*) FROM messages WHERE re_task=?1 AND text LIKE 'Your current assignment is Task #%';", [second["id"].as_i64().unwrap()], |row| row.get(0),
    ).unwrap();
    assert_eq!(mail_count, 1, "deferred delivery must not duplicate assignment mail");
    ok(&f.engine, "session.report", json!({"session":name,"kind":"tool_use"}));
    ok(&f.engine, "session.report", json!({"session":name,"kind":"stop","data":{"message":"Second work finished"}}));
    let (count, body): (i64, String) = f.engine.store.lock().query_row(
        "SELECT COUNT(*),MAX(body) FROM notifications WHERE category='agent_done' AND json_extract(link,'$.payload.task_id')=?1", [second["id"].as_i64().unwrap()], |row| Ok((row.get(0)?,row.get(1)?)),
    ).unwrap();
    assert_eq!((count,body),(1,"Second work finished".into()));
    ok(&f.engine, "session.close", json!({"session":name}));
}

#[test]
fn unassigned_done_stop_marker_is_consumed_once_and_reset_on_session_start() {
    let f = fixture();
    let created = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude"}));
    let name = created["name"].as_str().unwrap();
    let report = |kind: &str| ok(&f.engine, "session.report", json!({"session":name,"kind":kind}));
    let complete = || f.engine.dispatch(Request::new(Actor::agent(name), "session.done", json!({"session":name,"summary":"Finished"})), Door::InProcess).into_result().unwrap();
    let unread = || ok(&f.engine,"notify.list",json!({"unread_only":true}))["notifications"].as_array().unwrap().clone();
    let ack = || { for notification in unread() { ok(&f.engine,"notify.ack",json!({"notification_id":notification["id"]})); } };
    report("session_start");
    complete();
    ack();
    report("tool_use");
    report("stop");
    assert!(unread().is_empty(), "acknowledged Done must not be resent by its trailing Stop");
    report("tool_use");
    report("stop");
    assert_eq!(unread().len(),1,"marker must be consumed, allowing a later real turn completion");
    ack();
    complete();
    ack();
    report("session_start");
    report("stop");
    assert_eq!(unread().len(),1,"a fresh provider session must not inherit an old pending Stop");
    ok(&f.engine,"session.close",json!({"session":name}));
}

#[test]
fn phase_six_park_wake_restore_and_immutable_launch_options() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let created = ok(&f.engine, "session.create", json!({
        "project_id": 1, "provider": "claude", "model": "model-x", "effort": "high"
    }));
    let name = created["name"].as_str().unwrap().to_string();
    let updated = ok(&f.engine, "session.update", json!({"session": name, "branch": "relay/custom", "allow_ui": true}));
    assert_eq!(updated["branch"], "relay/custom");
    assert_eq!(updated["allow_ui"], true);
    assert_eq!(git(Path::new(updated["worktree"].as_str().unwrap()), &["branch", "--show-current"]), "relay/custom");

    let running = ok(&f.engine, "session.spawn", json!({"session": name, "prompt": "start here"}));
    let first_pid = running["pid"].as_i64().unwrap();
    wait_until("provider greeting", || ok(&f.engine, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("hello-from-pty"));
    wait_until("provider start prompt", || ok(&f.engine, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("echo:Call session.bootstrap first, then begin this assignment: start here"));
    let worktree = PathBuf::from(running["worktree"].as_str().unwrap());
    let brief_path = format!(".relay/sessions/{name}/session-brief.md");
    let role_path = format!(".relay/sessions/{name}/role-instructions.md");
    let brief = std::fs::read_to_string(worktree.join(&brief_path)).unwrap();
    assert!(brief.contains("## Current state"));
    assert!(brief.contains(&format!("session: {name}")));
    assert!(brief.contains("Launch assignment:\nstart here"));
    let role = std::fs::read_to_string(worktree.join(&role_path)).unwrap();
    assert!(role.contains("Relay session's builder"));
    assert!(role.contains("session.bootstrap"));
    assert!(role.contains("query task.list"));
    // F1/F9: the injected prompt carries the brief and names the cross-provider channel, and
    // the skill bodies stay in their own file rather than in every system prompt.
    assert!(role.contains("## Live peers"), "the brief is written but never delivered");
    assert!(role.contains("mailbox.send"), "a builder is never told the mailbox exists");
    assert!(role.contains("$RELAY_BRIEF"));
    assert!(!role.contains("start here"), "the launch assignment stays out of the prompt file");
    assert!(worktree.join(".relay/session-skills.md").is_file(), "skills are split out");
    let provider_args = std::fs::read_to_string(worktree.join(".relay/provider-args.txt")).unwrap();
    assert!(provider_args.contains(&format!("--append-system-prompt-file\n{role_path}")));
    assert!(!provider_args.contains("Start by reading"));
    assert!(!provider_args.contains("start here"));
    assert!(!provider_args.contains("## Current state"));
    let bootstrap = f.engine.dispatch(
        Request::new(Actor::agent(&name), "session.bootstrap", json!({})),
        Door::InProcess,
    ).into_result().unwrap();
    assert_eq!(bootstrap["session"], name);
    assert_eq!(bootstrap["role"], "builder");
    assert_eq!(bootstrap["project_id"], 1);
    assert_eq!(bootstrap["assignment"], "start here");
    assert_eq!(bootstrap["brief_path"], brief_path);
    assert!(bootstrap["task"].is_null());
    assert_eq!(code(call(&f.engine, "session.bootstrap", json!({}))), "bus.actor");
    assert_eq!(code(call(&f.engine, "session.update", json!({"session": name, "model": "other"}))), "session.already_spawned");

    let parked = ok(&f.engine, "session.park", json!({"session": name}));
    assert_eq!(parked["state"], "parked");
    wait_until("parked process gone", || !alive(first_pid));
    let saved = ok(&f.engine, "session.scrollback", json!({"session": name}));
    assert!(saved["text"].as_str().unwrap().contains("hello-from-pty"));

    let woken = ok(&f.engine, "session.wake", json!({"session": name}));
    assert_eq!(woken["state"], "running");
    assert_ne!(woken["pid"].as_i64().unwrap(), first_pid);
    assert!(ok(&f.engine, "session.scrollback", json!({"session": name}))["text"].as_str().unwrap().contains("hello-from-pty"));

    f.engine.shutdown();
    let recoverable = ok(&f.engine, "session.restorable", json!({}));
    assert_eq!(recoverable["sessions"][0]["session"]["name"], name);
    assert_eq!(recoverable["sessions"][0]["reason"], "app_restart");
    let resumed = ok(&f.engine, "session.resume", json!({"session": name}));
    assert_eq!(resumed["state"], "running");
    ok(&f.engine, "session.close", json!({"session": name}));
}

#[test]
fn clear_restorable_fresh_starts_the_same_terminal_without_saved_context() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let created = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = created["name"].as_str().unwrap().to_string();
    let id = created["id"].as_i64().unwrap();
    let worktree = created["worktree"].clone();
    let branch = created["branch"].clone();

    ok(&f.engine, "session.spawn", json!({"session": name, "prompt": "keep this assignment"}));
    let bootstrap = |name: &str| f.engine.dispatch(
        Request::new(Actor::agent(name), "session.bootstrap", json!({})),
        Door::InProcess,
    ).into_result().unwrap();
    assert_eq!(bootstrap(&name)["assignment"], "keep this assignment", "the first launch's work to begin");
    ok(&f.engine, "session.input", json!({"session": name, "data": "old-context-marker\n"}));
    wait_until("old context in scrollback", || {
        ok(&f.engine, "session.scrollback", json!({"session": name}))["text"]
            .as_str().unwrap().contains("echo:old-context-marker")
    });
    f.engine.store.lock().execute(
        "UPDATE sessions SET provider_ref='saved-provider-context' WHERE id=?1",
        [id],
    ).unwrap();
    f.engine.shutdown();
    assert_eq!(ok(&f.engine, "session.get", json!({"session": name}))["state"], "restorable");

    let cleared = ok(&f.engine, "session.clear_restorable", json!({"session": name}));
    assert_eq!(cleared["state"], "running");
    assert_eq!(cleared["id"], id);
    assert_eq!(cleared["worktree"], worktree);
    assert_eq!(cleared["branch"], branch);
    assert!(cleared["provider_ref"].is_null());
    let args = std::fs::read_to_string(
        PathBuf::from(cleared["worktree"].as_str().unwrap()).join(".relay/provider-args.txt"),
    ).unwrap();
    assert!(!args.contains("--resume"), "Clear resumed the old provider context: {args}");
    assert!(!args.contains("saved-provider-context"), "Clear passed the old provider handle: {args}");
    let scrollback = ok(&f.engine, "session.scrollback", json!({"session": name}));
    assert!(!scrollback["text"].as_str().unwrap().contains("old-context-marker"));
    // RA-397: a fresh start long after the first launch is not handed that launch's work again;
    // the brief keeps it, named for what it now is.
    assert!(bootstrap(&name)["assignment"].is_null(), "a relaunch was handed the first launch's assignment");
    let brief = ok(&f.engine, "session.brief", json!({"session": name}));
    assert!(brief["text"].as_str().unwrap().contains("Original launch assignment (from the first launch; may already be done):\nkeep this assignment"), "{brief}");
    assert_eq!(
        code(call(&f.engine, "session.clear_restorable", json!({"session": name}))),
        "session.state",
    );
    ok(&f.engine, "session.close", json!({"session": name}));
}

#[test]
fn allocated_assignments_survive_partial_launch_and_omitted_spawn_prompt() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(
        &f.engine,
        "settings.set",
        json!({"path":"providers.claude.path","value":provider}),
    );
    for (override_prompt, expected) in [
        (None, Some("preserved assignment")),
        (
            Some("replacement assignment"),
            Some("replacement assignment"),
        ),
        (Some(""), None),
    ] {
        let session = ok(
            &f.engine,
            "session.create",
            json!({"project_id":1,"provider":"claude","prompt":"preserved assignment"}),
        );
        let id = session["id"].as_i64().unwrap();
        let stored = || {
            f.engine
                .store
                .lock()
                .query_row(
                    "SELECT launch_prompt FROM sessions WHERE id=?1",
                    [id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap()
        };
        assert_eq!(stored().as_deref(), Some("preserved assignment"));
        // This session's own launch can fail without losing its allocated assignment (RA-677).
        ok(
            &f.engine,
            "settings.set",
            json!({"path":"providers.claude.path","value":f.root.join("missing-claude")}),
        );
        assert_eq!(
            code(call(&f.engine, "session.spawn", json!({"session":session["name"]}))),
            "provider.not_installed"
        );
        assert_eq!(stored().as_deref(), Some("preserved assignment"));
        ok(
            &f.engine,
            "settings.set",
            json!({"path":"providers.claude.path","value":provider}),
        );
        let mut spawn = json!({"session":session["name"]});
        if let Some(prompt) = override_prompt {
            spawn["prompt"] = json!(prompt);
        }
        ok(&f.engine, "session.spawn", spawn);
        assert_eq!(stored().as_deref(), expected);
        if let Some(prompt) = expected {
            wait_until("allocated assignment reaches fake provider", || {
                ok(
                    &f.engine,
                    "session.scrollback",
                    json!({"session":session["name"]}),
                )["text"]
                    .as_str()
                    .unwrap()
                    .contains(&format!(
                        "echo:Call session.bootstrap first, then begin this assignment: {prompt}"
                    ))
            });
        }
        ok(
            &f.engine,
            "session.close",
            json!({"session":session["name"]}),
        );
    }
}

#[test]
fn pair_sessions_share_checkout_and_teardown_safely() {
    let f = fixture();
    let provider = fake_discovery_provider(&f.root, "claude", "claude 1.0.0");
    ok(&f.engine, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let builder = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}));
    let builder_name = builder["name"].as_str().unwrap().to_string();
    let reviewer = ok(&f.engine, "session.create", json!({
        "project_id": 1, "provider": "claude", "role": "reviewer", "pair_with": builder_name
    }));
    let reviewer_name = reviewer["name"].as_str().unwrap().to_string();
    assert_eq!(builder["worktree"], reviewer["worktree"]);
    assert_eq!(builder["branch"], reviewer["branch"]);
    assert_eq!(ok(&f.engine, "session.get", json!({"session": builder_name}))["pair_with"], reviewer_name);
    let second_builder = ok(&f.engine, "session.create", json!({
        "project_id": 1, "provider": "claude", "role": "builder", "pair_with": reviewer_name
    }));
    let second_builder_name = second_builder["name"].as_str().unwrap().to_string();
    assert_eq!(builder["worktree"], second_builder["worktree"]);
    assert_eq!(second_builder["pair_with"], reviewer_name);
    assert_eq!(ok(&f.engine, "session.get", json!({"session": reviewer_name}))["pair_with"], builder_name);
    assert_eq!(code(call(&f.engine, "session.create", json!({
        "project_id": 1, "provider": "claude", "role": "builder", "pair_with": reviewer_name
    }))), "session.review_group_full");
    let hook_dir = git(Path::new(reviewer["worktree"].as_str().unwrap()), &["config", "--worktree", "--get", "core.hooksPath"]);
    let hook = PathBuf::from(hook_dir).join("pre-commit");
    assert!(std::fs::read_to_string(hook).unwrap().contains("$RELAY_SESSION"));

    let task = ok(&f.engine, "task.create", json!({"project_id": 1, "title": "Review together"}));
    ok(&f.engine, "task.dispatch", json!({"task_id": task["id"], "session": reviewer_name}));
    assert_eq!(ok(&f.engine, "session.get", json!({"session": builder_name}))["task_id"], task["id"]);
    assert_eq!(ok(&f.engine, "session.get", json!({"session": second_builder_name}))["task_id"], task["id"]);
    let worktree = PathBuf::from(builder["worktree"].as_str().unwrap());
    let reviewer_role = worktree.join(format!(".relay/sessions/{reviewer_name}/role-instructions.md"));
    let reviewer_brief = worktree.join(format!(".relay/sessions/{reviewer_name}/session-brief.md"));
    assert!(std::fs::read_to_string(&reviewer_role).unwrap().contains("Relay session's reviewer"));
    assert!(std::fs::read_to_string(&reviewer_brief).unwrap().contains(&format!("session: {reviewer_name}")));
    ok(&f.engine, "session.spawn", json!({"session": builder_name}));
    let builder_role = worktree.join(format!(".relay/sessions/{builder_name}/role-instructions.md"));
    let builder_brief = worktree.join(format!(".relay/sessions/{builder_name}/session-brief.md"));
    assert!(std::fs::read_to_string(&builder_role).unwrap().contains("Relay session's builder"));
    assert!(std::fs::read_to_string(&builder_brief).unwrap().contains(&format!("session: {builder_name}")));
    assert!(std::fs::read_to_string(&reviewer_role).unwrap().contains("Relay session's reviewer"),
        "the paired builder overwrote the reviewer's role instructions");
    assert!(std::fs::read_to_string(&reviewer_brief).unwrap().contains(&format!("session: {reviewer_name}")),
        "the paired builder overwrote the reviewer's brief");
    ok(&f.engine, "session.spawn", json!({"session": second_builder_name}));
    let bootstrap = f.engine.dispatch(
        Request::new(Actor::agent(&reviewer_name), "session.bootstrap", json!({})),
        Door::InProcess,
    ).into_result().unwrap();
    assert_eq!(bootstrap["role"], "reviewer");
    assert_eq!(bootstrap["pair"]["session"], builder_name);
    assert_eq!(bootstrap["pair"]["branch"], reviewer["branch"]);
    assert_eq!(code(call(&f.engine, "session.close", json!({"session": builder_name}))), "session.pair_live");
    ok(&f.engine, "session.close", json!({"session": builder_name, "remove_worktree": false}));
    let worktree = PathBuf::from(reviewer["worktree"].as_str().unwrap());
    assert!(worktree.exists());
    // The reviewer keeps its second builder rather than leaving the group half-linked (RA-398).
    assert_eq!(ok(&f.engine, "session.get", json!({"session": reviewer_name}))["pair_with"], second_builder_name);
    assert_eq!(code(call(&f.engine, "session.close", json!({"session": reviewer_name}))), "session.pair_live");
    ok(&f.engine, "session.close", json!({"session": reviewer_name, "remove_worktree": false}));
    assert!(ok(&f.engine, "session.get", json!({"session": second_builder_name}))["pair_with"].is_null());
    ok(&f.engine, "session.close", json!({"session": second_builder_name}));
    assert!(!worktree.exists());
}

#[test]
fn review_group_keeps_shared_worktree_until_every_participant_finishes_then_advances_fifo() {
    let f = fixture();
    let first = ok(&f.engine,"task.create",json!({"project_id":1,"title":"First group task"}));
    let second = ok(&f.engine,"task.create",json!({"project_id":1,"title":"Second group task"}));
    let b1 = ok(&f.engine,"session.create",json!({"project_id":1,"provider":"codex","role":"builder"}));
    let reviewer = ok(&f.engine,"session.create",json!({"project_id":1,"provider":"codex","role":"reviewer","pair_with":b1["name"]}));
    let b2 = ok(&f.engine,"session.create",json!({"project_id":1,"provider":"codex","role":"builder","pair_with":reviewer["name"]}));
    let names: Vec<_> = [&b1,&b2,&reviewer].into_iter().map(|s|s["name"].as_str().unwrap().to_string()).collect();
    for task in [&first,&second] { ok(&f.engine,"task.dispatch",json!({"task_id":task["id"],"session":names[0],"start":false})); }
    let current = |name: &str| ok(&f.engine,"session.get",json!({"session":name}))["task_id"].clone();
    let done = |name: &str,status: &str| f.engine.dispatch(Request::new(Actor::agent(name),"session.done",json!({"session":name,"status":status,"summary":"fixture report","blockers":if status == "completed" {vec![]} else {vec!["fixture blocker"]}})),Door::InProcess);
    for name in &names { assert_eq!(current(name),first["id"],"staging must preserve FIFO current"); }
    assert_eq!(code(done(&names[2],"completed")),"session.review_not_ready");
    done(&names[0],"completed").into_result().unwrap();
    // A repeated report before the barrier cannot count as the other builder's work.
    done(&names[0],"completed").into_result().unwrap();
    assert_eq!(ok(&f.engine,"task.get",json!({"task_id":first["id"]}))["column"],"active");
    for name in &names { assert_eq!(current(name),first["id"]); }
    done(&names[1],"partial").into_result().unwrap();
    assert_eq!(ok(&f.engine,"task.get",json!({"task_id":first["id"]}))["state"],"blocked");
    done(&names[1],"completed").into_result().unwrap();
    assert_eq!(ok(&f.engine,"task.get",json!({"task_id":first["id"]}))["column"],"in_review");
    for name in &names { assert_eq!(current(name),first["id"],"review must retain a stable shared worktree"); }
    let mail = ok(&f.engine,"mailbox.list",json!({"project_id":1,"session":names[2]}));
    assert_eq!(mail["messages"].as_array().unwrap().iter().filter(|m|m["re_task"] == first["id"] && m["text"].as_str().unwrap().starts_with("All builders finished")).count(),1);
    done(&names[2],"blocked").into_result().unwrap();
    for name in &names { assert_eq!(current(name),first["id"]); }
    done(&names[2],"completed").into_result().unwrap();
    for name in &names {
        assert_eq!(current(name),second["id"]);
        let mail = ok(&f.engine,"mailbox.list",json!({"project_id":1,"session":name}));
        assert_eq!(mail["messages"].as_array().unwrap().iter().filter(|m|m["re_task"] == second["id"] && m["text"].as_str().unwrap().starts_with("Your current assignment")).count(),1,"each participant must get a durable next assignment");
    }
    // A new taskless Done after this point names the new current task by contract.
    // Only reusing the original req_id is replay-idempotent across an advancement.
}

#[test]
fn slow_checkout_does_not_hold_the_store_lock() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let marker = f.root.join("checkout-started");
    let finished = f.root.join("checkout-finished");
    let filter = f.root.join("slow-filter");
    std::fs::write(&filter, format!(
        "#!/bin/sh\ntouch '{}'\nsleep 2\ntouch '{}'\ncat\n", marker.display(), finished.display(),
    )).unwrap();
    std::fs::set_permissions(&filter, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(f.repo.join(".gitattributes"), "README.md filter=slow\n").unwrap();
    git(&f.repo, &["config", "filter.slow.smudge", filter.to_str().unwrap()]);
    git(&f.repo, &["config", "filter.slow.clean", "cat"]);
    git(&f.repo, &["config", "filter.slow.required", "true"]);
    git(&f.repo, &["add", ".gitattributes"]);
    git(&f.repo, &["commit", "-qm", "slow checkout fixture"]);
    let engine = f.engine.clone();
    let creating = std::thread::spawn(move || ok(&engine, "session.create",
        json!({"project_id":1, "provider":"codex", "worktree":"new"})));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "Git did not start the slow checkout filter");
    // Answered while the filter still sleeps: no wall-clock bound to race (RA-742).
    ok(&f.engine, "app.status", json!({}));
    let answered_during_checkout = !finished.exists();
    let session = creating.join().unwrap();
    assert_eq!(session["state"], "created");
    assert_eq!(std::fs::read_to_string(Path::new(session["worktree"].as_str().unwrap()).join("README.md")).unwrap(), "hi\n");
    assert!(answered_during_checkout, "app.status blocked behind the checkout");
}

/// Opening and closing one agent must never stall every other pane: a park or a close that
/// removes the checkout waits out the child's TERM grace with the store unlocked, and the
/// silenced exit callback leaves the row at the state the request wrote — never `exited`,
/// and never a builder's task marked failed on the way.
#[test]
fn park_and_close_stop_the_child_without_holding_the_store() {
    let f = fixture();
    let provider = fake_provider(&f.root, false);
    std::fs::write(&provider, "#!/bin/sh\ntrap '' TERM\necho ready\nwhile IFS= read -r line; do :; done\n").unwrap();
    ok(&f.engine, "settings.set", json!({"path":"providers.claude.path","value":provider}));
    let task = ok(&f.engine, "task.create", json!({"project_id":1,"title":"Survives a close"}));
    for op in ["session.park", "session.close"] {
        let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude"}));
        let name = session["name"].as_str().unwrap().to_string();
        ok(&f.engine, "task.dispatch", json!({"task_id":task["id"],"session":name,"start":false}));
        let spawned = ok(&f.engine, "session.spawn", json!({"session":name}));
        wait_until("ignoring TERM", || ok(&f.engine,"session.scrollback",json!({"session":name}))["text"].as_str().unwrap().contains("ready"));
        let engine = f.engine.clone();
        let payload = if op == "session.close" { json!({"session":name,"remove_worktree":true}) } else { json!({"session":name}) };
        let (id, pid) = (session["id"].as_i64().unwrap(), spawned["pid"].as_i64().unwrap());
        let stopping = std::thread::spawn(move || ok(&engine, op, payload));
        // Synchronized, not slept for (RA-742): once the PTY is out of the registry and the
        // TERM-ignoring child is still alive, the request is inside its grace (3 s to park,
        // CLOSE_GRACE to close). A store held through it would keep session.list waiting until
        // the child had been killed, so the child must still be alive when the list answers.
        let deadline = Instant::now() + Duration::from_secs(10);
        while f.engine.pty(id).is_some() {
            assert!(Instant::now() < deadline, "{op} never took the PTY");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(alive(pid), "{op}'s grace ran out before the probe could start");
        ok(&f.engine, "session.list", json!({}));
        assert!(alive(pid), "session.list waited behind {op}'s kill");
        stopping.join().unwrap();
        assert!(!alive(pid), "{op} returned before its child died");
        let row = ok(&f.engine, "session.list", json!({"include_closed":true}))["sessions"]
            .as_array().unwrap().iter().find(|s| s["name"] == name.as_str()).cloned().unwrap();
        let want = if op == "session.park" { "parked" } else { "closed" };
        assert_eq!(row["state"], want, "{op}");
        // Nothing lands after the response either.
        std::thread::sleep(Duration::from_millis(100));
        let row = ok(&f.engine, "session.list", json!({"include_closed":true}))["sessions"]
            .as_array().unwrap().iter().find(|s| s["name"] == name.as_str()).cloned().unwrap();
        assert_eq!(row["state"], want, "{op} was overwritten by the exit callback");
        assert_ne!(ok(&f.engine, "task.get", json!({"task_id":task["id"]}))["state"], "failed", "{op}");
        if op == "session.park" {
            assert!(ok(&f.engine, "session.scrollback", json!({"session":name}))["text"].as_str().unwrap().contains("ready"));
            ok(&f.engine, "session.close", json!({"session":name,"remove_worktree":true}));
        } else {
            assert!(!Path::new(session["worktree"].as_str().unwrap()).exists());
        }
    }
}

/// Every session that ever ran in one checkout is torn down with one read of its hook path,
/// and the path that was there before Relay comes back, not a dead session's hook directory.
#[test]
fn closing_the_last_of_several_sessions_restores_the_original_hook_path() {
    let f = fixture();
    let hooks = |repo: &Path| {
        let out = git_command(repo).args(["config", "--worktree", "--get", "core.hooksPath"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(&f.repo, &["config", "extensions.worktreeConfig", "true"]);
    git(&f.repo, &["config", "--worktree", "core.hooksPath", "user-hooks"]);
    let mut names = Vec::new();
    for _ in 0..3 {
        let s = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
        names.push(s["name"].as_str().unwrap().to_string());
        assert!(hooks(&f.repo).ends_with(&format!(".relay/hooks/{}", names.last().unwrap())));
    }
    // Closed oldest first, so the newest session's hook stays active until the very last close.
    for name in &names {
        ok(&f.engine, "session.close", json!({"session":name,"remove_worktree":false}));
    }
    assert_eq!(hooks(&f.repo), "user-hooks");
    for name in &names {
        assert!(!f.repo.join(".relay/hooks").join(name).exists(), "{name}'s hook directory survived");
    }
}

/// Closing the session whose hook directory is the checkout's active hook path, while others
/// still work there, hands the path to a survivor instead of deleting it out from under them:
/// the commit gate stays on, and the user's own hook stays in the chain to the very end.
#[test]
fn closing_the_newest_of_several_sessions_keeps_the_commit_gate_wired() {
    let f = fixture();
    let hooks = |repo: &Path| {
        let out = git_command(repo).args(["config", "--worktree", "--get", "core.hooksPath"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(&f.repo, &["config", "extensions.worktreeConfig", "true"]);
    git(&f.repo, &["config", "--worktree", "core.hooksPath", "user-hooks"]);
    let mut names = Vec::new();
    for _ in 0..3 {
        let s = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
        names.push(s["name"].as_str().unwrap().to_string());
    }
    // Newest first: every close but the last removes the active directory.
    for (closed, name) in names.iter().enumerate().rev() {
        ok(&f.engine, "session.close", json!({"session":name,"remove_worktree":false}));
        assert!(!f.repo.join(".relay/hooks").join(name).exists(), "{name}'s hook directory survived");
        if closed > 0 {
            let active = hooks(&f.repo);
            assert!(Path::new(&active).join("pre-commit").is_file(), "the hook path {active} points at nothing");
            assert!(names[..closed].iter().any(|live| active.ends_with(&format!(".relay/hooks/{live}"))), "{active}");
        }
    }
    assert_eq!(hooks(&f.repo), "user-hooks");
}

/// A launch that finds its row moved on while it was writing hooks and briefs is refused
/// instead of starting a second child: two concurrent spawns start exactly one.
#[test]
fn concurrent_spawns_start_exactly_one_child() {
    let f = fixture();
    let provider = fake_provider(&f.root, false);
    ok(&f.engine, "settings.set", json!({"path":"providers.claude.path","value":provider}));
    let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude"}));
    let name = session["name"].as_str().unwrap().to_string();
    let spawns: Vec<_> = (0..4).map(|_| {
        let engine = f.engine.clone();
        let name = name.clone();
        std::thread::spawn(move || call(&engine, "session.spawn", json!({"session":name})))
    }).collect();
    let results: Vec<Response> = spawns.into_iter().map(|h| h.join().unwrap()).collect();
    let started = results.iter().filter(|r| r.error.is_none()).count();
    assert_eq!(started, 1, "{:?}", results.iter().map(|r| r.error.as_ref().map(|e| e.code.clone())).collect::<Vec<_>>());
    for r in results.iter().filter(|r| r.error.is_some()) {
        assert_eq!(r.error.as_ref().unwrap().code, "session.already_spawned");
    }
    assert_eq!(f.engine.live_pty_count(), 1);
    ok(&f.engine, "session.close", json!({"session":name,"remove_worktree":true}));
}

/// An agent that writes a file as it is stopped: what it wrote after `session.close` checked the
/// checkout is found once it is gone, and the checkout is kept. The session reads `exited`.
#[test]
fn close_keeps_a_checkout_its_agent_wrote_into_after_the_dirty_check() {
    let f = fixture();
    let provider = fake_provider(&f.root, false);
    std::fs::write(&provider, "#!/bin/sh\ntrap 'echo late > \"$PWD/late.txt\"; exit 0' TERM\necho ready\nwhile :; do sleep 0.05; done\n").unwrap();
    ok(&f.engine, "settings.set", json!({"path":"providers.claude.path","value":provider}));
    let session = ok(&f.engine, "session.create", json!({"project_id":1,"provider":"claude"}));
    let name = session["name"].as_str().unwrap().to_string();
    let wt = PathBuf::from(session["worktree"].as_str().unwrap());
    ok(&f.engine, "session.spawn", json!({"session":name}));
    wait_until("the agent to start", || ok(&f.engine,"session.scrollback",json!({"session":name}))["text"].as_str().unwrap().contains("ready"));
    let error = common::refused(call(&f.engine, "session.close", json!({"session":name})));
    assert_eq!(error.code, "worktree.dirty", "{}", error.message);
    assert!(error.message.contains("stopped"), "{}", error.message);
    assert!(wt.join("late.txt").is_file(), "the late file was deleted with the checkout");
    assert!(wt.join("README.md").is_file());
    let state = || ok(&f.engine, "session.list", json!({"include_closed":true}))["sessions"].as_array().unwrap()
        .iter().find(|s| s["name"] == name.as_str()).map(|s| s["state"].clone()).unwrap();
    wait_until("the session to read exited", || state() == "exited");
    // Asked again, it is refused again until the work is dealt with.
    assert_eq!(code(call(&f.engine, "session.close", json!({"session":name}))), "worktree.dirty");
    ok(&f.engine, "session.close", json!({"session":name, "discard_changes": true}));
    assert!(!wt.exists());
}

/// A clean checkout on a detached HEAD can hold commits no branch has. Removing it pins them
/// under `refs/relay/rescue/` rather than leaving them to a reflog that goes with the checkout.
#[test]
fn removing_a_detached_checkout_pins_commits_no_branch_holds() {
    let f = fixture();
    let e = &f.engine;
    let session = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = session["name"].as_str().unwrap().to_string();
    let wt = PathBuf::from(session["worktree"].as_str().unwrap());
    let commit_detached = |wt: &Path, file: &str| -> String {
        git(wt, &["checkout", "-q", "--detach"]);
        std::fs::write(wt.join(file), "work\n").unwrap();
        git(wt, &["add", file]);
        git(wt, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--no-verify", "-m", "detached work"]);
        git(wt, &["rev-parse", "HEAD"])
    };
    let lost = commit_detached(&wt, "detached.txt");
    ok(e, "session.close", json!({"session": name}));
    assert!(!wt.exists());
    assert_eq!(git(&f.repo, &["rev-parse", &format!("refs/relay/rescue/{name}")]), lost);

    // `worktree.remove` names the ref it made; a detached HEAD a branch contains needs none.
    let w = ok(e, "worktree.create", json!({"project_id": 1, "branch": "relay/loose"}));
    let path = PathBuf::from(w["path"].as_str().unwrap());
    let lost = commit_detached(&path, "loose.txt");
    let r = ok(e, "worktree.remove", json!({"project_id": 1, "path": path}));
    let rescued = r["rescued"].as_str().expect("a rescue ref").to_string();
    assert!(rescued.starts_with("refs/relay/rescue/"), "{rescued}");
    assert_eq!(git(&f.repo, &["rev-parse", &rescued]), lost);
    let w = ok(e, "worktree.create", json!({"project_id": 1, "branch": "relay/held"}));
    let path = PathBuf::from(w["path"].as_str().unwrap());
    git(&path, &["checkout", "-q", "--detach"]);
    let r = ok(e, "worktree.remove", json!({"project_id": 1, "path": path}));
    assert!(r.get("rescued").is_none_or(|v| v.is_null()), "{r}");
}

/// Only a direct child of `.relay/worktrees` counts as one of Relay's own checkouts: a directory
/// inside a live pooled checkout is not one, and is neither purged nor deleted.
#[test]
fn worktree_remove_refuses_a_directory_inside_a_pooled_checkout() {
    let f = fixture();
    let e = &f.engine;
    let w = ok(e, "worktree.create", json!({"project_id": 1, "branch": "relay/inner"}));
    let pooled = PathBuf::from(w["path"].as_str().unwrap());
    let inner = pooled.join("src");
    std::fs::create_dir_all(inner.join("build")).unwrap();
    std::fs::write(inner.join("keep.txt"), "mine").unwrap();
    std::fs::write(inner.join("build/out.bin"), "x").unwrap();
    for discard in [false, true] {
        let r = call(e, "worktree.remove", json!({"project_id": 1, "path": inner, "discard_changes": discard}));
        assert_eq!(code(r), "worktree.remove_failed");
        assert!(inner.join("keep.txt").is_file() && inner.join("build/out.bin").is_file());
    }
    assert!(pooled.join("README.md").is_file());
}
