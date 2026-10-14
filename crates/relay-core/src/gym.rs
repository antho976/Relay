//! Avex's training history on the PC (docs/GYM.md): `gym.db` beside the store, opened on first
//! use (in memory when the store is, as in tests), and the history it holds, loaded once and kept.
//!
//! Nothing here touches the store or its lock. Reads take the shelf's mutex only to clone the
//! history's `Arc`, then read without it; an import holds it for the one transaction that
//! replaces the file.

use crate::engine::Engine;
use relay_bus::BusError;
use relay_gym::model::History;
use relay_gym::store::Gym;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
pub struct Shelf {
    opened: Mutex<Option<Opened>>,
}

struct Opened {
    gym: Gym,
    history: Arc<History>,
    imported: bool,
    /// Where the text of the last `gym.upload` is kept, beside `gym.db`; `None` in memory.
    raw_path: Option<PathBuf>,
    /// That text when the store is in memory (tests), where there is no file to keep it in.
    raw_memory: Option<String>,
}

fn bus(e: rusqlite::Error) -> BusError {
    BusError::internal(format!("gym.db: {e}"))
}

fn opened(engine: &Engine) -> Result<MutexGuard<'_, Option<Opened>>, BusError> {
    let mut slot = engine.gym.opened.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        let store = engine.store.path();
        let in_memory = store == Path::new(":memory:");
        let gym = if in_memory { Gym::open_in_memory() } else {
            let path = store.with_file_name("gym.db");
            Gym::open(&path).inspect(|_| crate::private::private_db(&path))
        }.map_err(bus)?;
        let history = Arc::new(gym.load().map_err(bus)?);
        let imported = gym.imported().map_err(bus)?;
        let raw_path = (!in_memory).then(|| store.with_file_name("gym-upload.json"));
        *slot = Some(Opened { gym, history, imported, raw_path, raw_memory: None });
    }
    Ok(slot)
}

/// The history, and whether an export was ever imported.
pub fn history(engine: &Engine) -> Result<(Arc<History>, bool), BusError> {
    let slot = opened(engine)?;
    let o = slot.as_ref().expect("opened above");
    Ok((o.history.clone(), o.imported))
}

/// Replaces the PC's copy with `h`. An upload kept from before no longer describes the copy, so
/// it goes.
pub fn replace(engine: &Engine, h: History) -> Result<(), BusError> {
    let mut slot = opened(engine)?;
    let o = slot.as_mut().expect("opened above");
    o.raw_memory = None;
    if let Some(path) = &o.raw_path {
        let _ = std::fs::remove_file(path);
    }
    o.gym.replace(&h).map_err(bus)?;
    o.history = Arc::new(h);
    o.imported = true;
    Ok(())
}

/// Replaces the PC's copy with `h`, read from `text`, and keeps `text` for `gym.raw`: what a
/// cloud copy (docs/CLOUD.md) hands the PC's link, which imports it as it came.
pub fn replace_uploaded(engine: &Engine, h: History, text: &str) -> Result<(), BusError> {
    let mut slot = opened(engine)?;
    let o = slot.as_mut().expect("opened above");
    match &o.raw_path {
        Some(path) => {
            // Renamed in, so a reader never finds half a file; the user's alone, like gym.db.
            crate::private::write_private_replace(path, text.as_bytes())
                .map_err(|e| BusError::internal(format!("{}: {e}", path.display())))?;
        }
        None => o.raw_memory = Some(text.to_string()),
    }
    o.gym.replace(&h).map_err(bus)?;
    o.history = Arc::new(h);
    o.imported = true;
    Ok(())
}

/// The text of the last upload, if one was kept.
pub fn uploaded(engine: &Engine) -> Result<Option<String>, BusError> {
    let slot = opened(engine)?;
    let o = slot.as_ref().expect("opened above");
    match &o.raw_path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BusError::internal(format!("{}: {e}", path.display()))),
        },
        None => Ok(o.raw_memory.clone()),
    }
}

/// Forgets the copy, and the upload it came from.
pub fn clear(engine: &Engine) -> Result<(), BusError> {
    let mut slot = opened(engine)?;
    let o = slot.as_mut().expect("opened above");
    o.gym.clear().map_err(bus)?;
    o.history = Arc::new(History::default());
    o.imported = false;
    o.raw_memory = None;
    if let Some(path) = &o.raw_path {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(BusError::internal(format!("{}: {e}", path.display()))),
        }
    }
    Ok(())
}
