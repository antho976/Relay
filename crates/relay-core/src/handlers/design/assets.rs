//! `design.asset.add`: a picture read, sniffed and stored beside the store, unlocked; and
//! `design.asset.get`: a stored picture read back in pieces, for a client its file is out of
//! reach of (the phone). Pictures are named by their content, shared by every design, and swept
//! once nothing has named them for an undo window (`disk_sweep::sweep_design_assets`, B66).

use super::{get_design, live_design};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::{assert_actor_project, check_project};
use crate::picture;
use base64::Engine as _;
use relay_bus::design::{valid_src, MAX_ASSET_B64, MAX_ASSET_FILE, MAX_COORD};
use relay_bus::error::BusError;
use relay_bus::ops::design::{AssetAdd, AssetAddOut, AssetGet, AssetGetOut, ASSET_PIECE};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The node an asset name stands for, and its natural size: `design.asset.add`'s read phase,
/// done with the store unlocked.
struct StagedAsset {
    src: String,
    path: String,
    w: u32,
    h: u32,
}

/// Why the file `raw` names could not be read, as the caller can act on it (TM-15): a missing
/// file is not found, one the engine may not read is refused, anything else is the caller's to
/// look at, never an internal error. Every path quoted, so a stray space shows.
fn read_error(raw: &str, error: std::io::Error) -> BusError {
    match error.kind() {
        ErrorKind::NotFound => BusError::not_found("design.asset", format!("no file {raw:?}")),
        ErrorKind::PermissionDenied => BusError::refused("design.asset", format!("cannot read {raw:?}: permission denied"))
            .with_hint("make it readable by the Relay engine, or pass bytes_b64"),
        _ => BusError::invalid("design.asset", format!("cannot read {raw:?}: {error}")),
    }
}

/// The picture at `path` (as the caller wrote it, `raw`): a regular file of at most
/// [`MAX_ASSET_FILE`].
fn read_asset(raw: &str, path: &Path) -> Result<Vec<u8>, BusError> {
    let meta = std::fs::metadata(path).map_err(|e| read_error(raw, e))?;
    if !meta.is_file() {
        return Err(BusError::invalid("design.asset", format!("{raw:?} is not a regular file")));
    }
    let too_big = || BusError::invalid("design.asset", format!("{raw:?} is over {} MiB", MAX_ASSET_FILE >> 20));
    if meta.len() > MAX_ASSET_FILE {
        return Err(too_big());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    let file = std::fs::File::open(path).map_err(|e| read_error(raw, e))?;
    file.take(MAX_ASSET_FILE + 1).read_to_end(&mut bytes).map_err(|e| read_error(raw, e))?;
    if bytes.len() as u64 > MAX_ASSET_FILE {
        return Err(too_big());
    }
    Ok(bytes)
}

/// Where `raw` points: itself when absolute; for an agent, relative to its own checkout `root`
/// (canonical), as `file.*` reads a path (I-10). A path that leads out of the checkout, by `..`
/// or through a link, is refused: an agent that means a file elsewhere names it whole.
fn asset_path(raw: &str, root: Option<&Path>) -> Result<PathBuf, BusError> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let Some(root) = root else {
        return Err(BusError::invalid("design.asset", format!("{raw:?} is not an absolute path")));
    };
    let real = std::fs::canonicalize(root.join(path)).map_err(|e| read_error(raw, e))?;
    if !real.starts_with(root) {
        return Err(BusError::invalid("design.asset", format!("{raw:?} leads out of your worktree"))
            .with_hint("give a path inside your worktree, or an absolute one"));
    }
    Ok(real)
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
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()).map_err(|_| {
        let error = BusError::invalid("design.asset", "bytes_b64 is not base64");
        if compact.starts_with("data:") { error.with_hint("drop the data:…;base64, prefix") } else { error }
    })?;
    if bytes.len() > MAX_ASSET_B64 {
        return Err(too_big());
    }
    Ok(bytes)
}

/// Keep `bytes` as `src` in `dir`, made if missing (a name is its content's hash), durably and
/// never removed from under this answer by the design-assets sweep: [`picture::keep_swept`].
/// Slow for a large picture (it syncs it to disk): call it unlocked.
pub(crate) fn store_asset(dir: &Path, src: &str, bytes: &[u8]) -> Result<(), BusError> {
    std::fs::create_dir_all(dir).bus()?;
    picture::keep_swept(dir, src, bytes, &crate::disk_sweep::ASSET_FILES).map_err(crate::engine::internal)?;
    Ok(())
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
        let relative = p.path.as_deref().is_some_and(|path| Path::new(path).is_relative());
        let checkout = ctx.read(|conn| {
            let design = live_design(conn, p.design_id)?;
            check_project(conn, design.project_id, session_id, agent)?;
            // An agent's relative path is read from its own checkout, as file.* reads one.
            match session_id.filter(|_| relative) {
                Some(_) => crate::handlers::file::root_choice(conn, session_id, design.project_id, None).map(Some),
                None => Ok(None),
            }
        })?;
        let root = match checkout {
            Some((project, chosen)) => Some(crate::handlers::file::root_verify(&project, chosen, "design.asset")?),
            None => None,
        };
        let bytes = match (&p.path, &p.bytes_b64) {
            (Some(raw), None) => read_asset(raw, &asset_path(raw, root.as_deref())?)?,
            (None, Some(encoded)) => decode_asset(encoded)?,
            _ => return Err(BusError::invalid("design.asset", "give the picture as path or as bytes_b64: one of the two")),
        };
        let picture = picture::sniff(&bytes).ok_or_else(|| {
            BusError::invalid("design.asset", "not a PNG, JPEG, GIF, WebP or SVG picture whose size can be read")
        })?;
        // B65: a raster's header may claim any size; a side no image node could take is refused
        // here, before anything is written, rather than when the node is added.
        if f64::from(picture.w.max(picture.h)) > MAX_COORD {
            return Err(BusError::invalid("design.asset", format!(
                "the picture says it is {}×{} px; a side may be at most {}", picture.w, picture.h, MAX_COORD as u64,
            )).with_hint("scale it down first"));
        }
        let digest = {
            use sha2::Digest;
            crate::hex(&sha2::Sha256::digest(&bytes))
        };
        let src = format!("{}.{}", &digest[..32], picture.ext);
        debug_assert!(valid_src(&src));
        let dir = ctx.engine().store.design_assets_dir();
        store_asset(&dir, &src, &bytes)?;
        Ok(StagedAsset { path: dir.join(&src).display().to_string(), src, w: picture.w, h: picture.h })
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

    #[test]
    fn a_relative_path_is_an_agents_and_stays_in_its_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("wt/img")).unwrap();
        std::fs::write(root.join("wt/img/dot.png"), b"x").unwrap();
        std::fs::write(root.join("outside.png"), b"x").unwrap();
        std::os::unix::fs::symlink(root.join("outside.png"), root.join("wt/link.png")).unwrap();
        let wt = root.join("wt");
        assert_eq!(asset_path("img/dot.png", Some(&wt)).unwrap(), wt.join("img/dot.png"));
        assert_eq!(asset_path("img/../img/dot.png", Some(&wt)).unwrap(), wt.join("img/dot.png"));
        let abs = root.join("outside.png").display().to_string();
        assert_eq!(asset_path(&abs, Some(&wt)).unwrap(), root.join("outside.png"), "a whole path is read as given");
        for out in ["../outside.png", "link.png"] {
            let error = asset_path(out, Some(&wt)).unwrap_err();
            assert_eq!(error.code, "design.asset", "{out}");
            assert!(error.message.contains("leads out of your worktree"), "{out}: {}", error.message);
        }
        assert_eq!(asset_path("img/none.png", Some(&wt)).unwrap_err().kind, relay_bus::error::ErrorKind::NotFound);
        assert!(asset_path("img/dot.png", None).unwrap_err().message.contains("is not an absolute path"), "the person names a file whole");
    }
}
