//! The phone's copy of a `design.get` reply and of the style table it resolves looks with
//! (apps/relay-android/core/src/test/resources/design/), which the phone's DesignModelTest and
//! DesignLookTest read. These tests keep the reply one this engine gives, holding every kind of
//! node, every field and every value of the enums that change how a node is placed or drawn, and
//! keep the style table the engine's. A field, kind or value added here fails until the fixture
//! uses it; changing the fixture runs the phone's tests, which fail until the phone reads it
//! (feature-phone §4.1, W45's phone column).

use relay_bus::design::{field_kinds, parse_color, Node, NodeKind, Styles, MAX_DEPTH, STYLE_FIELDS};
use relay_bus::ops::design::GetOut;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const FIXTURE: &str = "apps/relay-android/core/src/test/resources/design/get-reply.json";
const REPLY: &str = include_str!("../../../apps/relay-android/core/src/test/resources/design/get-reply.json");
const STYLE_KINDS: &str = include_str!("../../../apps/relay-android/core/src/test/resources/design/style-kinds.json");

/// What a missing field or value asks of whoever added it.
fn ask(what: &str) -> String {
    format!("add a node using {what} to {FIXTURE} and say how the phone draws it (docs/ANDROID.md, Design)")
}

fn json() -> Value {
    serde_json::from_str(REPLY).expect("get-reply.json is JSON")
}

fn reply() -> GetOut {
    serde_json::from_str(REPLY).unwrap_or_else(|e| panic!("{FIXTURE} is not a design.get reply: {e}"))
}

/// The schema agents read, whose definitions name every field and value.
fn defs() -> Value {
    let schema: Value = serde_json::from_str(&relay_bus::schema::render_pretty()).expect("the schema is JSON");
    schema["$defs"].clone()
}

/// The properties definition `name` has.
fn properties(defs: &Value, name: &str) -> BTreeSet<String> {
    defs[name]["properties"].as_object().unwrap_or_else(|| panic!("no definition {name} with properties")).keys().cloned().collect()
}

/// The values enum definition `name` allows: its `enum`, or the `const` of each `oneOf`.
fn variants(defs: &Value, name: &str) -> BTreeSet<String> {
    let def = &defs[name];
    let listed = def["enum"].as_array().into_iter().flatten();
    let consts = def["oneOf"].as_array().into_iter().flatten().map(|v| &v["const"]);
    let out: BTreeSet<String> = listed.chain(consts).filter_map(Value::as_str).map(str::to_string).collect();
    assert!(!out.is_empty(), "no enum definition {name}");
    out
}

/// The objects held under `key` by each of `holders`: the value itself, or each object of a list.
fn under<'a>(holders: &[&'a Value], key: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    for holder in holders {
        match holder.get(key) {
            Some(v @ Value::Object(_)) => out.push(v),
            Some(Value::Array(items)) => out.extend(items.iter().filter(|v| v.is_object())),
            _ => {}
        }
    }
    out
}

/// The string values `key` holds in `holders`.
fn values(holders: &[&Value], key: &str) -> BTreeSet<String> {
    holders.iter().filter_map(|h| h.get(key)?.as_str()).map(str::to_string).collect()
}

/// The keys `objects` use between them.
fn keys(objects: &[&Value]) -> BTreeSet<String> {
    objects.iter().filter_map(|o| o.as_object()).flat_map(Map::keys).cloned().collect()
}

#[test]
fn the_phone_reply_reads_whole_as_this_engine_reads_it() {
    let raw = json();
    let got = reply();
    let written = raw["nodes"].as_array().expect("nodes").len();
    assert_eq!(got.nodes.len(), written, "a node of {FIXTURE} did not read (design.get leaves out what does not)");
    for n in &got.nodes {
        assert!(n.extra.is_empty(), "node {:?}: {:?} did not read; the fixture holds only what this build reads", n.id, n.extra.keys().collect::<Vec<_>>());
    }
    for (name, style) in &got.styles {
        assert!(style.extra.is_empty(), "style {name:?}: {:?} did not read", style.extra.keys().collect::<Vec<_>>());
    }
    let pages = got.nodes.iter().filter(|n| n.kind == NodeKind::Page).count();
    assert_eq!(got.design.pages, pages as i64, "the design row's pages");
    assert_eq!(got.design.nodes, got.nodes.len() as i64, "the design row's nodes (pages included)");
}

#[test]
fn the_phone_reply_has_every_kind_field_and_value() {
    let defs = defs();
    let raw = json();
    let nodes: Vec<&Value> = raw["nodes"].as_array().expect("nodes").iter().collect();
    let styles: Vec<&Value> = raw["styles"].as_object().expect("styles").values().collect();

    let kinds = values(&nodes, "kind");
    for kind in NodeKind::ALL {
        assert!(kinds.contains(kind.name()), "{}", ask(&format!("kind `{}`", kind.name())));
    }

    // Every field of a node, but the names an add also reads them by.
    let used = keys(&nodes);
    for field in properties(&defs, "DesignNode") {
        if field != "width" && field != "height" {
            assert!(used.contains(&field), "{}", ask(&format!("`{field}`")));
        }
    }

    // Every field of what a node or a style holds.
    let holders: Vec<&Value> = nodes.iter().chain(&styles).copied().collect();
    let shadows: Vec<&Value> = under(&holders, "shadow").into_iter().chain(under(&holders, "shadows")).collect();
    let gradients: Vec<&Value> = under(&holders, "gradient").into_iter().chain(under(&holders, "stroke_gradient")).collect();
    let motions = under(&nodes, "motion");
    let nested = [
        ("DesignShadow", "shadow", shadows.clone()),
        ("DesignGradient", "gradient", gradients.clone()),
        ("DesignGradientStop", "gradient.stops", under(&gradients, "stops")),
        ("DesignArc", "arc", under(&nodes, "arc")),
        ("DesignTextRun", "runs", under(&nodes, "runs")),
        ("DesignMotion", "motion", motions.clone()),
        ("DesignKey", "motion.keys", under(&motions, "keys")),
        ("DesignTimeline", "timeline", under(&nodes, "timeline")),
        ("DesignStyle", "a style", styles.clone()),
    ];
    for (def, path, objects) in nested {
        let used = keys(&objects);
        for field in properties(&defs, def) {
            assert!(used.contains(&field), "{}", ask(&format!("`{path}` with `{field}`")));
        }
    }

    // Every value of the enums that place or paint a node.
    let arcs = under(&nodes, "arc");
    let enums: [(&str, &str, BTreeSet<String>); 15] = [
        ("DesignNodeKind", "kind", kinds.clone()),
        ("DesignLayout", "layout", values(&nodes, "layout")),
        ("DesignSizing", "w_sizing or h_sizing", &values(&nodes, "w_sizing") | &values(&nodes, "h_sizing")),
        ("DesignJustify", "justify", values(&nodes, "justify")),
        ("DesignItems", "items", values(&nodes, "items")),
        ("DesignTextAlign", "align", values(&nodes, "align")),
        ("DesignImageFit", "fit", values(&nodes, "fit")),
        ("DesignRoute", "route", values(&nodes, "route")),
        ("DesignPinX", "pin_x", values(&nodes, "pin_x")),
        ("DesignPinY", "pin_y", values(&nodes, "pin_y")),
        ("DesignScroll", "scroll", values(&nodes, "scroll")),
        ("DesignStrokeCap", "stroke_cap or arc.cap", &values(&nodes, "stroke_cap") | &values(&arcs, "cap")),
        ("DesignStrokeJoin", "stroke_join", values(&nodes, "stroke_join")),
        ("DesignTextCase", "text_case", values(&nodes, "text_case")),
        ("DesignGradientKind", "a gradient's kind", values(&gradients, "kind")),
    ];
    for (def, field, used) in enums {
        for value in variants(&defs, def) {
            assert!(used.contains(&value), "{}", ask(&format!("{field} `{value}`")));
        }
    }
}

#[test]
fn the_phone_reply_is_a_design_the_engine_takes() {
    let got = reply();
    let names = Styles { colors: got.colors.clone(), styles: got.styles.clone() };
    for (name, value) in &got.colors {
        assert!(parse_color(value).is_some(), "colour {name:?}: {value:?} is not a literal colour");
    }
    for (name, style) in &got.styles {
        style.validate().unwrap_or_else(|e| panic!("style {name:?}: {e}"));
        for (field, colour) in style.names() {
            assert!(got.colors.contains_key(colour), "style {name:?}: {field} names ${colour}, which the design does not hold");
        }
    }
    // Each node as an agent's strict add would write it, its names held by the design.
    let by_id: HashMap<&str, &Node> = got.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    assert_eq!(by_id.len(), got.nodes.len(), "every id once");
    let mut seen = BTreeSet::new();
    for n in &got.nodes {
        n.validate().unwrap_or_else(|e| panic!("node {:?}: {e}", n.id));
        n.check_new().unwrap_or_else(|e| panic!("node {:?}: {e}", n.id));
        names.check(n).unwrap_or_else(|e| panic!("node {:?}: {e}", n.id));
        // A free line or path holds its points from 0,0, as the engine stores one (B3).
        let mut contained = n.clone();
        assert!(!contained.contain_points(), "node {:?}: its points do not start at 0,0 and fill its box", n.id);
        if let Some(parent) = &n.parent {
            let p = by_id.get(parent.as_str()).unwrap_or_else(|| panic!("node {:?}: no parent {parent:?}", n.id));
            assert!(p.kind.holds_children() && seen.contains(parent.as_str()), "node {:?}: its parent comes first and holds children", n.id);
        }
        seen.insert(n.id.as_str());
    }
    // Pages first, then each node after its parent, depth first, siblings bottom to top: the
    // order design.get answers in.
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in &got.nodes {
        if let Some(parent) = &n.parent {
            children.entry(parent.as_str()).or_default().push(&n.id);
        }
    }
    let pages: Vec<&str> = got.nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.id.as_str()).collect();
    let mut order = pages.clone();
    let mut depth: HashMap<&str, usize> = pages.iter().map(|p| (*p, 0)).collect();
    for page in &pages {
        let mut stack: Vec<&str> = children.get(page).into_iter().flatten().rev().copied().collect();
        while let Some(id) = stack.pop() {
            order.push(id);
            let d = depth[by_id[id].parent.as_deref().unwrap_or_default()] + 1;
            assert!(d <= MAX_DEPTH, "node {id:?} is {d} deep");
            depth.insert(id, d);
            stack.extend(children.get(id).into_iter().flatten().rev().copied());
        }
    }
    let given: Vec<&str> = got.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(given, order, "the nodes in design.get's order");
    // A connector's ends are nodes on its page, neither itself nor a page.
    let page_of = |id: &str| {
        let mut at = by_id[id];
        while let Some(parent) = &at.parent {
            at = by_id[parent.as_str()];
        }
        at.id.clone()
    };
    for n in &got.nodes {
        for end in [&n.from, &n.to].into_iter().flatten() {
            let target = by_id.get(end.as_str()).unwrap_or_else(|| panic!("node {:?}: no end {end:?}", n.id));
            assert!(target.kind != NodeKind::Page && *end != n.id && page_of(end) == page_of(&n.id), "node {:?}: end {end:?}", n.id);
        }
    }
    for src in got.credits.keys() {
        assert!(got.nodes.iter().any(|n| n.src.as_deref() == Some(src)), "credit for {src}, which no image shows");
    }
}

#[test]
fn the_phone_lays_a_style_on_the_kinds_the_engine_does() {
    let written: BTreeMap<String, Vec<String>> = serde_json::from_str(STYLE_KINDS).expect("style-kinds.json is a map of lists");
    let expected: BTreeMap<String, Vec<String>> = STYLE_FIELDS
        .iter()
        .map(|field| {
            let kinds = field_kinds(field).unwrap_or_else(|| panic!("{field} has no kinds row"));
            (field.to_string(), kinds.iter().map(|k| k.name().to_string()).collect())
        })
        .collect();
    if written != expected {
        let lines: Vec<String> = STYLE_FIELDS
            .iter()
            .map(|f| format!("  {:?}: {}", f, serde_json::to_string(&expected[*f]).unwrap_or_default().replace(',', ", ")))
            .collect();
        panic!(
            "apps/relay-android/core/src/test/resources/design/style-kinds.json is not fields.rs's table; it should read\n{{\n{}\n}}\nand Look.kt's STYLE_KINDS with it",
            lines.join(",\n")
        );
    }
}

/// B57: the phone draws a star without `sides` with the 5 points the PC draws, as the field's doc
/// says; DesignLookTest reads the same doc from schema/bus.v1.json.
#[test]
fn a_star_without_sides_has_the_points_its_doc_names() {
    let sides = defs()["DesignNode"]["properties"]["sides"]["description"].as_str().unwrap_or_default().to_string();
    assert!(sides.contains("default 3, or 5 points for a star"), "the sides doc the phone holds its default to: {sides}");
    let star = reply().nodes.into_iter().find(|n| n.kind == NodeKind::Polygon && n.star.is_some() && n.sides.is_none());
    assert!(star.is_some(), "{}", ask("a polygon with `star` and no `sides`"));
}
