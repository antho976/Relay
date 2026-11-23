//! Talking to leaf-server, always from a worker thread: the HTTP client, the jobs the screens ask
//! for (JSON in and out), and the copies of the library and each series kept on disk, so the app
//! shows the last ones it had when the server can't be reached.
//!
//! A job runs on one of a few worker threads and answers through a channel; the worker then wakes
//! the app's loop ([`crate::wake`]), which takes the answers at the start of its next frame.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use leaf_proto::{Follow, Pages, Progress, Series, SeriesDetail, Status};
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
                        let next = tasks.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok((server, job)) = next else { break };
                        let send = |d: Done| {
                            let _ = done_tx.send(d);
                            crate::wake::wake();
                        };
                        // A job that panics (it should not) still ends: the app hears nothing,
                        // and the worker goes on.
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&client, &data, server, job, &send)));
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

fn series_file(data: &Path, id: i64) -> PathBuf {
    data.join("series").join(format!("{id}.json"))
}

/// Write a file whole or not at all: a crash halfway leaves the old copy.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
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
                Some(s) => serde_json::to_string(&p)
                    .map_err(|_| Fail::Garbled)
                    .and_then(|body| client.post(s, &format!("/v1/chapters/{chapter}/progress"), body))
                    .map(|_| ()),
                None => no_server(),
            };
            send(Done::Progress(chapter, r));
        }
        Job::PageCount(chapter) => {
            let r = match &server {
                Some(s) => client.json::<Pages>(s, &format!("/v1/chapters/{chapter}/pages")).map(|p| p.count),
                None => no_server(),
            };
            send(Done::PageCount(chapter, r));
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
    fn failures_read_as_words() {
        assert_eq!(Fail::Unreachable.words("192.168.1.5:4580"), "Can't reach your server at 192.168.1.5:4580");
        assert_eq!(Fail::Answer(502, "suwayomi: timed out".into()).words("x"), "Your server says: suwayomi: timed out");
    }
}
