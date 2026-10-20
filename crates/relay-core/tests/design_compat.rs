//! Design bodies across builds, and the rules on what a caller writes (MODEL-SPEC §2.3, §2.8;
//! B8, B43, B61, B71, B72): a body a newer Relay wrote is read and kept through every edit, a
//! row this Relay cannot read is left out of reads and never edited, a strict list is held to
//! the fields each kind uses, and an undo (a lenient list) puts back exactly what was stored,
//! telling an agent what it let through.

mod common;

use common::{call, call_as, code, committed_repo, engine_with_project, err, ok, ok_as};
use relay_bus::Actor;
use relay_core::engine::Engine;
use rusqlite::params;
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

fn lenient(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops, "lenient": true}))
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

/// A row's body as stored, parsed.
fn body(e: &Engine, design: i64, id: &str) -> Value {
    let text: String = e.store.lock()
        .query_row("SELECT body FROM design_nodes WHERE design_id = ?1 AND node_id = ?2", params![design, id], |r| r.get(0))
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Write a row as another build would have: any kind, any body.
fn put(e: &Engine, design: i64, id: &str, parent: &str, kind: &str, body: &str) {
    let store = e.store.lock();
    store.execute("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, ?3, ?4, 100.0, ?5)",
        params![design, id, parent, kind, body]).unwrap();
    store.execute("UPDATE designs SET nodes = nodes + 1 WHERE id = ?1", [design]).unwrap();
}

/// Give a stored row another body, as an earlier or a newer build left it.
fn rewrite(e: &Engine, design: i64, id: &str, body: Value) {
    e.store.lock().execute("UPDATE design_nodes SET body = ?3 WHERE design_id = ?1 AND node_id = ?2",
        params![design, id, body.to_string()]).unwrap();
}

fn spiral() -> Value {
    json!({"kind": "spiral", "stops": [{"at": 0, "color": "#fff"}, {"at": 1, "color": "#000"}]})
}

/// W65, B61: a field this build does not know, and one whose value it cannot read, are read
/// past, shown, and written back as they were by an update, a delete and its undo.
#[test]
fn a_body_from_a_newer_build_is_read_and_kept() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}}]));
    let mut newer = body(&e, id, "r");
    newer["future"] = json!({"depth": 2});
    newer["gradient"] = spiral();
    rewrite(&e, id, "r", newer.clone());

    let got = find(&nodes(&e, id), "r");
    assert_eq!((got["future"].clone(), got["gradient"].clone(), got["w"].clone()), (json!({"depth": 2}), spiral(), json!(10.0)));
    // An update of another field keeps both.
    let out = apply(&e, id, json!([{"op": "update", "id": "r", "set": {"x": 7}}]));
    assert_eq!(out["nodes"][0]["future"], json!({"depth": 2}));
    let mut moved = newer.clone();
    moved["x"] = json!(7.0);
    assert_eq!(body(&e, id, "r"), moved);
    // A strict list cannot write a value this build cannot read; its undo could, as it was.
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "r", "set": {"gradient": spiral()}}]}));
    assert_eq!(err(&refused).code, "design.node_invalid");
    assert!(err(&refused).message.starts_with(r#"ops[0] update "r": gradient: unknown variant `spiral`"#), "{}", err(&refused).message);
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "r", "set": {"future": 3}}]}));
    assert!(err(&refused).message.starts_with(r#"ops[0] update "r": future: not a node field"#), "{}", err(&refused).message);
    let undo = apply(&e, id, json!([{"op": "update", "id": "r", "set": {"future": null}}]))["inverse"].clone();
    assert_eq!(undo, json!([{"op": "update", "id": "r", "set": {"future": {"depth": 2}}}]), "the undo captures what it removed");
    lenient(&e, id, undo);
    assert_eq!(body(&e, id, "r"), moved);
    // Deleted and undone: back as it was.
    let out = apply(&e, id, json!([{"op": "delete", "id": "r"}]));
    assert_eq!(out["inverse"][0]["node"]["gradient"], spiral());
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(body(&e, id, "r"), moved);
}

/// B61: one row this build cannot read no longer makes the whole design unreadable; it is left
/// out of `design.get` with what is inside it.
#[test]
fn an_unreadable_row_is_left_out_not_the_design() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([{"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect", "w": 10, "h": 10}}]));
    put(&e, id, "w", "page-1", "widget", r#"{"kind":"widget","x":5}"#);
    put(&e, id, "w-child", "w", "rect", r#"{"kind":"rect","x":1}"#);
    put(&e, id, "bad", "page-1", "rect", "not json");
    assert_eq!(ids(&nodes(&e, id)), ["page-1", "r"]);
    // One page asked for alone reads the same way.
    assert_eq!(ids(&ok(&e, "design.get", json!({"design_id": id, "page": "page-1"}))["nodes"]), ["page-1", "r"]);
}

/// A row of a kind this build does not know is a leaf it cannot edit: every scan reads past it,
/// and every edit that would touch it (it, or a delete whose subtree holds it) is refused, or
/// skipped when lenient, leaving it exactly as it was.
#[test]
fn a_row_of_an_unknown_kind_is_never_edited() {
    let (e, _root) = project();
    let id = create(&e);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "f", "parent": "page-1", "kind": "frame", "w": 100, "h": 100}},
        {"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect", "x": 200, "w": 10, "h": 10}},
        {"op": "add", "node": {"id": "s", "parent": "page-1", "kind": "rect", "x": 300, "w": 10, "h": 10}},
    ]));
    let widget = r#"{"kind":"widget","x":5,"from":"r","to":"s"}"#;
    put(&e, id, "w", "page-1", "widget", widget);
    put(&e, id, "w2", "f", "widget", widget);
    apply(&e, id, json!([{"op": "add", "node": {"id": "ln", "parent": "page-1", "kind": "line", "points": [[0, 0], [5, 5]], "from": "w", "to": "r"}}]));
    let at = rev(&e, id);
    let refusal = |ops: Value| {
        let response = call(&e, "design.apply", json!({"design_id": id, "ops": ops}));
        (err(&response).code.clone(), err(&response).message.clone())
    };
    for op in [
        json!({"op": "update", "id": "w", "set": {"x": 1}}),
        json!({"op": "move", "id": "w", "index": 0}),
        json!({"op": "delete", "id": "w"}),
        json!({"op": "delete", "id": "f"}),
    ] {
        let (code, message) = refusal(json!([op]));
        assert_eq!(code, "design.node_unreadable", "{op}");
        assert!(message.contains(r#"is of a kind this Relay cannot read ("widget"); open it in a newer Relay"#), "{message}");
    }
    assert_eq!(refusal(json!([{"op": "add", "node": {"parent": "w", "kind": "rect"}}])).0, "design.parent");
    assert_eq!(refusal(json!([{"op": "move", "id": "r", "parent": "w"}])).0, "design.parent");
    assert_eq!(rev(&e, id), at, "nothing applied");
    // An undo skips them, as it skips a node that is gone.
    let out = lenient(&e, id, json!([{"op": "update", "id": "w", "set": {"x": 1}}, {"op": "delete", "id": "f"},
        {"op": "move", "id": "w2", "index": 0}, {"op": "move", "id": "r", "parent": "w"}]));
    assert_eq!((out["ids"].clone(), out.get("warnings")), (json!(["", "", "", ""]), None));
    assert_eq!(find(&nodes(&e, id), "r")["parent"], "page-1");
    assert!(ids(&nodes(&e, id)).contains(&"f".to_string()));
    // A connector's end on it is a node that is there: deleting the other end frees only that one.
    let out = apply(&e, id, json!([{"op": "delete", "id": "r"}]));
    let line = find(&nodes(&e, id), "ln");
    assert_eq!((line["from"].clone(), line.get("to")), (json!("w"), None), "{line}");
    // The widget's own `to` names the deleted node; the scan reads past it, leaving it as written.
    assert_eq!(body(&e, id, "w"), serde_json::from_str::<Value>(widget).unwrap());
    assert_eq!(body(&e, id, "w2"), serde_json::from_str::<Value>(widget).unwrap());
    lenient(&e, id, out["inverse"].clone());
    assert_eq!(find(&nodes(&e, id), "ln")["to"], "r");
}

/// B8: a strict list writes only fields the node's kind uses (a typo is no field at all); a null
/// clears any known field; and an undo still puts back a stale field a node held.
#[test]
fn fields_a_kind_does_not_use_are_refused_but_undo_still_restores_them() {
    let (e, _root) = project();
    let id = create(&e);
    let refused = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"id": "e", "parent": "page-1", "kind": "ellipse", "radius": 4}}]}));
    assert_eq!(err(&refused).code, "design.node_invalid");
    assert_eq!(err(&refused).message, r#"ops[0] add "e": radius: ellipse nodes do not use it (it is for frame, rect, sticky, image)"#);
    let typo = call(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "add", "node": {"parent": "page-1", "kind": "rect", "fil": "#fff"}}]}));
    assert!(err(&typo).message.starts_with("ops[0] add rect: fil: not a node field"), "{}", err(&typo).message);
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "e", "parent": "page-1", "kind": "ellipse", "w": 10, "h": 10}},
        {"op": "update", "id": "e", "set": {"radius": null, "src": null}},
        {"op": "add", "node": {"id": "g", "parent": "page-1", "kind": "group"}},
        {"op": "add", "node": {"id": "in", "parent": "g", "kind": "rect", "w": 10, "h": 10}},
    ]));
    // A group given a shadow before groups were held to their fields.
    let mut stale = body(&e, id, "g");
    stale["shadow"] = json!({"x": 0.0, "y": 2.0, "blur": 6.0, "spread": 0.0, "color": "#000000"});
    rewrite(&e, id, "g", stale.clone());
    apply(&e, id, json!([{"op": "update", "id": "g", "set": {"x": 5}}]));
    stale["x"] = json!(5.0);
    assert_eq!(body(&e, id, "g"), stale, "an edit of something else keeps it");
    let before = nodes(&e, id);
    let out = apply(&e, id, json!([{"op": "delete", "id": "g"}]));
    let undone = lenient(&e, id, out["inverse"].clone());
    assert_eq!(nodes(&e, id), before);
    assert_eq!(body(&e, id, "g"), stale);
    assert!(undone.get("warnings").is_none(), "the person hears nothing: {undone}");
}

/// §1 #5: an agent replaying the undo of its own delete of legacy nodes gets them back exactly,
/// and hears what a strict list would have refused; its lenient lists stay within MAX_OPS.
#[test]
fn an_agent_replaying_its_undo_of_legacy_nodes_hears_what_it_put_back() {
    let (e, _root) = project();
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let agent = Actor::agent(&name);
    let id = ok_as(&e, agent.clone(), "design.create", json!({"title": "Legacy"}))["design"]["id"].as_i64().unwrap();
    ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "g", "parent": "page-1", "kind": "group"}},
        {"op": "add", "node": {"id": "t", "parent": "g", "kind": "text", "text": "Hi"}},
    ]}));
    let mut g = body(&e, id, "g");
    g["shadow"] = json!({"x": 0.0, "y": 2.0, "blur": 6.0, "spread": 0.0, "color": "#000000"});
    g["future"] = json!(1);
    rewrite(&e, id, "g", g.clone());
    let mut t = body(&e, id, "t");
    t["name"] = json!("bell\u{7}");
    rewrite(&e, id, "t", t.clone());
    let before = nodes(&e, id);

    let out = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "delete", "id": "g"}], "reply": "full"}));
    let undone = ok_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true}));
    assert_eq!(nodes(&e, id), before);
    assert_eq!((body(&e, id, "g"), body(&e, id, "t")), (g, t));
    let warnings: Vec<&str> = undone["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert_eq!(warnings, [
        r#"ops[0] add "g": future: not a node field; design.get lists a node's fields; kept as it was"#,
        r#"ops[1] add "t": name: no control characters; kept as it was"#,
    ]);
    // The same list, strict, is refused at its first breach.
    let strict = call_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "delete", "id": "g"}]}));
    assert!(strict.ok);
    let replay = call_as(&e, agent.clone(), "design.apply", json!({"design_id": id, "ops": out["inverse"]}));
    assert_eq!(code(replay), "design.node_invalid");

    // An agent's lenient lists stay within MAX_OPS; the person's undo may hold a design's worth.
    let ghosts: Vec<Value> = (0..2001).map(|i| json!({"op": "update", "id": format!("ghost-{i}"), "set": {"x": 1}})).collect();
    assert_eq!(code(call_as(&e, agent, "design.apply", json!({"design_id": id, "ops": ghosts, "lenient": true}))), "design.ops");
    lenient(&e, id, json!(ghosts));
}

/// B43, B71, B72, TM-16, B96, B99: what a strict list writes is held to the written rules, and a
/// refusal leaves the design as it was.
#[test]
fn written_rules_hold_a_strict_list() {
    let (e, _root) = project();
    let id = create(&e);
    // B43: a title, a name, a font or a text with a control character.
    assert_eq!(code(call(&e, "design.create", json!({"project_id": 1, "title": "Home\u{0}x"}))), "design.title");
    assert_eq!(code(call(&e, "design.update", json!({"design_id": id, "title": "Bell\u{7}"}))), "design.title");
    assert_eq!(code(call(&e, "design.duplicate", json!({"design_id": id, "title": "Tab\tbed"}))), "design.title");
    apply(&e, id, json!([
        {"op": "add", "node": {"id": "r", "parent": "page-1", "kind": "rect", "w": 100, "h": 50}},
        {"op": "add", "node": {"id": "words", "parent": "page-1", "kind": "text", "text": "one\ntwo\tthree"}},
        {"op": "add", "node": {"id": "ln", "parent": "page-1", "kind": "line", "points": [[0, 0], [9, 9]]}},
        {"op": "add", "node": {"id": "i", "parent": "page-1", "kind": "icon", "icon": "home", "w": 24, "h": 24, "dash": [2, 3]}},
    ]));
    let at = rev(&e, id);
    let refusal = |ops: Value| {
        let response = call(&e, "design.apply", json!({"design_id": id, "ops": ops}));
        let error = err(&response);
        assert_eq!(error.code, "design.node_invalid", "{}", error.message);
        error.message.clone()
    };
    assert!(refusal(json!([{"op": "add", "node": {"id": "t", "parent": "page-1", "kind": "text", "text": "a\u{0}b"}}]))
        .starts_with(r#"ops[0] add "t": text: no control characters other than tab and newline"#));
    assert!(refusal(json!([{"op": "update", "id": "words", "set": {"font": "Geist\u{1}"}}])).starts_with(r#"ops[0] update "words": font: no control"#));
    // B71: both spellings of a size.
    assert_eq!(refusal(json!([{"op": "update", "id": "r", "set": {"w": 10, "width": 20}}])), r#"ops[0] update "r": w: give w or width, not both"#);
    assert_eq!(refusal(json!([{"op": "update", "id": "r", "set": {"height": 20, "h": 10}}])), r#"ops[0] update "r": h: give h or height, not both"#);
    // B72: a shadow's blur stops at 200.
    assert_eq!(refusal(json!([{"op": "update", "id": "r", "set": {"shadow": {"blur": 201, "color": "#000"}}}])),
        r#"ops[0] update "r": shadow.blur: 201 is outside 0 to 200"#);
    // TM-16: a nested object written as an array.
    assert!(refusal(json!([{"op": "update", "id": "r", "set": {"shadow": [0, 2, 6, 0, "#000"]}}]))
        .starts_with(r#"ops[0] update "r": shadow: invalid type: sequence, expected an object"#));
    // B99: a line turns by its points.
    assert!(refusal(json!([{"op": "update", "id": "ln", "set": {"rotation": 30}}])).starts_with(r#"ops[0] update "ln": rotation: a page does not turn, and a line turns by its points"#));
    assert_eq!(rev(&e, id), at, "nothing applied");
    assert_eq!(find(&nodes(&e, id), "r")["w"], 100.0);
    // What holds to the rules goes through: a blur of 200, a null rotation on a line, a dashed
    // icon (B96), `width` alone.
    apply(&e, id, json!([
        {"op": "update", "id": "r", "set": {"shadow": {"blur": 200, "color": "#000"}, "width": 120}},
        {"op": "update", "id": "ln", "set": {"rotation": null}},
        {"op": "update", "id": "i", "set": {"dash": [1, 4]}},
    ]));
    assert_eq!(find(&nodes(&e, id), "r")["w"], 120.0);
}
