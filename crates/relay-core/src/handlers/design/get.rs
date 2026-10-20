//! `design.get`: a design's rows copied out under the lock, ordered and parsed after it.

use super::{get_design, node_of};
use crate::engine::{Engine, IntoBus};
use crate::handlers::notes::check_project;
use relay_bus::design::Node;
use relay_bus::error::BusError;
use relay_bus::ops::design::{Get, GetOut};
use std::collections::HashMap;

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

pub(super) fn register(e: &mut Engine) {
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
}
