//! `phone.app.*` (BUS.md §10.21): the phone app's updates, served by the PC it is linked to.
//! `apps/relay-code/tools/publish-apk.sh` writes `relay-code.apk` then `latest.json` into the
//! store's `phone/` folder, each under a temporary name then renamed, so `latest.json` never names
//! an APK still being copied. Both ops are disk reads with nothing locked (D149).

use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

use base64::Engine as _;
use relay_bus::ops::phone::{Apk, ApkOut, AppLatest, Latest, APK_PIECE};
use relay_bus::BusError;

use crate::engine::Engine;

/// The published APK and what it is, in the phone folder.
pub const APK: &str = "relay-code.apk";
pub const LATEST: &str = "latest.json";

pub fn register(e: &mut Engine) {
    e.register_unlocked::<Latest>(|ctx, _p| latest(&ctx.engine().store.phone_dir()));
    e.register_unlocked::<Apk>(|ctx, p| {
        let dir = ctx.engine().store.phone_dir();
        if let Some(want) = p.sha256.as_deref() {
            same_build(&dir, want)?;
        }
        piece(&dir, p.offset.unwrap_or(0), p.len)
    });
}

/// What `latest.json` says, or none when nothing is published.
pub fn latest(dir: &Path) -> Result<Option<AppLatest>, BusError> {
    let text = match std::fs::read_to_string(dir.join(LATEST)) {
        Ok(t) => t,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(BusError::unavailable("phone.app", format!("cannot read {}: {e}", dir.join(LATEST).display()))),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| BusError::unavailable("phone.app", format!("{} does not read: {e}", dir.join(LATEST).display())).with_hint("publish again with apps/relay-code/tools/publish-apk.sh"))
}

/// Refuse when the published build is no longer the one being downloaded: a publish mid-download
/// would otherwise hand the phone the rest of another APK.
pub fn same_build(dir: &Path, sha256: &str) -> Result<(), BusError> {
    match latest(dir)? {
        Some(l) if l.sha256.eq_ignore_ascii_case(sha256) => Ok(()),
        _ => Err(BusError::conflict("phone.app_changed", "the PC published another build of the phone app since this download began")
            .with_hint("start again from phone.app.latest")),
    }
}

/// The piece of the APK from `offset`: at most `len` (and [`APK_PIECE`]) bytes.
pub fn piece(dir: &Path, offset: u64, len: Option<u64>) -> Result<ApkOut, BusError> {
    let path = dir.join(APK);
    let mut file = std::fs::File::open(&path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => BusError::not_found("phone.app_missing", "no phone app is published on this PC").with_hint("publish one with apps/relay-code/tools/publish-apk.sh"),
        _ => BusError::unavailable("phone.app", format!("cannot read {}: {e}", path.display())),
    })?;
    let size = file.metadata().map_err(|e| BusError::unavailable("phone.app", e.to_string()))?.len();
    if offset > size {
        return Err(BusError::invalid("phone.app", format!("offset {offset} is past the end of the APK ({size} bytes)")).with_hint("start at 0, then at each piece's end"));
    }
    let want = len.unwrap_or(APK_PIECE).clamp(1, APK_PIECE);
    file.seek(SeekFrom::Start(offset)).map_err(|e| BusError::unavailable("phone.app", e.to_string()))?;
    let mut bytes = Vec::with_capacity(want.min(size - offset) as usize);
    file.take(want).read_to_end(&mut bytes).map_err(|e| BusError::unavailable("phone.app", e.to_string()))?;
    Ok(ApkOut { size, eof: offset + bytes.len() as u64 >= size, bytes_b64: base64::engine::general_purpose::STANDARD.encode(&bytes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unb64(s: &str) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD.decode(s).unwrap()
    }

    #[test]
    fn nothing_published_is_none_and_no_apk() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(latest(dir.path()).unwrap(), None);
        assert_eq!(piece(dir.path(), 0, None).unwrap_err().code, "phone.app_missing");
    }

    #[test]
    fn latest_reads_what_publish_wrote() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LATEST), r#"{"build":312,"version":"0.312","size":5,"sha256":"ab","made_at":1700000000000}"#).unwrap();
        let l = latest(dir.path()).unwrap().unwrap();
        assert_eq!((l.build, l.version.as_str(), l.size, l.sha256.as_str()), (312, "0.312", 5, "ab"));
        std::fs::write(dir.path().join(LATEST), "{").unwrap();
        assert_eq!(latest(dir.path()).unwrap_err().code, "phone.app");
    }

    #[test]
    fn the_apk_reads_in_pieces_to_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let apk: Vec<u8> = (0..10u8).collect();
        std::fs::write(dir.path().join(APK), &apk).unwrap();
        let first = piece(dir.path(), 0, Some(4)).unwrap();
        assert_eq!((first.size, first.eof, unb64(&first.bytes_b64)), (10, false, vec![0, 1, 2, 3]));
        let middle = piece(dir.path(), 4, Some(4)).unwrap();
        assert_eq!((middle.eof, unb64(&middle.bytes_b64)), (false, vec![4, 5, 6, 7]));
        let last = piece(dir.path(), 8, Some(4)).unwrap();
        assert_eq!((last.eof, unb64(&last.bytes_b64)), (true, vec![8, 9]));
        // Asking for more than a piece gets a piece; at the very end, an empty last piece.
        let whole = piece(dir.path(), 0, Some(APK_PIECE * 4)).unwrap();
        assert!(whole.eof && unb64(&whole.bytes_b64) == apk);
        let end = piece(dir.path(), 10, None).unwrap();
        assert!(end.eof && end.bytes_b64.is_empty());
        assert_eq!(piece(dir.path(), 11, None).unwrap_err().code, "phone.app");
    }

    #[test]
    fn a_download_of_a_build_published_over_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LATEST), r#"{"build":2,"version":"0.2","size":3,"sha256":"bb","made_at":1}"#).unwrap();
        assert!(same_build(dir.path(), "BB").is_ok());
        assert_eq!(same_build(dir.path(), "aa").unwrap_err().code, "phone.app_changed");
        std::fs::remove_file(dir.path().join(LATEST)).unwrap();
        assert_eq!(same_build(dir.path(), "bb").unwrap_err().code, "phone.app_changed", "unpublished since");
    }
}
