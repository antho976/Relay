//! `thread.*` — Threads: conversations with an agent that reads and changes the person's own
//! data (docs/THREADS.md).
//!
//! A thread is kept by the engine in its own SQLite file (`threads.db`), so a thread reopens
//! from Relay's copy without asking the agent anything. The agent itself runs in the background
//! while the thread is in use and is resumed by its own conversation id when a cold thread gets
//! a new message. Only the person starts, steers or removes threads (`Actors::UserOnly`); the
//! agent reaches the person's data through the ops its thread is given, not through these.
//!
//! Events: `thread.changed` (a thread was created, renamed, deleted, or started or finished a
//! turn), `thread.message` (a message was stored), `thread.delta` (a piece of the reply being
//! written, for live display; not stored) and `thread.proposal` (a proposal was made or answered).
//!
//! Asking first: what the agent may not change alone it proposes (`thread.propose`, one
//! [`Action`] each), and the person approves or declines it on a card (`thread.proposal.*`).
//!
//! The assistant (`kind: "assistant"`) is a thread about Relay itself, opened from the Dev space's
//! title bar: it reads sessions, tasks, projects and mail, and what a lone session may not do it
//! asks for (`thread.ask`, a few steps of user-only ops), the person approving each on a card
//! (`thread.ask.*`).
use crate::ops::money::{AccountAddIn, AccountUpdateIn, BudgetSetIn, IdIn as MoneyIdIn, TxUpdateIn};
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::types::Id;
use crate::{op, Empty};
use relay_money::views::{AccountView, ActivityView, Tx};
use serde_json::Value;

result!(#[schemars(rename = "ThreadView")] ThreadView {
    pub id: Id,
    pub title: String,
    /// `claude`; the agent that answers in this thread.
    pub provider: String,
    pub model: Option<String>,
    /// `low`, `medium`, `high`, `xhigh` or `max`; the provider's default when absent.
    pub effort: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// The agent is writing a reply.
    pub working: bool,
    /// The agent is running for this thread now, so a new message costs no resume.
    pub live: bool,
    /// The start of the last message, for the thread list.
    pub preview: Option<String>,
    /// `money` (a Threads conversation about Tally, Arbiter and Avex) or `assistant` (the Dev
    /// space's assistant, about Relay itself).
    pub kind: String,
});
result!(#[schemars(rename = "ThreadMessage")] MessageView {
    pub id: Id,
    pub thread_id: Id,
    /// `user`, `assistant` (text and tool calls), `tool` (a tool's answer) or `error`.
    pub role: String,
    /// `{"text"}` for `user` and `error`; a `user` message with pictures is
    /// `{"text", "images": [{"id","media_type","width","height"}]}` (`text` may then be empty;
    /// `images` is absent when there are none), each drawn from `thread.image {id}`;
    /// `{"blocks": [...]}` for `assistant`, each block `{"type":"text","text"}` or
    /// `{"type":"tool_use","id","name","input"}`; `{"tool_use_id","is_error","text"}` for `tool`.
    pub body: Value,
    pub created_at: String,
});

payload!(#[schemars(rename = "ThreadListIn")] ListIn {
    /// `assistant` for the Dev space's assistant conversations; absent or `money`, the Threads
    /// space's.
    pub kind: Option<String>,
});
result!(#[schemars(rename = "ThreadListOut")] ListOut { pub threads: Vec<ThreadView> });
op!(List, "thread.list", ListIn => ListOut, OpMeta::query(Scope::Global, 5, "Every thread of one kind, most recently used first"));

payload!(#[schemars(rename = "ThreadGetIn")] GetIn {
    pub id: Id,
    /// Only the messages stored after this message id, from the oldest on.
    pub after: Option<Id>,
    /// Only the messages stored before this message id: the page before one already shown.
    pub before: Option<Id>,
});
result!(#[schemars(rename = "ThreadGetOut")] GetOut {
    pub thread: ThreadView,
    /// Oldest first, about 1 MiB of them at most: the newest, or with `after` the oldest after
    /// it. A tool answer longer than 8,000 characters is cut, and its body says `"cut": true`.
    pub messages: Vec<MessageView>,
    /// Older messages were left out: `before` the first id reads them.
    pub earlier: bool,
    /// Newer messages were left out (only with `after`): `after` the last id reads them.
    pub later: bool,
});
op!(Get, "thread.get", GetIn => GetOut, OpMeta::query(Scope::Global, 5, "One thread and its newest messages (about 1 MiB), oldest first"));

payload!(#[schemars(rename = "ThreadCreateIn")] CreateIn {
    /// The first message; the thread is named after it and the agent starts on it.
    pub text: Option<String>,
    /// Pictures for the first message, by the ids `thread.upload` gave, at most 4; with them
    /// `text` may be empty or absent, and the thread is named "Picture" (or "2 pictures").
    pub images: Option<Vec<String>>,
    /// A model for this thread's agent; the provider's default when absent.
    pub model: Option<String>,
    /// How hard it thinks: `low`, `medium`, `high`, `xhigh` or `max`; the default when absent.
    pub effort: Option<String>,
    /// `assistant` for the Dev space's assistant; absent or `money`, a Threads conversation.
    pub kind: Option<String>,
});
op!(Create, "thread.create", CreateIn => ThreadView,
    OpMeta::mutation(Scope::Global, 5, "Start a thread, optionally with its first message").actors(Actors::UserOnly).emits(&["thread.changed", "thread.message"]));

payload!(#[schemars(rename = "ThreadSendIn")] SendIn {
    pub id: Id,
    /// May be empty when the message carries pictures.
    pub text: String,
    /// Pictures, by the ids `thread.upload` gave, at most 4. Refused with
    /// `thread.images_unsupported` in a thread on a model on this PC, which cannot read them.
    pub images: Option<Vec<String>>,
});
op!(Send, "thread.send", SendIn => MessageView,
    OpMeta::mutation(Scope::Global, 5, "Send a message in a thread; the agent answers in thread.delta and thread.message events")
        .actors(Actors::UserOnly).emits(&["thread.changed", "thread.message", "thread.delta"]));

payload!(#[schemars(rename = "ThreadUploadIn")] UploadIn {
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`. The bytes decide: a picture is
    /// stored as what it is.
    pub media_type: String,
    /// The picture, base64: at most 4 MiB decoded.
    pub bytes_b64: String,
});
result!(#[schemars(rename = "ThreadImage")] ImageView {
    /// 32 lowercase hex characters: what a message's `images` names it by.
    pub id: String,
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`, read from its bytes.
    pub media_type: String,
    /// Its size in px.
    pub width: u32,
    pub height: u32,
    /// Its length in bytes.
    pub bytes: u64,
});
result!(#[schemars(rename = "ThreadUploadOut")] UploadOut { pub image: ImageView });
op!(Upload, "thread.upload", UploadIn => UploadOut,
    OpMeta::mutation(Scope::Global, 5, "A picture for a thread's next message: PNG, JPEG, GIF or WebP, at most 4 MiB. Send its id in thread.send's or thread.create's images; one never sent is removed after a day")
        .audit(Audit::AgentOnly).actors(Actors::UserOnly));

payload!(#[schemars(rename = "ThreadImageIn")] ImageIn {
    /// The id in a message's `images`.
    pub id: String,
});
result!(#[schemars(rename = "ThreadImageOut")] ImageOut {
    pub media_type: String,
    /// The picture, base64.
    pub bytes_b64: String,
});
op!(Image, "thread.image", ImageIn => ImageOut,
    OpMeta::query(Scope::Global, 5, "A picture a message in a thread carries, to draw it").actors(Actors::UserOnly));

payload!(#[schemars(rename = "ThreadIdIn")] IdIn { pub id: Id });
op!(Stop, "thread.stop", IdIn => Empty,
    OpMeta::mutation(Scope::Global, 5, "Stop the agent's reply in a thread; the next message resumes it").actors(Actors::UserOnly).emits(&["thread.changed"]));

payload!(#[schemars(rename = "ThreadSetIn")] SetIn {
    pub id: Id,
    /// The agent's model; empty or absent for the provider's default.
    pub model: Option<String>,
    /// The agent's effort; empty or absent for the default.
    pub effort: Option<String>,
});
op!(Set, "thread.set", SetIn => ThreadView,
    OpMeta::mutation(Scope::Global, 5, "Choose a thread's model and effort; a running agent restarts with them on the next message")
        .actors(Actors::UserOnly).emits(&["thread.changed"]));

payload!(#[schemars(rename = "ThreadRenameIn")] RenameIn { pub id: Id, pub title: String });
op!(Rename, "thread.rename", RenameIn => ThreadView,
    OpMeta::mutation(Scope::Global, 5, "Rename a thread").actors(Actors::UserOnly).emits(&["thread.changed"]));

op!(Delete, "thread.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Global, 5, "Delete a thread and its messages, stopping its agent; its pictures go too, unless a message of another thread carries one").actors(Actors::UserOnly).emits(&["thread.changed"]));

result!(#[schemars(rename = "ThreadLocalModel")] LocalModel {
    /// The model id a thread stores and the picker sends.
    pub id: String,
    pub name: String,
    /// Its GGUF file.
    pub file: String,
    /// The file is in the models folder, so the model can run.
    pub present: bool,
});
result!(#[schemars(rename = "ThreadModelsOut")] ModelsOut {
    /// The models a thread may run on this PC, besides the provider's.
    pub models: Vec<LocalModel>,
    /// The local model the server runs now, if any.
    pub running: Option<String>,
    /// It has loaded and answers.
    pub ready: bool,
    /// Minutes unused before the server stops; 0 never (`threads.local.idle_minutes`).
    pub idle_minutes: u64,
    /// The `llama-server` that runs them; absent when it cannot be found, with `server_error`.
    pub server: Option<String>,
    pub server_error: Option<String>,
});
op!(Models, "thread.models", Empty => ModelsOut, OpMeta::query(Scope::Global, 5, "The local models a thread may run on, and the one running now"));

payload!(#[schemars(rename = "ThreadPickIn")] PickIn {
    /// The model picked: a local one starts loading, anything else (or none) stops the local
    /// model server.
    pub model: Option<String>,
});
op!(Pick, "thread.pick", PickIn => Empty,
    OpMeta::mutation(Scope::Global, 5, "A model was picked in the Threads picker: load a local one now, or stop the local one")
        .actors(Actors::UserOnly).emits(&["thread.local"]));

// ── Asking first (docs/THREADS.md, phase 4) ───────────────────────────────

/// One change a thread's agent asks the person for: the op approving runs, keyed by its name,
/// holding a list of that op's own payloads, all applied together or none,
/// e.g. `{"money.tx.delete": [{"id": 42}, {"id": 43}]}`.
#[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
#[schemars(rename = "ThreadAction")]
pub enum Action {
    /// Delete entries, up to 50.
    #[serde(rename = "money.tx.delete")]
    TxDelete(Vec<MoneyIdIn>),
    /// Change entries, up to 50: recategorize them, or fix an amount, a date or a note.
    #[serde(rename = "money.tx.update")]
    TxUpdate(Vec<TxUpdateIn>),
    /// Set monthly budgets, up to 20; an amount of zero removes one.
    #[serde(rename = "money.budget.set")]
    BudgetSet(Vec<BudgetSetIn>),
    /// Add accounts, up to 5.
    #[serde(rename = "money.account.add")]
    AccountAdd(Vec<AccountAddIn>),
    /// Rename or archive accounts, up to 5.
    #[serde(rename = "money.account.update")]
    AccountUpdate(Vec<AccountUpdateIn>),
    /// Delete investment activities, up to 50.
    #[serde(rename = "money.invest.delete")]
    InvestDelete(Vec<MoneyIdIn>),
}

impl Action {
    /// The op approving runs, once per payload.
    pub fn op(&self) -> &'static str {
        match self {
            Action::TxDelete(_) => "money.tx.delete",
            Action::TxUpdate(_) => "money.tx.update",
            Action::BudgetSet(_) => "money.budget.set",
            Action::AccountAdd(_) => "money.account.add",
            Action::AccountUpdate(_) => "money.account.update",
            Action::InvestDelete(_) => "money.invest.delete",
        }
    }

    /// How many payloads it holds.
    pub fn len(&self) -> usize {
        match self {
            Action::TxDelete(v) | Action::InvestDelete(v) => v.len(),
            Action::TxUpdate(v) => v.len(),
            Action::BudgetSet(v) => v.len(),
            Action::AccountAdd(v) => v.len(),
            Action::AccountUpdate(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The most payloads one proposal of this kind may hold.
    pub fn most(&self) -> usize {
        match self {
            Action::TxDelete(_) | Action::TxUpdate(_) | Action::InvestDelete(_) => 50,
            Action::BudgetSet(_) => 20,
            Action::AccountAdd(_) | Action::AccountUpdate(_) => 5,
        }
    }
}

/// One thing a proposal acts on, read from the ledger: as it is now while the proposal waits,
/// and as it was just before it was answered after that, so a card still shows what was deleted.
#[derive(Debug, Clone, Default, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
#[schemars(rename = "ThreadProposalItem")]
pub struct Item {
    /// `same` as when it was proposed, `changed` since, `gone` from the ledger since, or `new`
    /// (an account to add).
    pub state: String,
    /// Entries: the entry, and as the change would leave it.
    pub entry: Option<Tx>,
    pub entry_after: Option<Tx>,
    /// Investment activities to delete.
    pub activity: Option<ActivityView>,
    /// Accounts: the account (absent for one to add), and as the change would leave it.
    pub account: Option<AccountView>,
    pub account_after: Option<AccountView>,
    /// Budgets: the category's name (absent for the overall monthly budget), the budget now and
    /// the one proposed; absent when there is none.
    pub category: Option<String>,
    #[schemars(extend("x-unit" = "minor"))]
    pub budget: Option<i64>,
    #[schemars(extend("x-unit" = "minor"))]
    pub budget_after: Option<i64>,
}

payload!(#[schemars(rename = "ThreadProposeIn")] ProposeIn {
    /// The one change: `{"<op>": [<that op's payload>, ...]}`.
    pub action: Action,
    /// Why, in a sentence the person reads on the card.
    pub why: String,
});
result!(#[schemars(rename = "ThreadProposal")] ProposalView {
    pub id: Id,
    pub thread_id: Id,
    pub action: Action,
    pub why: String,
    /// What approving does, in words read from the ledger. Quote it.
    pub summary: String,
    /// `pending`, `running` (approved, under way), `approved`, `declined`, `failed` or `undone`.
    pub status: String,
    /// What approving did, or why it could not be done.
    pub outcome: Option<String>,
    /// The ledger's currency.
    pub currency: String,
    /// What it acts on, one per payload, in order.
    pub items: Vec<Item>,
    /// Something changed or went since it was proposed: approving needs a fresh look (`seen`).
    pub stale: bool,
    /// The fingerprint of `items` as read now: an approval passes it back as `seen`.
    pub seen: String,
    /// Approved, and `thread.proposal.undo` can put it back.
    pub undoable: bool,
    pub created_at: String,
    pub resolved_at: Option<String>,
});
op!(Propose, "thread.propose", ProposeIn => ProposalView,
    OpMeta::mutation(Scope::Global, 5, "A thread's agent asks the person for a change it may not make alone: deleting entries or investment activities, changing many entries, budgets, adding, renaming or archiving accounts. It waits on a card; nothing changes until they approve")
        .emits(&["thread.proposal"]));

payload!(#[schemars(rename = "ThreadProposalListIn")] ProposalListIn {
    pub thread_id: Id,
    /// Only those still waiting when true.
    pub pending: Option<bool>,
});
result!(#[schemars(rename = "ThreadProposalsOut")] ProposalsOut { pub proposals: Vec<ProposalView> });
op!(ProposalList, "thread.proposal.list", ProposalListIn => ProposalsOut,
    OpMeta::query(Scope::Global, 5, "A thread's proposals, oldest first, each with what it acts on read from the ledger").actors(Actors::UserOnly));
op!(ProposalGet, "thread.proposal.get", IdIn => ProposalView,
    OpMeta::query(Scope::Global, 5, "One proposal, with what it acts on read from the ledger").actors(Actors::UserOnly));

payload!(#[schemars(rename = "ThreadProposalResolveIn")] ProposalResolveIn {
    pub id: Id,
    pub approve: bool,
    /// The `seen` of the reading the person approved. Without it, an approval is refused once
    /// anything changed since the proposal.
    pub seen: Option<String>,
});
op!(ProposalResolve, "thread.proposal.resolve", ProposalResolveIn => ProposalView,
    OpMeta::mutation(Scope::Global, 5, "Approve or decline a proposal. Approving applies all of it once, as the person, or none of it; a failure is kept on the proposal with its reason. The agent hears the answer with the next message")
        .actors(Actors::UserOnly).emits(&["thread.proposal", "money.changed"]));
op!(ProposalUndo, "thread.proposal.undo", IdIn => ProposalView,
    OpMeta::mutation(Scope::Global, 5, "Put an approved proposal back: entries and activities restored, budgets and accounts as they were, all or none")
        .actors(Actors::UserOnly).emits(&["thread.proposal", "money.changed"]));

// ── The assistant's asks ─────────────────────────────────────────────────

payload!(
/// One step of an ask: an op the person's approval runs, as the person, with its own payload.
#[schemars(rename = "ThreadAskStep")] AskStep {
    /// One of the ops an assistant may ask for: `session.create`, `session.spawn`,
    /// `session.resume`, `session.park`, `session.wake`, `session.close`, `session.input`,
    /// `session.update`, `task.create`, `task.update`, `task.move`, `task.dispatch`,
    /// `task.delete`, `notes.create`, `notes.update`, `workspace.create`, `project.add`.
    pub op: String,
    /// That op's own payload. A `session` left out of a step after `session.create` is the
    /// session it made.
    pub payload: Value,
});
payload!(#[schemars(rename = "ThreadAskIn")] AskIn {
    /// What approving runs, in order, at most 5; the first that fails stops the rest.
    pub steps: Vec<AskStep>,
    /// Why, in a sentence the person reads on the card.
    pub why: String,
});
result!(#[schemars(rename = "ThreadAsk")] AskView {
    pub id: Id,
    pub thread_id: Id,
    pub steps: Vec<AskStep>,
    pub why: String,
    /// What approving does, in words. Quote it.
    pub summary: String,
    /// `pending`, `running` (approved, under way), `approved`, `declined` or `failed`.
    pub status: String,
    /// What approving did, or why a step failed (and which steps ran before it).
    pub outcome: Option<String>,
    /// Each step's result that ran, in order.
    pub results: Vec<Value>,
    pub created_at: String,
    pub resolved_at: Option<String>,
});
op!(Ask, "thread.ask", AskIn => AskView,
    OpMeta::mutation(Scope::Global, 5, "The assistant asks the person for what a lone session may not do: launching, parking, waking, closing or typing into sessions, changing tasks, notes, workspaces and projects. It waits on a card; nothing runs until they approve")
        .emits(&["thread.ask"]));

payload!(#[schemars(rename = "ThreadAskListIn")] AskListIn {
    pub thread_id: Id,
    /// Only those still waiting when true.
    pub pending: Option<bool>,
});
result!(#[schemars(rename = "ThreadAsksOut")] AsksOut { pub asks: Vec<AskView> });
op!(AskList, "thread.ask.list", AskListIn => AsksOut,
    OpMeta::query(Scope::Global, 5, "An assistant thread's asks, oldest first").actors(Actors::UserOnly));

payload!(#[schemars(rename = "ThreadAskResolveIn")] AskResolveIn { pub id: Id, pub approve: bool });
op!(AskResolve, "thread.ask.resolve", AskResolveIn => AskView,
    OpMeta::mutation(Scope::Global, 5, "Approve or decline an assistant's ask. Approving runs its steps once, in order, as the person; the first failure stops the rest and is kept with its reason. The assistant hears the answer with the next message")
        .actors(Actors::UserOnly).emits(&["thread.ask"]));

entries!(List, Get, Create, Send, Upload, Image, Stop, Set, Rename, Delete, Models, Pick, Propose, ProposalList, ProposalGet, ProposalResolve, ProposalUndo, Ask, AskList, AskResolve);
