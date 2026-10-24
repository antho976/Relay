//! Asking first (docs/THREADS.md, phase 4): what a thread's agent may not change alone it
//! proposes, and the person approves or declines each proposal on a card.
//!
//! A proposal is one [`Action`]: one op from a short list and a batch of that op's own payloads.
//! It lives in `threads.db`, the PC's own (`money.db` syncs with the phone and carries none of
//! this), tied to the thread whose agent sent it (`Ctx::thread`, which the socket door sets from
//! the process that connected). It is checked when it is made, by the ledger's own rules in a
//! trial that is put back: what it acts on must be there, and it must change something. What it
//! acts on is kept as it was then; a reading re-reads it and marks what changed or went since.
//!
//! Approving applies the whole batch once, as the person, in one ledger transaction inside the
//! handler: a short local write like any money op's, no other op's handler run, and the thread
//! store's lock and the ledger's taken one after the other, never one inside the other. The
//! proposal is claimed (`pending` → `running`) first, so a second answer is refused; what came of
//! it, or the failure and its message, is kept on it. An approval is refused when anything
//! changed since the person last read it. Nothing expires.
//!
//! The agent hears how each proposal was answered at the head of the person's next message
//! ([`heard`]), once per answer.

use super::{cut, with_db, ThreadStore};
use crate::engine::Engine;
use crate::handlers::money::ledger;
use relay_bus::ops::thread::{Action, Item, ProposalView};
use relay_bus::types::Id;
use relay_bus::BusError;
use relay_money::csv::major_string;
use relay_money::ledger::{Ledger, LedgerError, TxPatch};
use relay_money::money::fraction_digits;
use relay_money::views::{AccountPatch, AccountView, Tx};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The most proposals one thread may have waiting at once.
pub const MAX_PENDING: i64 = 20;
/// The longest a reason may be, in characters.
const MAX_WHY: usize = 1_000;

pub(super) const SCHEMA: &str = "
CREATE TABLE proposal (
    id INTEGER PRIMARY KEY,
    thread_id INTEGER NOT NULL REFERENCES thread(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    why TEXT NOT NULL,
    summary TEXT NOT NULL,
    status TEXT NOT NULL,
    outcome TEXT,
    items TEXT NOT NULL,
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    told TEXT
);
CREATE INDEX proposal_thread ON proposal(thread_id, id);
";

/// A proposal row. `items` is what it acts on as proposed, then as it was when answered.
struct Row {
    id: Id,
    thread_id: Id,
    action: String,
    why: String,
    summary: String,
    status: String,
    outcome: Option<String>,
    items: String,
    created_at: String,
    resolved_at: Option<String>,
}

const COLUMNS: &str = "id, thread_id, action, why, summary, status, outcome, items, created_at, resolved_at";

/// A proposal's answer, for the agent: (id, status, summary, outcome).
type Answer = (Id, String, String, Option<String>);

fn row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        thread_id: r.get(1)?,
        action: r.get(2)?,
        why: r.get(3)?,
        summary: r.get(4)?,
        status: r.get(5)?,
        outcome: r.get(6)?,
        items: r.get(7)?,
        created_at: r.get(8)?,
        resolved_at: r.get(9)?,
    })
}

impl ThreadStore {
    /// A proposal Relay stopped in the middle of applying: said, never left running.
    pub(super) fn settle_interrupted(&self, now: &str) -> rusqlite::Result<()> {
        self.conn.prepare_cached(
            "UPDATE proposal SET status = 'failed', outcome = 'Relay stopped while it was being applied: check the ledger before asking again', resolved_at = ?1 WHERE status = 'running'",
        )?.execute([now])?;
        Ok(())
    }

    fn propose(&self, thread: Id, action: &str, why: &str, summary: &str, items: &str, now: &str) -> rusqlite::Result<Id> {
        self.conn
            .prepare_cached("INSERT INTO proposal (thread_id, action, why, summary, status, items, created_at) VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6)")?
            .execute(params![thread, action, why, summary, items, now])?;
        Ok(self.conn.last_insert_rowid())
    }

    fn proposal(&self, id: Id) -> rusqlite::Result<Option<Row>> {
        self.conn.prepare_cached(&format!("SELECT {COLUMNS} FROM proposal WHERE id = ?1"))?.query_row([id], row).optional()
    }

    fn proposals(&self, thread: Id, pending: bool) -> rusqlite::Result<Vec<Row>> {
        let mut stmt = self.conn.prepare_cached(&format!("SELECT {COLUMNS} FROM proposal WHERE thread_id = ?1 AND (?2 = 0 OR status = 'pending') ORDER BY id"))?;
        let rows = stmt.query_map(params![thread, pending], row)?;
        rows.collect()
    }

    /// A proposal of the same thread already waiting with the same action, and how many wait.
    fn waiting(&self, thread: Id, action: &str) -> rusqlite::Result<(Option<Id>, i64)> {
        let same = self.conn
            .prepare_cached("SELECT id FROM proposal WHERE thread_id = ?1 AND status = 'pending' AND action = ?2 ORDER BY id LIMIT 1")?
            .query_row(params![thread, action], |r| r.get(0))
            .optional()?;
        let count = self.conn.prepare_cached("SELECT COUNT(*) FROM proposal WHERE thread_id = ?1 AND status = 'pending'")?.query_row([thread], |r| r.get(0))?;
        Ok((same, count))
    }

    /// Move a proposal from `from` to `to`, keeping `items` as what it acted on; `false` when it
    /// was not `from` any more (another answer came first).
    fn claim(&self, id: Id, from: &str, to: &str, items: &str, now: &str) -> rusqlite::Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE proposal SET status = ?3, items = ?4, resolved_at = ?5 WHERE id = ?1 AND status = ?2")?
            .execute(params![id, from, to, items, now])? > 0)
    }

    fn settle(&self, id: Id, from: &str, to: &str, outcome: &str, now: &str) -> rusqlite::Result<bool> {
        Ok(self.conn
            .prepare_cached("UPDATE proposal SET status = ?3, outcome = ?4, resolved_at = ?5 WHERE id = ?1 AND status = ?2")?
            .execute(params![id, from, to, outcome, now])? > 0)
    }

    /// Answers the thread's agent has not been told.
    fn untold(&self, thread: Id) -> rusqlite::Result<Vec<Answer>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, status, summary, outcome FROM proposal
             WHERE thread_id = ?1 AND status IN ('approved', 'declined', 'failed', 'undone') AND (told IS NULL OR told != status) ORDER BY id",
        )?;
        let rows = stmt.query_map([thread], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect()
    }

    fn told(&self, id: Id, status: &str) -> rusqlite::Result<()> {
        self.conn.prepare_cached("UPDATE proposal SET told = ?2 WHERE id = ?1")?.execute(params![id, status])?;
        Ok(())
    }
}

fn not_found(id: Id) -> BusError {
    BusError::not_found("thread.proposal_not_found", format!("No proposal {id}"))
}

fn internal(e: impl std::fmt::Display) -> BusError {
    BusError::internal(format!("threads: {e}"))
}

fn load(engine: &Engine, id: Id) -> Result<Row, BusError> {
    with_db(engine, |db| db.proposal(id))?.ok_or_else(|| not_found(id))
}

fn answered(status: &str) -> BusError {
    let status = if status == "running" { "being applied" } else { status };
    BusError::conflict("thread.proposal_answered", format!("That proposal is already {status}"))
}

// ── Reading what a proposal acts on ───────────────────────────────────────

/// What of an item counts when telling whether it changed: the entry, activity, account or
/// budget as the ledger has it, without what moves on its own (an account's balance, a
/// category's colour).
fn substance(item: &Item) -> Value {
    let entry = item.entry.as_ref().map(|t| json!([t.id, t.r#type, t.amount, t.date, t.account_id, t.to_account_id, t.category_id, t.note]));
    let activity = item.activity.as_ref().map(|a| json!([a.id, a.r#type, a.date, a.amount, a.quantity, a.symbol, a.account_id]));
    let account = item.account.as_ref().map(|a| json!([a.id, a.name, a.r#type, a.archived, a.registration, a.institution]));
    json!([item.state == "gone", entry, activity, account, item.category, item.budget])
}

/// The fingerprint an approval binds to: what the person was shown is what is applied.
fn fingerprint(items: &[Item]) -> String {
    let all: Vec<Value> = items.iter().map(substance).collect();
    let digest = Sha256::digest(Value::Array(all).to_string().as_bytes());
    crate::hex(&digest[..8])
}

fn account_of(l: &Ledger, id: i64) -> Result<AccountView, LedgerError> {
    l.accounts()?.into_iter().find(|a| a.id == id).ok_or_else(|| LedgerError::NotFound(format!("No account {id}")))
}

fn category_name(l: &Ledger, id: i64) -> Result<String, LedgerError> {
    l.categories()?.into_iter().find(|c| c.id == id).map(|c| c.name).ok_or_else(|| LedgerError::NotFound(format!("No category {id}")))
}

fn gone(why: LedgerError) -> Result<Item, LedgerError> {
    match why {
        LedgerError::NotFound(_) => Ok(Item { state: "gone".into(), ..Default::default() }),
        other => Err(other),
    }
}

/// What the action acts on, one item per payload, as the ledger has it now; each change's result
/// comes from a trial of that payload alone, put back at once.
fn read(l: &mut Ledger, action: &Action) -> Result<Vec<Item>, LedgerError> {
    let same = |item: Item| Item { state: "same".into(), ..item };
    let mut items = Vec::with_capacity(action.len());
    match action {
        Action::TxDelete(ids) => {
            for p in ids {
                items.push(l.tx(p.id).map(|t| same(Item { entry: Some(t), ..Default::default() })).or_else(gone)?);
            }
        }
        Action::InvestDelete(ids) => {
            for p in ids {
                items.push(l.activity(p.id).map(|a| same(Item { activity: Some(a), ..Default::default() })).or_else(gone)?);
            }
        }
        Action::TxUpdate(patches) => {
            for p in patches {
                let item = match l.tx(p.id) {
                    Ok(entry) => {
                        let patch = TxPatch {
                            r#type: p.r#type, amount: p.amount, date: p.date.clone(), account_id: p.account_id,
                            to_account_id: p.to_account_id, category_id: p.category_id, note: p.note.clone(), clear_category: p.clear_category,
                        };
                        let after = l.trial(|l| l.tx_update(p.id, &patch)).ok();
                        same(Item { entry: Some(entry), entry_after: after, ..Default::default() })
                    }
                    Err(e) => gone(e)?,
                };
                items.push(item);
            }
        }
        Action::BudgetSet(budgets) => {
            for p in budgets {
                let category = match p.category_id.map(|c| category_name(l, c)).transpose() {
                    Ok(name) => name,
                    Err(e) => {
                        items.push(gone(e)?);
                        continue;
                    }
                };
                let budget = l.budget(p.category_id)?;
                items.push(same(Item { category, budget, budget_after: (p.amount > 0).then_some(p.amount), ..Default::default() }));
            }
        }
        Action::AccountAdd(accounts) => {
            for p in accounts {
                let institution = p.institution.as_deref().unwrap_or("");
                let after = l.trial(|l| l.account_add(&p.name, p.r#type, p.opening_balance.unwrap_or(0), p.registration, institution)).ok();
                items.push(Item { state: "new".into(), account_after: after.map(|a| AccountView { id: 0, ..a }), ..Default::default() });
            }
        }
        Action::AccountUpdate(accounts) => {
            for p in accounts {
                let item = match account_of(l, p.id) {
                    Ok(account) => {
                        let patch = AccountPatch { name: p.name.clone(), registration: p.registration, institution: p.institution.clone(), archived: p.archived };
                        let after = l.trial(|l| l.account_update(p.id, &patch)).ok();
                        same(Item { account: Some(account), account_after: after, ..Default::default() })
                    }
                    Err(e) => gone(e)?,
                };
                items.push(item);
            }
        }
    }
    Ok(items)
}

/// Mark each item read now against the same item when it was proposed: `changed` or `gone`.
fn against(mut live: Vec<Item>, then: &[Item]) -> Vec<Item> {
    for (now, was) in live.iter_mut().zip(then) {
        if now.state != "gone" && now.state != "new" && substance(now) != substance(was) {
            now.state = "changed".into();
        }
    }
    live
}

/// The whole action, applied: each payload as its op would, in order. Run inside
/// [`Ledger::atomically`] (approving) or [`Ledger::trial`] (checking).
fn apply(l: &mut Ledger, action: &Action) -> Result<(), LedgerError> {
    match action {
        Action::TxDelete(ids) => ids.iter().try_for_each(|p| l.tx_delete(p.id)),
        Action::InvestDelete(ids) => ids.iter().try_for_each(|p| l.invest_delete(p.id)),
        Action::TxUpdate(patches) => patches.iter().try_for_each(|p| {
            let patch = TxPatch {
                r#type: p.r#type, amount: p.amount, date: p.date.clone(), account_id: p.account_id,
                to_account_id: p.to_account_id, category_id: p.category_id, note: p.note.clone(), clear_category: p.clear_category,
            };
            l.tx_update(p.id, &patch).map(|_| ()).map_err(|e| about(e, &format!("Entry {}", p.id)))
        }),
        Action::BudgetSet(budgets) => budgets.iter().try_for_each(|p| l.budget_set(p.category_id, p.amount)),
        Action::AccountAdd(accounts) => accounts.iter().try_for_each(|p| {
            l.account_add(&p.name, p.r#type, p.opening_balance.unwrap_or(0), p.registration, p.institution.as_deref().unwrap_or("")).map(|_| ())
        }),
        Action::AccountUpdate(accounts) => accounts.iter().try_for_each(|p| {
            let patch = AccountPatch { name: p.name.clone(), registration: p.registration, institution: p.institution.clone(), archived: p.archived };
            l.account_update(p.id, &patch).map(|_| ()).map_err(|e| about(e, &format!("Account {}", p.id)))
        }),
    }
}

/// A refusal of one payload in a batch, saying which.
fn about(e: LedgerError, which: &str) -> LedgerError {
    match e {
        LedgerError::Invalid(m) => LedgerError::Invalid(format!("{which}: {m}")),
        other => other,
    }
}

/// Undoing an approved action: entries and activities restored, budgets and accounts as they
/// were. Refused when something was changed since the approval, which putting it back would
/// overwrite, or when the ledger cannot put it back exactly.
fn put_back(l: &mut Ledger, action: &Action, items: &[Item]) -> Result<(), LedgerError> {
    let changed = |what: &str| LedgerError::Invalid(format!("{what} changed after it was approved: change it in Tally instead"));
    match action {
        Action::TxDelete(ids) => ids.iter().try_for_each(|p| l.tx_restore(p.id).map(|_| ())),
        Action::InvestDelete(ids) => ids.iter().try_for_each(|p| l.invest_restore(p.id).map(|_| ())),
        Action::TxUpdate(_) => items.iter().try_for_each(|item| {
            let (Some(was), Some(after)) = (&item.entry, &item.entry_after) else { return Ok(()) };
            let now = l.tx(was.id)?;
            let probe = |t: &Tx| substance(&Item { entry: Some(t.clone()), ..Default::default() });
            if probe(&now) != probe(after) {
                return Err(changed(&format!("Entry {}", was.id)));
            }
            let patch = TxPatch {
                r#type: Some(was.r#type), amount: Some(was.amount), date: Some(was.date.clone()), account_id: Some(was.account_id),
                to_account_id: was.to_account_id, category_id: was.category_id, note: Some(was.note.clone()),
                clear_category: Some(was.category_id.is_none()),
            };
            let back = l.tx_update(was.id, &patch)?;
            if probe(&back) != probe(was) {
                return Err(LedgerError::Invalid(format!("Entry {} cannot be put back exactly: change it in Tally", was.id)));
            }
            Ok(())
        }),
        Action::BudgetSet(budgets) => budgets.iter().zip(items).try_for_each(|(p, item)| {
            if l.budget(p.category_id)? != item.budget_after {
                return Err(changed(&format!("The {} budget", item.category.as_deref().unwrap_or("monthly"))));
            }
            l.budget_set(p.category_id, item.budget.unwrap_or(0))
        }),
        Action::AccountUpdate(_) => items.iter().try_for_each(|item| {
            let (Some(was), Some(after)) = (&item.account, &item.account_after) else { return Ok(()) };
            let probe = |a: &AccountView| substance(&Item { account: Some(a.clone()), ..Default::default() });
            if probe(&account_of(l, was.id)?) != probe(after) {
                return Err(changed(&format!("The account \u{201c}{}\u{201d}", was.name)));
            }
            let patch = AccountPatch { name: Some(was.name.clone()), registration: was.registration, institution: Some(was.institution.clone()), archived: Some(was.archived) };
            if probe(&l.account_update(was.id, &patch)?) != probe(was) {
                return Err(LedgerError::Invalid(format!("The account \u{201c}{}\u{201d} cannot be put back exactly: change it in Tally", was.name)));
            }
            Ok(())
        }),
        Action::AccountAdd(_) => Err(LedgerError::Invalid("An account added cannot be taken away here: archive it in Tally".into())),
    }
}

// ── Words ─────────────────────────────────────────────────────────────────

/// How many items a summary names before "and N more".
const NAMED: usize = 5;

struct Words {
    digits: u32,
    currency: String,
}

impl Words {
    fn money(&self, minor: i64) -> String {
        format!("{} {}", major_string(minor, self.digits), self.currency)
    }

    /// "2026-10-02 \u{201c}Loblaws\u{201d} 84.12 CAD"
    fn entry(&self, t: &Tx) -> String {
        let what = Some(t.note.trim()).filter(|n| !n.is_empty()).map(|n| format!("\u{201c}{n}\u{201d}")).or_else(|| t.category.clone()).unwrap_or_else(|| kind(t).to_string());
        format!("{} {what} {}", t.date, self.money(t.amount))
    }

    /// What a change does to an entry: "Shopping → Groceries, 12.00 CAD → 21.00 CAD".
    fn entry_change(&self, was: &Tx, now: &Tx) -> String {
        let mut parts = Vec::new();
        if was.r#type != now.r#type {
            parts.push(format!("{} → {}", kind(was), kind(now)));
        }
        if was.category != now.category {
            parts.push(format!("{} → {}", was.category.as_deref().unwrap_or("no category"), now.category.as_deref().unwrap_or("no category")));
        }
        if was.amount != now.amount {
            parts.push(format!("{} → {}", self.money(was.amount), self.money(now.amount)));
        }
        if was.date != now.date {
            parts.push(format!("{} → {}", was.date, now.date));
        }
        if was.account != now.account || was.to_account != now.to_account {
            parts.push(format!("{} → {}", was.account, now.account));
        }
        if was.note != now.note {
            parts.push(format!("note \u{201c}{}\u{201d} → \u{201c}{}\u{201d}", was.note, now.note));
        }
        parts.join(", ")
    }

    fn budget(&self, amount: Option<i64>) -> String {
        amount.map_or_else(|| String::from("none"), |a| self.money(a))
    }
}

/// An enum's name as the ledger stores it: `BUY`, `SAVINGS`.
fn name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn kind(t: &Tx) -> &'static str {
    match t.r#type {
        relay_money::model::TxType::Income => "income",
        relay_money::model::TxType::Transfer => "transfer",
        relay_money::model::TxType::Expense => "expense",
    }
}

/// "a; b; c and 4 more"
fn listed(parts: Vec<String>) -> String {
    let more = parts.len().saturating_sub(NAMED);
    let mut out = parts.into_iter().take(NAMED).collect::<Vec<_>>().join("; ");
    if more > 0 {
        out.push_str(&format!(" and {more} more"));
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// What approving does, in words read from the ledger: what the agent quotes and the person reads.
fn summary(action: &Action, items: &[Item], w: &Words) -> String {
    let n = action.len();
    match action {
        Action::TxDelete(_) => format!("Delete {}: {}", plural(n, "entry", "entries"), listed(items.iter().filter_map(|i| i.entry.as_ref()).map(|t| w.entry(t)).collect())),
        Action::InvestDelete(_) => format!("Delete {}: {}", plural(n, "investment activity", "investment activities"), listed(items.iter().filter_map(|i| i.activity.as_ref()).map(|a| {
            let what = a.symbol.as_deref().unwrap_or(a.account.as_str());
            format!("{} {} {what} {} {} in {}", a.date, name(&a.r#type), major_string(a.amount, fraction_digits(&a.currency).unwrap_or(2)), a.currency, a.account)
        }).collect())),
        Action::TxUpdate(_) => format!("Change {}: {}", plural(n, "entry", "entries"), listed(items.iter().filter_map(|i| {
            let (was, now) = (i.entry.as_ref()?, i.entry_after.as_ref()?);
            Some(format!("{}, {}", w.entry(was), w.entry_change(was, now)))
        }).collect())),
        Action::BudgetSet(_) => format!("Set {}: {}", plural(n, "budget", "budgets"), listed(items.iter().map(|i| {
            let name = i.category.as_deref().map_or_else(|| String::from("the monthly budget"), str::to_string);
            match i.budget_after {
                Some(_) => format!("{name} {} → {}", w.budget(i.budget), w.budget(i.budget_after)),
                None => format!("{name} {} → removed", w.budget(i.budget)),
            }
        }).collect())),
        Action::AccountAdd(_) => format!("Add {}: {}", plural(n, "account", "accounts"), listed(items.iter().filter_map(|i| i.account_after.as_ref()).map(|a| {
            let mut about = vec![name(&a.r#type).to_lowercase()];
            if let Some(r) = a.registration {
                about.push(r.label().to_string());
            }
            if !a.institution.is_empty() {
                about.push(format!("at {}", a.institution));
            }
            if a.balance != 0 {
                about.push(format!("opening balance {}", w.money(a.balance)));
            }
            format!("\u{201c}{}\u{201d} ({})", a.name, about.join(", "))
        }).collect())),
        Action::AccountUpdate(_) => format!("Change {}: {}", plural(n, "account", "accounts"), listed(items.iter().filter_map(|i| {
            let (was, now) = (i.account.as_ref()?, i.account_after.as_ref()?);
            let mut parts = Vec::new();
            if was.name != now.name {
                parts.push(format!("rename to \u{201c}{}\u{201d}", now.name));
            }
            if was.archived != now.archived {
                parts.push(String::from(if now.archived { "archive it" } else { "bring it back from the archive" }));
            }
            if was.registration != now.registration {
                parts.push(format!("registration {}", now.registration.map_or("none", |r| r.label())));
            }
            if was.institution != now.institution {
                parts.push(format!("held at {}", now.institution));
            }
            Some(format!("\u{201c}{}\u{201d}: {}", was.name, parts.join(", ")))
        }).collect())),
    }
}

/// What approving did, for the card and the agent.
fn done(action: &Action) -> String {
    let n = action.len();
    match action {
        Action::TxDelete(_) => format!("Deleted {}.", plural(n, "entry", "entries")),
        Action::InvestDelete(_) => format!("Deleted {}.", plural(n, "investment activity", "investment activities")),
        Action::TxUpdate(_) => format!("Changed {}.", plural(n, "entry", "entries")),
        Action::BudgetSet(_) => format!("Set {}.", plural(n, "budget", "budgets")),
        Action::AccountAdd(_) => format!("Added {}.", plural(n, "account", "accounts")),
        Action::AccountUpdate(_) => format!("Changed {}.", plural(n, "account", "accounts")),
    }
}

// ── Ops ───────────────────────────────────────────────────────────────────

fn words(l: &Ledger) -> Result<Words, LedgerError> {
    let currency = l.settings()?.currency;
    Ok(Words { digits: fraction_digits(&currency).unwrap_or(2), currency })
}

/// The proposal as a card draws it: what it acts on read now while it waits, as it was when it
/// was answered after that.
fn view(engine: &Engine, r: Row) -> Result<ProposalView, BusError> {
    let action: Action = serde_json::from_str(&r.action).map_err(internal)?;
    let then: Vec<Item> = serde_json::from_str(&r.items).map_err(internal)?;
    let pending = r.status == "pending";
    let (currency, items) = ledger(engine, |l| {
        let currency = l.settings()?.currency;
        Ok((currency, if pending { against(read(l, &action)?, &then) } else { then }))
    })?;
    let undoable = r.status == "approved" && !matches!(action, Action::AccountAdd(_));
    Ok(ProposalView {
        id: r.id,
        thread_id: r.thread_id,
        stale: items.iter().any(|i| i.state == "changed" || i.state == "gone"),
        seen: fingerprint(&items),
        action,
        why: r.why,
        summary: r.summary,
        status: r.status,
        outcome: r.outcome,
        currency,
        items,
        undoable,
        created_at: r.created_at,
        resolved_at: r.resolved_at,
    })
}

pub fn get(engine: &Engine, id: Id) -> Result<ProposalView, BusError> {
    view(engine, load(engine, id)?)
}

pub fn list(engine: &Engine, thread: Id, pending: bool) -> Result<Vec<ProposalView>, BusError> {
    super::get(engine, thread)?;
    let rows = with_db(engine, |db| db.proposals(thread, pending))?;
    rows.into_iter().map(|r| view(engine, r)).collect()
}

/// `thread.propose` from `thread`'s agent: checked by a trial of the whole action, then kept,
/// waiting. The same action already waiting in the thread is answered with that proposal.
pub fn propose(engine: &Engine, thread: Id, action: Action, why: &str, now: &str) -> Result<ProposalView, BusError> {
    super::get(engine, thread)?;
    let why = why.trim();
    if why.is_empty() || why.chars().count() > MAX_WHY {
        return Err(BusError::invalid("thread.invalid", format!("Say why in a sentence of at most {MAX_WHY} characters: the person reads it on the card")));
    }
    if action.is_empty() || action.len() > action.most() {
        return Err(BusError::invalid("thread.invalid", format!("A proposal to {} holds 1 to {}", action.op(), action.most())));
    }
    twice(&action)?;
    let key = serde_json::to_string(&action).map_err(internal)?;
    let (same, waiting) = with_db(engine, |db| db.waiting(thread, &key))?;
    if let Some(id) = same {
        return get(engine, id);
    }
    if waiting >= MAX_PENDING {
        return Err(BusError::conflict("thread.too_many_proposals", format!("{waiting} proposals already wait in this thread"))
            .with_hint("wait for the person to answer them"));
    }
    let (items, summary) = ledger(engine, |l| {
        let items = read(l, &action)?;
        if let Some(i) = items.iter().position(|i| i.state == "gone") {
            return Err(LedgerError::NotFound(format!("{} is not in the ledger", which(&action, i))));
        }
        unchanged(&action, &items)?;
        // The ledger's own rules, on the whole batch at once, then put back.
        l.trial(|l| apply(l, &action))?;
        let said = summary(&action, &items, &words(l)?);
        Ok((items, said))
    })?;
    let stored = serde_json::to_string(&items).map_err(internal)?;
    let id = with_db(engine, |db| db.propose(thread, &key, why, &summary, &stored, now))?;
    get(engine, id)
}

/// "Entry 42", "The budget of category 7": the payload at `at`, for a refusal.
fn which(action: &Action, at: usize) -> String {
    match action {
        Action::TxDelete(v) | Action::InvestDelete(v) => format!("{} {}", if matches!(action, Action::TxDelete(_)) { "Entry" } else { "Activity" }, v[at].id),
        Action::TxUpdate(v) => format!("Entry {}", v[at].id),
        Action::AccountUpdate(v) => format!("Account {}", v[at].id),
        Action::BudgetSet(v) => v[at].category_id.map_or_else(|| String::from("The monthly budget"), |c| format!("The budget of category {c}")),
        Action::AccountAdd(_) => format!("New account {}", at + 1),
    }
}

/// One target named twice in a batch would be applied, and put back, twice.
fn twice(action: &Action) -> Result<(), BusError> {
    let keys: Vec<Option<i64>> = match action {
        Action::TxDelete(v) | Action::InvestDelete(v) => v.iter().map(|p| Some(p.id)).collect(),
        Action::TxUpdate(v) => v.iter().map(|p| Some(p.id)).collect(),
        Action::AccountUpdate(v) => v.iter().map(|p| Some(p.id)).collect(),
        Action::BudgetSet(v) => v.iter().map(|p| p.category_id).collect(),
        Action::AccountAdd(_) => return Ok(()),
    };
    for (at, key) in keys.iter().enumerate() {
        if keys[..at].contains(key) {
            return Err(BusError::invalid("thread.invalid", format!("{} is named twice: once is enough", which(action, at))));
        }
    }
    Ok(())
}

/// A change that changes nothing is not one to ask about.
fn unchanged(action: &Action, items: &[Item]) -> Result<(), LedgerError> {
    for (at, i) in items.iter().enumerate() {
        let same = match action {
            Action::TxUpdate(_) => i.entry_after.as_ref().is_some_and(|after| substance(&Item { entry: Some(after.clone()), ..Default::default() }) == substance(&Item { entry: i.entry.clone(), ..Default::default() })),
            Action::AccountUpdate(_) => i.account_after.as_ref().is_some_and(|after| substance(&Item { account: Some(after.clone()), ..Default::default() }) == substance(&Item { account: i.account.clone(), ..Default::default() })),
            Action::BudgetSet(_) => i.budget == i.budget_after,
            _ => false,
        };
        if same {
            return Err(LedgerError::Invalid(format!("{} already is that way", which(action, at))));
        }
    }
    Ok(())
}

/// The person's answer. Declining keeps what it would have acted on and changes nothing.
/// Approving reads it again: refused when any of it went, or changed since the reading the person
/// approved (`seen`; the proposal as made when absent). Then it is claimed, applied whole or not
/// at all, and what came of it kept.
pub fn resolve(engine: &Engine, id: Id, approve: bool, seen: Option<&str>) -> Result<ProposalView, BusError> {
    let r = load(engine, id)?;
    if r.status != "pending" {
        return Err(answered(&r.status));
    }
    let action: Action = serde_json::from_str(&r.action).map_err(internal)?;
    let then: Vec<Item> = serde_json::from_str(&r.items).map_err(internal)?;
    let now_items = against(ledger(engine, |l| read(l, &action))?, &then);
    let stored = serde_json::to_string(&now_items).map_err(internal)?;
    let now = crate::time::now();
    if !approve {
        if !with_db(engine, |db| db.claim(id, "pending", "declined", &stored, &now))? {
            return Err(answered("answered"));
        }
        return get(engine, id);
    }
    let missing = now_items.iter().filter(|i| i.state == "gone").count();
    if missing > 0 {
        return Err(BusError::conflict("thread.proposal_gone", format!("{} of what it acts on is no longer in the ledger", plural(missing, "item", "items")))
            .with_hint("decline it, and ask again if it is still wanted"));
    }
    let want = seen.map_or_else(|| fingerprint(&then), str::to_string);
    if fingerprint(&now_items) != want {
        return Err(BusError::conflict("thread.proposal_changed", "It changed since you read it: read it again before approving"));
    }
    if !with_db(engine, |db| db.claim(id, "pending", "running", &stored, &now))? {
        return Err(answered("answered"));
    }
    let applied = ledger(engine, |l| l.atomically(|l| apply(l, &action)));
    let (status, outcome) = match applied {
        Ok(()) => ("approved", done(&action)),
        Err(e) => ("failed", e.message),
    };
    with_db(engine, |db| db.settle(id, "running", status, &outcome, &crate::time::now()))?;
    get(engine, id)
}

/// Put an approved proposal back, whole or not at all (`put_back`).
pub fn undo(engine: &Engine, id: Id) -> Result<ProposalView, BusError> {
    let r = load(engine, id)?;
    if r.status != "approved" {
        return Err(BusError::conflict("thread.proposal_not_approved", format!("Only an approved proposal can be undone; this one is {}", r.status)));
    }
    let action: Action = serde_json::from_str(&r.action).map_err(internal)?;
    let items: Vec<Item> = serde_json::from_str(&r.items).map_err(internal)?;
    ledger(engine, |l| l.atomically(|l| put_back(l, &action, &items)))?;
    with_db(engine, |db| db.settle(id, "approved", "undone", "Put back as it was.", &crate::time::now()))?;
    get(engine, id)
}

/// What the agent has not heard of its proposals' answers, as lines for the head of the person's
/// next message, and the answers they tell. Empty when there is nothing new.
pub(super) fn heard(engine: &Engine, thread: Id) -> (String, Vec<(Id, String)>) {
    let Ok(untold) = with_db(engine, |db| db.untold(thread)) else { return (String::new(), Vec::new()) };
    let lines: Vec<String> = untold.iter().map(|(id, status, summary, outcome)| {
        let summary = cut(summary, 240);
        match status.as_str() {
            "approved" => format!("[Relay: the person approved proposal {id} ({summary}); it is done.]"),
            "declined" => format!("[Relay: the person declined proposal {id} ({summary}); nothing changed.]"),
            "undone" => format!("[Relay: the person undid proposal {id} ({summary}); it is back as it was.]"),
            _ => format!("[Relay: proposal {id} ({summary}) was approved but failed, so nothing changed: {}]", outcome.as_deref().unwrap_or("no reason given")),
        }
    }).collect();
    (lines.join("\n"), untold.into_iter().map(|(id, status, _, _)| (id, status)).collect())
}

/// The agent was given these answers.
pub(super) fn told(engine: &Engine, answers: &[(Id, String)]) {
    for (id, status) in answers {
        if let Err(e) = with_db(engine, |db| db.told(*id, status)) {
            tracing::warn!(proposal = id, "marking a proposal's answer told failed: {}", e.message);
        }
    }
}
