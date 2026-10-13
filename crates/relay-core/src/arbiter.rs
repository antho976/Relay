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
/// A scheduled buy missed by more than this (the app was closed) is skipped, not caught up.
const SCHEDULE_GRACE: i64 = 3600;

#[derive(Default)]
pub struct Desk {
    book: Mutex<Option<Book>>,
    /// The client for the saved key: `None` not yet loaded, `Some(None)` no key.
    keyed: Mutex<Option<Option<Arc<dyn Exchange>>>>,
    public: Mutex<Option<Arc<dyn Exchange>>>,
    /// Tests put a fake exchange here; it then serves both the market and the account.
    fake: Mutex<Option<Arc<dyn Exchange>>>,
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

fn load_key(engine: &Engine) -> Option<Credentials> {
    if in_memory(engine) {
        return None;
    }
    let body = if engine.instance == Instance::Test {
        std::fs::read_to_string(test_key_file(engine)).ok()?
    } else {
        let mut cmd = Command::new("secret-tool");
        cmd.args(["lookup", "application", "Relay", "purpose", "arbiter-coinbase"]);
        keyring(&mut cmd).ok()?
    };
    let v: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    Credentials::parse(v["key_name"].as_str()?, v["private_key"].as_str()?).ok()
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

/// The account client for the saved key, loaded from the keyring once.
pub fn account(engine: &Engine) -> Option<Arc<dyn Exchange>> {
    if let Some(f) = lock(&engine.arbiter.fake).clone() {
        return Some(f);
    }
    let mut slot = lock(&engine.arbiter.keyed);
    if slot.is_none() {
        let client = load_key(engine).and_then(|c| Coinbase::with_key(&c).ok()).map(|c| Arc::new(c) as Arc<dyn Exchange>);
        *slot = Some(client);
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

/// Reads permissions, fees and balances, and values the balances in the home currency.
pub fn refresh_account(engine: &Engine, force: bool) -> Result<Connection, BusError> {
    let now = book::now();
    {
        let mut at = lock(&engine.arbiter.account_at);
        if !force && now - *at < ACCOUNT_TTL {
            return with_book(engine, |b| b.connection());
        }
        *at = now;
    }
    let Some(client) = account(engine) else {
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
        let mut pnl_all = Decimal::ZERO;
        for other in b.strategies()?.iter().filter(|o| o.venue == s.venue) {
            let p = b.position(other.id, s.venue)?;
            exposure += p.cost;
            pnl_all += p.realized_today + p.open_pnl(last_price(engine, &other.product), taker).min(Decimal::ZERO);
        }
        let n = b.orders_since(Some(s.id), s.venue, now - 3600)?;
        let n_all = b.orders_since(None, s.venue, now - 3600)?;
        Ok((pos, exposure, (-pnl_all).max(Decimal::ZERO), n, n_all, b.halted()?.is_some()))
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
                // Nothing is known about whether it arrived: the order stays pending and the
                // runner asks again with the same client_order_id, which cannot place it twice.
                Err(ExchangeError::Unreachable(m)) => with_book(engine, |b| b.log(Some(s.id), "error", &format!("{what}: Coinbase did not answer ({m}); will check again."), Some(id)).map(|_| ()))?,
                Err(e) => with_book(engine, |b| {
                    b.fail_order(id, &e.to_string())?;
                    b.log(Some(s.id), "error", &format!("{what} failed: {e}"), Some(id)).map(|_| ())
                })?,
            }
            let _ = reconcile_order(engine, id);
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

/// Preview, then place. The preview's refusals stop it before anything is sent.
fn place_live(client: &dyn Exchange, req: &OrderRequest) -> exchange::Result<ExchangeOrder> {
    let preview = client.preview(req)?;
    if !preview.errors.is_empty() {
        return Err(ExchangeError::Refused(preview.errors.join("; ")));
    }
    client.place(req, preview.preview_id.as_deref())
}

/// Brings one order up to date with its venue: status and fills.
pub fn reconcile_order(engine: &Engine, id: i64) -> Result<(), BusError> {
    let o = with_book(engine, |b| b.order(id))?;
    if o.status.is_done() {
        return Ok(());
    }
    match o.venue {
        Venue::Paper => {
            let Some(sid) = o.strategy_id else { return Ok(()) };
            let req = OrderRequest { client_order_id: o.client_order_id.clone(), product: o.product.clone(), side: o.side, quote_size: o.quote_size, base_size: o.base_size, limit_price: o.limit_price };
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
            let Some(eid) = o.exchange_order_id.clone() else {
                // Sent, but no answer came: ask again with the same id (the exchange returns the
                // order it has), for a minute; after that, say so plainly.
                let age = book::now() - jiff::Timestamp::from_str(&o.created_at).map(|t| t.as_second()).unwrap_or(0);
                if age < 60 {
                    let req = OrderRequest { client_order_id: o.client_order_id.clone(), product: o.product.clone(), side: o.side, quote_size: o.quote_size, base_size: o.base_size, limit_price: o.limit_price };
                    if let Ok(order) = client.place(&req, None) {
                        with_book(engine, |b| b.update_order(id, &order))?;
                    }
                } else {
                    with_book(engine, |b| {
                        b.fail_order(id, "Coinbase never confirmed it. Check the Coinbase app: if it shows this order, it is real.")?;
                        b.log(o.strategy_id, "error", "An order was never confirmed by Coinbase; check the Coinbase app.", Some(id)).map(|_| ())
                    })?;
                }
                return Ok(());
            };
            let order = client.order(&eid).map_err(ex)?;
            book_live_fills(engine, client.as_ref(), &o, &eid, &order)?;
            with_book(engine, |b| b.update_order(id, &order))?;
            if !order.status.is_done() {
                if let Some(sid) = o.strategy_id {
                    if stale_limit(engine, sid, &o) {
                        cancel_one(engine, &o, "it did not fill within a bar")?;
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
    let created = jiff::Timestamp::from_str(&o.created_at).map(|t| t.as_second()).unwrap_or(0);
    book::now() - created > bar
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

fn cancel_one(engine: &Engine, o: &OrderView, why: &str) -> Result<(), BusError> {
    if o.venue == Venue::Live {
        if let (Some(client), Some(eid)) = (account(engine), o.exchange_order_id.as_ref()) {
            client.cancel(std::slice::from_ref(eid)).map_err(ex)?;
            // A cancel races the order's fills: it may have filled, wholly or in part, since the
            // last pass, and the exchange answers a filled order's cancel as done. Book what the
            // exchange says it did; a cancel still queued stays open for the next pass.
            let mut order = client.order(eid).map_err(ex)?;
            book_live_fills(engine, client.as_ref(), o, eid, &order)?;
            if order.status == OrderStatus::Cancelled && order.reason.is_none() {
                order.reason = Some(why.to_string());
            }
            return with_book(engine, |b| {
                b.update_order(o.id, &order)?;
                b.log(o.strategy_id, "cancel", &format!("Cancelled an order: {why}."), Some(o.id)).map(|_| ())
            });
        }
    }
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

/// Approves or dismisses a proposal. An approved order passes the gate again, now.
pub fn resolve(engine: &Engine, id: i64, approve: bool, draft_hash: Option<&str>) -> Result<ProposalView, BusError> {
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
            let venue = with_book(engine, |b| b.strategy(sid))?.venue;
            if p.venue.is_some_and(|v| v != venue) {
                let text = format!("The strategy has switched to {} since this was asked; nothing was placed.", venue.as_str());
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
                Some(sid) => b.save_strategy(sid, &draft).map(|_| sid),
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
        if let Err(e) = reconcile_order(engine, o.id) {
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

fn global_loss(engine: &Engine) -> Result<(), BusError> {
    let g = with_book(engine, |b| b.global_limits())?;
    let Some(limit) = g.daily_loss else { return Ok(()) };
    let taker = fees(engine).taker;
    let loss = with_book(engine, |b| {
        let mut pnl = Decimal::ZERO;
        for s in b.strategies()? {
            let p = b.position(s.id, s.venue)?;
            pnl += p.realized_today + p.open_pnl(last_price(engine, &s.product), taker).min(Decimal::ZERO);
        }
        Ok((-pnl).max(Decimal::ZERO))
    })?;
    if loss >= limit {
        halt(engine, None, &format!("Arbiter's daily loss limit of {} reached", limit.normalize()))?;
    }
    Ok(())
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
