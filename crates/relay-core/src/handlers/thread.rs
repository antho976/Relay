//! `thread.*` — Threads (docs/THREADS.md).
//!
//! Threads live in `threads.db` behind their own mutex (`crate::threads`), so reads run unlocked
//! and the mutations only store a row while the store's transaction is open. Starting the agent
//! and handing it a message happen after the handler returns.

use crate::engine::{Ctx, Engine};
use crate::threads::{asks, proposals};
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
    // A message's pictures are only looked up here, in `threads.db`; the agent is handed their
    // bytes after the handler (`threads::deliver`).
    e.register::<Create>(|ctx, p| {
        let pictures = p.images.unwrap_or_default();
        // Checked before the thread is made, so a refused first message leaves no thread behind.
        let first = if p.text.is_some() || !pictures.is_empty() {
            let (model, _) = threads::checked_agent(p.model.as_deref(), p.effort.as_deref())?;
            Some(threads::checked_message(ctx.engine(), p.text.as_deref().unwrap_or(""), &pictures, model.as_deref())?)
        } else {
            None
        };
        let title = match &first {
            Some((text, _)) if !text.is_empty() => threads::title_from(text),
            Some((_, images)) => threads::images::title(images.len()),
            None => "New thread".to_string(),
        };
        let now = ctx.now.clone();
        let thread = threads::create(ctx.engine(), &title, p.model.as_deref(), p.effort.as_deref(), &now)?;
        ctx.emit("thread.changed", json!({"thread": thread}));
        let Some((text, images)) = first else { return Ok(thread) };
        let (message, turn) = threads::post(ctx.engine(), thread.id, &text, &images, &now)?;
        ctx.emit("thread.message", json!({"thread": thread.id, "message": message}));
        let id = thread.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, turn, &text, &images));
        threads::get(ctx.engine(), id)
    });
    e.register::<Send>(|ctx, p| {
        let pictures = p.images.unwrap_or_default();
        let model = if pictures.is_empty() { None } else { threads::get(ctx.engine(), p.id)?.model };
        let (text, images) = threads::checked_message(ctx.engine(), &p.text, &pictures, model.as_deref())?;
        let now = ctx.now.clone();
        let (message, turn) = threads::post(ctx.engine(), p.id, &text, &images, &now)?;
        ctx.emit("thread.message", json!({"thread": p.id, "message": message}));
        let id = p.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, turn, &text, &images));
        Ok(message)
    });
    // Decoding and writing the picture, and sweeping old uploads, are file work: none of it
    // holds the store's lock, and the transaction has nothing to do.
    e.register_staged::<Upload, _>(|ctx, p| {
        let now = ctx.now.clone();
        threads::images::upload(ctx.engine(), &p.media_type, &p.bytes_b64, &now)
    }, |_ctx: &mut Ctx, _p, image: ImageView| Ok(UploadOut { image }));
    e.register_unlocked::<Image>(|ctx, p| {
        use base64::Engine as _;
        let (image, bytes) = threads::images::read(ctx.engine(), &p.id)?;
        Ok(ImageOut { media_type: image.media_type, bytes_b64: base64::engine::general_purpose::STANDARD.encode(bytes) })
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
        let files = threads::delete(ctx.engine(), p.id)?;
        ctx.emit("thread.changed", json!({"id": p.id, "deleted": true}));
        // Its pictures' files go once the lock is released.
        if !files.is_empty() {
            ctx.after_commit(move |_| threads::images::remove(&files));
        }
        Ok(Empty {})
    });
    register_proposals(e);
    register_asks(e);
}

/// Full access (`threads::asks`). The access and the answers are short writes to `threads.db`;
/// the agent is told its answer after the handler, as a message is handed on.
fn register_asks(e: &mut Engine) {
    e.register::<Access>(|ctx, p| {
        person(ctx)?;
        asks::set_access(ctx.engine(), p.id, p.access.trim())?;
        let thread = threads::get(ctx.engine(), p.id)?;
        ctx.emit("thread.changed", json!({"thread": thread}));
        Ok(thread)
    });
    e.register_unlocked::<AskList>(|ctx, p| Ok(AsksOut { asks: asks::list(ctx.engine(), p.thread_id, p.pending.unwrap_or(false))? }));
    e.register::<AskAnswer>(|ctx, p| {
        person(ctx)?;
        let now = ctx.now.clone();
        let (ask, told) = asks::answer(ctx.engine(), p.id, p.allow, p.scope.as_deref(), p.message.as_deref(), &now)?;
        for other in asks::others(ctx.engine(), ask.id, &told) {
            ctx.emit("thread.ask", json!({"thread": other.thread_id, "ask": other}));
        }
        ctx.emit("thread.ask", json!({"thread": ask.thread_id, "ask": ask}));
        ctx.emit("thread.changed", json!({"thread": threads::get(ctx.engine(), ask.thread_id)?}));
        let message = ask.message.clone();
        ctx.after_commit(move |engine| asks::deliver(&engine, &told, message.as_deref()));
        Ok(ask)
    });
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

/// An answer is the person's: a thread's agent never answers a proposal or an ask, its own or
/// another's, nor sets a thread's access, whatever its door lets through.
fn person(ctx: &Ctx) -> Result<(), BusError> {
    match ctx.thread() {
        Some(thread) => Err(BusError::refused("thread.not_person", format!("Thread {thread}'s agent cannot answer for the person: the person does"))),
        None => Ok(()),
    }
}

fn answered(ctx: &mut Ctx, proposal: &ProposalView) {
    if matches!(proposal.status.as_str(), "approved" | "undone") {
        ctx.emit("money.changed", json!({"proposal": proposal.id}));
    }
    ctx.emit("thread.proposal", json!({"thread": proposal.thread_id, "proposal": proposal}));
}
