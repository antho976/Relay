//! `file.*` — BUS.md §10.13. Paths are worktree-relative; `..` and absolute are `file.path`.
use crate::op;
use crate::registry::{Actors, OpMeta, Scope, Undo};
use crate::types::{Entry, Id, Ts};

payload!(#[schemars(rename = "FileTreeIn")] TreeIn {
    pub project_id: Id, pub worktree: Option<String>, pub path: Option<String>, pub depth: Option<u32>, pub git_badges: Option<bool>,
    /// Most entries listed per directory, default 2000, at most 5000.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "FileTreeOut")] TreeOut {
    pub entries: Vec<Entry>,
    /// Directories cut short at `limit`, by worktree-relative path, with how many entries each
    /// really holds. Ask for one again with its `path` and a larger `limit` to see more.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub truncated: std::collections::BTreeMap<String, u32>,
});
op!(Tree, "file.tree", TreeIn => TreeOut, OpMeta::query(Scope::Project, 8, "Directory listing with git badges"));
payload!(#[schemars(rename = "FileReadIn")] ReadIn {
    pub project_id: Id, pub worktree: Option<String>, pub path: String, pub max_bytes: Option<u64>,
    /// The file as it was at this revision (a commit, a branch, `HEAD`), from git, in place of
    /// the one on disk: what a change's "before" side shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
});
result!(#[schemars(rename = "FileReadOut")] ReadOut { pub text: Option<String>, pub bytes_b64: Option<String>, pub mime: String, pub size: u64, pub truncated: bool });
op!(Read, "file.read", ReadIn => ReadOut, OpMeta::query(Scope::Project, 8, "Read a file (text or base64)"));
payload!(#[schemars(rename = "FileWriteIn")] WriteIn { pub project_id: Id, pub worktree: Option<String>, pub path: String, pub text: String, pub expected_sha256: Option<String> });
result!(#[schemars(rename = "FileWriteOut")] WriteOut { pub bytes: u64, pub removed_lines: i64, pub added_lines: i64 });
op!(Write, "file.write", WriteIn => WriteOut,
    OpMeta::mutation(Scope::Project, 8, "Write a file through the guardrails").emits(&["file.changed"]));
payload!(#[schemars(rename = "FileCreateIn")] CreateIn { pub project_id: Id, pub worktree: Option<String>, pub path: String, pub kind: String, pub text: Option<String> });
op!(Create, "file.create", CreateIn => Entry,
    OpMeta::mutation(Scope::Project, 8, "Create a file or directory").emits(&["file.changed"]));
payload!(#[schemars(rename = "FileRenameIn")] RenameIn { pub project_id: Id, pub worktree: Option<String>, pub path: String, pub new_name: String });
op!(Rename, "file.rename", RenameIn => Entry,
    OpMeta::mutation(Scope::Project, 8, "Rename in place").emits(&["file.changed"]));
payload!(#[schemars(rename = "FileMoveIn")] MoveIn { pub project_id: Id, pub worktree: Option<String>, pub path: String, pub into: String });
op!(Move, "file.move", MoveIn => Entry,
    OpMeta::mutation(Scope::Project, 8, "Move into a directory").emits(&["file.changed"]));
payload!(#[schemars(rename = "FilePathIn")] PathIn { pub project_id: Id, pub worktree: Option<String>, pub path: String });
result!(#[schemars(rename = "FileDeleteOut")] DeleteOut { pub trash_id: Id });
op!(Delete, "file.delete", PathIn => DeleteOut,
    OpMeta::mutation(Scope::Project, 8, "Soft-delete to .relay/trash").undo(Undo::Inverse).emits(&["file.changed"]));
payload!(#[schemars(rename = "FileRestoreIn")] RestoreIn {
    pub project_id: Id, pub trash_id: Id,
    /// The checkout to put it back into (`@project` or a worktree path). Default: the one it was
    /// deleted from, or the project's primary checkout once that one is gone.
    pub worktree: Option<String>,
});
result!(#[schemars(rename = "FileRestoreOut")] RestoreOut {
    #[serde(flatten)]
    pub entry: Entry,
    /// The checkout it was put back into.
    pub worktree: String,
    /// Set when no `worktree` was named and the one it was deleted from is gone, so it went to
    /// the project's primary checkout instead.
    pub fallback: bool,
});
op!(Restore, "file.restore", RestoreIn => RestoreOut,
    OpMeta::mutation(Scope::Project, 8, "Restore from trash").emits(&["file.changed"]));
payload!(#[schemars(rename = "FileTrashListIn")] TrashListIn {
    pub project_id: Id,
    /// Default 200, at most 1000.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "FileTrashEntry")] TrashEntry {
    /// The `trash_id` that `file.restore` takes.
    pub id: Id,
    pub original_path: String,
    /// The checkout it was deleted from, and where `file.restore` puts it back by default.
    pub worktree: String,
    pub created_at: Ts,
    /// Whether the trashed bytes are still on disk.
    pub available: bool,
});
result!(#[schemars(rename = "FileTrashListOut")] TrashListOut {
    /// Newest first.
    pub entries: Vec<TrashEntry>,
});
op!(TrashList, "file.trash.list", TrashListIn => TrashListOut,
    OpMeta::query(Scope::Project, 8, "Trashed files not yet restored or expired").actors(Actors::UserOnly));
op!(RestoreHead, "file.restore_head", PathIn => Entry,
    OpMeta::mutation(Scope::Project, 8, "git checkout HEAD -- <path>: put a tracked file back as HEAD has it, in the index and the worktree").actors(Actors::UserOnly).emits(&["file.changed"]));
payload!(#[schemars(rename = "ImportIn")] ImportIn { pub project_id: Id, pub worktree: Option<String>, pub into: String, pub sources: Vec<String> });
result!(#[schemars(rename = "ImportOut")] ImportOut { pub entries: Vec<Entry> });
op!(Import, "file.import", ImportIn => ImportOut,
    OpMeta::mutation(Scope::Project, 8, "Copy files in from the OS (drag-in)").actors(Actors::UserOnly).emits(&["file.changed"]));
payload!(#[schemars(rename = "FileSearchIn")] SearchIn {
    pub project_id: Id, pub worktree: Option<String>, pub query: String,
    /// A glob without `/` (`*.rs`) matches the file name at any depth; one with `/`, or a
    /// leading `/` to anchor it, matches the whole worktree-relative path.
    pub glob: Option<String>,
    pub regex: Option<bool>, pub limit: Option<u32>,
});
result!(#[schemars(rename = "Hit")] Hit {
    pub path: String, pub line: u32,
    /// 1-based byte offset of the match in the whole line.
    pub col: u32,
    /// The line, or for a long one a window of it around the match.
    pub text: String,
    /// Set when `text` is a window: the byte offset in the line where it starts, so the match
    /// begins `col - 1 - text_offset` bytes into `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_offset: Option<u32>,
});
result!(#[schemars(rename = "FileSearchOut")] SearchOut { pub hits: Vec<Hit> });
op!(Search, "file.search", SearchIn => SearchOut, OpMeta::query(Scope::Project, 8, "Content search"));

entries!(
    Tree,
    Read,
    Write,
    Create,
    Rename,
    Move,
    Delete,
    Restore,
    TrashList,
    RestoreHead,
    Import,
    Search
);
