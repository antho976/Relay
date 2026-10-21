//! Components in `design.apply`: a copy with `instance: true` makes an instance of a main, which
//! then follows every change to it (fields, nodes gained and lost, order) but the fields it keeps
//! as its own; a main inside another is brought to its instances first; an instance's structure
//! is the main's; detaching leaves plain nodes; and one undo takes back a change with what it did
//! to the instances.

mod common;

use common::{call, call_as, committed_repo, engine_with_project, ok, ok_as, refused};
use relay_bus::Actor;
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

fn agent(e: &Engine) -> Actor {
    let name = ok(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    Actor::agent(&name)
}

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
}

fn nodes(e: &Engine, design: i64) -> Vec<Value> {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].as_array().unwrap().clone()
}

fn node(e: &Engine, design: i64, id: &str) -> Value {
    nodes(e, design).into_iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("no node {id}"))
}

fn children(e: &Engine, design: i64, parent: &str) -> Vec<String> {
    nodes(e, design).iter().filter(|n| n["parent"] == parent).map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

/// Everything the design holds, for comparing before and after an undo.
fn content(e: &Engine, design: i64) -> Value {
    let got = ok(e, "design.get", json!({"design_id": design}));
    json!({"nodes": got["nodes"], "colors": got["colors"], "styles": got["styles"]})
}

/// A card main: a frame with a title and a price, and a design to put instances in.
fn card(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Budget"}))["design"]["id"].as_i64().unwrap();
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 96, "radius": 12, "fill": "#FFFFFF",
            "layout": "column", "gap": 4, "padding": [16], "shadow": {"y": 2, "blur": 8, "color": "#0000001F"}}},
        {"op": "add", "node": {"id": "card-title", "parent": "card", "kind": "text", "text": "Groceries", "font_weight": 600}},
        {"op": "add", "node": {"id": "card-price", "parent": "card", "kind": "text", "text": "$212.40", "color": "#5F6368"}},
    ]));
    id
}

/// An instance is a copy that follows its main: the main becomes a component, each node names
/// the one it follows, and the main's later changes reach it, but for what it set itself.
#[test]
fn an_instance_follows_its_main_but_for_its_overrides() {
    let (e, _root) = project();
    let id = card(&e);
    let out = apply(&e, id, json!([
        {"op": "copy", "id": "card", "instance": true, "dy": 120, "rename": {"card": "rent"}},
        {"op": "update", "id": "rent-title", "set": {"text": "Rent"}},
    ]));
    assert_eq!(out["ids"], json!(["rent", "rent-title"]));
    assert_eq!(node(&e, id, "card")["component"], true);
    let rent = node(&e, id, "rent");
    assert_eq!((rent["instance_of"].as_str(), rent.get("component")), (Some("card"), None));
    assert_eq!(rent["y"].as_f64(), Some(120.0));
    let title = node(&e, id, "rent-title");
    assert_eq!((title["instance_of"].as_str(), title["text"].as_str(), title["overrides"].clone()), (Some("card-title"), Some("Rent"), json!(["text"])));
    // The main's fill, weight and price change; the instance's own title text stays.
    apply(&e, id, json!([
        {"op": "update", "id": "card", "set": {"fill": "#F4F5F7", "x": 40}},
        {"op": "update", "id": "card-title", "set": {"text": "Food", "font_weight": 700}},
        {"op": "update", "id": "card-price", "set": {"text": "$0.00"}},
    ]));
    let (rent, title, price) = (node(&e, id, "rent"), node(&e, id, "rent-title"), node(&e, id, "rent-price"));
    assert_eq!((rent["fill"].as_str(), rent["x"].as_f64()), (Some("#F4F5F7"), Some(0.0)), "the root keeps its own place");
    assert_eq!((title["text"].as_str(), title["font_weight"].as_u64()), (Some("Rent"), Some(700)));
    assert_eq!(price["text"], "$0.00");
    // Its overrides put back: it takes the main's text again.
    apply(&e, id, json!([{"op": "update", "id": "rent-title", "set": {"overrides": []}}]));
    assert_eq!(node(&e, id, "rent-title")["text"], "Food");
}

/// What the main gains its instances gain, what it loses they lose, and its order is theirs.
#[test]
fn an_instance_takes_the_mains_structure() {
    let (e, _root) = project();
    let id = card(&e);
    apply(&e, id, json!([{"op": "copy", "id": "card", "as": "rent", "instance": true}]));
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "card-badge", "parent": "card", "kind": "ellipse", "w": 8, "h": 8}, "index": 0},
        {"op": "delete", "id": "card-price"},
    ]));
    assert_eq!(children(&e, id, "rent"), ["rent-badge", "rent-title"], "{:?}", children(&e, id, "rent"));
    assert_eq!(node(&e, id, "rent-badge")["instance_of"], "card-badge");
    apply(&e, id, json!([{"op": "move", "id": "card-badge", "index": 1}]));
    assert_eq!(children(&e, id, "rent"), ["rent-title", "rent-badge"]);
}

/// The instance's structure is the main's: a node of it is not added to, moved or deleted by
/// hand, and only the root detaches; detaching leaves plain nodes as they looked.
#[test]
fn an_instances_structure_is_refused_by_hand_and_detaching_frees_it() {
    let (e, _root) = project();
    let id = card(&e);
    apply(&e, id, json!([{"op": "copy", "id": "card", "as": "rent", "instance": true}]));
    let refusal = |ops: Value| refused(call(&e, "design.apply", json!({"design_id": id, "ops": ops})));
    for ops in [
        json!([{"op": "add", "node": {"parent": "rent", "kind": "rect"}}]),
        json!([{"op": "delete", "id": "rent-title"}]),
        json!([{"op": "move", "id": "rent-title", "parent": "page-1"}]),
        json!([{"op": "update", "id": "rent-title", "set": {"instance_of": null}}]),
        json!([{"op": "update", "id": "rent", "set": {"instance_of": "card-title"}}]),
        json!([{"op": "copy", "id": "rent-title", "instance": true}]),
    ] {
        let error = refusal(ops.clone());
        assert_eq!(error.code, "design.instance", "{ops}: {}", error.message);
    }
    // Hiding a node of it is an override, so it stays hidden.
    apply(&e, id, json!([{"op": "update", "id": "rent-price", "set": {"hidden": true}}]));
    apply(&e, id, json!([{"op": "update", "id": "card-price", "set": {"text": "$1"}}]));
    assert_eq!(node(&e, id, "rent-price")["hidden"], true);
    // Detached, it keeps its look and follows nothing.
    apply(&e, id, json!([{"op": "update", "id": "rent", "set": {"instance_of": null}}]));
    assert!(nodes(&e, id).iter().filter(|n| n["id"].as_str().unwrap().starts_with("rent")).all(|n| n.get("instance_of").is_none() && n.get("overrides").is_none()));
    apply(&e, id, json!([{"op": "update", "id": "card-price", "set": {"text": "$2"}}]));
    assert_eq!(node(&e, id, "rent-price")["text"], "$1");
    apply(&e, id, json!([{"op": "add", "node": {"parent": "rent", "kind": "rect"}}]));
}

/// A main holding an instance of another: changing the inner main reaches the outer main's
/// instances too, through the outer main. A main deleted detaches its instances. An instance
/// inside its own main is refused.
#[test]
fn nested_mains_settle_in_order_and_cycles_are_refused() {
    let (e, _root) = project();
    let id = card(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "button", "parent": "page-1", "kind": "rect", "x": 600, "w": 96, "h": 32, "text": "Pay", "fill": "#2D6BFF", "color": "#FFFFFF"}},
        {"op": "copy", "id": "button", "as": "card-pay", "parent": "card", "instance": true},
        {"op": "copy", "id": "card", "as": "rent", "instance": true, "dy": 200},
    ]));
    assert_eq!(node(&e, id, "card-pay")["instance_of"], "button");
    assert_eq!(node(&e, id, "rent-pay")["instance_of"], "card-pay");
    apply(&e, id, json!([{"op": "update", "id": "button", "set": {"fill": "#0F9D58"}}]));
    assert_eq!(node(&e, id, "card-pay")["fill"], "#0F9D58");
    assert_eq!(node(&e, id, "rent-pay")["fill"], "#0F9D58", "through the card");
    // Detaching the card's instance leaves its button an instance of the button.
    apply(&e, id, json!([{"op": "update", "id": "rent", "set": {"instance_of": null}}]));
    assert_eq!(node(&e, id, "rent-pay")["instance_of"], "button");
    apply(&e, id, json!([{"op": "update", "id": "button", "set": {"text": "Send"}}]));
    assert_eq!(node(&e, id, "rent-pay")["text"], "Send");
    // An instance of the card inside the card never settles.
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "copy", "id": "card", "as": "loop", "instance": true},
        {"op": "move", "id": "loop", "parent": "card"},
    ]})));
    assert_eq!(error.code, "design.component_cycle", "{}", error.message);
    // The button deleted: its instances are plain, as they were drawn.
    apply(&e, id, json!([{"op": "delete", "id": "button"}]));
    assert!(node(&e, id, "rent-pay").get("instance_of").is_none());
    assert_eq!(node(&e, id, "rent-pay")["text"], "Send");
}

/// One undo takes back the main's change and what it did to every instance, and the redo puts
/// both back; audit.undo does the same.
#[test]
fn one_undo_takes_back_a_change_and_its_instances() {
    let (e, _root) = project();
    let id = card(&e);
    apply(&e, id, json!([
        {"op": "copy", "id": "card", "as": "rent", "instance": true},
        {"op": "copy", "id": "card", "as": "fun", "instance": true, "dx": 400},
        {"op": "update", "id": "fun-title", "set": {"text": "Fun"}},
    ]));
    let before = content(&e, id);
    let out = ok(&e, "design.apply", json!({"design_id": id, "reply": "full", "ops": [
        {"op": "update", "id": "card-title", "set": {"color": "#111111"}},
        {"op": "add", "node": {"id": "card-note", "parent": "card", "kind": "text", "text": "Monthly"}},
    ]}));
    let after = content(&e, id);
    assert_eq!(node(&e, id, "fun-title")["color"], "#111111");
    let back = ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true, "expect": out["expect"], "reply": "full"}));
    assert_eq!(content(&e, id)["nodes"], before["nodes"]);
    ok(&e, "design.apply", json!({"design_id": id, "ops": back["inverse"], "lenient": true, "expect": back["expect"]}));
    assert_eq!(content(&e, id)["nodes"], after["nodes"]);
    let row = e.store.lock().query_row("SELECT MAX(id) FROM audit WHERE op = 'design.apply'", [], |r| r.get::<_, i64>(0)).unwrap();
    ok(&e, "audit.undo", json!({"audit_id": row}));
    assert_eq!(content(&e, id)["nodes"], before["nodes"]);
}

/// A design is marked as one that holds a main by the copy that makes one, and a duplicate of it
/// is marked too; a design without one is not, so its lists skip the component rules.
#[test]
fn a_design_is_marked_once_it_holds_a_main() {
    let (e, _root) = project();
    let id = card(&e);
    let marked = |design: i64| e.store.lock().query_row("SELECT components FROM designs WHERE id = ?1", [design], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(marked(id), 0);
    apply(&e, id, json!([{"op": "copy", "id": "card", "as": "rent", "instance": true}]));
    assert_eq!(marked(id), 1);
    let copy = ok(&e, "design.duplicate", json!({"design_id": id}))["id"].as_i64().unwrap();
    assert_eq!(marked(copy), 1);
    apply(&e, copy, json!([{"op": "update", "id": "card-title", "set": {"text": "Copied"}}]));
    assert_eq!(node(&e, copy, "rent-title")["text"], "Copied", "the duplicate's instances follow its own main");
    assert_eq!(node(&e, id, "rent-title")["text"], "Groceries");
}

/// An agent makes and changes components too, and a design without one is not slowed by them:
/// a list of one edit asks once whether the design has any.
#[test]
fn an_agent_uses_components_and_the_reply_names_the_instance() {
    let (e, _root) = project();
    let id = card(&e);
    let me = agent(&e);
    let out = ok_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "copy", "id": "card", "instance": true, "as": "rent"}]}));
    assert_eq!(out["copied"], json!({"rent": "card"}));
    let error = refused(call_as(&e, me, "design.apply", json!({"design_id": id, "ops": [{"op": "copy", "id": "page-1", "instance": true}]})));
    assert_eq!(error.code, "design.instance");
}

/// What a main's change costs with many instances: a row main of a title, an amount, a badge and
/// seven more labels (10 nodes), 40 instances of it, and one edit of the main's title that reaches
/// them all; beside the same edit of a plain node of the same design. Prints the medians of 15;
/// run with `cargo test -p relay-core --test design_components -- --ignored --nocapture`.
#[test]
#[ignore]
fn a_mains_change_costs_with_its_instances() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Rows"}))["design"]["id"].as_i64().unwrap();
    let mut ops = vec![json!({"op": "add", "node": {"id": "row", "parent": "page-1", "kind": "frame", "w": 360, "h": 56, "layout": "row", "gap": 8, "padding": [8]}})];
    ops.extend((0..9).map(|k| json!({"op": "add", "node": {"id": format!("row-t{k}"), "parent": "row", "kind": "text", "text": format!("Label {k}")}})));
    ops.push(json!({"op": "add", "node": {"id": "plain", "parent": "page-1", "kind": "rect", "x": 800}}));
    apply(&e, id, Value::Array(ops));
    let copies: Vec<Value> = (0..40).map(|k| json!({"op": "copy", "id": "row", "as": format!("r{k}"), "instance": true, "dy": 64 * (k + 1)})).collect();
    apply(&e, id, Value::Array(copies));
    let time = |target: &str, field: &str| {
        let mut runs: Vec<f64> = (0..15).map(|i| {
            let ops = json!([{"op": "update", "id": target, "set": {field: if i % 2 == 0 { "#111111" } else { "#222222" }}}]);
            let started = std::time::Instant::now();
            apply(&e, id, ops);
            started.elapsed().as_secs_f64() * 1000.0
        }).collect();
        runs.sort_by(f64::total_cmp);
        runs[runs.len() / 2]
    };
    let (plain, main) = (time("plain", "fill"), time("row-t0", "color"));
    assert!(node(&e, id, "r39-t0")["color"].as_str().is_some(), "the last instance followed");
    println!("441 nodes, 40 instances of 10: a plain node's edit {plain:.2} ms, the main's {main:.2} ms (median of 15)");
}
