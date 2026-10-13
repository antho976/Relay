//! `money.*` — the Money space's ledger (docs/MONEY.md).
//!
//! The ledger is Tally's, kept by the engine in its own SQLite file (`money.db`); its rules and
//! read shapes are `relay_money`'s. Amounts are minor units, dates `YYYY-MM-DD`, enum values
//! Tally's names. Every mutation emits `money.changed`. What replaces or erases the ledger, or
//! changes its currency, is the person's alone (`Actors::UserOnly`): an agent may read and add
//! entries, never wipe them.
//!
//! Investments (docs/INVESTMENTS.md) are `money.invest.*`, `money.fx.*` and `money.value.set`.
//! Units held and prices are integers at 1e-8 (`relay_money::invest::QTY_SCALE`, `PRICE_SCALE`),
//! FX rates units of `quote` per one `base` at 1e-8 (`RATE_SCALE`). What reads a file on the PC or
//! goes out to the network is the person's alone.
use crate::registry::{Actors, OpMeta, Scope};
use crate::{op, Empty};
use relay_money::model::{AccountType, ActivityType, Registration, TxType};
use relay_money::series::{Series, SeriesQuery};
use relay_money::views::{
    AccountView, ActivityView, Change, ImportAccount, ImportPreview, ImportResult, Lists, Portfolio, Settings, Summary, SyncOut, Tx, TxPage,
};

payload!(#[schemars(rename = "MoneySummaryIn")] SummaryIn {
    /// The day to read the month from; today in the engine's time zone when absent.
    pub today: Option<String>,
});
op!(SummaryOp, "money.summary", SummaryIn => Summary,
    OpMeta::query(Scope::Global, 12, "The Money Home reading: what is left this budget period and its pace, budgets, bills, accounts, goals and recent entries. Posts any bill due by today first"));
op!(ListsOp, "money.lists", Empty => Lists, OpMeta::query(Scope::Global, 12, "The ledger's accounts (with balances) and categories, for pickers"));
payload!(#[schemars(rename = "MoneyTxListIn")] TxListIn {
    /// 0 is the current budget period, -1 the one before.
    pub period_offset: Option<i64>,
    /// Matches the note, category or account in any case and alphabet.
    pub query: Option<String>,
    pub account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub limit: Option<u32>,
    pub today: Option<String>,
});
op!(TxList, "money.tx.list", TxListIn => TxPage, OpMeta::query(Scope::Global, 12, "One budget period's entries, newest first"));
op!(SeriesOp, "money.series", SeriesQuery => Series,
    OpMeta::query(Scope::Global, 12, "A chart's numbers: spending, income or net by category, week, day or budget period, over one or more periods"));
payload!(#[schemars(rename = "MoneyTxAddIn")] TxAddIn {
    pub r#type: TxType, #[schemars(extend("x-unit" = "minor"))] pub amount: i64, pub date: String, pub account_id: i64,
    /// Transfers only: the account the money goes to.
    pub to_account_id: Option<i64>,
    /// Expenses and income only.
    pub category_id: Option<i64>,
    pub note: Option<String>,
});
op!(TxAdd, "money.tx.add", TxAddIn => Tx, OpMeta::mutation(Scope::Global, 12, "Add an expense, income or transfer").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyTxUpdateIn")] TxUpdateIn {
    pub id: i64, pub r#type: Option<TxType>, #[schemars(extend("x-unit" = "minor"))] pub amount: Option<i64>, pub date: Option<String>,
    pub account_id: Option<i64>, pub to_account_id: Option<i64>, pub category_id: Option<i64>, pub note: Option<String>,
});
op!(TxUpdate, "money.tx.update", TxUpdateIn => Tx, OpMeta::mutation(Scope::Global, 12, "Change an entry; absent fields keep their value").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyIdIn")] IdIn { pub id: i64 });
op!(TxDelete, "money.tx.delete", IdIn => Empty, OpMeta::mutation(Scope::Global, 12, "Delete an entry (kept as a tombstone for sync)").emits(&["money.changed"]));
op!(TxRestore, "money.tx.restore", IdIn => Tx, OpMeta::mutation(Scope::Global, 12, "Bring back a deleted entry: the Undo of money.tx.delete").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyBudgetSetIn")] BudgetSetIn {
    /// Absent: the overall monthly budget.
    pub category_id: Option<i64>,
    /// Zero or less removes the budget.
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
});
op!(BudgetSet, "money.budget.set", BudgetSetIn => Empty, OpMeta::mutation(Scope::Global, 12, "Set or remove a monthly budget").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyAccountAddIn")] AccountAddIn {
    pub name: String, pub r#type: AccountType, #[schemars(extend("x-unit" = "minor"))] pub opening_balance: Option<i64>,
    /// Investment accounts only: TFSA, RRSP, FHSA…
    pub registration: Option<Registration>,
    /// Where the account is held, e.g. Wealthsimple.
    pub institution: Option<String>,
});
op!(AccountAdd, "money.account.add", AccountAddIn => AccountView,
    OpMeta::mutation(Scope::Global, 12, "Add an account; the first one also seeds Tally's default categories").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyAccountUpdateIn")] AccountUpdateIn {
    pub id: i64, pub name: Option<String>,
    /// Investment accounts only.
    pub registration: Option<Registration>,
    pub institution: Option<String>, pub archived: Option<bool>,
});
op!(AccountUpdate, "money.account.update", AccountUpdateIn => AccountView,
    OpMeta::mutation(Scope::Global, 12, "Rename an account, set its registration or institution, or archive it; absent fields keep their value").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyValueSetIn")] ValueSetIn { pub account_id: i64, pub date: String, #[schemars(extend("x-unit" = "minor"))] pub value: i64 });
op!(ValueSet, "money.value.set", ValueSetIn => AccountView,
    OpMeta::mutation(Scope::Global, 12, "Record what an investment account was worth on a day; a second value the same day replaces the first").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneySettingsSetIn")] SettingsSetIn {
    /// An ISO 4217 code, e.g. CAD.
    pub currency: Option<String>,
    /// The day the budget month starts on, 1 to 28.
    pub month_start_day: Option<i64>,
});
op!(SettingsSet, "money.settings.set", SettingsSetIn => Settings, OpMeta::mutation(Scope::Global, 12, "Change the currency or the day the budget month starts").actors(Actors::UserOnly).emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyPathIn")] PathIn {
    /// An absolute path on the PC.
    pub path: String,
});
result!(#[schemars(rename = "MoneyImportOut")] ImportOut {
    /// `backup`: a Tally backup, which replaced the ledger.
    pub kind: String, pub transactions: usize, pub accounts: usize,
});
op!(Import, "money.import", PathIn => ImportOut,
    OpMeta::mutation(Scope::Global, 12, "Restore a Tally backup (.json): it replaces the whole ledger, as a restore does on the phone").actors(Actors::UserOnly).emits(&["money.changed"]));
result!(#[schemars(rename = "MoneyExportOut")] ExportOut { pub path: String, pub transactions: usize });
op!(Export, "money.export", PathIn => ExportOut, OpMeta::mutation(Scope::Global, 12, "Write the ledger as a Tally backup the phone can restore"));
result!(#[schemars(rename = "MoneySampleOut")] SampleOut { pub transactions: usize });
op!(Sample, "money.sample", Empty => SampleOut,
    OpMeta::mutation(Scope::Global, 12, "Load Tally's sample household, only into an empty ledger; its accounts are named Sample").actors(Actors::UserOnly).emits(&["money.changed"]));
op!(Reset, "money.reset", Empty => Empty, OpMeta::mutation(Scope::Global, 12, "Erase the whole ledger").actors(Actors::UserOnly).emits(&["money.changed"]));

payload!(#[schemars(rename = "MoneySyncIn")] SyncIn {
    /// The phone's name, shown on the PC.
    pub device: String,
    /// The phone's first sync with this PC: erase the PC's ledger and take the phone's.
    pub replace: Option<bool>,
    /// The cursor this PC returned last time; 0 the first time.
    pub since: i64,
    /// The generation this PC returned last time; absent the first time. When the PC's ledger
    /// is no longer that one, the sync is refused with `money.sync_stale`, and the phone takes
    /// the PC's ledger whole (`since: 0`, no generation, no changes).
    pub generation: Option<String>,
    /// The phone's rows changed since its last sync, and its tombstones.
    pub changes: Vec<Change>,
    /// At most this many changes in the answer (1 or more). When more are left, the answer says
    /// `more: true` and its `cursor` is where this page ended: ask again from there, and apply
    /// the pages together once `more` is false. Absent, the whole answer at once.
    pub limit: Option<u32>,
});
op!(Sync, "money.sync", SyncIn => SyncOut,
    OpMeta::mutation(Scope::Global, 12, "Tally's two-way sync (docs/MONEY.md): apply the phone's changes, newest edit winning per row, and answer with the PC's changes since the phone's cursor").actors(Actors::UserOnly).emits(&["money.changed"]));

// ── Investments (docs/INVESTMENTS.md) ─────────────────────────────────────

payload!(#[schemars(rename = "MoneyInvestSummaryIn")] InvestSummaryIn {
    /// The day to value the portfolio on; today in the engine's time zone when absent.
    pub today: Option<String>,
});
op!(InvestSummary, "money.invest.summary", InvestSummaryIn => Portfolio,
    OpMeta::query(Scope::Global, 12, "The investments: value, book, gain and income, each account with its money-weighted return, the allocation, holdings, and the TFSA, RRSP and FHSA room left this year"));
payload!(#[schemars(rename = "MoneyInvestListIn")] InvestListIn {
    pub account_id: Option<i64>, pub r#type: Option<ActivityType>,
    /// `YYYY-MM-DD`: activities on or after it.
    pub since: Option<String>,
    /// 200 when absent.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "MoneyInvestListOut")] InvestListOut { pub activities: Vec<ActivityView> });
op!(InvestList, "money.invest.list", InvestListIn => InvestListOut,
    OpMeta::query(Scope::Global, 12, "Investment activities (buys, sells, dividends, deposits…), newest first"));
payload!(#[schemars(rename = "MoneyInvestAddIn")] InvestAddIn {
    pub account_id: i64, pub r#type: ActivityType, pub date: String,
    /// The security, e.g. XEQT; it is made when the ledger does not know it yet.
    pub symbol: Option<String>,
    /// The ledger's when absent.
    pub currency: Option<String>,
    /// Units at 1e-8: 100000000 is one unit.
    #[schemars(extend("x-unit" = "e8"))]
    pub quantity: Option<i64>,
    /// Minor units of `currency`, never negative: the type says which way the money went.
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub amount: i64,
    #[schemars(extend("x-unit" = "minor:currency"))]
    pub fee: Option<i64>, pub note: Option<String>,
    /// `FX` only: what the money became, in `to_currency`.
    #[schemars(extend("x-unit" = "minor:to_currency"))]
    pub to_amount: Option<i64>, pub to_currency: Option<String>,
});
op!(InvestAdd, "money.invest.add", InvestAddIn => ActivityView,
    OpMeta::mutation(Scope::Global, 12, "Record an investment activity in an investment account: a buy, sell, dividend, deposit, fee…").emits(&["money.changed"]));
op!(InvestDelete, "money.invest.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Delete an investment activity (kept as a tombstone, so an imported line is never imported again)").emits(&["money.changed"]));
op!(InvestRestore, "money.invest.restore", IdIn => ActivityView,
    OpMeta::mutation(Scope::Global, 12, "Bring back a deleted investment activity: the Undo of money.invest.delete").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyInvestPreviewIn")] InvestPreviewIn {
    /// An absolute path on the PC: a Wealthsimple holdings report, activities export or monthly statement (CSV).
    pub path: String,
    /// A statement names no account: the one it would go into, so lines already there count as duplicates.
    pub account_id: Option<i64>,
});
op!(InvestPreview, "money.invest.preview", InvestPreviewIn => ImportPreview,
    OpMeta::query(Scope::Global, 12, "Read a Wealthsimple file without importing it: its kind, its accounts and the Tally accounts already holding them, and how many lines are new").actors(Actors::UserOnly));
payload!(#[schemars(rename = "MoneyInvestImportIn")] InvestImportIn {
    /// An absolute path on the PC.
    pub path: String,
    /// Where each of the file's accounts goes, by Wealthsimple's account number: an `account_id`,
    /// or a new account when it has none. A number not listed goes to the account already holding it.
    pub accounts: Vec<ImportAccount>,
    /// A statement only: the account it belongs to.
    pub account_id: Option<i64>,
});
op!(InvestImport, "money.invest.import", InvestImportIn => ImportResult,
    OpMeta::mutation(Scope::Global, 12, "Import a Wealthsimple file: holdings, activities, prices and the accounts' values. A line already imported, or imported and deleted, is left out").actors(Actors::UserOnly).emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyInvestRoomIn")] InvestRoomIn {
    /// TFSA, RRSP or FHSA.
    pub registration: Registration,
    pub year: i32,
    /// The person's figure from CRA My Account (or the Notice of Assessment for an RRSP), minor units. Zero or less removes it.
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
});
op!(InvestRoom, "money.invest.room", InvestRoomIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Set or remove the CRA contribution room for a registration and year").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyInvestPriceIn")] InvestPriceIn {
    pub symbol: String, pub date: String,
    /// At 1e-8 of the security's currency's major unit: 3812000000 is 38.12.
    #[schemars(extend("x-unit" = "e8"))]
    pub price: i64,
});
op!(InvestPrice, "money.invest.price", InvestPriceIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Record a security's price on a day").emits(&["money.changed"]));
payload!(#[schemars(rename = "MoneyFxSetIn")] FxSetIn {
    /// ISO 4217 codes: `rate` is units of `quote` for one `base`, at 1e-8.
    pub base: String, pub quote: String, pub date: String, #[schemars(extend("x-unit" = "e8"))] pub rate: i64,
});
op!(FxSet, "money.fx.set", FxSetIn => Empty,
    OpMeta::mutation(Scope::Global, 12, "Record an exchange rate on a day").emits(&["money.changed"]));
result!(#[schemars(rename = "MoneyFxFetchOut")] FxFetchOut {
    /// False when a fetch is already on its way.
    pub started: bool,
});
op!(FxFetch, "money.fx.fetch", Empty => FxFetchOut,
    OpMeta::mutation(Scope::Global, 12, "Fetch the US dollar's recent daily rates in Canadian dollars from the Bank of Canada (no key, nothing personal sent) in the background; money.changed follows").actors(Actors::UserOnly).emits(&["money.changed"]));

entries!(
    SummaryOp, ListsOp, TxList, SeriesOp, TxAdd, TxUpdate, TxDelete, TxRestore, BudgetSet, AccountAdd, AccountUpdate, ValueSet, SettingsSet,
    Import, Export, Sample, Reset, Sync, InvestSummary, InvestList, InvestAdd, InvestDelete, InvestRestore, InvestPreview, InvestImport,
    InvestRoom, InvestPrice, FxSet, FxFetch,
);
