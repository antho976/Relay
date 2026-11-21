//! Accounts, banks and the Entries and Insights tabs: what the delete dialog counts for an
//! investment account, a recorded value taken back and put back, what a bank needing a new
//! sign-in says, and the tabs opening on this month.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use relay_money::model::{AccountType, Registration};
use relay_money::money::{Locale, MoneyFormatter};
use tally::accounts_store as accounts;
use tally::app::App;
use tally::banks_logic;
use tally::model::Store;
use tally::nav::Route;

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

/// A folder of its own, removed when the test is done with it.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(name: &str) -> Dir {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-rs-accounts-banks-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    Dir(dir)
}

fn store() -> (Store, Dir) {
    let d = dir("store");
    (Store::open(&d.0.join("tally.db")).expect("a ledger opens"), d)
}

#[test]
fn an_investment_account_with_only_values_is_not_empty_to_the_delete_dialog() {
    let (mut store, _dir) = store();
    let id = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    store.ledger.value_set(id, "2026-10-11", 841_200).unwrap();
    store.ledger.value_set(id, "2026-11-01", 850_000).unwrap();
    let u = accounts::account_use(&store, id).unwrap();
    assert_eq!((u.entries, u.bills, u.values), (0, 0, 2));
    // Archive is offered, and the dialog names what goes.
    assert!(!u.is_empty());
    assert_eq!(
        accounts::delete_line("TFSA", &u, &fmt(), true),
        "Delete TFSA? Its 2 recorded values go with it. This cannot be undone. Archiving keeps all of it and takes the account out of the pickers."
    );
    assert_eq!(accounts::deleted_line("TFSA", &u), "TFSA deleted with its 2 recorded values");
    // An account with nothing in it still says so.
    let empty = store.ledger.account_add("Spare", AccountType::Chequing, 0, None, "").unwrap().id;
    let e = accounts::account_use(&store, empty).unwrap();
    assert!(e.is_empty());
    assert_eq!(accounts::delete_line("Spare", &e, &fmt(), false), "Delete Spare? It has no entries and no bills. This cannot be undone.");
}

#[test]
fn what_goes_with_an_account_reads_as_a_list() {
    let u = accounts::AccountUse { entries: 3, bills: 1, holdings: 2, values: 1, activities: 1, others: Vec::new(), goals: Vec::new() };
    assert_eq!(accounts::goes_with(&u).as_deref(), Some("3 entries, 1 bill, 2 holdings, 1 recorded value and 1 activity"));
    let one = accounts::AccountUse { holdings: 1, ..Default::default() };
    assert_eq!(accounts::delete_line("RRSP", &one, &fmt(), true), "Delete RRSP? Its 1 holding goes with it. This cannot be undone. Archiving keeps all of it and takes the account out of the pickers.");
}

#[test]
fn a_removed_value_comes_back() {
    let (mut store, _dir) = store();
    let id = store.ledger.account_add("TFSA", AccountType::Investment, 0, Some(Registration::Tfsa), "").unwrap().id;
    store.ledger.value_set(id, "2026-10-11", 841_200).unwrap();
    let vid = accounts::values(&store, id).unwrap()[0].id;
    accounts::delete_value(&store, vid).unwrap();
    assert!(accounts::values(&store, id).unwrap().is_empty());
    accounts::restore_value(&store, vid).unwrap();
    let back = accounts::values(&store, id).unwrap();
    assert_eq!((back.len(), back[0].value), (1, 841_200));
}

#[test]
fn a_bank_needing_a_new_sign_in_is_signed_in_to_in_place() {
    // Fix names the bank, so the server opens Plaid's update mode rather than a second connection.
    assert_eq!(banks_logic::hosted_link_payload(Some("item-1"))["bank_id"], "item-1");
    assert!(banks_logic::hosted_link_payload(None).get("bank_id").is_none(), "Connect makes a new one");
    // A server from before that refuses the field; then the old way out is said instead.
    let old = tally::sync::Failure { code: "bus.schema".into(), message: "payload for money.bank.hosted_link does not match its schema: unknown field `bank_id`".into() };
    assert!(banks_logic::fix_not_on_server(&old));
    assert!(!banks_logic::fix_not_on_server(&tally::sync::Failure { code: "money.plaid".into(), message: "Plaid said: no".into() }));
    let line = banks_logic::sign_in_again_line("Desjardins");
    assert!(line.contains("Desjardins") && line.contains("Disconnect"), "{line}");
}

struct Host(PathBuf);

impl tally::app::Host for Host {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _: &str) {}
}

#[test]
fn entries_and_insights_open_on_this_month_from_the_tab() {
    let d = dir("tabs");
    let mut app = App::new(Arc::new(Host(d.0.clone())), 1_000_000_000);
    app.resize(1080.0, 2340.0, 90.0, 40.0);
    app.store.load_sample(tally::model::today()).unwrap();
    app.invalidate();
    let mut clock = 1_000_000_000u64;
    let mut frame = |app: &mut App| {
        clock += 16_666_666;
        app.frame(clock);
    };
    app.push(Route::Overview);
    frame(&mut app);

    app.push(Route::Entries);
    frame(&mut app);
    app.st.activity.offset = -2;
    app.st.activity.category = Some(1);
    // Back from an entry it stays where it was.
    let id = app.store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions[0].id;
    app.push(Route::EntryView { id });
    frame(&mut app);
    app.pop();
    frame(&mut app);
    assert_eq!((app.st.activity.offset, app.st.activity.category), (-2, Some(1)));
    // Opened again from the tab, it starts on this month with everything.
    app.push(Route::Overview);
    frame(&mut app);
    app.push(Route::Entries);
    frame(&mut app);
    assert_eq!((app.st.activity.offset, app.st.activity.category), (0, None));

    app.push(Route::Insights);
    frame(&mut app);
    app.st.insights.offset = -3;
    app.push(Route::Overview);
    frame(&mut app);
    app.push(Route::Insights);
    frame(&mut app);
    assert_eq!(app.st.insights.offset, 0);
}
