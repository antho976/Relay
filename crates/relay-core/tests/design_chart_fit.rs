//! Charts that fill: a chart frame with `w_sizing` or `h_sizing` `fill` is drawn at the size its
//! laid-out parent gives it, and drawn again when that room changes (a resize further up, a
//! sibling added); one stretched by a pin is drawn again at its new size; one undo takes back the
//! resize and the chart's new size together; and one given a fixed width again keeps it.

mod common;

use common::{committed_repo, engine_with_project, ok};
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

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
}

fn nodes(e: &Engine, design: i64) -> Vec<Value> {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].as_array().unwrap().clone()
}

fn node(e: &Engine, design: i64, id: &str) -> Value {
    nodes(e, design).into_iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("no {id}"))
}

fn w(e: &Engine, design: i64, id: &str) -> f64 {
    node(e, design, id)["w"].as_f64().unwrap()
}

/// How far right the chart's parts reach, in the frame's coordinates.
fn reach(e: &Engine, design: i64, frame: &str) -> f64 {
    nodes(e, design).iter().filter(|n| n["parent"] == frame && n["generated"] == true)
        .map(|n| n["x"].as_f64().unwrap() + n["w"].as_f64().unwrap_or(0.0))
        .fold(0.0, f64::max)
}

/// A dashboard row of two cards that fill it, each a column holding a chart that fills it.
fn dashboard(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Coach"}))["design"]["id"].as_i64().unwrap();
    let chart = |labels: Value| json!({"kind": "bar", "stacked": true, "labels": labels,
        "series": [{"name": "Run", "values": [20, 0, 15]}, {"name": "Ride", "values": [0, 30, 10]}]});
    apply(e, id, json!([
        {"op": "add", "node": {"id": "desk", "parent": "page-1", "kind": "frame", "w": 800, "h": 400, "layout": "row", "gap": 16, "padding": [16]}},
        {"op": "add", "node": {"id": "card-a", "parent": "desk", "kind": "frame", "w_sizing": "fill", "h": 300, "layout": "column", "padding": [16]}},
        {"op": "add", "node": {"id": "card-b", "parent": "desk", "kind": "frame", "w_sizing": "fill", "h": 300, "layout": "column", "padding": [16]}},
        {"op": "add", "node": {"id": "week", "parent": "card-a", "kind": "frame", "w": 500, "w_sizing": "fill", "h": 200, "chart": chart(json!(["Mo", "Tu", "We"]))}},
        {"op": "add", "node": {"id": "month", "parent": "card-b", "kind": "frame", "w": 500, "h": 200, "chart": chart(json!(["Jan", "Feb", "Mar"]))}},
    ]));
    id
}

#[test]
fn a_chart_that_fills_is_drawn_at_its_laid_out_width() {
    let (e, _root) = project();
    let id = dashboard(&e);
    // (800 - 2 × 16 - 16) / 2 = 376 a card, less its padding.
    assert_eq!(w(&e, id, "week"), 344.0);
    assert!(reach(&e, id, "week") <= 344.0, "its parts within it: {}", reach(&e, id, "week"));
    assert_eq!(w(&e, id, "month"), 500.0, "a fixed chart keeps its width");

    // The desk shrinks: the chart is drawn again in the room left.
    let out = apply(&e, id, json!([{"op": "update", "id": "desk", "set": {"w": 640}}]));
    assert_eq!(w(&e, id, "week"), 264.0);
    assert!(reach(&e, id, "week") <= 264.0, "its parts within it: {}", reach(&e, id, "week"));
    assert!(out["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "week"), "the chart is in what the list changed");

    // One undo takes back the resize and the chart's size with it.
    ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true, "expect": out["expect"]}));
    assert_eq!((w(&e, id, "desk"), w(&e, id, "week")), (800.0, 344.0));
    assert!(reach(&e, id, "week") > 264.0, "drawn at its old width again");

    // A sibling takes room in the card's row: nothing of the chart itself was edited.
    apply(&e, id, json!([
        {"op": "update", "id": "card-a", "set": {"layout": "row", "gap": 8}},
        {"op": "add", "node": {"id": "side", "parent": "card-a", "kind": "rect", "w": 100, "h": 40}},
    ]));
    assert_eq!(w(&e, id, "week"), 344.0 - 108.0);

    // Given a fixed width again, it keeps it.
    apply(&e, id, json!([{"op": "update", "id": "week", "set": {"w_sizing": "fixed", "w": 200}}]));
    apply(&e, id, json!([{"op": "update", "id": "desk", "set": {"w": 1000}}]));
    assert_eq!(w(&e, id, "week"), 200.0);
}

/// A chart pinned to stretch with its free parent is drawn again when the parent's resize moves it.
#[test]
fn a_chart_a_pin_stretches_is_drawn_again() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Pins"}))["design"]["id"].as_i64().unwrap();
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "panel", "parent": "page-1", "kind": "frame", "w": 600, "h": 300}},
        {"op": "add", "node": {"id": "trend", "parent": "panel", "kind": "frame", "x": 20, "y": 20, "w": 560, "h": 200, "pin_x": "left_right",
            "chart": {"kind": "bar", "labels": ["A", "B", "C", "D"], "series": [{"values": [1, 2, 3, 4]}]}}},
    ]));
    assert!(reach(&e, id, "trend") > 400.0);
    apply(&e, id, json!([{"op": "update", "id": "panel", "set": {"w": 400}}]));
    assert_eq!(w(&e, id, "trend"), 360.0);
    assert!(reach(&e, id, "trend") <= 360.0, "its parts within it: {}", reach(&e, id, "trend"));
}

/// A chart of a fixed width keeps it whatever the room around it.
#[test]
fn a_fixed_chart_is_left_alone() {
    let (e, _root) = project();
    let id = dashboard(&e);
    let before = w(&e, id, "month");
    apply(&e, id, json!([{"op": "update", "id": "desk", "set": {"w": 700}}]));
    assert_eq!(w(&e, id, "month"), before);
}

/// A copy of a row holding a chart that fills is fitted, and keeps being fitted as its row is
/// resized: the copy is listed with the charts that fill.
#[test]
fn a_copied_fill_chart_is_fitted() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Rows"}))["design"]["id"].as_i64().unwrap();
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "row", "parent": "page-1", "kind": "frame", "w": 400, "h": 200, "layout": "row"}},
        {"op": "add", "node": {"id": "row-c", "parent": "row", "kind": "frame", "h": 200, "w_sizing": "fill", "chart": {"kind": "bar", "labels": ["A", "B"], "series": [{"values": [1, 2]}]}}},
        {"op": "copy", "id": "row", "as": "row-2", "dy": 300},
    ]));
    assert_eq!(w(&e, id, "row-2-c"), 400.0);
    apply(&e, id, json!([{"op": "update", "id": "row-2", "set": {"w": 200}}]));
    assert_eq!(w(&e, id, "row-2-c"), 200.0);
    assert!(reach(&e, id, "row-2-c") <= 200.5, "drawn to its new width");
    assert_eq!(w(&e, id, "row-c"), 400.0, "the original keeps its own");
}

/// A style edit that changes the layout around a chart that fills (a label's font size beside
/// it) fits the chart again, though it touches no node.
#[test]
fn a_style_that_moves_the_room_fits_the_chart_again() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Styled"}))["design"]["id"].as_i64().unwrap();
    apply(&e, id, json!([
        {"op": "style", "name": "label", "set": {"font_size": 12}},
        {"op": "add", "node": {"id": "row", "parent": "page-1", "kind": "frame", "w": 400, "h": 200, "layout": "row"}},
        {"op": "add", "node": {"id": "t", "parent": "row", "kind": "text", "text": "Weekly spend", "style": "label"}},
        {"op": "add", "node": {"id": "c", "parent": "row", "kind": "frame", "h": 200, "w_sizing": "fill", "chart": {"kind": "bar", "labels": ["A", "B"], "series": [{"values": [1, 2]}]}}},
    ]));
    let before = w(&e, id, "c");
    apply(&e, id, json!([{"op": "style", "name": "label", "set": {"font_size": 32}}]));
    let after = w(&e, id, "c");
    assert!(after < before, "{before} then {after}");
    apply(&e, id, json!([{"op": "update", "id": "t", "set": {"name": "Label"}}]));
    assert_eq!(w(&e, id, "c"), after, "it was fitted already");
}
