//! The sync guards on the ledger side, against a server running relay-money's own
//! `money.sync`: each one held here so it stays fixed. No network: the server is in this process.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use relay_money::ledger::{LedgerError, Ledger};
use relay_money::model::{AccountType, TxType};
use relay_money::views::{Change, TxInput, TxPatch, TxQuery};
use serde_json::Value;
use tally::model::Store;
use tally::sync::Failure;
use tally::sync_ledger::{self as sl, Answer, Applied, Ask};
use tally::sync_net::Requester;

/// The owner's server: `money.sync` as Relay runs it, never asked to replace.
struct Server {
    ledger: Ledger,
    replaces: usize,
}

impl Server {
    fn with_owners_ledger() -> Server {
        let mut ledger = Ledger::open_in_memory().unwrap();
        ledger.ensure_categories().unwrap();
        ledger.load_sample(tally::model::today(), "CAD").unwrap();
        Server { ledger, replaces: 0 }
    }
}

impl Requester for Server {
    fn request(&mut self, op: &str, p: &Value, _wait: Duration) -> Result<Value, Failure> {
        assert_eq!(op, "money.sync");
        let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
        self.replaces += usize::from(replace);
        let changes: Vec<Change> = serde_json::from_value(p["changes"].clone()).unwrap();
        let limit = p.get("limit").and_then(Value::as_u64).map(|l| l as usize);
        let out = self
            .ledger
            .sync_page(p["device"].as_str().unwrap(), replace, p["since"].as_i64().unwrap(), p.get("generation").and_then(Value::as_str), &changes, limit, "t")
            .map_err(|e| match e {
                LedgerError::Stale(m) => Failure { code: sl::STALE.into(), message: m },
                e => Failure { code: "money.invalid".into(), message: e.to_string() },
            })?;
        Ok(serde_json::to_value(out).unwrap())
    }
}

fn uids(l: &Ledger) -> Vec<String> {
    let q = TxQuery { limit: Some(100_000), from: Some(relay_money::Date::constant(2000, 1, 1)), to: Some(relay_money::Date::constant(2100, 1, 1)), ..Default::default() };
    let mut v: Vec<String> = l.tx_list(&q, tally::model::today()).unwrap().transactions.into_iter().map(|t| t.uid).collect();
    v.sort();
    v
}

/// A phone ledger in a folder of its own (the second connection is there, as on the phone),
/// removed when the test is done with it.
struct Phone(Store, PathBuf);

impl Drop for Phone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

fn phone() -> Phone {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-sync-guards-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    Phone(Store::open(&dir.join("tally.db")).unwrap(), dir)
}

fn sync(store: &mut Store, server: &mut Server) -> (Answer, Applied) {
    let (ask, mark) = sl::next_ask(store).unwrap();
    let answer = sl::exchange(server, "Tally RS", ask).unwrap();
    let applied = sl::apply(store, &answer, &mark, false).unwrap();
    (answer, applied)
}

fn entry(store: &mut Store, account: i64, note: &str) -> relay_money::views::Tx {
    store
        .add(&TxInput { r#type: TxType::Expense, amount: 5_00, date: tally::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some(note.into()) })
        .unwrap()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

// 1. A whole pull never erases entries made here while it ran.

#[test]
fn a_whole_pull_is_not_taken_over_entries_made_while_it_ran() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    let (ask, mark) = sl::next_ask(store).unwrap();
    assert!(matches!(ask, Ask::Whole));
    let answer = sl::exchange(&mut server, "Tally RS", ask).unwrap();
    // The person adds an account and an entry while the answer is on its way.
    let account = store.ledger.account_add("Cash", AccountType::Cash, 0, None, "").unwrap().id;
    let mine = entry(store, account, "Made during the pull");
    let applied = sl::apply(store, &answer, &mark, false).unwrap();
    assert_eq!(applied, Applied { received: 0, again: false, ask_first: true, ahead: 0, fewer: None });
    assert!(uids(&store.ledger).contains(&mine.uid), "the entry is still here");
    assert!(store.pref(sl::PULLED).is_none(), "nothing of the answer was kept");
    // Asked and answered yes, a pull from where the ledger stands now takes the server's.
    let (_, applied) = sync(store, &mut server);
    assert!(!applied.ask_first && applied.received > 0);
    assert_eq!(uids(&store.ledger), uids(&server.ledger));
    assert_eq!(server.replaces, 0);
}

#[test]
fn an_unchanged_ledger_takes_the_whole_pull() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let (answer, applied) = sync(&mut p.0, &mut server);
    assert!(answer.whole && !applied.ask_first);
    assert_eq!(uids(&p.0.ledger), uids(&server.ledger));
}

// 3. A server ledger replaced since never silently takes the place of changes never sent.

fn replace_servers_ledger(server: &mut Server) {
    let mut other = Ledger::open_in_memory().unwrap();
    other.ensure_categories().unwrap();
    other.load_sample(tally::model::today(), "USD").unwrap();
    let rows = other.sync("x", false, 0, None, &[], "t").unwrap().changes;
    server.ledger.sync("Kotlin", true, 0, None, &rows, "t").unwrap();
}

#[test]
fn a_replaced_server_ledger_asks_first_when_changes_here_never_went() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    replace_servers_ledger(&mut server);
    let account = store.ledger.accounts().unwrap()[0].id;
    let mine = entry(store, account, "Never sent");
    let (ask, _) = sl::next_ask(store).unwrap();
    let refused = sl::exchange(&mut server, "Tally RS", ask).unwrap_err();
    assert_eq!(refused.code, sl::STALE_UNSENT);
    assert!(uids(&store.ledger).contains(&mine.uid), "nothing here was replaced");
    assert!(!uids(&server.ledger).contains(&mine.uid), "and nothing went up");
    assert_eq!(server.replaces, 0);
    // The person's yes asks for the whole ledger, which then replaces this phone's.
    let mark = sl::Mark::now(store).unwrap();
    let answer = sl::exchange(&mut server, "Tally RS", Ask::Whole).unwrap();
    sl::apply(store, &answer, &mark, true).unwrap();
    assert_eq!(uids(&store.ledger), uids(&server.ledger));
}

#[test]
fn a_replaced_server_ledger_is_taken_by_itself_when_nothing_here_is_unsent() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    sync(&mut p.0, &mut server);
    replace_servers_ledger(&mut server);
    let (answer, applied) = sync(&mut p.0, &mut server);
    assert!(answer.whole && !applied.ask_first);
    assert_eq!(answer.note.as_deref(), Some(sl::TOOK_SERVER_LEDGER));
    assert_eq!(uids(&p.0.ledger), uids(&server.ledger));
}

// A server ledger lost or made again empty (a restart on a disk that does not last, a reinstall,
// an erase on the PC) answers stale: this phone, which may hold the only copy, is not emptied by
// itself.

#[test]
fn a_server_ledger_made_again_empty_does_not_empty_this_phone_unasked() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let account = store.ledger.accounts().unwrap()[0].id;
    let mine = entry(store, account, "Sent before the server lost its ledger");
    sync(store, &mut server);
    let held = uids(&store.ledger);
    assert!(held.contains(&mine.uid));
    // The server comes back with a new, empty ledger.
    let mut fresh = Ledger::open_in_memory().unwrap();
    fresh.ensure_categories().unwrap();
    server.ledger = fresh;
    let (answer, applied) = sync(store, &mut server);
    assert!(answer.whole, "the merge was refused stale and the server's ledger read whole");
    assert!(applied.ask_first, "the person is asked");
    let (there, here) = applied.fewer.expect("held for having far fewer rows");
    assert_eq!(there, 0);
    assert!(here > held.len(), "this phone's accounts and entries: {here}");
    assert_eq!(uids(&store.ledger), held, "this phone keeps its entries");
    assert_eq!(server.replaces, 0);
    // Asked again, the same.
    let (_, applied) = sync(store, &mut server);
    assert!(applied.ask_first && applied.fewer.is_some());
    assert_eq!(uids(&store.ledger), held);
    // Their yes takes the server's ledger, empty as it is.
    let mark = sl::Mark::now(store).unwrap();
    let answer = sl::exchange(&mut server, "Tally RS", Ask::Whole).unwrap();
    let applied = sl::apply(store, &answer, &mark, true).unwrap();
    assert!(!applied.ask_first);
    assert!(uids(&store.ledger).is_empty());
}

// Categories the person changed are something a first pull would replace; the seeded ones are not.

#[test]
fn changed_categories_count_as_something_held() {
    let p = phone();
    assert!(!sl::holds_own(&p.0), "a new ledger holds only the seeded categories");
    let groceries = p.0.ledger.categories().unwrap().into_iter().find(|c| c.name == "Groceries").unwrap();
    p.0.sql().unwrap().execute("UPDATE categories SET name = 'Food' WHERE id = ?1", [groceries.id]).unwrap();
    assert!(!p.0.holds_anything() && sl::holds_own(&p.0), "a renamed category is the person's");
    let q = phone();
    q.0.sql().unwrap().execute("UPDATE categories SET deleted = 1 WHERE name = 'Gifts'", []).unwrap();
    assert!(sl::holds_own(&q.0), "so is one deleted");
    let r = phone();
    r.0.sql()
        .unwrap()
        .execute("INSERT INTO categories (uid, name, kind, color, icon, sort_order, updated_at) VALUES ('mine', 'Pets', 'EXPENSE', 3, 'dots', 99, 1)", [])
        .unwrap();
    assert!(sl::holds_own(&r.0), "and one added");
}

// 5. A change the server skips is not counted as sent, is said, and goes again.

#[test]
fn a_change_the_server_does_not_take_goes_again_and_is_said() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let pushed = store.pref(sl::PUSHED);
    let cursor = store.pref(sl::CURSOR);
    let account = store.ledger.accounts().unwrap()[0].id;
    // One entry stamped by a clock two days fast, which the server refuses, and one it takes.
    let ahead = entry(store, account, "Clock ahead");
    store.sql().unwrap().execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms() + 2 * 86_400_000, &ahead.uid)).unwrap();
    let fine = entry(store, account, "Clock right");
    for round in 0..3 {
        let (answer, applied) = sync(store, &mut server);
        assert_eq!(answer.not_taken, 1, "round {round}: the refused one is counted, the taken one is not");
        assert_eq!(answer.sent, if round == 0 { 1 } else { 0 }, "round {round}: only what the server took counts as sent");
        assert_eq!(answer.note.as_deref(), Some(sl::not_taken_note(1).as_str()));
        assert!(!applied.again, "round {round}: it waits for the next sync, it does not loop");
        assert_eq!(store.pref(sl::PUSHED), pushed, "round {round}: where the push began is kept");
        assert_eq!(store.pref(sl::CURSOR), cursor);
        assert!(!uids(&server.ledger).contains(&ahead.uid));
        assert!(uids(&server.ledger).contains(&fine.uid));
    }
    // The clock is put right and the entry edited: it goes, and the push moves on.
    store.ledger.tx_update(ahead.id, &TxPatch { note: Some("Clock fixed".into()), ..Default::default() }).unwrap();
    store.sql().unwrap().execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms(), &ahead.uid)).unwrap();
    let (answer, _) = sync(store, &mut server);
    assert_eq!((answer.not_taken, answer.note), (0, None));
    assert!(uids(&server.ledger).contains(&ahead.uid));
    assert_ne!(store.pref(sl::PUSHED), pushed);
    let (quiet, applied) = sync(store, &mut server);
    assert_eq!((quiet.sent, quiet.not_taken, applied.received), (0, 0, 0));
}

#[test]
fn an_edit_the_server_holds_newer_is_not_said_to_be_refused() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let account = store.ledger.accounts().unwrap()[0].id;
    let shared = entry(store, account, "Both edit this");
    sync(store, &mut server);
    // Edited here, then on the server later: the server's wins, and that is no refusal.
    store.ledger.tx_update(shared.id, &TxPatch { note: Some("Phone's".into()), ..Default::default() }).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let theirs = server.ledger.tx_list(&TxQuery { limit: Some(100_000), ..Default::default() }, tally::model::today()).unwrap().transactions.into_iter().find(|t| t.uid == shared.uid).unwrap();
    server.ledger.tx_update(theirs.id, &TxPatch { note: Some("Server's".into()), ..Default::default() }).unwrap();
    let (answer, _) = sync(store, &mut server);
    assert_eq!((answer.sent, answer.not_taken, answer.note), (0, 0, None));
    let (quiet, _) = sync(store, &mut server);
    assert_eq!((quiet.sent, quiet.not_taken), (0, 0));
}

#[test]
fn an_edit_made_while_a_sync_ran_goes_next_time_with_no_refusal_said() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    // A change on the server comes down while the person adds an entry here.
    let theirs_account = server.ledger.accounts().unwrap()[0].id;
    server.ledger.tx_add(&TxInput { r#type: TxType::Income, amount: 7_00, date: tally::model::today().to_string(), account_id: theirs_account, to_account_id: None, category_id: None, note: None }).unwrap();
    let (ask, mark) = sl::next_ask(store).unwrap();
    let answer = sl::exchange(&mut server, "Tally RS", ask).unwrap();
    let account = store.ledger.accounts().unwrap()[0].id;
    let meanwhile = entry(store, account, "Meanwhile");
    let applied = sl::apply(store, &answer, &mark, false).unwrap();
    assert!(applied.again);
    // The rows the last answer brought go back up with it; the server has them already.
    let (answer, applied) = sync(store, &mut server);
    assert!(uids(&server.ledger).contains(&meanwhile.uid));
    assert_eq!((answer.not_taken, answer.note, applied.again), (0, None, false));
    assert_eq!(uids(&store.ledger), uids(&server.ledger));
}

// A budget set on the same category on both sides: the server matches it by category, so its own
// comes back under another uid. That is the server's newer copy, not a refusal.

#[test]
fn a_budget_set_on_both_sides_settles_with_no_refusal_said() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let here = store.ledger.categories().unwrap().into_iter().find(|c| format!("{:?}", c.kind).to_uppercase().contains("EXPENSE") && store.ledger.budget(Some(c.id)).unwrap().is_none()).unwrap();
    let there = server.ledger.categories().unwrap().into_iter().find(|c| c.name == here.name).unwrap();
    store.ledger.budget_set(Some(here.id), 10_000).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    server.ledger.budget_set(Some(there.id), 20_000).unwrap();
    let num = |store: &Store, k: &str| store.pref(k).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    let (cursor, pushed) = (num(store, sl::CURSOR), num(store, sl::PUSHED));
    for round in 0..2 {
        let (answer, _) = sync(store, &mut server);
        assert_eq!((answer.not_taken, answer.note.as_deref()), (0, None), "round {round}: the server's newer budget is no refusal");
    }
    assert!(num(store, sl::CURSOR) > cursor && num(store, sl::PUSHED) > pushed, "where sync stands moved on past the budget");
    let (quiet, applied) = sync(store, &mut server);
    assert_eq!((quiet.sent, quiet.not_taken, quiet.changes.len(), applied.received, applied.again), (0, 0, 0, 0, false));
    assert_eq!(store.ledger.budget(Some(here.id)).unwrap(), Some(20_000), "the newer one, the server's, is here");
    assert_eq!(server.ledger.budget(Some(there.id)).unwrap(), Some(20_000));
}

// The same budget held under another uid on each side: an edit of it here the server takes comes
// back under the server's uid with the stamp it went with. That is the edit taken, not the server
// holding it newer, so a refusal in the same push is still said and still goes again.

#[test]
fn a_budget_taken_under_the_servers_uid_does_not_hide_a_refusal() {
    let mut server = Server::with_owners_ledger();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let here = store.ledger.categories().unwrap().into_iter().find(|c| format!("{:?}", c.kind).to_uppercase().contains("EXPENSE") && store.ledger.budget(Some(c.id)).unwrap().is_none()).unwrap();
    let there = server.ledger.categories().unwrap().into_iter().find(|c| c.name == here.name).unwrap();
    store.ledger.budget_set(Some(here.id), 10_000).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    server.ledger.budget_set(Some(there.id), 20_000).unwrap();
    sync(store, &mut server);
    sync(store, &mut server);
    let mine: String = store.sql().unwrap().query_row("SELECT uid FROM budgets WHERE category_id = ?1", [here.id], |r| r.get(0)).unwrap();
    let theirs = server.ledger.sync("probe", false, 0, None, &[], "t").unwrap().changes;
    assert!(theirs.iter().any(|c| c.table == "budgets") && !theirs.iter().any(|c| c.table == "budgets" && c.uid == mine), "each side holds it under its own uid");
    let pushed = store.pref(sl::PUSHED);
    // The budget edited here, and an entry stamped by a clock two days fast in the same push.
    std::thread::sleep(Duration::from_millis(5));
    store.ledger.budget_set(Some(here.id), 30_000).unwrap();
    let account = store.ledger.accounts().unwrap()[0].id;
    let ahead = entry(store, account, "Clock ahead");
    store.sql().unwrap().execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms() + 2 * 86_400_000, &ahead.uid)).unwrap();
    for round in 0..2 {
        let (answer, _) = sync(store, &mut server);
        assert_eq!(server.ledger.budget(Some(there.id)).unwrap(), Some(30_000), "round {round}: the edit was taken");
        assert_eq!(answer.not_taken, 1, "round {round}: the refused entry is said");
        assert_eq!(store.pref(sl::PUSHED), pushed, "round {round}: the push does not move past it");
        assert!(!uids(&server.ledger).contains(&ahead.uid));
    }
    assert_eq!(server.replaces, 0);
}

/// A server whose answers come in pages smaller than the phone asks: a long backlog, made short.
struct SmallPages(Server, u64);

impl Requester for SmallPages {
    fn request(&mut self, op: &str, p: &Value, wait: Duration) -> Result<Value, Failure> {
        let mut p = p.clone();
        if let Some(l) = p.get("limit").and_then(Value::as_u64) {
            p["limit"] = Value::from(l.min(self.1));
        }
        self.0.request(op, &p, wait)
    }
}

#[test]
fn a_refused_change_is_still_said_when_the_answer_comes_in_pages() {
    let mut p = phone();
    let store = &mut p.0;
    let mut server = SmallPages(Server::with_owners_ledger(), 4);
    let (ask, mark) = sl::next_ask(store).unwrap();
    let answer = sl::exchange(&mut server, "Tally RS", ask).unwrap();
    sl::apply(store, &answer, &mark, false).unwrap();
    // The server gets more than a page of changes while this phone makes two entries, one of them
    // stamped by a clock two days fast.
    let theirs = server.0.ledger.accounts().unwrap()[0].id;
    for i in 0..10 {
        server.0.ledger.tx_add(&TxInput { r#type: TxType::Expense, amount: 1_00 + i, date: tally::model::today().to_string(), account_id: theirs, to_account_id: None, category_id: None, note: None }).unwrap();
    }
    let account = store.ledger.accounts().unwrap()[0].id;
    let ahead = entry(store, account, "Clock ahead");
    store.sql().unwrap().execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms() + 2 * 86_400_000, &ahead.uid)).unwrap();
    let fine = entry(store, account, "Clock right");
    let pushed = store.pref(sl::PUSHED);
    for round in 0..2 {
        let (ask, mark) = sl::next_ask(store).unwrap();
        let answer = sl::exchange(&mut server, "Tally RS", ask).unwrap();
        sl::apply(store, &answer, &mark, false).unwrap();
        assert_eq!(answer.not_taken, 1, "round {round}: the refused one is said, the pages notwithstanding");
        assert_eq!(store.pref(sl::PUSHED), pushed, "round {round}: the push does not move past it");
    }
    let held = uids(&server.0.ledger);
    assert!(held.contains(&fine.uid) && !held.contains(&ahead.uid));
    assert_eq!(server.0.replaces, 0);
}

#[test]
fn a_first_sync_still_comes_in_pages() {
    let mut p = phone();
    let mut server = SmallPages(Server::with_owners_ledger(), 4);
    let pages = std::rc::Rc::new(std::cell::Cell::new(0));
    struct Count<'a>(&'a mut SmallPages, std::rc::Rc<std::cell::Cell<usize>>);
    impl Requester for Count<'_> {
        fn request(&mut self, op: &str, p: &Value, wait: Duration) -> Result<Value, Failure> {
            assert_eq!(p.get("limit").and_then(Value::as_u64), Some(u64::from(sl::PAGE)), "a pull asks for pages");
            self.1.set(self.1.get() + 1);
            self.0.request(op, p, wait)
        }
    }
    let (ask, mark) = sl::next_ask(&mut p.0).unwrap();
    let answer = sl::exchange(&mut Count(&mut server, pages.clone()), "Tally RS", ask).unwrap();
    sl::apply(&mut p.0, &answer, &mark, false).unwrap();
    assert!(pages.get() > 5);
    assert_eq!(uids(&p.0.ledger), uids(&server.0.ledger));
}

// A server's rows stamped further ahead than this phone's clock allows are not taken here: they
// must come again, not be stepped over.

/// A server whose ledger is in a file, so a row's stamp can be set as a fast clock would.
fn server_in_file() -> (Server, PathBuf) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("tally-sync-guards-srv-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut ledger = Ledger::open(&dir.join("money.db")).unwrap();
    ledger.ensure_categories().unwrap();
    ledger.load_sample(tally::model::today(), "CAD").unwrap();
    (Server { ledger, replaces: 0 }, dir)
}

/// An entry on the server stamped two days ahead of this phone's clock.
fn entry_ahead_on(server: &mut Server, dir: &std::path::Path) -> String {
    let account = server.ledger.accounts().unwrap()[0].id;
    let t = server.ledger.tx_add(&TxInput { r#type: TxType::Expense, amount: 9_99, date: tally::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some("From a PC".into()) }).unwrap();
    let c = rusqlite::Connection::open(dir.join("money.db")).unwrap();
    c.execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms() + 2 * 86_400_000, &t.uid)).unwrap();
    t.uid
}

#[test]
fn a_server_row_ahead_of_this_phones_clock_comes_again() {
    let (mut server, dir) = server_in_file();
    let mut p = phone();
    let store = &mut p.0;
    sync(store, &mut server);
    let ahead = entry_ahead_on(&mut server, &dir);
    let cursor = store.pref(sl::CURSOR);
    for round in 0..2 {
        let (answer, applied) = sync(store, &mut server);
        assert!(answer.changes.iter().any(|c| c.uid == ahead), "round {round}: it comes again");
        assert_eq!(applied.ahead, 1, "round {round}: and is said");
        assert!(!applied.again, "round {round}: it waits for the next sync, it does not loop");
        assert_eq!(store.pref(sl::CURSOR), cursor, "round {round}: where the pull stands is kept");
        assert!(!uids(&store.ledger).contains(&ahead));
    }
    // The phone's clock put right (the stamp no longer ahead): it comes, and the pull moves on.
    let c = rusqlite::Connection::open(dir.join("money.db")).unwrap();
    c.execute("UPDATE transactions SET updated_at = ?1 WHERE uid = ?2", (now_ms(), &ahead)).unwrap();
    let (_, applied) = sync(store, &mut server);
    assert_eq!(applied.ahead, 0);
    assert!(uids(&store.ledger).contains(&ahead));
    assert_ne!(store.pref(sl::CURSOR), cursor);
    drop(server);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_whole_pull_with_a_row_ahead_of_this_phones_clock_is_not_taken_as_complete() {
    let (mut server, dir) = server_in_file();
    let ahead = entry_ahead_on(&mut server, &dir);
    let mut p = phone();
    let store = &mut p.0;
    let (answer, applied) = sync(store, &mut server);
    assert!(answer.whole);
    assert_eq!(applied.ahead, 1);
    assert!(!uids(&store.ledger).contains(&ahead));
    // The next sync asks for the server's rows from the start again, so the one left out comes.
    let (answer, applied) = sync(store, &mut server);
    assert!(!answer.whole && answer.changes.iter().any(|c| c.uid == ahead));
    assert_eq!(applied.ahead, 1);
    drop(server);
    let _ = std::fs::remove_dir_all(dir);
}

// A paired phone that cannot sync now (sync turned off, or a pull waiting on the person's yes)
// asks for no frames on sync's account: the platform loops read a wake time in the past as draw
// now, so one would redraw for good.

struct Host(PathBuf);

impl tally::app::Host for Host {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _: &str) {}
}

/// A data folder paired with a server nobody can reach, with `prefs` and the sample in its ledger
/// when `sample`.
fn paired_dir(prefs: &str, sample: bool) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("tally-sync-guards-app-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("sync-secret.json"), r#"{"creds":{"host":"h","routes":["wss://127.0.0.1:1/x"],"device":"d","token":"t","paired_at":0},"pending":null}"#).unwrap();
    std::fs::write(d.join("prefs.json"), prefs).unwrap();
    if sample {
        Store::open(&d.join("tally.db")).unwrap().load_sample(tally::model::today()).unwrap();
    }
    d
}

/// Frames a minute after the start, an edit made: sync wants no wake, and once the page's own
/// motion ends (its pictures decoded, off this thread), none is asked for sooner than input.
fn assert_frames_rest(d: &std::path::Path) -> tally::app::App {
    use tally::app::{App, Next};
    let mut app = App::new(std::sync::Arc::new(Host(d.to_path_buf())), 1_000_000_000);
    app.resize(1080.0, 2340.0, 90.0, 40.0);
    let mut t = 2_000_000_000u64;
    app.frame(t);
    app.invalidate();
    t += 60_000_000_000;
    let started = std::time::Instant::now();
    let mut resting = 0;
    for i in 0u64.. {
        let next = app.frame(t);
        t += 16_000_000;
        if i < 10 {
            continue;
        }
        assert_eq!(app.st.sync.wake_at(t), None, "frame {i}: sync wants no wake");
        let soon = match next {
            Next::Now => true,
            Next::At(at) => at <= t,
            Next::OnInput => false,
        };
        resting = if soon { 0 } else { resting + 1 };
        if resting == 20 {
            return app;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "frame {i}: frames are still asked for at once");
        std::thread::sleep(Duration::from_millis(2));
    }
    unreachable!()
}

#[test]
fn a_paired_phone_with_sync_off_asks_for_no_frames() {
    let d = paired_dir(r#"{"onboarded":"1","sync.phone_only":"1"}"#, false);
    assert_frames_rest(&d);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_pull_waiting_on_a_yes_asks_for_no_frames() {
    let d = paired_dir(r#"{"onboarded":"1"}"#, true);
    let app = assert_frames_rest(&d);
    assert!(app.st.sync.awaiting_yes(), "the first pull waits on the person's yes");
    let _ = std::fs::remove_dir_all(d);
}
