//! `design.asset.add`: a picture read, sniffed and stored beside the store, unlocked; and
//! `design.asset.get`: a stored picture read back in pieces, for a client its file is out of
//! reach of (the phone).

use super::{get_design, live_design};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{assert_actor_project, check_project};
use crate::picture;
use base64::Engine as _;
use relay_bus::design::{valid_src, MAX_ASSET_B64, MAX_ASSET_FILE};
use relay_bus::error::BusError;
use relay_bus::ops::design::{AssetAdd, AssetAddOut, AssetGet, AssetGetOut, ASSET_PIECE};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// The node an asset name stands for, and its natural size: `design.asset.add`'s read phase,
/// done with the store unlocked.
struct StagedAsset {
    src: String,
    path: String,
    w: u32,
    h: u32,
}

/// The picture at `raw`: an absolute path to a regular file of at most [`MAX_ASSET_FILE`].
fn read_asset(raw: &str) -> Result<Vec<u8>, BusError> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(BusError::invalid("design.asset", format!("{raw:?} is not an absolute path")));
    }
    let meta = std::fs::metadata(path).map_err(|_| BusError::not_found("design.asset", format!("no file {raw}")))?;
    if !meta.is_file() {
        return Err(BusError::invalid("design.asset", format!("{raw} is not a regular file")));
    }
    let too_big = || BusError::invalid("design.asset", format!("{raw} is over {} MiB", MAX_ASSET_FILE >> 20));
    if meta.len() > MAX_ASSET_FILE {
        return Err(too_big());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    std::fs::File::open(path).bus()?.take(MAX_ASSET_FILE + 1).read_to_end(&mut bytes).bus()?;
    if bytes.len() as u64 > MAX_ASSET_FILE {
        return Err(too_big());
    }
    Ok(bytes)
}

/// `bytes_b64` decoded (line breaks allowed), at most [`MAX_ASSET_B64`] bytes.
fn decode_asset(encoded: &str) -> Result<Vec<u8>, BusError> {
    let compact: std::borrow::Cow<str> = if encoded.bytes().any(|b| b.is_ascii_whitespace()) {
        encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect::<String>().into()
    } else {
        encoded.into()
    };
    let too_big = || BusError::invalid("design.asset", format!("bytes_b64 is over {} KiB decoded", MAX_ASSET_B64 >> 10))
        .with_hint("pass a larger picture as a path");
    if compact.len() > MAX_ASSET_B64.div_ceil(3) * 4 {
        return Err(too_big());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
        .map_err(|_| BusError::invalid("design.asset", "bytes_b64 is not base64"))?;
    if bytes.len() > MAX_ASSET_B64 {
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

/// A stored picture's media type, from its name's extension (`valid_src` names one of five).
fn media_type(src: &str) -> &'static str {
    match src.rsplit_once('.').map_or("", |(_, ext)| ext) {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/svg+xml",
    }
}

/// The piece of stored picture `src` that starts at `offset`: at most [`ASSET_PIECE`] bytes,
/// and where the next one starts.
fn read_piece(dir: &Path, src: &str, offset: u64) -> Result<AssetGetOut, BusError> {
    let missing = || BusError::not_found("design.asset_not_found", format!("no picture {src} on this machine"))
        .with_hint("design.asset.add stores it again from its file");
    let mut file = match std::fs::File::open(dir.join(src)) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(missing()),
        Err(error) => return Err(crate::engine::internal(error)),
    };
    let meta = file.metadata().bus()?;
    // An empty file at a picture's name is one a crash cut short: there is no picture to give.
    if !meta.is_file() || meta.len() == 0 {
        return Err(missing());
    }
    let size = meta.len();
    if offset >= size {
        return Err(BusError::invalid("design.asset", format!("offset {offset} is at or past the end of {src} ({size} bytes)"))
            .with_hint("start at 0, then at each piece's next"));
    }
    file.seek(SeekFrom::Start(offset)).bus()?;
    let mut bytes = Vec::with_capacity((size - offset).min(ASSET_PIECE as u64) as usize);
    file.take(ASSET_PIECE as u64).read_to_end(&mut bytes).bus()?;
    // A file that shrank under the read would otherwise answer an empty piece that names
    // itself as the next, forever.
    if bytes.is_empty() {
        return Err(missing());
    }
    let end = offset + bytes.len() as u64;
    Ok(AssetGetOut {
        media_type: media_type(src).to_string(),
        size,
        bytes_b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        next: (end < size).then_some(end),
    })
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
        debug_assert!(valid_src(&src));
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
    // Unlocked (D149): one short read to check the design, then the file, which may be 20 MiB,
    // read with nothing held. A deleted design's pictures still read, as design.get reads it.
    e.register_unlocked::<AssetGet>(|ctx, p| {
        if !valid_src(&p.src) {
            return Err(BusError::invalid("design.asset", format!("src {:?} is not a design picture's name (design.asset.add gives one)", p.src)));
        }
        let (session_id, agent) = (ctx.actor_session_id(), ctx.actor.is_agent());
        ctx.read(|conn| {
            let design = get_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)
        })?;
        read_piece(&ctx.engine().store.design_assets_dir(), &p.src, p.offset.unwrap_or(0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_picture_names_its_media_type() {
        for (src, media) in [("a.png", "image/png"), ("a.jpg", "image/jpeg"), ("a.gif", "image/gif"), ("a.webp", "image/webp"), ("a.svg", "image/svg+xml")] {
            assert_eq!(media_type(src), media);
            assert_eq!(picture::raster_ext(media).unwrap_or("svg"), src.trim_start_matches("a."), "the names asset.add stores under");
        }
    }
}
