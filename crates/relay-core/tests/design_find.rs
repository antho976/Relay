//! `design.find` (feature-agent-api W22, §3.6; MODEL-SPEC §1 #28): a design's nodes found by
//! kind, name, words, colour or font as they are drawn, named colours and styles resolved, in
//! paint order; and `design.fonts` with no desktop app connected.

mod common;

use common::{call, committed_repo, engine_with_project, ok, refused};
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

fn find(e: &Engine, design: i64, filters: Value) -> Value {
    let mut payload = filters;
    payload["design_id"] = json!(design);
    ok(e, "design.find", payload)
}

fn ids(found: &Value) -> Vec<&str> {
    found["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect()
}

/// A card with a title, a styled button and a gradient badge; a note on a second page.
fn sample(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Home"}))["design"]["id"].as_i64().unwrap();
    ok(e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "color", "name": "accent", "value": "#4C9DFF"},
        {"op": "style", "name": "button", "set": {"fill": "$accent", "radius": 8}},
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "name": "Login card", "w": 320, "h": 200, "fill": "#FFFFFF"}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Sign in", "font": "Geist", "color": "#111111",
            "runs": [{"start": 0, "end": 4, "font": "Sora", "color": "#FF000080"}]}},
        {"op": "add", "node": {"id": "save", "parent": "card", "kind": "rect", "name": "Save button", "w": 120, "h": 40, "style": "button"}},
        {"op": "add", "node": {"id": "badge", "parent": "card", "kind": "ellipse", "w": 20, "h": 20,
            "gradient": {"stops": [{"at": 0, "color": "#FFFFFF"}, {"at": 1, "color": "#00FF00"}]}, "shadow": {"blur": 4, "color": "#00000040"}}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Later"}},
        {"op": "add", "node": {"id": "note", "parent": "page-2", "kind": "sticky", "text": "sign up later"}},
    ]}));
    id
}

/// W22: by kind, by words in a name or a text (any case), by font (its own or a run's), and by a
/// colour as drawn, a named colour or a style's resolved; `#fff` is `#FFFFFF`; an 8-digit colour
/// compares alpha too; any_color reaches shadows, gradient stops and runs; inside and page narrow it.
#[test]
fn find_matches_kind_name_text_colour_and_font() {
    let (e, _root) = project();
    let id = sample(&e);
    assert_eq!(ids(&find(&e, id, json!({}))), ["card", "title", "save", "badge", "note"], "every node but the pages, in paint order");
    assert_eq!(ids(&find(&e, id, json!({"kind": ["rect", "ellipse"]}))), ["save", "badge"]);
    assert_eq!(ids(&find(&e, id, json!({"name": "BUTTON"}))), ["save"]);
    assert_eq!(ids(&find(&e, id, json!({"text": "sign"}))), ["title", "note"]);
    assert_eq!(ids(&find(&e, id, json!({"text": "sign", "page": "Later"}))), ["note"]);
    assert_eq!(ids(&find(&e, id, json!({"font": "sora"}))), ["title"], "a run's font");
    assert_eq!(ids(&find(&e, id, json!({"fill": "#fff"}))), ["card"]);
    assert_eq!(ids(&find(&e, id, json!({"fill": "#4c9dff"}))), ["save"], "the style's $accent, as drawn");
    assert_eq!(ids(&find(&e, id, json!({"any_color": "#00ff00"}))), ["badge"], "a gradient stop");
    assert_eq!(ids(&find(&e, id, json!({"any_color": "#000000"}))), ["badge"], "a shadow, alpha aside");
    assert!(ids(&find(&e, id, json!({"any_color": "#000000FF"}))).is_empty(), "eight digits compare alpha");
    assert_eq!(ids(&find(&e, id, json!({"any_color": "#FF000080"}))), ["title"], "a run's colour");
    assert_eq!(ids(&find(&e, id, json!({"inside": "card", "kind": ["text"]}))), ["title"]);
    assert_eq!(ids(&find(&e, id, json!({"inside": "card"}))), ["title", "save", "badge"], "not the card itself");
}

/// with_nodes answers the matches as stored, at most 200; cut says when ids go on.
#[test]
fn find_with_nodes_is_cut_at_200() {
    let (e, _root) = project();
    let id = sample(&e);
    let ops: Vec<Value> = (0..230).map(|i| json!({"op": "add", "node": {"id": format!("r{i}"), "parent": "page-1", "kind": "rect", "w": 4, "h": 4}})).collect();
    ok(&e, "design.apply", json!({"design_id": id, "ops": ops}));
    let found = find(&e, id, json!({"kind": ["rect"], "with_nodes": true}));
    assert_eq!((found["ids"].as_array().unwrap().len(), found["nodes"].as_array().unwrap().len(), found["cut"].as_bool()), (231, 200, Some(true)));
    assert_eq!(found["nodes"][0]["style"], "button", "as stored: the style's name, not its fields");
    let small = find(&e, id, json!({"name": "save", "with_nodes": true}));
    assert!(small.get("cut").is_none() && small["nodes"].as_array().unwrap().len() == 1);
}

/// Words match whole by default, so "Edit" passes over "CREDIT"; prefix, contains and exact ask
/// for the others, for a name as for a text.
#[test]
fn find_matches_whole_words_unless_asked_otherwise() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Wallet"}))["design"]["id"].as_i64().unwrap();
    ok(&e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "edit", "parent": "page-1", "kind": "text", "text": "Edit profile"}},
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "text", "text": "CREDIT CARD", "name": "Credit row"}},
        {"op": "add", "node": {"id": "edited", "parent": "page-1", "kind": "text", "text": "Edited 2 min ago"}},
        {"op": "add", "node": {"id": "plain", "parent": "page-1", "kind": "text", "text": " edit "}},
    ]}));
    let by = |how: Option<&str>, field: &str, words: &str| {
        let mut filters = json!({field: words});
        if let Some(how) = how {
            filters["match"] = json!(how);
        }
        ids(&find(&e, id, filters)).into_iter().map(String::from).collect::<Vec<_>>()
    };
    assert_eq!(by(None, "text", "Edit"), ["edit", "plain"], "whole words: not CREDIT, not Edited");
    assert_eq!(by(Some("word"), "text", "edit profile"), ["edit"], "a phrase");
    assert_eq!(by(Some("prefix"), "text", "edit"), ["edit", "edited", "plain"]);
    assert_eq!(by(Some("prefix"), "text", "cred"), ["card"]);
    assert_eq!(by(Some("contains"), "text", "edit"), ["edit", "card", "edited", "plain"]);
    assert_eq!(by(Some("exact"), "text", "EDIT"), ["plain"], "the whole text, its outer spaces aside");
    assert_eq!(by(None, "name", "credit"), ["card"]);
    assert!(by(None, "name", "cred").is_empty());
    let error = refused(call(&e, "design.find", json!({"design_id": id, "text": "x", "match": "fuzzy"})));
    assert!(error.message.contains("fuzzy"), "{}", error.message);
}

#[test]
fn find_refuses_a_colour_it_cannot_read() {
    let (e, _root) = project();
    let id = sample(&e);
    for (field, value) in [("fill", "blue"), ("any_color", "$accent"), ("stroke", "#12")] {
        let error = refused(call(&e, "design.find", json!({"design_id": id, field: value})));
        assert_eq!(error.code, "design.find", "{field}");
        assert!(error.message.starts_with(&format!("{field}: ")), "{}", error.message);
    }
    let error = refused(call(&e, "design.find", json!({"design_id": id, "inside": "nowhere"})));
    assert_eq!(error.code, "design.node_not_found");
}

/// W68: with no desktop app connected, the three families every Relay draws, incomplete; and an
/// apply naming another font is not warned of, since nobody said what the desktop has.
#[test]
fn fonts_without_an_app_are_the_three_always_there() {
    let (e, _root) = project();
    assert_eq!(ok(&e, "design.fonts", json!({})), json!({"families": ["Geist", "Geist Mono", "Sora"], "complete": false}));
    assert_eq!(ok(&e, "design.fonts", json!({"name": "mono"})), json!({"families": ["Geist Mono"], "complete": false}));
}
