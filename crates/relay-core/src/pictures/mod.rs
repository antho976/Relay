//! The Pictures plugin's engine side (pictures-SPEC-v2 §7): picture ids and the words a search
//! sends, the providers' answers read with their licences ([`parse`]), a client for their three
//! fixed hosts ([`net`]), the thumbnail cache, and the [`Desk`]: what the engine remembers between
//! requests (hits, searches, cooldowns, the agents' allowance, the Pexels key's state). The ops
//! come with pictures-key, pictures-search and pictures-add (`handlers/picture.rs`).
//!
//! Lock rules (§7.1): no `Desk` mutex is held across I/O, or while the store is taken
//! (`ctx.read`): a caller copies out what it needs and lets go, so the store, when held, is always
//! taken first. Every `Desk` mutex is taken poison-tolerantly, so one panic breaks no later call.
//! Nothing here sleeps: a provider that asks to wait is refused locally until then.

pub mod net;
pub mod parse;

use crate::picture::{self, Picture};
use crate::Instance;
use net::{Fetch, Req, Ureq, Want};
use parse::Item;
use relay_bus::ops::picture::{Orientation, SearchOut};
pub use relay_bus::ops::picture::{PictureCredit, PictureHit, PictureSource, PLUGIN};
use relay_bus::BusError;
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::hash::Hash;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

/// Openverse searches agents together may send in any 24 hours: half its anonymous day, so they
/// can never spend the person's share (§1.8).
pub const AGENT_SEARCHES_PER_DAY: usize = 100;
/// Uncached Openverse thumbnails agents together may fetch in any 24 hours.
pub const AGENT_THUMBS_PER_DAY: usize = 300;
/// How long a search is answered again from memory; nothing older is ever served.
pub const FRESH: Duration = Duration::from_secs(3600);
/// A cached thumbnail nobody asked for in this long is swept.
pub const THUMB_TTL: Duration = Duration::from_secs(7 * DAY_S);
/// The thumbnail sweep runs at most this often.
pub const SWEEP_EVERY: Duration = Duration::from_secs(DAY_S);
/// The most pixels a stored photo may have: past what either source serves (Pexels `large2x` is
/// 1880 × 1300, Flickr through Openverse 1024 px) and a decoded texture near 100 MB.
pub const MAX_PIXELS: u64 = 25_000_000;
/// The latest hits remembered by id, so `thumb` and `add` need no detail read for them.
pub const HITS: usize = 2000;
/// Searches answered again from memory.
pub const SEARCHES: usize = 64;
/// How long a 429 without `Retry-After` cools its bucket, in seconds.
pub const COOL_DEFAULT_S: u64 = 60;
/// The longest a provider can make a bucket cool, in seconds; a longer `Retry-After` is cut.
pub const COOL_MAX_S: u64 = DAY_S;
/// The most characters a search sends (Openverse's own limit on `q`).
pub const MAX_QUERY: usize = 200;
/// The last page a search reaches.
pub const MAX_PAGE: u32 = 10;
/// The most results one search answers: Openverse's anonymous `page_size` limit.
pub const MAX_COUNT: u32 = 20;
pub const DEFAULT_COUNT: u32 = 12;

const DAY_S: u64 = 24 * 3600;
const DAY: Duration = Duration::from_secs(DAY_S);

// ------------------------------------------------------------------ ids and searches

/// A picture id as search answers it: `openverse:<uuid>` (lowercase 8-4-4-4-12 hex) or
/// `pexels:<1 to 12 digits>`. It is checked before any URL or file name is built from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id {
    pub source: PictureSource,
    /// The provider's own id: the uuid, or the digits.
    pub key: String,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", slug(self.source), self.key)
    }
}

/// `id` read as an [`Id`], or `design.picture.id`.
pub fn parse_id(id: &str) -> Result<Id, BusError> {
    match id.split_once(':') {
        Some(("openverse", key)) if is_uuid(key) => Ok(Id { source: PictureSource::Openverse, key: key.to_string() }),
        Some(("pexels", key)) if is_pexels_key(key) => Ok(Id { source: PictureSource::Pexels, key: key.to_string() }),
        _ => Err(bad_id(id)),
    }
}

/// Lowercase 8-4-4-4-12 hex, as Openverse writes its ids.
pub fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// 1 to 12 ASCII digits with no leading zero, as Pexels numbers its photos: one id per photo.
pub fn is_pexels_key(s: &str) -> bool {
    (1..=12).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()) && !s.starts_with('0')
}

/// The source as ids and cache file names write it.
pub fn slug(source: PictureSource) -> &'static str {
    match source {
        PictureSource::Openverse => "openverse",
        PictureSource::Pexels => "pexels",
    }
}

/// The source as a sentence names it.
pub fn name(source: PictureSource) -> &'static str {
    match source {
        PictureSource::Openverse => "Openverse",
        PictureSource::Pexels => "Pexels",
    }
}

/// A search's words as they are sent: trimmed, each run of whitespace one space, 1 to
/// [`MAX_QUERY`] characters, no control characters; else `design.picture.query`.
pub fn normalise_query(query: &str) -> Result<String, BusError> {
    if query.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(bad_query("it holds a control character".into()));
    }
    let words = query.split_whitespace().collect::<Vec<_>>().join(" ");
    match words.chars().count() {
        0 => Err(bad_query("it is empty".into())),
        n if n > MAX_QUERY => Err(bad_query(format!("it is {n} characters; at most {MAX_QUERY}"))),
        _ => Ok(words),
    }
}

/// The page asked for: 1 by default, 1 to [`MAX_PAGE`]; else `design.picture.page`.
pub fn page(page: Option<u32>) -> Result<u32, BusError> {
    match page.unwrap_or(1) {
        p @ 1..=MAX_PAGE => Ok(p),
        p => Err(BusError::invalid("design.picture.page", format!("page {p} is outside 1 to {MAX_PAGE}"))
            .with_hint("search with other words instead")),
    }
}

/// How many results to ask for: [`DEFAULT_COUNT`] by default, cut to 1 to [`MAX_COUNT`].
pub fn count(count: Option<u32>) -> u32 {
    count.unwrap_or(DEFAULT_COUNT).clamp(1, MAX_COUNT)
}

/// The provider's word for a shape: Openverse's `aspect_ratio` (`wide`, `tall`, `square`), Pexels'
/// `orientation` (`landscape`, `portrait`, `square`).
pub fn orientation(source: PictureSource, shape: Orientation) -> &'static str {
    match (source, shape) {
        (PictureSource::Openverse, Orientation::Landscape) => "wide",
        (PictureSource::Openverse, Orientation::Portrait) => "tall",
        (PictureSource::Pexels, Orientation::Landscape) => "landscape",
        (PictureSource::Pexels, Orientation::Portrait) => "portrait",
        (_, Orientation::Square) => "square",
    }
}

/// What a search is remembered by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchKey {
    pub source: PictureSource,
    /// As [`normalise_query`] answers it.
    pub query: String,
    pub orientation: Option<Orientation>,
    pub page: u32,
    pub count: u32,
}

/// The picture in fetched bytes: a PNG, JPEG, GIF or WebP (never SVG) of at most [`MAX_PIXELS`];
/// else `design.picture.not_raster` or `design.picture.too_big`.
pub fn photo(bytes: &[u8]) -> Result<Picture, BusError> {
    let picture = picture::raster(bytes).ok_or_else(not_raster)?;
    if u64::from(picture.w) * u64::from(picture.h) > MAX_PIXELS {
        return Err(too_big(format!("{} × {} px, over {MAX_PIXELS} pixels", picture.w, picture.h)));
    }
    Ok(picture)
}

// ------------------------------------------------------------------ refusals (§6)

fn bad_id(id: &str) -> BusError {
    let shown: String = id.chars().take(80).collect();
    BusError::invalid("design.picture.id", format!("{shown:?} is not a picture id (openverse:<uuid> or pexels:<digits>)"))
        .with_hint("use an id from design.picture.search")
}

fn bad_query(why: String) -> BusError {
    BusError::invalid("design.picture.query", format!("the query cannot be sent: {why}"))
        .with_hint("give two to four concrete nouns")
}

/// `design.picture.no_key`: Pexels with no saved key.
pub fn no_key() -> BusError {
    BusError::refused("design.picture.no_key", "no Pexels key is saved")
        .with_hint("use source openverse; the person can save a Pexels key in the Design view")
}

/// `design.picture.key_rejected`: Pexels answered 401 or 403 to the key.
pub fn key_rejected() -> BusError {
    BusError::refused("design.picture.key_rejected", "Pexels did not accept the key")
        .with_hint("use openverse; the person should save a new Pexels key")
}

/// `design.picture.key_format`: `why` must never quote the key.
pub fn key_format(why: &str) -> BusError {
    BusError::invalid("design.picture.key_format", format!("the key {why}")).with_hint("paste the whole key")
}

/// `design.picture.keyring`: the desktop keyring could not be read or written.
pub fn keyring(why: impl Into<String>) -> BusError {
    BusError::unavailable("design.picture.keyring", why)
        .with_hint("use openverse; the person can unlock the desktop keyring (a prompt may be waiting) and try again")
}

/// `design.picture.rate_limited`: a provider asked to wait, a bucket still cools, or agents spent
/// their day's allowance. `retry_after_s` is when asking again can answer.
pub fn rate_limited(source: PictureSource, retry_after_s: u64) -> BusError {
    BusError::unavailable("design.picture.rate_limited", format!("{} asks Relay to wait {retry_after_s} s", name(source)))
        .with_details(json!({"source": source, "retry_after_s": retry_after_s}))
        .with_hint("use the other source, or draw it; never retry in a loop")
}

fn allowance_spent(source: PictureSource, limit: usize, retry_after_s: u64) -> BusError {
    BusError::unavailable(
        "design.picture.rate_limited",
        format!("agents together have made {limit} such {} requests in 24 hours; the rest of the day is the person's", name(source)),
    )
    .with_details(json!({"source": source, "retry_after_s": retry_after_s}))
    .with_hint("use the other source, or draw it; never retry in a loop")
}

/// `design.picture.offline`: the provider could not be reached.
pub fn offline(source: PictureSource, why: &str) -> BusError {
    BusError::unavailable("design.picture.offline", format!("{} could not be reached: {why}", name(source)))
        .with_details(json!({"source": source}))
        .with_hint("draw it; photos already placed still work")
}

/// `design.picture.provider`: the provider answered, but not with what was asked. `why` names
/// what went wrong; a provider's own words are never echoed.
pub fn provider(source: PictureSource, status: Option<u16>, why: impl fmt::Display) -> BusError {
    let details = match status {
        Some(status) => json!({"source": source, "status": status}),
        None => json!({"source": source}),
    };
    BusError::unavailable("design.picture.provider", format!("{} {why}", name(source)))
        .with_details(details)
        .with_hint("try once more later")
}

/// `design.picture.gone`: the provider no longer has the photo, or its own host is gone.
pub fn gone(source: PictureSource) -> BusError {
    BusError::not_found("design.picture.gone", format!("{} no longer has this photo", name(source)))
        .with_details(json!({"source": source}))
        .with_hint("pick another result")
}

/// `design.picture.licence`: a licence outside CC BY, CC BY-SA, CC0, public domain and the Pexels
/// License (`licence` is the provider's code), or a photo marked mature (`licence` is `mature`).
pub fn licence(licence: &str) -> BusError {
    let message = if licence == "mature" {
        "this photo is marked mature".to_string()
    } else {
        format!("this photo's licence ({licence}) is not one Relay keeps: CC BY, CC BY-SA, CC0, public domain or the Pexels License")
    };
    BusError::refused("design.picture.licence", message)
        .with_details(json!({"licence": licence}))
        .with_hint("use a result from design.picture.search")
}

/// `design.picture.not_raster`: the bytes are not a PNG, JPEG, GIF or WebP.
pub fn not_raster() -> BusError {
    BusError::invalid("design.picture.not_raster", "the download is not a PNG, JPEG, GIF or WebP picture")
        .with_hint("pick another result")
}

/// `design.picture.too_big`: over 20 MiB, or over [`MAX_PIXELS`].
pub fn too_big(why: String) -> BusError {
    BusError::invalid("design.picture.too_big", format!("the photo is {why}")).with_hint("pick another result")
}

// ------------------------------------------------------------------ the key's state

/// The Pexels key in memory. Its `Debug` hides it; it has no `Display` or `Serialize`, so it can
/// reach no answer, log or audit row by accident. It is sent only as `Authorization` to
/// api.pexels.com ([`net`]).
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(key: impl Into<String>) -> SecretKey {
        SecretKey(key.into())
    }

    /// The key itself: for the `Authorization` header and the keyring, nothing else.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(…)")
    }
}

/// What the engine knows of the saved Pexels key.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Key {
    /// Not read since the engine started.
    #[default]
    Unknown,
    /// None is saved.
    Absent,
    Saved(SecretKey),
    /// The keyring could not be read; agents are refused at once until then.
    Unavailable(Instant),
}

// ------------------------------------------------------------------ the desk

/// Where a request spends: each provider limit is counted, and cooled after a 429, on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Openverse searches and detail reads: 20 a minute, 200 a day, anonymous.
    OpenverseApi,
    /// Openverse's image proxy (`/thumb/`, the originals): 1000 a day, anonymous.
    OpenverseImages,
    /// Pexels: 200 an hour on the key.
    Pexels,
}

impl Bucket {
    pub fn source(self) -> PictureSource {
        match self {
            Bucket::OpenverseApi | Bucket::OpenverseImages => PictureSource::Openverse,
            Bucket::Pexels => PictureSource::Pexels,
        }
    }

    /// What agents together may spend in 24 hours; Pexels has no allowance (its hourly limit
    /// already bounds a loop).
    pub fn allowance(self) -> Option<usize> {
        match self {
            Bucket::OpenverseApi => Some(AGENT_SEARCHES_PER_DAY),
            Bucket::OpenverseImages => Some(AGENT_THUMBS_PER_DAY),
            Bucket::Pexels => None,
        }
    }
}

/// A bounded map that drops its oldest entry when full; putting a key again makes it newest.
#[derive(Debug)]
pub struct Recent<K, V> {
    cap: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K: Hash + Eq + Clone, V> Recent<K, V> {
    pub fn new(cap: usize) -> Self {
        Recent { cap: cap.max(1), map: HashMap::new(), order: VecDeque::new() }
    }

    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.map.get(key)
    }

    pub fn put(&mut self, key: K, value: V) {
        if self.map.insert(key.clone(), value).is_some() {
            self.order.retain(|k| k != &key);
        }
        self.order.push_back(key);
        while self.order.len() > self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// What the engine remembers of pictures between requests, all in memory: a restart loses it and
/// nothing else, because ids resolve again through detail reads (§1.7). An `Engine` field.
pub struct Desk {
    /// A fake that serves every request: tests, and the smoke run's fixtures.
    fetch: Mutex<Option<Arc<dyn Fetch>>>,
    /// The key's state, and a generation [`Desk::set_key`] advances, so a slow lookup can never
    /// overwrite a key saved or forgotten while it ran.
    key: Mutex<(u64, Key)>,
    /// The latest hits, by id.
    hits: Mutex<Recent<String, Item>>,
    /// Searches answered again for [`FRESH`].
    searches: Mutex<Recent<SearchKey, (Instant, SearchOut)>>,
    /// Until when each bucket cools after a 429.
    cool: Mutex<HashMap<Bucket, Instant>>,
    /// When agents spent each Openverse bucket in the last 24 hours, oldest first.
    allowance: Mutex<HashMap<Bucket, VecDeque<Instant>>>,
    /// When thumbnails were last swept.
    swept: Mutex<Option<Instant>>,
}

impl Default for Desk {
    fn default() -> Self {
        Desk {
            fetch: Mutex::new(None),
            key: Mutex::new((0, Key::Unknown)),
            hits: Mutex::new(Recent::new(HITS)),
            searches: Mutex::new(Recent::new(SEARCHES)),
            cool: Mutex::new(HashMap::new()),
            allowance: Mutex::new(HashMap::new()),
            swept: Mutex::new(None),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Desk {
    /// Serve every request from `fake` from now on: tests, and the smoke run's fixtures.
    pub fn set_fake_fetch(&self, fake: Arc<dyn Fetch>) {
        *lock(&self.fetch) = Some(fake);
    }

    /// What sends requests: the installed fake; on the test instance, which never goes online,
    /// nothing (`design.picture.offline`), so no test reaches the network by accident; else the
    /// real client.
    pub fn fetcher(&self, instance: Instance, source: PictureSource) -> Result<Arc<dyn Fetch>, BusError> {
        if let Some(fake) = lock(&self.fetch).clone() {
            return Ok(fake);
        }
        if instance == Instance::Test {
            return Err(offline(source, "the test instance never goes online"));
        }
        Ok(Arc::new(Ureq))
    }

    /// Sends `req` through `fetch` and reads the answer (§8.4): refused at once, with nothing sent,
    /// while its bucket cools; a 429 cools the bucket for its `Retry-After` (else 60 s). Answers
    /// the body of a 200. Holds no lock while the request runs.
    pub fn send(&self, fetch: &dyn Fetch, req: &Req) -> Result<Vec<u8>, BusError> {
        let bucket = req.bucket();
        if let Some(left) = self.cooling(bucket, Instant::now()) {
            return Err(rate_limited(bucket.source(), left));
        }
        let got = fetch.get(req).map_err(|e| net::failed(req, e))?;
        if got.status == 429 {
            self.cool(bucket, got.retry_after.unwrap_or(COOL_DEFAULT_S), Instant::now());
        }
        net::classify(req, got)
    }

    /// Seconds until `bucket` may be asked again, while it cools.
    pub fn cooling(&self, bucket: Bucket, now: Instant) -> Option<u64> {
        let until = *lock(&self.cool).get(&bucket)?;
        let left = until.checked_duration_since(now).filter(|left| !left.is_zero())?;
        Some(left.as_secs() + u64::from(left.subsec_nanos() > 0))
    }

    /// Cool `bucket` for `secs` (at least 1, at most [`COOL_MAX_S`]) from `now`; an earlier,
    /// longer cooldown stands.
    pub fn cool(&self, bucket: Bucket, secs: u64, now: Instant) {
        let until = now + Duration::from_secs(secs.clamp(1, COOL_MAX_S));
        let mut cool = lock(&self.cool);
        let at = cool.entry(bucket).or_insert(until);
        *at = (*at).max(until);
    }

    /// Counts one agent request against `bucket`'s 24-hour allowance, or refuses it with
    /// `rate_limited` until the oldest counted request leaves the window. Pexels has none. The
    /// person is never counted: call this for agents only.
    pub fn spend_allowance(&self, bucket: Bucket, now: Instant) -> Result<(), BusError> {
        let Some(limit) = bucket.allowance() else { return Ok(()) };
        let mut allowance = lock(&self.allowance);
        let spent = allowance.entry(bucket).or_default();
        while spent.front().is_some_and(|at| now.saturating_duration_since(*at) >= DAY) {
            spent.pop_front();
        }
        if spent.len() >= limit {
            let oldest = spent.front().copied().unwrap_or(now);
            let left = DAY.saturating_sub(now.saturating_duration_since(oldest));
            return Err(allowance_spent(bucket.source(), limit, left.as_secs() + u64::from(left.subsec_nanos() > 0)));
        }
        spent.push_back(now);
        Ok(())
    }

    /// Remember search results by id, so `thumb` and `add` find them without a detail read.
    pub fn remember(&self, items: impl IntoIterator<Item = Item>) {
        let mut hits = lock(&self.hits);
        for item in items {
            hits.put(item.id.clone(), item);
        }
    }

    /// A hit remembered by its id (`openverse:<uuid>` or `pexels:<digits>`).
    pub fn hit(&self, id: &str) -> Option<Item> {
        lock(&self.hits).get(id).cloned()
    }

    /// The answer to `search` if it was given less than [`FRESH`] ago.
    pub fn fresh_search(&self, search: &SearchKey, now: Instant) -> Option<SearchOut> {
        let searches = lock(&self.searches);
        let (at, out) = searches.get(search)?;
        (now.saturating_duration_since(*at) < FRESH).then(|| out.clone())
    }

    /// Keep an answer to give again for [`FRESH`].
    pub fn keep_search(&self, search: SearchKey, out: SearchOut, now: Instant) {
        lock(&self.searches).put(search, (now, out));
    }

    /// Whether thumbnails are due a sweep: never swept by this engine, or not for
    /// [`SWEEP_EVERY`]. Answering true notes `now` as the sweep's time.
    pub fn sweep_due(&self, now: Instant) -> bool {
        let mut swept = lock(&self.swept);
        let due = swept.is_none_or(|at| now.saturating_duration_since(at) >= SWEEP_EVERY);
        if due {
            *swept = Some(now);
        }
        due
    }

    /// The key's state now.
    pub fn key_state(&self) -> Key {
        lock(&self.key).1.clone()
    }

    /// The generation a loader notes before reading the keyring with no lock held.
    pub fn begin_load(&self) -> u64 {
        lock(&self.key).0
    }

    /// Keep what a loader read, unless a [`Desk::set_key`] came since its [`Desk::begin_load`]:
    /// then the newer key stands and this answer is dropped. Answers whether it was kept.
    pub fn finish_load(&self, generation: u64, key: Key) -> bool {
        let mut slot = lock(&self.key);
        let kept = slot.0 == generation;
        if kept {
            slot.1 = key;
        }
        kept
    }

    /// Set the key's state after a save or a forget, advancing the generation so a lookup that
    /// started earlier cannot overwrite it.
    pub fn set_key(&self, key: Key) {
        let mut slot = lock(&self.key);
        slot.0 += 1;
        slot.1 = key;
    }

    /// The saved Pexels key (§7.4), read with `load` when this engine does not know it yet, or
    /// when the keyring failed before and the asker is the person (who may have unlocked it
    /// since). An agent is answered `keyring` at once until the failure's time is up, so agents
    /// do not raise an unlock prompt on every call. `load` runs with no lock held, and its answer
    /// is kept only if no save or forget came meanwhile; the answer is then the newer state.
    pub fn key_with(&self, agent: bool, load: impl FnOnce() -> Key) -> Result<SecretKey, BusError> {
        let generation = {
            let slot = lock(&self.key);
            match &slot.1 {
                Key::Saved(key) => return Ok(key.clone()),
                Key::Absent => return Err(no_key()),
                Key::Unavailable(until) if agent && Instant::now() < *until => return Err(keyring_unread()),
                Key::Unknown | Key::Unavailable(_) => slot.0,
            }
        };
        self.finish_load(generation, load());
        match self.key_state() {
            Key::Saved(key) => Ok(key),
            Key::Absent => Err(no_key()),
            Key::Unknown | Key::Unavailable(_) => Err(keyring_unread()),
        }
    }
}

fn keyring_unread() -> BusError {
    keyring("the desktop keyring could not be read")
}

// ------------------------------------------------------------------ the thumbnail cache

/// Where a thumbnail of `id` is kept: `<source>-<key>.<ext>` in the picture cache. Built from a
/// checked id only, so no provider text becomes a path.
pub fn thumb_path(dir: &Path, id: &Id, ext: &str) -> PathBuf {
    dir.join(format!("{}-{}.{ext}", slug(id.source), id.key))
}

/// The thumbnail of `id` fetched before, if one is kept as a regular file (never through a link)
/// that still reads as a raster of its own extension; a corrupt one is a miss, fetched again. Its
/// modification time is set to now first, so a sweep cannot take a file a hit just handed out.
pub fn cached_thumb(dir: &Path, id: &Id) -> Option<(PathBuf, Picture)> {
    use std::os::unix::fs::OpenOptionsExt;
    let cap = Want::Thumb.cap();
    ["jpg", "png", "webp", "gif"].into_iter().find_map(|ext| {
        let path = thumb_path(dir, id, ext);
        let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(&path).ok()?;
        let meta = file.metadata().ok()?;
        if !meta.is_file() || meta.len() > cap {
            return None;
        }
        let mut bytes = Vec::new();
        (&file).take(cap).read_to_end(&mut bytes).ok()?;
        let picture = picture::raster(&bytes).filter(|p| p.ext == ext)?;
        if let Err(e) = file.set_modified(SystemTime::now()) {
            tracing::debug!(path = %path.display(), error = %e, "pictures: a cached thumbnail kept its old time");
        }
        Some((path, picture))
    })
}

/// Keep a fetched thumbnail of `id` (its bytes already checked by [`photo`]), written whole
/// through a temporary file so no reader sees half of one. Answers its path.
pub fn keep_thumb(dir: &Path, id: &Id, bytes: &[u8], picture: &Picture) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = thumb_path(dir, id, picture.ext);
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.persist(&path).map_err(|e| e.error)?;
    Ok(path)
}

/// Remove what in the picture cache nobody asked for in [`THUMB_TTL`]: regular files only, never
/// a link or a folder. Answers how many went.
pub fn sweep(dir: &Path, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .filter(|entry| {
            // DirEntry::metadata does not follow a link.
            entry.metadata().is_ok_and(|meta| {
                meta.is_file() && meta.modified().is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age >= THUMB_TTL))
            })
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}


#[cfg(test)]
mod tests {
    use super::net::{Got, NetError};
    use super::*;

    const PHOTO: &[u8] = include_bytes!("../../tests/fixtures/pictures/photo.png");
    const HUGE_HEADER: &[u8] = include_bytes!("../../tests/fixtures/pictures/huge-header.png");
    const VECTOR: &[u8] = include_bytes!("../../tests/fixtures/pictures/vector.svg");
    const UUID: &str = "48e4625b-0930-43db-990c-22858b1d633e";

    /// Answers from a list, then 200s; notes every URL it was sent.
    #[derive(Default)]
    struct Fake {
        answers: Mutex<VecDeque<Result<Got, NetError>>>,
        seen: Mutex<Vec<String>>,
    }

    impl Fake {
        fn answering(answers: Vec<Result<Got, NetError>>) -> Fake {
            Fake { answers: Mutex::new(answers.into()), seen: Mutex::default() }
        }
        fn calls(&self) -> usize {
            lock(&self.seen).len()
        }
    }

    impl Fetch for Fake {
        fn get(&self, req: &Req) -> Result<Got, NetError> {
            lock(&self.seen).push(req.url.clone());
            lock(&self.answers).pop_front().unwrap_or(Ok(Got { status: 200, retry_after: None, body: b"ok".to_vec() }))
        }
    }

    fn status(status: u16, retry_after: Option<u64>) -> Result<Got, NetError> {
        Ok(Got { status, retry_after, body: Vec::new() })
    }

    fn ov() -> Id {
        parse_id(&format!("openverse:{UUID}")).unwrap()
    }

    #[test]
    fn ids_read_in_both_forms_and_nothing_else() {
        let id = ov();
        assert_eq!((id.source, id.key.as_str(), id.to_string()), (PictureSource::Openverse, UUID, format!("openverse:{UUID}")));
        let pexels = parse_id("pexels:2014422").unwrap();
        assert_eq!((pexels.source, pexels.to_string()), (PictureSource::Pexels, "pexels:2014422".to_string()));
        assert!(parse_id("pexels:123456789012").is_ok(), "12 digits");
        for bad in ["openverse:../x", "openverse:48E4625B-0930-43DB-990C-22858B1D633E", "openverse:48e4625b09304", "pexels:",
            "pexels:1234567890123", "pexels:12a", "pexels:-1", "pexels:0123", "pexels:0", "http://x", "openverse", "", "flickr:123", " pexels:1",
            "pexels:1/../2", &format!("openverse:{UUID}/"), &format!("Openverse:{UUID}")] {
            let e = parse_id(bad).unwrap_err();
            assert_eq!(e.code, "design.picture.id", "{bad}");
        }
        let long = parse_id(&"x".repeat(5000)).unwrap_err();
        assert!(long.message.len() < 200, "an id is not echoed whole");
    }

    #[test]
    fn a_query_is_trimmed_collapsed_and_held_to_200_characters() {
        assert_eq!(normalise_query("  sunlit \t kitchen\n plants ").unwrap(), "sunlit kitchen plants");
        assert_eq!(normalise_query(&"é".repeat(200)).unwrap().chars().count(), 200);
        let refused = |q: &str| normalise_query(q).unwrap_err().code;
        assert_eq!(refused(&"é".repeat(201)), "design.picture.query");
        assert_eq!(refused(&format!("{} {}", "a".repeat(100), "b".repeat(100))), "design.picture.query", "201 with the space");
        assert_eq!(refused("   "), "design.picture.query");
        assert_eq!(refused("kitchen\u{0007}"), "design.picture.query");
        assert_eq!(refused("kitchen\u{0000}plants"), "design.picture.query");
        assert_eq!(page(None).unwrap(), 1);
        assert_eq!(page(Some(10)).unwrap(), 10);
        assert_eq!(page(Some(11)).unwrap_err().code, "design.picture.page");
        assert_eq!(page(Some(0)).unwrap_err().code, "design.picture.page");
        assert_eq!((count(None), count(Some(50)), count(Some(0)), count(Some(7))), (12, 20, 1, 7));
        assert_eq!(orientation(PictureSource::Openverse, Orientation::Landscape), "wide");
        assert_eq!(orientation(PictureSource::Openverse, Orientation::Portrait), "tall");
        assert_eq!(orientation(PictureSource::Pexels, Orientation::Portrait), "portrait");
        assert_eq!(orientation(PictureSource::Pexels, Orientation::Square), "square");
    }

    #[test]
    fn recent_drops_the_oldest() {
        let mut recent = Recent::new(3);
        for (k, v) in [("a", 1), ("b", 2), ("c", 3)] {
            recent.put(k, v);
        }
        recent.put("a", 10);
        recent.put("d", 4);
        assert_eq!((recent.get(&"a"), recent.get(&"b"), recent.get(&"c"), recent.get(&"d")), (Some(&10), None, Some(&3), Some(&4)),
            "putting a again made it newest, so b went first");
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn the_sweep_takes_only_what_nobody_asked_for_in_a_week() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let aged = |name: &str, age_days: u64| {
            let path = dir.path().join(name);
            std::fs::write(&path, PHOTO).unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(now - Duration::from_secs(age_days * DAY_S)).unwrap();
            path
        };
        let old = aged("openverse-0b5c3a8e-5a8e-4c31-9d5a-111111111111.png", 8);
        let fresh = aged("openverse-0b5c3a8e-5a8e-4c31-9d5a-222222222222.png", 6);
        let hit = aged(&format!("openverse-{UUID}.png"), 30);
        std::fs::create_dir(dir.path().join("old-folder")).unwrap();
        std::os::unix::fs::symlink(&fresh, dir.path().join("link.png")).unwrap();
        let (path, picture) = cached_thumb(dir.path(), &ov()).expect("a hit");
        assert_eq!((path.as_path(), picture.w, picture.h), (hit.as_path(), 64, 64));
        assert_eq!(sweep(dir.path(), now), 1);
        assert!(!old.exists() && fresh.exists() && hit.exists(), "the hit's refreshed file survives");
        assert!(dir.path().join("old-folder").is_dir() && dir.path().join("link.png").symlink_metadata().is_ok());
        assert_eq!(sweep(&dir.path().join("missing"), now), 0);
    }

    #[test]
    fn a_kept_thumbnail_is_a_hit_until_it_stops_reading_as_one() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("picture-cache");
        let id = ov();
        assert!(cached_thumb(&cache, &id).is_none());
        let picture = photo(PHOTO).unwrap();
        let path = keep_thumb(&cache, &id, PHOTO, &picture).unwrap();
        assert_eq!(path, cache.join(format!("openverse-{UUID}.png")));
        assert_eq!(cached_thumb(&cache, &id).map(|(p, _)| p), Some(path.clone()));
        let leftovers: Vec<_> = std::fs::read_dir(&cache).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(leftovers.len(), 1, "no temporary file stays: {leftovers:?}");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\ncut short").unwrap();
        assert!(cached_thumb(&cache, &id).is_none(), "corrupt");
        std::fs::remove_file(&path).unwrap();
        std::fs::write(cache.join(format!("openverse-{UUID}.jpg")), PHOTO).unwrap();
        assert!(cached_thumb(&cache, &id).is_none(), "a PNG under .jpg");
        std::fs::remove_file(cache.join(format!("openverse-{UUID}.jpg"))).unwrap();
        let elsewhere = dir.path().join("elsewhere.png");
        std::fs::write(&elsewhere, PHOTO).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        assert!(cached_thumb(&cache, &id).is_none(), "never through a link");
        let pexels = parse_id("pexels:2014422").unwrap();
        assert_eq!(thumb_path(&cache, &pexels, "jpg"), cache.join("pexels-2014422.jpg"));
    }

    #[test]
    fn a_photo_is_a_raster_of_at_most_25_megapixels() {
        let picture = photo(PHOTO).unwrap();
        assert_eq!((picture.ext, picture.w, picture.h), ("png", 64, 64));
        assert_eq!(photo(VECTOR).unwrap_err().code, "design.picture.not_raster");
        assert_eq!(photo(b"<html>").unwrap_err().code, "design.picture.not_raster");
        let huge = photo(HUGE_HEADER).unwrap_err();
        assert_eq!((huge.code.as_str(), huge.message.as_str()), ("design.picture.too_big", "the photo is 70000 × 1000 px, over 25000000 pixels"));
    }

    #[test]
    fn a_poisoned_desk_mutex_is_still_usable() {
        let desk = Arc::new(Desk::default());
        let poisoner = desk.clone();
        let panicked = std::thread::spawn(move || {
            let _held = poisoner.cool.lock().unwrap();
            let _key = poisoner.key.lock().unwrap();
            panic!("poison the desk");
        })
        .join();
        assert!(panicked.is_err() && desk.cool.is_poisoned());
        let now = Instant::now();
        desk.cool(Bucket::Pexels, 30, now);
        assert_eq!(desk.cooling(Bucket::Pexels, now), Some(30));
        desk.set_key(Key::Absent);
        assert_eq!(desk.key_state(), Key::Absent);
    }

    #[test]
    fn agents_share_a_daily_openverse_allowance() {
        let desk = Desk::default();
        let start = Instant::now();
        for i in 0..AGENT_SEARCHES_PER_DAY {
            desk.spend_allowance(Bucket::OpenverseApi, start + Duration::from_secs(i as u64)).unwrap();
        }
        let refused = desk.spend_allowance(Bucket::OpenverseApi, start + Duration::from_secs(3600)).unwrap_err();
        assert_eq!(refused.code, "design.picture.rate_limited");
        assert_eq!(refused.details, Some(json!({"source": "openverse", "retry_after_s": DAY_S - 3600})), "when the oldest leaves");
        desk.spend_allowance(Bucket::OpenverseApi, start + DAY).expect("the oldest is 24 hours old");
        assert!(desk.spend_allowance(Bucket::OpenverseApi, start + DAY).is_err(), "only the oldest left");
        for _ in 0..AGENT_THUMBS_PER_DAY {
            desk.spend_allowance(Bucket::OpenverseImages, start).unwrap();
        }
        assert!(desk.spend_allowance(Bucket::OpenverseImages, start).is_err(), "thumbnails count apart, to 300");
        for _ in 0..1000 {
            desk.spend_allowance(Bucket::Pexels, start).unwrap();
        }
    }

    #[test]
    fn a_late_lookup_never_overwrites_a_newer_key() {
        let desk = Desk::default();
        let generation = desk.begin_load();
        assert_eq!(generation, 0);
        desk.set_key(Key::Saved(SecretKey::new("new")));
        assert!(!desk.finish_load(generation, Key::Absent));
        assert_eq!(desk.key_state(), Key::Saved(SecretKey::new("new")));
        let generation = desk.begin_load();
        desk.set_key(Key::Absent);
        assert!(!desk.finish_load(generation, Key::Saved(SecretKey::new("old"))));
        assert_eq!(desk.key_state(), Key::Absent);
        let generation = desk.begin_load();
        assert!(desk.finish_load(generation, Key::Saved(SecretKey::new("read"))), "nothing came between");
        assert_eq!(desk.key_state(), Key::Saved(SecretKey::new("read")));
    }

    #[test]
    fn the_key_is_read_once_and_agents_wait_out_a_keyring_failure() {
        let desk = Desk::default();
        let never = || -> Key { panic!("the keyring is not asked again") };
        assert_eq!(desk.key_with(true, || Key::Absent).unwrap_err().code, "design.picture.no_key");
        assert_eq!(desk.key_with(false, never).unwrap_err().code, "design.picture.no_key");
        desk.set_key(Key::Unknown);
        let failed = desk.key_with(true, || Key::Unavailable(Instant::now() + Duration::from_secs(60))).unwrap_err();
        assert_eq!(failed.code, "design.picture.keyring");
        assert_eq!(desk.key_with(true, never).unwrap_err().code, "design.picture.keyring", "agents are refused at once");
        let read = desk.key_with(false, || Key::Saved(SecretKey::new("k"))).unwrap();
        assert_eq!(read, SecretKey::new("k"), "the person's call asks the keyring again");
        assert_eq!(desk.key_with(true, never).unwrap(), SecretKey::new("k"));
        desk.set_key(Key::Unavailable(Instant::now()));
        assert!(desk.key_with(true, || Key::Saved(SecretKey::new("k2"))).is_ok(), "the agents' wait is over");
        // A save during a slow lookup stands, and the lookup's caller is answered with it.
        desk.set_key(Key::Unknown);
        let saved = desk.key_with(true, || {
            desk.set_key(Key::Saved(SecretKey::new("saved meanwhile")));
            Key::Absent
        });
        assert_eq!(saved.unwrap(), SecretKey::new("saved meanwhile"));
    }

    #[test]
    fn the_key_never_shows() {
        let key = SecretKey::new("sekrit-pexels-key");
        assert_eq!(format!("{key:?}"), "SecretKey(…)");
        assert!(!format!("{:?}", Key::Saved(key.clone())).contains("sekrit"));
        assert_eq!(key.expose(), "sekrit-pexels-key");
    }

    #[test]
    fn a_429_cools_its_bucket_without_waiting() {
        let desk = Desk::default();
        let fake = Fake::answering(vec![status(429, Some(30))]);
        let search = Req::openverse_search("kitchen", None, 1, 12);
        let started = Instant::now();
        let e = desk.send(&fake, &search).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!((e.code.as_str(), e.details), ("design.picture.rate_limited", Some(json!({"source": "openverse", "retry_after_s": 30}))));
        let again = desk.send(&fake, &search).unwrap_err();
        assert_eq!(again.code, "design.picture.rate_limited");
        assert!(matches!(again.details.as_ref().and_then(|d| d["retry_after_s"].as_u64()), Some(29 | 30)));
        assert_eq!(fake.calls(), 1, "nothing was sent while it cools");
        assert_eq!(desk.send(&fake, &Req::openverse_thumb(&ov())).unwrap(), b"ok", "thumbnails spend their own bucket");
        let pexels = Desk::default();
        let fake = Fake::answering(vec![status(429, None)]);
        assert!(pexels.send(&fake, &Req::pexels_key_check(SecretKey::new("k"))).is_err());
        assert_eq!(pexels.cooling(Bucket::Pexels, Instant::now()).map(|s| s > 58), Some(true), "60 s without Retry-After");
    }

    #[test]
    fn an_openverse_thumbnail_429_does_not_cool_searches() {
        let desk = Desk::default();
        let fake = Fake::answering(vec![status(429, Some(30))]);
        assert!(desk.send(&fake, &Req::openverse_thumb(&ov())).is_err());
        assert!(desk.cooling(Bucket::OpenverseImages, Instant::now()).is_some());
        assert_eq!(desk.send(&fake, &Req::openverse_search("kitchen", None, 1, 12)).unwrap(), b"ok");
    }

    #[test]
    fn a_dead_photo_is_gone_with_no_cooldown() {
        let desk = Desk::default();
        let fake = Fake::answering(vec![status(424, None)]);
        assert_eq!(desk.send(&fake, &Req::openverse_thumb(&ov())).unwrap_err().code, "design.picture.gone");
        assert_eq!(desk.cooling(Bucket::OpenverseImages, Instant::now()), None);
        assert_eq!(desk.send(&fake, &Req::openverse_thumb(&ov())).unwrap(), b"ok");
        assert_eq!(fake.calls(), 2);
        let offline = Fake::answering(vec![Err(NetError::Offline)]);
        let e = desk.send(&offline, &Req::openverse_search("kitchen", None, 1, 12)).unwrap_err();
        assert_eq!((e.code.as_str(), e.details), ("design.picture.offline", Some(json!({"source": "openverse"}))));
    }

    #[test]
    fn the_test_instance_never_goes_online() {
        let desk = Desk::default();
        let e = desk.fetcher(Instance::Test, PictureSource::Pexels).err().expect("no fake, no client");
        assert_eq!((e.code.as_str(), e.details), ("design.picture.offline", Some(json!({"source": "pexels"}))));
        assert!(desk.fetcher(Instance::Dev, PictureSource::Openverse).is_ok());
        let fake = Arc::new(Fake::default());
        desk.set_fake_fetch(fake.clone());
        let fetch = desk.fetcher(Instance::Test, PictureSource::Openverse).unwrap();
        assert_eq!(desk.send(&*fetch, &Req::openverse_search("kitchen", None, 1, 12)).unwrap(), b"ok");
        assert_eq!(fake.calls(), 1);
    }

    #[test]
    fn hits_and_searches_are_remembered_for_their_while() {
        let desk = Desk::default();
        let body = include_str!("../../tests/fixtures/pictures/openverse-search.json");
        let (items, has_more) = parse::openverse_page(body.as_bytes(), 1).unwrap();
        desk.remember(items.clone());
        assert_eq!(desk.hit(&items[2].id), Some(items[2].clone()));
        assert_eq!(desk.hit("openverse:0b5c3a8e-5a8e-4c31-9d5a-111111111111"), None, "a dropped result is no hit");
        let search = SearchKey { source: PictureSource::Openverse, query: "kitchen".into(), orientation: None, page: 1, count: 12 };
        let out = SearchOut { source: PictureSource::Openverse, page: 1, has_more, results: items.iter().map(Item::hit).collect() };
        let at = Instant::now();
        desk.keep_search(search.clone(), out.clone(), at);
        assert_eq!(desk.fresh_search(&search, at + FRESH - Duration::from_secs(1)), Some(out));
        assert_eq!(desk.fresh_search(&search, at + FRESH), None, "nothing older than an hour");
        assert_eq!(desk.fresh_search(&SearchKey { page: 2, ..search }, at), None);
        let many = Desk { hits: Mutex::new(Recent::new(2)), ..Desk::default() };
        many.remember(items.iter().take(3).cloned());
        assert_eq!((many.hit(&items[0].id), many.hit(&items[2].id).is_some()), (None, true));
    }

    #[test]
    fn the_sweep_is_due_once_a_day() {
        let desk = Desk::default();
        let at = Instant::now();
        assert!(desk.sweep_due(at), "never swept by this engine");
        assert!(!desk.sweep_due(at + Duration::from_secs(3600)));
        assert!(desk.sweep_due(at + SWEEP_EVERY));
        assert!(!desk.sweep_due(at + SWEEP_EVERY));
    }
}
