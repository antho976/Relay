//! A design's nodes as a tree, and the edits of `design.apply` run against it the way the
//! engine runs them, so the canvas draws an edit at once and the engine's answer only confirms
//! it. Children are kept bottom to top, as they paint.

use relay_bus::design::{valid_id, Node, NodeKind, NodeOp, MAX_NODES};
use std::collections::HashMap;

/// The key pages are listed under in `children`.
const ROOT: &str = "";

#[derive(Debug, Clone, Default)]
pub struct Scene {
    nodes: HashMap<String, Node>,
    children: HashMap<String, Vec<String>>,
    /// Makes this client's new ids unlike another's: a few characters of the time it opened.
    seed: String,
    counter: u64,
}

fn base36(mut n: u64) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "page",
        NodeKind::Frame => "frame",
        NodeKind::Group => "group",
        NodeKind::Rect => "rect",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Line => "line",
        NodeKind::Text => "text",
        NodeKind::Sticky => "sticky",
        NodeKind::Path => "path",
        NodeKind::Image => "image",
        NodeKind::Polygon => "polygon",
        NodeKind::Icon => "icon",
        NodeKind::Vector => "vector",
        NodeKind::Comment => "comment",
    }
}

fn insert_at(list: &mut Vec<String>, id: String, index: Option<u32>) {
    match index.map(|i| i as usize) {
        Some(i) if i < list.len() => list.insert(i, id),
        _ => list.push(id),
    }
}

impl Scene {
    /// From `design.get`'s nodes: parents before children, siblings bottom to top.
    pub fn new(nodes: Vec<Node>) -> Scene {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let mut scene = Scene { seed: base36(seed % 36u64.pow(4)), ..Scene::default() };
        for node in nodes {
            let key = node.parent.clone().unwrap_or_default();
            scene.children.entry(key).or_default().push(node.id.clone());
            scene.nodes.insert(node.id.clone(), node);
        }
        scene
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// A node to change in place, for a gesture's preview: what the pointer is dragging shows
    /// where it would land, and the gesture puts it back before it sends the real edit.
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Add `node` without the checks of an edit, for a text being typed before it is sent;
    /// `remove` takes it out again.
    pub fn insert_preview(&mut self, node: Node) {
        let key = node.parent.clone().unwrap_or_default();
        self.children.entry(key).or_default().push(node.id.clone());
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn remove_preview(&mut self, id: &str) {
        if let Some(node) = self.nodes.remove(id) {
            if let Some(list) = self.children.get_mut(node.parent.as_deref().unwrap_or(ROOT)) {
                list.retain(|c| c != id);
            }
        }
    }

    pub fn pages(&self) -> &[String] {
        self.children.get(ROOT).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `id`'s children, bottom to top.
    pub fn children(&self, id: &str) -> &[String] {
        self.children.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn parent(&self, id: &str) -> Option<&str> {
        self.nodes.get(id)?.parent.as_deref()
    }

    /// `id`'s place among its siblings, 0 at the bottom.
    pub fn index(&self, id: &str) -> Option<usize> {
        let key = self.parent(id).unwrap_or(ROOT);
        self.children.get(key)?.iter().position(|c| c == id)
    }

    /// The page `id` is on (itself, for a page).
    pub fn page_of(&self, id: &str) -> Option<&str> {
        let mut at = self.nodes.get(id)?;
        while let Some(parent) = at.parent.as_deref() {
            at = self.nodes.get(parent)?;
        }
        Some(&at.id)
    }

    /// `id`'s ancestors, nearest first, up to and including its page.
    pub fn ancestors(&self, id: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut at = self.parent(id);
        while let Some(p) = at {
            out.push(p);
            at = self.parent(p);
        }
        out
    }

    /// Whether `id` is somewhere inside `ancestor`.
    pub fn is_inside(&self, id: &str, ancestor: &str) -> bool {
        self.ancestors(id).contains(&ancestor)
    }

    /// `id` and everything inside it, each parent before its children, siblings bottom to top.
    pub fn subtree(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(at) = stack.pop() {
            stack.extend(self.children(&at).iter().rev().cloned());
            out.push(at);
        }
        out
    }

    /// Every node, in `design.get`'s order.
    pub fn to_nodes(&self) -> Vec<Node> {
        let mut out: Vec<Node> = self.pages().iter().filter_map(|p| self.nodes.get(p).cloned()).collect();
        for page in self.pages() {
            out.extend(self.subtree(page).iter().skip(1).filter_map(|id| self.nodes.get(id).cloned()));
        }
        out
    }

    /// An id no node has, shaped like the engine's (`frame-…`) and unlike another client's.
    pub fn fresh_id(&mut self, kind: NodeKind) -> String {
        loop {
            self.counter += 1;
            let id = format!("{}-{}{}", kind_name(kind), self.seed, base36(self.counter));
            if !self.nodes.contains_key(&id) {
                return id;
            }
        }
    }

    /// An id no node has, starting `prefix` (an asset's name), unlike another client's.
    pub fn fresh_named(&mut self, prefix: &str) -> String {
        let prefix: String = prefix.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(24).collect();
        loop {
            self.counter += 1;
            let id = format!("{prefix}-{}{}", self.seed, base36(self.counter));
            if !self.nodes.contains_key(&id) {
                return id;
            }
        }
    }

    /// Run one edit as the engine does; on an error nothing changed.
    pub fn apply(&mut self, op: &NodeOp) -> Result<(), String> {
        match op {
            NodeOp::Add { node, index } => {
                if !valid_id(&node.id) {
                    return Err(format!("add: id {:?} is not a node id", node.id));
                }
                if self.nodes.contains_key(&node.id) {
                    return Err(format!("add: {} already exists", node.id));
                }
                if self.nodes.len() >= MAX_NODES {
                    return Err(format!("a design holds at most {MAX_NODES} nodes"));
                }
                node.validate()?;
                if let Some(parent) = &node.parent {
                    self.check_container(parent)?;
                }
                let key = node.parent.clone().unwrap_or_default();
                insert_at(self.children.entry(key).or_default(), node.id.clone(), *index);
                self.nodes.insert(node.id.clone(), node.clone());
            }
            NodeOp::Update { id, set } => {
                let node = self.nodes.get(id).ok_or_else(|| format!("update: no node {id}"))?;
                let mut patched = node.patched(set).map_err(|e| format!("update {id}: {e}"))?;
                // As the engine does: a copy edited by hand remembers what changed.
                if patched.master.is_some() && !set.contains_key("overridden") {
                    let mut own = patched.overridden.take().unwrap_or_default();
                    for field in relay_bus::design::overriding(set) {
                        if !own.iter().any(|f| f == field) {
                            own.push(field.to_string());
                        }
                    }
                    patched.overridden = (!own.is_empty()).then_some(own);
                }
                self.nodes.insert(id.clone(), patched);
            }
            NodeOp::Move { id, parent, index } => {
                let node = self.nodes.get(id).ok_or_else(|| format!("move: no node {id}"))?;
                let old = node.parent.clone();
                let new = if node.kind == NodeKind::Page {
                    if parent.is_some() {
                        return Err("move: a page has no parent".into());
                    }
                    None
                } else {
                    let to = parent.clone().or(old.clone()).unwrap_or_default();
                    self.check_container(&to)?;
                    if to == *id || self.is_inside(&to, id) {
                        return Err(format!("move: {to} is inside {id}"));
                    }
                    Some(to)
                };
                if let Some(list) = self.children.get_mut(old.as_deref().unwrap_or(ROOT)) {
                    list.retain(|c| c != id);
                }
                insert_at(self.children.entry(new.clone().unwrap_or_default()).or_default(), id.clone(), *index);
                if let Some(node) = self.nodes.get_mut(id) {
                    node.parent = new;
                }
            }
            NodeOp::Insert { item, parent, x, y, index, id, set } => {
                // The canvas always names the root, so its ids match the engine's.
                let root = id.as_deref().ok_or("insert: the canvas names the root")?;
                let nodes = match item.strip_prefix('#') {
                    Some(component) => {
                        let subtree: Vec<Node> = self.subtree(component).iter().filter_map(|i| self.nodes.get(i).cloned()).collect();
                        if subtree.first().is_none_or(|n| n.component != Some(true)) {
                            return Err(format!("insert: {component} is not a component"));
                        }
                        relay_bus::kits::instance(&subtree, root, parent, *x, *y, set.as_ref())?
                    }
                    None => {
                        let (_, found) = relay_bus::kits::item(item).ok_or_else(|| format!("insert: no pack item {item}"))?;
                        relay_bus::kits::expand(found, root, parent, *x, *y, set.as_ref())?
                    }
                };
                let mut trial = self.clone();
                for (i, node) in nodes.into_iter().enumerate() {
                    trial.apply(&NodeOp::Add { node, index: if i == 0 { *index } else { None } })?;
                }
                *self = trial;
            }
            NodeOp::Delete { id } => {
                let node = self.nodes.get(id).ok_or_else(|| format!("delete: no node {id}"))?;
                if node.kind == NodeKind::Page && self.pages().len() <= 1 {
                    return Err("a design keeps at least one page".into());
                }
                let key = node.parent.clone().unwrap_or_default();
                if let Some(list) = self.children.get_mut(&key) {
                    list.retain(|c| c != id);
                }
                for gone in self.subtree(id) {
                    self.children.remove(&gone);
                    self.nodes.remove(&gone);
                }
            }
        }
        Ok(())
    }

    /// Run every edit, all or none.
    pub fn apply_all(&mut self, ops: &[NodeOp]) -> Result<(), String> {
        let mut trial = self.clone();
        for (i, op) in ops.iter().enumerate() {
            trial.apply(op).map_err(|e| format!("ops[{i}]: {e}"))?;
        }
        *self = trial;
        Ok(())
    }

    /// Take the engine's copy of a node that an edit of ours changed. Its place stays: the
    /// edit put it there already.
    pub fn confirm(&mut self, node: Node) {
        if let Some(mine) = self.nodes.get_mut(&node.id) {
            if mine.parent == node.parent {
                *mine = node;
            }
        }
    }

    fn check_container(&self, id: &str) -> Result<(), String> {
        match self.nodes.get(id) {
            Some(p) if p.kind.holds_children() => Ok(()),
            Some(p) => Err(format!("a {} cannot hold other nodes", kind_name(p.kind))),
            None => Err(format!("no parent {id}")),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn node(id: &str, parent: Option<&str>, kind: NodeKind, rect: [f64; 4]) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        [n.x, n.y, n.w, n.h] = rect;
        if kind == NodeKind::Text {
            n.text = Some("Hi".into());
        }
        n
    }

    pub(crate) fn sample() -> Scene {
        Scene::new(vec![
            node("p1", None, NodeKind::Page, [0.0; 4]),
            node("a", Some("p1"), NodeKind::Frame, [0.0, 0.0, 400.0, 300.0]),
            node("b", Some("a"), NodeKind::Rect, [10.0, 10.0, 50.0, 50.0]),
            node("c", Some("a"), NodeKind::Text, [100.0, 10.0, 0.0, 0.0]),
            node("d", Some("p1"), NodeKind::Ellipse, [500.0, 0.0, 80.0, 80.0]),
        ])
    }

    #[test]
    fn edits_land_where_the_engine_puts_them() {
        let mut s = sample();
        let add = NodeOp::Add { node: node("e", Some("a"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]), index: Some(0) };
        s.apply(&add).unwrap();
        assert_eq!(s.children("a"), ["e", "b", "c"]);
        s.apply(&NodeOp::Move { id: "e".into(), parent: None, index: None }).unwrap();
        assert_eq!(s.children("a"), ["b", "c", "e"]);
        s.apply(&NodeOp::Move { id: "e".into(), parent: Some("p1".into()), index: Some(1) }).unwrap();
        assert_eq!((s.children("p1"), s.parent("e")), (&["a".to_string(), "e".into(), "d".into()][..], Some("p1")));
        let set = json!({"fill": "#ff0000"}).as_object().unwrap().clone();
        s.apply(&NodeOp::Update { id: "b".into(), set }).unwrap();
        assert_eq!(s.node("b").unwrap().fill.as_deref(), Some("#ff0000"));
        s.apply(&NodeOp::Delete { id: "a".into() }).unwrap();
        assert!(s.node("b").is_none() && s.node("c").is_none());
        assert_eq!(s.children("p1"), ["e", "d"]);
    }

    #[test]
    fn refuses_what_the_engine_refuses() {
        let mut s = sample();
        assert!(s.apply(&NodeOp::Move { id: "a".into(), parent: Some("b".into()), index: None }).is_err());
        s.apply(&NodeOp::Add { node: node("g", Some("a"), NodeKind::Group, [0.0; 4]), index: None }).unwrap();
        assert!(s.apply(&NodeOp::Move { id: "a".into(), parent: Some("g".into()), index: None }).unwrap_err().contains("inside"));
        assert!(s.apply(&NodeOp::Delete { id: "p1".into() }).is_err());
        assert!(s.apply(&NodeOp::Add { node: node("b", Some("a"), NodeKind::Rect, [0.0; 4]), index: None }).is_err());
        let before = s.to_nodes();
        let ops = [
            NodeOp::Delete { id: "d".into() },
            NodeOp::Update { id: "zz".into(), set: Default::default() },
        ];
        assert!(s.apply_all(&ops).unwrap_err().starts_with("ops[1]"));
        assert_eq!(s.to_nodes(), before);
    }

    #[test]
    fn order_and_ids() {
        let mut s = sample();
        assert_eq!(s.subtree("a"), ["a", "b", "c"]);
        assert_eq!(s.page_of("c"), Some("p1"));
        assert_eq!(s.ancestors("c"), ["a", "p1"]);
        let ids: Vec<String> = s.to_nodes().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, ["p1", "a", "b", "c", "d"]);
        let fresh = s.fresh_id(NodeKind::Frame);
        assert!(fresh.starts_with("frame-") && valid_id(&fresh) && s.node(&fresh).is_none());
        assert_ne!(fresh, s.fresh_id(NodeKind::Frame));
    }
}
