//! Charts in `design.apply`: a frame's `chart` is drawn as its `generated` children once the list
//! has run, and drawn again when the chart, the frame's size or its padding changes; a part is not
//! edited by hand; `null` takes the parts out; one undo takes a change back with its parts; and a
//! chart in an instance follows its main's.

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

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
}

fn nodes(e: &Engine, design: i64) -> Vec<Value> {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].as_array().unwrap().clone()
}

fn node(e: &Engine, design: i64, id: &str) -> Option<Value> {
    nodes(e, design).into_iter().find(|n| n["id"] == id)
}

/// The ids of a frame's children, in paint order.
fn children(e: &Engine, design: i64, parent: &str) -> Vec<String> {
    nodes(e, design).iter().filter(|n| n["parent"] == parent).map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn spend(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Budget"}))["design"]["id"].as_i64().unwrap();
    apply(e, id, json!([
        {"op": "add", "node": {"id": "spend", "parent": "page-1", "kind": "frame", "w": 400, "h": 240, "padding": [16], "fill": "#FFFFFF",
            "chart": {"kind": "bar", "labels": ["Jan", "Feb", "Mar"], "series": [{"name": "Spent", "values": [420, 380, 515]}], "prefix": "$"}}},
        {"op": "add", "node": {"id": "spend-title", "parent": "spend", "kind": "text", "text": "Spending"}},
    ]));
    id
}

/// The chart is drawn as generated children under the frame's own, each a node like any other.
#[test]
fn a_chart_is_drawn_as_its_frames_children() {
    let (e, _root) = project();
    let id = spend(&e);
    let kids = children(&e, id, "spend");
    assert_eq!(kids.last().map(String::as_str), Some("spend-title"), "the frame's own child stays on top: {kids:?}");
    for part in ["spend.bar-0-0", "spend.bar-0-2", "spend.label-1", "spend.tick-0", "spend.grid-3"] {
        let n = node(&e, id, part).unwrap_or_else(|| panic!("no {part} in {kids:?}"));
        assert_eq!(n["generated"], true, "{part}");
    }
    assert_eq!(node(&e, id, "spend.label-1").unwrap()["text"], "Feb");
    assert_eq!(node(&e, id, "spend.tick-3").unwrap()["text"], "$600");
    // A list that does not touch the chart draws nothing.
    let rev = ok(&e, "design.get", json!({"design_id": id, "pages_only": true}))["design"]["rev"].as_i64().unwrap();
    let out = apply(&e, id, json!([{"op": "update", "id": "spend-title", "set": {"text": "Spent"}}]));
    assert_eq!(out["rev"].as_i64(), Some(rev + 1));
}

/// New data redraws what changed: a fourth month adds its bar and label, a dropped one deletes
/// them, a taller one moves the top; a resize redraws to the new size; null takes the parts out.
#[test]
fn a_change_of_its_data_or_size_redraws_it() {
    let (e, _root) = project();
    let id = spend(&e);
    let before = node(&e, id, "spend.bar-0-2").unwrap();
    apply(&e, id, json!([{"op": "update", "id": "spend", "set": {"chart": {"kind": "bar", "labels": ["Jan", "Feb", "Mar", "Apr"],
        "series": [{"name": "Spent", "values": [420, 380, 515, 610]}], "prefix": "$"}}}]));
    assert!(node(&e, id, "spend.bar-0-3").is_some() && node(&e, id, "spend.label-3").unwrap()["text"] == "Apr");
    assert_ne!(node(&e, id, "spend.bar-0-2").unwrap()["x"], before["x"], "narrower rooms");
    assert_eq!(node(&e, id, "spend.tick-4").unwrap()["text"], "$800");
    apply(&e, id, json!([{"op": "update", "id": "spend", "set": {"chart": {"kind": "bar", "labels": ["Jan"], "series": [{"values": [5]}]}}}]));
    assert!(node(&e, id, "spend.bar-0-1").is_none() && node(&e, id, "spend.label-1").is_none(), "what is gone goes");
    let narrow = node(&e, id, "spend.bar-0-0").unwrap();
    apply(&e, id, json!([{"op": "update", "id": "spend", "set": {"w": 800}}]));
    assert!(node(&e, id, "spend.bar-0-0").unwrap()["w"].as_f64() > narrow["w"].as_f64(), "drawn to its new width");
    apply(&e, id, json!([{"op": "update", "id": "spend", "set": {"chart": null}}]));
    assert_eq!(children(&e, id, "spend"), ["spend-title"]);
}

/// A part is the chart's: an update, a move or a delete of it by hand is refused; a bad chart is
/// refused by its field; a chart's frame lays nothing out.
#[test]
fn its_parts_are_not_edited_by_hand() {
    let (e, _root) = project();
    let id = spend(&e);
    for op in [
        json!({"op": "update", "id": "spend.bar-0-0", "set": {"fill": "#FF0000"}}),
        json!({"op": "move", "id": "spend.bar-0-0", "parent": "page-1"}),
        json!({"op": "delete", "id": "spend.label-0"}),
    ] {
        let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [op.clone()]})));
        assert_eq!(error.code, "design.chart", "{op}: {}", error.message);
    }
    for (chart, why) in [
        (json!({"kind": "bar", "labels": ["A", "B"], "series": [{"values": [1]}]}), "chart.series[0].values: 1 of them, one per label (2)"),
        (json!({"kind": "donut", "labels": ["A"], "series": [{"values": [-1]}]}), "no value below 0"),
        (json!({"kind": "line", "horizontal": true, "labels": ["A"], "series": [{"values": [1]}]}), "only a bar chart"),
    ] {
        let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "spend", "set": {"chart": chart}}]})));
        assert!(error.message.contains(why), "{}", error.message);
    }
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "spend", "set": {"layout": "row"}}]})));
    assert!(error.message.contains("give it no layout"), "{}", error.message);
}

/// One undo takes back a chart's change and every part it redrew; a copy of a chart frame is a
/// chart with its own parts; a chart in an instance follows its main's.
#[test]
fn undo_copies_and_instances() {
    let (e, _root) = project();
    let id = spend(&e);
    let before = nodes(&e, id);
    let out = ok(&e, "design.apply", json!({"design_id": id, "reply": "full", "ops": [{"op": "update", "id": "spend", "set": {"chart": {"kind": "line",
        "labels": ["A", "B"], "series": [{"values": [1, 2]}]}}}]}));
    assert!(node(&e, id, "spend.line-0").is_some());
    ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true, "expect": out["expect"]}));
    assert_eq!(nodes(&e, id), before);
    apply(&e, id, json!([{"op": "copy", "id": "spend", "as": "spend2", "dx": 500}]));
    assert_eq!(node(&e, id, "spend2.bar-0-1").unwrap()["generated"], true);
    apply(&e, id, json!([{"op": "update", "id": "spend2", "set": {"chart": {"kind": "pie", "labels": ["A", "B"], "series": [{"values": [1, 3]}]}}}]));
    assert!(node(&e, id, "spend2.slice-1").is_some() && node(&e, id, "spend.bar-0-1").is_some(), "each draws its own");
    apply(&e, id, json!([{"op": "copy", "id": "spend", "as": "mirror", "instance": true, "dy": 300}]));
    apply(&e, id, json!([{"op": "update", "id": "spend", "set": {"chart": {"kind": "bar", "labels": ["Q1", "Q2"], "series": [{"values": [9, 4]}]}}}]));
    assert_eq!(node(&e, id, "mirror.label-0").unwrap()["text"], "Q1", "the instance's parts follow the main's");
    assert!(node(&e, id, "mirror.label-2").is_none());
}

/// A page of one chart of each kind, written as design.get answers it to the file `CHART_DUMP`
/// names, to look at; run with `CHART_DUMP=/path cargo test -p relay-core --test all -- design_charts:: --ignored`.
#[test]
#[ignore]
fn dump_a_page_of_charts() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Charts"}))["design"]["id"].as_i64().unwrap();
    let months = json!(["Jan", "Feb", "Mar", "Apr", "May", "Jun"]);
    let charts = [
        json!({"kind": "bar", "labels": months, "series": [{"name": "Spent", "values": [420, 380, 515, 610, 470, 530]}], "prefix": "$", "values": true}),
        json!({"kind": "bar", "stacked": true, "labels": months, "series": [{"name": "Food", "values": [200, 180, 240, 260, 210, 230]}, {"name": "Rent", "values": [900, 900, 900, 950, 950, 950]}, {"name": "Fun", "values": [80, 60, 140, 90, 120, 100]}], "prefix": "$"}),
        json!({"kind": "bar", "horizontal": true, "values": true, "labels": ["Groceries", "Rent", "Transport", "Eating out"], "series": [{"values": [212.4, 1400, 96, 180]}], "prefix": "$"}),
        json!({"kind": "line", "points": true, "labels": months, "series": [{"name": "Income", "values": [3200, 3300, 3250, 3600, 3500, 3700]}, {"name": "Spending", "values": [2400, 2600, 2550, 2900, 2700, 2650]}], "prefix": "$"}),
        json!({"kind": "area", "smooth": true, "labels": ["M", "T", "W", "T", "F", "S", "S"], "series": [{"name": "Steps", "values": [6200, 8400, 7100, 9800, 5300, 12000, 4000]}]}),
        json!({"kind": "donut", "values": true, "center": "$1,240", "labels": ["Food", "Rent", "Transport", "Fun"], "series": [{"values": [320, 650, 120, 150]}]}),
    ];
    let ops: Vec<Value> = charts.iter().enumerate().map(|(i, chart)| json!({"op": "add", "node": {
        "id": format!("c{i}"), "parent": "page-1", "kind": "frame", "x": (i % 2) as f64 * 460.0, "y": (i / 2) as f64 * 300.0,
        "w": 420, "h": 260, "padding": [16], "fill": "#FFFFFF", "radius": 12, "chart": chart}})).collect();
    apply(&e, id, Value::Array(ops));
    let path = std::env::var("CHART_DUMP").expect("CHART_DUMP names the file");
    std::fs::write(path, serde_json::to_string(&ok(&e, "design.get", json!({"design_id": id}))).unwrap()).unwrap();
}

