//! Settings, Import and Connected banks: their rules, the import's writes and dedupe, the backup's
//! round trip, the server's bank answers, and (with TALLY_SHOT_DIR set) the screens in the states
//! the desktop's `--shot` cannot reach.

use std::path::PathBuf;
use std::sync::Arc;

use relay_money::Date;
use relay_money::bank_statements::{self, StatementRead};
use relay_money::model::AccountType;
use serde_json::json;
use tally::app::{App, Host};
use tally::banks_logic::{self as banks, Answer, LinkState};
use tally::import_logic::{self as import, Choices, TransferMode};
use tally::import_store::{self, Target};
use tally::model::Store;
use tally::settings_logic as settings;
use tally::sync::Failure;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tally-setup-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn store(tag: &str) -> Store {
    Store::open(&temp_dir(tag).join("tally.db")).unwrap()
}

const STATEMENT: &str = "Date,Description,Amount,Balance\n\
2026-11-02,METRO PLUS #4411 MONTREAL QC,-45.20,954.80\n\
2026-11-03,PAYROLL ACME INC,2000.00,2954.80\n\
2026-11-04,STARBUCKS 0231,-6.75,2948.05\n";

fn statement() -> relay_money::bank_statements::Statement {
    match bank_statements::read(STATEMENT, 2, false) {
        StatementRead::Ok(s) => s,
        other => panic!("the statement reads: {other:?}"),
    }
}

// Settings

#[test]
fn settings_rows_read_as_the_kotlin_ones() {
    assert_eq!(settings::ordinal(1), "1st");
    assert_eq!(settings::ordinal(2), "2nd");
    assert_eq!(settings::ordinal(11), "11th");
    assert_eq!(settings::ordinal(23), "23rd");
    assert_eq!(settings::format_summary("CAD", 15, false), "CAD · month starts on the 15th · week from Sunday");
    assert_eq!(settings::entries_text(1), "1 entry");
    assert_eq!(settings::entries_text(1284), "1,284 entries");
    assert_eq!(settings::accounts_summary(0, "$0"), "No accounts yet");
    assert_eq!(settings::categories_summary(13, 4), "13 for spending · 4 for income");
    assert_eq!(settings::palette_name("mint"), "Light", "a sky palette opens on Light");
    assert_eq!(settings::palette_name("night"), "Dark");
    assert_eq!(settings::palette_name("system"), "System");
    assert_eq!(settings::ledger_line(1284, Some("2024-03-02")), "1,284 entries since March 2024");
    assert_eq!(settings::ledger_line(1, None), "1 entry");
    assert_eq!(settings::ledger_line(0, None), "No entries yet");
    assert_eq!(settings::currency_codes("XOF")[0], "XOF", "an uncommon pick stays in view");
    let usd = settings::currency_option("USD", &relay_money::money::Locale::new("en-CA"));
    assert_eq!(usd.name, "US dollar");
    assert!(usd.subtitle().contains("1,234.56"));
}

#[test]
fn a_backup_restores_what_it_saved_and_refuses_what_is_not_one() {
    let mut a = store("backup-a");
    a.load_sample(tally::model::today()).unwrap();
    let file = a.ledger.export_backup("2026-11-19").unwrap();
    let bytes = relay_money::backup::encode(&file).into_bytes();
    let read = settings::read_backup(&bytes).unwrap();
    let preview = settings::restore_preview(&read);
    assert_eq!(preview.entries, file.transactions.len());
    assert_eq!(preview.saved_on, Some(Date::constant(2026, 11, 19)));
    let prompt = settings::restore_prompt(&preview, Some("19 Nov"));
    assert!(prompt.contains("saved 19 Nov") && prompt.contains("replaced"));
    let mut b = store("backup-b");
    let n = b.ledger.import_backup(&read).unwrap();
    assert_eq!(n, file.transactions.len());
    assert_eq!(tally::settings_store::counts(&b).entries, tally::settings_store::counts(&a).entries);
    assert!(settings::read_backup(b"date,amount\n").is_err());
    assert_eq!(settings::read_backup(b"").unwrap_err(), "That file could not be opened.");
}

#[test]
fn the_week_start_is_written_where_the_ledger_reads_it() {
    let s = store("week");
    tally::settings_store::set_week_starts_monday(&s, false).unwrap();
    assert!(!s.ledger.settings().unwrap().week_starts_monday);
    tally::settings_store::set_week_starts_monday(&s, true).unwrap();
    assert!(s.ledger.settings().unwrap().week_starts_monday);
}

// Import

#[test]
fn an_import_writes_once_and_finds_its_own_lines_already_there() {
    let mut s = store("import");
    let st = statement();
    let categories = s.ledger.categories().unwrap();
    let plan = import::plan_import(&st, &Choices::default(), &categories, &Default::default(), &[]);
    assert_eq!(plan.count(), 3);
    assert_eq!(plan.duplicates, 0);
    assert_eq!(plan.out, 4520 + 675);
    assert_eq!(plan.inn, 200_000);
    assert_eq!(plan.opening_from_statement, Some(100_000), "the balance before the first line");
    let target = Target::New { name: "Desjardins chequing".into(), ty: AccountType::Chequing, opening: plan.opening_from_statement.unwrap() };
    let id = import_store::write(&mut s.ledger, &target, &plan.to_write).unwrap();
    let account = s.ledger.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap();
    assert_eq!(account.balance, 294_805, "the account ends where the statement does");
    // The same file again: every line already there, nothing to write.
    let existing = import_store::existing_effects(&s, id, plan.first.unwrap(), plan.last.unwrap());
    let again = import::plan_import(&st, &Choices::default(), &categories, &Default::default(), &existing);
    assert_eq!((again.count(), again.duplicates), (0, 3));
}

#[test]
fn a_flipped_card_statement_and_the_payees_the_owner_filed() {
    let st = statement();
    let s = store("learned");
    let categories = s.ledger.categories().unwrap();
    let dining = categories.iter().find(|c| c.kind == relay_money::model::CategoryKind::Expense && c.name != "Groceries").unwrap();
    let uses = vec![import::NoteUse { note: "Metro Plus".into(), category_id: dining.id, r#type: relay_money::model::TxType::Expense }];
    let learned = import::learned_categories(&uses, &categories);
    let plan = import::plan_import(&st, &Choices::default(), &categories, &learned, &[]);
    let metro = plan.lines.iter().find(|l| l.note.to_lowercase().contains("metro")).unwrap();
    assert_eq!(metro.category_id, Some(dining.id));
    assert!(metro.learned);
    let flipped = import::plan_import(&st, &Choices { flip: true, ..Choices::default() }, &categories, &learned, &[]);
    assert_eq!(flipped.inn, 4520 + 675);
    assert!(!import::guess_flip(&st, AccountType::Chequing));
}

#[test]
fn transfers_are_skipped_or_paired_as_the_owner_says() {
    let text = "Date,Description,Amount\n2026-11-02,PAYMENT THANK YOU / PAIEMENT MERCI,-300.00\n2026-11-03,STARBUCKS 0231,-6.75\n";
    let StatementRead::Ok(st) = bank_statements::read(text, 2, false) else { panic!("reads") };
    let transfers = st.rows.iter().filter(|r| relay_money::payees::looks_like_transfer(&r.description, r.code.as_deref())).count();
    let skip = import::plan_import(&st, &Choices::default(), &[], &Default::default(), &[]);
    assert_eq!(skip.count(), 2 - transfers);
    let paired = import::plan_import(&st, &Choices { transfers: TransferMode::Transfers, counterpart: Some(7), ..Choices::default() }, &[], &Default::default(), &[]);
    assert_eq!(paired.count(), 2);
    assert_eq!(paired.to_write.iter().filter(|l| l.counterpart == Some(7)).count(), transfers);
}

#[test]
fn tallys_own_csv_comes_back_with_its_accounts_and_categories() {
    let mut a = store("csv-a");
    a.load_sample(tally::model::today()).unwrap();
    let rows = tally::settings_store::csv_rows(&a);
    let text = relay_money::csv::encode(&rows, "CAD", 2);
    assert_eq!(bank_statements::read(&text, 2, false), StatementRead::TallyCsv);
    let mut b = store("csv-b");
    let words = import_store::import_tally_csv(&mut b, &text).unwrap();
    assert!(words.starts_with("Imported"), "{words}");
    assert_eq!(tally::settings_store::counts(&b).entries as usize, rows.len());
}

#[test]
fn import_words() {
    assert_eq!(import::imported_line(1, "Visa", 0), "1 entry imported into Visa");
    assert_eq!(import::imported_line(84, "Desjardins chequing", 12), "84 entries imported into Desjardins chequing · 12 already there");
    assert_eq!(import::invest_imported_line(12, 0, 2, 0), "Imported 12 holdings · 2 accounts added");
    assert_eq!(import::invest_imported_line(0, 0, 0, 3), "Nothing new to add · 3 already in Tally");
    assert!(import::belongs_to("Desjardins chequing", Some(import::Source::Desjardins)));
    assert!(!import::belongs_to("Desjardins chequing", Some(import::Source::Other)));
    assert!(!import::day_first_for("en", "CA"));
    assert!(import::day_first_for("fr", "CA"));
}

// Banks

fn bank_json() -> serde_json::Value {
    json!({
        "id": "b1", "institution": "Desjardins", "last_sync": 1_000,
        "accounts": [
            { "id": "a1", "name": "Chequing", "mask": "4411", "kind": "checking", "follow": true },
            { "id": "a2", "name": "Visa", "mask": "0007", "kind": "credit card", "follow": false },
            { "id": "a3", "name": "TFSA", "mask": "", "kind": "tfsa", "follow": false }
        ]
    })
}

#[test]
fn the_servers_bank_answers_read_as_the_kotlin_ones() {
    let fail = |code: &str| Failure { code: code.into(), message: "no".into() };
    assert_eq!(banks::answer(Err(Failure::not_paired())), Answer::NotPaired);
    assert_eq!(banks::answer(Err(fail("bus.unknown_op"))), Answer::NotOnServer);
    assert_eq!(banks::answer(Err(fail("remote.op"))), Answer::NotOnServer);
    assert_eq!(banks::answer(Err(fail("other"))), Answer::Failed("no".into()));
    assert_eq!(banks::failure_words(&fail("bus.unknown_op")), banks::NOT_ON_SERVER);
    let Answer::Ready(list) = banks::answer(Ok(json!({ "banks": [bank_json(), { "no": "id" }] }))) else { panic!("ready") };
    assert_eq!(list.len(), 1, "a bank without an id is left out");
    assert_eq!(list[0].accounts.len(), 3);
    assert_eq!(banks::bank_line(&list[0], 61_000 + 1_000), "1 of 3 accounts followed · read 1m ago");
}

#[test]
fn a_new_bank_follows_its_spending_accounts_into_the_ones_they_are() {
    let bank = banks::parse_bank(&bank_json()).unwrap();
    let mine = vec![
        banks::Mine { id: 1, uid: "u-cheq".into(), name: "Desjardins chequing".into(), r#type: AccountType::Chequing, archived: false, institution: String::new(), external_ref: String::new() },
        banks::Mine { id: 2, uid: "u-visa".into(), name: "Visa Desjardins".into(), r#type: AccountType::Credit, archived: false, institution: String::new(), external_ref: String::new() },
    ];
    let s = banks::suggested(&bank, &mine);
    let follows: Vec<bool> = s.accounts.iter().map(|a| a.follow).collect();
    assert_eq!(follows, vec![true, true, false]);
    assert_eq!(s.accounts[0].into.as_deref(), Some("u-cheq"));
    assert_eq!(s.accounts[1].into.as_deref(), Some("u-visa"));
    let payload = banks::follow_payload(&s, "2026-10-20");
    assert_eq!(payload["bank_id"], "b1");
    assert_eq!(payload["since"], "2026-10-20");
    assert_eq!(payload["accounts"][1], json!({ "id": "a2", "follow": true, "into": "u-visa" }));
    assert_eq!(payload["accounts"][2]["follow"], false);
    assert_eq!(banks::follow_label(0), "Follow nothing for now");
    assert_eq!(banks::follow_label(2), "Follow 2 accounts");
    assert_eq!(banks::Since::Year.date(Date::constant(2026, 11, 19)), "2026-01-01");
}

#[test]
fn hosted_link_answers() {
    let link = banks::parse_hosted_link(&json!({ "link_token": "link-1", "hosted_link_url": "https://hosted.plaid.com/link/abc", "expires_at": 5 })).unwrap();
    assert_eq!((link.link_token.as_str(), link.expires_at), ("link-1", Some(5)));
    assert!(banks::parse_hosted_link(&json!({ "link_token": "x", "hosted_link_url": "javascript:alert(1)" })).is_none(), "only a web page opens");
    assert_eq!(banks::parse_link_state(&json!({ "state": "waiting" })), Some(LinkState::Waiting));
    assert_eq!(banks::parse_link_state(&json!({ "state": "expired" })), Some(LinkState::Expired));
    assert_eq!(banks::parse_link_state(&json!({ "state": "exited", "problem": "Closed" })), Some(LinkState::Exited(Some("Closed".into()))));
    assert!(matches!(banks::parse_link_state(&json!({ "state": "connected", "bank": bank_json() })), Some(LinkState::Connected(b)) if b.id == "b1"));
    assert_eq!(banks::parse_link_state(&json!({ "state": "connected" })), None, "connected names its bank");
}

#[test]
fn a_bank_says_a_week_ahead_that_it_wants_a_sign_in() {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    let now = jiff::Timestamp::now().as_millisecond();
    let bank = |id: &str, name: &str, problem: Option<&str>, expires: Option<i64>| {
        let mut b = bank_json();
        b["id"] = json!(id);
        b["institution"] = json!(name);
        if let Some(p) = problem {
            b["problem"] = json!(p);
        }
        if let Some(at) = expires {
            b["expires_at"] = json!(at);
        }
        banks::parse_bank(&b).unwrap()
    };
    let list = vec![
        bank("far", "Far Bank", None, Some(now + 60 * DAY)),
        bank("soon", "Wealthsimple", None, Some(now + 3 * DAY)),
        bank("none", "No End", None, None),
        bank("out", "Desjardins", Some("Sign in to Desjardins again"), None),
    ];
    assert_eq!(list[1].expires_at, Some(now + 3 * DAY));
    let r = banks::renewals(&list, now);
    // Signed out first, then the soonest; a bank a month off, or with no end, says nothing yet.
    let ids: Vec<&str> = r.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["out", "soon"]);
    assert!(r[0].signed_out && !r[1].signed_out);
    assert_eq!(banks::renew_line(&r[0], now), "Signed out: sign in to bring new purchases in");
    assert!(banks::renew_line(&r[1], now).starts_with("Asks you to sign in again by "), "{}", banks::renew_line(&r[1], now));
    assert!(banks::wants_sign_in(&list[1], now) && !banks::wants_sign_in(&list[0], now) && !banks::wants_sign_in(&list[2], now));
    // A consent already over is signed out, whatever the last read said.
    assert!(banks::renewals(&[bank("over", "Over", None, Some(now - DAY))], now)[0].signed_out);
    assert_eq!(banks::by_words(now, now), "today");
    assert_eq!(banks::by_words(now + DAY, now), "tomorrow");
    assert!(banks::by_words(now + 20 * DAY, now).starts_with("by "));
    assert_eq!(banks::home_words(&r[0], now), ("Sign in to Desjardins again".into(), "New purchases are paused until you do".into()));
    let (title, line) = banks::home_words(&r[1], now);
    assert_eq!(title, "Sign in to Wealthsimple again");
    assert!(line.starts_with("By ") && line.ends_with(", or new purchases pause"), "{line}");

    // Kept for Overview, and listed again only every few hours.
    let mut s = store("renewals");
    assert!(banks::check_due(&s, now));
    banks::remember_renewals(&mut s, &list, now);
    assert_eq!(banks::renew_pref(&s), r);
    assert!(!banks::check_due(&s, now + 60_000));
    assert!(banks::check_due(&s, now + banks::CHECK_EVERY_MS));
    assert!(banks::check_due(&s, now - DAY), "a clock set back asks again");
}

// The screens, drawn (TALLY_SHOT_DIR=dir cargo test -p tally-rs --test setup shots)

struct TestHost {
    dir: PathBuf,
}

impl Host for TestHost {
    fn data_dir(&self) -> PathBuf {
        self.dir.clone()
    }
    fn log(&self, _text: &str) {}
}

fn app(tag: &str, palette: &str) -> App {
    let mut a = App::new(Arc::new(TestHost { dir: temp_dir(tag) }), 1_000_000_000);
    a.resize(1080.0, 2340.0, 1080.0 * 0.085, 1080.0 * 0.04);
    a.store.load_sample(tally::model::today()).unwrap();
    a.invalidate();
    a.set_appearance(palette);
    a
}

fn frames(a: &mut App, clock: &mut u64, n: usize) {
    for _ in 0..n {
        *clock += 16_666_666;
        a.frame(*clock);
    }
}

#[cfg(not(target_os = "android"))]
fn shoot(a: &mut App, clock: &mut u64, gfx: &mut tally::gfx::Gfx, out: &std::path::Path) {
    // Frames until the pictures the page asked for have been decoded (a second at most).
    for i in 0..60 {
        *clock += 16_666_666;
        a.frame(*clock);
        gfx.upload_glyphs(&mut a.canvas.uploads);
        gfx.upload_images(&mut a.canvas.image_uploads);
        if i >= 3 && !tally::pictures::pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    *clock += 16_666_666;
    a.frame(*clock);
    gfx.upload_glyphs(&mut a.canvas.uploads);
    gfx.upload_images(&mut a.canvas.image_uploads);
    let px = gfx.render_to_pixels(1080, 2340, &a.canvas.globals, a.clear_colour(), &a.canvas.insts, &a.canvas.batches);
    let file = std::fs::File::create(out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 1080, 2340);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px).unwrap();
}

#[cfg(not(target_os = "android"))]
#[test]
fn shots() {
    let Some(dir) = std::env::var_os("TALLY_SHOT_DIR").map(PathBuf::from) else { return };
    let Ok(mut gfx) = tally::gfx::Gfx::headless() else { return };
    for palette in ["light", "dark"] {
        // Import: a statement read and planned.
        let mut clock = 1_000_000_000;
        let mut a = app(&format!("shot-import-{palette}"), palette);
        a.push(tally::nav::Route::Import);
        frames(&mut a, &mut clock, 1);
        tally::import::open_file(&mut a, "statement.csv".into(), STATEMENT.as_bytes().to_vec());
        for _ in 0..200 {
            frames(&mut a, &mut clock, 1);
            if tally::import::stage(&a) == "preview" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(tally::import::stage(&a), "preview");
        shoot(&mut a, &mut clock, &mut gfx, &dir.join(format!("import-preview-{palette}.png")));
        a.scroll().off = 700.0;
        shoot(&mut a, &mut clock, &mut gfx, &dir.join(format!("import-preview-lines-{palette}.png")));

        // Banks: the list, empty and with a bank, and a bank just connected.
        let mut a = app(&format!("shot-banks-{palette}"), palette);
        a.push(tally::nav::Route::Banks);
        frames(&mut a, &mut clock, 2);
        tally::banks::show(&mut a, json!({ "banks": [] }), None);
        shoot(&mut a, &mut clock, &mut gfx, &dir.join(format!("banks-welcome-{palette}.png")));
        let mut listed = bank_json();
        listed["last_sync"] = json!(jiff::Timestamp::now().as_millisecond() - 3 * 3_600_000);
        let mut troubled = bank_json();
        troubled["id"] = json!("b2");
        troubled["institution"] = json!("Wealthsimple");
        troubled["problem"] = json!("Sign in again to keep syncing");
        let mut soon = bank_json();
        soon["id"] = json!("b3");
        soon["institution"] = json!("TD Canada Trust");
        soon["last_sync"] = json!(jiff::Timestamp::now().as_millisecond() - 20 * 60_000);
        soon["expires_at"] = json!(jiff::Timestamp::now().as_millisecond() + 4 * 24 * 3_600_000);
        tally::banks::show(&mut a, json!({ "banks": [listed, troubled, soon] }), None);
        shoot(&mut a, &mut clock, &mut gfx, &dir.join(format!("banks-list-{palette}.png")));
        tally::banks::show(&mut a, json!({ "banks": [] }), Some(bank_json()));
        shoot(&mut a, &mut clock, &mut gfx, &dir.join(format!("banks-choose-{palette}.png")));
    }
}
