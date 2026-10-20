//! `design.apply`: a list of node edits run in order against the request's transaction.

use super::revs::{self, removed_rev};
use super::{body_of, emit_applied, expect, kind_of, live_design, node_of, row_bytes};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{actor_name, assert_actor_project};
use relay_bus::design::{details, field_of, label, nearest, Conflict, ConflictReason, Node, NodeKind, NodeOp, Styles, MAX_DEPTH, MAX_DESIGN_BYTES, MAX_LENIENT_OPS, MAX_NODES, MAX_OPS};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Apply, ApplyOut, Reply};
use relay_bus::types::{Design, Id};
use relay_bus::ErrorKind;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

/// The largest `design.apply` undo the audit keeps for `audit.undo`.
const UNDO_KEPT: usize = 256 * 1024;
/// B7: how large a full answer's nodes and undo may be, by estimate: the rest of it (each edit's
/// id, a capped `skipped`, 20 warnings) keeps it under the clients' 2 MiB line.
const REPLY_BUDGET: usize = 1024 * 1024;
/// Most `warnings` one answer carries.
const MAX_WARNINGS: usize = 20;
/// Most conflicts `skipped` lists; a warning counts the rest.
const MAX_SKIPPED: usize = 100;
/// How far a walk up or down a design goes: past MAX_DEPTH, so a design another build nested
/// deeper is still measured, but never the whole of a chain gone wrong.
pub(super) const WALK: usize = MAX_DEPTH + 6;
const TOO_LARGE_HINT: &str = "split it across designs, or keep long text and big paths out of it";

/// A row's kind: `Err` holds the name of one this build does not know (a newer build's), whose
/// row is a leaf it can neither read nor edit.
pub(super) type RowKind = Result<NodeKind, String>;

fn row_kind(text: String) -> RowKind {
    kind_of(&text).ok_or(text)
}

/// The refusal of an edit naming node `id`, of a kind this build does not know.
fn unknown_kind(id: &str, kind: &str) -> String {
    format!("node {id:?} is of a kind this Relay cannot read ({kind:?}); open it in a newer Relay")
}

/// A node row: where it sits, what it is, and its JSON.
pub(super) struct Stored {
    pub(super) parent: Option<String>,
    pub(super) kind: RowKind,
    pub(super) ord: f64,
    pub(super) body: String,
}

/// A connector end or a picture an edit wrote (B6), checked as the node holds it once the list
/// has run, so an end may name a node a later edit adds.
pub(super) struct Ref {
    pub(super) id: String,
    /// `from`, `to` or `src`.
    pub(super) field: &'static str,
    /// The edit that wrote it, and its label.
    pub(super) at: usize,
    pub(super) label: String,
}

/// One `design.apply` under way: the design's counters as its edits change them, and what the
/// answer reports. Every edit reads and writes the transaction, so the next one sees it.
pub(super) struct Run<'a> {
    pub(super) tx: &'a Connection,
    pub(super) design: Id,
    /// An undo or a redo: a written rule's breach is applied as written, and an edit of a node
    /// that is gone or cannot be read is skipped, where a strict list refuses either.
    pub(super) lenient: bool,
    /// An agent asked: what a lenient list let through is told it in `warnings`.
    pub(super) agent: bool,
    pub(super) warnings: Vec<String>,
    pub(super) pages: i64,
    pub(super) nodes: i64,
    /// The design's size as `designs.bytes` counts it ([`row_bytes`] per node, B60).
    pub(super) bytes: i64,
    pub(super) next_node: i64,
    /// The rev this list makes: every row it writes is dated with it.
    pub(super) rev: i64,
    /// Whether the design has removed ids on record (`design_removed`), so an add clears its
    /// id's; set by any delete.
    pub(super) tombstones: bool,
    /// The edit under way: its index, [`label`] and the node it names (none for an add that
    /// leaves its id to the engine).
    pub(super) at: usize,
    pub(super) label: String,
    pub(super) target: Option<String>,
    /// Nodes an edit added, changed or moved, in the order first touched.
    pub(super) touched: Vec<String>,
    pub(super) seen: HashSet<String>,
    pub(super) removed: Vec<String>,
    pub(super) ids: Vec<String>,
    /// Each applied edit's undo, in edit order; a delete's is the whole subtree's re-adds.
    pub(super) inverse: Vec<Vec<NodeOp>>,
    /// Where nodes sit, as far as this list has looked: each node's parent, read a chain at a
    /// time when first needed (or whole, once a move to another page has the connectors' ends
    /// placed) and kept as its adds, moves and deletes change them. Depths and pages are read
    /// off it (W14, B6).
    pub(super) parents: HashMap<String, Option<String>>,
    /// The last node an add asked the depth of, and that depth: a list adds a node's children
    /// one after another. A move or a delete forgets it.
    pub(super) last_depth: Option<(String, usize)>,
    /// The connector ends and pictures the list wrote, in edit order (B6).
    pub(super) refs: Vec<Ref>,
    /// Each node the list deleted, with what was inside it, and the last edit that did.
    pub(super) removed_at: HashMap<String, usize>,
    /// Each node the list moved to another page, and the last edit that did (B6); a node a later
    /// edit took out of one so moved, with that one's edit, since its own move crossed nothing.
    pub(super) crossed: HashMap<String, usize>,
    /// Where pictures are stored: an image's `src` names a file in it (B6).
    pub(super) assets: PathBuf,
    /// W6, W7: what changed since a lenient list's `base_rev`, and what its `expect` no longer
    /// finds, by node: an edit of one is skipped, or an update loses the fields that moved,
    /// rather than overwrite that change ([`Run::stale`]). Empty for a strict list, which such a
    /// change refuses before any edit runs.
    pub(super) conflicts: HashMap<String, Vec<Conflict>>,
    /// The ids whose add a lenient list skipped: an add into one is skipped too
    /// (`parent_skipped`), and so is a later edit of one, which meant the node not added.
    pub(super) skipped_adds: HashSet<String>,
    /// W7: what the undo of this list will expect: the fields each update wrote, by node; the
    /// nodes it moved; the nodes it added.
    pub(super) updated: HashMap<String, BTreeSet<String>>,
    pub(super) moved: HashSet<String>,
    pub(super) added: HashSet<String>,
    /// Edits not applied as sent, and why: the first [`MAX_SKIPPED`]; `skipped_total` counts all.
    pub(super) skipped: Vec<Conflict>,
    pub(super) skipped_total: usize,
    /// The design's colours and styles, read on first need ([`Run::styles`]), the bytes of the
    /// column as read, and whether an edit changed them.
    pub(super) styles: Option<Styles>,
    pub(super) styles_len: i64,
    pub(super) styles_changed: bool,
    /// Each copy edit's new node, and the node it copied.
    pub(super) copied: BTreeMap<String, String>,
}

const NODE_META: &str = "SELECT parent_id, kind, ord FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
const NODE_ROW: &str = "SELECT parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND node_id = ?2";
/// Siblings in paint order from `?4`, leaving out `?3` (the node being moved, if any).
const SIBLING_ORDS: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord, node_id LIMIT 2 OFFSET ?4+0";
const TOP_ORD: &str = "SELECT ord FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2 AND node_id IS NOT ?3
    ORDER BY ord DESC, node_id DESC LIMIT 1";
/// Node `?2` and its ancestors as far as `?3` levels up, nearest first.
const ANCESTORS: &str = "WITH RECURSIVE up(node_id, parent_id, d) AS (
        SELECT node_id, parent_id, 0 FROM design_nodes WHERE design_id = ?1 AND node_id = ?2
        UNION ALL
        SELECT n.node_id, n.parent_id, up.d + 1 FROM up JOIN design_nodes n ON n.design_id = ?1 AND n.node_id = up.parent_id
        WHERE up.d < ?3)
    SELECT node_id, parent_id FROM up";
/// How many ancestors node `?2` has, counted to its page as far as `?3` levels up: none when its
/// chain does not reach a page by then.
const DEPTH: &str = "WITH RECURSIVE up(parent_id, d) AS (
        SELECT parent_id, 0 FROM design_nodes WHERE design_id = ?1 AND node_id = ?2
        UNION ALL
        SELECT n.parent_id, up.d + 1 FROM up JOIN design_nodes n ON n.design_id = ?1 AND n.node_id = up.parent_id
        WHERE up.d < ?3)
    SELECT MAX(d) FROM up WHERE parent_id IS NULL";
/// Every node of the design and its parent, from the parent index alone.
const PARENTS: &str = "SELECT node_id, parent_id FROM design_nodes WHERE design_id = ?1";
/// How many levels below node `?2` the deepest node inside it sits, counted as far as `?3`.
const HEIGHT: &str = "WITH RECURSIVE down(node_id, d) AS (
        SELECT ?2, 0
        UNION ALL
        SELECT n.node_id, down.d + 1 FROM down JOIN design_nodes n ON n.design_id = ?1 AND n.parent_id = down.node_id
        WHERE down.d < ?3)
    SELECT MAX(d) FROM down";

/// Which edit [`Run::stale`] asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Edit {
    Update,
    Move,
    Delete,
}

/// What a lenient list does with an edit of a node that changed since it was read.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Stale {
    No,
    Skip,
    /// An update goes ahead without these fields.
    Drop(Vec<String>),
}

impl Run<'_> {
    /// The refusal of the edit under way: `ops[3] update "hero": fill: …`, with `details` naming
    /// the edit and, when the reason starts with one, the field (I-2).
    pub(super) fn fail(&self, kind: ErrorKind, code: &str, why: impl std::fmt::Display) -> BusError {
        let why = why.to_string();
        BusError::new(kind, code, format!("{}: {why}", self.label)).with_details(details(self.at, self.target.as_deref(), field_of(&why)))
    }

    /// [`Run::fail`] naming `field` where the reason does not start with it.
    pub(super) fn fail_on(&self, field: &str, kind: ErrorKind, code: &str, why: impl std::fmt::Display) -> BusError {
        let why = why.to_string();
        BusError::new(kind, code, format!("{}: {why}", self.label)).with_details(details(self.at, self.target.as_deref(), Some(field)))
    }

    /// The node an edit names is not there: skipped when lenient (`missing`, with the rev it went
    /// at when that is on record), else refused with the nearest id the design holds (W19).
    pub(super) fn missing(&mut self, what: impl std::fmt::Display) -> Result<Option<String>, BusError> {
        let id = self.target.clone().unwrap_or_default();
        if self.lenient {
            let rev = removed_rev(self.tx, self.design, &id)?;
            self.skip(Conflict { rev, ..Conflict::new(Some(self.at as u32), id, ConflictReason::Missing) });
            return Ok(None);
        }
        let id = id.as_str();
        let hint = self.nearest_id(id)?;
        Err(self.fail(ErrorKind::NotFound, "design.node_not_found", format!("{what}{}", self.gone_note(id)?)).with_hint(hint))
    }

    /// W19: what a refusal adds about an id the design does not hold that an earlier list
    /// removed (` (removed at rev 14)`, from the removals on record), or nothing. One this list
    /// removed is not noted: its own edit did.
    pub(super) fn gone_note(&self, id: &str) -> Result<String, BusError> {
        Ok(match removed_rev(self.tx, self.design, id)? {
            Some(rev) if rev < self.rev => format!(" (removed at rev {rev})"),
            _ => String::new(),
        })
    }

    /// W6, W7: what a lenient list does with the edit under way, of node `id`, for a change made
    /// since its `base_rev` or its `expect`: skip it when the node changed or went, when (a move)
    /// it sits in another parent, or when (a delete) something was added or changed inside it,
    /// which the delete would take; an update only drops the fields that hold other values now.
    /// An edit of an id whose add this list skipped is skipped too: what holds the id is not the
    /// node it meant. Each skip is reported in `skipped`, at this edit.
    pub(super) fn stale(&mut self, id: &str, edit: Edit) -> Stale {
        if self.skipped_adds.contains(id) {
            self.skip(Conflict::new(Some(self.at as u32), id, ConflictReason::Exists));
            return Stale::Skip;
        }
        let Some(found) = self.conflicts.get(id) else { return Stale::No };
        let blocks = |c: &&Conflict| match c.reason {
            ConflictReason::Missing | ConflictReason::Changed => true,
            ConflictReason::Parent => edit == Edit::Move,
            ConflictReason::Inside => edit == Edit::Delete,
            _ => false,
        };
        let at = Some(self.at as u32);
        let hits: Vec<Conflict> = found.iter().filter(blocks).map(|c| Conflict { at, ..c.clone() }).collect();
        if !hits.is_empty() {
            for conflict in hits {
                self.skip(conflict);
            }
            return Stale::Skip;
        }
        let fields: Vec<String> = match edit {
            Edit::Update => found.iter().filter(|c| c.reason == ConflictReason::Fields).flat_map(|c| c.fields.iter().cloned()).collect(),
            _ => Vec::new(),
        };
        if fields.is_empty() {
            return Stale::No;
        }
        self.skip(Conflict { fields: fields.clone(), ..Conflict::new(at, id, ConflictReason::Fields) });
        Stale::Drop(fields)
    }

    /// Report an edit (or part of one) not applied as sent.
    pub(super) fn skip(&mut self, conflict: Conflict) {
        self.skipped_total += 1;
        if self.skipped.len() < MAX_SKIPPED {
            self.skipped.push(conflict);
        }
    }

    /// W19: the hint for an id the design does not hold, the nearest one it does or where to
    /// find them. It reads every id, on the way to a refusal only.
    pub(super) fn nearest_id(&self, want: &str) -> Result<String, BusError> {
        let ids: Vec<String> = self.tx.prepare_cached("SELECT node_id FROM design_nodes WHERE design_id = ?1").bus()?
            .query_map([self.design], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        Ok(match nearest(want, ids.iter().map(String::as_str)) {
            Some(id) => format!("did you mean {id:?}?"),
            None => "design.get lists the ids (pass page to read one page)".to_string(),
        })
    }

    /// Learn where node `id` sits: its chain up to its page (or WALK levels) into `parents`.
    /// A node the list has not seen is read alone when its parent has been (a leaf it added,
    /// most often), and with the rest of its chain in one query when not.
    pub(super) fn learn(&mut self, id: &str) -> Result<(), BusError> {
        let mut at = id.to_string();
        for _ in 0..=WALK {
            let parent = match self.parents.get(&at) {
                Some(Some(parent)) => parent.clone(),
                Some(None) => return Ok(()),
                None => {
                    let Some((parent, _, _)) = self.meta(&at)? else { return Ok(()) };
                    self.parents.insert(at, parent.clone());
                    let Some(parent) = parent else { return Ok(()) };
                    if !self.parents.contains_key(&parent) {
                        let rows: Vec<(String, Option<String>)> = self.tx.prepare_cached(ANCESTORS).bus()?
                            .query_map(params![self.design, parent, WALK as i64], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
                            .collect::<rusqlite::Result<_>>().bus()?;
                        self.parents.extend(rows);
                        return Ok(());
                    }
                    parent
                }
            };
            at = parent;
        }
        Ok(())
    }

    /// Node `id` and its ancestors as `parents` knows them, nearest first, at most WALK + 1.
    pub(super) fn chain<'s>(&'s self, id: &'s str) -> impl Iterator<Item = &'s str> + 's {
        std::iter::successors(Some(id), |at| self.parents.get(*at)?.as_deref()).take(WALK + 1)
    }

    /// How many ancestors node `id` has by `parents`: `None` when it meets a node it has not
    /// seen, `Some(None)` past WALK.
    fn known_depth(&self, id: &str) -> Option<Option<usize>> {
        let mut at = id;
        for depth in 0..=WALK {
            match self.parents.get(at)? {
                Some(parent) => at = parent,
                None => return Some(Some(depth)),
            }
        }
        Some(None)
    }

    /// How many ancestors node `id` has (a page's child 1): `None` when it is not in the design
    /// or sits too deep to tell. Each add asks it of its parent, so it walks what the list has
    /// seen, and reads the design only for a chain it has not.
    pub(super) fn depth(&mut self, id: &str) -> Result<Option<usize>, BusError> {
        if let Some((last, depth)) = &self.last_depth {
            if last == id {
                return Ok(Some(*depth));
            }
        }
        let depth = match self.known_depth(id) {
            Some(depth) => depth,
            None => {
                self.learn(id)?;
                self.known_depth(id).flatten()
            }
        };
        self.last_depth = depth.map(|depth| (id.to_string(), depth));
        Ok(depth)
    }

    /// Where node `id` sits: how many ancestors it has (a page's child 1) and its page (a page
    /// its own). `None` when it is not in the design, or sits too deep to tell (a chain past
    /// WALK another build left).
    pub(super) fn place(&mut self, id: &str) -> Result<Option<(usize, String)>, BusError> {
        self.learn(id)?;
        if !self.parents.contains_key(id) {
            return Ok(None);
        }
        let (depth, top) = self.chain(id).enumerate().last().unwrap_or((0, id));
        Ok(matches!(self.parents.get(top), Some(None)).then(|| (depth, top.to_string())))
    }

    /// How many ancestors node `id` has however deep it sits, for a node past WALK (a chain
    /// another build nested): one query up its whole chain, bounded by MAX_NODES. `None` when it
    /// does not reach a page.
    fn full_depth(&self, id: &str) -> Result<Option<usize>, BusError> {
        let depth: Option<i64> = self.tx.prepare_cached(DEPTH).bus()?
            .query_row(params![self.design, id, MAX_NODES as i64], |r| r.get(0)).bus()?;
        Ok(depth.map(|depth| depth as usize))
    }

    /// Where every node of the design sits, read whole: what [`Run::release_ends`] places each
    /// connector's ends by after a move to another page, one read rather than a chain per end.
    fn learn_all(&mut self) -> Result<(), BusError> {
        let rows: Vec<(String, Option<String>)> = self.tx.prepare_cached(PARENTS).bus()?
            .query_map([self.design], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        self.parents.reserve(rows.len().saturating_sub(self.parents.len()));
        self.parents.extend(rows);
        Ok(())
    }

    /// How many levels below node `id` the deepest node inside it sits, counted as far as WALK.
    pub(super) fn height(&self, id: &str) -> Result<usize, BusError> {
        let height: Option<i64> = self.tx.prepare_cached(HEIGHT).bus()?
            .query_row(params![self.design, id, WALK as i64], |r| r.get(0)).bus()?;
        Ok(height.unwrap_or(0) as usize)
    }

    /// The ancestors a node put in `parent` would have, page first, for a refusal: two at each
    /// end of a long chain (`page-1 › shell › … › row-63 › row-64`).
    fn chain_text(&self, parent: &str) -> String {
        let mut chain: Vec<&str> = self.chain(parent).collect();
        chain.reverse();
        if chain.len() > 5 {
            chain = [&chain[..2], &["…"], &chain[chain.len() - 2..]].concat();
        }
        chain.join(" › ")
    }

    /// W14: whether node `id`, with the `height` levels it holds below it (0 for one holding
    /// nothing), fits in `parent`, at most MAX_DEPTH deep. Past it a strict list is refused,
    /// naming the chain it would hang from; a lenient one skips the edit (`Ok(false)`), telling
    /// an agent.
    pub(super) fn fits(&mut self, parent: &str, id: &str, height: usize) -> Result<bool, BusError> {
        let deepest = self.depth(parent)?.map(|depth| depth + 1 + height);
        if deepest.is_some_and(|deepest| deepest <= MAX_DEPTH) {
            return Ok(true);
        }
        let deep = deepest.map_or_else(|| format!("more than {WALK}"), |deepest| deepest.to_string());
        let chain = self.chain_text(parent);
        let why = match height {
            0 => format!("it would sit {deep} deep ({chain}); a design nests at most {MAX_DEPTH} levels"),
            _ => format!("what it holds would sit {deep} deep ({chain} › {id} › …); a design nests at most {MAX_DEPTH} levels"),
        };
        if self.lenient {
            self.skip(Conflict::new(Some(self.at as u32), id, ConflictReason::TooDeep));
            return Ok(false);
        }
        Err(self.fail(ErrorKind::Invalid, "design.too_deep", why).with_hint("nest less: lay rows out in one frame rather than a frame per row"))
    }

    /// Tell an agent what its lenient list let through or skipped (at most [`MAX_WARNINGS`]).
    pub(super) fn warn(&mut self, what: impl std::fmt::Display) {
        if self.agent && self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(format!("{}: {what}", self.label));
        }
    }

    /// A written rule `why` is broken: refused in a strict list; a lenient one (an undo putting
    /// back what was stored) applies it as written and tells an agent.
    pub(super) fn breach(&mut self, why: String) -> Result<(), BusError> {
        if !self.lenient {
            return Err(self.fail(ErrorKind::Invalid, "design.node_invalid", why));
        }
        self.warn(format_args!("{why}; kept as it was"));
        Ok(())
    }

    /// The node row `id`, if this build can read it: an edit naming one it cannot (a kind it
    /// does not know, a body it cannot parse) is refused, or skipped when lenient. Nothing of
    /// such a row is lost: it is left as it is.
    pub(super) fn readable(&mut self, id: &str, row: &Stored) -> Result<Option<Node>, BusError> {
        let why = match &row.kind {
            Err(kind) => unknown_kind(id, kind),
            Ok(_) => match node_of(id.to_string(), row.parent.clone(), &row.body) {
                Ok(node) => return Ok(Some(node)),
                Err(error) => error.message,
            },
        };
        self.unreadable(why)?;
        Ok(None)
    }

    /// An edit cannot touch a node this build cannot read (`why`): refused, or skipped (`Ok`,
    /// `unreadable`) when lenient.
    pub(super) fn unreadable(&mut self, why: String) -> Result<(), BusError> {
        if !self.lenient {
            return Err(self.fail(ErrorKind::Invalid, "design.node_unreadable", why));
        }
        let id = self.target.clone().unwrap_or_default();
        self.skip(Conflict::new(Some(self.at as u32), id, ConflictReason::Unreadable));
        Ok(())
    }

    pub(super) fn meta(&self, id: &str) -> Result<Option<(Option<String>, RowKind, f64)>, BusError> {
        let found: Option<(Option<String>, String, f64)> = self.tx.prepare_cached(NODE_META).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().bus()?;
        Ok(found.map(|(parent, kind, ord)| (parent, row_kind(kind), ord)))
    }

    pub(super) fn row(&self, id: &str) -> Result<Option<Stored>, BusError> {
        let found: Option<(Option<String>, String, f64, String)> = self.tx.prepare_cached(NODE_ROW).bus()?
            .query_row(params![self.design, id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().bus()?;
        Ok(found.map(|(parent, kind, ord, body)| Stored { parent, kind: row_kind(kind), ord, body }))
    }

    pub(super) fn touch(&mut self, id: &str) {
        if self.seen.insert(id.to_string()) {
            self.touched.push(id.to_string());
        }
    }

    /// The `ord` that puts a node at `index` among `parent`'s children (the pages, for `None`),
    /// `skip` aside: 0 at the bottom, absent or past the end on top.
    pub(super) fn slot(&self, parent: Option<&str>, index: Option<u32>, skip: Option<&str>) -> Result<f64, BusError> {
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

    pub(super) fn renumber(&self, parent: Option<&str>) -> Result<(), BusError> {
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
    pub(super) fn index_of(&self, parent: Option<&str>, id: &str, ord: f64) -> Result<u32, BusError> {
        let below: i64 = self.tx
            .prepare_cached("SELECT COUNT(*) FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2
                AND (ord < ?3 OR (ord = ?3 AND node_id < ?4))").bus()?
            .query_row(params![self.design, parent, ord, id], |r| r.get(0)).bus()?;
        Ok(below as u32)
    }

    /// Whether a move of node `id` (at `ord`, `at` among its siblings) to `index` among the same
    /// `parent`'s children leaves it where it is: that index, or the top when there is none or
    /// it is past the end and nothing sits above it. As [`Run::slot`] places it.
    fn in_place(&self, parent: Option<&str>, id: &str, ord: f64, at: u32, index: Option<u32>) -> Result<bool, BusError> {
        if index == Some(at) {
            return Ok(true);
        }
        if index.is_some_and(|index| index < at) {
            return Ok(false);
        }
        let above: bool = self.tx.prepare_cached("SELECT EXISTS(SELECT 1 FROM design_nodes WHERE design_id = ?1 AND parent_id IS ?2
            AND (ord > ?3 OR (ord = ?3 AND node_id > ?4)))").bus()?
            .query_row(params![self.design, parent, ord, id], |r| r.get(0)).bus()?;
        Ok(!above)
    }

    /// `{kind}-{n}` from the design's counter, past any id already taken.
    pub(super) fn assign(&mut self, kind: NodeKind) -> Result<String, BusError> {
        loop {
            let id = format!("{}-{}", kind.name(), self.next_node);
            self.next_node += 1;
            if self.meta(&id)?.is_none() {
                return Ok(id);
            }
        }
    }

    /// `parent` can take a child (`child`, or the parent itself for an add leaving its id to the
    /// engine): it is there and is a page, frame or group. `Ok(false)` when it is gone and the
    /// list is lenient (`no_parent`). A node of a kind this build cannot read holds nothing it
    /// adds or moves (an undo skips the edit, `unreadable`).
    pub(super) fn container(&mut self, parent: &str, child: &str) -> Result<bool, BusError> {
        let child = if child.is_empty() { parent } else { child };
        match self.meta(parent)? {
            None if self.lenient => {
                self.skip(Conflict::new(Some(self.at as u32), child, ConflictReason::NoParent));
                Ok(false)
            }
            None => Err(self.fail(ErrorKind::NotFound, "design.parent", format!("parent {parent:?} is not in the design{}", self.gone_note(parent)?))
                .with_hint(self.nearest_id(parent)?)),
            Some((_, Ok(kind), _)) if !kind.holds_children() => Err(self.fail(ErrorKind::Invalid, "design.parent",
                format!("parent {parent:?} is a {}, which holds nothing; use a page, frame or group", kind.name()))),
            Some((_, Err(_), _)) if self.lenient => {
                self.skip(Conflict::new(Some(self.at as u32), parent, ConflictReason::Unreadable));
                Ok(false)
            }
            Some((_, Err(kind), _)) => Err(self.fail(ErrorKind::Invalid, "design.parent",
                format!("parent {parent:?} is of a kind this Relay cannot read ({kind:?}); use a page, frame or group"))),
            Some(_) => Ok(true),
        }
    }

    pub(super) fn add(&mut self, mut node: Node, index: Option<u32>) -> Result<Option<String>, BusError> {
        node.validate().map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        if let Err(why) = node.check_new() {
            self.breach(why)?;
        }
        self.check_names(&mut node)?;
        if node.parent.as_ref().is_some_and(|parent| self.skipped_adds.contains(parent)) {
            // W6: what it goes in was not added; neither is it.
            self.skip(Conflict::new(Some(self.at as u32), node.id.clone(), ConflictReason::ParentSkipped));
            if !node.id.is_empty() {
                self.skipped_adds.insert(node.id);
            }
            return Ok(None);
        }
        if let Some(parent) = node.parent.as_deref() {
            if !self.container(parent, &node.id)? || !self.fits(parent, &node.id, 0)? {
                // What was to go inside it is skipped with it (`parent_skipped`).
                if !node.id.is_empty() {
                    self.skipped_adds.insert(node.id);
                }
                return Ok(None);
            }
        }
        if node.id.is_empty() {
            node.id = self.assign(node.kind)?;
        } else if self.meta(&node.id)?.is_some() {
            if self.lenient {
                // MODEL-SPEC §1 #8 (B50): an undo that would add back an id taken since skips
                // that node and what it held, and says so; it never renames. The same when this
                // list's delete of it was skipped for a change since base_rev (W6).
                self.skip(Conflict::new(Some(self.at as u32), node.id.clone(), ConflictReason::Exists));
                self.skipped_adds.insert(node.id);
                return Ok(None);
            }
            return Err(self.fail_on("id", ErrorKind::Conflict, "design.node_exists", format!("{:?} is already in the design", node.id))
                .with_hint("update or move that node, or leave the id out to have one assigned"));
        }
        if self.nodes >= MAX_NODES as i64 {
            return Err(self.fail(ErrorKind::Invalid, "design.too_large", format!("a design holds at most {MAX_NODES} nodes")));
        }
        let ord = self.slot(node.parent.as_deref(), index, None)?;
        let body = body_of(&mut node)?;
        self.tx.prepare_cached("INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body, rev) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)").bus()?
            .execute(params![self.design, node.id, node.parent, node.kind.name(), ord, body, self.rev]).bus()?;
        if self.tombstones {
            // An id back in the design is no longer removed.
            self.tx.prepare_cached("DELETE FROM design_removed WHERE design_id = ?1 AND node_id = ?2").bus()?
                .execute(params![self.design, node.id]).bus()?;
        }
        // W6: a later edit of it edits the node this list added, whatever went on under its id
        // since base_rev (revs::base_conflicts counts no edit after an add as a conflict).
        self.conflicts.remove(&node.id);
        self.nodes += 1;
        self.pages += i64::from(node.kind == NodeKind::Page);
        self.bytes += row_bytes(&node.id, node.parent.as_deref(), &body);
        self.touch(&node.id);
        self.added.insert(node.id.clone());
        self.inverse.push(vec![NodeOp::Delete { id: node.id.clone() }]);
        // What a later add may go in; where a leaf sits is read when asked (B6), seldom.
        if node.kind.holds_children() {
            self.parents.insert(node.id.clone(), node.parent.clone());
        }
        for (field, written) in [("from", node.from.is_some()), ("to", node.to.is_some()), ("src", node.src.is_some())] {
            if written {
                self.wrote(&node.id, field);
            }
        }
        Ok(Some(node.id))
    }

    /// Note that the edit under way wrote `field` (a connector end or a picture) of node `id`,
    /// for [`Run::check_refs`].
    pub(super) fn wrote(&mut self, id: &str, field: &'static str) {
        self.refs.push(Ref { id: id.to_string(), field, at: self.at, label: self.label.clone() });
    }

    pub(super) fn update(&mut self, id: String, mut set: Map<String, Value>) -> Result<Option<String>, BusError> {
        match self.stale(&id, Edit::Update) {
            Stale::No => {}
            Stale::Skip => return Ok(None),
            Stale::Drop(fields) => {
                set.retain(|key, _| !fields.iter().any(|field| field == key || (field == "w" && key == "width") || (field == "h" && key == "height")));
                if set.is_empty() {
                    return Ok(None);
                }
            }
        }
        let Some(row) = self.row(&id)? else { return self.missing("no such node") };
        let Some(node) = self.readable(&id, &row)? else { return Ok(None) };
        let mut next = node.patched(&set).map_err(|why| self.fail(ErrorKind::Invalid, "design.node_invalid", why))?;
        if let Err(why) = Node::check_update(&node, &set, &next) {
            self.breach(why)?;
        }
        self.check_names(&mut next)?;
        let body = body_of(&mut next)?;
        if body == row.body {
            // TM-58: it already holds what was sent. Nothing is written or touched, and nothing
            // needs undoing; a list of only such edits leaves the design at its rev.
            return Ok(Some(next.id));
        }
        self.tx.prepare_cached("UPDATE design_nodes SET body = ?3, rev = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, next.id, body, self.rev]).bus()?;
        self.bytes += body.len() as i64 - row.body.len() as i64;
        self.touch(&next.id);
        let before = node.before(&set);
        self.updated.entry(next.id.clone()).or_default().extend(before.keys().cloned());
        self.inverse.push(vec![NodeOp::Update { id: next.id.clone(), set: before }]);
        for field in ["from", "to", "src"] {
            if set.contains_key(field) {
                self.wrote(&next.id, field);
            }
        }
        Ok(Some(next.id))
    }

    pub(super) fn move_(&mut self, id: String, parent: Option<String>, index: Option<u32>) -> Result<Option<String>, BusError> {
        if self.stale(&id, Edit::Move) != Stale::No {
            return Ok(None);
        }
        let Some((from, kind, ord)) = self.meta(&id)? else { return self.missing("no such node") };
        let kind = match kind {
            Ok(kind) => kind,
            Err(kind) => {
                self.unreadable(unknown_kind(&id, &kind))?;
                return Ok(None);
            }
        };
        let mut crossed = None;
        let to = if kind == NodeKind::Page {
            if parent.is_some() {
                return Err(self.fail_on("parent", ErrorKind::Invalid, "design.parent", "a page sits in no parent; a move only reorders it among the pages"));
            }
            None
        } else {
            let to = parent.or_else(|| from.clone()).unwrap_or_default();
            if to == id {
                return Err(self.fail_on("parent", ErrorKind::Invalid, "design.cycle", "a node cannot sit inside itself"));
            }
            if !self.container(&to, &id)? {
                return Ok(None);
            }
            // Into another parent: not one inside the node itself.
            if from.as_deref() != Some(to.as_str()) {
                let mut up = self.meta(&to)?.and_then(|(parent, _, _)| parent);
                while let Some(ancestor) = up {
                    if ancestor == id {
                        return Err(self.fail_on("parent", ErrorKind::Invalid, "design.cycle", format!("{to:?} is inside {id:?}")));
                    }
                    up = self.meta(&ancestor)?.and_then(|(parent, _, _)| parent);
                }
                // W14: no deeper than a design nests, unless no deeper than it sat, so a design
                // another build nested past the limit can still be flattened.
                let (here, there) = (self.place(&id)?, self.place(&to)?);
                let deeper = match (&here, &there) {
                    (Some((now, _)), Some((above, _))) => above + 1 > *now,
                    (Some(_), None) => true,
                    // It sits past WALK and goes where a walk reaches a page: no deeper than it was.
                    (None, Some(_)) => false,
                    // Both past it: each counted to its page.
                    (None, None) => match (self.full_depth(&id)?, self.full_depth(&to)?) {
                        (Some(now), Some(above)) => above + 1 > now,
                        _ => true,
                    },
                };
                if deeper {
                    let height = self.height(&id)?;
                    if !self.fits(&to, &id, height)? {
                        return Ok(None);
                    }
                }
                // B6: carried to another page, it parts the connectors between it and its page;
                // taken out of a node an earlier edit carried there, it stays carried as of then.
                let carried = self.chain(&id).skip(1).filter_map(|up| self.crossed.get(up).copied()).max();
                crossed = match (here, there) {
                    (Some((_, was)), Some((_, will))) if was == will => carried,
                    _ => Some(self.at),
                };
            }
            Some(to)
        };
        let old_index = self.index_of(from.as_deref(), &id, ord)?;
        if to == from && self.in_place(to.as_deref(), &id, ord, old_index, index)? {
            // TM-58: it already sits where it was sent. Nothing is written or touched, and
            // nothing needs undoing.
            return Ok(Some(id));
        }
        let ord = self.slot(to.as_deref(), index, Some(&id))?;
        self.tx.prepare_cached("UPDATE design_nodes SET parent_id = ?3, ord = ?4, rev = ?5 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, id, to, ord, self.rev]).bus()?;
        // The parent's id is part of what design.get returns of the node.
        self.bytes += to.as_ref().map_or(0, String::len) as i64 - from.as_ref().map_or(0, String::len) as i64;
        self.touch(&id);
        self.moved.insert(id.clone());
        self.last_depth = None;
        if to.is_some() {
            self.parents.insert(id.clone(), to.clone());
        }
        if let Some(at) = crossed {
            let last = self.crossed.entry(id.clone()).or_insert(at);
            *last = (*last).max(at);
        }
        self.inverse.push(vec![NodeOp::Move { id: id.clone(), parent: from, index: Some(old_index), keep_position: None }]);
        Ok(Some(id))
    }

    pub(super) fn delete(&mut self, id: String) -> Result<Option<String>, BusError> {
        if self.stale(&id, Edit::Delete) != Stale::No {
            return Ok(None);
        }
        let Some(root) = self.row(&id)? else { return self.missing("no such node") };
        if root.kind == Ok(NodeKind::Page) && self.pages <= 1 {
            return Err(self.fail(ErrorKind::Conflict, "design.last_page", "a design keeps at least one page")
                .with_hint("add another page first, or delete the design"));
        }
        let index = self.index_of(root.parent.as_deref(), &id, root.ord)?;
        // Its undo adds them back in this order, so each lands where it was.
        let doomed = self.subtree_rows(id, root)?;
        // Every node of it is read before any goes: the undo re-adds each, and a node this build
        // cannot read could not be put back, so a subtree holding one is not deleted.
        let mut undo = Vec::with_capacity(doomed.len());
        for (i, (node_id, row)) in doomed.iter().enumerate() {
            let Some(node) = self.readable(node_id, row)? else { return Ok(None) };
            undo.push(NodeOp::Add { node, index: (i == 0).then_some(index) });
        }
        let mut remove = self.tx.prepare_cached("DELETE FROM design_nodes WHERE design_id = ?1 AND node_id = ?2").bus()?;
        // Each id removed, at this rev: what design.get's since_rev answers as gone.
        let mut tombstone = self.tx.prepare_cached("INSERT OR REPLACE INTO design_removed(design_id, node_id, rev) VALUES (?1, ?2, ?3)").bus()?;
        for (node_id, row) in doomed {
            remove.execute(params![self.design, node_id]).bus()?;
            tombstone.execute(params![self.design, node_id, self.rev]).bus()?;
            self.bytes -= row_bytes(&node_id, row.parent.as_deref(), &row.body);
            self.parents.remove(&node_id);
            self.removed_at.insert(node_id.clone(), self.at);
            self.removed.push(node_id);
        }
        self.tombstones = true;
        self.last_depth = None;
        self.nodes -= undo.len() as i64;
        self.pages -= i64::from(matches!(&undo[0], NodeOp::Add { node, .. } if node.kind == NodeKind::Page));
        let id = undo[0].target().to_string();
        self.inverse.push(undo);
        Ok(Some(id))
    }

    /// Node `id` (stored as `root`) and every node inside it, each parent before its children and
    /// siblings bottom to top.
    pub(super) fn subtree_rows(&self, id: String, root: Stored) -> Result<Vec<(String, Stored)>, BusError> {
        let mut rows = vec![(id, root)];
        let mut next = 0;
        let mut children = self.tx
            .prepare_cached("SELECT node_id, kind, ord, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id").bus()?;
        while next < rows.len() {
            if matches!(rows[next].1.kind, Ok(kind) if kind.holds_children()) {
                let parent = rows[next].0.clone();
                let found = children.query_map(params![self.design, parent], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?))).bus()?
                    .collect::<rusqlite::Result<Vec<(String, String, f64, String)>>>().bus()?;
                for (child, kind, ord, body) in found {
                    rows.push((child, Stored { parent: Some(parent.clone()), kind: row_kind(kind), ord, body }));
                }
            }
            next += 1;
        }
        Ok(rows)
    }

    /// Node `id` as stored now, or as an edit of this list deleted it (its undo re-adds it).
    /// A node this build cannot read has no box it can use: `None`.
    pub(super) fn stored(&self, id: &str) -> Result<Option<Node>, BusError> {
        if let Some(row) = self.row(id)? {
            return Ok(node_of(id.to_string(), row.parent, &row.body).ok());
        }
        Ok(self.inverse.iter().rev().flatten().find_map(|op| match op {
            NodeOp::Add { node, .. } if node.id == id => Some(node.clone()),
            _ => None,
        }))
    }

    /// The page box of node `id` from the stored `x`/`y` up its parents (no auto layout).
    pub(super) fn stored_box(&self, id: &str) -> Result<Option<(f64, f64, f64, f64)>, BusError> {
        let Some(node) = self.stored(id)? else { return Ok(None) };
        let (w, h) = (node.w, node.h);
        let (mut x, mut y, mut up) = (node.x, node.y, node.parent);
        for _ in 0..MAX_NODES {
            let Some(parent) = up else { return Ok(Some((x, y, w, h))) };
            let Some(p) = self.stored(&parent)? else { return Ok(None) };
            (x, y, up) = (x + p.x, y + p.y, p.parent);
        }
        Ok(None)
    }

    /// Connector `id` lets go of its `ends` (`from`, `to`). It keeps its own points (where it
    /// was last drawn); one with fewer than two is given its ends' stored centres. The undo
    /// attaches it again.
    pub(super) fn free(&mut self, id: &str, ends: &[&str]) -> Result<(), BusError> {
        let Some(row) = self.row(id)? else { return Ok(()) };
        let Ok(node) = node_of(id.to_string(), row.parent, &row.body) else { return Ok(()) };
        let set = self.letting_go(&node, ends)?;
        // A line that cannot be freed (none should fail) keeps its attachment.
        let Ok(mut next) = node.patched(&set) else { return Ok(()) };
        let body = body_of(&mut next)?;
        self.tx.prepare_cached("UPDATE design_nodes SET body = ?3, rev = ?4 WHERE design_id = ?1 AND node_id = ?2").bus()?
            .execute(params![self.design, next.id, body, self.rev]).bus()?;
        self.bytes += body.len() as i64 - row.body.len() as i64;
        self.touch(&next.id);
        let before = node.before(&set);
        self.updated.entry(next.id.clone()).or_default().extend(before.keys().cloned());
        self.inverse.push(vec![NodeOp::Update { id: next.id.clone(), set: before }]);
        Ok(())
    }

    /// The update that lets connector `node`, as stored, go of its `ends` where it is drawn by
    /// the stored boxes: each end `null`, and its points kept, or for one with fewer than two,
    /// its ends' stored centres.
    pub(super) fn letting_go(&self, node: &Node, ends: &[&str]) -> Result<Map<String, Value>, BusError> {
        let mut set: Map<String, Value> = ends.iter().map(|end| (end.to_string(), Value::Null)).collect();
        if node.points.as_ref().is_none_or(|p| p.len() < 2) {
            let centre = |b: Option<(f64, f64, f64, f64)>| b.map(|(x, y, w, h)| (x + w / 2.0, y + h / 2.0));
            let ends = match node.from.as_deref().zip(node.to.as_deref()) {
                Some((from, to)) => (self.stored_box(&node.id)?, centre(self.stored_box(from)?), centre(self.stored_box(to)?)),
                None => (None, None, None),
            };
            let points = match ends {
                (Some((ox, oy, _, _)), Some(a), Some(b)) => json!([[(a.0 - ox).round(), (a.1 - oy).round()], [(b.0 - ox).round(), (b.1 - oy).round()]]),
                _ => json!([[0.0, 0.0], [node.w, node.h]]),
            };
            set.insert("points".into(), points);
        }
        Ok(set)
    }

    /// The last edit of this list that moved connector `line` or its `end`, or a node holding
    /// either, to another page, when they now sit on different pages. `None` when they share a
    /// page, or when the list moved neither: a connector it leaves alone keeps its ends. Read off
    /// `parents` alone, which holds the whole design by then ([`Run::learn_all`]).
    fn parted_at(&self, line: &str, end: &str) -> Option<usize> {
        let at = self.chain(line).chain(self.chain(end)).filter_map(|id| self.crossed.get(id).copied()).max()?;
        let page = |id| self.chain(id).last().filter(|top| matches!(self.parents.get(*top), Some(None)));
        match (page(line), page(end)) {
            (Some(a), Some(b)) if a != b => Some(at),
            _ => None,
        }
    }

    /// Connectors attached to a node this list deleted let go of that end, so no line names a
    /// node that is gone; so do those a move of this list left on another page than an end of
    /// theirs (B6, MODEL-SPEC §2.11). An end the list wrote after the edit that took its node
    /// away is left for [`Run::check_refs`], which refuses it in a strict list.
    pub(super) fn release_ends(&mut self) -> Result<(), BusError> {
        if self.removed.is_empty() && self.crossed.is_empty() {
            return Ok(());
        }
        // The last edit that wrote each line's `from` and `to`.
        let mut written: HashMap<String, [Option<usize>; 2]> = HashMap::new();
        for r in self.refs.iter().filter(|r| r.field != "src") {
            written.entry(r.id.clone()).or_default()[usize::from(r.field == "to")] = Some(r.at);
        }
        // The text test is loose (a text could hold the words); each body is read to be sure.
        let attached: Vec<(String, Option<String>, String)> = self.tx
            .prepare_cached("SELECT node_id, parent_id, body FROM design_nodes WHERE design_id = ?1
                AND (instr(body, '\"from\":') > 0 OR instr(body, '\"to\":') > 0)").bus()?
            .query_map([self.design], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        // After a move to another page each connector's ends are placed, so where every node sits
        // is read in one query rather than a chain at a time (MODEL-SPEC §5 #5).
        if !self.crossed.is_empty() && !attached.is_empty() {
            self.learn_all()?;
        }
        let mut releases = Vec::new();
        for (id, parent, body) in attached {
            // A row this build cannot read is left as it is (a newer build's own kind of line).
            let node = match node_of(id, parent, &body) {
                Ok(node) => node,
                Err(error) => {
                    tracing::warn!(design = self.design, "design.apply: {}; its ends are left as they are", error.message);
                    continue;
                }
            };
            let mut ends = Vec::new();
            for (i, (field, end)) in [("from", &node.from), ("to", &node.to)].into_iter().enumerate() {
                let Some(end) = end.as_deref() else { continue };
                let parted = match self.removed_at.get(end) {
                    Some(&at) if self.meta(end)?.is_none() => Some(at),
                    _ if self.crossed.is_empty() => None,
                    _ => self.parted_at(&node.id, end),
                };
                let set = written.get(node.id.as_str()).and_then(|at| at[i]);
                if parted.is_some_and(|parted| set.is_none_or(|set| set < parted)) {
                    ends.push(field);
                }
            }
            if !ends.is_empty() {
                releases.push((node.id, ends));
            }
        }
        for (id, ends) in releases {
            self.free(&id, &ends)?;
        }
        Ok(())
    }

    /// Why a connector `line` cannot hold `end` at either end, if it cannot (B6): the refusal's
    /// kind and code, and what `end` is. Nothing when it sits too deep to tell.
    fn misplaced(&mut self, line: &str, end: &str) -> Result<Option<(ErrorKind, &'static str, String)>, BusError> {
        let connector = |why: String| Ok(Some((ErrorKind::Invalid, "design.connector", why)));
        if end == line {
            return connector("is this line itself".into());
        }
        let Some((_, page)) = self.place(end)? else {
            if self.parents.contains_key(end) {
                return Ok(None);
            }
            return Ok(Some((ErrorKind::NotFound, "design.node_not_found", format!("is not in the design{}", self.gone_note(end)?))));
        };
        if page == end {
            return connector("is a page, not a node on one".into());
        }
        match self.place(line)? {
            Some((_, own)) if own != page => connector(format!("is on page {page:?}, not on this line's page {own:?}")),
            _ => Ok(None),
        }
    }

    /// B6, once the edits have run and the ends they parted have let go: each connector end and
    /// picture the list wrote names what is there, as the node holds it now. An end is a node of
    /// the line's own page that is not the line or a page; a `src`, a stored picture (a stat,
    /// not a read). A strict list is refused at the edit that wrote it; a lenient one lets go of
    /// such an end (as a delete does) and keeps such a picture, telling an agent.
    pub(super) fn check_refs(&mut self) -> Result<(), BusError> {
        let refs = std::mem::take(&mut self.refs);
        let last: HashMap<(&str, &str), usize> = refs.iter().enumerate().map(|(i, r)| ((r.id.as_str(), r.field), i)).collect();
        let mut held: Option<Node> = None;
        for (i, r) in refs.iter().enumerate() {
            if last[&(r.id.as_str(), r.field)] != i {
                continue;
            }
            if held.as_ref().is_none_or(|node| node.id != r.id) {
                held = match self.row(&r.id)? {
                    Some(row) => node_of(r.id.clone(), row.parent, &row.body).ok(),
                    None => None,
                };
            }
            // Gone by now: a later edit deleted it.
            let Some(node) = held.as_ref() else { continue };
            (self.at, self.label, self.target) = (r.at, r.label.clone(), Some(r.id.clone()));
            if r.field == "src" {
                let Some(src) = node.src.as_deref().filter(|src| !self.assets.join(src).is_file()) else { continue };
                let why = format!("src: {src:?} is not a stored picture");
                if !self.lenient {
                    return Err(self.fail(ErrorKind::NotFound, "design.asset_not_found", why)
                        .with_hint("store it with design.asset.add and use the src it returns"));
                }
                self.warn(format_args!("{why}; kept as it was"));
                continue;
            }
            let Some(end) = (if r.field == "from" { node.from.clone() } else { node.to.clone() }) else { continue };
            let Some((kind, code, what)) = self.misplaced(&r.id, &end)? else { continue };
            if !self.lenient {
                let hint = match kind {
                    ErrorKind::NotFound => self.nearest_id(&end)?,
                    _ => "attach it to a node on the same page".to_string(),
                };
                let mut error = self.fail(kind, code, format!("{}: {end:?} {what}", r.field)).with_hint(hint);
                if let Some(Value::Object(details)) = error.details.as_mut() {
                    details.insert("end".into(), end.into());
                }
                return Err(error);
            }
            self.warn(format_args!("{}: let go of {end:?}, which {what}", r.field));
            self.free(&r.id, &[r.field])?;
            held = None;
        }
        Ok(())
    }

    /// Nothing after the edits can change the design's size: no node was deleted or moved to
    /// another page (whose connectors [`Run::release_ends`] rewrites) and no lenient end may let
    /// go ([`Run::check_refs`]). A pass that rewrites nodes after the edits joins this test.
    pub(super) fn settled(&self) -> bool {
        self.removed.is_empty() && self.crossed.is_empty() && !(self.lenient && self.refs.iter().any(|r| r.field != "src"))
    }

    /// `design.too_large` for a design of `self.bytes` (B60): `at_least` while edits remain.
    fn too_large(&self, at_least: bool) -> BusError {
        let why = format!("the design would hold {}{} KiB of nodes as design.get returns them; the limit is {} KiB",
            if at_least { "at least " } else { "" }, self.bytes >> 10, MAX_DESIGN_BYTES >> 10);
        let error = if at_least { self.fail(ErrorKind::Invalid, "design.too_large", why) } else { BusError::invalid("design.too_large", why) };
        error.with_hint(TOO_LARGE_HINT)
    }
}

pub(super) fn register(e: &mut Engine) {
    e.register::<Apply>(|ctx: &mut Ctx, p| {
        // An undo re-adds every node a delete took, one edit each, so a lenient list of the
        // person's (an undo or a redo) may hold a whole design's worth besides a list's. An
        // agent's lists, lenient or not, stay within MAX_OPS.
        let (lenient, agent) = (p.lenient.unwrap_or(false), ctx.actor.is_agent());
        let most = if lenient && !agent { MAX_LENIENT_OPS } else { MAX_OPS };
        if p.ops.is_empty() || p.ops.len() > most {
            return Err(BusError::invalid("design.ops", format!("give 1 to {most} edits; this list has {}", p.ops.len())));
        }
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        // W5: an agent reads what changed with design.get or design.render; the person's clients
        // keep the nodes and the undo.
        let full = p.reply.unwrap_or(if agent { Reply::Short } else { Reply::Full }) == Reply::Full;
        // W6: a list sent against an older rev applies on top of what changed since, unless it
        // writes a node that changed: refused, or when lenient, that node's edits skipped.
        let (mut rebased, mut stale) = (false, HashMap::new());
        if let Some(base) = p.base_rev.filter(|&base| base != design.rev) {
            if base > design.rev {
                return Err(revs::ahead(&design, base));
            }
            let conflicts = revs::base_conflicts(ctx.tx(), design.id, &p.ops, base)?;
            if !conflicts.is_empty() && !lenient {
                return Err(revs::stale(&design, base, &conflicts));
            }
            rebased = conflicts.is_empty();
            for conflict in conflicts {
                stale.entry(conflict.id.clone()).or_insert_with(Vec::new).push(conflict);
            }
        }
        // W7, W8: what the nodes must still hold (an undo's `expect`): refused when they do not,
        // or when lenient, just the fields or edits that would overwrite a later change skipped.
        if let Some(expect) = p.expect.as_deref().filter(|expect| !expect.is_empty()) {
            let conflicts = expect::expect_conflicts(ctx.tx(), design.id, &p.ops, expect, false)?;
            if !conflicts.is_empty() && !lenient {
                return Err(expect::stale(&design, &conflicts));
            }
            for conflict in conflicts {
                stale.entry(conflict.id.clone()).or_insert_with(Vec::new).push(conflict);
            }
        }
        let by = actor_name(ctx)?;
        let (bytes, next_node): (i64, i64) = ctx.tx().prepare_cached("SELECT bytes, next_node FROM designs WHERE id = ?1").bus()?
            .query_row([design.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?;
        let tombstones: bool = ctx.tx().prepare_cached("SELECT EXISTS(SELECT 1 FROM design_removed WHERE design_id = ?1)").bus()?
            .query_row([design.id], |r| r.get(0)).bus()?;
        let assets = ctx.engine().store.design_assets_dir();
        let mut run = Run {
            tx: ctx.tx(),
            design: design.id,
            lenient,
            agent,
            warnings: Vec::new(),
            pages: design.pages,
            nodes: design.nodes,
            bytes,
            next_node,
            rev: design.rev + 1,
            tombstones,
            at: 0,
            label: String::new(),
            target: None,
            touched: Vec::new(),
            seen: HashSet::new(),
            removed: Vec::new(),
            ids: Vec::with_capacity(p.ops.len()),
            inverse: Vec::with_capacity(p.ops.len()),
            parents: HashMap::new(),
            last_depth: None,
            refs: Vec::new(),
            removed_at: HashMap::new(),
            crossed: HashMap::new(),
            assets,
            conflicts: stale,
            skipped_adds: HashSet::new(),
            skipped: Vec::new(),
            skipped_total: 0,
            updated: HashMap::new(),
            moved: HashSet::new(),
            added: HashSet::new(),
            styles: None,
            styles_len: 0,
            styles_changed: false,
            copied: BTreeMap::new(),
        };
        // B59: past the last edit that can make the design smaller (any but an add or a copy),
        // the list only grows it, so one over the cap by then is refused at the edit that took
        // it over rather than after writing the rest.
        let grows_from = p.ops.iter().rposition(|op| !matches!(op, NodeOp::Add { .. } | NodeOp::Copy { .. }));
        for (at, op) in p.ops.into_iter().enumerate() {
            (run.at, run.label) = (at, label(at, &op));
            run.target = Some(op.target()).filter(|id| !id.is_empty()).map(str::to_string);
            let id = match op {
                NodeOp::Add { node, index } => run.add(node, index)?,
                NodeOp::Update { id, set } => run.update(id, set)?,
                // Until engine-pins rewrites x/y from page points (MODEL-SPEC §2.11).
                NodeOp::Move { keep_position: Some(true), .. } => {
                    return Err(run.fail_on("keep_position", ErrorKind::Invalid, "design.ops", "keep_position is not supported yet")
                        .with_hint("move it without keep_position, then set x and y in an update"));
                }
                NodeOp::Move { id, parent, index, .. } => run.move_(id, parent, index)?,
                NodeOp::Delete { id } => run.delete(id)?,
                NodeOp::Copy { id, as_id, parent, index, dx, dy } => run.copy(id, as_id, parent, index, dx, dy)?,
                NodeOp::Color { name, value } => run.color(name, value)?,
                NodeOp::Style { name, set } => run.style(name, set)?,
                NodeOp::DeleteStyle { name } => run.delete_style(name)?,
            };
            run.ids.push(id.unwrap_or_default());
            if run.bytes > MAX_DESIGN_BYTES && run.bytes > bytes && grows_from.is_none_or(|last| at >= last) && run.settled() {
                return Err(run.too_large(true));
            }
        }
        run.release_ends()?;
        run.check_refs()?;
        run.write_styles()?;
        // B60: a design over the cap (one counted before ids were) can still be edited down; a
        // list is refused only for growing it past the cap.
        if run.bytes > MAX_DESIGN_BYTES && run.bytes > bytes {
            return Err(run.too_large(false));
        }
        if run.skipped_total > run.skipped.len() && run.warnings.len() < MAX_WARNINGS {
            run.warnings.push(format!("{} more edits were skipped; skipped lists the first {MAX_SKIPPED}", run.skipped_total - run.skipped.len()));
        }
        let skipped = std::mem::take(&mut run.skipped);
        let prev_rev = design.rev;
        let mut out = ApplyOut {
            design_id: design.id,
            rev: prev_rev,
            prev_rev,
            rebased,
            ids: std::mem::take(&mut run.ids),
            removed_count: 0,
            audit_id: None,
            skipped,
            warnings: Vec::new(),
            copied: std::mem::take(&mut run.copied),
            design: None,
            nodes: None,
            removed: None,
            inverse: None,
            expect: None,
            colors: None,
            styles: None,
            reread: false,
        };
        if run.inverse.is_empty() {
            // TM-58, MODEL-SPEC §1 #20: every edit found what it sent already there, or was
            // skipped. The design stays as it was: the same rev, no event and no undo.
            out.warnings = std::mem::take(&mut run.warnings);
            if full {
                (out.design, out.nodes, out.removed, out.inverse) = (Some(design), Some(Vec::new()), Some(Vec::new()), Some(Vec::new()));
            }
            return Ok(out);
        }
        ctx.tx().prepare_cached(
            "UPDATE designs SET rev = rev + 1, pages = ?2, nodes = ?3, bytes = ?4, next_node = ?5, updated_by = ?6, updated_at = ?7 WHERE id = ?1",
        ).bus()?.execute(params![design.id, run.pages, run.nodes, run.bytes, run.next_node, by, ctx.now]).bus()?;
        if !run.removed.is_empty() {
            revs::prune(ctx.tx(), design.id, run.rev)?;
        }
        // Each node touched as stored now, and what it costs a full answer (B7): its body, id
        // and parent, and the JSON around them.
        let mut nodes = Vec::with_capacity(run.touched.len());
        let mut nodes_len = 0;
        for id in std::mem::take(&mut run.touched) {
            if let Some(row) = run.row(&id)? {
                nodes_len += row.body.len() + id.len() + row.parent.as_ref().map_or(0, String::len) + 32;
                nodes.push(node_of(id, row.parent, &row.body)?);
            }
        }
        // What is gone now: a node deleted and added back in the same list is in `nodes`.
        let mut gone: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let removed: Vec<String> = std::mem::take(&mut run.removed).into_iter()
            .filter(|id| gone.insert(id.clone()))
            .collect();
        let inverse: Vec<NodeOp> = std::mem::take(&mut run.inverse).into_iter().rev().flatten().collect();
        // W7: what the undo expects to find, kept with it in the audit and given to the person's
        // clients with it.
        let expect = expect::undo_expect(ctx.tx(), design.id, &nodes, &run.updated, &run.moved, &run.added)?;
        let mut warnings = std::mem::take(&mut run.warnings);
        let (pages, count) = (run.pages, run.nodes);
        let design = Design { rev: prev_rev + 1, pages, nodes: count, updated_by: by, updated_at: ctx.now.clone(), ..design };
        // The audit keeps the undo uncapped (payloads and results stop at 64 KiB): a big delete's
        // would store every node it took. Past UNDO_KEPT only the caller's copy, in the answer,
        // can undo it. The replay's own answer, which audit.undo drops, is kept short.
        let undo = json!({"design_id": design.id, "ops": inverse, "lenient": true, "reply": "short"});
        let kept = json!({"nodes": expect});
        let undo_len = serde_json::to_vec(&undo).map_or(usize::MAX, |bytes| bytes.len())
            .saturating_add(serde_json::to_vec(&kept).map_or(usize::MAX, |bytes| bytes.len()));
        if undo_len <= UNDO_KEPT {
            ctx.set_undo("design.apply", undo, Some(kept));
        } else if warnings.len() < MAX_WARNINGS {
            warnings.push(format!("this change is too large for the audit log to undo (over {} KiB)", UNDO_KEPT >> 10));
        }
        emit_applied(ctx, &design, prev_rev, nodes.iter().map(|n| n.id.as_str()), &removed, run.styles_changed)?;
        // The audit row this request's answer is written as, in this transaction (audit.undo's
        // replay answers with the row of the undo itself, which it does not return).
        out.audit_id = Some(crate::audit::next_id(ctx.tx()).bus()?);
        (out.rev, out.removed_count) = (design.rev, removed.len() as u32);
        if full {
            // B7: a full answer stays under the clients' 2 MiB line: past REPLY_BUDGET it leaves
            // out the nodes (the client reads the design again), and past it without them the
            // undo too, which the person is told.
            let removed_len: usize = removed.iter().map(|id| id.len() + 3).sum();
            out.reread = nodes_len + undo_len + removed_len > REPLY_BUDGET;
            if !out.reread {
                out.nodes = Some(nodes);
            }
            if undo_len + removed_len > REPLY_BUDGET {
                if warnings.len() < MAX_WARNINGS {
                    warnings.push("this change is too large to undo from the board".to_string());
                }
            } else {
                (out.inverse, out.expect, out.removed) = (Some(inverse), Some(expect), Some(removed));
            }
            out.design = Some(design);
            if let Some(styles) = run.styles.take().filter(|_| run.styles_changed) {
                (out.colors, out.styles) = (Some(styles.colors), Some(styles.styles));
            }
        }
        out.warnings = warnings;
        Ok(out)
    });
}
