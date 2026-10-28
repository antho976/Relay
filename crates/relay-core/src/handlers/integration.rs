//! `integration.*` (phase 8): durable, request-driven throwaway merge/build worktrees.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::workspace::get_project;
use crate::worktree;
use relay_bus::error::BusError;
use relay_bus::ops::git::*;
use relay_bus::types::{Id, Integration, IntegrationState};
use relay_bus::Empty;
use rusqlite::{params, OptionalExtension, Row};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The hold policy for an agent asking the engine to run a build (RA-167).
const AGENT_BUILD_POLICY: &str = "agent_build";

/// An agent reaches only its own project's integrations, like every other project-scoped op
/// that names a `project_id` (`not_own("project")`).
fn assert_own_project(ctx: &Ctx, project_id: Id) -> Result<(), BusError> {
    own_project(ctx.tx(), &ctx.actor, ctx.actor_session_id(), project_id)
}

fn own_project(conn: &rusqlite::Connection, actor: &relay_bus::Actor, session: Option<Id>, project_id: Id) -> Result<(), BusError> {
    if let Some(session_id) = session {
        let row = crate::sessions::by_id(conn, session_id)?
            .ok_or_else(|| BusError::actor("bound session no longer exists"))?;
        if row.session.project_id != project_id {
            return Err(BusError::not_own("project"));
        }
    } else if actor.is_agent() {
        return Err(BusError::actor("agent actor is not bound to a live session"));
    }
    Ok(())
}

/// The most branches one integration merges. Each is a ref lookup in the project's repository,
/// and an octopus merge of more is not a test anyone reads.
const MAX_BRANCHES: usize = 32;

pub fn register(e: &mut Engine) {
    // Each branch is looked up in the project's repository, with the store released: opening
    // it and reading refs is file I/O no other request should wait on (D149).
    e.register_staged::<IntegrationRequest, Vec<String>>(|ctx, p| {
        let asked = p.branches.as_ref().map_or(0, Vec::len) + p.sessions.as_ref().map_or(0, Vec::len);
        if asked > MAX_BRANCHES {
            return Err(BusError::invalid("integration.branches", format!("at most {MAX_BRANCHES} branches can be integrated at once; {asked} were asked for")));
        }
        let (project, mut branches) = ctx.read(|conn| {
            let project = get_project(conn, p.project_id)?;
            own_project(conn, &ctx.actor, ctx.actor_session_id(), project.id)?;
            let mut branches = p.branches.clone().unwrap_or_default();
            for session in p.sessions.iter().flatten() {
                let branch: Option<String> = conn.prepare_cached(
                    "SELECT branch FROM sessions WHERE project_id=?1 AND name=?2 AND state!='closed'",
                ).bus()?.query_row(params![project.id, session], |r| r.get(0)).optional().bus()?;
                branches.push(branch.ok_or_else(|| BusError::not_found("integration.session_not_found", format!("no open session {session}")))?);
            }
            Ok((project, branches))
        })?;
        // A deploy is a `device.run`, which is user-only: an agent asking for one here would
        // reach the device through a side door.
        if p.deploy.is_some() && ctx.actor.is_agent() {
            return Err(BusError::allowlist("device.run", "agent")
                .with_hint("deploy is user-only; request the integration without deploy and ask the user to run it"));
        }
        branches.retain(|b| !b.trim().is_empty()); branches.sort(); branches.dedup();
        if branches.len() < 2 { return Err(BusError::invalid("integration.branches", "select at least two distinct session branches")); }
        let repo = gix::open(&project.path).map_err(|e| BusError::unavailable("git.open_failed", e.to_string()))?;
        for branch in &branches {
            // `git merge` would read a ref spelled like an option as one.
            if branch.starts_with('-') { return Err(BusError::invalid("integration.branch", format!("{branch}: a branch name cannot start with '-'"))); }
            // A local branch by its exact name, never a revspec: `:/text` and `@{…}` walk history,
            // and the name goes to `git merge` as it is.
            let full = format!("refs/heads/{branch}");
            if !repo.find_reference(full.as_str()).is_ok_and(|found| found.name().as_bstr() == full.as_str()) {
                return Err(BusError::invalid("integration.branch", format!("{branch}: no such local branch")));
            }
        }
        Ok(branches)
    }, |ctx: &mut Ctx, p, branches| {
        let project = get_project(ctx.tx(), p.project_id)?;
        assert_own_project(ctx, project.id)?;
        // The build is the project's own command, run by the engine outside any sandbox with the
        // user's rights. An agent may ask for one only with a person's say-so, unless the project
        // trusts agent builds (`guardrails.agent_builds`); a merge-only request needs neither.
        let build = p.build.unwrap_or(true);
        if build && ctx.actor.is_agent() && ctx.skip_policy() != Some(AGENT_BUILD_POLICY) {
            let trusted = crate::guardrail::config_for(ctx.tx(), crate::guardrail::ConfigScope::Project(project.id))?.agent_builds;
            if !trusted {
                let command = project.build_cmd.clone().unwrap_or_default();
                let details = json!({"policy": AGENT_BUILD_POLICY, "branches": branches, "build_cmd": command});
                let error = BusError::held("integration.agent_build", format!(
                    "running the build command ({}) for an agent needs a person to confirm it",
                    if command.is_empty() { "none configured" } else { command.as_str() },
                ))
                .with_details(details.clone())
                .with_hint("wait for the user to confirm, or request a merge-only integration with build: false");
                return Err(super::guardrail::hold_op(ctx, project.id, AGENT_BUILD_POLICY, error, &details)?);
            }
        }
        ctx.tx().execute(
            "INSERT INTO integrations(project_id,branches,state,build,deploy,created_at) VALUES (?1,?2,'queued',?3,?4,?5)",
            params![project.id, serde_json::to_string(&branches).bus()?, build as i64, p.deploy, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        let integration = get(ctx.tx(), id)?;
        let parent = ctx.req_id;
        ctx.set_project(project.id);
        ctx.emit("integration.changed", serde_json::to_value(&integration).bus()?);
        let project_id = project.id;
        ctx.after_commit(move |engine| enqueue(engine, project_id, id, parent));
        Ok(integration)
    });
    e.register::<IntegrationGet>(|ctx, p| {
        let integration = get(ctx.tx(), p.integration_id)?;
        assert_own_project(ctx, integration.project_id)?;
        Ok(integration)
    });
    e.register::<IntegrationList>(|ctx, p| {
        get_project(ctx.tx(), p.project_id)?;
        assert_own_project(ctx, p.project_id)?;
        let mut st = ctx
            .tx()
            .prepare_cached("SELECT * FROM integrations WHERE project_id=?1 ORDER BY id DESC LIMIT 100")
            .bus()?;
        let rows = st
            .query_map([p.project_id], row)
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        Ok(IntegrationListOut { integrations: rows })
    });
    // Removing a checkout walks and deletes its build output: seconds to minutes for a large
    // one, so it happens before the transaction opens (D149), which then only records it.
    e.register_staged::<IntegrationDiscard, Option<i64>>(|ctx, p| {
        let (integration, project, in_use) = ctx.read(|conn| {
            let integration = get(conn, p.integration_id)?;
            let project = get_project(conn, integration.project_id)?;
            let in_use = match &integration.worktree {
                Some(path) => runs_in(conn, path)?,
                None => 0,
            };
            Ok((integration, project, in_use))
        })?;
        if in_use > 0 {
            return Err(BusError::conflict("integration.in_use", "a device run is using this integration's checkout")
                .with_hint("stop the run, then discard"));
        }
        // A build still running in the checkout is stopped before the checkout goes.
        stop_runner(ctx.engine(), integration.id, Stop::Discard);
        if let Some(path) = &integration.worktree {
            remove_checkout(Path::new(&project.path), Path::new(path), integration.id)
                .map_err(|e| BusError::conflict("integration.discard_failed", e.to_string()))?;
        }
        Ok(Some(integration.project_id))
    }, |ctx: &mut Ctx, p, project_id| {
        ctx.tx().execute("UPDATE integrations SET state='discarded',finished_at=COALESCE(finished_at,?1) WHERE id=?2", params![ctx.now,p.integration_id]).bus()?;
        if let Some(project_id) = project_id { ctx.set_project(project_id); }
        ctx.emit("integration.changed", json!({"integration_id": p.integration_id,"state":"discarded"}));
        Ok(Empty {})
    });
}

/// How many finished integrations per project keep their checkout for inspection or a device
/// run (D38). Older ones are discarded as each new one finishes.
const KEEP_FINISHED: usize = 3;

/// Integrations waiting for their project's runner, keyed by engine and project. A key is
/// present while a runner thread is working through that project: one integration at a time
/// per project, so repeated "Test together" clicks queue rather than start concurrent
/// checkouts and builds. The runner holds the engine, so its address cannot be reused while
/// the key exists.
type Queues = std::collections::HashMap<(usize, i64), std::collections::VecDeque<(i64, uuid::Uuid)>>;
static QUEUES: std::sync::Mutex<Option<Queues>> = std::sync::Mutex::new(None);

fn enqueue(engine: std::sync::Arc<Engine>, project_id: i64, id: i64, parent: uuid::Uuid) {
    let key = (std::sync::Arc::as_ptr(&engine) as usize, project_id);
    {
        let mut queues = QUEUES.lock().unwrap_or_else(|p| p.into_inner());
        let queues = queues.get_or_insert_with(Default::default);
        if let Some(waiting) = queues.get_mut(&key) {
            waiting.push_back((id, parent));
            return;
        }
        queues.insert(key, Default::default());
    }
    let spawned = std::thread::Builder::new().name(format!("integration-{project_id}")).spawn(move || {
        let mut next = Some((id, parent));
        while let Some((id, parent)) = next {
            run(engine.clone(), id, parent);
            prune(&engine, project_id);
            let mut queues = QUEUES.lock().unwrap_or_else(|p| p.into_inner());
            let queues = queues.get_or_insert_with(Default::default);
            next = queues.get_mut(&key).and_then(|waiting| waiting.pop_front());
            if next.is_none() {
                queues.remove(&key);
            }
        }
    });
    if spawned.is_err() {
        if let Some(queues) = QUEUES.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
            queues.remove(&key);
        }
    }
}

/// Discard every finished integration of `project_id` past the newest [`KEEP_FINISHED`] that
/// still has a checkout, unless a device run is using it. Runs on the integration thread.
fn prune(engine: &Engine, project_id: i64) {
    let doomed: Vec<(i64, String, String)> = {
        let conn = engine.store.lock();
        let rows = conn.prepare_cached(
            "SELECT i.id, i.worktree, p.path FROM integrations i JOIN projects p ON p.id=i.project_id
             WHERE i.project_id=?1 AND i.state IN ('passed','failed','conflict') AND i.worktree IS NOT NULL
             ORDER BY i.id DESC LIMIT -1 OFFSET ?2",
        ).and_then(|mut stmt| stmt.query_map(params![project_id, KEEP_FINISHED as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>());
        match rows {
            Ok(rows) => rows.into_iter().filter(|(_, path, _)| runs_in(&conn, path).is_ok_and(|n| n == 0)).collect(),
            Err(error) => {
                tracing::warn!(project_id, %error, "listing integrations to prune");
                return;
            }
        }
    };
    for (id, path, repo) in doomed {
        if let Err(error) = remove_checkout(Path::new(&repo), Path::new(&path), id) {
            tracing::warn!(integration = id, %error, "pruning an old integration");
            continue;
        }
        let _ = engine.system_write("integration.prune", None, Some(project_id), None, json!({"integration_id": id}), |tx, now| {
            tx.execute("UPDATE integrations SET state='discarded',finished_at=COALESCE(finished_at,?1) WHERE id=?2", params![now, id]).bus()?;
            Ok(((), vec![("integration.changed".into(), json!({"integration_id": id, "state": "discarded"}))]))
        });
    }
}

/// Live device runs whose checkout is `path`.
fn runs_in(conn: &rusqlite::Connection, path: &str) -> Result<i64, BusError> {
    conn.prepare_cached("SELECT COUNT(*) FROM device_runs WHERE worktree=?1 AND state IN ('building','running')").bus()?
        .query_row([path], |r| r.get(0)).bus()
}

/// An integration's checkout, its build output and its `relay/integration-<id>` branch.
pub(crate) fn remove_checkout(repo: &Path, path: &Path, id: i64) -> anyhow::Result<()> {
    if path.exists() {
        worktree::remove(repo, path, true)?;
    }
    let _ = worktree::git_mutate(repo, &["branch", "-D", &format!("relay/integration-{id}")]);
    Ok(())
}

/// The longest a build may run. Generous — a cold Gradle build — but never unbounded (D144).
const BUILD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// One `git merge-tree` probe while naming the branches a failed merge conflicts on.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// How much of a build's output is kept while it runs; the row keeps a shorter tail still.
const OUTPUT_KEEP: usize = 64 * 1024;
/// How often a waiting runner looks for a stop or the deadline.
const STEP_POLL: Duration = Duration::from_millis(100);
/// How long the pipes may stay open after the build's process group is gone: only a daemon
/// that left the group can hold them, and the runner does not wait for it.
const DRAIN_GRACE: Duration = Duration::from_secs(2);
/// How long a discard waits for the runner it stopped to let go of the checkout.
const STOP_WAIT: Duration = Duration::from_secs(10);
/// How long a runner stopped by a discard waits for that discard to record `discarded`
/// (removing a large build output takes a while) before it closes the row itself.
const DISCARD_WAIT: Duration = Duration::from_secs(5 * 60);

/// Why a runner was asked to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Discard,
    Quit,
}

/// A runner at work on one integration: the stop it was asked for, and the process group of
/// the build it is waiting on, so that a stop kills the build at once instead of leaving it
/// running in a checkout that is about to be deleted.
#[derive(Default)]
struct Runner {
    stop: Mutex<Option<Stop>>,
    child: Mutex<Option<u32>>,
}

impl Runner {
    fn stopped(&self) -> Option<Stop> {
        *self.stop.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Runners at work, keyed by engine and integration id, like [`QUEUES`].
type Runners = std::collections::HashMap<(usize, i64), Arc<Runner>>;
static RUNNERS: Mutex<Option<Runners>> = Mutex::new(None);

fn runners() -> std::sync::MutexGuard<'static, Option<Runners>> {
    RUNNERS.lock().unwrap_or_else(|p| p.into_inner())
}

fn engine_key(engine: &Engine) -> usize {
    engine as *const Engine as usize
}

/// A stop for the runner: its build's group is sent SIGTERM now, so a git in it drops its locks
/// (RA-160). The step's own loop sees the stop and finishes the job, SIGKILL included, while its
/// slot still holds the pid: a SIGKILL sent from here could land after that loop reaped the group.
fn signal(runner: &Runner, why: Stop) {
    runner.stop.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(why);
    if let Some(pid) = *runner.child.lock().unwrap_or_else(|p| p.into_inner()) {
        crate::proc::ask_group_to_stop(pid);
    }
}

/// Stop the runner working on integration `id`, if there is one, and wait (bounded) for it to
/// let go of the checkout. Called with the store unlocked.
fn stop_runner(engine: &Engine, id: i64, why: Stop) {
    let key = (engine_key(engine), id);
    let Some(runner) = runners().as_ref().and_then(|r| r.get(&key).cloned()) else {
        return;
    };
    signal(&runner, why);
    let deadline = Instant::now() + STOP_WAIT;
    while Instant::now() < deadline && runners().as_ref().is_some_and(|r| r.contains_key(&key)) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Engine shutdown: kill every build this engine's runners are waiting on, drop what is still
/// queued, and close the rows, so a quit leaves nothing `merging`/`building`/`deploying` to
/// block `project.remove` until the next start (startup recovery closes what a crash leaves).
pub fn shutdown(engine: &Engine) {
    let key = engine_key(engine);
    let live: Vec<Arc<Runner>> = runners()
        .as_ref()
        .map(|r| r.iter().filter(|((e, _), _)| *e == key).map(|(_, runner)| runner.clone()).collect())
        .unwrap_or_default();
    for runner in &live {
        signal(runner, Stop::Quit);
    }
    if let Some(queues) = QUEUES.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        for ((e, _), waiting) in queues.iter_mut() {
            if *e == key {
                waiting.clear();
            }
        }
    }
    let conn = engine.store.lock();
    let _ = conn.execute(
        "UPDATE integrations SET state='failed',finished_at=COALESCE(finished_at,?1),
         log_tail=COALESCE(log_tail,'')||'\nInterrupted: Relay quit before this integration finished.'
         WHERE state IN ('queued','merging','building','deploying')",
        [crate::time::now()],
    );
}

fn is_live(state: IntegrationState) -> bool {
    matches!(state, IntegrationState::Queued | IntegrationState::Merging | IntegrationState::Building | IntegrationState::Deploying)
}

fn state_of(engine: &Engine, id: i64) -> Option<IntegrationState> {
    let conn = engine.store.lock();
    get(&conn, id).ok().map(|i| i.state)
}

fn run(engine: Arc<Engine>, id: i64, parent: uuid::Uuid) {
    let key = (engine_key(&engine), id);
    let runner = Arc::new(Runner::default());
    runners().get_or_insert_with(Default::default).insert(key, runner.clone());
    let driven = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drive(&engine, &runner, id, parent)));
    if let Some(r) = runners().as_mut() {
        r.remove(&key);
    }
    // However the drive ended, the row must not stay live with nothing behind it: it would
    // block project.remove and workspace.remove until the next start.
    let stop = runner.stopped();
    match stop {
        // Shutdown closes the rows itself.
        Some(Stop::Quit) => return,
        // The discard that stopped this run records `discarded` once the checkout is gone.
        Some(Stop::Discard) => {
            let deadline = Instant::now() + DISCARD_WAIT;
            while state_of(&engine, id).is_some_and(is_live) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        None => {}
    }
    if state_of(&engine, id).is_some_and(is_live) {
        let why = match (driven.is_err(), stop) {
            (true, _) => "The integration runner crashed before it finished.",
            (false, Some(_)) => "Stopped for a discard that did not complete.",
            (false, None) => "The integration runner stopped without a result.",
        };
        fail(&engine, id, parent, IntegrationState::Failed, why, None);
    }
}

fn drive(engine: &Arc<Engine>, runner: &Runner, id: i64, parent: uuid::Uuid) {
    let loaded = {
        let conn = engine.store.lock();
        let integration = get(&conn, id);
        let project = integration
            .as_ref()
            .ok()
            .and_then(|i| get_project(&conn, i.project_id).ok());
        integration.ok().zip(project)
    };
    let Some((integration, project)) = loaded else {
        return;
    };
    // Discarded (or closed by a quit) while it waited in the queue.
    if integration.state != IntegrationState::Queued {
        return;
    }
    let repo = PathBuf::from(&project.path);
    let path = repo
        .join(".relay")
        .join("integrations")
        .join(id.to_string());
    let branch = format!("relay/integration-{id}");
    let _ = set_state(
        engine,
        id,
        parent,
        IntegrationState::Merging,
        None,
        "Preparing integration worktree",
        false,
        None,
    );
    // Always a new branch from the base (`-b`, which git refuses when the name is taken), and
    // the checkout is recorded only once it is this run's: a discard or prune force-removes the
    // recorded path and deletes `relay/integration-<id>`, and another instance sharing the
    // repository numbers its integrations from 1 too. Reusing a branch of that name would
    // also have merged into whatever it held instead of the base.
    let created = worktree::create_at(&repo, &path, &branch, &worktree::Start::New(Some(project.base_branch.clone())));
    if let Err(error) = created {
        fail(
            engine,
            id,
            parent,
            IntegrationState::Failed,
            &format!("worktree: {error}"),
            None,
        );
        return;
    }
    let _ = set_state(engine, id, parent, IntegrationState::Merging, Some(&path), "Merging branches", false, None);
    if state_of(engine, id).is_none_or(|state| state == IntegrationState::Discarded) {
        let _ = worktree::remove(&repo, &path, true);
        let _ = worktree::git_mutate(&repo, &["branch", "-D", &branch]);
        return;
    }
    if runner.stopped().is_some() {
        return;
    }
    let merge_args: Vec<&str> = std::iter::once("merge")
        .chain(["--no-ff", "--no-edit", "--"])
        .chain(integration.branches.iter().map(String::as_str))
        .collect();
    if let Err(error) = worktree::git_mutate(&path, &merge_args) {
        let (state, pair, log) = merge_failure(&path, &project.base_branch, &integration.branches, &error.to_string());
        fail(engine, id, parent, state, &log, pair);
        return;
    }
    let build: bool = {
        let conn = engine.store.lock();
        conn.query_row("SELECT build FROM integrations WHERE id=?1", [id], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap_or(0)
            != 0
    };
    let mut log = String::from("Merge passed.\n");
    if build {
        if let Some(command) = project.build_cmd.as_deref() {
            if runner.stopped().is_some() {
                return;
            }
            let _ = set_state(
                engine,
                id,
                parent,
                IntegrationState::Building,
                Some(&path),
                &log,
                false,
                None,
            );
            match step(runner, &path, command, BUILD_TIMEOUT) {
                Step::Passed(text) => log.push_str(&text),
                Step::Failed(text) => {
                    log.push_str(&text);
                    fail(engine, id, parent, IntegrationState::Failed, &log, None);
                    return;
                }
                Step::Stopped => return,
            }
        } else {
            log.push_str("No build command configured.\n");
        }
    }
    let deploy: Option<String> = {
        let conn = engine.store.lock();
        conn.query_row("SELECT deploy FROM integrations WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .ok()
        .flatten()
    };
    if let Some(device) = deploy {
        if runner.stopped().is_some() {
            return;
        }
        // The deploy is a `device.run` of this checkout, so it holds the device lease, targets
        // the device by serial and falls back to the Gradle install like any other run. That op
        // runs only a passed integration: the row passes first, and is finished (or failed)
        // with what the run's start said.
        log.push_str(&format!("Deploying to {device} with device.run.\n"));
        let _ = set_state(
            engine,
            id,
            parent,
            IntegrationState::Passed,
            Some(&path),
            &log,
            false,
            None,
        );
        let started = engine
            .dispatch(
                relay_bus::Request::new(
                    relay_bus::Actor::User,
                    "device.run",
                    json!({"project_id": project.id, "device": device, "integration_id": id}),
                ),
                crate::engine::Door::InProcess,
            )
            .into_result();
        match started {
            Ok(run) => log.push_str(&format!(
                "Device run {} started; it reports through run.changed.\n",
                run["id"]
            )),
            Err(error) => {
                log.push_str(&format!("Deploy refused: {} ({})\n", error.message, error.code));
                fail(engine, id, parent, IntegrationState::Failed, &log, None);
                return;
            }
        }
    }
    let _ = set_state(
        engine,
        id,
        parent,
        IntegrationState::Passed,
        Some(&path),
        &log,
        true,
        None,
    );
}

/// What a build child came to.
enum Step {
    Passed(String),
    Failed(String),
    /// A discard or a quit stopped it; whoever stopped it records the outcome.
    Stopped,
}

/// Run `command` through the login shell in `cwd`, in its own process group, until it exits,
/// [`Runner`]'s stop arrives or `timeout` elapses; either of the last two ends the group, SIGTERM
/// first ([`crate::proc::terminate_group`]).
/// Only the last [`OUTPUT_KEEP`] bytes of its interleaved output are kept.
fn step(runner: &Runner, cwd: &Path, command: &str, timeout: Duration) -> Step {
    let mut cmd = Command::new("fish");
    cmd.current_dir(cwd)
        .args(["-lc", command])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = {
        // Spawned under the child lock: a stop either finds the pid or came first and is seen here.
        let mut slot = runner.child.lock().unwrap_or_else(|p| p.into_inner());
        if runner.stopped().is_some() {
            return Step::Stopped;
        }
        match cmd.spawn() {
            Ok(child) => {
                *slot = Some(child.id());
                child
            }
            Err(error) => return Step::Failed(format!("cannot start fish: {error}\n")),
        }
    };
    let tail = Arc::new(Mutex::new(Vec::new()));
    let (done_tx, done) = std::sync::mpsc::channel::<()>();
    if let Some(pipe) = child.stdout.take() {
        keep_tail(pipe, tail.clone(), done_tx.clone());
    }
    if let Some(pipe) = child.stderr.take() {
        keep_tail(pipe, tail.clone(), done_tx.clone());
    }
    drop(done_tx);
    let deadline = Instant::now() + timeout;
    let outcome = loop {
        let mut slot = runner.child.lock().unwrap_or_else(|p| p.into_inner());
        match child.try_wait() {
            Ok(Some(status)) => {
                *slot = None;
                break Ok(status);
            }
            Ok(None) if runner.stopped().is_none() && Instant::now() < deadline => {}
            waited => {
                // Let go of the slot first: a stop arriving during the grace has nothing to add.
                *slot = None;
                drop(slot);
                crate::proc::terminate_group(child.id(), crate::proc::TERM_GRACE, || !matches!(child.try_wait(), Ok(None)));
                let _ = child.wait();
                break Err(waited.err());
            }
        }
        drop(slot);
        std::thread::sleep(STEP_POLL);
    };
    // Both senders drop at EOF; this returns as soon as they have, or after the grace.
    let _ = done.recv_timeout(DRAIN_GRACE);
    let text = String::from_utf8_lossy(&tail.lock().unwrap_or_else(|p| p.into_inner())).into_owned();
    match outcome {
        // A build that exited on its own after a stop heard the SIGTERM: it is stopped, not
        // passed or failed.
        _ if runner.stopped().is_some() => Step::Stopped,
        Ok(status) if status.success() => Step::Passed(text),
        Ok(status) => Step::Failed(format!("{text}\nBuild command {status}.\n")),
        Err(Some(error)) => Step::Failed(format!("{text}\nwaiting on the build: {error}\n")),
        Err(None) => Step::Failed(format!(
            "{text}\nThe build did not finish within {} minutes and was stopped.\n",
            timeout.as_secs() / 60
        )),
    }
}

/// Read `pipe` on its own thread into `tail`, keeping the newest [`OUTPUT_KEEP`] bytes.
fn keep_tail<R: std::io::Read + Send + 'static>(mut pipe: R, tail: Arc<Mutex<Vec<u8>>>, done: std::sync::mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let _done = done;
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let mut tail = tail.lock().unwrap_or_else(|p| p.into_inner());
                    tail.extend_from_slice(&chunk[..n]);
                    if tail.len() > 2 * OUTPUT_KEEP {
                        let cut = tail.len() - OUTPUT_KEEP;
                        tail.drain(..cut);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
}

/// Why the octopus merge failed. `git merge` reports only that the octopus did not work, so
/// each branch is tried against the base, then against each other branch, with `git
/// merge-tree` (which touches neither the checkout nor a ref) to name the pair that really
/// conflicts. A failure that is no conflict at all (a timeout, a missing ref) is `failed`.
fn merge_failure(path: &Path, base: &str, branches: &[String], error: &str) -> (IntegrationState, Option<(String, String)>, String) {
    let mut pairs: Vec<(&str, &str, &str)> = branches.iter().map(|b| ("HEAD", base, b.as_str())).collect();
    for (i, left) in branches.iter().enumerate() {
        for right in &branches[i + 1..] {
            pairs.push((left, left, right));
        }
    }
    for (left, left_name, right) in pairs {
        if let Some(files) = conflict_between(path, left, right) {
            let log = format!("merge: {error}\n{left_name} and {right} conflict in: {}\n", files.join(", "));
            return (IntegrationState::Conflict, Some((left_name.to_string(), right.to_string())), log);
        }
    }
    let unmerged: Vec<String> = git_probe(path, &["diff", "--name-only", "--diff-filter=U"])
        .map(|(_, out)| out.lines().filter(|l| !l.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();
    if !unmerged.is_empty() {
        return (IntegrationState::Conflict, None, format!("merge: {error}\nUnmerged: {}\n", unmerged.join(", ")));
    }
    (IntegrationState::Failed, None, format!("merge: {error}\n"))
}

/// The files `left` and `right` conflict in, or `None` when they merge cleanly or the probe
/// could not tell. `merge-tree` exits 1 both for a conflict and for a bad ref; only a
/// conflict prints the resulting tree first.
fn conflict_between(path: &Path, left: &str, right: &str) -> Option<Vec<String>> {
    let (code, out) = git_probe(path, &["merge-tree", "--write-tree", "--name-only", "--no-messages", left, right])?;
    let mut lines = out.lines();
    let tree = lines.next()?;
    if code != 1 || tree.len() < 40 || !tree.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut files: Vec<String> = lines.filter(|l| !l.is_empty()).map(str::to_string).collect();
    files.dedup();
    Some(files)
}

fn git_probe(path: &Path, args: &[&str]) -> Option<(i32, String)> {
    let mut command = Command::new("git");
    command.arg("-C").arg(path).args(args);
    let out = crate::proc::output_with_timeout(&mut command, PROBE_TIMEOUT).ok()??;
    Some((out.status.code()?, String::from_utf8_lossy(&out.stdout).into_owned()))
}

fn fail(
    engine: &Engine,
    id: i64,
    parent: uuid::Uuid,
    state: IntegrationState,
    log: &str,
    conflict: Option<(String, String)>,
) {
    let _ = set_state(engine, id, parent, state, None, log, true, conflict);
}

#[allow(clippy::too_many_arguments)] // one transition record; a builder would hide required audit fields
fn set_state(
    engine: &Engine,
    id: i64,
    parent: uuid::Uuid,
    state: IntegrationState,
    path: Option<&Path>,
    log: &str,
    finished: bool,
    conflict: Option<(String, String)>,
) -> Result<(), BusError> {
    let state_s = state_name(state);
    let tail = tail(log, 12_000);
    let project_id = get(&engine.store.lock(), id)?.project_id;
    // A discard is final. It is checked in the write itself: checked under an earlier lock, a
    // discard committing in between was overwritten with a result and a notification (RA-384).
    let written = engine.system_write("integration.advance",Some(parent),Some(project_id),None,json!({"integration_id":id,"state":state_s}),|tx,now|{
        let changed = tx.execute("UPDATE integrations SET state=?1,worktree=COALESCE(?2,worktree),log_tail=?3,conflict=?4,started_at=COALESCE(started_at,?5),finished_at=CASE WHEN ?6 THEN ?5 ELSE finished_at END WHERE id=?7 AND state!='discarded'",params![state_s,path.map(|p|p.display().to_string()),tail,conflict.as_ref().map(|v|serde_json::to_string(v).unwrap()),now,finished as i64,id]).bus()?;
        // Rolled back, audit row and all: nothing happened.
        if changed == 0 { return Err(BusError::conflict(DISCARDED, format!("integration {id} was discarded"))); }
        if finished { tx.execute("INSERT INTO notifications(project_id,category,title,body,link,read,created_at) VALUES (?1,'integration',?2,?3,NULL,0,?4)",params![project_id,format!("Integration {state_s}"),format!("Integration {id}: {}",tail.lines().last().unwrap_or(state_s)),now]).bus()?; }
        let value=serde_json::to_value(get(tx,id)?).bus()?;let mut events=vec![("integration.changed".into(),value.clone())];if finished{events.push(("integration.result".into(),value));events.push(("notify.new".into(),json!({"category":"integration","project_id":project_id,"integration_id":id})));}Ok(((),events))
    });
    match written {
        Err(error) if error.code == DISCARDED => Ok(()),
        written => written,
    }
}

/// The internal refusal [`set_state`] rolls back with when the row was discarded under it.
const DISCARDED: &str = "integration.discarded";

fn get(conn: &rusqlite::Connection, id: i64) -> Result<Integration, BusError> {
    conn.query_row("SELECT * FROM integrations WHERE id=?1", [id], row)
        .optional()
        .bus()?
        .ok_or_else(|| BusError::not_found("integration.not_found", format!("no integration {id}")))
}
fn row(r: &Row) -> rusqlite::Result<Integration> {
    let state = match r.get::<_, String>("state")?.as_str() {
        "queued" => IntegrationState::Queued,
        "merging" => IntegrationState::Merging,
        "building" => IntegrationState::Building,
        "deploying" => IntegrationState::Deploying,
        "passed" => IntegrationState::Passed,
        "failed" => IntegrationState::Failed,
        "conflict" => IntegrationState::Conflict,
        "discarded" => IntegrationState::Discarded,
        _ => IntegrationState::Failed,
    };
    let conflict = r
        .get::<_, Option<String>>("conflict")?
        .and_then(|v| serde_json::from_str(&v).ok());
    Ok(Integration {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        branches: serde_json::from_str(&r.get::<_, String>("branches")?).unwrap_or_default(),
        worktree: r.get("worktree")?,
        state,
        conflict,
        log_tail: r.get("log_tail")?,
        started_at: r.get("started_at")?,
        finished_at: r.get("finished_at")?,
    })
}
fn state_name(state: IntegrationState) -> &'static str {
    match state {
        IntegrationState::Queued => "queued",
        IntegrationState::Merging => "merging",
        IntegrationState::Building => "building",
        IntegrationState::Deploying => "deploying",
        IntegrationState::Passed => "passed",
        IntegrationState::Failed => "failed",
        IntegrationState::Conflict => "conflict",
        IntegrationState::Discarded => "discarded",
    }
}
fn tail(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut start = value.len() - max;
    while !value.is_char_boundary(start) {
        start += 1;
    }
    value[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runner finishing after a discard leaves the row discarded, with no result and no
    /// notification (RA-384).
    #[test]
    fn a_transition_never_overwrites_a_discard() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        engine.store.lock().execute_batch(
            "INSERT INTO workspaces(id,path,name,created_at,updated_at) VALUES (1,'/w','w','t','t');
             INSERT INTO projects(id,workspace_id,path,name,created_at,updated_at) VALUES (1,1,'/w/p','p','t','t');
             INSERT INTO integrations(id,project_id,branches,state,created_at) VALUES (1,1,'[\"a\",\"b\"]','building','t');",
        ).unwrap();
        let mut events = engine.subscribe();
        engine.store.lock().execute("UPDATE integrations SET state='discarded' WHERE id=1", []).unwrap();
        set_state(&engine, 1, uuid::Uuid::new_v4(), IntegrationState::Passed, Some(Path::new("/gone")), "Merge passed.\n", true, None).unwrap();
        let conn = engine.store.lock();
        let integration = get(&conn, 1).unwrap();
        assert_eq!((integration.state, integration.worktree), (IntegrationState::Discarded, None));
        let notified: i64 = conn.query_row("SELECT COUNT(*) FROM notifications", [], |r| r.get(0)).unwrap();
        assert_eq!(notified, 0);
        assert!(events.try_recv().is_err(), "no integration.changed for a transition that did not happen");
    }
}
