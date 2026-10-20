//! A design's named colours and styles in `design.apply` (MODEL-SPEC §2.11): the copy, color, style
//! and delete_style edits, and the `designs.styles` column they read and write whole. An
//! `impl Run` of engine-reuse's.
//!
//! The engine never draws a node through its style: it only checks that every name a node uses
//! is one the design holds. The canvas resolves them (`Styles::resolve`), with the same rules the
//! edits here run through (`Styles::set_color`, `set_style`, `delete`, `detach`), so the engine
//! and the scene apply a colour or style edit identically.

use super::apply::Run;
use super::{body_of, row_bytes};
use crate::engine::IntoBus;
use relay_bus::design::{copy_id, valid_id, Conflict, ConflictReason, Node, NodeKind, NodeOp, StyleError, Styles, MAX_NODES};
use relay_bus::error::BusError;
use relay_bus::ErrorKind;
use rusqlite::params;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// Most names a refusal of an unknown one lists in its hint.
const HINT_NAMES: usize = 10;

/// What the `designs.styles` column counts in `designs.bytes`: its JSON beyond `{}`.
fn styles_bytes(json: &str) -> i64 {
    (json.len() as i64 - 2).max(0)
}

impl Run<'_> {
    /// The design's colours and styles, read from the store on first need.
    pub(super) fn styles(&mut self) -> Result<&mut Styles, BusError> {
        if self.styles.is_none() {
            let json: String = self.tx.prepare_cached("SELECT styles FROM designs WHERE id = ?1").bus()?
                .query_row([self.design], |r| r.get(0)).bus()?;
            let styles: Styles = serde_json::from_str(&json)
                .map_err(|why| BusError::internal(format!("design {}: its colours and styles do not read: {why}", self.design)))?;
            self.styles_len = styles_bytes(&json);
            self.styles = Some(styles);
        }
        Ok(self.styles.as_mut().expect("read above"))
    }

    /// Every colour and style `node` names is one the design holds: refused by name, with the
    /// names it does hold. A lenient list (an undo putting back what was stored) takes out what
    /// names one that is gone, as the canvas draws it, and tells an agent.
    pub(super) fn check_names(&mut self, node: &mut Node) -> Result<(), BusError> {
        if !Styles::uses(node) {
            return Ok(());
        }
        if self.lenient {
            let forgot = self.styles()?.forget_missing(node);
            if !forgot.is_empty() {
                self.warn(format_args!("{}: names a colour or style the design no longer has; left out", forgot.join(", ")));
            }
            return Ok(());
        }
        let checked = self.styles()?.check(node);
        let Err(error) = checked else { return Ok(()) };
        let styles = self.styles()?;
        let names: Vec<String> = styles.colors.keys().map(|name| format!("${name}"))
            .chain(styles.styles.keys().cloned()).take(HINT_NAMES).collect();
        let hint = match names.is_empty() {
            true => "name a colour with {\"op\": \"color\"} or a style with {\"op\": \"style\"} first".to_string(),
            false => format!("the design's colours and styles: {}", names.join(", ")),
        };
        Err(self.fail(ErrorKind::NotFound, "design.style_not_found", error.why).with_hint(hint))
    }

    /// A colour or style edit refused, as the error codes name it.
    fn style_error(&self, error: StyleError) -> BusError {
        match error.code {
            "style_exists" => self.fail(ErrorKind::Conflict, "design.style_exists", error.why),
            "style_not_found" => self.fail(ErrorKind::NotFound, "design.style_not_found", error.why),
            "too_large" => self.fail(ErrorKind::Invalid, "design.too_large", error.why),
            _ => self.fail(ErrorKind::Invalid, "design.style_invalid", error.why),
        }
    }

    /// `color {name, value}`: name a colour, or change it. Naming it as it is changes nothing.
    /// A lenient list (an undo) skips a colour whose name a style took since (`exists`).
    pub(super) fn color(&mut self, name: String, value: String) -> Result<Option<String>, BusError> {
        let styles = self.styles()?;
        let before = styles.colors.get(&name).cloned();
        let set = styles.set_color(&name, &value);
        let undo = match set {
            Ok(undo) => undo,
            Err(error) if error.code == "style_exists" && self.lenient => {
                self.skip(Conflict::new(Some(self.at as u32), name, ConflictReason::Exists));
                return Ok(None);
            }
            Err(error) => return Err(self.style_error(error)),
        };
        if before.as_ref() != Some(&value) {
            self.restyled(undo);
        }
        Ok(Some(name))
    }

    /// `style {name, set}`: make a style, or change one. A change to what it already holds
    /// changes nothing. Its undo gives back the whole style as it was, so one replayed after the
    /// style was deleted makes it again whole, never a part of it. A lenient list (an undo) makes
    /// a style naming colours deleted since without them, and says so.
    pub(super) fn style(&mut self, name: String, mut set: Map<String, Value>) -> Result<Option<String>, BusError> {
        let styles = self.styles()?;
        let old = styles.styles.get(&name).cloned();
        let mut made = styles.set_style(&name, &set);
        if self.lenient && made.as_ref().is_err_and(|error| error.code == "style_not_found") {
            let held = self.styles()?.colors.clone();
            let gone: Vec<&'static str> = relay_bus::design::Style::default().patched(&set).map(|style| {
                style.names().into_iter().filter(|(_, color)| !held.contains_key(*color)).map(|(field, _)| field).collect()
            }).unwrap_or_default();
            if !gone.is_empty() {
                let mut gone = gone;
                gone.dedup();
                for field in &gone {
                    set.remove(*field);
                }
                self.warn(format_args!("{}: names a colour the design no longer has; left out", gone.join(", ")));
                made = self.styles()?.set_style(&name, &set);
            }
        }
        let undo = made.map_err(|error| self.style_error(error))?;
        let now = self.styles()?.styles.get(&name).cloned();
        if now == old {
            return Ok(Some(name));
        }
        let undo = match old {
            Some(old) => {
                let mut whole = old.to_set();
                for key in now.map(|now| now.to_set()).unwrap_or_default().keys() {
                    whole.entry(key.clone()).or_insert(Value::Null);
                }
                NodeOp::Style { name: name.clone(), set: whole }
            }
            None => undo,
        };
        self.restyled(undo);
        Ok(Some(name))
    }

    /// After a colour or style edit that changed them: its undo kept.
    fn restyled(&mut self, undo: NodeOp) {
        self.styles_changed = true;
        self.inverse.push(vec![undo]);
    }

    /// `delete_style {name}`: take a colour or style out of the design. Every node using it is
    /// written what it drew of it (`Styles::detach`), so it keeps its look; the undo makes it
    /// again first, then names it in each of them again. A name the design does not hold is
    /// refused, or skipped (`missing`) when lenient.
    pub(super) fn delete_style(&mut self, name: String) -> Result<Option<String>, BusError> {
        let taken = self.styles()?.delete(&name);
        let (removed, mut undo) = match taken {
            Ok(taken) => taken,
            Err(error) if error.code == "style_not_found" && self.lenient => {
                self.skip(Conflict::new(Some(self.at as u32), name, ConflictReason::Missing));
                return Ok(None);
            }
            Err(error) => return Err(self.style_error(error)),
        };
        // Names are id characters, so the compact JSON holds them as written; the text test is
        // loose (a text could hold the words), and each body is read to be sure.
        let (styled, colored) = (format!("\"style\":\"{name}\""), format!("\"${name}\""));
        let users: Vec<(String, Option<String>, String)> = self.tx
            .prepare_cached("SELECT node_id, parent_id, body FROM design_nodes WHERE design_id = ?1
                AND (instr(body, ?2) > 0 OR instr(body, ?3) > 0)").bus()?
            .query_map(params![self.design, styled, colored], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        for (id, parent, body) in users {
            // A row this build cannot read is left as it is: it names what is gone, which the
            // canvas draws as the kind's default.
            let Ok(node) = super::node_of(id.clone(), parent, &body) else { continue };
            let Some(set) = Styles::detach(&node, &name, &removed) else { continue };
            self.target = Some(id);
            let mut next = node.patched(&set).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
            let written = body_of(&mut next)?;
            self.tx.prepare_cached("UPDATE design_nodes SET body = ?3, rev = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
                .execute(params![self.design, next.id, written, self.rev]).bus()?;
            self.bytes += written.len() as i64 - body.len() as i64;
            self.touch(&next.id);
            let before = node.before(&set);
            self.updated.entry(next.id.clone()).or_default().extend(before.keys().cloned());
            undo.push(NodeOp::Update { id: next.id, set: before });
        }
        self.target = Some(name.clone());
        self.styles_changed = true;
        self.inverse.push(undo);
        Ok(Some(name))
    }

    /// Write the design's colours and styles back when an edit of the list changed them, and
    /// count the change in its size.
    pub(super) fn write_styles(&mut self) -> Result<(), BusError> {
        if !self.styles_changed {
            return Ok(());
        }
        let Some(styles) = self.styles.as_ref() else { return Ok(()) };
        let json = serde_json::to_string(styles).bus()?;
        self.tx.prepare_cached("UPDATE designs SET styles = ?2 WHERE id = ?1").bus()?
            .execute(params![self.design, json]).bus()?;
        self.bytes += styles_bytes(&json) - self.styles_len;
        self.styles_len = styles_bytes(&json);
        Ok(())
    }

    /// `copy {id, as?, parent?, index?, dx?, dy?}`: copy a node and everything inside it, as one
    /// edit (MODEL-SPEC §2.11). Ids by `copy_id`; a connector inside joins the copies of its
    /// ends, and an end outside what is copied lets go where the stored boxes draw it, as the
    /// canvas's duplicate does; a page copies as a page. The undo deletes the copy.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn copy(&mut self, id: String, as_id: Option<String>, parent: Option<String>, index: Option<u32>, dx: f64, dy: f64)
        -> Result<Option<String>, BusError> {
        let Some(root) = self.row(&id)? else { return self.missing("no such node") };
        let page = root.kind == Ok(NodeKind::Page);
        // Where it goes: a page among the pages; anything else in `parent`, else beside the
        // original, never in itself.
        let to = if page {
            if parent.is_some() {
                return Err(self.fail_on("parent", ErrorKind::Invalid, "design.parent", "a page's copy is a page; it sits in no parent"));
            }
            None
        } else {
            let to = parent.or_else(|| root.parent.clone()).unwrap_or_default();
            if !self.container(&to, &id)? {
                return Ok(None);
            }
            self.learn(&to)?;
            if self.chain(&to).any(|at| at == id) {
                return Err(self.fail_on("parent", ErrorKind::Invalid, "design.cycle", format!("{to:?} is inside {id:?}, which this copies")));
            }
            Some(to)
        };
        let rows = self.subtree_rows(id.clone(), root)?;
        if let Some(to) = to.as_deref() {
            if !self.fits(to, &id, self.height(&id)?)? {
                return Ok(None);
            }
        }
        if self.nodes + rows.len() as i64 > MAX_NODES as i64 {
            return Err(self.fail(ErrorKind::Invalid, "design.too_large", format!("a design holds at most {MAX_NODES} nodes; this copy adds {}", rows.len())));
        }
        // Every node of it is read: one this build cannot read could not be copied whole.
        let mut nodes = Vec::with_capacity(rows.len());
        for (node_id, row) in &rows {
            let Some(node) = self.readable(node_id, row)? else { return Ok(None) };
            nodes.push(node);
        }
        let kind = nodes[0].kind;
        let new_root = match as_id {
            Some(new_root) if !valid_id(&new_root) => {
                return Err(self.fail_on("as", ErrorKind::Invalid, "design.node_invalid", format!("as: {new_root:?} is not an id: use 1 to 64 letters, digits, '-', '_', '.' or ':'")));
            }
            Some(new_root) => new_root,
            None => self.assign(kind)?,
        };
        let mut map: HashMap<&str, String> = HashMap::with_capacity(rows.len());
        let mut taken: HashMap<String, &str> = HashMap::with_capacity(rows.len());
        for node in &nodes {
            let Some(new) = copy_id(&id, &new_root, &node.id) else {
                return Err(self.fail_on("as", ErrorKind::Invalid, "design.node_invalid", format!("the copy of {:?} would be over 64 characters", node.id))
                    .with_hint("use a shorter as"));
            };
            if let Some(other) = taken.insert(new.clone(), &node.id) {
                return Err(self.fail(ErrorKind::Invalid, "design.node_invalid", format!("{other:?} and {:?} would both be copied as {new:?}", node.id))
                    .with_hint("rename one of them"));
            }
            if self.meta(&new)?.is_some() {
                if self.lenient {
                    // Nothing of the copy is made: later edits naming any of its ids are skipped.
                    self.skip(Conflict::new(Some(self.at as u32), new_root.clone(), ConflictReason::Exists));
                    self.skipped_adds.insert(new_root.clone());
                    for node in &nodes {
                        if let Some(copy) = copy_id(&id, &new_root, &node.id) {
                            self.skipped_adds.insert(copy);
                        }
                    }
                    return Ok(None);
                }
                return Err(self.fail_on("as", ErrorKind::Conflict, "design.node_exists", format!("{new:?} is already in the design"))
                    .with_hint("give the copy another as, or leave it out to have one assigned"));
            }
            map.insert(node.id.as_str(), new);
        }
        let inside: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        // A connector's end outside the copy lets go where the stored boxes draw the original.
        let mut freed: HashMap<&str, Map<String, Value>> = HashMap::new();
        for node in &nodes {
            let ends: Vec<&str> = [("from", &node.from), ("to", &node.to)].into_iter()
                .filter(|(_, end)| end.as_deref().is_some_and(|end| !inside.contains(end)))
                .map(|(field, _)| field).collect();
            if !ends.is_empty() {
                freed.insert(node.id.as_str(), self.letting_go(node, &ends)?);
            }
        }
        // Children 1, 2, 3… under each new parent in walk order, which is paint order.
        let mut ords: HashMap<String, f64> = HashMap::new();
        let mut insert = self.tx.prepare_cached("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body, rev) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)").bus()?;
        let root_ord = self.slot(to.as_deref(), index, None)?;
        let mut made = Vec::with_capacity(nodes.len());
        for (i, original) in nodes.iter().enumerate() {
            let mut node = match freed.get(original.id.as_str()) {
                Some(set) => original.patched(set).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?,
                None => original.clone(),
            };
            node.id = map[original.id.as_str()].clone();
            let ord = if i == 0 {
                node.parent = to.clone();
                (node.x, node.y) = (node.x + dx, node.y + dy);
                root_ord
            } else {
                node.parent = original.parent.as_deref().map(|p| map[p].clone());
                let next = ords.entry(node.parent.clone().unwrap_or_default()).or_insert(0.0);
                *next += 1.0;
                *next
            };
            for end in [&mut node.from, &mut node.to] {
                if let Some(copy) = end.as_deref().and_then(|e| map.get(e)) {
                    *end = Some(copy.clone());
                }
            }
            let body = body_of(&mut node)?;
            insert.execute(params![self.design, node.id, node.parent, node.kind.name(), ord, body, self.rev]).bus()?;
            self.bytes += row_bytes(&node.id, node.parent.as_deref(), &body);
            made.push(node);
        }
        drop(insert);
        if self.tombstones {
            let mut back = self.tx.prepare_cached("DELETE FROM design_removed WHERE design_id = ?1 AND node_id = ?2").bus()?;
            for node in &made {
                back.execute(params![self.design, node.id]).bus()?;
            }
        }
        self.nodes += made.len() as i64;
        self.pages += i64::from(page);
        for node in &made {
            self.conflicts.remove(&node.id);
            self.touch(&node.id);
            self.added.insert(node.id.clone());
            if node.kind.holds_children() {
                self.parents.insert(node.id.clone(), node.parent.clone());
            }
        }
        self.copied.insert(new_root.clone(), id);
        self.inverse.push(vec![NodeOp::Delete { id: new_root.clone() }]);
        Ok(Some(new_root))
    }
}
