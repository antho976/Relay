//! `session.*` — BUS.md §10.8. Lifecycle ops are user-only; agents get done/report/attach/brief/peers/get/list.
use crate::registry::{Actors, Audit, OpMeta, Scope, Undo};
use crate::types::{GuardrailCaps, Id, Peer, Provider, Role, Session, SessionState, Ts};
use crate::{op, Empty};
use serde_json::Value;

payload!(#[schemars(rename = "SessionCreateIn")] CreateIn {
    pub project_id: Id, pub provider: Provider, pub role: Option<Role>, pub model: Option<String>, pub effort: Option<String>,
    pub branch: Option<String>,
    /// `"new"` (default: pooled worktree), `"primary"`, or an absolute path of an existing worktree.
    pub worktree: Option<String>,
    pub task_id: Option<Id>, pub module_id: Option<Id>, pub pair_with: Option<String>,
    pub bus_writes: Option<bool>, pub allow_ui: Option<bool>,
    /// Persist the initial assignment before spawning, so a partial launch can recover it.
    pub prompt: Option<String>,
});
op!(Create, "session.create", CreateIn => Session,
    OpMeta::mutation(Scope::Project, 3, "Allocate a session (name, worktree, token); spawns nothing").actors(Actors::UserOnly).emits(&["session.changed"]));

payload!(#[schemars(rename = "SessionSpawnIn")] SpawnIn {
    pub session: String,
    /// Omitted preserves the allocated assignment; an empty string explicitly clears it.
    pub prompt: Option<String>,
});
op!(Spawn, "session.spawn", SpawnIn => Session,
    OpMeta::mutation(Scope::Session, 3, "Start the provider CLI in the worktree with the brief").actors(Actors::UserOnly).emits(&["session.changed"]));
payload!(#[schemars(rename = "SessionNameIn")] NameIn { pub session: String });
op!(Resume, "session.resume", NameIn => Session,
    OpMeta::mutation(Scope::Session, 6, "Provider-resume a restorable session").actors(Actors::UserOnly).emits(&["session.changed"]));
op!(ClearRestorable, "session.clear_restorable", NameIn => Session,
    OpMeta::mutation(Scope::Session, 6, "Fresh-spawn a restorable session without its saved provider context").actors(Actors::UserOnly).emits(&["session.changed"]));
op!(Park, "session.park", NameIn => Session,
    OpMeta::mutation(Scope::Session, 6, "Kill the CLI, keep pane/scrollback/worktree/token").actors(Actors::UserOnly).emits(&["session.changed"]));
op!(Wake, "session.wake", NameIn => Session,
    OpMeta::mutation(Scope::Session, 6, "Respawn a parked session with provider resume").actors(Actors::UserOnly).emits(&["session.changed"]));
payload!(#[schemars(rename = "SessionCloseIn")] CloseIn {
    pub session: String, pub remove_worktree: Option<bool>, pub purge_build: Option<bool>,
    /// Delete the worktree even with uncommitted changes (tracked edits or untracked files).
    /// Without it, removing a dirty Relay-pool worktree is refused `worktree.dirty` (RA-405).
    pub discard_changes: Option<bool>,
});
result!(#[schemars(rename = "SessionCloseOut")] CloseOut { pub freed_mb: f64 });
op!(Close, "session.close", CloseIn => CloseOut,
    OpMeta::mutation(Scope::Session, 3, "Kill, purge build output, remove worktree (refused if dirty unless discard_changes); branch kept").actors(Actors::UserOnly).emits(&["session.changed", "worktree.changed"]));
payload!(#[schemars(rename = "SessionDoneIn")] DoneIn {
    pub session: String, pub summary: Option<String>, pub sha: Option<String>,
    /// `completed` (default), `blocked`, or `partial`. An op that can only report success
    /// gets reported success (D118).
    pub status: Option<String>,
    /// What stopped it, for `blocked` and `partial`.
    pub blockers: Option<Vec<String>>,
});
op!(Done, "session.done", DoneIn => Session,
    OpMeta::mutation(Scope::Session, 5, "Record completion; all builders finish before review, and all reviewers finish before the group advances. Blocked/partial retain the assignment.").actors(Actors::AgentOnly).emits(&["session.changed", "task.changed", "notify.new", "notify.changed", "mailbox.new"]));
payload!(#[schemars(rename = "SessionIntentIn")] IntentIn {
    pub session: String,
    /// One line, present tense. Empty clears it.
    pub text: String,
});
op!(Intent, "session.intent", IntentIn => Session,
    OpMeta::mutation(Scope::Session, 5, "Declare in one line what this session is doing; peers read it in session.peers").actors(Actors::AgentOnly).emits(&["session.changed"]));
payload!(#[schemars(rename = "SessionClaimIn")] ClaimIn {
    /// Worktree-relative paths this session is taking on.
    pub paths: Vec<String>,
    /// Narrow the claim to one symbol within each path.
    pub symbol: Option<String>,
    pub note: Option<String>,
    /// Refuse the whole claim if any path is already claimed by a peer, instead of recording
    /// it alongside theirs and reporting the collision.
    pub exclusive: Option<bool>,
});
result!(#[schemars(rename = "SessionClaimCollision")] Collision {
    pub path: String, pub symbol: String, pub session: String, pub since: Ts,
});
result!(#[schemars(rename = "SessionClaimOut")] ClaimOut {
    pub claimed: Vec<String>,
    /// Peers already holding one of these paths. Empty is the quiet, expected case.
    pub collisions: Vec<Collision>,
});
op!(Claim, "session.claim", ClaimIn => ClaimOut,
    OpMeta::mutation(Scope::Session, 5, "Declare the files this session is working on and learn who else holds them").actors(Actors::AgentOnly).emits(&["overlap.changed"]));
payload!(#[schemars(rename = "SessionReleaseIn")] ReleaseIn {
    /// Omit to release every claim this session holds.
    pub paths: Option<Vec<String>>,
});
result!(#[schemars(rename = "SessionReleaseOut")] ReleaseOut { pub released: u32 });
op!(Release, "session.release", ReleaseIn => ReleaseOut,
    OpMeta::mutation(Scope::Session, 5, "Drop claims this session no longer holds").actors(Actors::AgentOnly).emits(&["overlap.changed"]));
op!(Get, "session.get", NameIn => Session, OpMeta::query(Scope::Session, 3, "One session"));
payload!(#[schemars(rename = "SessionListIn")] ListIn { pub project_id: Option<Id>, pub state: Option<Vec<SessionState>>, pub include_closed: Option<bool> });
result!(#[schemars(rename = "SessionListOut")] ListOut { pub sessions: Vec<Session> });
op!(List, "session.list", ListIn => ListOut, OpMeta::query(Scope::Global, 3, "Sessions, filtered; agents see their own project only"));
payload!(#[schemars(rename = "SessionPeersIn")] PeersIn { pub session: Option<String>, pub project_id: Option<Id> });
result!(#[schemars(rename = "SessionPeersOut")] PeersOut { pub peers: Vec<Peer> });
op!(Peers, "session.peers", PeersIn => PeersOut, OpMeta::query(Scope::Project, 5, "The live peer table"));
result!(#[schemars(rename = "SessionBriefParts")] BriefParts { pub state: String, pub peers: String, pub adjacent: String, pub skills: String, #[serde(default)] pub plugins: String });
result!(#[schemars(rename = "SessionBriefOut")] BriefOut {
    /// Everything, skills included. What the UI shows.
    pub text: String,
    /// State, peers and adjacent work only — the part small enough to inject on every
    /// spawn. Skill bodies are referenced by path instead of inlined (D101).
    pub compact: String,
    pub parts: BriefParts,
});
op!(Brief, "session.brief", NameIn => BriefOut, OpMeta::query(Scope::Session, 5, "The knowledge injection, inspectable"));
result!(#[schemars(rename = "SessionBootstrapTask")] BootstrapTask {
    pub id: Id, pub title: String, pub body: String, pub changelog: String,
    pub column: String, pub state: String,
});
result!(#[schemars(rename = "SessionBootstrapPair")] BootstrapPair {
    pub session: String, pub provider: Provider, pub role: Role,
    pub branch: String, pub state: SessionState,
});
result!(#[schemars(rename = "SessionBootstrapGuardrails")] BootstrapGuardrails {
    pub caps: GuardrailCaps,
    pub denied_commands: Vec<String>,
    /// Absolute roots this session may write to, worktree first.
    pub write_roots: Vec<String>,
    /// The op that answers "would this be allowed?" without doing it.
    pub dry_run: String,
    /// What to do when a refusal stands between you and the task: how to ask the user for an
    /// exception and wait for the answer.
    pub exceptions: String,
});
result!(#[schemars(rename = "SessionBootstrapOut")] BootstrapOut {
    pub session: String, pub role: Role, pub project_id: Id, pub project: String, pub base_branch: String,
    pub worktree: String, pub branch: String, pub module: Option<String>,
    /// Current task retained for older clients and one-task agents.
    pub task: Option<BootstrapTask>,
    /// Current task followed by every queued or in-review task attached to this session.
    pub tasks: Vec<BootstrapTask>,
    pub pair: Option<BootstrapPair>,
    /// The launch prompt, on the session's first launch only: a relaunch may come long after
    /// that work was done (RA-397). The brief keeps it as the original launch assignment.
    pub assignment: Option<String>,
    /// Worktree-relative path of the full brief.
    pub brief_path: String,
    /// Everyone else live in this project, now.
    pub peers: Vec<Peer>,
    /// Every op this session may really call, all three gating layers applied.
    pub can_call: Vec<String>,
    /// How to inspect operation summaries and exact payload/result schemas without guessing.
    pub discovery: String,
    /// How to reach the peers above, and which channel actually reaches all of them.
    pub comms: String,
    pub guardrails: BootstrapGuardrails,
});
op!(Bootstrap, "session.bootstrap", Empty => BootstrapOut,
    OpMeta::query(Scope::Session, 12, "Who am I, who else is here, and what may I call: the one call every agent makes first").actors(Actors::AgentOnly));
payload!(#[schemars(rename = "SessionUpdateIn")] UpdateIn {
    pub session: String, pub branch: Option<String>, pub model: Option<String>, pub effort: Option<String>,
    #[serde(default, deserialize_with = "crate::nullable")] pub task_id: Option<Option<Id>>, #[serde(default, deserialize_with = "crate::nullable")] pub module_id: Option<Option<Id>>, pub bus_writes: Option<bool>, pub allow_ui: Option<bool>,
});
op!(Update, "session.update", UpdateIn => Session,
    OpMeta::mutation(Scope::Session, 6, "Patch a session; branch/model/effort conflict after first spawn").actors(Actors::UserOnly).undo(Undo::Inverse).emits(&["session.changed"]));
payload!(#[schemars(rename = "SessionReportIn")] ReportIn { pub session: String, pub kind: String, pub data: Option<Value> });
op!(Report, "session.report", ReportIn => Empty,
    OpMeta::mutation(Scope::Session, 5, "Provider hooks calling home: session_start | tool_use | stop | notification | idle | blocked").audit(Audit::AgentOnly).actors(Actors::AgentOnly).emits(&["session.changed", "notify.new"]));
payload!(#[schemars(rename = "SessionAttachIn")] AttachIn { pub session: String, pub from_seq: Option<u64>, pub epoch: Option<u64> });
op!(Attach, "session.attach", AttachIn => Empty, OpMeta::query(Scope::Session, 3, "Attach the pty stream, replaying from (epoch, seq)").stream("pty"));
op!(Detach, "session.detach", NameIn => Empty, OpMeta::query(Scope::Session, 3, "Detach the pty stream"));
payload!(#[schemars(rename = "SessionInputIn")] InputIn { pub session: String, pub data: String });
op!(Input, "session.input", InputIn => Empty,
    OpMeta::mutation(Scope::Session, 3, "Text to the PTY").audit(Audit::AgentOnly).actors(Actors::UserOnly).emits(&["session.changed"]));
payload!(#[schemars(rename = "SessionResizeIn")] ResizeIn {
    pub session: String, pub cols: u16, pub rows: u16,
    /// Borrow the size: on a connection door, the PTY goes back to the size it had when this
    /// connection detaches from the session or goes away, unless someone resized it since.
    pub until_detach: Option<bool>,
});
op!(Resize, "session.resize", ResizeIn => Empty,
    OpMeta::mutation(Scope::Session, 3, "Resize the PTY, for good or until this connection detaches").audit(Audit::AgentOnly).actors(Actors::UserOnly));
payload!(#[schemars(rename = "SessionScrollbackIn")] ScrollbackIn { pub session: String, pub lines: Option<u32> });
result!(#[schemars(rename = "SessionScrollbackOut")] ScrollbackOut {
    pub text: String, pub epoch: u64, pub seq: u64,
    /// The live PTY's size; absent when the text comes from a saved scrollback.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rows: Option<u16>,
});
op!(Scrollback, "session.scrollback", ScrollbackIn => ScrollbackOut, OpMeta::query(Scope::Session, 3, "Scrollback text, the (epoch, seq) it ends at, and the PTY's size"));
payload!(#[schemars(rename = "SessionTranscriptIn")] TranscriptIn {
    pub session: String,
    /// The `cursor` of the previous answer: only what the agent wrote since. Absent, the
    /// conversation's last `limit` messages.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub after: Option<u64>,
    /// At most this many messages, the newest kept (default 200, at most 1000).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub limit: Option<u32>,
});
result!(#[schemars(rename = "SessionTranscriptMessage")] TranscriptMessage {
    /// Stable across reads: the provider's own id for the record, or its byte offset.
    pub id: String,
    /// `user`, `assistant` (`blocks` of `text` and `tool_use {id, name, summary}`), or `tool`
    /// (`{tool_use_id, is_error, text}`) — a thread message's shape (`thread.get`).
    pub role: String,
    pub body: Value,
    pub created_at: String,
});
result!(#[schemars(rename = "SessionTranscriptOut")] TranscriptOut {
    pub messages: Vec<TranscriptMessage>,
    /// Pass it back as `after` for what comes next.
    pub cursor: u64,
    /// The messages replace everything read before: the conversation moved to another file
    /// (a resume, a new conversation) or `after` was past its end.
    pub reset: bool,
    /// Whether the provider's conversation was found; false before its first hook names it.
    pub found: bool,
});
op!(Transcript, "session.transcript", TranscriptIn => TranscriptOut, OpMeta::query(Scope::Session, 3, "The agent's conversation as messages, read from the provider's own log, from a cursor"));
payload!(#[schemars(rename = "SessionUploadIn")] UploadIn {
    pub session: String,
    /// A PNG, JPEG, GIF or WebP picture, base64 (at most 4 MiB decoded): a photo or a
    /// screenshot from the phone for the agent to look at.
    pub bytes_b64: String,
});
result!(#[schemars(rename = "SessionUploadOut")] UploadOut {
    /// Where it was written, from the session's checkout: `.relay/attachments/<name>`, which
    /// git never sees. A message naming this path shows the agent the picture.
    pub path: String,
    /// The same, absolute.
    pub absolute: String,
    /// Its size in px.
    pub w: u32,
    pub h: u32,
});
op!(Upload, "session.upload", UploadIn => UploadOut,
    OpMeta::mutation(Scope::Session, 3, "A picture for the agent, into its checkout's .relay/attachments (out of git); returns the path to name in a message")
        .audit(Audit::AgentOnly).actors(Actors::UserOnly));
result!(#[schemars(rename = "SessionRestorable")] Restorable { pub session: Session, pub reason: String, pub worktree_dirty: bool });
result!(#[schemars(rename = "SessionRestorableOut")] RestorableOut { pub sessions: Vec<Restorable> });
payload!(#[schemars(rename = "SessionRestorableIn")] RestorableIn {
    /// Only this project's sessions. Each one costs a `git status` of its worktree, so a
    /// client describing one project's panes should not pay for every project's (RA-235).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub project_id: Option<Id>,
    /// Only this session.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub session: Option<String>,
});
op!(RestorableList, "session.restorable", RestorableIn => RestorableOut, OpMeta::query(Scope::Global, 6, "Sessions offering resume at launch, optionally one project's or one session's"));
payload!(#[schemars(rename = "SessionDiscardRestorableIn")] DiscardRestorableIn {
    pub session: String,
    /// Delete the worktree even with uncommitted changes. Without it, a dirty Relay-pool
    /// worktree that the discard would delete is refused `worktree.dirty` (RA-405).
    pub discard_changes: Option<bool>,
});
op!(DiscardRestorable, "session.discard_restorable", DiscardRestorableIn => Empty,
    OpMeta::mutation(Scope::Session, 6, "Decline resume: clean the session").actors(Actors::UserOnly).emits(&["session.changed"]));

// Linked repositories: another repository cloned into one session's checkout, at
// `.relay/linked/<name>`, for as long as that session needs it. It goes with the checkout.
result!(#[schemars(rename = "LinkedRepoUnpushed")] LinkedRepoUnpushed {
    /// Entries `git status` lists, untracked files included; `None` when the scan failed.
    pub changed: Option<i64>,
    /// Commits on a branch (or a detached HEAD) that no remote-tracking ref contains, as of the
    /// clone's last fetch; `None` when git could not tell.
    pub commits: Option<i64>,
    /// The local branches holding those commits.
    pub branches: Vec<String>,
    /// Entries in `git stash list`.
    pub stashes: i64,
    /// Git-ignored files nothing could bring back: an `.env`, a keystore, a local database.
    pub ignored: Vec<String>,
});
result!(#[schemars(rename = "LinkedRepo")] LinkedRepo {
    /// The folder under `.relay/linked/`, which names it to `session.repo.remove`.
    pub name: String,
    /// Absolute path of the clone.
    pub path: String,
    /// Its `origin` remote; absent when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub url: Option<String>,
    /// The branch checked out; absent on a detached HEAD.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub branch: Option<String>,
    /// What removing it would lose; absent when nothing would be. `session.repo.add` does not
    /// look: it has just cloned it.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub unpushed: Option<LinkedRepoUnpushed>,
});
payload!(#[schemars(rename = "SessionRepoAddIn")] RepoAddIn {
    pub session: String,
    /// Anything `git clone` takes: an https or ssh URL, or `owner/name` for a GitHub repository.
    pub url: String,
    /// The folder name under `.relay/linked/`; the repository's name when omitted.
    pub name: Option<String>,
    /// Check out this branch instead of the remote's default.
    pub branch: Option<String>,
});
op!(RepoAdd, "session.repo.add", RepoAddIn => LinkedRepo,
    OpMeta::mutation(Scope::Session, 6, "Clone another repository into this session's checkout (.relay/linked/<name>) for as long as the session needs it").emits(&["session.repo.changed"]));
result!(#[schemars(rename = "SessionRepoListOut")] RepoListOut { pub repos: Vec<LinkedRepo> });
op!(RepoList, "session.repo.list", NameIn => RepoListOut,
    OpMeta::query(Scope::Session, 6, "A session's linked repositories, each with what removing it would lose"));
payload!(#[schemars(rename = "SessionRepoRemoveIn")] RepoRemoveIn {
    pub session: String,
    pub name: String,
    /// Delete it even with uncommitted changes, unpushed commits, stashes or ignored keepers.
    /// Without it, such a repository is refused `session.repo_unpushed` and kept.
    pub discard_changes: Option<bool>,
});
result!(#[schemars(rename = "SessionRepoRemoveOut")] RepoRemoveOut { pub freed_mb: f64 });
op!(RepoRemove, "session.repo.remove", RepoRemoveIn => RepoRemoveOut,
    OpMeta::mutation(Scope::Session, 6, "Delete a linked repository (refused if it holds unpushed work unless discard_changes)").actors(Actors::UserOnly).emits(&["session.repo.changed"]));

entries!(
    Create,
    Spawn,
    Resume,
    ClearRestorable,
    Park,
    Wake,
    Close,
    Done,
    Intent,
    Claim,
    Release,
    Get,
    List,
    Peers,
    Brief,
    Bootstrap,
    Update,
    Report,
    Attach,
    Detach,
    Input,
    Resize,
    Scrollback,
    Transcript,
    Upload,
    RestorableList,
    DiscardRestorable,
    RepoAdd,
    RepoList,
    RepoRemove
);
