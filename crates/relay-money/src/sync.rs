//! Two-way sync between a device's ledger and this one (docs/MONEY.md, "Sync").
//!
//! Each side holds the whole ledger. A device sends the rows it changed since its last sync and
//! the cursor this ledger gave it then; this ledger applies them, newest `updated_at` winning per
//! row, and answers with every row it numbered above that cursor. References travel as uids, so
//! local ids never leave the device. A device's first sync replaces this ledger outright: the
//! phone is where the ledger lives.

use crate::ledger::{Ledger, LedgerError, Result};
use crate::views::{Change, SyncOut};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{Map, Value};
use std::collections::HashSet;

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
    /// With `replace`, this ledger is erased first and becomes the device's (its first sync).
    ///
    /// `generation` is the one this ledger gave the device last time. A device whose ledger is not
    /// this one any more (restored, erased or replaced since, or a cursor this ledger never gave)
    /// is refused with [`LedgerError::Stale`]: it then takes this ledger whole, asking with
    /// `since: 0`, no generation and no changes, rather than merging into a ledger it no longer
    /// shares rows with.
    pub fn sync(&mut self, device: &str, replace: bool, since: i64, generation: Option<&str>, changes: &[Change], now: &str) -> Result<SyncOut> {
        if !replace {
            let mine = self.generation()?;
            if generation.is_some_and(|g| g != mine) || since > self.cursor()? {
                return Err(LedgerError::Stale(
                    "This PC's ledger changed since this phone last synced: restored, erased or replaced. Take the PC's ledger, or send this phone's again.".into(),
                ));
            }
        }
        let tx = self.conn.transaction()?;
        if replace {
            Ledger::clear(&tx, true)?;
        }
        let mut sorted: Vec<&Change> = changes.iter().collect();
        sorted.sort_by_key(|c| order(&c.table));
        let mut applied = HashSet::new();
        let mut skipped = 0;
        for c in sorted {
            if apply(&tx, c)? {
                applied.insert((c.table.clone(), c.uid.clone()));
            } else {
                skipped += 1;
            }
        }
        let changes = if replace { Vec::new() } else { changed_since(&tx, since, &applied)? };
        tx.prepare_cached(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, 0) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )?.execute(params![format!("sync.device.{device}"), now])?;
        tx.commit()?;
        Ok(SyncOut { cursor: self.cursor()?, generation: self.generation()?, changes, replaced: replace, applied: applied.len(), skipped })
    }

    /// Devices that have synced with this ledger and when they last did, newest first.
    pub fn synced_devices(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare_cached("SELECT substr(key, 13), value FROM settings WHERE key LIKE 'sync.device.%' ORDER BY value DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
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
                let category = c.row.get("category").and_then(Value::as_str);
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

/// Every row and tombstone numbered above `since`, but the ones this sync just applied: the
/// device already has those.
fn changed_since(tx: &Transaction, since: i64, applied: &HashSet<(String, String)>) -> Result<Vec<Change>> {
    let mut out = Vec::new();
    let mut st = tx.prepare_cached("SELECT key, value, updated_at FROM settings WHERE seq > ?1")?;
    let rows: Vec<(String, String, i64)> = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<_, _>>()?;
    for (key, value, updated_at) in rows {
        if SYNCED_SETTINGS.contains(&key.as_str()) && !applied.contains(&("settings".to_string(), key.clone())) {
            let mut row = Map::new();
            row.insert("value".into(), Value::String(value));
            out.push(Change { table: "settings".into(), uid: key, updated_at, deleted: false, row });
        }
    }
    for (table, cols) in TABLES {
        let names: Vec<&str> = cols.iter().map(|c| c.sql).collect();
        let sql = format!("SELECT uid, updated_at, deleted, {} FROM {table} WHERE seq > ?1 ORDER BY seq", names.join(", "));
        let rows: Vec<(String, i64, bool, Vec<rusqlite::types::Value>)> = {
            let mut st = tx.prepare_cached(&sql)?;
            let mapped = st.query_map([since], |r| {
                let mut vals = Vec::with_capacity(cols.len());
                for i in 0..cols.len() {
                    vals.push(r.get::<_, rusqlite::types::Value>(i + 3)?);
                }
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, vals))
            })?;
            mapped.collect::<std::result::Result<_, _>>()?
        };
        for (uid, updated_at, deleted, vals) in rows {
            if applied.contains(&(table.to_string(), uid.clone())) {
                continue;
            }
            let mut row = Map::new();
            for (c, v) in cols.iter().zip(vals) {
                // The overall budget's category 0 matches no row, so it travels as null.
                row.insert(c.json.into(), to_json(tx, c, v)?);
            }
            out.push(Change { table: table.to_string(), uid, updated_at, deleted, row });
        }
    }
    let mut st = tx.prepare_cached("SELECT tbl, uid, updated_at, category FROM tombstones WHERE seq > ?1 ORDER BY seq")?;
    let rows: Vec<(String, String, i64, Option<String>)> =
        st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
    for (table, uid, updated_at, category) in rows {
        if !applied.contains(&(table.clone(), uid.clone())) {
            let mut row = Map::new();
            // A budget's tombstone says which budget: null is the overall one.
            if table == "budgets" {
                row.insert("category".into(), category.map_or(Value::Null, Value::String));
            }
            out.push(Change { table, uid, updated_at, deleted: true, row });
        }
    }
    Ok(out)
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

    #[test]
    fn an_older_ledger_file_gains_sync_with_every_row_numbered() {
        let mut l = Ledger::open_in_memory().unwrap();
        l.load_sample(date(2026, 10, 4), "CAD").unwrap();
        let out = l.sync("Pixel", false, 0, None, &[], "t").unwrap();
        assert!(out.changes.iter().filter(|c| c.table == "transactions").count() > 50);
        assert!(out.changes.iter().any(|c| c.table == "settings" && c.uid == "currency"));
    }
}
