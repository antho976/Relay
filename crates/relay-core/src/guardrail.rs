//! Guardrails and session authorization (SPEC §5, BUS.md §9). Evaluation is pure:
//! [`evaluate`] returns allow/refuse/hold. The `guardrail.gate` handler is the only caller
//! that persists a hold; `guardrail.check` never writes.

use crate::engine::IntoBus;
use crate::sessions;
use crate::shell::{self, Word};
use relay_bus::envelope::{Actor, Request};
use relay_bus::error::BusError;
use relay_bus::registry::{Callable, OpEntry, OpKind, Registry};
use relay_bus::types::{
    GateKind, GuardrailConfig, GuardrailLayer, Hold, HoldState, Id, Role, Session, ShapeGate,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

pub mod grants;
pub use grants::Grants;

/// Result of pure policy evaluation. A hold names the one policy confirmation may skip.
///
/// A rewrite whose size Relay merely *inferred* is a judgement call, so it holds and the
/// person who can judge it is one confirmation away. Everything the user stated outright — a
/// cap, a protected path, a denied command, a permitted write root — refuses: re-asking on
/// every occurrence of a rule you already wrote down is how a guardrail becomes noise. Both
/// kinds say what would unblock them.
#[derive(Debug, Clone)]
pub enum Decision {
    Allow,
    Refuse(BusError),
    Hold { policy: String, error: BusError, details: Value },
}

#[derive(Debug, Clone)]
pub struct GateRequest<'a> {
    pub actor: &'a Actor,
    pub project_id: Id,
    pub worktree: &'a Path,
    pub kind: GateKind,
    pub path: Option<&'a str>,
    pub new_text: Option<&'a str>,
    pub diff: Option<&'a str>,
    pub command: Option<&'a str>,
    /// Confirmation skips exactly this policy and no other (BUS.md §9.4).
    pub skip_policy: Option<&'a str>,
    /// Exceptions a person granted this session. Each lifts only the rule it names.
    pub grants: Option<&'a Grants>,
    /// A rename, move or delete rather than a write of new content (BUS.md §9.2): only the
    /// path rules apply — where it lands, protected paths, and a shape gate on a destination
    /// (validated against `new_text` when the caller supplies it). Never reads the file.
    pub path_only: bool,
    /// What the disk and git said, read before the store lock was taken (D149). `None` reads
    /// them on the spot, which is what an in-transaction caller (`file.*`) still does.
    pub probes: Option<&'a Probes>,
}

/// The file a write would replace, as the destructive-write rule can measure it.
#[derive(Debug)]
pub enum OldFile {
    Missing,
    Text(String),
    /// Past the comparison cap: never read into memory.
    TooLarge(u64),
    /// There, but not something a line count can describe: not UTF-8, unreadable, or not a
    /// regular file. It used to read as empty, so replacing it never counted as destructive.
    Opaque(&'static str),
}

/// The slow facts a gate judges by — the old file, git's view of it, the staged numstat —
/// memoized per request. `guardrail.gate`, `.check` and `.explain` fill these with the store
/// lock released; the evaluation inside the transaction then only looks them up (D149).
#[derive(Debug, Default)]
pub struct Probes {
    old: RefCell<HashMap<PathBuf, Arc<OldFile>>>,
    recoverable: RefCell<HashMap<PathBuf, Option<&'static str>>>,
    numstat: RefCell<HashMap<PathBuf, Result<String, BusError>>>,
}

impl GateRequest<'_> {
    fn old_file(&self, path: &Path) -> Arc<OldFile> {
        let absolute = self.worktree.join(path);
        match self.probes {
            Some(probes) => probes.old.borrow_mut().entry(absolute.clone()).or_insert_with(|| Arc::new(read_old(&absolute))).clone(),
            None => Arc::new(read_old(&absolute)),
        }
    }

    fn recoverable(&self, path: &Path) -> Option<&'static str> {
        match self.probes {
            Some(probes) => *probes.recoverable.borrow_mut().entry(self.worktree.join(path)).or_insert_with(|| recoverable(self.worktree, path)),
            None => recoverable(self.worktree, path),
        }
    }

    fn staged_numstat(&self) -> Result<String, BusError> {
        let read = || git_output(self.worktree, &["diff", "--cached", "--numstat", "-z", "--no-renames", "--no-ext-diff"]);
        match self.probes {
            Some(probes) => probes.numstat.borrow_mut().entry(self.worktree.to_path_buf()).or_insert_with(read).clone(),
            None => read(),
        }
    }
}

const COMPARISON_CAP: u64 = 64 * 1024 * 1024;

fn read_old(absolute: &Path) -> OldFile {
    match std::fs::metadata(absolute) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => OldFile::Missing,
        Err(_) => OldFile::Opaque("unreadable"),
        // A FIFO would block the read for good; a directory cannot be overwritten as text.
        Ok(meta) if !meta.is_file() => OldFile::Opaque("not a regular file"),
        Ok(meta) if meta.len() > COMPARISON_CAP => OldFile::TooLarge(meta.len()),
        Ok(_) => match std::fs::read(absolute) {
            Ok(bytes) => String::from_utf8(bytes).map_or(OldFile::Opaque("not UTF-8 text"), OldFile::Text),
            Err(_) => OldFile::Opaque("unreadable"),
        },
    }
}

// ---------------------------------------------------------------- configuration

/// Where a guardrail config is read for. A project's layer sits on its workspace's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Global,
    Workspace(Id),
    Project(Id),
}

/// Read the effective config: defaults <- global settings <- project override <- the
/// project's legacy `protected_paths` / `critical_files` fields.
pub fn config(conn: &Connection, project_id: Option<Id>) -> Result<GuardrailConfig, BusError> {
    let mut cfg = config_for(conn, project_id.map_or(ConfigScope::Global, ConfigScope::Project))?;
    // A project with the Video plugin on has its videos' folders beside the store, written with
    // the agents' own file tools (D167): every session of it may write there, as it may in its
    // checkout, and its agents get the folder with --add-dir at launch. The person's stored
    // settings (`config_for`) do not show it.
    if let Some(id) = project_id {
        let on = crate::plugins::enabled_ids(conn, id).bus()?.iter().any(|p| p == crate::video::PLUGIN);
        if let Some(db) = conn.path().filter(|path| !path.is_empty() && on) {
            if let Some(dir) = Path::new(db).parent() {
                let videos = crate::video::project_dir(&dir.join("videos"), id).display().to_string();
                if !cfg.allowed_write_roots.contains(&videos) {
                    cfg.allowed_write_roots.push(videos);
                }
            }
        }
    }
    Ok(cfg)
}

/// The effective config at one scope: defaults <- global <- the workspace <- the project <- the
/// project's legacy `protected_paths` / `critical_files` columns.
pub fn config_for(conn: &Connection, scope: ConfigScope) -> Result<GuardrailConfig, BusError> {
    let layers = layers(conn, scope)?;
    let root = layers.stages.last().map(|(_, value)| value.clone()).unwrap_or_default();
    typed(root)
}

/// Every layer that applies at a scope: each one's raw stored overrides, and the effective
/// value after it was applied. Read with one pass over the stored `guardrails.*` leaves.
pub struct Layers {
    pub workspace_id: Option<Id>,
    /// `(layer, raw overrides)`, defaults excluded, in application order.
    pub raw: Vec<(GuardrailLayer, Value)>,
    /// `(layer, effective after it)`, defaults first.
    pub stages: Vec<(GuardrailLayer, Value)>,
    /// The project's legacy columns contributed something.
    pub legacy: bool,
}

pub fn layers(conn: &Connection, scope: ConfigScope) -> Result<Layers, BusError> {
    // The project row first: it names the workspace whose layer sits under it.
    let (workspace_id, project) = match scope {
        ConfigScope::Global => (None, None),
        ConfigScope::Workspace(id) => (Some(id), None),
        ConfigScope::Project(project_id) => {
            let row: Option<(Id, String, String)> = conn
                .prepare_cached("SELECT workspace_id, protected_paths, critical_files FROM projects WHERE id = ?1")
                .bus()?
                .query_row([project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()
                .bus()?;
            let Some((workspace_id, protected, critical)) = row else {
                return Err(BusError::not_found("project.not_found", format!("no project {project_id}")));
            };
            (Some(workspace_id), Some((project_id, protected, critical)))
        }
    };
    let workspace_prefix = workspace_id.map(|id| format!("workspaces.{id}"));
    let project_prefix = project.as_ref().map(|(id, _, _)| format!("projects.{id}"));
    let mut global = json!({});
    let mut workspace = json!({});
    let mut own = json!({});
    let mut stmt = conn
        .prepare_cached("SELECT path, value FROM settings WHERE path LIKE 'guardrails.%' ORDER BY path")
        .bus()?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .bus()?;
    for row in rows {
        let (path, raw) = row.bus()?;
        let Some(relative) = path.strip_prefix("guardrails.") else { continue };
        let under = |prefix: &str| -> Option<String> {
            let rest = relative.strip_prefix(prefix)?;
            if rest.is_empty() {
                Some(String::new())
            } else {
                rest.strip_prefix('.').map(str::to_owned)
            }
        };
        let (target, at) = if let Some(at) = workspace_prefix.as_deref().and_then(under) {
            (&mut workspace, at)
        } else if let Some(at) = project_prefix.as_deref().and_then(under) {
            (&mut own, at)
        } else if under("projects").is_some() || under("workspaces").is_some() {
            continue;
        } else {
            (&mut global, relative.to_string())
        };
        let value: Value = serde_json::from_str(&raw).map_err(crate::engine::internal)?;
        // A layer is an object of overrides. A scalar stored as the whole layer (a raw
        // `settings.set` from before writes were checked) used to replace the tree and panic
        // the project read below; now it fails the read, closed, as a bad global key does.
        if at.is_empty() && !(value.is_object() || value.is_null()) {
            return Err(BusError::invalid("guardrail.config", format!("{path} must be an object of overrides, not {value}"))
                .with_hint(format!("settings.reset {{\"path\":\"{path}\"}} clears it")));
        }
        put_raw(target, &at, value);
    }

    let mut current = crate::handlers::settings::defaults()["guardrails"].clone();
    if let Some(map) = current.as_object_mut() {
        map.remove("projects");
        map.remove("workspaces");
    }
    let mut stages = vec![(GuardrailLayer::Default, current.clone())];
    let mut raws = Vec::new();
    let mut apply = |layer: GuardrailLayer, raw: Value, current: &mut Value| {
        crate::handlers::settings::merge_value(current, &raw);
        stages.push((layer, current.clone()));
        raws.push((layer, raw));
    };
    apply(GuardrailLayer::Global, global, &mut current);
    if workspace_id.is_some() {
        apply(GuardrailLayer::Workspace, workspace, &mut current);
    }
    let mut legacy = false;
    if let Some((_, protected, critical)) = project {
        let mut next = current.clone();
        crate::handlers::settings::merge_value(&mut next, &own);
        let mut protected_paths: Vec<String> = serde_json::from_str(&protected).unwrap_or_default();
        legacy |= !protected_paths.is_empty();
        protected_paths.extend(
            next["protected_paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
        protected_paths.sort();
        protected_paths.dedup();
        let not_object = || BusError::invalid("guardrail.config", "a guardrail layer must be an object");
        let map = next.as_object_mut().ok_or_else(not_object)?;
        map.insert("protected_paths".into(), json!(protected_paths));

        let critical: Vec<String> = serde_json::from_str(&critical).unwrap_or_default();
        legacy |= !critical.is_empty();
        let gates = map.get_mut("shape_gates").and_then(Value::as_array_mut).ok_or_else(|| {
            BusError::invalid("guardrail.config", "shape_gates must be an array")
        })?;
        for path in critical {
            if !gates.iter().any(|g| g["path"] == path) {
                gates.push(json!({"path": path, "validator": "non_empty"}));
            }
        }
        stages.push((GuardrailLayer::Project, next));
        raws.push((GuardrailLayer::Project, own));
    }
    Ok(Layers { workspace_id, raw: raws, stages, legacy })
}

/// Deserialize and validate one effective tree.
pub fn typed(mut root: Value) -> Result<GuardrailConfig, BusError> {
    // A stored key this build does not know — a newer build's, or one written before writes
    // were checked — is ignored with a warning instead of failing every guardrail read.
    crate::handlers::settings::ignore_unknown_guardrail_keys(&mut root);
    let cfg: GuardrailConfig = serde_json::from_value(root)
        .map_err(|e| BusError::invalid("guardrail.config", format!("invalid guardrail config: {e}")))?;
    validate_config(&cfg)?;
    Ok(cfg)
}

/// Settings rows are leaves. Rebuild one layer's overrides from them. A stored `null` means
/// nothing; a stored object (the `{}` a cleared subtree leaves behind) merges rather than
/// replacing the subtree, which used to wipe `caps` and fail every read after.
fn put_raw(root: &mut Value, path: &str, value: Value) {
    if value.is_null() {
        return;
    }
    if path.is_empty() {
        crate::handlers::settings::merge_value(root, &value);
        return;
    }
    let mut current = root;
    for part in path.split('.') {
        if !current.is_object() {
            *current = json!({});
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(part.to_string())
            .or_insert(Value::Null);
    }
    if value.is_object() && current.is_object() {
        crate::handlers::settings::merge_value(current, &value);
    } else {
        *current = value;
    }
}

/// Drop every object that a `null` patch emptied, bottom up. An empty object stored as a leaf
/// carries no meaning, and the root itself stays.
pub fn prune_empty(value: &mut Value) {
    if let Value::Object(map) = value {
        for child in map.values_mut() {
            prune_empty(child);
        }
        map.retain(|_, child| !(child.is_null() || child.as_object().is_some_and(|m| m.is_empty())));
    }
}

/// The patch that undoes `before -> after` under [`merge_value`]: keys `after` added become
/// `null`, everything `before` held comes back.
pub fn inverse_patch(before: &Value, after: &Value) -> Value {
    let (Some(before_map), Some(after_map)) = (before.as_object(), after.as_object()) else {
        return before.clone();
    };
    let mut patch = serde_json::Map::new();
    for (key, value) in before_map {
        match after_map.get(key) {
            Some(next) if next == value => {}
            Some(next) if value.is_object() && next.is_object() => {
                patch.insert(key.clone(), inverse_patch(value, next));
            }
            _ => {
                patch.insert(key.clone(), value.clone());
            }
        }
    }
    for key in after_map.keys() {
        if !before_map.contains_key(key) {
            patch.insert(key.clone(), Value::Null);
        }
    }
    Value::Object(patch)
}

/// Leaf paths of an effective tree: objects recurse, arrays and scalars are leaves.
pub fn leaf_paths(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value.as_object() {
        Some(map) if !map.is_empty() => {
            for (key, child) in map {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                leaf_paths(child, &path, out);
            }
        }
        _ => out.push(prefix.to_string()),
    }
}

/// Does a raw override tree set `path` (or an ancestor of it, as a non-object)?
pub fn sets_path(raw: &Value, path: &str) -> bool {
    let mut current = raw;
    for part in path.split('.') {
        match current.get(part) {
            Some(next) if next.is_object() => current = next,
            Some(_) => return true,
            None => return false,
        }
    }
    true
}

pub fn validate_config(cfg: &GuardrailConfig) -> Result<(), BusError> {
    if cfg.caps.files == 0 || cfg.caps.lines == 0 {
        return Err(BusError::invalid("guardrail.config", "caps.files and caps.lines must be > 0"));
    }
    if !(0.0..=100.0).contains(&cfg.destructive_write.min_removed_pct) {
        return Err(BusError::invalid("guardrail.config", "destructive_write.min_removed_pct must be 0..=100"));
    }
    for path in &cfg.protected_paths {
        validate_pattern(path)?;
    }
    // `write_roots` keeps absolute entries only; a `~/scratch` or `build/out` used to be
    // dropped there without a word, and every write to it still refused (RA-319).
    for root in &cfg.allowed_write_roots {
        if !Path::new(root).is_absolute() {
            return Err(BusError::invalid(
                "guardrail.config",
                format!("allowed_write_roots entry {root:?} must be an absolute path (no ~, nothing relative)"),
            ));
        }
    }
    for gate in &cfg.shape_gates {
        validate_pattern(&gate.path)?;
        if !matches!(gate.validator.as_str(), "non_empty" | "json" | "json_non_empty_array" | "json_non_empty_object") {
            return Err(BusError::invalid(
                "guardrail.config",
                format!("unknown shape validator {:?} for {}", gate.validator, gate.path),
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- authorization

/// Layer 2 allowlist + own-session/task scope, the plugin layer, then layer 3 (BUS.md §9.1).
/// Layer 1 is registry metadata and runs immediately before this in the engine.
pub fn authorize(
    conn: &Connection,
    session_id: Id,
    entry: &OpEntry,
    payload: &Value,
) -> Result<(), BusError> {
    let row = sessions::by_id(conn, session_id)?
        .ok_or_else(|| BusError::actor(format!("unknown session id {session_id}")))?;
    let session = &row.session;

    if let Some(name) = payload.get("session").and_then(Value::as_str) {
        let own = name == session.name;
        let pair_read = entry.meta.kind == OpKind::Query && session.pair_with.as_deref() == Some(name);
        if !own && !pair_read {
            // A name that is no live session of this project is a typo or a stale name, not a
            // permission problem, and is answered as one; another project's sessions are not
            // this agent's to know of, so they are not found here either (E-agent-2).
            let here = match sessions::by_name(conn, name) {
                Ok(peer) => peer.session.project_id == session.project_id,
                Err(err) if err.code == "session.not_found" => false,
                Err(err) => return Err(err),
            };
            if !here {
                return Err(no_such_session(conn, session.project_id, name));
            }
            // Reads reach any session in the caller's own project — the same boundary
            // `session.list` draws. Private surfaces (scrollback, brief) narrow it again in the
            // handler; mutations remain own-session, or the PAIR partner, only (D106).
            if entry.meta.kind != OpKind::Query {
                return Err(BusError::not_own("session"));
            }
        }
    }
    if let Some(task_id) = payload.get("task_id").and_then(Value::as_i64) {
        let assigned: bool = conn
            .prepare_cached("SELECT EXISTS(SELECT 1 FROM task_sessions WHERE session_id=?1 AND task_id=?2)")
            .map_err(crate::engine::internal)?
            .query_row(rusqlite::params![session.id, task_id], |record| record.get(0))
            .map_err(crate::engine::internal)?;
        if !assigned {
            return Err(BusError::not_own("task"));
        }
    }
    // The plugin layer, for queries and mutations alike. Only an op a plugin brings reads the
    // switch, so every other request skips the query.
    if entry.meta.plugin.is_some() {
        let on = crate::plugins::enabled_ids(conn, session.project_id).bus()?;
        if let Some(plugin) = plugin_layer(entry, &on) {
            return Err(plugin_off(plugin));
        }
    }
    if entry.meta.kind == OpKind::Query {
        return Ok(());
    }

    // Asking a person is always open: a role that may not act may still say it is stuck.
    if entry.name == grants::OP {
        return Ok(());
    }
    let cfg = config(conn, Some(session.project_id))?;
    let Err(hatch) = layer3(&cfg, session, entry.name) else {
        return Ok(());
    };
    // A role whose shell is otherwise closed (the reviewer) may still run what its instructions
    // depend on: a Codex session reaches the bus only through `$RELAY_BIN q …` in its shell, and
    // refusing that left a Codex reviewer unable to bootstrap, mail or finish (RA-014). The
    // gate still judges the command as it judges every other.
    if entry.name == "guardrail.gate"
        && payload.get("kind").and_then(Value::as_str) == Some("exec")
        && payload.get("command").and_then(Value::as_str).is_some_and(read_only_exec)
    {
        return Ok(());
    }
    let role = sessions::role_str(session.role);
    let mut err = BusError::allowlist(entry.name, role);
    if let Some(hatch) = hatch {
        err = err
            .with_details(json!({"op": entry.name, "role": role, "option": hatch.option()}))
            .with_hint(format!("enable {} for this session deliberately", hatch.option()));
    }
    Err(err)
}

/// `session.not_found` for a `session` an agent named that is no live session of its project,
/// with the nearest name among that project's live sessions, so the refusal reads as the slip it
/// usually is. Never names a session of another project. Runs on the refusal path only.
fn no_such_session(conn: &Connection, project_id: Id, name: &str) -> BusError {
    let names: Vec<String> = conn
        .prepare_cached("SELECT name FROM sessions WHERE project_id=?1 AND state != 'closed' ORDER BY id")
        .and_then(|mut stmt| stmt.query_map([project_id], |row| row.get(0))?.collect())
        .unwrap_or_default();
    let hint = match relay_bus::design::nearest(name, names.iter().map(String::as_str)) {
        Some(near) => format!("did you mean {near:?}?"),
        None => "session.list names this project's live sessions".to_string(),
    };
    BusError::not_found("session.not_found", format!("no live session named {name:?} in this project")).with_hint(hint)
}

/// The two deliberate per-session escape hatches from a role's allowlist (BUS.md §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hatch {
    /// `file.*` and the git index/commit ops, through the bus.
    BusWrites,
    /// `ui.*` and `os.*`.
    AllowUi,
}

impl Hatch {
    fn of(op: &str) -> Option<Hatch> {
        if op.starts_with("file.") || matches!(op, "git.stage" | "git.unstage" | "git.commit") {
            Some(Hatch::BusWrites)
        } else if op.starts_with("ui.") || op.starts_with("os.") {
            Some(Hatch::AllowUi)
        } else {
            None
        }
    }
    fn option(self) -> &'static str {
        match self {
            Hatch::BusWrites => "bus_writes",
            Hatch::AllowUi => "allow_ui",
        }
    }
    fn open(self, session: &Session) -> bool {
        match self {
            Hatch::BusWrites => session.bus_writes,
            Hatch::AllowUi => session.allow_ui,
        }
    }
}

/// Layer 3 for one session: its role's allowlist, then the escape hatch the op belongs to.
/// `Ok(None)` is the allowlist, `Ok(Some(hatch))` an open hatch; `Err` carries the hatch that
/// would admit it, if any. [`authorize`] and [`callability`] both answer from this, so
/// discovery and execution cannot disagree (D104).
fn layer3(cfg: &GuardrailConfig, session: &Session, op: &str) -> Result<Option<Hatch>, Option<Hatch>> {
    if role_admits(cfg, session.role, op) {
        return Ok(None);
    }
    match Hatch::of(op) {
        Some(hatch) if hatch.open(session) => Ok(Some(hatch)),
        hatch => Err(hatch),
    }
}

/// The plugin layer (BUS.md §9.1): an op a plugin brings exists for an agent only in a project
/// with that plugin on. The plugin it lacks, if any, given the ids on for the session's project
/// (`plugins::enabled_ids`). [`authorize`] and [`callability`] both answer from this, as from
/// `layer3`, so discovery and execution cannot disagree (D104).
fn plugin_layer(entry: &OpEntry, plugins_on: &[String]) -> Option<&'static str> {
    entry.meta.plugin.filter(|plugin| !plugins_on.iter().any(|id| id == plugin))
}

/// `refused` / `plugin.off`: the op's plugin is off in the agent's project. Only the person
/// switches a plugin on (`plugin.enable` is theirs), so the hint says to ask.
fn plugin_off(plugin: &str) -> BusError {
    let name = crate::plugins::get(plugin).map_or(plugin, |found| found.manifest.name.as_str());
    BusError::refused("plugin.off", format!("{name} is off for this project"))
        .with_details(json!({"plugin": plugin}))
        .with_hint(format!("ask the person to switch on {name} for this project under Plugins, or do without it"))
}

/// All the gating layers, answered for one op without calling it (BUS.md §9.1). This is
/// the single source `bus.ops`, `relay ops --mine`, the MCP tool list and `session.bootstrap`
/// all read, so discovery can never again promise more than execution delivers (D104).
/// `plugins_on` holds the plugin ids on for the session's project (empty without a session).
pub fn callability(
    entry: &OpEntry,
    actor: &Actor,
    session: Option<&Session>,
    cfg: Option<&GuardrailConfig>,
    plugins_on: &[String],
) -> (Callable, String) {
    // Layer 1: registry metadata.
    if !entry.meta.actors.admits(actor) {
        return (Callable::No, "user_only".to_string());
    }
    let Some(session) = session else {
        return (Callable::Yes, if actor.is_agent() { "agent".into() } else { "user".into() });
    };
    // The plugin layer: an op a plugin brings, where that plugin is off.
    if let Some(plugin) = plugin_layer(entry, plugins_on) {
        return (Callable::No, format!("plugin:{plugin} is off for this project"));
    }
    let role = sessions::role_str(session.role);
    // Layer 2: row scope. An op that names a session or a task only ever acts on this one.
    let scoped = Registry::global().payload_has(entry.name, "session")
        || Registry::global().payload_has(entry.name, "task_id");
    let self_only = |why: String| {
        (if scoped { Callable::SelfOnly } else { Callable::Yes }, why)
    };
    if entry.meta.kind == OpKind::Query {
        return self_only(if scoped { "own session only".into() } else { "query".into() });
    }
    if entry.name == grants::OP {
        return self_only("every agent may ask for an exception".to_string());
    }
    // Layer 3: the role allowlist, then the two deliberate per-session escape hatches.
    let Some(cfg) = cfg else {
        return (Callable::No, "guardrail config unavailable".to_string());
    };
    match layer3(cfg, session, entry.name) {
        Ok(None) => self_only(format!("role:{role} allowlist")),
        Ok(Some(hatch)) => self_only(format!("session.{}", hatch.option())),
        Err(None) => (Callable::No, format!("role:{role} not in allowlist")),
        Err(Some(hatch)) => (Callable::No, format!("role:{role} not in allowlist (needs session.{})", hatch.option())),
    }
}

/// The allow-set for one role, as configured (BUS.md §9.1 layer 3).
pub fn role_allowlist(cfg: &GuardrailConfig, role: Role) -> &Vec<String> {
    match role {
        Role::Builder => &cfg.roles.builder,
        Role::Reviewer => &cfg.roles.reviewer,
        Role::Docs => &cfg.roles.docs,
    }
}

/// Ops every role may call whatever its allowlist says, so a saved list that predates them
/// cannot shut them out. Designs are the person's to ask for and to undo (every `design.apply`
/// carries its inverse, and the board's Undo replays it); `design.purge` stays the person's
/// alone by its registry actors. Videos likewise (D167): every change to one is undone by its
/// inverse or a version, and the Video plugin decides where they exist at all.
pub const OPEN_TO_EVERY_ROLE: &[&str] = &["design.*", "video.*"];

/// Would layer 3 admit `op` for `role`? The escape hatches (`bus_writes`, `allow_ui`) are a
/// property of the session, not the role, so callers apply those separately.
pub fn role_admits(cfg: &GuardrailConfig, role: Role, op: &str) -> bool {
    OPEN_TO_EVERY_ROLE.iter().any(|pattern| op_matches(pattern, op))
        || role_allowlist(cfg, role).iter().any(|pattern| op_matches(pattern, op))
}

pub fn op_matches(pattern: &str, op: &str) -> bool {
    pattern == op
        || pattern == "*"
        || pattern
            .strip_suffix(".*")
            .map(|prefix| op.starts_with(prefix) && op.as_bytes().get(prefix.len()) == Some(&b'.'))
            .unwrap_or(false)
}

// ---------------------------------------------------------------- policy evaluation

pub fn evaluate(conn: &Connection, request: &GateRequest<'_>) -> Result<Decision, BusError> {
    evaluate_with(&config(conn, Some(request.project_id))?, request)
}

/// [`evaluate`] against a config already read: touches no connection, so it can run with the
/// store lock released.
pub fn evaluate_with(cfg: &GuardrailConfig, request: &GateRequest<'_>) -> Result<Decision, BusError> {
    match request.kind {
        GateKind::Write => evaluate_write(cfg, request),
        GateKind::Commit => evaluate_commit(cfg, request),
        GateKind::Exec => evaluate_exec(cfg, request),
    }
}

/// [`evaluate`], and when that would not allow an agent's action, once more with the grants
/// its session holds. The common path pays nothing for grants. Returns the decision and the
/// grants it leaned on — consume those only if the action then really happens.
pub fn evaluate_granted(
    conn: &Connection,
    request: &GateRequest<'_>,
    session_id: Option<Id>,
) -> Result<(Decision, Vec<Id>), BusError> {
    let cfg = config(conn, Some(request.project_id))?;
    let first = evaluate_with(&cfg, request)?;
    let (Some(session_id), true) = (session_id, request.actor.is_agent()) else {
        return Ok((first, Vec::new()));
    };
    if matches!(first, Decision::Allow) {
        return Ok((first, Vec::new()));
    }
    let grants = Grants::load(conn, session_id)?;
    granted_retry(&cfg, request, first, &grants)
}

/// The second half of [`evaluate_granted`], for a caller that loaded the grants itself.
pub fn granted_retry(
    cfg: &GuardrailConfig,
    request: &GateRequest<'_>,
    first: Decision,
    grants: &Grants,
) -> Result<(Decision, Vec<Id>), BusError> {
    if matches!(first, Decision::Allow) || grants.is_empty() || !request.actor.is_agent() {
        return Ok((first, Vec::new()));
    }
    let second = evaluate_with(cfg, &GateRequest { grants: Some(grants), ..request.clone() })?;
    let used = if matches!(second, Decision::Allow) { grants.used() } else { Vec::new() };
    Ok((second, used))
}

/// Read, with nothing locked, every slow fact `request` could be judged by — as itself, with
/// the session's grants, and with the policy a hold names waived (a confirmed pass) — so the
/// evaluation that follows inside the transaction finds them all in `probes`.
pub fn warm(cfg: &GuardrailConfig, request: &GateRequest<'_>, grants: &Grants) {
    let Ok(first) = evaluate_with(cfg, request) else { return };
    if let Decision::Hold { policy, .. } = &first {
        let _ = evaluate_with(cfg, &GateRequest { skip_policy: Some(policy), ..request.clone() });
    }
    let _ = granted_retry(cfg, request, first, grants);
}

fn granted_path(request: &GateRequest<'_>, path: &Path) -> bool {
    request.grants.is_some_and(|grants| grants.covers_path(request.worktree, path))
}

/// A write that lands in the worktree: the worktree-relative path it really reaches, symlinks
/// followed, and the path as it was spelled when that differs. Both are judged: a protected
/// path stays protected whether it is reached through `./`, `a//b` or a symlink (RA-101).
struct Target {
    path: PathBuf,
    written: Option<PathBuf>,
}

fn evaluate_write(cfg: &GuardrailConfig, request: &GateRequest<'_>) -> Result<Decision, BusError> {
    let raw_path = request.path.ok_or_else(|| {
        BusError::invalid("guardrail.path", "kind=write requires path")
    })?;
    let target = match write_target(cfg, request, raw_path)? {
        WriteTarget::InWorktree(target) => target,
        WriteTarget::Scratch => return Ok(Decision::Allow),
        WriteTarget::Outside(decision) => return Ok(*decision),
    };
    let path = target.path.as_path();
    let written = target.written.as_deref();
    let matches = |pattern: &str| path_matches(pattern, path) || written.is_some_and(|w| path_matches(pattern, w));
    let resolved_protected = cfg.protected_paths.iter().any(|p| path_matches(p, path));
    if let Some(pattern) = cfg.protected_paths.iter().find(|p| matches(p)) {
        // A grant names where the write lands; one for the spelling only counts when the place
        // it lands is not itself protected.
        let granted = granted_path(request, path)
            || !resolved_protected && written.is_some_and(|w| granted_path(request, w));
        if request.skip_policy != Some("protected_path") && !granted {
            let shown = match written {
                Some(w) => format!("{} (which is {})", w.display(), path.display()),
                None => path.display().to_string(),
            };
            return Ok(refuse_or_user_hold(
                request.actor,
                "protected_path",
                "guardrail.protected_path",
                format!("{shown} is protected by {pattern:?}"),
                json!({"path": path, "written": written, "pattern": pattern}),
            ));
        }
    }
    let gate = cfg.shape_gates.iter().find(|g| matches(&g.path));
    if request.path_only {
        if let Some(gate) = gate {
            if request.skip_policy != Some("shape_gate") {
                let Some(new_text) = request.new_text else {
                    return Ok(hold(
                        "shape_gate",
                        "guardrail.shape_gate",
                        format!("{} needs complete text for validator {}", path.display(), gate.validator),
                        json!({"path": path, "validator": gate.validator, "reason": "new_text missing"}),
                    ));
                };
                if let Err(reason) = validate_shape(gate, new_text) {
                    return Ok(refuse_or_user_hold(
                        request.actor,
                        "shape_gate",
                        "guardrail.shape_gate",
                        format!("{} failed {}: {reason}", path.display(), gate.validator),
                        json!({"path": path, "validator": gate.validator, "reason": reason}),
                    ));
                }
            }
        }
        return Ok(Decision::Allow);
    }
    let Some(new_text) = request.new_text else {
        // Edit hooks may only provide a diff. Destructive counts still work from it; shape
        // gates need complete text, so a critical path without text is held conservatively.
        if let Some(gate) = gate {
            if request.skip_policy != Some("shape_gate") {
                return Ok(hold(
                    "shape_gate",
                    "guardrail.shape_gate",
                    format!("{} needs complete text for validator {}", path.display(), gate.validator),
                    json!({"path": path, "validator": gate.validator, "reason": "new_text missing"}),
                ));
            }
        }
        if let Some(diff) = request.diff {
            // The file itself is right there. Measuring it is strictly better than inferring
            // a size from the diff, which is what made every diff-only write read as 100%.
            let (removed, added) = diff_counts(diff);
            // A diff that removes nothing is under every limit whatever the file's size, so
            // the file is not read just to be discarded (RA-378).
            let old_lines = match removed {
                0 => None,
                _ => match &*request.old_file(path) {
                    OldFile::Text(text) => Some(text.lines().count() as u32),
                    _ => None,
                },
            };
            return destructive_decision(cfg, request, path, removed, added, old_lines);
        }
        return Err(BusError::invalid(
            "guardrail.write",
            "kind=write requires new_text or diff",
        ));
    };

    if let Some(gate) = gate {
        if request.skip_policy != Some("shape_gate") {
            if let Err(reason) = validate_shape(gate, new_text) {
                return Ok(refuse_or_user_hold(
                    request.actor,
                    "shape_gate",
                    "guardrail.shape_gate",
                    format!("{} failed {}: {reason}", path.display(), gate.validator),
                    json!({"path": path, "validator": gate.validator, "reason": reason}),
                ));
            }
        }
    }

    let old = request.old_file(path);
    let old = match &*old {
        OldFile::Missing => "",
        OldFile::Text(text) => text.as_str(),
        OldFile::TooLarge(bytes) => {
            return Ok(unmeasured(
                cfg, request, path,
                format!("{} is larger than the 64 MiB comparison cap", path.display()),
                json!({"path": path, "bytes": bytes, "reason": "comparison_cap"}),
            ));
        }
        // Replacing bytes Relay cannot count in lines is the whole file going, not none of it.
        OldFile::Opaque(why) => {
            return Ok(unmeasured(
                cfg, request, path,
                format!("{} replaces a file whose content cannot be compared ({why})", path.display()),
                json!({"path": path, "reason": why}),
            ));
        }
    };
    let (removed, added) = changed_line_counts(old, new_text);
    destructive_decision(
        cfg,
        request,
        path,
        removed,
        added,
        Some(old.lines().count() as u32),
    )
}

/// A rewrite of a file that cannot be measured is judged as a destructive one: a confirmed
/// hold, a path grant or git being able to restore it lets it through, nothing else.
fn unmeasured(cfg: &GuardrailConfig, request: &GateRequest<'_>, path: &Path, message: String, details: Value) -> Decision {
    if request.skip_policy == Some("destructive_write") || granted_path(request, path) {
        return Decision::Allow;
    }
    if cfg.destructive_write.allow_if_recoverable && request.recoverable(path).is_some() {
        return Decision::Allow;
    }
    refuse_or_user_hold(request.actor, "destructive_write", "guardrail.destructive_write", message, details)
}

/// Where a write is aimed. Scratch space outside the worktree is allowed outright: it is not
/// a repo-integrity concern, and refusing it puts an agent between two mandatory systems (D102).
enum WriteTarget {
    InWorktree(Target),
    Scratch,
    Outside(Box<Decision>),
}

/// Absolute roots this session may write to, worktree first. The process temp directory is
/// always included, which is where every provider harness puts its own scratchpad.
pub fn write_roots(cfg: &GuardrailConfig, worktree: &Path) -> Vec<PathBuf> {
    let mut roots = vec![worktree.to_path_buf()];
    if let Some(home) = directories::BaseDirs::new() {
        let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from)
            .unwrap_or_else(|| home.home_dir().join(".codex"));
        roots.push(codex.join("memories"));
        let claude = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from)
            .unwrap_or_else(|| home.home_dir().join(".claude"));
        // Claude shares auto-memory across a repository's worktrees. Keep the
        // older per-checkout location usable too, without granting transcripts.
        // https://code.claude.com/docs/en/memory
        // A linked worktree's common dir reads `.git/worktrees/<name>/../..`; resolve it before
        // taking the parent, or the parent is `.git/worktrees` rather than the primary checkout.
        let primary = gix::open(worktree).ok().and_then(|repo| {
            let common = std::fs::canonicalize(repo.common_dir()).ok()?;
            common.parent().map(Path::to_path_buf)
        });
        for project in std::iter::once(worktree).chain(primary.as_deref()) {
            let project = std::fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
            let project_key: String = project.to_string_lossy().chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
            roots.push(claude.join("projects").join(project_key).join("memory"));
        }
    }
    roots.extend(cfg.allowed_write_roots.iter().map(PathBuf::from).filter(|path| path.is_absolute()));
    roots.push(std::env::temp_dir());
    let mut seen = std::collections::HashSet::new();
    roots.retain(|root| seen.insert(root.clone()));
    roots
}

/// Where an absolute path really leads: every symlink along it followed (dangling ones too,
/// which is where a write through one would create the file), `.` and `..` applied, and the
/// parts that do not exist yet kept as written — `realpath -m`. A path whose links loop is
/// returned as far as it got.
pub(crate) fn resolve(path: &Path) -> PathBuf {
    let mut todo: Vec<OsString> = Vec::new();
    push_parts(&mut todo, path);
    let mut out = PathBuf::from("/");
    let mut hops = 0;
    while let Some(part) = todo.pop() {
        if part == "." || part.is_empty() {
            continue;
        }
        if part == ".." {
            out.pop();
            continue;
        }
        let next = out.join(&part);
        match std::fs::symlink_metadata(&next) {
            Ok(meta) if meta.file_type().is_symlink() && hops < 40 => {
                hops += 1;
                let Ok(link) = std::fs::read_link(&next) else {
                    out = next;
                    continue;
                };
                if link.is_absolute() {
                    out = PathBuf::from("/");
                }
                push_parts(&mut todo, &link);
            }
            _ => out = next,
        }
    }
    out
}

/// Push `path`'s parts onto a stack so the first part pops first.
fn push_parts(todo: &mut Vec<OsString>, path: &Path) {
    let parts: Vec<OsString> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_os_string()),
            Component::ParentDir => Some(OsString::from("..")),
            _ => None,
        })
        .collect();
    todo.extend(parts.into_iter().rev());
}

/// A worktree-relative path with `.` parts and doubled separators taken out.
fn normalized(path: &Path) -> PathBuf {
    path.components().filter(|c| !matches!(c, Component::CurDir)).collect()
}

fn write_target(
    cfg: &GuardrailConfig,
    request: &GateRequest<'_>,
    raw_path: &str,
) -> Result<WriteTarget, BusError> {
    let candidate = Path::new(raw_path);
    let absolute = if candidate.is_absolute() {
        if candidate.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(BusError::invalid(
                "guardrail.path",
                format!("{raw_path:?} must contain no .."),
            ));
        }
        candidate.to_path_buf()
    } else {
        request.worktree.join(relative_path(raw_path)?)
    };
    let as_written = absolute.strip_prefix(request.worktree).ok().map(normalized);
    if as_written.as_ref().is_some_and(|relative| relative.as_os_str().is_empty()) {
        return Err(BusError::invalid("guardrail.path", "write target is the worktree itself"));
    }
    // Where the write really lands. A rename, move or delete acts on the last part itself, so
    // only the directories above it are followed; a write follows the file's own link too.
    let landing = match (request.path_only, absolute.parent(), absolute.file_name()) {
        (true, Some(parent), Some(name)) => resolve(parent).join(name),
        _ => resolve(&absolute),
    };
    let worktree = resolve(request.worktree);
    if let Ok(relative) = landing.strip_prefix(&worktree) {
        if relative.as_os_str().is_empty() {
            return Err(BusError::invalid("guardrail.path", "write target is the worktree itself"));
        }
        let path = relative.to_path_buf();
        let written = as_written.filter(|written| *written != path);
        return Ok(WriteTarget::InWorktree(Target { path, written }));
    }
    // Outside the worktree — as written, or because a symlink inside it leads out. Judged by
    // where it lands: a link in scratch space must not reach the repository or anywhere else.
    let roots = write_roots(cfg, request.worktree);
    if roots.iter().skip(1).any(|root| landing.starts_with(root) || landing.starts_with(resolve(root))) {
        return Ok(WriteTarget::Scratch);
    }
    // A person let this session write here. It is outside the repository, so nothing below
    // (protected paths, shape gates, rewrite size) has anything left to judge.
    if request.grants.is_some_and(|grants| grants.covers_root(&landing)) {
        return Ok(WriteTarget::Scratch);
    }
    let listed: Vec<String> = roots.iter().map(|root| root.display().to_string()).collect();
    let shown = if landing == absolute { raw_path.to_string() } else { format!("{raw_path} (which is {})", landing.display()) };
    Ok(WriteTarget::Outside(Box::new(refuse_or_user_hold(
        request.actor,
        "write_root",
        "guardrail.write_root",
        format!(
            "{shown} is outside every root this session may write to ({})",
            listed.join(", ")
        ),
        json!({"path": landing, "written": raw_path, "write_roots": listed}),
    ))))
}

/// Can git put this file back exactly as it is now? Returns why, or `None` when the content
/// would be lost for good. Committed-and-unmodified is restorable with `git checkout --`.
/// Untracked, modified and ignored files carry bytes that exist nowhere else — an ignored file
/// is as often `.env`, local config or Terraform state as it is build output, and git cannot
/// bring any of them back (D114, RA-094).
fn recoverable(worktree: &Path, path: &Path) -> Option<&'static str> {
    // A file in a linked repository is that repository's to judge: the checkout excludes
    // `.relay/`, so its own status would call every file there ignored.
    let linked = crate::linked::containing(worktree, path);
    let (worktree, path) = match &linked {
        Some((repo, inner)) => (repo.as_path(), inner.as_path()),
        None => (worktree, path),
    };
    let out = crate::proc::output_with_timeout(
        Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["status", "--porcelain", "--ignored=matching", "--untracked-files=all", "--"])
            .arg(path),
        GIT_TIMEOUT,
    )
    .ok()??;
    if !out.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&out.stdout);
    match status.trim_end().lines().next() {
        // No status line at all: git is tracking it and it matches HEAD.
        None => Some("committed and unmodified"),
        // Untracked, ignored, staged, or modified — the current bytes exist only here.
        Some(_) => None,
    }
}

fn destructive_decision(
    cfg: &GuardrailConfig,
    request: &GateRequest<'_>,
    path: &Path,
    removed: u32,
    added: u32,
    old_lines: Option<u32>,
) -> Result<Decision, BusError> {
    let limits = &cfg.destructive_write;
    // A share of the old file is only meaningful when there was a file to take a share of.
    // With no known size — a hook that supplied a diff and nothing else — the old code
    // substituted the removed count, which made every such write exactly 100% and therefore
    // always destructive. The absolute rule still applies; the percentage one stays quiet.
    let removed_pct = old_lines
        .filter(|old| *old >= limits.min_file_lines.max(1))
        .map(|old| removed as f64 * 100.0 / old as f64);
    let over_lines = removed > limits.min_removed_lines;
    let over_pct = removed_pct.is_some_and(|pct| pct > limits.min_removed_pct);
    if (over_lines || over_pct) && request.skip_policy != Some("destructive_write") && !granted_path(request, path) {
        // Volume is a proxy; what actually matters is whether the work can come back. Asked
        // only here, on the path that was about to block, so the common write pays nothing.
        if limits.allow_if_recoverable {
            if let Some(reason) = request.recoverable(path) {
                tracing::debug!(path = %path.display(), reason, "large rewrite allowed: git can restore it");
                return Ok(Decision::Allow);
            }
        }
        let share = match removed_pct {
            Some(pct) => format!(" ({pct:.1}% of the file)"),
            None => String::new(),
        };
        let breached = if over_lines {
            format!("more than {} lines", limits.min_removed_lines)
        } else {
            format!("more than {:.1}% of a file of {} lines or more", limits.min_removed_pct, limits.min_file_lines)
        };
        return Ok(refuse_or_user_hold(
            request.actor,
            "destructive_write",
            "guardrail.destructive_write",
            format!("{} removes {removed} lines{share}: {breached}", path.display()),
            json!({
                "path": path, "removed_lines": removed, "added_lines": added,
                "old_lines": old_lines, "removed_pct": removed_pct,
                "limit_lines": limits.min_removed_lines,
                "limit_pct": limits.min_removed_pct,
                "min_file_lines": limits.min_file_lines,
            }),
        ));
    }
    Ok(Decision::Allow)
}

/// One file a commit changes, from `git diff --numstat`: its counts and every path it names
/// (both sides of a rename).
#[derive(Debug, PartialEq)]
struct NumstatEntry {
    added: u32,
    removed: u32,
    paths: Vec<String>,
}

/// Read numstat output. Relay's own probe asks for `-z --no-renames`, where each record is
/// `added\tremoved\tpath\0` with the path verbatim. A caller-supplied numstat may be the
/// plain form instead: a path git C-quoted (`"caf\303\251.txt"`) is unquoted, and a rename
/// (`old => new`, `src/{a => b}/x`) names both sides — the old reading saw neither, so a
/// renamed or non-ASCII protected path never matched (RA-095).
fn parse_numstat(text: &str) -> Vec<NumstatEntry> {
    let counts = |field: Option<&str>| field.and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
    let mut entries = Vec::new();
    if text.contains('\0') {
        let mut records = text.split('\0');
        while let Some(record) = records.next() {
            let mut fields = record.splitn(3, '\t');
            let (added, removed) = (counts(fields.next()), counts(fields.next()));
            let Some(path) = fields.next() else { continue };
            let paths = if path.is_empty() {
                // `-z` with renames on: the two sides follow as records of their own.
                records.by_ref().take(2).filter(|p| !p.is_empty()).map(str::to_owned).collect()
            } else {
                vec![path.to_string()]
            };
            entries.push(NumstatEntry { added, removed, paths });
        }
        return entries;
    }
    for line in text.lines() {
        let mut fields = line.splitn(3, '\t');
        let (added, removed) = (counts(fields.next()), counts(fields.next()));
        let Some(path) = fields.next() else { continue };
        entries.push(NumstatEntry { added, removed, paths: rename_sides(&unquote_c(path)) });
    }
    entries
}

/// Git's C-style path quoting undone: `"a\tb\303\251"` is `a<TAB>bé`. Unquoted text is as is.
fn unquote_c(path: &str) -> String {
    let Some(inner) = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')) else {
        return path.to_string();
    };
    let bytes = inner.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        let octal = bytes.get(i + 1..i + 4).filter(|d| d.iter().all(|b| (b'0'..=b'7').contains(b)));
        if let Some(digits) = octal {
            out.push(digits.iter().fold(0u8, |acc, d| acc.wrapping_mul(8).wrapping_add(d - b'0')));
            i += 4;
            continue;
        }
        out.push(match next {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'a' => 7,
            b'b' => 8,
            b'f' => 12,
            b'v' => 11,
            other => other,
        });
        i += 2;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Both sides of a numstat rename: `old => new`, or `pre{old => new}post`. Anything else is
/// one path.
fn rename_sides(path: &str) -> Vec<String> {
    if let (Some(open), Some(close)) = (path.find('{'), path.rfind('}')) {
        if open < close {
            if let Some((old, new)) = path[open + 1..close].split_once(" => ") {
                let (pre, post) = (&path[..open], &path[close + 1..]);
                let join = |middle: &str| format!("{pre}{middle}{post}").replace("//", "/");
                return vec![join(old), join(new)];
            }
        }
    }
    match path.split_once(" => ") {
        Some((old, new)) => vec![old.to_string(), new.to_string()],
        None => vec![path.to_string()],
    }
}

fn evaluate_commit(cfg: &GuardrailConfig, request: &GateRequest<'_>) -> Result<Decision, BusError> {
    let numstat = match request.diff {
        Some(diff) => diff.to_string(),
        None => request.staged_numstat()?,
    };
    let mut files = 0u32;
    let mut lines = 0u32;
    let mut touched = Vec::new();
    for entry in parse_numstat(&numstat) {
        files += 1;
        lines = lines.saturating_add(entry.added).saturating_add(entry.removed);
        touched.extend(entry.paths);
    }
    // Every protected path the commit touches needs its own grant.
    if let Some((path, pattern)) = touched.iter().find_map(|path| {
        cfg.protected_paths
            .iter()
            .find(|pattern| path_matches(pattern, Path::new(path)))
            .filter(|_| !granted_path(request, Path::new(path)))
            .map(|pattern| (path, pattern))
    }) {
        if request.skip_policy != Some("protected_path") {
            return Ok(refuse_or_user_hold(
                request.actor,
                "protected_path",
                "guardrail.protected_path",
                format!("commit touches protected path {path}"),
                json!({"path": path, "pattern": pattern, "files": touched}),
            ));
        }
    }
    if (files > cfg.caps.files || lines > cfg.caps.lines)
        && request.skip_policy != Some("cap")
        && !request.grants.is_some_and(|grants| grants.covers_caps(files, lines, &cfg.caps))
    {
        return Ok(refuse_or_user_hold(
            request.actor,
            "cap",
            "guardrail.cap",
            format!(
                "commit changes {files} files / {lines} lines; caps are {} files / {} lines. \
                 Commit in smaller pieces; if the change cannot be split, ask the user to let it through",
                cfg.caps.files, cfg.caps.lines
            ),
            json!({"files": files, "lines": lines, "cap_files": cfg.caps.files, "cap_lines": cfg.caps.lines, "paths": touched}),
        ));
    }
    Ok(Decision::Allow)
}

// ---------------------------------------------------------------- the branch, after the fact

/// How many of a branch's commits `session.done` judges one by one. Past this, the newest are
/// judged on their own and the older ones only as part of the branch's net change.
pub const RECHECK_MAX_COMMITS: usize = 200;
/// How many shape-gated files a done reads back from git.
const RECHECK_MAX_SHAPED: usize = 50;

/// What a task's branch did since the project base, read by [`measure_branch`] with the store
/// lock released, for [`recheck_branch`] to judge inside the transaction (RA-109, RA-567).
#[derive(Debug, Default)]
pub struct BranchWork {
    /// `git diff --numstat <base>...HEAD`: the net change the task brings to review.
    range: String,
    /// Each non-merge commit since the base, newest first, with its own numstat, renames found.
    commits: Vec<(String, String)>,
    /// More commits than [`RECHECK_MAX_COMMITS`]: the older ones are judged only in the net
    /// change.
    partial: bool,
    /// Each shape-gated file the branch changes, as it is at `HEAD`, or why it cannot be read.
    shaped: Vec<(String, Result<String, &'static str>)>,
    /// Why git could not measure the branch. Such a branch is not taken as within limits: the
    /// done is refused until it can be measured, or a person moves the task.
    unmeasured: Option<String>,
}

/// Repository settings a measurement pins, so a checkout's own config cannot change what git
/// reports (an agent writes that config). A value git cannot parse at all still fails the
/// command — `-c` does not reach past a broken file — and that refuses the done instead.
const MEASURE_CONFIG: &[&str] = &[
    "-c", "core.quotePath=true", "-c", "core.fsmonitor=false", "-c", "color.ui=never", "-c", "diff.noprefix=false",
    "-c", "diff.relative=false", "-c", "diff.ignoreSubmodules=none",
];

/// `git <MEASURE_CONFIG> <args>`, for [`git_output`] and [`git_with_input`].
fn measuring<'a>(args: &[&'a str]) -> Vec<&'a str> {
    MEASURE_CONFIG.iter().copied().chain(args.iter().copied()).collect()
}

/// Read what [`recheck_branch`] judges: the branch's net numstat against `base`, each commit's
/// numstat, and the shape-gated files it changes. Always `Some`; when git cannot measure the
/// branch (no base, no shared history, a config git refuses to read) it says why, and the
/// done is refused rather than waved through — an agent can break a measurement on purpose
/// (`git config core.quotePath x`, an orphan branch). Every call goes through [`crate::proc`]
/// with a deadline, so it belongs before the store lock.
///
/// `base` comes from the repository (its `HEAD` at clone) or the person, so it is never let
/// near git's option parser: a base that starts with `-` is not measured at all, and every
/// revision goes after `--end-of-options` — a remote whose default branch is
/// `--output=/some/path` would otherwise have git truncate that file on every done.
pub fn measure_branch(cfg: &GuardrailConfig, worktree: &Path, base: &str) -> Option<BranchWork> {
    let unmeasured = |what: &str, why: String| {
        tracing::warn!(worktree = %worktree.display(), %what, error = %why, "measuring task work");
        Some(BranchWork { unmeasured: Some(format!("git {what}: {why}")), ..BranchWork::default() })
    };
    if base.starts_with('-') {
        return unmeasured("diff", format!("the base branch {base:?} starts with '-'"));
    }
    let range = match git_output(worktree, &measuring(&["diff", "--numstat", "-z", "--no-renames", "--no-ext-diff", "--end-of-options", &format!("{base}...HEAD"), "--"])) {
        Ok(range) => range,
        Err(error) => return unmeasured("diff", error.message),
    };
    let mut work = BranchWork { range, ..BranchWork::default() };
    let limit = format!("--max-count={}", RECHECK_MAX_COMMITS + 1);
    let listed = git_output(worktree, &measuring(&["rev-list", "--no-merges", &limit, "--end-of-options", &format!("{base}..HEAD"), "--"]));
    match listed {
        Ok(listed) => {
            let mut shas: Vec<&str> = listed.lines().filter(|line| !line.is_empty()).collect();
            if shas.len() > RECHECK_MAX_COMMITS {
                shas.truncate(RECHECK_MAX_COMMITS);
                work.partial = true;
                tracing::info!(worktree = %worktree.display(), "more than {RECHECK_MAX_COMMITS} commits since {base}; older ones are judged only in the net diff");
            }
            if !shas.is_empty() {
                let input = shas.iter().map(|sha| format!("{sha}\n")).collect::<String>();
                match git_with_input(worktree, &measuring(&["diff-tree", "--stdin", "-r", "-z", "-M", "--numstat", "--root"]), input.as_bytes()) {
                    Ok(out) => work.commits = split_commits(&String::from_utf8_lossy(&out)),
                    Err(error) => return unmeasured("diff-tree", error.message),
                }
            }
        }
        Err(error) => return unmeasured("rev-list", error.message),
    }
    let shaped: Vec<String> = parse_numstat(&work.range)
        .into_iter()
        .flat_map(|entry| entry.paths)
        .filter(|path| !path.contains('\n') && cfg.shape_gates.iter().any(|gate| path_matches(&gate.path, Path::new(path))))
        .take(RECHECK_MAX_SHAPED)
        .collect();
    if !shaped.is_empty() {
        match read_at_head(worktree, &shaped) {
            Some(texts) => work.shaped = shaped.into_iter().zip(texts).collect(),
            None => return unmeasured("cat-file", "could not read the shape-gated files at HEAD".into()),
        }
    }
    Some(work)
}

/// `git diff-tree --stdin -z` output split per commit: each `<sha>\0` header, then its numstat
/// records, kept in the `-z` form [`parse_numstat`] reads (a rename is its counts and an empty
/// path, then both sides).
fn split_commits(out: &str) -> Vec<(String, String)> {
    let mut commits: Vec<(String, String)> = Vec::new();
    let mut records = out.split('\0');
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if !record.contains('\t') {
            commits.push((record.trim().to_string(), String::new()));
            continue;
        }
        let Some((_, numstat)) = commits.last_mut() else { continue };
        numstat.push_str(record);
        numstat.push('\0');
        if record.splitn(3, '\t').nth(2) == Some("") {
            for path in records.by_ref().take(2) {
                numstat.push_str(path);
                numstat.push('\0');
            }
        }
    }
    commits
}

/// Each of `paths` as it is at `HEAD`, in one `git cat-file --batch`. `None` when git fails.
fn read_at_head(worktree: &Path, paths: &[String]) -> Option<Vec<Result<String, &'static str>>> {
    let input: String = paths.iter().map(|path| format!("HEAD:{path}\n")).collect();
    let out = git_with_input(worktree, &measuring(&["cat-file", "--batch"]), input.as_bytes()).ok()?;
    let mut at = 0;
    let mut texts = Vec::with_capacity(paths.len());
    for _ in paths {
        let end = at + out.get(at..)?.iter().position(|&b| b == b'\n')?;
        let header = std::str::from_utf8(&out[at..end]).ok()?;
        at = end + 1;
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            texts.push(Err("deleted on this branch"));
            continue;
        }
        let mut fields = header.rsplitn(3, ' ');
        let size: usize = fields.next()?.parse().ok()?;
        let kind = fields.next()?;
        let body = out.get(at..at + size)?;
        at += size + 1;
        texts.push(match kind {
            "blob" => String::from_utf8(body.to_vec()).map_err(|_| "not UTF-8 text"),
            _ => Err("not a file"),
        });
    }
    Some(texts)
}

/// Judge a finished task's branch as commits (RA-109): each commit since the base against the
/// protected paths, the net change against the caps and protected paths, and each shape-gated
/// file it changes against its validator. An agent's own `git commit` meets these at the
/// pre-commit hook, which `--no-verify`, `core.hooksPath` and commits made without pre-commit
/// (`cherry-pick`, `commit-tree`) all skip; this is the check they cannot.
///
/// The session's grants count and are not spent again — a `once` grant included after its one
/// use, since that use may have been this very commit — and so does a person's confirmation
/// of a shape-gate hold for the same file. Returns the refusal, if any.
pub fn recheck_branch(
    conn: &Connection,
    actor: &Actor,
    project_id: Id,
    worktree: &Path,
    session_id: Id,
    work: &BranchWork,
) -> Result<(), BusError> {
    if let Some(why) = &work.unmeasured {
        return Err(BusError::refused(
            "guardrail.unmeasured",
            format!("Relay could not measure this branch against the project's base ({why}), so it is not handed to review"),
        )
        .with_details(json!({"reason": why}))
        .with_hint(
            "Put the checkout back the way git can read it (a config value it rejects, a branch with no history in \
             common with the base) and call session.done again; if you cannot, report the task blocked and say why, \
             and the person moves it.",
        ));
    }
    let cfg = config(conn, Some(project_id))?;
    let grants = grants_given(conn, session_id)?;
    let judge = |diff: &str, skip_policy: Option<&str>| -> Result<Option<BusError>, BusError> {
        let request = GateRequest {
            actor, project_id, worktree, kind: GateKind::Commit, path: None, new_text: None, diff: Some(diff),
            command: None, skip_policy, grants: None, path_only: false, probes: None,
        };
        let first = evaluate_with(&cfg, &request)?;
        Ok(match granted_retry(&cfg, &request, first, &grants)?.0 {
            Decision::Allow => None,
            Decision::Refuse(error) | Decision::Hold { error, .. } => Some(error),
        })
    };
    // Caps are the task's (BUS.md §9.2), so they apply to the net change below, not per commit.
    for (sha, numstat) in &work.commits {
        if let Some(mut error) = judge(numstat, Some("cap"))? {
            let short = &sha[..sha.len().min(12)];
            error.message = format!("commit {short} on this branch: {}", error.message);
            if let Some(Value::Object(details)) = error.details.as_mut() {
                details.insert("commit".into(), json!(sha));
            }
            return Err(error);
        }
    }
    if let Some(mut error) = judge(&work.range, None)? {
        if let Some(Value::Object(details)) = error.details.as_mut() {
            details.insert("commits_checked".into(), json!(work.commits.len()));
            details.insert("commits_partial".into(), json!(work.partial));
        }
        return Err(error);
    }
    for (path, text) in &work.shaped {
        let Some(gate) = cfg.shape_gates.iter().find(|gate| path_matches(&gate.path, Path::new(path))) else { continue };
        let failure = match text {
            Ok(text) => validate_shape(gate, text).err(),
            Err(why) => Some(why.to_string()),
        };
        let Some(reason) = failure else { continue };
        if shape_confirmed(conn, session_id, path)? {
            continue;
        }
        return Err(BusError::refused(
            "guardrail.shape_gate",
            format!("{path} on this branch fails {}: {reason}", gate.validator),
        )
        .with_details(json!({"path": path, "validator": gate.validator, "reason": reason}))
        .with_hint("Fix the file and commit the fix, then call session.done again; if it must stay this way, report the task blocked and say why."));
    }
    Ok(())
}

/// Every grant a person gave this session that has not been revoked, a used-up `once` grant
/// included: for judging work already done, never for letting something new through.
fn grants_given(conn: &Connection, session_id: Id) -> Result<Grants, BusError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, json_extract(details, '$.kind'), json_extract(details, '$.value'), json_extract(details, '$.grant.scope')
             FROM holds
             WHERE session_id = ?1 AND op = 'guardrail.request' AND state = 'confirmed'
               AND json_extract(details, '$.grant.revoked_at') IS NULL
             ORDER BY id",
        )
        .bus()?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok((r.get::<_, Id>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .bus()?;
    let mut grants = Grants::default();
    grants.list = rows
        .into_iter()
        .filter_map(|(id, kind, value, scope)| {
            Some(grants::Grant {
                id,
                kind: serde_json::from_value(Value::String(kind?)).ok()?,
                value: value.unwrap_or_default(),
                scope: serde_json::from_value(Value::String(scope?)).ok()?,
            })
        })
        .collect();
    Ok(grants)
}

/// Did a person confirm a shape-gate hold on `path` for this session — their own write, or an
/// agent's held one?
fn shape_confirmed(conn: &Connection, session_id: Id, path: &str) -> Result<bool, BusError> {
    conn.prepare_cached(
        "SELECT EXISTS(SELECT 1 FROM holds WHERE session_id = ?1 AND policy = 'shape_gate' AND state = 'confirmed'
           AND ?2 IN (json_extract(details, '$.path'), json_extract(details, '$.original.path')))",
    )
    .bus()?
    .query_row(params![session_id, path], |r| r.get(0))
    .bus()
}

fn evaluate_exec(cfg: &GuardrailConfig, request: &GateRequest<'_>) -> Result<Decision, BusError> {
    let command = request.command.ok_or_else(|| {
        BusError::invalid("guardrail.command", "kind=exec requires command")
    })?;
    let commands = shell_commands(command);
    if request.actor.is_agent() {
        if let Some(argv) = self_approval(command) {
            // No grant lifts this one: an agent approving its own exception is no exception.
            return Ok(Decision::Refuse(
                BusError::refused(
                    "guardrail.self_approval",
                    format!(
                        "{} would act as the user (answer a guardrail, or pair, approve or revoke a phone or a cloud \
                         link); only the user can do that",
                        argv.join(" ")
                    ),
                )
                .with_details(json!({"command": command, "argv": argv}))
                .with_hint(
                    "Ask with guardrail.request and wait for guardrail.request_resolved; the user answers it in Relay. \
                     Pairing and cloud links the user sets up themselves: say what you need in your report.",
                ),
            ));
        }
        // Nor this one: it would switch off the check a grant is an exception to.
        if let Some((argv, what)) = hook_bypass(&commands) {
            let (why, hint) = if what == DEFINES_ALIAS {
                (
                    "the guardrail cannot follow an alias into the commands that later use it, so an agent spells \
                     its git commands out",
                    "Run the git command itself (`git log --oneline`, not an alias for it). A person's own aliases \
                     are theirs to set.",
                )
            } else if what == PUSH_SKIPS_CHECK {
                (
                    "Relay's pre-push hook is where a push meets the project's check",
                    "Push with the hook on (plain `git push`). If the check fails, fix what it reports and push \
                     again; if it cannot pass for a reason outside your work, say so in your report.",
                )
            } else {
                (
                    "Relay's pre-commit hook is where your commits meet the caps and protected paths",
                    "Commit with the hooks on (plain `git commit`, or git.commit). If the hook refuses, change what it \
                     names, or ask for that cap or path with guardrail.request. session.done checks every commit on \
                     the branch either way.",
                )
            };
            return Ok(Decision::Refuse(
                BusError::refused("guardrail.hook_bypass", format!("{} {what}, and {why}", argv.join(" ")))
                    .with_details(json!({"command": command, "argv": argv}))
                    .with_hint(hint),
            ));
        }
        // Nor this one: an upload to Google Play cannot be taken back, and it is the user's.
        if let Some(DeniedMatch { words, .. }) = play_publish(&commands) {
            let argv: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
            return Ok(Decision::Refuse(
                BusError::refused(
                    "guardrail.play_publish",
                    format!("{} would publish to Google Play; only the user publishes a release", argv.join(" ")),
                )
                .with_details(json!({"command": command, "argv": argv}))
                .with_hint(
                    "Build the release without publishing (`./gradlew bundleRelease`, or assemble/bundle tasks), \
                     and say in your report that it is ready: the user publishes it from Relay's Build panel.",
                ),
            ));
        }
    }
    if request.skip_policy == Some("denied_command") {
        return Ok(Decision::Allow);
    }
    let uncovered = denied_matches_in(&cfg.denied_commands, &commands).into_iter().find(|hit| {
        !request.grants.is_some_and(|grants| grants.covers_command(&hit.words, &commands))
    });
    if let Some(DeniedMatch { pattern, words, .. }) = uncovered {
        let argv: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
        return Ok(refuse_or_user_hold(
            request.actor,
            "denied_command",
            "guardrail.command",
            format!(
                "{} runs the denied command {pattern:?}; test a command with guardrail.check first",
                argv.join(" ")
            ),
            json!({"command": command, "pattern": pattern, "argv": argv}),
        ));
    }
    Ok(Decision::Allow)
}

/// The user-only answers to a guardrail, reached through the CLI as the user. Best effort: the
/// socket does not authenticate `user`, so this closes the obvious door, not every door.
const USER_ONLY_ANSWERS: &[&str] = &[
    "guardrail.confirm", "guardrail.reject", "guardrail.config.set", "guardrail.grant.revoke", "settings.set", "settings.reset",
];

/// The CLI's own commands that are the user's alone, as `(command, subcommands)`: they pair a
/// phone or let one in, forget one, point the engine at a rendezvous, or link this PC to a cloud
/// engine — each hands out (or takes away) a phone token or the room secret. They talk to the
/// door and the data directory, not the bus, so no op check sees them. `relay remote serve`
/// counts only with `--pair`, which opens a pairing window the agent could answer itself.
const USER_ONLY_COMMANDS: &[(&str, &[&str])] = &[
    ("remote", &["approve", "pair", "decline", "revoke", "via"]),
    ("cloud", &["connect", "pair", "off"]),
];

/// The environment the CLI takes its identity from (`actor_from_env`): without `RELAY_SESSION`
/// it falls back to the user.
const IDENTITY_VARS: &[&str] = &["RELAY_SESSION", "RELAY_TOKEN", "RELAY_ACTOR"];

/// Programs that run their argument as another user or with a fresh environment, so whatever
/// they start no longer carries the session's identity.
const IDENTITY_RESETTERS: &[&str] = &["sudo", "su", "doas", "runuser", "pkexec", "systemd-run", "machinectl"];

/// Does a command line invoke Relay as the user, or answer a guardrail through it? Returns the
/// offending argv.
///
/// Read through the same expanded command list as the denied-command rule, so `bash -c`,
/// `env …`, `python -c`, a script piped into `sh` and a heredoc fed to a shell are all seen.
/// Searching for these names (`rg guardrail.confirm crates/relay-core`) is not running them:
/// only a Relay *invocation* — `relay`/`$RELAY_BIN` followed by `q`/`cmd` and an op — counts,
/// and then both of the CLI's forms, `q <op>` and `cmd '<envelope>'` (RA-096, RA-097).
fn self_approval(line: &str) -> Option<Vec<String>> {
    if claims_user_envelope(line) {
        return Some(vec![line.to_string()]);
    }
    let commands = shell_commands(line);
    let runs_relay = commands.iter().any(|command| !data_only(command) && command.iter().any(|word| is_relay(&word.text)));
    for command in &commands {
        if data_only(command) {
            continue;
        }
        let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
        let argv = || command.iter().map(|word| word.text.clone()).collect();
        if sheds_identity(&words, runs_relay) {
            return Some(argv());
        }
        let impersonates = words.iter().enumerate().any(|(at, word)| {
            matches!(*word, "--actor=user" | "--actor=test")
                || *word == "--actor" && matches!(words.get(at + 1), Some(&"user") | Some(&"test"))
        });
        let relay_at: Vec<usize> = (0..words.len()).filter(|at| is_relay(words[*at])).collect();
        // A renamed copy of the binary is still the CLI: `--actor user` next to `q`/`cmd`.
        if impersonates && (!relay_at.is_empty() || words.iter().any(|word| matches!(*word, "q" | "cmd"))) {
            return Some(argv());
        }
        for at in relay_at {
            if user_only_command(&words, at) {
                return Some(argv());
            }
            let Some(op) = relay_call(&words, at) else { continue };
            let answers = match op {
                // `q` with nothing after it takes the op from somewhere this line does not show.
                None => true,
                Some(op) if op.trim_start().starts_with('{') => {
                    USER_ONLY_ANSWERS.iter().any(|answer| op.contains(answer)) || op.contains("\"actor\"")
                }
                Some(op) => USER_ONLY_ANSWERS.contains(&op) || op.contains(['$', '`']),
            };
            if answers {
                return Some(argv());
            }
        }
    }
    None
}

/// A raw request envelope that claims the user (or test) actor for an op, in any quoting:
/// `{"actor":"user","op":…}` written to the socket with `socat`, `nc` or a script.
fn claims_user_envelope(line: &str) -> bool {
    let bare: String = line.chars().filter(|c| !c.is_whitespace() && !matches!(c, '\\' | '"' | '\'')).collect();
    (bare.contains("actor:user") || bare.contains("actor:test")) && bare.contains("op:")
}

/// The Relay CLI, by name or through `$RELAY_BIN`: `relay`, `/usr/bin/relay`, `relay-cli`
/// (`cargo run -p relay-cli`). A word with spaces in it is text, not a program.
fn is_relay(word: &str) -> bool {
    word.contains("RELAY_BIN")
        || !word.contains(char::is_whitespace) && shell::program(word).to_ascii_lowercase().starts_with("relay")
}

/// The op a Relay CLI call at `at` runs: `Some(Some(op))` for `relay [flags] q|cmd <op>`,
/// `Some(None)` when `q`/`cmd` has no op after it, `None` when this is no bus call at all.
fn relay_call<'w>(words: &[&'w str], at: usize) -> Option<Option<&'w str>> {
    let sub = skip_cli_flags(words, at + 1);
    if !matches!(words.get(sub), Some(&"q") | Some(&"cmd")) {
        return None;
    }
    Some(words.get(skip_cli_flags(words, sub + 1)).copied())
}

/// Is the Relay CLI call at `at` one of [`USER_ONLY_COMMANDS`]: `relay [flags] remote approve`,
/// `relay cloud off`, `relay remote serve --pair`?
fn user_only_command(words: &[&str], at: usize) -> bool {
    let command = skip_cli_flags(words, at + 1);
    let Some(&name) = words.get(command) else { return false };
    let sub = skip_cli_flags(words, command + 1);
    let Some(&sub_name) = words.get(sub) else { return false };
    if name == "remote" && sub_name == "serve" {
        return words[sub + 1..].contains(&"--pair");
    }
    USER_ONLY_COMMANDS.iter().any(|(cli, subs)| *cli == name && subs.contains(&sub_name))
}

/// The first word from `at` on that is not one of the CLI's global options.
fn skip_cli_flags(words: &[&str], mut at: usize) -> usize {
    while let Some(word) = words.get(at) {
        if matches!(*word, "--instance" | "--actor") {
            at += 2;
        } else if word.starts_with('-') {
            at += 1;
        } else {
            break;
        }
    }
    at
}

/// Clearing or overriding the session identity, which makes the CLI fall back to the user:
/// `RELAY_SESSION=`, `unset RELAY_SESSION`, `env -u RELAY_SESSION` / `--unset=…` / `-uRELAY_…`,
/// and — when the line runs Relay at all — `env -i` or a program that resets the environment.
fn sheds_identity(words: &[&str], runs_relay: bool) -> bool {
    let names = |word: &str| IDENTITY_VARS.contains(&word);
    let program = words.first().map(|word| shell::program(word)).unwrap_or_default();
    words.iter().enumerate().any(|(at, word)| {
        IDENTITY_VARS.iter().any(|var| word.strip_prefix(var).is_some_and(|rest| rest.starts_with('=')))
            || matches!(*word, "-u" | "--unset") && words.get(at + 1).is_some_and(|next| names(next))
            || word.strip_prefix("--unset=").or_else(|| word.strip_prefix("-u")).is_some_and(names)
            || program == "unset" && at > 0 && names(word)
            || runs_relay && IDENTITY_RESETTERS.contains(&shell::program(word))
            || runs_relay && shell::program(word) == "env" && words[at + 1..].iter().take_while(|w| w.starts_with('-')).any(|flag| {
                *flag == "-" || *flag == "--ignore-environment" || !flag.starts_with("--") && flag.contains('i')
            })
    })
}

/// Does a command line switch off Relay's pre-commit hook, the one place an agent's own `git
/// commit` meets the caps and protected paths (RA-109)? `git commit -n` / `--no-verify` (also
/// in a short-flag cluster, `-anm`), a `core.hooksPath` override for one command (`git -c`,
/// `--config-env`, `GIT_CONFIG_KEY_<n>` / `GIT_CONFIG_PARAMETERS`), or a `git config` that sets,
/// unsets or removes it. A `git push --no-verify` likewise skips the pre-push hook, where the
/// project's check runs (`check_on_push`). Defining a git alias at all ([`git_aliases`]) is
/// refused as well: a `git config alias.ci commit` now makes a `git ci -n` later that no reader
/// of that later line can tell from any other command. Returns the command and what it does. Shell indirection or a commit
/// that never runs pre-commit (`cherry-pick`, `commit-tree`) still gets past this, which is why
/// `session.done` re-checks the branch's commits ([`recheck_commits`]).
fn hook_bypass(commands: &[Vec<Word>]) -> Option<(Vec<String>, &'static str)> {
    let hooks_path = |assignment: &str| assignment.split('=').next().is_some_and(|key| key.trim().eq_ignore_ascii_case("core.hookspath"));
    for command in commands {
        if data_only(command) || is_guardrail_dry_run(command) {
            continue;
        }
        let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
        let argv = || words.iter().map(|word| word.to_string()).collect::<Vec<_>>();
        let by_environment = words.iter().any(|word| {
            let Some((name, value)) = word.split_once('=') else { return false };
            let key = name.strip_prefix("GIT_CONFIG_KEY_").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            key && hooks_path(value) || name == "GIT_CONFIG_PARAMETERS" && value.to_ascii_lowercase().contains("core.hookspath")
        });
        if by_environment {
            return Some((argv(), "overrides core.hooksPath"));
        }
        for at in (0..words.len()).filter(|&at| program_at(command, at) && shell::program(words[at]) == "git") {
            let mut i = at + 1;
            while let Some(&word) = words.get(i) {
                let value = match word {
                    "-c" | "--config-env" => words.get(i + 1).copied(),
                    _ => word.strip_prefix("--config-env="),
                };
                if value.is_some_and(hooks_path) {
                    return Some((argv(), "overrides core.hooksPath"));
                }
                if !word.starts_with('-') {
                    break;
                }
                i += if matches!(word, "-c" | "--config-env") || takes_value("git", word) { 2 } else { 1 };
            }
            let rest = words.get(i + 1..).unwrap_or_default();
            match words.get(i) {
                Some(&"commit") if commit_skips_hooks(rest) => return Some((argv(), "skips the commit hooks")),
                Some(&"push") if push_skips_hooks(rest) => return Some((argv(), PUSH_SKIPS_CHECK)),
                Some(&"config") if config_writes_hooks_path(rest) => return Some((argv(), "rewrites core.hooksPath")),
                _ => {}
            }
        }
        if !git_aliases(std::slice::from_ref(command)).0.is_empty() {
            return Some((argv(), DEFINES_ALIAS));
        }
    }
    // An alias whose name and body are set by separate words (`export GIT_CONFIG_KEY_0=…`).
    let running: Vec<Vec<Word>> = commands.iter().filter(|command| !data_only(command)).cloned().collect();
    if !git_aliases(&running).0.is_empty() {
        let argv = running.first().map(|command| command.iter().map(|word| word.text.clone()).collect()).unwrap_or_default();
        return Some((argv, DEFINES_ALIAS));
    }
    None
}

const DEFINES_ALIAS: &str = "defines a git alias";

/// `git commit` options that take the next word as their value, so a `-n` there is a value.
const COMMIT_VALUE_OPTIONS: &[&str] = &[
    "message", "file", "author", "date", "reuse-message", "reedit-message", "fixup", "squash", "template",
    "cleanup", "trailer", "pathspec-from-file",
];

/// `-n` or `--no-verify` among `git commit`'s arguments, as git's option parser reads them: a
/// unique abbreviation (`--no-veri`) counts, a short cluster is read up to the option that
/// takes the rest as its value (`-anm` skips, `-m -n` is a message), and `--` ends the options.
fn commit_skips_hooks(args: &[&str]) -> bool {
    let mut i = 0;
    while let Some(&word) = args.get(i) {
        i += 1;
        if word == "--" {
            return false;
        }
        if let Some(long) = word.strip_prefix("--") {
            let (name, attached) = match long.split_once('=') {
                Some((name, _)) => (name, true),
                None => (long, false),
            };
            if name.len() >= "no-veri".len() && "no-verify".starts_with(name) {
                return true;
            }
            if !attached && COMMIT_VALUE_OPTIONS.contains(&name) {
                i += 1;
            }
            continue;
        }
        let Some(cluster) = word.strip_prefix('-').filter(|cluster| !cluster.is_empty()) else { continue };
        for (at, flag) in cluster.char_indices() {
            match flag {
                'n' => return true,
                // The rest of the word is the value; with nothing left, the next word is.
                'm' | 'F' | 'c' | 'C' | 't' => {
                    if at + 1 == cluster.len() {
                        i += 1;
                    }
                    break;
                }
                // An optional value, attached only.
                'S' | 'u' => break,
                _ => {}
            }
        }
    }
    false
}

const PUSH_SKIPS_CHECK: &str = "skips the pre-push check";

/// `--no-verify` among `git push`'s options, or a unique abbreviation of it (`--no-veri`;
/// `--no-ve` could also be `--no-verbose`, which git refuses). `-n` is `--dry-run` for push.
fn push_skips_hooks(args: &[&str]) -> bool {
    let mut words = args.iter().take_while(|word| **word != "--");
    while let Some(word) = words.next() {
        // A value, not an option: `-o --no-verify` hands the server that text.
        if matches!(*word, "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec") {
            words.next();
            continue;
        }
        let name = word.strip_prefix("--").map(|long| long.split('=').next().unwrap_or(long));
        if name.is_some_and(|name| name.len() >= "no-veri".len() && "no-verify".starts_with(name)) {
            return true;
        }
    }
    false
}

/// A `git config` that sets, unsets or removes `core.hooksPath`, in the classic form
/// (`git config --worktree core.hooksPath /dev/null`, `--unset`, `--remove-section core`) or the
/// subcommand form (`git config set|unset core.hooksPath`). Reading it is fine.
fn config_writes_hooks_path(args: &[&str]) -> bool {
    let mut positional = Vec::new();
    let mut flags = Vec::new();
    let mut i = 0;
    while let Some(&word) = args.get(i) {
        i += 1;
        if word == "--" {
            positional.extend(args.get(i..).unwrap_or_default());
            break;
        }
        if word.starts_with('-') && word.len() > 1 {
            if matches!(word, "-f" | "--file" | "--blob" | "--type" | "--default" | "--comment" | "--value") {
                i += 1;
            }
            flags.push(word);
        } else {
            positional.push(word);
        }
    }
    let key = |word: &&str| word.eq_ignore_ascii_case("core.hookspath");
    let flagged = |names: &[&str]| flags.iter().any(|flag| names.contains(flag));
    let first = positional.first().copied();
    if flagged(&["--get", "--get-all", "--get-regexp", "--get-urlmatch", "-l", "--list"]) || matches!(first, Some("get" | "list")) {
        return false;
    }
    if flagged(&["--remove-section", "--rename-section"]) || matches!(first, Some("remove-section" | "rename-section")) {
        return positional.iter().any(|word| word.eq_ignore_ascii_case("core"));
    }
    if matches!(first, Some("set" | "unset")) {
        return positional.get(1).is_some_and(key);
    }
    let Some(at) = positional.iter().position(key) else { return false };
    flagged(&["--unset", "--unset-all", "--add", "--replace-all"]) || positional.len() > at + 1
}

/// A command whose program reads its arguments as data — text to print, a pattern to search
/// for — and never runs them: `echo rm -rf /`, `rg guardrail.confirm crates/relay-core`.
/// Only its own program word is a command (RA-099).
fn data_only(command: &[Word]) -> bool {
    let Some(first) = command.first() else { return false };
    match shell::program(&first.text) {
        "echo" | "printf" | "grep" | "egrep" | "fgrep" | "ag" | "ack" | "man" | "which" | "whatis" | "apropos" => true,
        // `rg --pre` runs a program on every file it searches.
        "rg" => !command.iter().any(|word| word.text.starts_with("--pre")),
        _ => false,
    }
}

/// Shells whose script can come from `-c`, standard input or a heredoc.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish"];

/// A `NAME=value` word: an assignment before a command, or an `export`/`env` argument.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Can the word at `at` be the program a command runs? Any word can, quoted or not, wherever
/// it stands: `"rm" -rf`, `env 'rm' -rf`, `if 'rm' -rf x; then`, `find -exec sudo 'rm' -rf`,
/// and wrappers no list keeps up with (`uv run`, `pkexec`). Only quoted text with a space in it
/// is never one: that is a string (a commit message, a JSON payload), which names a command
/// without running it (D103). The commands whose arguments are all data (`echo`, `rg`) are
/// told apart by [`data_only`].
fn program_at(command: &[Word], at: usize) -> bool {
    let word = &command[at];
    at == 0 || !word.quoted || !word.text.contains(char::is_whitespace)
}

/// An interpreter whose inline code (`python3 -c`, `perl -e`, `node -e`) can run a command.
fn is_interpreter(program: &str) -> bool {
    let program = program.to_ascii_lowercase();
    program == "nodejs"
        || ["python", "perl", "ruby", "node", "php", "lua", "deno", "bun"].iter().any(|name| {
            program.strip_prefix(name).is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'))
        })
}

/// How many levels of script-inside-a-line are followed beyond what [`shell::commands`] reads.
const MAX_SCRIPT_DEPTH: usize = 4;

/// Every command `line` runs, each as its words (D103): what [`shell::commands`] reads, minus
/// what is only data — a comment, a heredoc body fed to `cat` — plus what it cannot see on its
/// own: a heredoc or a quoted script fed to a shell on standard input (`echo '…' | sh`,
/// `bash <<< '…'`, `bash <<EOF`), and the strings in an interpreter's inline code
/// (`python3 -c "os.system('…')"`), each read as a command line (RA-098, RA-099). A script
/// file (`bash deploy.sh`) is not opened.
fn shell_commands(line: &str) -> Vec<Vec<Word>> {
    let mut out = Vec::new();
    expand(line, 0, &mut out);
    out
}

fn expand(line: &str, depth: usize, out: &mut Vec<Vec<Word>>) {
    let (code, mut scripts) = strip_data(line);
    let commands: Vec<Vec<Word>> = shell::commands(&code)
        .into_iter()
        .map(|mut command| {
            // A comment inside a nested script, which `strip_data` did not see.
            if let Some(at) = command.iter().position(|word| !word.quoted && word.text.starts_with('#')) {
                command.truncate(at);
            }
            command
        })
        .filter(|command| !command.is_empty())
        .collect();
    let stdin_script = commands.iter().any(|command| reads_stdin_script(command));
    scripts.extend(git_alias_scripts(&commands));
    scripts.extend(commands.iter().filter_map(|command| env_split_string(command)));
    for command in &commands {
        if let Some(code) = inline_code(command) {
            scripts.extend(literal_lines(code));
        }
        if stdin_script {
            scripts.extend(command.iter().filter(|word| word.quoted).map(|word| word.text.clone()));
            if data_only(command) && command.len() > 1 {
                scripts.push(command[1..].iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" "));
            }
        }
    }
    out.extend(commands);
    if depth < MAX_SCRIPT_DEPTH {
        for script in scripts {
            expand(&script, depth + 1, out);
        }
    }
}

/// The command line GNU `env -S` / `--split-string` splits and runs (`env -S 'rm -rf build'`,
/// `env -iS"…"`), with the words after it.
fn env_split_string(command: &[Word]) -> Option<String> {
    let at = (0..command.len()).find(|&at| program_at(command, at) && shell::program(&command[at].text) == "env")?;
    let words: Vec<&str> = command[at + 1..].iter().map(|word| word.text.as_str()).collect();
    let mut i = 0;
    while let Some(&word) = words.get(i) {
        i += 1;
        if matches!(word, "-u" | "-C" | "--unset" | "--chdir") {
            i += 1;
            continue;
        }
        let split = if word == "--split-string" {
            i += 1;
            words.get(i - 1).copied()
        } else if let Some(value) = word.strip_prefix("--split-string=") {
            Some(value)
        } else if let Some(cluster) = word.strip_prefix('-').filter(|cluster| !cluster.starts_with('-') && cluster.contains('S')) {
            let value = &cluster[cluster.find('S').unwrap_or(0) + 1..];
            if value.is_empty() {
                i += 1;
                words.get(i - 1).copied()
            } else {
                Some(value)
            }
        } else if word.starts_with('-') || is_assignment(word) {
            continue;
        } else {
            return None;
        };
        let rest = words.get(i..).unwrap_or_default().join(" ");
        return split.map(|line| format!("{line} {rest}"));
    }
    None
}

/// The git aliases a line defines, as (name, body): `git -c alias.x='!…' x`, `git config
/// [--global] alias.x '…'` (an alias for later), `--config-env=alias.x=VAR`, and
/// `GIT_CONFIG_KEY_<n>=alias.x` / `GIT_CONFIG_VALUE_<n>` or `GIT_CONFIG_PARAMETERS` (both its
/// `'key'='value'` and its older `'key=value'` form). And every git subcommand the line calls,
/// with its arguments, since any of them may be one of those aliases.
fn git_aliases(commands: &[Vec<Word>]) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let assignments: HashMap<&str, &str> = commands
        .iter()
        .flatten()
        .filter(|word| is_assignment(&word.text))
        .filter_map(|word| word.text.split_once('='))
        .collect();
    let alias = |key: &str| {
        key.get(..6).filter(|prefix| prefix.eq_ignore_ascii_case("alias.")).map(|_| key[6..].to_ascii_lowercase())
    };
    let mut defined: Vec<(String, String)> = Vec::new();
    for (name, value) in &assignments {
        if let Some(n) = name.strip_prefix("GIT_CONFIG_KEY_") {
            if let (Some(alias), Some(body)) = (alias(value), assignments.get(format!("GIT_CONFIG_VALUE_{n}").as_str())) {
                defined.push((alias, body.to_string()));
            }
        }
        if *name == "GIT_CONFIG_PARAMETERS" {
            // Single-quoted parts: `'key'='value'`, or `'key=value'`.
            let parts: Vec<&str> = value.split('\'').collect();
            let mut i = 1;
            while let Some(part) = parts.get(i) {
                let pair = if parts.get(i + 1) == Some(&"=") {
                    i += 2;
                    parts.get(i).map(|body| (*part, *body))
                } else {
                    part.split_once('=')
                };
                if let Some((alias, body)) = pair.and_then(|(key, body)| alias(key).map(|alias| (alias, body))) {
                    defined.push((alias, body.to_string()));
                }
                i += 2;
            }
        }
    }
    let mut calls: Vec<(String, String)> = Vec::new();
    for command in commands {
        let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
        for at in (0..words.len()).filter(|&at| program_at(command, at) && shell::program(words[at]) == "git") {
            let mut i = at + 1;
            while let Some(&word) = words.get(i).filter(|word| word.starts_with('-')) {
                let setting = match word {
                    "-c" => words.get(i + 1).and_then(|value| value.split_once('=')).map(|(key, body)| (key, body.to_string())),
                    _ => word.strip_prefix("--config-env=").or_else(|| (word == "--config-env").then(|| words.get(i + 1).copied()).flatten())
                        .and_then(|value| value.split_once('='))
                        .map(|(key, var)| (key, assignments.get(var).copied().unwrap_or_default().to_string())),
                };
                if let Some((alias, body)) = setting.and_then(|(key, body)| alias(key).map(|alias| (alias, body))) {
                    defined.push((alias, body));
                }
                i += if matches!(word, "-c" | "--config-env") || takes_value("git", word) { 2 } else { 1 };
            }
            let Some(&sub) = words.get(i) else { continue };
            let rest = words.get(i + 1..).unwrap_or_default();
            if sub == "config" {
                let mut positional = Vec::new();
                let mut words = rest.iter().copied();
                while let Some(word) = words.next() {
                    if matches!(word, "-f" | "--file" | "--blob" | "--type" | "--default" | "--comment" | "--value") {
                        words.next();
                    } else if !word.starts_with('-') {
                        positional.push(word);
                    }
                }
                let (key, body) = match positional.as_slice() {
                    ["set", key, body, ..] => (*key, *body),
                    [key, body, ..] => (*key, *body),
                    _ => continue,
                };
                if let Some(alias) = alias(key) {
                    defined.push((alias, body.to_string()));
                }
            } else {
                calls.push((sub.to_ascii_lowercase(), rest.join(" ")));
            }
        }
    }
    (defined, calls)
}

/// What the git aliases a line defines run, each as a command line ([`git_aliases`]). A `!`
/// alias is shell (`!rm -rf build` runs `rm -rf build`), any other is git (`reset --hard` runs
/// `git reset --hard`), an alias of an alias is followed, and each call of one on the line gets
/// its own line with its arguments after (`git r --hard`). Every definition counts called or
/// not, bare.
fn git_alias_scripts(commands: &[Vec<Word>]) -> Vec<String> {
    let (defined, calls) = git_aliases(commands);
    if defined.is_empty() {
        return Vec::new();
    }
    let body_of = |name: &str| defined.iter().rev().find(|(alias, _)| alias == name).map(|(_, body)| body.as_str());
    let resolve = |body: &str, args: &str| -> String {
        let mut body = body.to_string();
        // git refuses a loop; the bound only stops one here.
        for _ in 0..8 {
            if let Some(shell) = body.strip_prefix('!') {
                return format!("{shell} {args}");
            }
            let (first, rest) = body.trim_start().split_once(char::is_whitespace).unwrap_or((body.trim(), ""));
            match body_of(&first.to_ascii_lowercase()) {
                Some(next) => body = format!("{next} {rest}"),
                None => break,
            }
        }
        format!("git {body} {args}")
    };
    let mut scripts: Vec<String> = defined.iter().map(|(_, body)| resolve(body, "")).collect();
    for (sub, args) in &calls {
        if let Some(body) = body_of(sub) {
            scripts.push(resolve(body, args));
        }
    }
    scripts
}

/// Is this a shell reading its script from standard input — `sh`, `bash -s`, `bash <<< …` —
/// rather than from `-c` or a file?
fn reads_stdin_script(command: &[Word]) -> bool {
    if data_only(command) {
        return false;
    }
    let Some(at) = command
        .iter()
        .enumerate()
        .position(|(at, word)| program_at(command, at) && SHELLS.contains(&shell::program(&word.text)))
    else {
        return false;
    };
    let rest = &command[at + 1..];
    let mut i = 0;
    while let Some(word) = rest.get(i) {
        let text = word.text.as_str();
        if text == "-s" || text == "<<<" {
            return true;
        }
        if text.starts_with('-') && !text.starts_with("--") && text.contains('c') {
            return false;
        }
        if matches!(text, "-o" | "+o" | "-O" | "+O" | "<" | "--rcfile" | "--init-file") {
            i += 2;
        } else if text.starts_with(['-', '+', '<', '>']) || text.starts_with(|c: char| c.is_ascii_digit()) && text.contains(['<', '>']) {
            i += 1;
        } else {
            // A script file: its content is not on this line.
            return false;
        }
    }
    true
}

/// The inline code an interpreter runs from its arguments: `python3 -c CODE`, `perl -ne CODE`,
/// `node --eval CODE`, `php -r CODE`.
fn inline_code(command: &[Word]) -> Option<&str> {
    if data_only(command) {
        return None;
    }
    let at = command
        .iter()
        .enumerate()
        .position(|(at, word)| program_at(command, at) && is_interpreter(shell::program(&word.text)))?;
    let mut i = at + 1;
    while let Some(word) = command.get(i) {
        let text = word.text.as_str();
        if matches!(text, "--eval" | "--print" | "--command") {
            return command.get(i + 1).map(|word| word.text.as_str());
        }
        if let Some(code) = text.strip_prefix("--eval=").or_else(|| text.strip_prefix("--print=")) {
            return Some(code);
        }
        if text.starts_with("--") {
            i += 1;
        } else if text.starts_with('-') && text.len() > 1 {
            if text.ends_with(['c', 'e', 'E', 'r', 'p']) {
                return command.get(i + 1).map(|word| word.text.as_str());
            }
            i += 1;
        } else {
            return None;
        }
    }
    None
}

/// The string literals in a piece of interpreter code, each a possible command line, and all
/// of them joined: `os.system('rm -rf /')` and `subprocess.run(["rm", "-rf", "/"])` both run
/// `rm -rf /`.
fn literal_lines(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut literals = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let quote = chars[i];
        i += 1;
        if !matches!(quote, '\'' | '"' | '`') {
            continue;
        }
        let mut text = String::new();
        while i < chars.len() && chars[i] != quote {
            if chars[i] == '\\' && i + 1 < chars.len() {
                text.push(match chars[i + 1] {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                i += 2;
                continue;
            }
            text.push(chars[i]);
            i += 1;
        }
        i += 1;
        if !text.trim().is_empty() {
            literals.push(text);
        }
    }
    if literals.len() > 1 {
        literals.push(literals.join(" "));
    }
    literals
}

/// Who reads a heredoc's body.
#[derive(Clone, Copy, PartialEq)]
enum Feed {
    Shell,
    Interpreter,
    Data,
}

#[derive(Clone, Copy, PartialEq)]
enum Frame {
    /// Live shell code: the line itself, or the inside of `$( )` (`subst`).
    Code { subst: bool, parens: u32 },
    Double,
    Backtick,
}

/// `line` with what is only data taken out — `#` comments, and heredoc bodies fed to anything
/// but a shell or an interpreter — and the scripts it feeds elsewhere: a heredoc body fed to a
/// shell, the strings of one fed to an interpreter, and the substitutions an unquoted heredoc
/// still expands. A heredoc whose terminator never comes is left in place, read as commands.
fn strip_data(line: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut scripts = Vec::new();
    let mut stack = vec![Frame::Code { subst: false, parens: 0 }];
    // (delimiter, `<<-`, expands, who reads it)
    let mut pending: Vec<(String, bool, bool, Feed)> = Vec::new();
    let mut line_start = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let top = *stack.last().unwrap_or(&Frame::Code { subst: false, parens: 0 });
        if top == Frame::Double {
            match c {
                '"' => {
                    stack.pop();
                }
                '\\' => {
                    out.push(c);
                    i += 1;
                    if let Some(&next) = chars.get(i) {
                        out.push(next);
                        i += 1;
                    }
                    continue;
                }
                '$' if chars.get(i + 1) == Some(&'(') => {
                    out.push_str("$(");
                    stack.push(Frame::Code { subst: true, parens: 0 });
                    i += 2;
                    continue;
                }
                '`' => stack.push(Frame::Backtick),
                _ => {}
            }
            out.push(c);
            i += 1;
            continue;
        }
        let at_word_start = i == 0 || matches!(chars[i - 1], ' ' | '\t' | '\n' | ';' | '&' | '|' | '(' | ')');
        match c {
            '\\' => {
                out.push(c);
                i += 1;
                if let Some(&next) = chars.get(i) {
                    out.push(next);
                    i += 1;
                }
                continue;
            }
            '\'' => {
                out.push(c);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    out.push('\'');
                    i += 1;
                }
                continue;
            }
            '"' => stack.push(Frame::Double),
            '`' if top == Frame::Backtick => {
                stack.pop();
            }
            '`' => stack.push(Frame::Backtick),
            '$' if chars.get(i + 1) == Some(&'(') => {
                out.push_str("$(");
                stack.push(Frame::Code { subst: true, parens: 0 });
                i += 2;
                continue;
            }
            '(' => {
                if let Some(Frame::Code { parens, .. }) = stack.last_mut() {
                    *parens += 1;
                }
            }
            ')' => match stack.last_mut() {
                Some(Frame::Code { parens, .. }) if *parens > 0 => *parens -= 1,
                Some(Frame::Code { subst: true, .. }) => {
                    stack.pop();
                }
                _ => {}
            },
            '#' if at_word_start => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '<' if chars.get(i + 1) == Some(&'<')
                && chars.get(i + 2) != Some(&'<')
                && (i == 0 || chars[i - 1] != '<') =>
            {
                let mut j = i + 2;
                let strip_tabs = chars.get(j) == Some(&'-');
                if strip_tabs {
                    j += 1;
                }
                while matches!(chars.get(j), Some(' ' | '\t')) {
                    j += 1;
                }
                let mut delimiter = String::new();
                let mut quoted = false;
                while let Some(&d) = chars.get(j) {
                    if d.is_whitespace() || matches!(d, ';' | '&' | '|' | '(' | ')' | '<' | '>') {
                        break;
                    }
                    if matches!(d, '\'' | '"' | '\\') {
                        quoted = true;
                    } else {
                        delimiter.push(d);
                    }
                    j += 1;
                }
                if !delimiter.is_empty() {
                    let end = chars[i..].iter().position(|&d| d == '\n').map_or(chars.len(), |n| i + n);
                    let physical: String = chars[line_start..end].iter().collect();
                    pending.push((delimiter, strip_tabs, !quoted, feed_of(&physical)));
                }
                out.extend(&chars[i..j]);
                i = j;
                continue;
            }
            '\n' => {
                out.push('\n');
                i += 1;
                line_start = i;
                for (delimiter, strip_tabs, expands, feed) in std::mem::take(&mut pending) {
                    let mut at = i;
                    let mut body = String::new();
                    let mut closed = false;
                    while at < chars.len() {
                        let end = chars[at..].iter().position(|&d| d == '\n').map_or(chars.len(), |n| at + n);
                        let text: String = chars[at..end].iter().collect();
                        at = (end + 1).min(chars.len());
                        let check = if strip_tabs { text.trim_start_matches('\t') } else { text.as_str() };
                        if check == delimiter {
                            closed = true;
                            break;
                        }
                        body.push_str(&text);
                        body.push('\n');
                    }
                    if !closed {
                        // No terminator: keep the rest as commands rather than guess.
                        break;
                    }
                    match feed {
                        Feed::Shell => scripts.push(body),
                        Feed::Interpreter => scripts.extend(literal_lines(&body)),
                        // Data, but an unquoted delimiter still runs `$( )` and backticks in it.
                        Feed::Data if expands && body.contains(['$', '`']) => {
                            scripts.push(format!("echo \"{}\"", body.replace('"', "\\\"")));
                        }
                        Feed::Data => {}
                    }
                    i = at;
                    line_start = i;
                }
                continue;
            }
            _ => {}
        }
        out.push(c);
        i += 1;
    }
    (out, scripts)
}

/// Who reads a heredoc started on this physical line: a shell or an interpreter anywhere on
/// it (`cat <<EOF | sh`, `python3 - <<EOF`), else it is data (`cat <<EOF > notes.md`).
fn feed_of(line: &str) -> Feed {
    let mut feed = Feed::Data;
    for token in line.split(|c: char| c.is_whitespace() || matches!(c, '|' | ';' | '&' | '(' | ')' | '<' | '>')) {
        let token: String = token.chars().filter(|c| !matches!(c, '\'' | '"' | '\\')).collect();
        let program = shell::program(&token);
        if SHELLS.contains(&program) || matches!(program, "eval" | "source" | ".") {
            return Feed::Shell;
        }
        if is_interpreter(program) {
            feed = Feed::Interpreter;
        }
    }
    feed
}

/// One place a command line runs a denied pattern.
struct DeniedMatch {
    pattern: String,
    /// The command, every word as written: what a command grant is compared with.
    words: Vec<Word>,
}

/// Every command in `line` that actually *runs* one of the denied patterns. Quoted data that
/// merely mentions a pattern is not a match. All of them, not the first: a grant has to cover
/// each one for the line to pass.
#[cfg(test)]
fn denied_matches(patterns: &[String], line: &str) -> Vec<DeniedMatch> {
    denied_matches_in(patterns, &shell_commands(line))
}

fn denied_matches_in(patterns: &[String], commands: &[Vec<Word>]) -> Vec<DeniedMatch> {
    let patterns: Vec<(&String, DeniedPattern)> = patterns
        .iter()
        .filter_map(|pattern| DeniedPattern::parse(pattern).map(|parsed| (pattern, parsed)))
        .collect();
    let mut hits = Vec::new();
    for command in commands {
        if is_guardrail_dry_run(command) {
            continue;
        }
        // The arguments of `echo` or `rg` are data; only the program itself runs.
        let data = data_only(command);
        for (pattern, parsed) in &patterns {
            if if data { parsed.runs_at(command, 0) } else { parsed.runs_in(command) } {
                hits.push(DeniedMatch { pattern: (*pattern).clone(), words: command.clone() });
            }
        }
    }
    hits
}

/// A denied pattern read the way its program reads it: `git reset --hard` is the program `git`,
/// the subcommand `reset` and the flag `--hard`. Matching on that, rather than on the pattern's
/// literal words in a row, is what makes `git -C app reset HEAD~1 --hard`, `rm -fr build` and
/// `git push origin main --force` the commands they are (RA-011).
struct DeniedPattern {
    program: String,
    /// The words right after the program that are not options, in order: `reset`, `push`.
    subcommands: Vec<String>,
    /// Every flag the pattern names, as a set, clusters expanded: `-rf` is `-r` and `-f`.
    flags: Vec<String>,
    /// Any other word the pattern names (`rm -rf /` names `/`), anywhere after the subcommand.
    operands: Vec<String>,
    /// The long options a command's abbreviation can stand for: the program's destructive ones
    /// ([`LONG_OPTIONS`]) and the ones the pattern names, as written (`hard`, `force`).
    long_options: Vec<String>,
}

impl DeniedPattern {
    fn parse(pattern: &str) -> Option<Self> {
        let mut words = pattern.split_whitespace();
        let program = canonical_program(words.next()?);
        let long_options = LONG_OPTIONS.iter().filter(|(of, _)| *of == program).map(|(_, name)| name.to_string()).collect();
        let mut parsed = Self { program, subcommands: Vec::new(), flags: Vec::new(), operands: Vec::new(), long_options };
        for word in words {
            if word.starts_with('-') && word.len() > 1 {
                if let Some(long) = word.strip_prefix("--").map(|long| long.split('=').next().unwrap_or(long).to_ascii_lowercase()) {
                    if !long.is_empty() && !parsed.long_options.contains(&long) {
                        parsed.long_options.push(long);
                    }
                }
                parsed.flags.extend(expand_flags(&parsed.program, word));
            } else if parsed.flags.is_empty() && parsed.operands.is_empty() {
                parsed.subcommands.push(word.to_ascii_lowercase());
            } else {
                parsed.operands.push(word.to_ascii_lowercase());
            }
        }
        Some(parsed)
    }

    /// Whether `command` runs this pattern. The program may sit anywhere in the command, so
    /// `sudo rm -rf`, `timeout 60 git clean -fd` and `find . -exec rm -rf {} ;` are all seen.
    fn runs_in(&self, command: &[Word]) -> bool {
        (0..command.len()).any(|at| self.runs_at(command, at))
    }

    fn runs_at(&self, command: &[Word], at: usize) -> bool {
        let word = &command[at];
        // A quoted program still runs (`"rm" -rf`, `env 'rm' -rf`); elsewhere a quoted word is
        // an argument, which is data ([`program_at`]).
        if !program_at(command, at) || canonical_program(&word.text) != self.program {
            return false;
        }
        // Quoted text with a space in it is a string, never a flag or a subcommand (D103).
        let rest: Vec<&str> = command[at + 1..]
            .iter()
            .filter(|word| !(word.quoted && word.text.contains(char::is_whitespace)))
            .map(|word| word.text.as_str())
            .collect();
        if self.program == "gradle" {
            return self.runs_gradle(&rest);
        }
        let mut next = 0;
        for wanted in &self.subcommands {
            while let Some(word) = rest.get(next).filter(|word| word.starts_with('-')) {
                next += if takes_value(&self.program, word) { 2 } else { 1 };
            }
            match rest.get(next) {
                Some(word) if word.eq_ignore_ascii_case(wanted) => next += 1,
                _ => return false,
            }
        }
        let after = rest.get(next..).unwrap_or_default();
        let flags: Vec<String> = after
            .iter()
            .filter(|word| word.starts_with('-') && word.len() > 1)
            .flat_map(|word| self.command_flags(word))
            .collect();
        let operands: Vec<String> = after
            .iter()
            .filter(|word| !word.starts_with('-'))
            .map(|word| word.to_ascii_lowercase())
            .collect();
        // A forced push is any push that can destroy what the remote holds: `+main` and `:main`
        // as refspecs, `--force-with-lease`, `--mirror`, `--delete`/`-d` and `--prune`.
        let destroys_remote = self.program == "git"
            && self.subcommands.first().is_some_and(|sub| sub == "push")
            && (operands.iter().any(|word| word.len() > 1 && (word.starts_with('+') || word.starts_with(':')))
                || flags.iter().any(|flag| {
                    matches!(flag.as_str(), "-d" | "--delete" | "--mirror" | "--prune" | "--force-with-lease" | "--force-if-includes")
                }));
        self.flags.iter().all(|flag| flags.contains(flag) || flag == "-f" && destroys_remote)
            && self.operands.iter().all(|operand| operands.contains(operand))
    }

    /// A flag word of the command as the flags it sets ([`expand_flags`]), reading a long option
    /// the way git's parse-options and GNU getopt do: a unique prefix is the option it starts
    /// (`--dele` is `--delete`, `--har` is `--hard`, `--r` is rm's `--recursive`). A prefix of
    /// several stands for all of them: the program refuses an ambiguous one itself, so that
    /// over-matches nothing that would run. An exact name is only itself.
    fn command_flags(&self, word: &str) -> Vec<String> {
        let Some(long) = word.strip_prefix("--") else { return expand_flags(&self.program, word) };
        let name = long.split('=').next().unwrap_or(long).to_ascii_lowercase();
        if name.is_empty() || self.long_options.contains(&name) {
            return expand_flags(&self.program, word);
        }
        let mut flags = expand_flags(&self.program, word);
        for option in self.long_options.iter().filter(|option| option.starts_with(&name)) {
            flags.extend(expand_flags(&self.program, &format!("--{option}")));
        }
        flags
    }

    /// Gradle reads every word that is not an option as a task, in any order and any number:
    /// `gradle publish*` is any invocation that runs a task the glob names, whatever else it
    /// runs beside it (`./gradlew assembleRelease :app:publishReleaseBundle`).
    fn runs_gradle(&self, rest: &[&str]) -> bool {
        let mut tasks = Vec::new();
        let mut flags = Vec::new();
        let mut words = rest.iter();
        while let Some(word) = words.next() {
            if word.starts_with('-') && word.len() > 1 {
                flags.extend(expand_flags(&self.program, word));
                // `-x publishBundle` excludes the task; `-p app` is a directory.
                if gradle_takes_value(word) {
                    words.next();
                }
            } else {
                tasks.push(*word);
            }
        }
        self.subcommands.iter().all(|glob| tasks.iter().any(|task| gradle_task_matches(glob, task)))
            && self.flags.iter().all(|flag| flags.contains(flag))
    }
}

/// A program word as a denied pattern names it: the basename, lower case, with the Gradle
/// wrapper and Gradle itself one program (`./gradlew`, `gradlew.bat`, `/opt/gradle/bin/gradle`).
fn canonical_program(word: &str) -> String {
    let name = shell::program(word).to_ascii_lowercase();
    match name.as_str() {
        "gradlew" | "gradlew.bat" | "gradle.bat" => "gradle".into(),
        _ => name,
    }
}

/// Gradle options whose value is the next word, so it is not taken for a task.
fn gradle_takes_value(word: &str) -> bool {
    matches!(
        word,
        "-x" | "--exclude-task" | "-p" | "--project-dir" | "-b" | "--build-file" | "-c" | "--settings-file"
            | "-I" | "--init-script" | "-g" | "--gradle-user-home" | "--project-cache-dir" | "-D" | "--system-prop"
            | "-P" | "--project-prop" | "--console" | "--warning-mode" | "--include-build"
    )
}

/// Does the task word `task` (`publishReleaseBundle`, `:app:publishReleaseBundle`) run a task
/// `glob` names? Gradle matches a name without regard to case, and takes an abbreviation for
/// the task it is short for (`pRB`, `pub-rel-bun`, `pub`): for a `prefix*` glob, a word whose
/// first segment begins `prefix` counts too. Any task it could be short for is enough.
fn gradle_task_matches(glob: &str, task: &str) -> bool {
    let name = task.rsplit(':').next().unwrap_or(task);
    if name.is_empty() || name.contains('=') {
        return false;
    }
    if wildcard_matches(glob, &name.to_ascii_lowercase()) {
        return true;
    }
    let Some(prefix) = glob.strip_suffix('*').filter(|prefix| !prefix.is_empty() && !prefix.contains('*')) else {
        return false;
    };
    let mut chars = name.chars();
    let first: String = chars.next().into_iter().chain(chars.take_while(char::is_ascii_lowercase)).collect();
    prefix.starts_with(&first.to_ascii_lowercase())
}

/// `*` matches any run of characters, nothing else is special.
fn wildcard_matches(glob: &str, text: &str) -> bool {
    let mut parts = glob.split('*');
    let Some(mut rest) = text.strip_prefix(parts.next().unwrap_or("")) else { return false };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else { return rest.is_empty() };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// Gradle Play Publisher's upload tasks (`publishReleaseBundle`, `publishListing`,
/// `promoteReleaseArtifact`, …) and the `uploadToPlay` kind a build script adds. An agent never
/// runs these: a release reaches Google Play only when the user asks for it (`device.build`
/// with `publish`). Built in, not a default denied command, because a stored
/// `denied_commands` list replaces the defaults whole and would leave it out.
const PLAY_PUBLISH: &[&str] = &["gradle publish*", "gradle promote*", "gradle *toplay*"];

fn play_publish(commands: &[Vec<Word>]) -> Option<DeniedMatch> {
    let patterns: Vec<String> = PLAY_PUBLISH.iter().map(|pattern| pattern.to_string()).collect();
    denied_matches_in(&patterns, commands).into_iter().next()
}

/// A flag word as the individual flags it sets, each spelled one way: `-rf` is `-r` and `-f`;
/// `--force=x` is `--force`; for `rm` and `git`, `--force` is `-f`; for `rm`, `-R` and
/// `--recursive` are `-r`. Short flags keep their case — `-X` and `-x` differ in most tools.
fn expand_flags(program: &str, word: &str) -> Vec<String> {
    let canonical = |flag: String| -> String {
        match (program, flag.as_str()) {
            ("rm" | "git", "--force") => "-f".into(),
            ("rm", "-R" | "--recursive") => "-r".into(),
            _ => flag,
        }
    };
    if let Some(long) = word.strip_prefix("--") {
        if long.is_empty() {
            return Vec::new();
        }
        let name = long.split('=').next().unwrap_or(long).to_ascii_lowercase();
        return vec![canonical(format!("--{name}"))];
    }
    word.chars().skip(1).map(|c| canonical(format!("-{c}"))).collect()
}

/// The destructive long options an abbreviation can stand for even when a pattern does not
/// spell them out: what makes a push forced ([`DeniedPattern::runs_at`]), and the long forms of
/// the default patterns' short flags (`rm --rec --forc`, `git clean --fo`).
const LONG_OPTIONS: &[(&str, &str)] = &[
    ("git", "force"), ("git", "delete"), ("git", "mirror"), ("git", "prune"), ("git", "force-with-lease"),
    ("git", "force-if-includes"), ("git", "hard"), ("rm", "recursive"), ("rm", "force"),
];

/// Global options that take the next word as their value, so it is not mistaken for the
/// subcommand: `git -C ../app reset --hard`, `git -c core.x=y push --force`.
fn takes_value(program: &str, word: &str) -> bool {
    program == "git"
        && matches!(word, "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--exec-path" | "--config-env" | "--attr-source")
}

/// Is `line` one command that changes nothing but what the bus itself authorizes: a Relay CLI
/// call through `$RELAY_BIN` (each op is then authorized on its own), or a read-only look at
/// the checkout (`git diff`, `cat`, `rg`, …)? Deliberately narrow: no chaining, pipes,
/// redirection, substitution or expansion outside single quotes, and no option that lets a
/// reader write or run something (`git -c`, `--output`, `--ext-diff`, `rg --pre`).
pub(crate) fn read_only_exec(line: &str) -> bool {
    read_only_exec_at(line, 0)
}

fn read_only_exec_at(line: &str, depth: u8) -> bool {
    let line = line.trim();
    let (relay, rest) = ["\"$RELAY_BIN\"", "\"${RELAY_BIN}\"", "$RELAY_BIN", "${RELAY_BIN}"]
        .iter()
        .find_map(|bin| line.strip_prefix(bin).filter(|rest| rest.starts_with(char::is_whitespace)))
        .map_or((false, line), |rest| (true, rest));
    if !plain_words(rest) {
        return false;
    }
    // The reader also lists what a `bash -lc '…'` runs as a command of its own; the wrapper
    // case below judges that inner line itself, so only the outer command is read here.
    let commands = shell::commands(rest);
    let Some(command) = commands.first() else { return false };
    let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
    let wrapper = depth == 0 && matches!(words.first(), Some(&("bash" | "sh" | "zsh")));
    if commands.len() != 1 && !wrapper {
        return false;
    }
    if relay {
        // The session's own identity, never an override: `--actor user` is the user.
        return matches!(words.first(), Some(&("q" | "cmd" | "ops" | "schema" | "ping")))
            && !words.iter().any(|word| word.starts_with("--actor") || word.starts_with("--instance"));
    }
    let Some((&program, args)) = words.split_first() else { return false };
    match program {
        // Codex may hand over its own wrapper: judge the one command inside it.
        "bash" | "sh" | "zsh" if depth == 0 => {
            matches!(args, [flag, script] if matches!(*flag, "-c" | "-lc")) && read_only_exec_at(args[1], 1)
        }
        "ls" | "cat" | "head" | "tail" | "wc" | "pwd" | "grep" => true,
        "rg" => !args.iter().any(|arg| arg.starts_with("--pre")),
        "git" => {
            // The subcommand is the first word that is neither an option nor `-C`'s directory.
            let sub = args.iter().enumerate()
                .find(|(at, arg)| !arg.starts_with('-') && (*at == 0 || args[at - 1] != "-C"))
                .map(|(_, arg)| arg);
            matches!(sub, Some(&("diff" | "log" | "show" | "status" | "blame")))
                && !args.iter().any(|arg| {
                    *arg == "-c" || ["--output", "--ext-diff", "--exec-path", "--config-env"].iter().any(|bad| arg.starts_with(bad))
                })
        }
        _ => false,
    }
}

/// No shell syntax outside single quotes beyond words and spaces: nothing that chains, pipes,
/// redirects, substitutes or expands. Inside double quotes, no `$`, backtick or backslash.
fn plain_words(line: &str) -> bool {
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('\''), _) => {}
            (Some(_), '$' | '`' | '\\') => return false,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, ';' | '&' | '|' | '<' | '>' | '`' | '$' | '(' | ')' | '{' | '}' | '\\' | '\n' | '#') => return false,
            (None, _) => {}
        }
    }
    quote.is_none()
}

/// `relay q guardrail.check ...` is the sanctioned way to ask "would this be allowed?".
/// Blocking the question would make the safe path the unavailable one, so it stays open — the
/// call itself, read as the CLI reads it, not any line that mentions `guardrail.check`.
fn is_guardrail_dry_run(command: &[Word]) -> bool {
    let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
    words.first().is_some_and(|first| is_relay(first))
        && matches!(relay_call(&words, 0), Some(Some("guardrail.check" | "guardrail.explain")))
}

fn refuse_or_user_hold(
    actor: &Actor,
    policy: &str,
    code: &str,
    message: String,
    details: Value,
) -> Decision {
    if matches!(actor, Actor::User | Actor::Test) {
        hold(
            policy,
            "guardrail.user_bypass",
            format!("{message}; confirm to bypass this policy"),
            json!({"policy": policy, "original_code": code, "original": details}),
        )
    } else if matches!(policy, "destructive_write" | "shape_gate") {
        // Held for a person; for a destructive write the agent may also ask for the path.
        let mut error = BusError::held(code, message).with_details(details.clone());
        if let Some((kind, value)) = grants::suggestion(policy, &details) {
            error = error.with_hint(grants::hint(kind, &value));
        }
        Decision::Hold { policy: policy.to_string(), error, details }
    } else {
        let mut error = BusError::refused(code, message);
        let mut details = details;
        // The agent is told it can ask, and exactly what to ask for.
        if let Some((kind, value)) = grants::suggestion(policy, &details) {
            error = error.with_hint(grants::hint(kind, &value));
            if let Some(map) = details.as_object_mut() {
                map.insert("exception".into(), json!({"kind": kind, "value": value}));
            }
        }
        Decision::Refuse(error.with_details(details))
    }
}

fn hold(policy: &str, code: &str, message: String, details: Value) -> Decision {
    Decision::Hold {
        policy: policy.to_string(),
        error: BusError::held(code, message).with_details(details.clone()),
        details,
    }
}

fn validate_shape(gate: &ShapeGate, text: &str) -> Result<(), String> {
    match gate.validator.as_str() {
        "non_empty" if text.trim().is_empty() => Err("file is empty".into()),
        "non_empty" => Ok(()),
        "json" => serde_json::from_str::<Value>(text).map(|_| ()).map_err(|e| e.to_string()),
        "json_non_empty_array" => match serde_json::from_str::<Value>(text) {
            Ok(Value::Array(v)) if !v.is_empty() => Ok(()),
            Ok(Value::Array(_)) => Err("array is empty".into()),
            Ok(_) => Err("root is not an array".into()),
            Err(e) => Err(e.to_string()),
        },
        "json_non_empty_object" => match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(v)) if !v.is_empty() => Ok(()),
            Ok(Value::Object(_)) => Err("object is empty".into()),
            Ok(_) => Err("root is not an object".into()),
            Err(e) => Err(e.to_string()),
        },
        other => Err(format!("unknown validator {other:?}")),
    }
}

/// Lines a unified diff removes and adds. `---`/`+++` are file headers only outside a hunk:
/// inside one, `--- x` is a removed `-- x` (an SQL or Lua comment, a YAML separator) and
/// counts like any other removed line (RA-321). A hunk ends when the line counts its `@@`
/// header promised are used up; one whose header does not parse runs to the next `diff`/`@@`.
fn diff_counts(diff: &str) -> (u32, u32) {
    let mut removed = 0;
    let mut added = 0;
    // `(old, new)` lines left in the current hunk; `None` between hunks.
    let mut hunk: Option<(u32, u32)> = None;
    for line in diff.lines() {
        if line.starts_with("@@") {
            hunk = Some(hunk_lengths(line).unwrap_or((u32::MAX, u32::MAX))).filter(|&(old, new)| old > 0 || new > 0);
            continue;
        }
        if line.starts_with("diff ") {
            hunk = None;
            continue;
        }
        let Some((old, new)) = hunk.as_mut() else {
            if line.starts_with("---") || line.starts_with("+++") {
                continue;
            }
            // A diff with no hunk headers at all: every marked line is a change.
            if line.starts_with('-') {
                removed += 1;
            } else if line.starts_with('+') {
                added += 1;
            }
            continue;
        };
        match line.as_bytes().first() {
            Some(b'-') => { removed += 1; *old = old.saturating_sub(1); }
            Some(b'+') => { added += 1; *new = new.saturating_sub(1); }
            Some(b'\\') => {}
            _ => { *old = old.saturating_sub(1); *new = new.saturating_sub(1); }
        }
        if *old == 0 && *new == 0 {
            hunk = None;
        }
    }
    (removed, added)
}

/// The old and new line counts of a `@@ -a[,b] +c[,d] @@` header; a missing count is 1.
fn hunk_lengths(header: &str) -> Option<(u32, u32)> {
    let mut parts = header.strip_prefix("@@")?.split_whitespace();
    let count = |part: Option<&str>, sign: char| -> Option<u32> {
        let range = part?.strip_prefix(sign)?;
        match range.split_once(',') {
            Some((_, n)) => n.parse().ok(),
            None => range.parse::<u32>().ok().map(|_| 1),
        }
    };
    Some((count(parts.next(), '-')?, count(parts.next(), '+')?))
}

/// Linear multiset line comparison. Moving an unchanged line is not destructive; removing
/// one of N duplicate lines counts exactly once. This deliberately avoids quadratic LCS work
/// on large generated files (SPEC §15's performance doctrine).
fn changed_line_counts(old: &str, new: &str) -> (u32, u32) {
    let mut old_counts: HashMap<&str, u32> = HashMap::new();
    let mut new_counts: HashMap<&str, u32> = HashMap::new();
    for line in old.lines() {
        *old_counts.entry(line).or_default() += 1;
    }
    for line in new.lines() {
        *new_counts.entry(line).or_default() += 1;
    }
    let removed = old_counts
        .iter()
        .map(|(line, count)| count.saturating_sub(*new_counts.get(line).unwrap_or(&0)))
        .sum();
    let added = new_counts
        .iter()
        .map(|(line, count)| count.saturating_sub(*old_counts.get(line).unwrap_or(&0)))
        .sum();
    (removed, added)
}

/// Every git call here gets a deadline (D144): some still run with the store lock held.
const GIT_TIMEOUT: Duration = Duration::from_secs(10);

fn git_output(worktree: &Path, args: &[&str]) -> Result<String, BusError> {
    let out = crate::proc::output_with_timeout(Command::new("git").arg("-C").arg(worktree).args(args), GIT_TIMEOUT)
        .map_err(|e| BusError::unavailable("git.unavailable", e.to_string()))?
        .ok_or_else(|| BusError::unavailable("git.timeout", format!("git {} took longer than 10s", args.join(" "))))?;
    if !out.status.success() {
        return Err(BusError::conflict(
            "git.failed",
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// [`git_output`] for a git command that reads its list from stdin; the bytes as they came.
fn git_with_input(worktree: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, BusError> {
    let out = crate::proc::output_with_input(Command::new("git").arg("-C").arg(worktree).args(args), input, GIT_TIMEOUT)
        .map_err(|e| BusError::unavailable("git.unavailable", e.to_string()))?
        .ok_or_else(|| BusError::unavailable("git.timeout", format!("git {} took longer than 10s", args.join(" "))))?;
    if !out.status.success() {
        return Err(BusError::conflict("git.failed", String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(out.stdout)
}

fn relative_path(path: &str) -> Result<PathBuf, BusError> {
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(BusError::invalid(
            "guardrail.path",
            format!("{path:?} must be worktree-relative and contain no .."),
        ));
    }
    Ok(path.to_path_buf())
}

fn validate_pattern(pattern: &str) -> Result<(), BusError> {
    if pattern.is_empty() || Path::new(pattern).is_absolute() || pattern.split('/').any(|p| p == "..") {
        return Err(BusError::invalid(
            "guardrail.config",
            format!("bad relative path pattern {pattern:?}"),
        ));
    }
    Ok(())
}

/// Exact path, directory prefix, or a glob. `*` matches within one path segment; `**`
/// crosses separators, and a `**/` component matches zero or more whole directories, so
/// `**/.env` covers the root `.env` as well as `a/b/.env`. The distinction matters: `src/*`
/// used to quietly protect the whole subtree under `src`, so a pattern written for one
/// directory locked an entire tree (D113).
pub fn path_matches(pattern: &str, path: &Path) -> bool {
    let pattern = pattern.trim_matches('/');
    let path = path.to_string_lossy().trim_matches('/').to_string();
    if pattern == path || path.starts_with(&format!("{pattern}/")) {
        return true;
    }
    glob_match(pattern.as_bytes(), path.as_bytes())
}

/// Recursive matcher: every star gets its own backtrack point. One shared resume point used to
/// let a later `*` overwrite an earlier `**`'s, so `**/*.pem` matched only one directory deep.
fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    // Every (pattern, text) position is tried at most once, so an agent-supplied
    // `file.search` glob such as `*a*a*a*a*b` costs pattern x text, not an exponential walk.
    let mut failed = vec![false; (pattern.len() + 1) * (text.len() + 1)];
    Glob { pattern, text, failed: &mut failed }.from(0, 0)
}

struct Glob<'a> {
    pattern: &'a [u8],
    text: &'a [u8],
    failed: &'a mut [bool],
}

impl Glob<'_> {
    fn from(&mut self, p: usize, t: usize) -> bool {
        let slot = p * (self.text.len() + 1) + t;
        if self.failed[slot] {
            return false;
        }
        let matched = self.step(p, t);
        if !matched {
            self.failed[slot] = true;
        }
        matched
    }

    fn step(&mut self, p: usize, t: usize) -> bool {
        let (pattern, text) = (self.pattern, self.text);
        let Some(&c) = pattern.get(p) else {
            return t == text.len();
        };
        if c != b'*' {
            return text.get(t) == Some(&c) && self.from(p + 1, t + 1);
        }
        let mut after = p;
        while pattern.get(after) == Some(&b'*') {
            after += 1;
        }
        if after - p == 1 {
            // A segment-local star: it may consume anything up to, never across, a separator.
            let end = text[t..].iter().position(|&c| c == b'/').map_or(text.len(), |i| t + i);
            return (t..=end).any(|i| self.from(after, i));
        }
        // `**/` as a whole component matches zero or more directories: nothing at all, or any
        // run that ends on a separator.
        if pattern.get(after) == Some(&b'/') && (p == 0 || pattern[p - 1] == b'/') {
            return self.from(after + 1, t)
                || (t..text.len()).any(|i| text[i] == b'/' && self.from(after + 1, i + 1));
        }
        // Any other `**` (`**.pem`, a trailing `src/**`) crosses separators freely.
        (t..=text.len()).any(|i| self.from(after, i))
    }
}

// ---------------------------------------------------------------- holds / notifications

#[allow(clippy::too_many_arguments)] // frozen request attribution is clearer as named scalar fields
pub fn insert_hold(
    tx: &Transaction,
    request: &Request,
    project_id: Id,
    session_id: Option<Id>,
    session: Option<&str>,
    policy: &str,
    details: &Value,
    now: &str,
) -> Result<Id, BusError> {
    let mut frozen = request.clone();
    frozen.token = None; // never persist session secrets
    let payload_hash = crate::audit::payload_hash(&frozen.payload);
    // A held write carries the whole new file. Past the size a hold is shown cut at anyway, the
    // text goes to a file named by its hash, and the envelope keeps the cut copy (RA-102).
    let mut kept_aside = Vec::new();
    if let (Some(dir), Value::Object(fields)) = (blob_dir(tx), &mut frozen.payload) {
        for (key, value) in fields.iter_mut() {
            let Value::String(text) = value else { continue };
            if text.len() > SHOWN_STRING_MAX {
                kept_aside.push((format!("/{}", key.replace('~', "~0").replace('/', "~1")), store_blob(&dir, text)?));
                elide_text(text);
            }
        }
    }
    tx.prepare_cached(
        "INSERT INTO holds(project_id, session_id, session, actor, op, envelope, policy, details, state, created_at, payload_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', ?9, ?10)",
    )
    .bus()?
    .execute(params![
        project_id,
        session_id,
        session,
        request.actor.to_string(),
        request.op,
        serde_json::to_string(&frozen).bus()?,
        policy,
        serde_json::to_string(details).bus()?,
        now,
        payload_hash,
    ])
    .bus()?;
    let hold_id = tx.last_insert_rowid();
    for (pointer, hash) in kept_aside {
        tx.prepare_cached("INSERT INTO hold_blobs(hold_id, pointer, hash) VALUES (?1, ?2, ?3)")
            .bus()?
            .execute(params![hold_id, pointer, hash])
            .bus()?;
    }
    tx.execute(
        "INSERT INTO notifications(project_id, category, title, body, link, read, created_at)
         VALUES (?1, 'guardrail', 'Guardrail hold', ?2, ?3, 0, ?4)",
        params![
            project_id,
            format!("{} held {}", request.actor, request.op),
            // The hold is answered inside the session that tripped it, so the deep link
            // carries the session name the notification centre needs to get there (D101).
            json!({"op": "guardrail.confirm", "payload": {"hold_id": hold_id, "session": session}}).to_string(),
            now,
        ],
    )
    .bus()?;
    Ok(hold_id)
}

/// A refusal is the agent's to work around, not the person's to answer: it is filed in the
/// notification centre already read, with no sound and no phone alert. Holds and exception
/// requests are what wait on the person.
pub fn insert_refusal_notification(
    tx: &Transaction,
    project_id: Id,
    actor: &Actor,
    error: &BusError,
    now: &str,
) -> Result<(), BusError> {
    tx.execute(
        "INSERT INTO notifications(project_id, category, title, body, link, read, created_at)
         VALUES (?1, 'guardrail', 'Guardrail refusal', ?2, NULL, 1, ?3)",
        params![project_id, format!("{actor}: {} — {}", error.code, error.message), now],
    )
    .bus()?;
    Ok(())
}

/// The holds [`prune_holds`] forgets: answered before `?1`. Open holds stay, and so does
/// anything confirmed for a session still alive: its exception grant or its one-time pass may
/// yet be used.
macro_rules! answered_before {
    () => {
        "state != 'open' AND COALESCE(resolved_at, created_at) < ?1
           AND NOT (state = 'confirmed' AND session_id IN (SELECT id FROM sessions WHERE state != 'closed'))"
    };
}

/// Forget answered holds resolved before `cutoff` (RA-102). Each frozen envelope carries the
/// whole action, so they are not kept forever. Returns how many went, and the held-text files
/// no remaining hold names, for the caller to remove once the transaction has committed.
///
/// A new hold freezing the same text between that commit and the removal would lose its file;
/// confirming it then fails as `guardrail.hold_text_missing` rather than replay anything else.
pub fn prune_holds(tx: &Transaction, cutoff: &str) -> rusqlite::Result<(usize, Vec<PathBuf>)> {
    let hashes: Vec<String> = tx
        .prepare_cached(concat!("SELECT DISTINCT hash FROM hold_blobs WHERE hold_id IN (SELECT id FROM holds WHERE ", answered_before!(), ")"))?
        .query_map([cutoff], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    tx.prepare_cached(concat!("DELETE FROM hold_blobs WHERE hold_id IN (SELECT id FROM holds WHERE ", answered_before!(), ")"))?
        .execute([cutoff])?;
    let pruned = tx.prepare_cached(concat!("DELETE FROM holds WHERE ", answered_before!()))?.execute([cutoff])?;
    let mut unused = Vec::new();
    if let Some(dir) = blob_dir(tx) {
        let mut named = tx.prepare_cached("SELECT EXISTS(SELECT 1 FROM hold_blobs WHERE hash = ?1)")?;
        for hash in hashes.into_iter().filter(|hash| blob_name(hash)) {
            if !named.query_row([&hash], |r| r.get::<_, bool>(0))? {
                unused.push(dir.join(hash));
            }
        }
    }
    Ok((pruned, unused))
}

/// A string in a hold past this size is cut wherever a hold is shown: a held rewrite of a
/// large file carries the whole file, and a reply over the client's line cap tore its
/// connection down (RA-217). It is also where a frozen payload's text moves out of the
/// envelope into a file of its own (RA-102).
pub(crate) const SHOWN_STRING_MAX: usize = 64 * 1024;
/// How much of a cut string is kept.
const SHOWN_STRING_KEEP: usize = 4 * 1024;

/// Cut `text` past [`SHOWN_STRING_MAX`] to its first [`SHOWN_STRING_KEEP`] bytes and a note of
/// what was dropped. Returns whether it cut.
pub(crate) fn elide_text(text: &mut String) -> bool {
    if text.len() <= SHOWN_STRING_MAX {
        return false;
    }
    let mut end = SHOWN_STRING_KEEP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let dropped = text.len() - end;
    text.truncate(end);
    text.push_str(&format!("… [{dropped} more bytes elided]"));
    true
}

/// `<store dir>/hold-blobs`, where a frozen payload's large strings live, one file per
/// SHA-256. `None` for an in-memory store, which keeps them in the envelope.
fn blob_dir(conn: &Connection) -> Option<PathBuf> {
    let db = conn.path().filter(|path| !path.is_empty())?;
    Some(Path::new(db).parent()?.join("hold-blobs"))
}

/// A stored name is a lowercase SHA-256 and nothing else: a corrupt row never names a path.
fn blob_name(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Write `text` to its content-addressed file, unless an intact copy is already there.
fn store_blob(dir: &Path, text: &str) -> Result<String, BusError> {
    use sha2::Digest;
    let hash = crate::hex(&sha2::Sha256::digest(text.as_bytes()));
    let path = dir.join(&hash);
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() == text.len() as u64) {
        return Ok(hash);
    }
    let failed = |e: std::io::Error| BusError::unavailable("guardrail.hold_text", format!("keeping a held action's text: {e}"));
    std::fs::create_dir_all(dir).map_err(failed)?;
    let temp = dir.join(format!(".{hash}.{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, text.as_bytes()).map_err(failed)?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        failed(e)
    })?;
    Ok(hash)
}

/// The text a hold kept beside the store, checked against its name: a confirm replays exactly
/// what was held or nothing.
fn load_blob(dir: &Path, hold_id: Id, hash: &str) -> Result<String, BusError> {
    use sha2::Digest;
    let missing = || {
        BusError::conflict(
            "guardrail.hold_text_missing",
            format!("the text hold {hold_id} froze is no longer stored intact; reject it and retry the action"),
        )
    };
    if !blob_name(hash) {
        return Err(missing());
    }
    let bytes = std::fs::read(dir.join(hash)).map_err(|_| missing())?;
    if crate::hex(&sha2::Sha256::digest(&bytes)) != hash {
        return Err(missing());
    }
    String::from_utf8(bytes).map_err(|_| missing())
}

pub fn hold_by_id(conn: &Connection, id: Id) -> Result<Hold, BusError> {
    conn.prepare_cached("SELECT * FROM holds WHERE id = ?1")
        .bus()?
        .query_row([id], hold_row)
        .optional()
        .bus()?
        .ok_or_else(|| BusError::not_found("guardrail.hold_not_found", format!("no hold {id}")))
}

/// The request hold `id` froze, whole: what a confirm replays.
pub fn frozen_request(conn: &Connection, id: Id) -> Result<Request, BusError> {
    frozen_envelope(conn, id, true).map(|(request, _)| request)
}

/// The request hold `id` froze, and the JSON pointers into its payload of the strings kept
/// beside the store. `inflate` reads them back whole; without it they stay as stored — cut,
/// exactly as a hold is shown.
pub fn frozen_envelope(conn: &Connection, id: Id, inflate: bool) -> Result<(Request, Vec<String>), BusError> {
    let raw: String = conn
        .prepare_cached("SELECT envelope FROM holds WHERE id = ?1")
        .bus()?
        .query_row([id], |r| r.get(0))
        .optional()
        .bus()?
        .ok_or_else(|| BusError::not_found("guardrail.hold_not_found", format!("no hold {id}")))?;
    let mut request: Request = serde_json::from_str(&raw).map_err(crate::engine::internal)?;
    let kept: Vec<(String, String)> = conn
        .prepare_cached("SELECT pointer, hash FROM hold_blobs WHERE hold_id = ?1 ORDER BY pointer")
        .bus()?
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
        .bus()?
        .collect::<rusqlite::Result<_>>()
        .bus()?;
    if inflate && !kept.is_empty() {
        let dir = blob_dir(conn).ok_or_else(|| BusError::internal("a held text outside an on-disk store"))?;
        for (pointer, hash) in &kept {
            let text = load_blob(&dir, id, hash)?;
            let slot = request.payload.pointer_mut(pointer).ok_or_else(|| BusError::internal(format!("hold {id} has no {pointer}")))?;
            *slot = Value::String(text);
        }
    }
    Ok((request, kept.into_iter().map(|(pointer, _)| pointer).collect()))
}

/// Store the payload hash of holds frozen before it was taken at insert (v23), a page at a
/// time: SQLite cannot hash, so each is hashed once, the first time a list or a look reads it.
pub fn fill_payload_hashes(conn: &Connection) -> Result<(), BusError> {
    let unhashed: Vec<(Id, String)> = conn
        .prepare_cached("SELECT id, envelope FROM holds WHERE payload_hash IS NULL ORDER BY id LIMIT 200")
        .bus()?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .bus()?
        .collect::<rusqlite::Result<_>>()
        .bus()?;
    let mut store = conn.prepare_cached("UPDATE holds SET payload_hash = ?1 WHERE id = ?2").bus()?;
    for (id, envelope) in unhashed {
        store.execute(params![envelope_hash(envelope), id]).bus()?;
    }
    Ok(())
}

/// The payload hash of an envelope stored whole (every hold from before v23).
fn envelope_hash(envelope: String) -> String {
    serde_json::from_str::<Request>(&envelope)
        .map(|request| crate::audit::payload_hash(&request.payload))
        .unwrap_or_else(|_| crate::audit::payload_hash(&Value::String(envelope)))
}

pub fn hold_row(row: &Row) -> rusqlite::Result<Hold> {
    // Taken at insert; a hold from before that is hashed here until `fill_payload_hashes` stores it.
    let payload_hash = match row.get::<_, Option<String>>("payload_hash") {
        Ok(Some(hash)) => hash,
        _ => envelope_hash(row.get("envelope")?),
    };
    let actor = Actor::parse(&row.get::<_, String>("actor")?).unwrap_or(Actor::System);
    let resolved_by = row
        .get::<_, Option<String>>("resolved_by")?
        .and_then(|v| Actor::parse(&v).ok());
    let state = match row.get::<_, String>("state")?.as_str() {
        "confirmed" => HoldState::Confirmed,
        "rejected" => HoldState::Rejected,
        "expired" => HoldState::Expired,
        _ => HoldState::Open,
    };
    Ok(Hold {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        session_id: row.get("session_id")?,
        session: row.get("session")?,
        actor,
        op: row.get("op")?,
        payload_hash,
        policy: row.get("policy")?,
        details: serde_json::from_str(&row.get::<_, String>("details")?).unwrap_or(Value::Null),
        state,
        created_at: row.get("created_at")?,
        resolved_at: row.get("resolved_at")?,
        resolved_by,
    })
}

#[cfg(test)]
mod glob_tests {
    use super::path_matches;
    use std::path::Path;

    fn m(pattern: &str, path: &str) -> bool {
        path_matches(pattern, Path::new(path))
    }

    #[test]
    fn a_single_star_stays_inside_one_segment() {
        assert!(m("src/*.rs", "src/main.rs"));
        assert!(!m("src/*.rs", "src/deep/main.rs"), "one star must not cross a separator");
        assert!(m("src/**/*.rs", "src/deep/main.rs"));
        assert!(m("**/secrets.toml", "a/b/secrets.toml"));
        assert!(m("*.env", ".env"));
        assert!(!m("*.env", "config/.env"));
    }

    #[test]
    fn directory_prefixes_and_exact_paths_still_match() {
        assert!(m("secret", "secret/key"));
        assert!(m("secret/key", "secret/key"));
        assert!(!m("secret", "secretly/key"), "a prefix must end on a separator");
        assert!(m("**", "anything/at/all"));
        assert!(m("src/*", "src/main.rs"));
        assert!(!m("src/*", "src/a/b.rs"));
    }

    /// RA-012: one shared backtrack point let a later `*` overwrite the `**`'s, so these
    /// patterns matched exactly one directory deep and never at the top level.
    #[test]
    fn a_double_star_component_matches_zero_or_more_directories() {
        for path in ["c.pem", "x/c.pem", "x/y/c.pem", "x/y/z/c.pem"] {
            assert!(m("**/*.pem", path), "**/*.pem must match {path}");
        }
        assert!(!m("**/*.pem", "x/c.pem.bak"));
        for path in ["src/main.rs", "src/a/main.rs", "src/a/b/main.rs", "src/a/b/c/main.rs"] {
            assert!(m("src/**/*.rs", path), "src/**/*.rs must match {path}");
        }
        assert!(!m("src/**/*.rs", "lib/a/main.rs"));
        assert!(m("secrets/**/*.json", "secrets/a.json"));
        assert!(m("secrets/**/*.json", "secrets/x/y/a.json"));
        assert!(m("**/secrets.toml", "secrets.toml"));
        assert!(m("**/.env", ".env"));
        assert!(m("**/.env", "a/b/c/.env"));
        assert!(!m("**/.env", "a/b/c/x.env"), "`**/` matches whole directories only");
        assert!(m("**/*.key", "deploy/keys/prod.key"));
        assert!(m("a/**/b/*.txt", "a/x/b/y/b/z.txt"), "the inner star must not starve the outer");
        // A `**` that is not a whole component keeps crossing separators, as it always has.
        assert!(m("**.pem", "x/y/c.pem"));
        assert!(m("src/**", "src/a/b/c.rs"));
        assert!(!m("*.pem", "x/c.pem"));
        let long = "a".repeat(4000);
        assert!(!m("*a*a*a*a*a*a*a*a*b", &long), "a pathological glob must still finish");
    }
}

#[cfg(test)]
mod denied_tests {
    use super::{denied_matches, hook_bypass, self_approval, shell_commands};

    fn denied(line: &str) -> bool {
        let defaults = ["rm -rf", "git reset --hard", "git clean -fd", "git push --force"].map(String::from);
        !denied_matches(&defaults, line).is_empty()
    }

    /// RA-010 / RA-011: everyday spellings of the default denied commands, and the same
    /// commands run inside subshells, substitutions and `sh -c`.
    #[test]
    fn every_spelling_of_a_denied_command_is_seen() {
        for line in [
            "rm -rf build", "rm -fr build", "rm -r -f build", "rm -rfv build", "rm -Rf build",
            "rm --recursive --force build", "/bin/rm -rf build", "\"rm\" -rf build", "rm '-rf' build",
            "\\rm -rf build", "sudo rm -rf build", "find . -name x -exec rm -rf {} ;",
            "git reset --hard", "git reset HEAD~1 --hard", "git -C ../app reset --hard",
            "git -c core.x=y reset --hard origin/main",
            "git clean -fd", "git clean -df", "git clean -fdx", "git clean -xfd", "git clean -f -d",
            "git -C sub clean -fd",
            "git push --force", "git push -f origin main", "git push origin main --force",
            "git push origin +main", "git push --force=true origin main",
            "git push --mirror origin", "git push origin :main", "git push --delete origin main", "git push -d origin main",
            "git push --force-with-lease origin main", "git push --force-if-includes origin main", "git push --prune origin",
            "(git reset --hard)", "cd sub && (git clean -fd)", "(cd frontend && rm -rf node_modules)",
            "echo $(rm -rf build)", "echo `rm -rf build`", "echo \"$(rm -rf build)\"",
            "sh -c 'rm -rf build'", "bash -lc \"git reset --hard\"", "eval 'git push -f'",
            "echo \"\\\"\" ; rm -rf build ; echo \"\\\"\"",
            "relay q guardrail.check $(rm -rf ~)",
        ] {
            assert!(denied(line), "{line:?} runs a denied command");
        }
    }

    /// D103 still holds: naming a command is not running it.
    #[test]
    fn quoted_mentions_and_near_misses_are_not_denied() {
        for line in [
            "echo \"rm -rf /\"", "git commit -m \"git reset --hard was a mistake\"",
            "relay q guardrail.check '{\"command\":\"rm -rf /\"}'", "rm -r build", "rm -f build",
            "git reset --soft HEAD~1", "git clean -n", "git push origin main", "git push -u origin main",
            "git push origin :", "git push --tags origin", "git push -o ci.skip origin relay/x", "git branch -d old",
            "git log --grep reset", "cargo test", "grep -rf patterns.txt src",
        ] {
            assert!(!denied(line), "{line:?} does not run a denied command");
        }
        // A dry run is exempt, and only the dry run.
        assert!(denied("relay q guardrail.check && rm -rf /tmp/x"));
    }

    /// An agent's Gradle invocation that would upload to Google Play, in every spelling Gradle
    /// accepts: any position among several tasks, a project path, other case, an abbreviation.
    #[test]
    fn a_play_publish_is_seen_in_every_gradle_spelling() {
        let publishes = |line: &str| super::play_publish(&shell_commands(line)).is_some();
        for line in [
            "./gradlew publishReleaseBundle", "./gradlew :app:publishReleaseBundle", "gradle publishBundle",
            "./gradlew promoteReleaseArtifact", "./gradlew publishListing", "./gradlew publishApk --track internal",
            "./gradlew assembleRelease publishReleaseBundle", "./gradlew --offline -q :app:bundleRelease :app:publishBundle",
            "cd android && ./gradlew clean publishReleaseBundle", "sh ./gradlew publishReleaseBundle",
            "/opt/gradle/bin/gradle publishBundle", "gradlew.bat publishBundle", "./gradlew PUBLISHRELEASEBUNDLE",
            "./gradlew pRB", "./gradlew pub-rel-bun", "./gradlew :app:pRA", "./gradlew uploadToPlay",
            "./gradlew -p app publishBundle", "bash -c './gradlew publishReleaseBundle'", "timeout 600 ./gradlew publishBundle",
        ] {
            assert!(publishes(line), "{line:?} publishes to Google Play");
        }
        for line in [
            "./gradlew bundleRelease", "./gradlew assembleRelease test lint", "./gradlew :app:installDebug",
            "./gradlew -x publishReleaseBundle build", "./gradlew -p publish assembleDebug", "./gradlew preBuild processDebugResources",
            "./gradlew projects properties", "echo ./gradlew publishReleaseBundle", "rg publishReleaseBundle app/build.gradle.kts",
            "git commit -m './gradlew publishBundle is not run here'", "./gradlew -Pplay.publish=true assembleRelease",
        ] {
            assert!(!publishes(line), "{line:?} does not publish");
        }
    }

    #[test]
    fn a_pattern_operand_must_be_present() {
        let patterns = ["rm -rf /".to_string(), "make deploy".to_string()];
        assert!(!denied_matches(&patterns, "rm -rf build").iter().any(|hit| hit.pattern == "rm -rf /"));
        assert!(denied_matches(&patterns, "rm -fr /").iter().any(|hit| hit.pattern == "rm -rf /"));
        assert!(!denied_matches(&patterns, "make -j8 deploy").is_empty());
        assert!(denied_matches(&patterns, "make test").is_empty());
    }

    #[test]
    fn self_approval_is_seen_inside_a_subshell() {
        assert!(self_approval("(relay --actor user q guardrail.confirm '{\"hold_id\":1}')").is_some());
        assert!(self_approval("echo $(relay q guardrail.confirm '{}')").is_some());
    }

    /// RA-099: a comment, a heredoc body written to a file, and the arguments of `echo` or a
    /// search are data. A comment no longer hides the next line behind its apostrophe either.
    #[test]
    fn comments_heredoc_data_and_plain_arguments_are_not_commands() {
        for line in [
            "echo rm -rf build", "printf '%s\\n' rm -rf build", "rg -n 'rm -rf' src", "grep -r git reset --hard .",
            "ls # rm -rf /", "cargo test # then git push --force",
            "cat <<'EOF' > notes.md\nrm -rf build is what broke it\nEOF",
            "cat > notes.md <<-EOF\n\tgit reset --hard\n\tEOF\necho done",
            "git commit -m \"$(cat <<'EOF'\nStop running rm -rf build by hand\nEOF\n)\"",
        ] {
            assert!(!denied(line), "{line:?} only mentions a denied command");
        }
        for line in [
            "ls # it's fine\nrm -rf build",
            "cat <<'EOF' > notes.md\nnotes\nEOF\nrm -rf build",
            "cat <<EOF | bash\nrm -rf build\nEOF",
            "bash <<'EOF'\ngit reset --hard\nEOF",
            "cat <<EOF > out.txt\n$(rm -rf build)\nEOF",
            "cat <<EOF\nrm -rf build",
            "echo x; rm -rf build",
        ] {
            assert!(denied(line), "{line:?} runs a denied command");
        }
    }

    /// RA-098: a script handed to a shell on standard input, `-c` in every shell, `env`, and
    /// the strings an interpreter's inline code runs.
    #[test]
    fn wrapped_commands_are_seen() {
        for line in [
            "echo 'rm -rf build' | sh", "echo rm -rf build | bash", "printf 'git reset --hard' | bash -s",
            "bash <<< 'rm -rf build'", "sh -c 'rm -rf build'", "zsh -c \"rm -rf build\"", "dash -c 'rm -rf build'",
            "env FOO=1 rm -rf build", "env -i bash -c 'git clean -fd'",
            "python3 -c \"import os; os.system('rm -rf build')\"",
            "python3 -c 'import subprocess; subprocess.run([\"rm\", \"-rf\", \"build\"])'",
            "perl -e 'system \"git push --force\"'",
            "node -e \"require('child_process').execSync('rm -rf build')\"",
            "python3 - <<'EOF'\nimport os\nos.system('git reset --hard')\nEOF",
        ] {
            assert!(denied(line), "{line:?} runs a denied command");
        }
        assert!(!denied("python3 -c 'print(1)'"));
        assert!(!denied("echo hello | sh"));
    }

    /// The 2026-10-24 survey's live bypasses: a unique prefix of a long option (git and GNU
    /// getopt both take one), a quoted program after a wrapper, and a git alias, inline or
    /// defined for later.
    #[test]
    fn abbreviations_quoted_programs_after_wrappers_and_aliases_are_seen() {
        for line in [
            // Abbreviated long options.
            "git push --dele origin main", "git push --del origin main", "git push --force-w origin main",
            "git push --force-wi=main origin main", "git push --mir origin", "git push --pru origin", "git push --forc origin main",
            "git reset --har HEAD~1", "git reset --ha", "git clean --fo -d", "rm --r --f build", "rm --rec --forc build",
            "rm -r --fo build",
            // A quoted program after a wrapper, an assignment or `find -exec`.
            "env 'rm' -rf build", "timeout 60 \"git\" reset --hard HEAD~1", "sudo 'rm' -rf build", "nice -n 5 'rm' -rf build",
            "FOO=1 'rm' -rf build", "echo build | xargs 'rm' -rf", "find . -exec 'rm' -rf {} ;", "env 'bash' -c 'rm -rf build'",
            "env 'python3' -c \"import os; os.system('rm -rf build')\"", "sudo env 'git' push --dele origin main",
            // After a shell keyword, a wrapper no list names, a nested wrapper; and `env -S`.
            "if 'rm' -rf build; then :; fi", "for d in a; do 'rm' -rf build; done", "{ 'rm' -rf build; }", "! 'rm' -rf build",
            "uv run 'rm' -rf build", "pkexec 'rm' -rf build", "find . -exec sudo 'rm' -rf {} ;", "find . -exec env 'rm' -rf {} +",
            "env -S 'rm -rf build'", "env -S\"rm -rf build\"", "env -iS 'git reset --hard'", "env --split-string='rm -rf build'",
            "env -u HOME -S 'rm -rf build'",
            // A global option that takes the next word.
            "git --attr-source HEAD reset --hard", "git --attr-source HEAD push --dele origin main",
            // Git aliases.
            "git -c alias.nuke='!rm -rf build' nuke", "git -c alias.r='reset --hard' r", "git -c alias.r='reset' r --hard",
            "git -c Alias.P='push --delete' p origin main", "git -c alias.a=b -c alias.b='!rm -rf build' a",
            "git config alias.nuke '!rm -rf build'", "git config --global alias.zap 'reset --hard'",
            "git config set alias.zap 'clean -fd'", "git config --file ../x alias.zap '!rm -rf build'",
            "X='!rm -rf build' git --config-env=alias.nuke=X nuke",
            "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.nuke GIT_CONFIG_VALUE_0='!rm -rf build' git nuke",
            "GIT_CONFIG_PARAMETERS=\"'alias.nuke'='!rm -rf build'\" git nuke",
            "bash -c \"git -c alias.nuke='!rm -rf build' nuke\"",
            // An alias of an alias with the arguments on the call, an alias called twice, the
            // older `'key=value'` form.
            "git -c alias.a=b -c alias.b=reset a --hard", "git config alias.r reset && git r --soft HEAD && git r --hard",
            "GIT_CONFIG_PARAMETERS=\"'alias.nuke=!rm -rf build'\" git nuke",
        ] {
            assert!(denied(line), "{line:?} runs a denied command");
        }
        for line in [
            "git push --dry-run origin main", "git push --tags origin", "git push --porcelain origin main",
            "git push --no-force origin main", "git reset --soft HEAD~1", "git clean --dry-run -d", "rm --verbose build",
            "rm --dir build", "git log --hard-to-say", "echo 'rm' -rf build", "rg 'rm' -rf src",
            "ls 'rm -rf build'", "env 'rm -rf build'", "sudo apt install 'git'", "env -S 'cargo test'",
            "git commit -m 'rm -rf build was slow'",
            "git -c alias.st='status -sb' st", "git config alias.lg 'log --oneline --graph'", "git config --get alias.nuke",
            "git -c user.name=x commit -m 'alias.r=reset --hard'",
        ] {
            assert!(!denied(line), "{line:?} does not run a denied command");
        }
    }

    /// RA-096 / RA-097: the CLI's envelope form, every way of shedding the session identity,
    /// and the raw socket; and a search for the op names is not an invocation.
    #[test]
    fn self_approval_covers_envelopes_and_identity_shedding_but_not_searches() {
        for line in [
            "relay cmd '{\"op\":\"guardrail.confirm\",\"payload\":{\"hold_id\":1}}'",
            "$RELAY_BIN cmd '{\"actor\":\"user\",\"op\":\"task.list\",\"payload\":{}}'",
            "echo '{\"actor\": \"user\", \"op\": \"guardrail.confirm\", \"payload\": {\"hold_id\": 1}}' | socat - UNIX-CONNECT:/run/relay.sock",
            "env -i relay q task.list", "env -i PATH=/usr/bin relay q task.list",
            "env --unset=RELAY_SESSION relay q task.list", "env -uRELAY_SESSION relay q task.list",
            "unset -v RELAY_TOKEN RELAY_SESSION", "export RELAY_SESSION=", "RELAY_ACTOR=user relay q task.list",
            "sudo -u me relay q task.list", "cargo run -p relay-cli -- --actor user q task.list",
            "/tmp/renamed --actor user q task.list", "relay --instance dev q guardrail.confirm '{}'",
            "relay q \"$OP\" '{}'", "echo guardrail.confirm | xargs relay q",
            "bash -c 'relay --actor user q task.list'", "echo 'relay q guardrail.confirm {}' | sh",
            "python3 -c \"import os; os.system('relay --actor user q guardrail.confirm {}')\"",
            "python3 -c 'import subprocess; subprocess.run([\"relay\", \"q\", \"guardrail.reject\", \"{}\"])'",
        ] {
            assert!(self_approval(line).is_some(), "{line:?} answers as the user");
        }
        for line in [
            "rg guardrail.confirm crates/relay-core", "grep -rn 'RELAY_SESSION=' crates", "git log --grep guardrail.confirm",
            "relay q guardrail.check '{\"command\":\"relay --actor user q guardrail.confirm\"}'",
            "cargo test -p relay-core guardrail", "$RELAY_BIN q session.done '{\"summary\":\"fixed guardrail.confirm\"}'",
            "relay q task.list", "env RUST_LOG=debug cargo test",
        ] {
            assert!(self_approval(line).is_none(), "{line:?} is not self-approval");
        }
        // The dry-run exemption is the call itself, not a mention of it.
        assert!(denied("relay q guardrail.confirm guardrail.check && rm -rf x"));
        assert!(!denied("relay q guardrail.check '{\"command\":\"rm -rf /\"}'"));
    }

    /// RA-109: every way of switching off the pre-commit hook for an agent's own commit, and
    /// the look-alikes that do not.
    #[test]
    fn hook_bypasses_are_seen_and_look_alikes_are_not() {
        let bypass = |line: &str| hook_bypass(&shell_commands(line)).is_some();
        for line in [
            "git commit --no-verify -m wip", "git commit -n -m wip", "git commit -anm wip", "git commit -am wip -n",
            "git commit --no-verif -m x", "git commit --no-verify=1", "git commit \"-n\" -m x", "git -C app commit -n",
            "/usr/bin/git commit -n", "env FOO=1 git commit -n -m x", "bash -c 'git commit --no-verify'",
            "cd app && git add . && git commit -nm x", "timeout 60 git commit -n",
            "git -c core.hooksPath=/dev/null commit -m x", "git -c core.hookspath= commit -m x", "git -c CORE.HOOKSPATH=x merge main",
            "git --config-env=core.hooksPath=EMPTY commit", "git --config-env core.hooksPath=EMPTY commit",
            "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/dev/null git commit -m x",
            "export GIT_CONFIG_PARAMETERS=\"'core.hooksPath'='/dev/null'\"",
            "git config core.hooksPath /dev/null", "git config --worktree core.hooksPath .",
            "git config --worktree --unset core.hooksPath", "git config --unset-all core.hookspath",
            "git config unset core.hooksPath", "git config set core.hooksPath x", "git config --remove-section core",
            "git config -f .git/config.worktree core.hooksPath x",
            // The pre-push check, and both hooks through an alias or a quoted program.
            "git push --no-verify", "git push --no-verify origin relay/x", "git push origin relay/x --no-veri",
            "git -C app push --no-verify", "env 'git' push --no-verify", "env 'git' commit -n -m x",
            "git -c alias.ci='commit --no-verify' ci -m x", "git -c alias.ci=commit ci -n -m x",
            "git -c alias.pu='push --no-verify' pu origin main", "git config alias.ci 'commit -n'",
            "if 'git' commit -n -m x; then :; fi", "git --attr-source HEAD commit -n", "git --attr-source HEAD push --no-verify",
            // Any alias at all: a later line calling it cannot be told from another command.
            "git config alias.ci commit", "git config --global alias.r reset", "git -c alias.st=status st",
            "git -c alias.ci=commit ci -m x", "GIT_CONFIG_PARAMETERS=\"'alias.x=!true'\" git x",
            "export GIT_CONFIG_KEY_0=alias.x; export GIT_CONFIG_VALUE_0=commit",
        ] {
            assert!(bypass(line), "{line:?} switches the hook off");
        }
        for line in [
            "git commit -m wip", "git commit -am 'fix -n handling'", "git commit -m -n", "git commit -m 'no -n here'",
            "git commit --amend --no-edit", "git commit -F notes.txt", "git commit -- -n", "git commit --verbose",
            "git log -n 3", "git merge -n main", "git diff --no-index a b", "git -c user.name=x commit -m y",
            "git config core.hooksPath", "git config --get core.hooksPath", "git config get core.hooksPath", "git config --list",
            "git config user.name x", "echo git commit -n", "rg 'commit --no-verify' docs", "grep -rn core.hooksPath crates",
            "relay q guardrail.check '{\"command\":\"git commit -n\"}'",
            "git push -n origin main", "git push --no-ve origin main", "git push --verify origin main", "git push origin -- --no-verify",
            "git push -o --no-verify origin main", "git config --get alias.ci", "git config alias.ci", "git log --grep alias.x=y",
            "echo GIT_CONFIG_KEY_0=alias.x GIT_CONFIG_VALUE_0=commit",
        ] {
            assert!(!bypass(line), "{line:?} leaves the hook on");
        }
    }
}

#[cfg(test)]
mod probe_tests {
    use super::{config, evaluate_with, Decision, GateKind, GateRequest, Probes};
    use relay_bus::Actor;

    /// RA-378: a diff-only write that removes nothing never reads the file it lands on; one that
    /// removes lines measures it.
    #[test]
    fn a_diff_that_removes_nothing_does_not_read_the_old_file() {
        let store = crate::Store::open_memory().unwrap();
        let cfg = config(&store.lock(), None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        let judge = |diff: &str| {
            let probes = Probes::default();
            let request = GateRequest {
                actor: &Actor::User, project_id: 1, worktree: dir.path(), kind: GateKind::Write, path: Some("a.txt"),
                new_text: None, diff: Some(diff), command: None, skip_policy: None, grants: None, path_only: false,
                probes: Some(&probes),
            };
            assert!(matches!(evaluate_with(&cfg, &request).unwrap(), Decision::Allow));
            let read = probes.old.borrow().len();
            read
        };
        assert_eq!(judge("--- a/a.txt\n+++ b/a.txt\n@@ -2,0 +3 @@\n+three\n"), 0);
        assert_eq!(judge("--- a/a.txt\n+++ b/a.txt\n@@ -2 +2 @@\n-two\n+2\n"), 1);
    }
}

#[cfg(test)]
mod path_tests {
    use super::{diff_counts, parse_numstat, resolve, split_commits, NumstatEntry};
    use std::path::Path;

    fn paths(text: &str) -> Vec<Vec<String>> {
        parse_numstat(text).into_iter().map(|entry| entry.paths).collect()
    }

    /// RA-095: every way git can spell a path in numstat names the real path.
    #[test]
    fn numstat_reads_renames_quoting_and_nul_separated_records() {
        assert_eq!(
            parse_numstat("3\t1\tsecret/key\0-\t-\tlogo.png\0"),
            vec![
                NumstatEntry { added: 3, removed: 1, paths: vec!["secret/key".into()] },
                NumstatEntry { added: 0, removed: 0, paths: vec!["logo.png".into()] },
            ]
        );
        assert_eq!(paths("0\t0\t\0old/key\0secret/key\0"), vec![vec!["old/key".to_string(), "secret/key".into()]]);
        assert_eq!(paths("0\t0\told => secret/key\n"), vec![vec!["old".to_string(), "secret/key".into()]]);
        assert_eq!(paths("1\t2\tsrc/{a => secret}/key\n"), vec![vec!["src/a/key".to_string(), "src/secret/key".into()]]);
        assert_eq!(paths("1\t0\t{ => secret}/key\n"), vec![vec!["/key".to_string(), "secret/key".into()]]);
        assert_eq!(paths("1\t0\t\"caf\\303\\251/k\\tey\"\n"), vec![vec!["café/k\tey".to_string()]]);
    }

    /// RA-321: inside a hunk `--- x` is a removed `-- x`; only the file headers are skipped.
    #[test]
    fn diff_counts_skips_headers_but_not_removed_comment_lines() {
        let diff = "diff --git a/q.sql b/q.sql\n--- a/q.sql\n+++ b/q.sql\n@@ -1,4 +1,2 @@\n--- one\n--- two\n select 1;\n-select 2;\n+++ added\n\
                    --- b.yml\n+++ b.yml\n@@ -1 +1 @@\n----\n+++\n";
        assert_eq!(diff_counts(diff), (4, 2));
        // No hunk headers at all: the old reading, headers skipped and every marked line counted.
        assert_eq!(diff_counts("--- a\n+++ b\n-x\n+y\n+z\n"), (1, 2));
    }

    /// RA-109: `diff-tree --stdin -z` output, one commit after another, renames and all.
    #[test]
    fn diff_tree_output_splits_per_commit() {
        let out = "aaaa\x001\t0\ta\x000\t0\t\x00s/key\x00s/key2\x00bbbb\x001\t0\tb\x00";
        let commits = split_commits(out);
        assert_eq!(commits.iter().map(|(sha, _)| sha.as_str()).collect::<Vec<_>>(), ["aaaa", "bbbb"]);
        assert_eq!(
            parse_numstat(&commits[0].1).into_iter().map(|entry| entry.paths).collect::<Vec<_>>(),
            vec![vec!["a".to_string()], vec!["s/key".to_string(), "s/key2".into()]]
        );
        assert_eq!(parse_numstat(&commits[1].1)[0].paths, vec!["b".to_string()]);
    }

    /// RA-101: a symlink, dangling or not, and `..` after one, lead where the OS would go.
    #[test]
    fn resolve_follows_links_like_the_kernel() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("real/secret")).unwrap();
        std::os::unix::fs::symlink(root.join("real/secret"), root.join("link")).unwrap();
        std::os::unix::fs::symlink("real/secret/new.key", root.join("dangling")).unwrap();
        assert_eq!(resolve(&root.join("link/key")), root.join("real/secret/key"));
        assert_eq!(resolve(&root.join("dangling")), root.join("real/secret/new.key"));
        assert_eq!(resolve(&root.join("link/../x")), root.join("real/x"));
        assert_eq!(resolve(&root.join("./a//b")), root.join("a/b"));
        assert_eq!(resolve(Path::new("/")), Path::new("/"));
    }
}

#[cfg(test)]
mod measure_tests {
    use super::{config, measure_branch};
    use std::path::Path;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) {
        let out = Command::new("git").arg("-C").arg(repo).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    /// A base branch read from a cloned repository's `HEAD` can be named `--output=<path>`;
    /// measuring a done against it never hands that to git as an option, so no file appears.
    #[test]
    fn a_base_that_looks_like_an_option_writes_no_file() {
        let store = crate::Store::open_memory().unwrap();
        let cfg = config(&store.lock(), None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        git(&repo, &["add", "a.txt"]);
        git(&repo, &["commit", "-q", "-m", "work"]);
        // Relative, so it lands in the worktree git runs in (a temp dir's `.tmp…` component is
        // not a valid ref name); a cloned remote's HEAD can as well name an absolute path.
        let base = "--output=pwned";
        git(&repo, &["update-ref", &format!("refs/heads/{base}"), "HEAD~1"]);

        let unmeasured = measure_branch(&cfg, &repo, base).unwrap().unmeasured;
        assert!(unmeasured.is_some_and(|why| why.contains("starts with '-'")), "an option-like base is not measured");
        let written: Vec<_> = std::fs::read_dir(&repo).unwrap().map(|entry| entry.unwrap().file_name()).filter(|name| name.to_string_lossy().starts_with("pwned")).collect();
        assert!(written.is_empty(), "git wrote a file named by the base branch: {written:?}");

        // An ordinary base still measures, revisions after `--end-of-options` and all.
        let work = measure_branch(&cfg, &repo, "main~1").unwrap();
        assert!(work.range.contains("a.txt"), "{:?}", work.range);
        assert_eq!(work.commits.len(), 1);
        assert!(!work.partial);
        assert_eq!(work.unmeasured, None);
    }
}
