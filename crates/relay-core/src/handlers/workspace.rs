//! `workspace.*` / `project.*` (BUS.md §10.4). All mutations user-only (registry).

use crate::engine::{Ctx, Engine, IntoBus, Prepared, Unlocked};
use relay_bus::error::BusError;
use relay_bus::ops::workspace::*;
use relay_bus::types::{Id, Project, Workspace};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ws_row(r: &Row) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: r.get("id")?, path: r.get("path")?, name: r.get("name")?, order: r.get("ord")?,
        created_at: r.get("created_at")?, updated_at: r.get("updated_at")?,
    })
}

fn project_row(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get("id")?, workspace_id: r.get("workspace_id")?, path: r.get("path")?, name: r.get("name")?,
        base_branch: r.get("base_branch")?, build_cmd: r.get("build_cmd")?, run_cmd: r.get("run_cmd")?,
        protected_paths: super::util::json_vec(&r.get::<_, String>("protected_paths")?),
        critical_files: super::util::json_vec(&r.get::<_, String>("critical_files")?),
        order: r.get("ord")?, pinned: r.get::<_, i64>("pinned")? != 0,
        created_at: r.get("created_at")?, updated_at: r.get("updated_at")?,
    })
}

pub fn get_workspace(tx: &Connection, id: Id) -> Result<Workspace, BusError> {
    tx.query_row("SELECT * FROM workspaces WHERE id = ?1", [id], ws_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("workspace.not_found", format!("no workspace {id}")))
}

/// Nearly every project-scoped op starts here, so the statement is cached rather than
/// re-compiled per call.
pub fn get_project(tx: &Connection, id: Id) -> Result<Project, BusError> {
    tx.prepare_cached("SELECT * FROM projects WHERE id = ?1").bus()?
        .query_row([id], project_row).optional().bus()?
        .ok_or_else(|| BusError::not_found("project.not_found", format!("no project {id}")))
}

/// A reorder names each row once, and at least one (`project.reorder` / `workspace.reorder`).
fn check_orders(code: &str, ids: impl Iterator<Item = Id>) -> Result<(), BusError> {
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(BusError::invalid(code, format!("{id} is named twice; give each one order")));
        }
    }
    if seen.is_empty() {
        return Err(BusError::invalid(code, "orders is empty; name every row whose order changes"));
    }
    Ok(())
}

fn canon(path: &str, code: &str) -> Result<PathBuf, BusError> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(BusError::invalid(code, format!("{path:?} must be an absolute path")));
    }
    std::fs::canonicalize(p).map_err(|e| BusError::invalid(code, format!("{path:?}: {e}")))
}

fn canon_workspace(path: &str) -> Result<PathBuf, BusError> {
    let suggested;
    let p = if path.trim().is_empty() {
        suggested = suggested_workspace()?;
        suggested.as_path()
    } else {
        Path::new(path)
    };
    if !p.is_absolute() {
        return Err(BusError::invalid("workspace.path", format!("{path:?} must be an absolute path")));
    }
    if !p.exists() {
        std::fs::create_dir_all(p).map_err(|e| BusError::invalid("workspace.path", format!("cannot create {path:?}: {e}")))?;
    }
    std::fs::canonicalize(p).map_err(|e| BusError::invalid("workspace.path", format!("{path:?}: {e}")))
}

/// The default for a blank workspace path: the *engine's* working directory, or the parent of
/// the Git checkout it lies in, so discovery presents that repository as a project instead of
/// cloning another copy into its source tree. The engine cannot see its caller's directory, and
/// `relay serve` runs from the engine home, so a client that means "here" sends its own path —
/// the desktop onboarding prefills it (BUS.md §10.4).
fn suggested_workspace() -> Result<PathBuf, BusError> {
    let cwd = std::env::current_dir()
        .map_err(|error| BusError::unavailable("workspace.cwd", error.to_string()))?;
    let cwd = std::fs::canonicalize(&cwd)
        .map_err(|error| BusError::unavailable("workspace.cwd", error.to_string()))?;
    Ok(suggested_workspace_from(&cwd))
}

fn suggested_workspace_from(cwd: &Path) -> PathBuf {
    for ancestor in cwd.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor.parent().unwrap_or(ancestor).to_path_buf();
        }
    }
    cwd.to_path_buf()
}

/// A clone in progress lives in `<workspace>/.relay-clone-<uuid>` until it is complete.
const CLONE_SCRATCH: &str = ".relay-clone-";
/// Just under the desktop client's 30-minute allowance for this op, so the engine reports the
/// timeout rather than the client giving up on a clone that is still running.
const CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(29 * 60);

fn name_of(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string())
}

pub fn register(e: &mut Engine) {
    e.register::<WsCreate>(|ctx: &mut Ctx, p| {
        let path = canon_workspace(p.path.as_deref().unwrap_or(""))?;
        if !path.is_dir() {
            return Err(BusError::invalid("workspace.path", format!("{} is not a directory", path.display())));
        }
        let path_s = path.display().to_string();
        let dup: Option<Id> = ctx.tx().query_row("SELECT id FROM workspaces WHERE path = ?1", [&path_s], |r| r.get(0)).optional().bus()?;
        if let Some(id) = dup {
            return Err(BusError::conflict("workspace.exists", format!("{path_s} is already workspace {id}")).with_details(json!({"workspace_id": id})));
        }
        let name = p.name.unwrap_or_else(|| name_of(&path));
        let ord: i64 = ctx.tx().query_row("SELECT COALESCE(MAX(ord), -1) + 1 FROM workspaces", [], |r| r.get(0)).bus()?;
        let id = next_id(ctx.tx(), NEXT_WORKSPACE_ID, "ids.workspaces")?;
        ctx.tx().execute(
            "INSERT INTO workspaces(id, path, name, ord, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, path_s, name, ord, ctx.now],
        ).bus()?;
        let ws = get_workspace(ctx.tx(), id)?;
        ctx.emit("workspace.changed", serde_json::to_value(&ws).bus()?);
        Ok(ws)
    });
    // A tree walk up to four levels deep that never touches the store (D149).
    e.register_unlocked::<WsDiscover>(|_, p| {
        let path = if p.path.as_deref().is_none_or(|path| path.trim().is_empty()) {
            suggested_workspace()?
        } else {
            let candidate = PathBuf::from(p.path.unwrap());
            if !candidate.is_absolute() {
                return Err(BusError::invalid("workspace.path", "workspace path must be absolute"));
            }
            if !candidate.exists() {
                return Ok(WsDiscoverOut { path: candidate.display().to_string(), repositories: Vec::new() });
            }
            std::fs::canonicalize(&candidate).map_err(|error| BusError::invalid("workspace.path", error.to_string()))?
        };
        if !path.is_dir() {
            return Err(BusError::invalid("workspace.path", format!("{} is not a directory", path.display())));
        }
        Ok(WsDiscoverOut { path: path.display().to_string(), repositories: discover_repositories(&path)? })
    });
    e.register::<WsList>(|ctx, _| {
        let mut stmt = ctx.tx().prepare_cached("SELECT * FROM workspaces ORDER BY ord, id").bus()?;
        let rows = stmt.query_map([], ws_row).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        Ok(WsListOut { workspaces: rows.bus()? })
    });
    e.register::<WsUpdate>(|ctx: &mut Ctx, p| {
        let before = get_workspace(ctx.tx(), p.workspace_id)?;
        let name = p.name.clone().unwrap_or_else(|| before.name.clone());
        let ord = p.order.unwrap_or(before.order);
        ctx.tx().execute("UPDATE workspaces SET name = ?1, ord = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, ord, ctx.now, p.workspace_id]).bus()?;
        let ws = get_workspace(ctx.tx(), p.workspace_id)?;
        ctx.set_undo("workspace.update",
            json!({ "workspace_id": before.id, "name": before.name, "order": before.order }),
            Some(json!({ "updated_at": ws.updated_at })));
        ctx.emit("workspace.changed", serde_json::to_value(&ws).bus()?);
        Ok(ws)
    });
    // A sidebar drag renumbers a whole group: one transaction, so a failure part-way leaves
    // nothing half-applied, one undo entry that restores every previous order, and one event
    // carrying every row it touched (RA-502).
    e.register::<WsReorder>(|ctx: &mut Ctx, p| {
        check_orders("workspace.orders", p.orders.iter().map(|o| o.workspace_id))?;
        let mut previous = Vec::with_capacity(p.orders.len());
        for o in &p.orders {
            previous.push(json!({ "workspace_id": o.workspace_id, "order": get_workspace(ctx.tx(), o.workspace_id)?.order }));
        }
        let now = ctx.now.clone();
        let mut workspaces = Vec::with_capacity(p.orders.len());
        {
            let mut update = ctx.tx().prepare_cached("UPDATE workspaces SET ord = ?1, updated_at = ?2 WHERE id = ?3").bus()?;
            let mut read = ctx.tx().prepare_cached("SELECT * FROM workspaces WHERE id = ?1").bus()?;
            for o in &p.orders {
                update.execute(params![o.order, now, o.workspace_id]).bus()?;
                workspaces.push(read.query_row([o.workspace_id], ws_row).bus()?);
            }
        }
        ctx.set_undo("workspace.reorder", json!({ "orders": previous }), Some(json!({ "updated_at": now })));
        ctx.emit("workspace.changed", json!({ "workspaces": workspaces }));
        Ok(WsListOut { workspaces })
    });
    // Closing an agent undoes its hooks (git subprocesses, file writes): that half of every
    // close runs here, before the transaction, as does the store backup (RA-194, RA-195).
    e.register_staged::<WsRemove, WorkspaceRemoval>(|ctx, p| {
        let force = p.force.unwrap_or(false);
        let projects = ctx.read(|conn| workspace_scope(conn, p.workspace_id, force))?;
        let mut scopes = Vec::new();
        for &id in &projects {
            let (pr, open, _) = ctx.read(|conn| removal_scope(conn, id, true))?;
            scopes.push((id, pr.path, open));
        }
        // Refused before any backup is taken or any hook undone (RA-405).
        let doomed: Vec<super::session::Checkout> = if p.remove_worktrees.unwrap_or(false) {
            scopes.iter().flat_map(|(_, path, open)| pooled_checkouts(Path::new(path), open)).collect()
        } else {
            Vec::new()
        };
        if !p.discard_changes.unwrap_or(false) {
            super::session::refuse_dirty(&doomed)?;
        }
        let backup = if projects.is_empty() { None } else { backup_before(ctx, "workspace-remove")? };
        let mut closes = Vec::new();
        for (id, _, open) in &scopes {
            closes.push((*id, prepare_closes(ctx, open)?));
        }
        Ok(WorkspaceRemoval { closes, backup, approved: doomed.into_iter().map(|(wt, _)| wt).collect() })
    }, |ctx: &mut Ctx, p, staged| {
        let WorkspaceRemoval { mut closes, backup, approved } = staged;
        let projects = workspace_scope(ctx.tx(), p.workspace_id, p.force.unwrap_or(false))?;
        let n = projects.len();
        let mut sessions_closed = 0;
        for id in projects {
            let prepared = closes.iter().position(|(project, _)| *project == id).map(|i| closes.swap_remove(i).1).unwrap_or_default();
            let out = remove_project(ctx, id, true, p.remove_worktrees.unwrap_or(false), p.discard_changes.unwrap_or(false), &approved, prepared, None)?;
            sessions_closed += out.sessions_closed;
        }
        // The workspace's own guardrail layer goes with it, or the next workspace given this
        // id would inherit it.
        crate::handlers::settings::delete_under(ctx.tx(), &format!("guardrails.workspaces.{}", p.workspace_id))?;
        ctx.tx().execute("DELETE FROM workspaces WHERE id = ?1", [p.workspace_id]).bus()?;
        ctx.emit("workspace.deleted", json!({ "id": p.workspace_id, "backup": backup }));
        Ok(WsRemoveOut { projects_removed: n as i64, sessions_closed })
    });

    e.register::<ProjectAdd>(|ctx: &mut Ctx, p| {
        let ws = get_workspace(ctx.tx(), p.workspace_id)?;
        let path = canon(&p.path, "project.path")?;
        if !path.join(".git").exists() {
            return Err(BusError::invalid("project.path", format!("{} is not a git repository root", path.display())));
        }
        if !path.starts_with(&ws.path) {
            return Err(BusError::invalid("project.outside_workspace",
                format!("{} is not inside workspace {}", path.display(), ws.path)));
        }
        let path_s = path.display().to_string();
        let dup: Option<Id> = ctx.tx().query_row("SELECT id FROM projects WHERE path = ?1", [&path_s], |r| r.get(0)).optional().bus()?;
        if let Some(id) = dup {
            return Err(BusError::conflict("project.exists", format!("{path_s} is already project {id}")).with_details(json!({"project_id": id})));
        }
        let name = p.name.unwrap_or_else(|| name_of(&path));
        let ord: i64 = ctx.tx().query_row("SELECT COALESCE(MAX(ord), -1) + 1 FROM projects WHERE workspace_id = ?1", [ws.id], |r| r.get(0)).bus()?;
        let base = detect_default_branch(&path);
        // The check an agent's push runs and an integration builds with, until the person
        // names a stricter one (`check::detect`).
        let build_cmd = crate::check::detect(&path);
        let id = next_id(ctx.tx(), NEXT_PROJECT_ID, "ids.projects")?;
        ctx.tx().execute(
            "INSERT INTO projects(id, workspace_id, path, name, base_branch, build_cmd, ord, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![id, ws.id, path_s, name, base, build_cmd, ord, ctx.now],
        ).bus()?;
        let pr = get_project(ctx.tx(), id)?;
        // Skills are app-wide (D147): a project added today starts with every installed skill
        // enabled, and gets the folders as soon as the transaction commits.
        ctx.tx().execute(
            "INSERT OR IGNORE INTO skill_projects(skill_id,project_id) SELECT id,?1 FROM skills WHERE deleted_at IS NULL",
            [pr.id],
        ).bus()?;
        let root = path.clone();
        ctx.after_commit(move |engine| {
            std::thread::Builder::new().name("skill-materialize".into()).spawn(move || {
                let plan = { crate::skills::plan(&engine.store.lock(), &root, pr.id) };
                match plan {
                    Ok(plan) => if let Err(error) = crate::skills::apply(&plan, &engine.store) {
                        tracing::warn!(error = %error, "materializing skills");
                    },
                    Err(error) => tracing::warn!(error = %error, "planning skills"),
                }
            }).ok();
        });
        ctx.set_project(pr.id);
        ctx.emit("project.changed", serde_json::to_value(&pr).bus()?);
        Ok(pr)
    });
    // The clone is network work of unbounded length, so it runs before the transaction opens
    // (D149) and under a deadline (D144). It lands in a scratch directory beside the
    // destination and is renamed into place in `finish`, so a concurrent clone to the same name
    // can never be deleted by this one's cleanup.
    e.register_staged::<ProjectClone, PathBuf>(
        |ctx, p| {
            let workspace = ctx.read(|conn| get_workspace(conn, p.workspace_id))?;
            let destination = clone_destination(&workspace.path, &p.url, p.dest.as_deref())?;
            if destination.exists() {
                return Err(BusError::conflict("project.clone_destination", format!("{} already exists", destination.display())));
            }
            let scratch = Path::new(&workspace.path).join(format!("{CLONE_SCRATCH}{}", uuid::Uuid::new_v4().simple()));
            let mut command = Command::new("git");
            command.current_dir(&workspace.path);
            crate::proc::quiet_network_git(&mut command);
            command.arg("clone").arg("--").arg(&p.url).arg(&scratch);
            let output = crate::proc::output_with_timeout(&mut command, CLONE_TIMEOUT);
            let failed = |error: BusError| {
                let _ = fs::remove_dir_all(&scratch);
                Err(error)
            };
            match output {
                Err(error) => failed(BusError::unavailable("project.git_missing", format!("cannot start git: {error}"))),
                Ok(None) => failed(BusError::unavailable("project.clone_timeout",
                    format!("git clone did not finish within {} minutes", CLONE_TIMEOUT.as_secs() / 60))),
                Ok(Some(output)) if !output.status.success() => failed(BusError::unavailable(
                    "project.clone_failed", String::from_utf8_lossy(&output.stderr).trim().to_string())),
                Ok(Some(_)) => Ok(scratch),
            }
        },
        |ctx: &mut Ctx, p, scratch| {
            let discard = |error: BusError| {
                let _ = fs::remove_dir_all(&scratch);
                Err(error)
            };
            let found = get_workspace(ctx.tx(), p.workspace_id)
                .and_then(|workspace| Ok((clone_destination(&workspace.path, &p.url, p.dest.as_deref())?, workspace)));
            let (destination, workspace) = match found {
                Ok(found) => found,
                Err(error) => return discard(error),
            };
            if destination.exists() {
                return discard(BusError::conflict("project.clone_destination", format!("{} already exists", destination.display())));
            }
            if let Err(error) = fs::rename(&scratch, &destination) {
                return discard(BusError::unavailable("project.clone_failed", format!("moving the clone into place: {error}")));
            }
            let added = ctx.invoke_registered("project.add", json!({"workspace_id":workspace.id,"path":destination}))
                .and_then(|value| serde_json::from_value::<Project>(value).map_err(|error| BusError::internal(error.to_string())));
            match added {
                Ok(project) => Ok(ProjectCloneOut { project }),
                Err(error) => {
                    // This request put it there a moment ago; nothing else can own it yet.
                    let _ = fs::remove_dir_all(&destination);
                    Err(error)
                }
            }
        },
    );
    e.register::<ProjectList>(|ctx, p| {
        let projects = match p.workspace_id {
            Some(w) => {
                let mut stmt = ctx.tx().prepare_cached("SELECT * FROM projects WHERE workspace_id = ?1 ORDER BY pinned DESC, ord, id").bus()?;
                let rows = stmt.query_map([w], project_row).bus()?.collect::<rusqlite::Result<Vec<_>>>();
                rows.bus()?
            }
            None => {
                let mut stmt = ctx.tx().prepare_cached("SELECT * FROM projects ORDER BY workspace_id, pinned DESC, ord, id").bus()?;
                let rows = stmt.query_map([], project_row).bus()?.collect::<rusqlite::Result<Vec<_>>>();
                rows.bus()?
            }
        };
        Ok(ProjectListOut { projects })
    });
    e.register::<ProjectGet>(|ctx, p| get_project(ctx.tx(), p.project_id));
    e.register::<ProjectUpdate>(|ctx: &mut Ctx, p| {
        let b = get_project(ctx.tx(), p.project_id)?;
        let name = p.name.clone().unwrap_or_else(|| b.name.clone());
        let build_cmd = p.build_cmd.clone().unwrap_or_else(|| b.build_cmd.clone());
        let run_cmd = p.run_cmd.clone().unwrap_or_else(|| b.run_cmd.clone());
        let base_branch = p.base_branch.clone().unwrap_or_else(|| b.base_branch.clone());
        if p.base_branch.is_some() && !plausible_branch(&base_branch) {
            return Err(BusError::invalid("project.base_branch", format!("{base_branch:?} is not a branch name"))
                .with_hint("a branch name cannot start with '-' or hold spaces, '..', '~', '^', ':' or the like"));
        }
        let protected = p.protected_paths.clone().unwrap_or_else(|| b.protected_paths.clone());
        let critical = p.critical_files.clone().unwrap_or_else(|| b.critical_files.clone());
        let ord = p.order.unwrap_or(b.order);
        let pinned = p.pinned.unwrap_or(b.pinned);
        ctx.tx().execute(
            "UPDATE projects SET name=?1, build_cmd=?2, run_cmd=?3, base_branch=?4, protected_paths=?5, critical_files=?6, ord=?7, pinned=?8, updated_at=?9 WHERE id=?10",
            params![name, build_cmd, run_cmd, base_branch, serde_json::to_string(&protected).bus()?, serde_json::to_string(&critical).bus()?, ord, pinned as i64, ctx.now, b.id],
        ).bus()?;
        // These legacy columns feed the project's guardrail config: one bad pattern there makes
        // it unreadable and refuses every agent mutation in the project, so it is checked the
        // way `guardrail.config.set` checks its own before the write commits (RA-422).
        if p.protected_paths.is_some() || p.critical_files.is_some() {
            crate::guardrail::config(ctx.tx(), Some(b.id))?;
        }
        let pr = get_project(ctx.tx(), b.id)?;
        ctx.set_project(pr.id);
        ctx.set_undo("project.update", json!({
            "project_id": b.id, "name": b.name, "build_cmd": b.build_cmd, "run_cmd": b.run_cmd, "base_branch": b.base_branch,
            "protected_paths": b.protected_paths, "critical_files": b.critical_files, "order": b.order, "pinned": b.pinned,
        }), Some(json!({ "updated_at": pr.updated_at })));
        ctx.emit("project.changed", serde_json::to_value(&pr).bus()?);
        Ok(pr)
    });
    // As `workspace.reorder` (RA-502). Pinned projects still sort first within a workspace.
    e.register::<ProjectReorder>(|ctx: &mut Ctx, p| {
        check_orders("project.orders", p.orders.iter().map(|o| o.project_id))?;
        let mut previous = Vec::with_capacity(p.orders.len());
        for o in &p.orders {
            previous.push(json!({ "project_id": o.project_id, "order": get_project(ctx.tx(), o.project_id)?.order }));
        }
        let now = ctx.now.clone();
        let mut projects = Vec::with_capacity(p.orders.len());
        {
            let mut update = ctx.tx().prepare_cached("UPDATE projects SET ord = ?1, updated_at = ?2 WHERE id = ?3").bus()?;
            for o in &p.orders {
                update.execute(params![o.order, now, o.project_id]).bus()?;
                projects.push(get_project(ctx.tx(), o.project_id)?);
            }
        }
        ctx.set_undo("project.reorder", json!({ "orders": previous }), Some(json!({ "updated_at": now })));
        ctx.emit("project.changed", json!({ "projects": projects }));
        Ok(ProjectListOut { projects })
    });
    // As `workspace.remove`: hook teardown and the store backup before the transaction opens.
    // A checkout `remove_worktrees` would delete with uncommitted work in it refuses the whole
    // removal first, unless `discard_changes` (RA-405).
    e.register_staged::<ProjectRemove, ProjectRemoval>(|ctx, p| {
        let (pr, open, _) = ctx.read(|conn| removal_scope(conn, p.project_id, p.force.unwrap_or(false)))?;
        let doomed = if p.remove_worktrees.unwrap_or(false) { pooled_checkouts(Path::new(&pr.path), &open) } else { Vec::new() };
        if !p.discard_changes.unwrap_or(false) {
            super::session::refuse_dirty(&doomed)?;
        }
        let backup = backup_before(ctx, "project-remove")?;
        Ok(ProjectRemoval { closes: prepare_closes(ctx, &open)?, backup, approved: doomed.into_iter().map(|(wt, _)| wt).collect() })
    }, |ctx: &mut Ctx, p, staged| {
        remove_project(ctx, p.project_id, p.force.unwrap_or(false), p.remove_worktrees.unwrap_or(false), p.discard_changes.unwrap_or(false), &staged.approved, staged.closes, staged.backup)
    });
    e.register::<ProjectRemovePreview>(|ctx, p| {
        let projects: Vec<Id> = match (p.project_id, p.workspace_id) {
            (Some(id), None) => vec![get_project(ctx.tx(), id)?.id],
            (None, Some(ws)) => {
                get_workspace(ctx.tx(), ws)?;
                let mut stmt = ctx.tx().prepare_cached("SELECT id FROM projects WHERE workspace_id = ?1").bus()?;
                let rows = stmt.query_map([ws], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
                rows.bus()?
            }
            _ => return Err(BusError::invalid("project.remove_preview", "pass exactly one of project_id and workspace_id")),
        };
        let mut out = ProjectRemovePreviewOut { projects: projects.len() as i64, tasks: 0, notes: 0, modules: 0 };
        for id in projects {
            let (tasks, notes, modules): (i64, i64, i64) = ctx.tx().prepare_cached(
                "SELECT (SELECT COUNT(*) FROM tasks WHERE project_id=?1 AND deleted_at IS NULL),
                        (SELECT COUNT(*) FROM notes WHERE project_id=?1 AND deleted_at IS NULL),
                        (SELECT COUNT(*) FROM modules WHERE project_id=?1 AND deleted_at IS NULL)",
            ).bus()?.query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).bus()?;
            out.tasks += tasks;
            out.notes += notes;
            out.modules += modules;
        }
        Ok(out)
    });
    // A moved or renamed repository: the project keeps its id, board and notes, and only its
    // path (and the stored paths beneath it) change. No subprocess under the lock; the
    // `git worktree repair` its moved checkouts need runs after the commit.
    e.register::<ProjectRelink>(|ctx: &mut Ctx, p| {
        let before = get_project(ctx.tx(), p.project_id)?;
        let path = canon(&p.path, "project.path")?;
        if !path.join(".git").exists() {
            return Err(BusError::invalid("project.path", format!("{} is not a git repository root", path.display())));
        }
        let path_s = path.display().to_string();
        if path_s == before.path {
            return Ok(before);
        }
        let dup: Option<Id> = ctx.tx().prepare_cached("SELECT id FROM projects WHERE path = ?1").bus()?
            .query_row([&path_s], |r| r.get(0)).optional().bus()?;
        if let Some(id) = dup {
            return Err(BusError::conflict("project.exists", format!("{path_s} is already project {id}")).with_details(json!({"project_id": id})));
        }
        let open: i64 = ctx.tx().prepare_cached("SELECT COUNT(*) FROM sessions WHERE project_id=?1 AND state!='closed'").bus()?
            .query_row([before.id], |r| r.get(0)).bus()?;
        if open > 0 {
            return Err(BusError::conflict("project.sessions_live", format!("project {} still has {open} open session(s)", before.id))
                .with_details(json!({ "open_sessions": open }))
                .with_hint("close its sessions before relinking the project"));
        }
        let runs: i64 = ctx.tx().prepare_cached("SELECT COUNT(*) FROM device_runs WHERE project_id=?1 AND state IN ('building','running')").bus()?
            .query_row([before.id], |r| r.get(0)).bus()?;
        if runs > 0 || integrations_live(ctx.tx(), before.id)? > 0 {
            return Err(BusError::conflict("project.activity_live", "project still has an integration or device run in progress")
                .with_hint("wait for it to finish, then relink the project"));
        }
        // Membership as `project.add` decides it: the project must sit inside a workspace. It
        // stays in its own when that still contains it, else joins the innermost one that does.
        let current = get_workspace(ctx.tx(), before.workspace_id)?;
        let workspace = if path.starts_with(&current.path) {
            current
        } else {
            let all = {
                let mut stmt = ctx.tx().prepare_cached("SELECT * FROM workspaces ORDER BY ord, id").bus()?;
                let rows = stmt.query_map([], ws_row).bus()?.collect::<rusqlite::Result<Vec<_>>>();
                rows.bus()?
            };
            all.into_iter().filter(|w| path.starts_with(&w.path)).max_by_key(|w| w.path.len()).ok_or_else(|| {
                BusError::invalid("project.outside_workspace", format!("{} is not inside any workspace", path.display()))
                    .with_hint("add a workspace that contains the new location first (workspace.create)")
            })?
        };
        let ord: i64 = if workspace.id == before.workspace_id {
            before.order
        } else {
            ctx.tx().query_row("SELECT COALESCE(MAX(ord), -1) + 1 FROM projects WHERE workspace_id = ?1", [workspace.id], |r| r.get(0)).bus()?
        };
        ctx.tx().execute("UPDATE projects SET path=?1, workspace_id=?2, ord=?3, updated_at=?4 WHERE id=?5",
            params![path_s, workspace.id, ord, ctx.now, before.id]).bus()?;
        // Relay-pool worktrees, the file trash and integration checkouts live under the
        // repository, so they moved with it.
        for (table, column) in [("sessions", "worktree"), ("file_trash", "worktree"), ("file_trash", "trash_path"),
            ("integrations", "worktree"), ("device_runs", "worktree")] {
            let sql = format!("UPDATE {table} SET {column} = ?2 || substr({column}, length(?1) + 1)
                WHERE project_id = ?3 AND ({column} = ?1 OR substr({column}, 1, length(?1) + 1) = ?1 || '/')");
            ctx.tx().prepare_cached(&sql).bus()?.execute(params![before.path, path_s, before.id]).bus()?;
        }
        let moved: Vec<PathBuf> = {
            let mut stmt = ctx.tx().prepare_cached(
                "SELECT DISTINCT worktree FROM sessions WHERE project_id=?1 AND substr(worktree, 1, length(?2) + 1) = ?2 || '/'
                 UNION SELECT DISTINCT worktree FROM integrations WHERE project_id=?1 AND substr(worktree, 1, length(?2) + 1) = ?2 || '/'",
            ).bus()?;
            let rows = stmt.query_map(params![before.id, path_s], |r| r.get::<_, String>(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
            rows.bus()?.into_iter().map(PathBuf::from).collect()
        };
        let pr = get_project(ctx.tx(), before.id)?;
        ctx.set_project(pr.id);
        ctx.set_undo("project.relink", json!({ "project_id": before.id, "path": before.path }), Some(json!({ "updated_at": pr.updated_at })));
        ctx.emit("project.changed", serde_json::to_value(&pr).bus()?);
        ctx.after_commit(move |_| {
            // Their `.git` links still name the old location, in both directions.
            let moved: Vec<PathBuf> = moved.into_iter().filter(|wt| wt.join(".git").is_file()).collect();
            if moved.is_empty() {
                return;
            }
            let mut command = Command::new("git");
            command.arg("-C").arg(&path).args(["worktree", "repair"]).args(&moved);
            match crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(30)) {
                Ok(Some(output)) if output.status.success() => {}
                Ok(Some(output)) => tracing::warn!(stderr = %String::from_utf8_lossy(&output.stderr).trim(), "repairing a relinked project's worktrees"),
                Ok(None) => tracing::warn!("repairing a relinked project's worktrees timed out"),
                Err(error) => tracing::warn!(%error, "repairing a relinked project's worktrees"),
            }
        });
        Ok(pr)
    });
    // Reading the backup is file I/O, so it happens before the transaction opens, through a
    // read-only connection of its own; the copy back is one transaction.
    e.register_unlocked::<ProjectRemovedList>(|ctx, _| {
        let live: std::collections::HashSet<Id> = ctx.read(|conn| {
            let mut stmt = conn.prepare_cached("SELECT id FROM projects").bus()?;
            let rows = stmt.query_map([], |r| r.get(0)).bus()?.collect::<rusqlite::Result<_>>();
            rows.bus()
        })?;
        let backups = ctx.engine().store.list_backups().bus()?;
        let mut seen = std::collections::HashSet::new();
        let mut removed = Vec::new();
        // Newest first, so each project is offered from the last copy taken before it went.
        for backup in backups.into_iter().filter(|b| matches!(b.reason.as_str(), "project-remove" | "workspace-remove")) {
            let Ok(conn) = open_backup(&backup.path) else { continue };
            let Ok(mut stmt) = conn.prepare("SELECT id, workspace_id, name, path FROM projects ORDER BY id") else { continue };
            let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, Id>(0)?, r.get::<_, Id>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))) else { continue };
            for (project_id, workspace_id, name, path) in rows.flatten() {
                if !live.contains(&project_id) && seen.insert(project_id) {
                    removed.push(RemovedProject {
                        backup_path: backup.path.display().to_string(), created_at: backup.created_at.clone(),
                        reason: backup.reason.clone(), project_id, workspace_id, name, path,
                    });
                }
            }
        }
        Ok(ProjectRemovedListOut { removed })
    });
    e.register_staged::<ProjectRestore, Vec<Copied>>(|ctx, p| {
        let path = removal_backup(&ctx.engine().store, &p.backup_path)?;
        ctx.read(|conn| absent(conn, p.project_id))?;
        let conn = open_backup(&path).map_err(|error| BusError::invalid("project.backup_path", format!("{}: {error}", path.display())))?;
        let found: Option<Id> = conn.query_row("SELECT id FROM projects WHERE id = ?1", [p.project_id], |r| r.get(0)).optional().bus()?;
        if found.is_none() {
            return Err(BusError::not_found("project.not_in_backup", format!("{} holds no project {}", path.display(), p.project_id)));
        }
        RESTORED.iter().map(|&(part, table, sql)| -> Result<Copied, BusError> {
            // A backup from before a table existed has none of its rows to bring back, nor one from
            // before a table its select reads (design_removed's rows are found through designs').
            let reads = std::iter::once(table).chain(sql.split("FROM ").skip(1)
                .filter_map(|rest| rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next()));
            let mut held = true;
            for read in reads {
                held &= conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)", [read], |r| r.get::<_, bool>(0)).bus()?;
            }
            if !held {
                return Ok(Copied { part, table, columns: Vec::new(), rows: Vec::new() });
            }
            let mut stmt = conn.prepare(sql).bus()?;
            let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_string).collect();
            let rows = stmt.query_map([p.project_id], |r| (0..columns.len()).map(|i| r.get::<_, rusqlite::types::Value>(i)).collect())
                .bus()?.collect::<rusqlite::Result<Vec<Vec<_>>>>().bus()?;
            Ok(Copied { part, table, columns, rows })
        }).collect()
    }, |ctx: &mut Ctx, p, copied| restore_project(ctx, p.project_id, copied));
}

/// What `project.restore` copies back: every row `remove_project` deletes that is the
/// project's own content, in the order the transaction inserts it. Sessions and what hangs off
/// them (scrollback, claims, the mailbox, overlaps), integrations and device runs are runtime
/// state and stay gone. `(part, table, select)`: the select runs against the backup with the
/// project id as `?1`. The two `workspace*` parts apply only when the workspace went too.
const RESTORED: &[(&str, &str, &str)] = &[
    ("workspace", "workspaces", "SELECT * FROM workspaces WHERE id = (SELECT workspace_id FROM projects WHERE id = ?1)"),
    ("workspace_settings", "settings", "SELECT * FROM settings WHERE path = 'guardrails.workspaces.' || (SELECT workspace_id FROM projects WHERE id = ?1)
        OR path LIKE 'guardrails.workspaces.' || (SELECT workspace_id FROM projects WHERE id = ?1) || '.%'"),
    ("project", "projects", "SELECT * FROM projects WHERE id = ?1"),
    ("labels", "labels", "SELECT * FROM labels WHERE project_id = ?1"),
    ("modules", "modules", "SELECT * FROM modules WHERE project_id = ?1"),
    ("tasks", "tasks", "SELECT * FROM tasks WHERE project_id = ?1"),
    ("task_labels", "task_labels", "SELECT * FROM task_labels WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)"),
    ("task_relations", "task_relations", "SELECT * FROM task_relations WHERE from_task IN (SELECT id FROM tasks WHERE project_id = ?1)
        OR to_task IN (SELECT id FROM tasks WHERE project_id = ?1)"),
    ("task_commits", "task_commits", "SELECT * FROM task_commits WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)"),
    ("attachments", "attachments", "SELECT * FROM attachments WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)"),
    ("module_unlinked_tasks", "module_unlinked_tasks", "SELECT * FROM module_unlinked_tasks WHERE module_id IN (SELECT id FROM modules WHERE project_id = ?1)
        OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)"),
    ("notes", "notes", "SELECT * FROM notes WHERE project_id = ?1"),
    ("designs", "designs", "SELECT * FROM designs WHERE project_id = ?1"),
    ("design_nodes", "design_nodes", "SELECT * FROM design_nodes WHERE design_id IN (SELECT id FROM designs WHERE project_id = ?1)"),
    ("design_removed", "design_removed", "SELECT * FROM design_removed WHERE design_id IN (SELECT id FROM designs WHERE project_id = ?1)"),
    ("notifications", "notifications", "SELECT * FROM notifications WHERE project_id = ?1"),
    ("file_trash", "file_trash", "SELECT * FROM file_trash WHERE project_id = ?1"),
    ("ui_layouts", "ui_layouts", "SELECT * FROM ui_layouts WHERE project_id = ?1"),
    ("skill_projects", "skill_projects", "SELECT * FROM skill_projects WHERE project_id = ?1"),
    ("plugin_projects", "plugin_projects", "SELECT * FROM plugin_projects WHERE project_id = ?1"),
    ("settings", "settings", "SELECT * FROM settings WHERE path = 'layout.current.' || ?1 OR path = 'guardrails.projects.' || ?1
        OR path LIKE 'guardrails.projects.' || ?1 || '.%'"),
];

/// One part of [`RESTORED`], read out of the backup.
struct Copied {
    part: &'static str,
    table: &'static str,
    columns: Vec<String>,
    rows: Vec<Vec<rusqlite::types::Value>>,
}

/// A backup file, opened so that nothing can write to it.
fn open_backup(path: &Path) -> rusqlite::Result<Connection> {
    use rusqlite::OpenFlags;
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

/// `raw`, if it names a store backup directly inside the store's `backups/` directory.
fn removal_backup(store: &crate::Store, raw: &str) -> Result<PathBuf, BusError> {
    let refuse = || BusError::invalid("project.backup_path", format!("{raw:?} is not a backup in {}", store.backup_dir().display()))
        .with_hint("pass a backup_path from project.removed.list");
    let dir = fs::canonicalize(store.backup_dir()).map_err(|_| refuse())?;
    let path = canon(raw, "project.backup_path")?;
    let named = path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("store-") && name.ends_with(".db"));
    if !named || path.parent() != Some(dir.as_path()) || !path.is_file() {
        return Err(refuse());
    }
    Ok(path)
}

fn absent(conn: &Connection, project_id: Id) -> Result<(), BusError> {
    let live: Option<String> = conn.prepare_cached("SELECT name FROM projects WHERE id = ?1").bus()?
        .query_row([project_id], |r| r.get(0)).optional().bus()?;
    match live {
        Some(name) => Err(BusError::conflict("project.exists", format!("project {project_id} ({name}) is in the store; nothing to restore"))
            .with_details(json!({ "project_id": project_id }))),
        None => Ok(()),
    }
}

/// `project.restore`'s transaction: the rows [`RESTORED`] read, inserted with their original
/// ids. Ids are never handed out twice (see [`next_id`] and the AUTOINCREMENT tables), so none
/// of them can collide with a row made since.
fn restore_project(ctx: &mut Ctx, project_id: Id, copied: Vec<Copied>) -> Result<ProjectRestoreOut, BusError> {
    absent(ctx.tx(), project_id)?;
    let value = |part: &str, column: &str| -> Option<rusqlite::types::Value> {
        let c = copied.iter().find(|c| c.part == part)?;
        let i = c.columns.iter().position(|name| name == column)?;
        c.rows.first().map(|row| row[i].clone())
    };
    use rusqlite::types::Value as Sql;
    let (Some(Sql::Text(path)), Some(Sql::Integer(workspace_id))) = (value("project", "path"), value("project", "workspace_id")) else {
        return Err(BusError::internal(format!("the backup's project {project_id} has no path or workspace")));
    };
    let dup: Option<Id> = ctx.tx().prepare_cached("SELECT id FROM projects WHERE path = ?1").bus()?
        .query_row([&path], |r| r.get(0)).optional().bus()?;
    if let Some(id) = dup {
        return Err(BusError::conflict("project.exists", format!("{path} is now project {id}"))
            .with_details(json!({ "project_id": id }))
            .with_hint("remove or relink that project first"));
    }
    // The workspace went too (`workspace.remove`): bring it back, unless its directory is a
    // workspace again under a new id, which then takes the project.
    let mut workspace = workspace_id;
    let mut workspace_restored = false;
    if get_workspace(ctx.tx(), workspace_id).is_err() {
        let Some(Sql::Text(ws_path)) = value("workspace", "path") else {
            return Err(BusError::not_found("workspace.not_found", format!("the backup holds no workspace {workspace_id}")));
        };
        let again: Option<Id> = ctx.tx().prepare_cached("SELECT id FROM workspaces WHERE path = ?1").bus()?
            .query_row([&ws_path], |r| r.get(0)).optional().bus()?;
        match again {
            Some(id) => workspace = id,
            None => workspace_restored = true,
        }
    }
    // Rows go in in table order, so a sub-task may precede its parent: check once, at commit.
    ctx.tx().execute_batch("PRAGMA defer_foreign_keys = ON").bus()?;
    let mut counts = std::collections::HashMap::new();
    for c in &copied {
        if c.part.starts_with("workspace") && !workspace_restored {
            continue;
        }
        let live: Vec<String> = {
            let mut stmt = ctx.tx().prepare_cached("SELECT name FROM pragma_table_info(?1)").bus()?;
            let rows = stmt.query_map([c.table], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
            rows.bus()?
        };
        // A backup from an older schema lacks the columns added since; their defaults apply.
        let keep: Vec<usize> = (0..c.columns.len()).filter(|&i| live.contains(&c.columns[i])).collect();
        if keep.is_empty() || c.rows.is_empty() {
            continue;
        }
        let names = keep.iter().map(|&i| format!("\"{}\"", c.columns[i])).collect::<Vec<_>>().join(", ");
        let marks = (1..=keep.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
        // Settings and link rows are keyed by their content; anything already there since wins.
        let verb = if matches!(c.table, "settings" | "skill_projects" | "plugin_projects" | "ui_layouts") { "INSERT OR IGNORE" } else { "INSERT" };
        let sql = format!("{verb} INTO {} ({names}) VALUES ({marks})", c.table);
        let workspace_column = (c.table == "projects").then(|| c.columns.iter().position(|name| name == "workspace_id")).flatten();
        let mut stmt = ctx.tx().prepare(&sql).bus()?;
        for row in &c.rows {
            let values = keep.iter().map(|&i| if Some(i) == workspace_column { Sql::Integer(workspace) } else { row[i].clone() });
            stmt.execute(rusqlite::params_from_iter(values)).map_err(|error| {
                BusError::conflict("project.restore_failed", format!("restoring {}: {error}", c.table))
            })?;
        }
        counts.insert(c.part, c.rows.len() as i64);
    }
    // Links to rows that are gone for good — a relation to another removed project's task, a
    // skill uninstalled since — are dropped rather than restored dangling.
    for sql in [
        "UPDATE tasks SET parent_id = NULL WHERE project_id = ?1 AND parent_id IS NOT NULL AND parent_id NOT IN (SELECT id FROM tasks)",
        "UPDATE tasks SET module_id = NULL WHERE project_id = ?1 AND module_id IS NOT NULL AND module_id NOT IN (SELECT id FROM modules)",
        "UPDATE tasks SET note_id = NULL WHERE project_id = ?1 AND note_id IS NOT NULL AND note_id NOT IN (SELECT id FROM notes)",
        "DELETE FROM task_relations WHERE from_task NOT IN (SELECT id FROM tasks) OR to_task NOT IN (SELECT id FROM tasks)",
        "DELETE FROM module_unlinked_tasks WHERE module_id NOT IN (SELECT id FROM modules) OR task_id NOT IN (SELECT id FROM tasks)",
        "DELETE FROM task_labels WHERE label_id NOT IN (SELECT id FROM labels)",
        "DELETE FROM skill_projects WHERE project_id = ?1 AND skill_id NOT IN (SELECT id FROM skills WHERE deleted_at IS NULL)",
    ] {
        let mut stmt = ctx.tx().prepare_cached(sql).bus()?;
        if sql.contains("?1") { stmt.execute([project_id]) } else { stmt.execute([]) }.bus()?;
    }
    // A design from a backup older than v30 counted its size without its ids, and its rows carry
    // no rev (0): count it again, as every write keeps it now (B60), and date the rows at their
    // design's rev, so since_rev and base_rev see them as they are (W20).
    let designs: Vec<Id> = {
        let mut stmt = ctx.tx().prepare_cached("SELECT id FROM designs WHERE project_id = ?1").bus()?;
        let rows = stmt.query_map([project_id], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        rows.bus()?
    };
    for design in designs {
        super::design::recount_bytes(ctx.tx(), design)?;
        ctx.tx().prepare_cached("UPDATE design_nodes SET rev = (SELECT rev FROM designs WHERE id = ?1) WHERE design_id = ?1 AND rev = 0").bus()?
            .execute([design]).bus()?;
    }
    // Keep the high-water marks past the restored ids, whatever the backup predates.
    for (key, id) in [("ids.projects", project_id), ("ids.workspaces", workspace)] {
        ctx.tx().prepare_cached("INSERT INTO meta(key, value) VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = MAX(CAST(value AS INTEGER), CAST(excluded.value AS INTEGER))").bus()?
            .execute(params![key, id.to_string()]).bus()?;
    }
    let project = get_project(ctx.tx(), project_id)?;
    ctx.set_project(project.id);
    if workspace_restored {
        let ws = get_workspace(ctx.tx(), workspace)?;
        ctx.emit("workspace.changed", serde_json::to_value(&ws).bus()?);
    }
    ctx.emit("project.changed", serde_json::to_value(&project).bus()?);
    let count = |part: &str| counts.get(part).copied().unwrap_or(0);
    Ok(ProjectRestoreOut { tasks: count("tasks"), notes: count("notes"), modules: count("modules"), project, workspace_restored })
}

/// What `project.remove`'s unlocked half settled: each open session's `session.close`
/// prepared (its hooks already undone), the store backup taken first, and the checkouts
/// `remove_worktrees` may delete: those checked for uncommitted work (RA-405).
struct ProjectRemoval {
    closes: Vec<(String, Prepared)>,
    backup: Option<String>,
    approved: Vec<PathBuf>,
}

/// The same for each project of a removed workspace, and one backup for all of them.
struct WorkspaceRemoval {
    closes: Vec<(Id, Vec<(String, Prepared)>)>,
    backup: Option<String>,
    approved: Vec<PathBuf>,
}

/// The Relay-pool checkouts of `open` sessions, each once with the sessions on it: what
/// `remove_worktrees` deletes.
fn pooled_checkouts(repo: &Path, open: &[(String, String)]) -> Vec<super::session::Checkout> {
    let pool = crate::worktree::pool_dir(repo);
    let mut out: Vec<super::session::Checkout> = Vec::new();
    for (name, wt) in open {
        let wt = PathBuf::from(wt);
        if !wt.starts_with(&pool) { continue; }
        match out.iter_mut().find(|(seen, _)| *seen == wt) {
            Some((_, names)) => names.push(name.clone()),
            None => out.push((wt, vec![name.clone()])),
        }
    }
    out
}

/// The projects `workspace.remove` would remove, or its refusal. Read in both phases.
fn workspace_scope(conn: &Connection, workspace_id: Id, force: bool) -> Result<Vec<Id>, BusError> {
    get_workspace(conn, workspace_id)?;
    let projects: Vec<Id> = {
        let mut stmt = conn.prepare_cached("SELECT id FROM projects WHERE workspace_id = ?1 ORDER BY id").bus()?;
        let rows = stmt.query_map([workspace_id], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        rows.bus()?
    };
    let n = projects.len();
    if n > 0 && !force {
        return Err(BusError::conflict("workspace.has_projects", format!("workspace {workspace_id} still has {n} project(s)"))
            .with_details(json!({ "projects": n }))
            .with_hint("remove its projects first (project.remove), or pass force to remove them with it"));
    }
    // Refuse before closing anything: one project mid-integration must not leave the rest
    // half-removed. Each project removal repeats the check for itself.
    for &id in &projects {
        if integrations_live(conn, id)? > 0 {
            return Err(BusError::conflict("project.activity_live", format!("project {id} has an integration in progress"))
                .with_hint("wait for the integration to finish, then remove the workspace"));
        }
    }
    Ok(projects)
}

/// The project `project.remove` would remove, its open sessions (name, worktree) and its live
/// device runs, or its refusal. Read in both phases.
#[allow(clippy::type_complexity)]
fn removal_scope(conn: &Connection, project_id: Id, force: bool) -> Result<(Project, Vec<(String, String)>, Vec<Id>), BusError> {
    let pr = get_project(conn, project_id)?;
    let open: Vec<(String, String)> = {
        let mut stmt = conn.prepare_cached(
            "SELECT name, worktree FROM sessions WHERE project_id=?1 AND state!='closed' ORDER BY id",
        ).bus()?;
        let rows = stmt.query_map([pr.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        rows.bus()?
    };
    if !open.is_empty() && !force {
        return Err(BusError::conflict("project.sessions_live", format!("project {} still has {} open session(s)", pr.id, open.len()))
            .with_details(json!({ "open_sessions": open.len() }))
            .with_hint("close its sessions before removing the project, or pass force to close them with it"));
    }
    // An integration may be mid-merge in the primary checkout; never interrupt it, force or not.
    if integrations_live(conn, pr.id)? > 0 {
        return Err(BusError::conflict("project.activity_live", "project still has an integration in progress")
            .with_hint("wait for the integration to finish before removing the project"));
    }
    let live_runs: Vec<Id> = {
        let mut stmt = conn.prepare_cached(
            "SELECT id FROM device_runs WHERE project_id=?1 AND state IN ('building','running') ORDER BY id",
        ).bus()?;
        let rows = stmt.query_map([pr.id], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        rows.bus()?
    };
    if !live_runs.is_empty() && !force {
        return Err(BusError::conflict("project.activity_live", "project still has an active device run")
            .with_details(json!({ "active_runs": live_runs.len() }))
            .with_hint("stop active runs before removing the project, or pass force to stop them with it"));
    }
    Ok((pr, open, live_runs))
}

fn close_payload(name: &str) -> serde_json::Value {
    json!({ "session": name, "remove_worktree": false })
}

/// Run the unlocked half of each `session.close` a removal will make. The last session on a
/// shared checkout (a PAIR, a review group) is left to the transaction: only once its partners
/// are closed in there does it see that nobody is left and undo the checkout's hooks.
fn prepare_closes(ctx: &Unlocked, open: &[(String, String)]) -> Result<Vec<(String, Prepared)>, BusError> {
    let mut closes = Vec::new();
    for (i, (name, worktree)) in open.iter().enumerate() {
        if open[i + 1..].iter().any(|(_, other)| other == worktree) || open.iter().filter(|(_, other)| other == worktree).count() == 1 {
            if let Some(prepared) = ctx.prepare_registered("session.close", &close_payload(name), ctx.actor.clone(), ctx.actor_session_id())? {
                closes.push((name.clone(), prepared));
            }
        }
    }
    Ok(closes)
}

/// A copy of the store, taken before a removal deletes a project's board, notes and modules
/// for good; `app.backup.list` lists it. Its path rides on the `*.deleted` event.
fn backup_before(ctx: &Unlocked, reason: &str) -> Result<Option<String>, BusError> {
    let store = &ctx.engine().store;
    if store.path() == Path::new(":memory:") {
        return Ok(None);
    }
    // Not through `ctx.read`: a copy of the whole store under the lock stalled every request for
    // as long as the file took to copy (142-453 ms on a 260 MB store, BASELINE-2026-10-14 §1.1).
    ctx.backup_store(reason).map(|path| Some(path.display().to_string())).map_err(|error| {
        BusError::unavailable("project.backup_failed", format!("backing up the store before the removal: {error}"))
            .with_hint("nothing was removed; free space for the store's backups/ directory and try again")
    })
}

/// `project.remove`'s transaction, also run once per project by `workspace.remove`.
#[allow(clippy::too_many_arguments)]
fn remove_project(ctx: &mut Ctx, project_id: Id, force: bool, remove_worktrees: bool, discard: bool, approved: &[PathBuf], mut closes: Vec<(String, Prepared)>, backup: Option<String>) -> Result<ProjectRemoveOut, BusError> {
    let (pr, open, live_runs) = removal_scope(ctx.tx(), project_id, force)?;
    // `force` closes through `session.close` itself, so every teardown rule holds: scrollback
    // saved, claims released, holds expired, hooks uninstalled. Each close keeps its worktree,
    // which also defers the agent's kill until the store unlocks; deleting checkouts (build
    // purge, `git worktree remove`) is seconds of disk work per agent, so `remove_worktrees`
    // queues it for after the commit too instead of holding the bus through all of it (BUS.md
    // §5.1). A checkout shared by a PAIR or review group is in `open` once per session but
    // removed once, after all of them have closed. A session that opened after the unlocked
    // half ran has nothing prepared and closes the old way, here, and its checkout is kept:
    // only checkouts the unlocked half checked for uncommitted work are deleted (RA-405).
    let repo = PathBuf::from(&pr.path);
    let pool = crate::worktree::pool_dir(&repo);
    let mut doomed: Vec<PathBuf> = if remove_worktrees {
        open.iter().map(|(_, wt)| PathBuf::from(wt)).filter(|wt| wt.starts_with(&pool)).collect()
    } else {
        Vec::new()
    };
    doomed.retain(|wt| approved.contains(wt));
    doomed.sort();
    doomed.dedup();
    // The agents in a checkout about to be deleted are taken out of the registry here, so their
    // `session.close` finds no PTY to kill on a detached thread; the closure below kills them
    // and waits for it before deleting anything, as `session.close` does when it removes its
    // own checkout. Otherwise an agent mid-build writes into a tree being deleted (RA-423).
    let mut writers = Vec::new();
    // The checkouts those agents were writing in, checked again once they have stopped.
    let mut written: Vec<PathBuf> = Vec::new();
    if !doomed.is_empty() {
        let rows: Vec<(Id, String)> = {
            let mut stmt = ctx.tx().prepare_cached("SELECT id, worktree FROM sessions WHERE project_id=?1 AND state!='closed'").bus()?;
            let rows = stmt.query_map([pr.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?.collect::<rusqlite::Result<Vec<_>>>();
            rows.bus()?
        };
        for (id, wt) in rows {
            if doomed.iter().any(|d| d.as_path() == Path::new(&wt)) {
                if let Some(pty) = ctx.engine().take_pty(id) {
                    writers.push(pty);
                    written.push(PathBuf::from(wt));
                }
            }
        }
    }
    for (name, _) in &open {
        let prepared = closes.iter().position(|(n, _)| n == name).map(|i| closes.swap_remove(i).1);
        ctx.invoke_prepared("session.close", close_payload(name), prepared)?;
    }
    if remove_worktrees {
        // Integration checkouts and their branches go too: once the rows below are deleted,
        // nothing else could ever find them again.
        let integrations: Vec<(Id, String)> = {
            let mut stmt = ctx.tx().prepare_cached(
                "SELECT id, worktree FROM integrations WHERE project_id=?1 AND worktree IS NOT NULL AND state!='discarded'",
            ).bus()?;
            let rows = stmt.query_map([pr.id], |r| Ok((r.get(0)?, r.get(1)?))).bus()?.collect::<rusqlite::Result<Vec<_>>>();
            rows.bus()?
        };
        if !doomed.is_empty() || !integrations.is_empty() {
            let project_id = pr.id;
            ctx.after_commit(move |engine| {
                std::thread::scope(|scope| {
                    for pty in &writers {
                        pty.silence_exit();
                        scope.spawn(move || pty.kill(std::time::Duration::from_millis(150)));
                    }
                });
                // The unlocked half checked these while their agents could still write; what
                // one wrote before its kill is found now. The project is gone by then, so such
                // a checkout cannot refuse anything: it is kept, logged and named on the event.
                let kept: Vec<PathBuf> = if discard {
                    Vec::new()
                } else {
                    written.sort();
                    written.dedup();
                    let recheck: Vec<super::session::Checkout> = written.into_iter().map(|wt| (wt, Vec::new())).collect();
                    super::session::dirty_checkouts(&recheck).into_iter().map(|((wt, _), _)| wt.clone()).collect()
                };
                for wt in &doomed {
                    if kept.contains(wt) {
                        tracing::warn!(worktree = %wt.display(), code = "worktree.dirty",
                            "an agent wrote into a removed project's worktree after the dirty check; keeping it");
                        continue;
                    }
                    if let Err(error) = crate::worktree::remove(&repo, wt, true) {
                        tracing::warn!(worktree = %wt.display(), %error, "removing a removed project's worktree");
                    }
                }
                for (id, wt) in &integrations {
                    if let Err(error) = super::integration::remove_checkout(&repo, Path::new(wt), *id) {
                        tracing::warn!(worktree = %wt, %error, "removing a removed project's integration");
                    }
                }
                engine.emit_system("worktree.changed", json!({ "project_id": project_id, "kept_dirty": kept }));
            });
        }
    }
    for run in &live_runs {
        ctx.invoke_registered("device.run.stop", json!({ "run_id": run }))?;
    }

    // Every design, deleted or not, is announced deleted once the project is gone (B67): a
    // board or window showing one closes, as on design.purge. project.restore brings them back
    // with the same ids.
    let designs: Vec<Id> = {
        let mut stmt = ctx.tx().prepare_cached("SELECT id FROM designs WHERE project_id = ?1 ORDER BY id").bus()?;
        let rows = stmt.query_map([pr.id], |r| r.get(0)).bus()?.collect::<rusqlite::Result<Vec<_>>>();
        rows.bus()?
    };
    // `project.remove` forgets Relay metadata only. Audit rows deliberately remain as the
    // immutable history, while every FK-owned row is removed in dependency order.
    for sql in [
        "DELETE FROM message_recipients WHERE message_id IN (SELECT id FROM messages WHERE project_id=?1)",
        "DELETE FROM messages WHERE project_id=?1",
        "DELETE FROM session_scrollback WHERE session_id IN (SELECT id FROM sessions WHERE project_id=?1)",
        "DELETE FROM claims WHERE project_id=?1",
        "DELETE FROM task_sessions WHERE session_id IN (SELECT id FROM sessions WHERE project_id=?1) OR task_id IN (SELECT id FROM tasks WHERE project_id=?1)",
        "DELETE FROM sessions WHERE project_id=?1",
        "DELETE FROM task_commits WHERE task_id IN (SELECT id FROM tasks WHERE project_id=?1)",
        "DELETE FROM attachments WHERE task_id IN (SELECT id FROM tasks WHERE project_id=?1)",
        "DELETE FROM module_unlinked_tasks WHERE module_id IN (SELECT id FROM modules WHERE project_id=?1) OR task_id IN (SELECT id FROM tasks WHERE project_id=?1)",
        "DELETE FROM tasks WHERE project_id=?1",
        // `task_labels` and `task_comments` cascade with the tasks; the vocabulary itself is project-owned.
        "DELETE FROM labels WHERE project_id=?1",
        "DELETE FROM modules WHERE project_id=?1",
        "DELETE FROM notes WHERE project_id=?1",
        "DELETE FROM design_removed WHERE design_id IN (SELECT id FROM designs WHERE project_id=?1)",
        "DELETE FROM design_nodes WHERE design_id IN (SELECT id FROM designs WHERE project_id=?1)",
        "DELETE FROM designs WHERE project_id=?1",
        "DELETE FROM notifications WHERE project_id=?1",
        "DELETE FROM overlaps WHERE project_id=?1",
        "DELETE FROM file_trash WHERE project_id=?1",
        "DELETE FROM integrations WHERE project_id=?1",
        "DELETE FROM ui_layouts WHERE project_id=?1",
        "DELETE FROM device_runs WHERE project_id=?1",
        "DELETE FROM skill_projects WHERE project_id=?1",
        "DELETE FROM plugin_projects WHERE project_id=?1",
    ] {
        ctx.tx().execute(sql, [pr.id]).bus()?;
    }
    // Each key with everything under it: the shell saves its arrangement as a tree, and the
    // native client under its own `native.` key, which ui.rs reads first (RA-419).
    for path in [format!("layout.current.{}", pr.id), format!("native.layout.current.{}", pr.id), format!("guardrails.projects.{}", pr.id)] {
        crate::handlers::settings::delete_under(ctx.tx(), &path)?;
    }
    // A hold no session owns (the user's) would otherwise wait on a project that is gone.
    ctx.tx().execute("UPDATE holds SET state='expired', resolved_at=?1, resolved_by='system' WHERE project_id=?2 AND state='open'",
        params![ctx.now, pr.id]).bus()?;
    ctx.tx().execute("DELETE FROM projects WHERE id = ?1", [pr.id]).bus()?;
    ctx.set_project(pr.id);
    for id in designs {
        ctx.emit("design.deleted", json!({ "id": id, "project_id": pr.id }));
    }
    ctx.emit("project.deleted", json!({ "id": pr.id, "backup": backup }));
    Ok(ProjectRemoveOut { sessions_closed: open.len() as i64, runs_stopped: live_runs.len() as i64 })
}

const NEXT_PROJECT_ID: &str = "SELECT MAX(COALESCE((SELECT MAX(id) FROM projects), 0), COALESCE((SELECT MAX(project_id) FROM audit), 0),
    COALESCE((SELECT MAX(project_id) FROM holds), 0), COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'ids.projects'), 0)) + 1";
const NEXT_WORKSPACE_ID: &str = "SELECT MAX(COALESCE((SELECT MAX(id) FROM workspaces), 0),
    COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'ids.workspaces'), 0)) + 1";

/// The id for a new workspace or project: past every one handed out before, never a removed
/// one's. The tables predate AUTOINCREMENT, so SQLite gives the next row MAX(id)+1 and reuses
/// the id of the newest one once it is removed; whatever still names that id — audit history,
/// a hold, a client's remembered selection — then reads as the new project's. `meta` keeps the
/// high-water mark from here on; the audit and holds cover projects removed before it existed.
fn next_id(tx: &Connection, sql: &str, key: &str) -> Result<Id, BusError> {
    let id: Id = tx.prepare_cached(sql).bus()?.query_row([], |r| r.get(0)).bus()?;
    tx.prepare_cached("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value").bus()?
        .execute(params![key, id.to_string()]).bus()?;
    Ok(id)
}

fn integrations_live(tx: &Connection, project_id: Id) -> Result<i64, BusError> {
    tx.prepare_cached(
        "SELECT COUNT(*) FROM integrations WHERE project_id=?1 AND state IN ('queued','merging','building','deploying')",
    ).bus()?.query_row([project_id], |row| row.get(0)).bus()
}

fn discover_repositories(root: &Path) -> Result<Vec<relay_bus::types::LocalRepo>, BusError> {
    fn walk(path: &Path, depth: usize, out: &mut Vec<relay_bus::types::LocalRepo>) -> std::io::Result<()> {
        if path.join(".git").exists() {
            out.push(relay_bus::types::LocalRepo { path: path.display().to_string(), name: name_of(path) });
            return Ok(());
        }
        if depth >= 4 { return Ok(()); }
        // Only the workspace itself must be readable: one root-owned folder in it (a container
        // volume, lost+found) is skipped rather than hiding every repository beside it (RA-424).
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if depth > 0 => {
                tracing::debug!(path = %path.display(), %error, "skipping an unreadable folder");
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if !kind.is_dir() || kind.is_symlink() { continue; }
            let name = entry.file_name();
            // Build output and caches hold no repository worth offering (RA-638).
            if name.to_str().is_some_and(|name| name == ".git" || crate::watch::generated_name(name)) { continue; }
            if name.to_str().is_some_and(|name| name.starts_with(CLONE_SCRATCH)) { continue; }
            walk(&entry.path(), depth + 1, out)?;
        }
        Ok(())
    }
    let mut repositories = Vec::new();
    walk(root, 0, &mut repositories).map_err(|error| BusError::unavailable("workspace.discover_failed", error.to_string()))?;
    repositories.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(repositories)
}

fn clone_destination(workspace: &str, url: &str, requested: Option<&str>) -> Result<PathBuf, BusError> {
    let name = requested.filter(|value| !value.trim().is_empty()).map(str::trim).map(str::to_string).unwrap_or_else(|| {
        url.trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or("repository").trim_end_matches(".git").to_string()
    });
    let relative = Path::new(&name);
    if name.is_empty() || relative.is_absolute() || relative.components().any(|component| !matches!(component, std::path::Component::Normal(_))) {
        return Err(BusError::invalid("project.clone_dest", "clone destination must be one directory name"));
    }
    Ok(Path::new(workspace).join(relative))
}

/// `main` unless the repo's HEAD says otherwise; no subprocess (SPEC §15: zero idle spawns,
/// and this is a one-off anyway — but a file read is cheaper and enough).
/// The branch the repository's HEAD names, or `main`. HEAD is the repository's to write, and a
/// crafted one can name `refs/heads/--output=/some/path`: the base branch reaches git's command
/// line in several places, where that would read as an option. So a name that is not a
/// [`plausible_branch`] is not taken.
fn detect_default_branch(repo: &Path) -> String {
    let head = std::fs::read_to_string(repo.join(".git").join("HEAD")).unwrap_or_default();
    head.trim().strip_prefix("ref: refs/heads/").filter(|name| plausible_branch(name)).map(str::to_string).unwrap_or_else(|| "main".to_string())
}

/// A branch name git would accept that cannot be read as an option: `git check-ref-format
/// --branch`'s rules, checked in process since `project.add` and `project.update` run under the
/// store lock, plus no leading `-`.
fn plausible_branch(name: &str) -> bool {
    !name.is_empty() && !name.starts_with(['-', '/', '.']) && !name.ends_with(['/', '.']) && !name.ends_with(".lock")
        && !name.contains("..") && !name.contains("//") && !name.contains("@{") && !name.contains("/.") && name != "@"
        && !name.chars().any(|c| c.is_ascii_control() || c == ' ' || "~^:?*[\\".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_naming_an_option_is_not_taken_as_the_base_branch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".git")).unwrap();
        let head = |value: &str| {
            std::fs::write(root.path().join(".git/HEAD"), value).unwrap();
            detect_default_branch(root.path())
        };
        assert_eq!(head("ref: refs/heads/develop\n"), "develop");
        assert_eq!(head("ref: refs/heads/feature/x\n"), "feature/x");
        assert_eq!(head("ref: refs/heads/--output=/tmp/owned\n"), "main");
        assert_eq!(head("ref: refs/heads/a..b\n"), "main");
        assert_eq!(head("ref: refs/heads/has space\n"), "main");
        assert_eq!(head("0123456789abcdef0123456789abcdef01234567\n"), "main");
    }

    #[test]
    fn suggested_workspace_uses_the_parent_of_a_containing_repository() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("relay");
        let deep = repo.join("apps/relay-app/src-tauri");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(suggested_workspace_from(&deep), root.path());
    }

    #[test]
    fn suggested_workspace_keeps_a_non_repository_directory() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("projects");
        fs::create_dir_all(&directory).unwrap();
        assert_eq!(suggested_workspace_from(&directory), directory);
    }

    /// RA-424, RA-649: an unreadable folder in the workspace is skipped, not fatal to the discovery.
    #[test]
    fn discovery_skips_a_folder_it_cannot_read() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("repo/.git")).unwrap();
        let locked = root.path().join("locked");
        fs::create_dir_all(locked.join("inner")).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let found = discover_repositories(root.path());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        let found = found.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, root.path().join("repo").display().to_string());
        assert!(discover_repositories(&root.path().join("missing")).is_err(), "the workspace itself must be readable");
    }
}
