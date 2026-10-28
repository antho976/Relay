//! `aide.*` — Aide, the person's own agent (docs/AIDE.md): jobs that wake by the clock or on the
//! person's data, run as turns in an Aide thread on the model each names (Claude through the
//! person's account, a model on this PC, or one on their own server), speak first through one
//! outbox, and keep what they learn.
//!
//! Every op is the person's (`Actors::UserOnly`). Aide's own agent calls them as the person, as a
//! thread's agent does, through its door's list: its reads, `aide.notify`, `aide.run.finish`,
//! `aide.followup`, its facts, errands and pages, and `aide.job.propose`. The engine knows a call
//! from it (`Ctx::thread`): a job it writes waits as a proposal, a fact it adds waits to be
//! confirmed, and an errand it calls done waits for the person to close it.
//!
//! Events: `aide.changed` (a job, run, fact, errand, message, page or the settings changed).
use crate::registry::{Actors, OpMeta, Scope};
use crate::{op, Empty};
use relay_aide::templates::Template;
use relay_aide::model::{Errand, ErrandState, Fact, Job, JobSpec, JobState, Level, Message, Page, Run, RunState, Settings};

const CHANGED: &[&str] = &["aide.changed"];

const fn person(meta: OpMeta) -> OpMeta {
    meta.actors(Actors::UserOnly)
}

result!(#[schemars(rename = "AideModel")] ModelOption {
    /// What a job names.
    pub id: String,
    pub name: String,
    /// `claude` (the person's Claude account), `pc` (a model Relay runs on this PC) or `server`
    /// (one on the person's own server).
    pub host: String,
    /// It can run now: Claude signed in, the model's file present, the server answering.
    pub available: bool,
    /// Why not, when it cannot.
    pub note: Option<String>,
});

result!(#[schemars(rename = "AideSummary")] SummaryOut {
    pub paused: bool,
    pub jobs: u32,
    pub active: u32,
    /// Jobs Aide drafted, waiting for the person to turn them on.
    pub proposed: u32,
    /// The next job its schedule wakes, and when.
    pub next_job: Option<String>,
    pub next_at: Option<String>,
    /// Runs queued or running now.
    pub running: Vec<Run>,
    /// Errands waiting on the person.
    pub waiting: u32,
    /// Messages held for the next brief.
    pub held: u32,
    /// Interrupts shown today, and the day's cap.
    pub interrupts_today: u32,
    pub interrupt_cap: u32,
    /// Facts Aide learned that wait to be confirmed.
    pub unconfirmed: u32,
});
op!(Summary, "aide.summary", Empty => SummaryOut, person(OpMeta::query(Scope::Global, 12, "Aide at a glance: jobs, the next wake, what runs now, what waits on you, what is held for the brief")));

op!(SettingsGet, "aide.settings.get", Empty => Settings, person(OpMeta::query(Scope::Global, 12, "Aide's settings: paused, the default model and fallback, quiet hours, the interrupt cap, each source's privacy tier, your own servers' models")));
op!(SettingsSet, "aide.settings.set", Settings => Settings,
    person(OpMeta::mutation(Scope::Global, 12, "Replace Aide's settings").emits(CHANGED)));

result!(#[schemars(rename = "AideModelsOut")] ModelsOut { pub models: Vec<ModelOption> });
op!(Models, "aide.models", Empty => ModelsOut, person(OpMeta::query(Scope::Global, 12, "The models a job may name: Claude's, those Relay runs on this PC, and your own servers' (each server is asked, briefly, whether it answers)")));

result!(#[schemars(rename = "AideJobView")] JobView {
    #[serde(flatten)]
    pub job: Job,
    /// What wakes it, one line each: its schedule and its rules.
    pub when: Vec<String>,
});
result!(#[schemars(rename = "AideJobsOut")] JobsOut { pub jobs: Vec<JobView> });
op!(JobList, "aide.job.list", Empty => JobsOut, person(OpMeta::query(Scope::Global, 12, "Every job: what it does, what wakes it, its model, its state and its next wake")));

result!(#[schemars(rename = "AideTemplatesOut")] TemplatesOut { pub templates: Vec<Template> });
op!(JobTemplates, "aide.job.templates", Empty => TemplatesOut, person(OpMeta::query(Scope::Global, 12, "The jobs Aide starts with, to turn on as they are or change first: the morning brief, an agent waiting on you, budget and Arbiter watches, the weekly review")));

payload!(#[schemars(rename = "AideIdIn")] IdIn { pub id: i64 });
result!(#[schemars(rename = "AideJobOut")] JobOut {
    pub job: JobView,
    /// Its last runs, newest first.
    pub runs: Vec<Run>,
});
op!(JobGet, "aide.job.get", IdIn => JobOut, person(OpMeta::query(Scope::Global, 12, "One job and its last runs")));

payload!(#[schemars(rename = "AideJobSaveIn")] JobSaveIn {
    /// The job to change; a new one when absent.
    pub id: Option<i64>,
    pub spec: JobSpec,
    /// Turned on at once (a new job); otherwise it waits paused.
    pub active: Option<bool>,
});
op!(JobSave, "aide.job.save", JobSaveIn => JobView,
    person(OpMeta::mutation(Scope::Global, 12, "Make a job, or change one. From Aide's own agent it is a proposal instead (aide.job.propose)").emits(CHANGED)));

payload!(#[schemars(rename = "AideJobProposeIn")] JobProposeIn {
    pub spec: JobSpec,
    /// Why it would help, for the person's card.
    pub why: String,
});
op!(JobPropose, "aide.job.propose", JobProposeIn => JobView,
    person(OpMeta::mutation(Scope::Global, 12, "Draft a new job for the person to turn on: it does nothing until they do").emits(CHANGED)));

payload!(#[schemars(rename = "AideJobSetIn")] JobSetIn {
    pub id: i64,
    /// `active` (turning on a proposal approves it) or `paused`.
    pub state: JobState,
});
op!(JobSet, "aide.job.set", JobSetIn => JobView, person(OpMeta::mutation(Scope::Global, 12, "Turn a job on or pause it; turning a proposal on approves it").emits(CHANGED)));
op!(JobDelete, "aide.job.delete", IdIn => Empty, person(OpMeta::mutation(Scope::Global, 12, "Remove a job and its runs; its channel thread stays").emits(CHANGED)));

payload!(#[schemars(rename = "AideRunNowIn")] RunNowIn {
    pub job_id: i64,
    /// Said to the run along with the job's instructions.
    pub note: Option<String>,
});
op!(RunNow, "aide.run.now", RunNowIn => Run, person(OpMeta::mutation(Scope::Global, 12, "Run a job now, whatever its schedule (a paused job too)").emits(CHANGED)));
op!(RunStop, "aide.run.stop", IdIn => Run, person(OpMeta::mutation(Scope::Global, 12, "Stop a queued or running run").emits(CHANGED)));

payload!(#[schemars(rename = "AideRunListIn")] RunListIn {
    pub job_id: Option<i64>,
    /// 50 by default, 500 at most.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "AideRunsOut")] RunsOut { pub runs: Vec<Run> });
op!(RunList, "aide.run.list", RunListIn => RunsOut, person(OpMeta::query(Scope::Global, 12, "Runs, newest first: why each woke, its model, what it read, how it ended")));
op!(RunGet, "aide.run.get", IdIn => Run, person(OpMeta::query(Scope::Global, 12, "One run")));

payload!(#[schemars(rename = "AideRunFinishIn")] RunFinishIn {
    /// `done` (it did the job), `nothing` (it looked and there was nothing worth saying), `waiting`
    /// (it needs the person) or `failed`.
    pub outcome: RunState,
    /// One line on how it went.
    pub note: Option<String>,
});
op!(RunFinish, "aide.run.finish", RunFinishIn => Run,
    person(OpMeta::mutation(Scope::Global, 12, "Aide's agent: say how this run ended, as its last step").emits(CHANGED)));

payload!(#[schemars(rename = "AideNotifyIn")] NotifyIn {
    /// One line.
    pub title: String,
    /// The detail, in a few sentences of Markdown.
    pub body: Option<String>,
    /// `interrupt` (a notification now), `brief` (kept for the next brief) or `quiet` (on Aide's
    /// page only); the job's own level when absent.
    pub level: Option<Level>,
});
result!(#[schemars(rename = "AideNotifyOut")] NotifyOut {
    pub message: Message,
    /// It was shown as a notification now.
    pub notified: bool,
    /// It waits for the brief, and why: quiet hours, the day's cap, or its level.
    pub held: Option<String>,
});
op!(Notify, "aide.notify", NotifyIn => NotifyOut,
    person(OpMeta::mutation(Scope::Global, 12, "Aide's agent: tell the person something. An interrupt in quiet hours or past the day's cap waits for the brief").emits(&["aide.changed", "notify.new"])));

payload!(#[schemars(rename = "AideFollowupIn")] FollowupIn {
    /// Wake again this many minutes from now (1 to 10080).
    pub minutes: u32,
    /// What to look at then, said to that run.
    pub note: String,
});
result!(#[schemars(rename = "AideFollowupOut")] FollowupOut { pub due_at: String });
op!(Followup, "aide.followup", FollowupIn => FollowupOut,
    person(OpMeta::mutation(Scope::Global, 12, "Aide's agent: wake this job once more later (at most five waiting per job)").emits(CHANGED)));

result!(#[schemars(rename = "AideFactsOut")] FactsOut { pub facts: Vec<Fact> });
op!(FactList, "aide.fact.list", Empty => FactsOut, person(OpMeta::query(Scope::Global, 12, "What Aide has learned, pinned first, each with where it came from")));
payload!(#[schemars(rename = "AideFactAddIn")] FactAddIn {
    /// One sentence.
    pub text: String,
    /// Where it came from: a source (`dev`, `money`, `arbiter`, `gym`) or `person`. From Aide's
    /// agent, its thread when absent.
    pub source: Option<String>,
});
op!(FactAdd, "aide.fact.add", FactAddIn => Fact,
    person(OpMeta::mutation(Scope::Global, 12, "Keep a fact. One Aide's agent adds waits for the person to confirm it before runs are given it").emits(CHANGED)));
payload!(#[schemars(rename = "AideFactUpdateIn")] FactUpdateIn {
    pub id: i64,
    pub text: Option<String>,
    pub confirmed: Option<bool>,
    pub pinned: Option<bool>,
});
op!(FactUpdate, "aide.fact.update", FactUpdateIn => Fact, person(OpMeta::mutation(Scope::Global, 12, "Edit, confirm or pin a fact").emits(CHANGED)));
payload!(#[schemars(rename = "AideFactForgetIn")] FactForgetIn {
    /// One fact.
    pub id: Option<i64>,
    /// Every fact learned from this source.
    pub source: Option<String>,
});
result!(#[schemars(rename = "AideFactForgetOut")] FactForgetOut { pub forgotten: u32 });
op!(FactForget, "aide.fact.forget", FactForgetIn => FactForgetOut, person(OpMeta::mutation(Scope::Global, 12, "Forget one fact, or every fact from one source").emits(CHANGED)));

payload!(#[schemars(rename = "AideErrandListIn")] ErrandListIn {
    /// Closed ones too.
    pub closed: Option<bool>,
});
result!(#[schemars(rename = "AideErrandsOut")] ErrandsOut { pub errands: Vec<Errand> });
op!(ErrandList, "aide.errand.list", ErrandListIn => ErrandsOut, person(OpMeta::query(Scope::Global, 12, "What Aide is doing and what waits on the person, waiting first")));
payload!(#[schemars(rename = "AideErrandAddIn")] ErrandAddIn {
    pub title: String,
    pub detail: Option<String>,
    /// When it will be done (RFC 3339); past it, Aide says it is late.
    pub due_at: Option<String>,
    /// It waits on the person.
    pub waiting: Option<bool>,
});
op!(ErrandAdd, "aide.errand.add", ErrandAddIn => Errand, person(OpMeta::mutation(Scope::Global, 12, "Put an errand on the list").emits(CHANGED)));
payload!(#[schemars(rename = "AideErrandUpdateIn")] ErrandUpdateIn {
    pub id: i64,
    pub title: Option<String>,
    pub detail: Option<String>,
    /// From Aide's agent, `done` or `dropped` asks the person to close it.
    pub state: Option<ErrandState>,
    pub due_at: Option<String>,
});
op!(ErrandUpdate, "aide.errand.update", ErrandUpdateIn => Errand, person(OpMeta::mutation(Scope::Global, 12, "Change an errand; only the person closes one").emits(CHANGED)));

result!(#[schemars(rename = "AidePagesOut")] PagesOut { pub pages: Vec<Page> });
op!(PageList, "aide.page.list", Empty => PagesOut, person(OpMeta::query(Scope::Global, 12, "Aide's living pages, most recently written first")));
op!(PageGet, "aide.page.get", IdIn => Page, person(OpMeta::query(Scope::Global, 12, "One living page")));
payload!(#[schemars(rename = "AidePageWriteIn")] PageWriteIn {
    /// Its title: writing the same title again replaces the page.
    pub title: String,
    /// What it answers, kept so the next run writes it again from current data.
    pub question: String,
    /// Markdown; name the row each number came from.
    pub body: String,
});
op!(PageWrite, "aide.page.write", PageWriteIn => Page, person(OpMeta::mutation(Scope::Global, 12, "Write a living page, or write it again").emits(CHANGED)));
op!(PageDelete, "aide.page.delete", IdIn => Empty, person(OpMeta::mutation(Scope::Global, 12, "Remove a living page").emits(CHANGED)));

payload!(#[schemars(rename = "AideMessageListIn")] MessageListIn {
    /// 50 by default, 500 at most.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "AideMessagesOut")] MessagesOut { pub messages: Vec<Message> });
op!(MessageList, "aide.message.list", MessageListIn => MessagesOut, person(OpMeta::query(Scope::Global, 12, "What Aide said, newest first")));

entries!(
    Summary, SettingsGet, SettingsSet, Models, JobList, JobTemplates, JobGet, JobSave, JobPropose, JobSet, JobDelete, RunNow, RunStop, RunList, RunGet,
    RunFinish, Notify, Followup, FactList, FactAdd, FactUpdate, FactForget, ErrandList, ErrandAdd, ErrandUpdate, PageList, PageGet,
    PageWrite, PageDelete, MessageList,
);
