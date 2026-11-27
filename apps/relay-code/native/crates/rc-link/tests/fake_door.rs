//! The link against a stand-in door that speaks the phone door's handshake (crates/relay-remote's
//! wire.rs) on loopback: pairing, then connecting with no Connect key, answering calls, coming
//! back by itself after the door drops it, and stopping when the PC forgets the phone. Runs in
//! every `cargo test`; `live_door.rs` does the same against a real door.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use rc_link::link::Profile;
use rc_link::{Link, LinkState, Note, PairLink, Route, Store};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

const DEVICE: &str = "dev-1";
const TOKEN: &str = "tok-1";

struct Door {
    port: u16,
    /// Close every open connection now (the PC went away for a moment).
    drop_all: Arc<AtomicBool>,
    /// Refuse the phone's proof from now on (the PC forgot it).
    forget: Arc<AtomicBool>,
    connections: Arc<AtomicUsize>,
}

/// Who a door is and how it behaves.
#[derive(Clone)]
struct Who {
    instance: &'static str,
    host_id: &'static str,
    /// Whether it holds this phone's pairing.
    knows_phone: bool,
    ping_delay: Duration,
    refuse_subscribe: bool,
}

/// The phone's own PC.
const PC: Who = Who { instance: "test", host_id: "h1", knows_phone: true, ping_delay: Duration::ZERO, refuse_subscribe: false };

fn door() -> Door {
    door_as(PC)
}

fn door_as(who: Who) -> Door {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let port = std_listener.local_addr().unwrap().port();
    let drop_all = Arc::new(AtomicBool::new(false));
    let forget = Arc::new(AtomicBool::new(false));
    let connections = Arc::new(AtomicUsize::new(0));
    let (d, f, c) = (drop_all.clone(), forget.clone(), connections.clone());
    std::thread::spawn(move || {
        rt.block_on(async move {
            let listener = TcpListener::from_std(std_listener).unwrap();
            loop {
                let (mut tcp, _) = listener.accept().await.unwrap();
                let (d, f, c, who) = (d.clone(), f.clone(), c.clone(), who.clone());
                tokio::spawn(async move {
                    // `GET /info` on the same port, as the real door answers it.
                    let mut head = [0u8; 9];
                    let mut n = 0;
                    for _ in 0..50 {
                        n = tcp.peek(&mut head).await.unwrap_or(0);
                        if n >= head.len() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    if &head[..n] == b"GET /info" {
                        use tokio::io::AsyncWriteExt;
                        let body = json!({"relay": "remote", "host": "desk", "host_id": who.host_id, "instance": who.instance}).to_string();
                        let _ = tcp.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}").as_bytes()).await;
                        let _ = tcp.shutdown().await;
                        return;
                    }
                    let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    let challenge = format!("ch-{n}");
                    let greet = json!({"relay": "remote", "v": 1, "challenge": challenge, "instance": who.instance, "engine": "running", "host": "", "host_id": "", "version": ""});
                    if ws.send(Message::Text(greet.to_string().into())).await.is_err() {
                        return;
                    }
                    let Some(Ok(Message::Text(hello))) = ws.next().await else { return };
                    let hello: Value = serde_json::from_str(hello.as_str()).unwrap();
                    let welcome = if hello.get("pair").and_then(Value::as_str) == Some("CODE1234") {
                        json!({"ok": true, "device": DEVICE, "token": TOKEN, "host": "desk", "host_id": "h1", "ops": "phone"})
                    } else if who.knows_phone && !f.load(Ordering::SeqCst) && hello["device"] == DEVICE && hello["proof"] == rc_link::wire::proof(&challenge, TOKEN) {
                        json!({"ok": true, "host": "desk", "host_id": who.host_id, "version": "test", "ops": "phone"})
                    } else {
                        json!({"ok": false, "error": "auth.unknown_device"})
                    };
                    let admitted = welcome["ok"] == true && hello.get("pair").is_none();
                    let _ = ws.send(Message::Text(welcome.to_string().into())).await;
                    if !admitted {
                        return;
                    }
                    loop {
                        if d.load(Ordering::SeqCst) {
                            return;
                        }
                        let msg = tokio::time::timeout(Duration::from_millis(50), ws.next()).await;
                        let Ok(msg) = msg else { continue };
                        let Some(Ok(Message::Text(line))) = msg else { return };
                        let req: Value = serde_json::from_str(line.as_str()).unwrap();
                        if req["op"] == "bus.ping" && !who.ping_delay.is_zero() {
                            tokio::time::sleep(who.ping_delay).await;
                        }
                        let result = match req["op"].as_str() {
                            Some("session.list") => json!({"sessions": [{"name": "a-b", "project_id": 1, "provider": "claude", "state": "running"}]}),
                            Some("bus.subscribe") => json!({"subscribed": req["payload"]["events"]}),
                            _ => json!({}),
                        };
                        // The engine reads ids as UUIDs and writes them back hyphenated: an id in
                        // any other form comes back looking different, as from the real door.
                        let hexid: String = req["id"].as_str().unwrap_or_default().chars().filter(|c| *c != '-').collect();
                        let id = if hexid.len() == 32 { format!("{}-{}-{}-{}-{}", &hexid[..8], &hexid[8..12], &hexid[12..16], &hexid[16..20], &hexid[20..]) } else { String::new() };
                        let res = if who.refuse_subscribe && req["op"] == "bus.subscribe" {
                            json!({"v": 1, "id": id, "ok": false, "error": {"kind": "internal", "code": "bus.subscribe", "message": "no"}})
                        } else {
                            json!({"v": 1, "id": id, "ok": true, "result": result})
                        };
                        if ws.send(Message::Text(res.to_string().into())).await.is_err() {
                            return;
                        }
                        // Something happens on the PC right after the phone subscribes.
                        if req["op"] == "bus.subscribe" {
                            let ev = json!({"v": 1, "ev": "session.changed", "ts": "t", "project_id": 1, "payload": {"name": "a-b", "state": "idle"}});
                            if ws.send(Message::Text(ev.to_string().into())).await.is_err() {
                                return;
                            }
                        }
                    }
                });
            }
        });
    });
    Door { port, drop_all, forget, connections }
}

#[derive(Default)]
struct Memory(std::sync::Mutex<Option<Profile>>);

impl Store for Memory {
    fn load(&self) -> Option<Profile> {
        self.0.lock().unwrap().clone()
    }
    fn save(&self, p: &Profile) {
        *self.0.lock().unwrap() = Some(p.clone());
    }
    fn forget(&self) {
        *self.0.lock().unwrap() = None;
    }
}

fn wait_for(link: &Link, what: &str, timeout: Duration, ok: impl Fn(&LinkState) -> bool) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if ok(&link.state()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{what}: still {:?} after {timeout:?}", link.state());
}

#[test]
fn paired_it_connects_by_itself_answers_and_comes_back_after_a_drop() {
    let d = door();
    let store = Arc::new(Memory::default());
    let link = Link::start(store.clone(), Arc::new(|| {}));
    assert_eq!(link.state(), LinkState::Unpaired, "nothing to dial before pairing");

    // Pairing is the person's last step: the link connects with no Connect key.
    let url = format!("ws://127.0.0.1:{}", d.port);
    let pl = PairLink::parse(&format!("relay://pair?v=1&code=CODE1234&direct={}", url.replace(':', "%3A").replace('/', "%2F"))).unwrap();
    let p = rc_link::link::pair(&pl, "test phone").expect("paired");
    assert_eq!((p.device.as_str(), p.token.as_str(), p.name.as_str()), (DEVICE, TOKEN, "desk"));
    link.paired(&p);
    wait_for(&link, "online after pairing", Duration::from_secs(5), LinkState::online);

    let v = link.call("session.list", json!({})).wait(Duration::from_secs(5)).expect("an answer");
    assert_eq!(v["sessions"][0]["name"], "a-b");

    // The PC drops every connection: the link is back by itself, with nothing tapped.
    let before = d.connections.load(Ordering::SeqCst);
    d.drop_all.store(true, Ordering::SeqCst);
    wait_for(&link, "noticed the drop", Duration::from_secs(5), |s| !s.online());
    d.drop_all.store(false, Ordering::SeqCst);
    wait_for(&link, "back online by itself", Duration::from_secs(10), LinkState::online);
    assert!(d.connections.load(Ordering::SeqCst) > before);

    // A new start of the app (the stored pairing alone) connects too.
    let again = Link::start(store, Arc::new(|| {}));
    wait_for(&again, "online on the next start", Duration::from_secs(5), LinkState::online);

    // The PC forgot this phone: the link stops and says so instead of dialing forever.
    d.forget.store(true, Ordering::SeqCst);
    d.drop_all.store(true, Ordering::SeqCst);
    wait_for(&link, "told to pair again", Duration::from_secs(10), |s| matches!(s, LinkState::Revoked { .. }));
}

#[test]
fn a_route_that_does_not_answer_gives_way_to_the_next() {
    let d = door();
    // A LAN address nothing listens on comes first, as a phone away from home has.
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = dead.local_addr().unwrap().port();
    drop(dead);
    let p = Profile {
        host_id: "h1".into(),
        name: "desk".into(),
        instance: "test".into(),
        device: DEVICE.into(),
        token: TOKEN.into(),
        routes: vec![Route::new(&format!("ws://127.0.0.1:{dead_port}")), Route::new(&format!("ws://127.0.0.1:{}", d.port))],
        last_route: None,
        paired_at_ms: 0,
        version: String::new(),
    };
    let store = Arc::new(Memory::default());
    store.save(&p);
    let link = Link::start(store.clone(), Arc::new(|| {}));
    wait_for(&link, "online through the second route", Duration::from_secs(10), LinkState::online);
    assert_eq!(store.load().unwrap().last_route.as_deref(), Some(format!("ws://127.0.0.1:{}", d.port).as_str()), "the route that worked is kept");
}

#[test]
fn a_listener_beside_the_app_hears_every_event_and_state_and_takes_none() {
    let d = door();
    let p = Profile {
        host_id: "h1".into(),
        name: "desk".into(),
        instance: "test".into(),
        device: DEVICE.into(),
        token: TOKEN.into(),
        routes: vec![Route::new(&format!("ws://127.0.0.1:{}", d.port))],
        last_route: None,
        paired_at_ms: 0,
        version: String::new(),
    };
    let store = Arc::new(Memory::default());
    store.save(&p);
    let link = Link::start(store, Arc::new(|| {}));
    let heard = link.listen();
    let mut notes = Vec::new();
    let start = Instant::now();
    while !notes.iter().any(|n| matches!(n, Note::Event(_))) && start.elapsed() < Duration::from_secs(10) {
        if let Ok(n) = heard.recv_timeout(Duration::from_millis(100)) {
            notes.push(n);
        }
    }
    assert!(notes.iter().any(|n| matches!(n, Note::State(s) if s.online())), "online is heard: {notes:?}");
    let Some(Note::Event(e)) = notes.iter().find(|n| matches!(n, Note::Event(_))) else { panic!("no event heard: {notes:?}") };
    assert_eq!((e.ev.as_str(), e.payload["state"].as_str()), ("session.changed", Some("idle")));
    // The app's own buffer still has it.
    assert_eq!(link.take_events().iter().map(|e| e.ev.as_str()).collect::<Vec<_>>(), ["session.changed"]);
}

fn profile(routes: &[&Door], last: Option<&Door>) -> Profile {
    Profile {
        host_id: "h1".into(),
        name: "desk".into(),
        instance: "test".into(),
        device: DEVICE.into(),
        token: TOKEN.into(),
        routes: routes.iter().map(|d| Route::new(&format!("ws://127.0.0.1:{}", d.port))).collect(),
        last_route: last.map(|d| format!("ws://127.0.0.1:{}", d.port)),
        paired_at_ms: 0,
        version: String::new(),
    }
}

/// Every state the link shows for `long`, polled.
fn states_for(link: &Link, long: Duration) -> Vec<LinkState> {
    let start = Instant::now();
    let mut seen: Vec<LinkState> = Vec::new();
    while start.elapsed() < long {
        let s = link.state();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    seen
}

#[test]
fn a_wrong_door_on_the_way_is_passed_over_and_never_unpairs() {
    let pc = door();
    // The dev instance at the address that worked last, and another PC with the same instance
    // name: neither knows this phone.
    let dev = door_as(Who { instance: "dev", host_id: "h1", knows_phone: false, ..PC });
    let other = door_as(Who { host_id: "h2", knows_phone: false, ..PC });
    let store = Arc::new(Memory::default());
    store.save(&profile(&[&dev, &other, &pc], Some(&dev)));
    let link = Link::start(store.clone(), Arc::new(|| {}));
    wait_for(&link, "online through the PC's own door", Duration::from_secs(10), LinkState::online);
    let LinkState::Online { route, .. } = link.state() else { unreachable!() };
    assert_eq!(route.url, format!("ws://127.0.0.1:{}", pc.port));
    assert_eq!(store.load().unwrap().last_route.as_deref(), Some(route.url.as_str()), "the right door leads next time");

    // Only another PC answers: offline, still paired, dialing again.
    let store = Arc::new(Memory::default());
    store.save(&profile(&[&other], None));
    let link = Link::start(store, Arc::new(|| {}));
    let seen = states_for(&link, Duration::from_secs(3));
    assert!(!seen.iter().any(|s| matches!(s, LinkState::Revoked { .. } | LinkState::Online { .. })), "{seen:?}");
    assert!(seen.iter().any(|s| matches!(s, LinkState::Offline { reason, .. } if reason.contains("Another PC"))), "{seen:?}");
}

#[test]
fn pairing_again_mid_connect_drops_the_old_pairings_session() {
    // The old PC's engine is slow to answer the first ping.
    let old = door_as(Who { ping_delay: Duration::from_millis(1500), ..PC });
    let new = door();
    let store = Arc::new(Memory::default());
    store.save(&profile(&[&old], None));
    let link = Link::start(store, Arc::new(|| {}));
    let start = Instant::now();
    while old.connections.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(300));
    link.paired(&profile(&[&new], None));
    let seen = states_for(&link, Duration::from_secs(4));
    let to_old = format!("ws://127.0.0.1:{}", old.port);
    assert!(!seen.iter().any(|s| matches!(s, LinkState::Online { route, .. } if route.url == to_old)), "the old PC never shows: {seen:?}");
    assert!(matches!(link.state(), LinkState::Online { route, .. } if route.url == format!("ws://127.0.0.1:{}", new.port)), "{seen:?}");
}

#[test]
fn a_subscribe_the_pc_refuses_is_a_reconnect_not_online() {
    let d = door_as(Who { refuse_subscribe: true, ..PC });
    let store = Arc::new(Memory::default());
    store.save(&profile(&[&d], None));
    let link = Link::start(store, Arc::new(|| {}));
    let seen = states_for(&link, Duration::from_secs(3));
    assert!(!seen.iter().any(LinkState::online), "{seen:?}");
    assert!(d.connections.load(Ordering::SeqCst) >= 2, "it dials again");
}
