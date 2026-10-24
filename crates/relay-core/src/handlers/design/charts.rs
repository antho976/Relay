//! Charts (`chart` on a frame): once a list's edits have run, each chart frame whose chart, size or
//! padding an edit changed has its parts drawn again (`relay_client::design::chart::parts`) and
//! written as the frame's `generated` children: a part drawn as before is left alone, one that
//! moved or changed is updated, a new one is added and one no longer drawn is deleted, the parts
//! under the frame's other children in the order they paint. Every write is an edit like any
//! other, so one undo takes back the change and its parts. A chart in an instance is its main's
//! (the component sync, which runs after, carries the parts); the person's lenient list (an undo,
//! a redo) draws nothing, its parts being in it (`Run::derives`). An
//! `impl Run`, as the follow rule is.
//!
//! A chart frame that fills its laid-out parent (`w_sizing` or `h_sizing` `fill`) is drawn at the
//! size the layout gives it: once the follow rule has run, each of a page's children the list
//! reached that holds one (an artboard, most often) is laid out by the canvas's own code (words
//! measured roughly, `Estimate`), and a chart whose laid-out size is not its stored one has that
//! size written to `w`/`h`, which draws it again. A page lays nothing out, so where a chart is
//! drawn depends on nothing outside its page child, and only that is read. Such charts are listed
//! on the design (`designs.fill_charts`), so a list on a design without one lays nothing out.

use super::apply::Run;
use super::get::{ordered, Raw, Scope};
use super::node_of;
use crate::engine::IntoBus;
use relay_bus::design::{Node, Sizing, MAX_DEPTH};
use relay_bus::error::BusError;
use relay_bus::ErrorKind;
use relay_client::design::chart::parts;
use relay_client::design::layout::{place, Estimate};
use relay_client::design::scene::Scene;
use rusqlite::params;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A frame's children in paint order, with their bodies: its parts are among them.
const CHILDREN: &str = "SELECT node_id, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id";

/// What an update of a chart frame changes that its parts are drawn from.
pub(super) const DRAWN_FROM: &[&str] = &["chart", "w", "h", "width", "height", "padding"];

/// Node `?2` and every node inside it as far as `?3` levels down, through the parent index and
/// then the primary key (`CROSS JOIN` keeps `down` the outer loop of both, as `HEIGHT` in apply
/// does: as a plain join the planner reads every row of the design for the bodies).
const SUBTREE: &str = "WITH RECURSIVE down(node_id, n) AS (
        SELECT ?2, 0
        UNION ALL
        SELECT d.node_id, down.n + 1 FROM down CROSS JOIN design_nodes d ON d.design_id = ?1 AND d.parent_id = down.node_id
        WHERE down.n < ?3)
    SELECT n.node_id, n.parent_id, n.ord, n.body FROM down CROSS JOIN design_nodes n ON n.design_id = ?1 AND n.node_id = down.node_id";

/// Whether `n` is a chart frame the layout sizes: its width or height fills.
pub(super) fn fills(n: &Node) -> bool {
    n.chart.is_some() && (n.w_rule() == Sizing::Fill || n.h_rule() == Sizing::Fill)
}

impl Run<'_> {
    /// The refusal of an edit by hand of `id`, a part a chart drew.
    pub(super) fn chart_part(&self, id: &str) -> BusError {
        self.fail(ErrorKind::Invalid, "design.chart", format!("{id:?} is drawn by its frame's chart"))
            .with_hint("change the chart (its labels, series, colours or options) on the frame, and its parts are drawn again; add your own nodes beside them")
    }

    /// Whether a `generated` node under `parent` is a chart's part: its frame holds a chart. One
    /// left anywhere else (written by hand before that was refused) is edited as any node is.
    pub(super) fn drawn_by_chart(&mut self, parent: Option<&str>) -> Result<bool, BusError> {
        let Some(parent) = parent else { return Ok(false) };
        Ok(self.row(parent)?.is_some_and(|row| row.body.contains("\"chart\":")))
    }

    /// Note chart frame `id` to draw again once the list has run.
    pub(super) fn chart_changed(&mut self, id: &str) {
        if self.derives() && !self.charts.iter().any(|c| c == id) {
            self.charts.push(id.to_string());
        }
    }

    /// Draw each chart the list changed again (see the module).
    pub(super) fn draw_charts(&mut self) -> Result<(), BusError> {
        let frames = std::mem::take(&mut self.charts);
        if frames.is_empty() {
            return Ok(());
        }
        let was = self.syncing;
        self.syncing = true;
        let out = frames.iter().try_for_each(|frame| self.draw_chart(frame));
        self.syncing = was;
        out
    }

    /// Note node `n`, as an edit wrote it, on the design's list of charts that fill.
    pub(super) fn note_fill(&mut self, n: &Node) {
        if fills(n) && !self.fill_charts.contains(&n.id) {
            self.fill_charts.push(n.id.clone());
            self.fill_charts_changed = true;
        }
    }

    /// Give each chart that fills, on a page the list reached, its laid-out size (see the module),
    /// and keep the design's list of them: one gone or no longer filling leaves it.
    pub(super) fn fit_charts(&mut self) -> Result<(), BusError> {
        if self.derives() && !self.fill_charts.is_empty() {
            self.fit()?;
        }
        if std::mem::take(&mut self.fill_charts_changed) {
            let list = serde_json::to_string(&self.fill_charts).map_err(|why| BusError::internal(why.to_string()))?;
            self.tx.prepare_cached("UPDATE designs SET fill_charts = ?2 WHERE id = ?1").bus()?
                .execute(params![self.design, list]).bus()?;
        }
        Ok(())
    }

    fn fit(&mut self) -> Result<(), BusError> {
        let mut reached: Vec<(String, String)> = Vec::new();
        for id in self.touched.clone().iter().chain(std::mem::take(&mut self.fit_reached).iter()) {
            if let Some(top) = self.top(id)? {
                if !reached.contains(&top) {
                    reached.push(top);
                }
            }
        }
        // Each chart that still fills, by the page child it is in; one in a page child the list
        // did not reach keeps the size it was given.
        let mut by_top: BTreeMap<(String, String), Vec<Node>> = BTreeMap::new();
        for id in self.fill_charts.clone() {
            // Where it is comes from the parent index; its body is read only where it is reached.
            let top = self.top(&id)?;
            // A style or colour edit touches no node but can change any layout that names it
            // (a label's font size beside the chart): every chart that fills is laid out again.
            if !self.styles_changed && top.as_ref().is_some_and(|top| !reached.contains(top)) {
                continue;
            }
            let chart = self.stored(&id)?.filter(|n| fills(n) && n.instance_of.is_none());
            match (top, chart) {
                (Some(top), Some(chart)) => by_top.entry(top).or_default().push(chart),
                _ => {
                    self.fill_charts.retain(|c| *c != id);
                    self.fill_charts_changed = true;
                }
            }
        }
        for ((page, top), charts) in by_top {
            let Some(page_row) = self.row(&page)? else { continue };
            let mut rows = vec![Raw { id: page.clone(), parent: None, ord: page_row.ord, known: true, changed: true, body: Some(page_row.body) }];
            rows.extend(self.tx.prepare_cached(SUBTREE).bus()?
                .query_map(params![self.design, top, MAX_DEPTH as i64], |r| {
                    Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: true, changed: true, body: Some(r.get(3)?) })
                }).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?);
            let read = ordered(self.design, rows, &Scope { page: None, named: None, delta: false })?;
            let scene = Scene::new(read.nodes).with_styles(self.styles()?.clone());
            let placed = place(&scene, &page, &Estimate);
            for chart in charts {
                let Some(rect) = placed.rect(&chart.id) else { continue };
                let mut set = Map::new();
                if chart.w_rule() == Sizing::Fill && (rect.w - chart.w).abs() >= 0.5 {
                    set.insert("w".into(), Value::from(half(rect.w)));
                }
                if chart.h_rule() == Sizing::Fill && (rect.h - chart.h).abs() >= 0.5 {
                    set.insert("h".into(), Value::from(half(rect.h)));
                }
                if !set.is_empty() {
                    self.label = format!("fit chart {:?}", chart.id);
                    self.update(chart.id.clone(), set)?;
                }
            }
        }
        Ok(())
    }

    /// Node `id`'s page and the page's child it is in (itself, for one): none for a page, or for
    /// a node not in the design.
    fn top(&mut self, id: &str) -> Result<Option<(String, String)>, BusError> {
        let Some((depth, page)) = self.place(id)? else { return Ok(None) };
        if depth == 0 {
            return Ok(None);
        }
        let top = self.chain(id).nth(depth - 1).map(str::to_string);
        Ok(top.map(|top| (page, top)))
    }

    fn draw_chart(&mut self, frame: &str) -> Result<(), BusError> {
        let Some(node) = self.stored(frame)? else { return Ok(()) };
        if node.instance_of.is_some() {
            return Ok(());
        }
        self.label = format!("chart {frame:?}");
        // Each part as an add stores it (B49, B3), so one drawn as before compares equal.
        let want: Vec<Node> = parts(&node).into_iter().map(|mut part| {
            part.fill_default_size();
            part.contain_points();
            part
        }).collect();
        let children: Vec<(String, String)> = self.tx.prepare_cached(CHILDREN).bus()?
            .query_map(params![self.design, frame], |r| Ok((r.get(0)?, r.get(1)?))).bus()?
            .collect::<rusqlite::Result<_>>().bus()?;
        let mut had: Vec<Node> = Vec::new();
        for (id, body) in children {
            if body.contains("\"generated\":true") {
                if let Ok(part) = node_of(id, Some(frame.to_string()), &body) {
                    had.push(part);
                }
            }
        }
        // What is no longer drawn goes first, so a new part may take a freed id.
        for old in had.iter().filter(|old| !want.iter().any(|w| w.id == old.id)) {
            self.delete(old.id.clone())?;
        }
        for part in &want {
            match had.iter().find(|old| old.id == part.id) {
                Some(old) => {
                    let (now, then) = (part.to_map(), old.to_map());
                    let mut set = Map::new();
                    for key in now.keys().chain(then.keys()).filter(|k| !matches!(k.as_str(), "id" | "parent" | "kind")) {
                        if now.get(key) != then.get(key) {
                            set.insert(key.clone(), now.get(key).cloned().unwrap_or(Value::Null));
                        }
                    }
                    if !set.is_empty() {
                        self.update(part.id.clone(), set)?;
                    }
                }
                None => {
                    self.add(part.clone(), None)?;
                }
            }
            // Placed by the chart: the follow rule leaves it where it was drawn.
            self.set_axes(&part.id, true, true);
        }
        // The parts in the order they paint, under the frame's other children.
        let order: Vec<String> = self.tx.prepare_cached(CHILDREN).bus()?
            .query_map(params![self.design, frame], |r| r.get::<_, String>(0)).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        let wanted: Vec<String> = want.iter().map(|p| p.id.clone()).collect();
        if order.get(..wanted.len()) != Some(&wanted[..]) {
            for (k, id) in wanted.iter().enumerate() {
                self.move_(id.clone(), Some(frame.to_string()), Some(k as u32), false)?;
            }
        }
        Ok(())
    }
}

/// `v` to the half pixel, as a chart's parts are placed.
fn half(v: f64) -> f64 {
    (v.max(0.0) * 2.0).round() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page child's nodes are found through the parent index and their bodies by key, never by
    /// a pass over every row of the design: as a plain join, an edit beside a chart that fills
    /// cost 6.1 M instructions at 1,721 nodes and 12.3 M at 4,903, and 2.8 M at either this way.
    #[test]
    fn the_subtree_is_read_by_index() {
        let store = crate::Store::open_memory().unwrap();
        let conn = store.lock();
        let plan: Vec<String> = conn.prepare(&format!("EXPLAIN QUERY PLAN {SUBTREE}")).unwrap()
            .query_map(params![1, "a", 70], |r| r.get::<_, String>(3)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert!(plan.iter().any(|step| step.contains("INDEX design_nodes_parent (design_id=? AND parent_id=?)")), "{plan:?}");
        assert!(plan.iter().any(|step| step.contains("PRIMARY KEY (design_id=? AND node_id=?)")), "{plan:?}");
        assert!(plan.iter().all(|step| !step.starts_with("SCAN") || step == "SCAN down" || step == "SCAN CONSTANT ROW"), "{plan:?}");
    }
}
