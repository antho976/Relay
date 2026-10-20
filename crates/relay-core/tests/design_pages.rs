//! `design.get` by page (MODEL-SPEC §3.1, engine-pages): `pages_only` answers a design's page
//! list without what is on the pages, for a reader that then reads one page at a time (the
//! phone, an agent finding its way around a large design), and `page` takes a page's id or the
//! name it is shown by, so the name the person sees on the canvas reads the page they mean
//! (TM-4) instead of `design.page_not_found`.

mod common;

use common::{call, call_as, committed_repo, engine_with_project, err, ok, ok_as};
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

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
}

fn get(e: &Engine, payload: Value) -> Value {
    ok(e, "design.get", payload)
}

fn ids(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()).collect()
}

/// Three pages: `blank` with no name, put first, so the canvas shows it as "Page 1"; `page-1`
/// ("Page 1", as design.create names it) with a card on it; `checkout` ("Checkout") with a frame
/// and a text.
fn sample(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Shop"}))["design"]["id"].as_i64().unwrap();
    apply(e, id, json!([
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 200}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hello"}},
        {"op": "add", "node": {"id": "checkout", "kind": "page", "name": "Checkout"}},
        {"op": "add", "node": {"id": "pay", "parent": "checkout", "kind": "frame", "w": 200, "h": 80}},
        {"op": "add", "node": {"id": "total", "parent": "pay", "kind": "text", "text": "Total"}},
        {"op": "add", "node": {"id": "blank", "kind": "page"}, "index": 0},
    ]));
    id
}

/// The phone's and an agent's first read of a design: its pages in their order, nothing on them,
/// beside the whole design's row; with `page`, that page alone.
#[test]
fn pages_only_lists_the_pages_alone() {
    let (e, _root) = project();
    let id = sample(&e);
    let out = get(&e, json!({"design_id": id, "pages_only": true}));
    assert_eq!(ids(&out["nodes"]), ["blank", "page-1", "checkout"], "the pages in their order");
    assert!(out["nodes"].as_array().unwrap().iter().all(|n| n["kind"] == "page" && n.get("parent").is_none()), "{}", out["nodes"]);
    assert_eq!(out["nodes"][2]["name"], "Checkout", "a page comes whole");
    assert_eq!((out["design"]["pages"].as_i64(), out["design"]["nodes"].as_i64()), (Some(3), Some(7)), "the row counts the whole design");
    assert!(out["assets_dir"].is_string());

    // Reordered, the list follows.
    apply(&e, id, json!([{"op": "move", "id": "checkout", "index": 0}]));
    assert_eq!(ids(&get(&e, json!({"design_id": id, "pages_only": true}))["nodes"]), ["checkout", "blank", "page-1"]);

    // With a page: that page alone, by id or by name.
    assert_eq!(ids(&get(&e, json!({"design_id": id, "pages_only": true, "page": "page-1"}))["nodes"]), ["page-1"]);
    assert_eq!(ids(&get(&e, json!({"design_id": id, "pages_only": true, "page": "Checkout"}))["nodes"]), ["checkout"]);
    let unknown = call(&e, "design.get", json!({"design_id": id, "pages_only": true, "page": "nope"}));
    assert_eq!(err(&unknown).code, "design.page_not_found");

    // `pages_only: false` is a plain read.
    assert_eq!(get(&e, json!({"design_id": id, "pages_only": false})), get(&e, json!({"design_id": id})));
    // A deleted design still reads, its pages too.
    ok(&e, "design.delete", json!({"design_id": id}));
    assert_eq!(get(&e, json!({"design_id": id, "pages_only": true}))["nodes"].as_array().unwrap().len(), 3);
}

/// pages_only answers the pages without what is on them, so it means nothing beside a node scope
/// or a since_rev: refused by name rather than one of them ignored.
#[test]
fn pages_only_goes_alone_or_with_page() {
    let (e, _root) = project();
    let id = sample(&e);
    for (field, value) in [("nodes", json!(["card"])), ("since_rev", json!(1))] {
        let mut payload = json!({"design_id": id, "pages_only": true, "page": "page-1"});
        payload[field] = value;
        let refused = call(&e, "design.get", payload);
        let error = err(&refused);
        assert_eq!((error.code.as_str(), format!("{:?}", error.kind)), ("design.pages_only", "Invalid".to_string()), "{field}");
        assert_eq!(error.message, format!("pages_only: give it alone or with page, not with {field}"));
        assert!(error.hint.as_deref().is_some_and(|h| h.starts_with("drop pages_only")), "{:?}", error.hint);
    }
    // The refusal comes before the design is read: nothing to find for it.
    let refused = call(&e, "design.get", json!({"design_id": 999, "pages_only": true, "since_rev": 0}));
    assert_eq!(err(&refused).code, "design.pages_only");
    // Alone, it reads the design like any other read: there is none.
    assert_eq!(err(&call(&e, "design.get", json!({"design_id": 999, "pages_only": true}))).code, "design.not_found");
}

/// TM-4: the canvas, the phone and the Ask prompt show a page by its name; that name reads it.
#[test]
fn a_page_by_name_resolves() {
    let (e, _root) = project();
    let id = sample(&e);
    // Unnamed `blank` stands first, so the canvas shows it as "Page 1" too, and two pages show that
    // name: rename `page-1` so each name is one page's.
    apply(&e, id, json!([{"op": "update", "id": "page-1", "set": {"name": "Home"}}]));
    let by_id = get(&e, json!({"design_id": id, "page": "checkout"}));
    assert_eq!(ids(&by_id["nodes"]), ["checkout", "pay", "total"]);
    assert_eq!(get(&e, json!({"design_id": id, "page": "Checkout"})), by_id, "the name reads what the id reads");
    assert_eq!(ids(&get(&e, json!({"design_id": id, "page": "Home"}))["nodes"]), ["page-1", "card", "title"]);
    // A page with no name answers to the name it is shown by: "Page n", n its place among the pages.
    assert_eq!(ids(&get(&e, json!({"design_id": id, "page": "Page 1"}))["nodes"]), ["blank"]);
    apply(&e, id, json!([{"op": "move", "id": "blank", "index": 2}]));
    assert_eq!(ids(&get(&e, json!({"design_id": id, "page": "Page 3"}))["nodes"]), ["blank"], "its place now");
    assert_eq!(err(&call(&e, "design.get", json!({"design_id": id, "page": "Page 1"}))).code, "design.page_not_found");
    // An id wins over a name: a page named after another page's id does not shadow it.
    apply(&e, id, json!([{"op": "update", "id": "checkout", "set": {"name": "page-1"}}]));
    assert_eq!(ids(&get(&e, json!({"design_id": id, "page": "page-1"}))["nodes"]), ["page-1", "card", "title"]);
    // A name is matched as it is written.
    assert_eq!(err(&call(&e, "design.get", json!({"design_id": id, "page": "home"}))).code, "design.page_not_found");
    // An agent of the project reads by name as the person does.
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let agent = Actor::agent(&name);
    assert_eq!(ids(&ok_as(&e, agent.clone(), "design.get", json!({"design_id": id, "page": "Home", "pages_only": true}))["nodes"]), ["page-1"]);
    let refused = call_as(&e, agent, "design.get", json!({"design_id": id, "page": "Shop"}));
    assert_eq!(err(&refused).code, "design.page_not_found");
}

/// An unknown name lists the pages to pick from, with their ids; a name two pages share lists
/// those two; a near miss is named.
#[test]
fn an_unknown_or_shared_name_lists_the_pages() {
    let (e, _root) = project();
    let id = sample(&e);
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "Cart"}));
    let error = err(&refused);
    assert_eq!((error.code.as_str(), format!("{:?}", error.kind)), ("design.page_not_found", "NotFound".to_string()));
    assert_eq!(error.message, r#"the design has no page "Cart""#);
    assert_eq!(error.hint.as_deref(), Some(r#"pages: blank ("Page 1"), page-1 ("Page 1"), checkout ("Checkout"); pass a page's id"#));

    // "Page 1" is shown on two pages: the unnamed first one and `page-1`. The hint lists those.
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "Page 1"}));
    let error = err(&refused);
    assert_eq!(error.message, r#"2 pages are named "Page 1""#);
    assert_eq!(error.hint.as_deref(), Some(r#"pages: blank ("Page 1"), page-1 ("Page 1"); pass a page's id"#));
    // Stored names count the same way.
    apply(&e, id, json!([{"op": "add", "node": {"id": "dup", "kind": "page", "name": "Checkout"}}]));
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "Checkout", "pages_only": true}));
    assert_eq!(err(&refused).message, r#"2 pages are named "Checkout""#);
    assert_eq!(err(&refused).hint.as_deref(), Some(r#"pages: checkout ("Checkout"), dup ("Checkout"); pass a page's id"#));

    // A near miss, by id or by name, is named.
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "chekout"}));
    assert_eq!(err(&refused).message, r#"the design has no page "chekout"; did you mean "checkout"?"#);
    apply(&e, id, json!([{"op": "update", "id": "page-1", "set": {"name": "Home"}}]));
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "home"}));
    assert_eq!(err(&refused).message, r#"the design has no page "home"; did you mean "Home" (page-1)?"#);
    // A node that is not a page is no page.
    assert_eq!(err(&call(&e, "design.get", json!({"design_id": id, "page": "card"}))).code, "design.page_not_found");
}

/// A design of many pages names the first 20 and counts the rest, pointing at the whole list.
#[test]
fn a_long_page_list_is_cut_in_the_hint() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1}))["design"]["id"].as_i64().unwrap();
    let ops: Vec<Value> = (2..=25).map(|n| json!({"op": "add", "node": {"id": format!("p{n}"), "kind": "page", "name": format!("Screen {n}")}})).collect();
    apply(&e, id, Value::Array(ops));
    let refused = call(&e, "design.get", json!({"design_id": id, "page": "Settings"}));
    let hint = err(&refused).hint.clone().unwrap();
    assert!(hint.starts_with(r#"pages: page-1 ("Page 1"), p2 ("Screen 2"), "#), "{hint}");
    assert!(hint.contains(r#"p20 ("Screen 20"), and 5 more (design.get pages_only lists them all); pass a page's id"#), "{hint}");
    assert!(!hint.contains("p21"), "{hint}");
    assert_eq!(get(&e, json!({"design_id": id, "pages_only": true}))["nodes"].as_array().unwrap().len(), 25);
}
