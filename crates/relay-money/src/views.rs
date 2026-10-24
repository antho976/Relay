//! What the ledger answers with: the shapes the `money.*` bus ops return. Apart from
//! [`crate::ledger`] so the bus contract does not pull in SQLite.

use crate::model::{AccountType, ActivityType, CategoryKind, GoalKind, Registration, TxType};
use crate::pace::{PaceReading, PaceStatus};
use crate::period::BudgetPeriod;
use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use crate::invest::Portfolio;
pub use crate::wealthsimple::{Skipped, WsKind};

/// The ledger's own settings, carried in a backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneySettings")]
pub struct Settings {
    pub currency: String,
    pub month_start_day: i64,
    pub week_starts_monday: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyPeriod")]
pub struct PeriodView {
    pub start: String,
    pub end_exclusive: String,
    pub days: i64,
    pub days_left: i64,
}

impl PeriodView {
    pub fn of(p: &BudgetPeriod, today: Date) -> Self {
        PeriodView { start: p.start.to_string(), end_exclusive: p.end_exclusive.to_string(), days: p.days(), days_left: p.days_left(today) }
    }
}

/// One entry as a client draws it, its account and category names joined in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyTx")]
pub struct Tx {
    pub id: i64,
    #[schemars(extend("x-unit" = "omit"))]
    pub uid: String,
    pub r#type: TxType,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
    pub date: String,
    pub account_id: i64,
    pub account: String,
    pub to_account_id: Option<i64>,
    pub to_account: Option<String>,
    pub category_id: Option<i64>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub color: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyAccount")]
pub struct AccountView {
    pub id: i64,
    pub name: String,
    pub r#type: AccountType,
    #[schemars(extend("x-unit" = "minor"))]
    pub balance: i64,
    pub archived: bool,
    /// An investment account's tax wrapper; `None` on every other account.
    pub registration: Option<Registration>,
    pub institution: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyCategory")]
pub struct CategoryView {
    pub id: i64,
    pub name: String,
    pub kind: CategoryKind,
    #[schemars(extend("x-unit" = "omit"))]
    pub icon: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub color: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyEnvelope")]
pub struct EnvelopeView {
    pub category_id: i64,
    pub name: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub icon: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub color: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub budget: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub spent: i64,
    /// What is still to spend: `budget` less `spent`, below zero once it is over budget.
    #[schemars(extend("x-unit" = "minor"))]
    pub left: i64,
    /// Above zero when spending runs ahead of an even pace through the period.
    #[schemars(extend("x-unit" = "minor", "x-agent-name" = "over_pace_by"))]
    pub pace_delta: i64,
    pub status: PaceStatus,
    /// Where it stands in words, to the cent: "$87.60 over pace, $118.85 left", "$7.99 over budget".
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyBill")]
pub struct BillView {
    pub id: i64,
    pub name: String,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
    pub r#type: TxType,
    pub next_date: String,
    pub days_until: i64,
    pub due_line: String,
    pub auto_post: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyGoal")]
pub struct GoalView {
    pub id: i64,
    pub name: String,
    pub kind: GoalKind,
    #[schemars(extend("x-unit" = "minor"))]
    pub target: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub saved: i64,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyLines")]
pub struct Lines {
    pub margin: String,
    pub pace: String,
    pub versus_last: String,
}

/// The Home reading (docs/MONEY.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneySummary")]
pub struct Summary {
    pub currency: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub fraction_digits: u32,
    pub month_start_day: i64,
    /// No accounts and no entries: draw the honest empty state.
    pub empty: bool,
    pub period: PeriodView,
    pub pace: PaceReading,
    #[schemars(extend("x-unit" = "minor"))]
    pub income: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub spent: i64,
    pub lines: Lines,
    pub budgets: Vec<EnvelopeView>,
    pub accounts: Vec<AccountView>,
    #[schemars(extend("x-unit" = "minor"))]
    pub net_worth: i64,
    pub bills: Vec<BillView>,
    pub goals: Vec<GoalView>,
    /// The latest few entries, for Home. Left out for a model, which answered "the biggest expense"
    /// from these few instead of the period's entries.
    #[schemars(extend("x-unit" = "omit"))]
    pub recent: Vec<Tx>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyLists")]
pub struct Lists {
    pub currency: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub fraction_digits: u32,
    /// Every unarchived account's balance added up, as money.summary's `net_worth`.
    #[schemars(extend("x-unit" = "minor"))]
    pub net_worth: i64,
    pub accounts: Vec<AccountView>,
    pub categories: Vec<CategoryView>,
    /// Phones that sync with this ledger, the latest first.
    #[schemars(extend("x-unit" = "omit"))]
    pub devices: Vec<DeviceView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyDevice")]
pub struct DeviceView {
    pub name: String,
    /// When it last synced (RFC 3339).
    pub last_sync: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyTxQuery")]
pub struct TxQuery {
    /// 0 is the current budget period, -1 the one before.
    pub period_offset: Option<i64>,
    pub query: Option<String>,
    pub account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyTxPage")]
pub struct TxPage {
    pub period: PeriodView,
    /// The newest matching entries, at most the query's `limit`.
    pub transactions: Vec<Tx>,
    /// What every matching entry in the period took in and spent, listed or not.
    #[schemars(extend("x-unit" = "minor"))]
    pub income: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub spent: i64,
}

/// What a new entry carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyTxInput")]
pub struct TxInput {
    pub r#type: TxType,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
    pub date: String,
    pub account_id: i64,
    pub to_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub note: Option<String>,
}

/// What an edit changes; absent fields keep their value. A transfer's `to_account_id` and an
/// entry's `category_id` are cleared by the type they no longer fit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyTxPatch")]
pub struct TxPatch {
    pub r#type: Option<TxType>,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: Option<i64>,
    pub date: Option<String>,
    pub account_id: Option<i64>,
    pub to_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub note: Option<String>,
    /// True takes the entry's category off (an absent `category_id` keeps it): what an Undo needs
    /// to put back an entry that had none. Ignored when `category_id` is given.
    pub clear_category: Option<bool>,
}

/// What an account edit changes; absent fields keep their value. A registration is kept only on
/// an investment account.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyAccountPatch")]
pub struct AccountPatch {
    pub name: Option<String>,
    pub registration: Option<Registration>,
    pub institution: Option<String>,
    pub archived: Option<bool>,
}

/// One investment activity as a client draws it (docs/INVESTMENTS.md), its account and security
/// joined in. `quantity` is at 1e-8 of a unit; `amount` and `fee` are minor units of `currency`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyActivity")]
pub struct ActivityView {
    pub id: i64,
    #[schemars(extend("x-unit" = "omit"))]
    pub uid: String,
    pub account_id: i64,
    pub account: String,
    pub security_id: Option<i64>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub r#type: ActivityType,
    pub date: String,
    #[schemars(extend("x-unit" = "e8"))]
    pub quantity: i64,
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub amount: i64,
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub fee: i64,
    pub currency: String,
    #[schemars(extend("x-unit" = "minor:to_currency"))]
    pub to_amount: Option<i64>,
    pub to_currency: Option<String>,
    pub note: String,
    /// `MANUAL` or `WEALTHSIMPLE`.
    pub source: String,
}

/// What a new activity carries. `symbol` finds or makes the security `sec:<SYMBOL>`; `currency`
/// is the ledger's when absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyActivityInput")]
pub struct ActivityInput {
    pub account_id: i64,
    pub r#type: ActivityType,
    pub date: String,
    pub symbol: Option<String>,
    pub currency: Option<String>,
    #[schemars(extend("x-unit" = "e8"))]
    pub quantity: Option<i64>,
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub amount: i64,
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub fee: Option<i64>,
    pub note: Option<String>,
    #[schemars(extend("x-unit" = "minor:to_currency"))]
    pub to_amount: Option<i64>,
    pub to_currency: Option<String>,
}

/// Which activities to list, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyActivityQuery")]
pub struct ActivityQuery {
    pub account_id: Option<i64>,
    pub r#type: Option<ActivityType>,
    /// `YYYY-MM-DD`: activities on or after it.
    pub since: Option<String>,
    pub limit: Option<u32>,
}

/// An account a Wealthsimple file names, and the Tally account already holding its number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyImportPreviewAccount")]
pub struct ImportPreviewAccount {
    pub number: String,
    pub name: String,
    pub registration: Registration,
    pub account_id: Option<i64>,
    pub rows: usize,
}

/// What importing a Wealthsimple file would do, before it does it. `new` and `duplicates` count
/// its holdings and activities: a duplicate is a line already imported (or imported and deleted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyImportPreview")]
pub struct ImportPreview {
    pub kind: WsKind,
    pub as_of: Option<String>,
    pub accounts: Vec<ImportPreviewAccount>,
    pub holdings: usize,
    pub activities: usize,
    pub new: usize,
    pub duplicates: usize,
    pub skipped: Vec<Skipped>,
}

/// Where one Wealthsimple account goes: `account_id`, or a new account when it has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyImportAccount")]
pub struct ImportAccount {
    pub number: String,
    pub account_id: Option<i64>,
}

/// What an import wrote. `duplicates` are lines it left out because they were already in (or
/// deleted); `skipped` lines it could not read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyImportResult")]
pub struct ImportResult {
    pub kind: WsKind,
    pub accounts_created: usize,
    pub securities: usize,
    pub holdings: usize,
    pub activities: usize,
    pub duplicates: usize,
    pub prices: usize,
    pub values: usize,
    pub skipped: usize,
}

/// One row's change on the sync wire (docs/MONEY.md, "Sync"). `row` holds the table's fields in
/// camelCase, references as uids; empty for a tombstone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyChange")]
pub struct Change {
    pub table: String,
    pub uid: String,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub row: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneySyncOut")]
pub struct SyncOut {
    /// Store it and send it back as `since` next time.
    pub cursor: i64,
    /// Store it and send it back as `generation`: it changes when this ledger is restored,
    /// erased or replaced, and a device holding an old one must take the ledger whole.
    pub generation: String,
    /// This ledger's changes the device has not seen.
    pub changes: Vec<Change>,
    /// This ledger was replaced by the device's.
    pub replaced: bool,
    /// The device's changes taken, and those older than what is here or not placeable.
    pub applied: usize,
    pub skipped: usize,
    /// Only when the device asked with a `limit`: `changes` is one page and more are left. Ask
    /// again with `since` set to `cursor` (and the same generation), and apply the pages together
    /// once this is false. Left out when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub more: bool,
}
