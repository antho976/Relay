//! Avex's training history across the bus: importing its export, the readings, what a wrong file
//! says, and what an agent may not do.

mod common;

use common::{call_as, code, engine, ok};
use relay_bus::Actor;
use serde_json::json;

const EXPORT: &str = include_str!("../../relay-gym/tests/avex_export.json");

#[test]
fn an_export_comes_in_and_reads_back() {
    let engine = engine();
    let empty = ok(&engine, "gym.summary", json!({}));
    assert!(empty["imported"].is_null(), "nothing imported yet");
    assert_eq!(empty["totals"]["sessions"], 0);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("avex_export.json");
    std::fs::write(&path, EXPORT).unwrap();
    let imported = ok(&engine, "gym.import", json!({"path": path}));
    assert_eq!(imported["exported_at"], "2026-10-08 21:14");
    let sessions = imported["sessions"].as_u64().unwrap();
    assert!(sessions > 40, "{imported}");

    let summary = ok(&engine, "gym.summary", json!({}));
    assert_eq!(summary["imported"]["file"], "avex_export.json");
    assert_eq!(summary["unit"], "lb");
    // One workout in the sample is untracked: listed, not counted.
    assert_eq!(summary["totals"]["sessions"].as_u64().unwrap(), sessions - 1);
    assert_eq!(summary["goals"].as_array().unwrap().len(), 1, "the met goal is not open");
    assert_eq!(summary["totals"]["untracked"], 1);
    assert_eq!(summary["goals"][0]["best"]["lift"], "Bench Press", "the goal's lift, by Avex's exercise id");
    // As of the export's day, its week has workouts in it, whatever today is.
    let then = ok(&engine, "gym.summary", json!({"today": "2026-10-08"}));
    assert_eq!(then["this_week"]["start"], "2026-10-05");
    assert!(then["this_week"]["sessions"].as_u64().unwrap() > 0, "{}", then["this_week"]);
    assert_eq!(code(call_as(&engine, Actor::User, "gym.summary", json!({"today": "Oct 8"}))), "gym.invalid");

    let page = ok(&engine, "gym.sessions", json!({"limit": 3}));
    assert_eq!(page["sessions"].as_array().unwrap().len(), 3);
    assert_eq!(page["total"].as_u64().unwrap(), sessions);
    let id = page["sessions"][0]["id"].as_i64().unwrap();
    let detail = ok(&engine, "gym.session.get", json!({"id": id}));
    assert_eq!(detail["row"]["id"], id);
    assert!(!detail["exercises"].as_array().unwrap().is_empty());
    // The newest workout has an older neighbour and none newer, and its lifts were done before.
    assert!(detail["older_id"].is_i64() && detail["newer_id"].is_null(), "{detail}");
    assert!(detail["exercises"].as_array().unwrap().iter().any(|e| e["previous"]["session_id"].is_i64()));

    let lifts = ok(&engine, "gym.lifts", json!({"query": "bench"}));
    assert_eq!(lifts["lifts"][0]["name"], "Bench Press");
    // Counted back from the export's day, the bench has both four-week spans to compare.
    let then = ok(&engine, "gym.lifts", json!({"query": "bench", "today": "2026-10-08"}));
    assert!(then["lifts"][0]["change_pct"].is_f64(), "{then}");
    assert!(ok(&engine, "gym.lift.get", json!({"name": "bench", "today": "2026-10-08"}))["row"]["change_pct"].is_f64());
    let bench = ok(&engine, "gym.lift.get", json!({"name": "bench press", "limit": 5}));
    assert_eq!(bench["history"].as_array().unwrap().len(), 5);
    assert_eq!(code(call_as(&engine, Actor::User, "gym.lift.get", json!({"name": "press"}))), "gym.not_found", "several lifts say press");

    let weekly = ok(&engine, "gym.series", json!({"measure": "sessions", "periods": 8}));
    assert_eq!(weekly["labels"].as_array().unwrap().len(), 8);
    assert_eq!(weekly["unit"], "sessions");
    let e1rm = ok(&engine, "gym.series", json!({"measure": "e1rm", "by": "session", "lift": "Back Squat", "periods": 52}));
    assert_eq!(e1rm["lift"], "Back Squat");
    assert_eq!(code(call_as(&engine, Actor::User, "gym.series", json!({"measure": "e1rm"}))), "gym.invalid");
    assert!(!ok(&engine, "gym.cardio", json!({}))["entries"].as_array().unwrap().is_empty());

    // Importing again replaces, never doubles.
    ok(&engine, "gym.import", json!({"path": path}));
    assert_eq!(ok(&engine, "gym.sessions", json!({"limit": 1}))["total"].as_u64().unwrap(), sessions);

    ok(&engine, "gym.reset", json!({}));
    assert!(ok(&engine, "gym.summary", json!({}))["imported"].is_null());
}

#[test]
fn a_wrong_file_says_which_one_to_send() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let weekly = dir.path().join("avex_weekly_export.json");
    std::fs::write(&weekly, r#"{"exportedAt": "x", "periodStart": "2026-10-05", "periodDays": 7, "sessions": [], "cardio": []}"#).unwrap();
    let r = call_as(&engine, Actor::User, "gym.import", json!({"path": weekly}));
    let e = r.error.expect("refused");
    assert_eq!(e.code, "gym.import");
    assert!(e.message.contains("Training history"), "{}", e.message);
    let tally = dir.path().join("tally.json");
    std::fs::write(&tally, r#"{"accounts": [], "transactions": []}"#).unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "gym.import", json!({"path": tally}))), "gym.import");
    assert_eq!(code(call_as(&engine, Actor::User, "gym.import", json!({"path": "avex_export.json"}))), "gym.path");
    assert!(ok(&engine, "gym.summary", json!({}))["imported"].is_null(), "nothing came in");
}

#[test]
fn an_upload_comes_in_as_text_and_goes_out_for_the_pc() {
    let engine = engine();
    assert_eq!(ok(&engine, "gym.raw", json!({})), json!({"digest": null, "json": null}), "nothing uploaded yet");

    let imported = ok(&engine, "gym.upload", json!({"json": EXPORT}));
    assert_eq!(imported["exported_at"], "2026-10-08 21:14");
    let summary = ok(&engine, "gym.summary", json!({}));
    assert_eq!(summary["imported"]["file"], "avex_export.json");

    // The PC's link takes the text as it came, once.
    let raw = ok(&engine, "gym.raw", json!({}));
    assert_eq!(raw["json"], EXPORT);
    let digest = raw["digest"].as_str().unwrap().to_string();
    assert_eq!(digest.len(), 64);
    let again = ok(&engine, "gym.raw", json!({"have": digest}));
    assert!(again["json"].is_null(), "the caller already holds it");
    assert_eq!(again["digest"], digest.as_str());

    // A wrong file is refused as an import of it would be, and changes nothing.
    assert_eq!(code(call_as(&engine, Actor::User, "gym.upload", json!({"json": "{}"}))), "gym.import");
    assert_eq!(ok(&engine, "gym.raw", json!({"have": digest}))["digest"], digest.as_str());

    // Forgetting the copy forgets the upload too.
    ok(&engine, "gym.reset", json!({}));
    assert!(ok(&engine, "gym.raw", json!({}))["digest"].is_null());
}

#[test]
fn an_agent_reads_but_never_imports_or_forgets() {
    let engine = engine();
    let agent = Actor::agent("brisk-otter");
    assert!(call_as(&engine, agent.clone(), "gym.summary", json!({})).error.is_none());
    for (op, payload) in [
        ("gym.import", json!({"path": "/tmp/avex_export.json"})),
        ("gym.upload", json!({"json": EXPORT})),
        ("gym.raw", json!({})),
        ("gym.reset", json!({})),
    ] {
        assert!(call_as(&engine, agent.clone(), op, payload).error.is_some(), "{op} must refuse an agent");
    }
}

/// `/dev/zero` has a length of 0 and no end, and a FIFO never answers: an import of either is
/// refused at once instead of reading until the engine is out of memory or holding the request
/// for good, as money.import does (`a_backup_is_only_ever_a_regular_file`).
#[test]
fn an_export_is_only_ever_a_regular_file() {
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe.json");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let answering = engine.clone();
    let (pipe, folder) = (fifo.clone(), dir.path().to_path_buf());
    std::thread::spawn(move || {
        let codes: Vec<String> = [json!({"path": pipe}), json!({"path": "/dev/zero"}), json!({"path": folder})]
            .into_iter()
            .map(|payload| code(call_as(&answering, Actor::User, "gym.import", payload)))
            .collect();
        let _ = tx.send(codes);
    });
    let codes = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("a pipe or a device held the import open");
    assert_eq!(codes, ["gym.path"; 3]);
    let not_text = dir.path().join("avex.json");
    std::fs::write(&not_text, b"{\"x\": \"caf\xe9\"}").unwrap();
    assert_eq!(code(call_as(&engine, Actor::User, "gym.import", json!({"path": not_text}))), "gym.import");
}
