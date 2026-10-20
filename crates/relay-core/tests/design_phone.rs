//! The phone's design.get fixture (`apps/relay-android/core/src/test/resources/design/get-reply.json`,
//! which the phone's tests read) is an answer the engine really gives: made through design.apply,
//! its colours and styles first and its pictures stored, it reads back node for node. And the
//! phone's reads: the pages alone, then a page.

mod common;

use common::{committed_repo, engine_with_project, ok};
use relay_bus::design::{Node, Styles};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::sync::Arc;

const REPLY: &str = include_str!("../../../apps/relay-android/core/src/test/resources/design/get-reply.json");

fn project() -> (Arc<Engine>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    (engine, root)
}

/// The fixture made again through design.apply: its colours and styles, its pictures (any bytes:
/// an image node only needs its file there), the first page's fields, then every other node in
/// order. Answers the design's id.
fn made(e: &Engine, fixture: &Value) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Phone fixture"}))["design"]["id"].as_i64().unwrap();
    let dir = e.store.design_assets_dir();
    std::fs::create_dir_all(&dir).unwrap();
    for n in fixture["nodes"].as_array().unwrap() {
        if let Some(src) = n["src"].as_str() {
            std::fs::write(dir.join(src), b"picture").unwrap();
        }
    }
    let mut ops: Vec<Value> = fixture["colors"].as_object().unwrap().iter().map(|(name, value)| json!({"op": "color", "name": name, "value": value})).collect();
    for (name, style) in fixture["styles"].as_object().unwrap() {
        ops.push(json!({"op": "style", "name": name, "set": style}));
    }
    let nodes = fixture["nodes"].as_array().unwrap();
    let mut page = nodes[0].as_object().unwrap().clone();
    for key in ["id", "kind", "x", "y", "w", "h"] {
        page.remove(key);
    }
    ops.push(json!({"op": "update", "id": "page-1", "set": page}));
    ops.extend(nodes[1..].iter().map(|n| json!({"op": "add", "node": n})));
    ok(e, "design.apply", json!({"design_id": id, "ops": ops}));
    id
}

fn read_nodes(v: &Value) -> Vec<Node> {
    v["nodes"].as_array().unwrap().iter().map(|n| Node::read(n.clone()).unwrap()).collect()
}

#[test]
fn the_phone_fixture_round_trips_through_the_engine() {
    let (e, _root) = project();
    let fixture: Value = serde_json::from_str(REPLY).unwrap();
    let id = made(&e, &fixture);
    let got = ok(&e, "design.get", json!({"design_id": id}));
    assert_eq!(read_nodes(&got), read_nodes(&fixture), "node for node, as the engine stores what a strict list writes");
    let styles = |v: &Value| -> Styles { serde_json::from_value(json!({"colors": v["colors"], "styles": v["styles"]})).unwrap() };
    assert_eq!(styles(&got), styles(&fixture), "the same colours and styles, however their numbers are written");
}

/// The phone's two reads: the pages alone (its chips), then one page with what is on it.
#[test]
fn the_phone_reads_the_pages_then_a_page() {
    let (e, _root) = project();
    let fixture: Value = serde_json::from_str(REPLY).unwrap();
    let id = made(&e, &fixture);
    ok(&e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Tablet"}},
        {"op": "add", "node": {"id": "t", "parent": "page-2", "kind": "text", "text": "Hi"}},
    ]}));
    let pages = ok(&e, "design.get", json!({"design_id": id, "pages_only": true}));
    let names: Vec<&str> = pages["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Mobile", "Tablet"]);
    let second = ok(&e, "design.get", json!({"design_id": id, "page": "page-2"}));
    let ids: Vec<&str> = second["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["page-2", "t"]);
}
