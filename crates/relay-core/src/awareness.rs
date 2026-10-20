//! Deterministic Phase-5 knowledge injection (SPEC §3 / BUS.md §10.9).

use crate::engine::IntoBus;
use crate::sessions;
use relay_bus::envelope::Actor;
use relay_bus::error::BusError;
use relay_bus::ops::session::{
    BootstrapBaseSync, BootstrapGuardrails, BootstrapOut, BootstrapPair, BootstrapTask, BriefOut, BriefParts,
};
use relay_bus::registry::{Callable, Registry};
use relay_bus::types::{Id, Peer};
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone)]
struct AssignedTask {
    id: Id,
    title: String,
    body: String,
    changelog: String,
    column: String,
    state: String,
    module_id: Option<Id>,
    /// This session already reported the task done; the rest of its group has not.
    completed: bool,
}

/// `sessions.task_id` is the current task, while `task_sessions` is the ordered queue.
/// Done tasks are historical links, not launch work.
fn assigned_tasks(conn: &Connection, session_id: Id) -> Result<Vec<AssignedTask>, BusError> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id,t.title,t.body,t.changelog,t.col,t.state,t.module_id,ts.completed_at IS NOT NULL
         FROM task_sessions ts JOIN tasks t ON t.id=ts.task_id
         WHERE ts.session_id=?1 AND t.deleted_at IS NULL AND t.col!='done'
         ORDER BY CASE WHEN t.id=(SELECT task_id FROM sessions WHERE id=?1) THEN 0 ELSE 1 END,
                  CASE t.col WHEN 'active' THEN 0 WHEN 'in_review' THEN 1 ELSE 2 END,
                  ts.queue_ord,t.position,t.id",
        )
        .bus()?;
    let tasks = stmt
        .query_map([session_id], |record| {
            Ok(AssignedTask {
                id: record.get(0)?,
                title: record.get(1)?,
                body: record.get(2)?,
                changelog: record.get(3)?,
                column: record.get(4)?,
                state: record.get(5)?,
                module_id: record.get(6)?,
                completed: record.get(7)?,
            })
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    Ok(tasks)
}

fn bootstrap_task(task: &AssignedTask) -> BootstrapTask {
    BootstrapTask {
        id: task.id,
        title: task.title.clone(),
        body: task.body.clone(),
        changelog: task.changelog.clone(),
        column: task.column.clone(),
        state: task.state.clone(),
    }
}

/// Where `launch` writes each half of the brief, worktree-relative. The compact half is
/// injected into the system prompt on every spawn; the skills half is referenced by path so
/// 27 KB of skill bodies never crowd out the four lines that say who the peers are (D101).
pub const SESSION_SKILLS_PATH: &str = ".relay/session-skills.md";

pub fn session_brief_path(session: &str) -> String {
    format!(".relay/sessions/{session}/session-brief.md")
}

/// The one line that overrides an agent's reflex to reach for its provider's own messaging.
pub const COMMS_HINT: &str = "Coordinate with peers through Relay mailbox.send \
(`$RELAY_BIN q mailbox.send`). It reaches every provider, including Codex sessions; your \
provider's own cross-agent messaging does not see them. Send \
`{\"to\":\"*\",\"text\":\"...\"}` to broadcast; mailbox.outbox shows what you sent.";

/// The live peer table. `engine` supplies each peer's most recent PTY output, which is the
/// only signal Relay has for a provider that runs no lifecycle hooks at all (D108).
pub fn peers(
    conn: &Connection,
    project_id: Id,
    except: Option<&str>,
    engine: Option<&crate::engine::Engine>,
) -> Result<Vec<Peer>, BusError> {
    // Scoped so the statement is back in the cache before the claims query below borrows it.
    let rows = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT s.*, t.title AS task_title
         FROM sessions s LEFT JOIN tasks t ON t.id=s.task_id AND t.deleted_at IS NULL
         WHERE s.project_id=?1 AND s.state!='closed' ORDER BY s.name, s.id",
            )
            .bus()?;
        let rows = stmt
            .query_map([project_id], |row| {
                let session = sessions::row(row)?.session;
                let task_title = row.get::<_, Option<String>>("task_title")?;
                Ok((session, task_title))
            })
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        rows
    };

    // Prepared once, not once per peer: `dashboard.get` calls this for every project.
    let mut claims_stmt = conn
        .prepare_cached("SELECT path, symbol FROM claims WHERE session_id=?1 ORDER BY path, symbol")
        .bus()?;
    let mut out = Vec::new();
    for (session, task_title) in rows {
        if except == Some(session.name.as_str()) {
            continue;
        }
        let claimed = claims_stmt
            .query_map([session.id], |row| {
                let path: String = row.get(0)?;
                let symbol: String = row.get(1)?;
                Ok(if symbol.is_empty() {
                    path
                } else {
                    format!("{path}#{symbol}")
                })
            })
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        let last_output_at = engine
            .and_then(|engine| engine.pty(session.id))
            .and_then(|pty| pty.last_output_at())
            .or(session.last_output_at);
        out.push(Peer {
            session: session.name,
            provider: session.provider,
            role: session.role,
            branch: session.branch,
            state: session.state,
            task_title,
            claimed,
            last_output_at,
            intent: session.intent,
        });
    }
    Ok(out)
}

/// Each brief section's budget (RA-385). The compact half is injected into every spawn — for
/// Codex as one argv element, cut at 64 KiB — so one long task body or a module of hundreds
/// of tasks crowded out everything after it. A section that is cut says so and names the op
/// that returns the whole of it.
const SECTION_LINES: usize = 40;
const SECTION_BYTES: usize = 8 * 1024;
/// Per task body and per changelog, in the current-state section.
const TASK_TEXT_BYTES: usize = 4 * 1024;

/// `lines`, one per line, at most [`SECTION_LINES`] of them and about [`SECTION_BYTES`]; when
/// some are left out, a last line counts them and names `rest`, the op with the full list.
fn capped_lines(lines: Vec<String>, rest: &str) -> String {
    let total = lines.len();
    let mut out = String::new();
    let mut kept = 0;
    for line in lines {
        if kept == SECTION_LINES || (kept > 0 && out.len() + 1 + line.len() > SECTION_BYTES) {
            break;
        }
        if kept > 0 {
            out.push('\n');
        }
        out.push_str(&line);
        kept += 1;
    }
    if kept < total {
        out.push_str(&format!("\n- … and {} more, not shown here; the full list: {rest}", total - kept));
    }
    out
}

/// `text` cut to at most `max` bytes, at a line break when one is near, then a line that says
/// it was cut and names `whole`, the op that returns all of it.
fn capped_text(text: &str, max: usize, whole: &str) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut at = max;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    let at = text[..at].rfind('\n').filter(|line| *line >= at / 2).unwrap_or(at);
    format!("{}\n[… cut here: {at} of {} bytes shown; the whole text: {whole}]", text[..at].trim_end(), text.len())
}

/// Whether the session is on its first launch, or not launched yet. Every spawn, fresh or
/// resumed, moves `epoch` on by one from 0, so a relaunch is past 1 — the same line the launch
/// nudge draws with `spawned_at` before it spawns (RA-397).
fn first_launch(row: &sessions::Row_) -> bool {
    row.epoch <= 1
}

/// `git` is [`crate::handlers::git::briefing_state`] for the session, taken by the caller with
/// the store unlocked: it walks commit history, and this runs under the store lock.
pub fn brief(
    conn: &Connection,
    session_name: &str,
    engine: Option<&crate::engine::Engine>,
    git: &str,
) -> Result<BriefOut, BusError> {
    let row = sessions::by_name(conn, session_name)?;
    let session = &row.session;
    let project = conn
        .query_row(
            "SELECT name, path, base_branch FROM projects WHERE id=?1",
            [session.project_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .bus()?
        .ok_or_else(|| {
            BusError::not_found(
                "project.not_found",
                format!("no project {}", session.project_id),
            )
        })?;

    let tasks = assigned_tasks(conn, session.id)?;
    let task = session
        .task_id
        .and_then(|id| tasks.iter().find(|task| task.id == id))
        .or_else(|| tasks.first());
    let module_id = session
        .module_id
        .or_else(|| task.and_then(|task| task.module_id));
    let module_name = match module_id {
        Some(id) => conn
            .query_row(
                "SELECT name FROM modules WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .bus()?,
        None => None,
    };

    let mut state = format!(
        "session: {}\nprovider: {}\nrole: {}\nproject: {} ({})\nworktree: {}\nbranch: {}",
        session.name,
        sessions::provider_str(session.provider),
        sessions::role_str(session.role),
        project.0,
        project.1,
        session.worktree,
        session.branch,
    );
    state.push('\n');
    state.push_str(git);
    // Repositories linked to this session (`session.repo.add`); one read_dir.
    let worktree = std::path::Path::new(&session.worktree);
    if !crate::linked::list(worktree).is_empty() {
        state.push_str(&format!("\nlinked_repos: {}", crate::linked::brief_line(worktree)));
    }
    if let Some(module) = &module_name {
        state.push_str(&format!("\nmodule: {module}"));
    }
    if !tasks.is_empty() {
        if let Some(task_id) = session.task_id {
            state.push_str(&format!("\ncurrent_task: #{task_id}"));
        }
        state.push_str(&format!("\nassigned_tasks: {}", tasks.len()));
        for task in &tasks {
            // Labelled from the task's real column: a task moved back to the backlog is
            // parked, not in review (RA-308).
            let queue_state = if Some(task.id) == session.task_id {
                "CURRENT"
            } else if task.completed {
                "DONE, WAITING FOR GROUP"
            } else {
                match task.column.as_str() {
                    "active" => "QUEUED",
                    "in_review" => "IN REVIEW",
                    "ready" => "READY",
                    _ => "BACKLOG",
                }
            };
            state.push_str(&format!(
                "\n\nTask #{} [{}]: {}\ncolumn: {}\ntask_state: {}",
                task.id, queue_state, task.title, task.column, task.state,
            ));
            let whole = format!("`task.get {{\"task_id\": {}}}`", task.id);
            if !task.body.is_empty() {
                state.push_str(&format!("\nTask body:\n{}", capped_text(&task.body, TASK_TEXT_BYTES, &whole)));
            }
            if !task.changelog.is_empty() {
                state.push_str(&format!("\nTask changelog:\n{}", capped_text(&task.changelog, TASK_TEXT_BYTES, &whole)));
            }
        }
    } else {
        state.push_str("\ntask: unassigned");
    }
    // The compact brief is injected into system prompts and, for Codex, into argv. The launch
    // assignment stays out of it: `ps` is world-readable, and `session.bootstrap` — which
    // every role is told to call first — is the private channel that already carries it.
    let state_public = state.clone();
    if let Some(assignment) = row
        .launch_prompt
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        // Relaunched, the session may be long past it: name it for what it now is (RA-397).
        let label = if first_launch(&row) { "Launch assignment" } else { "Original launch assignment (from the first launch; may already be done)" };
        state.push_str(&format!("\n\n{label}:\n{assignment}"));
    }

    let peer_rows = peers(conn, session.project_id, Some(&session.name), engine)?;
    let peers_text = if peer_rows.is_empty() {
        "No live peers.".to_string()
    } else {
        let lines = peer_rows
            .iter()
            .map(|p| {
                let task = p.task_title.as_deref().unwrap_or("unassigned");
                let claims = if p.claimed.is_empty() {
                    "none".to_string()
                } else {
                    p.claimed.join(", ")
                };
                format!(
                    "- {} | {} | {} | {} | task: {} | claims: {}",
                    p.session,
                    sessions::provider_str(p.provider),
                    sessions::role_str(p.role),
                    sessions::state_str(p.state),
                    task,
                    claims
                )
            })
            .collect::<Vec<_>>();
        capped_lines(lines, "`session.peers`")
    };

    let adjacent = if let Some(module_id) = module_id {
        let mut stmt = conn
            .prepare_cached(
                "SELECT title, col, state FROM tasks
             WHERE project_id=?1 AND module_id=?2 AND deleted_at IS NULL AND (?3 IS NULL OR id!=?3)
             ORDER BY position, id",
            )
            .bus()?;
        let lines = stmt
            .query_map(
                rusqlite::params![session.project_id, module_id, session.task_id],
                |r| {
                    Ok(format!(
                        "- {} | {} | {}",
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?
                    ))
                },
            )
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        if lines.is_empty() {
            "No adjacent tasks.".to_string()
        } else {
            capped_lines(lines, &format!("`task.list {{\"module_id\": {module_id}}}`"))
        }
    } else {
        "No module context.".to_string()
    };

    let mut skill_stmt = conn
        .prepare_cached(
            "SELECT s.name,s.body,s.id FROM skills s
             JOIN skill_projects sp ON sp.skill_id=s.id
             WHERE sp.project_id=?1 AND s.deleted_at IS NULL
             ORDER BY s.name COLLATE NOCASE,s.id",
        )
        .bus()?;
    // The folder the materializer writes, which is not always the bare name (two names can
    // reduce to one folder).
    let dirs = crate::skills::folder_names(conn).bus()?;
    let skill_rows = skill_stmt
        .query_map([session.project_id], |row| {
            let name = row.get::<_, String>(0)?;
            let dir = dirs.get(&row.get::<_, Id>(2)?).cloned().unwrap_or_else(|| crate::skills::folder_name(&name));
            Ok((name, row.get::<_, String>(1)?, dir))
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    // The injected half names the skills and where they are registered rather than the file
    // they were dumped into (D147): a path to 27 KB of bodies tells an agent nothing about
    // what it may load, and nothing makes it read the file before the work the skill covers.
    let skills_summary = if skill_rows.is_empty() {
        "No enabled Relay skills.".to_string()
    } else {
        let listed = skill_rows
            .iter()
            .map(|(name, _, dir)| {
                format!(
                    "- {name} — {}/{dir}/, {}/{dir}/",
                    crate::skills::TARGETS[0],
                    crate::skills::TARGETS[1]
                )
            })
            .collect::<Vec<_>>();
        let listed = capped_lines(listed, "`skill.list`");
        format!(
            "These are installed in this checkout as skill folders. Load one through your own skill mechanism before work it covers; if your provider has none, read its SKILL.md at the path below.\n{listed}\nFull bodies, inline: {SESSION_SKILLS_PATH}"
        )
    };
    let skills = if skill_rows.is_empty() {
        "No enabled Relay skills.".to_string()
    } else {
        skill_rows
            .into_iter()
            .map(|(name, body, _)| format!("### {name}\n\n{body}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    // A plugin that is on is a standing rule, not an option: its instructions travel in the
    // injected half so the agent works through it from the first turn (D159).
    let plugins = crate::plugins::brief_section(conn, session.project_id).bus()?;
    let parts = BriefParts {
        state,
        peers: peers_text,
        adjacent,
        skills,
        plugins,
    };
    let compact = format!(
        "# Relay session brief\n\n## Current state\n{}\n\n## Live peers\n{}\n\n## Adjacent tasks\n{}\n\n## Comms\n{}\n\n## Assignment\nRun `session.bootstrap` for your launch assignment and current task.\n\n## Enabled skills\n{}\n\n## Enabled plugins\n{}",
        state_public, parts.peers, parts.adjacent, COMMS_HINT, skills_summary, parts.plugins,
    );
    let text = format!(
        "# Relay session brief\n\n## Current state\n{}\n\n## Live peers\n{}\n\n## Adjacent tasks\n{}\n\n## Comms\n{}\n\n## Enabled skills\n{}\n\n## Enabled plugins\n{}",
        parts.state, parts.peers, parts.adjacent, COMMS_HINT, parts.skills, parts.plugins,
    );
    Ok(BriefOut {
        text,
        compact,
        parts,
    })
}

/// The actor-bound startup payload: identity, the live peer table, the ops this session may
/// really call, how to reach the peers, and the guardrails it will meet. Every field is data
/// the engine already holds; the one call every agent makes is the wrong place to be coy
/// about it (D105). Adjacent work and skills stay in the brief; notes are in neither, since an
/// agent works from its task and reads a note only when it asks for one.
pub fn bootstrap(
    conn: &Connection,
    session_name: &str,
    engine: &crate::engine::Engine,
    base_sync: Option<BootstrapBaseSync>,
) -> Result<BootstrapOut, BusError> {
    let row = sessions::by_name(conn, session_name)?;
    let session = &row.session;
    let (project, base_branch) = conn
        .query_row(
            "SELECT name,base_branch FROM projects WHERE id=?1",
            [session.project_id],
            |record| Ok((record.get::<_, String>(0)?, record.get::<_, String>(1)?)),
        )
        .optional()
        .bus()?
        .ok_or_else(|| {
            BusError::not_found(
                "project.not_found",
                format!("no project {}", session.project_id),
            )
        })?;

    let pair_row = match session.pair_with.as_deref() {
        Some(name) => conn.query_row(
            "SELECT name,provider,role,branch,state,task_id,module_id FROM sessions WHERE name=?1 AND state!='closed' ORDER BY id DESC LIMIT 1",
            [name],
            |record| Ok((
                record.get::<_, String>(0)?, record.get::<_, String>(1)?, record.get::<_, String>(2)?,
                record.get::<_, String>(3)?, record.get::<_, String>(4)?, record.get::<_, Option<Id>>(5)?,
                record.get::<_, Option<Id>>(6)?,
            )),
        ).optional().bus()?,
        None => None,
    };
    let pair = pair_row.as_ref().map(|peer| BootstrapPair {
        session: peer.0.clone(),
        provider: sessions::parse_provider(&peer.1),
        role: sessions::parse_role(&peer.2),
        branch: peer.3.clone(),
        state: sessions::parse_state(&peer.4),
    });

    let assigned = assigned_tasks(conn, session.id)?;
    let task_id = session
        .task_id
        .or_else(|| pair_row.as_ref().and_then(|peer| peer.5));
    let task_row = task_id
        .and_then(|id| assigned.iter().find(|task| task.id == id))
        .or_else(|| assigned.first());
    let module_id = session
        .module_id
        .or_else(|| task_row.and_then(|task| task.module_id))
        .or_else(|| pair_row.as_ref().and_then(|peer| peer.6));
    let module = match module_id {
        Some(id) => conn
            .query_row(
                "SELECT name FROM modules WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |record| record.get::<_, String>(0),
            )
            .optional()
            .bus()?,
        None => None,
    };
    let task = task_row.map(bootstrap_task);
    let tasks = assigned.iter().map(bootstrap_task).collect();
    // The work to begin only on the first launch, as the launch nudge hands it over: a later
    // fresh start may come long after it was done, and is nudged toward its current task
    // instead (RA-397). The text stays in the brief, labelled as the original instructions.
    let assignment = row.launch_prompt.clone().filter(|text| first_launch(&row) && !text.trim().is_empty());

    let cfg = crate::guardrail::config(conn, Some(session.project_id))?;
    let plugins_on = crate::plugins::enabled_ids(conn, session.project_id).bus()?;
    let actor = Actor::Agent(session.name.clone());
    let can_call = Registry::global()
        .entries()
        .iter()
        .filter(|entry| engine.is_implemented(entry.name))
        .filter(|entry| {
            crate::guardrail::callability(entry, &actor, Some(session), Some(&cfg), &plugins_on).0
                != Callable::No
        })
        .map(|entry| entry.name.to_string())
        .collect();
    let write_roots = crate::guardrail::write_roots(&cfg, std::path::Path::new(&session.worktree))
        .iter()
        .map(|root| root.display().to_string())
        .collect();
    let guardrails = BootstrapGuardrails {
        caps: cfg.caps.clone(),
        denied_commands: cfg.denied_commands.clone(),
        write_roots,
        dry_run: "guardrail.check".to_string(),
        exceptions: crate::guardrail::grants::BOOTSTRAP_HINT.to_string(),
    };

    Ok(BootstrapOut {
        session: session.name.clone(),
        role: session.role,
        project_id: session.project_id,
        project,
        base_branch,
        worktree: session.worktree.clone(),
        branch: session.branch.clone(),
        module,
        base_sync,
        task,
        tasks,
        pair,
        assignment,
        brief_path: session_brief_path(&session.name),
        peers: peers(conn, session.project_id, Some(&session.name), Some(engine))?,
        can_call,
        discovery: crate::providers::DISCOVERY_HINT.to_string(),
        comms: COMMS_HINT.to_string(),
        guardrails,
    })
}
