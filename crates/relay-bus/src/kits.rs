//! Asset packs: Relay's built-in libraries of ready design pieces (buttons, fields, cards, nav
//! bars, flowchart shapes, device frames) and whole-page templates, compiled in from `kits/*.json`
//! so the engine, the canvas and agents read one catalogue. A `design.apply` `insert` places an
//! item as ordinary nodes under fresh ids, every piece of it editable; `design.kits` lists them.

use crate::design::{valid_id, Node, MAX_ID};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DesignKit")]
pub struct Kit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub items: Vec<KitItem>,
}

/// One piece of a pack: its nodes, root first (local id `root`), with local ids the `insert`
/// edit maps to fresh ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DesignKitItem")]
pub struct KitItem {
    pub id: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// Local ids of the texts and icons worth changing, with what each is: the keys an
    /// `insert`'s `set` takes.
    #[serde(default)]
    pub slots: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
}

impl KitItem {
    /// The root's size, for a listing.
    pub fn size(&self) -> (f64, f64) {
        self.nodes.first().map_or((0.0, 0.0), |n| (n.w, n.h))
    }
}

const SOURCES: &[&str] = &[
    include_str!("../kits/relay-ui.json"),
    include_str!("../kits/wireframe.json"),
    include_str!("../kits/flowchart.json"),
    include_str!("../kits/devices.json"),
    include_str!("../kits/templates.json"),
];

/// Every pack, parsed once.
pub fn kits() -> &'static [Kit] {
    static KITS: OnceLock<Vec<Kit>> = OnceLock::new();
    KITS.get_or_init(|| SOURCES.iter().filter_map(|s| serde_json::from_str(s).ok()).collect())
}

/// An item by its path, `kit/item` (`relay-ui/button-primary`).
pub fn item(path: &str) -> Option<(&'static Kit, &'static KitItem)> {
    let (kit, item) = path.split_once('/')?;
    let kit = kits().iter().find(|k| k.id == kit)?;
    Some((kit, kit.items.iter().find(|i| i.id == item)?))
}

/// An item's node id under `root`: the root itself, else `root.local`.
pub fn mapped(root: &str, local: &str) -> String {
    if local == "root" {
        root.to_string()
    } else {
        format!("{root}.{local}")
    }
}

/// The item's nodes, ready to add: ids under `root`, the root in `parent` at `(x, y)`, and `set`
/// (`{local id: {field: value}}`) applied to the nodes it names. Each node is validated.
pub fn expand(item: &KitItem, root: &str, parent: &str, x: f64, y: f64, set: Option<&Map<String, Value>>) -> Result<Vec<Node>, String> {
    if !valid_id(root) {
        return Err(format!("id {root:?}: use 1 to {MAX_ID} letters, digits, '-', '_', '.' or ':'"));
    }
    if let Some(set) = set {
        if let Some(unknown) = set.keys().find(|k| !item.nodes.iter().any(|n| n.id == **k)) {
            let known: Vec<&str> = item.nodes.iter().map(|n| n.id.as_str()).collect();
            return Err(format!("set: {unknown:?} is not a part of {} (its parts: {})", item.id, known.join(", ")));
        }
    }
    let mut out = Vec::with_capacity(item.nodes.len());
    for (i, n) in item.nodes.iter().enumerate() {
        let mut node = n.clone();
        node.id = mapped(root, &n.id);
        if !valid_id(&node.id) {
            return Err(format!("id {root:?} is too long for {}'s parts: keep it under {} characters", item.id, MAX_ID - 12));
        }
        if i == 0 {
            node.parent = Some(parent.to_string());
            node.x = x;
            node.y = y;
        } else {
            node.parent = n.parent.as_deref().map(|p| mapped(root, p));
        }
        node.from = n.from.as_deref().map(|p| mapped(root, p));
        node.to = n.to.as_deref().map(|p| mapped(root, p));
        if let Some(patch) = set.and_then(|s| s.get(&n.id)) {
            let Value::Object(patch) = patch else {
                return Err(format!("set.{}: an object of fields", n.id));
            };
            node = node.patched(patch).map_err(|e| format!("set.{}: {e}", n.id))?;
        }
        node.validate().map_err(|e| format!("{}: {e}", n.id))?;
        out.push(node);
    }
    Ok(out)
}

/// An instance of a component: copies of the component's nodes (`subtree`, its root first,
/// parents before children) under `root`, each remembering the node it copies (`master`), the
/// root naming the component (`instance_of`). `set` overrides fields by component node id.
pub fn instance(subtree: &[Node], root: &str, parent: &str, x: f64, y: f64, set: Option<&Map<String, Value>>) -> Result<Vec<Node>, String> {
    let Some(component) = subtree.first() else {
        return Err("the component has no nodes".into());
    };
    if !valid_id(root) || root.len() > MAX_ID - 6 {
        return Err(format!("id {root:?}: use 1 to {} letters, digits, '-', '_', '.' or ':'", MAX_ID - 6));
    }
    let ids: BTreeMap<&str, String> = subtree
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), if i == 0 { root.to_string() } else { format!("{root}.{i}") }))
        .collect();
    let mut out = Vec::with_capacity(subtree.len());
    for (i, n) in subtree.iter().enumerate() {
        let mut node = n.clone();
        node.id = ids[n.id.as_str()].clone();
        node.master = Some(n.id.clone());
        node.component = None;
        node.overridden = None;
        if i == 0 {
            node.parent = Some(parent.to_string());
            node.instance_of = Some(component.id.clone());
            (node.x, node.y) = (x, y);
        } else {
            node.parent = n.parent.as_deref().and_then(|p| ids.get(p)).cloned();
            node.instance_of = n.instance_of.clone();
        }
        node.from = n.from.as_deref().map(|e| ids.get(e).cloned().unwrap_or_else(|| e.to_string()));
        node.to = n.to.as_deref().map(|e| ids.get(e).cloned().unwrap_or_else(|| e.to_string()));
        if let Some(Value::Object(patch)) = set.and_then(|s| s.get(&n.id)) {
            node = node.patched(patch).map_err(|e| format!("set.{}: {e}", n.id))?;
            let fields: Vec<String> = crate::design::overriding(patch).map(str::to_string).collect();
            node.overridden = (!fields.is_empty()).then_some(fields);
        }
        node.validate().map_err(|e| format!("{}: {e}", n.id))?;
        out.push(node);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::NodeKind;

    #[test]
    fn every_pack_parses_and_every_item_expands() {
        assert_eq!(kits().len(), SOURCES.len(), "a pack failed to parse");
        let mut count = 0;
        for kit in kits() {
            assert!(!kit.items.is_empty(), "{} is empty", kit.id);
            for item in &kit.items {
                let first = item.nodes.first().unwrap_or_else(|| panic!("{}/{} has no nodes", kit.id, item.id));
                assert_eq!((first.id.as_str(), first.parent.as_deref()), ("root", None), "{}/{}", kit.id, item.id);
                let nodes = expand(item, "x", "page-1", 10.0, 20.0, None).unwrap_or_else(|e| panic!("{}/{}: {e}", kit.id, item.id));
                let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
                for n in &nodes[1..] {
                    let parent = n.parent.as_deref().unwrap();
                    let at = ids.iter().position(|i| *i == parent).unwrap_or_else(|| panic!("{}/{}: {} has no parent {parent}", kit.id, item.id, n.id));
                    assert!(at < ids.iter().position(|i| *i == n.id).unwrap(), "parents come first");
                    assert!(nodes[at].kind.holds_children(), "{}/{}: {parent} cannot hold {}", kit.id, item.id, n.id);
                }
                for slot in item.slots.keys() {
                    let n = item.nodes.iter().find(|n| n.id == *slot).unwrap_or_else(|| panic!("{}/{}: slot {slot}", kit.id, item.id));
                    assert!(matches!(n.kind, NodeKind::Text | NodeKind::Icon | NodeKind::Sticky | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon), "{}/{}: slot {slot}", kit.id, item.id);
                }
                count += 1;
            }
        }
        assert!(count >= 60, "only {count} items");
    }

    #[test]
    fn insert_maps_ids_places_the_root_and_fills_slots() {
        let item = KitItem {
            id: "b".into(),
            name: "B".into(),
            category: "Buttons".into(),
            description: String::new(),
            slots: BTreeMap::new(),
            nodes: serde_json::from_value(serde_json::json!([
                {"id": "root", "kind": "frame", "w": 100, "h": 40},
                {"id": "label", "parent": "root", "kind": "text", "text": "Go"}
            ]))
            .unwrap(),
        };
        let set = serde_json::json!({"label": {"text": "Save"}}).as_object().unwrap().clone();
        let nodes = expand(&item, "save", "card", 5.0, 6.0, Some(&set)).unwrap();
        assert_eq!((nodes[0].id.as_str(), nodes[0].parent.as_deref(), nodes[0].x, nodes[0].y), ("save", Some("card"), 5.0, 6.0));
        assert_eq!((nodes[1].id.as_str(), nodes[1].parent.as_deref(), nodes[1].text.as_deref()), ("save.label", Some("save"), Some("Save")));
        let bad = serde_json::json!({"nope": {"text": "x"}}).as_object().unwrap().clone();
        assert!(expand(&item, "save", "card", 0.0, 0.0, Some(&bad)).unwrap_err().contains("its parts: root, label"));
    }
}
