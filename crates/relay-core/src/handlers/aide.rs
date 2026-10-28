//! `aide.*` — Aide, the person's own agent (docs/AIDE.md). `aide.db` is its own file, so reads
//! are `register_unlocked`; changes are audited like any mutation, and start nothing slow inside
//! the handler: a run is queued after it returns (`Ctx::after_commit`).
//!
//! A call from an Aide thread's agent (`Ctx::thread`) is Aide's own: the job it saves waits as a
//! proposal, the fact it adds waits to be confirmed, and the errand it closes asks the person.

use crate::aide::{self, outbox, runs, with_db};
use crate::engine::{Ctx, Engine};
use relay_aide::model::*;
use relay_aide::schedule;
use relay_bus::ops::aide::*;
use relay_bus::{BusError, Empty};
use serde_json::json;

fn invalid(m: impl Into<String>) -> BusError {
    BusError::invalid("aide.invalid", m)
}

fn not_found(what: &str, id: i64) -> BusError {
    BusError::not_found("aide.not_found", format!("No {what} {id}"))
}

fn view(job: Job) -> JobView {
    let mut when: Vec<String> = job.spec.schedule.as_ref().map(schedule::describe).into_iter().collect();
    when.extend(job.spec.rules.iter().map(Rule::describe));
    JobView { job, when }
}

fn job(engine: &Engine, id: i64) -> Result<Job, BusError> {
    with_db(engine, |db| db.job(id))?.ok_or_else(|| not_found("job", id))
}

/// The job an Aide thread's request comes from: its channel's.
fn own_job(engine: &Engine, thread: i64) -> Result<Option<Job>, BusError> {
    with_db(engine, |db| db.job_of_thread(thread))
}

/// When an active job's schedule next wakes it.
fn next_due(spec: &JobSpec, state: JobState) -> Option<String> {
    let s = spec.schedule.filter(|_| state == JobState::Active)?;
    schedule::next_after(&s, jiff::Timestamp::now()).map(crate::time::stamp)
}

fn checked_spec(spec: JobSpec) -> Result<JobSpec, BusError> {
    spec.checked().map_err(invalid)
}

fn rfc3339(field: &str, raw: &str) -> Result<String, BusError> {
    crate::time::bound(field, raw)
}

fn text(field: &str, raw: &str, max: usize) -> Result<String, BusError> {
    let t = raw.trim();
    if t.is_empty() || t.chars().count() > max {
        return Err(invalid(format!("{field} is 1 to {max} characters")));
    }
    Ok(t.to_string())
}

/// After a job is saved or turned on: what its rules find now is taken as seen, and the
/// scheduler looks at its schedule. Off the request path.
fn settle(ctx: &mut Ctx, job_id: i64) {
    ctx.after_commit(move |engine| {
        if let Ok(Some(job)) = with_db(&engine, |db| db.job(job_id)) {
            if job.state == JobState::Active {
                aide::rules::baseline(&engine, &job);
            }
        }
        aide::poke(&engine);
    });
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<Summary>(|ctx, _: Empty| {
        let engine = ctx.engine();
        let settings = aide::settings(engine)?;
        let jobs = with_db(engine, |db| db.jobs())?;
        let next = jobs.iter().filter(|j| j.state == JobState::Active).filter_map(|j| j.next_due_at.as_ref().map(|at| (at.clone(), j.spec.name.clone()))).min();
        let today = crate::time::stamp(schedule::day_start(jiff::Timestamp::now()));
        Ok(SummaryOut {
            paused: settings.paused,
            jobs: jobs.len() as u32,
            active: jobs.iter().filter(|j| j.state == JobState::Active).count() as u32,
            proposed: jobs.iter().filter(|j| j.state == JobState::Proposed).count() as u32,
            next_job: next.as_ref().map(|n| n.1.clone()),
            next_at: next.map(|n| n.0),
            running: with_db(engine, |db| db.open_runs())?,
            waiting: with_db(engine, |db| db.errands(false))?.iter().filter(|e| e.state == ErrandState::Waiting || e.proposed_done).count() as u32,
            held: with_db(engine, |db| db.held())?.len() as u32,
            interrupts_today: with_db(engine, |db| db.notified_since(&today))?,
            interrupt_cap: settings.interrupt_cap,
            unconfirmed: with_db(engine, |db| db.facts())?.iter().filter(|f| !f.confirmed).count() as u32,
        })
    });

    e.register_unlocked::<SettingsGet>(|ctx, _: Empty| aide::settings(ctx.engine()));
    e.register::<SettingsSet>(|ctx, p| {
        let s = p.checked().map_err(invalid)?;
        with_db(ctx.engine(), |db| db.set_settings(&s))?;
        ctx.emit("aide.changed", json!({"what": "settings"}));
        ctx.after_commit(|engine| {
            aide::poke(&engine);
            runs::advance(&engine);
        });
        Ok(s)
    });
    // Asking each server whether it answers is network work: no lock is held for it.
    e.register_unlocked::<Models>(|ctx, _: Empty| {
        let engine = ctx.engine();
        Ok(ModelsOut { models: aide::models::options(engine, &aide::settings(engine)?, true) })
    });

    e.register_unlocked::<JobList>(|ctx, _: Empty| Ok(JobsOut { jobs: with_db(ctx.engine(), |db| db.jobs())?.into_iter().map(view).collect() }));
    e.register_unlocked::<JobGet>(|ctx, p| {
        let engine = ctx.engine();
        let j = job(engine, p.id)?;
        Ok(JobOut { runs: with_db(engine, |db| db.runs(Some(p.id), 20))?, job: view(j) })
    });
    e.register::<JobSave>(|ctx, p| {
        let spec = checked_spec(p.spec)?;
        let now = ctx.now.clone();
        let from_aide = ctx.thread().is_some();
        let id = match p.id {
            Some(id) => {
                let old = job(ctx.engine(), id)?;
                if from_aide && old.state != JobState::Proposed {
                    return Err(BusError::refused("aide.person", "Aide may change only a job it proposed and the person has not turned on")
                        .with_hint("propose a new job with aide.job.propose"));
                }
                with_db(ctx.engine(), |db| db.update_job(id, &spec, next_due(&spec, old.state).as_deref(), &now))?;
                id
            }
            None => {
                let state = if from_aide { JobState::Proposed } else if p.active.unwrap_or(true) { JobState::Active } else { JobState::Paused };
                with_db(ctx.engine(), |db| db.add_job(&spec, state, next_due(&spec, state).as_deref(), &now))?
            }
        };
        ctx.emit("aide.changed", json!({"what": "job", "id": id}));
        settle(ctx, id);
        Ok(view(job(ctx.engine(), id)?))
    });
    e.register::<JobPropose>(|ctx, p| {
        let spec = checked_spec(p.spec)?;
        let why = text("why", &p.why, 600)?;
        let now = ctx.now.clone();
        let id = with_db(ctx.engine(), |db| db.add_job(&spec, JobState::Proposed, None, &now))?;
        let title = format!("Aide proposes a job: {}", spec.name);
        let (job_id, run_id) = match ctx.thread() {
            Some(t) => {
                let run = with_db(ctx.engine(), |db| db.last_run_of_thread(t))?;
                (run.as_ref().map(|r| r.job_id), run.map(|r| r.id))
            }
            None => (None, None),
        };
        outbox::keep(ctx.engine(), job_id, run_id, Level::Brief, &title, &why, false, true)?;
        ctx.emit("aide.changed", json!({"what": "job", "id": id}));
        Ok(view(job(ctx.engine(), id)?))
    });
    e.register::<JobSet>(|ctx, p| {
        if ctx.thread().is_some() {
            return Err(BusError::refused("aide.person", "Only the person turns a job on or off"));
        }
        if p.state == JobState::Proposed {
            return Err(invalid("A job is set active or paused"));
        }
        let j = job(ctx.engine(), p.id)?;
        let now = ctx.now.clone();
        with_db(ctx.engine(), |db| db.set_job_state(p.id, p.state, next_due(&j.spec, p.state).as_deref(), &now))?;
        ctx.emit("aide.changed", json!({"what": "job", "id": p.id}));
        settle(ctx, p.id);
        Ok(view(job(ctx.engine(), p.id)?))
    });
    e.register::<JobDelete>(|ctx, p| {
        job(ctx.engine(), p.id)?;
        for r in with_db(ctx.engine(), |db| db.open_runs())?.into_iter().filter(|r| r.job_id == p.id) {
            if let Some(t) = r.thread_id {
                ctx.after_commit(move |engine| {
                    crate::threads::stop(&engine, t);
                });
            }
        }
        with_db(ctx.engine(), |db| db.delete_job(p.id))?;
        ctx.emit("aide.changed", json!({"what": "job", "id": p.id}));
        ctx.after_commit(|engine| {
            aide::poke(&engine);
            runs::advance(&engine);
        });
        Ok(Empty {})
    });

    e.register::<RunNow>(|ctx, p| {
        let j = job(ctx.engine(), p.job_id)?;
        if j.state == JobState::Proposed {
            return Err(BusError::refused("aide.proposed", "Turn the job on first: it is a proposal"));
        }
        let trigger = p.note.map(|n| n.trim().chars().take(1000).collect::<String>()).filter(|n| !n.is_empty()).unwrap_or_else(|| "pressed Run".into());
        // Queued now, so the answer carries the run; started after the handler returns.
        let at = ctx.now.clone();
        let id = with_db(ctx.engine(), |db| db.add_run(p.job_id, Reason::Manual, &trigger, RunState::Queued, None, &at))?;
        ctx.emit("aide.changed", json!({"what": "run", "id": id}));
        ctx.after_commit(|engine| runs::advance(&engine));
        with_db(ctx.engine(), |db| db.run(id))?.ok_or_else(|| not_found("run", id))
    });
    e.register::<RunStop>(|ctx, p| {
        let r = with_db(ctx.engine(), |db| db.run(p.id))?.ok_or_else(|| not_found("run", p.id))?;
        if r.state.is_open() {
            let now = ctx.now.clone();
            with_db(ctx.engine(), |db| db.end_run(p.id, RunState::Failed, Some("Stopped by the person"), &now))?;
            let thread = r.thread_id.filter(|_| r.state == RunState::Running);
            ctx.after_commit(move |engine| {
                if let Some(t) = thread {
                    crate::threads::stop(&engine, t);
                    engine.emit_system("thread.changed", json!({"thread": crate::threads::get(&engine, t).ok()}));
                }
                runs::advance(&engine);
            });
        }
        ctx.emit("aide.changed", json!({"what": "run", "id": p.id}));
        with_db(ctx.engine(), |db| db.run(p.id))?.ok_or_else(|| not_found("run", p.id))
    });
    e.register_unlocked::<RunList>(|ctx, p| {
        let limit = p.limit.unwrap_or(50).clamp(1, 500);
        Ok(RunsOut { runs: with_db(ctx.engine(), |db| db.runs(p.job_id, limit))? })
    });
    e.register_unlocked::<RunGet>(|ctx, p| with_db(ctx.engine(), |db| db.run(p.id))?.ok_or_else(|| not_found("run", p.id)));
    e.register::<RunFinish>(|ctx, p| {
        let thread = ctx.thread().ok_or_else(|| BusError::refused("aide.agent", "aide.run.finish is Aide's agent's, for the run that woke it"))?;
        let r = runs::finish(ctx.engine(), thread, p.outcome, p.note.as_deref())?;
        ctx.emit("aide.changed", json!({"what": "run", "id": r.id}));
        ctx.after_commit(|engine| runs::advance(&engine));
        Ok(r)
    });

    e.register::<Notify>(|ctx, p| {
        let title = text("title", &p.title, 200)?;
        let body = p.body.map(|b| b.trim().chars().take(4000).collect::<String>()).unwrap_or_default();
        let (job_id, run_id, job_level) = match ctx.thread() {
            Some(t) => {
                let run = with_db(ctx.engine(), |db| db.last_run_of_thread(t))?;
                let j = own_job(ctx.engine(), t)?;
                (j.as_ref().map(|j| j.id), run.map(|r| r.id), j.map(|j| j.spec.level))
            }
            None => (None, None, None),
        };
        let level = p.level.or(job_level).unwrap_or_default();
        let (notified, held) = outbox::decide(ctx.engine(), level)?;
        let message = outbox::keep(ctx.engine(), job_id, run_id, level, &title, &body, notified, held.is_some())?;
        if notified {
            let now = ctx.now.clone();
            let id = outbox::notification(ctx.tx(), &title, &body, &now).map_err(|e| BusError::internal(e.to_string()))?;
            ctx.emit("notify.new", json!({"category": outbox::CATEGORY, "notification_id": id}));
        }
        ctx.emit("aide.changed", json!({"what": "message", "id": message.id}));
        Ok(NotifyOut { message, notified, held })
    });
    e.register::<Followup>(|ctx, p| {
        let thread = ctx.thread().ok_or_else(|| BusError::refused("aide.agent", "aide.followup is Aide's agent's"))?;
        let j = own_job(ctx.engine(), thread)?.ok_or_else(|| BusError::conflict("aide.no_run", "This thread is no job's channel"))?;
        if !(1..=10_080).contains(&p.minutes) {
            return Err(invalid("A follow-up is 1 minute to a week away"));
        }
        if with_db(ctx.engine(), |db| db.follow_ups_of(j.id))? >= 5 {
            return Err(BusError::conflict("aide.too_many", "This job already has five follow-ups waiting"));
        }
        let note = text("note", &p.note, 600)?;
        let due = jiff::Timestamp::now().checked_add(jiff::SignedDuration::from_mins(i64::from(p.minutes))).map_err(|e| invalid(e.to_string()))?;
        let due_at = crate::time::stamp(due);
        with_db(ctx.engine(), |db| db.add_follow_up(j.id, &due_at, &note))?;
        ctx.emit("aide.changed", json!({"what": "follow_up"}));
        ctx.after_commit(|engine| aide::poke(&engine));
        Ok(FollowupOut { due_at })
    });

    e.register_unlocked::<FactList>(|ctx, _: Empty| Ok(FactsOut { facts: with_db(ctx.engine(), |db| db.facts())? }));
    e.register::<FactAdd>(|ctx, p| {
        let fact_text = text("A fact", &p.text, 400)?;
        let now = ctx.now.clone();
        let from_aide = ctx.thread();
        let source = match (p.source.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()), from_aide) {
            (Some(s), _) if s == "person" || Source::parse(&s).is_some() => s,
            (Some(s), _) => return Err(invalid(format!("A fact's source is person, dev, money, arbiter or gym, not {s:?}"))),
            (None, Some(t)) => format!("thread:{t}"),
            (None, None) => "person".into(),
        };
        let job_id = match from_aide {
            Some(t) => own_job(ctx.engine(), t)?.map(|j| j.id),
            None => None,
        };
        let id = with_db(ctx.engine(), |db| db.add_fact(&fact_text, &source, job_id, from_aide.is_none(), &now))?;
        ctx.emit("aide.changed", json!({"what": "fact", "id": id}));
        with_db(ctx.engine(), |db| db.fact(id))?.ok_or_else(|| not_found("fact", id))
    });
    e.register::<FactUpdate>(|ctx, p| {
        let t = p.text.as_deref().map(|t| text("A fact", t, 400)).transpose()?;
        if !with_db(ctx.engine(), |db| db.update_fact(p.id, t.as_deref(), p.confirmed, p.pinned))? {
            return Err(not_found("fact", p.id));
        }
        ctx.emit("aide.changed", json!({"what": "fact", "id": p.id}));
        with_db(ctx.engine(), |db| db.fact(p.id))?.ok_or_else(|| not_found("fact", p.id))
    });
    e.register::<FactForget>(|ctx, p| {
        let forgotten = match (p.id, p.source.as_deref()) {
            (Some(id), None) => u32::from(with_db(ctx.engine(), |db| db.delete_fact(id))?),
            (None, Some(source)) => with_db(ctx.engine(), |db| db.forget_source(source.trim()))? as u32,
            _ => return Err(invalid("Give a fact's id, or a source to forget everything learned from it")),
        };
        ctx.emit("aide.changed", json!({"what": "fact"}));
        Ok(FactForgetOut { forgotten })
    });

    e.register_unlocked::<ErrandList>(|ctx, p| Ok(ErrandsOut { errands: with_db(ctx.engine(), |db| db.errands(p.closed.unwrap_or(false)))? }));
    e.register::<ErrandAdd>(|ctx, p| {
        let title = text("An errand's title", &p.title, 200)?;
        let detail = p.detail.unwrap_or_default().trim().chars().take(2000).collect::<String>();
        let due = p.due_at.as_deref().map(|d| rfc3339("due_at", d)).transpose()?;
        let job_id = match ctx.thread() {
            Some(t) => own_job(ctx.engine(), t)?.map(|j| j.id),
            None => None,
        };
        let state = if p.waiting.unwrap_or(false) { ErrandState::Waiting } else { ErrandState::Open };
        let now = ctx.now.clone();
        let id = with_db(ctx.engine(), |db| db.add_errand(&title, &detail, job_id, state, due.as_deref(), &now))?;
        ctx.emit("aide.changed", json!({"what": "errand", "id": id}));
        ctx.after_commit(|engine| aide::poke(&engine));
        with_db(ctx.engine(), |db| db.errand(id))?.ok_or_else(|| not_found("errand", id))
    });
    e.register::<ErrandUpdate>(|ctx, p| {
        with_db(ctx.engine(), |db| db.errand(p.id))?.ok_or_else(|| not_found("errand", p.id))?;
        let title = p.title.as_deref().map(|t| text("An errand's title", t, 200)).transpose()?;
        let detail = p.detail.map(|d| d.trim().chars().take(2000).collect::<String>());
        let due = p.due_at.as_deref().map(|d| rfc3339("due_at", d)).transpose()?;
        let from_aide = ctx.thread().is_some();
        // Only the person closes an errand: Aide's "done" asks them to.
        let (state, proposed) = match p.state {
            Some(ErrandState::Done | ErrandState::Dropped) if from_aide => (None, Some(true)),
            Some(s) => (Some(s), Some(false)),
            None => (None, None),
        };
        let now = ctx.now.clone();
        with_db(ctx.engine(), |db| db.update_errand(p.id, title.as_deref(), detail.as_deref(), state, due.as_deref().map(Some), proposed, &now))?;
        ctx.emit("aide.changed", json!({"what": "errand", "id": p.id}));
        ctx.after_commit(|engine| aide::poke(&engine));
        with_db(ctx.engine(), |db| db.errand(p.id))?.ok_or_else(|| not_found("errand", p.id))
    });

    e.register_unlocked::<PageList>(|ctx, _: Empty| Ok(PagesOut { pages: with_db(ctx.engine(), |db| db.pages())? }));
    e.register_unlocked::<PageGet>(|ctx, p| with_db(ctx.engine(), |db| db.page(p.id))?.ok_or_else(|| not_found("page", p.id)));
    e.register::<PageWrite>(|ctx, p| {
        let title = text("A page's title", &p.title, 120)?;
        let question = text("A page's question", &p.question, 1000)?;
        if p.body.chars().count() > 60_000 {
            return Err(invalid("A page is at most 60000 characters"));
        }
        let job_id = match ctx.thread() {
            Some(t) => own_job(ctx.engine(), t)?.map(|j| j.id),
            None => None,
        };
        let now = ctx.now.clone();
        let id = with_db(ctx.engine(), |db| db.write_page(&title, &question, p.body.trim(), job_id, &now))?;
        ctx.emit("aide.changed", json!({"what": "page", "id": id}));
        with_db(ctx.engine(), |db| db.page(id))?.ok_or_else(|| not_found("page", id))
    });
    e.register::<PageDelete>(|ctx, p| {
        if !with_db(ctx.engine(), |db| db.delete_page(p.id))? {
            return Err(not_found("page", p.id));
        }
        ctx.emit("aide.changed", json!({"what": "page", "id": p.id}));
        Ok(Empty {})
    });

    e.register_unlocked::<MessageList>(|ctx, p| Ok(MessagesOut { messages: with_db(ctx.engine(), |db| db.messages(p.limit.unwrap_or(50).clamp(1, 500)))? }));
}
