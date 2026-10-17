//! `design.*` (BUS.md §10.18): design canvases through the bus — the node tree `design.apply`
//! builds, its refusals and its undo, who may tag a design with which terminal, the soft-delete
//! lifecycle, pictures, and a project removed with its designs.

mod common;

use common::{call, call_as, code, committed_repo, engine_with_project, err, ok, ok_as, refused};
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

#[test]
fn a_stale_base_rev_is_refused_with_the_current_one() {
    let (e, _root) = project();
    let id = create(&e);
    let ops = json!([{"op": "add", "node": {"parent": "page-1", "kind": "rect"}}]);
    ok(&e, "design.apply", json!({"design_id": id, "ops": ops, "base_rev": 1}));
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": ops, "base_rev": 1}));
    let error = err(&refused);
    assert_eq!(error.code, "design.stale");
    assert_eq!(error.details.as_ref().unwrap()["rev"], 2);
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
    let out = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "text", "text": "hi"}}]}));
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
fn a_pack_item_is_inserted_as_ordinary_nodes_and_undone_whole() {
    let (e, _root) = project();
    let id = sample(&e);
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([
        {"op": "insert", "item": "relay-ui/button-primary", "parent": "side", "x": 12, "y": 30, "id": "save", "set": {"label": {"text": "Save"}}},
        {"op": "insert", "item": "relay-ui/button-primary", "parent": "page-1"},
    ]));
    assert_eq!(out["ids"][0], "save");
    let second = out["ids"][1].as_str().unwrap().to_string();
    assert!(second.starts_with("button-primary-"), "{second}");
    let after = nodes(&e, id);
    let save = after.as_array().unwrap().iter().find(|n| n["id"] == "save").unwrap();
    assert_eq!((save["parent"].as_str(), save["x"].as_f64(), save["y"].as_f64()), (Some("side"), Some(12.0), Some(30.0)));
    let label = after.as_array().unwrap().iter().find(|n| n["id"] == "save.label").unwrap();
    assert_eq!((label["parent"].as_str(), label["text"].as_str()), (Some("save"), Some("Save")));
    // The undo is one delete per piece.
    assert_eq!(out["inverse"], json!([{"op": "delete", "id": second}, {"op": "delete", "id": "save"}]));
    apply(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before);

    // Refusals: an unknown item, a part that is not there, an id in use.
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "insert", "item": "relay-ui/nope", "parent": "page-1"}]}))), "design.kit_not_found");
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "insert", "item": "relay-ui/button-primary", "parent": "page-1", "set": {"nope": {"text": "x"}}}]}))), "design.node_invalid");
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "insert", "item": "relay-ui/button-primary", "parent": "page-1", "id": "card"}]}))), "design.node_exists");
    assert_eq!(nodes(&e, id), before);

    // design.kits lists every pack, roots only unless asked.
    let kits = ok(&e, "design.kits", json!({}))["kits"].clone();
    assert!(kits.as_array().unwrap().len() >= 5);
    assert!(kits.as_array().unwrap().iter().all(|k| k["items"].as_array().unwrap().iter().all(|i| i["nodes"].as_array().unwrap().len() == 1)));
    let ui = ok(&e, "design.kits", json!({"kit": "relay-ui", "with_nodes": true}))["kits"].clone();
    assert_eq!(ui.as_array().unwrap().len(), 1);
    assert!(ui[0]["items"].as_array().unwrap().iter().any(|i| i["nodes"].as_array().unwrap().len() > 1));
}

#[test]
fn a_design_starts_from_a_template() {
    let (e, _root) = project();
    let template = relay_bus::kits::kits().iter().find(|k| k.id == "templates").unwrap().items[0].clone();
    let made = ok(&e, "design.create", json!({"project_id": 1, "title": "From a template", "template": format!("templates/{}", template.id)}));
    let all = made["nodes"].as_array().unwrap();
    assert_eq!(all.len(), 1 + template.nodes.len());
    assert_eq!(made["design"]["nodes"].as_i64(), Some(all.len() as i64));
    assert_eq!(nodes(&e, made["design"]["id"].as_i64().unwrap()), made["nodes"]);
    assert_eq!(code(call(&e, "design.create", json!({"project_id": 1, "template": "templates/nope"}))), "design.kit_not_found");
}

fn node(nodes: &Value, id: &str) -> Value {
    nodes.as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or(Value::Null)
}

#[test]
fn instances_follow_their_component_except_where_overridden() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "btn", "parent": "page-1", "kind": "frame", "w": 120, "h": 40, "fill": "#3366ff", "component": true}},
        {"op": "add", "node": {"id": "btn-label", "parent": "btn", "kind": "text", "text": "Go", "fill": "#ffffff"}},
    ]));
    let made = apply(&e, id, json!([
        {"op": "insert", "item": "#btn", "parent": "page-1", "x": 200, "y": 0, "id": "a"},
        {"op": "insert", "item": "#btn", "parent": "page-1", "x": 400, "y": 0, "id": "b", "set": {"btn-label": {"text": "Stop"}}},
    ]));
    let all = nodes(&e, id);
    assert_eq!((node(&all, "a")["instance_of"].as_str(), node(&all, "a")["master"].as_str(), node(&all, "a")["x"].as_f64()), (Some("btn"), Some("btn"), Some(200.0)));
    assert_eq!((node(&all, "a.1")["master"].as_str(), node(&all, "a.1")["text"].as_str()), (Some("btn-label"), Some("Go")));
    assert_eq!((node(&all, "b.1")["text"].as_str(), node(&all, "b.1")["overridden"].clone()), (Some("Stop"), json!(["text"])));
    assert!(node(&all, "a")["component"].is_null(), "a copy is no component itself");
    assert_eq!(made["inverse"], json!([{"op": "delete", "id": "b"}, {"op": "delete", "id": "a"}]));

    // The component changes: both copies follow, b keeps its own text, and the places stay put.
    let out = apply(&e, id, json!([
        {"op": "update", "id": "btn", "set": {"fill": "#ff3366", "x": 9}},
        {"op": "update", "id": "btn-label", "set": {"text": "Next", "font_size": 18}},
    ]));
    let all = nodes(&e, id);
    assert_eq!((node(&all, "a")["fill"].as_str(), node(&all, "a")["x"].as_f64()), (Some("#ff3366"), Some(200.0)));
    assert_eq!((node(&all, "a.1")["text"].as_str(), node(&all, "a.1")["font_size"].as_f64()), (Some("Next"), Some(18.0)));
    assert_eq!((node(&all, "b.1")["text"].as_str(), node(&all, "b.1")["font_size"].as_f64()), (Some("Stop"), Some(18.0)));
    assert!(node(&all, "a.1")["overridden"].is_null(), "following is no override");
    let changed = ids(&out["nodes"]);
    assert!(changed.contains(&"a.1".to_string()) && changed.contains(&"b".to_string()), "copies are in the answer: {changed:?}");
    // Its undo puts the component back, and the copies follow it back un-overridden.
    apply(&e, id, out["inverse"].clone());
    let all = nodes(&e, id);
    assert_eq!((node(&all, "a")["fill"].as_str(), node(&all, "a.1")["text"].as_str()), (Some("#3366ff"), Some("Go")));
    assert!(node(&all, "a.1")["overridden"].is_null() && node(&all, "a")["overridden"].is_null());

    // A part added to the component is added to every copy; one removed leaves them.
    apply(&e, id, json!([{"op": "add", "node": {"id": "btn-icon", "parent": "btn", "kind": "icon", "icon": "check", "w": 16, "h": 16}}]));
    let all = nodes(&e, id);
    let copies: Vec<&Value> = all.as_array().unwrap().iter().filter(|n| n["master"] == "btn-icon").collect();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().all(|c| c["parent"] == "a" || c["parent"] == "b"));
    apply(&e, id, json!([{"op": "delete", "id": "btn-icon"}]));
    assert!(!nodes(&e, id).as_array().unwrap().iter().any(|n| n["master"] == "btn-icon"));

    // A hand edit of a copy is an override; its undo takes the override back too.
    let out = apply(&e, id, json!([{"op": "update", "id": "a.1", "set": {"fill": "#000000"}}]));
    assert_eq!(node(&nodes(&e, id), "a.1")["overridden"], json!(["fill"]));
    apply(&e, id, out["inverse"].clone());
    assert!(node(&nodes(&e, id), "a.1")["overridden"].is_null());

    // Resetting overrides brings the component's values back.
    apply(&e, id, json!([{"op": "update", "id": "b.1", "set": {"overridden": null}}]));
    assert_eq!(node(&nodes(&e, id), "b.1")["text"], "Go");

    // Detached, a copy goes its own way.
    apply(&e, id, json!([{"op": "update", "id": "a", "set": {"instance_of": null}}]));
    apply(&e, id, json!([{"op": "update", "id": "btn", "set": {"fill": "#00ff00"}}]));
    let all = nodes(&e, id);
    assert_eq!((node(&all, "a")["fill"].as_str(), node(&all, "b")["fill"].as_str()), (Some("#3366ff"), Some("#00ff00")));

    // Only a component makes copies.
    assert_eq!(code(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "insert", "item": "#a", "parent": "page-1"}]}))), "design.not_component");
}

#[test]
fn a_component_inside_a_component_follows_through_both() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "chip", "parent": "page-1", "kind": "frame", "w": 40, "h": 20, "fill": "#111111", "component": true}},
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 100, "w": 200, "h": 100, "component": true}},
        {"op": "insert", "item": "#chip", "parent": "card", "id": "card-chip"},
        {"op": "insert", "item": "#card", "parent": "page-1", "x": 400, "id": "c"},
    ]));
    let all = nodes(&e, id);
    let inner = all.as_array().unwrap().iter().find(|n| n["parent"] == "c").unwrap().clone();
    assert_eq!((inner["master"].as_str(), inner["instance_of"].as_str()), (Some("card-chip"), Some("chip")));
    // The innermost component changes; its copy in the card follows, and the card's copy in turn.
    apply(&e, id, json!([{"op": "update", "id": "chip", "set": {"fill": "#ee0000"}}]));
    let all = nodes(&e, id);
    assert_eq!(node(&all, "card-chip")["fill"], "#ee0000");
    assert_eq!(node(&all, inner["id"].as_str().unwrap())["fill"], "#ee0000");
}

#[test]
fn shared_styles_lend_their_fields_until_set_by_hand() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "styles", "kind": "page", "name": "Styles"}},
        {"op": "add", "node": {"id": "brand", "parent": "styles", "kind": "rect", "name": "Brand / Primary", "fill": "#3366ff", "shared": true}},
        {"op": "add", "node": {"id": "h1", "parent": "styles", "kind": "text", "name": "Heading 1", "text": "Heading 1", "font_size": 32, "font_weight": 700, "shared": true}},
        {"op": "add", "node": {"id": "lift", "parent": "styles", "kind": "rect", "name": "Lift", "shadow": {"y": 4, "blur": 12, "color": "#00000033"}, "shared": true}},
        {"op": "add", "node": {"id": "cta", "parent": "page-1", "kind": "rect", "fill_style": "brand", "effect_style": "lift"}},
        {"op": "add", "node": {"id": "title", "parent": "page-1", "kind": "text", "text": "Hello", "text_style": "h1"}},
    ]));
    let all = nodes(&e, id);
    assert_eq!(node(&all, "cta")["fill"], "#3366ff", "added with a style, it takes the style's fill");
    assert_eq!(node(&all, "cta")["shadow"]["blur"], 12.0);
    assert_eq!((node(&all, "title")["font_size"].as_f64(), node(&all, "title")["font_weight"].as_u64()), (Some(32.0), Some(700)));

    // The style changes; its users follow, and the undo puts both back.
    let out = apply(&e, id, json!([{"op": "update", "id": "brand", "set": {"fill": "#ff3366"}}, {"op": "update", "id": "h1", "set": {"font_size": 40}}]));
    let all = nodes(&e, id);
    assert_eq!((node(&all, "cta")["fill"].as_str(), node(&all, "title")["font_size"].as_f64()), (Some("#ff3366"), Some(40.0)));
    assert!(ids(&out["nodes"]).contains(&"cta".to_string()));
    apply(&e, id, out["inverse"].clone());
    let all = nodes(&e, id);
    assert_eq!((node(&all, "cta")["fill"].as_str(), node(&all, "title")["font_size"].as_f64()), (Some("#3366ff"), Some(32.0)));

    // A node takes a style up later; undone, it has its own fill back.
    apply(&e, id, json!([{"op": "add", "node": {"id": "plain", "parent": "page-1", "kind": "rect", "fill": "#00ff00"}}]));
    let out = apply(&e, id, json!([{"op": "update", "id": "plain", "set": {"fill_style": "brand"}}]));
    assert_eq!(node(&nodes(&e, id), "plain")["fill"], "#3366ff");
    apply(&e, id, out["inverse"].clone());
    let plain = node(&nodes(&e, id), "plain");
    assert_eq!((plain["fill"].as_str(), plain["fill_style"].clone()), (Some("#00ff00"), Value::Null));

    // A fill set by hand lets go of the style (and its undo takes the style back).
    let out = apply(&e, id, json!([{"op": "update", "id": "cta", "set": {"fill": "#000000"}}]));
    assert!(node(&nodes(&e, id), "cta")["fill_style"].is_null());
    apply(&e, id, json!([{"op": "update", "id": "brand", "set": {"fill": "#123456"}}]));
    assert_eq!(node(&nodes(&e, id), "cta")["fill"], "#000000");
    assert_eq!(node(&nodes(&e, id), "cta")["effect_style"], "lift", "the other styles stay");
    apply(&e, id, out["inverse"].clone());
    assert_eq!(node(&nodes(&e, id), "cta")["fill_style"], "brand");
}

#[test]
fn comments_are_signed_by_who_wrote_them() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "c1", "parent": "page-1", "kind": "comment", "x": 40, "y": 40, "text": "Bigger title?"}}]));
    assert_eq!(node(&nodes(&e, id), "c1")["author"], "user");
    apply(&e, id, json!([{"op": "update", "id": "c1", "set": {"replies": [{"text": "Done", "author": "keen-raven"}, {"text": "Thanks"}], "resolved": true}}]));
    let c = node(&nodes(&e, id), "c1");
    assert_eq!((c["replies"][0]["author"].as_str(), c["replies"][1]["author"].as_str(), c["resolved"].as_bool()), (Some("keen-raven"), Some("user"), Some(true)));
}

#[test]
fn vectors_crops_and_the_richer_fields_are_checked() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "v", "parent": "page-1", "kind": "vector", "w": 100, "h": 80, "d": "M0 0 C 20 -20 80 -20 100 0 L 50 80 Z", "view": [100, 80]}},
        {"op": "add", "node": {"id": "t", "parent": "page-1", "kind": "text", "text": "Hi there", "spans": [{"text": "Hi ", "weight": 700}, {"text": "there", "link": "https://relay.dev"}], "list": "bullet"}},
        {"op": "add", "node": {"id": "f", "parent": "page-1", "kind": "frame", "w": 400, "h": 300, "grids": [{"kind": "columns", "count": 12, "gutter": 16, "margin": 24}], "start": true}},
        {"op": "add", "node": {"id": "r", "parent": "f", "kind": "rect", "constraints": {"h": "both", "v": "end"}, "flip_x": true, "goto": "f", "transition": "slide_left", "filters": {"saturation": -1}}},
    ]));
    for (bad, why) in [
        (json!({"parent": "page-1", "kind": "vector"}), "a vector needs d"),
        (json!({"parent": "page-1", "kind": "vector", "d": "L 0 0"}), "d:"),
        (json!({"parent": "page-1", "kind": "rect", "crop": [0.5, 0, 0.6, 1]}), "crop"),
        (json!({"parent": "page-1", "kind": "rect", "filters": {"hue": 400}}), "filters.hue"),
        (json!({"parent": "page-1", "kind": "rect", "grids": [{"kind": "columns", "count": 0}]}), "grids"),
        (json!({"parent": "page-1", "kind": "rect", "src": "../x.png"}), "src"),
    ] {
        let refused = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": bad}]})));
        assert!(refused.message.contains(why), "{why}: {}", refused.message);
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
