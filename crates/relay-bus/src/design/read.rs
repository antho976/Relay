//! Reading and writing whole nodes (W65, B61). A node another build stored or sent reads
//! whatever it holds: a field this build cannot read waits in `extra` and is written back as it
//! was, so a write-back never loses what a newer Relay put there. Every place a node is parsed
//! goes through [`Node::read`] (the derived `Deserialize` is its fast path) and every whole-JSON
//! form through [`Node::to_map`]; strictness lives in `check_new` / `check_update` alone.

use super::{Node, NodeKind};
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

impl Node {
    /// A node as another build stored or sent it. A key this build cannot read (unknown, or a
    /// value it does not understand, such as a newer gradient kind) is kept in `extra`. Fails only
    /// when the body is not an object or `kind` is missing or unknown.
    pub fn read(value: Value) -> Result<Node, String> {
        match Node::deserialize(&value) {
            Ok(node) => Ok(node),
            Err(_) => match value {
                Value::Object(map) => read_map(map),
                _ => Err("a node is a JSON object".into()),
            },
        }
    }

    /// [`Node::read`] from JSON text: a row's body.
    pub fn read_str(body: &str) -> Result<Node, String> {
        match serde_json::from_str::<Node>(body) {
            Ok(node) => Ok(node),
            Err(error) => match serde_json::from_str::<Value>(body) {
                Ok(Value::Object(map)) => read_map(map),
                Ok(_) => Err("a node is a JSON object".into()),
                Err(_) => Err(error.to_string()),
            },
        }
    }

    /// The node's JSON: its fields, then each `extra` key no field already wrote, so a field set
    /// after a read wins over a stale value of the same name and no key is written twice. The
    /// one whole-JSON form of a node (bodies, patches, undos, replies).
    pub fn to_map(&self) -> Map<String, Value> {
        let mut map = match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        for (key, value) in &self.extra {
            if !map.contains_key(key) {
                map.insert(key.clone(), value.clone());
            }
        }
        map
    }
}

/// The slow path of [`Node::read`]: each key read alone beside `kind`; the ones that do not read
/// go to `extra`.
fn read_map(mut map: Map<String, Value>) -> Result<Node, String> {
    let kind = match map.get("kind") {
        None => return Err("kind: a node needs one".into()),
        Some(kind) => kind.clone(),
    };
    NodeKind::deserialize(&kind).map_err(|e| format!("kind: {e}"))?;
    let mut unread = Map::new();
    // Both spellings of one size do not read together: the alias waits aside.
    for (field, alias) in [("w", "width"), ("h", "height")] {
        if map.contains_key(field) {
            if let Some(value) = map.remove(alias) {
                unread.insert(alias.to_string(), value);
            }
        }
    }
    let keys: Vec<String> = map.keys().filter(|k| *k != "kind").cloned().collect();
    for key in keys {
        let probe = Map::from_iter([("kind".to_string(), kind.clone()), (key.clone(), map[&key].clone())]);
        if Node::deserialize(&Value::Object(probe)).is_err() {
            if let Some(value) = map.remove(&key) {
                unread.insert(key, value);
            }
        }
    }
    let mut node = Node::deserialize(&Value::Object(map)).map_err(|e| e.to_string())?;
    node.extra.extend(unread);
    Ok(node)
}

/// Why `value` does not read as field `key` of a `kind` node, if it does not.
pub(super) fn probe(kind: NodeKind, key: &str, value: &Value) -> Result<(), String> {
    let probe = Map::from_iter([("kind".to_string(), Value::from(kind.name())), (key.to_string(), value.clone())]);
    Node::deserialize(&Value::Object(probe)).map(drop).map_err(|e| e.to_string())
}

/// `deserialize_with` for a node: [`Node::read`].
pub fn read_node<'de, D: Deserializer<'de>>(d: D) -> Result<Node, D::Error> {
    Node::read(Value::deserialize(d)?).map_err(de::Error::custom)
}

/// `deserialize_with` for a reply's list of nodes, each by [`Node::read`]. A node of a kind this
/// build does not know (a newer engine's) is left out, as the engine's `design.get` leaves out
/// a row it cannot read, so an older client still opens the design.
pub fn read_nodes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Node>, D::Error> {
    Ok(Vec::<Value>::deserialize(d)?.into_iter().filter_map(|v| Node::read(v).ok()).collect())
}

/// [`read_nodes`] for an optional list.
pub fn read_nodes_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Node>>, D::Error> {
    Ok(Option::<Vec<Value>>::deserialize(d)?.map(|list| list.into_iter().filter_map(|v| Node::read(v).ok()).collect()))
}

/// `serialize_with` for a node: [`Node::to_map`], or the derived form when it holds no `extra`.
pub fn write_node<S: Serializer>(node: &Node, s: S) -> Result<S::Ok, S::Error> {
    if node.extra.is_empty() {
        node.serialize(s)
    } else {
        node.to_map().serialize(s)
    }
}

/// `serialize_with` for a list of nodes, each by [`write_node`].
pub fn write_nodes<S: Serializer>(nodes: &[Node], s: S) -> Result<S::Ok, S::Error> {
    let mut seq = s.serialize_seq(Some(nodes.len()))?;
    for node in nodes {
        seq.serialize_element(&Written(node))?;
    }
    seq.end()
}

/// [`write_nodes`] for an optional list.
pub fn write_nodes_opt<S: Serializer>(nodes: &Option<Vec<Node>>, s: S) -> Result<S::Ok, S::Error> {
    match nodes {
        Some(nodes) => s.serialize_some(&All(nodes)),
        None => s.serialize_none(),
    }
}

struct Written<'a>(&'a Node);

impl Serialize for Written<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_node(self.0, s)
    }
}

struct All<'a>(&'a [Node]);

impl Serialize for All<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_nodes(self.0, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::NodeOp;
    use serde_json::json;

    fn spiral() -> Value {
        json!({"kind": "spiral", "stops": [{"at": 0, "color": "#fff"}, {"at": 1, "color": "#000"}]})
    }

    /// W65, B61: what a newer build wrote reads, waits in `extra` and is written back.
    #[test]
    fn fields_from_a_newer_build_pass_through() {
        let body = json!({"parent": "p", "kind": "rect", "x": 4, "w": 10, "h": 10, "future": 1, "gradient": spiral()}).to_string();
        let node = Node::read_str(&body).unwrap();
        assert_eq!((node.x, node.gradient.clone()), (4.0, None));
        assert_eq!(Value::Object(node.extra.clone()), json!({"future": 1, "gradient": spiral()}));
        // Written back unchanged.
        let written = serde_json::to_value(Written(&node)).unwrap();
        assert_eq!(written, json!({"parent": "p", "kind": "rect", "x": 4.0, "y": 0.0, "w": 10.0, "h": 10.0, "future": 1, "gradient": spiral()}));
        assert_eq!(Node::read(written).unwrap(), node);
        // An update keeps them, and its undo captures them.
        let set = json!({"x": 3}).as_object().unwrap().clone();
        let moved = node.patched(&set).unwrap();
        assert_eq!((moved.x, moved.extra.clone()), (3.0, node.extra.clone()));
        let replace = json!({"future": 2, "fill": "#fff"}).as_object().unwrap().clone();
        assert_eq!(Value::Object(node.before(&replace)), json!({"future": 1, "fill": null}));
        // A strict add refuses each by name.
        let mut unknown = node.clone();
        unknown.extra.remove("gradient");
        assert!(unknown.check_new().unwrap_err().starts_with("future: not a node field"));
        let mut kind = node.clone();
        kind.extra.remove("future");
        let why = kind.check_new().unwrap_err();
        assert!(why.starts_with("gradient: unknown variant `spiral`"), "{why}");
        // A body of a kind this build does not know, or with none, or not an object, is an error.
        assert!(Node::read_str(r#"{"kind": "widget", "x": 1}"#).unwrap_err().starts_with("kind: unknown variant `widget`"));
        assert!(Node::read(json!({"x": 1})).is_err());
        assert!(Node::read(json!([1, 2])).is_err());
        assert!(Node::read_str("{not json").is_err());
        // The undo replay path: an add holding the spiral reads as an edit, and writes it back.
        let add = json!({"op": "add", "node": {"id": "r", "parent": "p", "kind": "rect", "gradient": spiral()}});
        let op: NodeOp = serde_json::from_value(add.clone()).unwrap();
        let NodeOp::Add { node, .. } = &op else { panic!() };
        assert_eq!(node.extra.get("gradient"), Some(&spiral()));
        assert_eq!(serde_json::to_value(&op).unwrap()["node"]["gradient"], spiral());
    }

    /// A field set after a read wins over a stale value of the same name waiting in `extra`.
    #[test]
    fn node_serialization_never_repeats_a_key() {
        let mut node = Node::read(json!({"kind": "path", "parent": "p", "points": "not a list"})).unwrap();
        assert_eq!(node.extra.get("points"), Some(&json!("not a list")));
        node.points = Some(vec![[1.0, 2.0]]);
        let text = serde_json::to_string(&Written(&node)).unwrap();
        assert_eq!(text.matches("\"points\"").count(), 1, "{text}");
        assert_eq!(node.to_map()["points"], json!([[1.0, 2.0]]));
        assert_eq!(Node::read_str(&text).unwrap().points, Some(vec![[1.0, 2.0]]));
        // The derived form leaves `extra` out entirely.
        assert!(!serde_json::to_string(&node).unwrap().contains("not a list"));
    }

    /// The `width`/`height` spellings still read beside the passthrough; both of one size at once
    /// are refused by name.
    #[test]
    fn aliases_still_read_with_the_passthrough() {
        let node = Node::read(json!({"kind": "frame", "parent": "p", "width": 320, "height": 200})).unwrap();
        assert_eq!((node.w, node.h, node.extra.len()), (320.0, 200.0, 0));
        let both = Node::read(json!({"kind": "frame", "parent": "p", "w": 10, "width": 320})).unwrap();
        assert_eq!((both.w, both.extra.get("width")), (10.0, Some(&json!(320))));
        assert_eq!(both.check_new().unwrap_err(), "w: give w or width, not both");
        let unread = Node::read(json!({"kind": "frame", "parent": "p", "width": "wide"})).unwrap();
        assert!(unread.check_new().unwrap_err().starts_with("width: invalid type"), "{:?}", unread.check_new());
    }

    #[test]
    fn lists_of_nodes_read_and_write_whole() {
        #[derive(Serialize, Deserialize)]
        struct Reply {
            #[serde(serialize_with = "write_nodes", deserialize_with = "read_nodes")]
            nodes: Vec<Node>,
            #[serde(default, serialize_with = "write_nodes_opt", deserialize_with = "read_nodes_opt")]
            more: Option<Vec<Node>>,
        }
        let raw = json!({"nodes": [{"kind": "page", "future": true}], "more": [{"kind": "rect", "parent": "p", "arc": "x"}]});
        let reply: Reply = serde_json::from_value(raw).unwrap();
        assert_eq!(reply.nodes[0].extra.get("future"), Some(&json!(true)));
        assert_eq!(reply.more.as_ref().unwrap()[0].extra.get("arc"), Some(&json!("x")));
        let back = serde_json::to_value(&reply).unwrap();
        assert_eq!(back["nodes"][0]["future"], true);
        assert_eq!(back["more"][0]["arc"], "x");
        let none: Reply = serde_json::from_value(json!({"nodes": [], "more": null})).unwrap();
        assert!(none.more.is_none());
        // A newer engine's kind is left out of a reply, not the reply.
        let newer: Reply = serde_json::from_value(json!({"nodes": [{"kind": "widget"}, {"kind": "page"}]})).unwrap();
        assert_eq!(newer.nodes.iter().map(|n| n.kind).collect::<Vec<_>>(), [NodeKind::Page]);
        // An edit's node is not optional: one this build cannot read fails the edit.
        assert!(serde_json::from_value::<NodeOp>(json!({"op": "add", "node": {"kind": "widget"}})).is_err());
    }
}
