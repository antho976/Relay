//! The follow rule, `keep_position` and the writes a strict list normalizes (feature-layout §2.1,
//! §2.6, §3.2; MODEL-SPEC §2.11; B3, B49, B70; TM-60): a resized frame's pinned children follow
//! it, in the engine as in the canvas's Scene; a move can keep a node where it is on its page; a
//! shape gets a default size and a free line a box that fits its points; and a connector freed by
//! a delete is drawn again from the stored boxes, within range.

mod common;

use common::{call, committed_repo, engine_with_project, ok, refused};
use relay_bus::design::{Node, NodeOp};
use relay_bus::ops::design::GetOut;
use relay_client::design::scene::Scene;
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

fn read(e: &Engine, design: i64) -> GetOut {
    serde_json::from_value(ok(e, "design.get", json!({"design_id": design}))).unwrap()
}

fn node(e: &Engine, design: i64, id: &str) -> Node {
    read(e, design).nodes.into_iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no {id}"))
}

fn counted(e: &Engine, design: i64) {
    let conn = e.store.lock();
    let stored: i64 = conn.query_row("SELECT bytes FROM designs WHERE id = ?1", [design], |r| r.get(0)).unwrap();
    assert_eq!(stored, relay_core::handlers::design::recount_bytes(&conn, design).unwrap());
}

/// Undo `out` (a full answer) as the canvas does.
fn undo(e: &Engine, design: i64, out: &Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": out["inverse"], "lenient": true, "expect": out["expect"]}))
}

/// A 400 × 200 panel with a right-pinned close key, a bottom-pinned bar that stretches across,
/// a centred badge, a left-pinned note, and a right-pinned frame holding its own right-pinned dot.
fn panel() -> Value {
    json!([
        {"op": "add", "node": {"id": "panel", "parent": "page-1", "kind": "frame", "w": 400, "h": 200}},
        {"op": "add", "node": {"id": "close", "parent": "panel", "kind": "rect", "x": 360, "y": 10, "w": 24, "h": 24, "pin_x": "right"}},
        {"op": "add", "node": {"id": "bar", "parent": "panel", "kind": "rect", "x": 10, "y": 160, "w": 380, "h": 30, "pin_x": "left_right", "pin_y": "bottom"}},
        {"op": "add", "node": {"id": "badge", "parent": "panel", "kind": "rect", "x": 180, "y": 80, "w": 40, "h": 40, "pin_x": "center"}},
        {"op": "add", "node": {"id": "note", "parent": "panel", "kind": "rect", "x": 10, "y": 10, "w": 40, "h": 20}},
        {"op": "add", "node": {"id": "side", "parent": "panel", "kind": "frame", "x": 300, "y": 40, "w": 80, "h": 100, "pin_x": "left_right"}},
        {"op": "add", "node": {"id": "dot", "parent": "side", "kind": "ellipse", "x": 60, "y": 10, "w": 10, "h": 10, "pin_x": "right"}},
    ])
}

/// The stored nodes of a design as the engine has them, sorted, against the same lists run by
/// the canvas's Scene from the same start.
fn same_in_both(lists: &[Value]) {
    let (e, _root) = project();
    let id = create(&e);
    let mut scene = Scene::new(read(&e, id).nodes);
    for list in lists {
        apply(&e, id, list.clone());
        let ops: Vec<NodeOp> = serde_json::from_value(list.clone()).unwrap();
        scene.apply_all(&ops).unwrap_or_else(|why| panic!("{why}"));
        let mut engine = read(&e, id).nodes;
        engine.sort_by(|a, b| a.id.cmp(&b.id));
        let mut mine = scene.to_nodes();
        mine.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(mine, engine, "after {list}");
    }
}

// --- the follow rule --------------------------------------------------------------------------

/// A frame given a new size moves and stretches its pinned children with it; a child a list sets
/// itself keeps what it was set; the undo puts every one back.
#[test]
fn a_resized_frame_moves_its_pinned_children() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, panel());
    let before = read(&e, id);
    let out = apply(&e, id, json!([{"op": "update", "id": "panel", "set": {"w": 600, "h": 300}}]));
    let at = |id_: &str| { let n = node(&e, id, id_); (n.x, n.y, n.w, n.h) };
    assert_eq!(at("close"), (560.0, 10.0, 24.0, 24.0));
    assert_eq!(at("bar"), (10.0, 260.0, 580.0, 30.0));
    assert_eq!(at("badge"), (280.0, 80.0, 40.0, 40.0));
    assert_eq!(at("note"), (10.0, 10.0, 40.0, 20.0));
    // The side frame stretched by 200, so its own right-pinned dot moved with it.
    assert_eq!(at("side"), (300.0, 40.0, 280.0, 100.0));
    assert_eq!(at("dot"), (260.0, 10.0, 10.0, 10.0));
    let changed: Vec<&str> = out["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(changed, ["panel", "close", "bar", "badge", "side", "dot"]);
    counted(&e, id);
    undo(&e, id, &out);
    assert_eq!(read(&e, id).nodes, before.nodes);
    // Set in the same list, a child keeps what it was given on that axis.
    apply(&e, id, json!([{"op": "update", "id": "panel", "set": {"w": 500}}, {"op": "update", "id": "close", "set": {"x": 5}}]));
    assert_eq!(at("close").0, 5.0);
    assert_eq!(at("badge").0, 230.0, "the others follow");
}

/// The engine and the canvas's Scene follow alike, nested frames and fits included.
#[test]
fn the_scene_follows_as_the_engine_does() {
    same_in_both(&[
        panel(),
        json!([{"op": "update", "id": "panel", "set": {"w": 700}}]),
        json!([{"op": "update", "id": "panel", "set": {"width": 300, "height": 120}}, {"op": "update", "id": "badge", "set": {"y": 5}}]),
        json!([{"op": "add", "node": {"id": "icon", "parent": "panel", "kind": "icon", "icon": "x"}},
            {"op": "add", "node": {"id": "squiggle", "parent": "panel", "kind": "path", "x": 10, "y": 10, "points": [[0, 0], [30, -20], [60, 5]]}}]),
        json!([{"op": "update", "id": "squiggle", "set": {"points": [[-10, 0], [30, 30]]}}]),
    ]);
}

/// A chain another build nested far past the limit, resized at its top, follows level by level
/// without overflowing the stack, as far as a design nests.
#[test]
fn a_legacy_chain_five_thousand_deep_follows_without_overflow() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "f0", "parent": "page-1", "kind": "frame", "w": 100, "h": 100}}]));
    {
        let conn = e.store.lock();
        let mut insert = conn.prepare("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body, rev) VALUES (?1, ?2, ?3, 'frame', 1, ?4, 1)").unwrap();
        for i in 1..5000 {
            insert.execute(rusqlite::params![id, format!("f{i}"), format!("f{}", i - 1), r#"{"kind":"frame","w":100,"h":100,"pin_x":"left_right"}"#]).unwrap();
        }
        conn.execute("UPDATE designs SET nodes = nodes + 4999 WHERE id = ?1", [id]).unwrap();
        relay_core::handlers::design::recount_bytes(&conn, id).unwrap();
    }
    apply(&e, id, json!([{"op": "update", "id": "f0", "set": {"w": 150}}]));
    assert_eq!(node(&e, id, "f1").w, 150.0);
    assert_eq!(node(&e, id, "f64").w, 150.0, "as deep as a design nests");
    assert_eq!(node(&e, id, "f200").w, 100.0, "and no deeper");
    counted(&e, id);
}

// --- keep_position ----------------------------------------------------------------------------

/// W67: a move with keep_position keeps the node where it is on its page; one into a frame that
/// lays it out writes nothing; one whose place is laid out is refused by name; the undo puts it
/// back, x and y included.
#[test]
fn keep_position_keeps_a_node_where_it_is() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "a", "parent": "page-1", "kind": "frame", "x": 100, "y": 100, "w": 300, "h": 300}},
        {"op": "add", "node": {"id": "chip", "parent": "a", "kind": "rect", "x": 20, "y": 30, "w": 40, "h": 20}},
        {"op": "add", "node": {"id": "b", "parent": "page-1", "kind": "frame", "x": 500, "y": 50, "w": 300, "h": 300}},
        {"op": "add", "node": {"id": "row", "parent": "page-1", "kind": "frame", "x": 0, "y": 500, "w": 300, "h": 60, "layout": "row"}},
        {"op": "add", "node": {"id": "cell", "parent": "row", "kind": "rect", "w": 40, "h": 20}},
    ]));
    let before = read(&e, id);
    let out = apply(&e, id, json!([{"op": "move", "id": "chip", "parent": "b", "keep_position": true}]));
    let chip = node(&e, id, "chip");
    assert_eq!((chip.parent.as_deref(), chip.x, chip.y), (Some("b"), -380.0, 80.0), "still at 120, 130 on the page");
    counted(&e, id);
    undo(&e, id, &out);
    assert_eq!(read(&e, id).nodes, before.nodes);
    // Into a frame that lays it out: x and y are unused there, and left as they were.
    apply(&e, id, json!([{"op": "move", "id": "chip", "parent": "row", "keep_position": true}]));
    assert_eq!((node(&e, id, "chip").x, node(&e, id, "chip").y), (20.0, 30.0));
    // Out of one: where it is drawn is not stored.
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "move", "id": "cell", "parent": "b", "keep_position": true}]})));
    assert_eq!((error.code.as_str(), error.message.as_str()),
        ("design.position", "ops[0] move \"cell\": keep_position: \"cell\" sits in \"row\"'s auto layout, so where it is drawn is not stored"));
    assert_eq!(error.hint.as_deref(), Some("move it without keep_position and set x and y in an update"));
}

// --- what a strict list normalizes -------------------------------------------------------------

/// B49: a shape, frame or icon added without a size gets one; what fills or hugs, a line, and
/// what an undo puts back keep what they were given.
#[test]
fn a_shape_added_without_a_size_gets_one() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect"}},
        {"op": "add", "node": {"id": "i", "parent": "page-1", "kind": "icon", "icon": "home"}},
        {"op": "add", "node": {"id": "row", "parent": "page-1", "kind": "frame", "w": 300, "h": 50, "layout": "row"}},
        {"op": "add", "node": {"id": "fill", "parent": "row", "kind": "rect", "w_sizing": "fill", "h": 20}},
        {"op": "add", "node": {"id": "ln", "parent": "page-1", "kind": "line", "points": [[0, 0], [100, 0]]}},
    ]));
    let size = |n: &str| { let n = node(&e, id, n); (n.w, n.h) };
    assert_eq!((size("r"), size("i"), size("fill"), size("ln")), ((100.0, 100.0), (24.0, 24.0), (0.0, 20.0), (100.0, 0.0)));
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "image", "src": "0123456789abcdef0123456789abcdef.png"}}]})));
    assert_eq!(error.code, "design.node_invalid");
    ok(&e, "design.apply", json!({"design_id": id, "lenient": true, "ops": [{"op": "add", "node": {"id": "zero", "parent": "page-1", "kind": "rect"}}]}));
    assert_eq!(size("zero"), (0.0, 0.0), "an undo's re-add is put back as it was");
}

/// B3: a free line's box fits its points, on add and on an update, with the drawing where it
/// was (a turned path does not move); the undo gives back the box and the points as they were.
#[test]
fn a_free_lines_box_fits_its_points() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "ln", "parent": "page-1", "kind": "line", "x": 100, "y": 100, "points": [[0, 0], [300, -50]]}}]));
    let ln = node(&e, id, "ln");
    assert_eq!((ln.x, ln.y, ln.w, ln.h, ln.points.clone()), (100.0, 50.0, 300.0, 50.0, Some(vec![[0.0, 50.0], [300.0, 0.0]])));
    let before = read(&e, id);
    let out = apply(&e, id, json!([{"op": "update", "id": "ln", "set": {"points": [[-20, 0], [10, 10]]}}]));
    let ln = node(&e, id, "ln");
    assert_eq!((ln.x, ln.y, ln.w, ln.h), (80.0, 50.0, 30.0, 10.0));
    undo(&e, id, &out);
    assert_eq!(read(&e, id).nodes, before.nodes);
    // A turned path: refitted about the turn, so what it draws stays where it was.
    apply(&e, id, json!([{"op": "add", "node": {"id": "pa", "parent": "page-1", "kind": "path", "x": 0, "y": 0, "w": 100, "h": 100, "rotation": 90,
        "points": [[0, 0], [100, 0], [100, 100]]}}]));
    apply(&e, id, json!([{"op": "update", "id": "pa", "set": {"points": [[0, 0], [40, 0], [40, 40]]}}]));
    let pa = node(&e, id, "pa");
    assert_eq!((pa.x, pa.y, pa.w, pa.h), (60.0, 0.0, 40.0, 40.0), "its first point still drawn at the turned box's top right");
    counted(&e, id);
}

/// TM-60, B70: a connector freed by a delete is drawn again from the stored centres, not its
/// stale route, within range even across a huge span; no end names a node that is gone.
#[test]
fn a_freed_connector_is_drawn_again_from_the_boxes() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "g", "parent": "page-1", "kind": "group", "x": 900000}},
        {"op": "add", "node": {"id": "a", "parent": "g", "kind": "rect", "x": 900000, "w": 20, "h": 20}},
        {"op": "add", "node": {"id": "b", "parent": "page-1", "kind": "rect", "x": -900000, "w": 20, "h": 20}},
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "x": -900000, "from": "a", "to": "b", "points": [[0, 0], [5, 5]]}},
    ]));
    apply(&e, id, json!([{"op": "delete", "id": "g"}]));
    let link = node(&e, id, "link");
    assert!(link.from.is_none() && link.to.as_deref() == Some("b"));
    let points = link.points.unwrap();
    assert!(points.iter().flatten().all(|v| v.abs() <= 1_000_000.0), "{points:?}");
    assert_eq!(points[1], [10.0, 10.0], "b's centre, from the line's own corner");
}
