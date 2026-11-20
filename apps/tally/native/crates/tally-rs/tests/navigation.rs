//! Going between pages as a finger would: what a page keeps when Back returns to it, what Back
//! closes first, the notice's Undo wherever it shows, and a list stopped by a touch.

// Amounts written as dollars and cents: 12_50 is $12.50.
#![allow(clippy::inconsistent_digit_grouping)]

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::model::{AccountType, CategoryKind, GoalKind, TxType};
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::views::{TxInput, TxQuery};
use tally::app::{App, Next};
use tally::entry::{Entry, Pick};
use tally::nav::Route;
use tally::ui::{Phase, Scroll, Touch, Ui};

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
        let dir = std::env::temp_dir().join(format!("tally-nav-{}-{name}", std::process::id()));
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
    fn frame(&mut self) -> Next {
        self.clock += 16_666_666;
        self.app.frame(self.clock)
    }
    fn go(&mut self, r: Route) {
        self.app.push(r);
        self.frame();
    }
    fn tap(&mut self, x: f32, y: f32) {
        self.app.ui.tap = Some((x, y));
        self.frame();
    }
    /// The notice's Undo, with its pill's bottom at `bottom` (screen units).
    fn tap_undo(&mut self, bottom: f32) {
        self.tap(390.0 - 16.0 - 40.0, bottom - 12.0 - 24.0);
    }
    fn entries(&self) -> usize {
        let today = tally::model::today();
        let q = TxQuery { from: today.checked_sub(jiff::Span::new().days(400)).ok(), to: Some(today), limit: Some(100_000), ..Default::default() };
        self.app.store.ledger.tx_list(&q, today).unwrap().transactions.len()
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn new_entry() -> Route {
    Route::Entry { id: None, kind: None, account: None, category: None }
}

#[test]
fn a_draft_survives_making_an_account_and_takes_the_new_one() {
    let mut d = Driver::new("entry-account");
    d.go(new_entry());
    let food = d.app.entry.categories.iter().find(|c| c.kind == CategoryKind::Expense).unwrap().id;
    for k in [4, 2] {
        d.app.entry.amount = d.app.entry.amount.digit(k);
    }
    d.app.entry.note = "Farmers market".into();
    d.app.entry.category = Some(food);
    d.app.entry.category_chosen = true;
    // The account sheet's New account, as its row would.
    d.app.entry.made_for = Some(Pick::From);
    d.go(Route::AccountEdit { id: None });
    let made = d.app.store.ledger.account_add("Market card", AccountType::Credit, 0, None, "").unwrap().id;
    d.app.invalidate();
    assert!(d.app.pop());
    d.frame();

    let e = &d.app.entry;
    assert_eq!(d.app.route(), &new_entry());
    assert_eq!((e.amount.minor(), e.note.as_str(), e.category), (42_00, "Farmers market", Some(food)), "the draft stays");
    assert_eq!(e.account, Some(made), "the account just made is picked");
    assert!(e.accounts.iter().any(|a| a.id == made));
}

#[test]
fn a_draft_survives_making_a_category_and_a_transfer_takes_the_new_account_as_its_to() {
    let mut d = Driver::new("entry-category");
    d.go(new_entry());
    d.app.entry.amount = d.app.entry.amount.digit(7);
    d.go(Route::CategoryEdit { id: None, kind: CategoryKind::Expense });
    let made = tally::categories_store::add(&d.app.store, "Plants", CategoryKind::Expense, "dots", 3).unwrap();
    d.app.invalidate();
    d.app.pop();
    d.frame();
    assert_eq!(d.app.entry.amount.minor(), 7_00);
    assert_eq!(d.app.entry.category, Some(made), "the category just made is picked");

    d.app.entry.kind = TxType::Transfer;
    let from = d.app.entry.account;
    d.app.entry.made_for = Some(Pick::To);
    d.go(Route::AccountEdit { id: None });
    let to = d.app.store.ledger.account_add("Trip fund", AccountType::Savings, 0, None, "").unwrap().id;
    d.app.invalidate();
    d.app.pop();
    d.frame();
    assert_eq!((d.app.entry.account, d.app.entry.to_account), (from, Some(to)));
}

#[test]
fn a_new_entry_from_the_tab_starts_empty() {
    let mut d = Driver::new("entry-fresh");
    d.go(new_entry());
    d.app.entry.amount = d.app.entry.amount.digit(5);
    d.app.pop();
    d.frame();
    d.go(new_entry());
    assert!(d.app.entry.amount.is_empty(), "opened anew, not returned to");
}

#[test]
fn back_closes_the_sheet_before_the_page() {
    let mut d = Driver::new("back-sheet");
    d.go(new_entry());
    d.app.entry.amount = d.app.entry.amount.digit(3);
    d.app.entry.picking = Some(Pick::Day);
    d.frame();
    assert!(d.app.back());
    assert_eq!(d.app.route(), &new_entry(), "Back closed the sheet only");
    assert!(d.app.entry.picking.is_none());
    assert_eq!(d.app.entry.amount.minor(), 3_00);
    assert!(d.app.back());
    assert_eq!(d.app.route(), &Route::Overview);
}

#[test]
fn undo_on_a_tab_page_takes_back_the_entry() {
    let mut d = Driver::new("undo-tab");
    let account = d.app.store.ledger.accounts().unwrap()[0].id;
    let before = d.entries();
    let input = TxInput { r#type: TxType::Expense, amount: 9_99, date: tally::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None };
    let tx = d.app.store.add(&input).unwrap();
    d.app.invalidate();
    d.app.say("Expense saved", Some(tx.id));
    d.frame();
    assert_eq!(d.entries(), before + 1);
    let bottom = d.app.canvas.h - d.app.bar_h();
    d.tap_undo(bottom);
    assert_eq!(d.entries(), before, "Undo on Overview deleted the entry");
    assert!(d.app.toast.is_none());
}

#[test]
fn undo_after_save_and_add_another_is_not_a_second_save() {
    let mut d = Driver::new("undo-entry");
    d.go(new_entry());
    let food = d.app.entry.categories.iter().find(|c| c.kind == CategoryKind::Expense).unwrap().id;
    d.app.entry.amount = d.app.entry.amount.digit(6);
    d.app.entry.category = Some(food);
    d.app.entry.pad_open = false;
    d.frame();
    let before = d.entries();
    // "Save and add another", under the Save key.
    let top = d.app.entry.footer_top;
    d.tap(195.0, top + 12.0 + 52.0 + 4.0 + 18.0);
    assert_eq!(d.entries(), before + 1, "saved");
    assert_eq!(d.app.route(), &new_entry(), "and ready for the next");
    d.frame();
    // The notice sits over the keys at the bottom, not on them.
    let bottom = d.app.entry.footer_top - 4.0;
    assert!(bottom <= d.app.entry.footer_top);
    d.tap_undo(bottom);
    assert_eq!(d.entries(), before, "Undo took the entry back and saved nothing");
}

#[test]
fn a_category_deleted_with_nothing_in_it_comes_back_on_undo() {
    let mut d = Driver::new("category-undo");
    let id = tally::categories_store::add(&d.app.store, "Hobbies", CategoryKind::Expense, "dots", 2).unwrap();
    d.app.store.ledger.budget_set(Some(id), 50_00).unwrap();
    d.app.invalidate();
    d.go(Route::Categories);
    d.go(Route::CategoryEdit { id: Some(id), kind: CategoryKind::Expense });
    // "Delete category" ends the editor: scroll to the end and tap it.
    let max = d.app.scroll().max;
    d.app.scroll().off = max;
    d.frame();
    let c = &d.app.canvas;
    let y = c.h - c.inset_bottom - 24.0 - tally::accounts_kit::ACTION_H / 2.0;
    d.tap(195.0, y);
    assert_eq!(d.app.route(), &Route::Categories);
    let gone = |d: &Driver| !d.app.store.ledger.categories().unwrap().iter().any(|c| c.id == id);
    assert!(gone(&d), "deleted at once");
    let bottom = d.app.canvas.h - d.app.canvas.inset_bottom - 8.0;
    d.tap_undo(bottom);
    assert!(!gone(&d), "Undo brought it back");
    assert_eq!(d.app.store.ledger.budget(Some(id)).unwrap(), Some(50_00), "with its budget");
}

#[test]
fn a_goal_deleted_from_the_page_overview_opened_has_undo() {
    let mut d = Driver::new("goal-undo");
    let input = tally::plan_store::GoalInput {
        name: "Bike".into(),
        target: 800_00,
        target_date: None,
        color: 1,
        kind: GoalKind::Savings,
        account_id: None,
        percent: 0,
        start_date: None,
        start_amount: 0,
    };
    let id = tally::plan_store::goal_save(d.app.store.sql().unwrap(), None, &input).unwrap();
    d.app.invalidate();
    d.frame();
    d.go(Route::Goal { id });
    // Its more key, then Delete goal: the second row of the sheet.
    let top = d.app.canvas.inset_top + 10.0;
    d.tap(390.0 - 16.0 + 4.0 - 20.0, top + 2.0 + 20.0);
    let c = &d.app.canvas;
    let sheet_top = c.h - (64.0 + 56.0 + 64.0 + 16.0 + c.inset_bottom);
    d.tap(195.0, sheet_top + 64.0 + 56.0 + 32.0);
    assert_eq!(d.app.route(), &Route::Overview);
    let goal = |d: &Driver| tally::plan_store::goal(d.app.store.sql().unwrap(), id).unwrap().is_some();
    assert!(!goal(&d), "deleted");
    assert!(d.app.toast.as_ref().is_some_and(|t| t.action.is_some()), "the app's notice carries the Undo");
    let bottom = d.app.canvas.h - d.app.bar_h();
    d.tap_undo(bottom);
    assert!(goal(&d), "Undo brought the goal back");
}

#[test]
fn search_keeps_what_was_typed_when_back_from_a_result() {
    let mut d = Driver::new("search-keep");
    d.go(Route::Search);
    assert!(d.app.focus.id.is_some(), "opened from the tab, it types");
    d.app.set_text("rent");
    d.frame();
    let id = d.app.store.ledger.tx_list(&TxQuery { limit: Some(1), ..Default::default() }, tally::model::today()).unwrap().transactions[0].id;
    d.go(Route::EntryView { id });
    d.app.pop();
    d.frame();
    assert_eq!(d.app.st.activity.query, "rent", "back from a result, the search stays");
    assert!(d.app.focus.id.is_none(), "and the keyboard stays down");
    d.app.pop();
    d.go(Route::Overview);
    d.go(Route::Search);
    assert!(d.app.st.activity.query.is_empty(), "opened anew it starts empty");
}

#[test]
fn an_entrys_overview_reads_its_category_as_the_editor_does() {
    let d = Driver::new("entry-view");
    let today = tally::model::today();
    let txs = d.app.store.ledger.tx_list(&TxQuery { limit: Some(200), ..Default::default() }, today).unwrap().transactions;
    let t = txs.iter().find(|t| t.category_id.is_some() && t.r#type == TxType::Expense).expect("a sample expense with a category");
    let fmt = d.app.store.formatter();
    let full = Entry::open(&d.app.store, Some(t.id), None, None, None);
    let view = Entry::view(&d.app.store, t.id);
    assert_eq!(view.original.as_ref().map(|x| x.id), Some(t.id));
    assert_eq!(view.category_line(&fmt), full.category_line(&fmt));
    assert_eq!((view.account, view.amount.minor()), (full.account, full.amount.minor()));
    assert!(Entry::view(&d.app.store, -5).original.is_none(), "a gone entry reads as gone");
}

#[test]
fn the_typed_amount_is_dressed_in_the_locales_own_way() {
    let en = MoneyFormatter::new("CAD", Locale::new("en-CA"));
    assert_eq!(tally::entry::dress("1250.5", &en), "$1,250.5");
    assert_eq!(tally::entry::dress("12", &en), "$12");
    let fr = MoneyFormatter::new("CAD", Locale::new("fr-CA"));
    let shown = tally::entry::dress("12.5", &fr);
    assert!(shown.starts_with("12,5") && shown.ends_with('$'), "{shown:?}");
}

#[test]
fn a_keyboard_the_system_took_away_lets_the_field_go() {
    let mut d = Driver::new("keyboard-gone");
    d.go(Route::Search);
    assert!(d.app.focus.id.is_some());
    d.app.keyboard_up = true;
    d.frame();
    d.app.keyboard_up = false;
    d.frame();
    assert!(d.app.focus.id.is_some(), "a dip (a keyboard switching) keeps it");
    d.app.keyboard_up = true;
    d.frame();
    assert!(d.app.focus.id.is_some());
    d.app.keyboard_up = false;
    d.frame();
    d.clock += 400_000_000;
    d.frame();
    assert!(d.app.focus.id.is_none(), "gone for good, the field let go");
}

#[test]
fn a_notice_wakes_the_app_when_it_is_due_to_go_not_every_frame() {
    let mut d = Driver::new("toast-wake");
    d.app.say("Copied to today", None);
    // Pictures decode on a thread of their own (shared by every test here): give them real time,
    // and the toast very little of its own.
    let mut next = Next::Now;
    for _ in 0..500 {
        d.clock += 1_000_000;
        next = d.app.frame(d.clock);
        if next != Next::Now {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let until = d.app.toast.as_ref().expect("still showing").until_ns;
    assert_eq!(next, Next::At(until + 1));
}

fn touch(ui: &mut Ui, phase: Phase, y: f32, t_ns: u64) {
    ui.touch(Touch { id: 1, phase, x: 100.0, y, t_ns }, 100.0, y);
}

#[test]
fn a_touch_on_a_coasting_list_stops_it_and_opens_nothing() {
    let (mut ui, mut sc) = (Ui::default(), Scroll::default());
    sc.max = 5000.0;
    let view = [0.0, 0.0, 390.0, 800.0];
    // A quick flick up.
    touch(&mut ui, Phase::Down, 600.0, 0);
    for (i, y) in [560.0, 500.0, 430.0, 350.0].iter().enumerate() {
        touch(&mut ui, Phase::Move, *y, (i as u64 + 1) * 10_000_000);
    }
    touch(&mut ui, Phase::Up, 350.0, 50_000_000);
    assert!(sc.update(&ui, view, 0.016));
    ui.end_frame();
    assert!(sc.update(&ui, view, 0.016), "it coasts");
    ui.end_frame();

    // A tap on it, down and up within one frame.
    touch(&mut ui, Phase::Down, 300.0, 200_000_000);
    touch(&mut ui, Phase::Up, 300.0, 260_000_000);
    let moving = sc.update(&ui, view, 0.016);
    assert!(!moving, "the touch stopped it");
    assert!(!ui.tapped(0.0, 0.0, 390.0, 800.0), "and is not a tap on a row");
    ui.end_frame();
    let off = sc.off;
    sc.update(&ui, view, 0.016);
    assert_eq!(sc.off, off);

    // On a still list a tap is a tap.
    touch(&mut ui, Phase::Down, 300.0, 400_000_000);
    touch(&mut ui, Phase::Up, 300.0, 450_000_000);
    sc.update(&ui, view, 0.016);
    assert!(ui.tapped(0.0, 0.0, 390.0, 800.0));
}
