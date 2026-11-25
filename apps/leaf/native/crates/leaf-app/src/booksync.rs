//! A copy of the books on the server (the owner, 2026-11-25): the books themselves (`books/`), their
//! covers and the shelf (`shelf.db`: places, highlights, notes, reading time), so a lost phone
//! loses none of it. Comics need none of this: the server holds them already.
//!
//! Once a day while the app is open ([`due`]), a thread copies the shelf as it stands (`VACUUM
//! INTO`, beside the shelf's own connection), sends the books and covers the server doesn't have
//! yet ([`to_send`]), then the shelf. A phone that never made a copy doesn't send its shelf over
//! one already there (a new phone's empty shelf would replace the real one): it says a copy was
//! found ([`Outcome::Found`]), and Settings offers Restore. Restore fetches every file back, then
//! the shelf's worker takes the copy's rows in one transaction (`shelf::Op::Restore`).
//!
//! The server keeps files under 100 MB the way the public address reaches it: a book over
//! [`MAX_FILE`] stays on this phone only, and the copy says how many did.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use leaf_proto::PhoneFile;
use serde::{Deserialize, Serialize};

use crate::net::{Client, Fail, Server, query_escape};

/// A copy is made at most this often.
pub const EVERY_MS: i64 = 20 * 3_600_000;
/// After a copy that failed, the next try waits this long.
const RETRY_MS: i64 = 3_600_000;
/// The largest book sent: Cloudflare, in front of the public address, takes 100 MB at most.
pub const MAX_FILE: u64 = 95 << 20;

/// The last copy this phone made (kept in `config.json`).
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, PartialEq)]
pub struct Copied {
    pub at_ms: i64,
    pub books: usize,
}

/// How a copy or a restore ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Sent: the books in the shelf, and how many were too big to send.
    Copied { books: usize, too_big: usize },
    /// The server holds a copy this phone didn't make (made when, in ms): nothing was sent.
    Found(i64),
    /// Every file came back; the shelf's copy waits at this path for the shelf to take.
    Restored(PathBuf),
    Failed(String),
}

/// What Settings shows while a copy or restore runs, and what it last found.
#[derive(Default)]
pub struct BookSync {
    running: Arc<AtomicBool>,
    restoring: bool,
    out: Arc<Mutex<Option<Outcome>>>,
    last_try_ms: i64,
    /// A copy on the server this phone didn't make, as of when (ms).
    pub found: Option<i64>,
    pub error: Option<String>,
    /// Books the last copy left out for their size.
    pub too_big: usize,
}

/// A copy is due: there are books, the last one is a day old, and no try failed within the hour.
pub fn due(copied: Copied, last_try_ms: i64, now_ms: i64, books: usize) -> bool {
    books > 0 && now_ms - copied.at_ms >= EVERY_MS && now_ms - last_try_ms >= RETRY_MS
}

/// Which of this phone's files (kind, name, size) the server lacks or holds another size of, and
/// how many are too big to send.
pub fn to_send(local: &[(&'static str, String, u64)], remote: &[PhoneFile]) -> (Vec<(&'static str, String)>, usize) {
    let mut send = Vec::new();
    let mut too_big = 0;
    for (kind, name, size) in local {
        if remote.iter().any(|f| f.kind == *kind && f.name == *name && f.size == *size) {
            continue;
        }
        if *size > MAX_FILE {
            too_big += 1;
        } else {
            send.push((*kind, name.clone()));
        }
    }
    (send, too_big)
}

fn files_path(kind: &str, name: &str) -> String {
    format!("/v1/phone/files/{kind}/{}", query_escape(name))
}

fn cover_name(id: i64) -> String {
    crate::images::CoverOf::Book(id).file(Path::new("")).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn local_path(data: &Path, kind: &str, name: &str) -> PathBuf {
    match kind {
        "shelf" => data.join("shelf.restore.db"),
        k => data.join(k).join(name),
    }
}

impl BookSync {
    pub fn busy(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn restoring(&self) -> bool {
        self.busy() && self.restoring
    }

    /// The thread last started was a restore.
    pub fn last_was_restore(&self) -> bool {
        self.restoring
    }

    /// How the thread ended, once it has.
    pub fn take(&mut self) -> Option<Outcome> {
        let o = self.out.lock().unwrap_or_else(|e| e.into_inner()).take()?;
        match &o {
            Outcome::Copied { too_big, .. } => {
                self.error = None;
                self.found = None;
                self.too_big = *too_big;
            }
            Outcome::Found(at) => self.found = Some(*at),
            Outcome::Restored(_) => {
                self.error = None;
                self.found = None;
                // The copy this phone now makes is due at once.
                self.last_try_ms = 0;
            }
            Outcome::Failed(e) => self.error = Some(e.clone()),
        }
        Some(o)
    }

    /// Copy the books to the server on a thread of its own. `made_before`: this phone has made a
    /// copy, so one on the server is its own to replace.
    pub fn copy(&mut self, client: Client, server: Server, data: PathBuf, made_before: bool, now_ms: i64) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.restoring = false;
        self.last_try_ms = now_ms;
        let (running, out) = (self.running.clone(), self.out.clone());
        let _ = std::thread::Builder::new().name("leaf-booksync".into()).spawn(move || {
            let o = copy(&client, &server, &data, made_before).unwrap_or_else(Outcome::Failed);
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(o);
            running.store(false, Ordering::SeqCst);
            crate::wake::wake();
        });
    }

    /// Fetch every file of the copy on the server.
    pub fn restore(&mut self, client: Client, server: Server, data: PathBuf) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.restoring = true;
        let (running, out) = (self.running.clone(), self.out.clone());
        let _ = std::thread::Builder::new().name("leaf-booksync".into()).spawn(move || {
            let o = restore(&client, &server, &data).unwrap_or_else(Outcome::Failed);
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(o);
            running.store(false, Ordering::SeqCst);
            crate::wake::wake();
        });
    }

    pub fn last_try_ms(&self) -> i64 {
        self.last_try_ms
    }
}

fn fail_words(f: Fail, server: &Server) -> String {
    f.words(&server.base)
}

fn copy(client: &Client, server: &Server, data: &Path, made_before: bool) -> Result<Outcome, String> {
    let remote: Vec<PhoneFile> = client.json(server, "/v1/phone/files").map_err(|f| fail_words(f, server))?;
    if !made_before && let Some(f) = remote.iter().find(|f| f.kind == "shelf" && f.name == "shelf.db") {
        return Ok(Outcome::Found(f.at_ms));
    }
    // The shelf as it stands, read beside the shelf's own connection (WAL lets both).
    let snap = data.join("shelf.copy.db");
    let _ = std::fs::remove_file(&snap);
    let (files, covers) = {
        let c = rusqlite::Connection::open_with_flags(data.join("shelf.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| format!("Your books couldn't be read: {e}"))?;
        c.execute("VACUUM INTO ?1", [snap.to_string_lossy()]).map_err(|e| format!("Your books couldn't be copied: {e}"))?;
        let mut st = c.prepare("SELECT file, id, cover FROM books").map_err(|e| e.to_string())?;
        let rows: Vec<(String, i64, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
        (rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>(), rows.iter().filter(|r| r.2 != 0).map(|r| r.1).collect::<Vec<_>>())
    };
    let books = files.len();
    let mut local: Vec<(&'static str, String, u64)> = Vec::new();
    for f in files {
        if let Ok(m) = std::fs::metadata(data.join("books").join(&f)) {
            local.push(("books", f, m.len()));
        }
    }
    for id in covers {
        let name = cover_name(id);
        if let Ok(m) = std::fs::metadata(data.join("covers").join(&name)) {
            local.push(("covers", name, m.len()));
        }
    }
    let (send, too_big) = to_send(&local, &remote);
    for (kind, name) in send {
        client.put_file(server, &files_path(kind, &name), &data.join(kind).join(&name)).map_err(|f| fail_words(f, server))?;
    }
    // The shelf last: a copy that stops halfway names no book the server lacks.
    let r = client.put_file(server, &files_path("shelf", "shelf.db"), &snap);
    let _ = std::fs::remove_file(&snap);
    r.map_err(|f| fail_words(f, server))?;
    Ok(Outcome::Copied { books, too_big })
}

fn restore(client: &Client, server: &Server, data: &Path) -> Result<Outcome, String> {
    let remote: Vec<PhoneFile> = client.json(server, "/v1/phone/files").map_err(|f| fail_words(f, server))?;
    if !remote.iter().any(|f| f.kind == "shelf" && f.name == "shelf.db") {
        return Err("Your server holds no copy of your books".into());
    }
    let got = std::sync::atomic::AtomicU64::new(0);
    // Books and covers first, the ones not here already; the shelf last.
    for f in remote.iter().filter(|f| f.kind != "shelf").chain(remote.iter().filter(|f| f.kind == "shelf" && f.name == "shelf.db")) {
        let to = local_path(data, &f.kind, &f.name);
        if f.kind != "shelf" && std::fs::metadata(&to).is_ok_and(|m| m.len() == f.size) {
            continue;
        }
        client.download(server, &files_path(&f.kind, &f.name), &to, &got).map_err(|e| fail_words(e, server))?;
    }
    Ok(Outcome::Restored(data.join("shelf.restore.db")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(kind: &str, name: &str, size: u64) -> PhoneFile {
        PhoneFile { kind: kind.into(), name: name.into(), size, at_ms: 0 }
    }

    #[test]
    fn only_what_the_server_lacks_is_sent_and_never_a_book_too_big_for_the_way_there() {
        let local = vec![("books", "a.epub".to_string(), 10), ("books", "b.epub".to_string(), 20), ("books", "big.pdf".to_string(), MAX_FILE + 1), ("covers", "c-k1".to_string(), 5)];
        let remote = vec![kept("books", "a.epub", 10), kept("books", "b.epub", 19), kept("shelf", "shelf.db", 3)];
        let (send, too_big) = to_send(&local, &remote);
        assert_eq!(send, vec![("books", "b.epub".to_string()), ("covers", "c-k1".to_string())], "a changed size goes again");
        assert_eq!(too_big, 1);
    }

    #[test]
    fn a_copy_is_made_once_a_day_with_books_and_not_again_within_the_hour_after_a_failure() {
        let day = EVERY_MS;
        assert!(due(Copied::default(), 0, day * 10, 1));
        assert!(!due(Copied::default(), 0, day * 10, 0), "no books, nothing to keep");
        assert!(!due(Copied { at_ms: day * 10 - 1000, books: 1 }, 0, day * 10, 1), "made today");
        assert!(!due(Copied::default(), day * 10 - 1000, day * 10, 1), "tried a moment ago");
    }

    #[test]
    fn a_cover_is_named_as_the_phone_keeps_it() {
        assert_eq!(cover_name(12), "c-k12");
    }
}
