//! `thread.*` — Threads (docs/THREADS.md).
//!
//! Threads live in `threads.db` behind their own mutex (`crate::threads`), so reads run unlocked
//! and the mutations only store a row while the store's transaction is open. Starting the agent
//! and handing it a message happen after the handler returns.

use crate::engine::{Ctx, Engine};
use crate::threads::proposals;
use crate::{local_models, threads};
use relay_bus::ops::thread::*;
use relay_bus::{BusError, Empty};
use serde_json::json;

pub fn register(e: &mut Engine) {
    e.register_unlocked::<List>(|ctx, _: Empty| Ok(ListOut { threads: threads::list(ctx.engine())? }));
    e.register_unlocked::<Get>(|ctx, p| {
        let engine = ctx.engine();
        let messages = threads::messages(engine, p.id, p.after)?;
        Ok(GetOut { thread: threads::get(engine, p.id)?, messages })
    });
    e.register::<Create>(|ctx, p| {
        let text = p.text.as_deref().map(threads::checked_text).transpose()?;
        let title = text.as_deref().map_or_else(|| "New thread".to_string(), threads::title_from);
        let now = ctx.now.clone();
        let thread = threads::create(ctx.engine(), &title, p.model.as_deref(), p.effort.as_deref(), &now)?;
        ctx.emit("thread.changed", json!({"thread": thread}));
        let Some(text) = text else { return Ok(thread) };
        let (message, turn) = threads::post(ctx.engine(), thread.id, &text, &now)?;
        ctx.emit("thread.message", json!({"thread": thread.id, "message": message}));
        let id = thread.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, turn, &text));
        threads::get(ctx.engine(), id)
    });
    e.register::<Send>(|ctx, p| {
        let text = threads::checked_text(&p.text)?;
        let now = ctx.now.clone();
        let (message, turn) = threads::post(ctx.engine(), p.id, &text, &now)?;
        ctx.emit("thread.message", json!({"thread": p.id, "message": message}));
        let id = p.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, turn, &text));
        Ok(message)
    });
    e.register::<Stop>(|ctx, p| {
        threads::get(ctx.engine(), p.id)?;
        if threads::stop(ctx.engine(), p.id) {
            ctx.emit("thread.changed", json!({"thread": threads::get(ctx.engine(), p.id)?}));
        }
        Ok(Empty {})
    });
    e.register::<Set>(|ctx, p| {
        let thread = threads::set_agent(ctx.engine(), p.id, p.model.as_deref(), p.effort.as_deref())?;
        ctx.emit("thread.changed", json!({"thread": thread}));
        Ok(thread)
    });
    e.register::<Rename>(|ctx, p| {
        let thread = threads::rename(ctx.engine(), p.id, &p.title)?;
        ctx.emit("thread.changed", json!({"thread": thread}));
        Ok(thread)
    });
    // Reads a setting or two and looks for three files: no transaction needed.
    e.register_unlocked::<Models>(|ctx, _: Empty| {
        let engine = ctx.engine();
        let dir = local_models::dir(engine);
        let models = local_models::MODELS.iter().map(|m| LocalModel {
            id: m.id.into(), name: m.name.into(), file: m.file.into(), present: dir.join(m.file).is_file(),
        }).collect();
        let status = local_models::status(engine);
        let server = local_models::server_bin(engine);
        Ok(ModelsOut {
            models,
            running: status.map(|(m, _)| m.id.to_string()),
            ready: status.is_some_and(|(_, ready)| ready),
            idle_minutes: local_models::idle_minutes(engine),
            server: server.as_ref().ok().map(|p| p.display().to_string()),
            server_error: server.err(),
        })
    });
    e.register::<Pick>(|ctx, p| {
        let model = p.model.filter(|m| !m.trim().is_empty());
        ctx.after_commit(move |engine| local_models::pick(&engine, model.as_deref()));
        Ok(Empty {})
    });
    e.register::<Delete>(|ctx, p| {
        threads::delete(ctx.engine(), p.id)?;
        ctx.emit("thread.changed", json!({"id": p.id, "deleted": true}));
        Ok(Empty {})
    });
    register_proposals(e);
}

/// Asking first (`threads::proposals`). Proposing reads the ledger and stores a row, and an
/// answer applies the batch in one short ledger transaction: local work like any money op's, so
/// all run in the request's transaction. The thread store's lock and the ledger's are never held
/// together, and no other op's handler runs inside these.
fn register_proposals(e: &mut Engine) {
    e.register::<Propose>(|ctx, p| {
        // Only an agent's own thread: the door names it from the process that connected.
        let Some(thread) = ctx.thread() else {
            return Err(BusError::refused("thread.not_agent", "Only a thread's agent proposes, for its own thread")
                .with_hint("the person changes the ledger directly; a session has no thread"));
        };
        let now = ctx.now.clone();
        let proposal = proposals::propose(ctx.engine(), thread, p.action, &p.why, &now)?;
        ctx.emit("thread.proposal", json!({"thread": thread, "proposal": proposal}));
        Ok(proposal)
    });
    e.register_unlocked::<ProposalList>(|ctx, p| Ok(ProposalsOut { proposals: proposals::list(ctx.engine(), p.thread_id, p.pending.unwrap_or(false))? }));
    e.register_unlocked::<ProposalGet>(|ctx, p| proposals::get(ctx.engine(), p.id));
    e.register::<ProposalResolve>(|ctx, p| {
        person(ctx)?;
        let proposal = proposals::resolve(ctx.engine(), p.id, p.approve, p.seen.as_deref())?;
        answered(ctx, &proposal);
        Ok(proposal)
    });
    e.register::<ProposalUndo>(|ctx, p| {
        person(ctx)?;
        let proposal = proposals::undo(ctx.engine(), p.id)?;
        answered(ctx, &proposal);
        Ok(proposal)
    });
}

/// An answer is the person's: a thread's agent never answers a proposal, its own or another's,
/// whatever its door lets through.
fn person(ctx: &Ctx) -> Result<(), BusError> {
    match ctx.thread() {
        Some(thread) => Err(BusError::refused("thread.not_person", format!("Thread {thread}'s agent cannot answer a proposal: the person does"))),
        None => Ok(()),
    }
}

fn answered(ctx: &mut Ctx, proposal: &ProposalView) {
    if matches!(proposal.status.as_str(), "approved" | "undone") {
        ctx.emit("money.changed", json!({"proposal": proposal.id}));
    }
    ctx.emit("thread.proposal", json!({"thread": proposal.thread_id, "proposal": proposal}));
}
