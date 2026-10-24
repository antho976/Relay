//! A design's nodes as a tree, and the edits of `design.apply` run against it the way the
//! engine runs them, so the canvas draws an edit at once and the engine's answer only confirms
//! it. Children are kept bottom to top, as they paint.
//!
//! A node that names a colour or a style is held twice: as stored ([`Scene::stored`]: what the
//! engine keeps, what [`Scene::to_nodes`] gives and what every edit builder copies or compares
//! with) and as drawn ([`Scene::node`]: its style laid under it and each `$name` the colour it
//! names), which paint, layout, hit testing, snapping and export read. A node that names nothing
//! is the same both ways and held once. The design's colours and styles come with its nodes
//! (`Scene::new(nodes).with_styles(..)`), and the `color`, `style` and `delete_style` edits change
//! them through the rules the engine runs them by (`relay_bus::design::Styles`).

use relay_bus::design::{fit_written, valid_id, Layout, Node, NodeKind, NodeOp, Styles, MAX_DEPTH, MAX_NODES, MAX_OPS};
use serde_json::{Map, Value};
use rustc_hash::{FxHashMap, FxHashSet};

/// The key pages are listed under in `children`.
const ROOT: &str = "";

#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// Every node as drawn.
    nodes: FxHashMap<String, Node>,
    /// As stored, the nodes that name a colour or a style; any other is stored as it draws.
    stored: FxHashMap<String, Node>,
    /// The design's named colours and styles.
    styles: Styles,
    children: FxHashMap<String, Vec<String>>,
    /// The ids a lenient edit left out as too deep, and those a later one put inside them: a
    /// lenient edit naming one the scene does not hold, as its node or its parent, is skipped
    /// too, as the engine's list skips an edit whose node or parent is gone (W14).
    left_out: FxHashSet<String>,
    /// Makes this client's new ids unlike another's: eight random characters drawn when the
    /// scene was made.
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

/// Eight base-36 characters from the time and the process's hash keys, which the standard
/// library draws at random: two clients (or two scenes of one) start from different seeds.
fn seed() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(nanos);
    h.write_u32(std::process::id());
    let digits = base36(h.finish() % 36u64.pow(8));
    format!("{digits:0>8}")
}

fn insert_at(list: &mut Vec<String>, id: String, index: Option<u32>) {
    match index.map(|i| i as usize) {
        Some(i) if i < list.len() => list.insert(i, id),
        _ => list.push(id),
    }
}

/// Whether stored `node` names colour or style `name`, itself or through its style.
fn names(styles: &Styles, node: &Node, name: &str) -> bool {
    node.names().iter().any(|(_, used)| *used == name)
        || node.style.as_deref().and_then(|s| styles.styles.get(s)).is_some_and(|s| s.names().iter().any(|(_, used)| *used == name))
}

/// B44: the engine's refusal of a list longer than it takes, said before anything runs.
/// A frame to follow: its id, its stored size before and after, and how many levels down.
type Resize = (String, (f64, f64), (f64, f64), usize);

/// What a list sets that the follow rule reads (feature-layout §2.1).
#[derive(Default)]
struct Following {
    resized: Vec<(String, (f64, f64))>,
    set_here: FxHashSet<(String, bool)>,
}

impl Following {
    fn set_here(&mut self, id: &str, across: bool, down: bool) {
        if across {
            self.set_here.insert((id.to_string(), true));
        }
        if down {
            self.set_here.insert((id.to_string(), false));
        }
    }
}

fn too_many(n: usize) -> String {
    format!("that is {n} changes at once; one edit makes at most {MAX_OPS}. Select fewer layers.")
}

impl Scene {
    /// From `design.get`'s nodes: parents before children, siblings bottom to top. A design that
    /// names colours or styles gets them with [`Scene::with_styles`]; until then a node draws as
    /// if what it names were missing (the kind's default shows).
    pub fn new(nodes: Vec<Node>) -> Scene {
        let mut scene = Scene { seed: seed(), ..Scene::default() };
        scene.nodes.reserve(nodes.len());
        for node in nodes {
            let key = node.parent.clone().unwrap_or_default();
            scene.children.entry(key).or_default().push(node.id.clone());
            scene.put(node);
        }
        scene
    }

    /// With the design's colours and styles (`design.get`'s `colors` and `styles`).
    pub fn with_styles(mut self, styles: Styles) -> Scene {
        self.set_styles(styles);
        self
    }

    /// Take the design's colours and styles as an answer or a read gives them whole, and draw
    /// every node that names one again. The same ones leave the scene as it is.
    pub fn set_styles(&mut self, styles: Styles) {
        if self.styles != styles {
            self.styles = styles;
            self.restyle(None);
        }
    }

    /// The design's named colours and styles, as this scene holds them.
    pub fn styles(&self) -> &Styles {
        &self.styles
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// A node as it draws: its style laid under it and each `$name` the colour it names.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// A node as it is stored, colour and style names and all: what an edit copies, compares
    /// with or sends back.
    pub fn stored(&self, id: &str) -> Option<&Node> {
        self.stored.get(id).or_else(|| self.nodes.get(id))
    }

    /// A node to change in place, for a gesture's preview: what the pointer is dragging shows
    /// where it would land, and the gesture puts it back before it sends the real edit. It is the
    /// node as drawn: the edit that follows patches the stored node, and the drawn one is made
    /// again from it.
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Add `node` without the checks of an edit, for a text being typed before it is sent;
    /// `remove` takes it out again.
    pub fn insert_preview(&mut self, node: Node) {
        let key = node.parent.clone().unwrap_or_default();
        self.children.entry(key).or_default().push(node.id.clone());
        self.put(node);
    }

    pub fn remove_preview(&mut self, id: &str) {
        if let Some(node) = self.nodes.remove(id) {
            self.stored.remove(id);
            if let Some(list) = self.children.get_mut(node.parent.as_deref().unwrap_or(ROOT)) {
                list.retain(|c| c != id);
            }
        }
    }

    /// Hold `node` (as stored) and draw it. Its place among its siblings is the caller's.
    fn put(&mut self, node: Node) {
        if Styles::uses(&node) {
            self.nodes.insert(node.id.clone(), self.styles.resolve(&node));
            self.stored.insert(node.id.clone(), node);
        } else {
            if !self.stored.is_empty() {
                self.stored.remove(&node.id);
            }
            self.nodes.insert(node.id.clone(), node);
        }
    }

    /// Draw again each node that names `name` (every node that names anything, for `None`).
    fn restyle(&mut self, name: Option<&str>) {
        for (id, node) in &self.stored {
            if name.is_none_or(|name| names(&self.styles, node, name)) {
                if let Some(drawn) = self.nodes.get_mut(id) {
                    *drawn = self.styles.resolve(node);
                }
            }
        }
    }

    /// The nodes that name colour or style `name`, themselves or through their style, in
    /// `design.get`'s order: the pages first, then what is on each page.
    pub fn users(&self, name: &str) -> Vec<String> {
        let hits: FxHashSet<&str> =
            self.stored.iter().filter(|(_, n)| names(&self.styles, n, name)).map(|(id, _)| id.as_str()).collect();
        if hits.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<String> = self.pages().iter().filter(|p| hits.contains(p.as_str())).cloned().collect();
        for page in self.pages() {
            let mut stack: Vec<&str> = self.children(page).iter().rev().map(String::as_str).collect();
            while let Some(at) = stack.pop() {
                if hits.contains(at) {
                    out.push(at.to_string());
                }
                stack.extend(self.children(at).iter().rev().map(String::as_str));
            }
        }
        out
    }

    pub fn pages(&self) -> &[String] {
        self.children.get(ROOT).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The page a design opens on and shows as its picture: the first not hidden from the
    /// pages lists, or the first of all when every one is.
    pub fn first_page(&self) -> Option<&String> {
        let pages = self.pages();
        pages.iter().find(|p| self.node(p).is_some_and(|n| !n.is_hidden())).or(pages.first())
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

    /// How deep `id` sits: its ancestors, its page included (a page is 0 deep, its child 1).
    /// The walk stops past [`MAX_DEPTH`], so a deeper chain an older Relay stored reads as
    /// `MAX_DEPTH + 1`.
    pub fn depth(&self, id: &str) -> usize {
        let mut depth = 0;
        let mut at = self.parent(id);
        while let Some(p) = at {
            depth += 1;
            if depth > MAX_DEPTH {
                break;
            }
            at = self.parent(p);
        }
        depth
    }

    /// How many levels `id` holds below it (0 for a node with no children), counted level by
    /// level and stopping past [`MAX_DEPTH`], as [`Scene::depth`] does.
    pub fn height(&self, id: &str) -> usize {
        self.levels_below(id, MAX_DEPTH + 1)
    }

    fn levels_below(&self, id: &str, most: usize) -> usize {
        let mut level = vec![id];
        let mut height = 0;
        while height < most {
            let next: Vec<&str> = level.iter().flat_map(|at| self.children(at)).map(String::as_str).collect();
            if next.is_empty() {
                break;
            }
            height += 1;
            level = next;
        }
        height
    }

    /// The refusal of an add under `parent` (with `moved`, a move of that node there), worded as
    /// the engine's `design.too_deep`: how deep it, or the deepest node it holds, would sit, and
    /// the chain it would hang from, page first and ending at `parent` in both forms, with
    /// ` › id › …` after it when what the moved node holds is what goes too deep.
    fn too_deep(&self, parent: &str, moved: Option<&str>) -> String {
        let mut chain = self.ancestors(parent);
        chain.reverse();
        chain.push(parent);
        let below = moved.map_or(0, |id| self.levels_below(id, usize::MAX));
        let deep = chain.len() + below;
        let shown = match chain.len() {
            n if n > 5 => format!("{} › {} › … › {} › {}", chain[0], chain[1], chain[n - 2], chain[n - 1]),
            _ => chain.join(" › "),
        };
        match moved {
            Some(id) if below > 0 => format!("what it holds would sit {deep} deep ({shown} › {id} › …); a design nests at most {MAX_DEPTH} levels"),
            _ => format!("it would sit {deep} deep ({shown}); a design nests at most {MAX_DEPTH} levels"),
        }
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

    /// Every node as stored, in `design.get`'s order.
    pub fn to_nodes(&self) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::with_capacity(self.nodes.len());
        out.extend(self.pages().iter().filter_map(|p| self.stored(p).cloned()));
        for page in self.pages() {
            out.extend(self.subtree(page).iter().skip(1).filter_map(|id| self.stored(id).cloned()));
        }
        out
    }

    /// An id no node has, shaped like the engine's but never one it assigns: the engine's are
    /// `frame-12`, a client's `frame-<seed>-<n>`, the seed random per scene, so neither the
    /// engine nor another client makes the same one.
    pub fn fresh_id(&mut self, kind: NodeKind) -> String {
        loop {
            self.counter += 1;
            let id = format!("{}-{}-{}", kind.name(), self.seed, base36(self.counter));
            if !self.nodes.contains_key(&id) {
                return id;
            }
        }
    }

    /// Run one edit as the engine runs a lenient list's (an undo or a redo replaying what was
    /// stored): the value rules only. What names a colour or style the design no longer holds is
    /// taken out, an add or move past [`MAX_DEPTH`] is skipped (an add with what a later edit puts
    /// inside it, moves there or changes in it), and a `delete_style` of a name that is gone does
    /// nothing, as the engine skips them. On an error nothing changed.
    pub fn apply(&mut self, op: &NodeOp) -> Result<(), String> {
        self.run(op, false)
    }

    /// Run one new edit as the engine runs a strict list's: [`Scene::apply`], and the rules on
    /// what a caller writes (`Node::check_new`, `Node::check_update`), every colour and style a
    /// node names held by the design, and no node more than [`MAX_DEPTH`] deep, so the canvas
    /// refuses exactly what the engine would. On an error nothing changed.
    pub fn apply_strict(&mut self, op: &NodeOp) -> Result<(), String> {
        self.run(op, true)
    }

    /// Whether a lenient edit naming `id` is skipped: the scene does not hold it, because a
    /// lenient add left it out as too deep (or what it sat in).
    fn skips(&self, id: &str) -> bool {
        !self.left_out.is_empty() && !self.nodes.contains_key(id) && self.left_out.contains(id)
    }

    fn run(&mut self, op: &NodeOp, strict: bool) -> Result<(), String> {
        match op {
            NodeOp::Add { node, index } => {
                if !strict && node.parent.as_deref().is_some_and(|p| self.skips(p)) {
                    self.left_out.insert(node.id.clone());
                    return Ok(());
                }
                if strict {
                    node.check_new().map_err(|e| format!("add {}: {e}", node.id))?;
                }
                if !valid_id(&node.id) {
                    return Err(format!("add: id {:?} is not a node id", node.id));
                }
                if self.nodes.contains_key(&node.id) {
                    return Err(format!("add: {} already exists", node.id));
                }
                if self.nodes.len() >= MAX_NODES {
                    return Err(format!("a design holds at most {MAX_NODES} nodes"));
                }
                // B49 and B3, as the engine's strict add: a default size, a box fitted to points.
                let mut fitted;
                let node = if strict {
                    fitted = node.clone();
                    fitted.fill_default_size();
                    fitted.contain_points();
                    &fitted
                } else {
                    node
                };
                node.validate()?;
                if let Some(parent) = &node.parent {
                    self.check_container(parent)?;
                    if self.depth(parent) + 1 > MAX_DEPTH {
                        // A lenient list skips it, and what it holds, as the engine's does (W14).
                        if !strict {
                            self.left_out.insert(node.id.clone());
                            return Ok(());
                        }
                        return Err(format!("add {}: {}", node.id, self.too_deep(parent, None)));
                    }
                }
                let mut node = node.clone();
                if Styles::uses(&node) {
                    if strict {
                        self.styles.check(&node).map_err(|e| format!("add {}: {e}", node.id))?;
                    } else {
                        self.styles.forget_missing(&mut node);
                    }
                }
                let key = node.parent.clone().unwrap_or_default();
                insert_at(self.children.entry(key).or_default(), node.id.clone(), *index);
                self.put(node);
            }
            NodeOp::Update { id, set } => {
                if !strict && self.skips(id) {
                    return Ok(());
                }
                let node = self.stored(id).ok_or_else(|| format!("update: no node {id}"))?;
                let mut patched = node.patched(set).map_err(|e| format!("update {id}: {e}"))?;
                if strict {
                    Node::check_update(node, set, &patched).map_err(|e| format!("update {id}: {e}"))?;
                    // B3, as the engine's strict update: the box fitted to new points.
                    if patched.contain_points() {
                        patched.validate().map_err(|e| format!("update {id}: {e}"))?;
                    }
                }
                if Styles::uses(&patched) {
                    if strict {
                        self.styles.check(&patched).map_err(|e| format!("update {id}: {e}"))?;
                    } else {
                        self.styles.forget_missing(&mut patched);
                    }
                }
                self.put(patched);
            }
            NodeOp::Move { id, parent, index, keep_position } => {
                // The canvas's own moves write exact x/y from where it draws the node; one that
                // keeps its place is an agent's, which the engine runs and a reread brings here.
                if *keep_position == Some(true) {
                    return Err(format!("move {id}: the canvas does not send keep_position"));
                }
                if !strict && (self.skips(id) || parent.as_deref().is_some_and(|p| self.skips(p))) {
                    return Ok(());
                }
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
                    // Only a new parent can take it deeper: a node a deeper chain stored still
                    // reorders among its siblings.
                    if old.as_deref() != Some(to.as_str()) && self.depth(&to) + 1 + self.height(id) > MAX_DEPTH {
                        return match strict {
                            true => Err(format!("move {id}: {}", self.too_deep(&to, Some(id)))),
                            false => Ok(()),
                        };
                    }
                    Some(to)
                };
                if let Some(list) = self.children.get_mut(old.as_deref().unwrap_or(ROOT)) {
                    list.retain(|c| c != id);
                }
                insert_at(self.children.entry(new.clone().unwrap_or_default()).or_default(), id.clone(), *index);
                for held in [&mut self.nodes, &mut self.stored] {
                    if let Some(node) = held.get_mut(id) {
                        node.parent = new.clone();
                    }
                }
            }
            // An agent's copy runs in the engine and arrives by a reread (MODEL-SPEC §2.11).
            NodeOp::Copy { id, .. } => return Err(format!("copy {id}: the canvas does not send copy")),
            NodeOp::Color { name, value } => {
                self.styles.set_color(name, value).map_err(|e| format!("color {name}: {e}"))?;
                self.restyle(Some(name));
            }
            NodeOp::Style { name, set } => {
                self.styles.set_style(name, set).map_err(|e| format!("style {name}: {e}"))?;
                self.restyle(Some(name));
            }
            NodeOp::DeleteStyle { name } => self.delete_style(name, strict)?,
            // An agent's edits, which the engine runs one a node; the canvas sends one a node.
            NodeOp::UpdateMany { .. } | NodeOp::DeleteMany { .. } => return Err(format!("{} is run by the engine, one edit a node", op.name())),
            NodeOp::Delete { id } => {
                if !strict && self.skips(id) {
                    return Ok(());
                }
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
                    self.stored.remove(&gone);
                }
            }
        }
        Ok(())
    }

    /// Take colour or style `name` out, and write into each node that used it what it drew of
    /// it (`Styles::detach`), so every user keeps its look, as the engine does. All or nothing.
    fn delete_style(&mut self, name: &str, strict: bool) -> Result<(), String> {
        let mut styles = self.styles.clone();
        let removed = match styles.delete(name) {
            Ok((removed, _)) => removed,
            // A lenient list skips a name that is gone, as the engine's does.
            Err(e) if !strict && e.code == "style_not_found" => return Ok(()),
            Err(e) => return Err(format!("delete_style {name}: {e}")),
        };
        let mut kept = Vec::new();
        for (id, node) in &self.stored {
            if let Some(set) = Styles::detach(node, name, &removed) {
                kept.push(node.patched(&set).map_err(|e| format!("delete_style {name}: {id}: {e}"))?);
            }
        }
        self.styles = styles;
        for node in kept {
            self.put(node);
        }
        // What used it through a style (a colour a style named) draws from the style now.
        self.restyle(None);
        Ok(())
    }

    /// Run every edit of a new list, all or none, as a strict list ([`Scene::apply_strict`]).
    pub fn apply_all(&mut self, ops: &[NodeOp]) -> Result<(), String> {
        let mut trial = self.clone();
        let mut follow = Following::default();
        for (i, op) in ops.iter().enumerate() {
            trial.note(op, &mut follow);
            trial.apply_strict(op).map_err(|e| format!("ops[{i}]: {e}"))?;
        }
        trial.follow_pins(follow);
        *self = trial;
        Ok(())
    }

    /// [`Scene::apply_all`] for an edit the canvas is about to send, refused whole, before
    /// anything runs, when it holds more edits than the engine takes in one list (B44).
    pub fn apply_edit(&mut self, ops: &[NodeOp]) -> Result<(), String> {
        if ops.len() > MAX_OPS {
            return Err(too_many(ops.len()));
        }
        self.apply_all(ops)
    }

    /// Run an edit the canvas is about to send, all or none, as [`Scene::apply_all`] does, leaving
    /// out each update that would change nothing of the stored node as the list has it by then
    /// (B86: a click on the key already lit). Answers what is left to send; empty, nothing is.
    /// Refused whole, the scene as it was, when what is left holds more edits than the engine
    /// takes in one list (B44). An error names the edit by its place in `ops`.
    pub fn apply_effective(&mut self, ops: Vec<NodeOp>) -> Result<Vec<NodeOp>, String> {
        let mut trial = self.clone();
        let mut kept = Vec::with_capacity(ops.len());
        let mut follow = Following::default();
        for (i, op) in ops.into_iter().enumerate() {
            if let NodeOp::Update { id, set } = &op {
                if trial.stored(id).is_some_and(|node| node.patched(set).is_ok_and(|after| after == *node)) {
                    continue;
                }
            }
            trial.note(&op, &mut follow);
            trial.apply_strict(&op).map_err(|e| format!("ops[{i}]: {e}"))?;
            kept.push(op);
        }
        if kept.len() > MAX_OPS {
            return Err(too_many(kept.len()));
        }
        trial.follow_pins(follow);
        *self = trial;
        Ok(kept)
    }

    /// Note what edit `op`, about to run, sets for the follow rule, as the engine's `Run` does:
    /// a frame an update resizes, at its size before, and each axis set on a node itself.
    fn note(&self, op: &NodeOp, follow: &mut Following) {
        match op {
            NodeOp::Add { node, .. } => follow.set_here(&node.id, true, true),
            NodeOp::Update { id, set } => {
                let has = |keys: &[&str]| keys.iter().any(|k| set.contains_key(*k));
                if let Some(n) = self.stored(id).filter(|n| n.kind == NodeKind::Frame && has(&["w", "h", "width", "height"])) {
                    if !follow.resized.iter().any(|(frame, _)| frame == id) {
                        follow.resized.push((id.clone(), (n.w, n.h)));
                    }
                }
                follow.set_here(id, has(&["x", "w", "width"]), has(&["y", "h", "height"]));
            }
            NodeOp::Move { id, parent: Some(to), .. } if self.parent(id) != Some(to.as_str()) => follow.set_here(id, true, true),
            _ => {}
        }
    }

    /// The updates the follow rule gives frame `frame`'s children when its stored size goes from
    /// `old` to `new` (feature-layout §2.1): each pinned free or absolute child's `Node::follow`,
    /// on the axes `keeps` does not say it keeps (one an edit set itself). The canvas's resize
    /// preview reads them too.
    pub fn followers(&self, frame: &str, old: (f64, f64), new: (f64, f64), keeps: impl Fn(&str, bool) -> bool) -> Vec<(String, Map<String, Value>)> {
        let Some(holder) = self.stored(frame) else {
            return Vec::new();
        };
        let laid = holder.layout_mode() != Layout::None;
        self.children(frame)
            .iter()
            .filter_map(|child| {
                let n = self.stored(child)?;
                if !n.is_pinned() || (laid && !n.is_absolute()) {
                    return None;
                }
                let set = n.follow(old, new, !keeps(child, true), !keeps(child, false));
                (!set.is_empty()).then(|| (child.clone(), set))
            })
            .collect()
    }

    /// The follow rule once a list has run, as the engine's `Run::follow_pins`: level by level
    /// from each frame it resized, never more than [`MAX_DEPTH`] levels down.
    fn follow_pins(&mut self, follow: Following) {
        let mut queue: std::collections::VecDeque<Resize> = follow
            .resized
            .iter()
            .filter_map(|(frame, old)| self.stored(frame).map(|n| (frame.clone(), *old, (n.w, n.h), 0)))
            .filter(|(_, old, new, _)| old != new)
            .collect();
        while let Some((frame, old, new, level)) = queue.pop_front() {
            if level >= MAX_DEPTH {
                continue;
            }
            for (child, set) in self.followers(&frame, old, new, |id, across| follow.set_here.contains(&(id.to_string(), across))) {
                let Some(n) = self.stored(&child) else { continue };
                let Ok(next) = n.patched(&set) else { continue };
                if next.kind == NodeKind::Frame && (next.w, next.h) != (n.w, n.h) {
                    queue.push_back((child.clone(), (n.w, n.h), (next.w, next.h), level + 1));
                }
                self.put(next);
            }
        }
    }

    /// Fit edits about to be sent to what a strict list takes (`relay_bus::design::fit_written`),
    /// each update read against its node's kind here or as an earlier add of the list made it;
    /// an update left with nothing to set is dropped. Returns the keys taken out, as `id.key`.
    pub fn fit(&self, ops: &mut Vec<NodeOp>) -> Vec<String> {
        let mut added: FxHashMap<String, NodeKind> = FxHashMap::default();
        let mut dropped = Vec::new();
        ops.retain_mut(|op| {
            let was_empty = matches!(op, NodeOp::Update { set, .. } if set.is_empty());
            let target = op.target().to_string();
            let keys = fit_written(op, |id| added.get(id).copied().or_else(|| self.nodes.get(id).map(|n| n.kind)));
            dropped.extend(keys.into_iter().map(|key| format!("{target}.{key}")));
            match op {
                NodeOp::Add { node, .. } => {
                    added.insert(node.id.clone(), node.kind);
                    true
                }
                NodeOp::Update { set, .. } => was_empty || !set.is_empty(),
                _ => true,
            }
        });
        dropped
    }

    /// Take the engine's copy of a node that an edit of ours changed (as stored; it draws again
    /// from it). Its place stays: the edit put it there already.
    pub fn confirm(&mut self, node: Node) {
        if self.nodes.get(&node.id).is_some_and(|mine| mine.parent == node.parent) {
            self.put(node);
        }
    }

    /// [`Scene::confirm`] each node of an answer, but those that an edit still waiting to be sent
    /// or answered (`ahead`, already run here) changes: its node, and for a `delete_style` every
    /// node naming what it deletes (B27). Taking the engine's copy of those would undo the later
    /// edit here; its own answer confirms them.
    pub fn confirm_all<'a>(&mut self, nodes: Vec<Node>, ahead: impl IntoIterator<Item = &'a NodeOp>) {
        let mut targets: FxHashSet<&str> = FxHashSet::default();
        let mut deleted: Vec<&str> = Vec::new();
        for op in ahead {
            match op {
                NodeOp::DeleteStyle { name } => deleted.push(name),
                _ => targets.extend(op.node_target()),
            }
        }
        for node in nodes {
            let named = || node.names().iter().any(|(_, used)| deleted.contains(used));
            if targets.contains(node.id.as_str()) || (!deleted.is_empty() && named()) {
                continue;
            }
            self.confirm(node);
        }
    }

    /// Bring the scene up to a read of what changed since it matched the engine (TM-80:
    /// `design.get` with `since_rev`), so that it holds what [`Scene::new`] of a whole read
    /// would. `upserts` are the nodes added, changed or moved since, as stored, in `design.get`'s
    /// order, each with its place among its siblings as they are now (0 at the bottom: how many
    /// paint below it, as the engine's `index_of` counts); `removed` are the ids gone since. Each
    /// is put in its place and drawn again, and only the children lists that nodes leave, join
    /// or are removed from change.
    ///
    /// The place is what lets a patch follow a move: a node moved among its siblings and changed
    /// in the same read looks, without it, like one only changed, and keeps a z-order the engine
    /// no longer has: design.get's since_rev answer carries it (`GetOut.indexes`). A delta that does not fit the scene (a removed node whose child stays, a
    /// parent neither holds, a node inside itself, two nodes in one place, a place past the
    /// end) is refused, the scene as it was: read the design whole.
    pub fn patch(&mut self, upserts: Vec<(Node, u32)>, removed: &[String]) -> Result<(), String> {
        let gone: FxHashSet<&str> = removed.iter().map(String::as_str).filter(|id| self.nodes.contains_key(*id)).collect();
        let mut changed: FxHashMap<&str, &Node> = FxHashMap::default();
        for (node, _) in &upserts {
            if gone.contains(node.id.as_str()) {
                return Err(format!("{} is both changed and removed", node.id));
            }
            if changed.insert(&node.id, node).is_some() {
                return Err(format!("{} is changed twice", node.id));
            }
        }
        let stays = |id: &str| !gone.contains(id) && !changed.contains_key(id);
        // A node's parent and kind once patched; None for one the scene will not hold.
        let after = |id: &str| match changed.get(id).copied() {
            Some(n) => Some((n.parent.as_deref(), n.kind)),
            None if gone.contains(id) => None,
            None => self.nodes.get(id).map(|n| (n.parent.as_deref(), n.kind)),
        };
        for id in &gone {
            if let Some(kid) = self.children(id).iter().find(|kid| stays(kid)) {
                return Err(format!("{kid} would be left without its parent {id}"));
            }
        }
        // Every changed node sits in something the patched scene holds that holds children, and
        // not inside itself; the walk up is bounded, so a delta that makes a loop is refused.
        let bound = self.nodes.len() + upserts.len();
        for (node, _) in &upserts {
            let id = node.id.as_str();
            if !node.kind.holds_children() {
                if let Some(kid) = self.children(id).iter().find(|kid| stays(kid)) {
                    return Err(format!("{kid} would sit in {id}, a {}, which holds nothing", node.kind.name()));
                }
            }
            let mut at = node.parent.as_deref();
            for step in 0.. {
                let Some(p) = at else { break };
                let (up, kind) = after(p).ok_or_else(|| format!("{id} would sit in {p}, which is not in the design"))?;
                if step == 0 && !kind.holds_children() {
                    return Err(format!("{id} would sit in {p}, a {}, which holds nothing", kind.name()));
                }
                if p == id {
                    return Err(format!("{id} would sit inside itself"));
                }
                if step > bound {
                    return Err(format!("the parents above {id} would go round in a loop"));
                }
                at = up;
            }
        }
        // Each list a node joins: the places in it are distinct and within it.
        let mut places: FxHashMap<&str, Vec<(u32, &str)>> = FxHashMap::default();
        for (node, index) in &upserts {
            places.entry(node.parent.as_deref().unwrap_or(ROOT)).or_default().push((*index, &node.id));
        }
        for (parent, placed) in &mut places {
            placed.sort_unstable();
            let total = self.children(parent).iter().filter(|c| stays(c)).count() + placed.len();
            if let Some(pair) = placed.windows(2).find(|pair| pair[0].0 == pair[1].0) {
                return Err(format!("{} and {} would both sit at {} among their siblings", pair[0].1, pair[1].1, pair[0].0));
            }
            if let Some((index, id)) = placed.last().filter(|(index, _)| *index as usize >= total) {
                return Err(format!("{id} would sit at {index} among {total} siblings"));
            }
        }
        // The lists nodes leave or join, made again: what stays, in its order, and each node that
        // joins at its place. Placed in rising order, each place is at most the list's length so
        // far, since the places are distinct and below the list's final length.
        let mut lists: FxHashSet<String> = places.keys().map(|p| p.to_string()).collect();
        for id in gone.iter().copied().chain(changed.keys().copied()) {
            if let Some(node) = self.nodes.get(id) {
                lists.insert(node.parent.clone().unwrap_or_default());
            }
        }
        for key in lists {
            let old = self.children.remove(&key).unwrap_or_default();
            if gone.contains(key.as_str()) {
                continue;
            }
            let placed = places.get(key.as_str()).map(Vec::as_slice).unwrap_or_default();
            let mut kept = old.into_iter().filter(|c| stays(c));
            let mut list = Vec::with_capacity(kept.size_hint().1.unwrap_or(0) + placed.len());
            let mut next = placed.iter().peekable();
            loop {
                match next.next_if(|(index, _)| *index as usize == list.len()) {
                    Some((_, id)) => list.push(id.to_string()),
                    None => match kept.next() {
                        Some(id) => list.push(id),
                        None => break,
                    },
                }
            }
            if !list.is_empty() {
                self.children.insert(key, list);
            }
        }
        for id in &gone {
            self.nodes.remove(*id);
            self.stored.remove(*id);
            self.children.remove(*id);
        }
        for (node, _) in upserts {
            self.put(node);
        }
        Ok(())
    }

    fn check_container(&self, id: &str) -> Result<(), String> {
        match self.nodes.get(id) {
            Some(p) if p.kind.holds_children() => Ok(()),
            Some(p) => Err(format!("a {} cannot hold other nodes", p.kind.name())),
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
    fn a_design_opens_on_its_first_page_shown() {
        let page = |id: &str, hidden: bool| {
            let mut n = node(id, None, NodeKind::Page, [0.0; 4]);
            n.hidden = hidden.then_some(true);
            n
        };
        let s = Scene::new(vec![page("a", true), page("b", false), page("c", false)]);
        assert_eq!(s.first_page().map(String::as_str), Some("b"));
        let all = Scene::new(vec![page("a", true), page("b", true)]);
        assert_eq!(all.first_page().map(String::as_str), Some("a"), "every page hidden: the first");
        assert_eq!(Scene::new(vec![]).first_page(), None);
    }

    #[test]
    fn edits_land_where_the_engine_puts_them() {
        let mut s = sample();
        let add = NodeOp::Add { node: node("e", Some("a"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]), index: Some(0) };
        s.apply(&add).unwrap();
        assert_eq!(s.children("a"), ["e", "b", "c"]);
        s.apply(&NodeOp::Move { id: "e".into(), parent: None, index: None, keep_position: None }).unwrap();
        assert_eq!(s.children("a"), ["b", "c", "e"]);
        s.apply(&NodeOp::Move { id: "e".into(), parent: Some("p1".into()), index: Some(1), keep_position: None }).unwrap();
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
        assert!(s.apply(&NodeOp::Move { id: "a".into(), parent: Some("b".into()), index: None, keep_position: None }).is_err());
        s.apply(&NodeOp::Add { node: node("g", Some("a"), NodeKind::Group, [0.0; 4]), index: None }).unwrap();
        assert!(s.apply(&NodeOp::Move { id: "a".into(), parent: Some("g".into()), index: None, keep_position: None }).unwrap_err().contains("inside"));
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

    /// A new list is held to what the engine holds a strict one to; an undo's replay is not.
    #[test]
    fn new_edits_are_strict_and_replays_are_not() {
        let mut s = sample();
        let radius = json!({"radius": 4}).as_object().unwrap().clone();
        let why = s.apply_all(&[NodeOp::Update { id: "d".into(), set: radius }]).unwrap_err();
        assert!(why.starts_with("ops[0]: update d: radius: ellipse nodes do not use it"), "{why}");
        let mut legacy = Node::read(json!({"id": "g", "parent": "p1", "kind": "group", "future": 1,
            "shadow": {"blur": 4, "color": "#000"}})).unwrap();
        let add = NodeOp::Add { node: legacy.clone(), index: None };
        assert!(s.apply_all(std::slice::from_ref(&add)).unwrap_err().starts_with("ops[0]: add g: future: not a node field"));
        // The undo of a delete puts it back as it was stored.
        s.apply(&add).unwrap();
        assert_eq!(s.node("g"), Some(&legacy));
        // And it stays editable: a strict update checks only what it writes.
        let x = json!({"x": 5}).as_object().unwrap().clone();
        s.apply_all(&[NodeOp::Update { id: "g".into(), set: x }]).unwrap();
        legacy.x = 5.0;
        assert_eq!(s.node("g"), Some(&legacy));
    }

    /// What the canvas never sends is refused, leaving the scene as it was: an agent's edit of
    /// that kind runs in the engine and comes back by a reread.
    #[test]
    fn the_canvas_refuses_what_it_never_sends() {
        let mut s = sample();
        let before = s.to_nodes();
        let keep = NodeOp::Move { id: "b".into(), parent: Some("p1".into()), index: None, keep_position: Some(true) };
        assert_eq!(s.apply(&keep).unwrap_err(), "move b: the canvas does not send keep_position");
        assert!(s.apply_all(&[keep]).unwrap_err().starts_with("ops[0]: move b"));
        assert_eq!(s.to_nodes(), before);
        s.apply(&NodeOp::Move { id: "b".into(), parent: Some("p1".into()), index: None, keep_position: Some(false) }).unwrap();
        assert_eq!(s.parent("b"), Some("p1"));
        let before = s.to_nodes();
        let copy = NodeOp::Copy { id: "a".into(), as_id: Some("a2".into()), parent: None, index: None, dx: 0.0, dy: 0.0, rename: None, instance: None };
        assert_eq!(s.apply(&copy).unwrap_err(), "copy a: the canvas does not send copy");
        assert!(s.apply_all(&[copy]).unwrap_err().starts_with("ops[0]: copy a"));
        assert_eq!(s.to_nodes(), before);
        // The colour and style edits it sends (its Styles section) run, and fit leaves them be.
        let reuse = vec![
            NodeOp::Color { name: "accent".into(), value: "#fff".into() },
            NodeOp::Style { name: "card".into(), set: Default::default() },
            NodeOp::DeleteStyle { name: "card".into() },
        ];
        let mut ops = reuse.clone();
        assert!(s.fit(&mut ops).is_empty());
        assert_eq!(ops, reuse);
        s.apply_all(&reuse).unwrap();
        assert_eq!((s.styles().colors.len(), s.styles().styles.len()), (1, 0));
        assert_eq!(s.to_nodes(), before);
    }

    /// What the canvas sends is fitted to the kinds of the nodes it writes, an add earlier in the
    /// list included, and an update left with nothing goes.
    #[test]
    fn fit_drops_what_each_kind_does_not_use() {
        let s = sample();
        let mut ops = vec![
            NodeOp::Update { id: "d".into(), set: json!({"radius": 4, "fill": "#fff"}).as_object().unwrap().clone() },
            NodeOp::Add { node: node("img", Some("p1"), NodeKind::Image, [0.0, 0.0, 10.0, 10.0]), index: None },
            NodeOp::Update { id: "img".into(), set: json!({"fill": "#fff"}).as_object().unwrap().clone() },
            NodeOp::Update { id: "b".into(), set: Default::default() },
            NodeOp::Delete { id: "c".into() },
        ];
        assert_eq!(s.fit(&mut ops), ["d.radius", "img.fill"]);
        assert_eq!(ops.len(), 4, "{ops:?}");
        assert!(matches!(&ops[0], NodeOp::Update { set, .. } if set.len() == 1 && set.contains_key("fill")));
        assert!(matches!(&ops[2], NodeOp::Update { id, set } if id == "b" && set.is_empty()), "an empty update sent as such stays");
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
        // Never `frame-<digits>`, the engine's own shape, and unlike another scene's.
        assert_eq!(fresh.matches('-').count(), 2, "{fresh}");
        assert_ne!(sample().fresh_id(NodeKind::Frame), sample().fresh_id(NodeKind::Frame));
        let mut long = sample();
        long.counter = u64::MAX - 1;
        assert!(valid_id(&long.fresh_id(NodeKind::Polygon)));
    }

    pub(crate) fn set(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().unwrap().clone()
    }

    fn update(id: &str, value: serde_json::Value) -> NodeOp {
        NodeOp::Update { id: id.into(), set: set(value) }
    }

    /// Two colours and two styles: `card`, a box filled `$accent`, rounded and outlined, and
    /// `title`, a large text in `$ink`.
    pub(crate) fn palette() -> Styles {
        serde_json::from_value(json!({
            "colors": {"accent": "#4C9DFF", "ink": "#111111"},
            "styles": {
                "card": {"fill": "$accent", "radius": 12, "stroke": "#000000", "stroke_width": 2},
                "title": {"font_size": 32, "color": "$ink"},
            },
        }))
        .unwrap()
    }

    /// The sample naming them: rect `b` takes style `card`, text `c` style `title`, and ellipse
    /// `d` is filled `$accent`.
    pub(crate) fn styled() -> Scene {
        let mut nodes = sample().to_nodes();
        for n in &mut nodes {
            match n.id.as_str() {
                "b" => n.style = Some("card".into()),
                "c" => n.style = Some("title".into()),
                "d" => n.fill = Some("$accent".into()),
                _ => {}
            }
        }
        Scene::new(nodes).with_styles(palette())
    }

    /// `id` as drawn, without the name of its style (which a detach takes off).
    fn look(s: &Scene, id: &str) -> Node {
        let mut n = s.node(id).unwrap().clone();
        n.style = None;
        n
    }

    /// The scenes hold the same nodes, stored and drawn, in the same order, with the same names.
    fn same(a: &Scene, b: &Scene) {
        assert_eq!(a.to_nodes(), b.to_nodes());
        assert_eq!((a.len(), a.pages(), a.styles()), (b.len(), b.pages(), b.styles()));
        for n in b.to_nodes() {
            assert_eq!(a.children(&n.id), b.children(&n.id), "children of {}", n.id);
            assert_eq!(a.node(&n.id), b.node(&n.id), "{} as drawn", n.id);
            assert_eq!(a.stored(&n.id), b.stored(&n.id), "{} as stored", n.id);
        }
    }

    /// The undo the engine makes of a `delete_style`: the name made again, then each node it
    /// detached put back.
    fn undo_of_delete(s: &Scene, name: &str) -> Vec<NodeOp> {
        let mut styles = s.styles().clone();
        let (removed, mut undo) = styles.delete(name).unwrap();
        for n in s.to_nodes() {
            if let Some(set) = Styles::detach(&n, name, &removed) {
                undo.push(NodeOp::Update { id: n.id.clone(), set: n.before(&set) });
            }
        }
        undo
    }

    #[test]
    fn a_styled_node_draws_its_style_and_stores_its_names() {
        let s = styled();
        let b = s.node("b").unwrap();
        assert_eq!((b.fill.as_deref(), b.radius, b.stroke_width), (Some("#4C9DFF"), Some(12.0), Some(2.0)));
        let stored = s.stored("b").unwrap();
        assert_eq!((stored.fill.as_deref(), stored.radius, stored.style.as_deref()), (None, None, Some("card")));
        let c = s.node("c").unwrap();
        assert_eq!((c.font_size, c.color.as_deref()), (Some(32.0), Some("#111111")));
        assert_eq!(s.node("d").unwrap().fill.as_deref(), Some("#4C9DFF"));
        assert_eq!(s.stored("d").unwrap().fill.as_deref(), Some("$accent"));
        // What goes back to the engine is what it stored; a node that names nothing is held once.
        assert_eq!(s.to_nodes().iter().find(|n| n.id == "b"), Some(stored));
        assert!(std::ptr::eq(s.node("a").unwrap(), s.stored("a").unwrap()));
        // Without the design's names, a node draws as if they were missing: the kind's default.
        let bare = Scene::new(s.to_nodes());
        assert_eq!((bare.node("d").unwrap().fill.as_deref(), bare.node("b").unwrap().radius), (None, None));
        assert_eq!(bare.to_nodes(), s.to_nodes());
        same(&bare.with_styles(palette()), &s);
    }

    #[test]
    fn a_colour_change_restyles_every_user() {
        let mut s = styled();
        assert_eq!(s.users("accent"), ["b", "d"], "b through its style, d itself");
        assert_eq!((s.users("card"), s.users("ink"), s.users("nope")), (vec!["b".to_string()], vec!["c".to_string()], vec![]));
        // In design.get's order: every page first, then what is on each. A page takes a fill.
        let mut two = s.clone();
        let mut page = node("p2", None, NodeKind::Page, [0.0; 4]);
        page.fill = Some("$accent".into());
        let mut e = node("e", Some("p2"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]);
        e.fill = Some("$accent".into());
        two.apply_all(&[NodeOp::Add { node: page, index: None }, NodeOp::Add { node: e, index: None }]).unwrap();
        let read: Vec<String> = two.to_nodes().into_iter().filter(|n| names(two.styles(), n, "accent")).map(|n| n.id).collect();
        assert_eq!((two.users("accent"), read), (vec!["p2".to_string(), "b".into(), "d".into(), "e".into()], vec!["p2".to_string(), "b".into(), "d".into(), "e".into()]));
        let stored = s.to_nodes();
        s.apply_all(&[NodeOp::Color { name: "accent".into(), value: "#ff0000".into() }]).unwrap();
        assert_eq!((s.node("b").unwrap().fill.as_deref(), s.node("d").unwrap().fill.as_deref()), (Some("#ff0000"), Some("#ff0000")));
        assert_eq!(s.to_nodes(), stored, "a colour change rewrites no node");
        s.apply_all(&[NodeOp::Style { name: "card".into(), set: set(json!({"radius": 4, "fill": null})) }]).unwrap();
        let b = s.node("b").unwrap();
        assert_eq!((b.radius, b.fill.as_deref()), (Some(4.0), None));
        assert_eq!(s.users("accent"), ["d"]);
        assert_eq!(s.to_nodes(), stored);
        // A refusal names the edit and what is wrong, and leaves the scene as it was.
        let why = s.apply_all(&[NodeOp::Color { name: "card".into(), value: "#fff".into() }]).unwrap_err();
        assert!(why.starts_with("ops[0]: color card: name: \"card\" is a style"), "{why}");
        let why = s.apply_all(&[NodeOp::Style { name: "card".into(), set: set(json!({"fill": "$acent"})) }]).unwrap_err();
        assert!(why.starts_with("ops[0]: style card: fill: the design has no colour $acent; did you mean $accent?"), "{why}");
        assert_eq!(s.node("b").unwrap().radius, Some(4.0));
    }

    #[test]
    fn deleting_a_style_keeps_each_users_look_and_its_undo_brings_it_back() {
        let mut s = styled();
        let original = s.clone();
        let looks = |s: &Scene| ["a", "b", "c", "d"].map(|id| look(s, id));
        let before = looks(&s);
        let undo_card = undo_of_delete(&s, "card");
        s.apply_all(&[NodeOp::DeleteStyle { name: "card".into() }]).unwrap();
        let b = s.stored("b").unwrap();
        assert_eq!((b.style.as_deref(), b.fill.as_deref(), b.radius), (None, Some("$accent"), Some(12.0)));
        assert!(!s.styles().styles.contains_key("card"));
        assert_eq!(looks(&s), before);
        // A colour is written in where it was named, a style naming it included.
        let undo_accent = undo_of_delete(&s, "accent");
        s.apply_all(&[NodeOp::DeleteStyle { name: "accent".into() }]).unwrap();
        assert_eq!((s.stored("b").unwrap().fill.as_deref(), s.stored("d").unwrap().fill.as_deref()), (Some("#4C9DFF"), Some("#4C9DFF")));
        assert_eq!(looks(&s), before);
        assert!(s.users("accent").is_empty());
        // The engine's undos, replayed newest first as a lenient list does, give it all back.
        for op in undo_accent.iter().chain(&undo_card) {
            s.apply(op).unwrap();
        }
        same(&s, &original);
    }

    #[test]
    fn names_must_exist_in_new_edits_and_replays_forget_them() {
        let mut s = styled();
        let before = s.to_nodes();
        let mut e = node("e", Some("a"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]);
        e.fill = Some("$acent".into());
        let why = s.apply_all(&[NodeOp::Add { node: e.clone(), index: None }]).unwrap_err();
        assert!(why.starts_with("ops[0]: add e: fill: the design has no colour $acent; did you mean $accent?"), "{why}");
        let why = s.apply_all(&[update("b", json!({"style": "cards"}))]).unwrap_err();
        assert!(why.starts_with(r#"ops[0]: update b: style: the design has no style "cards"; did you mean "card"?"#), "{why}");
        let why = s.apply_all(&[NodeOp::DeleteStyle { name: "nope".into() }]).unwrap_err();
        assert!(why.starts_with("ops[0]: delete_style nope: name: the design has no colour or style"), "{why}");
        assert_eq!(s.to_nodes(), before);
        // A replay takes out what names something gone, as the engine's lenient list does.
        e.style = Some("gone".into());
        s.apply(&NodeOp::Add { node: e, index: None }).unwrap();
        let kept = s.stored("e").unwrap();
        assert_eq!((kept.fill.as_deref(), kept.style.as_deref()), (None, None));
        s.apply(&update("b", json!({"stroke": "$nope"}))).unwrap();
        assert_eq!((s.stored("b").unwrap().stroke.as_deref(), s.stored("b").unwrap().style.as_deref()), (None, Some("card")));
        s.apply(&NodeOp::DeleteStyle { name: "nope".into() }).unwrap();
    }

    #[test]
    fn a_move_keeps_both_forms_in_step() {
        let mut s = styled();
        s.apply_all(&[NodeOp::Move { id: "b".into(), parent: Some("p1".into()), index: Some(0), keep_position: None }]).unwrap();
        assert_eq!((s.node("b").unwrap().parent.as_deref(), s.stored("b").unwrap().parent.as_deref()), (Some("p1"), Some("p1")));
        assert_eq!(s.children("p1"), ["b", "a", "d"]);
        s.apply_all(&[NodeOp::Delete { id: "b".into() }]).unwrap();
        assert!(s.node("b").is_none() && s.stored("b").is_none() && s.users("card").is_empty());
    }

    #[test]
    fn a_preview_on_a_styled_node_is_replaced_by_the_edit() {
        let mut s = styled();
        s.node_mut("b").unwrap().x = 99.0;
        assert_eq!((s.node("b").unwrap().x, s.stored("b").unwrap().x), (99.0, 10.0));
        // The gesture puts it back and sends its edit: the drawn node is made again from it.
        s.node_mut("b").unwrap().x = 10.0;
        s.apply_all(&[update("b", json!({"x": 50}))]).unwrap();
        let b = s.node("b").unwrap();
        assert_eq!((b.x, b.fill.as_deref(), b.radius, s.stored("b").unwrap().x), (50.0, Some("#4C9DFF"), Some(12.0), 50.0));
        // A preview of a field the style gives; the engine's copy draws from what it stored.
        s.node_mut("b").unwrap().radius = Some(3.0);
        let engine = s.stored("b").unwrap().clone();
        s.confirm(engine);
        assert_eq!(s.node("b").unwrap().radius, Some(12.0));
        // A text being typed shows in its style at once, and goes without a trace.
        let mut t = node("t", Some("a"), NodeKind::Text, [0.0; 4]);
        t.style = Some("title".into());
        s.insert_preview(t);
        assert_eq!(s.node("t").unwrap().font_size, Some(32.0));
        s.remove_preview("t");
        assert!(s.node("t").is_none() && s.stored("t").is_none() && !s.children("a").contains(&"t".to_string()));
    }

    /// Layout reads the drawn node: a text measures at the size its style gives it.
    #[test]
    fn a_text_measures_at_the_size_its_style_gives() {
        struct BySize;
        impl crate::design::layout::Measure for BySize {
            fn measure(&self, n: &Node, _: Option<f64>) -> (f64, f64) {
                let size = n.font_size.unwrap_or(16.0);
                (n.text.as_deref().unwrap_or("").chars().count() as f64 * size / 2.0, size)
            }
        }
        let placed = crate::design::layout::place(&styled(), "p1", &BySize);
        assert_eq!(placed.rect("c").map(|r| (r.w, r.h)), Some((32.0, 32.0)));
        let placed = crate::design::layout::place(&sample(), "p1", &BySize);
        assert_eq!(placed.rect("c").map(|r| (r.w, r.h)), Some((16.0, 16.0)));
    }

    /// A page and a chain of frames under it, `f1` to `f{deep}`, each inside the one before.
    fn chain(deep: usize) -> Vec<Node> {
        let mut nodes = vec![node("p1", None, NodeKind::Page, [0.0; 4])];
        for i in 1..=deep {
            let parent = if i == 1 { "p1".to_string() } else { format!("f{}", i - 1) };
            nodes.push(node(&format!("f{i}"), Some(&parent), NodeKind::Frame, [0.0, 0.0, 10.0, 10.0]));
        }
        nodes
    }

    #[test]
    fn nesting_stops_at_max_depth() {
        let add = |id: &str, parent: &str| NodeOp::Add { node: node(id, Some(parent), NodeKind::Frame, [0.0, 0.0, 10.0, 10.0]), index: None };
        let to = |id: &str, parent: &str| NodeOp::Move { id: id.into(), parent: Some(parent.into()), index: None, keep_position: None };
        let mut s = Scene::new(chain(63));
        assert_eq!((s.depth("p1"), s.depth("f63"), s.height("p1"), s.height("f63")), (0, 63, 63, 0));
        s.apply_all(&[add("f64", "f63")]).unwrap();
        assert_eq!(s.depth("f64"), MAX_DEPTH);
        let why = s.apply_all(&[add("f65", "f64")]).unwrap_err();
        assert_eq!(why, "ops[0]: add f65: it would sit 65 deep (p1 › f1 › … › f63 › f64); a design nests at most 64 levels");
        // A replay skips it, as the engine's lenient list does.
        s.apply(&add("f65", "f64")).unwrap();
        assert!(s.node("f65").is_none());
        // A move counts what the moved node holds. Both forms end the chain at the parent, as the
        // add's does; the one for what it holds names the moved node after it.
        s.apply_all(&[add("g", "p1"), add("h", "g")]).unwrap();
        let why = s.apply_all(&[to("g", "f63")]).unwrap_err();
        assert_eq!(why, "ops[0]: move g: what it holds would sit 65 deep (p1 › f1 › … › f62 › f63 › g › …); a design nests at most 64 levels");
        let why = s.apply_all(&[to("h", "f64")]).unwrap_err();
        assert_eq!(why, "ops[0]: move h: it would sit 65 deep (p1 › f1 › … › f63 › f64); a design nests at most 64 levels");
        let why = s.apply_all(&[to("h", "p1"), to("h", "f64")]).unwrap_err();
        assert_eq!(why, "ops[1]: move h: it would sit 65 deep (p1 › f1 › … › f63 › f64); a design nests at most 64 levels");
        // A short chain is shown whole.
        let mut short = Scene::new(chain(3));
        short.apply_all(&[add("g", "p1"), add("h", "g")]).unwrap();
        for i in 0..MAX_DEPTH - 2 {
            let (id, parent) = (format!("k{i}"), if i == 0 { "h".to_string() } else { format!("k{}", i - 1) });
            short.apply_all(&[add(&id, &parent)]).unwrap();
        }
        let why = short.apply_all(&[to("g", "f3")]).unwrap_err();
        assert_eq!(why, "ops[0]: move g: what it holds would sit 67 deep (p1 › f1 › f2 › f3 › g › …); a design nests at most 64 levels");
        s.apply(&to("g", "f63")).unwrap();
        assert_eq!(s.parent("g"), Some("p1"));
        s.apply_all(&[to("g", "f62")]).unwrap();
        assert_eq!(s.depth("h"), MAX_DEPTH);
        // A deeper chain an older Relay stored: the walks stop counting, and nothing recurses.
        let mut legacy = Scene::new(chain(4990));
        assert_eq!((legacy.depth("f4990"), legacy.height("p1"), legacy.to_nodes().len()), (MAX_DEPTH + 1, MAX_DEPTH + 1, 4991));
        // Its nodes still reorder among their siblings, and nothing goes deeper.
        legacy.apply_all(&[add("x", "p1"), NodeOp::Move { id: "f1".into(), parent: None, index: None, keep_position: None }]).unwrap();
        assert_eq!(legacy.children("p1"), ["x", "f1"]);
        let why = legacy.apply_all(&[add("y", "f4990")]).unwrap_err();
        assert!(why.starts_with("ops[0]: add y: it would sit 4991 deep (p1 › f1 › … › f4989 › f4990)"), "{why}");
    }

    /// W14: a replay skips an add past MAX_DEPTH and all the list does inside it, as the engine's
    /// lenient list skips an edit whose parent or node is gone, so an undo or a redo that holds
    /// one is not taken for a scene gone out of step.
    #[test]
    fn a_replay_skips_what_it_does_inside_an_add_too_deep() {
        let add = |id: &str, parent: &str| NodeOp::Add { node: node(id, Some(parent), NodeKind::Frame, [0.0, 0.0, 10.0, 10.0]), index: None };
        let to = |id: &str, parent: &str| NodeOp::Move { id: id.into(), parent: Some(parent.into()), index: None, keep_position: None };
        let mut s = Scene::new(chain(MAX_DEPTH));
        let replay = [add("g1", "f64"), add("g2", "g1"), add("g3", "g2"), update("g2", json!({"x": 4})), to("g3", "p1"),
            to("f64", "g2"), NodeOp::Delete { id: "g1".into() }];
        for op in &replay {
            s.apply(op).unwrap_or_else(|e| panic!("{op:?}: {e}"));
        }
        assert_eq!((s.len(), s.parent("f64")), (MAX_DEPTH + 1, Some("f63")));
        assert!(["g1", "g2", "g3"].iter().all(|id| s.node(id).is_none()));
        // A new list is held to what the engine holds it to: there is no g1.
        assert_eq!(s.apply_all(&[add("g4", "g1")]).unwrap_err(), "ops[0]: no parent g1");
        // An id it holds is not skipped for having been left out once.
        s.apply_all(&[add("g1", "p1")]).unwrap();
        s.apply(&add("g2", "g1")).unwrap();
        assert_eq!(s.parent("g2"), Some("g1"));
        // Nor is one it never left out: that is a scene out of step.
        assert_eq!(s.apply(&add("k", "nope")).unwrap_err(), "no parent nope");
    }

    /// B27: an answer to an earlier edit does not undo, here, a later edit still on its way.
    #[test]
    fn an_answer_leaves_nodes_a_queued_edit_changes() {
        let mut s = styled();
        let x = |v: f64| update("b", json!({"x": v}));
        s.apply_all(&[x(11.0)]).unwrap();
        // A second nudge while the first is in flight: run here, queued to send.
        let queued = vec![x(12.0)];
        s.apply_all(&queued).unwrap();
        let mut engine_b = s.stored("b").unwrap().clone();
        engine_b.x = 11.0;
        let mut engine_d = s.stored("d").unwrap().clone();
        engine_d.opacity = Some(0.5);
        s.confirm_all(vec![engine_b.clone(), engine_d], &queued);
        assert_eq!((s.node("b").unwrap().x, s.node("d").unwrap().opacity), (12.0, Some(0.5)));
        // So a third nudge starts from 12, and the queued edit's own answer confirms it.
        assert_eq!(s.stored("b").unwrap().x + 1.0, 13.0);
        engine_b.x = 12.0;
        s.confirm_all(vec![engine_b.clone()], &Vec::new());
        assert_eq!(s.stored("b"), Some(&engine_b));
        // A queued delete_style: what it detached keeps the look it was given here.
        let queued = vec![NodeOp::DeleteStyle { name: "card".into() }];
        s.apply_all(&queued).unwrap();
        let detached = s.stored("b").unwrap().clone();
        s.confirm_all(vec![engine_b], &queued);
        assert_eq!(s.stored("b"), Some(&detached));
    }

    /// B44: a list longer than the engine takes is refused whole, before any of it runs.
    #[test]
    fn an_edit_past_max_ops_is_refused_whole() {
        let x = |v: usize| update("b", json!({"x": v}));
        let mut s = sample();
        let before = s.to_nodes();
        let ops: Vec<NodeOp> = (0..=MAX_OPS).map(x).collect();
        let why = s.apply_edit(&ops).unwrap_err();
        assert_eq!(why, format!("that is {} changes at once; one edit makes at most {MAX_OPS}. Select fewer layers.", MAX_OPS + 1));
        assert_eq!(s.to_nodes(), before);
        s.apply_edit(&ops[..MAX_OPS]).unwrap();
        assert_eq!(s.node("b").unwrap().x, (MAX_OPS - 1) as f64);
        // What apply_effective would send is what counts: a no-op left out makes room.
        let mut s = sample();
        let mut ops: Vec<NodeOp> = (1..=MAX_OPS).map(x).collect();
        ops.insert(0, x(10));
        assert_eq!(s.apply_effective(ops).unwrap().len(), MAX_OPS);
        let mut s = sample();
        assert!(s.apply_effective((1..=MAX_OPS + 1).map(x).collect()).unwrap_err().starts_with("that is 2001 changes"));
        assert_eq!(s.to_nodes(), before);
    }

    /// B86: an update that changes nothing is not sent, so it neither takes an undo step nor
    /// clears the redo stack.
    #[test]
    fn apply_effective_drops_unchanged_updates() {
        let fill = |id: &str, c: &str| update(id, json!({"fill": c}));
        let mut s = sample();
        s.apply_all(&[fill("b", "#ff0000")]).unwrap();
        let before = s.to_nodes();
        assert!(s.apply_effective(vec![fill("b", "#ff0000")]).unwrap().is_empty());
        assert_eq!(s.to_nodes(), before);
        let moved = update("b", json!({"x": 5}));
        assert_eq!(s.apply_effective(vec![fill("b", "#ff0000"), moved.clone()]).unwrap(), [moved]);
        assert_eq!(s.node("b").unwrap().x, 5.0);
        // Judged against the list so far: an update after the add it follows.
        let mut r2 = node("r2", Some("a"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]);
        r2.fill = Some("#00ff00".into());
        let kept = s.apply_effective(vec![NodeOp::Add { node: r2, index: None }, fill("r2", "#00ff00")]).unwrap();
        assert!(matches!(kept.as_slice(), [NodeOp::Add { .. }]), "{kept:?}");
        // A refusal names the edit by its place in the list sent, and changes nothing.
        let before = s.to_nodes();
        let why = s.apply_effective(vec![update("b", json!({"x": 6})), update("b", json!({"opacity": 2}))]).unwrap_err();
        assert!(why.starts_with("ops[1]: update b: opacity"), "{why}");
        assert_eq!(s.to_nodes(), before);
        // Against what is stored: the name it holds is no change, the colour it draws is one.
        let mut s = styled();
        assert!(s.apply_effective(vec![fill("d", "$accent")]).unwrap().is_empty());
        assert_eq!(s.apply_effective(vec![fill("d", "#4C9DFF")]).unwrap().len(), 1);
        assert_eq!(s.stored("d").unwrap().fill.as_deref(), Some("#4C9DFF"));
    }

    /// What `design.get` with since_rev answers after `ops`, as the engine writes revs: every
    /// node added, changed, moved or detached since, as stored, in its order and with its place
    /// among its siblings, and the ids removed (those added again are not). A move bumps only the
    /// rev of the node it moves, so its siblings are not in the answer.
    fn changes(base: &Scene, ops: &[NodeOp]) -> (Vec<(Node, u32)>, Vec<String>, Scene) {
        let mut after = base.clone();
        let mut changed: FxHashSet<String> = FxHashSet::default();
        let mut removed: Vec<String> = Vec::new();
        for op in ops {
            let old: FxHashMap<String, Node> = after.to_nodes().into_iter().map(|n| (n.id.clone(), n)).collect();
            after.apply(op).unwrap();
            for n in after.to_nodes() {
                if old.get(&n.id) != Some(&n) {
                    removed.retain(|id| *id != n.id);
                    changed.insert(n.id);
                }
            }
            for id in old.keys().filter(|id| after.stored(id).is_none()) {
                changed.remove(id);
                removed.push(id.clone());
            }
            if let NodeOp::Move { id, .. } = op {
                changed.insert(id.clone());
            }
        }
        let upserts = after.to_nodes().into_iter().filter(|n| changed.contains(&n.id)).map(|n| {
            let index = after.index(&n.id).unwrap() as u32;
            (n, index)
        });
        (upserts.collect(), removed, after)
    }

    /// TM-80: patching `base` with what changed in `ops` gives what a whole read of the result
    /// builds: the same nodes, stored and drawn, and every children list in the same order.
    fn patches(base: &Scene, ops: &[NodeOp]) {
        let (upserts, removed, after) = changes(base, ops);
        let whole = Scene::new(after.to_nodes()).with_styles(after.styles().clone());
        let mut patched = base.clone();
        patched.patch(upserts, &removed).unwrap_or_else(|e| panic!("{ops:?}: {e}"));
        patched.set_styles(after.styles().clone());
        same(&patched, &whole);
    }

    /// TM-80: a read of what changed, each node with its place among its siblings, patches the
    /// scene into what a whole read builds, whatever the edits were; a delta that does not fit
    /// the scene is refused and leaves it as it was.
    #[test]
    fn a_read_of_what_changed_patches_the_scene_as_a_whole_read_builds_it() {
        let base = styled();
        let to = |id: &str, parent: Option<&str>, index: Option<u32>| NodeOp::Move { id: id.into(), parent: parent.map(Into::into), index, keep_position: None };
        let add = |n: Node, index: Option<u32>| NodeOp::Add { node: n, index };
        let delete = |id: &str| NodeOp::Delete { id: id.into() };
        let mut e = node("e", Some("a"), NodeKind::Rect, [0.0, 0.0, 5.0, 5.0]);
        e.fill = Some("#000000".into());
        let mut d = base.stored("d").unwrap().clone();
        let d_again = d.clone();
        d.opacity = Some(0.5);
        let ring = node("b", Some("a"), NodeKind::Ellipse, [0.0, 0.0, 8.0, 8.0]);
        let page = node("p2", None, NodeKind::Page, [0.0; 4]);
        let on_page = node("r", Some("p2"), NodeKind::Rect, [1.0, 1.0, 4.0, 4.0]);
        let group = node("g", Some("a"), NodeKind::Group, [0.0; 4]);
        let in_group = node("h", Some("g"), NodeKind::Rect, [0.0; 4]);
        let lists: &[&[NodeOp]] = &[
            &[update("c", json!({"text": "Hello"}))],
            &[update("b", json!({"x": 20, "style": null})), update("d", json!({"fill": "#123456"}))],
            &[delete("a")],
            &[delete("c"), update("b", json!({"opacity": 0.5}))],
            &[NodeOp::Color { name: "accent".into(), value: "#ff0000".into() }],
            &[NodeOp::Style { name: "card".into(), set: set(json!({"radius": 2})) }, update("d", json!({"style": "card"}))],
            &[NodeOp::DeleteStyle { name: "card".into() }, NodeOp::DeleteStyle { name: "accent".into() }],
            &[],
            // What the delta alone cannot place: adds, moves and reorders.
            &[add(e.clone(), Some(0))],
            &[to("b", Some("p1"), None)],
            &[to("b", None, Some(1))],
            &[update("b", json!({"x": 11})), update("b", json!({"x": 10}))],
            &[delete("d"), add(d_again, Some(0))],
            // A move among its siblings and a change in the same read: b goes to the top of a.
            &[to("b", None, None), update("b", json!({"x": 20}))],
            &[delete("d"), add(d, Some(0))],
            &[delete("b"), add(ring, Some(1))],
            &[to("b", Some("p1"), Some(0)), delete("a")],
            &[to("d", Some("p1"), Some(0)), to("b", Some("p1"), Some(1)), update("c", json!({"text": "Hey"}))],
            &[add(page, Some(0)), add(on_page, None), to("a", Some("p2"), Some(0))],
            &[add(group.clone(), Some(1)), add(in_group, None), to("c", Some("g"), Some(0)), add(e, Some(3))],
            &[NodeOp::DeleteStyle { name: "card".into() }, to("b", None, None)],
        ];
        for ops in lists {
            patches(&base, ops);
        }
        // A delta that does not fit the scene is refused, the scene as it was.
        let held = |id: &str| base.stored(id).unwrap().clone();
        let with = |id: &str, f: &dyn Fn(&mut Node)| {
            let mut n = held(id);
            f(&mut n);
            n
        };
        type Upserts = Vec<(Node, u32)>;
        let unfit: Vec<(Upserts, Vec<String>, &str)> = vec![
            (vec![], vec!["a".into()], "b would be left without its parent a"),
            (vec![(with("b", &|n| n.x = 3.0), 0)], vec!["a".into(), "b".into(), "c".into()], "b is both changed and removed"),
            (vec![(held("b"), 0), (held("b"), 0)], vec![], "b is changed twice"),
            (vec![(with("b", &|n| n.parent = Some("nope".into())), 0)], vec![], "b would sit in nope, which is not in the design"),
            (vec![(with("c", &|n| n.parent = Some("b".into())), 0)], vec![], "c would sit in b, a rect, which holds nothing"),
            (vec![(group.clone(), 0), (with("a", &|n| n.parent = Some("g".into())), 0)], vec![], "g would sit inside itself"),
            (vec![(with("a", &|n| n.kind = NodeKind::Rect), 0)], vec![], "b would sit in a, a rect, which holds nothing"),
            (vec![(held("b"), 2)], vec![], "b would sit at 2 among 2 siblings"),
            (vec![(held("b"), 1), (held("c"), 1)], vec![], "b and c would both sit at 1 among their siblings"),
        ];
        for (upserts, removed, why) in unfit {
            let mut s = base.clone();
            assert_eq!(s.patch(upserts, &removed).unwrap_err(), why);
            same(&s, &base);
        }
        // Ids removed that it never held are nothing to it.
        let mut s = base.clone();
        s.patch(Vec::new(), &["never".into()]).unwrap();
        same(&s, &base);
    }
}
