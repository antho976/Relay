//! `design.find` and `design.fonts` (MODEL-SPEC §3.3; feature-agent-api §3.6-§3.7): a design's
//! nodes found by kind, name, words, colour or font as drawn, and the font families the desktop
//! app draws. Both unlocked queries: `find` holds the store only to copy the rows out, `fonts`
//! not at all.

use super::get::{ordered, Raw, Scope, ROWS};
use super::get_design;
use crate::engine::{Engine, IntoBus};
use crate::handlers::notes::check_project;
use relay_bus::design::{parse_color, Node, NodeKind, Styles};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Find, FindIn, FindOut, Fonts, FontsOut};
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

fn contains(hay: Option<&str>, needle: &str) -> bool {
    hay.is_some_and(|hay| hay.to_lowercase().contains(needle))
}

pub(super) fn register(e: &mut Engine) {
    // Unlocked (D149): only copying the rows out holds the store; resolving and matching run after.
    e.register_unlocked::<Find>(|ctx, p: FindIn| {
        let fill = Wanted::read("fill", &p.fill)?;
        let stroke = Wanted::read("stroke", &p.stroke)?;
        let color = Wanted::read("color", &p.color)?;
        let any = Wanted::read("any_color", &p.any_color)?;
        let (name, text, font) = (p.name.as_deref().map(str::to_lowercase), p.text.as_deref().map(str::to_lowercase), p.font.as_deref().map(str::to_lowercase));
        let kinds: Option<HashSet<NodeKind>> = p.kind.as_ref().map(|k| k.iter().copied().collect());
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let (design, rows, styles) = ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            let rows = conn.prepare_cached(ROWS).bus()?
                .query_map([design.id], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: true, changed: true, body: Some(r.get(3)?) })).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            let styles: String = conn.prepare_cached("SELECT styles FROM designs WHERE id = ?1").bus()?.query_row([design.id], |r| r.get(0)).bus()?;
            Ok((design, rows, styles))
        })?;
        if let Some(inside) = p.inside.as_deref().filter(|inside| !rows.iter().any(|r| r.id == *inside)) {
            return Err(BusError::not_found("design.node_not_found", format!("inside: {inside:?} is not in the design"))
                .with_hint("design.get lists the ids (pass page to read one page)"));
        }
        let styles: Styles = serde_json::from_str(&styles)
            .map_err(|why| BusError::internal(format!("design {}: its colours and styles do not read: {why}", design.id)))?;
        let scope = Scope { page: p.page.as_deref(), named: p.inside.as_deref().map(|inside| HashSet::from([inside])), delta: false };
        let read = ordered(design.id, rows, &scope)?;
        let filtered = kinds.is_some() || name.is_some() || text.is_some() || font.is_some() || fill.is_some() || stroke.is_some() || color.is_some() || any.is_some();
        let found: Vec<&Node> = read.nodes.iter().filter(|stored| {
            if Some(stored.id.as_str()) == p.inside.as_deref() || (!filtered && stored.kind == NodeKind::Page) {
                return false;
            }
            if kinds.as_ref().is_some_and(|kinds| !kinds.contains(&stored.kind)) {
                return false;
            }
            let n = styles.resolve(stored);
            name.as_deref().is_none_or(|w| contains(n.name.as_deref(), w))
                && text.as_deref().is_none_or(|w| contains(n.text.as_deref(), w))
                && font.as_deref().is_none_or(|w| families(&n).any(|f| f.to_lowercase() == w))
                && fill.as_ref().is_none_or(|c| c.is(n.fill.as_deref()))
                && stroke.as_ref().is_none_or(|c| c.is(n.stroke.as_deref()))
                && color.as_ref().is_none_or(|c| c.is(n.color.as_deref()))
                && any.as_ref().is_none_or(|c| colors(&n).into_iter().any(|got| c.is(Some(got))))
        }).collect();
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
                    kept.push((*n).clone());
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
