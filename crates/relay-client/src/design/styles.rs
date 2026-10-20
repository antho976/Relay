//! The edits behind the inspector's Style section and its named swatches: make a style from a
//! layer, push a layer's own look into its style, put a layer back to its style, let go of a
//! style, and name a colour. Each builder answers one list, so each is one undo step, and reads
//! the layers as stored ([`Scene::stored`]), so the colour and style names they hold survive. The
//! scene draws each edit as the engine applies it (`relay_bus::design::Styles`).

use super::scene::Scene;
use relay_bus::design::{parse_color, takes, Node, NodeOp, Removed, Style, Styles};
use serde_json::{json, Map, Value};

/// The field that counts as one with `field` when a style lies under a node, as
/// `Style::lay_under` pairs them: a node's own `fill` keeps its style's `gradient` off it, its
/// `radius` the style's `radii`, its `shadow` the style's `shadows`, its `stroke` the style's
/// `stroke_gradient`, and the other way round.
fn partner(field: &str) -> Option<&'static str> {
    Some(match field {
        "fill" => "gradient",
        "gradient" => "fill",
        "radius" => "radii",
        "radii" => "radius",
        "shadow" => "shadows",
        "shadows" => "shadow",
        "stroke" => "stroke_gradient",
        "stroke_gradient" => "stroke",
        _ => return None,
    })
}

/// The style fields `node` sets itself that its kind uses, as a `style` edit's `set`.
fn own(node: &Node) -> Map<String, Value> {
    let mut set = Style::of(node).to_set();
    set.retain(|field, _| takes(node.kind, field));
    set
}

/// An update's `set` taking each of `fields` off the node.
fn cleared<'a>(fields: impl IntoIterator<Item = &'a String>) -> Map<String, Value> {
    fields.into_iter().map(|field| (field.clone(), Value::Null)).collect()
}

/// Whether the design already has a colour or style called `name`.
fn taken(scene: &Scene, name: &str) -> bool {
    scene.styles().colors.contains_key(name) || scene.styles().styles.contains_key(name)
}

/// Make style `name` from layer `id`'s look, and give the layer that style: the style holds each
/// style field the layer draws with that its kind uses (its own, and what its present style lays
/// under it; `$name` colours kept as names), and the layer's own are taken off, so it looks as it
/// did. Nothing for a page, or when the design already has a colour or style called `name` (the
/// inspector says so: the edit would change that one).
pub fn new_style(scene: &Scene, id: &str, name: &str) -> Vec<NodeOp> {
    let Some(node) = scene.stored(id).filter(|n| takes(n.kind, "style")) else { return Vec::new() };
    if taken(scene, name) {
        return Vec::new();
    }
    let mut look = node.clone();
    if let Some(style) = node.style.as_deref().and_then(|s| scene.styles().styles.get(s)) {
        style.lay_under(&mut look);
    }
    let mut set = cleared(own(node).keys());
    set.insert("style".into(), json!(name));
    vec![NodeOp::Style { name: name.into(), set: own(&look) }, NodeOp::Update { id: id.into(), set }]
}

/// Update layer `id`'s style from it: the style takes the layer's own style fields (and drops
/// what each of them kept off it: a style's `gradient` under the layer's own `fill`), and the
/// layer's own are taken off, so it looks as it did and every other user of the style follows.
/// Nothing when the layer has no style, or sets none of the fields itself.
pub fn update_style(scene: &Scene, id: &str) -> Vec<NodeOp> {
    let Some(node) = scene.stored(id) else { return Vec::new() };
    let Some((name, style)) = node.style.as_deref().and_then(|s| scene.styles().styles.get_key_value(s)) else {
        return Vec::new();
    };
    let own = own(node);
    if own.is_empty() {
        return Vec::new();
    }
    let held = style.to_set();
    let mut set = own.clone();
    for field in own.keys() {
        if let Some(other) = partner(field).filter(|other| held.contains_key(*other) && !own.contains_key(*other)) {
            set.insert(other.into(), Value::Null);
        }
    }
    vec![NodeOp::Style { name: name.clone(), set }, NodeOp::Update { id: id.into(), set: cleared(own.keys()) }]
}

/// Put each of `ids` back to its style: the style fields it sets itself that its style sets too
/// (or keeps off, as a `fill` keeps a style's `gradient`) are taken off it. What it sets that its
/// style does not, it keeps. Layers without a style, or with nothing over it, are left out.
pub fn reset_to_style(scene: &Scene, ids: &[String]) -> Vec<NodeOp> {
    ids.iter()
        .filter_map(|id| {
            let node = scene.stored(id)?;
            let held = node.style.as_deref().and_then(|s| scene.styles().styles.get(s))?.to_set();
            let over = own(node).into_iter().map(|(field, _)| field).filter(|field| {
                held.contains_key(field) || partner(field).is_some_and(|other| held.contains_key(other))
            });
            let set: Map<String, Value> = over.map(|field| (field, Value::Null)).collect();
            (!set.is_empty()).then(|| NodeOp::Update { id: id.clone(), set })
        })
        .collect()
}

/// Let each of `ids` go of its style and keep its look: `style: null`, and each field the style
/// laid under it written into it (`Styles::detach`; `$name` colours kept as names). A style the
/// design no longer has only goes. Layers without a style are left out.
pub fn detach(scene: &Scene, ids: &[String]) -> Vec<NodeOp> {
    ids.iter()
        .filter_map(|id| {
            let node = scene.stored(id)?;
            let name = node.style.as_deref()?;
            let set = match scene.styles().styles.get(name) {
                Some(style) => Styles::detach(node, name, &Removed::Style(style.clone()))?,
                None => Map::from_iter([("style".to_string(), Value::Null)]),
            };
            Some(NodeOp::Update { id: id.clone(), set })
        })
        .collect()
}

/// Name the colour `field` (`fill`, `stroke` or `color`) of the first of `ids` that draws one as
/// `name`, and point that field of each of `ids` whose kind uses it at `$name`. Nothing when
/// none of them draws a colour there, or the design already has a colour or style called `name`
/// (picking a named colour sets a field to it without naming anything).
pub fn name_color(scene: &Scene, ids: &[String], field: &str, name: &str) -> Vec<NodeOp> {
    let pick = |n: &Node| match field {
        "fill" => n.fill.clone(),
        "stroke" => n.stroke.clone(),
        "color" => n.color.clone(),
        _ => None,
    };
    let value = ids.iter().filter_map(|id| scene.node(id)).find_map(|n| pick(n).filter(|c| parse_color(c).is_some()));
    let Some(value) = value.filter(|_| !taken(scene, name)) else { return Vec::new() };
    let named = json!(format!("${name}"));
    let mut ops = vec![NodeOp::Color { name: name.into(), value }];
    ops.extend(
        ids.iter()
            .filter(|id| scene.stored(id).is_some_and(|n| takes(n.kind, field)))
            .map(|id| NodeOp::Update { id: id.clone(), set: Map::from_iter([(field.to_string(), named.clone())]) }),
    );
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::scene::tests::{node, sample, set, styled};
    use relay_bus::design::NodeKind;

    fn look(s: &Scene, id: &str) -> Node {
        let mut n = s.node(id).unwrap().clone();
        n.style = None;
        n
    }

    #[test]
    fn new_style_then_detach_leaves_the_layer_as_it_looked() {
        let mut s = styled();
        s.apply_all(&[NodeOp::Update { id: "d".into(), set: set(json!({"opacity": 0.5, "stroke": "#000000"})) }]).unwrap();
        let before = look(&s, "d");
        let ops = new_style(&s, "d", "dot");
        s.apply_all(&ops).unwrap();
        let d = s.stored("d").unwrap();
        assert_eq!((d.style.as_deref(), d.fill.as_deref(), d.opacity, d.stroke.as_deref()), (Some("dot"), None, None, None));
        assert_eq!(s.styles().styles["dot"].fill.as_deref(), Some("$accent"), "names stay names");
        assert_eq!(look(&s, "d"), before);
        assert_eq!(s.users("accent"), ["b", "d"]);
        s.apply_all(&detach(&s, &["d".into()])).unwrap();
        let d = s.stored("d").unwrap();
        assert_eq!((d.style.as_deref(), d.fill.as_deref(), d.opacity), (None, Some("$accent"), Some(0.5)));
        assert_eq!(look(&s, "d"), before);
        // A layer with a style already: the new one holds what that one gave it, and its own.
        s.apply_all(&[NodeOp::Update { id: "c".into(), set: set(json!({"font_weight": 700})) }]).unwrap();
        let before = look(&s, "c");
        s.apply_all(&new_style(&s, "c", "heading")).unwrap();
        let heading = &s.styles().styles["heading"];
        assert_eq!((heading.font_size, heading.color.as_deref(), heading.font_weight), (Some(32.0), Some("$ink"), Some(700)));
        assert_eq!(s.stored("c").unwrap().font_weight, None);
        assert_eq!(look(&s, "c"), before);
        // A name the design has, or a page, makes nothing.
        assert!(new_style(&s, "b", "card").is_empty() && new_style(&s, "b", "accent").is_empty() && new_style(&s, "p1", "x").is_empty());
    }

    #[test]
    fn update_and_reset_round_trip() {
        let mut s = styled();
        let mut b2 = node("b2", Some("a"), NodeKind::Rect, [80.0, 10.0, 50.0, 50.0]);
        b2.style = Some("card".into());
        let gradient = json!({"stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#ffffff"}]});
        s.apply_all(&[
            NodeOp::Add { node: b2, index: None },
            NodeOp::Style { name: "card".into(), set: set(json!({"fill": null, "gradient": gradient})) },
            NodeOp::Update { id: "b".into(), set: set(json!({"fill": "#00ff00", "radius": 3})) },
        ])
        .unwrap();
        // b's own fill keeps the style's gradient off it.
        assert_eq!((s.node("b").unwrap().fill.as_deref(), s.node("b").unwrap().gradient.is_some()), (Some("#00ff00"), false));
        let before = look(&s, "b");
        s.apply_all(&update_style(&s, "b")).unwrap();
        let card = &s.styles().styles["card"];
        assert_eq!((card.fill.as_deref(), card.radius, card.gradient.is_none()), (Some("#00ff00"), Some(3.0), true));
        assert_eq!((s.stored("b").unwrap().fill.as_deref(), s.stored("b").unwrap().radius), (None, None));
        assert_eq!(look(&s, "b"), before);
        let b2 = s.node("b2").unwrap();
        assert_eq!((b2.fill.as_deref(), b2.radius, b2.gradient.is_none()), (Some("#00ff00"), Some(3.0), true), "every user follows");
        assert!(update_style(&s, "b").is_empty(), "nothing of its own left to push");
        // Own fields over the style go back to it; one the style does not set stays.
        s.apply_all(&[NodeOp::Update { id: "b".into(), set: set(json!({"radius": 8, "opacity": 0.5, "gradient": gradient})) }]).unwrap();
        let ops = reset_to_style(&s, &["b".into(), "d".into(), "a".into()]);
        assert_eq!(ops.len(), 1, "{ops:?}");
        s.apply_all(&ops).unwrap();
        let b = s.node("b").unwrap();
        assert_eq!((b.radius, b.gradient.is_none(), b.fill.as_deref(), b.opacity), (Some(3.0), true, Some("#00ff00"), Some(0.5)));
        assert!(reset_to_style(&s, &["b".into()]).is_empty());
    }

    #[test]
    fn detach_of_a_style_gone_only_lets_it_go() {
        let mut s = sample();
        s.apply(&NodeOp::Update { id: "b".into(), set: set(json!({"style": "gone"})) }).unwrap();
        assert!(detach(&s, &["b".into()]).is_empty(), "a replay took the missing name out already");
        let mut nodes = s.to_nodes();
        nodes.iter_mut().find(|n| n.id == "b").unwrap().style = Some("gone".into());
        let s = Scene::new(nodes);
        let ops = detach(&s, &["b".into(), "c".into()]);
        assert_eq!(ops, [NodeOp::Update { id: "b".into(), set: set(json!({"style": null})) }]);
    }

    #[test]
    fn name_color_names_the_colour_and_points_the_selection_at_it() {
        let mut s = styled();
        let ids: Vec<String> = ["c", "b", "d"].map(String::from).to_vec();
        // c is a text: its fill is its background, which it does not draw here; b's is the first.
        let ops = name_color(&s, &ids, "fill", "brand");
        assert_eq!(ops[0], NodeOp::Color { name: "brand".into(), value: "#4C9DFF".into() });
        assert_eq!(ops.len(), 4, "a text takes a fill too: {ops:?}");
        s.apply_all(&ops).unwrap();
        assert_eq!(s.stored("b").unwrap().fill.as_deref(), Some("$brand"));
        assert_eq!(s.users("brand"), ["b", "c", "d"]);
        s.apply_all(&[NodeOp::Color { name: "brand".into(), value: "#ff0000".into() }]).unwrap();
        assert_eq!(s.node("d").unwrap().fill.as_deref(), Some("#ff0000"));
        // Only a kind that uses the field is pointed at it; no colour, a taken name or a field
        // that is no colour makes nothing.
        let ops = name_color(&s, &["c".into(), "a".into()], "color", "text");
        assert_eq!(ops.len(), 2, "{ops:?}");
        assert!(name_color(&s, &ids, "fill", "accent").is_empty());
        assert!(name_color(&s, &["a".into()], "stroke", "edge").is_empty());
        assert!(name_color(&s, &ids, "radius", "r").is_empty());
    }
}
