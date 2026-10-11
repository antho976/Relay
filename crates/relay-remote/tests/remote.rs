//! A phone's view of the remote door, end to end: a real engine behind a real socket, a
//! WebSocket client that pairs, proves itself, calls the bus and streams a live terminal —
//! directly and through a rendezvous server and the host tunnel. The `wss://` dial is covered
//! only as far as `tls.rs` goes: no test here runs a TLS server.

use futures_util::{SinkExt, StreamExt};
use relay_core::engine::Engine;
use relay_core::socket::SocketServer;
use relay_core::{Instance, Store};
use relay_remote::direct::DirectServer;
use relay_remote::rendezvous::RendezvousServer;
use relay_remote::wire::{self, Greeting, Welcome};
use relay_remote::{Ctx, Registry};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Harness {
    _dir: tempfile::TempDir,
    engine: Arc<Engine>,
    _socket: SocketServer,
    ctx: Arc<Ctx>,
}

async fn harness() -> Harness {
    harness_with(relay_remote::wire::OpProfile::Phone).await
}

async fn harness_with(ops: relay_remote::wire::OpProfile) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("store.db"), false).unwrap();
    let engine = Engine::new(Instance::Test, store);
    let socket = SocketServer::start_in(engine.clone(), dir.path().join("run"))
        .await
        .expect("socket door");
    let ctx = Arc::new(Ctx {
        instance: Instance::Test,
        registry_path: dir.path().join("remote.json"),
        socket_path: socket.path.clone(),
        version: "test".into(),
        start_engine: None,
        ops,
    });
    Harness { _dir: dir, engine, _socket: socket, ctx }
}

/// Set up what a phone may not do itself, as the person at the PC would.
fn at_the_pc(h: &Harness, op: &str, payload: Value) -> Value {
    use relay_bus::envelope::{Actor, Request};
    let resp = h.engine.dispatch(Request::new(Actor::User, op, payload), relay_core::engine::Door::Socket);
    assert!(resp.ok, "{op}: {resp:?}");
    resp.result.unwrap_or(Value::Null)
}

/// A first-come window, as `relay remote pair --no-confirm` opens; confirmation has its own
/// tests in `bridge.rs`.
fn pair_code(ctx: &Ctx) -> String {
    Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.begin_pair(false).code)).unwrap()
}

async fn recv_text(ws: &mut Ws) -> String {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await.expect("no line in 5s") {
            Some(Ok(Message::Text(t))) => return t.to_string(),
            Some(Ok(Message::Close(_))) | None => panic!("socket closed"),
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("socket error: {e}"),
        }
    }
}

async fn recv_json(ws: &mut Ws) -> Value {
    serde_json::from_str(&recv_text(ws).await).unwrap()
}

async fn send(ws: &mut Ws, line: String) {
    ws.send(Message::text(line)).await.unwrap();
}

async fn call(ws: &mut Ws, op: &str, payload: Value) -> Value {
    let id = uuid_v4();
    send(ws, json!({"v":1,"id":id,"actor":"user","op":op,"payload":payload}).to_string()).await;
    loop {
        let line = recv_json(ws).await;
        if line.get("id").and_then(|i| i.as_str()) == Some(&id) {
            return line;
        }
    }
}

fn uuid_v4() -> String {
    // A UUID shape the engine accepts, without pulling `uuid` into dev-deps.
    let hex = relay_remote::registry::random_hex(16);
    format!("{}-{}-4{}-a{}-{}", &hex[..8], &hex[8..12], &hex[13..16], &hex[17..20], &hex[20..32])
}

/// Pair over a fresh socket; returns the credential and the greeting.
async fn pair(url: &str, code: &str) -> (String, String, Greeting) {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.expect("connect");
    let greeting: Greeting = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    assert_eq!(greeting.relay, "remote");
    assert_eq!(greeting.instance, "test");
    send(&mut ws, json!({"v":1,"pair":code,"device_name":"Test Phone"}).to_string()).await;
    let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    assert!(welcome.ok, "{welcome:?}");
    (welcome.device.unwrap(), welcome.token.unwrap(), greeting)
}

/// Connect with a credential; returns the admitted socket.
async fn admit(url: &str, device: &str, token: &str) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.expect("connect");
    let greeting: Greeting = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    send(
        &mut ws,
        json!({"v":1,"device":device,"proof":wire::proof(&greeting.challenge, token)}).to_string(),
    )
    .await;
    let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    assert!(welcome.ok, "{welcome:?}");
    assert!(welcome.token.is_none());
    ws
}

async fn exercise_bus(ws: &mut Ws) {
    let pong = call(ws, "bus.ping", json!({})).await;
    assert_eq!(pong["ok"], true, "{pong}");
    assert_eq!(pong["result"]["pong"], true);

    // A phone is the user: an agent envelope is refused here, with the engine's error shape.
    let id = uuid_v4();
    send(ws, json!({"v":1,"id":id,"actor":"agent:brisk-otter","op":"bus.ping","payload":{},"token":"nope"}).to_string()).await;
    let refused = recv_json(ws).await;
    assert_eq!(refused["id"], id);
    assert_eq!(refused["ok"], false);
    assert_eq!(refused["error"]["code"], "bus.actor");

    // Two lines in one frame are two requests to the engine, and the gate sees both.
    let (first, second) = (uuid_v4(), uuid_v4());
    send(ws, format!("{}\n{}",
        json!({"v":1,"id":first,"actor":"user","op":"bus.ping","payload":{}}),
        json!({"v":1,"id":second,"actor":"agent:brisk-otter","op":"bus.ping","payload":{},"token":"nope"}))).await;
    let (mut pong, mut refused) = (None, None);
    while pong.is_none() || refused.is_none() {
        let line = recv_json(ws).await;
        if line["id"] == first {
            pong = Some(line);
        } else if line["id"] == second {
            refused = Some(line);
        }
    }
    assert_eq!(pong.unwrap()["ok"], true);
    assert_eq!(refused.unwrap()["error"]["code"], "bus.actor", "the second line of a frame went ungated");

    // Events interleave with responses exactly as on the socket door.
    let sub = call(ws, "bus.subscribe", json!({"events":["workspace.*"]})).await;
    assert_eq!(sub["ok"], true, "{sub}");
    // The event and the response race for the wire; both must arrive, in either order.
    let dir = tempfile::tempdir().unwrap();
    let id = uuid_v4();
    send(ws, json!({"v":1,"id":id,"actor":"user","op":"workspace.create","payload":{"path": dir.path(), "name": "phone"}}).to_string()).await;
    let (mut saw_event, mut saw_response) = (false, false);
    while !(saw_event && saw_response) {
        let line = recv_json(ws).await;
        if line.get("ev").and_then(|e| e.as_str()) == Some("workspace.changed") {
            assert_eq!(line["payload"]["name"], "phone");
            saw_event = true;
        } else if line.get("id").and_then(|i| i.as_str()) == Some(&id) {
            assert_eq!(line["ok"], true, "{line}");
            saw_response = true;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_pairs_proves_itself_and_uses_the_bus_directly() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);

    // Nothing paired and no window open: the door shows the network nothing at all.
    assert!(tokio_tungstenite::connect_async(url.as_str()).await.is_err(), "an unused door answered");

    // A window is open, but a proof for an unknown device is turned away before the engine hears it.
    let code = pair_code(&h.ctx);
    {
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str()).await.unwrap();
        let _greeting = recv_text(&mut ws).await;
        send(&mut ws, json!({"v":1,"device":"nobody","proof":"00"}).to_string()).await;
        let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
        assert_eq!(welcome.error.as_deref(), Some("auth.unknown_device"));
    }

    let (device, token, greeting) = pair(&url, &code).await;
    assert_eq!(greeting.host, Registry::load(&h.ctx.registry_path).unwrap().host_name);

    // The code is spent; a second phone with the same code is refused.
    {
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str()).await.unwrap();
        let _ = recv_text(&mut ws).await;
        send(&mut ws, json!({"v":1,"pair":code,"device_name":"Imposter"}).to_string()).await;
        let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
        assert_eq!(welcome.error.as_deref(), Some("pair.invalid"));
    }

    let mut ws = admit(&url, &device, &token).await;
    exercise_bus(&mut ws).await;

    // A wrong proof is a wrong proof, even for a known device.
    {
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str()).await.unwrap();
        let _ = recv_text(&mut ws).await;
        send(&mut ws, json!({"v":1,"device":device,"proof":wire::proof("stale", &token)}).to_string()).await;
        let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
        assert_eq!(welcome.error.as_deref(), Some("auth.bad_proof"));
    }

    // Revoking the device ends its access on the next connection. (A second phone keeps the
    // door answering, so the refusal is the device's, not the door's.)
    let _second = pair(&url, &pair_code(&h.ctx)).await;
    assert!(Registry::update(&h.ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.revoke(&device))).unwrap());
    {
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str()).await.unwrap();
        let greeting: Greeting = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
        send(&mut ws, json!({"v":1,"device":device,"proof":wire::proof(&greeting.challenge, &token)}).to_string()).await;
        let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
        assert_eq!(welcome.error.as_deref(), Some("auth.unknown_device"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_info_answers_plain_http_without_a_handshake() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn get_info(addr: SocketAddr) -> String {
        let mut tcp = TcpStream::connect(addr).await.unwrap();
        tcp.write_all(b"GET /info HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut buf = String::new();
        // A door that closes without reading the request may reset the connection instead.
        let _ = tcp.read_to_string(&mut buf).await;
        buf
    }
    // Unused, the door does not even say what it is.
    assert_eq!(get_info(door.local_addr).await, "");

    pair_code(&h.ctx);
    let buf = get_info(door.local_addr).await;
    assert!(buf.starts_with("HTTP/1.1 200 OK"));
    // No web page may read it from the person's browser.
    assert!(!buf.to_ascii_lowercase().contains("access-control-allow-origin"), "{buf}");
    let body = buf.split("\r\n\r\n").nth(1).unwrap();
    let info: Value = serde_json::from_str(body).unwrap();
    assert_eq!(info["relay"], "remote");
    assert_eq!(info["instance"], "test");
    assert_eq!(info["pairing_open"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_web_page_cannot_open_the_door_but_the_phone_can() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    pair_code(&h.ctx);
    let url = format!("ws://{}", door.local_addr);
    let with_origin = |origin: &str| {
        let mut req = url.as_str().into_client_request().unwrap();
        req.headers_mut().insert("origin", origin.parse().unwrap());
        req
    };
    // A browser on another site.
    assert!(tokio_tungstenite::connect_async(with_origin("https://evil.example")).await.is_err());
    // React Native sends the URL's own origin.
    let (mut ws, _) = tokio_tungstenite::connect_async(with_origin(&format!("http://{}", door.local_addr)))
        .await
        .expect("same-origin upgrade");
    let greeting: Greeting = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    assert_eq!(greeting.relay, "remote");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_address_cannot_take_every_unproven_slot() {
    use tokio::io::AsyncReadExt;
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "0.0.0.0:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let code = pair_code(&h.ctx);
    let port = door.local_addr.port();

    // A peer that opens sockets and says nothing gets a few, then is refused at once.
    let mut idle = Vec::new();
    for _ in 0..4 {
        idle.push(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut refused = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut byte = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(2), refused.read(&mut byte))
        .await
        .expect("the extra connection was held open")
        .unwrap_or(0);
    assert_eq!(n, 0);

    // A phone on another address still pairs.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
    let stream = socket.connect(format!("127.0.0.1:{port}").parse().unwrap()).await.unwrap();
    let (mut ws, _) = tokio_tungstenite::client_async(format!("ws://127.0.0.1:{port}"), MaybeTlsStream::Plain(stream))
        .await
        .unwrap();
    let _greeting = recv_text(&mut ws).await;
    send(&mut ws, json!({"v":1,"pair":code,"device_name":"Phone"}).to_string()).await;
    let welcome: Welcome = serde_json::from_str(&recv_text(&mut ws).await).unwrap();
    assert!(welcome.ok, "{welcome:?}");
    drop(idle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_same_phone_reaches_the_engine_through_a_rendezvous() {
    let h = harness().await;
    let server = RendezvousServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let (rendezvous, code) = Registry::update(&h.ctx.registry_path, |r| {
        let rendezvous = r.set_rendezvous(&format!("ws://{}", server.local_addr)).clone();
        Ok::<_, anyhow::Error>((rendezvous, r.begin_pair(false).code))
    })
    .unwrap();
    let join = relay_remote::tunnel::join_url(&rendezvous);

    // No host yet: a phone that joins is told so and closed.
    {
        let (mut ws, _) = tokio_tungstenite::connect_async(join.as_str()).await.unwrap();
        let first = recv_json(&mut ws).await;
        assert_eq!(first["error"], "host.offline");
    }

    // The wrong secret cannot host the room, in the header or (from an older PC) the URL.
    {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut bad = relay_remote::tunnel::host_url(&rendezvous).into_client_request().unwrap();
        bad.headers_mut().insert("authorization", "Bearer wrong".parse().unwrap());
        assert!(tokio_tungstenite::connect_async(bad).await.is_err());
        let bad = format!("ws://{}/host/{}?secret=wrong", server.local_addr, rendezvous.room);
        assert!(tokio_tungstenite::connect_async(bad.as_str()).await.is_err());
    }
    // The secret never rides in the URL a proxy logs.
    assert!(!relay_remote::tunnel::host_url(&rendezvous).contains(&rendezvous.secret));

    let tunnel = relay_remote::tunnel::spawn(h.ctx.clone(), rendezvous.clone());
    await_hosts(server.local_addr, 1).await;

    let (device, token, _) = pair(&join, &code).await;
    let mut a = admit(&join, &device, &token).await;
    exercise_bus(&mut a).await;

    // Two phones at once are two lanes, each with its own engine connection.
    let mut b = admit(&join, &device, &token).await;
    let pong = call(&mut b, "bus.ping", json!({})).await;
    assert_eq!(pong["ok"], true);
    let pong = call(&mut a, "bus.ping", json!({})).await;
    assert_eq!(pong["ok"], true);

    tunnel.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_apps_new_request_payloads_reach_the_engine() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let code = pair_code(&h.ctx);
    let (device, token, _) = pair(&url, &code).await;
    let mut ws = admit(&url, &device, &token).await;

    // A session that does not exist is the engine's refusal, relayed verbatim.
    let missing = call(&mut ws, "session.attach", json!({"session":"no-such"})).await;
    assert_eq!(missing["ok"], false);
    assert_eq!(missing["error"]["kind"], "not_found");

    // A minimal project so a session can exist; a provider that is `cat` so it echoes.
    let repo = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("git").args(["-C", repo.path().to_str().unwrap(), "init", "-q"]).output().unwrap();
    assert!(out.status.success());
    let ws_dir = repo.path().parent().unwrap();
    let created = call(&mut ws, "workspace.create", json!({"path": ws_dir})).await;
    assert_eq!(created["ok"], true, "{created}");
    let project = call(&mut ws, "project.add", json!({"workspace_id": created["result"]["id"], "path": repo.path()})).await;
    assert_eq!(project["ok"], true, "{project}");
    let listed = call(&mut ws, "session.list", json!({})).await;
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["result"]["sessions"].as_array().map(|s| s.len()), Some(0));

    // The phone's "New request": exactly the payloads the app sends, so a schema drift on
    // either side fails here rather than on a phone. `start: false` stages the dispatch
    // without needing a provider binary on the test machine.
    let project_id = project["result"]["id"].clone();
    let task = call(&mut ws, "task.create", json!({"project_id": project_id, "title": "Fix the flaky login test", "body": "Fix the flaky login test\nand explain what was wrong."})).await;
    assert_eq!(task["ok"], true, "{task}");
    let dispatched = call(&mut ws, "task.dispatch", json!({
        "task_id": task["result"]["id"],
        "create": {"project_id": project_id, "provider": "claude", "role": "builder"},
        "start": false,
    })).await;
    assert_eq!(dispatched["ok"], true, "{dispatched}");
    let name = dispatched["result"]["session"]["name"].as_str().unwrap().to_string();
    assert_eq!(dispatched["result"]["task"]["column"], "active");

    // The one-off shape: a session with the text as its opening prompt.
    let created = call(&mut ws, "session.create", json!({"project_id": project_id, "provider": "codex", "role": "docs", "prompt": "Write the README."})).await;
    assert_eq!(created["ok"], true, "{created}");

    // Mail to the dispatched session, as the terminal's "mail" action sends it.
    let mailed = call(&mut ws, "mailbox.send", json!({"project_id": project_id, "to": name, "text": "Also update the changelog.", "priority": true})).await;
    assert_eq!(mailed["ok"], true, "{mailed}");

    let listed = call(&mut ws, "session.list", json!({})).await;
    assert_eq!(listed["result"]["sessions"].as_array().map(|s| s.len()), Some(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_fits_a_terminal_to_itself_and_hands_it_back() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let code = pair_code(&h.ctx);
    let (device, token, _) = pair(&url, &code).await;
    let mut ws = admit(&url, &device, &token).await;

    // A live PTY: a provider that is a shell script waiting on its input.
    let repo = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("git").args(["-C", repo.path().to_str().unwrap(), "init", "-q"]).output().unwrap();
    assert!(out.status.success());
    let provider = repo.path().join("fake-claude.sh");
    std::fs::write(&provider, "#!/bin/sh\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; done\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Where the engine finds an executable is the PC's to set, never a phone's.
    let set = call(&mut ws, "settings.set", json!({"path": "providers.claude.path", "value": provider})).await;
    assert_eq!(set["error"]["code"], "remote.op", "{set}");
    at_the_pc(&h, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let created = call(&mut ws, "workspace.create", json!({"path": repo.path().parent().unwrap()})).await;
    assert_eq!(created["ok"], true, "{created}");
    let project = call(&mut ws, "project.add", json!({"workspace_id": created["result"]["id"], "path": repo.path()})).await;
    assert_eq!(project["ok"], true, "{project}");
    let session = call(&mut ws, "session.create", json!({"project_id": project["result"]["id"], "provider": "claude"})).await;
    assert_eq!(session["ok"], true, "{session}");
    let name = session["result"]["name"].as_str().unwrap().to_string();
    let spawned = call(&mut ws, "session.spawn", json!({"session": name})).await;
    assert_eq!(spawned["ok"], true, "{spawned}");

    // The terminal screen's opening read: the text, where it ends, and the PC's size.
    let back = call(&mut ws, "session.scrollback", json!({"session": name, "lines": 400})).await;
    assert_eq!(back["ok"], true, "{back}");
    let (cols, rows) = (back["result"]["cols"].clone(), back["result"]["rows"].clone());
    assert_eq!((cols.as_u64(), rows.as_u64()), (Some(120), Some(40)));

    // Fit to the phone for as long as it looks: turning sideways refits, and leaving (the
    // screen detaching) hands the PC's size back.
    let size = |back: &Value| (back["result"]["cols"].as_u64(), back["result"]["rows"].as_u64());
    let attached = call(&mut ws, "session.attach", json!({"session": name, "epoch": back["result"]["epoch"], "from_seq": back["result"]["seq"]})).await;
    assert_eq!(attached["ok"], true, "{attached}");
    for (fit_cols, fit_rows) in [(52, 38), (96, 20)] {
        let fit = call(&mut ws, "session.resize", json!({"session": name, "cols": fit_cols, "rows": fit_rows, "until_detach": true})).await;
        assert_eq!(fit["ok"], true, "{fit}");
        let back = call(&mut ws, "session.scrollback", json!({"session": name, "lines": 400})).await;
        assert_eq!(size(&back), (Some(fit_cols), Some(fit_rows)));
    }
    let detached = call(&mut ws, "session.detach", json!({"session": name})).await;
    assert_eq!(detached["ok"], true, "{detached}");
    let back = call(&mut ws, "session.scrollback", json!({"session": name, "lines": 400})).await;
    assert_eq!(size(&back), (cols.as_u64(), rows.as_u64()));

    // "Fit to phone" off: the PC's size, set for good, which ends the loan.
    let fit = call(&mut ws, "session.resize", json!({"session": name, "cols": 52, "rows": 38, "until_detach": true})).await;
    assert_eq!(fit["ok"], true, "{fit}");
    let wide = call(&mut ws, "session.resize", json!({"session": name, "cols": cols, "rows": rows})).await;
    assert_eq!(wide["ok"], true, "{wide}");

    // A phone that drops mid-look hands the size back too: the bridge's socket closes with it.
    let fit = call(&mut ws, "session.resize", json!({"session": name, "cols": 52, "rows": 38, "until_detach": true})).await;
    assert_eq!(fit["ok"], true, "{fit}");
    drop(ws);
    let mut ws = admit(&url, &device, &token).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let back = call(&mut ws, "session.scrollback", json!({"session": name, "lines": 400})).await;
        if size(&back) == (cols.as_u64(), rows.as_u64()) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the size never came back: {back}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let closed = call(&mut ws, "session.close", json!({"session": name})).await;
    assert_eq!(closed["ok"], true, "{closed}");
}

/// The server's `/health` body.
async fn health(addr: SocketAddr) -> Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    tcp.write_all(b"GET /health HTTP/1.1\r\n\r\n").await.unwrap();
    let mut buf = String::new();
    tcp.read_to_string(&mut buf).await.unwrap();
    serde_json::from_str(buf.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

/// Poll `/health` until `hosts` reads `want`, or fail after a few seconds.
async fn await_hosts(addr: SocketAddr, want: u64) {
    for _ in 0..100 {
        if health(addr).await["hosts"] == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the rendezvous never listed {want} host(s): {}", health(addr).await);
}

/// A bare host socket for a room of the test's own making: no engine, no tunnel.
async fn raw_host(addr: SocketAddr, secret: &str) -> Ws {
    let room = relay_remote::registry::room_for(secret);
    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/host/{room}?secret={secret}"))
        .await
        .expect("host upgrade");
    ws
}

/// Whether the socket is closed (or errors) within `within`, skipping anything it still says.
async fn closes_within(ws: &mut Ws, within: Duration) -> bool {
    tokio::time::timeout(within, async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .is_ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoking_a_connected_phone_closes_its_connection() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let (device, token, _) = pair(&url, &pair_code(&h.ctx)).await;
    let (other, other_token, _) = pair(&url, &pair_code(&h.ctx)).await;
    let mut ws = admit(&url, &device, &token).await;
    let mut kept = admit(&url, &other, &other_token).await;
    assert_eq!(call(&mut ws, "bus.ping", json!({})).await["ok"], true);

    // Unrelated writes (another phone's `last_seen`) leave it connected.
    let _third = admit(&url, &other, &other_token).await;
    assert_eq!(call(&mut ws, "bus.ping", json!({})).await["ok"], true);

    // `relay remote revoke`, from another process: only the file changes.
    assert!(Registry::update(&h.ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.revoke(&device))).unwrap());
    assert!(closes_within(&mut ws, Duration::from_secs(5)).await, "a revoked phone kept its connection");
    // The other phone is untouched.
    assert_eq!(call(&mut kept, "bus.ping", json!({})).await["ok"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_joiner_message_drops_that_joiner_not_the_tunnel() {
    let h = harness().await;
    let server = RendezvousServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let (rendezvous, code) = Registry::update(&h.ctx.registry_path, |r| {
        let rendezvous = r.set_rendezvous(&format!("ws://{}", server.local_addr)).clone();
        Ok::<_, anyhow::Error>((rendezvous, r.begin_pair(false).code))
    })
    .unwrap();
    let join = relay_remote::tunnel::join_url(&rendezvous);
    let tunnel = relay_remote::tunnel::spawn(h.ctx.clone(), rendezvous.clone());
    await_hosts(server.local_addr, 1).await;
    let (device, token, _) = pair(&join, &code).await;
    let mut phone = admit(&join, &device, &token).await;
    assert_eq!(call(&mut phone, "bus.ping", json!({})).await["ok"], true);

    // A stranger who knows the room id, with no credential: one message over the phone limit,
    // and one under it that JSON escaping grows past what the host socket carries.
    for payload in ["x".repeat(9 << 20), "\u{1}".repeat(3 << 20)] {
        let (mut stranger, _) = tokio_tungstenite::connect_async(join.as_str()).await.unwrap();
        let _greeting = recv_text(&mut stranger).await;
        let _ = stranger.send(Message::text(payload)).await;
        assert!(closes_within(&mut stranger, Duration::from_secs(10)).await, "the oversized joiner stayed");
    }

    // The paired phone's lane — and so the PC's tunnel — never noticed.
    assert_eq!(call(&mut phone, "bus.ping", json!({})).await["ok"], true);
    assert_eq!(health(server.local_addr).await["hosts"], 1);
    tunnel.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replaced_host_dropping_late_leaves_the_new_host_in_the_room() {
    let server = RendezvousServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let secret = "a-secret-of-the-test's-own";
    let room = relay_remote::registry::room_for(secret);
    let old = raw_host(server.local_addr, secret).await;
    await_hosts(server.local_addr, 1).await;
    // The PC reconnects (its old socket half-open), then the old socket finally drops.
    let mut new = raw_host(server.local_addr, secret).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(old);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(health(server.local_addr).await["hosts"], 1, "the stale host's exit cleared the room");

    // A phone that joins now reaches the new host.
    let (_phone, _) = tokio_tungstenite::connect_async(format!("ws://{}/join/{room}", server.local_addr))
        .await
        .unwrap();
    let line = recv_json(&mut new).await;
    assert_eq!(line["t"], "open", "{line}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn silent_hosts_and_phones_are_dropped_and_live_ones_kept() {
    let limits = relay_remote::rendezvous::Limits {
        host_silence: Duration::from_millis(600),
        phone_silence: Duration::from_millis(600),
        ping_every: Duration::from_millis(150),
        ..Default::default()
    };
    let server = RendezvousServer::bind_with("127.0.0.1:0".parse::<SocketAddr>().unwrap(), limits)
        .await
        .unwrap();
    let addr = server.local_addr;

    // A host that keeps reading answers the server's pings and stays; one whose machine
    // vanished (it never reads or writes again) is dropped and its room empties.
    let (gone_secret, live_secret) = ("vanished-pc", "live-pc");
    let _gone = raw_host(addr, gone_secret).await;
    let mut live = raw_host(addr, live_secret).await;
    let (opened_tx, mut opened) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let reader = tokio::spawn(async move {
        while let Some(Ok(msg)) = live.next().await {
            if let Message::Text(t) = msg {
                let _ = opened_tx.send(serde_json::from_str(&t).unwrap());
            }
        }
    });
    await_hosts(addr, 2).await;
    await_hosts(addr, 1).await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(health(addr).await["hosts"], 1, "a live host was dropped");

    // The vanished PC's room answers host.offline straight away instead of hanging.
    let gone_room = relay_remote::registry::room_for(gone_secret);
    let (mut late, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/join/{gone_room}")).await.unwrap();
    assert_eq!(recv_json(&mut late).await["error"], "host.offline");

    // A phone that joins the live room and then goes silent (never reading, never sending)
    // loses its lane, and the host hears it closed.
    let live_room = relay_remote::registry::room_for(live_secret);
    let (_silent, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/join/{live_room}")).await.unwrap();
    let open = tokio::time::timeout(Duration::from_secs(5), opened.recv()).await.unwrap().unwrap();
    assert_eq!(open["t"], "open");
    let close = tokio::time::timeout(Duration::from_secs(5), opened.recv()).await.expect("the silent phone kept its lane").unwrap();
    assert_eq!(close["t"], "close");
    assert_eq!(close["c"], open["c"]);
    reader.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_pairing_is_announced_to_the_desktop() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let (device, token, _) = pair(&url, &pair_code(&h.ctx)).await;
    // A client listening as the desktop does.
    let mut desktop = admit(&url, &device, &token).await;
    let sub = call(&mut desktop, "bus.subscribe", json!({"events":["ui.toast"]})).await;
    assert_eq!(sub["ok"], true, "{sub}");

    let (second, _, _) = pair(&url, &pair_code(&h.ctx)).await;
    loop {
        let line = recv_json(&mut desktop).await;
        // The first pairing's own announcement is sent in the background and may land here.
        if line["ev"] == "ui.toast" && line["payload"]["text"].as_str().is_some_and(|t| t.contains(&second)) {
            let text = line["payload"]["text"].as_str().unwrap();
            assert!(text.contains("Test Phone") && text.contains("direct 127.0.0.1"), "{text}");
            break;
        }
    }
}

/// A session whose provider is a shell script that prints a line, then echoes what it is sent.
async fn live_session(h: &Harness, ws: &mut Ws) -> (String, tempfile::TempDir) {
    let repo = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("git").args(["-C", repo.path().to_str().unwrap(), "init", "-q"]).output().unwrap();
    assert!(out.status.success());
    let provider = repo.path().join("fake-claude.sh");
    std::fs::write(&provider, "#!/bin/sh\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; done\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    at_the_pc(h, "settings.set", json!({"path": "providers.claude.path", "value": provider}));
    let created = call(ws, "workspace.create", json!({"path": repo.path().parent().unwrap()})).await;
    assert_eq!(created["ok"], true, "{created}");
    let project = call(ws, "project.add", json!({"workspace_id": created["result"]["id"], "path": repo.path()})).await;
    assert_eq!(project["ok"], true, "{project}");
    let session = call(ws, "session.create", json!({"project_id": project["result"]["id"], "provider": "claude"})).await;
    assert_eq!(session["ok"], true, "{session}");
    let name = session["result"]["name"].as_str().unwrap().to_string();
    let spawned = call(ws, "session.spawn", json!({"session": name})).await;
    assert_eq!(spawned["ok"], true, "{spawned}");
    (name, repo)
}

/// Read until the `pty` frames for `session` seen so far, decoded, contain `want`.
async fn pty_until(ws: &mut Ws, session: &str, want: &str) {
    use base64::Engine as _;
    let mut seen = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !String::from_utf8_lossy(&seen).contains(want) {
            let line = recv_json(ws).await;
            if line["stream"] == "pty" && line["session"] == session {
                seen.extend(base64::engine::general_purpose::STANDARD.decode(line["data"].as_str().unwrap()).unwrap());
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {want:?} in the terminal: {:?}", String::from_utf8_lossy(&seen)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_terminal_streams_to_the_phone_directly_and_through_a_rendezvous() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let server = RendezvousServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let (rendezvous, code) = Registry::update(&h.ctx.registry_path, |r| {
        let rendezvous = r.set_rendezvous(&format!("ws://{}", server.local_addr)).clone();
        Ok::<_, anyhow::Error>((rendezvous, r.begin_pair(false).code))
    })
    .unwrap();
    let tunnel = relay_remote::tunnel::spawn(h.ctx.clone(), rendezvous.clone());
    await_hosts(server.local_addr, 1).await;
    let url = format!("ws://{}", door.local_addr);
    let join = relay_remote::tunnel::join_url(&rendezvous);
    let (device, token, _) = pair(&url, &code).await;
    let mut direct = admit(&url, &device, &token).await;
    let (name, _repo) = live_session(&h, &mut direct).await;
    let mut joined = admit(&join, &device, &token).await;

    // What the terminal screen does: attach, read what is there, type, read the answer. The
    // frames are read straight off the socket, so none is consumed waiting for a response.
    for (ws, typed) in [(&mut direct, "via-lan"), (&mut joined, "via-rendezvous")] {
        send(ws, json!({"v":1,"id":uuid_v4(),"actor":"user","op":"session.attach","payload":{"session": name}}).to_string()).await;
        pty_until(ws, &name, "hello-from-pty").await;
        send(ws, json!({"v":1,"id":uuid_v4(),"actor":"user","op":"session.input","payload":{"session": name, "data": format!("{typed}\n")}}).to_string()).await;
        pty_until(ws, &name, &format!("echo:{typed}")).await;
    }
    let closed = call(&mut direct, "session.close", json!({"session": name})).await;
    assert_eq!(closed["ok"], true, "{closed}");
    tunnel.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_is_let_go_when_the_engine_goes() {
    // An engine that takes the bridge's connection and then goes away, as one that restarts does.
    let dir = tempfile::tempdir().unwrap();
    let socket_path = dir.path().join("engine.sock");
    let engine = tokio::net::UnixListener::bind(&socket_path).unwrap();
    let ctx = Arc::new(Ctx {
        instance: Instance::Test,
        registry_path: dir.path().join("remote.json"),
        socket_path,
        version: "test".into(),
        start_engine: None,
        ops: relay_remote::wire::OpProfile::Phone,
    });
    let door = DirectServer::bind(ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let (device, token, _) = pair(&url, &pair_code(&ctx)).await;
    // Every connection is closed as soon as it is taken: the pairing's desktop announcement,
    // and the bridge's own once the phone is admitted.
    let engine = tokio::spawn(async move {
        while let Ok((conn, _)) = engine.accept().await {
            drop(conn);
        }
    });
    let mut ws = admit(&url, &device, &token).await;
    assert!(closes_within(&mut ws, Duration::from_secs(5)).await, "the phone stayed connected to no engine");
    engine.abort();
}

/// The login service's door (`relay remote serve --start-engine`, deploy/relay-door.service):
/// with no engine running, a paired phone is told so in the greeting and, once admitted, the door
/// starts one and carries the phone's requests to it. A stand-in engine plays the part here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_door_that_may_start_the_engine_starts_it_for_a_phone() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let socket_path = dir.path().join("engine.sock");
    let pid_path = dir.path().join("engine.pid");
    let relay = dir.path().join("relay");
    std::fs::write(
        &relay,
        format!(
            r#"#!/usr/bin/env python3
import json, os, socket, threading
open({pid:?}, "w").write(str(os.getpid()))
s = socket.socket(socket.AF_UNIX)
s.bind({sock:?})
s.listen(8)
def serve(c):
    f = c.makefile("rwb")
    for line in f:
        req = json.loads(line)
        f.write((json.dumps({{"v": 1, "id": req["id"], "ok": True, "result": {{"op": req["op"]}}}}) + "\n").encode())
        f.flush()
while True:
    c, _ = s.accept()
    threading.Thread(target=serve, args=(c,), daemon=True).start()
"#,
            pid = pid_path.display().to_string(),
            sock = socket_path.display().to_string(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&relay, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ctx = Arc::new(Ctx {
        instance: Instance::Test,
        registry_path: dir.path().join("remote.json"),
        socket_path: socket_path.clone(),
        version: "test".into(),
        start_engine: Some(relay),
        ops: relay_remote::wire::OpProfile::Phone,
    });
    let door = DirectServer::bind(ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let url = format!("ws://{}", door.local_addr);
    let (device, token, greeting) = pair(&url, &pair_code(&ctx)).await;
    assert_eq!(greeting.engine.as_deref(), Some("stopped"));
    assert!(!socket_path.exists(), "pairing alone must not start the engine");

    let mut ws = admit(&url, &device, &token).await;
    let resp = call(&mut ws, "bus.ping", json!({})).await;
    assert_eq!(resp["ok"], json!(true), "{resp}");
    assert_eq!(resp["result"]["op"], json!("bus.ping"));
    // A second phone finds it running.
    let (mut ws2, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let again: Greeting = serde_json::from_str(&recv_text(&mut ws2).await).unwrap();
    assert_eq!(again.engine.as_deref(), Some("running"));
    assert!(dir.path().join("engine.log").exists());

    let pid = std::fs::read_to_string(&pid_path).unwrap();
    let _ = std::process::Command::new("kill").arg(pid.trim()).status();
}

/// Tally's sync, as the phone runs it (apps/tally `PcSync`): pair, prove, send the whole ledger
/// with `replace` the first time, then trade changes by cursor. Everything else in the money
/// namespace stays behind the door.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tally_syncs_its_ledger_through_the_door() {
    let h = harness().await;
    let door = DirectServer::bind(h.ctx.clone(), "127.0.0.1:0".parse::<SocketAddr>().unwrap()).await.unwrap();
    let url = format!("ws://{}", door.local_addr);
    let (device, token, _) = pair(&url, &pair_code(&h.ctx)).await;
    let mut ws = admit(&url, &device, &token).await;

    let first = call(&mut ws, "money.sync", json!({"device": "Test Phone", "replace": true, "since": 0, "changes": [
        {"table": "transactions", "uid": "t1", "updated_at": 100, "deleted": false, "row": {"type": "EXPENSE", "amount": 4_250,
            "date": "2026-10-05", "account": "a1", "toAccount": null, "category": null, "note": "Metro", "recurring": null, "createdAt": 100}},
        {"table": "accounts", "uid": "a1", "updated_at": 100, "deleted": false, "row": {"name": "Chequing", "type": "CHEQUING",
            "openingBalance": 100_000, "archived": false, "sortOrder": 0}},
        {"table": "settings", "uid": "currency", "updated_at": 100, "deleted": false, "row": {"value": "CAD"}},
        // An investment the phone imported travels the same way (docs/INVESTMENTS.md).
        {"table": "accounts", "uid": "ws:HQ7XFMC41CAD", "updated_at": 100, "deleted": false, "row": {"name": "Wealthsimple TFSA",
            "type": "INVESTMENT", "openingBalance": 0, "archived": false, "sortOrder": 1, "registration": "TFSA",
            "institution": "Wealthsimple", "externalRef": "HQ7XFMC41CAD"}},
        {"table": "securities", "uid": "sec:XEQT", "updated_at": 100, "deleted": false, "row": {"symbol": "XEQT",
            "name": "iShares Core Equity ETF Portfolio", "currency": "CAD", "kind": "ETF", "exchange": "TSX"}},
        {"table": "activities", "uid": "imp:aaa32c148f3cea50442cb54fcf9cf2be", "updated_at": 100, "deleted": false, "row": {
            "account": "ws:HQ7XFMC41CAD", "security": "sec:XEQT", "type": "BUY", "date": "2026-08-08", "quantity": 1_000_000_000,
            "amount": 38_120, "fee": 0, "currency": "CAD", "toAmount": null, "toCurrency": null, "note": "", "source": "WEALTHSIMPLE",
            "createdAt": 100}}
    ]})).await;
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["result"]["applied"], 6);
    let cursor = first["result"]["cursor"].clone();

    // The PC sees the phone's ledger, and an entry made there goes back on the next sync.
    let home = at_the_pc(&h, "money.summary", json!({"today": "2026-10-07"}));
    assert_eq!((home["spent"].as_i64(), home["accounts"][0]["balance"].as_i64()), (Some(4_250), Some(95_750)));
    let invest = at_the_pc(&h, "money.invest.summary", json!({"today": "2026-10-07"}));
    let xeqt = &invest["holdings"][0];
    assert_eq!((xeqt["symbol"].as_str(), xeqt["quantity"].as_i64(), xeqt["book"].as_i64()), (Some("XEQT"), Some(1_000_000_000), Some(38_120)));
    assert_eq!(invest["accounts"][0]["registration"], "TFSA");
    at_the_pc(&h, "money.tx.add", json!({"type": "INCOME", "amount": 210_000, "date": "2026-10-06", "account_id": home["accounts"][0]["id"]}));
    let next = call(&mut ws, "money.sync", json!({"device": "Test Phone", "since": cursor, "changes": []})).await;
    let changes = next["result"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{next}");
    assert_eq!((changes[0]["row"]["account"].as_str(), changes[0]["row"]["amount"].as_i64()), (Some("a1"), Some(210_000)));

    // Relay's phone app reads the ledger for its Threads space; what it shows is what the PC has.
    let read = call(&mut ws, "money.summary", json!({"today": "2026-10-07"})).await;
    assert_eq!(read["result"]["spent"].as_i64(), Some(4_250), "{read}");
    // Beyond syncing, a phone only undoes an entry a thread added (`money.tx.delete` / `restore`);
    // it adds nothing directly, and wipes or replaces nothing.
    for op in ["money.reset", "money.import", "money.tx.add", "money.invest.import", "money.settings.set"] {
        let refused = call(&mut ws, op, json!({})).await;
        assert_eq!(refused["error"]["code"], "remote.op", "{op}: {refused}");
    }
}

/// docs/CLOUD.md end to end: a cloud engine behind a rendezvous takes Tally's ledger and Avex's
/// export while the PC is away, offers nothing else, and the PC's link brings both home and
/// sends the PC's own entries back.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cloud_copy_takes_the_phones_data_and_the_pc_link_brings_it_home() {
    let pc = harness().await;
    let cloud = harness_with(relay_remote::wire::OpProfile::Cloud).await;
    let server = RendezvousServer::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap()).await.unwrap();
    let rendezvous = Registry::update(&cloud.ctx.registry_path, |r| {
        Ok::<_, anyhow::Error>(r.set_rendezvous(&format!("ws://{}", server.local_addr)).clone())
    })
    .unwrap();
    let join = relay_remote::tunnel::join_url(&rendezvous);
    let tunnel = relay_remote::tunnel::spawn(cloud.ctx.clone(), rendezvous.clone());
    await_hosts(server.local_addr, 1).await;

    // Tally pairs with the cloud copy and sends its ledger; Avex sends its export.
    let (device, token, _) = pair(&join, &pair_code(&cloud.ctx)).await;
    let mut phone = admit(&join, &device, &token).await;
    let first = call(&mut phone, "money.sync", json!({"device": "Pixel", "replace": true, "since": 0, "changes": [
        {"table": "accounts", "uid": "a1", "updated_at": 100, "deleted": false, "row": {"name": "Chequing", "type": "CHEQUING",
            "openingBalance": 100_000, "archived": false, "sortOrder": 0}},
        {"table": "transactions", "uid": "t1", "updated_at": 100, "deleted": false, "row": {"type": "EXPENSE", "amount": 4_250,
            "date": "2026-10-05", "account": "a1", "toAccount": null, "category": null, "note": "Metro", "recurring": null, "createdAt": 100}},
    ]})).await;
    assert_eq!(first["ok"], true, "{first}");
    let cursor = first["result"]["cursor"].clone();
    let export = include_str!("../../relay-gym/tests/avex_export.json");
    let uploaded = call(&mut phone, "gym.upload", json!({"json": export})).await;
    assert_eq!(uploaded["ok"], true, "{uploaded}");

    // The cloud copy holds data and nothing else: no terminals, no files, no wiping.
    for op in ["session.spawn", "session.input", "file.write", "money.reset", "gym.import", "thread.send"] {
        let refused = call(&mut phone, op, json!({})).await;
        assert_eq!(refused["error"]["code"], "remote.op", "{op}: {refused}");
    }

    // The PC pairs with the cloud through the link the server printed, and merges.
    let code = pair_code(&cloud.ctx);
    let registry = Registry::load(&cloud.ctx.registry_path).unwrap();
    let link_url = relay_remote::pairlink::PairLink::build(&registry, "test", &code, 7420, &[]).to_url();
    let link_path = pc.ctx.registry_path.with_file_name("cloud.json");
    let link = relay_remote::cloud::pair(&link_path, &link_url, "PC (test)").await.unwrap();
    assert_eq!(link.url, join);

    let report = relay_remote::cloud::round(&pc.ctx.socket_path, &link_path, "PC (test)").await.unwrap();
    assert!(report.gym, "{report:?}");
    let home = at_the_pc(&pc, "money.summary", json!({"today": "2026-10-07"}));
    assert_eq!(home["spent"].as_i64(), Some(4_250), "Tally's entry reached the PC: {home}");
    let gym = at_the_pc(&pc, "gym.summary", json!({}));
    assert_eq!(gym["imported"]["exported_at"], "2026-10-08 21:14", "{gym}");

    // An entry made on the PC reaches the phone through the cloud copy.
    at_the_pc(&pc, "money.tx.add", json!({"type": "INCOME", "amount": 210_000, "date": "2026-10-06", "account_id": home["accounts"][0]["id"]}));
    let report = relay_remote::cloud::round(&pc.ctx.socket_path, &link_path, "PC (test)").await.unwrap();
    assert_eq!((report.to_cloud, report.gym), (1, false), "{report:?}");
    let next = call(&mut phone, "money.sync", json!({"device": "Pixel", "since": cursor, "changes": []})).await;
    let changes = next["result"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{next}");
    assert_eq!(changes[0]["row"]["amount"].as_i64(), Some(210_000));

    // Nothing new: a quiet round.
    let quiet = relay_remote::cloud::round(&pc.ctx.socket_path, &link_path, "PC (test)").await.unwrap();
    assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.gym), (0, 0, false), "{quiet:?}");
    let saved = relay_remote::cloud::Link::load(&link_path).unwrap().unwrap();
    assert!(saved.last_ok.is_some() && saved.last_error.is_none());

    tunnel.abort();
}
