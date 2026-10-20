//! `design.*` (BUS.md §10.18): design canvases through the bus — the node tree `design.apply`
//! builds, its refusals and its undo, who may tag a design with which terminal, the soft-delete
//! lifecycle, pictures, and a project removed with its designs.

mod common;

use common::{call, call_as, code, committed_repo, engine_with_project, err, ok, ok_as};
use relay_bus::{Actor, Event};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::sync::Arc;

/// A 1×1 PNG.
const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// A store on disk (so pictures land beside it) with project 1.
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

fn nodes(e: &Engine, design: i64) -> Value {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].clone()
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn count(e: &Engine, sql: &str) -> i64 {
    e.store.lock().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

/// A page holding a laid-out card (a title, a photo, a group with a dot in it) and a side
/// frame, and a second page with a sticky.
fn sample(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 40, "y": 40, "w": 320, "h": 200, "layout": "column", "gap": 8, "padding": [16]}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hello", "font_size": 20}},
        {"op": "add", "node": {"id": "photo", "parent": "card", "kind": "rect", "w": 100, "h": 60, "fill": "#ff0000"}},
        {"op": "add", "node": {"id": "inner", "parent": "card", "kind": "group"}},
        {"op": "add", "node": {"id": "dot", "parent": "inner", "kind": "ellipse", "w": 8, "h": 8}},
        {"op": "add", "node": {"id": "side", "parent": "page-1", "kind": "frame", "x": 400, "w": 200, "h": 400}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Page 2"}},
        {"op": "add", "node": {"id": "note", "parent": "page-2", "kind": "sticky", "text": "remember"}},
    ]));
    id
}

#[test]
fn a_new_design_has_one_page_and_lists_newest_first() {
    let (e, _root) = project();
    let made = ok(&e, "design.create", json!({"project_id": 1, "title": "  Home  "}));
    let design = &made["design"];
    assert_eq!((design["title"].as_str(), design["rev"].as_i64(), design["pages"].as_i64(), design["nodes"].as_i64()),
        (Some("Home"), Some(1), Some(1), Some(1)));
    assert_eq!((design["created_by"].as_str(), design["session"].clone()), (Some("user"), Value::Null));
    assert_eq!(made["nodes"], json!([{"id": "page-1", "kind": "page", "name": "Page 1", "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0}]));
    assert!(made["assets_dir"].as_str().unwrap().ends_with("design-assets"));
    assert_eq!(nodes(&e, design["id"].as_i64().unwrap()), made["nodes"]);

    let second = ok(&e, "design.create", json!({"project_id": 1}))["design"].clone();
    assert_eq!(second["title"], "Untitled design");
    let listed = ok(&e, "design.list", json!({"project_id": 1}))["designs"].clone();
    assert_eq!(listed.as_array().unwrap().iter().map(|d| d["id"].clone()).collect::<Vec<_>>(), vec![second["id"].clone(), design["id"].clone()]);

    assert_eq!(code(call(&e, "design.create", json!({"project_id": 1, "title": "x".repeat(201)}))), "design.title");
    assert_eq!(code(call(&e, "design.create", json!({"project_id": 1, "title": " "}))), "design.title");
    assert_eq!(code(call(&e, "design.get", json!({"design_id": 99}))), "design.not_found");
}

#[test]
fn apply_adds_nodes_with_given_and_assigned_ids_in_paint_order() {
    let (e, _root) = project();
    let id = create(&e);
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 200}},
        {"op": "add", "node": {"parent": "card", "kind": "text", "text": "Top"}},
        {"op": "add", "node": {"parent": "card", "kind": "rect", "w": 10, "h": 10}, "index": 0},
        {"op": "add", "node": {"kind": "page"}},
        {"op": "add", "node": {"id": "frame-5", "parent": "page-1", "kind": "frame"}},
        {"op": "add", "node": {"parent": "page-1", "kind": "frame"}},
    ]));
    // The counter starts past page-1 and skips an id a caller already took.
    assert_eq!(out["ids"], json!(["card", "text-2", "rect-3", "page-4", "frame-5", "frame-6"]));
    assert_eq!((out["prev_rev"].as_i64(), out["design"]["rev"].as_i64()), (Some(1), Some(2)));
    assert_eq!((out["design"]["pages"].as_i64(), out["design"]["nodes"].as_i64()), (Some(2), Some(7)));
    assert_eq!(out["removed"], json!([]));
    assert_eq!(ids(&out["nodes"]), ["card", "text-2", "rect-3", "page-4", "frame-5", "frame-6"]);
    assert_eq!(out["inverse"][0], json!({"op": "delete", "id": "frame-6"}), "the undo runs last edit first");

    // Pages first, then each page's tree in pre-order, siblings bottom to top.
    let got = nodes(&e, id);
    assert_eq!(ids(&got), ["page-1", "page-4", "card", "rect-3", "text-2", "frame-5", "frame-6"]);
    assert_eq!(got[3]["parent"], "card");
    // One page and what is on it.
    let page = ok(&e, "design.get", json!({"design_id": id, "page": "page-4"}));
    assert_eq!(ids(&page["nodes"]), ["page-4"]);
    assert_eq!(code(call(&e, "design.get", json!({"design_id": id, "page": "card"}))), "design.page_not_found");
    assert_eq!(code(call(&e, "design.get", json!({"design_id": id, "page": "nope"}))), "design.page_not_found");
    // The next assigned id carries on from the stored counter.
    let next = apply(&e, id, json!([{"op": "add", "node": {"parent": "card", "kind": "ellipse"}}]));
    assert_eq!(next["ids"], json!(["ellipse-7"]));
}

#[test]
fn update_move_and_delete_change_the_tree() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([
        {"op": "update", "id": "photo", "set": {"fill": "#00ff00", "radius": 4, "width": 120}},
        {"op": "move", "id": "dot", "parent": "side", "index": 0},
        {"op": "move", "id": "title", "index": 99},
        {"op": "move", "id": "page-2", "index": 0},
        {"op": "delete", "id": "card"},
    ]));
    assert_eq!(out["ids"], json!(["photo", "dot", "title", "page-2", "card"]));
    assert_eq!(out["removed"], json!(["card", "photo", "inner", "title"]), "the subtree, parent first, title moved on top");
    assert_eq!(ids(&out["nodes"]), ["dot", "page-2"], "only what still exists");
    assert_eq!(out["nodes"][0]["parent"], "side");
    assert_eq!((out["design"]["nodes"].as_i64(), out["design"]["pages"].as_i64()), (Some(5), Some(2)));
    assert_eq!(ids(&nodes(&e, id)), ["page-2", "page-1", "note", "side", "dot"]);

    let photo = apply(&e, sample(&e), json!([{"op": "update", "id": "photo", "set": {"fill": null, "width": 120}}]));
    assert_eq!(photo["nodes"][0]["w"], 120.0, "width is w");
    assert!(photo["nodes"][0].get("fill").is_none(), "null puts a field back to its default");
    assert_eq!(photo["inverse"], json!([{"op": "update", "id": "photo", "set": {"fill": "#ff0000", "w": 100.0}}]));
}

/// Whatever a list does, its `inverse` puts every node back as it was — by `design.apply`, as
/// the canvas undoes, and by `audit.undo`.
#[test]
fn the_inverse_restores_exactly_what_was_there() {
    let (e, _root) = project();
    let id = sample(&e);
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "badge", "parent": "card", "kind": "rect", "w": 10, "h": 10}, "index": 1},
        {"op": "update", "id": "title", "set": {"text": "Hi", "font_size": 24, "width": 300}},
        {"op": "update", "id": "photo", "set": {"fill": null, "radius": 8}},
        {"op": "move", "id": "inner", "parent": "side", "index": 0},
        {"op": "move", "id": "page-2", "index": 0},
        {"op": "delete", "id": "card"},
        {"op": "move", "id": "dot", "parent": "page-1"},
        {"op": "delete", "id": "page-2"},
        {"op": "add", "node": {"parent": "page-1", "kind": "text", "text": "new"}},
    ]));
    let removed: Vec<&str> = out["removed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(removed, ["card", "title", "badge", "photo", "page-2", "note"]);
    let after = nodes(&e, id);
    assert_eq!(ids(&after), ["page-1", "side", "inner", "dot", "text-2"]);
    let undone = apply(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before, "the inverse of {}", out["inverse"]);
    // And the inverse of the inverse is the list again.
    apply(&e, id, undone["inverse"].clone());
    assert_eq!(nodes(&e, id), after);

    // audit.undo replays the recorded inverse.
    let id = sample(&e);
    let before = nodes(&e, id);
    apply(&e, id, json!([{"op": "delete", "id": "card"}, {"op": "update", "id": "side", "set": {"fill": "#123456"}}]));
    let row = ok(&e, "audit.list", json!({"op_prefix": "design.apply", "limit": 1}))["rows"][0].clone();
    assert_eq!(row["undo_op"]["op"], "design.apply");
    assert_eq!(row["undo_op"]["payload"]["lenient"], true);
    ok(&e, "audit.undo", json!({"audit_id": row["id"]}));
    assert_eq!(nodes(&e, id), before);

    // A delete whose undo would re-add hundreds of KB of nodes keeps no undo in the audit (its
    // caller still has `inverse`): every node of it would otherwise be stored a second time.
    let id = sample(&e);
    let words = "x".repeat(400);
    let mut ops = vec![json!({"op": "add", "node": {"id": "big", "parent": "page-1", "kind": "frame", "w": 100, "h": 100}})];
    ops.extend((0..900).map(|i| json!({"op": "add", "node": {"parent": "big", "kind": "text", "text": words.clone(), "y": i}})));
    apply(&e, id, json!(ops));
    let out = apply(&e, id, json!([{"op": "delete", "id": "big"}]));
    assert_eq!(out["inverse"].as_array().unwrap().len(), 901);
    let row = ok(&e, "audit.list", json!({"op_prefix": "design.apply", "limit": 1}))["rows"][0].clone();
    assert!(row["undo_op"].is_null(), "an oversized undo stays out of the audit: {}", row["undo_op"]);
}

#[test]
fn a_lenient_list_skips_edits_of_nodes_that_are_gone() {
    let (e, _root) = project();
    let id = sample(&e);
    let ops = json!([
        {"op": "update", "id": "ghost", "set": {"x": 1}},
        {"op": "move", "id": "ghost", "index": 0},
        {"op": "delete", "id": "ghost"},
        {"op": "add", "node": {"id": "orphan", "parent": "ghost", "kind": "rect"}},
        {"op": "update", "id": "photo", "set": {"x": 5}},
    ]);
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": ops}))), "design.node_not_found");
    let out = ok(&e, "design.apply", json!({"design_id": id, "ops": ops, "lenient": true}));
    assert_eq!(out["ids"], json!(["", "", "", "", "photo"]));
    assert_eq!(out["inverse"], json!([{"op": "update", "id": "photo", "set": {"x": 0.0}}]));
}

/// Deleting a frame of more nodes than one list may hold: its undo re-adds each, and still
/// runs as one lenient list, from the canvas and from the audit.
#[test]
fn a_delete_bigger_than_a_list_is_undone_in_one() {
    let (e, _root) = project();
    let id = create(&e);
    let mut ops = vec![json!({"op": "add", "node": {"id": "big", "parent": "page-1", "kind": "frame", "w": 100, "h": 100}})];
    ops.extend((0..1999).map(|_| json!({"op": "add", "node": {"parent": "big", "kind": "rect"}})));
    apply(&e, id, json!(ops));
    apply(&e, id, json!((0..200).map(|_| json!({"op": "add", "node": {"parent": "big", "kind": "rect"}})).collect::<Vec<_>>()));
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([{"op": "delete", "id": "big"}]));
    let inverse = out["inverse"].clone();
    assert_eq!(inverse.as_array().unwrap().len(), 2200);
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": inverse}))), "design.ops", "a list a person sends");
    let undone = ok(&e, "design.apply", json!({"design_id": id, "ops": inverse, "lenient": true}));
    assert_eq!(nodes(&e, id), before);
    // And redone, as one lenient list too.
    ok(&e, "design.apply", json!({"design_id": id, "ops": undone["inverse"], "lenient": true}));
    assert_eq!(ids(&nodes(&e, id)), ["page-1"]);

    let row = ok(&e, "audit.list", json!({"op_prefix": "design.apply", "limit": 1}))["rows"][0].clone();
    assert_eq!(row["undo_op"]["payload"]["ops"].as_array().map(Vec::len), Some(2200));
    ok(&e, "audit.undo", json!({"audit_id": row["id"]}));
    assert_eq!(nodes(&e, id), before);
}

/// A connector attached to a node that is deleted lets go of that end where it was drawn, so
/// none names a node that is gone; the undo attaches it again.
#[test]
fn deleting_a_shape_frees_the_connectors_attached_to_it() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "box", "parent": "page-1", "kind": "group", "x": 100, "y": 100}},
        {"op": "add", "node": {"id": "a", "parent": "box", "kind": "rect", "x": 0, "y": 0, "w": 20, "h": 20}},
        {"op": "add", "node": {"id": "b", "parent": "page-1", "kind": "rect", "x": 300, "y": 100, "w": 20, "h": 40}},
        {"op": "add", "node": {"id": "joined", "parent": "page-1", "kind": "line", "x": 100, "y": 100, "from": "a", "to": "b"}},
        {"op": "add", "node": {"id": "drawn", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "b", "to": "a"}},
        {"op": "add", "node": {"id": "other", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "b"}},
    ]));
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([{"op": "delete", "id": "box"}]));
    assert_eq!(out["removed"], json!(["box", "a"]));
    let got = nodes(&e, id);
    let find = |id: &str| got.as_array().unwrap().iter().find(|n| n["id"] == id).unwrap().clone();
    let joined = find("joined");
    assert!(joined.get("from").is_none() && joined["to"] == "b", "{joined}");
    // From a's centre (110, 110) to b's (310, 120), counted from the line's own (100, 100).
    assert_eq!(joined["points"], json!([[10.0, 10.0], [210.0, 20.0]]));
    // TM-60: one drawn with points of its own is drawn again from the stored centres, b's
    // (310, 120) to where a was (110, 110), not along a route it was first given.
    let drawn = find("drawn");
    assert!(drawn["from"] == "b" && drawn.get("to").is_none() && drawn["points"] == json!([[310.0, 120.0], [110.0, 110.0]]), "{drawn}");
    assert_eq!(find("other"), before.as_array().unwrap().iter().find(|n| n["id"] == "other").unwrap().clone(), "not attached to it");
    let changed: Vec<String> = ids(&out["nodes"]);
    assert!(changed.contains(&"joined".to_string()) && changed.contains(&"drawn".to_string()), "the answer carries them: {changed:?}");
    // A node deleted and added back in the same list keeps its connectors.
    let again = apply(&e, id, json!([{"op": "delete", "id": "b"}, {"op": "add", "node": {"id": "b", "parent": "page-1", "kind": "rect"}}]));
    assert_eq!(ids(&again["nodes"]), ["b"]);
    apply(&e, id, again["inverse"].clone());

    apply(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before, "the undo attaches them again");
}

/// An add-only list sent against an older rev writes no node that changed since, so it applies
/// on top (W6); `design_revs.rs` holds the refusals of a list that does write one.
#[test]
fn an_add_only_list_against_an_older_rev_applies() {
    let (e, _root) = project();
    let id = create(&e);
    let ops = json!([{"op": "add", "node": {"parent": "page-1", "kind": "rect"}}]);
    ok(&e, "design.apply", json!({"design_id": id, "ops": ops, "base_rev": 1}));
    let again = ok(&e, "design.apply", json!({"design_id": id, "ops": ops, "base_rev": 1}));
    assert_eq!((again["rev"].as_i64(), again["prev_rev"].as_i64(), again["rebased"].clone()), (Some(3), Some(2), json!(true)));
    assert_eq!(nodes(&e, id).as_array().unwrap().len(), 3);
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": []}))), "design.ops");
}

/// A refusal names the edit and the field, and nothing before it in the list stays applied.
#[test]
fn a_refused_edit_names_itself_and_rolls_back_the_list() {
    let (e, _root) = project();
    let id = create(&e);
    let before = ok(&e, "design.get", json!({"design_id": id}));
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "hero", "parent": "page-1", "kind": "frame"}},
        {"op": "add", "node": {"parent": "hero", "kind": "text", "text": "Hi"}},
        {"op": "update", "id": "hero", "set": {"fill": "red"}},
    ]}));
    let error = err(&refused);
    assert_eq!(error.code, "design.node_invalid");
    assert!(error.message.starts_with(r#"ops[2] update "hero": fill: "red""#), "{}", error.message);
    assert_eq!(error.details.as_ref().unwrap()["op"], 2);
    assert_eq!(ok(&e, "design.get", json!({"design_id": id})), before, "ops 0 and 1 rolled back, rev and counter too");

    let typo = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "rect", "w": -1}}]}));
    assert!(err(&typo).message.starts_with("ops[0] add rect: w:"), "{}", err(&typo).message);
}

#[test]
fn the_tree_keeps_its_shape() {
    let (e, _root) = project();
    let id = sample(&e);
    let refusal = |ops: Value| code(call(&e, "design.apply", json!({"design_id": id, "ops": ops})));
    // A design keeps one page.
    apply(&e, id, json!([{"op": "delete", "id": "page-2"}]));
    assert_eq!(refusal(json!([{"op": "delete", "id": "page-1"}])), "design.last_page");
    // Nothing goes inside itself or what it holds.
    assert_eq!(refusal(json!([{"op": "move", "id": "card", "parent": "inner"}])), "design.cycle");
    assert_eq!(refusal(json!([{"op": "move", "id": "card", "parent": "card"}])), "design.cycle");
    // Only pages, frames and groups hold nodes.
    assert_eq!(refusal(json!([{"op": "add", "node": {"parent": "photo", "kind": "rect"}}])), "design.parent");
    assert_eq!(refusal(json!([{"op": "move", "id": "dot", "parent": "title"}])), "design.parent");
    assert_eq!(refusal(json!([{"op": "add", "node": {"parent": "nowhere", "kind": "rect"}}])), "design.parent");
    // A page sits in nothing.
    assert_eq!(refusal(json!([{"op": "move", "id": "page-1", "parent": "card"}])), "design.parent");
    assert_eq!(refusal(json!([{"op": "add", "node": {"kind": "page", "parent": "card"}}])), "design.node_invalid");
    assert_eq!(refusal(json!([{"op": "add", "node": {"kind": "rect"}}])), "design.node_invalid");
    // Ids are unique, and an update changes neither kind nor parent.
    assert_eq!(refusal(json!([{"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "rect"}}])), "design.node_exists");
    assert_eq!(refusal(json!([{"op": "update", "id": "card", "set": {"kind": "rect"}}])), "design.node_invalid");
    assert_eq!(refusal(json!([{"op": "update", "id": "card", "set": {"parent": "side"}}])), "design.node_invalid");
}

#[test]
fn siblings_keep_their_order_through_many_inserts_at_one_place() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "a", "parent": "page-1", "kind": "rect"}},
        {"op": "add", "node": {"id": "z", "parent": "page-1", "kind": "rect"}},
    ]));
    // Each lands between "a" and the one before it, halving the gap until it is spent.
    let ops: Vec<Value> = (0..80).map(|i| json!({"op": "add", "node": {"id": format!("n{i:02}"), "parent": "page-1", "kind": "rect"}, "index": 1})).collect();
    apply(&e, id, Value::Array(ops));
    let mut want = vec!["page-1".to_string(), "a".to_string()];
    want.extend((0..80).rev().map(|i| format!("n{i:02}")));
    want.push("z".into());
    assert_eq!(ids(&nodes(&e, id)), want);
}

#[test]
fn agents_tag_their_own_terminal_and_the_person_any_of_the_project() {
    let (e, _root) = project();
    let a = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let b = ok(&e, "session.create", json!({"project_id": 1, "provider": "codex", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let agent = Actor::agent(&a);

    let mine = ok_as(&e, agent.clone(), "design.create", json!({"title": "Mine"}))["design"].clone();
    assert_eq!((mine["session"].as_str(), mine["created_by"].as_str(), mine["project_id"].as_i64()), (Some(a.as_str()), Some(a.as_str()), Some(1)));
    let id = mine["id"].as_i64().unwrap();
    let out = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "text", "text": "hi"}}], "reply": "full"}));
    assert_eq!(out["design"]["updated_by"], a.as_str());
    assert_eq!(code(call_as(&e, agent.clone(), "design.create", json!({"session": b}))), "actor.scope");
    assert_eq!(code(call_as(&e, agent.clone(), "design.update", json!({"design_id": id, "session": b}))), "actor.scope");
    // An agent deletes, but only the person purges.
    ok_as(&e, agent.clone(), "design.delete", json!({"design_id": id}));
    assert_eq!(code(call_as(&e, agent.clone(), "design.purge", json!({"design_id": id}))), "actor.allowlist");

    let theirs = ok(&e, "design.create", json!({"project_id": 1, "session": b}))["design"].clone();
    assert_eq!((theirs["session"].as_str(), theirs["created_by"].as_str()), (Some(b.as_str()), Some("user")));
    assert_eq!(code(call(&e, "design.create", json!({"project_id": 1, "session": "nobody"}))), "design.session");
    // A list for one terminal, which an agent may read for a peer.
    let listed = ok_as(&e, agent, "design.list", json!({"session": b}))["designs"].clone();
    assert_eq!(listed.as_array().unwrap().iter().map(|d| d["id"].clone()).collect::<Vec<_>>(), vec![theirs["id"].clone()]);
    let untied = ok(&e, "design.update", json!({"design_id": theirs["id"], "session": null, "title": "Shared"}));
    assert_eq!((untied["session"].clone(), untied["title"].as_str(), untied["rev"].as_i64()), (Value::Null, Some("Shared"), Some(2)));
}

/// `design.*` is open to every role, whatever its saved allowlist says: a list saved before
/// designs existed (the person's own, which replaces the defaults) must not shut agents out.
/// Purging stays the person's.
#[test]
fn every_role_may_design_whatever_its_saved_allowlist() {
    let (e, _root) = project();
    ok(&e, "guardrail.config.set", json!({"patch": {"roles": {
        "builder": ["mailbox.*", "session.done"], "reviewer": ["mailbox.*"], "docs": ["notes.append"]}}}));
    for role in ["builder", "reviewer", "docs"] {
        let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": role}))["name"].as_str().unwrap().to_string();
        let agent = Actor::agent(&name);
        let id = ok_as(&e, agent.clone(), "design.create", json!({"title": role}))["design"]["id"].as_i64().unwrap();
        ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "rect"}}]}));
        let listed = ok_as(&e, agent.clone(), "bus.ops", json!({}));
        let callable = |op: &str| listed["ops"].as_array().unwrap().iter().find(|o| o["name"] == op).map(|o| o["call"].clone());
        assert_eq!(callable("design.apply"), Some(json!("yes")), "{role}: {listed}");
        // Something the saved list leaves out stays refused.
        assert_eq!(code(call_as(&e, agent.clone(), "notes.create", json!({"project_id": 1, "body": "x"}))), "actor.allowlist", "{role}");
        ok_as(&e, agent.clone(), "design.delete", json!({"design_id": id}));
        assert_eq!(code(call_as(&e, agent, "design.purge", json!({"design_id": id}))), "actor.allowlist", "{role}");
    }
}

#[test]
fn a_deleted_design_is_listed_apart_restored_or_purged() {
    let (e, _root) = project();
    let id = sample(&e);
    ok(&e, "design.delete", json!({"design_id": id}));
    assert!(ok(&e, "design.list", json!({"project_id": 1}))["designs"].as_array().unwrap().is_empty());
    let all = ok(&e, "design.list", json!({"project_id": 1, "include_deleted": true}))["designs"].clone();
    assert!(all[0]["deleted_at"].is_string());
    assert_eq!(ok(&e, "design.get", json!({"design_id": id}))["nodes"].as_array().unwrap().len(), 9, "a deleted design still reads");
    let edit = json!({"design_id": id, "ops": [{"op": "delete", "id": "card"}]});
    assert_eq!(code(call(&e, "design.apply", edit.clone())), "design.deleted");
    assert_eq!(code(call(&e, "design.delete", json!({"design_id": id}))), "design.deleted");

    let back = ok(&e, "design.restore", json!({"design_id": id}));
    assert_eq!(back["deleted_at"], Value::Null);
    assert_eq!(code(call(&e, "design.restore", json!({"design_id": id}))), "design.not_deleted");
    ok(&e, "design.apply", edit);

    assert_eq!(code(call(&e, "design.purge", json!({"design_id": id}))), "design.not_deleted");
    ok(&e, "design.delete", json!({"design_id": id}));
    ok(&e, "design.purge", json!({"design_id": id}));
    assert_eq!(code(call(&e, "design.get", json!({"design_id": id}))), "design.not_found");
    assert_eq!(count(&e, "SELECT COUNT(*) FROM design_nodes"), 0);
}

#[test]
fn the_retention_pass_purges_designs_deleted_long_ago() {
    let (e, _root) = project();
    let (old, recent) = (sample(&e), sample(&e));
    for id in [old, recent] {
        ok(&e, "design.delete", json!({"design_id": id}));
    }
    e.store.lock().execute("UPDATE designs SET deleted_at = '2000-01-01T00:00:00Z' WHERE id = ?1", [old]).unwrap();
    let actions = ok(&e, "app.reconcile", json!({}))["actions"].clone();
    assert!(actions.as_array().unwrap().iter().any(|a| a == "purged 1 deleted design(s) past the undo window"), "{actions}");
    assert_eq!(code(call(&e, "design.get", json!({"design_id": old}))), "design.not_found");
    assert_eq!(count(&e, &format!("SELECT COUNT(*) FROM design_nodes WHERE design_id = {old}")), 0);
    assert_eq!(ok(&e, "design.restore", json!({"design_id": recent}))["nodes"], 9);
}

#[test]
fn a_duplicate_copies_every_node_and_goes_its_own_way() {
    let (e, _root) = project();
    let id = sample(&e);
    let copy = ok(&e, "design.duplicate", json!({"design_id": id}));
    let copy_id = copy["id"].as_i64().unwrap();
    assert_ne!(copy_id, id);
    assert_eq!((copy["title"].as_str(), copy["rev"].as_i64(), copy["nodes"].as_i64(), copy["pages"].as_i64()), (Some("Home copy"), Some(1), Some(9), Some(2)));
    assert_eq!(nodes(&e, copy_id), nodes(&e, id));
    // Its counter came along: the next assigned id is the same in both.
    let next = |design: i64| apply(&e, design, json!([{"op": "add", "node": {"parent": "page-1", "kind": "rect"}}]))["ids"][0].clone();
    assert_eq!(next(copy_id), next(id));
    apply(&e, copy_id, json!([{"op": "delete", "id": "card"}]));
    assert!(ids(&nodes(&e, id)).contains(&"card".to_string()));
    let named = ok(&e, "design.duplicate", json!({"design_id": id, "title": "Variant B"}));
    assert_eq!(named["title"], "Variant B");
}

#[test]
fn changes_are_announced_with_their_project() {
    let (e, _root) = project();
    let mut rx = e.subscribe();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"parent": "page-1", "kind": "rect"}}]));
    ok(&e, "design.delete", json!({"design_id": id}));
    ok(&e, "design.purge", json!({"design_id": id}));
    let seen = drain(&mut rx);
    let changed: Vec<&Event> = seen.iter().filter(|ev| ev.ev == "design.changed").collect();
    assert_eq!(changed.len(), 3, "{seen:?}");
    assert!(changed.iter().all(|ev| ev.project_id == Some(1) && ev.payload["id"] == id));
    assert_eq!(changed[1].payload["rev"], 2);
    assert!(changed[2].payload["deleted_at"].is_string());
    let deleted = seen.iter().find(|ev| ev.ev == "design.deleted").unwrap();
    assert_eq!((deleted.project_id, deleted.payload.clone()), (Some(1), json!({"id": id, "project_id": 1})));
}

#[test]
fn a_picture_is_stored_once_by_its_content() {
    let (e, root) = project();
    let id = create(&e);
    let first = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": PNG_1X1}));
    let src = first["src"].as_str().unwrap();
    assert!(relay_bus::design::valid_src(src) && src.ends_with(".png"), "{src}");
    assert_eq!((first["w"].as_u64(), first["h"].as_u64()), (Some(1), Some(1)));
    let path = std::path::Path::new(first["path"].as_str().unwrap());
    assert_eq!(path.parent().unwrap(), root.path().join("store/design-assets"));
    assert!(path.is_file());
    let again = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": PNG_1X1}));
    assert_eq!(again["src"], src);
    // The same bytes from a file are the same picture.
    use base64::Engine as _;
    let file = root.path().join("dot.png");
    std::fs::write(&file, base64::engine::general_purpose::STANDARD.decode(PNG_1X1).unwrap()).unwrap();
    assert_eq!(ok(&e, "design.asset.add", json!({"design_id": id, "path": file}))["src"], src);
    // An image node takes it.
    apply(&e, id, json!([{"op": "add", "node": {"parent": "page-1", "kind": "image", "src": src, "w": 1, "h": 1}}]));

    let svg = base64::engine::general_purpose::STANDARD.encode(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 24"/>"#);
    let vector = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": svg}));
    assert_eq!((vector["src"].as_str().unwrap().ends_with(".svg"), vector["w"].as_u64(), vector["h"].as_u64()), (true, Some(48), Some(24)));

    let text = base64::engine::general_purpose::STANDARD.encode("just some text");
    assert_eq!(code(call(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": text}))), "design.asset");
    assert_eq!(code(call(&e, "design.asset.add", json!({"design_id": id}))), "design.asset");
    assert_eq!(code(call(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": PNG_1X1, "path": file}))), "design.asset");
    assert_eq!(code(call(&e, "design.asset.add", json!({"design_id": id, "path": "dot.png"}))), "design.asset");
    assert_eq!(code(call(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": "not base64!"}))), "design.asset");
}

#[test]
fn a_project_is_removed_and_restored_with_its_designs() {
    let (e, _root) = project();
    let id = sample(&e);
    let before = ok(&e, "design.get", json!({"design_id": id}));
    ok(&e, "design.create", json!({"project_id": 1, "title": "Second"}));
    ok(&e, "project.remove", json!({"project_id": 1}));
    assert_eq!((count(&e, "SELECT COUNT(*) FROM designs"), count(&e, "SELECT COUNT(*) FROM design_nodes")), (0, 0));

    let backup = ok(&e, "project.removed.list", json!({}))["removed"][0]["backup_path"].as_str().unwrap().to_string();
    ok(&e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    assert_eq!(ok(&e, "design.get", json!({"design_id": id})), before);
    assert_eq!(count(&e, "SELECT COUNT(*) FROM designs"), 2);
}

/// A removal backup taken before designs existed still restores the rest of the project.
#[test]
fn a_backup_from_before_designs_still_restores() {
    let (e, _root) = project();
    ok(&e, "notes.create", json!({"project_id": 1, "title": "plan", "body": "kept"}));
    ok(&e, "project.remove", json!({"project_id": 1}));
    let backup = ok(&e, "project.removed.list", json!({}))["removed"][0]["backup_path"].as_str().unwrap().to_string();
    rusqlite::Connection::open(&backup).unwrap().execute_batch("DROP TABLE design_nodes; DROP TABLE designs;").unwrap();
    let out = ok(&e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    assert_eq!(out["notes"], 1);
    assert!(ok(&e, "design.list", json!({"project_id": 1}))["designs"].as_array().unwrap().is_empty());
}

/// Timings for one `design.apply` of 500 nodes and a `design.get` of 5,000, through the
/// engine's ordinary call path on a store on disk. Run with
/// `cargo test --release -p relay-core --test design -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_apply_500_and_get_5000() {
    let (e, _root) = project();
    let batch = |prefix: &str, children: usize| -> Value {
        let mut ops = vec![json!({"op": "add", "node": {"id": prefix, "parent": "page-1", "kind": "frame", "w": 1200, "h": 900}})];
        ops.extend((0..children).map(|i| json!({"op": "add", "node": {
            "id": format!("{prefix}-{i}"), "parent": prefix, "kind": if i % 3 == 0 { "text" } else { "rect" },
            "x": i * 3, "y": i * 2, "w": 40, "h": 20, "fill": "#336699", "text": if i % 3 == 0 { Some("Label") } else { None },
        }})));
        Value::Array(ops)
    };
    let median = |mut runs: Vec<std::time::Duration>| { runs.sort(); runs[runs.len() / 2] };
    let mut runs = Vec::new();
    for n in 0..12 {
        let id = create(&e);
        let ops = batch("f", 499);
        let started = std::time::Instant::now();
        apply(&e, id, ops);
        if n >= 2 { runs.push(started.elapsed()); }
    }
    println!("design.apply, 1 frame + 499 children: median {:?} of {}", median(runs.clone()), runs.len());

    let id = create(&e);
    // The most a design holds: its page and 4,999 more.
    for k in 0..10 {
        apply(&e, id, batch(&format!("f{k}"), if k == 9 { 498 } else { 499 }));
    }
    let size = ok(&e, "design.get", json!({"design_id": id}));
    assert_eq!(size["nodes"].as_array().unwrap().len(), 5_000);
    let mut runs = Vec::new();
    for n in 0..12 {
        let started = std::time::Instant::now();
        let got = ok(&e, "design.get", json!({"design_id": id}));
        if n >= 2 { runs.push(started.elapsed()); }
        drop(got);
    }
    println!("design.get, 5,000 nodes: median {:?} of {}", median(runs.clone()), runs.len());
}
