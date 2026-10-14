//! The capabilities the 2026-08-19 agent surface audit asked for and did not find: claims,
//! a wake-up, a plan preflight, honest completion, peer intent, and one call that says who
//! you are (docs/DECISIONS.md D115 … D119). Exercised as a bound agent, through the bus.

mod common;

use common::{call_as as call, committed_repo, engine_with_project, ok_as as ok};
use relay_bus::{Actor, Request};
use relay_core::engine::{Door, Engine};
use serde_json::{json, Value};
use std::sync::Arc;

fn refusal(engine: &Engine, actor: Actor, op: &str, payload: Value) -> relay_bus::error::BusError {
    call(engine, actor, op, payload)
        .into_result()
        .expect_err(&format!("{op} was expected to refuse"))
}

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
    a: Value,
    b: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        let repo = ws.join("app");
        committed_repo(&repo, &[("src/lib.rs", "pub fn one() -> i32 { 1 }\n")]);
        let engine = engine_with_project(root.path(), &ws, &repo);
        engine.store.with_tx(|tx| {
            tx.execute(
                "INSERT INTO tasks(project_id,title,body,changelog,col,state,position,created_at,updated_at)
                 VALUES (1,'Ship the thing','','','active','dispatched',0,?1,?1)",
                ["2026-08-19T00:00:00Z"],
            )?;
            Ok(())
        }).unwrap();
        let a = ok(&engine, Actor::User, "session.create",
            json!({"project_id": 1, "provider": "claude", "role": "builder", "task_id": 1}));
        let b = ok(&engine, Actor::User, "session.create",
            json!({"project_id": 1, "provider": "codex", "role": "builder"}));
        Self { _root: root, engine, a, b }
    }
    fn me(&self) -> Actor { Actor::agent(self.a["name"].as_str().unwrap()) }
    fn peer(&self) -> Actor { Actor::agent(self.b["name"].as_str().unwrap()) }
    fn my_name(&self) -> &str { self.a["name"].as_str().unwrap() }
    fn peer_name(&self) -> &str { self.b["name"].as_str().unwrap() }
}

/// The data model already had a `claims` table and a `claimed` field on every peer. What was
/// missing was any way for an agent to put something in it.
#[test]
fn claims_are_declarable_releasable_and_report_who_else_holds_the_file() {
    let f = Fixture::new();
    let mine = ok(&f.engine, f.me(), "session.claim",
        json!({"paths": ["src/lib.rs", "src/main.rs"], "note": "extracting the parser"}));
    assert_eq!(mine["claimed"].as_array().unwrap().len(), 2);
    assert!(mine["collisions"].as_array().unwrap().is_empty(), "nobody else was here");

    // A peer taking the same file is told who has it, and still gets its claim.
    let theirs = ok(&f.engine, f.peer(), "session.claim", json!({"paths": ["src/lib.rs"]}));
    let collisions = theirs["collisions"].as_array().unwrap();
    assert_eq!(collisions.len(), 1, "the collision must be reported, not swallowed");
    assert_eq!(collisions[0]["session"], f.my_name());
    assert_eq!(collisions[0]["path"], "src/lib.rs");

    // …unless it asks to be the only holder, in which case nothing is recorded.
    ok(&f.engine, f.peer(), "session.release", json!({}));
    let refused = refusal(&f.engine, f.peer(), "session.claim",
        json!({"paths": ["src/lib.rs"], "exclusive": true}));
    assert_eq!(refused.code, "session.claim_held");
    assert!(refused.message.contains(f.my_name()));
    let after = ok(&f.engine, f.peer(), "session.claim", json!({"paths": ["src/other.rs"]}));
    assert_eq!(after["claimed"], json!(["src/other.rs"]), "the refused claim left no trace");

    // Claims surface where coordination happens.
    let peers = ok(&f.engine, f.peer(), "session.peers", json!({}));
    let claimed = peers["peers"][0]["claimed"].as_array().unwrap();
    assert!(claimed.iter().any(|c| c == "src/lib.rs"), "a peer's claims are visible: {claimed:?}");

    // Releasing one path leaves the rest.
    ok(&f.engine, f.me(), "session.release", json!({"paths": ["src/lib.rs"]}));
    let left = ok(&f.engine, f.peer(), "session.peers", json!({}));
    let claimed = left["peers"][0]["claimed"].as_array().unwrap();
    assert_eq!(claimed.len(), 1, "only the released path went: {claimed:?}");

    assert_eq!(ok(&f.engine, f.me(), "session.release", json!({}))["released"], 1);
    assert_eq!(refusal(&f.engine, f.me(), "session.claim", json!({"paths": ["/etc/passwd"]})).code,
        "session.claim_path");
    assert_eq!(refusal(&f.engine, f.me(), "session.claim", json!({"paths": []})).code, "session.claim");
}

/// Done, close and discard each free the session's claims for the next agent.
#[test]
fn finishing_or_closing_releases_claims_for_the_next_agent() {
    for action in ["done", "close", "discard"] {
        let f = Fixture::new();
        ok(&f.engine, f.me(), "session.claim", json!({"paths":["src/lib.rs"]}));
        match action {
            "done" => { ok(&f.engine, f.me(), "session.done", json!({"summary":"Finished"})); }
            "close" => { ok(&f.engine, Actor::User, "session.close", json!({"session":f.my_name(),"remove_worktree":false})); }
            _ => {
                f.engine.store.lock().execute("UPDATE sessions SET state='restorable' WHERE name=?1", [f.my_name()]).unwrap();
                ok(&f.engine, Actor::User, "session.discard_restorable", json!({"session":f.my_name()}));
            }
        }
        let claim = ok(&f.engine, f.peer(), "session.claim", json!({"paths":["src/lib.rs"],"exclusive":true}));
        assert!(claim["collisions"].as_array().unwrap().is_empty(), "{action}");
    }
}

/// A one-line answer to "what is that session doing?", which no amount of branch name gives.
#[test]
fn intent_is_one_line_and_reaches_the_peer_table() {
    let f = Fixture::new();
    ok(&f.engine, f.me(), "session.intent", json!({"text": "extracting the parser from lib.rs"}));
    let peers = ok(&f.engine, f.peer(), "session.peers", json!({}));
    assert_eq!(peers["peers"][0]["intent"], "extracting the parser from lib.rs");
    assert_eq!(ok(&f.engine, f.me(), "session.get", json!({"session": f.my_name()}))["intent"],
        "extracting the parser from lib.rs");

    // Empty clears it; a paragraph is not one line.
    ok(&f.engine, f.me(), "session.intent", json!({"text": "  "}));
    assert!(ok(&f.engine, f.peer(), "session.peers", json!({}))["peers"][0]["intent"].is_null());
    assert_eq!(
        refusal(&f.engine, f.me(), "session.intent", json!({"text": "x".repeat(201)})).code,
        "session.intent",
    );
    // It is this session's own line to write.
    assert_eq!(
        refusal(&f.engine, f.me(), "session.intent", json!({"session": f.peer_name(), "text": "no"})).code,
        "actor.scope",
    );
}

/// `guardrail.check` answers one action. A plan is decided before the first action.
#[test]
fn explain_preflights_a_whole_plan_before_the_first_action() {
    let f = Fixture::new();
    ok(&f.engine, Actor::User, "guardrail.config.set",
        json!({"patch": {"protected_paths": ["secrets/**"], "caps": {"files": 3, "lines": 100}}}));

    let plan = ok(&f.engine, f.me(), "guardrail.explain", json!({
        "paths": ["src/lib.rs", "secrets/key.pem"],
        "lines": 40,
        "commands": ["cargo test", "git push --force"],
    }));
    assert_eq!(plan["verdict"], "refuse", "the plan as a whole cannot proceed");

    let paths = plan["paths"].as_array().unwrap();
    assert_eq!(paths[0]["verdict"], "allow");
    assert_eq!(paths[1]["verdict"], "refuse");
    assert_eq!(paths[1]["policy"], "guardrail.protected_path");

    let commands = plan["commands"].as_array().unwrap();
    assert_eq!(commands[0]["verdict"], "allow", "cargo test is ordinary work");
    assert_eq!(commands[1]["verdict"], "refuse");
    assert_eq!(commands[1]["policy"], "guardrail.command");

    assert_eq!(plan["files"], 2);
    assert_eq!(plan["caps"]["files"], 3);
    assert_eq!(plan["over_caps"], false);
    assert!(!plan["write_roots"].as_array().unwrap().is_empty());

    // A plan that is merely too big is over caps, with nothing individually wrong.
    let big = ok(&f.engine, f.me(), "guardrail.explain",
        json!({"paths": ["a.rs", "b.rs", "c.rs", "d.rs"], "lines": 4000}));
    assert_eq!(big["over_caps"], true);
    assert_eq!(big["verdict"], "refuse");
    assert!(big["paths"].as_array().unwrap().iter().all(|p| p["verdict"] == "allow"));

    // It is a dry run: nothing was created.
    let holds: i64 = f.engine.store.lock()
        .query_row("SELECT COUNT(*) FROM holds", [], |row| row.get(0)).unwrap();
    assert_eq!(holds, 0);
}

/// An op that can only report success gets reported success.
#[test]
fn done_can_say_blocked_and_the_task_does_not_move() {
    let f = Fixture::new();
    let second = ok(&f.engine, Actor::User, "task.create",
        json!({"project_id": 1, "title": "Ship the other thing", "column": "active"}));
    ok(&f.engine, Actor::User, "session.update",
        json!({"session": f.my_name(), "task_id": second["id"]}));
    ok(&f.engine, f.me(), "task.update",
        json!({"task_id": 1, "body": "The first task remains agent-owned after another is attached"}));
    assert_eq!(ok(&f.engine, Actor::User, "task.get", json!({"task_id": 1}))["state"], "dispatched",
        "a body edit keeps the queued task's state");
    let blocked = ok(&f.engine, f.me(), "session.done", json!({
        "status": "blocked",
        "summary": "parser extracted",
        "blockers": ["upstream crate needs a version bump I cannot make"],
    }));
    assert_eq!(blocked["state"], "blocked");

    let task = ok(&f.engine, Actor::User, "task.get", json!({"task_id": 1}));
    assert_eq!(task["column"], "active");
    assert_eq!(task["state"], "dispatched", "blocking the current task must not touch its queue");
    let second_task = ok(&f.engine, Actor::User, "task.get", json!({"task_id": second["id"]}));
    assert_eq!(second_task["state"], "blocked", "only the scalar current task is blocked");

    let notes = ok(&f.engine, Actor::User, "notify.list", json!({}));
    let latest = &notes["notifications"].as_array().unwrap()[0];
    assert_eq!(latest["category"], "agent_blocked");
    assert!(latest["body"].as_str().unwrap().contains("version bump"), "the blocker is the message");

    // Saying you are blocked without saying by what is not a report.
    assert_eq!(
        refusal(&f.engine, f.me(), "session.done", json!({"status": "blocked"})).code,
        "session.done_blockers",
    );
    assert_eq!(
        refusal(&f.engine, f.me(), "session.done", json!({"status": "nearly"})).code,
        "session.done_status",
    );

    // Completion moves only the current task and advances the queue.
    let advanced = ok(&f.engine, f.me(), "session.done", json!({"status": "completed", "summary": "done"}));
    assert_eq!(advanced["task_id"], 1, "the next queued task becomes current");
    let task = ok(&f.engine, Actor::User, "task.get", json!({"task_id": 1}));
    assert_eq!(task["column"], "active", "the queued task is not completed by somebody else's report");
    let second_task = ok(&f.engine, Actor::User, "task.get", json!({"task_id": second["id"]}));
    assert_eq!(second_task["column"], "in_review");

    let bootstrap = ok(&f.engine, f.me(), "session.bootstrap", json!({}));
    assert_eq!(bootstrap["task"]["id"], 1);
    assert_eq!(bootstrap["tasks"][0]["id"], 1, "current task leads the returned queue");
    assert_eq!(bootstrap["tasks"][1]["id"], second["id"]);

    // A second report in the same turn is about the work already reported: the promoted task
    // is only the agent's once that turn's Stop hands it over.
    ok(&f.engine, f.me(), "session.done", json!({"status": "completed", "summary": "done, again"}));
    assert_eq!(ok(&f.engine, Actor::User, "task.get", json!({"task_id": 1}))["column"], "active");
    ok(&f.engine, f.me(), "session.report", json!({"session": f.my_name(), "kind": "stop"}));
    // The next turn's report applies to the promoted task, not the already reviewed one.
    ok(&f.engine, f.me(), "session.done", json!({"status": "completed", "summary": "queue drained"}));
    assert_eq!(ok(&f.engine, Actor::User, "task.get", json!({"task_id": 1}))["column"], "in_review");
}

/// One call that says who you are and what you may call — for the user too, unlike bootstrap.
#[test]
fn whoami_answers_for_an_agent_and_for_the_user() {
    let f = Fixture::new();
    let me = ok(&f.engine, f.me(), "bus.whoami", json!({}));
    assert_eq!(me["is_agent"], true);
    assert_eq!(me["session"], f.my_name());
    assert_eq!(me["role"], "builder");
    assert_eq!(me["project"], "app");
    assert_eq!(me["worktree"], f.a["worktree"]);
    let can_call: Vec<&str> = me["can_call"].as_array().unwrap().iter()
        .map(|op| op.as_str().unwrap()).collect();
    assert!(can_call.contains(&"session.claim"));
    assert!(!can_call.contains(&"task.create"));
    assert!(!me["write_roots"].as_array().unwrap().is_empty());

    let user = ok(&f.engine, Actor::User, "bus.whoami", json!({}));
    assert_eq!(user["is_agent"], false);
    assert!(user["session"].is_null());
    let user_can: Vec<&str> = user["can_call"].as_array().unwrap().iter()
        .map(|op| op.as_str().unwrap()).collect();
    assert!(user_can.contains(&"task.create"), "the user is not bound by a role allowlist");
    assert!(user_can.len() > can_call.len());
}

#[test]
fn priority_mail_rides_every_agent_response_until_acknowledged() {
    let f = Fixture::new();
    let sent = ok(
        &f.engine,
        Actor::User,
        "mailbox.send",
        json!({
            "project_id": 1,
            "to": f.my_name(),
            "text": "Review the guardrail decision",
            "priority": true
        }),
    );
    let message_id = sent["message"]["id"].as_i64().unwrap();
    assert_eq!(sent["message"]["priority"], true);

    let normal = call(
        &f.engine,
        f.me(),
        "session.get",
        json!({"session": f.my_name()}),
    );
    assert_eq!(normal.mail.as_ref().map(|mail| mail.priority), Some(1));

    let failed = call(&f.engine, f.me(), "no.such_op", json!({}));
    assert_eq!(failed.error.as_ref().unwrap().code, "bus.unknown_op");
    assert_eq!(failed.mail.as_ref().map(|mail| mail.priority), Some(1));

    let unread = call(
        &f.engine,
        f.me(),
        "mailbox.list",
        json!({"unread_only": true}),
    );
    assert_eq!(unread.mail.as_ref().map(|mail| mail.priority), Some(1));
    assert_eq!(unread.result.as_ref().unwrap()["messages"][0]["priority"], true);

    let fixed = Request::new(
        f.me(),
        "session.intent",
        json!({"text": "checking priority replay state"}),
    );
    let first = f.engine.dispatch(fixed.clone(), Door::InProcess);
    assert_eq!(first.mail.as_ref().map(|mail| mail.priority), Some(1));
    let acked = call(
        &f.engine,
        f.me(),
        "mailbox.ack",
        json!({"message_id": message_id}),
    );
    assert!(acked.mail.is_none(), "the acknowledgement must clear the live hint");
    let replay = f.engine.dispatch(fixed, Door::InProcess);
    assert_eq!(replay.replayed, Some(true));
    assert!(replay.mail.is_none(), "replay metadata must reflect current mail state");
}

#[test]
fn agent_priority_mail_is_direct_task_linked_and_backpressured() {
    let f = Fixture::new();
    assert_eq!(
        refusal(
            &f.engine,
            f.me(),
            "mailbox.send",
            json!({"to": "*", "text": "urgent", "re_task": 1, "priority": true}),
        )
        .code,
        "mailbox.priority_broadcast"
    );
    assert_eq!(
        refusal(
            &f.engine,
            f.me(),
            "mailbox.send",
            json!({"to": f.peer_name(), "text": "urgent", "priority": true}),
        )
        .code,
        "mailbox.priority_task"
    );

    let first = ok(
        &f.engine,
        f.me(),
        "mailbox.send",
        json!({"to": f.peer_name(), "text": "Need review", "re_task": 1, "priority": true}),
    );
    assert_eq!(
        refusal(
            &f.engine,
            f.me(),
            "mailbox.send",
            json!({"to": f.peer_name(), "text": "Still need review", "re_task": 1, "priority": true}),
        )
        .code,
        "mailbox.priority_pending"
    );
    ok(
        &f.engine,
        f.peer(),
        "mailbox.ack",
        json!({"message_id": first["message"]["id"]}),
    );
    ok(
        &f.engine,
        f.me(),
        "mailbox.send",
        json!({"to": f.peer_name(), "text": "New issue", "re_task": 1, "priority": true}),
    );
}

#[test]
fn agent_suggestions_are_attributed_unpinned_and_not_standing_context() {
    let f = Fixture::new();
    let first = ok(
        &f.engine,
        f.me(),
        "notes.append",
        json!({
            "project_id": 1,
            "target": "suggestions",
            "text": "While testing the parser, schema errors hid the bad field."
        }),
    );
    let note_id = first["id"].as_i64().unwrap();
    assert_eq!(first["title"], "Agent suggestions");
    assert_eq!(first["pinned"], false);
    let body = first["body"].as_str().unwrap();
    assert!(body.contains(f.my_name()));
    assert!(body.contains("task #1"));
    assert!(body.contains("While testing the parser"));

    let standing = ok(
        &f.engine,
        Actor::User,
        "notes.standing",
        json!({"project_id": 1}),
    );
    assert_eq!(standing["text"], "", "suggestions must never enter dispatch context");

    let second = ok(
        &f.engine,
        f.me(),
        "notes.append",
        json!({
            "target": "suggestions",
            "text": "While checking output, the refusal omitted the field name."
        }),
    );
    assert_eq!(second["id"], note_id);
    assert_eq!(second["body"].as_str().unwrap().lines().count(), 2);

    let notes = ok(
        &f.engine,
        Actor::User,
        "notes.list",
        json!({"project_id": 1}),
    );
    assert_eq!(
        notes["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|note| note["title"] == "Agent suggestions")
            .count(),
        1
    );
    assert_eq!(
        refusal(
            &f.engine,
            Actor::User,
            "notes.append",
            json!({"project_id": 1, "target": "suggestions", "text": "speculation"}),
        )
        .code,
        "notes.suggestion_actor"
    );
    assert_eq!(
        refusal(
            &f.engine,
            f.peer(),
            "notes.append",
            json!({"project_id": 1, "target": "suggestions", "text": "no task"}),
        )
        .code,
        "notes.suggestion_task"
    );
    assert_eq!(
        refusal(
            &f.engine,
            Actor::User,
            "notes.pin",
            json!({"note_id": note_id, "pinned": true}),
        )
        .code,
        "notes.suggestions_unpinned"
    );
}

/// `notes.list` runs unlocked (D149), as the desktop repeats it on every `notes.changed`, and
/// still scopes a bound agent to its own project.
#[test]
fn notes_list_runs_unlocked_and_keeps_an_agent_in_its_project() {
    let f = Fixture::new();
    assert!(f.engine.runs_unlocked("notes.list"));
    let other = f._root.path().join("ws/other");
    committed_repo(&other, &[("README.md", "other\n")]);
    ok(&f.engine, Actor::User, "project.add", json!({"workspace_id": 1, "path": other}));
    ok(&f.engine, Actor::User, "notes.create", json!({"project_id": 2, "title": "Elsewhere", "body": "x"}));
    ok(&f.engine, Actor::User, "notes.create", json!({"project_id": 1, "title": "Here", "body": "y"}));
    let mine = ok(&f.engine, f.me(), "notes.list", json!({"summary": true}));
    let titles: Vec<&str> = mine["notes"].as_array().unwrap().iter().filter_map(|n| n["title"].as_str()).collect();
    assert_eq!(titles, ["Here"]);
    assert_eq!(refusal(&f.engine, f.me(), "notes.list", json!({"project_id": 2})).code, "actor.scope");
    assert_eq!(ok(&f.engine, Actor::User, "notes.list", json!({"project_id": 2}))["notes"][0]["title"], "Elsewhere");
    assert_eq!(refusal(&f.engine, Actor::User, "notes.list", json!({"project_id": 9})).code, "project.not_found");
}

/// `bus.wait` is answered by the socket door, where blocking costs nothing that matters.
#[test]
fn wait_is_a_door_op_and_the_engine_says_so() {
    let f = Fixture::new();
    let direct = refusal(&f.engine, f.me(), "bus.wait", json!({}));
    assert_eq!(direct.code, "bus.door");
    assert!(direct.message.contains("socket door"));
}
