//! Entity shapes returned by ops (BUS.md §11) and the small enums they share.
//!
//! Timestamps are RFC 3339 UTC strings so this crate stays free of a time dependency;
//! `relay-core` produces them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Store row id (`i64` autoincrement, BUS.md §2).
pub type Id = i64;
/// RFC 3339 UTC.
pub type Ts = String;

// ---------------------------------------------------------------- enums

/// SPEC §6: five hardcoded columns.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Backlog,
    InReview,
    Ready,
    Active,
    Done,
}

/// Agent lifecycle on a task, independent of column (BUS.md §11.1, confirmed 2026-08-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    None,
    Dispatched,
    Running,
    Blocked,
    Failed,
    AwaitingReview,
}

/// BUS.md §11.1 (confirmed 2026-08-17).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Size {
    S,
    M,
    L,
}

/// A first-class classification, separate from the free-form `labels` (BUS.md §11.1).
/// `Task` is the neutral default and is what every pre-v16 row migrated to; the other four
/// are the vocabulary the board filters on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Task,
    Feature,
    Bug,
    Chore,
    Spike,
}

/// Task-to-task edges other than parent/child (BUS.md §11.1). Only these two are stored;
/// `blocks` is the inverse read of `blocked_by` and never has a row of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskRelation {
    BlockedBy,
    DuplicateOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Builder,
    Reviewer,
    Docs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Created,
    Spawning,
    Running,
    Idle,
    Blocked,
    Parked,
    Restorable,
    Exited,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoldState {
    Open,
    Confirmed,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Ok,
    Held,
    Refused,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotifyCategory {
    AgentDone,
    AgentBlocked,
    Guardrail,
    Integration,
    Provider,
    Disk,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
    Queued,
    Merging,
    Building,
    Deploying,
    Passed,
    Failed,
    Conflict,
    Discarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Allow,
    Refuse,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Write,
    Commit,
    Exec,
}

/// Per-task change caps (SPEC §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardrailCaps {
    pub files: u32,
    pub lines: u32,
}

/// A write is destructive when either threshold is exceeded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DestructiveWrite {
    pub min_removed_lines: u32,
    pub min_removed_pct: f64,
    /// The percentage rule is skipped for files shorter than this. A percentage of a very
    /// short file measures nothing: one line of a one-line file is 100%.
    #[serde(default = "default_min_file_lines")]
    pub min_file_lines: u32,
    /// Let a large rewrite through when git can put the file back — committed and unmodified,
    /// or ignored entirely. Set false to hold on volume alone.
    #[serde(default = "default_true")]
    pub allow_if_recoverable: bool,
}

fn default_true() -> bool {
    true
}

fn default_min_file_lines() -> u32 {
    30
}

/// A built-in validator attached to one critical, worktree-relative path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeGate {
    pub path: String,
    /// `non_empty` | `json` | `json_non_empty_array` | `json_non_empty_object`.
    pub validator: String,
}

/// Role allow-sets are configurable, but the defaults are the baseline in BUS.md §9.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleAllowlist {
    pub builder: Vec<String>,
    pub reviewer: Vec<String>,
    pub docs: Vec<String>,
}

/// Effective guardrail configuration after global + project merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardrailConfig {
    pub caps: GuardrailCaps,
    pub destructive_write: DestructiveWrite,
    pub protected_paths: Vec<String>,
    pub shape_gates: Vec<ShapeGate>,
    /// Command patterns refused for `kind: exec`. Matched against the *parsed argv* of each
    /// command in the line, never as a raw substring, so quoted data mentioning one is fine.
    pub denied_commands: Vec<String>,
    /// Absolute roots an agent may write to outside its worktree — scratch space, which is
    /// not a repo-integrity concern. `$TMPDIR` is always honoured in addition to these.
    #[serde(default)]
    pub allowed_write_roots: Vec<String>,
    pub roles: RoleAllowlist,
    /// Whether an agent's `integration.request` may run the project's build command without a
    /// person confirming it. Off by default: the build runs outside any sandbox (RA-167), so an
    /// agent's request that builds is held for confirmation; a merge-only request is not.
    #[serde(default)]
    pub agent_builds: bool,
    /// Whether an agent's `git push` first runs the project's check — its `build_cmd`, else
    /// the test command of the build tool at the checkout's root — and is refused when the
    /// check fails. On by default. A person's own push never runs it, so pushing a branch does
    /// not execute what that branch's code does.
    #[serde(default = "default_true")]
    pub check_on_push: bool,
}

impl GuardrailConfig {
    /// Remove every key `deny_unknown_fields` would reject — an object whose schema says
    /// `additionalProperties: false` keeps only its named properties — and return them as
    /// dotted paths. Works on a partial tree too: absent keys are not this function's concern.
    pub fn strip_unknown(value: &mut Value) -> Vec<String> {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        let schema = SCHEMA.get_or_init(|| schemars::schema_for!(GuardrailConfig).to_value());
        let mut unknown = Vec::new();
        strip_unknown_at(value, schema, schema, "", &mut unknown);
        unknown
    }
}

fn strip_unknown_at(value: &mut Value, schema: &Value, root: &Value, at: &str, unknown: &mut Vec<String>) {
    let schema = match schema.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")) {
        Some(name) => &root["$defs"][name],
        None => schema,
    };
    let join = |key: &str| if at.is_empty() { key.to_string() } else { format!("{at}.{key}") };
    match value {
        Value::Object(map) => {
            let Some(props) = schema.get("properties").and_then(Value::as_object) else { return };
            if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                map.retain(|key, _| props.contains_key(key) || { unknown.push(join(key)); false });
            }
            for (key, child) in map.iter_mut() {
                if let Some(sub) = props.get(key) {
                    strip_unknown_at(child, sub, root, &join(key), unknown);
                }
            }
        }
        Value::Array(items) => {
            if let Some(sub) = schema.get("items") {
                for (i, item) in items.iter_mut().enumerate() {
                    strip_unknown_at(item, sub, root, &join(&i.to_string()), unknown);
                }
            }
        }
        _ => {}
    }
}

/// Which layer of guardrail configuration a value lives in. Each layer overrides the one
/// before it: defaults, then global, then the project's workspace, then the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailLayer {
    Default,
    Global,
    Workspace,
    Project,
}

/// What an agent asks to be let past. `command` lifts a denied command, `path` a protected
/// path, a write root or a destructive-write rule, `cap` the commit caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionKind {
    Command,
    Path,
    Cap,
}

/// How long an approved exception lasts: one use, or until the session ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GrantScope {
    Once,
    Session,
}

/// An agent's request to be let past one guardrail, and — once a person answered — the grant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GuardrailException {
    /// Also the id of the hold that carries it, so `guardrail.confirm` / `guardrail.reject`
    /// answer it with `hold_id` set to this.
    pub id: Id,
    pub project_id: Option<Id>,
    pub session: Option<String>,
    pub kind: ExceptionKind,
    /// The exact command, the path (worktree-relative, a glob, or an absolute root), or the
    /// caps wanted (`files=N lines=M`; empty lifts the caps for the grant's life).
    pub value: String,
    pub reason: String,
    pub requested_scope: GrantScope,
    pub state: HoldState,
    /// Set once approved.
    pub scope: Option<GrantScope>,
    /// Approved, not revoked, and (for `once`) not yet used.
    pub active: bool,
    pub uses: u32,
    pub used_at: Option<Ts>,
    pub revoked_at: Option<Ts>,
    pub denial_reason: Option<String>,
    pub created_at: Ts,
    pub resolved_at: Option<Ts>,
    pub resolved_by: Option<crate::envelope::Actor>,
}

// ---------------------------------------------------------------- entities

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Workspace {
    pub id: Id,
    pub path: String,
    pub name: String,
    pub order: i64,
    pub created_at: Ts,
    pub updated_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: Id,
    pub workspace_id: Id,
    pub path: String,
    pub name: String,
    pub base_branch: String,
    pub build_cmd: Option<String>,
    pub run_cmd: Option<String>,
    pub protected_paths: Vec<String>,
    pub critical_files: Vec<String>,
    pub order: i64,
    pub pinned: bool,
    pub created_at: Ts,
    pub updated_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Attachment {
    pub id: Id,
    pub task_id: Id,
    pub name: String,
    pub mime: String,
    pub bytes: i64,
    /// Path under the store's attachment dir.
    pub path: String,
    pub created_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskCommit {
    pub sha: String,
    pub branch: Option<String>,
    pub linked_at: Ts,
}

/// Roll-up of a parent's descendants (BUS.md §11.1). Counts the whole subtree, not just the
/// direct children, so a grandparent reads as one body of work. Zero `total` = a leaf.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskRollup {
    pub total: i64,
    pub done: i64,
}

/// A free-form supplementary tag, scoped to a project. Types classify; labels annotate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    pub id: Id,
    pub project_id: Id,
    pub name: String,
    pub created_at: Ts,
}

/// One entry in a task's discussion, GitHub-issue style. `author` is the session name for an
/// agent and `user` for a person, as `mailbox.send` renders `from`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskComment {
    pub id: Id,
    pub task_id: Id,
    pub author: String,
    pub body: String,
    pub created_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    pub id: Id,
    pub project_id: Id,
    pub module_id: Option<Id>,
    /// The name of `module_id`'s module, completed (archived) ones included, so a card can be
    /// labelled without a `module.list` (RA-495). Absent from an older engine.
    #[serde(default)]
    pub module_name: Option<String>,
    pub title: String,
    pub body: String,
    pub changelog: String,
    pub column: Column,
    pub position: i64,
    pub state: TaskState,
    pub priority: Priority,
    pub size: Option<Size>,
    #[serde(rename = "type")]
    #[schemars(rename = "type")]
    pub task_type: TaskType,
    /// Parent task, if this is a sub-task. Nesting is capped at [`TASK_DEPTH_MAX`] levels.
    pub parent_id: Option<Id>,
    /// 0 for a root task, 1 for its child, and so on — never above `TASK_DEPTH_MAX - 1`.
    pub depth: i64,
    /// Direct children, in board order. Children stay real tasks with their own cards.
    pub children: Vec<Id>,
    /// Progress over the whole subtree, for the parent's `n/m done` badge.
    pub rollup: TaskRollup,
    /// Free-form tags, sorted; the supplementary layer under `type`.
    pub labels: Vec<String>,
    /// Tasks this one waits on (stored edges).
    pub blocked_by: Vec<Id>,
    /// Tasks waiting on this one (the inverse read of `blocked_by`).
    pub blocks: Vec<Id>,
    /// The task this duplicates, if any.
    pub duplicate_of: Option<Id>,
    /// The note this task was made from, if any (`task.create` with `note_id`). Cleared when
    /// that note is purged; absent from an older engine.
    #[serde(default)]
    pub note_id: Option<Id>,
    /// Every session ever dispatched to it, in order; a PAIR adds two.
    pub sessions: Vec<String>,
    pub commits: Vec<TaskCommit>,
    pub attachments: Vec<Attachment>,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub deleted_at: Option<Ts>,
}

/// How deep sub-tasks may nest: a root plus two levels of children. GitHub allows 8; a board
/// that must stay readable as columns of cards does not, so the bus refuses the fourth level
/// with `task.depth`.
pub const TASK_DEPTH_MAX: i64 = 3;

/// How many direct children one parent may hold, matching GitHub's sub-issue cap.
pub const TASK_CHILDREN_MAX: i64 = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Module {
    pub id: Id,
    pub project_id: Id,
    pub name: String,
    pub icon: Option<String>,
    pub priority: Priority,
    pub order: i64,
    pub completed_at: Option<Ts>,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub deleted_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleSummary {
    #[serde(flatten)]
    pub module: Module,
    /// Task counts by column.
    pub counts: std::collections::BTreeMap<Column, i64>,
    pub progress_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleHeader {
    pub count: i64,
    pub in_flight: i64,
    pub issues: i64,
    pub completed: i64,
    pub completion_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Note {
    pub id: Id,
    pub project_id: Id,
    pub title: Option<String>,
    pub body: String,
    pub pinned: bool,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub deleted_at: Option<Ts>,
}

/// A design canvas (`design.*`): pages of frames, shapes, text and pictures in a project,
/// optionally tied to one terminal. `design.get` has its nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Design {
    pub id: Id,
    pub project_id: Id,
    /// The terminal it belongs to: the agent session that made it, or the one it was linked
    /// to. `None` when it belongs to the project alone.
    pub session: Option<String>,
    pub title: String,
    /// Bumped by every change to it; a `rev` above the one you last read means someone else
    /// changed it since.
    pub rev: i64,
    pub pages: i64,
    pub nodes: i64,
    /// `user` or the session that made it.
    pub created_by: String,
    /// `user` or the session that changed it last.
    pub updated_by: String,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub deleted_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    pub id: Id,
    pub project_id: Id,
    pub from: String,
    /// Session name or `*`.
    pub to: String,
    pub text: String,
    pub re_task: Option<Id>,
    #[serde(default)]
    pub priority: bool,
    pub sent_at: Ts,
    pub acked_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Session {
    pub id: Id,
    pub name: String,
    /// One line of what this session is doing, in its own words (`session.intent`).
    #[serde(default)]
    pub intent: Option<String>,
    pub project_id: Id,
    pub provider: Provider,
    pub role: Role,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub branch: String,
    pub worktree: String,
    pub task_id: Option<Id>,
    pub module_id: Option<Id>,
    pub pair_with: Option<String>,
    pub bus_writes: bool,
    pub allow_ui: bool,
    /// Claude runs on the API key in the local keyring instead of the signed-in account. Relay
    /// never reads the key: Claude does, with `apiKeyHelper`. The agent can read it too.
    #[serde(default)]
    pub use_api_key: bool,
    pub state: SessionState,
    pub pid: Option<i64>,
    pub exit_code: Option<i64>,
    /// Provider's own resume handle.
    pub provider_ref: Option<String>,
    pub spawned_at: Option<Ts>,
    pub last_output_at: Option<Ts>,
    /// Provider units, opaque.
    pub usage: Option<Value>,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub closed_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Peer {
    pub session: String,
    pub provider: Provider,
    pub role: Role,
    pub branch: String,
    pub state: SessionState,
    pub task_title: Option<String>,
    pub claimed: Vec<String>,
    pub last_output_at: Option<Ts>,
    /// One line of what this peer is doing, in its own words (`session.intent`). Coordination
    /// needs to know what a peer is *up to*, which no amount of state and branch names says.
    pub intent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Overlap {
    pub id: Id,
    pub project_id: Id,
    pub sessions: Vec<String>,
    pub path: String,
    pub symbol: Option<String>,
    pub kind: OverlapKind,
    pub acked_by: Vec<String>,
    pub first_seen: Ts,
    pub last_seen: Ts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverlapKind {
    File,
    Symbol,
    Claim,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hold {
    pub id: Id,
    pub project_id: Option<Id>,
    pub session_id: Option<Id>,
    pub session: Option<String>,
    pub actor: crate::envelope::Actor,
    pub op: String,
    pub payload_hash: String,
    pub policy: String,
    pub details: Value,
    pub state: HoldState,
    pub created_at: Ts,
    pub resolved_at: Option<Ts>,
    pub resolved_by: Option<crate::envelope::Actor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UndoOp {
    pub op: String,
    pub payload: Value,
    /// `{updated_at}` of the entity after the op; `audit.undo` refuses on mismatch (§5.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditRow {
    pub id: Id,
    pub ts: Ts,
    pub req_id: String,
    pub parent_req: Option<String>,
    pub actor: crate::envelope::Actor,
    pub on_behalf_of: Option<crate::envelope::Actor>,
    pub session_id: Option<Id>,
    pub op: String,
    pub project_id: Option<Id>,
    pub kind: AuditKind,
    pub code: Option<String>,
    pub hold_id: Option<Id>,
    pub payload_hash: String,
    /// `null` when the payload was larger than 64 KiB.
    pub payload: Option<Value>,
    pub result_summary: Option<Value>,
    pub undo_op: Option<UndoOp>,
    pub undo_of: Option<Id>,
    pub undone_by: Option<Id>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Notification {
    pub id: Id,
    pub project_id: Option<Id>,
    pub category: NotifyCategory,
    pub title: String,
    pub body: String,
    /// Deep link: an op to run when clicked.
    pub link: Option<crate::error::Confirm>,
    pub read: bool,
    pub created_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
    pub head: String,
    pub session: Option<String>,
    pub dirty: bool,
    pub disk_mb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Integration {
    pub id: Id,
    pub project_id: Id,
    pub branches: Vec<String>,
    pub worktree: Option<String>,
    pub state: IntegrationState,
    pub conflict: Option<(String, String)>,
    pub log_tail: String,
    pub started_at: Option<Ts>,
    pub finished_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderInfo {
    pub provider: Provider,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub signed_in_as: Option<String>,
    pub last_seen_version: Option<String>,
    /// How Relay spawns it: flags, env, hook file layout. Opaque, for display and diagnosis.
    pub spawn_profile: Value,
    /// Whether this provider supports Relay's `PreToolUse` guardrail and lifecycle adapters.
    /// Provider-native trust may still require one explicit approval before a generated hook runs.
    pub guarded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    pub provider: Provider,
    /// Provider's own units and windows, opaque; the UI knows each provider's shape.
    pub windows: Value,
    pub taken_at: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Skill {
    pub id: Id,
    pub name: String,
    /// What the skill is for: the `description` of its SKILL.md front matter, else the first
    /// line of prose after it; empty when there is neither. Read from the whole body, so it is
    /// there even when `skill.list {summary: true}` cuts the body short.
    #[serde(default)]
    pub description: String,
    pub body: String,
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    /// The branch or tag a GitHub skill was installed from; `None` is the repository's
    /// default branch (or a local skill). A refresh pulls this ref again.
    pub source_ref: Option<String>,
    pub revision: Option<String>,
    pub enabled_in: Vec<Id>,
    pub created_at: Ts,
    pub updated_at: Ts,
}

/// A plugin bundled with Relay: skills, always-on agent instructions, documentation and MCP
/// servers that reach every agent of a project the plugin is switched on for (D159).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub summary: String,
    pub description: String,
    pub skills: Vec<PluginSkill>,
    pub mcp_servers: Vec<PluginMcpServer>,
    /// Documentation files, relative to the plugin root; `plugin.get` returns their text.
    pub docs: Vec<String>,
    /// Projects the plugin is switched on for.
    pub enabled_in: Vec<Id>,
    /// Projects whose checkout looks like this plugin's kind of project (for Unreal Engine, a
    /// `.uproject` at the root) and that do not have it on yet.
    pub suggested_for: Vec<Id>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginSkill {
    /// The skill's folder name, which is also its provider-visible name.
    pub name: String,
    pub description: String,
    /// Files in the skill folder, `SKILL.md` included.
    pub files: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginMcpServer {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginDoc {
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GitHubStatus {
    pub installed: bool,
    pub connected: bool,
    pub login: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GitHubRepo {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub clone_url: String,
    pub ssh_url: String,
    pub private: bool,
    pub archived: bool,
    pub updated_at: Option<Ts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocalRepo {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    pub serial: String,
    pub model: String,
    pub kind: DeviceKind,
    pub state: String,
    /// Who is using this device right now, if anyone. A conflicting run, install or claim
    /// from anyone else is refused with `device.busy` until it is released.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<DeviceLease>,
}

/// One holder's exclusive use of an Android device: a `device.run` in flight, an agent's
/// `adb install` / `gradlew install*` seen by the guardrail hook, or an explicit `device.claim`.
/// Released with a `device.lease.released` event, so `bus.wait` can wait for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceLease {
    /// The device serial, or `*` when the command named no device (adb's only device, or
    /// Gradle's every device): that lease conflicts with every device.
    pub device: String,
    /// The holding session's name; `None` when the user holds it directly.
    pub session: Option<String>,
    /// `run`, `shell` or `claim`.
    pub kind: String,
    /// What the holder is doing, in a few words (`device.run debug from relay/brisk-otter`,
    /// `adb install app-debug.apk`).
    pub action: String,
    pub since: Ts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Id>,
    /// Seconds until the lease lapses on its own; `None` while it lasts as long as its run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_s: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Usb,
    Avd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Avd {
    pub name: String,
    pub device: Option<String>,
    pub package: Option<String>,
    pub path: Option<String>,
    pub running_serial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Run {
    pub id: Id,
    pub project_id: Id,
    /// `run` installs and launches on `device`; `build` produces an artifact and
    /// leaves `device` empty (D152).
    pub kind: String,
    pub device: String,
    pub worktree: String,
    pub state: String,
    /// Absolute path of the APK / AAB a finished `build` produced.
    pub artifact: Option<String>,
    /// Gradle variant and artifact format for a release build; absent for device runs.
    pub variant: Option<String>,
    pub format: Option<String>,
    /// True when this build was also handed to Gradle Play Publisher.
    pub publish: bool,
    /// `signed`, `unsigned`, or `unverified` after Relay inspects the artifact.
    pub signing: Option<String>,
    pub started_at: Ts,
    pub finished_at: Option<Ts>,
}

// ---------------------------------------------------------------- git / files

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FileStatus {
    pub path: String,
    /// Porcelain-style: `M`, `A`, `D`, `R`, `?`, `!`, `U`.
    pub index: String,
    pub worktree: String,
    pub renamed_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub added: i64,
    pub removed: i64,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hunk {
    pub old_start: i64,
    pub old_lines: i64,
    pub new_start: i64,
    pub new_lines: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Commit {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub at: Ts,
    pub subject: String,
    pub body: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Branch {
    pub name: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: Option<i64>,
    pub behind: Option<i64>,
    pub merged: bool,
    pub session: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entry {
    /// Relative to the worktree root.
    pub path: String,
    pub name: String,
    pub kind: EntryKind,
    pub size: Option<i64>,
    pub modified_at: Option<Ts>,
    /// Git status badge, if any (`M`, `A`, `D`, `?`).
    pub badge: Option<String>,
    pub children: Option<Vec<Entry>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

// ---------------------------------------------------------------- ui

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    Terminal,
    Notes,
    Log,
    Editor,
    Diff,
    Files,
    Git,
    Mirror,
    Run,
}

/// What a pane shows; shape depends on `PaneKind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaneTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_id: Option<Id>,
}

/// A pane handle: the UI's id for it, stable for the pane's life.
pub type PaneRef = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaneInfo {
    pub pane: PaneRef,
    pub kind: PaneKind,
    pub target: PaneTarget,
    pub window_id: String,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowInfo {
    pub window_id: String,
    pub main: bool,
    pub panes: Vec<PaneRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    Agents,
    Code,
    Board,
    Modules,
    Plan,
    Notes,
    Dashboard,
    Skills,
    Plugins,
    Settings,
    Design,
}
