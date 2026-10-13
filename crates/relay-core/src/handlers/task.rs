//! `task.*` — the Phase 7 board, task lifecycle, attachments, and dispatch spine.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::sessions;
use base64::Engine as _;
use relay_bus::error::BusError;
use relay_bus::ops::task::*;
use relay_bus::types::{
    Attachment, Column, Id, Label, Priority, Size, Task, TaskComment, TaskCommit, TaskRelation,
    TaskRollup, TaskState, TaskType, TASK_CHILDREN_MAX, TASK_DEPTH_MAX,
};
use relay_bus::Empty;
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde_json::json;
use std::path::{Path, PathBuf};

pub(crate) fn column_str(value: Column) -> &'static str {
    match value {
        Column::Backlog => "backlog",
        Column::InReview => "in_review",
        Column::Ready => "ready",
        Column::Active => "active",
        Column::Done => "done",
    }
}
pub(crate) fn parse_column(value: &str) -> Column {
    match value {
        "in_review" => Column::InReview,
        "ready" => Column::Ready,
        "active" => Column::Active,
        "done" => Column::Done,
        _ => Column::Backlog,
    }
}
pub(crate) fn priority_str(value: Priority) -> &'static str {
    match value {
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Urgent => "urgent",
    }
}
pub(crate) fn parse_priority(value: &str) -> Priority {
    match value {
        "low" => Priority::Low,
        "high" => Priority::High,
        "urgent" => Priority::Urgent,
        _ => Priority::Medium,
    }
}
fn state_str(value: TaskState) -> &'static str {
    match value {
        TaskState::None => "none",
        TaskState::Dispatched => "dispatched",
        TaskState::Running => "running",
        TaskState::Blocked => "blocked",
        TaskState::Failed => "failed",
        TaskState::AwaitingReview => "awaiting_review",
    }
}
fn parse_task_state(value: &str) -> TaskState {
    match value {
        "dispatched" => TaskState::Dispatched,
        "running" => TaskState::Running,
        "blocked" => TaskState::Blocked,
        "failed" => TaskState::Failed,
        "awaiting_review" => TaskState::AwaitingReview,
        _ => TaskState::None,
    }
}
fn size_str(value: Size) -> &'static str {
    match value {
        Size::S => "S",
        Size::M => "M",
        Size::L => "L",
    }
}
fn parse_size(value: Option<String>) -> Option<Size> {
    match value.as_deref() {
        Some("S") => Some(Size::S),
        Some("M") => Some(Size::M),
        Some("L") => Some(Size::L),
        _ => None,
    }
}

pub(crate) fn type_str(value: TaskType) -> &'static str {
    match value {
        TaskType::Task => "task",
        TaskType::Feature => "feature",
        TaskType::Bug => "bug",
        TaskType::Chore => "chore",
        TaskType::Spike => "spike",
    }
}
fn parse_type(value: &str) -> TaskType {
    match value {
        "feature" => TaskType::Feature,
        "bug" => TaskType::Bug,
        "chore" => TaskType::Chore,
        "spike" => TaskType::Spike,
        _ => TaskType::Task,
    }
}
fn relation_str(value: TaskRelation) -> &'static str {
    match value {
        TaskRelation::BlockedBy => "blocked_by",
        TaskRelation::DuplicateOf => "duplicate_of",
    }
}

/// Labels are stored once per project and referenced; the wire form is the trimmed name.
fn normalise_label(value: &str) -> Result<String, BusError> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(BusError::invalid(
            "task.label",
            "a label is 1-40 characters of non-blank text",
        ));
    }
    Ok(name.to_string())
}

fn label_id(tx: &Transaction, project_id: Id, name: &str, now: &str) -> Result<Id, BusError> {
    if let Some(id) = tx
        .query_row(
            "SELECT id FROM labels WHERE project_id=?1 AND name=?2 COLLATE NOCASE",
            params![project_id, name],
            |r| r.get(0),
        )
        .optional()
        .bus()?
    {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO labels(project_id,name,created_at) VALUES (?1,?2,?3)",
        params![project_id, name, now],
    )
    .bus()?;
    Ok(tx.last_insert_rowid())
}

/// Distance from the root, walking parents. Bounded by `TASK_DEPTH_MAX` so a cycle that
/// somehow reached the store cannot spin here.
fn depth_of(tx: &rusqlite::Connection, task_id: Id) -> rusqlite::Result<i64> {
    let mut depth = 0;
    let mut cursor = task_id;
    while let Some(parent) = tx
        .prepare_cached("SELECT parent_id FROM tasks WHERE id=?1")?
        .query_row([cursor], |r| r.get::<_, Option<Id>>(0))
        .optional()?
        .flatten()
    {
        depth += 1;
        cursor = parent;
        if depth > TASK_DEPTH_MAX {
            break;
        }
    }
    Ok(depth)
}

fn child_ids(tx: &rusqlite::Connection, task_id: Id) -> Result<Vec<Id>, BusError> {
    let mut stmt = tx
        .prepare_cached(
            "SELECT id FROM tasks WHERE parent_id=?1 AND deleted_at IS NULL ORDER BY position,id",
        )
        .bus()?;
    let values = stmt
        .query_map([task_id], |r| r.get(0))
        .bus()?
        .collect::<rusqlite::Result<Vec<Id>>>()
        .bus()?;
    Ok(values)
}

/// The deepest level below `task_id`, so re-parenting a whole subtree can be checked in one go.
/// `assert_parent` refuses cycles on every write path; the visited set is what stops a
/// hand-edited store from blowing the stack inside a request transaction.
fn subtree_height(tx: &Transaction, task_id: Id) -> Result<i64, BusError> {
    fn walk(
        tx: &Transaction,
        id: Id,
        seen: &mut std::collections::HashSet<Id>,
    ) -> Result<i64, BusError> {
        if !seen.insert(id) {
            return Ok(0);
        }
        let mut height = 0;
        for child in child_ids(tx, id)? {
            height = height.max(1 + walk(tx, child, seen)?);
        }
        Ok(height)
    }
    walk(tx, task_id, &mut std::collections::HashSet::new())
}

/// Every descendant id, parents before their own children. Used for roll-up and fan-out.
fn descendants(tx: &rusqlite::Connection, task_id: Id) -> Result<Vec<Id>, BusError> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::from([task_id]);
    let mut frontier = std::collections::VecDeque::from([task_id]);
    while let Some(current) = frontier.pop_front() {
        for child in child_ids(tx, current)? {
            if seen.insert(child) {
                out.push(child);
                frontier.push_back(child);
            }
        }
    }
    Ok(out)
}

fn attachment_row(row: &Row) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        name: row.get("name")?,
        mime: row.get("mime")?,
        bytes: row.get("bytes")?,
        path: row.get("path")?,
        created_at: row.get("created_at")?,
    })
}

/// Attachment `id` of `task_id`: a live one, or with `detached` one `task.detach` soft-deleted.
fn attachment_on(tx: &Transaction, task_id: Id, id: Id, detached: bool) -> Result<Attachment, BusError> {
    let sql = if detached {
        "SELECT * FROM attachments WHERE id=?1 AND task_id=?2 AND deleted_at IS NOT NULL"
    } else {
        "SELECT * FROM attachments WHERE id=?1 AND task_id=?2 AND deleted_at IS NULL"
    };
    tx.prepare_cached(sql)
        .and_then(|mut stmt| stmt.query_row(params![id, task_id], attachment_row).optional())
        .bus()?
        .ok_or_else(|| {
            let what = if detached { "detached attachment" } else { "attachment" };
            BusError::not_found("task.attachment_not_found", format!("no {what} {id} on task {task_id}"))
        })
}

/// The task row's own columns, with every list field empty and depth 0: [`hydrate`] fills them.
pub(crate) fn task_columns(row: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        module_id: row.get("module_id")?,
        // `task.list` selects the name with the row; [`hydrate`] looks up the rest by key.
        // Neither asks whether the module is completed: an archived module still names its
        // cards (RA-495).
        module_name: match row.as_ref().column_index("module_name") {
            Ok(index) => row.get(index)?,
            Err(_) => None,
        },
        title: row.get("title")?,
        body: row.get("body")?,
        changelog: row.get("changelog")?,
        column: parse_column(&row.get::<_, String>("col")?),
        position: row.get("position")?,
        state: parse_task_state(&row.get::<_, String>("state")?),
        priority: parse_priority(&row.get::<_, String>("priority")?),
        size: parse_size(row.get("size")?),
        task_type: parse_type(&row.get::<_, String>("kind")?),
        parent_id: row.get("parent_id")?,
        depth: 0,
        children: Vec::new(),
        rollup: TaskRollup::default(),
        labels: Vec::new(),
        blocked_by: Vec::new(),
        blocks: Vec::new(),
        duplicate_of: None,
        note_id: row.get("note_id")?,
        sessions: Vec::new(),
        commits: Vec::new(),
        attachments: Vec::new(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

/// One row as a whole task. A list hydrates its page at once with [`hydrate`] instead.
pub(crate) fn row_task(tx: &rusqlite::Connection, row: &Row) -> Result<Task, rusqlite::Error> {
    let mut tasks = [task_columns(row)?];
    hydrate(tx, &mut tasks)?;
    let [task] = tasks;
    Ok(task)
}

/// Fill in what lives beside the task rows — sessions, commits, attachments, labels, relations
/// and children — with one statement each for the whole batch, the ids passed as one JSON
/// array, then the roll-up and depth, which walk the tree per task. `task.list` ran each of
/// those per task, seven statements a row (RA-412); `task.get` goes through here with one id,
/// so a task reads the same either way. The ids must be distinct. Every statement is
/// `prepare_cached` (PERF §1.2).
pub(crate) fn hydrate(tx: &rusqlite::Connection, tasks: &mut [Task]) -> rusqlite::Result<()> {
    if tasks.is_empty() {
        return Ok(());
    }
    let at: std::collections::HashMap<Id, usize> = tasks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
    let ids = serde_json::to_string(&tasks.iter().map(|t| t.id).collect::<Vec<_>>())
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    // Each statement returns (task id, value) in the per-task order the field is shown in.
    fn each<T>(tx: &rusqlite::Connection, sql: &str, ids: &str, map: impl FnMut(&Row) -> rusqlite::Result<(Id, T)>) -> rusqlite::Result<Vec<(Id, T)>> {
        let mut stmt = tx.prepare_cached(sql)?;
        let values = stmt.query_map([ids], map)?.collect();
        values
    }
    for (id, name) in each(tx, "SELECT ts.task_id, s.name FROM task_sessions ts JOIN sessions s ON s.id=ts.session_id WHERE ts.task_id IN (SELECT value FROM json_each(?1)) ORDER BY ts.task_id,ts.ord,ts.session_id", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        tasks[at[&id]].sessions.push(name);
    }
    for (id, commit) in each(tx, "SELECT task_id,sha,branch,linked_at FROM task_commits WHERE task_id IN (SELECT value FROM json_each(?1)) ORDER BY task_id,id", &ids, |r| {
        Ok((r.get(0)?, TaskCommit { sha: r.get(1)?, branch: r.get(2)?, linked_at: r.get(3)? }))
    })? {
        tasks[at[&id]].commits.push(commit);
    }
    for (id, attachment) in each(tx, "SELECT * FROM attachments WHERE task_id IN (SELECT value FROM json_each(?1)) AND deleted_at IS NULL ORDER BY task_id,id", &ids, |r| {
        Ok((r.get("task_id")?, attachment_row(r)?))
    })? {
        tasks[at[&id]].attachments.push(attachment);
    }
    for (id, label) in each(tx, "SELECT tl.task_id, l.name FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.task_id IN (SELECT value FROM json_each(?1)) ORDER BY tl.task_id, l.name COLLATE NOCASE", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        tasks[at[&id]].labels.push(label);
    }
    for (id, other) in each(tx, "SELECT r.from_task, r.to_task FROM task_relations r JOIN tasks t ON t.id=r.to_task WHERE r.from_task IN (SELECT value FROM json_each(?1)) AND r.rel='blocked_by' AND t.deleted_at IS NULL ORDER BY r.from_task, r.to_task", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        tasks[at[&id]].blocked_by.push(other);
    }
    for (id, other) in each(tx, "SELECT r.to_task, r.from_task FROM task_relations r JOIN tasks t ON t.id=r.from_task WHERE r.to_task IN (SELECT value FROM json_each(?1)) AND r.rel='blocked_by' AND t.deleted_at IS NULL ORDER BY r.to_task, r.from_task", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        tasks[at[&id]].blocks.push(other);
    }
    for (id, other) in each(tx, "SELECT r.from_task, r.to_task FROM task_relations r JOIN tasks t ON t.id=r.to_task WHERE r.from_task IN (SELECT value FROM json_each(?1)) AND r.rel='duplicate_of' AND t.deleted_at IS NULL ORDER BY r.from_task, r.to_task", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        // The first in id order, as one edge per task should be all there is (RA-418).
        tasks[at[&id]].duplicate_of.get_or_insert(other);
    }
    for (id, child) in each(tx, "SELECT parent_id, id FROM tasks WHERE parent_id IN (SELECT value FROM json_each(?1)) AND deleted_at IS NULL ORDER BY parent_id,position,id", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
        tasks[at[&id]].children.push(child);
    }
    if tasks.iter().any(|t| t.module_name.is_none() && t.module_id.is_some()) {
        for (id, name) in each(tx, "SELECT t.id, m.name FROM tasks t JOIN modules m ON m.id=t.module_id WHERE t.id IN (SELECT value FROM json_each(?1))", &ids, |r| Ok((r.get(0)?, r.get(1)?)))? {
            tasks[at[&id]].module_name.get_or_insert(name);
        }
    }
    for task in tasks.iter_mut() {
        task.rollup = rollup_of(tx, task.id, &task.children)?;
        task.depth = depth_of(tx, task.id)?;
    }
    Ok(())
}

/// Roll-up walks the subtree here rather than in a recursive CTE so it counts exactly the
/// rows `descendants` would fan out to; depth is capped at 3, so the walk is shallow.
fn rollup_of(tx: &rusqlite::Connection, id: Id, children: &[Id]) -> rusqlite::Result<TaskRollup> {
    let mut rollup = TaskRollup::default();
    let mut counted: std::collections::HashSet<Id> = std::collections::HashSet::from([id]);
    let mut frontier = children.to_vec();
    while let Some(current) = frontier.pop() {
        if !counted.insert(current) {
            continue;
        }
        let (col, kids) = {
            let column: String = tx
                .prepare_cached("SELECT col FROM tasks WHERE id=?1")?
                .query_row([current], |r| r.get(0))?;
            let mut stmt = tx.prepare_cached(
                "SELECT id FROM tasks WHERE parent_id=?1 AND deleted_at IS NULL ORDER BY position,id",
            )?;
            let kids = stmt
                .query_map([current], |r| r.get::<_, Id>(0))?
                .collect::<rusqlite::Result<Vec<Id>>>()?;
            (column, kids)
        };
        rollup.total += 1;
        if col == "done" {
            rollup.done += 1;
        }
        frontier.extend(kids);
    }
    Ok(rollup)
}

pub(crate) fn get_task(tx: &rusqlite::Connection, id: Id, include_deleted: bool) -> Result<Task, BusError> {
    let sql = if include_deleted {
        "SELECT * FROM tasks WHERE id=?1"
    } else {
        "SELECT * FROM tasks WHERE id=?1 AND deleted_at IS NULL"
    };
    tx.prepare_cached(sql)
        .and_then(|mut stmt| stmt.query_row([id], |row| row_task(tx, row)).optional())
        .bus()?
        .ok_or_else(|| BusError::not_found("task.not_found", format!("no task {id}")))
}

/// A live task's `project_id` and `parent_id`, without hydrating it: `assert_parent` walks every
/// ancestor, and `get_task` rolled up each one's whole subtree to read two columns (RA-408).
fn parent_link(tx: &rusqlite::Connection, id: Id) -> Result<(Id, Option<Id>), BusError> {
    tx.prepare_cached("SELECT project_id,parent_id FROM tasks WHERE id=?1 AND deleted_at IS NULL")
        .and_then(|mut stmt| stmt.query_row([id], |r| Ok((r.get(0)?, r.get(1)?))).optional())
        .bus()?
        .ok_or_else(|| BusError::not_found("task.not_found", format!("no task {id}")))
}

fn assert_module(tx: &Transaction, project_id: Id, module_id: Option<Id>) -> Result<(), BusError> {
    let Some(module_id) = module_id else {
        return Ok(());
    };
    let found: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM modules WHERE id=?1 AND project_id=?2 AND deleted_at IS NULL)", params![module_id, project_id], |r| r.get(0)).bus()?;
    if !found {
        return Err(BusError::not_found(
            "module.not_found",
            format!("no module {module_id} in project {project_id}"),
        ));
    }
    Ok(())
}

/// A task is made from a live note of its own project, or from none.
fn assert_note(tx: &Transaction, project_id: Id, note_id: Option<Id>) -> Result<(), BusError> {
    let Some(note_id) = note_id else {
        return Ok(());
    };
    let found: bool = tx
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND project_id=?2 AND deleted_at IS NULL)")
        .and_then(|mut stmt| stmt.query_row(params![note_id, project_id], |r| r.get(0)))
        .bus()?;
    if !found {
        return Err(BusError::not_found(
            "notes.not_found",
            format!("no note {note_id} in project {project_id}"),
        ));
    }
    Ok(())
}

/// A parent must exist, live in the same project, and leave room under the depth cap for the
/// subtree being hung off it. Cycles are refused by walking up from the candidate parent.
fn assert_parent(
    tx: &Transaction,
    project_id: Id,
    child: Option<Id>,
    parent_id: Id,
) -> Result<(), BusError> {
    let (parent_project, grandparent) = parent_link(tx, parent_id)?;
    if parent_project != project_id {
        return Err(BusError::conflict(
            "task.parent_project",
            "a sub-task must stay in its parent's project",
        ));
    }
    if Some(parent_id) == child {
        return Err(BusError::conflict(
            "task.parent_cycle",
            "a task cannot be its own parent",
        ));
    }
    if let Some(child) = child {
        let mut cursor = grandparent;
        while let Some(id) = cursor {
            if id == child {
                return Err(BusError::conflict(
                    "task.parent_cycle",
                    "that parent is already a descendant of this task",
                ));
            }
            cursor = parent_link(tx, id)?.1;
        }
    }
    let below = match child {
        Some(child) => subtree_height(tx, child)?,
        None => 0,
    };
    if depth_of(tx, parent_id).bus()? + 1 + below >= TASK_DEPTH_MAX {
        return Err(BusError::conflict(
            "task.depth",
            format!("sub-tasks nest {TASK_DEPTH_MAX} levels deep at most"),
        ));
    }
    let held: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE parent_id=?1 AND deleted_at IS NULL AND id IS NOT ?2",
            params![parent_id, child],
            |r| r.get(0),
        )
        .bus()?;
    if held >= TASK_CHILDREN_MAX {
        return Err(BusError::conflict(
            "task.children_full",
            format!("a task holds {TASK_CHILDREN_MAX} sub-tasks at most"),
        ));
    }
    Ok(())
}

fn set_labels(ctx: &mut Ctx, task: &Task, labels: &[String]) -> Result<(), BusError> {
    ctx.tx()
        .execute("DELETE FROM task_labels WHERE task_id=?1", [task.id])
        .bus()?;
    for raw in labels {
        let name = normalise_label(raw)?;
        let id = label_id(ctx.tx(), task.project_id, &name, &ctx.now)?;
        ctx.tx()
            .execute(
                "INSERT OR IGNORE INTO task_labels(task_id,label_id) VALUES (?1,?2)",
                params![task.id, id],
            )
            .bus()?;
    }
    Ok(())
}

fn emit_task(ctx: &mut Ctx, task: &Task) -> Result<(), BusError> {
    ctx.set_project(task.project_id);
    ctx.emit("task.changed", serde_json::to_value(task).bus()?);
    Ok(())
}

fn next_position(tx: &Transaction, project_id: Id, column: Column) -> Result<i64, BusError> {
    tx.query_row("SELECT COALESCE(MAX(position),-1)+1 FROM tasks WHERE project_id=?1 AND col=?2 AND deleted_at IS NULL", params![project_id, column_str(column)], |r| r.get(0)).bus()
}

/// Where `task` sits among the other live tasks of its column, counting from 0 in board order
/// (`position, id`). Undo records this rather than the raw `position`, which can hold gaps
/// and ties, so moving the task back to that index restores the exact order it left.
fn column_index(tx: &Transaction, task: &Task) -> Result<i64, BusError> {
    tx.prepare_cached("SELECT COUNT(*) FROM tasks WHERE project_id=?1 AND col=?2 AND deleted_at IS NULL AND id<>?3 AND (position<?4 OR (position=?4 AND id<?3))")
        .and_then(|mut stmt| stmt.query_row(params![task.project_id, column_str(task.column), task.id, task.position], |r| r.get(0)))
        .bus()
}

/// Opens slot `index` in `column` for `task_id`: the column's other tasks are renumbered
/// `0..` in board order, skipping `index`, which is returned clamped to the column's length.
fn open_slot(tx: &Transaction, project_id: Id, column: Column, task_id: Id, index: i64) -> Result<i64, BusError> {
    let ids: Vec<Id> = tx
        .prepare_cached("SELECT id FROM tasks WHERE project_id=?1 AND col=?2 AND deleted_at IS NULL AND id<>?3 ORDER BY position,id")
        .and_then(|mut stmt| stmt.query_map(params![project_id, column_str(column), task_id], |r| r.get(0))?.collect())
        .bus()?;
    let index = index.clamp(0, ids.len() as i64);
    let mut renumber = tx.prepare_cached("UPDATE tasks SET position=?1 WHERE id=?2 AND position<>?1").bus()?;
    for (at, id) in ids.iter().enumerate() {
        let at = at as i64;
        renumber.execute(params![if at < index { at } else { at + 1 }, id]).bus()?;
    }
    Ok(index)
}

fn attachment_root(engine: &Engine) -> PathBuf {
    engine
        .store
        .path()
        .parent()
        .map(|p| p.join("attachments"))
        .unwrap_or_else(|| PathBuf::from("attachments"))
}

fn safe_name(name: &str) -> Result<String, BusError> {
    let name = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(BusError::invalid(
            "task.attachment_name",
            "attachment name is invalid",
        ));
    }
    Ok(name.to_string())
}

fn attach_bytes(
    ctx: &mut Ctx,
    task: &Task,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<Attachment, BusError> {
    let name = safe_name(name)?;
    if bytes.is_empty() {
        return Err(BusError::invalid(
            "task.attachment_empty",
            "attachment cannot be empty",
        ));
    }
    ctx.tx().execute("INSERT INTO attachments(task_id,name,mime,bytes,path,created_at) VALUES (?1,?2,?3,?4,'',?5)", params![task.id, name, mime, bytes.len() as i64, ctx.now]).bus()?;
    let id = ctx.tx().last_insert_rowid();
    let dir = attachment_root(ctx.engine()).join(task.id.to_string());
    std::fs::create_dir_all(&dir).bus()?;
    let path = dir.join(format!("{id}-{name}"));
    let recorded = std::fs::write(&path, bytes).bus().and_then(|()| {
        ctx.tx()
            .execute(
                "UPDATE attachments SET path=?1 WHERE id=?2",
                params![path.display().to_string(), id],
            )
            .and_then(|_| ctx.tx().query_row("SELECT * FROM attachments WHERE id=?1", [id], attachment_row))
            .bus()
    });
    if recorded.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    recorded
}

/// An attachment copied into `attachments/.staging` by `task.attach`'s read phase. Removed on
/// drop unless [`attach_staged`] has already renamed it into place.
struct StagedFile(PathBuf);

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct PreparedAttach {
    staged: StagedFile,
    name: String,
    mime: String,
    bytes: u64,
}

/// [`attach_bytes`] for a file already staged: the row, and a rename instead of a write.
fn attach_staged(ctx: &mut Ctx, task: &Task, prepared: PreparedAttach) -> Result<Attachment, BusError> {
    let PreparedAttach { staged, name, mime, bytes } = prepared;
    ctx.tx().execute("INSERT INTO attachments(task_id,name,mime,bytes,path,created_at) VALUES (?1,?2,?3,?4,'',?5)", params![task.id, name, mime, bytes as i64, ctx.now]).bus()?;
    let id = ctx.tx().last_insert_rowid();
    let dir = attachment_root(ctx.engine()).join(task.id.to_string());
    std::fs::create_dir_all(&dir).bus()?;
    let path = dir.join(format!("{id}-{name}"));
    std::fs::rename(&staged.0, &path).bus()?;
    ctx.tx()
        .execute(
            "UPDATE attachments SET path=?1 WHERE id=?2",
            params![path.display().to_string(), id],
        )
        .bus()?;
    ctx.tx()
        .query_row(
            "SELECT * FROM attachments WHERE id=?1",
            [id],
            attachment_row,
        )
        .bus()
}

/// Link `sha` (trimmed) to the task. Returns it when this call added the link, `None` when it
/// was already there: `task.approve` undoes only a link it made itself.
fn link_commit(
    tx: &Transaction,
    task_id: Id,
    sha: &str,
    branch: Option<&str>,
    now: &str,
) -> Result<Option<String>, BusError> {
    let sha = sha.trim();
    if sha.is_empty() {
        return Err(BusError::invalid(
            "task.commit",
            "commit sha cannot be empty",
        ));
    }
    let added = tx.execute(
        "INSERT OR IGNORE INTO task_commits(task_id,sha,branch,linked_at) VALUES (?1,?2,?3,?4)",
        params![task_id, sha, branch, now],
    )
    .bus()?;
    Ok((added > 0).then(|| sha.to_string()))
}

fn assign_session(ctx: &mut Ctx, name: &str, task: &Task) -> Result<Vec<Id>, BusError> {
    let row = sessions::by_name(ctx.tx(), name)?;
    if row.session.project_id != task.project_id {
        return Err(BusError::conflict(
            "task.session_project",
            "task and session belong to different projects",
        ));
    }
    // The task is queued for the session's review group, not for everyone who happens to
    // share its checkout: independent agents on the primary each work their own tasks.
    let ids: Vec<Id> = sessions::review_group(ctx.tx(), &row.session)?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    crate::handlers::session::enqueue(ctx.tx(), task.id, &ids)?;
    let mut newly_current = Vec::new();
    for id in ids {
        let next = crate::handlers::session::next_queued_task(ctx.tx(), id, None)?;
        if let Some((task_id,module_id)) = next {
            let changed = ctx.tx().execute(
                "UPDATE sessions SET task_id=?1,module_id=?2,updated_at=?3 WHERE id=?4
                 AND (task_id IS NULL OR NOT EXISTS(SELECT 1 FROM tasks WHERE id=sessions.task_id AND deleted_at IS NULL AND col!='done'))",
                params![task_id,module_id,ctx.now,id]).bus()?;
            if changed > 0 { newly_current.push(id); }
        }
    }
    Ok(newly_current)
}

/// Re-emit `parent_id` for its roll-up, unless it is gone: a sub-task left under a deleted
/// parent by an older build must still be movable out from under it.
fn emit_live_parent(ctx: &mut Ctx, parent_id: Option<Id>) -> Result<(), BusError> {
    let Some(parent_id) = parent_id else { return Ok(()) };
    let parent = get_task(ctx.tx(), parent_id, true)?;
    if parent.deleted_at.is_none() {
        emit_task(ctx, &parent)?;
    }
    Ok(())
}

/// The sub-tasks soft-deleted together with `task_id`, by their shared `deleted_at` stamp.
fn deleted_with(tx: &rusqlite::Connection, task_id: Id, stamp: &str) -> Result<Vec<Id>, BusError> {
    let mut stmt = tx.prepare_cached("SELECT id FROM tasks WHERE parent_id=?1 AND deleted_at=?2 ORDER BY position,id").bus()?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::from([task_id]);
    let mut frontier = std::collections::VecDeque::from([task_id]);
    while let Some(current) = frontier.pop_front() {
        let children = stmt.query_map(params![current, stamp], |r| r.get::<_, Id>(0)).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()?;
        for child in children {
            if seen.insert(child) {
                out.push(child);
                frontier.push_back(child);
            }
        }
    }
    Ok(out)
}

/// Bump `updated_at` for edits that live in a side table (labels, relations) so the row the
/// UI re-reads is genuinely newer and `set_undo`'s guard has something to compare.
fn touch(ctx: &mut Ctx, task_id: Id) -> Result<(), BusError> {
    ctx.tx()
        .execute(
            "UPDATE tasks SET updated_at=?1 WHERE id=?2",
            params![ctx.now, task_id],
        )
        .bus()?;
    Ok(())
}

/// The other end of a relation must be a live task in the same project, and not the task itself.
fn assert_relatable(tx: &Transaction, task: &Task, other_id: Id) -> Result<Id, BusError> {
    if other_id == task.id {
        return Err(BusError::conflict(
            "task.relation_self",
            "a task cannot relate to itself",
        ));
    }
    let other = get_task(tx, other_id, false)?;
    if other.project_id != task.project_id {
        return Err(BusError::conflict(
            "task.relation_project",
            "related tasks must share a project",
        ));
    }
    Ok(other.id)
}

/// One task → one existing session (BUS.md §10.5). `task.dispatch` calls this once for the task
/// it was given and once more per not-done descendant when `fanout` is set. Returns the launch
/// the session still needs (`session.spawn`, `wake` or `resume`), which the caller starts after
/// the commit: a launch shells out and writes files, and the brief it writes must already see
/// this assignment.
fn dispatch_task(
    ctx: &mut Ctx,
    task_id: Id,
    name: &str,
    start: bool,
) -> Result<(Task, relay_bus::types::Session, Option<&'static str>), BusError> {
    let before = get_task(ctx.tx(), task_id, false)?;
    if before.column == Column::Done {
        return Err(BusError::conflict(
            "task.column_transition",
            "done tasks cannot be dispatched",
        ));
    }
    if before.column == Column::InReview {
        ctx.tx().execute("UPDATE task_sessions SET completed_at=NULL WHERE task_id=?1",[before.id]).bus()?;
    }
    ctx.tx().execute("UPDATE tasks SET col='active',position=?1,state='dispatched',updated_at=?2 WHERE id=?3",params![next_position(ctx.tx(),before.project_id,Column::Active)?,ctx.now,before.id]).bus()?;
    let active = get_task(ctx.tx(), before.id, false)?;
    let newly_current = assign_session(ctx, name, &active)?;
    let row = sessions::by_name(ctx.tx(), name)?;
    let launch_op = launch_op(name, row.session.state)?;
    if start {
        for id in newly_current {
            if let Some(assigned) = sessions::by_id(ctx.tx(), id)? {
                if matches!(assigned.session.state, relay_bus::types::SessionState::Idle | relay_bus::types::SessionState::Running | relay_bus::types::SessionState::Blocked) {
                    if let Some(task_id) = assigned.session.task_id { crate::handlers::session::announce_assignment(ctx, &assigned.session, task_id, false)?; }
                }
            }
        }
    }
    let task = get_task(ctx.tx(), active.id, false)?;
    let session = sessions::by_name(ctx.tx(), name)?.session;
    ctx.set_project(task.project_id);
    ctx.emit("session.changed", serde_json::to_value(&session).bus()?);
    emit_task(ctx, &task)?;
    Ok((task, session, if start { launch_op } else { None }))
}

/// The launch a session in `state` needs to start working, or a refusal when it cannot.
fn launch_op(name: &str, state: relay_bus::types::SessionState) -> Result<Option<&'static str>, BusError> {
    use relay_bus::types::SessionState as S;
    match state {
        S::Created => Ok(Some("session.spawn")),
        S::Parked => Ok(Some("session.wake")),
        S::Restorable | S::Exited => Ok(Some("session.resume")),
        S::Running | S::Idle | S::Blocked => Ok(None),
        _ => Err(BusError::conflict(
            "task.session_state",
            format!("session {name} cannot be dispatched while {}", sessions::state_str(state)),
        )),
    }
}

/// What `task.dispatch` settles before the transaction opens: the not-done descendants it will
/// fan out to, and the sessions it created for them (each by its own `session.create` request,
/// so the fetch and checkout never run under the store lock).
struct PreparedDispatch {
    children: Vec<Id>,
    created: Vec<String>,
}

/// Close sessions this dispatch created when it cannot go on; their checkouts go with them.
/// Nobody worked in them yet, so whatever a setup step left there is not work to keep (RA-405).
fn discard_created(engine: &Engine, created: &[String]) {
    for name in created {
        let closed = engine.dispatch(
            relay_bus::Request::new(relay_bus::Actor::User, "session.close", json!({"session": name, "remove_worktree": true, "discard_changes": true})),
            crate::engine::Door::InProcess,
        );
        if let Some(error) = closed.error {
            tracing::warn!(session = %name, code = %error.code, "closing a session an abandoned dispatch created");
        }
    }
}

/// The audit half of `task.activity`. It used to test every row the project ever audited with
/// up to three `json_extract`s — the result one for every op — under the store lock, each time
/// a task was opened (RA-188). Now `audit_op_ts` seeks each task op from just before the task
/// was made, and only those rows are parsed. `+project_id` keeps the planner off
/// `audit_project_ts`, which would walk everything the project ran in that window.
fn activity_sql() -> &'static str {
    static SQL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SQL.get_or_init(|| {
        let ops = relay_bus::registry::Registry::global()
            .entries()
            .iter()
            // `task.comment` is read back whole as `comments`; listing it here too reported
            // every comment twice.
            .filter(|op| op.name.starts_with("task.") && op.name != "task.comment" && matches!(op.meta.kind, relay_bus::registry::OpKind::Mutation))
            .map(|op| format!("'{}'", op.name))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "SELECT id FROM audit WHERE op IN ({ops}) AND ts >= ?5 AND +project_id = ?1 AND (?3 IS NULL OR id < ?3)
             AND (json_extract(payload, '$.task_id') = ?2
               OR (op = 'task.create' AND json_extract(result_summary, '$.id') = ?2)
               OR (op = 'task.dispatch' AND json_extract(result_summary, '$.task.id') = ?2))
             ORDER BY id DESC LIMIT ?4"
        )
    })
}

/// The longest comment anyone may post, as for a mailbox message.
const COMMENT_MAX: usize = 64 * 1024;

fn comment_row(row: &Row) -> rusqlite::Result<TaskComment> {
    Ok(TaskComment {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        author: row.get("author")?,
        body: row.get("body")?,
        created_at: row.get("created_at")?,
    })
}

/// `task.list` page size when the caller names none, and the most it may ask for.
const TASK_PAGE: u32 = 1000;
const TASK_PAGE_MAX: u32 = 2000;

pub fn register(e: &mut Engine) {
    e.register::<Create>(|ctx: &mut Ctx, p| {
        crate::handlers::workspace::get_project(ctx.tx(), p.project_id)?;
        let title = p.title.trim();
        if title.is_empty() { return Err(BusError::invalid("task.title", "task title cannot be empty")) }
        assert_module(ctx.tx(), p.project_id, p.module_id)?;
        if let Some(parent_id) = p.parent_id { assert_parent(ctx.tx(), p.project_id, None, parent_id)?; }
        assert_note(ctx.tx(), p.project_id, p.note_id)?;
        let column = p.column.unwrap_or(Column::Backlog);
        // An agent files work; it does not start or finish it. Active means a dispatched
        // session and done a linked commit, and task.move refuses both to an agent — creating
        // straight into them was the way around that. A person keeps the escape hatch (RA-410).
        if ctx.actor.is_agent() && (matches!(column, Column::Active | Column::Done) || p.state.is_some_and(|s| s != TaskState::None)) {
            return Err(BusError::conflict("task.column_transition", "agents create tasks in backlog, ready or in_review, with no run state"));
        }
        let position = next_position(ctx.tx(), p.project_id, column)?;
        ctx.tx().execute(
            "INSERT INTO tasks(project_id,module_id,title,body,changelog,col,position,state,priority,size,kind,parent_id,created_at,updated_at,note_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,?14)",
            params![p.project_id,p.module_id,title,p.body.unwrap_or_default(),p.changelog.unwrap_or_default(),column_str(column),position,state_str(p.state.unwrap_or(TaskState::None)),priority_str(p.priority.unwrap_or(Priority::Medium)),p.size.map(size_str),type_str(p.task_type.unwrap_or(TaskType::Task)),p.parent_id,ctx.now,p.note_id],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        let mut task = get_task(ctx.tx(), id, false)?;
        if let Some(labels) = p.labels.as_ref() { set_labels(ctx, &task, labels)?; }
        // The files are written before the transaction commits: one this request wrote is
        // removed again when a later attachment refuses and the rows roll back (RA-409).
        let mut written = Vec::new();
        for input in p.attachments.unwrap_or_default() {
            let stored = base64::engine::general_purpose::STANDARD.decode(&input.bytes_b64)
                .map_err(|_| BusError::invalid("task.attachment_base64", "attachment bytes_b64 is invalid"))
                .and_then(|bytes| attach_bytes(ctx, &task, &input.name, &input.mime, &bytes));
            match stored {
                Ok(attachment) => written.push(PathBuf::from(attachment.path)),
                Err(error) => {
                    for path in &written { let _ = std::fs::remove_file(path); }
                    return Err(error);
                }
            }
        }
        task = get_task(ctx.tx(), id, false)?;
        ctx.set_undo("task.delete", json!({"task_id": id}), Some(json!({"updated_at": task.updated_at})));
        emit_task(ctx, &task)?;
        Ok(task)
    });

    e.register::<Activity>(|ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let limit = p.limit.unwrap_or(100).clamp(1, 500) as usize;
        // Nothing is audited about a task before it exists. A day of slack absorbs a clock
        // step; an unparsable stamp (an imported row) just loses the bound.
        let since = task.created_at.parse::<jiff::Timestamp>().ok()
            .and_then(|at| at.checked_sub(jiff::SignedDuration::from_hours(24)).ok())
            .map(|at| at.to_string())
            .unwrap_or_default();
        let mut stmt = ctx.tx().prepare_cached(activity_sql()).bus()?;
        let ids = stmt
            .query_map(
                params![task.project_id, task.id, p.before_audit, (limit + 1) as i64, since],
                |row| row.get::<_, i64>(0),
            )
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        let next_audit = (ids.len() > limit).then(|| ids[limit - 1]);
        let history = ids
            .into_iter()
            .take(limit)
            .map(|id| {
                crate::audit::get(ctx.tx(), id).bus().and_then(|row| {
                    row.ok_or_else(|| BusError::internal("task audit row disappeared"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut stmt = ctx
            .tx()
            .prepare_cached(
                "SELECT * FROM messages WHERE project_id = ?1 AND re_task = ?2
             AND (?3 IS NULL OR id < ?3) ORDER BY id DESC LIMIT ?4",
            )
            .bus()?;
        let mut messages = stmt
            .query_map(
                params![
                    task.project_id,
                    task.id,
                    p.before_message,
                    (limit + 1) as i64
                ],
                |row| {
                    Ok(relay_bus::types::Message {
                        id: row.get("id")?,
                        project_id: row.get("project_id")?,
                        from: row.get("from_session")?,
                        to: row.get("to_spec")?,
                        text: row.get("text")?,
                        re_task: row.get("re_task")?,
                        priority: row.get::<_, i64>("priority")? != 0,
                        sent_at: row.get("sent_at")?,
                        acked_at: None,
                    })
                },
            )
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        let next_message = (messages.len() > limit).then(|| messages[limit - 1].id);
        messages.truncate(limit);
        let comments = ctx
            .tx()
            .prepare_cached("SELECT * FROM task_comments WHERE task_id = ?1 ORDER BY id")
            .and_then(|mut stmt| stmt.query_map([task.id], comment_row)?.collect::<rusqlite::Result<Vec<_>>>())
            .bus()?;
        Ok(ActivityOut {
            history,
            messages,
            next_audit,
            next_message,
            comments,
        })
    });
    // Append-only: a comment is not edited or undone, and the task's `updated_at` stays put so
    // a comment never trips another writer's `expected_updated_at`.
    e.register::<Comment>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        if p.body.trim().is_empty() { return Err(BusError::invalid("task.comment_empty", "comment cannot be empty")) }
        if p.body.len() > COMMENT_MAX {
            return Err(BusError::invalid("task.comment_size", format!("comment is {} bytes; the limit is {COMMENT_MAX}", p.body.len()))
                .with_hint("put long material in a note or an attachment and link it"));
        }
        let author = crate::handlers::notes::actor_name(ctx)?;
        ctx.set_project(task.project_id);
        ctx.tx()
            .prepare_cached("INSERT INTO task_comments(task_id,author,body,created_at) VALUES (?1,?2,?3,?4) RETURNING *")
            .and_then(|mut stmt| stmt.query_row(params![task.id, author, p.body, ctx.now], comment_row))
            .bus()
    });
    e.register::<Get>(|ctx, p| get_task(ctx.tx(), p.task_id, false));

    e.register::<List>(|ctx, mut p| {
        // An agent reads its own project's board, as it does its notes and peers (D106): it is
        // refused a task of the next project by task.get, and task.list handed it over (RA-411).
        if let Some(own) = crate::handlers::notes::actor_project(ctx)? {
            if p.project_id.is_some_and(|id| id != own) { return Err(BusError::not_own("project")) }
            p.project_id = Some(own);
        }
        if let Some(project_id) = p.project_id { crate::handlers::workspace::get_project(ctx.tx(), project_id)?; }
        if let Some(sort) = p.sort.as_deref() {
            if !matches!(sort, "column" | "priority" | "updated") { return Err(BusError::invalid("task.sort", "sort must be column, priority, or updated")) }
        }
        // The module's name rides along, completed modules included, so the board needs no
        // second `module.list` to label cards (RA-495). A keyed subquery is the LEFT JOIN on
        // `modules.id` without making every filter below name its table.
        let mut sql = String::from("SELECT *, (SELECT m.name FROM modules m WHERE m.id=tasks.module_id) AS module_name FROM tasks WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if !p.include_deleted.unwrap_or(false) { sql.push_str(" AND deleted_at IS NULL"); }
        if let Some(v)=p.project_id { sql.push_str(" AND project_id=?"); args.push(Box::new(v)); }
        if let Some(v)=p.column { sql.push_str(" AND col=?"); args.push(Box::new(column_str(v).to_string())); }
        if let Some(v)=p.state { sql.push_str(" AND state=?"); args.push(Box::new(state_str(v).to_string())); }
        if let Some(v)=p.module_id { sql.push_str(" AND module_id=?"); args.push(Box::new(v)); }
        if let Some(v)=p.priority { sql.push_str(" AND priority=?"); args.push(Box::new(priority_str(v).to_string())); }
        if let Some(v)=p.task_type { sql.push_str(" AND kind=?"); args.push(Box::new(type_str(v).to_string())); }
        match p.parent_id {
            Some(Some(v)) => { sql.push_str(" AND parent_id=?"); args.push(Box::new(v)); }
            Some(None) => sql.push_str(" AND parent_id IS NULL"),
            None => {}
        }
        if let Some(v)=p.label { sql.push_str(" AND EXISTS(SELECT 1 FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.task_id=tasks.id AND l.name=? COLLATE NOCASE)"); args.push(Box::new(normalise_label(&v)?)); }
        if let Some(v)=p.note_id { sql.push_str(" AND note_id=?"); args.push(Box::new(v)); }
        if let Some(v)=p.session { sql.push_str(" AND EXISTS(SELECT 1 FROM task_sessions ts WHERE ts.task_id=tasks.id AND ts.session_id=(SELECT MAX(id) FROM sessions WHERE name=?))"); args.push(Box::new(v)); }
        match p.sort.as_deref().unwrap_or("column") {
            "priority" => sql.push_str(" ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END,updated_at DESC,id"),
            "updated" => sql.push_str(" ORDER BY updated_at DESC,id DESC"),
            _ => sql.push_str(" ORDER BY CASE col WHEN 'backlog' THEN 0 WHEN 'in_review' THEN 1 WHEN 'ready' THEN 2 WHEN 'active' THEN 3 ELSE 4 END,position,id"),
        }
        // The reply is bounded: every task ever made, done ones included, each with its body,
        // changelog and roll-up, grew past what a client accepts on one line (RA-027).
        let limit = p.limit.unwrap_or(TASK_PAGE).clamp(1, TASK_PAGE_MAX);
        let offset = p.offset.unwrap_or(0);
        sql.push_str(" LIMIT ? OFFSET ?");
        args.push(Box::new(limit as i64 + 1));
        args.push(Box::new(offset as i64));
        // A dozen filter shapes at most, and the store's statement cache holds 128: the built
        // statement is compiled once per shape, not once per call.
        let mut stmt=ctx.tx().prepare_cached(&sql).bus()?;
        let mut rows=stmt.query(rusqlite::params_from_iter(args.iter().map(|v| v.as_ref()))).bus()?;
        let mut tasks=Vec::new();
        while let Some(row)=rows.next().bus()? { tasks.push(task_columns(row).bus()?); }
        drop(rows);
        let next_offset = (tasks.len() > limit as usize).then_some(offset + limit);
        tasks.truncate(limit as usize);
        // The page's side tables in one statement each, not seven per task (RA-412).
        hydrate(ctx.tx(), &mut tasks).bus()?;
        if p.summary.unwrap_or(false) {
            for task in &mut tasks {
                task.body.clear();
                task.changelog.clear();
            }
        }
        Ok(ListOut { tasks, next_offset })
    });

    e.register::<Update>(|ctx: &mut Ctx, p| {
        let before=get_task(ctx.tx(),p.task_id,false)?;
        if ctx.actor.is_agent() && (p.title.is_some() || p.priority.is_some() || p.size.is_some() || p.module_id.is_some() || p.state.is_some() || p.task_type.is_some()) {
            return Err(BusError::refused("actor.allowlist", "agents may update only their task body and changelog"));
        }
        if let Some(expected) = &p.expected {
            let current = serde_json::to_value(&before).bus()?;
            for (field, value) in expected {
                if !["title", "body", "priority", "size", "module_id", "state", "changelog", "type"].contains(&field.as_str()) {
                    return Err(BusError::invalid("task.expected_field", format!("{field} is not an editable task field")));
                }
                if current.get(field) != Some(value) {
                    return Err(BusError::conflict("task.edit_conflict", format!("Task {field} changed elsewhere; your draft was not saved")));
                }
            }
        }
        let title=p.title.as_deref().unwrap_or(&before.title).trim();
        if title.is_empty() { return Err(BusError::invalid("task.title", "task title cannot be empty")) }
        let module_id=p.module_id.unwrap_or(before.module_id);
        assert_module(ctx.tx(),before.project_id,module_id)?;
        let size=p.size.unwrap_or(before.size);
        ctx.tx().execute("UPDATE tasks SET title=?1,body=?2,priority=?3,size=?4,module_id=?5,state=?6,changelog=?7,kind=?8,updated_at=?9 WHERE id=?10",
            params![title,p.body.as_deref().unwrap_or(&before.body),priority_str(p.priority.unwrap_or(before.priority)),size.map(size_str),module_id,state_str(p.state.unwrap_or(before.state)),p.changelog.as_deref().unwrap_or(&before.changelog),type_str(p.task_type.unwrap_or(before.task_type)),ctx.now,before.id]).bus()?;
        let task=get_task(ctx.tx(),before.id,false)?;
        ctx.set_undo("task.update",json!({"task_id":before.id,"title":before.title,"body":before.body,"priority":priority_str(before.priority),"size":before.size.map(size_str),"module_id":before.module_id,"state":state_str(before.state),"changelog":before.changelog,"type":type_str(before.task_type)}),Some(json!({"updated_at":task.updated_at})));
        emit_task(ctx,&task)?; Ok(task)
    });

    e.register::<Move>(|ctx: &mut Ctx,p| {
        let before=get_task(ctx.tx(),p.task_id,false)?;
        if ctx.actor.is_agent() && !(before.column==Column::Active && p.column==Column::InReview) { return Err(BusError::conflict("task.column_transition","agents may only move their active task to in_review")) }
        // Done means a commit is linked, and task.approve is what links one. A task that already
        // has one — it left Done, and this is that move undone — may go straight back; refusing
        // it made every move out of Done un-undoable and jammed the board's Ctrl+Z (RA-189).
        if p.column==Column::Done && before.column!=Column::Done && before.commits.is_empty() { return Err(BusError::conflict("task.column_transition","move to done through task.approve")) }
        // An explicit position is the index the task ends at in its column; the others shift
        // around it, so a card can be dropped between two others. Without one it goes last.
        let index=column_index(ctx.tx(),&before)?;
        let position=match p.position { Some(at) => open_slot(ctx.tx(),before.project_id,p.column,before.id,at)?, None => next_position(ctx.tx(),before.project_id,p.column)? };
        let moved_state = match (before.column, p.column) {
            (Column::Done, Column::InReview) => TaskState::AwaitingReview,
            (from, Column::Done) if from != Column::Done => TaskState::None,
            _ => before.state,
        };
        ctx.tx().execute("UPDATE tasks SET col=?1,position=?2,state=?3,updated_at=?4 WHERE id=?5",params![column_str(p.column),position,state_str(moved_state),ctx.now,before.id]).bus()?;
        let task=get_task(ctx.tx(),before.id,false)?;
        ctx.set_undo("task.move",json!({"task_id":before.id,"column":column_str(before.column),"position":index}),Some(json!({"updated_at":task.updated_at})));
        emit_task(ctx,&task)?; Ok(task)
    });

    // A sub-task goes with its parent. Deleting the parent alone left its live sub-tasks
    // hanging off a row nothing can read — every task.parent.set on them then failed — and a
    // restore brought hidden subtrees back past the depth and children caps (RA-191).
    e.register::<Delete>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let subtree = descendants(ctx.tx(), task.id)?;
        {
            let mut delete = ctx.tx().prepare_cached("UPDATE tasks SET deleted_at=?1,updated_at=?1 WHERE id=?2").bus()?;
            for id in std::iter::once(task.id).chain(subtree.iter().copied()) {
                delete.execute(params![ctx.now, id]).bus()?;
            }
        }
        ctx.set_project(task.project_id);
        ctx.set_undo(
            "task.restore",
            json!({"task_id":task.id}),
            Some(json!({"updated_at":ctx.now})),
        );
        for id in std::iter::once(task.id).chain(subtree) {
            ctx.emit(
                "task.deleted",
                json!({"id":id,"project_id":task.project_id}),
            );
        }
        emit_live_parent(ctx, task.parent_id)?;
        Ok(Empty {})
    });
    e.register::<Restore>(|ctx: &mut Ctx, p| {
        let before = get_task(ctx.tx(), p.task_id, true)?;
        let Some(stamp) = before.deleted_at.clone() else {
            return Err(BusError::conflict(
                "task.not_deleted",
                format!("task {} is not deleted", before.id),
            ));
        };
        // The sub-tasks deleted with it come back with it; one deleted on its own earlier
        // stays deleted.
        let subtree = deleted_with(ctx.tx(), before.id, &stamp)?;
        {
            let mut restore = ctx.tx().prepare_cached("UPDATE tasks SET deleted_at=NULL,updated_at=?1 WHERE id=?2").bus()?;
            for id in std::iter::once(before.id).chain(subtree.iter().copied()) {
                restore.execute(params![ctx.now, id]).bus()?;
            }
        }
        // A parent that was deleted since, or filled up or deepened while this subtree was
        // away, cannot take it back: it returns as a top-level task rather than not at all,
        // so the undo that called this always lands.
        if let Some(parent_id) = before.parent_id {
            if let Err(error) = assert_parent(ctx.tx(), before.project_id, Some(before.id), parent_id) {
                if error.kind == relay_bus::ErrorKind::Internal {
                    return Err(error);
                }
                ctx.tx().execute("UPDATE tasks SET parent_id=NULL WHERE id=?1", [before.id]).bus()?;
            }
        }
        let task = get_task(ctx.tx(), before.id, false)?;
        ctx.set_undo(
            "task.delete",
            json!({"task_id":task.id}),
            Some(json!({"updated_at":task.updated_at})),
        );
        for id in subtree {
            let child = get_task(ctx.tx(), id, false)?;
            emit_task(ctx, &child)?;
        }
        emit_task(ctx, &task)?;
        emit_live_parent(ctx, task.parent_id)?;
        Ok(task)
    });
    e.register::<LinkCommit>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        link_commit(ctx.tx(), task.id, &p.sha, p.branch.as_deref(), &ctx.now)?;
        ctx.tx()
            .execute(
                "UPDATE tasks SET updated_at=?1 WHERE id=?2",
                params![ctx.now, task.id],
            )
            .bus()?;
        let task = get_task(ctx.tx(), task.id, false)?;
        emit_task(ctx, &task)?;
        Ok(task)
    });
    e.register::<ChangelogWrite>(|ctx: &mut Ctx, p| {
        let before = get_task(ctx.tx(), p.task_id, false)?;
        // Optional, for an agent that read the task and wants its write refused rather than
        // landing over an edit made since (RA-413). Nothing tracks what it read; it says.
        if p.expected_updated_at.as_ref().is_some_and(|at| *at != before.updated_at) {
            return Err(BusError::conflict("task.edit_conflict", "Task changed elsewhere since you read it; your changelog was not saved")
                .with_details(json!({"expected": p.expected_updated_at, "actual": before.updated_at})));
        }
        ctx.tx()
            .execute(
                "UPDATE tasks SET changelog=?1,updated_at=?2 WHERE id=?3",
                params![p.text, ctx.now, before.id],
            )
            .bus()?;
        let task = get_task(ctx.tx(), before.id, false)?;
        ctx.set_undo(
            "task.changelog.write",
            json!({"task_id":before.id,"text":before.changelog}),
            Some(json!({"updated_at":task.updated_at})),
        );
        emit_task(ctx, &task)?;
        Ok(task)
    });
    // Staged (D149, RA-190): the file is read and copied into the attachment store with the
    // lock released — it can be any size — and the transaction only records it and renames the
    // copy into place.
    e.register_staged::<Attach, PreparedAttach>(|ctx, p| {
        ctx.read(|conn| get_task(conn, p.task_id, false))?;
        let staging = attachment_root(ctx.engine()).join(".staging");
        std::fs::create_dir_all(&staging).bus()?;
        let staged = StagedFile(staging.join(uuid::Uuid::new_v4().to_string()));
        let (name, mime, bytes) = match (&p.path, &p.name, &p.mime, &p.bytes_b64) {
            // A path may carry the name and type to store it under; without them the file's
            // own name and a generic type are used (RA-414). Undoing a task.detach older than
            // soft-detach (audit rows from before v24) still comes through here.
            (Some(path), name, mime, None) => {
                let src = PathBuf::from(path);
                if !src.is_file() {
                    return Err(BusError::not_found(
                        "task.attachment_path",
                        format!("no file {path}"),
                    ));
                }
                let name = name.clone().unwrap_or_else(|| src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("attachment")
                    .to_string());
                let bytes = std::fs::copy(&src, &staged.0).bus()?;
                (name, mime.clone().unwrap_or_else(|| "application/octet-stream".to_string()), bytes)
            }
            (None, Some(name), Some(mime), Some(encoded)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|_| {
                        BusError::invalid(
                            "task.attachment_base64",
                            "attachment bytes_b64 is invalid",
                        )
                    })?;
                std::fs::write(&staged.0, &bytes).bus()?;
                (name.clone(), mime.clone(), bytes.len() as u64)
            }
            _ => {
                return Err(BusError::invalid(
                    "task.attachment_input",
                    "provide either path (optionally with name and mime) or name + mime + bytes_b64",
                ))
            }
        };
        let name = safe_name(&name)?;
        if bytes == 0 {
            return Err(BusError::invalid(
                "task.attachment_empty",
                "attachment cannot be empty",
            ));
        }
        Ok(PreparedAttach { staged, name, mime, bytes })
    }, |ctx: &mut Ctx, p, prepared| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let attachment = attach_staged(ctx, &task, prepared)?;
        emit_task(ctx, &get_task(ctx.tx(), task.id, false)?)?;
        Ok(attachment)
    });
    // Soft (RA-414): the row and its file stay until the retention pass, so the undo restores
    // the same id and file instead of copying the file in again under a new one.
    e.register::<Detach>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let attachment = attachment_on(ctx.tx(), task.id, p.attachment_id, false)?;
        ctx.tx()
            .execute("UPDATE attachments SET deleted_at=?1 WHERE id=?2", params![ctx.now, attachment.id])
            .bus()?;
        ctx.set_undo(
            "task.attachment.restore",
            json!({"task_id":task.id,"attachment_id":attachment.id}),
            None,
        );
        emit_task(ctx, &get_task(ctx.tx(), task.id, false)?)?;
        Ok(Empty {})
    });
    e.register::<AttachmentRestore>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let attachment = attachment_on(ctx.tx(), task.id, p.attachment_id, true)?;
        ctx.tx()
            .execute("UPDATE attachments SET deleted_at=NULL WHERE id=?1", [attachment.id])
            .bus()?;
        ctx.set_undo(
            "task.detach",
            json!({"task_id":task.id,"attachment_id":attachment.id}),
            None,
        );
        emit_task(ctx, &get_task(ctx.tx(), task.id, false)?)?;
        Ok(attachment)
    });
    // Staged (D149): creating a session fetches and checks out, and launching one installs
    // hooks and writes files. Both run as their own requests with the store unlocked — the
    // creates before the transaction, the launches after it, once the assignment they brief
    // the agent about is committed. Only the task and assignment writes hold the lock.
    e.register_staged::<Dispatch, PreparedDispatch>(|ctx, p| {
        if p.session.is_some() == p.create.is_some() {
            return Err(BusError::invalid(
                "task.dispatch_target",
                "provide exactly one of session or create",
            ));
        }
        let fanout = p.fanout.unwrap_or(false);
        let start = p.start.unwrap_or(true);
        if fanout && p.create.is_none() {
            return Err(BusError::invalid(
                "task.fanout_target",
                "fanout needs create: one session cannot hold several tasks",
            ));
        }
        let children = ctx.read(|conn| {
            let task = get_task(conn, p.task_id, false)?;
            if task.column == Column::Done {
                return Err(BusError::conflict("task.column_transition", "done tasks cannot be dispatched"));
            }
            if let Some(create) = &p.create {
                if create.project_id != task.project_id {
                    return Err(BusError::invalid("task.dispatch_project", "created session must use the task project"));
                }
                // A launch that fails after the commit cannot answer this request; the one
                // refusal people actually hit is a missing provider, so it is checked here.
                if start { crate::providers::executable(conn, create.provider)?; }
            }
            if let (Some(name), true) = (&p.session, start) {
                let row = sessions::by_name(conn, name)?;
                if launch_op(name, row.session.state)?.is_some() {
                    crate::providers::executable(conn, row.session.provider)?;
                }
            }
            if !fanout { return Ok(Vec::new()); }
            let mut children = Vec::new();
            for id in descendants(conn, p.task_id)? {
                if get_task(conn, id, false)?.column != Column::Done { children.push(id); }
            }
            Ok(children)
        })?;
        let mut created = Vec::new();
        if let Some(template) = &p.create {
            for at in 0..=children.len() {
                let mut create = template.clone();
                create.task_id = None;
                // The template is whole only for the task dispatched by name. Its branch is
                // checked out once already, its prompt is that task's assignment and its pair
                // partner pairs with one session: a fanned sub-task gets none of them (RA-415).
                if at > 0 {
                    create.branch = None;
                    create.prompt = None;
                    create.pair_with = None;
                }
                let out = ctx.engine().dispatch(
                    relay_bus::Request::new(ctx.actor.clone(), "session.create", serde_json::to_value(&create).bus()?),
                    crate::engine::Door::InProcess,
                );
                match out.into_result() {
                    Ok(session) => created.push(session["name"].as_str().unwrap_or_default().to_string()),
                    Err(error) => {
                        discard_created(ctx.engine(), &created);
                        return Err(error);
                    }
                }
            }
        }
        Ok(PreparedDispatch { children, created })
    }, |ctx: &mut Ctx, p, prepared| {
        let start = p.start.unwrap_or(true);
        let PreparedDispatch { children, created } = prepared;
        let mut names = created.iter().cloned();
        let main = match p.session.clone() {
            Some(name) => name,
            None => names.next().ok_or_else(|| BusError::internal("dispatch created no session"))?,
        };
        let mut launches = Vec::new();
        let (task, session, launch) = dispatch_task(ctx, p.task_id, &main, start)?;
        launches.extend(launch.map(|op| (op, main.clone())));
        let mut fanned = Vec::new();
        for id in children {
            if get_task(ctx.tx(), id, false)?.column == Column::Done {
                continue;
            }
            let name = names.next().ok_or_else(|| BusError::conflict(
                "task.changed", "sub-tasks were added while the dispatch was preparing; dispatch again",
            ))?;
            let (task, session, launch) = dispatch_task(ctx, id, &name, start)?;
            launches.extend(launch.map(|op| (op, name.clone())));
            fanned.push(Dispatched { task, session });
        }
        // Each launch is its own request, after the commit and before the reply, so a client
        // that attaches as soon as the dispatch answers finds the PTY. A launch that fails then
        // has nobody to answer: it is audited, and said in a notification.
        let project_id = task.project_id;
        ctx.after_commit(move |engine| {
            for (op, name) in launches {
                let launched = engine.dispatch(
                    relay_bus::Request::new(relay_bus::Actor::User, op, json!({"session": name})),
                    crate::engine::Door::InProcess,
                );
                if let Some(error) = launched.error {
                    tracing::warn!(session = %name, op, code = %error.code, "starting a dispatched session");
                    let _ = engine.system_write("task.dispatch.launch_failed", None, Some(project_id), None, json!({"session": name, "code": error.code}), |tx, now| {
                        tx.execute(
                            "INSERT INTO notifications(project_id,category,title,body,link,read,created_at) VALUES (?1,'session',?2,?3,NULL,0,?4)",
                            params![project_id, format!("{name} did not start"), format!("{op}: {}", error.message), now],
                        ).bus()?;
                        Ok(((), vec![("notify.new".into(), json!({"category": "session", "project_id": project_id}))]))
                    });
                }
            }
        });
        // Re-read: fanning out to a child does not change the parent row, but its roll-up and
        // its children's states did move, and the caller reads them off this Task.
        let task = get_task(ctx.tx(), task.id, false)?;
        Ok(DispatchOut {
            task,
            session,
            fanned,
        })
    });
    e.register::<Approve>(|ctx: &mut Ctx, p| {
        let before = get_task(ctx.tx(), p.task_id, false)?;
        if before.column == Column::Done {
            return Err(BusError::conflict(
                "task.column_transition",
                "task is already done",
            ));
        }
        let (sha, branch) = if let Some(sha) = p.sha {
            (sha, None)
        } else {
            let session_id = ctx.tx().query_row(
                "SELECT session_id FROM task_sessions WHERE task_id=?1 ORDER BY ord DESC,session_id DESC LIMIT 1",
                [before.id],
                |row| row.get::<_, Id>(0),
            ).optional().bus()?;
            let project = crate::handlers::workspace::get_project(ctx.tx(), before.project_id)?;
            let repo = gix::open(&project.path)
                .map_err(|e| BusError::unavailable("git.head", e.to_string()))?;
            // Approval is a historical task operation. The assigned session may be closed, its
            // worktree removed, and — once its work merged — its branch deleted by branch
            // cleanup; a session in a detached primary checkout records the branch `HEAD`,
            // which never resolves as a ref. So take the first of: the recorded branch's tip,
            // the commit `session.done` already linked, the base branch tip (which holds the
            // merged work), and the checkout HEAD. Fail only when the repository says nothing.
            let recorded = match session_id {
                Some(session_id) => Some(sessions::by_id(ctx.tx(), session_id)?
                    .ok_or_else(|| BusError::internal("task session vanished"))?.session.branch),
                None => None,
            };
            let tip = |reference: &str| repo.rev_parse_single(reference).ok().map(|id| id.detach().to_string());
            let on_branch = recorded.as_deref().filter(|branch| *branch != "HEAD")
                .and_then(|branch| Some((tip(&format!("refs/heads/{branch}"))?, Some(branch.to_string()))));
            let linked = || -> Result<Option<(String, Option<String>)>, BusError> {
                ctx.tx().query_row(
                    "SELECT sha, branch FROM task_commits WHERE task_id=?1 ORDER BY id DESC LIMIT 1",
                    [before.id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                ).optional().bus()
            };
            let found = match on_branch {
                Some(found) => Some(found),
                None if recorded.is_some() => linked()?.or_else(|| {
                    tip(&format!("refs/heads/{}", project.base_branch)).map(|sha| (sha, Some(project.base_branch.clone())))
                }),
                None => None,
            };
            match found {
                Some(found) => found,
                None => {
                    // Work completed directly in the project checkout still gets the same Done
                    // invariant: link the checkout HEAD even though Relay never dispatched it.
                    let branch = repo.head_name().ok().flatten().map(|name| name.shorten().to_string());
                    let sha = repo.head_id()
                        .map_err(|e| BusError::unavailable("git.head", e.to_string()))?
                        .to_string();
                    (sha, branch)
                }
            }
        };
        let added = link_commit(ctx.tx(), before.id, &sha, branch.as_deref(), &ctx.now)?;
        let index = column_index(ctx.tx(), &before)?;
        ctx.tx()
            .execute(
                "UPDATE tasks SET col='done',position=?1,state='none',updated_at=?2 WHERE id=?3",
                params![
                    next_position(ctx.tx(), before.project_id, Column::Done)?,
                    ctx.now,
                    before.id
                ],
            )
            .bus()?;
        let task = get_task(ctx.tx(), before.id, false)?;
        // Undone by its own inverse, not a plain move: that left the commit link behind
        // (RA-416). `sha` is set only when this approval made the link.
        ctx.set_undo(
            "task.unapprove",
            json!({"task_id":before.id,"column":column_str(before.column),"position":index,"state":state_str(before.state),"sha":added}),
            Some(json!({"updated_at":task.updated_at})),
        );
        emit_task(ctx, &task)?;
        Ok(task)
    });
    e.register::<Unapprove>(|ctx: &mut Ctx, p| {
        let before = get_task(ctx.tx(), p.task_id, false)?;
        if before.column != Column::Done {
            return Err(BusError::conflict("task.column_transition", "task is not done"));
        }
        if p.column == Column::Done {
            return Err(BusError::invalid("task.column", "unapprove moves a task out of done"));
        }
        // The commit goes back to the approval that would link it again: the one removed, or
        // the newest still linked when the approval found its link already there.
        let mut sha = None;
        if let Some(removed) = p.sha.as_deref().map(str::trim) {
            let unlinked = ctx.tx().execute("DELETE FROM task_commits WHERE task_id=?1 AND sha=?2", params![before.id, removed]).bus()?;
            if unlinked > 0 { sha = Some(removed.to_string()); }
        }
        if sha.is_none() {
            sha = before.commits.last().map(|c| c.sha.clone());
        }
        let position = open_slot(ctx.tx(), before.project_id, p.column, before.id, p.position)?;
        ctx.tx()
            .execute(
                "UPDATE tasks SET col=?1,position=?2,state=?3,updated_at=?4 WHERE id=?5",
                params![column_str(p.column), position, state_str(p.state), ctx.now, before.id],
            )
            .bus()?;
        let task = get_task(ctx.tx(), before.id, false)?;
        if sha.is_some() {
            ctx.set_undo(
                "task.approve",
                json!({"task_id":before.id,"sha":sha}),
                Some(json!({"updated_at":task.updated_at})),
            );
        }
        emit_task(ctx, &task)?;
        Ok(task)
    });
    e.register::<ParentSet>(|ctx: &mut Ctx, p| {
        let before = get_task(ctx.tx(), p.task_id, false)?;
        if let Some(parent_id) = p.parent_id {
            assert_parent(ctx.tx(), before.project_id, Some(before.id), parent_id)?;
        }
        // `position` is the board's column order, so it is taken as an index in the task's
        // column, the way task.move takes it, and the column renumbered around it. Written raw it
        // reordered the column by accident, and a huge one broke every append to it (RA-417).
        let mut undo = json!({"task_id":before.id,"parent_id":before.parent_id});
        let position = match p.position {
            Some(at) => {
                undo["position"] = json!(column_index(ctx.tx(), &before)?);
                open_slot(ctx.tx(), before.project_id, before.column, before.id, at)?
            }
            None => before.position,
        };
        ctx.tx()
            .execute(
                "UPDATE tasks SET parent_id=?1,position=?2,updated_at=?3 WHERE id=?4",
                params![p.parent_id, position, ctx.now, before.id],
            )
            .bus()?;
        let task = get_task(ctx.tx(), before.id, false)?;
        ctx.set_undo(
            "task.parent.set",
            undo,
            Some(json!({"updated_at":task.updated_at})),
        );
        emit_task(ctx, &task)?;
        // The old and the new parent both changed their roll-up; the board reads roll-ups off
        // the parent row, so both have to be re-emitted or a stale n/m survives on screen.
        for parent in [before.parent_id, task.parent_id] {
            emit_live_parent(ctx, parent)?;
        }
        Ok(task)
    });

    e.register::<Children>(|ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let ids = if p.recursive.unwrap_or(false) {
            descendants(ctx.tx(), task.id)?
        } else {
            task.children.clone()
        };
        let mut tasks = Vec::with_capacity(ids.len());
        for id in ids {
            tasks.push(get_task(ctx.tx(), id, false)?);
        }
        Ok(ListOut { tasks, next_offset: None })
    });

    e.register::<LabelAdd>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let name = normalise_label(&p.label)?;
        let id = label_id(ctx.tx(), task.project_id, &name, &ctx.now)?;
        let added = ctx
            .tx()
            .execute(
                "INSERT OR IGNORE INTO task_labels(task_id,label_id) VALUES (?1,?2)",
                params![task.id, id],
            )
            .bus()?;
        touch(ctx, task.id)?;
        let task = get_task(ctx.tx(), task.id, false)?;
        if added > 0 {
            ctx.set_undo(
                "task.label.remove",
                json!({"task_id":task.id,"label":name}),
                Some(json!({"updated_at":task.updated_at})),
            );
        }
        emit_task(ctx, &task)?;
        Ok(task)
    });

    e.register::<LabelRemove>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let name = normalise_label(&p.label)?;
        let removed = ctx.tx().execute(
            "DELETE FROM task_labels WHERE task_id=?1 AND label_id IN (SELECT id FROM labels WHERE project_id=?2 AND name=?3 COLLATE NOCASE)",
            params![task.id, task.project_id, name],
        ).bus()?;
        touch(ctx, task.id)?;
        let task = get_task(ctx.tx(), task.id, false)?;
        if removed > 0 {
            ctx.set_undo(
                "task.label.add",
                json!({"task_id":task.id,"label":name}),
                Some(json!({"updated_at":task.updated_at})),
            );
        }
        emit_task(ctx, &task)?;
        Ok(task)
    });

    e.register::<LabelList>(|ctx, p| {
        crate::handlers::notes::assert_actor_project(ctx, p.project_id)?;
        let mut stmt = ctx
            .tx()
            .prepare_cached("SELECT id,project_id,name,created_at FROM labels WHERE project_id=?1 ORDER BY name COLLATE NOCASE")
            .bus()?;
        let labels = stmt
            .query_map([p.project_id], |r| {
                Ok(Label { id: r.get(0)?, project_id: r.get(1)?, name: r.get(2)?, created_at: r.get(3)? })
            })
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        Ok(LabelListOut { labels })
    });

    e.register::<Relate>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let other = assert_relatable(ctx.tx(), &task, p.other_id)?;
        let rel = relation_str(p.relation);
        // duplicate_of is single-valued: a task duplicates one other task, so a second call
        // replaces the first rather than stacking edges — every first, an edge to a deleted
        // task included, which `task.duplicate_of` does not show (RA-418).
        let replaced = if p.relation == TaskRelation::DuplicateOf { task.duplicate_of.filter(|prior| *prior != other) } else { None };
        if p.relation == TaskRelation::DuplicateOf {
            ctx.tx().execute("DELETE FROM task_relations WHERE from_task=?1 AND rel='duplicate_of' AND to_task<>?2", params![task.id, other]).bus()?;
        }
        let added = ctx.tx()
            .execute(
                "INSERT OR IGNORE INTO task_relations(from_task,to_task,rel,created_at) VALUES (?1,?2,?3,?4)",
                params![task.id, other, rel, ctx.now],
            )
            .bus()?;
        touch(ctx, task.id)?;
        let task = get_task(ctx.tx(), task.id, false)?;
        // Re-linking an edge that is already there changes nothing, and an undo of it would
        // drop the edge that was there before.
        let undo = match replaced {
            Some(prior) => Some(("task.relate", json!({"task_id":task.id,"relation":rel,"other_id":prior}))),
            None if added > 0 => Some(("task.unrelate", json!({"task_id":task.id,"relation":rel,"other_id":other}))),
            None => None,
        };
        if let Some((op, undo)) = undo {
            ctx.set_undo(op, undo, Some(json!({"updated_at":task.updated_at})));
        }
        emit_task(ctx, &task)?;
        emit_task(ctx, &get_task(ctx.tx(), other, false)?)?;
        Ok(task)
    });

    e.register::<Unrelate>(|ctx: &mut Ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let other = assert_relatable(ctx.tx(), &task, p.other_id)?;
        let rel = relation_str(p.relation);
        let removed = ctx
            .tx()
            .execute(
                "DELETE FROM task_relations WHERE from_task=?1 AND to_task=?2 AND rel=?3",
                params![task.id, other, rel],
            )
            .bus()?;
        touch(ctx, task.id)?;
        let task = get_task(ctx.tx(), task.id, false)?;
        if removed > 0 {
            ctx.set_undo(
                "task.relate",
                json!({"task_id":task.id,"relation":rel,"other_id":other}),
                Some(json!({"updated_at":task.updated_at})),
            );
        }
        emit_task(ctx, &task)?;
        emit_task(ctx, &get_task(ctx.tx(), other, false)?)?;
        Ok(task)
    });

    e.register::<CopyText>(|ctx, p| {
        let task = get_task(ctx.tx(), p.task_id, false)?;
        let mut text = format!("#{} {}", task.id, task.title);
        if !task.body.trim().is_empty() {
            text.push_str("\n\n");
            text.push_str(&task.body)
        }
        Ok(CopyTextOut { text })
    });
}
