//! `design.picture.*` (pictures-SPEC-v2 §16): the Pictures plugin's ops through the bus, with a
//! fake standing in for the providers, so no test reaches the network. The test instance keeps
//! the Pexels key in `pictures-pexels.key` beside its store, never in the desktop keyring.

mod common;

use common::{call, call_as, code, ok, ok_as, refused};
use relay_bus::Actor;
use relay_core::engine::Engine;
use relay_core::pictures::key::TEST_FILE;
use relay_core::pictures::net::{Fetch, Got, NetError, Req};
use relay_core::pictures::{saved_key, set_fake_fetch, SecretKey};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A Pexels key as the person would paste it; distinctive enough to find in any byte.
const KEY: &str = "PxL5u3Qk7wRvT0yZb8mN2cJ4hG6fD1sA9eW3rT5yU7iO0pL2kJ4hG6fD8s";

/// One request the fake was sent: what it asked and whether it carried the key (never the key).
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    url: String,
    query: Vec<(&'static str, String)>,
    keyed: bool,
}

/// Answers from a list, then 200s with an empty JSON object; notes every request.
#[derive(Default)]
struct Fake {
    answers: Mutex<VecDeque<Result<Got, NetError>>>,
    seen: Mutex<Vec<Seen>>,
}

impl Fake {
    fn answering(answers: Vec<Result<Got, NetError>>) -> Arc<Fake> {
        Arc::new(Fake { answers: Mutex::new(answers.into()), seen: Mutex::default() })
    }
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Fetch for Fake {
    fn get(&self, req: &Req) -> Result<Got, NetError> {
        self.seen.lock().unwrap().push(Seen { url: req.url.clone(), query: req.query.clone(), keyed: req.key.is_some() });
        self.answers.lock().unwrap().pop_front().unwrap_or(Ok(Got { status: 200, retry_after: None, body: b"{}".to_vec() }))
    }
}

fn status(status: u16) -> Result<Got, NetError> {
    Ok(Got { status, retry_after: None, body: br#"{"code":"Unauthorized"}"#.to_vec() })
}

/// A test-instance engine on a store under `root`, with `fake` for the providers.
fn engine_at(root: &Path, fake: Arc<Fake>) -> Arc<Engine> {
    let engine = Engine::new(Instance::Test, Store::open(&root.join("store/store.db"), false).unwrap());
    set_fake_fetch(&engine, fake);
    engine
}

fn key_file(root: &Path) -> PathBuf {
    root.join("store").join(TEST_FILE)
}

/// An agent session in project 1 of a fresh workspace under `root`.
fn agent(engine: &Engine, root: &Path) -> Actor {
    let ws = root.join("ws");
    let repo = ws.join("app");
    common::committed_repo(&repo, &[("README.md", "# app\n")]);
    ok(engine, "workspace.create", json!({"path": ws}));
    ok(engine, "project.add", json!({"workspace_id": 1, "path": repo}));
    let name = ok(engine, "session.create", json!({"project_id": 1, "provider": "claude"}))["name"].as_str().unwrap().to_string();
    Actor::agent(&name)
}

#[test]
fn the_pexels_key_is_the_persons_alone() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let agent = agent(&engine, root.path());
    assert_eq!(code(call_as(&engine, agent.clone(), "design.picture.key_set", json!({"key": KEY}))), "actor.allowlist");
    assert_eq!(code(call_as(&engine, agent.clone(), "design.picture.key_remove", json!({}))), "actor.allowlist");
    assert!(fake.seen().is_empty() && !key_file(root.path()).exists());
    let listed = ok_as(&engine, agent, "bus.ops", json!({}));
    let names: Vec<&str> = listed["ops"].as_array().unwrap().iter().filter_map(|o| o["name"].as_str()).collect();
    assert!(names.contains(&"design.apply"), "the agent's list is the real one");
    assert!(!names.iter().any(|n| n.starts_with("design.picture.key")), "{names:?}");
}

#[test]
fn a_key_is_checked_then_kept() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![status(200)]);
    let engine = engine_at(root.path(), fake.clone());
    assert_eq!(ok(&engine, "design.picture.key_set", json!({"key": KEY})), json!({"saved": true}));
    let file = key_file(root.path());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), KEY);
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fake.seen(), [Seen {
        url: "https://api.pexels.com/v1/search".into(),
        query: vec![("query", "nature".into()), ("per_page", "1".into())],
        keyed: true,
    }]);
    assert_eq!(saved_key(&engine, true).unwrap(), SecretKey::new(KEY), "this engine's key at once, for agents too");
    // The surrounding whitespace of a paste is not part of it.
    let fake = Fake::answering(vec![status(200)]);
    let again = engine_at(root.path(), fake);
    ok(&again, "design.picture.key_set", json!({"key": format!("  {KEY}\n")}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), KEY);
}

#[test]
fn a_saved_key_is_read_again_after_a_restart() {
    let root = tempfile::tempdir().unwrap();
    ok(&engine_at(root.path(), Fake::answering(vec![])), "design.picture.key_set", json!({"key": KEY}));
    let restarted = engine_at(root.path(), Fake::answering(vec![]));
    assert_eq!(saved_key(&restarted, true).unwrap(), SecretKey::new(KEY));
}

#[test]
fn a_rejected_key_is_not_kept() {
    let root = tempfile::tempdir().unwrap();
    for answer in [401, 403] {
        let fake = Fake::answering(vec![status(answer)]);
        let engine = engine_at(root.path(), fake.clone());
        let e = refused(call(&engine, "design.picture.key_set", json!({"key": KEY})));
        assert_eq!(e.code, "design.picture.key_rejected", "{answer}");
        assert_eq!(fake.seen().len(), 1);
        assert!(!key_file(root.path()).exists(), "{answer}: nothing kept");
        assert_eq!(saved_key(&engine, false).unwrap_err().code, "design.picture.no_key");
    }
    // A key saved before stays when a new one is turned down.
    let engine = engine_at(root.path(), Fake::answering(vec![status(200), status(401)]));
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    assert_eq!(code(call(&engine, "design.picture.key_set", json!({"key": "another-key"}))), "design.picture.key_rejected");
    assert_eq!(std::fs::read_to_string(key_file(root.path())).unwrap(), KEY);
    assert_eq!(saved_key(&engine, false).unwrap(), SecretKey::new(KEY));
}

#[test]
fn a_key_pexels_cannot_be_asked_about_is_not_kept() {
    let root = tempfile::tempdir().unwrap();
    for (answer, want) in [(Err(NetError::Offline), "design.picture.offline"), (status(429), "design.picture.rate_limited"),
        (status(500), "design.picture.provider")] {
        let fake = Fake::answering(vec![answer]);
        let engine = engine_at(root.path(), fake);
        assert_eq!(code(call(&engine, "design.picture.key_set", json!({"key": KEY}))), want);
        assert!(!key_file(root.path()).exists(), "{want}");
    }
    // With no fake, the test instance never goes online.
    let bare = Engine::new(Instance::Test, Store::open(&root.path().join("bare/store.db"), false).unwrap());
    assert_eq!(code(call(&bare, "design.picture.key_set", json!({"key": KEY}))), "design.picture.offline");
}

/// Pexels counts each key apart: the saved key's cooldown holds that key back, never the check of
/// a new one, and a new key saved starts with no cooldown.
#[test]
fn a_new_key_is_checked_while_the_old_one_cools() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![status(200), status(429)]);
    let engine = engine_at(root.path(), fake.clone());
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    let e = refused(call(&engine, "design.picture.key_set", json!({"key": KEY})));
    assert_eq!((e.code.as_str(), e.details), ("design.picture.rate_limited", Some(json!({"source": "pexels", "retry_after_s": 60}))));
    assert_eq!(code(call(&engine, "design.picture.key_set", json!({"key": KEY}))), "design.picture.rate_limited");
    assert_eq!(fake.seen().len(), 2, "the same key waits its cooldown out, with nothing sent");
    assert_eq!(std::fs::read_to_string(key_file(root.path())).unwrap(), KEY, "and stays saved");
    assert_eq!(ok(&engine, "design.picture.key_set", json!({"key": "another-key"})), json!({"saved": true}));
    assert_eq!(fake.seen().len(), 3, "another key is asked about at once");
    ok(&engine, "design.picture.key_set", json!({"key": "another-key"}));
    assert_eq!(fake.seen().len(), 4, "and once saved, it has no cooldown");
    assert_eq!(saved_key(&engine, true).unwrap(), SecretKey::new("another-key"));
}

#[test]
fn a_malformed_key_is_refused_before_any_request() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    for bad in ["two words", "", "   ", "tab\there", "nul\u{0}key", &"k".repeat(201), "clé"] {
        let e = refused(call(&engine, "design.picture.key_set", json!({"key": bad})));
        assert_eq!(e.code, "design.picture.key_format", "{bad:?}");
        assert!(bad.trim().is_empty() || !e.message.contains(bad.trim()), "the key is never quoted: {}", e.message);
    }
    assert!(fake.seen().is_empty(), "nothing was sent");
    assert!(!key_file(root.path()).exists());
}

#[test]
fn an_empty_payload_forgets_nothing() {
    let root = tempfile::tempdir().unwrap();
    let engine = engine_at(root.path(), Fake::answering(vec![]));
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    assert_eq!(code(call(&engine, "design.picture.key_set", json!({}))), "bus.schema");
    assert_eq!(code(call(&engine, "design.picture.key_set", json!({"key": null}))), "bus.schema");
    assert_eq!(code(call(&engine, "design.picture.key_remove", json!({"key": KEY}))), "bus.schema", "remove takes nothing");
    assert_eq!(std::fs::read_to_string(key_file(root.path())).unwrap(), KEY);
    assert_eq!(saved_key(&engine, false).unwrap(), SecretKey::new(KEY));
}

#[test]
fn key_remove_forgets() {
    let root = tempfile::tempdir().unwrap();
    let engine = engine_at(root.path(), Fake::answering(vec![]));
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    assert_eq!(ok(&engine, "design.picture.key_remove", json!({})), json!({"saved": false}));
    assert!(!key_file(root.path()).exists());
    assert_eq!(saved_key(&engine, false).unwrap_err().code, "design.picture.no_key", "this engine forgot it");
    assert_eq!(saved_key(&engine_at(root.path(), Fake::answering(vec![])), false).unwrap_err().code, "design.picture.no_key",
        "and so does the next");
    assert_eq!(ok(&engine, "design.picture.key_remove", json!({})), json!({"saved": false}), "nothing saved is no error");
    // key_remove is audited, with nothing in it; key_set never is.
    let rows = ok(&engine, "audit.list", json!({"op_prefix": "design.picture", "limit": 10}));
    let ops: Vec<&str> = rows["rows"].as_array().unwrap().iter().filter_map(|r| r["op"].as_str()).collect();
    assert_eq!(ops, ["design.picture.key_remove", "design.picture.key_remove"], "{rows}");
}

#[test]
fn the_key_never_leaves() {
    let root = tempfile::tempdir().unwrap();
    let engine = engine_at(root.path(), Fake::answering(vec![status(200), status(401)]));
    let mut events = engine.subscribe();
    let mut said = Vec::new();
    for (op, payload) in [
        ("design.picture.key_set", json!({"key": KEY})),
        ("design.picture.key_set", json!({"key": format!("{KEY} {KEY}")})),
        ("design.picture.key_set", json!({"key": format!("{KEY}x")})),
        ("design.picture.key_set", json!({"key": KEY, "extra": 1})),
        ("design.picture.key_remove", json!({})),
        ("design.picture.key_set", json!({"key": KEY})),
    ] {
        said.push(serde_json::to_string(&call(&engine, op, payload)).unwrap());
    }
    let audit = ok(&engine, "audit.list", json!({"limit": 200}));
    said.push(audit.to_string());
    while let Ok(event) = events.try_recv() {
        said.push(serde_json::to_string(&event).unwrap());
    }
    for text in &said {
        assert!(!text.contains(KEY), "{text}");
    }
    let store = root.path().join("store");
    for file in ["store.db", "store.db-wal", "store.db-shm"] {
        let bytes = std::fs::read(store.join(file)).unwrap_or_default();
        assert!(!bytes.windows(KEY.len()).any(|w| w == KEY.as_bytes()), "{file}");
    }
    assert_eq!(std::fs::read_to_string(key_file(root.path())).unwrap(), KEY, "kept only where it belongs");
}

#[test]
fn an_in_memory_store_keeps_no_key() {
    let fake = Fake::answering(vec![]);
    let engine = Engine::new(Instance::Test, Store::open_memory().unwrap());
    set_fake_fetch(&engine, fake.clone());
    let e = refused(call(&engine, "design.picture.key_set", json!({"key": KEY})));
    assert_eq!((e.code.as_str(), e.message.as_str()), ("design.picture.keyring", "this store keeps no keys"));
    assert!(fake.seen().is_empty(), "nothing is sent for a key that cannot be kept");
    assert_eq!(ok(&engine, "design.picture.key_remove", json!({})), json!({"saved": false}));
    assert_eq!(saved_key(&engine, false).unwrap_err().code, "design.picture.no_key");
}

/// Neither op holds the store while it asks Pexels or the keyring: another request runs to its
/// end while the check is still waiting on the provider.
#[test]
fn a_slow_key_check_holds_no_lock() {
    struct Slow(Arc<std::sync::Barrier>);
    impl Fetch for Slow {
        fn get(&self, _: &Req) -> Result<Got, NetError> {
            self.0.wait();
            self.0.wait();
            Ok(Got { status: 200, retry_after: None, body: b"{}".to_vec() })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let engine = Engine::new(Instance::Test, Store::open(&root.path().join("store/store.db"), false).unwrap());
    set_fake_fetch(&engine, Arc::new(Slow(barrier.clone())));
    let saving = {
        let engine = engine.clone();
        std::thread::spawn(move || call(&engine, "design.picture.key_set", json!({"key": KEY})))
    };
    barrier.wait();
    // A mutation, so it needs the store in a transaction of its own.
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let other = engine.clone();
    std::thread::spawn(move || {
        let _ = tx.send(call(&other, "workspace.create", json!({"path": ws})));
    });
    let created: Value = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("answered while the check waits").into_result().unwrap();
    assert!(created["id"].is_i64(), "{created}");
    barrier.wait();
    assert_eq!(saving.join().unwrap().into_result().unwrap(), json!({"saved": true}));
}
