//! What a `design.apply` list may leave behind (MODEL-SPEC §2.8, §2.11; feature-agent-api W14, W19,
//! B6, B59): a design nests at most 64 levels; a connector's ends name nodes on its own page and an
//! image's `src` a stored picture, checked once the whole list has run; a move that carries a
//! node to another page lets go of the ends it parts; a missing id is answered with the nearest
//! one the design holds; and a list that takes a design over its size is refused at the edit that
//! did, before the rest of it is written.

mod common;

use common::{call, committed_repo, engine_with_project, ok, ok_as, refused};
use relay_bus::design::{MAX_DEPTH, MAX_DESIGN_BYTES, MAX_TEXT};
use relay_bus::{Actor, BusError, ErrorKind};
use relay_core::engine::Engine;
use relay_core::handlers::design::recount_bytes;
use rusqlite::params;
use serde_json::{json, Value};
use std::sync::Arc;

/// A 1×1 PNG.
const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

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

fn lenient(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops, "lenient": true}))
}

fn refusal(e: &Engine, design: i64, ops: Value) -> BusError {
    refused(call(e, "design.apply", json!({"design_id": design, "ops": ops})))
}

fn nodes(e: &Engine, design: i64) -> Value {
    ok(e, "design.get", json!({"design_id": design}))["nodes"].clone()
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

fn find(nodes: &Value, id: &str) -> Value {
    nodes.as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or_else(|| panic!("no {id} in {nodes}"))
}

fn rev(e: &Engine, design: i64) -> i64 {
    ok(e, "design.get", json!({"design_id": design}))["design"]["rev"].as_i64().unwrap()
}

/// `designs.bytes` is what the design's rows count to: every write kept it.
#[track_caller]
fn counted(e: &Engine, design: i64) {
    let kept: i64 = e.store.lock().query_row("SELECT bytes FROM designs WHERE id = ?1", [design], |r| r.get(0)).unwrap();
    assert_eq!(kept, recount_bytes(&e.store.lock(), design).unwrap(), "design {design}'s bytes drifted from its rows");
}

/// An agent of project 1.
fn agent(e: &Engine) -> Actor {
    let name = ok(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    Actor::agent(&name)
}

/// Frames `f1` inside page-1, `f2` inside `f1`, … to `f{n}`, which sits `n` deep.
fn nest(e: &Engine, design: i64, n: usize) {
    let ops: Vec<Value> = (1..=n).map(|i| {
        let parent = if i == 1 { "page-1".to_string() } else { format!("f{}", i - 1) };
        json!({"op": "add", "node": {"id": format!("f{i}"), "parent": parent, "kind": "frame", "w": 100, "h": 100}})
    }).collect();
    apply(e, design, Value::Array(ops));
}

/// W14: a node sits at most 64 deep (a page's child is 1 deep). An add or a move past it is
/// refused with the chain it would hang from; an undo skips it, and what was to go inside it.
#[test]
fn nesting_stops_at_64_and_names_the_chain() {
    let (e, _root) = project();
    let id = create(&e);
    nest(&e, id, MAX_DEPTH);
    let at = rev(&e, id);
    let error = refusal(&e, id, json!([{"op": "add", "node": {"id": "f65", "parent": "f64", "kind": "rect"}}]));
    assert_eq!((error.code.as_str(), error.kind), ("design.too_deep", ErrorKind::Invalid));
    assert_eq!(error.message, "ops[0] add \"f65\": it would sit 65 deep (page-1 › f1 › … › f63 › f64); a design nests at most 64 levels");
    assert_eq!(error.hint.as_deref(), Some("nest less: lay rows out in one frame rather than a frame per row"));
    assert_eq!(error.details.as_ref().unwrap()["op"], 0);
    // Within one list too: the 65th level of a chain the list builds.
    let chain: Vec<Value> = (1..=MAX_DEPTH + 1).map(|i| {
        let parent = if i == 1 { "page-1".to_string() } else { format!("g{}", i - 1) };
        json!({"op": "add", "node": {"id": format!("g{i}"), "parent": parent, "kind": "group"}})
    }).collect();
    let error = refusal(&e, id, Value::Array(chain));
    assert!(error.message.starts_with("ops[64] add \"g65\": it would sit 65 deep (page-1 › g1 › … › g63 › g64)"), "{}", error.message);
    assert_eq!(rev(&e, id), at, "nothing applied");

    // A move counts what the node holds: `box` and the dot in it fit one level less deep.
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "box", "parent": "page-1", "kind": "group"}},
        {"op": "add", "node": {"id": "dot", "parent": "box", "kind": "ellipse", "w": 4, "h": 4}},
    ]));
    let error = refusal(&e, id, json!([{"op": "move", "id": "box", "parent": "f63"}]));
    assert_eq!(error.code, "design.too_deep");
    assert_eq!(error.message, "ops[0] move \"box\": what it holds would sit 65 deep (page-1 › f1 › … › f62 › f63 › box › …); a design nests at most 64 levels");
    assert_eq!(refusal(&e, id, json!([{"op": "move", "id": "dot", "parent": "f64"}])).message,
        "ops[0] move \"dot\": it would sit 65 deep (page-1 › f1 › … › f63 › f64); a design nests at most 64 levels");
    apply(&e, id, json!([{"op": "move", "id": "box", "parent": "f62"}]));
    assert_eq!(find(&nodes(&e, id), "box")["parent"], "f62");
    // Reordered where it is, it is no deeper: nothing to check.
    apply(&e, id, json!([{"op": "move", "id": "box", "index": 0}]));

    // An undo (a lenient list) skips an add it cannot nest, and what was to go inside it, and
    // says why.
    let at = rev(&e, id);
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "lenient": true, "ops": [
        {"op": "add", "node": {"id": "deep", "parent": "f64", "kind": "frame"}},
        {"op": "add", "node": {"id": "deeper", "parent": "deep", "kind": "rect"}},
        {"op": "add", "node": {"id": "fine", "parent": "f63", "kind": "rect"}},
    ]}));
    assert_eq!(out["ids"], json!(["", "", "fine"]));
    assert_eq!(out["skipped"], json!([{"at": 0, "id": "deep", "reason": "too_deep"}, {"at": 1, "id": "deeper", "reason": "parent_skipped"}]));
    assert!(out.get("warnings").is_none(), "{out}");
    assert_eq!(rev(&e, id), at + 1);
    // Moved up, it and what it holds are no deeper: f64 now takes a child.
    apply(&e, id, json!([{"op": "move", "id": "f63", "parent": "page-1"}, {"op": "add", "node": {"id": "child", "parent": "f64", "kind": "rect"}}]));
}

/// W14 on a design nested deeper before the limit (rows another build wrote): it still reads and
/// can be edited and flattened, but nothing is put deeper in it, however deep it already sits.
#[test]
fn a_legacy_chain_past_the_limit_can_be_flattened_but_not_deepened() {
    let (e, _root) = project();
    let id = create(&e);
    // A chain l1 … l90, longer than a walk up the design looks (MAX_DEPTH + 6), and a leaf
    // hanging from l72, 73 deep.
    let long = MAX_DEPTH + 26;
    {
        let store = e.store.lock();
        let chain = (1..=long).map(|i| (format!("l{i}"), if i == 1 { "page-1".to_string() } else { format!("l{}", i - 1) }, "frame"));
        for (node, parent, kind) in chain.chain([("leaf".to_string(), "l72".to_string(), "rect")]) {
            store.execute("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, ?3, ?4, 1.0, ?5)",
                params![id, node, parent, kind, format!(r#"{{"kind":"{kind}","x":0.0,"y":0.0,"w":10.0,"h":10.0}}"#)]).unwrap();
        }
        store.execute("UPDATE designs SET nodes = nodes + ?2 WHERE id = ?1", params![id, (long + 1) as i64]).unwrap();
        recount_bytes(&store, id).unwrap();
    }
    assert_eq!(nodes(&e, id).as_array().unwrap().len(), long + 2);
    let deepest = format!("l{long}");
    apply(&e, id, json!([{"op": "update", "id": deepest, "set": {"fill": "#ffffff"}}]));
    assert_eq!(refusal(&e, id, json!([{"op": "add", "node": {"parent": deepest, "kind": "rect"}}])).code, "design.too_deep");
    assert_eq!(refusal(&e, id, json!([{"op": "add", "node": {"parent": "l64", "kind": "rect"}}])).code, "design.too_deep");
    // Both past where a walk looks, the leaf's place and the one it would take are counted to
    // the page: from 73 deep to 91 is deeper, refused; to 72 is not.
    let error = refusal(&e, id, json!([{"op": "move", "id": "leaf", "parent": deepest}]));
    assert_eq!(error.code, "design.too_deep");
    assert!(error.message.starts_with("ops[0] move \"leaf\": it would sit more than 70 deep"), "{}", error.message);
    apply(&e, id, json!([{"op": "move", "id": "leaf", "parent": "l71"}]));
    assert_eq!(find(&nodes(&e, id), "leaf")["parent"], "l71");
    assert_eq!(refusal(&e, id, json!([{"op": "move", "id": "leaf", "parent": "l72"}])).code, "design.too_deep", "73 is deeper than 72");
    // Moved within the chain, but no deeper than it was: the 66th level up to the 65th.
    apply(&e, id, json!([{"op": "move", "id": "l66", "parent": "l64"}]));
    // Moved up out of it: it and what it holds now fit.
    apply(&e, id, json!([{"op": "move", "id": "l60", "parent": "page-1"}]));
    let got = nodes(&e, id);
    assert_eq!(find(&got, "l60")["parent"], "page-1");
    apply(&e, id, json!([{"op": "add", "node": {"id": "ok", "parent": deepest, "kind": "rect"}}]));
    counted(&e, id);
}

/// A page with a card (holding its title), a rect beside it and a line between them, and a
/// second page with a rect of its own.
fn connected(e: &Engine) -> i64 {
    let id = create(e);
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "x": 100, "y": 100, "w": 200, "h": 100}},
        {"op": "add", "node": {"id": "card-title", "parent": "card", "kind": "text", "text": "Hi", "x": 0, "y": 0, "w": 20, "h": 20}},
        {"op": "add", "node": {"id": "b", "parent": "page-1", "kind": "rect", "x": 400, "y": 100, "w": 20, "h": 40}},
        {"op": "add", "node": {"id": "joined", "parent": "page-1", "kind": "line", "x": 100, "y": 100, "from": "card-title", "to": "b"}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Page 2"}},
        {"op": "add", "node": {"id": "far", "parent": "page-2", "kind": "rect", "w": 10, "h": 10}},
    ]));
    id
}

/// B6, W19: a connector's end names a node of its own page that is not the line itself or a
/// page. A missing one is `design.node_not_found` with the nearest id; the rest is
/// `design.connector`; `details` name the edit, the field and the end.
#[test]
fn a_connector_end_is_a_node_on_its_page() {
    let (e, _root) = project();
    let id = connected(&e);
    let at = rev(&e, id);
    let line = |set: Value| {
        let mut node = json!({"id": "link", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]]});
        node.as_object_mut().unwrap().extend(set.as_object().unwrap().clone());
        json!([{"op": "add", "node": node}])
    };
    let error = refusal(&e, id, line(json!({"from": "card-titel", "to": "b"})));
    assert_eq!((error.code.as_str(), error.kind), ("design.node_not_found", ErrorKind::NotFound));
    assert_eq!(error.message, "ops[0] add \"link\": from: \"card-titel\" is not in the design");
    assert_eq!(error.hint.as_deref(), Some("did you mean \"card-title\"?"));
    assert_eq!(error.details.as_ref().unwrap(), &json!({"op": 0, "index": 0, "id": "link", "field": "from", "end": "card-titel"}));
    let error = refusal(&e, id, line(json!({"to": "nothing-like-it"})));
    assert_eq!(error.hint.as_deref(), Some("design.get lists the ids (pass page to read one page)"));
    for (set, message) in [
        (json!({"from": "link"}), "ops[0] add \"link\": from: \"link\" is this line itself"),
        (json!({"to": "page-1"}), "ops[0] add \"link\": to: \"page-1\" is a page, not a node on one"),
        (json!({"to": "far"}), "ops[0] add \"link\": to: \"far\" is on page \"page-2\", not on this line's page \"page-1\""),
    ] {
        let error = refusal(&e, id, line(set.clone()));
        assert_eq!((error.code.as_str(), error.message.as_str()), ("design.connector", message), "{set}");
        assert_eq!(error.hint.as_deref(), Some("attach it to a node on the same page"));
        assert_eq!(error.details.as_ref().unwrap()["field"], set.as_object().unwrap().keys().next().unwrap().as_str());
    }
    // An update is held to it too, and named.
    let error = refusal(&e, id, json!([{"op": "update", "id": "b", "set": {"x": 1}}, {"op": "update", "id": "joined", "set": {"to": "far"}}]));
    assert!(error.message.starts_with("ops[1] update \"joined\": to: \"far\" is on page"), "{}", error.message);
    assert_eq!(rev(&e, id), at, "nothing applied");

    // Checked once the list is done: an end added later in the list is there.
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "ahead", "parent": "page-1", "kind": "line", "from": "x", "to": "y"}},
        {"op": "add", "node": {"id": "x", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}},
        {"op": "add", "node": {"id": "y", "parent": "card", "kind": "rect", "x": 50, "w": 10, "h": 10}},
    ]));
    assert_eq!(out["ids"], json!(["ahead", "x", "y"]));
    // An end deleted later in the list lets go, as any connector's does; one deleted before the
    // edit that names it is a mistake, refused.
    let out = apply(&e, id, json!([{"op": "update", "id": "ahead", "set": {"to": "b"}}, {"op": "delete", "id": "b"}]));
    let ahead = find(&out["nodes"], "ahead");
    assert!(ahead.get("to").is_none() && ahead["from"] == "x" && ahead["points"].is_array(), "{ahead}");
    let error = refusal(&e, id, json!([{"op": "delete", "id": "x"}, {"op": "update", "id": "joined", "set": {"from": "x"}}]));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.node_not_found", "ops[1] update \"joined\": from: \"x\" is not in the design"));
    counted(&e, id);
}

/// B6: a lenient list (an undo, or an agent's) puts a line back even when an end it names is gone
/// or misplaced, letting go of that end as a delete does, and tells an agent; its undo puts back
/// what was there.
#[test]
fn a_lenient_list_lets_go_of_a_missing_end_with_a_warning() {
    let (e, _root) = project();
    let id = connected(&e);
    let agent = agent(&e);
    let before = nodes(&e, id);
    let out = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "lenient": true, "reply": "full", "ops": [
        {"op": "add", "node": {"id": "link", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "gone", "to": "b"}},
        {"op": "update", "id": "joined", "set": {"to": "far"}},
    ]}));
    assert_eq!(out["warnings"], json!([
        "ops[0] add \"link\": from: let go of \"gone\", which is not in the design",
        "ops[1] update \"joined\": to: let go of \"far\", which is on page \"page-2\", not on this line's page \"page-1\"",
    ]));
    let got = nodes(&e, id);
    let link = find(&got, "link");
    assert!(link.get("from").is_none() && link["to"] == "b" && link["points"] == json!([[0.0, 0.0], [5.0, 5.0]]), "{link}");
    // A line with no points of its own is given its ends' stored centres, as a delete gives
    // them: the title's (110, 110) and far's (5, 5), counted from its own corner (100, 100).
    let joined = find(&got, "joined");
    assert!(joined.get("to").is_none() && joined["from"] == "card-title", "{joined}");
    assert_eq!(joined["points"], json!([[10.0, 10.0], [-95.0, -95.0]]));
    counted(&e, id);
    // The undo puts back exactly what was there.
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before);
    counted(&e, id);
    // The person's undo is told nothing.
    let out = lenient(&e, id, json!([{"op": "update", "id": "joined", "set": {"from": "gone"}}]));
    assert!(out.get("warnings").is_none(), "{out}");
    assert!(find(&nodes(&e, id), "joined").get("from").is_none());
}

/// B6: an image's `src` is a stored picture (`design.asset.add`'s answer); an undo puts back
/// whatever it named.
#[test]
fn an_image_names_a_stored_picture() {
    let (e, _root) = project();
    let id = create(&e);
    let src = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": PNG_1X1}))["src"].as_str().unwrap().to_string();
    let made_up = "0123456789abcdef0123456789abcdef.png";
    let image = |src: &str| json!([{"op": "add", "node": {"id": "pic", "parent": "page-1", "kind": "image", "src": src, "w": 10, "h": 10}}]);
    let error = refusal(&e, id, image(made_up));
    assert_eq!((error.code.as_str(), error.kind), ("design.asset_not_found", ErrorKind::NotFound));
    assert_eq!(error.message, format!("ops[0] add \"pic\": src: {made_up:?} is not a stored picture"));
    assert_eq!(error.hint.as_deref(), Some("store it with design.asset.add and use the src it returns"));
    assert_eq!(error.details.as_ref().unwrap()["field"], "src");
    apply(&e, id, image(&src));
    let error = refusal(&e, id, json!([{"op": "update", "id": "pic", "set": {"src": made_up}}]));
    assert_eq!(error.code, "design.asset_not_found");
    // Lenient: kept as written, and an agent is told.
    let out = ok_as(&e, agent(&e), "design.apply", json!({"design_id": id, "lenient": true, "ops": [{"op": "update", "id": "pic", "set": {"src": made_up}}]}));
    assert_eq!(out["warnings"], json!([format!("ops[0] update \"pic\": src: {made_up:?} is not a stored picture; kept as it was")]));
    assert_eq!(find(&nodes(&e, id), "pic")["src"], made_up);
    // An edit that leaves src alone is not held to it.
    apply(&e, id, json!([{"op": "update", "id": "pic", "set": {"x": 4}}]));
}

/// W19: an edit naming a node or parent the design does not hold is answered with the nearest id
/// it does hold, or where to find them.
#[test]
fn a_missing_parent_or_node_suggests_the_nearest_id() {
    let (e, _root) = project();
    let id = connected(&e);
    for (op, code) in [
        (json!({"op": "update", "id": "card-titel", "set": {"x": 1}}), "design.node_not_found"),
        (json!({"op": "move", "id": "Card-Title", "index": 0}), "design.node_not_found"),
        (json!({"op": "delete", "id": "cardtitle"}), "design.node_not_found"),
    ] {
        let error = refusal(&e, id, json!([op]));
        assert_eq!((error.code.as_str(), error.hint.as_deref()), (code, Some("did you mean \"card-title\"?")), "{op}");
    }
    let error = refusal(&e, id, json!([{"op": "add", "node": {"parent": "cards", "kind": "rect"}}]));
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.parent", "ops[0] add rect: parent \"cards\" is not in the design"));
    assert_eq!(error.hint.as_deref(), Some("did you mean \"card\"?"));
    assert_eq!(refusal(&e, id, json!([{"op": "move", "id": "b", "parent": "cards"}])).hint.as_deref(), Some("did you mean \"card\"?"));
    let error = refusal(&e, id, json!([{"op": "update", "id": "zzzzzzzz", "set": {"x": 1}}]));
    assert_eq!(error.hint.as_deref(), Some("design.get lists the ids (pass page to read one page)"));
}

/// B6, MODEL-SPEC §2.11: a move that leaves a connector and an end on different pages lets go of
/// that end where the stored boxes draw it, as a delete does, and its undo attaches it again. A
/// connector the list leaves alone keeps its ends, wherever they are.
#[test]
fn a_move_to_another_page_lets_go_of_the_ends_it_parts() {
    let (e, _root) = project();
    let id = connected(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "drawn", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "b", "to": "card-title"}},
        {"op": "add", "node": {"id": "near", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}},
        {"op": "add", "node": {"id": "near-link", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "b", "to": "near"}},
        {"op": "add", "node": {"id": "old", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "b"}},
    ]));
    // A connector a build before B6 left across two pages.
    e.store.lock().execute("UPDATE design_nodes SET body = replace(body, '\"from\":\"b\"', '\"from\":\"b\",\"to\":\"far\"') WHERE design_id = ?1 AND node_id = 'old'",
        [id]).unwrap();
    recount_bytes(&e.store.lock(), id).unwrap();
    let before = nodes(&e, id);
    // Moving within the page parts nothing.
    let out = apply(&e, id, json!([{"op": "move", "id": "card-title", "parent": "page-1"}, {"op": "move", "id": "card-title", "parent": "card"}]));
    assert_eq!(ids(&out["nodes"]), ["card-title"]);

    let out = apply(&e, id, json!([{"op": "move", "id": "card", "parent": "page-2"}]));
    let got = nodes(&e, id);
    let joined = find(&got, "joined");
    // From the title's stored centre (110, 110) to b's (410, 120), from the line's corner (100, 100).
    assert!(joined.get("from").is_none() && joined["to"] == "b", "{joined}");
    assert_eq!(joined["points"], json!([[10.0, 10.0], [310.0, 20.0]]));
    let drawn = find(&got, "drawn");
    assert!(drawn["from"] == "b" && drawn.get("to").is_none() && drawn["points"] == json!([[0.0, 0.0], [5.0, 5.0]]), "{drawn}");
    assert_eq!(find(&got, "near-link"), find(&before, "near-link"), "neither end moved");
    assert_eq!(find(&got, "old"), find(&before, "old"), "the list did not part it");
    let mut changed = ids(&out["nodes"]);
    changed.sort();
    assert_eq!(changed, ["card", "drawn", "joined"]);
    counted(&e, id);
    let after = got;
    let undone = lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before, "the undo attaches them again");
    counted(&e, id);
    lenient(&e, id, undone["inverse"].clone());
    assert_eq!(nodes(&e, id), after, "and the redo lets go again");
    lenient(&e, id, out["inverse"].clone());

    // The line moved away from both its ends lets go of both.
    let out = apply(&e, id, json!([{"op": "move", "id": "near-link", "parent": "page-2"}]));
    let link = find(&out["nodes"], "near-link");
    assert!(link.get("from").is_none() && link.get("to").is_none(), "{link}");
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before);

    // Written before the move that parts it, an end lets go; written after, it is refused.
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "late", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "to": "near"}},
        {"op": "move", "id": "near", "parent": "page-2"},
    ]));
    assert!(find(&out["nodes"], "late").get("to").is_none());
    lenient(&e, id, out["inverse"].clone());
    let error = refusal(&e, id, json!([
        {"op": "move", "id": "near", "parent": "page-2"},
        {"op": "add", "node": {"id": "early", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "to": "near"}},
    ]));
    assert_eq!((error.code.as_str(), error.details.as_ref().unwrap()["op"].as_u64()), ("design.connector", Some(1)));
    assert_eq!(nodes(&e, id), before);
    counted(&e, id);
}

/// B6: a node carried to another page inside what a move took there stays carried when a later
/// edit of the list takes it out: the ends it parts, at either end of a line, let go all the same.
#[test]
fn a_node_taken_out_of_a_carried_subtree_still_lets_go() {
    let (e, _root) = project();
    let id = connected(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "inner", "parent": "card", "kind": "line", "points": [[0, 0], [5, 5]], "to": "b"}}]));
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([
        {"op": "move", "id": "card", "parent": "page-2"},
        {"op": "move", "id": "card-title", "parent": "page-2"},
        {"op": "move", "id": "inner", "parent": "page-2"},
    ]));
    let got = nodes(&e, id);
    // From the title's stored centre, now (10, 10) on page-2, to b's (410, 120), from (100, 100).
    let joined = find(&got, "joined");
    assert!(joined.get("from").is_none() && joined["to"] == "b", "{joined}");
    assert_eq!(joined["points"], json!([[-90.0, -90.0], [310.0, 20.0]]));
    let inner = find(&got, "inner");
    assert!(inner.get("to").is_none() && inner["points"] == json!([[0.0, 0.0], [5.0, 5.0]]), "{inner}");
    counted(&e, id);
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before, "the undo attaches them again");
    counted(&e, id);

    // Taken back to the line's page, the title parts nothing.
    let out = apply(&e, id, json!([{"op": "move", "id": "card", "parent": "page-2"}, {"op": "move", "id": "card-title", "parent": "page-1"}]));
    assert_eq!(find(&nodes(&e, id), "joined"), find(&before, "joined"));
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before);
    counted(&e, id);
}

/// B59: past the last edit that could shrink it, a list that has taken the design over its size
/// only grows, so it is refused at the edit that took it over, before the rest is written; a list
/// that deletes enough later on applies.
#[test]
fn the_byte_cap_refuses_a_list_at_the_edit_that_takes_it_over() {
    let (e, _root) = project();
    let words = "x".repeat(MAX_TEXT);
    let adds = |n: usize| -> Vec<Value> {
        (0..n).map(|i| json!({"op": "add", "node": {"id": format!("t{i}"), "parent": "page-1", "kind": "text", "text": words}})).collect()
    };
    let id = create(&e);
    let before = ok(&e, "design.get", json!({"design_id": id}));
    let error = refusal(&e, id, json!(adds(200)));
    assert_eq!(error.code, "design.too_large");
    let over = error.details.as_ref().expect("the edit is named")["op"].as_u64().unwrap() as usize;
    assert!((140..170).contains(&over), "{over}");
    assert!(error.message.starts_with(&format!("ops[{over}] add \"t{over}\": the design would hold at least ")), "{}", error.message);
    assert!(error.message.ends_with(&format!("the limit is {} KiB", MAX_DESIGN_BYTES >> 10)), "{}", error.message);
    assert_eq!(ok(&e, "design.get", json!({"design_id": id})), before, "nothing written");
    // Exactly that edit: the list without it fits.
    apply(&e, id, json!(adds(over)));
    counted(&e, id);

    // An edit that may shrink the design comes later: the list is not refused before it, and
    // applies when it ends small enough.
    let id = create(&e);
    let mut ops = adds(200);
    ops.extend((0..100).map(|i| json!({"op": "delete", "id": format!("t{i}")})));
    let out = apply(&e, id, json!(ops));
    assert_eq!(out["design"]["nodes"], 101);
    counted(&e, id);
    let id = create(&e);
    let mut ops = adds(200);
    ops.push(json!({"op": "update", "id": "t0", "set": {"text": "short"}}));
    let error = refusal(&e, id, json!(ops));
    assert_eq!((error.code.as_str(), error.details.as_ref().unwrap()["op"].as_u64()), ("design.too_large", Some(200)));
    // After a delete, the connectors it frees are rewritten once the edits are done: only the
    // final count can say.
    let mut ops = vec![json!({"op": "add", "node": {"id": "z", "parent": "page-1", "kind": "rect"}}), json!({"op": "delete", "id": "z"})];
    ops.extend(adds(200));
    let error = refusal(&e, id, json!(ops));
    assert_eq!(error.code, "design.too_large");
    assert!(error.details.is_none() && error.message.starts_with("the design would hold "), "refused once every edit ran: {}", error.message);
}

/// The canvas's undo of an agent's work across pages: every check above runs on the list as a
/// whole, so an undo that attaches a line before it re-adds the node it attaches to applies.
#[test]
fn an_undo_attaches_before_it_re_adds() {
    let (e, _root) = project();
    let id = connected(&e);
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([{"op": "delete", "id": "card"}, {"op": "move", "id": "b", "parent": "page-2"}]));
    let inverse = out["inverse"].clone();
    assert_eq!(inverse[0]["op"], "update", "the line is attached first: {inverse}");
    lenient(&e, id, inverse);
    assert_eq!(nodes(&e, id), before);
    counted(&e, id);
}

/// Timings for `design.apply` on a design of 4,500 nodes holding 1,500 connectors: an update, a
/// move into another parent on the same page, a delete (which reads every connector's ends) and a
/// move to another page (which places them). Each list's undo runs after it, untimed, so every run
/// starts from the same design. Run with
/// `cargo test --release -p relay-core --test design_integrity -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_apply_beside_1500_connectors() {
    let (e, _root) = project();
    let id = create(&e);
    let n = 1_500;
    // 1,500 cards on page-1, each holding a title, and a line from each title to the next card's.
    apply(&e, id, json!([{"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Page 2"}},
        {"op": "add", "node": {"id": "spare", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}}]));
    for batch in (0..n).collect::<Vec<_>>().chunks(500) {
        let ops: Vec<Value> = batch.iter().flat_map(|i| [
            json!({"op": "add", "node": {"id": format!("c{i}"), "parent": "page-1", "kind": "frame", "x": i * 30, "y": 0, "w": 20, "h": 20}}),
            json!({"op": "add", "node": {"id": format!("t{i}"), "parent": format!("c{i}"), "kind": "text", "text": "Card", "w": 20, "h": 10}}),
        ]).collect();
        apply(&e, id, Value::Array(ops));
    }
    let lines: Vec<Value> = (0..n).map(|i| json!({"op": "add", "node": {
        "id": format!("l{i}"), "parent": "page-1", "kind": "line", "from": format!("t{i}"), "to": format!("t{}", (i + 1) % n)}})).collect();
    apply(&e, id, Value::Array(lines));
    assert_eq!(ok(&e, "design.get", json!({"design_id": id}))["design"]["nodes"], 3 * n + 3);
    let median = |mut runs: Vec<std::time::Duration>| { runs.sort(); runs[runs.len() / 2] };
    let time = |what: &str, ops: Value| {
        let mut runs = Vec::new();
        for k in 0..32 {
            let started = std::time::Instant::now();
            let out = apply(&e, id, ops.clone());
            if k >= 2 { runs.push(started.elapsed()); }
            lenient(&e, id, out["inverse"].clone());
        }
        println!("{what}: median {:?} of {}", median(runs.clone()), runs.len());
    };
    time("update", json!([{"op": "update", "id": "c0", "set": {"x": 5}}]));
    time("move on the page", json!([{"op": "move", "id": "c0", "parent": "c1"}]));
    time("delete", json!([{"op": "delete", "id": "spare"}]));
    time("move to another page", json!([{"op": "move", "id": "c0", "parent": "page-2"}]));
    counted(&e, id);
}
