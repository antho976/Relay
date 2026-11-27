//! Session lifecycle edges: repeated and blocked `session.done`, late hooks, deleted tasks,
//! per-task caps at done, reused session names, and discard sharing close's cleanup.

mod common;

use common::{call_as, committed_repo, engine_with_project, git, ok, ok_as};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let ws = root.join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "hi\n")]);
    let engine = engine_with_project(&root, &ws, &repo);
    Fixture { _root: tmp, engine }
}

fn task(f: &Fixture, title: &str) -> i64 {
    ok(&f.engine, "task.create", json!({"project_id": 1, "title": title}))["id"].as_i64().unwrap()
}

/// A builder session on its own checkout, current on `first` with `queued` behind it.
fn session_with(f: &Fixture, first: i64, queued: &[i64]) -> (String, i64) {
    let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude", "task_id": first}));
    let (name, id) = (s["name"].as_str().unwrap().to_string(), s["id"].as_i64().unwrap());
    let conn = f.engine.store.lock();
    conn.execute("UPDATE tasks SET col='active' WHERE id=?1", [first]).unwrap();
    for (n, task_id) in queued.iter().enumerate() {
        conn.execute("UPDATE tasks SET col='active' WHERE id=?1", [task_id]).unwrap();
        conn.execute(
            "INSERT INTO task_sessions(task_id, session_id, ord, queue_ord) VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![task_id, id, n as i64 + 1],
        ).unwrap();
    }
    drop(conn);
    (name, id)
}

fn report(f: &Fixture, name: &str, kind: &str) {
    ok_as(&f.engine, Actor::agent(name), "session.report", json!({"session": name, "kind": kind}));
}
fn state(f: &Fixture, name: &str) -> String {
    ok(&f.engine, "session.get", json!({"session": name}))["state"].as_str().unwrap().to_string()
}

#[test]
fn a_second_done_in_one_turn_does_not_complete_the_next_task() {
    let f = fixture();
    let (a, b) = (task(&f, "first"), task(&f, "second"));
    let (name, _) = session_with(&f, a, &[b]);
    report(&f, &name, "session_start");
    let done = |summary: &str| ok_as(&f.engine, Actor::agent(&name), "session.done", json!({"session": name, "summary": summary}));
    assert_eq!(done("first finished")["task_id"], b);
    // The same turn, again: the agent has not been told about `b` yet.
    assert_eq!(done("first finished, again")["task_id"], b);
    assert_eq!(ok(&f.engine, "task.get", json!({"task_id": b}))["column"], "active");
    report(&f, &name, "stop");
    // The next turn's done is about `b`, and completes it.
    report(&f, &name, "tool_use");
    assert_eq!(done("second finished")["task_id"], Value::Null);
    assert_eq!(ok(&f.engine, "task.get", json!({"task_id": b}))["column"], "in_review");
}

#[test]
fn a_blocked_done_survives_its_trailing_tool_and_stop_reports() {
    let f = fixture();
    let a = task(&f, "blocked work");
    let (name, _) = session_with(&f, a, &[]);
    report(&f, &name, "session_start");
    ok_as(&f.engine, Actor::agent(&name), "session.done",
        json!({"session": name, "status": "blocked", "blockers": ["needs a decision"]}));
    report(&f, &name, "tool_use");
    report(&f, &name, "stop");
    assert_eq!(state(&f, &name), "blocked");
    // A later turn moves it on as usual.
    report(&f, &name, "tool_use");
    assert_eq!(state(&f, &name), "running");
}

fn waiting(f: &Fixture, name: &str) -> Value {
    let listed = ok(&f.engine, "session.list", json!({"project_id": 1}))["sessions"].clone();
    let row = listed.as_array().unwrap().iter().find(|s| s["name"] == name).cloned().unwrap();
    assert_eq!(row["waiting"], ok(&f.engine, "session.get", json!({"session": name}))["waiting"]);
    row["waiting"].clone()
}

#[test]
fn a_blocked_session_says_whether_it_waits_at_a_prompt_or_on_its_report() {
    let f = fixture();
    let a = task(&f, "work");
    let (name, _) = session_with(&f, a, &[]);
    report(&f, &name, "session_start");
    assert_eq!(waiting(&f, &name), Value::Null, "running waits on nothing");
    // A permission prompt: the person answers it in the terminal.
    ok_as(&f.engine, Actor::agent(&name), "session.report",
        json!({"session": name, "kind": "notification", "data": {"notification_type": "permission_prompt"}}));
    assert_eq!((state(&f, &name), waiting(&f, &name)), ("blocked".into(), json!("prompt")));
    report(&f, &name, "tool_use");
    assert_eq!(waiting(&f, &name), Value::Null, "answered: running again");
    // A blocked done: nothing to answer, through its trailing tool and Stop reports.
    ok_as(&f.engine, Actor::agent(&name), "session.done",
        json!({"session": name, "status": "partial", "blockers": ["needs a key"]}));
    assert_eq!(waiting(&f, &name), json!("report"));
    report(&f, &name, "tool_use");
    report(&f, &name, "stop");
    // Claude's idle_prompt a minute later asks nothing new.
    ok_as(&f.engine, Actor::agent(&name), "session.report",
        json!({"session": name, "kind": "notification", "data": {"notification_type": "idle_prompt"}}));
    assert_eq!((state(&f, &name), waiting(&f, &name)), ("blocked".into(), json!("report")));
    // The next turn moves on; a prompt in it is a prompt again.
    report(&f, &name, "prompt");
    assert_eq!(waiting(&f, &name), Value::Null);
    report(&f, &name, "blocked");
    assert_eq!(waiting(&f, &name), json!("prompt"));
}

#[test]
fn a_late_hook_does_not_revive_a_parked_or_exited_session() {
    let f = fixture();
    let a = task(&f, "work");
    let (name, id) = session_with(&f, a, &[]);
    report(&f, &name, "session_start");
    for gone in ["parked", "exited"] {
        f.engine.store.lock().execute("UPDATE sessions SET state=?1 WHERE id=?2", rusqlite::params![gone, id]).unwrap();
        for kind in ["tool_use", "stop", "session_start"] {
            report(&f, &name, kind);
            assert_eq!(state(&f, &name), gone, "{kind} revived a {gone} session");
        }
    }
}

#[test]
fn deleting_the_current_task_does_not_break_done_or_hook_reports() {
    let f = fixture();
    let a = task(&f, "doomed");
    let (name, _) = session_with(&f, a, &[]);
    report(&f, &name, "session_start");
    ok(&f.engine, "task.delete", json!({"task_id": a}));
    ok_as(&f.engine, Actor::agent(&name), "session.report",
        json!({"session": name, "kind": "notification", "data": {"notification_type": "permission_prompt"}}));
    report(&f, &name, "tool_use");
    report(&f, &name, "stop");
    let done = ok_as(&f.engine, Actor::agent(&name), "session.done", json!({"session": name, "summary": "gone anyway"}));
    assert_eq!(done["task_id"], Value::Null, "the session moves off the deleted task");
}

#[test]
fn done_applies_the_per_task_caps_to_work_committed_past_the_gate() {
    let f = fixture();
    ok(&f.engine, "settings.set", json!({"path": "guardrails.caps", "value": {"files": 2, "lines": 100}}));
    let a = task(&f, "large");
    let (name, _) = session_with(&f, a, &[]);
    let worktree = PathBuf::from(ok(&f.engine, "session.get", json!({"session": name}))["worktree"].as_str().unwrap());
    report(&f, &name, "session_start");
    for n in 0..3 {
        std::fs::write(worktree.join(format!("f{n}.txt")), "x\n").unwrap();
    }
    git(&worktree, &["add", "."]);
    git(&worktree, &["commit", "-q", "--no-verify", "-m", "over the cap"]);
    let refused = call_as(&f.engine, Actor::agent(&name), "session.done", json!({"session": name}))
        .error.expect("done over the caps must be refused");
    assert_eq!(refused.code, "guardrail.cap");
    assert_eq!(ok(&f.engine, "task.get", json!({"task_id": a}))["column"], "active");
    // Saying it is stuck is always allowed.
    ok_as(&f.engine, Actor::agent(&name), "session.done",
        json!({"session": name, "status": "blocked", "blockers": ["over the caps"]}));
}

#[test]
fn a_reused_session_name_inherits_neither_inbox_nor_outbox() {
    let f = fixture();
    let old = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let old_name = old["name"].as_str().unwrap().to_string();
    let peer = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    ok(&f.engine, "mailbox.send", json!({"project_id": 1, "to": old_name, "text": "for the old one"}));
    ok_as(&f.engine, Actor::agent(&old_name), "mailbox.send",
        json!({"project_id": 1, "to": peer["name"], "text": "from the old one"}));
    ok(&f.engine, "session.close", json!({"session": old_name}));
    std::thread::sleep(std::time::Duration::from_millis(5));
    let new = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    f.engine.store.lock().execute("UPDATE sessions SET name=?1 WHERE id=?2", rusqlite::params![old_name, new["id"].as_i64().unwrap()]).unwrap();
    let inbox = ok_as(&f.engine, Actor::agent(&old_name), "mailbox.list", json!({"project_id": 1}));
    assert!(inbox["messages"].as_array().unwrap().is_empty(), "{inbox}");
    let outbox = ok_as(&f.engine, Actor::agent(&old_name), "mailbox.outbox", json!({"project_id": 1}));
    assert!(outbox["sent"].as_array().unwrap().is_empty(), "{outbox}");
    // The new session's own mail is its own.
    ok(&f.engine, "mailbox.send", json!({"project_id": 1, "to": old_name, "text": "for the new one"}));
    let inbox = ok_as(&f.engine, Actor::agent(&old_name), "mailbox.list", json!({"project_id": 1}));
    assert_eq!(inbox["messages"].as_array().unwrap().len(), 1);
}

#[test]
fn discarding_a_restorable_session_expires_its_holds_like_close() {
    let f = fixture();
    let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let (name, id) = (s["name"].as_str().unwrap().to_string(), s["id"].as_i64().unwrap());
    let conn = f.engine.store.lock();
    conn.execute("UPDATE sessions SET state='restorable' WHERE id=?1", [id]).unwrap();
    conn.execute(
        "INSERT INTO holds(project_id, session_id, session, actor, op, envelope, policy, details, state, created_at)
         VALUES (1, ?1, ?2, 'agent', 'file.write', '{}', 'destructive_write', '{}', 'open', 'now')",
        rusqlite::params![id, name],
    ).unwrap();
    drop(conn);
    ok(&f.engine, "session.discard_restorable", json!({"session": name}));
    let held: String = f.engine.store.lock()
        .query_row("SELECT state FROM holds WHERE session_id=?1", [id], |row| row.get(0)).unwrap();
    assert_eq!(held, "expired");
}

#[test]
fn an_assignment_to_a_busy_session_waits_for_its_next_stop() {
    let f = fixture();
    let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let (name, id) = (s["name"].as_str().unwrap().to_string(), s["id"].as_i64().unwrap());
    report(&f, &name, "session_start");
    let b = task(&f, "assigned while busy");
    ok(&f.engine, "task.dispatch", json!({"task_id": b, "session": name}));
    let marker = || -> Option<i64> {
        f.engine.store.lock().query_row("SELECT done_pending_stop FROM sessions WHERE id=?1", [id], |row| row.get(0)).unwrap()
    };
    assert!(marker().is_some(), "a running session keeps its assignment for its next Stop");
    report(&f, &name, "tool_use");
    assert!(marker().is_some(), "a tool event is not the handoff edge");
    report(&f, &name, "stop");
    // The Stop took it: it is typed into the PTY when there is one, and in mail either way.
    let completions: i64 = f.engine.store.lock().query_row(
        "SELECT COUNT(*) FROM notifications WHERE category='agent_done'", [], |row| row.get(0)).unwrap();
    assert_eq!(completions, 1, "the turn that ended still reports its completion");
}

#[test]
fn a_repeated_report_moves_its_unread_card_to_the_time_of_the_latest_one() {
    // RA-234: the dedup rewrote the body and kept the first report's time, so notify.list
    // (newest first) buried the card under older ones and showed a stale time.
    let f = fixture();
    let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap().to_string();
    let stop = |message: Option<&str>| {
        report(&f, &name, "session_start");
        let data = message.map_or(json!({}), |m| json!({"message": m}));
        ok_as(&f.engine, Actor::agent(&name), "session.report", json!({"session": name, "kind": "stop", "data": data}));
    };
    let card = || -> (i64, String, String) {
        f.engine.store.lock().query_row(
            "SELECT COUNT(*),MAX(body),MAX(created_at) FROM notifications WHERE category='agent_done'", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap()
    };
    stop(Some("first summary"));
    let age = || f.engine.store.lock().execute("UPDATE notifications SET created_at='2000-01-01T00:00:00Z'", []).unwrap();
    age();
    stop(Some("second summary"));
    let (count, body, at) = card();
    assert_eq!((count, body.as_str()), (1, "second summary"));
    assert!(at.as_str() > "2000-01-01T00:00:00Z", "kept the first report's time: {at}");
    // A bare hook keeps the real summary but is still the latest report.
    age();
    stop(None);
    let (count, body, at) = card();
    assert_eq!((count, body.as_str()), (1, "second summary"));
    assert!(at.as_str() > "2000-01-01T00:00:00Z");
    let listed = ok(&f.engine, "notify.list", json!({}));
    assert_eq!(listed["notifications"][0]["body"], "second summary");
}

#[test]
fn restorable_sessions_can_be_asked_for_by_project_or_session() {
    // RA-235: each row costs a git status of its checkout; a client describing one pane asks
    // for that pane's.
    let f = fixture();
    let names: Vec<String> = (0..2).map(|_| {
        let s = ok(&f.engine, "session.create", json!({"project_id": 1, "provider": "claude"}));
        f.engine.store.lock().execute("UPDATE sessions SET state='restorable' WHERE id=?1", [s["id"].as_i64().unwrap()]).unwrap();
        s["name"].as_str().unwrap().to_string()
    }).collect();
    let listed = |payload: Value| -> Vec<String> {
        ok(&f.engine, "session.restorable", payload)["sessions"].as_array().unwrap().iter()
            .map(|r| r["session"]["name"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(listed(json!({})), names);
    assert_eq!(listed(json!({"project_id": 1})), names);
    assert!(listed(json!({"project_id": 2})).is_empty());
    assert_eq!(listed(json!({"session": names[1]})), vec![names[1].clone()]);
    assert!(listed(json!({"project_id": 2, "session": names[1]})).is_empty());
}
