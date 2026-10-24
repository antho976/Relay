//! `design.render` at the socket door (render-SPEC-v2 §5.1-§5.4, render-door): the desktop app's
//! lease on a connection of its own, the render an agent or the person asks for, checked against
//! the store before anything is sent, the one targeted request line, the wait that holds no lock,
//! and every way it ends (the answer, the timeout, the app or the agent leaving).
//!
//! A fake desktop app plays the renderer: a user connection that serves, reads its own
//! `design.render.request` lines and answers on the same connection.

mod common;

use common::{committed_repo, engine_with_project, ok};
use relay_bus::ops::design::{RenderRequest, RENDER_BOX_FIELDS};
use relay_bus::{Actor, Event, Request, Response};
use relay_core::engine::Engine;
use relay_core::socket::{Client, Line, SocketServer};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A 1×1 PNG.
const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
/// Longer than any answer a test waits for, far shorter than a render's timeout: what "at once"
/// means on a machine busy building, where the alternative is a wait of seconds.
const AT_ONCE: Duration = Duration::from_secs(1);
/// How long a test listens for a line that must not come.
const QUIET: Duration = Duration::from_millis(300);

/// An engine with project 1 (and its agent `agent`) and project 2 (its agent `bystander`), on a
/// socket, and design `design` in project 1: `page-1` ("Page 1") holding a card with a title,
/// and `checkout` ("Checkout") holding a frame.
struct Fx {
    engine: Arc<Engine>,
    server: SocketServer,
    root: tempfile::TempDir,
    design: i64,
    agent: Agent,
    bystander: Agent,
}

#[derive(Clone)]
struct Agent {
    name: String,
    token: String,
}

impl Agent {
    fn request(&self, op: &str, payload: Value) -> Request {
        Request::new(Actor::agent(&self.name), op, payload).with_token(&self.token)
    }
}

async fn fixture() -> Fx {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    let other = ws.join("other");
    committed_repo(&other, &[("README.md", "# other\n")]);
    ok(&engine, "project.add", json!({"workspace_id": 1, "path": other}));
    let agent = agent_of(&engine, 1);
    let bystander = agent_of(&engine, 2);
    let design = ok(&engine, "design.create", json!({"project_id": 1, "title": "Shop"}))["design"]["id"].as_i64().unwrap();
    ok(&engine, "design.apply", json!({"design_id": design, "ops": [
        {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "frame", "w": 320, "h": 200}},
        {"op": "add", "node": {"id": "title", "parent": "card", "kind": "text", "text": "Hello"}},
        {"op": "add", "node": {"id": "checkout", "kind": "page", "name": "Checkout"}},
        {"op": "add", "node": {"id": "pay", "parent": "checkout", "kind": "frame", "w": 200, "h": 80}},
    ]}));
    let server = SocketServer::start_in(engine.clone(), root.path().join("run")).await.unwrap();
    Fx { engine, server, root, design, agent, bystander }
}

/// A new agent session of project `project`, with the token the socket door asks of it.
fn agent_of(engine: &Engine, project: i64) -> Agent {
    let name = ok(engine, "session.create", json!({"project_id": project, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    let token: String = engine.store.lock().query_row("SELECT token FROM sessions WHERE name = ?1", [&name], |r| r.get(0)).unwrap();
    Agent { name, token }
}

impl Fx {
    async fn connect(&self) -> Client {
        Client::connect(&self.server.path).await.unwrap()
    }

    fn renders_dir(&self) -> PathBuf {
        self.root.path().join("store/design-renders")
    }

    fn rev(&self) -> i64 {
        ok(&self.engine, "design.get", json!({"design_id": self.design, "pages_only": true}))["design"]["rev"].as_i64().unwrap()
    }

    /// `request` sent on a connection of its own, answered whenever the door answers it.
    async fn send(&self, request: Request) -> Call {
        let client = self.connect().await;
        Call(tokio::spawn(async move {
            let mut client = client;
            client.call(&request, |_| {}).await.unwrap()
        }))
    }

    /// `request` on a connection of its own, and its answer, which must come at once.
    async fn at_once(&self, request: Request) -> Response {
        let started = Instant::now();
        let response = self.send(request).await.within(AT_ONCE).await;
        assert!(started.elapsed() < AT_ONCE, "answered after {:?}", started.elapsed());
        response
    }

    /// The agent's `design.render` with `payload` (its design unless the payload names one).
    fn render(&self, mut payload: Value) -> Request {
        if payload.get("design_id").is_none() {
            payload["design_id"] = json!(self.design);
        }
        self.agent.request("design.render", payload)
    }

    /// The person's `design.render`.
    fn render_as_user(&self, mut payload: Value) -> Request {
        if payload.get("design_id").is_none() {
            payload["design_id"] = json!(self.design);
        }
        Request::new(Actor::User, "design.render", payload)
    }
}

/// A request on its way: the door answers it later.
struct Call(tokio::task::JoinHandle<Response>);

impl Call {
    async fn within(self, limit: Duration) -> Response {
        tokio::time::timeout(limit, self.0).await.expect("the door answered in time").unwrap()
    }
    async fn answer(self) -> Response {
        self.within(Duration::from_secs(10)).await
    }
    /// The connection goes away mid-call, as an MCP call the agent cancelled.
    fn leave(self) {
        self.0.abort();
    }
}

/// A fake desktop app: a user connection that serves and keeps the request lines it reads,
/// including those that arrive while it waits for an answer of its own.
struct Renderer {
    client: Client,
    lines: VecDeque<Event>,
}

impl Renderer {
    async fn serve(fx: &Fx) -> Renderer {
        Renderer::serve_with(fx, json!({})).await
    }

    async fn serve_with(fx: &Fx, payload: Value) -> Renderer {
        let mut renderer = Renderer { client: fx.connect().await, lines: VecDeque::new() };
        let served = renderer.call(Request::new(Actor::User, "design.render.serve", payload)).await;
        assert!(served.ok, "{:?}", served.error);
        assert_eq!(served.result, Some(json!({})));
        renderer
    }

    async fn call(&mut self, request: Request) -> Response {
        let lines = &mut self.lines;
        self.client.call(&request, |ev| lines.push_back(ev)).await.unwrap()
    }

    /// The next request line, which must come.
    async fn request(&mut self) -> RenderRequest {
        let ev = match self.lines.pop_front() {
            Some(ev) => ev,
            None => loop {
                match tokio::time::timeout(Duration::from_secs(10), self.client.next()).await.expect("a request line").unwrap() {
                    Some(Line::Event(ev)) => break ev,
                    Some(Line::Frame(_)) => continue,
                    other => panic!("not a request line: {:?}", other.map(|_| "a response")),
                }
            },
        };
        assert_eq!((ev.v, ev.ev.as_str(), &ev.actor), (1, "design.render.request", &Actor::System));
        serde_json::from_value(ev.payload).unwrap()
    }

    /// No request line comes.
    async fn quiet(&mut self) {
        assert!(self.lines.is_empty(), "{:?}", self.lines);
        if let Ok(line) = tokio::time::timeout(QUIET, self.client.next()).await {
            panic!("a line came: {}", match line.unwrap() {
                Some(Line::Event(ev)) => format!("{} {}", ev.ev, ev.payload),
                Some(_) => "a response or frame".into(),
                None => "the end of the connection".into(),
            });
        }
    }

    async fn answer(&mut self, answer: Value) -> bool {
        let response = self.call(Request::new(Actor::User, "design.render.answer", answer)).await;
        assert!(response.ok, "{:?}", response.error);
        response.result.unwrap()["accepted"].as_bool().unwrap()
    }

    /// Answer `request` with a 1×1 picture and one row per id in `rows`, drawn at `rev`.
    async fn draw(&mut self, request: &RenderRequest, rev: i64, rows: &[&str]) -> bool {
        self.answer(json!({"render_id": request.render_id, "drawn": drawn(rev, rows)})).await
    }

    async fn fail(&mut self, request: &RenderRequest, code: &str, message: &str) -> bool {
        self.answer(json!({"render_id": request.render_id, "error": {"code": code, "message": message}})).await
    }
}

fn drawn(rev: i64, rows: &[&str]) -> Value {
    let rows: Vec<Value> = rows.iter().map(|id| json!([id, "frame", "page-1", 0, 0, 320, 200, 0, 0, 0])).collect();
    json!({
        "rev": rev, "area": {"x": -32, "y": -32, "w": 1, "h": 1}, "scale": 1.0, "png_b64": PNG_1X1,
        "boxes": {"fields": RENDER_BOX_FIELDS, "rows": rows, "total": rows.len(), "hidden": 0},
        "issues": [], "issue_count": 0, "warnings": [],
        "timings_ms": {"read": 1.0, "layout": 1.0, "paint": 1.0, "encode": 1.0},
    })
}

fn error_of(response: &Response) -> &relay_bus::BusError {
    response.error.as_ref().unwrap_or_else(|| panic!("refused, not {:?}", response.result))
}

fn code_of(response: &Response) -> &str {
    error_of(response).code.as_str()
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

/// 1. With no desktop app connected, the agent hears so at once, with what to do instead, and
/// nothing is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_renderer_is_refused_at_once_with_its_hint() {
    let fx = fixture().await;
    let refused = fx.at_once(fx.render(json!({}))).await;
    let error = error_of(&refused);
    assert_eq!(error.code, "design.render.no_renderer");
    assert!(error.hint.as_deref().unwrap().contains("do not guess sizes"), "{error:?}");
    assert!(!fx.renders_dir().exists());
}

/// 2. Only the person's clients lease and answer; an agent's tool list holds neither, and holds
/// design.render as something it may call.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serve_and_answer_are_the_users() {
    let fx = fixture().await;
    let mut client = fx.connect().await;
    for (op, payload) in [("design.render.serve", json!({})), ("design.render.answer", json!({"render_id": "x", "error": {"code": "c", "message": "m"}}))] {
        let refused = client.call(&fx.agent.request(op, payload), |_| {}).await.unwrap();
        assert_eq!(code_of(&refused), "actor.allowlist", "{op}");
    }
    let listed = client.call(&fx.agent.request("bus.ops", json!({})), |_| {}).await.unwrap().result.unwrap();
    let op = |name: &str| listed["ops"].as_array().unwrap().iter().find(|op| op["name"] == name).cloned();
    assert_eq!(op("design.render.serve"), None);
    assert_eq!(op("design.render.answer"), None);
    let render = op("design.render").expect("listed");
    assert_eq!((render["call"].as_str(), render["implemented"].as_bool()), (Some("yes"), Some(true)), "{render}");
    // The door checks the envelope and the actor before anything else, as every door op's.
    let tokenless = client.call(&Request::new(Actor::agent(&fx.agent.name), "design.render", json!({"design_id": fx.design})), |_| {}).await.unwrap();
    assert_eq!(code_of(&tokenless), "bus.actor");
    let stray = client.call(&fx.render(json!({"rev": 3})), |_| {}).await.unwrap();
    assert_eq!(code_of(&stray), "bus.schema");
    let system = client.call(&Request::new(Actor::System, "design.render.serve", json!({})), |_| {}).await.unwrap();
    assert_eq!(code_of(&system), "bus.actor");
    // Only a connection renders or leases: the engine itself answers neither.
    for op in ["design.render", "design.render.serve"] {
        let payload = if op == "design.render" { json!({"design_id": fx.design}) } else { json!({}) };
        assert_eq!(common::code(common::call(&fx.engine, op, payload)), "bus.door", "{op}");
    }
}

/// 3. The round trip: the line names the resolved page, the asker and a deadline; the agent gets
/// the picture's path in the render folder, its size, the rows and the rev drawn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_round_trip_returns_the_picture_rows_and_rev() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let rev = fx.rev();
    let before = unix_ms();
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    assert_eq!(request.design_id, fx.design);
    assert_eq!(request.page, "page-1", "the first page, by its id");
    assert_eq!((request.nodes.as_ref(), request.png, request.scale, request.max_side, request.boxes_limit), (None, true, 1.0, 1568, 200));
    assert_eq!((request.at_ms, request.frames, request.by.as_deref()), (None, None, Some(fx.agent.name.as_str())));
    let deadline = request.deadline_unix_ms - before;
    assert!((19_500..=21_000).contains(&deadline), "the timeout less half a second from the call: {deadline}");
    assert!(uuid::Uuid::parse_str(&request.render_id).is_ok());

    assert!(renderer.draw(&request, rev, &["card", "title"]).await, "a render was waiting");
    let response = call.answer().await;
    assert!(response.ok, "{:?}", response.error);
    let out = response.result.unwrap();
    assert_eq!((out["design_id"].as_i64(), out["rev"].as_i64(), out["page"].as_str()), (Some(fx.design), Some(rev), Some("page-1")));
    let path = PathBuf::from(out["png"]["path"].as_str().unwrap());
    assert_eq!(path, fx.renders_dir().join(format!("{}.png", request.render_id)));
    assert_eq!((out["png"]["w"].as_u64(), out["png"]["h"].as_u64()), (Some(1), Some(1)));
    assert_eq!(std::fs::read(&path).unwrap()[..8], *b"\x89PNG\r\n\x1a\n");
    assert_eq!(out["boxes"]["rows"].as_array().unwrap().iter().map(|row| row[0].clone()).collect::<Vec<_>>(), [json!("card"), json!("title")]);
    assert_eq!(out["boxes"]["fields"], json!(RENDER_BOX_FIELDS));

    // The options asked reach the app as asked; a motion time and a contact sheet each alone.
    let call = fx.send(fx.render(json!({"png": false, "scale": 0.5, "max_side": 640, "boxes_limit": 0, "timeout_ms": 5000, "at_ms": 250}))).await;
    let request = renderer.request().await;
    assert_eq!((request.png, request.scale, request.max_side, request.boxes_limit, request.at_ms, request.frames), (false, 0.5, 640, 0, Some(250.0), None));
    assert!(request.deadline_unix_ms - unix_ms() <= 4_500);
    renderer.fail(&request, "design.render.failed", "this desktop app cannot play motion yet").await;
    assert_eq!(code_of(&call.answer().await), "design.render.failed");
    let call = fx.send(fx.render(json!({"frames": 6}))).await;
    let request = renderer.request().await;
    assert_eq!((request.at_ms, request.frames), (None, Some(6)));
    renderer.fail(&request, "design.render.failed", "no").await;
    call.answer().await;
    let both = fx.at_once(fx.render(json!({"frames": 6, "at_ms": 0}))).await;
    assert_eq!(code_of(&both), "design.render.args");
    assert!(error_of(&both).message.starts_with("give at_ms or frames, not both"));
    let out_of_range = fx.at_once(fx.render(json!({"scale": 4}))).await;
    assert_eq!((code_of(&out_of_range), error_of(&out_of_range).message.as_str()), ("design.render.args", "scale: 4 is outside 0.25 to 2"));
    renderer.quiet().await;
}

/// 4. The request line goes to the leased connection alone: never to a subscriber, never to an
/// agent's bus.wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_request_is_not_broadcast() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let mut everything = fx.connect().await;
    let subscribed = everything.call(&Request::new(Actor::User, "bus.subscribe", json!({})), |_| {}).await.unwrap();
    assert!(subscribed.ok);
    let waiting = fx.send(fx.agent.request("bus.wait", json!({"events": ["design.render.request"], "timeout_ms": 2000}))).await;
    // The bystander's wait, whose scope admits events of no project, as this line would be.
    let bystanding = fx.send(fx.bystander.request("bus.wait", json!({"events": ["design.*"], "timeout_ms": 2000}))).await;

    // Nothing says when the waits have subscribed, so render throughout them: a line broadcast
    // after either subscribed would wake it.
    for asker in 0..4 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let call = fx.send(if asker % 2 == 0 { fx.render_as_user(json!({})) } else { fx.render(json!({})) }).await;
        let request = renderer.request().await;
        assert!(renderer.draw(&request, fx.rev(), &[]).await);
        assert!(call.answer().await.ok);
    }

    for wait in [waiting, bystanding] {
        let woken = wait.answer().await.result.unwrap();
        assert_eq!(woken["timed_out"], true, "{woken}");
    }
    while let Ok(line) = tokio::time::timeout(QUIET, everything.next()).await {
        if let Some(Line::Event(ev)) = line.unwrap() {
            assert_ne!(ev.ev, "design.render.request", "{}", ev.payload);
        }
    }
}

/// 5. A renderer that closes cleanly takes its lease with it at once: the next render is
/// `no_renderer`, not sent to it. One that closes mid-render ends that render.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_clean_close_releases_the_lease() {
    let fx = fixture().await;
    let renderer = Renderer::serve(&fx).await;
    drop(renderer);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(code_of(&fx.at_once(fx.render(json!({}))).await), "design.render.no_renderer");

    let mut renderer = Renderer::serve(&fx).await;
    let call = fx.send(fx.render(json!({}))).await;
    renderer.request().await;
    drop(renderer);
    let gone = call.within(AT_ONCE).await;
    assert_eq!(code_of(&gone), "design.render.renderer_gone");
    assert!(error_of(&gone).hint.as_deref().unwrap().contains("once it is open"));
}

/// 6. Two apps: the newest draws; when it goes, the older one takes over.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_newest_lease_draws_and_the_older_takes_over() {
    let fx = fixture().await;
    let mut older = Renderer::serve(&fx).await;
    let mut newest = Renderer::serve(&fx).await;
    let call = fx.send(fx.render(json!({}))).await;
    let request = newest.request().await;
    older.quiet().await;
    assert!(newest.draw(&request, fx.rev(), &[]).await);
    assert!(call.answer().await.ok);

    drop(newest);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let call = fx.send(fx.render(json!({}))).await;
    let request = older.request().await;
    assert!(older.draw(&request, fx.rev(), &[]).await);
    assert!(call.answer().await.ok);

    // Serving again makes a lease the newest once more.
    let mut newer = Renderer::serve(&fx).await;
    let served = older.call(Request::new(Actor::User, "design.render.serve", json!({"fonts": ["Geist", "Sora"]}))).await;
    assert!(served.ok);
    let call = fx.send(fx.render(json!({}))).await;
    let request = older.request().await;
    newer.quiet().await;
    assert!(older.draw(&request, fx.rev(), &[]).await);
    assert!(call.answer().await.ok);
}

/// 7. An app that never answers: the agent hears so at its timeout, typed, and an answer after
/// that is not accepted and writes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_timeout_is_typed_and_a_late_answer_is_not_accepted() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let started = Instant::now();
    let call = fx.send(fx.render(json!({"timeout_ms": 2000}))).await;
    let request = renderer.request().await;
    let timed_out = call.answer().await;
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(2000), "{waited:?}");
    let error = error_of(&timed_out);
    assert_eq!(error.code, "design.render.timeout");
    assert!(error.details.as_ref().unwrap()["waited_ms"].as_u64().unwrap() >= 1_900, "{error:?}");
    assert!(error.hint.as_deref().unwrap().contains("carry on without the picture"));
    assert!(!renderer.draw(&request, fx.rev(), &[]).await, "nobody waits for it now");
    assert!(!fx.renders_dir().exists(), "a late answer writes no picture");
}

/// 8. Four renders wait at most; a fifth is told to come back, and nothing is sent for it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fifth_render_is_busy() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let mut calls = Vec::new();
    let mut requests = Vec::new();
    for _ in 0..4 {
        calls.push(fx.send(fx.render(json!({}))).await);
        requests.push(renderer.request().await);
    }
    let busy = fx.at_once(fx.render(json!({}))).await;
    assert_eq!(code_of(&busy), "design.render.busy");
    assert!(error_of(&busy).hint.as_deref().unwrap().contains("again in a few seconds"));
    renderer.quiet().await;
    for request in &requests {
        assert!(renderer.draw(request, fx.rev(), &[]).await);
    }
    for call in calls {
        assert!(call.answer().await.ok);
    }
}

/// 9. An agent sees only its own project's designs; another's is refused before anything is sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn another_projects_design_is_refused_before_any_push() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let refused = fx.at_once(fx.bystander.request("design.render", json!({"design_id": fx.design}))).await;
    assert_eq!(code_of(&refused), "actor.scope");
    let missing = fx.at_once(fx.render(json!({"design_id": 999}))).await;
    assert_eq!(code_of(&missing), "design.not_found");
    renderer.quiet().await;
}

/// 10. A design that moved past the agent's `base_rev` is refused before anything is sent, with
/// the rev it is at; at its rev, it is drawn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_base_rev_is_refused_before_any_push() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let rev = fx.rev();
    let stale = fx.at_once(fx.render(json!({"base_rev": rev - 1}))).await;
    let error = error_of(&stale);
    assert_eq!((error.code.as_str(), error.details.clone()), ("design.stale", Some(json!({"rev": rev}))));
    assert!(error.hint.as_deref().unwrap().contains("design.get"));
    renderer.quiet().await;

    let call = fx.send(fx.render(json!({"base_rev": rev}))).await;
    let request = renderer.request().await;
    // The design moved on while the app drew: the rev it drew is checked too.
    assert!(renderer.draw(&request, rev + 1, &[]).await);
    assert_eq!(code_of(&call.answer().await), "design.stale");
    let call = fx.send(fx.render(json!({"base_rev": rev}))).await;
    let request = renderer.request().await;
    assert!(renderer.draw(&request, rev, &[]).await);
    assert_eq!(call.answer().await.result.unwrap()["rev"].as_i64(), Some(rev));
}

/// 11. The nodes a render narrows to: at most 200, all on one page, each in the design, on the
/// page asked for; the page defaults to theirs, and each is sent once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_scope_is_checked() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let many: Vec<String> = (0..201).map(|i| format!("n{i}")).collect();
    let refused = |response: Response| (error_of(&response).code.clone(), error_of(&response).message.clone(), error_of(&response).hint.clone());

    let (code, message, _) = refused(fx.at_once(fx.render(json!({"nodes": many}))).await);
    assert_eq!((code.as_str(), message.as_str()), ("design.render.args", "nodes: 201 of them, over 200"));
    let (code, message, hint) = refused(fx.at_once(fx.render(json!({"nodes": ["card", "pay"]}))).await);
    assert_eq!(code, "design.render.args");
    assert_eq!(message, r#"nodes: "pay" is on page "checkout" and "card" on page "page-1"; a render pictures one page"#);
    assert!(hint.unwrap().contains("a call of its own"));
    let (code, message, hint) = refused(fx.at_once(fx.render(json!({"nodes": ["card", "titel"]}))).await);
    assert_eq!((code.as_str(), message.as_str()), ("design.node_not_found", &*format!(r#"nodes: design {} has no node "titel""#, fx.design)));
    assert_eq!(hint.as_deref(), Some(r#"did you mean "title"?"#));
    let (code, _, hint) = refused(fx.at_once(fx.render(json!({"nodes": ["zzzzzzzzzz"]}))).await);
    assert_eq!((code.as_str(), hint.as_deref()), ("design.node_not_found", Some("design.get lists the ids (pass page to read one page)")));
    let (code, message, _) = refused(fx.at_once(fx.render(json!({"page": "checkout", "nodes": ["title"]}))).await);
    assert_eq!(code, "design.render.args");
    assert_eq!(message, r#"nodes: "title" is on page "page-1", not on the page asked for ("checkout")"#);
    let (code, _, _) = refused(fx.at_once(fx.render(json!({"nodes": []}))).await);
    assert_eq!(code, "design.render.args");
    renderer.quiet().await;

    // Nodes of one page: that page, each node once, in the order first named.
    let call = fx.send(fx.render(json!({"nodes": ["pay", "checkout", "pay"]}))).await;
    let request = renderer.request().await;
    assert_eq!((request.page.as_str(), request.nodes.clone()), ("checkout", Some(vec!["pay".to_string(), "checkout".into()])));
    renderer.fail(&request, "design.node_not_found", "gone").await;
    assert_eq!(code_of(&call.answer().await), "design.node_not_found", "the app's own refusal keeps its code");
    let call = fx.send(fx.render(json!({"page": "Page 1", "nodes": ["title"]}))).await;
    let request = renderer.request().await;
    assert_eq!(request.page, "page-1");
    renderer.fail(&request, "design.render.failed", "no").await;
    call.answer().await;
}

/// 11b. `since_rev` pictures what changed after it: the outermost nodes changed on the page most
/// of them are on, the other pages named in a warning; a page asked for keeps to its own; with
/// nothing changed, the whole page. Not with `nodes`, nor a rev the design has not reached.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn since_rev_pictures_what_changed() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let before = fx.rev();
    ok(&fx.engine, "design.apply", json!({"design_id": fx.design, "ops": [
        {"op": "update", "id": "title", "set": {"text": "Hi"}},
        {"op": "add", "node": {"id": "badge", "parent": "card", "kind": "frame", "w": 20, "h": 20}},
        {"op": "add", "node": {"id": "dot", "parent": "badge", "kind": "ellipse", "w": 4, "h": 4}},
        {"op": "add", "node": {"id": "receipt", "parent": "pay", "kind": "text", "text": "Paid"}},
    ]}));
    let call = fx.send(fx.render(json!({"since_rev": before}))).await;
    let request = renderer.request().await;
    assert_eq!((request.page.as_str(), request.nodes.clone()), ("page-1", Some(vec!["badge".to_string(), "title".into()])), "dot comes inside badge");
    assert!(renderer.draw(&request, fx.rev(), &[]).await);
    let answer = call.answer().await;
    let warnings = answer.result.unwrap()["warnings"].clone();
    assert_eq!(warnings, json!([format!(r#"since_rev {before}: page "checkout" changed too; picture it with page "checkout" and since_rev"#)]));

    let call = fx.send(fx.render(json!({"since_rev": before, "page": "Checkout"}))).await;
    let request = renderer.request().await;
    assert_eq!((request.page.as_str(), request.nodes.clone()), ("checkout", Some(vec!["receipt".to_string()])));
    assert!(renderer.draw(&request, fx.rev(), &[]).await);
    assert!(call.answer().await.result.unwrap()["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains(r#"page "page-1""#)));

    let call = fx.send(fx.render(json!({"since_rev": fx.rev()}))).await;
    let request = renderer.request().await;
    assert_eq!((request.page.as_str(), request.nodes.clone()), ("page-1", None), "nothing changed: the whole first page");
    assert!(renderer.draw(&request, fx.rev(), &[]).await);
    assert_eq!(call.answer().await.result.unwrap()["warnings"], json!([]));

    assert_eq!(code_of(&fx.at_once(fx.render(json!({"since_rev": before, "nodes": ["card"]}))).await), "design.render.args");
    assert_eq!(code_of(&fx.at_once(fx.render(json!({"since_rev": fx.rev() + 1}))).await), "design.render.args");

    // A list that only deleted leaves no row to picture: the answer says so.
    let before = fx.rev();
    ok(&fx.engine, "design.apply", json!({"design_id": fx.design, "ops": [{"op": "delete", "id": "receipt"}]}));
    let call = fx.send(fx.render(json!({"since_rev": before}))).await;
    let request = renderer.request().await;
    assert!(renderer.draw(&request, fx.rev(), &[]).await);
    let warnings = call.answer().await.result.unwrap()["warnings"].clone();
    assert!(warnings.to_string().contains("only removals since (1)"), "{warnings}");
    renderer.quiet().await;
}

/// 12. The app's own failure reaches the agent: a missing surface with what to do, any other
/// code as a failed render with the app's code beside its reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_app_failure_reaches_the_agent() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    assert!(renderer.fail(&request, "design.render.no_surface", "the window has no renderer").await);
    let response = call.answer().await;
    let error = error_of(&response);
    assert_eq!((error.code.as_str(), error.message.as_str()), ("design.render.no_surface", "the window has no renderer"));
    assert!(error.hint.as_deref().unwrap().contains("png: false"));

    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    assert!(renderer.fail(&request, "gtk.cairo", "cairo ran out of memory").await);
    let response = call.answer().await;
    let error = error_of(&response);
    assert_eq!(error.code, "design.render.failed");
    assert_eq!(error.details, Some(json!({"reason": "cairo ran out of memory", "app_code": "gtk.cairo"})));

    // A drawn answer that does not hold up is a failed render too, with its reason.
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    let mut bad = drawn(fx.rev(), &["card"]);
    bad["png_b64"] = json!("not base64 at all");
    assert!(renderer.answer(json!({"render_id": request.render_id, "drawn": bad})).await);
    let response = call.answer().await;
    assert_eq!(code_of(&response), "design.render.failed");
    assert!(error_of(&response).details.as_ref().unwrap()["reason"].as_str().unwrap().contains("png_b64"));
}

/// 13. A render waiting for the app holds no lock: the store answers everyone else meanwhile.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_store_is_free_while_a_render_waits() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    let listed = fx.at_once(fx.agent.request("design.list", json!({}))).await;
    assert!(listed.ok, "{:?}", listed.error);
    let changed = fx.at_once(Request::new(Actor::User, "design.update", json!({"design_id": fx.design, "title": "Shop 2"}))).await;
    assert!(changed.ok, "{:?}", changed.error);
    renderer.draw(&request, fx.rev(), &[]).await;
    assert!(call.answer().await.ok);
}

/// 14. An agent that leaves mid-render frees its place at once: four new renders all wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_that_leaves_frees_its_slot() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let mut gone = Vec::new();
    for _ in 0..4 {
        let call = fx.send(fx.render(json!({}))).await;
        gone.push(renderer.request().await);
        call.leave();
    }
    // Each new render reaches the app; one sent before an old place is freed is told busy, and
    // sent again.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut waiting = Vec::new();
    while waiting.len() < 4 {
        assert!(Instant::now() < deadline, "the places of the agents that left were not freed");
        let mut call = fx.send(fx.render(json!({}))).await;
        tokio::select! {
            request = renderer.request() => waiting.push((call, request)),
            response = &mut call.0 => {
                assert_eq!(code_of(&response.unwrap()), "design.render.busy");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    // The renders whose agents left take no answer.
    for request in &gone {
        assert!(!renderer.draw(request, fx.rev(), &[]).await);
    }
    for (call, request) in waiting {
        assert!(renderer.draw(&request, fx.rev(), &[]).await);
        assert!(call.answer().await.ok);
    }
}

/// 15. The answer takes no store lock: with the store held throughout, it is accepted and the
/// agent has its picture before the store is let go.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_needs_no_store_lock() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let rev = fx.rev();
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let holder = {
        let (engine, released) = (fx.engine.clone(), released.clone());
        std::thread::spawn(move || {
            let held = engine.store.lock();
            held_tx.send(()).unwrap();
            std::thread::sleep(Duration::from_secs(2));
            released.store(true, std::sync::atomic::Ordering::SeqCst);
            drop(held);
        })
    };
    held_rx.recv().unwrap();
    assert!(renderer.draw(&request, rev, &["card"]).await);
    let response = call.answer().await;
    assert!(!released.load(std::sync::atomic::Ordering::SeqCst), "the answer waited for the store");
    assert!(response.ok, "{:?}", response.error);
    holder.join().unwrap();
}

/// 16. An answer naming a render nobody sent is not accepted, and the render waiting meanwhile
/// is untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_render_id_is_not_accepted() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    let mut stranger = Renderer { client: fx.connect().await, lines: VecDeque::new() };
    for render_id in [uuid::Uuid::new_v4().to_string(), "not a render id".to_string()] {
        assert!(!stranger.answer(json!({"render_id": render_id, "drawn": drawn(1, &[])})).await);
    }
    assert!(renderer.draw(&request, fx.rev(), &["card"]).await);
    assert!(call.answer().await.ok);
}

/// 17. A deleted design is drawn, as design.get reads it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deleted_design_renders() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    ok(&fx.engine, "design.delete", json!({"design_id": fx.design}));
    let call = fx.send(fx.render(json!({}))).await;
    let request = renderer.request().await;
    assert!(renderer.draw(&request, fx.rev(), &[]).await);
    assert!(call.answer().await.ok);
}

/// 18. The person renders any project's design, and the line names no asker.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_person_renders_with_no_project_check() {
    let fx = fixture().await;
    let mut renderer = Renderer::serve(&fx).await;
    let theirs = ok(&fx.engine, "design.create", json!({"project_id": 2, "title": "Other"}))["design"]["id"].as_i64().unwrap();
    let call = fx.send(fx.render_as_user(json!({"design_id": theirs}))).await;
    let request = renderer.request().await;
    assert_eq!((request.design_id, request.page.as_str(), request.by.as_deref()), (theirs, "page-1", None));
    assert!(renderer.draw(&request, 1, &[]).await);
    assert_eq!(call.answer().await.result.unwrap()["design_id"].as_i64(), Some(theirs));
}

/// 19. `page` is a page's id or the name it goes by, in design.get and design.render alike; an
/// unknown page, or a name two pages share, is refused with the pages to choose from.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_page_by_name_resolves_and_an_unknown_page_lists_the_pages() {
    let fx = fixture().await;
    ok(&fx.engine, "design.apply", json!({"design_id": fx.design, "ops": [
        {"op": "add", "node": {"id": "twin-a", "kind": "page", "name": "Twin"}},
        {"op": "add", "node": {"id": "twin-b", "kind": "page", "name": "Twin"}},
    ]}));
    let mut renderer = Renderer::serve(&fx).await;
    let got = ok(&fx.engine, "design.get", json!({"design_id": fx.design, "page": "Checkout"}));
    assert_eq!(got["nodes"][0]["id"], "checkout");
    let call = fx.send(fx.render(json!({"page": "Checkout"}))).await;
    let request = renderer.request().await;
    assert_eq!(request.page, "checkout");
    renderer.fail(&request, "design.render.failed", "no").await;
    call.answer().await;

    for page in ["Nope", "Twin"] {
        let read = common::call(&fx.engine, "design.get", json!({"design_id": fx.design, "page": page}));
        let rendered = fx.at_once(fx.render(json!({"page": page}))).await;
        let (read, rendered) = (read.error.unwrap(), error_of(&rendered).clone());
        assert_eq!(rendered.code, "design.page_not_found", "{page}");
        assert_eq!((&rendered.message, &rendered.hint), (&read.message, &read.hint), "one rule in both reads: {page}");
        let hint = rendered.hint.unwrap();
        match page {
            "Twin" => {
                assert_eq!(rendered.message, r#"2 pages are named "Twin""#);
                assert_eq!(hint, r#"pages: twin-a ("Twin"), twin-b ("Twin"); pass a page's id"#);
            }
            _ => assert!(hint.starts_with(r#"pages: page-1 ("Page 1"), checkout ("Checkout"), twin-a ("Twin")"#), "{hint}"),
        }
    }
    renderer.quiet().await;

    // A design with no page has nothing to picture. design.apply keeps a design's last page
    // (design.last_page), so only a store written elsewhere holds one.
    fx.engine.store.lock().execute("DELETE FROM design_nodes WHERE design_id = ?1", [fx.design]).unwrap();
    let empty = fx.at_once(fx.render(json!({}))).await;
    assert_eq!(code_of(&empty), "design.page_not_found");
    assert!(error_of(&empty).hint.as_deref().unwrap().contains("add one with design.apply"));
    renderer.quiet().await;
}

/// W68: design.fonts lists what the newest connected desktop app reported; an agent's apply
/// naming a family it lacks (on a node, a text run or a style) is warned of once and applied;
/// one it has, or a generic one, is not.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fonts_come_from_the_app_and_an_unknown_one_warns() {
    let fx = fixture().await;
    let _app = Renderer::serve_with(&fx, json!({"fonts": ["Geist", "Inter Display", "Sora"]})).await;
    let fonts = fx.at_once(fx.agent.request("design.fonts", json!({"name": "inter"}))).await;
    assert_eq!(fonts.result, Some(json!({"families": ["Inter Display"], "complete": true})));
    let apply = |ops: Value| fx.agent.request("design.apply", json!({"design_id": fx.design, "ops": ops}));
    let out = fx.at_once(apply(json!([
        {"op": "update", "id": "title", "set": {"font": "Inter"}},
        {"op": "add", "node": {"id": "sub", "parent": "card", "kind": "text", "text": "Hi there", "font": "inter display",
            "runs": [{"start": 0, "end": 2, "font": "Inter"}, {"start": 3, "end": 8, "font": "Comic"}]}},
        {"op": "style", "name": "body", "set": {"font": "monospace"}},
        {"op": "style", "name": "head", "set": {"font": "Papyrus"}},
    ]))).await;
    assert!(out.ok, "{:?}", out.error);
    let warnings = out.result.unwrap()["warnings"].clone();
    assert_eq!(warnings, json!([
        "ops[0] update \"title\": font \"Inter\" (title) is not on the desktop that draws this design; it is drawn in a fallback face. design.fonts lists what is there",
        "ops[1] add \"sub\": font \"Comic\" (sub) is not on the desktop that draws this design; it is drawn in a fallback face. design.fonts lists what is there",
        "ops[3] style \"head\": font \"Papyrus\" (style \"head\") is not on the desktop that draws this design; it is drawn in a fallback face. design.fonts lists what is there",
    ]), "once per family; a run's font and a style's count; a generic family does not");
}
