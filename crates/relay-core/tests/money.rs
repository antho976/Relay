//! The Money space's ledger across the bus door: what the desktop and, later, the phone call.

mod common;

use common::{call_as, code, engine, ok, wait_until};
use relay_bus::Actor;
use serde_json::json;

#[test]
fn a_first_account_then_an_entry_reads_on_home() {
    let engine = engine();
    let empty = ok(&engine, "money.summary", json!({"today": "2026-10-07"}));
    assert_eq!(empty["empty"], true);
    let account = ok(&engine, "money.account.add", json!({"name": "Chequing", "type": "CHEQUING", "opening_balance": 100_000}));
    let lists = ok(&engine, "money.lists", json!({}));
    let groceries = lists["categories"].as_array().unwrap().iter().find(|c| c["name"] == "Groceries").unwrap()["id"].clone();
    ok(&engine, "money.budget.set", json!({"amount": 310_000}));
    let tx = ok(&engine, "money.tx.add", json!({
        "type": "EXPENSE", "amount": 4_250, "date": "2026-10-05", "account_id": account["id"], "category_id": groceries, "note": "Metro"
    }));
    assert_eq!(tx["category"], "Groceries");
    let home = ok(&engine, "money.summary", json!({"today": "2026-10-07"}));
    assert_eq!(home["empty"], false);
    assert_eq!(home["spent"], 4_250);
    assert_eq!(home["accounts"][0]["balance"], 95_750);
    assert_eq!(home["pace"]["status"], "UNDER_PACE");
    assert!(home["lines"]["margin"].as_str().unwrap().contains("a day for 25 days"), "{}", home["lines"]["margin"]);
    let page = ok(&engine, "money.tx.list", json!({"query": "metro", "today": "2026-10-07"}));
    assert_eq!(page["transactions"].as_array().unwrap().len(), 1);

    ok(&engine, "money.tx.delete", json!({"id": tx["id"]}));
    assert_eq!(ok(&engine, "money.summary", json!({"today": "2026-10-07"}))["spent"], 0);
    ok(&engine, "money.tx.restore", json!({"id": tx["id"]}));
    assert_eq!(ok(&engine, "money.summary", json!({"today": "2026-10-07"}))["spent"], 4_250);
}

#[test]
fn bad_entries_are_refused_with_a_reason() {
    let engine = engine();
    ok(&engine, "money.account.add", json!({"name": "Cash", "type": "CASH"}));
    let refused = call_as(&engine, Actor::User, "money.tx.add", json!({"type": "EXPENSE", "amount": 0, "date": "2026-10-05", "account_id": 1}));
    assert_eq!(code(refused), "money.invalid");
    let unknown = call_as(&engine, Actor::User, "money.tx.add", json!({"type": "EXPENSE", "amount": 5, "date": "2026-10-05", "account_id": 1, "colour": 3}));
    assert_eq!(code(unknown), "bus.schema");
}

#[test]
fn a_tally_backup_restores_and_exports_the_same_ledger() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let sample = ok(&engine, "money.sample", json!({}));
    assert!(sample["transactions"].as_u64().unwrap() > 50);
    let out = dir.path().join("tally-backup.json");
    let exported = ok(&engine, "money.export", json!({"path": out}));
    assert_eq!(exported["transactions"], sample["transactions"]);
    ok(&engine, "money.reset", json!({}));
    assert_eq!(ok(&engine, "money.summary", json!({}))["empty"], true);
    let imported = ok(&engine, "money.import", json!({"path": out}));
    assert_eq!(imported["kind"], "backup");
    assert_eq!(imported["transactions"], sample["transactions"]);
    let again = dir.path().join("again.json");
    ok(&engine, "money.export", json!({"path": again}));
    let strip = |p: &std::path::Path| {
        let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        v["exportedAt"] = json!(null);
        v
    };
    assert_eq!(strip(&out), strip(&again));
    // An export over an old one is renamed in whole, and only the user reads it.
    std::fs::set_permissions(&out, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    ok(&engine, "money.export", json!({"path": out}));
    assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&out).unwrap().permissions()) & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2, "a temporary file was left beside the export");
    assert_eq!(strip(&out), strip(&again));
}

#[test]
fn an_agent_may_add_entries_but_never_wipe_the_ledger() {
    let engine = engine();
    ok(&engine, "money.account.add", json!({"name": "Cash", "type": "CASH"}));
    let agent = Actor::agent("brisk-otter");
    let add = call_as(&engine, agent.clone(), "money.tx.add", json!({"type": "INCOME", "amount": 500, "date": "2026-10-05", "account_id": 1}));
    assert!(add.error.is_none(), "{:?}", add.error);
    for op in ["money.reset", "money.sample"] {
        let r = call_as(&engine, agent.clone(), op, json!({}));
        assert!(r.error.is_some(), "{op} must refuse an agent");
    }
    assert!(call_as(&engine, agent, "money.import", json!({"path": "/tmp/x.json"})).error.is_some());
    // Writing the ledger over a file is the person's as much as reading one in: a role
    // allowlist that lets `money.*` through must still not let an agent overwrite ~/.bashrc.
    for op in ["money.import", "money.export", "money.reset", "money.sync"] {
        let entry = relay_bus::registry::Registry::global().get(op).unwrap();
        assert!(matches!(entry.meta.actors, relay_bus::registry::Actors::UserOnly), "{op} is the person's alone");
    }
}

#[test]
fn a_phone_syncs_its_ledger_across_the_bus() {
    let engine = engine();
    let changes = json!([
        {"table": "accounts", "uid": "a1", "updated_at": 100, "row": {"name": "Chequing", "type": "CHEQUING", "openingBalance": 50_000, "archived": false, "sortOrder": 0}},
        {"table": "transactions", "uid": "t1", "updated_at": 100, "row": {"type": "INCOME", "amount": 210_000, "date": "2026-10-01", "account": "a1",
            "toAccount": null, "category": null, "note": "Pay", "recurring": null, "createdAt": 100}}
    ]);
    let first = ok(&engine, "money.sync", json!({"device": "Pixel 9", "replace": true, "since": 0, "changes": changes}));
    assert_eq!(first["applied"], 2);
    let home = ok(&engine, "money.summary", json!({"today": "2026-10-07"}));
    assert_eq!(home["income"], 210_000);
    assert_eq!(ok(&engine, "money.lists", json!({}))["devices"][0]["name"], "Pixel 9");
    ok(&engine, "money.tx.add", json!({"type": "EXPENSE", "amount": 1_500, "date": "2026-10-06", "account_id": home["accounts"][0]["id"]}));
    let next = ok(&engine, "money.sync", json!({"device": "Pixel 9", "since": first["cursor"], "generation": first["generation"], "changes": []}));
    // Erasing on the PC makes it another ledger: the phone is told, not merged into it.
    ok(&engine, "money.reset", json!({}));
    let stale = call_as(&engine, Actor::User, "money.sync", json!({"device": "Pixel 9", "since": next["cursor"], "generation": first["generation"], "changes": []}));
    assert_eq!(code(stale), "money.sync_stale");
    let back = next["changes"].as_array().unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0]["row"]["account"], "a1");
    // A phone's credential is the person's; an agent cannot sync a ledger over it.
    let agent = call_as(&engine, Actor::agent("brisk-otter"), "money.sync", json!({"device": "x", "since": 0, "changes": []}));
    assert!(agent.error.is_some());
}

#[test]
fn a_change_stamped_far_in_the_future_is_skipped_and_the_ledger_goes_on_syncing() {
    let engine = engine();
    let changes = json!([
        {"table": "accounts", "uid": "a1", "updated_at": 100, "row": {"name": "Chequing", "type": "CHEQUING", "openingBalance": 50_000, "archived": false, "sortOrder": 0}},
        {"table": "transactions", "uid": "t1", "updated_at": i64::MAX, "row": {"type": "INCOME", "amount": 210_000, "date": "2026-10-01", "account": "a1",
            "toAccount": null, "category": null, "note": "Pay", "recurring": null, "createdAt": 100}}
    ]);
    let first = ok(&engine, "money.sync", json!({"device": "Avex (S21)", "replace": true, "since": 0, "changes": changes}));
    assert_eq!((first["applied"].as_u64(), first["skipped"].as_u64()), (Some(1), Some(1)));
    let home = ok(&engine, "money.summary", json!({"today": "2026-10-07"}));
    ok(&engine, "money.tx.add", json!({"type": "EXPENSE", "amount": 1_500, "date": "2026-10-06", "account_id": home["accounts"][0]["id"]}));
    let next = ok(&engine, "money.sync", json!({"device": "Pixel 9", "since": 0, "changes": []}));
    assert!(next["changes"].as_array().unwrap().iter().all(|c| c["updated_at"].as_i64().unwrap() < i64::MAX / 2), "{next}");
}

#[test]
fn a_sync_answer_comes_in_pages_when_asked_for_them() {
    let engine = engine();
    ok(&engine, "money.sample", json!({}));
    let whole = ok(&engine, "money.sync", json!({"device": "Pixel 9", "since": 0, "changes": []}));
    assert!(whole.get("more").is_none(), "an answer asked for whole reads as it always did");
    let total = whole["changes"].as_array().unwrap().len();
    let (mut since, mut generation, mut taken, mut pages) = (0, serde_json::Value::Null, 0, 0);
    loop {
        let page = ok(&engine, "money.sync", json!({"device": "Pixel 9", "since": since, "generation": generation, "changes": [], "limit": 50}));
        let n = page["changes"].as_array().unwrap().len();
        assert!(n <= 50);
        taken += n;
        pages += 1;
        since = page["cursor"].as_i64().unwrap();
        generation = page["generation"].clone();
        if page.get("more").is_none() {
            break;
        }
        assert_eq!(page["more"], true);
    }
    assert_eq!(taken, total);
    assert!(pages > 1, "the sample is more than one page");
    let zero = call_as(&engine, Actor::User, "money.sync", json!({"device": "Pixel 9", "since": 0, "changes": [], "limit": 0}));
    assert_eq!(code(zero), "money.invalid");
}

#[test]
fn a_chart_asks_its_question_and_reads_the_ledger_now() {
    let engine = engine();
    let account = ok(&engine, "money.account.add", json!({"name": "Chequing", "type": "CHEQUING"}));
    let lists = ok(&engine, "money.lists", json!({}));
    let groceries = lists["categories"].as_array().unwrap().iter().find(|c| c["name"] == "Groceries").unwrap()["id"].clone();
    for (amount, date) in [(8_800, "2026-09-03"), (14_100, "2026-09-16"), (4_218, "2026-10-06")] {
        ok(&engine, "money.tx.add", json!({"type": "EXPENSE", "amount": amount, "date": date, "account_id": account["id"], "category_id": groceries}));
    }
    let weeks = ok(&engine, "money.series", json!({"by": "week", "periods": 2, "category": "groceries", "today": "2026-10-07"}));
    assert_eq!(weeks["labels"][0], "W1");
    assert_eq!(weeks["series"][0]["name"], "September");
    assert_eq!(weeks["series"][0]["values"], json!([8_800, 0, 14_100, 0, 0]));
    assert_eq!(weeks["series"][1]["values"][0], 4_218);
    let where_it_went = ok(&engine, "money.series", json!({"today": "2026-10-07"}));
    assert_eq!(where_it_went["labels"], json!(["Groceries"]));
    assert!(where_it_went["label_colors"][0].is_i64());
    assert_eq!(code(call_as(&engine, Actor::User, "money.series", json!({"category": "Yachts"}))), "money.invalid");
    assert_eq!(code(call_as(&engine, Actor::User, "money.series", json!({"by": "fortnight"}))), "bus.schema");
}

/// Wealthsimple's holdings report, as the shared tests have it (docs/INVESTMENTS.md, W1): a TFSA
/// holding Apple in US dollars, XEQT and ARKK, as of 2026-05-08.
const HOLDINGS_REPORT: &str = "Account Name,Account Type,Account Classification,Account Number,Symbol,Exchange,MIC,Name,Security Type,Quantity,Position Direction,Market Price,Market Price Currency,Book Value (CAD),Book Value Currency (CAD),Book Value (Market),Book Value Currency (Market),Market Value,Market Value Currency,Market Unrealized Returns,Market Unrealized Returns Currency
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"AAPL\",\"NASDAQ\",\"XNAS\",\"Apple Inc\",\"EQUITY\",\"10\",\"LONG\",\"100\",\"USD\",\"1000\",\"CAD\",\"750\",\"USD\",\"1000\",\"USD\",\"0\",\"USD\"
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"XEQT\",\"TSX\",\"XTSE\",\"iShares Core Equity ETF Portfolio\",\"EXCHANGE_TRADED_FUND\",\"10\",\"LONG\",\"25\",\"CAD\",\"250\",\"CAD\",\"250\",\"CAD\",\"250\",\"CAD\",\"0\",\"CAD\"
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"ARKK\",\"BATS\",\"BATS\",\"ARK Innovation ETF\",\"EXCHANGE_TRADED_FUND\",\"1\",\"LONG\",\"50\",\"USD\",\"50\",\"CAD\",\"50\",\"USD\",\"50\",\"USD\",\"0\",\"USD\"

\"As of 2026-05-08 12:00 GMT-04:00\"
";

/// One unit, at the 1e-8 the ledger keeps units in.
const UNIT: i64 = 100_000_000;

fn holdings_report(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("holdings-report-2026-05-08.csv");
    std::fs::write(&path, HOLDINGS_REPORT).unwrap();
    path
}

#[test]
fn a_wealthsimple_holdings_report_is_previewed_imported_once_and_read() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = holdings_report(dir.path());

    let preview = ok(&engine, "money.invest.preview", json!({"path": path}));
    assert_eq!((preview["kind"].as_str(), preview["as_of"].as_str()), (Some("holdings"), Some("2026-05-08")));
    assert_eq!((preview["holdings"].as_u64(), preview["new"].as_u64(), preview["duplicates"].as_u64()), (Some(3), Some(3), Some(0)));
    let account = &preview["accounts"][0];
    assert_eq!((account["number"].as_str(), account["name"].as_str(), account["registration"].as_str()), (Some("DEMO0001CAD"), Some("Demo TFSA"), Some("TFSA")));
    assert!(account["account_id"].is_null(), "no Tally account holds it yet");

    let done = ok(&engine, "money.invest.import", json!({"path": path, "accounts": []}));
    assert_eq!(done["kind"], "holdings");
    let counts: Vec<u64> = ["accounts_created", "securities", "holdings", "duplicates", "prices", "values"].iter().map(|k| done[*k].as_u64().unwrap()).collect();
    assert_eq!(counts, [1, 3, 3, 0, 3, 1]);

    // The summary reads what came in; the account's recorded value is its balance on Home.
    let portfolio = ok(&engine, "money.invest.summary", json!({"today": "2026-05-08"}));
    assert_eq!((portfolio["empty"].as_bool(), portfolio["as_of"].as_str(), portfolio["value"].as_i64()), (Some(false), Some("2026-05-08"), Some(163_333)));
    let held: Vec<(&str, i64, i64)> = portfolio["holdings"].as_array().unwrap().iter()
        .map(|h| (h["symbol"].as_str().unwrap(), h["quantity"].as_i64().unwrap(), h["book"].as_i64().unwrap()))
        .collect();
    assert_eq!(held, [("AAPL", 10 * UNIT, 100_000), ("XEQT", 10 * UNIT, 25_000), ("ARKK", UNIT, 5_000)]);
    assert_eq!(portfolio["holdings"][0]["fx_estimated"], true, "no USD rate yet: valued by its own book");
    assert_eq!((portfolio["accounts"][0]["registration"].as_str(), portfolio["accounts"][0]["institution"].as_str()), (Some("TFSA"), Some("Wealthsimple")));
    let lists = ok(&engine, "money.lists", json!({}));
    assert_eq!((lists["accounts"][0]["balance"].as_i64(), lists["accounts"][0]["registration"].as_str()), (Some(163_333), Some("TFSA")));

    // The same file again adds nothing, and its preview says so.
    let again = ok(&engine, "money.invest.import", json!({"path": path, "accounts": []}));
    assert_eq!((again["accounts_created"].as_u64(), again["holdings"].as_u64(), again["duplicates"].as_u64()), (Some(0), Some(0), Some(3)));
    let preview = ok(&engine, "money.invest.preview", json!({"path": path}));
    assert_eq!((preview["new"].as_u64(), preview["duplicates"].as_u64()), (Some(0), Some(3)));
    assert_eq!(preview["accounts"][0]["account_id"], lists["accounts"][0]["id"]);
    assert_eq!(ok(&engine, "money.invest.summary", json!({"today": "2026-05-08"}))["value"], 163_333);

    // A file is named by its full path, and must be a Wealthsimple investment file.
    assert_eq!(code(call_as(&engine, Actor::User, "money.invest.preview", json!({"path": "holdings.csv"}))), "money.path");
    let cash = dir.path().join("chequing.csv");
    std::fs::write(&cash, "date,transaction,description,amount,balance,currency\n2026-01-02,SPEND,Metro,-42.18,957.82,CAD\n").unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "money.invest.import", json!({"path": cash, "accounts": []}))), "money.invalid");
}

#[test]
fn a_wealthsimple_file_is_read_as_the_phone_reads_it_and_only_a_file() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    // Re-saved by a spreadsheet in Windows-1252, as the phone also reads it: "Non enregistré".
    let path = dir.path().join("holdings-1252.csv");
    let text = HOLDINGS_REPORT.replace("\"Demo TFSA\",\"TFSA\"", "\"Demo\",\"Non enregistr\u{e9}\"");
    let bytes: Vec<u8> = text.chars().map(|c| u8::try_from(u32::from(c)).unwrap()).collect();
    std::fs::write(&path, bytes).unwrap();
    let preview = ok(&engine, "money.invest.preview", json!({"path": path}));
    assert_eq!(preview["accounts"][0]["registration"], "NON_REGISTERED");

    let not_a_file = call_as(&engine, Actor::User, "money.invest.preview", json!({"path": dir.path()}));
    assert_eq!(code(not_a_file), "money.path");
    let big = dir.path().join("big.csv");
    std::fs::File::create(&big).unwrap().set_len((16 << 20) + 1).unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "money.invest.import", json!({"path": big, "accounts": []}))), "money.import");
    assert_eq!(ok(&engine, "money.invest.summary", json!({}))["empty"], true, "nothing came in");
}

#[test]
fn an_investment_activity_is_recorded_undone_and_restored() {
    let engine = engine();
    let tfsa = ok(&engine, "money.account.add", json!({"name": "TFSA", "type": "INVESTMENT", "registration": "TFSA", "institution": "Wealthsimple"}));
    assert_eq!((tfsa["registration"].as_str(), tfsa["institution"].as_str()), (Some("TFSA"), Some("Wealthsimple")));
    let refused = call_as(&engine, Actor::User, "money.account.add", json!({"name": "Chequing", "type": "CHEQUING", "registration": "TFSA"}));
    assert_eq!(code(refused), "money.invalid");

    // The thread's agent may record one; the card it draws undoes it with money.invest.delete.
    let deposit = ok(&engine, "money.invest.add", json!({"account_id": tfsa["id"], "type": "DEPOSIT", "date": "2026-08-01", "amount": 50_000}));
    let buy = call_as(&engine, Actor::agent("brisk-otter"), "money.invest.add", json!({
        "account_id": tfsa["id"], "type": "BUY", "date": "2026-08-08", "symbol": "xeqt", "quantity": 10 * UNIT, "amount": 38_120, "note": "Monthly"
    }));
    let buy = buy.into_result().unwrap();
    assert_eq!((buy["symbol"].as_str(), buy["account"].as_str(), buy["currency"].as_str(), buy["source"].as_str()), (Some("XEQT"), Some("TFSA"), Some("CAD"), Some("MANUAL")));
    let listed = ok(&engine, "money.invest.list", json!({}));
    let ids: Vec<&serde_json::Value> = listed["activities"].as_array().unwrap().iter().map(|a| &a["id"]).collect();
    assert_eq!(ids, [&buy["id"], &deposit["id"]], "newest first");
    assert_eq!(ok(&engine, "money.invest.list", json!({"type": "DIVIDEND"}))["activities"], json!([]));
    assert_eq!(ok(&engine, "money.invest.list", json!({"account_id": tfsa["id"], "since": "2026-08-05"}))["activities"].as_array().unwrap().len(), 1);
    let refused = call_as(&engine, Actor::User, "money.invest.add", json!({"account_id": tfsa["id"], "type": "SELL", "date": "2026-08-09", "quantity": UNIT, "amount": 3_900}));
    assert_eq!(code(refused), "money.invalid", "a sale names its security");

    let held = |engine: &relay_core::engine::Engine| ok(engine, "money.invest.summary", json!({"today": "2026-10-09"}))["holdings"].as_array().unwrap().len();
    assert_eq!(held(&engine), 1);
    ok(&engine, "money.invest.delete", json!({"id": buy["id"]}));
    assert_eq!(held(&engine), 0);
    assert_eq!(code(call_as(&engine, Actor::User, "money.invest.delete", json!({"id": buy["id"]}))), "money.not_found");
    let back = ok(&engine, "money.invest.restore", json!({"id": buy["id"]}));
    assert_eq!(back["id"], buy["id"]);
    assert_eq!(held(&engine), 1);

    // A price, the room and a recorded value move the reading.
    ok(&engine, "money.invest.price", json!({"symbol": "XEQT", "date": "2026-10-01", "price": 40 * UNIT}));
    ok(&engine, "money.invest.room", json!({"registration": "TFSA", "year": 2026, "amount": 700_000}));
    let portfolio = ok(&engine, "money.invest.summary", json!({"today": "2026-10-09"}));
    let xeqt = &portfolio["holdings"][0];
    assert_eq!((xeqt["value"].as_i64(), xeqt["book"].as_i64(), xeqt["gain"].as_i64()), (Some(40_000), Some(38_120), Some(1_880)));
    let room = portfolio["room"].as_array().unwrap().iter().find(|r| r["registration"] == "TFSA").unwrap();
    assert_eq!((room["room"].as_i64(), room["contributed"].as_i64(), room["left"].as_i64()), (Some(700_000), Some(50_000), Some(650_000)));
    assert_eq!(code(call_as(&engine, Actor::User, "money.invest.room", json!({"registration": "RESP", "year": 2026, "amount": 1}))), "money.invalid");
    ok(&engine, "money.fx.set", json!({"base": "usd", "quote": "CAD", "date": "2026-10-01", "rate": 137_000_000}));
    assert_eq!(code(call_as(&engine, Actor::User, "money.fx.set", json!({"base": "CAD", "quote": "CAD", "date": "2026-10-01", "rate": 1}))), "money.invalid");
    let valued = ok(&engine, "money.value.set", json!({"account_id": tfsa["id"], "date": "2026-10-09", "value": 52_000}));
    assert_eq!(valued["balance"], 52_000);

    let renamed = ok(&engine, "money.account.update", json!({"id": tfsa["id"], "name": "Retirement", "registration": "RRSP"}));
    assert_eq!((renamed["name"].as_str(), renamed["registration"].as_str(), renamed["institution"].as_str()), (Some("Retirement"), Some("RRSP"), Some("Wealthsimple")));
    let archived = ok(&engine, "money.account.update", json!({"id": tfsa["id"], "archived": true}));
    assert_eq!(archived["archived"], true);
    assert_eq!(code(call_as(&engine, Actor::User, "money.account.update", json!({"id": 99, "name": "Nowhere"}))), "money.not_found");
}

#[test]
fn files_and_the_network_are_the_persons_alone() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = holdings_report(dir.path());
    let agent = Actor::agent("brisk-otter");
    for (op, payload) in [
        ("money.invest.preview", json!({"path": path})),
        ("money.invest.import", json!({"path": path, "accounts": []})),
        ("money.fx.fetch", json!({})),
    ] {
        assert_eq!(code(call_as(&engine, agent.clone(), op, payload)), "actor.allowlist", "{op}");
    }
    assert_eq!(ok(&engine, "money.invest.summary", json!({}))["empty"], true, "nothing was imported");
    // A thread's agent acts as the person, so what keeps it from these is its tool list.
    for op in ["money.invest.summary", "money.invest.list", "money.invest.add"] {
        assert!(relay_core::threads::peer_may_call(op), "{op}");
    }
    for op in ["money.invest.import", "money.invest.preview", "money.invest.delete", "money.invest.room", "money.fx.fetch", "money.fx.set", "money.value.set"] {
        assert!(!relay_core::threads::peer_may_call(op), "{op}");
    }
}

#[test]
fn the_bank_of_canadas_rates_arrive_in_the_background() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    ok(&engine, "money.invest.import", json!({"path": holdings_report(dir.path()), "accounts": []}));
    assert_eq!(ok(&engine, "money.invest.summary", json!({"today": "2026-10-09"}))["holdings"][0]["fx_estimated"], true);

    // A stand-in for curl that answers Valet's question the way the Bank of Canada does.
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let curl = bin.join("curl");
    std::fs::write(&curl, r#"#!/bin/sh
case "$*" in
  *https://www.bankofcanada.ca/valet/observations/FXUSDCAD/json*)
    echo '{"observations":[{"d":"2026-10-07","FXUSDCAD":{"v":"1.3712"}},{"d":"2026-10-08","FXUSDCAD":{"v":"1.3725"}}]}';;
  *) echo "unexpected: $*" >&2; exit 22;;
esac
"#).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![bin.clone()];
    dirs.extend(std::env::split_paths(&path));
    std::env::set_var("PATH", std::env::join_paths(dirs).unwrap());

    let mut events = engine.subscribe();
    let started = ok(&engine, "money.fx.fetch", json!({}));
    std::env::set_var("PATH", path);
    assert_eq!(started["started"], true);
    let mut fetched = serde_json::Value::Null;
    wait_until("the rates", || {
        while let Ok(event) = events.try_recv() {
            if event.ev == "money.changed" && event.payload.get("fx").is_some() {
                fetched = event.payload;
            }
        }
        !fetched.is_null()
    });
    assert_eq!(fetched, json!({"fx": 2}));
    // The newest rate on or before today values Apple's US dollars: 1,000.00 USD at 1.3725.
    let apple = &ok(&engine, "money.invest.summary", json!({"today": "2026-10-09"}))["holdings"][0];
    assert_eq!((apple["symbol"].as_str(), apple["value"].as_i64(), apple["fx_estimated"].as_bool()), (Some("AAPL"), Some(137_250), Some(false)));
}

/// `money.import` and `money.export` touch the file before the store's transaction opens, and only
/// a regular one: a FIFO blocked the read (or the write) for good with every request queued
/// behind it, and `/dev/zero`, which says it is empty, read until memory ran out.
#[test]
fn a_backup_is_only_ever_a_regular_file() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe.json");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let answering = engine.clone();
    let pipe = fifo.clone();
    let dir_path = dir.path().to_path_buf();
    std::thread::spawn(move || {
        let calls = [
            ("money.import", json!({"path": pipe})),
            ("money.import", json!({"path": "/dev/zero"})),
            ("money.import", json!({"path": dir_path})),
            ("money.export", json!({"path": pipe})),
            ("money.export", json!({"path": dir_path})),
        ];
        let codes: Vec<String> = calls.into_iter().map(|(op, payload)| code(call_as(&answering, Actor::User, op, payload))).collect();
        let _ = tx.send(codes);
    });
    let codes = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("a pipe or a device held the request open");
    assert_eq!(codes, ["money.path"; 5]);
    use std::os::unix::fs::FileTypeExt;
    assert!(std::fs::metadata(&fifo).unwrap().file_type().is_fifo(), "the pipe was left as it was");
    // The bus is still answering, and a real backup still round-trips.
    let out = dir.path().join("backup.json");
    ok(&engine, "money.sample", json!({}));
    ok(&engine, "money.export", json!({"path": out}));
    let not_text = dir.path().join("latin1.json");
    std::fs::write(&not_text, b"{\"format\": \"caf\xe9\"}").unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "money.import", json!({"path": not_text}))), "money.import");
    assert_eq!(ok(&engine, "money.import", json!({"path": out}))["kind"], "backup");
}

/// A backup that is a symlink into a synced folder is written through the link: renamed over the
/// link, it became a plain local file and the synced copy stopped updating.
#[test]
fn an_export_writes_through_a_symlink_and_keeps_it() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let (sync, backups) = (dir.path().join("Sync"), dir.path().join("backups"));
    std::fs::create_dir(&sync).unwrap();
    std::fs::create_dir(&backups).unwrap();
    let (real, link) = (sync.join("tally.json"), backups.join("tally.json"));
    std::os::unix::fs::symlink("../Sync/tally.json", &link).unwrap();
    ok(&engine, "money.sample", json!({}));
    // A link to a file not made yet, then to last week's (0644).
    for _ in 0..2 {
        let exported = ok(&engine, "money.export", json!({"path": link}));
        assert_eq!(exported["path"], link.display().to_string());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link was replaced");
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&real).unwrap().permissions()) & 0o777, 0o600);
        assert_eq!(ok(&engine, "money.import", json!({"path": real}))["kind"], "backup");
        assert_eq!(std::fs::read_dir(&sync).unwrap().count(), 1, "a temporary file was left beside the export");
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
        std::fs::set_permissions(&real, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    }
    // A link to something that is not a file is still refused.
    let fifo = sync.join("pipe.json");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let to_pipe = backups.join("pipe.json");
    std::os::unix::fs::symlink(&fifo, &to_pipe).unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "money.export", json!({"path": to_pipe}))), "money.path");
}

/// A reading for another day reads that day; it posts bills only up to the real one, and a day,
/// an offset or an amount the calendar or the sums cannot hold is refused, never a panic.
#[test]
fn a_reading_for_a_later_day_posts_nothing_and_extremes_are_refused() {
    let engine = engine();
    ok(&engine, "money.sample", json!({}));
    let entries = |engine: &std::sync::Arc<relay_core::engine::Engine>| {
        ok(engine, "money.tx.list", json!({"period_offset": 0, "limit": 2000}))["transactions"].as_array().unwrap().len()
    };
    ok(&engine, "money.summary", json!({}));
    let posted = entries(&engine);
    let next_year = (jiff::Zoned::now().date().year() + 1).to_string() + "-06-01";
    ok(&engine, "money.summary", json!({"today": next_year}));
    assert_eq!(entries(&engine), posted, "a later day's reading posted its bills");
    for today in ["9999-12-20", "0001-01-01"] {
        assert_eq!(code(call_as(&engine, Actor::User, "money.summary", json!({"today": today}))), "money.invalid", "{today}");
    }
    for offset in [200_000, -200_000] {
        ok(&engine, "money.tx.list", json!({"period_offset": offset}));
        ok(&engine, "money.series", json!({"period_offset": offset}));
    }
    let account = ok(&engine, "money.lists", json!({}))["accounts"][0]["id"].clone();
    let huge = call_as(&engine, Actor::User, "money.tx.add", json!({"type": "EXPENSE", "amount": i64::MAX - 1, "date": "2026-10-05", "account_id": account}));
    assert_eq!(code(huge), "money.invalid");
    ok(&engine, "money.summary", json!({}));
}
