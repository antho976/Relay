//! Components (feature-reuse, round three): a main component and its instances. An instance is
//! real nodes, so every painter, layout and check draws it as it draws anything else; what makes
//! it one is that each of its nodes names the main's node it follows (`instance_of`), and keeps
//! some fields as its own (`overrides`). An `impl Run`, as the follow rule is.
//!
//! Once a list's edits have run, every main they reached is brought to its instances
//! ([`Run::sync_components`]): each instance's nodes take the main's fields but their overrides
//! (and its root its own place), take the main's structure (a node the main gained is added, one
//! it lost is deleted, its order is the main's), and a main that holds an instance of another is
//! brought to its own instances after that one. Every write is an edit like any other, dated,
//! counted, undone and expected, so one undo takes back the change and what it did to the
//! instances. The person's lenient list (an undo, a redo) puts back what was stored and syncs
//! nothing; an agent's is synced as a strict one is (`Run::derives`).
//!
//! A design that never held a main component pays nothing for all this: the flag
//! (`designs.components`, migration v32) is read with the row every list reads.

use super::apply::Run;
use super::node_of;
use crate::engine::IntoBus;
use relay_bus::design::{copy_id, Node, NOT_OVERRIDDEN};
use relay_bus::error::BusError;
use relay_bus::ErrorKind;
use rusqlite::params;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// The nodes that may follow node `?2` (`?2` given as `"instance_of":"<id>"`): one pass over the
/// design's bodies, run only when a main of it changed.
const FOLLOWERS: &str = "SELECT node_id FROM design_nodes WHERE design_id = ?1 AND instr(body, ?2) > 0";

/// What an instance's root keeps as its own whatever the main says: where it is and whether it
/// shows or can be picked.
pub(super) const ROOT_OWN: &[&str] = &["x", "y", "rotation", "pin_x", "pin_y", "absolute", "hidden", "locked"];

/// How many times one list brings one main to its instances before it is a loop (a main inside
/// an instance of itself, through others).
const MAX_SYNCS: usize = 16;

/// A node as the component rules read it: whether it is a main, and the main node it follows.
#[derive(Clone)]
pub(super) struct Link {
    pub(super) main: bool,
    pub(super) of: Option<String>,
}

impl Run<'_> {
    /// Whether the design has held a main component (`designs.components`, read with the row the
    /// list starts from): one that never did has no instance either.
    pub(super) fn has_mains(&mut self) -> Result<bool, BusError> {
        Ok(self.has_mains == Some(true))
    }

    /// Mark the design as one that holds a main component, once per list.
    pub(super) fn mark_mains(&mut self) -> Result<(), BusError> {
        if self.has_mains != Some(true) {
            self.tx.prepare_cached("UPDATE designs SET components = 1 WHERE id = ?1").bus()?.execute([self.design]).bus()?;
            self.has_mains = Some(true);
        }
        Ok(())
    }

    /// Node `id` as the component rules read it, from its row; `None` when it is not there.
    pub(super) fn link(&self, id: &str) -> Result<Option<Link>, BusError> {
        let Some(row) = self.row(id)? else { return Ok(None) };
        if !row.body.contains("\"component\":true") && !row.body.contains("\"instance_of\":") {
            return Ok(Some(Link { main: false, of: None }));
        }
        let node = node_of(id.to_string(), row.parent, &row.body)?;
        Ok(Some(Link { main: node.component == Some(true), of: node.instance_of }))
    }

    /// Whether node `id` is an instance's root: it follows a main component's root.
    pub(super) fn instance_root(&self, id: &str) -> Result<Option<String>, BusError> {
        let Some(of) = self.link(id)?.and_then(|l| l.of) else { return Ok(None) };
        Ok(self.link(&of)?.filter(|l| l.main).map(|_| of))
    }

    /// The refusal of an edit that would change an instance's structure by hand (W-components):
    /// `what` names the edit. A lenient list and the sync itself are let through.
    pub(super) fn inside_instance(&self, what: &str) -> BusError {
        self.fail(ErrorKind::Invalid, "design.instance", format!("{what}: an instance takes its structure from its main component"))
            .with_hint("change the main (every instance follows), hide this node with hidden: true (kept as an override), or detach the instance with instance_of: null on its root")
    }

    /// Note a main the list reached by deleting `id` (its subtree's rows `bodies`): its parent's
    /// chain may hold a main, and a main deleted takes its instances' link with it.
    pub(super) fn note_deleted(&mut self, parent: Option<&str>, bodies: &[(String, &str)]) {
        if self.has_mains != Some(true) {
            return;
        }
        if let Some(parent) = parent {
            self.reached.push(parent.to_string());
        }
        for (id, body) in bodies {
            if body.contains("\"component\":true") {
                self.mains_gone.push(id.clone());
            }
        }
    }

    /// The fields an update of instance node `node` sets that it keeps as its own from now on,
    /// written into `set` as its `overrides` (W-components): every field it sets but what links
    /// it, and on an instance's root what it keeps anyway. An update giving `overrides` itself
    /// says which, and is taken as given.
    pub(super) fn record_overrides(&self, node: &Node, set: &mut Map<String, Value>) -> Result<(), BusError> {
        if node.instance_of.is_none() || set.contains_key("overrides") || set.contains_key("instance_of") {
            return Ok(());
        }
        let root = self.instance_root(&node.id)?.is_some();
        let mut fields: BTreeSet<String> = node.overrides.iter().flatten().cloned().collect();
        let before = fields.len();
        for key in set.keys() {
            let key = match key.as_str() {
                "width" => "w",
                "height" => "h",
                key => key,
            };
            if NOT_OVERRIDDEN.contains(&key) || (root && ROOT_OWN.contains(&key)) {
                continue;
            }
            fields.insert(key.to_string());
        }
        if fields.len() != before {
            set.insert("overrides".into(), Value::from(fields.into_iter().collect::<Vec<_>>()));
        }
        Ok(())
    }

    /// Detach the instance whose root is `root`: each of its nodes follows, one level down, what
    /// the main's node it followed follows (an instance inside the main stays an instance of its
    /// own main), or nothing, keeping both levels' overrides. The root's own link was cleared by
    /// the edit that asked.
    pub(super) fn detach(&mut self, root: &str) -> Result<(), BusError> {
        let Some(row) = self.row(root)? else { return Ok(()) };
        let rows = self.subtree_rows(root.to_string(), row)?;
        for (id, row) in rows.into_iter().skip(1) {
            let Ok(node) = node_of(id.clone(), row.parent.clone(), &row.body) else { continue };
            let Some(of) = node.instance_of.clone() else { continue };
            let main = self.stored(&of)?;
            let mut set = Map::new();
            match main.as_ref().and_then(|m| m.instance_of.clone()) {
                Some(deeper) => {
                    let kept: BTreeSet<String> = node.overrides.iter().flatten().chain(main.iter().flat_map(|m| m.overrides.iter().flatten())).cloned().collect();
                    set.insert("instance_of".into(), Value::from(deeper));
                    set.insert("overrides".into(), if kept.is_empty() { Value::Null } else { Value::from(kept.into_iter().collect::<Vec<_>>()) });
                }
                None => {
                    set.insert("instance_of".into(), Value::Null);
                    set.insert("overrides".into(), Value::Null);
                }
            }
            self.write_synced(&node, set)?;
        }
        Ok(())
    }

    /// Bring every main the list reached to its instances, then what that changed inside other
    /// mains to theirs (see the module). Refused (`design.component_cycle`) for an instance
    /// inside its own main, however deep.
    pub(super) fn sync_components(&mut self) -> Result<(), BusError> {
        if !self.derives() || !self.has_mains()? && self.mains_gone.is_empty() {
            return Ok(());
        }
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut cache: HashMap<String, Option<Link>> = HashMap::new();
        let reached: Vec<String> = self.touched.iter().cloned().chain(std::mem::take(&mut self.reached)).collect();
        for id in reached {
            self.mains_over(&id, None, &mut cache, &mut queue)?;
        }
        queue.extend(std::mem::take(&mut self.mains_gone));
        let mut synced: HashMap<String, usize> = HashMap::new();
        self.syncing = true;
        let out = (|| {
            while let Some(main) = queue.pop_front() {
                let count = synced.entry(main.clone()).or_default();
                *count += 1;
                if *count > MAX_SYNCS {
                    return Err(self.cycle(&main));
                }
                let written = self.sync_main(&main)?;
                // What changed inside another main is that main's change too; the instances just
                // brought to it are not its change, unless one is inside it (a loop, counted).
                let mut cache = HashMap::new();
                for id in written {
                    self.mains_over(&id, Some(&main), &mut cache, &mut queue)?;
                }
            }
            Ok(())
        })();
        self.syncing = false;
        out
    }

    /// Queue each main that holds node `id` (itself included), and the main of each instance it
    /// is in but `synced`'s, once.
    fn mains_over(&mut self, id: &str, synced: Option<&str>, cache: &mut HashMap<String, Option<Link>>, queue: &mut VecDeque<String>) -> Result<(), BusError> {
        self.learn(id)?;
        let chain: Vec<String> = self.chain(id).map(str::to_string).collect();
        for at in chain {
            if !cache.contains_key(&at) {
                let link = self.link(&at)?;
                cache.insert(at.clone(), link);
            }
            let Some(link) = cache[&at].clone() else { continue };
            if link.main && !queue.contains(&at) {
                queue.push_back(at.clone());
            }
            if let Some(of) = link.of {
                if !cache.contains_key(&of) {
                    let target = self.link(&of)?;
                    cache.insert(of.clone(), target);
                }
                if cache[&of].as_ref().is_some_and(|l| l.main) && Some(of.as_str()) != synced && !queue.contains(&of) {
                    queue.push_back(of);
                }
            }
        }
        Ok(())
    }

    fn cycle(&self, main: &str) -> BusError {
        BusError::new(ErrorKind::Invalid, "design.component_cycle", format!("{main:?} would hold an instance of itself, so it could never settle"))
            .with_details(serde_json::json!({"id": main}))
            .with_hint("take the instance out of its main, or detach it (instance_of: null on its root)")
    }

    /// Bring main `main` to each of its instances; the ids written. A main that is gone, or is
    /// a main no longer, detaches them.
    fn sync_main(&mut self, main: &str) -> Result<Vec<String>, BusError> {
        let pattern = format!("\"instance_of\":{}", Value::from(main));
        let followers: Vec<String> = self.tx.prepare_cached(FOLLOWERS).bus()?
            .query_map(params![self.design, pattern], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let is_main = self.link(main)?.is_some_and(|l| l.main);
        let mut written = Vec::new();
        // The main as it stands, read once for all its instances.
        let mut mains: Option<Vec<Node>> = None;
        for root in followers {
            // Only a node that follows the main's root is an instance's root.
            let Some(node) = self.stored(&root)? else { continue };
            if node.instance_of.as_deref() != Some(main) {
                continue;
            }
            if !is_main {
                let mut set = Map::new();
                set.insert("instance_of".into(), Value::Null);
                set.insert("overrides".into(), Value::Null);
                self.write_synced(&node, set)?;
                self.detach(&root)?;
                written.push(root);
                continue;
            }
            self.learn(&root)?;
            if self.chain(&root).any(|at| at == main) {
                return Err(self.cycle(main));
            }
            if mains.is_none() {
                let Some(row) = self.row(main)? else { break };
                mains = Some(self.subtree_rows(main.to_string(), row)?.into_iter()
                    .filter_map(|(id, row)| node_of(id, row.parent, &row.body).ok()).collect());
            }
            written.extend(self.sync_instance(main, &root, mains.as_deref().unwrap_or_default())?);
        }
        Ok(written)
    }

    /// Bring main `main` to its instance at `root`: fields, structure and order; the ids written.
    fn sync_instance(&mut self, main: &str, root: &str, mains: &[Node]) -> Result<Vec<String>, BusError> {
        self.label = format!("instance {root:?} of {main:?}");
        if mains.is_empty() {
            return Ok(Vec::new());
        }
        let Some(root_row) = self.row(root)? else { return Ok(Vec::new()) };
        let own: Vec<Node> = self.subtree_rows(root.to_string(), root_row)?.into_iter()
            .filter_map(|(id, row)| node_of(id, row.parent, &row.body).ok()).collect();
        let mut follower: HashMap<String, Node> = HashMap::new();
        for n in own.iter().skip(1) {
            if let Some(of) = &n.instance_of {
                follower.entry(of.clone()).or_insert_with(|| n.clone());
            }
        }
        let main_ids: HashSet<&str> = mains.iter().map(|m| m.id.as_str()).collect();
        let mut made: HashMap<String, String> = HashMap::from([(main.to_string(), root.to_string())]);
        let mut written = Vec::new();
        // What the main lost, its instance loses: the outermost of each such run of nodes.
        let mut dropped: HashSet<String> = HashSet::new();
        for n in own.iter().skip(1) {
            let lost = n.instance_of.as_deref().is_some_and(|of| !main_ids.contains(of));
            let under = n.parent.as_ref().is_some_and(|p| dropped.contains(p));
            if lost || under {
                dropped.insert(n.id.clone());
                if !under {
                    self.delete(n.id.clone())?;
                    written.push(n.parent.clone().unwrap_or_default());
                }
            }
        }
        // Siblings in the main's order, by main parent.
        let mut order: HashMap<String, Vec<String>> = HashMap::new();
        for m in &mains[1..] {
            order.entry(m.parent.clone().unwrap_or_default()).or_default().push(m.id.clone());
        }
        for (i, m) in mains.iter().enumerate() {
            let current = if i == 0 { own.first().cloned() } else { follower.get(&m.id).cloned() };
            let mut want = m.to_map();
            for key in NOT_OVERRIDDEN {
                want.remove(*key);
            }
            let kept: Vec<String> = current.as_ref().and_then(|c| c.overrides.clone()).unwrap_or_default();
            let mut keep: Vec<&str> = kept.iter().map(String::as_str).collect();
            if i == 0 {
                keep.extend(ROOT_OWN);
            }
            let now = current.as_ref().map(Node::to_map).unwrap_or_default();
            for key in keep {
                match now.get(key) {
                    Some(v) => want.insert(key.to_string(), v.clone()),
                    None => want.remove(key),
                };
            }
            want.insert("instance_of".into(), Value::from(m.id.clone()));
            if !kept.is_empty() {
                want.insert("overrides".into(), Value::from(kept.clone()));
            }
            let parent = if i == 0 { None } else { made.get(m.parent.as_deref().unwrap_or_default()).cloned() };
            match current {
                Some(c) => {
                    let mut set = Map::new();
                    for key in want.keys().chain(now.keys()).filter(|k| !matches!(k.as_str(), "id" | "parent" | "kind")) {
                        if want.get(key) != now.get(key) {
                            set.insert(key.clone(), want.get(key).cloned().unwrap_or(Value::Null));
                        }
                    }
                    if !set.is_empty() {
                        self.write_synced(&c, set)?;
                        written.push(c.id.clone());
                    }
                    if let Some(parent) = parent.filter(|p| c.parent.as_deref() != Some(p.as_str())) {
                        self.move_(c.id.clone(), Some(parent), None, false)?;
                        written.push(c.id.clone());
                    }
                    made.insert(m.id.clone(), c.id.clone());
                }
                None => {
                    let Some(parent) = parent else { continue };
                    let id = self.follower_id(main, root, &m.id)?;
                    want.insert("kind".into(), Value::from(m.kind.name()));
                    let mut node = Node::read(Value::Object(want)).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", format!("{}: {why}", m.id)))?;
                    (node.id, node.parent) = (id.clone(), Some(parent));
                    self.add(node, None)?;
                    written.push(id.clone());
                    made.insert(m.id.clone(), id);
                }
            }
        }
        // The main's order among the siblings it gave, each parent's followers first.
        for (main_parent, kids) in &order {
            let Some(parent) = made.get(main_parent).cloned() else { continue };
            let ids: Vec<String> = kids.iter().filter_map(|k| made.get(k).cloned()).collect();
            let now: Vec<String> = self.tx.prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id").bus()?
                .query_map(params![self.design, parent], |r| r.get(0)).bus()?
                .collect::<rusqlite::Result<Vec<String>>>().bus()?
                .into_iter().filter(|id| ids.contains(id)).collect();
            if now != ids {
                for (k, id) in ids.iter().enumerate() {
                    self.move_(id.clone(), Some(parent.clone()), Some(k as u32), false)?;
                }
                written.push(parent);
            }
        }
        Ok(written)
    }

    /// An id for the node an instance at `root` gains for the main's node `of`: by copy's rule
    /// from the main's id to the root's, else with a number after it.
    fn follower_id(&self, main: &str, root: &str, of: &str) -> Result<String, BusError> {
        let base = copy_id(main, root, of).unwrap_or_else(|| format!("{root}:n"));
        let mut id = base.clone();
        let mut n = 2;
        while self.meta(&id)?.is_some() {
            id = format!("{}-{n}", &base[..base.len().min(60)]);
            n += 1;
        }
        Ok(id)
    }

    /// Write `set` into instance node `node` as the sync does: no override is recorded and no
    /// structure rule asked, but it is dated, counted, undone and expected as any update is.
    pub(super) fn write_synced(&mut self, node: &Node, set: Map<String, Value>) -> Result<(), BusError> {
        let was = self.syncing;
        self.syncing = true;
        let out = self.update(node.id.clone(), set).map(|_| ());
        self.syncing = was;
        out
    }
}

