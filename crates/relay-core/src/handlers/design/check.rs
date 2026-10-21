//! design.apply's `contrast`: what an agent's change did to the contrast of the words it
//! reaches, before it renders. Darkening a named colour can fix five light screens and break five
//! labels on a dark one; this says which, in the reply, as design.render's `contrast` issues.
//!
//! Worked out once the change has committed, with the store lock released (`Ctx::finish_reply`):
//! one short read of the design as it stands, laid out by the canvas's own code
//! (`relay_client::design`) with words measured roughly ([`Estimate`]); the design before the
//! change is that one with the list's undo run on it. A flag is new when the same words on the
//! same ground were not flagged before.

use super::get::{ordered, Raw, Scope, ROWS};
use crate::engine::{Engine, IntoBus};
use relay_bus::design::{NodeOp, Styles};
use relay_bus::error::BusError;
use relay_bus::ops::design::RenderIssue;
use relay_bus::types::Id;
use relay_client::design::boxes::contrast_issues;
use relay_client::design::layout::{place, Estimate};
use relay_client::design::scene::Scene;
use std::collections::{BTreeSet, HashSet};

/// Most flags a reply lists; a warning counts the rest.
pub(super) const CONTRAST_LISTED: usize = 20;

/// What an applied list changed, for [`new_contrast`].
pub(super) struct Applied {
    pub(super) design: Id,
    /// The design's rev once the list applied: a later one means another change came in between.
    pub(super) rev: i64,
    /// The edits that undo the list, in the order to run them.
    pub(super) inverse: Vec<NodeOp>,
    /// Every node an edit added, changed, moved or deleted.
    pub(super) touched: Vec<String>,
    /// A colour or style edit ran: any page may draw differently.
    pub(super) restyled: bool,
}

/// What [`new_contrast`] found: the new flags (at most [`CONTRAST_LISTED`]) and the warning to
/// add, if any.
pub(super) struct Found {
    pub(super) contrast: Vec<RenderIssue>,
    pub(super) warning: Option<String>,
}

/// The contrast flags the list made, on each page it reached.
pub(super) fn new_contrast(engine: &Engine, applied: Applied) -> Result<Found, BusError> {
    let (rev, rows, styles) = engine.read_store("design.apply", |conn| {
        let (rev, styles): (i64, String) = conn.prepare_cached("SELECT rev, styles FROM designs WHERE id = ?1").bus()?
            .query_row([applied.design], |r| Ok((r.get(0)?, r.get(1)?))).bus()?;
        if rev != applied.rev {
            return Ok((rev, Vec::new(), styles));
        }
        let rows = conn.prepare_cached(ROWS).bus()?
            .query_map([applied.design], |r| Ok(Raw { id: r.get(0)?, parent: r.get(1)?, ord: r.get(2)?, known: true, changed: true, body: Some(r.get(3)?) })).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        Ok((rev, rows, styles))
    })?;
    if rev != applied.rev {
        let warning = format!("the design changed again (rev {rev}) before this change's contrast was checked; design.render checks it");
        return Ok(Found { contrast: Vec::new(), warning: Some(warning) });
    }
    let styles: Styles = serde_json::from_str(&styles)
        .map_err(|why| BusError::internal(format!("design {}: its colours and styles do not read: {why}", applied.design)))?;
    let read = ordered(applied.design, rows, &Scope { page: None, named: None, delta: false })?;
    let after = Scene::new(read.nodes).with_styles(styles);
    let mut before = after.clone();
    for op in &applied.inverse {
        // An undo replays leniently; an edit it cannot run leaves that part as it is now.
        let _ = before.apply(op);
    }
    let pages: BTreeSet<String> = if applied.restyled {
        after.pages().iter().chain(before.pages()).cloned().collect()
    } else {
        applied.touched.iter().filter_map(|id| after.page_of(id).or_else(|| before.page_of(id))).map(str::to_string).collect()
    };
    let mut found = Vec::new();
    for page in &pages {
        let flags = |scene: &Scene| -> Vec<RenderIssue> {
            if scene.node(page).is_none() {
                return Vec::new();
            }
            contrast_issues(scene, &place(scene, page, &Estimate), &Estimate, page)
        };
        let old: HashSet<Vec<String>> = flags(&before).into_iter().map(|i| i.ids).collect();
        found.extend(flags(&after).into_iter().filter(|i| !old.contains(&i.ids)));
    }
    let more = found.len().saturating_sub(CONTRAST_LISTED);
    found.truncate(CONTRAST_LISTED);
    let warning = (more > 0).then(|| format!("{more} more contrast flags; design.render lists every issue"));
    Ok(Found { contrast: found, warning })
}
