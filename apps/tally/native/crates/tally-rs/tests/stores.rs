//! The review's findings on the stores and the readings: each one held here so it stays fixed.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use jiff::civil::date;
use relay_money::ledger::MAX_AMOUNT;
use relay_money::model::{AccountType, CategoryKind, GoalKind, TxType};
use relay_money::period::BudgetPeriod;
use relay_money::recurrence::Frequency;
use relay_money::views::{Tx, TxInput, TxQuery};
use tally::accounts_store::{self as accounts, Draft};
use tally::categories_store as categories;
use tally::import_store;
use tally::insights_data as ins;
use tally::model::Store;
use tally::plan_logic::{self as logic, GoalLine};
use tally::plan_store::{self as plan, BillInput, GoalInput};

/// A ledger's folder, removed when the test is done with it.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Store, Dir) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-rs-stores-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    (Store::open(&dir.join("tally.db")).expect("a ledger opens"), Dir(dir))
}

fn entries(store: &Store) -> Vec<Tx> {
    store.ledger.tx_list(&TxQuery { from: Some(date(2000, 1, 1)), to: Some(date(2100, 1, 1)), limit: Some(10_000), ..Default::default() }, date(2026, 11, 20)).unwrap().transactions
}

fn category(store: &Store, name: &str) -> Option<relay_money::views::CategoryView> {
    store.ledger.categories().unwrap().into_iter().find(|c| c.name == name)
}

fn stamp_and_seq(store: &Store, table: &str, id: i64) -> (i64, i64) {
    store.sql().unwrap().query_row(&format!("SELECT updated_at, seq FROM {table} WHERE id = ?1"), [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

// 1. A pace far beyond the calendar

#[test]
fn a_goal_paced_past_the_calendar_has_no_date_and_does_not_crash() {
    let today = date(2026, 11, 20);
    // $50,000 to go at $5 put in two years ago: ten thousand years of months.
    let pace = logic::goal_pace(Some(date(2024, 11, 20)), 500, 5_000_000, today).unwrap();
    assert_eq!((pace.per_month, pace.reach_by), (20, None));
    let pace = logic::goal_pace(Some(date(2024, 11, 20)), 2400, i64::MAX / 2, today).unwrap();
    assert_eq!((pace.per_month, pace.reach_by), (100, None));
    let pace = logic::goal_pace(Some(date(2026, 10, 20)), 100_000, 10_000_000_000, today).unwrap();
    assert_eq!(pace.reach_by, None, "99,999 months out is past the year 9999");
    // Within the calendar it still lands.
    let pace = logic::goal_pace(Some(date(2026, 9, 20)), 20_000, 100_000, today).unwrap();
    assert_eq!(pace.reach_by, Some(date(2027, 7, 20)));

    let line = GoalLine {
        id: 1,
        name: "Worth".into(),
        color: 0,
        saved: 100_000 + 2400,
        target: i64::MAX / 2,
        target_date: None,
        months_left: None,
        pace_fraction: None,
        kind: GoalKind::Balance,
        start: 100_000,
        percent: 0,
        income: 0,
        account_name: None,
        history: Vec::new(),
    };
    let pace = logic::balance_pace(&line, Some(date(2024, 11, 20)), today).unwrap();
    assert_eq!((pace.per_month, pace.reach_by), (100, None));
    let near = GoalLine { target: 100_000 + 4800, ..line };
    assert_eq!(logic::balance_pace(&near, Some(date(2024, 11, 20)), today).unwrap().reach_by, Some(date(2028, 11, 20)));
}

// 2. The default account by uid

#[test]
fn the_default_account_follows_its_uid_through_a_restore_that_renumbers() {
    let (mut a, _da) = store();
    a.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    let visa = a.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    accounts::set_default_account(&mut a, Some(visa)).unwrap();
    assert_eq!(accounts::default_account(&a), Some(visa));
    let visa_uid: String = a.sql().unwrap().query_row("SELECT uid FROM accounts WHERE id = ?1", [visa], |r| r.get(0)).unwrap();

    // Another ledger holding the same Visa third, after two accounts of its own.
    let (mut b, _db) = store();
    b.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap();
    b.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap();
    let b_visa = b.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    b.sql().unwrap().execute("UPDATE accounts SET uid = ?1 WHERE id = ?2", rusqlite::params![visa_uid, b_visa]).unwrap();
    let file = b.ledger.export_backup("2026-11-20T00:00:00Z").unwrap();
    a.ledger.import_backup(&file).unwrap();
    // The id the default had (2) is now Cash: the default is still the Visa, at its new id.
    let now = accounts::default_account(&a).unwrap();
    assert_ne!(now, visa);
    assert_eq!(a.ledger.accounts().unwrap().into_iter().find(|x| x.id == now).unwrap().name, "Visa");
    assert_eq!(accounts::default_name(&a).unwrap().as_deref(), Some("Visa"));

    // A restore without the Visa leaves no default, never another account.
    let (mut c, _dc) = store();
    c.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap();
    c.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap();
    c.ledger.account_add("Chequing 2", AccountType::Chequing, 0, None, "").unwrap();
    a.ledger.import_backup(&c.ledger.export_backup("2026-11-20T00:00:00Z").unwrap()).unwrap();
    assert_eq!(accounts::default_account(&a), None);
}

#[test]
fn an_old_default_kept_by_id_is_dropped() {
    let (mut s, _d) = store();
    let id = s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    s.set_pref("default_account", &id.to_string()).unwrap();
    assert_eq!(accounts::default_account(&s), None, "an id from 0.3 cannot be trusted");
    accounts::set_default_account(&mut s, Some(id)).unwrap();
    assert_eq!(accounts::default_account(&s), Some(id));
    assert_eq!(s.pref("default_account"), None);
}

#[test]
fn an_archived_default_stays_undone_when_it_comes_back() {
    let (mut s, _d) = store();
    let id = s.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    accounts::set_default_account(&mut s, Some(id)).unwrap();
    accounts::set_archived(&mut s, id, true).unwrap();
    assert_eq!(accounts::default_account(&s), None);
    accounts::set_archived(&mut s, id, false).unwrap();
    assert_eq!(accounts::default_account(&s), None, "archiving cleared the default; bringing it back does not set it");
}

// 3. A deleted account's goals

#[test]
fn deleting_an_account_lets_its_goals_follow_everything() {
    let (mut s, _d) = store();
    let visa = s.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    let goal = GoalInput {
        name: "Pay it off".into(),
        target: 0,
        target_date: None,
        color: 1,
        kind: GoalKind::Balance,
        account_id: Some(visa),
        percent: 0,
        start_date: Some(date(2026, 11, 1)),
        start_amount: -50_000,
    };
    let g = plan::goal_save(s.sql().unwrap(), None, &goal).unwrap();
    let (stamp0, seq0) = stamp_and_seq(&s, "goals", g);
    accounts::delete(&mut s, visa).unwrap();
    assert_eq!(plan::goal(s.sql().unwrap(), g).unwrap().unwrap().account_id, None);
    let (stamp1, seq1) = stamp_and_seq(&s, "goals", g);
    assert!(stamp1 > stamp0 && seq1 > seq0, "the change is stamped and numbered for sync");
}

// 4. Tally's CSV read back

const HEADER: &str = "date,type,amount,currency,category,account,to_account,note\n";

#[test]
fn a_bad_line_is_skipped_and_counted_and_its_category_never_made() {
    let (mut s, _d) = store();
    s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    let text = format!("{HEADER}2026-10-01,expense,12.50,CAD,Coffee,Chequing,,Cafe\n2026-10-02,expense,100000000000.00,CAD,Yachts,Chequing,,Boat\n");
    let words = import_store::import_tally_csv(&mut s, &text).unwrap();
    assert_eq!(words, "Imported 1 entry, skipped 1");
    assert_eq!(entries(&s).len(), 1);
    assert!(category(&s, "Coffee").is_some());
    assert!(category(&s, "Yachts").is_none(), "a line not written makes no category");
}

#[test]
fn an_import_that_fails_takes_its_new_categories_back() {
    let (mut s, _d) = store();
    s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    // A refusal the ledger cannot shrug off as one bad line.
    s.sql().unwrap().execute_batch("CREATE TRIGGER boom BEFORE INSERT ON transactions WHEN NEW.note = 'boom' BEGIN SELECT RAISE(ABORT, 'boom'); END;").unwrap();
    let text = format!("{HEADER}2026-10-01,expense,12.50,CAD,Coffee,Chequing,,Cafe\n2026-10-02,expense,3.00,CAD,Snacks,Chequing,,boom\n");
    let why = import_store::import_tally_csv(&mut s, &text).unwrap_err();
    assert!(why.starts_with("The import did not finish. Nothing was added."), "{why}");
    assert!(entries(&s).is_empty());
    assert!(category(&s, "Coffee").is_none() && category(&s, "Snacks").is_none(), "nothing stays behind");
}

#[test]
fn lines_with_no_account_go_to_the_default() {
    let (mut s, _d) = store();
    s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    let visa = s.ledger.account_add("Visa", AccountType::Credit, 0, None, "").unwrap().id;
    accounts::set_default_account(&mut s, Some(visa)).unwrap();
    let text = format!("{HEADER}2026-10-01,expense,12.50,CAD,,,,Cafe\n");
    import_store::import_tally_csv(&mut s, &text).unwrap();
    assert_eq!(entries(&s)[0].account_id, visa);
}

#[test]
fn tallys_own_export_read_back_adds_nothing_twice() {
    let (mut s, _d) = store();
    s.load_sample(tally::model::today()).unwrap();
    let before = entries(&s).len();
    let text = relay_money::csv::encode(&tally::settings_store::csv_rows(&s), "CAD", 2);
    let words = import_store::import_tally_csv(&mut s, &text).unwrap();
    assert_eq!(entries(&s).len(), before);
    assert!(words.starts_with("Nothing new to add") && words.ends_with(&format!("{before} already there")), "{words}");
    // A file holding one more coffee than the ledger adds just that one.
    let (mut t, _dt) = store();
    t.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    let one = format!("{HEADER}2026-10-01,expense,4.25,CAD,Coffee,Chequing,,Cafe\n");
    import_store::import_tally_csv(&mut t, &one).unwrap();
    let two = format!("{one}2026-10-01,expense,4.25,CAD,Coffee,Chequing,,Cafe\n");
    assert_eq!(import_store::import_tally_csv(&mut t, &two).unwrap(), "Imported 1 entry · 1 already there");
    assert_eq!(entries(&t).len(), 2);
}

// 6. Amounts past what the ledger keeps

fn rent(account: i64, amount: i64) -> BillInput {
    BillInput {
        name: "Rent".into(),
        kind: TxType::Expense,
        amount,
        account_id: account,
        to_account_id: None,
        category_id: None,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor: date(2026, 11, 1),
        auto_post: true,
        active: true,
    }
}

fn pot(target: i64) -> GoalInput {
    GoalInput { name: "Trip".into(), target, target_date: None, color: 3, kind: GoalKind::Savings, account_id: None, percent: 0, start_date: None, start_amount: 0 }
}

#[test]
fn amounts_past_the_ledgers_limit_are_refused() {
    let (mut s, _d) = store();
    let acct = s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let today = date(2026, 11, 20);
    let conn = s.sql().unwrap();
    assert_eq!(plan::bill_save(conn, None, &rent(acct, MAX_AMOUNT + 1), today).unwrap_err(), plan::TOO_LARGE);
    assert!(plan::bill_save(conn, None, &rent(acct, MAX_AMOUNT), today).is_ok());
    assert_eq!(plan::goal_save(conn, None, &pot(MAX_AMOUNT + 1)).unwrap_err(), plan::TOO_LARGE);
    assert_eq!(plan::goal_save(conn, None, &GoalInput { start_amount: -MAX_AMOUNT - 1, ..pot(100) }).unwrap_err(), plan::TOO_LARGE);
    let g = plan::goal_save(conn, None, &pot(10_000)).unwrap();
    assert_eq!(plan::contribute(conn, g, MAX_AMOUNT + 1, today, "").unwrap_err(), plan::TOO_LARGE);
    assert_eq!(plan::contribute(conn, g, -MAX_AMOUNT - 1, today, "").unwrap_err(), plan::TOO_LARGE);
    // 100 billion dollars, in cents, is one past the limit.
    let d = Draft { name: "Vault".into(), opening_text: "100000000000".into(), ..Draft::default() };
    assert_eq!(accounts::problems(&d, 2).1, Some(plan::TOO_LARGE));
    assert_eq!(accounts::save(&mut s, None, &d, 2).unwrap_err(), plan::TOO_LARGE);
    assert_eq!(accounts::save(&mut s, Some(acct), &Draft { owe: true, ..d.clone() }, 2).unwrap_err(), plan::TOO_LARGE);
}

// 7. A save that changes nothing

#[test]
fn saving_a_bill_or_goal_unchanged_keeps_its_stamp() {
    let (mut s, _d) = store();
    let acct = s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let today = date(2026, 11, 20);
    let conn = s.sql().unwrap();
    let bill = plan::bill_save(conn, None, &rent(acct, 135_000), today).unwrap();
    let before = stamp_and_seq(&s, "recurring", bill);
    plan::bill_save(conn, Some(bill), &rent(acct, 135_000), today).unwrap();
    assert_eq!(stamp_and_seq(&s, "recurring", bill), before, "nothing changed, so a newer edit elsewhere still wins");
    plan::bill_save(conn, Some(bill), &rent(acct, 140_000), today).unwrap();
    let after = stamp_and_seq(&s, "recurring", bill);
    assert!(after.0 > before.0 && after.1 > before.1);

    let goal = plan::goal_save(conn, None, &pot(50_000)).unwrap();
    let before = stamp_and_seq(&s, "goals", goal);
    plan::goal_save(conn, Some(goal), &pot(50_000)).unwrap();
    assert_eq!(stamp_and_seq(&s, "goals", goal), before);
    plan::goal_save(conn, Some(goal), &GoalInput { target_date: Some(date(2027, 6, 1)), ..pot(50_000) }).unwrap();
    assert!(stamp_and_seq(&s, "goals", goal).0 > before.0);
}

// 8. A category's entries move only to one of its kind

#[test]
fn a_deleted_categorys_entries_move_only_to_one_of_its_kind() {
    let (mut s, _d) = store();
    let acct = s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let dining = categories::add(&s, "Dining out", CategoryKind::Expense, "dining", 2).unwrap();
    let salary = categories::add(&s, "Paycheque", CategoryKind::Income, "work", 3).unwrap();
    let groceries = categories::add(&s, "Food at home", CategoryKind::Expense, "cart", 4).unwrap();
    s.add(&TxInput { r#type: TxType::Expense, amount: 4000, date: "2026-11-02".into(), account_id: acct, to_account_id: None, category_id: Some(dining), note: None }).unwrap();
    assert_eq!(categories::delete(&s, dining, Some(salary)).unwrap_err(), "Move them to another spending category");
    assert_eq!(entries(&s)[0].category_id, Some(dining), "nothing moved");
    categories::delete(&s, dining, Some(groceries)).unwrap();
    assert_eq!(entries(&s)[0].category_id, Some(groceries));
}

// 9 to 11. Insights read as Kotlin reads them

#[test]
fn the_kept_share_rounds_halves_up_as_kotlin_does() {
    assert_eq!(ins::kept_share(200, 225), Some(-12));
    assert_eq!(ins::kept_share(200, 175), Some(13));
    assert_eq!(ins::kept_share(1000, 250), Some(75));
}

#[test]
fn a_tie_for_top_weekday_goes_to_the_first_the_period_meets() {
    // October 2026 opens on a Thursday; Thursday and Sunday each average $1.
    let p = BudgetPeriod::containing(date(2026, 10, 1), 1);
    let cal = ins::summarize_calendar(&[100, 0, 0, 100], &p, 4, true);
    assert_eq!(cal.top_weekday, Some(jiff::civil::Weekday::Thursday));
    let cal = ins::summarize_calendar(&[100, 0, 0, 300], &p, 4, true);
    assert_eq!(cal.top_weekday, Some(jiff::civil::Weekday::Sunday));
}

fn spent(id: i64, amount: i64, note: &str) -> Tx {
    Tx {
        id,
        uid: format!("t{id}"),
        r#type: TxType::Expense,
        amount,
        date: "2026-10-03".into(),
        account_id: 1,
        account: "Chequing".into(),
        to_account_id: None,
        to_account: None,
        category_id: None,
        category: None,
        icon: None,
        color: None,
        note: note.into(),
    }
}

#[test]
fn payees_are_grouped_on_the_note_as_written() {
    let p = ins::payees(&[spent(1, 500, "Metro"), spent(2, 700, "metro"), spent(3, 900, "IGA"), spent(4, 100, "Metro")], 5);
    let read: Vec<(&str, i64, usize)> = p.iter().map(|p| (p.note.as_str(), p.total, p.count)).collect();
    assert_eq!(read, vec![("IGA", 900, 1), ("metro", 700, 1), ("Metro", 600, 2)]);
}
