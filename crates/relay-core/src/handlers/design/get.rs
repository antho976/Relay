//! `design.get`: a design's rows copied out under the lock, ordered and parsed after it.

use super::{get_design, node_of};
use crate::engine::{Engine, IntoBus};
use crate::handlers::notes::check_project;
use relay_bus::design::{nearest, Node};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Get, GetIn, GetOut};
use relay_bus::types::Id;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

/// Every row of a design.
const ROWS: &str = "SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1";
/// A design's pages alone (`pages_only`), found through `design_nodes_parent` without a pass
/// over the rest of the design. The `ORDER BY` is what the index already gives, and what makes
/// SQLite take it: without statistics it reckons `IS NULL` matches most rows and scans the
/// design's whole primary key instead (5,000 rows, 1.5 MiB of bodies, under the store lock).
const PAGE_ROWS: &str = "SELECT node_id, parent_id, ord, body FROM design_nodes WHERE design_id = ?1 AND parent_id IS NULL ORDER BY ord, node_id";

/// Pages a `design.page_not_found` hint names before it counts the rest.
const HINT_PAGES: usize = 20;

/// One node row as `design.get` copies it out under the lock; parsed once the lock is gone.
struct Raw {
    id: String,
    parent: Option<String>,
    ord: f64,
    body: String,
}

/// `rows` in `design.get`'s order: the pages by `ord`, then each page's nodes in pre-order,
/// siblings bottom to top. With `page`, that page and what is on it. A node whose parent is
/// gone (none should be) is left out rather than shown nowhere; so is a row this build cannot
/// read (a newer build's kind, a body that is not a node), with what is inside it, rather than
/// failing the whole design (B61).
fn ordered(design: Id, rows: Vec<Raw>, page: Option<&str>) -> Result<Vec<Node>, BusError> {
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
    if let Some(wanted) = page {
        // A page by id costs nothing more; the pages' bodies are read for their names only when
        // no id matched.
        let id = if pages.iter().any(|&i| rows[i].id == wanted) {
            wanted.to_string()
        } else {
            let named: Vec<(String, Option<String>)> = pages.iter().filter_map(|&i| named_page(&rows[i].id, &rows[i].body)).collect();
            resolve_page(&named, wanted)?
        };
        pages.retain(|&i| rows[i].id == id);
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
    let mut left_out = vec![false; rows.len()];
    let mut rows: Vec<Option<Raw>> = rows.into_iter().map(Some).collect();
    let mut nodes = Vec::with_capacity(order.len());
    for (i, up) in order {
        let Some(row) = rows[i].take() else { continue };
        if up.is_some_and(|p| left_out[p]) {
            left_out[i] = true;
            continue;
        }
        match node_of(row.id, row.parent, &row.body) {
            Ok(node) => nodes.push(node),
            Err(error) => {
                left_out[i] = true;
                tracing::warn!(design, "design.get: {}; left out with what is inside it", error.message);
            }
        }
    }
    Ok(nodes)
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

pub(super) fn register(e: &mut Engine) {
    // Only copying the rows out holds the store; ordering and parsing them runs after.
    e.register_unlocked::<Get>(|ctx, p| {
        let pages_only = check_pages_only(&p)?;
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (design, rows) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(if pages_only { PAGE_ROWS } else { ROWS }).bus()?;
            let rows = stmt.query_map([design.id], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, body: r.get(3)? })).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok((design, rows))
        })?;
        let nodes = ordered(design.id, rows, p.page.as_deref())?;
        let assets_dir = ctx.engine().store.design_assets_dir().display().to_string();
        Ok(GetOut { design, nodes, assets_dir, removed: None, colors: BTreeMap::new(), styles: BTreeMap::new(), credits: BTreeMap::new() })
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
