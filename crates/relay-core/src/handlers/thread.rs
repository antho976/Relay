//! `thread.*` — Threads (docs/THREADS.md).
//!
//! Threads live in `threads.db` behind their own mutex (`crate::threads`), so reads run unlocked
//! and the mutations only store a row while the store's transaction is open. Starting the agent
//! and handing it a message happen after the handler returns.

use crate::engine::Engine;
use crate::{local_models, threads};
use relay_bus::ops::thread::*;
use relay_bus::Empty;
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
        let message = threads::post(ctx.engine(), thread.id, &text, &now)?;
        ctx.emit("thread.message", json!({"thread": thread.id, "message": message}));
        let id = thread.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, &text));
        threads::get(ctx.engine(), id)
    });
    e.register::<Send>(|ctx, p| {
        let text = threads::checked_text(&p.text)?;
        let now = ctx.now.clone();
        let message = threads::post(ctx.engine(), p.id, &text, &now)?;
        ctx.emit("thread.message", json!({"thread": p.id, "message": message}));
        let id = p.id;
        ctx.after_commit(move |engine| threads::deliver(&engine, id, &text));
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
}
