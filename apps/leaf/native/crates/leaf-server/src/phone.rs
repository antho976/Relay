//! The phone's own backup (the owner, 2026-11-25): its books (`books/<file>`), their covers
//! (`covers/<file>`) and the shelf (`shelf/shelf.db`, places, highlights, notes and reading time),
//! kept in `<LEAF_DATA>/phone/`. The phone sends a file only when it is new or changed, at most once
//! a day, and fetches them back on a new phone ("Restore"). Nothing here reaches a site.
//!
//! | Request | Answer |
//! |---|---|
//! | `GET /v1/phone/files` | `[PhoneFile]`: what is kept |
//! | `PUT /v1/phone/files/{kind}/{name}` | the file's bytes, kept whole or not at all (at most [`MAX_FILE`]) |
//! | `GET /v1/phone/files/{kind}/{name}` | the bytes back |

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use leaf_proto::PhoneFile;

/// The largest file the phone may keep here: a big PDF.
pub const MAX_FILE: u64 = 400 << 20;
const KINDS: [&str; 3] = ["shelf", "books", "covers"];

pub fn dir(data: &Path) -> PathBuf {
    data.join("phone")
}

/// Where a file is kept, for a kind this knows and a plain name: letters (any script), digits,
/// `-`, `_` and `.`, not starting with `.`, at most 200 characters; nothing that climbs out.
pub fn file(data: &Path, kind: &str, name: &str) -> Option<PathBuf> {
    let ok = KINDS.contains(&kind)
        && !name.is_empty()
        && name.chars().count() <= 200
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    ok.then(|| dir(data).join(kind).join(name))
}

/// Everything kept, by kind then name.
pub fn list(data: &Path) -> Vec<PhoneFile> {
    let mut out = Vec::new();
    for kind in KINDS {
        let Ok(rd) = std::fs::read_dir(dir(data).join(kind)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            let at_ms = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            out.push(PhoneFile { kind: kind.to_string(), name, size: m.len(), at_ms });
        }
    }
    out.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    out
}

/// Keep what `body` holds as `kind/name`: written beside it and moved into place, so a cut
/// connection leaves the copy before. Err with the status to answer.
pub fn put(data: &Path, kind: &str, name: &str, body: &mut dyn Read) -> Result<u64, (u16, String)> {
    let path = file(data, kind, name).ok_or((400, "not a name this keeps".to_string()))?;
    let parent = path.parent().expect("kept under a kind");
    std::fs::create_dir_all(parent).map_err(|e| (500, format!("couldn't make room: {e}")))?;
    let tmp = parent.join(format!(".{name}.part"));
    let written = (|| -> std::io::Result<u64> {
        let mut f = std::fs::File::create(&tmp)?;
        let n = std::io::copy(&mut body.take(MAX_FILE + 1), &mut f)?;
        f.flush()?;
        f.sync_all()?;
        Ok(n)
    })();
    match written {
        Ok(n) if n > MAX_FILE => {
            let _ = std::fs::remove_file(&tmp);
            Err((413, format!("over {} MB", MAX_FILE >> 20)))
        }
        Ok(n) => {
            std::fs::rename(&tmp, &path).map_err(|e| (500, format!("couldn't keep it: {e}")))?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err((500, format!("couldn't keep it: {e}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_names_of_known_kinds_are_kept_and_they_come_back_whole() {
        let data = std::env::temp_dir().join(format!("leaf-phone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        assert!(file(&data, "books", "1700-Dune.epub").is_some());
        assert!(file(&data, "books", "1700-Été-à-Paris.epub").is_some(), "any script");
        for bad in ["../leaf.db", ".hidden", "a/b", "", "a\\b"] {
            assert!(file(&data, "books", bad).is_none(), "{bad}");
        }
        assert!(file(&data, "app", "leaf.apk").is_none(), "not a kind this keeps");
        assert_eq!(put(&data, "shelf", "shelf.db", &mut &b"first"[..]), Ok(5));
        assert_eq!(put(&data, "shelf", "shelf.db", &mut &b"second"[..]), Ok(6));
        assert_eq!(std::fs::read(file(&data, "shelf", "shelf.db").unwrap()).unwrap(), b"second");
        let l = list(&data);
        assert_eq!(l.iter().map(|f| (f.kind.as_str(), f.name.as_str(), f.size)).collect::<Vec<_>>(), vec![("shelf", "shelf.db", 6)], "no part files listed");
        let _ = std::fs::remove_dir_all(&data);
    }
}
