//! `design.get`: a design's rows copied out under the lock, ordered and parsed after it.
//!
//! A read can be cut to a page, to some nodes and what is inside them, and to what changed since
//! a rev (`since_rev`, W20): every row is then still copied out for its place, but only the
//! bodies of rows written since, so a reader that holds the design at that rev brings it up to
//! now from a small answer.

use super::revs::removed_rev;
use super::{get_design, kind_of};
use crate::engine::{Engine, IntoBus};
use crate::handlers::notes::check_project;
use relay_bus::design::{nearest, Node, NodeKind, Styles, SINCE_WINDOW};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Get, GetIn, GetOut, GET_NODES_MAX};
use relay_bus::types::{Design, Id};
use rusqlite::params;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Every row of a design. A row of a kind this build does not know fails to parse, which is how a
/// whole read leaves it out, so its `kind` is not read.
pub(super) const ROWS: &str = "SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1";
/// A design's pages alone (`pages_only`), found through `design_nodes_parent` without a pass
/// over the rest of the design. The `ORDER BY` is what the index already gives, and what makes
/// SQLite take it: without statistics it reckons `IS NULL` matches most rows and scans the
/// design's whole primary key instead (5,000 rows, 1.5 MiB of bodies, under the store lock).
const PAGE_ROWS: &str = "SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1 AND parent_id IS NULL ORDER BY ord, node_id";
/// Every row of a design, its kind (a body not copied cannot say it is one this build cannot
/// read) and whether it was written after rev `?2`, with its body only where it was, where it is
/// a page (a page is found by its name), or where `?3` asks for every body: a read cut to a page
/// or to some nodes sends what is inside a changed node, which may have moved in from outside
/// what it reads.
const DELTA_ROWS: &str = "SELECT node_id, parent_id, ord, kind, rev > ?2, CASE WHEN ?3 OR rev > ?2 OR parent_id IS NULL THEN body END
    FROM design_nodes WHERE design_id = ?1";
/// The ids removed after rev `?2`, in the order they went.
const REMOVED: &str = "SELECT node_id FROM design_removed WHERE design_id = ?1 AND rev > ?2 ORDER BY rev, node_id";

/// Pages a `design.page_not_found` hint names before it counts the rest.
const HINT_PAGES: usize = 20;
/// Unknown ids a `design.node_not_found` for `nodes` lists in its details.
const HINT_IDS: usize = 20;

/// One node row as `design.get` copies it out under the lock; parsed once the lock is gone.
pub(super) struct Raw {
    pub(super) id: String,
    pub(super) parent: Option<String>,
    pub(super) ord: f64,
    /// Of a kind this build knows: one it does not is a row it cannot read.
    pub(super) known: bool,
    /// Written after `since_rev`; every row of a whole read.
    pub(super) changed: bool,
    /// None where a `since_rev` read does not need it.
    pub(super) body: Option<String>,
}

/// What a read takes of a design's rows.
pub(super) struct Scope<'a> {
    /// Only this page and what is on it: its id or the name it goes by.
    pub(super) page: Option<&'a str>,
    /// Only these nodes and what is inside them.
    pub(super) named: Option<HashSet<&'a str>>,
    /// Only what changed since a rev (`since_rev`).
    pub(super) delta: bool,
}

impl Scope<'_> {
    fn cut(&self) -> bool {
        self.page.is_some() || self.named.is_some()
    }
}

/// What a read answers of the rows.
pub(super) struct Read {
    pub(super) nodes: Vec<Node>,
    /// With `since_rev`: each node's index among its siblings now, in the order of `nodes`.
    pub(super) indexes: Vec<u32>,
    /// With `since_rev`: the rows a reader that held the read at that rev drops: each written
    /// since that the answer leaves out (outside what it reads now, or a row this build cannot
    /// read), and what is inside such a node that the read does not take.
    pub(super) left: Vec<String>,
}

/// Whether the `kind` at column `i` is one this build knows, read without copying it out.
fn known_kind(r: &rusqlite::Row, i: usize) -> rusqlite::Result<bool> {
    let kind = r.get_ref(i)?.as_str()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(kind_of(kind).is_some())
}

/// `rows` in `design.get`'s order: the pages by `ord`, then each page's nodes in pre-order,
/// siblings bottom to top, cut to the `scope`. A node whose parent is gone (none should be) is left
/// out rather than shown nowhere; so is a row this build cannot read (a newer build's kind, a body
/// that is not a node), with what is inside it, rather than failing the whole design (B61).
///
/// With `since_rev` (`scope.delta`) only the rows written since are answered, each with its index
/// among the siblings a read answers (a row of a kind this build does not know is not one).
/// Cut to a page or to nodes, a node written since comes with what is inside it (it may have
/// moved in, and what it holds with it), unless it is the page or a node named, which cannot
/// have; and a row written since that is no longer in what the read takes is `left` (it may
/// have moved out) with what is inside it that the read does not take. A body a delta did not
/// copy is not parsed, so an unchanged row of a known kind that does not read is not seen to be
/// left out.
pub(super) fn ordered(design: Id, rows: Vec<Raw>, scope: &Scope) -> Result<Read, BusError> {
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
    // Each row's index among its siblings as a reader holds them, which is how an add or a move
    // counts it but for rows of a kind this build does not know: no read answers those.
    let mut place = vec![0u32; if scope.delta { rows.len() } else { 0 }];
    if scope.delta {
        for list in std::iter::once(&pages).chain(children.values()) {
            for (k, &i) in list.iter().filter(|&&i| rows[i].known).enumerate() {
                place[i] = k as u32;
            }
        }
    }
    let mut chosen = None;
    if let Some(wanted) = scope.page {
        // A page by id costs nothing more; the pages' bodies are read for their names only when
        // no id matched.
        let id = if pages.iter().any(|&i| rows[i].id == wanted) {
            wanted.to_string()
        } else {
            let named: Vec<(String, Option<String>)> = pages.iter()
                .filter_map(|&i| named_page(&rows[i].id, rows[i].body.as_deref().unwrap_or_default())).collect();
            resolve_page(&named, wanted)?
        };
        // A delta walks every page, to find what left this one.
        if !scope.delta {
            pages.retain(|&i| rows[i].id == id);
        }
        chosen = Some(id);
    }
    // Each row in order, with the row it sits in (none for a page).
    let mut order: Vec<(usize, Option<usize>)> = pages.iter().map(|&p| (p, None)).collect();
    let mut stack = Vec::new();
    for &p in &pages {
        stack.extend(children.get(rows[p].id.as_str()).into_iter().flatten().rev().map(|&c| (c, p)));
        while let Some((i, up)) = stack.pop() {
            order.push((i, Some(up)));
            stack.extend(children.get(rows[i].id.as_str()).into_iter().flatten().rev().map(|&c| (c, i)));
        }
    }
    drop(children);
    let n = rows.len();
    // Per row: on the page read; inside a node named; kept (both, as the read asks, and
    // readable: a reader holds it, sent or not); left out, with what is inside it; in a delta cut
    // to a page or nodes, sent whole with what it holds; and in a delta, in `left` with what is
    // inside it that the read does not keep.
    let (mut on_page, mut within, mut kept) = (vec![false; n], vec![false; n], vec![false; n]);
    let (mut left_out, mut whole, mut dropping) = (vec![false; n], vec![false; n], vec![false; n]);
    let mut rows: Vec<Option<Raw>> = rows.into_iter().map(Some).collect();
    let mut read = Read { nodes: Vec::with_capacity(order.len()), indexes: Vec::new(), left: Vec::new() };
    for (i, up) in order {
        let Some(Raw { id, parent, known, changed, body, .. }) = rows[i].take() else { continue };
        let named = scope.named.as_ref().is_some_and(|named| named.contains(id.as_str()));
        let (page, inside, gone, sent, drops) = match up {
            None => (chosen.as_deref().is_none_or(|chosen| chosen == id), named, false, false, false),
            Some(p) => (on_page[p], named || within[p], left_out[p], kept[p] && whole[p], dropping[p]),
        };
        (on_page[i], within[i]) = (page, inside);
        let taken = page && (scope.named.is_none() || inside);
        left_out[i] = gone || !known;
        // Cut to a page or to nodes, a node written since may have moved in from outside, with all
        // it holds; the page or a node named cannot have.
        whole[i] = sent || (scope.delta && scope.cut() && changed && up.is_some() && !named);
        if taken && !left_out[i] && (!scope.delta || changed || sent) {
            // As `node_of` reads a row, keeping the id for a delta's `left` should it not read.
            match body.as_deref().map(Node::read_str) {
                Some(Ok(mut node)) => {
                    (node.id, node.parent) = (id, parent);
                    read.nodes.push(node);
                    if scope.delta {
                        read.indexes.push(place[i]);
                    }
                    kept[i] = true;
                    continue;
                }
                Some(Err(e)) => tracing::warn!(design, "design.get: node {id:?} cannot be read ({e}); left out with what is inside it"),
                None => tracing::warn!(design, "design.get: node {id:?}'s body was not read; left out with what is inside it"),
            }
            left_out[i] = true;
        }
        kept[i] = taken && !left_out[i];
        // Written since and not sent: it went out of what the read takes, or cannot be read; so did
        // what it holds, unless the read takes that by itself (a node named inside it). A page
        // moves nowhere: what is on another one was never in a page's read.
        if scope.delta && (changed || (drops && !kept[i])) {
            dropping[i] = up.is_some();
            read.left.push(id);
        }
    }
    Ok(read)
}

/// A page row as [`resolve_page`] takes it: its id and stored name, read as `design.get` reads the
/// page, so a name this build cannot read is none here as it is on the canvas. `None` for a row
/// that does not read at all, which `design.get` leaves out and the canvas never shows.
pub(crate) fn named_page(id: &str, body: &str) -> Option<(String, Option<String>)> {
    Node::read_str(body).ok().map(|page| (id.to_string(), page.name))
}

/// The name a page goes by: its own, or "Page n" for one without (n its place among the pages,
/// from 1), as the canvas and the phone show it.
fn shown(name: Option<&str>, at: usize) -> String {
    match name {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => format!("Page {}", at + 1),
    }
}

/// The id of the page `wanted` names among `pages` (each page's id and stored name, in the
/// design's page order): the page with that id, else the one page that goes by that name, its
/// own or, for a page without one, the "Page n" the canvas shows it by. Names match as written.
/// Shared by design.get and design.render, so `page` means one thing in both reads.
///
/// `design.page_not_found` otherwise, naming a near miss, with a hint that lists the pages to pick
/// from by id (those sharing the name, when two or more do).
pub(crate) fn resolve_page(pages: &[(String, Option<String>)], wanted: &str) -> Result<String, BusError> {
    if let Some((id, _)) = pages.iter().find(|(id, _)| id == wanted) {
        return Ok(id.clone());
    }
    let shown: Vec<String> = pages.iter().enumerate().map(|(at, (_, name))| shown(name.as_deref(), at)).collect();
    let named: Vec<usize> = (0..pages.len()).filter(|&at| shown[at] == wanted).collect();
    let message = match named[..] {
        [at] => return Ok(pages[at].0.clone()),
        [] => {
            // Each page's id, then the name it goes by: a near miss names the page's id beside a name.
            let near = |candidate: &str| pages.iter().zip(&shown).find_map(|((id, _), name)| match candidate {
                c if c == id => Some(format!("{c:?}")),
                c if c == name => Some(format!("{c:?} ({id})")),
                _ => None,
            });
            match nearest(wanted, pages.iter().zip(&shown).flat_map(|((id, _), name)| [id.as_str(), name.as_str()])).and_then(near) {
                Some(near) => format!("the design has no page {wanted:?}; did you mean {near}?"),
                None => format!("the design has no page {wanted:?}"),
            }
        }
        ref several => format!("{} pages are named {wanted:?}", several.len()),
    };
    let listed: Vec<usize> = if named.is_empty() { (0..pages.len()).collect() } else { named };
    let hint = if pages.is_empty() {
        "the design has no pages: add one with design.apply".to_string()
    } else {
        let mut hint = String::from("pages: ");
        for (k, &at) in listed.iter().take(HINT_PAGES).enumerate() {
            let sep = if k == 0 { "" } else { ", " };
            let _ = write!(hint, "{sep}{} ({:?})", pages[at].0, shown[at]);
        }
        if listed.len() > HINT_PAGES {
            let _ = write!(hint, ", and {} more (design.get pages_only lists them all)", listed.len() - HINT_PAGES);
        }
        hint.push_str("; pass a page's id");
        hint
    };
    Err(BusError::not_found("design.page_not_found", message).with_hint(hint))
}

/// `pages_only` answers the pages without what is on them, so a node scope or a `since_rev`
/// beside it would mean nothing: refused by name rather than one of them ignored.
fn check_pages_only(p: &GetIn) -> Result<bool, BusError> {
    if p.pages_only != Some(true) {
        return Ok(false);
    }
    let clash = match (&p.nodes, p.since_rev) {
        (Some(_), _) => Some(("nodes", "drop pages_only to read these nodes and what is inside them")),
        (None, Some(_)) => Some(("since_rev", "drop pages_only to read what changed since that rev")),
        (None, None) => None,
    };
    match clash {
        Some((field, hint)) => Err(BusError::invalid("design.pages_only", format!("pages_only: give it alone or with page, not with {field}"))
            .with_hint(hint)),
        None => Ok(true),
    }
}

/// `nodes` names 1 to [`GET_NODES_MAX`] ids, and `since_rev` is a rev: refused by name before
/// the design is read.
fn check_reach(p: &GetIn) -> Result<(), BusError> {
    if let Some(nodes) = &p.nodes {
        if nodes.is_empty() || nodes.len() > GET_NODES_MAX {
            return Err(BusError::invalid("design.nodes", format!("nodes: give 1 to {GET_NODES_MAX} ids; this has {}", nodes.len()))
                .with_hint("leave nodes out to read the whole design, or pass page to read one page"));
        }
    }
    if p.since_rev.is_some_and(|since| since < 0) {
        return Err(BusError::invalid("design.since_rev", "since_rev: a rev is 0 or more")
            .with_hint("send the rev design.get or design.apply last answered with"));
    }
    Ok(())
}

/// `since_rev` is a rev of `design` that a read can still reach: not past its rev, and within the
/// last [`SINCE_WINDOW`] revs, whose removals are on record, from `tracked_from` on (the rev a
/// design held when its store began keeping them).
fn check_since(design: &Design, tracked_from: i64, since: i64) -> Result<(), BusError> {
    if since > design.rev {
        return Err(BusError::invalid("design.since_rev", format!("since_rev: design {} is at rev {}; {since} is ahead of it", design.id, design.rev))
            .with_hint("send the rev design.get or design.apply last answered with"));
    }
    if since < design.rev - SINCE_WINDOW || since < tracked_from {
        let why = if since < tracked_from {
            format!("since_rev: rev {since} is before rev {tracked_from}, when this design's removals began to be kept")
        } else {
            format!("since_rev: rev {since} is more than {SINCE_WINDOW} revs behind the design's ({}); what was removed that long ago is not kept", design.rev)
        };
        return Err(BusError::conflict("design.since_too_old", why)
            .with_details(json!({"rev": design.rev}))
            .with_hint("read it whole: design.get without since_rev"));
    }
    Ok(())
}

/// `design.node_not_found` for the ids of `nodes` the design does not hold (`unknown`, in the
/// order named), with the nearest id it does hold and when the first was removed, if that is on
/// record. Read on the way to the refusal only.
fn unknown_nodes(conn: &rusqlite::Connection, design: Id, unknown: &[&str], ids: &[&str]) -> Result<BusError, BusError> {
    let first = unknown[0];
    let gone = removed_rev(conn, design, first)?.map(|rev| format!(" (removed at rev {rev})")).unwrap_or_default();
    let more = match unknown.len() {
        1 => String::new(),
        n => format!(", nor are {} more of them", n - 1),
    };
    let hint = match nearest(first, ids.iter().copied()) {
        Some(id) => format!("did you mean {id:?}?"),
        None => "design.get lists the ids (pass page to read one page)".to_string(),
    };
    Ok(BusError::not_found("design.node_not_found", format!("nodes: {first:?} is not in the design{gone}{more}"))
        .with_details(json!({"field": "nodes", "ids": &unknown[..unknown.len().min(HINT_IDS)]}))
        .with_hint(hint))
}

pub(super) fn register(e: &mut Engine) {
    // Only copying the rows out holds the store; ordering and parsing them runs after.
    e.register_unlocked::<Get>(|ctx, p| {
        let pages_only = check_pages_only(&p)?;
        check_reach(&p)?;
        let cut = p.page.is_some() || p.nodes.is_some();
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (design, rows, removed, styles) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            // Its colours and styles, copied out as text and parsed once the store is free.
            let (styles, tracked_from): (String, i64) = conn.prepare_cached("SELECT styles, tracked_from FROM designs WHERE id = ?1").bus()?
                .query_row([design.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?;
            let Some(since) = p.since_rev else {
                let mut stmt = conn.prepare_cached(if pages_only { PAGE_ROWS } else { ROWS }).bus()?;
                let rows = stmt.query_map([design.id], |r| Ok(Raw {
                    id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: true, changed: true, body: Some(r.get(3)?),
                })).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
                return Ok((design, rows, None, styles));
            };
            check_since(&design, tracked_from, since)?;
            let rows = conn.prepare_cached(DELTA_ROWS).bus()?
                .query_map(params![design.id, since, cut], |r| Ok(Raw {
                    id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: known_kind(r, 3)?, changed: r.get(4)?, body: r.get(5)?,
                })).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
            let removed: Vec<String> = conn.prepare_cached(REMOVED).bus()?
                .query_map(params![design.id, since], |r| r.get(0)).bus()?.collect::<rusqlite::Result<_>>().bus()?;
            Ok((design, rows, Some(removed), styles))
        })?;
        if let Some(named) = &p.nodes {
            // A node named that went since a `since_rev` is in `removed`: that is the answer.
            let have: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).chain(removed.iter().flatten().map(String::as_str)).collect();
            let unknown: Vec<&str> = named.iter().map(String::as_str).filter(|id| !have.contains(id)).collect();
            if !unknown.is_empty() {
                let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
                return Err(ctx.read(|conn| unknown_nodes(conn, design.id, &unknown, &ids))?);
            }
        }
        let scope = Scope {
            page: p.page.as_deref(),
            named: p.nodes.as_ref().map(|named| named.iter().map(String::as_str).collect()),
            delta: p.since_rev.is_some(),
        };
        let read = ordered(design.id, rows, &scope)?;
        let indexes = scope.delta.then_some(read.indexes);
        let assets_dir = ctx.engine().store.design_assets_dir().display().to_string();
        let Styles { colors, styles } = serde_json::from_str(&styles)
            .map_err(|why| BusError::internal(format!("design {}: its colours and styles do not read: {why}", design.id)))?;
        // Where each picture it shows came from, read from the files beside them, unlocked.
        let pictures = read.nodes.iter().filter(|n| n.kind == NodeKind::Image).filter_map(|n| n.src.as_deref());
        let credits = crate::pictures::credits_for(&ctx.engine().store.design_assets_dir(), pictures);
        Ok(GetOut { design, nodes: read.nodes, assets_dir, removed, indexes, left: read.left, colors, styles, credits })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(list: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        list.iter().map(|(id, name)| (id.to_string(), name.map(str::to_string))).collect()
    }

    #[test]
    fn a_page_is_found_by_id_then_by_the_name_it_goes_by() {
        let list = pages(&[("p1", None), ("p2", Some("Home")), ("p3", Some("  ")), ("Home", Some("Settings"))]);
        assert_eq!(resolve_page(&list, "p1").unwrap(), "p1");
        assert_eq!(resolve_page(&list, "Home").unwrap(), "Home", "an id before a name");
        assert_eq!(resolve_page(&list, "Settings").unwrap(), "Home");
        assert_eq!(resolve_page(&list, "Page 1").unwrap(), "p1", "no name: its place");
        assert_eq!(resolve_page(&list, "Page 3").unwrap(), "p3", "a blank name is none");
        let missed = resolve_page(&list, "Page 2").unwrap_err();
        assert_eq!((missed.code.as_str(), missed.message.as_str()), ("design.page_not_found", r#"the design has no page "Page 2"; did you mean "Page 1" (p1)?"#));
        assert_eq!(missed.hint.as_deref(), Some(r#"pages: p1 ("Page 1"), p2 ("Home"), p3 ("Page 3"), Home ("Settings"); pass a page's id"#));
        assert_eq!(resolve_page(&list, "p4").unwrap_err().message, r#"the design has no page "p4"; did you mean "p1"?"#);
        let none = resolve_page(&[], "Page 1").unwrap_err();
        assert_eq!((none.message.as_str(), none.hint.as_deref()), (r#"the design has no page "Page 1""#, Some("the design has no pages: add one with design.apply")));
    }

    #[test]
    fn a_page_reads_its_name_as_design_get_does() {
        assert_eq!(named_page("p", r##"{"kind":"page","name":"Home","fill":"#fff"}"##), Some(("p".into(), Some("Home".into()))));
        assert_eq!(named_page("p", r#"{"kind":"page","name":7}"#), Some(("p".into(), None)), "a name this build cannot read is none");
        assert_eq!(named_page("p", r#"{"kind":"page","name":"Home","sparkle":true}"#), Some(("p".into(), Some("Home".into()))), "a newer build's field");
        assert_eq!(named_page("p", "[1]"), None, "a body that is not a node");
    }

    /// `pages_only` reads the pages through `design_nodes_parent`, not by a pass over every row.
    #[test]
    fn the_pages_alone_are_read_through_the_parent_index() {
        let store = crate::Store::open_memory().unwrap();
        let conn = store.lock();
        let plan: Vec<String> = conn.prepare(&format!("EXPLAIN QUERY PLAN {PAGE_ROWS}")).unwrap()
            .query_map([1], |r| r.get::<_, String>(3)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert!(plan.iter().any(|step| step.contains("USING INDEX design_nodes_parent (design_id=? AND parent_id=?)")), "{plan:?}");
    }
}
