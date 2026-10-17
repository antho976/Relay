//! `design.*` (BUS.md §10.18): a project's design canvases. A design is a `designs` row and its
//! nodes, one `design_nodes` row each: the node's JSON beside its parent and its paint order
//! (`ord`, ascending is bottom to top), so a move rewrites two columns and never the node.
//!
//! Every change to the nodes is a `design.apply`: edits run in order against the request's
//! transaction, so a later edit sees an earlier one and a refusal anywhere rolls back them all.
//! It answers with the edits that undo it, which the canvas keeps for its undo and `audit.undo`
//! replays leniently.
//!
//! A version (`design_versions`) is the whole node list as `design.get` gives it, saved by name
//! or kept by the engine before an agent takes over a design someone else changed last, and
//! before a restore. `design.code` writes a frame out through [`crate::design_code`];
//! `design.render` only announces a request, which Relay's window answers with a picture.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{actor_name, assert_actor_project, check_project};
use base64::Engine as _;
use relay_bus::design::{Node, NodeKind, NodeOp, MAX_NAME, MAX_NODES, MAX_OPS};
use relay_bus::error::BusError;
use relay_bus::ops::design::*;
use relay_bus::types::{Design, DesignVersion, Id};
use relay_bus::{Empty, ErrorKind};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;

/// The most node JSON one design holds, in bytes. `design.get` returns it whole, and the
/// clients' sockets cap a line at 2 MiB (RA-227): this leaves room for the ids and the row.
const MAX_BYTES: i64 = 1536 * 1024;
/// The largest `design.apply` undo the audit keeps for `audit.undo`.
const UNDO_KEPT: usize = 256 * 1024;
const DEFAULT_TITLE: &str = "Untitled design";
/// The largest picture `design.asset.add` copies in from a path, and takes as `bytes_b64`.
const ASSET_FILE_MAX: u64 = 20 * 1024 * 1024;
const ASSET_B64_MAX: usize = 1024 * 1024;
/// The most JSON one version holds: a design at its node limit with its ids and parents, short
/// of the 2 MiB line `design.version.get` answers in.
const VERSION_MAX: usize = 1792 * 1024;
/// Automatic versions kept per design, the newest; named ones stay until deleted.
const AUTO_KEPT: i64 = 30;
/// Pictures `design.code` inlines as `data:` URLs, in all, before the rest keep their paths.
const INLINE_MAX: usize = 1024 * 1024;
/// How long an agent waits for `design.rendered` before taking Relay's window to be closed.
const RENDER_WAIT_MS: u64 = 20_000;

macro_rules! design_select {
    ($rest:literal) => {
        concat!(
            "SELECT id, project_id, session, title, rev, pages, nodes, created_by, updated_by, created_at, updated_at, deleted_at
             FROM designs ",
            $rest
        )
    };
}

fn design_row(row: &Row) -> rusqlite::Result<Design> {
    Ok(Design {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session: row.get(2)?,
        title: row.get(3)?,
        rev: row.get(4)?,
        pages: row.get(5)?,
        nodes: row.get(6)?,
        created_by: row.get(7)?,
        updated_by: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        deleted_at: row.get(11)?,
    })
}

/// Design `id`, deleted or not.
fn get_design(conn: &Connection, id: Id) -> Result<Design, BusError> {
    conn.prepare_cached(design_select!("WHERE id = ?1")).bus()?
        .query_row([id], design_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("design.not_found", format!("no design {id}")))
}

/// Design `id` if it is not deleted: what every change to one needs.
fn live_design(conn: &Connection, id: Id) -> Result<Design, BusError> {
    let design = get_design(conn, id)?;
    if design.deleted_at.is_some() {
        return Err(BusError::conflict("design.deleted", format!("design {id} is deleted"))
            .with_hint("design.restore brings it back"));
    }
    Ok(design)
}

fn emit_design(ctx: &mut Ctx, design: &Design) -> Result<(), BusError> {
    ctx.set_project(design.project_id);
    ctx.emit("design.changed", serde_json::to_value(design).bus()?);
    Ok(())
}

/// `raw` trimmed, or `default` when there is none; 1 to [`MAX_NAME`] characters either way.
fn title(raw: Option<&str>, default: &str) -> Result<String, BusError> {
    let title = raw.map_or(default, str::trim);
    let n = title.chars().count();
    if n == 0 || n > MAX_NAME {
        return Err(BusError::invalid("design.title", format!("a design title is 1 to {MAX_NAME} characters")));
    }
    Ok(title.to_string())
}

/// "<title> copy", the original cut short enough that the copy's title is still a title.
fn copy_title(original: &str) -> String {
    let kept: String = original.chars().take(MAX_NAME - " copy".len()).collect();
    format!("{} copy", kept.trim_end())
}

/// The terminal a design is tied to: an agent's own (layer 2 refuses another's name before the
/// handler runs), the person's any session the project has had.
fn check_session(ctx: &Ctx, project_id: Id, session: &str) -> Result<(), BusError> {
    if let Some(own) = ctx.actor.session_name() {
        return if own == session { Ok(()) } else { Err(BusError::not_own("session")) };
    }
    let known: bool = ctx.tx()
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM sessions WHERE project_id = ?1 AND name = ?2)").bus()?
        .query_row(params![project_id, session], |r| r.get(0)).bus()?;
    if !known {
        return Err(BusError::invalid("design.session", format!("project {project_id} has no session {session:?}"))
            .with_hint("name one of the project's sessions (session.list), or none"));
    }
    Ok(())
}

fn kind_str(kind: NodeKind) -> &'static str {
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

fn kind_of(text: &str) -> Result<NodeKind, BusError> {
    Ok(match text {
        "page" => NodeKind::Page,
        "frame" => NodeKind::Frame,
        "group" => NodeKind::Group,
        "rect" => NodeKind::Rect,
        "ellipse" => NodeKind::Ellipse,
        "line" => NodeKind::Line,
        "text" => NodeKind::Text,
        "sticky" => NodeKind::Sticky,
        "path" => NodeKind::Path,
        "image" => NodeKind::Image,
        "polygon" => NodeKind::Polygon,
        "icon" => NodeKind::Icon,
        "vector" => NodeKind::Vector,
        "comment" => NodeKind::Comment,
        other => return Err(BusError::internal(format!("a design node of unknown kind {other:?}"))),
    })
}

/// A node's JSON as stored: everything but its id and parent, which are columns.
fn body_of(node: &mut Node) -> Result<String, BusError> {
    let (id, parent) = (std::mem::take(&mut node.id), node.parent.take());
    let body = serde_json::to_string(node).bus();
    (node.id, node.parent) = (id, parent);
    body
}

/// The node a row holds.
fn node_of(id: String, parent: Option<String>, body: &str) -> Result<Node, BusError> {
    let mut node: Node = serde_json::from_str(body)
        .map_err(|e| BusError::internal(format!("design node {id:?} is unreadable: {e}")))?;
    (node.id, node.parent) = (id, parent);
    Ok(node)
}

/// `set` with the `width`/`height` spellings an `add` accepts renamed to the fields they fill,
/// so a patch never holds both names of one field and its undo restores the field it changed.
fn canonical(set: Map<String, Value>) -> Map<String, Value> {
    if !set.contains_key("width") && !set.contains_key("height") {
        return set;
    }
    set.into_iter()
        .map(|(key, value)| match key.as_str() {
            "width" => ("w".to_string(), value),
            "height" => ("h".to_string(), value),
            _ => (key, value),
        })
        .collect()
}

/// A node row: where it sits, what it is, and its JSON.
struct Stored {
    parent: Option<String>,
    kind: NodeKind,
    ord: f64,
    body: String,
}

/// One node row as `design.get` copies it out under the lock; parsed once the lock is gone.
struct Raw {
    id: String,
    parent: Option<String>,
    ord: f64,
    body: String,
}

/// `rows` in `design.get`'s order: the pages by `ord`, then each page's nodes in pre-order,
/// siblings bottom to top. With `page`, that page and what is on it. A node whose parent is
/// gone (none should be) is left out rather than shown nowhere.
fn ordered(rows: Vec<Raw>, page: Option<&str>) -> Result<Vec<Node>, BusError> {
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut pages = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match row.parent.as_deref() {
            Some(parent) => children.entry(parent).or_default().push(i),
            None => pages.push(i),
        }
    }
    let by_ord = |list: &mut Vec<usize>| list.sort_by(|&a, &b| rows[a].ord.total_cmp(&rows[b].ord).then_with(|| rows[a].id.cmp(&rows[b].id)));
    by_ord(&mut pages);
    children.values_mut().for_each(by_ord);
    if let Some(page) = page {
        pages.retain(|&i| rows[i].id == page);
        if pages.is_empty() {
            return Err(BusError::not_found("design.page_not_found", format!("the design has no page {page:?}")));
        }
    }
    let mut order = pages.clone();
    let mut stack = Vec::new();
    for &p in &pages {
        stack.extend(children.get(rows[p].id.as_str()).into_iter().flatten().rev());
        while let Some(i) = stack.pop() {
            order.push(i);
            stack.extend(children.get(rows[i].id.as_str()).into_iter().flatten().rev());
        }
    }
    drop(children);
    let mut rows: Vec<Option<Raw>> = rows.into_iter().map(Some).collect();
    order.into_iter()
        .filter_map(|i| rows[i].take())
        .map(|row| node_of(row.id, row.parent, &row.body))
        .collect()
}

/// Where edit `at` of a `design.apply` stands, for its refusals: `ops[3] update "hero"`.
fn label(at: usize, op: &NodeOp) -> String {
    match op {
        NodeOp::Add { node, .. } if node.id.is_empty() => format!("ops[{at}] add {}", kind_str(node.kind)),
        NodeOp::Add { node, .. } => format!("ops[{at}] add {:?}", node.id),
        NodeOp::Update { id, .. } => format!("ops[{at}] update {id:?}"),
        NodeOp::Move { id, .. } => format!("ops[{at}] move {id:?}"),
        NodeOp::Delete { id } => format!("ops[{at}] delete {id:?}"),
        NodeOp::Insert { item, .. } => format!("ops[{at}] insert {item:?}"),
    }
}

/// The fields `copy` must take from `master` to follow it: everything that differs except
/// the copy's own (`OWN_FIELDS`, an instance root's place) and what it overrode.
fn follow(master: &Node, copy: &Node, root: bool) -> Map<String, Value> {
    let (Ok(Value::Object(m)), Ok(Value::Object(c))) = (serde_json::to_value(master), serde_json::to_value(copy)) else {
        return Map::new();
    };
    let own = copy.overridden.clone().unwrap_or_default();
    let keep = |k: &str| {
        relay_bus::design::OWN_FIELDS.contains(&k)
            || (root && (relay_bus::design::ROOT_FIELDS.contains(&k) || k == "name"))
            || own.iter().any(|o| o == k)
            || k == "from"
            || k == "to"
    };
    let mut set = Map::new();
    for (k, v) in &m {
        if !keep(k) && c.get(k) != Some(v) {
            set.insert(k.clone(), v.clone());
        }
    }
    for k in c.keys() {
        if !keep(k) && !m.contains_key(k) {
            set.insert(k.clone(), Value::Null);
        }
    }
    set
}

/// Every node of a design in `design.get`'s order, read inside a transaction.
fn all_nodes(tx: &Connection, design: Id) -> Result<Vec<Node>, BusError> {
    let mut stmt = tx.prepare_cached("SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1").bus()?;
    let rows = stmt.query_map([design], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, body: r.get(3)? })).bus()?
        .collect::<rusqlite::Result<Vec<_>>>().bus()?;
    ordered(rows, None)
}

/// A design's versions, newest first, without their bodies.
const VERSIONS_OF: &str = "SELECT id, design_id, rev, label, auto, created_by, created_at, nodes FROM design_versions WHERE design_id = ?1 ORDER BY id DESC";

fn version_row(row: &Row) -> rusqlite::Result<DesignVersion> {
    Ok(DesignVersion {
        id: row.get(0)?,
        design_id: row.get(1)?,
        rev: row.get(2)?,
        label: row.get(3)?,
        auto: row.get(4)?,
        created_by: row.get(5)?,
        created_at: row.get(6)?,
        nodes: row.get(7)?,
    })
}

/// Version `version_id` of design `design_id`, and its body.
fn get_version(conn: &Connection, design_id: Id, version_id: Id) -> Result<(DesignVersion, String), BusError> {
    conn.prepare_cached("SELECT id, design_id, rev, label, auto, created_by, created_at, nodes, body FROM design_versions WHERE id = ?1 AND design_id = ?2").bus()?
        .query_row(params![version_id, design_id], |r| Ok((version_row(r)?, r.get::<_, String>(8)?))).optional().bus()?
        .ok_or_else(|| BusError::not_found("design.version_not_found", format!("design {design_id} has no version {version_id}"))
            .with_hint("design.version.list names its versions"))
}

/// `label`, or when it was saved: `2026-10-16 14:03` of an RFC 3339 time.
fn version_name(version: &DesignVersion) -> String {
    version.label.clone().unwrap_or_else(|| version.created_at.replacen('T', " ", 1).chars().take(16).collect())
}

/// `raw` cut to a label's length.
fn clip_label(raw: &str) -> String {
    raw.chars().take(MAX_NAME).collect()
}

/// Keep `design`'s nodes as they stand now as a version; `None` when they are too big for one,
/// which a caller that only keeps one for safety passes over and one asked to save refuses.
fn save_version(tx: &Connection, design: &Design, label: Option<&str>, auto: bool, by: &str, now: &str) -> Result<Option<DesignVersion>, BusError> {
    let nodes = all_nodes(tx, design.id)?;
    let body = serde_json::to_string(&nodes).bus()?;
    if body.len() > VERSION_MAX {
        return Ok(None);
    }
    let label = label.map(clip_label);
    tx.prepare_cached("INSERT INTO design_versions(design_id, rev, label, auto, created_by, created_at, nodes, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)").bus()?
        .execute(params![design.id, design.rev, label, auto, by, now, nodes.len() as i64, body]).bus()?;
    let id = tx.last_insert_rowid();
    if auto {
        tx.prepare_cached("DELETE FROM design_versions WHERE design_id = ?1 AND auto = 1 AND id NOT IN
            (SELECT id FROM design_versions WHERE design_id = ?1 AND auto = 1 ORDER BY id DESC LIMIT ?2+0)").bus()?
            .execute(params![design.id, AUTO_KEPT]).bus()?;
    }
    Ok(Some(DesignVersion { id, design_id: design.id, rev: design.rev, label, auto, created_by: by.to_string(), created_at: now.to_string(), nodes: nodes.len() as i64 }))
}

fn too_large_to_keep(design: Id) -> BusError {
    BusError::invalid("design.too_large", format!("design {design} is over {} KiB with its ids: too big to keep a version of", VERSION_MAX >> 10))
        .with_hint("split it across designs")
}

fn emit_version(ctx: &mut Ctx, project_id: Id, version: &DesignVersion) -> Result<(), BusError> {
    ctx.set_project(project_id);
    ctx.emit("design.version.changed", serde_json::to_value(version).bus()?);
    Ok(())
}

/// Replace design `design`'s nodes with `nodes` (in `design.get`'s order, so each parent comes
/// before its children and siblings bottom to top). Returns the pages, nodes and bytes now held
/// and the highest number an id ends in, for the id counter.
fn put_nodes(tx: &Connection, design: Id, nodes: Vec<Node>) -> Result<(i64, i64, i64, i64), BusError> {
    tx.prepare_cached("DELETE FROM design_nodes WHERE design_id = ?1").bus()?.execute([design]).bus()?;
    let mut insert = tx.prepare_cached("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)").bus()?;
    let mut next_ord: HashMap<Option<String>, f64> = HashMap::new();
    let (mut pages, mut count, mut bytes, mut top) = (0, 0, 0, 0);
    for mut node in nodes {
        let ord = next_ord.entry(node.parent.clone()).or_insert(0.0);
        *ord += 1.0;
        let ord = *ord;
        let body = body_of(&mut node)?;
        insert.execute(params![design, node.id, node.parent, kind_str(node.kind), ord, body]).bus()?;
        pages += i64::from(node.kind == NodeKind::Page);
        count += 1;
        bytes += body.len() as i64;
        if let Some(n) = node.id.rsplit_once('-').and_then(|(_, n)| n.parse::<i64>().ok()) {
            top = top.max(n);
        }
    }
    Ok((pages, count, bytes, top))
}

/// `design.code`'s pictures: a path beside the page, or the picture itself while the budget
/// lasts.
fn code_asset(dir: &Path, inline: bool, budget: &mut usize, src: &str) -> String {
    if inline && relay_bus::design::valid_src(src) {
        if let Ok(bytes) = std::fs::read(dir.join(src)) {
            if bytes.len() <= *budget {
                *budget -= bytes.len();
                let mime = match src.rsplit('.').next() {
                    Some("png") => "image/png",
                    Some("jpg") => "image/jpeg",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "image/svg+xml",
                };
                return format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
            }
        }
    }
    format!("design-assets/{src}")
}

/// One `design.apply` under way: the design's counters as its edits change them, and what the
/// answer reports. Every edit reads and writes the transaction, so the next one sees it.
struct Run<'a> {
    tx: &'a Connection,
    design: Id,
    /// Who is editing: a comment added without an author is theirs.
    by: String,
    lenient: bool,
    pages: i64,
    nodes: i64,
    bytes: i64,
    next_node: i64,
    /// The edit under way: its index and [`label`].
    at: usize,
    label: String,
    /// Nodes an edit added, changed or moved, in the order first touched.
    touched: Vec<String>,
    seen: HashSet<String>,
    removed: Vec<String>,
    ids: Vec<String>,
    /// Each applied edit's undo, in edit order; a delete's is the whole subtree's re-adds.
    inverse: Vec<Vec<NodeOp>>,
    /// Components an edit touched: their instances follow once the list is done.
    dirty: HashSet<String>,
    /// Shared styles an edit changed, and nodes that took up a style: their fields follow.
    restyle: HashSet<String>,
    pending: HashSet<String>,
    /// Nodes known to sit in no component (themselves and every ancestor), so the adds of a
    /// big list don't walk the same parents again. Cleared when the tree's shape or a
    /// component flag changes.
    plain: HashSet<String>,
    /// Edits made by following a component, which a copy does not remember as its own.
    syncing: bool,
}

const NODE_META: &str = "SELECT parent_id, kind, ord FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
const NODE_ROW: &str = "SELECT parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
/// Siblings in paint order from `?4`, leaving out `?3` (the node being moved, if any).
const SIBLING_ORDS: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord, node_id LIMIT 2 OFFSET ?4+0";
const TOP_ORD: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord DESC, node_id DESC LIMIT 1";

impl Run<'_> {
    fn fail(&self, kind: ErrorKind, code: &str, why: impl std::fmt::Display) -> BusError {
        BusError::new(kind, code, format!("{}: {why}", self.label)).with_details(json!({"op": self.at}))
    }

    /// The node an edit names is not there: skipped when lenient, else refused.
    fn missing(&self, what: impl std::fmt::Display) -> Result<Option<String>, BusError> {
        if self.lenient {
            return Ok(None);
        }
        Err(self.fail(ErrorKind::NotFound, "design.node_not_found", what))
    }

    fn meta(&self, id: &str) -> Result<Option<(Option<String>, NodeKind, f64)>, BusError> {
        let found: Option<(Option<String>, String, f64)> = self.tx.prepare_cached(NODE_META).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().bus()?;
        found.map(|(parent, kind, ord)| Ok((parent, kind_of(&kind)?, ord))).transpose()
    }

    fn row(&self, id: &str) -> Result<Option<Stored>, BusError> {
        let found: Option<(Option<String>, String, f64, String)> = self.tx.prepare_cached(NODE_ROW).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().bus()?;
        found.map(|(parent, kind, ord, body)| Ok(Stored { parent, kind: kind_of(&kind)?, ord, body })).transpose()
    }

    fn touch(&mut self, id: &str) {
        if self.seen.insert(id.to_string()) {
            self.touched.push(id.to_string());
        }
    }

    /// The `ord` that puts a node at `index` among `parent`'s children (the pages, for `None`),
    /// `skip` aside: 0 at the bottom, absent or past the end on top.
    fn slot(&self, parent: Option<&str>, index: Option<u32>, skip: Option<&str>) -> Result<f64, BusError> {
        let top = || -> Result<f64, BusError> {
            let last: Option<f64> = self.tx.prepare_cached(TOP_ORD).bus()?
                .query_row(params![self.design, parent, skip], |r| r.get(0)).optional().bus()?;
            Ok(last.map_or(1.0, |ord| ord + 1.0))
        };
        let Some(index) = index else { return top() };
        let around: Vec<f64> = self.tx.prepare_cached(SIBLING_ORDS).bus()?
            .query_map(params![self.design, parent, skip, i64::from(index.saturating_sub(1))], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        match (index, around.as_slice()) {
            (0, []) => Ok(1.0),
            (0, [first, ..]) => Ok(first - 1.0),
            (_, []) => top(),
            (_, [last]) => Ok(last + 1.0),
            (_, &[below, above, ..]) => {
                let mid = below + (above - below) / 2.0;
                if above - below > 1e-9 && below < mid && mid < above {
                    return Ok(mid);
                }
                // The gap is spent: space the siblings 1 apart and look again.
                self.renumber(parent)?;
                self.slot(parent, Some(index), skip)
            }
        }
    }

    fn renumber(&self, parent: Option<&str>) -> Result<(), BusError> {
        let ids: Vec<String> = self.tx
            .prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 ORDER BY ord, node_id").bus()?
            .query_map(params![self.design, parent], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let mut set = self.tx.prepare_cached("UPDATE design_nodes SET ord = ?3 WHERE design_id = ?1 AND node_id = ?2").bus()?;
        for (i, id) in ids.iter().enumerate() {
            set.execute(params![self.design, id, (i + 1) as f64]).bus()?;
        }
        Ok(())
    }

    /// Where node `id` (at `ord`) sits among its siblings: how many paint below it.
    fn index_of(&self, parent: Option<&str>, id: &str, ord: f64) -> Result<u32, BusError> {
        let below: i64 = self.tx
            .prepare_cached("SELECT COUNT(*) FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2
                AND (ord < ?3 OR (ord = ?3 AND node_id < ?4))").bus()?
            .query_row(params![self.design, parent, ord, id], |r| r.get(0)).bus()?;
        Ok(below as u32)
    }

    /// `{kind}-{n}` from the design's counter, past any id already taken.
    fn assign(&mut self, kind: NodeKind) -> Result<String, BusError> {
        loop {
            let id = format!("{}-{}", kind_str(kind), self.next_node);
            self.next_node += 1;
            if self.meta(&id)?.is_none() {
                return Ok(id);
            }
        }
    }

    /// `parent` can take a child: it is there and is a page, frame or group. `Ok(false)` when
    /// it is gone and the list is lenient.
    fn container(&self, parent: &str) -> Result<bool, BusError> {
        match self.meta(parent)? {
            None if self.lenient => Ok(false),
            None => Err(self.fail(ErrorKind::NotFound, "design.parent", format!("parent {parent:?} is not in the design"))),
            Some((_, kind, _)) if !kind.holds_children() => Err(self.fail(ErrorKind::Invalid, "design.parent",
                format!("parent {parent:?} is a {}, which holds nothing; use a page, frame or group", kind_str(kind)))),
            Some(_) => Ok(true),
        }
    }

    fn add(&mut self, mut node: Node, index: Option<u32>) -> Result<Option<String>, BusError> {
        node.validate().map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        self.sign(&mut node);
        if let Some(parent) = node.parent.as_deref() {
            if !self.container(parent)? {
                return Ok(None);
            }
        }
        if node.id.is_empty() {
            node.id = self.assign(node.kind)?;
        } else if self.meta(&node.id)?.is_some() {
            return Err(self.fail(ErrorKind::Conflict, "design.node_exists", format!("{:?} is already in the design", node.id))
                .with_hint("update or move that node, or leave the id out to have one assigned"));
        }
        if self.nodes >= MAX_NODES as i64 {
            return Err(self.fail(ErrorKind::Invalid, "design.too_large", format!("a design holds at most {MAX_NODES} nodes")));
        }
        let ord = self.slot(node.parent.as_deref(), index, None)?;
        let body = body_of(&mut node)?;
        self.tx.prepare_cached("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)").bus()?
            .execute(params![self.design, node.id, node.parent, kind_str(node.kind), ord, body]).bus()?;
        self.nodes += 1;
        self.pages += i64::from(node.kind == NodeKind::Page);
        self.bytes += body.len() as i64;
        self.touch(&node.id);
        self.inverse.push(vec![NodeOp::Delete { id: node.id.clone() }]);
        if node.component == Some(true) {
            self.dirty.insert(node.id.clone());
        } else if let Some(parent) = node.parent.clone() {
            self.mark(&parent)?;
        }
        self.note_styles(&node, None);
        Ok(Some(node.id))
    }

    fn update(&mut self, id: String, set: Map<String, Value>) -> Result<Option<String>, BusError> {
        let Some(row) = self.row(&id)? else { return self.missing("no such node") };
        let node = node_of(id, row.parent, &row.body)?;
        let set = canonical(set);
        let mut next = node.patched(&set).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        // A copy edited by hand keeps what was changed, whatever its component does next.
        if !self.syncing && next.master.is_some() && !set.contains_key("overridden") {
            let mut own = next.overridden.take().unwrap_or_default();
            for field in relay_bus::design::overriding(&set) {
                if !own.iter().any(|f| f == field) {
                    own.push(field.to_string());
                }
            }
            next.overridden = (!own.is_empty()).then_some(own);
        }
        // A field set by hand that a shared style lends lets go of the style.
        let mut let_go = Vec::new();
        if !self.syncing {
            for (which, fields) in relay_bus::design::STYLE_FIELDS {
                if next.style_ref(which).is_some() && !set.contains_key(*which) && set.keys().any(|k| fields.contains(&k.as_str())) {
                    let_go.push((*which, node.style_ref(which).map(str::to_string)));
                    next.clear_style_ref(which);
                }
            }
        }
        self.sign(&mut next);
        let body = body_of(&mut next)?;
        if body != row.body {
            self.tx.prepare_cached("UPDATE design_nodes SET body = ?3 WHERE design_id = ?1 AND node_id = ?2").bus()?
                .execute(params![self.design, next.id, body]).bus()?;
            self.bytes += body.len() as i64 - row.body.len() as i64;
        }
        self.touch(&next.id);
        let mut undo = node.before(&set);
        // Undoing a hand edit of a copy takes its override back with it.
        if next.overridden != node.overridden && !undo.contains_key("overridden") {
            undo.insert("overridden".into(), serde_json::to_value(&node.overridden).unwrap_or(Value::Null));
        }
        for (which, was) in let_go {
            undo.insert(which.into(), was.map_or(Value::Null, Value::String));
        }
        self.inverse.push(vec![NodeOp::Update { id: next.id.clone(), set: undo }]);
        if set.contains_key("component") {
            self.plain.clear();
        }
        self.mark(&next.id)?;
        self.note_styles(&next, Some(&set));
        // Overrides reset on a copy: its component's values come back.
        if set.contains_key("overridden") && next.master.is_some() {
            let mut at = Some(next.id.clone());
            while let Some(current) = at {
                let Some(n) = self.node(&current)? else { break };
                if let Some(c) = n.instance_of {
                    self.dirty.insert(c);
                    break;
                }
                at = n.parent;
            }
        }
        Ok(Some(next.id))
    }

    fn move_(&mut self, id: String, parent: Option<String>, index: Option<u32>) -> Result<Option<String>, BusError> {
        let Some((from, kind, ord)) = self.meta(&id)? else { return self.missing("no such node") };
        let to = if kind == NodeKind::Page {
            if parent.is_some() {
                return Err(self.fail(ErrorKind::Invalid, "design.parent", "a page sits in no parent; a move only reorders it among the pages"));
            }
            None
        } else {
            let to = parent.or_else(|| from.clone()).unwrap_or_default();
            if to == id {
                return Err(self.fail(ErrorKind::Invalid, "design.cycle", "a node cannot sit inside itself"));
            }
            if !self.container(&to)? {
                return Ok(None);
            }
            // Into another parent: not one inside the node itself.
            if from.as_deref() != Some(to.as_str()) {
                let mut up = self.meta(&to)?.and_then(|(parent, _, _)| parent);
                while let Some(ancestor) = up {
                    if ancestor == id {
                        return Err(self.fail(ErrorKind::Invalid, "design.cycle", format!("{to:?} is inside {id:?}")));
                    }
                    up = self.meta(&ancestor)?.and_then(|(parent, _, _)| parent);
                }
            }
            Some(to)
        };
        let old_index = self.index_of(from.as_deref(), &id, ord)?;
        let ord = self.slot(to.as_deref(), index, Some(&id))?;
        self.tx.prepare_cached("UPDATE design_nodes SET parent_id = ?3, ord = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, id, to, ord]).bus()?;
        self.plain.clear();
        if let Some(old) = &from {
            self.mark(old)?;
        }
        self.touch(&id);
        self.inverse.push(vec![NodeOp::Move { id: id.clone(), parent: from, index: Some(old_index) }]);
        self.mark(&id)?;
        Ok(Some(id))
    }

    /// A pack item's nodes added one by one (its root at `index`), undone by deleting its root.
    #[allow(clippy::too_many_arguments)]
    fn insert(&mut self, item: String, parent: String, x: f64, y: f64, index: Option<u32>, id: Option<String>, set: Option<Map<String, Value>>) -> Result<Option<String>, BusError> {
        if let Some(component) = item.strip_prefix('#') {
            return self.insert_instance(component, parent, x, y, index, id, set);
        }
        let Some((_, found)) = relay_bus::kits::item(&item) else {
            return Err(self.fail(ErrorKind::NotFound, "design.kit_not_found", format!("no pack item {item:?}"))
                .with_hint("design.kits lists the packs and their items as kit/item, and #<id> names one of this design's components"));
        };
        let root = match id {
            Some(id) => {
                if self.meta(&id)?.is_some() {
                    return Err(self.fail(ErrorKind::Conflict, "design.node_exists", format!("{id:?} is already in the design")));
                }
                id
            }
            None => loop {
                let id = format!("{}-{}", found.id, self.next_node);
                self.next_node += 1;
                if self.meta(&id)?.is_none() {
                    break id;
                }
            },
        };
        let nodes = relay_bus::kits::expand(found, &root, &parent, x, y, set.as_ref())
            .map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        let before = self.inverse.len();
        for (i, node) in nodes.into_iter().enumerate() {
            if self.add(node, if i == 0 { index } else { None })?.is_none() {
                return Ok(None);
            }
        }
        self.inverse.truncate(before);
        self.inverse.push(vec![NodeOp::Delete { id: root.clone() }]);
        Ok(Some(root))
    }

    /// An instance of one of this design's components: copies that follow it.
    #[allow(clippy::too_many_arguments)]
    fn insert_instance(&mut self, component: &str, parent: String, x: f64, y: f64, index: Option<u32>, id: Option<String>, set: Option<Map<String, Value>>) -> Result<Option<String>, BusError> {
        let subtree = self.subtree(component)?;
        if subtree.first().is_none_or(|n| n.component != Some(true)) {
            return Err(self.fail(ErrorKind::NotFound, "design.not_component", format!("{component:?} is not a component of this design"))
                .with_hint("make a frame a component by setting component: true"));
        }
        let root = match id {
            Some(id) => id,
            None => loop {
                let id = format!("{component}-copy-{}", self.next_node);
                self.next_node += 1;
                if self.meta(&id)?.is_none() {
                    break id;
                }
            },
        };
        if self.meta(&root)?.is_some() {
            return Err(self.fail(ErrorKind::Conflict, "design.node_exists", format!("{root:?} is already in the design")));
        }
        let nodes = relay_bus::kits::instance(&subtree, &root, &parent, x, y, set.as_ref())
            .map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        let before = self.inverse.len();
        for (i, node) in nodes.into_iter().enumerate() {
            if self.add(node, if i == 0 { index } else { None })?.is_none() {
                return Ok(None);
            }
        }
        self.inverse.truncate(before);
        self.inverse.push(vec![NodeOp::Delete { id: root.clone() }]);
        Ok(Some(root))
    }

    fn node(&self, id: &str) -> Result<Option<Node>, BusError> {
        self.row(id)?.map(|row| node_of(id.to_string(), row.parent, &row.body)).transpose()
    }

    /// `id` and everything inside it as nodes, parents first, siblings in paint order.
    fn subtree(&self, id: &str) -> Result<Vec<Node>, BusError> {
        let Some(root) = self.node(id)? else {
            return Ok(Vec::new());
        };
        let mut out = vec![root];
        let mut next = 0;
        let mut children = self.tx
            .prepare_cached("SELECT node_id, parent_id, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id").bus()?;
        while next < out.len() {
            if out[next].kind.holds_children() {
                let found = children.query_map(params![self.design, out[next].id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))).bus()?
                    .collect::<rusqlite::Result<Vec<_>>>().bus()?;
                for (cid, parent, body) in found {
                    out.push(node_of(cid, parent, &body)?);
                }
            }
            next += 1;
        }
        Ok(out)
    }

    /// Note the component `id` sits in (itself, or an ancestor), so its copies follow.
    fn mark(&mut self, id: &str) -> Result<(), BusError> {
        let mut at = Some(id.to_string());
        let mut depth = 0;
        let mut walked = Vec::new();
        // One indexed row per ancestor, its body unparsed: every add of a big list walks this.
        let mut up = self.tx
            .prepare_cached("SELECT parent_id, json_extract(body, '$.component') IS 1 FROM design_nodes WHERE design_id = ?1 AND node_id = ?2").bus()?;
        while let Some(current) = at {
            if self.plain.contains(&current) {
                break;
            }
            let Some((parent, component)) = up.query_row(params![self.design, current], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, bool>(1)?))).optional().bus()? else {
                break;
            };
            if component {
                self.dirty.insert(current);
                return Ok(());
            }
            walked.push(current);
            at = parent;
            depth += 1;
            if depth > 64 {
                return Ok(());
            }
        }
        self.plain.extend(walked);
        Ok(())
    }

    /// A comment, and each reply, without an author is the editor's.
    fn sign(&self, node: &mut Node) {
        if node.kind == NodeKind::Comment && node.author.is_none() {
            node.author = Some(self.by.clone());
        }
        for reply in node.replies.iter_mut().flatten() {
            if reply.author.is_empty() {
                reply.author = self.by.clone();
            }
        }
    }

    /// A shared style changed, or a node took one up (added with it, or `set` naming it).
    fn note_styles(&mut self, node: &Node, set: Option<&Map<String, Value>>) {
        if node.shared == Some(true) {
            self.restyle.insert(node.id.clone());
        }
        let took = relay_bus::design::STYLE_FIELDS.iter().any(|(which, _)| {
            node.style_ref(which).is_some() && set.is_none_or(|s| s.contains_key(*which))
        });
        if took {
            self.pending.insert(node.id.clone());
        }
    }

    /// Every node naming a changed style, and every node that took one up, given its fields.
    /// Taking one up is part of the edit and of its undo; following a changed style is not
    /// (undoing the style's edit makes its users follow it back).
    fn sync_styles(&mut self) -> Result<(), BusError> {
        if self.restyle.is_empty() && self.pending.is_empty() {
            return Ok(());
        }
        let mut took: Vec<String> = std::mem::take(&mut self.pending).into_iter().collect();
        took.sort();
        let mut followers = Vec::new();
        for style in std::mem::take(&mut self.restyle) {
            let mut stmt = self.tx.prepare_cached(
                "SELECT node_id FROM design_nodes WHERE design_id = ?1 AND (json_extract(body, '$.fill_style') = ?2 OR json_extract(body, '$.text_style') = ?2 OR json_extract(body, '$.effect_style') = ?2)",
            ).bus()?;
            let found = stmt.query_map(params![self.design, style], |r| r.get::<_, String>(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            followers.extend(found.into_iter().filter(|id| took.binary_search(id).is_err()));
        }
        followers.sort();
        followers.dedup();
        let was = std::mem::replace(&mut self.syncing, true);
        for id in took {
            self.restyle_node(id)?;
        }
        let undo_len = self.inverse.len();
        for id in followers {
            self.restyle_node(id)?;
        }
        self.inverse.truncate(undo_len);
        self.syncing = was;
        Ok(())
    }

    fn restyle_node(&mut self, id: String) -> Result<(), BusError> {
        let Some(node) = self.node(&id)? else { return Ok(()) };
        let mut set = Map::new();
        for (which, _) in relay_bus::design::STYLE_FIELDS {
            let Some(style) = node.style_ref(which) else { continue };
            if let Some(style) = self.node(style)?.filter(|s| s.shared == Some(true)) {
                set.extend(relay_bus::design::styled(&style, &node, which));
            }
        }
        if !set.is_empty() {
            self.update(id, set)?;
        }
        Ok(())
    }

    /// Bring every instance of the components this list touched up to date: the component's
    /// fields onto each copy except those the copy changed itself, copies added for new parts
    /// and removed for parts gone. Nested components follow in turn, a few levels deep.
    fn sync(&mut self) -> Result<(), BusError> {
        let mut rounds = 0;
        while !self.dirty.is_empty() && rounds < 4 {
            let components: Vec<String> = self.dirty.drain().collect();
            for c in components {
                self.sync_component(&c)?;
            }
            rounds += 1;
        }
        Ok(())
    }

    fn sync_component(&mut self, component: &str) -> Result<(), BusError> {
        let master = self.subtree(component)?;
        if master.first().is_none_or(|n| n.component != Some(true)) {
            return Ok(());
        }
        let instances: Vec<String> = self.tx
            .prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND json_extract(body, '$.instance_of') = ?2").bus()?
            .query_map(params![self.design, component], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        // What following changes is no part of the undo: undoing the component's edit makes its
        // copies follow it back, where replaying their own edits would mark them overridden.
        let was = std::mem::replace(&mut self.syncing, true);
        let undo_len = self.inverse.len();
        for instance in instances {
            let copies = self.subtree(&instance)?;
            let mut copy_of: HashMap<String, String> = HashMap::new();
            for (i, m) in master.iter().enumerate() {
                let copy = if i == 0 {
                    copies.first()
                } else {
                    copies.iter().skip(1).find(|c| c.master.as_deref() == Some(m.id.as_str()))
                };
                match copy {
                    Some(c) => {
                        copy_of.insert(m.id.clone(), c.id.clone());
                        let set = follow(m, c, i == 0);
                        if !set.is_empty() {
                            self.update(c.id.clone(), set)?;
                        }
                    }
                    None => {
                        let Some(parent) = m.parent.as_ref().and_then(|p| copy_of.get(p)).cloned() else {
                            continue;
                        };
                        let mut n = 1;
                        let id = loop {
                            let id = format!("{instance}.{}", copies.len() + n);
                            if self.meta(&id)?.is_none() {
                                break id;
                            }
                            n += 1;
                        };
                        let mut node = m.clone();
                        node.id = id.clone();
                        node.parent = Some(parent);
                        node.master = Some(m.id.clone());
                        node.component = None;
                        node.overridden = None;
                        copy_of.insert(m.id.clone(), id);
                        self.add(node, None)?;
                    }
                }
            }
            for c in copies.iter().skip(1) {
                let gone = c.master.as_ref().is_some_and(|m| !master.iter().any(|n| n.id == *m));
                if gone && self.meta(&c.id)?.is_some() {
                    self.delete(c.id.clone())?;
                }
            }
        }
        self.inverse.truncate(undo_len);
        self.syncing = was;
        Ok(())
    }

    fn delete(&mut self, id: String) -> Result<Option<String>, BusError> {
        let Some(root) = self.row(&id)? else { return self.missing("no such node") };
        self.plain.clear();
        if root.kind == NodeKind::Page && self.pages <= 1 {
            return Err(self.fail(ErrorKind::Conflict, "design.last_page", "a design keeps at least one page")
                .with_hint("add another page first, or delete the design"));
        }
        let index = self.index_of(root.parent.as_deref(), &id, root.ord)?;
        if let Some(parent) = &root.parent {
            self.mark(parent)?;
        }
        // The subtree, each parent before its children and siblings bottom to top: the order
        // its undo adds them back in, so each lands where it was.
        let mut doomed = vec![(id, root)];
        let mut next = 0;
        let mut children = self.tx
            .prepare_cached("SELECT node_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id").bus()?;
        while next < doomed.len() {
            if doomed[next].1.kind.holds_children() {
                let parent = doomed[next].0.clone();
                let found = children.query_map(params![self.design, parent], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?))).bus()?
                    .collect::<rusqlite::Result<Vec<(String, String, f64, String)>>>().bus()?;
                for (child, kind, ord, body) in found {
                    doomed.push((child, Stored { parent: Some(parent.clone()), kind: kind_of(&kind)?, ord, body }));
                }
            }
            next += 1;
        }
        drop(children);
        let mut remove = self.tx.prepare_cached("DELETE FROM design_nodes WHERE design_id = ?1 AND node_id = ?2").bus()?;
        let mut undo = Vec::with_capacity(doomed.len());
        for (i, (node_id, row)) in doomed.into_iter().enumerate() {
            remove.execute(params![self.design, node_id]).bus()?;
            self.bytes -= row.body.len() as i64;
            self.removed.push(node_id.clone());
            let node = node_of(node_id, row.parent, &row.body)?;
            undo.push(NodeOp::Add { node, index: (i == 0).then_some(index) });
        }
        self.nodes -= undo.len() as i64;
        self.pages -= i64::from(matches!(&undo[0], NodeOp::Add { node, .. } if node.kind == NodeKind::Page));
        let id = undo[0].target().to_string();
        self.inverse.push(undo);
        Ok(Some(id))
    }
}

/// The node an asset name stands for, and its natural size: `design.asset.add`'s read phase,
/// done with the store unlocked.
struct StagedAsset {
    src: String,
    path: String,
    w: u32,
    h: u32,
}

/// The picture at `raw`: an absolute path to a regular file of at most [`ASSET_FILE_MAX`].
fn read_asset(raw: &str) -> Result<Vec<u8>, BusError> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(BusError::invalid("design.asset", format!("{raw:?} is not an absolute path")));
    }
    let meta = std::fs::metadata(path).map_err(|_| BusError::not_found("design.asset", format!("no file {raw}")))?;
    if !meta.is_file() {
        return Err(BusError::invalid("design.asset", format!("{raw} is not a regular file")));
    }
    let too_big = || BusError::invalid("design.asset", format!("{raw} is over {} MiB", ASSET_FILE_MAX >> 20));
    if meta.len() > ASSET_FILE_MAX {
        return Err(too_big());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    std::fs::File::open(path).bus()?.take(ASSET_FILE_MAX + 1).read_to_end(&mut bytes).bus()?;
    if bytes.len() as u64 > ASSET_FILE_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

/// `bytes_b64` decoded (line breaks allowed), at most [`ASSET_B64_MAX`] bytes.
fn decode_asset(encoded: &str) -> Result<Vec<u8>, BusError> {
    let compact: std::borrow::Cow<str> = if encoded.bytes().any(|b| b.is_ascii_whitespace()) {
        encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect::<String>().into()
    } else {
        encoded.into()
    };
    let too_big = || BusError::invalid("design.asset", format!("bytes_b64 is over {} KiB decoded", ASSET_B64_MAX >> 10))
        .with_hint("pass a larger picture as a path");
    if compact.len() > ASSET_B64_MAX.div_ceil(3) * 4 {
        return Err(too_big());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
        .map_err(|_| BusError::invalid("design.asset", "bytes_b64 is not base64"))?;
    if bytes.len() > ASSET_B64_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

/// Write `bytes` to `path` in `dir` unless it is there already (a name is its content's hash),
/// through a temporary file renamed into place, so a reader never sees half a picture.
fn store_asset(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), BusError> {
    if path.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).bus()?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).bus()?;
    tmp.write_all(bytes).bus()?;
    match tmp.persist_noclobber(path) {
        Ok(_) => Ok(()),
        // Another request stored the same picture first.
        Err(_) if path.is_file() => Ok(()),
        Err(error) => Err(crate::engine::internal(error.error)),
    }
}

pub fn register(e: &mut Engine) {
    // Unlocked (D149): pure reads the canvas repeats on every `design.changed`.
    e.register_unlocked::<List>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let sql = if p.include_deleted.unwrap_or(false) {
            design_select!("WHERE project_id = ?1 AND (?2 IS NULL OR session = ?2) ORDER BY updated_at DESC, id DESC")
        } else {
            design_select!("WHERE project_id = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR session = ?2) ORDER BY updated_at DESC, id DESC")
        };
        let designs = ctx.read(|conn| {
            check_project(conn, p.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(sql).bus()?;
            let designs = stmt.query_map(params![p.project_id, p.session], design_row).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(designs)
        })?;
        Ok(ListOut { designs })
    });
    // Only copying the rows out holds the store; ordering and parsing them runs after.
    e.register_unlocked::<Get>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (design, rows) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached("SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1").bus()?;
            let rows = stmt.query_map([design.id], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, body: r.get(3)? })).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok((design, rows))
        })?;
        let nodes = ordered(rows, p.page.as_deref())?;
        let assets_dir = ctx.engine().store.design_assets_dir().display().to_string();
        Ok(GetOut { design, nodes, assets_dir })
    });
    e.register::<Create>(|ctx: &mut Ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let by = actor_name(ctx)?;
        let title = title(p.title.as_deref(), DEFAULT_TITLE)?;
        // An agent's design is its own terminal's unless it says otherwise (which layer 2
        // refuses for any name but its own); the person's belongs to the project alone.
        let session = p.session.or_else(|| ctx.actor.session_name().map(str::to_string));
        if let Some(session) = &session {
            check_session(ctx, p.project_id, session)?;
        }
        let mut page = Node::new(NodeKind::Page);
        page.id = "page-1".into();
        page.name = Some("Page 1".into());
        let body = body_of(&mut page)?;
        ctx.tx().execute(
            "INSERT INTO designs(project_id, session, title, rev, pages, nodes, bytes, next_node, created_by, updated_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, 1, 1, ?4, 2, ?5, ?5, ?6, ?6)",
            params![p.project_id, session, title, body.len() as i64, by, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        ctx.tx().execute(
            "INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, NULL, 'page', 1.0, ?3)",
            params![id, page.id, body],
        ).bus()?;
        let mut nodes = vec![page];
        if let Some(template) = p.template {
            // A template is a pack item placed on the first page.
            let mut run = Run {
                tx: ctx.tx(),
                design: id,
                by: by.clone(),
                lenient: false,
                pages: 1,
                nodes: 1,
                bytes: body.len() as i64,
                next_node: 2,
                at: 0,
                label: format!("template {template:?}"),
                touched: Vec::new(),
                seen: HashSet::new(),
                removed: Vec::new(),
                ids: Vec::new(),
                inverse: Vec::new(),
                dirty: HashSet::new(),
                restyle: HashSet::new(),
                pending: HashSet::new(),
                plain: HashSet::new(),
                syncing: false,
            };
            run.insert(template, "page-1".into(), 0.0, 0.0, None, None, None)?;
            ctx.tx().execute(
                "UPDATE designs SET pages = ?2, nodes = ?3, bytes = ?4, next_node = ?5 WHERE id = ?1",
                params![id, run.pages, run.nodes, run.bytes, run.next_node],
            ).bus()?;
            nodes = all_nodes(ctx.tx(), id)?;
        }
        let design = get_design(ctx.tx(), id)?;
        ctx.set_undo("design.delete", json!({"design_id": id}), None);
        emit_design(ctx, &design)?;
        let assets_dir = ctx.engine().store.design_assets_dir().display().to_string();
        Ok(GetOut { design, nodes, assets_dir })
    });
    e.register::<Update>(|ctx: &mut Ctx, p| {
        let before = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, before.project_id)?;
        let by = actor_name(ctx)?;
        let title = match p.title.as_deref() {
            Some(raw) => title(Some(raw), DEFAULT_TITLE)?,
            None => before.title.clone(),
        };
        let session = match p.session {
            None => before.session.clone(),
            Some(None) => None,
            Some(Some(session)) => {
                check_session(ctx, before.project_id, &session)?;
                Some(session)
            }
        };
        ctx.tx().execute(
            "UPDATE designs SET title = ?2, session = ?3, rev = rev + 1, updated_by = ?4, updated_at = ?5 WHERE id = ?1",
            params![before.id, title, session, by, ctx.now],
        ).bus()?;
        let design = get_design(ctx.tx(), before.id)?;
        ctx.set_undo("design.update", json!({"design_id": before.id, "title": before.title, "session": before.session}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    e.register::<Duplicate>(|ctx: &mut Ctx, p| {
        let from = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, from.project_id)?;
        let by = actor_name(ctx)?;
        let title = match p.title.as_deref() {
            Some(raw) => title(Some(raw), DEFAULT_TITLE)?,
            None => copy_title(&from.title),
        };
        ctx.tx().execute(
            "INSERT INTO designs(project_id, session, title, rev, pages, nodes, bytes, next_node, created_by, updated_by, created_at, updated_at)
             SELECT project_id, session, ?2, 1, pages, nodes, bytes, next_node, ?3, ?3, ?4, ?4 FROM designs WHERE id = ?1",
            params![from.id, title, by, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        ctx.tx().execute(
            "INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body)
             SELECT ?1, node_id, parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?2",
            params![id, from.id],
        ).bus()?;
        let design = get_design(ctx.tx(), id)?;
        ctx.set_undo("design.delete", json!({"design_id": id}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    // Soft (BUS.md §5.4): the nodes stay until `design.purge` or the retention pass.
    e.register::<Delete>(|ctx: &mut Ctx, p| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        let by = actor_name(ctx)?;
        ctx.tx().execute(
            "UPDATE designs SET deleted_at = ?2, updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![design.id, ctx.now, by],
        ).bus()?;
        let design = get_design(ctx.tx(), design.id)?;
        ctx.set_undo("design.restore", json!({"design_id": design.id}), None);
        emit_design(ctx, &design)?;
        Ok(Empty {})
    });
    e.register::<Restore>(|ctx: &mut Ctx, p| {
        let before = get_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, before.project_id)?;
        if before.deleted_at.is_none() {
            return Err(BusError::conflict("design.not_deleted", format!("design {} is not deleted", before.id)));
        }
        let by = actor_name(ctx)?;
        ctx.tx().execute(
            "UPDATE designs SET deleted_at = NULL, updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![before.id, ctx.now, by],
        ).bus()?;
        let design = get_design(ctx.tx(), before.id)?;
        ctx.set_undo("design.delete", json!({"design_id": design.id}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    e.register::<Purge>(|ctx: &mut Ctx, p| {
        let design = get_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        if design.deleted_at.is_none() {
            return Err(BusError::conflict("design.not_deleted", format!("design {} is not deleted", design.id))
                .with_hint("design.delete it first; only a deleted design is purged"));
        }
        ctx.tx().execute("DELETE FROM design_versions WHERE design_id = ?1", [design.id]).bus()?;
        ctx.tx().execute("DELETE FROM design_nodes WHERE design_id = ?1", [design.id]).bus()?;
        ctx.tx().execute("DELETE FROM designs WHERE id = ?1", [design.id]).bus()?;
        ctx.set_project(design.project_id);
        ctx.emit("design.deleted", json!({"id": design.id, "project_id": design.project_id}));
        Ok(Empty {})
    });
    e.register::<Apply>(|ctx: &mut Ctx, p| {
        if p.ops.is_empty() || p.ops.len() > MAX_OPS {
            return Err(BusError::invalid("design.ops", format!("give 1 to {MAX_OPS} edits; this list has {}", p.ops.len())));
        }
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        if p.base_rev.is_some_and(|rev| rev != design.rev) {
            return Err(BusError::conflict("design.stale", format!("design {} is at rev {}, not {}", design.id, design.rev, p.base_rev.unwrap_or_default()))
                .with_details(json!({"rev": design.rev}))
                .with_hint("read it again with design.get and redo the edits against what is there"));
        }
        let by = actor_name(ctx)?;
        // An agent taking over a design someone else changed last: keep what they left, so the
        // person can go back to it whatever the agent does.
        if ctx.actor.is_agent() && design.updated_by != by {
            if let Some(kept) = save_version(ctx.tx(), &design, Some(&format!("Before {by}'s edits")), true, &by, &ctx.now)? {
                emit_version(ctx, design.project_id, &kept)?;
            }
        }
        let (bytes, next_node): (i64, i64) = ctx.tx().prepare_cached("SELECT bytes, next_node FROM designs WHERE id = ?1").bus()?
            .query_row([design.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?;
        let mut run = Run {
            tx: ctx.tx(),
            design: design.id,
            by: by.clone(),
            lenient: p.lenient.unwrap_or(false),
            pages: design.pages,
            nodes: design.nodes,
            bytes,
            next_node,
            at: 0,
            label: String::new(),
            touched: Vec::new(),
            seen: HashSet::new(),
            removed: Vec::new(),
            ids: Vec::with_capacity(p.ops.len()),
            inverse: Vec::with_capacity(p.ops.len()),
            dirty: HashSet::new(),
            restyle: HashSet::new(),
            pending: HashSet::new(),
            plain: HashSet::new(),
            syncing: false,
        };
        let p_len = p.ops.len();
        for (at, op) in p.ops.into_iter().enumerate() {
            (run.at, run.label) = (at, label(at, &op));
            let id = match op {
                NodeOp::Add { node, index } => run.add(node, index)?,
                NodeOp::Update { id, set } => run.update(id, set)?,
                NodeOp::Move { id, parent, index } => run.move_(id, parent, index)?,
                NodeOp::Delete { id } => run.delete(id)?,
                NodeOp::Insert { item, parent, x, y, index, id, set } => run.insert(item, parent, x, y, index, id, set)?,
            };
            run.ids.push(id.unwrap_or_default());
        }
        (run.at, run.label) = (p_len, "following components".to_string());
        run.sync_styles()?;
        run.sync()?;
        run.sync_styles()?;
        if run.bytes > MAX_BYTES {
            return Err(BusError::invalid("design.too_large",
                format!("the design would hold {} KiB of nodes; the limit is {} KiB", run.bytes >> 10, MAX_BYTES >> 10))
                .with_hint("split it across designs, or keep long text and big paths out of it"));
        }
        ctx.tx().prepare_cached(
            "UPDATE designs SET rev = rev + 1, pages = ?2, nodes = ?3, bytes = ?4, next_node = ?5, updated_by = ?6, updated_at = ?7 WHERE id = ?1",
        ).bus()?.execute(params![design.id, run.pages, run.nodes, run.bytes, run.next_node, by, ctx.now]).bus()?;
        let mut nodes = Vec::with_capacity(run.touched.len());
        for id in std::mem::take(&mut run.touched) {
            if let Some(row) = run.row(&id)? {
                nodes.push(node_of(id, row.parent, &row.body)?);
            }
        }
        // What is gone now: a node deleted and added back in the same list is in `nodes`.
        let mut gone: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let removed: Vec<String> = std::mem::take(&mut run.removed).into_iter()
            .filter(|id| gone.insert(id.clone()))
            .collect();
        let inverse: Vec<NodeOp> = std::mem::take(&mut run.inverse).into_iter().rev().flatten().collect();
        let ids = std::mem::take(&mut run.ids);
        let (pages, count) = (run.pages, run.nodes);
        let prev_rev = design.rev;
        let design = Design { rev: prev_rev + 1, pages, nodes: count, updated_by: by, updated_at: ctx.now.clone(), ..design };
        // The audit keeps the undo uncapped (payloads and results stop at 64 KiB): a big delete's
        // would store every node it took. Past UNDO_KEPT only the caller's copy, in the answer,
        // can undo it.
        let undo = json!({"design_id": design.id, "ops": inverse, "lenient": true});
        if serde_json::to_vec(&undo).is_ok_and(|bytes| bytes.len() <= UNDO_KEPT) {
            ctx.set_undo("design.apply", undo, None);
        }
        emit_design(ctx, &design)?;
        Ok(ApplyOut { design, prev_rev, nodes, removed, ids, inverse })
    });
    e.register_unlocked::<Kits>(|_ctx, p| {
        let with_nodes = p.with_nodes.unwrap_or(false);
        let kits = relay_bus::kits::kits()
            .iter()
            .filter(|k| p.kit.as_deref().is_none_or(|want| want == k.id))
            .map(|k| {
                let mut k = k.clone();
                if !with_nodes {
                    for item in &mut k.items {
                        item.nodes.truncate(1);
                    }
                }
                k
            })
            .collect();
        Ok(KitsOut { kits })
    });
    // Staged (D149): the picture is read, sniffed and written beside the store with the lock
    // released — a path may name a 20 MiB file — and the transaction only checks the design.
    e.register_staged::<AssetAdd, StagedAsset>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        ctx.read(|conn| {
            let design = live_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)
        })?;
        let bytes = match (&p.path, &p.bytes_b64) {
            (Some(path), None) => read_asset(path)?,
            (None, Some(encoded)) => decode_asset(encoded)?,
            _ => return Err(BusError::invalid("design.asset", "give the picture as path or as bytes_b64: one of the two")),
        };
        let picture = picture::sniff(&bytes).ok_or_else(|| {
            BusError::invalid("design.asset", "not a PNG, JPEG, GIF, WebP or SVG picture whose size can be read")
        })?;
        let digest = {
            use sha2::Digest;
            crate::hex(&sha2::Sha256::digest(&bytes))
        };
        let src = format!("{}.{}", &digest[..32], picture.ext);
        debug_assert!(relay_bus::design::valid_src(&src));
        let dir = ctx.engine().store.design_assets_dir();
        let path = dir.join(&src);
        store_asset(&dir, &path, &bytes)?;
        Ok(StagedAsset { src, path: path.display().to_string(), w: picture.w, h: picture.h })
    }, |ctx: &mut Ctx, p, staged| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        ctx.set_project(design.project_id);
        Ok(AssetAddOut { src: staged.src, path: staged.path, w: staged.w, h: staged.h })
    });
    e.register::<VersionSave>(|ctx: &mut Ctx, p| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        let by = actor_name(ctx)?;
        let label = p.label.as_deref().map(str::trim).filter(|l| !l.is_empty());
        if label.is_some_and(|l| l.chars().count() > MAX_NAME) {
            return Err(BusError::invalid("design.version_label", format!("a version's label is at most {MAX_NAME} characters")));
        }
        let version = save_version(ctx.tx(), &design, label, false, &by, &ctx.now)?.ok_or_else(|| too_large_to_keep(design.id))?;
        ctx.set_undo("design.version.delete", json!({"design_id": design.id, "version_id": version.id}), None);
        emit_version(ctx, design.project_id, &version)?;
        Ok(version)
    });
    e.register_unlocked::<VersionList>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let versions = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(VERSIONS_OF).bus()?;
            let versions = stmt.query_map([design.id], version_row).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(versions)
        })?;
        Ok(VersionListOut { versions })
    });
    // Only copying the row out holds the store; parsing its nodes runs after.
    e.register_unlocked::<VersionGet>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (version, body) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            get_version(conn, design.id, p.version_id)
        })?;
        let nodes: Vec<Node> = serde_json::from_str(&body)
            .map_err(|e| BusError::internal(format!("version {} of design {} is unreadable: {e}", version.id, version.design_id)))?;
        Ok(VersionGetOut { version, nodes })
    });
    e.register::<VersionRestore>(|ctx: &mut Ctx, p| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        let by = actor_name(ctx)?;
        let (version, body) = get_version(ctx.tx(), design.id, p.version_id)?;
        let nodes: Vec<Node> = serde_json::from_str(&body)
            .map_err(|e| BusError::internal(format!("version {} of design {} is unreadable: {e}", version.id, design.id)))?;
        // What is there now is kept first, so the restore itself can be undone.
        let label = format!("Before restoring \u{201c}{}\u{201d}", version_name(&version));
        let saved = save_version(ctx.tx(), &design, Some(&label), true, &by, &ctx.now)?.ok_or_else(|| too_large_to_keep(design.id))?;
        let next_node: i64 = ctx.tx().prepare_cached("SELECT next_node FROM designs WHERE id = ?1").bus()?
            .query_row([design.id], |r| r.get(0)).bus()?;
        let (pages, count, bytes, top) = put_nodes(ctx.tx(), design.id, nodes)?;
        ctx.tx().prepare_cached(
            "UPDATE designs SET rev = rev + 1, pages = ?2, nodes = ?3, bytes = ?4, next_node = ?5, updated_by = ?6, updated_at = ?7 WHERE id = ?1",
        ).bus()?.execute(params![design.id, pages, count, bytes, next_node.max(top + 1), by, ctx.now]).bus()?;
        let design = get_design(ctx.tx(), design.id)?;
        ctx.set_undo("design.version.restore", json!({"design_id": design.id, "version_id": saved.id}), None);
        emit_version(ctx, design.project_id, &saved)?;
        emit_design(ctx, &design)?;
        Ok(VersionRestoreOut { design, saved_version_id: saved.id })
    });
    // An agent deletes only a version it saved by name: the person's, and those the engine
    // kept before an agent's edits, are the person's to let go of.
    e.register::<VersionDelete>(|ctx: &mut Ctx, p| {
        let design = get_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        let (version, _) = get_version(ctx.tx(), design.id, p.version_id)?;
        if let Some(own) = ctx.actor.session_name() {
            if version.auto || version.created_by != own {
                return Err(BusError::not_own("version").with_hint("an agent deletes only the versions it saved itself"));
            }
        }
        ctx.tx().prepare_cached("DELETE FROM design_versions WHERE id = ?1").bus()?.execute([version.id]).bus()?;
        ctx.set_project(design.project_id);
        ctx.emit("design.version.deleted", json!({"id": version.id, "design_id": design.id, "project_id": design.project_id}));
        Ok(Empty {})
    });
    // Unlocked: the rows are copied out under the lock; writing the page, and reading pictures
    // to inline, runs after.
    e.register_unlocked::<Code>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let rows = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached("SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1").bus()?;
            let rows = stmt.query_map([design.id], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, body: r.get(3)? })).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(rows)
        })?;
        let nodes = ordered(rows, None)?;
        let dir = ctx.engine().store.design_assets_dir();
        let (inline, mut budget) = (p.inline_assets.unwrap_or(false), INLINE_MAX);
        let code = crate::design_code::generate(&nodes, &p.node_id, &mut |src| code_asset(&dir, inline, &mut budget, src))
            .map_err(|refusal| match refusal {
                crate::design_code::Refusal::NotFound(why) => BusError::not_found("design.node_not_found", why),
                crate::design_code::Refusal::Page(why) => BusError::invalid("design.code", why).with_hint("name a frame on the page"),
            })?;
        Ok(CodeOut { html: code.html, css: code.css, assets: code.assets })
    });
    // Event-only: the request id is the design's and this request's own, so the answer names its
    // design and nothing is kept between the two.
    e.register_unlocked::<Render>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let scale = p.scale.unwrap_or(2.0);
        if !scale.is_finite() || !(1.0..=4.0).contains(&scale) {
            return Err(BusError::invalid("design.scale", format!("scale is 1 to 4, not {scale}")));
        }
        let (design, node_id) = ctx.read(|conn| {
            let design = live_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let node_id: Option<String> = match &p.node_id {
                Some(id) => conn.prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND node_id = ?2").bus()?
                    .query_row(params![design.id, id], |r| r.get(0)).optional().bus()?,
                None => conn.prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1 AND parent_id IS NULL ORDER BY ord, node_id LIMIT 1").bus()?
                    .query_row([design.id], |r| r.get(0)).optional().bus()?,
            };
            let node_id = node_id.ok_or_else(|| BusError::not_found("design.node_not_found",
                format!("design {} has no node {:?}", design.id, p.node_id.as_deref().unwrap_or("(page)"))))?;
            Ok((design, node_id))
        })?;
        let request_id = format!("{}-{}", design.id, ctx.req_id);
        ctx.set_project(design.project_id);
        ctx.emit("design.render_requested", json!({
            "request_id": request_id, "project_id": design.project_id, "design_id": design.id, "node_id": node_id, "scale": scale,
        }));
        let wait = json!({"events": ["design.rendered"], "matching": {"request_id": request_id}, "timeout_ms": RENDER_WAIT_MS});
        Ok(RenderOut { request_id, node_id, wait })
    });
    e.register_unlocked::<RenderDone>(|ctx, p| {
        if p.request_id.is_empty() || p.request_id.len() > 64 {
            return Err(BusError::invalid("design.render", "request_id is the one design.render_requested carried"));
        }
        match (&p.path, &p.error) {
            (None, None) => return Err(BusError::invalid("design.render", "give the PNG's path, or the error that stopped it")),
            (Some(path), _) if !Path::new(path).is_absolute() || !Path::new(path).is_file() => {
                return Err(BusError::invalid("design.render", format!("{path:?} is not a file at an absolute path")));
            }
            _ => {}
        }
        let design_id: Id = p.request_id.split_once('-').and_then(|(id, _)| id.parse().ok())
            .ok_or_else(|| BusError::invalid("design.render", "request_id is the one design.render_requested carried"))?;
        let design = ctx.read(|conn| get_design(conn, design_id))?;
        ctx.set_project(design.project_id);
        ctx.emit("design.rendered", json!({
            "request_id": p.request_id, "project_id": design.project_id, "design_id": design.id, "path": p.path, "error": p.error,
        }));
        Ok(Empty {})
    });
}

/// What `design.asset.add` takes: a picture's format from its first bytes, and its natural
/// size from its header, without decoding it.
pub(crate) mod picture {
    use relay_bus::design::MAX_COORD;

    #[derive(Debug, PartialEq, Eq)]
    pub struct Picture {
        /// The extension it is stored under.
        pub ext: &'static str,
        pub w: u32,
        pub h: u32,
    }

    /// PNG, JPEG, GIF, WebP or SVG, with a size; `None` for anything else, or a raster whose
    /// header is cut short.
    pub fn sniff(bytes: &[u8]) -> Option<Picture> {
        let (ext, size) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            ("png", png(bytes))
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            ("jpg", jpeg(bytes))
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            ("gif", gif(bytes))
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
            ("webp", webp(bytes))
        } else {
            ("svg", svg(bytes))
        };
        let (w, h) = size?;
        (w > 0 && h > 0).then_some(Picture { ext, w, h })
    }

    fn be16(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?)))
    }
    fn le16(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?)))
    }
    fn le24(b: &[u8], at: usize) -> Option<u32> {
        let b = b.get(at..at + 3)?;
        Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
    }
    fn le32(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
    }

    /// The IHDR chunk, always first.
    fn png(b: &[u8]) -> Option<(u32, u32)> {
        if b.get(12..16)? != b"IHDR" {
            return None;
        }
        Some((u32::from_be_bytes(b.get(16..20)?.try_into().ok()?), u32::from_be_bytes(b.get(20..24)?.try_into().ok()?)))
    }

    /// The logical screen.
    fn gif(b: &[u8]) -> Option<(u32, u32)> {
        Some((le16(b, 6)?, le16(b, 8)?))
    }

    /// The first frame header (SOF0 to SOF15, less the DHT, JPG and DAC markers that share
    /// the range), walking the segments before it.
    fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
        let mut at = 2;
        loop {
            if *b.get(at)? != 0xFF {
                return None;
            }
            while *b.get(at)? == 0xFF {
                at += 1;
            }
            let marker = b[at];
            at += 1;
            match marker {
                // Markers with no length.
                0x01 | 0xD0..=0xD8 => continue,
                // The image data or its end, and no frame header before them.
                0xD9 | 0xDA => return None,
                0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                    // length(2), precision(1), height(2), width(2)
                    return Some((be16(b, at + 5)?, be16(b, at + 3)?));
                }
                _ => {
                    let len = be16(b, at)? as usize;
                    if len < 2 {
                        return None;
                    }
                    at += len;
                }
            }
        }
    }

    /// The first chunk: lossy (`VP8 `), lossless (`VP8L`) or extended (`VP8X`, its canvas).
    fn webp(b: &[u8]) -> Option<(u32, u32)> {
        match b.get(12..16)? {
            b"VP8 " => {
                if b.get(23..26)? != [0x9Du8, 0x01, 0x2A] {
                    return None;
                }
                Some((le16(b, 26)? & 0x3FFF, le16(b, 28)? & 0x3FFF))
            }
            b"VP8L" => {
                if *b.get(20)? != 0x2F {
                    return None;
                }
                let bits = le32(b, 21)?;
                Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
            }
            b"VP8X" => Some((le24(b, 24)? + 1, le24(b, 27)? + 1)),
            _ => None,
        }
    }

    /// Text whose first tag, past an XML declaration, comments and a doctype, is `<svg`. Its
    /// size is `width`/`height` in px (or unitless), the other taken from `viewBox`'s aspect
    /// when only one is given, else the `viewBox`, else a browser's default 300 by 150.
    fn svg(b: &[u8]) -> Option<(u32, u32)> {
        let head = &b[..b.len().min(64 * 1024)];
        let text = match std::str::from_utf8(head) {
            Ok(text) => text,
            Err(e) => std::str::from_utf8(&head[..e.valid_up_to()]).ok()?,
        };
        let mut rest = text.trim_start_matches('\u{feff}');
        loop {
            rest = rest.trim_start();
            if let Some(r) = rest.strip_prefix("<?") {
                rest = &r[r.find("?>")? + 2..];
            } else if let Some(r) = rest.strip_prefix("<!--") {
                rest = &r[r.find("-->")? + 3..];
            } else if let Some(r) = rest.strip_prefix("<!") {
                // A doctype, past its internal subset when it has one.
                let end = match (r.find('['), r.find('>')) {
                    (Some(open), Some(close)) if open < close => r[open..].find("]>")? + open + 1,
                    (_, close) => close?,
                };
                rest = &r[end + 1..];
            } else {
                break;
            }
        }
        let tag = rest.strip_prefix("<svg")?;
        if !tag.starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/') {
            return None;
        }
        let tag = &tag[..tag.find('>')?];
        let (w, h) = (attr(tag, "width").and_then(px), attr(tag, "height").and_then(px));
        let view = attr(tag, "viewBox").and_then(|v| {
            let n: Vec<f64> = v.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|s| !s.is_empty())
                .map(str::parse).collect::<Result<_, _>>().ok()?;
            match n[..] {
                [_, _, vw, vh] if vw > 0.0 && vh > 0.0 && vw.is_finite() && vh.is_finite() => Some((vw, vh)),
                _ => None,
            }
        });
        let (w, h) = match (w, h, view) {
            (Some(w), Some(h), _) => (w, h),
            (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
            (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
            (None, None, Some(view)) => view,
            (w, h, None) => (w.unwrap_or(300.0), h.unwrap_or(150.0)),
        };
        let dim = |v: f64| v.round().clamp(1.0, MAX_COORD) as u32;
        Some((dim(w), dim(h)))
    }

    /// The value of attribute `name` in the inside of a start tag.
    fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let mut rest = tag;
        loop {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
            let eq = rest.find('=')?;
            let key = rest[..eq].trim();
            let after = rest[eq + 1..].trim_start();
            let quote = after.chars().next().filter(|q| *q == '"' || *q == '\'')?;
            let end = after[1..].find(quote)? + 1;
            if key == name {
                return Some(&after[1..end]);
            }
            rest = &after[end + 1..];
        }
    }

    /// A length in px: unitless or `px`, above zero.
    fn px(value: &str) -> Option<f64> {
        let value = value.trim();
        let n: f64 = value.strip_suffix("px").unwrap_or(value).trim().parse().ok()?;
        (n.is_finite() && n > 0.0).then_some(n)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn pic(ext: &'static str, w: u32, h: u32) -> Option<Picture> {
            Some(Picture { ext, w, h })
        }

        #[test]
        fn rasters_are_sized_from_their_headers() {
            let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            png.extend(640u32.to_be_bytes());
            png.extend(480u32.to_be_bytes());
            assert_eq!(sniff(&png), pic("png", 640, 480));
            assert_eq!(sniff(&png[..20]), None, "a PNG cut short has no size");

            let gif = [b"GIF89a".as_slice(), &[0x20, 0x01, 0x10, 0x00]].concat();
            assert_eq!(sniff(&gif), pic("gif", 288, 16));

            // SOI, an APP0 segment of 16 bytes, then SOF0: length, precision, height, width.
            let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
            jpeg.extend([0u8; 14]);
            jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x96, 0x01, 0x2C]);
            assert_eq!(sniff(&jpeg), pic("jpg", 300, 150));
            // A DHT (0xC4) before the frame is skipped, not read as one.
            let mut dht = vec![0xFF, 0xD8, 0xFF, 0xC4, 0x00, 0x04, 0x00, 0x00];
            dht.extend([0xFF, 0xC2, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x03]);
            assert_eq!(sniff(&dht), pic("jpg", 3, 2));
            assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00]), None, "scan data before any frame header");

            let riff = |chunk: &[u8], data: &[u8]| [b"RIFF\0\0\0\0WEBP".as_slice(), chunk, b"\0\0\0\0", data].concat();
            let lossy = riff(b"VP8 ", &[0, 0, 0, 0x9D, 0x01, 0x2A, 0x90, 0x01, 0x2C, 0x01]);
            assert_eq!(sniff(&lossy), pic("webp", 400, 300));
            // 14 bits of width - 1 (99), then 14 of height - 1 (49).
            let bits: u32 = 99 | 49 << 14;
            let lossless = riff(b"VP8L", &[[0x2F].as_slice(), &bits.to_le_bytes()].concat());
            assert_eq!(sniff(&lossless), pic("webp", 100, 50));
            let extended = riff(b"VP8X", &[0, 0, 0, 0, 0xFF, 0x03, 0x00, 0xFF, 0x01, 0x00]);
            assert_eq!(sniff(&extended), pic("webp", 1024, 512));
        }

        #[test]
        fn svg_is_sized_from_its_root_tag() {
            let svg = |text: &str| sniff(text.as_bytes());
            assert_eq!(svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80px"/>"#), pic("svg", 120, 80));
            assert_eq!(svg("\u{feff}<?xml version=\"1.0\"?>\n<!-- logo -->\n<!DOCTYPE svg [ <!ENTITY a \"b\"> ]>\n<svg viewBox='0 0 64 32'></svg>"), pic("svg", 64, 32));
            assert_eq!(svg(r#"<svg width="200" viewBox="0,0,100,50"/>"#), pic("svg", 200, 100), "the other side from the aspect");
            assert_eq!(svg(r#"<svg stroke-width="9" width="100%"><rect/></svg>"#), pic("svg", 300, 150), "neither is in px");
            assert_eq!(svg("<svg>"), pic("svg", 300, 150));
            assert_eq!(svg("<svgx width='1'>"), None);
            assert_eq!(svg("<html><svg/></html>"), None, "the first tag must be the svg");
            assert_eq!(svg("hello"), None);
            assert_eq!(sniff(&[0u8, 1, 2, 3]), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_title_stays_a_title() {
        assert_eq!(copy_title("Home"), "Home copy");
        let long = "x".repeat(MAX_NAME);
        assert_eq!(copy_title(&long).chars().count(), MAX_NAME);
        assert!(title(Some("  "), DEFAULT_TITLE).is_err());
        assert_eq!(title(None, DEFAULT_TITLE).unwrap(), DEFAULT_TITLE);
    }

    #[test]
    fn every_kind_reads_back_as_itself() {
        for kind in [NodeKind::Page, NodeKind::Frame, NodeKind::Group, NodeKind::Rect, NodeKind::Ellipse,
                     NodeKind::Line, NodeKind::Text, NodeKind::Sticky, NodeKind::Path, NodeKind::Image,
                     NodeKind::Polygon, NodeKind::Icon] {
            assert_eq!(kind_of(kind_str(kind)).unwrap(), kind);
            assert_eq!(serde_json::to_value(kind).unwrap(), kind_str(kind), "the column names a kind as its JSON does");
        }
    }

    #[test]
    fn a_stored_body_has_no_id_or_parent() {
        let mut node = Node::new(NodeKind::Rect);
        node.id = "r".into();
        node.parent = Some("p".into());
        node.fill = Some("#fff".into());
        let body = body_of(&mut node).unwrap();
        assert!(!body.contains("\"id\"") && !body.contains("\"parent\""), "{body}");
        assert_eq!((node.id.as_str(), node.parent.as_deref()), ("r", Some("p")), "the node keeps both");
        assert_eq!(node_of("r".into(), Some("p".into()), &body).unwrap(), node);
    }
}
