//! The link against a real door, as the Kotlin app's LiveDoorTest: pair with a code, connect with
//! no Connect key, ask the engine something. Skipped unless `RELAY_E2E_LINK` holds a pairing link
//! from a throwaway engine (README.md, "Testing against a real door"):
//!
//! ```fish
//! relay --instance test remote serve --bind 127.0.0.1:7431 --start-engine &
//! set -x RELAY_E2E_LINK (relay --instance test remote pair --no-confirm | string match -r 'relay://pair\S+')
//! cargo test -p rc-link --test live_door -- --nocapture
//! ```

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
    let pl = PairLink::parse(&text).expect("a pairing link");
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
    let _ = std::fs::remove_dir_all(dir);
}
