//! The Plan rows relay-money has no calls for yet: bills (`recurring`), goals (`goals`) and their
//! contributions (`contributions`), read and written through the store's second connection
//! (`Store::sql`), with relay-money's conventions (ledger.rs): a random `uid` on insert,
//! `updated_at` in milliseconds and bumped past the stored one (`MAX(now, MIN(updated_at, ceiling)
//! + 1)`) by a write that changes something, never by one that does not (it would outrank a newer
//! edit from another device), deletes as `deleted = 1` so sync carries them, and
//! `seq` never touched (the tables' triggers number every change). Enums are stored by their
//! Kotlin names, dates as `YYYY-MM-DD`.
//!
//! A few reads the Plan pages need over ranges of days (spending by category, money in and out,
//! transfers) live here too, as plain SQL over the same rows relay-money reads.

use relay_money::Date;
use relay_money::model::{GoalKind, TxType};
use relay_money::recurrence::{BillSchedule, Frequency, Recurrence, next_due_on_save};
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::Store;

pub type Result<T> = std::result::Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The stamp arithmetic relay-money uses on every update: never behind the stored stamp.
const BUMP: &str = "updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)";

/// What a writer here says to an amount past relay-money's [`MAX_AMOUNT`]: a bill posts its amount
/// as an entry, which the ledger would refuse to edit.
///
/// [`MAX_AMOUNT`]: relay_money::ledger::MAX_AMOUNT
pub const TOO_LARGE: &str = "That amount is larger than any the ledger keeps";

/// Whether `amount` (either sign) is past what the ledger keeps.
pub fn too_large(amount: i64) -> bool {
    amount.unsigned_abs() > relay_money::ledger::MAX_AMOUNT as u64
}

/// The names the ledger stores its enums by (the Kotlin constant names).
pub fn tx_type_name(t: TxType) -> &'static str {
    match t {
        TxType::Expense => "EXPENSE",
        TxType::Income => "INCOME",
        TxType::Transfer => "TRANSFER",
    }
}

pub fn tx_type_of(s: &str) -> TxType {
    match s {
        "INCOME" => TxType::Income,
        "TRANSFER" => TxType::Transfer,
        _ => TxType::Expense,
    }
}

pub fn frequency_name(f: Frequency) -> &'static str {
    match f {
        Frequency::Weekly => "WEEKLY",
        Frequency::Monthly => "MONTHLY",
        Frequency::Yearly => "YEARLY",
    }
}

pub fn frequency_of(s: &str) -> Frequency {
    match s {
        "WEEKLY" => Frequency::Weekly,
        "YEARLY" => Frequency::Yearly,
        _ => Frequency::Monthly,
    }
}

pub fn goal_kind_name(k: GoalKind) -> &'static str {
    match k {
        GoalKind::Savings => "SAVINGS",
        GoalKind::Balance => "BALANCE",
        GoalKind::Invest => "INVEST",
        GoalKind::Save => "SAVE",
    }
}

pub fn goal_kind_of(s: &str) -> GoalKind {
    match s {
        "BALANCE" => GoalKind::Balance,
        "INVEST" => GoalKind::Invest,
        "SAVE" => GoalKind::Save,
        _ => GoalKind::Savings,
    }
}

fn date(s: Option<String>) -> Option<Date> {
    s.and_then(|s| s.parse().ok())
}

// Bills

/// A bill as the `recurring` table holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bill {
    pub id: i64,
    pub name: String,
    pub kind: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub to_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub frequency: Frequency,
    pub interval: i32,
    pub anchor: Date,
    pub next: Date,
    pub end: Option<Date>,
    pub auto_post: bool,
    pub active: bool,
}

impl Bill {
    pub fn rule(&self) -> Recurrence {
        Recurrence::new(self.anchor, self.frequency, self.interval.clamp(1, 52))
    }
}

/// A bill with the names and colours its row draws.
#[derive(Clone, Debug)]
pub struct BillRow {
    pub bill: Bill,
    pub account: String,
    pub to_account: Option<String>,
    pub icon: Option<String>,
    pub color: Option<i64>,
}

const BILL_COLS: &str = "r.id, r.name, r.type, r.amount, r.account_id, r.to_account_id, r.category_id, r.frequency, r.interval, r.anchor_date, r.next_date, r.end_date, r.auto_post, r.active";

fn bill_row(r: &rusqlite::Row) -> rusqlite::Result<Option<Bill>> {
    let kind: String = r.get(2)?;
    let freq: String = r.get(7)?;
    let (Some(anchor), Some(next)) = (date(r.get(9)?), date(r.get(10)?)) else { return Ok(None) };
    Ok(Some(Bill {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: tx_type_of(&kind),
        amount: r.get(3)?,
        account_id: r.get(4)?,
        to_account_id: r.get(5)?,
        category_id: r.get(6)?,
        frequency: frequency_of(&freq),
        interval: r.get::<_, i64>(8)?.clamp(1, 52) as i32,
        anchor,
        next,
        end: date(r.get(11)?),
        auto_post: r.get(12)?,
        active: r.get(13)?,
    }))
}

/// Every bill, running ones first, then by next date: what the Bills page lists.
pub fn bills(conn: &Connection) -> Result<Vec<BillRow>> {
    let mut st = conn
        .prepare_cached(&format!(
            "SELECT {BILL_COLS}, a.name, ta.name, c.icon, c.color FROM recurring r
             JOIN accounts a ON a.id = r.account_id
             LEFT JOIN accounts ta ON ta.id = r.to_account_id
             LEFT JOIN categories c ON c.id = r.category_id AND c.deleted = 0
             WHERE r.deleted = 0 ORDER BY r.active DESC, r.next_date, r.id"
        ))
        .map_err(err)?;
    let rows = st
        .query_map([], |r| {
            Ok(bill_row(r)?.map(|bill| BillRow { bill, account: r.get(14).unwrap_or_default(), to_account: r.get(15).ok().flatten(), icon: r.get(16).ok().flatten(), color: r.get(17).ok().flatten() }))
        })
        .map_err(err)?;
    let mut out = Vec::new();
    for row in rows {
        if let Some(b) = row.map_err(err)? {
            out.push(b);
        }
    }
    Ok(out)
}

/// One bill, unless it is deleted.
pub fn bill(conn: &Connection, id: i64) -> Result<Option<Bill>> {
    conn.prepare_cached(&format!("SELECT {BILL_COLS} FROM recurring r WHERE r.id = ?1 AND r.deleted = 0"))
        .map_err(err)?
        .query_row([id], bill_row)
        .optional()
        .map(Option::flatten)
        .map_err(err)
}

/// The latest date entered against bill `id`: the entries it posted.
pub fn last_posted(conn: &Connection, id: i64) -> Result<Option<Date>> {
    let d: Option<String> = conn.prepare_cached("SELECT MAX(date) FROM transactions WHERE recurring_id = ?1 AND deleted = 0").map_err(err)?.query_row([id], |r| r.get(0)).map_err(err)?;
    Ok(date(d))
}

/// The latest date posted against every bill that has posted: (bill id, date).
pub fn last_posted_all(conn: &Connection) -> Result<Vec<(i64, Date)>> {
    let mut st = conn
        .prepare_cached("SELECT recurring_id, MAX(date) FROM transactions WHERE recurring_id IS NOT NULL AND deleted = 0 GROUP BY recurring_id")
        .map_err(err)?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(err)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, d) = row.map_err(err)?;
        if let Ok(d) = d.parse() {
            out.push((id, d));
        }
    }
    Ok(out)
}

/// What the bill editor saves.
#[derive(Clone, Debug, PartialEq)]
pub struct BillInput {
    pub name: String,
    pub kind: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub to_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub frequency: Frequency,
    pub interval: i32,
    pub anchor: Date,
    pub auto_post: bool,
    pub active: bool,
}

impl BillInput {
    pub fn rule(&self) -> Recurrence {
        Recurrence::new(self.anchor, self.frequency, self.interval.clamp(1, 52))
    }
}

/// The date a bill is next due once saved, by relay-money's rule over the stored row (the
/// authority on where its schedule stands) and the latest date already posted against it.
pub fn next_due(input: &BillInput, stored: Option<&Bill>, today: Date, last_posted: Option<Date>) -> Date {
    let schedule = stored.map(|b| BillSchedule { rule: b.rule(), next: b.next, auto_post: b.auto_post, active: b.active });
    next_due_on_save(input.rule(), input.auto_post, input.active, schedule, today, last_posted, stored.is_none().then_some(input.anchor))
}

/// Saves a bill: a new one when `id` is none or names a bill since deleted, else an update of
/// that one. Its next date is [`next_due`] read in the same transaction, so an editor left open
/// while bills posted never rewinds a date. Returns the bill's id.
pub fn bill_save(conn: &Connection, id: Option<i64>, input: &BillInput, today: Date) -> Result<i64> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Name the bill so you can find it later".into());
    }
    if input.amount <= 0 {
        return Err("Enter an amount above zero".into());
    }
    if too_large(input.amount) {
        return Err(TOO_LARGE.into());
    }
    let transfer = input.kind == TxType::Transfer;
    if transfer && (input.to_account_id.is_none() || input.to_account_id == Some(input.account_id)) {
        return Err("A transfer needs two accounts".into());
    }
    let to = input.to_account_id.filter(|_| transfer);
    let category = input.category_id.filter(|_| !transfer);
    let tx = conn.unchecked_transaction().map_err(err)?;
    let stored = match id {
        Some(id) => bill(&tx, id)?,
        None => None,
    };
    let posted = match &stored {
        Some(b) => last_posted(&tx, b.id)?,
        None => None,
    };
    let next = next_due(input, stored.as_ref(), today, posted);
    let interval = input.interval.clamp(1, 52);
    let (kind, freq) = (tx_type_name(input.kind), frequency_name(input.frequency));
    let out = match stored {
        Some(b) => {
            tx.prepare_cached(&format!(
                "UPDATE recurring SET name = ?3, type = ?4, amount = ?5, account_id = ?6, to_account_id = ?7, category_id = ?8,
                 frequency = ?9, interval = ?10, anchor_date = ?11, next_date = ?12, auto_post = ?13, active = ?14, {BUMP}
                 WHERE id = ?1 AND (name IS NOT ?3 OR type IS NOT ?4 OR amount IS NOT ?5 OR account_id IS NOT ?6 OR to_account_id IS NOT ?7
                    OR category_id IS NOT ?8 OR frequency IS NOT ?9 OR interval IS NOT ?10 OR anchor_date IS NOT ?11 OR next_date IS NOT ?12
                    OR auto_post IS NOT ?13 OR active IS NOT ?14)"
            ))
            .map_err(err)?
            .execute(params![b.id, Store::stamp().1, name, kind, input.amount, input.account_id, to, category, freq, interval, input.anchor.to_string(), next.to_string(), input.auto_post, input.active])
            .map_err(err)?;
            b.id
        }
        None => {
            let (uid, now) = Store::stamp();
            tx.prepare_cached(
                "INSERT INTO recurring (uid, name, type, amount, account_id, to_account_id, category_id, frequency, interval, anchor_date, next_date, end_date, auto_post, active, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14)",
            )
            .map_err(err)?
            .execute(params![uid, name, kind, input.amount, input.account_id, to, category, freq, interval, input.anchor.to_string(), next.to_string(), input.auto_post, input.active, now])
            .map_err(err)?;
            tx.last_insert_rowid()
        }
    };
    tx.commit().map_err(err)?;
    Ok(out)
}

/// Deletes a bill (entries it posted stay). Returns its name, for the Undo line.
pub fn bill_delete(conn: &Connection, id: i64) -> Result<String> {
    let name: Option<String> = conn.prepare_cached("SELECT name FROM recurring WHERE id = ?1 AND deleted = 0").map_err(err)?.query_row([id], |r| r.get(0)).optional().map_err(err)?;
    let Some(name) = name else { return Err("That bill is already gone".into()) };
    conn.prepare_cached(&format!("UPDATE recurring SET deleted = 1, {BUMP} WHERE id = ?1")).map_err(err)?.execute(params![id, Store::stamp().1]).map_err(err)?;
    Ok(name)
}

/// Brings a deleted bill back as it was: the Undo of [`bill_delete`].
pub fn bill_restore(conn: &Connection, id: i64) -> Result<()> {
    conn.prepare_cached(&format!("UPDATE recurring SET deleted = 0, {BUMP} WHERE id = ?1 AND deleted = 1")).map_err(err)?.execute(params![id, Store::stamp().1]).map_err(err)?;
    Ok(())
}

// Goals

/// A goal as the `goals` table holds it, with what its contributions add up to.
#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub target: i64,
    pub target_date: Option<Date>,
    pub color: i64,
    pub archived: bool,
    pub kind: GoalKind,
    pub account_id: Option<i64>,
    pub percent: i64,
    pub start_date: Option<Date>,
    pub start_amount: i64,
    /// The sum of its contributions.
    pub contributed: i64,
    /// Its first contribution's day.
    pub first: Option<Date>,
}

const GOAL_SELECT: &str = "SELECT g.id, g.name, g.target, g.target_date, g.color, g.archived, g.kind, g.account_id, g.percent, g.start_date, g.start_amount,
        COALESCE((SELECT SUM(amount) FROM contributions c WHERE c.deleted = 0 AND c.goal_id = g.id), 0),
        (SELECT MIN(date) FROM contributions c WHERE c.deleted = 0 AND c.goal_id = g.id)
     FROM goals g";

fn goal_row(r: &rusqlite::Row) -> rusqlite::Result<Goal> {
    let kind: String = r.get(6)?;
    Ok(Goal {
        id: r.get(0)?,
        name: r.get(1)?,
        target: r.get(2)?,
        target_date: date(r.get(3)?),
        color: r.get(4)?,
        archived: r.get(5)?,
        kind: goal_kind_of(&kind),
        account_id: r.get(7)?,
        percent: r.get(8)?,
        start_date: date(r.get(9)?),
        start_amount: r.get(10)?,
        contributed: r.get(11)?,
        first: date(r.get(12)?),
    })
}

/// Every goal not deleted, in relay-money's order: by target date, those without one last.
pub fn goals(conn: &Connection) -> Result<Vec<Goal>> {
    let mut st = conn.prepare_cached(&format!("{GOAL_SELECT} WHERE g.deleted = 0 ORDER BY g.archived, g.target_date IS NULL, g.target_date, g.id")).map_err(err)?;
    let rows = st.query_map([], goal_row).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn goal(conn: &Connection, id: i64) -> Result<Option<Goal>> {
    conn.prepare_cached(&format!("{GOAL_SELECT} WHERE g.id = ?1 AND g.deleted = 0")).map_err(err)?.query_row([id], goal_row).optional().map_err(err)
}

/// What the goal editor saves.
#[derive(Clone, Debug, PartialEq)]
pub struct GoalInput {
    pub name: String,
    pub target: i64,
    pub target_date: Option<Date>,
    pub color: i64,
    pub kind: GoalKind,
    pub account_id: Option<i64>,
    pub percent: i64,
    pub start_date: Option<Date>,
    pub start_amount: i64,
}

/// Saves a goal: new when `id` is none (or names one since deleted), else an update. Its archived
/// flag is kept. Returns the goal's id.
pub fn goal_save(conn: &Connection, id: Option<i64>, g: &GoalInput) -> Result<i64> {
    let name = g.name.trim();
    if name.is_empty() {
        return Err("Name the goal so you can find it later".into());
    }
    if too_large(g.target) || too_large(g.start_amount) {
        return Err(TOO_LARGE.into());
    }
    let existing = match id {
        Some(id) => goal(conn, id)?,
        None => None,
    };
    let (kind, date_s, start_s) = (goal_kind_name(g.kind), g.target_date.map(|d| d.to_string()), g.start_date.map(|d| d.to_string()));
    match existing {
        Some(old) => {
            conn.prepare_cached(&format!(
                "UPDATE goals SET name = ?3, target = ?4, target_date = ?5, color = ?6, kind = ?7, account_id = ?8, percent = ?9,
                 start_date = ?10, start_amount = ?11, {BUMP}
                 WHERE id = ?1 AND (name IS NOT ?3 OR target IS NOT ?4 OR target_date IS NOT ?5 OR color IS NOT ?6 OR kind IS NOT ?7
                    OR account_id IS NOT ?8 OR percent IS NOT ?9 OR start_date IS NOT ?10 OR start_amount IS NOT ?11)"
            ))
            .map_err(err)?
            .execute(params![old.id, Store::stamp().1, name, g.target, date_s, g.color, kind, g.account_id, g.percent, start_s, g.start_amount])
            .map_err(err)?;
            Ok(old.id)
        }
        None => {
            let (uid, now) = Store::stamp();
            conn.prepare_cached(
                "INSERT INTO goals (uid, name, target, target_date, color, archived, kind, account_id, percent, start_date, start_amount, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9, ?10, ?11)",
            )
            .map_err(err)?
            .execute(params![uid, name, g.target, date_s, g.color, kind, g.account_id, g.percent, start_s, g.start_amount, now])
            .map_err(err)?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// A goal deleted, with the contributions deleted with it: what its Undo brings back.
#[derive(Clone, Debug, PartialEq)]
pub struct DeletedGoal {
    pub id: i64,
    pub name: String,
    pub contributions: Vec<i64>,
}

/// Deletes a goal and its contributions, as one change.
pub fn goal_delete(conn: &Connection, id: i64) -> Result<DeletedGoal> {
    let Some(g) = goal(conn, id)? else { return Err("That goal is already gone".into()) };
    let tx = conn.unchecked_transaction().map_err(err)?;
    let ids: Vec<i64> = tx
        .prepare_cached("SELECT id FROM contributions WHERE goal_id = ?1 AND deleted = 0")
        .map_err(err)?
        .query_map([id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let now = Store::stamp().1;
    tx.prepare_cached(&format!("UPDATE contributions SET deleted = 1, {BUMP} WHERE goal_id = ?1 AND deleted = 0")).map_err(err)?.execute(params![id, now]).map_err(err)?;
    tx.prepare_cached(&format!("UPDATE goals SET deleted = 1, {BUMP} WHERE id = ?1")).map_err(err)?.execute(params![id, now]).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(DeletedGoal { id, name: g.name, contributions: ids })
}

/// The Undo of [`goal_delete`]: the goal and exactly the contributions deleted with it.
pub fn goal_restore(conn: &Connection, d: &DeletedGoal) -> Result<()> {
    let tx = conn.unchecked_transaction().map_err(err)?;
    let now = Store::stamp().1;
    tx.prepare_cached(&format!("UPDATE goals SET deleted = 0, {BUMP} WHERE id = ?1 AND deleted = 1")).map_err(err)?.execute(params![d.id, now]).map_err(err)?;
    for c in &d.contributions {
        tx.prepare_cached(&format!("UPDATE contributions SET deleted = 0, {BUMP} WHERE id = ?1 AND deleted = 1")).map_err(err)?.execute(params![c, now]).map_err(err)?;
    }
    tx.commit().map_err(err)
}

/// Money put into a pot (positive) or taken out (negative).
#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    pub id: i64,
    pub amount: i64,
    pub date: Date,
    pub note: String,
}

/// A goal's contributions, newest first.
pub fn contributions(conn: &Connection, goal_id: i64) -> Result<Vec<Contribution>> {
    let mut st = conn.prepare_cached("SELECT id, amount, date, note FROM contributions WHERE goal_id = ?1 AND deleted = 0 ORDER BY date DESC, id DESC").map_err(err)?;
    let rows = st.query_map([goal_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))).map_err(err)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, amount, d, note) = row.map_err(err)?;
        if let Ok(date) = d.parse() {
            out.push(Contribution { id, amount, date, note });
        }
    }
    Ok(out)
}

/// Puts money into a goal (or takes it out, negative) on `date`. Returns the row's id.
pub fn contribute(conn: &Connection, goal_id: i64, amount: i64, date: Date, note: &str) -> Result<i64> {
    if amount == 0 {
        return Err("Enter an amount above zero".into());
    }
    if too_large(amount) {
        return Err(TOO_LARGE.into());
    }
    if goal(conn, goal_id)?.is_none() {
        return Err("That goal is gone".into());
    }
    let (uid, now) = Store::stamp();
    conn.prepare_cached("INSERT INTO contributions (uid, goal_id, amount, date, note, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .map_err(err)?
        .execute(params![uid, goal_id, amount, date.to_string(), note.trim(), now])
        .map_err(err)?;
    Ok(conn.last_insert_rowid())
}

/// Deletes one contribution. Returns its amount, for the Undo line.
pub fn contribution_delete(conn: &Connection, id: i64) -> Result<i64> {
    let amount: Option<i64> = conn.prepare_cached("SELECT amount FROM contributions WHERE id = ?1 AND deleted = 0").map_err(err)?.query_row([id], |r| r.get(0)).optional().map_err(err)?;
    let Some(amount) = amount else { return Err("That row is already gone".into()) };
    conn.prepare_cached(&format!("UPDATE contributions SET deleted = 1, {BUMP} WHERE id = ?1")).map_err(err)?.execute(params![id, Store::stamp().1]).map_err(err)?;
    Ok(amount)
}

pub fn contribution_restore(conn: &Connection, id: i64) -> Result<()> {
    conn.prepare_cached(&format!("UPDATE contributions SET deleted = 0, {BUMP} WHERE id = ?1 AND deleted = 1")).map_err(err)?.execute(params![id, Store::stamp().1]).map_err(err)?;
    Ok(())
}

// Reads over a range of days (`from` inclusive, `to` exclusive)

/// Spending by category: (category id, total, entries), biggest first. Uncategorised left out.
pub fn spent_by_category(conn: &Connection, from: Date, to: Date) -> Result<Vec<(i64, i64, i64)>> {
    let mut st = conn
        .prepare_cached(
            "SELECT category_id, SUM(amount), COUNT(*) FROM transactions
             WHERE deleted = 0 AND type = 'EXPENSE' AND category_id IS NOT NULL AND date >= ?1 AND date < ?2
             GROUP BY category_id ORDER BY SUM(amount) DESC",
        )
        .map_err(err)?;
    let rows = st.query_map([from.to_string(), to.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// (money in, money out) over the days; transfers are neither.
pub fn flows(conn: &Connection, from: Date, to: Date) -> Result<(i64, i64)> {
    conn.prepare_cached(
        "SELECT COALESCE(SUM(CASE WHEN type = 'INCOME' THEN amount END), 0), COALESCE(SUM(CASE WHEN type = 'EXPENSE' THEN amount END), 0)
         FROM transactions WHERE deleted = 0 AND date >= ?1 AND date < ?2",
    )
    .map_err(err)?
    .query_row([from.to_string(), to.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))
    .map_err(err)
}

/// Transfers over the days: (date, from, to, amount).
pub fn transfers(conn: &Connection, from: Date, to: Date) -> Result<Vec<(Date, i64, i64, i64)>> {
    let mut st = conn
        .prepare_cached(
            "SELECT date, account_id, to_account_id, amount FROM transactions
             WHERE deleted = 0 AND type = 'TRANSFER' AND to_account_id IS NOT NULL AND date >= ?1 AND date < ?2",
        )
        .map_err(err)?;
    let rows = st.query_map([from.to_string(), to.to_string()], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(err)?;
    let mut out = Vec::new();
    for row in rows {
        let (d, a, b, amount) = row.map_err(err)?;
        if let Ok(d) = d.parse() {
            out.push((d, a, b, amount));
        }
    }
    Ok(out)
}

/// What went into the goals over the days, less what was taken out: contributions to goals not
/// deleted.
pub fn contributed(conn: &Connection, from: Date, to: Date) -> Result<i64> {
    conn.prepare_cached(
        "SELECT COALESCE(SUM(c.amount), 0) FROM contributions c JOIN goals g ON g.id = c.goal_id
         WHERE c.deleted = 0 AND g.deleted = 0 AND c.date >= ?1 AND c.date < ?2",
    )
    .map_err(err)?
    .query_row([from.to_string(), to.to_string()], |r| r.get(0))
    .map_err(err)
}
