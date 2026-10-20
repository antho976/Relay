//! `design.asset.add`: a picture read, sniffed and stored beside the store, unlocked.

use super::live_design;
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{assert_actor_project, check_project};
use crate::picture;
use base64::Engine as _;
use relay_bus::error::BusError;
use relay_bus::ops::design::{AssetAdd, AssetAddOut};
use std::io::{Read, Write};
use std::path::Path;

/// The largest picture `design.asset.add` copies in from a path, and takes as `bytes_b64`.
const ASSET_FILE_MAX: u64 = 20 * 1024 * 1024;
const ASSET_B64_MAX: usize = 1024 * 1024;

/// The node an asset name stands for, and its natural size: `design.asset.add`'s read phase,
/// done with the store unlocked.
struct StagedAsset {
    src: String,
    path: String,
    w: u32,
    h: u32,
}

/// The picture at `raw`: an absolute path to a regular file of at most [`ASSET_FILE_MAX`].
fn read_asset(raw: &str) -> Result<Vec<u8>, BusError> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(BusError::invalid("design.asset", format!("{raw:?} is not an absolute path")));
    }
    let meta = std::fs::metadata(path).map_err(|_| BusError::not_found("design.asset", format!("no file {raw}")))?;
    if !meta.is_file() {
        return Err(BusError::invalid("design.asset", format!("{raw} is not a regular file")));
    }
    let too_big = || BusError::invalid("design.asset", format!("{raw} is over {} MiB", ASSET_FILE_MAX >> 20));
    if meta.len() > ASSET_FILE_MAX {
        return Err(too_big());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    std::fs::File::open(path).bus()?.take(ASSET_FILE_MAX + 1).read_to_end(&mut bytes).bus()?;
    if bytes.len() as u64 > ASSET_FILE_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

/// `bytes_b64` decoded (line breaks allowed), at most [`ASSET_B64_MAX`] bytes.
fn decode_asset(encoded: &str) -> Result<Vec<u8>, BusError> {
    let compact: std::borrow::Cow<str> = if encoded.bytes().any(|b| b.is_ascii_whitespace()) {
        encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect::<String>().into()
    } else {
        encoded.into()
    };
    let too_big = || BusError::invalid("design.asset", format!("bytes_b64 is over {} KiB decoded", ASSET_B64_MAX >> 10))
        .with_hint("pass a larger picture as a path");
    if compact.len() > ASSET_B64_MAX.div_ceil(3) * 4 {
        return Err(too_big());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
        .map_err(|_| BusError::invalid("design.asset", "bytes_b64 is not base64"))?;
    if bytes.len() > ASSET_B64_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

/// Write `bytes` to `path` in `dir` unless it is there already (a name is its content's hash),
/// through a temporary file renamed into place, so a reader never sees half a picture.
fn store_asset(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), BusError> {
    if path.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).bus()?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).bus()?;
    tmp.write_all(bytes).bus()?;
    match tmp.persist_noclobber(path) {
        Ok(_) => Ok(()),
        // Another request stored the same picture first.
        Err(_) if path.is_file() => Ok(()),
        Err(error) => Err(crate::engine::internal(error.error)),
    }
}

pub(super) fn register(e: &mut Engine) {
    // Staged (D149): the picture is read, sniffed and written beside the store with the lock
    // released — a path may name a 20 MiB file — and the transaction only checks the design.
    e.register_staged::<AssetAdd, StagedAsset>(|ctx, p| {
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        ctx.read(|conn| {
            let design = live_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)
        })?;
        let bytes = match (&p.path, &p.bytes_b64) {
            (Some(path), None) => read_asset(path)?,
            (None, Some(encoded)) => decode_asset(encoded)?,
            _ => return Err(BusError::invalid("design.asset", "give the picture as path or as bytes_b64: one of the two")),
        };
        let picture = picture::sniff(&bytes).ok_or_else(|| {
            BusError::invalid("design.asset", "not a PNG, JPEG, GIF, WebP or SVG picture whose size can be read")
        })?;
        let digest = {
            use sha2::Digest;
            crate::hex(&sha2::Sha256::digest(&bytes))
        };
        let src = format!("{}.{}", &digest[..32], picture.ext);
        debug_assert!(relay_bus::design::valid_src(&src));
        let dir = ctx.engine().store.design_assets_dir();
        let path = dir.join(&src);
        store_asset(&dir, &path, &bytes)?;
        Ok(StagedAsset { src, path: path.display().to_string(), w: picture.w, h: picture.h })
    }, |ctx: &mut Ctx, p, staged| {
        let design = live_design(ctx.tx(), p.design_id)?;
        assert_actor_project(ctx, design.project_id)?;
        ctx.set_project(design.project_id);
        Ok(AssetAddOut { src: staged.src, path: staged.path, w: staged.w, h: staged.h })
    });
}
