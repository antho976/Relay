//! `task.*` — BUS.md §10.5.
use crate::registry::{Actors, OpMeta, Scope, Undo};
use crate::types::{
    Attachment, AuditRow, Column, Id, Label, Message, Priority, Session, Size, Task, TaskComment,
    TaskRelation, TaskState, TaskType, Ts,
};
use crate::{op, Empty};

payload!(#[schemars(rename = "TaskAttachmentIn")] AttachmentIn { pub name: String, pub mime: String, pub bytes_b64: String });

payload!(#[schemars(rename = "TaskCreateIn")] CreateIn {
    pub project_id: Id, pub title: String, pub body: Option<String>, pub column: Option<Column>,
    pub state: Option<TaskState>, pub priority: Option<Priority>, pub size: Option<Size>,
    pub module_id: Option<Id>, pub changelog: Option<String>, pub attachments: Option<Vec<AttachmentIn>>,
    #[serde(rename = "type")] #[schemars(rename = "type")] pub task_type: Option<TaskType>,
    /// Create it straight as a sub-task of this one; refused past `TASK_DEPTH_MAX`.
    pub parent_id: Option<Id>,
    /// Free-form tags; unknown names are created in the project's label set.
    pub labels: Option<Vec<String>>,
    /// The note this task is made from: a live note in the same project, or `notes.not_found`.
    pub note_id: Option<Id>,
});
op!(Create, "task.create", CreateIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Create a task; returns it").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskGetIn")] GetIn { pub task_id: Id });
op!(Get, "task.get", GetIn => Task, OpMeta::query(Scope::Project, 7, "One task with attachments and commits"));

payload!(#[schemars(rename = "TaskActivityIn")] ActivityIn {
    pub task_id: Id, pub before_audit: Option<Id>, pub before_message: Option<Id>, pub limit: Option<u32>,
});
result!(#[schemars(rename = "TaskActivityOut")] ActivityOut {
    pub history: Vec<AuditRow>, pub messages: Vec<Message>,
    pub next_audit: Option<Id>, pub next_message: Option<Id>,
    /// Every comment on the task, oldest first; not paged. `task.comment` rows stay out of
    /// `history`, so a comment is reported once.
    #[serde(default)]
    pub comments: Vec<TaskComment>,
});
op!(Activity, "task.activity", ActivityIn => ActivityOut,
    OpMeta::query(Scope::Project, 7, "Task-scoped audit history and explicitly associated messages, newest first").actors(Actors::UserOnly));

payload!(#[schemars(rename = "TaskCommentIn")] CommentIn { pub task_id: Id, pub body: String });
op!(Comment, "task.comment", CommentIn => TaskComment,
    OpMeta::mutation(Scope::Project, 7, "Comment on a task, GitHub-issue style; task.activity lists every comment oldest first"));

payload!(#[schemars(rename = "TaskListIn")] ListIn {
    pub project_id: Option<Id>, pub column: Option<Column>, pub state: Option<TaskState>, pub module_id: Option<Id>,
    pub priority: Option<Priority>, pub include_deleted: Option<bool>, pub sort: Option<String>,
    #[serde(rename = "type")] #[schemars(rename = "type")] pub task_type: Option<TaskType>,
    /// One label name, exact and case-insensitive.
    pub label: Option<String>,
    /// A session name; matches every task that session was ever dispatched to.
    pub session: Option<String>,
    /// Only the tasks made from this note.
    pub note_id: Option<Id>,
    /// `Some(Some(id))` = children of that task, `Some(None)` = roots only, absent = every task.
    #[serde(default, deserialize_with = "crate::nullable")] pub parent_id: Option<Option<Id>>,
    /// Page size, default 1000, at most 2000. Done cards sort last, so a page cut short drops
    /// the oldest finished work first.
    pub limit: Option<u32>,
    /// Skip this many tasks of the ordered result: the previous page's `next_offset`.
    pub offset: Option<u32>,
    /// Leave each task's `body` and `changelog` empty: a board needs titles, not essays.
    pub summary: Option<bool>,
});
result!(#[schemars(rename = "TaskListOut")] ListOut {
    pub tasks: Vec<Task>,
    /// Set when more tasks match than this page holds: pass it as `offset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<u32>,
});
op!(List, "task.list", ListIn => ListOut, OpMeta::query(Scope::Global, 7, "Tasks, filtered; project_id optional for cross-project views"));

payload!(#[schemars(rename = "TaskUpdateIn")] UpdateIn {
    pub task_id: Id, pub title: Option<String>, pub body: Option<String>, pub priority: Option<Priority>,
    #[serde(default, deserialize_with = "crate::nullable")] pub size: Option<Option<Size>>, #[serde(default, deserialize_with = "crate::nullable")] pub module_id: Option<Option<Id>>, pub state: Option<TaskState>, pub changelog: Option<String>,
    #[serde(rename = "type")] #[schemars(rename = "type")] pub task_type: Option<TaskType>,
    /// Original editable field values; every supplied value must still match atomically.
    pub expected: Option<serde_json::Map<String, serde_json::Value>>,
});
op!(Update, "task.update", UpdateIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Patch a task (agents: own task, body/changelog only)").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskMoveIn")] MoveIn { pub task_id: Id, pub column: Column, pub position: Option<i64> });
op!(Move, "task.move", MoveIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Move a task to a column (agents: own task, active→in_review only)").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskIdIn")] IdIn { pub task_id: Id });
op!(Delete, "task.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Project, 7, "Soft-delete a task").undo(Undo::Inverse).emits(&["task.deleted"]));
op!(Restore, "task.restore", IdIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Restore a soft-deleted task").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskLinkCommitIn")] LinkCommitIn { pub task_id: Id, pub sha: String, pub branch: Option<String> });
op!(LinkCommit, "task.link_commit", LinkCommitIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Link a commit sha to a task").emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskChangelogWriteIn")] ChangelogWriteIn {
    pub task_id: Id, pub text: String,
    /// The task's `updated_at` as you last read it. When given and the task has changed since,
    /// the write is refused (`task.edit_conflict`) instead of overwriting someone else's edit;
    /// omitted, the write goes through as before.
    pub expected_updated_at: Option<Ts>,
});
op!(ChangelogWrite, "task.changelog.write", ChangelogWriteIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Write the sentence that ships in patch notes; pass expected_updated_at (from your last read) to refuse the write if the task changed since").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskAttachIn")] AttachIn { pub task_id: Id, pub name: Option<String>, pub mime: Option<String>, pub bytes_b64: Option<String>, pub path: Option<String> });
op!(Attach, "task.attach", AttachIn => Attachment,
    OpMeta::mutation(Scope::Project, 7, "Attach an image by bytes or by path").emits(&["task.changed"]));
payload!(#[schemars(rename = "TaskDetachIn")] DetachIn { pub task_id: Id, pub attachment_id: Id });
op!(Detach, "task.detach", DetachIn => Empty,
    OpMeta::mutation(Scope::Project, 7, "Remove an attachment (soft: task.attachment.restore brings it back)").undo(Undo::Inverse).emits(&["task.changed"]));
op!(AttachmentRestore, "task.attachment.restore", DetachIn => Attachment,
    OpMeta::mutation(Scope::Project, 7, "Restore a detached attachment with its id and file").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskDispatchIn")] DispatchIn {
    pub task_id: Id, pub session: Option<String>, pub create: Option<crate::ops::session::CreateIn>,
    /// Also dispatch every not-done descendant, one fresh session each. Requires `create`.
    pub fanout: Option<bool>,
    /// Defaults to true. False stages the assignment without starting the provider, so a
    /// multi-task launch can attach its complete queue before the first agent turn.
    pub start: Option<bool>,
});
result!(#[schemars(rename = "TaskDispatched")] Dispatched { pub task: Task, pub session: Session });
result!(#[schemars(rename = "TaskDispatchOut")] DispatchOut { pub task: Task, pub session: Session, #[serde(default)] pub fanned: Vec<Dispatched> });
op!(Dispatch, "task.dispatch", DispatchIn => DispatchOut,
    OpMeta::mutation(Scope::Project, 7, "Send or stage a task; fanout sends its sub-tasks too").actors(Actors::UserOnly).emits(&["task.changed", "session.changed"]));

payload!(#[schemars(rename = "TaskApproveIn")] ApproveIn { pub task_id: Id, pub sha: Option<String> });
op!(Approve, "task.approve", ApproveIn => Task,
    OpMeta::mutation(Scope::Project, 7, "open task → done, linking the commit").actors(Actors::UserOnly).undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskUnapproveIn")] UnapproveIn {
    pub task_id: Id,
    /// The column the task goes back to; not `done`.
    pub column: Column,
    /// Its 0-based index in that column, as in `task.move`.
    pub position: i64,
    /// The run state it had before the approval.
    pub state: TaskState,
    /// The commit link the approval created, removed again. Absent when the approval linked a
    /// commit that was already there, which then stays.
    pub sha: Option<String>,
});
op!(Unapprove, "task.unapprove", UnapproveIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Undo task.approve: done task → its old column, slot and state, dropping the commit link the approval added").actors(Actors::UserOnly).undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskParentSetIn")] ParentSetIn {
    pub task_id: Id,
    /// Absent or null detaches the task back to a root.
    pub parent_id: Option<Id>,
    pub position: Option<i64>,
});
op!(ParentSet, "task.parent.set", ParentSetIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Promote a task into a sub-task, re-parent it, or detach it").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskChildrenIn")] ChildrenIn { pub task_id: Id, pub recursive: Option<bool> });
op!(Children, "task.children", ChildrenIn => ListOut,
    OpMeta::query(Scope::Project, 7, "Direct children in board order, or the whole subtree"));

payload!(#[schemars(rename = "TaskLabelIn")] LabelIn { pub task_id: Id, pub label: String });
op!(LabelAdd, "task.label.add", LabelIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Tag a task, creating the project label if it is new").undo(Undo::Inverse).emits(&["task.changed"]));
op!(LabelRemove, "task.label.remove", LabelIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Untag a task").undo(Undo::Inverse).emits(&["task.changed"]));

payload!(#[schemars(rename = "TaskLabelListIn")] LabelListIn { pub project_id: Id });
result!(#[schemars(rename = "TaskLabelListOut")] LabelListOut { pub labels: Vec<Label> });
op!(LabelList, "task.label.list", LabelListIn => LabelListOut,
    OpMeta::query(Scope::Project, 7, "Every label in the project, alphabetical"));

payload!(#[schemars(rename = "TaskRelateIn")] RelateIn { pub task_id: Id, pub relation: TaskRelation, pub other_id: Id });
op!(Relate, "task.relate", RelateIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Record blocked_by or duplicate_of; stored and rendered, not enforced").undo(Undo::Inverse).emits(&["task.changed"]));
op!(Unrelate, "task.unrelate", RelateIn => Task,
    OpMeta::mutation(Scope::Project, 7, "Drop a blocked_by or duplicate_of edge").undo(Undo::Inverse).emits(&["task.changed"]));

result!(#[schemars(rename = "TaskCopyTextOut")] CopyTextOut { pub text: String });
op!(CopyText, "task.copy_text", IdIn => CopyTextOut, OpMeta::query(Scope::Project, 7, "A task as plain text: `#id title`, then the body"));

entries!(
    Create,
    Get,
    Activity,
    Comment,
    List,
    Update,
    Move,
    Delete,
    Restore,
    LinkCommit,
    ChangelogWrite,
    Attach,
    Detach,
    AttachmentRestore,
    ParentSet,
    Children,
    LabelAdd,
    LabelRemove,
    LabelList,
    Relate,
    Unrelate,
    Dispatch,
    Approve,
    Unapprove,
    CopyText
);
