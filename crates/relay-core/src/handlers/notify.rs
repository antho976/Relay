//! Phase 9 notification center and one-shot dashboard aggregation.

use crate::engine::{Ctx, Engine, IntoBus};
use relay_bus::error::Confirm;
use relay_bus::ops::notify::*;
use relay_bus::types::{Notification, NotifyCategory};
use relay_bus::Empty;
use rusqlite::Row;
use serde_json::json;

fn category(value: &str) -> NotifyCategory {
    match value {
        "agent_done" => NotifyCategory::AgentDone,
        "agent_blocked" => NotifyCategory::AgentBlocked,
        "guardrail" => NotifyCategory::Guardrail,
        "integration" => NotifyCategory::Integration,
        "provider" => NotifyCategory::Provider,
        "disk" => NotifyCategory::Disk,
        "aide" => NotifyCategory::Aide,
        _ => NotifyCategory::System,
    }
}

fn category_str(value: NotifyCategory) -> &'static str {
    match value {
        NotifyCategory::AgentDone => "agent_done",
        NotifyCategory::AgentBlocked => "agent_blocked",
        NotifyCategory::Guardrail => "guardrail",
        NotifyCategory::Integration => "integration",
        NotifyCategory::Provider => "provider",
        NotifyCategory::Disk => "disk",
        NotifyCategory::Aide => "aide",
        NotifyCategory::System => "system",
    }
}

fn notification_row(row: &Row) -> rusqlite::Result<Notification> {
    let link = row
        .get::<_, Option<String>>("link")?
        .and_then(|value| serde_json::from_str::<Confirm>(&value).ok());
    Ok(Notification {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        category: category(&row.get::<_, String>("category")?),
        title: row.get("title")?,
        body: row.get("body")?,
        link,
        read: row.get::<_, i64>("read")? != 0,
        created_at: row.get("created_at")?,
    })
}

fn list(
    ctx: &Ctx,
    project_id: Option<i64>,
    unread: Option<bool>,
    category_: Option<NotifyCategory>,
    limit: Option<u32>,
) -> Result<Vec<Notification>, relay_bus::BusError> {
    let mut sql = String::from("SELECT * FROM notifications WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(project_id) = project_id {
        sql.push_str(" AND project_id=?");
        args.push(Box::new(project_id));
    }
    if unread.unwrap_or(false) {
        sql.push_str(" AND read=0");
    }
    if let Some(category_) = category_ {
        sql.push_str(" AND category=?");
        args.push(Box::new(category_str(category_).to_string()));
    }
    // `?+0`: SQLite re-prepares a statement whose LIMIT is a bare parameter on every run.
    sql.push_str(" ORDER BY created_at DESC,id DESC LIMIT ?+0");
    args.push(Box::new(i64::from(limit.unwrap_or(100).min(500))));
    // Eight filter shapes at most: each is compiled once, not once per badge refresh.
    let mut stmt = ctx.tx().prepare_cached(&sql).bus()?;
    let notifications = stmt
        .query_map(
            rusqlite::params_from_iter(args.iter().map(|value| value.as_ref())),
            notification_row,
        )
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    Ok(notifications)
}

/// Unread notifications under the same project and category filters as [`list`], with no
/// limit: the badge and "Mark all read" count what is unread, not what fits in one page (RA-490).
fn unread(ctx: &Ctx, project_id: Option<i64>, category_: Option<NotifyCategory>) -> Result<i64, relay_bus::BusError> {
    let category_ = category_.map(category_str);
    let sql = match (project_id.is_some(), category_.is_some()) {
        (false, false) => "SELECT COUNT(*) FROM notifications WHERE read=0",
        (true, false) => "SELECT COUNT(*) FROM notifications WHERE read=0 AND project_id=?1",
        (false, true) => "SELECT COUNT(*) FROM notifications WHERE read=0 AND category=?2",
        (true, true) => "SELECT COUNT(*) FROM notifications WHERE read=0 AND project_id=?1 AND category=?2",
    };
    let mut stmt = ctx.tx().prepare_cached(sql).bus()?;
    // Unused numbered parameters are still bound: SQLite numbers them up to the highest used.
    let count = if category_.is_some() {
        stmt.query_row(rusqlite::params![project_id, category_], |row| row.get(0))
    } else if project_id.is_some() {
        stmt.query_row([project_id], |row| row.get(0))
    } else {
        stmt.query_row([], |row| row.get(0))
    };
    count.bus()
}

/// The one project an agent sees (D106; BUS.md §9.1 layer 2), or `None` for a person, who sees
/// them all. The dashboard and the notification list are otherwise every project's (RA-386).
fn agent_project(ctx: &Ctx) -> Result<Option<i64>, relay_bus::BusError> {
    if let Some(session_id) = ctx.actor_session_id() {
        let row = crate::sessions::by_id(ctx.tx(), session_id)?
            .ok_or_else(|| relay_bus::BusError::actor("bound session no longer exists"))?;
        return Ok(Some(row.session.project_id));
    }
    if ctx.actor.is_agent() {
        return Err(relay_bus::BusError::actor("agent actor is not bound to a live session"));
    }
    Ok(None)
}

pub fn register(engine: &mut Engine) {
    engine.register::<List>(|ctx, payload| {
        let project_id = match (agent_project(ctx)?, payload.project_id) {
            (Some(own), Some(asked)) if asked != own => return Err(relay_bus::BusError::not_own("project")),
            (Some(own), _) => Some(own),
            (None, asked) => asked,
        };
        let unread = unread(ctx, project_id, payload.category)?;
        let notifications = if payload.count_only.unwrap_or(false) {
            Vec::new()
        } else {
            list(ctx, project_id, payload.unread_only, payload.category, payload.limit)?
        };
        Ok(ListOut { notifications, unread })
    });
    engine.register::<Ack>(|ctx: &mut Ctx, payload| {
        let changed = ctx
            .tx()
            .execute(
                "UPDATE notifications SET read=1 WHERE id=?1",
                [payload.notification_id],
            )
            .bus()?;
        if changed == 0 {
            return Err(relay_bus::BusError::not_found(
                "notify.not_found",
                format!("no notification {}", payload.notification_id),
            ));
        }
        ctx.emit(
            "notify.changed",
            json!({"notification_id":payload.notification_id,"read":true}),
        );
        Ok(Empty {})
    });
    engine.register::<AckAll>(|ctx: &mut Ctx, payload| {
        if let Some(category) = payload.category {
            ctx.tx()
                .execute(
                    "UPDATE notifications SET read=1 WHERE read=0 AND category=?1",
                    [category_str(category)],
                )
                .bus()?;
        } else {
            ctx.tx()
                .execute("UPDATE notifications SET read=1 WHERE read=0", [])
                .bus()?;
        }
        ctx.emit(
            "notify.changed",
            json!({"all":true,"category":payload.category}),
        );
        Ok(Empty {})
    });
    engine.register::<SettingsGetNotify>(|ctx, _| {
        crate::handlers::settings::get(ctx.tx(), Some("notifications"))
    });
    engine.register::<SettingsSetNotify>(|ctx: &mut Ctx, payload| {
        if !payload.patch.is_object() {
            return Err(relay_bus::BusError::invalid(
                "notify.settings",
                "patch must be an object",
            ));
        }
        let before = crate::handlers::settings::get(ctx.tx(), Some("notifications"))?;
        let mut next = before.clone();
        crate::handlers::settings::merge_value(&mut next, &payload.patch);
        crate::handlers::settings::set(ctx.tx(), "notifications", &next, &ctx.now)?;
        // The inverse is a patch too, so it must null the keys this one added: replaying the old
        // tree as a merge patch would leave them in place.
        ctx.set_undo("notify.settings.set", json!({"patch":crate::guardrail::inverse_patch(&before, &next)}), None);
        ctx.emit(
            "settings.changed",
            json!({"path":"notifications","value":next}),
        );
        Ok(next)
    });
    engine.register::<DashboardGet>(|ctx, _| {
        // An agent's dashboard is its own project's; NULL is every project.
        let only = agent_project(ctx)?;
        let project_ids = ctx.tx().prepare_cached("SELECT id FROM projects WHERE ?1 IS NULL OR id=?1 ORDER BY id").bus()?
            .query_map([only], |row| row.get::<_, i64>(0)).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        let mut sessions_live = Vec::new();
        for project_id in project_ids { sessions_live.extend(crate::awareness::peers(ctx.tx(), project_id, None, Some(ctx.engine()))?); }
        sessions_live.retain(|peer| matches!(peer.state, relay_bus::types::SessionState::Spawning | relay_bus::types::SessionState::Running | relay_bus::types::SessionState::Idle | relay_bus::types::SessionState::Blocked));
        let mut stmt = ctx.tx().prepare_cached("SELECT * FROM tasks WHERE col='in_review' AND deleted_at IS NULL AND (?1 IS NULL OR project_id=?1) ORDER BY updated_at DESC,id DESC").bus()?;
        let cols = crate::handlers::task::TaskCols::of(&stmt).bus()?;
        let mut rows = stmt.query([only]).bus()?;
        let mut in_review = Vec::new();
        while let Some(row) = rows.next().bus()? { in_review.push(crate::handlers::task::task_columns_at(row, &cols).bus()?); }
        drop(rows);
        crate::handlers::task::hydrate(ctx.tx(), &mut in_review).bus()?;
        // The newest open holds, cut like guardrail.holds.list cuts them: the dashboard is one
        // reply, and a client drops any line over its cap (RA-217).
        let mut holds_open = ctx.tx().prepare_cached("SELECT * FROM holds WHERE state='open' AND (?1 IS NULL OR project_id=?1) ORDER BY created_at DESC,id DESC LIMIT 100").bus()?
            .query_map([only], crate::guardrail::hold_row).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        for hold in &mut holds_open { crate::handlers::guardrail::elide_hold(hold, &mut Vec::new()); }
        let projects = ctx.tx().prepare_cached(
            "SELECT p.id,p.name,p.base_branch,
             COALESCE(SUM(CASE WHEN t.col!='done' THEN 1 ELSE 0 END),0),
             COALESCE(SUM(CASE WHEN t.col='ready' THEN 1 ELSE 0 END),0),
             COALESCE(SUM(CASE WHEN t.col='active' THEN 1 ELSE 0 END),0),
             COALESCE(SUM(CASE WHEN t.col='in_review' THEN 1 ELSE 0 END),0),
             COALESCE(SUM(CASE WHEN t.col='done' AND datetime(t.updated_at)>=datetime('now','-7 days') THEN 1 ELSE 0 END),0),
             (SELECT COUNT(*) FROM sessions s WHERE s.project_id=p.id AND s.state IN ('spawning','running','idle','blocked') AND s.provider!='shell'),
             (SELECT COUNT(*) FROM sessions s WHERE s.project_id=p.id AND s.state='blocked')
             FROM projects p LEFT JOIN tasks t ON t.project_id=p.id AND t.deleted_at IS NULL
             WHERE ?1 IS NULL OR p.id=?1
             GROUP BY p.id,p.name,p.base_branch ORDER BY p.name,p.id"
        ).bus()?.query_map([only], |row| Ok(DashboardProject {
            project_id: row.get(0)?, name: row.get(1)?, base_branch: row.get(2)?, tasks_open: row.get(3)?,
            ready: row.get(4)?, active: row.get(5)?, in_review: row.get(6)?, done_recent: row.get(7)?,
            live_sessions: row.get(8)?, blocked_sessions: row.get(9)?,
        })).bus()?.collect::<rusqlite::Result<Vec<_>>>().bus()?;
        let notifications = list(ctx, only, Some(false), None, Some(8))?;
        let resources = crate::handlers::app::resources(ctx.tx(), ctx.engine())?;
        Ok(DashboardOut { projects, sessions_live, in_review, holds_open, notifications, resources })
    });
}
