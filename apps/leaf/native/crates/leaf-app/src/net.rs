//! Talking to leaf-server, always from a worker thread: the HTTP client, the jobs the screens ask
//! for (JSON in and out), and the copies of the library and each series kept on disk, so the app
//! shows the last ones it had when the server can't be reached.
//!
//! A job runs on one of a few worker threads and answers through a channel; the worker then wakes
//! the app's loop ([`crate::wake`]), which takes the answers at the start of its next frame.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use leaf_proto::{Alternative, AnilistToken, Bell, ChapterRead, Follow, Pages, Progress, ReadBefore, Repin, Series, SeriesDetail, ShelfSet, SourceHealth, Status, Update};
use serde::{Deserialize, Serialize};

/// Where the server is, and its token if it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Server {
    /// "http://192.168.1.20:4580", as [`crate::logic::normalize_server`] makes it.
    pub base: String,
    pub token: Option<String>,
}

/// Why a request failed, in the words the app shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Fail {
    /// No answer at all: off, asleep, another network.
    Unreachable,
    /// The server wants a token, or another one.
    Token,
    /// It answered, with an error.
    Answer(u16, String),
    /// It answered something that isn't what Leaf asked for.
    Garbled,
}

impl Fail {
    pub fn words(&self, server: &str) -> String {
        match self {
            Fail::Unreachable => format!("Can't reach your server at {server}"),
            Fail::Token => "Your server wants its token. Check it in Settings.".into(),
            Fail::Answer(404, _) => "Your server doesn't have that any more".into(),
            Fail::Answer(code, msg) if msg.is_empty() => format!("Your server answered with an error ({code})"),
            Fail::Answer(_, msg) => format!("Your server says: {msg}"),
            Fail::Garbled => format!("The server at {server} doesn't answer like Leaf's server"),
        }
    }
}

/// The HTTP client, shared by every worker.
#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
}

impl Default for Client {
    fn default() -> Self {
        Client::new()
    }
}

impl Client {
    pub fn new() -> Client {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(6)))
            // A search asks every source in turn; give it the time that takes.
            .timeout_global(Some(Duration::from_secs(90)))
            .http_status_as_error(false)
            .build()
            .into();
        Client { agent }
    }

    fn url(s: &Server, path: &str) -> String {
        format!("{}{path}", s.base.trim_end_matches('/'))
    }

    fn read(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Vec<u8>, Fail> {
        let mut r = r.map_err(|_| Fail::Unreachable)?;
        let status = r.status().as_u16();
        let body = r.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|_| Fail::Unreachable)?;
        match status {
            200..=299 => Ok(body),
            401 | 403 => Err(Fail::Token),
            code => {
                let msg = serde_json::from_slice::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                    .unwrap_or_default();
                Err(Fail::Answer(code, msg))
            }
        }
    }

    /// A file: its bytes, its name (from `Content-Disposition`) and its type.
    pub fn file(&self, s: &Server, path: &str, post: bool) -> Result<BackupFile, Fail> {
        let url = Client::url(s, path);
        let r = if post {
            let mut req = self.agent.post(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.send_empty()
        } else {
            let mut req = self.agent.get(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        };
        let r = r.map_err(|_| Fail::Unreachable)?;
        let header = |name: &str| r.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let name = header("content-disposition").and_then(|h| crate::logic::disposition_name(&h)).unwrap_or_else(|| "leaf-backup.json".into());
        let mime = header("content-type").map(|m| m.split(';').next().unwrap_or("").trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "application/json".into());
        let bytes = Client::read(Ok(r))?;
        Ok(BackupFile { name, mime, bytes })
    }

    pub fn get(&self, s: &Server, path: &str) -> Result<Vec<u8>, Fail> {
        let mut req = self.agent.get(Client::url(s, path));
        if let Some(t) = &s.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        Client::read(req.call())
    }

    pub fn post(&self, s: &Server, path: &str, body: String) -> Result<Vec<u8>, Fail> {
        let mut req = self.agent.post(Client::url(s, path)).header("content-type", "application/json");
        if let Some(t) = &s.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        Client::read(req.send(body))
    }

    pub fn delete(&self, s: &Server, path: &str) -> Result<Vec<u8>, Fail> {
        let mut req = self.agent.delete(Client::url(s, path));
        if let Some(t) = &s.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        Client::read(req.call())
    }

    pub fn json<T: for<'de> Deserialize<'de>>(&self, s: &Server, path: &str) -> Result<T, Fail> {
        serde_json::from_slice(&self.get(s, path)?).map_err(|_| Fail::Garbled)
    }
}

/// `q` made safe for a query string.
pub fn query_escape(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for b in q.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A backup as the server handed it.
#[derive(Debug, Clone)]
pub struct BackupFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// What a screen asks the server.
#[derive(Clone, Debug)]
pub enum Job {
    Status(Server),
    Library,
    Series(i64),
    Search(String),
    Follow(Follow),
    Unfollow(i64),
    Progress(i64, Progress),
    PageCount(i64),
    /// Turn a series' bell on or off (`POST /v1/series/{id}/bell`).
    Bell(i64, bool),
    /// Download a chapter on the server, or remove it (`POST` / `DELETE
    /// /v1/chapters/{id}/download`).
    Download(i64, bool),
    /// Chapters that came lately for series in My space, over this many days (`/v1/updates`).
    Updates(u32),
    /// A chapter read or unread (`POST /v1/chapters/{id}/read`).
    ChapterRead(i64, bool),
    /// Every chapter of a series numbered below this, read (`POST /v1/series/{id}/read`).
    ReadBefore(i64, f64),
    /// A series put on a shelf (`POST /v1/series/{id}/shelf`).
    Shelf(i64, Option<String>),
    /// How each source is doing (`/v1/health`).
    Health,
    /// The latest backup (`GET /v1/backup`), or one made now (`POST`).
    Backup(bool),
    /// An AniList token kept on the server, or removed (`POST /v1/settings/anilist`).
    Anilist(Option<String>),
    /// The other entries for a series' work (`/v1/series/{id}/alternatives`; slow).
    Alternatives(i64),
    /// Read a series from another entry (`POST /v1/series/{id}/repin`).
    Repin(i64, i64),
}

impl Job {
    /// It changes something on the server (a screenshot never sends these).
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Job::Follow(_) | Job::Unfollow(_) | Job::Progress(..) | Job::Bell(..) | Job::Download(..) | Job::ChapterRead(..) | Job::ReadBefore(..) | Job::Shelf(..) | Job::Backup(true) | Job::Anilist(_) | Job::Repin(..)
        )
    }
}

/// An answer. `cached` ones come from the copy on disk, before (or instead of) the server's.
#[derive(Debug)]
pub enum Done {
    Status(Server, Result<Status, Fail>),
    Library { cached: bool, result: Result<Vec<Series>, Fail> },
    Series { id: i64, cached: bool, result: Result<SeriesDetail, Fail> },
    Search(String, Result<Vec<leaf_proto::Candidate>, Fail>),
    Followed(Result<Series, Fail>),
    Unfollowed(i64, Result<(), Fail>),
    Progress(i64, Result<(), Fail>),
    PageCount(i64, Result<u32, Fail>),
    Bell(i64, bool, Result<(), Fail>),
    Download(i64, bool, Result<(), Fail>),
    Updates { cached: bool, result: Result<Vec<Update>, Fail> },
    ChapterRead(i64, bool, Result<(), Fail>),
    ReadBefore(i64, Result<(), Fail>),
    Shelf(i64, Result<(), Fail>),
    Health(Result<Vec<SourceHealth>, Fail>),
    Backup(Result<BackupFile, Fail>),
    /// Whether a token was set (true) or removed.
    Anilist(bool, Result<(), Fail>),
    Alternatives(i64, Result<Vec<Alternative>, Fail>),
    Repinned(i64, Result<Series, Fail>),
}

type Task = (Option<Server>, Job);

/// The workers and their answers.
pub struct Jobs {
    tx: Sender<Task>,
    rx: Receiver<Done>,
    running: Arc<AtomicUsize>,
}

/// How many requests may be out at once: a slow search never holds up the library.
const WORKERS: usize = 3;

impl Jobs {
    pub fn new(data: PathBuf) -> Jobs {
        let (tx, tasks) = channel::<Task>();
        let (done_tx, rx) = channel::<Done>();
        let tasks = Arc::new(Mutex::new(tasks));
        let running = Arc::new(AtomicUsize::new(0));
        let client = Client::new();
        for i in 0..WORKERS {
            let (tasks, done_tx, client, data, running) = (tasks.clone(), done_tx.clone(), client.clone(), data.clone(), running.clone());
            std::thread::Builder::new()
                .name(format!("leaf-net-{i}"))
                .spawn(move || {
                    loop {
                        let next = {
                            // Counted before the queue is let go: a fetch taken after this report
                            // always sees it out. Only reports running count, so a fetch never
                            // waits on one still queued behind it.
                            let queue = tasks.lock().unwrap_or_else(|e| e.into_inner());
                            let next = queue.recv();
                            if let Ok((_, Job::Progress(..))) = &next {
                                *PROGRESS_OUT.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                            }
                            next
                        };
                        let Ok((server, job)) = next else { break };
                        let send = |d: Done| {
                            let _ = done_tx.send(d);
                            crate::wake::wake();
                        };
                        // A job that panics (it should not) still ends: the app hears nothing,
                        // and the worker goes on.
                        let progress = matches!(job, Job::Progress(..));
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&client, &data, server, job, &send)));
                        if progress {
                            let mut out = PROGRESS_OUT.lock().unwrap_or_else(|e| e.into_inner());
                            *out = out.saturating_sub(1);
                            PROGRESS_LANDED.notify_all();
                        }
                        running.fetch_sub(1, Ordering::SeqCst);
                        crate::wake::wake();
                    }
                })
                .expect("a network thread starts");
        }
        Jobs { tx, rx, running }
    }

    /// Ask, of `server` (none: the app has no server yet, and the job answers from disk alone).
    pub fn send(&self, server: Option<&Server>, job: Job) {
        self.running.fetch_add(1, Ordering::SeqCst);
        if self.tx.send((server.cloned(), job)).is_err() {
            self.running.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Every answer that came since the last call.
    pub fn take(&self) -> Vec<Done> {
        self.rx.try_iter().collect()
    }

    /// Jobs asked for and not finished.
    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }
}

fn library_file(data: &Path) -> PathBuf {
    data.join("library.json")
}

// Progress, in order and never lost

/// Progress reports being sent. A library or series fetched while one is out
/// would race it (the server's "done" takes a while) and show the chapter just read as unread, so
/// those fetches wait for them first.
static PROGRESS_OUT: Mutex<usize> = Mutex::new(0);
static PROGRESS_LANDED: Condvar = Condvar::new();

/// Waits, a while at most, until no progress report is out.
fn wait_for_progress() {
    let out = PROGRESS_OUT.lock().unwrap_or_else(|e| e.into_inner());
    let _ = PROGRESS_LANDED.wait_timeout_while(out, Duration::from_secs(20), |n| *n > 0);
}

/// Reports the server did not take (offline, or it failed): sent again before the next library or
/// series is fetched. One per chapter, the latest; a chapter once finished stays finished.
static OUTBOX: Mutex<()> = Mutex::new(());

fn outbox_file(data: &Path) -> PathBuf {
    data.join("progress-outbox.json")
}

fn outbox_read(data: &Path) -> Vec<(i64, Progress)> {
    std::fs::read(outbox_file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn outbox_write(data: &Path, list: &[(i64, Progress)]) {
    let file = outbox_file(data);
    let _ = if list.is_empty() { std::fs::remove_file(&file) } else { write_atomic(&file, &serde_json::to_vec(list).unwrap_or_default()) };
}

/// The report kept for later: merged with one kept for the same chapter.
pub fn outbox_put(data: &Path, chapter: i64, p: Progress) {
    let _g = OUTBOX.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = outbox_read(data);
    match list.iter_mut().find(|(c, _)| *c == chapter) {
        Some((_, old)) => *old = Progress { page: if old.done && !p.done { old.page } else { p.page }, done: old.done || p.done },
        None => list.push((chapter, p)),
    }
    outbox_write(data, &list);
}

/// A later report for the chapter went through: one kept for it is moot.
fn outbox_drop(data: &Path, chapter: i64) {
    if !outbox_file(data).exists() {
        return;
    }
    let _g = OUTBOX.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = outbox_read(data);
    let n = list.len();
    list.retain(|(c, _)| *c != chapter);
    if list.len() != n {
        outbox_write(data, &list);
    }
}

/// Worth sending again: the server was out of reach, or failed itself (not "no such chapter").
fn retry_later(f: &Fail) -> bool {
    matches!(f, Fail::Unreachable | Fail::Token) || matches!(f, Fail::Answer(code, _) if *code >= 500)
}

fn post_progress(client: &Client, s: &Server, chapter: i64, p: &Progress) -> Result<(), Fail> {
    let body = serde_json::to_string(p).map_err(|_| Fail::Garbled)?;
    client.post(s, &format!("/v1/chapters/{chapter}/progress"), body).map(|_| ())
}

/// Sends what the outbox holds, oldest first; stops at the first that can't get through.
fn flush_outbox(client: &Client, data: &Path, s: &Server) {
    if !outbox_file(data).exists() {
        return;
    }
    let _g = OUTBOX.lock().unwrap_or_else(|e| e.into_inner());
    let list = outbox_read(data);
    let mut left = Vec::new();
    for (i, (c, p)) in list.iter().enumerate() {
        match post_progress(client, s, *c, p) {
            Ok(()) => {}
            Err(f) if retry_later(&f) => {
                left.extend_from_slice(&list[i..]);
                break;
            }
            Err(_) => {}
        }
    }
    outbox_write(data, &left);
}

fn updates_file(data: &Path) -> PathBuf {
    data.join("updates.json")
}

/// POST `body` as JSON, the answer dropped.
fn post_json<T: Serialize>(client: &Client, server: &Option<Server>, path: &str, body: &T) -> Result<Vec<u8>, Fail> {
    let Some(s) = server else { return no_server() };
    let body = serde_json::to_string(body).map_err(|_| Fail::Garbled)?;
    client.post(s, path, body)
}

fn series_file(data: &Path, id: i64) -> PathBuf {
    data.join("series").join(format!("{id}.json"))
}

/// Write a file whole or not at all: a crash halfway leaves the old copy. Each write has its own
/// temporary file, so two threads writing the same file never mix their bytes.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("part{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// No server set yet: nothing to ask.
fn no_server<T>() -> Result<T, Fail> {
    Err(Fail::Unreachable)
}

fn run(client: &Client, data: &Path, server: Option<Server>, job: Job, send: &dyn Fn(Done)) {
    match job {
        Job::Status(s) => {
            let r = client.json::<Status>(&s, "/v1/status");
            send(Done::Status(s, r));
        }
        Job::Library => {
            let file = library_file(data);
            if let Some(list) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Vec<Series>>(&b).ok()) {
                send(Done::Library { cached: true, result: Ok(list) });
            }
            let Some(s) = server else { return send(Done::Library { cached: false, result: no_server() }) };
            wait_for_progress();
            flush_outbox(client, data, &s);
            let r = client.get(&s, "/v1/library").and_then(|b| {
                let list = serde_json::from_slice::<Vec<Series>>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok(list)
            });
            send(Done::Library { cached: false, result: r });
        }
        Job::Series(id) => {
            let file = series_file(data, id);
            if let Some(d) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<SeriesDetail>(&b).ok()) {
                send(Done::Series { id, cached: true, result: Ok(d) });
            }
            let Some(s) = server else { return send(Done::Series { id, cached: false, result: no_server() }) };
            wait_for_progress();
            flush_outbox(client, data, &s);
            let r = client.get(&s, &format!("/v1/series/{id}")).and_then(|b| {
                let d = serde_json::from_slice::<SeriesDetail>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok(d)
            });
            send(Done::Series { id, cached: false, result: r });
        }
        Job::Search(q) => {
            let r = match &server {
                Some(s) => client.json(s, &format!("/v1/search?q={}", query_escape(&q))),
                None => no_server(),
            };
            send(Done::Search(q, r));
        }
        Job::Follow(f) => {
            let r = match &server {
                Some(s) => serde_json::to_string(&f)
                    .map_err(|_| Fail::Garbled)
                    .and_then(|body| client.post(s, "/v1/follow", body))
                    .and_then(|b| serde_json::from_slice::<Series>(&b).map_err(|_| Fail::Garbled)),
                None => no_server(),
            };
            send(Done::Followed(r));
        }
        Job::Unfollow(id) => {
            let r = match &server {
                Some(s) => client.delete(s, &format!("/v1/series/{id}")).map(|_| ()),
                None => no_server(),
            };
            if r.is_ok() {
                let _ = std::fs::remove_file(series_file(data, id));
            }
            send(Done::Unfollowed(id, r));
        }
        Job::Progress(chapter, p) => {
            // Read now: the page cache keeps this chapter a week from today.
            crate::images::touch_chapter(data, chapter);
            let r = match &server {
                Some(s) => post_progress(client, s, chapter, &p),
                None => no_server(),
            };
            // Kept and sent again later, rather than lost: reading offline still counts.
            match &r {
                Ok(()) => outbox_drop(data, chapter),
                Err(f) if retry_later(f) => outbox_put(data, chapter, p),
                Err(_) => {}
            }
            send(Done::Progress(chapter, r));
        }
        Job::PageCount(chapter) => {
            let r = match &server {
                Some(s) => client.json::<Pages>(s, &format!("/v1/chapters/{chapter}/pages")).map(|p| p.count),
                None => no_server(),
            };
            send(Done::PageCount(chapter, r));
        }
        Job::Bell(id, on) => {
            let r = match &server {
                Some(s) => serde_json::to_string(&Bell { on }).map_err(|_| Fail::Garbled).and_then(|body| client.post(s, &format!("/v1/series/{id}/bell"), body)).map(|_| ()),
                None => no_server(),
            };
            send(Done::Bell(id, on, r));
        }
        Job::Download(chapter, on) => {
            let path = format!("/v1/chapters/{chapter}/download");
            let r = match &server {
                Some(s) if on => client.post(s, &path, String::new()).map(|_| ()),
                Some(s) => client.delete(s, &path).map(|_| ()),
                None => no_server(),
            };
            send(Done::Download(chapter, on, r));
        }
        Job::Updates(days) => {
            let file = updates_file(data);
            if let Some(list) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Vec<Update>>(&b).ok()) {
                send(Done::Updates { cached: true, result: Ok(list) });
            }
            let Some(s) = server else { return send(Done::Updates { cached: false, result: no_server() }) };
            wait_for_progress();
            let r = client.get(&s, &format!("/v1/updates?days={days}")).and_then(|b| {
                let list = serde_json::from_slice::<Vec<Update>>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok(list)
            });
            send(Done::Updates { cached: false, result: r });
        }
        Job::ChapterRead(chapter, read) => {
            let r = post_json(client, &server, &format!("/v1/chapters/{chapter}/read"), &ChapterRead { read }).map(|_| ());
            send(Done::ChapterRead(chapter, read, r));
        }
        Job::ReadBefore(id, before) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/read"), &ReadBefore { before }).map(|_| ());
            send(Done::ReadBefore(id, r));
        }
        Job::Shelf(id, shelf) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/shelf"), &ShelfSet { shelf }).map(|_| ());
            send(Done::Shelf(id, r));
        }
        Job::Health => {
            let r = match &server {
                Some(s) => client.json(s, "/v1/health"),
                None => no_server(),
            };
            send(Done::Health(r));
        }
        Job::Backup(now) => {
            let r = match &server {
                Some(s) => client.file(s, "/v1/backup", now),
                None => no_server(),
            };
            send(Done::Backup(r));
        }
        Job::Anilist(token) => {
            let on = token.is_some();
            let r = post_json(client, &server, "/v1/settings/anilist", &AnilistToken { token }).map(|_| ());
            send(Done::Anilist(on, r));
        }
        Job::Alternatives(id) => {
            let r = match &server {
                Some(s) => client.json(s, &format!("/v1/series/{id}/alternatives")),
                None => no_server(),
            };
            send(Done::Alternatives(id, r));
        }
        Job::Repin(id, manga_id) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/repin"), &Repin { manga_id }).and_then(|b| serde_json::from_slice::<Series>(&b).map_err(|_| Fail::Garbled));
            send(Done::Repinned(id, r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_escaped() {
        assert_eq!(query_escape("nano machine"), "nano+machine");
        assert_eq!(query_escape("a&b=c/é"), "a%26b%3Dc%2F%C3%A9");
    }

    #[test]
    fn the_outbox_keeps_the_latest_and_never_unfinishes() {
        let dir = std::env::temp_dir().join(format!("leaf-outbox-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        outbox_put(&dir, 7, Progress { page: 3, done: false });
        outbox_put(&dir, 7, Progress { page: 9, done: false });
        outbox_put(&dir, 8, Progress { page: 29, done: true });
        outbox_put(&dir, 8, Progress { page: 2, done: false });
        assert_eq!(outbox_read(&dir), vec![(7, Progress { page: 9, done: false }), (8, Progress { page: 29, done: true })]);
        outbox_drop(&dir, 7);
        assert_eq!(outbox_read(&dir), vec![(8, Progress { page: 29, done: true })]);
        outbox_drop(&dir, 8);
        assert!(!outbox_file(&dir).exists(), "an empty outbox leaves no file");
        assert!(retry_later(&Fail::Unreachable) && retry_later(&Fail::Answer(502, String::new())));
        assert!(!retry_later(&Fail::Answer(404, String::new())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failures_read_as_words() {
        assert_eq!(Fail::Unreachable.words("192.168.1.5:4580"), "Can't reach your server at 192.168.1.5:4580");
        assert_eq!(Fail::Answer(502, "suwayomi: timed out".into()).words("x"), "Your server says: suwayomi: timed out");
    }
}
