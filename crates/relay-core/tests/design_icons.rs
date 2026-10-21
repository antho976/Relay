//! Relay's icon set (`design.icon.*`): an agent saves an outline icon once, as path data or an SVG,
//! and any design names it; an icon node that does carries the outline itself, so it draws the
//! same after the icon changes or goes. Built-in icons stay as they were.

mod common;

use common::{call, call_as, committed_repo, engine_with_project, ok, ok_as, refused};
use relay_bus::design::ICONS;
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

fn node(e: &Engine, design: i64, id: &str) -> Value {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap()
}

fn last_audit(e: &Engine, op: &str) -> i64 {
    e.store.lock().query_row("SELECT MAX(id) FROM audit WHERE op = ?1", [op], |r| r.get(0)).unwrap()
}

const PIGGY: &str = "M4 12a8 6 0 1 0 16 0a8 6 0 1 0-16 0M8 17v3M16 17v3";

/// An agent saves an icon; every caller lists it with Relay's own, found by name or keyword; its
/// path is stored in its plain form; a second save of the name is refused unless it replaces.
#[test]
fn an_icon_is_saved_listed_and_replaced() {
    let (e, _root) = project();
    let me = agent(&e);
    let out = ok_as(&e, me.clone(), "design.icon.save", json!({"name": "piggy-bank", "path": PIGGY, "keywords": ["Savings", "money", "money"]}));
    assert_eq!(out["icon"]["path"], "M4 12 a8 6 0 1 0 16 0 a8 6 0 1 0 -16 0 M8 17 v3 M16 17 v3");
    assert_eq!(out["icon"]["keywords"], json!(["savings", "money"]));
    assert_eq!(out["icon"]["updated_by"], me.to_string().trim_start_matches("agent:"));
    assert_eq!(out["replaced"], false);
    let listed = ok(&e, "design.icon.list", json!({}));
    assert_eq!(listed["built_in"].as_array().unwrap().len(), ICONS.len());
    assert_eq!(listed["saved"][0]["name"], "piggy-bank");
    assert_eq!(ok(&e, "design.icon.list", json!({"query": "SAVINGS"}))["saved"].as_array().unwrap().len(), 1, "by keyword");
    let found = ok(&e, "design.icon.list", json!({"query": "wallet"}));
    assert_eq!((found["built_in"].clone(), found["saved"].as_array().unwrap().len()), (json!(["wallet"]), 0));
    let error = refused(call_as(&e, me.clone(), "design.icon.save", json!({"name": "piggy-bank", "path": "M0 0h24"})));
    assert_eq!(error.code, "design.icon_exists");
    let again = ok_as(&e, me, "design.icon.save", json!({"name": "piggy-bank", "path": "M0 0h24", "replace": true}));
    assert_eq!((again["replaced"].as_bool(), again["icon"]["path"].as_str()), (Some(true), Some("M0 0 h24")));
    // The replacement is undone to the first outline.
    ok(&e, "audit.undo", json!({"audit_id": last_audit(&e, "design.icon.save")}));
    assert_eq!(ok(&e, "design.icon.list", json!({}))["saved"][0]["path"], "M4 12 a8 6 0 1 0 16 0 a8 6 0 1 0 -16 0 M8 17 v3 M16 17 v3");
}

/// What cannot be saved is refused by its field: a built-in name, a bad name, path data that
/// does not read, both or neither of path and svg, a bad keyword.
#[test]
fn what_cannot_be_saved_is_refused() {
    let (e, _root) = project();
    for (payload, field, why) in [
        (json!({"name": "wallet", "path": "M0 0h1"}), "name", "one of Relay's own icons"),
        (json!({"name": "a b", "path": "M0 0h1"}), "name", "is not a name"),
        (json!({"name": "a", "path": "L0 0"}), "path", "starts with a move"),
        (json!({"name": "a"}), "path", "give path"),
        (json!({"name": "a", "path": "M0 0h1", "svg": "<svg/>"}), "path", "not both"),
        (json!({"name": "a", "svg": "<svg viewBox=\"0 0 24 24\"><g transform=\"scale(2)\"/></svg>"}), "svg", "transform"),
        (json!({"name": "a", "path": "M0 0h1", "keywords": ["two words"]}), "keywords", "one word"),
    ] {
        let error = refused(call(&e, "design.icon.save", payload.clone()));
        assert!(error.message.contains(why), "{payload}: {}", error.message);
        assert_eq!(error.details.as_ref().unwrap()["field"], field, "{payload}");
    }
}

/// An SVG is fitted onto the grid; a filled one is saved with a warning.
#[test]
fn an_svg_is_saved_as_its_outline() {
    let (e, _root) = project();
    let out = ok(&e, "design.icon.save", json!({"name": "box", "svg": "<svg viewBox=\"0 0 48 48\" fill=\"none\" stroke=\"currentColor\"><rect x=\"8\" y=\"8\" width=\"32\" height=\"32\"/></svg>"}));
    assert_eq!(out["icon"]["path"], "M4 4 h16 v16 h-16 Z");
    assert!(out.get("warnings").is_none());
    let filled = ok(&e, "design.icon.save", json!({"name": "blob", "svg": "<svg viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"8\"/></svg>"}));
    assert!(filled["warnings"][0].as_str().unwrap().contains("filled"), "{filled}");
}

/// An icon node naming a saved icon carries its outline; naming a built-in one drops it; an
/// unknown name is refused with a hint; deleting the saved icon leaves the node drawn as it was.
#[test]
fn a_node_names_a_saved_icon_and_keeps_its_outline() {
    let (e, _root) = project();
    let me = agent(&e);
    ok_as(&e, me.clone(), "design.icon.save", json!({"name": "piggy-bank", "path": PIGGY}));
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Budget"}))["design"]["id"].as_i64().unwrap();
    let apply = |ops: Value| call_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": ops}));
    assert!(apply(json!([{"op": "add", "node": {"id": "save-icon", "parent": "page-1", "kind": "icon", "icon": "piggy-bank"}}])).into_result().is_ok());
    let saved = node(&e, id, "save-icon");
    assert_eq!((saved["icon"].as_str(), saved["icon_path"].as_str()), (Some("piggy-bank"), Some("M4 12 a8 6 0 1 0 16 0 a8 6 0 1 0 -16 0 M8 17 v3 M16 17 v3")));
    // To a built-in icon: the outline goes.
    assert!(apply(json!([{"op": "update", "id": "save-icon", "set": {"icon": "wallet"}}])).into_result().is_ok());
    assert!(node(&e, id, "save-icon").get("icon_path").is_none());
    // Back, then the saved icon deleted: the node keeps what it drew.
    assert!(apply(json!([{"op": "update", "id": "save-icon", "set": {"icon": "piggy-bank"}}])).into_result().is_ok());
    assert_eq!(ok(&e, "design.icon.delete", json!({"name": "piggy-bank"}))["deleted"], true);
    let deleted = last_audit(&e, "design.icon.delete");
    assert!(node(&e, id, "save-icon")["icon_path"].as_str().is_some());
    assert_eq!(ok(&e, "design.icon.delete", json!({"name": "piggy-bank"}))["deleted"], false, "already gone");
    let error = refused(apply(json!([{"op": "add", "node": {"parent": "page-1", "kind": "icon", "icon": "walet"}}])));
    assert_eq!(error.code, "design.node_invalid");
    assert!(error.message.contains("neither one of Relay's icons nor one saved"), "{}", error.message);
    assert!(error.hint.as_deref().unwrap().contains("did you mean \"wallet\"?"), "{:?}", error.hint);
    // A node's own outline, given straight, is written in its plain form; one that does not read is refused.
    assert!(apply(json!([{"op": "add", "node": {"id": "own", "parent": "page-1", "kind": "icon", "icon": "mine", "icon_path": "M0 0h24"}}])).into_result().is_ok());
    assert_eq!(node(&e, id, "own")["icon_path"], "M0 0 h24");
    let error = refused(apply(json!([{"op": "update", "id": "own", "set": {"icon_path": "Q"}}])));
    assert!(error.message.contains("icon_path"), "{}", error.message);
    // The delete is undone.
    ok(&e, "audit.undo", json!({"audit_id": deleted}));
    assert_eq!(ok(&e, "design.icon.list", json!({}))["saved"][0]["name"], "piggy-bank");
}
