//! Accounts, Investments and Categories: the writes Tally RS makes through its own connection,
//! read back through relay-money, and the words the pages print.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use relay_money::model::{AccountType, CategoryKind, Registration, SecurityKind, TxType};
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::views::{TxInput, TxQuery};
use tally::accounts_store::{self as accounts, Draft};
use tally::categories_store::{self as categories, CategoryUse};
use tally::invest_logic as invest;
use tally::model::{self, Store};

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

/// A ledger's folder, removed when the test is done with it.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A ledger on disk of its own (the second connection needs a file).
fn store() -> (Store, Dir) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-rs-money-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    (Store::open(&dir.join("tally.db")).expect("a ledger opens"), Dir(dir))
}

fn day(s: &str) -> relay_money::Date {
    s.parse().unwrap()
}

fn spend(store: &mut Store, account: i64, category: Option<i64>, amount: i64) -> i64 {
    let today = model::today().to_string();
    store.add(&TxInput { r#type: TxType::Expense, amount, date: today, account_id: account, to_account_id: None, category_id: category, note: None }).unwrap().id
}

fn category_id(store: &Store, name: &str) -> i64 {
    store.ledger.categories().unwrap().into_iter().find(|c| c.name == name).unwrap().id
}

// Categories

#[test]
fn a_category_added_here_is_read_back_by_relay_money_and_numbered_for_sync() {
    let (store, _dir) = store();
    let before = store.ledger.cursor().unwrap();
    let id = categories::add(&store, "  Coffee  ", CategoryKind::Expense, "coffee", 7).unwrap();
    let c = store.ledger.categories().unwrap().into_iter().find(|c| c.id == id).unwrap();
    assert_eq!((c.name.as_str(), c.kind, c.icon.as_str(), c.color, c.archived), ("Coffee", CategoryKind::Expense, "coffee", 7, false));
    // A uid and a stamp as relay-money writes them, and the sync trigger numbered the row.
    let (uid, stamp, seq): (String, i64, i64) =
        store.sql().unwrap().query_row("SELECT uid, updated_at, seq FROM categories WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(uid.len(), 36);
    assert!(stamp > 1_700_000_000_000);
    assert!(seq > before);
    // It goes at the end of its kind.
    let order: i64 = store.sql().unwrap().query_row("SELECT sort_order FROM categories WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    let max: i64 = store.sql().unwrap().query_row("SELECT MAX(sort_order) FROM categories WHERE kind = 'EXPENSE'", [], |r| r.get(0)).unwrap();
    assert_eq!(order, max);
}

#[test]
fn a_category_edit_restamps_and_an_unchanged_save_writes_nothing() {
    let (store, _dir) = store();
    let id = category_id(&store, "Dining");
    let seq = |s: &Store| -> (i64, i64) { s.sql().unwrap().query_row("SELECT seq, updated_at FROM categories WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
    let (seq0, at0) = seq(&store);
    categories::update(&store, id, "Eating out", "pizza", 3, true).unwrap();
    let c = store.ledger.categories().unwrap().into_iter().find(|c| c.id == id).unwrap();
    assert_eq!((c.name.as_str(), c.icon.as_str(), c.color, c.archived), ("Eating out", "pizza", 3, true));
    let (seq1, at1) = seq(&store);
    assert!(seq1 > seq0 && at1 > at0);
    categories::update(&store, id, "Eating out", "pizza", 3, true).unwrap();
    assert_eq!(seq(&store), (seq1, at1));
}

#[test]
fn deleting_a_category_moves_its_entries_and_bills_and_takes_its_budget() {
    let (mut store, _dir) = store();
    let account = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let (dining, groceries) = (category_id(&store, "Dining"), category_id(&store, "Groceries"));
    let a = spend(&mut store, account, Some(dining), 1_200);
    let b = spend(&mut store, account, Some(dining), 800);
    store.ledger.budget_set(Some(dining), 30_000).unwrap();
    assert_eq!(categories::category_use(&store, dining).unwrap(), CategoryUse { entries: 2, bills: 0, budget: Some(30_000) });

    categories::delete(&store, dining, Some(groceries)).unwrap();
    assert!(store.ledger.categories().unwrap().iter().all(|c| c.id != dining));
    assert_eq!(store.ledger.tx(a).unwrap().category_id, Some(groceries));
    assert_eq!(store.ledger.tx(b).unwrap().category.as_deref(), Some("Groceries"));
    assert_eq!(store.ledger.budget(Some(dining)).unwrap(), None);
    // The row stays, soft-deleted, so sync carries the delete.
    let deleted: bool = store.sql().unwrap().query_row("SELECT deleted FROM categories WHERE id = ?1", [dining], |r| r.get(0)).unwrap();
    assert!(deleted);
}

#[test]
fn deleting_a_category_with_nowhere_to_go_leaves_its_entries_uncategorised_and_restore_brings_it_back() {
    let (mut store, _dir) = store();
    let account = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    let travel = category_id(&store, "Travel");
    let t = spend(&mut store, account, Some(travel), 5_000);
    categories::delete(&store, travel, None).unwrap();
    assert_eq!(store.ledger.tx(t).unwrap().category_id, None);
    assert!(categories::delete(&store, travel, None).is_err());

    let gifts = category_id(&store, "Gifts");
    store.ledger.budget_set(Some(gifts), 5_000).unwrap();
    categories::delete(&store, gifts, None).unwrap();
    categories::restore(&store, gifts, Some(5_000)).unwrap();
    assert!(store.ledger.categories().unwrap().iter().any(|c| c.id == gifts));
    assert_eq!(store.ledger.budget(Some(gifts)).unwrap(), Some(5_000));
}

#[test]
fn category_names_are_unique_within_a_kind_without_case() {
    let (store, _dir) = store();
    let all = store.ledger.categories().unwrap();
    assert_eq!(categories::name_problem("  ", CategoryKind::Expense, None, &all).as_deref(), Some(categories::MISSING_NAME));
    assert_eq!(categories::name_problem("groceries", CategoryKind::Expense, None, &all).as_deref(), Some("You already have a spending category called Groceries"));
    // The other kind, or the category itself, is no clash.
    assert_eq!(categories::name_problem("Groceries", CategoryKind::Income, None, &all), None);
    let id = category_id(&store, "Groceries");
    assert_eq!(categories::name_problem("GROCERIES", CategoryKind::Expense, Some(id), &all), None);
    assert_eq!(categories::suggest_color(&[0, 1, 2, 4]), 3);
}

#[test]
fn the_categories_page_reads_this_month_by_category() {
    let (mut store, _dir) = store();
    let account = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let (dining, groceries) = (category_id(&store, "Dining"), category_id(&store, "Groceries"));
    spend(&mut store, account, Some(dining), 3_000);
    spend(&mut store, account, Some(dining), 3_000);
    spend(&mut store, account, Some(groceries), 4_000);
    spend(&mut store, account, None, 500);
    let s = categories::read_state(&store, model::today()).unwrap();
    let k = &s.spending;
    assert_eq!(k.period_total, 10_500);
    assert_eq!(k.used_this_period, 2);
    assert_eq!(k.most_used.as_deref(), Some("Dining"));
    assert_eq!(k.segments.iter().map(|g| (g.name.as_str(), g.total)).collect::<Vec<_>>(), vec![("Dining", 6_000), ("Groceries", 4_000), ("Uncategorized", 500)]);
    let line = k.active.iter().find(|l| l.id == dining).unwrap();
    assert_eq!(categories::tile_line(line), "2 entries · 57% of this month");
    let none = k.active.iter().find(|l| l.name == "Health").unwrap();
    assert_eq!(categories::tile_line(none), "0 entries");
}

#[test]
fn every_icon_the_picker_offers_is_drawn_once() {
    let mut seen = Vec::new();
    for (_, keys) in categories::ICON_GROUPS {
        for k in *keys {
            assert!(tally::category_icons::path(k).is_some(), "{k} has no drawing");
            assert!(!seen.contains(k), "{k} twice");
            seen.push(*k);
        }
    }
    for c in relay_money::model::DEFAULT_CATEGORIES {
        assert!(seen.contains(&c.icon), "{} is not offered", c.icon);
    }
}

#[test]
fn the_delete_sheet_names_what_moves() {
    let u = CategoryUse { entries: 3, bills: 1, budget: Some(20_000) };
    assert_eq!(categories::move_heading(&u), "Move its 3 entries and 1 bill to");
    assert_eq!(categories::move_note("Dining", Some("$200.00")), "Dining is deleted, with its $200.00 budget. This cannot be undone.");
    assert_eq!(categories::leave_title(&u), "Leave them uncategorized");
    assert_eq!(categories::deleted_line("Dining", &u, Some("$200.00"), Some("Groceries")), "Dining deleted with its $200.00 budget · 3 entries and 1 bill moved to Groceries");
    assert_eq!(categories::deleted_line("Dining", &CategoryUse::default(), None, None), "Dining deleted");
}

// Accounts

#[test]
fn a_new_account_is_added_through_relay_money_and_becomes_the_default() {
    let (mut store, _dir) = store();
    let draft = Draft { name: "Desjardins Visa".into(), kind: AccountType::Credit, opening_text: "412.20".into(), owe: true, is_default: true, ..Draft::default() };
    let id = accounts::save(&mut store, None, &draft, 2).unwrap();
    let a = store.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap();
    assert_eq!((a.name.as_str(), a.r#type, a.balance), ("Desjardins Visa", AccountType::Credit, -41_220));
    assert_eq!(accounts::default_account(&store), Some(id));
    assert_eq!(accounts::default_name(&store).unwrap().as_deref(), Some("Desjardins Visa"));
}

#[test]
fn an_account_edit_changes_its_type_opening_and_registration() {
    let (mut store, _dir) = store();
    let id = store.ledger.account_add("Wealthsimple TFSA", AccountType::Savings, 10_000, None, "").unwrap().id;
    let s = accounts::stored(&store, id).unwrap().unwrap();
    let mut d = accounts::draft_of(&s, None, 2);
    assert_eq!(d.opening_text, "100.00");
    d.kind = AccountType::Investment;
    d.registration = Some(Registration::Tfsa);
    d.opening_text = "250".into();
    accounts::save(&mut store, Some(id), &d, 2).unwrap();
    let a = store.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap();
    assert_eq!((a.r#type, a.registration, a.balance, a.institution.as_str()), (AccountType::Investment, Some(Registration::Tfsa), 25_000, "Wealthsimple"));
    // Back to savings: the registration goes, as only an investment account keeps one.
    d.kind = AccountType::Savings;
    accounts::save(&mut store, Some(id), &d, 2).unwrap();
    let a = store.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap();
    assert_eq!((a.r#type, a.registration), (AccountType::Savings, None));
    // Archived, it stops being the default.
    accounts::set_default_account(&mut store, Some(id)).unwrap();
    d.archived = true;
    d.is_default = true;
    accounts::save(&mut store, Some(id), &d, 2).unwrap();
    assert_eq!(accounts::default_account(&store), None);
    assert!(store.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap().archived);
}

#[test]
fn deleting_an_account_takes_its_entries_and_says_what_its_transfers_did() {
    let (mut store, _dir) = store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 100_000, None, "").unwrap().id;
    let visa = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    spend(&mut store, visa, None, 4_000);
    spend(&mut store, visa, None, 1_000);
    // Chequing paid the card twice.
    let today = model::today().to_string();
    for amount in [2_000, 3_000] {
        store.add(&TxInput { r#type: TxType::Transfer, amount, date: today.clone(), account_id: chequing, to_account_id: Some(visa), category_id: None, note: None }).unwrap();
    }
    let u = accounts::account_use(&store, visa).unwrap();
    assert_eq!((u.entries, u.bills), (4, 0));
    assert_eq!(u.others.len(), 1);
    assert_eq!((u.others[0].name.as_str(), u.others[0].transfers, u.others[0].change), ("Chequing", 2, 5_000));
    assert_eq!(
        accounts::delete_line("Visa", &u, &fmt(), true),
        "Delete Visa? Its 4 entries go with it. Chequing goes up by $50.00, since 2 transfers between them go too. This cannot be undone. Archiving keeps all of it and takes the account out of the pickers."
    );
    let before = store.ledger.accounts().unwrap().into_iter().find(|a| a.id == chequing).unwrap().balance;
    accounts::delete(&mut store, visa).unwrap();
    let after = store.ledger.accounts().unwrap();
    assert!(after.iter().all(|a| a.id != visa));
    assert_eq!(after.iter().find(|a| a.id == chequing).unwrap().balance, before + 5_000);
    let page = store.ledger.tx_list(&TxQuery { limit: Some(100), ..Default::default() }, model::today()).unwrap();
    assert!(page.transactions.is_empty());
    assert_eq!(accounts::deleted_line("Visa", &u), "Visa deleted with its 4 entries");
}

#[test]
fn a_recorded_value_can_be_taken_back() {
    let (mut store, _dir) = store();
    let id = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    store.ledger.value_set(id, "2026-10-11", 841_200).unwrap();
    let values = accounts::values(&store, id).unwrap();
    assert_eq!(values.len(), 1);
    let row = accounts::rows(&store).unwrap().into_iter().find(|r| r.view.id == id).unwrap();
    assert_eq!((row.view.balance, row.valued_on), (841_200, Some(day("2026-10-11"))));
    assert!(row.tracked());
    accounts::delete_value(&store, values[0].id).unwrap();
    assert!(accounts::values(&store, id).unwrap().is_empty());
    assert_eq!(store.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap().balance, 0);
}

#[test]
fn the_accounts_page_leads_with_spending_and_says_balances_in_words() {
    let (mut store, _dir) = store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 318_450, None, "").unwrap().id;
    let visa = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    spend(&mut store, chequing, None, 6_400);
    spend(&mut store, visa, None, 3_600);
    accounts::set_default_account(&mut store, Some(chequing)).unwrap();
    let s = accounts::read_state(&store, model::today()).unwrap();
    assert_eq!(s.month_spent, 10_000);
    assert_eq!(accounts::top_line(&s), "64% of it from Chequing");
    assert_eq!(s.default_name.as_deref(), Some("Chequing"));
    assert!(s.can_transfer() && s.any_tracked());
    // A card counts what was spent on it as owed, even with no starting balance.
    assert_eq!((s.tracked_held, s.tracked_owed), (312_050, 3_600));
    assert_eq!(accounts::caption(&s.active[0], &fmt()), "Chequing · default · holds $3,120.50");
    // A card with no starting balance owes what was spent on it.
    assert_eq!(accounts::caption(&s.active[1], &fmt()), "Credit card · owes $36.00");
    assert_eq!(accounts::balance_words(AccountType::Credit, -41_220, &fmt()), "owes $412.20");
    assert_eq!(accounts::balance_words(AccountType::Cash, -5_000, &fmt()), "$50.00 below zero");
    let free = accounts::free_templates(&["desjardins visa".to_string()]);
    assert!(free.iter().all(|t| t.name != "Desjardins Visa") && free.len() == 8);
}

#[test]
fn the_account_editor_checks_its_draft() {
    let d = Draft { opening_text: "12,5x".into(), ..Draft::default() };
    let (name, opening) = accounts::problems(&d, 2);
    assert_eq!(name, Some("Give the account a name"));
    assert_eq!(opening, None, "stray letters are dropped as the field reads them");
    let d = Draft { name: "Visa".into(), opening_text: "".into(), owe: true, ..Draft::default() };
    assert_eq!(d.signed_opening(2), Some(0));
    let d = Draft { name: "Visa".into(), opening_text: "10".into(), owe: true, ..Draft::default() };
    assert_eq!(d.signed_opening(2), Some(-1_000));
    assert_eq!(accounts::institution_for("Wealthsimple TFSA", ""), "Wealthsimple");
    assert_eq!(accounts::institution_for("TFSA", "Questrade"), "Questrade");
}

// Investments

#[test]
fn room_set_here_reads_back_on_the_page() {
    let (mut store, _dir) = store();
    let today = model::today();
    let id = store.ledger.account_add("Wealthsimple TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "Wealthsimple").unwrap().id;
    store.ledger.value_set(id, &today.to_string(), 500_000).unwrap();
    let year = invest::room_year(Registration::Tfsa, today);
    store.ledger.room_set(Registration::Tfsa, year, 700_000).unwrap();
    let p = store.ledger.portfolio(today).unwrap();
    let rows = accounts::rows(&store).unwrap();
    let v = invest::view(Some(&p), &rows, None);
    assert_eq!(v.accounts.len(), 1);
    assert_eq!(v.worth, 500_000);
    assert!(v.accounts[0].by_hand);
    let room = v.room.iter().find(|r| r.registration == Registration::Tfsa && r.year == year).unwrap();
    assert_eq!(room.room, Some(700_000));
    assert_eq!(invest::room_reading(room, &fmt()), "$0 of $7,000");
    assert!(invest::room_text(room, today, &fmt()).starts_with("$7,000 left"));
    // A figure of zero removes it.
    store.ledger.room_set(Registration::Tfsa, year, 0).unwrap();
    let p = store.ledger.portfolio(today).unwrap();
    assert!(p.room.iter().all(|r| r.year != year || r.room.is_none()));
    assert_eq!(invest::context(1, Some(today), today), format!("1 account · valued {}", invest::short(today, today)));
}

#[test]
fn a_lens_needs_two_kinds_and_narrows_the_page() {
    let (mut store, _dir) = store();
    let today = model::today();
    let a = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    store.ledger.value_set(a, &today.to_string(), 100_000).unwrap();
    let rows = accounts::rows(&store).unwrap();
    assert!(invest::view(None, &rows, None).lenses.is_empty());
    let b = store.ledger.account_add("RRSP", AccountType::Investment, 0, Some(Registration::Rrsp), "").unwrap().id;
    store.ledger.value_set(b, &today.to_string(), 300_000).unwrap();
    let rows = accounts::rows(&store).unwrap();
    let v = invest::view(None, &rows, None);
    assert_eq!(v.lenses.iter().map(|l| l.label).collect::<Vec<_>>(), vec!["TFSA", "RRSP"]);
    assert_eq!(v.allocation.iter().map(|s| (s.label, s.share_bps)).collect::<Vec<_>>(), vec![("RRSP", 7_500), ("TFSA", 2_500)]);
    let narrowed = invest::view(None, &rows, Some(Some(Registration::Rrsp)));
    assert_eq!((narrowed.worth, narrowed.accounts.len(), narrowed.allocation.len()), (300_000, 1, 0));
}

#[test]
fn investment_words() {
    assert_eq!(invest::percent_text(812, true), "+8.1%");
    assert_eq!(invest::percent_text(-45, false), "−0.5%");
    assert_eq!(invest::percent_text(4, true), "0.0%");
    assert_eq!(invest::share_percent(30), "under 1%");
    assert_eq!(invest::share_percent(1_849), "18%");
    assert_eq!(invest::quantity_text(1_250_000_000), "12.5");
    assert_eq!(invest::quantity_text(420_000), "0.0042");
    assert_eq!(invest::units_line(100_000_000, SecurityKind::Etf), "1 share");
    assert_eq!(invest::gain_text(41_200, Some(812), &fmt()), "+$412 · +8.1%");
    assert_eq!(invest::monthly_to_fill(700_000, day("2026-11-19"), day("2026-12-31"), 100), Some(350_000));
    assert_eq!(invest::monthly_to_fill(700_000, day("2027-01-01"), day("2026-12-31"), 100), None);
    assert_eq!(invest::stale_line(day("2026-09-01"), day("2026-11-19")), "2 months since the last report");
    assert_eq!(invest::registration_named("TFSA"), Some(Registration::Tfsa));
    assert_eq!(invest::registration_named("NON_REGISTERED"), Some(Registration::NonRegistered));
    let pace = invest::room_pace_fraction(Registration::Tfsa, 2026, day("2026-07-02"));
    assert!((pace - 0.5).abs() < 0.01, "{pace}");
}

// The premium Accounts and Investments readings

#[test]
fn net_worth_counts_accounts_with_a_balance_and_reads_last_month_end() {
    let (mut store, _dir) = store();
    let today = model::today();
    let since = accounts::last_month_end(today);
    assert_eq!(since, today.first_of_month().yesterday().unwrap());
    let chequing = store.ledger.account_add("Desjardins chequing", AccountType::Chequing, 100_000, None, "").unwrap().id;
    let visa = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    spend(&mut store, chequing, None, 20_000);
    spend(&mut store, visa, None, 5_000);
    let rows = accounts::rows(&store).unwrap();
    let w = accounts::worth_reading(&store, &rows, today).unwrap();
    // A card counts from zero: what was spent on it is what it owes, so paying it moves nothing.
    assert_eq!((w.of(chequing), w.of(visa)), (Some(80_000), Some(-5_000)));
    assert_eq!(w.total(), 75_000);
    assert_eq!(w.change(), Some(-25_000));
    assert_eq!(accounts::change_line(&w, &fmt()).unwrap(), format!("−$250 since {}", since.strftime("%B %-d")));
    assert_eq!(accounts::balances_on(&store, since).unwrap()[&chequing], 100_000);

    // The card's next payment comes from the bill that pays it.
    let next = today.checked_add(jiff::Span::new().days(9)).unwrap();
    store
        .sql()
        .unwrap()
        .execute(
            "INSERT INTO recurring (uid, name, type, amount, account_id, to_account_id, frequency, anchor_date, next_date, updated_at)
             VALUES ('pay-visa', 'Card payment', 'TRANSFER', 61240, ?1, ?2, 'MONTHLY', ?3, ?3, 1)",
            rusqlite::params![chequing, visa, next.to_string()],
        )
        .unwrap();
    accounts::set_default_account(&mut store, Some(chequing)).unwrap();
    let s = accounts::read_state(&store, today).unwrap();
    let line_of = |id: i64| accounts::row_line(s.active.iter().find(|l| l.row.view.id == id).unwrap(), &s, &fmt());
    assert_eq!(line_of(chequing), "Default for new entries");
    assert_eq!(line_of(visa), format!("Payment {} · $612.40", next.strftime("%b %-d")));
    assert_eq!(accounts::Group::of(AccountType::Savings), accounts::Group::Everyday);
    assert_eq!(accounts::bank_name(&s.active[0].row), "Desjardins");
}

#[test]
fn the_value_chart_reads_each_month_end_from_recorded_values() {
    let (mut store, _dir) = store();
    let today = day("2026-11-19");
    let ends = invest::month_ends(today);
    assert_eq!(ends.len(), 12);
    assert_eq!((ends[0], ends[10], ends[11]), (day("2025-12-31"), day("2026-10-31"), today));
    let id = store.ledger.account_add("TFSA", AccountType::Investment, 500_000, Some(Registration::Tfsa), "").unwrap().id;
    store.ledger.value_set(id, "2026-06-15", 620_000).unwrap();
    let input = store.ledger.portfolio_input(today).unwrap();
    let at = |d: &str| {
        let b = accounts::balances_on(&store, day(d)).unwrap();
        invest::values_on(&input, &b, day(d))[&id]
    };
    // Before the value, what it opened at; after, the value; never from holdings.
    assert_eq!(at("2026-05-31"), (500_000, false));
    assert_eq!(at("2026-06-30"), (620_000, false));
    let history = vec![(day("2026-05-31"), [(id, 500_000)].into()), (day("2026-06-30"), [(id, 620_000)].into())];
    assert_eq!(invest::worth_series(&history, &[id]).iter().map(|(_, v)| *v).collect::<Vec<_>>(), vec![500_000, 620_000]);
    assert_eq!(invest::year_line(620_000, 500_000, &fmt()), "+$1,200 this year, 24.0%");
    assert_eq!(invest::year_line(50_000, 0, &fmt()), "+$500 this year");
}

#[test]
fn chart_gridlines_hold_the_values_at_round_amounts() {
    use tally::accounts_kit::nice_range;
    for (lo, hi) in [(1_500_000, 2_200_000), (935_000, 935_000), (0, 0), (-50_000, 120_000), (820_000, 935_000)] {
        let (floor, ceil, step) = nice_range(lo, hi);
        assert!(floor <= lo && ceil >= hi, "{lo}..{hi} -> {floor}..{ceil}");
        assert_eq!(ceil - floor, step * 2);
        assert!(step > 0);
    }
    assert_eq!(nice_range(1_500_000, 2_200_000), (1_500_000, 2_500_000, 500_000));
}
