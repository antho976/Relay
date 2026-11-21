//! The app as the platform drives it: a ledger that will not open, text and Done in one batch of
//! input, a page under a sheet, and the pages kept across the process being stopped.

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::model::TxType;
use tally::app::{App, Saved};
use tally::nav::Route;
use tally::ui::{Phase, Touch};

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

fn dir(name: &str) -> Dir {
    let d = std::env::temp_dir().join(format!("tally-platform-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    Dir(d)
}

fn app(dir: &Dir) -> App {
    let mut app = App::new(Arc::new(Host(dir.0.clone())), 1_000_000_000);
    app.resize(1080.0, 2340.0, 90.0, 40.0);
    app
}

#[test]
fn a_ledger_that_will_not_open_blocks_the_app_until_it_does() {
    let d = dir("blocked");
    // A folder where the ledger's file should be: it cannot be opened.
    std::fs::create_dir_all(d.0.join("tally.db")).unwrap();
    let mut a = app(&d);
    assert!(a.ledger_error.is_some(), "it says so");
    a.frame(1_016_000_000);
    assert!(!a.back(), "Back leaves; there is no page to go to");

    // Still in the way: Try again says why, and the app stays blocked.
    assert!(!a.retry_ledger());
    assert!(a.ledger_error.is_some());

    std::fs::remove_dir(d.0.join("tally.db")).unwrap();
    assert!(a.retry_ledger(), "opens once the cause is gone");
    assert!(a.ledger_error.is_none());
    assert_eq!(a.route(), &Route::Onboarding, "a new ledger starts at the welcome");
    a.store.set_pref("onboarded", "1").unwrap();
    a.frame(1_032_000_000);
    assert!(d.0.join("tally.db").is_file(), "written to disk, not memory");
}

fn tick(a: &mut App, clock: &mut u64) {
    *clock += 16_666_666;
    a.frame(*clock);
}

#[test]
fn text_that_comes_with_done_reaches_the_field() {
    let d = dir("done");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::AccountEdit { id: None, kind: None });
    let mut clock = 1_000_000_000;
    tick(&mut a, &mut clock);
    // The name field has the keyboard; then one batch of input brings its text and Done, with no
    // frame between them.
    a.focus.id = Some("account.name");
    a.focus.text.clear();
    tick(&mut a, &mut clock);
    a.set_text("Cash box");
    a.done_typing(clock + 1);
    assert!(a.focus.id.is_none(), "the field let go");
    tick(&mut a, &mut clock);
    // Save, at the header's right.
    let top = a.canvas.inset_top + 10.0;
    a.ui.tap = Some((390.0 - 16.0 - 20.0, top + 22.0));
    tick(&mut a, &mut clock);
    let names: Vec<String> = a.store.ledger.accounts().unwrap().into_iter().map(|x| x.name).collect();
    assert!(names.iter().any(|n| n == "Cash box"), "saved with what was typed: {names:?}");
}

#[test]
fn the_page_under_a_sheet_does_not_scroll() {
    let d = dir("covered");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    let mut clock = 1_000_000_000u64;
    for _ in 0..3 {
        clock += 16_666_666;
        a.frame(clock);
    }
    assert!(a.scroll().max > 100.0, "Overview scrolls");
    let u = a.canvas.u;
    let mut drag = |a: &mut App, covered: bool| -> f32 {
        let before = a.scroll().off;
        if covered {
            // As a sheet's `widgets::hold` marks it on the frame before.
            a.ui.cover();
            a.ui.end_frame();
        }
        let t = clock;
        a.touch(Touch { id: 1, phase: Phase::Down, x: 500.0, y: 1600.0, t_ns: t });
        a.touch(Touch { id: 1, phase: Phase::Move, x: 500.0, y: 1600.0 - 60.0 * u, t_ns: t + 10_000_000 });
        a.touch(Touch { id: 1, phase: Phase::Move, x: 500.0, y: 1600.0 - 120.0 * u, t_ns: t + 20_000_000 });
        if covered {
            // The move is the sheet's this frame too.
            a.ui.cover();
        }
        clock += 16_666_666;
        a.frame(clock);
        a.touch(Touch { id: 1, phase: Phase::Cancel, x: 500.0, y: 1600.0 - 120.0 * u, t_ns: t + 30_000_000 });
        clock += 16_666_666;
        a.frame(clock);
        a.scroll().off - before
    };
    assert_eq!(drag(&mut a, true), 0.0, "covered: still");
    assert!(drag(&mut a, false) > 50.0, "uncovered: it follows the finger");
}

#[test]
fn the_pages_open_outlast_the_process() {
    let d = dir("saved");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.open_from_drawer(Route::Budgets);
    a.push(Route::Entry { id: None, kind: Some(TxType::Income), account: None, category: Some(3) });
    let saved = a.saved();
    let bytes = serde_json::to_vec(&Saved { picking: Some("text/*".into()), ..saved.clone() }).unwrap();
    drop(a);

    let mut b = app(&d);
    let back: Saved = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back.picking.as_deref(), Some("text/*"));
    b.restore(&back);
    assert_eq!(b.saved(), saved, "the same pages, in order");
    assert!(b.drawer_page() || b.is_sub_page());
    assert!(b.pop());
    assert!(b.drawer_page(), "Budgets is still the drawer's page");

    // Nothing kept is nothing done; a first run is never skipped.
    let e = dir("saved-empty");
    let mut c = app(&e);
    c.restore(&saved);
    assert_eq!(c.route(), &Route::Onboarding);
}

#[test]
fn an_entry_being_typed_outlasts_the_process() {
    let d = dir("draft");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Entry { id: None, kind: None, account: None, category: None });
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    // Typed and picked: an amount mid-way ("12." keeps its point), a note, another account, a
    // category, yesterday.
    let accounts: Vec<i64> = a.entry.accounts.iter().map(|x| x.id).collect();
    assert!(accounts.len() >= 2, "the sample has accounts to choose from");
    let other = *accounts.iter().find(|id| Some(**id) != a.entry.account).unwrap();
    let category = a.entry.categories.iter().find(|c| c.kind == relay_money::model::CategoryKind::Expense).unwrap().id;
    a.entry.amount = relay_money::amount_input::AmountInput::new("12.", a.entry.amount.fraction_digits);
    a.entry.note = "Farmers market".into();
    a.entry.account = Some(other);
    a.entry.category = Some(category);
    a.entry.category_chosen = true;
    let yesterday = tally::model::today().yesterday().unwrap();
    a.entry.date = yesterday;
    tick(&mut a, &mut clock);
    let saved = a.saved();
    assert!(saved.entry.is_some(), "kept while the editor is open");
    let bytes = serde_json::to_vec(&saved).unwrap();
    drop(a);

    let mut b = app(&d);
    b.restore(&serde_json::from_slice(&bytes).unwrap());
    // The page's own frames keep it: nothing opens it afresh after the restore.
    let mut clock = 1_000_000_000u64;
    for _ in 0..3 {
        tick(&mut b, &mut clock);
    }
    assert!(matches!(b.route(), Route::Entry { .. }));
    assert_eq!(b.entry.amount.text, "12.");
    assert_eq!(b.entry.note, "Farmers market");
    assert_eq!(b.entry.account, Some(other));
    assert_eq!(b.entry.category, Some(category));
    assert_eq!(b.entry.date, yesterday);
    assert_eq!(b.saved(), saved, "the same draft, kept again");

    // An account or category gone since is dropped; the editor keeps the one it opened with.
    let mut gone = saved.clone();
    let draft = gone.entry.as_mut().unwrap();
    draft.account = Some(999_999);
    draft.category = Some(999_998);
    let e = dir("draft-gone");
    let mut c = app(&e);
    c.store.load_sample(tally::model::today()).unwrap();
    c.invalidate();
    c.push(Route::Overview);
    c.restore(&gone);
    assert_eq!(c.entry.note, "Farmers market", "the rest is kept");
    assert!(c.entry.account.is_some_and(|id| id != 999_999 && c.entry.accounts.iter().any(|x| x.id == id)));
    assert_ne!(c.entry.category, Some(999_998));

    // Kept state from before drafts were kept still loads.
    let old = br#"{"nav":["Overview"],"drawer_depth":null}"#;
    let old: Saved = serde_json::from_slice(old).unwrap();
    assert!(old.entry.is_none());
}

#[test]
fn entries_opened_from_overview_outlast_the_process() {
    let d = dir("see-all");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    // Overview's "See all" opens Entries over it, with Back; a new entry from there.
    a.push_over(Route::Entries);
    a.push(Route::Entry { id: None, kind: None, account: None, category: None });
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    a.entry.note = "Bakery".into();
    tick(&mut a, &mut clock);
    let saved = a.saved();
    assert_eq!(saved.nav.len(), 3);
    assert!(saved.entry.is_some());
    let bytes = serde_json::to_vec(&saved).unwrap();
    drop(a);

    let mut b = app(&d);
    b.restore(&serde_json::from_slice(&bytes).unwrap());
    let mut clock = 1_000_000_000u64;
    tick(&mut b, &mut clock);
    assert_eq!(b.saved(), saved, "the same pages and draft");
    assert_eq!(b.entry.note, "Bakery");
    assert!(b.pop());
    assert_eq!(b.route(), &Route::Entries);
    assert!(b.pop(), "Back from Entries goes to Overview");
    assert_eq!(b.route(), &Route::Overview);

    // Stacks no page can build are still not opened.
    for nav in [
        vec![Route::Overview, Route::Budgets, Route::Entries],
        vec![Route::Entries, Route::Insights],
        vec![Route::Overview, Route::Overview],
    ] {
        let e = dir("see-all-nonsense");
        let mut c = app(&e);
        c.store.load_sample(tally::model::today()).unwrap();
        c.invalidate();
        c.push(Route::Overview);
        c.restore(&Saved { nav, drawer_depth: None, picking: None, entry: None, ..Default::default() });
        assert_eq!(c.saved().nav, vec![Route::Overview]);
    }
}

/// Picks are numbered process-wide: the tests that pick take turns.
static PICKS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn picks() -> std::sync::MutexGuard<'static, ()> {
    PICKS.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn a_backup_picked_as_tally_restarted_waits_on_the_question() {
    let _turn = picks();
    let d = dir("backup-src");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    let file = a.store.ledger.export_backup("2026-11-19").unwrap();
    let bytes = relay_money::backup::encode(&file).into_bytes();
    drop(a);

    // A new process, its ledger empty, with no page waiting for the file: as android.rs's
    // `adopt_pick` does, Backup opens and takes it.
    let e = dir("backup-dst");
    let mut b = app(&e);
    b.store.set_pref("onboarded", "1").unwrap();
    b.push(Route::Overview);
    b.push(Route::Backup);
    tally::settings::open_backup(&mut b, "tally-backup.json".into(), bytes);
    let mut clock = 1_000_000_000u64;
    for _ in 0..200 {
        tick(&mut b, &mut clock);
        if tally::settings::restore_waiting(&b) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(tally::settings::restore_waiting(&b), "read, and asking \"Restore this backup?\"");
    assert_eq!(b.route(), &Route::Backup);
    assert!(b.store.is_empty(), "nothing restored without a yes");
    // Back says no: still nothing restored.
    assert!(b.back());
    assert!(!tally::settings::restore_waiting(&b));
    tick(&mut b, &mut clock);
    assert!(b.store.is_empty());
}

#[test]
fn a_file_that_comes_after_its_page_is_gone_is_not_the_next_picks() {
    use tally::settings_files::{begin_pick, deliver, take_delivered, take_picked};
    let _turn = picks();
    let d = dir("stale-pick");
    let mut a = app(&d);
    // Import asks for a statement; its page is gone when the file comes, and the platform's loop
    // moves it into the app as it does every file.
    begin_pick();
    deliver("statement.csv".into(), b"Date,Amount\n".to_vec());
    let (pick, name, bytes) = take_delivered().expect("delivered");
    a.file_delivered(pick, name, bytes);
    // Backup's Restore asks next: the statement is not its backup.
    begin_pick();
    assert!(take_picked(&mut a).is_none(), "the earlier pick's file is dropped");
    assert!(a.picked.is_none());
    // What comes for this pick is taken.
    deliver("tally-backup.json".into(), b"{}".to_vec());
    assert_eq!(take_picked(&mut a).map(|(n, _)| n).as_deref(), Some("tally-backup.json"));
    // A file in hand (one a stopped process asked for, as android.rs's `adopt_pick` hands on)
    // answers the pick out now.
    a.file_picked("statement.csv".into(), Vec::new());
    assert!(take_picked(&mut a).is_some());
    // A new pick also empties a delivery no page has looked at yet.
    deliver("late.csv".into(), Vec::new());
    begin_pick();
    assert!(take_delivered().is_none());
}

#[test]
fn the_server_address_gets_a_keyboard_for_addresses() {
    use std::sync::Mutex;
    use tally::widgets::{Focus, Kb};

    // A host that only says what it was asked for: the old flag still means words or digits.
    struct Keys(Mutex<Vec<Kb>>);
    impl tally::app::Host for Keys {
        fn data_dir(&self) -> PathBuf {
            PathBuf::new()
        }
        fn log(&self, _: &str) {}
        fn keyboard_kind(&self, _: bool, _: &str, kind: Kb) {
            self.0.lock().unwrap().push(kind);
        }
    }
    let keys = Keys(Mutex::new(Vec::new()));
    tally::app::Host::keyboard(&keys, true, "", true);
    tally::app::Host::keyboard(&keys, true, "", false);
    assert_eq!(*keys.0.lock().unwrap(), vec![Kb::Number, Kb::Text]);

    // The kind belongs to the field that asked for it, never to the next one focused.
    let mut f = Focus { id: Some("sync.server"), kind: Some(("sync.server", Kb::Url)), ..Focus::default() };
    assert_eq!(f.kb(), Kb::Url);
    f.id = Some("plan.amount");
    f.numeric = true;
    assert_eq!(f.kb(), Kb::Number);
    f.id = Some("account.name");
    f.numeric = false;
    assert_eq!(f.kb(), Kb::Text);
}

#[test]
fn the_launcher_icon_follows_only_an_explicit_light_or_dark() {
    let d = dir("icon");
    let mut a = app(&d);
    assert_eq!(a.icon_dark(), None, "System, the default: the icon is left alone");
    a.set_system_dark(true);
    assert_eq!(a.icon_dark(), None, "the phone going dark does not move it");
    a.set_system_dark(false);
    assert_eq!(a.icon_dark(), None);
    a.set_appearance("dark");
    assert_eq!(a.icon_dark(), Some(true));
    a.set_appearance("light");
    assert_eq!(a.icon_dark(), Some(false));
    a.set_appearance("system");
    assert_eq!(a.icon_dark(), None);
}

#[test]
fn a_toast_on_import_leaves_the_footer_keys_free() {
    let _turn = picks();
    let d = dir("import-toast");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Import);
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    let csv = "Date,Description,Amount,Balance\n2026-11-02,METRO PLUS #4411 MONTREAL QC,-45.20,954.80\n2026-11-04,STARBUCKS 0231,-6.75,2948.05\n";
    tally::import::open_file(&mut a, "statement.csv".into(), csv.as_bytes().to_vec());
    for _ in 0..200 {
        tick(&mut a, &mut clock);
        if tally::import::stage(&a) == "preview" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(tally::import::stage(&a), "preview");
    a.say("Something to say", None);
    tick(&mut a, &mut clock);
    // "Another file", the footer's left key, in its middle: where the toast used to sit.
    let key_mid = a.canvas.h - a.canvas.inset_bottom - tally::settings_kit::FOOTER_KEYS - 28.0 + 14.0 + 26.0;
    a.ui.tap = Some((tally::kit::PAD + 30.0, key_mid));
    tick(&mut a, &mut clock);
    tick(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "choose", "the key took the tap, not the toast");
    assert!(a.toast.is_some(), "the toast still shows, above the footer");
}

#[test]
fn a_file_or_a_cancel_for_an_earlier_pick_is_not_the_latest_picks() {
    use tally::settings_files::{PICK_TAG_MASK, begin_pick, cancel, deliver_for, pick_from_tag, take_cancelled, take_picked};
    let _turn = picks();
    let d = dir("tagged-pick");
    let mut a = app(&d);
    // Restore asks; before its slow read (a cloud drive) comes back, Import asks too.
    let restore = begin_pick();
    let import = begin_pick();
    // The platform hands back only the number's low bits: they name the pick they answer.
    assert_eq!(pick_from_tag(import & PICK_TAG_MASK), Some(import));
    assert_eq!(pick_from_tag(restore & PICK_TAG_MASK), None, "an earlier pick's answer");
    // Restore's backup arrives: it is not Import's statement.
    deliver_for(restore, "tally-backup.json".into(), b"{}".to_vec());
    assert!(take_picked(&mut a).is_none());
    deliver_for(import, "statement.csv".into(), b"Date,Amount\n".to_vec());
    assert_eq!(take_picked(&mut a).map(|(n, _)| n).as_deref(), Some("statement.csv"));

    // A cancel is the page's to take, once, and only for the pick out now.
    let next = begin_pick();
    cancel(next - 1);
    assert!(!take_cancelled(), "an earlier pick's cancel");
    cancel(next);
    assert!(take_cancelled());
    assert!(!take_cancelled(), "taken once");
    // A new pick forgets a cancel no page took.
    cancel(begin_pick());
    begin_pick();
    assert!(!take_cancelled());
}

#[test]
fn pages_kept_while_the_ledger_is_in_the_way_open_once_it_opens() {
    let d = dir("kept-blocked");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Entry { id: None, kind: None, account: None, category: None });
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    a.entry.note = "Kept".into();
    tick(&mut a, &mut clock);
    let saved = a.saved();
    assert!(saved.entry.is_some());
    drop(a);

    // The process comes back with the ledger in the way (a folder in its place for now).
    let db = d.0.join("tally.db");
    let aside = d.0.join("aside");
    std::fs::create_dir_all(&aside).unwrap();
    for ext in ["", "-wal", "-shm"] {
        let from = d.0.join(format!("tally.db{ext}"));
        if from.exists() {
            std::fs::rename(&from, aside.join(format!("tally.db{ext}"))).unwrap();
        }
    }
    std::fs::create_dir_all(&db).unwrap();
    let mut b = app(&d);
    assert!(b.ledger_error.is_some());
    b.restore(&saved);
    assert_eq!(b.saved(), saved, "kept again as it was, not the stand-in's pages");

    // The ledger is back: the pages and the draft open.
    std::fs::remove_dir(&db).unwrap();
    for ext in ["", "-wal", "-shm"] {
        let from = aside.join(format!("tally.db{ext}"));
        if from.exists() {
            std::fs::rename(&from, d.0.join(format!("tally.db{ext}"))).unwrap();
        }
    }
    assert!(b.retry_ledger());
    assert!(matches!(b.route(), Route::Entry { .. }));
    assert_eq!(b.entry.note, "Kept");
    assert_eq!(b.saved(), saved);
}

#[test]
fn a_bill_being_typed_outlasts_the_process() {
    let d = dir("bill-draft");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Bills);
    a.push(Route::BillEdit { id: None });
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    // The name typed and Done: it lives only in the editor's draft now.
    a.focus.id = Some("plan.bill.name");
    a.focus.text = "Rent for the flat".into();
    tick(&mut a, &mut clock);
    a.finish_typing();
    tick(&mut a, &mut clock);
    let saved = a.saved();
    assert!(saved.typing.is_none());
    assert_eq!(saved.drafts.len(), 1, "the bill editor's draft is kept");
    let bytes = serde_json::to_vec(&saved).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("Rent for the flat"));
    drop(a);

    let mut b = app(&d);
    b.restore(&serde_json::from_slice(&bytes).unwrap());
    // The editor opens afresh on the restored route, and its draft goes over what it read.
    let mut clock = 1_000_000_000u64;
    for _ in 0..3 {
        tick(&mut b, &mut clock);
    }
    assert_eq!(b.route(), &Route::BillEdit { id: None });
    assert_eq!(b.saved(), saved, "the same draft, kept again");

    // Gone from its page, the draft is never put into a later editor.
    assert!(b.pop());
    b.push(Route::BillEdit { id: None });
    tick(&mut b, &mut clock);
    let again = serde_json::to_string(&b.saved()).unwrap();
    assert!(!again.contains("Rent for the flat"), "a new bill starts blank");
}

#[test]
fn a_field_half_typed_outlasts_the_process() {
    let d = dir("typing");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Accounts);
    a.push(Route::AccountEdit { id: None, kind: None });
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    a.focus.id = Some("account.name");
    a.focus.text = "Cash bo".into();
    tick(&mut a, &mut clock);
    let saved = a.saved();
    assert_eq!(saved.typing.as_ref().map(|t| t.text.as_str()), Some("Cash bo"));
    let bytes = serde_json::to_vec(&saved).unwrap();
    drop(a);

    let mut b = app(&d);
    b.restore(&serde_json::from_slice(&bytes).unwrap());
    let mut clock = 1_000_000_000u64;
    tick(&mut b, &mut clock);
    assert!(b.focus.is("account.name"), "the field types again");
    // Done, then the text is the draft's.
    b.finish_typing();
    tick(&mut b, &mut clock);
    let kept = serde_json::to_string(&b.saved()).unwrap();
    assert!(kept.contains("Cash bo"), "{kept}");
}

#[test]
fn a_pots_money_card_being_typed_outlasts_the_process() {
    use tally::app::{Draft, PageDraft, SavedTyping};
    use tally::plan::PlanDraft;
    let d = dir("goal-draft");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    let goals = tally::plan_store::goals(a.store.sql().unwrap()).unwrap();
    let pot = goals.iter().find(|g| g.kind == relay_money::model::GoalKind::Savings).expect("a pot in the sample").id;
    drop(a);

    // Typing the amount, and typing the note: each comes back as it was, frame after frame.
    for (field, text) in [("plan.goal.amount", "50"), ("plan.goal.note", "gift")] {
        let mut b = app(&d);
        let route = Route::Goal { id: pot };
        let saved = Saved {
            nav: vec![Route::Overview, route.clone()],
            drawer_depth: None,
            picking: None,
            entry: None,
            drafts: vec![PageDraft { depth: 1, route, draft: Draft::Plan(PlanDraft::GoalPage { amount: "50".into(), note: "gift".into(), adding: true }) }],
            typing: Some(SavedTyping { id: field.into(), text: text.into(), numeric: field.ends_with("amount") }),
        };
        b.restore(&saved);
        let mut clock = 1_000_000_000u64;
        for _ in 0..4 {
            tick(&mut b, &mut clock);
        }
        assert!(b.focus.is(field), "the field types again");
        assert_eq!(b.focus.text, text);
        let kept = b.saved();
        assert_eq!(kept.drafts.len(), 1);
        assert_eq!(kept.drafts[0].draft, Draft::Plan(PlanDraft::GoalPage { amount: "50".into(), note: "gift".into(), adding: true }), "{field}");
    }

    // A draft for a goal gone: the card closes and nothing is left typing.
    let mut b = app(&d);
    let gone = Route::Goal { id: 999_999 };
    let saved = Saved {
        nav: vec![Route::Overview, gone.clone()],
        drawer_depth: None,
        picking: None,
        entry: None,
        drafts: vec![PageDraft { depth: 1, route: gone, draft: Draft::Plan(PlanDraft::GoalPage { amount: "50".into(), note: String::new(), adding: true }) }],
        typing: Some(SavedTyping { id: "plan.goal.amount".into(), text: "50".into(), numeric: true }),
    };
    b.restore(&saved);
    let mut clock = 1_000_000_000u64;
    tick(&mut b, &mut clock);
    tick(&mut b, &mut clock);
    assert!(b.focus.id.is_none(), "no field types on a goal that is gone");
}

#[test]
fn a_contribution_room_being_typed_outlasts_the_process() {
    use tally::app::{Draft, PageDraft};
    let d = dir("room-draft");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    drop(a);

    // What a stopped process kept: the TFSA room's figure half typed on its keypad.
    let room = Route::RoomEdit { kind: "TFSA".into() };
    let saved = Saved {
        nav: vec![Route::Investments, room.clone()],
        drafts: vec![PageDraft { depth: 1, route: room.clone(), draft: Draft::Room { input: "7000".into() } }],
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&saved).unwrap();
    let mut b = app(&d);
    b.restore(&serde_json::from_slice(&bytes).unwrap());
    let mut clock = 1_000_000_000u64;
    for _ in 0..3 {
        tick(&mut b, &mut clock);
    }
    assert_eq!(b.route(), &room);
    // Kept again from the editor itself (the restored draft was taken as it opened).
    assert_eq!(b.saved(), saved, "the editor took the figure, and keeps it");

    // Gone from its page, the figure is not kept, nor put into the next room editor.
    assert!(b.pop());
    assert!(b.saved().drafts.is_empty());
    b.push(room.clone());
    tick(&mut b, &mut clock);
    assert!(!serde_json::to_string(&b.saved()).unwrap().contains("7000"), "a room opened again starts from what is stored");
}

#[test]
fn a_toast_on_the_welcome_leaves_start_fresh_free() {
    let d = dir("welcome-toast");
    let mut a = app(&d);
    assert_eq!(a.route(), &Route::Onboarding);
    let mut clock = 1_000_000_000u64;
    tick(&mut a, &mut clock);
    a.say("The sample could not be made", None);
    tick(&mut a, &mut clock);
    // "Start fresh on this phone", the last thing at the foot, in its middle.
    let foot = a.canvas.h - a.canvas.inset_bottom - 20.0;
    a.ui.tap = Some((tally::draw::WIDTH / 2.0, foot - 20.0));
    tick(&mut a, &mut clock);
    tick(&mut a, &mut clock);
    assert_eq!(a.route(), &Route::Overview, "the key took the tap, not the toast");
}

/// A data folder paired with a server nobody can reach, sync off, not onboarded: where Erase
/// everything leaves a paired phone whose sync was off.
fn paired_with_sync_off(d: &Dir) {
    std::fs::create_dir_all(&d.0).unwrap();
    std::fs::write(d.0.join("sync-secret.json"), r#"{"creds":{"host":"h","routes":["wss://127.0.0.1:1/x"],"device":"d","token":"t","paired_at":0},"pending":null}"#).unwrap();
    std::fs::write(d.0.join("prefs.json"), r#"{"sync.phone_only":"1"}"#).unwrap();
}

/// The welcome scrolled to its foot. Returns where its last thing ends on screen.
fn welcome_foot(a: &mut App, clock: &mut u64) -> f32 {
    let max = a.nav.last().unwrap().1.max;
    a.nav.last_mut().unwrap().1.off = max.max(0.0);
    tick(a, clock);
    a.canvas.h - a.canvas.inset_bottom - 24.0 + max.min(0.0)
}

#[test]
fn a_paired_phone_erased_with_sync_off_is_not_stuck_on_the_welcome() {
    // Its keys: "Turn sync on and bring the ledger over", then "Start on this phone" at the foot.
    for start_here in [false, true] {
        let d = dir(if start_here { "erased-off-start" } else { "erased-off-on" });
        paired_with_sync_off(&d);
        let mut a = app(&d);
        assert_eq!(a.route(), &Route::Onboarding);
        let mut clock = 1_000_000_000u64;
        for _ in 0..3 {
            tick(&mut a, &mut clock);
        }
        assert!(a.st.sync.paired(&a.store), "paired");
        let foot = welcome_foot(&mut a, &mut clock);
        let y = if start_here { foot - 20.0 } else { foot - 40.0 - 10.0 - 28.0 };
        a.ui.tap = Some((tally::draw::WIDTH / 2.0, y));
        tick(&mut a, &mut clock);
        tick(&mut a, &mut clock);
        if start_here {
            assert_eq!(a.route(), &Route::Overview, "it starts on this phone");
            assert!(a.store.pref("sync.phone_only").is_some(), "with sync still off");
        } else {
            assert!(a.store.pref("sync.phone_only").is_none(), "sync is on, to bring the ledger over");
            assert_eq!(a.route(), &Route::Onboarding, "waiting for the ledger");
        }
    }
}

#[test]
fn a_field_the_keyboard_rises_over_is_shown_without_waiting_for_a_key() {
    let d = dir("keyboard-rises");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.push(Route::Overview);
    a.push(Route::Accounts);
    a.push(Route::AccountEdit { id: None, kind: None });
    let mut clock = 1_000_000_000u64;
    for _ in 0..5 {
        tick(&mut a, &mut clock);
    }
    a.focus.id = Some("account.opening");
    a.focus.numeric = true;
    tick(&mut a, &mut clock);
    let before = a.scroll().off;
    // The keyboard comes up over the bottom half (the platform lays the app out above it).
    a.resize(1080.0, 2340.0, 90.0, 1200.0);
    a.keyboard_up = true;
    clock += 16_666_666;
    let next = a.frame(clock);
    let raised = a.scroll().off;
    assert!(raised > before, "the page scrolls the field up: {before} -> {raised}");
    assert!(matches!(next, tally::app::Next::Now) || matches!(next, tally::app::Next::At(at) if at <= clock), "drawn again at once: {next:?}");
    // The next frame draws at the new offset, with the field above the keyboard, and then rests.
    clock += 16_666_666;
    let next = a.frame(clock);
    assert_eq!(a.scroll().off, raised, "it stays put");
    let (y, h) = a.focus.at.expect("the field drew");
    assert!(y + h - raised <= a.canvas.h - a.canvas.inset_bottom, "the field is above the keyboard");
    // Pictures decoding for other tests in this process may still ask for frames, so the rest is
    // read from the offset: further frames don't move it again.
    let _ = next;
    clock += 16_666_666;
    a.frame(clock);
    assert_eq!(a.scroll().off, raised, "nothing more to scroll");
}

#[test]
fn a_tap_on_the_bars_under_the_month_switcher_only_closes_it() {
    let d = dir("months-bars");
    let mut a = app(&d);
    a.store.load_sample(tally::model::today()).unwrap();
    a.store.set_pref("onboarded", "1").unwrap();
    a.invalidate();
    a.push(Route::Entries);
    let mut clock = 1_000_000_000u64;
    for _ in 0..3 {
        tick(&mut a, &mut clock);
    }
    // The month's title opens the switcher.
    let top = a.canvas.inset_top + tally::app::TOP_ROW + 6.0;
    a.ui.tap = Some((tally::kit::PAD + 20.0, top + 2.0 + 16.0));
    tick(&mut a, &mut clock);
    tick(&mut a, &mut clock);
    // Home, on the tab bar.
    let slot = (tally::draw::WIDTH - 24.0) / 5.0;
    a.ui.tap = Some((12.0 + slot / 2.0, a.canvas.h - a.bar_h() + 30.0));
    tick(&mut a, &mut clock);
    tick(&mut a, &mut clock);
    assert_eq!(a.route(), &Route::Entries, "the tap went to the switcher, not the tab bar");
    assert!(!tally::activity::back(&mut a), "and closed it");
}

#[test]
fn a_currency_with_other_decimals_is_refused_once_a_budget_is_kept() {
    let d = dir("currency-budget");
    let mut a = app(&d);
    // No accounts, no entries: only a budget.
    a.store.ledger.budget_set(None, 500_00).unwrap();
    tally::settings::change_currency(&mut a, "JPY");
    assert_eq!(a.store.ledger.settings().unwrap().currency, "CAD", "refused");
    assert_eq!(a.store.ledger.budget(None).unwrap(), Some(500_00), "the budget reads as it did");
    // Same decimals is fine.
    tally::settings::change_currency(&mut a, "USD");
    assert_eq!(a.store.ledger.settings().unwrap().currency, "USD");
}

#[test]
fn pick_numbers_stay_within_the_tag_and_never_restart_at_one() {
    use tally::settings_files::{PICK_TAG_MASK, next_pick};
    // A process's first pick comes from its seed, so a stopped process's slow file (numbered
    // from that process's own start) is unlikely to carry the same tag.
    assert_eq!(next_pick(0, 0), 1);
    assert_eq!(next_pick(0, 41), 42);
    assert_eq!(next_pick(0, u64::MAX), u64::MAX % PICK_TAG_MASK + 1);
    // Then they count up, and wrap past the mask to 1: never 0, "nothing asked yet".
    assert_eq!(next_pick(7, 99), 8);
    assert_eq!(next_pick(PICK_TAG_MASK, 99), 1);
    for seed in [0, 1, PICK_TAG_MASK - 1, PICK_TAG_MASK, PICK_TAG_MASK + 1, u64::MAX] {
        let n = next_pick(0, seed);
        assert!((1..=PICK_TAG_MASK).contains(&n), "{n}");
    }
}

#[test]
fn each_share_has_a_folder_of_its_own_and_old_ones_go() {
    use tally::settings_files::write_share;
    let d = dir("share");
    let shared = d.0.join("shared");
    // A file an older build left loose in the folder.
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("tally-backup-2026-11-19.json"), b"old").unwrap();
    let first = write_share(&shared, "tally-backup-2026-11-20.json", b"one").unwrap();
    assert!(!shared.join("tally-backup-2026-11-19.json").exists(), "the loose file went");
    // The same name again the same day: the first share's file is left as it was.
    let second = write_share(&shared, "tally-backup-2026-11-20.json", b"two").unwrap();
    assert_ne!(first, second);
    assert_eq!(std::fs::read(&first).unwrap(), b"one", "still readable by the app it went to");
    assert_eq!(std::fs::read(&second).unwrap(), b"two");
    assert!(first.starts_with(&shared) && second.starts_with(&shared));
    // A third share deletes all but the latest before it.
    let third = write_share(&shared, "export.csv", b"three").unwrap();
    assert!(!first.exists(), "the oldest share's folder went");
    assert!(second.exists() && third.exists());
    assert_eq!(std::fs::read_dir(&shared).unwrap().count(), 2);
    // A name that would climb out stays inside.
    let odd = write_share(&shared, "..", b"x").unwrap();
    assert!(odd.starts_with(&shared) && odd.file_name().unwrap() == "tally-file");
}
