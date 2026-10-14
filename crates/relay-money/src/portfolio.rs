//! Investments in the PC's ledger (docs/INVESTMENTS.md): the portfolio reading, activities, room,
//! prices, rates, recorded values, and the Wealthsimple import.
//!
//! The rules are [`crate::invest`]'s and [`crate::wealthsimple`]'s, the phone's twins; this file
//! only reads and writes rows. Rows an import or a write derives a uid for (`sec:`, `hold:`,
//! `imp:`, `px:`, `fx:`, `room:`, `val:`, `ws:`) are the same rows on both devices, so the same
//! file imported on the phone and here merges on sync.

use crate::invest::{
    self, AccountRow, ActivityRow, FxRow, HoldingRow, Portfolio, PortfolioInput, PriceRow, RoomRow, SecurityRow, TransferRow,
    ROOM_REGISTRATIONS, SOURCE_BANK_OF_CANADA, SOURCE_IMPORT, SOURCE_MANUAL, SOURCE_WEALTHSIMPLE,
};
use crate::ledger::{invalid, name_of, new_uid, now_ms, parse_date, parse_name, Ledger, LedgerError, Result};
use crate::model::{AccountType, ActivityType, Registration, SecurityKind};
use crate::money::fraction_digits;
use crate::views::{
    AccountPatch, AccountView, ActivityInput, ActivityQuery, ActivityView, ImportAccount, ImportPreview, ImportPreviewAccount, ImportResult,
};
use crate::wealthsimple::{self, ImportPlan, PlanInput, WsFile, WsKind, INSTITUTION};
use jiff::civil::Date;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

/// What the PC says for a Wealthsimple cash statement ([`wealthsimple::NOT_INVESTMENT`]): where it goes.
const CASH_STATEMENT: &str = "This is a cash account's statement (Wealthsimple Cash or chequing), not an investment account's. \
Import it with your bank statements in Tally on your phone.";

/// Every row `sql` (no parameters) yields that `f` reads; a row `f` cannot read is left out.
fn rows<T>(conn: &Connection, sql: &str, mut f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<Option<T>>) -> Result<Vec<T>> {
    let mut st = conn.prepare_cached(sql)?;
    let mut out = Vec::new();
    for row in st.query_map([], &mut f)? {
        out.extend(row?);
    }
    Ok(out)
}

fn date_of(r: &rusqlite::Row, i: usize) -> rusqlite::Result<Option<Date>> {
    Ok(parse_date(&r.get::<_, String>(i)?))
}

fn id_of(conn: &Connection, table: &str, uid: &str) -> Result<Option<i64>> {
    Ok(conn.prepare_cached(&format!("SELECT id FROM {table} WHERE uid = ?1"))?.query_row([uid], |r| r.get(0)).optional()?)
}

/// Whether a row with `uid` is in `table` (deleted or not) or was erased: an import never adds it again.
fn known(conn: &Connection, table: &str, uid: &str) -> Result<bool> {
    let here = conn.prepare_cached(&format!("SELECT 1 FROM {table} WHERE uid = ?1"))?.exists([uid])?;
    Ok(here || conn.prepare_cached("SELECT 1 FROM tombstones WHERE tbl = ?1 AND uid = ?2")?.exists(params![table, uid])?)
}

/// The stamp for a row written under a derived uid: past the tombstone that buried it, as Tally's
/// insert trigger stamps it (`MAX(now, deletedAt + 1)`), or a device holding that tombstone would
/// refuse the row as older than the delete.
fn stamp_over(conn: &Connection, table: &str, uid: &str, now: i64) -> Result<i64> {
    let buried: Option<i64> = conn.prepare_cached("SELECT updated_at FROM tombstones WHERE tbl = ?1 AND uid = ?2")?
        .query_row(params![table, uid], |r| r.get(0)).optional()?;
    Ok(buried.map_or(now, |t| now.max(t.saturating_add(1))))
}

/// A row written again under a derived uid stays: the erasure that buried it is over.
fn unbury(conn: &Connection, table: &str, uid: &str) -> Result<()> {
    conn.prepare_cached("DELETE FROM tombstones WHERE tbl = ?1 AND uid = ?2")?.execute(params![table, uid])?;
    Ok(())
}

fn currency_code(text: &str) -> Result<String> {
    let code = text.trim().to_uppercase();
    if fraction_digits(&code).is_none() {
        return invalid(format!("Not a currency code: {}", text.trim()));
    }
    Ok(code)
}

fn day(text: &str) -> Result<Date> {
    parse_date(text).ok_or_else(|| LedgerError::Invalid(format!("Not a date: {text}")))
}

/// The security a symbol names: the row with its uid, else a live one with that symbol.
fn security_of(conn: &Connection, symbol: &str) -> Result<Option<(i64, String)>> {
    let symbol = symbol.trim().to_uppercase();
    let by_uid = conn.prepare_cached("SELECT id, uid FROM securities WHERE uid = ?1")?
        .query_row([wealthsimple::security_uid(&symbol)], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    if by_uid.is_some() {
        return Ok(by_uid);
    }
    Ok(conn.prepare_cached("SELECT id, uid FROM securities WHERE deleted = 0 AND symbol = ?1 ORDER BY id LIMIT 1")?
        .query_row([symbol], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
}

const ACTIVITY_SELECT: &str = "SELECT x.id, x.uid, x.account_id, a.name, x.security_id, s.symbol, s.name, x.type, x.date, x.quantity,
            x.amount, x.fee, x.currency, x.to_amount, x.to_currency, x.note, x.source
     FROM activities x
     LEFT JOIN accounts a ON a.id = x.account_id
     LEFT JOIN securities s ON s.id = x.security_id";

fn activity_row(r: &rusqlite::Row) -> rusqlite::Result<ActivityView> {
    Ok(ActivityView {
        id: r.get(0)?,
        uid: r.get(1)?,
        account_id: r.get(2)?,
        account: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        security_id: r.get(4)?,
        symbol: r.get(5)?,
        name: r.get(6)?,
        r#type: parse_name(&r.get::<_, String>(7)?).unwrap_or(ActivityType::Deposit),
        date: r.get(8)?,
        quantity: r.get(9)?,
        amount: r.get(10)?,
        fee: r.get(11)?,
        currency: r.get(12)?,
        to_amount: r.get(13)?,
        to_currency: r.get(14)?,
        note: r.get(15)?,
        source: r.get(16)?,
    })
}

impl Ledger {
    /// The investments on `today` (docs/INVESTMENTS.md, "The portfolio reading"). A day whose
    /// year the room's deadlines or the income months would run off the calendar from is refused.
    pub fn portfolio(&self, today: Date) -> Result<Portfolio> {
        if !(1..=9998).contains(&today.year()) {
            return invalid(format!("Not a day to read investments on: {today}"));
        }
        Ok(invest::portfolio(&self.portfolio_input(today)?))
    }

    /// The exchange rates the ledger knows: the reading's and an import's, so a holding in US
    /// dollars is valued the same here as on the phone.
    fn fx_rows(&self) -> Result<Vec<FxRow>> {
        rows(&self.conn, "SELECT base, quote, date, rate FROM fx_rates WHERE deleted = 0", |r| {
            let Some(date) = date_of(r, 2)? else { return Ok(None) };
            Ok(Some(FxRow { base: r.get(0)?, quote: r.get(1)?, date, rate: r.get(3)? }))
        })
    }

    /// The rows the reading takes: the investment accounts in use and everything touching them.
    pub fn portfolio_input(&self, today: Date) -> Result<PortfolioInput> {
        let c = &self.conn;
        let accounts = rows(
            c,
            "SELECT id, uid, name, registration, institution FROM accounts
             WHERE deleted = 0 AND archived = 0 AND type = 'INVESTMENT' ORDER BY sort_order, id",
            |r| {
                Ok(Some(AccountRow {
                    id: r.get(0)?,
                    uid: r.get(1)?,
                    name: r.get(2)?,
                    registration: r.get::<_, Option<String>>(3)?.as_deref().and_then(parse_name),
                    institution: r.get(4)?,
                }))
            },
        )?;
        let securities = rows(c, "SELECT id, uid, symbol, name, currency, kind, exchange FROM securities WHERE deleted = 0", |r| {
            Ok(Some(SecurityRow {
                id: r.get(0)?,
                uid: r.get(1)?,
                symbol: r.get(2)?,
                name: r.get(3)?,
                currency: r.get(4)?,
                kind: parse_name(&r.get::<_, String>(5)?).unwrap_or(SecurityKind::Other),
                exchange: r.get(6)?,
            }))
        })?;
        let holdings = rows(c, "SELECT account_id, security_id, date, quantity, book, book_market FROM holdings WHERE deleted = 0", |r| {
            let Some(date) = date_of(r, 2)? else { return Ok(None) };
            Ok(Some(HoldingRow { account_id: r.get(0)?, security_id: r.get(1)?, date, quantity: r.get(3)?, book: r.get(4)?, book_market: r.get(5)? }))
        })?;
        let activities = rows(
            c,
            "SELECT id, uid, account_id, security_id, type, date, quantity, amount, fee, currency, to_amount, to_currency
             FROM activities WHERE deleted = 0",
            |r| {
                let (Some(date), Some(ty)) = (date_of(r, 5)?, parse_name::<ActivityType>(&r.get::<_, String>(4)?)) else { return Ok(None) };
                Ok(Some(ActivityRow {
                    id: r.get(0)?,
                    uid: r.get(1)?,
                    account_id: r.get(2)?,
                    security_id: r.get(3)?,
                    r#type: ty,
                    date,
                    quantity: r.get(6)?,
                    amount: r.get(7)?,
                    fee: r.get(8)?,
                    currency: r.get(9)?,
                    to_amount: r.get(10)?,
                    to_currency: r.get(11)?,
                }))
            },
        )?;
        let prices = rows(c, "SELECT security_id, date, price FROM prices WHERE deleted = 0", |r| {
            let Some(date) = date_of(r, 1)? else { return Ok(None) };
            Ok(Some(PriceRow { security_id: r.get(0)?, date, price: r.get(2)? }))
        })?;
        let fx_rates = self.fx_rows()?;
        let room_facts = rows(c, "SELECT registration, year, amount FROM room_facts WHERE deleted = 0", |r| {
            let Some(registration) = parse_name::<Registration>(&r.get::<_, String>(0)?) else { return Ok(None) };
            Ok(Some(RoomRow { registration, year: r.get(1)?, amount: r.get(2)? }))
        })?;
        // Tally's own transfers into and out of investment accounts, for room where no activity says it.
        let transfers = rows(
            c,
            "SELECT t.to_account_id, t.date, t.amount FROM transactions t JOIN accounts a ON a.id = t.to_account_id
             WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND a.type = 'INVESTMENT'
             UNION ALL
             SELECT t.account_id, t.date, -t.amount FROM transactions t JOIN accounts a ON a.id = t.account_id
             WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND a.type = 'INVESTMENT'",
            |r| {
                let Some(date) = date_of(r, 1)? else { return Ok(None) };
                Ok(Some(TransferRow { account_id: r.get(0)?, date, amount: r.get(2)? }))
            },
        )?;
        Ok(PortfolioInput {
            currency: self.settings()?.currency,
            today,
            accounts,
            securities,
            holdings,
            activities,
            prices,
            fx_rates,
            room_facts,
            transfers,
        })
    }

    pub fn activity(&self, id: i64) -> Result<ActivityView> {
        self.conn.prepare_cached(&format!("{ACTIVITY_SELECT} WHERE x.id = ?1 AND x.deleted = 0"))?
            .query_row([id], activity_row)
            .optional()?
            .ok_or_else(|| LedgerError::NotFound(format!("No activity {id}")))
    }

    /// Activities, newest first, narrowed by account, type and a first day.
    pub fn invest_list(&self, q: &ActivityQuery) -> Result<Vec<ActivityView>> {
        if let Some(since) = q.since.as_deref() {
            day(since)?;
        }
        let mut st = self.conn.prepare_cached(&format!(
            "{ACTIVITY_SELECT}
             WHERE x.deleted = 0 AND (?1 IS NULL OR x.account_id = ?1) AND (?2 IS NULL OR x.type = ?2) AND (?3 IS NULL OR x.date >= ?3)
             ORDER BY x.date DESC, x.id DESC LIMIT ?4"
        ))?;
        let rows = st.query_map(params![q.account_id, q.r#type.as_ref().map(name_of), q.since, q.limit.unwrap_or(200)], activity_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Records an activity by hand. A symbol finds its security, or makes `sec:<SYMBOL>` in the
    /// activity's currency; the currency is the ledger's when none is given.
    pub fn invest_add(&mut self, input: &ActivityInput) -> Result<ActivityView> {
        use ActivityType::*;
        let ty = input.r#type;
        let investing = self.conn.prepare_cached("SELECT 1 FROM accounts WHERE id = ?1 AND deleted = 0 AND type = 'INVESTMENT'")?.exists([input.account_id])?;
        if !investing {
            return invalid("Pick an investment account");
        }
        day(&input.date)?;
        let (quantity, fee) = (input.quantity.unwrap_or(0), input.fee.unwrap_or(0));
        if input.amount < 0 || quantity < 0 || fee < 0 || input.to_amount.is_some_and(|a| a < 0) {
            return invalid("Amounts, units and fees are never negative: the type says which way they go");
        }
        if input.amount == 0 && !matches!(ty, Split | TransferIn | TransferOut) {
            return invalid("An activity needs an amount above zero");
        }
        let currency = match input.currency.as_deref().filter(|c| !c.trim().is_empty()) {
            Some(c) => currency_code(c)?,
            None => self.settings()?.currency,
        };
        let symbol = input.symbol.as_deref().map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty());
        if symbol.is_none() && matches!(ty, Buy | Sell | Reinvest | Split | NotionalDistribution | ReturnOfCapital) {
            return invalid("Name the security: its symbol, such as XEQT");
        }
        if quantity == 0 && matches!(ty, Buy | Sell | Reinvest | Split) {
            return invalid("Say how many units");
        }
        let (to_amount, to_currency) = if ty == Fx {
            let to = match input.to_currency.as_deref() {
                Some(c) => currency_code(c)?,
                None => return invalid("An exchange needs the currency the money became"),
            };
            if to == currency || input.to_amount.unwrap_or(0) <= 0 {
                return invalid("An exchange needs the amount it became, in another currency");
            }
            (input.to_amount, Some(to))
        } else {
            (None, None)
        };
        let now = now_ms();
        let tx = self.conn.transaction()?;
        let security = match symbol {
            None => None,
            Some(symbol) => Some(match security_of(&tx, &symbol)? {
                Some((id, uid)) => {
                    tx.prepare_cached("UPDATE securities SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1")?
                        .execute(params![id, now])?;
                    unbury(&tx, "securities", &uid)?;
                    id
                }
                None => {
                    let uid = wealthsimple::security_uid(&symbol);
                    let at = stamp_over(&tx, "securities", &uid, now)?;
                    tx.prepare_cached("INSERT INTO securities (uid, symbol, currency, kind, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)")?
                        .execute(params![uid, symbol, currency, name_of(&SecurityKind::Other), at])?;
                    unbury(&tx, "securities", &uid)?;
                    tx.last_insert_rowid()
                }
            }),
        };
        tx.prepare_cached(
            "INSERT INTO activities (uid, account_id, security_id, type, date, quantity, amount, fee, currency, to_amount, to_currency, note, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
        )?.execute(params![
            new_uid(), input.account_id, security, name_of(&ty), input.date, quantity, input.amount, fee, currency, to_amount, to_currency,
            input.note.as_deref().unwrap_or("").trim(), SOURCE_MANUAL, now
        ])?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        self.activity(id)
    }

    /// Deletes are tombstones, so sync carries them, and an imported line deleted here is never
    /// imported again.
    pub fn invest_delete(&mut self, id: i64) -> Result<()> {
        let n = self.conn.prepare_cached("UPDATE activities SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 0")?
            .execute(params![id, now_ms()])?;
        if n == 0 {
            return Err(LedgerError::NotFound(format!("No activity {id}")));
        }
        Ok(())
    }

    /// Brings back a deleted activity: the Undo of [`Ledger::invest_delete`].
    pub fn invest_restore(&mut self, id: i64) -> Result<ActivityView> {
        let n = self.conn.prepare_cached("UPDATE activities SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1")?
            .execute(params![id, now_ms()])?;
        if n == 0 {
            return Err(LedgerError::NotFound(format!("No deleted activity {id}")));
        }
        self.activity(id)
    }

    /// A live investment account's uid, or a refusal.
    fn investment_uid(&self, id: i64) -> Result<String> {
        self.conn.prepare_cached("SELECT uid FROM accounts WHERE id = ?1 AND deleted = 0 AND type = 'INVESTMENT'")?
            .query_row([id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| LedgerError::Invalid("Pick an investment account".into()))
    }

    /// The live account already holding Wealthsimple's `number`.
    fn account_by_ref(&self, number: &str) -> Result<Option<(i64, String)>> {
        Ok(self.conn.prepare_cached("SELECT id, uid FROM accounts WHERE deleted = 0 AND external_ref = ?1 AND external_ref != '' ORDER BY id LIMIT 1")?
            .query_row([number], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    /// A Wealthsimple file, read. A cash statement is told where it goes instead.
    fn read_file(text: &str) -> Result<WsFile> {
        wealthsimple::read(text).map_err(|e| LedgerError::Invalid(if e == wealthsimple::NOT_INVESTMENT { CASH_STATEMENT.into() } else { e }))
    }

    /// What importing `text` would do: its accounts (each with the Tally account already holding
    /// its number), and how many of its lines are new or already in. A statement names no account:
    /// with no `account_id` its lines all count as new.
    pub fn invest_preview(&self, text: &str, account_id: Option<i64>) -> Result<ImportPreview> {
        let file = Ledger::read_file(text)?;
        let mut input = PlanInput::new(self.settings()?.currency);
        input.fx_rates = self.fx_rows()?;
        let mut accounts = Vec::new();
        for a in &file.accounts {
            let found = self.account_by_ref(&a.number)?;
            if let Some((_, uid)) = &found {
                input.accounts.insert(a.number.clone(), uid.clone());
            }
            let rows = file.holdings.iter().filter(|h| h.account == a.number).count()
                + file.activities.iter().filter(|x| x.account.as_deref() == Some(a.number.as_str())).count();
            accounts.push(ImportPreviewAccount {
                number: a.number.clone(),
                name: a.name.clone(),
                registration: a.registration,
                account_id: found.map(|(id, _)| id),
                rows,
            });
        }
        if file.kind == WsKind::Statement {
            input.statement_account = Some(match account_id {
                Some(id) => self.investment_uid(id)?,
                None => String::new(),
            });
        }
        let plan = wealthsimple::plan(&file, &input).map_err(LedgerError::Invalid)?;
        let mut duplicates = 0;
        for uid in plan.holdings.iter().map(|h| &h.uid) {
            duplicates += usize::from(known(&self.conn, "holdings", uid)?);
        }
        for uid in plan.activities.iter().map(|a| &a.uid) {
            duplicates += usize::from(known(&self.conn, "activities", uid)?);
        }
        Ok(ImportPreview {
            kind: file.kind,
            as_of: file.as_of.map(|d| d.to_string()),
            accounts,
            holdings: plan.holdings.len(),
            activities: plan.activities.len(),
            new: plan.holdings.len() + plan.activities.len() - duplicates,
            duplicates,
            skipped: plan.skipped,
        })
    }

    /// Imports a Wealthsimple file. Each of its accounts goes where `accounts` says: an entry with
    /// an `account_id` into that account (which then remembers the number), an entry without one
    /// into a new account; an account not listed into the account already holding its number, else
    /// a new one. A statement goes into `account_id`. A line is added only when its uid is neither
    /// here nor deleted, so importing a file twice adds it once.
    pub fn invest_import(&mut self, text: &str, accounts: &[ImportAccount], account_id: Option<i64>) -> Result<ImportResult> {
        let file = Ledger::read_file(text)?;
        let mut input = PlanInput::new(self.settings()?.currency);
        input.fx_rates = self.fx_rows()?;
        let mut remember: Vec<(i64, String, Registration)> = Vec::new();
        for a in &file.accounts {
            match accounts.iter().find(|m| m.number == a.number) {
                Some(ImportAccount { account_id: Some(id), .. }) => {
                    input.accounts.insert(a.number.clone(), self.investment_uid(*id)?);
                    remember.push((*id, a.number.clone(), a.registration));
                }
                Some(ImportAccount { account_id: None, .. }) => {}
                None => {
                    if let Some((_, uid)) = self.account_by_ref(&a.number)? {
                        input.accounts.insert(a.number.clone(), uid);
                    }
                }
            }
        }
        if file.kind == WsKind::Statement {
            match account_id {
                Some(id) => input.statement_account = Some(self.investment_uid(id)?),
                None => return invalid("Choose the account this statement belongs to"),
            }
        }
        let plan = wealthsimple::plan(&file, &input).map_err(LedgerError::Invalid)?;
        let now = now_ms();
        let tx = self.conn.transaction()?;
        // An account mapped by hand keeps the number, so the next import finds it.
        for (id, number, registration) in remember {
            tx.prepare_cached(
                "UPDATE accounts SET external_ref = ?2, institution = CASE WHEN institution = '' THEN ?3 ELSE institution END,
                        registration = COALESCE(registration, ?4), updated_at = MAX(?5, MIN(updated_at, 4000000000000000) + 1)
                 WHERE id = ?1 AND (external_ref != ?2 OR institution = '' OR registration IS NULL)",
            )?.execute(params![id, number, INSTITUTION, name_of(&registration), now])?;
        }
        let result = Ledger::write_plan(&tx, &plan, file.kind, now)?;
        tx.commit()?;
        Ok(result)
    }

    fn write_plan(tx: &Connection, plan: &ImportPlan, kind: WsKind, now: i64) -> Result<ImportResult> {
        let mut result = ImportResult {
            kind,
            accounts_created: 0,
            securities: 0,
            holdings: 0,
            activities: 0,
            duplicates: 0,
            prices: 0,
            values: 0,
            skipped: plan.skipped.len(),
        };
        for a in &plan.accounts {
            let order: i64 = tx.prepare_cached("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM accounts")?.query_row([], |r| r.get(0))?;
            // "A new account" for a number an earlier import created is that account again, not a new one.
            let created = id_of(tx, "accounts", &a.uid)?.is_none();
            tx.prepare_cached(
                "INSERT INTO accounts (uid, name, type, opening_balance, sort_order, registration, institution, external_ref, updated_at)
                 VALUES (?1, ?2, 'INVESTMENT', 0, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(uid) DO UPDATE SET deleted = 0, type = 'INVESTMENT', registration = excluded.registration,
                     institution = excluded.institution, external_ref = excluded.external_ref,
                     updated_at = MAX(excluded.updated_at, MIN(accounts.updated_at, 4000000000000000) + 1)",
            )?.execute(params![a.uid, a.name, order, name_of(&a.registration), a.institution, a.external_ref, stamp_over(tx, "accounts", &a.uid, now)?])?;
            unbury(tx, "accounts", &a.uid)?;
            result.accounts_created += usize::from(created);
        }
        let account = |uid: &str| -> Result<i64> { id_of(tx, "accounts", uid)?.ok_or_else(|| LedgerError::NotFound(format!("No account {uid}"))) };
        let mut security_ids: HashMap<&str, i64> = HashMap::new();
        for s in &plan.securities {
            let found = match id_of(tx, "securities", &s.uid)? {
                Some(id) => Some(id),
                None => security_of(tx, &s.symbol)?.map(|(id, _)| id),
            };
            let id = match found {
                // A holdings report knows a security's name, kind and exchange; an activity does not.
                Some(id) if kind == WsKind::Holdings => {
                    tx.prepare_cached(
                        "UPDATE securities SET name = ?2, currency = ?3, kind = ?4, exchange = ?5, deleted = 0, updated_at = MAX(?6, MIN(updated_at, 4000000000000000) + 1)
                         WHERE id = ?1 AND (name != ?2 OR currency != ?3 OR kind != ?4 OR exchange != ?5 OR deleted = 1)",
                    )?.execute(params![id, s.name, s.currency, name_of(&s.kind), s.exchange, now])?;
                    id
                }
                Some(id) => {
                    tx.prepare_cached("UPDATE securities SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1")?
                        .execute(params![id, now])?;
                    id
                }
                None => {
                    tx.prepare_cached("INSERT INTO securities (uid, symbol, name, currency, kind, exchange, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
                        .execute(params![s.uid, s.symbol, s.name, s.currency, name_of(&s.kind), s.exchange, stamp_over(tx, "securities", &s.uid, now)?])?;
                    result.securities += 1;
                    tx.last_insert_rowid()
                }
            };
            unbury(tx, "securities", &s.uid)?;
            security_ids.insert(&s.uid, id);
        }
        let security = |uid: &str| security_ids.get(uid).copied().ok_or_else(|| LedgerError::NotFound(format!("No security {uid}")));
        for h in &plan.holdings {
            let added = tx.prepare_cached(
                "INSERT OR IGNORE INTO holdings (uid, account_id, security_id, date, quantity, book, book_market, updated_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 WHERE NOT EXISTS (SELECT 1 FROM tombstones WHERE tbl = 'holdings' AND uid = ?1)",
            )?.execute(params![h.uid, account(&h.account_uid)?, security(&h.security_uid)?, h.date.to_string(), h.quantity, h.book, h.book_market, now])?;
            result.holdings += added;
            result.duplicates += 1 - added;
        }
        for a in &plan.activities {
            let security_id = a.security_uid.as_deref().map(security).transpose()?;
            let added = tx.prepare_cached(
                "INSERT OR IGNORE INTO activities (uid, account_id, security_id, type, date, quantity, amount, fee, currency, to_amount, to_currency, note, source, created_at, updated_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14
                 WHERE NOT EXISTS (SELECT 1 FROM tombstones WHERE tbl = 'activities' AND uid = ?1)",
            )?.execute(params![
                a.uid, account(&a.account_uid)?, security_id, name_of(&a.r#type), a.date.to_string(), a.quantity, a.amount, a.fee, a.currency,
                a.to_amount, a.to_currency, a.note, SOURCE_WEALTHSIMPLE, now
            ])?;
            result.activities += added;
            result.duplicates += 1 - added;
        }
        for p in &plan.prices {
            result.prices += Ledger::put_price(tx, &p.uid, security(&p.security_uid)?, &p.date.to_string(), p.price, SOURCE_IMPORT, now)?;
        }
        for v in &plan.values {
            result.values += Ledger::put_value(tx, &v.uid, account(&v.account_uid)?, &v.date.to_string(), v.value, now)?;
        }
        Ok(result)
    }

    /// Writes a price under its derived uid; 0 when it was already so.
    fn put_price(tx: &Connection, uid: &str, security_id: i64, date: &str, price: i64, source: &str, now: i64) -> Result<usize> {
        let n = tx.prepare_cached(
            "INSERT INTO prices (uid, security_id, date, price, source, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(uid) DO UPDATE SET security_id = excluded.security_id, date = excluded.date, price = excluded.price,
                 source = excluded.source, deleted = 0, updated_at = MAX(excluded.updated_at, MIN(prices.updated_at, 4000000000000000) + 1)
             WHERE prices.price != excluded.price OR prices.source != excluded.source OR prices.security_id != excluded.security_id OR prices.deleted = 1",
        )?.execute(params![uid, security_id, date, price, source, stamp_over(tx, "prices", uid, now)?])?;
        unbury(tx, "prices", uid)?;
        Ok(n)
    }

    /// Writes an account's value for a day under `uid`, and retires any other value it had that
    /// day: one value per account per day, as the phone keeps them. 0 when it was already so.
    fn put_value(tx: &Connection, uid: &str, account_id: i64, date: &str, value: i64, now: i64) -> Result<usize> {
        let n = tx.prepare_cached(
            "INSERT INTO account_values (uid, account_id, date, value, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(uid) DO UPDATE SET account_id = excluded.account_id, date = excluded.date, value = excluded.value, deleted = 0,
                 updated_at = MAX(excluded.updated_at, MIN(account_values.updated_at, 4000000000000000) + 1)
             WHERE account_values.value != excluded.value OR account_values.account_id != excluded.account_id OR account_values.deleted = 1",
        )?.execute(params![uid, account_id, date, value, stamp_over(tx, "account_values", uid, now)?])?;
        unbury(tx, "account_values", uid)?;
        tx.prepare_cached(
            "UPDATE account_values SET deleted = 1, updated_at = MAX(?4, MIN(updated_at, 4000000000000000) + 1) WHERE account_id = ?1 AND date = ?2 AND uid != ?3 AND deleted = 0",
        )?.execute(params![account_id, date, uid, now])?;
        Ok(n)
    }

    /// The person's CRA room figure for a registration and year; zero or less removes it.
    pub fn room_set(&mut self, registration: Registration, year: i32, amount: i64) -> Result<()> {
        if !ROOM_REGISTRATIONS.contains(&registration) {
            return invalid("Room is kept for a TFSA, an RRSP or an FHSA");
        }
        if !(2009..=2200).contains(&year) {
            return invalid(format!("Not a year for room: {year}"));
        }
        let uid = format!("room:{}:{year}", name_of(&registration));
        let now = now_ms();
        if amount <= 0 {
            self.conn.prepare_cached("UPDATE room_facts SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE uid = ?1 AND deleted = 0")?
                .execute(params![uid, now])?;
            return Ok(());
        }
        self.conn.prepare_cached(
            "INSERT INTO room_facts (uid, registration, year, amount, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(uid) DO UPDATE SET amount = excluded.amount, deleted = 0, updated_at = MAX(excluded.updated_at, MIN(room_facts.updated_at, 4000000000000000) + 1)",
        )?.execute(params![uid, name_of(&registration), year, amount, stamp_over(&self.conn, "room_facts", &uid, now)?])?;
        unbury(&self.conn, "room_facts", &uid)
    }

    /// A security's price on a day, typed by the person, at [`invest::PRICE_SCALE`].
    pub fn price_set(&mut self, symbol: &str, date: &str, price: i64) -> Result<()> {
        let Some((id, uid)) = security_of(&self.conn, symbol)? else {
            return invalid(format!("No security {} in the ledger yet: record an activity for it first", symbol.trim().to_uppercase()));
        };
        day(date)?;
        if price <= 0 {
            return invalid("A price is above zero");
        }
        let tx = self.conn.transaction()?;
        Ledger::put_price(&tx, &format!("px:{uid}:{date}"), id, date, price, SOURCE_MANUAL, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Units of `quote` for one `base` on a day, at [`invest::RATE_SCALE`]. `source` is
    /// [`SOURCE_MANUAL`] for a rate typed in, [`SOURCE_BANK_OF_CANADA`] for one fetched.
    pub fn fx_set(&mut self, base: &str, quote: &str, date: &str, rate: i64, source: &str) -> Result<()> {
        let (base, quote) = (currency_code(base)?, currency_code(quote)?);
        if base == quote {
            return invalid("A rate is between two different currencies");
        }
        day(date)?;
        if rate <= 0 {
            return invalid("A rate is above zero");
        }
        if ![SOURCE_MANUAL, SOURCE_BANK_OF_CANADA].contains(&source) {
            return invalid(format!("Not a rate source: {source}"));
        }
        let uid = format!("fx:{base}:{quote}:{date}");
        self.conn.prepare_cached(
            "INSERT INTO fx_rates (uid, base, quote, date, rate, source, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(uid) DO UPDATE SET rate = excluded.rate, source = excluded.source, deleted = 0,
                 updated_at = MAX(excluded.updated_at, MIN(fx_rates.updated_at, 4000000000000000) + 1)
             WHERE fx_rates.rate != excluded.rate OR fx_rates.source != excluded.source OR fx_rates.deleted = 1",
        )?.execute(params![uid, base, quote, date, rate, source, stamp_over(&self.conn, "fx_rates", &uid, now_ms())?])?;
        unbury(&self.conn, "fx_rates", &uid)
    }

    /// What an investment account was worth on a day: one value per account per day, a second the
    /// same day replacing the first, as the phone keeps them.
    pub fn value_set(&mut self, account_id: i64, date: &str, value: i64) -> Result<AccountView> {
        self.investment_uid(account_id)?;
        day(date)?;
        if value < 0 {
            return invalid("A value can't be below zero");
        }
        let now = now_ms();
        let same: Option<i64> = self.conn.prepare_cached("SELECT id FROM account_values WHERE account_id = ?1 AND date = ?2 AND deleted = 0 ORDER BY id DESC LIMIT 1")?
            .query_row(params![account_id, date], |r| r.get(0))
            .optional()?;
        match same {
            Some(id) => {
                self.conn.prepare_cached("UPDATE account_values SET value = ?2, updated_at = MAX(?3, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND value != ?2")?
                    .execute(params![id, value, now])?;
            }
            None => {
                self.conn.prepare_cached("INSERT INTO account_values (uid, account_id, date, value, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)")?
                    .execute(params![new_uid(), account_id, date, value, now])?;
            }
        }
        self.account(account_id)
    }

    /// Renames an account, sets its registration or institution, or archives it.
    pub fn account_update(&mut self, id: i64, patch: &AccountPatch) -> Result<AccountView> {
        let current: Option<(String, String, Option<String>, String, bool)> = self.conn
            .prepare_cached("SELECT name, type, registration, institution, archived FROM accounts WHERE id = ?1 AND deleted = 0")?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional()?;
        let Some((name, ty, registration, institution, archived)) = current else {
            return Err(LedgerError::NotFound(format!("No account {id}")));
        };
        let name = patch.name.as_deref().map_or(name, |n| n.trim().to_string());
        if name.is_empty() {
            return invalid("An account needs a name");
        }
        if patch.registration.is_some() && parse_name::<AccountType>(&ty) != Some(AccountType::Investment) {
            return invalid("Only an investment account has a registration");
        }
        let registration = patch.registration.as_ref().map(name_of).or(registration);
        let institution = patch.institution.as_deref().map_or(institution, |i| i.trim().to_string());
        let archived = patch.archived.unwrap_or(archived);
        self.conn.prepare_cached(
            "UPDATE accounts SET name = ?2, registration = ?3, institution = ?4, archived = ?5, updated_at = MAX(?6, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1",
        )?.execute(params![id, name, registration, institution, archived, now_ms()])?;
        self.account(id)
    }

    fn account(&self, id: i64) -> Result<AccountView> {
        self.accounts()?.into_iter().find(|a| a.id == id).ok_or_else(|| LedgerError::NotFound(format!("No account {id}")))
    }
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use crate::invest::{PRICE_SCALE, QTY_SCALE};
    use crate::views::Change;
    use jiff::civil::date;
    use serde_json::json;

    const HOLDINGS_REPORT: &str = crate::wealthsimple::tests::W1;
    const ACTIVITIES_EXPORT: &str = crate::wealthsimple::tests::W2;

    fn ledger() -> Ledger {
        Ledger::open_in_memory().unwrap()
    }

    fn tfsa(l: &mut Ledger) -> i64 {
        l.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "Wealthsimple").unwrap().id
    }

    fn count(l: &Ledger, sql: &str) -> i64 {
        l.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn the_portfolio_of_an_imported_holdings_report_matches_the_report() {
        let mut l = ledger();
        let preview = l.invest_preview(HOLDINGS_REPORT, None).unwrap();
        assert_eq!((WsKind::Holdings, Some("2026-05-08"), 3, 3, 0), (preview.kind, preview.as_of.as_deref(), preview.holdings, preview.new, preview.duplicates));
        assert_eq!((None, "Demo TFSA", Registration::Tfsa, 3), (preview.accounts[0].account_id, preview.accounts[0].name.as_str(), preview.accounts[0].registration, preview.accounts[0].rows));
        let done = l.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        assert_eq!((1, 3, 3, 0, 3, 1), (done.accounts_created, done.securities, done.holdings, done.duplicates, done.prices, done.values));

        let account = &l.accounts().unwrap()[0];
        assert_eq!(("Demo TFSA", Some(Registration::Tfsa), "Wealthsimple"), (account.name.as_str(), account.registration, account.institution.as_str()));
        assert_eq!(1_633_33, account.balance, "the import records the account's value, so Home and net worth see it");
        let p = l.portfolio(date(2026, 5, 8)).unwrap();
        assert_eq!((1_633_33, Some("2026-05-08")), (p.value, p.as_of.as_deref()));
        let held: Vec<(&str, i64, i64)> = p.holdings.iter().map(|h| (h.symbol.as_str(), h.quantity, h.book)).collect();
        assert_eq!(vec![("AAPL", 10 * QTY_SCALE, 1_000_00), ("XEQT", 10 * QTY_SCALE, 250_00), ("ARKK", QTY_SCALE, 50_00)], held);
        assert_eq!(Some(100 * PRICE_SCALE), p.holdings[0].price);
        assert_eq!(account.id, l.invest_preview(HOLDINGS_REPORT, None).unwrap().accounts[0].account_id.unwrap(), "the account holds the number now");
    }

    #[test]
    fn an_import_twice_adds_once() {
        let mut l = ledger();
        let first = l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        assert_eq!((1, 5, 0, 1), (first.accounts_created, first.activities, first.duplicates, first.skipped));
        let preview = l.invest_preview(ACTIVITIES_EXPORT, None).unwrap();
        assert_eq!((5, 0, 5), (preview.activities, preview.new, preview.duplicates));
        let again = l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        assert_eq!((0, 0, 0, 5), (again.accounts_created, again.securities, again.activities, again.duplicates));
        assert_eq!(5, count(&l, "SELECT COUNT(*) FROM activities"));
        assert_eq!(1, count(&l, "SELECT COUNT(*) FROM accounts"));
        // "A new account" for a number an import already created is that account: none is counted.
        let chosen = l.invest_import(ACTIVITIES_EXPORT, &[ImportAccount { number: "HQ7XFMC41CAD".into(), account_id: None }], None).unwrap();
        assert_eq!((0, 0, 5), (chosen.accounts_created, chosen.activities, chosen.duplicates));
        assert_eq!(1, count(&l, "SELECT COUNT(*) FROM accounts"));
        // The same holdings report twice: the value and the prices are written once too.
        l.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        let twice = l.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        assert_eq!((0, 3, 0, 0), (twice.holdings, twice.duplicates, twice.prices, twice.values));
    }

    #[test]
    fn a_deleted_imported_activity_is_not_imported_again() {
        let mut l = ledger();
        l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        let buy = l.invest_list(&ActivityQuery { r#type: Some(ActivityType::Buy), ..Default::default() }).unwrap().into_iter().last().unwrap();
        assert_eq!(("WEALTHSIMPLE", Some("XEQT")), (buy.source.as_str(), buy.symbol.as_deref()));
        l.invest_delete(buy.id).unwrap();
        let again = l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        assert_eq!((0, 5), (again.activities, again.duplicates));
        assert_eq!(4, l.invest_list(&ActivityQuery::default()).unwrap().len());
        // Erased outright, it leaves a tombstone, which keeps it out too.
        l.conn.execute("INSERT INTO tombstones (tbl, uid, updated_at) SELECT 'activities', uid, 1 FROM activities", []).unwrap();
        l.conn.execute("DELETE FROM activities", []).unwrap();
        assert_eq!(0, l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap().activities);
        assert!(l.invest_restore(buy.id).is_err(), "nothing left to bring back");
    }

    #[test]
    fn a_file_goes_into_the_account_it_is_mapped_to() {
        let mut l = ledger();
        let mine = tfsa(&mut l);
        let done = l.invest_import(ACTIVITIES_EXPORT, &[ImportAccount { number: "HQ7XFMC41CAD".into(), account_id: Some(mine) }], None).unwrap();
        assert_eq!((0, 5), (done.accounts_created, done.activities));
        assert!(l.invest_list(&ActivityQuery::default()).unwrap().iter().all(|a| a.account_id == mine));
        assert_eq!(Some((mine, l.investment_uid(mine).unwrap())), l.account_by_ref("HQ7XFMC41CAD").unwrap(), "it remembers the number");
        let statement = crate::wealthsimple::tests::W3;
        assert!(matches!(l.invest_import(statement, &[], None), Err(LedgerError::Invalid(_))), "a statement names no account");
        assert_eq!(3, l.invest_import(statement, &[], Some(mine)).unwrap().activities);
        assert!(matches!(l.invest_import("Date,Amount\n2026-01-01,4\n", &[], None), Err(LedgerError::Invalid(_))));
        // A cash statement is told where it goes.
        let cash = "date,transaction,description,amount,balance,currency\n2026-09-01,SPEND,Metro,-42.10,500.00,CAD\n";
        match l.invest_preview(cash, Some(mine)) {
            Err(LedgerError::Invalid(why)) => assert_eq!(CASH_STATEMENT, why),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn activities_by_hand_add_list_delete_and_restore() {
        let mut l = ledger();
        let account = tfsa(&mut l);
        let input = |ty: ActivityType, symbol: Option<&str>, quantity: Option<i64>, amount: i64| ActivityInput {
            account_id: account, r#type: ty, date: "2026-10-01".into(), symbol: symbol.map(Into::into), currency: None, quantity, amount,
            fee: None, note: None, to_amount: None, to_currency: None,
        };
        l.invest_add(&input(ActivityType::Deposit, None, None, 1_000_00)).unwrap();
        let buy = l.invest_add(&input(ActivityType::Buy, Some(" xeqt "), Some(10 * QTY_SCALE), 381_20)).unwrap();
        assert_eq!((Some("XEQT"), "CAD", "MANUAL"), (buy.symbol.as_deref(), buy.currency.as_str(), buy.source.as_str()));
        assert_eq!(1, count(&l, "SELECT COUNT(*) FROM securities WHERE uid = 'sec:XEQT'"));
        let bad = |l: &mut Ledger, i: ActivityInput| matches!(l.invest_add(&i), Err(LedgerError::Invalid(_)));
        assert!(bad(&mut l, input(ActivityType::Buy, None, Some(QTY_SCALE), 5_00)), "a buy names its security");
        assert!(bad(&mut l, input(ActivityType::Buy, Some("XEQT"), None, 5_00)), "and its units");
        assert!(bad(&mut l, input(ActivityType::Deposit, None, None, -5_00)));
        assert!(bad(&mut l, input(ActivityType::Fx, None, None, 5_00)), "an exchange says what it became");
        assert!(bad(&mut l, ActivityInput { account_id: 404, ..input(ActivityType::Deposit, None, None, 5_00) }));
        l.price_set("XEQT", "2026-10-02", 40 * PRICE_SCALE).unwrap();
        let p = l.portfolio(date(2026, 10, 9)).unwrap();
        assert_eq!((400_00 + 618_80, 381_20 + 618_80), (p.value, p.book), "10 at $40 and the cash left");
        l.invest_delete(buy.id).unwrap();
        assert_eq!(1, l.invest_list(&ActivityQuery { account_id: Some(account), ..Default::default() }).unwrap().len());
        assert!(matches!(l.invest_delete(buy.id), Err(LedgerError::NotFound(_))));
        assert_eq!(381_20, l.invest_restore(buy.id).unwrap().amount, "Undo brings it back as it was");
    }

    #[test]
    fn room_rates_values_and_account_edits() {
        let mut l = ledger();
        let account = tfsa(&mut l);
        l.room_set(Registration::Tfsa, 2026, 7_000_00).unwrap();
        assert_eq!(Some(7_000_00), l.portfolio(date(2026, 10, 9)).unwrap().room[0].room);
        l.room_set(Registration::Tfsa, 2026, 0).unwrap();
        assert_eq!(None, l.portfolio(date(2026, 10, 9)).unwrap().room[0].room, "zero removes the figure");
        assert!(matches!(l.room_set(Registration::Resp, 2026, 1), Err(LedgerError::Invalid(_))));
        l.fx_set("usd", "CAD", "2026-10-01", 137_125_000, SOURCE_BANK_OF_CANADA).unwrap();
        assert!(matches!(l.fx_set("CAD", "CAD", "2026-10-01", 1, SOURCE_MANUAL), Err(LedgerError::Invalid(_))));
        assert_eq!(1, count(&l, "SELECT COUNT(*) FROM fx_rates WHERE uid = 'fx:USD:CAD:2026-10-01'"));
        assert_eq!(6_000_00, l.value_set(account, "2026-10-01", 6_000_00).unwrap().balance);
        assert_eq!(6_100_00, l.value_set(account, "2026-10-01", 6_100_00).unwrap().balance);
        assert_eq!(1, count(&l, "SELECT COUNT(*) FROM account_values WHERE deleted = 0"), "one value per account per day");
        let edited = l.account_update(account, &AccountPatch { name: Some(" My TFSA ".into()), registration: Some(Registration::Fhsa), ..Default::default() }).unwrap();
        assert_eq!(("My TFSA", Some(Registration::Fhsa), "Wealthsimple"), (edited.name.as_str(), edited.registration, edited.institution.as_str()));
        let chequing = l.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
        let refused = l.account_update(chequing, &AccountPatch { registration: Some(Registration::Tfsa), ..Default::default() });
        assert!(matches!(refused, Err(LedgerError::Invalid(_))));
        assert!(matches!(l.value_set(chequing, "2026-10-01", 1), Err(LedgerError::Invalid(_))));
    }

    #[test]
    fn investments_and_registrations_ride_through_a_backup() {
        let mut l = ledger();
        l.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        l.room_set(Registration::Tfsa, 2026, 7_000_00).unwrap();
        l.fx_set("USD", "CAD", "2026-05-01", 137_125_000, SOURCE_MANUAL).unwrap();
        let file = l.export_backup("2026-10-09").unwrap();
        assert_eq!((3, 3, 5, 3, 1, 1), (file.securities.len(), file.holdings.len(), file.activities.len(), file.prices.len(), file.fx_rates.len(), file.room_facts.len()));
        let demo = file.accounts.iter().find(|a| a.external_ref == "DEMO0001CAD").unwrap();
        assert_eq!((Some(Registration::Tfsa), "Wealthsimple"), (demo.registration, demo.institution.as_str()));
        let mut restored = ledger();
        let crate::backup::BackupReadResult::Ok(read) = crate::backup::decode(&crate::backup::encode(&file)) else { panic!("the export reads back") };
        restored.import_backup(&read).unwrap();
        assert_eq!(file, restored.export_backup("2026-10-09").unwrap());
        assert_eq!(l.portfolio(date(2026, 10, 9)).unwrap(), restored.portfolio(date(2026, 10, 9)).unwrap());
    }

    #[test]
    fn a_restore_keeps_investment_uids_so_a_re_import_adds_nothing() {
        let mut l = ledger();
        l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        let file = l.export_backup("2026-10-09").unwrap();
        assert_eq!(Some("ws:HQ7XFMC41CAD"), file.accounts[0].uid.as_deref());
        assert!(file.securities.iter().all(|s| s.uid.as_deref().is_some_and(|u| u.starts_with("sec:"))));
        assert!(file.activities.iter().all(|a| a.uid.as_deref().is_some_and(|u| u.starts_with("imp:"))));
        let crate::backup::BackupReadResult::Ok(read) = crate::backup::decode(&crate::backup::encode(&file)) else { panic!("the export reads back") };
        // Restored on a device that never saw the file, and over the ledger that wrote it.
        for mut restored in [ledger(), l] {
            restored.import_backup(&read).unwrap();
            let preview = restored.invest_preview(ACTIVITIES_EXPORT, None).unwrap();
            assert_eq!((0, 5), (preview.new, preview.duplicates));
            let again = restored.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
            assert_eq!((0, 0, 0, 5), (again.accounts_created, again.securities, again.activities, again.duplicates));
            assert_eq!((1, 5), (count(&restored, "SELECT COUNT(*) FROM accounts"), count(&restored, "SELECT COUNT(*) FROM activities")));
            let buried = "SELECT COUNT(*) FROM tombstones t WHERE EXISTS (SELECT 1 FROM activities a WHERE a.uid = t.uid) AND t.tbl = 'activities'";
            assert_eq!(0, count(&restored, buried), "a restored row leaves no tombstone that would erase it on the phone");
        }
    }

    #[test]
    fn a_restore_gives_a_blank_or_repeated_uid_a_fresh_one() {
        let mut l = ledger();
        l.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        let mut file = l.export_backup("2026-10-09").unwrap();
        file.activities[1].uid = file.activities[0].uid.clone();
        file.activities[2].uid = Some(" ".into());
        file.activities[3].uid = None;
        let mut restored = ledger();
        restored.import_backup(&file).unwrap();
        assert_eq!(5, count(&restored, "SELECT COUNT(DISTINCT uid) FROM activities"));
        assert_eq!(2, count(&restored, "SELECT COUNT(*) FROM activities WHERE uid LIKE 'imp:%'"));
        assert_eq!(3, restored.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap().activities, "the lines whose uid was lost come in again");
    }

    #[test]
    fn an_import_values_us_dollars_at_the_ledgers_rate_as_the_phone_does() {
        let mut l = ledger();
        l.fx_set("USD", "CAD", "2026-05-01", 130_000_000, SOURCE_MANUAL).unwrap();
        l.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        // AAPL's 1,000.00 USD and ARKK's 50.00 USD at 1.30, and XEQT's 250.00 CAD; not by their book (1,633.33).
        assert_eq!(1_300_00 + 250_00 + 65_00, l.accounts().unwrap()[0].balance);
        let file = wealthsimple::read(HOLDINGS_REPORT).unwrap();
        let mut input = PlanInput::new("CAD");
        input.fx_rates = l.portfolio_input(date(2026, 5, 8)).unwrap().fx_rates;
        assert_eq!(1_615_00, wealthsimple::plan(&file, &input).unwrap().values[0].value, "the plan the phone makes with the same rates");
    }

    #[test]
    fn a_day_off_the_calendar_is_refused_not_read() {
        let mut l = ledger();
        tfsa(&mut l);
        l.account_add("RRSP", AccountType::Investment, 0, Some(Registration::Rrsp), "").unwrap();
        assert!(matches!(l.portfolio(date(9999, 6, 1)), Err(LedgerError::Invalid(_))));
        assert!(matches!(l.portfolio(date(-9999, 1, 5)), Err(LedgerError::Invalid(_))));
        assert_eq!(2, l.portfolio(date(9998, 6, 1)).unwrap().accounts.len());
    }

    #[test]
    fn a_line_deleted_on_the_phone_is_not_imported_again_after_the_phone_pairs_again() {
        let mut pc = ledger();
        pc.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        // The phone takes the PC's ledger, deletes an imported line, and syncs.
        let whole = pc.sync("Pixel", false, 0, None, &[], "t").unwrap();
        let line = whole.changes.iter().find(|c| c.table == "activities").unwrap().clone();
        assert!(line.uid.starts_with("imp:"));
        let deleted = Change { deleted: true, updated_at: line.updated_at.min(crate::ledger::STAMP_CEILING) + 1, row: Default::default(), ..line.clone() };
        pc.sync("Pixel", false, whole.cursor, Some(&whole.generation), &[deleted], "t").unwrap();
        // It pairs again: its first sync sends what it holds, never its tombstones.
        let rows: Vec<Change> = whole.changes.iter().filter(|c| !c.deleted && c.uid != line.uid).cloned().collect();
        pc.sync("Pixel", true, 0, None, &rows, "t").unwrap();
        assert_eq!(4, pc.invest_list(&ActivityQuery::default()).unwrap().len());
        // The same file imported here again leaves the deleted line out, as before the replace.
        let again = pc.invest_import(ACTIVITIES_EXPORT, &[], None).unwrap();
        assert_eq!((0, 5), (again.activities, again.duplicates));
        assert_eq!(4, pc.invest_list(&ActivityQuery::default()).unwrap().len());
    }

    #[test]
    fn a_derived_row_written_again_is_stamped_past_its_tombstone() {
        let mut pc = ledger();
        tfsa(&mut pc);
        pc.invest_add(&ActivityInput { account_id: pc.accounts().unwrap()[0].id, r#type: ActivityType::Buy, date: "2026-10-01".into(),
            symbol: Some("XEQT".into()), currency: Some("CAD".into()), quantity: Some(QTY_SCALE), amount: 40_00, fee: None, note: None,
            to_amount: None, to_currency: None }).unwrap();
        let first = pc.sync("Pixel", false, 0, None, &[], "t").unwrap();
        // The phone, whose clock runs an hour ahead, deleted these.
        let ahead = now_ms() + 60 * 60 * 1000;
        let tomb = |table: &str, uid: &str| Change { table: table.into(), uid: uid.into(), updated_at: ahead, deleted: true, row: Default::default() };
        pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[
            tomb("prices", "px:sec:XEQT:2026-10-02"), tomb("fx_rates", "fx:USD:CAD:2026-10-02"), tomb("room_facts", "room:TFSA:2026"),
        ], "t").unwrap();
        pc.price_set("XEQT", "2026-10-02", 40 * PRICE_SCALE).unwrap();
        pc.fx_set("USD", "CAD", "2026-10-02", 137_000_000, SOURCE_MANUAL).unwrap();
        pc.room_set(Registration::Tfsa, 2026, 7_000_00).unwrap();
        for (table, uid) in [("prices", "px:sec:XEQT:2026-10-02"), ("fx_rates", "fx:USD:CAD:2026-10-02"), ("room_facts", "room:TFSA:2026")] {
            let at: i64 = pc.conn.query_row(&format!("SELECT updated_at FROM {table} WHERE uid = ?1"), [uid], |r| r.get(0)).unwrap();
            assert!(at > ahead, "{uid} at {at}, its tombstone at {ahead}: the phone would refuse it");
        }
        assert_eq!(0, count(&pc, "SELECT COUNT(*) FROM tombstones"));
    }

    #[test]
    fn investment_rows_sync_both_ways() {
        let mut pc = ledger();
        let phone = |table: &str, uid: &str, row: serde_json::Value| Change {
            table: table.into(), uid: uid.into(), updated_at: 100, deleted: false, row: row.as_object().unwrap().clone(),
        };
        let first = pc.sync(
            "Pixel",
            true,
            0,
            None,
            &[
                phone("activities", "imp:1", json!({"account": "ws:A1", "security": "sec:XEQT", "type": "BUY", "date": "2026-08-18",
                    "quantity": 1_000_000_000, "amount": 381_20, "fee": 0, "currency": "CAD", "toAmount": null, "toCurrency": null,
                    "note": "", "source": "WEALTHSIMPLE", "createdAt": 100})),
                phone("securities", "sec:XEQT", json!({"symbol": "XEQT", "name": "", "currency": "CAD", "kind": "ETF", "exchange": "TSX"})),
                // An account from a phone that predates investments: no registration, institution or number.
                phone("accounts", "ws:A1", json!({"name": "TFSA", "type": "INVESTMENT", "openingBalance": 0, "archived": false, "sortOrder": 0})),
                phone("room_facts", "room:TFSA:2026", json!({"registration": "TFSA", "year": 2026, "amount": 7_000_00})),
                phone("prices", "px:sec:XEQT:2026-10-01", json!({"security": "sec:XEQT", "date": "2026-10-01", "price": 4_000_000_000_i64, "source": "MANUAL"})),
            ],
            "t",
        ).unwrap();
        assert_eq!((5, 0), (first.applied, first.skipped));
        let account = &pc.accounts().unwrap()[0];
        assert_eq!((None, ""), (account.registration, account.institution.as_str()), "missing keys read as null and empty");
        let p = pc.portfolio(date(2026, 10, 9)).unwrap();
        assert_eq!((40_000, Some(7_000_00)), (p.holdings[0].value, p.room[0].room));

        // The PC's own investment writes travel back, references as uids.
        pc.fx_set("USD", "CAD", "2026-10-01", 137_125_000, SOURCE_MANUAL).unwrap();
        pc.account_update(account.id, &AccountPatch { registration: Some(Registration::Tfsa), ..Default::default() }).unwrap();
        pc.invest_import(HOLDINGS_REPORT, &[], None).unwrap();
        let out = pc.sync("Pixel", false, first.cursor, Some(&first.generation), &[], "t").unwrap();
        let tables: Vec<&str> = out.changes.iter().map(|c| c.table.as_str()).collect();
        for t in ["accounts", "securities", "holdings", "prices", "fx_rates", "account_values"] {
            assert!(tables.contains(&t), "{t} in {tables:?}");
        }
        let tfsa = out.changes.iter().find(|c| c.uid == "ws:A1").unwrap();
        assert_eq!((&json!("TFSA"), &json!("")), (&tfsa.row["registration"], &tfsa.row["externalRef"]));
        let holding = out.changes.iter().find(|c| c.table == "holdings" && c.uid == "hold:ws:DEMO0001CAD:sec:XEQT:2026-05-08").unwrap();
        assert_eq!((&json!("ws:DEMO0001CAD"), &json!("sec:XEQT"), &json!(25_000)), (&holding.row["account"], &holding.row["security"], &holding.row["bookMarket"]));
        let created = out.changes.iter().find(|c| c.uid == "ws:DEMO0001CAD").unwrap();
        assert_eq!((&json!("Wealthsimple"), &json!("DEMO0001CAD")), (&created.row["institution"], &created.row["externalRef"]));
        // A deleted activity travels as a tombstone.
        let id = pc.invest_list(&ActivityQuery::default()).unwrap()[0].id;
        pc.invest_delete(id).unwrap();
        let gone = pc.sync("Pixel", false, out.cursor, Some(&out.generation), &[], "t").unwrap();
        assert!(gone.changes.iter().any(|c| c.table == "activities" && c.uid == "imp:1" && c.deleted));
    }
}
