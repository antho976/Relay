//! The bank import's reads and its one write (Kotlin `ImportRepository.kt`), and Tally's own CSV
//! read back (`DataRepository.importCsvText`).

use jiff::ToSpan;
use relay_money::Date;
use relay_money::ledger::{Ledger, LedgerError};
use relay_money::model::{AccountType, CategoryKind, TxType};
use relay_money::views::{TxInput, TxQuery};

use crate::import_logic::{ImportLine, NoteUse};
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

/// Tally's own CSV (its export) read back: accounts and categories by name, made when missing.
/// Returns what it did, in words.
pub fn import_tally_csv(store: &mut Store, text: &str) -> Result<String, String> {
    let digits = store.formatter().fraction_digits;
    let accounts = store.ledger.accounts().map_err(|e| e.to_string())?;
    let default_account = accounts.iter().find(|a| !a.archived).map(|a| a.name.clone()).unwrap_or_else(|| "Imported".into());
    let parsed = relay_money::csv::decode(text, digits, &default_account);
    if parsed.rows.is_empty() {
        return Err(parsed.errors.first().cloned().unwrap_or_else(|| "No entries found in that file.".into()));
    }
    // Categories first, through the second connection (relay-money has no call to add one); the
    // rest goes in one transaction.
    let mut categories = store.ledger.categories().map_err(|e| e.to_string())?;
    let mut made = 0usize;
    for r in &parsed.rows {
        let Some(name) = r.category.as_deref().map(str::trim).filter(|n| !n.is_empty()) else { continue };
        if r.r#type == TxType::Transfer {
            continue;
        }
        let kind = if r.r#type == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if categories.iter().any(|c| c.kind == kind && c.name.trim().to_lowercase() == name.to_lowercase()) {
            continue;
        }
        add_category(store, name, kind, ((made + 3) % 12) as i64)?;
        made += 1;
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
    let rows: Vec<(relay_money::csv::CsvRow, Option<i64>)> = parsed.rows.iter().map(|r| (r.clone(), category_id(&r.category, r.r#type))).collect();
    let errors = parsed.errors.len();
    let result = store.ledger.atomically(|l| {
        let mut by_name: Vec<(String, i64)> = l.accounts()?.into_iter().map(|a| (a.name.to_lowercase(), a.id)).collect();
        let mut account = |l: &mut Ledger, name: &str| -> Result<i64, LedgerError> {
            if let Some((_, id)) = by_name.iter().find(|(n, _)| *n == name.to_lowercase()) {
                return Ok(*id);
            }
            let id = l.account_add(name, AccountType::Chequing, 0, None, "")?.id;
            by_name.push((name.to_lowercase(), id));
            Ok(id)
        };
        let (mut added, mut same_account) = (0usize, 0usize);
        for (r, category) in &rows {
            let from = account(l, &r.account)?;
            let to = match &r.to_account {
                Some(n) if r.r#type == TxType::Transfer => Some(account(l, n)?),
                _ => None,
            };
            if r.r#type == TxType::Transfer && to == Some(from) {
                same_account += 1;
                continue;
            }
            l.tx_add(&TxInput { r#type: r.r#type, amount: r.amount, date: r.date.to_string(), account_id: from, to_account_id: to, category_id: *category, note: Some(r.note.clone()) })?;
            added += 1;
        }
        Ok::<(usize, usize), LedgerError>((added, same_account))
    });
    match result {
        Ok((n, same)) => {
            let skipped = errors + same;
            Ok(if skipped == 0 { format!("Imported {n} entries") } else { format!("Imported {n} entries, skipped {skipped}") })
        }
        Err(e) => Err(format!("The import did not finish. Nothing was added. {e}")),
    }
}

/// A category, written as relay-money writes its own: a fresh uid, stamped now, last in order.
fn add_category(store: &Store, name: &str, kind: CategoryKind, color: i64) -> Result<(), String> {
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
    .map(|_| ())
    .map_err(|e| e.to_string())
}
