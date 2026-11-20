//! Categories' readings and writes (Kotlin: CategoriesLogic.kt, CategoryEditViewModel.kt and the
//! category half of LedgerRepository.kt).
//!
//! relay-money reads categories (`categories`) but has no call to add, edit or delete one, so those
//! are written here through the store's second connection, as PORTING.md says, mirroring
//! relay-money's schema (`ledger.rs`): a new row gets a `uid` and `updated_at` from
//! [`Store::stamp`]; an edit restamps `updated_at` as relay-money does (now, or one past a stamp
//! ahead of this clock); a delete sets `deleted = 1`; `seq` is left to the triggers.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::copy;
use relay_money::model::{CategoryKind, TxType};
use relay_money::views::{CategoryView, TxQuery};
use rusqlite::params;

use crate::model::{Result, Store};

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// How many hues the category palette has.
pub const PALETTE_SIZE: i64 = crate::theme::CATEGORY_PAIRS.len() as i64;

/// A kind as the lens names it.
pub fn kind_label(kind: CategoryKind) -> &'static str {
    if kind == CategoryKind::Income { "Income" } else { "Spending" }
}

pub fn kind_name(kind: CategoryKind) -> &'static str {
    if kind == CategoryKind::Income { "INCOME" } else { "EXPENSE" }
}

fn tx_type(kind: CategoryKind) -> TxType {
    if kind == CategoryKind::Income { TxType::Income } else { TxType::Expense }
}

/// One category row: what it is, how many entries it holds, and this period's money in it.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryLine {
    pub id: i64,
    pub name: String,
    pub kind: CategoryKind,
    pub icon: String,
    pub color: i64,
    pub entries: i64,
    pub period_total: i64,
    /// Its share of the kind's period total, in whole percent.
    pub percent: i64,
    pub archived: bool,
}

/// One slice of the period's money by category; `color` none is the uncategorised remainder.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub name: String,
    pub color: Option<i64>,
    pub total: i64,
}

/// Everything one lens (spending or income) draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KindSummary {
    pub active: Vec<CategoryLine>,
    pub archived: Vec<CategoryLine>,
    /// This period's total for the kind, uncategorised entries included.
    pub period_total: i64,
    /// This period by category, largest first.
    pub segments: Vec<Segment>,
    /// Active categories with money in them this period.
    pub used_this_period: i64,
    /// The active category holding the most entries.
    pub most_used: Option<String>,
}

/// Both lenses and the period they read.
#[derive(Clone, Debug, Default)]
pub struct CategoriesState {
    pub spending: KindSummary,
    pub income: KindSummary,
    /// "October", or "Oct 15 – Nov 14".
    pub period: String,
}

impl CategoriesState {
    pub fn of(&self, kind: CategoryKind) -> &KindSummary {
        if kind == CategoryKind::Income { &self.income } else { &self.spending }
    }
}

/// How many entries each category holds, ever.
pub fn entry_counts(store: &Store) -> Result<HashMap<i64, i64>> {
    let db = store.sql()?;
    let mut st = db
        .prepare_cached("SELECT category_id, COUNT(*) FROM transactions WHERE deleted = 0 AND category_id IS NOT NULL GROUP BY category_id")
        .map_err(err)?;
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)
}

/// This period's money by (type of entry, category); `None` is uncategorised.
pub type Totals = HashMap<(TxType, Option<i64>), i64>;

/// This period's totals by category for one type of entry (`None`: uncategorised), and the period.
pub fn period_totals(store: &Store, today: Date) -> Result<(Totals, String)> {
    let page = store.ledger.tx_list(&TxQuery { limit: Some(100_000), ..Default::default() }, today).map_err(err)?;
    let mut out = HashMap::new();
    for t in page.transactions.iter().filter(|t| t.r#type != TxType::Transfer) {
        *out.entry((t.r#type, t.category_id)).or_insert(0) += t.amount;
    }
    Ok((out, crate::model::period_name(&page.period.start, &page.period.end_exclusive)))
}

/// CategoriesLogic.kt `summarize`.
pub fn summarize(kind: CategoryKind, categories: &[CategoryView], counts: &HashMap<i64, i64>, totals: &Totals) -> KindSummary {
    let ty = tx_type(kind);
    let period_total: i64 = totals.iter().filter(|((t, _), _)| *t == ty).map(|(_, v)| *v).sum();
    let lines: Vec<CategoryLine> = categories
        .iter()
        .filter(|c| c.kind == kind)
        .map(|c| {
            let total = totals.get(&(ty, Some(c.id))).copied().unwrap_or(0);
            CategoryLine {
                id: c.id,
                name: c.name.clone(),
                kind,
                icon: c.icon.clone(),
                color: c.color,
                entries: counts.get(&c.id).copied().unwrap_or(0),
                period_total: total,
                percent: if period_total > 0 { (total as f64 * 100.0 / period_total as f64).round() as i64 } else { 0 },
                archived: c.archived,
            }
        })
        .collect();
    let filed: i64 = lines.iter().map(|l| l.period_total).sum();
    let mut by_total: Vec<&CategoryLine> = lines.iter().filter(|l| l.period_total > 0).collect();
    by_total.sort_by_key(|l| std::cmp::Reverse(l.period_total));
    let mut segments: Vec<Segment> = by_total.iter().map(|l| Segment { name: l.name.clone(), color: Some(l.color), total: l.period_total }).collect();
    if period_total - filed > 0 {
        segments.push(Segment { name: "Uncategorized".into(), color: None, total: period_total - filed });
    }
    let (archived, active): (Vec<CategoryLine>, Vec<CategoryLine>) = lines.into_iter().partition(|l| l.archived);
    // The most entries wins; on a tie, the first in the list, as Kotlin's maxByOrNull keeps.
    let mut most: Option<&CategoryLine> = None;
    for l in active.iter().filter(|l| l.entries > 0) {
        if most.is_none_or(|m| l.entries > m.entries) {
            most = Some(l);
        }
    }
    KindSummary {
        most_used: most.map(|l| l.name.clone()),
        used_this_period: active.iter().filter(|l| l.period_total > 0).count() as i64,
        active,
        archived,
        period_total,
        segments,
    }
}

pub fn read_state(store: &Store, today: Date) -> Result<CategoriesState> {
    let categories = store.ledger.categories().map_err(|e| e.to_string())?;
    let counts = entry_counts(store)?;
    let (totals, period) = period_totals(store, today)?;
    Ok(CategoriesState {
        spending: summarize(CategoryKind::Expense, &categories, &counts, &totals),
        income: summarize(CategoryKind::Income, &categories, &counts, &totals),
        period,
    })
}

/// The line under a category's figure: "4 entries · 37% of this month".
pub fn tile_line(l: &CategoryLine) -> String {
    let n = copy::plural_as(l.entries, "entry", "entries");
    if l.percent > 0 { format!("{n} · {}% of this month", l.percent) } else { n }
}

// The editor

pub const MISSING_NAME: &str = "Give the category a name";

/// Why `name` cannot be saved as a `kind` category, or none. Names are unique within a kind,
/// compared without case, archived categories included.
pub fn name_problem(name: &str, kind: CategoryKind, self_id: Option<i64>, all: &[CategoryView]) -> Option<String> {
    let clean = name.trim();
    if clean.is_empty() {
        return Some(MISSING_NAME.into());
    }
    let clash = all.iter().find(|c| c.kind == kind && Some(c.id) != self_id && c.name.trim().to_lowercase() == clean.to_lowercase())?;
    let what = kind_label(kind).to_lowercase();
    Some(if clash.archived {
        format!("An archived {what} category is already called {}", clash.name)
    } else {
        format!("You already have a {what} category called {}", clash.name)
    })
}

/// A new category's first hue: the first its kind does not use yet.
pub fn suggest_color(used: &[i64]) -> i64 {
    (0..PALETTE_SIZE).find(|i| !used.contains(i)).unwrap_or(used.len() as i64 % PALETTE_SIZE)
}

/// Adds a category at the end of its kind. Returns its id.
pub fn add(store: &Store, name: &str, kind: CategoryKind, icon: &str, color: i64) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(MISSING_NAME.into());
    }
    let db = store.sql()?;
    let order: i64 = db
        .prepare_cached("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE kind = ?1")
        .map_err(err)?
        .query_row([kind_name(kind)], |r| r.get(0))
        .map_err(err)?;
    let (uid, now) = Store::stamp();
    db.prepare_cached("INSERT INTO categories (uid, name, kind, color, icon, archived, sort_order, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)")
        .map_err(err)?
        .execute(params![uid, name, kind_name(kind), color.rem_euclid(PALETTE_SIZE), icon, order, now])
        .map_err(err)?;
    Ok(db.last_insert_rowid())
}

/// Saves a category's name, icon, colour and whether it is archived. Nothing changed, nothing is
/// written, so sync carries no empty edit.
pub fn update(store: &Store, id: i64, name: &str, icon: &str, color: i64, archived: bool) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(MISSING_NAME.into());
    }
    let (_, now) = Store::stamp();
    let n = store
        .sql()?
        .prepare_cached(
            "UPDATE categories SET name = ?2, icon = ?3, color = ?4, archived = ?5, updated_at = MAX(?6, MIN(updated_at, 4000000000000000) + 1)
             WHERE id = ?1 AND deleted = 0 AND (name != ?2 OR icon != ?3 OR color != ?4 OR archived != ?5)",
        )
        .map_err(err)?
        .execute(params![id, name, icon, color.rem_euclid(PALETTE_SIZE), archived, now])
        .map_err(err)?;
    if n == 0 && !exists(store, id)? {
        return Err("That category is gone".into());
    }
    Ok(())
}

fn exists(store: &Store, id: i64) -> Result<bool> {
    store.sql()?.prepare_cached("SELECT 1 FROM categories WHERE id = ?1 AND deleted = 0").map_err(err)?.exists([id]).map_err(err)
}

/// What a category's delete touches: entries, bills, and the budget set on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CategoryUse {
    pub entries: i64,
    pub bills: i64,
    pub budget: Option<i64>,
}

impl CategoryUse {
    /// Entries or bills filed under it, which the delete asks where to move.
    pub fn has_movable(&self) -> bool {
        self.entries > 0 || self.bills > 0
    }
}

pub fn category_use(store: &Store, id: i64) -> Result<CategoryUse> {
    let db = store.sql()?;
    let count = |sql: &str| -> Result<i64> { db.prepare_cached(sql).map_err(err)?.query_row([id], |r| r.get(0)).map_err(err) };
    Ok(CategoryUse {
        entries: count("SELECT COUNT(*) FROM transactions WHERE deleted = 0 AND category_id = ?1")?,
        bills: count("SELECT COUNT(*) FROM recurring WHERE deleted = 0 AND category_id = ?1")?,
        budget: store.ledger.budget(Some(id)).map_err(err)?,
    })
}

/// Deletes a category in one transaction. Its entries and bills move to `move_to` when given, or
/// lose their category (as the Kotlin app's foreign key leaves them); its budget goes with it.
pub fn delete(store: &Store, id: i64, move_to: Option<i64>) -> Result<()> {
    let (_, now) = Store::stamp();
    let db = store.sql()?;
    let tx = db.unchecked_transaction().map_err(err)?;
    let restamp = "updated_at = MAX(?3, MIN(updated_at, 4000000000000000) + 1)";
    let target = move_to.filter(|t| *t != id);
    if let Some(t) = target {
        let ok: bool = tx.prepare("SELECT 1 FROM categories WHERE id = ?1 AND deleted = 0").map_err(err)?.exists([t]).map_err(err)?;
        if !ok {
            return Err("The category to move them to is gone".into());
        }
    }
    for table in ["transactions", "recurring"] {
        tx.execute(&format!("UPDATE {table} SET category_id = ?2, {restamp} WHERE deleted = 0 AND category_id = ?1"), params![id, target, now]).map_err(err)?;
    }
    let gone = "deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)";
    tx.execute(&format!("UPDATE budgets SET {gone} WHERE deleted = 0 AND category_id = ?1"), params![id, now]).map_err(err)?;
    let n = tx.execute(&format!("UPDATE categories SET {gone} WHERE deleted = 0 AND id = ?1"), params![id, now]).map_err(err)?;
    if n == 0 {
        return Err("That category is already gone".into());
    }
    tx.commit().map_err(err)
}

/// Undo for a category deleted with nothing filed under it: the row comes back, with its budget.
pub fn restore(store: &Store, id: i64, budget: Option<i64>) -> Result<()> {
    let (_, now) = Store::stamp();
    let db = store.sql()?;
    let tx = db.unchecked_transaction().map_err(err)?;
    tx.execute("UPDATE categories SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1", params![id, now])
        .map_err(err)?;
    if let Some(b) = budget.filter(|b| *b > 0) {
        tx.execute(
            "UPDATE budgets SET deleted = 0, amount = ?3, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE category_id = ?1",
            params![id, now, b],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)
}

/// What a deleted category's move covers, as one phrase ("3 entries and 1 bill").
pub fn moving_phrase(u: &CategoryUse) -> Option<String> {
    let mut parts = Vec::new();
    if u.entries > 0 {
        parts.push(copy::plural_as(u.entries, "entry", "entries"));
    }
    if u.bills > 0 {
        parts.push(copy::plural(u.bills, "bill"));
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}

/// The move sheet's heading: "Move its 3 entries and 1 bill to".
pub fn move_heading(u: &CategoryUse) -> String {
    format!("Move its {} to", moving_phrase(u).unwrap_or_default())
}

/// The line under the heading: what goes for good, the budget by its amount.
pub fn move_note(name: &str, budget: Option<&str>) -> String {
    match budget {
        Some(b) => format!("{name} is deleted, with its {b} budget. This cannot be undone."),
        None => format!("{name} is deleted. This cannot be undone."),
    }
}

pub fn leave_title(u: &CategoryUse) -> &'static str {
    if u.entries + u.bills == 1 { "Leave it uncategorized" } else { "Leave them uncategorized" }
}

pub fn leave_subtitle(u: &CategoryUse) -> &'static str {
    if u.entries > 0 && u.bills > 0 {
        "Entries stay in the ledger and bills keep posting, with no category"
    } else if u.bills > 0 {
        if u.bills == 1 { "It keeps posting, with no category" } else { "They keep posting, with no category" }
    } else {
        "They stay in the ledger with no category"
    }
}

/// The toast after a delete.
pub fn deleted_line(name: &str, u: &CategoryUse, budget: Option<&str>, moved_to: Option<&str>) -> String {
    let head = match budget {
        Some(b) => format!("{name} deleted with its {b} budget"),
        None => format!("{name} deleted"),
    };
    match (moving_phrase(u), moved_to) {
        (None, _) => head,
        (Some(m), Some(to)) => format!("{head} · {m} moved to {to}"),
        (Some(m), None) => format!("{head} · {m} left uncategorized"),
    }
}

/// The picker's sections, in reading order (Kotlin Icons.kt `CategoryIcons.groups`).
pub const ICON_GROUPS: &[(&str, &[&str])] = &[
    (
        "Food and drink",
        &["cart", "dining", "coffee", "tea", "pizza", "fastfood", "lunch", "ramen", "bakery", "icecream", "kitchen", "bar", "beer", "wine", "liquor"],
    ),
    ("Getting around", &["transport", "train", "subway", "car", "taxi", "fuel", "ev", "parking", "carrepair", "bike", "scooter", "boat", "flight"]),
    (
        "Home and bills",
        &["home", "apartment", "bolt", "water", "heat", "wifi", "phone", "tv", "insurance", "furniture", "repairs", "cleaning", "laundry", "garden", "repeat"],
    ),
    ("Shopping", &["bag", "store", "clothes", "computer", "headphones", "camera", "watch", "jewelry", "toys", "flowers", "book", "gift"]),
    ("Health and care", &["health", "medical", "hospital", "pharmacy", "medication", "eyes", "therapy", "wellness", "sport", "beauty", "haircut"]),
    (
        "Going out",
        &["ticket", "movie", "music", "theatre", "party", "games", "casino", "art", "outdoors", "hiking", "soccer", "pool", "hotel", "luggage", "beach"],
    ),
    ("People", &["child", "baby", "school", "family", "elderly", "pets", "charity", "church", "birthday"]),
    (
        "Money",
        &["work", "business", "spark", "refund", "sale", "savings", "invest", "crypto", "bank", "card", "cash", "wallet", "atm", "fees", "tax", "loan", "legal"],
    ),
    ("Other", &["dots", "star", "idea", "online", "cloud", "delivery", "mail"]),
];
