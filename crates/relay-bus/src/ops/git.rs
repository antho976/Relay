//! `worktree.*` / `git.*` / `integration.*` — BUS.md §10.12.
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::types::{Branch, Commit, DiffFile, FileStatus, Hunk, Id, Integration, Worktree};
use crate::{Empty, op};

payload!(#[schemars(rename = "GitProjectIn")] ProjectIn { pub project_id: Id });
payload!(#[schemars(rename = "WorktreeListIn")] WorktreeListIn { pub project_id: Id, pub include_dirty: Option<bool> });
result!(#[schemars(rename = "WorktreeListOut")] WorktreeListOut { pub worktrees: Vec<Worktree> });
op!(WorktreeList, "worktree.list", WorktreeListIn => WorktreeListOut, OpMeta::query(Scope::Project, 3, "Worktrees of a project"));
payload!(#[schemars(rename = "WorktreeCreateIn")] WorktreeCreateIn { pub project_id: Id, pub branch: String, pub from: Option<String> });
op!(WorktreeCreate, "worktree.create", WorktreeCreateIn => Worktree,
    OpMeta::mutation(Scope::Project, 3, "Create a worktree on a branch").actors(Actors::UserOnly).emits(&["worktree.changed"]));
payload!(#[schemars(rename = "WorktreeRemoveIn")] WorktreeRemoveIn {
    pub project_id: Id, pub path: String, pub purge_build: Option<bool>,
    /// Delete the worktree even with uncommitted changes or git-ignored files worth keeping.
    /// Without it, removing a dirty worktree is refused `worktree.dirty`, as `session.close` is.
    pub discard_changes: Option<bool>,
});
result!(#[schemars(rename = "GitFreedOut")] FreedOut {
    pub freed_mb: f64,
    /// The ref (`refs/relay/rescue/...`) now holding the commits of a detached HEAD that no
    /// branch or tag contained; absent when there were none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescued: Option<String>,
});
op!(WorktreeRemove, "worktree.remove", WorktreeRemoveIn => FreedOut,
    OpMeta::mutation(Scope::Project, 3, "Remove a worktree; conflict if a session owns it or it is dirty (unless discard_changes)").actors(Actors::UserOnly).emits(&["worktree.changed"]));
result!(#[schemars(rename = "WorktreeDiskOut")] WorktreeDiskOut { pub worktrees: Vec<crate::ops::app::WorktreeDisk> });
op!(WorktreeDisk, "worktree.disk", ProjectIn => WorktreeDiskOut, OpMeta::query(Scope::Project, 3, "Disk per worktree, build output separately"));

payload!(#[schemars(rename = "GitWtIn")] WtIn { pub project_id: Id, pub worktree: Option<String> });
result!(#[schemars(rename = "GitStatusOut")] StatusOut {
    pub branch: String, pub upstream: Option<String>, pub ahead: Option<i64>, pub behind: Option<i64>,
    /// A wholly untracked directory is one entry, its path ending in `/`. At most a few thousand
    /// entries: conflicts first, then other tracked changes, then untracked ones.
    pub files: Vec<FileStatus>,
    /// Whether `files` stops short of every change git reported.
    #[serde(default)]
    pub truncated: bool,
    /// How many entries git reported, `files` included; more than `files.len()` when truncated.
    #[serde(default)]
    pub total: u64,
});
op!(Status, "git.status", WtIn => StatusOut, OpMeta::query(Scope::Project, 8, "Working tree status (gix)"));
payload!(#[schemars(rename = "GitDiffIn")] DiffIn { pub project_id: Id, pub worktree: Option<String>, pub base: Option<String>, pub staged: Option<bool> });
result!(#[schemars(rename = "GitDiffOut")] DiffOut { pub files: Vec<DiffFile> });
op!(Diff, "git.diff", DiffIn => DiffOut, OpMeta::query(Scope::Project, 8, "Changed files with counts"));
payload!(#[schemars(rename = "GitDiffFileIn")] DiffFileIn {
    pub project_id: Id, pub worktree: Option<String>, pub path: String, pub base: Option<String>,
    /// Compare with the index, what a commit would record, instead of the working tree.
    pub staged: Option<bool>,
    /// For a rename, the file's path in the old side (`renamed_from` in `git.status`).
    pub old_path: Option<String>,
});
result!(#[schemars(rename = "GitDiffFileOut")] DiffFileOut { pub old: String, pub new: String, pub hunks: Vec<Hunk> });
op!(DiffFileOp, "git.diff.file", DiffFileIn => DiffFileOut, OpMeta::query(Scope::Project, 8, "One file's old/new text and hunks (for @codemirror/merge)"));
payload!(#[schemars(rename = "GitLogIn")] LogIn { pub project_id: Id, pub worktree: Option<String>, pub branch: Option<String>, pub limit: Option<u32> });
result!(#[schemars(rename = "GitLogOut")] LogOut { pub commits: Vec<Commit> });
op!(Log, "git.log", LogIn => LogOut, OpMeta::query(Scope::Project, 8, "History"));
payload!(#[schemars(rename = "GitShowIn")] ShowIn { pub project_id: Id, pub sha: String });
result!(#[schemars(rename = "GitShowOut")] ShowOut { pub commit: Commit, pub files: Vec<DiffFile> });
op!(Show, "git.show", ShowIn => ShowOut, OpMeta::query(Scope::Project, 8, "One commit and its files"));
result!(#[schemars(rename = "GitRemoteBranch")] RemoteBranch {
    /// `origin/feature`, as `git branch -r` prints it.
    pub name: String,
    pub remote: String,
    /// The branch name on the remote, `feature`.
    pub branch: String,
    pub head: String,
    /// The local branch of the same name, if there is one; switching to the remote selects it.
    pub local: Option<String>,
});
result!(#[schemars(rename = "GitBranchesOut")] BranchesOut {
    pub current: String,
    pub branches: Vec<Branch>,
    /// Remote-tracking branches (`refs/remotes/*`), without the symbolic `origin/HEAD`.
    #[serde(default)]
    pub remote_branches: Vec<RemoteBranch>,
});
op!(Branches, "git.branches", WtIn => BranchesOut, OpMeta::query(Scope::Project, 8, "Branches with merged flag and session owner for the selected worktree"));
payload!(#[schemars(rename = "GitBranchCreateIn")] BranchCreateIn {
    pub project_id: Id,
    pub worktree: Option<String>,
    pub name: String,
    pub start_point: Option<String>,
    pub checkout: Option<bool>,
});
result!(#[schemars(rename = "GitBranchCreateOut")] BranchCreateOut {
    pub name: String,
    pub head: String,
    pub worktree: String,
});
op!(BranchCreate, "git.branch.create", BranchCreateIn => BranchCreateOut,
    OpMeta::mutation(Scope::Project, 8, "Create a branch in a project worktree and optionally check it out").actors(Actors::UserOnly).emits(&["git.changed"]));
payload!(#[schemars(rename = "GitBranchSwitchIn")] BranchSwitchIn {
    pub project_id: Id, pub worktree: Option<String>,
    /// A local branch, or a remote-tracking one (`origin/feature`) to check out as a new local
    /// tracking branch.
    pub name: String,
    /// Bring uncommitted changes to tracked files along, as `git switch` does. Git still refuses
    /// when the target branch would overwrite them; nothing is ever discarded.
    pub carry_changes: Option<bool>,
});
result!(#[schemars(rename = "GitBranchSwitchOut")] BranchSwitchOut {
    /// The local branch now checked out.
    pub branch: String,
    /// Whether a local tracking branch was created for a remote branch.
    pub created: bool,
});
op!(BranchSwitch, "git.branch.switch", BranchSwitchIn => BranchSwitchOut,
    OpMeta::mutation(Scope::Project, 8, "Switch a checkout without a live session to a local or remote branch; uncommitted changes are carried or refused, never discarded").actors(Actors::UserOnly).emits(&["git.changed"]));
payload!(#[schemars(rename = "GitBranchDeleteIn")] BranchDeleteIn { pub project_id: Id, pub name: String });
op!(BranchDelete, "git.branch.delete", BranchDeleteIn => Empty,
    OpMeta::mutation(Scope::Project, 8, "Delete one merged local branch that is not checked out or owned by a session").actors(Actors::UserOnly).emits(&["git.changed"]));
payload!(#[schemars(rename = "GitPathsIn")] PathsIn { pub project_id: Id, pub worktree: Option<String>, pub paths: Vec<String> });
op!(Stage, "git.stage", PathsIn => Empty, OpMeta::mutation(Scope::Project, 8, "Stage paths").audit(Audit::AgentOnly));
op!(Unstage, "git.unstage", PathsIn => Empty, OpMeta::mutation(Scope::Project, 8, "Unstage paths").audit(Audit::AgentOnly));
payload!(#[schemars(rename = "GitCommitIn")] CommitIn { pub project_id: Id, pub worktree: Option<String>, pub message: String, pub all: Option<bool> });
result!(#[schemars(rename = "GitCommitOut")] CommitOut { pub sha: String });
op!(CommitOp, "git.commit", CommitIn => CommitOut,
    OpMeta::mutation(Scope::Project, 8, "Commit (caps and protected paths apply)").emits(&["git.changed"]));
result!(#[schemars(rename = "GitFetchOut")] FetchOut { pub ahead: Option<i64>, pub behind: Option<i64> });
op!(Fetch, "git.fetch", ProjectIn => FetchOut, OpMeta::mutation(Scope::Project, 8, "Fetch upstream").audit(Audit::AgentOnly).emits(&["git.changed"]));
payload!(#[schemars(rename = "GitPushIn")] PushIn { pub project_id: Id, pub worktree: Option<String>, pub set_upstream: Option<bool> });
op!(Push, "git.push", PushIn => Empty, OpMeta::mutation(Scope::Project, 8, "Push (system git)").emits(&["git.changed"]));
result!(#[schemars(rename = "GitPullRequest")] PullRequest {
    pub number: u64,
    pub branch: String,
    pub draft: bool,
    pub url: String,
    pub title: String,
    pub state: String,
    pub same_repository: bool,
});
result!(#[schemars(rename = "GitPrListOut")] PrListOut { pub pull_requests: Vec<PullRequest>, pub complete: bool });
payload!(#[schemars(rename = "GitPrListIn")] PrListIn {
    pub project_id: Id,
    /// Ask GitHub now instead of answering from the last listing (kept up to a minute, and
    /// dropped by `git.push` and `git.pr.open`). For an explicit refresh, not for every redraw.
    pub refresh: Option<bool>,
});
op!(PrList, "git.pr.list", PrListIn => PrListOut, OpMeta::query(Scope::Project, 8, "Open pull requests for the project, and the 100 most recently updated closed or merged ones; cached for a minute unless refresh, failures too"));
payload!(#[schemars(rename = "GitPrOpenIn")] PrOpenIn { pub project_id: Id, pub worktree: Option<String>, pub title: Option<String>, pub body: Option<String> });
result!(#[schemars(rename = "GitPrOpenOut")] PrOpenOut { pub url: String });
op!(PrOpen, "git.pr.open", PrOpenIn => PrOpenOut, OpMeta::mutation(Scope::Project, 8, "Open a PR for the branch"));
payload!(#[schemars(rename = "GitCleanMergedIn")] CleanMergedIn { pub project_id: Id, pub dry_run: Option<bool> });
result!(#[schemars(rename = "GitCleanMergedFailed")] CleanMergedFailed { pub branch: String, pub reason: String });
result!(#[schemars(rename = "GitCleanMergedOut")] CleanMergedOut {
    pub deleted: Vec<String>,
    /// Branches found merged that were not deleted (or, on a dry run, would not be), with why:
    /// a dirty or still-held checkout, a rebase in progress, a failed delete (RA-373).
    #[serde(default)]
    pub failed: Vec<CleanMergedFailed>,
});
op!(CleanMerged, "git.branch.clean_merged", CleanMergedIn => CleanMergedOut,
    OpMeta::mutation(Scope::Project, 8, "Alias of git.branch.cleanup that answers with the names of the branches deleted (or, on a dry run, that would be), and the merged ones it could not delete").actors(Actors::UserOnly).emits(&["git.changed", "worktree.changed"]));
payload!(#[schemars(rename = "GitBranchCleanupIn")] BranchCleanupIn { pub project_id: Id, pub dry_run: Option<bool> });
result!(#[schemars(rename = "GitBranchCleanup")] BranchCleanupRow {
    pub branch: String,
    /// The closed session that worked on it.
    pub session: Option<String>,
    /// `deleted`, `would_delete` (dry run) or `kept`.
    pub outcome: String,
    /// Why: how the work was found merged, or what unmerged work kept it.
    pub reason: String,
    pub pr: Option<u64>,
    pub removed_worktree: bool,
    pub deleted_remote: bool,
    /// Its work was found merged; a `kept` row with this set is one cleanup could not delete.
    #[serde(default)]
    pub merged: bool,
});
result!(#[schemars(rename = "GitBranchCleanupOut")] BranchCleanupOut { pub branches: Vec<BranchCleanupRow> });
op!(BranchCleanup, "git.branch.cleanup", BranchCleanupIn => BranchCleanupOut,
    OpMeta::mutation(Scope::Project, 8, "Remove the worktree and branch of every closed session whose work is merged (into the base branch, or by a merged GitHub PR); unmerged work is kept").actors(Actors::UserOnly).emits(&["git.changed", "worktree.changed"]));
result!(#[schemars(rename = "GitSuggestOut")] SuggestOut { pub message: String });
op!(SuggestMessage, "git.suggest_message", WtIn => SuggestOut, OpMeta::query(Scope::Project, 8, "Heuristic commit subject from the diff"));

payload!(#[schemars(rename = "IntegrationRequestIn")] IntegrationRequestIn {
    pub project_id: Id, pub sessions: Option<Vec<String>>, pub branches: Option<Vec<String>>,
    pub build: Option<bool>, pub deploy: Option<String>,
});
op!(IntegrationRequest, "integration.request", IntegrationRequestIn => Integration,
    OpMeta::mutation(Scope::Project, 8, "Octopus-merge N branches into a throwaway worktree, build, optionally deploy; results via events").emits(&["integration.changed", "integration.result", "notify.new"]));
payload!(#[schemars(rename = "IntegrationIdIn")] IntegrationIdIn { pub integration_id: Id });
op!(IntegrationGet, "integration.get", IntegrationIdIn => Integration, OpMeta::query(Scope::Project, 8, "One integration"));
result!(#[schemars(rename = "IntegrationListOut")] IntegrationListOut { pub integrations: Vec<Integration> });
op!(IntegrationList, "integration.list", ProjectIn => IntegrationListOut, OpMeta::query(Scope::Project, 8, "Integrations of a project"));
op!(IntegrationDiscard, "integration.discard", IntegrationIdIn => Empty,
    OpMeta::mutation(Scope::Project, 8, "Remove the throwaway worktree").actors(Actors::UserOnly).emits(&["integration.changed"]));

entries!(
    WorktreeList,
    WorktreeCreate,
    WorktreeRemove,
    WorktreeDisk,
    Status,
    Diff,
    DiffFileOp,
    Log,
    Show,
    Branches,
    BranchCreate,
    BranchSwitch,
    BranchDelete,
    Stage,
    Unstage,
    CommitOp,
    Fetch,
    Push,
    PrList,
    PrOpen,
    CleanMerged,
    BranchCleanup,
    SuggestMessage,
    IntegrationRequest,
    IntegrationGet,
    IntegrationList,
    IntegrationDiscard
);
