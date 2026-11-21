//! The Plan's own writes (bills, goals, contributions through `plan_store`) read back through
//! relay-money, and the Plan's readings (`plan_logic`).

// Amounts written as dollars and cents: 1_350_00 is $1,350.00.
#![allow(clippy::inconsistent_digit_grouping)]

use jiff::civil::date;
use relay_money::Date;
use relay_money::model::{AccountType, GoalKind, TxType};
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::recurrence::Frequency;
use relay_money::views::TxQuery;
use tally::model::Store;
use tally::plan_logic as logic;
use tally::plan_store::{self as store, BillInput, GoalInput};

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

/// A ledger in a file of its own, with one chequing account, and that account's id.
fn ledger(name: &str) -> (Store, i64, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("tally-plan-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Store::open(&dir.join("tally.db")).expect("a ledger opens");
    let a = s.ledger.account_add("Chequing", AccountType::Chequing, 500_000, None, "").expect("an account");
    (s, a.id, dir)
}

fn category(s: &Store, name: &str) -> i64 {
    s.ledger.categories().unwrap().into_iter().find(|c| c.name == name).unwrap().id
}

fn rent(account: i64, category: Option<i64>, anchor: Date) -> BillInput {
    BillInput {
        name: "  Rent ".into(),
        kind: TxType::Expense,
        amount: 1_350_00,
        account_id: account,
        to_account_id: None,
        category_id: category,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor,
        auto_post: true,
        active: true,
    }
}

fn seq(s: &Store, table: &str, id: i64) -> (i64, i64) {
    s.sql().unwrap().query_row(&format!("SELECT seq, updated_at FROM {table} WHERE id = ?1"), [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

#[test]
fn a_new_bill_is_one_relay_money_lists_and_posts() {
    let (mut s, acct, dir) = ledger("bill-new");
    let today = date(2026, 11, 19);
    let housing = category(&s, "Housing");
    let id = store::bill_save(s.sql().unwrap(), None, &rent(acct, Some(housing), date(2026, 12, 1)), today).unwrap();

    // The anchor ahead of today is its first date; relay-money reads it as a bill due in 12 days.
    let sum = s.ledger.summary(today, &s.locale).unwrap();
    let bill = sum.bills.iter().find(|b| b.id == id).expect("the bill is listed");
    assert_eq!((bill.name.as_str(), bill.amount, bill.next_date.as_str(), bill.days_until), ("Rent", 1_350_00, "2026-12-01", 12));
    assert!(bill.auto_post);

    // The sync triggers numbered the row, and it carries a uid and a stamp.
    let (n, stamp) = seq(&s, "recurring", id);
    assert!(n > 0 && stamp > 0);

    // On its day relay-money posts it, once, with the bill's name, and moves it on a month.
    assert_eq!(s.ledger.post_due(date(2026, 12, 1)).unwrap(), 1);
    assert_eq!(s.ledger.post_due(date(2026, 12, 1)).unwrap(), 0);
    let page = s.ledger.tx_list(&TxQuery { from: Some(date(2026, 12, 1)), to: Some(date(2026, 12, 1)), ..Default::default() }, date(2026, 12, 1)).unwrap();
    assert_eq!(page.transactions.len(), 1);
    assert_eq!((page.transactions[0].note.as_str(), page.transactions[0].category_id), ("Rent", Some(housing)));
    assert_eq!(store::bill(s.sql().unwrap(), id).unwrap().unwrap().next, date(2027, 1, 1));
    assert_eq!(store::last_posted(s.sql().unwrap(), id).unwrap(), Some(date(2026, 12, 1)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_edit_keeps_the_stored_date_and_a_new_rule_starts_again() {
    let (s, acct, dir) = ledger("bill-edit");
    let today = date(2026, 11, 19);
    let conn = s.sql().unwrap();
    let id = store::bill_save(conn, None, &rent(acct, None, date(2026, 11, 1)), today).unwrap();
    // A new bill never back-fills: its anchor behind today starts at the next date.
    assert_eq!(store::bill(conn, id).unwrap().unwrap().next, date(2026, 12, 1));
    let (seq0, stamp0) = seq(&s, "recurring", id);

    // Renamed and re-priced, same rule: the stored next date stands.
    let edit = BillInput { name: "Rent, new lease".into(), amount: 1_400_00, ..rent(acct, None, date(2026, 11, 1)) };
    assert_eq!(store::bill_save(conn, Some(id), &edit, today).unwrap(), id);
    let b = store::bill(conn, id).unwrap().unwrap();
    assert_eq!((b.name.as_str(), b.amount, b.next), ("Rent, new lease", 1_400_00, date(2026, 12, 1)));
    let (seq1, stamp1) = seq(&s, "recurring", id);
    assert!(seq1 > seq0 && stamp1 > stamp0, "an edit is numbered for sync and stamped past the last one");

    // Every two weeks from Nov 2: Nov 30 is the first from today.
    let weekly = BillInput { frequency: Frequency::Weekly, interval: 2, anchor: date(2026, 11, 2), ..edit };
    store::bill_save(conn, Some(id), &weekly, today).unwrap();
    assert_eq!(store::bill(conn, id).unwrap().unwrap().next, date(2026, 11, 30));

    // Paused, it leaves relay-money's coming-up list.
    store::bill_save(conn, Some(id), &BillInput { active: false, ..weekly }, today).unwrap();
    assert!(s.ledger.summary(today, &s.locale).unwrap().bills.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_transfer_bill_needs_two_accounts_and_carries_no_category() {
    let (mut s, acct, dir) = ledger("bill-transfer");
    let savings = s.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
    let today = date(2026, 11, 19);
    let conn = s.sql().unwrap();
    let food = category(&s, "Groceries");
    let mut t = BillInput { name: "To savings".into(), kind: TxType::Transfer, to_account_id: Some(acct), category_id: Some(food), ..rent(acct, None, date(2026, 11, 25)) };
    assert!(store::bill_save(conn, None, &t, today).is_err(), "from and to the same account");
    t.to_account_id = Some(savings);
    let id = store::bill_save(conn, None, &t, today).unwrap();
    let b = store::bill(conn, id).unwrap().unwrap();
    assert_eq!((b.kind, b.to_account_id, b.category_id), (TxType::Transfer, Some(savings), None));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_deleted_bill_leaves_a_tombstone_row_and_comes_back_on_undo() {
    let (s, acct, dir) = ledger("bill-delete");
    let today = date(2026, 11, 19);
    let conn = s.sql().unwrap();
    let id = store::bill_save(conn, None, &rent(acct, None, date(2026, 11, 25)), today).unwrap();
    let (seq0, _) = seq(&s, "recurring", id);
    assert_eq!(store::bill_delete(conn, id).unwrap(), "Rent");
    assert!(s.ledger.summary(today, &s.locale).unwrap().bills.is_empty());
    assert!(store::bills(conn).unwrap().is_empty());
    let (seq1, _) = seq(&s, "recurring", id);
    assert!(seq1 > seq0, "the delete is carried by sync");
    assert!(store::bill_delete(conn, id).is_err());
    store::bill_restore(conn, id).unwrap();
    assert_eq!(s.ledger.summary(today, &s.locale).unwrap().bills.len(), 1);
    // Saving a bill deleted while its editor was open makes a new one.
    store::bill_delete(conn, id).unwrap();
    let again = store::bill_save(conn, Some(id), &rent(acct, None, date(2026, 11, 25)), today).unwrap();
    assert_ne!(again, id);
    let _ = std::fs::remove_dir_all(dir);
}

fn pot(name: &str, target: i64, by: Option<Date>) -> GoalInput {
    GoalInput { name: name.into(), target, target_date: by, color: 3, kind: GoalKind::Savings, account_id: None, percent: 0, start_date: None, start_amount: 0 }
}

#[test]
fn a_pot_and_its_contributions_read_in_relay_moneys_summary() {
    let (s, _, dir) = ledger("goal-pot");
    let today = date(2026, 11, 19);
    let conn = s.sql().unwrap();
    let id = store::goal_save(conn, None, &pot("Lisbon trip", 2_400_00, Some(date(2027, 5, 1)))).unwrap();
    store::contribute(conn, id, 400_00, date(2026, 9, 1), "First").unwrap();
    let out = store::contribute(conn, id, -100_00, date(2026, 10, 1), "").unwrap();
    store::contribute(conn, id, 300_00, today, "").unwrap();

    let sum = s.ledger.summary(today, &s.locale).unwrap();
    let g = sum.goals.iter().find(|g| g.id == id).unwrap();
    assert_eq!((g.saved, g.target), (600_00, 2_400_00));
    // $1,800 left over six months (Nov 19 to May 1, rounded up).
    assert_eq!(g.line, "$300 a month for 6 months");

    // The page's own reading agrees, and lists the rows newest first.
    let rows = store::contributions(conn, id).unwrap();
    assert_eq!(rows.iter().map(|c| c.amount).collect::<Vec<_>>(), vec![300_00, -100_00, 400_00]);
    let goal = store::goal(conn, id).unwrap().unwrap();
    assert_eq!((goal.contributed, goal.first), (600_00, Some(date(2026, 9, 1))));

    // Removing the withdrawal raises what it holds; Undo puts it back.
    assert_eq!(store::contribution_delete(conn, out).unwrap(), -100_00);
    assert_eq!(s.ledger.summary(today, &s.locale).unwrap().goals[0].saved, 700_00);
    store::contribution_restore(conn, out).unwrap();
    assert_eq!(s.ledger.summary(today, &s.locale).unwrap().goals[0].saved, 600_00);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_deleted_goal_takes_its_contributions_and_undo_brings_back_exactly_those() {
    let (s, _, dir) = ledger("goal-delete");
    let today = date(2026, 11, 19);
    let conn = s.sql().unwrap();
    let id = store::goal_save(conn, None, &pot("Cushion", 5_000_00, None)).unwrap();
    store::contribute(conn, id, 1_000_00, today, "").unwrap();
    let gone = store::contribute(conn, id, 50_00, today, "").unwrap();
    store::contribution_delete(conn, gone).unwrap();
    let d = store::goal_delete(conn, id).unwrap();
    assert_eq!(d.contributions.len(), 1);
    assert!(s.ledger.summary(today, &s.locale).unwrap().goals.is_empty());
    store::goal_restore(conn, &d).unwrap();
    let sum = s.ledger.summary(today, &s.locale).unwrap();
    // The contribution removed before the goal stays removed.
    assert_eq!(sum.goals[0].saved, 1_000_00);
    assert_eq!(sum.goals[0].line, "$4,000 to go");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn balance_and_monthly_goals_are_stored_as_relay_money_reads_them() {
    let (mut s, acct, dir) = ledger("goal-kinds");
    let today = date(2026, 11, 19);
    let tfsa = s.ledger.account_add("TFSA", AccountType::Investment, 0, None, "").unwrap().id;
    let worth = GoalInput { kind: GoalKind::Balance, account_id: Some(acct), start_date: Some(today), start_amount: 5_000_00, target_date: Some(date(2027, 3, 1)), ..pot("Ten thousand", 10_000_00, None) };
    let w = store::goal_save(s.sql().unwrap(), None, &worth).unwrap();
    let invest = GoalInput { kind: GoalKind::Invest, percent: 10, target: 0, ..pot("Invest a tenth", 0, None) };
    let i = store::goal_save(s.sql().unwrap(), None, &invest).unwrap();
    s.ledger
        .tx_add(&relay_money::views::TxInput { r#type: TxType::Income, amount: 3_000_00, date: today.to_string(), account_id: acct, to_account_id: None, category_id: None, note: None })
        .unwrap();
    s.ledger
        .tx_add(&relay_money::views::TxInput { r#type: TxType::Transfer, amount: 100_00, date: today.to_string(), account_id: acct, to_account_id: Some(tfsa), category_id: None, note: None })
        .unwrap();
    let sum = s.ledger.summary(today, &s.locale).unwrap();
    let wg = sum.goals.iter().find(|g| g.id == w).unwrap();
    // The account reads $5,000 + $3,000 - $100: $2,100 to go over four months.
    assert_eq!((wg.kind, wg.saved), (GoalKind::Balance, 7_900_00));
    assert_eq!(wg.line, "$525 a month for 4 months");
    let ig = sum.goals.iter().find(|g| g.id == i).unwrap();
    assert_eq!((ig.kind, ig.saved, ig.target), (GoalKind::Invest, 100_00, 300_00));

    // The Plan's own reading of the same goals agrees.
    let data = logic::load(&s, today).unwrap();
    let line = data.goals.goals.iter().find(|g| g.id == i).unwrap();
    assert_eq!(logic::goal_reading(line, &fmt()), "3% of 10%");
    assert_eq!(logic::goal_row_line(line, &fmt(), today), "$200 to go this month for 10% of what came in");

    // An edit keeps the archived flag and the kind's columns.
    store::goal_save(s.sql().unwrap(), Some(w), &GoalInput { name: "Ten k".into(), ..worth }).unwrap();
    let g = store::goal(s.sql().unwrap(), w).unwrap().unwrap();
    assert_eq!((g.name.as_str(), g.start_amount, g.account_id, g.archived), ("Ten k", 5_000_00, Some(acct), false));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_sample_ledger_reads_on_every_plan_page() {
    let dir = std::env::temp_dir().join(format!("tally-plan-{}-sample", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Store::open(&dir.join("tally.db")).unwrap();
    let today = date(2026, 11, 19);
    s.load_sample(today).unwrap();
    let d = logic::load(&s, today).unwrap();
    assert!(d.budgets.overall.is_some());
    assert_eq!(d.budgets.envelopes.len(), 5);
    assert_eq!(d.bills.active.len(), 10);
    assert!(d.bills.monthly_cost > 0 && d.bills.monthly_income > 0);
    assert_eq!(d.goals.goals.len(), 4);
    assert_eq!(d.goals.pots, 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bills_read_as_a_month_of_cost_and_what_falls_due() {
    assert_eq!(logic::per_month(100_00, Frequency::Monthly, 1), 100_00);
    assert_eq!(logic::per_month(1_200_00, Frequency::Yearly, 1), 100_00);
    assert_eq!(logic::per_month(100_00, Frequency::Weekly, 2), 217_43);
    assert_eq!(logic::every_label(Frequency::Weekly, 1), "Every week");
    assert_eq!(logic::every_label(Frequency::Monthly, 3), "Every 3 months");
    let today = date(2026, 10, 14);
    assert_eq!(logic::bill_line(date(2026, 10, 19), 5, "Every month", today), "Due in 5 days · Every month");
    assert_eq!(logic::bill_line(date(2027, 1, 19), 97, "Every year", today), "19 Jan 2027 · Every year");
}

#[test]
fn a_bill_says_what_stops_it_saving() {
    let p = logic::bill_problems(" ", Some(0), TxType::Transfer, Some(1), Some(1));
    assert_eq!(p.name, Some("Name the bill so you can find it later"));
    assert_eq!(p.amount, Some("Enter an amount above zero"));
    assert_eq!(p.account, Some("From and to are the same account. A transfer needs two"));
    assert!(!logic::bill_problems("Rent", Some(1), TxType::Expense, Some(1), None).any());
}

#[test]
fn goals_count_months_and_pace_as_the_kotlin_app_does() {
    assert_eq!(logic::months_until(date(2026, 1, 31), date(2026, 2, 28)), 1);
    assert_eq!(logic::months_until(date(2026, 1, 15), date(2026, 3, 15)), 2);
    assert_eq!(logic::months_until(date(2026, 1, 15), date(2026, 3, 16)), 3);
    assert_eq!(logic::months_until(date(2026, 3, 1), date(2026, 2, 1)), 0);
    assert_eq!(logic::goal_pace_fraction(Some(date(2026, 1, 1)), Some(date(2026, 1, 11)), date(2026, 1, 6)), Some(0.5));
    assert_eq!(logic::goal_pace_fraction(None, Some(date(2026, 1, 11)), date(2026, 1, 6)), None);
    let pace = logic::goal_pace(Some(date(2026, 9, 1)), 600_00, 2_400_00, date(2026, 11, 19)).unwrap();
    assert_eq!((pace.per_month, pace.months), (200_00, 3));
    assert_eq!(pace.reach_by, Some(date(2027, 8, 19)));
    assert_eq!(logic::goal_problems("", GoalKind::Savings, None, false, 0), (Some("Name what you are saving for"), Some("Enter a target above zero")));
    assert_eq!(logic::goal_problems("Invest", GoalKind::Invest, None, true, 10), (None, None));
    assert_eq!(logic::goal_problems("Worth", GoalKind::Balance, None, false, 0).1, Some("Enter the amount to reach"));
}

#[test]
fn budgets_suggest_from_months_on_record_and_name_the_reset_day() {
    assert_eq!(logic::average_on_record(&[0, 300_00, 500_00]), Some(400_00));
    assert_eq!(logic::round_up_to(401_00, 10_00), 410_00);
    assert_eq!(logic::round_to(404_00, 10_00), 400_00);
    assert_eq!(logic::projected_spend(1_000_00, 30, 10), Some(3_000_00));
    assert_eq!(logic::projected_spend(1_000_00, 30, 2), None);
    assert_eq!(logic::reset_line(1), "Resets on the 1st of every month");
    assert_eq!(logic::reset_line(12), "Resets on the 12th of every month");
    assert_eq!(logic::reset_line(22), "Resets on the 22nd of every month");
}

// The pages, driven as a finger would: taps in content units, frames between them.

struct Host(std::path::PathBuf);

impl tally::app::Host for Host {
    fn data_dir(&self) -> std::path::PathBuf {
        self.0.clone()
    }
    fn log(&self, _: &str) {}
}

struct Driver {
    app: tally::app::App,
    clock: u64,
    dir: std::path::PathBuf,
}

impl Driver {
    fn new(name: &str) -> Driver {
        let dir = std::env::temp_dir().join(format!("tally-plan-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = tally::app::App::new(std::sync::Arc::new(Host(dir.clone())), 1_000_000_000);
        app.resize(1080.0, 2340.0, 90.0, 40.0);
        app.store.load_sample(tally::model::today()).unwrap();
        app.invalidate();
        app.push(tally::nav::Route::Overview);
        let mut d = Driver { app, clock: 1_000_000_000, dir };
        d.frame();
        d
    }
    fn frame(&mut self) {
        self.clock += 16_666_666;
        self.app.frame(self.clock);
    }
    fn go(&mut self, r: tally::nav::Route) {
        self.app.push(r);
        self.frame();
    }
    fn tap(&mut self, x: f32, y: f32) {
        self.app.ui.tap = Some((x, y));
        self.frame();
    }
    fn top(&self) -> f32 {
        self.app.canvas.inset_top + 10.0
    }
    /// The header's Save pill, at the page's top.
    fn save_key(&mut self) {
        let top = self.top();
        self.tap(390.0 - 16.0 - 28.0, top + 22.0);
    }
    /// Scrolls the page to its end and taps the key across the page that ends it (Delete).
    fn last_key(&mut self) {
        let max = self.app.scroll().max;
        self.app.scroll().off = max;
        self.frame();
        let c = &self.app.canvas;
        let y = c.h - c.inset_bottom - 24.0 - 26.0;
        self.tap(195.0, y);
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_bill_typed_into_its_editor_is_saved_and_its_delete_is_undone() {
    use tally::nav::Route;
    let mut d = Driver::new("drive-bill");
    d.go(Route::Bills);
    d.go(Route::BillEdit { id: None });
    // Saving with nothing typed saves nothing.
    d.save_key();
    assert_eq!(d.app.route(), &Route::BillEdit { id: None });
    let top = d.top();
    d.tap(100.0, top + 172.0);
    d.app.set_text("Netflix");
    d.frame();
    d.tap(100.0, top + 266.0);
    d.app.set_text("15.99");
    d.frame();
    d.app.finish_typing();
    d.frame();
    d.save_key();
    assert_eq!(d.app.route(), &Route::Bills);
    let bills = store::bills(d.app.store.sql().unwrap()).unwrap();
    let b = bills.iter().find(|b| b.bill.name == "Netflix").expect("the bill was saved");
    assert_eq!((b.bill.amount, b.bill.kind, b.bill.frequency), (15_99, TxType::Expense, Frequency::Monthly));

    // Its Delete key, at the end of the editor, deletes it at once; the Plan's toast brings it back.
    let id = b.bill.id;
    d.go(Route::BillEdit { id: Some(id) });
    d.last_key();
    assert_eq!(d.app.route(), &Route::Bills);
    assert!(store::bill(d.app.store.sql().unwrap(), id).unwrap().is_none());
    let bottom = d.app.canvas.h - d.app.canvas.inset_bottom - 8.0;
    d.tap(390.0 - 16.0 - 40.0, bottom - 12.0 - 24.0);
    assert!(store::bill(d.app.store.sql().unwrap(), id).unwrap().is_some(), "Undo restored the bill");
}

#[test]
fn a_budget_typed_on_the_keypad_is_set() {
    use tally::nav::Route;
    let mut d = Driver::new("drive-budget");
    let health = d.app.store.ledger.categories().unwrap().into_iter().find(|c| c.name == "Health").unwrap().id;
    d.go(Route::Budgets);
    d.go(Route::BudgetEdit { category: Some(health) });
    // The keypad: 1 at its first key, then 2, 0 and 0 along the rows.
    let c = &d.app.canvas;
    let top = c.h - c.inset_bottom - tally::plan_kit::KEYPAD_H;
    let (kw, kh, gap) = ((358.0 - 12.0) / 3.0, 46.0, 6.0);
    let key = |col: f32, row: f32| (16.0 + col * (kw + gap) + kw / 2.0, top + 14.0 + row * (kh + gap) + kh / 2.0);
    for (x, y) in [key(0.0, 0.0), key(1.0, 0.0), key(1.0, 3.0), key(1.0, 3.0)] {
        d.tap(x, y);
    }
    d.save_key();
    assert_eq!(d.app.route(), &Route::Budgets);
    assert_eq!(d.app.store.ledger.budget(Some(health)).unwrap(), Some(120_000));
}

// The premium pages' words.

fn bill_line(id: i64, name: &str, kind: TxType, amount: i64, next: Date, today: Date) -> logic::BillLine {
    let bill = store::Bill {
        id,
        name: name.into(),
        kind,
        amount,
        account_id: 1,
        to_account_id: (kind == TxType::Transfer).then_some(2),
        category_id: None,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor: next,
        next,
        end: None,
        auto_post: true,
        active: true,
    };
    let to_account = (kind == TxType::Transfer).then(|| "Savings".to_string());
    logic::BillLine { row: store::BillRow { bill, account: "Visa".into(), to_account, icon: None, color: None }, days_until: (next - today).get_days() as i64, per_month: amount }
}

#[test]
fn budgets_read_as_what_is_left_and_the_pace_in_words() {
    use relay_money::pace::PaceReading;
    let on = PaceReading::new(3_200_00, 1_600_00, 30, 15);
    assert_eq!(logic::left_words(&on, &fmt()), ("$1,600 left".to_string(), false));
    assert_eq!(logic::budgets_hero_line(&on, &fmt(), 15), "$1,600 of $3,200 spent. On pace with 15 days to go.");
    let over = PaceReading::new(120_00, 140_00, 30, 20);
    assert_eq!(logic::left_words(&over, &fmt()), ("$20 over".to_string(), true));
    assert_eq!(logic::budgets_hero_line(&over, &fmt(), 10), "$140 of $120 spent, with 10 days to go.");
}

#[test]
fn bills_group_by_when_they_fall_due_and_say_what_they_are() {
    let today = date(2026, 11, 19);
    let active = vec![
        bill_line(1, "Internet", TxType::Expense, 60_00, date(2026, 11, 22), today),
        bill_line(2, "Card", TxType::Transfer, 900_00, date(2026, 11, 25), today),
        bill_line(3, "Netflix", TxType::Expense, 19_00, date(2026, 11, 28), today),
        bill_line(4, "Rent", TxType::Expense, 1_350_00, date(2026, 12, 1), today),
        bill_line(5, "Insurance", TxType::Expense, 112_00, date(2027, 1, 3), today),
    ];
    let groups = logic::bill_groups(&active, today);
    let titles: Vec<&str> = groups.iter().map(|g| g.0.as_str()).collect();
    assert_eq!(titles, ["This week", "Later in November", "December", "Later"]);
    assert_eq!(groups[0].1, [0, 1]);
    assert_eq!(logic::bill_note(&active[0]), "In 3 days · Visa");
    assert_eq!(logic::bill_note(&active[1]), "Transfer to Savings");
    assert_eq!(logic::bill_note(&active[3]), "Autopay · Visa");
    let (title, line) = logic::bills_hero(&logic::bills_reading(Vec::new(), today), today, &fmt());
    assert_eq!((title.as_str(), line.as_str()), ("Nothing due", "No bills fall due in the next 30 days."));
    let r = logic::BillsReading { month: logic::DueSum { total: 1_429_00, payments: 3 }, active: active.clone(), ..Default::default() };
    let (title, line) = logic::bills_hero(&r, today, &fmt());
    assert_eq!(title, "$1,429.00");
    assert_eq!(line, "3 bills in the next 30 days. Rent is the big one, on December 1.");
    // Posted within the week: paid lately, oldest first; a week back is not.
    let posted = [(3, date(2026, 11, 18)), (1, date(2026, 11, 14)), (4, date(2026, 11, 12))];
    assert_eq!(logic::paid_lately(&active, &posted, today), [(0, date(2026, 11, 14)), (2, date(2026, 11, 18))]);
}

fn goal(name: &str, kind: GoalKind, saved: i64, target: i64, months_left: Option<i64>) -> logic::GoalLine {
    logic::GoalLine {
        id: 1,
        name: name.into(),
        color: 0,
        saved,
        target,
        target_date: months_left.map(|m| date(2026, 11, 19).checked_add(jiff::Span::new().months(m)).unwrap()),
        months_left,
        pace_fraction: None,
        kind,
        start: 0,
        percent: 0,
        income: 0,
        account_name: None,
        history: Vec::new(),
    }
}

#[test]
fn goals_feature_the_closest_and_say_what_a_month_needs() {
    let today = date(2026, 11, 19);
    let goals = vec![
        goal("Trip", GoalKind::Savings, 2_140_00, 4_000_00, Some(6)),
        goal("Cushion", GoalKind::Savings, 5_200_00, 6_000_00, Some(2)),
        goal("Done", GoalKind::Savings, 500_00, 500_00, None),
        goal("Invest", GoalKind::Invest, 900_00, 1_000_00, None),
    ];
    assert_eq!(logic::closest_goal(&goals), Some(1));
    assert_eq!(logic::closest_goal(&goals[2..3]), Some(0));
    assert_eq!(logic::closest_goal(&[]), None);
    assert_eq!(logic::goal_card_line(&goals[1], &fmt()), "$5,200 of $6,000 · 2 months to go");
    assert_eq!(logic::goal_need_line(&goals[1], &fmt(), today), "$400 a month reaches it by January 2027.");
    assert_eq!(logic::goal_need_line(&goals[2], &fmt(), today), "Reached, with $500 put in.");
    let pace = logic::goal_pace(Some(date(2026, 6, 1)), 2_140_00, 4_000_00, today);
    assert_eq!(
        logic::goal_pace_words(&goals[0], pace, Some(date(2026, 6, 1)), &fmt(), today).as_deref(),
        Some("You have put in $357 a month since June. At that pace it lands in May 2027.")
    );
    assert_eq!(logic::goal_pace_words(&goals[0], None, None, &fmt(), today).as_deref(), Some("Nothing put in yet."));
}

/// The owner's rule: a page the drawer opens shows the drawer's key; the same page opened from
/// another page (Overview's "All", the bell) shows Back, and so does a tab's page opened over one.
#[test]
fn drawer_key_only_where_the_drawer_opened_the_page() {
    use tally::nav::Route;
    let mut d = Driver::new("drawer-key");
    let app = &mut d.app;
    app.push(Route::Budgets);
    assert!(!app.drawer_page() && app.is_sub_page(), "Budgets from Overview has Back");
    app.pop();
    app.open_from_drawer(Route::Budgets);
    assert!(app.drawer_page(), "Budgets from the drawer has the drawer's key");
    app.push(Route::BudgetEdit { category: None });
    assert!(!app.drawer_page(), "an editor over it has Back");
    app.pop();
    assert!(app.drawer_page(), "back on Budgets, the key again");
    app.pop();
    assert_eq!(app.route(), &Route::Overview);
    app.push(Route::Budgets);
    assert!(!app.drawer_page(), "the drawer's mark does not outlive its page");
    app.pop();
    app.push_over(Route::Entries);
    assert_eq!(app.nav.len(), 2, "See all opens Entries over Overview");
    assert!(app.is_sub_page());
    app.pop();
    assert_eq!(app.route(), &Route::Overview);
}
