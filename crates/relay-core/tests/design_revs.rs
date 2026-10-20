//! Revisions (TM-58): an edit that changes nothing makes no rev, event or undo.

mod common;

use common::{committed_repo, engine_with_project, ok};
use relay_bus::Event;
use relay_core::engine::Engine;
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

fn get(e: &Engine, payload: Value) -> Value {
    ok(e, "design.get", payload)
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn strings(list: &Value) -> Vec<String> {
    list.as_array().unwrap_or_else(|| panic!("not a list: {list}")).iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

fn rev(e: &Engine, design: i64) -> i64 {
    get(e, json!({"design_id": design, "pages_only": true}))["design"]["rev"].as_i64().unwrap()
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn changed_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Value> {
    drain(rx).into_iter().filter(|ev| ev.ev == "design.changed").map(|ev| ev.payload).collect()
}

/// The undo the audit kept for the last `op`, if any.
fn last_undo(e: &Engine, op: &str) -> Option<String> {
    e.store.lock().query_row("SELECT undo_op FROM audit WHERE op = ?1 AND kind = 'ok' ORDER BY id DESC LIMIT 1", [op], |r| r.get(0)).unwrap()
}

/// A page holding a laid-out card (a title, a photo, a group with a dot in it), a side frame
/// with a chip in it and a line from the dot to the chip, and a second page with a sticky.
/// The design is at rev 2.
fn sample(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 40, "y": 40, "w": 320, "h": 200, "layout": "column", "gap": 8}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hello", "font_size": 20}},
        {"op": "add", "node": {"id": "photo", "parent": "card", "kind": "rect", "w": 100, "h": 60, "fill": "#ff0000"}},
        {"op": "add", "node": {"id": "inner", "parent": "card", "kind": "group"}},
        {"op": "add", "node": {"id": "dot", "parent": "inner", "kind": "ellipse", "w": 8, "h": 8}},
        {"op": "add", "node": {"id": "side", "parent": "page-1", "kind": "frame", "x": 400, "w": 200, "h": 400}},
        {"op": "add", "node": {"id": "chip", "parent": "side", "kind": "rect", "x": 10, "y": 10, "w": 40, "h": 20}},
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "from": "dot", "to": "chip"}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Page 2"}},
        {"op": "add", "node": {"id": "note", "parent": "page-2", "kind": "sticky", "text": "remember"}},
    ]));
    id
}

// --- TM-58: what changes nothing makes no rev -------------------------------------------------

/// TM-58: a design.update that leaves the title and terminal as they are writes nothing: no
/// rev, no updated_at, no event, no undo. One that changes either still does all four.
#[test]
fn an_update_to_what_a_design_already_is_changes_nothing() {
    let (e, _root) = project();
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let id = create(&e);
    ok(&e, "design.update", json!({"design_id": id, "session": name}));
    let before = get(&e, json!({"design_id": id, "pages_only": true}))["design"].clone();
    let audit_rows = || -> i64 { e.store.lock().query_row("SELECT COUNT(*) FROM audit WHERE op = 'design.update' AND undo_op IS NOT NULL", [], |r| r.get(0)).unwrap() };
    let undos = audit_rows();
    let mut rx = e.subscribe();
    for same in [json!({"title": "Home"}), json!({"title": "  Home "}), json!({"session": name}), json!({"title": "Home", "session": name}), json!({})] {
        let mut payload = same.clone();
        payload["design_id"] = json!(id);
        let answer = ok(&e, "design.update", payload);
        assert_eq!(answer, before, "{same}");
    }
    assert_eq!(get(&e, json!({"design_id": id, "pages_only": true}))["design"], before);
    assert!(changed_events(&mut rx).is_empty());
    assert_eq!(audit_rows(), undos, "no undo kept");
    // A real change: a new rev, an event and an undo.
    let renamed = ok(&e, "design.update", json!({"design_id": id, "title": "Home 2"}));
    assert_eq!(renamed["rev"].as_i64(), before["rev"].as_i64().map(|rev| rev + 1));
    assert_eq!(changed_events(&mut rx).len(), 1);
    assert_eq!(audit_rows(), undos + 1);
    let untied = ok(&e, "design.update", json!({"design_id": id, "session": null}));
    assert_eq!(untied["session"], Value::Null);
    assert_eq!(ok(&e, "design.update", json!({"design_id": id, "session": null}))["rev"], untied["rev"], "untied already");
}

/// TM-58: an apply whose every edit finds what it sends already there makes no rev, event or
/// undo, and answers an empty undo; a list mixing one with a real change undoes only the change.
#[test]
fn an_apply_of_identical_sets_makes_no_rev() {
    let (e, _root) = project();
    let id = sample(&e);
    let now = rev(&e, id);
    let undo = last_undo(&e, "design.apply");
    let mut rx = e.subscribe();
    let applied = apply(&e, id, json!([
        {"op": "update", "id": "title", "set": {"text": "Hello", "font_size": 20.0}},
        {"op": "update", "id": "photo", "set": {"fill": "#ff0000", "w": 100}},
    ]));
    assert_eq!((applied["rev"].as_i64(), applied["prev_rev"].as_i64()), (Some(now), Some(now)));
    assert_eq!((applied["inverse"].clone(), applied["nodes"].clone(), applied["removed_count"].clone()), (json!([]), json!([]), json!(0)));
    assert_eq!(strings(&applied["ids"]), ["title", "photo"]);
    assert!(applied.get("audit_id").is_none());
    assert!(changed_events(&mut rx).is_empty());
    assert_eq!(rev(&e, id), now);
    // The audit row of this list keeps no undo.
    assert_eq!(last_undo(&e, "design.apply"), None);
    assert!(undo.is_some(), "the sample's list kept one");
    let mixed = apply(&e, id, json!([
        {"op": "update", "id": "title", "set": {"text": "Hello"}},
        {"op": "update", "id": "photo", "set": {"fill": "#00ff00"}},
    ]));
    assert_eq!(mixed["rev"].as_i64(), Some(now + 1));
    assert_eq!(mixed["inverse"], json!([{"op": "update", "id": "photo", "set": {"fill": "#ff0000"}}]));
    assert_eq!(ids(&mixed["nodes"]), ["photo"]);
}
