//! Bus-driven integration tests (SPEC §16): the bus is the test API — the same door agents use.

mod common;

use common::{call_as as call, err, git, wait_until, wait_until_within};
use relay_bus::{Actor, ErrorKind, Request};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

fn engine(inst: Instance) -> Arc<Engine> {
    Engine::new(inst, Store::open_memory().unwrap())
}

fn audit_rows(e: &Engine) -> Vec<Value> {
    call(e, Actor::User, "audit.list", json!({"limit": 1000})).into_result().unwrap()["rows"].as_array().unwrap().clone()
}

fn tmp_repo() -> (tempfile::TempDir, String) {
    let ws = tempfile::tempdir().unwrap();
    let repo = ws.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/trunk\n").unwrap();
    let p = std::fs::canonicalize(&repo).unwrap().display().to_string();
    (ws, p)
}

#[test]
fn ping_and_ops() {
    let e = engine(Instance::Test);
    let r = call(&e, Actor::User, "bus.ping", json!({})).into_result().unwrap();
    assert_eq!(r["pong"], true);
    assert_eq!(r["instance"], "test");
    let ops = call(&e, Actor::User, "bus.ops", json!({})).into_result().unwrap();
    let ops = ops["ops"].as_array().unwrap();
    assert!(ops.len() >= 150);
    let ping = ops.iter().find(|o| o["name"] == "bus.ping").unwrap();
    assert_eq!(ping["implemented"], true);
    let task = ops.iter().find(|o| o["name"] == "task.create").unwrap();
    assert_eq!(task["implemented"], true);
    // agents don't see user-only ops
    let ops = call(&e, Actor::User, "bus.ops", json!({"actor": "agent:x"})).into_result().unwrap();
    assert!(!ops["ops"].as_array().unwrap().iter().any(|o| o["name"] == "session.spawn"));
}

#[test]
fn envelope_and_schema_errors() {
    let e = engine(Instance::Test);
    // unknown op
    let r = call(&e, Actor::User, "nope.nope", json!({}));
    assert_eq!(err(&r).code, "bus.unknown_op");
    // unknown field is bus.schema, not silently dropped (BUS.md §5.2)
    let r = call(&e, Actor::User, "workspace.create", json!({"path": "/tmp", "bogus": 1}));
    assert_eq!(err(&r).code, "bus.schema");
    assert_eq!(err(&r).kind, ErrorKind::Invalid);
    // wrong type
    let r = call(&e, Actor::User, "audit.get", json!({"audit_id": "one"}));
    assert_eq!(err(&r).code, "bus.schema");
    // payload must be an object
    let mut req = Request::new(Actor::User, "bus.ping", json!({}));
    req.payload = json!([]);
    let r = e.dispatch(req, Door::InProcess);
    assert_eq!(err(&r).code, "bus.envelope");
    // wrong v
    let mut req = Request::new(Actor::User, "bus.ping", json!({}));
    req.v = 2;
    assert_eq!(err(&e.dispatch(req, Door::InProcess)).code, "bus.envelope");
    // parse failures
    let r = Engine::parse("not json").unwrap_err();
    assert_eq!(r.id, None);
    assert_eq!(r.error.unwrap().code, "bus.parse");
    let r = Engine::parse(r#"{"id":"3f0f7c1a-3b8e-4a5f-9a4b-2f0d3d1a2b3c","op":5}"#).unwrap_err();
    assert!(r.id.is_some());
    assert_eq!(r.error.unwrap().code, "bus.envelope");
    // invalid requests are never audited (§5.1a)
    assert!(audit_rows(&e).is_empty());
}

#[test]
fn actors_and_doors() {
    let e = engine(Instance::Test);
    // system is never claimable
    let r = call(&e, Actor::System, "bus.ping", json!({}));
    assert_eq!(err(&r).code, "bus.actor");
    // test actor ok on test instance
    assert!(call(&e, Actor::Test, "bus.ping", json!({})).ok);
    // ...but not on stable
    let stable = engine(Instance::Stable);
    assert_eq!(err(&call(&stable, Actor::Test, "bus.ping", json!({}))).code, "bus.actor");
    // socket door needs a token for agents
    let r = e.dispatch(Request::new(Actor::agent("x"), "bus.ping", json!({})), Door::Socket);
    assert_eq!(err(&r).code, "bus.actor");
    // a socket-only op that reaches the engine itself is refused: only a connection can subscribe
    let r = e.dispatch(Request::new(Actor::User, "bus.subscribe", json!({})), Door::InProcess);
    assert_eq!(err(&r).code, "bus.door");
    // layer-1 allowlist: an agent may not create workspaces; the refusal is audited
    let r = call(&e, Actor::agent("brisk-otter"), "workspace.create", json!({"path": "/tmp"}));
    assert_eq!(err(&r).kind, ErrorKind::Refused);
    assert_eq!(err(&r).code, "actor.allowlist");
    let rows = audit_rows(&e);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "refused");
    assert_eq!(rows[0]["actor"], "agent:brisk-otter");
    assert_eq!(rows[0]["op"], "workspace.create");
}

#[test]
fn clone_errors_are_typed_and_schema_failures_are_not_audited() {
    let e = engine(Instance::Test);
    let r = call(&e, Actor::User, "project.clone", json!({"workspace_id": 1, "url": "https://example.invalid/repo.git"}));
    assert_eq!(err(&r).kind, ErrorKind::NotFound);
    assert_eq!(err(&r).code, "workspace.not_found");
    // malformed clone payloads are rejected before the handler and are not audited
    let r = call(&e, Actor::User, "project.clone", json!({}));
    assert_eq!(err(&r).code, "bus.schema");
    let r = call(&e, Actor::User, "project.clone", json!({"workspace_id": 1, "url": "x", "nope": 1}));
    assert_eq!(err(&r).code, "bus.schema");
    let rows = audit_rows(&e);
    assert_eq!(rows.len(), 1, "schema failures are not audited");
    assert_eq!(rows[0]["op"], "project.clone");
    assert_eq!(rows[0]["kind"], "error");
    assert_eq!(rows[0]["code"], "workspace.not_found");
}

fn fake_adb(dir: &std::path::Path) -> String {
    let path = dir.join("adb");
    std::fs::write(&path, r#"#!/bin/sh
if [ "$1" = "track-devices" ]; then
  # As adb sends it: a %04x byte count, then the device list.
  printf '004frelay-phone device product:relay model:Pixel_9_Pro device:relay transport_id:1\n'
  # exec, so stopping the watch kills the sleep itself rather than orphaning it on the pipe.
  exec sleep 5
fi
if [ "$1" = "devices" ]; then
  printf 'List of devices attached\nrelay-phone device product:relay model:Pixel_9_Pro device:relay transport_id:1\n'
  exit 0
fi
if [ "$3" = "shell" ] && [ "$4" = "wm" ] && [ "$5" = "size" ]; then
  printf 'Physical size: 1080x2400\n'
  exit 0
fi
if [ "$3" = "shell" ] && [ "$4" = "cmd" ]; then
  printf 'priority=0 preferredOrder=0\ncom.example.app/.MainActivity\n'
  exit 0
fi
if [ "$3" = "shell" ] && [ "$4" = "am" ]; then
  printf 'Status: ok\nActivity: com.example.app/.MainActivity\n'
  exit 0
fi
if [ "$3" = "shell" ] && [ "$4" = "pidof" ]; then
  printf '1000\n'
  exit 0
fi
if [ "$3" = "logcat" ] && [ "$4" = "-c" ]; then exit 0; fi
if [ "$3" = "logcat" ]; then
  printf '%s\n' "$*" > "$0.logcat"
  printf '08-17 21:04:27.000  1000  1000 I RelayTest: app ready\n'
  # Live until device.run.stop kills it, or until the fixture directory goes away: no
  # deadline after which the run would read "finished" instead of "stopped".
  while [ -e "$0" ]; do sleep 0.1; done
  exit 0
fi
exit 1
"#).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path.display().to_string()
}

#[test]
fn device_discovery_mirror_input_and_run_lifecycle() {
    let e = engine(Instance::Test);
    let fixture = tempfile::tempdir().unwrap();
    let adb = fake_adb(fixture.path());
    call(&e, Actor::User, "settings.set", json!({"path":"device.adb_path","value":adb})).into_result().unwrap();
    call(&e, Actor::User, "settings.set", json!({"path":"device.sdk_path","value":fixture.path()})).into_result().unwrap();

    let listed = call(&e, Actor::User, "device.list", json!({})).into_result().unwrap();
    assert_eq!(listed["devices"][0]["serial"], "relay-phone");
    assert_eq!(listed["devices"][0]["model"], "Pixel 9 Pro");
    assert_eq!(listed["devices"][0]["kind"], "usb");

    let mut events = e.subscribe();
    call(&e, Actor::User, "device.watch", json!({"on":true})).into_result().unwrap();
    wait_until("device detection event", || {
        while let Ok(event) = events.try_recv() {
            if event.ev == "device.changed" && event.payload["reason"] == "system" { return true; }
        }
        false
    });
    call(&e, Actor::User, "device.watch", json!({"on":false})).into_result().unwrap();

    let mirror = e.dispatch(Request::new(Actor::User, "device.mirror.start", json!({"device":"relay-phone","max_size":1080,"bitrate":4_000_000})), Door::InProcess).into_result().unwrap();
    assert_eq!(mirror["width"], 576);
    assert_eq!(mirror["height"], 1280);
    let mirror_id = mirror["mirror_id"].as_i64().unwrap();
    call(&e, Actor::User, "device.mirror.input", json!({"mirror_id":mirror_id,"event":{"type":"tap","x":100,"y":200}})).into_result().unwrap();
    call(&e, Actor::User, "device.mirror.input", json!({"mirror_id":mirror_id,"event":{"type":"swipe","x1":0,"y1":0,"x2":575,"y2":1279}})).into_result().unwrap();
    // The test instance starts no mirror worker, so the input waits in the control queue; a
    // control socket installed now receives it, in order, as the scrcpy messages it encodes.
    let runtime = relay_core::handlers::device::mirror_by_id(&e, mirror_id).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    runtime.install_control(std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap()).unwrap();
    let (mut control, _) = listener.accept().unwrap();
    use relay_core::mirror::{touch, ACTION_DOWN, ACTION_MOVE, ACTION_UP};
    let mut expected = Vec::new();
    for message in [
        touch(ACTION_DOWN, 100, 200, 576, 1280, 1.0), touch(ACTION_UP, 100, 200, 576, 1280, 0.0),
        touch(ACTION_DOWN, 0, 0, 576, 1280, 1.0), touch(ACTION_MOVE, 575, 1279, 576, 1280, 1.0), touch(ACTION_UP, 575, 1279, 576, 1280, 0.0),
    ] {
        expected.extend_from_slice(&message);
    }
    let mut sent = vec![0; expected.len()];
    std::io::Read::read_exact(&mut control, &mut sent).unwrap();
    assert_eq!(sent, expected);
    // A swipe is instant; a duration it would ignore is refused rather than dropped.
    let timed = call(&e, Actor::User, "device.mirror.input", json!({"mirror_id":mirror_id,"event":{"type":"swipe","x1":0,"y1":0,"x2":575,"y2":1279,"duration_ms":300}})).into_result().unwrap_err();
    assert_eq!(timed.code, "device.input");
    assert!(timed.message.contains("duration_ms"), "{}", timed.message);
    // One clipboard message cannot be split: too long is an error, not a pasted prefix.
    let long = "x".repeat(relay_core::mirror::SET_CLIPBOARD_MAX_LENGTH + 1);
    let clipped = call(&e, Actor::User, "device.mirror.input", json!({"mirror_id":mirror_id,"event":{"type":"setclipboard","text":long,"paste":true}})).into_result().unwrap_err();
    assert_eq!(clipped.code, "device.input");
    call(&e, Actor::User, "device.mirror.input", json!({"mirror_id":mirror_id,"event":{"type":"setclipboard","text":"x".repeat(relay_core::mirror::SET_CLIPBOARD_MAX_LENGTH),"paste":true}})).into_result().unwrap();
    call(&e, Actor::User, "device.mirror.stop", json!({"mirror_id":mirror_id})).into_result().unwrap();

    let (workspace, repo) = tmp_repo();
    git(std::path::Path::new(&repo), &["init", "-b", "trunk"]);
    let workspace_path = std::fs::canonicalize(workspace.path()).unwrap().display().to_string();
    call(&e, Actor::User, "workspace.create", json!({"path":workspace_path})).into_result().unwrap();
    let project = call(&e, Actor::User, "project.add", json!({"workspace_id":1,"path":repo})).into_result().unwrap();
    let project_id = project["id"].as_i64().unwrap();
    let wrapper = std::path::Path::new(project["path"].as_str().unwrap()).join("gradlew");
    std::fs::write(&wrapper, r#"#!/bin/sh
[ "$1" = "--init-script" ] || exit 2
grep -q 'installOptions.add("-d")' "$2" || exit 3
[ "$3" = "installDebug" ] || exit 4
printf 'installed sdk=%s\n' "$ANDROID_HOME"
"#).unwrap();
    let mut permissions = std::fs::metadata(&wrapper).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper, permissions).unwrap();
    let metadata = std::path::Path::new(project["path"].as_str().unwrap()).join("app/build/outputs/apk/debug");
    std::fs::create_dir_all(&metadata).unwrap();
    std::fs::write(metadata.join("output-metadata.json"), r#"{
  "variantName": "debug",
  "applicationId": "com.example.app"
}"#).unwrap();
    let run = call(&e, Actor::User, "device.run", json!({"project_id":project_id,"device":"relay-phone","variant":"debug"})).into_result().unwrap();
    let run_id = run["id"].as_i64().unwrap();
    wait_until("the run to reach running", || {
        let result = call(&e, Actor::User, "device.run.list", json!({"project_id":project_id})).into_result().unwrap();
        let state = &result["runs"][0]["state"];
        assert!(state == "building" || state == "running", "{state}");
        state == "running"
    });
    let runtime = relay_core::handlers::device::run_by_id(&e, run_id).unwrap();
    let mut history = None;
    wait_until("logcat attachment", || {
        let (_, current, _) = runtime.attach();
        let attached = current.iter().any(|line| line.line.contains("logcat attached pid=1000"));
        if attached { history = Some(current); }
        attached
    });
    let history = history.unwrap();
    assert!(history.iter().any(|line| line.line.contains(&format!("installed sdk={}", fixture.path().display()))));
    assert!(history.iter().any(|line| line.line.contains("app launched: com.example.app/.MainActivity")));
    assert!(history.iter().any(|line| line.line.contains("logcat attached pid=1000")));
    let mut logcat = String::new();
    wait_until("logcat arguments", || {
        logcat = std::fs::read_to_string(format!("{adb}.logcat")).unwrap_or_default();
        logcat.contains("-b main,system,crash") && logcat.contains("--pid=1000")
    });
    assert!(logcat.contains("-b main,system,crash"), "{logcat}");
    assert!(logcat.contains("--pid=1000"), "{logcat}");
    call(&e, Actor::User, "device.run.stop", json!({"run_id":run_id})).into_result().unwrap();
    let result = call(&e, Actor::User, "device.run.list", json!({"project_id":project_id})).into_result().unwrap();
    assert_eq!(result["runs"][0]["state"], "stopped");
}

#[test]
fn device_release_build_records_its_artifact() {
    // A store on disk, so the signing profile (keystore, password file) lands in this
    // directory's `signing/` and goes with it — an in-memory store would put it in /tmp.
    let data = tempfile::tempdir().unwrap();
    let e = Engine::new(Instance::Test, Store::open(&data.path().join("store.db"), false).unwrap());
    let sdk = tempfile::tempdir().unwrap();
    let apksigner = sdk.path().join("build-tools/35.0.0/apksigner");
    std::fs::create_dir_all(apksigner.parent().unwrap()).unwrap();
    std::fs::write(&apksigner, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&apksigner).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&apksigner, permissions).unwrap();
    call(
        &e,
        Actor::User,
        "settings.set",
        json!({"path":"device.sdk_path","value":sdk.path()}),
    )
    .into_result()
    .unwrap();

    let (workspace, repo) = tmp_repo();
    git(std::path::Path::new(&repo), &["init", "-b", "trunk"]);
    let workspace_path = std::fs::canonicalize(workspace.path())
        .unwrap()
        .display()
        .to_string();
    call(
        &e,
        Actor::User,
        "workspace.create",
        json!({"path":workspace_path}),
    )
    .into_result()
    .unwrap();
    let project = call(
        &e,
        Actor::User,
        "project.add",
        json!({"workspace_id":1,"path":repo}),
    )
    .into_result()
    .unwrap();
    let project_id = project["id"].as_i64().unwrap();
    let root = std::path::Path::new(project["path"].as_str().unwrap()).to_path_buf();

    let signing = call(
        &e,
        Actor::User,
        "device.signing.get",
        json!({"project_id":project_id}),
    )
    .into_result()
    .unwrap();
    assert_eq!(signing["configured"], false);
    assert_eq!(signing["enabled"], false);

    // The wrapper records the task Relay chose — asserting on that log cannot silently skip
    // the way reading a finished run's buffer can — and leaves the artifact AGP would.
    let wrapper = root.join("gradlew");
    std::fs::write(&wrapper, r#"#!/bin/sh
printf 'args:%s\n' "$*"
i=0
while [ -e hold ] && [ "$i" -lt 400 ]; do i=$((i+1)); sleep 0.05; done
if [ "$RELAY_SIGNING_STORE_PASSWORD" = "test-secret-123" ]; then secret=set; else secret=bad; fi
if [ "$secret" = set ]; then
  [ "$2" = "--init-script" ] || exit 5
  grep -q 'gradle.beforeProject' "$3" || exit 6
  grep -q 'androidComponents.finalizeDsl' "$3" || exit 7
  ! grep -q '^allprojects' "$3" || exit 8
fi
printf '%s|%s|%s|%s|%s\n' "$1" "$ANDROID_HOME" "$RELAY_SIGNING_KEY_ALIAS" "$secret" "$2" >> tasks.log
case "$1" in
  assembleRelease)
    mkdir -p app/build/outputs/apk/release
    : > app/build/outputs/apk/release/app-release.apk
    ;;
  publishReleaseBundle)
    mkdir -p app/build/outputs/bundle/release
    : > app/build/outputs/bundle/release/app-release.aab
    ;;
  assembleHold)
    i=0
    while [ "$i" -lt 200 ]; do i=$((i+1)); sleep 0.05; done
    ;;
  *) exit 4 ;;
esac
"#).unwrap();
    let mut permissions = std::fs::metadata(&wrapper).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper, permissions).unwrap();

    let build = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id}),
    )
    .into_result()
    .unwrap();
    assert_eq!(build["kind"], "build");
    assert_eq!(build["device"], "");
    assert_eq!(build["state"], "building");
    assert!(build["artifact"].is_null());
    assert_eq!(build["variant"], "release");
    assert_eq!(build["format"], "apk");
    assert_eq!(build["publish"], false);
    assert!(build["signing"].is_null());
    let build_id = build["id"].as_i64().unwrap();

    let mut finished = json!(null);
    wait_until("the release build to finish", || {
        let result = call(
            &e,
            Actor::User,
            "device.run.list",
            json!({"project_id":project_id}),
        )
        .into_result()
        .unwrap();
        finished = result["runs"][0].clone();
        finished["state"] == "finished" || finished["state"] == "failed"
    });
    assert_eq!(finished["state"], "finished", "{finished}");
    assert_eq!(finished["id"].as_i64().unwrap(), build_id);
    assert_eq!(
        finished["artifact"].as_str().unwrap(),
        root.join("app/build/outputs/apk/release/app-release.apk")
            .display()
            .to_string()
    );
    assert_eq!(finished["signing"], "signed");

    // The task Relay handed Gradle, and the SDK it handed with it.
    let tasks = std::fs::read_to_string(root.join("tasks.log")).unwrap();
    assert!(
        tasks
            .lines()
            .any(|line| line == format!("assembleRelease|{}||bad|", sdk.path().display())),
        "{tasks}"
    );

    // Opting into Relay signing creates one private profile, never audits the password, and
    // adds a temporary init script plus secret environment only to later release builds.
    let mut events = e.subscribe();
    let signing = e
        .dispatch(
            Request::new(
                Actor::User,
                "device.signing.create",
                json!({"project_id":project_id,"key_alias":"upload","password":"test-secret-123"}),
            ),
            Door::Socket,
        )
        .into_result()
        .unwrap();
    assert_eq!(signing["configured"], true);
    assert_eq!(signing["enabled"], true);
    assert_eq!(signing["key_alias"], "upload");
    assert!(std::path::Path::new(signing["keystore"].as_str().unwrap()).is_file());
    assert!(signing["keystore"].as_str().unwrap().starts_with(&data.path().join("signing").display().to_string()), "{signing}");
    assert!(
        audit_rows(&e)
            .iter()
            .all(|row| row["op"] != "device.signing.create"),
        "password-bearing operations are never audited"
    );
    // The wrapper waits on `hold`, so the run's live buffer can be taken before it ends.
    std::fs::write(root.join("hold"), "").unwrap();
    let profile_build = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id}),
    )
    .into_result()
    .unwrap();
    let profile_build_id = profile_build["id"].as_i64().unwrap();
    let profile_runtime = relay_core::handlers::device::run_by_id(&e, profile_build_id).unwrap();
    std::fs::remove_file(root.join("hold")).unwrap();
    wait_until("the Relay-signed build to finish", || {
        let result = call(
            &e,
            Actor::User,
            "device.run.list",
            json!({"project_id":project_id}),
        )
        .into_result()
        .unwrap();
        result["runs"][0]["id"].as_i64() == Some(profile_build_id)
            && result["runs"][0]["state"] == "finished"
    });
    let tasks = std::fs::read_to_string(root.join("tasks.log")).unwrap();
    assert!(
        tasks.lines().any(|line| line.starts_with(&format!(
            "assembleRelease|{}|upload|set|--init-script",
            sdk.path().display()
        ))),
        "{tasks}"
    );
    assert!(
        !tasks.contains("test-secret-123"),
        "the signing password must not enter build output"
    );
    // Nor the run's own log (the command line Relay echoes, the arguments Gradle got), the
    // events the build raised, or the audit trail.
    let output: Vec<String> = profile_runtime.attach().1.iter().map(|line| line.line.to_string()).collect();
    assert!(output.iter().any(|line| line.starts_with("args:assembleRelease")), "{output:?}");
    assert!(output.iter().all(|line| !line.contains("test-secret-123")), "{output:?}");
    let mut run_events = 0;
    while let Ok(event) = events.try_recv() {
        if event.ev == "run.changed" { run_events += 1; }
        assert!(!event.payload.to_string().contains("test-secret-123"), "{}: {}", event.ev, event.payload);
    }
    assert!(run_events > 0, "the build raised no run.changed to check");
    assert!(!Value::Array(audit_rows(&e)).to_string().contains("test-secret-123"));

    // A generated key can be parked without deleting it when Google Play already expects the
    // project's established upload key. The next build then uses Gradle signing unchanged.
    let disabled = call(
        &e,
        Actor::User,
        "device.signing.set_enabled",
        json!({"project_id":project_id,"enabled":false}),
    )
    .into_result()
    .unwrap();
    assert_eq!(disabled["configured"], true);
    assert_eq!(disabled["enabled"], false);
    let gradle_build = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id}),
    )
    .into_result()
    .unwrap();
    let gradle_build_id = gradle_build["id"].as_i64().unwrap();
    wait_until("the Gradle-signed build to finish", || {
        let result = call(
            &e,
            Actor::User,
            "device.run.list",
            json!({"project_id":project_id}),
        )
        .into_result()
        .unwrap();
        result["runs"][0]["id"].as_i64() == Some(gradle_build_id)
            && result["runs"][0]["state"] == "finished"
    });
    let tasks = std::fs::read_to_string(root.join("tasks.log")).unwrap();
    assert_eq!(
        tasks
            .lines()
            .rfind(|line| line.starts_with("assembleRelease|"))
            .unwrap(),
        format!("assembleRelease|{}||bad|", sdk.path().display())
    );
    let enabled = call(
        &e,
        Actor::User,
        "device.signing.set_enabled",
        json!({"project_id":project_id,"enabled":true}),
    )
    .into_result()
    .unwrap();
    assert_eq!(enabled["enabled"], true);

    // A bundle asks for a different Gradle task, and this wrapper only answers assembleRelease.
    // The pickers name it by its extension, `aab`: it is a bundle all the same.
    let bundle = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id,"format":"aab"}),
    )
    .into_result()
    .unwrap();
    assert_eq!(bundle["format"], "bundle");
    let bundle_id = bundle["id"].as_i64().unwrap();
    wait_until("the bundle build to fail", || {
        let result = call(
            &e,
            Actor::User,
            "device.run.list",
            json!({"project_id":project_id}),
        )
        .into_result()
        .unwrap();
        result["runs"][0]["id"].as_i64() == Some(bundle_id)
            && result["runs"][0]["state"] == "failed"
    });
    assert!(std::fs::read_to_string(root.join("tasks.log")).unwrap().lines().any(|line| line.starts_with("bundleRelease|")));

    // The Play half: the same run row, a different Gradle task, an .aab artifact. Gradle Play
    // Publisher does the upload, so this is the whole of Relay's part in publishing.
    let published = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id,"format":"bundle","publish":true}),
    )
    .into_result()
    .unwrap();
    let published_id = published["id"].as_i64().unwrap();
    let mut row = json!(null);
    // A bundle's signature is checked with the host's own `jarsigner` (JAVA_HOME or PATH), a JVM:
    // 90 ms here, but on a cold CI runner it once took past the usual 10 s. Wait as long as
    // Relay itself would (SIGNING_VERIFY_TIMEOUT); the row says "unverified" either way.
    wait_until_within("the publishing build to finish", std::time::Duration::from_secs(70), || {
        let result = call(
            &e,
            Actor::User,
            "device.run.list",
            json!({"project_id":project_id}),
        )
        .into_result()
        .unwrap();
        row = result["runs"][0].clone();
        row["id"].as_i64() == Some(published_id)
            && (row["state"] == "finished" || row["state"] == "failed")
    });
    assert_eq!(row["state"], "finished", "{row}");
    assert!(
        row["artifact"]
            .as_str()
            .unwrap()
            .ends_with("app-release.aab"),
        "{row}"
    );
    assert_eq!(row["variant"], "release");
    assert_eq!(row["format"], "bundle");
    assert_eq!(row["publish"], true);
    assert_eq!(row["signing"], "unverified");
    let tasks = std::fs::read_to_string(root.join("tasks.log")).unwrap();
    assert!(
        tasks
            .lines()
            .any(|line| line.starts_with("publishReleaseBundle|")),
        "{tasks}"
    );

    // A build is stoppable mid-flight like a run: the command reaches the tile while it is
    // live, and device.run.stop leaves the row stopped rather than failed.
    let held = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id,"variant":"hold"}),
    )
    .into_result()
    .unwrap();
    let held_id = held["id"].as_i64().unwrap();
    wait_until("the held build to stream its command", || {
        relay_core::handlers::device::run_by_id(&e, held_id).is_ok_and(|runtime| {
            runtime
                .attach()
                .1
                .iter()
                .any(|line| line.line.contains("$ ./gradlew assembleHold"))
        })
    });
    call(
        &e,
        Actor::User,
        "device.run.stop",
        json!({"run_id":held_id}),
    )
    .into_result()
    .unwrap();
    let stopped = call(
        &e,
        Actor::User,
        "device.run.list",
        json!({"project_id":project_id}),
    )
    .into_result()
    .unwrap();
    assert_eq!(stopped["runs"][0]["id"].as_i64(), Some(held_id));
    assert_eq!(stopped["runs"][0]["state"], "stopped");

    // Typed refusals, not a shell surprise.
    let bad_format = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id,"format":"ipa"}),
    );
    assert_eq!(err(&bad_format).code, "device.format");
    let bad_variant = call(
        &e,
        Actor::User,
        "device.build",
        json!({"project_id":project_id,"variant":"release; rm -rf /"}),
    );
    assert_eq!(err(&bad_variant).code, "device.variant");
    let agent = call(
        &e,
        Actor::Agent("merry-badger".into()),
        "device.build",
        json!({"project_id":project_id}),
    );
    assert_eq!(err(&agent).code, "actor.allowlist", "a release build is user-only");
}

#[test]
fn idempotent_replay() {
    let e = engine(Instance::Test);
    let id = Uuid::new_v4();
    let req = Request::new(Actor::User, "settings.set", json!({"path": "a.b", "value": 1})).with_id(id);
    let r1 = e.dispatch(req.clone(), Door::InProcess);
    assert!(r1.ok && r1.replayed.is_none());
    let r2 = e.dispatch(req.clone(), Door::InProcess);
    assert!(r2.ok);
    assert_eq!(r2.replayed, Some(true));
    assert_eq!(r1.result, r2.result);
    // same id, different payload
    let mut req3 = req.clone();
    req3.payload = json!({"path": "a.b", "value": 2});
    let r3 = e.dispatch(req3, Door::InProcess);
    assert_eq!(err(&r3).code, "bus.id_reused");
    // only one audit row for the three
    assert_eq!(audit_rows(&e).len(), 1);
    // queries are not deduplicated
    let q = Request::new(Actor::User, "settings.get", json!({"path": "a.b"}));
    let a = e.dispatch(q.clone(), Door::InProcess);
    let b = e.dispatch(q, Door::InProcess);
    assert!(a.replayed.is_none() && b.replayed.is_none());
    // a recorded error replays as that error
    let id = Uuid::new_v4();
    let bad = Request::new(Actor::User, "workspace.remove", json!({"workspace_id": 99})).with_id(id);
    let r1 = e.dispatch(bad.clone(), Door::InProcess);
    assert_eq!(err(&r1).code, "workspace.not_found");
    let r2 = e.dispatch(bad, Door::InProcess);
    assert_eq!(err(&r2).code, "workspace.not_found");
    assert_eq!(r2.replayed, Some(true));
}

#[test]
fn usage_display_preferences_default_on_and_override_per_meter() {
    let e = engine(Instance::Test);
    let v = call(&e, Actor::User, "settings.get", json!({"path": "usage"})).into_result().unwrap();
    assert_eq!(v["value"]["refresh_minutes"], 0, "no timer unless the user asks for one");
    for (provider, meters) in [("claude", &["enabled", "five_hour", "weekly", "fable"][..]), ("codex", &["enabled", "five_hour", "weekly"][..])] {
        for meter in meters {
            assert_eq!(v["value"][provider][meter], true, "{provider}.{meter} shows by default");
        }
    }
    call(&e, Actor::User, "settings.set", json!({"path": "usage.codex.enabled", "value": false})).into_result().unwrap();
    call(&e, Actor::User, "settings.set", json!({"path": "usage.claude.fable", "value": false})).into_result().unwrap();
    call(&e, Actor::User, "settings.set", json!({"path": "usage.refresh_minutes", "value": 5})).into_result().unwrap();
    let v = call(&e, Actor::User, "settings.get", json!({"path": "usage"})).into_result().unwrap();
    assert_eq!(v["value"]["codex"]["enabled"], false);
    assert_eq!(v["value"]["codex"]["weekly"], true, "hiding a provider keeps its meter choices");
    assert_eq!(v["value"]["claude"]["fable"], false);
    assert_eq!(v["value"]["claude"]["five_hour"], true);
    assert_eq!(v["value"]["refresh_minutes"], 5);
}

#[test]
fn settings_tree_and_undo_op() {
    let e = engine(Instance::Test);
    let v = call(&e, Actor::User, "settings.get", json!({})).into_result().unwrap();
    assert_eq!(v["value"]["undo"]["grace_days"], 7, "defaults show through");
    call(&e, Actor::User, "settings.set", json!({"path": "appearance.mode", "value": "oled"})).into_result().unwrap();
    call(&e, Actor::User, "settings.set", json!({"path": "keybindings", "value": {"ctrl+k": {"op": "ui.page.switch"}}})).into_result().unwrap();
    let v = call(&e, Actor::User, "settings.get", json!({"path": "appearance"})).into_result().unwrap();
    assert_eq!(v["value"]["mode"], "oled");
    assert_eq!(v["value"]["panel_alpha"], 1.0);
    let v = call(&e, Actor::User, "settings.get", json!({"path": "keybindings.ctrl-k"})).into_result().unwrap();
    assert!(v["value"].is_null(), "dashes vs plus: different key");
    let v = call(&e, Actor::User, "settings.get", json!({"path": "keybindings"})).into_result().unwrap();
    assert_eq!(v["value"]["ctrl+k"]["op"], "ui.page.switch");
    // reset a subtree, defaults come back
    call(&e, Actor::User, "settings.reset", json!({"path": "appearance"})).into_result().unwrap();
    let v = call(&e, Actor::User, "settings.get", json!({"path": "appearance.mode"})).into_result().unwrap();
    assert_eq!(v["value"], "dark");
    // bad path
    let r = call(&e, Actor::User, "settings.set", json!({"path": "a..b", "value": 1}));
    assert_eq!(err(&r).code, "settings.path");
    // undo op recorded on the audit rows
    let rows = audit_rows(&e);
    let set_row = rows.iter().find(|r| r["op"] == "settings.set" && r["payload"]["path"] == "appearance.mode").unwrap();
    assert_eq!(set_row["undo_op"]["op"], "settings.set");
    assert_eq!(set_row["undo_op"]["payload"]["path"], "appearance.mode");
    assert_eq!(set_row["undo_op"]["payload"]["value"], "dark");
    // agents may not touch settings
    let r = call(&e, Actor::agent("x"), "settings.set", json!({"path": "a", "value": 1}));
    assert_eq!(err(&r).code, "actor.allowlist");
}

/// D150: a leaf read must not go past the tree; the answer still has to be the one the full tree
/// would have given, ancestors and defaults included.
#[test]
fn settings_leaf_reads_agree_with_the_full_tree() {
    let e = engine(Instance::Test);
    let set = |path: &str, value: Value| {
        call(&e, Actor::User, "settings.set", json!({"path": path, "value": value}))
            .into_result()
            .unwrap();
    };
    set("appearance.mode", json!("oled"));
    set("appearance.wallpaper", json!("x".repeat(4096)));
    set("layout.current.7", json!({"page": "code", "agent_layout": "grid"}));
    set("theme", json!({"a": {"b": {"c": 1}}}));

    let tree = call(&e, Actor::User, "settings.get", json!({}))
        .into_result()
        .unwrap()["value"]
        .clone();
    for path in [
        "appearance",
        "appearance.mode",
        "appearance.panel_alpha",
        "appearance.wallpaper",
        "appearance.missing",
        "layout",
        "layout.current.7.page",
        "theme.a.b.c",
        "guardrails.caps.files",
        "undo.grace_days",
        "nothing.here.at.all",
    ] {
        let leaf = call(&e, Actor::User, "settings.get", json!({"path": path}))
            .into_result()
            .unwrap()["value"]
            .clone();
        let mut expected = &tree;
        for segment in path.split('.') {
            expected = expected.get(segment).unwrap_or(&Value::Null);
        }
        assert_eq!(&leaf, expected, "leaf read of {path:?} disagrees with the tree");
    }

    // A scalar stored at an ancestor replaces the default object below it, for both readers.
    set("appearance", json!("flattened"));
    let leaf = call(&e, Actor::User, "settings.get", json!({"path": "appearance.mode"}))
        .into_result()
        .unwrap()["value"]
        .clone();
    assert!(leaf.is_null(), "a scalar ancestor hides its former children");
    let tree = call(&e, Actor::User, "settings.get", json!({}))
        .into_result()
        .unwrap()["value"]
        .clone();
    assert_eq!(tree["appearance"], json!("flattened"));
}

/// Retention keeps the log a window, not an archive — but an undo pair is the record of what was
/// reversed, so it survives whatever its age.
#[test]
fn audit_retention_prunes_old_rows_and_keeps_undo_pairs() {
    let e = engine(Instance::Test);
    call(&e, Actor::User, "settings.set", json!({"path": "appearance.mode", "value": "oled"}))
        .into_result()
        .unwrap();
    call(&e, Actor::User, "settings.set", json!({"path": "appearance.panel_alpha", "value": 0.5}))
        .into_result()
        .unwrap();
    let rows = audit_rows(&e);
    let undoable = rows
        .iter()
        .find(|r| r["payload"]["path"] == "appearance.mode")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    call(&e, Actor::User, "audit.undo", json!({"audit_id": undoable}))
        .into_result()
        .unwrap();

    // Age everything past the window; the undo pair is still linked.
    e.store.lock()
        .execute("UPDATE audit SET ts = '2020-01-01T00:00:00Z'", [])
        .unwrap();
    relay_core::recovery::run(&e).unwrap();

    let kept = audit_rows(&e);
    let ops: Vec<&str> = kept.iter().map(|r| r["op"].as_str().unwrap()).collect();
    assert!(kept.iter().any(|r| r["id"] == undoable), "the undone row stays: {ops:?}");
    assert!(
        kept.iter().any(|r| r["undo_of"] == undoable),
        "so does the row that undid it: {ops:?}"
    );
    assert!(
        !kept.iter().any(|r| r["payload"]["path"] == "appearance.panel_alpha"),
        "an unlinked row past the window is gone: {ops:?}"
    );

    // Retention is a setting, and 0 means keep everything.
    call(&e, Actor::User, "settings.set", json!({"path": "audit.retention_days", "value": 0}))
        .into_result()
        .unwrap();
    e.store.lock()
        .execute("UPDATE audit SET ts = '2020-01-01T00:00:00Z'", [])
        .unwrap();
    let before = audit_rows(&e).len();
    relay_core::recovery::run(&e).unwrap();
    assert_eq!(audit_rows(&e).len(), before, "retention 0 prunes nothing");
}

#[test]
fn workspace_and_project_flow() {
    let e = engine(Instance::Test);
    let (ws_dir, repo) = tmp_repo();
    let ws_path = std::fs::canonicalize(ws_dir.path()).unwrap().display().to_string();
    // events arrive after commit
    let mut rx = e.subscribe();

    let ws = call(&e, Actor::User, "workspace.create", json!({"path": ws_path})).into_result().unwrap();
    assert_eq!(ws["id"], 1);
    let ev = rx.try_recv().unwrap();
    assert_eq!(ev.ev, "workspace.changed");
    assert_eq!(ev.actor, Actor::User);
    assert!(ev.cause.is_some());
    // duplicate
    let r = call(&e, Actor::User, "workspace.create", json!({"path": ws_path}));
    assert_eq!(err(&r).code, "workspace.exists");
    // relative path
    let r = call(&e, Actor::User, "workspace.create", json!({"path": "relative"}));
    assert_eq!(err(&r).code, "workspace.path");

    // project outside workspace
    let other = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(other.path().join(".git")).unwrap();
    let r = call(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": other.path().display().to_string()}));
    assert_eq!(err(&r).code, "project.outside_workspace");
    // not a repo
    let plain = ws_dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let r = call(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": plain.display().to_string()}));
    assert_eq!(err(&r).code, "project.path");
    // ok
    let pr = call(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": repo})).into_result().unwrap();
    assert_eq!(pr["base_branch"], "trunk", "detected from .git/HEAD");
    assert_eq!(pr["name"], "repo");
    let ev = rx.try_recv().unwrap();
    assert_eq!(ev.ev, "project.changed");
    assert_eq!(ev.project_id, Some(1));
    // duplicate
    let r = call(&e, Actor::User, "project.add", json!({"workspace_id": 1, "path": repo}));
    assert_eq!(err(&r).code, "project.exists");
    // update + inverse recorded with expect
    let pr = call(&e, Actor::User, "project.update", json!({"project_id": 1, "name": "Renamed", "pinned": true, "build_cmd": "make"})).into_result().unwrap();
    assert_eq!(pr["name"], "Renamed");
    assert_eq!(pr["pinned"], true);
    assert_eq!(pr["build_cmd"], "make");
    let row = audit_rows(&e).into_iter().find(|r| r["op"] == "project.update").unwrap();
    assert_eq!(row["undo_op"]["payload"]["name"], "repo");
    assert_eq!(row["undo_op"]["expect"]["updated_at"], pr["updated_at"]);
    assert_eq!(row["project_id"], 1);
    // clearing an optional: build_cmd: null
    let pr = call(&e, Actor::User, "project.update", json!({"project_id": 1, "build_cmd": null})).into_result().unwrap();
    assert!(pr["build_cmd"].is_null());
    // list
    let l = call(&e, Actor::User, "project.list", json!({"workspace_id": 1})).into_result().unwrap();
    assert_eq!(l["projects"].as_array().unwrap().len(), 1);
    // remove workspace with a project -> conflict; then remove project, then workspace
    let r = call(&e, Actor::User, "workspace.remove", json!({"workspace_id": 1}));
    assert_eq!(err(&r).code, "workspace.has_projects");
    call(&e, Actor::User, "project.remove", json!({"project_id": 1})).into_result().unwrap();
    call(&e, Actor::User, "workspace.remove", json!({"workspace_id": 1})).into_result().unwrap();
    assert!(call(&e, Actor::User, "workspace.list", json!({})).into_result().unwrap()["workspaces"].as_array().unwrap().is_empty());
    // failed mutation left nothing behind, but was audited as error
    let r = call(&e, Actor::User, "project.get", json!({"project_id": 1}));
    assert_eq!(err(&r).code, "project.not_found");
    let rows = audit_rows(&e);
    let failed = rows.iter().find(|row| row["op"] == "workspace.remove" && row["kind"] == "error").expect("the refused workspace.remove was audited");
    assert_eq!(failed["code"], "workspace.has_projects");
}

#[test]
fn audit_row_shape() {
    let e = engine(Instance::Test);
    let (ws_dir, _repo) = tmp_repo();
    let ws_path = std::fs::canonicalize(ws_dir.path()).unwrap().display().to_string();
    let req = Request::new(Actor::User, "workspace.create", json!({"path": ws_path}));
    let id = req.id;
    e.dispatch(req, Door::InProcess).into_result().unwrap();
    let row = call(&e, Actor::User, "audit.get", json!({"audit_id": 1})).into_result().unwrap();
    assert_eq!(row["req_id"], id.to_string());
    assert_eq!(row["kind"], "ok");
    assert_eq!(row["actor"], "user");
    assert_eq!(row["payload"]["path"], ws_path);
    assert_eq!(row["result_summary"]["id"], 1);
    assert_eq!(row["payload_hash"].as_str().unwrap().len(), 64);
    // filters
    let rows = call(&e, Actor::User, "audit.list", json!({"op_prefix": "workspace."})).into_result().unwrap();
    assert_eq!(rows["rows"].as_array().unwrap().len(), 1);
    let rows = call(&e, Actor::User, "audit.list", json!({"op_prefix": "task."})).into_result().unwrap();
    assert!(rows["rows"].as_array().unwrap().is_empty());
    let r = call(&e, Actor::User, "audit.get", json!({"audit_id": 99}));
    assert_eq!(err(&r).code, "audit.not_found");
}

#[test]
fn project_remove_forgets_owned_metadata_but_keeps_the_repository() {
    let e = engine(Instance::Test);
    let (workspace_dir, repo) = tmp_repo();
    let workspace = call(&e, Actor::User, "workspace.create", json!({"path": workspace_dir.path()})).into_result().unwrap();
    let project = call(&e, Actor::User, "project.add", json!({"workspace_id":workspace["id"],"path":repo})).into_result().unwrap();
    let project_id = project["id"].as_i64().unwrap();
    let task = call(&e, Actor::User, "task.create", json!({"project_id":project_id,"title":"Disposable metadata"})).into_result().unwrap();
    call(&e, Actor::User, "notes.create", json!({"project_id":project_id,"title":"Context","body":"Stored in Relay"})).into_result().unwrap();
    let skill = call(&e, Actor::User, "skill.create", json!({"name":"removal-test","body":"test"})).into_result().unwrap();
    call(&e, Actor::User, "skill.enable", json!({"skill_id":skill["id"],"project_id":project_id,"enabled":true})).into_result().unwrap();
    assert_eq!(task["project_id"], project_id);

    call(&e, Actor::User, "project.remove", json!({"project_id":project_id})).into_result().unwrap();
    assert!(std::path::Path::new(&repo).exists(), "project removal must not touch the repository");
    let conn = e.store.lock();
    for table in ["projects", "tasks", "notes", "skill_projects"] {
        let count: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "{table} metadata survived project removal");
    }
}

#[test]
fn store_migrations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    {
        let s = Store::open(&path, false).unwrap();
        assert_eq!(s.version().unwrap(), relay_core::store::SCHEMA_VERSION);
    }
    // reopen: no-op migration
    {
        let s = Store::open(&path, false).unwrap();
        assert_eq!(s.version().unwrap(), relay_core::store::SCHEMA_VERSION);
    }
    // a newer store refuses to open in an older build
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 999).unwrap();
    }
    assert!(Store::open(&path, false).is_err());
}

/// RA-013: the lock file was opened following symlinks and then truncated, so whoever could
/// plant `<instance>.lock` in the runtime dir could have the engine empty any file of ours.
#[tokio::test]
async fn the_socket_door_refuses_a_planted_lock_and_tightens_its_dir() {
    use relay_core::socket::SocketServer;
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let victim = root.path().join("authorized_keys");
    std::fs::write(&victim, "ssh-ed25519 AAAA keep-me\n").unwrap();
    let dir = root.path().join("run");
    std::fs::create_dir(&dir).unwrap();
    std::os::unix::fs::symlink(&victim, dir.join("test.lock")).unwrap();
    assert!(SocketServer::start_in(engine(Instance::Test), dir.clone()).await.is_err());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "ssh-ed25519 AAAA keep-me\n");

    // A runtime dir that is a symlink is refused too, wherever it points.
    let link = root.path().join("linked");
    std::os::unix::fs::symlink(root.path().join("elsewhere"), &link).unwrap();
    std::fs::create_dir(root.path().join("elsewhere")).unwrap();
    assert!(SocketServer::start_in(engine(Instance::Test), link).await.is_err());

    // Our own directory, left open, is closed down to 0700 before anything is put in it.
    let open = root.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
    let server = SocketServer::start_in(engine(Instance::Test), open.clone()).await.unwrap();
    assert_eq!(std::fs::metadata(&open).unwrap().permissions().mode() & 0o777, 0o700);
    drop(server);
}

/// RA-015: unlocked queries on one connection run side by side and answer by id, while a
/// mutation still waits its turn behind the queries sent before it.
#[tokio::test]
async fn one_connection_runs_its_queries_concurrently_and_keeps_writes_in_order() {
    use relay_core::socket::{Client, Line, SocketServer};
    let dir = tempfile::tempdir().unwrap();
    let e = engine(Instance::Test);
    // The ops the door handles itself must never take the concurrent path.
    for op in ["bus.wait", "bus.subscribe", "bus.unsubscribe", "session.attach", "session.detach", "session.resize",
        "device.watch", "app.resources.watch", "device.run", "device.build", "device.mirror.start", "device.run.stop"] {
        assert!(!e.runs_unlocked(op), "{op} is connection bookkeeping, not a free-standing query");
    }
    assert!(e.runs_unlocked("git.pr.list") && e.runs_unlocked("file.search"));

    let server = SocketServer::start_in(e.clone(), dir.path().to_path_buf()).await.unwrap();
    let mut c = Client::connect(&server.path).await.unwrap();
    let mut sent = Vec::new();
    for n in 0..20 {
        let req = if n == 15 {
            Request::new(Actor::User, "workspace.create", json!({"path": dir.path().join("ws")}))
        } else {
            Request::new(Actor::User, "worktree.list", json!({"project_id": 99}))
        };
        c.send_raw(&serde_json::to_string(&req).unwrap()).await.unwrap();
        sent.push(req.id);
    }
    let mut answered = Vec::new();
    while answered.len() < sent.len() {
        if let Some(Line::Response(r)) = c.next().await.unwrap() {
            answered.push(r.id.unwrap());
        }
    }
    let mut sorted = answered.clone();
    sorted.sort();
    let mut expected = sent.clone();
    expected.sort();
    assert_eq!(sorted, expected, "every request is answered exactly once");
    let write_at = answered.iter().position(|id| *id == sent[15]).unwrap();
    for earlier in &sent[..15] {
        assert!(answered.iter().position(|id| id == earlier).unwrap() < write_at, "a write overtook a query sent before it");
    }
}

#[tokio::test]
async fn socket_door_round_trip_and_events() {
    use relay_core::socket::{Client, Line, SocketServer};
    let dir = tempfile::tempdir().unwrap();
    let e = engine(Instance::Test);
    let server = SocketServer::start_in(e.clone(), dir.path().to_path_buf()).await.unwrap();
    // second engine in the same dir is refused
    let e2 = engine(Instance::Test);
    assert!(matches!(SocketServer::start_in(e2, dir.path().to_path_buf()).await, Err(relay_core::socket::BindError::AlreadyRunning { .. })));

    let mut c = Client::connect(&server.path).await.unwrap();
    let r = c.call(&Request::new(Actor::User, "bus.ping", json!({})), |_| {}).await.unwrap();
    assert!(r.ok);
    // agent without token
    let r = c.call(&Request::new(Actor::agent("x"), "bus.ping", json!({})), |_| {}).await.unwrap();
    assert_eq!(r.error.unwrap().code, "bus.actor");
    // garbage line
    c.send_raw("{nope").await.unwrap();
    match c.next().await.unwrap().unwrap() {
        Line::Response(r) => { assert_eq!(r.id, None); assert_eq!(r.error.unwrap().code, "bus.parse"); }
        _ => panic!("expected response"),
    }
    // subscribe on a second connection, mutate on the first, see the event
    let mut sub = Client::connect(&server.path).await.unwrap();
    let r = sub.call(&Request::new(Actor::User, "bus.subscribe", json!({"events": ["settings.*"]})), |_| {}).await.unwrap();
    assert_eq!(r.result.unwrap()["subscribed"][0], "settings.*");
    // The workspace event is raised first, so if the filter let it through it would be the
    // first thing this connection reads.
    let (ws_dir, _) = tmp_repo();
    c.call(&Request::new(Actor::User, "workspace.create", json!({"path": std::fs::canonicalize(ws_dir.path()).unwrap()})), |_| {}).await.unwrap();
    c.call(&Request::new(Actor::User, "settings.set", json!({"path": "x", "value": 1})), |_| {}).await.unwrap();
    match tokio::time::timeout(std::time::Duration::from_secs(2), sub.next()).await.unwrap().unwrap().unwrap() {
        Line::Event(ev) => assert_eq!(ev.ev, "settings.changed", "the workspace event was not filtered out"),
        _ => panic!("expected event"),
    }
    // unsubscribe, then nothing more arrives
    sub.call(&Request::new(Actor::User, "bus.unsubscribe", json!({})), |_| {}).await.unwrap();
    c.call(&Request::new(Actor::User, "settings.set", json!({"path": "y", "value": 1})), |_| {}).await.unwrap();
    assert!(tokio::time::timeout(std::time::Duration::from_millis(200), sub.next()).await.is_err());
    // bus.wait blocks until a matching event arrives — the wake-up an agent has instead of a
    // poll loop (D115). A waiter on one connection, the mutation on another.
    let mut waiter = Client::connect(&server.path).await.unwrap();
    let wait = tokio::spawn(async move {
        waiter
            .call(
                &Request::new(Actor::User, "bus.wait", json!({"events": ["settings.*"], "timeout_ms": 5000})),
                |_| {},
            )
            .await
            .unwrap()
    });
    // Nothing says when the server has subscribed the waiter, so raise the event until it
    // wakes: one that lands before the subscription is simply missed, never a failure.
    let mut wait = wait;
    let woken = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut n = 0;
        loop {
            n += 1;
            c.call(&Request::new(Actor::User, "settings.set", json!({"path": "z", "value": n})), |_| {}).await.unwrap();
            if let Ok(woken) = tokio::time::timeout(std::time::Duration::from_millis(50), &mut wait).await {
                break woken.unwrap();
            }
        }
    }).await.unwrap();
    let result = woken.result.unwrap();
    assert_eq!(result["timed_out"], false);
    assert_eq!(result["event"]["ev"], "settings.changed");

    // `matching` takes this waiter's own event, not the first one of the same name.
    let mut picky = Client::connect(&server.path).await.unwrap();
    let wait = tokio::spawn(async move {
        picky
            .call(
                &Request::new(Actor::User, "bus.wait", json!({
                    "events": ["settings.*"], "matching": {"path": "mine"}, "timeout_ms": 5000,
                })),
                |_| {},
            )
            .await
            .unwrap()
    });
    // As above, repeated until the waiter wakes; "theirs" always goes first, so a waiter that
    // ignored `matching` would take it.
    let mut wait = wait;
    let woken = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut n = 0;
        loop {
            n += 1;
            c.call(&Request::new(Actor::User, "settings.set", json!({"path": "theirs", "value": n})), |_| {}).await.unwrap();
            c.call(&Request::new(Actor::User, "settings.set", json!({"path": "mine", "value": n})), |_| {}).await.unwrap();
            if let Ok(woken) = tokio::time::timeout(std::time::Duration::from_millis(50), &mut wait).await {
                break woken.unwrap();
            }
        }
    }).await.unwrap();
    let result = woken.result.unwrap();
    assert_eq!(result["event"]["payload"]["path"], "mine", "the waiter took someone else's event");

    // …and gives up rather than hanging when nothing matches.
    let mut idle = Client::connect(&server.path).await.unwrap();
    let timed_out = idle
        .call(&Request::new(Actor::User, "bus.wait", json!({"events": ["nothing.happens"], "timeout_ms": 1000})), |_| {})
        .await
        .unwrap();
    let result = timed_out.result.unwrap();
    assert_eq!(result["timed_out"], true);
    assert!(result["event"].is_null());

    // quit via bus
    let r = c.call(&Request::new(Actor::User, "app.quit", json!({})), |_| {}).await.unwrap();
    assert!(r.ok);
    tokio::time::timeout(std::time::Duration::from_secs(1), e.wait_quit()).await.unwrap();
    drop(server);
    assert!(!dir.path().join("test.sock").exists(), "socket unlinked on drop");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_mirror_socket_stream_and_disconnect_cleanup() {
    use base64::Engine as _;
    use relay_core::socket::{Client, Line, SocketServer};
    let e = engine(Instance::Test);
    let fixture = tempfile::tempdir().unwrap();
    let adb = fake_adb(fixture.path());
    call(
        &e,
        Actor::User,
        "settings.set",
        json!({"path":"device.adb_path","value":adb}),
    )
    .into_result()
    .unwrap();
    let server = SocketServer::start_in(e.clone(), fixture.path().join("socket"))
        .await
        .unwrap();
    let mut client = Client::connect(&server.path).await.unwrap();
    let result = client
        .call(
            &Request::new(
                Actor::User,
                "device.mirror.start",
                json!({"device":"relay-phone","max_size":1024}),
            ),
            |_| {},
        )
        .await
        .unwrap()
        .into_result()
        .unwrap();
    let id = result["mirror_id"].as_i64().unwrap();
    let runtime = relay_core::handlers::device::mirror_by_id(&e, id).unwrap();
    runtime.push(vec![3, 0, 0, 0, 1, 7]);
    // The stream opens with the mirror's status (an object); video packets are strings.
    let frame = loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match &frame {
            Line::Frame(f) if f.data.is_object() => assert_eq!(f.data["state"], "starting"),
            _ => break frame,
        }
    };
    match frame {
        Line::Frame(frame) => {
            assert_eq!(frame.stream, "mirror");
            assert_eq!(frame.mirror_id, Some(id));
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(frame.data.as_str().unwrap())
                    .unwrap(),
                vec![3, 0, 0, 0, 1, 7]
            );
        }
        _ => panic!("expected native mirror frame"),
    }
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !runtime.stopped() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(relay_core::handlers::device::mirror_by_id(&e, id).is_err());
}
