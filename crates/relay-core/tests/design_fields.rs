//! The node fields and edits of the design canvas's second round, through the engine
//! (MODEL-SPEC §2.4-§2.11): what a strict list stores and refuses, and the edits whose engine
//! part a later stream brings, refused until then by name.

mod common;

use common::{call, committed_repo, engine_with_project, err, ok};
use relay_bus::BusError;
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

fn find(nodes: &Value, id: &str) -> Value {
    nodes.as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or_else(|| panic!("no {id} in {nodes}"))
}

/// The refusal of `ops`, with the design left as it was.
fn refused(e: &Engine, design: i64, ops: Value) -> BusError {
    let before = ok(e, "design.get", json!({"design_id": design}));
    let error = err(&call(e, "design.apply", json!({"design_id": design, "ops": ops}))).clone();
    assert_eq!(ok(e, "design.get", json!({"design_id": design})), before, "nothing of a refused list stays");
    error
}

/// A frame with a card in it, the first edits of most lists here.
fn card(e: &Engine, design: i64) {
    apply(e, design, json!([
        {"op": "add", "node": {"id": "screen", "parent": "page-1", "kind": "frame", "w": 390, "h": 844}},
        {"op": "add", "node": {"id": "card", "parent": "screen", "kind": "frame", "x": 16, "y": 16, "w": 358, "h": 120}},
    ]));
}

/// Layout fields are stored as written and read back; their value rules refuse by field.
#[test]
fn layout_fields_are_stored_and_checked() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "badge", "parent": "card", "kind": "ellipse", "x": 326, "y": 8, "w": 24, "h": 24,
            "pin_x": "right", "absolute": true}},
        {"op": "add", "node": {"id": "chips", "parent": "card", "kind": "frame", "w": 300, "h": 60, "layout": "wrap", "gap": 8,
            "row_gap": 4, "min_w": 200, "max_w": 400, "pin_x": "left_right"}},
        {"op": "add", "node": {"id": "rail", "parent": "screen", "kind": "frame", "y": 200, "w": 390, "h": 120, "layout": "row",
            "scroll": "x"}},
    ]));
    let nodes = ok(&e, "design.get", json!({"design_id": id}))["nodes"].clone();
    assert_eq!((find(&nodes, "badge")["pin_x"].clone(), find(&nodes, "badge")["absolute"].clone()), (json!("right"), json!(true)));
    let chips = find(&nodes, "chips");
    assert_eq!((chips["layout"].clone(), chips["row_gap"].clone(), chips["max_w"].clone()), (json!("wrap"), json!(4.0), json!(400.0)));
    assert_eq!(find(&nodes, "rail")["scroll"], "x");
    assert_eq!(out["ids"], json!(["badge", "chips", "rail"]));
    for (node, code, message) in [
        (json!({"id": "x", "parent": "card", "kind": "frame", "w": 500, "max_w": 400}), "design.node_invalid",
            "ops[0] add \"x\": w: 500 is outside min_w to max_w (0 to 400); a fixed width stays within them"),
        (json!({"id": "x", "parent": "card", "kind": "line", "points": [[0, 0], [9, 0]], "pin_x": "scale"}), "design.node_invalid",
            "ops[0] add \"x\": pin_x: a line's points do not stretch; use left, right or center"),
        (json!({"id": "x", "parent": "card", "kind": "rect", "w": 9, "h": 9, "scroll": "y"}), "design.node_invalid",
            "ops[0] add \"x\": scroll: only a frame scrolls"),
        (json!({"id": "x", "parent": "card", "kind": "group", "min_h": 4}), "design.node_invalid",
            "ops[0] add \"x\": min_h: group nodes do not use it (it is for frame, rect, ellipse, text, sticky, image, polygon, icon)"),
    ] {
        let error = refused(&e, id, json!([{"op": "add", "node": node}]));
        assert_eq!((error.code.as_str(), error.message.as_str()), (code, message));
    }
}

/// `keep_position` is read, and refused by name until the engine rewrites x/y from page points.
#[test]
fn keep_position_is_refused_until_the_engine_keeps_it() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    let error = refused(&e, id, json!([{"op": "move", "id": "card", "parent": "page-1", "keep_position": true}]));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.ops", "ops[0] move \"card\": keep_position is not supported yet"));
    assert_eq!(error.details, Some(json!({"op": 0, "index": 0, "id": "card", "field": "keep_position"})));
    // Without it (or false) a move runs as before, and its undo carries none.
    let out = apply(&e, id, json!([{"op": "move", "id": "card", "parent": "page-1", "keep_position": false}]));
    assert_eq!(out["inverse"], json!([{"op": "move", "id": "card", "parent": "screen", "index": 0}]));
}
