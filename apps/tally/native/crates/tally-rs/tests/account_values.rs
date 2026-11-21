//! The delete dialog's word on another account that has a recorded value, and which bank an
//! account's name points at.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use relay_money::model::{AccountType, Registration, TxType};
use relay_money::views::TxInput;
use tally::accounts_store as accounts;
use tally::model::Store;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Store, Dir) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("tally-rs-account-values-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    (Store::open(&d.join("tally.db")).expect("a ledger opens"), Dir(d))
}

fn transfer(store: &mut Store, from: i64, to: i64, amount: i64, date: &str) {
    store
        .add(&TxInput { r#type: TxType::Transfer, amount, date: date.into(), account_id: from, to_account_id: Some(to), category_id: None, note: None })
        .unwrap();
}

#[test]
fn transfers_a_recorded_value_already_holds_do_not_move_the_other_balance() {
    let (mut store, _dir) = store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    transfer(&mut store, chequing, tfsa, 500_00, "2026-09-15");
    transfer(&mut store, chequing, tfsa, 500_00, "2026-10-15");
    store.ledger.value_set(tfsa, "2026-10-31", 1_050_00).unwrap();
    let u = accounts::account_use(&store, chequing).unwrap();
    assert_eq!(u.others.len(), 1);
    assert_eq!((u.others[0].transfers, u.others[0].change), (2, 0));

    // One after the value does move it.
    transfer(&mut store, chequing, tfsa, 200_00, "2026-11-05");
    let u = accounts::account_use(&store, chequing).unwrap();
    assert_eq!((u.others[0].transfers, u.others[0].change), (3, -200_00));
}

#[test]
fn a_bank_is_named_by_whole_words_only() {
    let (mut store, _dir) = store();
    for name in ["Outdoor savings", "TD chequing", "My National Bank card", "Wealthsimple Cash"] {
        store.ledger.account_add(name, AccountType::Chequing, 0, None, "").unwrap();
    }
    let rows = accounts::rows(&store).unwrap();
    let bank = |n: &str| accounts::bank_name(rows.iter().find(|r| r.view.name == n).unwrap());
    assert_eq!(bank("Outdoor savings"), "your bank");
    assert_eq!(bank("TD chequing"), "TD");
    assert_eq!(bank("My National Bank card"), "National Bank");
    assert_eq!(bank("Wealthsimple Cash"), "Wealthsimple");
}

#[test]
fn a_first_value_typed_today_is_not_a_gain_since_last_month() {
    let (mut store, _dir) = store();
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    let today = tally::model::today();
    store.ledger.value_set(tfsa, &today.to_string(), 20_000_00).unwrap();
    let rows = accounts::rows(&store).unwrap();
    let w = accounts::worth_reading(&store, &rows, today).unwrap();
    assert_eq!(w.total(), 20_000_00);
    assert_eq!(w.change(), Some(0));
}

#[test]
fn settings_and_goals_count_net_worth_as_the_accounts_page_does() {
    let (mut store, _dir) = store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    store.ledger.account_add("Savings", AccountType::Savings, 10_000_00, None, "").unwrap();
    let today = tally::model::today();
    store
        .add(&TxInput { r#type: TxType::Expense, amount: 3_000_00, date: today.to_string(), account_id: chequing, to_account_id: None, category_id: None, note: None })
        .unwrap();
    let rows = accounts::rows(&store).unwrap();
    let page = accounts::worth_reading(&store, &rows, today).unwrap().total();
    assert_eq!(page, 10_000_00);
    assert_eq!(tally::settings_store::counts(&store).net, page);
    assert_eq!(tally::plan_logic::worth_total(&store, today), page);
}

#[test]
fn a_tally_csv_in_a_currency_with_other_decimals_is_refused() {
    let (mut store, _dir) = store();
    store.ledger.set_currency("JPY").unwrap();
    let text = "date,type,amount,currency,category,account,to_account,note\n2026-11-02,EXPENSE,12.50,CAD,Food,Cash,,Lunch\n";
    let why = tally::import_store::import_tally_csv(&mut store, text).unwrap_err();
    assert!(why.contains("CAD"), "{why}");
    assert!(store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions.is_empty());
}

#[test]
fn a_negative_amount_under_one_unit_keeps_its_sign_as_text() {
    use tally::widgets::minor_text;
    assert_eq!(minor_text(-50, 2), "-0.50");
    assert_eq!(minor_text(-100_000, 2), "-1000.00");
    assert_eq!(minor_text(1_205, 2), "12.05");
    assert_eq!(minor_text(-7, 0), "-7");
}

fn goal(name: &str, kind: relay_money::model::GoalKind, account_id: Option<i64>, start: i64, target: i64) -> tally::plan_store::GoalInput {
    tally::plan_store::GoalInput { name: name.into(), target, target_date: None, color: 1, kind, account_id, percent: 10, start_date: None, start_amount: start }
}

#[test]
fn deleting_an_account_archives_the_goal_to_reach_an_amount_in_it() {
    use relay_money::model::GoalKind;
    let (mut store, _dir) = store();
    let savings = store.ledger.account_add("Savings", AccountType::Savings, 1_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    let reach = tally::plan_store::goal_save(store.sql().unwrap(), None, &goal("Savings to 3,000", GoalKind::Balance, Some(savings), 1_000_00, 3_000_00)).unwrap();
    let invest = tally::plan_store::goal_save(store.sql().unwrap(), None, &goal("Invest a tenth", GoalKind::Invest, Some(savings), 0, 0)).unwrap();
    let other = tally::plan_store::goal_save(store.sql().unwrap(), None, &goal("TFSA to 10k", GoalKind::Balance, Some(tfsa), 0, 10_000_00)).unwrap();

    let u = accounts::account_use(&store, savings).unwrap();
    assert_eq!(u.goals, vec!["Savings to 3,000".to_string()]);
    assert!(!u.is_empty(), "archiving the account would keep the goal, so it is offered");
    assert_eq!(
        accounts::delete_line("Savings", &u, &relay_money::money::MoneyFormatter::new("CAD", relay_money::money::Locale::new("en-CA")), true),
        "Delete Savings? It has no entries and no bills. Your goal Savings to 3,000 is archived, since it reads this account. This cannot be undone. Archiving keeps all of it and takes the account out of the pickers."
    );

    accounts::delete(&mut store, savings).unwrap();
    let read = |id| tally::plan_store::goal(store.sql().unwrap(), id).unwrap().unwrap();
    // Read against net worth, "Savings to 3,000" would flip to reached: it is put away instead.
    let g = read(reach);
    assert!(g.archived);
    assert_eq!(g.account_id, Some(savings));
    // A monthly goal follows every investment account again.
    let g = read(invest);
    assert!(!g.archived);
    assert_eq!(g.account_id, None);
    // A goal on another account is left alone.
    let g = read(other);
    assert_eq!((g.archived, g.account_id), (false, Some(tfsa)));
}

#[test]
fn a_goal_on_one_account_reads_it_as_the_accounts_page_does() {
    use relay_money::model::GoalKind;
    let (mut store, _dir) = store();
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 1_000_00, Some(Registration::Tfsa), "").unwrap().id;
    let today = tally::model::today();
    let id = tally::plan_store::goal_save(store.sql().unwrap(), None, &goal("TFSA to 5k", GoalKind::Balance, Some(tfsa), 1_000_00, 5_000_00)).unwrap();
    let g = tally::plan_store::goal(store.sql().unwrap(), id).unwrap().unwrap();
    let accounts_now = store.ledger.accounts().unwrap();
    // Holdings put the account at $4,200 while its ledger balance is still its $1,000 opening.
    let mut inputs = tally::plan_logic::GoalInputs { accounts: accounts_now.clone(), worth: 9_999_00, ..Default::default() };
    inputs.worth_of.insert(tfsa, 4_200_00);
    assert_eq!(tally::plan_logic::goal_line(&g, &inputs, today).saved, 4_200_00);
    // With nothing priced, the balance it holds.
    inputs.worth_of.clear();
    assert_eq!(tally::plan_logic::goal_line(&g, &inputs, today).saved, 1_000_00);
    // Its account gone, it reads where it started, never net worth.
    inputs.accounts.clear();
    assert_eq!(tally::plan_logic::goal_line(&g, &inputs, today).saved, 1_000_00);
    // And the reading the Plan loads carries the same values as the Accounts page.
    let rows = accounts::rows(&store).unwrap();
    let page = accounts::worth_reading(&store, &rows, today).unwrap();
    let w = tally::plan_logic::worth_now(&store, today);
    assert_eq!((w.total, w.of.get(&tfsa).copied()), (page.total(), page.of(tfsa)));
}

#[test]
fn the_largest_account_is_among_the_tracked_ones() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    // Chequing was given nowhere to start: its pay reads high but says nothing it holds.
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    store.ledger.account_add("Savings", AccountType::Savings, 500_00, None, "").unwrap();
    store
        .add(&TxInput { r#type: TxType::Income, amount: 3_000_00, date: today.to_string(), account_id: chequing, to_account_id: None, category_id: None, note: None })
        .unwrap();
    let s = accounts::read_state(&store, today).unwrap();
    assert_eq!(s.largest_tracked.as_deref(), Some("Savings"));
}

#[test]
fn an_imported_wealthsimple_account_is_not_connected_to_a_bank() {
    let (mut store, _dir) = store();
    let ws = store.ledger.account_add("Wealthsimple TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "Wealthsimple").unwrap().id;
    let linked = store.ledger.account_add("Desjardins Visa", AccountType::Credit, 0, None, "Desjardins").unwrap().id;
    let db = store.sql().unwrap();
    db.execute("UPDATE accounts SET external_ref = 'HQ7XFMC41CAD' WHERE id = ?1", [ws]).unwrap();
    db.execute("UPDATE accounts SET external_ref = '4321' WHERE id = ?1", [linked]).unwrap();
    let rows = accounts::rows(&store).unwrap();
    let connected = |id| rows.iter().find(|r| r.view.id == id).unwrap().connected();
    assert!(!connected(ws));
    assert!(connected(linked));
    // Once the Banks page has read what the server follows, that decides.
    store.set_pref(tally::banks_logic::FOLLOWED_PREF, "[]").unwrap();
    let rows = accounts::rows(&store).unwrap();
    assert!(rows.iter().all(|r| !r.connected()));
}

#[test]
fn a_statement_opening_counts_the_lines_before_its_first_balance() {
    use relay_money::bank_statements::{self, StatementRead};
    use tally::import_logic as import;
    let text = "Date,Description,Amount,Balance\n2026-10-01,COFFEE,-10.00,\n2026-10-02,METRO,-20.00,970.00\n";
    let StatementRead::Ok(st) = bank_statements::read(text, 2, false) else { panic!("reads") };
    let plan = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[]);
    assert_eq!((plan.opening_from_statement, plan.end_balance), (Some(1_000_00), Some(970_00)));
    assert_eq!(import::new_account_gap(&plan), None, "nothing left out, nothing to explain");
}

// Transfers and net worth

fn spend_from(store: &mut Store, account: i64, amount: i64, date: &str) {
    store.add(&TxInput { r#type: TxType::Expense, amount, date: date.into(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
}

/// Net worth as the Accounts page reads it, with Settings and the goals checked against it.
fn worth(store: &Store, today: relay_money::Date) -> accounts::Worth {
    let rows = accounts::rows(store).unwrap();
    let w = accounts::worth_reading(store, &rows, today).unwrap();
    assert_eq!(tally::settings_store::counts(store).net, w.total());
    assert_eq!(tally::plan_logic::worth_total(store, today), w.total());
    w
}

#[test]
fn moving_money_into_a_new_savings_account_keeps_net_worth() {
    let today = tally::model::today();
    let last_month = accounts::last_month_end(today).to_string();
    for date in [last_month, today.to_string()] {
        let (mut store, _dir) = store();
        let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
        let savings = store.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
        transfer(&mut store, chequing, savings, 300_00, &date);
        let w = worth(&store, today);
        assert_eq!((w.total(), w.change()), (1_000_00, Some(0)), "moved on {date}");
        assert_eq!((w.of(chequing), w.of(savings)), (Some(700_00), Some(300_00)));
        // A goal on the savings account alone reads what it holds.
        assert_eq!(tally::plan_logic::worth_now(&store, today).of.get(&savings), Some(&300_00));
    }
}

#[test]
fn a_transfer_between_two_accounts_given_nowhere_to_start_changes_nothing() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let savings = store.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    transfer(&mut store, chequing, savings, 300_00, &accounts::last_month_end(today).to_string());
    let before = worth(&store, today);
    transfer(&mut store, savings, cash, 100_00, &today.to_string());
    let after = worth(&store, today);
    assert_eq!((after.total(), after.change()), (before.total(), before.change()));
    assert_eq!((after.of(savings), after.of(cash)), (Some(200_00), Some(100_00)));
    // What is spent from the cash is gone from net worth, though the cash was given nowhere to start.
    spend_from(&mut store, cash, 100_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(cash)), (900_00, Some(-100_00), None));
}

#[test]
fn an_investment_account_built_by_transfers_counts_them_and_its_first_value_is_not_a_gain() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    transfer(&mut store, chequing, tfsa, 200_00, &accounts::last_month_end(today).to_string());
    transfer(&mut store, chequing, tfsa, 300_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (1_000_00, Some(0), Some(500_00)));
    // The Investments page reads it the same way.
    let rows = accounts::rows(&store).unwrap();
    assert_eq!(tally::invest_logic::view(None, &rows, None).worth, 500_00);

    // Its first value, typed today, holds both transfers and $20 more. With nowhere to start by
    // last month it read then as the value less this month's $300: the first value is not a
    // gain, and the $300 that left Chequing this month is not read as a loss.
    store.ledger.value_set(tfsa, &today.to_string(), 520_00).unwrap();
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (1_020_00, Some(0), Some(520_00)));
    assert_eq!(tally::invest_logic::worth_before(520_00, 300_00), 220_00);
    assert_eq!(tally::invest_logic::worth_before(100_00, 300_00), 0, "never a debt then");
}

#[test]
fn a_card_left_at_zero_owes_its_spending_and_paying_it_moves_nothing() {
    let (mut store, _dir) = store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 2_000_00, None, "").unwrap().id;
    let visa = store.ledger.account_add("Desjardins Visa", AccountType::Credit, 0, None, "").unwrap().id;
    let today = tally::model::today();
    store
        .add(&TxInput { r#type: TxType::Expense, amount: 1_000_00, date: today.to_string(), account_id: visa, to_account_id: None, category_id: None, note: None })
        .unwrap();
    let worth = |s: &Store| accounts::worth_reading(s, &accounts::rows(s).unwrap(), today).unwrap();
    let before = worth(&store);
    assert_eq!(before.total(), 1_000_00, "the card's debt counts");
    assert_eq!(before.of(visa), Some(-1_000_00));
    transfer(&mut store, chequing, visa, 1_000_00, &today.to_string());
    let after = worth(&store);
    assert_eq!(after.total(), before.total(), "paying the card moves nothing");
}

#[test]
fn a_one_day_statement_newest_first_opens_at_the_right_balance() {
    use relay_money::bank_statements::{self, StatementRead};
    use tally::import_logic as import;
    let text = "Date,Description,Amount,Balance\n2026-10-02,METRO,-50.00,850.00\n2026-10-02,RENT,-100.00,900.00\n";
    let StatementRead::Ok(st) = bank_statements::read(text, 2, false) else { panic!("reads") };
    let plan = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[]);
    assert_eq!((plan.opening_from_statement, plan.end_balance), (Some(1_000_00), Some(850_00)));
    // Oldest first on one day still reads as written.
    let text = "Date,Description,Amount,Balance\n2026-10-02,RENT,-100.00,900.00\n2026-10-02,METRO,-50.00,850.00\n";
    let StatementRead::Ok(st) = bank_statements::read(text, 2, false) else { panic!("reads") };
    let plan = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[]);
    assert_eq!((plan.opening_from_statement, plan.end_balance), (Some(1_000_00), Some(850_00)));
}

// An account given nowhere to start: its spending never reads as a debt, its transfers count in
// full, so moving money between the person's own accounts never moves net worth.

#[test]
fn paying_a_card_from_a_chequing_given_nowhere_to_start_moves_nothing() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let last_month = accounts::last_month_end(today).to_string();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let visa = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    spend_from(&mut store, chequing, 1_200_00, &last_month);
    spend_from(&mut store, visa, 800_00, &last_month);
    let before = worth(&store, today);
    assert_eq!((before.total(), before.change()), (-800_00, Some(0)));
    assert_eq!(before.of(chequing), None, "spending alone is not a debt");
    // Paying the card: the card owes $800 less, and the chequing holds $800 less than the
    // nothing it read, rather than both readings dropping the payment.
    transfer(&mut store, chequing, visa, 800_00, &today.to_string());
    let after = worth(&store, today);
    assert_eq!((after.total(), after.change()), (-800_00, Some(0)));
    assert_eq!((after.of(chequing), after.of(visa)), (Some(-800_00), Some(0)));
    // Spending more from it still reads as money Tally was never told about.
    spend_from(&mut store, chequing, 100_00, &today.to_string());
    assert_eq!(worth(&store, today).total(), -800_00);
}

#[test]
fn saving_from_a_chequing_given_nowhere_to_start_keeps_net_worth() {
    let today = tally::model::today();
    let last_month = accounts::last_month_end(today).to_string();
    for date in [last_month.clone(), today.to_string()] {
        let (mut store, _dir) = store();
        let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
        let savings = store.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
        spend_from(&mut store, chequing, 500_00, &last_month);
        transfer(&mut store, chequing, savings, 300_00, &date);
        let w = worth(&store, today);
        assert_eq!((w.total(), w.change()), (0, Some(0)), "moved on {date}");
        assert_eq!((w.of(chequing), w.of(savings)), (Some(-300_00), Some(300_00)));
    }
}

#[test]
fn spending_cash_moved_into_an_account_given_nowhere_to_start_lowers_net_worth() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    transfer(&mut store, chequing, cash, 200_00, &accounts::last_month_end(today).to_string());
    spend_from(&mut store, cash, 50_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(cash)), (950_00, Some(-50_00), Some(150_00)));
    // Spending past what it was given reads as money Tally was never told about.
    spend_from(&mut store, cash, 200_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(cash)), (800_00, Some(-200_00), None));
    // Withdrawn and spent on the same day: what moved in that day pays for that day's spending.
    let (mut store, _dir) = self::store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    spend_from(&mut store, cash, 30_00, &today.to_string());
    transfer(&mut store, chequing, cash, 100_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(cash)), (970_00, Some(-30_00), Some(70_00)));
}

#[test]
fn a_transfer_into_an_account_priced_by_its_holdings_counts_once() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let last_month = accounts::last_month_end(today).to_string();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    // Ten units at $100, then fifteen: the holdings report holds what was moved in.
    let unit = relay_money::invest::QTY_SCALE;
    let price = 100 * relay_money::invest::PRICE_SCALE;
    let holdings = |store: &Store, uid: &str, date: &str, units: i64| {
        let db = store.sql().unwrap();
        db.execute(
            "INSERT INTO holdings (uid, account_id, security_id, date, quantity, book, book_market, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?5, 1)",
            rusqlite::params![uid, tfsa, date, units * unit, units * 100_00],
        )
        .unwrap();
        db.execute("INSERT INTO prices (uid, security_id, date, price, source, updated_at) VALUES (?1, 1, ?2, ?3, 'TEST', 1)", rusqlite::params![format!("p-{uid}"), date, price])
            .unwrap();
    };
    store
        .sql()
        .unwrap()
        .execute("INSERT INTO securities (id, uid, symbol, name, currency, kind, updated_at) VALUES (1, 'sec:XEQT', 'XEQT', 'XEQT', 'CAD', 'ETF', 1)", [])
        .unwrap();
    transfer(&mut store, chequing, tfsa, 1_000_00, &last_month);
    holdings(&store, "h1", &last_month, 10);
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (5_000_00, Some(0), Some(1_000_00)));

    transfer(&mut store, chequing, tfsa, 500_00, &today.to_string());
    holdings(&store, "h2", &today.to_string(), 15);
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (5_000_00, Some(0), Some(1_500_00)));
    // The Investments page and its history read the same.
    let rows = accounts::rows(&store).unwrap();
    let portfolio = store.ledger.portfolio(today).unwrap();
    assert_eq!(tally::invest_logic::view(Some(&portfolio), &rows, None).worth, 1_500_00);
    let input = store.ledger.portfolio_input(today).unwrap();
    let now = accounts::worth_today(&rows, Some(&input), today);
    let then = accounts::worth_back(&store, &rows, Some(&input), accounts::last_month_end(today), &now).unwrap();
    assert_eq!((now[&tfsa], then[&tfsa]), ((1_500_00, true), 1_000_00));
}

// The net-worth rules all at once: every kind of account, read now and at last month's end.

fn days_back(d: relay_money::Date, n: i64) -> relay_money::Date {
    d.checked_sub(jiff::Span::new().days(n)).unwrap()
}

/// A security at $100 a unit, made once per ledger.
fn security(store: &Store) {
    store
        .sql()
        .unwrap()
        .execute("INSERT OR IGNORE INTO securities (id, uid, symbol, name, currency, kind, updated_at) VALUES (1, 'sec:XEQT', 'XEQT', 'XEQT', 'CAD', 'ETF', 1)", [])
        .unwrap();
}

/// A holdings report for `account` on `date`: `units` units at $100.
fn report(store: &Store, account: i64, date: &str, units: i64) {
    security(store);
    let unit = relay_money::invest::QTY_SCALE;
    let price = 100 * relay_money::invest::PRICE_SCALE;
    let db = store.sql().unwrap();
    let uid = format!("h-{account}-{date}");
    db.execute(
        "INSERT INTO holdings (uid, account_id, security_id, date, quantity, book, book_market, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?5, 1)",
        rusqlite::params![uid, account, date, units * unit, units * 100_00],
    )
    .unwrap();
    db.execute("INSERT OR IGNORE INTO prices (uid, security_id, date, price, source, updated_at) VALUES (?1, 1, ?2, ?3, 'TEST', 1)", rusqlite::params![format!("p-{date}"), date, price])
        .unwrap();
}

/// A cash activity (a deposit or a withdrawal) the brokerage recorded.
fn cash_activity(store: &Store, account: i64, kind: &str, date: &str, amount: i64) {
    store
        .sql()
        .unwrap()
        .execute(
            "INSERT INTO activities (uid, account_id, type, date, amount, currency, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'CAD', 1, 1)",
            rusqlite::params![format!("a-{account}-{kind}-{date}"), account, kind, date, amount],
        )
        .unwrap();
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// Given an opening balance.
    Opening,
    /// Given a recorded value.
    Valued,
    /// A card: owes its spending from zero.
    Card,
    /// No opening, no value: spending alone reads $0.
    Untracked,
    /// An investment account priced from its holdings.
    Holdings,
    /// An investment account valued by hand.
    ByHand,
}

const KINDS: [Kind; 6] = [Kind::Opening, Kind::Valued, Kind::Card, Kind::Untracked, Kind::Holdings, Kind::ByHand];

/// One account of each kind, each given its start ten days before last month's end.
fn every_kind(store: &mut Store, early: &str) -> Vec<(Kind, i64)> {
    KINDS
        .iter()
        .map(|&k| {
            let id = match k {
                Kind::Opening => store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id,
                Kind::Valued => {
                    // relay-money records values on investment accounts only; one on another
                    // kind can still come in by sync.
                    let id = store.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
                    store
                        .sql()
                        .unwrap()
                        .execute("INSERT INTO account_values (uid, account_id, date, value, updated_at) VALUES ('v-savings', ?1, ?2, 200000, 1)", rusqlite::params![id, early])
                        .unwrap();
                    id
                }
                Kind::Card => {
                    let id = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
                    spend_from(store, id, 300_00, early);
                    id
                }
                Kind::Untracked => {
                    let id = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
                    spend_from(store, id, 50_00, early);
                    id
                }
                Kind::Holdings => {
                    let id = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
                    report(store, id, early, 10);
                    id
                }
                Kind::ByHand => {
                    let id = store.ledger.account_add("RRSP", AccountType::Investment, 0, Some(Registration::Rrsp), "").unwrap().id;
                    store.ledger.value_set(id, early, 3_000_00).unwrap();
                    id
                }
            };
            (k, id)
        })
        .collect()
}

#[test]
fn r1_a_transfer_between_any_two_kinds_of_account_never_moves_net_worth() {
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let early = days_back(month_end, 10).to_string();
    let before_end = days_back(month_end, 3).to_string();
    for from in KINDS {
        for to in KINDS.into_iter().filter(|k| *k != from) {
            for date in [before_end.clone(), month_end.to_string(), today.to_string()] {
                let (mut store, _dir) = store();
                let ids = every_kind(&mut store, &early);
                let id = |k: Kind| ids.iter().find(|(kk, _)| *kk == k).unwrap().1;
                let w0 = worth(&store, today);
                transfer(&mut store, id(from), id(to), 400_00, &date);
                let w1 = worth(&store, today);
                let case = format!("{from:?} -> {to:?} on {date}");
                assert_eq!(w1.total(), w0.total(), "net worth, {case}");
                assert_eq!(w1.change(), w0.change(), "change since last month end, {case}");
                let of = |w: &accounts::Worth, k: Kind| w.of(id(k)).unwrap_or(0);
                assert_eq!(of(&w1, from) - of(&w0, from), -400_00, "the account it left, {case}");
                assert_eq!(of(&w1, to) - of(&w0, to), 400_00, "the account it went to, {case}");
            }
        }
    }
}

#[test]
fn r2_an_account_given_nowhere_to_start_reads_its_spending_as_nothing() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    spend_from(&mut store, cash, 80_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.of(cash)), (1_000_00, None), "spending alone is not a debt");
    transfer(&mut store, chequing, cash, 200_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.of(cash)), (920_00, Some(120_00)), "what moved in counts, and paid for the day's spending");
}

#[test]
fn r3_a_card_owes_its_spending_and_paying_it_moves_nothing() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let visa = store.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    spend_from(&mut store, visa, 250_00, &today.to_string());
    let w0 = worth(&store, today);
    assert_eq!((w0.total(), w0.of(visa), w0.change()), (750_00, Some(-250_00), Some(-250_00)));
    transfer(&mut store, chequing, visa, 250_00, &today.to_string());
    let w1 = worth(&store, today);
    assert_eq!((w1.total(), w1.change(), w1.of(visa)), (750_00, Some(-250_00), Some(0)));
}

#[test]
fn r4_a_first_holdings_import_is_not_a_gain_and_money_moved_in_is_not_lost() {
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    for moved_on in [days_back(month_end, 5), today] {
        let (mut store, _dir) = store();
        let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
        let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
        transfer(&mut store, chequing, tfsa, 500_00, &moved_on.to_string());
        let w = worth(&store, today);
        assert_eq!((w.total(), w.change(), w.of(tfsa)), (5_000_00, Some(0), Some(500_00)), "moved on {moved_on}");
        // Its first report, today, holds the $500 and $100 it earned.
        report(&store, tfsa, &today.to_string(), 6);
        let w = worth(&store, today);
        assert_eq!((w.total(), w.change(), w.of(tfsa)), (5_100_00, Some(0), Some(600_00)), "moved on {moved_on}");
    }
}

#[test]
fn r5_money_moved_after_the_newest_holdings_report_counts() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    report(&store, tfsa, &month_end.to_string(), 10);
    assert_eq!((worth(&store, today).total(), worth(&store, today).change()), (6_000_00, Some(0)));

    // $500 in today, no report since: still $6,000.
    transfer(&mut store, chequing, tfsa, 500_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa), w.of(chequing)), (6_000_00, Some(0), Some(1_500_00), Some(4_500_00)));
    // $200 back out: the same.
    transfer(&mut store, tfsa, chequing, 200_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(1_300_00)));

    // The Investments page, its history and the account's own page read it the same way.
    let rows = accounts::rows(&store).unwrap();
    let input = store.ledger.portfolio_input(today).unwrap();
    let mut portfolio = store.ledger.portfolio(today).unwrap();
    tally::invest_logic::count_moves(&mut portfolio, &input);
    let v = tally::invest_logic::view(Some(&portfolio), &rows, None);
    assert_eq!((v.worth, v.accounts[0].by_hand), (1_300_00, false));
    let now = accounts::worth_today(&rows, Some(&input), today);
    let then = accounts::worth_back(&store, &rows, Some(&input), month_end, &now).unwrap();
    assert_eq!((now[&tfsa], then[&tfsa]), ((1_300_00, true), 1_000_00));
    let (page, _) = tally::activity_data::subject_listing(&store, today, None, Some(tfsa), "").unwrap().unwrap();
    assert_eq!((page.balance, page.tracks_balance), (1_300_00, true));
}

#[test]
fn r5_a_report_that_holds_an_earlier_transfer_is_not_a_gain() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    // Reported ten days before the month's end, $500 in five days later, a new report today
    // holding it.
    report(&store, tfsa, &days_back(month_end, 10).to_string(), 10);
    transfer(&mut store, chequing, tfsa, 500_00, &days_back(month_end, 5).to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(1_500_00)));
    report(&store, tfsa, &today.to_string(), 15);
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(1_500_00)));

    // A withdrawal the same way: $300 out after the first report, the next one without it.
    let (mut store, _dir) = self::store();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    report(&store, tfsa, &days_back(month_end, 10).to_string(), 10);
    transfer(&mut store, tfsa, chequing, 300_00, &days_back(month_end, 5).to_string());
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(700_00)));
    report(&store, tfsa, &today.to_string(), 7);
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(700_00)));
}

#[test]
fn r5_a_deposit_the_brokerage_recorded_counts_once() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    report(&store, tfsa, &month_end.to_string(), 10);
    transfer(&mut store, chequing, tfsa, 500_00, &today.to_string());
    cash_activity(&store, tfsa, "DEPOSIT", &today.to_string(), 500_00);
    let w = worth(&store, today);
    assert_eq!((w.total(), w.change(), w.of(tfsa)), (6_000_00, Some(0), Some(1_500_00)));
}

#[test]
fn r6_a_goal_on_an_account_given_nowhere_to_start_reads_it_as_the_accounts_page_does() {
    use relay_money::model::GoalKind;
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let savings = store.ledger.account_add("Savings", AccountType::Savings, 2_000_00, None, "").unwrap().id;
    spend_from(&mut store, chequing, 500_00, &days_back(today, 1).to_string());
    let read = |store: &Store| {
        let w = tally::plan_logic::worth_now(store, today);
        let start = w.of.get(&chequing).copied();
        let g = tally::plan_store::goal_save(store.sql().unwrap(), None, &goal("Reach $1,000 in Chequing", GoalKind::Balance, Some(chequing), start.unwrap_or(-1), 1_000_00)).unwrap();
        let g = tally::plan_store::goal(store.sql().unwrap(), g).unwrap().unwrap();
        let inputs = tally::plan_logic::GoalInputs { accounts: store.ledger.accounts().unwrap(), worth: w.total, worth_of: w.of.clone(), ..Default::default() };
        (start, tally::plan_logic::goal_line(&g, &inputs, today).saved)
    };
    assert_eq!(read(&store), (Some(0), 0), "never the -$500 its balance says");
    transfer(&mut store, savings, chequing, 300_00, &today.to_string());
    assert_eq!(read(&store), (Some(300_00), 300_00));
}

#[test]
fn r7_an_accounts_own_page_shows_what_its_row_does() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 1_000_00, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    spend_from(&mut store, cash, 80_00, &today.to_string());
    let page = |store: &Store, id| tally::activity_data::subject_listing(store, today, None, Some(id), "").unwrap().unwrap().0;
    let s = page(&store, cash);
    assert!(!s.tracks_balance, "the row says No balance, so the page says what went out");
    transfer(&mut store, chequing, cash, 200_00, &today.to_string());
    let w = worth(&store, today);
    let s = page(&store, cash);
    assert_eq!((s.balance, s.tracks_balance), (w.of(cash).unwrap(), true));
    assert_eq!(s.balance, 120_00);
    let s = page(&store, chequing);
    assert_eq!((s.balance, s.tracks_balance), (800_00, true));
}

#[test]
fn r8_the_delete_dialog_says_what_another_account_will_read() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let cash = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    // $300 into the cash, $100 of it spent: it reads $200, and $0 once the $300 is gone.
    transfer(&mut store, chequing, cash, 300_00, &today.to_string());
    spend_from(&mut store, cash, 100_00, &today.to_string());
    // $400 into the TFSA before its report (which holds it), $250 after.
    transfer(&mut store, chequing, tfsa, 400_00, &days_back(month_end, 5).to_string());
    report(&store, tfsa, &month_end.to_string(), 4);
    transfer(&mut store, chequing, tfsa, 250_00, &today.to_string());
    let u = accounts::account_use(&store, chequing).unwrap();
    let change = |name: &str| u.others.iter().find(|o| o.name == name).map(|o| o.change);
    assert_eq!(change("Cash"), Some(-200_00));
    assert_eq!(change("TFSA"), Some(-250_00));
    let w = worth(&store, today);
    let (cash_before, tfsa_before) = (w.of(cash).unwrap_or(0), w.of(tfsa).unwrap_or(0));
    accounts::delete(&mut store, chequing).unwrap();
    let w = worth(&store, today);
    assert_eq!(w.of(cash).unwrap_or(0) - cash_before, -200_00);
    assert_eq!(w.of(tfsa).unwrap_or(0) - tfsa_before, -250_00);
}

#[test]
fn a_value_recorded_after_the_holdings_report_is_what_the_account_holds() {
    let (mut store, _dir) = store();
    let today = tally::model::today();
    let month_end = accounts::last_month_end(today);
    let early = days_back(month_end, 20).to_string();
    let late = days_back(month_end, 10).to_string();
    let chequing = store.ledger.account_add("Chequing", AccountType::Chequing, 5_000_00, None, "").unwrap().id;
    let tfsa = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    // A report of ten units ($1,000), then a value typed by hand ten days later: $1,200.
    report(&store, tfsa, &early, 10);
    store.ledger.value_set(tfsa, &late, 1_200_00).unwrap();
    // Then $300 moved in after the value: it counts on top of it.
    transfer(&mut store, chequing, tfsa, 300_00, &today.to_string());
    let w = worth(&store, today);
    assert_eq!((w.of(tfsa), w.total()), (Some(1_500_00), 4_700_00 + 1_500_00));
    let rows = accounts::rows(&store).unwrap();
    let input = store.ledger.portfolio_input(today).unwrap();
    let now = accounts::worth_today(&rows, Some(&input), today);
    let then = accounts::worth_back(&store, &rows, Some(&input), month_end, &now).unwrap();
    assert_eq!((now[&tfsa], then[&tfsa]), ((1_500_00, false), 1_200_00));
    // The Investments page reads it the same, by hand, and lists none of the older report's holdings.
    let mut portfolio = store.ledger.portfolio(today).unwrap();
    tally::invest_logic::set_aside_older_reports(&mut portfolio, &input, &rows);
    tally::invest_logic::count_moves(&mut portfolio, &input);
    let v = tally::invest_logic::view(Some(&portfolio), &rows, None);
    assert_eq!((v.worth, v.accounts[0].by_hand, v.holdings.len()), (1_500_00, true, 0));

    // A newer report wins again: fifteen units, today.
    report(&store, tfsa, &today.to_string(), 15);
    let w = worth(&store, today);
    assert_eq!(w.of(tfsa), Some(1_500_00));
    let rows = accounts::rows(&store).unwrap();
    let input = store.ledger.portfolio_input(today).unwrap();
    let now = accounts::worth_today(&rows, Some(&input), today);
    assert_eq!(now[&tfsa], (1_500_00, true));
    // Last month's end still reads the value, which was newer then.
    let then = accounts::worth_back(&store, &rows, Some(&input), month_end, &now).unwrap();
    assert_eq!(then[&tfsa], 1_200_00);
}

#[test]
fn the_delete_dialog_counts_a_long_list_so_the_warning_stays_on_it() {
    let other = |name: &str| accounts::OtherChange { name: name.into(), transfers: 2, change: -50_00 };
    let goals = vec!["Trip".to_string(), "Car".to_string(), "Roof".to_string()];
    let u = accounts::AccountUse { entries: 4, others: vec![other("Savings"), other("TFSA"), other("Visa")], goals, ..Default::default() };
    let fmt = relay_money::money::MoneyFormatter::new("CAD", relay_money::money::Locale::new("en-CA"));
    assert_eq!(
        accounts::delete_line("Chequing", &u, &fmt, true),
        "Delete Chequing? Its 4 entries go with it. 3 other accounts change too, since the transfers between them and Chequing go. 3 goals that read this account are archived. This cannot be undone. Archiving keeps all of it and takes the account out of the pickers."
    );
}
