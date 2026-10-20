//! Phase 5: standing context, persistent mailbox, deterministic briefs, lifecycle reports,
//! completion, and file/symbol overlap detection. Every feature assertion crosses the bus.

mod common;

use common::{call_as as call, committed_repo, engine_with_project, ok_as as ok};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

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
        committed_repo(&repo, &[(
            "src/lib.rs",
            "pub fn shared() -> i32 { 1 }\n\npub fn untouched() -> i32 { 9 }\n",
        )]);
        let engine = engine_with_project(root.path(), &ws, &repo);
        engine.store.with_tx(|tx| {
            tx.execute(
                "INSERT INTO modules(project_id,name,created_at,updated_at) VALUES (1,'Core',?1,?1)",
                ["2026-08-17T00:00:00Z"],
            )?;
            tx.execute(
                "INSERT INTO tasks(project_id,module_id,title,body,changelog,col,state,position,created_at,updated_at)
                 VALUES (1,1,'Build awareness','Implement Phase 5','Phase 4 complete','active','dispatched',0,?1,?1)",
                ["2026-08-17T00:00:00Z"],
            )?;
            tx.execute(
                "INSERT INTO tasks(project_id,module_id,title,body,col,state,position,created_at,updated_at)
                 VALUES (1,1,'Adjacent task','Neighbor','backlog','none',1,?1,?1)",
                ["2026-08-17T00:00:00Z"],
            )?;
            Ok(())
        }).unwrap();
        let a = ok(
            &engine,
            Actor::User,
            "session.create",
            json!({
                "project_id":1, "provider":"codex", "role":"builder", "task_id":1, "module_id":1
            }),
        );
        let b = ok(
            &engine,
            Actor::User,
            "session.create",
            json!({
                "project_id":1, "provider":"claude", "role":"builder", "module_id":1
            }),
        );
        Self {
            _root: root,
            engine,
            a,
            b,
        }
    }

    fn a_name(&self) -> &str {
        self.a["name"].as_str().unwrap()
    }
    fn b_name(&self) -> &str {
        self.b["name"].as_str().unwrap()
    }
}

#[test]
fn notes_mailbox_and_brief_are_persistent_and_scoped() {
    let f = Fixture::new();
    let e = &f.engine;
    let standing = ok(
        e,
        Actor::agent(f.a_name()),
        "notes.append",
        json!({
            "project_id":1, "text":"Always run cargo fmt before completion."
        }),
    );
    assert_eq!(standing["title"], "Standing notes");
    assert_eq!(
        ok(e, Actor::User, "notes.standing", json!({"project_id":1}))["text"],
        "Always run cargo fmt before completion."
    );

    let created = ok(
        e,
        Actor::User,
        "notes.create",
        json!({"project_id":1,"title":"Design","body":"Bus first","pinned":false}),
    );
    let id = created["id"].as_i64().unwrap();
    let updated = ok(
        e,
        Actor::User,
        "notes.update",
        json!({"note_id":id,"title":null,"pinned":true}),
    );
    assert!(updated["title"].is_null());
    assert_eq!(updated["pinned"], true);

    let broadcast = ok(
        e,
        Actor::User,
        "mailbox.send",
        json!({"project_id":1,"to":"*","text":"Project context changed"}),
    );
    let message_id = broadcast["message"]["id"].as_i64().unwrap();
    // A send now says where it went, not just that it was stored (F10). Neither fixture
    // session was ever spawned, so nobody will read it until one is started.
    assert_eq!(broadcast["delivery"], "not_running", "delivery names where the message stands");
    let recipients = broadcast["recipients"].as_array().unwrap();
    let addressed: Vec<&str> = recipients.iter().map(|r| r["session"].as_str().unwrap()).collect();
    assert_eq!(addressed.len(), 2, "broadcast names every addressee: {addressed:?}");
    assert!(recipients.iter().all(|r| r["state"] == "created"), "{recipients:?}");
    let a_mail = ok(
        e,
        Actor::agent(f.a_name()),
        "mailbox.list",
        json!({"project_id":1,"unread_only":true}),
    );
    assert_eq!(a_mail["messages"].as_array().unwrap().len(), 1);
    ok(
        e,
        Actor::agent(f.a_name()),
        "mailbox.ack",
        json!({"message_id":message_id}),
    );
    assert!(ok(
        e,
        Actor::agent(f.a_name()),
        "mailbox.list",
        json!({"project_id":1,"unread_only":true})
    )["messages"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        ok(
            e,
            Actor::agent(f.b_name()),
            "mailbox.list",
            json!({"project_id":1,"unread_only":true})
        )["messages"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "ack is per recipient"
    );

    let skill = ok(
        e,
        Actor::User,
        "skill.create",
        json!({"name":"Verification","body":"Run the repository test suite before reporting completion."}),
    );
    ok(
        e,
        Actor::User,
        "skill.enable",
        json!({"skill_id":skill["id"],"project_id":1,"enabled":true}),
    );
    let brief = ok(
        e,
        Actor::agent(f.a_name()),
        "session.brief",
        json!({"session":f.a_name()}),
    );
    let text = brief["text"].as_str().unwrap();
    assert!(text.contains("Build awareness"));
    assert!(text.contains("Phase 4 complete"));
    // Notes are not injected: an agent works from its task and reads a note when it asks (D166).
    assert!(!text.contains("Always run cargo fmt"), "the standing note leaked into the brief");
    assert!(!text.contains("Standing notes"), "the brief still has a notes section");
    assert!(brief["parts"].get("notes").is_none(), "the brief still has a notes part");
    assert!(text.contains(f.b_name()));
    assert!(text.contains("Adjacent task"));
    assert!(brief["parts"]["skills"].as_str().unwrap().contains("### Verification"));
    assert!(text.contains("Run the repository test suite"));
    // The injected half names the skill and the folder it is registered in, not just a path
    // to the bodies: an agent has to know a skill exists before it can decide to load it.
    let compact = brief["compact"].as_str().unwrap();
    assert!(compact.contains("Verification"), "the injected half never names the skill");
    assert!(compact.contains(".claude/skills/verification/"), "no registered location: {compact}");
    assert!(compact.contains(".relay/session-skills.md"), "the bodies are still reachable");
    assert!(!compact.contains("Run the repository test suite"), "bodies stay out of the prompt (D101)");
    assert!(!compact.contains("Always run cargo fmt"), "the standing note leaked into the injected half");

    let bootstrap = ok(
        e,
        Actor::agent(f.a_name()),
        "session.bootstrap",
        json!({}),
    );
    assert_eq!(bootstrap["session"], f.a_name());
    assert_eq!(bootstrap["role"], "builder");
    assert_eq!(bootstrap["project_id"], 1);
    assert_eq!(bootstrap["project"], "app");
    assert_eq!(bootstrap["base_branch"], "main");
    assert_eq!(bootstrap["module"], "Core");
    assert_eq!(bootstrap["task"]["id"], 1);
    assert_eq!(bootstrap["task"]["title"], "Build awareness");
    assert_eq!(bootstrap["task"]["body"], "Implement Phase 5");
    assert_eq!(bootstrap["task"]["changelog"], "Phase 4 complete");
    // F8: the one call every agent makes answers "who else is here and what may I do?".
    // Skill bodies still live in the brief, not here; notes are in neither.
    let peer_names: Vec<&str> = bootstrap["peers"].as_array().unwrap().iter()
        .map(|peer| peer["session"].as_str().unwrap()).collect();
    assert!(peer_names.contains(&f.b_name()), "bootstrap hides the live peers: {peer_names:?}");
    assert!(!peer_names.contains(&f.a_name()), "bootstrap lists the caller as its own peer");
    assert_eq!(bootstrap["brief_path"], format!(".relay/sessions/{}/session-brief.md", f.a_name()));
    assert!(bootstrap["comms"].as_str().unwrap().contains("mailbox.send"));
    assert!(bootstrap["comms"].as_str().unwrap().contains("{\"to\":\"*\",\"text\":\"...\"}"));
    assert!(bootstrap["discovery"].as_str().unwrap().contains("bus.ops"));
    assert!(bootstrap["discovery"].as_str().unwrap().contains("bus.schema"));
    assert!(bootstrap["discovery"].as_str().unwrap().contains("$RELAY_BIN schema <op>"));
    assert!(bootstrap["discovery"].as_str().unwrap().contains("project_id"));
    assert!(bootstrap["discovery"].as_str().unwrap().contains("omit `task` and `task_id`"));
    let can_call: Vec<&str> = bootstrap["can_call"].as_array().unwrap().iter()
        .map(|op| op.as_str().unwrap()).collect();
    assert!(can_call.contains(&"mailbox.send"), "a builder may send mail");
    assert!(can_call.contains(&"session.claim"), "a builder may claim files");
    assert!(can_call.contains(&"session.intent"), "a builder may publish its intent");
    assert!(can_call.contains(&"session.release"), "a builder may release files");
    assert!(can_call.contains(&"session.done"), "a builder may report completion");
    assert!(!can_call.contains(&"task.create"), "task.create is outside the builder allowlist");
    assert!(!can_call.contains(&"session.spawn"), "session.spawn is user-only");
    assert!(bootstrap["guardrails"]["caps"]["files"].is_number());
    assert_eq!(bootstrap["guardrails"]["dry_run"], "guardrail.check");
    let roots: Vec<&str> = bootstrap["guardrails"]["write_roots"].as_array().unwrap().iter()
        .map(|root| root.as_str().unwrap()).collect();
    assert_eq!(roots.first().copied(), f.a["worktree"].as_str(), "the worktree is the first write root");
    assert!(bootstrap.get("notes").is_none());
    assert!(bootstrap.get("skills").is_none());

    // One launch may stage any number of board tasks without spawning halfway through. The
    // compatibility `task` is the current one and the complete queue reaches both channels.
    let staged = ok(e, Actor::User, "task.dispatch", json!({
        "task_id":2,"session":f.a_name(),"start":false
    }));
    assert_eq!(staged["session"]["state"], "created");
    let multi = ok(e, Actor::agent(f.a_name()), "session.bootstrap", json!({}));
    let task_ids: Vec<i64> = multi["tasks"].as_array().unwrap().iter().map(|task| task["id"].as_i64().unwrap()).collect();
    assert_eq!(task_ids, [1, 2]);
    assert_eq!(multi["task"]["id"], 1, "the current task stays compatible with scalar clients");
    let multi_brief = ok(e, Actor::agent(f.a_name()), "session.brief", json!({"session":f.a_name()}));
    assert!(multi_brief["text"].as_str().unwrap().contains("assigned_tasks: 2"));
    assert!(multi_brief["text"].as_str().unwrap().contains("current_task: #1"));
    assert!(multi_brief["text"].as_str().unwrap().contains("Task #2 [QUEUED]: Adjacent task"));
    assert!(multi_brief["text"].as_str().unwrap().contains("Task #1 [CURRENT]: Build awareness"));
    // A queued task moved back to the backlog is announced as parked, not as in review.
    ok(e, Actor::User, "task.move", json!({"task_id":2,"column":"backlog"}));
    let parked = ok(e, Actor::agent(f.a_name()), "session.brief", json!({"session":f.a_name()}));
    assert!(parked["text"].as_str().unwrap().contains("Task #2 [BACKLOG]: Adjacent task"), "{}", parked["text"]);
}

#[test]
fn overlap_scan_detects_shared_file_and_rust_symbol_then_acknowledges() {
    let f = Fixture::new();
    let a_wt = PathBuf::from(f.a["worktree"].as_str().unwrap());
    let b_wt = PathBuf::from(f.b["worktree"].as_str().unwrap());
    std::fs::write(
        a_wt.join("src/lib.rs"),
        "pub fn shared() -> i32 { 2 }\n\npub fn untouched() -> i32 { 9 }\n",
    )
    .unwrap();
    std::fs::write(
        b_wt.join("src/lib.rs"),
        "pub fn shared() -> i32 { 3 }\n\npub fn untouched() -> i32 { 9 }\n",
    )
    .unwrap();

    let scanned = ok(
        &f.engine,
        Actor::User,
        "overlap.scan",
        json!({"project_id":1}),
    );
    let overlaps = scanned["overlaps"].as_array().unwrap();
    assert!(overlaps
        .iter()
        .any(|o| o["kind"] == "file" && o["path"] == "src/lib.rs"));
    let symbol = overlaps
        .iter()
        .find(|o| o["kind"] == "symbol" && o["symbol"] == "shared")
        .expect("shared symbol overlap");
    assert!(!overlaps
        .iter()
        .any(|o| o["kind"] == "symbol" && o["symbol"] == "untouched"));
    let id = symbol["id"].as_i64().unwrap();
    let acked = ok(
        &f.engine,
        Actor::agent(f.a_name()),
        "overlap.ack",
        json!({"overlap_id":id}),
    );
    assert!(acked["acked_by"]
        .as_array()
        .unwrap()
        .iter()
        .any(|name| name == f.a_name()));

    let claim = ok(
        &f.engine,
        Actor::agent(f.a_name()),
        "overlap.flag",
        json!({
            "project_id":1,"path":"src/lib.rs","symbol":"shared","note":"editing return contract"
        }),
    );
    assert_eq!(claim["kind"], "claim");
    let peers = ok(
        &f.engine,
        Actor::agent(f.b_name()),
        "session.peers",
        json!({"session":f.b_name()}),
    );
    let a = peers["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["session"] == f.a_name())
        .unwrap();
    assert!(a["claimed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|claim| claim == "src/lib.rs#shared"));

    // Durable rows from a closed session must not keep every future scan in conflict. Older
    // stores may contain these even though current session.close deletes claims eagerly.
    f.engine.store.with_tx(|tx| {
        tx.execute(
            "UPDATE sessions SET state='closed', closed_at=?1 WHERE name=?2",
            ["2026-08-20T00:00:00Z", f.a_name()],
        )?;
        Ok(())
    }).unwrap();
    let rescanned = ok(&f.engine, Actor::User, "overlap.scan", json!({"project_id":1}));
    assert!(rescanned["overlaps"].as_array().unwrap().iter().all(|overlap| {
        !overlap["sessions"].as_array().unwrap().iter().any(|name| name == f.a_name())
    }), "closed-session claims remain active: {}", rescanned["overlaps"]);
}

#[test]
fn lifecycle_report_and_done_update_session_task_notifications_and_mailbox() {
    let f = Fixture::new();
    let e = &f.engine;
    ok(
        e,
        Actor::agent(f.a_name()),
        "session.report",
        json!({
            "session":f.a_name(), "kind":"session_start", "data":{"session_id":"provider-42"}
        }),
    );
    let running = ok(
        e,
        Actor::agent(f.a_name()),
        "session.get",
        json!({"session":f.a_name()}),
    );
    assert_eq!(running["state"], "running");
    assert_eq!(running["provider_ref"], "provider-42");
    // The ref is replayed into the provider's argv on resume, so an option-shaped one is refused
    // and the stored ref is left as it was.
    let refused = call(
        e,
        Actor::agent(f.a_name()),
        "session.report",
        json!({"session":f.a_name(), "kind":"tool_use", "data":{"provider_ref":"--dangerously-skip-permissions"}}),
    );
    assert_eq!(refused.error.expect("an option-shaped provider_ref is refused").code, "session.provider_ref");
    assert_eq!(
        ok(e, Actor::agent(f.a_name()), "session.get", json!({"session":f.a_name()}))["provider_ref"],
        "provider-42"
    );
    ok(
        e,
        Actor::agent(f.a_name()),
        "session.report",
        json!({
            "session":f.a_name(), "kind":"blocked", "data":{"message":"Need an API decision"}
        }),
    );
    assert_eq!(
        ok(
            e,
            Actor::agent(f.a_name()),
            "session.get",
            json!({"session":f.a_name()})
        )["state"],
        "blocked"
    );

    let done = ok(
        e,
        Actor::agent(f.a_name()),
        "session.done",
        json!({
            "session":f.a_name(), "summary":"Awareness implemented", "sha":"abc123"
        }),
    );
    assert_eq!(done["state"], "idle");
    let conn = e.store.lock();
    let task: (String, String) = conn
        .query_row("SELECT col,state FROM tasks WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(task, ("in_review".into(), "awaiting_review".into()));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE category='agent_blocked'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE category='agent_done'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT sha FROM task_commits WHERE task_id=1", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "abc123"
    );
    drop(conn);
    let peer_mail = ok(
        e,
        Actor::agent(f.b_name()),
        "mailbox.list",
        json!({"project_id":1}),
    );
    let texts = peer_mail["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(texts.iter().any(|text| text.contains("blocked")));
    assert!(texts.iter().any(|text| text.contains("finished")));
}

/// Task 30: an agent that just stops — the common case, since `session.done` is optional —
/// must still reach the notification centre, and the Stop hook that trails `session.done`
/// must not double-report.
#[test]
fn a_session_falling_idle_notifies_once_whether_or_not_it_reported_done() {
    let f = Fixture::new();
    let e = &f.engine;
    let count = |category: &str| {
        f.engine
            .store
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE category=?1",
                [category],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    let report = |name: &str, kind: &str| {
        ok(
            e,
            Actor::agent(name),
            "session.report",
            json!({"session":name, "kind":kind}),
        );
    };

    // A: works, then simply stops. No session.done anywhere.
    report(f.a_name(), "session_start");
    report(f.a_name(), "tool_use");
    assert_eq!(count("agent_done"), 0);
    report(f.a_name(), "stop");
    assert_eq!(
        ok(
            e,
            Actor::agent(f.a_name()),
            "session.get",
            json!({"session":f.a_name()})
        )["state"],
        "idle"
    );
    assert_eq!(count("agent_done"), 1, "a bare Stop hook must notify");
    let link: String = f
        .engine
        .store
        .lock()
        .query_row(
            "SELECT link FROM notifications WHERE category='agent_done'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(link.contains("task.get"), "link points at the task: {link}");

    // A repeated stop while already idle is not a second completion.
    report(f.a_name(), "stop");
    assert_eq!(count("agent_done"), 1);

    // B: reports done, then its Stop hook fires. Exactly one notification for the pair.
    report(f.b_name(), "session_start");
    ok(
        e,
        Actor::agent(f.b_name()),
        "session.done",
        json!({"session":f.b_name(), "summary":"Reviewed"}),
    );
    assert_eq!(count("agent_done"), 2);
    report(f.b_name(), "stop");
    assert_eq!(
        count("agent_done"),
        2,
        "session.done already parked the session at idle"
    );
}

#[test]
fn lifecycle_stop_notifies_once_when_agent_omits_done() {
    let f = Fixture::new();
    let e = &f.engine;
    ok(e, Actor::agent(f.a_name()), "session.report", json!({"session":f.a_name(),"kind":"session_start"}));
    ok(e, Actor::agent(f.a_name()), "session.report", json!({
        "session":f.a_name(), "kind":"stop",
        "data":{"type":"agent-turn-complete","last-assistant-message":"Focused checks passed"}
    }));
    assert_eq!(ok(e, Actor::agent(f.a_name()), "session.get", json!({"session":f.a_name()}))["state"], "idle");
    let conn = e.store.lock();
    let notifications: Vec<(String, String)> = conn.prepare(
        "SELECT title,body FROM notifications WHERE category='agent_done' ORDER BY id",
    ).unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap()
        .collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(notifications, vec![(format!("{} finished", f.a_name()), "Focused checks passed".into())]);
    drop(conn);

    ok(e, Actor::agent(f.a_name()), "session.report", json!({
        "session":f.a_name(), "kind":"stop", "data":{"last-assistant-message":"duplicate"}
    }));
    assert_eq!(e.store.lock().query_row(
     "SELECT COUNT(*) FROM notifications WHERE category='agent_done'", [], |row| row.get::<_, i64>(0),
    ).unwrap(), 1);
}

#[test]
fn repeated_reports_coalesce_unread_cards_and_do_not_replay_alerts() {
    let f = Fixture::new();
    let mut events = f.engine.subscribe();
    for pass in 0..6 {
        ok(&f.engine, Actor::agent(f.b_name()), "session.report", json!({"session":f.b_name(),"kind":"tool_use"}));
        ok(&f.engine, Actor::agent(f.b_name()), "session.done", json!({"session":f.b_name(),"summary":format!("Result {pass}")}));
        ok(&f.engine, Actor::agent(f.b_name()), "session.report", json!({"session":f.b_name(),"kind":"tool_use"}));
        ok(&f.engine, Actor::agent(f.b_name()), "session.report", json!({"session":f.b_name(),"kind":"stop"}));
    }
    let mut alerts = 0;
    let mut broadcasts = 0;
    while let Ok(event) = events.try_recv() {
        alerts += usize::from(event.ev == "notify.new");
        broadcasts += usize::from(event.ev == "mailbox.new");
    }
    assert_eq!((alerts, broadcasts), (1, 1));
    let cards = ok(&f.engine, Actor::User, "notify.list", json!({"unread_only":true}));
    assert_eq!(cards["notifications"].as_array().unwrap().len(), 1);
    assert_eq!(cards["notifications"][0]["body"], "Result 5");
    ok(&f.engine, Actor::User, "notify.ack", json!({"notification_id":cards["notifications"][0]["id"]}));
    ok(&f.engine, Actor::agent(f.b_name()), "session.report", json!({"session":f.b_name(),"kind":"tool_use"}));
    ok(&f.engine, Actor::agent(f.b_name()), "session.report", json!({"session":f.b_name(),"kind":"stop","data":{"message":"New work finished"}}));
    let unread = ok(&f.engine, Actor::User, "notify.list", json!({"unread_only":true}));
    assert_eq!(unread["notifications"].as_array().unwrap().len(), 1);
    assert_eq!(unread["notifications"][0]["body"], "New work finished");
}

#[test]
fn mailbox_list_is_paged_newest_first_for_people_and_oldest_unread_for_agents() {
    let f = Fixture::new();
    let e = &f.engine;
    for n in 0..250 {
        ok(e, Actor::User, "mailbox.send", json!({"project_id":1,"to":f.a_name(),"text":format!("m{n}")}));
    }
    let texts = |page: &Value| -> Vec<String> {
        page["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap().to_string()).collect()
    };
    // The person's view: the newest page, in reading order, with a cursor to the older ones.
    let newest = ok(e, Actor::User, "mailbox.list", json!({"project_id":1}));
    let page = texts(&newest);
    assert_eq!(page.len(), 200);
    assert_eq!((page[0].as_str(), page[199].as_str()), ("m50", "m249"));
    let older = ok(e, Actor::User, "mailbox.list", json!({"project_id":1,"before":newest["next_before"]}));
    let page = texts(&older);
    assert_eq!((page.len(), page[0].as_str(), page[49].as_str()), (50, "m0", "m49"));
    assert!(older.get("next_before").is_none());

    // An agent working through its inbox gets the oldest unread first, and is told more wait.
    let inbox = ok(e, Actor::agent(f.a_name()), "mailbox.list", json!({"project_id":1,"unread_only":true,"limit":100}));
    assert_eq!(texts(&inbox)[0], "m0");
    assert_eq!(inbox["more_unread"], true);

    // Text is bounded: a person's message has a ceiling.
    let long = "x".repeat(70 * 1024);
    let refused = call(e, Actor::User, "mailbox.send", json!({"project_id":1,"to":f.a_name(),"text":long}));
    assert_eq!(refused.error.unwrap().code, "mailbox.text");
}

/// RA-385: an oversized brief section is cut, and says where the whole of it is.
#[test]
fn an_oversized_brief_section_is_cut_with_a_pointer_to_the_full_list() {
    let f = Fixture::new();
    let e = &f.engine;
    let long_note: String = (0..2000).map(|i| format!("standing rule number {i}\n")).collect();
    e.store.with_tx(|tx| {
        for i in 0..100 {
            tx.execute(
                "INSERT INTO tasks(project_id,module_id,title,body,col,state,position,created_at,updated_at)
                 VALUES (1,1,?1,'','backlog','none',?2,'t','t')",
                rusqlite::params![format!("Neighbour {i}"), 10 + i],
            )?;
        }
        tx.execute(
            "INSERT INTO notes(project_id,title,body,standing,created_at,updated_at) VALUES (1,'Standing notes',?1,1,'t','t')",
            [&long_note],
        )?;
        Ok(())
    }).unwrap();
    let brief = ok(e, Actor::agent(f.a_name()), "session.brief", json!({"session":f.a_name()}));
    let adjacent = brief["parts"]["adjacent"].as_str().unwrap();
    assert!(adjacent.lines().count() <= 41, "{adjacent}");
    assert!(adjacent.ends_with("not shown here; the full list: `task.list {\"module_id\": 1}`"), "{adjacent}");
    // However long the standing note, none of it rides in the brief (D166).
    assert!(brief["parts"].get("notes").is_none());
    assert!(!brief["text"].as_str().unwrap().contains("standing rule number"));
    // The compact half carries the same cut sections.
    assert!(brief["compact"].as_str().unwrap().contains("the full list: `task.list"));
    // A section within its budget is whole and says nothing more.
    assert!(!brief["parts"]["peers"].as_str().unwrap().contains("full list"));
}

/// A session can sit for hours between launch and its first turn while the base moves on, and
/// the brief's counts are from launch. Bootstrap reads the refs again at the call.
#[test]
fn bootstrap_says_how_far_the_checkout_is_behind_the_base_now() {
    let f = Fixture::new();
    let e = &f.engine;
    let current = ok(e, Actor::agent(f.a_name()), "session.bootstrap", json!({}));
    assert_eq!(current["base_sync"]["base"], "main");
    assert_eq!(current["base_sync"]["behind"], 0);
    assert_eq!(current["base_sync"]["ahead"], 0);
    assert!(current["base_sync"]["hint"].as_str().unwrap().starts_with("Up to date with main."));

    let repo = f._root.path().join("ws").join("app");
    common::git(&repo, &["commit", "-q", "--allow-empty", "-m", "merged while the session waited"]);
    let tip = common::git(&repo, &["rev-parse", "main"]);
    let behind = ok(e, Actor::agent(f.a_name()), "session.bootstrap", json!({}));
    assert_eq!(behind["base_sync"]["behind"], 1);
    assert_eq!(behind["base_sync"]["ahead"], 0);
    assert_eq!(behind["base_sync"]["head"], tip.as_str());
    let hint = behind["base_sync"]["hint"].as_str().unwrap();
    assert!(hint.contains("1 commit behind main"), "{hint}");
    assert!(hint.contains("`git merge --ff-only main`"), "{hint}");
    // session.create fetched a moment ago, so the count needs no new fetch.
    assert!(behind["base_sync"]["fetched_secs_ago"].as_u64().unwrap() < 3600);
    assert!(!hint.contains("git fetch"), "{hint}");
    // A fetch hours old is worth repeating before the count is trusted.
    let touched = std::process::Command::new("touch")
        .args(["-d", "2 hours ago"]).arg(repo.join(".git").join("FETCH_HEAD")).status().unwrap();
    assert!(touched.success());
    let stale = ok(e, Actor::agent(f.a_name()), "session.bootstrap", json!({}));
    assert!(stale["base_sync"]["fetched_secs_ago"].as_u64().unwrap() >= 7200);
    assert!(stale["base_sync"]["hint"].as_str().unwrap().contains("`git fetch` first"));

    let worktree = PathBuf::from(f.a["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("own.txt"), "own work\n").unwrap();
    common::git(&worktree, &["add", "own.txt"]);
    // The session's checkout carries Relay's commit hooks, which want a live engine socket.
    common::git(&worktree, &["-c", "core.hooksPath=/dev/null", "commit", "-q", "-m", "own work"]);
    let diverged = ok(e, Actor::agent(f.a_name()), "session.bootstrap", json!({}));
    assert_eq!(diverged["base_sync"]["behind"], 1);
    assert_eq!(diverged["base_sync"]["ahead"], 1);
    let hint = diverged["base_sync"]["hint"].as_str().unwrap();
    assert!(hint.contains("has 1 commit of its own. Merge main into it"), "{hint}");
}
