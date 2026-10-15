//! `app.*` — BUS.md §10.2. Every mutation but `app.resources.watch`, which only turns sampling
//! on and off, is user-only (§9.1 layer 1).
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::types::{Id, ProviderInfo, Ts};
use crate::{op, Empty};
use std::collections::BTreeMap;

result!(#[schemars(rename = "BuildInfo")] BuildInfo { pub profile: String, pub git_sha: Option<String>, pub built_at: Option<String> });
result!(#[schemars(rename = "AppVersionOut")] VersionOut { pub version: String, pub instance: String, pub build: BuildInfo });
op!(Version, "app.version", Empty => VersionOut, OpMeta::query(Scope::Global, 1, "Version, instance and build info"));

// Gaming mode (`settings: performance.gaming`): while `active`, agents and what they run share
// `threads` CPU threads at the lowest CPU and disk priority. `supported` is false without a
// systemd user manager; `game` is what `auto` found running. `gaming.changed` carries the same.
result!(#[schemars(rename = "GamingStatus")] GamingStatus {
    pub supported: bool, pub on: bool, pub auto: bool, pub threads: u32, pub active: bool, pub game: Option<String>,
});
result!(#[schemars(rename = "AppStatusOut")] StatusOut {
    pub pid: u32, pub uptime_s: u64, pub store_path: String, pub socket_path: String,
    pub sessions_live: i64, pub providers: Vec<ProviderInfo>, pub gaming: GamingStatus,
});
op!(Status, "app.status", Empty => StatusOut, OpMeta::query(Scope::Global, 1, "Engine status: paths, live sessions, providers, gaming mode"));

payload!(#[schemars(rename = "AppQuitIn")] QuitIn { pub force: Option<bool> });
op!(Quit, "app.quit", QuitIn => Empty,
    OpMeta::mutation(Scope::Global, 1, "Quit the engine; conflict app.sessions_live unless force").actors(Actors::UserOnly));

result!(#[schemars(rename = "PaneResource")] PaneResource { pub session: String, pub pid: Option<i64>, pub rss_mb: f64, pub cpu_pct: f64 });
result!(#[schemars(rename = "RelayResource")] RelayResource { pub pid: i64, pub rss_mb: f64, pub cpu_pct: f64 });
result!(#[schemars(rename = "WorktreeDisk")] WorktreeDisk { pub path: String, pub disk_mb: f64, pub build_mb: Option<f64> });
result!(#[schemars(rename = "ResourcesOut")] ResourcesOut { pub relay: RelayResource, pub panes: Vec<PaneResource>, pub worktrees: Vec<WorktreeDisk>, pub store_mb: f64, pub total_rss_mb: f64 });
op!(ResourcesGet, "app.resources.get", Empty => ResourcesOut, OpMeta::query(Scope::Global, 9, "Per-pane RAM/CPU, per-worktree disk, store size"));

payload!(#[schemars(rename = "ResourcesWatchIn")] ResourcesWatchIn { pub on: bool });
op!(ResourcesWatch, "app.resources.watch", ResourcesWatchIn => Empty,
    OpMeta::mutation(Scope::Global, 9, "While any client watches, resource.sample events flow; otherwise none").audit(Audit::Never).emits(&["resource.sample"]));

result!(#[schemars(rename = "RecoveryReport")] RecoveryReport {
    pub at: Ts, pub reaped_pids: Vec<i64>, pub fsck_fixes: Vec<String>,
    pub dirty_worktrees: Vec<String>, pub tasks_reset_offered: Vec<Id>,
});
op!(RecoveryLast, "app.recovery.last", Empty => Option<RecoveryReport>, OpMeta::query(Scope::Global, 3, "What crash recovery did at the last launch"));

payload!(#[schemars(rename = "AppLogTailIn")] LogTailIn { pub level: Option<String>, pub filter: Option<String> });
op!(LogTail, "app.log.tail", LogTailIn => Empty, OpMeta::query(Scope::Global, 9, "Not built: answers bus.not_implemented and attaches no stream; the engine logs to relay serve's output (RELAY_LOG filters it)").stream("log"));

result!(#[schemars(rename = "BackupOut")] BackupOut { pub path: String, pub bytes: u64 });
op!(BackupNow, "app.backup.now", Empty => BackupOut, OpMeta::mutation(Scope::Global, 2, "Copy store.db to the backup dir now; keep last 5").actors(Actors::UserOnly));

result!(#[schemars(rename = "BackupInfo")] BackupInfo { pub path: String, pub bytes: u64, pub created_at: Ts, pub reason: String });
result!(#[schemars(rename = "BackupListOut")] BackupListOut { pub backups: Vec<BackupInfo> });
op!(BackupList, "app.backup.list", Empty => BackupListOut, OpMeta::query(Scope::Global, 2, "List backups"));

payload!(#[schemars(rename = "ImportV3In")] ImportV3In { pub source: String, pub project_id: Id, pub dry_run: Option<bool> });
result!(#[schemars(rename = "ImportCounts")] ImportCounts { pub tasks: i64, pub modules: i64, pub notes: i64, pub sessions: i64 });
result!(#[schemars(rename = "ImportIdMap")] ImportIdMap { pub tasks: BTreeMap<String, Id>, pub modules: BTreeMap<String, Id>, pub notes: BTreeMap<String, Id> });
result!(#[schemars(rename = "ImportV3Out")] ImportV3Out { pub counts: ImportCounts, pub id_map: ImportIdMap, pub warnings: Vec<String> });
op!(ImportV3, "app.import.v3", ImportV3In => ImportV3Out,
    OpMeta::mutation(Scope::Global, 2, "One-time import of a v3 .relay/relay.db into a project").actors(Actors::UserOnly).emits(&["task.changed", "module.changed", "notes.changed"]));

result!(#[schemars(rename = "AppFirstRunOut")] FirstRunOut { pub needed: bool, pub steps: BTreeMap<String, String> });
op!(FirstRunState, "app.first_run.state", Empty => FirstRunOut, OpMeta::query(Scope::Global, 11, "First-run wizard state: workspace / providers / project / import"));

result!(#[schemars(rename = "AppReconcileOut")] ReconcileOut { pub actions: Vec<String> });
op!(Reconcile, "app.reconcile", Empty => ReconcileOut,
    OpMeta::mutation(Scope::Global, 1, "Run the trust-but-verify pass now: retention of soft-deleted rows, notifications and mail").actors(Actors::UserOnly).emits(&["notify.changed"]));

entries!(Version, Status, Quit, ResourcesGet, ResourcesWatch, RecoveryLast, LogTail, BackupNow, BackupList, ImportV3, FirstRunState, Reconcile);
