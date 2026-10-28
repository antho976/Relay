//! A shell session (`Provider::Shell`): the person's own login shell in a real PTY, in the
//! project's folder, opened from the desktop or a paired phone. Only the person opens one; it has
//! no checkout, hooks, brief, task or pair; nothing it runs inherits an agent's Relay variables;
//! a thread's agent reads nothing from it; and agents neither see it as a peer nor mail it in a
//! broadcast. All through the bus.

use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn call(e: &Engine, actor: Actor, op: &str, payload: Value) -> Response {
    e.dispatch(Request::new(actor, op, payload), Door::InProcess)
}

fn user(e: &Engine, op: &str, payload: Value) -> Value {
    call(e, Actor::User, op, payload).into_result()
        .unwrap_or_else(|error| panic!("{op} failed: {} {}", error.code, error.message))
}

fn code(response: Response) -> String {
    response.into_result().expect_err("expected a refusal").code
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Fixture {
    _tmp: tempfile::TempDir,
    app: PathBuf,
    engine: Arc<Engine>,
}

/// Project 1 at `ws/app`, a git repository on `main` with one commit and an active task 1.
fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let app = root.join("ws/app");
    std::fs::create_dir_all(&app).unwrap();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.email", "t@t"], &["config", "user.name", "t"]] {
        git(&app, args);
    }
    std::fs::write(app.join("README"), "app\n").unwrap();
    git(&app, &["add", "."]);
    git(&app, &["commit", "-qm", "init"]);
    let engine = Engine::new(Instance::Test, Store::open(&root.join("store/store.db"), false).unwrap());
    user(&engine, "workspace.create", json!({"path": root.join("ws")}));
    user(&engine, "project.add", json!({"workspace_id": 1, "path": app}));
    engine.store.with_tx(|tx| {
        tx.execute(
            "INSERT INTO tasks(project_id,title,body,changelog,col,state,position,created_at,updated_at)
             VALUES (1,'Ship it','','','active','working',0,?1,?1)",
            ["2026-10-01T00:00:00Z"],
        )?;
        Ok(())
    }).unwrap();
    // A plain POSIX shell stands in for the person's login shell.
    user(&engine, "settings.set", json!({"path": "providers.shell.path", "value": "/bin/sh"}));
    Fixture { _tmp: tmp, app, engine }
}

fn name(session: &Value) -> String { session["name"].as_str().unwrap().to_string() }

fn scrollback(e: &Engine, session: &str) -> String {
    user(e, "session.scrollback", json!({"session": session}))["text"].as_str().unwrap_or_default().to_string()
}

fn wait_for(e: &Engine, session: &str, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = scrollback(e, session);
        if text.contains(needle) {
            return text;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {needle:?} in:\n{text}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_person_opens_a_shell_in_the_project_folder_with_no_agent_around_it() {
    let f = fixture();
    let e = &f.engine;
    let hooks_before = git(&f.app, &["config", "--get", "core.hooksPath"]);
    let shell = user(e, "session.create", json!({"project_id": 1, "provider": "shell"}));
    assert_eq!((shell["provider"].as_str(), shell["worktree"].as_str(), shell["branch"].as_str()), (Some("shell"), Some(""), Some("")), "{shell}");
    let shell = name(&shell);
    let spawned = user(e, "session.spawn", json!({"session": shell}));
    assert_eq!(spawned["state"], "running");

    // It runs where the project is, and inherits none of an agent's variables, even from an
    // engine that has them itself (a test run inside a Relay session does).
    user(e, "session.input", json!({"session": shell, "data": "pwd; echo \"tok=[$RELAY_TOKEN] ses=[$RELAY_SESSION]\"\r"}));
    let text = wait_for(e, &shell, "tok=[] ses=[]");
    assert!(text.contains(&f.app.display().to_string()), "{text}");

    // No agent, so no brief and no transcript to read, and nothing in the checkout's hooks.
    assert_eq!(code(call(e, Actor::User, "session.brief", json!({"session": shell}))), "session.shell");
    assert_eq!(user(e, "session.transcript", json!({"session": shell}))["found"], false);
    assert_eq!(git(&f.app, &["config", "--get", "core.hooksPath"]), hooks_before);

    // Parked and woken, it comes back as a fresh shell with what it showed before.
    user(e, "session.park", json!({"session": shell}));
    user(e, "session.wake", json!({"session": shell}));
    assert!(scrollback(e, &shell).contains("tok=[] ses=[]"));

    // Closing it leaves the project's own checkout and branch exactly as they were.
    user(e, "session.close", json!({"session": shell}));
    assert!(f.app.join("README").is_file());
    assert_eq!(git(&f.app, &["branch", "--show-current"]), "main");
    assert_eq!(git(&f.app, &["config", "--get", "core.hooksPath"]), hooks_before);
}

#[test]
fn a_shell_that_exited_is_discarded_without_touching_the_checkout() {
    let f = fixture();
    let e = &f.engine;
    let hooks_before = git(&f.app, &["config", "--get", "core.hooksPath"]);
    let shell = name(&user(e, "session.create", json!({"project_id": 1, "provider": "shell"})));
    user(e, "session.spawn", json!({"session": shell}));
    user(e, "session.input", json!({"session": shell, "data": "exit\r"}));
    let deadline = Instant::now() + Duration::from_secs(10);
    while user(e, "session.get", json!({"session": shell}))["state"] != "exited" {
        assert!(Instant::now() < deadline, "the shell never exited");
        std::thread::sleep(Duration::from_millis(20));
    }
    user(e, "session.discard_restorable", json!({"session": shell}));
    assert!(f.app.join("README").is_file());
    assert_eq!(git(&f.app, &["status", "--porcelain"]), "");
    assert_eq!(git(&f.app, &["config", "--get", "core.hooksPath"]), hooks_before);
}

#[test]
fn only_the_person_opens_a_shell_and_it_takes_nothing_an_agent_has() {
    let f = fixture();
    let e = &f.engine;
    let agent = name(&user(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"})));

    // An agent, and a thread's agent acting as the person, are refused.
    assert!(call(e, Actor::agent(&agent), "session.create", json!({"project_id": 1, "provider": "shell"})).into_result().is_err());
    let from_thread = e.dispatch_from_thread(Request::new(Actor::User, "session.create", json!({"project_id": 1, "provider": "shell"})), Door::InProcess, 1);
    assert_eq!(code(from_thread), "session.shell");

    // Nothing that only means something to an agent.
    for extra in [
        json!({"role": "builder"}), json!({"model": "opus"}), json!({"effort": "high"}), json!({"task_id": 1}),
        json!({"pair_with": agent}), json!({"worktree": "new"}), json!({"branch": "x"}), json!({"prompt": "go"}),
        json!({"bus_writes": true}), json!({"allow_ui": true}), json!({"use_api_key": true}),
    ] {
        let mut payload = json!({"project_id": 1, "provider": "shell"});
        payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        assert_eq!(code(call(e, Actor::User, "session.create", payload.clone())), "session.shell", "{payload}");
    }

    // A task is never typed into one: not dispatched to a new shell, nor to an open one.
    let dispatch = json!({"task_id": 1, "start": false, "create": {"project_id": 1, "provider": "shell"}});
    assert_eq!(code(call(e, Actor::User, "task.dispatch", dispatch)), "task.dispatch_shell");
    let shell = name(&user(e, "session.create", json!({"project_id": 1, "provider": "shell"})));
    assert_eq!(code(call(e, Actor::User, "task.dispatch", json!({"task_id": 1, "session": shell, "start": false}))), "task.dispatch_shell");
    assert_eq!(code(call(e, Actor::User, "session.update", json!({"session": shell, "task_id": 1}))), "session.shell");

    // Agents do not see it as a peer, and a broadcast does not reach it.
    let peers = user(e, "session.peers", json!({"project_id": 1}))["peers"].clone();
    let names: Vec<&str> = peers.as_array().unwrap().iter().filter_map(|p| p["session"].as_str()).collect();
    assert!(names.contains(&agent.as_str()) && !names.contains(&shell.as_str()), "{peers}");
    let reached = |session: &str| -> i64 {
        e.store.lock().query_row("SELECT COUNT(*) FROM message_recipients WHERE session=?1", [session], |r| r.get(0)).unwrap()
    };
    let other = name(&user(e, "session.create", json!({"project_id": 1, "provider": "codex"})));
    call(e, Actor::agent(&agent), "mailbox.send", json!({"to": "*", "text": "hello all"})).into_result().unwrap();
    assert_eq!((reached(&other), reached(&shell)), (1, 0));
}

#[test]
fn a_threads_agent_reads_nothing_from_a_shell() {
    let f = fixture();
    let e = &f.engine;
    let shell = name(&user(e, "session.create", json!({"project_id": 1, "provider": "shell"})));
    user(e, "session.spawn", json!({"session": shell}));
    user(e, "session.input", json!({"session": shell, "data": "echo secret-output\r"}));
    wait_for(e, &shell, "secret-output\r\n");
    for op in ["session.scrollback", "session.transcript", "session.attach"] {
        let refused = e.dispatch_from_thread(Request::new(Actor::User, op, json!({"session": shell})), Door::InProcess, 1);
        assert_eq!(code(refused), "session.shell", "{op}");
    }
    // The person reads it as before.
    assert!(scrollback(e, &shell).contains("secret-output"));
    user(e, "session.close", json!({"session": shell}));
}
