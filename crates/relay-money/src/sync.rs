//! Two-way sync between a device's ledger and this one (docs/MONEY.md, "Sync").
//!
//! Each side holds the whole ledger. A device sends the rows it changed since its last sync and
//! the cursor this ledger gave it then; this ledger applies them, newest `updated_at` winning per
//! row, and answers with every row it numbered above that cursor. References travel as uids, so
//! local ids never leave the device. A device's first sync replaces this ledger outright: the
//! phone is where the ledger lives.

use crate::ledger::{now_ms, stamp_ok, Ledger, LedgerError, Result};
use crate::views::{Change, SyncOut};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::PathBuf;

/// How many copies of the ledger a replace keeps in `money-backups/` beside `money.db`: the newest
/// this many, and the first of each of the last [`SNAPSHOT_DAYS`] days.
pub const SNAPSHOTS_KEPT: usize = 10;
pub const SNAPSHOT_DAYS: usize = 14;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Int,
    Text,
    Bool,
    OptText,
    OptInt,
    /// Text a device from before the column was added leaves out: missing, it reads as empty.
    TextOrEmpty,
    /// A reference to another table's row, by uid on the wire and by id here.
    Ref(&'static str),
    OptRef(&'static str),
}

struct Col {
    json: &'static str,
    sql: &'static str,
    kind: Kind,
}

const fn col(json: &'static str, sql: &'static str, kind: Kind) -> Col {
    Col { json, sql, kind }
}

/// The synced tables, in the order a batch is applied: what a row refers to comes first.
const TABLES: &[(&str, &[Col])] = &[
    ("accounts", &[
        col("name", "name", Kind::Text), col("type", "type", Kind::Text), col("openingBalance", "opening_balance", Kind::Int),
        col("archived", "archived", Kind::Bool), col("sortOrder", "sort_order", Kind::Int),
        col("registration", "registration", Kind::OptText), col("institution", "institution", Kind::TextOrEmpty),
        col("externalRef", "external_ref", Kind::TextOrEmpty),
    ]),
    ("categories", &[
        col("name", "name", Kind::Text), col("kind", "kind", Kind::Text), col("color", "color", Kind::Int),
        col("icon", "icon", Kind::Text), col("archived", "archived", Kind::Bool), col("sortOrder", "sort_order", Kind::Int),
    ]),
    ("recurring", &[
        col("name", "name", Kind::Text), col("type", "type", Kind::Text), col("amount", "amount", Kind::Int),
        col("account", "account_id", Kind::Ref("accounts")), col("toAccount", "to_account_id", Kind::OptRef("accounts")),
        col("category", "category_id", Kind::OptRef("categories")), col("frequency", "frequency", Kind::Text),
        col("interval", "interval", Kind::Int), col("anchorDate", "anchor_date", Kind::Text), col("nextDate", "next_date", Kind::Text),
        col("endDate", "end_date", Kind::OptText), col("autoPost", "auto_post", Kind::Bool), col("active", "active", Kind::Bool),
    ]),
    ("goals", &[
        col("name", "name", Kind::Text), col("target", "target", Kind::Int), col("targetDate", "target_date", Kind::OptText),
        col("color", "color", Kind::Int), col("archived", "archived", Kind::Bool), col("kind", "kind", Kind::Text),
        col("account", "account_id", Kind::OptRef("accounts")), col("percent", "percent", Kind::Int),
        col("startDate", "start_date", Kind::OptText), col("startAmount", "start_amount", Kind::Int),
    ]),
    ("transactions", &[
        col("type", "type", Kind::Text), col("amount", "amount", Kind::Int), col("date", "date", Kind::Text),
        col("account", "account_id", Kind::Ref("accounts")), col("toAccount", "to_account_id", Kind::OptRef("accounts")),
        col("category", "category_id", Kind::OptRef("categories")), col("note", "note", Kind::Text),
        col("recurring", "recurring_id", Kind::OptRef("recurring")), col("createdAt", "created_at", Kind::Int),
    ]),
    // Matched by category, not uid: each category has one budget. 0 is the overall budget.
    ("budgets", &[col("category", "category_id", Kind::OptRef("categories")), col("amount", "amount", Kind::Int)]),
    ("contributions", &[
        col("goal", "goal_id", Kind::Ref("goals")), col("amount", "amount", Kind::Int), col("date", "date", Kind::Text),
        col("note", "note", Kind::Text),
    ]),
    ("account_values", &[
        col("account", "account_id", Kind::Ref("accounts")), col("date", "date", Kind::Text), col("value", "value", Kind::Int),
    ]),
    // Investments (docs/INVESTMENTS.md), after what they refer to.
    ("securities", &[
        col("symbol", "symbol", Kind::Text), col("name", "name", Kind::TextOrEmpty), col("currency", "currency", Kind::Text),
        col("kind", "kind", Kind::Text), col("exchange", "exchange", Kind::TextOrEmpty),
    ]),
    ("holdings", &[
        col("account", "account_id", Kind::Ref("accounts")), col("security", "security_id", Kind::Ref("securities")),
        col("date", "date", Kind::Text), col("quantity", "quantity", Kind::Int), col("book", "book", Kind::Int),
        col("bookMarket", "book_market", Kind::Int),
    ]),
    ("activities", &[
        col("account", "account_id", Kind::Ref("accounts")), col("security", "security_id", Kind::OptRef("securities")),
        col("type", "type", Kind::Text), col("date", "date", Kind::Text), col("quantity", "quantity", Kind::Int),
        col("amount", "amount", Kind::Int), col("fee", "fee", Kind::Int), col("currency", "currency", Kind::Text),
        col("toAmount", "to_amount", Kind::OptInt), col("toCurrency", "to_currency", Kind::OptText), col("note", "note", Kind::TextOrEmpty),
        col("source", "source", Kind::Text), col("createdAt", "created_at", Kind::Int),
    ]),
    ("prices", &[
        col("security", "security_id", Kind::Ref("securities")), col("date", "date", Kind::Text), col("price", "price", Kind::Int),
        col("source", "source", Kind::Text),
    ]),
    ("fx_rates", &[
        col("base", "base", Kind::Text), col("quote", "quote", Kind::Text), col("date", "date", Kind::Text), col("rate", "rate", Kind::Int),
        col("source", "source", Kind::Text),
    ]),
    ("room_facts", &[col("registration", "registration", Kind::Text), col("year", "year", Kind::Int), col("amount", "amount", Kind::Int)]),
];

/// The settings that travel; the rest are this device's own.
const SYNCED_SETTINGS: &[&str] = &["currency", "month_start_day", "week_starts_monday"];

/// The prefixes of uids both devices derive the same way: a posted bill's, and what an import
/// writes (docs/INVESTMENTS.md). Tally's `SyncTables.DERIVED`. Such a row is written only when its
/// uid is neither present nor deleted, so its tombstone is kept through a replace.
pub const DERIVED: &[&str] = &["bill:", "imp:", "sec:", "hold:", "px:", "fx:", "room:", "val:", "ws:"];

pub fn is_derived(uid: &str) -> bool {
    DERIVED.iter().any(|p| uid.starts_with(p))
}

/// The stamp of a tombstone a replace keeps: older than any edit, so a row the device sends
/// (in this request or a later batch of the same first sync) always wins over it. It is there
/// for its presence, which keeps an import or the poster from writing that uid again.
const KEPT_STAMP: i64 = 1;

/// What a budget tombstone keeps in `category` when the device did not say which budget it was:
/// it is matched by uid, and goes out again with `row: {}`. Null is the overall budget.
const UNSAID: &str = "";

fn spec(table: &str) -> Option<&'static [Col]> {
    TABLES.iter().find(|(t, _)| *t == table).map(|(_, c)| *c)
}

/// Where `table`'s changes go in a batch: what a row refers to comes first. A device that splits
/// its changes over several requests keeps this order across them.
pub fn order(table: &str) -> usize {
    if table == "settings" {
        return 0;
    }
    TABLES.iter().position(|(t, _)| *t == table).map_or(usize::MAX, |i| i + 1)
}

fn id_of(tx: &Transaction, table: &str, uid: &str) -> Result<Option<i64>> {
    Ok(tx.prepare_cached(&format!("SELECT id FROM {table} WHERE uid = ?1"))?.query_row([uid], |r| r.get(0)).optional()?)
}

fn uid_of(tx: &Transaction, table: &str, id: i64) -> Result<Option<String>> {
    Ok(tx.prepare_cached(&format!("SELECT uid FROM {table} WHERE id = ?1"))?.query_row([id], |r| r.get(0)).optional()?)
}

/// A wire value as SQL, references resolved to local ids. `Err(())`: a value of the wrong
/// shape, or a reference to a row this ledger does not have; the change is skipped.
fn to_sql(tx: &Transaction, c: &Col, v: Option<&Value>) -> Result<std::result::Result<rusqlite::types::Value, ()>> {
    use rusqlite::types::Value as Sql;
    let v = v.filter(|v| !v.is_null());
    Ok(match (c.kind, v) {
        (Kind::Int | Kind::OptInt, Some(Value::Number(n))) => n.as_i64().map(Sql::Integer).ok_or(()),
        (Kind::Bool, Some(Value::Bool(b))) => Ok(Sql::Integer(i64::from(*b))),
        (Kind::Text, Some(Value::String(s))) => Ok(Sql::Text(s.clone())),
        (Kind::OptText | Kind::TextOrEmpty, Some(Value::String(s))) => Ok(Sql::Text(s.clone())),
        (Kind::OptText | Kind::OptInt | Kind::OptRef(_), None) => Ok(Sql::Null),
        (Kind::TextOrEmpty, None) => Ok(Sql::Text(String::new())),
        (Kind::Ref(t) | Kind::OptRef(t), Some(Value::String(uid))) => match id_of(tx, t, uid)? {
            Some(id) => Ok(Sql::Integer(id)),
            None => Err(()),
        },
        _ => Err(()),
    })
}

fn to_json(tx: &Transaction, c: &Col, v: rusqlite::types::Value) -> Result<Value> {
    use rusqlite::types::Value as Sql;
    Ok(match (c.kind, v) {
        (_, Sql::Null) => Value::Null,
        (Kind::Bool, Sql::Integer(i)) => Value::Bool(i != 0),
        (Kind::Ref(t) | Kind::OptRef(t), Sql::Integer(id)) => uid_of(tx, t, id)?.map_or(Value::Null, Value::String),
        (_, Sql::Integer(i)) => Value::from(i),
        (_, Sql::Text(s)) => Value::String(s),
        (_, Sql::Real(f)) => Value::from(f),
        (_, Sql::Blob(_)) => Value::Null,
    })
}

impl Ledger {
    /// The cursor a device stores: the highest change number so far.
    pub fn cursor(&self) -> Result<i64> {
        Ok(self.conn.prepare_cached("SELECT n FROM sync_counter")?.query_row([], |r| r.get(0))?)
    }

    /// Applies `changes` from `device` and answers with this ledger's changes after `since`.
    /// With `replace`, this ledger is erased first and becomes the device's (its first sync); a
    /// ledger that holds anything is copied to `money-backups/` first ([`Ledger::snapshot`]).
    ///
    /// A change stamped outside [`stamp_ok`] (below 0, or more than a day ahead of this clock) is
    /// not applied, and counts as skipped: the rest of the request goes on.
    ///
    /// `generation` is the one this ledger gave the device last time. A device whose ledger is not
    /// this one any more (restored, erased or replaced since, or a cursor this ledger never gave)
    /// is refused with [`LedgerError::Stale`]: it then takes this ledger whole, asking with
    /// `since: 0`, no generation and no changes, rather than merging into a ledger it no longer
    /// shares rows with.
    pub fn sync(&mut self, device: &str, replace: bool, since: i64, generation: Option<&str>, changes: &[Change], now: &str) -> Result<SyncOut> {
        self.sync_page(device, replace, since, generation, changes, None, now)
    }

    /// [`Ledger::sync`], with the answer cut to at most `limit` changes. The changes go in the
    /// order this ledger numbered them; when some are left, the answer says `more` and its
    /// `cursor` is the number of the last one sent, which the device sends back as `since` to
    /// ask for the rest. A page may refer to rows a later page holds: a device applies the pages
    /// together once `more` is false. Without `limit`, the whole answer at once.
    #[allow(clippy::too_many_arguments)]
    pub fn sync_page(
        &mut self,
        device: &str,
        replace: bool,
        since: i64,
        generation: Option<&str>,
        changes: &[Change],
        limit: Option<usize>,
        now: &str,
    ) -> Result<SyncOut> {
        if !replace {
            let mine = self.generation()?;
            if generation.is_some_and(|g| g != mine) || since > self.cursor()? {
                return Err(LedgerError::Stale(
                    "This PC's ledger changed since this phone last synced: restored, erased or replaced. Take the PC's ledger, or send this phone's again.".into(),
                ));
            }
        }
        // Whoever sends a replace (any paired device, a cloud copy's included) erases this ledger:
        // what it held is kept on disk first, and a copy that cannot be kept stops the replace.
        if replace && self.holds_anything()? {
            self.snapshot()?;
        }
        let clock = now_ms();
        let tx = self.conn.transaction()?;
        // A derived row deleted here stays deleted through the replace: the device's first sync
        // sends no tombstones, and the next import of the same file would bring it back.
        let kept = if replace { derived_buried(&tx)? } else { Vec::new() };
        if replace {
            Ledger::clear(&tx, true)?;
        }
        let (mut sorted, out_of_range): (Vec<&Change>, Vec<&Change>) = changes.iter().partition(|c| stamp_ok(c.updated_at, clock));
        sorted.sort_by_key(|c| order(&c.table));
        let mut applied = HashSet::new();
        let mut skipped = out_of_range.len();
        for c in sorted {
            if apply(&tx, c)? {
                applied.insert((c.table.clone(), c.uid.clone()));
            } else {
                skipped += 1;
            }
        }
        for (table, uid) in &kept {
            let here = tx.prepare_cached(&format!("SELECT 1 FROM {table} WHERE uid = ?1"))?.exists([uid])?;
            if !here {
                tx.prepare_cached("INSERT OR IGNORE INTO tombstones (tbl, uid, updated_at) VALUES (?1, ?2, ?3)")?
                    .execute(params![table, uid, KEPT_STAMP])?;
            }
        }
        let (changes, page) = if replace { (Vec::new(), None) } else { changed_since(&tx, since, &applied, limit)? };
        tx.prepare_cached(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, 0) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )?.execute(params![format!("sync.device.{device}"), now])?;
        tx.commit()?;
        let cursor = match page {
            Some(last) => last,
            None => self.cursor()?,
        };
        Ok(SyncOut { cursor, generation: self.generation()?, changes, replaced: replace, applied: applied.len(), skipped, more: page.is_some() })
    }

    /// Whether any row is live here but the categories, which every new ledger is given.
    pub(crate) fn holds_anything(&self) -> Result<bool> {
        for (table, _) in TABLES.iter().filter(|(t, _)| *t != "categories") {
            if self.conn.prepare_cached(&format!("SELECT 1 FROM {table} WHERE deleted = 0 LIMIT 1"))?.exists([])? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Copies this ledger, whole, to `money-backups/money-<time>-<n>.db` beside its file (0700
    /// folder, 0600 file), and keeps the newest [`SNAPSHOTS_KEPT`] and the first of each of the last
    /// [`SNAPSHOT_DAYS`] days. A copy is a `money.db` like any
    /// other: put it in place of `money.db` while Relay is stopped to go back to it. `None` in memory.
    pub fn snapshot(&self) -> Result<Option<PathBuf>> {
        let Some(path) = &self.path else { return Ok(None) };
        let failed = |e: std::io::Error| LedgerError::Invalid(format!("Could not keep a copy of the ledger before replacing it, so nothing was replaced: {e}"));
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new(".")).join("money-backups");
        private_dir(&dir).map_err(failed)?;
        // To the millisecond, so the names sort as the copies were made, two in one second included.
        let at = jiff::Timestamp::now();
        let name = format!("money-{}.{:03}Z-{}.db", at.strftime("%Y%m%dT%H%M%S"), at.subsec_millisecond(), &crate::ledger::new_uid()[..8]);
        let to = dir.join(name);
        self.conn.execute("VACUUM INTO ?1", [to.to_string_lossy().into_owned()])?;
        private_file(&to).map_err(failed)?;
        // The newest stay: the names sort by time.
        let mut kept: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(failed)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("money-") && n.ends_with(".db")))
            .collect();
        kept.sort();
        // Any credential that may sync can send a replace, so a burst of them would push the newest
        // ten out within a minute; the first copy of a day is the ledger before its first replace.
        let mut firsts: std::collections::BTreeMap<&str, &PathBuf> = std::collections::BTreeMap::new();
        for p in &kept {
            if let Some(day) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.get(6..14)) {
                firsts.entry(day).or_insert(p);
            }
        }
        let pinned: Vec<&PathBuf> = firsts.values().rev().take(SNAPSHOT_DAYS).copied().collect();
        for old in kept.iter().rev().skip(SNAPSHOTS_KEPT).filter(|p| !pinned.contains(p)) {
            let _ = std::fs::remove_file(old);
        }
        Ok(Some(to))
    }

    /// Devices that have synced with this ledger and when they last did, newest first.
    pub fn synced_devices(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare_cached("SELECT substr(key, 13), value FROM settings WHERE key LIKE 'sync.device.%' ORDER BY value DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(unix)]
fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn private_file(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn private_file(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Applies one change; false when it was older than what is here or could not be placed.
fn apply(tx: &Transaction, c: &Change) -> Result<bool> {
    if c.table == "settings" {
        if !SYNCED_SETTINGS.contains(&c.uid.as_str()) {
            return Ok(false);
        }
        let mine: Option<i64> = tx.prepare_cached("SELECT updated_at FROM settings WHERE key = ?1")?.query_row([&c.uid], |r| r.get(0)).optional()?;
        if mine.is_some_and(|m| m >= c.updated_at) {
            return Ok(false);
        }
        if c.deleted {
            tx.prepare_cached("DELETE FROM settings WHERE key = ?1")?.execute([&c.uid])?;
            return Ok(true);
        }
        let Some(value) = c.row.get("value").and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
            _ => None,
        }) else {
            return Ok(false);
        };
        tx.prepare_cached(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )?.execute(params![c.uid, value, c.updated_at])?;
        return Ok(true);
    }
    let Some(cols) = spec(&c.table) else { return Ok(false) };
    let table = c.table.as_str();
    // An erased row stays erased against anything older.
    let buried: Option<i64> = tx.prepare_cached("SELECT updated_at FROM tombstones WHERE tbl = ?1 AND uid = ?2")?
        .query_row(params![table, c.uid], |r| r.get(0)).optional()?;
    if buried.is_some_and(|t| t >= c.updated_at) {
        return Ok(false);
    }
    // Budgets are one per category, whatever uid each device gave theirs.
    // A budget tombstone that does not say its category is matched by uid, never read as the
    // overall budget.
    let by_category = table == "budgets" && (!c.deleted || c.row.contains_key("category"));
    let existing: Option<(i64, i64)> = if by_category {
        let category = match c.row.get("category").filter(|v| !v.is_null()) {
            None => Some(0),
            Some(Value::String(uid)) => id_of(tx, "categories", uid)?,
            Some(_) => None,
        };
        let Some(category) = category else { return Ok(false) };
        tx.prepare_cached("SELECT id, updated_at FROM budgets WHERE category_id = ?1")?.query_row([category], |r| Ok((r.get(0)?, r.get(1)?))).optional()?
    } else {
        tx.prepare_cached(&format!("SELECT id, updated_at FROM {table} WHERE uid = ?1"))?
            .query_row([&c.uid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?
    };
    if existing.is_some_and(|(_, mine)| mine >= c.updated_at) {
        return Ok(false);
    }
    if c.deleted {
        match existing {
            Some((id, _)) => {
                tx.prepare_cached(&format!("UPDATE {table} SET deleted = 1, updated_at = ?2 WHERE id = ?1"))?.execute(params![id, c.updated_at])?;
            }
            None => {
                // A budget's says which budget it was, or that it did not say: never the overall
                // one by omission.
                let category = match c.row.get("category") {
                    None if table == "budgets" => Some(UNSAID),
                    v => v.and_then(Value::as_str),
                };
                tx.prepare_cached("INSERT OR REPLACE INTO tombstones (tbl, uid, updated_at, category) VALUES (?1, ?2, ?3, ?4)")?
                    .execute(params![table, c.uid, c.updated_at, category])?;
            }
        }
        return Ok(true);
    }
    let mut values = Vec::with_capacity(cols.len());
    for col in cols {
        let mut v = to_sql(tx, col, c.row.get(col.json))?;
        // The overall budget has no category: 0 here.
        if table == "budgets" && col.json == "category" && v == Ok(rusqlite::types::Value::Null) {
            v = Ok(rusqlite::types::Value::Integer(0));
        }
        match v {
            Ok(v) => values.push(v),
            Err(()) => return Ok(false),
        }
    }
    match existing {
        Some((id, _)) => {
            let sets: Vec<String> = cols.iter().enumerate().map(|(i, col)| format!("{} = ?{}", col.sql, i + 3)).collect();
            let sql = format!("UPDATE {table} SET updated_at = ?2, deleted = 0, {} WHERE id = ?1", sets.join(", "));
            let mut args: Vec<rusqlite::types::Value> = vec![id.into(), c.updated_at.into()];
            args.extend(values);
            tx.prepare_cached(&sql)?.execute(rusqlite::params_from_iter(args))?;
        }
        None => {
            let names: Vec<&str> = cols.iter().map(|col| col.sql).collect();
            let marks: Vec<String> = (0..cols.len()).map(|i| format!("?{}", i + 3)).collect();
            let sql = format!("INSERT INTO {table} (uid, updated_at, {}) VALUES (?1, ?2, {})", names.join(", "), marks.join(", "));
            let mut args: Vec<rusqlite::types::Value> = vec![c.uid.clone().into(), c.updated_at.into()];
            args.extend(values);
            tx.prepare_cached(&sql)?.execute(rusqlite::params_from_iter(args))?;
        }
    }
    tx.prepare_cached("DELETE FROM tombstones WHERE tbl = ?1 AND uid = ?2")?.execute(params![table, c.uid])?;
    Ok(true)
}

/// Every derived uid this ledger holds deleted, as `(table, uid)`: its tombstones and its
/// deleted rows.
fn derived_buried(tx: &Transaction) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for (table, _) in TABLES {
        let mut st = tx.prepare_cached(&format!("SELECT uid FROM {table} WHERE deleted = 1"))?;
        let uids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        out.extend(uids.into_iter().filter(|u| is_derived(u)).map(|u| (table.to_string(), u)));
    }
    let mut st = tx.prepare_cached("SELECT tbl, uid FROM tombstones")?;
    let rows: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
    // Only a synced table's: its name goes into SQL.
    out.extend(rows.into_iter().filter(|(t, u)| spec(t).is_some() && is_derived(u)));
    Ok(out)
}

/// Every row and tombstone numbered above `since`, but the ones this sync just applied: the
/// device already has those. With `limit`, the first that many in the order they were numbered,
/// and the number of the last one when more are left.
fn changed_since(tx: &Transaction, since: i64, applied: &HashSet<(String, String)>, limit: Option<usize>) -> Result<(Vec<Change>, Option<i64>)> {
    // Each table's first rows are enough for a page: the ones applied are skipped, and the rest
    // of a page's rows are each table's earliest.
    let per_table = limit.map_or(-1, |l| i64::try_from(l.saturating_add(applied.len()).saturating_add(1)).unwrap_or(i64::MAX));
    let mut out: Vec<(i64, Change)> = Vec::new();
    let mut st = tx.prepare_cached("SELECT key, value, updated_at, seq FROM settings WHERE seq > ?1")?;
    let rows: Vec<(String, String, i64, i64)> = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
    for (key, value, updated_at, seq) in rows {
        if SYNCED_SETTINGS.contains(&key.as_str()) && !applied.contains(&("settings".to_string(), key.clone())) {
            let mut row = Map::new();
            row.insert("value".into(), Value::String(value));
            out.push((seq, Change { table: "settings".into(), uid: key, updated_at, deleted: false, row }));
        }
    }
    for (table, cols) in TABLES {
        let names: Vec<&str> = cols.iter().map(|c| c.sql).collect();
        let sql = format!("SELECT uid, updated_at, deleted, seq, {} FROM {table} WHERE seq > ?1 ORDER BY seq LIMIT ?2", names.join(", "));
        let rows: Vec<(String, i64, bool, i64, Vec<rusqlite::types::Value>)> = {
            let mut st = tx.prepare_cached(&sql)?;
            let mapped = st.query_map(params![since, per_table], |r| {
                let mut vals = Vec::with_capacity(cols.len());
                for i in 0..cols.len() {
                    vals.push(r.get::<_, rusqlite::types::Value>(i + 4)?);
                }
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, vals))
            })?;
            mapped.collect::<std::result::Result<_, _>>()?
        };
        for (uid, updated_at, deleted, seq, vals) in rows {
            if applied.contains(&(table.to_string(), uid.clone())) {
                continue;
            }
            let mut row = Map::new();
            for (c, v) in cols.iter().zip(vals) {
                // The overall budget's category 0 matches no row, so it travels as null.
                row.insert(c.json.into(), to_json(tx, c, v)?);
            }
            out.push((seq, Change { table: table.to_string(), uid, updated_at, deleted, row }));
        }
    }
    let mut st = tx.prepare_cached("SELECT tbl, uid, updated_at, category, seq FROM tombstones WHERE seq > ?1 ORDER BY seq LIMIT ?2")?;
    let rows: Vec<(String, String, i64, Option<String>, i64)> =
        st.query_map(params![since, per_table], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<std::result::Result<_, _>>()?;
    for (table, uid, updated_at, category, seq) in rows {
        if !applied.contains(&(table.clone(), uid.clone())) {
            let mut row = Map::new();
            // A budget's tombstone says which budget: null is the overall one. One the device
            // sent without saying goes out without saying, matched by its uid.
            if table == "budgets" && category.as_deref() != Some(UNSAID) {
                row.insert("category".into(), category.map_or(Value::Null, Value::String));
            }
            out.push((seq, Change { table, uid, updated_at, deleted: true, row }));
        }
    }
    let Some(limit) = limit.filter(|&l| out.len() > l) else {
        return Ok((out.into_iter().map(|(_, c)| c).collect(), None));
    };
    out.sort_by_key(|(seq, _)| *seq);
    out.truncate(limit.max(1));
    let last = out.last().map(|(seq, _)| *seq);
    // In dependency order within the page, as a whole answer is.
    out.sort_by_key(|(_, c)| order(&c.table));
    Ok((out.into_iter().map(|(_, c)| c).collect(), last))
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use crate::ledger::{TxInput, TxQuery};
    use crate::model::TxType;
    use crate::money::Locale;
    use jiff::civil::date;
    use serde_json::json;

    fn change(table: &str, uid: &str, at: i64, row: Value) -> Change {
        Change { table: table.into(), uid: uid.into(), updated_at: at, deleted: false, row: row.as_object().unwrap().clone() }
    }

    fn tomb(table: &str, uid: &str, at: i64) -> Change {
        Change { table: table.into(), uid: uid.into(), updated_at: at, deleted: true, row: Map::new() }
    }

    /// What a phone with one account, one category, a budget and one entry sends first.
    fn phone() -> Vec<Change> {
        vec![
            change("transactions", "t1", 100, json!({"type": "EXPENSE", "amount": 42_50, "date": "2026-10-05", "account": "a1",
                "toAccount": null, "category": "c1", "note": "Metro", "recurring": null, "createdAt": 100})),
            change("accounts", "a1", 100, json!({"name": "Chequing", "type": "CHEQUING", "openingBalance": 1_000_00, "archived": false, "sortOrder": 0})),
            change("categories", "c1", 100, json!({"name": "Groceries", "kind": "EXPENSE", "color": 0, "icon": "cart", "archived": false, "sortOrder": 0})),
            change("budgets", "b1", 100, json!({"category": null, "amount": 310_00})),
            change("settings", "currency", 100, json!({"value": "CAD"})),
        ]
    }

    fn spent(l: &Ledger) -> i64 {
        l.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap().spent
    }

    #[test]
    fn a_first_sync_makes_this_ledger_the_phones() {
        let mut pc = Ledger::open_in_memory().unwrap();
        pc.account_add("Old PC account", crate::model::AccountType::Cash, 5_00, None, "").unwrap();
        let out = pc.sync("Pixel", true, 0, None, &phone(), "2026-10-07T12:00:00Z").unwrap();
        assert!(out.replaced);
        assert_eq!((out.applied, out.skipped), (5, 0), "applied in dependency order, whatever order they came in");
        assert!(out.changes.is_empty(), "nothing to send back: the phone has it all");
        let accounts = pc.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!((accounts[0].name.as_str(), accounts[0].balance), ("Chequing", 957_50));
        let s = pc.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap();
        assert_eq!((s.spent, s.pace.budget), (42_50, 310_00));
        assert_eq!(pc.synced_devices().unwrap()[0].0, "Pixel");
    }

    #[test]
    fn newest_edit_wins_per_row() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let older = change("transactions", "t1", 50, json!({"type": "EXPENSE", "amount": 1, "date": "2026-10-05", "account": "a1",
            "toAccount": null, "category": "c1", "note": "", "recurring": null, "createdAt": 100}));
        let out = pc.sync("Pixel", false, first.cursor, None, &[older], "t").unwrap();
        assert_eq!((out.applied, out.skipped), (0, 1));
        assert_eq!(spent(&pc), 42_50);
        let newer = change("transactions", "t1", 200, json!({"type": "EXPENSE", "amount": 60_00, "date": "2026-10-05", "account": "a1",
            "toAccount": null, "category": "c1", "note": "Metro", "recurring": null, "createdAt": 100}));
        pc.sync("Pixel", false, out.cursor, None, &[newer], "t").unwrap();
        assert_eq!(spent(&pc), 60_00);
    }

    #[test]
    fn the_pc_sends_back_only_what_the_phone_has_not_seen() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let account = pc.accounts().unwrap()[0].id;
        let category = pc.categories().unwrap()[0].id;
        let added = pc.tx_add(&TxInput { r#type: TxType::Expense, amount: 9_00, date: "2026-10-06".into(), account_id: account,
            to_account_id: None, category_id: Some(category), note: Some("Coffee".into()) }).unwrap();
        let out = pc.sync("Pixel", false, first.cursor, None, &[], "t").unwrap();
        assert_eq!(out.changes.len(), 1, "{:?}", out.changes);
        let c = &out.changes[0];
        assert_eq!((c.table.as_str(), c.uid.as_str()), ("transactions", added.uid.as_str()));
        assert_eq!(c.row["account"], "a1", "references travel as uids");
        assert_eq!(c.row["category"], "c1");
        assert_eq!(c.row["note"], "Coffee");
        let again = pc.sync("Pixel", false, out.cursor, None, &[], "t").unwrap();
        assert!(again.changes.is_empty());
    }

    #[test]
    fn deletes_travel_both_ways() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let out = pc.sync("Pixel", false, first.cursor, None, &[tomb("transactions", "t1", 300)], "t").unwrap();
        assert_eq!(spent(&pc), 0);
        assert!(out.changes.is_empty(), "the phone deleted it; it needs no echo");
        // An older edit of the deleted row does not bring it back.
        let stale = change("transactions", "t1", 250, json!({"type": "EXPENSE", "amount": 1, "date": "2026-10-05", "account": "a1",
            "toAccount": null, "category": "c1", "note": "", "recurring": null, "createdAt": 100}));
        pc.sync("Pixel", false, out.cursor, None, &[stale], "t").unwrap();
        assert_eq!(spent(&pc), 0);
        let id = pc.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions.len();
        assert_eq!(id, 0);
        // Erasing on the PC tells the phone to erase everything it sent.
        let before = pc.cursor().unwrap();
        pc.reset().unwrap();
        let out = pc.sync("Pixel", false, before, None, &[], "t").unwrap();
        assert!(out.changes.iter().all(|c| c.deleted));
        assert!(out.changes.iter().any(|c| c.table == "accounts" && c.uid == "a1"));
    }

    #[test]
    fn a_budget_is_matched_by_its_category_not_its_uid() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let other_uid = change("budgets", "b-from-elsewhere", 400, json!({"category": null, "amount": 500_00}));
        let out = pc.sync("Pixel", false, first.cursor, None, &[other_uid], "t").unwrap();
        let s = pc.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap();
        assert_eq!(s.pace.budget, 500_00);
        // A tombstone for some other budget, its category unsaid, leaves the overall one alone.
        pc.sync("Pixel", false, out.cursor, None, &[tomb("budgets", "b-unknown", 500)], "t").unwrap();
        assert_eq!(pc.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap().pace.budget, 500_00);
    }

    #[test]
    fn a_change_that_refers_to_a_missing_row_is_skipped() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let orphan = change("transactions", "t9", 100, json!({"type": "EXPENSE", "amount": 1_00, "date": "2026-10-05", "account": "nope",
            "toAccount": null, "category": null, "note": "", "recurring": null, "createdAt": 100}));
        let out = pc.sync("Pixel", true, 0, None, &[orphan], "t").unwrap();
        assert_eq!((out.applied, out.skipped), (0, 1));
    }

    #[test]
    fn a_pc_that_a_phone_syncs_with_leaves_bills_to_the_phone() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let mut batch = phone();
        batch.push(change("recurring", "r1", 100, json!({"name": "Rent", "type": "EXPENSE", "amount": 900_00, "account": "a1",
            "toAccount": null, "category": "c1", "frequency": "MONTHLY", "interval": 1, "anchorDate": "2026-10-01",
            "nextDate": "2026-10-01", "endDate": null, "autoPost": true, "active": true})));
        let first = pc.sync("Pixel", true, 0, None, &batch, "t").unwrap();
        // Posting here too would stamp the rent with this PC's time, and that later post would
        // undo an edit or a delete the phone made to its own copy in between.
        assert_eq!(pc.post_due(date(2026, 10, 7)).unwrap(), 0);
        let phones = change("transactions", "bill:r1:2026-10-01", 5_000, json!({"type": "EXPENSE", "amount": 900_00, "date": "2026-10-01",
            "account": "a1", "toAccount": null, "category": "c1", "note": "Rent", "recurring": "r1", "createdAt": 5_000}));
        pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[phones], "t").unwrap();
        assert_eq!(spent(&pc), 42_50 + 900_00);
    }

    #[test]
    fn a_pc_edit_wins_even_when_the_phone_clock_runs_ahead() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let far_future = jiff::Timestamp::now().as_millisecond() + 3 * 60 * 60 * 1000;
        let mut batch = phone();
        batch[0].updated_at = far_future;
        let first = pc.sync("Pixel", true, 0, None, &batch, "t").unwrap();
        let id = pc.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions[0].id;
        pc.tx_update(id, &crate::ledger::TxPatch { amount: Some(50_00), ..Default::default() }).unwrap();
        let out = pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[], "t").unwrap();
        let edit = out.changes.iter().find(|c| c.uid == "t1").unwrap();
        assert!(edit.updated_at > far_future, "a stamp never goes backwards, whatever this clock says");
    }

    #[test]
    fn a_phone_that_synced_with_another_ledger_is_told_to_take_this_one() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let ok = pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[], "t");
        assert!(ok.is_ok());
        // Another phone takes over the ledger: the first one's merge would mix two ledgers.
        let other = pc.sync("Other", true, 0, None, &phone(), "t").unwrap();
        assert_ne!(other.generation, first.generation);
        assert!(matches!(pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[], "t"), Err(LedgerError::Stale(_))));
        // A cursor this ledger never gave (its file was lost and made again) is refused too.
        let mut fresh = Ledger::open_in_memory().unwrap();
        assert!(matches!(fresh.sync("Pixel", false, 5_000, None, &[], "t"), Err(LedgerError::Stale(_))));
        // Taking the ledger whole: since 0, no generation, nothing sent.
        let whole = pc.sync("Pixel", false, 0, None, &[], "t").unwrap();
        assert!(whole.changes.iter().any(|c| c.table == "transactions" && c.uid == "t1"));
        assert_eq!(whole.generation, other.generation);
    }

    #[test]
    fn an_erased_budget_tells_the_phone_which_category_it_was() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let mut batch = phone();
        batch.push(change("budgets", "b2", 100, json!({"category": "c1", "amount": 200_00})));
        let first = pc.sync("Pixel", true, 0, None, &batch, "t").unwrap();
        pc.reset().unwrap();
        let out = pc.sync("Pixel", false, first.cursor, None, &[], "t").unwrap();
        let budgets: Vec<_> = out.changes.iter().filter(|c| c.table == "budgets").collect();
        assert_eq!(budgets.len(), 2);
        assert!(budgets.iter().all(|c| c.deleted && c.row.contains_key("category")));
        assert!(budgets.iter().any(|c| c.row["category"] == "c1"));
        assert!(budgets.iter().any(|c| c.row["category"].is_null()), "the overall budget");
    }

    /// Every page of the answer to one request and the ones after it, as a device asks for them.
    fn pages(l: &mut Ledger, since: i64, generation: Option<&str>, first: &[Change], limit: usize) -> (Vec<Vec<Change>>, SyncOut) {
        let mut out = l.sync_page("Pixel", false, since, generation, first, Some(limit), "t").unwrap();
        let mut all = vec![out.changes.clone()];
        while out.more {
            assert!(out.cursor > since);
            let g = out.generation.clone();
            out = l.sync_page("Pixel", false, out.cursor, Some(&g), &[], Some(limit), "t").unwrap();
            all.push(out.changes.clone());
        }
        (all, out)
    }

    fn keys(changes: &[Change]) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = changes.iter().map(|c| (c.table.clone(), c.uid.clone())).collect();
        v.sort();
        v
    }

    #[test]
    fn a_long_answer_comes_in_pages_and_misses_nothing() {
        let mut l = Ledger::open_in_memory().unwrap();
        l.load_sample(date(2026, 10, 4), "CAD").unwrap();
        let account = l.accounts().unwrap()[0].id;
        l.tx_delete(l.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions[0].id).unwrap();
        let whole = l.sync("Pixel", false, 0, None, &[], "t").unwrap();
        assert!(!whole.more);
        assert!(!serde_json::to_string(&whole).unwrap().contains("more"), "an answer asked for whole reads as it always did");
        let (paged, last) = pages(&mut l, 0, None, &[], 7);
        assert!(paged.len() > 5);
        assert!(paged.iter().all(|p| p.len() <= 7));
        assert_eq!(keys(&paged.concat()), keys(&whole.changes), "every row, deleted ones included, exactly once");
        assert_eq!((last.cursor, &last.generation), (l.cursor().unwrap(), &whole.generation));

        // An edit made between two pages is in a later one.
        let mut out = l.sync_page("Pixel", false, 0, None, &[], Some(3), "t").unwrap();
        let added = l.tx_add(&TxInput { r#type: TxType::Expense, amount: 1_00, date: "2026-10-06".into(), account_id: account,
            to_account_id: None, category_id: None, note: None }).unwrap();
        let mut seen = out.changes.clone();
        while out.more {
            let g = out.generation.clone();
            out = l.sync_page("Pixel", false, out.cursor, Some(&g), &[], Some(3), "t").unwrap();
            seen.extend(out.changes.clone());
        }
        assert!(seen.iter().any(|c| c.uid == added.uid));
        // A page's cursor is one this ledger gave: asking from it is not stale.
        assert!(l.sync_page("Pixel", false, 0, None, &[], Some(1), "t").unwrap().more);
    }

    #[test]
    fn a_paged_answer_does_not_send_back_what_its_request_brought() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let account = pc.accounts().unwrap()[0].id;
        for i in 0..5 {
            pc.tx_add(&TxInput { r#type: TxType::Expense, amount: 1_00 + i, date: "2026-10-06".into(), account_id: account,
                to_account_id: None, category_id: None, note: None }).unwrap();
        }
        let edit = change("transactions", "t1", 900, json!({"type": "EXPENSE", "amount": 50_00, "date": "2026-10-05", "account": "a1",
            "toAccount": null, "category": "c1", "note": "Metro", "recurring": null, "createdAt": 100}));
        let (paged, _) = pages(&mut pc, first.cursor, Some(&first.generation), &[edit], 2);
        let back = paged.concat();
        assert_eq!(paged[0].len(), 2);
        assert_eq!(back.iter().filter(|c| c.table == "transactions" && c.uid != "t1").count(), 5, "the PC's five entries");
        assert!(paged[0].iter().all(|c| c.uid != "t1"), "the page answering the edit leaves it out");
        // A later page, asked for without it, may hand it back as it was sent: no news to the device.
        assert!(back.iter().filter(|c| c.uid == "t1").all(|c| c.updated_at == 900));
    }

    #[test]
    fn a_budget_tombstone_that_did_not_say_its_category_goes_on_without_one() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        // An older Tally deletes a budget this PC never had, and does not say which.
        let out = pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[tomb("budgets", "b-elsewhere", 500)], "t").unwrap();
        let whole = pc.sync("Probe", false, 0, None, &[], "t").unwrap();
        let sent = whole.changes.iter().find(|c| c.uid == "b-elsewhere").unwrap();
        assert!(sent.deleted && sent.row.is_empty(), "not the overall budget's {:?}", sent.row);
        // On to the cloud copy: its overall budget stays.
        let mut cloud = Ledger::open_in_memory().unwrap();
        let cf = cloud.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        cloud.sync("PC", false, cf.cursor, Some(&cf.generation), &whole.changes, "t").unwrap();
        assert_eq!(cloud.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap().pace.budget, 310_00);
        // A tombstone that names its budget still does, the overall one as null.
        pc.sync("Pixel", false, out.cursor, Some(&out.generation), &[Change { row: json!({"category": null}).as_object().unwrap().clone(), ..tomb("budgets", "b1", 600) }], "t").unwrap();
        assert_eq!(pc.summary(date(2026, 10, 7), &Locale::new("en-CA")).unwrap().pace.budget, 0);
    }

    #[test]
    fn a_derived_row_deleted_here_stays_deleted_through_a_replace() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let first = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let line = change("transactions", "imp:9", 200, json!({"type": "EXPENSE", "amount": 5_00, "date": "2026-10-05", "account": "a1",
            "toAccount": null, "category": "c1", "note": "Imported", "recurring": null, "createdAt": 200}));
        let out = pc.sync("Pixel", false, first.cursor, Some(&first.generation), std::slice::from_ref(&line), "t").unwrap();
        // The phone deletes the imported line and a posted bill it never sent; both reach the PC.
        pc.sync("Pixel", false, out.cursor, Some(&out.generation), &[tomb("transactions", "imp:9", 300), tomb("transactions", "bill:r1:2026-10-01", 300)], "t").unwrap();
        // The phone pairs again: its first sync sends its rows, never its tombstones.
        let again = pc.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let buried: Vec<(String, i64)> = pc.conn.prepare("SELECT uid, updated_at FROM tombstones ORDER BY uid").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(buried, [("bill:r1:2026-10-01".to_string(), KEPT_STAMP), ("imp:9".to_string(), KEPT_STAMP)]);
        // A device taking this ledger whole hears of them.
        let whole = pc.sync("Other", false, 0, None, &[], "t").unwrap();
        assert!(whole.changes.iter().any(|c| c.uid == "imp:9" && c.deleted));
        assert!(whole.changes.iter().any(|c| c.uid == "bill:r1:2026-10-01" && c.deleted));
        // A later batch of the same first sync that holds the row still wins over the kept tombstone.
        pc.sync("Pixel", false, again.cursor, Some(&again.generation), &[line], "t").unwrap();
        assert_eq!(spent(&pc), 42_50 + 5_00);
        // A random uid's tombstone goes with the replace, as before.
        pc.sync("Pixel", false, pc.cursor().unwrap(), None, &[tomb("transactions", "t1", 9_000)], "t").unwrap();
        pc.sync("Pixel", true, 0, None, &[], "t").unwrap();
        assert!(!pc.conn.prepare("SELECT 1 FROM tombstones WHERE uid = 't1'").unwrap().exists([]).unwrap());
    }

    #[test]
    fn a_stamp_far_ahead_or_below_zero_is_not_taken_and_the_rest_is() {
        let mut pc = Ledger::open_in_memory().unwrap();
        let mut batch = phone();
        let tx = |uid: &str, at: i64| change("transactions", uid, at, json!({"type": "EXPENSE", "amount": 1_00, "date": "2026-10-05",
            "account": "a1", "toAccount": null, "category": null, "note": "", "recurring": null, "createdAt": 1}));
        let now = crate::ledger::now_ms();
        batch.extend([tx("max", i64::MAX), tx("days", now + 2 * crate::ledger::STAMP_AHEAD_MS), tx("neg", -5), tx("hours", now + 3 * 3_600_000)]);
        let first = pc.sync("Evil", true, 0, None, &batch, "t").unwrap();
        assert_eq!((first.applied, first.skipped), (6, 3), "the phone's five and the entry a few hours fast");
        assert_eq!(spent(&pc), 42_50 + 1_00);
        // Settings stamped 0, as an older ledger's are, still travel.
        let zero = change("settings", "month_start_day", 0, json!({"value": "15"}));
        assert_eq!(pc.sync("Evil", false, first.cursor, Some(&first.generation), &[zero], "t").unwrap().applied, 1);
        // Edits and syncs go on.
        let id = pc.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions[0].id;
        pc.tx_update(id, &crate::ledger::TxPatch { amount: Some(2_00), ..Default::default() }).unwrap();
        assert!(pc.sync("Other", false, 0, None, &[], "t").is_ok());
    }

    #[test]
    fn a_ledger_poisoned_by_a_stamp_at_the_end_of_i64_recovers_when_opened() {
        let dir = std::env::temp_dir().join(format!("relay-money-{}", crate::ledger::new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("money.db");
        {
            let mut l = Ledger::open(&path).unwrap();
            l.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
            // What a build without the bound let in, and an edit then made of it: a real.
            l.conn.execute("UPDATE transactions SET updated_at = 9223372036854775807 WHERE uid = 't1'", []).unwrap();
            l.conn.execute("UPDATE transactions SET updated_at = updated_at + 1 WHERE uid = 't1'", []).unwrap();
            l.conn.execute("UPDATE accounts SET updated_at = 9223372036854775807", []).unwrap();
            l.conn.execute("INSERT INTO tombstones (tbl, uid, updated_at) VALUES ('transactions', 'gone', 9.5e18)", []).unwrap();
            assert!(l.sync("Other", false, 0, None, &[], "t").is_err(), "the poison as it was");
        }
        let mut l = Ledger::open(&path).unwrap();
        let whole = l.sync("Other", false, 0, None, &[], "t").unwrap();
        let ceiling = crate::ledger::now_ms() + crate::ledger::STAMP_AHEAD_MS;
        assert!(whole.changes.iter().all(|c| c.updated_at <= ceiling), "{:?}", whole.changes);
        // And an edit of the row stamps past the repaired stamp, as any edit does.
        let id = l.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions[0].id;
        let before = whole.changes.iter().find(|c| c.uid == "t1").unwrap().updated_at;
        l.tx_update(id, &crate::ledger::TxPatch { amount: Some(1_00), ..Default::default() }).unwrap();
        let after = l.sync("Other", false, whole.cursor, Some(&whole.generation), &[], "t").unwrap();
        assert!(after.changes.iter().find(|c| c.uid == "t1").unwrap().updated_at > before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_stamp_arithmetic_saturates_instead_of_overflowing() {
        let mut l = Ledger::open_in_memory().unwrap();
        l.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        l.conn.execute("UPDATE transactions SET updated_at = 9223372036854775807", []).unwrap();
        let id = l.tx_list(&TxQuery::default(), date(2026, 10, 7)).unwrap().transactions[0].id;
        l.tx_update(id, &crate::ledger::TxPatch { amount: Some(1_00), ..Default::default() }).unwrap();
        let (kind, at): (String, i64) = l.conn.query_row("SELECT typeof(updated_at), updated_at FROM transactions", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((kind.as_str(), at), ("integer", crate::ledger::STAMP_CEILING + 1));
        l.reset().unwrap();
        assert!(l.sync("Other", false, 0, None, &[], "t").is_ok());
    }

    /// A restore of the wrong file and an erase each keep a copy of the ledger they erase, as a
    /// replace does; neither has anything to keep in a new ledger.
    #[test]
    fn a_restore_and_an_erase_keep_a_copy_first() {
        let dir = std::env::temp_dir().join(format!("relay-money-{}", crate::ledger::new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let backups = dir.join("money-backups");
        let mut l = Ledger::open(&dir.join("money.db")).unwrap();
        l.reset().unwrap();
        assert!(!backups.exists(), "nothing to keep");
        l.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        let file = crate::sample_data::build(date(2026, 10, 4), 2, "CAD", crate::sample_data::DEFAULT_SEED);
        l.import_backup(&file).unwrap();
        l.reset().unwrap();
        let mut copies: Vec<PathBuf> = std::fs::read_dir(&backups).unwrap().map(|e| e.unwrap().path()).collect();
        copies.sort();
        assert_eq!(copies.len(), 2);
        assert_eq!(Ledger::open(&copies[0]).unwrap().accounts().unwrap()[0].name, "Chequing", "the ledger the restore erased");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_replace_keeps_a_private_copy_of_the_ledger_it_erases() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("relay-money-{}", crate::ledger::new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("money.db");
        let backups = dir.join("money-backups");
        let mut l = Ledger::open(&path).unwrap();
        // Nothing to keep yet: a new ledger's categories are not the person's.
        l.ensure_categories().unwrap();
        l.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        assert!(!backups.exists());
        // Any credential that may sync can send a replace with nothing in it.
        l.sync("Avex (S21)", true, 0, None, &[], "t").unwrap();
        assert!(l.accounts().unwrap().is_empty());
        let copies: Vec<PathBuf> = std::fs::read_dir(&backups).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(std::fs::metadata(&copies[0]).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&backups).unwrap().permissions().mode() & 0o777, 0o700);
        let kept = Ledger::open(&copies[0]).unwrap();
        assert_eq!(kept.accounts().unwrap()[0].name, "Chequing", "the copy is the ledger as it was");
        drop(kept);
        let before = copies[0].clone();
        // The newest ten stay, and the day's first: a burst of replaces cannot push out the copy of
        // the ledger as it was before them.
        for _ in 0..SNAPSHOTS_KEPT + 3 {
            l.sync("Pixel", true, 0, None, &phone(), "t").unwrap();
        }
        let mut copies: Vec<PathBuf> = std::fs::read_dir(&backups).unwrap().map(|e| e.unwrap().path()).filter(|p| p.to_string_lossy().ends_with(".db")).collect();
        copies.sort();
        assert_eq!(copies.len(), SNAPSHOTS_KEPT + 1);
        assert_eq!(copies[0], before, "the day's first copy is still the ledger before the burst");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_older_ledger_file_gains_sync_with_every_row_numbered() {
        let mut l = Ledger::open_in_memory().unwrap();
        l.load_sample(date(2026, 10, 4), "CAD").unwrap();
        let out = l.sync("Pixel", false, 0, None, &[], "t").unwrap();
        assert!(out.changes.iter().filter(|c| c.table == "transactions").count() > 50);
        assert!(out.changes.iter().any(|c| c.table == "settings" && c.uid == "currency"));
    }
}
