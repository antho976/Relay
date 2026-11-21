//! The bank import's reads and its one write (Kotlin `ImportRepository.kt`), and Tally's own CSV
//! read back (`DataRepository.importCsvText`).

use jiff::ToSpan;
use relay_money::Date;
use relay_money::ledger::{Ledger, LedgerError, MAX_AMOUNT};
use relay_money::model::{AccountType, CategoryKind, TxType};
use relay_money::views::{TxInput, TxQuery};

use crate::import_logic::{ImportLine, NoteUse, Opening, Twins};
use crate::model::Store;

fn range(from: Date, to: Date, account: Option<i64>) -> TxQuery {
    TxQuery { from: Some(from), to: Some(to), account_id: account, limit: Some(u32::MAX), ..Default::default() }
}

/// What `account` already holds between `first` and `last`, as signed effects by day: income and
/// transfers in above zero, spending and transfers out below. A statement line that finds its
/// twin here is already logged.
pub fn existing_effects(store: &Store, account: i64, first: Date, last: Date) -> Vec<(Date, i64)> {
    let Ok(page) = store.ledger.tx_list(&range(first, last, Some(account)), crate::model::today()) else { return Vec::new() };
    page.transactions
        .into_iter()
        .filter_map(|t| {
            let signed = match t.r#type {
                TxType::Transfer if t.to_account_id == Some(account) && t.account_id != account => t.amount,
                TxType::Transfer => -t.amount,
                TxType::Income => t.amount,
                TxType::Expense => -t.amount,
            };
            Some((t.date.parse().ok()?, signed))
        })
        .collect()
}

/// Notes filed under a category over the last year: what the import learns payees from.
pub fn learned_notes(store: &Store, today: Date) -> Vec<NoteUse> {
    let from = today.checked_sub(1.year()).unwrap_or(today);
    let Ok(page) = store.ledger.tx_list(&range(from, today, None), today) else { return Vec::new() };
    page.transactions
        .into_iter()
        .filter(|t| t.r#type != TxType::Transfer && !t.note.trim().is_empty())
        .filter_map(|t| Some(NoteUse { note: t.note, category_id: t.category_id?, r#type: t.r#type }))
        .collect()
}

/// Where an import lands: an account that exists, or one it creates.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Existing(i64),
    New { name: String, ty: AccountType, opening: i64 },
}

/// Writes `lines` into `target` in one transaction: a failure adds nothing. Returns the account
/// the lines went into.
pub fn write(ledger: &mut Ledger, target: &Target, lines: &[ImportLine]) -> Result<i64, String> {
    ledger
        .atomically(|l| {
            let account = match target {
                Target::Existing(id) => *id,
                Target::New { name, ty, opening } => l.account_add(name, *ty, *opening, None, "")?.id,
            };
            for line in lines.iter().filter(|x| x.amount != 0) {
                let size = line.amount.abs();
                let other = line.counterpart.filter(|&o| o != account);
                let input = match other {
                    Some(o) if line.amount < 0 => TxInput {
                        r#type: TxType::Transfer,
                        amount: size,
                        date: line.date.to_string(),
                        account_id: account,
                        to_account_id: Some(o),
                        category_id: None,
                        note: Some(line.note.clone()),
                    },
                    Some(o) => TxInput {
                        r#type: TxType::Transfer,
                        amount: size,
                        date: line.date.to_string(),
                        account_id: o,
                        to_account_id: Some(account),
                        category_id: None,
                        note: Some(line.note.clone()),
                    },
                    None => TxInput {
                        r#type: if line.amount < 0 { TxType::Expense } else { TxType::Income },
                        amount: size,
                        date: line.date.to_string(),
                        account_id: account,
                        to_account_id: None,
                        category_id: line.category_id,
                        note: Some(line.note.clone()),
                    },
                };
                l.tx_add(&input)?;
            }
            Ok::<i64, LedgerError>(account)
        })
        .map_err(|e| e.to_string())
}

/// What an older statement's lines meet in `account`: its opening balance, its first entry's day
/// and whether a recorded value sets its balance. None when it is gone, or the ledger is held in
/// memory (no second connection to read the opening by: relay-money's views leave it out).
pub fn opening_of(store: &Store, account: i64) -> Option<Opening> {
    use rusqlite::OptionalExtension;
    let db = store.sql().ok()?;
    let (opening, first, valued): (i64, Option<String>, bool) = db
        .prepare_cached(
            "SELECT a.opening_balance,
                    (SELECT MIN(t.date) FROM transactions t WHERE t.deleted = 0 AND (t.account_id = a.id OR t.to_account_id = a.id)),
                    EXISTS (SELECT 1 FROM account_values v WHERE v.deleted = 0 AND v.account_id = a.id)
             FROM accounts a WHERE a.id = ?1 AND a.deleted = 0",
        )
        .ok()?
        .query_row([account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()
        .ok()
        .flatten()?;
    Some(Opening { opening, first_entry: first.and_then(|d| d.parse().ok()), valued })
}

/// What [`write_moving_opening`] did: the account the lines went into, and whether its opening
/// balance moved with them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Written {
    pub account: i64,
    pub moved: bool,
}

/// Writes `lines` into `account` and moves its opening balance from `from` to `to`: the opening
/// already held the lines dated before its first entry ([`crate::import_logic::Earlier::Move`]).
///
/// relay-money has no call that changes an opening balance, so it goes through the second
/// connection, which cannot join the ledger's transaction (the ledger holds the write lock while
/// its lines go in). So the lines go first, in one transaction, and the opening after. Stopped
/// between the two (the app killed), the lines are in and the opening is not moved: the file
/// read again finds every line already logged and still offers the move
/// ([`crate::import_logic::earlier_lines`]), so nothing is lost and nothing counts twice. The
/// other order would leave a moved opening with its lines missing, which nothing finds again.
///
/// It only moves from the `from` the preview read: one changed since (a sync) refuses before
/// anything is written. A move that fails after the lines are in says so ([`Written::moved`]).
/// Stamped as accounts_store stamps an edit, so the move syncs and outranks what it replaces.
pub fn write_moving_opening(store: &mut Store, account: i64, from: i64, to: i64, lines: &[ImportLine]) -> Result<Written, String> {
    if from == to {
        return write(&mut store.ledger, &Target::Existing(account), lines).map(|account| Written { account, moved: false });
    }
    if opening_of(store, account).map(|o| o.opening) != Some(from) {
        return Err("The account's starting balance changed since the preview: open the file again.".into());
    }
    let account = write(&mut store.ledger, &Target::Existing(account), lines)?;
    Ok(Written { account, moved: move_opening(store, account, from, to).is_ok_and(|n| n == 1) })
}

/// Sets `account`'s opening balance from `was` to `now_is`, only when it still reads `was`.
/// Returns the rows changed.
fn move_opening(store: &Store, account: i64, was: i64, now_is: i64) -> Result<usize, String> {
    let (_, now) = Store::stamp();
    store
        .sql()?
        .prepare_cached(
            "UPDATE accounts SET opening_balance = ?3, updated_at = MAX(?4, MIN(updated_at, 4000000000000000) + 1)
             WHERE id = ?1 AND deleted = 0 AND opening_balance = ?2",
        )
        .and_then(|mut q| q.execute(rusqlite::params![account, was, now_is, now]))
        .map_err(|e| e.to_string())
}

/// A line of Tally's CSV as the ledger would match it: day, type, amount, the account names (any
/// case) and the note. Its category is left out: a line filed elsewhere since is still the same line.
type CsvKey = (Date, TxType, i64, String, Option<String>, String);

fn csv_key(date: Date, ty: TxType, amount: i64, account: &str, to: Option<&str>, note: &str) -> CsvKey {
    let to = to.filter(|_| ty == TxType::Transfer).map(str::to_lowercase);
    (date, ty, amount, account.to_lowercase(), to, note.trim().to_string())
}

/// Tally's own CSV (its export) read back: accounts and categories by name, made when missing.
/// A line the ledger would refuse (an amount past [`MAX_AMOUNT`], a transfer to its own account)
/// is skipped and counted, never the whole file; a line already in the ledger (its export read
/// back) is counted and left, by the same twin count as a bank statement's. Lines with no account
/// go to the default account. Returns what it did, in words.
///
/// [`MAX_AMOUNT`]: relay_money::ledger::MAX_AMOUNT
pub fn import_tally_csv(store: &mut Store, text: &str) -> Result<String, String> {
    let digits = store.formatter().fraction_digits;
    let accounts = store.ledger.accounts().map_err(|e| e.to_string())?;
    // Kotlin: the default account, else the first one, else a new "Imported".
    let default_id = crate::accounts_store::default_account(store);
    let default_account = default_id
        .and_then(|id| accounts.iter().find(|a| a.id == id))
        .or_else(|| accounts.iter().find(|a| !a.archived))
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "Imported".into());
    // The export names its currency on every line. Amounts are read in this ledger's decimals, so
    // a file in a currency with other decimals ("12.50" into yen) would come in 100 times off. One
    // with the same decimals (USD into CAD) would come in at the right size but counted as this
    // ledger's money, which it is not: refused too, with nothing converted behind the owner's back.
    let records = relay_money::csv::parse_records(text, ',');
    if let Some(cur_col) = records.first().and_then(|h| h.iter().position(|c| c.trim().eq_ignore_ascii_case("currency"))) {
        let ours = store.formatter().currency.to_uppercase();
        let codes: Vec<String> = records.iter().skip(1).filter_map(|r| r.get(cur_col).map(|c| c.trim().to_uppercase())).filter(|c| !c.is_empty()).collect();
        if let Some(code) = codes.iter().find(|c| relay_money::money::fraction_digits(c).is_some_and(|d| d != digits)) {
            return Err(format!(
                "That file's amounts are in {code}, which has a different number of decimals from this ledger's currency. Nothing was imported."
            ));
        }
        if let Some(code) = codes.iter().find(|c| **c != ours) {
            return Err(format!(
                "That file's amounts are in {code}, but this ledger keeps {ours}. Nothing was imported."
            ));
        }
    }
    let parsed = relay_money::csv::decode(text, digits, &default_account);
    if parsed.rows.is_empty() {
        return Err(parsed.errors.first().cloned().unwrap_or_else(|| "No entries found in that file.".into()));
    }
    let mut skipped = parsed.errors.len();
    // What the ledger already holds over the file's days.
    let (first, last) = parsed.rows.iter().fold((parsed.rows[0].date, parsed.rows[0].date), |(a, b), r| (a.min(r.date), b.max(r.date)));
    let logged = store.ledger.tx_list(&range(first, last, None), crate::model::today()).map_err(|e| e.to_string())?;
    let mut twins = Twins::new(
        logged.transactions.iter().filter_map(|t| Some(csv_key(t.date.parse().ok()?, t.r#type, t.amount, &t.account, t.to_account.as_deref(), &t.note))),
    );
    let mut already = 0usize;
    let mut rows = Vec::new();
    for r in parsed.rows {
        let to = r.to_account.as_deref().filter(|_| r.r#type == TxType::Transfer);
        if r.amount <= 0 || r.amount > MAX_AMOUNT || to.is_some_and(|t| t.to_lowercase() == r.account.to_lowercase()) {
            skipped += 1;
        } else if twins.take(&csv_key(r.date, r.r#type, r.amount, &r.account, to, &r.note)) {
            already += 1;
        } else {
            rows.push(r);
        }
    }
    // Categories first, through the second connection (relay-money has no call to add one), and
    // only for lines that will be written. The ledger cannot take them into its own transaction,
    // so a failure below takes them back out.
    let mut categories = store.ledger.categories().map_err(|e| e.to_string())?;
    let mut made: Vec<i64> = Vec::new();
    for r in &rows {
        let Some(name) = r.category.as_deref().map(str::trim).filter(|n| !n.is_empty()) else { continue };
        if r.r#type == TxType::Transfer {
            continue;
        }
        let kind = if r.r#type == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if categories.iter().any(|c| c.kind == kind && c.name.trim().to_lowercase() == name.to_lowercase()) {
            continue;
        }
        match add_category(store, name, kind, ((made.len() + 3) % 12) as i64) {
            Ok(id) => made.push(id),
            Err(e) => {
                forget_categories(store, &made);
                return Err(format!("The import did not finish. Nothing was added. {e}"));
            }
        }
        categories = store.ledger.categories().map_err(|e| e.to_string())?;
    }
    let category_id = |name: &Option<String>, ty: TxType| -> Option<i64> {
        let name = name.as_deref()?.trim();
        if name.is_empty() || ty == TxType::Transfer {
            return None;
        }
        let kind = if ty == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        categories.iter().find(|c| c.kind == kind && c.name.trim().to_lowercase() == name.to_lowercase()).map(|c| c.id)
    };
    let rows: Vec<(Option<i64>, relay_money::csv::CsvRow)> = rows.into_iter().map(|r| (category_id(&r.category, r.r#type), r)).collect();
    let result = store.ledger.atomically(|l| {
        let mut by_name: Vec<(String, i64)> = l.accounts()?.into_iter().map(|a| (a.name.to_lowercase(), a.id)).collect();
        let (mut added, mut refused) = (0usize, 0usize);
        for (category, r) in &rows {
            // Each line in a savepoint of its own: one the ledger refuses leaves nothing behind,
            // not even the account it would have made.
            let known = by_name.len();
            let line = l.atomically(|l| {
                let mut account = |l: &mut Ledger, name: &str| -> Result<i64, LedgerError> {
                    if let Some((_, id)) = by_name.iter().find(|(n, _)| *n == name.to_lowercase()) {
                        return Ok(*id);
                    }
                    let id = l.account_add(name, AccountType::Chequing, 0, None, "")?.id;
                    by_name.push((name.to_lowercase(), id));
                    Ok(id)
                };
                let from = account(l, &r.account)?;
                let to = match &r.to_account {
                    Some(n) if r.r#type == TxType::Transfer => Some(account(l, n)?),
                    _ => None,
                };
                l.tx_add(&TxInput { r#type: r.r#type, amount: r.amount, date: r.date.to_string(), account_id: from, to_account_id: to, category_id: *category, note: Some(r.note.clone()) })
            });
            match line {
                Ok(_) => added += 1,
                Err(LedgerError::Invalid(_)) => {
                    by_name.truncate(known);
                    refused += 1;
                }
                Err(e) => return Err(e),
            }
        }
        Ok::<(usize, usize), LedgerError>((added, refused))
    });
    match result {
        Ok((n, refused)) => Ok(crate::import_logic::csv_imported_line(n, skipped + refused, already)),
        Err(e) => {
            forget_categories(store, &made);
            Err(format!("The import did not finish. Nothing was added. {e}"))
        }
    }
}

/// Takes back categories this import made, when it did not finish: they were never synced (a
/// sync reads the ledger on this thread too), so they go outright rather than as deletes.
fn forget_categories(store: &Store, ids: &[i64]) {
    let Ok(db) = store.sql() else { return };
    for id in ids {
        let _ = db.execute("DELETE FROM categories WHERE id = ?1", [id]);
    }
}

/// A category, written as relay-money writes its own: a fresh uid, stamped now, last in order.
/// Returns its id.
fn add_category(store: &Store, name: &str, kind: CategoryKind, color: i64) -> Result<i64, String> {
    let db = store.sql()?;
    let (uid, now) = Store::stamp();
    let kind = match kind {
        CategoryKind::Expense => "EXPENSE",
        CategoryKind::Income => "INCOME",
    };
    db.execute(
        "INSERT INTO categories (uid, name, kind, color, icon, sort_order, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'dots', (SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories), ?5)",
        rusqlite::params![uid, name.trim(), kind, color, now],
    )
    .map(|_| db.last_insert_rowid())
    .map_err(|e| e.to_string())
}
