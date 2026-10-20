//! Arbiter's desk (docs/ARBITER.md): the book, the exchange, the one path every order takes, and
//! the runner that wakes strategies on their bars.
//!
//! Nothing here touches the store or its lock. The book is its own file behind its own mutex,
//! held only for short reads and writes, never across a network call. Placing orders is
//! serialized by a second mutex, [`Desk::trade`], held from the gate's check to the order being
//! recorded, so two sources can never both pass a limit that only one of them fits under.
//!
//! The runner is a plain thread, as `purge::spawn_timer` is: it never goes through the bus, so
//! no audit row lands every pass. It emits `arbiter.changed` when a pass changed something.

use crate::engine::Engine;
use crate::Instance;
use relay_arbiter::book::{self, Book, BookError, NewOrder, NewProposal, Position, StrategyRec};
use relay_arbiter::coinbase::{Coinbase, Credentials};
use relay_arbiter::exchange::{self, Account, ExchangeError, Market};
use relay_arbiter::gate::{self, Intent, Refusal, Situation};
use relay_arbiter::model::*;
use relay_arbiter::views::{Connection, OrderView, ProposalView};
use relay_arbiter::{paper, rules, Decimal};
use relay_bus::BusError;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

/// An exchange: its market data and the person's account on it.
pub trait Exchange: Market + Account {}
impl<T: Market + Account> Exchange for T {}

/// How often the runner looks, and how long a quote is reused.
const PASS: Duration = Duration::from_secs(20);
/// How long a failed exit waits before it is tried again, in seconds.
const EXIT_RETRY: i64 = 60;
const QUOTE_TTL: i64 = 10;
/// The product list is re-read hourly; sizes and increments rarely change.
const PRODUCTS_TTL: i64 = 3600;
/// Balances and fees are re-read this often while anything is live.
const ACCOUNT_TTL: i64 = 300;
/// A keyring that could not be read is asked again after this, seconds: often enough to pick
/// the key up once it is unlocked, not so often that a locked one prompts or times out each pass.
const KEY_RETRY: i64 = 60;
/// A scheduled buy missed by more than this (the app was closed) is skipped, not caught up.
const SCHEDULE_GRACE: i64 = 3600;
/// An order sent with no answer is sent again for this long, seconds; once Coinbase still has no
/// order with its id after it, it was never placed.
const UNANSWERED: i64 = 60;
/// How far before an order's own time a lookup by its id starts: the PC's clock and Coinbase's
/// may differ.
const LOOKUP_SLACK: i64 = 300;
/// How long an order Coinbase calls done waits for the rest of its fills, seconds, before it is
/// closed with what the book has: far past any lag in Coinbase's fill list.
const FILLS_WAIT: i64 = 600;
/// An order sent less than this long ago may still be waiting for create's answer (preview and
/// create may each take the client's 15-second timeout): the runner leaves it to the call that
/// sent it.
const CREATE_WAIT: i64 = 30;

#[derive(Default)]
pub struct Desk {
    book: Mutex<Option<Book>>,
    /// The client for the saved key: `None` not yet loaded, `Some(None)` no key.
    keyed: Mutex<Option<Option<Arc<dyn Exchange>>>>,
    /// When the keyring last failed to answer, Unix seconds: nothing is cached for it.
    key_failed_at: Mutex<Option<i64>>,
    public: Mutex<Option<Arc<dyn Exchange>>>,
    /// Tests put a fake exchange here; it then serves both the market and the account.
    fake: Mutex<Option<Arc<dyn Exchange>>>,
    /// Tests: the fake serves no account, as when the key cannot be read.
    fake_keyless: Mutex<bool>,
    pub(crate) trade: Mutex<()>,
    quotes: Mutex<HashMap<String, Quote>>,
    /// Each product's price from the last product list: what [`last_price`] falls back on. Kept
    /// here, not read from the book, so it can be asked while the book is held.
    listed: Mutex<HashMap<String, f64>>,
    account_at: Mutex<i64>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn bus(e: BookError) -> BusError {
    match e {
        BookError::Invalid(m) => BusError::invalid("arbiter.invalid", m),
        BookError::NotFound(m) => BusError::not_found("arbiter.not_found", m),
        BookError::Sql(e) => BusError::internal(format!("arbiter book: {e}")),
    }
}

pub fn ex(e: ExchangeError) -> BusError {
    match e {
        ExchangeError::Auth(m) => BusError::refused("arbiter.key_refused", m).with_hint("check the key in Arbiter's settings"),
        ExchangeError::Refused(m) => BusError::refused("arbiter.exchange_refused", m),
        ExchangeError::RateLimited => BusError::unavailable("arbiter.rate_limited", "Coinbase asked to slow down; try again in a moment"),
        ExchangeError::Unreachable(m) => BusError::unavailable("arbiter.unreachable", format!("Coinbase could not be reached: {m}")),
        ExchangeError::Malformed(m) => BusError::unavailable("arbiter.malformed", format!("Coinbase's answer could not be read: {m}")),
        ExchangeError::Duplicate(m) => BusError::conflict("arbiter.duplicate_order", format!("Coinbase already has this order: {m}")),
    }
}

/// Runs `f` on the book, opening `arbiter.db` beside the store on first use (in memory when the
/// store is, as in tests). Keep `f` short: no network inside.
pub fn with_book<T>(engine: &Engine, f: impl FnOnce(&mut Book) -> book::Result<T>) -> Result<T, BusError> {
    let mut slot = lock(&engine.arbiter.book);
    if slot.is_none() {
        let store = engine.store.path();
        let opened = if store == Path::new(":memory:") { Book::open_in_memory() } else {
            let path = store.with_file_name("arbiter.db");
            Book::open(&path).inspect(|_| crate::private::private_db(&path))
        };
        *slot = Some(opened.map_err(bus)?);
    }
    f(slot.as_mut().expect("opened above")).map_err(bus)
}

/// Serve every exchange call from `fake` (tests).
pub fn set_fake_exchange(engine: &Engine, fake: Arc<dyn Exchange>) {
    *lock(&engine.arbiter.fake) = Some(fake);
}

/// Tests: whether the fake exchange serves the account too, as a key that can be read does.
pub fn set_fake_key(engine: &Engine, readable: bool) {
    *lock(&engine.arbiter.fake_keyless) = !readable;
}

/// Public market data: needs no key.
pub fn market(engine: &Engine) -> Arc<dyn Exchange> {
    if let Some(f) = lock(&engine.arbiter.fake).clone() {
        return f;
    }
    lock(&engine.arbiter.public).get_or_insert_with(|| Arc::new(Coinbase::public())).clone()
}

// ------------------------------------------------------------------ the key

fn in_memory(engine: &Engine) -> bool {
    engine.store.path() == Path::new(":memory:")
}

fn test_key_file(engine: &Engine) -> PathBuf {
    engine.store.path().with_file_name("arbiter-key.json")
}

const STORE_KEY: &str = r#"printf %s "$RELAY_ARBITER_KEY" | secret-tool store --label="Relay Arbiter: Coinbase key" application Relay purpose arbiter-coinbase"#;

/// Keeps the key in the Linux Secret Service, as Relay's Android signing passwords are; the test
/// instance keeps it in a private file beside its store instead.
fn store_key(engine: &Engine, c: &Credentials) -> Result<(), BusError> {
    let body = json!({"key_name": c.key_name, "private_key": c.private_key_pem}).to_string();
    if in_memory(engine) {
        return Ok(());
    }
    if engine.instance == Instance::Test {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(test_key_file(engine))
            .map_err(|e| BusError::unavailable("arbiter.keyring", e.to_string()))?;
        return f.write_all(body.as_bytes()).map_err(|e| BusError::unavailable("arbiter.keyring", e.to_string()));
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", STORE_KEY]).env("RELAY_ARBITER_KEY", body);
    keyring(&mut cmd).map(|_| ())
}

fn keyring(cmd: &mut Command) -> Result<String, BusError> {
    let out = crate::proc::output_with_timeout(cmd, Duration::from_secs(30))
        .map_err(|e| BusError::unavailable("arbiter.keyring", e.to_string()).with_hint("install libsecret (secret-tool) and unlock your desktop keyring"))?
        .ok_or_else(|| BusError::unavailable("arbiter.keyring", "The system keyring did not answer within 30 seconds"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(BusError::unavailable("arbiter.keyring", String::from_utf8_lossy(&out.stderr).trim().to_string())
            .with_hint("install libsecret (secret-tool) and unlock your desktop keyring"))
    }
}

/// What a keyring lookup answered: the secret, `None` when none is stored (`secret-tool` exits 1
/// and says nothing), or why the keyring could not be read (it says so on stderr). A silent exit
/// is also how a dismissed unlock prompt can end, so while the book still records a key
/// (`key_saved`) it is a failure, asked again later, not "no key" kept until Relay restarts.
fn looked_up(out: &std::process::Output, key_saved: bool) -> Result<Option<String>, BusError> {
    if out.status.success() {
        return Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()));
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.stdout.is_empty() && err.is_empty() {
        if key_saved {
            return Err(BusError::unavailable("arbiter.keyring", "The keyring did not give back the Coinbase key Arbiter saved; it may be locked").with_hint("unlock your desktop keyring"));
        }
        return Ok(None);
    }
    Err(BusError::unavailable("arbiter.keyring", err).with_hint("unlock your desktop keyring"))
}

/// The saved key: `None` when none is saved (or what is saved cannot be a key), an error when the
/// keyring could not be read, which says nothing about whether a key is saved.
fn load_key(engine: &Engine) -> Result<Option<Credentials>, BusError> {
    if in_memory(engine) {
        return Ok(None);
    }
    let body = if engine.instance == Instance::Test {
        match std::fs::read_to_string(test_key_file(engine)) {
            Ok(body) => body,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(BusError::unavailable("arbiter.keyring", e.to_string())),
        }
    } else {
        let mut cmd = Command::new("secret-tool");
        cmd.args(["lookup", "application", "Relay", "purpose", "arbiter-coinbase"]);
        let key_saved = with_book(engine, |b| b.connection()).is_ok_and(|c| !matches!(c.state.as_str(), "none" | ""));
        match crate::proc::output_with_timeout(&mut cmd, Duration::from_secs(30)) {
            Ok(Some(out)) => match looked_up(&out, key_saved)? {
                Some(body) => body,
                None => return Ok(None),
            },
            Ok(None) => return Err(BusError::unavailable("arbiter.keyring", "The system keyring did not answer within 30 seconds")),
            // Without the tool no key can have been saved: `store_key` uses it too.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(BusError::unavailable("arbiter.keyring", e.to_string())),
        }
    };
    let parse = || {
        let v: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
        Credentials::parse(v["key_name"].as_str()?, v["private_key"].as_str()?).ok()
    };
    Ok(parse())
}

pub fn forget_key(engine: &Engine) -> Result<(), BusError> {
    *lock(&engine.arbiter.keyed) = Some(None);
    if in_memory(engine) {
        return Ok(());
    }
    if engine.instance == Instance::Test {
        let _ = std::fs::remove_file(test_key_file(engine));
        return Ok(());
    }
    let mut cmd = Command::new("secret-tool");
    cmd.args(["clear", "application", "Relay", "purpose", "arbiter-coinbase"]);
    keyring(&mut cmd).map(|_| ())
}

/// The account client for the saved key, loaded from the keyring once it answers: a key saved,
/// or none, is kept; a keyring that could not be read (locked, or the lookup timing out) is asked
/// again on a call [`KEY_RETRY`] later.
pub fn account(engine: &Engine) -> Option<Arc<dyn Exchange>> {
    account_asking(engine, false)
}

/// [`account`], asking the keyring now even within [`KEY_RETRY`] of a failure: for what the
/// person starts (the kill switch, Refresh), which must not wait out a keyring they just unlocked.
pub fn account_now(engine: &Engine) -> Option<Arc<dyn Exchange>> {
    account_asking(engine, true)
}

fn account_asking(engine: &Engine, now: bool) -> Option<Arc<dyn Exchange>> {
    if let Some(f) = lock(&engine.arbiter.fake).clone() {
        return (!*lock(&engine.arbiter.fake_keyless)).then_some(f);
    }
    let mut slot = lock(&engine.arbiter.keyed);
    if slot.is_none() {
        let mut failed = lock(&engine.arbiter.key_failed_at);
        if !now && failed.is_some_and(|at| book::now() - at < KEY_RETRY) {
            return None;
        }
        match load_key(engine) {
            Ok(creds) => {
                *slot = Some(creds.and_then(|c| Coinbase::with_key(&c).ok()).map(|c| Arc::new(c) as Arc<dyn Exchange>));
                *failed = None;
            }
            Err(_) => {
                *failed = Some(book::now());
                return None;
            }
        }
    }
    slot.clone().flatten()
}

fn mask(key_name: &str) -> String {
    match key_name.rsplit_once('/') {
        Some((_, id)) => format!("organizations/…/apiKeys/{}…", id.chars().take(4).collect::<String>()),
        None => format!("{}…", key_name.chars().take(4).collect::<String>()),
    }
}

/// Checks a key with Coinbase and keeps it. Refuses a key that can move money out, or cannot
/// even read the account. Slow: call it with nothing locked.
pub fn set_key(engine: &Engine, key_name: &str, private_key: &str) -> Result<Connection, BusError> {
    let creds = Credentials::parse(key_name, private_key).map_err(|m| BusError::invalid("arbiter.bad_key", m))?;
    let client: Arc<dyn Exchange> = match lock(&engine.arbiter.fake).clone() {
        Some(f) => f,
        None => Arc::new(Coinbase::with_key(&creds).map_err(|m| BusError::invalid("arbiter.bad_key", m))?),
    };
    let perms = client.permissions().map_err(ex)?;
    if perms.can_transfer {
        return Err(BusError::refused("arbiter.key_can_transfer", "This key can transfer money out of the account. Arbiter refuses it: make a key with View and Trade only.")
            .with_hint("in the Coinbase Developer Platform, create a new key and leave Transfer off"));
    }
    if !perms.can_view {
        return Err(BusError::refused("arbiter.key_cannot_view", "This key cannot read the account. Give it the View permission."));
    }
    let fees = client.fees().unwrap_or_default();
    store_key(engine, &creds)?;
    *lock(&engine.arbiter.keyed) = Some(Some(client.clone()));
    let conn = Connection {
        state: "ok".into(),
        exchange: "Coinbase".into(),
        key: Some(mask(&creds.key_name)),
        permissions: Some(perms),
        fees,
        checked_at: Some(book::rfc3339(book::now())),
        error: None,
    };
    with_book(engine, |b| {
        b.set_connection(&conn)?;
        b.log(None, "setting", &format!("Connected Coinbase key {}.", conn.key.as_deref().unwrap_or("")), None)?;
        Ok(())
    })?;
    // Balances on the next refresh, which the caller runs now.
    *lock(&engine.arbiter.account_at) = 0;
    let _ = refresh_account(engine, true);
    with_book(engine, |b| b.connection())
}

/// Reads permissions, fees and balances, and values the balances in the home currency. `force`
/// is the person asking (Refresh, a key just set): the keyring is asked now, too.
pub fn refresh_account(engine: &Engine, force: bool) -> Result<Connection, BusError> {
    let now = book::now();
    {
        let mut at = lock(&engine.arbiter.account_at);
        if !force && now - *at < ACCOUNT_TTL {
            return with_book(engine, |b| b.connection());
        }
        *at = now;
    }
    let Some(client) = account_asking(engine, force) else {
        return with_book(engine, |b| b.connection());
    };
    let mut conn = with_book(engine, |b| b.connection())?;
    conn.exchange = "Coinbase".into();
    let read = (|| -> exchange::Result<(Permissions, Fees, Vec<Balance>)> { Ok((client.permissions()?, client.fees()?, client.balances()?)) })();
    match read {
        Ok((perms, fees, mut balances)) => {
            let home = with_book(engine, |b| b.home())?;
            for bal in &mut balances {
                let total = bal.available + bal.hold;
                bal.value = if bal.currency == home {
                    Some(total)
                } else {
                    quote(engine, &format!("{}-{home}", bal.currency)).ok().and_then(|q| Decimal::from_f64(q.bid)).map(|p| (total * p).round_dp(2))
                };
            }
            if perms.can_transfer {
                conn.state = "error".into();
                conn.error = Some("This key can now transfer money out; Arbiter will not use it. Replace it with a View and Trade key.".into());
            } else {
                conn.state = "ok".into();
                conn.error = None;
            }
            conn.permissions = Some(perms);
            conn.fees = fees;
            conn.checked_at = Some(book::rfc3339(now));
            let value: f64 = balances.iter().filter_map(|b| b.value).filter_map(|v| v.to_f64()).sum();
            with_book(engine, |b| {
                b.set_balances(&balances)?;
                b.set_connection(&conn)?;
                b.snapshot(Venue::Live, now, value)
            })?;
        }
        Err(e) => {
            conn.state = "error".into();
            conn.error = Some(e.to_string());
            conn.checked_at = Some(book::rfc3339(now));
            with_book(engine, |b| b.set_connection(&conn))?;
        }
    }
    Ok(conn)
}

/// Whether live trading may run: a key, checked, that can trade and cannot transfer.
fn live_ready(engine: &Engine) -> Result<(), String> {
    let c = with_book(engine, |b| b.connection()).map_err(|e| e.message)?;
    match (&c.state[..], &c.permissions) {
        ("ok", Some(p)) if p.can_trade && !p.can_transfer => Ok(()),
        ("ok", Some(p)) if !p.can_trade => Err("The Coinbase key cannot trade: give it the Trade permission".into()),
        ("none", _) | ("", _) => Err("Connect a Coinbase key before going live".into()),
        _ => Err(c.error.unwrap_or_else(|| "The Coinbase key is not working".into())),
    }
}

pub fn check_live(engine: &Engine) -> Result<(), BusError> {
    live_ready(engine).map_err(|m| BusError::refused("arbiter.not_connected", m))
}

// ------------------------------------------------------------------ market data

pub fn products(engine: &Engine) -> Result<Vec<Product>, BusError> {
    let list = match with_book(engine, |b| b.cached_products(PRODUCTS_TTL))? {
        Some(p) if !p.is_empty() => p,
        _ => match market(engine).products() {
            Ok(list) => {
                with_book(engine, |b| b.store_products(&list))?;
                list
            }
            // Stale is better than nothing when the exchange is down.
            Err(e) => match with_book(engine, |b| b.cached_products(i64::MAX))? {
                Some(p) if !p.is_empty() => p,
                _ => return Err(ex(e)),
            },
        },
    };
    let mut listed = lock(&engine.arbiter.listed);
    for p in &list {
        if let Some(price) = p.price {
            listed.insert(p.id.clone(), price);
        }
    }
    Ok(list)
}

/// The product as the exchange lists it, or `None` when it does not exist (an invented symbol).
pub fn product(engine: &Engine, id: &str) -> Result<Option<Product>, BusError> {
    if !relay_arbiter::coinbase::product_id_ok(id) {
        return Ok(None);
    }
    let list = products(engine)?;
    Ok(list.into_iter().find(|p| p.id == id))
}

pub fn quote(engine: &Engine, product: &str) -> Result<Quote, BusError> {
    if let Some(q) = lock(&engine.arbiter.quotes).get(product).copied() {
        if book::now() - q.at <= QUOTE_TTL {
            return Ok(q);
        }
    }
    fresh_quote(engine, product)
}

/// A quote read from the exchange now: what the runner decides on.
pub fn fresh_quote(engine: &Engine, product: &str) -> Result<Quote, BusError> {
    let now = book::now();
    let mut q = market(engine).quote(product).map_err(ex)?;
    // The book's own time can lag; what matters to staleness is when it was read.
    q.at = q.at.max(now - 1).min(now);
    lock(&engine.arbiter.quotes).insert(product.to_string(), q);
    Ok(q)
}

/// The last price known for a product, without asking the exchange or taking the book (callers
/// hold it): the runner's quote, else the product list's.
pub fn last_price(engine: &Engine, product: &str) -> Option<f64> {
    if let Some(q) = lock(&engine.arbiter.quotes).get(product) {
        if book::now() - q.at <= 300 {
            return Some(q.mid());
        }
    }
    lock(&engine.arbiter.listed).get(product).copied()
}

/// Candles for `[start, end)`, from the cache where it has them.
pub fn candles(engine: &Engine, product: &str, g: Granularity, start: i64, end: i64) -> Result<Vec<Candle>, BusError> {
    let step = g.seconds();
    let start = start.div_euclid(step) * step;
    let cached = with_book(engine, |b| b.cached_candles(product, g, start, end))?;
    let have_to = cached.last().map(|c| c.start + step);
    let complete_from_start = cached.first().is_some_and(|c| c.start <= start + step);
    let fetch_from = if complete_from_start { have_to.unwrap_or(start) } else { start };
    if fetch_from >= end {
        return Ok(cached);
    }
    let fresh = exchange::candles_between(market(engine).as_ref(), product, g, fetch_from, end).map_err(ex)?;
    with_book(engine, |b| b.store_candles(product, g, &fresh))?;
    let mut all: Vec<Candle> = cached.into_iter().filter(|c| c.start < fetch_from).collect();
    all.extend(fresh);
    all.sort_by_key(|c| c.start);
    all.dedup_by_key(|c| c.start);
    Ok(all)
}

// ------------------------------------------------------------------ the order path

/// What happened to an intent.
pub enum Outcome {
    Placed(Box<OrderView>),
    Proposed(Box<ProposalView>),
    Refused(Refusal),
}

fn fees(engine: &Engine) -> Fees {
    with_book(engine, |b| b.connection()).map(|c| c.fees).unwrap_or_default()
}

/// Everything the gate weighs for `s`, read now.
fn situation_for<'a>(engine: &Engine, s: &'a StrategyRec, product: Option<&'a Product>, q: Option<Quote>, global: &'a GlobalLimits) -> Result<(Situation<'a>, Position), BusError> {
    let now = book::now();
    let taker = fees(engine).taker;
    let (pos, exposure, loss_all, n, n_all, halted_all) = with_book(engine, |b| {
        let pos = b.position(s.id, s.venue)?;
        let mut exposure = Decimal::ZERO;
        for other in b.strategies()?.iter().filter(|o| o.venue == s.venue) {
            exposure += b.position(other.id, s.venue)?.cost;
        }
        // Holdings come from fills, and a market buy is usually still pending when it is placed:
        // two strategies due in the same pass would each pass the Arbiter-wide limit alone. What
        // other strategies' open buys will still spend counts too (this one's own open order
        // already stops it placing another).
        for o in b.open_orders(None)?.iter().filter(|o| o.venue == s.venue && o.side == Side::Buy && o.strategy_id != Some(s.id)) {
            exposure += still_to_spend(o, last_price(engine, &o.product));
        }
        let n = b.orders_since(Some(s.id), s.venue, now - 3600)?;
        let n_all = b.orders_since(None, s.venue, now - 3600)?;
        Ok((pos, exposure, venue_loss(engine, b, s.venue, taker)?, n, n_all, b.halted()?.is_some()))
    })?;
    let price = q.map(|q| q.bid);
    let loss = (-(pos.realized_today + pos.open_pnl(price, taker).min(Decimal::ZERO))).max(Decimal::ZERO);
    let sit = Situation {
        product,
        quote: q,
        now,
        limits: &s.limits,
        global,
        held_base: pos.base,
        held_cost: pos.cost,
        exposure,
        loss_today: loss,
        loss_today_all: loss_all,
        orders_last_hour: n,
        orders_last_hour_all: n_all,
        last_loss_at: pos.last_loss_at,
        halted: s.state == RunState::Halted,
        all_halted: halted_all,
    };
    Ok((sit, pos))
}

/// What an open buy will still spend: its quote amount, else its size at its limit price, else at
/// `last`; less what has filled, which its strategy's holding already counts.
fn still_to_spend(o: &OrderView, last: Option<f64>) -> Decimal {
    let price = o.limit_price.or_else(|| last.and_then(Decimal::from_f64));
    let whole = match (o.quote_size, o.base_size, price) {
        (Some(q), _, _) => q,
        (None, Some(b), Some(p)) => b * p,
        _ => Decimal::ZERO,
    };
    (whole - o.filled_base * o.average_price.or(price).unwrap_or_default()).max(Decimal::ZERO)
}

/// Runs the gate for an intent on `s`, without placing anything: what a proposal checks first.
fn gate_only(engine: &Engine, s: &StrategyRec, intent: &Intent) -> Result<Result<OrderRequest, Refusal>, BusError> {
    let p = product(engine, &intent.product)?;
    let q = if p.is_some() { quote(engine, &intent.product).ok() } else { None };
    let global = with_book(engine, |b| b.global_limits())?;
    let (sit, _) = situation_for(engine, s, p.as_ref(), q, &global)?;
    let checked = gate::check(intent, &sit);
    Ok(checked)
}

fn describe_order(r: &OrderRequest, venue: Venue) -> String {
    let (base, quote) = r.product.split_once('-').unwrap_or((&r.product, ""));
    let how = if r.limit_price.is_some() { format!(" with a limit at {}", r.limit_price.unwrap_or_default().normalize()) } else { String::new() };
    let what = match (r.side, r.quote_size, r.base_size) {
        (Side::Buy, Some(q), _) => format!("Buy {} {quote} of {base}", q.normalize()),
        (side, _, Some(b)) => format!("{} {} {base}", if side == Side::Buy { "Buy" } else { "Sell" }, b.normalize()),
        _ => format!("{} {base}", if r.side == Side::Buy { "Buy" } else { "Sell" }),
    };
    format!("{what}{how}{}", if venue == Venue::Paper { " on paper" } else { "" })
}

/// The one path an order takes: the gate, the book, then the venue. Holds [`Desk::trade`] from
/// the check to the order being recorded and sent. Slow on a live venue: nothing else locked.
pub fn execute(engine: &Engine, strategy_id: i64, intent: &Intent, why: &str) -> Result<Outcome, BusError> {
    let _turn = lock(&engine.arbiter.trade);
    let s = with_book(engine, |b| b.strategy(strategy_id))?;
    if s.venue == Venue::Live {
        if let Err(m) = live_ready(engine) {
            let r = Refusal { code: "arbiter.not_connected".into(), message: m };
            with_book(engine, |b| b.log(Some(s.id), "refused", &format!("{why} — refused: {}", r.message), None))?;
            return Ok(Outcome::Refused(r));
        }
    }
    // The gate weighs holdings from fills only, so an order still resting is not in them: a
    // second order beside it could pass every size cap twice, or sell what the first already
    // sells. One open order per strategy, as the runner already keeps; the same order asked
    // again still comes back as itself below.
    let cid = gate::client_order_id(&intent.cause);
    let open = with_book(engine, |b| b.open_orders(Some(s.id)))?;
    if !open.is_empty() && open.iter().all(|o| o.client_order_id != cid) {
        let r = Refusal { code: "arbiter.order_open".into(), message: "This strategy has an order open: wait for it, or halt the strategy to cancel it.".into() };
        with_book(engine, |b| b.log(Some(s.id), "refused", &format!("{why} — refused: {}", r.message), None))?;
        return Ok(Outcome::Refused(r));
    }
    let req = match gate_only(engine, &s, intent)? {
        Ok(r) => r,
        Err(r) => {
            with_book(engine, |b| b.log(Some(s.id), "refused", &format!("{why} — refused: {}", r.message), None))?;
            return Ok(Outcome::Refused(r));
        }
    };
    let what = describe_order(&req, s.venue);
    let (id, new) = with_book(engine, |b| b.insert_order(&NewOrder { strategy_id: Some(s.id), venue: s.venue, source: intent.source, request: &req, why }))?;
    if !new {
        // The same decision already placed this order: nothing more to do.
        return Ok(Outcome::Placed(Box::new(with_book(engine, |b| b.order(id))?)));
    }
    with_book(engine, |b| b.log(Some(s.id), req.side.as_str(), &format!("{what}: {why}"), Some(id)))?;
    match s.venue {
        Venue::Paper => {
            let q = quote(engine, &req.product).inspect_err(|e| {
                let _ = with_book(engine, |b| b.fail_order(id, &e.message));
            })?;
            let global = with_book(engine, |b| b.global_limits())?;
            match paper::try_fill(&req, &q, &fees(engine), global.slippage_pct, book::now()) {
                Some((order, fill)) => with_book(engine, |b| {
                    b.update_order(id, &order)?;
                    b.add_fill(id, Venue::Paper, &fill).map(|_| ())
                })?,
                None => with_book(engine, |b| b.update_order(id, &resting(&req)))?,
            }
            if req.side == Side::Buy {
                with_book(engine, |b| b.set_peak(s.id, Some(q.ask)))?;
            }
        }
        Venue::Live => {
            let Some(client) = account(engine) else {
                with_book(engine, |b| b.fail_order(id, "No Coinbase key is saved"))?;
                return Ok(Outcome::Placed(Box::new(with_book(engine, |b| b.order(id))?)));
            };
            match place_live(client.as_ref(), &req) {
                Ok(order) => with_book(engine, |b| b.update_order(id, &order))?,
                // Nothing is known about whether it arrived: create timed out, its answer could not
                // be read, or Coinbase says it has this id already. The order stays pending, and the
                // runner looks it up by its client_order_id before it sends it again or gives up.
                Err((sent, e)) if matches!(e, ExchangeError::Unreachable(_)) || sent && matches!(e, ExchangeError::Malformed(_) | ExchangeError::Duplicate(_)) => {
                    with_book(engine, |b| b.log(Some(s.id), "error", &format!("{what}: no answer from Coinbase ({e}); will look it up."), Some(id)).map(|_| ()))?;
                    if matches!(e, ExchangeError::Duplicate(_)) {
                        remember_duplicate(engine, id, Some(s.id))?;
                    }
                }
                Err((_, e)) => with_book(engine, |b| {
                    b.fail_order(id, &e.to_string())?;
                    b.log(Some(s.id), "error", &format!("{what} failed: {e}"), Some(id)).map(|_| ())
                })?,
            }
            let _ = reconcile_order(engine, id, true);
            if req.side == Side::Buy {
                if let Ok(q) = quote(engine, &req.product) {
                    with_book(engine, |b| b.set_peak(s.id, Some(q.ask)))?;
                }
            }
        }
    }
    Ok(Outcome::Placed(Box::new(with_book(engine, |b| b.order(id))?)))
}

fn resting(req: &OrderRequest) -> ExchangeOrder {
    ExchangeOrder {
        order_id: format!("paper-{}", req.client_order_id),
        client_order_id: req.client_order_id.clone(),
        status: OrderStatus::Open,
        filled_size: Decimal::ZERO,
        average_filled_price: Decimal::ZERO,
        total_fees: Decimal::ZERO,
        reason: None,
    }
}

/// Preview, then place. The preview's refusals stop it before anything is sent. An error says
/// whether create was sent: only then may Coinbase have the order.
fn place_live(client: &dyn Exchange, req: &OrderRequest) -> Result<ExchangeOrder, (bool, ExchangeError)> {
    let preview = client.preview(req).map_err(|e| (false, e))?;
    if !preview.errors.is_empty() {
        return Err((false, ExchangeError::Refused(preview.errors.join("; "))));
    }
    client.place(req, preview.preview_id.as_deref()).map_err(|e| (true, e))
}

fn request_of(o: &OrderView) -> OrderRequest {
    OrderRequest { client_order_id: o.client_order_id.clone(), product: o.product.clone(), side: o.side, quote_size: o.quote_size, base_size: o.base_size, limit_price: o.limit_price }
}

/// When the order was recorded, just before it was sent, Unix seconds.
fn created_at(o: &OrderView) -> i64 {
    jiff::Timestamp::from_str(&o.created_at).map(|t| t.as_second()).unwrap_or(0)
}

/// Coinbase's order sent with this one's client_order_id, if it has one: the lookup for an order
/// whose answer never came back.
fn find_live(client: &dyn Exchange, o: &OrderView) -> Result<Option<ExchangeOrder>, BusError> {
    client.find_order(&o.product, &o.client_order_id, created_at(o) - LOOKUP_SLACK).map_err(ex)
}

/// Coinbase answered a create with "duplicate": it has this order, whatever its order list shows.
/// Kept on the row, so the order is never failed as never placed, and told once.
fn remember_duplicate(engine: &Engine, id: i64, strategy: Option<i64>) -> Result<(), BusError> {
    with_book(engine, |b| {
        if b.mark_exchange_has(id)? {
            b.log(strategy, "error", "Coinbase says it already has this order but does not list it yet: it stays pending until Coinbase shows it. Check the Coinbase app.", Some(id))?;
        }
        Ok(())
    })
}

/// An order sent with no answer read back (create timed out or answered unreadably, or Relay
/// stopped in between): Coinbase may have it or not, so it is looked up first. Found, it is
/// returned, to be reconciled as any known order. Not found, it is sent again with the same
/// client_order_id, which cannot place it twice, for a minute, unless a halt came since (the
/// person's own sell excepted, as the gate excepts it); after that it was never placed and is
/// failed. A lookup that fails leaves it pending, unchanged. One Coinbase called a duplicate is
/// neither sent again nor failed: it exists, and it waits for a lookup to show it.
fn reconcile_unanswered(engine: &Engine, client: &dyn Exchange, o: &OrderView) -> Result<Option<ExchangeOrder>, BusError> {
    if let Some(found) = find_live(client, o)? {
        return Ok(Some(found));
    }
    if with_book(engine, |b| b.exchange_has(o.id))? {
        return Ok(None);
    }
    if book::now() - created_at(o) >= UNANSWERED {
        with_book(engine, |b| {
            b.fail_order(o.id, "Coinbase has no order with this id a minute after it was sent: it was never placed.")?;
            b.log(o.strategy_id, "error", "An order Coinbase never answered is not on Coinbase a minute later: it was never placed.", Some(o.id)).map(|_| ())
        })?;
        return Ok(None);
    }
    let halted = with_book(engine, |b| Ok(b.halted()?.is_some() || o.strategy_id.is_some_and(|sid| b.strategy(sid).is_ok_and(|s| s.state == RunState::Halted))))?;
    if !halted || o.source == Source::Person && o.side == Side::Sell {
        // Any refusal here leaves it pending: a duplicate means Coinbase has it after all, which
        // is kept; anything else is decided by the lookups until the minute ends.
        match client.place(&request_of(o), None) {
            Ok(order) => with_book(engine, |b| b.update_order(o.id, &order))?,
            Err(ExchangeError::Duplicate(_)) => remember_duplicate(engine, o.id, o.strategy_id)?,
            Err(_) => {}
        }
    }
    Ok(None)
}

/// Brings one order up to date with its venue: status and fills. `turn_held` says the caller
/// holds [`Desk::trade`], as `execute` does straight after sending.
pub fn reconcile_order(engine: &Engine, id: i64, turn_held: bool) -> Result<(), BusError> {
    let mut o = with_book(engine, |b| b.order(id))?;
    if o.status.is_done() {
        return Ok(());
    }
    match o.venue {
        Venue::Paper => {
            let Some(sid) = o.strategy_id else { return Ok(()) };
            let req = request_of(&o);
            let q = quote(engine, &o.product)?;
            let global = with_book(engine, |b| b.global_limits())?;
            if let Some((order, fill)) = paper::try_fill(&req, &q, &fees(engine), global.slippage_pct, book::now()) {
                with_book(engine, |b| {
                    b.update_order(id, &order)?;
                    b.add_fill(id, Venue::Paper, &fill)?;
                    b.log(Some(sid), "fill", &format!("{} filled at {}.", describe_order(&req, Venue::Paper), fill.price.round_dp(2).normalize()), Some(id)).map(|_| ())
                })?;
            } else if stale_limit(engine, sid, &o) {
                cancel_one(engine, &o, "it did not fill within a bar")?;
            }
        }
        Venue::Live => {
            let Some(client) = account(engine) else { return Ok(()) };
            // An order with no answer is decided in the trade turn, as the call that sent it and
            // the kill switch are: a resend is never in flight while a halt looks the order up and
            // closes it, nor sent while the first create still waits for its answer. It is read
            // again in the turn, which the call that sent it may have answered meanwhile.
            let unanswered = o.exchange_order_id.is_none() && !turn_held;
            if unanswered && book::now() - created_at(&o) < CREATE_WAIT {
                return Ok(());
            }
            let _turn = unanswered.then(|| lock(&engine.arbiter.trade));
            if unanswered {
                o = with_book(engine, |b| b.order(id))?;
                if o.status.is_done() {
                    return Ok(());
                }
            }
            let order = match o.exchange_order_id.clone() {
                Some(eid) => client.order(&eid).map_err(ex)?,
                None => match reconcile_unanswered(engine, client.as_ref(), &o)? {
                    Some(found) => found,
                    None => return Ok(()),
                },
            };
            let eid = order.order_id.clone();
            book_live_fills(engine, client.as_ref(), &o, &eid, &order)?;
            let done_there = order.status.is_done();
            let order = settled(engine, &o, order)?;
            with_book(engine, |b| b.update_order(id, &order))?;
            if !done_there {
                if let Some(sid) = o.strategy_id {
                    if stale_limit(engine, sid, &o) {
                        // Read again: an order just found by its id has that id now.
                        cancel_one(engine, &with_book(engine, |b| b.order(id))?, "it did not fill within a bar")?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// A limit order that has rested longer than its strategy's bar.
fn stale_limit(engine: &Engine, sid: i64, o: &OrderView) -> bool {
    if o.limit_price.is_none() {
        return false;
    }
    let bar = with_book(engine, |b| b.strategy(sid)).map(|s| s.granularity.seconds()).unwrap_or(3600);
    book::now() - created_at(o) > bar
}

/// What a live order's record says once its fills are booked: the exchange's answer, except that
/// an order the exchange calls done with fills still missing from the book stays open, so the
/// next pass books the rest (fills can lag the order's status, and a done order is never read
/// again). A booked fill is rounded to the base increment, so a shortfall within one increment a
/// fill is rounding, not a fill to wait for. The wait is bounded: [`FILLS_WAIT`] after Coinbase
/// first called it done, a shortfall is not lag (a size reported with less precision, a
/// correction, an increment that changed), and an order held open for good would stop its
/// strategy's exits and every switch. It closes with what the book has, and says so.
fn settled(engine: &Engine, o: &OrderView, mut order: ExchangeOrder) -> Result<ExchangeOrder, BusError> {
    if !order.status.is_done() || order.filled_size <= Decimal::ZERO {
        return Ok(order);
    }
    let now = book::now();
    let (booked, since) = with_book(engine, |b| {
        let (booked, n) = b.booked_fills(o.id)?;
        let step = b.cached_product(&o.product)?.map(|p| p.base_increment).filter(|i| *i > Decimal::ZERO).unwrap_or(Decimal::new(1, 8));
        let short = order.filled_size - booked > step * Decimal::from(n);
        Ok((booked, if short { Some(b.fills_short_since(o.id, now)?) } else { None }))
    })?;
    match since {
        None => {}
        Some(at) if now - at < FILLS_WAIT => order.status = OrderStatus::Open,
        Some(_) => {
            let base = o.product.split_once('-').map_or(o.product.as_str(), |(b, _)| b);
            let text = format!(
                "Coinbase reports {} {base} filled, but lists fills for only {}: closed with what was booked after waiting {} minutes for the rest. Check the Coinbase app.",
                order.filled_size.normalize(), booked.normalize(), FILLS_WAIT / 60
            );
            with_book(engine, |b| b.log(o.strategy_id, "error", &text, Some(o.id)))?;
            order.reason = Some(text);
        }
    }
    Ok(order)
}

/// Books a live order's fills the exchange reports and the book does not have yet.
fn book_live_fills(engine: &Engine, client: &dyn Exchange, o: &OrderView, eid: &str, order: &ExchangeOrder) -> Result<(), BusError> {
    if order.filled_size <= Decimal::ZERO {
        return Ok(());
    }
    for f in client.fills(eid).map_err(ex)? {
        let added = with_book(engine, |b| b.add_fill(o.id, Venue::Live, &f))?;
        if added.is_some() {
            with_book(engine, |b| b.log(o.strategy_id, "fill", &format!("{} {} {} filled at {}.", if o.side == Side::Buy { "Bought" } else { "Sold" }, f.size.normalize(), o.product, f.price.round_dp(2).normalize()), Some(o.id)).map(|_| ()))?;
        }
    }
    Ok(())
}

/// Cancels an order. A live one is cancelled on Coinbase or not at all: without the key, or with
/// Coinbase not answering, it stays open and the error says so (the kill switch lists it). Only
/// an order Coinbase says it has no record of is cancelled here alone.
fn cancel_one(engine: &Engine, o: &OrderView, why: &str) -> Result<(), BusError> {
    if o.venue == Venue::Live {
        let client = account(engine).ok_or_else(|| {
            BusError::unavailable("arbiter.no_key", "The Coinbase key could not be read, so the order was not cancelled on Coinbase; it stays open")
                .with_hint("unlock your desktop keyring, then halt again")
        })?;
        let eid = match o.exchange_order_id.clone() {
            Some(eid) => eid,
            // Sent with no answer read back: Coinbase may have it under its client_order_id.
            None => match find_live(client.as_ref(), o)? {
                Some(found) => found.order_id,
                // Coinbase may not list an order it took moments ago, so within the minute "not
                // listed" says nothing: it stays pending, and the runner, which does not send it
                // again while halted, settles it once the minute is up.
                None if book::now() - created_at(o) < UNANSWERED => {
                    return Err(BusError::unavailable("arbiter.unconfirmed", "Coinbase does not list this order yet, so it was not cancelled: it stays pending, is not sent again while halted, and is settled within a minute"));
                }
                None if with_book(engine, |b| b.exchange_has(o.id))? => {
                    return Err(BusError::unavailable("arbiter.unconfirmed", "Coinbase says it has this order but does not list it, so it could not be cancelled: cancel it in the Coinbase app"));
                }
                None => return cancel_here(engine, o, &format!("{why} (Coinbase has no record of it)")),
            },
        };
        client.cancel(std::slice::from_ref(&eid)).map_err(ex)?;
        // A cancel races the order's fills: it may have filled, wholly or in part, since the
        // last pass, and the exchange answers a filled order's cancel as done. Book what the
        // exchange says it did; a cancel still queued stays open for the next pass.
        let order = client.order(&eid).map_err(ex)?;
        book_live_fills(engine, client.as_ref(), o, &eid, &order)?;
        let mut order = settled(engine, o, order)?;
        if order.status == OrderStatus::Cancelled && order.reason.is_none() {
            order.reason = Some(why.to_string());
        }
        return with_book(engine, |b| {
            b.update_order(o.id, &order)?;
            b.log(o.strategy_id, "cancel", &format!("Cancelled an order: {why}."), Some(o.id)).map(|_| ())
        });
    }
    cancel_here(engine, o, why)
}

/// Marks an order cancelled in the book only: a paper order, or a live one Coinbase never had.
fn cancel_here(engine: &Engine, o: &OrderView, why: &str) -> Result<(), BusError> {
    let x = ExchangeOrder {
        order_id: o.exchange_order_id.clone().unwrap_or_default(),
        client_order_id: o.client_order_id.clone(),
        status: OrderStatus::Cancelled,
        filled_size: o.filled_base,
        average_filled_price: o.average_price.unwrap_or_default(),
        total_fees: o.fees,
        reason: Some(why.to_string()),
    };
    with_book(engine, |b| {
        b.update_order(o.id, &x)?;
        b.log(o.strategy_id, "cancel", &format!("Cancelled an order: {why}."), Some(o.id)).map(|_| ())
    })
}

/// Routes an intent by the strategy's mode: placed, proposed, or refused. A rule's intent is
/// placed in "rules" and "agent" modes; an agent's only in "agent" mode; in "ask" everything but
/// the person's own waits for the person. What would be refused is refused now, not proposed.
pub fn route(engine: &Engine, strategy_id: i64, intent: &Intent, why: &str, thread_id: Option<i64>) -> Result<Outcome, BusError> {
    let s = with_book(engine, |b| b.strategy(strategy_id))?;
    let asks = match intent.source {
        Source::Person => false,
        Source::Rule => s.mode == Mode::Ask,
        Source::Agent => s.mode != Mode::Agent,
    };
    if !asks {
        return execute(engine, strategy_id, intent, why);
    }
    if let Some(existing) = with_book(engine, |b| b.proposal_for_cause(&intent.cause))? {
        return Ok(Outcome::Proposed(Box::new(existing)));
    }
    let req = match gate_only(engine, &s, intent)? {
        Ok(r) => r,
        Err(r) => {
            with_book(engine, |b| b.log(Some(s.id), "refused", &format!("{why} — refused: {}", r.message), None))?;
            return Ok(Outcome::Refused(r));
        }
    };
    let minutes = with_book(engine, |b| b.global_limits())?.approval_minutes;
    let p = NewProposal {
        kind: "order".into(),
        strategy_id: Some(s.id),
        title: describe_order(&req, s.venue),
        why: why.to_string(),
        source: Some(intent.source),
        side: Some(req.side),
        product: Some(req.product.clone()),
        quote: intent.quote,
        base: req.base_size.filter(|_| req.side == Side::Sell),
        limit: intent.limit,
        venue: Some(s.venue),
        thread_id,
        cause: Some(intent.cause.clone()),
        expires_at: Some(book::now() + i64::from(minutes) * 60),
        ..Default::default()
    };
    let id = with_book(engine, |b| b.add_proposal(&p))?;
    Ok(Outcome::Proposed(Box::new(with_book(engine, |b| b.proposal(id))?.0)))
}

/// Approves or dismisses a proposal. An approved order passes the gate again, now, and only on
/// the venue and product it was asked for; an approved change only onto the strategy it was made
/// from. A card past its time is expired, whether or not the runner has been by since.
pub fn resolve(engine: &Engine, id: i64, approve: bool, draft_hash: Option<&str>) -> Result<ProposalView, BusError> {
    with_book(engine, |b| b.expire_proposals(book::now()))?;
    let (p, cause) = with_book(engine, |b| b.proposal(id))?;
    if p.status != "pending" {
        return Err(BusError::conflict("arbiter.answered", format!("That proposal is already {}", p.status)));
    }
    if !approve {
        with_book(engine, |b| b.resolve_proposal(id, "dismissed", None))?;
        return Ok(with_book(engine, |b| b.proposal(id))?.0);
    }
    match p.kind.as_str() {
        "order" => {
            let sid = p.strategy_id.ok_or_else(|| BusError::invalid("arbiter.invalid", "The proposal names no strategy"))?;
            // The card said which venue it was for; `execute` uses the strategy's venue now. A
            // card asked on paper must never become a real order because the strategy went live.
            // Nor may a card for one product be placed on a strategy that trades another now: the
            // gate checks the product against the card's own.
            let s = with_book(engine, |b| b.strategy(sid))?;
            let changed = if p.venue.is_some_and(|v| v != s.venue) {
                Some(format!("The strategy has switched to {} since this was asked; nothing was placed.", s.venue.as_str()))
            } else if p.product.as_deref().is_some_and(|x| x != s.product) {
                Some(format!("The strategy trades {} now; nothing was placed.", s.product))
            } else {
                None
            };
            if let Some(text) = changed {
                with_book(engine, |b| b.resolve_proposal(id, "failed", Some(&text)))?;
                return Ok(with_book(engine, |b| b.proposal(id))?.0);
            }
            let intent = Intent {
                product: p.product.clone().unwrap_or_default(),
                side: p.side.unwrap_or(Side::Buy),
                quote: p.quote,
                base: p.base,
                limit: p.limit,
                // Approved by the person: placed whatever the strategy's mode.
                source: Source::Person,
                cause: cause.unwrap_or_else(|| format!("proposal:{id}")),
            };
            let outcome = execute(engine, sid, &intent, &format!("Approved: {}", p.why))?;
            let (status, text) = match &outcome {
                Outcome::Placed(o) => ("approved", format!("Order {} is {}.", o.id, o.status.as_str())),
                Outcome::Refused(r) => ("failed", r.message.clone()),
                Outcome::Proposed(_) => ("failed", "It asked again".into()),
            };
            with_book(engine, |b| b.resolve_proposal(id, status, Some(&text)))?;
        }
        _ => {
            let draft = p.draft.clone().ok_or_else(|| BusError::invalid("arbiter.invalid", "The proposal has no strategy in it"))?;
            if draft_hash.is_some_and(|h| Some(h) != p.draft_hash.as_deref()) {
                return Err(BusError::conflict("arbiter.changed_since", "The proposal is not the one shown; reopen it"));
            }
            let result = with_book(engine, |b| match p.strategy_id {
                Some(sid) => {
                    // The draft carries the rule and limits as they were when it was asked: saved
                    // over a strategy changed since, it would put back what the person changed.
                    let now = book::strategy_hash(&b.strategy(sid)?);
                    if b.proposal_based_on(id)?.is_some_and(|h| h != now) {
                        return Err(BookError::Invalid("The strategy has changed since this was asked, so nothing was saved: ask again for a change to it as it is now.".into()));
                    }
                    b.save_strategy(sid, &draft).map(|_| sid)
                }
                None => b.create_strategy(&draft),
            });
            match result {
                Ok(sid) => with_book(engine, |b| b.resolve_proposal(id, "approved", Some(&format!("Saved as strategy {sid}, version {}.", b.strategy(sid)?.version))))?,
                Err(e) => {
                    with_book(engine, |b| b.resolve_proposal(id, "failed", Some(&e.message)))?;
                }
            }
        }
    }
    Ok(with_book(engine, |b| b.proposal(id))?.0)
}

/// The kill switch, or one strategy's halt: cancels open orders, places none until restarted.
pub fn halt(engine: &Engine, strategy: Option<i64>, reason: &str) -> Result<(u32, Vec<String>), BusError> {
    let _turn = lock(&engine.arbiter.trade);
    match strategy {
        Some(id) => {
            with_book(engine, |b| b.set_state(id, RunState::Halted, Some(reason)))?;
        }
        None => {
            with_book(engine, |b| {
                b.set_halted(Some(reason))?;
                b.log(None, "halt", &format!("Halted everything: {reason}."), None).map(|_| ())
            })?;
        }
    }
    let open = with_book(engine, |b| b.open_orders(strategy))?;
    // The person pressed it, perhaps just after unlocking the keyring: ask it now, not after the
    // retry wait, so every live cancel below has the key if it can be read at all.
    if open.iter().any(|o| o.venue == Venue::Live) {
        let _ = account_now(engine);
    }
    let (mut cancelled, mut failed) = (0, Vec::new());
    for o in open {
        match cancel_one(engine, &o, "halted") {
            Ok(()) => cancelled += 1,
            Err(e) => failed.push(format!("Order {}: {}", o.id, e.message)),
        }
    }
    Ok((cancelled, failed))
}

// ------------------------------------------------------------------ the runner

/// One pass: reconcile open orders, expire proposals, check limits, and let each running
/// strategy decide on its newest closed bar. Returns whether anything changed.
pub fn pass(engine: &Engine) -> bool {
    let now = book::now();
    let before = fingerprint(engine);
    let mut errors: Vec<String> = Vec::new();
    let _ = with_book(engine, |b| b.expire_proposals(now));
    for o in with_book(engine, |b| b.open_orders(None)).unwrap_or_default() {
        if let Err(e) = reconcile_order(engine, o.id, false) {
            errors.push(e.message);
        }
    }
    let strategies = with_book(engine, |b| b.strategies()).unwrap_or_default();
    let halted_all = with_book(engine, |b| b.halted()).ok().flatten().is_some();
    if strategies.iter().any(|s| s.venue == Venue::Live) {
        let _ = refresh_account(engine, false);
    }
    if !halted_all {
        for s in strategies.iter().filter(|s| s.state == RunState::Running) {
            if let Err(e) = decide(engine, s, now) {
                errors.push(format!("{}: {}", s.name, e.message));
            }
        }
        if let Err(e) = global_loss(engine) {
            errors.push(e.message);
        }
    }
    let _ = snapshot_paper(engine, now);
    let _ = with_book(engine, |b| b.set_runner_status(now, (!errors.is_empty()).then(|| errors.join("; ")).as_deref()));
    fingerprint(engine) != before
}

/// What a pass may change, to tell whether it did.
fn fingerprint(engine: &Engine) -> (i64, i64, String) {
    with_book(engine, |b| {
        let d = b.decisions(None, 1)?.first().map_or(0, |d| d.id);
        let o = b.orders(None, 1)?.first().map(|o| (o.id, o.updated_at.clone())).unwrap_or_default();
        Ok((d, o.0, o.1))
    })
    .unwrap_or_default()
}

/// What every strategy together has lost on `venue` today at the last prices: realized, plus
/// what holdings there are down. Each venue is weighed on its own, so paper's gains never offset
/// a real loss; a strategy that has moved off the venue since still counts what it made there.
/// What the gate and Arbiter's daily loss limit weigh.
fn venue_loss(engine: &Engine, b: &Book, venue: Venue, taker: Decimal) -> book::Result<Decimal> {
    let mut pnl = Decimal::ZERO;
    for s in b.strategies()? {
        let p = b.position(s.id, venue)?;
        pnl += p.realized_today + p.open_pnl(last_price(engine, &s.product), taker).min(Decimal::ZERO);
    }
    Ok((-pnl).max(Decimal::ZERO))
}

/// The first venue whose loss today is at or past Arbiter's daily limit, live weighed first.
fn venue_past(engine: &Engine, limit: Decimal) -> Result<Option<Venue>, BusError> {
    let taker = fees(engine).taker;
    with_book(engine, |b| {
        for venue in [Venue::Live, Venue::Paper] {
            if venue_loss(engine, b, venue, taker)? >= limit {
                return Ok(Some(venue));
            }
        }
        Ok(None)
    })
}

/// Arbiter's daily loss limit: either venue past it halts everything, as the kill switch does,
/// so a day lost on paper stops live trading too.
fn global_loss(engine: &Engine) -> Result<(), BusError> {
    let g = with_book(engine, |b| b.global_limits())?;
    let Some(limit) = g.daily_loss else { return Ok(()) };
    if let Some(venue) = venue_past(engine, limit)? {
        halt(engine, None, &format!("Arbiter's daily loss limit of {} reached on {}", limit.normalize(), venue.as_str()))?;
    }
    Ok(())
}

/// Refuses to lift a halt the runner's next pass would only put back: today's loss still at or
/// past the daily limit, Arbiter's on either venue (no `strategy`) or the strategy's own. No
/// network: the last prices, as the meters read them.
pub fn check_restart(engine: &Engine, strategy: Option<i64>) -> Result<(), BusError> {
    let past = |limit: Decimal, currency: &str, own: bool, on: &str| -> Result<(), BusError> {
        let (whose, raise) = if own { ("its", "raise it under Limits on its page") } else { ("the", "raise it in Settings") };
        Err(BusError::refused("arbiter.daily_loss", format!("Today's loss{on} is still past {whose} daily limit of {} {currency}: {raise}, or restart tomorrow", limit.normalize())))
    };
    match strategy {
        None => {
            let (g, home) = with_book(engine, |b| Ok((b.global_limits()?, b.home()?)))?;
            let Some(limit) = g.daily_loss else { return Ok(()) };
            match venue_past(engine, limit)? {
                Some(venue) => past(limit, &home, false, &format!(" on {}", venue.as_str())),
                None => Ok(()),
            }
        }
        Some(id) => {
            let s = with_book(engine, |b| b.strategy(id))?;
            let Some(limit) = s.limits.daily_loss else { return Ok(()) };
            let pos = with_book(engine, |b| b.position(s.id, s.venue))?;
            // The runner weighs it at the bid: the last one read, else the last price.
            let read = lock(&engine.arbiter.quotes).get(&s.product).filter(|q| book::now() - q.at <= 300).map(|q| q.bid);
            let bid = read.or_else(|| last_price(engine, &s.product));
            let loss = (-(pos.realized_today + pos.open_pnl(bid, fees(engine).taker).min(Decimal::ZERO))).max(Decimal::ZERO);
            if loss >= limit { past(limit, s.product.split_once('-').map_or("", |(_, q)| q), true, "") } else { Ok(()) }
        }
    }
}

fn snapshot_paper(engine: &Engine, now: i64) -> Result<(), BusError> {
    let value = paper_value(engine)?;
    with_book(engine, |b| b.snapshot(Venue::Paper, now, value.to_f64().unwrap_or(0.0)))
}

/// Paper cash plus what paper strategies hold at the last price.
pub fn paper_value(engine: &Engine) -> Result<Decimal, BusError> {
    let taker = fees(engine).taker;
    with_book(engine, |b| {
        let mut v = b.paper_cash_now()?;
        for s in b.strategies()? {
            let p = b.position(s.id, Venue::Paper)?;
            if p.base > Decimal::ZERO {
                v += p.cost + p.open_pnl(last_price(engine, &s.product), taker);
            }
        }
        Ok(v)
    })
}

/// One strategy's turn: its limits, its exits, then its entry.
fn decide(engine: &Engine, s: &StrategyRec, now: i64) -> Result<(), BusError> {
    if !with_book(engine, |b| b.open_orders(Some(s.id)))?.is_empty() {
        return Ok(());
    }
    let q = fresh_quote(engine, &s.product)?;
    let taker = fees(engine).taker;
    let pos = with_book(engine, |b| b.position(s.id, s.venue))?;
    // The daily loss limit halts before anything else is decided.
    if let Some(limit) = s.limits.daily_loss {
        let loss = (-(pos.realized_today + pos.open_pnl(Some(q.bid), taker).min(Decimal::ZERO))).max(Decimal::ZERO);
        if loss >= limit {
            halt(engine, Some(s.id), &format!("daily loss limit of {} reached", limit.normalize()))?;
            return Ok(());
        }
    }
    // Price exits fire between bars.
    if pos.base > Decimal::ZERO {
        let peak = s.peak.unwrap_or(q.bid).max(q.bid);
        if s.peak != Some(peak) {
            with_book(engine, |b| b.set_peak(s.id, Some(peak)))?;
        }
        if let Some(reason) = rules::price_exit(&s.rule.exit, pos.avg_price().unwrap_or(0.0), peak, q.bid) {
            // The cause names the holding as it is now: a sell that failed, or filled in part,
            // leaves a new cause behind it, so the stop is tried again instead of being answered
            // by the order that did not get out. A failure waits a minute before the next try.
            let opened = pos.opened_at.unwrap_or(0);
            let (failed, last) = with_book(engine, |b| b.failed_sells_since(s.id, s.venue, opened))?;
            if last.is_some_and(|t| now - t < EXIT_RETRY) {
                return Ok(());
            }
            let cause = format!("s{}:v{}:exit:{opened}:{}:{failed}", s.id, s.version, pos.base.normalize());
            let intent = Intent { product: s.product.clone(), side: Side::Sell, quote: None, base: None, limit: false, source: Source::Rule, cause };
            route(engine, s.id, &intent, &format!("{} at {:.2}", reason.as_str(), q.bid), None)?;
            return Ok(());
        }
    }
    // Scheduled buys: the latest one due, if not missed by long.
    if let Entry::Schedule { .. } = s.rule.entry {
        let after = s.last_schedule.unwrap_or(s.created_at);
        if let Some(t) = rules::next_scheduled(&s.rule.entry, after).filter(|t| *t <= now) {
            let mut due = t;
            while let Some(n) = rules::next_scheduled(&s.rule.entry, due).filter(|n| *n <= now) {
                due = n;
            }
            with_book(engine, |b| b.set_cursor(s.id, None, Some(due)))?;
            if now - due <= SCHEDULE_GRACE {
                let intent = Intent { product: s.product.clone(), side: Side::Buy, quote: Some(s.rule.buy), base: None, limit: s.rule.pricing == Pricing::Limit, source: Source::Rule, cause: format!("s{}:sched:{due}", s.id) };
                route(engine, s.id, &intent, "scheduled buy", None)?;
            } else {
                with_book(engine, |b| b.log(Some(s.id), "skip", "Missed a scheduled buy while Relay was closed; waiting for the next.", None).map(|_| ()))?;
            }
        }
    }
    // Bar decisions: on the newest closed bar, once.
    let g = s.granularity.seconds();
    let closed_start = now.div_euclid(g) * g - g;
    if s.last_bar.is_some_and(|l| l >= closed_start) {
        return Ok(());
    }
    let needs_bars = matches!(s.rule.entry, Entry::Signal { .. }) || s.rule.exit.when.is_some() || s.rule.exit.max_bars.is_some();
    if !needs_bars {
        with_book(engine, |b| b.set_cursor(s.id, Some(closed_start), None))?;
        return Ok(());
    }
    let warm = i64::from(rules::warmup(&s.rule)) + 2;
    let bars = candles(engine, &s.product, s.granularity, closed_start - warm * g, closed_start + g)?;
    let Some(i) = bars.iter().rposition(|c| c.start == closed_start) else {
        // The exchange has no bar yet (no trades in it); decide on the next.
        return Ok(());
    };
    with_book(engine, |b| b.set_cursor(s.id, Some(closed_start), None))?;
    let eval = rules::Evaluator::for_rule(&bars, &s.rule);
    if pos.base > Decimal::ZERO {
        let held = pos.opened_at.map_or(0, |t| ((closed_start - t.div_euclid(g) * g) / g).max(0) as u32);
        if let Some(reason) = rules::bar_exit(&s.rule.exit, &eval, i, held) {
            let intent = Intent { product: s.product.clone(), side: Side::Sell, quote: None, base: None, limit: false, source: Source::Rule, cause: format!("s{}:v{}:bar:{closed_start}:sell", s.id, s.version) };
            route(engine, s.id, &intent, reason.as_str(), None)?;
        }
    } else if let Entry::Signal { when } = &s.rule.entry {
        if eval.holds(when, i) {
            let intent = Intent { product: s.product.clone(), side: Side::Buy, quote: Some(s.rule.buy), base: None, limit: s.rule.pricing == Pricing::Limit, source: Source::Rule, cause: format!("s{}:v{}:bar:{closed_start}:buy", s.id, s.version) };
            route(engine, s.id, &intent, &format!("{} on the {} bar", rules::condition(when), s.granularity.short()), None)?;
        }
    }
    Ok(())
}

/// Wakes the runner every [`PASS`] while anything runs or waits. Holds only a weak reference,
/// so it never keeps an engine alive.
pub fn spawn_runner(engine: &Arc<Engine>) {
    let weak: Weak<Engine> = Arc::downgrade(engine);
    std::thread::Builder::new()
        .name("arbiter".into())
        .spawn(move || {
            crate::background_priority();
            let mut next = Instant::now() + Duration::from_secs(10);
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let Some(engine) = weak.upgrade() else { return };
                if engine.is_quitting() {
                    return;
                }
                if Instant::now() < next {
                    continue;
                }
                next = Instant::now() + PASS;
                // Nothing to do until the book has been opened and something runs or waits.
                if lock(&engine.arbiter.book).is_none() && !store_has_book(&engine) {
                    continue;
                }
                let busy = with_book(&engine, |b| {
                    Ok(b.strategies()?.iter().any(|s| s.state == RunState::Running) || !b.open_orders(None)?.is_empty() || !b.proposals(true, 1)?.is_empty())
                })
                .unwrap_or(false);
                if busy && pass(&engine) {
                    engine.emit_system("arbiter.changed", json!({"runner": true}));
                }
            }
        })
        .ok();
}

fn store_has_book(engine: &Engine) -> bool {
    !in_memory(engine) && engine.store.path().with_file_name("arbiter.db").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    /// A throwaway P-256 key made for tests; it has never been near an exchange.
    const KEY: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEICr3hFJQLruuvjzXzQvrZyWUlBfa0SdE9HtDCMhP7NYqoAoGCCqGSM49
AwEHoUQDQgAEIgIeCM0SMcTdKeIUVa5s6GrENsIc3sFPBcefipt5Ciw6PtUG6nMV
TsGghZLrhr8kSV5nZX8i1b+Q+3+QzuY0wQ==
-----END EC PRIVATE KEY-----";

    #[test]
    fn a_keyring_that_could_not_be_read_is_asked_again_and_no_key_stays_known() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(Instance::Test, crate::Store::open(&dir.path().join("store.db"), false).unwrap());
        // No key saved: that is known, and kept.
        assert!(account(&engine).is_none());
        assert!(matches!(*lock(&engine.arbiter.keyed), Some(None)));
        *lock(&engine.arbiter.keyed) = None;
        // The keyring cannot be read (here the key's file is a folder): nothing is kept.
        std::fs::create_dir(test_key_file(&engine)).unwrap();
        assert!(account(&engine).is_none());
        assert!(lock(&engine.arbiter.keyed).is_none());
        // A key is there now: within the minute the keyring is not asked again, after it it is.
        std::fs::remove_dir(test_key_file(&engine)).unwrap();
        store_key(&engine, &Credentials::parse("organizations/o/apiKeys/k1", KEY).unwrap()).unwrap();
        assert!(account(&engine).is_none());
        *lock(&engine.arbiter.key_failed_at) = Some(book::now() - KEY_RETRY);
        assert!(account(&engine).is_some());
    }

    #[test]
    fn what_the_person_starts_asks_the_keyring_now_not_after_the_retry_wait() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(Instance::Test, crate::Store::open(&dir.path().join("store.db"), false).unwrap());
        std::fs::create_dir(test_key_file(&engine)).unwrap();
        assert!(account(&engine).is_none());
        // Unlocked a moment later: the runner waits out the minute, the kill switch does not.
        std::fs::remove_dir(test_key_file(&engine)).unwrap();
        store_key(&engine, &Credentials::parse("organizations/o/apiKeys/k1", KEY).unwrap()).unwrap();
        assert!(account(&engine).is_none());
        assert!(account_now(&engine).is_some());
        assert!(account(&engine).is_some());
    }

    #[test]
    fn a_lookup_that_says_nothing_is_no_key_and_one_that_says_why_is_a_failure() {
        let out = |code: i32, stdout: &str, stderr: &str| std::process::Output { status: std::process::ExitStatus::from_raw(code << 8), stdout: stdout.into(), stderr: stderr.into() };
        assert_eq!(looked_up(&out(0, "{}", ""), true).unwrap().as_deref(), Some("{}"));
        assert_eq!(looked_up(&out(1, "", ""), false).unwrap(), None);
        // Silent, while the book records a key: a dismissed unlock prompt, not "no key".
        assert_eq!(looked_up(&out(1, "", ""), true).unwrap_err().code, "arbiter.keyring");
        let locked = looked_up(&out(1, "", "Cannot get secret of a locked object\n"), false).unwrap_err();
        assert_eq!((locked.code.as_str(), locked.message.as_str()), ("arbiter.keyring", "Cannot get secret of a locked object"));
    }
}
