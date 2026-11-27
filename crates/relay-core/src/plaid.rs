//! Bank connections through Plaid (Tally's `docs/PLAID.md`; docs/MONEY.md, "Banks").
//!
//! The server keeps Plaid's keys (`PLAID_CLIENT_ID`, `PLAID_SECRET`, `PLAID_ENV`) and each bank's
//! access token, and reads transactions every four hours and on `money.bank.refresh`. They land in
//! the ledger as ordinary entries (`relay_money::plaid`) and reach every device by `money.sync`.
//! The phone never holds a secret: it asks for a link token, opens Plaid's sign-in, and hands back
//! the one-time public token (or, without Plaid's SDK, opens Plaid's hosted page and asks
//! `money.bank.link_status` how it went).
//!
//! The banks are kept in `plaid.db` beside the store (0600), behind their own mutex, and never
//! held across a call to Plaid. Access tokens are sealed with AES-256-GCM under
//! `PLAID_TOKEN_KEY` (64 hex digits), so the file alone opens nothing. Plaid's access is read only.

use crate::engine::Engine;
use relay_bus::ops::money::{BankAccountView, BankFollow, BankView};
use relay_bus::BusError;
use relay_money::plaid::{BankAccount, PlaidTx, Target};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// How often every bank is read.
pub const EVERY: Duration = Duration::from_secs(4 * 60 * 60);
/// The first read after the engine starts.
const FIRST: Duration = Duration::from_secs(60);
/// How long Plaid has to answer one request.
const TIMEOUT: Duration = Duration::from_secs(30);
/// Transactions per `/transactions/sync` page: Plaid's most.
const PAGE: u32 = 500;
/// The history Plaid is asked to keep for a new connection, so a `since` far back finds
/// something: its most, two years. Plaid's default is 90 days.
const DAYS_REQUESTED: u32 = 730;
/// Plaid's `client_user_id`. This server holds one person's ledger, so one user.
const CLIENT_USER: &str = "relay-owner";
/// What a hosted link lives for when Plaid does not say.
const HOSTED_LINK_MS: i64 = 30 * 60 * 1000;

const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS banks (
    item_id TEXT PRIMARY KEY, institution TEXT NOT NULL, institution_id TEXT NOT NULL DEFAULT '',
    token BLOB NOT NULL, cursor TEXT NOT NULL DEFAULT '', since TEXT, last_sync INTEGER, problem TEXT,
    created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS accounts (
    item_id TEXT NOT NULL, account_id TEXT NOT NULL, name TEXT NOT NULL, mask TEXT NOT NULL, kind TEXT NOT NULL,
    follow INTEGER NOT NULL DEFAULT 0, into_uid TEXT, sort INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (item_id, account_id));
CREATE TABLE IF NOT EXISTS links (
    link_token TEXT PRIMARY KEY, expires_at INTEGER NOT NULL, item_id TEXT, exited INTEGER NOT NULL DEFAULT 0,
    problem TEXT, created_at INTEGER NOT NULL);
";

/// What a request to Plaid answered, or why it did not.
pub type Answer = Result<Value, PlaidError>;
/// Stands in for Plaid in tests: the path (`/item/public_token/exchange`) and the body sent, keys
/// left out.
pub type Fake = Arc<dyn Fn(&str, &Value) -> Answer + Send + Sync>;

/// Plaid's error object, or a network failure (`code` empty).
#[derive(Debug, Clone, PartialEq)]
pub struct PlaidError {
    pub code: String,
    pub message: String,
    /// The text Plaid meant for the person, when it has one.
    pub display: Option<String>,
}

impl PlaidError {
    fn offline(message: impl Into<String>) -> PlaidError {
        PlaidError { code: String::new(), message: message.into(), display: None }
    }

    /// Whether the person must sign in to the bank again before it reads anything.
    fn needs_login(&self) -> bool {
        matches!(self.code.as_str(), "ITEM_LOGIN_REQUIRED" | "PENDING_EXPIRATION" | "PENDING_DISCONNECT" | "ACCESS_NOT_GRANTED" | "ITEM_LOCKED" | "USER_SETUP_REQUIRED")
    }

    /// The item is gone at Plaid already: there is nothing left to remove.
    fn item_gone(&self) -> bool {
        matches!(self.code.as_str(), "ITEM_NOT_FOUND" | "INVALID_ACCESS_TOKEN")
    }

    /// What the person reads.
    fn human(&self, institution: &str) -> String {
        let bank = if institution.trim().is_empty() { "your bank" } else { institution };
        match self.code.as_str() {
            _ if self.needs_login() => format!("Sign in to {bank} again"),
            "INVALID_API_KEYS" | "INVALID_SECRET" | "INVALID_CLIENT_ID" => "Plaid refused this server's keys: check PLAID_CLIENT_ID, PLAID_SECRET and PLAID_ENV".into(),
            "INVALID_FIELD" if self.message.contains("android_package_name") => {
                "Plaid does not know this app: add its package name under Allowed Android package names in Plaid's dashboard".into()
            }
            "" => self.message.clone(),
            _ => self.display.clone().filter(|d| !d.trim().is_empty()).unwrap_or_else(|| format!("Plaid said: {}", self.message)),
        }
    }

    fn bus(&self, institution: &str) -> BusError {
        if self.code.is_empty() {
            BusError::unavailable("money.plaid_offline", self.human(institution)).with_hint("check the server's network, then try again")
        } else {
            BusError::invalid("money.plaid", self.human(institution))
        }
    }
}

/// The server's Plaid settings.
#[derive(Clone)]
pub struct Config {
    client_id: String,
    secret: String,
    /// `https://production.plaid.com` or `https://sandbox.plaid.com`.
    base: String,
    key: [u8; 32],
}

impl Config {
    /// From the environment. Not set up: a readable error, which the phone shows as is.
    pub fn from_env() -> Result<Config, BusError> {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let (Some(client_id), Some(secret)) = (var("PLAID_CLIENT_ID"), var("PLAID_SECRET")) else {
            return Err(BusError::unavailable("money.plaid_setup", "This server is not set up for Plaid yet: it needs PLAID_CLIENT_ID and PLAID_SECRET"));
        };
        let base = match var("PLAID_ENV").as_deref().unwrap_or("production") {
            "production" => "https://production.plaid.com",
            "sandbox" => "https://sandbox.plaid.com",
            other => return Err(BusError::unavailable("money.plaid_setup", format!("PLAID_ENV is {other:?}: set it to production or sandbox"))),
        };
        let key = var("PLAID_TOKEN_KEY").and_then(|k| hex32(&k)).ok_or_else(|| {
            BusError::unavailable("money.plaid_setup", "This server needs PLAID_TOKEN_KEY (64 hex digits, e.g. from `openssl rand -hex 32`) to keep bank logins sealed")
        })?;
        Ok(Config { client_id, secret, base: base.into(), key })
    }

    #[cfg(test)]
    pub fn for_tests() -> Config {
        Config { client_id: "client".into(), secret: "secret".into(), base: "https://sandbox.plaid.com".into(), key: [7; 32] }
    }

    fn seal(&self, token: &str) -> Result<Vec<u8>, BusError> {
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &self.key).map_err(|_| BusError::internal("plaid: the token key"))?);
        let mut nonce = [0u8; NONCE_LEN];
        SystemRandom::new().fill(&mut nonce).map_err(|_| BusError::internal("plaid: no randomness"))?;
        let mut sealed = token.as_bytes().to_vec();
        key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut sealed).map_err(|_| BusError::internal("plaid: sealing a token"))?;
        Ok([&nonce[..], &sealed].concat())
    }

    fn open(&self, sealed: &[u8]) -> Result<String, BusError> {
        let wrong = || BusError::unavailable("money.plaid_setup", "This server cannot open its bank logins: PLAID_TOKEN_KEY changed. Remove the bank and connect it again");
        if sealed.len() < NONCE_LEN {
            return Err(wrong());
        }
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &self.key).map_err(|_| wrong())?);
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        let mut body = body.to_vec();
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| wrong())?;
        let plain = key.open_in_place(nonce, Aad::empty(), &mut body).map_err(|_| wrong())?;
        String::from_utf8(plain.to_vec()).map_err(|_| wrong())
    }
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// `plaid.db` and the engine's Plaid state.
pub struct Store {
    conn: Connection,
    /// Tests: the settings and the stand-in for Plaid.
    config: Option<Config>,
    fake: Option<Fake>,
    /// A read is running; `again` asks it for one more pass.
    reading: bool,
    again: bool,
    /// Link tokens whose public token is being exchanged right now, so two `link_status` calls
    /// never exchange it twice.
    exchanging: HashSet<String>,
}

impl Store {
    fn open(path: Option<&Path>) -> Result<Store, BusError> {
        let sql = |e: rusqlite::Error| BusError::internal(format!("plaid.db: {e}"));
        let conn = match path {
            Some(p) => {
                let conn = Connection::open(p).map_err(sql)?;
                conn.pragma_update(None, "journal_mode", "WAL").map_err(sql)?;
                #[cfg(unix)]
                for side in ["", "-wal", "-shm"] {
                    use std::os::unix::fs::PermissionsExt;
                    let mut name = p.as_os_str().to_owned();
                    name.push(side);
                    let _ = std::fs::set_permissions(Path::new(&name), std::fs::Permissions::from_mode(0o600));
                }
                conn.busy_timeout(Duration::from_secs(5)).map_err(sql)?;
                conn
            }
            None => Connection::open_in_memory().map_err(sql)?,
        };
        conn.execute_batch(SCHEMA).map_err(sql)?;
        Ok(Store { conn, config: None, fake: None, reading: false, again: false, exchanging: HashSet::new() })
    }
}

fn sql(e: rusqlite::Error) -> BusError {
    BusError::internal(format!("plaid.db: {e}"))
}

/// Runs `f` on the banks, opening `plaid.db` beside the store on first use (in memory when the
/// store is). Never call Plaid inside `f`.
pub(crate) fn store<T>(engine: &Engine, f: impl FnOnce(&mut Store) -> Result<T, BusError>) -> Result<T, BusError> {
    let mut slot = engine.banks.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        let path = engine.store.path();
        *slot = Some(if path == Path::new(":memory:") { Store::open(None)? } else { Store::open(Some(&path.with_file_name("plaid.db")))? });
    }
    f(slot.as_mut().expect("opened above"))
}

/// Tests: answer for Plaid with `fake`, under test settings.
#[cfg(test)]
pub(crate) fn set_fake(engine: &Engine, fake: Fake) {
    store(engine, |s| {
        s.config = Some(Config::for_tests());
        s.fake = Some(fake);
        Ok(())
    }).unwrap();
}

/// The settings, and how to reach Plaid.
#[derive(Clone)]
struct Plaid {
    config: Config,
    fake: Option<Fake>,
}

fn plaid(engine: &Engine) -> Result<Plaid, BusError> {
    let (config, fake) = store(engine, |s| Ok((s.config.clone(), s.fake.clone())))?;
    Ok(Plaid { config: match config { Some(c) => c, None => Config::from_env()? }, fake })
}

/// Whether this engine is set up for Plaid: the readable error when it is not.
pub fn check(engine: &Engine) -> Result<(), BusError> {
    plaid(engine).map(|_| ())
}

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(TIMEOUT))
            .user_agent("relay-money")
            .build()
            .into()
    })
}

impl Plaid {
    /// One POST to Plaid, the keys added to the body.
    fn call(&self, path: &str, body: Value) -> Answer {
        if let Some(fake) = &self.fake {
            return fake(path, &body);
        }
        let mut body = body;
        body["client_id"] = json!(self.config.client_id);
        body["secret"] = json!(self.config.secret);
        let url = format!("{}{path}", self.config.base);
        let mut response = agent().post(&url).header("Content-Type", "application/json").send_json(&body).map_err(|e| match e {
            ureq::Error::Timeout(_) => PlaidError::offline(format!("Plaid did not answer within {} s", TIMEOUT.as_secs())),
            e => {
                tracing::debug!(error = %e, path, "plaid: no answer");
                PlaidError::offline("Plaid could not be reached from the server")
            }
        })?;
        let status = response.status().as_u16();
        let text = response.body_mut().with_config().limit(32 << 20).read_to_string().map_err(|_| PlaidError::offline("Plaid's answer was cut off"))?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(v);
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let error = PlaidError { code: s("error_code"), message: s("error_message"), display: v.get("display_message").and_then(Value::as_str).map(str::to_string) };
        tracing::info!(path, status, code = %error.code, request_id = %s("request_id"), "plaid: refused");
        Err(if error.code.is_empty() { PlaidError { code: format!("HTTP_{status}"), message: format!("Plaid answered {status}"), display: None } } else { error })
    }

    /// `/link/token/create` for Transactions in Canada, in English.
    fn link_token(&self, client_name: &str, android_package_name: Option<&str>, hosted: Option<Option<&str>>) -> Answer {
        let mut body = json!({
            "client_name": client_name, "products": ["transactions"], "country_codes": ["CA"], "language": "en",
            "user": {"client_user_id": CLIENT_USER}, "transactions": {"days_requested": DAYS_REQUESTED},
        });
        if let Some(package) = android_package_name {
            body["android_package_name"] = json!(package);
        }
        if let Some(redirect) = hosted {
            body["hosted_link"] = match redirect {
                Some(uri) => json!({"completion_redirect_uri": uri}),
                None => json!({}),
            };
        }
        self.call("/link/token/create", body)
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string()
}

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// A bank as the phone reads it.
fn bank_view(conn: &Connection, item_id: &str) -> Result<Option<BankView>, BusError> {
    let bank = conn.prepare_cached("SELECT institution, last_sync, problem FROM banks WHERE item_id = ?1").map_err(sql)?
        .query_row([item_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<String>>(2)?))).optional().map_err(sql)?;
    let Some((institution, last_sync, problem)) = bank else { return Ok(None) };
    let mut st = conn.prepare_cached("SELECT account_id, name, mask, kind, follow, into_uid FROM accounts WHERE item_id = ?1 ORDER BY sort, account_id").map_err(sql)?;
    let accounts = st.query_map([item_id], |r| {
        Ok(BankAccountView { id: r.get(0)?, name: r.get(1)?, mask: r.get(2)?, kind: r.get(3)?, follow: r.get(4)?, into: r.get(5)? })
    }).map_err(sql)?.collect::<Result<Vec<_>, _>>().map_err(sql)?;
    Ok(Some(BankView { id: item_id.to_string(), institution, last_sync, problem, accounts }))
}

/// Every bank, oldest connection first.
pub fn list(engine: &Engine) -> Result<Vec<BankView>, BusError> {
    store(engine, |s| {
        let ids: Vec<String> = s.conn.prepare_cached("SELECT item_id FROM banks ORDER BY created_at, item_id").map_err(sql)?
            .query_map([], |r| r.get(0)).map_err(sql)?.collect::<Result<_, _>>().map_err(sql)?;
        ids.iter().filter_map(|id| bank_view(&s.conn, id).transpose()).collect()
    })
}

/// A link token for the SDK's sign-in on an Android phone.
pub fn link_token(engine: &Engine, name: &str, android_package_name: &str) -> Result<String, BusError> {
    let package = android_package_name.trim();
    if package.is_empty() || package.len() > 255 {
        return Err(BusError::invalid("money.invalid", "Name the Android app asking (its package name)"));
    }
    let p = plaid(engine)?;
    let v = p.link_token(client_name(name)?, Some(package), None).map_err(|e| e.bus(""))?;
    nonempty(s(&v, "link_token"), "a link token")
}

fn client_name(name: &str) -> Result<&str, BusError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return Err(BusError::invalid("money.invalid", "The app's name for Plaid's sign-in is 1 to 30 characters"));
    }
    Ok(name)
}

fn nonempty(v: String, what: &str) -> Result<String, BusError> {
    if v.is_empty() {
        Err(BusError::unavailable("money.plaid", format!("Plaid did not hand back {what}")))
    } else {
        Ok(v)
    }
}

/// A sign-in on Plaid's hosted page: its token, its address and when it runs out.
pub fn hosted_link(engine: &Engine, name: &str, redirect: Option<&str>) -> Result<(String, String, Option<i64>), BusError> {
    let redirect = redirect.map(str::trim).filter(|r| !r.is_empty());
    if let Some(uri) = redirect {
        // An app's own scheme or an https page; never a script or a file.
        let scheme = uri.split_once("://").map(|(s, _)| s.to_ascii_lowercase());
        if !scheme.is_some_and(|s| !s.is_empty() && !matches!(s.as_str(), "javascript" | "data" | "file" | "http") && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))) || uri.len() > 512 {
            return Err(BusError::invalid("money.invalid", "The address Plaid's page returns to is an app's own link or an https page"));
        }
    }
    let p = plaid(engine)?;
    let v = p.link_token(client_name(name)?, None, Some(redirect)).map_err(|e| e.bus(""))?;
    let token = nonempty(s(&v, "link_token"), "a link token")?;
    let url = nonempty(s(&v, "hosted_link_url"), "a sign-in page")?;
    if !url.starts_with("https://") {
        return Err(BusError::unavailable("money.plaid", "Plaid's sign-in page is not an https address"));
    }
    let expires_at = s(&v, "expiration").parse::<jiff::Timestamp>().ok().map(|t| t.as_millisecond());
    let created = now_ms();
    store(engine, |st| {
        st.conn.prepare_cached("INSERT OR REPLACE INTO links (link_token, expires_at, created_at) VALUES (?1, ?2, ?3)").map_err(sql)?
            .execute(params![token, expires_at.unwrap_or(created + HOSTED_LINK_MS), created]).map_err(sql)?;
        // Links long over are of no use to anyone.
        st.conn.prepare_cached("DELETE FROM links WHERE expires_at < ?1").map_err(sql)?.execute([created - 7 * 24 * 60 * 60 * 1000]).map_err(sql)?;
        Ok(())
    })?;
    Ok((token, url, expires_at))
}

/// Trades a public token for a connection kept here: the token sealed, the accounts listed and
/// none followed.
pub fn exchange(engine: &Engine, public_token: &str, institution_id: Option<&str>, institution_name: Option<&str>) -> Result<BankView, BusError> {
    let public_token = public_token.trim();
    if public_token.is_empty() || public_token.len() > 512 {
        return Err(BusError::invalid("money.invalid", "Plaid's public token is missing"));
    }
    let p = plaid(engine)?;
    connect(engine, &p, public_token, institution_id, institution_name)
}

fn connect(engine: &Engine, p: &Plaid, public_token: &str, institution_id: Option<&str>, institution_name: Option<&str>) -> Result<BankView, BusError> {
    let given = institution_name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or("");
    let v = p.call("/item/public_token/exchange", json!({"public_token": public_token})).map_err(|e| e.bus(given))?;
    let access = nonempty(s(&v, "access_token"), "an access token")?;
    let item_id = nonempty(s(&v, "item_id"), "the connection's id")?;
    let sealed = p.config.seal(&access)?;
    // Kept before anything else can fail, so a connection made at Plaid is never lost here.
    store(engine, |st| {
        st.conn.prepare_cached("INSERT OR REPLACE INTO banks (item_id, institution, token, created_at) VALUES (?1, ?2, ?3, ?4)").map_err(sql)?
            .execute(params![item_id, if given.is_empty() { "A bank" } else { given }, sealed, now_ms()]).map_err(sql)?;
        Ok(())
    })?;
    let accounts = p.call("/accounts/get", json!({"access_token": access})).map_err(|e| e.bus(given))?;
    let inst_id = institution_id.map(str::trim).filter(|i| !i.is_empty()).map(str::to_string).unwrap_or_else(|| s(&accounts["item"], "institution_id"));
    let institution = if !given.is_empty() {
        given.to_string()
    } else if inst_id.is_empty() {
        "A bank".to_string()
    } else {
        p.call("/institutions/get_by_id", json!({"institution_id": inst_id, "country_codes": ["CA"]}))
            .ok().map(|v| s(&v["institution"], "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| "A bank".into())
    };
    let list: Vec<(String, String, String, String)> = accounts["accounts"].as_array().into_iter().flatten().filter_map(|a| {
        let id = s(a, "account_id");
        if id.is_empty() {
            return None;
        }
        let name = [s(a, "name"), s(a, "official_name")].into_iter().find(|n| !n.is_empty()).unwrap_or_else(|| "Account".into());
        let kind = [s(a, "subtype"), s(a, "type")].into_iter().find(|k| !k.is_empty()).unwrap_or_default().replace('_', " ");
        Some((id, name, s(a, "mask"), kind))
    }).collect();
    store(engine, |st| {
        let tx = st.conn.transaction().map_err(sql)?;
        tx.prepare_cached("UPDATE banks SET institution = ?2, institution_id = ?3 WHERE item_id = ?1").map_err(sql)?.execute(params![item_id, institution, inst_id]).map_err(sql)?;
        for (i, (id, name, mask, kind)) in list.iter().enumerate() {
            tx.prepare_cached(
                "INSERT INTO accounts (item_id, account_id, name, mask, kind, sort) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (item_id, account_id) DO UPDATE SET name = excluded.name, mask = excluded.mask, kind = excluded.kind, sort = excluded.sort",
            ).map_err(sql)?.execute(params![item_id, id, name, mask, kind, i as i64]).map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        bank_view(&st.conn, &item_id)?.ok_or_else(|| BusError::internal("plaid: the bank just kept is gone"))
    })
}

/// How a hosted sign-in went: `waiting`, `connected` (with the bank), `exited` (with Plaid's
/// reason) or `expired`. Once connected, asked again, the same bank: nothing is exchanged twice.
pub fn link_status(engine: &Engine, link_token: &str) -> Result<(String, Option<BankView>, Option<String>), BusError> {
    let token = link_token.trim().to_string();
    let not_found = || BusError::not_found("money.not_found", "That sign-in is not one this server started");
    let row = store(engine, |st| {
        st.conn.prepare_cached("SELECT expires_at, item_id, exited, problem FROM links WHERE link_token = ?1").map_err(sql)?
            .query_row([&token], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, bool>(2)?, r.get::<_, Option<String>>(3)?)))
            .optional().map_err(sql)
    })?;
    let Some((expires_at, item, exited, problem)) = row else { return Err(not_found()) };
    if let Some(item) = item {
        let bank = store(engine, |st| bank_view(&st.conn, &item))?;
        // A bank removed since is no longer connected; the sign-in is over all the same.
        return Ok(match bank {
            Some(bank) => ("connected".into(), Some(bank), None),
            None => ("exited".into(), None, Some("That bank was disconnected since".into())),
        });
    }
    if exited {
        return Ok(("exited".into(), None, problem));
    }
    // One exchange per token: a second ask while one runs waits for its answer.
    let claimed = store(engine, |st| Ok(st.exchanging.insert(token.clone())))?;
    if !claimed {
        return Ok(("waiting".into(), None, None));
    }
    let outcome = (|| {
        let p = plaid(engine)?;
        let v = p.call("/link/token/get", json!({"link_token": token})).map_err(|e| e.bus(""))?;
        match session_outcome(&v) {
            Outcome::Connected { public_token, institution_id, institution } => {
                let bank = connect(engine, &p, &public_token, institution_id.as_deref(), institution.as_deref())?;
                store(engine, |st| {
                    st.conn.prepare_cached("UPDATE links SET item_id = ?2 WHERE link_token = ?1").map_err(sql)?.execute(params![token, bank.id]).map_err(sql)?;
                    Ok(())
                })?;
                Ok(("connected".to_string(), Some(bank), None))
            }
            Outcome::Exited(problem) => {
                store(engine, |st| {
                    st.conn.prepare_cached("UPDATE links SET exited = 1, problem = ?2 WHERE link_token = ?1").map_err(sql)?.execute(params![token, problem]).map_err(sql)?;
                    Ok(())
                })?;
                Ok(("exited".to_string(), None, problem))
            }
            Outcome::Waiting if now_ms() > expires_at => Ok(("expired".to_string(), None, None)),
            Outcome::Waiting => Ok(("waiting".to_string(), None, None)),
        }
    })();
    store(engine, |st| Ok(st.exchanging.remove(&token)))?;
    outcome
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Waiting,
    Connected { public_token: String, institution_id: Option<String>, institution: Option<String> },
    Exited(Option<String>),
}

/// What `/link/token/get` says came of a hosted sign-in: a public token in a session's results
/// (or its `on_success`), else a session that finished on an exit, else still waiting.
fn session_outcome(v: &Value) -> Outcome {
    let sessions = v["link_sessions"].as_array().cloned().unwrap_or_default();
    for session in &sessions {
        let added = session["results"]["item_add_results"].as_array().and_then(|r| r.first()).cloned();
        let success = added.or_else(|| session.get("on_success").cloned());
        if let Some(found) = success {
            let public_token = s(&found, "public_token");
            if !public_token.is_empty() {
                let institution = found.get("institution").or_else(|| found["metadata"].get("institution")).cloned().unwrap_or_default();
                let opt = |k: &str| Some(s(&institution, k)).filter(|x| !x.is_empty());
                return Outcome::Connected { public_token, institution_id: opt("institution_id"), institution: opt("name") };
            }
        }
    }
    for session in &sessions {
        if let Some(exit) = session.get("exit").filter(|e| !e.is_null()) {
            let problem = [s(&exit["error"], "display_message"), s(&exit["error"], "error_message")].into_iter().find(|m| !m.is_empty());
            return Outcome::Exited(problem);
        }
    }
    Outcome::Waiting
}

/// Which of a bank's accounts are followed and where each lands, and the first day to bring in.
/// A followed account gets its ledger account now (chosen, found or made). Following an account
/// not followed before, or moving `since` earlier, reads the bank from the start again: what was
/// passed over before is brought in, and what is already there is not written twice.
pub fn follow(engine: &Engine, bank_id: &str, since: Option<&str>, accounts: &[BankFollow]) -> Result<BankView, BusError> {
    let since = since.map(str::trim).filter(|d| !d.is_empty());
    if let Some(day) = since {
        if relay_money::ledger::parse_date(day).is_none() {
            return Err(BusError::invalid("money.invalid", format!("Not a date: {day}")));
        }
    }
    let not_found = || BusError::not_found("money.not_found", "That bank is not connected to this server");
    let (institution, old_since, known) = store(engine, |st| {
        let bank = st.conn.prepare_cached("SELECT institution, since FROM banks WHERE item_id = ?1").map_err(sql)?
            .query_row([bank_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))).optional().map_err(sql)?;
        let Some((institution, since)) = bank else { return Err(not_found()) };
        let mut st = st.conn.prepare_cached("SELECT account_id, name, mask, kind, follow, into_uid FROM accounts WHERE item_id = ?1").map_err(sql)?;
        let known: HashMap<String, (String, String, String, bool, Option<String>)> = st
            .query_map([bank_id], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))))
            .map_err(sql)?.collect::<Result<_, _>>().map_err(sql)?;
        Ok((institution, since, known))
    })?;
    if let Some(unknown) = accounts.iter().find(|a| !known.contains_key(&a.id)) {
        return Err(BusError::invalid("money.invalid", format!("{institution} has no account {}", unknown.id)));
    }
    // The ledger accounts first, outside the banks' lock.
    let mut chosen: Vec<(String, bool, Option<String>)> = Vec::new();
    let mut reread = since.is_some_and(|d| old_since.as_deref().is_none_or(|o| d < o));
    for a in accounts {
        let (name, mask, kind, was, into) = &known[&a.id];
        let into = if a.follow {
            let want = a.into.as_deref().or(into.as_deref());
            let (_, uid) = crate::handlers::money::ledger(engine, |l| {
                l.plaid_account(&BankAccount { into: want, name, mask, kind, institution: &institution })
            })?;
            reread |= !was;
            Some(uid)
        } else {
            into.clone()
        };
        chosen.push((a.id.clone(), a.follow, into));
    }
    store(engine, |st| {
        let tx = st.conn.transaction().map_err(sql)?;
        for (id, follow, into) in &chosen {
            tx.prepare_cached("UPDATE accounts SET follow = ?3, into_uid = ?4 WHERE item_id = ?1 AND account_id = ?2").map_err(sql)?
                .execute(params![bank_id, id, follow, into]).map_err(sql)?;
        }
        if since.is_some() {
            tx.prepare_cached("UPDATE banks SET since = ?2 WHERE item_id = ?1").map_err(sql)?.execute(params![bank_id, since]).map_err(sql)?;
        }
        if reread {
            tx.prepare_cached("UPDATE banks SET cursor = '' WHERE item_id = ?1").map_err(sql)?.execute([bank_id]).map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        bank_view(&st.conn, bank_id)?.ok_or_else(not_found)
    })
}

/// Disconnects a bank at Plaid and here. Entries already written stay.
pub fn remove(engine: &Engine, bank_id: &str) -> Result<(), BusError> {
    let row = store(engine, |st| {
        st.conn.prepare_cached("SELECT institution, token FROM banks WHERE item_id = ?1").map_err(sql)?
            .query_row([bank_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))).optional().map_err(sql)
    })?;
    let Some((institution, sealed)) = row else {
        return Err(BusError::not_found("money.not_found", "That bank is not connected to this server"));
    };
    let p = plaid(engine)?;
    // A token this server can no longer open cannot be removed at Plaid; dropping it here still
    // stops every read (Plaid's dashboard removes the item itself).
    match p.config.open(&sealed) {
        Ok(access) => match p.call("/item/remove", json!({"access_token": access})) {
            Ok(_) => {}
            Err(e) if e.item_gone() => {}
            Err(e) => return Err(e.bus(&institution)),
        },
        Err(e) => tracing::warn!(bank = bank_id, error = %e.message, "plaid: dropping a bank whose token cannot be opened"),
    }
    store(engine, |st| {
        let tx = st.conn.transaction().map_err(sql)?;
        tx.execute("DELETE FROM accounts WHERE item_id = ?1", [bank_id]).map_err(sql)?;
        tx.execute("DELETE FROM banks WHERE item_id = ?1", [bank_id]).map_err(sql)?;
        tx.commit().map_err(sql)
    })
}

/// One bank to read: its sealed token, cursor and followed accounts.
struct Due {
    item_id: String,
    institution: String,
    sealed: Vec<u8>,
    cursor: String,
    since: Option<String>,
    /// Plaid account id → (name, mask, kind, into).
    follows: Vec<(String, String, String, String, Option<String>)>,
}

/// A bank's row: item id, institution, sealed token, cursor, since.
type BankRow = (String, String, Vec<u8>, String, Option<String>);

/// What one read of every bank did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Read {
    pub banks: usize,
    pub written: usize,
    pub failed: usize,
}

/// Reads every bank with a followed account now, on this thread: `/transactions/sync` from each
/// bank's cursor to the end, written in one ledger transaction, then the cursor moves. A bank
/// that fails keeps its cursor and says why in `problem`; the others go on.
pub fn read_all(engine: &Engine) -> Result<Read, BusError> {
    let p = plaid(engine)?;
    let due: Vec<Due> = store(engine, |st| {
        let banks: Vec<BankRow> = st.conn
            .prepare_cached("SELECT item_id, institution, token, cursor, since FROM banks ORDER BY created_at, item_id").map_err(sql)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map_err(sql)?
            .collect::<Result<_, _>>().map_err(sql)?;
        let mut out = Vec::new();
        for (item_id, institution, sealed, cursor, since) in banks {
            let follows = st.conn.prepare_cached("SELECT account_id, name, mask, kind, into_uid FROM accounts WHERE item_id = ?1 AND follow = 1").map_err(sql)?
                .query_map([&item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map_err(sql)?
                .collect::<Result<Vec<_>, _>>().map_err(sql)?;
            if !follows.is_empty() {
                out.push(Due { item_id, institution, sealed, cursor, since, follows });
            }
        }
        Ok(out)
    })?;
    let mut read = Read::default();
    for bank in due {
        read.banks += 1;
        match read_bank(engine, &p, &bank) {
            Ok(written) => read.written += written,
            Err(problem) => {
                read.failed += 1;
                tracing::info!(bank = %bank.item_id, problem = %problem, "plaid: a bank was not read");
                store(engine, |st| {
                    st.conn.prepare_cached("UPDATE banks SET problem = ?2 WHERE item_id = ?1").map_err(sql)?.execute(params![bank.item_id, problem]).map_err(sql)?;
                    Ok(())
                })?;
            }
        }
    }
    Ok(read)
}

/// One bank, start to end. The error is what the person reads in `problem`.
fn read_bank(engine: &Engine, p: &Plaid, bank: &Due) -> Result<usize, String> {
    let access = p.config.open(&bank.sealed).map_err(|e| e.message)?;
    let (mut added, mut modified, mut removed) = (Vec::new(), Vec::new(), Vec::new());
    let mut cursor = bank.cursor.clone();
    let mut restarts = 0;
    loop {
        let mut body = json!({"access_token": access, "count": PAGE, "options": {"include_personal_finance_category": true}});
        if !cursor.is_empty() {
            body["cursor"] = json!(cursor);
        }
        match p.call("/transactions/sync", body) {
            Ok(v) => {
                added.extend(v["added"].as_array().into_iter().flatten().filter_map(plaid_tx));
                modified.extend(v["modified"].as_array().into_iter().flatten().filter_map(plaid_tx));
                removed.extend(v["removed"].as_array().into_iter().flatten().map(|r| s(r, "transaction_id")).filter(|id| !id.is_empty()));
                cursor = s(&v, "next_cursor");
                if !v["has_more"].as_bool().unwrap_or(false) {
                    break;
                }
            }
            // Plaid's data moved under the pages: start again from where this read began.
            Err(e) if e.code == "TRANSACTIONS_SYNC_MUTATION_DURING_PAGINATION" && restarts < 3 => {
                restarts += 1;
                (added, modified, removed, cursor) = (Vec::new(), Vec::new(), Vec::new(), bank.cursor.clone());
            }
            Err(e) => return Err(e.human(&bank.institution)),
        }
    }
    // Desjardins sends CAD amounts labelled USD through Plaid.
    let any_currency = bank.institution.to_lowercase().contains("desjardins");
    let (applied, intos) = crate::handlers::money::ledger(engine, |l| {
        let mut targets = HashMap::new();
        let mut intos = Vec::new();
        for (id, name, mask, kind, into) in &bank.follows {
            let (account_id, uid) = l.plaid_account(&BankAccount { into: into.as_deref(), name, mask, kind, institution: &bank.institution })?;
            targets.insert(id.clone(), Target { account_id, since: bank.since.clone(), any_currency });
            intos.push((id.clone(), uid));
        }
        Ok((l.plaid_apply(&targets, &added, &modified, &removed)?, intos))
    }).map_err(|e| e.message)?;
    if applied.other_currency > 0 {
        tracing::info!(bank = %bank.item_id, left_out = applied.other_currency, "plaid: transactions in another currency than the ledger's were left out");
    }
    store(engine, |st| {
        let tx = st.conn.transaction().map_err(sql)?;
        tx.prepare_cached("UPDATE banks SET cursor = ?2, last_sync = ?3, problem = NULL WHERE item_id = ?1").map_err(sql)?
            .execute(params![bank.item_id, cursor, now_ms()]).map_err(sql)?;
        for (id, uid) in &intos {
            tx.prepare_cached("UPDATE accounts SET into_uid = ?3 WHERE item_id = ?1 AND account_id = ?2").map_err(sql)?.execute(params![bank.item_id, id, uid]).map_err(sql)?;
        }
        tx.commit().map_err(sql)
    }).map_err(|e| e.message)?;
    if applied.changed() {
        engine.emit_system("money.changed", json!({"bank": bank.item_id, "written": applied.written, "transfers": applied.transfers, "removed": applied.removed}));
    }
    Ok(applied.written)
}

/// A transaction from `/transactions/sync`; its amount kept as the text Plaid wrote.
fn plaid_tx(v: &Value) -> Option<PlaidTx> {
    let opt = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string);
    let amount = match v.get("amount")? {
        Value::Number(n) => n.to_string(),
        Value::String(t) => t.clone(),
        _ => return None,
    };
    let pfc = &v["personal_finance_category"];
    Some(PlaidTx {
        transaction_id: opt("transaction_id")?,
        account_id: opt("account_id")?,
        amount,
        iso_currency_code: opt("iso_currency_code"),
        date: opt("date")?,
        pending: v["pending"].as_bool().unwrap_or(false),
        merchant_name: opt("merchant_name"),
        name: opt("name"),
        category_primary: pfc.get("primary").and_then(Value::as_str).map(str::to_string),
        category_detailed: pfc.get("detailed").and_then(Value::as_str).map(str::to_string),
    })
}

/// Reads every bank on a thread of its own. A read already running takes one more pass instead.
pub fn read_soon(engine: &Arc<Engine>) {
    let start = store(engine, |st| {
        if st.reading {
            st.again = true;
            return Ok(false);
        }
        st.reading = true;
        Ok(true)
    }).unwrap_or(false);
    if !start {
        return;
    }
    let weak = Arc::downgrade(engine);
    let run = move || loop {
        let Some(engine) = weak.upgrade() else { return };
        if let Err(e) = read_all(&engine) {
            tracing::info!(error = %e.message, "plaid: reading the banks");
        }
        let again = store(&engine, |st| {
            st.reading = std::mem::take(&mut st.again);
            Ok(st.reading)
        }).unwrap_or(false);
        if !again {
            return;
        }
    };
    if let Err(e) = std::thread::Builder::new().name("plaid-read".into()).spawn(run) {
        tracing::warn!(error = %e, "plaid: starting a read");
        let _ = store(engine, |st| {
            st.reading = false;
            Ok(())
        });
    }
}

/// Every four hours, reads every bank, on an engine set up for Plaid that holds one.
pub fn spawn_timer(engine: &Arc<Engine>) {
    if Config::from_env().is_err() {
        return;
    }
    let weak = Arc::downgrade(engine);
    let started = std::thread::Builder::new().name("plaid-timer".into()).spawn(move || {
        crate::background_priority();
        let mut wait = FIRST;
        loop {
            let deadline = std::time::Instant::now() + wait;
            while std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_secs(1));
                match weak.upgrade() {
                    Some(engine) if !engine.is_quitting() => {}
                    _ => return,
                }
            }
            let Some(engine) = weak.upgrade() else { return };
            read_soon(&engine);
            wait = EVERY;
        }
    });
    if let Err(e) = started {
        tracing::warn!(error = %e, "plaid: starting the timer");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instance, Store as EngineStore};
    use std::sync::Mutex;

    fn engine() -> Arc<Engine> {
        Engine::new(Instance::Test, EngineStore::open_memory().unwrap())
    }

    /// A Plaid with one Desjardins item: a chequing account and a Visa, and the pages of
    /// `/transactions/sync` it hands out in turn.
    fn fake(pages: Vec<Value>) -> (Fake, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let pages = Arc::new(Mutex::new(pages.into_iter()));
        let log = calls.clone();
        let fake: Fake = Arc::new(move |path: &str, body: &Value| {
            log.lock().unwrap().push(path.to_string());
            assert!(body.get("secret").is_none(), "the fake never sees the keys");
            match path {
                "/link/token/create" => {
                    let hosted = body.get("hosted_link").is_some();
                    assert_eq!(body["transactions"]["days_requested"], 730);
                    Ok(json!({"link_token": if hosted { "link-hosted" } else { "link-sdk" }, "expiration": "2099-11-19T12:30:00Z",
                              "hosted_link_url": if hosted { json!("https://hosted.plaid.com/link/abc") } else { Value::Null }}))
                }
                "/item/public_token/exchange" => {
                    assert_eq!(body["public_token"], "public-1");
                    Ok(json!({"access_token": "access-1", "item_id": "item-1"}))
                }
                "/accounts/get" => {
                    assert_eq!(body["access_token"], "access-1");
                    Ok(json!({"item": {"institution_id": "ins_desj"}, "accounts": [
                        {"account_id": "acc-chq", "name": "Compte chèques", "mask": "1111", "type": "depository", "subtype": "checking"},
                        {"account_id": "acc-visa", "name": "Visa Desjardins", "mask": "4321", "type": "credit", "subtype": "credit card"},
                    ]}))
                }
                "/institutions/get_by_id" => Ok(json!({"institution": {"name": "Desjardins"}})),
                "/link/token/get" => Ok(json!({"link_sessions": [{"results": {"item_add_results": [
                    {"public_token": "public-1", "institution": {"name": "Desjardins", "institution_id": "ins_desj"}}]}}]})),
                "/transactions/sync" => Ok(pages.lock().unwrap().next().unwrap_or_else(|| json!({"added": [], "modified": [], "removed": [], "next_cursor": "end", "has_more": false}))),
                "/item/remove" => Ok(json!({})),
                other => panic!("unexpected {other}"),
            }
        });
        (fake, calls)
    }

    fn page(added: Value, cursor: &str, more: bool) -> Value {
        json!({"added": added, "modified": [], "removed": [], "next_cursor": cursor, "has_more": more})
    }

    #[test]
    fn a_bank_connects_and_its_followed_accounts_fill_the_ledger() {
        let e = engine();
        let (f, calls) = fake(vec![
            page(json!([
                {"transaction_id": "t1", "account_id": "acc-chq", "amount": 12.34, "iso_currency_code": "USD", "date": "2026-11-03", "pending": false,
                 "merchant_name": "IGA", "personal_finance_category": {"primary": "FOOD_AND_DRINK", "detailed": "FOOD_AND_DRINK_GROCERIES"}},
                {"transaction_id": "t2", "account_id": "acc-visa", "amount": 40, "iso_currency_code": "USD", "date": "2026-11-04", "pending": false, "name": "Netflix"},
            ]), "c1", true),
            page(json!([{"transaction_id": "t3", "account_id": "acc-chq", "amount": 5, "date": "2026-11-05", "pending": true}]), "c2", false),
        ]);
        set_fake(&e, f);
        assert_eq!(link_token(&e, "Tally", "com.quietsoftware.tally2").unwrap(), "link-sdk");
        let bank = exchange(&e, "public-1", None, None).unwrap();
        assert_eq!((bank.id.as_str(), bank.institution.as_str(), bank.accounts.len()), ("item-1", "Desjardins", 2));
        assert!(bank.accounts.iter().all(|a| !a.follow && a.into.is_none()));
        assert_eq!(bank.accounts[1].kind, "credit card");
        // Nothing followed: nothing read.
        assert_eq!(read_all(&e).unwrap(), Read::default());
        let follows = [BankFollow { id: "acc-chq".into(), follow: true, into: None }, BankFollow { id: "acc-visa".into(), follow: true, into: None }];
        let bank = follow(&e, "item-1", Some("2026-11-01"), &follows).unwrap();
        assert!(bank.accounts.iter().all(|a| a.follow && a.into.is_some()));
        let read = read_all(&e).unwrap();
        assert_eq!((read.banks, read.written, read.failed), (1, 2, 0));
        let accounts = crate::handlers::money::ledger(&e, |l| l.accounts()).unwrap();
        let visa = accounts.iter().find(|a| a.name == "Visa Desjardins").unwrap();
        assert_eq!((visa.r#type, visa.balance), (relay_money::model::AccountType::Credit, -4000));
        let bank = &list(&e).unwrap()[0];
        assert!(bank.last_sync.is_some() && bank.problem.is_none());
        // The token is sealed on disk.
        let sealed: Vec<u8> = store(&e, |st| Ok(st.conn.query_row("SELECT token FROM banks", [], |r| r.get(0)).unwrap())).unwrap();
        assert!(!sealed.windows(8).any(|w| w == b"access-1"));
        assert_eq!(Config::for_tests().open(&sealed).unwrap(), "access-1");
        remove(&e, "item-1").unwrap();
        assert!(list(&e).unwrap().is_empty());
        assert!(calls.lock().unwrap().contains(&"/item/remove".to_string()));
        assert_eq!(crate::handlers::money::ledger(&e, |l| l.accounts()).unwrap().len(), 2, "the ledger keeps what was written");
    }

    #[test]
    fn a_hosted_sign_in_connects_once() {
        let e = engine();
        let (f, calls) = fake(vec![]);
        set_fake(&e, f);
        let (token, url, expires) = hosted_link(&e, "Tally", Some("tallyrs://banks")).unwrap();
        assert_eq!((token.as_str(), url.as_str()), ("link-hosted", "https://hosted.plaid.com/link/abc"));
        assert!(expires.is_some());
        let (state, bank, _) = link_status(&e, &token).unwrap();
        assert_eq!((state.as_str(), bank.map(|b| b.id)), ("connected", Some("item-1".to_string())));
        let (state, bank, _) = link_status(&e, &token).unwrap();
        assert_eq!((state.as_str(), bank.map(|b| b.institution)), ("connected", Some("Desjardins".to_string())));
        assert_eq!(calls.lock().unwrap().iter().filter(|c| *c == "/item/public_token/exchange").count(), 1);
        assert_eq!(link_status(&e, "someone-elses").unwrap_err().code, "money.not_found");
        assert_eq!(hosted_link(&e, "Tally", Some("javascript://x")).unwrap_err().code, "money.invalid");
    }

    #[test]
    fn a_login_that_expired_is_the_banks_problem_and_others_go_on() {
        let e = engine();
        let (f, _) = fake(vec![]);
        let failing: Fake = Arc::new(move |path: &str, body: &Value| {
            if path == "/transactions/sync" {
                return Err(PlaidError { code: "ITEM_LOGIN_REQUIRED".into(), message: "the login details of this item have changed".into(), display: None });
            }
            f(path, body)
        });
        set_fake(&e, failing);
        exchange(&e, "public-1", Some("ins_desj"), Some("Desjardins")).unwrap();
        follow(&e, "item-1", None, &[BankFollow { id: "acc-chq".into(), follow: true, into: None }]).unwrap();
        assert_eq!(read_all(&e).unwrap().failed, 1);
        assert_eq!(list(&e).unwrap()[0].problem.as_deref(), Some("Sign in to Desjardins again"));
    }

    #[test]
    fn session_outcomes_read_as_plaid_writes_them() {
        assert_eq!(session_outcome(&json!({"link_sessions": []})), Outcome::Waiting);
        assert_eq!(session_outcome(&json!({"link_sessions": [{"on_success": {"public_token": "p", "metadata": {"institution": {"name": "WS", "institution_id": "ins_ws"}}}}]})),
            Outcome::Connected { public_token: "p".into(), institution_id: Some("ins_ws".into()), institution: Some("WS".into()) });
        assert_eq!(session_outcome(&json!({"link_sessions": [{"exit": {"error": {"display_message": "The bank is down"}}}]})), Outcome::Exited(Some("The bank is down".into())));
        assert_eq!(session_outcome(&json!({"link_sessions": [{"exit": {}}]})), Outcome::Exited(None));
    }

    #[test]
    fn a_token_key_is_64_hex_digits_and_a_wrong_one_opens_nothing() {
        assert!(hex32(&"ab".repeat(32)).is_some());
        assert!(hex32("abc").is_none() && hex32(&"zz".repeat(32)).is_none());
        let sealed = Config::for_tests().seal("access").unwrap();
        let mut other = Config::for_tests();
        other.key = [8; 32];
        assert_eq!(other.open(&sealed).unwrap_err().code, "money.plaid_setup");
    }
}
