//! Charts (`chart` on a frame): once a list's edits have run, each chart frame whose chart, size or
//! padding an edit changed has its parts drawn again (`relay_client::design::chart::parts`) and
//! written as the frame's `generated` children: a part drawn as before is left alone, one that
//! moved or changed is updated, a new one is added and one no longer drawn is deleted, the parts
//! under the frame's other children in the order they paint. Every write is an edit like any
//! other, so one undo takes back the change and its parts. A chart in an instance is its main's
//! (the component sync, which runs after, carries the parts); a lenient list draws nothing. An
//! `impl Run`, as the follow rule is.

use super::apply::Run;
use super::node_of;
use crate::engine::IntoBus;
use relay_bus::design::Node;
use relay_bus::error::BusError;
use relay_bus::ErrorKind;
use relay_client::design::chart::parts;
use rusqlite::params;
use serde_json::{Map, Value};

/// A frame's children in paint order, with their bodies: its parts are among them.
const CHILDREN: &str = "SELECT node_id, body FROM design_nodes WHERE design_id = ?1 AND parent_id = ?2 ORDER BY ord, node_id";

/// What an update of a chart frame changes that its parts are drawn from.
pub(super) const DRAWN_FROM: &[&str] = &["chart", "w", "h", "width", "height", "padding"];

impl Run<'_> {
    /// The refusal of an edit by hand of `id`, a part a chart drew.
    pub(super) fn chart_part(&self, id: &str) -> BusError {
        self.fail(ErrorKind::Invalid, "design.chart", format!("{id:?} is drawn by its frame's chart"))
            .with_hint("change the chart (its labels, series, colours or options) on the frame, and its parts are drawn again; add your own nodes beside them")
    }

    /// Note chart frame `id` to draw again once the list has run.
    pub(super) fn chart_changed(&mut self, id: &str) {
        if !self.lenient && !self.charts.iter().any(|c| c == id) {
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
