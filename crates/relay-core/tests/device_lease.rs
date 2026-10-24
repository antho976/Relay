//! Device leases: sessions sharing one phone see who is using it, and a conflicting install is
//! refused with `device.busy` instead of clobbering the other session's build.

mod common;

use common::{call_as as call, committed_repo, engine_with_project, ok_as as ok, refused as refusal};
use relay_bus::{Actor, Response};
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let ws = base.join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "hi\n")]);
    let adb = base.join("adb");
    std::fs::write(&adb, "#!/bin/sh\nif [ \"$1\" = devices ]; then printf 'List of devices attached\\nrelay-phone device product:relay model:Pixel_9 device:relay transport_id:1\\n'; fi\nexit 0\n").unwrap();
    std::fs::set_permissions(&adb, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engine = engine_with_project(&base, &ws, &repo);
    ok(&engine, Actor::User, "settings.set", json!({"path": "device.adb_path", "value": adb}));
    // A run whose "install" outlives the test's assertions; stopped explicitly below.
    ok(&engine, Actor::User, "project.update", json!({"project_id": 1, "run_cmd": "sleep 30"}));
    Fixture { _root: root, engine }
}

fn session(engine: &Engine) -> (String, String) {
    let created = ok(engine, Actor::User, "session.create", json!({"project_id": 1, "provider": "claude"}));
    (created["name"].as_str().unwrap().to_string(), created["worktree"].as_str().unwrap().to_string())
}

fn gate(engine: &Engine, session: &str, command: &str) -> Response {
    call(engine, Actor::agent(session), "guardrail.gate", json!({"session": session, "kind": "exec", "command": command}))
}

fn released(events: &mut tokio::sync::broadcast::Receiver<relay_bus::envelope::Event>) -> Vec<Value> {
    let mut out = Vec::new();
    while let Ok(event) = events.try_recv() {
        if event.ev == "device.lease.released" { out.push(event.payload); }
    }
    out
}

#[test]
fn a_second_session_installing_on_a_held_phone_is_told_who_has_it() {
    let f = fixture();
    let e = &f.engine;
    let (a, _) = session(e);
    let (b, _) = session(e);
    let mut events = e.subscribe();

    let listed = ok(e, Actor::User, "device.list", json!({}));
    assert!(listed["devices"][0].get("lease").is_none(), "nobody holds a fresh device: {listed}");

    ok(e, Actor::agent(&a), "guardrail.gate", json!({"session": a, "kind": "exec", "command": "adb -s relay-phone install -r app/build/outputs/apk/debug/app-debug.apk"}));
    let mut acquired = false;
    while let Ok(event) = events.try_recv() { acquired |= event.ev == "device.lease.acquired"; }
    assert!(acquired, "taking a lease is announced");

    // Known up front: device.list names the holder and what it is doing.
    let listed = ok(e, Actor::agent(&b), "device.list", json!({}));
    let lease = &listed["devices"][0]["lease"];
    assert_eq!(lease["session"], a.as_str());
    assert_eq!(lease["kind"], "shell");
    assert_eq!(lease["action"], "adb install -r app-debug.apk");

    // A conflicting install from another session is refused, naming holder, action and time.
    let busy = refusal(gate(e, &b, "cd android && ./gradlew :app:installDebug"));
    assert_eq!(busy.code, "device.busy");
    assert!(busy.message.contains(&a), "{}", busy.message);
    assert!(busy.message.contains("adb install"), "{}", busy.message);
    assert!(busy.message.contains("since"), "{}", busy.message);
    assert!(busy.hint.as_deref().unwrap().contains("bus.wait"), "{:?}", busy.hint);
    assert!(busy.hint.as_deref().unwrap().contains("device.lease.released"), "{:?}", busy.hint);
    assert_eq!(busy.details.as_ref().unwrap()["lease"]["session"], a.as_str());
    // Reads stay free, and unrelated commands never consult the lease.
    ok(e, Actor::agent(&b), "guardrail.gate", json!({"session": b, "kind": "exec", "command": "adb -s relay-phone logcat -d | tail"}));
    ok(e, Actor::agent(&b), "guardrail.gate", json!({"session": b, "kind": "exec", "command": "cargo test"}));
    // The holder itself is never refused.
    ok(e, Actor::agent(&a), "guardrail.gate", json!({"session": a, "kind": "exec", "command": "adb -s relay-phone shell am start -n com.example/.Main"}));

    // Both commands finished: the lease survives only for its grace period. The first to end
    // leaves it alone, since the other may still be using the device.
    ok(e, Actor::agent(&a), "session.report", json!({"session": a, "kind": "tool_use",
        "data": {"tool_name": "Bash", "tool_input": {"command": "adb -s relay-phone install -r app/build/outputs/apk/debug/app-debug.apk"}}}));
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert!(leases["leases"][0]["expires_in_s"].as_u64().unwrap() > 90, "{leases}");
    ok(e, Actor::agent(&a), "session.report", json!({"session": a, "kind": "tool_use",
        "data": {"tool_name": "Bash", "tool_input": {"command": "adb -s relay-phone shell am start -n com.example/.Main"}}}));
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert!(leases["leases"][0]["expires_in_s"].as_u64().unwrap() <= 90, "{leases}");

    // Releasing announces it, and the other session gets the phone.
    let out = ok(e, Actor::agent(&a), "device.release", json!({}));
    assert_eq!(out["released"].as_array().unwrap().len(), 1);
    let gone = released(&mut events);
    assert_eq!(gone.len(), 1, "{gone:?}");
    assert_eq!(gone[0]["session"], a.as_str());
    ok(e, Actor::agent(&b), "guardrail.gate", json!({"session": b, "kind": "exec", "command": "./gradlew installDebug"}));

    // Closing a session gives back its leases, with the release event bus.wait wakes on.
    let busy = refusal(gate(e, &a, "adb install x.apk"));
    assert!(busy.message.contains(&b) && busy.message.contains("every connected device"), "{}", busy.message);
    ok(e, Actor::User, "session.close", json!({"session": b}));
    let gone = released(&mut events);
    assert!(gone.iter().any(|lease| lease["session"] == b.as_str()), "{gone:?}");
    assert!(ok(e, Actor::User, "device.leases", json!({}))["leases"].as_array().unwrap().is_empty());
}

#[test]
fn a_device_run_holds_the_phone_until_it_stops() {
    let f = fixture();
    let e = &f.engine;
    let (a, worktree_a) = session(e);
    let (b, _) = session(e);

    // An agent's claim refuses the user's run for another session, naming the claim.
    let claim = ok(e, Actor::agent(&b), "device.claim", json!({"device": "relay-phone", "action": "testing the login flow", "minutes": 5}));
    assert_eq!(claim["session"], b.as_str());
    assert!(claim["expires_in_s"].as_u64().unwrap() <= 300);
    let busy = refusal(call(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_a, "device": "relay-phone"})));
    assert_eq!(busy.code, "device.busy");
    assert!(busy.message.contains(&b) && busy.message.contains("testing the login flow"), "{}", busy.message);
    // The user may release anyone's claim; an agent may not release another's.
    let (c, _) = session(e);
    assert_eq!(refusal(call(e, Actor::agent(&c), "device.release", json!({"device": "relay-phone"}))).code, "device.busy");
    ok(e, Actor::User, "device.release", json!({"device": "relay-phone"}));

    // The "Build & install" path: a user-started run for session A holds the lease as A.
    let run = ok(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_a, "device": "relay-phone"}));
    let run_id = run["id"].as_i64().unwrap();
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    let lease = &leases["leases"][0];
    assert_eq!(lease["kind"], "run");
    assert_eq!(lease["session"], a.as_str());
    assert_eq!(lease["run_id"], run_id);
    assert!(lease["action"].as_str().unwrap().contains(&format!("relay/{a}")), "{lease}");
    assert!(lease.get("expires_in_s").is_none(), "a run lease lasts as long as the run");

    let busy = refusal(gate(e, &b, "adb install other.apk"));
    assert!(busy.message.contains(&a) && busy.message.contains(&format!("device run {run_id}")), "{}", busy.message);
    // Its own session's commands are not refused, and do not downgrade the run's lease.
    ok(e, Actor::agent(&a), "guardrail.gate", json!({"session": a, "kind": "exec", "command": "adb -s relay-phone shell input tap 1 1"}));
    // A run lease cannot be released, only its run stopped.
    assert_eq!(refusal(call(e, Actor::User, "device.release", json!({"device": "relay-phone"}))).code, "device.lease_run");
    // A second run on the same phone for another session is refused before any build work.
    let worktree_c = ok(e, Actor::User, "session.get", json!({"session": c}))["worktree"].as_str().unwrap().to_string();
    assert_eq!(refusal(call(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_c, "device": "relay-phone"}))).code, "device.busy");

    let mut events = e.subscribe();
    ok(e, Actor::User, "device.run.stop", json!({"run_id": run_id}));
    let gone = released(&mut events);
    assert!(gone.iter().any(|lease| lease["run_id"] == run_id), "{gone:?}");
    // A's own command, taken under the run, still holds the phone until A lets it go.
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert_eq!(leases["leases"][0]["kind"], "shell", "{leases}");
    ok(e, Actor::agent(&a), "device.release", json!({}));
    ok(e, Actor::agent(&b), "guardrail.gate", json!({"session": b, "kind": "exec", "command": "adb install other.apk"}));
}

#[test]
fn a_run_that_fails_releases_its_lease_on_its_own() {
    let f = fixture();
    let e = &f.engine;
    let (a, worktree_a) = session(e);
    let (b, _) = session(e);
    ok(e, Actor::User, "project.update", json!({"project_id": 1, "run_cmd": "exit 7"}));
    let run = ok(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_a, "device": "relay-phone"}));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let runs = ok(e, Actor::User, "device.run.list", json!({"project_id": 1}));
        if runs["runs"][0]["state"] == "failed" { break; }
        assert!(Instant::now() < deadline, "run never failed: {runs}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ok(e, Actor::User, "device.leases", json!({}))["leases"].as_array().unwrap().is_empty(), "run {run} kept its lease");
    ok(e, Actor::agent(&b), "guardrail.gate", json!({"session": b, "kind": "exec", "command": "adb install x.apk"}));
    drop(a);
}

#[test]
fn a_claim_outlives_the_run_its_holder_starts_under_it() {
    let f = fixture();
    let e = &f.engine;
    let (a, worktree_a) = session(e);
    let (b, _) = session(e);
    ok(e, Actor::agent(&a), "device.claim", json!({"device": "relay-phone", "action": "testing the login flow"}));
    let run = ok(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_a, "device": "relay-phone"}));
    let run_id = run["id"].as_i64().unwrap();
    assert_eq!(ok(e, Actor::User, "device.leases", json!({}))["leases"][0]["kind"], "run");
    ok(e, Actor::User, "device.run.stop", json!({"run_id": run_id}));
    // The run is over; the claim it was started under still holds the phone.
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert_eq!(leases["leases"][0]["kind"], "claim", "{leases}");
    assert_eq!(leases["leases"][0]["session"], a.as_str());
    assert_eq!(refusal(gate(e, &b, "adb install other.apk")).code, "device.busy");
}

/// Gradle's daemon outlives a stop, and its `adb install` with it: a run stopped while it builds
/// leaves the device its holder's for a grace period, not open for another install to collide.
#[test]
fn a_run_stopped_while_building_keeps_the_device_for_a_grace_period() {
    let f = fixture();
    let e = &f.engine;
    let (a, worktree_a) = session(e);
    let (b, _) = session(e);
    // Stopped once its build has started: one stopped before (during the checkout's sync) never
    // ran Gradle and frees the device at once.
    let marker = f._root.path().join("build-started");
    ok(e, Actor::User, "project.update", json!({"project_id": 1, "run_cmd": format!("touch '{}'; sleep 30", marker.display())}));
    let run = ok(e, Actor::User, "device.run", json!({"project_id": 1, "worktree": worktree_a, "device": "relay-phone"}));
    let run_id = run["id"].as_i64().unwrap();
    common::wait_until("the build to start", || marker.exists());
    let mut events = e.subscribe();
    ok(e, Actor::User, "device.run.stop", json!({"run_id": run_id}));
    let gone = released(&mut events);
    assert!(gone.iter().any(|lease| lease["run_id"] == run_id), "the run's own lease is gone: {gone:?}");
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    let lease = &leases["leases"][0];
    assert_eq!((lease["kind"].as_str(), lease["session"].as_str()), (Some("shell"), Some(a.as_str())), "{leases}");
    assert!(lease["expires_in_s"].as_u64().unwrap() <= 90, "{leases}");
    assert_eq!(refusal(gate(e, &b, "adb install other.apk")).code, "device.busy");
    ok(e, Actor::agent(&a), "guardrail.gate", json!({"session": a, "kind": "exec", "command": "adb -s relay-phone install x.apk"}));
}

/// An install stopped with Esc sends neither PostToolUse nor Stop: the next prompt ends that
/// turn, so its lease keeps only the grace period instead of the whole term.
#[test]
fn a_new_prompt_ends_the_lease_of_an_interrupted_command() {
    let f = fixture();
    let e = &f.engine;
    let (a, _) = session(e);
    ok(e, Actor::agent(&a), "guardrail.gate", json!({"session": a, "kind": "exec", "command": "adb -s relay-phone install -r app-debug.apk"}));
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert!(leases["leases"][0]["expires_in_s"].as_u64().unwrap() > 90, "{leases}");
    ok(e, Actor::agent(&a), "session.report", json!({"session": a, "kind": "prompt", "data": {"prompt": "try again"}}));
    let leases = ok(e, Actor::User, "device.leases", json!({}));
    assert!(leases["leases"].as_array().unwrap().iter().all(|lease| lease["expires_in_s"].as_u64().unwrap() <= 90), "{leases}");
}
