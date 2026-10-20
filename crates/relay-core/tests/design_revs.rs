//! Revisions (feature-agent-api W20; MODEL-SPEC §4; TM-58): every node row carries the rev that
//! last wrote it and every id a delete took is on record for 1000 revs, so `design.get` answers
//! only what changed since a rev, or some nodes and what is inside them; and an edit that changes
//! nothing makes no rev, event or undo.

mod common;

use common::{call, committed_repo, engine_with_project, ok, refused};
use relay_bus::design::{Node, SINCE_WINDOW};
use relay_bus::ops::design::GetOut;
use relay_bus::{BusError, ErrorKind, Event};
use relay_client::design::scene::Scene;
use relay_core::engine::Engine;
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::BTreeMap;
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

fn get(e: &Engine, payload: Value) -> Value {
    ok(e, "design.get", payload)
}

fn get_refused(e: &Engine, payload: Value) -> BusError {
    refused(call(e, "design.get", payload))
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn strings(list: &Value) -> Vec<String> {
    list.as_array().unwrap_or_else(|| panic!("not a list: {list}")).iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

fn rev(e: &Engine, design: i64) -> i64 {
    get(e, json!({"design_id": design, "pages_only": true}))["design"]["rev"].as_i64().unwrap()
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn changed_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Value> {
    drain(rx).into_iter().filter(|ev| ev.ev == "design.changed").map(|ev| ev.payload).collect()
}

/// The undo the audit kept for the last `op`, if any.
fn last_undo(e: &Engine, op: &str) -> Option<String> {
    e.store.lock().query_row("SELECT undo_op FROM audit WHERE op = ?1 AND kind = 'ok' ORDER BY id DESC LIMIT 1", [op], |r| r.get(0)).unwrap()
}

/// Move design `design` on by `revs` revs without writing a row: what that many lists elsewhere
/// in it would leave behind, for the window's tests.
fn age(e: &Engine, design: i64, revs: i64) {
    e.store.lock().execute("UPDATE designs SET rev = rev + ?2 WHERE id = ?1", params![design, revs]).unwrap();
}

fn removed_at(e: &Engine, design: i64, id: &str) -> Option<i64> {
    use rusqlite::OptionalExtension;
    e.store.lock().query_row("SELECT rev FROM design_removed WHERE design_id = ?1 AND node_id = ?2", params![design, id], |r| r.get(0))
        .optional().unwrap()
}

/// A page holding a laid-out card (a title, a photo, a group with a dot in it), a side frame
/// with a chip in it and a line from the dot to the chip, and a second page with a sticky.
/// The design is at rev 2.
fn sample(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 40, "y": 40, "w": 320, "h": 200, "layout": "column", "gap": 8}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hello", "font_size": 20}},
        {"op": "add", "node": {"id": "photo", "parent": "card", "kind": "rect", "w": 100, "h": 60, "fill": "#ff0000"}},
        {"op": "add", "node": {"id": "inner", "parent": "card", "kind": "group"}},
        {"op": "add", "node": {"id": "dot", "parent": "inner", "kind": "ellipse", "w": 8, "h": 8}},
        {"op": "add", "node": {"id": "side", "parent": "page-1", "kind": "frame", "x": 400, "w": 200, "h": 400}},
        {"op": "add", "node": {"id": "chip", "parent": "side", "kind": "rect", "x": 10, "y": 10, "w": 40, "h": 20}},
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "from": "dot", "to": "chip"}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Page 2"}},
        {"op": "add", "node": {"id": "note", "parent": "page-2", "kind": "sticky", "text": "remember"}},
    ]));
    id
}

// --- TM-58: what changes nothing makes no rev -------------------------------------------------

/// TM-58: a design.update that leaves the title and terminal as they are writes nothing: no
/// rev, no updated_at, no event, no undo. One that changes either still does all four.
#[test]
fn an_update_to_what_a_design_already_is_changes_nothing() {
    let (e, _root) = project();
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let id = create(&e);
    ok(&e, "design.update", json!({"design_id": id, "session": name}));
    let before = get(&e, json!({"design_id": id, "pages_only": true}))["design"].clone();
    let audit_rows = || -> i64 { e.store.lock().query_row("SELECT COUNT(*) FROM audit WHERE op = 'design.update' AND undo_op IS NOT NULL", [], |r| r.get(0)).unwrap() };
    let undos = audit_rows();
    let mut rx = e.subscribe();
    for same in [json!({"title": "Home"}), json!({"title": "  Home "}), json!({"session": name}), json!({"title": "Home", "session": name}), json!({})] {
        let mut payload = same.clone();
        payload["design_id"] = json!(id);
        let answer = ok(&e, "design.update", payload);
        assert_eq!(answer, before, "{same}");
    }
    assert_eq!(get(&e, json!({"design_id": id, "pages_only": true}))["design"], before);
    assert!(changed_events(&mut rx).is_empty());
    assert_eq!(audit_rows(), undos, "no undo kept");
    // A real change: a new rev, an event and an undo.
    let renamed = ok(&e, "design.update", json!({"design_id": id, "title": "Home 2"}));
    assert_eq!(renamed["rev"].as_i64(), before["rev"].as_i64().map(|rev| rev + 1));
    assert_eq!(changed_events(&mut rx).len(), 1);
    assert_eq!(audit_rows(), undos + 1);
    let untied = ok(&e, "design.update", json!({"design_id": id, "session": null}));
    assert_eq!(untied["session"], Value::Null);
    assert_eq!(ok(&e, "design.update", json!({"design_id": id, "session": null}))["rev"], untied["rev"], "untied already");
}

/// TM-58: an apply whose every edit finds what it sends already there makes no rev, event or
/// undo, and answers an empty undo; a list mixing one with a real change undoes only the change.
#[test]
fn an_apply_of_identical_sets_makes_no_rev() {
    let (e, _root) = project();
    let id = sample(&e);
    let now = rev(&e, id);
    let undo = last_undo(&e, "design.apply");
    let mut rx = e.subscribe();
    let applied = apply(&e, id, json!([
        {"op": "update", "id": "title", "set": {"text": "Hello", "font_size": 20.0}},
        {"op": "update", "id": "photo", "set": {"fill": "#ff0000", "w": 100}},
    ]));
    assert_eq!((applied["rev"].as_i64(), applied["prev_rev"].as_i64()), (Some(now), Some(now)));
    assert_eq!((applied["inverse"].clone(), applied["nodes"].clone(), applied["removed_count"].clone()), (json!([]), json!([]), json!(0)));
    assert_eq!(strings(&applied["ids"]), ["title", "photo"]);
    assert!(applied.get("audit_id").is_none());
    assert!(changed_events(&mut rx).is_empty());
    assert_eq!(rev(&e, id), now);
    // The audit row of this list keeps no undo.
    assert_eq!(last_undo(&e, "design.apply"), None);
    assert!(undo.is_some(), "the sample's list kept one");
    let mixed = apply(&e, id, json!([
        {"op": "update", "id": "title", "set": {"text": "Hello"}},
        {"op": "update", "id": "photo", "set": {"fill": "#00ff00"}},
    ]));
    assert_eq!(mixed["rev"].as_i64(), Some(now + 1));
    assert_eq!(mixed["inverse"], json!([{"op": "update", "id": "photo", "set": {"fill": "#ff0000"}}]));
    assert_eq!(ids(&mixed["nodes"]), ["photo"]);
}

// --- W20: since_rev and nodes -----------------------------------------------------------------

/// W20: since_rev answers the nodes added, changed or moved since, in design.get's order, each
/// with its place among its siblings, and the ids removed since; an id removed and added back is
/// a node, not a removal. At the design's rev it answers nothing.
#[test]
fn since_rev_returns_changed_nodes_and_removed_ids_in_order() {
    let (e, _root) = project();
    let id = sample(&e);
    let base = rev(&e, id);
    let none = get(&e, json!({"design_id": id, "since_rev": base}));
    assert_eq!((none["nodes"].clone(), none["removed"].clone(), none["indexes"].clone()), (json!([]), json!([]), json!([])));
    apply(&e, id, json!([
        {"op": "update", "id": "photo", "set": {"fill": "#00ff00"}},
        {"op": "move", "id": "title", "index": 2},
        {"op": "delete", "id": "inner"},
        {"op": "add", "node": {"id": "inner", "parent": "page-2", "kind": "rect"}},
        {"op": "add", "node": {"id": "page-3", "kind": "page"}},
    ]));
    let since = get(&e, json!({"design_id": id, "since_rev": base}));
    // `link` let go of the dot that went with the group.
    assert_eq!(ids(&since["nodes"]), ["page-3", "photo", "title", "link", "inner"]);
    assert_eq!(since["indexes"], json!([2, 0, 1, 2, 1]));
    assert_eq!(strings(&since["removed"]), ["dot"], "inner is back, so not removed");
    let photo = &since["nodes"][1];
    assert_eq!((photo["fill"].clone(), photo["parent"].clone()), (json!("#00ff00"), json!("card")), "as stored");
    assert_eq!(since["design"]["rev"].as_i64(), Some(base + 1));
    // A whole read carries neither field.
    let whole = get(&e, json!({"design_id": id}));
    assert!(whole.get("removed").is_none() && whole.get("indexes").is_none());
}

/// since_rev reaches back 1000 revs; older, or not a rev of the design, is refused by name.
#[test]
fn since_rev_older_than_the_window_is_refused() {
    let (e, _root) = project();
    let id = sample(&e);
    let at = rev(&e, id);
    let error = get_refused(&e, json!({"design_id": id, "since_rev": -1}));
    assert_eq!((error.kind, error.code.as_str(), error.message.as_str()), (ErrorKind::Invalid, "design.since_rev", "since_rev: a rev is 0 or more"));
    let error = get_refused(&e, json!({"design_id": id, "since_rev": at + 1}));
    assert_eq!((error.code.as_str(), error.message.clone()), ("design.since_rev", format!("since_rev: design {id} is at rev {at}; {} is ahead of it", at + 1)));
    age(&e, id, SINCE_WINDOW);
    let now = at + SINCE_WINDOW;
    assert!(get(&e, json!({"design_id": id, "since_rev": now - SINCE_WINDOW}))["nodes"].as_array().unwrap().is_empty(), "the window's edge reads");
    let error = get_refused(&e, json!({"design_id": id, "since_rev": now - SINCE_WINDOW - 1}));
    assert_eq!((error.kind, error.code.as_str()), (ErrorKind::Conflict, "design.since_too_old"));
    assert_eq!(error.hint.as_deref(), Some("read it whole: design.get without since_rev"));
    assert_eq!(error.details.unwrap(), json!({"rev": now}));
}

/// The removals on record are kept for the window and no longer: a list that removes something
/// forgets those no since_rev can reach.
#[test]
fn removals_older_than_the_window_are_forgotten() {
    let (e, _root) = project();
    let id = sample(&e);
    apply(&e, id, json!([{"op": "delete", "id": "note"}]));
    let gone_at = rev(&e, id);
    assert_eq!(removed_at(&e, id, "note"), Some(gone_at));
    // Still within reach of the next list's window: kept.
    age(&e, id, SINCE_WINDOW - 1);
    apply(&e, id, json!([{"op": "delete", "id": "chip"}]));
    assert_eq!(removed_at(&e, id, "note"), Some(gone_at));
    // Out of reach: forgotten once a list removes something; the new removal stays.
    age(&e, id, 5);
    apply(&e, id, json!([{"op": "update", "id": "title", "set": {"text": "Hi"}}]));
    assert_eq!(removed_at(&e, id, "note"), Some(gone_at), "only a list that removes something prunes");
    apply(&e, id, json!([{"op": "delete", "id": "photo"}]));
    let now = rev(&e, id);
    assert_eq!((removed_at(&e, id, "note"), removed_at(&e, id, "chip").is_some(), removed_at(&e, id, "photo")), (None, true, Some(now)));
    let since = get(&e, json!({"design_id": id, "since_rev": now - SINCE_WINDOW}));
    assert_eq!(strings(&since["removed"]), ["chip", "photo"]);
}

/// W20: `nodes` reads those nodes and what is inside them, in design.get's order; 1 to 500 ids;
/// an id the design does not hold is refused with the nearest one it does, and one it removed
/// says when it went.
#[test]
fn get_nodes_returns_subtrees_and_suggests_unknown_ids() {
    let (e, _root) = project();
    let id = sample(&e);
    let read = get(&e, json!({"design_id": id, "nodes": ["note", "inner", "card"]}));
    assert_eq!(ids(&read["nodes"]), ["card", "title", "photo", "inner", "dot", "note"], "each once, in the design's order");
    assert!(read.get("removed").is_none());
    let read = get(&e, json!({"design_id": id, "nodes": ["page-2"], "page": "page-1"}));
    assert!(read["nodes"].as_array().unwrap().is_empty(), "with page: only what is on it");
    assert_eq!(ids(&get(&e, json!({"design_id": id, "nodes": ["chip"], "page": "Page 1"}))["nodes"]), ["chip"]);
    for (nodes, n) in [(json!([]), 0), (Value::Array((0..501).map(|i| json!(format!("n{i}"))).collect()), 501)] {
        let error = get_refused(&e, json!({"design_id": id, "nodes": nodes}));
        assert_eq!((error.code.as_str(), error.message.clone()), ("design.nodes", format!("nodes: give 1 to 500 ids; this has {n}")));
    }
    let error = get_refused(&e, json!({"design_id": id, "nodes": ["card", "phot", "zzz"]}));
    assert_eq!((error.kind, error.code.as_str()), (ErrorKind::NotFound, "design.node_not_found"));
    assert_eq!(error.message, "nodes: \"phot\" is not in the design, nor are 1 more of them");
    assert_eq!(error.hint.as_deref(), Some("did you mean \"photo\"?"));
    assert_eq!(error.details.unwrap(), json!({"field": "nodes", "ids": ["phot", "zzz"]}));
    apply(&e, id, json!([{"op": "delete", "id": "side"}]));
    let gone = rev(&e, id);
    let error = get_refused(&e, json!({"design_id": id, "nodes": ["chip"]}));
    assert_eq!(error.message, format!("nodes: \"chip\" is not in the design (removed at rev {gone})"));
    // Since a rev before it went, a removed node is the answer: it is in `removed`, after the
    // removals in the order they went, the changed nodes the read does not take (the line that
    // let go of the chip), which a reader holding them drops.
    let since = get(&e, json!({"design_id": id, "nodes": ["chip", "card"], "since_rev": gone - 1}));
    assert_eq!((since["nodes"].clone(), strings(&since["removed"])), (json!([]), vec!["chip".to_string(), "side".to_string(), "link".to_string()]));
}

// --- A full read plus deltas is a full read ---------------------------------------------------

fn read_out(reply: Value) -> GetOut {
    serde_json::from_value(reply).unwrap_or_else(|e| panic!("not a design.get reply: {e}"))
}

/// The lists the delta tests run, one after another on the sample: updates, a reorder with a
/// change, moves between parents and pages, a delete with what is inside it, a re-add of a
/// removed id elsewhere, a page added and the pages reordered, a delete and re-add in one list.
fn history() -> Vec<Value> {
    vec![
        json!([{"op": "update", "id": "title", "set": {"text": "Hi"}}]),
        json!([{"op": "move", "id": "photo", "index": 0}, {"op": "update", "id": "photo", "set": {"fill": "#00ff00"}}]),
        json!([{"op": "add", "node": {"id": "badge", "parent": "side", "kind": "rect", "w": 4, "h": 4}}, {"op": "move", "id": "chip", "parent": "card", "index": 1}]),
        json!([{"op": "delete", "id": "inner"}]),
        json!([{"op": "add", "node": {"id": "dot", "parent": "page-2", "kind": "ellipse", "w": 9, "h": 9}}]),
        json!([{"op": "move", "id": "side", "parent": "page-2"}]),
        json!([{"op": "add", "node": {"id": "page-3", "kind": "page", "name": "Third"}}, {"op": "add", "node": {"id": "shell", "parent": "page-3", "kind": "frame"}},
            {"op": "add", "node": {"id": "kid", "parent": "shell", "kind": "text", "text": "k"}}]),
        json!([{"op": "move", "id": "page-2", "index": 0}, {"op": "move", "id": "note", "parent": "card"}]),
        json!([{"op": "move", "id": "shell", "parent": "page-1"}, {"op": "update", "id": "kid", "set": {"text": "kk"}}]),
        json!([{"op": "delete", "id": "title"}, {"op": "add", "node": {"id": "title", "parent": "shell", "kind": "text", "text": "Back"}}]),
        json!([{"op": "move", "id": "side", "parent": "card", "index": 0}, {"op": "delete", "id": "badge"}]),
    ]
}

/// The scene a read builds.
fn scene_of(out: &GetOut) -> Scene {
    Scene::new(out.nodes.clone())
}

/// `base` patched with a since_rev read, as the canvas brings its scene up to date (TM-80).
fn patched(base: &Scene, delta: GetOut) -> Scene {
    let indexes = delta.indexes.expect("a since_rev read places its nodes");
    assert_eq!(indexes.len(), delta.nodes.len());
    let mut scene = base.clone();
    scene.patch(delta.nodes.into_iter().zip(indexes).collect(), &delta.removed.expect("a since_rev read lists removals"))
        .unwrap_or_else(|why| panic!("the delta does not fit the scene: {why}"));
    scene
}

/// W20: a scene built from a whole read at one rev and patched with a since_rev read of every
/// rev after it holds what a whole read at the later rev builds, node for node and in order:
/// over adds, updates, moves within a parent and to another one or another page, deletes,
/// re-adds and page reorders, a rev at a time and across all of them at once. Read cut to a
/// page, the same holds for that page's read, nodes moving in and out of it with what they hold.
#[test]
fn a_whole_read_plus_what_changed_since_is_a_whole_read() {
    let (e, _root) = project();
    let id = sample(&e);
    let first = rev(&e, id);
    let reads = |page: Option<&str>| {
        let mut payload = json!({"design_id": id});
        if let Some(page) = page {
            payload["page"] = json!(page);
        }
        payload
    };
    let mut kept: Vec<(i64, BTreeMap<&str, Scene>)> = Vec::new();
    let snapshot = |e: &Engine| -> BTreeMap<&str, Scene> {
        [("whole", None), ("page-1", Some("page-1")), ("page-2", Some("Page 2"))].into_iter()
            .map(|(name, page)| (name, scene_of(&read_out(get(e, reads(page))))))
            .collect()
    };
    kept.push((first, snapshot(&e)));
    for (step, ops) in history().into_iter().enumerate() {
        apply(&e, id, ops.clone());
        let now = snapshot(&e);
        // From every rev kept so far: a rev at a time, and across many.
        for (at, then) in &kept {
            for (name, page) in [("whole", None), ("page-1", Some("page-1")), ("page-2", Some("Page 2"))] {
                let mut payload = reads(page);
                payload["since_rev"] = json!(at);
                let delta = read_out(get(&e, payload));
                let scene = patched(&then[name], delta);
                assert_eq!(scene.to_nodes(), now[name].to_nodes(), "step {step} ({ops}), {name} since rev {at}");
            }
        }
        kept.push((rev(&e, id), now));
    }
    assert_eq!(kept.len(), history().len() + 1);
}

/// A reader of some nodes, holding them by id: dropping what a since_rev read removes and
/// putting in what it answers gives what a whole read of those nodes gives, as nodes move in
/// and out of them with what they hold.
#[test]
fn a_read_of_some_nodes_plus_what_changed_since_is_a_read_of_them() {
    let (e, _root) = project();
    let id = sample(&e);
    let by_id = |nodes: &[Node]| -> BTreeMap<String, Value> { nodes.iter().map(|n| (n.id.clone(), serde_json::to_value(n).unwrap())).collect() };
    let mut kept: Vec<(i64, BTreeMap<String, Value>)> = Vec::new();
    let wanted = json!(["card", "side"]);
    let read = |since: Option<i64>| {
        let mut payload = json!({"design_id": id, "nodes": wanted});
        if let Some(since) = since {
            payload["since_rev"] = json!(since);
        }
        read_out(get(&e, payload))
    };
    kept.push((rev(&e, id), by_id(&read(None).nodes)));
    for (step, ops) in history().into_iter().enumerate() {
        // The named nodes stay: the history deletes neither (it moves side into card at the end).
        apply(&e, id, ops.clone());
        let now = by_id(&read(None).nodes);
        for (at, then) in &kept {
            let delta = read(Some(*at));
            let mut held = then.clone();
            for gone in delta.removed.as_deref().unwrap_or_default() {
                held.remove(gone);
            }
            held.extend(by_id(&delta.nodes));
            assert_eq!(held, now, "step {step} ({ops}) since rev {at}");
        }
        kept.push((rev(&e, id), now));
    }
}

/// A row of a kind this build does not know is no sibling a reader holds: the places a delta
/// gives count past it, as a whole read leaves it out.
#[test]
fn a_delta_places_nodes_among_the_siblings_a_read_answers() {
    let (e, _root) = project();
    let id = sample(&e);
    e.store.lock().execute("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body, rev)
        VALUES (?1, 'future', 'card', 'component', 0.5, '{\"kind\":\"component\"}', 2)", [id]).unwrap();
    let base = read_out(get(&e, json!({"design_id": id})));
    assert!(!base.nodes.iter().any(|n| n.id == "future"));
    apply(&e, id, json!([{"op": "move", "id": "photo", "index": 3}, {"op": "update", "id": "title", "set": {"text": "x"}}]));
    let delta = read_out(get(&e, json!({"design_id": id, "since_rev": base.design.rev})));
    // card holds future (unreadable), title, inner, photo: the reader holds title, inner, photo.
    assert_eq!(delta.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["title", "photo"]);
    assert_eq!(delta.indexes, Some(vec![0, 2]));
    let scene = patched(&scene_of(&base), delta);
    assert_eq!(scene.to_nodes(), scene_of(&read_out(get(&e, json!({"design_id": id})))).to_nodes());
}
