//! design.apply's `contrast`: an agent's change is answered with the contrast flags it makes,
//! worked out without the desktop app once it has committed: new flags only, on the pages it
//! reached (every page after a colour edit), none for the person's own edits.

mod common;

use common::{committed_repo, engine_with_project, ok, ok_as};
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

fn agent(e: &Engine) -> Actor {
    let name = ok(e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    Actor::agent(&name)
}

/// The flags' ids, each as `text/ground`.
fn flagged(reply: &Value) -> Vec<String> {
    reply.get("contrast").and_then(Value::as_array).map_or_else(Vec::new, |all| {
        all.iter().map(|i| i["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>().join("/")).collect()
    })
}

/// A light screen and a dark one, each with a caption in `$muted`, which reads on the dark one
/// only; a third page the edits never reach.
fn screens(e: &Engine) -> i64 {
    let id = ok(e, "design.create", json!({"project_id": 1, "title": "Budget"}))["design"]["id"].as_i64().unwrap();
    ok(e, "design.apply", json!({"design_id": id, "ops": [
        {"op": "color", "name": "muted", "value": "#9AA0A6"},
        {"op": "add", "node": {"id": "light", "parent": "page-1", "kind": "frame", "w": 390, "h": 200, "fill": "#FFFFFF"}},
        {"op": "add", "node": {"id": "light-caption", "parent": "light", "kind": "text", "x": 24, "y": 24, "text": "Spent this month", "color": "$muted"}},
        {"op": "add", "node": {"id": "dark", "parent": "page-1", "kind": "frame", "x": 470, "w": 390, "h": 200, "fill": "#101418"}},
        {"op": "add", "node": {"id": "dark-caption", "parent": "dark", "kind": "text", "x": 24, "y": 24, "text": "Left to spend", "color": "$muted"}},
        {"op": "add", "node": {"id": "ring", "parent": "dark", "kind": "ellipse", "x": 100, "y": 60, "w": 120, "h": 120, "fill": "#E8F0FF",
            "arc": {"start": 0, "sweep": 250, "inner": 0.8}}},
        {"op": "add", "node": {"id": "pct", "parent": "dark", "kind": "text", "x": 140, "y": 110, "text": "69%", "color": "#FFFFFF"}},
        {"op": "add", "node": {"id": "page-2", "kind": "page", "name": "Other"}},
        {"op": "add", "node": {"id": "far", "parent": "page-2", "kind": "frame", "w": 200, "h": 100, "fill": "#FFFFFF"}},
        {"op": "add", "node": {"id": "far-note", "parent": "far", "kind": "text", "x": 10, "y": 10, "text": "Faint", "color": "#DDDDDD"}},
    ]}));
    id
}

/// Darkening `$muted` fixes the light caption and breaks the dark one: the reply names the dark
/// one alone. The ring's light arc is not read as the ground of the words in its hole, and the
/// faint note that was flagged before, on a page no edit changed, is not new.
#[test]
fn a_colour_edit_is_answered_with_the_flags_it_makes() {
    let (e, _root) = project();
    let id = screens(&e);
    let me = agent(&e);
    let reply = ok_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": "#5F6368"}]}));
    assert_eq!(flagged(&reply), ["dark-caption/dark"], "{reply}");
    let message = reply["contrast"][0]["message"].as_str().unwrap();
    assert!(message.starts_with("dark-caption: #5F6368 on #101418 (dark) is ") && message.ends_with("needs 4.5:1"), "{message}");
    assert_eq!(reply["contrast"][0]["code"], "contrast");
    // Putting it back breaks the light one again and mends the dark one.
    let back = ok_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": "#9AA0A6"}]}));
    assert_eq!(flagged(&back), ["light-caption/light"]);
    // An edit that makes nothing worse says nothing; one on another page checks that page.
    let quiet = ok_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "pct", "set": {"text": "70%"}}]}));
    assert!(quiet.get("contrast").is_none(), "{quiet}");
    let added = ok_as(&e, me, "design.apply", json!({"design_id": id, "ops": [
        {"op": "add", "node": {"id": "far-more", "parent": "far", "kind": "text", "x": 10, "y": 50, "text": "Also faint", "color": "#EEEEEE"}},
    ]}));
    assert_eq!(flagged(&added), ["far-more/far"]);
}

/// The person's own edits are not checked: the canvas shows what they did.
#[test]
fn the_persons_edits_carry_no_flags() {
    let (e, _root) = project();
    let id = screens(&e);
    let reply = ok(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": "#5F6368"}]}));
    assert!(reply.get("contrast").is_none(), "{reply}");
}

/// A node with skip_checks is never flagged; deleting the ground under a text flags it on what
/// shows instead.
#[test]
fn skip_checks_and_deletes() {
    let (e, _root) = project();
    let id = screens(&e);
    let me = agent(&e);
    ok(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "update", "id": "dark-caption", "set": {"skip_checks": true}}]}));
    let reply = ok_as(&e, me.clone(), "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": "#5F6368"}]}));
    assert!(reply.get("contrast").is_none(), "{reply}");
    // The light caption reads now; without its white screen it sits on the dark canvas.
    let gone = ok_as(&e, me, "design.apply", json!({"design_id": id, "ops": [
        {"op": "move", "id": "light-caption", "parent": "page-1"},
        {"op": "delete", "id": "light"},
    ]}));
    assert_eq!(flagged(&gone), ["light-caption/page-1"], "on the page's canvas: {gone}");
}

/// What the check costs an agent's reply on a full design: 10 pages of 400 nodes (screens of
/// cards with a title, a caption and a badge), a colour edit that reaches every page. Prints the
/// median of an agent's apply and of the person's (unchecked) one; run with
/// `cargo test -p relay-core --test design_contrast -- --ignored --nocapture`.
#[test]
#[ignore]
fn the_check_costs_on_a_full_design() {
    let (e, _root) = project();
    let id = ok(&e, "design.create", json!({"project_id": 1, "title": "Big"}))["design"]["id"].as_i64().unwrap();
    ok(&e, "design.apply", json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": "#9AA0A6"}]}));
    for page in 0..10 {
        let pid = if page == 0 { "page-1".to_string() } else { format!("pg{page}") };
        let mut ops = Vec::new();
        if page > 0 {
            ops.push(json!({"op": "add", "node": {"id": pid, "kind": "page"}}));
        }
        for screen in 0..4 {
            let s = format!("{pid}-s{screen}");
            ops.push(json!({"op": "add", "node": {"id": s, "parent": pid, "kind": "frame", "x": screen * 470, "w": 390, "h": 2000, "layout": "column", "gap": 8, "padding": [16], "fill": if screen % 2 == 0 { "#FFFFFF" } else { "#101418" }}}));
            for card in 0..24 {
                let c = format!("{s}-c{card}");
                ops.push(json!({"op": "add", "node": {"id": c, "parent": s, "kind": "frame", "w_sizing": "fill", "h": 72, "layout": "row", "gap": 8, "padding": [12], "radius": 12, "fill": "#F4F5F7"}}));
                ops.push(json!({"op": "add", "node": {"id": format!("{c}-t"), "parent": c, "kind": "text", "text": "Groceries and household", "font_weight": 600}}));
                ops.push(json!({"op": "add", "node": {"id": format!("{c}-m"), "parent": c, "kind": "text", "text": "$212.40 of $300", "color": "$muted"}}));
                ops.push(json!({"op": "add", "node": {"id": format!("{c}-b"), "parent": c, "kind": "ellipse", "w": 24, "h": 24, "text": "3", "font_size": 12}}));
            }
        }
        ok(&e, "design.apply", json!({"design_id": id, "ops": ops}));
    }
    let nodes = ok(&e, "design.get", json!({"design_id": id, "pages_only": true}))["design"]["nodes"].as_i64().unwrap();
    let me = agent(&e);
    let time = |as_agent: bool| {
        let mut runs: Vec<f64> = (0..15).map(|i| {
            let value = if i % 2 == 0 { "#5F6368" } else { "#9AA0A6" };
            let payload = json!({"design_id": id, "ops": [{"op": "color", "name": "muted", "value": value}]});
            let started = std::time::Instant::now();
            if as_agent { ok_as(&e, me.clone(), "design.apply", payload); } else { ok(&e, "design.apply", payload); }
            started.elapsed().as_secs_f64() * 1000.0
        }).collect();
        runs.sort_by(f64::total_cmp);
        runs[runs.len() / 2]
    };
    let (person, agent) = (time(false), time(true));
    println!("{nodes} nodes: the person's apply {person:.2} ms, an agent's {agent:.2} ms (median of 15)");
}
