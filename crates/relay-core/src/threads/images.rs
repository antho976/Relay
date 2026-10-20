//! Pictures in threads (docs/THREADS.md, "Pictures"): a receipt, a screenshot, a statement the
//! person shows the agent.
//!
//! `thread.upload` writes one under `threads/images/<id>.<ext>` beside `threads.db`, with the
//! store lock never taken, and `threads.db` keeps its row (`image`) and which messages carry it
//! (`message_image`). A message names its pictures by id; the agent is handed their bytes when
//! the message reaches it ([`super::deliver`], after the handler), and a client draws them with
//! `thread.image`. An upload no message carries is removed after [`UNSENT`] (swept when another
//! is uploaded), and deleting a thread removes the pictures no other message carries.
//!
//! A model on this PC reads no pictures: a message with one is refused in its thread.

use super::{agent_dir, with_db, ThreadStore};
use crate::engine::Engine;
use relay_bus::ops::thread::ImageView;
use relay_bus::types::Id;
use relay_bus::BusError;
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// The most pictures one message carries.
pub const MAX: usize = 4;
/// The most pictures, and bytes of them, one thread carries. Every turn hands the model the whole
/// conversation, pictures and all, and the API refuses a request of more than 32 MB (these, in
/// base64, come to 27 MB), or a picture over 2000 px a side once a request holds more than 20.
/// A refused request is refused again on every later turn.
pub const THREAD_MAX: usize = 20;
pub const THREAD_BYTES: u64 = 20 << 20;
/// How long an upload no message carries is kept.
pub const UNSENT: Duration = Duration::from_secs(24 * 60 * 60);

pub(super) const SCHEMA: &str = "
CREATE TABLE image (
    id TEXT PRIMARY KEY,
    media_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE message_image (
    message_id INTEGER NOT NULL REFERENCES message(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES image(id),
    PRIMARY KEY (message_id, image_id)
);
CREATE INDEX message_image_image ON message_image(image_id);
";

/// What a thread on a model on this PC answers a message with a picture.
pub const UNSUPPORTED: &str =
    "This thread runs on a model on this PC, which cannot read pictures: choose a Claude model for it to send them.";

/// `threads/images` beside `threads.db` (beside the store; in the agents' folder in a temporary
/// directory when the store is in memory, as in tests).
pub fn dir(engine: &Engine) -> PathBuf {
    agent_dir(engine).join("images")
}

/// Where picture `image` is kept.
pub fn path(engine: &Engine, image: &ImageView) -> PathBuf {
    let ext = crate::picture::raster_ext(&image.media_type).unwrap_or("bin");
    dir(engine).join(format!("{}.{ext}", image.id))
}

/// 32 lowercase hex characters, as [`upload`] names a picture.
pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A picture's place in a message's stored body: `{"id", "media_type", "width", "height"}`.
pub fn body(image: &ImageView) -> serde_json::Value {
    serde_json::json!({"id": image.id, "media_type": image.media_type, "width": image.width, "height": image.height})
}

/// The name of a thread its first message's pictures start without words.
pub fn title(count: usize) -> String {
    if count == 1 { "Picture".into() } else { format!("{count} pictures") }
}

fn row(row: &rusqlite::Row) -> rusqlite::Result<ImageView> {
    Ok(ImageView { id: row.get(0)?, media_type: row.get(1)?, width: row.get(2)?, height: row.get(3)?, bytes: row.get::<_, i64>(4)?.max(0) as u64 })
}

impl ThreadStore {
    fn add_image(&self, image: &ImageView, now: &str) -> rusqlite::Result<()> {
        self.conn
            .prepare_cached("INSERT INTO image (id, media_type, width, height, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?
            .execute(params![image.id, image.media_type, image.width, image.height, image.bytes as i64, now])?;
        Ok(())
    }

    pub fn image(&self, id: &str) -> rusqlite::Result<Option<ImageView>> {
        self.conn.prepare_cached("SELECT id, media_type, width, height, bytes FROM image WHERE id = ?1")?.query_row([id], row).optional()
    }

    /// Mark message `message` as carrying `images`; the first of them that is gone (swept as
    /// unsent since it was looked up) when one is, and the caller rolls the message back.
    pub(super) fn carry(&self, message: Id, images: &[ImageView]) -> rusqlite::Result<Option<String>> {
        let mut stmt = self.conn.prepare_cached("INSERT OR IGNORE INTO message_image (message_id, image_id) SELECT ?1, id FROM image WHERE id = ?2")?;
        for image in images {
            if stmt.execute(params![message, image.id])? == 0 {
                return Ok(Some(image.id.clone()));
            }
        }
        Ok(None)
    }

    /// How many pictures thread `id`'s messages carry, and their bytes, counting `adding` too
    /// where none of them carries it yet.
    fn carried_size(&self, id: Id, adding: &[ImageView]) -> rusqlite::Result<(usize, u64)> {
        let carried: HashSet<String> = self.carried_by(id)?.into_iter().collect();
        let mut count = carried.len();
        let mut bytes: u64 = self.conn
            .prepare_cached("SELECT COALESCE(SUM(bytes), 0) FROM image WHERE id IN (SELECT DISTINCT mi.image_id FROM message_image mi JOIN message m ON m.id = mi.message_id WHERE m.thread_id = ?1)")?
            .query_row([id], |r| r.get::<_, i64>(0))?.max(0) as u64;
        for image in adding.iter().filter(|image| !carried.contains(&image.id)) {
            count += 1;
            bytes += image.bytes;
        }
        Ok((count, bytes))
    }

    /// The pictures thread `id`'s messages carry.
    pub(super) fn carried_by(&self, id: Id) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT mi.image_id FROM message_image mi JOIN message m ON m.id = mi.message_id WHERE m.thread_id = ?1",
        )?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect()
    }

    /// Forget those of `ids` no message carries any more, and say which they were.
    pub(super) fn forget_uncarried(&self, ids: &[String]) -> rusqlite::Result<Vec<ImageView>> {
        let mut gone = Vec::new();
        for id in ids {
            let uncarried = self.conn
                .prepare_cached("SELECT id, media_type, width, height, bytes FROM image i WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM message_image WHERE image_id = i.id)")?
                .query_row([id], row)
                .optional()?;
            if let Some(image) = uncarried {
                self.conn.prepare_cached("DELETE FROM image WHERE id = ?1")?.execute([id])?;
                gone.push(image);
            }
        }
        Ok(gone)
    }

    /// Forget the uploads no message carries that were made before `cutoff`, and say which.
    fn forget_unsent(&self, cutoff: &str) -> rusqlite::Result<Vec<ImageView>> {
        let tx = self.conn.unchecked_transaction()?;
        let ids: Vec<String> = {
            let mut stmt = self.conn.prepare_cached(
                "SELECT id FROM image i WHERE created_at < ?1 AND NOT EXISTS (SELECT 1 FROM message_image WHERE image_id = i.id)",
            )?;
            let rows = stmt.query_map([cutoff], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let gone = self.forget_uncarried(&ids)?;
        tx.commit()?;
        Ok(gone)
    }

    fn image_ids(&self) -> rusqlite::Result<HashSet<String>> {
        let mut stmt = self.conn.prepare_cached("SELECT id FROM image")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }
}

fn upload_error(message: &str) -> BusError {
    BusError::invalid("thread.upload", message)
}

/// Keep a picture for a thread's next message (`thread.upload`): `media_type` and `bytes_b64` as
/// sent, checked, then written under [`dir`]. File work only, so it runs with the store unlocked.
pub fn upload(engine: &Engine, media_type: &str, bytes_b64: &str, now: &str) -> Result<ImageView, BusError> {
    if crate::picture::raster_ext(media_type).is_none() {
        return Err(upload_error("A picture is a PNG, JPEG, GIF or WebP (image/png, image/jpeg, image/gif or image/webp)"));
    }
    let bytes = crate::picture::decode_upload(bytes_b64, "thread.upload")?;
    // The bytes decide what it is: a model is told the type its bytes are.
    let picture = crate::picture::raster(&bytes).ok_or_else(|| upload_error("not a PNG, JPEG, GIF or WebP picture whose size can be read"))?;
    if !crate::picture::whole(&bytes, &picture) {
        return Err(upload_error("The picture is cut short: it ends before its image does").with_hint("copy or save it again, whole"));
    }
    if picture.w.max(picture.h) > crate::picture::MAX_SIDE {
        return Err(upload_error(&format!("The picture is {} by {} px; a model takes at most {} px a side", picture.w, picture.h, crate::picture::MAX_SIDE))
            .with_hint("send it smaller"));
    }
    sweep(engine, now);
    let image = ImageView {
        id: uuid::Uuid::new_v4().simple().to_string(),
        media_type: picture.media_type().into(),
        width: picture.w,
        height: picture.h,
        bytes: bytes.len() as u64,
    };
    let dir = dir(engine);
    std::fs::create_dir_all(&dir).map_err(crate::engine::internal)?;
    let file = path(engine, &image);
    crate::private::write_private_replace(&file, &bytes).map_err(crate::engine::internal)?;
    if let Err(e) = with_db(engine, |db| db.add_image(&image, now)) {
        let _ = std::fs::remove_file(&file);
        return Err(e);
    }
    Ok(image)
}

/// Remove what an upload left that no message carries, made over [`UNSENT`] before `now`: rows and
/// their files, and files with no row (a write cut short). A failure is logged: the upload goes on.
fn sweep(engine: &Engine, now: &str) {
    let cutoff = now.parse::<jiff::Timestamp>().ok()
        .and_then(|now| now.checked_sub(jiff::SignedDuration::from_secs(UNSENT.as_secs() as i64)).ok())
        .map_or_else(|| crate::time::days_ago(1), crate::time::stamp);
    match with_db(engine, |db| db.forget_unsent(&cutoff)) {
        Ok(gone) => remove(&gone.iter().map(|image| path(engine, image)).collect::<Vec<_>>()),
        Err(e) => tracing::warn!("sweeping unsent thread pictures failed: {}", e.message),
    }
    let dir = dir(engine);
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    // Without the rows to tell what is kept, nothing is removed.
    let Ok(known) = with_db(engine, |db| db.image_ids()) else { return };
    let old = SystemTime::now().checked_sub(UNSENT);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let kept = name.to_str().and_then(|n| n.split_once('.')).is_some_and(|(id, _)| known.contains(id));
        let Ok(meta) = entry.metadata() else { continue };
        let stale = old.is_some_and(|old| meta.modified().is_ok_and(|at| at < old));
        if !kept && meta.is_file() && stale {
            remove(&[entry.path()]);
        }
    }
}

/// Remove picture files, once nothing carries them; one already gone is fine.
pub fn remove(files: &[PathBuf]) {
    for file in files {
        if let Err(e) = std::fs::remove_file(file) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(path = %file.display(), "removing a thread picture failed: {e}");
            }
        }
    }
}

/// A message's pictures as given: each named once, at most [`MAX`].
pub fn checked_ids(ids: &[String]) -> Result<Vec<String>, BusError> {
    let mut seen = HashSet::new();
    let ids: Vec<String> = ids.iter().map(|id| id.trim().to_string()).filter(|id| seen.insert(id.clone())).collect();
    if ids.len() > MAX {
        return Err(BusError::invalid("thread.invalid", format!("A message carries at most {MAX} pictures")));
    }
    Ok(ids)
}

/// Pictures may go to a thread on `model` unless it is one on this PC, which reads none.
pub fn readable(model: Option<&str>) -> Result<(), BusError> {
    if model.and_then(crate::local_models::find).is_some() {
        return Err(BusError::refused("thread.images_unsupported", UNSUPPORTED).with_hint("pick a Claude model with thread.set, or send the words alone"));
    }
    Ok(())
}

/// Whether thread `id` has room for `adding` beside the pictures it already carries
/// ([`THREAD_MAX`], [`THREAD_BYTES`]).
pub fn room(engine: &Engine, id: Id, adding: &[ImageView]) -> Result<(), BusError> {
    let (count, bytes) = with_db(engine, |db| db.carried_size(id, adding))?;
    if count > THREAD_MAX || bytes > THREAD_BYTES {
        return Err(BusError::refused("thread.images_full", format!(
            "A thread carries at most {THREAD_MAX} pictures and {} MiB of them, and this one would carry {count} and {:.1} MiB",
            THREAD_BYTES >> 20, bytes as f64 / f64::from(1 << 20),
        )).with_hint("start a new thread for these pictures, or send the words alone"));
    }
    Ok(())
}

/// The uploads `ids` name, each one known.
pub fn find(engine: &Engine, ids: &[String]) -> Result<Vec<ImageView>, BusError> {
    let unknown = |id: &str| {
        BusError::invalid("thread.image_unknown", format!("No picture {id}")).with_hint("upload it with thread.upload first; one never sent is removed after a day")
    };
    ids.iter().map(|id| {
        if !valid_id(id) {
            return Err(unknown(id));
        }
        with_db(engine, |db| db.image(id))?.ok_or_else(|| unknown(id))
    }).collect()
}

/// Picture `id` and its bytes (`thread.image`).
pub fn read(engine: &Engine, id: &str) -> Result<(ImageView, Vec<u8>), BusError> {
    let missing = || BusError::not_found("thread.image_not_found", format!("No picture {id}"));
    if !valid_id(id) {
        return Err(missing());
    }
    let image = with_db(engine, |db| db.image(id))?.ok_or_else(missing)?;
    match std::fs::read(path(engine, &image)) {
        Ok(bytes) => Ok((image, bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(missing()),
        Err(e) => Err(crate::engine::internal(e)),
    }
}

/// Each of `images` as `(media_type, base64)`, read for the agent; why not, when one cannot be.
pub(super) fn encoded(engine: &Engine, images: &[ImageView]) -> Result<Vec<(String, String)>, String> {
    use base64::Engine as _;
    images.iter().map(|image| {
        let file = path(engine, image);
        let bytes = std::fs::read(&file).map_err(|e| format!("A picture could not be read for the agent ({}): {e}", file.display()))?;
        Ok((image.media_type.clone(), base64::engine::general_purpose::STANDARD.encode(bytes)))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> String {
        use base64::Engine as _;
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend(w.to_be_bytes());
        bytes.extend(h.to_be_bytes());
        bytes.extend(IEND);
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    const IEND: &[u8] = b"\0\0\0\0IEND\xAE\x42\x60\x82";

    #[test]
    fn a_picture_cut_short_or_too_large_is_refused() {
        use base64::Engine as _;
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let now = crate::time::now();
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let whole = base64::engine::general_purpose::STANDARD.decode(png(4, 3)).unwrap();
        let cut = &whole[..whole.len() - IEND.len()];
        let refused = upload(&engine, "image/png", &b64(cut), &now).unwrap_err();
        assert_eq!(refused.code, "thread.upload");
        assert!(refused.message.contains("cut short"), "{}", refused.message);
        assert!(upload(&engine, "image/png", &png(4, 3), &now).is_ok());
        let wide = upload(&engine, "image/png", &png(8001, 3), &now).unwrap_err();
        assert!(wide.message.contains("8001 by 3 px"), "{}", wide.message);
        assert!(upload(&engine, "image/png", &png(8000, 3), &now).is_ok());
    }

    fn file_of(engine: &Engine, image: &ImageView) -> PathBuf {
        path(engine, image)
    }

    #[test]
    fn an_upload_never_sent_goes_after_a_day_and_a_sent_one_stays() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let now = crate::time::now();
        let two_days_ago = crate::time::days_ago(2);
        let unsent = upload(&engine, "image/png", &png(4, 3), &two_days_ago).unwrap();
        let sent = upload(&engine, "image/png", &png(5, 3), &two_days_ago).unwrap();
        let thread = super::super::create(&engine, super::super::MONEY, "Receipts", None, None, &two_days_ago).unwrap().id;
        super::super::post(&engine, thread, "", std::slice::from_ref(&sent), &two_days_ago).unwrap();
        // A file left with no row by a write cut short, a day and more ago, and one just now.
        let stray = dir(&engine).join(format!("{}.png", "0".repeat(32)));
        let fresh = dir(&engine).join(format!("{}.png", "1".repeat(32)));
        for file in [&stray, &fresh] {
            std::fs::write(file, b"x").unwrap();
        }
        let old = SystemTime::now() - UNSENT - Duration::from_secs(60);
        std::fs::File::options().write(true).open(&stray).unwrap().set_modified(old).unwrap();
        assert!(file_of(&engine, &unsent).is_file());

        let next = upload(&engine, "image/png", &png(6, 3), &now).unwrap();
        assert!(!file_of(&engine, &unsent).exists(), "unsent for two days");
        assert!(with_db(&engine, |db| db.image(&unsent.id)).unwrap().is_none());
        assert!(file_of(&engine, &sent).is_file() && file_of(&engine, &next).is_file());
        assert!(!stray.exists() && fresh.exists(), "a stray file goes only once it is a day old");
        std::fs::remove_file(&fresh).unwrap();
        assert!(find(&engine, std::slice::from_ref(&unsent.id)).is_err() && find(&engine, std::slice::from_ref(&sent.id)).is_ok());
    }
}
