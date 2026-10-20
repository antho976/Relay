//! `design.*` (BUS.md §10.18): a project's design canvases. A design is a `designs` row and its
//! nodes, one `design_nodes` row each: the node's JSON beside its parent and its paint order
//! (`ord`, ascending is bottom to top), so a move rewrites two columns and never the node.
//!
//! Every change to the nodes is a `design.apply`: edits run in order against the request's
//! transaction, so a later edit sees an earlier one and a refusal anywhere rolls back them all.
//! It answers with the edits that undo it, which the canvas keeps for its undo and `audit.undo`
//! replays leniently.
//!
//! `get` reads a design's nodes in order, `apply` runs a `design.apply` list, and `assets` stores
//! the pictures image nodes name; this file holds the design rows and the other ops.

mod apply;
mod assets;
mod get;

use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{actor_name, assert_actor_project, check_project};
use relay_bus::design::{plain_label, Node, NodeKind, MAX_NAME};
use relay_bus::error::BusError;
use relay_bus::ops::design::{Create, Delete, Duplicate, GetOut, List, ListOut, Purge, Restore, Update};
use relay_bus::types::{Design, Id};
use relay_bus::Empty;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;

const DEFAULT_TITLE: &str = "Untitled design";

macro_rules! design_select {
    ($rest:literal) => {
        concat!(
            "SELECT id, project_id, session, title, rev, pages, nodes, created_by, updated_by, created_at, updated_at, deleted_at
             FROM designs ",
            $rest
        )
    };
}

fn design_row(row: &Row) -> rusqlite::Result<Design> {
    Ok(Design {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session: row.get(2)?,
        title: row.get(3)?,
        rev: row.get(4)?,
        pages: row.get(5)?,
        nodes: row.get(6)?,
        created_by: row.get(7)?,
        updated_by: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        deleted_at: row.get(11)?,
    })
}

/// Design `id`, deleted or not.
fn get_design(conn: &Connection, id: Id) -> Result<Design, BusError> {
    conn.prepare_cached(design_select!("WHERE id = ?1")).bus()?
        .query_row([id], design_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("design.not_found", format!("no design {id}")))
}

/// Design `id` if it is not deleted: what every change to one needs.
fn live_design(conn: &Connection, id: Id) -> Result<Design, BusError> {
    let design = get_design(conn, id)?;
    if design.deleted_at.is_some() {
        return Err(BusError::conflict("design.deleted", format!("design {id} is deleted"))
            .with_hint("design.restore brings it back"));
    }
    Ok(design)
}

fn emit_design(ctx: &mut Ctx, design: &Design) -> Result<(), BusError> {
    ctx.set_project(design.project_id);
    ctx.emit("design.changed", serde_json::to_value(design).bus()?);
    Ok(())
}

/// `raw` trimmed, or `default` when there is none; 1 to [`MAX_NAME`] characters either way,
/// and none a control character (B43: a NUL would cut it short in GTK, or panic a debug build).
fn title(raw: Option<&str>, default: &str) -> Result<String, BusError> {
    let title = raw.map_or(default, str::trim);
    let n = title.chars().count();
    if n == 0 || n > MAX_NAME {
        return Err(BusError::invalid("design.title", format!("a design title is 1 to {MAX_NAME} characters")));
    }
    if !plain_label(title) {
        return Err(BusError::invalid("design.title", "a design title has no control characters"));
    }
    Ok(title.to_string())
}

/// "<title> copy", the original cut short enough that the copy's title is still a title.
fn copy_title(original: &str) -> String {
    let kept: String = original.chars().take(MAX_NAME - " copy".len()).collect();
    format!("{} copy", kept.trim_end())
}

/// The terminal a design is tied to: an agent's own (layer 2 refuses another's name before the
/// handler runs), the person's any session the project has had.
fn check_session(ctx: &Ctx, project_id: Id, session: &str) -> Result<(), BusError> {
    if let Some(own) = ctx.actor.session_name() {
        return if own == session { Ok(()) } else { Err(BusError::not_own("session")) };
    }
    let known: bool = ctx.tx()
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM sessions WHERE project_id = ?1 AND name = ?2)").bus()?
        .query_row(params![project_id, session], |r| r.get(0)).bus()?;
    if !known {
        return Err(BusError::invalid("design.session", format!("project {project_id} has no session {session:?}"))
            .with_hint("name one of the project's sessions (session.list), or none"));
    }
    Ok(())
}

/// The kind a row's `kind` column names; `None` for one this build does not know (a newer
/// build's), whose row every scan leaves alone and every edit refuses to touch.
fn kind_of(text: &str) -> Option<NodeKind> {
    NodeKind::from_name(text)
}

/// A node's JSON as stored: everything but its id and parent, which are columns, with what this
/// build could not read of it (`extra`) written back as it came.
fn body_of(node: &mut Node) -> Result<String, BusError> {
    let (id, parent) = (std::mem::take(&mut node.id), node.parent.take());
    let body = if node.extra.is_empty() { serde_json::to_string(node) } else { serde_json::to_string(&node.to_map()) }.bus();
    (node.id, node.parent) = (id, parent);
    body
}

/// The node a row holds, a field this build cannot read kept in `extra`. A row it cannot read
/// at all (a kind it does not know, a body that is not a node) is refused as
/// `design.node_unreadable`, never edited.
fn node_of(id: String, parent: Option<String>, body: &str) -> Result<Node, BusError> {
    let mut node = Node::read_str(body).map_err(|e| {
        BusError::invalid("design.node_unreadable", format!("node {id:?} cannot be read ({e}); open it in a newer Relay"))
    })?;
    (node.id, node.parent) = (id, parent);
    Ok(node)
}

pub fn register(e: &mut Engine) {
    // Unlocked (D149): pure reads the canvas repeats on every `design.changed`.
    e.register_unlocked::<List>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        let sql = if p.include_deleted.unwrap_or(false) {
            design_select!("WHERE project_id = ?1 AND (?2 IS NULL OR session = ?2) ORDER BY updated_at DESC, id DESC")
        } else {
            design_select!("WHERE project_id = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR session = ?2) ORDER BY updated_at DESC, id DESC")
        };
        let designs = ctx.read(|conn| {
            check_project(conn, p.project_id, session_id, agent)?;
            let mut stmt = conn.prepare_cached(sql).bus()?;
            let designs = stmt.query_map(params![p.project_id, p.session], design_row).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()?;
            Ok(designs)
        })?;
        Ok(ListOut { designs })
    });
    get::register(e);
    e.register::<Create>(|ctx: &mut Ctx, p| {
        assert_actor_project(ctx, p.project_id)?;
        let by = actor_name(ctx)?;
        let title = title(p.title.as_deref(), DEFAULT_TITLE)?;
        // An agent's design is its own terminal's unless it says otherwise (which layer 2
        // refuses for any name but its own); the person's belongs to the project alone.
        let session = p.session.or_else(|| ctx.actor.session_name().map(str::to_string));
        if let Some(session) = &session {
            check_session(ctx, p.project_id, session)?;
        }
        let mut page = Node::new(NodeKind::Page);
        page.id = "page-1".into();
        page.name = Some("Page 1".into());
        let body = body_of(&mut page)?;
        ctx.tx().execute(
            "INSERT INTO designs(project_id, session, title, rev, pages, nodes, bytes, next_node, created_by, updated_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, 1, 1, ?4, 2, ?5, ?5, ?6, ?6)",
            params![p.project_id, session, title, body.len() as i64, by, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        ctx.tx().execute(
            "INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body) VALUES (?1, ?2, NULL, 'page', 1.0, ?3)",
            params![id, page.id, body],
        ).bus()?;
        let design = get_design(ctx.tx(), id)?;
        ctx.set_undo("design.delete", json!({"design_id": id}), None);
        emit_design(ctx, &design)?;
        let assets_dir = ctx.engine().store.design_assets_dir().display().to_string();
        Ok(GetOut { design, nodes: vec![page], assets_dir })
    });
    e.register::<Update>(|ctx: &mut Ctx, p| {
        let before = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, before.project_id)?;
        let by = actor_name(ctx)?;
        let title = match p.title.as_deref() {
            Some(raw) => title(Some(raw), DEFAULT_TITLE)?,
            None => before.title.clone(),
        };
        let session = match p.session {
            None => before.session.clone(),
            Some(None) => None,
            Some(Some(session)) => {
                check_session(ctx, before.project_id, &session)?;
                Some(session)
            }
        };
        ctx.tx().execute(
            "UPDATE designs SET title = ?2, session = ?3, rev = rev + 1, updated_by = ?4, updated_at = ?5 WHERE id = ?1",
            params![before.id, title, session, by, ctx.now],
        ).bus()?;
        let design = get_design(ctx.tx(), before.id)?;
        ctx.set_undo("design.update", json!({"design_id": before.id, "title": before.title, "session": before.session}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    e.register::<Duplicate>(|ctx: &mut Ctx, p| {
        let from = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, from.project_id)?;
        let by = actor_name(ctx)?;
        let title = match p.title.as_deref() {
            Some(raw) => title(Some(raw), DEFAULT_TITLE)?,
            None => copy_title(&from.title),
        };
        ctx.tx().execute(
            "INSERT INTO designs(project_id, session, title, rev, pages, nodes, bytes, next_node, created_by, updated_by, created_at, updated_at)
             SELECT project_id, session, ?2, 1, pages, nodes, bytes, next_node, ?3, ?3, ?4, ?4 FROM designs WHERE id = ?1",
            params![from.id, title, by, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        ctx.tx().execute(
            "INSERT INTO design_nodes(design_id, node_id, parent_id, kind, ord, body)
             SELECT ?1, node_id, parent_id, kind, ord, body FROM design_nodes WHERE design_id = ?2",
            params![id, from.id],
        ).bus()?;
        let design = get_design(ctx.tx(), id)?;
        ctx.set_undo("design.delete", json!({"design_id": id}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    // Soft (BUS.md §5.4): the nodes stay until `design.purge` or the retention pass.
    e.register::<Delete>(|ctx: &mut Ctx, p| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        let by = actor_name(ctx)?;
        ctx.tx().execute(
            "UPDATE designs SET deleted_at = ?2, updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![design.id, ctx.now, by],
        ).bus()?;
        let design = get_design(ctx.tx(), design.id)?;
        ctx.set_undo("design.restore", json!({"design_id": design.id}), None);
        emit_design(ctx, &design)?;
        Ok(Empty {})
    });
    e.register::<Restore>(|ctx: &mut Ctx, p| {
        let before = get_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, before.project_id)?;
        if before.deleted_at.is_none() {
            return Err(BusError::conflict("design.not_deleted", format!("design {} is not deleted", before.id)));
        }
        let by = actor_name(ctx)?;
        ctx.tx().execute(
            "UPDATE designs SET deleted_at = NULL, updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![before.id, ctx.now, by],
        ).bus()?;
        let design = get_design(ctx.tx(), before.id)?;
        ctx.set_undo("design.delete", json!({"design_id": design.id}), None);
        emit_design(ctx, &design)?;
        Ok(design)
    });
    e.register::<Purge>(|ctx: &mut Ctx, p| {
        let design = get_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        if design.deleted_at.is_none() {
            return Err(BusError::conflict("design.not_deleted", format!("design {} is not deleted", design.id))
                .with_hint("design.delete it first; only a deleted design is purged"));
        }
        ctx.tx().execute("DELETE FROM design_nodes WHERE design_id = ?1", [design.id]).bus()?;
        ctx.tx().execute("DELETE FROM designs WHERE id = ?1", [design.id]).bus()?;
        ctx.set_project(design.project_id);
        ctx.emit("design.deleted", json!({"id": design.id, "project_id": design.project_id}));
        Ok(Empty {})
    });
    apply::register(e);
    assets::register(e);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_title_stays_a_title() {
        assert_eq!(copy_title("Home"), "Home copy");
        let long = "x".repeat(MAX_NAME);
        assert_eq!(copy_title(&long).chars().count(), MAX_NAME);
        assert!(title(Some("  "), DEFAULT_TITLE).is_err());
        assert_eq!(title(None, DEFAULT_TITLE).unwrap(), DEFAULT_TITLE);
    }

    #[test]
    fn every_kind_reads_back_as_itself() {
        for kind in NodeKind::ALL {
            assert_eq!(kind_of(kind.name()), Some(kind));
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.name(), "the column names a kind as its JSON does");
        }
        assert_eq!(kind_of("component"), None, "a newer build's kind");
    }

    #[test]
    fn a_stored_body_has_no_id_or_parent() {
        let mut node = Node::new(NodeKind::Rect);
        node.id = "r".into();
        node.parent = Some("p".into());
        node.fill = Some("#fff".into());
        let body = body_of(&mut node).unwrap();
        assert!(!body.contains("\"id\"") && !body.contains("\"parent\""), "{body}");
        assert_eq!((node.id.as_str(), node.parent.as_deref()), ("r", Some("p")), "the node keeps both");
        assert_eq!(node_of("r".into(), Some("p".into()), &body).unwrap(), node);
    }
}
