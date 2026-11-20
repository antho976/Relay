//! What Entries, one category's or account's entries, and Search read from the ledger, and the
//! pure rules behind them (Kotlin: ActivityViewModel.kt, TransactionsViewModel.kt, LedgerCopy.kt,
//! EntryRow.kt). Every total is counted here, never while drawing.

use std::collections::HashSet;

use jiff::ToSpan;
use relay_money::Date;
use relay_money::copy;
use relay_money::model::{AccountType, CategoryKind, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::pace::PaceReading;
use relay_money::period::BudgetPeriod;
use relay_money::views::{Tx, TxQuery};

use crate::model::{Result, Store};

/// How many entries a search or a category's list shows; the totals still count every match.
pub const SEARCH_LIMIT: usize = 400;

/// The Entries lens: every entry, or one type.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Filter {
    #[default]
    All,
    Spent,
    Income,
    Transfers,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Spent, Filter::Income, Filter::Transfers];
    pub const LABELS: [&'static str; 4] = ["All", "Spent", "Income", "Transfers"];

    pub fn index(self) -> usize {
        Filter::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }

    pub fn admits(self, t: TxType) -> bool {
        match self {
            Filter::All => true,
            Filter::Spent => t == TxType::Expense,
            Filter::Income => t == TxType::Income,
            Filter::Transfers => t == TxType::Transfer,
        }
    }
}

/// One day of the ledger: its entries newest first, and what went out and came in that day.
#[derive(Clone, Debug)]
pub struct DayGroup {
    pub date: Date,
    pub rows: Vec<Tx>,
    pub spent: i64,
    pub income: i64,
}

/// Rows newest first, one group per day, newest day first; each day keeps its rows in order.
pub fn group_by_day(rows: &[Tx]) -> Vec<DayGroup> {
    let mut out: Vec<DayGroup> = Vec::new();
    for t in rows {
        let Ok(date) = t.date.parse::<Date>() else { continue };
        let i = match out.iter().position(|d| d.date == date) {
            Some(i) => i,
            None => {
                out.push(DayGroup { date, rows: Vec::new(), spent: 0, income: 0 });
                out.len() - 1
            }
        };
        let d = &mut out[i];
        match t.r#type {
            TxType::Expense => d.spent += t.amount,
            TxType::Income => d.income += t.amount,
            TxType::Transfer => {}
        }
        d.rows.push(t.clone());
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.date));
    out
}

/// The readings over every entry a filter matches, past the list's cap included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub count: usize,
    pub spent: i64,
    pub income: i64,
    pub active_days: usize,
}

pub fn totals(rows: &[Tx]) -> Totals {
    let mut days: HashSet<&str> = HashSet::new();
    let mut t = Totals::default();
    for r in rows {
        t.count += 1;
        match r.r#type {
            TxType::Expense => t.spent += r.amount,
            TxType::Income => t.income += r.amount,
            TxType::Transfer => {}
        }
        days.insert(r.date.as_str());
    }
    t.active_days = days.len();
    t
}

/// What an entry is called in a row: its note, else "Transfer", else its category.
pub fn entry_name(t: &Tx) -> String {
    if !t.note.trim().is_empty() {
        t.note.trim().to_string()
    } else if t.r#type == TxType::Transfer {
        "Transfer".into()
    } else {
        t.category.clone().unwrap_or_else(|| "Uncategorized".into())
    }
}

/// The line under it: the category (when the note is the title) and the account, or the
/// transfer's way; led by the day when one is given.
pub fn entry_line(t: &Tx, day: Option<&str>) -> String {
    let way = if t.r#type == TxType::Transfer {
        format!("{} to {}", t.account, t.to_account.as_deref().unwrap_or("another account"))
    } else {
        let mut parts = Vec::new();
        if !t.note.trim().is_empty()
            && let Some(c) = &t.category
        {
            parts.push(c.clone());
        }
        if !t.account.is_empty() {
            parts.push(t.account.clone());
        }
        parts.join(" · ")
    };
    [day.map(str::to_string), (!way.is_empty()).then_some(way)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// "7 entries · $350 out · $2,150 in".
pub fn context_line(t: &Totals, fmt: &MoneyFormatter) -> String {
    format!("{} · {} out · {} in", copy::plural_as(t.count as i64, "entry", "entries"), fmt.format_whole(t.spent), fmt.format_whole(t.income))
}

/// "A day" for a period: what was spent over the days read so far (every day of a past one).
pub fn per_day(spent: i64, period: &BudgetPeriod, current: bool, today: Date) -> Option<i64> {
    let days = if current { period.elapsed_days(today) } else { period.days() };
    (days > 0).then(|| spent / days)
}

/// The period a ledger's settings give for today, shifted by `offset`.
pub fn period_at(store: &Store, today: Date, offset: i64) -> BudgetPeriod {
    let start_day = store.ledger.settings().map(|s| s.month_start_day).unwrap_or(1).clamp(1, 28) as i8;
    BudgetPeriod::containing(today, start_day).shift(offset)
}

/// "October", or "Oct 15 – Nov 14" for a period that does not start on the 1st.
pub fn period_title(p: &BudgetPeriod) -> String {
    crate::model::period_name(&p.start.to_string(), &p.end_exclusive.to_string())
}

/// Ids of the entries a bill posted, so their rows carry the repeat glyph.
pub fn bill_posted(store: &Store) -> HashSet<i64> {
    let Ok(db) = store.sql() else { return HashSet::new() };
    let Ok(mut st) = db.prepare_cached("SELECT id FROM transactions WHERE recurring_id IS NOT NULL AND deleted = 0") else { return HashSet::new() };
    st.query_map([], |r| r.get::<_, i64>(0)).map(|rows| rows.flatten().collect()).unwrap_or_default()
}

fn every_day(query: Option<String>, category: Option<i64>, account: Option<i64>) -> TxQuery {
    TxQuery {
        from: Some(jiff::civil::date(1900, 1, 1)),
        to: Some(jiff::civil::date(2999, 12, 31)),
        query: query.filter(|q| !q.trim().is_empty()).map(|q| q.trim().to_string()),
        category_id: category,
        account_id: account,
        limit: Some(100_000),
        ..Default::default()
    }
}

/// A list as the screens draw it: days, their totals, and whether it was cut short.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub days: Vec<DayGroup>,
    pub totals: Totals,
    pub truncated: bool,
    pub repeats: HashSet<i64>,
    /// The categories the whole list holds (before a category lens), the largest first: Entries'
    /// chips.
    pub categories: Vec<(i64, String)>,
}

fn listing(store: &Store, rows: Vec<Tx>, filter: Filter, cap: usize) -> Listing {
    let rows: Vec<Tx> = rows.into_iter().filter(|t| filter.admits(t.r#type)).collect();
    let totals = totals(&rows);
    let truncated = rows.len() > cap;
    Listing { days: group_by_day(&rows[..rows.len().min(cap)]), totals, truncated, repeats: bill_posted(store), categories: Vec::new() }
}

/// The categories rows hold, by what went through each, the largest first; ties by name.
pub fn categories_by_size(rows: &[Tx]) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String, i64)> = Vec::new();
    for t in rows {
        let (Some(id), Some(name)) = (t.category_id, t.category.as_ref()) else { continue };
        match out.iter_mut().find(|(c, _, _)| *c == id) {
            Some(e) => e.2 += t.amount,
            None => out.push((id, name.clone(), t.amount)),
        }
    }
    out.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
    out.into_iter().map(|(id, name, _)| (id, name)).collect()
}

/// One period's entries (Entries).
pub fn period_listing(store: &Store, today: Date, offset: i64, filter: Filter) -> Result<(BudgetPeriod, Listing)> {
    period_listing_in(store, today, offset, filter, None)
}

/// One period's entries, of one category when `category` is given. The listing's categories are
/// the whole period's, so the chips stay put while one is chosen.
pub fn period_listing_in(store: &Store, today: Date, offset: i64, filter: Filter, category: Option<i64>) -> Result<(BudgetPeriod, Listing)> {
    let period = period_at(store, today, offset);
    let q = TxQuery { from: Some(period.start), to: Some(period.last_day()), limit: Some(100_000), ..Default::default() };
    let rows = store.ledger.tx_list(&q, today).map_err(|e| e.to_string())?.transactions;
    let categories = categories_by_size(&rows);
    let rows: Vec<Tx> = match category {
        Some(c) => rows.into_iter().filter(|t| t.category_id == Some(c)).collect(),
        None => rows,
    };
    let mut l = listing(store, rows, filter, usize::MAX);
    l.categories = categories;
    Ok((period, l))
}

/// What the month switcher offers: the periods from this one back a year, as (offset, name).
/// A period in another year than today's carries its year, its month short ("Dec 2025").
pub fn recent_periods(store: &Store, today: Date, count: i64) -> Vec<(i64, String)> {
    (0..count)
        .map(|i| {
            let p = period_at(store, today, -i);
            let name = if p.start.year() != today.year() && p.start.day() == 1 { p.start.strftime("%b %Y").to_string() } else { period_title(&p) };
            (-i, name)
        })
        .collect()
}

/// "November", or "December 2025" for a period in another year.
pub fn period_label(p: &BudgetPeriod, today: Date) -> String {
    let title = period_title(p);
    if p.start.year() != today.year() && p.start.day() == 1 { format!("{title} {}", p.start.year()) } else { title }
}

/// Every month at once, by what was typed (Search).
pub fn search_listing(store: &Store, today: Date, query: &str, filter: Filter) -> Result<Listing> {
    let page = store.ledger.tx_list(&every_day(Some(query.to_string()), None, None), today).map_err(|e| e.to_string())?;
    Ok(listing(store, page.transactions, filter, SEARCH_LIMIT))
}

/// What one category's or one account's page reads at the top.
#[derive(Clone, Debug)]
pub struct Subject {
    pub name: String,
    pub icon: String,
    pub color: Option<i64>,
    pub kind: CategoryKind,
    pub account_type: Option<AccountType>,
    pub balance: i64,
    /// An account given a balance to start from, or valued: only then does it read what it holds.
    pub tracks_balance: bool,
    /// Every entry it holds, all time.
    pub entry_count: usize,
    /// Category: this period's total and count, the period before, and that one by this day.
    pub period_total: i64,
    pub period_count: usize,
    pub last_total: i64,
    pub last_same_day: i64,
    pub budget: Option<i64>,
    pub reading: Option<PaceReading>,
    /// Account: this period's money in and out through it.
    pub money_in: i64,
    pub money_out: i64,
    pub in_count: usize,
    pub out_count: usize,
}

impl Subject {
    pub fn is_account(&self) -> bool {
        self.account_type.is_some()
    }
}

/// In and out for one account: income into it and transfers to it come in; expenses from it and
/// transfers out of it go out.
pub fn account_flows(rows: &[Tx], account: i64) -> (i64, i64, usize, usize) {
    let (mut i, mut o, mut ic, mut oc) = (0, 0, 0, 0);
    for r in rows {
        let incoming = match r.r#type {
            TxType::Income => r.account_id == account,
            TxType::Transfer => r.to_account_id == Some(account),
            TxType::Expense => false,
        };
        let outgoing = match r.r#type {
            TxType::Expense | TxType::Transfer => r.account_id == account,
            TxType::Income => false,
        };
        if incoming {
            i += r.amount;
            ic += 1;
        }
        if outgoing {
            o += r.amount;
            oc += 1;
        }
    }
    (i, o, ic, oc)
}

fn tracks_balance(store: &Store, account: i64) -> bool {
    let Ok(db) = store.sql() else { return false };
    db.query_row(
        "SELECT (SELECT opening_balance FROM accounts WHERE id = ?1) != 0
             OR EXISTS (SELECT 1 FROM account_values WHERE account_id = ?1 AND deleted = 0)",
        [account],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// One category's (or, with no category, one account's) page: what it is and its list. `None`
/// when it is gone.
pub fn subject_listing(store: &Store, today: Date, category: Option<i64>, account: Option<i64>, query: &str) -> Result<Option<(Subject, Listing)>> {
    let err = |e: relay_money::ledger::LedgerError| e.to_string();
    let period = period_at(store, today, 0);
    let in_period = |c: Option<i64>, a: Option<i64>, p: &BudgetPeriod, last: Date| TxQuery {
        from: Some(p.start),
        to: Some(last.max(p.start)),
        category_id: c,
        account_id: a,
        limit: Some(100_000),
        ..Default::default()
    };
    let all = store.ledger.tx_list(&every_day(None, category, account.filter(|_| category.is_none())), today).map_err(err)?.transactions;
    let mut s = match (category, account) {
        (Some(id), _) => {
            let Some(c) = store.ledger.categories().map_err(err)?.into_iter().find(|c| c.id == id) else { return Ok(None) };
            let ty = if c.kind == CategoryKind::Income { TxType::Income } else { TxType::Expense };
            let sum = |rows: &[Tx]| rows.iter().filter(|t| t.r#type == ty).fold((0i64, 0usize), |(s, n), t| (s + t.amount, n + 1));
            let now = store.ledger.tx_list(&in_period(Some(id), None, &period, period.last_day()), today).map_err(err)?.transactions;
            let (total, count) = sum(&now);
            let prev = period.shift(-1);
            let last = store.ledger.tx_list(&in_period(Some(id), None, &prev, prev.last_day()), today).map_err(err)?.transactions;
            let elapsed = period.elapsed_days(today);
            let same = prev.start.checked_add((elapsed - 1).max(0).days()).unwrap_or(prev.start).min(prev.last_day());
            let last_same = last.iter().filter(|t| t.r#type == ty && t.date.parse::<Date>().is_ok_and(|d| d <= same)).map(|t| t.amount).sum();
            let budget = if ty == TxType::Expense { store.ledger.budget(Some(id)).map_err(err)?.filter(|b| *b > 0) } else { None };
            Subject {
                name: c.name,
                icon: c.icon,
                color: Some(c.color),
                kind: c.kind,
                account_type: None,
                balance: 0,
                tracks_balance: false,
                entry_count: 0,
                period_total: total,
                period_count: count,
                last_total: sum(&last).0,
                last_same_day: last_same,
                budget,
                reading: budget.map(|b| PaceReading::new(b, total, period.days(), elapsed)),
                money_in: 0,
                money_out: 0,
                in_count: 0,
                out_count: 0,
            }
        }
        (None, Some(id)) => {
            let Some(a) = store.ledger.accounts().map_err(err)?.into_iter().find(|a| a.id == id) else { return Ok(None) };
            let now = store.ledger.tx_list(&in_period(None, Some(id), &period, period.last_day()), today).map_err(err)?.transactions;
            let (mi, mo, ic, oc) = account_flows(&now, id);
            Subject {
                name: a.name,
                icon: String::new(),
                color: None,
                kind: CategoryKind::Expense,
                account_type: Some(a.r#type),
                balance: a.balance,
                tracks_balance: tracks_balance(store, id),
                entry_count: 0,
                period_total: 0,
                period_count: 0,
                last_total: 0,
                last_same_day: 0,
                budget: None,
                reading: None,
                money_in: mi,
                money_out: mo,
                in_count: ic,
                out_count: oc,
            }
        }
        (None, None) => return Ok(None),
    };
    s.entry_count = all.len();
    let rows = if query.trim().is_empty() {
        all
    } else {
        store.ledger.tx_list(&every_day(Some(query.to_string()), category, account.filter(|_| category.is_none())), today).map_err(err)?.transactions
    };
    Ok(Some((s, listing(store, rows, Filter::All, SEARCH_LIMIT))))
}

/// The day an entry happened, in full: "Today, November 19", "Tuesday, November 17", or with its
/// year when it is not this one ("Monday, June 16, 2025").
pub fn full_date(d: Date, today: Date) -> String {
    let lead = match (today - d).get_days() {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        _ => d.strftime("%A").to_string(),
    };
    if d.year() == today.year() { format!("{lead}, {}", d.strftime("%B %-d")) } else { format!("{lead}, {}", d.strftime("%B %-d, %Y")) }
}

/// An account's kind in words.
pub fn account_type_label(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "Cash",
        AccountType::Chequing => "Chequing",
        AccountType::Savings => "Savings",
        AccountType::Credit => "Credit card",
        AccountType::Investment => "Investment",
    }
}

/// An account's kind as a glyph.
pub fn account_glyph(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "coins",
        AccountType::Chequing => "bank",
        AccountType::Savings => "target",
        AccountType::Credit => "card",
        AccountType::Investment => "trend",
    }
}
