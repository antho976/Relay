//! The rebuilt board (task #31): sub-tasks with roll-up, first-class types, labels, and
//! stored blocked-by / duplicate-of edges. Every assertion crosses the bus door, because the
//! CLI and agents reach this model through exactly the same ops the UI does.

mod common;

use common::{call_as as call, code, committed_repo, engine_with_project, ok};
use relay_bus::Actor;
use relay_core::engine::Engine;
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

fn ids(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect()
}

/// A provider that answers the discovery probe and then echoes, so a dispatch can reach a PTY
/// without the machine having a real one.
///
/// `task.dispatch` refuses with `provider.not_installed` unless a provider resolves, and
/// resolving it from `PATH` means the test passes or fails on what happens to be installed —
/// it passed on developer machines and on agent containers, where `claude` is present, and
/// failed the first time CI ran it anywhere else. `sessions.rs` already fakes providers this
/// way; the board should not be the one file that does not.
fn fake_provider(dir: &Path, binary: &str) -> std::path::PathBuf {
    let path = dir.join(binary);
    std::fs::write(
        &path,
        "#!/bin/sh\ncase \"${1:-}\" in\n  --version) echo 'fixture 1.0'; exit 0;;\n  auth|login) echo '{\"loggedIn\":true}'; exit 0;;\nesac\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; done\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        let repo = ws.join("app");
        committed_repo(&repo, &[("README.md", "board\n")]);
        let engine = engine_with_project(root.path(), &ws, &repo);
        let claude = fake_provider(root.path(), "claude");
        ok(&engine, "settings.set", json!({"path":"providers.claude.path","value":claude}));
        Self {
            _root: root,
            engine,
        }
    }
    fn task(&self, title: &str, extra: Value) -> Value {
        let mut payload = json!({"project_id":1,"title":title});
        let map = payload.as_object_mut().unwrap();
        for (k, v) in extra.as_object().unwrap() {
            map.insert(k.clone(), v.clone());
        }
        ok(&self.engine, "task.create", payload)
    }
}

#[test]
fn task_note_and_module_draft_expectations_reject_stale_writes_atomically() {
    let f = Fixture::new();
    for (kind, id_key, field, original) in [
        (
            "task",
            "task_id",
            "body",
            f.task("Draft", json!({"body":"original"})),
        ),
        (
            "notes",
            "note_id",
            "body",
            ok(
                &f.engine,
                "notes.create",
                json!({"project_id":1,"body":"original"}),
            ),
        ),
        (
            "module",
            "module_id",
            "name",
            ok(
                &f.engine,
                "module.create",
                json!({"project_id":1,"name":"original"}),
            ),
        ),
    ] {
        let op = format!("{kind}.update");
        let gate = Arc::new(std::sync::Barrier::new(3));
        let mut writers = Vec::new();
        for body in ["first writer", "second writer"] {
            let engine = f.engine.clone();
            let gate = gate.clone();
            let op = op.clone();
            let payload = json!({id_key:original["id"],field:body,"expected":{field:"original"}});
            writers.push(std::thread::spawn(move || {
                gate.wait();
                call(&engine, Actor::User, &op, payload)
            }));
        }
        gate.wait();
        let replies: Vec<_> = writers.into_iter().map(|w| w.join().unwrap()).collect();
        assert_eq!(replies.iter().filter(|r| r.ok).count(), 1);
        assert_eq!(
            replies
                .iter()
                .find(|r| !r.ok)
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .code,
            format!("{kind}.edit_conflict")
        );
        let winner = &replies
            .iter()
            .find(|r| r.ok)
            .unwrap()
            .result
            .as_ref()
            .unwrap()[field];
        assert_eq!(
            ok(
                &f.engine,
                &format!("{kind}.get"),
                json!({id_key:original["id"]})
            )[field],
            *winner
        );

        // A legacy caller can still patch, while a stale editor cannot overwrite its change.
        ok(
            &f.engine,
            &op,
            json!({id_key:original["id"],field:"legacy edit"}),
        );
        assert_eq!(
            code(call(
                &f.engine,
                Actor::User,
                &op,
                json!({id_key:original["id"],field:"stale draft","expected":{field:winner}})
            )),
            format!("{kind}.edit_conflict")
        );
        assert_eq!(
            code(call(
                &f.engine,
                Actor::User,
                &op,
                json!({id_key:original["id"],"expected":{"typo":null}})
            )),
            format!("{kind}.expected_field")
        );
    }
}

#[test]
fn draft_expectations_preserve_nulls_and_ignore_unmentioned_fields() {
    let f = Fixture::new();
    let task = f.task("Draft", json!({}));
    ok(
        &f.engine,
        "task.update",
        json!({"task_id":task["id"],"priority":"high"}),
    );
    let updated = ok(
        &f.engine,
        "task.update",
        json!({"task_id":task["id"],"body":"edited","expected":{"body":"","size":null,"module_id":null,"type":task["type"]}}),
    );
    assert_eq!(updated["priority"], "high");
    let note = ok(
        &f.engine,
        "notes.create",
        json!({"project_id":1,"body":"original"}),
    );
    assert!(note["title"].is_null());
    let updated = ok(
        &f.engine,
        "notes.update",
        json!({"note_id":note["id"],"title":"Named","expected":{"title":null,"pinned":false}}),
    );
    assert_eq!(updated["title"], "Named");
    assert_eq!(
        code(call(
            &f.engine,
            Actor::User,
            "notes.update",
            json!({"note_id":note["id"],"title":null,"expected":{"title":null}})
        )),
        "notes.edit_conflict"
    );
}

#[test]
fn sub_tasks_roll_up_and_stay_real_cards() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Rebuild the board", json!({"type":"feature"}));
    let child_a = f.task("Model", json!({"parent_id":parent["id"],"type":"chore"}));
    let child_b = f.task("Board", json!({"parent_id":parent["id"]}));
    let grandchild = f.task("Card", json!({"parent_id":child_b["id"]}));

    assert_eq!(child_a["depth"], 1);
    assert_eq!(grandchild["depth"], 2);
    assert_eq!(grandchild["parent_id"], child_b["id"]);

    // A sub-task is a task: it has its own column and its own card.
    assert_eq!(child_a["column"], "backlog");
    let listed = ok(e, "task.list", json!({"project_id":1}))["tasks"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(listed, 4, "children are listed alongside their parent");

    // Roll-up counts the whole subtree, not just the direct children.
    let reread = ok(e, "task.get", json!({"task_id":parent["id"]}));
    assert_eq!(reread["rollup"]["total"], 3);
    assert_eq!(reread["rollup"]["done"], 0);
    assert_eq!(ids(&reread["children"]), vec![child_a["id"].as_i64().unwrap(), child_b["id"].as_i64().unwrap()]);

    ok(e, "task.move", json!({"task_id":grandchild["id"],"column":"in_review"}));
    ok(e, "task.approve", json!({"task_id":grandchild["id"],"sha":"deadbeef"}));
    let reread = ok(e, "task.get", json!({"task_id":parent["id"]}));
    assert_eq!(reread["rollup"], json!({"total":3,"done":1}));

    // Filtering by parent is how a group is pulled out of the columns.
    let direct = ok(e, "task.children", json!({"task_id":parent["id"]}))["tasks"].clone();
    assert_eq!(direct.as_array().unwrap().len(), 2);
    let whole = ok(
        e,
        "task.children",
        json!({"task_id":parent["id"],"recursive":true}),
    )["tasks"]
        .clone();
    assert_eq!(whole.as_array().unwrap().len(), 3);
    let roots = ok(e, "task.list", json!({"project_id":1,"parent_id":null}))["tasks"].clone();
    assert_eq!(roots.as_array().unwrap().len(), 1);
    assert_eq!(roots[0]["id"], parent["id"]);
}

/// A page's roll-ups and depths are walked over one read of the task tree; each must read
/// exactly as `task.get` walks it for one task, with done, deleted and nested sub-tasks.
#[test]
fn listed_roll_ups_and_depths_match_each_task() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = f.task("A", json!({}));
    let b = f.task("B", json!({"parent_id":a["id"]}));
    let c = f.task("C", json!({"parent_id":b["id"]}));
    let d = f.task("D", json!({"parent_id":a["id"]}));
    let gone = f.task("Gone", json!({"parent_id":b["id"]}));
    let other = f.task("Other", json!({}));
    f.task("Other child", json!({"parent_id":other["id"]}));
    f.task("Loose", json!({}));
    ok(e, "task.move", json!({"task_id":c["id"],"column":"in_review"}));
    ok(e, "task.approve", json!({"task_id":c["id"],"sha":"deadbeef"}));
    ok(e, "task.move", json!({"task_id":d["id"],"column":"in_review"}));
    ok(e, "task.approve", json!({"task_id":d["id"],"sha":"deadbeef"}));
    ok(e, "task.delete", json!({"task_id":gone["id"]}));

    let listed = ok(e, "task.list", json!({"project_id":1,"summary":true}))["tasks"].clone();
    let listed = listed.as_array().unwrap();
    assert_eq!(listed.len(), 7);
    for task in listed {
        let one = ok(e, "task.get", json!({"task_id":task["id"]}));
        assert_eq!(task["rollup"], one["rollup"], "roll-up of #{}", task["id"]);
        assert_eq!(task["depth"], one["depth"], "depth of #{}", task["id"]);
        assert_eq!(task["children"], one["children"], "children of #{}", task["id"]);
    }
    let of = |id: &Value| listed.iter().find(|t| t["id"] == *id).unwrap().clone();
    assert_eq!(of(&a["id"])["rollup"], json!({"total":3,"done":2}));
    assert_eq!(of(&b["id"])["rollup"], json!({"total":1,"done":1}));
    assert_eq!(of(&c["id"])["depth"], 2);
    // A summary leaves out the essays and nothing else.
    assert_eq!(of(&a["id"])["body"], "");
    assert_eq!(of(&a["id"])["title"], "A");
}

#[test]
fn nesting_is_capped_and_cycles_refused() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = f.task("A", json!({}));
    let b = f.task("B", json!({"parent_id":a["id"]}));
    let c = f.task("C", json!({"parent_id":b["id"]}));

    // TASK_DEPTH_MAX is 3: a root plus two levels. The fourth is refused, not silently flattened.
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.create",
            json!({"project_id":1,"title":"D","parent_id":c["id"]})
        )),
        "task.depth"
    );
    // Re-parenting checks the height of the subtree that would move, not just the task itself.
    // `b` is one level tall, so hanging it under `a`'s child would push `c` past the cap.
    let d = f.task("D", json!({"parent_id":a["id"]}));
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.parent.set",
            json!({"task_id":b["id"],"parent_id":d["id"]})
        )),
        "task.depth"
    );
    // And a task cannot be re-parented under its own descendant.
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.parent.set",
            json!({"task_id":a["id"],"parent_id":c["id"]})
        )),
        "task.parent_cycle"
    );
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.parent.set",
            json!({"task_id":a["id"],"parent_id":a["id"]})
        )),
        "task.parent_cycle"
    );
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.parent.set",
            json!({"task_id":a["id"],"parent_id":b["id"]})
        )),
        "task.parent_cycle"
    );
}

#[test]
fn promote_and_detach_survive_undo() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Group", json!({}));
    let loose = f.task("Loose", json!({}));

    let promoted = ok(
        e,
        "task.parent.set",
        json!({"task_id":loose["id"],"parent_id":parent["id"]}),
    );
    assert_eq!(promoted["parent_id"], parent["id"]);
    assert_eq!(
        ok(e, "task.get", json!({"task_id":parent["id"]}))["rollup"]["total"],
        1
    );

    let row = ok(e, "audit.list", json!({"op_prefix":"task.parent.set","limit":1}))["rows"][0].clone();
    ok(e, "audit.undo", json!({"audit_id":row["id"]}));
    let back = ok(e, "task.get", json!({"task_id":loose["id"]}));
    assert!(back["parent_id"].is_null());
    assert_eq!(
        ok(e, "task.get", json!({"task_id":parent["id"]}))["rollup"]["total"],
        0
    );
}

#[test]
fn types_and_labels_are_separate_layers() {
    let f = Fixture::new();
    let e = &f.engine;
    let bug = f.task("Crash on launch", json!({"type":"bug","labels":["ui","regression"]}));
    let spike = f.task("Try gix", json!({"type":"spike"}));
    f.task("Plain", json!({}));

    assert_eq!(bug["type"], "bug");
    assert_eq!(spike["type"], "spike");
    assert_eq!(
        ok(e, "task.get", json!({"task_id":bug["id"]}))["type"],
        "bug"
    );
    // No type given = the neutral default, never an invented classification.
    assert_eq!(
        ok(e, "task.list", json!({"project_id":1,"type":"task"}))["tasks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(bug["labels"], json!(["regression", "ui"]));

    let labels = ok(e, "task.label.list", json!({"project_id":1}))["labels"].clone();
    assert_eq!(labels.as_array().unwrap().len(), 2);
    assert_eq!(labels[0]["name"], "regression");

    let tagged = ok(
        e,
        "task.label.add",
        json!({"task_id":spike["id"],"label":"UI"}),
    );
    // The stored name wins, so one label reads one way everywhere on the board.
    assert_eq!(tagged["labels"], json!(["ui"]));
    // Label names are case-insensitive, so "UI" reuses the existing row rather than forking it.
    assert_eq!(
        ok(e, "task.label.list", json!({"project_id":1}))["labels"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        ok(e, "task.list", json!({"project_id":1,"label":"ui"}))["tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let untagged = ok(
        e,
        "task.label.remove",
        json!({"task_id":spike["id"],"label":"ui"}),
    );
    assert_eq!(untagged["labels"], json!([]));
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.label.add",
            json!({"task_id":spike["id"],"label":"   "})
        )),
        "task.label"
    );
    // Changing a type is a normal patch, and it round-trips through undo like every other field.
    ok(e, "task.update", json!({"task_id":spike["id"],"type":"chore"}));
    let row = ok(e, "audit.list", json!({"op_prefix":"task.update","limit":1}))["rows"][0].clone();
    ok(e, "audit.undo", json!({"audit_id":row["id"]}));
    assert_eq!(
        ok(e, "task.get", json!({"task_id":spike["id"]}))["type"],
        "spike"
    );
}

#[test]
fn blocked_by_is_stored_both_ways_and_duplicate_of_is_single_valued() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = f.task("Ship the card", json!({}));
    let b = f.task("Ship the model", json!({}));
    let c = f.task("Older report", json!({}));

    let blocked = ok(
        e,
        "task.relate",
        json!({"task_id":a["id"],"relation":"blocked_by","other_id":b["id"]}),
    );
    assert_eq!(ids(&blocked["blocked_by"]), vec![b["id"].as_i64().unwrap()]);
    // The inverse is a read of the same row — `blocks` is never stored separately.
    assert_eq!(
        ids(&ok(e, "task.get", json!({"task_id":b["id"]}))["blocks"]),
        vec![a["id"].as_i64().unwrap()]
    );

    ok(
        e,
        "task.relate",
        json!({"task_id":a["id"],"relation":"duplicate_of","other_id":c["id"]}),
    );
    let redirected = ok(
        e,
        "task.relate",
        json!({"task_id":a["id"],"relation":"duplicate_of","other_id":b["id"]}),
    );
    assert_eq!(redirected["duplicate_of"], b["id"]);

    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.relate",
            json!({"task_id":a["id"],"relation":"blocked_by","other_id":a["id"]})
        )),
        "task.relation_self"
    );

    let dropped = ok(
        e,
        "task.unrelate",
        json!({"task_id":a["id"],"relation":"blocked_by","other_id":b["id"]}),
    );
    assert_eq!(dropped["blocked_by"], json!([]));
    let row = ok(e, "audit.list", json!({"op_prefix":"task.unrelate","limit":1}))["rows"][0].clone();
    ok(e, "audit.undo", json!({"audit_id":row["id"]}));
    assert_eq!(
        ids(&ok(e, "task.get", json!({"task_id":a["id"]}))["blocked_by"]),
        vec![b["id"].as_i64().unwrap()]
    );
    // Deleting the other end hides the edge without losing it; restoring brings it back.
    ok(e, "task.delete", json!({"task_id":b["id"]}));
    assert_eq!(
        ok(e, "task.get", json!({"task_id":a["id"]}))["blocked_by"],
        json!([])
    );
    ok(e, "task.restore", json!({"task_id":b["id"]}));
    assert_eq!(
        ids(&ok(e, "task.get", json!({"task_id":a["id"]}))["blocked_by"]),
        vec![b["id"].as_i64().unwrap()]
    );
}

#[test]
fn dispatch_fans_out_to_sub_tasks() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Body of work", json!({"type":"feature"}));
    let child = f.task("Half of it", json!({"parent_id":parent["id"]}));
    let grandchild = f.task("A quarter", json!({"parent_id":child["id"]}));
    let done = f.task("Already shipped", json!({"parent_id":parent["id"]}));
    ok(e, "task.move", json!({"task_id":done["id"],"column":"in_review"}));
    ok(e, "task.approve", json!({"task_id":done["id"],"sha":"cafe"}));

    let result = ok(
        e,
        "task.dispatch",
        json!({"task_id":parent["id"],"create":{"project_id":1,"provider":"claude","role":"builder"},"fanout":true}),
    );
    assert_eq!(result["task"]["column"], "active");
    let fanned = result["fanned"].as_array().unwrap();
    assert_eq!(fanned.len(), 2, "the done sub-task is left alone");
    let touched: Vec<i64> = fanned.iter().map(|d| d["task"]["id"].as_i64().unwrap()).collect();
    assert!(touched.contains(&child["id"].as_i64().unwrap()));
    assert!(touched.contains(&grandchild["id"].as_i64().unwrap()));
    // One session each, so every card on the board names the agent working it.
    let names: Vec<&str> = fanned.iter().map(|d| d["session"]["name"].as_str().unwrap()).collect();
    assert_ne!(names[0], names[1]);
    assert_ne!(names[0], result["session"]["name"].as_str().unwrap());
    assert_eq!(
        ok(e, "task.get", json!({"task_id":child["id"]}))["state"],
        "dispatched"
    );

    // A single sub-task dispatches on its own, and fanout without `create` is a typed refusal
    // rather than quietly piling several tasks onto one session.
    let lone = f.task("Standalone", json!({"parent_id":parent["id"]}));
    assert_eq!(
        code(call(
            e,
            Actor::User,
            "task.dispatch",
            json!({"task_id":lone["id"],"session":result["session"]["name"],"fanout":true})
        )),
        "task.fanout_target"
    );
    let solo = ok(
        e,
        "task.dispatch",
        json!({"task_id":lone["id"],"session":result["session"]["name"]}),
    );
    assert_eq!(solo["task"]["state"], "dispatched");
    assert_eq!(solo["fanned"], json!([]));
    // task.list by session is the "which cards is this agent on" query the board filters with.
    let name = result["session"]["name"].as_str().unwrap();
    let mine = ok(e, "task.list", json!({"project_id":1,"session":name}))["tasks"].clone();
    assert_eq!(mine.as_array().unwrap().len(), 2);
}

/// Rows written before v16 are typed `task`, parentless, unlabelled and unrelated; the
/// migration must not invent a classification for them (D139). The row is written by a v15
/// store and read back through the bus after the current build has migrated it.
#[test]
fn tasks_from_before_v16_keep_neutral_board_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        for m in &relay_core::store::MIGRATIONS[..15] {
            c.execute_batch(m).unwrap();
        }
        c.pragma_update(None, "user_version", 15).unwrap();
        c.execute_batch(
            "INSERT INTO workspaces(path,name,created_at,updated_at) VALUES ('/ws','ws','then','then');
             INSERT INTO projects(workspace_id,path,name,created_at,updated_at) VALUES (1,'/ws/app','app','then','then');
             INSERT INTO tasks(project_id,title,col,state,position,created_at,updated_at)
               VALUES (1,'Written before the redesign','ready','dispatched',0,'then','then');",
        )
        .unwrap();
    }
    let e = Engine::new(Instance::Test, Store::open(&path, false).unwrap());
    let reread = ok(&e, "task.get", json!({"task_id":1}));
    assert_eq!(reread["title"], "Written before the redesign");
    assert_eq!(reread["column"], "ready");
    assert_eq!(reread["state"], "dispatched");
    assert_eq!(reread["type"], "task");
    assert!(reread["parent_id"].is_null());
    assert_eq!(reread["depth"], 0);
    assert_eq!(reread["labels"], json!([]));
    assert_eq!(reread["blocked_by"], json!([]));
    assert_eq!(reread["blocks"], json!([]));
    assert!(reread["duplicate_of"].is_null());
    assert_eq!(reread["rollup"], json!({"total":0,"done":0}));
}

#[test]
fn task_activity_is_exactly_scoped_paginated_and_user_only() {
    let f = Fixture::new();
    let e = &f.engine;
    let first = f.task("First task", json!({}));
    let other = f.task("Other task", json!({}));
    let session = ok(
        e,
        "session.create",
        json!({"project_id":1,"provider":"codex","role":"builder"}),
    );
    let name = session["name"].as_str().unwrap();
    for column in ["ready", "active", "backlog"] {
        ok(
            e,
            "task.move",
            json!({"task_id":first["id"],"column":column}),
        );
        ok(
            e,
            "task.move",
            json!({"task_id":other["id"],"column":column}),
        );
    }
    for index in 0..3 {
        ok(
            e,
            "mailbox.send",
            json!({"project_id":1,"to":name,"text":format!("first {index}"),"re_task":first["id"]}),
        );
        ok(
            e,
            "mailbox.send",
            json!({"project_id":1,"to":name,"text":format!("other {index}"),"re_task":other["id"]}),
        );
    }
    ok(
        e,
        "mailbox.send",
        json!({"project_id":1,"to":name,"text":"not task scoped"}),
    );
    let mut before_audit = Value::Null;
    let mut before_message = Value::Null;
    let mut audit_ids = std::collections::BTreeSet::new();
    let mut message_ids = std::collections::BTreeSet::new();
    loop {
        let page = ok(
            e,
            "task.activity",
            json!({"task_id":first["id"],"limit":1,"before_audit":before_audit,"before_message":before_message}),
        );
        for row in page["history"].as_array().unwrap() {
            assert!(
                audit_ids.insert(row["id"].as_i64().unwrap()),
                "history cursor repeated a row"
            );
            assert!(
                row["payload"]["task_id"] == first["id"]
                    || (row["op"] == "task.create" && row["result_summary"]["id"] == first["id"])
            );
        }
        for message in page["messages"].as_array().unwrap() {
            assert_eq!(message["re_task"], first["id"]);
            assert!(
                message_ids.insert(message["id"].as_i64().unwrap()),
                "message cursor repeated a row"
            );
        }
        if page["next_audit"].is_null() && page["next_message"].is_null() {
            break;
        }
        before_audit = page["next_audit"].as_i64().map_or(json!(0), |id| json!(id));
        before_message = page["next_message"]
            .as_i64()
            .map_or(json!(0), |id| json!(id));
    }
    assert_eq!(audit_ids.len(), 4, "creation plus three moves");
    assert_eq!(message_ids.len(), 3);
    let response = call(
        e,
        Actor::Agent(name.to_string()),
        "task.activity",
        json!({"task_id":first["id"]}),
    );
    assert_eq!(
        code(response),
        "actor.allowlist",
        "agent cannot read user-wide task conversations"
    );
}

/// A card dropped between two others lands there: `position` on `task.move` is the index the
/// task ends at in its column, and undo puts it back exactly where it was, gaps and all.
#[test]
fn move_with_position_reorders_a_column_and_undo_restores_it() {
    let f = Fixture::new();
    let e = &f.engine;
    let ready = |f: &Fixture, title: &str| f.task(title, json!({"column":"ready"}))["id"].as_i64().unwrap();
    let (a, b, c, d) = (ready(&f, "A"), ready(&f, "B"), ready(&f, "C"), ready(&f, "D"));
    let order = |column: &str| -> Vec<i64> {
        ok(e, "task.list", json!({"project_id":1}))["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["column"] == column)
            .map(|t| t["id"].as_i64().unwrap())
            .collect()
    };
    let undo_last = |op: &str| {
        let row = ok(e, "audit.list", json!({"op_prefix":op,"limit":1}))["rows"][0].clone();
        ok(e, "audit.undo", json!({"audit_id":row["id"]}));
    };
    assert_eq!(order("ready"), vec![a, b, c, d]);

    // Down the column: A ends at index 2, after C.
    ok(e, "task.move", json!({"task_id":a,"column":"ready","position":2}));
    assert_eq!(order("ready"), vec![b, c, a, d]);
    undo_last("task.move");
    assert_eq!(order("ready"), vec![a, b, c, d]);

    // Up the column: D ends at index 1, and undo returns it to the bottom.
    ok(e, "task.move", json!({"task_id":d,"column":"ready","position":1}));
    assert_eq!(order("ready"), vec![a, d, b, c]);
    undo_last("task.move");
    assert_eq!(order("ready"), vec![a, b, c, d]);

    // Into another column at a given slot, and back out on undo.
    let x = f.task("X", json!({"column":"active"}))["id"].as_i64().unwrap();
    let y = f.task("Y", json!({"column":"active"}))["id"].as_i64().unwrap();
    ok(e, "task.move", json!({"task_id":b,"column":"active","position":1}));
    assert_eq!(order("active"), vec![x, b, y]);
    assert_eq!(order("ready"), vec![a, c, d]);
    undo_last("task.move");
    assert_eq!(order("ready"), vec![a, b, c, d]);
    assert_eq!(order("active"), vec![x, y]);

    // An index past the end clamps to last; without a position a move still appends.
    ok(e, "task.move", json!({"task_id":a,"column":"ready","position":99}));
    assert_eq!(order("ready"), vec![b, c, d, a]);
    ok(e, "task.move", json!({"task_id":x,"column":"ready"}));
    assert_eq!(order("ready"), vec![b, c, d, a, x]);

    // Approval out of a reordered column is undone to the same slot as well.
    ok(e, "task.approve", json!({"task_id":c,"sha":"cafe"}));
    assert_eq!(order("ready"), vec![b, d, a, x]);
    undo_last("task.approve");
    assert_eq!(order("ready"), vec![b, c, d, a, x]);
}

/// Independent agents that share the primary checkout (every session of an Unreal-plugin
/// project, D160) are not one review group: each one's task reaches review on its own done,
/// and dispatching to one never queues the task for the others.
#[test]
fn independent_builders_on_the_primary_checkout_do_not_wait_on_each_other() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = ok(e, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
    let b = ok(e, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
    let (a, b) = (a["name"].as_str().unwrap().to_string(), b["name"].as_str().unwrap().to_string());
    let first = f.task("A's task", json!({}));
    let second = f.task("B's task", json!({}));
    ok(e, "task.dispatch", json!({"task_id":first["id"],"session":a,"start":false}));
    ok(e, "task.dispatch", json!({"task_id":second["id"],"session":b,"start":false}));
    let mine = |name: &str| ids(&ok(e, "task.list", json!({"project_id":1,"session":name}))["tasks"].as_array().unwrap()
        .iter().map(|t| t["id"].clone()).collect::<Value>());
    assert_eq!(mine(&a), [first["id"].as_i64().unwrap()]);
    assert_eq!(mine(&b), [second["id"].as_i64().unwrap()]);

    let done = |name: &str| call(e, Actor::agent(name), "session.done", json!({"session":name,"summary":"done"}))
        .into_result().unwrap_or_else(|err| panic!("session.done: {} {}", err.code, err.message));
    done(&a);
    assert_eq!(ok(e, "task.get", json!({"task_id":first["id"]}))["column"], "in_review");
    assert_eq!(ok(e, "task.get", json!({"task_id":second["id"]}))["column"], "active");
    done(&b);
    assert_eq!(ok(e, "task.get", json!({"task_id":second["id"]}))["column"], "in_review");

    // Closing either keeps the shared checkout, and is not refused for the other being there.
    ok(e, "session.close", json!({"session":a}));
}

/// `task.dispatch {create}` fetches and checks out a new worktree: that happens in a request of
/// its own, before the dispatch takes the store, and the launch happens after the assignment is
/// committed — yet before the reply, so the session is already running when the caller looks.
#[test]
fn dispatch_with_create_starts_the_session_and_a_refused_one_leaves_nothing_behind() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Ship it", json!({}));
    let out = ok(e, "task.dispatch", json!({"task_id":task["id"],"create":{"project_id":1,"provider":"claude","role":"builder"}}));
    let name = out["session"]["name"].as_str().unwrap();
    let session = ok(e, "session.get", json!({"session":name}));
    assert_eq!(session["state"], "running");
    assert_eq!(session["task_id"], task["id"]);
    let brief = std::fs::read_to_string(std::path::Path::new(session["worktree"].as_str().unwrap()).join(".relay/sessions").join(name).join("session-brief.md")).unwrap();
    assert!(brief.contains("Ship it"), "the brief was written before the assignment: {brief}");

    // A missing provider is refused before any session or checkout is made.
    ok(e, "settings.set", json!({"path":"providers.codex.path","value":"/nonexistent/codex"}));
    let before = ok(e, "session.list", json!({"project_id":1}))["sessions"].as_array().unwrap().len();
    let other = f.task("Not today", json!({}));
    assert_eq!(code(call(e, Actor::User, "task.dispatch",
        json!({"task_id":other["id"],"create":{"project_id":1,"provider":"codex","role":"builder"}}))), "provider.not_installed");
    assert_eq!(ok(e, "session.list", json!({"project_id":1}))["sessions"].as_array().unwrap().len(), before);
    assert_eq!(ok(e, "task.get", json!({"task_id":other["id"]}))["column"], "backlog");
    ok(e, "session.close", json!({"session":name}));
}

#[test]
fn task_list_is_paged_and_a_summary_leaves_out_the_long_fields() {
    let f = Fixture::new();
    let e = &f.engine;
    for n in 0..5 {
        f.task(&format!("Task {n}"), json!({"body": "a long body", "changelog": "notes"}));
    }
    let first = ok(e, "task.list", json!({"project_id":1,"limit":2}));
    assert_eq!(first["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(first["next_offset"], 2);
    let last = ok(e, "task.list", json!({"project_id":1,"limit":2,"offset":4}));
    assert_eq!(last["tasks"].as_array().unwrap().len(), 1);
    assert!(last.get("next_offset").is_none());
    let all = ok(e, "task.list", json!({"project_id":1}));
    assert_eq!(all["tasks"].as_array().unwrap().len(), 5, "the default page holds a normal board");
    let summary = ok(e, "task.list", json!({"project_id":1,"summary":true}));
    assert!(summary["tasks"].as_array().unwrap().iter().all(|t| t["body"] == "" && t["changelog"] == "" && t["title"] != ""));
}

/// RA-495: every task read names its module, a completed (archived) one included, so the board
/// labels cards without a second `module.list`.
#[test]
fn tasks_name_their_module_even_once_it_is_completed() {
    let f = Fixture::new();
    let e = &f.engine;
    let module = ok(e, "module.create", json!({"project_id":1,"name":"Checkout"}));
    let task = f.task("Card", json!({"module_id": module["id"]}));
    assert_eq!(task["module_name"], "Checkout", "task.create");
    let loose = f.task("Loose", json!({}));
    assert!(loose["module_name"].is_null(), "{loose}");
    ok(e, "module.complete", json!({"module_id": module["id"]}));
    let listed = ok(e, "task.list", json!({"project_id":1}));
    let card = listed["tasks"].as_array().unwrap().iter().find(|t| t["id"] == task["id"]).unwrap();
    assert_eq!(card["module_name"], "Checkout", "task.list: {card}");
    let other = listed["tasks"].as_array().unwrap().iter().find(|t| t["id"] == loose["id"]).unwrap();
    assert!(other["module_name"].is_null(), "{other}");
    let filtered = ok(e, "task.list", json!({"project_id":1,"module_id":module["id"],"sort":"priority"}));
    assert_eq!(filtered["tasks"][0]["module_name"], "Checkout", "filters and sorts still resolve against tasks");
    assert_eq!(ok(e, "task.get", json!({"task_id": task["id"]}))["module_name"], "Checkout", "task.get");
    ok(e, "module.update", json!({"module_id": module["id"], "name": "Payments"}));
    assert_eq!(ok(e, "task.get", json!({"task_id": task["id"]}))["module_name"], "Payments", "the name is read, not copied");
}

/// Stores written under the old rule still hold the cross-queued rows; the startup check
/// drops the ones that are certainly another group's, and the deadlock lifts.
#[test]
fn startup_drops_task_rows_the_old_shared_checkout_rule_cross_queued() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = ok(e, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
    let b = ok(e, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary"}));
    let (a, b) = (a["name"].as_str().unwrap().to_string(), b["name"].as_str().unwrap().to_string());
    let first = f.task("A's task", json!({}));
    let second = f.task("B's task", json!({}));
    ok(e, "task.dispatch", json!({"task_id":first["id"],"session":a,"start":false}));
    ok(e, "task.dispatch", json!({"task_id":second["id"],"session":b,"start":false}));
    // What the old rule left behind: each task queued for the other agent too.
    e.store.lock().execute_batch(&format!(
        "INSERT INTO task_sessions(task_id, session_id, ord, queue_ord)
           SELECT {first}, id, 9, 9 FROM sessions WHERE name='{b}';
         INSERT INTO task_sessions(task_id, session_id, ord, queue_ord)
           SELECT {second}, id, 9, 9 FROM sessions WHERE name='{a}';",
        first = first["id"], second = second["id"],
    )).unwrap();
    let report = relay_core::recovery::run(e).unwrap();
    assert!(report.fsck_fixes.iter().any(|fix| fix.contains("dropped 2 task queue row")), "{:?}", report.fsck_fixes);
    call(e, Actor::agent(&a), "session.done", json!({"session":a,"summary":"done"})).into_result().unwrap();
    assert_eq!(ok(e, "task.get", json!({"task_id":first["id"]}))["column"], "in_review");
}
