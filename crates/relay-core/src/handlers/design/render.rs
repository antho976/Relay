//! `design.render`'s engine side (render-SPEC-v2 §5): the socket door's registrations for the
//! render and its lease, what a render resolves against the store before anything goes to the
//! desktop app ([`render_target`]), and the app's answer, unlocked.

use super::get::{named_page, resolve_page};
use super::get_design;
use crate::design_render::{args, stale, Target};
use crate::engine::{Engine, IntoBus};
use crate::handlers::bus::door_only;
use relay_bus::design::nearest;
use relay_bus::ops::design::RENDER_NODES_MAX;
use relay_bus::error::BusError;
use relay_bus::ops::design::{Render, RenderAnswer, RenderServe};
use relay_bus::registry::Op;
use relay_bus::types::Id;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// Every row of a design: its id, its parent and its place, no body.
const IDS: &str = "SELECT node_id, parent_id, ord FROM design_nodes WHERE design_id = ?1";
/// A design's pages with their bodies, in page order, through `design_nodes_parent` as
/// design.get's `pages_only` reads them: read only for a page asked for by a name no id matched.
const PAGE_BODIES: &str = "SELECT node_id, body FROM design_nodes WHERE design_id = ?1 AND parent_id IS NULL ORDER BY ord, node_id";
/// The rows written after a rev, through `design_nodes_rev`: what a `since_rev` render pictures.
const CHANGED: &str = "SELECT node_id FROM design_nodes WHERE design_id = ?1 AND rev > ?2 ORDER BY node_id";

pub(super) fn register(e: &mut Engine) {
    // Both answered by the socket door (socket.rs), which runs the pipeline's checks through these
    // handlers and takes their `bus.door` as the pass. Locked registrations, as bus.wait's: an
    // unlocked op is routed before the door's branches.
    e.register::<Render>(|_, _| Err(door_only(Render::NAME)));
    e.register::<RenderServe>(|_, _| Err(door_only(RenderServe::NAME)));
    // Unlocked (D149): the answer decodes and checks up to 4 MiB of picture and writes it beside
    // the store, none of which needs the store. The app sends it on its render connection, which
    // carries only unlocked queries, so nothing there waits behind it.
    e.register_unlocked::<RenderAnswer>(|ctx, p| Ok(crate::design_render::answer(ctx.engine(), p)));
}

/// What a render asks of the store: the design, the page, the nodes and the rev it expects. The
/// picture's own options are the door's.
pub(crate) struct Ask {
    pub design_id: Id,
    pub page: Option<String>,
    /// Each once, at least one.
    pub nodes: Option<Vec<String>>,
    pub base_rev: Option<i64>,
    /// Picture what changed after this rev instead of `nodes`.
    pub since_rev: Option<i64>,
}

/// `ask` resolved against the store before anything goes to the desktop app, so a refusal sends
/// nothing. The design is read as design.get reads it (deleted or not); an agent's must be its
/// own project's (`project`); with `base_rev`, it must still be at that rev. The page is the one
/// `page` names by id or by the name it goes by ([`resolve_page`], as design.get), else the page
/// of the first node named, else the design's first page; every node named must be on it.
///
/// One short hold of the store: the design's row and one scan of its ids and parents (at most
/// `MAX_NODES` rows, no body parsed), and the pages' bodies only for a page asked for by name.
pub(crate) fn render_target(conn: &Connection, ask: &Ask, project: Option<Id>) -> Result<Target, BusError> {
    let design = get_design(conn, ask.design_id)?;
    if project.is_some_and(|own| own != design.project_id) {
        return Err(BusError::not_own("project"));
    }
    if let Some(base_rev) = ask.base_rev.filter(|base_rev| *base_rev != design.rev) {
        return Err(stale(design.id, design.rev, base_rev));
    }
    let changed: Option<Vec<String>> = match ask.since_rev {
        None => None,
        Some(_) if ask.nodes.is_some() => {
            return Err(args("give nodes or since_rev, not both: since_rev pictures the nodes that changed after it"));
        }
        Some(since) if since < 0 || since > design.rev => {
            return Err(args(format!("since_rev: {since} is not a rev of design {} (0 to {})", design.id, design.rev))
                .with_hint("pass the prev_rev your last design.apply answered"));
        }
        Some(since) => Some(conn.prepare_cached(CHANGED).bus()?.query_map(rusqlite::params![design.id, since], |r| r.get(0)).bus()?
            .collect::<rusqlite::Result<_>>().bus()?),
    };
    let rows: Vec<(String, Option<String>, f64)> = conn.prepare_cached(IDS).bus()?
        .query_map([design.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?
        .collect::<rusqlite::Result<_>>().bus()?;
    let parents: HashMap<&str, Option<&str>> = rows.iter().map(|(id, parent, _)| (id.as_str(), parent.as_deref())).collect();
    // The page a node is on (a page is on itself); none when the node is not in the design or its
    // chain breaks. A walk, never deeper than the design has rows, so a loop cannot hold it.
    let page_of = |id: &str| -> Option<String> {
        let mut at = id;
        for _ in 0..=rows.len() {
            match parents.get(at)? {
                None => return Some(at.to_string()),
                Some(parent) => at = parent,
            }
        }
        None
    };
    let missing = |id: &str| {
        let hint = match nearest(id, rows.iter().map(|(id, _, _)| id.as_str())) {
            Some(near) => format!("did you mean {near:?}?"),
            None => "design.get lists the ids (pass page to read one page)".to_string(),
        };
        BusError::not_found("design.node_not_found", format!("nodes: design {} has no node {id:?}", design.id)).with_hint(hint)
    };
    // What changed, by the page it is on: the page most of it is on is the one pictured.
    let mut by_page: Vec<(String, usize)> = Vec::new();
    for page in changed.iter().flatten().filter_map(|id| page_of(id)) {
        match by_page.iter_mut().find(|(p, _)| *p == page) {
            Some((_, n)) => *n += 1,
            None => by_page.push((page, 1)),
        }
    }
    let busiest = by_page.iter().max_by_key(|(_, n)| *n).map(|(page, _)| page.clone());
    let first = ask.nodes.as_deref().and_then(<[String]>::first);
    let page = match (ask.page.as_deref().or(busiest.as_deref()), first) {
        (Some(wanted), _) if parents.get(wanted) == Some(&None) => wanted.to_string(),
        (Some(wanted), _) => {
            let bodies: Vec<(String, String)> = conn.prepare_cached(PAGE_BODIES).bus()?
                .query_map([design.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
                .collect::<rusqlite::Result<_>>().bus()?;
            let pages: Vec<(String, Option<String>)> = bodies.iter().filter_map(|(id, body)| named_page(id, body)).collect();
            resolve_page(&pages, wanted)?
        }
        (None, Some(first)) => page_of(first).ok_or_else(|| missing(first))?,
        (None, None) => {
            let mut pages: Vec<&(String, Option<String>, f64)> = rows.iter().filter(|(_, parent, _)| parent.is_none()).collect();
            pages.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
            match pages.first() {
                Some((id, _, _)) => id.clone(),
                None => {
                    return Err(BusError::not_found("design.page_not_found", format!("design {} has no pages", design.id))
                        .with_hint("the design has no pages: add one with design.apply"))
                }
            }
        }
    };
    for id in ask.nodes.iter().flatten() {
        let on = page_of(id).ok_or_else(|| missing(id))?;
        if on != page {
            return Err(match (&ask.page, first) {
                (Some(_), _) => args(format!("nodes: {id:?} is on page {on:?}, not on the page asked for ({page:?})"))
                    .with_hint("name nodes of that page, or leave page out to picture the page they are on"),
                (None, first) => args(format!(
                    "nodes: {id:?} is on page {on:?} and {:?} on page {page:?}; a render pictures one page",
                    first.map_or("", String::as_str)
                ))
                .with_hint("render each page's nodes in a call of its own"),
            });
        }
    }
    let (nodes, note) = match &changed {
        None => (ask.nodes.clone(), None),
        Some(changed) => {
            let on: HashSet<&str> = changed.iter().map(String::as_str).filter(|id| page_of(id).as_deref() == Some(page.as_str())).collect();
            // The outermost: none inside another that changed. A page that changed is the whole.
            let outermost: Vec<String> = changed.iter().map(String::as_str)
                .filter(|id| on.contains(id) && parents.get(id).is_some_and(Option::is_some))
                .filter(|id| !ancestors(&parents, id, rows.len()).any(|up| on.contains(up)))
                .map(str::to_string)
                .collect();
            let whole = on.contains(page.as_str()) || outermost.is_empty() || outermost.len() > RENDER_NODES_MAX;
            let others: Vec<&str> = by_page.iter().filter(|(p, _)| *p != page).map(|(p, _)| p.as_str()).collect();
            let note = (!others.is_empty()).then(|| format!(
                "since_rev {}: page {} changed too; picture it with page {:?} and since_rev",
                ask.since_rev.unwrap_or_default(),
                others.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", "),
                others[0],
            ));
            ((!whole).then_some(outermost), note)
        }
    };
    Ok(Target { design_id: design.id, page, nodes, note })
}

/// The ids above `id`, its parent first, to its page: at most `limit` steps, so a loop in the
/// rows cannot hold the walk.
fn ancestors<'a>(parents: &'a HashMap<&'a str, Option<&'a str>>, id: &'a str, limit: usize) -> impl Iterator<Item = &'a str> + 'a {
    let mut at = parents.get(id).copied().flatten();
    std::iter::from_fn(move || {
        let up = at?;
        at = parents.get(up).copied().flatten();
        Some(up)
    })
    .take(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page by name reads only the pages' bodies, through `design_nodes_parent`, never a pass
    /// over the whole design.
    #[test]
    fn the_pages_named_are_read_through_the_parent_index() {
        let store = crate::Store::open_memory().unwrap();
        let conn = store.lock();
        let plan: Vec<String> = conn.prepare(&format!("EXPLAIN QUERY PLAN {PAGE_BODIES}")).unwrap()
            .query_map([1], |r| r.get::<_, String>(3)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert!(plan.iter().any(|step| step.contains("USING INDEX design_nodes_parent (design_id=? AND parent_id=?)")), "{plan:?}");
        let plan: Vec<String> = conn.prepare(&format!("EXPLAIN QUERY PLAN {IDS}")).unwrap()
            .query_map([1], |r| r.get::<_, String>(3)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert!(plan.iter().all(|step| !step.starts_with("SCAN design_nodes")), "the ids are one design's rows: {plan:?}");
    }
}
