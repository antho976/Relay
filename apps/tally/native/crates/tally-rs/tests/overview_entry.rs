//! Overview's goal cards read as the Plan reads them, and a new transfer never starts from an
//! account to itself.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use jiff::civil::date;
use relay_money::model::{AccountType, GoalKind};
use tally::accounts_store as accounts;
use tally::entry::Entry;
use tally::model::Store;
use tally::plan_logic as logic;
use tally::plan_store::{self as plan, GoalInput};

/// A ledger's folder, removed when the test is done with it.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Store, Dir) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-rs-overview-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    (Store::open(&dir.join("tally.db")).expect("a ledger opens"), Dir(dir))
}

fn goal(name: &str, kind: GoalKind, target: i64, account: Option<i64>, start: i64) -> GoalInput {
    GoalInput { name: name.into(), target, target_date: None, color: 1, kind, account_id: account, percent: 0, start_date: Some(date(2026, 11, 1)), start_amount: start }
}

#[test]
fn a_balance_goal_on_overview_fills_from_where_it_started() {
    let (mut s, _d) = store();
    let today = date(2026, 11, 20);
    // A card at -$3,000, paying down from -$5,000 to $0: 40% of the way.
    let card = s.ledger.account_add("Visa", AccountType::Chequing, -300_000, None, "").unwrap().id;
    let conn = s.sql().unwrap();
    plan::goal_save(conn, None, &goal("Debt free", GoalKind::Balance, 0, Some(card), -500_000)).unwrap();
    plan::goal_save(conn, None, &goal("Trip", GoalKind::Savings, 100_000, None, 0)).unwrap();

    let home = s.home(today).unwrap();
    assert_eq!(home.goals.len(), home.s.goals.len(), "the same goals as the summary");
    let debt = home.goals.iter().find(|g| g.name == "Debt free").unwrap();
    assert!((debt.fraction() - 0.4).abs() < 1e-4, "{}", debt.fraction());

    // The same reading as the Plan's Goals page.
    let data = logic::load(&s, today).unwrap();
    let on_plan = data.goals.goals.iter().find(|g| g.id == debt.id).unwrap();
    assert_eq!(on_plan, debt);
}

#[test]
fn a_new_transfer_goes_to_another_account_than_it_leaves() {
    let (mut s, _d) = store();
    s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
    let savings = s.ledger.account_add("Savings", AccountType::Savings, 0, None, "").unwrap().id;
    // New entries start in the second account: the transfer must not also go to it.
    accounts::set_default_account(&mut s, Some(savings)).unwrap();
    let e = Entry::fresh(&s);
    assert_eq!(e.account, Some(savings));
    assert!(e.to_account.is_some() && e.to_account != e.account);
}
