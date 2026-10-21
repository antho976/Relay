//! `design.find` and `design.fonts` (MODEL-SPEC §3.3; feature-agent-api §3.6-§3.7): a design's
//! nodes found by kind, name, words, colour or font as drawn, and the font families the desktop
//! app draws. Both unlocked queries: `find` holds the store only to copy the rows out, `fonts`
//! not at all.

use super::get::{ordered, Raw, Scope, ROWS};
use super::get_design;
use crate::engine::{Engine, IntoBus};
use crate::handlers::notes::check_project;
use relay_bus::design::{parse_color, Node, NodeKind, Styles, Where};
use relay_bus::error::BusError;
use relay_bus::types::Id;
use relay_bus::ops::design::{Find, FindIn, FindOut, Fonts, FontsOut, TextMatch};
use std::collections::HashSet;

/// Most nodes `with_nodes` answers, and most bytes of them.
const FOUND_NODES: usize = 200;
const FOUND_BYTES: usize = 512 << 10;

/// The families every Relay draws, whatever the desktop reported: what design.fonts lists when
/// no app has said.
pub(super) const ALWAYS: [&str; 3] = ["Geist", "Geist Mono", "Sora"];

/// A colour a find asks for: its RGBA, and whether its alpha counts (`#RRGGBBAA` only).
struct Wanted {
    rgba: [u8; 4],
    alpha: bool,
}

impl Wanted {
    fn read(field: &str, text: &Option<String>) -> Result<Option<Wanted>, BusError> {
        let Some(text) = text else { return Ok(None) };
        let digits = text.trim_start_matches('#').len();
        match parse_color(text).filter(|_| text.starts_with('#')) {
            Some(rgba) => Ok(Some(Wanted { rgba, alpha: digits == 8 })),
            None => Err(BusError::invalid("design.find", format!("{field}: {text:?} is not #RGB, #RRGGBB or #RRGGBBAA"))
                .with_hint("give the colour itself; a $name colour is found by the value it names")),
        }
    }

    /// Whether colour `text` (as drawn: a `$name` already resolved) is this one.
    fn is(&self, text: Option<&str>) -> bool {
        text.and_then(parse_color).is_some_and(|got| if self.alpha { got == self.rgba } else { got[..3] == self.rgba[..3] })
    }
}

/// Every colour node `n` draws with, as drawn: fill, stroke, text colour, each shadow, each stop
/// of its gradients, and each run's colour.
fn colors(n: &Node) -> Vec<&str> {
    let mut all: Vec<&str> = [&n.fill, &n.stroke, &n.color].into_iter().filter_map(|c| c.as_deref()).collect();
    all.extend(n.shadow.iter().chain(n.shadows.iter().flatten()).map(|s| s.color.as_str()));
    all.extend([&n.gradient, &n.stroke_gradient].into_iter().flatten().flat_map(|g| g.stops.iter().map(|s| s.color.as_str())));
    all.extend(n.runs.iter().flatten().filter_map(|r| r.color.as_deref()));
    all
}

/// Every font family node `n` draws in: its own and each run's.
pub(super) fn families(n: &Node) -> impl Iterator<Item = &str> {
    n.font.as_deref().into_iter().chain(n.runs.iter().flatten().filter_map(|r| r.font.as_deref()))
}

fn matches(how: TextMatch, hay: Option<&str>, needle: &str) -> bool {
    hay.is_some_and(|hay| how.is(&hay.to_lowercase(), needle))
}

/// What a design.find, or an edit's `where`, asks for: each colour read and each word lowercased,
/// so a query is checked once before any row is read.
pub(super) struct Query {
    page: Option<String>,
    inside: Option<String>,
    kinds: Option<HashSet<NodeKind>>,
    name: Option<String>,
    text: Option<String>,
    font: Option<String>,
    how: TextMatch,
    fill: Option<Wanted>,
    stroke: Option<Wanted>,
    color: Option<Wanted>,
    any: Option<Wanted>,
}

impl Query {
    pub(super) fn new(w: &Where) -> Result<Query, BusError> {
        Ok(Query {
            page: w.page.clone(),
            inside: w.inside.clone(),
            kinds: w.kind.as_ref().map(|k| k.iter().copied().collect()),
            name: w.name.as_deref().map(str::to_lowercase),
            text: w.text.as_deref().map(str::to_lowercase),
            font: w.font.as_deref().map(str::to_lowercase),
            how: w.match_.unwrap_or_default(),
            fill: Wanted::read("fill", &w.fill)?,
            stroke: Wanted::read("stroke", &w.stroke)?,
            color: Wanted::read("color", &w.color)?,
            any: Wanted::read("any_color", &w.any_color)?,
        })
    }

    fn filtered(&self) -> bool {
        self.kinds.is_some() || self.name.is_some() || self.text.is_some() || self.font.is_some()
            || self.fill.is_some() || self.stroke.is_some() || self.color.is_some() || self.any.is_some()
    }

    /// The rows the query reads: the design's, copied out with its named colours and styles.
    pub(super) fn rows(conn: &rusqlite::Connection, design: Id) -> Result<(Vec<Raw>, String), BusError> {
        let rows = conn.prepare_cached(ROWS).bus()?
            .query_map([design], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: true, changed: true, body: Some(r.get(3)?) })).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        let styles: String = conn.prepare_cached("SELECT styles FROM designs WHERE id = ?1").bus()?.query_row([design], |r| r.get(0)).bus()?;
        Ok((rows, styles))
    }

    /// The nodes of `rows` the query finds, in design.get's order: with no filter, every node but
    /// the pages (on its page, inside its node). An `inside` not in the design is refused.
    pub(super) fn run(&self, design: Id, rows: Vec<Raw>, styles: &str) -> Result<Vec<Node>, BusError> {
        if let Some(inside) = self.inside.as_deref().filter(|inside| !rows.iter().any(|r| r.id == *inside)) {
            return Err(BusError::not_found("design.node_not_found", format!("inside: {inside:?} is not in the design"))
                .with_hint("design.get lists the ids (pass page to read one page)"));
        }
        let styles: Styles = serde_json::from_str(styles)
            .map_err(|why| BusError::internal(format!("design {design}: its colours and styles do not read: {why}")))?;
        let scope = Scope { page: self.page.as_deref(), named: self.inside.as_deref().map(|inside| HashSet::from([inside])), delta: false };
        let read = ordered(design, rows, &scope)?;
        let filtered = self.filtered();
        Ok(read.nodes.into_iter().filter(|stored| {
            if Some(stored.id.as_str()) == self.inside.as_deref() || (!filtered && stored.kind == NodeKind::Page) {
                return false;
            }
            if self.kinds.as_ref().is_some_and(|kinds| !kinds.contains(&stored.kind)) {
                return false;
            }
            let n = styles.resolve(stored);
            let how = self.how;
            self.name.as_deref().is_none_or(|w| matches(how, n.name.as_deref(), w))
                && self.text.as_deref().is_none_or(|w| matches(how, n.text.as_deref(), w))
                && self.font.as_deref().is_none_or(|w| families(&n).any(|f| f.to_lowercase() == w))
                && self.fill.as_ref().is_none_or(|c| c.is(n.fill.as_deref()))
                && self.stroke.as_ref().is_none_or(|c| c.is(n.stroke.as_deref()))
                && self.color.as_ref().is_none_or(|c| c.is(n.color.as_deref()))
                && self.any.as_ref().is_none_or(|c| colors(&n).into_iter().any(|got| c.is(Some(got))))
        }).collect())
    }
}

pub(super) fn register(e: &mut Engine) {
    // Unlocked (D149): only copying the rows out holds the store; resolving and matching run after.
    e.register_unlocked::<Find>(|ctx, p: FindIn| {
        let query = Query::new(&Where {
            page: p.page.clone(),
            inside: p.inside.clone(),
            kind: p.kind.clone(),
            name: p.name.clone(),
            text: p.text.clone(),
            match_: p.match_,
            fill: p.fill.clone(),
            stroke: p.stroke.clone(),
            color: p.color.clone(),
            any_color: p.any_color.clone(),
            font: p.font.clone(),
        })?;
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (design, rows, styles) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let (rows, styles) = Query::rows(conn, design.id)?;
            Ok((design, rows, styles))
        })?;
        let found = query.run(design.id, rows, &styles)?;
        let ids: Vec<String> = found.iter().map(|n| n.id.clone()).collect();
        let (nodes, cut) = match p.with_nodes {
            Some(true) => {
                let (mut kept, mut bytes) = (Vec::new(), 0);
                for n in &found {
                    let size = serde_json::to_vec(&n.to_map()).map_or(0, |b| b.len());
                    if kept.len() == FOUND_NODES || bytes + size > FOUND_BYTES {
                        break;
                    }
                    bytes += size;
                    kept.push(n.clone());
                }
                let cut = kept.len() < found.len();
                (Some(kept), cut)
            }
            _ => (None, false),
        };
        Ok(FindOut { ids, nodes, cut })
    });
    // No store: the families of the newest connected desktop app that reported any.
    e.register_unlocked::<Fonts>(|ctx, p| {
        let wanted = p.name.as_deref().map(str::to_lowercase);
        let named = |f: &str| wanted.as_deref().is_none_or(|w| f.to_lowercase().contains(w));
        Ok(match ctx.engine().renders.fonts() {
            Some(families) => FontsOut { families: families.iter().filter(|f| named(f)).cloned().collect(), complete: true },
            None => FontsOut { families: ALWAYS.iter().filter(|f| named(f)).map(|f| f.to_string()).collect(), complete: false },
        })
    });
}
