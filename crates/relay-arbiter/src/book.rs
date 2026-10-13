//! The book: Arbiter's own SQLite file (`arbiter.db`) beside the store, apart from it as Tally's
//! ledger is — its own schema versions, and nothing slow ever waits on the store's lock for it.
//!
//! What it keeps: strategies (each save a new version, its rule hashed), orders keyed by their
//! `client_order_id`, fills keyed by the exchange's trade id, proposals, an append-only decision
//! log (triggers refuse to change or delete a line), equity snapshots, and caches of products and
//! candles. Positions and profit are never stored: they are read back from the fills, so a fill
//! recorded twice cannot happen and a position cannot drift from its history.

use crate::backtest::Report;
use crate::model::*;
use crate::rules;
use crate::views::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;

/// Bumped with every entry appended to [`MIGRATIONS`]; every earlier version must stay openable.
pub const SCHEMA_VERSION: i64 = 1;

const MIGRATIONS: &[&str] = &[r"
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE strategies (
    id INTEGER PRIMARY KEY, name TEXT NOT NULL, product TEXT NOT NULL, granularity TEXT NOT NULL,
    rule TEXT NOT NULL, limits TEXT NOT NULL, mode TEXT NOT NULL DEFAULT 'rules',
    venue TEXT NOT NULL DEFAULT 'paper', state TEXT NOT NULL DEFAULT 'stopped', halt_reason TEXT,
    version INTEGER NOT NULL DEFAULT 1, rule_hash TEXT NOT NULL, variants_tried INTEGER NOT NULL DEFAULT 0,
    backtest TEXT,
    -- The runner's place: the last closed bar it decided on, the last schedule it ran, and the
    -- highest price seen since the holding was bought (for a trailing stop).
    last_bar INTEGER, last_schedule INTEGER, peak REAL,
    created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE orders (
    id INTEGER PRIMARY KEY, strategy_id INTEGER, venue TEXT NOT NULL, source TEXT NOT NULL,
    side TEXT NOT NULL, product TEXT NOT NULL, quote_size TEXT, base_size TEXT, limit_price TEXT,
    status TEXT NOT NULL, filled_base TEXT NOT NULL DEFAULT '0', average_price TEXT,
    fees TEXT NOT NULL DEFAULT '0', why TEXT NOT NULL, error TEXT,
    client_order_id TEXT NOT NULL UNIQUE, exchange_order_id TEXT,
    created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE INDEX orders_strategy ON orders (strategy_id, created_at);
CREATE TABLE fills (
    id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), strategy_id INTEGER,
    venue TEXT NOT NULL, side TEXT NOT NULL, product TEXT NOT NULL,
    price TEXT NOT NULL, size TEXT NOT NULL, fee TEXT NOT NULL, pnl TEXT,
    trade_id TEXT NOT NULL UNIQUE, at INTEGER NOT NULL);
CREATE INDEX fills_strategy ON fills (strategy_id, venue, at);
CREATE TABLE decisions (
    id INTEGER PRIMARY KEY, strategy_id INTEGER, kind TEXT NOT NULL, text TEXT NOT NULL,
    order_id INTEGER, at INTEGER NOT NULL);
CREATE INDEX decisions_strategy ON decisions (strategy_id, id);
CREATE TRIGGER decisions_no_update BEFORE UPDATE ON decisions BEGIN SELECT RAISE(ABORT, 'the decision log is append-only'); END;
CREATE TRIGGER decisions_no_delete BEFORE DELETE ON decisions BEGIN SELECT RAISE(ABORT, 'the decision log is append-only'); END;
CREATE TABLE proposals (
    id INTEGER PRIMARY KEY, kind TEXT NOT NULL, strategy_id INTEGER, title TEXT NOT NULL,
    why TEXT NOT NULL, source TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', outcome TEXT,
    side TEXT, product TEXT, quote TEXT, base TEXT, is_limit INTEGER NOT NULL DEFAULT 0, venue TEXT,
    draft TEXT, draft_hash TEXT, changes TEXT NOT NULL DEFAULT '[]', thread_id INTEGER,
    cause TEXT, created_at INTEGER NOT NULL, expires_at INTEGER, resolved_at INTEGER);
CREATE INDEX proposals_status ON proposals (status, id);
CREATE TABLE equity (venue TEXT NOT NULL, at INTEGER NOT NULL, value REAL NOT NULL, PRIMARY KEY (venue, at));
CREATE TABLE products (id TEXT PRIMARY KEY, body TEXT NOT NULL, fetched_at INTEGER NOT NULL);
CREATE TABLE candles (
    product TEXT NOT NULL, granularity TEXT NOT NULL, start INTEGER NOT NULL,
    open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL, volume REAL NOT NULL,
    PRIMARY KEY (product, granularity, start)) WITHOUT ROWID;
"];

#[derive(Debug)]
pub enum BookError {
    Invalid(String),
    NotFound(String),
    Sql(rusqlite::Error),
}

impl std::fmt::Display for BookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookError::Invalid(m) | BookError::NotFound(m) => f.write_str(m),
            BookError::Sql(e) => write!(f, "arbiter.db: {e}"),
        }
    }
}

impl From<rusqlite::Error> for BookError {
    fn from(e: rusqlite::Error) -> Self {
        BookError::Sql(e)
    }
}

pub type Result<T> = std::result::Result<T, BookError>;

fn invalid<T>(m: impl Into<String>) -> Result<T> {
    Err(BookError::Invalid(m.into()))
}

pub fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// RFC 3339 for a view.
pub fn rfc3339(at: i64) -> String {
    jiff::Timestamp::from_second(at).map(|t| t.to_string()).unwrap_or_default()
}

fn dec(s: Option<String>) -> Option<Decimal> {
    s.and_then(|s| Decimal::from_str(&s).ok())
}

fn dec0(s: String) -> Decimal {
    Decimal::from_str(&s).unwrap_or_default()
}

fn text(d: Option<Decimal>) -> Option<String> {
    d.map(|d| d.normalize().to_string())
}

/// Midnight UTC of the day `at` falls in: "today" for daily limits.
pub fn day_start(at: i64) -> i64 {
    at.div_euclid(86400) * 86400
}

/// A strategy as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyRec {
    pub id: i64,
    pub name: String,
    pub product: String,
    pub granularity: Granularity,
    pub rule: Rule,
    pub limits: Limits,
    pub mode: Mode,
    pub venue: Venue,
    pub state: RunState,
    pub halt_reason: Option<String>,
    pub version: i64,
    pub rule_hash: String,
    pub variants_tried: u32,
    pub backtest: Option<Report>,
    pub last_bar: Option<i64>,
    pub last_schedule: Option<i64>,
    pub peak: Option<f64>,
    pub created_at: i64,
}

impl StrategyRec {
    pub fn draft(&self) -> Draft {
        Draft { name: self.name.clone(), product: self.product.clone(), granularity: self.granularity, rule: self.rule.clone(), limits: self.limits.clone() }
    }
}

const STRATEGY_COLUMNS: &str = "id, name, product, granularity, rule, limits, mode, venue, state, halt_reason, version, rule_hash, variants_tried, backtest, last_bar, last_schedule, peak, created_at";

fn strategy_row(r: &Row) -> rusqlite::Result<StrategyRec> {
    let rule: String = r.get(4)?;
    let limits: String = r.get(5)?;
    let backtest: Option<String> = r.get(13)?;
    let bad = |i: usize, e: serde_json::Error| rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, Box::new(e));
    Ok(StrategyRec {
        id: r.get(0)?,
        name: r.get(1)?,
        product: r.get(2)?,
        granularity: Granularity::parse(&r.get::<_, String>(3)?).unwrap_or_default(),
        rule: serde_json::from_str(&rule).map_err(|e| bad(4, e))?,
        limits: serde_json::from_str(&limits).map_err(|e| bad(5, e))?,
        mode: Mode::parse(&r.get::<_, String>(6)?).unwrap_or_default(),
        venue: Venue::parse(&r.get::<_, String>(7)?).unwrap_or_default(),
        state: RunState::parse(&r.get::<_, String>(8)?).unwrap_or_default(),
        halt_reason: r.get(9)?,
        version: r.get(10)?,
        rule_hash: r.get(11)?,
        variants_tried: r.get::<_, i64>(12)? as u32,
        backtest: backtest.and_then(|b| serde_json::from_str(&b).ok()),
        last_bar: r.get(14)?,
        last_schedule: r.get(15)?,
        peak: r.get(16)?,
        created_at: r.get(17)?,
    })
}

/// An order to record before it is sent.
#[derive(Debug, Clone)]
pub struct NewOrder<'a> {
    pub strategy_id: Option<i64>,
    pub venue: Venue,
    pub source: Source,
    pub request: &'a OrderRequest,
    pub why: &'a str,
}

/// What a strategy holds on one venue and what it has made, read back from its fills.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Position {
    pub base: Decimal,
    /// What the holding cost, fees included.
    pub cost: Decimal,
    pub realized: Decimal,
    pub realized_today: Decimal,
    pub fees: Decimal,
    /// Closed round trips (sells), and how many made money.
    pub sells: u32,
    pub won: u32,
    pub first_at: Option<i64>,
    pub last_loss_at: Option<i64>,
    /// When the holding started (the first buy since it was last empty).
    pub opened_at: Option<i64>,
    /// The most held at once, at cost.
    pub max_cost: Decimal,
}

impl Position {
    pub fn avg_price(&self) -> Option<f64> {
        (self.base > Decimal::ZERO).then(|| (self.cost / self.base).to_f64().unwrap_or(0.0))
    }
    /// What the holding is up or down at `price`, after the fee to sell it.
    pub fn open_pnl(&self, price: Option<f64>, taker: Decimal) -> Decimal {
        match price.and_then(Decimal::from_f64) {
            Some(p) if self.base > Decimal::ZERO => self.base * p * (Decimal::ONE - taker) - self.cost,
            _ => Decimal::ZERO,
        }
    }
}

pub struct Book {
    conn: Connection,
}

impl Book {
    pub fn open(path: &Path) -> Result<Book> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Book::init(conn)
    }

    pub fn open_in_memory() -> Result<Book> {
        Book::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Book> {
        let mut book = Book { conn };
        let have: i64 = book.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if have > SCHEMA_VERSION {
            return invalid(format!("arbiter.db is version {have}; this build reads up to {SCHEMA_VERSION}"));
        }
        let tx = book.conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(have as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        Ok(book)
    }

    // ------------------------------------------------------------------ settings

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?.query_row([key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.prepare_cached("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?.execute(params![key, v])?,
            None => self.conn.prepare_cached("DELETE FROM settings WHERE key = ?1")?.execute([key])?,
        };
        Ok(())
    }

    fn json_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        Ok(self.setting(key)?.and_then(|s| serde_json::from_str(&s).ok()))
    }

    /// The currency totals are in. CAD until set: the person is in Québec.
    pub fn home(&self) -> Result<String> {
        Ok(self.setting("home")?.unwrap_or_else(|| "CAD".into()))
    }

    pub fn set_home(&self, code: &str) -> Result<()> {
        let code = code.trim().to_ascii_uppercase();
        if !(2..=10).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return invalid(format!("{code} is not a currency code"));
        }
        self.set_setting("home", Some(&code))
    }

    /// What paper trading starts with.
    pub fn paper_cash(&self) -> Result<Decimal> {
        Ok(dec(self.setting("paper_cash")?).unwrap_or(Decimal::new(1000, 0)))
    }

    pub fn set_paper_cash(&self, d: Decimal) -> Result<()> {
        if d <= Decimal::ZERO {
            return invalid("Paper trading needs some cash to start with");
        }
        self.set_setting("paper_cash", Some(&d.normalize().to_string()))
    }

    pub fn global_limits(&self) -> Result<GlobalLimits> {
        Ok(self.json_setting("global_limits")?.unwrap_or_default())
    }

    pub fn set_global_limits(&self, g: &GlobalLimits) -> Result<()> {
        if g.price_band_pct <= Decimal::ZERO || g.price_band_pct > Decimal::new(20, 0) {
            return invalid("The price band must be between 0 and 20 percent");
        }
        if g.slippage_pct < Decimal::ZERO || g.slippage_pct > Decimal::new(5, 0) {
            return invalid("Slippage must be between 0 and 5 percent");
        }
        if g.approval_minutes == 0 || g.approval_minutes > 24 * 60 {
            return invalid("An approval must wait between 1 minute and a day");
        }
        for (name, v) in [("most held", g.max_exposure), ("daily loss", g.daily_loss)] {
            if v.is_some_and(|v| v <= Decimal::ZERO) {
                return invalid(format!("The {name} limit must be more than zero"));
            }
        }
        self.set_setting("global_limits", Some(&serde_json::to_string(g).unwrap_or_default()))
    }

    /// The kill switch: its reason while on.
    pub fn halted(&self) -> Result<Option<String>> {
        self.setting("halted")
    }

    pub fn set_halted(&self, reason: Option<&str>) -> Result<()> {
        self.set_setting("halted", reason)
    }

    pub fn connection(&self) -> Result<Connection_> {
        Ok(self.json_setting("connection")?.unwrap_or_default())
    }

    pub fn set_connection(&self, c: &Connection_) -> Result<()> {
        self.set_setting("connection", Some(&serde_json::to_string(c).unwrap_or_default()))
    }

    pub fn balances(&self) -> Result<Vec<Balance>> {
        Ok(self.json_setting("balances")?.unwrap_or_default())
    }

    pub fn set_balances(&self, b: &[Balance]) -> Result<()> {
        self.set_setting("balances", Some(&serde_json::to_string(b).unwrap_or_default()))
    }

    /// The runner's last pass and its error, for the overview.
    pub fn runner_status(&self) -> Result<(Option<i64>, Option<String>)> {
        Ok((self.setting("runner_at")?.and_then(|s| s.parse().ok()), self.setting("runner_error")?))
    }

    pub fn set_runner_status(&self, at: i64, error: Option<&str>) -> Result<()> {
        self.set_setting("runner_at", Some(&at.to_string()))?;
        self.set_setting("runner_error", error)
    }

    // ------------------------------------------------------------------ strategies

    pub fn strategies(&self) -> Result<Vec<StrategyRec>> {
        let mut st = self.conn.prepare_cached(&format!("SELECT {STRATEGY_COLUMNS} FROM strategies WHERE deleted = 0 ORDER BY id"))?;
        let rows = st.query_map([], strategy_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn strategy(&self, id: i64) -> Result<StrategyRec> {
        self.conn
            .prepare_cached(&format!("SELECT {STRATEGY_COLUMNS} FROM strategies WHERE id = ?1 AND deleted = 0"))?
            .query_row([id], strategy_row)
            .optional()?
            .ok_or_else(|| BookError::NotFound(format!("There is no strategy {id}")))
    }

    fn check_draft(d: &Draft) -> Result<()> {
        if d.name.trim().is_empty() || d.name.chars().count() > 60 {
            return invalid("A strategy needs a name of 1 to 60 characters");
        }
        if !crate::coinbase::product_id_ok(&d.product) {
            return invalid(format!("{} is not a product id like BTC-CAD", d.product));
        }
        rules::validate(&d.rule).map_err(BookError::Invalid)?;
        for (name, v) in [("most per order", d.limits.max_order), ("most held", d.limits.max_position), ("daily loss", d.limits.daily_loss)] {
            if v.is_some_and(|v| v <= Decimal::ZERO) {
                return invalid(format!("The {name} limit must be more than zero"));
            }
        }
        if d.limits.orders_per_hour == Some(0) {
            return invalid("Orders per hour must be at least one, or no limit");
        }
        if let Some(max) = d.limits.max_order.filter(|m| d.rule.buy > *m) {
            return invalid(format!("Each buy spends {}, more than the {} allowed per order", d.rule.buy.normalize(), max.normalize()));
        }
        Ok(())
    }

    pub fn create_strategy(&self, d: &Draft) -> Result<i64> {
        Book::check_draft(d)?;
        let t = now();
        self.conn.prepare_cached(
            "INSERT INTO strategies (name, product, granularity, rule, limits, rule_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        )?.execute(params![
            d.name.trim(), d.product, d.granularity.as_str(), serde_json::to_string(&d.rule).unwrap_or_default(),
            serde_json::to_string(&d.limits).unwrap_or_default(), rules::hash(&d.rule), t
        ])?;
        let id = self.conn.last_insert_rowid();
        self.log(Some(id), "setting", &format!("Created \"{}\" on {} {} bars, on paper and stopped.", d.name.trim(), d.product, d.granularity.short()), None)?;
        Ok(id)
    }

    /// A new version of a strategy. Its backtest belonged to the old rule, so it goes; the
    /// runner's place is kept so a saved edit does not refire on a bar already decided.
    pub fn save_strategy(&self, id: i64, d: &Draft) -> Result<()> {
        Book::check_draft(d)?;
        let old = self.strategy(id)?;
        if old.product != d.product && self.position(id, old.venue)?.base > Decimal::ZERO {
            return invalid("It still holds what it bought: sell it before changing the product");
        }
        Book::check_limits_for(&old, &d.limits)?;
        let hash = rules::hash(&d.rule);
        let changed_rule = hash != old.rule_hash || old.product != d.product || old.granularity != d.granularity;
        self.conn.prepare_cached(
            "UPDATE strategies SET name = ?2, product = ?3, granularity = ?4, rule = ?5, limits = ?6, rule_hash = ?7,
             version = version + 1, backtest = CASE WHEN ?8 THEN NULL ELSE backtest END, updated_at = ?9 WHERE id = ?1",
        )?.execute(params![
            id, d.name.trim(), d.product, d.granularity.as_str(), serde_json::to_string(&d.rule).unwrap_or_default(),
            serde_json::to_string(&d.limits).unwrap_or_default(), hash, changed_rule, now()
        ])?;
        let mut changes = changes(&old.draft(), d);
        if changes.is_empty() {
            changes.push("Saved with no changes.".into());
        }
        self.log(Some(id), "setting", &format!("Version {}: {}", old.version + 1, changes.join(" ")), None)?;
        Ok(())
    }

    pub fn set_strategy_limits(&self, id: i64, limits: &Limits) -> Result<()> {
        let s = self.strategy(id)?;
        let mut d = s.draft();
        d.limits = limits.clone();
        Book::check_draft(&d)?;
        Book::check_limits_for(&s, limits)?;
        self.conn.prepare_cached("UPDATE strategies SET limits = ?2, updated_at = ?3 WHERE id = ?1")?
            .execute(params![id, serde_json::to_string(limits).unwrap_or_default(), now()])?;
        self.log(Some(id), "setting", &format!("Limits: {}.", limits_text(limits)), None)?;
        Ok(())
    }

    /// What `set_mode` and `set_venue` asked of a strategy's limits still holds whichever way
    /// the limits change afterwards: an edit, or an agent's approved change, cannot drop them.
    fn check_limits_for(s: &StrategyRec, limits: &Limits) -> Result<()> {
        if s.mode == Mode::Agent && !limits.bounded() {
            return invalid("While the AI trades this strategy, it needs a limit per order, per holding and per day");
        }
        if s.venue == Venue::Live && s.mode != Mode::Ask && limits.max_order.is_none() {
            return invalid("While it trades real money on its own, it needs a limit per order (or let every order ask)");
        }
        Ok(())
    }

    pub fn set_mode(&self, id: i64, mode: Mode) -> Result<()> {
        let s = self.strategy(id)?;
        if mode == Mode::Agent && !s.limits.bounded() {
            return invalid("Before the AI may trade this strategy, give it a limit per order, per holding and per day");
        }
        if s.venue == Venue::Live && mode != Mode::Ask && s.limits.max_order.is_none() {
            return invalid("Before it trades real money on its own, give it a limit per order (or let every order ask)");
        }
        self.conn.prepare_cached("UPDATE strategies SET mode = ?2, updated_at = ?3 WHERE id = ?1")?.execute(params![id, mode.as_str(), now()])?;
        self.log(Some(id), "setting", &format!("Who decides: {}.", mode.label()), None)?;
        Ok(())
    }

    pub fn set_venue(&self, id: i64, venue: Venue) -> Result<()> {
        let s = self.strategy(id)?;
        if s.venue == venue {
            return Ok(());
        }
        if !self.open_orders(Some(id))?.is_empty() {
            return invalid("It has an order open: wait for it, or halt the strategy, before switching");
        }
        if venue == Venue::Live && s.mode != Mode::Ask && s.limits.max_order.is_none() {
            return invalid("Before it trades real money on its own, give it a limit per order (or let every order ask)");
        }
        // The runner starts afresh on the new venue: no bar or schedule from the old one carries.
        self.conn.prepare_cached("UPDATE strategies SET venue = ?2, peak = NULL, updated_at = ?3 WHERE id = ?1")?.execute(params![id, venue.as_str(), now()])?;
        self.log(Some(id), "setting", if venue == Venue::Live { "Switched to live: orders go to the exchange." } else { "Switched to paper." }, None)?;
        Ok(())
    }

    pub fn set_state(&self, id: i64, state: RunState, reason: Option<&str>) -> Result<()> {
        let s = self.strategy(id)?;
        if s.state == state && s.halt_reason.as_deref() == reason {
            return Ok(());
        }
        // A strategy started afresh decides from the next bar on, never the bars it missed.
        self.conn.prepare_cached(
            "UPDATE strategies SET state = ?2, halt_reason = ?3, last_bar = CASE WHEN ?2 = 'running' THEN NULL ELSE last_bar END,
             last_schedule = CASE WHEN ?2 = 'running' THEN ?4 ELSE last_schedule END, updated_at = ?4 WHERE id = ?1",
        )?.execute(params![id, state.as_str(), reason, now()])?;
        let text = match state {
            RunState::Running => "Started.".to_string(),
            RunState::Stopped => "Stopped.".to_string(),
            RunState::Halted => format!("Halted: {}.", reason.unwrap_or("halted")),
        };
        self.log(Some(id), if state == RunState::Halted { "halt" } else { "setting" }, &text, None)?;
        Ok(())
    }

    pub fn delete_strategy(&self, id: i64) -> Result<()> {
        let s = self.strategy(id)?;
        if s.state == RunState::Running {
            return invalid("Stop the strategy before deleting it");
        }
        for v in [Venue::Paper, Venue::Live] {
            if self.position(id, v)?.base > Decimal::ZERO {
                return invalid(format!("It still holds {} on {}: sell it first", s.product, v.as_str()));
            }
        }
        if !self.open_orders(Some(id))?.is_empty() {
            return invalid("It has an order open: halt it first");
        }
        self.conn.prepare_cached("UPDATE strategies SET deleted = 1, state = 'stopped', updated_at = ?2 WHERE id = ?1")?.execute(params![id, now()])?;
        self.log(Some(id), "setting", &format!("Deleted \"{}\".", s.name), None)?;
        Ok(())
    }

    pub fn set_cursor(&self, id: i64, last_bar: Option<i64>, last_schedule: Option<i64>) -> Result<()> {
        self.conn.prepare_cached("UPDATE strategies SET last_bar = COALESCE(?2, last_bar), last_schedule = COALESCE(?3, last_schedule) WHERE id = ?1")?
            .execute(params![id, last_bar, last_schedule])?;
        Ok(())
    }

    pub fn set_peak(&self, id: i64, peak: Option<f64>) -> Result<()> {
        self.conn.prepare_cached("UPDATE strategies SET peak = ?2 WHERE id = ?1")?.execute(params![id, peak])?;
        Ok(())
    }

    /// Records a backtest of the strategy's current rule and counts it as a version tried
    /// whenever the rule differs from the last one tried.
    pub fn record_backtest(&self, id: i64, rule_hash: &str, report: &Report) -> Result<()> {
        let s = self.strategy(id)?;
        let tried_key = format!("tried:{id}");
        let mut tried: Vec<String> = self.json_setting(&tried_key)?.unwrap_or_default();
        if !tried.iter().any(|h| h == rule_hash) {
            tried.push(rule_hash.to_string());
            self.set_setting(&tried_key, Some(&serde_json::to_string(&tried).unwrap_or_default()))?;
        }
        let same = s.rule_hash == rule_hash;
        self.conn.prepare_cached("UPDATE strategies SET variants_tried = ?2, backtest = CASE WHEN ?3 THEN ?4 ELSE backtest END WHERE id = ?1")?
            .execute(params![id, tried.len() as i64, same, serde_json::to_string(report).unwrap_or_default()])?;
        Ok(())
    }

    // ------------------------------------------------------------------ orders and fills

    /// Records an order before it is sent. The same `client_order_id` twice is the same order:
    /// its id comes back, and nothing is added.
    pub fn insert_order(&self, o: &NewOrder) -> Result<(i64, bool)> {
        if let Some(id) = self.conn.prepare_cached("SELECT id FROM orders WHERE client_order_id = ?1")?.query_row([&o.request.client_order_id], |r| r.get(0)).optional()? {
            return Ok((id, false));
        }
        let t = now();
        let r = o.request;
        self.conn.prepare_cached(
            "INSERT INTO orders (strategy_id, venue, source, side, product, quote_size, base_size, limit_price, status, why, client_order_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?10, ?11, ?11)",
        )?.execute(params![
            o.strategy_id, o.venue.as_str(), o.source.as_str(), r.side.as_str(), r.product, text(r.quote_size), text(r.base_size),
            text(r.limit_price), o.why, r.client_order_id, t
        ])?;
        Ok((self.conn.last_insert_rowid(), true))
    }

    pub fn update_order(&self, id: i64, x: &ExchangeOrder) -> Result<()> {
        self.conn.prepare_cached(
            "UPDATE orders SET status = ?2, filled_base = ?3, average_price = ?4, fees = ?5, exchange_order_id = COALESCE(NULLIF(?6, ''), exchange_order_id),
             error = COALESCE(?7, error), updated_at = ?8 WHERE id = ?1",
        )?.execute(params![
            id, x.status.as_str(), x.filled_size.normalize().to_string(),
            (x.average_filled_price > Decimal::ZERO).then(|| x.average_filled_price.normalize().to_string()),
            x.total_fees.normalize().to_string(), x.order_id, x.reason, now()
        ])?;
        Ok(())
    }

    pub fn fail_order(&self, id: i64, error: &str) -> Result<()> {
        self.conn.prepare_cached("UPDATE orders SET status = 'failed', error = ?2, updated_at = ?3 WHERE id = ?1")?.execute(params![id, error, now()])?;
        Ok(())
    }

    pub fn order(&self, id: i64) -> Result<OrderView> {
        self.orders_where("o.id = ?1", params![id], 1)?.pop().ok_or_else(|| BookError::NotFound(format!("There is no order {id}")))
    }

    /// The order and its strategy, by `client_order_id`.
    pub fn order_by_client_id(&self, cid: &str) -> Result<Option<OrderView>> {
        Ok(self.orders_where("o.client_order_id = ?1", params![cid], 1)?.pop())
    }

    pub fn orders(&self, strategy: Option<i64>, limit: u32) -> Result<Vec<OrderView>> {
        match strategy {
            Some(s) => self.orders_where("o.strategy_id = ?1", params![s], limit),
            None => self.orders_where("1", params![], limit),
        }
    }

    /// Orders still pending or open, oldest first.
    pub fn open_orders(&self, strategy: Option<i64>) -> Result<Vec<OrderView>> {
        let mut v = match strategy {
            Some(s) => self.orders_where("o.status IN ('pending', 'open') AND o.strategy_id = ?1", params![s], 500)?,
            None => self.orders_where("o.status IN ('pending', 'open')", params![], 500)?,
        };
        v.reverse();
        Ok(v)
    }

    fn orders_where(&self, cond: &str, p: &[&dyn rusqlite::ToSql], limit: u32) -> Result<Vec<OrderView>> {
        let sql = format!(
            "SELECT o.id, o.strategy_id, s.name, o.venue, o.source, o.side, o.product, o.quote_size, o.base_size, o.limit_price, o.status,
                    o.filled_base, o.average_price, o.fees, o.why, o.error, o.client_order_id, o.exchange_order_id, o.created_at, o.updated_at
             FROM orders o LEFT JOIN strategies s ON s.id = o.strategy_id WHERE {cond} ORDER BY o.id DESC LIMIT {}",
            limit.clamp(1, 1000)
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(p, |r| {
            Ok(OrderView {
                id: r.get(0)?,
                strategy_id: r.get(1)?,
                strategy: r.get(2)?,
                venue: Venue::parse(&r.get::<_, String>(3)?).unwrap_or_default(),
                source: Source::parse(&r.get::<_, String>(4)?).unwrap_or(Source::Rule),
                side: Side::parse(&r.get::<_, String>(5)?).unwrap_or(Side::Buy),
                product: r.get(6)?,
                quote_size: dec(r.get(7)?),
                base_size: dec(r.get(8)?),
                limit_price: dec(r.get(9)?),
                status: OrderStatus::parse(&r.get::<_, String>(10)?).unwrap_or(OrderStatus::Pending),
                filled_base: dec0(r.get(11)?),
                average_price: dec(r.get(12)?),
                fees: dec0(r.get(13)?),
                why: r.get(14)?,
                error: r.get(15)?,
                client_order_id: r.get(16)?,
                exchange_order_id: r.get(17)?,
                created_at: rfc3339(r.get(18)?),
                updated_at: rfc3339(r.get(19)?),
            })
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Records a fill once (its trade id is unique) and, for a sell, the profit it realized on
    /// the holding's average cost. Returns the fill's id, or `None` when it was already recorded.
    pub fn add_fill(&self, order_id: i64, venue: Venue, f: &ExchangeFill) -> Result<Option<i64>> {
        let trade_id = format!("{}:{}", venue.as_str(), f.trade_id);
        if self.conn.prepare_cached("SELECT 1 FROM fills WHERE trade_id = ?1")?.exists([&trade_id])? {
            return Ok(None);
        }
        let o = self.order(order_id)?;
        // An order placed in quote reports its fills in quote, and size over price runs to 28
        // places. The account moves in the product's base increment, so the fill is booked in it:
        // otherwise a sell, floored to that increment, leaves dust that reads as still holding.
        let mut f = f.clone();
        if let Some(inc) = self.cached_product(&o.product)?.map(|p| p.base_increment).filter(|i| *i > Decimal::ZERO) {
            f.size = (f.size / inc).round() * inc;
        }
        let f = &f;
        let pnl = match (o.side, o.strategy_id) {
            (Side::Sell, Some(sid)) => {
                let p = self.position(sid, venue)?;
                let share = if p.base > Decimal::ZERO { (f.size / p.base).min(Decimal::ONE) } else { Decimal::ZERO };
                Some(f.price * f.size - f.fee - p.cost * share)
            }
            _ => None,
        };
        self.conn.prepare_cached(
            "INSERT INTO fills (order_id, strategy_id, venue, side, product, price, size, fee, pnl, trade_id, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?.execute(params![
            order_id, o.strategy_id, venue.as_str(), o.side.as_str(), o.product, f.price.normalize().to_string(),
            f.size.normalize().to_string(), f.fee.normalize().to_string(), text(pnl), trade_id, f.at
        ])?;
        Ok(Some(self.conn.last_insert_rowid()))
    }

    pub fn fills(&self, strategy: Option<i64>, limit: u32) -> Result<Vec<FillView>> {
        let cond = if strategy.is_some() { "f.strategy_id = ?1" } else { "?1 IS NULL" };
        let sql = format!(
            "SELECT f.id, f.order_id, f.strategy_id, s.name, f.venue, f.side, f.product, f.price, f.size, f.fee, f.pnl, f.at
             FROM fills f LEFT JOIN strategies s ON s.id = f.strategy_id WHERE {cond} ORDER BY f.at DESC, f.id DESC LIMIT {}",
            limit.clamp(1, 1000)
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(params![strategy], |r| {
            Ok(FillView {
                id: r.get(0)?,
                order_id: r.get(1)?,
                strategy_id: r.get(2)?,
                strategy: r.get(3)?,
                venue: Venue::parse(&r.get::<_, String>(4)?).unwrap_or_default(),
                side: Side::parse(&r.get::<_, String>(5)?).unwrap_or(Side::Buy),
                product: r.get(6)?,
                price: dec0(r.get(7)?),
                size: dec0(r.get(8)?),
                fee: dec0(r.get(9)?),
                pnl: dec(r.get(10)?),
                at: rfc3339(r.get(11)?),
            })
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// A strategy's holding and record on one venue, from its fills in order.
    pub fn position(&self, strategy: i64, venue: Venue) -> Result<Position> {
        let today = day_start(now());
        let mut st = self.conn.prepare_cached("SELECT side, price, size, fee, pnl, at FROM fills WHERE strategy_id = ?1 AND venue = ?2 ORDER BY at, id")?;
        let mut rows = st.query(params![strategy, venue.as_str()])?;
        let mut p = Position::default();
        while let Some(r) = rows.next()? {
            let side = Side::parse(&r.get::<_, String>(0)?).unwrap_or(Side::Buy);
            let (price, size, fee) = (dec0(r.get(1)?), dec0(r.get(2)?), dec0(r.get(3)?));
            let at: i64 = r.get(5)?;
            p.first_at.get_or_insert(at);
            p.fees += fee;
            match side {
                Side::Buy => {
                    if p.base <= Decimal::ZERO {
                        p.opened_at = Some(at);
                    }
                    p.base += size;
                    p.cost += price * size + fee;
                    p.max_cost = p.max_cost.max(p.cost);
                }
                Side::Sell => {
                    let share = if p.base > Decimal::ZERO { (size / p.base).min(Decimal::ONE) } else { Decimal::ZERO };
                    let pnl = dec(r.get(4)?).unwrap_or(price * size - fee - p.cost * share);
                    p.cost -= p.cost * share;
                    p.base = (p.base - size).max(Decimal::ZERO);
                    if p.base.is_zero() {
                        p.cost = Decimal::ZERO;
                        p.opened_at = None;
                    }
                    p.realized += pnl;
                    if at >= today {
                        p.realized_today += pnl;
                    }
                    p.sells += 1;
                    if pnl > Decimal::ZERO {
                        p.won += 1;
                    } else {
                        p.last_loss_at = Some(at);
                    }
                }
            }
        }
        Ok(p)
    }

    /// Orders placed since `since` (Unix seconds), by one strategy or all, on a venue.
    pub fn orders_since(&self, strategy: Option<i64>, venue: Venue, since: i64) -> Result<u32> {
        let n: i64 = self.conn.prepare_cached(
            "SELECT COUNT(*) FROM orders WHERE venue = ?1 AND created_at >= ?2 AND (?3 IS NULL OR strategy_id = ?3) AND status != 'failed'",
        )?.query_row(params![venue.as_str(), since, strategy], |r| r.get(0))?;
        Ok(n as u32)
    }

    /// A strategy's sells on a venue that failed since `since`: how many, and when the last one
    /// did. An exit that failed is tried again under a new cause, not answered by the dead order.
    pub fn failed_sells_since(&self, strategy: i64, venue: Venue, since: i64) -> Result<(u32, Option<i64>)> {
        let (n, last): (i64, Option<i64>) = self.conn.prepare_cached(
            "SELECT COUNT(*), MAX(updated_at) FROM orders WHERE strategy_id = ?1 AND venue = ?2 AND side = 'sell' AND status = 'failed' AND created_at >= ?3",
        )?.query_row(params![strategy, venue.as_str(), since], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok((n as u32, last))
    }

    /// Fees paid on a venue since `since`.
    pub fn fees_since(&self, venue: Venue, since: i64) -> Result<Decimal> {
        let mut st = self.conn.prepare_cached("SELECT fee FROM fills WHERE venue = ?1 AND at >= ?2")?;
        let fees = st.query_map(params![venue.as_str(), since], |r| r.get::<_, String>(0))?.filter_map(|f| f.ok()).map(dec0).sum();
        Ok(fees)
    }

    /// Paper cash: what paper started with, less what paper buys spent, plus what paper sells
    /// brought in.
    pub fn paper_cash_now(&self) -> Result<Decimal> {
        let mut cash = self.paper_cash()?;
        let mut st = self.conn.prepare_cached("SELECT side, price, size, fee FROM fills WHERE venue = 'paper'")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (price, size, fee) = (dec0(r.get(1)?), dec0(r.get(2)?), dec0(r.get(3)?));
            match Side::parse(&r.get::<_, String>(0)?) {
                Some(Side::Sell) => cash += price * size - fee,
                _ => cash -= price * size + fee,
            }
        }
        Ok(cash)
    }

    /// Paper starts over: its orders, fills, proposals and equity go; the decision log keeps a
    /// line saying so.
    pub fn reset_paper(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM fills WHERE venue = 'paper';
             DELETE FROM orders WHERE venue = 'paper';
             DELETE FROM equity WHERE venue = 'paper';
             UPDATE strategies SET peak = NULL, last_bar = NULL WHERE venue = 'paper';",
        )?;
        self.log(None, "setting", "Paper trading started over.", None)?;
        Ok(())
    }

    // ------------------------------------------------------------------ decisions

    pub fn log(&self, strategy: Option<i64>, kind: &str, text: &str, order: Option<i64>) -> Result<i64> {
        self.conn.prepare_cached("INSERT INTO decisions (strategy_id, kind, text, order_id, at) VALUES (?1, ?2, ?3, ?4, ?5)")?
            .execute(params![strategy, kind, text, order, now()])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn decisions(&self, strategy: Option<i64>, limit: u32) -> Result<Vec<DecisionView>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT id, strategy_id, kind, text, order_id, at FROM decisions WHERE ?1 IS NULL OR strategy_id = ?1 ORDER BY id DESC LIMIT {}",
            limit.clamp(1, 1000)
        ))?;
        let rows = st.query_map(params![strategy], |r| {
            Ok(DecisionView { id: r.get(0)?, strategy_id: r.get(1)?, kind: r.get(2)?, text: r.get(3)?, order_id: r.get(4)?, at: rfc3339(r.get(5)?) })
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    // ------------------------------------------------------------------ proposals

    pub fn add_proposal(&self, p: &NewProposal) -> Result<i64> {
        self.conn.prepare_cached(
            "INSERT INTO proposals (kind, strategy_id, title, why, source, side, product, quote, base, is_limit, venue, draft, draft_hash, changes, thread_id, cause, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        )?.execute(params![
            p.kind, p.strategy_id, p.title, p.why, p.source.as_str(), p.side.map(Side::as_str), p.product, text(p.quote), text(p.base),
            p.limit, p.venue.map(Venue::as_str), p.draft.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
            p.draft.as_ref().map(draft_hash), serde_json::to_string(&p.changes).unwrap_or_default(), p.thread_id, p.cause,
            now(), p.expires_at
        ])?;
        let id = self.conn.last_insert_rowid();
        self.log(p.strategy_id, "proposal", &format!("{} ({} asks: {})", p.title, p.source.as_str(), p.why), None)?;
        Ok(id)
    }

    pub fn proposal(&self, id: i64) -> Result<(ProposalView, Option<String>)> {
        self.proposals_where("p.id = ?1", params![id], 1)?.pop().ok_or_else(|| BookError::NotFound(format!("There is no proposal {id}")))
    }

    pub fn proposals(&self, pending_only: bool, limit: u32) -> Result<Vec<ProposalView>> {
        let cond = if pending_only { "p.status = 'pending'" } else { "1" };
        Ok(self.proposals_where(cond, params![], limit)?.into_iter().map(|(p, _)| p).collect())
    }

    /// The pending order proposal for a cause, so the runner asks once per decision.
    /// The proposal a cause already made, unless it expired or failed: then the cause may ask
    /// again. One the person dismissed stays the answer.
    pub fn proposal_for_cause(&self, cause: &str) -> Result<Option<ProposalView>> {
        Ok(self.proposals_where("p.cause = ?1 AND p.status NOT IN ('expired', 'failed')", params![cause], 1)?.pop().map(|(p, _)| p))
    }

    fn proposals_where(&self, cond: &str, p: &[&dyn rusqlite::ToSql], limit: u32) -> Result<Vec<(ProposalView, Option<String>)>> {
        let sql = format!(
            "SELECT p.id, p.kind, p.strategy_id, s.name, p.title, p.why, p.source, p.status, p.outcome, p.side, p.product, p.quote, p.base,
                    p.is_limit, p.venue, p.draft, p.draft_hash, p.changes, p.created_at, p.expires_at, p.cause
             FROM proposals p LEFT JOIN strategies s ON s.id = p.strategy_id WHERE {cond}
             ORDER BY CASE p.status WHEN 'pending' THEN 0 ELSE 1 END, p.id DESC LIMIT {}",
            limit.clamp(1, 500)
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(p, |r| {
            let draft: Option<String> = r.get(15)?;
            let changes: String = r.get(17)?;
            Ok((
                ProposalView {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    strategy_id: r.get(2)?,
                    strategy: r.get(3)?,
                    title: r.get(4)?,
                    why: r.get(5)?,
                    source: Source::parse(&r.get::<_, String>(6)?).unwrap_or(Source::Agent),
                    status: r.get(7)?,
                    outcome: r.get(8)?,
                    side: r.get::<_, Option<String>>(9)?.and_then(|s| Side::parse(&s)),
                    product: r.get(10)?,
                    quote: dec(r.get(11)?),
                    base: dec(r.get(12)?),
                    limit: r.get(13)?,
                    venue: r.get::<_, Option<String>>(14)?.and_then(|v| Venue::parse(&v)),
                    draft: draft.and_then(|d| serde_json::from_str(&d).ok()),
                    draft_hash: r.get(16)?,
                    changes: serde_json::from_str(&changes).unwrap_or_default(),
                    created_at: rfc3339(r.get(18)?),
                    expires_at: r.get::<_, Option<i64>>(19)?.map(rfc3339),
                },
                r.get(20)?,
            ))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn resolve_proposal(&self, id: i64, status: &str, outcome: Option<&str>) -> Result<()> {
        let n = self.conn.prepare_cached("UPDATE proposals SET status = ?2, outcome = ?3, resolved_at = ?4 WHERE id = ?1 AND status = 'pending'")?
            .execute(params![id, status, outcome, now()])?;
        if n == 0 {
            return invalid("That proposal was already answered");
        }
        let (p, _) = self.proposal(id)?;
        let kind = match status {
            "approved" => "approved",
            "dismissed" => "dismissed",
            _ => "proposal",
        };
        let text = match outcome {
            Some(o) => format!("{} — {status}: {o}", p.title),
            None => format!("{} — {status}.", p.title),
        };
        self.log(p.strategy_id, kind, &text, None)?;
        Ok(())
    }

    /// Pending proposals past their time become expired. Returns how many.
    pub fn expire_proposals(&self, at: i64) -> Result<usize> {
        let mut st = self.conn.prepare_cached("SELECT id FROM proposals WHERE status = 'pending' AND expires_at IS NOT NULL AND expires_at <= ?1")?;
        let ids: Vec<i64> = st.query_map([at], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for id in &ids {
            self.resolve_proposal(*id, "expired", Some("nobody answered in time"))?;
        }
        Ok(ids.len())
    }

    // ------------------------------------------------------------------ equity

    /// A snapshot of a venue's value, at most one per 15 minutes.
    pub fn snapshot(&self, venue: Venue, at: i64, value: f64) -> Result<()> {
        let last: Option<i64> = self.conn.prepare_cached("SELECT MAX(at) FROM equity WHERE venue = ?1")?.query_row([venue.as_str()], |r| r.get(0))?;
        if last.is_none_or(|l| at - l >= 900) {
            self.conn.prepare_cached("INSERT OR REPLACE INTO equity (venue, at, value) VALUES (?1, ?2, ?3)")?.execute(params![venue.as_str(), at, value])?;
        }
        Ok(())
    }

    pub fn equity(&self, venue: Venue, since: i64) -> Result<Vec<(i64, f64)>> {
        let mut st = self.conn.prepare_cached("SELECT at, value FROM equity WHERE venue = ?1 AND at >= ?2 ORDER BY at")?;
        let rows = st.query_map(params![venue.as_str(), since], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    // ------------------------------------------------------------------ caches

    /// The cached product list, when fetched within `max_age` seconds.
    pub fn cached_products(&self, max_age: i64) -> Result<Option<Vec<Product>>> {
        let fresh: Option<i64> = self.conn.prepare_cached("SELECT MIN(fetched_at) FROM products")?.query_row([], |r| r.get(0))?;
        if fresh.is_none_or(|f| now() - f > max_age) {
            return Ok(None);
        }
        let mut st = self.conn.prepare_cached("SELECT body FROM products ORDER BY id")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?.filter_map(|b| b.ok()).filter_map(|b| serde_json::from_str(&b).ok()).collect();
        Ok(Some(rows))
    }

    /// One product from the cache, however old: its sizes and increments rarely change, and the
    /// gate still asks the exchange for a fresh price.
    pub fn cached_product(&self, id: &str) -> Result<Option<Product>> {
        let body: Option<String> = self.conn.prepare_cached("SELECT body FROM products WHERE id = ?1")?.query_row([id], |r| r.get(0)).optional()?;
        Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
    }

    pub fn store_products(&mut self, products: &[Product]) -> Result<()> {
        let t = now();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM products", [])?;
        {
            let mut st = tx.prepare_cached("INSERT INTO products (id, body, fetched_at) VALUES (?1, ?2, ?3)")?;
            for p in products {
                st.execute(params![p.id, serde_json::to_string(p).unwrap_or_default(), t])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cached_candles(&self, product: &str, g: Granularity, start: i64, end: i64) -> Result<Vec<Candle>> {
        let mut st = self.conn.prepare_cached(
            "SELECT start, open, high, low, close, volume FROM candles WHERE product = ?1 AND granularity = ?2 AND start >= ?3 AND start < ?4 ORDER BY start",
        )?;
        let rows = st.query_map(params![product, g.as_str(), start, end], |r| {
            Ok(Candle { start: r.get(0)?, open: r.get(1)?, high: r.get(2)?, low: r.get(3)?, close: r.get(4)?, volume: r.get(5)? })
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Keeps closed candles (a bar still forming would be cached half-made).
    pub fn store_candles(&mut self, product: &str, g: Granularity, candles: &[Candle]) -> Result<()> {
        let open_from = now() - g.seconds();
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare_cached("INSERT OR REPLACE INTO candles (product, granularity, start, open, high, low, close, volume) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
            for c in candles.iter().filter(|c| c.start < open_from) {
                st.execute(params![product, g.as_str(), c.start, c.open, c.high, c.low, c.close, c.volume])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// `Connection` would clash with rusqlite's.
pub type Connection_ = crate::views::Connection;

/// A proposal to record.
#[derive(Debug, Clone, Default)]
pub struct NewProposal {
    pub kind: String,
    pub strategy_id: Option<i64>,
    pub title: String,
    pub why: String,
    pub source: Option<Source>,
    pub side: Option<Side>,
    pub product: Option<String>,
    pub quote: Option<Decimal>,
    pub base: Option<Decimal>,
    pub limit: bool,
    pub venue: Option<Venue>,
    pub draft: Option<Draft>,
    pub changes: Vec<String>,
    pub thread_id: Option<i64>,
    pub cause: Option<String>,
    pub expires_at: Option<i64>,
}

trait SourceStr {
    fn as_str(&self) -> &'static str;
}

impl SourceStr for Option<Source> {
    fn as_str(&self) -> &'static str {
        self.map_or("agent", Source::as_str)
    }
}

/// What an approval binds to: the whole draft, name and limits included.
pub fn draft_hash(d: &Draft) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_string(d).unwrap_or_default();
    Sha256::digest(json.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn opt_money(d: Option<Decimal>) -> String {
    d.map_or("none".into(), |d| d.normalize().to_string())
}

pub fn limits_text(l: &Limits) -> String {
    format!(
        "most per order {}, most held {}, daily loss {}, orders per hour {}, cooldown {}",
        opt_money(l.max_order),
        opt_money(l.max_position),
        opt_money(l.daily_loss),
        l.orders_per_hour.map_or("none".into(), |n| n.to_string()),
        l.cooldown_minutes.map_or("none".into(), |n| format!("{n} min"))
    )
}

/// What changes from `a` to `b`, as sentences for a proposal card or the log.
pub fn changes(a: &Draft, b: &Draft) -> Vec<String> {
    let mut out = Vec::new();
    if a.name.trim() != b.name.trim() {
        out.push(format!("Name: {} → {}.", a.name.trim(), b.name.trim()));
    }
    if a.product != b.product {
        out.push(format!("Product: {} → {}.", a.product, b.product));
    }
    if a.granularity != b.granularity {
        out.push(format!("Bars: {} → {}.", a.granularity.short(), b.granularity.short()));
    }
    if a.rule != b.rule {
        let (sa, sb) = (rules::describe(&a.rule, &a.product, a.granularity), rules::describe(&b.rule, &b.product, b.granularity));
        for (x, y) in sa.iter().zip(sb.iter()) {
            if x != y {
                out.push(format!("Was: {x} Now: {y}"));
            }
        }
    }
    if a.limits != b.limits {
        out.push(format!("Limits: {}.", limits_text(&b.limits)));
    }
    out
}

/// A row's "doing" line: what the strategy waits for or holds.
pub fn doing(s: &StrategyRec, p: &Position, open: usize) -> String {
    match s.state {
        RunState::Halted => return format!("Halted: {}", s.halt_reason.as_deref().unwrap_or("halted")),
        RunState::Stopped => return "Stopped".into(),
        RunState::Running => {}
    }
    if open > 0 {
        return "An order is open".into();
    }
    if p.base > Decimal::ZERO {
        let x = &s.rule.exit;
        let mut parts = Vec::new();
        if let Some(t) = x.take_profit_pct {
            parts.push(format!("+{}%", t.normalize()));
        }
        if let Some(t) = x.stop_loss_pct {
            parts.push(format!("−{}%", t.normalize()));
        }
        return if parts.is_empty() { "Holding".into() } else { format!("Holding: sells at {}", parts.join(" or ")) };
    }
    match &s.rule.entry {
        Entry::Schedule { .. } => rules::next_scheduled(&s.rule.entry, now())
            .map(|t| format!("Next buy {}", jiff::Timestamp::from_second(t).map(|t| t.strftime("%a %d %b %H:%M UTC").to_string()).unwrap_or_default()))
            .unwrap_or_else(|| "Waiting for its schedule".into()),
        Entry::Signal { when } => format!("Waiting for {}", rules::condition(when)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Draft {
        Draft {
            name: "ETH dip".into(),
            product: "ETH-CAD".into(),
            granularity: Granularity::OneHour,
            rule: Rule {
                entry: Entry::Signal { when: Condition::Compare { left: Operand::Rsi { period: 14 }, op: Compare::Below, right: Operand::Number { value: 30.0 } } },
                buy: Decimal::new(50, 0),
                pricing: Pricing::Market,
                exit: Exit { take_profit_pct: Some(Decimal::new(4, 0)), ..Default::default() },
            },
            limits: Limits { max_order: Some(Decimal::new(50, 0)), ..Default::default() },
        }
    }

    fn order(book: &Book, sid: i64, side: Side, cid: &str) -> i64 {
        let req = OrderRequest { client_order_id: cid.into(), product: "ETH-CAD".into(), side, quote_size: Some(Decimal::new(50, 0)), base_size: None, limit_price: None };
        book.insert_order(&NewOrder { strategy_id: Some(sid), venue: Venue::Paper, source: Source::Rule, request: &req, why: "test" }).unwrap().0
    }

    fn fill(id: &str, price: i64, size: Decimal, fee: Decimal, at: i64) -> ExchangeFill {
        ExchangeFill { trade_id: id.into(), order_id: "x".into(), price: Decimal::new(price, 0), size, fee, at }
    }

    #[test]
    fn positions_come_back_from_fills_and_a_fill_counts_once() {
        let book = Book::open_in_memory().unwrap();
        let sid = book.create_strategy(&draft()).unwrap();
        let buy = order(&book, sid, Side::Buy, "a");
        assert!(book.add_fill(buy, Venue::Paper, &fill("t1", 100, Decimal::new(5, 1), Decimal::new(3, 1), now())).unwrap().is_some());
        assert!(book.add_fill(buy, Venue::Paper, &fill("t1", 100, Decimal::new(5, 1), Decimal::new(3, 1), now())).unwrap().is_none());
        let p = book.position(sid, Venue::Paper).unwrap();
        assert_eq!(p.base, Decimal::new(5, 1));
        assert_eq!(p.cost, Decimal::new(503, 1)); // 50 + 0.3 fee
        let sell = order(&book, sid, Side::Sell, "b");
        book.add_fill(sell, Venue::Paper, &fill("t2", 110, Decimal::new(5, 1), Decimal::new(3, 1), now())).unwrap();
        let p = book.position(sid, Venue::Paper).unwrap();
        assert_eq!(p.base, Decimal::ZERO);
        // 55 - 0.3 - 50.3
        assert_eq!(p.realized, Decimal::new(44, 1));
        assert_eq!((p.sells, p.won), (1, 1));
        assert_eq!(book.paper_cash_now().unwrap(), Decimal::new(1000, 0) + Decimal::new(44, 1));
    }

    #[test]
    fn a_fill_reported_in_quote_is_booked_in_the_base_increment() {
        let mut book = Book::open_in_memory().unwrap();
        let product = Product {
            id: "ETH-CAD".into(), base: "ETH".into(), quote: "CAD".into(), base_increment: Decimal::new(1, 8),
            quote_increment: Decimal::new(1, 2), price_increment: Decimal::new(1, 2), base_min_size: Decimal::new(1, 8),
            quote_min_size: Decimal::ONE, price: None, change_24h: None, tradable: true, limit_only: false,
        };
        book.store_products(&[product]).unwrap();
        let sid = book.create_strategy(&draft()).unwrap();
        let buy = order(&book, sid, Side::Buy, "a");
        // 50 spent at 3 per coin: 16.666… to 28 places, which no sell floored to 1e-8 empties.
        let size = Decimal::new(50, 0) / Decimal::new(3, 0);
        book.add_fill(buy, Venue::Paper, &fill("t1", 3, size, Decimal::ZERO, now())).unwrap();
        assert_eq!(book.position(sid, Venue::Paper).unwrap().base, Decimal::new(1666666667, 8));
        let sell = order(&book, sid, Side::Sell, "b");
        book.add_fill(sell, Venue::Paper, &fill("t2", 3, Decimal::new(1666666667, 8), Decimal::ZERO, now())).unwrap();
        assert!(book.position(sid, Venue::Paper).unwrap().base.is_zero());
    }

    #[test]
    fn a_cause_asks_again_once_its_card_expired_but_not_once_dismissed() {
        let book = Book::open_in_memory().unwrap();
        let sid = book.create_strategy(&draft()).unwrap();
        let ask = |cause: &str| book.add_proposal(&NewProposal { kind: "order".into(), strategy_id: Some(sid), title: "Sell".into(), why: "stop".into(), cause: Some(cause.into()), expires_at: Some(now() - 1), ..Default::default() }).unwrap();
        ask("stop");
        book.expire_proposals(now()).unwrap();
        assert!(book.proposal_for_cause("stop").unwrap().is_none());
        let id = ask("again");
        book.resolve_proposal(id, "dismissed", None).unwrap();
        assert!(book.proposal_for_cause("again").unwrap().is_some());
    }

    #[test]
    fn a_failed_sell_is_counted_from_when_the_holding_opened() {
        let book = Book::open_in_memory().unwrap();
        let sid = book.create_strategy(&draft()).unwrap();
        let sell = order(&book, sid, Side::Sell, "s1");
        assert_eq!(book.failed_sells_since(sid, Venue::Paper, 0).unwrap(), (0, None));
        book.fail_order(sell, "refused").unwrap();
        let (n, last) = book.failed_sells_since(sid, Venue::Paper, 0).unwrap();
        assert_eq!(n, 1);
        assert!(last.is_some());
        assert_eq!(book.failed_sells_since(sid, Venue::Paper, now() + 10).unwrap().0, 0);
    }

    #[test]
    fn the_same_client_order_id_is_the_same_order() {
        let book = Book::open_in_memory().unwrap();
        let sid = book.create_strategy(&draft()).unwrap();
        assert_eq!(order(&book, sid, Side::Buy, "same"), order(&book, sid, Side::Buy, "same"));
    }

    #[test]
    fn the_decision_log_cannot_be_rewritten() {
        let book = Book::open_in_memory().unwrap();
        let id = book.log(None, "buy", "bought", None).unwrap();
        assert!(book.conn.execute("UPDATE decisions SET text = 'sold' WHERE id = ?1", [id]).is_err());
        assert!(book.conn.execute("DELETE FROM decisions WHERE id = ?1", [id]).is_err());
    }

    #[test]
    fn the_ai_cannot_trade_without_limits() {
        let book = Book::open_in_memory().unwrap();
        let mut d = draft();
        d.limits = Limits::default();
        let sid = book.create_strategy(&d).unwrap();
        assert!(book.set_mode(sid, Mode::Agent).is_err());
        assert!(book.set_mode(sid, Mode::Ask).is_ok());
        d.limits = Limits { max_order: Some(Decimal::new(50, 0)), max_position: Some(Decimal::new(200, 0)), daily_loss: Some(Decimal::new(15, 0)), ..Default::default() };
        book.set_strategy_limits(sid, &d.limits).unwrap();
        assert!(book.set_mode(sid, Mode::Agent).is_ok());
        // And its limits cannot then be lifted while it trades.
        assert!(book.set_strategy_limits(sid, &Limits::default()).is_err());
    }

    #[test]
    fn a_buy_bigger_than_its_order_limit_is_refused_at_save() {
        let book = Book::open_in_memory().unwrap();
        let mut d = draft();
        d.rule.buy = Decimal::new(60, 0);
        assert!(book.create_strategy(&d).is_err());
    }

    #[test]
    fn proposals_expire_and_answer_once() {
        let book = Book::open_in_memory().unwrap();
        let id = book.add_proposal(&NewProposal { kind: "order".into(), title: "Buy".into(), why: "dip".into(), expires_at: Some(now() - 1), ..Default::default() }).unwrap();
        assert_eq!(book.expire_proposals(now()).unwrap(), 1);
        assert_eq!(book.proposal(id).unwrap().0.status, "expired");
        assert!(book.resolve_proposal(id, "approved", None).is_err());
    }

    #[test]
    fn changes_read_as_sentences() {
        let a = draft();
        let mut b = draft();
        b.rule.exit.take_profit_pct = Some(Decimal::new(3, 0));
        let c = changes(&a, &b);
        assert_eq!(c.len(), 1);
        assert!(c[0].contains("up 4%") && c[0].contains("up 3%"), "{c:?}");
    }
}
