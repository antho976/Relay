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
use relay_bus::ops::video::FileDiff;
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

/// Paths as an agent or the person names them, made the way a version writes them: `/`-separated,
/// relative, without `./` or a trailing `/`. `None` for one that leaves the folder or names nothing.
pub fn clean_paths(raw: &[String]) -> Option<Vec<String>> {
    raw.iter().map(|p| {
        let parts: Vec<&str> = p.trim().split('/').filter(|c| !c.is_empty() && *c != ".").collect();
        (!parts.is_empty() && !parts.contains(&"..") && !p.trim().starts_with('/')).then(|| parts.join("/"))
    }).collect()
}

/// Whether `path` is one of `only`, or under one of them; anything when there is no `only`.
pub fn within(path: &str, only: Option<&[String]>) -> bool {
    only.is_none_or(|only| only.iter().any(|o| path == o || path.strip_prefix(o.as_str()).is_some_and(|rest| rest.starts_with('/'))))
}

/// Put the folder back as `entries` has it: each file written from its blob, and the files a
/// version would hold that it does not name removed, but those no version could put back: one
/// too large to keep (a version names it with no copy, so the undo could not bring it back) and
/// a link, which stay as they are. A file too large to have been kept is left as it is. With
/// `only`, just those files and folders; the rest stay as they are. What it did: files written,
/// files removed, those it could not put back, and those it left.
pub fn restore(dir: &Path, blobs: &Path, entries: &[Entry], only: Option<&[String]>) -> Result<Restored, String> {
    let now = scan_paths(dir)?;
    let wanted: std::collections::HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let (mut removed, mut left) = (0, Vec::new());
    for rel in &now {
        if !wanted.contains(rel.as_str()) && within(rel, only) {
            let path = dir.join(rel);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink() || m.len() > MOST_KEPT) {
                left.push(rel.clone());
                continue;
            }
            match remove_in(dir, rel) {
                Ok(()) => removed += 1,
                // A folder on its way became a link since it was read: left, never followed.
                Err(e) if e.raw_os_error() == Some(libc::ELOOP) => left.push(rel.clone()),
                Err(e) => return Err(format!("{rel}: {e}")),
            }
        }
    }
    let (mut written, mut missing) = (0, Vec::new());
    for entry in entries.iter().filter(|e| within(&e.path, only)) {
        if entry.hash.is_empty() {
            // Too large to keep: told only when it is not there as the version had it.
            if fs::metadata(dir.join(&entry.path)).map(|m| m.len()).ok() != Some(entry.size) {
                missing.push(entry.path.clone());
            }
            continue;
        }
        let target = dir.join(&entry.path);
        // A link where the version had a file (or a folder of it) is left: writing through it
        // would change what it points at, outside the folder.
        if through_link(dir, &entry.path) {
            left.push(entry.path.clone());
            continue;
        }
        if fs::read(&target).ok().is_some_and(|b| hex(&Sha256::digest(&b)) == entry.hash) {
            continue;
        }
        match fs::read(blob_path(blobs, &entry.hash)) {
            Ok(bytes) => match write_in(dir, &entry.path, &bytes) {
                Ok(()) => written += 1,
                Err(e) if e.raw_os_error() == Some(libc::ELOOP) => left.push(entry.path.clone()),
                Err(e) => return Err(format!("{}: {e}", entry.path)),
            },
            Err(_) => missing.push(entry.path.clone()),
        }
    }
    Ok(Restored { written, removed, missing, left })
}

/// [`restore`], and when it fails partway (a full disk, a folder where the version has a file)
/// the folder put back as `before` has it, the scan taken just before, whose files are all kept
/// as blobs: a failed restore leaves the code as it found it, not half of each.
pub fn restore_or_back(dir: &Path, blobs: &Path, entries: &[Entry], before: &[Entry], only: Option<&[String]>) -> Result<Restored, String> {
    restore(dir, blobs, entries, only).map_err(|e| match restore(dir, blobs, before, only) {
        Ok(_) => format!("{e}; the folder was put back as it was"),
        Err(back) => format!("{e}; putting the folder back failed too: {back}"),
    })
}

/// The folder holding `rel` in `dir`, opened a folder at a time from `dir` itself and never
/// through a link (`O_NOFOLLOW`): one swapped in for any of them since it was read fails the
/// open with `ELOOP` rather than leading out of the folder. With `make`, missing folders are
/// made. The name left to act on in it.
fn open_parent(dir: &Path, rel: &str, make: bool) -> std::io::Result<(std::os::fd::OwnedFd, std::ffi::CString)> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    let name = |part: &[u8]| std::ffi::CString::new(part).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput));
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let open = |at: libc::c_int, part: &std::ffi::CStr| -> std::io::Result<OwnedFd> {
        // SAFETY: `part` is a NUL-terminated name, `at` an open folder or AT_FDCWD.
        let fd = unsafe { libc::openat(at, part.as_ptr(), flags) };
        if fd >= 0 {
            return Ok(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        let e = std::io::Error::last_os_error();
        // With O_DIRECTORY a link is told as "not a folder": said as the link it is.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: as above; `st` is written by the call.
        let link = e.raw_os_error() == Some(libc::ENOTDIR)
            && unsafe { libc::fstatat(at, part.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } == 0
            && st.st_mode & libc::S_IFMT == libc::S_IFLNK;
        Err(if link { std::io::Error::from_raw_os_error(libc::ELOOP) } else { e })
    };
    let mut at = open(libc::AT_FDCWD, &name(dir.as_os_str().as_bytes())?)?;
    let mut parts = rel.split('/').filter(|p| !p.is_empty()).peekable();
    while let Some(part) = parts.next() {
        let part = name(part.as_bytes())?;
        if parts.peek().is_none() {
            return Ok((at, part));
        }
        // SAFETY: as above.
        if make && unsafe { libc::mkdirat(at.as_raw_fd(), part.as_ptr(), 0o777) } < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        }
        at = open(at.as_raw_fd(), &part)?;
    }
    Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

/// Write `bytes` as `rel` in `dir`: to a hidden file beside it, then renamed over it, so a link
/// there is replaced rather than written through, and its folders opened as [`open_parent`]
/// opens them.
fn write_in(dir: &Path, rel: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    let (parent, name) = open_parent(dir, rel, true)?;
    let part = std::ffi::CString::new(format!(".{}.restoring-{}", name.to_string_lossy(), std::process::id()))
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: `part` is NUL-terminated and `parent` an open folder.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), part.as_ptr(), flags, 0o666 as libc::c_uint) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut file = fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
    let written = file.write_all(bytes).and_then(|_| {
        // SAFETY: both names NUL-terminated, both in `parent`.
        if unsafe { libc::renameat(parent.as_raw_fd(), part.as_ptr(), parent.as_raw_fd(), name.as_ptr()) } < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    });
    if written.is_err() {
        // SAFETY: as above.
        unsafe { libc::unlinkat(parent.as_raw_fd(), part.as_ptr(), 0) };
    }
    written
}

/// Remove `rel` in `dir` (a link itself, never what it points at), its folders opened as
/// [`open_parent`] opens them.
fn remove_in(dir: &Path, rel: &str) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let (parent, name) = open_parent(dir, rel, false)?;
    // SAFETY: `name` is NUL-terminated and `parent` an open folder.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Whether `rel` in `dir`, or a folder on the way to it, is a link.
fn through_link(dir: &Path, rel: &str) -> bool {
    let mut at = dir.to_path_buf();
    rel.split('/').any(|part| {
        at.push(part);
        fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink())
    })
}

/// Most lines of diff one answer holds: the files past them are listed without theirs.
const MOST_DIFF_LINES: usize = 3000;
/// How long one file's diff may take before `similar` settles for a coarser one (a generated
/// file of thousands of lines).
const DIFF_WITHIN: std::time::Duration = std::time::Duration::from_millis(200);

/// What differs from manifest `before` to `after`, in path order, each text file as a unified diff
/// of its kept contents; with `only`, those files and folders alone.
pub fn diff(blobs: &Path, before: &[Entry], after: &[Entry], only: Option<&[String]>) -> Vec<FileDiff> {
    let side = |entries: &[Entry]| -> std::collections::BTreeMap<String, Entry> {
        entries.iter().filter(|e| within(&e.path, only)).map(|e| (e.path.clone(), e.clone())).collect()
    };
    let (old, new) = (side(before), side(after));
    let paths: std::collections::BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    let text = |e: Option<&Entry>| -> Result<String, &'static str> {
        match e {
            None => Ok(String::new()),
            Some(e) if e.hash.is_empty() => Err("not_kept"),
            Some(e) => String::from_utf8(fs::read(blob_path(blobs, &e.hash)).map_err(|_| "not_kept")?).map_err(|_| "binary"),
        }
    };
    let mut budget = MOST_DIFF_LINES;
    let mut out = Vec::new();
    for path in paths {
        let (a, b) = (old.get(path), new.get(path));
        let change = match (a, b) {
            (Some(a), Some(b)) if a.hash == b.hash && a.size == b.size => continue,
            (Some(_), Some(_)) => "changed",
            (None, _) => "added",
            (_, None) => "removed",
        };
        let (diff, note) = match (text(a), text(b)) {
            (Err(why), _) | (_, Err(why)) => (None, Some(why)),
            (Ok(a), Ok(b)) => {
                let unified = similar::TextDiff::configure().timeout(DIFF_WITHIN).diff_lines(&a, &b)
                    .unified_diff().context_radius(3).header(&format!("a/{path}"), &format!("b/{path}")).to_string();
                let lines = unified.lines().count();
                if lines > budget {
                    budget = 0;
                    (None, Some("cut"))
                } else {
                    budget -= lines;
                    (Some(unified), None)
                }
            }
        };
        out.push(FileDiff { path: path.clone(), change: change.into(), diff, note: note.map(str::to_string) });
    }
    out
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
        let done = restore(&dir, &blobs, &first.entries, None).unwrap();
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
        let done = restore(&dir, &blobs, &first.entries, None).unwrap();
        assert_eq!(done.removed, 1);
        assert!(!dir.join("src/b.ts").exists());
        let mut left = done.left.clone();
        left.sort();
        assert_eq!(left, ["public/clip.mp4", "public/music.mp3"]);
        assert!(dir.join("public/music.mp3").exists() && fs::symlink_metadata(dir.join("public/clip.mp4")).is_ok());
    }

    /// The writes a restore makes never pass through a link, even one swapped in after the
    /// folder was read: a link as a folder on the way fails with ELOOP, and a link as the file
    /// itself is replaced, not written through; a removal takes the link, not its target.
    #[test]
    fn writes_and_removals_open_each_folder_without_following_a_link() {
        let root = tempfile::tempdir().unwrap();
        let (dir, outside) = (root.path().join("v"), root.path().join("outside"));
        tree(&dir, &[("public/keep.txt", "k")]);
        tree(&outside, &[("a.ts", "theirs"), ("song.mp3", "mine")]);
        std::os::unix::fs::symlink(&outside, dir.join("src")).unwrap();
        assert_eq!(write_in(&dir, "src/a.ts", b"ours").unwrap_err().raw_os_error(), Some(libc::ELOOP));
        assert_eq!(remove_in(&dir, "src/a.ts").unwrap_err().raw_os_error(), Some(libc::ELOOP));
        std::os::unix::fs::symlink(outside.join("song.mp3"), dir.join("public/music.mp3")).unwrap();
        write_in(&dir, "public/music.mp3", b"old").unwrap();
        assert!(!fs::symlink_metadata(dir.join("public/music.mp3")).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(dir.join("public/music.mp3")).unwrap(), "old");
        std::os::unix::fs::symlink(outside.join("song.mp3"), dir.join("public/clip.mp3")).unwrap();
        remove_in(&dir, "public/clip.mp3").unwrap();
        assert_eq!((fs::read_to_string(outside.join("a.ts")).unwrap(), fs::read_to_string(outside.join("song.mp3")).unwrap()), ("theirs".into(), "mine".into()));
        write_in(&dir, "src2/deep/new.ts", b"n").unwrap();
        assert_eq!(fs::read_to_string(dir.join("src2/deep/new.ts")).unwrap(), "n", "folders made as it goes");
        assert!(fs::read_dir(dir.join("public")).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("restoring")), "no part file left");
    }

    /// A restore that fails partway (here a folder where the version has a file) puts the folder
    /// back as it was before it started.
    #[test]
    fn a_restore_that_fails_partway_puts_the_folder_back() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/a.ts", "old a"), ("src/z", "old z")]);
        let first = scan(&dir, &blobs).unwrap();
        fs::write(dir.join("src/a.ts"), "new a").unwrap();
        fs::remove_file(dir.join("src/z")).unwrap();
        tree(&dir, &[("src/z/x.ts", "x"), ("src/z/y.ts", "y")]);
        let before = scan(&dir, &blobs).unwrap();
        let why = restore_or_back(&dir, &blobs, &first.entries, &before.entries, None).unwrap_err();
        assert!(why.contains("src/z") && why.contains("put back as it was"), "{why}");
        assert_eq!(scan(&dir, &blobs).unwrap().digest, before.digest, "as it was before the restore");
    }

    /// A file the version holds, now a link (or in a folder that is now a link), is not written
    /// through it: what it points at, outside the folder, stays as it is.
    #[test]
    fn a_restore_never_writes_through_a_link() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("public/music.mp3", "old"), ("src/a.ts", "a")]);
        let first = scan(&dir, &blobs).unwrap();
        let outside = root.path().join("outside");
        tree(&outside, &[("song.mp3", "mine"), ("a.ts", "theirs")]);
        fs::remove_file(dir.join("public/music.mp3")).unwrap();
        std::os::unix::fs::symlink(outside.join("song.mp3"), dir.join("public/music.mp3")).unwrap();
        fs::remove_file(dir.join("src/a.ts")).unwrap();
        fs::remove_dir(dir.join("src")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("src")).unwrap();
        let done = restore(&dir, &blobs, &first.entries, None).unwrap();
        let mut left = done.left.clone();
        left.sort();
        assert_eq!(left, ["public/music.mp3", "src", "src/a.ts"], "the link itself, and what the version holds through each");
        assert_eq!(fs::read_to_string(outside.join("song.mp3")).unwrap(), "mine");
        assert_eq!(fs::read_to_string(outside.join("a.ts")).unwrap(), "theirs");
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

    /// A scene brought back from a take: only the files named, or under the folders named, go
    /// back as the take had them; what came after stays.
    #[test]
    fn a_restore_of_some_files_leaves_the_rest() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/Video.tsx", "take one"), ("src/scenes/Open.tsx", "open one"), ("src/scenes/End.tsx", "end one")]);
        let take = scan(&dir, &blobs).unwrap();
        tree(&dir, &[("src/Video.tsx", "take two"), ("src/scenes/Open.tsx", "open two"), ("src/scenes/Extra.tsx", "extra")]);
        fs::remove_file(dir.join("src/scenes/End.tsx")).unwrap();
        let only = clean_paths(&["./src/scenes/".to_string()]).unwrap();
        assert_eq!(only, ["src/scenes"]);
        let done = restore(&dir, &blobs, &take.entries, Some(&only)).unwrap();
        assert_eq!((done.written, done.removed), (2, 1));
        assert_eq!(fs::read_to_string(dir.join("src/scenes/Open.tsx")).unwrap(), "open one");
        assert_eq!(fs::read_to_string(dir.join("src/scenes/End.tsx")).unwrap(), "end one");
        assert!(!dir.join("src/scenes/Extra.tsx").exists());
        assert_eq!(fs::read_to_string(dir.join("src/Video.tsx")).unwrap(), "take two", "outside what was named");
        // One file alone; a name that is a prefix but not a folder is not under it.
        fs::write(dir.join("src/Video.tsx"), "take three").unwrap();
        restore(&dir, &blobs, &take.entries, Some(&["src/Video.tsx".to_string()])).unwrap();
        assert_eq!(fs::read_to_string(dir.join("src/Video.tsx")).unwrap(), "take one");
        assert!(within("src/scenes/Open.tsx", Some(&["src/scenes".to_string()])));
        assert!(!within("src/scenes-old/Open.tsx", Some(&["src/scenes".to_string()])));
        for bad in ["../x", "/etc/passwd", "", "src/../../x", "  "] {
            assert!(clean_paths(&[bad.to_string()]).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn a_diff_shows_each_text_file_that_changed() {
        let root = tempfile::tempdir().unwrap();
        let (dir, blobs) = (root.path().join("v"), root.path().join("blobs"));
        tree(&dir, &[("src/Video.tsx", "a\nb\nc\n"), ("src/Old.tsx", "old\n"), ("src/Same.tsx", "same\n")]);
        let first = scan(&dir, &blobs).unwrap();
        tree(&dir, &[("src/Video.tsx", "a\nB\nc\n"), ("src/New.tsx", "new\n")]);
        fs::remove_file(dir.join("src/Old.tsx")).unwrap();
        fs::write(dir.join("public/logo.bin"), [0xff, 0xfe, 0x00]).unwrap_or_else(|_| {
            fs::create_dir_all(dir.join("public")).unwrap();
            fs::write(dir.join("public/logo.bin"), [0xff, 0xfe, 0x00]).unwrap();
        });
        let second = scan(&dir, &blobs).unwrap();
        let files = diff(&blobs, &first.entries, &second.entries, None);
        let said: Vec<(&str, &str)> = files.iter().map(|f| (f.path.as_str(), f.change.as_str())).collect();
        assert_eq!(said, [("public/logo.bin", "added"), ("src/New.tsx", "added"), ("src/Old.tsx", "removed"), ("src/Video.tsx", "changed")]);
        let video = &files[3];
        assert!(video.diff.as_deref().unwrap().contains("-b\n+B\n"), "{video:?}");
        assert!(video.diff.as_deref().unwrap().starts_with("--- a/src/Video.tsx\n+++ b/src/Video.tsx\n"), "{video:?}");
        assert_eq!((files[0].diff.as_deref(), files[0].note.as_deref()), (None, Some("binary")));
        assert_eq!(diff(&blobs, &first.entries, &second.entries, Some(&["src/Old.tsx".to_string()])).len(), 1);
    }
}
