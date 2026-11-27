//! The link against a real door, as the Kotlin app's LiveDoorTest: pair with a code, connect with
//! no Connect key, ask the engine something. Skipped unless `RELAY_E2E_LINK` holds a pairing link
//! from a throwaway engine. `tools/live-check.sh` sets everything up and runs it (README.md,
//! "Testing against a real door"); with what it sets, it goes on to the ops the app is new to:
//!
//! - `RELAY_E2E_URL`: the door to dial, in place of the link's own addresses (a door on loopback);
//! - `RELAY_E2E_PROJECT`: a project to open a shell in, for `chat.send` (once, and once again with
//!   the same id) and `session.transcript` with `before`;
//! - `RELAY_E2E_BUILD`: the build of a fake APK published for `phone.app.latest` and `phone.app.apk`.
//!
//! A pairing code is spent once, so it is one test.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rc_link::{FileStore, Link, LinkState, PairLink, Store};
use serde_json::json;

fn wait_for(link: &Link, what: &str, timeout: Duration, ok: impl Fn(&LinkState) -> bool) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if ok(&link.state()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{what}: still {:?} after {timeout:?}", link.state());
}

#[test]
fn pairs_then_connects_by_itself_and_answers() {
    let Ok(text) = std::env::var("RELAY_E2E_LINK") else {
        eprintln!("RELAY_E2E_LINK not set: skipped");
        return;
    };
    let mut pl = PairLink::parse(&text).expect("a pairing link");
    if let Ok(url) = std::env::var("RELAY_E2E_URL") {
        pl.direct = vec![url];
    }
    let dir = std::env::temp_dir().join(format!("rc-link-e2e-{}", std::process::id()));
    let store: Arc<dyn Store> = Arc::new(FileStore(dir.join("pc.json")));
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = wakes.clone();
    let link = Link::start(store.clone(), Arc::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(link.state(), LinkState::Unpaired);

    let profile = rc_link::link::pair(&pl, "rc-link e2e").expect("paired");
    assert!(!profile.token.is_empty());
    // Pairing is the last thing the person does: the link connects with no other step.
    link.paired(&profile);
    wait_for(&link, "online after pairing", Duration::from_secs(60), LinkState::online);
    assert!(wakes.load(Ordering::SeqCst) > 0, "the app was woken for the change");

    let answer = link.call("session.list", json!({})).wait(Duration::from_secs(30)).expect("session.list");
    assert!(answer.get("sessions").is_some_and(|s| s.is_array()), "{answer}");

    // A second start (the app opened again) connects from the stored profile alone.
    let again = Link::start(store, Arc::new(|| {}));
    wait_for(&again, "online on the next start", Duration::from_secs(30), LinkState::online);

    if let Ok(project) = std::env::var("RELAY_E2E_PROJECT") {
        chat_and_paging(&link, project.parse().expect("a project id"));
    }
    if let Ok(build) = std::env::var("RELAY_E2E_BUILD") {
        app_update(&link, build.parse().expect("a build number"));
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn ask(link: &Link, op: &str, payload: serde_json::Value) -> serde_json::Value {
    link.call(op, payload).wait(Duration::from_secs(30)).unwrap_or_else(|e| panic!("{op}: {} {}", e.code, e.message))
}

/// A shell on the PC: two messages with chat.send, the first sent twice under one id, typed once
/// each; then a page back of its transcript.
fn chat_and_paging(link: &Link, project: i64) {
    let shell = ask(link, "session.create", json!({"project_id": project, "provider": "shell"}))["name"].as_str().expect("a name").to_string();
    ask(link, "session.spawn", json!({"session": shell}));
    let send = |id: &str, text: &str| ask(link, "chat.send", json!({"session": shell, "text": text, "client_id": id}))["typed"].clone();
    assert_eq!(send("live-1", "echo live-$((40+2))"), json!(true));
    assert_eq!(send("live-1", "echo live-$((40+2))"), json!(false), "the same id again types nothing");
    assert_eq!(send("live-2", "echo second-$((1+1))"), json!(true));
    let start = Instant::now();
    let shown = loop {
        let text = ask(link, "session.scrollback", json!({"session": shell}))["text"].as_str().unwrap_or_default().to_string();
        if text.contains("second-2") || start.elapsed() > Duration::from_secs(15) {
            break text;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!((shown.matches("live-42").count(), shown.matches("second-2").count()), (1, 1), "each typed once:\n{shown}");
    // A shell keeps no conversation: the answer says so, in the shape that pages back.
    let page = ask(link, "session.transcript", json!({"session": shell, "before": 0, "limit": 20}));
    assert_eq!((page["found"].as_bool(), page["older"].as_bool(), page["pending"].as_array().map(Vec::len)), (Some(false), Some(false), Some(0)), "{page}");
    ask(link, "session.close", json!({"session": shell}));
    eprintln!("chat.send and paging: ok");
}

/// The fake APK the script published: its build, and its bytes in one piece matching the hash.
fn app_update(link: &Link, build: u64) {
    use sha2::Digest;
    let latest = ask(link, "phone.app.latest", json!({}));
    assert_eq!(latest["build"].as_u64(), Some(build), "{latest}");
    let piece = ask(link, "phone.app.apk", json!({"offset": 0}));
    let bytes = unbase64(piece["bytes_b64"].as_str().expect("bytes"));
    assert_eq!((piece["eof"].as_bool(), Some(bytes.len() as u64)), (Some(true), latest["size"].as_u64()));
    let sha: String = sha2::Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(Some(sha.as_str()), latest["sha256"].as_str());
    eprintln!("phone.app.latest and phone.app.apk: ok ({} bytes)", bytes.len());
}

fn unbase64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let clean: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, c)| n | (u32::from(val(*c)) << (18 - 6 * i)));
        let take = chunk.len() - 1;
        out.extend_from_slice(&n.to_be_bytes()[1..1 + take]);
    }
    out
}

#[test]
fn base64_reads_as_the_engine_writes_it() {
    assert_eq!(unbase64("aGVsbG8="), b"hello");
    assert_eq!(unbase64("aGk="), b"hi");
    assert_eq!(unbase64("YQ=="), b"a");
    assert_eq!(unbase64("+/8A"), [0xfb, 0xff, 0x00]);
}
