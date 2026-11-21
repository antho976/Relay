//! Files in and out of the setup pages, and work that must not run on the UI thread.
//!
//! - A picked file comes back from the platform on its own thread (Android's activity result, the
//!   desktop's `TALLY_PICK`). It is left in [`deliver`]'s inbox; the Android loop moves it into
//!   [`App::file_picked`] and wakes, and [`take_picked`] (what a page calls each frame) drains the
//!   inbox too, so a page reads a picked file the same way on either platform.
//! - Each pick is numbered ([`begin_pick`]) and the platform hands the number back with the file
//!   (Android in the request code, [`pick_from_tag`]): a file that answers an earlier pick, its
//!   page gone or still reading slowly from a cloud drive, is dropped, never handed to the next,
//!   unrelated pick (a statement restored as a backup, a backup read as a statement).
//! - A pick the person cancels is said too ([`cancel`]): the page that asked takes it
//!   ([`take_cancelled`]) and stops waiting, so it does not take the next page's file.
//! - [`Job`] runs a closure on a thread (reading a big CSV, encoding a backup) and is polled each
//!   frame until it answers.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::app::App;

/// The largest file a page reads: a bank statement or a backup is far smaller.
pub const MAX_PICK_BYTES: usize = 32 * 1024 * 1024;

/// The file delivered, with the number of the pick it answers.
static INBOX: Mutex<Option<(u64, String, Vec<u8>)>> = Mutex::new(None);

/// The latest pick's number (0 before any: a file a stopped process asked for answers that).
/// Always within [`PICK_TAG_MASK`], so the tag a platform carries is the whole number.
static PICK: AtomicU64 = AtomicU64::new(0);

/// The pick the person cancelled, until its page takes it ([`take_cancelled`]).
static CANCELLED: Mutex<Option<u64>> = Mutex::new(None);

/// The bits of a pick's number a platform carries with it (Android's request code, which must
/// stay within 16 bits: TallyActivity's PICK_MASK, the same number).
pub const PICK_TAG_MASK: u64 = 0x3fff;

/// A new pick goes out (the platform's `Host::pick_file`): what an earlier one left, delivered or
/// still to come, is not for it. Returns its number.
pub fn begin_pick() -> u64 {
    if let Ok(mut inbox) = INBOX.lock() {
        *inbox = None;
    }
    if let Ok(mut c) = CANCELLED.lock() {
        *c = None;
    }
    // A process starts its numbering at a random number, not at 1: a stopped process's picks were
    // numbered from 1 too, and a file one of them asked for, read slowly after the restart, would
    // otherwise carry the same tag as this process's first pick and be taken for it.
    let seed = getrandom::u64().unwrap_or_else(|_| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64) ^ u64::from(std::process::id())
    });
    let mut cur = PICK.load(Ordering::SeqCst);
    loop {
        let next = next_pick(cur, seed);
        match PICK.compare_exchange(cur, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(now) => cur = now,
        }
    }
}

/// The number after `cur`, within 1..=[`PICK_TAG_MASK`] (0 is "nothing asked yet"): the first
/// comes from `seed`, then they count up and wrap past the mask back to 1.
pub fn next_pick(cur: u64, seed: u64) -> u64 {
    if cur == 0 { seed % PICK_TAG_MASK + 1 } else { cur % PICK_TAG_MASK + 1 }
}

/// The pick a platform's tag (the number's low bits, [`PICK_TAG_MASK`]) answers: the latest
/// pick, or none when it answers an earlier one. Before this process has asked for anything
/// (it was started again with a picker out), it answers what the stopped process asked for:
/// pick 0, as android.rs hands such a file to the page that asks for its kind.
pub fn pick_from_tag(tag: u64) -> Option<u64> {
    let now = current_pick();
    (now == 0 || now & PICK_TAG_MASK == tag & PICK_TAG_MASK).then_some(now)
}

/// The latest pick's number.
pub fn current_pick() -> u64 {
    PICK.load(Ordering::SeqCst)
}

/// A picked file arrived for the pick out now (the desktop, which answers as it asks). An empty
/// `bytes` means it could not be read.
pub fn deliver(name: String, bytes: Vec<u8>) {
    deliver_for(current_pick(), name, bytes);
}

/// A picked file arrived (any thread), for the pick numbered `pick`. An empty `bytes` means it
/// could not be read. One for an earlier pick is dropped.
pub fn deliver_for(pick: u64, name: String, bytes: Vec<u8>) {
    if pick != current_pick() {
        return;
    }
    if let Ok(mut inbox) = INBOX.lock() {
        // Kept with its number: a pick that starts before a page takes it drops it.
        *inbox = Some((pick, name, bytes));
    }
}

/// The picker for pick `pick` came back with no file (any thread). One for an earlier pick
/// changes nothing.
pub fn cancel(pick: u64) {
    if pick != current_pick() {
        return;
    }
    if let Ok(mut c) = CANCELLED.lock() {
        *c = Some(pick);
    }
}

/// True once when the latest pick was cancelled: the page that asked stops waiting for it.
pub fn take_cancelled() -> bool {
    let Ok(mut c) = CANCELLED.lock() else { return false };
    let now = current_pick();
    c.take_if(|p| *p == now).is_some()
}

/// What [`deliver`] left, once, with the number of the pick it answers.
pub fn take_delivered() -> Option<(u64, String, Vec<u8>)> {
    INBOX.lock().ok().and_then(|mut inbox| inbox.take())
}

/// The file the person picked, once: whatever reached the app or is still in the inbox. A file
/// that answers an earlier pick is dropped.
pub fn take_picked(app: &mut App) -> Option<(String, Vec<u8>)> {
    if let Some((pick, name, bytes)) = take_delivered() {
        app.file_delivered(pick, name, bytes);
    }
    let file = app.picked.take()?;
    (app.picked_for == current_pick()).then_some(file)
}

/// Work on a thread of its own, polled from the frame.
pub struct Job<T> {
    rx: Receiver<T>,
}

/// Where a job stands.
pub enum Poll<T> {
    Running,
    Done(T),
    /// The thread died without answering (a panic): say so rather than wait for ever.
    Lost,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Job<T> {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Job { rx }
    }

    pub fn poll(&self) -> Poll<T> {
        match self.rx.try_recv() {
            Ok(v) => Poll::Done(v),
            Err(TryRecvError::Empty) => Poll::Running,
            Err(TryRecvError::Disconnected) => Poll::Lost,
        }
    }

    /// Waits for the answer: for tests only, never from a frame.
    pub fn wait(self) -> Option<T> {
        self.rx.recv().ok()
    }
}

/// Writes a file to be shared (Android's share sheet) under `shared` (the cache's folder the
/// FileProvider lends), in a folder of its own: a second share of the same name (two exports the
/// same day) never rewrites the file behind the first. Older shares' folders are deleted, all but
/// the latest, which an app the person shared with a moment ago may still be reading; files from
/// before shares had folders go too. Returns the file's path.
pub fn write_share(shared: &std::path::Path, name: &str, bytes: &[u8]) -> std::io::Result<std::path::PathBuf> {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_".contains(c) { c } else { '-' }).collect();
    // ".." would climb out of the folder.
    let safe = if safe.is_empty() || safe.chars().all(|c| c == '.') { "tally-file".to_string() } else { safe };
    std::fs::create_dir_all(shared)?;
    // Each share's folder is numbered, so the latest is the largest number.
    let mut old: Vec<(u64, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(shared)?.flatten() {
        let path = entry.path();
        match entry.file_name().to_str().and_then(|n| n.parse::<u64>().ok()) {
            Some(n) if path.is_dir() => old.push((n, path)),
            _ if path.is_dir() => {}
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    old.sort();
    let latest = old.last().map_or(0, |(n, _)| *n);
    // The latest share is kept; the rest go.
    old.pop();
    for (_, path) in old {
        let _ = std::fs::remove_dir_all(path);
    }
    // The clock's milliseconds, or one past the latest if the clock stands behind it.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let dir = shared.join(now.max(latest + 1).to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(safe);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Today as the backup and export file names carry it: "2026-11-19".
pub fn stamp_today() -> String {
    crate::model::today().to_string()
}
