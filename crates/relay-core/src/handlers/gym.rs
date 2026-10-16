//! `gym.*` — Avex's training history (docs/GYM.md). The shelf (`crate::gym`) holds `gym.db` and
//! the loaded history; these handlers read it through `relay_gym::views`.
//!
//! Every read is `register_unlocked`: the history is its own file, never the store. The import
//! is staged, reading and parsing the file and replacing `gym.db` in `prepare`, outside the
//! store's lock; as with Tally's ledger, that write is kept whether or not the store
//! transaction around `finish` commits.

use crate::engine::{Ctx, Engine};
use crate::gym;
use relay_bus::ops::gym::*;
use relay_bus::{BusError, Empty};
use relay_gym::export::{self, ReadError};
use relay_gym::model::History;
use relay_gym::views::{self, Clock};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

fn invalid(m: impl Into<String>) -> BusError {
    BusError::invalid("gym.invalid", m)
}

/// Runs `f` on the history, the PC's clock and whether anything was imported.
fn read<T>(engine: &Engine, f: impl FnOnce(&History, &Clock, bool) -> T) -> Result<T, BusError> {
    read_on(engine, Clock::system(), f)
}

fn read_on<T>(engine: &Engine, clock: Clock, f: impl FnOnce(&History, &Clock, bool) -> T) -> Result<T, BusError> {
    let (h, imported): (Arc<History>, bool) = gym::history(engine)?;
    Ok(f(&h, &clock, imported))
}

/// The PC's clock, on `day` when one is given.
fn clock_on(day: Option<&str>) -> Result<Clock, BusError> {
    let mut clock = Clock::system();
    if let Some(d) = day {
        clock.today = d.trim().parse().map_err(|_| invalid(format!("Not a date: {d} (YYYY-MM-DD)")))?;
    }
    Ok(clock)
}

fn file(path: &str) -> Result<(String, String), BusError> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(BusError::invalid("gym.path", "Give the file's full path"));
    }
    let size = std::fs::metadata(p).map_err(|e| BusError::invalid("gym.path", format!("{}: {e}", p.display())))?.len();
    if size > export::MAX_BYTES {
        return Err(BusError::invalid("gym.import", "That file is too big to be an Avex export"));
    }
    let text = std::fs::read_to_string(p).map_err(|e| BusError::invalid("gym.path", format!("{}: {e}", p.display())))?;
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((text, name))
}

/// An export's history, or the refusal that says which file to send instead.
fn parse(text: &str, name: &str) -> Result<History, BusError> {
    export::read(text, name, &crate::time::now()).map_err(|e| match e {
        ReadError::TooNew(_) => BusError::refused("gym.too_new", e.to_string()),
        _ => BusError::invalid("gym.import", e.to_string()),
    })
}

fn counts(h: &History) -> ImportOut {
    ImportOut {
        sessions: h.sessions.len() as u32,
        sets: h.sessions.iter().flat_map(|s| &s.exercises).map(|e| e.sets.len() as u32).sum(),
        cardio: h.cardio.len() as u32,
        goals: h.goals.len() as u32,
        exported_at: h.meta.exported_at.clone(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<SummaryOp>(|ctx, p| read_on(ctx.engine(), clock_on(p.today.as_deref())?, views::summary));
    e.register_unlocked::<Sessions>(|ctx, p| {
        let limit = p.limit.unwrap_or(50).clamp(1, 500) as usize;
        let (sessions, total) =
            read(ctx.engine(), |h, c, _| views::sessions(h, c, p.offset.unwrap_or(0) as usize, limit, p.lift.as_deref()))?.map_err(invalid)?;
        Ok(SessionsOut { sessions, total: total as u32 })
    });
    e.register_unlocked::<SessionGet>(|ctx, p| {
        read(ctx.engine(), |h, c, _| views::session(h, c, p.id))?.ok_or_else(|| BusError::not_found("gym.not_found", format!("No workout {} in Avex's history", p.id)))
    });
    e.register_unlocked::<Lifts>(|ctx, p| {
        read_on(ctx.engine(), clock_on(p.today.as_deref())?, |h, c, _| LiftsOut {
            unit: if h.meta.use_kg { "kg".into() } else { "lb".into() },
            lifts: views::lifts(h, c, p.query.as_deref()),
        })
    });
    e.register_unlocked::<LiftGet>(|ctx, p| {
        let limit = p.limit.unwrap_or(100).clamp(1, 2000) as usize;
        read_on(ctx.engine(), clock_on(p.today.as_deref())?, |h, c, _| views::lift(h, c, &p.name, limit))?.map_err(|m| BusError::not_found("gym.not_found", m))
    });
    e.register_unlocked::<Cardio>(|ctx, p| {
        let limit = p.limit.unwrap_or(50).clamp(1, 1000) as usize;
        read(ctx.engine(), |h, c, _| CardioOut { entries: views::cardio(h, c, limit) })
    });
    e.register_unlocked::<SeriesOp>(|ctx, p| read(ctx.engine(), |h, c, _| views::series(h, c, &p))?.map_err(invalid));
    e.register_staged::<Import, _>(
        |ctx, p| {
            let (text, name) = file(&p.path)?;
            let h = parse(&text, &name)?;
            let out = counts(&h);
            gym::replace(ctx.engine(), h)?;
            Ok(out)
        },
        |ctx: &mut Ctx, _, out: ImportOut| {
            ctx.emit("gym.changed", json!({"imported": out.sessions}));
            Ok(out)
        },
    );
    e.register_staged::<Upload, _>(
        |ctx, p| {
            if p.json.len() as u64 > export::MAX_BYTES {
                return Err(BusError::invalid("gym.import", "That is too big to be an Avex export"));
            }
            let name = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("avex_export.json");
            let h = parse(&p.json, name)?;
            let out = counts(&h);
            gym::replace_uploaded(ctx.engine(), h, &p.json)?;
            Ok(out)
        },
        |ctx: &mut Ctx, _, out: ImportOut| {
            ctx.emit("gym.changed", json!({"imported": out.sessions}));
            Ok(out)
        },
    );
    e.register_unlocked::<Raw>(|ctx, p| {
        let Some(text) = gym::uploaded(ctx.engine())? else {
            return Ok(RawOut { digest: None, json: None });
        };
        let digest = hex(&Sha256::digest(text.as_bytes()));
        let json = (p.have.as_deref() != Some(digest.as_str())).then_some(text);
        Ok(RawOut { digest: Some(digest), json })
    });
    e.register_staged::<Reset, _>(
        |ctx, _| gym::clear(ctx.engine()),
        |ctx: &mut Ctx, _, ()| {
            ctx.emit("gym.changed", json!({}));
            Ok(Empty {})
        },
    );
}
