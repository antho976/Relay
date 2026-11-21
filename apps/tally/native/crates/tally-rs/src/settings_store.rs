//! Settings' reads and the writes relay-money has no call for yet: the entry count, the week's
//! first day, the entries as CSV rows.

use relay_money::csv::CsvRow;
use relay_money::model::CategoryKind;
use relay_money::views::TxQuery;

use crate::model::Store;

/// What the Settings rows say about the ledger.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counts {
    pub entries: i64,
    pub accounts: usize,
    pub net: i64,
    pub spending: usize,
    pub income: usize,
    /// The day of the oldest live entry, as the ledger writes it ("2024-03-02").
    pub first: Option<String>,
}

pub fn counts(store: &Store) -> Counts {
    let (entries, first) = match store.sql() {
        Ok(db) => db
            .query_row("SELECT COUNT(*), MIN(date) FROM transactions WHERE deleted = 0", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))
            .unwrap_or((0, None)),
        Err(_) => {
            let all = every_entry(store);
            (all.len() as i64, all.last().map(|t| t.date.clone()))
        }
    };
    let accounts: Vec<_> = store.ledger.accounts().unwrap_or_default().into_iter().filter(|a| !a.archived).collect();
    let categories: Vec<_> = store.ledger.categories().unwrap_or_default().into_iter().filter(|c| !c.archived).collect();
    Counts {
        entries,
        accounts: accounts.len(),
        // The net worth the Accounts page shows, not every balance summed.
        net: crate::plan_logic::worth_total(store, crate::model::today()),
        spending: categories.iter().filter(|c| c.kind == CategoryKind::Expense).count(),
        income: categories.iter().filter(|c| c.kind == CategoryKind::Income).count(),
        first,
    }
}

/// Every live entry, newest first.
pub fn every_entry(store: &Store) -> Vec<relay_money::views::Tx> {
    let q = TxQuery {
        from: Some(relay_money::Date::constant(1900, 1, 1)),
        to: Some(relay_money::Date::constant(9999, 12, 30)),
        limit: Some(u32::MAX),
        ..Default::default()
    };
    store.ledger.tx_list(&q, crate::model::today()).map(|p| p.transactions).unwrap_or_default()
}

/// The entries as Tally's CSV writes them, oldest first.
pub fn csv_rows(store: &Store) -> Vec<CsvRow> {
    let mut rows: Vec<CsvRow> = every_entry(store)
        .into_iter()
        .filter_map(|t| {
            Some(CsvRow {
                date: t.date.parse().ok()?,
                r#type: t.r#type,
                amount: t.amount,
                category: t.category,
                account: t.account,
                to_account: t.to_account,
                note: t.note,
            })
        })
        .collect();
    rows.reverse();
    rows
}

/// The week's first day. relay-money keeps it in `settings` but has no call to change it: written
/// as its own `set_setting` writes, the stamp moved past the stored one so sync carries it.
pub fn set_week_starts_monday(store: &Store, monday: bool) -> Result<(), String> {
    let db = store.sql()?;
    let (_, now) = Store::stamp();
    db.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES ('week_starts_monday', ?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = MAX(excluded.updated_at, MIN(settings.updated_at, 4000000000000000) + 1)",
        rusqlite::params![if monday { "1" } else { "0" }, now],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}
