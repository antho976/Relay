//! The node fields and edits of the design canvas's second round, through the engine
//! (MODEL-SPEC §2.4-§2.11): what a strict list stores and refuses, and the edits whose engine
//! part a later stream brings, refused until then by name.

mod common;

use common::{call, committed_repo, engine_with_project, err, ok};
use relay_bus::BusError;
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

fn find(nodes: &Value, id: &str) -> Value {
    nodes.as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap_or_else(|| panic!("no {id} in {nodes}"))
}

/// The refusal of `ops`, with the design left as it was.
fn refused(e: &Engine, design: i64, ops: Value) -> BusError {
    let before = ok(e, "design.get", json!({"design_id": design}));
    let error = err(&call(e, "design.apply", json!({"design_id": design, "ops": ops}))).clone();
    assert_eq!(ok(e, "design.get", json!({"design_id": design})), before, "nothing of a refused list stays");
    error
}

/// A frame with a card in it, the first edits of most lists here.
fn card(e: &Engine, design: i64) {
    apply(e, design, json!([
        {"op": "add", "node": {"id": "screen", "parent": "page-1", "kind": "frame", "w": 390, "h": 844}},
        {"op": "add", "node": {"id": "card", "parent": "screen", "kind": "frame", "x": 16, "y": 16, "w": 358, "h": 120}},
    ]));
}

/// Layout fields are stored as written and read back; their value rules refuse by field.
#[test]
fn layout_fields_are_stored_and_checked() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "badge", "parent": "card", "kind": "ellipse", "x": 326, "y": 8, "w": 24, "h": 24,
            "pin_x": "right", "absolute": true}},
        {"op": "add", "node": {"id": "chips", "parent": "card", "kind": "frame", "w": 300, "h": 60, "layout": "wrap", "gap": 8,
            "row_gap": 4, "min_w": 200, "max_w": 400, "pin_x": "left_right"}},
        {"op": "add", "node": {"id": "rail", "parent": "screen", "kind": "frame", "y": 200, "w": 390, "h": 120, "layout": "row",
            "scroll": "x"}},
    ]));
    let nodes = ok(&e, "design.get", json!({"design_id": id}))["nodes"].clone();
    assert_eq!((find(&nodes, "badge")["pin_x"].clone(), find(&nodes, "badge")["absolute"].clone()), (json!("right"), json!(true)));
    let chips = find(&nodes, "chips");
    assert_eq!((chips["layout"].clone(), chips["row_gap"].clone(), chips["max_w"].clone()), (json!("wrap"), json!(4.0), json!(400.0)));
    assert_eq!(find(&nodes, "rail")["scroll"], "x");
    assert_eq!(out["ids"], json!(["badge", "chips", "rail"]));
    for (node, code, message) in [
        (json!({"id": "x", "parent": "card", "kind": "frame", "w": 500, "max_w": 400}), "design.node_invalid",
            "ops[0] add \"x\": w: 500 is outside min_w to max_w (0 to 400); a fixed width stays within them"),
        (json!({"id": "x", "parent": "card", "kind": "line", "points": [[0, 0], [9, 0]], "pin_x": "scale"}), "design.node_invalid",
            "ops[0] add \"x\": pin_x: a line's points do not stretch; use left, right or center"),
        (json!({"id": "x", "parent": "card", "kind": "rect", "w": 9, "h": 9, "scroll": "y"}), "design.node_invalid",
            "ops[0] add \"x\": scroll: only a frame scrolls"),
        (json!({"id": "x", "parent": "card", "kind": "group", "min_h": 4}), "design.node_invalid",
            "ops[0] add \"x\": min_h: group nodes do not use it (it is for frame, rect, ellipse, text, sticky, image, polygon, icon)"),
    ] {
        let error = refused(&e, id, json!([{"op": "add", "node": node}]));
        assert_eq!((error.code.as_str(), error.message.as_str()), (code, message));
    }
}

/// `keep_position: false` moves as a plain move does, and its undo carries none (what true does
/// is design_pins').
#[test]
fn keep_position_false_is_a_plain_move() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    let out = apply(&e, id, json!([{"op": "move", "id": "card", "parent": "page-1", "keep_position": false}]));
    assert_eq!(out["inverse"], json!([{"op": "move", "id": "card", "parent": "screen", "index": 0}]));
}

/// Drawing: a text's words and its runs change together, and the undo restores both.
#[test]
fn text_and_runs_change_together() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    apply(&e, id, json!([{"op": "add", "node": {"id": "km", "parent": "card", "kind": "text", "text": "8.4 km", "w": 160, "max_lines": 1,
        "runs": [{"start": 3, "end": 6, "font_size": 14, "color": "#8C877F"}]}}]));
    let error = refused(&e, id, json!([{"op": "update", "id": "km", "set": {"text": "12.1 km"}}]));
    assert_eq!((error.code.as_str(), error.message.as_str()),
        ("design.node_invalid", "ops[0] update \"km\": runs: this text has runs; send runs (or runs: null) with its new text"));
    assert_eq!(error.details.unwrap()["field"], "runs");
    let out = apply(&e, id, json!([{"op": "update", "id": "km", "set": {"text": "12.1 km", "runs": [{"start": 4, "end": 7, "font_size": 14}]}}]));
    assert_eq!(find(&out["nodes"], "km")["runs"], json!([{"start": 4, "end": 7, "font_size": 14.0}]));
    let undo = ok(&e, "design.apply", json!({"design_id": id, "ops": out["inverse"], "lenient": true}));
    let km = find(&undo["nodes"], "km");
    assert_eq!((km["text"].clone(), km["runs"].clone()), (json!("8.4 km"), json!([{"start": 3, "end": 6, "font_size": 14.0, "color": "#8C877F"}])));
    // A run past its text is a value rule: refused whatever the list.
    let error = refused(&e, id, json!([{"op": "update", "id": "km", "set": {"runs": [{"start": 3, "end": 9, "italic": true}]}}]));
    assert_eq!(error.message, "ops[0] update \"km\": runs[0].end: 9 is past the text's 6 characters");
}

/// Drawing: a text, line, path or icon casts outer shadows (a glow) and refuses inner ones and
/// spread in a strict list; one node holds `shadow` or `shadows`. An undo puts back what was
/// stored, telling an agent.
#[test]
fn shadows_follow_the_kinds_that_cast_them() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    let out = apply(&e, id, json!([
        {"op": "add", "node": {"id": "glow", "parent": "card", "kind": "line", "points": [[0, 0], [200, 0]], "stroke_cap": "round",
            "stroke_gradient": {"angle": 90, "stops": [{"at": 0, "color": "#4AF0FF"}, {"at": 1, "color": "#7B61FF"}]},
            "shadows": [{"blur": 4, "color": "#4AF0FFCC"}, {"blur": 18, "color": "#4AF0FF66"}]}},
        {"op": "add", "node": {"id": "ring", "parent": "card", "kind": "ellipse", "w": 96, "h": 96,
            "arc": {"start": 0, "sweep": 252, "inner": 0.82, "cap": "round"},
            "gradient": {"kind": "conic", "angle": 350, "stops": [{"at": 0, "color": "#4AF0FF"}, {"at": 1, "color": "#7B61FF"}]}}},
    ]));
    assert_eq!(find(&out["nodes"], "ring")["gradient"]["kind"], "conic");
    for (set, message) in [
        (json!({"shadow": {"inset": true, "color": "#000"}}), "ops[0] update \"glow\": shadow.inset: line nodes cast no inner shadow"),
        (json!({"shadows": [{"spread": 2, "color": "#000"}]}), "ops[0] update \"glow\": shadows[0].spread: line nodes cast their shadow without spread; widen it with blur"),
        (json!({"shadow": {"blur": 8, "color": "#000"}}), "ops[0] update \"glow\": shadows: give shadow or shadows, not both (shadows is the list)"),
    ] {
        let error = refused(&e, id, json!([{"op": "update", "id": "glow", "set": set}]));
        assert_eq!((error.code.as_str(), error.message.as_str()), ("design.node_invalid", message));
    }
    // One shadow replacing the list in the same set is taken.
    apply(&e, id, json!([{"op": "update", "id": "glow", "set": {"shadows": null, "shadow": {"blur": 8, "color": "#4AF0FF99"}}}]));
    // A legacy inner shadow on a text, put back by an undo, is kept with a warning to an agent.
    let name = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let agent = relay_bus::Actor::agent(&name);
    let stored = json!({"id": "t", "parent": "card", "kind": "text", "text": "Hi", "shadow": {"inset": true, "color": "#000"}});
    let out = common::ok_as(&e, agent, "design.apply", json!({"design_id": id, "lenient": true, "ops": [{"op": "add", "node": stored}], "reply": "full"}));
    assert_eq!(out["warnings"], json!(["ops[0] add \"t\": shadow.inset: text nodes cast no inner shadow; kept as it was"]));
    assert_eq!(find(&out["nodes"], "t")["shadow"]["inset"], true);
}

/// B49: an image added without its size, or a shape told to hug, would draw nothing; a strict
/// list refuses both by field, and an undo puts back what was stored.
#[test]
fn an_image_needs_its_size_and_only_what_has_content_hugs() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    // A stored picture (B6: a strict list's image names one), a 1×1 PNG.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let src = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": png}))["src"].as_str().unwrap().to_string();
    let src = src.as_str();
    for (node, message) in [
        (json!({"id": "pic", "parent": "card", "kind": "image", "src": src}),
            "ops[0] add \"pic\": w: an image needs its size; design.asset.add answers w and h"),
        (json!({"id": "pic", "parent": "card", "kind": "image", "src": src, "w": 320}),
            "ops[0] add \"pic\": h: an image needs its size; design.asset.add answers w and h"),
        (json!({"id": "dot", "parent": "card", "kind": "ellipse", "w": 9, "h_sizing": "hug"}),
            "ops[0] add \"dot\": h_sizing: an ellipse cannot hug; only a text, a sticky or a frame sizes to its content"),
    ] {
        let error = refused(&e, id, json!([{"op": "add", "node": node}]));
        assert_eq!((error.code.as_str(), error.message.as_str()), ("design.node_invalid", message));
    }
    let error = refused(&e, id, json!([{"op": "update", "id": "card", "set": {"w_sizing": "hug"}}, {"op": "add",
        "node": {"id": "r", "parent": "card", "kind": "rect", "w": 4, "h": 4}}, {"op": "update", "id": "r", "set": {"w_sizing": "hug"}}]));
    assert_eq!(error.message, "ops[2] update \"r\": w_sizing: a rect cannot hug; only a text, a sticky or a frame sizes to its content");
    // A frame hugs; an image filling a laid-out row takes its width from it.
    apply(&e, id, json!([
        {"op": "update", "id": "card", "set": {"layout": "row", "w_sizing": "hug"}},
        {"op": "add", "node": {"id": "pic", "parent": "card", "kind": "image", "src": src, "w_sizing": "fill", "h": 80}},
    ]));
}

/// A node naming a colour or style the design does not hold is refused by name, saying how to
/// make it, with the names it does hold; nothing of the list stays. (Copy and the colour and style
/// edits themselves are design_reuse's.)
#[test]
fn named_colours_and_styles_are_refused_until_the_design_holds_them() {
    let (e, _root) = project();
    let id = create(&e);
    card(&e, id);
    for (ops, message) in [
        (json!([{"op": "add", "node": {"id": "cta", "parent": "card", "kind": "rect", "fill": "$accent"}}]),
            "ops[0] add \"cta\": fill: the design has no colour $accent; name it first with {\"op\": \"color\", \"name\": \"accent\", \"value\": \"#RRGGBB\"}"),
        (json!([{"op": "add", "node": {"id": "cta", "parent": "card", "kind": "rect", "style": "button"}}]),
            "ops[0] add \"cta\": style: the design has no style \"button\"; make it first with {\"op\": \"style\", \"name\": \"button\", \"set\": {…}}"),
        (json!([{"op": "update", "id": "card", "set": {"shadow": {"blur": 4, "color": "$shade"}}}]),
            "ops[0] update \"card\": shadow: the design has no colour $shade; name it first with {\"op\": \"color\", \"name\": \"shade\", \"value\": \"#RRGGBB\"}"),
    ] {
        let error = refused(&e, id, ops);
        assert_eq!((error.code.as_str(), error.message.as_str()), ("design.style_not_found", message));
        assert_eq!(error.hint.as_deref(), Some("name a colour with {\"op\": \"color\"} or a style with {\"op\": \"style\"} first"));
    }
    // With some named, the hint lists them, and a near miss is named.
    apply(&e, id, json!([{"op": "color", "name": "accent", "value": "#4C9DFF"}, {"op": "style", "name": "button", "set": {"radius": 8}}]));
    let error = refused(&e, id, json!([{"op": "update", "id": "card", "set": {"fill": "$acent"}}]));
    assert_eq!(error.message, "ops[0] update \"card\": fill: the design has no colour $acent; did you mean $accent?");
    assert_eq!(error.hint.as_deref(), Some("the design's colours and styles: $accent, button"));
    // A name that is not one is a value error, as any colour.
    let error = refused(&e, id, json!([{"op": "update", "id": "card", "set": {"fill": "$"}}]));
    assert_eq!((error.code.as_str(), error.message.as_str()),
        ("design.node_invalid", "ops[0] update \"card\": fill: \"$\" is not #RGB, #RRGGBB, #RRGGBBAA or a $name colour"));
}
