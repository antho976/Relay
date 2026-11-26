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

use leaf_proto::{Alternative, AnilistToken, AppLatest, Bell, ChapterRead, Follow, PaidAhead, Pages, Progress, ReadBefore, Rec, RecTag, Recs, Repin, Series, SeriesDetail, ShelfSet, SourceHealth, Status, Update};
use serde::{Deserialize, Serialize};

/// Where the server is, and its token if it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Server {
    /// "http://192.168.1.20:4580", as [`crate::logic::normalize_server`] makes it.
    pub base: String,
    pub token: Option<String>,
    /// A second address for the same server (its Tailscale one), asked when `base` can't be
    /// reached; none here falls back to the one built in ([`ALT_BASE`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
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
    /// The server could not be reached: no answer at all, or a proxy in front of it answering for
    /// it that it is down (a gateway error with none of the server's own words).
    pub fn out_of_reach(&self) -> bool {
        match self {
            Fail::Unreachable => true,
            Fail::Answer(code, msg) => msg.is_empty() && matches!(code, 502..=504 | 520..=530),
            _ => false,
        }
    }

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

type Resp = ureq::http::Response<ureq::Body>;

/// The server's second address, built in (`LEAF_ALT_BASE` at build time, e.g. its Tailscale
/// address): asked when the first can't be reached. None in a build without one.
pub const ALT_BASE: Option<&str> = option_env!("LEAF_ALT_BASE");
/// How long the second address is asked first once it answered for the first.
const ALT_STICK_MS: i64 = 10 * 60 * 1000;
/// Until when (ms since the epoch) the second address is asked first.
static PREFER_ALT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The second address for this server, when there is one and it isn't this one.
fn alt_for(s: &Server) -> Option<String> {
    s.alt.as_deref().or(ALT_BASE).map(str::trim).filter(|a| !a.is_empty() && a.trim_end_matches('/') != s.base.trim_end_matches('/')).map(str::to_string)
}

/// A gateway's own "the server behind me didn't answer" (502–504, Cloudflare's 520–530).
fn gateway(code: u16) -> bool {
    matches!(code, 502..=504 | 520..=530)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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

    /// Sends a request to the server's address, or to its second one ([`ALT_BASE`], over
    /// Tailscale) when the first can't be reached (a network that blocks it): the one that
    /// answered is asked first for the next [`ALT_STICK_MS`].
    fn go(&self, s: &Server, path: &str, send: impl Fn(String) -> Result<Resp, ureq::Error>) -> Result<Resp, Fail> {
        let Some(alt) = alt_for(s) else { return send(Client::url(s, path)).map_err(|_| Fail::Unreachable) };
        let alt_first = PREFER_ALT.load(Ordering::Relaxed) > now_ms();
        let bases = if alt_first { [alt.as_str(), s.base.as_str()] } else { [s.base.as_str(), alt.as_str()] };
        for (k, base) in bases.into_iter().enumerate() {
            match send(format!("{}{path}", base.trim_end_matches('/'))) {
                // A gateway that answers for a server it can't reach is no answer either.
                Ok(r) if k == 0 && gateway(r.status().as_u16()) => continue,
                Ok(r) => {
                    let on_alt = base == alt.as_str();
                    PREFER_ALT.store(if on_alt { now_ms() + ALT_STICK_MS } else { 0 }, Ordering::Relaxed);
                    return Ok(r);
                }
                Err(_) => continue,
            }
        }
        Err(Fail::Unreachable)
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
        let r = self.go(s, path, |url| {
            if post {
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
            }
        })?;
        let header = |name: &str| r.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let name = header("content-disposition").and_then(|h| crate::logic::disposition_name(&h)).unwrap_or_else(|| "leaf-backup.json".into());
        let mime = header("content-type").map(|m| m.split(';').next().unwrap_or("").trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "application/json".into());
        let bytes = Client::read(Ok(r))?;
        Ok(BackupFile { name, mime, bytes })
    }

    pub fn get(&self, s: &Server, path: &str) -> Result<Vec<u8>, Fail> {
        Client::read(Ok(self.go(s, path, |url| {
            let mut req = self.agent.get(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        })?))
    }

    /// A page's picture, or None when the server skips it (a scanlator's promo page: `204` with
    /// `x-leaf-skip: 1`).
    pub fn get_page(&self, s: &Server, path: &str) -> Result<Option<Vec<u8>>, Fail> {
        let r = self.go(s, path, |url| {
            let mut req = self.agent.get(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        })?;
        if r.headers().get("x-leaf-skip").is_some_and(|v| v.as_bytes() == b"1") {
            return Ok(None);
        }
        Client::read(Ok(r)).map(Some)
    }

    pub fn post(&self, s: &Server, path: &str, body: String) -> Result<Vec<u8>, Fail> {
        Client::read(Ok(self.go(s, path, |url| {
            let mut req = self.agent.post(url).header("content-type", "application/json");
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.send(body.as_str())
        })?))
    }

    pub fn delete(&self, s: &Server, path: &str) -> Result<Vec<u8>, Fail> {
        Client::read(Ok(self.go(s, path, |url| {
            let mut req = self.agent.delete(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        })?))
    }

    /// DELETE with a JSON body (`/v1/pages/hide`).
    pub fn delete_json(&self, s: &Server, path: &str, body: String) -> Result<Vec<u8>, Fail> {
        Client::read(Ok(self.go(s, path, |url| {
            let mut req = self.agent.delete(url).force_send_body().header("content-type", "application/json");
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.send(body.as_str())
        })?))
    }

    /// A large file streamed to `to` (written as `to.part`, then renamed), `got` counting the
    /// bytes as they come; its SHA-256, in lower-case hex. Slow networks get the time they need.
    pub fn download(&self, s: &Server, path: &str, to: &Path, got: &AtomicU64) -> Result<String, Fail> {
        use std::io::{Read, Write};
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(10))).timeout_global(Some(Duration::from_secs(20 * 60))).http_status_as_error(false).build().into();
        let mut r = self.go(s, path, |url| {
            let mut req = agent.get(url);
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        })?;
        match r.status().as_u16() {
            200..=299 => {}
            401 | 403 => return Err(Fail::Token),
            code => return Err(Fail::Answer(code, String::new())),
        }
        let part = to.with_extension("part");
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|_| Fail::Garbled)?;
        }
        let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|_| Fail::Garbled)?);
        let mut body = r.body_mut().with_config().limit(400 << 20).reader();
        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = body.read(&mut buf).map_err(|_| Fail::Unreachable)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|_| Fail::Garbled)?;
            got.fetch_add(n as u64, Ordering::Relaxed);
        }
        out.flush().map_err(|_| Fail::Garbled)?;
        drop(out);
        std::fs::rename(&part, to).map_err(|_| Fail::Garbled)?;
        Ok(hash.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect())
    }

    /// A file on disk sent as the body of a `PUT`, streamed; slow networks get the time they need.
    pub fn put_file(&self, s: &Server, path: &str, file: &Path) -> Result<(), Fail> {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(10))).timeout_global(Some(Duration::from_secs(20 * 60))).http_status_as_error(false).build().into();
        let r = self.go(s, path, |url| {
            let f = std::fs::File::open(file).map_err(ureq::Error::Io)?;
            let mut req = agent.put(url).header("content-type", "application/octet-stream");
            if let Some(t) = &s.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.send(&f)
        })?;
        Client::read(Ok(r)).map(|_| ())
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
    /// A title not added yet (`/v1/preview?manga_id=`), shown under its preview id
    /// (`-manga_id` of the first entry) with the entries to try, in order.
    Preview(i64, Vec<i64>),
    /// "Did you mean": AniList's title for words that found nothing (`/v1/search/suggest`).
    Suggest(String),
    /// A search: the broad sources one by one (each answer as it comes), or every source (`true`,
    /// "Search all sources") at once.
    Search(String, bool),
    Follow(Follow),
    Unfollow(i64),
    Progress(i64, Progress),
    PageCount(i64),
    /// Turn a series' bell on or off (`POST /v1/series/{id}/bell`).
    Bell(i64, bool),
    /// Hear once when a source other than Webtoon has it (`POST /v1/series/{id}/watch`).
    Watch(i64, bool),
    /// Followed series that look like one work (`GET /v1/duplicates`).
    Duplicates,
    /// The second series made part of the first (`POST /v1/series/{id}/merge`).
    Merge(i64, i64),
    /// "Not the same" (`POST /v1/duplicates/dismiss`).
    NotSame(i64, i64),
    /// Chapters that came lately for series in My space, over this many days (`/v1/updates`).
    Updates(u32),
    /// A chapter read or unread (`POST /v1/chapters/{id}/read`).
    ChapterRead(i64, bool),
    /// Every chapter of a series numbered below this, read (`POST /v1/series/{id}/read`).
    ReadBefore(i64, f64, bool),
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
    /// The recommendations (`/v1/recs`), from the copy on disk when it is younger than
    /// [`RECS_FRESH_MS`] unless `true` (asked again by hand).
    Recs(bool),
    /// "Not interested" (`POST /v1/recs/{anilist_id}/hide`).
    HideRec(i64),
    /// Fewer recommendations with a tag, or as before (`POST /v1/recs/tags`).
    RecTag(String, bool),
    /// "Readers also like" for a series (`/v1/series/{id}/also`).
    Also(i64),
    /// The newest app the server holds (`/v1/app/latest`).
    AppLatest,
    /// A page hidden by hand with the ones like it, or that undone (`POST` / `DELETE
    /// /v1/pages/hide`).
    HidePage(i64, u32, bool),
    /// How many pages were hidden by hand (`/v1/pages/hidden`).
    HiddenPages,
    /// Every page hidden by hand shown again (`DELETE /v1/pages/hide`, `all`).
    ClearHidden,
    /// A page was broken: the chapter read from another source (`POST /v1/chapters/{id}/elsewhere`).
    Elsewhere(i64),
    /// Your stars for a series, or none (`POST /v1/series/{id}/rating`).
    Stars(i64, Option<u8>),
    /// What the server says of itself for Settings (`GET /v1/server`).
    ServerInfo,
    /// Every chapter read and when, for Wrapped (`GET /v1/history`).
    History,
    /// The room its chapters may take, in GB, or no limit (`POST /v1/settings/disk`).
    DiskCap(Option<u32>),
    /// Your lists (`GET /v1/lists`), the copy on disk first.
    Lists,
    /// Every list kept as it is now (`POST /v1/lists`), with the edit's number.
    SaveLists(u64, Vec<leaf_proto::List>),
}

/// `GET /v1/server`: the last pass, the download line, the room chapters take and the backups.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct ServerInfo {
    pub pass_at: i64,
    pub queued: u32,
    pub series: u32,
    pub room: Room,
    pub backup_at: i64,
    /// When this PC last took a copy of the server's data (0: never).
    pub offsite_at: i64,
}

/// The room the server's chapters take (`disk.rs` there).
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Room {
    pub used: Option<u64>,
    pub cap_gb: Option<u32>,
    pub cleaned_at: i64,
    pub cleaned: u32,
    pub stuck: bool,
}

/// The recommendations on disk are asked for again after this long (they are made once a day).
pub const RECS_FRESH_MS: i64 = 30 * 60_000;

/// What `/v1/library` and `/v1/series/{id}` send beyond leaf-proto's [`Series`] and
/// [`leaf_proto::Chapter`]: `paid` on each series and `thumb` on each chapter. leaf-server builds
/// those structs field by field, so they gain these only once it does; until then the phone reads
/// them from the same JSON.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct SeriesExtra {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub paid: Option<PaidAhead>,
    /// Only Webtoon has it: whether you asked to hear when another source does (None: one does).
    #[serde(default)]
    pub watch: Option<bool>,
    #[serde(flatten)]
    pub more: More,
}

/// What a series carries beside its struct for the page and My space: your stars, when its newest
/// chapters came out (newest first, ms), and its genres.
#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
pub struct More {
    #[serde(default)]
    pub stars: Option<u8>,
    #[serde(default)]
    pub uploads: Vec<i64>,
    #[serde(default)]
    pub genres: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
struct ChapterExtra {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    thumb: bool,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
struct DetailExtra {
    #[serde(default)]
    series: SeriesExtra,
    #[serde(default)]
    chapters: Vec<ChapterExtra>,
}

/// A series page's extras: its paid episodes ahead, and which chapters have a picture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extras {
    pub paid: Option<PaidAhead>,
    pub watch: Option<bool>,
    pub thumbs: Vec<(i64, bool)>,
    pub more: More,
}

/// The extras of a `/v1/series/{id}` answer.
pub fn detail_extras(bytes: &[u8]) -> Extras {
    let d: DetailExtra = serde_json::from_slice(bytes).unwrap_or_default();
    Extras { paid: d.series.paid, watch: d.series.watch, thumbs: d.chapters.into_iter().map(|c| (c.id, c.thumb)).collect(), more: d.series.more }
}

/// Each series' paid episodes ahead, from a `/v1/library` answer.
pub fn library_extras(bytes: &[u8]) -> Vec<SeriesExtra> {
    serde_json::from_slice(bytes).unwrap_or_default()
}

impl Job {
    /// It changes something on the server (a screenshot never sends these).
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Job::Follow(_)
                | Job::Unfollow(_)
                | Job::Progress(..)
                | Job::Bell(..)
                | Job::Watch(..)
                | Job::Merge(..)
                | Job::NotSame(..)
                | Job::ChapterRead(..)
                | Job::ReadBefore(..)
                | Job::Shelf(..)
                | Job::Backup(true)
                | Job::Anilist(_)
                | Job::Repin(..)
                | Job::HideRec(_)
                | Job::RecTag(..)
                | Job::HidePage(..)
                | Job::ClearHidden
                | Job::Elsewhere(_)
                | Job::Stars(..)
                | Job::DiskCap(_)
                | Job::SaveLists(..)
        )
    }
}

/// An answer. `cached` ones come from the copy on disk, before (or instead of) the server's.
#[derive(Debug)]
pub enum Done {
    Status(Server, Result<Status, Fail>),
    Library { cached: bool, result: Result<(Vec<Series>, Vec<SeriesExtra>), Fail> },
    Series { id: i64, cached: bool, result: Result<(SeriesDetail, Extras), Fail> },
    /// A search's answer, or one source's part of it: (query, every source, result, the last part).
    Search { q: String, all: bool, result: Result<Vec<leaf_proto::Candidate>, Fail>, last: bool },
    Followed(Result<Series, Fail>),
    /// (the words, the title they probably meant).
    Suggested(String, Result<Option<String>, Fail>),
    Unfollowed(i64, Result<(), Fail>),
    Progress(i64, Result<(), Fail>),
    PageCount(i64, Result<u32, Fail>),
    Bell(i64, bool, Result<(), Fail>),
    Watch(i64, bool, Result<(), Fail>),
    Duplicates(Result<Vec<[i64; 2]>, Fail>),
    Merged(i64, i64, Result<(), Fail>),
    NotSame(Result<(), Fail>),
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
    Recs { cached: bool, result: Result<Recs, Fail> },
    RecHidden(i64, Result<(), Fail>),
    RecTag(String, bool, Result<(), Fail>),
    Also(i64, Result<Vec<Rec>, Fail>),
    /// None: the server holds no app.
    AppLatest(Result<Option<AppLatest>, Fail>),
    /// (chapter, page, hidden or shown again, the count of pages hidden by hand).
    HidePage(i64, u32, bool, Result<u32, Fail>),
    /// The count of pages hidden by hand (asked, or after Clear).
    HiddenPages(Result<u32, Fail>),
    /// The chapter's new page count, read from another source.
    Elsewhere(i64, Result<u32, Fail>),
    Stars(i64, Option<u8>, Result<(), Fail>),
    ServerInfo(Result<ServerInfo, Fail>),
    History(Result<Vec<leaf_proto::HistoryRead>, Fail>),
    DiskCap(Result<Room, Fail>),
    Lists { cached: bool, result: Result<Vec<leaf_proto::List>, Fail> },
    /// The edit's number, and the lists as the server kept them.
    SavedLists(u64, Result<Vec<leaf_proto::List>, Fail>),
}

impl Done {
    /// What this answer says of the server's reach: Some(true) it answered (even with an error of
    /// its own), Some(false) it could not be reached, None for a copy from disk or the check of
    /// another server (Connect's).
    pub fn reach(&self, server: Option<&Server>) -> Option<bool> {
        fn of<T>(r: &Result<T, Fail>) -> Option<bool> {
            Some(match r {
                Ok(_) => true,
                Err(f) => !f.out_of_reach(),
            })
        }
        match self {
            Done::Status(s, r) => server.filter(|mine| mine.base == s.base).and_then(|_| of(r)),
            Done::Library { cached: true, .. } | Done::Series { cached: true, .. } | Done::Updates { cached: true, .. } | Done::Recs { cached: true, .. } | Done::Lists { cached: true, .. } => None,
            Done::Library { result, .. } => of(result),
            Done::Series { result, .. } => of(result),
            Done::Search { result, .. } => of(result),
            Done::Followed(r) => of(r),
            Done::Suggested(_, r) => of(r),
            Done::Unfollowed(_, r) => of(r),
            Done::Progress(_, r) => of(r),
            Done::PageCount(_, r) => of(r),
            Done::Bell(_, _, r) => of(r),
            Done::Watch(_, _, r) => of(r),
            Done::Duplicates(r) => of(r),
            Done::Merged(_, _, r) => of(r),
            Done::NotSame(r) => of(r),
            Done::Updates { result, .. } => of(result),
            Done::ChapterRead(_, _, r) => of(r),
            Done::ReadBefore(_, r) => of(r),
            Done::Shelf(_, r) => of(r),
            Done::Health(r) => of(r),
            Done::Backup(r) => of(r),
            Done::Anilist(_, r) => of(r),
            Done::Alternatives(_, r) => of(r),
            Done::Repinned(_, r) => of(r),
            Done::Recs { result, .. } => of(result),
            Done::RecHidden(_, r) => of(r),
            Done::RecTag(_, _, r) => of(r),
            Done::Also(_, r) => of(r),
            Done::AppLatest(r) => of(r),
            Done::HidePage(_, _, _, r) => of(r),
            Done::HiddenPages(r) => of(r),
            Done::Elsewhere(_, r) => of(r),
            Done::Stars(_, _, r) => of(r),
            Done::ServerInfo(r) => of(r),
            Done::History(r) => of(r),
            Done::DiskCap(r) => of(r),
            Done::Lists { result, .. } => of(result),
            Done::SavedLists(_, r) => of(r),
        }
    }
}

type Task = (Option<Server>, Job);

/// The workers and their answers.
pub struct Jobs {
    tx: Sender<Task>,
    rx: Receiver<Done>,
    /// For what is answered from this phone at once, never queued behind the network.
    done_tx: Sender<Done>,
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
        Jobs { tx, rx, done_tx, running }
    }

    /// Ask, of `server` (none: the app has no server yet, and the job answers from disk alone).
    pub fn send(&self, server: Option<&Server>, job: Job) {
        self.running.fetch_add(1, Ordering::SeqCst);
        if self.tx.send((server.cloned(), job)).is_err() {
            self.running.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// An answer known already (from what this phone holds): taken with the workers' next.
    pub fn answer(&self, done: Done) {
        let _ = self.done_tx.send(done);
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

/// The lists as last known, for opening with no server.
pub fn lists_file(data: &Path) -> PathBuf {
    data.join("lists.json")
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

/// Forget the copy of a series kept on disk (its id now names another series).
pub fn forget_series_copy(data: &Path, id: i64) {
    let _ = std::fs::remove_file(series_file(data, id));
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
            if let Some(b) = std::fs::read(&file).ok()
                && let Ok(list) = serde_json::from_slice::<Vec<Series>>(&b)
            {
                send(Done::Library { cached: true, result: Ok((list, library_extras(&b))) });
            }
            let Some(s) = server else { return send(Done::Library { cached: false, result: no_server() }) };
            wait_for_progress();
            flush_outbox(client, data, &s);
            let r = client.get(&s, "/v1/library").and_then(|b| {
                let list = serde_json::from_slice::<Vec<Series>>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok((list, library_extras(&b)))
            });
            send(Done::Library { cached: false, result: r });
        }
        Job::Series(id) => {
            let file = series_file(data, id);
            if let Some(b) = std::fs::read(&file).ok()
                && let Ok(d) = serde_json::from_slice::<SeriesDetail>(&b)
            {
                send(Done::Series { id, cached: true, result: Ok((d, detail_extras(&b))) });
            }
            let Some(s) = server else { return send(Done::Series { id, cached: false, result: no_server() }) };
            wait_for_progress();
            flush_outbox(client, data, &s);
            let r = client.get(&s, &format!("/v1/series/{id}")).and_then(|b| {
                let d = serde_json::from_slice::<SeriesDetail>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok((d, detail_extras(&b)))
            });
            send(Done::Series { id, cached: false, result: r });
        }
        Job::Preview(id, ids) => {
            // Never kept on disk: a preview is only for now.
            let Some(s) = server else { return send(Done::Series { id, cached: false, result: no_server() }) };
            let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            let r = client.get(&s, &format!("/v1/preview?manga_id={list}")).and_then(|b| {
                let d = serde_json::from_slice::<SeriesDetail>(&b).map_err(|_| Fail::Garbled)?;
                Ok((d, detail_extras(&b)))
            });
            send(Done::Series { id, cached: false, result: r });
        }
        Job::Suggest(q) => {
            #[derive(Deserialize)]
            struct Said {
                title: Option<String>,
            }
            let r = match &server {
                Some(s) => client.json::<Said>(s, &format!("/v1/search/suggest?q={}", query_escape(&q))).map(|v| v.title),
                None => no_server(),
            };
            send(Done::Suggested(q, r));
        }
        Job::Search(q, all) => {
            let Some(s) = &server else { return send(Done::Search { q, all, result: no_server(), last: true }) };
            let path = format!("/v1/search?q={}", query_escape(&q));
            // The broad sources, each asked apart so its results show as they come; a server
            // without the list (older) answers the whole search at once.
            let broad = if all { None } else { client.json::<leaf_proto::SearchSources>(s, "/v1/search/sources").ok().filter(|l| !l.broad.is_empty()) };
            match broad {
                Some(list) => {
                    let (tx, rx) = channel();
                    std::thread::scope(|sc| {
                        for name in &list.broad {
                            let (tx, path) = (tx.clone(), format!("{path}&source={}", query_escape(name)));
                            sc.spawn(move || {
                                let _ = tx.send(client.json::<Vec<leaf_proto::Candidate>>(s, &path));
                            });
                        }
                        drop(tx);
                        let n = list.broad.len();
                        let mut got = 0;
                        for result in rx.iter() {
                            got += 1;
                            send(Done::Search { q: q.clone(), all, result, last: got == n });
                        }
                        // A part that never came (its thread ended early) still ends the search.
                        if got < n {
                            send(Done::Search { q: q.clone(), all, result: Ok(Vec::new()), last: true });
                        }
                    });
                }
                None => {
                    let result = client.json(s, &if all { format!("{path}&all=1") } else { path });
                    send(Done::Search { q, all, result, last: true });
                }
            }
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
            // Downloaded to this phone: its length is in its manifest.
            let saved = std::fs::read(crate::offline::chapter_dir(data, chapter).join("manifest.json")).ok().and_then(|b| serde_json::from_slice::<crate::offline::Manifest>(&b).ok());
            if let Some(m) = saved {
                return send(Done::PageCount(chapter, Ok(m.pages)));
            }
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
        Job::Duplicates => {
            let r = match &server {
                Some(s) => client.json(s, "/v1/duplicates"),
                None => no_server(),
            };
            send(Done::Duplicates(r));
        }
        Job::Merge(keep, other) => {
            let r = post_json(client, &server, &format!("/v1/series/{keep}/merge"), &leaf_proto::Merge { other }).map(|_| ());
            send(Done::Merged(keep, other, r));
        }
        Job::NotSame(a, b) => {
            let r = post_json(client, &server, "/v1/duplicates/dismiss", &[a, b]).map(|_| ());
            send(Done::NotSame(r));
        }
        Job::Watch(id, on) => {
            let r = match &server {
                Some(s) => serde_json::to_string(&Bell { on }).map_err(|_| Fail::Garbled).and_then(|body| client.post(s, &format!("/v1/series/{id}/watch"), body)).map(|_| ()),
                None => no_server(),
            };
            send(Done::Watch(id, on, r));
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
        Job::ReadBefore(id, before, unread_after) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/read"), &ReadBefore { before, unread_after }).map(|_| ());
            send(Done::ReadBefore(id, r));
        }
        Job::Shelf(id, shelf) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/shelf"), &ShelfSet { shelf }).map(|_| ());
            send(Done::Shelf(id, r));
        }
        Job::HidePage(chapter, page, on) => {
            let body = serde_json::to_string(&leaf_proto::PageHide { chapter, page, all: false }).unwrap_or_default();
            let r = match &server {
                Some(s) if on => client.post(s, "/v1/pages/hide", body),
                Some(s) => client.delete_json(s, "/v1/pages/hide", body),
                None => no_server(),
            };
            send(Done::HidePage(chapter, page, on, r.map(|b| count_of(&b))));
        }
        Job::HiddenPages => {
            let r = match &server {
                Some(s) => client.json::<leaf_proto::HiddenPages>(s, "/v1/pages/hidden").map(|h| h.count),
                None => no_server(),
            };
            send(Done::HiddenPages(r));
        }
        Job::ClearHidden => {
            let body = serde_json::to_string(&leaf_proto::PageHide { all: true, ..Default::default() }).unwrap_or_default();
            let r = match &server {
                Some(s) => client.delete_json(s, "/v1/pages/hide", body).map(|b| count_of(&b)),
                None => no_server(),
            };
            send(Done::HiddenPages(r));
        }
        Job::Elsewhere(chapter) => {
            let r = match &server {
                Some(s) => client.post(s, &format!("/v1/chapters/{chapter}/elsewhere"), "{}".into()).map(|b| count_of(&b)),
                None => no_server(),
            };
            send(Done::Elsewhere(chapter, r));
        }
        Job::Stars(id, stars) => {
            let r = post_json(client, &server, &format!("/v1/series/{id}/rating"), &leaf_proto::Stars { stars }).map(|_| ());
            send(Done::Stars(id, stars, r));
        }
        Job::ServerInfo => {
            let r = match &server {
                Some(s) => client.json::<ServerInfo>(s, "/v1/server"),
                None => no_server(),
            };
            send(Done::ServerInfo(r));
        }
        Job::History => {
            let r = match &server {
                Some(s) => client.json::<Vec<leaf_proto::HistoryRead>>(s, "/v1/history?since=0"),
                None => no_server(),
            };
            send(Done::History(r));
        }
        Job::Lists => {
            let file = lists_file(data);
            if let Some(v) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Vec<leaf_proto::List>>(&b).ok()) {
                send(Done::Lists { cached: true, result: Ok(v) });
            }
            let r = match &server {
                Some(s) => client.json::<Vec<leaf_proto::List>>(s, "/v1/lists"),
                None => no_server(),
            };
            if let Ok(v) = &r {
                let _ = write_atomic(&file, &serde_json::to_vec(v).unwrap_or_default());
            }
            send(Done::Lists { cached: false, result: r });
        }
        Job::SaveLists(rev, lists) => {
            let r = post_json(client, &server, "/v1/lists", &lists).and_then(|b| serde_json::from_slice::<Vec<leaf_proto::List>>(&b).map_err(|_| Fail::Garbled));
            send(Done::SavedLists(rev, r));
        }
        Job::DiskCap(cap_gb) => {
            let r = post_json(client, &server, "/v1/settings/disk", &leaf_proto::DiskCap { cap_gb }).and_then(|b| serde_json::from_slice::<Room>(&b).map_err(|_| Fail::Garbled));
            send(Done::DiskCap(r));
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
        Job::Recs(force) => {
            let file = data.join("recs.json");
            let disk = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Recs>(&b).ok());
            let age = std::fs::metadata(&file).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).map_or(i64::MAX, |d| d.as_millis() as i64);
            if let Some(r) = &disk {
                // Young enough: the copy on disk is the answer, and the server is not asked.
                let fresh = !force && age < RECS_FRESH_MS;
                send(Done::Recs { cached: !fresh, result: Ok(r.clone()) });
                if fresh {
                    return;
                }
            }
            let Some(s) = server else { return send(Done::Recs { cached: false, result: no_server() }) };
            let r = client.get(&s, "/v1/recs").and_then(|b| {
                let recs = serde_json::from_slice::<Recs>(&b).map_err(|_| Fail::Garbled)?;
                let _ = write_atomic(&file, &b);
                Ok(recs)
            });
            send(Done::Recs { cached: false, result: r });
        }
        Job::HideRec(id) => {
            let r = post_json(client, &server, &format!("/v1/recs/{id}/hide"), &serde_json::json!({})).map(|_| ());
            send(Done::RecHidden(id, r));
        }
        Job::RecTag(tag, less) => {
            let r = post_json(client, &server, "/v1/recs/tags", &RecTag { tag: tag.clone(), less }).map(|_| ());
            send(Done::RecTag(tag, less, r));
        }
        Job::Also(id) => {
            let r = match &server {
                Some(s) => client.json(s, &format!("/v1/series/{id}/also")),
                None => no_server(),
            };
            send(Done::Also(id, r));
        }
        Job::AppLatest => {
            let r = match &server {
                Some(s) => match client.json::<AppLatest>(s, "/v1/app/latest") {
                    Ok(l) => Ok(Some(l)),
                    Err(Fail::Answer(404, _)) => Ok(None),
                    Err(f) => Err(f),
                },
                None => no_server(),
            };
            send(Done::AppLatest(r));
        }
    }
}

/// `{"ok":true,"count":n}`: n.
fn count_of(body: &[u8]) -> u32 {
    serde_json::from_slice::<serde_json::Value>(body).ok().and_then(|v| v.get("count").and_then(|c| c.as_u64())).unwrap_or(0) as u32
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
    fn the_extras_are_read_beside_the_structs() {
        let detail = br#"{"series":{"id":7,"title":"T","author":null,"source":"S","status":"ongoing","chapters":2,"read_upto":null,"unread":2,"latest":2,"gaps":[],"updated_at":0,"note":null,"paid":{"from":9,"to":23,"url":"https://w"}},"description":null,"chapters":[{"id":70,"number":1,"name":"","group":null,"pages":3,"read":false,"last_page":0,"downloaded":true,"uploaded_at":0,"thumb":true},{"id":71,"number":2,"name":"","group":null,"pages":3,"read":false,"last_page":0,"downloaded":false,"uploaded_at":0}]}"#;
        assert!(serde_json::from_slice::<SeriesDetail>(detail).is_ok(), "the struct ignores what it lacks");
        let e = detail_extras(detail);
        assert_eq!(e.paid, Some(PaidAhead { from: 9.0, to: 23.0, url: "https://w".into() }));
        assert_eq!(e.thumbs, vec![(70, true), (71, false)], "no thumb field: none");
        let lib = br#"[{"id":7,"paid":null},{"id":8,"paid":{"from":1,"to":2,"url":"u"}}]"#;
        let l = library_extras(lib);
        assert_eq!((l[0].paid.is_none(), l[1].paid.as_ref().map(|p| p.to)), (true, Some(2.0)));
        assert_eq!(detail_extras(b"not json"), Extras::default());
        assert_eq!(count_of(br#"{"ok":true,"count":12}"#), 12);
    }

    #[test]
    fn failures_read_as_words() {
        assert_eq!(Fail::Unreachable.words("192.168.1.5:4580"), "Can't reach your server at 192.168.1.5:4580");
        // A proxy saying the server is down is out of reach; the server's own 502 is an answer.
        assert!(Fail::Unreachable.out_of_reach() && Fail::Answer(502, String::new()).out_of_reach() && Fail::Answer(530, String::new()).out_of_reach());
        assert!(!Fail::Answer(502, "source failed".into()).out_of_reach() && !Fail::Token.out_of_reach() && !Fail::Answer(404, String::new()).out_of_reach());
        assert_eq!(Fail::Answer(502, "suwayomi: timed out".into()).words("x"), "Your server says: suwayomi: timed out");
    }
}
