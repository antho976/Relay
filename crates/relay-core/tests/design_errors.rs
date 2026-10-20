//! What `design.apply` says when it refuses (W16, W19, TM-2, I-2): an edit that does not read
//! is named by its index, label and key, with advice for what an agent meant, and every refusal
//! carries `details {index, id, field}` so an agent or the canvas can point at the edit without
//! reading the message.

mod common;

use common::{call, call_as, committed_repo, engine_with_project, err, ok, ok_as};
use relay_bus::design::{MAX_LENIENT_OPS, MAX_OPS};
use relay_bus::{Actor, BusError};
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

/// The refusal of `ops`, with the design left as it was.
fn refused(e: &Engine, design: i64, ops: Value) -> BusError {
    let before = ok(e, "design.get", json!({"design_id": design}));
    let response = call(e, "design.apply", json!({"design_id": design, "ops": ops}));
    let error = err(&response).clone();
    assert_eq!(ok(e, "design.get", json!({"design_id": design})), before, "nothing of a refused list stays");
    error
}

/// The first edits of most lists here: a frame and a text in it, at ops[0] and ops[1].
fn opening() -> Vec<Value> {
    vec![
        json!({"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 200}}),
        json!({"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hi"}}),
    ]
}

fn with(edits: &[Value]) -> Value {
    Value::Array(opening().into_iter().chain(edits.iter().cloned()).collect())
}

/// TM-2: what agents used to meet as serde's bare message, each named by its edit and field.
#[test]
fn each_refusal_names_its_edit_and_field() {
    let (e, _root) = project();
    let id = create(&e);
    let cases = [
        (with(&[json!({"op": "delete", "id": "title"}), json!({"op": "add", "node": {"id": "x", "parent": "card", "kind": "frame", "layout": "grid"}})]),
            "ops[3] add \"x\": layout: unknown variant `grid`, expected one of `none`, `row`, `column`, `wrap`", 3, Some("x"), Some("layout")),
        (with(&[json!({"op": "add", "node": {"id": "star", "parent": "card", "kind": "polygon", "star": 0.4, "sides": 300}})]),
            "ops[2] add \"star\": sides: 300 is outside 3 to 24", 2, Some("star"), Some("sides")),
        (with(&[json!({"op": "add", "node": {"id": "x", "parent": "card", "kind": "rect", "fil": "#fff"}})]),
            "ops[2] add \"x\": fil: not a node field; did you mean \"fill\"?", 2, Some("x"), Some("fil")),
        (with(&[json!({"op": "update", "id": "card", "set": {"padding": 16}})]),
            "ops[2] update \"card\": padding: invalid type: integer `16`, expected a sequence; give a list: [all], [vertical, horizontal] or [top, right, bottom, left]",
            2, Some("card"), Some("padding")),
        (with(&[json!({"op": "update", "id": "card", "set": {"fill": "red"}})]),
            "ops[2] update \"card\": fill: \"red\" is not #RGB, #RRGGBB or #RRGGBBAA", 2, Some("card"), Some("fill")),
        (with(&[json!({"op": "update", "id": "title", "set": {"x": "12"}})]),
            "ops[2] update \"title\": x: invalid type: string \"12\", expected f64", 2, Some("title"), Some("x")),
        (with(&[json!({"op": "update", "id": "card", "set": {"background": "#fff"}})]),
            "ops[2] update \"card\": background: not a node field; use fill", 2, Some("card"), Some("background")),
        (with(&[json!({"op": "add", "node": {"id": "col", "parent": "card", "kind": "frame", "layout": "vertical"}})]),
            "ops[2] add \"col\": layout: unknown variant `vertical`, expected one of `none`, `row`, `column`, `wrap`; use \"column\"", 2, Some("col"), Some("layout")),
        (with(&[json!({"op": "add", "node": {"parent": "card", "kind": "rect", "shadow": {"blur": 300, "color": "#000"}}})]),
            "ops[2] add rect: shadow.blur: 300 is outside 0 to 200", 2, None, Some("shadow.blur")),
    ];
    for (ops, message, index, node, field) in cases {
        let error = refused(&e, id, ops);
        assert_eq!((error.code.as_str(), error.message.as_str()), ("design.node_invalid", message));
        assert_eq!(error.details, Some(json!({"op": index, "index": index, "id": node, "field": field})), "{message}");
    }
}

/// I-2: refusals of the engine's own rules point at the edit too.
#[test]
fn engine_refusals_carry_their_details() {
    let (e, _root) = project();
    let id = create(&e);
    let cases = [
        (with(&[json!({"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "rect"}})]), "design.node_exists", 2, Some("card"), Some("id")),
        (with(&[json!({"op": "add", "node": {"id": "x", "parent": "nope", "kind": "rect"}})]), "design.parent", 2, Some("x"), Some("parent")),
        (with(&[json!({"op": "move", "id": "card", "parent": "title"})]), "design.parent", 2, Some("card"), Some("parent")),
        (with(&[json!({"op": "move", "id": "card", "parent": "card"})]), "design.cycle", 2, Some("card"), Some("parent")),
        (with(&[json!({"op": "delete", "id": "ghost"})]), "design.node_not_found", 2, Some("ghost"), None),
        (json!([{"op": "delete", "id": "page-1"}]), "design.last_page", 0, Some("page-1"), None),
    ];
    for (ops, code, index, node, field) in cases {
        let error = refused(&e, id, ops);
        assert_eq!(error.code, code, "{}", error.message);
        assert_eq!(error.details, Some(json!({"op": index, "index": index, "id": node, "field": field})), "{}", error.message);
    }
}

/// W16: an edit that does not read at all is refused before anything runs, as `bus.schema`
/// naming the edit, with advice and `details` (I-2).
#[test]
fn an_edit_that_does_not_read_is_named_before_the_list_runs() {
    let (e, _root) = project();
    let id = create(&e);
    let prefix = "payload for design.apply does not match its schema: ";
    let cases = [
        (with(&[json!({"op": "copy", "id": "card"})]),
            "ops[2]: op \"copy\" is not an edit; there is no copy edit: add the node again under a new id (design.get has its fields), then its children",
            json!({"op": 2, "index": 2, "id": null, "field": "op"})),
        (with(&[json!({"op": "add", "node": {"id": "cta", "parent": "card", "kind": "button"}})]),
            "ops[2] add \"cta\": node.kind: \"button\" is not a kind; use frame (layout: row or column lays out its children)",
            json!({"op": 2, "index": 2, "id": "cta", "field": "node.kind"})),
        (json!([{"op": "update", "id": "card", "fill": "#fff"}]),
            "ops[0] update \"card\": fill: a node field goes in set: {\"set\": {\"fill\": …}}",
            json!({"op": 0, "index": 0, "id": "card", "field": "fill"})),
        (json!([{"op": "move", "id": "card", "index": -1}]),
            "ops[0] move \"card\": index: invalid value: integer `-1`, expected u32; 0 is the bottom of the parent's children",
            json!({"op": 0, "index": 0, "id": "card", "field": "index"})),
    ];
    for (ops, message, details) in cases {
        let error = refused(&e, id, ops);
        assert_eq!(error.code, "bus.schema");
        assert_eq!(error.message, format!("{prefix}{message}"));
        assert_eq!(error.details, Some(details), "{message}");
    }
    // A list past what anyone may send stops while it is read.
    let ghosts: Vec<Value> = (0..=MAX_LENIENT_OPS).map(|i| json!({"op": "delete", "id": format!("g{i}")})).collect();
    let error = refused(&e, id, Value::Array(ghosts));
    assert_eq!(error.message, format!("{prefix}ops: more than {MAX_LENIENT_OPS} edits; send at most {MAX_OPS} in one list"));
    assert_eq!(error.details, Some(json!({"field": "ops"})));
    // Another op's refusal carries no edit details.
    let error = err(&call(&e, "design.get", json!({"design_id": "one"}))).clone();
    assert_eq!((error.code.as_str(), error.details), ("bus.schema", None));
}

/// What a lenient list of an agent's let through is told with the same advice.
#[test]
fn an_agents_warnings_carry_the_advice() {
    let (e, _root) = project();
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let agent = Actor::agent(&name);
    let id = ok_as(&e, agent.clone(), "design.create", json!({"title": "Card"}))["design"]["id"].as_i64().unwrap();
    let out = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "lenient": true, "ops": [
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "background": "#fff", "layout": "vertical"}},
    ]}));
    let warnings: Vec<&str> = out["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, ["ops[0] add \"card\": background: not a node field; use fill; kept as it was"]);
    // The strict list is refused at the same key.
    let strict = call_as(&e, agent, "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "other", "parent": "page-1", "kind": "frame", "background": "#fff"}},
    ]}));
    assert_eq!(err(&strict).message, "ops[0] add \"other\": background: not a node field; use fill");
}
