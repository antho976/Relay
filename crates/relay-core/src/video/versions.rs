//! A video's versions (docs/VIDEO.md, "Versions"): its code as it stood each time Relay read it
//! changed. Agents edit the folder with their own tools, so Relay never sees an edit happen; each
//! check, still and render reads the folder first, and records a version when its files differ
//! from the newest one's. A version is a manifest (path, content hash, size) whose files are
//! kept once each by hash in `videos/.relay/blobs/`, so a hundred versions of a ten-file video
//! cost the files that changed.
//!
//! What a version holds: every file under the folder but `out/` (renders), `public/assets/`
//! (what `video.asset.add` copied in, never changed in place), dot-files and folders, and files
//! over 2 MiB, which a version names without keeping.

use relay_bus::error::BusError;
use relay_bus::types::Id;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::IntoBus;

/// Files above this are named in a version but not kept.
pub const MOST_KEPT: u64 = 2 * 1024 * 1024;
/// Most files a version reads.
const MOST_FILES: usize = 4000;

/// One file of a version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Relative to the video's folder, `/`-separated.
    pub path: String,
    /// sha-256 of its bytes, hex; empty for a file too large to keep.
    pub hash: String,
    pub size: u64,
}

/// The folder as it is now.
#[derive(Debug, Clone, PartialEq)]
pub struct Scan {
    pub entries: Vec<Entry>,
    /// sha-256 of the manifest: equal scans have equal digests.
    pub digest: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether `rel` (relative to the folder) belongs in a version.
fn kept(rel: &Path) -> bool {
    let mut parts = rel.components().map(|c| c.as_os_str().to_string_lossy());
    let first = parts.next().unwrap_or_default();
    if first == "out" || first == "node_modules" {
        return false;
    }
    if first == "public" && parts.next().is_some_and(|p| p == "assets") {
        return false;
    }
    !rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

/// Read the folder: every file a version holds, its content stored by hash in `blobs` as it is
/// read (a blob already there is not written again).
pub fn scan(dir: &Path, blobs: &Path) -> Result<Scan, String> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let entries = match fs::read_dir(&at) {
            Ok(entries) => entries,
            Err(e) if at == dir => return Err(format!("{}: {e}", dir.display())),
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
            if !kept(&rel) {
                continue;
            }
            // A link is read as what it is, never followed out of the folder.
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() => files.push(rel),
                _ => {}
            }
            if files.len() > MOST_FILES {
                return Err(format!("{} holds over {MOST_FILES} files", dir.display()));
            }
        }
    }
    files.sort();
    let mut entries = Vec::with_capacity(files.len());
    for rel in files {
        let path = dir.join(&rel);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let rel = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        if size > MOST_KEPT {
            entries.push(Entry { path: rel, hash: String::new(), size });
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        let hash = hex(&Sha256::digest(&bytes));
        let blob = blob_path(blobs, &hash);
        if !blob.exists() {
            if let Some(parent) = blob.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            // Written aside and renamed, so a blob is whole or absent.
            let part = blob.with_extension("part");
            fs::write(&part, &bytes).and_then(|_| fs::rename(&part, &blob)).map_err(|e| format!("{}: {e}", blob.display()))?;
        }
        entries.push(Entry { path: rel, hash, size: bytes.len() as u64 });
    }
    let manifest = serde_json::to_vec(&entries).unwrap_or_default();
    Ok(Scan { digest: hex(&Sha256::digest(&manifest)), entries })
}

fn blob_path(blobs: &Path, hash: &str) -> PathBuf {
    blobs.join(&hash[..2.min(hash.len())]).join(hash)
}

/// Record `scan` as video `video_id`'s next version unless its newest already has those files;
/// the version the code is at, and whether it is new.
pub fn record(tx: &Transaction, video_id: Id, scan: &Scan, actor: &str, now: &str) -> Result<(i64, bool), BusError> {
    let newest: Option<(i64, String)> = tx.prepare_cached("SELECT n, digest FROM video_versions WHERE video_id = ?1 ORDER BY n DESC LIMIT 1").bus()?
        .query_row([video_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().bus()?;
    if let Some((n, digest)) = &newest {
        if *digest == scan.digest {
            return Ok((*n, false));
        }
    }
    let n = newest.map_or(1, |(n, _)| n + 1);
    let manifest = serde_json::to_string(&scan.entries).bus()?;
    tx.prepare_cached("INSERT INTO video_versions(video_id, n, digest, manifest, files, actor, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)").bus()?
        .execute(params![video_id, n, scan.digest, manifest, scan.entries.len() as i64, actor, now]).bus()?;
    tx.prepare_cached("UPDATE videos SET version = ?2 WHERE id = ?1").bus()?.execute(params![video_id, n]).bus()?;
    Ok((n, true))
}

/// What changed from manifest `before` to `after`: changed paths as they are, added ones with a
/// `+`, removed ones with a `-`; at most `most`, then how many more.
pub fn changes(before: &[Entry], after: &[Entry], most: usize) -> Vec<String> {
    let old: std::collections::HashMap<&str, &str> = before.iter().map(|e| (e.path.as_str(), e.hash.as_str())).collect();
    let new: std::collections::HashSet<&str> = after.iter().map(|e| e.path.as_str()).collect();
    let mut out: Vec<String> = after.iter().filter_map(|e| match old.get(e.path.as_str()) {
        None => Some(format!("+{}", e.path)),
        Some(hash) if *hash != e.hash => Some(e.path.clone()),
        Some(_) => None,
    }).collect();
    out.extend(before.iter().filter(|e| !new.contains(e.path.as_str())).map(|e| format!("-{}", e.path)));
    if out.len() > most {
        let more = out.len() - most;
        out.truncate(most);
        out.push(format!("{more} more"));
    }
    out
}

/// Version `n`'s manifest.
pub fn manifest(conn: &Connection, video_id: Id, n: i64) -> Result<Vec<Entry>, BusError> {
    let text: String = conn.prepare_cached("SELECT manifest FROM video_versions WHERE video_id = ?1 AND n = ?2").bus()?
        .query_row(params![video_id, n], |r| r.get(0)).optional().bus()?
        .ok_or_else(|| BusError::not_found("video.version", format!("video {video_id} has no version {n}"))
            .with_hint("video.get lists its versions"))?;
    serde_json::from_str(&text).bus()
}

/// Put the folder back as `entries` has it: each file written from its blob, and the files a
/// version would hold that it does not name removed, but those no version could put back: one
/// too large to keep (a version names it with no copy, so the undo could not bring it back) and
/// a link, which stay as they are. A file too large to have been kept is left as it is. What it
/// did: files written, files removed, those it could not put back, and those it left.
pub fn restore(dir: &Path, blobs: &Path, entries: &[Entry]) -> Result<Restored, String> {
    let now = scan_paths(dir)?;
    let wanted: std::collections::HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let (mut removed, mut left) = (0, Vec::new());
    for rel in &now {
        if !wanted.contains(rel.as_str()) {
            let path = dir.join(rel);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink() || m.len() > MOST_KEPT) {
                left.push(rel.clone());
                continue;
            }
            fs::remove_file(&path).map_err(|e| format!("{rel}: {e}"))?;
            removed += 1;
        }
    }
    let (mut written, mut missing) = (0, Vec::new());
    for entry in entries {
        if entry.hash.is_empty() {
            continue;
        }
        let target = dir.join(&entry.path);
        if fs::read(&target).ok().is_some_and(|b| hex(&Sha256::digest(&b)) == entry.hash) {
            continue;
        }
        match fs::read(blob_path(blobs, &entry.hash)) {
            Ok(bytes) => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", entry.path))?;
                }
                fs::write(&target, bytes).map_err(|e| format!("{}: {e}", entry.path))?;
                written += 1;
            }
            Err(_) => missing.push(entry.path.clone()),
        }
    }
    Ok(Restored { written, removed, missing, left })
}

/// What [`restore`] did.
#[derive(Debug, Default)]
pub struct Restored {
    pub written: usize,
    pub removed: usize,
    /// Named by the version, with no copy kept to write back.
    pub missing: Vec<String>,
    /// Not named by the version, and left: no version could put them back.
    pub left: Vec<String>,
}

/// The relative paths of the files a version would hold, as they are now.
fn scan_paths(dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in fs::read_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
            if !kept(&rel) {
                continue;
            }
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() || t.is_symlink() => {
                    out.push(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(dir: &Path, files: &[(&str, &str)]) {
        for (path, text) in files {
            let p = dir.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
    }

    #[test]
    fn a_scan_holds_the_code_and_leaves_out_renders_assets_and_dotfiles() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/Video.tsx", "a"), ("src/index.ts", "b"), ("tsconfig.json", "{}"), ("public/logo.svg", "<svg/>"),
            ("public/assets/clip.mp4", "big"), ("out/main.mp4", "render"), (".cache/x", "y"), ("src/.hidden", "z")]);
        let scan = scan(&dir, &blobs).unwrap();
        let paths: Vec<&str> = scan.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["public/logo.svg", "src/Video.tsx", "src/index.ts", "tsconfig.json"]);
        assert!(blob_path(&blobs, &scan.entries[1].hash).exists());
        // The same files are the same digest; an edit is another.
        assert_eq!(super::scan(&dir, &blobs).unwrap().digest, scan.digest);
        fs::write(dir.join("src/Video.tsx"), "a2").unwrap();
        assert_ne!(super::scan(&dir, &blobs).unwrap().digest, scan.digest);
    }

    #[test]
    fn a_restore_puts_the_files_back_and_takes_out_new_ones() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/Video.tsx", "first"), ("src/Scene.tsx", "scene"), ("out/main.mp4", "render")]);
        let first = scan(&dir, &blobs).unwrap();
        fs::write(dir.join("src/Video.tsx"), "second").unwrap();
        fs::remove_file(dir.join("src/Scene.tsx")).unwrap();
        tree(&dir, &[("src/New.tsx", "new")]);
        let done = restore(&dir, &blobs, &first.entries).unwrap();
        assert_eq!((done.written, done.removed, done.missing.len()), (2, 1, 0));
        assert_eq!(fs::read_to_string(dir.join("src/Video.tsx")).unwrap(), "first");
        assert_eq!(fs::read_to_string(dir.join("src/Scene.tsx")).unwrap(), "scene");
        assert!(!dir.join("src/New.tsx").exists());
        // Renders are not the code's: a restore leaves them.
        assert!(dir.join("out/main.mp4").exists());
        assert_eq!(scan(&dir, &blobs).unwrap().digest, first.digest);
    }

    /// A file too large for a version to keep, or a link, is never removed by a restore to a
    /// version without it: nothing could put it back, the undo included.
    #[test]
    fn a_restore_leaves_what_no_version_could_put_back() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/a.ts", "a")]);
        let first = scan(&dir, &blobs).unwrap();
        tree(&dir, &[("src/b.ts", "b")]);
        fs::create_dir_all(dir.join("public")).unwrap();
        fs::write(dir.join("public/music.mp3"), vec![7u8; (MOST_KEPT + 1) as usize]).unwrap();
        std::os::unix::fs::symlink(root.path().join("elsewhere.mp4"), dir.join("public/clip.mp4")).unwrap();
        let done = restore(&dir, &blobs, &first.entries).unwrap();
        assert_eq!(done.removed, 1);
        assert!(!dir.join("src/b.ts").exists());
        let mut left = done.left.clone();
        left.sort();
        assert_eq!(left, ["public/clip.mp4", "public/music.mp3"]);
        assert!(dir.join("public/music.mp3").exists() && fs::symlink_metadata(dir.join("public/clip.mp4")).is_ok());
    }

    #[test]
    fn a_version_says_what_changed_from_the_one_before() {
        let e = |path: &str, hash: &str| Entry { path: path.into(), hash: hash.into(), size: 1 };
        let before = [e("src/Video.tsx", "a"), e("src/Old.tsx", "b"), e("tsconfig.json", "c")];
        let after = [e("src/Video.tsx", "a2"), e("src/Hook.tsx", "d"), e("tsconfig.json", "c")];
        assert_eq!(changes(&before, &after, 12), ["src/Video.tsx", "+src/Hook.tsx", "-src/Old.tsx"]);
        assert_eq!(changes(&before, &after, 1), ["src/Video.tsx", "2 more"]);
        assert!(changes(&after, &after, 12).is_empty());
    }

    #[test]
    fn a_large_file_is_named_but_not_kept() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/a.ts", "a")]);
        fs::create_dir_all(dir.join("public")).unwrap();
        fs::write(dir.join("public/big.bin"), vec![7u8; (MOST_KEPT + 1) as usize]).unwrap();
        let scan = scan(&dir, &blobs).unwrap();
        let big = scan.entries.iter().find(|e| e.path == "public/big.bin").unwrap();
        assert_eq!((big.hash.as_str(), big.size), ("", MOST_KEPT + 1));
    }
}
