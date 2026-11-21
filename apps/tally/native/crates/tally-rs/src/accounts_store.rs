//! Accounts' readings and writes (Kotlin: AccountsLogic.kt, AccountEditViewModel.kt and the
//! account half of LedgerRepository.kt).
//!
//! relay-money's `accounts`, `account_add`, `account_update` and `value_set` do most of it. What it
//! has no call for is written here through the store's second connection, as PORTING.md says: an
//! edit that changes an account's type or opening balance (or clears its registration), the
//! account's delete with everything that goes with it, and taking back a recorded value. Each
//! write stamps `updated_at` as relay-money does and never touches `seq`; deletes set `deleted`.
//!
//! The account new entries start in is this phone's own preference ([`DEFAULT_PREF`]), as the
//! Kotlin app keeps it in its settings rather than the ledger. It names the account by its `uid`:
//! ids are this file's own and start again at 1 after a first sync, an erase or a restore, where
//! a stored id would quietly name another account.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::copy;
use relay_money::model::{AccountType, Registration, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::views::{AccountPatch, AccountView, TxQuery};
use rusqlite::{OptionalExtension, params};

use crate::model::{Result, Store};

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The preference naming the account new entries start in, by its uid.
pub const DEFAULT_PREF: &str = "default_account_uid";

/// Where 0.3 kept the default, by the account's id. An id cannot be trusted after a renumbering,
/// so it is never read: the first change to the default drops it, and until then new entries
/// start where they would with no default set.
const OLD_DEFAULT_PREF: &str = "default_account";

/// relay-money's stamp for a row written again: now, or just past the row's own stamp when that
/// is ahead of this clock, so an edit always outranks what it replaces.
const RESTAMP: &str = "updated_at = MAX(?8, MIN(updated_at, 4000000000000000) + 1)";

/// The account new entries start in, if one is set and it is still open: its id in this file.
pub fn default_account(store: &Store) -> Option<i64> {
    let uid = store.pref(DEFAULT_PREF)?;
    let db = store.sql().ok()?;
    db.prepare_cached("SELECT id FROM accounts WHERE uid = ?1 AND deleted = 0 AND archived = 0")
        .ok()?
        .query_row([uid], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

/// Sets (or clears) the default by the account's id, stored as its uid.
pub fn set_default_account(store: &mut Store, id: Option<i64>) -> Result<()> {
    if store.pref(OLD_DEFAULT_PREF).is_some() {
        store.clear_pref(OLD_DEFAULT_PREF)?;
    }
    let uid: Option<String> = match id {
        Some(id) => Some(
            store
                .sql()?
                .prepare_cached("SELECT uid FROM accounts WHERE id = ?1 AND deleted = 0")
                .map_err(err)?
                .query_row([id], |r| r.get(0))
                .optional()
                .map_err(err)?
                .ok_or("That account is gone")?,
        ),
        None => None,
    };
    match uid {
        Some(uid) => store.set_pref(DEFAULT_PREF, &uid),
        None => store.clear_pref(DEFAULT_PREF),
    }
}

/// What [`save`] says to an opening balance past what the ledger keeps.
const TOO_LARGE: &str = crate::plan_store::TOO_LARGE;

// Words

pub fn type_label(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "Cash",
        AccountType::Chequing => "Chequing",
        AccountType::Savings => "Savings",
        AccountType::Credit => "Credit card",
        AccountType::Investment => "Investment",
    }
}

/// The one line under the kinds in the editor.
pub fn type_hint(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "Notes and coins in your wallet",
        AccountType::Chequing => "The everyday bank account",
        AccountType::Savings => "Money set aside, earning or waiting",
        AccountType::Credit => "A card you pay off later",
        AccountType::Investment => "A TFSA, an RRSP or a brokerage account, valued by its Wealthsimple files or by hand",
    }
}

/// Relay's glyph for each kind of account.
pub fn account_glyph(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "coins",
        AccountType::Chequing => "bank",
        AccountType::Savings => "wallet",
        AccountType::Credit => "card",
        AccountType::Investment => "trend",
    }
}

/// Each kind's hue on its tile.
pub fn kind_hue(t: AccountType) -> crate::draw::Rgba {
    use crate::theme::hue;
    match t {
        AccountType::Cash => hue::MINT,
        AccountType::Chequing => hue::SKY,
        AccountType::Savings => hue::SUN,
        AccountType::Credit => hue::VIOLET,
        AccountType::Investment => hue::TEAL,
    }
}

pub const TYPES: [AccountType; 5] = [AccountType::Cash, AccountType::Chequing, AccountType::Savings, AccountType::Credit, AccountType::Investment];

/// The kinds of investment account, in the order the editor offers them.
pub const REGISTRATIONS: [Registration; 8] = [
    Registration::Tfsa,
    Registration::Rrsp,
    Registration::Fhsa,
    Registration::NonRegistered,
    Registration::Resp,
    Registration::Lira,
    Registration::Rrif,
    Registration::Other,
];

/// A balance in words, never a negative figure: "holds $3,184.50", "owes $412.20" on a card,
/// "$50.00 below zero".
pub fn balance_words(t: AccountType, balance: i64, fmt: &MoneyFormatter) -> String {
    if balance >= 0 {
        format!("holds {}", fmt.format(balance))
    } else if t == AccountType::Credit {
        format!("owes {}", fmt.format(-balance))
    } else {
        format!("{} below zero", fmt.format(-balance))
    }
}

// The list

/// An account as the list reads it: relay-money's view, plus what it opened at, the day of its
/// newest recorded value, and how many entries touch it.
#[derive(Clone, Debug)]
pub struct AccountRow {
    pub view: AccountView,
    pub opening: i64,
    pub valued_on: Option<Date>,
    pub entries: i64,
    /// The number the account is known by elsewhere: a Wealthsimple import's account number, or
    /// the bank account's mask when a bank connection made it. Empty when nothing set one.
    pub external_ref: String,
    /// It follows an account at a connected bank (see [`follows_bank`]).
    pub follows_bank: bool,
    /// What its balance says it is worth to net worth today: the balance itself when it is
    /// [`tracked`](Self::tracked), else the reading [`worth_by_entries`] gives one given nowhere
    /// to start.
    pub worth: i64,
}

impl AccountRow {
    /// Tally can say what it holds: it was given somewhere to start. Otherwise a balance built
    /// from spending alone would read as a debt that is not there. A card always can: what it
    /// owes is its spending, so its balance counts from zero, and paying it moves nothing.
    pub fn tracked(&self) -> bool {
        self.opening != 0 || self.valued_on.is_some() || self.view.r#type == AccountType::Credit
    }

    /// It follows an account at a connected bank.
    pub fn connected(&self) -> bool {
        self.follows_bank
    }
}

/// Whether an account follows a bank. A reference alone cannot say: the Wealthsimple import
/// writes its account number there too, and an account picked as "Into" gets none. So it is the
/// server's own word when the Banks page has read it (`followed`, the `into` uids of the accounts
/// it follows), else what only the bank link leaves in the ledger: entries it wrote (uid
/// `plaid:…`), or the short mask it names an account it made by.
pub fn follows_bank(uid: &str, external_ref: &str, bank_entries: bool, followed: Option<&[String]>) -> bool {
    match followed {
        Some(uids) => uids.iter().any(|u| u == uid),
        None => bank_entries || crate::banks_logic::is_bank_mask(external_ref),
    }
}

/// Every account (archived ones after), with what relay-money's view leaves out.
pub fn rows(store: &Store) -> Result<Vec<AccountRow>> {
    let views = store.ledger.accounts().map_err(err)?;
    let db = store.sql()?;
    let mut st = db
        .prepare_cached(
            "SELECT a.id, a.opening_balance,
                    (SELECT MAX(v.date) FROM account_values v WHERE v.account_id = a.id AND v.deleted = 0),
                    (SELECT COUNT(*) FROM transactions t WHERE t.deleted = 0 AND (t.account_id = a.id OR t.to_account_id = a.id)),
                    a.external_ref, a.uid,
                    EXISTS (SELECT 1 FROM transactions t WHERE t.deleted = 0 AND t.uid LIKE 'plaid:%'
                              AND (t.account_id = a.id OR t.to_account_id = a.id))
             FROM accounts a WHERE a.deleted = 0",
        )
        .map_err(err)?;
    type Extra = (i64, Option<String>, i64, String, String, bool);
    let extra: HashMap<i64, Extra> = st
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))))
        .map_err(err)?
        .collect::<std::result::Result<_, _>>()
        .map_err(err)?;
    let followed = crate::banks_logic::followed_pref(store);
    let by_entries = worth_by_entries(store, None)?;
    Ok(views
        .into_iter()
        .map(|view| {
            let (opening, valued, entries, external_ref, uid, bank_entries) = extra.get(&view.id).cloned().unwrap_or_default();
            let follows_bank = follows_bank(&uid, &external_ref, bank_entries, followed.as_deref());
            let mut row = AccountRow { valued_on: valued.and_then(|d| d.parse().ok()), opening, entries, external_ref, follows_bank, worth: 0, view };
            row.worth = if row.tracked() { row.view.balance } else { by_entries.get(&row.view.id).copied().unwrap_or(0) };
            row
        })
        .collect())
}

/// Every entry in the ledger (a transfer once, though it touches two accounts).
pub fn entry_count(store: &Store) -> Result<i64> {
    store.sql()?.prepare_cached("SELECT COUNT(*) FROM transactions WHERE deleted = 0").map_err(err)?.query_row([], |r| r.get(0)).map_err(err)
}

/// What was spent from each account this budget period.
pub fn spent_by_account(store: &Store, today: Date) -> Result<HashMap<i64, i64>> {
    let page = store.ledger.tx_list(&TxQuery { limit: Some(100_000), ..Default::default() }, today).map_err(err)?;
    let mut out = HashMap::new();
    for t in page.transactions.iter().filter(|t| t.r#type == TxType::Expense) {
        *out.entry(t.account_id).or_insert(0) += t.amount;
    }
    Ok(out)
}

/// One account row: what it paid out this period and its share of the period's spending.
#[derive(Clone, Debug)]
pub struct AccountLine {
    pub row: AccountRow,
    pub is_default: bool,
    pub spent: i64,
    /// 0..1 of the period's spending across the active accounts.
    pub spent_share: f32,
}

/// Everything the Accounts list draws, summed once.
#[derive(Clone, Debug, Default)]
pub struct AccountsState {
    pub active: Vec<AccountLine>,
    pub archived: Vec<AccountLine>,
    pub entries: i64,
    pub default_name: Option<String>,
    /// What the active accounts paid out this period: the page's lead.
    pub month_spent: i64,
    /// The active account most of it came from, and its share.
    pub top_spender: Option<(String, f32)>,
    /// The active account holding the most, when it tracks a balance.
    pub largest_tracked: Option<String>,
    /// What the accounts that track a balance hold and owe.
    pub tracked_held: i64,
    pub tracked_owed: i64,
    /// What the accounts are worth now and at the end of last month (filled by [`read_state`]).
    pub worth: Worth,
    /// Each card's next payment from a bill: (day, amount).
    pub payments: HashMap<i64, (Date, i64)>,
}

impl AccountsState {
    /// A transfer needs two places for the money to move between.
    pub fn can_transfer(&self) -> bool {
        self.active.len() >= 2
    }
    pub fn any_tracked(&self) -> bool {
        self.active.iter().any(|l| l.row.tracked())
    }
}

/// AccountsLogic.kt `buildAccountsState`.
pub fn build_state(rows: Vec<AccountRow>, default_id: Option<i64>, entries: i64, spent: &HashMap<i64, i64>) -> AccountsState {
    let (archived, active): (Vec<AccountRow>, Vec<AccountRow>) = rows.into_iter().partition(|r| r.view.archived);
    let month_spent: i64 = active.iter().map(|a| spent.get(&a.view.id).copied().unwrap_or(0)).sum();
    let lines: Vec<AccountLine> = active
        .into_iter()
        .map(|row| {
            let out = spent.get(&row.view.id).copied().unwrap_or(0);
            AccountLine {
                is_default: Some(row.view.id) == default_id,
                spent: out,
                spent_share: if month_spent > 0 { (out as f64 / month_spent as f64) as f32 } else { 0.0 },
                row,
            }
        })
        .collect();
    // Among the accounts that track a balance only: one given nowhere to start may read high on
    // income alone, and must not hide the tracked account that really holds the most.
    let largest = lines.iter().filter(|l| l.row.tracked() && l.row.view.balance > 0).max_by_key(|l| l.row.view.balance);
    let top = lines.iter().filter(|l| l.spent > 0).max_by_key(|l| l.spent);
    let tracked = lines.iter().filter(|l| l.row.tracked());
    let (mut held, mut owed) = (0, 0);
    for l in tracked {
        let b = l.row.view.balance;
        if b > 0 {
            held += b;
        } else {
            owed -= b;
        }
    }
    AccountsState {
        default_name: lines.iter().find(|l| l.is_default).map(|l| l.row.view.name.clone()),
        month_spent,
        top_spender: top.map(|l| (l.row.view.name.clone(), l.spent_share)),
        largest_tracked: largest.map(|l| l.row.view.name.clone()),
        tracked_held: held,
        tracked_owed: owed,
        archived: archived.into_iter().map(|row| AccountLine { is_default: Some(row.view.id) == default_id, spent: 0, spent_share: 0.0, row }).collect(),
        active: lines,
        entries,
        worth: Worth::default(),
        payments: HashMap::new(),
    }
}

/// Everything the list reads, in one go.
pub fn read_state(store: &Store, today: Date) -> Result<AccountsState> {
    let rows = rows(store)?;
    let worth = worth_reading(store, &rows, today)?;
    let mut s = build_state(rows, default_account(store), entry_count(store)?, &spent_by_account(store, today)?);
    s.worth = worth;
    s.payments = payments(store)?;
    Ok(s)
}

/// The line under the lead figure: where most of it came from.
pub fn top_line(s: &AccountsState) -> String {
    match &s.top_spender {
        Some((name, share)) if *share >= 0.999 => format!("All of it from {name}"),
        Some((name, share)) => format!("{}% of it from {name}", (share * 100.0).round() as i64),
        None => "Nothing spent from your accounts yet this month".into(),
    }
}

/// The words under an account's name: its kind, default, the balance when it tracks one, its
/// valuation day.
pub fn caption(line: &AccountLine, fmt: &MoneyFormatter) -> String {
    let a = &line.row.view;
    let mut parts = vec![type_label(a.r#type).to_string()];
    if line.is_default {
        parts.push("default".into());
    }
    if line.row.tracked() {
        parts.push(balance_words(a.r#type, a.balance, fmt));
    }
    if let Some(d) = line.row.valued_on {
        parts.push(format!("valued {}", d.strftime("%b %-d")));
    }
    parts.join(" · ")
}

// The editor

/// A ready-made account for the banks the owner uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Template {
    pub name: &'static str,
    pub kind: AccountType,
    pub registration: Option<Registration>,
}

/// Desjardins and Wealthsimple first, as the owner banks there; cash last.
pub const TEMPLATES: [Template; 9] = [
    Template { name: "Desjardins chequing", kind: AccountType::Chequing, registration: None },
    Template { name: "Desjardins savings", kind: AccountType::Savings, registration: None },
    Template { name: "Desjardins Visa", kind: AccountType::Credit, registration: None },
    Template { name: "Wealthsimple Chequing", kind: AccountType::Chequing, registration: None },
    Template { name: "Wealthsimple TFSA", kind: AccountType::Investment, registration: Some(Registration::Tfsa) },
    Template { name: "Wealthsimple RRSP", kind: AccountType::Investment, registration: Some(Registration::Rrsp) },
    Template { name: "Wealthsimple FHSA", kind: AccountType::Investment, registration: Some(Registration::Fhsa) },
    Template { name: "Wealthsimple credit card", kind: AccountType::Credit, registration: None },
    Template { name: "Cash", kind: AccountType::Cash, registration: None },
];

/// The templates not yet taken: no account by that name exists, in any case.
pub fn free_templates(existing: &[String]) -> Vec<Template> {
    let taken: Vec<String> = existing.iter().map(|n| n.trim().to_lowercase()).collect();
    TEMPLATES.iter().copied().filter(|t| !taken.contains(&t.name.to_lowercase())).collect()
}

/// The editor's fields as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub name: String,
    pub kind: AccountType,
    pub opening_text: String,
    /// The opening balance is money owed, stored below zero.
    pub owe: bool,
    pub is_default: bool,
    pub archived: bool,
    pub registration: Option<Registration>,
}

impl Default for Draft {
    fn default() -> Self {
        Draft { name: String::new(), kind: AccountType::Chequing, opening_text: String::new(), owe: false, is_default: false, archived: false, registration: None }
    }
}

impl Draft {
    /// The owe switch shows for a card, and for any account that opens below zero.
    pub fn shows_owe(&self) -> bool {
        self.kind == AccountType::Credit || self.owe
    }

    /// The typed opening balance, unsigned: blank reads as zero; `None` when it is not an amount.
    pub fn opening(&self, digits: u32) -> Option<i64> {
        if self.opening_text.trim().is_empty() { Some(0) } else { crate::widgets::parse_minor(&self.opening_text, digits) }
    }

    /// As stored: a card you owe on opens below zero.
    pub fn signed_opening(&self, digits: u32) -> Option<i64> {
        self.opening(digits).map(|o| if self.owe { -o } else { o })
    }

    pub fn apply(&mut self, t: &Template) {
        self.name = t.name.to_string();
        self.kind = t.kind;
        self.owe = t.kind == AccountType::Credit;
        self.registration = t.registration;
    }
}

/// Why the draft cannot be saved: (name, opening).
pub fn problems(d: &Draft, digits: u32) -> (Option<&'static str>, Option<&'static str>) {
    (
        d.name.trim().is_empty().then_some("Give the account a name"),
        match d.opening(digits) {
            None => Some("Enter an amount, like 250 or 1,250.50"),
            Some(o) if crate::plan_store::too_large(o) => Some(TOO_LARGE),
            Some(_) => None,
        },
    )
}

/// The institution an investment account is kept under: the stored one, else Wealthsimple when
/// the name says so.
pub fn institution_for(name: &str, stored: &str) -> String {
    if !stored.trim().is_empty() {
        return stored.to_string();
    }
    let lower = name.to_lowercase();
    if lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == "wealthsimple") { "Wealthsimple".into() } else { String::new() }
}

/// An account as stored, for the editor.
#[derive(Clone, Debug)]
pub struct Stored {
    pub row: AccountRow,
    pub institution: String,
}

pub fn stored(store: &Store, id: i64) -> Result<Option<Stored>> {
    Ok(rows(store)?.into_iter().find(|r| r.view.id == id).map(|row| Stored { institution: row.view.institution.clone(), row }))
}

/// The draft for an account as stored, its opening shown as typed.
pub fn draft_of(s: &Stored, default_id: Option<i64>, digits: u32) -> Draft {
    let a = &s.row.view;
    Draft {
        name: a.name.clone(),
        kind: a.r#type,
        opening_text: if s.row.opening == 0 { String::new() } else { crate::widgets::minor_text(s.row.opening.abs(), digits) },
        // A card stored at zero (left blank, or made by a bank link or a CSV) is still money
        // owed: a balance typed in later saves as a debt, not as money held.
        owe: s.row.opening < 0 || (s.row.opening == 0 && a.r#type == AccountType::Credit),
        is_default: default_id == Some(a.id),
        archived: a.archived,
        registration: a.registration,
    }
}

/// Saves the draft: a new account through relay-money's `account_add`, an existing one in one
/// write (relay-money's `account_update` cannot change a type or an opening balance). The default
/// follows the draft. Returns the account's id.
pub fn save(store: &mut Store, id: Option<i64>, d: &Draft, digits: u32) -> Result<i64> {
    let name = d.name.trim();
    if name.is_empty() {
        return Err("Give the account a name".into());
    }
    let opening = d.signed_opening(digits).ok_or("Enter an amount, like 250 or 1,250.50")?;
    if crate::plan_store::too_large(opening) {
        return Err(TOO_LARGE.into());
    }
    let registration = if d.kind == AccountType::Investment { d.registration } else { None };
    // Read before the write: an account archived by it no longer reads as the default.
    let current = default_account(store);
    let id = match id {
        None => {
            let institution = if d.kind == AccountType::Investment { institution_for(name, "") } else { String::new() };
            store.ledger.account_add(name, d.kind, opening, registration, &institution).map_err(err)?.id
        }
        Some(id) => {
            let old = stored(store, id)?.ok_or("That account is gone")?;
            let institution = if d.kind == AccountType::Investment { institution_for(name, &old.institution) } else { old.institution.clone() };
            let (_, now) = Store::stamp();
            store
                .sql()?
                .prepare_cached(&format!(
                    "UPDATE accounts SET name = ?2, type = ?3, opening_balance = ?4, archived = ?5, registration = ?6, institution = ?7, {RESTAMP}
                     WHERE id = ?1 AND deleted = 0 AND (name != ?2 OR type != ?3 OR opening_balance != ?4 OR archived != ?5
                        OR registration IS NOT ?6 OR institution != ?7)"
                ))
                .map_err(err)?
                .execute(params![id, name, type_name(d.kind), opening, d.archived, registration.map(registration_name), institution, now])
                .map_err(err)?;
            id
        }
    };
    if d.is_default && !d.archived {
        if current != Some(id) {
            set_default_account(store, Some(id))?;
        }
    } else if current == Some(id) {
        set_default_account(store, None)?;
    }
    Ok(id)
}

/// A type as the ledger stores it.
pub fn type_name(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "CASH",
        AccountType::Chequing => "CHEQUING",
        AccountType::Savings => "SAVINGS",
        AccountType::Credit => "CREDIT",
        AccountType::Investment => "INVESTMENT",
    }
}

/// A registration as the ledger stores it.
pub fn registration_name(r: Registration) -> &'static str {
    match r {
        Registration::NonRegistered => "NON_REGISTERED",
        Registration::Tfsa => "TFSA",
        Registration::Rrsp => "RRSP",
        Registration::Fhsa => "FHSA",
        Registration::Resp => "RESP",
        Registration::Lira => "LIRA",
        Registration::Rrif => "RRIF",
        Registration::Other => "OTHER",
    }
}

/// Archives (or brings back) an account through relay-money; an archived account is never the
/// default.
pub fn set_archived(store: &mut Store, id: i64, archived: bool) -> Result<()> {
    let was_default = default_account(store) == Some(id);
    store.ledger.account_update(id, &AccountPatch { archived: Some(archived), ..Default::default() }).map_err(err)?;
    if archived && was_default {
        set_default_account(store, None)?;
    }
    Ok(())
}

// Values

/// A value recorded for an investment account.
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub id: i64,
    pub date: String,
    pub value: i64,
}

/// An account's recorded values, newest first.
pub fn values(store: &Store, account_id: i64) -> Result<Vec<Value>> {
    let db = store.sql()?;
    let mut st = db.prepare_cached("SELECT id, date, value FROM account_values WHERE account_id = ?1 AND deleted = 0 ORDER BY date DESC, id DESC").map_err(err)?;
    st.query_map([account_id], |r| Ok(Value { id: r.get(0)?, date: r.get(1)?, value: r.get(2)? }))
        .map_err(err)?
        .collect::<std::result::Result<_, _>>()
        .map_err(err)
}

/// Takes back one recorded value.
pub fn delete_value(store: &Store, value_id: i64) -> Result<()> {
    let (_, now) = Store::stamp();
    store
        .sql()?
        .prepare_cached("UPDATE account_values SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 0")
        .map_err(err)?
        .execute(params![value_id, now])
        .map_err(err)?;
    Ok(())
}

/// Puts back a value `delete_value` took (the Undo on "Value removed").
pub fn restore_value(store: &Store, value_id: i64) -> Result<()> {
    let (_, now) = Store::stamp();
    store
        .sql()?
        .prepare_cached("UPDATE account_values SET deleted = 0, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1 AND deleted = 1")
        .map_err(err)?
        .execute(params![value_id, now])
        .map_err(err)?;
    Ok(())
}

// Delete

/// An account that stays, and how far its balance moves once its transfers with the deleted one go.
#[derive(Clone, Debug, PartialEq)]
pub struct OtherChange {
    pub name: String,
    pub transfers: i64,
    pub change: i64,
}

/// What goes with an account: its entries, its bills, its investment rows (holdings, recorded
/// values, activities) and the other accounts its transfers touch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountUse {
    pub entries: i64,
    pub bills: i64,
    pub holdings: i64,
    pub values: i64,
    pub activities: i64,
    pub others: Vec<OtherChange>,
    /// The open "reach an amount" goals that read this account, by name: `delete` archives them,
    /// as their start and target were this account's, not everything's.
    pub goals: Vec<String>,
}

impl AccountUse {
    /// Nothing `delete` would take with the account, so there is nothing worth archiving for.
    pub fn is_empty(&self) -> bool {
        self.count() == 0 && self.goals.is_empty()
    }

    fn count(&self) -> i64 {
        self.entries + self.bills + self.holdings + self.values + self.activities
    }
}

pub fn account_use(store: &Store, id: i64) -> Result<AccountUse> {
    let db = store.sql()?;
    let entries: i64 = db
        .prepare_cached("SELECT COUNT(*) FROM transactions WHERE deleted = 0 AND (account_id = ?1 OR to_account_id = ?1)")
        .map_err(err)?
        .query_row([id], |r| r.get(0))
        .map_err(err)?;
    let bills: i64 = db
        .prepare_cached("SELECT COUNT(*) FROM recurring WHERE deleted = 0 AND (account_id = ?1 OR to_account_id = ?1)")
        .map_err(err)?
        .query_row([id], |r| r.get(0))
        .map_err(err)?;
    // `delete` takes these too, so the dialog has to say so and offer Archive for them.
    let count = |table: &str| -> Result<i64> {
        db.prepare_cached(&format!("SELECT COUNT(*) FROM {table} WHERE deleted = 0 AND account_id = ?1"))
            .map_err(err)?
            .query_row([id], |r| r.get(0))
            .map_err(err)
    };
    // Holdings are kept as one report per date; what the account holds is its newest report.
    let holdings: i64 = db
        .prepare_cached(
            "SELECT COUNT(*) FROM holdings WHERE deleted = 0 AND account_id = ?1
             AND date = (SELECT MAX(date) FROM holdings WHERE deleted = 0 AND account_id = ?1)",
        )
        .map_err(err)?
        .query_row([id], |r| r.get(0))
        .map_err(err)?;
    let (values, activities) = (count("account_values")?, count("activities")?);
    // The other side of each transfer, as it moved the other account: in is more, out is less.
    let mut st = db
        .prepare_cached(
            "SELECT a.id, a.name, t.date, CASE WHEN t.account_id = a.id THEN -t.amount ELSE t.amount END
             FROM transactions t JOIN accounts a ON a.id = CASE WHEN t.account_id = ?1 THEN t.to_account_id ELSE t.account_id END
             WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND (t.account_id = ?1 OR t.to_account_id = ?1) AND a.id != ?1 AND a.deleted = 0
             ORDER BY a.sort_order, a.id, t.date",
        )
        .map_err(err)?;
    let moves: Vec<Move> =
        st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)?;
    let others = if moves.is_empty() { Vec::new() } else { others_after_delete(store, id, &moves)? };
    let goals = db
        .prepare_cached("SELECT name FROM goals WHERE deleted = 0 AND archived = 0 AND kind = 'BALANCE' AND account_id = ?1 ORDER BY id")
        .map_err(err)?
        .query_map([id], |r| r.get(0))
        .map_err(err)?
        .collect::<std::result::Result<_, _>>()
        .map_err(err)?;
    Ok(AccountUse { entries, bills, holdings, values, activities, others, goals })
}

/// A transfer as it moved another account: that account's id, its name, the day, the amount (in
/// above zero, out below).
type Move = (i64, String, String, i64);

/// How far each other account's worth moves once the transfers with the deleted one go (`moves`:
/// each transfer's other account, its name, day and what it moved that account), by the rules
/// net worth reads it with ([`worth_today`]). One priced from holdings, or given somewhere to
/// start, moves by the transfers after its newest report or recorded value: those already hold
/// the rest. One given nowhere to start is read by its entries again without them, as spending
/// that was paid from money moved in may then read as money Tally was never told about.
fn others_after_delete(store: &Store, deleted: i64, moves: &[Move]) -> Result<Vec<OtherChange>> {
    let today = crate::model::today();
    let rows = rows(store)?;
    let input = store.ledger.portfolio_input(today).ok();
    let now = worth_today(&rows, input.as_ref(), today);
    let mut without: Option<HashMap<i64, i64>> = None;
    let mut out: Vec<OtherChange> = Vec::new();
    let mut i = 0;
    while i < moves.len() {
        let other = moves[i].0;
        let n = moves[i..].iter().take_while(|m| m.0 == other).count();
        let these = &moves[i..i + n];
        i += n;
        let after = |from: Option<Date>| -> i64 {
            -these.iter().filter(|m| from.is_none_or(|f| m.2.parse::<Date>().is_ok_and(|d| d > f))).map(|m| m.3).sum::<i64>()
        };
        let row = rows.iter().find(|r| r.view.id == other);
        let change = match (row, now.get(&other)) {
            (_, Some(&(_, true))) => after(input.as_ref().and_then(|i| crate::invest_logic::report_day(i, other, today))),
            (Some(r), _) if r.tracked() => after(r.valued_on),
            (Some(r), _) => {
                if without.is_none() {
                    without = Some(entries_worth(store, None, Some(deleted))?);
                }
                without.as_ref().and_then(|w| w.get(&other)).copied().unwrap_or(0) - r.worth
            }
            (None, _) => after(None),
        };
        out.push(OtherChange { name: these[0].1.clone(), transfers: n as i64, change });
    }
    Ok(out)
}

/// Deletes an account and, as the Kotlin app's foreign keys do, every entry and bill that touches
/// it and its recorded values and investment rows. A monthly goal that followed it stays and
/// follows everything again (Kotlin `GoalDao.forgetAccount`): "invest 10% every month" still means
/// the same over every investment account. A goal to reach an amount in it is archived instead:
/// its start and target were this account's, and read against net worth it would show as reached
/// or far off for nothing. All in one change, which cannot be undone; the dialog says so and names
/// those goals.
pub fn delete(store: &mut Store, id: i64) -> Result<()> {
    let (_, now) = Store::stamp();
    let was_default = default_account(store) == Some(id);
    {
        let db = store.sql()?;
        let tx = db.unchecked_transaction().map_err(err)?;
        let gone = "deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)";
        for sql in [
            format!("UPDATE transactions SET {gone} WHERE deleted = 0 AND (account_id = ?1 OR to_account_id = ?1)"),
            format!("UPDATE recurring SET {gone} WHERE deleted = 0 AND (account_id = ?1 OR to_account_id = ?1)"),
            format!("UPDATE account_values SET {gone} WHERE deleted = 0 AND account_id = ?1"),
            format!("UPDATE holdings SET {gone} WHERE deleted = 0 AND account_id = ?1"),
            format!("UPDATE activities SET {gone} WHERE deleted = 0 AND account_id = ?1"),
            "UPDATE goals SET archived = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)
             WHERE deleted = 0 AND archived = 0 AND kind = 'BALANCE' AND account_id = ?1"
                .to_string(),
            "UPDATE goals SET account_id = NULL, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)
             WHERE deleted = 0 AND kind IS NOT 'BALANCE' AND account_id = ?1"
                .to_string(),
        ] {
            tx.execute(&sql, params![id, now]).map_err(err)?;
        }
        let n = tx.execute(&format!("UPDATE accounts SET {gone} WHERE deleted = 0 AND id = ?1"), params![id, now]).map_err(err)?;
        if n == 0 {
            return Err("That account is already gone".into());
        }
        tx.commit().map_err(err)?;
    }
    if was_default {
        set_default_account(store, None)?;
    }
    Ok(())
}

/// What goes with a deleted account, as one phrase ("24 entries, 5 bills and 3 holdings").
pub fn goes_with(u: &AccountUse) -> Option<String> {
    let mut parts = Vec::new();
    if u.entries > 0 {
        parts.push(copy::plural_as(u.entries, "entry", "entries"));
    }
    if u.bills > 0 {
        parts.push(copy::plural(u.bills, "bill"));
    }
    if u.holdings > 0 {
        parts.push(copy::plural(u.holdings, "holding"));
    }
    if u.values > 0 {
        parts.push(copy::plural(u.values, "recorded value"));
    }
    if u.activities > 0 {
        parts.push(copy::plural_as(u.activities, "activity", "activities"));
    }
    let last = parts.pop()?;
    Some(if parts.is_empty() { last } else { format!("{} and {last}", parts.join(", ")) })
}

fn other_line(o: &OtherChange, fmt: &MoneyFormatter) -> String {
    let transfers = copy::plural(o.transfers, "transfer");
    let go = if o.transfers == 1 { "goes" } else { "go" };
    match o.change.cmp(&0) {
        std::cmp::Ordering::Greater => format!("{} goes up by {}, since {transfers} between them {go} too.", o.name, fmt.format(o.change)),
        std::cmp::Ordering::Less => format!("{} goes down by {}, since {transfers} between them {go} too.", o.name, fmt.format(-o.change)),
        std::cmp::Ordering::Equal => format!("{transfers} with {} {go} too, which leaves its balance where it is.", o.name),
    }
}

/// The delete dialog's words, with the real counts.
pub fn delete_line(name: &str, u: &AccountUse, fmt: &MoneyFormatter, can_archive: bool) -> String {
    let go = if u.count() == 1 { "goes" } else { "go" };
    let mut s = vec![match goes_with(u) {
        None => format!("Delete {name}? It has no entries and no bills."),
        Some(what) => format!("Delete {name}? Its {what} {go} with it."),
    }];
    // Past two, the other accounts and the goals are counted, not named one by one: a long list
    // would push the warning below off the dialog.
    if u.others.len() > 2 {
        s.push(format!("{} other accounts change too, since the transfers between them and {name} go.", u.others.len()));
    } else {
        s.extend(u.others.iter().map(|o| other_line(o, fmt)));
    }
    if u.goals.len() > 2 {
        s.push(format!("{} goals that read this account are archived.", u.goals.len()));
    } else if let Some(goals) = goals_line(&u.goals) {
        s.push(goals);
    }
    s.push("This cannot be undone.".into());
    if can_archive {
        s.push("Archiving keeps all of it and takes the account out of the pickers.".into());
    }
    s.join(" ")
}

/// The goals the delete archives: "Your goal Trip fund is archived, since it reads this account."
fn goals_line(goals: &[String]) -> Option<String> {
    let (last, rest) = goals.split_last()?;
    Some(if rest.is_empty() {
        format!("Your goal {last} is archived, since it reads this account.")
    } else {
        format!("Your goals {} and {last} are archived, since they read this account.", rest.join(", "))
    })
}

/// The toast after the delete.
pub fn deleted_line(name: &str, u: &AccountUse) -> String {
    match goes_with(u) {
        Some(w) => format!("{name} deleted with its {w}"),
        None => format!("{name} deleted"),
    }
}

/// The balance the editor reads with the draft's opening swapped in. One that starts from a
/// recorded value holds no opening at all.
pub fn preview_balance(s: Option<&Stored>, signed_opening: i64) -> i64 {
    match s {
        Some(s) if s.row.valued_on.is_some() => s.row.view.balance,
        Some(s) => s.row.view.balance - s.row.opening + signed_opening,
        None => signed_opening,
    }
}

/// Every recorded value's day as the editor lists it.
pub fn value_day(date: &str, today: Date) -> String {
    crate::model::day_word(date, today)
}

/// The name of the account new entries start in.
pub fn default_name(store: &Store) -> Result<Option<String>> {
    let Some(id) = default_account(store) else { return Ok(None) };
    store.sql()?.prepare_cached("SELECT name FROM accounts WHERE id = ?1 AND deleted = 0").map_err(err)?.query_row([id], |r| r.get(0)).optional().map_err(err)
}

// Net worth

/// Each account's balance at the end of `day`, read the way relay-money reads today's: the newest
/// value recorded on or before it plus what moved after it, else what it opened at plus
/// everything up to that day.
pub fn balances_on(store: &Store, day: Date) -> Result<HashMap<i64, i64>> {
    let db = store.sql()?;
    let mut st = db
        .prepare_cached(
            "SELECT a.id,
                    COALESCE(v.value, a.opening_balance)
                    + COALESCE((SELECT SUM(CASE WHEN t.type = 'INCOME' THEN t.amount ELSE -t.amount END)
                                FROM transactions t WHERE t.deleted = 0 AND t.account_id = a.id AND t.date <= ?1
                                  AND (v.date IS NULL OR t.date > v.date)), 0)
                    + COALESCE((SELECT SUM(t.amount) FROM transactions t
                                WHERE t.deleted = 0 AND t.type = 'TRANSFER' AND t.to_account_id = a.id AND t.date <= ?1
                                  AND (v.date IS NULL OR t.date > v.date)), 0)
             FROM accounts a
             LEFT JOIN account_values v ON v.id = (
                 SELECT v2.id FROM account_values v2 WHERE v2.deleted = 0 AND v2.account_id = a.id AND v2.date <= ?1
                 ORDER BY v2.date DESC, v2.id DESC LIMIT 1)
             WHERE a.deleted = 0",
        )
        .map_err(err)?;
    st.query_map([day.to_string()], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)
}

/// What each account given nowhere to start (no opening, no recorded value, not a card: see
/// [`AccountRow::tracked`]) is worth at the end of `day`, or with every entry counted as its
/// balance counts them when `day` is `None`.
///
/// Its balance cannot say: one built from spending alone would read as a debt that is not there.
/// So its own entries are read with a floor and its transfers in full, day by day: what moves in
/// that day first, then the day's income less its spending, which never takes it below zero (or
/// below where it already was, when what moved out had taken it there), then what moves out.
/// Spending past what it is known to hold is taken to come from money Tally was never told about.
/// So spending alone reads as $0, what is spent from money moved in is gone from net worth, and a
/// transfer moves this account by exactly what it moves the other one, whichever way it goes.
pub fn worth_by_entries(store: &Store, day: Option<Date>) -> Result<HashMap<i64, i64>> {
    entries_worth(store, day, None)
}

/// [`worth_by_entries`] with every entry that touches `without` left out: what the accounts will
/// read once it is deleted.
fn entries_worth(store: &Store, day: Option<Date>, without: Option<i64>) -> Result<HashMap<i64, i64>> {
    let db = store.sql()?;
    // Each account's days: what moved in, its own entries (as relay-money's balance counts
    // them), what moved out.
    let mut st = db
        .prepare_cached(
            "SELECT x.id, x.date, SUM(x.moved_in), SUM(x.own), SUM(x.moved_out) FROM (
                 SELECT to_account_id AS id, date, amount AS moved_in, 0 AS own, 0 AS moved_out FROM transactions
                 WHERE deleted = 0 AND type = 'TRANSFER' AND to_account_id IS NOT NULL AND (?2 IS NULL OR account_id <> ?2)
                 UNION ALL
                 SELECT account_id, date, 0,
                        CASE type WHEN 'INCOME' THEN amount WHEN 'TRANSFER' THEN 0 ELSE -amount END,
                        CASE type WHEN 'TRANSFER' THEN amount ELSE 0 END
                 FROM transactions WHERE deleted = 0 AND (?2 IS NULL OR to_account_id IS NOT ?2)) x
             JOIN accounts a ON a.id = x.id
             WHERE a.deleted = 0 AND a.opening_balance = 0 AND a.type <> 'CREDIT' AND (?2 IS NULL OR a.id <> ?2)
               AND NOT EXISTS (SELECT 1 FROM account_values v WHERE v.account_id = a.id AND v.deleted = 0)
               AND (?1 IS NULL OR x.date <= ?1)
             GROUP BY x.id, x.date
             ORDER BY x.id, x.date",
        )
        .map_err(err)?;
    let days = st
        .query_map(params![day.map(|d| d.to_string()), without], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)))
        .map_err(err)?;
    let mut out: HashMap<i64, i64> = HashMap::new();
    for d in days {
        let (id, moved_in, own, moved_out) = d.map_err(err)?;
        let w = out.entry(id).or_insert(0);
        *w += moved_in;
        *w = if own >= 0 { *w + own } else { (*w + own).max((*w).min(0)) };
        *w -= moved_out;
    }
    Ok(out)
}

/// Each account's next payment from an active bill moving money into it (a card's payment):
/// its day and amount.
pub fn payments(store: &Store) -> Result<HashMap<i64, (Date, i64)>> {
    let db = store.sql()?;
    let mut st = db
        .prepare_cached(
            "SELECT to_account_id, next_date, amount FROM recurring
             WHERE deleted = 0 AND active = 1 AND type = 'TRANSFER' AND to_account_id IS NOT NULL
             ORDER BY next_date DESC, id DESC",
        )
        .map_err(err)?;
    let found: Vec<(i64, String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)?;
    // Newest first, so the soonest one is written last and stays.
    Ok(found.into_iter().filter_map(|(id, d, a)| d.parse().ok().map(|d| (id, (d, a)))).collect())
}

/// What the accounts are worth, now and at the end of last month.
#[derive(Clone, Debug, Default)]
pub struct Worth {
    /// Each counted account today: an investment account with holdings at their value, the rest
    /// at their balance. An account given nowhere to start is counted by [`worth_by_entries`]:
    /// its spending alone never reads as a debt, but money moved into or out of it always counts,
    /// and it is left out while that reads $0.
    pub now: HashMap<i64, i64>,
    /// The same accounts at the end of `since`.
    pub then: HashMap<i64, i64>,
    /// The last day of last month.
    pub since: Option<Date>,
}

impl Worth {
    pub fn total(&self) -> i64 {
        self.now.values().sum()
    }

    pub fn of(&self, id: i64) -> Option<i64> {
        self.now.get(&id).copied()
    }

    /// How far the total moved since `since`.
    pub fn change(&self) -> Option<i64> {
        self.since.map(|_| self.total() - self.then.values().sum::<i64>())
    }
}

/// The last day of the month before `today`'s.
pub fn last_month_end(today: Date) -> Date {
    today.first_of_month().yesterday().unwrap_or(today)
}

/// The accounts with a value recorded on or before `day`, each with the newest such value's day.
pub fn valued_by(store: &Store, day: Date) -> Result<HashMap<i64, Date>> {
    let db = store.sql()?;
    let mut st = db
        .prepare_cached("SELECT account_id, MAX(date) FROM account_values WHERE deleted = 0 AND date <= ?1 GROUP BY account_id")
        .map_err(err)?;
    let found: Vec<(i64, String)> = st.query_map([day.to_string()], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)?;
    Ok(found.into_iter().filter_map(|(id, d)| d.parse().ok().map(|d| (id, d))).collect())
}

/// What moved into each account by transfer after `day`, less what moved out: every later date,
/// as its balance counts them.
pub fn moved_after(store: &Store, day: Date) -> Result<HashMap<i64, i64>> {
    let db = store.sql()?;
    let mut st = db
        .prepare_cached(
            "SELECT id, SUM(amount) FROM (
                 SELECT to_account_id AS id, amount FROM transactions
                 WHERE deleted = 0 AND type = 'TRANSFER' AND to_account_id IS NOT NULL AND date > ?1
                 UNION ALL
                 SELECT account_id, -amount FROM transactions WHERE deleted = 0 AND type = 'TRANSFER' AND date > ?1)
             GROUP BY id",
        )
        .map_err(err)?;
    st.query_map([day.to_string()], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.collect::<std::result::Result<_, _>>().map_err(err)
}

/// How an account's worth on a day is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    /// An investment account the portfolio prices: its newest holdings report's value, plus what
    /// moved in or out after it (invest_logic::moved_since_report), as a recorded value is read.
    Holdings,
    /// Its balance: it was given somewhere to start (an opening balance, a value recorded by
    /// then, or a card, which counts what it owes from zero).
    Balance,
    /// One given nowhere to start, read by [`worth_by_entries`].
    Entries,
}

/// What the ledger says about each account as of one day, for [`read_on`].
struct DayFacts {
    /// The investment accounts the portfolio prices, at their value, where holdings were said
    /// last ([`priced_where_newer`]).
    priced: HashMap<i64, i64>,
    balances: HashMap<i64, i64>,
    by_entries: HashMap<i64, i64>,
    /// The accounts with a value recorded by then, and the newest one's day.
    valued: HashMap<i64, Date>,
}

/// `priced` (invest_logic::priced_on) less each account whose newest holdings report is older
/// than the newest value recorded by hand by `day` (invest_logic::holdings_win): that value is
/// newer word on what it holds, so it reads from its balance instead, now and then alike.
fn priced_where_newer(input: &relay_money::invest::PortfolioInput, priced: HashMap<i64, i64>, day: Date, valued: &HashMap<i64, Date>) -> HashMap<i64, i64> {
    priced.into_iter().filter(|(id, _)| crate::invest_logic::holdings_win(input, *id, day, valued.get(id).copied())).collect()
}

/// Every open account as of one day, by the same rules now and then, so the two can't drift:
/// priced from holdings if the portfolio prices it and no value was recorded by hand after its
/// report, else its balance if it was given somewhere
/// to start, else by its entries.
fn read_on(rows: &[AccountRow], f: &DayFacts) -> HashMap<i64, (i64, Basis)> {
    rows.iter()
        .filter(|r| !r.view.archived)
        .map(|r| {
            let id = r.view.id;
            let read = if let Some(&v) = f.priced.get(&id) {
                (v, Basis::Holdings)
            } else if r.opening != 0 || r.view.r#type == AccountType::Credit || f.valued.contains_key(&id) {
                (f.balances.get(&id).copied().unwrap_or(0), Basis::Balance)
            } else {
                (f.by_entries.get(&id).copied().unwrap_or(0), Basis::Entries)
            };
            (id, read)
        })
        .collect()
}

/// Each open account's worth today as net worth reads it, and whether holdings priced it
/// ([`read_on`]): every entry counted, as its balance counts them, and for one priced from
/// holdings every transfer after its report.
pub fn worth_today(rows: &[AccountRow], input: Option<&relay_money::invest::PortfolioInput>, today: Date) -> HashMap<i64, (i64, bool)> {
    let valued: HashMap<i64, Date> = rows.iter().filter_map(|r| r.valued_on.map(|d| (r.view.id, d))).collect();
    let facts = DayFacts {
        priced: input.map(|i| priced_where_newer(i, crate::invest_logic::priced_on(i, today, None), today, &valued)).unwrap_or_default(),
        balances: rows.iter().map(|r| (r.view.id, r.view.balance)).collect(),
        // A row's worth is its reading by entries when it was given nowhere to start.
        by_entries: rows.iter().map(|r| (r.view.id, r.worth)).collect(),
        valued,
    };
    read_on(rows, &facts).into_iter().map(|(id, (v, b))| (id, (v, b == Basis::Holdings))).collect()
}

/// Each open account's worth and whether holdings priced it, by id.
pub type WorthMap = HashMap<i64, (i64, bool)>;

/// Every open account's worth today ([`worth_today`]) with the rows it was read from: for a page
/// that shows one account, or every account without last month's.
pub fn worth_now(store: &Store, today: Date) -> Result<(Vec<AccountRow>, WorthMap)> {
    let rows = rows(store)?;
    let input = store.ledger.portfolio_input(today).ok();
    let now = worth_today(&rows, input.as_ref(), today);
    Ok((rows, now))
}

/// The Accounts page shows a figure for it: holdings price it, it was given somewhere to start,
/// or its entries say it holds (or moved out) something. Otherwise it says "No balance".
pub fn counted(r: &AccountRow, value: i64, priced: bool) -> bool {
    priced || r.tracked() || value != 0
}

/// What each account in `now` (as [`worth_today`] read it) was worth at the end of `day`, read by
/// [`read_on`] with the rows up to that day. One read by its entries then but priced or valued
/// now (a first holdings import or a first value since) had nothing that said what it was worth,
/// so it reads as what it is worth now less what was moved into it since: the first value is not
/// a gain, and money moved in from another account, which that account reads as gone, is not
/// lost from net worth.
pub fn worth_back(
    store: &Store,
    rows: &[AccountRow],
    input: Option<&relay_money::invest::PortfolioInput>,
    day: Date,
    now: &HashMap<i64, (i64, bool)>,
) -> Result<HashMap<i64, i64>> {
    let valued = valued_by(store, day)?;
    let facts = DayFacts {
        priced: input.map(|i| priced_where_newer(i, crate::invest_logic::priced_on(i, day, Some(day)), day, &valued)).unwrap_or_default(),
        balances: balances_on(store, day)?,
        by_entries: worth_by_entries(store, Some(day))?,
        valued,
    };
    let then = read_on(rows, &facts);
    let moved = moved_after(store, day)?;
    let mut out = HashMap::new();
    for r in rows {
        let id = r.view.id;
        let (Some(&(now_v, priced_now)), Some(&(v, basis))) = (now.get(&id), then.get(&id)) else { continue };
        let started = match basis {
            Basis::Holdings | Basis::Balance => true,
            // Read the same way now.
            Basis::Entries => !priced_now && !r.tracked(),
        };
        let before = crate::invest_logic::worth_before(now_v, moved.get(&id).copied().unwrap_or(0));
        out.insert(id, crate::invest_logic::worth_then(Some((v, basis == Basis::Holdings)), started, before));
    }
    Ok(out)
}

/// Net worth now and at the end of last month, over the open accounts. A transfer between them
/// never moves it: the account the money left reads less, the one it went to more, by the same
/// amount, including one given nowhere to start (see [`worth_by_entries`]).
pub fn worth_reading(store: &Store, rows: &[AccountRow], today: Date) -> Result<Worth> {
    let since = last_month_end(today);
    let input = store.ledger.portfolio_input(today).ok();
    let now = worth_today(rows, input.as_ref(), today);
    let then = worth_back(store, rows, input.as_ref(), since, &now)?;
    let mut w = Worth { since: Some(since), ..Worth::default() };
    for r in rows.iter().filter(|r| !r.view.archived) {
        let id = r.view.id;
        let Some(&(value, priced)) = now.get(&id) else { continue };
        let was = then.get(&id).copied().unwrap_or(value);
        // One given nowhere to start and holding nothing (then or now) stays out, so the page
        // says "No balance" for it rather than $0.
        if counted(r, value, priced) {
            w.now.insert(id, value);
        }
        if counted(r, was, priced) {
            w.then.insert(id, was);
        }
    }
    Ok(w)
}

/// How the Accounts page groups accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Everyday,
    Credit,
    Investing,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Everyday, Group::Credit, Group::Investing];

    pub fn of(t: AccountType) -> Group {
        match t {
            AccountType::Credit => Group::Credit,
            AccountType::Investment => Group::Investing,
            _ => Group::Everyday,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Group::Everyday => "Everyday",
            Group::Credit => "Credit",
            Group::Investing => "Investing",
        }
    }
}

/// The bank an account is kept at, by its institution or its name.
pub fn bank_name(row: &AccountRow) -> String {
    if !row.view.institution.trim().is_empty() {
        return row.view.institution.trim().to_string();
    }
    let n = row.view.name.to_lowercase();
    ["Desjardins", "Wealthsimple", "Tangerine", "Scotiabank", "RBC", "TD", "BMO", "National Bank"]
        .iter()
        // Whole words only: "Outdoor savings" holds "td" but is not at TD. A multi-word name
        // ("National Bank") matches as a phrase.
        .find(|b| {
            let b = b.to_lowercase();
            let words: Vec<&str> = n.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
            let want: Vec<&str> = b.split(' ').collect();
            words.windows(want.len()).any(|w| w == want.as_slice())
        })
        .map(|b| b.to_string())
        .unwrap_or_else(|| "your bank".into())
}

/// The line under an account's name on the Accounts page: what it is for, a card's next payment,
/// the bank it follows, and, for one with no balance to show, what went out this month.
pub fn row_line(l: &AccountLine, s: &AccountsState, fmt: &MoneyFormatter) -> String {
    let a = &l.row.view;
    let mut parts: Vec<String> = Vec::new();
    if a.archived {
        parts.push(format!("Archived {}", type_label(a.r#type).to_lowercase()));
    } else if l.is_default {
        parts.push("Default for new entries".into());
    }
    match a.r#type {
        AccountType::Credit => match s.payments.get(&a.id) {
            Some((d, amount)) if !a.archived => parts.push(format!("Payment {} · {}", d.strftime("%b %-d"), fmt.format(*amount))),
            _ if parts.is_empty() => parts.push("Credit card".into()),
            _ => {}
        },
        AccountType::Investment => {
            if parts.is_empty() {
                parts.push(crate::invest_logic::registration_label(a.registration).to_string());
            }
            if let Some(d) = l.row.valued_on {
                parts.push(format!("valued {}", d.strftime("%b %-d")));
            }
        }
        t if parts.is_empty() => parts.push(type_label(t).to_string()),
        _ => {}
    }
    if l.row.connected() {
        parts.push(format!("Connected to {}", bank_name(&l.row)));
    }
    let paid_by_bill = a.r#type == AccountType::Credit && s.payments.contains_key(&a.id);
    if !a.archived && s.worth.of(a.id).is_none() && l.spent > 0 && !paid_by_bill {
        parts.push(format!("{} out this month", fmt.format_whole(l.spent)));
    }
    parts.join(" · ")
}

/// A figure with its sign written out, whole: "+$1,204", "−$80".
pub fn signed_whole(v: i64, fmt: &MoneyFormatter) -> String {
    if v >= 0 { format!("+{}", fmt.format_whole(v)) } else { format!("−{}", fmt.format_whole(-v)) }
}

/// The line under net worth: "+$1,204 since October 31".
pub fn change_line(w: &Worth, fmt: &MoneyFormatter) -> Option<String> {
    let (change, since) = (w.change()?, w.since?);
    let day = since.strftime("%B %-d");
    Some(if change == 0 { format!("No change since {day}") } else { format!("{} since {day}", signed_whole(change, fmt)) })
}
