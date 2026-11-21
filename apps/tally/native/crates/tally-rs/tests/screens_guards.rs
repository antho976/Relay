//! Guards on the money screens: a card stored at zero reopens as money owed, a new account opened
//! for an investment starts as one (and pages saved before that still load), an account's earlier
//! worth with nowhere to start reads as its worth now, and one subject's entries page opened twice
//! keeps each page's search.

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::model::AccountType;
use tally::app::App;
use tally::nav::Route;

// The app, driven

struct Host(PathBuf);

impl tally::app::Host for Host {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _: &str) {}
}

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn app(name: &str) -> (App, Dir) {
    let d = std::env::temp_dir().join(format!("tally-screens-guards-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut app = App::new(Arc::new(Host(d.clone())), 1_000_000_000);
    app.resize(1080.0, 2340.0, 90.0, 40.0);
    app.store.load_sample(tally::model::today()).unwrap();
    app.invalidate();
    app.push(Route::Overview);
    (app, Dir(d))
}

fn tick(a: &mut App, clock: &mut u64) {
    *clock += 16_666_666;
    a.frame(*clock);
}

/// Types `text` into the field `id` on the page showing, then lets it go.
fn type_into(a: &mut App, clock: &mut u64, id: &'static str, text: &str) {
    a.focus.id = Some(id);
    a.focus.text.clear();
    tick(a, clock);
    a.set_text(text);
    tick(a, clock);
    a.finish_typing();
    tick(a, clock);
}

fn tap_save(a: &mut App, clock: &mut u64) {
    let top = a.canvas.inset_top + 10.0;
    a.ui.tap = Some((390.0 - 16.0 - 20.0, top + 22.0));
    tick(a, clock);
}

#[test]
fn a_card_stored_at_zero_reopens_as_money_owed() {
    let (mut a, _d) = app("card-zero");
    let digits = a.store.formatter().fraction_digits;
    let id = a.store.ledger.account_add("Bank-made Visa", AccountType::Credit, 0, None, "").unwrap().id;
    let s = tally::accounts_store::stored(&a.store, id).unwrap().unwrap();
    let mut d = tally::accounts_store::draft_of(&s, None, digits);
    assert!(d.owe, "a card's balance is owed");
    d.opening_text = "500".into();
    tally::accounts_store::save(&mut a.store, Some(id), &d, digits).unwrap();
    let s = tally::accounts_store::stored(&a.store, id).unwrap().unwrap();
    assert!(s.row.opening < 0, "saved as a debt, not +500: {}", s.row.opening);
    // Not a card, at zero: nothing owed.
    let id = a.store.ledger.account_add("Bank-made chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let s = tally::accounts_store::stored(&a.store, id).unwrap().unwrap();
    assert!(!tally::accounts_store::draft_of(&s, None, digits).owe);
}

#[test]
fn a_new_account_opened_for_an_investment_starts_as_one() {
    let (mut a, _d) = app("new-investment");
    let mut clock = 1_000_000_000;
    a.push(Route::AccountEdit { id: None, kind: Some(AccountType::Investment) });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "My TFSA");
    tap_save(&mut a, &mut clock);
    let saved = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "My TFSA").expect("saved");
    assert_eq!(saved.r#type, AccountType::Investment);
}

#[test]
fn an_account_page_saved_before_its_kind_still_loads() {
    let old: Route = serde_json::from_str(r#"{"AccountEdit":{"id":null}}"#).unwrap();
    assert_eq!(old, Route::AccountEdit { id: None, kind: None });
    let old: Route = serde_json::from_str(r#"{"AccountEdit":{"id":7}}"#).unwrap();
    assert_eq!(old, Route::AccountEdit { id: Some(7), kind: None });
    let new = Route::AccountEdit { id: None, kind: Some(AccountType::Investment) };
    assert_eq!(serde_json::from_str::<Route>(&serde_json::to_string(&new).unwrap()).unwrap(), new);
}

#[test]
fn an_earlier_worth_with_nowhere_to_start_reads_as_its_worth_now() {
    use tally::invest_logic::worth_then;
    // Priced from holdings then: what it was.
    assert_eq!(worth_then(Some((1_000, true)), false, 9_000), 1_000);
    // Started by then (an opening or a value recorded): its balance then.
    assert_eq!(worth_then(Some((0, false)), true, 9_000), 0);
    // Neither: a first holdings import is not a gain.
    assert_eq!(worth_then(Some((0, false)), false, 9_000), 9_000);
    assert_eq!(worth_then(None, false, 9_000), 9_000);
}

#[test]
fn one_subjects_entries_page_twice_keeps_each_search() {
    let (mut a, _d) = app("same-subject");
    let mut clock = 1_000_000_000;
    let c = a.store.ledger.categories().unwrap().into_iter().find(|c| !c.archived).unwrap().id;
    a.push(Route::Transactions { category: Some(c), account: None });
    tick(&mut a, &mut clock);
    a.st.activity.tx_query = "metro".into();
    tick(&mut a, &mut clock);
    // The same category's page again, over it, from an entry's overview.
    let id = a.store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions[0].id;
    a.push(Route::EntryView { id });
    tick(&mut a, &mut clock);
    a.push(Route::Transactions { category: Some(c), account: None });
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.tx_query, "", "opened anew it starts empty");
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    assert_eq!(a.route(), &Route::Transactions { category: Some(c), account: None });
    assert_eq!(a.st.activity.tx_query, "metro", "the lower page's search outlasts the upper");
}
