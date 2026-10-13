//! `notes.*` / `mailbox.*` — BUS.md §10.7.
use crate::registry::{Audit, OpMeta, Scope, Undo};
use crate::types::{Id, Message, Note, SessionState, Ts};
use crate::{op, Empty};

payload!(#[schemars(rename = "NotesListIn")] ListIn {
    pub project_id: Id, pub pinned_only: Option<bool>,
    /// Also list soft-deleted notes (`deleted_at` set), which `notes.restore` brings back.
    pub include_deleted: Option<bool>,
    /// Cut each `body` to a preview of its first 240 characters; `notes.get` has the whole note.
    pub summary: Option<bool>,
});
result!(#[schemars(rename = "NotesListOut")] ListOut { pub notes: Vec<Note> });
op!(List, "notes.list", ListIn => ListOut, OpMeta::query(Scope::Project, 5, "Notes for a project"));
payload!(#[schemars(rename = "NotesIdIn")] IdIn { pub note_id: Id });
op!(Get, "notes.get", IdIn => Note, OpMeta::query(Scope::Project, 5, "One note"));
payload!(#[schemars(rename = "NotesCreateIn")] CreateIn { pub project_id: Id, pub title: Option<String>, pub body: String, pub pinned: Option<bool> });
op!(Create, "notes.create", CreateIn => Note,
    OpMeta::mutation(Scope::Project, 5, "Create a note").undo(Undo::Inverse).emits(&["notes.changed"]));
payload!(#[schemars(rename = "NotesUpdateIn")] UpdateIn {
    pub note_id: Id,
    #[serde(default, deserialize_with = "crate::nullable")] pub title: Option<Option<String>>,
    pub body: Option<String>, pub pinned: Option<bool>,
    /// Original editable field values; every supplied value must still match atomically.
    pub expected: Option<serde_json::Map<String, serde_json::Value>>,
});
op!(Update, "notes.update", UpdateIn => Note,
    OpMeta::mutation(Scope::Project, 5, "Patch a note").undo(Undo::Inverse).emits(&["notes.changed"]));
payload!(#[schemars(rename = "NotesAppendIn")] AppendIn {
    pub note_id: Option<Id>, pub project_id: Option<Id>, pub target: Option<String>, pub text: String,
    /// The target note's `updated_at` as you last read it. When given and the note has changed
    /// since, nothing is appended (`notes.edit_conflict`); omitted, the text is appended as before.
    pub expected_updated_at: Option<Ts>,
});
op!(Append, "notes.append", AppendIn => Note,
    OpMeta::mutation(Scope::Project, 5, "Append text to a note, the project's standing note, or its agent suggestions note; pass expected_updated_at (from your last read) to refuse if the note changed since").emits(&["notes.changed"]));
payload!(#[schemars(rename = "NotesPinIn")] PinIn { pub note_id: Id, pub pinned: bool });
op!(Pin, "notes.pin", PinIn => Note,
    OpMeta::mutation(Scope::Project, 5, "Pin / unpin").undo(Undo::Inverse).emits(&["notes.changed"]));
op!(Delete, "notes.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Project, 5, "Soft-delete a note").undo(Undo::Inverse).emits(&["notes.deleted"]));
op!(Restore, "notes.restore", IdIn => Note,
    OpMeta::mutation(Scope::Project, 5, "Restore a note").undo(Undo::Inverse).emits(&["notes.changed"]));
payload!(#[schemars(rename = "NotesStandingIn")] StandingIn { pub project_id: Id });
result!(#[schemars(rename = "NotesStandingOut")] StandingOut { pub text: String });
op!(Standing, "notes.standing", StandingIn => StandingOut, OpMeta::query(Scope::Project, 5, "The project's standing note: agents read it on request; it is not injected into briefs"));

payload!(#[schemars(rename = "NotesSendIn")] SendIn { pub project_id: Id, pub to: String, pub text: String, pub re_task: Option<Id>, pub priority: Option<bool> });
// One addressee of a message and where it currently stands.
result!(#[schemars(rename = "MailboxRecipient")] Recipient {
    pub session: String,
    /// The recipient session's state right now: `running` will see it, `parked` will not
    /// until it wakes.
    pub state: SessionState,
    pub acked_at: Option<Ts>,
});
result!(#[schemars(rename = "MailboxSendOut")] SendOut {
    pub message: Message,
    pub recipients: Vec<Recipient>,
    /// `queued` (at least one addressee is live), `session_parked` (all parked),
    /// `not_running` (all created or exited), or `no_recipients` (an empty broadcast).
    pub delivery: String,
});
op!(MailboxSend, "mailbox.send", SendIn => SendOut,
    OpMeta::mutation(Scope::Project, 5, "Send normal or priority mail to a session name, or normal mail to * (broadcast); reaches every provider").emits(&["mailbox.new"]));
payload!(#[schemars(rename = "MailboxListIn")] MailboxListIn {
    pub project_id: Id, pub session: Option<String>, pub unread_only: Option<bool>, pub since: Option<Ts>,
    /// Page size, default 200, at most 1000.
    pub limit: Option<u32>,
    /// Only messages older than this message id: the previous page's `next_before`.
    pub before: Option<Id>,
});
result!(#[schemars(rename = "MailboxListOut")] MailboxListOut {
    /// Oldest first. Without `unread_only` this is the newest page; with it, the oldest unread.
    pub messages: Vec<Message>,
    /// Set when older messages remain: pass it as `before` for the page before this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before: Option<Id>,
    /// More unread messages remain past this page (`unread_only`): ack these, then ask again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub more_unread: bool,
});
op!(MailboxList, "mailbox.list", MailboxListIn => MailboxListOut, OpMeta::query(Scope::Project, 5, "Messages, optionally for one session"));
payload!(#[schemars(rename = "MailboxOutboxIn")] OutboxIn { pub project_id: Id, pub since: Option<Ts>, pub limit: Option<u32> });
result!(#[schemars(rename = "MailboxOutboxEntry")] OutboxEntry { pub message: Message, pub recipients: Vec<Recipient> });
result!(#[schemars(rename = "MailboxOutboxOut")] OutboxOut { pub sent: Vec<OutboxEntry> });
op!(MailboxOutbox, "mailbox.outbox", OutboxIn => OutboxOut,
    OpMeta::query(Scope::Project, 5, "Messages this actor sent, with each addressee's state"));
payload!(#[schemars(rename = "NotesAckIn")] AckIn { pub message_id: Id });
op!(MailboxAck, "mailbox.ack", AckIn => Empty,
    OpMeta::mutation(Scope::Project, 5, "Mark a message read").audit(Audit::AgentOnly).emits(&["mailbox.changed"]));

entries!(
    List,
    Get,
    Create,
    Update,
    Append,
    Pin,
    Delete,
    Restore,
    Standing,
    MailboxSend,
    MailboxList,
    MailboxOutbox,
    MailboxAck
);
