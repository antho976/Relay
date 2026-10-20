//! `design.picture.*` (pictures-SPEC-v2 §16): the Pictures plugin's ops through the bus, with a
//! fake standing in for the providers, so no test reaches the network: it answers from the saved
//! fixtures (`fixtures/pictures/`) unless a test queues another answer. The test instance keeps
//! the Pexels key in `pictures-pexels.key` beside its store, never in the desktop keyring.

mod common;

use common::{call, call_as, code, ok, ok_as, refused};
use relay_bus::Actor;
use relay_core::engine::Engine;
use relay_core::pictures::key::TEST_FILE;
use relay_core::pictures::net::{Fetch, Got, NetError, Req};
use relay_core::pictures::{saved_key, set_fake_fetch, SecretKey, AGENT_SEARCHES_PER_DAY};
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

const OPENVERSE_SEARCH: &[u8] = include_bytes!("fixtures/pictures/openverse-search.json");
const OPENVERSE_DETAIL: &[u8] = include_bytes!("fixtures/pictures/openverse-detail.json");
const PEXELS_SEARCH: &[u8] = include_bytes!("fixtures/pictures/pexels-search.json");
const PEXELS_PHOTO: &[u8] = include_bytes!("fixtures/pictures/pexels-photo.json");
const PHOTO: &[u8] = include_bytes!("fixtures/pictures/photo.png");
const VECTOR: &[u8] = include_bytes!("fixtures/pictures/vector.svg");
/// The Openverse photo `openverse-detail.json` describes, first in `openverse-search.json`.
const OPENVERSE_ID: &str = "openverse:48e4625b-0930-43db-990c-22858b1d633e";
/// The Pexels photo `pexels-photo.json` describes, first in `pexels-search.json`.
const PEXELS_ID: &str = "pexels:2014422";

/// Answers from a list, then as the providers would from the saved fixtures; notes every request.
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
    fn calls(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
    /// Answer the next request with `answer`.
    fn then(&self, answer: Result<Got, NetError>) {
        self.answers.lock().unwrap().push_back(answer);
    }
}

impl Fetch for Fake {
    fn get(&self, req: &Req) -> Result<Got, NetError> {
        self.seen.lock().unwrap().push(Seen { url: req.url.clone(), query: req.query.clone(), keyed: req.key.is_some() });
        self.answers.lock().unwrap().pop_front().unwrap_or_else(|| fixture(&req.url))
    }
}

/// What a provider answers at `url`, from the saved fixtures: a search, a photo's description,
/// any picture as `photo.png` (64 × 64); 404 for anything else.
fn fixture(url: &str) -> Result<Got, NetError> {
    let openverse = url.strip_prefix("https://api.openverse.org/v1/images/");
    let body = match (openverse, url) {
        (Some(""), _) => OPENVERSE_SEARCH,
        (Some(rest), _) if rest.ends_with("/thumb/") => PHOTO,
        (Some(_), _) => OPENVERSE_DETAIL,
        (None, "https://api.pexels.com/v1/search") => PEXELS_SEARCH,
        (None, url) if url.starts_with("https://api.pexels.com/v1/photos/") => PEXELS_PHOTO,
        (None, url) if url.starts_with("https://images.pexels.com/") => PHOTO,
        _ => return status(404),
    };
    Ok(Got { status: 200, retry_after: None, body: body.to_vec() })
}

fn status(status: u16) -> Result<Got, NetError> {
    Ok(Got { status, retry_after: None, body: br#"{"code":"Unauthorized"}"#.to_vec() })
}

fn picture(bytes: &[u8]) -> Result<Got, NetError> {
    Ok(Got { status: 200, retry_after: None, body: bytes.to_vec() })
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

// ------------------------------------------------------------------ search and thumb (C4)

/// An agent of project 1 under `root`, and a design of that project the person made.
fn designed(engine: &Engine, root: &Path) -> (Actor, i64) {
    let agent = agent(engine, root);
    let design = ok(engine, "design.create", json!({"project_id": 1, "title": "Kitchen"}))["design"]["id"].as_i64().unwrap();
    (agent, design)
}

fn pictures_on(engine: &Engine, on: bool) {
    ok(engine, "plugin.enable", json!({"plugin_id": "pictures", "project_id": 1, "enabled": on}));
}

fn search(design: i64, query: &str) -> Value {
    json!({"design_id": design, "query": query})
}

fn thumb(design: i64, id: &str) -> Value {
    json!({"design_id": design, "id": id})
}

/// The row bus.ops answers `actor` for `op`.
fn listed(engine: &Engine, actor: Actor, op: &str) -> Value {
    let ops = ok_as(engine, actor, "bus.ops", json!({}));
    ops["ops"].as_array().unwrap().iter().find(|row| row["name"] == op).cloned().unwrap_or_else(|| panic!("{op} is not listed"))
}

#[test]
fn pictures_are_off_until_the_person_turns_them_on() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (agent, design) = designed(&engine, root.path());
    for (op, payload) in [("design.picture.search", search(design, "kitchen")), ("design.picture.thumb", thumb(design, OPENVERSE_ID))] {
        let e = refused(call_as(&engine, agent.clone(), op, payload));
        assert_eq!((e.code.as_str(), e.details), ("plugin.off", Some(json!({"plugin": "pictures"}))), "{op}");
        assert_eq!(e.message, "Pictures is off for this project");
        assert!(e.hint.unwrap().contains("switch on Pictures"));
        let row = listed(&engine, agent.clone(), op);
        assert_eq!((row["call"].as_str(), row["why"].as_str()), (Some("no"), Some("plugin:pictures is off for this project")), "{op}");
    }
    assert_eq!(fake.calls(), 0, "nothing was asked of a provider");
    // The engine does not gate the person: only their own words leave, as when it is on.
    assert_eq!(ok(&engine, "design.picture.search", search(design, "kitchen"))["results"].as_array().unwrap().len(), 6);
    pictures_on(&engine, true);
    for op in ["design.picture.search", "design.picture.thumb"] {
        assert_eq!(listed(&engine, agent.clone(), op)["call"], "yes", "{op}");
    }
    let found = ok_as(&engine, agent.clone(), "design.picture.search", search(design, "kitchen table"));
    assert_eq!(found["source"], "openverse");
    let thumbed = ok_as(&engine, agent.clone(), "design.picture.thumb", thumb(design, OPENVERSE_ID));
    assert_eq!((thumbed["w"].as_u64(), thumbed["h"].as_u64()), (Some(64), Some(64)));
    pictures_on(&engine, false);
    let calls = fake.calls();
    assert_eq!(code(call_as(&engine, agent, "design.picture.search", search(design, "kitchen chair"))), "plugin.off", "at once");
    assert_eq!(fake.calls(), calls);
}

#[test]
fn search_answers_lean_licensed_results() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let found = ok(&engine, "design.picture.search", search(design, "kitchen"));
    assert_eq!((found["source"].as_str(), found["page"].as_u64(), found["has_more"].as_bool()), (Some("openverse"), Some(1), Some(true)));
    let results = found["results"].as_array().unwrap();
    let licences: Vec<&str> = results.iter().map(|hit| hit["licence"].as_str().unwrap()).collect();
    assert_eq!(licences, ["CC BY 2.0", "CC BY 2.0", "CC BY 2.0", "CC BY-SA 2.0", "CC BY-SA 2.0", "Public domain"],
        "the ND, NC, mature and sizeless results are left out");
    for hit in results {
        let mut keys: Vec<&str> = hit.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["creator", "h", "id", "licence", "title", "w"], "{hit}");
        assert!(hit["id"].as_str().unwrap().starts_with("openverse:"), "{hit}");
    }
    assert_eq!(results[0], json!({"id": OPENVERSE_ID, "title": "Designed by Williams Kitchen & Bath",
        "creator": results[0]["creator"], "licence": "CC BY 2.0", "w": 1024, "h": 768}));
    // Pexels, with the key the person saved: its results carry their source, and a photo whose
    // download is not on images.pexels.com is left out.
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    let found = ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen plants", "source": "pexels"}));
    assert_eq!((found["source"].as_str(), found["has_more"].as_bool()), (Some("pexels"), Some(true)));
    let ids: Vec<&str> = found["results"].as_array().unwrap().iter().map(|hit| hit["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [PEXELS_ID, "pexels:1080721", "pexels:2062426"]);
    assert!(found["results"].as_array().unwrap().iter().all(|hit| hit["licence"] == "Pexels License"));
}

#[test]
fn openverse_is_asked_politely() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let asked = json!({"design_id": design, "query": "  sunlit \t kitchen  ", "count": 50, "orientation": "landscape", "page": 2});
    ok(&engine, "design.picture.search", asked);
    assert_eq!(fake.seen(), [Seen {
        url: "https://api.openverse.org/v1/images/".into(),
        query: vec![("q", "sunlit kitchen".into()), ("license_type", "commercial,modification".into()), ("mature", "false".into()),
            ("page_size", "20".into()), ("page", "2".into()), ("aspect_ratio", "wide".into())],
        keyed: false,
    }]);
    let portrait = json!({"design_id": design, "query": "kitchen", "orientation": "portrait"});
    ok(&engine, "design.picture.search", portrait);
    assert_eq!(fake.seen()[1].query, vec![("q", "kitchen".to_string()), ("license_type", "commercial,modification".into()),
        ("mature", "false".into()), ("page_size", "12".into()), ("page", "1".into()), ("aspect_ratio", "tall".into())]);
    // Nothing is sent for a page past 10, or for words that cannot be sent.
    for (payload, want) in [
        (json!({"design_id": design, "query": "kitchen", "page": 11}), "design.picture.page"),
        (json!({"design_id": design, "query": "kitchen", "page": 0}), "design.picture.page"),
        (json!({"design_id": design, "query": "   "}), "design.picture.query"),
        (json!({"design_id": design, "query": "a".repeat(201)}), "design.picture.query"),
        (json!({"design_id": design, "query": "kitchen\u{7}"}), "design.picture.query"),
    ] {
        let e = refused(call(&engine, "design.picture.search", payload));
        assert_eq!(e.code, want);
        assert!(e.hint.is_some(), "{want}");
    }
    assert_eq!(fake.calls(), 2);
    // Page 10 is the last, whatever Openverse says it has (12 pages).
    let last = ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen", "page": 10}));
    assert_eq!((last["page"].as_u64(), last["has_more"].as_bool()), (Some(10), Some(false)));
}

#[test]
fn a_repeat_search_is_answered_from_memory() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (agent, design) = designed(&engine, root.path());
    pictures_on(&engine, true);
    let first = ok(&engine, "design.picture.search", search(design, "kitchen plants"));
    assert_eq!(ok(&engine, "design.picture.search", search(design, "  kitchen   plants ")), first, "the same words");
    assert_eq!(ok_as(&engine, agent.clone(), "design.picture.search", search(design, "kitchen plants")), first, "whoever asks");
    assert_eq!(fake.calls(), 1);
    ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen plants", "page": 2}));
    ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen plants", "count": 5}));
    ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen plants", "orientation": "square"}));
    assert_eq!(fake.calls(), 4, "another page, count or shape is another search");
    let five = ok(&engine, "design.picture.search", json!({"design_id": design, "query": "kitchen plants", "count": 5}));
    assert_eq!(five["results"].as_array().unwrap().len(), 5, "no more than asked for");
}

#[test]
fn offline_is_offline() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let a = ok(&engine, "design.picture.search", search(design, "kitchen"));
    fake.then(Err(NetError::Offline));
    assert_eq!(ok(&engine, "design.picture.search", search(design, "kitchen")), a, "asked within the hour");
    let e = refused(call(&engine, "design.picture.search", search(design, "garden")));
    assert_eq!((e.code.as_str(), e.details), ("design.picture.offline", Some(json!({"source": "openverse"}))));
    assert_eq!(e.hint.as_deref(), Some("draw it; photos already placed still work"));
    fake.then(Err(NetError::Timeout));
    let e = refused(call(&engine, "design.picture.search", search(design, "garden")));
    assert_eq!((e.code.as_str(), e.message.as_str()), ("design.picture.provider", "Openverse did not answer within 8 s"));
    assert_eq!(fake.calls(), 3);
    // A provider's answer Relay cannot read is the provider's fault, never echoed.
    fake.then(Ok(Got { status: 200, retry_after: None, body: b"<html>busy</html>".to_vec() }));
    let e = refused(call(&engine, "design.picture.search", search(design, "garden")));
    assert_eq!(e.code, "design.picture.provider");
    assert!(!e.message.contains("busy"), "{}", e.message);
}

#[test]
fn a_429_cools_the_source_without_waiting() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![Ok(Got { status: 429, retry_after: Some(30), body: Vec::new() })]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let started = std::time::Instant::now();
    let e = refused(call(&engine, "design.picture.search", search(design, "kitchen")));
    assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
    assert_eq!((e.code.as_str(), e.details), ("design.picture.rate_limited", Some(json!({"source": "openverse", "retry_after_s": 30}))));
    assert_eq!(e.hint.as_deref(), Some("use the other source, or draw it; never retry in a loop"));
    let again = refused(call(&engine, "design.picture.search", search(design, "garden")));
    assert_eq!(again.code, "design.picture.rate_limited");
    assert!(matches!(again.details.unwrap()["retry_after_s"].as_u64(), Some(29 | 30)));
    assert_eq!(fake.calls(), 1, "nothing is sent while it cools");
    // Previews spend Openverse's other limit, so they still come while searches cool.
    ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID));
    assert_eq!(fake.calls(), 2);

    // And a 429 on previews leaves searches alone; with no Retry-After it cools a minute.
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![Ok(Got { status: 429, retry_after: None, body: Vec::new() })]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let e = refused(call(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID)));
    assert_eq!((e.code.as_str(), e.details), ("design.picture.rate_limited", Some(json!({"source": "openverse", "retry_after_s": 60}))));
    assert_eq!(code(call(&engine, "design.picture.thumb", thumb(design, "openverse:6f31796e-019b-4888-948c-882a30a648dd"))),
        "design.picture.rate_limited");
    assert_eq!(fake.calls(), 1);
    ok(&engine, "design.picture.search", search(design, "kitchen"));
    assert_eq!(fake.calls(), 2);
}

#[test]
fn agents_share_a_daily_openverse_allowance() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (agent, design) = designed(&engine, root.path());
    pictures_on(&engine, true);
    // The person's searches are not counted.
    for i in 0..3 {
        ok(&engine, "design.picture.search", search(design, &format!("person {i}")));
    }
    for i in 0..AGENT_SEARCHES_PER_DAY {
        ok_as(&engine, agent.clone(), "design.picture.search", search(design, &format!("kitchen {i}")));
    }
    let calls = fake.calls();
    let e = refused(call_as(&engine, agent.clone(), "design.picture.search", search(design, "one more")));
    assert_eq!(e.code, "design.picture.rate_limited");
    let details = e.details.unwrap();
    assert_eq!(details["source"], "openverse");
    assert!(details["retry_after_s"].as_u64().unwrap() > 23 * 3600, "when the oldest leaves the day: {details}");
    assert!(e.message.contains("agents together"), "{}", e.message);
    assert_eq!(fake.calls(), calls, "nothing was sent");
    // What an agent asked within the hour is still answered (of the 64 searches kept), the
    // person still searches, and Pexels is not counted.
    ok_as(&engine, agent.clone(), "design.picture.search", search(design, &format!("kitchen {}", AGENT_SEARCHES_PER_DAY - 1)));
    ok(&engine, "design.picture.search", search(design, "person 4"));
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    ok_as(&engine, agent.clone(), "design.picture.search", json!({"design_id": design, "query": "one more", "source": "pexels"}));
    // Previews have their own allowance, so the agent may still look.
    ok_as(&engine, agent, "design.picture.thumb", thumb(design, OPENVERSE_ID));
}

#[test]
fn pexels_needs_the_saved_key() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let pexels = json!({"design_id": design, "query": "kitchen plants", "source": "pexels"});
    let e = refused(call(&engine, "design.picture.search", pexels.clone()));
    assert_eq!((e.code.as_str(), e.hint.as_deref()),
        ("design.picture.no_key", Some("use source openverse; the person can save a Pexels key in the Design view")));
    assert_eq!(code(call(&engine, "design.picture.thumb", thumb(design, PEXELS_ID))), "design.picture.no_key", "a detail read needs it");
    assert_eq!(fake.calls(), 0);
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    ok(&engine, "design.picture.search", pexels);
    let shown = ok(&engine, "design.picture.thumb", thumb(design, PEXELS_ID));
    assert!(shown["path"].as_str().unwrap().ends_with("/picture-cache/pexels-2014422.png"), "{shown}");
    let seen = fake.seen();
    assert_eq!(seen.len(), 3, "the key's check, the search and the preview: {seen:?}");
    assert_eq!(seen[1], Seen {
        url: "https://api.pexels.com/v1/search".into(),
        query: vec![("query", "kitchen plants".into()), ("per_page", "12".into()), ("page", "1".into())],
        keyed: true,
    });
    assert_eq!((seen[2].url.as_str(), seen[2].keyed),
        ("https://images.pexels.com/photos/2014422/pexels-photo-2014422.jpeg?auto=compress&cs=tinysrgb&h=350", false),
        "the search's own preview link, never with the key");
    // A Pexels 401 later says the key was turned down.
    fake.then(status(401));
    let e = refused(call(&engine, "design.picture.search", json!({"design_id": design, "query": "garden", "source": "pexels"})));
    assert_eq!(e.code, "design.picture.key_rejected");
}

/// An id that is no remembered hit (from before a restart) is described again by its provider,
/// through the same allowlist as a search.
#[test]
fn a_thumbnail_of_an_id_not_remembered_reads_it_again() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    ok(&engine, "design.picture.key_set", json!({"key": KEY}));
    ok(&engine, "design.picture.thumb", thumb(design, PEXELS_ID));
    let seen = fake.seen();
    assert_eq!(seen.iter().map(|s| (s.url.as_str(), s.keyed)).collect::<Vec<_>>(), [
        ("https://api.pexels.com/v1/search", true),
        ("https://api.pexels.com/v1/photos/2014422", true),
        ("https://images.pexels.com/photos/2014422/pexels-photo-2014422.jpeg?auto=compress&cs=tinysrgb&h=350", false),
    ]);
    // A description of another photo than the one asked for is not believed.
    let e = refused(call(&engine, "design.picture.thumb", thumb(design, "pexels:77")));
    assert_eq!(e.code, "design.picture.provider");
    // Openverse needs no description: the preview comes by the id.
    ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID));
    assert_eq!(fake.seen().last().unwrap().url, "https://api.openverse.org/v1/images/48e4625b-0930-43db-990c-22858b1d633e/thumb/");
}

#[test]
fn a_thumbnail_is_fetched_once() {
    use std::time::{Duration, SystemTime};
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let cache = root.path().join("store/picture-cache");
    std::fs::create_dir_all(&cache).unwrap();
    let aged = |name: &str, days: u64| {
        let path = cache.join(name);
        std::fs::write(&path, PHOTO).unwrap();
        std::fs::File::options().write(true).open(&path).unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(days * 24 * 3600)).unwrap();
        path
    };
    let stale = aged("openverse-6f31796e-019b-4888-948c-882a30a648dd.png", 8);
    let recent = aged("openverse-71e28cde-167a-4dfa-86c4-e2fd9959d446.png", 2);
    let shown = ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID));
    let path = PathBuf::from(shown["path"].as_str().unwrap());
    assert_eq!(path, cache.join("openverse-48e4625b-0930-43db-990c-22858b1d633e.png"));
    assert_eq!(std::fs::read(&path).unwrap(), PHOTO);
    assert_eq!((shown["w"].as_u64(), shown["h"].as_u64()), (Some(64), Some(64)));
    assert_eq!(fake.calls(), 1);
    assert!(!stale.exists() && recent.exists(), "the first fetch swept what nobody asked for in a week");
    // Asked again, it is the kept file, its time refreshed so the next sweep keeps it.
    let old = SystemTime::now() - Duration::from_secs(6 * 24 * 3600);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
    assert_eq!(ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID)), shown);
    assert_eq!(fake.calls(), 1, "no second request");
    assert!(std::fs::metadata(&path).unwrap().modified().unwrap() > old + Duration::from_secs(24 * 3600));
    // A recent preview kept before is a hit too; a corrupt one is fetched again.
    ok(&engine, "design.picture.thumb", thumb(design, "openverse:71e28cde-167a-4dfa-86c4-e2fd9959d446"));
    assert_eq!(fake.calls(), 1);
    std::fs::write(&path, b"\x89PNG\r\n\x1a\ncut short").unwrap();
    assert_eq!(ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID)), shown);
    assert_eq!((fake.calls(), std::fs::read(&path).unwrap().as_slice()), (2, PHOTO));
    // What is not a raster is not kept.
    let vector = "openverse:ca32bd72-5e2b-4949-b641-8a9a13a1b6ad";
    fake.then(picture(VECTOR));
    let e = refused(call(&engine, "design.picture.thumb", thumb(design, vector)));
    assert_eq!((e.code.as_str(), e.hint.as_deref()), ("design.picture.not_raster", Some("pick another result")));
    let kept: Vec<String> = std::fs::read_dir(&cache).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(!kept.iter().any(|name| name.contains("ca32bd72")), "{kept:?}");
    assert_eq!(kept.len(), 2, "no temporary file stays: {kept:?}");
}

#[test]
fn a_dead_photo_is_gone() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![status(424)]);
    let engine = engine_at(root.path(), fake.clone());
    let (_, design) = designed(&engine, root.path());
    let e = refused(call(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID)));
    assert_eq!((e.code.as_str(), e.details, e.hint.as_deref()),
        ("design.picture.gone", Some(json!({"source": "openverse"})), Some("pick another result")));
    ok(&engine, "design.picture.thumb", thumb(design, OPENVERSE_ID));
    assert_eq!(fake.calls(), 2, "no cooldown: the next preview is fetched");
    fake.then(status(404));
    assert_eq!(code(call(&engine, "design.picture.thumb", thumb(design, "openverse:6f31796e-019b-4888-948c-882a30a648dd"))), "design.picture.gone");
}

/// The design a picture is for is checked before anything is sent, as every design op checks it.
#[test]
fn a_picture_is_for_a_live_design_of_the_callers_project() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::answering(vec![]);
    let engine = engine_at(root.path(), fake.clone());
    let (agent, design) = designed(&engine, root.path());
    pictures_on(&engine, true);
    let other_repo = root.path().join("ws/other");
    common::committed_repo(&other_repo, &[("README.md", "# other\n")]);
    ok(&engine, "project.add", json!({"workspace_id": 1, "path": other_repo}));
    let theirs = ok(&engine, "design.create", json!({"project_id": 2, "title": "Other"}))["design"]["id"].as_i64().unwrap();
    let gone = ok(&engine, "design.create", json!({"project_id": 1, "title": "Gone"}))["design"]["id"].as_i64().unwrap();
    ok(&engine, "design.delete", json!({"design_id": gone}));
    for op in ["design.picture.search", "design.picture.thumb"] {
        let payload = |design: i64| if op.ends_with("search") { search(design, "kitchen") } else { thumb(design, OPENVERSE_ID) };
        assert_eq!(code(call(&engine, op, payload(999))), "design.not_found", "{op}");
        let e = refused(call(&engine, op, payload(gone)));
        assert_eq!((e.code.as_str(), e.hint.as_deref()), ("design.deleted", Some("design.restore brings it back")), "{op}");
        assert_eq!(code(call_as(&engine, agent.clone(), op, payload(theirs))), "actor.scope", "{op}");
        // The person may look for any project's design.
        ok(&engine, op, payload(theirs));
    }
    let calls = fake.calls();
    for bad in ["nope", "openverse:../../etc/passwd", "pexels:0", "http://example.com/a.png", "OPENVERSE:48e4625b-0930-43db-990c-22858b1d633e"] {
        let e = refused(call(&engine, "design.picture.thumb", thumb(design, bad)));
        assert_eq!((e.code.as_str(), e.hint.as_deref()), ("design.picture.id", Some("use an id from design.picture.search")), "{bad}");
    }
    assert_eq!(fake.calls(), calls, "nothing is sent for an id that is not one");
}

#[test]
fn a_slow_search_holds_no_lock() {
    struct Slow(Arc<std::sync::Barrier>);
    impl Fetch for Slow {
        fn get(&self, req: &Req) -> Result<Got, NetError> {
            self.0.wait();
            self.0.wait();
            fixture(&req.url)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let engine = engine_at(root.path(), Fake::answering(vec![]));
    let (_, design) = designed(&engine, root.path());
    set_fake_fetch(&engine, Arc::new(Slow(barrier.clone())));
    for (op, payload) in [("design.picture.search", search(design, "kitchen")), ("design.picture.thumb", thumb(design, OPENVERSE_ID))] {
        let asking = {
            let engine = engine.clone();
            std::thread::spawn(move || call(&engine, op, payload))
        };
        barrier.wait();
        let (tx, rx) = std::sync::mpsc::channel();
        let other = engine.clone();
        std::thread::spawn(move || {
            let _ = tx.send(call(&other, "design.create", json!({"project_id": 1, "title": "Meanwhile"})));
        });
        let created: Value = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("answered while the provider waits").into_result().unwrap();
        assert!(created["design"]["id"].is_i64(), "{created}");
        barrier.wait();
        assert!(asking.join().unwrap().into_result().is_ok(), "{op}");
    }
}

#[test]
fn the_test_instance_never_goes_online() {
    let root = tempfile::tempdir().unwrap();
    let engine = Engine::new(Instance::Test, Store::open(&root.path().join("store/store.db"), false).unwrap());
    let (_, design) = designed(&engine, root.path());
    let started = std::time::Instant::now();
    for (op, payload) in [("design.picture.search", search(design, "kitchen")), ("design.picture.thumb", thumb(design, OPENVERSE_ID))] {
        let e = refused(call(&engine, op, payload));
        assert_eq!((e.code.as_str(), e.message.as_str()), ("design.picture.offline", "Openverse could not be reached: the test instance never goes online"), "{op}");
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "at once: {:?}", started.elapsed());
}
