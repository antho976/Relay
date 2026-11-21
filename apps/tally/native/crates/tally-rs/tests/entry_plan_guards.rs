//! The entry editor's and the Plan's guards: the notice never sits on a keypad, an Undo stays on
//! its own pages, a pot can't go below nothing, Copy to today copies what the editor shows, and a
//! balance goal's negative target keeps its sign.

// Amounts written as dollars and cents: 1_000_00 is $1,000.00.
#![allow(clippy::inconsistent_digit_grouping)]

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::amount_input::AmountInput;
use relay_money::model::{AccountType, CategoryKind, GoalKind, TxType};
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::views::{TxInput, TxQuery};
use tally::app::App;
use tally::entry::{Entry, Pick, SavedDraft};
use tally::nav::Route;
use tally::plan_store::{self as plan, GoalInput};

struct Host(PathBuf);

impl tally::app::Host for Host {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _: &str) {}
}

struct Driver {
    app: App,
    clock: u64,
    dir: PathBuf,
}

impl Driver {
    fn new(name: &str) -> Driver {
        let dir = std::env::temp_dir().join(format!("tally-guards-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new(Arc::new(Host(dir.clone())), 1_000_000_000);
        app.resize(1080.0, 2340.0, 90.0, 40.0);
        app.store.load_sample(tally::model::today()).unwrap();
        app.invalidate();
        app.push(Route::Overview);
        let mut d = Driver { app, clock: 1_000_000_000, dir };
        d.frame();
        d
    }
    fn frame(&mut self) {
        self.clock += 16_666_666;
        self.app.frame(self.clock);
    }
    fn go(&mut self, r: Route) {
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
    /// The notice's Undo, with its pill's bottom at `bottom` (screen units).
    fn tap_undo(&mut self, bottom: f32) {
        self.tap(390.0 - 16.0 - 40.0, bottom - 12.0 - 24.0);
    }
    /// An expense in the sample ledger with a category: its id, category id and category name.
    fn an_expense(&self) -> (i64, i64, String) {
        let today = tally::model::today();
        let q = TxQuery { from: today.checked_sub(jiff::Span::new().days(400)).ok(), to: Some(today), limit: Some(100_000), ..Default::default() };
        let t = self.app.store.ledger.tx_list(&q, today).unwrap().transactions.into_iter().find(|t| t.r#type == TxType::Expense && t.category_id.is_some()).expect("an expense");
        (t.id, t.category_id.unwrap(), t.category.unwrap())
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

#[test]
fn the_budget_notice_sits_above_the_keypad_not_on_its_bottom_row() {
    let mut d = Driver::new("budget-notice");
    let health = d.app.store.ledger.categories().unwrap().into_iter().find(|c| c.name == "Health").unwrap().id;
    d.go(Route::Budgets);
    d.go(Route::BudgetEdit { category: Some(health) });
    // Save with nothing typed: the notice says to type first, and stays for a while.
    d.save_key();
    assert_eq!(d.app.route(), &Route::BudgetEdit { category: Some(health) });
    assert!(d.app.toast.is_some(), "the notice shows");
    // 5, 0, 0: the zeros are on the keypad's bottom row, where the notice used to sit.
    let c = &d.app.canvas;
    let top = c.h - c.inset_bottom - tally::plan_kit::KEYPAD_H;
    let (kw, kh, gap) = ((358.0 - 12.0) / 3.0, 46.0, 6.0);
    let key = |col: f32, row: f32| (16.0 + col * (kw + gap) + kw / 2.0, top + 14.0 + row * (kh + gap) + kh / 2.0);
    for (x, y) in [key(1.0, 1.0), key(1.0, 3.0), key(1.0, 3.0)] {
        d.tap(x, y);
    }
    assert!(d.app.toast.is_some(), "the notice still shows");
    d.save_key();
    assert_eq!(d.app.route(), &Route::Budgets);
    assert_eq!(d.app.store.ledger.budget(Some(health)).unwrap(), Some(500_00), "$500, not $5");
}

#[test]
fn a_new_page_drops_an_undo_but_words_alone_stay() {
    let mut d = Driver::new("undo-push");
    let account = d.app.store.ledger.accounts().unwrap()[0].id;
    let input = TxInput { r#type: TxType::Expense, amount: 9_99, date: tally::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None };
    let tx = d.app.store.add(&input).unwrap();
    d.app.invalidate();
    d.app.say("Expense saved", Some(tx.id));
    d.frame();
    d.go(Route::Bills);
    assert!(d.app.toast.is_none(), "an Undo for the entry is not offered on Bills");
    assert!(d.app.store.ledger.tx(tx.id).is_ok(), "and the entry stays");

    // Words alone carry on.
    d.app.say("Synced", None);
    d.go(Route::Goals);
    assert!(d.app.toast.is_some());
}

#[test]
fn delete_names_the_stored_entry_and_undo_works_back_on_the_list() {
    let mut d = Driver::new("delete-words");
    let (id, category, name) = d.an_expense();
    d.go(Route::Entries);
    d.go(Route::Entry { id: Some(id), kind: None, account: None, category: None });
    // Changed in the editor to another category, then deleted.
    let other = d.app.entry.categories.iter().find(|c| c.kind == CategoryKind::Expense && c.id != category).unwrap().id;
    d.app.entry.category = Some(other);
    d.frame();
    let top = d.top();
    d.tap(390.0 - 10.0 - 22.0, top + 22.0);
    assert_eq!(d.app.route(), &Route::Entries);
    assert!(d.app.store.ledger.tx(id).is_err(), "deleted");
    let t = d.app.toast.as_ref().expect("the notice");
    assert_eq!(t.words, format!("{name} entry deleted"));
    let bottom = d.app.canvas.h - d.app.bar_h();
    d.tap_undo(bottom);
    assert!(d.app.store.ledger.tx(id).is_ok(), "Undo put it back");
}

#[test]
fn copy_to_today_copies_what_the_editor_shows() {
    let d = Driver::new("copy");
    let (id, _, _) = d.an_expense();
    let mut e = Entry::open(&d.app.store, Some(id), None, None, None);
    let today = tally::model::today();
    e.amount = AmountInput::of(42_00, 2);
    e.note = "  Edited  ".into();
    let input = e.copy_input(today).expect("it can be copied");
    let stored = d.app.store.ledger.tx(id).unwrap();
    assert_eq!(input.amount, 42_00, "the amount as typed now");
    assert_eq!((input.date, input.note.as_deref(), input.category_id), (today.to_string(), Some("Edited"), stored.category_id));

    // Nothing to copy says why, as Save would.
    e.category = None;
    assert_eq!(e.copy_input(today).unwrap_err(), "Pick a category");
}

#[test]
fn an_edited_entry_switched_to_transfer_never_goes_to_its_own_account() {
    let mut d = Driver::new("from-to");
    let accounts = d.app.store.ledger.accounts().unwrap();
    assert!(accounts.len() >= 2);
    // An expense in the account a fresh entry would send a transfer to.
    let fresh = Entry::fresh(&d.app.store);
    let to = fresh.to_account.unwrap();
    let input = TxInput { r#type: TxType::Expense, amount: 5_00, date: tally::model::today().to_string(), account_id: to, to_account_id: None, category_id: None, note: None };
    let tx = d.app.store.add(&input).unwrap();
    d.app.invalidate();
    let e = Entry::open(&d.app.store, Some(tx.id), None, None, None);
    assert_eq!(e.account, Some(to));
    assert!(e.to_account.is_some() && e.to_account != e.account, "{:?} -> {:?}", e.account, e.to_account);
}

#[test]
fn a_kept_draft_remembers_which_side_a_new_account_is_for() {
    let d = Driver::new("made-for");
    let mut e = Entry::fresh(&d.app.store);
    e.made_for = Some(Pick::To);
    let kept = e.saved_draft();
    let json = serde_json::to_string(&kept).unwrap();
    let back: SavedDraft = serde_json::from_str(&json).unwrap();
    let mut again = Entry::fresh(&d.app.store);
    again.restore_draft(&back);
    assert_eq!(again.made_for, Some(Pick::To));

    // A draft kept before it had the field still reads.
    let mut old = serde_json::to_value(&kept).unwrap();
    old.as_object_mut().unwrap().remove("made_for");
    let d: SavedDraft = serde_json::from_value(old).unwrap();
    assert_eq!(d.made_for, None);
}

#[test]
fn a_pot_never_gives_out_more_than_it_holds() {
    let f = fmt();
    assert_eq!(tally::plan::take_out_problem(100_00, 100_00, &f), None, "all of it is fine");
    assert_eq!(tally::plan::take_out_problem(100_00, 40_00, &f), None);
    assert!(tally::plan::take_out_problem(100_00, 500_00, &f).is_some_and(|w| w.contains("$100.00")));
    assert!(tally::plan::take_out_problem(0, 1, &f).is_some());
}

#[test]
fn a_balance_goal_target_keeps_its_minus() {
    use tally::plan::goal_target;
    assert_eq!(goal_target("-1000.00", GoalKind::Balance, 2), Some(-1_000_00));
    assert_eq!(goal_target(" -0.50", GoalKind::Balance, 2), Some(-50));
    assert_eq!(goal_target("\u{2212}20", GoalKind::Balance, 2), Some(-20_00));
    assert_eq!(goal_target("1000", GoalKind::Balance, 2), Some(1_000_00));
    assert_eq!(goal_target("0", GoalKind::Balance, 2), Some(0));
    // Other kinds aim above zero, as ever.
    assert_eq!(goal_target("-5", GoalKind::Savings, 2), Some(5_00));
}

#[test]
fn saving_a_negative_balance_goal_unchanged_keeps_it_negative() {
    let mut d = Driver::new("balance-goal");
    let card = d.app.store.ledger.account_add("Visa", AccountType::Credit, -3_000_00, None, "").unwrap().id;
    let input = GoalInput {
        name: "Down to a grand".into(),
        target: -1_000_00,
        target_date: None,
        color: 1,
        kind: GoalKind::Balance,
        account_id: Some(card),
        percent: 0,
        start_date: Some(tally::model::today()),
        start_amount: -3_000_00,
    };
    let id = plan::goal_save(d.app.store.sql().unwrap(), None, &input).unwrap();
    d.app.invalidate();
    d.go(Route::Goals);
    d.go(Route::GoalEdit { id: Some(id) });
    d.save_key();
    assert_eq!(d.app.route(), &Route::Goals, "saved");
    assert_eq!(plan::goal(d.app.store.sql().unwrap(), id).unwrap().unwrap().target, -1_000_00);

    // Typed with a minus on purpose, it is saved below zero too.
    d.go(Route::GoalEdit { id: Some(id) });
    d.app.focus.id = Some("plan.goal.target");
    d.app.set_text("-250");
    d.frame();
    d.app.finish_typing();
    d.frame();
    d.save_key();
    assert_eq!(plan::goal(d.app.store.sql().unwrap(), id).unwrap().unwrap().target, -250_00);
}

#[test]
fn back_on_the_editor_picks_only_what_it_asked_to_make() {
    let mut d = Driver::new("came-back");
    let (id, category, _) = d.an_expense();
    let mut e = Entry::open(&d.app.store, Some(id), None, None, None);
    let account = e.account;

    // An account and a category that arrive while the editor waits (a sync brought them in) are
    // offered, not picked: Save would move the entry to them.
    let synced = d.app.store.ledger.account_add("Synced card", AccountType::Credit, 0, None, "").unwrap().id;
    let synced_cat = tally::categories_store::add(&d.app.store, "Synced", CategoryKind::Expense, "dots", 3).unwrap();
    e.came_back(&d.app.store);
    assert_eq!((e.account, e.category), (account, Some(category)));
    assert!(e.accounts.iter().any(|a| a.id == synced) && e.categories.iter().any(|c| c.id == synced_cat));

    // Asked for, each is picked.
    e.made_for = Some(Pick::From);
    e.made_category = true;
    let made = d.app.store.ledger.account_add("Made card", AccountType::Credit, 0, None, "").unwrap().id;
    let made_cat = tally::categories_store::add(&d.app.store, "Made", CategoryKind::Expense, "dots", 4).unwrap();
    e.came_back(&d.app.store);
    assert_eq!((e.account, e.category), (Some(made), Some(made_cat)));
    assert!(e.made_for.is_none() && !e.made_category, "each ask is used once");

    // The ask for a category is kept across the system stopping Tally, and an old draft reads.
    e.made_category = true;
    let kept = e.saved_draft();
    let mut again = Entry::open(&d.app.store, Some(id), None, None, None);
    again.restore_draft(&serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap());
    assert!(again.made_category);
    let mut old = serde_json::to_value(&kept).unwrap();
    old.as_object_mut().unwrap().remove("made_category");
    assert!(!serde_json::from_value::<SavedDraft>(old).unwrap().made_category);
}

#[test]
fn removing_money_put_in_never_leaves_a_pot_below_nothing() {
    use tally::plan::remove_problem;
    // $100 in, $60 out: the pot holds $40, so the $100 can't go.
    assert!(remove_problem(40_00, 100_00).is_some());
    assert_eq!(remove_problem(100_00, 100_00), None, "down to nothing is fine");
    // Removing a take-out puts the money back.
    assert_eq!(remove_problem(40_00, -60_00), None);
}

#[test]
fn the_notice_waits_unseen_while_the_note_keyboard_is_up() {
    let mut d = Driver::new("toast-keyboard");
    let (id, _, _) = d.an_expense();
    d.go(Route::Entry { id: None, kind: None, account: None, category: None });
    d.app.say("Expense saved", Some(id));
    d.app.keyboard_up = true;
    d.frame();
    // Where the notice would sit, just above the keyboard: a tap there is not its Undo.
    let bottom = d.app.canvas.h - d.app.canvas.inset_bottom - 4.0;
    d.tap_undo(bottom);
    assert!(d.app.store.ledger.tx(id).is_ok(), "the saved entry stays");
    assert!(d.app.toast.is_some(), "the notice stays live");
    d.app.keyboard_up = false;
    d.frame();
    assert!(d.app.toast.is_some(), "and shows again once the keyboard goes");
}

#[test]
fn a_new_entry_for_an_archived_account_or_category_shows_it_and_keeps_it() {
    let mut d = Driver::new("archived-route");
    let account = d.app.store.ledger.accounts().unwrap()[0].id;
    tally::accounts_store::set_archived(&mut d.app.store, account, true).unwrap();
    let cat = d.app.store.ledger.categories().unwrap().into_iter().find(|c| c.kind == CategoryKind::Expense && !c.archived).unwrap();
    tally::categories_store::update(&d.app.store, cat.id, &cat.name, &cat.icon, cat.color, true).unwrap();
    d.app.invalidate();

    let mut e = Entry::open(&d.app.store, None, None, Some(account), Some(cat.id));
    assert_eq!((e.account, e.category), (Some(account), Some(cat.id)));
    assert!(e.accounts.iter().any(|a| a.id == account), "the chip names the archived account");
    assert!(e.categories.iter().any(|c| c.id == cat.id), "and the archived category");
    // A trip to another page and back keeps both.
    e.came_back(&d.app.store);
    assert_eq!((e.account, e.category), (Some(account), Some(cat.id)));

    // An account or category gone altogether is not used.
    let e = Entry::open(&d.app.store, None, None, Some(999_999), Some(999_999));
    assert!(e.account.is_some_and(|a| a != 999_999) && e.category.is_none());
}

#[test]
fn editing_an_entry_deleted_meanwhile_leaves_rather_than_add_a_new_one() {
    let mut d = Driver::new("edit-gone");
    let (id, _, _) = d.an_expense();
    d.app.store.delete(id).unwrap();
    d.app.invalidate();
    d.go(Route::Entries);
    let count = |d: &Driver| {
        let today = tally::model::today();
        let q = TxQuery { from: today.checked_sub(jiff::Span::new().days(4000)).ok(), to: Some(today), limit: Some(100_000), ..Default::default() };
        d.app.store.ledger.tx_list(&q, today).unwrap().transactions.len()
    };
    let before = count(&d);
    d.go(Route::Entry { id: Some(id), kind: None, account: None, category: None });
    d.frame();
    assert_eq!(d.app.route(), &Route::Entries, "the editor left");
    assert_eq!(d.app.toast.as_ref().map(|t| t.words.as_str()), Some("This entry is gone"));
    assert_eq!(count(&d), before, "nothing was added");
}

#[test]
fn putting_back_a_take_out_never_leaves_a_pot_below_nothing() {
    use tally::plan::restore_problem;
    // $100 in, $60 out, the $60 removed, then the $100 removed: the pot holds nothing, so the
    // take-out can't come back.
    assert!(restore_problem(0, -60_00).is_some());
    assert_eq!(restore_problem(60_00, -60_00), None, "down to nothing is fine");
    assert_eq!(restore_problem(0, 100_00), None, "money put in always comes back");
}

#[test]
fn an_undo_waits_while_the_keypad_is_up_but_words_alone_show() {
    let mut d = Driver::new("toast-keypad");
    let (id, _, _) = d.an_expense();
    d.go(Route::Entry { id: None, kind: None, account: None, category: None });
    assert!(d.app.entry.pad_open);
    d.app.say("Expense saved", Some(id));
    d.frame();
    // Where the notice would sit, over the account and day chips: a tap there is not its Undo.
    let bottom = d.app.entry.footer_top - 4.0;
    d.tap_undo(bottom);
    assert!(d.app.store.ledger.tx(id).is_ok(), "the saved entry stays");
    assert!(d.app.toast.is_some(), "the notice stays live");
    // Folded (a category or a chip picked folds it) it still waits: it would sit over the
    // account and day chips, Undo under the day.
    d.app.entry.pad_open = false;
    d.frame();
    let bottom = d.app.entry.footer_top - 4.0;
    d.tap_undo(bottom);
    assert!(d.app.store.ledger.tx(id).is_ok(), "the saved entry still stays");
    // Words alone show on the new entry (why Save did nothing).
    d.app.say("Pick a category", None);
    d.frame();
    assert!(d.app.toast.is_some());
    // Back on the page under it, an Undo shows, and works.
    d.app.say("Expense saved", Some(id));
    d.app.pop();
    d.frame();
    let bottom = d.app.canvas.h - d.app.bar_h();
    d.tap_undo(bottom);
    assert!(d.app.store.ledger.tx(id).is_err(), "Undo took it back");
}

#[test]
fn the_notice_never_takes_a_tap_meant_for_a_sheet_or_the_drawer() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut d = Driver::new("toast-covered");
    let undone = Arc::new(AtomicBool::new(false));
    let say = |d: &mut Driver| {
        let u = undone.clone();
        d.app.say_undo("Goal deleted", move |_| {
            u.store(true, Ordering::SeqCst);
            Ok(())
        });
        d.frame();
    };
    let bottom = d.app.canvas.h - d.app.bar_h();
    // The drawer out: a tap where Undo would be closes the drawer instead.
    say(&mut d);
    d.app.drawer_open = true;
    d.app.drawer = 1.0;
    d.tap_undo(bottom);
    assert!(!undone.load(Ordering::SeqCst), "no Undo under the drawer");
    assert!(!d.app.drawer_open, "the tap closed the drawer");
    assert!(d.app.toast.is_some(), "the notice stays live");
    d.app.drawer = 0.0;
    d.frame();
    d.tap_undo(bottom);
    assert!(undone.load(Ordering::SeqCst), "the drawer gone, Undo works");

    // A sheet or dialog over the page (`widgets::hold` marks it): the tap is not Undo's.
    undone.store(false, Ordering::SeqCst);
    say(&mut d);
    d.app.ui.cover();
    d.app.ui.end_frame();
    d.tap_undo(bottom);
    assert!(!undone.load(Ordering::SeqCst), "no Undo over a sheet");
}

#[test]
fn an_entry_deleted_while_its_editor_was_covered_leaves_the_editor() {
    let mut d = Driver::new("edit-gone-later");
    let (id, _, _) = d.an_expense();
    d.go(Route::Entries);
    d.go(Route::Entry { id: Some(id), kind: None, account: None, category: None });
    // A page the editor opened (a new account), and meanwhile a sync deletes the entry.
    d.go(Route::AccountEdit { id: None, kind: None });
    d.app.store.delete(id).unwrap();
    d.app.invalidate();
    assert!(d.app.pop());
    d.frame();
    assert_eq!(d.app.route(), &Route::Entries, "the editor left");
    assert_eq!(d.app.toast.as_ref().map(|t| t.words.as_str()), Some("This entry is gone"));
}

/// How many live entries a bill named `name` has posted.
fn posted(app: &App, name: &str) -> i64 {
    app.store
        .sql()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM transactions t JOIN recurring r ON r.id = t.recurring_id WHERE r.name = ?1 AND t.deleted = 0", [name], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_new_auto_post_bill_due_today_posts_today_and_only_once() {
    let mut d = Driver::new("bill-due-today");
    d.go(Route::Bills);
    d.go(Route::BillEdit { id: None });
    // The editor's fields as typed, through its kept draft: auto-post, first due today.
    let mut saved = serde_json::to_value(d.app.saved()).unwrap();
    let draft = saved["drafts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["route"] == serde_json::to_value(Route::BillEdit { id: None }).unwrap())
        .expect("the bill editor's draft");
    let bill = draft["draft"].as_object_mut().unwrap().values_mut().next().unwrap().as_object_mut().unwrap().values_mut().next().unwrap();
    bill["name"] = "Due today gym".into();
    bill["amount_text"] = "25".into();
    bill["anchor"] = tally::model::today().to_string().into();
    bill["auto_post"] = true.into();
    bill["active"] = true.into();
    d.app.restore(&serde_json::from_value(saved).unwrap());
    d.frame();
    assert_eq!(d.app.route(), &Route::BillEdit { id: None });
    d.save_key();
    assert_eq!(d.app.route(), &Route::Bills, "saved");
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(posted(&d.app, "Due today gym"), 1, "posted today, once");
}

#[test]
fn copy_to_today_offers_an_undo_that_takes_the_copy_out() {
    let mut d = Driver::new("copy-undo");
    let (id, _, _) = d.an_expense();
    let count = |d: &Driver| d.app.store.sql().unwrap().query_row("SELECT COUNT(*) FROM transactions WHERE deleted = 0", [], |r| r.get::<_, i64>(0)).unwrap();
    let before = count(&d);
    d.go(Route::Entries);
    d.go(Route::Entry { id: Some(id), kind: None, account: None, category: None });
    let top = d.top();
    // The header's Copy key, left of Delete.
    d.tap(390.0 - 10.0 - 44.0 - 46.0 + 22.0, top + 22.0);
    assert_eq!(d.app.route(), &Route::Entries);
    assert_eq!(count(&d), before + 1, "copied");
    let t = d.app.toast.as_ref().expect("the notice");
    assert_eq!(t.words, "Copied to today");
    assert!(t.undo.is_some(), "with an Undo");
    let bottom = d.app.canvas.h - d.app.bar_h();
    d.tap_undo(bottom);
    assert_eq!(count(&d), before, "Undo took the copy out");
    assert!(d.app.store.ledger.tx(id).is_ok(), "the entry copied stays");
}

/// Opens the bill's editor, scrolled to its foot, and taps Delete bill there.
fn delete_bill(d: &mut Driver, id: i64) {
    d.go(Route::Bills);
    d.go(Route::BillEdit { id: Some(id) });
    let max = d.app.nav.last().unwrap().1.max;
    d.app.nav.last_mut().unwrap().1.off = max.max(0.0);
    d.frame();
    let foot = d.app.canvas.h - d.app.canvas.inset_bottom - 24.0 + max.min(0.0);
    d.tap(195.0, foot - 26.0);
    d.frame();
    assert_eq!(d.app.route(), &Route::Bills);
    assert!(plan::bill(d.app.store.sql().unwrap(), id).unwrap().is_none(), "deleted");
}

#[test]
fn a_plan_undo_stays_on_the_page_it_was_made_on() {
    let mut d = Driver::new("plan-undo-route");
    let id = plan::bills(d.app.store.sql().unwrap()).unwrap()[0].bill.id;
    // The Plan's pill sits with its bottom 8 above the screen's.
    let pill = |d: &Driver| d.app.canvas.h - d.app.canvas.inset_bottom - 8.0;

    // On Bills, where it was made, its Undo works.
    delete_bill(&mut d, id);
    let bottom = pill(&d);
    d.tap_undo(bottom);
    assert!(plan::bill(d.app.store.sql().unwrap(), id).unwrap().is_some(), "Undo put it back");

    // Another Plan page opened (Goals, from the drawer) drops it: a tap where the pill was is
    // not its Undo.
    delete_bill(&mut d, id);
    d.go(Route::Goals);
    d.frame();
    let bottom = pill(&d);
    d.tap_undo(bottom);
    assert!(plan::bill(d.app.store.sql().unwrap(), id).unwrap().is_none(), "no Undo on Goals");
}

#[test]
fn editing_a_bill_or_goal_deleted_meanwhile_leaves_rather_than_make_a_new_one() {
    let mut d = Driver::new("plan-gone");
    let count = |d: &Driver, table: &str| d.app.store.sql().unwrap().query_row(&format!("SELECT COUNT(*) FROM {table} WHERE deleted = 0"), [], |r| r.get::<_, i64>(0)).unwrap();
    let bill: i64 = d.app.store.sql().unwrap().query_row("SELECT id FROM recurring WHERE deleted = 0 LIMIT 1", [], |r| r.get(0)).expect("a sample bill");
    plan::bill_delete(d.app.store.sql().unwrap(), bill).unwrap();
    d.app.invalidate();
    d.go(Route::Bills);
    let bills = count(&d, "recurring");
    d.go(Route::BillEdit { id: Some(bill) });
    d.frame();
    assert_eq!(d.app.route(), &Route::Bills, "the bill editor left");
    assert_eq!(d.app.toast.as_ref().map(|t| t.words.as_str()), Some("This bill is gone"));
    assert_eq!(count(&d, "recurring"), bills, "no bill was made");

    let goal: i64 = d.app.store.sql().unwrap().query_row("SELECT id FROM goals WHERE deleted = 0 LIMIT 1", [], |r| r.get(0)).expect("a sample goal");
    plan::goal_delete(d.app.store.sql().unwrap(), goal).unwrap();
    d.app.invalidate();
    d.go(Route::Goals);
    let goals = count(&d, "goals");
    d.go(Route::GoalEdit { id: Some(goal) });
    d.frame();
    assert_eq!(d.app.route(), &Route::Goals, "the goal editor left");
    assert_eq!(d.app.toast.as_ref().map(|t| t.words.as_str()), Some("This goal is gone"));
    assert_eq!(count(&d, "goals"), goals, "no goal was made");
}

#[test]
fn an_entry_shows_its_bill_again_after_the_bill_delete_is_undone() {
    let mut d = Driver::new("bill-undo-entry");
    let (tx, bill): (i64, i64) = d
        .app
        .store
        .sql()
        .unwrap()
        .query_row(
            "SELECT t.id, r.id FROM transactions t JOIN recurring r ON r.id = t.recurring_id AND r.deleted = 0 WHERE t.deleted = 0 LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("an entry a sample bill posted");
    d.go(Route::Entries);
    d.go(Route::Entry { id: Some(tx), kind: None, account: None, category: None });
    assert!(d.app.entry.bill.is_some(), "the entry shows its bill");
    plan::bill_delete(d.app.store.sql().unwrap(), bill).unwrap();
    d.app.invalidate();
    d.frame();
    assert!(d.app.entry.bill.is_none(), "gone with the bill");
    plan::bill_restore(d.app.store.sql().unwrap(), bill).unwrap();
    d.app.invalidate();
    d.frame();
    assert!(d.app.entry.bill.is_some(), "back with the bill's Undo");
}
