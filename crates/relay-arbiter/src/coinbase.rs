//! Coinbase's Advanced Trade API (`api.coinbase.com/api/v3/brokerage`), behind [`Market`] and
//! [`Account`].
//!
//! Market data comes from the public `/market/` endpoints even when a key is saved: they need no
//! signature, so paper trading works before any key exists and a revoked key cannot stop the
//! charts. Account calls carry an ES256 JWT per request, signed here with `ring` (no Coinbase
//! crate, no `p256`): the key is parsed once into a [`Signer`] and each JWT lives two minutes.
//!
//! The JSON → model conversions are pure functions over `serde_json::Value`, tested against the
//! shapes in Coinbase's API reference; the HTTP layer only fetches, throttles and classifies.

use crate::exchange::{Account, ExchangeError, Market, Result};
use crate::model::{Balance, Candle, ExchangeFill, ExchangeOrder, Fees, Granularity, OrderRequest, OrderStatus, Permissions, Preview, Product, Quote, Side};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BASE: &str = "https://api.coinbase.com";
/// The host as the JWT's `uri` claim names it: no scheme.
const HOST: &str = "api.coinbase.com";
/// A JWT's life. Coinbase refuses more than two minutes.
const JWT_SECONDS: i64 = 120;
/// Spacing between requests. Coinbase allows 10/s on public endpoints and 30/s on private ones,
/// per IP; staying under the lower one means a burst from the runner never earns a 429.
const SPACING: Duration = Duration::from_millis(100);
/// Pages followed for a paginated list before giving up: a cursor that never ends must not spin.
const MAX_PAGES: usize = 50;

const OID_P256: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
const OID_EC_PUBLIC_KEY: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
const OID_ED25519: &[u8] = &[0x2B, 0x65, 0x70];
const ED25519_MESSAGE: &str = "This is an Ed25519 key. Coinbase's trading API needs an ECDSA key: create a new one and choose ECDSA.";

/// A product id as Coinbase spells it, `BTC-USD`: refuses an invented or mistyped symbol before
/// any request is made. Halves are 1 to 10 characters because Coinbase lists one-letter bases
/// (`T-USD`).
pub fn product_id_ok(id: &str) -> bool {
    let half = |s: &str| (1..=10).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    matches!(id.split_once('-'), Some((b, q)) if half(b) && half(q))
}

// ---- The key ----------------------------------------------------------------------------------

/// A CDP API key as the person pasted it. `Debug` never shows the private key.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// `organizations/{org}/apiKeys/{id}`.
    pub key_name: String,
    /// Normalized PEM: real newlines, one trailing.
    pub private_key_pem: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials").field("key_name", &self.key_name).field("private_key_pem", &"<hidden>").finish()
    }
}

impl Credentials {
    /// Checks a pasted key name and private key, and that the key can sign, so a bad paste is
    /// refused at setup rather than as a 401 at the first order. The PEM may carry literal `\n`
    /// escapes and surrounding quotes, as it does when copied out of Coinbase's JSON download.
    pub fn parse(key_name: &str, pem: &str) -> std::result::Result<Credentials, String> {
        let key_name = key_name.trim().trim_matches('"').trim();
        if key_name.is_empty() {
            return Err("The key name is empty. It looks like organizations/…/apiKeys/….".into());
        }
        if key_name.chars().any(char::is_whitespace) {
            return Err("The key name has spaces in it. It looks like organizations/…/apiKeys/….".into());
        }
        let creds = Credentials { key_name: key_name.to_string(), private_key_pem: normalize_pem(pem) };
        Signer::new(&creds)?;
        Ok(creds)
    }
}

/// Turns a pasted key into PEM with real newlines: `\n` escapes, `\r`, quotes and indentation
/// are what copying it out of a JSON file or a chat leaves behind.
fn normalize_pem(pem: &str) -> String {
    let pem = pem.trim().trim_matches('"').replace("\\r", "").replace("\\n", "\n").replace('\r', "");
    let mut out: String = pem.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    out.push('\n');
    out
}

/// Each `-----BEGIN X-----` block: its label and its decoded bytes.
fn pem_blocks(pem: &str) -> std::result::Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut rest = pem;
    while let Some(i) = rest.find("-----BEGIN ") {
        let after = &rest[i + 11..];
        let label_end = after.find("-----").ok_or("The key's BEGIN line is cut short.")?;
        let label = &after[..label_end];
        let body = &after[label_end + 5..];
        let end_marker = format!("-----END {label}-----");
        let end = body.find(&end_marker).ok_or_else(|| format!("The key has no -----END {label}----- line: copy all of it."))?;
        let b64: String = body[..end].chars().filter(|c| !c.is_whitespace()).collect();
        let der = STANDARD.decode(b64.as_bytes()).map_err(|_| "The key's text is damaged: copy it again, all of it.".to_string())?;
        out.push((label.to_string(), der));
        rest = &body[end + end_marker.len()..];
    }
    Ok(out)
}

/// A minimal DER reader: enough for SEC1 and PKCS#8 headers, nothing more.
struct Der<'a>(&'a [u8]);

impl<'a> Der<'a> {
    /// The next element's tag and contents.
    fn next(&mut self) -> Option<(u8, &'a [u8])> {
        let (&tag, rest) = self.0.split_first()?;
        let (&l0, rest) = rest.split_first()?;
        let (len, rest) = if l0 < 0x80 {
            (l0 as usize, rest)
        } else {
            let n = (l0 & 0x7f) as usize;
            if n == 0 || n > 2 || rest.len() < n {
                return None;
            }
            (rest[..n].iter().fold(0usize, |a, &b| a << 8 | b as usize), &rest[n..])
        };
        if rest.len() < len {
            return None;
        }
        let (v, rest) = rest.split_at(len);
        self.0 = rest;
        Some((tag, v))
    }
    fn expect(&mut self, tag: u8) -> Option<&'a [u8]> {
        match self.next() {
            Some((t, v)) if t == tag => Some(v),
            _ => None,
        }
    }
}

/// The private scalar (32 bytes) and the uncompressed public point (65 bytes, when present) of
/// a SEC1 `ECPrivateKey`, after checking that its named curve, if given, is P-256.
fn parse_sec1(der: &[u8]) -> std::result::Result<(Vec<u8>, Option<Vec<u8>>), String> {
    const BAD: &str = "The key could not be read as an EC private key.";
    let body = Der(der).expect(0x30).ok_or(BAD)?;
    let mut d = Der(body);
    if d.expect(0x02).ok_or(BAD)? != [1] {
        return Err(BAD.into());
    }
    let private = d.expect(0x04).ok_or(BAD)?;
    if private.is_empty() || private.len() > 32 {
        return Err("The key is not a P-256 key. Coinbase needs an ECDSA P-256 key.".into());
    }
    // SEC1 fixes the length, but a short scalar padded back to 32 is still the same key.
    let mut scalar = vec![0u8; 32 - private.len()];
    scalar.extend_from_slice(private);
    let mut public = None;
    while let Some((tag, v)) = d.next() {
        match tag {
            0xA0 => {
                let oid = Der(v).expect(0x06).ok_or(BAD)?;
                if oid != OID_P256 {
                    return Err("The key is on another curve than P-256. Coinbase needs an ECDSA P-256 key.".into());
                }
            }
            0xA1 => {
                let bits = Der(v).expect(0x03).ok_or(BAD)?;
                match bits.split_first() {
                    Some((0, point)) if point.len() == 65 && point[0] == 4 => public = Some(point.to_vec()),
                    _ => return Err("The key's public half is not a P-256 point.".into()),
                }
            }
            _ => {}
        }
    }
    Ok((scalar, public))
}

/// What a PKCS#8 `PrivateKeyInfo` holds.
struct Pkcs8<'a> {
    algorithm: &'a [u8],
    curve: Option<&'a [u8]>,
    /// The algorithm's own key: SEC1 for EC.
    key: &'a [u8],
}

fn parse_pkcs8(der: &[u8]) -> Option<Pkcs8<'_>> {
    let mut d = Der(Der(der).expect(0x30)?);
    d.expect(0x02)?;
    let mut alg = Der(d.expect(0x30)?);
    let algorithm = alg.expect(0x06)?;
    let curve = alg.expect(0x06);
    Some(Pkcs8 { algorithm, curve, key: d.expect(0x04)? })
}

/// The key pair from a pasted key, or why it cannot sign for Coinbase.
fn key_pair(pem: &str, rng: &SystemRandom) -> std::result::Result<EcdsaKeyPair, String> {
    let blocks = pem_blocks(pem)?;
    if blocks.is_empty() {
        // Coinbase hands out Ed25519 secrets as bare base64 of 64 bytes, with no PEM lines.
        let bare: String = pem.chars().filter(|c| !c.is_whitespace()).collect();
        if STANDARD.decode(bare.as_bytes()).is_ok_and(|b| b.len() == 64 || b.len() == 32) {
            return Err(ED25519_MESSAGE.into());
        }
        return Err("This does not look like a private key: it should start with -----BEGIN EC PRIVATE KEY-----.".into());
    }
    // `openssl ecparam -genkey` writes an EC PARAMETERS block first.
    let Some((label, der)) = blocks.into_iter().find(|(l, _)| l != "EC PARAMETERS") else {
        return Err("This holds no private key, only curve parameters.".into());
    };
    let rejected = |e: ring::error::KeyRejected| format!("The key was refused: {e}. Copy it again, all of it.");
    match label.as_str() {
        "EC PRIVATE KEY" => {
            let (private, public) = parse_sec1(&der)?;
            let public = public.ok_or(
                "The key leaves out its public half, which signing needs here. Paste the key exactly as Coinbase gave it.",
            )?;
            EcdsaKeyPair::from_private_key_and_public_key(&ECDSA_P256_SHA256_FIXED_SIGNING, &private, &public, rng).map_err(rejected)
        }
        "PRIVATE KEY" => {
            let p8 = parse_pkcs8(&der).ok_or("The key could not be read as a private key.")?;
            if p8.algorithm == OID_ED25519 {
                return Err(ED25519_MESSAGE.into());
            }
            if p8.algorithm != OID_EC_PUBLIC_KEY || p8.curve != Some(OID_P256) {
                return Err("The key is not an ECDSA P-256 key, which Coinbase needs.".into());
            }
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, rng).or_else(|e| {
                // ring wants the public half inside; the SEC1 path says so in plain words.
                let (private, public) = parse_sec1(p8.key)?;
                let public = public.ok_or_else(|| rejected(e))?;
                EcdsaKeyPair::from_private_key_and_public_key(&ECDSA_P256_SHA256_FIXED_SIGNING, &private, &public, rng).map_err(rejected)
            })
        }
        "OPENSSH PRIVATE KEY" => Err("This is an SSH key, not a Coinbase API key.".into()),
        "ENCRYPTED PRIVATE KEY" => Err("The key is password-protected. Paste it as Coinbase gave it, unencrypted.".into()),
        other => Err(format!("This is a {other}, not a private key.")),
    }
}

/// Signs Coinbase's per-request JWTs. Built once per key: parsing and checking the key is the
/// slow part, signing is not.
pub struct Signer {
    key_name: String,
    key: EcdsaKeyPair,
    rng: SystemRandom,
}

impl fmt::Debug for Signer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signer").field("key_name", &self.key_name).finish_non_exhaustive()
    }
}

impl Signer {
    pub fn new(creds: &Credentials) -> std::result::Result<Signer, String> {
        let rng = SystemRandom::new();
        let key = key_pair(&creds.private_key_pem, &rng)?;
        Ok(Signer { key_name: creds.key_name.clone(), key, rng })
    }

    pub fn key_name(&self) -> &str {
        &self.key_name
    }

    /// The bearer token for one request. `path` may carry a query string; the `uri` claim leaves
    /// it out, as Coinbase checks it without one.
    pub fn jwt(&self, method: &str, path: &str, now: i64) -> std::result::Result<String, String> {
        let mut nonce = [0u8; 16];
        self.rng.fill(&mut nonce).map_err(|_| "The system's random source failed.".to_string())?;
        let nonce: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let path = path.split('?').next().unwrap_or(path);
        let header = json!({"alg": "ES256", "kid": self.key_name, "nonce": nonce, "typ": "JWT"});
        let claims = json!({
            "sub": self.key_name, "iss": "cdp", "nbf": now, "exp": now + JWT_SECONDS,
            "uri": format!("{} {HOST}{path}", method.to_ascii_uppercase()),
        });
        let input = format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), URL_SAFE_NO_PAD.encode(claims.to_string()));
        let sig = self.key.sign(&self.rng, input.as_bytes()).map_err(|_| "Signing the request failed.".to_string())?;
        Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref())))
    }
}

// ---- The client -------------------------------------------------------------------------------

/// When the next request may leave. Process-wide, because Coinbase counts per IP, not per
/// client value: two `Coinbase`s must not double the rate.
static NEXT_SLOT: Mutex<Option<Instant>> = Mutex::new(None);

/// Waits for this request's turn. The slot is reserved under the lock and slept outside it, so
/// one waiting caller never holds up another's reservation.
fn throttle() {
    let wait = {
        let mut slot = NEXT_SLOT.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let at = slot.map_or(now, |s| s.max(now));
        *slot = Some(at + SPACING);
        at - now
    };
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
}

pub struct Coinbase {
    agent: ureq::Agent,
    signer: Option<Signer>,
    base: String,
}

impl fmt::Debug for Coinbase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Coinbase").field("signer", &self.signer).field("base", &self.base).finish_non_exhaustive()
    }
}

impl Coinbase {
    /// Market data only; every [`Account`] call answers `Auth`.
    pub fn public() -> Coinbase {
        // Statuses are not errors here: a 400's body says why, and that reason is the message.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent("relay-arbiter")
            .build()
            .into();
        Coinbase { agent, signer: None, base: BASE.to_string() }
    }

    pub fn with_key(creds: &Credentials) -> std::result::Result<Coinbase, String> {
        Ok(Coinbase { signer: Some(Signer::new(creds)?), ..Coinbase::public() })
    }

    pub fn has_key(&self) -> bool {
        self.signer.is_some()
    }

    /// One request: `query` is percent-encoded by ureq and kept out of the JWT. Answers the body
    /// as JSON, or the status classified into an [`ExchangeError`].
    fn request(&self, method: &str, path: &str, query: &[(&str, &str)], body: Option<&Value>, signed: bool) -> Result<Value> {
        let auth = if signed {
            let signer = self.signer.as_ref().ok_or_else(|| ExchangeError::Auth("No Coinbase key is saved".into()))?;
            Some(format!("Bearer {}", signer.jwt(method, path, now()).map_err(ExchangeError::Auth)?))
        } else {
            None
        };
        throttle();
        let url = format!("{}{path}", self.base);
        let sent = match body {
            Some(body) => {
                let mut rb = self.agent.post(&url).header("accept", "application/json");
                if let Some(a) = &auth {
                    rb = rb.header("authorization", a);
                }
                rb.send_json(body)
            }
            None => {
                let mut rb = self.agent.get(&url).header("accept", "application/json").header("cache-control", "no-cache");
                for (k, v) in query {
                    rb = rb.query(k, v);
                }
                if let Some(a) = &auth {
                    rb = rb.header("authorization", a);
                }
                rb.call()
            }
        };
        let mut resp = sent.map_err(transport)?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().map_err(transport)?;
        classify(status, &text)
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        self.request("GET", path, query, None, false)
    }

    fn get_signed(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        self.request("GET", path, query, None, true)
    }

    fn post_signed(&self, path: &str, body: &Value) -> Result<Value> {
        self.request("POST", path, &[], Some(body), true)
    }
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn transport(e: ureq::Error) -> ExchangeError {
    match e {
        ureq::Error::Timeout(_) => ExchangeError::Unreachable("the request timed out".into()),
        ureq::Error::Json(e) => ExchangeError::Malformed(e.to_string()),
        e => ExchangeError::Unreachable(e.to_string()),
    }
}

/// A status and body into the answer or the error. Pure, so the mapping is tested.
fn classify(status: u16, text: &str) -> Result<Value> {
    match status {
        200..=299 if text.trim().is_empty() => Ok(json!({})),
        200..=299 => serde_json::from_str(text).map_err(|e| ExchangeError::Malformed(e.to_string())),
        401 | 403 => Err(ExchangeError::Auth(error_message(text).unwrap_or_else(|| format!("HTTP {status}")))),
        429 => Err(ExchangeError::RateLimited),
        404 => Err(ExchangeError::Refused(error_message(text).unwrap_or_else(|| "not found".into()))),
        400..=499 => Err(ExchangeError::Refused(error_message(text).unwrap_or_else(|| format!("HTTP {status}")))),
        _ => Err(ExchangeError::Unreachable(error_message(text).map_or_else(|| format!("HTTP {status}"), |m| format!("HTTP {status}: {m}")))),
    }
}

/// The reason in an error body: `message`, `error_details` or `error`, else the text itself when
/// it is short (a proxy's "Unauthorized").
fn error_message(text: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        for k in ["message", "error_details", "error"] {
            if let Some(m) = v[k].as_str().filter(|m| !m.trim().is_empty()) {
                return Some(m.trim().to_string());
            }
        }
        return None;
    }
    let t = text.trim();
    (!t.is_empty() && t.len() <= 200).then(|| t.to_string())
}

// ---- JSON → model -----------------------------------------------------------------------------

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn nonempty(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn flag(v: &Value) -> bool {
    v.as_bool().unwrap_or(false)
}

/// A decimal given as a string (or, rarely, a number). Absent or `""` is `None`.
fn opt_dec(v: &Value) -> Result<Option<Decimal>> {
    let s = match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => return Ok(None),
        other => return Err(ExchangeError::Malformed(format!("expected a number, got {other}"))),
    };
    if s.is_empty() {
        return Ok(None);
    }
    let d = if s.contains(['e', 'E']) { Decimal::from_scientific(&s) } else { Decimal::from_str(&s) };
    d.map(Some).map_err(|_| ExchangeError::Malformed(format!("not a number: {s:?}")))
}

/// A decimal where absent or `""` means zero, as Coinbase leaves unset amounts.
fn dec(v: &Value) -> Result<Decimal> {
    Ok(opt_dec(v)?.unwrap_or(Decimal::ZERO))
}

/// A float for prices that only feed charts; `"9.43%"` reads as 9.43.
fn float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches('%').trim().parse().ok(),
        _ => None,
    }
}

fn time(v: &Value) -> Option<i64> {
    v.as_str()?.parse::<jiff::Timestamp>().ok().map(|t| t.as_second())
}

fn malformed(what: &str) -> ExchangeError {
    ExchangeError::Malformed(format!("no {what} in the answer"))
}

fn parse_product(v: &Value) -> Result<Product> {
    let id = nonempty(&v["product_id"]).ok_or_else(|| malformed("product_id"))?;
    let pick = |a: &str, b: &str| nonempty(&v[a]).or_else(|| nonempty(&v[b]));
    let (b, q) = id.split_once('-').unwrap_or((&id, ""));
    let base = pick("base_currency_id", "base_display_symbol").unwrap_or_else(|| b.to_string());
    let quote = pick("quote_currency_id", "quote_display_symbol").unwrap_or_else(|| q.to_string());
    let tradable = text(&v["status"]).eq_ignore_ascii_case("online")
        && !flag(&v["trading_disabled"])
        && !flag(&v["is_disabled"])
        && !flag(&v["cancel_only"])
        && !flag(&v["view_only"]);
    Ok(Product {
        id,
        base,
        quote,
        base_increment: dec(&v["base_increment"])?,
        quote_increment: dec(&v["quote_increment"])?,
        price_increment: dec(&v["price_increment"])?,
        base_min_size: dec(&v["base_min_size"])?,
        quote_min_size: dec(&v["quote_min_size"])?,
        price: float(&v["price"]).filter(|p| *p > 0.0),
        change_24h: float(&v["price_percentage_change_24h"]),
        tradable,
        limit_only: flag(&v["limit_only"]),
    })
}

/// One page of `/market/products`, spot only, and the cursor for the next when there is one.
fn parse_products(v: &Value) -> Result<(Vec<Product>, Option<String>)> {
    let list = v["products"].as_array().ok_or_else(|| malformed("products"))?;
    let products = list
        .iter()
        .filter(|p| p["product_type"].as_str().is_none_or(|t| t == "SPOT"))
        .map(parse_product)
        .collect::<Result<Vec<_>>>()?;
    let next = flag(&v["pagination"]["has_next"]).then(|| nonempty(&v["pagination"]["next_cursor"])).flatten();
    Ok((products, next))
}

/// Candles with `start` in `[start, end)`, oldest first: Coinbase returns them newest first and
/// may include the bar at `end`.
fn parse_candles(v: &Value, start: i64, end: i64) -> Result<Vec<Candle>> {
    let list = v["candles"].as_array().ok_or_else(|| malformed("candles"))?;
    let mut out = Vec::with_capacity(list.len());
    for c in list {
        let at = match &c["start"] {
            Value::String(s) => s.trim().parse::<i64>().ok(),
            n => n.as_i64(),
        }
        .ok_or_else(|| malformed("candle start"))?;
        if at < start || at >= end {
            continue;
        }
        let f = |k: &str| float(&c[k]).ok_or_else(|| malformed(&format!("candle {k}")));
        out.push(Candle { start: at, open: f("open")?, high: f("high")?, low: f("low")?, close: f("close")?, volume: f("volume")? });
    }
    out.sort_by_key(|c| c.start);
    out.dedup_by_key(|c| c.start);
    Ok(out)
}

fn parse_quote(v: &Value, now: i64) -> Result<Quote> {
    let book = &v["pricebook"];
    let best = |side: &str| book[side].as_array().and_then(|l| l.first()).and_then(|l| float(&l["price"]));
    match (best("bids"), best("asks")) {
        (Some(bid), Some(ask)) => Ok(Quote { bid, ask, at: time(&book["time"]).unwrap_or(now) }),
        _ => {
            let id = nonempty(&book["product_id"]).unwrap_or_else(|| "this product".into());
            Err(ExchangeError::Refused(format!("the order book for {id} is empty")))
        }
    }
}

fn parse_permissions(v: &Value) -> Result<Permissions> {
    if !v.is_object() {
        return Err(malformed("permissions"));
    }
    Ok(Permissions {
        can_view: flag(&v["can_view"]),
        can_trade: flag(&v["can_trade"]),
        can_transfer: flag(&v["can_transfer"]),
        portfolio_uuid: nonempty(&v["portfolio_uuid"]),
        portfolio_type: nonempty(&v["portfolio_type"]),
    })
}

/// One page of `/accounts`, without the empty ones, and the cursor for the next.
fn parse_balances(v: &Value) -> Result<(Vec<Balance>, Option<String>)> {
    let list = v["accounts"].as_array().ok_or_else(|| malformed("accounts"))?;
    let mut out = Vec::new();
    for a in list {
        let currency = nonempty(&a["currency"]).ok_or_else(|| malformed("account currency"))?;
        let available = dec(&a["available_balance"]["value"])?;
        let hold = dec(&a["hold"]["value"])?;
        if !available.is_zero() || !hold.is_zero() {
            out.push(Balance { currency, available, hold, value: None });
        }
    }
    let next = flag(&v["has_next"]).then(|| nonempty(&v["cursor"])).flatten();
    Ok((out, next))
}

fn parse_fees(v: &Value) -> Result<Fees> {
    let tier = &v["fee_tier"];
    let rate = |k: &str| opt_dec(&tier[k])?.ok_or_else(|| malformed(&format!("fee_tier.{k}")));
    Ok(Fees { maker: rate("maker_fee_rate")?, taker: rate("taker_fee_rate")?, tier: nonempty(&tier["pricing_tier"]) })
}

/// A Coinbase reason code in words: `INSUFFICIENT_FUND` → "not enough funds". Codes not known
/// here still read, lowercased and spaced.
fn reason_words(code: &str) -> String {
    let code = code.trim();
    let bare = code.strip_prefix("PREVIEW_").unwrap_or(code);
    let known = match bare {
        "INSUFFICIENT_FUND" | "INSUFFICIENT_FUNDS" | "REJECT_REASON_INSUFFICIENT_FUNDS" => "not enough funds",
        "INVALID_SIZE_PRECISION" => "the size has more decimals than the product allows",
        "INVALID_PRICE_PRECISION" => "the price has more decimals than the product allows",
        "INVALID_LIMIT_PRICE_POST_ONLY" => "a post-only limit at this price would have filled at once",
        "INVALID_LIMIT_PRICE" => "the limit price is not allowed",
        "INVALID_PRODUCT_ID" => "no such product",
        "ORDER_ENTRY_DISABLED" => "new orders are turned off for this product",
        "INELIGIBLE_PAIR" => "this account may not trade this product",
        "UNSUPPORTED_ORDER_CONFIGURATION" | "INVALID_ORDER_CONFIG" => "the exchange does not take this kind of order",
        "INVALID_SIDE" => "the side is not buy or sell",
        "ORDER_SIZE_TOO_SMALL" | "INVALID_ORDER_SIZE_TOO_SMALL" => "the order is below the product's minimum",
        "ORDER_SIZE_TOO_LARGE" | "INVALID_ORDER_SIZE_TOO_LARGE" => "the order is above the product's maximum",
        "TOO_MANY_OPEN_ORDERS" => "too many open orders",
        "HOLD_FAILURE" => "the funds could not be held",
        "RATE_LIMIT_EXCEEDED" => "too many requests",
        "DUPLICATE_CLIENT_ORDER_ID" => "duplicate client order id",
        "BIG_ORDER" => "a large order for this product",
        "SMALL_ORDER" => "a small order for this product",
        _ => "",
    };
    if known.is_empty() { bare.to_ascii_lowercase().replace('_', " ") } else { known.to_string() }
}

/// An `errs` or `warning` entry: a code string, or an object carrying one.
fn issue(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => nonempty(v).map(|_| reason_words(s)),
        Value::Object(_) => ["message", "error_details", "preview_failure_reason", "error"]
            .iter()
            .find_map(|k| nonempty(&v[*k]))
            .map(|s| if s.contains(' ') { s } else { reason_words(&s) }),
        _ => None,
    }
}

fn parse_preview(v: &Value) -> Result<Preview> {
    if !v.is_object() {
        return Err(malformed("preview"));
    }
    let list = |k: &str| v[k].as_array().map(|l| l.iter().filter_map(issue).collect::<Vec<_>>()).unwrap_or_default();
    Ok(Preview {
        preview_id: nonempty(&v["preview_id"]),
        commission: opt_dec(&v["commission_total"])?,
        quote_size: opt_dec(&v["quote_size"])?,
        base_size: opt_dec(&v["base_size"])?,
        errors: list("errs"),
        // `UNKNOWN` is the enum's default, not a warning.
        warnings: v["warning"]
            .as_array()
            .map(|l| l.iter().filter(|w| w.as_str() != Some("UNKNOWN")).filter_map(issue).collect())
            .unwrap_or_default(),
    })
}

fn amount(d: Decimal) -> String {
    d.normalize().to_string()
}

/// The order's body for preview and create: product, side and `order_configuration`. A buy is
/// sized by what it spends when it says so; a sell by what it gives.
fn order_body(o: &OrderRequest) -> Result<Value> {
    let config = match o.limit_price {
        Some(price) => {
            let base = o.base_size.ok_or_else(|| ExchangeError::Refused("a limit order needs a base size".into()))?;
            json!({"limit_limit_gtc": {"base_size": amount(base), "limit_price": amount(price), "post_only": true}})
        }
        None => {
            let (first, second) = match o.side {
                Side::Buy => ((o.quote_size, "quote_size"), (o.base_size, "base_size")),
                Side::Sell => ((o.base_size, "base_size"), (o.quote_size, "quote_size")),
            };
            let (size, key) = match (first, second) {
                ((Some(d), k), _) | (_, (Some(d), k)) => (d, k),
                _ => return Err(ExchangeError::Refused("the order has no size".into())),
            };
            json!({"market_market_ioc": {key: amount(size)}})
        }
    };
    Ok(json!({"product_id": o.product, "side": o.side.upper(), "order_configuration": config}))
}

fn place_body(o: &OrderRequest, preview_id: Option<&str>) -> Result<Value> {
    let mut body = order_body(o)?;
    body["client_order_id"] = json!(o.client_order_id);
    if let Some(p) = preview_id {
        body["preview_id"] = json!(p);
    }
    Ok(body)
}

/// The answer to create. `success` only means accepted: the order comes back `Pending` and its
/// state is read with [`Account::order`]. A repeated `client_order_id` is answered by Coinbase
/// with the order already placed, so it lands here as a success too, or with
/// `DUPLICATE_CLIENT_ORDER_ID`, which is [`ExchangeError::Duplicate`]: the order exists.
fn parse_place(v: &Value, o: &OrderRequest) -> Result<ExchangeOrder> {
    if flag(&v["success"]) {
        let ok = &v["success_response"];
        let order_id = nonempty(&ok["order_id"]).or_else(|| nonempty(&v["order_id"])).ok_or_else(|| malformed("order_id"))?;
        return Ok(ExchangeOrder {
            order_id,
            client_order_id: nonempty(&ok["client_order_id"]).unwrap_or_else(|| o.client_order_id.clone()),
            status: OrderStatus::Pending,
            filled_size: Decimal::ZERO,
            average_filled_price: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            reason: None,
        });
    }
    let err = &v["error_response"];
    let code = ["new_order_failure_reason", "error", "preview_failure_reason"]
        .iter()
        .filter_map(|k| nonempty(&err[*k]))
        .chain(nonempty(&v["failure_reason"]))
        .find(|c| !c.starts_with("UNKNOWN_"));
    let detail = nonempty(&err["error_details"]).or_else(|| nonempty(&err["message"]));
    let duplicate = code.as_deref() == Some("DUPLICATE_CLIENT_ORDER_ID")
        || detail.as_deref().is_some_and(|d| d.to_ascii_lowercase().contains("duplicate"));
    if duplicate {
        return Err(ExchangeError::Duplicate(format!("an order with client order id {} was already placed", o.client_order_id)));
    }
    let reason = match (code, detail) {
        (Some(c), Some(d)) => format!("{} ({d})", reason_words(&c)),
        (Some(c), None) => reason_words(&c),
        (None, Some(d)) => d,
        (None, None) => "the order was not accepted, and no reason was given".into(),
    };
    Err(ExchangeError::Refused(reason))
}

fn order_status(s: &str) -> OrderStatus {
    match s {
        "OPEN" | "CANCEL_QUEUED" | "EDIT_QUEUED" => OrderStatus::Open,
        "FILLED" => OrderStatus::Filled,
        "CANCELLED" => OrderStatus::Cancelled,
        "EXPIRED" => OrderStatus::Expired,
        "FAILED" => OrderStatus::Failed,
        // PENDING, QUEUED, UNKNOWN_ORDER_STATUS: not known yet, ask again.
        _ => OrderStatus::Pending,
    }
}

/// Get Order's answer, which wraps the order in `order`.
fn parse_order(v: &Value) -> Result<ExchangeOrder> {
    order_fields(&v["order"])
}

/// One order object, as Get Order wraps it and List Orders lists it.
fn order_fields(o: &Value) -> Result<ExchangeOrder> {
    let order_id = nonempty(&o["order_id"]).ok_or_else(|| malformed("order"))?;
    let reason = nonempty(&o["reject_message"])
        .or_else(|| nonempty(&o["reject_reason"]).filter(|r| r != "REJECT_REASON_UNSPECIFIED").map(|r| reason_words(&r)))
        .or_else(|| nonempty(&o["cancel_message"]));
    Ok(ExchangeOrder {
        order_id,
        client_order_id: nonempty(&o["client_order_id"]).unwrap_or_default(),
        status: order_status(text(&o["status"])),
        filled_size: dec(&o["filled_size"])?,
        average_filled_price: dec(&o["average_filled_price"])?,
        total_fees: dec(&o["total_fees"])?,
        reason,
    })
}

/// One page of List Orders: the order placed with `client_order_id`, when this page has it, and
/// the cursor for the next page while `has_next` says there is one. Only the order asked for is
/// read, so another order this client cannot read does not stop the lookup.
fn find_in_orders(v: &Value, client_order_id: &str) -> Result<(Option<ExchangeOrder>, Option<String>)> {
    let list = v["orders"].as_array().ok_or_else(|| malformed("orders"))?;
    let found = list.iter().find(|o| text(&o["client_order_id"]) == client_order_id).map(order_fields).transpose()?;
    // More pages with no cursor to them: not reading them could miss the order.
    let next = if flag(&v["has_next"]) { Some(nonempty(&v["cursor"]).ok_or_else(|| malformed("cursor"))?) } else { None };
    Ok((found, next))
}

/// One page of fills and the cursor for the next. `entry_id` is the fill's own id; `trade_id`
/// repeats across adjusted fills, so it is only the fallback.
fn parse_fills(v: &Value) -> Result<(Vec<ExchangeFill>, Option<String>)> {
    let list = v["fills"].as_array().ok_or_else(|| malformed("fills"))?;
    let mut out = Vec::with_capacity(list.len());
    for f in list {
        let trade_id = nonempty(&f["entry_id"]).or_else(|| nonempty(&f["trade_id"])).ok_or_else(|| malformed("fill id"))?;
        let price = dec(&f["price"])?;
        let mut size = dec(&f["size"])?;
        // An order placed in quote reports its fills' size in quote.
        if flag(&f["size_in_quote"]) && !price.is_zero() {
            size = (size / price).normalize();
        }
        out.push(ExchangeFill {
            trade_id,
            order_id: nonempty(&f["order_id"]).ok_or_else(|| malformed("fill order_id"))?,
            price,
            size,
            fee: dec(&f["commission"])?,
            at: time(&f["trade_time"]).or_else(|| time(&f["sequence_timestamp"])).ok_or_else(|| malformed("fill trade_time"))?,
        });
    }
    Ok((out, nonempty(&v["cursor"])))
}

/// Cancel's per-order results. An order already filled, or already being cancelled, is done
/// either way; anything else is a refusal the caller must see.
fn parse_cancel(v: &Value) -> Result<()> {
    let results = v["results"].as_array().ok_or_else(|| malformed("results"))?;
    let refused: Vec<String> = results
        .iter()
        .filter(|r| !flag(&r["success"]))
        .filter(|r| !matches!(text(&r["failure_reason"]), "ORDER_IS_FULLY_FILLED" | "DUPLICATE_CANCEL_REQUEST"))
        .map(|r| format!("{}: {}", text(&r["order_id"]), reason_words(text(&r["failure_reason"]))))
        .collect();
    if refused.is_empty() { Ok(()) } else { Err(ExchangeError::Refused(format!("could not cancel {}", refused.join(", ")))) }
}

fn check_id(id: &str) -> Result<()> {
    if product_id_ok(id) { Ok(()) } else { Err(ExchangeError::Refused(format!("{id:?} is not a product id like BTC-USD"))) }
}

// ---- The traits -------------------------------------------------------------------------------

const PRODUCTS: &str = "/api/v3/brokerage/market/products";
const ORDERS: &str = "/api/v3/brokerage/orders/historical";

impl Market for Coinbase {
    fn products(&self) -> Result<Vec<Product>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut q = vec![("product_type", "SPOT")];
            if let Some(c) = &cursor {
                q.push(("cursor", c));
            }
            let (page, next) = parse_products(&self.get(PRODUCTS, &q)?)?;
            out.extend(page);
            match next {
                Some(n) if cursor.as_ref() != Some(&n) => cursor = Some(n),
                _ => break,
            }
        }
        Ok(out)
    }

    fn product(&self, id: &str) -> Result<Product> {
        check_id(id)?;
        parse_product(&self.get(&format!("{PRODUCTS}/{id}"), &[])?)
    }

    fn candles(&self, product: &str, granularity: Granularity, start: i64, end: i64) -> Result<Vec<Candle>> {
        check_id(product)?;
        if end <= start {
            return Ok(Vec::new());
        }
        let (s, e) = (start.to_string(), end.to_string());
        let q = [("start", s.as_str()), ("end", e.as_str()), ("granularity", granularity.as_str()), ("limit", "350")];
        parse_candles(&self.get(&format!("{PRODUCTS}/{product}/candles"), &q)?, start, end)
    }

    fn quote(&self, product: &str) -> Result<Quote> {
        check_id(product)?;
        parse_quote(&self.get("/api/v3/brokerage/market/product_book", &[("product_id", product), ("limit", "1")])?, now())
    }
}

impl Account for Coinbase {
    fn permissions(&self) -> Result<Permissions> {
        parse_permissions(&self.get_signed("/api/v3/brokerage/key_permissions", &[])?)
    }

    fn balances(&self) -> Result<Vec<Balance>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut q = vec![("limit", "250")];
            if let Some(c) = &cursor {
                q.push(("cursor", c));
            }
            let (page, next) = parse_balances(&self.get_signed("/api/v3/brokerage/accounts", &q)?)?;
            out.extend(page);
            match next {
                Some(n) if cursor.as_ref() != Some(&n) => cursor = Some(n),
                _ => break,
            }
        }
        Ok(out)
    }

    fn fees(&self) -> Result<Fees> {
        parse_fees(&self.get_signed("/api/v3/brokerage/transaction_summary", &[])?)
    }

    fn preview(&self, order: &OrderRequest) -> Result<Preview> {
        check_id(&order.product)?;
        parse_preview(&self.post_signed("/api/v3/brokerage/orders/preview", &order_body(order)?)?)
    }

    fn place(&self, order: &OrderRequest, preview_id: Option<&str>) -> Result<ExchangeOrder> {
        check_id(&order.product)?;
        parse_place(&self.post_signed("/api/v3/brokerage/orders", &place_body(order, preview_id)?)?, order)
    }

    fn order(&self, order_id: &str) -> Result<ExchangeOrder> {
        if order_id.is_empty() || !order_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(ExchangeError::Refused(format!("{order_id:?} is not an order id")));
        }
        parse_order(&self.get_signed(&format!("{ORDERS}/{order_id}"), &[])?)
    }

    /// List Orders for the one product since `since`, every status, page by page until the order
    /// turns up or the list ends. A list that does not end is an error, not "no such order".
    fn find_order(&self, product: &str, client_order_id: &str, since: i64) -> Result<Option<ExchangeOrder>> {
        check_id(product)?;
        let start = jiff::Timestamp::from_second(since).map_err(|e| ExchangeError::Refused(e.to_string()))?.to_string();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut q = vec![("product_ids", product), ("start_date", start.as_str()), ("limit", "100")];
            if let Some(c) = &cursor {
                q.push(("cursor", c));
            }
            let (found, next) = find_in_orders(&self.get_signed(&format!("{ORDERS}/batch"), &q)?, client_order_id)?;
            match (found, next) {
                (Some(o), _) => return Ok(Some(o)),
                (None, None) => return Ok(None),
                (None, Some(n)) if cursor.as_ref() != Some(&n) => cursor = Some(n),
                (None, Some(_)) => return Err(ExchangeError::Malformed("the order list gave the same cursor twice".into())),
            }
        }
        Err(ExchangeError::Malformed(format!("the order list ran past {MAX_PAGES} pages")))
    }

    fn fills(&self, order_id: &str) -> Result<Vec<ExchangeFill>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut q = vec![("order_ids", order_id), ("limit", "250")];
            if let Some(c) = &cursor {
                q.push(("cursor", c));
            }
            let (page, next) = parse_fills(&self.get_signed(&format!("{ORDERS}/fills"), &q)?)?;
            let empty = page.is_empty();
            out.extend(page);
            match next {
                Some(n) if !empty && cursor.as_ref() != Some(&n) => cursor = Some(n),
                _ => break,
            }
        }
        Ok(out)
    }

    fn cancel(&self, order_ids: &[String]) -> Result<()> {
        if order_ids.is_empty() {
            return Ok(());
        }
        parse_cancel(&self.post_signed("/api/v3/brokerage/orders/batch_cancel", &json!({"order_ids": order_ids}))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{ECDSA_P256_SHA256_FIXED, KeyPair, UnparsedPublicKey};

    /// A throwaway P-256 key made for these tests with `openssl ecparam -name prime256v1 -genkey
    /// -noout`. It has never been near an exchange.
    const SEC1: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEICr3hFJQLruuvjzXzQvrZyWUlBfa0SdE9HtDCMhP7NYqoAoGCCqGSM49
AwEHoUQDQgAEIgIeCM0SMcTdKeIUVa5s6GrENsIc3sFPBcefipt5Ciw6PtUG6nMV
TsGghZLrhr8kSV5nZX8i1b+Q+3+QzuY0wQ==
-----END EC PRIVATE KEY-----
";
    /// The same key as PKCS#8 (`openssl pkcs8 -topk8 -nocrypt`).
    const PKCS8: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgKveEUlAuu66+PNfN
C+tnJZSUF9rRJ0T0e0MIyE/s1iqhRANCAAQiAh4IzRIxxN0p4hRVrmzoasQ2whze
wU8Fx5+Km3kKLDo+1QbqcxVOwaCFkuuGvyRJXmdlfyLVv5D7f5DO5jTB
-----END PRIVATE KEY-----
";
    /// The same key with its public half left out (`openssl ec -no_public`).
    const SEC1_NO_PUBLIC: &str = "-----BEGIN EC PRIVATE KEY-----
MDECAQEEICr3hFJQLruuvjzXzQvrZyWUlBfa0SdE9HtDCMhP7NYqoAoGCCqGSM49
AwEH
-----END EC PRIVATE KEY-----
";
    /// `openssl genpkey -algorithm ed25519`.
    const ED25519_PEM: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEINjXFvuxDstdwgWaOYAI7I3r1qbmuoq+7SNE9GeEB1fD
-----END PRIVATE KEY-----
";
    const NAME: &str = "organizations/0b1c/apiKeys/9f2e";

    fn b64json(s: &str) -> Value {
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(s).unwrap()).unwrap()
    }

    /// Splits a JWT, checks its signature against `public`, and returns header and claims.
    fn verify(jwt: &str, public: &[u8]) -> (Value, Value) {
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        assert_eq!(sig.len(), 64, "ES256 is r||s, 64 bytes");
        let input = format!("{}.{}", parts[0], parts[1]);
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public).verify(input.as_bytes(), &sig).expect("signature verifies");
        (b64json(parts[0]), b64json(parts[1]))
    }

    #[test]
    fn signs_a_jwt_with_a_generated_pkcs8_key() {
        let rng = SystemRandom::new();
        let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let pem = format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", STANDARD.encode(doc.as_ref()));
        let public = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, doc.as_ref(), &rng).unwrap().public_key().as_ref().to_vec();
        let creds = Credentials::parse(NAME, &pem).unwrap();
        let signer = Signer::new(&creds).unwrap();
        let jwt = signer.jwt("get", "/api/v3/brokerage/accounts?limit=250&cursor=abc", 1_700_000_000).unwrap();
        let (header, claims) = verify(&jwt, &public);
        assert_eq!(header["alg"], "ES256");
        assert_eq!(header["typ"], "JWT");
        assert_eq!(header["kid"], NAME);
        let nonce = header["nonce"].as_str().unwrap();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(claims["sub"], NAME);
        assert_eq!(claims["iss"], "cdp");
        assert_eq!(claims["nbf"], 1_700_000_000);
        assert_eq!(claims["exp"], 1_700_000_120);
        assert_eq!(claims["uri"], "GET api.coinbase.com/api/v3/brokerage/accounts");
        // Two tokens never share a nonce.
        let again = signer.jwt("GET", "/x", 1).unwrap();
        assert_ne!(b64json(again.split('.').next().unwrap())["nonce"], header["nonce"]);
    }

    #[test]
    fn reads_a_sec1_key_and_signs() {
        let der = &pem_blocks(SEC1).unwrap()[0].1;
        let (private, public) = parse_sec1(der).unwrap();
        assert_eq!(private[..4], [0x2A, 0xF7, 0x84, 0x52]);
        let public = public.unwrap();
        assert_eq!((public.len(), public[0]), (65, 4));
        let creds = Credentials::parse(NAME, SEC1).unwrap();
        let jwt = Signer::new(&creds).unwrap().jwt("POST", "/api/v3/brokerage/orders", 5).unwrap();
        let (_, claims) = verify(&jwt, &public);
        assert_eq!(claims["uri"], "POST api.coinbase.com/api/v3/brokerage/orders");
        // The PKCS#8 form of the same key signs for the same public key.
        let jwt = Signer::new(&Credentials::parse(NAME, PKCS8).unwrap()).unwrap().jwt("GET", "/a", 5).unwrap();
        verify(&jwt, &public);
    }

    #[test]
    fn takes_a_key_pasted_out_of_json() {
        let escaped = format!("\"{}\"", SEC1.trim().replace('\n', "\\n"));
        let creds = Credentials::parse("  organizations/0b1c/apiKeys/9f2e\n", &escaped).unwrap();
        assert_eq!(creds.key_name, NAME);
        assert_eq!(creds.private_key_pem, SEC1);
        let crlf = SEC1.replace('\n', "\r\n");
        assert_eq!(Credentials::parse(NAME, &crlf).unwrap().private_key_pem, SEC1);
        let debug = format!("{creds:?}");
        assert!(debug.contains(NAME) && !debug.contains("MHcCAQEE"), "{debug}");
    }

    #[test]
    fn refuses_keys_that_cannot_sign_for_coinbase() {
        let err = Credentials::parse(NAME, ED25519_PEM).unwrap_err();
        assert_eq!(err, ED25519_MESSAGE);
        // Coinbase's own Ed25519 secret: bare base64 of 64 bytes.
        let bare = STANDARD.encode([7u8; 64]);
        assert_eq!(Credentials::parse(NAME, &bare).unwrap_err(), ED25519_MESSAGE);
        assert!(Credentials::parse(NAME, SEC1_NO_PUBLIC).unwrap_err().contains("public half"));
        assert!(Credentials::parse("", SEC1).unwrap_err().contains("empty"));
        assert!(Credentials::parse(NAME, "hello").unwrap_err().contains("BEGIN EC PRIVATE KEY"));
        assert!(Credentials::parse(NAME, &SEC1.replace("-----END EC PRIVATE KEY-----", "")).unwrap_err().contains("END"));
        // Another named curve (1.2.840.10045.3.1.4) in place of P-256.
        let mut der = pem_blocks(SEC1).unwrap().remove(0).1;
        let at = der.windows(OID_P256.len()).position(|w| w == OID_P256).unwrap();
        der[at + OID_P256.len() - 1] = 0x04;
        assert!(parse_sec1(&der).unwrap_err().contains("P-256"));
    }

    #[test]
    fn product_ids() {
        for ok in ["BTC-USD", "ETH-CAD", "1INCH-USD", "T-USD", "USDC-EUR"] {
            assert!(product_id_ok(ok), "{ok}");
        }
        for bad in ["", "BTC", "btc-usd", "BTC-USD-X", "BTC_USD", "-USD", "BTC-", "ABCDEFGHIJK-USD", "BTC-US D"] {
            assert!(!product_id_ok(bad), "{bad}");
        }
        assert!(matches!(check_id("../x"), Err(ExchangeError::Refused(_))));
    }

    #[test]
    fn statuses_become_errors() {
        assert_eq!(classify(200, r#"{"a":1}"#).unwrap()["a"], 1);
        assert_eq!(classify(200, "").unwrap(), json!({}));
        assert!(matches!(classify(200, "<html>"), Err(ExchangeError::Malformed(_))));
        assert_eq!(classify(401, "Unauthorized\n"), Err(ExchangeError::Auth("Unauthorized".into())));
        let body = r#"{"error":"PERMISSION_DENIED","error_details":"Missing required scopes","message":"Missing required scopes"}"#;
        assert_eq!(classify(403, body), Err(ExchangeError::Auth("Missing required scopes".into())));
        assert_eq!(classify(429, ""), Err(ExchangeError::RateLimited));
        let body = r#"{"error":"INVALID_ARGUMENT","error_details":"start and end must be within 350 candles","message":""}"#;
        assert_eq!(classify(400, body), Err(ExchangeError::Refused("start and end must be within 350 candles".into())));
        assert_eq!(classify(404, r#"{"error":"NOT_FOUND"}"#), Err(ExchangeError::Refused("NOT_FOUND".into())));
        assert_eq!(classify(503, ""), Err(ExchangeError::Unreachable("HTTP 503".into())));
    }

    #[test]
    fn reads_products() {
        let v = json!({"products": [
            {"product_id": "BTC-USD", "price": "62123.45", "price_percentage_change_24h": "-1.25", "volume_24h": "1908432",
             "base_increment": "0.00000001", "quote_increment": "0.01", "quote_min_size": "1", "quote_max_size": "150000000",
             "base_min_size": "0.00000001", "base_max_size": "3400", "base_name": "Bitcoin", "quote_name": "US Dollar",
             "is_disabled": false, "status": "online", "cancel_only": false, "limit_only": false, "post_only": false,
             "trading_disabled": false, "product_type": "SPOT", "quote_currency_id": "USD", "base_currency_id": "BTC",
             "base_display_symbol": "BTC", "quote_display_symbol": "USD", "view_only": false, "price_increment": "0.01"},
            {"product_id": "XYZ-USD", "price": "", "price_percentage_change_24h": "", "base_increment": "", "quote_increment": "0.0001",
             "quote_min_size": "", "base_min_size": "", "status": "delisted", "trading_disabled": true, "limit_only": true,
             "product_type": "SPOT", "quote_currency_id": "", "base_currency_id": "", "base_display_symbol": "XYZ",
             "quote_display_symbol": "USD", "price_increment": "0.0001"},
            {"product_id": "BIT-31OCT26-CDE", "product_type": "FUTURE", "status": "online"}
        ], "num_products": 3, "pagination": {"has_next": true, "next_cursor": "QlRDLVVTRA=="}});
        let (products, next) = parse_products(&v).unwrap();
        assert_eq!(products.len(), 2, "futures are left out");
        assert_eq!(next.as_deref(), Some("QlRDLVVTRA=="));
        let btc = &products[0];
        assert_eq!((btc.id.as_str(), btc.base.as_str(), btc.quote.as_str()), ("BTC-USD", "BTC", "USD"));
        assert_eq!(btc.base_increment, Decimal::new(1, 8));
        assert_eq!(btc.quote_min_size, Decimal::ONE);
        assert_eq!((btc.price, btc.change_24h), (Some(62123.45), Some(-1.25)));
        assert!(btc.tradable && !btc.limit_only);
        let xyz = &products[1];
        assert_eq!((xyz.base.as_str(), xyz.quote.as_str()), ("XYZ", "USD"));
        assert_eq!((xyz.price, xyz.base_increment, xyz.base_min_size), (None, Decimal::ZERO, Decimal::ZERO));
        assert!(!xyz.tradable && xyz.limit_only);
        // A single product is the same object, unwrapped.
        assert_eq!(parse_product(&v["products"][0]).unwrap(), *btc);
        let cancel_only = json!({"product_id": "A-B", "status": "online", "cancel_only": true});
        assert!(!parse_product(&cancel_only).unwrap().tradable);
        assert_eq!(parse_products(&json!({"products": []})).unwrap(), (vec![], None));
    }

    #[test]
    fn reads_candles_oldest_first_within_the_range() {
        let v = json!({"candles": [
            {"start": "1700010800", "low": "100.5", "high": "103", "open": "101", "close": "102.5", "volume": "12.5"},
            {"start": "1700007200", "low": "99", "high": "101.5", "open": "100", "close": "101", "volume": "8"},
            {"start": "1700003600", "low": "98", "high": "100.5", "open": "99", "close": "100", "volume": "7.25"}
        ]});
        let c = parse_candles(&v, 1_700_003_600, 1_700_010_800).unwrap();
        assert_eq!(c.iter().map(|c| c.start).collect::<Vec<_>>(), [1_700_003_600, 1_700_007_200]);
        assert_eq!(c[1], Candle { start: 1_700_007_200, open: 100.0, high: 101.5, low: 99.0, close: 101.0, volume: 8.0 });
        assert!(parse_candles(&json!({"candles": []}), 0, 10).unwrap().is_empty());
        assert!(matches!(parse_candles(&json!({}), 0, 10), Err(ExchangeError::Malformed(_))));
    }

    #[test]
    fn reads_the_best_prices() {
        let v = json!({"pricebook": {"product_id": "BTC-USD",
            "bids": [{"price": "62100.01", "size": "0.5"}], "asks": [{"price": "62100.55", "size": "0.25"}],
            "time": "2026-10-09T14:00:05.123456Z"}, "last": "62100.30", "mid_market": "62100.28", "spread_bps": "0.87"});
        let q = parse_quote(&v, 1).unwrap();
        assert_eq!((q.bid, q.ask), (62100.01, 62100.55));
        assert_eq!(q.at, "2026-10-09T14:00:05Z".parse::<jiff::Timestamp>().unwrap().as_second());
        let no_time = json!({"pricebook": {"product_id": "BTC-USD", "bids": [{"price": "1"}], "asks": [{"price": "2"}]}});
        assert_eq!(parse_quote(&no_time, 42).unwrap().at, 42);
        let empty = json!({"pricebook": {"product_id": "XYZ-USD", "bids": [], "asks": []}});
        assert_eq!(parse_quote(&empty, 1), Err(ExchangeError::Refused("the order book for XYZ-USD is empty".into())));
    }

    #[test]
    fn reads_permissions() {
        let v = json!({"can_view": true, "can_trade": true, "can_transfer": false,
            "portfolio_uuid": "b87a2d3f-8a1e-49b3-a4ea-402d8c389aca", "portfolio_type": "DEFAULT"});
        let p = parse_permissions(&v).unwrap();
        assert!(p.can_view && p.can_trade && !p.can_transfer);
        assert_eq!(p.portfolio_uuid.as_deref(), Some("b87a2d3f-8a1e-49b3-a4ea-402d8c389aca"));
        assert_eq!(p.portfolio_type.as_deref(), Some("DEFAULT"));
    }

    #[test]
    fn reads_a_page_of_balances() {
        let v = json!({"accounts": [
            {"uuid": "8bfc20d7-f7c6-4422-bf07-8243ca4169fe", "name": "BTC Wallet", "currency": "BTC",
             "available_balance": {"value": "0.01234567", "currency": "BTC"}, "default": false, "active": true,
             "type": "ACCOUNT_TYPE_CRYPTO", "ready": true, "hold": {"value": "0.001", "currency": "BTC"}},
            {"uuid": "1", "currency": "ETH", "available_balance": {"value": "0", "currency": "ETH"}, "hold": {"value": "0", "currency": "ETH"}},
            {"uuid": "2", "currency": "CAD", "available_balance": {"value": "250.00", "currency": "CAD"}, "hold": {"value": "", "currency": "CAD"}}
        ], "has_next": true, "cursor": "789100", "size": 3});
        let (b, next) = parse_balances(&v).unwrap();
        assert_eq!(next.as_deref(), Some("789100"));
        assert_eq!(b.len(), 2, "the empty ETH account is skipped");
        assert_eq!(b[0], Balance { currency: "BTC".into(), available: Decimal::new(1234567, 8), hold: Decimal::new(1, 3), value: None });
        assert_eq!((b[1].currency.as_str(), b[1].available, b[1].hold), ("CAD", Decimal::new(25000, 2), Decimal::ZERO));
        let last = json!({"accounts": [], "has_next": false, "cursor": ""});
        assert_eq!(parse_balances(&last).unwrap(), (vec![], None));
    }

    #[test]
    fn reads_fees() {
        let v = json!({"total_volume": 1000, "total_fees": 25, "fee_tier": {"pricing_tier": "Advanced 1",
            "usd_from": "0", "usd_to": "10000", "taker_fee_rate": "0.012", "maker_fee_rate": "0.006",
            "aop_from": "", "aop_to": ""}, "margin_rate": null, "advanced_trade_only_volume": 1000});
        let f = parse_fees(&v).unwrap();
        assert_eq!((f.maker, f.taker), (Decimal::new(6, 3), Decimal::new(12, 3)));
        assert_eq!(f.tier.as_deref(), Some("Advanced 1"));
        assert!(matches!(parse_fees(&json!({"fee_tier": {}})), Err(ExchangeError::Malformed(_))));
    }

    fn market_buy() -> OrderRequest {
        OrderRequest {
            client_order_id: "arb-7-1700003600".into(),
            product: "BTC-USD".into(),
            side: Side::Buy,
            quote_size: Some(Decimal::new(5000, 2)),
            base_size: None,
            limit_price: None,
        }
    }

    #[test]
    fn builds_order_bodies() {
        let body = place_body(&market_buy(), Some("b40bbff9")).unwrap();
        assert_eq!(body, json!({"client_order_id": "arb-7-1700003600", "product_id": "BTC-USD", "side": "BUY",
            "order_configuration": {"market_market_ioc": {"quote_size": "50"}}, "preview_id": "b40bbff9"}));
        let sell = OrderRequest { side: Side::Sell, quote_size: None, base_size: Some(Decimal::new(1_2300, 8)), ..market_buy() };
        assert_eq!(order_body(&sell).unwrap()["order_configuration"], json!({"market_market_ioc": {"base_size": "0.000123"}}));
        let limit = OrderRequest { base_size: Some(Decimal::new(1000, 6)), limit_price: Some(Decimal::new(6210000, 2)), ..market_buy() };
        let body = order_body(&limit).unwrap();
        assert_eq!(body["order_configuration"], json!({"limit_limit_gtc": {"base_size": "0.001", "limit_price": "62100", "post_only": true}}));
        assert!(body.get("client_order_id").is_none(), "preview takes no client_order_id");
        let no_size = OrderRequest { quote_size: None, ..market_buy() };
        assert!(matches!(order_body(&no_size), Err(ExchangeError::Refused(_))));
        let limit_by_quote = OrderRequest { limit_price: Some(Decimal::ONE), ..market_buy() };
        assert!(matches!(order_body(&limit_by_quote), Err(ExchangeError::Refused(_))));
    }

    #[test]
    fn reads_a_preview() {
        let v = json!({"order_total": "50.30", "commission_total": "0.30", "errs": [], "warning": ["UNKNOWN"],
            "quote_size": "50", "base_size": "0.00080483", "best_bid": "62100.01", "best_ask": "62100.55", "is_max": false,
            "preview_id": "b40bbff9-17ce-4726-8b64-9de7ae57ad26", "slippage": "0"});
        let p = parse_preview(&v).unwrap();
        assert_eq!(p.preview_id.as_deref(), Some("b40bbff9-17ce-4726-8b64-9de7ae57ad26"));
        assert_eq!((p.commission, p.quote_size), (Some(Decimal::new(30, 2)), Some(Decimal::new(50, 0))));
        assert_eq!(p.base_size, Some(Decimal::new(80483, 8)));
        assert!(p.errors.is_empty() && p.warnings.is_empty());
        let refused = json!({"commission_total": "", "errs": ["PREVIEW_INSUFFICIENT_FUND", {"message": "Order too small"}, "PREVIEW_SOMETHING_NEW"],
            "warning": ["BIG_ORDER"], "quote_size": "", "base_size": ""});
        let p = parse_preview(&refused).unwrap();
        assert_eq!(p.errors, ["not enough funds", "Order too small", "something new"]);
        assert_eq!(p.warnings, ["a large order for this product"]);
        assert_eq!((p.preview_id, p.commission, p.base_size), (None, None, None));
    }

    #[test]
    fn reads_create_answers() {
        let ok = json!({"success": true, "success_response": {"order_id": "11111-00000-000000", "product_id": "BTC-USD",
            "side": "BUY", "client_order_id": "arb-7-1700003600"},
            "order_configuration": {"market_market_ioc": {"quote_size": "50"}}});
        let o = parse_place(&ok, &market_buy()).unwrap();
        assert_eq!((o.order_id.as_str(), o.client_order_id.as_str(), o.status), ("11111-00000-000000", "arb-7-1700003600", OrderStatus::Pending));
        let funds = json!({"success": false, "error_response": {"error": "INSUFFICIENT_FUND", "message": "Insufficient balance in source account",
            "error_details": "", "preview_failure_reason": "UNKNOWN_PREVIEW_FAILURE_REASON", "new_order_failure_reason": "INSUFFICIENT_FUND"}});
        assert_eq!(parse_place(&funds, &market_buy()), Err(ExchangeError::Refused("not enough funds (Insufficient balance in source account)".into())));
        let dup = json!({"success": false, "error_response": {"error": "UNKNOWN_FAILURE_REASON", "new_order_failure_reason": "DUPLICATE_CLIENT_ORDER_ID"}});
        let Err(ExchangeError::Duplicate(m)) = parse_place(&dup, &market_buy()) else { panic!() };
        assert!(m.contains("already placed") && m.contains("arb-7-1700003600"), "{m}");
        let bare = json!({"success": false, "failure_reason": "UNKNOWN_FAILURE_REASON", "error_response": {}});
        assert!(matches!(parse_place(&bare, &market_buy()), Err(ExchangeError::Refused(_))));
    }

    #[test]
    fn reads_an_order() {
        let v = json!({"order": {"order_id": "0000-000000-000000", "product_id": "BTC-USD", "user_id": "2222-000000-000000",
            "order_configuration": {"market_market_ioc": {"quote_size": "50"}}, "side": "BUY", "client_order_id": "arb-7-1700003600",
            "status": "FILLED", "time_in_force": "IMMEDIATE_OR_CANCEL", "created_time": "2026-10-09T14:00:05Z",
            "completion_percentage": "100", "filled_size": "0.0008", "average_filled_price": "62125.5", "fee": "",
            "number_of_fills": "2", "filled_value": "49.70", "pending_cancel": false, "size_in_quote": true, "total_fees": "0.298",
            "size_inclusive_of_fees": true, "total_value_after_fees": "50", "trigger_status": "INVALID_ORDER_TYPE",
            "order_type": "MARKET", "reject_reason": "REJECT_REASON_UNSPECIFIED", "settled": true, "product_type": "SPOT",
            "reject_message": "", "cancel_message": ""}});
        let o = parse_order(&v).unwrap();
        assert_eq!(o.status, OrderStatus::Filled);
        assert_eq!((o.filled_size, o.average_filled_price, o.total_fees), (Decimal::new(8, 4), Decimal::new(621255, 1), Decimal::new(298, 3)));
        assert_eq!(o.reason, None);
        let rejected = json!({"order": {"order_id": "1", "status": "FAILED", "filled_size": "", "average_filled_price": "",
            "total_fees": "", "reject_reason": "HOLD_FAILURE", "reject_message": ""}});
        let o = parse_order(&rejected).unwrap();
        assert_eq!((o.status, o.reason.as_deref(), o.filled_size), (OrderStatus::Failed, Some("the funds could not be held"), Decimal::ZERO));
        for (s, want) in [("OPEN", OrderStatus::Open), ("CANCEL_QUEUED", OrderStatus::Open), ("QUEUED", OrderStatus::Pending),
            ("UNKNOWN_ORDER_STATUS", OrderStatus::Pending), ("CANCELLED", OrderStatus::Cancelled), ("EXPIRED", OrderStatus::Expired)] {
            assert_eq!(order_status(s), want, "{s}");
        }
        assert!(matches!(parse_order(&json!({})), Err(ExchangeError::Malformed(_))));
    }

    #[test]
    fn finds_an_order_by_its_client_order_id_in_a_page_of_orders() {
        // List Orders (`GET /orders/historical/batch`): the same order objects Get Order wraps.
        let v = json!({"orders": [
            {"order_id": "0000-000000-000001", "product_id": "BTC-USD", "user_id": "2222-000000-000000",
             "order_configuration": {"limit_limit_gtc": {"base_size": "0.001", "limit_price": "62100", "post_only": true}},
             "side": "BUY", "client_order_id": "someone-else", "status": "OPEN", "time_in_force": "GOOD_UNTIL_CANCELLED",
             "created_time": "2026-10-09T14:00:01Z", "completion_percentage": "0", "filled_size": "0", "average_filled_price": "0",
             "number_of_fills": "0", "filled_value": "0", "pending_cancel": false, "size_in_quote": false, "total_fees": "0",
             "order_type": "LIMIT", "reject_reason": "REJECT_REASON_UNSPECIFIED", "settled": false, "product_type": "SPOT",
             "reject_message": "", "cancel_message": ""},
            {"order_id": "0000-000000-000002", "product_id": "BTC-USD", "user_id": "2222-000000-000000",
             "order_configuration": {"market_market_ioc": {"quote_size": "50"}}, "side": "BUY", "client_order_id": "arb-7-1700003600",
             "status": "FILLED", "time_in_force": "IMMEDIATE_OR_CANCEL", "created_time": "2026-10-09T14:00:05Z",
             "completion_percentage": "100", "filled_size": "0.0008", "average_filled_price": "62125.5", "fee": "",
             "number_of_fills": "2", "filled_value": "49.70", "pending_cancel": false, "size_in_quote": true, "total_fees": "0.298",
             "size_inclusive_of_fees": true, "total_value_after_fees": "50", "order_type": "MARKET",
             "reject_reason": "REJECT_REASON_UNSPECIFIED", "settled": true, "product_type": "SPOT", "reject_message": "", "cancel_message": ""}
        ], "sequence": "0", "has_next": true, "cursor": "789100"});
        let (found, next) = find_in_orders(&v, "arb-7-1700003600").unwrap();
        let o = found.unwrap();
        assert_eq!((o.order_id.as_str(), o.client_order_id.as_str(), o.status), ("0000-000000-000002", "arb-7-1700003600", OrderStatus::Filled));
        assert_eq!((o.filled_size, o.total_fees), (Decimal::new(8, 4), Decimal::new(298, 3)));
        assert_eq!(next.as_deref(), Some("789100"));
        // Not on this page, and the last page: Coinbase has no such order.
        let last = json!({"orders": [v["orders"][0].clone()], "sequence": "0", "has_next": false, "cursor": ""});
        assert_eq!(find_in_orders(&last, "arb-7-1700003600").unwrap(), (None, None));
        // More pages and no cursor to them is not an answer.
        let cut = json!({"orders": [], "has_next": true, "cursor": ""});
        assert!(matches!(find_in_orders(&cut, "x"), Err(ExchangeError::Malformed(_))));
        assert!(matches!(find_in_orders(&json!({}), "x"), Err(ExchangeError::Malformed(_))));
    }

    #[test]
    fn reads_fills_in_base() {
        let v = json!({"fills": [
            {"entry_id": "22222-2222222-22222222", "trade_id": "1111-11111-111111", "order_id": "0000-000000-000000",
             "trade_time": "2026-10-09T14:00:05Z", "trade_type": "FILL", "price": "62100", "size": "0.0005", "commission": "0.186",
             "product_id": "BTC-USD", "sequence_timestamp": "2026-10-09T14:00:05.5Z", "liquidity_indicator": "TAKER",
             "size_in_quote": false, "user_id": "3333", "side": "BUY"},
            {"entry_id": "22222-2222222-33333333", "trade_id": "1111-11111-222222", "order_id": "0000-000000-000000",
             "trade_time": "2026-10-09T14:00:06Z", "trade_type": "FILL", "price": "62150", "size": "18.645", "commission": "0.112",
             "product_id": "BTC-USD", "size_in_quote": true, "side": "BUY"}
        ], "cursor": "789100"});
        let (f, next) = parse_fills(&v).unwrap();
        assert_eq!(next.as_deref(), Some("789100"));
        assert_eq!(f[0].trade_id, "22222-2222222-22222222");
        assert_eq!((f[0].price, f[0].size, f[0].fee), (Decimal::new(62100, 0), Decimal::new(5, 4), Decimal::new(186, 3)));
        assert_eq!(f[0].at, "2026-10-09T14:00:05Z".parse::<jiff::Timestamp>().unwrap().as_second());
        assert_eq!(f[1].size, Decimal::new(3, 4), "18.645 quote at 62150 is 0.0003 base");
        assert_eq!(parse_fills(&json!({"fills": [], "cursor": ""})).unwrap(), (vec![], None));
    }

    #[test]
    fn reads_cancel_results() {
        let ok = json!({"results": [{"success": true, "failure_reason": "UNKNOWN_CANCEL_FAILURE_REASON", "order_id": "a"},
            {"success": false, "failure_reason": "ORDER_IS_FULLY_FILLED", "order_id": "b"},
            {"success": false, "failure_reason": "DUPLICATE_CANCEL_REQUEST", "order_id": "c"}]});
        assert_eq!(parse_cancel(&ok), Ok(()));
        let refused = json!({"results": [{"success": false, "failure_reason": "NOT_ALLOWED_TO_CANCEL", "order_id": "d"}]});
        assert_eq!(parse_cancel(&refused), Err(ExchangeError::Refused("could not cancel d: not allowed to cancel".into())));
    }

    #[test]
    fn account_calls_need_a_key() {
        let cb = Coinbase::public();
        assert_eq!(cb.permissions(), Err(ExchangeError::Auth("No Coinbase key is saved".into())));
        assert!(matches!(cb.quote("not a product"), Err(ExchangeError::Refused(_))));
        let keyed = Coinbase::with_key(&Credentials::parse(NAME, SEC1).unwrap()).unwrap();
        assert!(keyed.has_key() && !format!("{keyed:?}").contains("MHcCAQEE"));
    }
}
