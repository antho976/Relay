//! `guardrail.*` (SPEC §5, BUS.md §9): pure checks, enforcing gates, durable holds,
//! confirmation/rejection and effective configuration.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::guardrail::{self, grants, ConfigScope, Decision, GateRequest, Probes};
use crate::handlers::workspace::{get_project, get_workspace};
use crate::sessions;
use relay_bus::envelope::{Actor, Request, Response};
use relay_bus::error::BusError;
use relay_bus::ops::guardrail::*;
use relay_bus::types::{GateKind, GrantScope, GuardrailLayer, HoldState, Id, Verdict};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Which layer a `workspace_id` / `project_id` pair names. Both at once is ambiguous: a project
/// already sits on its workspace.
fn scope_of(conn: &Connection, workspace_id: Option<Id>, project_id: Option<Id>) -> Result<ConfigScope, BusError> {
    match (workspace_id, project_id) {
        (Some(_), Some(_)) => Err(BusError::invalid(
            "guardrail.scope",
            "give workspace_id or project_id, not both: a project already inherits its workspace",
        )),
        (Some(id), None) => get_workspace(conn, id).map(|_| ConfigScope::Workspace(id)),
        (None, Some(id)) => get_project(conn, id).map(|_| ConfigScope::Project(id)),
        (None, None) => Ok(ConfigScope::Global),
    }
}

pub fn register(engine: &mut Engine) {
    engine.register::<ConfigGet>(|ctx, payload| {
        let scope = scope_of(ctx.tx(), payload.workspace_id, payload.project_id)?;
        guardrail::config_for(ctx.tx(), scope)
    });

    engine.register::<ConfigSet>(|ctx: &mut Ctx, payload| {
        let Some(patch) = payload.patch.as_object() else {
            return Err(BusError::invalid("guardrail.config", "patch must be an object"));
        };
        if patch.contains_key("projects") || patch.contains_key("workspaces") {
            return Err(BusError::invalid(
                "guardrail.config",
                "patch a project or workspace layer with project_id / workspace_id, not a nested key",
            ));
        }
        let scope = scope_of(ctx.tx(), payload.workspace_id, payload.project_id)?;
        if let ConfigScope::Project(id) = scope {
            ctx.set_project(id);
        }
        // Each layer stores only what it overrides, so clearing a key (`null`) brings the
        // inherited value back instead of freezing a copy of it.
        let before = guardrail::layers(ctx.tx(), scope)?
            .raw
            .pop()
            .map(|(_, raw)| raw)
            .unwrap_or_else(|| json!({}));
        let mut merged = before.clone();
        crate::handlers::settings::merge_value(&mut merged, &payload.patch);
        guardrail::prune_empty(&mut merged);
        let now = ctx.now.clone();
        let layer_path = match scope {
            ConfigScope::Global => None,
            ConfigScope::Workspace(id) => Some(format!("guardrails.workspaces.{id}")),
            ConfigScope::Project(id) => Some(format!("guardrails.projects.{id}")),
        };
        match &layer_path {
            // `settings::set` replaces the subtree it writes; a layer or key cleared to nothing
            // has no write, so its old rows are deleted instead.
            Some(path) => {
                if merged.as_object().is_some_and(|map| !map.is_empty()) {
                    crate::handlers::settings::set(ctx.tx(), path, &merged, &now)?;
                } else {
                    crate::handlers::settings::delete_under(ctx.tx(), path)?;
                }
            }
            None => {
                // Write global keys independently: replacing `guardrails` as a whole would
                // delete the separately stored project and workspace layers.
                let keys: std::collections::BTreeSet<String> = before
                    .as_object()
                    .into_iter()
                    .chain(merged.as_object())
                    .flat_map(|map| map.keys().cloned())
                    .collect();
                for key in keys {
                    let path = format!("guardrails.{key}");
                    match merged.get(&key) {
                        Some(value) => crate::handlers::settings::set(ctx.tx(), &path, value, &now)?,
                        None => crate::handlers::settings::delete_under(ctx.tx(), &path)?,
                    }
                }
            }
        }
        // Typed read validates the merged result. An error rolls the handler transaction back.
        let effective = guardrail::config_for(ctx.tx(), scope)?;
        ctx.set_undo(
            "guardrail.config.set",
            json!({
                "workspace_id": payload.workspace_id, "project_id": payload.project_id,
                "patch": guardrail::inverse_patch(&before, &merged),
            }),
            None,
        );
        ctx.emit(
            "guardrail.config_changed",
            json!({"project_id": payload.project_id, "workspace_id": payload.workspace_id}),
        );
        Ok(effective)
    });

    engine.register::<ConfigLayers>(|ctx, payload| {
        let scope = scope_of(ctx.tx(), payload.workspace_id, payload.project_id)?;
        let mut layers = guardrail::layers(ctx.tx(), scope)?;
        let (layer, effective) = layers.stages.pop().ok_or_else(|| BusError::internal("no guardrail layers"))?;
        let (_, inherited) = layers.stages.pop().ok_or_else(|| BusError::internal("no inherited layer"))?;
        let overrides = layers.raw.last().map(|(_, raw)| raw.clone()).unwrap_or_else(|| json!({}));
        let effective = guardrail::typed(effective)?;
        let inherited = guardrail::typed(inherited)?;
        let effective_value = serde_json::to_value(&effective).map_err(crate::engine::internal)?;
        let inherited_value = serde_json::to_value(&inherited).map_err(crate::engine::internal)?;
        let mut leaves = Vec::new();
        guardrail::leaf_paths(&effective_value, "", &mut leaves);
        let at = |value: &Value, path: &str| path.split('.').try_fold(value, |v, part| v.get(part)).cloned();
        let sources = leaves
            .into_iter()
            .map(|path| {
                let set_by = layers.raw.iter().rev().find(|(_, raw)| guardrail::sets_path(raw, &path)).map(|(l, _)| *l);
                // The project's legacy columns add to protected paths and shape gates without
                // a stored override.
                let legacy = layer == GuardrailLayer::Project
                    && layers.legacy
                    && at(&effective_value, &path) != at(&inherited_value, &path);
                let source = set_by.or(legacy.then_some(GuardrailLayer::Project)).unwrap_or(GuardrailLayer::Default);
                (path, source)
            })
            .collect();
        Ok(ConfigLayersOut {
            scope: layer,
            workspace_id: layers.workspace_id,
            project_id: payload.project_id,
            effective,
            inherited,
            overrides,
            sources,
        })
    });

    // A dry run reads the file it would replace and may ask git about it, so it runs with the
    // store lock released: one short read for the project, config and grants, then the
    // judging with nothing held (D149, RA-100).
    engine.register_unlocked::<Check>(|ctx, payload| {
        let session_id = ctx.actor_session_id();
        let agent = ctx.actor.is_agent();
        let (project_id, worktree, cfg, granted) = ctx.read(|conn| {
            let project = get_project(conn, payload.project_id)?;
            // A dry run has to judge the tree the write would land in. Judging the project root
            // would answer a question the caller did not ask (D111).
            let worktree = crate::handlers::file::default_worktree_in(conn, session_id, &project, None)?;
            let cfg = guardrail::config(conn, Some(project.id))?;
            Ok((project.id, worktree, cfg, session_grants(conn, session_id, agent)?))
        })?;
        // A dry run honours this session's grants, so an agent can see an approval land; it
        // never uses one up.
        let request = GateRequest {
            actor: &ctx.actor,
            project_id,
            worktree: &worktree,
            kind: payload.kind,
            path: payload.path.as_deref(),
            new_text: payload.new_text.as_deref(),
            diff: payload.diff.as_deref(),
            command: payload.command.as_deref(),
            skip_policy: None,
            grants: None, path_only: false, probes: None,
        };
        let first = guardrail::evaluate_with(&cfg, &request)?;
        let (decision, _) = guardrail::granted_retry(&cfg, &request, first, &granted)?;
        Ok(check_out(decision))
    });

    engine.register_unlocked::<Explain>(|ctx, payload| {
        // `guardrail.check` answers one action. A plan is decided before the first action, and
        // learning at minute zero that it will trip a cap is worth more than learning it at
        // commit time (D117). Pure: it evaluates, it never holds and never writes.
        let session_id = ctx.actor_session_id();
        let agent = ctx.actor.is_agent();
        let (project_id, worktree, cfg, granted) = ctx.read(|conn| {
            let project = get_project(conn, payload.project_id)?;
            let worktree = crate::handlers::file::default_worktree_in(conn, session_id, &project, None)?;
            let cfg = guardrail::config(conn, Some(project.id))?;
            Ok((project.id, worktree, cfg, session_grants(conn, session_id, agent)?))
        })?;
        let judge = |request: GateRequest<'_>| -> Result<Decision, BusError> {
            let first = guardrail::evaluate_with(&cfg, &request)?;
            guardrail::granted_retry(&cfg, &request, first, &granted).map(|(decision, _)| decision)
        };
        let mut worst = Verdict::Allow;
        let mut note = |verdict: Verdict| {
            // refuse beats hold beats allow
            worst = match (worst, verdict) {
                (Verdict::Refuse, _) | (_, Verdict::Refuse) => Verdict::Refuse,
                (Verdict::Hold, _) | (_, Verdict::Hold) => Verdict::Hold,
                _ => Verdict::Allow,
            };
        };

        let requested = payload.paths.unwrap_or_default();
        let mut paths = Vec::with_capacity(requested.len());
        for path in &requested {
            // No text yet, so this reports the policies that can be judged from a path alone:
            // protected paths, write roots, and whether a shape gate will demand full text.
            // An approved exception counts, as it does in guardrail.check; neither uses it up.
            let decision = judge(GateRequest {
                actor: &ctx.actor,
                project_id,
                worktree: &worktree,
                kind: GateKind::Write,
                path: Some(path),
                new_text: None,
                diff: Some(""),
                command: None,
                skip_policy: None,
                grants: None, path_only: false, probes: None,
            });
            let item = match decision {
                Ok(guardrail::Decision::Allow) => ExplainItem {
                    subject: path.clone(), verdict: Verdict::Allow, policy: None, message: None,
                },
                Ok(guardrail::Decision::Refuse(error)) => ExplainItem {
                    subject: path.clone(), verdict: Verdict::Refuse,
                    policy: Some(error.code.clone()), message: Some(error.message.clone()),
                },
                Ok(guardrail::Decision::Hold { policy, error, .. }) => ExplainItem {
                    subject: path.clone(), verdict: Verdict::Hold,
                    policy: Some(policy), message: Some(error.message.clone()),
                },
                Err(error) => ExplainItem {
                    subject: path.clone(), verdict: Verdict::Refuse,
                    policy: Some(error.code.clone()), message: Some(error.message.clone()),
                },
            };
            note(item.verdict);
            paths.push(item);
        }

        let mut commands = Vec::new();
        for command in payload.commands.unwrap_or_default() {
            let decision = judge(GateRequest {
                actor: &ctx.actor,
                project_id,
                worktree: &worktree,
                kind: GateKind::Exec,
                path: None,
                new_text: None,
                diff: None,
                command: Some(&command),
                skip_policy: None,
                grants: None, path_only: false, probes: None,
            })?;
            let item = match decision {
                guardrail::Decision::Allow => ExplainItem {
                    subject: command, verdict: Verdict::Allow, policy: None, message: None,
                },
                guardrail::Decision::Refuse(error) => ExplainItem {
                    subject: command, verdict: Verdict::Refuse,
                    policy: Some(error.code.clone()), message: Some(error.message.clone()),
                },
                guardrail::Decision::Hold { policy, error, .. } => ExplainItem {
                    subject: command, verdict: Verdict::Hold,
                    policy: Some(policy), message: Some(error.message.clone()),
                },
            };
            note(item.verdict);
            commands.push(item);
        }

        let files = requested.len() as u32;
        let lines = payload.lines.unwrap_or(0);
        let over_caps = (files > cfg.caps.files || lines > cfg.caps.lines)
            && !granted.covers_caps(files, lines, &cfg.caps);
        if over_caps {
            note(Verdict::Refuse);
        }
        // Absolute path grants are roots this session may write to now, too.
        let mut write_roots: Vec<String> = guardrail::write_roots(&cfg, &worktree)
            .iter().map(|root| root.display().to_string()).collect();
        for grant in granted.list.iter().filter(|grant| grant.kind == relay_bus::types::ExceptionKind::Path) {
            let value = grant.value.trim();
            if Path::new(value).is_absolute() && !write_roots.iter().any(|root| root == value) {
                write_roots.push(value.to_string());
            }
        }
        Ok(ExplainOut {
            verdict: worst,
            paths,
            commands,
            files,
            lines,
            caps: cfg.caps.clone(),
            over_caps,
            write_roots,
        })
    });

    // The gate reads the old file, asks git whether it could restore it, and reads the staged
    // numstat: all of that happens first, with the store lock released, and the transaction
    // only judges what was read and records the hold or refusal (D149, RA-100).
    engine.register_staged::<Gate, Probes>(
        |ctx, payload| Ok(warm_gate(ctx, ctx.actor.clone(), payload, None)),
        |ctx: &mut Ctx, payload, probes| gate(ctx, payload, None, &probes),
    );

    engine.register::<HoldsList>(|ctx, payload| {
        guardrail::fill_payload_hashes(ctx.tx())?;
        let mut sql = String::from("SELECT * FROM holds WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(project_id) = visible_project(ctx, payload.project_id)? {
            sql.push_str(" AND project_id = ?");
            args.push(Box::new(project_id));
        }
        if let Some(session) = payload.session {
            sql.push_str(" AND session = ?");
            args.push(Box::new(session));
        }
        if payload.open_only.unwrap_or(true) {
            sql.push_str(" AND state = 'open'");
        }
        // A page, newest first: every client fetches this whole, on a socket that caps a line.
        // `?+0`: SQLite re-prepares a statement whose LIMIT is a bare parameter on every run.
        sql.push_str(" ORDER BY id DESC LIMIT ?+0");
        args.push(Box::new(payload.limit.unwrap_or(HOLDS_PAGE).clamp(1, HOLDS_PAGE_MAX)));
        let mut stmt = ctx.tx().prepare_cached(&sql).bus()?;
        let mut holds = stmt
            .query_map(
                rusqlite::params_from_iter(args.iter().map(|arg| arg.as_ref())),
                guardrail::hold_row,
            )
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        for hold in &mut holds {
            elide_hold(hold, &mut Vec::new());
        }
        Ok(HoldsListOut { holds })
    });

    engine.register::<HoldGet>(|ctx, payload| {
        guardrail::fill_payload_hashes(ctx.tx())?;
        let mut hold = guardrail::hold_by_id(ctx.tx(), payload.hold_id)?;
        assert_visible(ctx, hold.project_id)?;
        // A large text the hold kept beside the store is read back only when asked for whole;
        // the envelope holds it already cut. Confirm always replays it whole.
        let full = payload.full.unwrap_or(false);
        let (mut request, kept_aside) = guardrail::frozen_envelope(ctx.tx(), payload.hold_id, full)?;
        request.token = None;
        let mut elided = Vec::new();
        if !full {
            elide_hold(&mut hold, &mut elided);
            elided.extend(kept_aside.iter().map(|pointer| format!("/request/payload{pointer}")));
            elide_strings(&mut request.payload, "/request/payload", &mut elided);
        }
        Ok(HoldGetOut { hold, request, elided })
    });
    // Confirming replays the held op. Its read/external phase — for a held `git.commit`, the
    // staging, the user's pre-commit hook and the signature — runs here, before the
    // transaction, as the op's original caller; the transaction rechecks the hold and replays.
    // A held gate is judged again on confirm: what that reads is read here, before the lock.
    engine.register_staged::<Confirm, (Option<Result<crate::engine::Prepared, BusError>>, Probes)>(
        |ctx, payload| {
            let (hold, frozen) = ctx.read(|conn| {
                let hold = guardrail::hold_by_id(conn, payload.hold_id)?;
                let frozen = guardrail::frozen_request(conn, hold.id)?;
                Ok((hold, frozen))
            })?;
            if hold.state != HoldState::Open || frozen.op == grants::OP {
                return Ok((None, Probes::default()));
            }
            if frozen.op == "guardrail.gate" {
                let probes = match serde_json::from_value::<GateIn>(frozen.payload.clone()) {
                    Ok(gate) => warm_gate(ctx, hold.actor.clone(), &gate, Some(&hold.policy)),
                    Err(_) => Probes::default(),
                };
                return Ok((None, probes));
            }
            let prepared = ctx.prepare_registered(&frozen.op, &frozen.payload, hold.actor.clone(), hold.session_id).transpose();
            Ok((prepared, Probes::default()))
        },
        |ctx: &mut Ctx, payload, (prepared, probes)| confirm(ctx, payload, prepared, &probes),
    );
    engine.register::<Reject>(reject);
    engine.register::<ExceptionRequest>(request);
    engine.register::<ExceptionGet>(|ctx, payload| {
        let request = grants::by_id(ctx.tx(), payload.request_id)?;
        assert_visible(ctx, request.project_id)?;
        Ok(request)
    });
    engine.register::<ExceptionsList>(|ctx, payload| {
        // Whether the asking session is still alive decides whether its grant is (RA-105).
        let live = "EXISTS(SELECT 1 FROM sessions s WHERE s.id = holds.session_id AND s.state != 'closed')";
        let mut sql = format!("SELECT holds.*, {live} AS live FROM holds WHERE op = 'guardrail.request'");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(project_id) = visible_project(ctx, payload.project_id)? {
            sql.push_str(" AND project_id = ?");
            args.push(Box::new(project_id));
        }
        if let Some(session) = payload.session {
            sql.push_str(" AND session = ?");
            args.push(Box::new(session));
        }
        let filter = payload.state.unwrap_or(ExceptionFilter::All);
        match filter {
            ExceptionFilter::Open => sql.push_str(" AND state = 'open'"),
            ExceptionFilter::Active => sql.push_str(&format!(" AND state = 'confirmed' AND {live}")),
            ExceptionFilter::All => {}
        }
        sql.push_str(" ORDER BY id DESC LIMIT 500");
        let mut stmt = ctx.tx().prepare(&sql).bus()?;
        let holds = stmt
            .query_map(rusqlite::params_from_iter(args.iter().map(|arg| arg.as_ref())), |row| {
                Ok((guardrail::hold_row(row)?, row.get::<_, bool>("live")?))
            })
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        let requests = holds
            .iter()
            .map(|(hold, live)| {
                let mut request = grants::exception(hold);
                request.active &= *live;
                request
            })
            .filter(|request| filter != ExceptionFilter::Active || request.active)
            .collect();
        Ok(ExceptionsListOut { requests })
    });
    engine.register::<GrantRevoke>(revoke);
}

/// The project an agent's session belongs to, or `None` for a person. An agent sees holds and
/// exception requests in its own project and nothing outside it — the line `session.list`
/// draws (D106, RA-379).
fn actor_project(ctx: &Ctx) -> Result<Option<Id>, BusError> {
    let Some(session_id) = ctx.actor_session_id() else {
        if ctx.actor.is_agent() {
            return Err(BusError::actor("agent actor is not bound to a live session"));
        }
        return Ok(None);
    };
    let own = sessions::by_id(ctx.tx(), session_id)?.ok_or_else(|| BusError::actor("bound session vanished"))?;
    Ok(Some(own.session.project_id))
}

/// The project a list may show: the one asked for, narrowed to an agent's own.
fn visible_project(ctx: &Ctx, asked: Option<Id>) -> Result<Option<Id>, BusError> {
    match (actor_project(ctx)?, asked) {
        (Some(own), Some(asked)) if own != asked => Err(BusError::not_own("project")),
        (own, asked) => Ok(own.or(asked)),
    }
}

fn assert_visible(ctx: &Ctx, project_id: Option<Id>) -> Result<(), BusError> {
    match actor_project(ctx)? {
        Some(own) if project_id != Some(own) => Err(BusError::not_own("project")),
        _ => Ok(()),
    }
}

/// `guardrail.request`: an agent that cannot progress asks a person to let it past one rule.
fn request(ctx: &mut Ctx, payload: ExceptionRequestIn) -> Result<ExceptionRequestOut, BusError> {
    let session = sessions::by_name(ctx.tx(), &payload.session)?;
    if let Some(actor_session_id) = ctx.actor_session_id() {
        if actor_session_id != session.session.id {
            return Err(BusError::not_own("session"));
        }
    }
    grants::validate(payload.kind, &payload.value, &payload.reason)?;
    let project_id = session.session.project_id;
    let session_id = session.session.id;
    let name = session.session.name.clone();
    ctx.set_project(project_id);
    ctx.set_session(session_id);
    let wait_for = grants::RESOLVED_EVENT.to_string();
    // Asking twice is one question. An approval that is still live is the answer already.
    if let Some(request) = grants::existing(ctx.tx(), session_id, payload.kind, &payload.value)? {
        return Ok(ExceptionRequestOut { request, created: false, wait_for });
    }
    let scope = payload.scope.unwrap_or(GrantScope::Once);
    let details = grants::details(payload.kind, &payload.value, &payload.reason, scope);
    let frozen = Request::new(
        ctx.actor.clone(),
        grants::OP,
        serde_json::to_value(&payload).map_err(crate::engine::internal)?,
    )
    .with_id(ctx.req_id);
    let hold_id = grants::insert(
        ctx.tx(), &frozen, project_id, session_id, &name, &details,
        payload.kind, payload.value.trim(), &payload.reason, &ctx.now,
    )?;
    let request = grants::by_id(ctx.tx(), hold_id)?;
    ctx.emit("guardrail.requested", json!({
        "request_id": hold_id, "hold_id": hold_id, "session": name,
        "kind": payload.kind, "value": request.value, "reason": request.reason, "scope": scope,
    }));
    ctx.emit("guardrail.held", json!({
        "hold_id": hold_id, "request_id": hold_id, "session": name, "policy": grants::POLICY,
    }));
    ctx.emit("notify.new", json!({"category": "guardrail", "project_id": project_id, "hold_id": hold_id}));
    Ok(ExceptionRequestOut { request, created: true, wait_for })
}

/// Approve an exception request: no replay, just a grant the session's next gates can use.
fn approve(ctx: &mut Ctx, hold: relay_bus::types::Hold, scope: Option<GrantScope>) -> Result<ConfirmOut, BusError> {
    let requested = grants::exception(&hold);
    let scope = scope.unwrap_or(requested.requested_scope);
    ctx.tx().execute(
        "UPDATE holds SET state = 'confirmed', resolved_at = ?1, resolved_by = ?2,
             details = json_set(details, '$.grant', json(?3))
         WHERE id = ?4 AND state = 'open'",
        params![
            ctx.now, ctx.actor.to_string(),
            json!({"scope": scope, "uses": 0, "used_at": null, "revoked_at": null}).to_string(),
            hold.id,
        ],
    ).bus()?;
    let resolved = guardrail::hold_by_id(ctx.tx(), hold.id)?;
    let exception = grants::exception(&resolved);
    if let Some(project_id) = hold.project_id { ctx.set_project(project_id); }
    if let Some(session_id) = hold.session_id { ctx.set_session(session_id); }
    let by = ctx.actor.to_string();
    ctx.emit("guardrail.resolved", json!({
        "hold_id": hold.id, "request_id": hold.id, "state": "confirmed", "scope": scope, "by": by,
    }));
    ctx.emit(grants::RESOLVED_EVENT, json!({
        "request_id": hold.id, "session": hold.session, "state": "confirmed", "scope": scope,
    }));
    if let (Some(project_id), Some(session)) = (hold.project_id, hold.session.as_deref()) {
        let lasting = match scope {
            GrantScope::Once => "for one use",
            GrantScope::Session => "for the rest of this session",
        };
        if let Some(message) = super::notes::send_system_priority(
            ctx.tx(), project_id, session,
            &format!(
                "Guardrail exception {} approved {lasting} by {}: you may now {}. Retry the action.",
                hold.id, grants::by_line(&ctx.actor), grants::describe(exception.kind, &exception.value),
            ),
            None, &ctx.now,
        )? {
            ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
        }
    }
    let outcome = Response::ok(ctx.req_id, serde_json::to_value(&exception).map_err(crate::engine::internal)?);
    Ok(ConfirmOut { hold: resolved, outcome })
}

/// `guardrail.grant.revoke`: end an approved exception early.
fn revoke(ctx: &mut Ctx, payload: GrantRevokeIn) -> Result<relay_bus::types::GuardrailException, BusError> {
    let exception = grants::by_id(ctx.tx(), payload.request_id)?;
    if !exception.active {
        return Err(BusError::conflict(
            "guardrail.grant_inactive",
            format!("exception {} is not an active grant", exception.id),
        ));
    }
    ctx.tx().execute(
        "UPDATE holds SET details = json_set(details, '$.grant.revoked_at', ?1) WHERE id = ?2",
        params![ctx.now, exception.id],
    ).bus()?;
    let revoked = grants::by_id(ctx.tx(), exception.id)?;
    if let Some(project_id) = exception.project_id { ctx.set_project(project_id); }
    ctx.emit("guardrail.resolved", json!({
        "hold_id": exception.id, "request_id": exception.id, "state": "revoked", "by": ctx.actor.to_string(),
    }));
    ctx.emit(grants::RESOLVED_EVENT, json!({
        "request_id": exception.id, "session": exception.session, "state": "revoked",
    }));
    if let (Some(project_id), Some(session)) = (exception.project_id, exception.session.as_deref()) {
        if let Some(message) = super::notes::send_system_priority(
            ctx.tx(), project_id, session,
            &format!(
                "Guardrail exception {} was revoked by {}: you may no longer {}.",
                exception.id, grants::by_line(&ctx.actor), grants::describe(exception.kind, &exception.value),
            ),
            None, &ctx.now,
        )? {
            ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
        }
    }
    Ok(revoked)
}

/// The grants an agent's session holds, or none for anyone else.
fn session_grants(conn: &Connection, session_id: Option<Id>, agent: bool) -> Result<guardrail::Grants, BusError> {
    match session_id.filter(|_| agent) {
        Some(session_id) => guardrail::Grants::load(conn, session_id),
        None => Ok(guardrail::Grants::default()),
    }
}

/// The read phase of `guardrail.gate` (and of confirming a held one): read the session, its
/// config and grants in one short burst, then everything slow the evaluation could ask for —
/// the old file, `git status`, the staged numstat — with the store lock released. Anything
/// that fails here is simply not cached; the transaction then reports it as it always did.
fn warm_gate(ctx: &Unlocked, actor: Actor, payload: &GateIn, skip_policy: Option<&str>) -> Probes {
    let probes = Probes::default();
    let caller = ctx.actor_session_id();
    let read = ctx.read(|conn| {
        let session = sessions::by_name(conn, &payload.session)?;
        if caller.is_some_and(|id| id != session.session.id) {
            return Err(BusError::not_own("session"));
        }
        let cfg = guardrail::config(conn, Some(session.session.project_id))?;
        let granted = session_grants(conn, Some(session.session.id), actor.is_agent())?;
        Ok((session.session.project_id, PathBuf::from(&session.session.worktree), cfg, granted))
    });
    if let Ok((project_id, worktree, cfg, granted)) = read {
        guardrail::warm(&cfg, &GateRequest {
            actor: &actor,
            project_id,
            worktree: &worktree,
            kind: payload.kind,
            path: payload.path.as_deref(),
            new_text: payload.new_text.as_deref(),
            diff: payload.diff.as_deref(),
            command: payload.command.as_deref(),
            skip_policy,
            grants: None, path_only: false, probes: Some(&probes),
        }, &granted);
    }
    probes
}

fn gate(ctx: &mut Ctx, payload: GateIn, skip_policy: Option<&str>, probes: &Probes) -> Result<GateOut, BusError> {
    let session = sessions::by_name(ctx.tx(), &payload.session)?;
    if let Some(actor_session_id) = ctx.actor_session_id() {
        if actor_session_id != session.session.id {
            return Err(BusError::not_own("session"));
        }
    }
    let project_id = session.session.project_id;
    ctx.set_project(project_id);
    ctx.set_session(session.session.id);
    let (decision, used) = guardrail::evaluate_granted(
        ctx.tx(),
        &GateRequest {
            actor: &ctx.actor,
            project_id,
            worktree: Path::new(&session.session.worktree),
            kind: payload.kind,
            path: payload.path.as_deref(),
            new_text: payload.new_text.as_deref(),
            diff: payload.diff.as_deref(),
            command: payload.command.as_deref(),
            skip_policy,
            grants: None, path_only: false, probes: Some(probes),
        },
        Some(session.session.id),
    )?;
    // A person already confirmed exactly this action: judge it again without the policy they
    // waived, and spend their pass only if it then goes through.
    if let (None, Decision::Hold { policy, details, .. }) = (skip_policy, &decision) {
        if let Some(pass) = find_pass(ctx, session.session.id, policy, &payload, details)? {
            let policy = policy.clone();
            let out = gate(ctx, payload, Some(&policy), probes);
            if out.is_ok() {
                ctx.tx().execute(
                    "UPDATE holds SET details = json_set(details, '$.pass.used_at', ?1) WHERE id = ?2",
                    params![ctx.now, pass],
                ).bus()?;
            }
            return out;
        }
    }
    match decision {
        Decision::Allow => {
            // A command that writes to an Android device takes this session's device lease, or
            // is refused naming the session already using the device (device_lease).
            if let (GateKind::Exec, Some(command)) = (payload.kind, payload.command.as_deref()) {
                super::device_lease::gate_command(ctx, session.session.id, &session.session.name, command)?;
            }
            use_grants(ctx, &used, Some(&payload.session))?;
            Ok(GateOut { verdict: Verdict::Allow, error: None, hold_id: None })
        }
        Decision::Refuse(error) => {
            guardrail::insert_refusal_notification(ctx.tx(), project_id, &ctx.actor, &error, &ctx.now)?;
            ctx.commit_error(None);
            ctx.emit("guardrail.refused", json!({"session": payload.session, "code": error.code}));
            ctx.emit("notify.new", json!({"category": "guardrail", "project_id": project_id}));
            Err(error)
        }
        Decision::Hold { policy, mut error, details } => {
            let frozen = Request::new(
                ctx.actor.clone(),
                "guardrail.gate",
                serde_json::to_value(&payload).map_err(crate::engine::internal)?,
            )
            .with_id(ctx.req_id);
            let hold_id = guardrail::insert_hold(
                ctx.tx(),
                &frozen,
                project_id,
                Some(session.session.id),
                Some(&session.session.name),
                &policy,
                &details,
                &ctx.now,
            )?;
            error = error.with_confirm("guardrail.confirm", json!({"hold_id": hold_id}));
            ctx.commit_error(Some(hold_id));
            ctx.emit("guardrail.held", json!({"hold_id": hold_id, "session": payload.session, "policy": policy}));
            ctx.emit("notify.new", json!({"category": "guardrail", "project_id": project_id, "hold_id": hold_id}));
            Err(error)
        }
    }
}

fn confirm(
    ctx: &mut Ctx, payload: ConfirmIn, prepared: Option<Result<crate::engine::Prepared, BusError>>, probes: &Probes,
) -> Result<ConfirmOut, BusError> {
    let hold = guardrail::hold_by_id(ctx.tx(), payload.hold_id)?;
    if hold.state != HoldState::Open {
        return Err(BusError::conflict(
            "guardrail.hold_resolved",
            format!("hold {} is {:?}", hold.id, hold.state),
        ));
    }
    let frozen = guardrail::frozen_request(ctx.tx(), hold.id)?;
    if frozen.op == grants::OP {
        return approve(ctx, hold, payload.scope);
    }
    if frozen.op != "guardrail.gate" {
        let replayed = match prepared {
            Some(Err(error)) => Err(error),
            prepared => ctx.replay_prepared(
                &frozen.op, frozen.payload.clone(), hold.actor.clone(), hold.session_id, hold.policy.clone(),
                prepared.and_then(Result::ok),
            ),
        };
        let outcome = match replayed {
            Ok(value) => Response::ok(ctx.req_id, value),
            Err(error) => Response::err(ctx.req_id, error),
        };
        ctx.tx().execute(
            "UPDATE holds SET state = 'confirmed', resolved_at = ?1, resolved_by = ?2 WHERE id = ?3 AND state = 'open'",
            params![ctx.now, ctx.actor.to_string(), hold.id],
        ).bus()?;
        let resolved = guardrail::hold_by_id(ctx.tx(), hold.id)?;
        if let Some(project_id) = hold.project_id { ctx.set_project(project_id); }
        if let Some(session_id) = hold.session_id { ctx.set_session(session_id); }
        ctx.audit_as(frozen.op, hold.actor.clone());
        ctx.emit("guardrail.resolved", json!({"hold_id": hold.id, "state": "confirmed", "by": ctx.actor.to_string()}));
        return Ok(ConfirmOut { hold: resolved, outcome });
    }
    let gate_payload: GateIn = serde_json::from_value(frozen.payload.clone())
        .map_err(|e| BusError::schema("guardrail.gate", e))?;
    let session = sessions::by_name(ctx.tx(), &gate_payload.session)?;
    let decision = guardrail::evaluate(
        ctx.tx(),
        &GateRequest {
            // Other policies still judge the original actor. Confirmation skips only the
            // policy named by this hold; protected paths/caps can still refuse.
            actor: &hold.actor,
            project_id: session.session.project_id,
            worktree: Path::new(&session.session.worktree),
            kind: gate_payload.kind,
            path: gate_payload.path.as_deref(),
            new_text: gate_payload.new_text.as_deref(),
            diff: gate_payload.diff.as_deref(),
            command: gate_payload.command.as_deref(),
            skip_policy: Some(&hold.policy),
            grants: None, path_only: false, probes: Some(probes),
        },
    )?;

    // The hook that asked has already failed the action; the agent (or the person's own
    // commit) retries it. A confirmed hold therefore leaves a pass for exactly that action,
    // used once by the next identical gate from the same session (`spend_pass`).
    let allowed = matches!(decision, Decision::Allow);
    let outcome = match decision {
        Decision::Allow => Response::ok(
            ctx.req_id,
            serde_json::to_value(GateOut { verdict: Verdict::Allow, error: None, hold_id: None })
                .map_err(crate::engine::internal)?,
        ),
        Decision::Refuse(error) => Response::err(ctx.req_id, error),
        Decision::Hold { policy, mut error, details } => {
            let nested = Request::new(
                hold.actor.clone(),
                "guardrail.gate",
                frozen.payload.clone(),
            );
            let next_id = guardrail::insert_hold(
                ctx.tx(),
                &nested,
                session.session.project_id,
                Some(session.session.id),
                Some(&session.session.name),
                &policy,
                &details,
                &ctx.now,
            )?;
            error = error.with_confirm("guardrail.confirm", json!({"hold_id": next_id}));
            // A new hold, as gate() would have made it: every surface that follows holds and
            // the notification centre hear of it (RA-380).
            ctx.emit("guardrail.held", json!({"hold_id": next_id, "session": gate_payload.session, "policy": policy}));
            ctx.emit("notify.new", json!({"category": "guardrail", "project_id": session.session.project_id, "hold_id": next_id}));
            Response::err(ctx.req_id, error)
        }
    };
    ctx.tx()
        .execute(
            "UPDATE holds SET state = 'confirmed', resolved_at = ?1, resolved_by = ?2 WHERE id = ?3 AND state = 'open'",
            params![ctx.now, ctx.actor.to_string(), hold.id],
        )
        .bus()?;
    if allowed {
        ctx.tx().execute(
            "UPDATE holds SET details = json_set(details, '$.pass', json(?1)) WHERE id = ?2",
            params![json!({"key": pass_key(&hold.policy, &gate_payload, &hold.details), "used_at": null}).to_string(), hold.id],
        ).bus()?;
    }
    let resolved = guardrail::hold_by_id(ctx.tx(), hold.id)?;
    ctx.set_project(session.session.project_id);
    ctx.set_session(session.session.id);
    ctx.audit_as(frozen.op, hold.actor.clone());
    ctx.emit("guardrail.resolved", json!({"hold_id": hold.id, "state": "confirmed", "by": ctx.actor.to_string()}));
    if let (Some(project_id), Some(session)) = (hold.project_id, hold.session.as_deref()) {
        let text = if allowed {
            format!("Guardrail hold {} was confirmed by {}. Retry the identical action once and it will go through; anything different is checked afresh.", hold.id, ctx.actor)
        } else {
            format!("Guardrail hold {} was confirmed by {}.", hold.id, ctx.actor)
        };
        if let Some(message) = super::notes::send_system_priority(
            ctx.tx(), project_id, session,
            &text,
            None, &ctx.now,
        )? {
            ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
        }
    }
    Ok(ConfirmOut { hold: resolved, outcome })
}

/// What a confirmed gate hold lets through: the same policy held for the same action — same
/// kind, path, text, diff and command, and the same findings (for a commit, which carries none
/// of those, the files and counts the hold named).
fn pass_key(policy: &str, payload: &GateIn, details: &Value) -> String {
    let material = json!([policy, payload.kind, payload.path, payload.new_text, payload.diff, payload.command, details]);
    use sha2::Digest;
    crate::hex(&sha2::Sha256::digest(material.to_string().as_bytes()))
}

/// The unused pass a person's confirmation left for exactly this held action, if any.
fn find_pass(ctx: &Ctx, session_id: Id, policy: &str, payload: &GateIn, details: &Value) -> Result<Option<Id>, BusError> {
    use rusqlite::OptionalExtension;
    ctx.tx().prepare_cached(
        "SELECT id FROM holds WHERE session_id = ?1 AND op = 'guardrail.gate' AND state = 'confirmed'
         AND json_extract(details, '$.pass.key') = ?2 AND json_extract(details, '$.pass.used_at') IS NULL
         ORDER BY id LIMIT 1",
    ).bus()?.query_row(params![session_id, pass_key(policy, payload, details)], |r| r.get(0)).optional().bus()
}

/// Apply the phase-4 policy engine to a mutation that is itself a bus op. A hold freezes
/// that exact op so confirmation can replay it in the same transaction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn enforce(
    ctx: &mut Ctx, project_id: Id, worktree: &Path, kind: GateKind, path: Option<&str>,
    new_text: Option<&str>, diff: Option<&str>, command: Option<&str>,
) -> Result<(), BusError> {
    enforce_request(ctx, project_id, worktree, kind, path, new_text, diff, command, false, None)
}

/// [`enforce`] judging by what [`warm_enforce`] read before the transaction opened, so the
/// transaction only looks it up (D149). Whatever `probes` lacks is read on the spot, as
/// [`enforce`] reads everything; a caller whose probes may be stale passes fresh, empty ones.
#[allow(clippy::too_many_arguments)]
pub(crate) fn enforce_with(
    ctx: &mut Ctx, project_id: Id, worktree: &Path, kind: GateKind, path: Option<&str>,
    new_text: Option<&str>, diff: Option<&str>, command: Option<&str>, probes: &Probes,
) -> Result<(), BusError> {
    enforce_request(ctx, project_id, worktree, kind, path, new_text, diff, command, false, Some(probes))
}

/// The read phase of a staged op that [`enforce_with`]s, as [`warm_gate`] is `guardrail.gate`'s:
/// the project's config and the session's grants in one short read, then everything slow the
/// evaluation could ask for — the old file, `git status` — with the store lock released. A read
/// that fails caches nothing; the transaction then reads it and reports it as it always did.
#[allow(clippy::too_many_arguments)]
pub(crate) fn warm_enforce(
    ctx: &Unlocked, project_id: Id, worktree: &Path, kind: GateKind, path: Option<&str>,
    new_text: Option<&str>, diff: Option<&str>, command: Option<&str>,
) -> Probes {
    let probes = Probes::default();
    let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
    let read = ctx.read(|conn| {
        Ok((guardrail::config(conn, Some(project_id))?, session_grants(conn, session_id, agent)?))
    });
    if let Ok((cfg, granted)) = read {
        guardrail::warm(&cfg, &GateRequest {
            actor: &ctx.actor, project_id, worktree, kind, path, new_text, diff, command,
            skip_policy: None, grants: None, path_only: false, probes: Some(&probes),
        }, &granted);
    }
    probes
}

/// [`enforce`] for a rename, move or delete: the path rules only, never the file's content
/// (BUS.md §9.2). `new_text` is only for a shape gate on the destination.
pub(crate) fn enforce_path_mutation(
    ctx: &mut Ctx, project_id: Id, worktree: &Path, path: &str, new_text: Option<&str>,
) -> Result<(), BusError> {
    enforce_request(ctx, project_id, worktree, GateKind::Write, Some(path), new_text, None, None, true, None)
}

#[allow(clippy::too_many_arguments)]
fn enforce_request(
    ctx: &mut Ctx, project_id: Id, worktree: &Path, kind: GateKind, path: Option<&str>,
    new_text: Option<&str>, diff: Option<&str>, command: Option<&str>, path_only: bool,
    probes: Option<&Probes>,
) -> Result<(), BusError> {
    let session_id = ctx.actor_session_id();
    let (decision, used) = guardrail::evaluate_granted(ctx.tx(), &GateRequest {
        actor: &ctx.actor, project_id, worktree, kind, path, new_text, diff, command,
        skip_policy: ctx.skip_policy(), grants: None, path_only, probes,
    }, session_id)?;
    match decision {
        Decision::Allow => use_grants(ctx, &used, None),
        Decision::Refuse(error) => {
            guardrail::insert_refusal_notification(ctx.tx(), project_id, &ctx.actor, &error, &ctx.now)?;
            ctx.commit_error(None);
            ctx.emit("guardrail.refused", json!({"op": ctx.op, "code": error.code}));
            ctx.emit("notify.new", json!({"category": "guardrail", "project_id": project_id}));
            Err(error)
        }
        Decision::Hold { policy, error, details } => Err(hold_op(ctx, project_id, &policy, error, &details)?),
    }
}

/// Freeze the running bus op as a hold under `policy` and return the `held` error to answer it
/// with. `guardrail.confirm` replays the frozen op with `policy` waived (`Ctx::skip_policy`), so
/// a handler that holds itself must let the request through when that is the policy skipped.
pub(crate) fn hold_op(
    ctx: &mut Ctx, project_id: Id, policy: &str, error: BusError, details: &Value,
) -> Result<BusError, BusError> {
    let frozen = Request::new(ctx.actor.clone(), ctx.op, ctx.payload().clone()).with_id(ctx.req_id);
    let session_id = ctx.actor_session_id();
    let session = session_id.and_then(|id| ctx.tx().query_row(
        "SELECT name FROM sessions WHERE id = ?1", [id], |r| r.get::<_, String>(0),
    ).ok());
    let hold_id = guardrail::insert_hold(
        ctx.tx(), &frozen, project_id, session_id, session.as_deref(), policy, details, &ctx.now,
    )?;
    let error = error.with_confirm("guardrail.confirm", json!({"hold_id": hold_id}));
    ctx.commit_error(Some(hold_id));
    ctx.emit("guardrail.held", json!({"hold_id": hold_id, "op": ctx.op, "policy": policy}));
    ctx.emit("notify.new", json!({"category": "guardrail", "project_id": project_id, "hold_id": hold_id}));
    Ok(error)
}

fn reject(ctx: &mut Ctx, payload: RejectIn) -> Result<RejectOut, BusError> {
    let hold = guardrail::hold_by_id(ctx.tx(), payload.hold_id)?;
    if hold.state != HoldState::Open {
        return Err(BusError::conflict(
            "guardrail.hold_resolved",
            format!("hold {} is {:?}", hold.id, hold.state),
        ));
    }
    ctx.tx()
        .execute(
            "UPDATE holds SET state = 'rejected', resolved_at = ?1, resolved_by = ?2,
                 details = json_set(details, '$.rejection_reason', ?3)
             WHERE id = ?4 AND state = 'open'",
            params![ctx.now, ctx.actor.to_string(), payload.reason, hold.id],
        )
        .bus()?;
    let resolved = guardrail::hold_by_id(ctx.tx(), hold.id)?;
    if let Some(project_id) = hold.project_id {
        ctx.set_project(project_id);
    }
    if let Some(session_id) = hold.session_id {
        ctx.set_session(session_id);
    }
    let is_request = hold.op == grants::OP;
    let reason = payload.reason.as_deref().unwrap_or("no reason provided");
    if let (Some(project_id), Some(session)) = (hold.project_id, hold.session.as_deref()) {
        let text = if is_request {
            let asked = grants::exception(&hold);
            format!(
                "Guardrail exception {} was denied: {reason}. You may not {}; find another way, or report the task blocked.",
                hold.id, grants::describe(asked.kind, &asked.value),
            )
        } else {
            format!("Guardrail hold {} was rejected: {reason}", hold.id)
        };
        if let Some(message) = super::notes::send_system_priority(ctx.tx(), project_id, session, &text, None, &ctx.now)? {
            ctx.emit("mailbox.new", serde_json::to_value(message).bus()?);
        }
    }
    ctx.emit("guardrail.resolved", json!({"hold_id": hold.id, "state": "rejected", "by": ctx.actor.to_string(), "reason": payload.reason}));
    if is_request {
        ctx.emit(grants::RESOLVED_EVENT, json!({
            "request_id": hold.id, "session": hold.session, "state": "rejected", "reason": payload.reason,
        }));
    }
    Ok(RejectOut { hold: resolved })
}

/// Count a use against each grant an allowed action leaned on, and say so.
fn use_grants(ctx: &mut Ctx, used: &[Id], session: Option<&str>) -> Result<(), BusError> {
    if used.is_empty() {
        return Ok(());
    }
    let spent = grants::consume(ctx.tx(), used, &ctx.now)?;
    for id in used {
        ctx.emit("guardrail.grant_used", json!({"request_id": id, "session": session, "op": ctx.op}));
    }
    // A one-use grant ends here; say so on the event every guardrail surface already follows.
    for id in spent {
        ctx.emit("guardrail.resolved", json!({"hold_id": id, "request_id": id, "state": "used"}));
    }
    Ok(())
}

/// `guardrail.holds.list` page size when the caller names none, and the most it may ask for.
const HOLDS_PAGE: u32 = 200;
const HOLDS_PAGE_MAX: u32 = 1000;

/// Cut the large strings in a hold's details, for a list or an inspection.
pub(crate) fn elide_hold(hold: &mut relay_bus::types::Hold, elided: &mut Vec<String>) {
    elide_strings(&mut hold.details, "/hold/details", elided);
}

/// Cut every string in `value` past [`guardrail::SHOWN_STRING_MAX`] bytes, recording each
/// one's JSON pointer.
fn elide_strings(value: &mut Value, pointer: &str, elided: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if guardrail::elide_text(text) {
                elided.push(pointer.to_string());
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                elide_strings(item, &format!("{pointer}/{index}"), elided);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                let key = key.replace('~', "~0").replace('/', "~1");
                elide_strings(item, &format!("{pointer}/{key}"), elided);
            }
        }
        _ => {}
    }
}

fn check_out(decision: Decision) -> CheckOut {
    match decision {
        Decision::Allow => CheckOut { verdict: Verdict::Allow, error: None },
        Decision::Refuse(error) => CheckOut { verdict: Verdict::Refuse, error: Some(error) },
        Decision::Hold { error, .. } => CheckOut { verdict: Verdict::Hold, error: Some(error) },
    }
}
