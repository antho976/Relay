//! Replies, expectations and skip reasons (feature-agent-api W5, W7, W8, W18, B1, B7; MODEL-SPEC
//! §1 #8, #9, #20, §2.11, §3.2; TM-17; E-agent-7): an agent's `design.apply` answers short and the
//! person's clients full; a full answer stays under the clients' line; every applied list keeps
//! what its undo expects to find, so `audit.undo` refuses to overwrite a later change and a
//! lenient undo skips just what would; and a lenient list says why it skipped each edit.

mod common;

use common::{call, call_as, committed_repo, engine_with_project, ok, ok_as, refused};
use relay_bus::design::MAX_OPS;
use relay_bus::{Actor, ErrorKind, Event, Request};
use relay_core::engine::{Door, Engine};
use serde_json::{json, Value};
use std::sync::Arc;

fn project() -> (Arc<Engine>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    (engine, root)
}

fn create(e: &Engine) -> i64 {
    ok(e, "design.create", json!({"project_id": 1, "title": "Home"}))["design"]["id"].as_i64().unwrap()
}

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
}

fn agent(e: &Engine) -> Actor {
    let name = ok(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    Actor::agent(&name)
}

fn rev(e: &Engine, design: i64) -> i64 {
    ok(e, "design.get", json!({"design_id": design, "pages_only": true}))["design"]["rev"].as_i64().unwrap()
}

fn node(e: &Engine, design: i64, id: &str) -> Option<Value> {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned()
}

fn changed_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> usize {
    std::iter::from_fn(|| rx.try_recv().ok()).filter(|ev| ev.ev == "design.changed").count()
}

/// The person undoes `out` (a full answer) as the canvas does: its inverse, lenient, with what
/// it expects.
fn undo(e: &Engine, design: i64, out: &Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": out["inverse"], "lenient": true, "expect": out["expect"]}))
}

/// A card frame holding a button with a label, and a free sticky.
fn sample(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 200}},
        {"op": "add", "node": {"id": "save", "parent": "card", "kind": "rect", "w": 120, "h": 40, "fill": "#FF0000", "radius": 4}},
        {"op": "add", "node": {"id": "label", "parent": "card", "kind": "text", "text": "Save"}},
        {"op": "add", "node": {"id": "note", "parent": "page-1", "kind": "sticky", "x": 400, "text": "remember"}},
    ]));
    id
}

// --- W5: short and full replies ---------------------------------------------------------------

/// W5: an agent's answer is short: the rev, each edit's id, how many went, the audit row. The
/// person's clients get the design row, the nodes, the removals, the undo and what it expects.
#[test]
fn an_agent_gets_a_short_reply_and_the_person_a_full_one() {
    let (e, _root) = project();
    let id = sample(&e);
    let at = rev(&e, id);
    let short = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [
        {"op": "update", "id": "save", "set": {"fill": "#00FF00"}}, {"op": "delete", "id": "note"},
    ]}));
    let mut keys: Vec<&str> = short.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["audit_id", "design_id", "ids", "prev_rev", "removed_count", "rev"], "{short}");
    assert_eq!((short["rev"].as_i64(), short["prev_rev"].as_i64(), short["removed_count"].as_u64()), (Some(at + 1), Some(at), Some(1)));
    assert_eq!(short["ids"], json!(["save", "note"]));
    let full = ok(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#0000FF"}}]}));
    for key in ["design", "nodes", "removed", "inverse", "expect", "audit_id"] {
        assert!(full.get(key).is_some(), "{key} missing from {full}");
    }
    assert_eq!(full["expect"], json!([{"id": "save", "set": {"fill": "#0000FF"}}]));
    assert_eq!(full["inverse"], json!([{"op": "update", "id": "save", "set": {"fill": "#00FF00"}}]));
}

/// W5: an agent may ask for the full answer.
#[test]
fn reply_full_gives_an_agent_the_nodes() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "reply": "full", "ops": [{"op": "update", "id": "label", "set": {"text": "Send"}}]}));
    assert_eq!(out["nodes"][0]["text"], "Send");
    assert_eq!(out["inverse"], json!([{"op": "update", "id": "label", "set": {"text": "Save"}}]));
    // And the person a short one.
    let out = ok(&e, "design.apply", json!({"design_id": id, "reply": "short", "ops": [{"op": "update", "id": "label", "set": {"text": "Go"}}]}));
    assert!(out.get("nodes").is_none() && out.get("design").is_none(), "{out}");
}

/// W5, W7: `audit_id` is the row audit.get shows the change as, with its undo and what the undo
/// expects to find.
#[test]
fn audit_id_names_this_change() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "move", "id": "label", "parent": "page-1"}]}));
    let row = ok(&e, "audit.get", json!({"audit_id": out["audit_id"]}));
    assert_eq!((row["op"].as_str(), row["payload"]["ops"][0]["op"].as_str()), (Some("design.apply"), Some("move")));
    let undo = &row["undo_op"];
    assert_eq!((undo["op"].as_str(), undo["payload"]["lenient"].as_bool(), undo["payload"]["reply"].as_str()), (Some("design.apply"), Some(true), Some("short")));
    assert_eq!(undo["expect"]["nodes"], json!([{"id": "label", "parent": "page-1"}]));
}

// --- B7: a full answer stays under the clients' line ------------------------------------------

/// 150 texts of 9,000 characters: about 1.35 MiB of nodes.
fn big_texts() -> Vec<Value> {
    let text = "x".repeat(9_000);
    (0..150).map(|i| json!({"op": "add", "node": {"id": format!("t{i}"), "parent": "page-1", "kind": "text", "text": text}})).collect()
}

/// B7: a full answer whose nodes would pass 1 MiB leaves them out and asks for a reread; the
/// undo, under it, is still given.
#[test]
fn a_full_reply_over_budget_leaves_out_nodes_and_asks_for_a_reread() {
    let (e, _root) = project();
    let id = create(&e);
    let out = apply(&e, id, Value::Array(big_texts()));
    assert_eq!(out["reread"], true);
    assert!(out.get("nodes").is_none(), "nodes left out");
    assert_eq!(out["inverse"].as_array().unwrap().len(), 150, "the undo is small: deletes");
    assert!(out.get("warnings").is_none(), "{}", out["warnings"]);
}

/// B7: deleting a near-full design and adding it back in one list answers under the 2 MiB line:
/// without the nodes, and without the undo, which the person is told.
#[test]
fn the_largest_full_reply_fits_the_clients_line() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, Value::Array(big_texts()));
    let mut ops: Vec<Value> = (0..150).map(|i| json!({"op": "delete", "id": format!("t{i}")})).collect();
    ops.extend(big_texts());
    let out = apply(&e, id, Value::Array(ops));
    let bytes = serde_json::to_vec(&out).unwrap().len();
    assert!(bytes < 2 * 1024 * 1024, "{bytes} bytes");
    assert_eq!(out["reread"], true);
    assert!(out.get("inverse").is_none() && out.get("expect").is_none());
    assert_eq!(out["warnings"], json!(["this change is too large for the audit log to undo (over 256 KiB)", "this change is too large to undo from the board"]));
}

// --- W7, B1: audit.undo refuses to overwrite a later change -----------------------------------

fn audit_undo(e: &Engine, audit_id: &Value, force: bool) -> relay_bus::Response {
    call(e, "audit.undo", json!({"audit_id": audit_id, "force": force}))
}

/// B1: the agent sets a fill, the person changes it again; undoing the agent's change is refused
/// naming the field, and goes through with force.
#[test]
fn audit_undo_refuses_when_a_later_edit_changed_what_it_writes() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#0000FF", "radius": 8}}]}));
    apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#00FF00"}}]));
    let error = refused(audit_undo(&e, &out["audit_id"], false));
    assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Conflict, "audit.stale"));
    assert_eq!(error.details.unwrap()["conflicts"], json!([{"at": 0, "id": "save", "reason": "fields", "fields": ["fill"]}]));
    audit_undo(&e, &out["audit_id"], true).into_result().unwrap();
    let save = node(&e, id, "save").unwrap();
    assert_eq!((save["fill"].as_str(), save["radius"].as_f64()), (Some("#FF0000"), Some(4.0)));
}

/// B1: an undo that deletes the frame the agent added is refused once the person put a layer in
/// it, naming the layer.
#[test]
fn audit_undo_refuses_to_delete_what_was_added_inside_since() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "panel", "parent": "page-1", "kind": "frame", "w": 200, "h": 200}},
        {"op": "add", "node": {"id": "panel-title", "parent": "panel", "kind": "text", "text": "Hi"}},
    ]}));
    apply(&e, id, json!([{"op": "add", "node": {"id": "mine", "parent": "panel", "kind": "rect"}}]));
    let error = refused(audit_undo(&e, &out["audit_id"], false));
    assert_eq!(error.details.unwrap()["conflicts"], json!([{"at": 1, "id": "panel", "reason": "inside", "ids": ["mine"]}]));
    assert!(node(&e, id, "mine").is_some());
}

/// B1, B50: an undo of a delete whose id was taken since is refused (`exists`).
#[test]
fn audit_undo_refuses_to_re_add_an_id_taken_since() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "delete", "id": "note"}]}));
    apply(&e, id, json!([{"op": "add", "node": {"id": "note", "parent": "page-1", "kind": "text", "text": "new"}}]));
    let error = refused(audit_undo(&e, &out["audit_id"], false));
    assert_eq!(error.details.unwrap()["conflicts"], json!([{"at": 0, "id": "note", "reason": "exists"}]));
}

/// A node deleted since is no conflict for audit.undo: the replay skips its edits, as before.
#[test]
fn audit_undo_still_skips_nodes_deleted_since() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [
        {"op": "update", "id": "save", "set": {"fill": "#0000FF"}}, {"op": "update", "id": "label", "set": {"text": "Go"}},
    ]}));
    apply(&e, id, json!([{"op": "delete", "id": "save"}]));
    audit_undo(&e, &out["audit_id"], false).into_result().unwrap();
    assert_eq!(node(&e, id, "label").unwrap()["text"], "Save");
}

/// An undo kept before expectations were (no `expect.nodes`) undoes as it did.
#[test]
fn rows_from_before_expectations_undo_as_before() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#0000FF"}}]}));
    e.store.lock().execute("UPDATE audit SET undo_op = json_remove(undo_op, '$.expect') WHERE id = ?1", [out["audit_id"].as_i64().unwrap()]).unwrap();
    apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#00FF00"}}]));
    audit_undo(&e, &out["audit_id"], false).into_result().unwrap();
    assert_eq!(node(&e, id, "save").unwrap()["fill"], "#FF0000");
}

// --- W7, W8 through design.apply --------------------------------------------------------------

/// W7: the person's undo of their own change keeps an agent's later change to another field of
/// the node, and to the same field, and says which field it left.
#[test]
fn a_lenient_undo_keeps_later_changes_and_reports_them() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#0000FF", "radius": 8}}]));
    ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#00FF00", "w": 160}}]}));
    let undone = undo(&e, id, &out);
    assert_eq!(undone["skipped"], json!([{"at": 0, "id": "save", "reason": "fields", "fields": ["fill"]}]));
    let save = node(&e, id, "save").unwrap();
    assert_eq!((save["fill"].as_str(), save["radius"].as_f64(), save["w"].as_f64()), (Some("#00FF00"), Some(4.0), Some(160.0)));
}

/// TM-17, MODEL-SPEC §1 #20: the person adds a rect, an agent deletes it, the person's undo of
/// the add finds it gone: nothing applies, so no rev, no event and no undo.
#[test]
fn an_undo_with_every_edit_skipped_changes_nothing() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect"}}]));
    ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "delete", "id": "r"}]}));
    let (at, undos) = (rev(&e, id), e.store.lock().query_row("SELECT COUNT(*) FROM audit WHERE undo_op IS NOT NULL", [], |r| r.get::<_, i64>(0)).unwrap());
    let mut rx = e.subscribe();
    let undone = undo(&e, id, &out);
    assert_eq!((undone["rev"].as_i64(), undone["ids"].clone()), (Some(at), json!([""])));
    assert_eq!(undone["skipped"][0]["reason"], "missing");
    assert!(undone["skipped"][0]["rev"].is_i64(), "says when it went: {undone}");
    assert!(undone.get("audit_id").is_none());
    assert_eq!(changed_events(&mut rx), 0);
    let after = e.store.lock().query_row("SELECT COUNT(*) FROM audit WHERE undo_op IS NOT NULL", [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(after, undos, "no undo kept");
}

/// W8: a strict list sent with what it expects is refused, naming the fields that moved.
#[test]
fn an_edit_expecting_old_values_is_refused_with_the_fields_that_moved() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#00FF00"}}]));
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#0000FF"}}],
        "expect": [{"id": "save", "set": {"fill": "#FF0000", "radius": 4, "width": 120}}]})));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.stale", "nodes this list expects have changed: save (fill)"));
    // What it holds, written another way (12 as 12.0, width as w), is not a change.
    apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#0000FF"}}]));
    ok(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "save", "set": {"fill": "#FFFFFF"}}],
        "expect": [{"id": "save", "set": {"fill": "#0000FF", "radius": 4.0, "width": 120, "stroke": null}}]}));
}

/// An expectation on `runs` holds when only the text changed: values are compared, not
/// re-validated against the rest of the node.
#[test]
fn an_expectation_on_runs_holds_when_only_text_changed() {
    let (e, _root) = project();
    let id = sample(&e);
    let runs = json!([{"start": 0, "end": 4, "color": "#FF0000"}]);
    let out = apply(&e, id, json!([{"op": "update", "id": "label", "set": {"runs": runs}}]));
    // An agent changes the text, sending the runs it keeps (a text with runs takes both).
    ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "label", "set": {"text": "Saved", "runs": runs}}]}));
    let undone = undo(&e, id, &out);
    assert!(undone.get("skipped").is_none(), "{undone}");
    let label = node(&e, id, "label").unwrap();
    assert_eq!((label["text"].as_str(), label.get("runs")), (Some("Saved"), None));
}

/// MODEL-SPEC §1 #8 (B50): an undo that meets an id taken since skips that node and everything
/// that was to go inside it, and says so; it never renames.
#[test]
fn a_skipped_add_skips_what_was_inside_it() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "delete", "id": "card"}]));
    ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "rect"}}]}));
    let undone = undo(&e, id, &out);
    assert_eq!(undone["skipped"], json!([
        {"at": 0, "id": "card", "reason": "exists"}, {"at": 1, "id": "save", "reason": "parent_skipped"}, {"at": 2, "id": "label", "reason": "parent_skipped"},
    ]));
    assert_eq!(node(&e, id, "card").unwrap()["kind"], "rect");
    assert!(node(&e, id, "save").is_none());
}

/// MODEL-SPEC §1 #9 (B69): the person adds a frame, an agent fills it; the person's undo of the
/// add keeps the frame and what the agent put in it.
#[test]
fn a_lenient_delete_keeps_a_frame_someone_else_filled() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "add", "node": {"id": "f", "parent": "page-1", "kind": "frame"}}]));
    assert_eq!(out["expect"], json!([{"id": "f", "within": []}]));
    ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"id": "c", "parent": "f", "kind": "rect"}}]}));
    let undone = undo(&e, id, &out);
    assert_eq!(undone["skipped"], json!([{"at": 0, "id": "f", "reason": "inside", "ids": ["c"]}]));
    assert!(node(&e, id, "f").is_some() && node(&e, id, "c").is_some());
    // An undo with nothing added inside since still deletes it.
    let out = apply(&e, id, json!([{"op": "add", "node": {"id": "g", "parent": "page-1", "kind": "frame"}}, {"op": "add", "node": {"id": "g-1", "parent": "g", "kind": "rect"}}]));
    assert_eq!(out["expect"], json!([{"id": "g", "within": ["g-1"]}]));
    undo(&e, id, &out);
    assert!(node(&e, id, "g").is_none());
}

/// W18: every lenient skip says why: a node gone, its parent gone, and one this Relay cannot read.
#[test]
fn a_lenient_list_says_why_it_skipped_each_edit() {
    let (e, _root) = project();
    let id = sample(&e);
    e.store.lock().execute("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body, rev) VALUES (?1, 'future', 'page-1', 'hologram', 99, '{}', 1)", [id]).unwrap();
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "lenient": true, "ops": [
        {"op": "update", "id": "ghost", "set": {"x": 1}},
        {"op": "add", "node": {"id": "orphan", "parent": "nowhere", "kind": "rect"}},
        {"op": "update", "id": "future", "set": {"x": 1}},
        {"op": "update", "id": "save", "set": {"x": 1}},
    ]}));
    assert_eq!(out["skipped"], json!([
        {"at": 0, "id": "ghost", "reason": "missing"}, {"at": 1, "id": "orphan", "reason": "no_parent"}, {"at": 2, "id": "future", "reason": "unreadable"},
    ]));
    assert_eq!(out["ids"], json!(["", "", "", "save"]));
}

/// W18: an agent's lenient list is held to MAX_OPS; the person's undo of a whole design is not.
#[test]
fn an_agents_lenient_list_is_capped_at_max_ops_and_the_persons_is_not() {
    let (e, _root) = project();
    let id = create(&e);
    let ops: Vec<Value> = (0..=MAX_OPS).map(|i| json!({"op": "add", "node": {"id": format!("r{i}"), "parent": "page-1", "kind": "rect"}})).collect();
    let error = refused(call_as(&e, agent(&e), "design.apply", json!({"design_id": id, "lenient": true, "ops": ops})));
    assert_eq!(error.code, "design.ops");
    let out = ok(&e, "design.apply", json!({"design_id": id, "lenient": true, "ops": ops}));
    assert_eq!(out["ids"].as_array().unwrap().len(), MAX_OPS + 1);
}

// --- E-agent-7 ---------------------------------------------------------------------------------

/// E-agent-7: a request sent again whose result was too large to keep says it was applied, and
/// not to send it again.
#[test]
fn a_replay_whose_result_was_not_kept_says_it_applied() {
    let (e, _root) = project();
    let id = create(&e);
    let ops: Vec<Value> = (0..40).map(|i| json!({"op": "add", "node": {"id": format!("t{i}"), "parent": "page-1", "kind": "text", "text": "y".repeat(2_000)}})).collect();
    let request = Request::new(Actor::User, "design.apply", json!({"design_id": id, "ops": ops}));
    e.dispatch(request.clone(), Door::InProcess).into_result().unwrap();
    let error = e.dispatch(request, Door::InProcess).into_result().unwrap_err();
    assert_eq!((error.code.as_str(), error.message.as_str()), ("bus.replay_unavailable", "this request was already applied; its result was too large to keep"));
    assert_eq!(error.details.unwrap(), json!({"applied": true}));
    assert_eq!(error.hint.as_deref(), Some("read the current state (e.g. design.get) instead of sending it again"));
}

// --- Timings ----------------------------------------------------------------------------------

/// W7: an undo re-adding 5,000 top-level nodes, and the full answer that keeps what its own undo
/// expects. Run with `cargo test --release -p relay-core --test design_expect -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_an_undo_of_a_5000_node_delete() {
    let (e, _root) = project();
    let id = create(&e);
    for k in 0..3 {
        let ops: Vec<Value> = (0..if k == 2 { 999 } else { 2000 }).map(|i| json!({"op": "add", "node": {
            "id": format!("n{k}-{i}"), "parent": "page-1", "kind": if i % 5 == 0 { "frame" } else { "rect" }, "x": i, "w": 40, "h": 20, "fill": "#336699",
        }})).collect();
        apply(&e, id, Value::Array(ops));
    }
    let median = |mut runs: Vec<std::time::Duration>| { runs.sort(); runs[runs.len() / 2] };
    let (mut deletes, mut undos) = (Vec::new(), Vec::new());
    for n in 0..7 {
        let ops: Vec<Value> = (0..3).flat_map(|k| (0..if k == 2 { 999 } else { 2000 }).map(move |i| json!({"op": "delete", "id": format!("n{k}-{i}")}))).collect();
        let started = std::time::Instant::now();
        let out = ok(&e, "design.apply", json!({"design_id": id, "ops": ops, "lenient": true}));
        let took = started.elapsed();
        let started = std::time::Instant::now();
        let back = undo(&e, id, &out);
        let took_undo = started.elapsed();
        assert_eq!(back["ids"].as_array().unwrap().len(), 4_999);
        if n >= 2 {
            deletes.push(took);
            undos.push(took_undo);
        }
    }
    println!("design.apply delete of 4,999 top-level nodes, full answer: median {:?} of {}", median(deletes.clone()), deletes.len());
    println!("design.apply lenient undo re-adding them, with its expectations: median {:?} of {}", median(undos.clone()), undos.len());
}

// --- Review of engine-expect and engine-reuse --------------------------------------------------

/// A fields skip is reported at the update that writes the field, not at every update of the node.
#[test]
fn a_fields_skip_names_only_the_update_that_writes_it() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#0000FF"}}, {"op": "update", "id": "save", "set": {"radius": 8}}]));
    apply(&e, id, json!([{"op": "update", "id": "save", "set": {"fill": "#00FF00"}}]));
    let undone = undo(&e, id, &out);
    // The undo runs the radius back first, then the fill.
    assert_eq!(undone["skipped"], json!([{"at": 1, "id": "save", "reason": "fields", "fields": ["fill"]}]));
    assert_eq!(node(&e, id, "save").unwrap()["radius"].as_f64(), Some(4.0));
}

/// An undo that deletes a node and then adds back an id inside it is not refused as `exists`:
/// the delete frees what is inside it first.
#[test]
fn an_undo_that_deletes_the_holder_first_may_re_add_its_ids() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "card-2:save", "parent": "page-1", "kind": "rect"}}]));
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "ops": [
        {"op": "delete", "id": "card-2:save"}, {"op": "copy", "id": "card", "as": "card-2"},
    ]}));
    ok(&e, "audit.undo", json!({"audit_id": out["audit_id"]}));
    assert!(node(&e, id, "card-2").is_none());
    assert_eq!(node(&e, id, "card-2:save").unwrap()["parent"], "page-1");
}

/// A lenient copy whose ids are taken makes nothing, and what the list puts in it is skipped
/// too, never put in the node that holds the id.
#[test]
fn a_skipped_copy_skips_what_goes_in_it() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "c2", "parent": "page-1", "kind": "frame"}}]));
    let out = ok(&e, "design.apply", json!({"design_id": id, "lenient": true, "ops": [
        {"op": "copy", "id": "card", "as": "c2"}, {"op": "add", "node": {"id": "x", "parent": "c2", "kind": "rect"}},
    ]}));
    assert_eq!(out["skipped"], json!([{"at": 0, "id": "c2", "reason": "exists"}, {"at": 1, "id": "x", "reason": "parent_skipped"}]));
    assert!(node(&e, id, "x").is_none());
}
