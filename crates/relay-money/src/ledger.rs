//! The PC's copy of the ledger: Tally's tables in their own SQLite file (`money.db`), and the
//! readings the desktop draws from them.
//!
//! Apart from the dev store on purpose (docs/MONEY.md): its own schema versions, its own backups,
//! and the file the phone will sync with. Every row has a local `id`, a permanent `uid` sync will
//! use, `updated_at` (ms) and `deleted`. The sums follow Tally's DAOs (`Daos.kt`) so a backup read
//! here shows the numbers the phone shows.

use crate::backup::{
    AccountDto, AccountValueDto, ActivityDto, BackupFile, BudgetDto, CategoryDto, ContributionDto, FxRateDto, GoalDto, HoldingDto,
    PriceDto, RecurringDto, RoomFactDto, SecurityDto, TransactionDto,
};
use crate::copy;
use crate::csv::kt_is_blank;
use crate::model::{AccountType, ActivityType, CategoryKind, GoalKind, Registration, SecurityKind, TxType, DEFAULT_CATEGORIES};
use crate::money::{fraction_digits, Locale, MoneyFormatter};
use crate::pace::{PaceReading, PaceStatus};
use crate::period::{days_between, BudgetPeriod};
use crate::recurrence::{Frequency, Recurrence};
use crate::text_match;
pub use crate::views::*;
use jiff::civil::Date;
use jiff::ToSpan;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Bumped with every entry appended to [`MIGRATIONS`]; every earlier version must stay openable.
pub const SCHEMA_VERSION: i64 = 4;

const MIGRATIONS: &[&str] = &[r"
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL, type TEXT NOT NULL,
    opening_balance INTEGER NOT NULL DEFAULT 0, archived INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE categories (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL, kind TEXT NOT NULL,
    color INTEGER NOT NULL DEFAULT 0, icon TEXT NOT NULL DEFAULT 'dots', archived INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE transactions (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, type TEXT NOT NULL, amount INTEGER NOT NULL,
    date TEXT NOT NULL, account_id INTEGER NOT NULL, to_account_id INTEGER, category_id INTEGER,
    note TEXT NOT NULL DEFAULT '', recurring_id INTEGER, created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE INDEX transactions_date ON transactions (date);
CREATE TABLE budgets (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE,
    -- 0 is the overall monthly budget, as Room's BudgetEntity.OVERALL.
    category_id INTEGER NOT NULL UNIQUE, amount INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE recurring (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL, type TEXT NOT NULL,
    amount INTEGER NOT NULL, account_id INTEGER NOT NULL, to_account_id INTEGER, category_id INTEGER,
    frequency TEXT NOT NULL, interval INTEGER NOT NULL DEFAULT 1, anchor_date TEXT NOT NULL,
    next_date TEXT NOT NULL, end_date TEXT, auto_post INTEGER NOT NULL DEFAULT 1, active INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE goals (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL, target INTEGER NOT NULL,
    target_date TEXT, color INTEGER NOT NULL DEFAULT 0, archived INTEGER NOT NULL DEFAULT 0,
    kind TEXT NOT NULL DEFAULT 'SAVINGS', account_id INTEGER, percent INTEGER NOT NULL DEFAULT 0,
    start_date TEXT, start_amount INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE contributions (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, goal_id INTEGER NOT NULL, amount INTEGER NOT NULL,
    date TEXT NOT NULL, note TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE account_values (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, account_id INTEGER NOT NULL, date TEXT NOT NULL,
    value INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
", r"
-- Sync (docs/MONEY.md): every change takes the next number from one counter, so a device that
-- last synced at cursor N asks for everything numbered above N. Rows erased outright (a restore,
-- an erase) leave a tombstone, which takes a number too.
CREATE TABLE sync_counter (n INTEGER NOT NULL);
INSERT INTO sync_counter (n) VALUES (0);
CREATE TABLE tombstones (tbl TEXT NOT NULL, uid TEXT NOT NULL, updated_at INTEGER NOT NULL, seq INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (tbl, uid));
ALTER TABLE settings ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE accounts ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX accounts_seq ON accounts (seq);
CREATE TRIGGER accounts_seq_insert AFTER INSERT ON accounts BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE accounts SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER accounts_seq_update AFTER UPDATE ON accounts WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE accounts SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE categories ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX categories_seq ON categories (seq);
CREATE TRIGGER categories_seq_insert AFTER INSERT ON categories BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE categories SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER categories_seq_update AFTER UPDATE ON categories WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE categories SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE transactions ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX transactions_seq ON transactions (seq);
CREATE TRIGGER transactions_seq_insert AFTER INSERT ON transactions BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE transactions SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER transactions_seq_update AFTER UPDATE ON transactions WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE transactions SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE budgets ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX budgets_seq ON budgets (seq);
CREATE TRIGGER budgets_seq_insert AFTER INSERT ON budgets BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE budgets SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER budgets_seq_update AFTER UPDATE ON budgets WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE budgets SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE recurring ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX recurring_seq ON recurring (seq);
CREATE TRIGGER recurring_seq_insert AFTER INSERT ON recurring BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE recurring SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER recurring_seq_update AFTER UPDATE ON recurring WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE recurring SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE goals ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX goals_seq ON goals (seq);
CREATE TRIGGER goals_seq_insert AFTER INSERT ON goals BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE goals SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER goals_seq_update AFTER UPDATE ON goals WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE goals SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE contributions ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX contributions_seq ON contributions (seq);
CREATE TRIGGER contributions_seq_insert AFTER INSERT ON contributions BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE contributions SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER contributions_seq_update AFTER UPDATE ON contributions WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE contributions SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
ALTER TABLE account_values ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX account_values_seq ON account_values (seq);
CREATE TRIGGER account_values_seq_insert AFTER INSERT ON account_values BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE account_values SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER account_values_seq_update AFTER UPDATE ON account_values WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE account_values SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER settings_seq_insert AFTER INSERT ON settings BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE settings SET seq = (SELECT n FROM sync_counter) WHERE key = NEW.key; END;
CREATE TRIGGER settings_seq_update AFTER UPDATE ON settings WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE settings SET seq = (SELECT n FROM sync_counter) WHERE key = NEW.key; END;
CREATE TRIGGER tombstones_seq_insert AFTER INSERT ON tombstones BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE tombstones SET seq = (SELECT n FROM sync_counter) WHERE tbl = NEW.tbl AND uid = NEW.uid; END;
CREATE TRIGGER tombstones_seq_update AFTER UPDATE ON tombstones WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE tombstones SET seq = (SELECT n FROM sync_counter) WHERE tbl = NEW.tbl AND uid = NEW.uid; END;
-- Rows that existed before sync count as changed once, so a first sync carries them.
UPDATE accounts SET seq = 0;
UPDATE categories SET seq = 0;
UPDATE transactions SET seq = 0;
UPDATE budgets SET seq = 0;
UPDATE recurring SET seq = 0;
UPDATE goals SET seq = 0;
UPDATE contributions SET seq = 0;
UPDATE account_values SET seq = 0;
UPDATE settings SET seq = 0;
", r"
-- A budget's tombstone names its category's uid: budgets are matched by category on sync.
ALTER TABLE tombstones ADD COLUMN category TEXT;
", r"
-- Investments (docs/INVESTMENTS.md). An investment account's registration (null on the others),
-- its institution and the institution's own number for it; then what is held, what was done,
-- prices, rates and the CRA's room figures. Positions, value and room are read from these, never stored.
ALTER TABLE accounts ADD COLUMN registration TEXT;
ALTER TABLE accounts ADD COLUMN institution TEXT NOT NULL DEFAULT '';
ALTER TABLE accounts ADD COLUMN external_ref TEXT NOT NULL DEFAULT '';
CREATE TABLE securities (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, symbol TEXT NOT NULL, name TEXT NOT NULL DEFAULT '',
    currency TEXT NOT NULL, kind TEXT NOT NULL, exchange TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
-- A snapshot: an account's holdings are its rows on their newest date. `book` is in the ledger
-- currency, `book_market` in the security's; `quantity` at 1e-8 of a unit.
CREATE TABLE holdings (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, account_id INTEGER NOT NULL, security_id INTEGER NOT NULL,
    date TEXT NOT NULL, quantity INTEGER NOT NULL, book INTEGER NOT NULL, book_market INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
CREATE INDEX holdings_account ON holdings (account_id, date);
-- Never `transactions`: buys, sells and dividends are not spending or income.
CREATE TABLE activities (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, account_id INTEGER NOT NULL, security_id INTEGER,
    type TEXT NOT NULL, date TEXT NOT NULL, quantity INTEGER NOT NULL DEFAULT 0, amount INTEGER NOT NULL,
    fee INTEGER NOT NULL DEFAULT 0, currency TEXT NOT NULL, to_amount INTEGER, to_currency TEXT,
    note TEXT NOT NULL DEFAULT '', source TEXT NOT NULL DEFAULT 'MANUAL', created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
CREATE INDEX activities_account ON activities (account_id, date);
CREATE TABLE prices (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, security_id INTEGER NOT NULL, date TEXT NOT NULL,
    price INTEGER NOT NULL, source TEXT NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
CREATE INDEX prices_security ON prices (security_id, date);
CREATE TABLE fx_rates (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, base TEXT NOT NULL, quote TEXT NOT NULL, date TEXT NOT NULL,
    rate INTEGER NOT NULL, source TEXT NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
CREATE TABLE room_facts (
    id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, registration TEXT NOT NULL, year INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, seq INTEGER NOT NULL DEFAULT 0);
-- Sync numbers every change to these as migration 2 numbers the first eight.
CREATE INDEX securities_seq ON securities (seq);
CREATE TRIGGER securities_seq_insert AFTER INSERT ON securities BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE securities SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER securities_seq_update AFTER UPDATE ON securities WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE securities SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE INDEX holdings_seq ON holdings (seq);
CREATE TRIGGER holdings_seq_insert AFTER INSERT ON holdings BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE holdings SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER holdings_seq_update AFTER UPDATE ON holdings WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE holdings SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE INDEX activities_seq ON activities (seq);
CREATE TRIGGER activities_seq_insert AFTER INSERT ON activities BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE activities SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER activities_seq_update AFTER UPDATE ON activities WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE activities SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE INDEX prices_seq ON prices (seq);
CREATE TRIGGER prices_seq_insert AFTER INSERT ON prices BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE prices SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER prices_seq_update AFTER UPDATE ON prices WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE prices SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE INDEX fx_rates_seq ON fx_rates (seq);
CREATE TRIGGER fx_rates_seq_insert AFTER INSERT ON fx_rates BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE fx_rates SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER fx_rates_seq_update AFTER UPDATE ON fx_rates WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE fx_rates SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE INDEX room_facts_seq ON room_facts (seq);
CREATE TRIGGER room_facts_seq_insert AFTER INSERT ON room_facts BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE room_facts SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
CREATE TRIGGER room_facts_seq_update AFTER UPDATE ON room_facts WHEN NEW.seq = OLD.seq BEGIN
    UPDATE sync_counter SET n = n + 1; UPDATE room_facts SET seq = (SELECT n FROM sync_counter) WHERE id = NEW.id; END;
"];

const TABLES: [&str; 14] = [
    "accounts", "categories", "transactions", "budgets", "recurring", "goals", "contributions", "account_values",
    "securities", "holdings", "activities", "prices", "fx_rates", "room_facts",
];

#[derive(Debug)]
pub enum LedgerError {
    Sql(rusqlite::Error),
    /// A refusal the person can act on: a missing account, an amount of zero.
    Invalid(String),
    NotFound(String),
    /// A device synced with a ledger this one no longer is (restored, erased, replaced by
    /// another device, or a fresh file): it must take this ledger whole before it merges again.
    Stale(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::Sql(e) => write!(f, "money ledger: {e}"),
            LedgerError::Invalid(m) | LedgerError::NotFound(m) | LedgerError::Stale(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<rusqlite::Error> for LedgerError {
    fn from(e: rusqlite::Error) -> Self {
        LedgerError::Sql(e)
    }
}

pub type Result<T> = std::result::Result<T, LedgerError>;

pub(crate) fn invalid<T>(m: impl Into<String>) -> Result<T> {
    Err(LedgerError::Invalid(m.into()))
}

pub(crate) fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// How far ahead of this clock a device's stamp may be (docs/MONEY.md, "Sync"). A phone a few
/// hours fast is ordinary; a stamp days ahead, or near the end of i64, is a broken clock or a
/// forged change: no edit here could outrank it, and an edit's `+ 1` past i64 turned the column
/// into a real that every later sync failed to read.
pub const STAMP_AHEAD_MS: i64 = 24 * 60 * 60 * 1000;

/// Where the stamp arithmetic saturates, written out in the SQL as `MIN(updated_at,
/// 4000000000000000) + 1`: the year 128,000, far past any clock and far from an overflow.
pub const STAMP_CEILING: i64 = 4_000_000_000_000_000;

/// A stamp this ledger takes from a device: 0 (what an older ledger's settings carry) to a day
/// ahead of `now`. Tally's `SyncTables.stampOk`.
pub fn stamp_ok(at: i64, now: i64) -> bool {
    (0..=now.saturating_add(STAMP_AHEAD_MS)).contains(&at)
}

pub(crate) fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn name_of<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

pub(crate) fn parse_name<T: for<'de> Deserialize<'de>>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

pub fn parse_date(s: &str) -> Option<Date> {
    crate::csv::parse_iso_date(s)
}

/// A bill that posts itself: id, uid, type, amount, account, to-account, category, frequency,
/// interval, anchor, next date, end date.
type DueRow = (i64, String, String, i64, i64, Option<i64>, Option<i64>, String, i64, String, String, Option<String>);

/// A reminder whose day is over: id, frequency, interval, anchor, end date.
type ReminderRow = (i64, String, i64, String, Option<String>);

/// The most dates one bill posts in one pass, as Tally's `RecurringPoster.CATCH_UP_LIMIT`: a weekly
/// bill a year overdue. A longer backlog is paid over the passes that follow, never dropped.
pub const CATCH_UP_LIMIT: usize = 60;

/// The largest amount an entry may have, in minor units: ten integer digits with up to three
/// decimals, as far as the keypad goes. A thousand of them still sum far inside an `i64`.
pub const MAX_AMOUNT: i64 = 10_i64.pow(13) - 1;

/// How many periods a reading may step from today's, either way: a century of months.
pub const MAX_PERIOD_OFFSET: i64 = 1200;

pub struct Ledger {
    pub(crate) conn: Connection,
    /// The file, for the snapshot a replace keeps beside it (`sync.rs`); `None` in memory.
    pub(crate) path: Option<PathBuf>,
}

/// 0600 on the ledger and its `-wal` and `-shm` files: every account, balance and payee, which
/// other local users must not read where the folder lets them in. Run once the ledger is in WAL
/// mode; later side files take the ledger's mode. A file someone else owns stays as it is.
#[cfg(unix)]
fn private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for side in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(side);
        let _ = std::fs::set_permissions(Path::new(&name), std::fs::Permissions::from_mode(0o600));
    }
}

#[cfg(not(unix))]
fn private(_path: &Path) {}

impl Ledger {
    pub fn open(path: &Path) -> Result<Ledger> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        private(path);
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ledger::init(conn, Some(path.to_path_buf()))
    }

    pub fn open_in_memory() -> Result<Ledger> {
        Ledger::init(Connection::open_in_memory()?, None)
    }

    fn init(conn: Connection, path: Option<PathBuf>) -> Result<Ledger> {
        let mut ledger = Ledger { conn, path };
        ledger.migrate()?;
        ledger.repair_stamps()?;
        Ok(ledger)
    }

    fn migrate(&mut self) -> Result<()> {
        let have: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if have > SCHEMA_VERSION {
            return invalid(format!("money.db is version {have}; this build reads up to {SCHEMA_VERSION}"));
        }
        let tx = self.conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(have as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Brings back a ledger a device stamped out of range before [`stamp_ok`] was checked: a stamp
    /// more than a day ahead, or one an edit's `+ 1` overflowed into a real, becomes now. Such a row
    /// could not be edited again, and every sync failed reading it. Renumbered by the change,
    /// the rows go out again with their new stamps. Run on every open; it finds nothing after the
    /// first.
    fn repair_stamps(&mut self) -> Result<usize> {
        let now = now_ms();
        let tx = self.conn.transaction()?;
        let mut n = 0;
        for t in TABLES.iter().copied().chain(["tombstones", "settings"]) {
            n += tx.execute(
                &format!("UPDATE {t} SET updated_at = ?1 WHERE typeof(updated_at) <> 'integer' OR updated_at > ?2"),
                params![now, now.saturating_add(STAMP_AHEAD_MS)],
            )?;
        }
        tx.commit()?;
        Ok(n)
    }

    fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?.query_row([key], |r| r.get(0)).optional()?)
    }

    fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.prepare_cached(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = MAX(excluded.updated_at, MIN(settings.updated_at, 4000000000000000) + 1)",
        )?.execute(params![key, value, now_ms()])?;
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings> {
        let currency = self.setting("currency")?.filter(|c| fraction_digits(c).is_some()).unwrap_or_else(|| "CAD".into());
        let month_start_day = self.setting("month_start_day")?.and_then(|v| v.parse().ok()).unwrap_or(1);
        let week_starts_monday = self.setting("week_starts_monday")?.is_none_or(|v| v == "1");
        Ok(Settings { currency, month_start_day, week_starts_monday })
    }

    pub fn is_empty(&self) -> Result<bool> {
        let n: i64 = self.conn.prepare_cached(
            "SELECT (SELECT COUNT(*) FROM accounts WHERE deleted = 0) + (SELECT COUNT(*) FROM transactions WHERE deleted = 0)",
        )?.query_row([], |r| r.get(0))?;
        Ok(n == 0)
    }

    fn formatter(&self, locale: &Locale) -> Result<MoneyFormatter> {
        Ok(MoneyFormatter::new(&self.settings()?.currency, locale.clone()))
    }

    fn period(&self, today: Date) -> Result<BudgetPeriod> {
        Ok(BudgetPeriod::containing(today, self.settings()?.month_start_day.clamp(1, 28) as i8))
    }

    /// (income, spent) between two dates, end exclusive; transfers count as neither.
    fn flows(&self, start: Date, end_exclusive: Date) -> Result<(i64, i64)> {
        Ok(self.conn.prepare_cached(
            "SELECT COALESCE(SUM(CASE WHEN type = 'INCOME' THEN amount END), 0),
                    COALESCE(SUM(CASE WHEN type = 'EXPENSE' THEN amount END), 0)
             FROM transactions WHERE deleted = 0 AND date >= ?1 AND date < ?2",
        )?.query_row([start.to_string(), end_exclusive.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?)
    }

    /// Opening balance, plus income, minus expenses and outgoing transfers, plus incoming ones. An
    /// account with a recorded value starts from its newest value instead, and counts only the
    /// entries dated after it: the value already holds everything up to its day.
    pub fn accounts(&self) -> Result<Vec<AccountView>> {
        let mut st = self.conn.prepare_cached(
            "SELECT a.id, a.name, a.type, a.archived,
                    COALESCE(v.value, a.opening_balance)
                    + COALESCE((SELECT SUM(CASE WHEN t.type = 'INCOME' THEN t.amount ELSE -t.amount END)
                                FROM transactions t WHERE t.deleted = 0 AND t.account_id = a.id AND (v.date IS NULL OR t.date > v.date)), 0)
                    + COALESCE((SELECT SUM(t.amount) FROM transactions t
                                WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND t.to_account_id = a.id AND (v.date IS NULL OR t.date > v.date)), 0),
                    a.registration, a.institution
             FROM accounts a
             LEFT JOIN account_values v ON v.id = (
                 SELECT v2.id FROM account_values v2 WHERE v2.deleted = 0 AND v2.account_id = a.id ORDER BY v2.date DESC, v2.id DESC LIMIT 1)
             WHERE a.deleted = 0
             ORDER BY a.archived, a.sort_order, a.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(AccountView {
                id: r.get(0)?,
                name: r.get(1)?,
                r#type: parse_name(&r.get::<_, String>(2)?).unwrap_or(AccountType::Chequing),
                archived: r.get(3)?,
                balance: r.get(4)?,
                registration: r.get::<_, Option<String>>(5)?.as_deref().and_then(parse_name),
                institution: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn categories(&self) -> Result<Vec<CategoryView>> {
        let mut st = self.conn.prepare_cached(
            "SELECT id, name, kind, icon, color, archived FROM categories WHERE deleted = 0 ORDER BY archived, kind, sort_order, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(CategoryView {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: parse_name(&r.get::<_, String>(2)?).unwrap_or(CategoryKind::Expense),
                icon: r.get(3)?,
                color: r.get(4)?,
                archived: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn lists(&self) -> Result<Lists> {
        let s = self.settings()?;
        let accounts = self.accounts()?;
        Ok(Lists {
            fraction_digits: fraction_digits(&s.currency).unwrap_or(2),
            currency: s.currency,
            // As money.summary counts it: every account still in use.
            net_worth: accounts.iter().filter(|a| !a.archived).map(|a| a.balance).sum(),
            accounts,
            categories: self.categories()?,
            devices: self.synced_devices()?.into_iter().map(|(name, last_sync)| DeviceView { name, last_sync }).collect(),
        })
    }

    const TX_SELECT: &'static str = "SELECT t.id, t.uid, t.type, t.amount, t.date, t.account_id, a.name, t.to_account_id, b.name,
                t.category_id, c.name, c.icon, c.color, t.note
         FROM transactions t
         LEFT JOIN accounts a ON a.id = t.account_id
         LEFT JOIN accounts b ON b.id = t.to_account_id
         LEFT JOIN categories c ON c.id = t.category_id";

    fn tx_row(r: &rusqlite::Row) -> rusqlite::Result<Tx> {
        Ok(Tx {
            id: r.get(0)?,
            uid: r.get(1)?,
            r#type: parse_name(&r.get::<_, String>(2)?).unwrap_or(TxType::Expense),
            amount: r.get(3)?,
            date: r.get(4)?,
            account_id: r.get(5)?,
            account: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
            to_account_id: r.get(7)?,
            to_account: r.get(8)?,
            category_id: r.get(9)?,
            category: r.get(10)?,
            icon: r.get(11)?,
            color: r.get(12)?,
            note: r.get(13)?,
        })
    }

    pub fn tx(&self, id: i64) -> Result<Tx> {
        self.conn.prepare_cached(&format!("{} WHERE t.id = ?1 AND t.deleted = 0", Self::TX_SELECT))?
            .query_row([id], Self::tx_row)
            .optional()?
            .ok_or_else(|| LedgerError::NotFound(format!("No entry {id}")))
    }

    fn recent(&self, n: u32) -> Result<Vec<Tx>> {
        let mut st = self.conn.prepare_cached(&format!("{} WHERE t.deleted = 0 ORDER BY t.date DESC, t.id DESC LIMIT ?1", Self::TX_SELECT))?;
        let rows = st.query_map([n], Self::tx_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// One budget period's entries, newest first, narrowed by a search over the note, the
    /// category and the account in any case and alphabet ([`text_match`]).
    pub fn tx_list(&self, q: &TxQuery, today: Date) -> Result<TxPage> {
        let period = self.period(today)?.shift(q.period_offset.unwrap_or(0).clamp(-MAX_PERIOD_OFFSET, MAX_PERIOD_OFFSET));
        let pattern = q.query.as_deref().map(text_match::contains_pattern).unwrap_or_default();
        let mut st = self.conn.prepare_cached(&format!(
            "{} WHERE t.deleted = 0 AND t.date >= ?1 AND t.date < ?2
               AND (?3 = '' OR t.note GLOB ?3 OR COALESCE(c.name, '') GLOB ?3 OR COALESCE(a.name, '') GLOB ?3)
               AND (?4 IS NULL OR t.account_id = ?4 OR t.to_account_id = ?4)
               AND (?5 IS NULL OR t.category_id = ?5)
             ORDER BY t.date DESC, t.id DESC LIMIT ?6",
            Self::TX_SELECT
        ))?;
        let rows = st.query_map(
            params![period.start.to_string(), period.end_exclusive.to_string(), pattern, q.account_id, q.category_id, q.limit.unwrap_or(2000)],
            Self::tx_row,
        )?;
        let transactions: Vec<Tx> = rows.collect::<std::result::Result<_, _>>()?;
        let income = transactions.iter().filter(|t| t.r#type == TxType::Income).map(|t| t.amount).sum();
        let spent = transactions.iter().filter(|t| t.r#type == TxType::Expense).map(|t| t.amount).sum();
        Ok(TxPage { period: PeriodView::of(&period, today), transactions, income, spent })
    }

    /// A chart's series (`crate::series`): spending, income or net over `q`'s budget periods.
    pub fn series(&self, q: &crate::series::SeriesQuery, today: Date) -> Result<crate::series::Series> {
        use crate::series::{bucket, period_count, Named, Row, Series};
        let last = self.period(today)?.shift(q.period_offset.unwrap_or(0).clamp(-MAX_PERIOD_OFFSET, 0));
        let count = period_count(q) as i64;
        let periods: Vec<BudgetPeriod> = (0..count).map(|i| last.shift(i - (count - 1))).collect();
        let categories: Vec<Named> = self.categories()?.into_iter().map(|c| Named { id: c.id, name: c.name, color: c.color }).collect();
        let category = match q.category.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            None => None,
            Some(name) => {
                let wanted = name.to_lowercase();
                let found = categories.iter().find(|c| c.name.to_lowercase() == wanted)
                    .or_else(|| categories.iter().find(|c| c.name.to_lowercase().contains(&wanted)));
                match found {
                    Some(c) => Some(c.id),
                    None => return invalid(format!("No category named {name}")),
                }
            }
        };
        let mut st = self.conn.prepare_cached(
            "SELECT type, amount, date, category_id FROM transactions
             WHERE deleted = 0 AND type IN ('INCOME', 'EXPENSE') AND date >= ?1 AND date < ?2
               AND (?3 IS NULL OR category_id = ?3)",
        )?;
        let start = periods.first().expect("at least one period").start;
        let rows = st.query_map(params![start.to_string(), last.end_exclusive.to_string(), category], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<i64>>(3)?))
        })?;
        let mut read = Vec::new();
        for row in rows {
            let (ty, amount, date, category_id) = row?;
            let (Some(r#type), Some(date)) = (parse_name::<TxType>(&ty), parse_date(&date)) else { continue };
            read.push(Row { r#type, amount, date, category_id });
        }
        let (measure, by) = (q.measure.unwrap_or_default(), q.by.unwrap_or_default());
        let (labels, label_colors, series) = bucket(&read, &periods, by, measure, q.cumulative.unwrap_or(false), &categories, today);
        let currency = self.settings()?.currency;
        Ok(Series { fraction_digits: fraction_digits(&currency).unwrap_or(2), currency, measure, by, labels, label_colors, series })
    }

    fn check_entry(&self, ty: TxType, amount: i64, date: &str, account: i64, to: Option<i64>, category: Option<i64>) -> Result<()> {
        if amount <= 0 {
            return invalid("An entry needs an amount above zero");
        }
        // No keypad reaches it (`AmountInput::MAX_INTEGER_DIGITS`, with three decimals at most);
        // past it, the ledger's sums would overflow and every reading fail until it was deleted.
        if amount > MAX_AMOUNT {
            return invalid("That amount is larger than any entry the ledger keeps");
        }
        if parse_date(date).is_none() {
            return invalid(format!("Not a date: {date}"));
        }
        let account_exists = |id: i64| -> Result<bool> {
            Ok(self.conn.prepare_cached("SELECT 1 FROM accounts WHERE id = ?1 AND deleted = 0")?.exists([id])?)
        };
        if !account_exists(account)? {
            return invalid("Pick an account");
        }
        if ty == TxType::Transfer {
            match to {
                Some(to) if to == account => return invalid("A transfer moves money between two different accounts"),
                Some(to) if account_exists(to)? => {}
                _ => return invalid("Pick the account the money goes to"),
            }
        }
        if let Some(c) = category.filter(|_| ty != TxType::Transfer) {
            let kind: Option<String> =
                self.conn.prepare_cached("SELECT kind FROM categories WHERE id = ?1 AND deleted = 0")?.query_row([c], |r| r.get(0)).optional()?;
            let want = if ty == TxType::Income { "INCOME" } else { "EXPENSE" };
            if kind.as_deref() != Some(want) {
                return invalid("That category is not for this kind of entry");
            }
        }
        Ok(())
    }

    pub fn tx_add(&mut self, input: &TxInput) -> Result<Tx> {
        let to = input.to_account_id.filter(|_| input.r#type == TxType::Transfer);
        let category = input.category_id.filter(|_| input.r#type != TxType::Transfer);
        self.check_entry(input.r#type, input.amount, &input.date, input.account_id, to, category)?;
        let now = now_ms();
        self.conn.prepare_cached(
            "INSERT INTO transactions (uid, type, amount, date, account_id, to_account_id, category_id, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        )?.execute(params![
            new_uid(), name_of(&input.r#type), input.amount, input.date, input.account_id, to, category,
            input.note.as_deref().unwrap_or("").trim(), now
        ])?;
        self.tx(self.conn.last_insert_rowid())
    }

    pub fn tx_update(&mut self, id: i64, p: &TxPatch) -> Result<Tx> {
        let cur = self.tx(id)?;
        let ty = p.r#type.unwrap_or(cur.r#type);
        let amount = p.amount.unwrap_or(cur.amount);
        let date = p.date.clone().unwrap_or(cur.date);
        let account = p.account_id.unwrap_or(cur.account_id);
        let to = if ty == TxType::Transfer { p.to_account_id.or(cur.to_account_id) } else { None };
        let category = if ty == TxType::Transfer {
            None
        } else if p.category_id.is_some() || p.r#type.is_none_or(|t| t == cur.r#type) {
            p.category_id.or(cur.category_id)
        } else {
            // The type changed and no category came with it: the old one is of the other kind.
            None
        };
        self.check_entry(ty, amount, &date, account, to, category)?;
        let note = p.note.as_deref().map(str::trim).unwrap_or(&cur.note).to_string();
        self.conn.prepare_cached(
            "UPDATE transactions SET type = ?2, amount = ?3, date = ?4, account_id = ?5, to_account_id = ?6,
                    category_id = ?7, note = ?8, updated_at = MAX(?9, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1",
        )?.execute(params![id, name_of(&ty), amount, date, account, to, category, note, now_ms()])?;
        self.tx(id)
    }

    /// Deletes are tombstones, so sync can carry them to the other device.
    pub fn tx_delete(&mut self, id: i64) -> Result<()> {
        let n = self.conn.prepare_cached("UPDATE transactions SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 0")?
            .execute(params![id, now_ms()])?;
        if n == 0 {
            return Err(LedgerError::NotFound(format!("No entry {id}")));
        }
        Ok(())
    }

    /// Brings back a deleted entry: the Undo of [`Ledger::tx_delete`].
    pub fn tx_restore(&mut self, id: i64) -> Result<Tx> {
        let n = self.conn.prepare_cached("UPDATE transactions SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1")?
            .execute(params![id, now_ms()])?;
        if n == 0 {
            return Err(LedgerError::NotFound(format!("No deleted entry {id}")));
        }
        self.tx(id)
    }

    /// The day the budget month starts on, 1 to 28, for people paid on a fixed date.
    pub fn set_month_start_day(&mut self, day: i64) -> Result<()> {
        if !(1..=28).contains(&day) {
            return invalid("The month can start on day 1 to 28");
        }
        Ledger::set_setting(&self.conn, "month_start_day", &day.to_string())
    }

    /// The standing monthly limit for a category (`None`: the overall budget). Zero or less removes it.
    pub fn budget_set(&mut self, category_id: Option<i64>, amount: i64) -> Result<()> {
        let key = category_id.unwrap_or(0);
        if let Some(c) = category_id {
            let ok = self.conn.prepare_cached("SELECT 1 FROM categories WHERE id = ?1 AND deleted = 0 AND kind = 'EXPENSE'")?.exists([c])?;
            if !ok {
                return invalid("Budgets are set on spending categories");
            }
        }
        let now = now_ms();
        if amount <= 0 {
            self.conn.prepare_cached("UPDATE budgets SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE category_id = ?1")?.execute(params![key, now])?;
        } else {
            self.conn.prepare_cached(
                "INSERT INTO budgets (uid, category_id, amount, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(category_id) DO UPDATE SET amount = excluded.amount, deleted = 0, updated_at = MAX(excluded.updated_at, MIN(budgets.updated_at, 4000000000000000) + 1)",
            )?.execute(params![new_uid(), key, amount, now])?;
        }
        Ok(())
    }

    /// The monthly limit set for a category (`None`: the overall budget), if one is.
    pub fn budget(&self, category_id: Option<i64>) -> Result<Option<i64>> {
        Ok(self.conn.prepare_cached("SELECT amount FROM budgets WHERE category_id = ?1 AND deleted = 0")?
            .query_row([category_id.unwrap_or(0)], |r| r.get(0))
            .optional()?)
    }

    /// Runs `f` as one change to the ledger: all it writes, or none of it when it fails. For the
    /// PC's batches (a thread's proposal approved at once); `f` must not open a transaction itself.
    pub fn atomically<T>(&mut self, f: impl FnOnce(&mut Ledger) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("SAVEPOINT atomically")?;
        match f(self) {
            Ok(out) => {
                self.conn.execute_batch("RELEASE atomically")?;
                Ok(out)
            }
            Err(e) => {
                self.conn.execute_batch("ROLLBACK TO atomically; RELEASE atomically")?;
                Err(e)
            }
        }
    }

    /// Runs `f` and puts back everything it wrote: what a change would do, by the ledger's own
    /// rules, without doing it.
    pub fn trial<T>(&mut self, f: impl FnOnce(&mut Ledger) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("SAVEPOINT trial")?;
        let out = f(self);
        self.conn.execute_batch("ROLLBACK TO trial; RELEASE trial")?;
        out
    }

    fn budgets(&self) -> Result<Vec<(i64, i64)>> {
        let mut st = self.conn.prepare_cached("SELECT category_id, amount FROM budgets WHERE deleted = 0")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Posts every bill that posts itself and is due by `today`, one entry per date. Each posted
    /// entry's uid is derived from the bill and the date, so the phone and the PC posting the same
    /// rent produce one row once they sync, and posting twice here adds nothing.
    ///
    /// Once a phone syncs with this ledger, the phone posts bills and this ledger leaves them be:
    /// two devices posting the same bill at different times would each stamp it with their own
    /// time, and the later post would undo an edit or a delete made on the other side between.
    ///
    /// It also moves reminders on, as Tally's poster does: a reminder never posts, and nothing can
    /// tell whether it was paid, so once its day is over it moves to its next date. Left in place
    /// it would read as overdue for good. Reminders moved on are not counted: they post nothing.
    pub fn post_due(&mut self, today: Date) -> Result<usize> {
        if !self.synced_devices()?.is_empty() {
            return Ok(0);
        }
        let reminders: Vec<ReminderRow> = {
            let mut st = self.conn.prepare_cached(
                "SELECT id, frequency, interval, anchor_date, end_date
                 FROM recurring WHERE deleted = 0 AND active = 1 AND auto_post = 0 AND next_date < ?1",
            )?;
            let rows = st.query_map([today.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let due: Vec<DueRow> = {
            let mut st = self.conn.prepare_cached(
                "SELECT id, uid, type, amount, account_id, to_account_id, category_id, frequency, interval, anchor_date, next_date, end_date
                 FROM recurring WHERE deleted = 0 AND active = 1 AND auto_post = 1 AND next_date <= ?1",
            )?;
            let rows = st.query_map([today.to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?))
            })?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut posted = 0;
        let now = now_ms();
        let tx = self.conn.transaction()?;
        for (id, freq, interval, anchor, end) in reminders {
            let (Some(anchor), Some(freq)) = (parse_date(&anchor), parse_name::<Frequency>(&freq)) else {
                continue;
            };
            let following = Recurrence::new(anchor, freq, interval.clamp(1, 52) as i32).on_or_after(today);
            let active = end.as_deref().and_then(parse_date).is_none_or(|e| following <= e);
            tx.prepare_cached("UPDATE recurring SET next_date = ?2, active = ?3, updated_at = MAX(?4, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1")?
                .execute(params![id, following.to_string(), active, now])?;
        }
        for (id, uid, ty, amount, account, to, category, freq, interval, anchor, next, end) in due {
            let (Some(anchor), Some(next), Some(freq)) = (parse_date(&anchor), parse_date(&next), parse_name::<Frequency>(&freq)) else {
                continue;
            };
            let rule = Recurrence::new(anchor, freq, interval.clamp(1, 52) as i32);
            let end = end.as_deref().and_then(parse_date);
            let through = end.map_or(today, |e| e.min(today));
            // A category on a transfer bill, or a receiving account on any other, is a data slip
            // the entry must not carry (Tally's poster drops them the same way).
            let transfer = parse_name::<TxType>(&ty) == Some(TxType::Transfer);
            let (to, category) = (to.filter(|_| transfer), category.filter(|_| !transfer));
            let dates = rule.between_limited(next, through, CATCH_UP_LIMIT);
            for date in &dates {
                posted += tx.prepare_cached(
                    "INSERT OR IGNORE INTO transactions (uid, type, amount, date, account_id, to_account_id, category_id, note, recurring_id, created_at, updated_at)
                     SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, name, id, ?8, ?8 FROM recurring WHERE id = ?9
                       AND NOT EXISTS (SELECT 1 FROM tombstones WHERE tbl = 'transactions' AND uid = ?1)",
                )?.execute(params![format!("bill:{uid}:{date}"), ty, amount, date.to_string(), account, to, category, now, id])?;
            }
            // After the last date posted, not after today: a capped catch-up resumes there next pass.
            let following = rule.after(dates.last().copied().unwrap_or(today));
            let active = end.is_none_or(|e| following <= e);
            tx.prepare_cached("UPDATE recurring SET next_date = ?2, active = ?3, updated_at = MAX(?4, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1")?
                .execute(params![id, following.to_string(), active, now])?;
        }
        tx.commit()?;
        Ok(posted)
    }

    /// The Home reading for `today`.
    pub fn summary(&self, today: Date, locale: &Locale) -> Result<Summary> {
        let settings = self.settings()?;
        let fmt = self.formatter(locale)?;
        let period = self.period(today)?;
        let (income, spent) = self.flows(period.start, period.end_exclusive)?;
        let budgets = self.budgets()?;
        let overall = budgets.iter().find(|(c, _)| *c == 0).map_or(0, |(_, a)| *a);
        let elapsed = period.elapsed_days(today);
        let pace = PaceReading::new(overall, spent, period.days(), elapsed);
        // The same stretch of the period before: its first `elapsed` days.
        let last = period.shift(-1);
        let last_end = last.start.checked_add(elapsed.max(0).days()).expect("date in range").min(last.end_exclusive);
        let (_, last_spent) = self.flows(last.start, last_end)?;
        let lines = Lines {
            margin: copy::margin_line(&pace, &fmt),
            pace: copy::pace_line(&pace, &fmt),
            versus_last: copy::versus_last_line(spent, last_spent, &fmt),
        };
        let mut envelopes = Vec::new();
        {
            let mut st = self.conn.prepare_cached(
                "SELECT c.id, c.name, c.icon, c.color,
                        COALESCE((SELECT SUM(t.amount) FROM transactions t WHERE t.deleted = 0 AND t.type = 'EXPENSE'
                                  AND t.category_id = c.id AND t.date >= ?2 AND t.date < ?3), 0)
                 FROM categories c WHERE c.id = ?1 AND c.deleted = 0",
            )?;
            for (category, amount) in budgets.iter().filter(|(c, _)| *c != 0) {
                let row = st.query_row(params![category, period.start.to_string(), period.end_exclusive.to_string()], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))
                }).optional()?;
                if let Some((id, name, icon, color, used)) = row {
                    let r = PaceReading::new(*amount, used, period.days(), elapsed);
                    // In words, to the cent: a model read the sign of `pace_delta` wrong, and quotes
                    // what it is given, so Home's whole dollars would come back as "$8 over".
                    let left = fmt.format(r.remaining);
                    let line = match r.status {
                        PaceStatus::OverBudget => format!("{} over budget", fmt.format(-r.remaining)),
                        PaceStatus::OverPace => format!("{} over pace, {left} left", fmt.format(r.pace_delta)),
                        PaceStatus::UnderPace => format!("{} under pace, {left} left", fmt.format(-r.pace_delta)),
                        PaceStatus::OnPace | PaceStatus::NoBudget => format!("On pace, {left} left"),
                    };
                    envelopes.push(EnvelopeView { category_id: id, name, icon, color, budget: *amount, spent: used, left: *amount - used, pace_delta: r.pace_delta, status: r.status, line });
                }
            }
        }
        // Furthest over pace first, as Home lists them.
        envelopes.sort_by(|a, b| b.pace_delta.cmp(&a.pace_delta).then_with(|| a.name.cmp(&b.name)));
        let accounts = self.accounts()?;
        let net_worth = accounts.iter().filter(|a| !a.archived).map(|a| a.balance).sum();
        let bills = self.bills(today, 30)?;
        let goals = self.goals(today, &period, income, spent, &accounts, net_worth, &fmt)?;
        Ok(Summary {
            fraction_digits: fmt.fraction_digits,
            currency: settings.currency,
            month_start_day: settings.month_start_day,
            empty: self.is_empty()?,
            period: PeriodView::of(&period, today),
            pace,
            income,
            spent,
            lines,
            budgets: envelopes,
            accounts,
            net_worth,
            bills,
            goals,
            recent: self.recent(8)?,
        })
    }

    /// Active bills due within `days` of `today`, overdue ones included, soonest first.
    fn bills(&self, today: Date, days: i64) -> Result<Vec<BillView>> {
        let horizon = today.checked_add(days.days()).expect("date in range");
        let mut st = self.conn.prepare_cached(
            "SELECT id, name, amount, type, next_date, auto_post FROM recurring
             WHERE deleted = 0 AND active = 1 AND next_date <= ?1 ORDER BY next_date, name",
        )?;
        let rows = st.query_map([horizon.to_string()], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, bool>(5)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, amount, ty, next, auto_post) = row?;
            let Some(next_date) = parse_date(&next) else { continue };
            let days_until = days_between(today, next_date);
            out.push(BillView {
                id,
                name,
                amount,
                r#type: parse_name(&ty).unwrap_or(TxType::Expense),
                next_date: next,
                days_until,
                due_line: copy::due_line(days_until),
                auto_post,
            });
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn goals(
        &self, today: Date, period: &BudgetPeriod, income: i64, spent: i64, accounts: &[AccountView], net_worth: i64,
        fmt: &MoneyFormatter,
    ) -> Result<Vec<GoalView>> {
        let mut st = self.conn.prepare_cached(
            "SELECT g.id, g.name, g.kind, g.target, g.target_date, g.account_id, g.percent, g.start_amount,
                    COALESCE((SELECT SUM(amount) FROM contributions c WHERE c.deleted = 0 AND c.goal_id = g.id), 0)
             FROM goals g WHERE g.deleted = 0 AND g.archived = 0
             ORDER BY g.target_date IS NULL, g.target_date, g.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?, r.get::<_, Option<i64>>(5)?, r.get::<_, i64>(6)?, r.get::<_, i64>(7)?, r.get::<_, i64>(8)?,
            ))
        })?;
        // This period's transfers, for INVEST goals: each reads what moved into its own accounts.
        let transfers: Vec<(i64, i64, i64)> = self.conn.prepare_cached(
            "SELECT t.account_id, t.to_account_id, t.amount FROM transactions t
             WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND t.to_account_id IS NOT NULL AND t.date >= ?1 AND t.date < ?2",
        )?.query_map([period.start.to_string(), period.end_exclusive.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let investments: Vec<i64> = accounts.iter().filter(|a| a.r#type == AccountType::Investment).map(|a| a.id).collect();
        let mut out = Vec::new();
        for row in rows {
            let (id, name, kind, target, target_date, account, percent, start_amount, contributed) = row?;
            let kind = parse_name(&kind).unwrap_or(GoalKind::Savings);
            let months_left = || target_date.as_deref().and_then(parse_date).map(|d| months_until(today, d));
            let (saved, target, line) = match kind {
                GoalKind::Savings => {
                    let saved = start_amount + contributed;
                    (saved, target, copy::goal_line(saved, target, months_left(), fmt))
                }
                GoalKind::Balance => {
                    let saved = account.and_then(|a| accounts.iter().find(|x| x.id == a)).map_or(net_worth, |a| a.balance);
                    (saved, target, copy::balance_goal_line(saved, target, start_amount, months_left(), fmt))
                }
                // Monthly goals read this period: a share of what came in, or a set amount.
                GoalKind::Invest | GoalKind::Save => {
                    let saved = if kind == GoalKind::Invest {
                        // Its own account, or every investment account when it names none.
                        let into = account.map_or_else(|| investments.clone(), |a| vec![a]);
                        moved_into(&into, &transfers)
                    } else {
                        (income - spent).max(0)
                    };
                    let target = if percent > 0 { income * percent / 100 } else { target };
                    (saved, target, copy::goal_line(saved, target, None, fmt))
                }
            };
            out.push(GoalView { id, name, kind, target, saved, line });
        }
        Ok(out)
    }

    /// Erases every row. Each live one leaves a tombstone, so the next sync erases it on the
    /// other device too, unless `quietly` (the phone replacing this ledger has none of them).
    ///
    /// Either way the ledger takes a new generation: a device that synced with the old one is
    /// told so on its next sync (`LedgerError::Stale`) and takes this ledger whole, instead of
    /// merging two ledgers that no longer share their rows.
    pub(crate) fn clear(tx: &rusqlite::Transaction, quietly: bool) -> Result<()> {
        let now = now_ms();
        // Every tombstone first: a budget's names its category, which must still be there.
        for t in TABLES.iter().filter(|_| !quietly) {
            let category = if *t == "budgets" { "(SELECT c.uid FROM categories c WHERE c.id = budgets.category_id)" } else { "NULL" };
            tx.execute(
                &format!(
                    "INSERT OR REPLACE INTO tombstones (tbl, uid, updated_at, category)
                     SELECT '{t}', uid, MAX(?1, MIN(updated_at, 4000000000000000) + 1), {category} FROM {t} WHERE deleted = 0"
                ),
                [now],
            )?;
        }
        for t in TABLES {
            tx.execute(&format!("DELETE FROM {t}"), [])?;
        }
        // Which phones sync stays; the ledger's own settings go.
        tx.execute("DELETE FROM settings WHERE key NOT LIKE 'sync.device.%'", [])?;
        if quietly {
            tx.execute("DELETE FROM tombstones", [])?;
        }
        Ledger::set_setting(tx, "sync.generation", &new_uid())?;
        Ok(())
    }

    /// The ledger's generation: changes with every restore, erase or replace.
    pub fn generation(&self) -> Result<String> {
        if let Some(g) = self.setting("sync.generation")? {
            return Ok(g);
        }
        let g = new_uid();
        Ledger::set_setting(&self.conn, "sync.generation", &g)?;
        Ok(g)
    }

    /// Replaces the whole ledger with a Tally backup, as a restore does on the phone. Ids are kept
    /// so references hold. Accounts and investment rows keep the uid the file gives them (an
    /// import's `ws:`, `sec:`, `imp:`… uids, so the same Wealthsimple file imported again adds
    /// nothing); every other row, and one whose uid is blank or already taken in the file, gets a
    /// fresh one.
    pub fn import_backup(&mut self, file: &BackupFile) -> Result<usize> {
        if let Some(problem) = crate::backup::problem(file) {
            return invalid(problem);
        }
        let now = now_ms();
        let mut taken: HashSet<(&str, String)> = HashSet::new();
        let mut uid = |table: &'static str, kept: &Option<String>| -> String {
            match kept.as_deref().filter(|u| !kt_is_blank(u)) {
                Some(u) if taken.insert((table, u.to_string())) => u.to_string(),
                _ => new_uid(),
            }
        };
        // A file silent on these leaves this ledger's own (backup.rs): `clear` drops every
        // setting, so they are read first and written back. The sample, and an older backup
        // without them, no longer put the budget month back on the 1st.
        let month_start_day = self.setting("month_start_day")?;
        let week_starts_monday = self.setting("week_starts_monday")?;
        let tx = self.conn.transaction()?;
        Ledger::clear(&tx, false)?;
        Ledger::set_setting(&tx, "currency", &file.currency)?;
        match (file.month_start_day, &month_start_day) {
            (Some(d), _) => Ledger::set_setting(&tx, "month_start_day", &d.to_string())?,
            (None, Some(kept)) => Ledger::set_setting(&tx, "month_start_day", kept)?,
            (None, None) => {}
        }
        match (file.week_starts_monday, &week_starts_monday) {
            (Some(m), _) => Ledger::set_setting(&tx, "week_starts_monday", if m { "1" } else { "0" })?,
            (None, Some(kept)) => Ledger::set_setting(&tx, "week_starts_monday", kept)?,
            (None, None) => {}
        }
        for a in &file.accounts {
            tx.prepare_cached(
                "INSERT INTO accounts (id, uid, name, type, opening_balance, archived, sort_order, registration, institution, external_ref, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            )?.execute(params![
                a.id, uid("accounts", &a.uid), a.name, name_of(&a.r#type), a.opening_balance, a.archived, a.sort_order,
                a.registration.as_ref().map(name_of), a.institution, a.external_ref, now
            ])?;
        }
        for c in &file.categories {
            tx.prepare_cached("INSERT INTO categories (id, uid, name, kind, color, icon, archived, sort_order, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)")?
                .execute(params![c.id, new_uid(), c.name, name_of(&c.kind), c.color, c.icon, c.archived, c.sort_order, now])?;
        }
        for t in &file.transactions {
            tx.prepare_cached(
                "INSERT INTO transactions (id, uid, type, amount, date, account_id, to_account_id, category_id, note, recurring_id, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
            )?.execute(params![t.id, new_uid(), name_of(&t.r#type), t.amount, t.date, t.account_id, t.to_account_id, t.category_id, t.note, t.recurring_id, now])?;
        }
        for b in &file.budgets {
            tx.prepare_cached("INSERT INTO budgets (id, uid, category_id, amount, updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(category_id) DO UPDATE SET amount = excluded.amount")?
                .execute(params![b.id, new_uid(), b.category_id.unwrap_or(0), b.amount, now])?;
        }
        for r in &file.recurring {
            tx.prepare_cached(
                "INSERT INTO recurring (id, uid, name, type, amount, account_id, to_account_id, category_id, frequency, interval, anchor_date, next_date, end_date, auto_post, active, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            )?.execute(params![
                r.id, new_uid(), r.name, name_of(&r.r#type), r.amount, r.account_id, r.to_account_id, r.category_id, name_of(&r.frequency),
                r.interval, r.anchor_date, r.next_date, r.end_date, r.auto_post, r.active, now
            ])?;
        }
        for g in &file.goals {
            tx.prepare_cached(
                "INSERT INTO goals (id, uid, name, target, target_date, color, archived, kind, account_id, percent, start_date, start_amount, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            )?.execute(params![
                g.id, new_uid(), g.name, g.target, g.target_date, g.color, g.archived, name_of(&g.kind), g.account_id, g.percent, g.start_date,
                g.start_amount, now
            ])?;
        }
        for c in &file.contributions {
            tx.prepare_cached("INSERT INTO contributions (id, uid, goal_id, amount, date, note, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)")?
                .execute(params![c.id, new_uid(), c.goal_id, c.amount, c.date, c.note, now])?;
        }
        for v in &file.values {
            tx.prepare_cached("INSERT INTO account_values (id, uid, account_id, date, value, updated_at) VALUES (?1,?2,?3,?4,?5,?6)")?
                .execute(params![v.id, new_uid(), v.account_id, v.date, v.value, now])?;
        }
        for s in &file.securities {
            tx.prepare_cached("INSERT INTO securities (id, uid, symbol, name, currency, kind, exchange, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")?
                .execute(params![s.id, uid("securities", &s.uid), s.symbol, s.name, s.currency, name_of(&s.kind), s.exchange, now])?;
        }
        for h in &file.holdings {
            tx.prepare_cached(
                "INSERT INTO holdings (id, uid, account_id, security_id, date, quantity, book, book_market, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            )?.execute(params![h.id, uid("holdings", &h.uid), h.account_id, h.security_id, h.date, h.quantity, h.book, h.book_market, now])?;
        }
        for a in &file.activities {
            tx.prepare_cached(
                "INSERT INTO activities (id, uid, account_id, security_id, type, date, quantity, amount, fee, currency, to_amount, to_currency, note, source, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
            )?.execute(params![
                a.id, uid("activities", &a.uid), a.account_id, a.security_id, name_of(&a.r#type), a.date, a.quantity, a.amount, a.fee, a.currency,
                a.to_amount, a.to_currency, a.note, a.source, now
            ])?;
        }
        for p in &file.prices {
            tx.prepare_cached("INSERT INTO prices (id, uid, security_id, date, price, source, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)")?
                .execute(params![p.id, uid("prices", &p.uid), p.security_id, p.date, p.price, p.source, now])?;
        }
        for r in &file.fx_rates {
            tx.prepare_cached("INSERT INTO fx_rates (id, uid, base, quote, date, rate, source, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")?
                .execute(params![r.id, uid("fx_rates", &r.uid), r.base, r.quote, r.date, r.rate, r.source, now])?;
        }
        for r in &file.room_facts {
            tx.prepare_cached("INSERT INTO room_facts (id, uid, registration, year, amount, updated_at) VALUES (?1,?2,?3,?4,?5,?6)")?
                .execute(params![r.id, uid("room_facts", &r.uid), name_of(&r.registration), r.year, r.amount, now])?;
        }
        // A kept uid is live again: the tombstone the erase above left for it must not travel and
        // erase the restored row on the phone.
        for t in TABLES {
            tx.execute(&format!("DELETE FROM tombstones WHERE tbl = '{t}' AND uid IN (SELECT uid FROM {t})"), [])?;
        }
        tx.commit()?;
        Ok(file.transactions.len())
    }

    /// The ledger as a Tally backup the phone can restore.
    pub fn export_backup(&self, exported_at: &str) -> Result<BackupFile> {
        let s = self.settings()?;
        let mut file = BackupFile::new(exported_at, s.currency.clone());
        file.month_start_day = Some(s.month_start_day as i32);
        file.week_starts_monday = Some(s.week_starts_monday);
        let c = &self.conn;
        fn all<T>(c: &Connection, sql: &str, f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>) -> Result<Vec<T>> {
            let mut st = c.prepare(sql)?;
            let rows = st.query_map([], f)?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        }
        file.accounts = all(c, "SELECT id, name, type, opening_balance, archived, sort_order, registration, institution, external_ref, uid FROM accounts WHERE deleted = 0 ORDER BY id", |r| {
            Ok(AccountDto {
                id: r.get(0)?, name: r.get(1)?, r#type: parse_name(&r.get::<_, String>(2)?).unwrap_or(AccountType::Chequing), opening_balance: r.get(3)?,
                archived: r.get(4)?, sort_order: r.get(5)?, registration: r.get::<_, Option<String>>(6)?.as_deref().and_then(parse_name),
                institution: r.get(7)?, external_ref: r.get(8)?, uid: r.get(9)?,
            })
        })?;
        file.categories = all(c, "SELECT id, name, kind, color, icon, archived, sort_order FROM categories WHERE deleted = 0 ORDER BY id", |r| {
            Ok(CategoryDto { id: r.get(0)?, name: r.get(1)?, kind: parse_name(&r.get::<_, String>(2)?).unwrap_or(CategoryKind::Expense), color: r.get(3)?, icon: r.get(4)?, archived: r.get(5)?, sort_order: r.get(6)? })
        })?;
        file.transactions = all(c, "SELECT id, type, amount, date, account_id, to_account_id, category_id, note, recurring_id FROM transactions WHERE deleted = 0 ORDER BY id", |r| {
            Ok(TransactionDto { id: r.get(0)?, r#type: parse_name(&r.get::<_, String>(1)?).unwrap_or(TxType::Expense), amount: r.get(2)?, date: r.get(3)?, account_id: r.get(4)?, to_account_id: r.get(5)?, category_id: r.get(6)?, note: r.get(7)?, recurring_id: r.get(8)? })
        })?;
        file.budgets = all(c, "SELECT id, category_id, amount FROM budgets WHERE deleted = 0 ORDER BY id", |r| {
            let category: i64 = r.get(1)?;
            Ok(BudgetDto { id: r.get(0)?, category_id: (category != 0).then_some(category), amount: r.get(2)? })
        })?;
        file.recurring = all(c, "SELECT id, name, type, amount, account_id, to_account_id, category_id, frequency, interval, anchor_date, next_date, end_date, auto_post, active FROM recurring WHERE deleted = 0 ORDER BY id", |r| {
            Ok(RecurringDto {
                id: r.get(0)?, name: r.get(1)?, r#type: parse_name(&r.get::<_, String>(2)?).unwrap_or(TxType::Expense), amount: r.get(3)?,
                account_id: r.get(4)?, to_account_id: r.get(5)?, category_id: r.get(6)?,
                frequency: parse_name(&r.get::<_, String>(7)?).unwrap_or(Frequency::Monthly), interval: r.get(8)?,
                anchor_date: r.get(9)?, next_date: r.get(10)?, end_date: r.get(11)?, auto_post: r.get(12)?, active: r.get(13)?,
            })
        })?;
        file.goals = all(c, "SELECT id, name, target, target_date, color, archived, kind, account_id, percent, start_date, start_amount FROM goals WHERE deleted = 0 ORDER BY id", |r| {
            let mut g = GoalDto::new(r.get(0)?, r.get::<_, String>(1)?, r.get(2)?);
            g.target_date = r.get(3)?;
            g.color = r.get(4)?;
            g.archived = r.get(5)?;
            g.kind = parse_name(&r.get::<_, String>(6)?).unwrap_or(GoalKind::Savings);
            g.account_id = r.get(7)?;
            g.percent = r.get(8)?;
            g.start_date = r.get(9)?;
            g.start_amount = r.get(10)?;
            Ok(g)
        })?;
        file.contributions = all(c, "SELECT id, goal_id, amount, date, note FROM contributions WHERE deleted = 0 ORDER BY id", |r| {
            Ok(ContributionDto { id: r.get(0)?, goal_id: r.get(1)?, amount: r.get(2)?, date: r.get(3)?, note: r.get(4)? })
        })?;
        file.values = all(c, "SELECT id, account_id, date, value FROM account_values WHERE deleted = 0 ORDER BY id", |r| {
            Ok(AccountValueDto { id: r.get(0)?, account_id: r.get(1)?, date: r.get(2)?, value: r.get(3)? })
        })?;
        file.securities = all(c, "SELECT id, symbol, name, currency, kind, exchange, uid FROM securities WHERE deleted = 0 ORDER BY id", |r| {
            Ok(SecurityDto {
                id: r.get(0)?, symbol: r.get(1)?, name: r.get(2)?, currency: r.get(3)?,
                kind: parse_name(&r.get::<_, String>(4)?).unwrap_or(SecurityKind::Other), exchange: r.get(5)?, uid: r.get(6)?,
            })
        })?;
        file.holdings = all(c, "SELECT id, account_id, security_id, date, quantity, book, book_market, uid FROM holdings WHERE deleted = 0 ORDER BY id", |r| {
            Ok(HoldingDto {
                id: r.get(0)?, account_id: r.get(1)?, security_id: r.get(2)?, date: r.get(3)?, quantity: r.get(4)?, book: r.get(5)?, book_market: r.get(6)?,
                uid: r.get(7)?,
            })
        })?;
        file.activities = all(c, "SELECT id, account_id, security_id, type, date, quantity, amount, fee, currency, to_amount, to_currency, note, source, uid FROM activities WHERE deleted = 0 ORDER BY id", |r| {
            Ok(ActivityDto {
                id: r.get(0)?, account_id: r.get(1)?, security_id: r.get(2)?, r#type: parse_name(&r.get::<_, String>(3)?).unwrap_or(ActivityType::Deposit),
                date: r.get(4)?, quantity: r.get(5)?, amount: r.get(6)?, fee: r.get(7)?, currency: r.get(8)?, to_amount: r.get(9)?,
                to_currency: r.get(10)?, note: r.get(11)?, source: r.get(12)?, uid: r.get(13)?,
            })
        })?;
        file.prices = all(c, "SELECT id, security_id, date, price, source, uid FROM prices WHERE deleted = 0 ORDER BY id", |r| {
            Ok(PriceDto { id: r.get(0)?, security_id: r.get(1)?, date: r.get(2)?, price: r.get(3)?, source: r.get(4)?, uid: r.get(5)? })
        })?;
        file.fx_rates = all(c, "SELECT id, base, quote, date, rate, source, uid FROM fx_rates WHERE deleted = 0 ORDER BY id", |r| {
            Ok(FxRateDto { id: r.get(0)?, base: r.get(1)?, quote: r.get(2)?, date: r.get(3)?, rate: r.get(4)?, source: r.get(5)?, uid: r.get(6)? })
        })?;
        file.room_facts = all(c, "SELECT id, registration, year, amount, uid FROM room_facts WHERE deleted = 0 ORDER BY id", |r| {
            Ok(RoomFactDto {
                id: r.get(0)?, registration: parse_name(&r.get::<_, String>(1)?).unwrap_or(Registration::Other), year: r.get(2)?, amount: r.get(3)?,
                uid: r.get(4)?,
            })
        })?;
        Ok(file)
    }

    /// Tally's sample household, into an empty ledger only: it must never mix with real entries.
    pub fn load_sample(&mut self, today: Date, currency: &str) -> Result<usize> {
        if !self.is_empty()? {
            return invalid("The sample loads only into an empty ledger. Erase everything first.");
        }
        let currency = if fraction_digits(currency).is_some() { currency } else { "CAD" };
        let file = crate::sample_data::build(today, fraction_digits(currency).unwrap_or(2), currency, crate::sample_data::DEFAULT_SEED);
        self.import_backup(&file)
    }

    /// Seeds Tally's default categories when there are none, so a first entry has somewhere to go.
    pub fn ensure_categories(&mut self) -> Result<()> {
        let n: i64 = self.conn.prepare_cached("SELECT COUNT(*) FROM categories WHERE deleted = 0")?.query_row([], |r| r.get(0))?;
        if n > 0 {
            return Ok(());
        }
        let now = now_ms();
        for (i, c) in DEFAULT_CATEGORIES.iter().enumerate() {
            self.conn.prepare_cached("INSERT INTO categories (uid, name, kind, color, icon, sort_order, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)")?
                .execute(params![new_uid(), c.name, name_of(&c.kind), c.color, c.icon, i as i64, now])?;
        }
        Ok(())
    }

    /// Adds an account; the first one a person makes on the PC. Only an investment account takes a
    /// registration.
    pub fn account_add(
        &mut self, name: &str, ty: AccountType, opening_balance: i64, registration: Option<Registration>, institution: &str,
    ) -> Result<AccountView> {
        let name = name.trim();
        if name.is_empty() {
            return invalid("An account needs a name");
        }
        if registration.is_some() && ty != AccountType::Investment {
            return invalid("Only an investment account has a registration");
        }
        let registration = registration.map(|r| name_of(&r));
        let order: i64 = self.conn.prepare_cached("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM accounts")?.query_row([], |r| r.get(0))?;
        self.conn.prepare_cached(
            "INSERT INTO accounts (uid, name, type, opening_balance, sort_order, registration, institution, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        )?.execute(params![new_uid(), name, name_of(&ty), opening_balance, order, registration, institution.trim(), now_ms()])?;
        let id = self.conn.last_insert_rowid();
        self.ensure_categories()?;
        self.accounts()?.into_iter().find(|a| a.id == id).ok_or_else(|| LedgerError::NotFound("account".into()))
    }

    pub fn set_currency(&mut self, currency: &str) -> Result<()> {
        let Some(digits) = fraction_digits(currency) else {
            return invalid(format!("Not a currency code: {currency}"));
        };
        // Tally converts every amount when the decimals change; this ledger does not, so it
        // takes only a change that keeps them.
        let current = fraction_digits(&self.settings()?.currency).unwrap_or(2);
        if digits != current && !self.is_empty()? {
            return invalid("That currency has a different number of decimals. Change it in Tally on your phone, which converts every amount.");
        }
        Ledger::set_setting(&self.conn, "currency", currency)
    }

    /// Erases everything, settings included.
    pub fn reset(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        Ledger::clear(&tx, false)?;
        tx.commit()?;
        Ok(())
    }
}

/// Whole months from `today` to `date`, rounded up: at least 1 until the date passes, then 0.
/// Port of `monthsUntil` (PlanLogic.kt): Java's `ChronoUnit.MONTHS.between`, plus one when `today`
/// that many months on still falls short of `date`.
fn months_until(today: Date, date: Date) -> i64 {
    if date < today {
        return 0;
    }
    let mut months = (i64::from(date.year()) - i64::from(today.year())) * 12 + i64::from(date.month()) - i64::from(today.month());
    if months > 0 && date.day() < today.day() {
        months -= 1;
    }
    // `checked_add` clamps the day to the month's last, as `plusMonths` does.
    if today.checked_add(months.months()).is_ok_and(|d| d < date) {
        months += 1;
    }
    months.max(1)
}

/// What the period's `transfers` (from, to, amount) moved into `into` from the other accounts, less
/// what they moved back out. A move between two of `into` (TFSA to RRSP) is neither. Port of
/// `movedInto` (PlanLogic.kt).
fn moved_into(into: &[i64], transfers: &[(i64, i64, i64)]) -> i64 {
    transfers.iter().map(|&(from, to, amount)| match (into.contains(&from), into.contains(&to)) {
        (false, true) => amount,
        (true, false) => -amount,
        _ => 0,
    }).sum()
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn en() -> Locale {
        Locale::new("en-CA")
    }

    fn with_account() -> (Ledger, i64) {
        let mut l = Ledger::open_in_memory().unwrap();
        let a = l.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap();
        (l, a.id)
    }

    fn category(l: &Ledger, name: &str) -> i64 {
        l.categories().unwrap().into_iter().find(|c| c.name == name).unwrap().id
    }

    fn spend(l: &mut Ledger, account: i64, category: i64, amount: i64, day: &str) -> Tx {
        l.tx_add(&TxInput { r#type: TxType::Expense, amount, date: day.into(), account_id: account, to_account_id: None, category_id: Some(category), note: None }).unwrap()
    }

    #[test]
    fn a_new_ledger_is_empty_and_honest() {
        let l = Ledger::open_in_memory().unwrap();
        let s = l.summary(date(2026, 10, 7), &en()).unwrap();
        assert!(s.empty);
        assert_eq!(s.pace.status, PaceStatus::NoBudget);
        assert_eq!(s.lines.margin, "No budget set for this month");
        assert_eq!(s.currency, "CAD");
    }

    #[test]
    fn spending_moves_the_balance_the_budget_and_the_pace() {
        let (mut l, acct) = with_account();
        let groceries = category(&l, "Groceries");
        l.budget_set(None, 310_00).unwrap();
        l.budget_set(Some(groceries), 100_00).unwrap();
        spend(&mut l, acct, groceries, 150_00, "2026-10-05");
        let s = l.summary(date(2026, 10, 10), &en()).unwrap();
        assert_eq!(s.spent, 150_00);
        assert_eq!(s.pace.status, PaceStatus::OverPace);
        assert_eq!(s.lines.pace, "$50 over pace");
        assert_eq!(s.accounts[0].balance, 850_00);
        assert_eq!(s.budgets[0].status, PaceStatus::OverBudget);
        assert_eq!(s.budgets[0].line, "$50.00 over budget");
        assert_eq!(s.recent.len(), 1);
        assert_eq!(s.recent[0].category.as_deref(), Some("Groceries"));
    }

    #[test]
    fn a_transfer_moves_money_without_counting_as_spending() {
        let (mut l, acct) = with_account();
        let savings = l.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
        l.tx_add(&TxInput { r#type: TxType::Transfer, amount: 200_00, date: "2026-10-02".into(), account_id: acct, to_account_id: Some(savings), category_id: None, note: None }).unwrap();
        let s = l.summary(date(2026, 10, 3), &en()).unwrap();
        assert_eq!(s.spent, 0);
        assert_eq!(s.net_worth, 1_000_00);
        assert_eq!(s.accounts.iter().find(|a| a.id == savings).unwrap().balance, 200_00);
    }

    fn goal(l: &Ledger, name: &str, kind: GoalKind, target: i64, target_date: Option<&str>, account: Option<i64>, start_amount: i64) {
        l.conn.execute(
            "INSERT INTO goals (uid, name, target, target_date, kind, account_id, start_amount, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,0)",
            params![new_uid(), name, target, target_date, name_of(&kind), account, start_amount],
        ).unwrap();
    }

    fn transfer(l: &mut Ledger, from: i64, to: i64, amount: i64, day: &str) {
        l.tx_add(&TxInput { r#type: TxType::Transfer, amount, date: day.into(), account_id: from, to_account_id: Some(to), category_id: None, note: None }).unwrap();
    }

    fn goal_line(s: &Summary, name: &str) -> String {
        s.goals.iter().find(|g| g.name == name).unwrap().line.clone()
    }

    #[test]
    fn months_until_rounds_whole_months_up_as_tally_does() {
        let today = date(2026, 10, 13);
        assert_eq!(2, months_until(today, date(2026, 12, 13)));
        assert_eq!(3, months_until(today, date(2026, 12, 14)));
        assert_eq!(2, months_until(today, date(2026, 12, 12)));
        assert_eq!(1, months_until(today, today), "at least one until the date passes");
        assert_eq!(1, months_until(today, date(2026, 10, 31)));
        assert_eq!(0, months_until(today, date(2026, 10, 12)));
        // A month on from the 31st is the last of a shorter month, as `plusMonths` has it.
        assert_eq!(1, months_until(date(2026, 1, 31), date(2026, 2, 28)));
        assert_eq!(2, months_until(date(2026, 1, 31), date(2026, 3, 1)));
    }

    #[test]
    fn a_savings_goal_divides_what_is_left_over_whole_months_rounded_up() {
        let (l, _) = with_account();
        goal(&l, "Trip", GoalKind::Savings, 1_200_00, Some("2026-12-13"), None, 0);
        let s = l.summary(date(2026, 10, 13), &en()).unwrap();
        assert_eq!("$600 a month for 2 months", goal_line(&s, "Trip"));
    }

    #[test]
    fn an_invest_goal_reads_what_moved_into_its_own_accounts_less_what_moved_out() {
        let (mut l, cheq) = with_account();
        let tfsa = l.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
        let rrsp = l.account_add("RRSP", AccountType::Investment, 0, Some(Registration::Rrsp), "").unwrap().id;
        transfer(&mut l, cheq, tfsa, 500_00, "2026-10-02");
        transfer(&mut l, tfsa, rrsp, 200_00, "2026-10-03");
        transfer(&mut l, rrsp, cheq, 50_00, "2026-10-04");
        goal(&l, "Into the TFSA", GoalKind::Invest, 300_00, None, Some(tfsa), 0);
        goal(&l, "Invest", GoalKind::Invest, 1_000_00, None, None, 0);
        let s = l.summary(date(2026, 10, 13), &en()).unwrap();
        let saved = |name: &str| s.goals.iter().find(|g| g.name == name).unwrap().saved;
        // The TFSA: 500 in, 200 out to the RRSP.
        assert_eq!(300_00, saved("Into the TFSA"));
        assert_eq!("Reached", goal_line(&s, "Into the TFSA"));
        // Every investment account: 500 in, the TFSA to RRSP move is neither, 50 back out.
        assert_eq!(450_00, saved("Invest"));
        assert_eq!("$550 to go", goal_line(&s, "Invest"));
    }

    #[test]
    fn a_balance_goal_heading_down_is_not_reached_from_day_one() {
        let (l, cheq) = with_account();
        // The chequing account reads 1,000 and is to come down to 200.
        goal(&l, "Spend down", GoalKind::Balance, 200_00, Some("2026-12-13"), Some(cheq), 1_000_00);
        let s = l.summary(date(2026, 10, 13), &en()).unwrap();
        assert_eq!("$400 a month for 2 months", goal_line(&s, "Spend down"));
    }

    #[test]
    fn the_lists_total_is_the_summarys_net_worth() {
        let (mut l, acct) = with_account();
        let card = l.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
        let old = l.account_add("Old", AccountType::Savings, 50_00, None, "").unwrap().id;
        l.account_update(old, &AccountPatch { archived: Some(true), ..Default::default() }).unwrap();
        l.tx_add(&TxInput { r#type: TxType::Expense, amount: 40_00, date: "2026-10-02".into(), account_id: card, to_account_id: None, category_id: None, note: None }).unwrap();
        let s = l.summary(date(2026, 10, 3), &en()).unwrap();
        let lists = l.lists().unwrap();
        assert_eq!(lists.net_worth, s.net_worth);
        assert_eq!(lists.net_worth, 1_000_00 - 40_00);
        assert!(lists.accounts.iter().any(|a| a.id == acct));
    }

    #[test]
    fn entries_are_refused_when_they_cannot_be_right() {
        let (mut l, acct) = with_account();
        let salary = category(&l, "Salary");
        let bad = |l: &mut Ledger, input: TxInput| matches!(l.tx_add(&input), Err(LedgerError::Invalid(_)));
        let base = TxInput { r#type: TxType::Expense, amount: 5_00, date: "2026-10-02".into(), account_id: acct, to_account_id: None, category_id: None, note: None };
        assert!(bad(&mut l, TxInput { amount: 0, ..base.clone() }));
        assert!(bad(&mut l, TxInput { date: "02/10/2026".into(), ..base.clone() }));
        assert!(bad(&mut l, TxInput { account_id: 99, ..base.clone() }));
        assert!(bad(&mut l, TxInput { category_id: Some(salary), ..base.clone() }), "an expense is not filed under an income category");
        assert!(bad(&mut l, TxInput { r#type: TxType::Transfer, to_account_id: Some(acct), ..base.clone() }));
        assert!(l.tx_add(&base).is_ok());
    }

    #[test]
    fn edits_and_deletes_change_the_reading() {
        let (mut l, acct) = with_account();
        let dining = category(&l, "Dining");
        let t = spend(&mut l, acct, dining, 20_00, "2026-10-02");
        let t = l.tx_update(t.id, &TxPatch { amount: Some(35_00), note: Some(" Lunch ".into()), ..Default::default() }).unwrap();
        assert_eq!((t.amount, t.note.as_str()), (35_00, "Lunch"));
        let page = l.tx_list(&TxQuery { query: Some("LUNCH".into()), ..Default::default() }, date(2026, 10, 3)).unwrap();
        assert_eq!(page.transactions.len(), 1);
        assert_eq!(page.spent, 35_00);
        l.tx_delete(t.id).unwrap();
        assert!(l.tx_list(&TxQuery::default(), date(2026, 10, 3)).unwrap().transactions.is_empty());
        assert!(matches!(l.tx_delete(t.id), Err(LedgerError::NotFound(_))));
        assert_eq!(l.tx_restore(t.id).unwrap().amount, 35_00, "Undo brings it back as it was");
        assert_eq!(l.tx_list(&TxQuery::default(), date(2026, 10, 3)).unwrap().spent, 35_00);
    }

    #[test]
    fn a_batch_applies_whole_or_not_at_all() {
        let (mut l, acct) = with_account();
        let dining = category(&l, "Dining");
        let (a, b) = (spend(&mut l, acct, dining, 20_00, "2026-10-02"), spend(&mut l, acct, dining, 5_00, "2026-10-03"));
        let failed = l.atomically(|l| {
            l.tx_delete(a.id)?;
            l.budget_set(Some(dining), 300_00)?;
            l.tx_delete(b.id + 100)
        });
        assert!(matches!(failed, Err(LedgerError::NotFound(_))));
        assert!(l.tx(a.id).is_ok(), "the first delete went back with the failure");
        assert_eq!(l.budget(Some(dining)).unwrap(), None);
        l.atomically(|l| {
            l.tx_delete(a.id)?;
            l.tx_delete(b.id)?;
            l.budget_set(None, 2_000_00)
        })
        .unwrap();
        assert!(l.tx(a.id).is_err() && l.tx(b.id).is_err());
        assert_eq!(l.budget(None).unwrap(), Some(2_000_00));
        l.budget_set(None, 0).unwrap();
        assert_eq!(l.budget(None).unwrap(), None, "a removed budget reads as none");
        // A trial says what a change would do and leaves the ledger as it was.
        let restored = l.trial(|l| l.tx_restore(a.id)).unwrap();
        assert_eq!(restored.amount, 20_00);
        assert!(l.tx(a.id).is_err());
        assert!(matches!(l.trial(|l| l.budget_set(Some(category(l, "Salary")), 1_00)), Err(LedgerError::Invalid(_))));
    }

    #[test]
    fn switching_an_expense_to_income_drops_its_spending_category() {
        let (mut l, acct) = with_account();
        let c = category(&l, "Dining");
        let t = spend(&mut l, acct, c, 20_00, "2026-10-02");
        let t = l.tx_update(t.id, &TxPatch { r#type: Some(TxType::Income), ..Default::default() }).unwrap();
        assert_eq!(t.category_id, None);
    }

    /// A file that says nothing of the month's start day or the week's leaves this ledger's own,
    /// as a restore leaves the phone's; one that says replaces them.
    #[test]
    fn a_restore_silent_on_the_month_start_keeps_this_ledgers() {
        let mut l = Ledger::open_in_memory().unwrap();
        l.set_month_start_day(15).unwrap();
        Ledger::set_setting(&l.conn, "week_starts_monday", "1").unwrap();
        let today = date(2026, 10, 4);
        let mut sample = crate::sample_data::build(today, 2, "CAD", crate::sample_data::DEFAULT_SEED);
        (sample.month_start_day, sample.week_starts_monday) = (None, None);
        l.import_backup(&sample).unwrap();
        let settings = l.settings().unwrap();
        assert_eq!((settings.month_start_day, settings.week_starts_monday), (15, true));
        sample.month_start_day = Some(3);
        l.import_backup(&sample).unwrap();
        assert_eq!(l.settings().unwrap().month_start_day, 3);
    }

    #[test]
    fn the_sample_round_trips_through_a_backup_unchanged() {
        let mut l = Ledger::open_in_memory().unwrap();
        let today = date(2026, 10, 4);
        let n = l.load_sample(today, "CAD").unwrap();
        assert!(n > 50);
        let mut sample = crate::sample_data::build(today, 2, "CAD", crate::sample_data::DEFAULT_SEED);
        // The export lists rows by id; the sample builds them in its own order.
        sample.accounts.sort_by_key(|r| r.id);
        sample.categories.sort_by_key(|r| r.id);
        sample.transactions.sort_by_key(|r| r.id);
        sample.budgets.sort_by_key(|r| r.id);
        sample.recurring.sort_by_key(|r| r.id);
        sample.goals.sort_by_key(|r| r.id);
        sample.contributions.sort_by_key(|r| r.id);
        sample.values.sort_by_key(|r| r.id);
        sample.securities.sort_by_key(|r| r.id);
        sample.holdings.sort_by_key(|r| r.id);
        sample.activities.sort_by_key(|r| r.id);
        sample.prices.sort_by_key(|r| r.id);
        sample.fx_rates.sort_by_key(|r| r.id);
        sample.room_facts.sort_by_key(|r| r.id);
        let mut back = l.export_backup(&sample.exported_at).unwrap();
        // The backup does not say these for the sample; the ledger writes its own.
        back.month_start_day = sample.month_start_day;
        back.week_starts_monday = sample.week_starts_monday;
        // Nor its accounts' uids, which the ledger made when it loaded the sample.
        assert!(back.accounts.iter().all(|a| a.uid.as_deref().is_some_and(|u| !u.is_empty())));
        back.accounts.iter_mut().for_each(|a| a.uid = None);
        let (got, want) = (crate::backup::encode(&back), crate::backup::encode(&sample));
        if got != want {
            let _ = std::fs::write(std::env::temp_dir().join("ledger-got.json"), &got);
            let _ = std::fs::write(std::env::temp_dir().join("ledger-want.json"), &want);
        }
        assert!(got == want, "export differs from the sample; see ledger-got.json / ledger-want.json in the temp dir");
        assert!(matches!(l.load_sample(today, "CAD"), Err(LedgerError::Invalid(_))), "never into real entries");
    }

    #[test]
    fn a_restore_replaces_everything() {
        let (mut l, acct) = with_account();
        let c = category(&l, "Dining");
        spend(&mut l, acct, c, 20_00, "2026-10-02");
        let sample = crate::sample_data::build(date(2026, 10, 4), 2, "CAD", 7);
        l.import_backup(&sample).unwrap();
        assert_eq!(l.accounts().unwrap().len(), sample.accounts.len());
        assert!(l.accounts().unwrap().iter().all(|a| a.name.starts_with("Sample")));
        l.reset().unwrap();
        assert!(l.is_empty().unwrap());
    }

    #[test]
    fn due_bills_post_once_each_date() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        let mut file = l.export_backup("2026-10-01T00:00:00Z").unwrap();
        file.recurring.push(RecurringDto {
            id: 1, name: "Rent".into(), r#type: TxType::Expense, amount: 900_00, account_id: acct, to_account_id: None,
            category_id: Some(housing), frequency: Frequency::Monthly, interval: 1, anchor_date: "2026-08-01".into(),
            next_date: "2026-08-01".into(), end_date: None, auto_post: true, active: true,
        });
        l.import_backup(&file).unwrap();
        assert_eq!(l.post_due(date(2026, 10, 7)).unwrap(), 3, "August, September and October");
        assert_eq!(l.post_due(date(2026, 10, 7)).unwrap(), 0);
        let s = l.summary(date(2026, 10, 7), &en()).unwrap();
        assert_eq!(s.spent, 900_00);
        assert_eq!(s.bills[0].next_date, "2026-11-01");
        assert_eq!(s.bills[0].due_line, "Due in 25 days");
    }

    #[test]
    fn a_recorded_value_resets_an_investment_balance() {
        let (mut l, acct) = with_account();
        let tfsa = l.account_add("TFSA", AccountType::Investment, 5_000_00, Some(Registration::Tfsa), "Wealthsimple").unwrap().id;
        let mut file = l.export_backup("x").unwrap();
        file.values.push(AccountValueDto { id: 1, account_id: tfsa, date: "2026-10-01".into(), value: 6_000_00 });
        file.transactions.push(TransactionDto { id: 1, r#type: TxType::Transfer, amount: 100_00, date: "2026-09-15".into(), account_id: acct, to_account_id: Some(tfsa), category_id: None, note: String::new(), recurring_id: None });
        file.transactions.push(TransactionDto { id: 2, r#type: TxType::Transfer, amount: 50_00, date: "2026-10-03".into(), account_id: acct, to_account_id: Some(tfsa), category_id: None, note: String::new(), recurring_id: None });
        l.import_backup(&file).unwrap();
        let balance = l.accounts().unwrap().into_iter().find(|a| a.id == tfsa).unwrap().balance;
        assert_eq!(balance, 6_050_00, "the value holds September's transfer; October's adds to it");
    }

    #[test]
    fn the_currency_changes_here_only_when_its_decimals_do_not() {
        let (mut l, _) = with_account();
        assert!(l.set_currency("USD").is_ok());
        assert!(matches!(l.set_currency("JPY"), Err(LedgerError::Invalid(_))), "amounts would be read a hundred times too big");
        l.reset().unwrap();
        assert!(l.set_currency("JPY").is_ok(), "nothing to convert in an empty ledger");
    }

    #[test]
    fn an_erased_bill_entry_is_not_posted_again() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        let mut file = l.export_backup("x").unwrap();
        file.recurring.push(RecurringDto {
            id: 1, name: "Rent".into(), r#type: TxType::Expense, amount: 900_00, account_id: acct, to_account_id: None,
            category_id: Some(housing), frequency: Frequency::Monthly, interval: 1, anchor_date: "2026-10-01".into(),
            next_date: "2026-10-01".into(), end_date: None, auto_post: true, active: true,
        });
        l.import_backup(&file).unwrap();
        l.conn.execute("INSERT INTO tombstones (tbl, uid, updated_at) SELECT 'transactions', 'bill:' || uid || ':2026-10-01', 1 FROM recurring", []).unwrap();
        assert_eq!(l.post_due(date(2026, 10, 7)).unwrap(), 0);
    }

    // Twins of Tally's `RecurringPosterTest` (antho976/Tally, app/src/test/java/com/tally/app/data/)
    // for what the PC's poster shares with it: reminders moving on, a capped catch-up resumed,
    // transfer bills' references.

    /// Rent as `RecurringPosterTest.bill()` writes it: monthly from Jan 31, next due on the anchor.
    fn rent(id: i64, account: i64, category: i64) -> RecurringDto {
        RecurringDto {
            id, name: "Rent".into(), r#type: TxType::Expense, amount: 1_350_00, account_id: account, to_account_id: None,
            category_id: Some(category), frequency: Frequency::Monthly, interval: 1, anchor_date: "2026-01-31".into(),
            next_date: "2026-01-31".into(), end_date: None, auto_post: true, active: true,
        }
    }

    fn with_bills(l: &mut Ledger, bills: Vec<RecurringDto>) {
        let mut file = l.export_backup("x").unwrap();
        file.recurring.extend(bills);
        l.import_backup(&file).unwrap();
    }

    /// A bill's next date and whether it still runs.
    fn bill_state(l: &Ledger, id: i64) -> (String, bool) {
        l.conn.query_row("SELECT next_date, active FROM recurring WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
    }

    /// The entries bill `id` posted: date, to-account, category.
    fn posted(l: &Ledger, id: i64) -> Vec<(String, Option<i64>, Option<i64>)> {
        let mut st = l.conn.prepare("SELECT date, to_account_id, category_id FROM transactions WHERE deleted = 0 AND recurring_id = ?1 ORDER BY date").unwrap();
        st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().collect::<std::result::Result<_, _>>().unwrap()
    }

    fn entries(l: &Ledger) -> i64 {
        l.conn.query_row("SELECT COUNT(*) FROM transactions WHERE deleted = 0", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn paused_bills_and_reminders_never_post() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        with_bills(&mut l, vec![
            RecurringDto { name: "Paused".into(), active: false, ..rent(1, acct, housing) },
            RecurringDto { name: "Reminder".into(), auto_post: false, ..rent(2, acct, housing) },
        ]);
        assert_eq!(l.post_due(date(2026, 5, 15)).unwrap(), 0);
        assert_eq!(entries(&l), 0);
        assert_eq!(bill_state(&l, 1), ("2026-01-31".into(), false), "a paused bill keeps its date");
        assert_eq!(bill_state(&l, 2), ("2026-05-31".into(), true), "a reminder moves on to its next date");
    }

    #[test]
    fn a_reminder_stays_on_its_day_and_moves_on_once_the_day_is_over() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        with_bills(&mut l, vec![RecurringDto {
            name: "Phone".into(), amount: 45_00, anchor_date: "2026-01-15".into(), next_date: "2026-05-15".into(), auto_post: false,
            ..rent(1, acct, housing)
        }]);
        assert_eq!(l.post_due(date(2026, 5, 15)).unwrap(), 0);
        assert_eq!(bill_state(&l, 1).0, "2026-05-15", "due today stays due today");
        assert_eq!(l.post_due(date(2026, 5, 16)).unwrap(), 0);
        assert_eq!(bill_state(&l, 1).0, "2026-06-15");
        assert_eq!(entries(&l), 0);
    }

    #[test]
    fn a_reminder_left_for_months_is_not_overdue_once_the_poster_runs() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        // Due Jul 1 and paid by hand each month since; nothing ever marked it paid.
        with_bills(&mut l, vec![RecurringDto { anchor_date: "2026-07-01".into(), next_date: "2026-07-01".into(), auto_post: false, ..rent(1, acct, housing) }]);
        let today = date(2026, 10, 4);
        assert_eq!(l.post_due(today).unwrap(), 0);
        assert_eq!(bill_state(&l, 1).0, "2026-11-01");
        assert_eq!(entries(&l), 0);
        assert_eq!(l.summary(today, &en()).unwrap().bills[0].due_line, "Due in 28 days");
    }

    #[test]
    fn a_reminder_past_its_end_date_ends() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        with_bills(&mut l, vec![RecurringDto { name: "Lease".into(), end_date: Some("2026-04-30".into()), auto_post: false, ..rent(1, acct, housing) }]);
        l.post_due(date(2026, 5, 15)).unwrap();
        assert!(!bill_state(&l, 1).1);
        assert_eq!(entries(&l), 0);
    }

    #[test]
    fn transfer_bills_post_to_the_receiving_account_with_no_category() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        let savings = l.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
        // A category on a transfer bill is a data slip; the ledger entry must not carry it.
        with_bills(&mut l, vec![RecurringDto {
            name: "To savings".into(), r#type: TxType::Transfer, amount: 250_00, anchor_date: "2026-04-06".into(),
            next_date: "2026-04-06".into(), to_account_id: Some(savings), ..rent(1, acct, housing)
        }]);
        assert_eq!(l.post_due(date(2026, 5, 15)).unwrap(), 2);
        assert_eq!(posted(&l, 1), vec![("2026-04-06".into(), Some(savings), None), ("2026-05-06".into(), Some(savings), None)]);
    }

    #[test]
    fn an_expense_bill_never_posts_a_receiving_account() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        let savings = l.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
        with_bills(&mut l, vec![RecurringDto { anchor_date: "2026-05-01".into(), next_date: "2026-05-01".into(), to_account_id: Some(savings), ..rent(1, acct, housing) }]);
        assert_eq!(l.post_due(date(2026, 5, 15)).unwrap(), 1);
        assert_eq!(posted(&l, 1), vec![("2026-05-01".into(), None, Some(housing))]);
    }

    #[test]
    fn a_long_backlog_is_paid_over_several_runs_and_never_twice() {
        let (mut l, acct) = with_account();
        let housing = category(&l, "Housing");
        let today = date(2026, 5, 15);
        let anchor = today.checked_sub(100.weeks()).unwrap();
        with_bills(&mut l, vec![RecurringDto {
            name: "Lessons".into(), amount: 20_00, frequency: Frequency::Weekly, anchor_date: anchor.to_string(), next_date: anchor.to_string(),
            ..rent(1, acct, housing)
        }]);
        let first = l.post_due(today).unwrap();
        assert_eq!(first, CATCH_UP_LIMIT);
        let resume = anchor.checked_add((CATCH_UP_LIMIT as i64).weeks()).unwrap();
        assert_eq!(bill_state(&l, 1).0, resume.to_string(), "the dates past the cap are still owed");
        let second = l.post_due(today).unwrap();
        assert_eq!(first + second, 101, "weeks 0 through 100 inclusive");
        assert_eq!(l.post_due(today).unwrap(), 0);
        let dates: Vec<String> = posted(&l, 1).into_iter().map(|p| p.0).collect();
        assert_eq!(dates.iter().collect::<HashSet<_>>().len(), 101);
        assert_eq!(dates.last().unwrap(), &today.to_string());
        assert_eq!(bill_state(&l, 1).0, today.checked_add(1.weeks()).unwrap().to_string());
    }

    #[test]
    fn every_migration_has_its_version() {
        assert_eq!(SCHEMA_VERSION as usize, MIGRATIONS.len());
    }

    #[test]
    fn a_version_3_ledger_opens_with_everything_in_it() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..3] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 3).unwrap();
        conn.execute_batch(
            "INSERT INTO settings (key, value) VALUES ('currency', 'CAD');
             INSERT INTO accounts (uid, name, type, opening_balance, updated_at) VALUES ('a1', 'Chequing', 'CHEQUING', 100000, 1);
             INSERT INTO accounts (uid, name, type, opening_balance, updated_at) VALUES ('a2', 'TFSA', 'INVESTMENT', 500000, 1);
             INSERT INTO transactions (uid, type, amount, date, account_id, to_account_id, created_at, updated_at)
                 VALUES ('t1', 'TRANSFER', 5000, '2026-10-03', 1, 2, 1, 1);
             INSERT INTO account_values (uid, account_id, date, value, updated_at) VALUES ('v1', 2, '2026-10-01', 600000, 1);",
        )
        .unwrap();
        let mut l = Ledger::init(conn, None).unwrap();
        assert_eq!(4i64, l.conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap());
        let accounts = l.accounts().unwrap();
        let tfsa = accounts.iter().find(|a| a.name == "TFSA").unwrap();
        assert_eq!((tfsa.balance, tfsa.registration, tfsa.institution.as_str()), (6_050_00, None, ""));
        assert_eq!(accounts.iter().find(|a| a.name == "Chequing").unwrap().balance, 950_00);
        let p = l.portfolio(date(2026, 10, 9)).unwrap();
        assert_eq!((p.empty, p.accounts.len(), p.value), (false, 1, 0));
        // The new tables number their rows for sync like the old ones.
        let before = l.cursor().unwrap();
        l.room_set(Registration::Tfsa, 2026, 7_000_00).unwrap();
        assert!(l.cursor().unwrap() > before);
        let out = l.sync("Pixel", false, before, None, &[], "t").unwrap();
        assert!(out.changes.iter().any(|c| c.table == "room_facts" && c.uid == "room:TFSA:2026"));
    }

    #[test]
    fn a_ledger_file_reopens_with_its_entries() {
        let dir = std::env::temp_dir().join(format!("relay-money-{}", new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("money.db");
        {
            let mut l = Ledger::open(&path).unwrap();
            let a = l.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
            let c = category(&l, "Dining");
            spend(&mut l, a, c, 4_00, "2026-10-02");
        }
        // Only the user reads it, nor its side files, whatever mode an older install left.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let l = Ledger::open(&path).unwrap();
        assert_eq!(l.tx_list(&TxQuery::default(), date(2026, 10, 3)).unwrap().spent, 4_00);
        for file in ["money.db", "money.db-wal", "money.db-shm"] {
            assert_eq!(std::fs::metadata(dir.join(file)).unwrap().permissions().mode() & 0o777, 0o600, "{file}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
