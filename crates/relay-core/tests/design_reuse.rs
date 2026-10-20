//! Reuse in `design.apply` (feature-reuse §2.2, §8; MODEL-SPEC §1 #20, #30, §2.11): `copy` makes a
//! subtree again under ids by `copy_id`, joining connectors inside it and letting go of ends
//! outside it; `color`, `style` and `delete_style` keep the design's named colours and styles in
//! one column, and every user of a deleted one keeps its look. Each is undone by its inverse, and
//! the scene applies the colour and style edits as the engine does.

mod common;

use common::{call, committed_repo, engine_with_project, ok, refused};
use relay_bus::design::{Node, NodeOp, Styles};
use relay_bus::ops::design::GetOut;
use relay_bus::ErrorKind;
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

fn get(e: &Engine, design: i64) -> Value {
    ok(e, "design.get", json!({"design_id": design}))
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn node(e: &Engine, design: i64, id: &str) -> Option<Value> {
    get(e, design)["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned()
}

/// Undo `out` (a full answer) as the canvas does, and answer the undo's own answer.
fn undo(e: &Engine, design: i64, out: &Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": out["inverse"], "lenient": true, "expect": out["expect"]}))
}

/// The stored size and what the rows and styles count to agree.
fn counted(e: &Engine, design: i64) {
    let conn = e.store.lock();
    let stored: i64 = conn.query_row("SELECT bytes FROM designs WHERE id = ?1", [design], |r| r.get(0)).unwrap();
    assert_eq!(stored, relay_core::handlers::design::recount_bytes(&conn, design).unwrap());
}

/// A design.get with its rev and dates taken out: what two reads of the same design share.
fn content(e: &Engine, design: i64) -> Value {
    let mut read = get(e, design);
    for key in ["rev", "updated_at", "updated_by"] {
        read["design"].as_object_mut().unwrap().remove(key);
    }
    read
}

/// A page holding a card (a title, a photo, a group with a dot in it), a chip beside it, a line
/// from the dot to the chip and one from the title to the photo.
fn sample(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 40, "y": 40, "w": 320, "h": 200}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "x": 10, "y": 10, "text": "Hello"}},
        {"op": "add", "node": {"id": "photo", "parent": "card", "kind": "rect", "x": 10, "y": 60, "w": 100, "h": 60}},
        {"op": "add", "node": {"id": "inner", "parent": "card", "kind": "group"}},
        {"op": "add", "node": {"id": "dot", "parent": "inner", "kind": "ellipse", "x": 200, "y": 20, "w": 8, "h": 8}},
        {"op": "add", "node": {"id": "chip", "parent": "page-1", "kind": "rect", "x": 500, "y": 40, "w": 40, "h": 20}},
        {"op": "add", "node": {"id": "link", "parent": "card", "kind": "line", "from": "dot", "to": "chip"}},
        {"op": "add", "node": {"id": "card-arrow", "parent": "card", "kind": "line", "from": "title", "to": "photo"}},
    ]));
    id
}

// --- copy -------------------------------------------------------------------------------------

/// A copy makes the subtree again under ids by copy_id's rule, in order, its parents mapped, the
/// root moved by dx; `copied` maps it back, and a later edit of the list names a copy.
#[test]
fn a_copy_duplicates_a_subtree_with_derived_ids() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([
        {"op": "copy", "id": "card", "as": "card-2", "dx": 400},
        {"op": "update", "id": "card-2:title", "set": {"text": "Copy"}},
    ]));
    assert_eq!(out["ids"], json!(["card-2", "card-2:title"]));
    assert_eq!(out["copied"], json!({"card-2": "card"}));
    let read = get(&e, id);
    let copies: Vec<String> = ids(&read["nodes"]).into_iter().filter(|n| n.starts_with("card-2")).collect();
    assert_eq!(copies, ["card-2", "card-2:title", "card-2:photo", "card-2:inner", "card-2:dot", "card-2:link", "card-2-arrow"]);
    let copy = node(&e, id, "card-2").unwrap();
    assert_eq!((copy["x"].as_f64(), copy["parent"].as_str()), (Some(440.0), Some("page-1")));
    assert_eq!(node(&e, id, "card-2:dot").unwrap()["parent"], "card-2:inner");
    assert_eq!(node(&e, id, "card-2:title").unwrap()["text"], "Copy");
    assert_eq!(node(&e, id, "title").unwrap()["text"], "Hello", "the original is as it was");
    counted(&e, id);
}

/// MODEL-SPEC §1 #30: a connector inside a copy joins the copies of its ends; an end outside the
/// copy lets go where the stored boxes draw it, so the copy's line looks as the original's does.
#[test]
fn a_copy_repoints_connectors_inside_and_lets_go_of_ends_outside() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "copy", "id": "card", "as": "card-2", "dy": 300}]));
    let arrow = node(&e, id, "card-2-arrow").unwrap();
    assert_eq!((arrow["from"].as_str(), arrow["to"].as_str()), (Some("card-2:title"), Some("card-2:photo")));
    let link = node(&e, id, "card-2:link").unwrap();
    assert_eq!(link["from"], "card-2:dot", "the end inside is the copy's");
    assert!(link.get("to").is_none(), "the end outside let go: {link}");
    // Drawn from the original's box: the dot's centre (40+200+4, 40+20+4) and the chip's
    // (500+20, 40+10), from the line's box at the card's corner (40, 40).
    assert_eq!(link["points"], json!([[204.0, 24.0], [480.0, 10.0]]));
    // The original line keeps both ends.
    assert_eq!(node(&e, id, "link").unwrap()["to"], "chip");
}

/// A copy's undo deletes it; the undo's undo makes it again; audit.undo undoes it too.
#[test]
fn a_copy_is_undone_by_deleting_it() {
    let (e, _root) = project();
    let id = sample(&e);
    let before = content(&e, id);
    let out = apply(&e, id, json!([{"op": "copy", "id": "card", "as": "card-2", "dx": 400}]));
    assert_eq!(out["inverse"], json!([{"op": "delete", "id": "card-2"}]));
    let after = content(&e, id);
    let back = undo(&e, id, &out);
    assert_eq!(content(&e, id)["nodes"], before["nodes"]);
    undo(&e, id, &back);
    assert_eq!(content(&e, id)["nodes"], after["nodes"]);
    counted(&e, id);
    // The audit's undo of the redo.
    let row = e.store.lock().query_row("SELECT MAX(id) FROM audit WHERE op = 'design.apply'", [], |r| r.get::<_, i64>(0)).unwrap();
    ok(&e, "audit.undo", json!({"audit_id": row}));
    assert_eq!(content(&e, id)["nodes"], before["nodes"]);
}

/// A copy is refused for a taken id, an id that would be too long, two ids that would copy to
/// one, a place inside what it copies, and the node cap, each naming the edit.
#[test]
fn a_copy_refuses_taken_ids_long_ids_cycles_and_the_cap() {
    let (e, _root) = project();
    let id = sample(&e);
    let refusal = |op: Value| refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "chip", "set": {"x": 1}}, op]})));
    let error = refusal(json!({"op": "copy", "id": "card", "as": "chip"}));
    assert_eq!((error.kind, error.code.as_str(), error.message.as_str()), (ErrorKind::Conflict, "design.node_exists", "ops[1] copy \"card\": \"chip\" is already in the design"));
    assert_eq!(error.details.as_ref().unwrap()["index"], 1);
    let long = "c".repeat(60);
    let error = refusal(json!({"op": "copy", "id": "card", "as": long}));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.node_invalid", "ops[1] copy \"card\": the copy of \"title\" would be over 64 characters"));
    let error = refusal(json!({"op": "copy", "id": "card", "parent": "dot"}));
    assert_eq!(error.code, "design.parent", "a dot holds nothing");
    let error = refusal(json!({"op": "copy", "id": "card", "parent": "inner"}));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.cycle", "ops[1] copy \"card\": \"inner\" is inside \"card\", which this copies"));
    // "x" and "card:x" inside "card" would both be "card-2:x".
    apply(&e, id, json!([{"op": "add", "node": {"id": "x", "parent": "card", "kind": "rect"}}, {"op": "add", "node": {"id": "card:x", "parent": "card", "kind": "rect"}}]));
    let error = refusal(json!({"op": "copy", "id": "card", "as": "card-2"}));
    assert_eq!(error.message, "ops[1] copy \"card\": \"x\" and \"card:x\" would both be copied as \"card-2:x\"");
    // Past the cap.
    let fill: Vec<Value> = (0..1990).map(|i| json!({"op": "add", "node": {"id": format!("f{i}"), "parent": "card", "kind": "rect"}})).collect();
    apply(&e, id, Value::Array(fill.clone()));
    apply(&e, id, Value::Array(fill.iter().map(|op| { let mut op = op.clone(); op["node"]["id"] = json!(format!("g-{}", op["node"]["id"].as_str().unwrap())); op }).collect()));
    let error = refusal(json!({"op": "copy", "id": "card"}));
    assert_eq!(error.code, "design.too_large");
    assert_eq!(node(&e, id, "chip").unwrap()["x"].as_f64(), Some(500.0), "nothing of the list stayed");
}

/// `rename` picks a copy's ids: the longest start that fits is swapped, the root too when `as`
/// is left out, and ids no start fits keep copy_id's rule. A bad rename is refused by its field.
#[test]
fn a_copy_renames_the_ids_it_makes() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "v2-phone", "parent": "page-1", "kind": "frame", "w": 390, "h": 844}},
        {"op": "add", "node": {"id": "v2-fade", "parent": "v2-phone", "kind": "rect"}},
        {"op": "add", "node": {"id": "v2-head-title", "parent": "v2-phone", "kind": "text", "text": "Trail"}},
        {"op": "add", "node": {"id": "badge", "parent": "v2-phone", "kind": "ellipse"}},
    ]));
    let out = apply(&e, id, json!([
        {"op": "copy", "id": "v2-phone", "dx": 470, "rename": {"v2-": "v3-", "v2-head-": "v3-top-"}},
        {"op": "update", "id": "v3-fade", "set": {"opacity": 0.5}},
    ]));
    assert_eq!(out["ids"], json!(["v3-phone", "v3-fade"]));
    let made: Vec<String> = ids(&get(&e, id)["nodes"]).into_iter().filter(|n| !n.starts_with("v2-") && n != "badge" && n != "page-1").collect();
    assert_eq!(made, ["v3-phone", "v3-fade", "v3-top-title", "v3-phone:badge"], "the longest start, and the plain rule for the rest");
    let again = apply(&e, id, json!([{"op": "copy", "id": "v2-phone", "as": "v4", "rename": {"v2-": "v4-"}}]));
    assert_eq!(again["ids"], json!(["v4"]), "as names the root");
    assert!(node(&e, id, "v4-fade").is_some());
    for (rename, why) in [(json!({}), "rename: 1 to 16 id starts"), (json!({"v2-": "bad id"}), "rename: \"v2-\" to \"bad id\"")] {
        let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "copy", "id": "v2-phone", "rename": rename}]})));
        assert!(error.message.contains(why), "{}", error.message);
        assert_eq!(error.details.as_ref().unwrap()["field"], "rename");
    }
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "copy", "id": "v2-phone", "rename": {"v2-": "v3-"}}]})));
    assert_eq!(error.code, "design.node_exists", "{}", error.message);
}

/// A later edit naming an id no copy made is told what the copy made: the copy of the id it is
/// nearest when there is one, else the copy's ids.
#[test]
fn a_refused_id_after_a_copy_names_the_copys_ids() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "v2-phone", "parent": "page-1", "kind": "frame", "w": 390, "h": 844}},
        {"op": "add", "node": {"id": "v2-fade", "parent": "v2-phone", "kind": "rect"}},
    ]));
    let wrong = |guess: &str| refused(call(&e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "copy", "id": "v2-phone", "as": "v3-phone"},
        {"op": "update", "id": guess, "set": {"opacity": 0.5}},
    ]})));
    let error = wrong("v3-fade");
    assert_eq!(error.code, "design.node_not_found");
    let hint = error.hint.unwrap();
    assert!(hint.contains("did you mean \"v2-fade\"?") && hint.contains("ops[0] copied \"v2-fade\" as \"v3-phone:v2-fade\""), "{hint}");
    let hint = wrong("totally-else").hint.unwrap();
    assert!(hint.contains("ops[0]'s copy made \"v3-phone\", \"v3-phone:v2-fade\""), "{hint}");
}

/// A page copies as a page, after the others; an unnamed copy takes an id from the counter.
#[test]
fn a_page_copies_as_a_page_and_an_unnamed_copy_takes_an_assigned_id() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = apply(&e, id, json!([{"op": "copy", "id": "page-1", "as": "page-2"}, {"op": "copy", "id": "chip"}]));
    let read = get(&e, id);
    let pages: Vec<String> = read["nodes"].as_array().unwrap().iter().filter(|n| n["kind"] == "page").map(|n| n["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(pages, ["page-1", "page-2"]);
    assert_eq!(read["design"]["pages"], 2);
    assert_eq!(node(&e, id, "page-2:card").unwrap()["parent"], "page-2");
    let assigned = out["ids"][1].as_str().unwrap();
    assert!(assigned.starts_with("rect-"), "{assigned}");
    assert_eq!(node(&e, id, assigned).unwrap()["parent"], "page-1");
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "copy", "id": "page-1", "parent": "card"}]})));
    assert_eq!(error.message, "ops[0] copy \"page-1\": a page's copy is a page; it sits in no parent");
    counted(&e, id);
}

// --- colours and styles -----------------------------------------------------------------------

/// Named colours and styles: design.get answers them; a colour change leaves the rows as they
/// were and moves the rev; delete_style writes into each user what it drew; each is undone
/// exactly, through the canvas's undo and audit.undo.
#[test]
fn colours_and_styles_restyle_their_users_and_undo() {
    let (e, _root) = project();
    let id = sample(&e);
    let made = apply(&e, id, json!([
        {"op": "color", "name": "accent", "value": "#4C9DFF"},
        {"op": "style", "name": "button", "set": {"fill": "$accent", "radius": 8}},
        {"op": "update", "id": "photo", "set": {"style": "button"}},
        {"op": "update", "id": "chip", "set": {"stroke": "$accent"}},
    ]));
    assert_eq!((made["colors"].clone(), made["styles"]["button"]["radius"].as_f64()), (json!({"accent": "#4C9DFF"}), Some(8.0)));
    let read = get(&e, id);
    assert_eq!((read["colors"].clone(), read["styles"].clone()), (json!({"accent": "#4C9DFF"}), json!({"button": {"fill": "$accent", "radius": 8.0}})));
    let rows = |e: &Engine| -> Vec<(String, String)> {
        e.store.lock().prepare("SELECT node_id, body FROM design_nodes WHERE design_id = 1 ORDER BY node_id").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
    };
    let (body_before, rev_before) = (rows(&e), read["design"]["rev"].as_i64().unwrap());
    let recolored = apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#FF0000"}]));
    assert_eq!((rows(&e), recolored["rev"].as_i64()), (body_before.clone(), Some(rev_before + 1)), "a colour change writes no node");
    assert_eq!(recolored["inverse"], json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}]));
    // Naming it as it is changes nothing.
    assert_eq!(apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#FF0000"}]))["rev"].as_i64(), Some(rev_before + 1));
    let before_delete = content(&e, id);
    let deleted = apply(&e, id, json!([{"op": "delete_style", "name": "button"}, {"op": "delete_style", "name": "accent"}]));
    let photo = node(&e, id, "photo").unwrap();
    assert_eq!((photo.get("style"), photo["fill"].as_str(), photo["radius"].as_f64()), (None, Some("#FF0000"), Some(8.0)));
    assert_eq!(node(&e, id, "chip").unwrap()["stroke"], "#FF0000");
    let after = get(&e, id);
    assert!(after.get("colors").is_none() && after.get("styles").is_none(), "{after}");
    counted(&e, id);
    // The undo makes them again and names them in their users again.
    undo(&e, id, &deleted);
    assert_eq!(content(&e, id), before_delete);
    counted(&e, id);
    // audit.undo of the colour change.
    ok(&e, "audit.undo", json!({"audit_id": recolored["audit_id"]}));
    assert_eq!(get(&e, id)["colors"], json!({"accent": "#4C9DFF"}));
}

/// A node naming what the design does not hold is refused by name; a lenient list (an undo
/// putting back a node whose colour went since) leaves the name out and tells an agent.
#[test]
fn unknown_names_are_refused_and_lenient_clears_them() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}, {"op": "update", "id": "chip", "set": {"fill": "$accent"}}]));
    let removed = apply(&e, id, json!([{"op": "delete", "id": "chip"}]));
    apply(&e, id, json!([{"op": "delete_style", "name": "accent"}]));
    let back = undo(&e, id, &removed);
    assert!(back.get("skipped").is_none(), "{back}");
    assert!(node(&e, id, "chip").unwrap().get("fill").is_none(), "the colour that went is left out");
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "chip", "set": {"fill": "$accent"}}]})));
    assert_eq!(error.code, "design.style_not_found");
    // A colour and a style share one namespace.
    apply(&e, id, json!([{"op": "style", "name": "card", "set": {"radius": 4}}]));
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "card", "value": "#fff"}]})));
    assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Conflict, "design.style_exists"));
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "x", "value": "blue"}]})));
    assert_eq!(error.code, "design.style_invalid");
    let error = refused(call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "delete_style", "name": "nope"}]})));
    assert_eq!(error.code, "design.style_not_found");
    // A lenient delete of a name that is gone is skipped and says so.
    let out = ok(&e, "design.apply", json!({"design_id": id, "lenient": true, "ops": [{"op": "delete_style", "name": "nope"}]}));
    assert_eq!(out["skipped"], json!([{"at": 0, "id": "nope", "reason": "missing"}]));
}

/// A duplicate keeps the design's colours and styles, and so does a backup taken and restored.
#[test]
fn a_duplicate_and_a_backup_keep_colours_and_styles() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}, {"op": "style", "name": "button", "set": {"fill": "$accent"}}]));
    let copy = ok(&e, "design.duplicate", json!({"design_id": id}))["id"].as_i64().unwrap();
    let read = get(&e, copy);
    assert_eq!((read["colors"].clone(), read["styles"]["button"]["fill"].clone()), (json!({"accent": "#4C9DFF"}), json!("$accent")));
    counted(&e, copy);
    ok(&e, "project.remove", json!({"project_id": 1}));
    let backup = ok(&e, "project.removed.list", json!({}))["removed"][0]["backup_path"].as_str().unwrap().to_string();
    ok(&e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    assert_eq!(get(&e, id)["colors"], json!({"accent": "#4C9DFF"}));
    counted(&e, id);
}

/// An event from a list that changed the colours or styles says so.
#[test]
fn a_colour_change_is_flagged_in_its_event() {
    let (e, _root) = project();
    let id = sample(&e);
    let mut rx = e.subscribe();
    apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}]));
    apply(&e, id, json!([{"op": "update", "id": "chip", "set": {"x": 1}}]));
    let flags: Vec<Option<bool>> = std::iter::from_fn(|| rx.try_recv().ok()).filter(|ev| ev.ev == "design.changed")
        .map(|ev| ev.payload.get("styles").and_then(Value::as_bool)).collect();
    assert_eq!(flags, [Some(true), None]);
}

/// Every reuse edit and its undo: the inverse gives back the design as it was, colours and
/// styles included, and the inverse's inverse gives back what the edit made; the size stays as
/// counted throughout.
#[test]
fn reuse_edits_and_their_undos_round_trip() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}, {"op": "style", "name": "button", "set": {"fill": "$accent"}},
        {"op": "update", "id": "photo", "set": {"style": "button"}}]));
    for ops in [
        json!([{"op": "copy", "id": "card", "as": "card-2", "dx": 400}]),
        json!([{"op": "color", "name": "accent", "value": "#00FF00"}, {"op": "color", "name": "ink", "value": "#111111"}]),
        json!([{"op": "style", "name": "button", "set": {"radius": 12, "fill": null}}, {"op": "style", "name": "label", "set": {"font_size": 18}}]),
        json!([{"op": "delete_style", "name": "button"}]),
        json!([{"op": "delete_style", "name": "accent"}]),
    ] {
        let before = content(&e, id);
        let out = apply(&e, id, ops.clone());
        let after = content(&e, id);
        counted(&e, id);
        let back = undo(&e, id, &out);
        assert_eq!(content(&e, id), before, "undo of {ops}");
        counted(&e, id);
        undo(&e, id, &back);
        assert_eq!(content(&e, id), after, "redo of {ops}");
        counted(&e, id);
    }
}

/// The scene runs the same colour and style lists as the engine: what it draws is what the
/// engine's design draws.
#[test]
fn the_scene_runs_colour_and_style_edits_as_the_engine_does() {
    let (e, _root) = project();
    let id = sample(&e);
    let read: GetOut = serde_json::from_value(get(&e, id)).unwrap();
    let mut scene = Scene::new(read.nodes);
    let lists = [
        json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}, {"op": "style", "name": "button", "set": {"fill": "$accent", "radius": 6}},
            {"op": "update", "id": "photo", "set": {"style": "button"}}, {"op": "update", "id": "chip", "set": {"stroke": "$accent", "fill": "$accent"}}]),
        json!([{"op": "color", "name": "accent", "value": "#FF0000"}]),
        json!([{"op": "style", "name": "button", "set": {"radius": null, "stroke": "$accent"}}]),
        json!([{"op": "delete_style", "name": "accent"}]),
        json!([{"op": "delete_style", "name": "button"}]),
    ];
    for list in lists {
        apply(&e, id, list.clone());
        let ops: Vec<NodeOp> = serde_json::from_value(list.clone()).unwrap();
        scene.apply_all(&ops).unwrap();
        let read: GetOut = serde_json::from_value(get(&e, id)).unwrap();
        let engine = Styles { colors: read.colors, styles: read.styles };
        assert_eq!(scene.styles(), &engine, "after {list}");
        for stored in &read.nodes {
            let drawn: Node = engine.resolve(stored);
            assert_eq!(scene.node(&stored.id), Some(&drawn), "{} after {list}", stored.id);
        }
    }
}

/// Review: the undo of a style change gives back the whole style, so one replayed after the
/// style was deleted makes it again whole; one naming a colour deleted since leaves it out.
#[test]
fn a_style_undo_replayed_after_a_delete_makes_it_whole() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "style", "name": "card", "set": {"fill": "#FF0000", "radius": 4}}]));
    let changed = apply(&e, id, json!([{"op": "style", "name": "card", "set": {"fill": "#00FF00"}}]));
    apply(&e, id, json!([{"op": "delete_style", "name": "card"}]));
    undo(&e, id, &changed);
    assert_eq!(get(&e, id)["styles"]["card"], json!({"fill": "#FF0000", "radius": 4.0}));

    // An agent deletes a style naming a colour; the colour goes; the agent's change is undone.
    apply(&e, id, json!([{"op": "color", "name": "ink", "value": "#111111"}, {"op": "style", "name": "label", "set": {"fill": "$ink", "radius": 2}}]));
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let theirs = common::ok_as(&e, relay_bus::Actor::agent(&name), "design.apply", json!({"design_id": id, "ops": [{"op": "delete_style", "name": "label"}]}));
    apply(&e, id, json!([{"op": "delete_style", "name": "ink"}]));
    ok(&e, "audit.undo", json!({"audit_id": theirs["audit_id"]}));
    assert_eq!(get(&e, id)["styles"]["label"], json!({"radius": 2.0}), "made again without the colour that went");
    counted(&e, id);
}
