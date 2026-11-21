//! Where a connected bank's accounts and an imported statement are guessed to land, and the
//! account editor and Search as the app drives them: a minus typed into an opening balance, an
//! editor opened twice for one account, and Search opened anew; an import after another, an editor
//! under another editor (and kept when the app is stopped), an editor for something gone, a subject
//! page's search, and a statement from a bank whose last one went into an account renamed since,
//! an older statement imported after a later one (and one stopped before its opening moved), a
//! card's file remembered and placed (and kept apart from its bank's chequing), Tally's CSV in
//! another currency refused, and a failed import's words gone once a retry succeeds.

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::bank_statements::{self, StatementRead};
use relay_money::model::AccountType;
use relay_money::views::AccountView;
use tally::activity_data::Filter;
use tally::app::App;
use tally::banks_logic::{self as banks, Bank, BankAccount, Mine};
use tally::import_logic::{self as import, Source};
use tally::nav::Route;

fn mine(uid: &str, name: &str, ty: AccountType) -> Mine {
    Mine { id: uid.len() as i64, uid: uid.into(), name: name.into(), r#type: ty, archived: false, institution: String::new(), external_ref: String::new() }
}

fn bank_account(id: &str, kind: &str) -> BankAccount {
    BankAccount { id: id.into(), name: kind.into(), mask: String::new(), kind: kind.into(), follow: false, into: None }
}

fn bank(institution: &str, accounts: Vec<BankAccount>) -> Bank {
    Bank { id: "b1".into(), institution: institution.into(), accounts, last_sync: None, problem: None }
}

#[test]
fn a_card_is_never_suggested_into_a_chequing_account() {
    // The bank's only account of Tally's is chequing: the card gets a new account of its own.
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("Desjardins", vec![bank_account("a1", "credit card")]), &held);
    assert_eq!(s.accounts[0].into, None);
    // Nor an investment into chequing, nor a chequing account into a card.
    let held = vec![mine("u-visa", "Desjardins Visa", AccountType::Credit)];
    let s = banks::suggested(&bank("Desjardins", vec![bank_account("a1", "checking"), bank_account("a2", "tfsa")]), &held);
    assert_eq!((s.accounts[0].into.as_deref(), s.accounts[1].into.as_deref()), (None, None));
    // A kind Tally can't place (a loan) is never guessed by name.
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("Desjardins", vec![bank_account("a1", "loan")]), &held);
    assert_eq!(s.accounts[0].into, None);
}

#[test]
fn two_bank_accounts_never_land_in_the_same_tally_account() {
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("Desjardins", vec![bank_account("a1", "checking"), bank_account("a2", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-cheq"));
    assert_eq!(s.accounts[1].into, None, "the first took it");
    // One the server already placed is taken too.
    let mut b = bank("Desjardins", vec![bank_account("a1", "checking"), bank_account("a2", "checking")]);
    b.accounts[1].into = Some("u-cheq".into());
    let s = banks::suggested(&b, &held);
    assert_eq!((s.accounts[0].into.as_deref(), s.accounts[1].into.as_deref()), (None, Some("u-cheq")));
}

#[test]
fn savings_goes_to_savings_and_the_bank_is_read_as_a_whole_word() {
    let held = vec![mine("u-cheq", "TD chequing", AccountType::Chequing), mine("u-save", "TD savings", AccountType::Savings), mine("u-out", "Outdoor fund", AccountType::Savings)];
    let s = banks::suggested(&bank("TD Canada Trust", vec![bank_account("a1", "savings"), bank_account("a2", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-save"));
    assert_eq!(s.accounts[1].into.as_deref(), Some("u-cheq"));
    // "td" is in "Outdoor", but Outdoor fund is not at TD.
    let held = vec![mine("u-out", "Outdoor fund", AccountType::Savings)];
    let s = banks::suggested(&bank("TD", vec![bank_account("a1", "savings")]), &held);
    assert_eq!(s.accounts[0].into, None);
    // The number still matches, as long as the type agrees.
    let mut card = mine("u-visa", "Visa", AccountType::Credit);
    card.external_ref = "xxxx0007".into();
    let mut cheq = mine("u-cheq", "Chequing", AccountType::Chequing);
    cheq.external_ref = "xxxx4411".into();
    let mut a = bank_account("a1", "credit card");
    a.mask = "4411".into();
    assert_eq!(banks::match_account("TD", &a, &[cheq.clone(), card.clone()], &[]), None, "a card's number on a chequing account is not it");
    a.mask = "0007".into();
    assert_eq!(banks::match_account("TD", &a, &[cheq, card], &[]).map(|m| m.uid.as_str()), Some("u-visa"));
}

fn view(id: i64, name: &str, ty: AccountType) -> AccountView {
    AccountView { id, name: name.into(), r#type: ty, balance: 0, archived: false, registration: None, institution: String::new() }
}

fn statement(text: &str) -> relay_money::bank_statements::Statement {
    let StatementRead::Ok(s) = bank_statements::read(text, 2, false) else { panic!("reads") };
    s
}

#[test]
fn a_card_statement_goes_into_the_card_not_chequing() {
    // Purchases written positive, one payment negative: a card's statement.
    let card = statement("Date,Description,Amount\n2026-11-02,METRO,45.20\n2026-11-03,STARBUCKS,6.75\n2026-11-04,AMAZON,30.00\n2026-11-05,PAYMENT,-100.00\n");
    let cheq_like = statement("Date,Description,Amount\n2026-11-02,METRO,-45.20\n2026-11-03,STARBUCKS,-6.75\n2026-11-04,PAY,1200.00\n");
    assert!(card.positive_share() > import::CARD_POSITIVE_SHARE);
    let both = vec![view(1, "Desjardins chequing", AccountType::Chequing), view(2, "Desjardins Visa", AccountType::Credit)];
    let d = Some(Source::Desjardins);
    assert_eq!(import::guess_target(&both, d, card.positive_share()), Some(2));
    assert_eq!(import::guess_target(&both, d, cheq_like.positive_share()), Some(1));
    // Only chequing at the bank: a card's statement is not guessed into it (every purchase would
    // read as money in).
    let cheq = vec![view(1, "Desjardins chequing", AccountType::Chequing)];
    assert_eq!(import::guess_target(&cheq, d, card.positive_share()), None);
    assert_eq!(import::guess_target(&cheq, d, cheq_like.positive_share()), Some(1));
    // A card statement written with purchases below zero still finds the one card.
    let visa = vec![view(2, "Desjardins Visa", AccountType::Credit)];
    assert_eq!(import::guess_target(&visa, d, cheq_like.positive_share()), Some(2));
}

#[test]
fn the_last_import_is_remembered_by_how_its_file_reads() {
    let both = vec![view(1, "Chequing", AccountType::Chequing), view(2, "Visa", AccountType::Credit)];
    // A file that reads like chequing may have gone into a card (one writing purchases below zero).
    assert_eq!(import::remembered_target(&both, Some(2), false), Some(2));
    assert_eq!(import::remembered_target(&both, Some(1), false), Some(1));
    // A card's file (purchases positive) only into a card.
    assert_eq!(import::remembered_target(&both, Some(1), true), None);
    assert_eq!(import::remembered_target(&both, Some(2), true), Some(2));
}

#[test]
fn a_card_that_writes_purchases_below_zero_gets_its_next_file_too() {
    let (mut a, _d) = app("import-negative-card");
    let mut clock = 1_000_000_000;
    let card = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.r#type == AccountType::Credit && !x.archived).expect("a card in the sample").id;
    // Purchases below zero: it reads like chequing, and the person puts it into the card.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-02,METRO,-45.20\n2026-11-03,STARBUCKS,-6.75\n2026-11-04,PAYMENT,100.00\n");
    tally::import::pick_into(&mut a, Some(card));
    tick(&mut a, &mut clock);
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    tap_footer(&mut a, &mut clock);
    // The next one from it goes there again, not into a new chequing account counting it twice.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-09,IGA,-12.40\n2026-11-10,TIM HORTONS,-3.10\n2026-11-11,PAYMENT,50.00\n");
    assert_eq!(tally::import::into(&a), Some(card));
}

#[test]
fn a_new_account_picked_for_a_cards_file_is_a_card() {
    let (mut a, _d) = app("import-new-card");
    let mut clock = 1_000_000_000;
    import_file(&mut a, &mut clock, "Date,Description,Amount\n2026-11-02,METRO,45.20\n2026-11-03,STARBUCKS,6.75\n2026-11-04,AMAZON,30.00\n2026-11-05,PAYMENT,-100.00\n");
    let first = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "Imported card").expect("the card").id;
    // The next card's file finds that card; "A new account" picked instead is a card as well, so
    // its purchases are spending, not money in.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-09,IGA,12.40\n2026-11-10,TIM HORTONS,3.10\n2026-11-11,SAQ,25.00\n");
    assert_eq!(tally::import::into(&a), Some(first));
    tally::import::pick_into(&mut a, None);
    tick(&mut a, &mut clock);
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    let made = tally::import::into(&a).expect("made");
    assert_ne!(made, first);
    let made = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.id == made).unwrap();
    assert_eq!(made.r#type, AccountType::Credit);
    assert!(made.balance < 0, "owed, not money in: {}", made.balance);
}

#[test]
fn an_investment_or_two_alike_accounts_are_never_guessed() {
    let d = Some(Source::Desjardins);
    let invest = vec![view(1, "Desjardins TFSA", AccountType::Investment), view(2, "Desjardins chequing", AccountType::Chequing)];
    assert_eq!(import::guess_target(&invest, d, 0.2), Some(2));
    let two = vec![view(1, "Desjardins chequing", AccountType::Chequing), view(2, "Desjardins savings", AccountType::Savings)];
    assert_eq!(import::guess_target(&two, d, 0.2), None, "which one is the owner's to say");
    let cards = vec![view(1, "Desjardins Visa", AccountType::Credit), view(2, "Desjardins Mastercard", AccountType::Credit)];
    assert_eq!(import::guess_target(&cards, d, 0.9), None);
}

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
    let d = std::env::temp_dir().join(format!("tally-bank-import-guesses-{}-{name}", std::process::id()));
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
fn a_minus_in_the_opening_balance_opens_the_account_below_zero() {
    let (mut a, _d) = app("minus");
    let mut clock = 1_000_000_000;
    a.push(Route::AccountEdit { id: None, kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "Overdrawn");
    type_into(&mut a, &mut clock, "account.opening", "-50");
    tap_save(&mut a, &mut clock);
    let saved = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "Overdrawn").expect("saved");
    let row = tally::accounts_store::stored(&a.store, saved.id).unwrap().unwrap();
    assert_eq!(row.row.opening, -5_000, "not +50");
}

#[test]
fn an_account_edited_from_its_own_entries_goes_back_to_the_editor_open() {
    let (mut a, _d) = app("twice");
    let mut clock = 1_000_000_000;
    let id = a.store.ledger.accounts().unwrap().into_iter().find(|x| !x.archived).unwrap().id;
    a.push(Route::AccountEdit { id: Some(id), kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "Renamed, not saved");
    // "See its entries", then that page's edit key.
    a.push(Route::Transactions { category: None, account: Some(id) });
    tick(&mut a, &mut clock);
    a.push(Route::AccountEdit { id: Some(id), kind: None });
    tick(&mut a, &mut clock);
    let editors = a.nav.iter().filter(|(r, _)| *r == Route::AccountEdit { id: Some(id), kind: None }).count();
    assert_eq!(editors, 1, "one editor, not two");
    assert_eq!(a.route(), &Route::AccountEdit { id: Some(id), kind: None });
    tap_save(&mut a, &mut clock);
    let name = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.id == id).unwrap().name;
    assert_eq!(name, "Renamed, not saved", "the draft stayed");
}

#[test]
fn search_opened_anew_starts_on_every_kind() {
    let (mut a, _d) = app("search");
    let mut clock = 1_000_000_000;
    a.push(Route::Search);
    tick(&mut a, &mut clock);
    a.st.activity.search_filter = Filter::Spent;
    // Back from a result the chip stays.
    let id = a.store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions[0].id;
    a.push(Route::EntryView { id });
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.search_filter, Filter::Spent);
    // Opened again from the tab it starts on every kind, as the query starts empty.
    a.push(Route::Overview);
    tick(&mut a, &mut clock);
    a.push(Route::Search);
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.search_filter, Filter::All);
}

/// Reads `csv` on the Import page showing until its preview is up.
fn import_preview(a: &mut App, clock: &mut u64, csv: &str) {
    tally::import::open_file(a, "statement.csv".into(), csv.as_bytes().to_vec());
    for _ in 0..400 {
        tick(a, clock);
        if tally::import::stage(a) == "preview" {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("no preview: {}", tally::import::stage(a));
}

/// The footer's right-hand key (Import), or the whole-width one (Done).
fn tap_footer(a: &mut App, clock: &mut u64) {
    let top = a.canvas.h - a.canvas.inset_bottom - 52.0 - 28.0;
    // A toast over the footer would take the tap.
    a.toast = None;
    a.ui.tap = Some((300.0, top + 14.0 + 26.0));
    tick(a, clock);
}

#[test]
fn a_new_import_starts_from_its_own_defaults_not_the_last_ones() {
    let (mut a, _d) = app("import-defaults");
    let mut clock = 1_000_000_000;
    // A card's statement with nowhere to go: it makes a new card.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-02,METRO,45.20\n2026-11-03,STARBUCKS,6.75\n2026-11-04,AMAZON,30.00\n2026-11-05,PAYMENT,-100.00\n");
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    let card = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "Imported card").expect("the card");
    assert_eq!(card.r#type, AccountType::Credit);
    tap_footer(&mut a, &mut clock);
    // Then a chequing statement: a new chequing account of its own name, not a second card.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-06,IGA,-12.40\n2026-11-07,TIM HORTONS,-3.10\n2026-11-08,PAY,1500.00\n");
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    let new = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "Imported account").expect("a chequing account");
    assert_eq!(new.r#type, AccountType::Chequing);
}

#[test]
fn an_editor_draft_outlasts_another_editor_opened_over_it() {
    let (mut a, _d) = app("drafts");
    let mut clock = 1_000_000_000;
    let id = a.store.ledger.accounts().unwrap().into_iter().find(|x| !x.archived).unwrap().id;
    a.push(Route::AccountEdit { id: Some(id), kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "Renamed, kept");
    // Its entries, then a new account from there, and Back twice.
    a.push(Route::Transactions { category: None, account: Some(id) });
    tick(&mut a, &mut clock);
    a.push(Route::AccountEdit { id: None, kind: None });
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    assert_eq!(a.route(), &Route::AccountEdit { id: Some(id), kind: None });
    tap_save(&mut a, &mut clock);
    let name = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.id == id).unwrap().name;
    assert_eq!(name, "Renamed, kept");

    // The same for a category.
    let c = a.store.ledger.categories().unwrap().into_iter().find(|c| !c.archived).unwrap();
    a.push(Route::CategoryEdit { id: Some(c.id), kind: c.kind });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "category.name", "Renamed category");
    a.push(Route::Transactions { category: Some(c.id), account: None });
    tick(&mut a, &mut clock);
    a.push(Route::CategoryEdit { id: None, kind: c.kind });
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    a.pop();
    tick(&mut a, &mut clock);
    tap_save(&mut a, &mut clock);
    let name = a.store.ledger.categories().unwrap().into_iter().find(|x| x.id == c.id).unwrap().name;
    assert_eq!(name, "Renamed category");
}

#[test]
fn an_editor_for_something_gone_saves_nothing() {
    let (mut a, _d) = app("gone");
    let mut clock = 1_000_000_000;
    let accounts = a.store.ledger.accounts().unwrap().len();
    a.push(Route::AccountEdit { id: Some(987_654), kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "Phantom");
    tap_save(&mut a, &mut clock);
    assert_eq!(a.store.ledger.accounts().unwrap().len(), accounts, "no new account in its place");
    let categories = a.store.ledger.categories().unwrap().len();
    a.push(Route::CategoryEdit { id: Some(987_654), kind: relay_money::model::CategoryKind::Expense });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "category.name", "Phantom");
    tap_save(&mut a, &mut clock);
    assert_eq!(a.store.ledger.categories().unwrap().len(), categories, "no new category in its place");
}

#[test]
fn a_category_pages_search_outlasts_an_account_page_opened_over_it() {
    let (mut a, _d) = app("tx-search");
    let mut clock = 1_000_000_000;
    let c = a.store.ledger.categories().unwrap().into_iter().find(|c| !c.archived).unwrap().id;
    let acc = a.store.ledger.accounts().unwrap().into_iter().find(|x| !x.archived).unwrap().id;
    a.push(Route::Transactions { category: Some(c), account: None });
    tick(&mut a, &mut clock);
    a.st.activity.tx_query = "metro".into();
    tick(&mut a, &mut clock);
    a.push(Route::Transactions { category: None, account: Some(acc) });
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.tx_query, "", "a new subject starts with no search");
    a.pop();
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.tx_query, "metro");
    // Opened anew it starts empty.
    a.pop();
    tick(&mut a, &mut clock);
    a.push(Route::Transactions { category: Some(c), account: None });
    tick(&mut a, &mut clock);
    assert_eq!(a.st.activity.tx_query, "");
}

// What an import writes

#[test]
fn a_skipped_transfer_does_not_take_the_match_a_purchase_needs() {
    // A card payment and a purchase of the same day and amount; the ledger holds the purchase once.
    let st = statement("Date,Description,Amount\n2026-11-02,PAYMENT THANK YOU / PAIEMENT MERCI,-50.00\n2026-11-02,METRO,-50.00\n");
    assert!(relay_money::payees::looks_like_transfer(&st.rows[0].description, st.rows[0].code.as_deref()));
    let day: relay_money::Date = "2026-11-02".parse().unwrap();
    let plan = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[(day, -50_00)]);
    // The payment is skipped, the purchase is the one already there: nothing is written twice.
    assert_eq!((plan.count(), plan.duplicates, plan.skipped), (0, 1, 1));
    assert!(plan.lines[1].duplicate && !plan.lines[0].duplicate);
}

#[test]
fn a_zero_amount_line_is_neither_counted_nor_read_as_money_in() {
    // The file reader drops zero lines itself; a statement handed over whole may still hold one.
    let mut st = statement("Date,Description,Amount\n2026-11-02,METRO,-45.20\n2026-11-03,CARD CHECK,-1.00\n");
    st.rows[1].amount = 0;
    let plan = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[]);
    assert_eq!(plan.count(), 1);
    assert_eq!(plan.uncategorized, 1, "only the purchase is an entry");
    assert!(plan.to_write.iter().all(|l| l.amount != 0));
    assert_eq!(plan.lines.len(), 2);
}

#[test]
fn a_new_account_left_off_its_statement_end_says_why() {
    let text = "Date,Description,Amount,Balance\n2026-11-02,METRO,-45.20,954.80\n2026-11-03,PAYMENT THANK YOU / PAIEMENT MERCI,-300.00,654.80\n";
    let st = statement(text);
    let money = |m: i64| format!("${}.{:02}", m / 100, m % 100);
    let skip = import::plan_import(&st, &import::Choices::default(), &[], &Default::default(), &[]);
    assert_eq!((skip.opening_from_statement, skip.end_balance), (Some(1_000_00), Some(654_80)));
    assert_eq!(import::new_account_gap(&skip), Some((954_80, 654_80)));
    assert_eq!(
        import::new_account_gap_line(&skip, money).as_deref(),
        Some("The new account would read $954.80, but the statement ends at $654.80. The transfer left out makes the difference: bring it in as Transfers or Entries.")
    );
    // Brought in as an entry, it ends where the statement does, and nothing is said.
    let entries = import::plan_import(&st, &import::Choices { transfers: import::TransferMode::Entries, ..Default::default() }, &[], &Default::default(), &[]);
    assert_eq!(import::new_account_gap(&entries), None);
    assert_eq!(import::new_account_gap_line(&entries, money), None);
}

#[test]
fn only_what_the_bank_link_writes_reads_as_connected() {
    use tally::accounts_store::follows_bank;
    // A Wealthsimple import's account number is not a bank connection; Plaid's mask is.
    assert!(!banks::is_bank_mask("HQ7XFMC41CAD"));
    assert!(banks::is_bank_mask("4411") && banks::is_bank_mask(" 07 "));
    assert!(!banks::is_bank_mask(""));
    assert!(!follows_bank("ws:HQ7XFMC41CAD", "HQ7XFMC41CAD", false, None));
    assert!(follows_bank("u1", "4411", false, None));
    // An account picked as "Into" carries no reference, but holds the entries the link wrote.
    assert!(follows_bank("u1", "", true, None));
    // Once the server has said what it follows, its word decides: a bank disconnected since
    // leaves its entries, not its connection.
    let followed = vec!["u2".to_string()];
    assert!(!follows_bank("u1", "4411", true, Some(&followed)));
    assert!(follows_bank("u2", "", false, Some(&followed)));
    let mut a = bank_account("a1", "checking");
    a.follow = true;
    a.into = Some("u2".into());
    let mut off = bank_account("a2", "savings");
    off.into = Some("u3".into());
    assert_eq!(banks::followed_uids(&[bank("Desjardins", vec![a, off])]), vec!["u2".to_string()]);
}

#[test]
fn another_banks_second_statement_goes_into_the_account_the_first_made() {
    let cheq_like = statement("Date,Description,Amount\n2026-11-06,IGA,-12.40\n2026-11-07,TIM HORTONS,-3.10\n2026-11-08,PAY,1500.00\n");
    let card = statement("Date,Description,Amount\n2026-11-02,METRO,45.20\n2026-11-03,STARBUCKS,6.75\n2026-11-04,AMAZON,30.00\n2026-11-05,PAYMENT,-100.00\n");
    let made = vec![view(1, "Cash", AccountType::Cash), view(7, "Imported account", AccountType::Chequing), view(8, "imported card ", AccountType::Credit)];
    for source in [None, Some(Source::Other)] {
        assert_eq!(import::guess_target(&made, source, cheq_like.positive_share()), Some(7));
        assert_eq!(import::guess_target(&made, source, card.positive_share()), Some(8), "by name, whatever its case");
    }
    // Its lines already logged there are found, so nothing is written twice.
    let existing: Vec<_> = cheq_like.rows.iter().map(|r| (r.date, r.amount)).collect();
    let plan = import::plan_import(&cheq_like, &import::Choices::default(), &[], &Default::default(), &existing);
    assert_eq!((plan.count(), plan.duplicates), (0, 3));
    // Two by that name: which one is the owner's to say. A card's statement never into chequing.
    let two = vec![view(7, "Imported account", AccountType::Chequing), view(9, "Imported account", AccountType::Savings)];
    assert_eq!(import::guess_target(&two, None, cheq_like.positive_share()), None);
    let cheq = vec![view(8, "Imported card", AccountType::Chequing)];
    assert_eq!(import::guess_target(&cheq, None, card.positive_share()), None);
    // An investment or archived account of that name is not one.
    let mut gone = view(7, "Imported account", AccountType::Chequing);
    gone.archived = true;
    assert_eq!(import::guess_target(&[gone, view(9, "Imported account", AccountType::Investment)], None, 0.2), None);
    // A bank's own account still wins: the fallback is only for when none is named for it.
    let d = vec![view(1, "Desjardins chequing", AccountType::Chequing), view(2, "Desjardins savings", AccountType::Savings), view(7, "Imported account", AccountType::Chequing)];
    assert_eq!(import::guess_target(&d, Some(Source::Desjardins), 0.2), None);
}

#[test]
fn a_second_statement_from_another_bank_adds_only_its_new_lines() {
    let (mut a, _d) = app("import-second");
    let mut clock = 1_000_000_000;
    let first = "Date,Description,Amount\n2026-11-06,IGA,-12.40\n2026-11-07,TIM HORTONS,-3.10\n2026-11-08,PAY,1500.00\n";
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, first);
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    tap_footer(&mut a, &mut clock);
    // The next month's file overlaps the first by its three lines.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, &format!("{first}2026-11-09,METRO,-20.00\n"));
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    let imported: Vec<AccountView> = a.store.ledger.accounts().unwrap().into_iter().filter(|x| x.name == "Imported account").collect();
    assert_eq!(imported.len(), 1, "no second account counting the money twice");
    assert_eq!(imported[0].balance, 1_464_50, "the overlap is not written again");
}

#[test]
fn a_bank_opened_from_the_list_is_suggested_places_but_keeps_what_it_follows() {
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing), mine("u-visa", "Desjardins Visa", AccountType::Credit)];
    let mut cheq = bank_account("a1", "checking");
    cheq.follow = false;
    let mut card = bank_account("a2", "credit card");
    card.follow = true;
    card.into = Some("u-cheq".into());
    let b = banks::with_places(&bank("Desjardins", vec![cheq, card]), &held);
    // The chequing account is placed, but the card already took u-cheq, so it isn't offered twice.
    assert_eq!((b.accounts[0].into.as_deref(), b.accounts[1].into.as_deref()), (None, Some("u-cheq")));
    assert_eq!((b.accounts[0].follow, b.accounts[1].follow), (false, true));
    let mut card = bank_account("a2", "credit card");
    card.follow = true;
    let b = banks::with_places(&bank("Desjardins", vec![bank_account("a1", "checking"), card]), &held);
    assert_eq!((b.accounts[0].into.as_deref(), b.accounts[1].into.as_deref()), (Some("u-cheq"), Some("u-visa")));
    assert_eq!((b.accounts[0].follow, b.accounts[1].follow), (false, true), "follow is the server's, unchanged");
}

#[test]
fn a_bank_account_into_a_deleted_tally_account_is_placed_afresh() {
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let mut cheq = bank_account("a1", "checking");
    cheq.follow = true;
    cheq.into = Some("u-gone".into());
    let b = banks::with_places(&bank("Desjardins", vec![cheq]), &held);
    // Not the deleted uid Follow would send while the page reads "Into a new account".
    assert_eq!(b.accounts[0].into.as_deref(), Some("u-cheq"));
    let b = banks::with_places(&bank("Desjardins", vec![b.accounts[0].clone()]), &[]);
    assert_eq!(b.accounts[0].into, None);
}

#[test]
fn a_statement_goes_where_the_last_one_from_its_bank_went_even_renamed() {
    let (mut a, _d) = app("import-renamed");
    let mut clock = 1_000_000_000;
    let first = "Date,Description,Amount\n2026-11-06,IGA,-12.40\n2026-11-07,TIM HORTONS,-3.10\n2026-11-08,PAY,1500.00\n";
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, first);
    // The new account's name left blank: it is saved, and said, as "Imported account".
    type_into(&mut a, &mut clock, "import.new_name", "");
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    assert!(a.toast.as_ref().is_some_and(|t| t.words.contains("into Imported account")), "{:?}", a.toast.as_ref().map(|t| &t.words));
    tap_footer(&mut a, &mut clock);
    let id = a.store.ledger.accounts().unwrap().into_iter().find(|x| x.name == "Imported account").expect("the account").id;
    // Renamed since, it no longer reads as the import's own by its name.
    a.store.ledger.account_update(id, &relay_money::views::AccountPatch { name: Some("RBC chequing".into()), ..Default::default() }).unwrap();
    a.invalidate();
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, &format!("{first}2026-11-09,METRO,-20.00\n"));
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    let accounts = a.store.ledger.accounts().unwrap();
    assert!(!accounts.iter().any(|x| x.name == "Imported account"), "no second account counting the money twice");
    let renamed = accounts.iter().find(|x| x.id == id).unwrap();
    assert_eq!(renamed.balance, 1_464_50, "the overlap is not written again");
}

#[test]
fn an_editor_parked_under_another_keeps_its_draft_when_the_app_is_stopped() {
    let (mut a, _d) = app("parked-saved");
    let mut clock = 1_000_000_000;
    let id = a.store.ledger.accounts().unwrap().into_iter().find(|x| !x.archived).unwrap().id;
    a.push(Route::AccountEdit { id: Some(id), kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "Parked, kept");
    a.push(Route::Transactions { category: None, account: Some(id) });
    tick(&mut a, &mut clock);
    a.push(Route::AccountEdit { id: None, kind: None });
    tick(&mut a, &mut clock);
    type_into(&mut a, &mut clock, "account.name", "On top");
    let saved = a.saved();
    let accounts: Vec<usize> = saved.drafts.iter().filter(|d| matches!(d.draft, tally::app::Draft::Account(_))).map(|d| d.depth).collect();
    assert_eq!(accounts.len(), 2, "the parked editor's draft is kept too: {accounts:?}");
    // Brought back, the lower editor finds what was typed in it.
    let (mut b, _e) = app("parked-restored");
    b.restore(&saved);
    tick(&mut b, &mut clock);
    b.pop();
    tick(&mut b, &mut clock);
    b.pop();
    tick(&mut b, &mut clock);
    assert_eq!(b.route(), &Route::AccountEdit { id: Some(id), kind: None });
    tap_save(&mut b, &mut clock);
    let name = b.store.ledger.accounts().unwrap().into_iter().find(|x| x.id == id).unwrap().name;
    assert_eq!(name, "Parked, kept");
}

// An older statement into the account a later one made

const SEPTEMBER: &str = "Date,Description,Amount,Balance\n2026-09-03,METRO,-50.00,800.00\n2026-09-15,PAY,350.00,1150.00\n";
const AUGUST: &str = "Date,Description,Amount,Balance\n2026-08-05,IGA,-100.00,900.00\n2026-08-20,TIM HORTONS,-50.00,850.00\n";
const OCTOBER: &str = "Date,Description,Amount,Balance\n2026-10-02,METRO,-20.00,1130.00\n";

/// Imports `csv` into wherever the import guesses, and says what it said it did.
fn import_file(a: &mut App, clock: &mut u64, csv: &str) -> String {
    a.push(Route::Import);
    tick(a, clock);
    import_preview(a, clock, csv);
    tap_footer(a, clock);
    assert_eq!(tally::import::stage(a), "done");
    let words = a.toast.as_ref().map(|t| t.words.clone()).unwrap_or_default();
    tap_footer(a, clock);
    words
}

fn imported(a: &App) -> (i64, i64) {
    let found: Vec<AccountView> = a.store.ledger.accounts().unwrap().into_iter().filter(|x| x.name == "Imported account").collect();
    assert_eq!(found.len(), 1, "one account, not a second counting the money twice");
    let opening = a.store.sql().unwrap().query_row("SELECT opening_balance FROM accounts WHERE id = ?1", [found[0].id], |r| r.get(0)).unwrap();
    (found[0].balance, opening)
}

#[test]
fn an_older_statement_after_a_later_one_ends_where_the_bank_does() {
    let (mut a, _d) = app("import-older");
    let mut clock = 1_000_000_000;
    import_file(&mut a, &mut clock, SEPTEMBER);
    // Made from September, it opens at the 31 August balance and ends where the bank does.
    assert_eq!(imported(&a), (1_150_00, 850_00));
    // August's lines are already in that opening: it moves back to before them, not counting
    // them twice ($1,000 where the bank says $1,150).
    let words = import_file(&mut a, &mut clock, AUGUST);
    assert_eq!(imported(&a), (1_150_00, 1_000_00), "the balance is the bank's and the opening stands on 31 July");
    assert!(words.contains("2 entries imported") && words.contains("starting balance moved back to"), "{words}");
    // Read again, nothing is new and nothing moves.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, AUGUST);
    tap_footer(&mut a, &mut clock);
    assert_eq!(imported(&a), (1_150_00, 1_000_00));
}

#[test]
fn a_later_statement_after_an_earlier_one_leaves_the_opening_alone() {
    let (mut a, _d) = app("import-in-order");
    let mut clock = 1_000_000_000;
    import_file(&mut a, &mut clock, SEPTEMBER);
    let words = import_file(&mut a, &mut clock, OCTOBER);
    assert_eq!(imported(&a), (1_130_00, 850_00));
    assert!(!words.contains("starting balance"), "{words}");
}

#[test]
fn an_older_statement_the_opening_does_not_hold_is_said_not_moved() {
    let money = |m: i64| format!("${}.{:02}", m / 100, m % 100);
    let short = |d: relay_money::Date| d.to_string();
    let aug = import::plan_import(&statement(AUGUST), &import::Choices::default(), &[], &Default::default(), &[]);
    let first = Some("2026-09-03".parse().unwrap());
    // The opening is where August ends: it holds August's lines, and moves back by them.
    let held = import::Opening { opening: 850_00, first_entry: first, valued: false };
    let date = "2026-08-05".parse().unwrap();
    assert_eq!(import::earlier_lines(&aug, &held), Some(import::Earlier::Move { from: 850_00, to: 1_000_00, back_to: date }));
    // Typed as the balance before August: the lines just add, and nothing is said.
    let before = import::Opening { opening: 1_000_00, ..held };
    assert_eq!(import::earlier_lines(&aug, &before), None);
    // Neither: the opening is left, and the preview says the account won't match the bank.
    let other = import::Opening { opening: 500_00, ..held };
    let e = import::earlier_lines(&aug, &other).expect("a word");
    assert_eq!(e, import::Earlier::Off { day: "2026-08-20".parse().unwrap(), reads: 350_00, says: 850_00 });
    assert_eq!(
        import::earlier_line(&e, "Imported account", 0, short, money),
        "Imported account would read $350.00 on 2026-08-20, but the statement says $850.00. Its starting balance is left as it is: change it in Accounts if it is off."
    );
    // A recorded value sets the balance: lines before it never move it.
    assert_eq!(import::earlier_lines(&aug, &import::Opening { valued: true, ..held }), None);
    // No balances to tell by: an opening that may hold them is said, not moved.
    let bare = import::plan_import(&statement("Date,Description,Amount\n2026-08-05,IGA,-100.00\n"), &import::Choices::default(), &[], &Default::default(), &[]);
    let e = import::earlier_lines(&bare, &held).expect("a word");
    assert_eq!(e, import::Earlier::Unsure { lines: 1, first_entry: first, opening: 850_00 });
    assert_eq!(
        import::earlier_line(&e, "Imported account", 0, short, money),
        "This line comes before Imported account's first entry, on 2026-09-03. If its starting balance of $850.00 already holds this line, it would count twice: change it in Accounts after."
    );
    assert_eq!(import::earlier_lines(&bare, &import::Opening { opening: 0, ..held }), None);
    // Lines on or after the first entry are no business of the opening's.
    let later = import::Opening { first_entry: Some("2026-08-01".parse().unwrap()), ..held };
    assert_eq!(import::earlier_lines(&aug, &later), None);
}

#[test]
fn a_bank_account_into_an_archived_tally_account_is_placed_afresh() {
    let mut old = mine("u-old", "Desjardins old chequing", AccountType::Chequing);
    old.archived = true;
    let held = vec![old, mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let mut cheq = bank_account("a1", "checking");
    cheq.follow = true;
    cheq.into = Some("u-old".into());
    let b = banks::with_places(&bank("Desjardins", vec![cheq]), &held);
    assert_eq!(b.accounts[0].into.as_deref(), Some("u-cheq"), "not the archived one the sheet can't show");
}

#[test]
fn a_second_connection_is_not_suggested_into_an_account_another_one_follows() {
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let s = banks::suggested_beside(&bank("Desjardins", vec![bank_account("a1", "checking")]), &held, &["u-cheq".to_string()]);
    assert_eq!(s.accounts[0].into, None);
    let s = banks::suggested_beside(&bank("Desjardins", vec![bank_account("a1", "checking")]), &held, &[]);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-cheq"));
}

#[test]
fn a_bank_named_by_a_generic_first_word_is_matched_by_the_word_that_names_it() {
    // "Bank of America" is not "My bank account".
    let held = vec![mine("u-mine", "My bank account", AccountType::Chequing)];
    let s = banks::suggested(&bank("Bank of America", vec![bank_account("a1", "checking")]), &held);
    assert_eq!(s.accounts[0].into, None);
    let held = vec![mine("u-boa", "America chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("Bank of America", vec![bank_account("a1", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-boa"));
    // "Caisse populaire Desjardins" is Desjardins; RBC and Wealthsimple still match.
    let held = vec![mine("u-cheq", "Desjardins chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("Caisse populaire Desjardins", vec![bank_account("a1", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-cheq"));
    let held = vec![mine("u-rbc", "RBC chequing", AccountType::Chequing)];
    let s = banks::suggested(&bank("RBC Royal Bank", vec![bank_account("a1", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-rbc"));
    let held = vec![mine("u-ws", "Wealthsimple Cash", AccountType::Chequing)];
    let s = banks::suggested(&bank("Wealthsimple", vec![bank_account("a1", "checking")]), &held);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-ws"));
}

#[test]
fn a_banks_shared_memory_gives_way_to_a_guess_of_another_kind() {
    let both = vec![view(1, "Desjardins chequing", AccountType::Chequing), view(2, "Desjardins Visa", AccountType::Credit)];
    let cheq_like = statement("Date,Description,Amount\n2026-11-02,METRO,-45.20\n2026-11-04,PAY,1200.00\n");
    let guess = import::guess_target(&both, Some(Source::Desjardins), cheq_like.positive_share());
    assert_eq!(guess, Some(1));
    // The last file that read like chequing went into the Visa (it writes purchases below zero):
    // the next chequing file is not put there, pay and spending landing in the card.
    assert_eq!(import::pick_target(&both, None, Some(2), false, guess), Some(1));
    // The file named the account it is from: that account's own memory wins.
    assert_eq!(import::pick_target(&both, Some(2), Some(1), false, guess), Some(2));
    // A memory of the guess's kind, or with no guess at all (renamed since), still wins.
    let two = vec![view(1, "Desjardins chequing", AccountType::Chequing), view(3, "Joint chequing", AccountType::Chequing)];
    assert_eq!(import::pick_target(&two, None, Some(3), false, Some(1)), Some(3));
    assert_eq!(import::pick_target(&both, None, Some(2), false, None), Some(2));
    // A card's file never into chequing, by either memory.
    assert_eq!(import::pick_target(&both, Some(1), Some(1), true, Some(2)), Some(2));
}

#[test]
fn a_banks_card_and_chequing_files_each_find_their_own_account() {
    let (mut a, _d) = app("import-card-and-chequing");
    let mut clock = 1_000_000_000;
    let cheq = a.store.ledger.account_add("Desjardins chequing", AccountType::Chequing, 0, None, "").unwrap().id;
    let visa = a.store.ledger.account_add("Desjardins Visa", AccountType::Credit, 0, None, "").unwrap().id;
    a.invalidate();
    let card_file = |day: u32| format!("Date,Description,Amount,Account\n2026-11-{day:02},METRO,-45.20,VISA 4510\n2026-11-{:02},PAYMENT,100.00,VISA 4510\n", day + 1);
    let cheq_file = "Date,Description,Amount,Account\n2026-11-06,IGA,-12.40,EOP 81234\n2026-11-08,PAY,1500.00,EOP 81234\n";
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    tally::import::choose_source(&mut a, Source::Desjardins);
    // The card writes purchases below zero: it reads like chequing, and is put into the Visa.
    import_preview(&mut a, &mut clock, &card_file(2));
    assert_eq!(tally::import::into(&a), Some(cheq), "the bank-name guess");
    tally::import::pick_into(&mut a, Some(visa));
    tick(&mut a, &mut clock);
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    tap_footer(&mut a, &mut clock);
    // The chequing file reads the same way, but names another account: it is not put in the Visa.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, cheq_file);
    assert_eq!(tally::import::into(&a), Some(cheq));
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    tap_footer(&mut a, &mut clock);
    // And the card's next file still finds the Visa, by the account it names.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, &card_file(12));
    assert_eq!(tally::import::into(&a), Some(visa));
}

#[test]
fn an_older_statement_stopped_before_its_opening_moved_moves_it_when_read_again() {
    let (mut a, _d) = app("import-older-stopped");
    let mut clock = 1_000_000_000;
    import_file(&mut a, &mut clock, SEPTEMBER);
    assert_eq!(imported(&a), (1_150_00, 850_00));
    // The opening cannot move (as when the app is stopped between the lines and the move).
    a.store.sql().unwrap().execute_batch("CREATE TRIGGER hold BEFORE UPDATE OF opening_balance ON accounts BEGIN SELECT RAISE(ABORT, 'held'); END;").unwrap();
    let words = import_file(&mut a, &mut clock, AUGUST);
    assert!(words.contains("2 entries imported") && words.contains("could not be moved"), "{words}");
    // The lines are in and the opening is as it was: never a moved opening with its lines missing.
    assert_eq!(imported(&a), (1_000_00, 850_00));
    // Read again, every line is already in, and the move is still offered: Import moves just it.
    a.store.sql().unwrap().execute_batch("DROP TRIGGER hold;").unwrap();
    let words = import_file(&mut a, &mut clock, AUGUST);
    assert!(words.contains("starting balance moved back to") && words.contains("2 already there"), "{words}");
    assert_eq!(imported(&a), (1_150_00, 1_000_00), "the balance is the bank's");
    // And once more, nothing is left to do.
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, AUGUST);
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "preview", "nothing to import");
    assert_eq!(imported(&a), (1_150_00, 1_000_00));
}

#[test]
fn the_move_left_undone_is_told_by_the_statement_alone() {
    let aug = statement(AUGUST);
    // August's two lines are logged (the account's first entry is now 5 August), and the opening
    // still stands where August ends.
    let existing: Vec<_> = aug.rows.iter().map(|r| (r.date, r.amount)).collect();
    let plan = import::plan_import(&aug, &import::Choices::default(), &[], &Default::default(), &existing);
    assert_eq!(plan.count(), 0);
    let first = Some("2026-08-05".parse().unwrap());
    let held = import::Opening { opening: 850_00, first_entry: first, valued: false };
    assert_eq!(import::earlier_lines(&plan, &held), Some(import::Earlier::Move { from: 850_00, to: 1_000_00, back_to: "2026-08-05".parse().unwrap() }));
    // Moved already: nothing to do.
    assert_eq!(import::earlier_lines(&plan, &import::Opening { opening: 1_000_00, ..held }), None);
    // An opening the statement never stood at is not moved.
    assert_eq!(import::earlier_lines(&plan, &import::Opening { opening: 700_00, ..held }), None);
    // Entries before the statement: it cannot say what stood on the eve of the first one.
    assert_eq!(import::earlier_lines(&plan, &import::Opening { first_entry: Some("2026-07-01".parse().unwrap()), ..held }), None);
    // None of its lines logged: they are written, and the opening is the earlier case's business.
    let fresh = import::plan_import(&aug, &import::Choices::default(), &[], &Default::default(), &[]);
    let later = import::Opening { first_entry: Some("2026-08-05".parse().unwrap()), ..held };
    assert_eq!(import::earlier_lines(&fresh, &later), None);
}

#[test]
fn a_tally_csv_in_another_currency_with_the_same_decimals_is_refused() {
    let (mut a, _d) = app("csv-usd");
    let before = a.store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions.len();
    let ours = a.store.formatter().currency;
    let other = if ours == "USD" { "CAD" } else { "USD" };
    let text = format!("date,type,amount,currency,category,account,to_account,note\n2026-11-02,EXPENSE,12.50,{other},Food,Cash,,Lunch\n");
    let why = tally::import_store::import_tally_csv(&mut a.store, &text).unwrap_err();
    assert!(why.contains(other) && why.contains(&ours) && why.contains("Nothing was imported"), "{why}");
    assert_eq!(a.store.ledger.tx_list(&Default::default(), tally::model::today()).unwrap().transactions.len(), before);
    // In its own currency (any case) it comes in.
    let text = format!("date,type,amount,currency,category,account,to_account,note\n2026-11-02,EXPENSE,12.50,{},Food,Cash,,Lunch\n", ours.to_lowercase());
    assert!(tally::import_store::import_tally_csv(&mut a.store, &text).is_ok());
}

#[test]
fn a_retry_that_succeeds_leaves_no_failure_on_done() {
    let (mut a, _d) = app("import-retry");
    let mut clock = 1_000_000_000;
    a.store.sql().unwrap().execute_batch("CREATE TRIGGER boom BEFORE INSERT ON transactions BEGIN SELECT RAISE(ABORT, 'boom'); END;").unwrap();
    a.push(Route::Import);
    tick(&mut a, &mut clock);
    import_preview(&mut a, &mut clock, "Date,Description,Amount\n2026-11-06,IGA,-12.40\n2026-11-08,PAY,1500.00\n");
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "preview");
    assert!(tally::import::problem(&a).is_some_and(|p| p.starts_with("The import did not finish")));
    a.store.sql().unwrap().execute_batch("DROP TRIGGER boom;").unwrap();
    tap_footer(&mut a, &mut clock);
    assert_eq!(tally::import::stage(&a), "done");
    assert_eq!(tally::import::problem(&a), None);
}
