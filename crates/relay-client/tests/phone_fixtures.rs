//! The phone lays out and draws a design itself, with ports of layout.rs, shapes.rs and bounds.rs
//! (`Layout.kt` and `Shapes.kt` in apps/relay-android/core/src/main/kotlin/com/quietsoftware/
//! relay/core/design/), and draws icons from a copy of icons.rs (`design/icons.json` in that
//! module's resources). This holds the two sides together: every case of `layout-cases.json` is
//! laid out here by the PC's own code, and each box, group origin, wrap, scroll, connector,
//! polygon and painted bound it holds must be what the PC computes; the phone's DesignLayoutTest
//! and DesignShapesTest hold its port to the same file. A change to the PC's layout or geometry
//! fails here until the fixture says so, and the changed fixture fails on the phone until its port
//! follows, in the same piece of work (feature-phone §1, §8.1).
//!
//! On a mismatch the case is printed as the PC computes it. `PHONE_FIXTURES=write cargo test -p
//! relay-client --test phone_fixtures` writes both files from the PC's code instead, keeping each
//! case's name, page and nodes.

use relay_bus::design::{Node, Styles, ICONS};
use relay_client::design::bounds::painted_bounds;
use relay_client::design::icons;
use relay_client::design::layout::{place, Measure, Placed};
use relay_client::design::shapes::{line_points, node_polygon};
use relay_client::design::{NodeKind, Rect, Scene};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

const CASES_PATH: &str = "apps/relay-android/core/src/test/resources/design/layout-cases.json";
const ICONS_PATH: &str = "apps/relay-android/core/src/main/resources/design/icons.json";
const CASES: &str = include_str!("../../../apps/relay-android/core/src/test/resources/design/layout-cases.json");
const ICON_PATHS: &str = include_str!("../../../apps/relay-android/core/src/main/resources/design/icons.json");

/// How the cases are measured, said in the file for whoever reads it.
const MEASURE: &str = "mono: 8 px a character (Unicode scalar values), 20 px a line, wrapping whole characters (layout.rs tests::Mono)";

/// How far apart two numbers may be and still agree: the phone's sines and square roots may
/// round otherwise in the last place.
const TOLERANCE: f64 = 1e-6;

/// What each case holds after its nodes, in the order the file holds them.
const SECTIONS: [&str; 7] = ["rects", "origins", "wraps", "scrolls", "lines", "polygons", "bounds"];

/// 8 px per character, 20 px a line, wrapping whole characters: layout.rs's tests' `Mono`, which
/// the phone's tests count the same way (code points).
struct Mono;

impl Measure for Mono {
    fn measure(&self, n: &Node, wrap: Option<f64>) -> (f64, f64) {
        let chars = n.text.as_deref().unwrap_or("").chars().count() as f64;
        let natural = chars * 8.0;
        match wrap {
            Some(w) if natural > w && w >= 8.0 => {
                let per_line = (w / 8.0).floor();
                (per_line * 8.0, (chars / per_line).ceil() * 20.0)
            }
            _ => (natural, 20.0),
        }
    }
}

/// The repository's root, where the fixtures' paths start.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases() -> Value {
    serde_json::from_str(CASES).unwrap_or_else(|e| panic!("{CASES_PATH} is not JSON: {e}"))
}

/// A number as the file holds it: a whole one without its `.0`.
fn num(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

fn rect(r: &Rect) -> Value {
    json!([num(r.x), num(r.y), num(r.w), num(r.h)])
}

fn point((x, y): (f64, f64)) -> Value {
    json!([num(x), num(y)])
}

/// A case's scene: its nodes, read as the engine reads them, and its colours and styles.
fn scene_of(case: &Value) -> Scene {
    let name = case["name"].as_str().unwrap_or("?");
    let nodes: Vec<Node> = case["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("case {name:?} has no nodes"))
        .iter()
        .map(|v| Node::read(v.clone()).unwrap_or_else(|e| panic!("case {name:?}: {v} is not a node: {e}")))
        .collect();
    let mut looks = Map::new();
    for key in ["colors", "styles"] {
        if let Some(v) = case.get(key) {
            looks.insert(key.into(), v.clone());
        }
    }
    let styles: Styles = serde_json::from_value(Value::Object(looks)).unwrap_or_else(|e| panic!("case {name:?}: its colours and styles do not read: {e}"));
    Scene::new(nodes).with_styles(styles)
}

/// What the PC computes for `case`, each section in the order of the case's nodes: every placed
/// box, every group's origin, every wrap a column gave a hugging text, every scrolling frame's
/// content, every line's points, every polygon's corners, and everything each node paints.
fn computed(case: &Value) -> Vec<(&'static str, Vec<(String, Value)>)> {
    let scene = scene_of(case);
    let page = case["page"].as_str().expect("a case names its page");
    let placed: Placed = place(&scene, page, &Mono);
    let ids: Vec<String> = case["nodes"].as_array().into_iter().flatten().filter_map(|n| n["id"].as_str()).map(str::to_string).collect();
    let each = |f: &dyn Fn(&str) -> Option<Value>| -> Vec<(String, Value)> { ids.iter().filter_map(|id| Some((id.clone(), f(id)?))).collect() };
    let kind = |id: &str| scene.node(id).map(|n| n.kind);
    vec![
        ("rects", each(&|id| placed.rect(id).map(|r| rect(&r)))),
        ("origins", each(&|id| placed.origins.get(id).copied().map(point))),
        ("wraps", each(&|id| placed.wraps.get(id).copied().map(num))),
        ("scrolls", each(&|id| placed.scrolls.get(id).map(|s| rect(&s.content)))),
        (
            "lines",
            each(&|id| {
                placed.rect(id)?;
                let n = scene.node(id).filter(|n| n.kind == NodeKind::Line)?;
                let (pts, curved) = line_points(&scene, &placed, n);
                Some(json!({"points": pts.into_iter().map(point).collect::<Vec<_>>(), "curved": curved}))
            }),
        ),
        (
            "polygons",
            each(&|id| {
                let r = placed.rect(id).filter(|_| kind(id) == Some(NodeKind::Polygon))?;
                Some(Value::Array(node_polygon(scene.node(id)?, &r).into_iter().map(point).collect()))
            }),
        ),
        (
            "bounds",
            each(&|id| {
                placed.rect(id)?;
                Some(painted_bounds(&scene, &placed, &Mono, id).map_or(Value::Null, |b| rect(&b)))
            }),
        ),
    ]
}

/// Whether two values agree: numbers within [`TOLERANCE`], lists and objects entry by entry.
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x.as_f64().unwrap_or(f64::NAN) - y.as_f64().unwrap_or(f64::NAN)).abs() <= TOLERANCE,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| close(x, y)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| close(v, w))),
        _ => a == b,
    }
}

/// A node on one line, its id, parent and kind first.
fn node_line(n: &Value) -> String {
    let Some(o) = n.as_object() else {
        return n.to_string();
    };
    let mut ordered = Map::new();
    for key in ["id", "parent", "kind"] {
        if let Some(v) = o.get(key) {
            ordered.insert(key.into(), v.clone());
        }
    }
    for (k, v) in o {
        ordered.entry(k.clone()).or_insert_with(|| v.clone());
    }
    Value::Object(ordered).to_string()
}

/// A case as the file holds it: its name, page, colours, styles and nodes as written, then each
/// section the PC computes, one entry a line.
fn case_text(case: &Value, sections: &[(&'static str, Vec<(String, Value)>)]) -> String {
    let mut parts = vec![format!("      \"name\": {}", case["name"]), format!("      \"page\": {}", case["page"])];
    for key in ["colors", "styles"] {
        if let Some(v) = case.get(key) {
            parts.push(format!("      {}: {v}", json!(key)));
        }
    }
    let nodes: Vec<String> = case["nodes"].as_array().into_iter().flatten().map(|n| format!("        {}", node_line(n))).collect();
    parts.push(format!("      \"nodes\": [\n{}\n      ]", nodes.join(",\n")));
    for (name, entries) in sections {
        if entries.is_empty() {
            continue;
        }
        let lines: Vec<String> = entries.iter().map(|(id, v)| format!("        {}: {v}", json!(id))).collect();
        parts.push(format!("      {}: {{\n{}\n      }}", json!(name), lines.join(",\n")));
    }
    format!("    {{\n{}\n    }}", parts.join(",\n"))
}

#[test]
fn the_phone_layout_cases_hold_here() {
    let file = cases();
    let all = file["cases"].as_array().unwrap_or_else(|| panic!("{CASES_PATH} has no cases"));
    assert!(!all.is_empty(), "{CASES_PATH} has no cases");
    let mut texts = Vec::new();
    let mut wrong = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for case in all {
        let name = case["name"].as_str().unwrap_or_else(|| panic!("a case of {CASES_PATH} has no name"));
        assert!(names.insert(name), "two cases are named {name:?}");
        let sections = computed(case);
        let differ: Vec<&str> = sections
            .iter()
            .filter(|(section, entries)| {
                let want = case.get(*section).cloned().unwrap_or_else(|| json!({}));
                let got = Value::Object(entries.iter().cloned().collect());
                !close(&want, &got)
            })
            .map(|(section, _)| *section)
            .collect();
        let text = case_text(case, &sections);
        if !differ.is_empty() {
            wrong.push(format!("case {name:?} differs in {}; the PC computes it as\n{text}", differ.join(", ")));
        }
        texts.push(text);
    }
    if std::env::var("PHONE_FIXTURES").as_deref() == Ok("write") {
        let text = format!("{{\n  \"measure\": {},\n  \"cases\": [\n{}\n  ]\n}}\n", json!(MEASURE), texts.join(",\n"));
        std::fs::write(root().join(CASES_PATH), text).expect("write the layout cases");
        return;
    }
    assert_eq!(file["measure"], json!(MEASURE), "{CASES_PATH} says how its cases are measured");
    assert!(
        wrong.is_empty(),
        "{} of {CASES_PATH}'s cases are not what the PC lays out (PHONE_FIXTURES=write cargo test -p relay-client --test phone_fixtures writes them; then make the phone's port agree):\n\n{}",
        wrong.len(),
        wrong.join("\n\n")
    );
}

/// Every section a case holds is one this test computes, so a misspelt one is not skipped.
#[test]
fn the_phone_layout_cases_hold_only_what_is_checked() {
    for case in cases()["cases"].as_array().into_iter().flatten() {
        for key in case.as_object().into_iter().flat_map(Map::keys) {
            let known = ["name", "page", "colors", "styles", "nodes"].contains(&key.as_str()) || SECTIONS.contains(&key.as_str());
            assert!(known, "case {} holds {key:?}, which nothing checks", case["name"]);
        }
    }
}

/// The phone draws every icon the PC draws, with the same path.
#[test]
fn the_phone_has_every_icon() {
    let want: Vec<String> = ICONS.iter().map(|name| format!("  {}: {}", json!(name), json!(icons::path(name).expect("every icon has a path")))).collect();
    let text = format!("{{\n{}\n}}\n", want.join(",\n"));
    if std::env::var("PHONE_FIXTURES").as_deref() == Ok("write") {
        std::fs::write(root().join(ICONS_PATH), &text).expect("write the icons");
        return;
    }
    let got: Map<String, Value> = serde_json::from_str(ICON_PATHS).unwrap_or_else(|e| panic!("{ICONS_PATH} is not a JSON object: {e}"));
    let names: Vec<&str> = got.keys().map(String::as_str).collect();
    assert_eq!(names, ICONS, "{ICONS_PATH} names the PC's icons, in their order");
    for name in ICONS {
        assert_eq!(got[*name].as_str(), icons::path(name), "{name}'s path differs on the phone");
    }
}
