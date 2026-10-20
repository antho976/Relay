//! The client for the Pictures providers (pictures-SPEC-v2 §8): three fixed hosts, https only,
//! never following a redirect, every body capped and every request under one deadline.
//!
//! No caller passes a URL. Every request is built here from constants and a checked id, except a
//! Pexels picture link, which [`super::parse`] kept only when it is https on images.pexels.com.
//! [`allowed_host`] is checked again before every send. The Pexels key goes only to
//! api.pexels.com, as `Authorization`. The seam is [`Fetch`]: tests and the smoke run's fixtures
//! install their own, and the test instance never builds the real client ([`super::Desk::fetcher`]).

use super::{gone, key_rejected, offline, orientation, provider, rate_limited, slug, too_big, Bucket, Id, SecretKey, COOL_DEFAULT_S, COOL_MAX_S, MAX_COUNT};
use relay_bus::design::MAX_ASSET_FILE;
use relay_bus::ops::picture::{Orientation, PictureSource};
use relay_bus::BusError;
use std::sync::OnceLock;
use std::time::Duration;

pub const OPENVERSE_HOST: &str = "api.openverse.org";
pub const PEXELS_API_HOST: &str = "api.pexels.com";
pub const PEXELS_IMAGES_HOST: &str = "images.pexels.com";
const HOSTS: [&str; 3] = [OPENVERSE_HOST, PEXELS_API_HOST, PEXELS_IMAGES_HOST];
const OPENVERSE_IMAGES: &str = "https://api.openverse.org/v1/images/";
const PEXELS_API: &str = "https://api.pexels.com/v1/";
/// Only what both allow anyone to use commercially and change; the allowlist after parsing is the
/// guarantee, this only saves results that would be dropped.
const OPENVERSE_LICENCES: &str = "commercial,modification";

/// What a request fetches, which sets its limits (§8.5) and how its failures read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Search,
    /// One photo's description, by id.
    Detail,
    /// A small preview.
    Thumb,
    /// The picture `add` stores.
    Original,
    /// The Pexels search that checks a key before it is saved.
    KeyCheck,
}

impl Want {
    /// The most bytes its answer may have.
    pub fn cap(self) -> u64 {
        match self {
            Want::Search | Want::Detail | Want::Thumb => 2 << 20,
            Want::Original => MAX_ASSET_FILE,
            Want::KeyCheck => 64 << 10,
        }
    }

    /// The time from sending it to the end of its answer.
    pub fn deadline(self) -> Duration {
        match self {
            Want::Original => Duration::from_secs(30),
            _ => Duration::from_secs(8),
        }
    }

    /// Whether it names one photo, so a 404 means the photo is gone.
    fn names_a_photo(self) -> bool {
        matches!(self, Want::Detail | Want::Thumb | Want::Original)
    }
}

/// One GET to a provider.
#[derive(Debug, Clone)]
pub struct Req {
    pub source: PictureSource,
    pub want: Want,
    pub url: String,
    /// Query parameters; ureq encodes them.
    pub query: Vec<(&'static str, String)>,
    /// The Pexels key, sent as `Authorization` and only to api.pexels.com.
    pub key: Option<SecretKey>,
    pub cap: u64,
    pub deadline: Duration,
}

impl Req {
    fn new(source: PictureSource, want: Want, url: String) -> Req {
        Req { source, want, url, query: Vec::new(), key: None, cap: want.cap(), deadline: want.deadline() }
    }

    /// The limit it spends.
    pub fn bucket(&self) -> Bucket {
        match (self.source, self.want) {
            (PictureSource::Pexels, _) => Bucket::Pexels,
            (PictureSource::Openverse, Want::Thumb | Want::Original) => Bucket::OpenverseImages,
            (PictureSource::Openverse, _) => Bucket::OpenverseApi,
        }
    }

    /// `GET /v1/images/`: only licences that allow commercial use and changes, nothing mature, at
    /// most [`MAX_COUNT`] a page (Openverse's anonymous limit).
    pub fn openverse_search(query: &str, shape: Option<Orientation>, page: u32, count: u32) -> Req {
        let mut req = Req::new(PictureSource::Openverse, Want::Search, OPENVERSE_IMAGES.to_string());
        req.query = vec![
            ("q", query.to_string()),
            ("license_type", OPENVERSE_LICENCES.to_string()),
            ("mature", "false".to_string()),
            ("page_size", count.clamp(1, MAX_COUNT).to_string()),
            ("page", page.to_string()),
        ];
        if let Some(shape) = shape {
            req.query.push(("aspect_ratio", orientation(PictureSource::Openverse, shape).to_string()));
        }
        req
    }

    /// `GET /v1/images/<uuid>/`.
    pub fn openverse_detail(id: &Id) -> Req {
        debug_assert_eq!(id.source, PictureSource::Openverse);
        Req::new(PictureSource::Openverse, Want::Detail, format!("{OPENVERSE_IMAGES}{}/", id.key))
    }

    /// `GET /v1/images/<uuid>/thumb/`: Openverse's own small copy, from its image proxy.
    pub fn openverse_thumb(id: &Id) -> Req {
        debug_assert_eq!(id.source, PictureSource::Openverse);
        Req::new(PictureSource::Openverse, Want::Thumb, format!("{OPENVERSE_IMAGES}{}/thumb/", id.key))
    }

    /// `GET /v1/images/<uuid>/thumb/?full_size=true`: the original, through the same proxy, so no
    /// request ever goes to the photographer's own host.
    pub fn openverse_original(id: &Id) -> Req {
        debug_assert_eq!(id.source, PictureSource::Openverse);
        let mut req = Req::new(PictureSource::Openverse, Want::Original, format!("{OPENVERSE_IMAGES}{}/thumb/", id.key));
        req.query.push(("full_size", "true".to_string()));
        req
    }

    /// `GET /v1/search` with the key.
    pub fn pexels_search(key: SecretKey, query: &str, shape: Option<Orientation>, page: u32, count: u32) -> Req {
        let mut req = Req::new(PictureSource::Pexels, Want::Search, format!("{PEXELS_API}search"));
        req.query = vec![("query", query.to_string()), ("per_page", count.clamp(1, MAX_COUNT).to_string()), ("page", page.to_string())];
        if let Some(shape) = shape {
            req.query.push(("orientation", orientation(PictureSource::Pexels, shape).to_string()));
        }
        req.key = Some(key);
        req
    }

    /// `GET /v1/photos/<id>` with the key.
    pub fn pexels_detail(key: SecretKey, id: &Id) -> Req {
        debug_assert_eq!(id.source, PictureSource::Pexels);
        let mut req = Req::new(PictureSource::Pexels, Want::Detail, format!("{PEXELS_API}photos/{}", id.key));
        req.key = Some(key);
        req
    }

    /// The smallest search, to learn whether Pexels takes `key` before it is saved.
    pub fn pexels_key_check(key: SecretKey) -> Req {
        let mut req = Req::new(PictureSource::Pexels, Want::KeyCheck, format!("{PEXELS_API}search"));
        req.query = vec![("query", "nature".to_string()), ("per_page", "1".to_string())];
        req.key = Some(key);
        req
    }

    /// A Pexels picture (`src.medium` for `Thumb`, `src.large2x` for `Original`), never with the
    /// key; `None` unless `url` is https on images.pexels.com.
    pub fn pexels_image(url: &str, want: Want) -> Option<Req> {
        (allowed_host(url) == Some(PEXELS_IMAGES_HOST)).then(|| Req::new(PictureSource::Pexels, want, url.to_string()))
    }
}

/// A provider's answer: any status, read whole within the request's cap.
#[derive(Debug, Clone, PartialEq)]
pub struct Got {
    pub status: u16,
    /// `Retry-After` in whole seconds (an HTTP date is not read), at most [`COOL_MAX_S`].
    pub retry_after: Option<u64>,
    pub body: Vec<u8>,
}

/// Why no answer came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// No name, no connection, no TLS: nothing reached the provider.
    Offline,
    /// The deadline passed.
    Timeout,
    /// The answer was over the cap.
    TooBig,
    /// A URL outside the three hosts: an engine bug, never sent.
    NotAllowed,
}

/// What sends requests: [`Ureq`], a test's fake, or the smoke run's fixtures.
pub trait Fetch: Send + Sync {
    fn get(&self, req: &Req) -> Result<Got, NetError>;
}

/// Which of the three hosts `url` is on: https, the host exactly, no port but 443, no user.
pub fn allowed_host(url: &str) -> Option<&'static str> {
    let uri: ureq::http::Uri = url.parse().ok()?;
    if uri.scheme_str() != Some("https") {
        return None;
    }
    let authority = uri.authority()?;
    if authority.as_str().contains('@') || !matches!(authority.port_u16(), None | Some(443)) {
        return None;
    }
    HOSTS.into_iter().find(|host| *host == authority.host())
}

/// Whether `url` may be sent at all.
pub fn host_allowed(url: &str) -> bool {
    allowed_host(url).is_some()
}

/// `Retry-After` as whole seconds, at most [`COOL_MAX_S`].
pub fn retry_after(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().map(|secs| secs.min(COOL_MAX_S))
}

/// A status as the answer or its refusal (§8.4). Pure. A 429 answers `rate_limited` here;
/// [`super::Desk::send`] also cools the bucket. A 3xx is never followed: nothing goes to its
/// `Location`.
pub fn classify(req: &Req, got: Got) -> Result<Vec<u8>, BusError> {
    let source = req.source;
    match got.status {
        200 => Ok(got.body),
        429 => Err(rate_limited(source, got.retry_after.unwrap_or(COOL_DEFAULT_S))),
        401 | 403 if req.key.is_some() => Err(key_rejected()),
        404 if req.want.names_a_photo() => Err(gone(source)),
        // Openverse's image proxy, when the photo's own host failed (UpstreamThumbnailException):
        // its index holds many dead links, so pick another, and no cooldown.
        424 if req.bucket() == Bucket::OpenverseImages => Err(gone(source)),
        status @ 300..=399 => Err(provider(source, Some(status), format!("answered HTTP {status}, a redirect Relay does not follow"))),
        status => Err(provider(source, Some(status), format!("answered HTTP {status}"))),
    }
}

/// A request that got no answer, as its refusal.
pub fn failed(req: &Req, e: NetError) -> BusError {
    match e {
        NetError::Offline => offline(req.source, "no connection"),
        NetError::Timeout => provider(req.source, None, format!("did not answer within {} s", req.deadline.as_secs())),
        NetError::TooBig if req.want == Want::Original => too_big(format!("over {} MiB", req.cap >> 20)),
        NetError::TooBig => provider(req.source, None, format!("answered with more than {} KiB", req.cap >> 10)),
        NetError::NotAllowed => BusError::internal(format!("pictures: a {} request to a host outside the allowed three", slug(req.source))),
    }
}

/// The real client: one agent for the engine's life.
pub struct Ureq;

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| settings().https_only(true).build().into())
}

/// What every request shares: a status is an answer, not an error; a redirect is answered, never
/// followed; 4 s to connect. The proxy from the environment is kept: the host check is on the URL.
fn settings() -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_connect(Some(Duration::from_secs(4)))
        .user_agent("relay-pictures")
}

impl Fetch for Ureq {
    fn get(&self, req: &Req) -> Result<Got, NetError> {
        send(agent(), req, host_allowed)
    }
}

fn send(agent: &ureq::Agent, req: &Req, allowed: fn(&str) -> bool) -> Result<Got, NetError> {
    if !allowed(&req.url) {
        return Err(NetError::NotAllowed);
    }
    // timeout_global runs from the call to the end of the body read.
    let mut answer = build(agent, req).config().timeout_global(Some(req.deadline)).build().call().map_err(lost)?;
    let status = answer.status().as_u16();
    let retry = answer.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(retry_after);
    // One byte past the cap, so a body of exactly the cap reads and anything longer is refused.
    let body = answer.body_mut().with_config().limit(req.cap.saturating_add(1)).read_to_vec().map_err(lost)?;
    if body.len() as u64 > req.cap {
        return Err(NetError::TooBig);
    }
    Ok(Got { status, retry_after: retry, body })
}

/// The request as it is sent: its query, and the key only when the host is the Pexels API.
fn build(agent: &ureq::Agent, req: &Req) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    let mut builder = agent.get(&req.url);
    for (name, value) in &req.query {
        builder = builder.query(*name, value);
    }
    if let Some(key) = req.key.as_ref().filter(|_| allowed_host(&req.url) == Some(PEXELS_API_HOST)) {
        builder = builder.header("authorization", key.expose());
    }
    builder
}

fn lost(e: ureq::Error) -> NetError {
    match e {
        ureq::Error::Timeout(_) => NetError::Timeout,
        ureq::Error::BodyExceedsLimit(_) => NetError::TooBig,
        e => {
            // Kept for the log, never for the answer.
            tracing::debug!(error = %e, "pictures: a request got no answer");
            NetError::Offline
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pictures::parse_id;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    fn ov() -> Id {
        parse_id("openverse:48e4625b-0930-43db-990c-22858b1d633e").unwrap()
    }

    fn got(status: u16, retry_after: Option<u64>) -> Got {
        Got { status, retry_after, body: b"body".to_vec() }
    }

    fn code(r: Result<Vec<u8>, BusError>) -> (String, Option<serde_json::Value>) {
        let e = r.unwrap_err();
        (e.code, e.details)
    }

    #[test]
    fn only_the_three_hosts_over_https_are_allowed() {
        for url in ["https://api.openverse.org/v1/images/", "https://api.pexels.com/v1/search",
            "https://images.pexels.com/photos/1/pexels-photo-1.jpeg?h=350", "https://api.openverse.org:443/v1/images/"] {
            assert!(host_allowed(url), "{url}");
        }
        for url in ["http://api.openverse.org/v1/images/", "https://api.openverse.org.evil.example/v1/",
            "https://evil.api.openverse.org/v1/", "https://127.0.0.1/v1/", "https://api.openverse.org:8443/v1/",
            "https://user@api.openverse.org/v1/", "https://user:pw@api.pexels.com/v1/", "https://API.openverse.org/v1/",
            "api.openverse.org/v1/images/", "ftp://api.pexels.com/", "https://api.openverse.org./v1/", ""] {
            assert!(!host_allowed(url), "{url}");
        }
        assert_eq!(allowed_host("https://images.pexels.com/p.jpeg"), Some(PEXELS_IMAGES_HOST));
    }

    #[test]
    fn requests_are_built_from_constants_and_a_checked_id() {
        let search = Req::openverse_search("sunlit kitchen", Some(Orientation::Landscape), 2, 50);
        assert_eq!(search.url, "https://api.openverse.org/v1/images/");
        assert_eq!(search.query, [("q", "sunlit kitchen".to_string()), ("license_type", "commercial,modification".into()),
            ("mature", "false".into()), ("page_size", "20".into()), ("page", "2".into()), ("aspect_ratio", "wide".into())]);
        assert_eq!((search.bucket(), search.cap, search.deadline), (Bucket::OpenverseApi, 2 << 20, Duration::from_secs(8)));
        assert_eq!(Req::openverse_search("x", Some(Orientation::Portrait), 1, 1).query.last().unwrap().1, "tall");
        assert_eq!(Req::openverse_detail(&ov()).url, "https://api.openverse.org/v1/images/48e4625b-0930-43db-990c-22858b1d633e/");
        let thumb = Req::openverse_thumb(&ov());
        assert_eq!((thumb.url.as_str(), thumb.bucket()), ("https://api.openverse.org/v1/images/48e4625b-0930-43db-990c-22858b1d633e/thumb/", Bucket::OpenverseImages));
        let original = Req::openverse_original(&ov());
        assert_eq!((original.query.clone(), original.bucket(), original.cap, original.deadline),
            (vec![("full_size", "true".to_string())], Bucket::OpenverseImages, 20 << 20, Duration::from_secs(30)));
        let pexels = Req::pexels_search(SecretKey::new("k"), "plants", Some(Orientation::Square), 1, 12);
        assert_eq!(pexels.url, "https://api.pexels.com/v1/search");
        assert_eq!(pexels.query, [("query", "plants".to_string()), ("per_page", "12".into()), ("page", "1".into()), ("orientation", "square".into())]);
        assert_eq!((pexels.bucket(), pexels.key.is_some()), (Bucket::Pexels, true));
        assert_eq!(Req::pexels_detail(SecretKey::new("k"), &parse_id("pexels:2014422").unwrap()).url, "https://api.pexels.com/v1/photos/2014422");
        let check = Req::pexels_key_check(SecretKey::new("k"));
        assert_eq!((check.query.len(), check.cap), (2, 64 << 10));
        let image = Req::pexels_image("https://images.pexels.com/photos/1/p.jpeg?h=350", Want::Thumb).unwrap();
        assert_eq!((image.key.is_some(), image.bucket()), (false, Bucket::Pexels));
        assert!(Req::pexels_image("https://api.pexels.com/v1/photos/1", Want::Thumb).is_none());
        assert!(Req::pexels_image("https://live.staticflickr.com/1/p.jpg", Want::Original).is_none());
    }

    #[test]
    fn the_key_reaches_only_the_pexels_api() {
        let agent: ureq::Agent = ureq::Agent::config_builder().build().into();
        let auth = |req: &Req| build(&agent, req).headers_ref().and_then(|h| h.get("authorization")).map(|v| v.to_str().unwrap().to_string());
        assert_eq!(auth(&Req::pexels_search(SecretKey::new("sekrit"), "plants", None, 1, 12)).as_deref(), Some("sekrit"));
        let mut image = Req::pexels_image("https://images.pexels.com/photos/1/p.jpeg", Want::Original).unwrap();
        image.key = Some(SecretKey::new("sekrit"));
        assert_eq!(auth(&image), None, "an image host never sees it, even when a request carries it");
        let mut search = Req::openverse_search("plants", None, 1, 12);
        search.key = Some(SecretKey::new("sekrit"));
        assert_eq!(auth(&search), None);
        let mut loopback = Req::pexels_key_check(SecretKey::new("sekrit"));
        loopback.url = "http://127.0.0.1:9/v1/search".into();
        assert_eq!(auth(&loopback), None);
        assert_eq!(format!("{:?}", Req::pexels_key_check(SecretKey::new("sekrit"))).matches("sekrit").count(), 0);
    }

    #[test]
    fn every_status_has_its_answer() {
        let search = Req::openverse_search("x", None, 1, 12);
        let detail = Req::openverse_detail(&ov());
        let thumb = Req::openverse_thumb(&ov());
        let original = Req::openverse_original(&ov());
        let pexels = Req::pexels_search(SecretKey::new("k"), "x", None, 1, 12);
        let pexels_image = Req::pexels_image("https://images.pexels.com/p.jpeg", Want::Thumb).unwrap();
        let ov = json!({"source": "openverse"});
        let status = |s: u16| json!({"source": "openverse", "status": s});
        assert_eq!(classify(&search, got(200, None)).unwrap(), b"body");
        for s in [301, 302, 307, 308] {
            assert_eq!(code(classify(&detail, got(s, None))), ("design.picture.provider".into(), Some(status(s))));
        }
        assert_eq!(code(classify(&pexels, got(401, None))).0, "design.picture.key_rejected");
        assert_eq!(code(classify(&Req::pexels_key_check(SecretKey::new("k")), got(403, None))).0, "design.picture.key_rejected");
        assert_eq!(code(classify(&pexels_image, got(403, None))).0, "design.picture.provider", "no key was sent there");
        assert_eq!(code(classify(&search, got(401, None))), ("design.picture.provider".into(), Some(status(401))));
        assert_eq!(code(classify(&detail, got(404, None))), ("design.picture.gone".into(), Some(ov.clone())));
        assert_eq!(code(classify(&original, got(404, None))).0, "design.picture.gone");
        assert_eq!(code(classify(&search, got(404, None))).0, "design.picture.provider", "a search names no photo");
        assert_eq!(code(classify(&thumb, got(424, None))), ("design.picture.gone".into(), Some(ov.clone())));
        assert_eq!(code(classify(&original, got(424, None))).0, "design.picture.gone");
        assert_eq!(code(classify(&detail, got(424, None))).0, "design.picture.provider", "424 is the image proxy's");
        for s in [400, 422, 418] {
            assert_eq!(code(classify(&search, got(s, None))), ("design.picture.provider".into(), Some(status(s))));
        }
        assert_eq!(code(classify(&search, got(429, Some(30)))),
            ("design.picture.rate_limited".into(), Some(json!({"source": "openverse", "retry_after_s": 30}))));
        assert_eq!(code(classify(&pexels, got(429, None))),
            ("design.picture.rate_limited".into(), Some(json!({"source": "pexels", "retry_after_s": 60}))));
        for s in [500, 502, 503] {
            assert_eq!(code(classify(&search, got(s, None))), ("design.picture.provider".into(), Some(status(s))));
        }
        assert_eq!(code(classify(&search, got(204, None))).0, "design.picture.provider");
    }

    #[test]
    fn a_request_with_no_answer_says_why() {
        let search = Req::openverse_search("x", None, 1, 12);
        let original = Req::openverse_original(&ov());
        assert_eq!(failed(&search, NetError::Offline).code, "design.picture.offline");
        assert_eq!(failed(&search, NetError::Timeout).code, "design.picture.provider");
        assert_eq!(failed(&search, NetError::Timeout).message, "Openverse did not answer within 8 s");
        assert_eq!(failed(&search, NetError::TooBig).code, "design.picture.provider");
        let big = failed(&original, NetError::TooBig);
        assert_eq!((big.code.as_str(), big.message.as_str()), ("design.picture.too_big", "the photo is over 20 MiB"));
        assert_eq!(failed(&search, NetError::NotAllowed).code, "internal");
        assert_eq!(retry_after(" 30 "), Some(30));
        assert_eq!(retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(retry_after("99999999999999"), Some(COOL_MAX_S));
    }

    // ---- the transport, against a server on the loopback ----

    /// The shipped client's settings, but allowed plain http to the loopback, with no proxy.
    fn loopback() -> ureq::Agent {
        settings().proxy(None).build().into()
    }

    fn on_loopback(url: &str) -> bool {
        url.starts_with("http://127.0.0.1:")
    }

    fn get(url: String, cap: u64, deadline: Duration) -> Result<Got, NetError> {
        let mut req = Req::openverse_search("x", None, 1, 12);
        (req.url, req.cap, req.deadline) = (url, cap, deadline);
        send(&loopback(), &req, on_loopback)
    }

    /// A server that answers every connection with `reply` (after reading the request's head),
    /// counting them.
    fn serve(reply: impl Fn(&mut TcpStream) + Send + Sync + 'static) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counted = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                counted.fetch_add(1, Ordering::SeqCst);
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                    head.push(byte[0]);
                }
                reply(&mut stream);
            }
        });
        (url, hits)
    }

    #[test]
    fn a_body_over_its_cap_is_too_big() {
        let (url, _) = serve(|s| {
            let body = vec![b'x'; 3 << 20];
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len()).as_bytes());
            let _ = s.write_all(&body);
        });
        assert_eq!(get(url.clone(), 2 << 20, Duration::from_secs(8)), Err(NetError::TooBig));
        let exact = get(url, 3 << 20, Duration::from_secs(8)).unwrap();
        assert_eq!(exact.body.len(), 3 << 20, "exactly the cap reads");
    }

    #[test]
    fn a_redirect_is_answered_never_followed() {
        let (second, second_hits) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        });
        let (first, _) = serve(move |s| {
            let _ = s.write_all(format!("HTTP/1.1 302 Found\r\nlocation: {second}\r\ncontent-length: 0\r\n\r\n").as_bytes());
        });
        let got = get(first, 2 << 20, Duration::from_secs(8)).unwrap();
        assert_eq!(got.status, 302);
        assert_eq!(second_hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_server_that_never_answers_times_out_at_the_deadline() {
        let (url, _) = serve(|s| {
            let held = s.try_clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(10));
                drop(held);
            });
        });
        let started = Instant::now();
        assert_eq!(get(url, 2 << 20, Duration::from_secs(1)), Err(NetError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    }

    #[test]
    fn retry_after_is_read() {
        let (url, _) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 429 Too Many Requests\r\nretry-after: 30\r\ncontent-length: 0\r\n\r\n");
        });
        assert_eq!(get(url, 2 << 20, Duration::from_secs(8)), Ok(Got { status: 429, retry_after: Some(30), body: Vec::new() }));
    }

    #[test]
    fn nothing_is_sent_outside_the_allowed_hosts() {
        let (url, hits) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        });
        let mut req = Req::openverse_search("x", None, 1, 12);
        req.url = url;
        assert_eq!(Ureq.get(&req), Err(NetError::NotAllowed));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
