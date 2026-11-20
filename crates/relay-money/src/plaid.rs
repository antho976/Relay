//! Bank transactions read through Plaid, written into the ledger as ordinary entries
//! (Tally's `docs/PLAID.md`, "Turning a transaction into an entry").
//!
//! Server only: Tally never reads Plaid itself, it receives these entries by `money.sync` like any
//! other. So this module has no Kotlin twin. The engine (`relay-core`, `plaid.rs`) talks to Plaid
//! and keeps the banks; what it reads lands here, in one ledger transaction per batch.
//!
//! - An entry's uid is `plaid:<transaction_id>`. A uid present (deleted or not) or tombstoned is
//!   never written again, as for posted bills.
//! - Pending transactions are skipped: the posted one has its own id and comes later.
//! - Plaid's amount is positive for money out: an EXPENSE; negative is INCOME.
//! - A payment to a followed credit card from another followed account arrives as two
//!   transactions; the second to arrive turns the first's entry into one TRANSFER and is
//!   tombstoned, so it is never written on its own.
//! - An entry the CSV import already wrote (same account, amount and kind, within two days, no
//!   `plaid:` uid) is matched and skipped.
//! - A transaction Plaid removes deletes its entry only if nobody edited it.

use crate::invest::parse_scaled;
use crate::ledger::{name_of, new_uid, now_ms, parse_date, Ledger, LedgerError, MAX_AMOUNT};
use crate::model::{AccountType, TxType};
use crate::money::fraction_digits;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, LedgerError>;

/// The uid prefix of an entry read from a bank.
pub const UID_PREFIX: &str = "plaid:";
/// How far apart, in days, the two halves of a card payment may post.
pub const TRANSFER_DAYS: i64 = 3;
/// How far apart, in days, an imported entry and the same purchase from Plaid may be dated.
pub const DUPLICATE_DAYS: i64 = 2;

/// One transaction as `/transactions/sync` gives it, the fields this module reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaidTx {
    pub transaction_id: String,
    pub account_id: String,
    /// The amount as Plaid wrote it, a decimal in text ("12.34", "-1500"): never a float.
    pub amount: String,
    pub iso_currency_code: Option<String>,
    /// `YYYY-MM-DD`, the posted date.
    pub date: String,
    pub pending: bool,
    pub merchant_name: Option<String>,
    pub name: Option<String>,
    /// `personal_finance_category.primary`, e.g. `FOOD_AND_DRINK`.
    pub category_primary: Option<String>,
    /// `personal_finance_category.detailed`, e.g. `FOOD_AND_DRINK_GROCERIES`.
    pub category_detailed: Option<String>,
}

/// Where a followed Plaid account's transactions go.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// The ledger account (its row id).
    pub account_id: i64,
    /// The first day to bring in; earlier transactions are left out.
    pub since: Option<String>,
    /// Take every amount as the ledger's currency, whatever Plaid labels it: Desjardins sends CAD
    /// amounts labelled USD.
    pub any_currency: bool,
}

/// What one batch did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applied {
    /// New entries.
    pub written: usize,
    /// Entries turned into a transfer by the other half of a card payment.
    pub transfers: usize,
    /// Transactions skipped as an entry the ledger already had from an import.
    pub matched: usize,
    /// Entries deleted because Plaid removed their transaction.
    pub removed: usize,
    /// Transactions left out because they are in another currency than the ledger's.
    pub other_currency: usize,
}

impl Applied {
    /// Whether the ledger changed.
    pub fn changed(&self) -> bool {
        self.written + self.transfers + self.removed > 0
    }
}

/// The ledger account type for Plaid's subtype: credit card → CREDIT, savings → SAVINGS, else
/// CHEQUING.
pub fn account_type_for(subtype: &str) -> AccountType {
    match subtype.trim().to_ascii_lowercase().as_str() {
        "credit card" | "credit_card" | "credit" => AccountType::Credit,
        "savings" | "hsa" | "cd" | "money market" | "money_market" => AccountType::Savings,
        _ => AccountType::Chequing,
    }
}

/// The default category (by name) a transaction goes under, from Plaid's personal finance
/// category; `None` leaves it uncategorised for the person to file, as an import does. Transfers,
/// loan payments, fees and services have no default home.
pub fn category_for(primary: Option<&str>, detailed: Option<&str>, income: bool) -> Option<&'static str> {
    let primary = primary?.trim();
    let detailed = detailed.unwrap_or("").trim();
    if income {
        return match primary {
            "INCOME" if detailed == "INCOME_WAGES" => Some("Salary"),
            "INCOME" if detailed == "INCOME_TAX_REFUND" => Some("Refunds"),
            "INCOME" => Some("Other income"),
            "TRANSFER_IN" | "TRANSFER_OUT" | "LOAN_PAYMENTS" | "BANK_FEES" => None,
            // Money back from a shop, a restaurant, a trip: a refund.
            _ => Some("Refunds"),
        };
    }
    Some(match primary {
        "FOOD_AND_DRINK" if detailed == "FOOD_AND_DRINK_GROCERIES" => "Groceries",
        "FOOD_AND_DRINK" => "Dining",
        "TRANSPORTATION" => "Transport",
        "TRAVEL" => "Travel",
        "RENT_AND_UTILITIES" if detailed == "RENT_AND_UTILITIES_RENT" => "Housing",
        "RENT_AND_UTILITIES" if matches!(detailed, "RENT_AND_UTILITIES_INTERNET_AND_CABLE" | "RENT_AND_UTILITIES_TELEPHONE") => "Phone & internet",
        "RENT_AND_UTILITIES" => "Utilities",
        "HOME_IMPROVEMENT" => "Housing",
        "GENERAL_MERCHANDISE" => "Shopping",
        "ENTERTAINMENT" => "Entertainment",
        "MEDICAL" | "PERSONAL_CARE" => "Health",
        _ => return None,
    })
}

/// A bank account's ledger account, as [`Ledger::plaid_account`] is asked for it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BankAccount<'a> {
    /// The ledger account uid the person chose (`into`), if any.
    pub into: Option<&'a str>,
    pub name: &'a str,
    pub mask: &'a str,
    /// Plaid's subtype in words ("checking", "credit card").
    pub kind: &'a str,
    pub institution: &'a str,
}

impl Ledger {
    /// The ledger account a followed bank account writes into, and its uid: the one the person
    /// chose; else the one an import made for it (same institution and number); else a new one
    /// named after the bank's, with the institution and number set so later imports find it.
    pub fn plaid_account(&mut self, a: &BankAccount) -> Result<(i64, String)> {
        if let Some(uid) = a.into.map(str::trim).filter(|u| !u.is_empty()) {
            let found = self.conn.prepare_cached("SELECT id, uid FROM accounts WHERE uid = ?1 AND deleted = 0")?
                .query_row([uid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            if let Some(found) = found {
                return Ok(found);
            }
        }
        let (institution, mask) = (a.institution.trim(), a.mask.trim());
        if !institution.is_empty() && !mask.is_empty() {
            let found = self.conn.prepare_cached(
                "SELECT id, uid FROM accounts WHERE deleted = 0 AND institution = ?1 COLLATE NOCASE AND external_ref = ?2 ORDER BY id LIMIT 1",
            )?.query_row(params![institution, mask], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            if let Some(found) = found {
                return Ok(found);
            }
        }
        let name = match (a.name.trim(), mask) {
            ("", "") => institution.to_string(),
            ("", m) => format!("{institution} ••{m}"),
            (n, _) => n.to_string(),
        };
        let name = if name.is_empty() { "Bank account".to_string() } else { name };
        let order: i64 = self.conn.prepare_cached("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM accounts")?.query_row([], |r| r.get(0))?;
        let uid = new_uid();
        self.conn.prepare_cached(
            "INSERT INTO accounts (uid, name, type, opening_balance, sort_order, institution, external_ref, updated_at) VALUES (?1,?2,?3,0,?4,?5,?6,?7)",
        )?.execute(params![uid, name, name_of(&account_type_for(a.kind)), order, institution, mask, now_ms()])?;
        let id = self.conn.last_insert_rowid();
        self.ensure_categories()?;
        Ok((id, uid))
    }

    /// Writes one batch from `/transactions/sync` in one transaction: `added` and `modified` (a
    /// modified transaction the ledger does not hold yet is taken as new; one it holds is left as
    /// written), then `removed`. `targets` maps each followed Plaid account id to its ledger
    /// account; transactions on any other account are left out.
    pub fn plaid_apply(&mut self, targets: &HashMap<String, Target>, added: &[PlaidTx], modified: &[PlaidTx], removed: &[String]) -> Result<Applied> {
        let currency = self.settings()?.currency;
        let digits = fraction_digits(&currency).unwrap_or(2);
        let categories: Vec<(i64, String, String)> = {
            let mut st = self.conn.prepare_cached("SELECT id, name, kind FROM categories WHERE deleted = 0 AND archived = 0 ORDER BY sort_order, id")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let category = |name: &str, kind: &str| categories.iter().find(|(_, n, k)| k == kind && n.eq_ignore_ascii_case(name)).map(|(id, _, _)| *id);
        // Oldest first, so the two halves of a card payment meet the same way in one batch as
        // across two.
        let mut batch: Vec<&PlaidTx> = added.iter().chain(modified).filter(|t| !t.pending).collect();
        batch.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.transaction_id.cmp(&b.transaction_id)));
        let mut out = Applied::default();
        let now = now_ms();
        let tx = self.conn.transaction()?;
        for t in batch {
            let Some(target) = targets.get(&t.account_id) else { continue };
            let Some(date) = parse_date(&t.date) else { continue };
            if target.since.as_deref().and_then(parse_date).is_some_and(|since| date < since) {
                continue;
            }
            let uid = format!("{UID_PREFIX}{}", t.transaction_id.trim());
            if t.transaction_id.trim().is_empty() {
                continue;
            }
            let known = tx.prepare_cached(
                "SELECT EXISTS (SELECT 1 FROM transactions WHERE uid = ?1) OR EXISTS (SELECT 1 FROM tombstones WHERE tbl = 'transactions' AND uid = ?1)",
            )?.query_row([&uid], |r| r.get::<_, bool>(0))?;
            if known {
                continue;
            }
            if !target.any_currency && t.iso_currency_code.as_deref().is_some_and(|c| !c.trim().eq_ignore_ascii_case(&currency)) {
                out.other_currency += 1;
                continue;
            }
            let Some(scaled) = parse_scaled(&t.amount, digits) else { continue };
            let (amount, ty) = match scaled.value {
                0 => continue,
                v if v > 0 => (v, TxType::Expense),
                v => (v.saturating_neg(), TxType::Income),
            };
            if amount > MAX_AMOUNT {
                continue;
            }
            let day = date.to_string();
            let account = target.account_id;
            // An import wrote it already: the same account, amount and direction, close by.
            let imported = tx.prepare_cached(
                "SELECT EXISTS (SELECT 1 FROM transactions WHERE deleted = 0 AND amount = ?2 AND uid NOT LIKE 'plaid:%'
                   AND abs(julianday(date) - julianday(?3)) <= ?5
                   AND ((type = ?4 AND account_id = ?1)
                     OR (type = 'TRANSFER' AND ((?4 = 'EXPENSE' AND account_id = ?1) OR (?4 = 'INCOME' AND to_account_id = ?1)))))",
            )?.query_row(params![account, amount, day, name_of(&ty), DUPLICATE_DAYS], |r| r.get::<_, bool>(0))?;
            if imported {
                out.matched += 1;
                continue;
            }
            // The other half of a card payment, written earlier and untouched since: money out of
            // one followed account and into a followed credit card.
            let (want, credit_side) = if ty == TxType::Expense { ("INCOME", "t.account_id") } else { ("EXPENSE", "?1") };
            let half: Option<(i64, i64, String)> = tx.prepare_cached(&format!(
                "SELECT t.id, t.account_id, t.date FROM transactions t
                 WHERE t.deleted = 0 AND t.uid LIKE 'plaid:%' AND t.updated_at = t.created_at AND t.type = ?2
                   AND t.amount = ?3 AND t.account_id <> ?1 AND abs(julianday(t.date) - julianday(?4)) <= ?5
                   AND EXISTS (SELECT 1 FROM accounts a WHERE a.id = {credit_side} AND a.type = 'CREDIT' AND a.deleted = 0)
                 ORDER BY abs(julianday(t.date) - julianday(?4)), t.id LIMIT 1"
            ))?.query_row(params![account, want, amount, day, TRANSFER_DAYS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
            if let Some((other_id, other_account, other_date)) = half {
                // From the account the money left, to the card, on the day it left.
                let (from, to, on) = if ty == TxType::Expense { (account, other_account, day.clone()) } else { (other_account, account, other_date) };
                tx.prepare_cached(
                    "UPDATE transactions SET type = 'TRANSFER', account_id = ?2, to_account_id = ?3, category_id = NULL, date = ?4,
                            updated_at = MAX(?5, MIN(updated_at, 4000000000000000) + 1) WHERE id = ?1",
                )?.execute(params![other_id, from, to, on, now])?;
                // This half is now inside that transfer: it is never written on its own.
                tx.prepare_cached("INSERT OR IGNORE INTO tombstones (tbl, uid, updated_at) VALUES ('transactions', ?1, ?2)")?.execute(params![uid, now])?;
                out.transfers += 1;
                continue;
            }
            let kind = if ty == TxType::Income { "INCOME" } else { "EXPENSE" };
            let category_id = category_for(t.category_primary.as_deref(), t.category_detailed.as_deref(), ty == TxType::Income).and_then(|n| category(n, kind));
            let note = t.merchant_name.as_deref().map(str::trim).filter(|n| !n.is_empty()).or(t.name.as_deref().map(str::trim)).unwrap_or("");
            tx.prepare_cached(
                "INSERT INTO transactions (uid, type, amount, date, account_id, category_id, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            )?.execute(params![uid, name_of(&ty), amount, day, account, category_id, note, now])?;
            out.written += 1;
        }
        for id in removed {
            out.removed += tx.prepare_cached(
                "UPDATE transactions SET deleted = 1, updated_at = MAX(?2, MIN(updated_at, 4000000000000000) + 1)
                 WHERE uid = ?1 AND deleted = 0 AND type <> 'TRANSFER' AND updated_at = created_at",
            )?.execute(params![format!("{UID_PREFIX}{}", id.trim()), now])?;
        }
        tx.commit()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::{TxInput, TxPatch};

    fn tx(id: &str, account: &str, amount: &str, date: &str) -> PlaidTx {
        PlaidTx {
            transaction_id: id.into(), account_id: account.into(), amount: amount.into(), iso_currency_code: Some("CAD".into()),
            date: date.into(), name: Some(format!("name {id}")), ..Default::default()
        }
    }

    fn account(l: &mut Ledger, name: &str, kind: &str) -> i64 {
        l.plaid_account(&BankAccount { name, mask: name, kind, institution: "Desjardins", ..Default::default() }).unwrap().0
    }

    fn targets(pairs: &[(&str, i64)]) -> HashMap<String, Target> {
        pairs.iter().map(|(p, a)| (p.to_string(), Target { account_id: *a, since: None, any_currency: false })).collect()
    }

    type Entry = (String, TxType, i64, String, i64, Option<i64>, Option<i64>, String);

    /// The live entries, by uid.
    fn entries(l: &Ledger) -> Vec<Entry> {
        let mut st = l.conn.prepare(
            "SELECT uid, type, amount, date, account_id, to_account_id, category_id, note FROM transactions WHERE deleted = 0 ORDER BY uid",
        ).unwrap();
        let rows = st.query_map([], |r| {
            let ty: String = r.get(1)?;
            Ok((r.get(0)?, crate::ledger::parse_name(&ty).unwrap(), r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
        }).unwrap();
        rows.collect::<std::result::Result<_, _>>().unwrap()
    }

    #[test]
    fn a_purchase_is_an_expense_and_money_in_is_income_once() {
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        let t = targets(&[("p-chq", chq)]);
        let mut grocery = tx("a", "p-chq", "12.34", "2026-11-03");
        grocery.merchant_name = Some("IGA".into());
        grocery.category_primary = Some("FOOD_AND_DRINK".into());
        grocery.category_detailed = Some("FOOD_AND_DRINK_GROCERIES".into());
        let mut pay = tx("b", "p-chq", "-1500", "2026-11-04");
        pay.category_primary = Some("INCOME".into());
        pay.category_detailed = Some("INCOME_WAGES".into());
        let mut pending = tx("c", "p-chq", "3", "2026-11-05");
        pending.pending = true;
        let other = tx("d", "p-elsewhere", "9", "2026-11-05");
        let out = l.plaid_apply(&t, &[grocery.clone(), pay.clone(), pending, other], &[], &[]).unwrap();
        assert_eq!((out.written, out.transfers, out.matched), (2, 0, 0));
        let cats = l.categories().unwrap();
        let id = |n: &str| cats.iter().find(|c| c.name == n).map(|c| c.id);
        let e = entries(&l);
        assert_eq!(e[0], ("plaid:a".into(), TxType::Expense, 1234, "2026-11-03".into(), chq, None, id("Groceries"), "IGA".into()));
        assert_eq!(e[1], ("plaid:b".into(), TxType::Income, 150000, "2026-11-04".into(), chq, None, id("Salary"), "name b".into()));
        // Asked again (a cursor lost, a retry): nothing is written twice.
        assert_eq!(l.plaid_apply(&t, &[grocery, pay], &[], &[]).unwrap(), Applied::default());
    }

    #[test]
    fn a_deleted_entry_stays_deleted_and_since_holds_older_ones_back() {
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        let mut t = targets(&[("p", chq)]);
        l.plaid_apply(&t, &[tx("a", "p", "5", "2026-11-03")], &[], &[]).unwrap();
        let id: i64 = l.conn.query_row("SELECT id FROM transactions WHERE uid = 'plaid:a'", [], |r| r.get(0)).unwrap();
        l.tx_delete(id).unwrap();
        assert_eq!(l.plaid_apply(&t, &[tx("a", "p", "5", "2026-11-03")], &[], &[]).unwrap().written, 0);
        t.get_mut("p").unwrap().since = Some("2026-11-10".into());
        assert_eq!(l.plaid_apply(&t, &[tx("old", "p", "5", "2026-11-09"), tx("new", "p", "5", "2026-11-10")], &[], &[]).unwrap().written, 1);
    }

    #[test]
    fn a_card_payment_is_one_transfer_whichever_half_comes_first() {
        for card_first in [false, true] {
            let mut l = Ledger::open_in_memory().unwrap();
            let chq = account(&mut l, "Chequing", "checking");
            let card = account(&mut l, "Visa", "credit card");
            let t = targets(&[("p-chq", chq), ("p-card", card)]);
            let out_of_chq = tx("out", "p-chq", "250.00", "2026-11-10");
            let into_card = tx("in", "p-card", "-250", "2026-11-12");
            let (first, second) = if card_first { (into_card.clone(), out_of_chq.clone()) } else { (out_of_chq.clone(), into_card.clone()) };
            l.plaid_apply(&t, &[first], &[], &[]).unwrap();
            let out = l.plaid_apply(&t, &[second], &[], &[]).unwrap();
            assert_eq!((out.written, out.transfers), (0, 1), "card first: {card_first}");
            let e = entries(&l);
            assert_eq!(e.len(), 1);
            let (_, ty, amount, date, from, to, category, _) = &e[0];
            assert_eq!((*ty, *amount, date.as_str(), *from, *to, *category), (TxType::Transfer, 25000, "2026-11-10", chq, Some(card), None));
            // Neither half comes back on its own.
            assert_eq!(l.plaid_apply(&t, &[out_of_chq, into_card], &[], &[]).unwrap(), Applied::default());
        }
        // In one batch it is the same, and two equal amounts between two chequing accounts are
        // not a card payment.
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        let card = account(&mut l, "Visa", "credit card");
        let sav = account(&mut l, "Savings", "savings");
        let t = targets(&[("p-chq", chq), ("p-card", card), ("p-sav", sav)]);
        let out = l.plaid_apply(&t, &[tx("in", "p-card", "-80", "2026-11-12"), tx("out", "p-chq", "80", "2026-11-11"), tx("s", "p-sav", "-80", "2026-11-11")], &[], &[]).unwrap();
        assert_eq!((out.written, out.transfers), (2, 1));
    }

    #[test]
    fn an_imported_entry_is_matched_and_skipped() {
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        l.tx_add(&TxInput { r#type: TxType::Expense, amount: 4_20, date: "2026-11-03".into(), account_id: chq, to_account_id: None, category_id: None, note: Some("from a CSV".into()) }).unwrap();
        let t = targets(&[("p", chq)]);
        let out = l.plaid_apply(&t, &[tx("a", "p", "4.2", "2026-11-05"), tx("b", "p", "4.20", "2026-11-06")], &[], &[]).unwrap();
        assert_eq!((out.written, out.matched), (1, 1));
    }

    #[test]
    fn a_removed_transaction_deletes_only_an_untouched_entry() {
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        let t = targets(&[("p", chq)]);
        l.plaid_apply(&t, &[tx("a", "p", "5", "2026-11-03"), tx("b", "p", "6", "2026-11-03")], &[], &[]).unwrap();
        let b: i64 = l.conn.query_row("SELECT id FROM transactions WHERE uid = 'plaid:b'", [], |r| r.get(0)).unwrap();
        l.tx_update(b, &TxPatch { note: Some("lunch with Sam".into()), ..Default::default() }).unwrap();
        let out = l.plaid_apply(&t, &[], &[], &["a".into(), "b".into(), "never".into()]).unwrap();
        assert_eq!(out.removed, 1);
        assert_eq!(entries(&l).iter().map(|e| e.0.as_str()).collect::<Vec<_>>(), ["plaid:b"]);
    }

    #[test]
    fn another_currency_is_left_out_unless_the_bank_mislabels_it() {
        let mut l = Ledger::open_in_memory().unwrap();
        let chq = account(&mut l, "Chequing", "checking");
        let mut usd = tx("a", "p", "5", "2026-11-03");
        usd.iso_currency_code = Some("USD".into());
        let mut t = targets(&[("p", chq)]);
        assert_eq!(l.plaid_apply(&t, std::slice::from_ref(&usd), &[], &[]).unwrap().other_currency, 1);
        t.get_mut("p").unwrap().any_currency = true;
        assert_eq!(l.plaid_apply(&t, &[usd], &[], &[]).unwrap().written, 1);
    }

    #[test]
    fn a_bank_account_lands_in_the_chosen_or_imported_account_else_a_new_one() {
        let mut l = Ledger::open_in_memory().unwrap();
        let (visa, visa_uid) = l.plaid_account(&BankAccount { name: "Visa Desjardins", mask: "4321", kind: "credit card", institution: "Desjardins", into: None }).unwrap();
        let made = l.accounts().unwrap().into_iter().find(|a| a.id == visa).unwrap();
        assert_eq!((made.name.as_str(), made.r#type, made.institution.as_str()), ("Visa Desjardins", AccountType::Credit, "Desjardins"));
        // The same institution and number find it again, in any case.
        assert_eq!(l.plaid_account(&BankAccount { name: "Other", mask: "4321", kind: "credit card", institution: "desjardins", into: None }).unwrap().0, visa);
        // The chosen account wins; a chosen account that is gone falls back.
        let (chq, chq_uid) = l.plaid_account(&BankAccount { name: "Chèques", mask: "1111", kind: "checking", institution: "Desjardins", into: None }).unwrap();
        assert_eq!(l.plaid_account(&BankAccount { into: Some(&chq_uid), mask: "4321", institution: "Desjardins", ..Default::default() }).unwrap(), (chq, chq_uid));
        assert_eq!(l.plaid_account(&BankAccount { into: Some("gone"), mask: "4321", institution: "Desjardins", ..Default::default() }).unwrap(), (visa, visa_uid));
    }
}
