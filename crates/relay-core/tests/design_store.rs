//! A design's rows as store v30 keeps them (MODEL-SPEC §4, §5 #4): `designs.bytes` counts each
//! node as design.get returns it, ids and parents included (B60), so the size cap keeps a design
//! under the clients' 2 MiB line; every row a write touches carries the rev of the list that wrote
//! it, and every id a delete removes is on record; a store or a backup from v29 comes forward
//! counted and dated. And who a duplicate belongs to (TM-14).

mod common;

use common::{call, committed_repo, engine_with_project, ok, ok_as};
use relay_bus::design::{MAX_DESIGN_BYTES, NODE_OVERHEAD};
use relay_bus::Actor;
use relay_core::engine::Engine;
use relay_core::handlers::design::recount_bytes;
use relay_core::store::MIGRATIONS;
use relay_core::Store;
use rusqlite::{params, Connection, OptionalExtension};
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

fn bytes(e: &Engine, design: i64) -> i64 {
    e.store.lock().query_row("SELECT bytes FROM designs WHERE id = ?1", [design], |r| r.get(0)).unwrap()
}

/// `designs.bytes` is what the design's rows count to: every write kept it.
#[track_caller]
fn counted(e: &Engine, design: i64) {
    let kept = bytes(e, design);
    let counted = recount_bytes(&e.store.lock(), design).unwrap();
    assert_eq!(kept, counted, "design {design}'s bytes drifted from its rows");
}

fn rev(e: &Engine, design: i64) -> i64 {
    e.store.lock().query_row("SELECT rev FROM designs WHERE id = ?1", [design], |r| r.get(0)).unwrap()
}

/// The rev row `id` was last written at.
fn row_rev(e: &Engine, design: i64, id: &str) -> Option<i64> {
    e.store.lock().query_row("SELECT rev FROM design_nodes WHERE design_id = ?1 AND node_id = ?2", params![design, id], |r| r.get(0))
        .optional().unwrap()
}

/// The rev `id` was removed at, if it is on record as removed.
fn removed_at(e: &Engine, design: i64, id: &str) -> Option<i64> {
    e.store.lock().query_row("SELECT rev FROM design_removed WHERE design_id = ?1 AND node_id = ?2", params![design, id], |r| r.get(0))
        .optional().unwrap()
}

fn long(name: &str) -> String {
    format!("{name}-{}", "x".repeat(40))
}

/// B60, MODEL-SPEC §4: create, add, update, move, delete, the release of a connector's end, an
/// undo and a duplicate each leave `bytes` at what the rows count to.
#[test]
fn bytes_follow_every_write() {
    let (e, _root) = project();
    let id = create(&e);
    counted(&e, id);
    let (card, side) = (long("card"), "s".to_string());
    apply(&e, id, json!([
        {"op": "add", "node": {"id": card, "parent": "page-1", "kind": "frame", "w": 300, "h": 200}},
        {"op": "add", "node": {"id": side, "parent": "page-1", "kind": "frame", "x": 400, "w": 100, "h": 100}},
        {"op": "add", "node": {"id": "a", "parent": card, "kind": "rect", "w": 20, "h": 20}},
        {"op": "add", "node": {"id": long("b"), "parent": card, "kind": "text", "text": "hello"}},
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "from": "a", "to": long("b")}},
        {"op": "add", "node": {"kind": "page", "name": "Page 2"}},
    ]));
    counted(&e, id);
    apply(&e, id, json!([{"op": "update", "id": long("b"), "set": {"text": "a much longer greeting than before"}},
        {"op": "update", "id": "a", "set": {"fill": "#4C9DFF", "name": "Square"}}]));
    counted(&e, id);
    // Moves between parents whose ids differ in length, and a page reordered among the pages.
    apply(&e, id, json!([{"op": "move", "id": "a", "parent": side}, {"op": "move", "id": long("b"), "parent": "page-1"},
        {"op": "move", "id": "page-1", "index": 1}]));
    counted(&e, id);
    // A delete of a node a connector joins: the connector lets go of that end (release_ends).
    let out = apply(&e, id, json!([{"op": "delete", "id": side}]));
    let link = out["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "link").expect("the connector was freed").clone();
    assert!(link.get("from").is_none() && link["points"].is_array(), "{link}");
    counted(&e, id);
    // Undone: everything back, connector and all.
    ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true}));
    counted(&e, id);
    let copy = ok(&e, "design.duplicate", json!({"design_id": id}))["id"].as_i64().unwrap();
    assert_eq!(bytes(&e, copy), bytes(&e, id), "a copy is as large as its original");
    counted(&e, copy);
    // Deleting a whole page takes its rows' bytes with it.
    apply(&e, id, json!([{"op": "delete", "id": "page-1"}]));
    counted(&e, id);
    // The page a design starts with counts its id and the JSON around it.
    let fresh = create(&e);
    let body: String = e.store.lock().query_row("SELECT body FROM design_nodes WHERE design_id = ?1", [fresh], |r| r.get(0)).unwrap();
    assert_eq!(bytes(&e, fresh), (body.len() + "page-1".len()) as i64 + NODE_OVERHEAD);
}

/// B60: a design held to the size cap reads back on one client line, whatever its ids. Counted
/// by its bodies alone, a design of 64-character ids reached 5000 nodes and a 2.2 MB design.get,
/// which the desktop could not open.
#[test]
fn a_full_design_still_fits_one_client_line() {
    let (e, _root) = project();
    let id = create(&e);
    let frame = "f".repeat(64);
    apply(&e, id, json!([{"op": "add", "node": {"id": frame, "parent": "page-1", "kind": "frame", "w": 400, "h": 400}}]));
    let name = "n".repeat(200);
    let mut added = 0;
    let stop = loop {
        let ops: Vec<Value> = (added..added + 500).map(|i| json!({"op": "add", "node": {"id": format!("{i:0>64}"), "parent": frame,
            "kind": "rect", "name": name, "w": 10, "h": 10, "fill": "#4C9DFF", "stroke": "#000000"}})).collect();
        match call(&e, "design.apply", json!({"design_id": id, "ops": ops})).error {
            Some(error) => break error,
            None => added += 500,
        }
    };
    assert_eq!(stop.code, "design.too_large");
    assert!(stop.message.contains("KiB"), "the byte cap, not the node cap, ends it: {}", stop.message);
    assert!(added >= 3000, "{added} nodes");
    counted(&e, id);
    assert!(bytes(&e, id) <= MAX_DESIGN_BYTES);
    let got = ok(&e, "design.get", json!({"design_id": id}));
    let line = serde_json::to_vec(&got).unwrap().len();
    assert!(line < 2 * 1024 * 1024 - 4096, "design.get is {line} bytes");
    // What design.get returns of the nodes is what `bytes` counts, give or take the page's
    // parent it does not have.
    let nodes: usize = got["nodes"].as_array().unwrap().iter().map(|n| serde_json::to_vec(n).unwrap().len() + 1).sum();
    assert!((nodes as i64 - bytes(&e, id)).abs() <= 64, "{nodes} written, {} counted", bytes(&e, id));
}

/// B60, MODEL-SPEC §1 #25: a design already over the cap (counted anew by v30, its ids now
/// included) is not frozen. A list is refused only for leaving it over the cap and larger than
/// it was; one that shrinks it, or keeps its size, applies.
#[test]
fn a_design_over_the_cap_can_still_shrink() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "t", "parent": "page-1", "kind": "text", "text": "a fairly long line of words"}},
        {"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}},
    ]));
    let over = MAX_DESIGN_BYTES + 500;
    e.store.lock().execute("UPDATE designs SET bytes = ?2 WHERE id = ?1", params![id, over]).unwrap();
    let grow = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "t", "set": {"text": "a fairly long line of words, longer"}}]}));
    assert_eq!(grow.error.map(|e| e.code).as_deref(), Some("design.too_large"));
    assert_eq!(bytes(&e, id), over, "refused: nothing written");
    apply(&e, id, json!([{"op": "update", "id": "r", "set": {"x": 4}}]));
    apply(&e, id, json!([{"op": "update", "id": "t", "set": {"text": "short"}}]));
    apply(&e, id, json!([{"op": "delete", "id": "r"}]));
    assert!(bytes(&e, id) < over);
}

/// MODEL-SPEC §4: each row a list writes carries that list's rev, a row it leaves alone keeps
/// its own, each id a delete takes is on record at that rev, and an id added back is no longer.
#[test]
fn rows_carry_the_rev_that_wrote_them_and_deletes_leave_tombstones() {
    let (e, _root) = project();
    let id = create(&e);
    assert_eq!((rev(&e, id), row_rev(&e, id, "page-1")), (1, Some(1)), "a new design's page is at rev 1");
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 200, "h": 100}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hi"}},
        {"op": "add", "node": {"id": "dot", "parent": "page-1", "kind": "ellipse", "w": 8, "h": 8}},
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "from": "dot", "to": "card"}},
        {"op": "add", "node": {"id": "side", "parent": "page-1", "kind": "frame", "w": 50, "h": 50}},
    ]));
    assert_eq!(rev(&e, id), 2);
    assert!(["card", "title", "dot", "link", "side"].iter().all(|n| row_rev(&e, id, n) == Some(2)));
    assert_eq!(row_rev(&e, id, "page-1"), Some(1), "untouched");
    apply(&e, id, json!([{"op": "update", "id": "title", "set": {"text": "Hello"}}]));
    assert_eq!((row_rev(&e, id, "title"), row_rev(&e, id, "card")), (Some(3), Some(2)));
    // An update that changes nothing stored writes nothing.
    apply(&e, id, json!([{"op": "update", "id": "title", "set": {"text": "Hello"}}]));
    assert_eq!(row_rev(&e, id, "title"), Some(3));
    apply(&e, id, json!([{"op": "move", "id": "title", "parent": "side"}]));
    assert_eq!(row_rev(&e, id, "title"), Some(5));
    // A delete records every id it takes; the connector it frees is written at the same rev.
    apply(&e, id, json!([{"op": "delete", "id": "side"}, {"op": "delete", "id": "dot"}]));
    assert_eq!((removed_at(&e, id, "side"), removed_at(&e, id, "title"), removed_at(&e, id, "dot")), (Some(6), Some(6), Some(6)));
    assert_eq!(row_rev(&e, id, "link"), Some(6), "its end let go");
    assert_eq!(removed_at(&e, id, "card"), None);
    // Added back: no longer removed, and written at the new rev.
    apply(&e, id, json!([{"op": "add", "node": {"id": "dot", "parent": "page-1", "kind": "ellipse", "w": 8, "h": 8}}]));
    assert_eq!((removed_at(&e, id, "dot"), row_rev(&e, id, "dot")), (None, Some(7)));
    assert_eq!(removed_at(&e, id, "side"), Some(6), "the others stay on record");
    // A duplicate starts its own history: rows at rev 1, nothing removed.
    let copy = ok(&e, "design.duplicate", json!({"design_id": id}))["id"].as_i64().unwrap();
    let revs: Vec<i64> = e.store.lock().prepare("SELECT DISTINCT rev FROM design_nodes WHERE design_id = ?1").unwrap()
        .query_map([copy], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!((revs, rev(&e, copy)), (vec![1], 1));
    let tombstones: i64 = e.store.lock().query_row("SELECT COUNT(*) FROM design_removed WHERE design_id = ?1", [copy], |r| r.get(0)).unwrap();
    assert_eq!(tombstones, 0);
    // Purged: nothing of it stays, its removals included.
    ok(&e, "design.delete", json!({"design_id": id}));
    ok(&e, "design.purge", json!({"design_id": id}));
    let left: i64 = e.store.lock().query_row("SELECT COUNT(*) FROM design_removed WHERE design_id = ?1", [id], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
}

/// TM-14: an agent's duplicate is tied to its own terminal, not to the original's (another
/// agent's, or none); the person's stays where the original is.
#[test]
fn a_duplicate_belongs_to_the_agent_that_made_it() {
    let (e, _root) = project();
    let session = |provider: &str| ok(&e, "session.create", json!({"project_id": 1, "provider": provider, "role": "builder"}))["name"]
        .as_str().unwrap().to_string();
    let (amber, silver) = (session("claude"), session("codex"));
    let theirs = ok_as(&e, Actor::agent(&amber), "design.create", json!({"title": "Amber's"}))["design"]["id"].as_i64().unwrap();
    let copy = ok_as(&e, Actor::agent(&silver), "design.duplicate", json!({"design_id": theirs}));
    assert_eq!(copy["session"], silver.as_str());
    let persons = ok(&e, "design.duplicate", json!({"design_id": theirs}));
    assert_eq!(persons["session"], amber.as_str());
    let shared = create(&e);
    let copy = ok_as(&e, Actor::agent(&silver), "design.duplicate", json!({"design_id": shared}));
    assert_eq!(copy["session"], silver.as_str());
    assert_eq!(ok(&e, "design.duplicate", json!({"design_id": shared}))["session"], Value::Null);
}

/// A store's file built by v29's migrations, with a project, a design whose `bytes` counted only
/// its bodies, and its rows.
fn v29_store(path: &std::path::Path) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    for m in &MIGRATIONS[..29] {
        c.execute_batch(m).unwrap();
    }
    c.pragma_update(None, "user_version", 29).unwrap();
    c.execute_batch(r#"
        INSERT INTO meta(key, value) VALUES ('created_at', 'then');
        INSERT INTO workspaces(id, name, path, created_at, updated_at) VALUES (1, 'w', '/w', 'then', 'then');
        INSERT INTO projects(id, workspace_id, name, path, created_at, updated_at) VALUES (1, 1, 'p', '/w/p', 'then', 'then');
        INSERT INTO designs(id, project_id, title, rev, pages, nodes, bytes, next_node, created_by, updated_by, created_at, updated_at)
            VALUES (5, 1, 'Old', 7, 1, 3, 0, 4, 'user', 'user', 'then', 'then');
        INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES
            (5, 'page-1', NULL, 'page', 1, '{"kind":"page","name":"Page 1"}'),
            (5, 'card-with-a-long-id', 'page-1', 'frame', 1, '{"kind":"frame","x":0.0,"y":0.0,"w":200.0,"h":100.0}'),
            (5, 'title', 'card-with-a-long-id', 'text', 1, '{"kind":"text","x":0.0,"y":0.0,"w":0.0,"h":0.0,"text":"Hé"}');
        UPDATE designs SET bytes = (SELECT SUM(length(CAST(body AS BLOB))) FROM design_nodes WHERE design_id = 5);
    "#).unwrap();
}

/// B60, W20: v30 recounts a v29 design's bytes with its ids and dates its rows at its rev.
#[test]
fn a_v29_store_migrates_counted_and_dated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    v29_store(&path);
    let old: i64 = Connection::open(&path).unwrap().query_row("SELECT bytes FROM designs WHERE id = 5", [], |r| r.get(0)).unwrap();
    let store = Store::open(&path, false).unwrap();
    assert_eq!(store.version().unwrap(), 30);
    let conn = store.lock();
    let (rows, revs): (i64, i64) = conn.query_row("SELECT COUNT(*), MIN(rev) FROM design_nodes WHERE design_id = 5 AND rev = 7", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((rows, revs), (3, 7), "every row kept, at its design's rev");
    let (bytes, styles): (i64, String) = conn.query_row("SELECT bytes, styles FROM designs WHERE id = 5", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    let ids = ("page-1".len() + 2 * "card-with-a-long-id".len() + "title".len() + "page-1".len()) as i64;
    assert_eq!(bytes, old + ids + 3 * NODE_OVERHEAD, "bodies, ids, parents and overhead");
    assert_eq!(styles, "{}");
    assert_eq!(recount_bytes(&conn, 5).unwrap(), bytes);
    let removed: i64 = conn.query_row("SELECT COUNT(*) FROM design_removed", [], |r| r.get(0)).unwrap();
    assert_eq!(removed, 0);
}

/// MODEL-SPEC §4: a project restored from a backup a v29 Relay took (no styles, no rev on its
/// rows, no removals, bytes counted by bodies alone) comes back counted and dated as v30 keeps it.
#[test]
fn a_v29_backup_restores_counted_and_dated() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": long("card"), "parent": "page-1", "kind": "frame", "w": 200, "h": 100}},
        {"op": "add", "node": {"id": "title", "parent": long("card"), "kind": "text", "text": "Hi"}},
        {"op": "add", "node": {"id": "gone", "parent": "page-1", "kind": "rect", "w": 4, "h": 4}},
    ]));
    apply(&e, id, json!([{"op": "delete", "id": "gone"}]));
    let at = rev(&e, id);
    ok(&e, "project.remove", json!({"project_id": 1}));
    let listed = ok(&e, "project.removed.list", json!({}));
    let backup = listed["removed"][0]["backup_path"].as_str().unwrap().to_string();
    // Taken by v29: what v30 added is not in it.
    Connection::open(&backup).unwrap().execute_batch("
        DROP INDEX design_nodes_rev;
        ALTER TABLE design_nodes DROP COLUMN rev;
        ALTER TABLE designs DROP COLUMN styles;
        DROP TABLE design_removed;
        UPDATE designs SET bytes = (SELECT SUM(length(CAST(body AS BLOB))) FROM design_nodes WHERE design_id = designs.id);
        PRAGMA user_version = 29;
    ").unwrap();
    ok(&e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    counted(&e, id);
    let undated: i64 = e.store.lock().query_row("SELECT COUNT(*) FROM design_nodes WHERE design_id = ?1 AND rev != ?2", params![id, at], |r| r.get(0)).unwrap();
    assert_eq!(undated, 0, "every row at its design's rev");
    assert_eq!(ok(&e, "design.get", json!({"design_id": id}))["nodes"].as_array().unwrap().len(), 3);
    // And a v30 backup brings its rows' revs and its removals back as they were.
    let title_rev = row_rev(&e, id, "title");
    apply(&e, id, json!([{"op": "delete", "id": "title"}]));
    ok(&e, "project.remove", json!({"project_id": 1}));
    let listed = ok(&e, "project.removed.list", json!({}));
    let backup = listed["removed"][0]["backup_path"].as_str().unwrap().to_string();
    ok(&e, "project.restore", json!({"backup_path": backup, "project_id": 1}));
    counted(&e, id);
    assert_eq!(removed_at(&e, id, "title"), Some(at + 1));
    assert!(title_rev.is_some() && row_rev(&e, id, "title").is_none());
}

/// feature-motion §10: motion and a timeline go through design.apply like any field: stored as
/// written, refused by the key and field at fault, and undone whole.
#[test]
fn motion_and_timeline_are_stored_checked_and_undone() {
    let (e, _root) = project();
    let id = create(&e);
    let timeline = json!({"duration_ms": 3000.0, "loop": false});
    let motion = json!({"ease": "ease_out", "keys": [{"at_ms": 0.0, "dy": 24.0, "opacity": 0.0}, {"at_ms": 500.0, "dy": 0.0, "opacity": 1.0}]});
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "hero", "parent": "page-1", "kind": "frame", "w": 390, "h": 844, "timeline": timeline}},
        {"op": "add", "node": {"id": "r", "parent": "hero", "kind": "rect", "w": 40, "h": 40, "motion": motion}},
    ]));
    let got = ok(&e, "design.get", json!({"design_id": id}));
    let find = |id: &str| got["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap().clone();
    assert_eq!((find("hero")["timeline"].clone(), find("r")["motion"].clone()), (timeline, motion.clone()));
    let pop = json!({"ease": "overshoot", "loop": true, "keys": [{"at_ms": 300.0, "scale": 0.6}, {"at_ms": 700.0, "scale": 1.0}]});
    let out = apply(&e, id, json!([{"op": "update", "id": "r", "set": {"motion": pop}}]));
    assert_eq!(out["nodes"][0]["motion"], pop);
    ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true}));
    let back = ok(&e, "design.get", json!({"design_id": id}))["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "r").unwrap().clone();
    assert_eq!(back["motion"], motion, "the undo puts the motion back whole");
    counted(&e, id);
    let at = rev(&e, id);
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "r", "set": {"motion": {"keys": [{"at_ms": 0, "opacity": 0}, {"at_ms": 0.5, "opacity": 1}]}}}]}));
    let error = refused.error.unwrap();
    assert_eq!(error.code, "design.node_invalid");
    assert_eq!(error.message, "ops[0] update \"r\": motion.keys: the last key is at 0.5 ms; times are milliseconds (half a second is 500)");
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "r", "set": {"timeline": {"duration_ms": 1000}}}]}));
    assert_eq!(refused.error.unwrap().message, "ops[0] update \"r\": timeline: only a frame or a page keeps time");
    assert_eq!(rev(&e, id), at, "nothing applied");
}
