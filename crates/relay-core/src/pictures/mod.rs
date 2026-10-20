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

use relay_bus::ops::picture::Orientation;
pub use relay_bus::ops::picture::{PictureCredit, PictureHit, PictureSource, PLUGIN};
use relay_bus::BusError;
use serde_json::json;
use std::fmt;

/// Openverse searches agents together may send in any 24 hours: half its anonymous day, so they
/// can never spend the person's share (§1.8).
pub const AGENT_SEARCHES_PER_DAY: usize = 100;
/// Uncached Openverse thumbnails agents together may fetch in any 24 hours.
pub const AGENT_THUMBS_PER_DAY: usize = 300;
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

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "48e4625b-0930-43db-990c-22858b1d633e";

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
    fn the_key_never_shows() {
        let key = SecretKey::new("sekrit-pexels-key");
        assert_eq!(format!("{key:?}"), "SecretKey(…)");
        assert_eq!(key.expose(), "sekrit-pexels-key");
    }
}
