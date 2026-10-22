//! MCP stdio door (BUS.md §6.4). This is transport and argument shaping only: every tool
//! call is still a normal request through the socket door and the engine pipeline.

use crate::units;
use anyhow::{Context, Result};
use relay_bus::registry::{Op as _, OpEntry, OpKind, Registry};
use relay_bus::{Actor, MailHint, Request};
use relay_core::socket::Client;
use relay_core::Instance;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use tokio::sync::mpsc;

const FALLBACK_PROTOCOL: &str = "2025-06-18";
const SUPPORTED_PROTOCOLS: &[&str] = &["2026-07-28", "2025-11-25", FALLBACK_PROTOCOL];
/// Images one plugin call may return; each is a full PNG in the agent's context.
const MAX_IMAGES: usize = 16;

/// The protocol revision to answer an `initialize` with: the client's, when this side speaks it.
fn protocol(message: &Value) -> &'static str {
    let requested = message.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(FALLBACK_PROTOCOL);
    SUPPORTED_PROTOCOLS.iter().copied().find(|version| *version == requested).unwrap_or(FALLBACK_PROTOCOL)
}

pub async fn serve(instance: Instance, actor: Actor, token: Option<String>) -> Result<u8> {
    let output = Output::stdout();
    let watcher = tokio::spawn(watch_tools(instance, actor.clone(), token.clone(), output.clone()));
    let code = dispatch(stdin_lines(), output, move |message| {
        let (actor, token) = (actor.clone(), token.clone());
        async move { handle(instance, &actor, token.as_deref(), message).await }
    })
    .await;
    watcher.abort();
    code
}

/// MCP's own word that the tools changed, which the client answers with a new `tools/list`.
const TOOLS_CHANGED: &str = "notifications/tools/list_changed";

/// Whether `event` changes what this session may call: a plugin switched on or off for its
/// project (a session with no project hears every project's).
fn changes_tools(event: &relay_bus::Event, project: Option<i64>) -> bool {
    event.ev == "plugin.changed"
        && (project.is_none() || event.project_id.or_else(|| event.payload["project_id"].as_i64()).is_none_or(|p| Some(p) == project))
}

/// Tell the client when this session's tools change: a plugin turned on mid-session brings its
/// ops (Video's `video.*`), and one turned off takes them, without a restart. One subscription
/// on its own connection, for as long as the server runs; a client that never asks again is no
/// worse off than before.
async fn watch_tools(instance: Instance, actor: Actor, token: Option<String>, output: Output) {
    let ask = |op: &str, payload: Value| {
        let request = Request::new(actor.clone(), op, payload);
        match &token {
            Some(token) => request.with_token(token.clone()),
            None => request,
        }
    };
    let Ok(mut client) = connect(instance).await else { return };
    let project = client.call(&ask("bus.whoami", json!({})), |_| {}).await.ok()
        .and_then(|r| r.result).and_then(|r| r["project_id"].as_i64());
    match client.call(&ask("bus.subscribe", json!({"events": ["plugin.changed"]})), |_| {}).await {
        Ok(response) if response.ok => {}
        _ => return,
    }
    while let Ok(Some(line)) = client.next().await {
        if let relay_core::socket::Line::Event(event) = line {
            if changes_tools(&event, project) {
                let _ = output.send(&json!({"jsonrpc":"2.0","method":TOOLS_CHANGED}));
            }
        }
    }
}

/// The bus server's loop (RA-061). Each request runs as its own task, so one long `bus.wait`
/// no longer queues every later call behind it, and answers go out whole lines at a time in
/// whatever order they finish. `notifications/cancelled` aborts the call's task: the client
/// gets no answer, as MCP asks, and the dropped socket ends a wait on the engine side. The
/// engine itself serializes what needs it, so nothing here has to stay in order.
async fn dispatch<F, Fut>(mut lines: mpsc::UnboundedReceiver<std::io::Result<String>>, output: Output, handler: F) -> Result<u8>
where
    F: Fn(Value) -> Fut,
    Fut: Future<Output = Option<Value>> + Send + 'static,
{
    let running: Arc<Mutex<HashMap<String, tokio::task::AbortHandle>>> = Arc::default();
    while let Some(line) = lines.recv().await {
        let Some(message) = parse_line(&line.context("reading MCP stdin")?, &output)? else { continue };
        if let Some(id) = cancelled(&message) {
            if let Some(task) = lock(&running).remove(&id) {
                task.abort();
            }
            continue;
        }
        // Other notifications never receive a response.
        let Some(key) = message.get("id").map(Value::to_string) else { continue };
        let call = handler(message);
        let (out, table, mine) = (output.clone(), running.clone(), key.clone());
        // Held across the spawn, so a call that finishes at once still finds itself listed.
        let mut listed = lock(&running);
        let task = tokio::spawn(async move {
            let response = call.await;
            if lock(&table).remove(&mine).is_some() {
                if let Some(response) = response {
                    let _ = out.send(&response);
                }
            }
        });
        listed.insert(key, task.abort_handle());
    }
    // The client has gone; nobody is left to read an answer.
    for (_, task) in lock(&running).drain() {
        task.abort();
    }
    Ok(0)
}

/// Whether a plugin tool call may run beside others or must wait its turn.
pub(crate) enum Lane {
    Parallel,
    /// One at a time per server, e.g. everything that drives the one Unreal editor.
    Serial,
}

/// The loop of the synchronous plugin servers (`blender-mcp`, `unreal-mcp`; RA-061). Every
/// `tools/call` runs on its own thread, so a render or a build no longer blocks the server;
/// `lane` keeps the calls that share one external thing (an editor) in order. A cancelled call
/// that has not started is skipped; one already running is a subprocess or an editor request
/// this side cannot interrupt, so it runs to its own timeout and its answer is dropped.
pub(crate) fn serve_sync(handle: fn(&Value) -> Option<Value>, lane: fn(&str, &Value) -> Lane) -> Result<u8> {
    serve_sync_on(std::io::stdin().lock(), Output::stdout(), handle, lane)
}

fn serve_sync_on(input: impl BufRead, output: Output, handle: fn(&Value) -> Option<Value>, lane: fn(&str, &Value) -> Lane) -> Result<u8> {
    // Request id -> cancelled.
    let running: Arc<Mutex<HashMap<String, bool>>> = Arc::default();
    let serial = Arc::new(Mutex::new(()));
    let mut threads: Vec<std::thread::JoinHandle<()>> = Vec::new();
    for line in input.lines() {
        let Some(message) = parse_line(&line.context("reading MCP stdin")?, &output)? else { continue };
        if let Some(id) = cancelled(&message) {
            if let Some(flag) = lock(&running).get_mut(&id) {
                *flag = true;
            }
            continue;
        }
        let Some(key) = message.get("id").map(Value::to_string) else { continue };
        if message["method"] != "tools/call" {
            if let Some(response) = handle(&message) {
                output.send(&response)?;
            }
            continue;
        }
        let name = message.pointer("/params/name").and_then(Value::as_str).unwrap_or_default();
        let turn = matches!(lane(name, message.pointer("/params/arguments").unwrap_or(&Value::Null)), Lane::Serial)
            .then(|| serial.clone());
        lock(&running).insert(key.clone(), false);
        let (out, table) = (output.clone(), running.clone());
        threads.retain(|thread| !thread.is_finished());
        threads.push(std::thread::spawn(move || {
            let _turn = turn.as_deref().map(lock);
            // Its own statement: the guard must be gone before the call runs.
            let wanted = lock(&table).get(&key) == Some(&false);
            let response = if wanted { handle(&message) } else { None };
            if lock(&table).remove(&key) == Some(false) {
                if let Some(response) = response {
                    let _ = out.send(&response);
                }
            }
        }));
    }
    // Calls already running finish, as they did when the loop handled them one by one.
    for thread in threads {
        let _ = thread.join();
    }
    Ok(0)
}

/// A plugin server (`blender-mcp`, `unreal-mcp`): who it is, its tools and how to run one.
/// [`plugin_reply`] is the JSON-RPC side they share, so a protocol revision or an error code
/// changes in one place (RA-590).
pub(crate) struct PluginServer {
    pub name: &'static str,
    pub title: &'static str,
    pub instructions: &'static str,
    pub tools: fn() -> Vec<Value>,
    pub call: fn(&str, &Value) -> Result<Value>,
}

/// The answer to one message for `server`, or `None` for a notification.
pub(crate) fn plugin_reply(server: &PluginServer, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let result = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => json!({
            "protocolVersion": protocol(message),
            "capabilities": {"tools":{"listChanged":false}},
            "serverInfo": {"name":server.name,"title":server.title,"version":env!("CARGO_PKG_VERSION")},
            "instructions": server.instructions
        }),
        Some("ping") => json!({}),
        Some("tools/list") => json!({"tools": (server.tools)()}),
        Some("tools/call") => {
            let Some(name) = message.pointer("/params/name").and_then(Value::as_str) else {
                return Some(rpc_error(id, -32602, "Invalid params", Some(json!({"message":"tools/call requires params.name"}))));
            };
            let arguments = message.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
            if !(server.tools)().iter().any(|tool| tool["name"] == name) {
                return Some(rpc_error(id, -32602, "Unknown tool", Some(json!({"name":name}))));
            }
            match (server.call)(name, &arguments) {
                Ok(value) => tool_result(value, false),
                Err(error) => tool_result(json!({"error": format!("{error:#}")}), true),
            }
        }
        Some(other) => return Some(rpc_error(id, -32601, "Method not found", Some(json!({"method":other})))),
        None => return Some(rpc_error(id, -32600, "Invalid Request", None)),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

/// A plugin tool's MCP description.
pub(crate) fn tool(name: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type":"object","properties":properties,"required":required,"additionalProperties":false},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": !read_only, "openWorldHint": false}
    })
}

/// A plugin call's result as MCP content. `_images` (label and picture path) become image
/// content of the type their bytes are, up to [`MAX_IMAGES`]; `_cleanup` names a folder of them
/// to delete once they are read.
pub(crate) fn tool_result(mut value: Value, is_error: bool) -> Value {
    let images = value.as_object_mut().and_then(|o| o.remove("_images")).and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let cleanup = value.as_object_mut().and_then(|o| o.remove("_cleanup"));
    let mut content = Vec::new();
    let mut shown = Vec::new();
    for image in images.iter().take(MAX_IMAGES) {
        let Some(path) = image["path"].as_str() else { continue };
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Some(media_type) = preview_type(&bytes, Path::new(path)) else { continue };
        content.push(json!({"type":"text","text":format!("Image: {}", image["label"].as_str().unwrap_or(""))}));
        content.push(image_content(&bytes, media_type));
        shown.push(image["label"].clone());
    }
    if !images.is_empty() {
        value["images"] = json!({"shown": shown, "requested": images.len(), "limit": MAX_IMAGES});
    }
    if let Some(dir) = cleanup.as_ref().and_then(Value::as_str).filter(|dir| relay_scratch(Path::new(dir))) {
        let _ = std::fs::remove_dir_all(dir);
    }
    let text = serde_json::to_string(&value).unwrap_or_default();
    content.insert(0, json!({"type":"text","text":text}));
    let mut result = json!({"content":content,"isError":is_error});
    if !is_error {
        result["structuredContent"] = value;
    }
    result
}

/// A plugin preview's media type, from its bytes (a capture may be a JPEG, whatever its name);
/// from its name when its header is not one this side reads; `None` for what a model cannot be
/// shown (SVG, EXR, text), which is then left out rather than sent under a type it is not (W73).
fn preview_type(bytes: &[u8], path: &Path) -> Option<&'static str> {
    match relay_core::picture::sniff(bytes) {
        Some(picture) => (picture.ext != "svg").then(|| picture.media_type()),
        None => match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "gif" => Some("image/gif"),
            "webp" => Some("image/webp"),
            _ => None,
        },
    }
}

/// Whether `dir` is a folder a plugin made for one call's images, so `_cleanup` may delete it: a
/// Blender render folder directly in the temp dir, or an Unreal capture folder
/// (`Saved/Relay/Captures/<uuid>`), named as Relay names them and not a symlink. Anything else
/// is left alone, whatever a result says: a folder named there by mistake, or by a script, is
/// not deleted outside every sandbox.
fn relay_scratch(dir: &Path) -> bool {
    use std::path::Component;
    let plain = dir.is_absolute() && dir.components().all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    let real_dir = std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_dir());
    let (Some(name), Some(parent)) = (dir.file_name().and_then(|n| n.to_str()), dir.parent()) else { return false };
    let named = |prefix: &str| name.strip_prefix(prefix).is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
    let render = named(crate::blender::RENDER_PREFIX) && parent == std::env::temp_dir();
    let capture = named("") && parent.ends_with("Saved/Relay/Captures");
    plain && real_dir && (render || capture)
}

fn stdin_lines() -> mpsc::UnboundedReceiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// A JSON-RPC message, or `None` for a blank line or one that was answered with a parse error.
fn parse_line(line: &str, output: &Output) -> Result<Option<Value>> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    match serde_json::from_str::<Value>(line) {
        Ok(message) => Ok(Some(message)),
        Err(error) => {
            output.send(&rpc_error(Value::Null, -32700, "Parse error", Some(json!({"message":error.to_string()}))))?;
            Ok(None)
        }
    }
}

/// The request a `notifications/cancelled` gives up on, keyed like a request id.
fn cancelled(message: &Value) -> Option<String> {
    (message["method"] == "notifications/cancelled" && message.get("id").is_none())
        .then(|| message.pointer("/params/requestId").map(Value::to_string))
        .flatten()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Stdout shared by every call in flight; each answer is written and flushed as one line under
/// the lock, so concurrent answers never interleave.
#[derive(Clone)]
struct Output(Arc<Mutex<Box<dyn Write + Send>>>);

impl Output {
    fn stdout() -> Self {
        Self(Arc::new(Mutex::new(Box::new(std::io::BufWriter::new(std::io::stdout())))))
    }

    fn send(&self, message: &Value) -> Result<()> {
        let mut out = lock(&self.0);
        serde_json::to_writer(&mut *out, message)?;
        out.write_all(b"\n")?;
        out.flush()?;
        Ok(())
    }
}

static PLUGIN_INSTANCE: OnceLock<Instance> = OnceLock::new();

/// The engine instance a plugin server's `relay --instance …` names; its guardrail is the one
/// that counts for this session.
pub(crate) fn use_instance(instance: Instance) {
    let _ = PLUGIN_INSTANCE.set(instance);
}

fn plugin_instance() -> Result<Instance> {
    if let Some(instance) = PLUGIN_INSTANCE.get() {
        return Ok(*instance);
    }
    let instance = std::env::var("RELAY_INSTANCE").unwrap_or_else(|_| "stable".into());
    Instance::parse(&instance).ok_or_else(|| anyhow::anyhow!("bad RELAY_INSTANCE {instance:?}"))
}

/// Plugin tools that run agent-supplied code (`blender_python`, `ue_python`, `ue_console`, …)
/// meet `guardrail.gate` before they run, as a Bash call does through the PreToolUse hook
/// (RA-077): an `exec` gate naming the tool, so a denied-command rule such as `blender_python`
/// or `ue_console` refuses it, a grant lifts that, and every call is on the audit record; and a
/// `write` gate for each file the tool says it will write, so protected paths and write roots
/// apply to a save.
///
/// What this cannot do: the code is arbitrary Python running as the user. It can write, delete
/// or run anything without declaring it, and no gate sees inside it — matching its source
/// against command patterns would only catch `rm -rf` spelled as shell, never `shutil.rmtree`.
/// The gate decides whether the tool runs at all and checks what it declares; it is not a
/// sandbox.
///
/// Outside a Relay session (no `RELAY_SESSION`) there is no guardrail to ask and the plugin runs
/// as a plain MCP server. Inside one, an engine that does not answer blocks the call.
pub(crate) fn plugin_gate(tool: &str, file: Option<&Path>, writes: &[&Path]) -> Result<()> {
    let Some(session) = std::env::var("RELAY_SESSION").ok().filter(|s| !s.is_empty()) else { return Ok(()) };
    let root = std::env::var_os("RELAY_WORKTREE").map(PathBuf::from).or_else(|| std::env::current_dir().ok());
    let payloads = plugin_gate_payloads(&session, root.as_deref(), tool, file, writes);
    if cfg!(test) {
        // Unit tests run inside agent sessions; they must not reach the live engine.
        return Ok(());
    }
    let instance = plugin_instance()?;
    let (actor, token) = crate::actor_from_env(None)?;
    // These servers are synchronous; the bus client is not. A thread of its own keeps the
    // short-lived runtime clear of any runtime the caller may already be inside.
    let outcome = std::thread::scope(|scope| {
        scope.spawn(|| {
            tokio::runtime::Builder::new_current_thread().enable_all().build()?
                .block_on(crate::first_refusal(connect(instance), actor, token, payloads, crate::GATE_DEADLINE))
        }).join()
    }).map_err(|_| anyhow::anyhow!("the guardrail check panicked"))?;
    match outcome {
        Ok(None) => Ok(()),
        Ok(Some(error)) => anyhow::bail!("RELAY blocked {tool}: {}", crate::refusal_text(error.as_ref())),
        Err(error) => anyhow::bail!("RELAY guardrail unavailable, so {tool} was not run: {error:#}"),
    }
}

/// The folders this session may write to, as the guardrail reckons them (`bus.whoami`'s
/// `write_roots`: the worktree first, then scratch and memory roots and configured extras).
/// The Blender sandbox lets a child write there and nowhere else.
pub(crate) fn session_write_roots() -> Result<Vec<std::path::PathBuf>> {
    let instance = plugin_instance()?;
    let (actor, token) = crate::actor_from_env(None)?;
    let query = async {
        let mut client = connect(instance).await?;
        let mut request = Request::new(actor, "bus.whoami", json!({}));
        if let Some(token) = token { request = request.with_token(token); }
        let response = client.call(&request, |_| {}).await?;
        response.into_result().map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))
    };
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            tokio::runtime::Builder::new_current_thread().enable_all().build()?
                .block_on(async { tokio::time::timeout(crate::GATE_DEADLINE, query).await.map_err(|_| anyhow::anyhow!("bus.whoami did not answer"))? })
        }).join()
    }).map_err(|_| anyhow::anyhow!("bus.whoami panicked"))??;
    Ok(result["write_roots"].as_array().into_iter().flatten().filter_map(Value::as_str).map(std::path::PathBuf::from).collect())
}

fn plugin_gate_payloads(session: &str, root: Option<&Path>, tool: &str, file: Option<&Path>, writes: &[&Path]) -> Vec<Value> {
    // The file a command names is worktree-relative, as exec rules write it; the plugins hand
    // over files they have already resolved inside the checkout.
    let command = match file {
        Some(file) => {
            let root = root.map(|root| std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()));
            let named = root.as_deref().and_then(|root| file.strip_prefix(root).ok()).unwrap_or(file);
            format!("{tool} {}", named.display())
        }
        None => tool.to_string(),
    };
    let mut payloads = vec![json!({"session": session, "kind": "exec", "command": command})];
    for path in writes {
        // Binary files: no text to judge, so the empty diff of an edit in place; the path rules
        // still meet it.
        payloads.push(crate::write_gate(session, path, "diff", json!("")));
    }
    payloads
}

async fn handle(instance: Instance, actor: &Actor, token: Option<&str>, message: Value) -> Option<Value> {
    let method = message.get("method").and_then(Value::as_str);
    let Some(id) = message.get("id").cloned() else {
        // MCP notifications never receive a response.
        return None;
    };
    let result = match method {
        Some("initialize") => {
            // A small model's thread is told what the tools are for, not how the bus works.
            let style = Style::from_env();
            let instructions = if style.lean && style.assistant {
                "Relay itself: workspaces, projects, sessions, the board, notes and mail. What a session may not do is asked for with thread.ask."
            } else if style.lean {
                "The person's own data: Tally, their budget (money.*), and Avex, their gym log (gym.*)."
            } else {
                "Relay's typed command bus. Tool names are bus op names; typed refusals are returned as tool errors. The tool list is filtered to what this session may actually call, all three gating layers applied — if an op is missing, your role cannot call it. Result shapes are not inlined here: `bus.schema {op}` returns one op's full payload and result schema. `session.bootstrap` returns your peers, your callable ops and your guardrails."
            };
            Ok(json!({
                "protocolVersion": protocol(&message),
                // The tools follow the project's plugins as they are switched (`watch_tools`).
                "capabilities": {"tools":{"listChanged":true}},
                "serverInfo": {"name":"relay","title":"Relay command bus","version":env!("CARGO_PKG_VERSION")},
                "instructions": instructions
            }))
        }
        Some("ping") => Ok(json!({})),
        Some("tools/list") => tools(instance, actor, token).await,
        Some("tools/call") => {
            let Some(name) = message.pointer("/params/name").and_then(Value::as_str) else {
                return Some(rpc_error(id, -32602, "Invalid params", Some(json!({"message":"tools/call requires params.name"}))));
            };
            let Some(entry) = Registry::global().get(name) else {
                return Some(rpc_error(id, -32602, "Unknown tool", Some(json!({"name":name}))));
            };
            if !exposed(entry) {
                return Some(rpc_error(id, -32602, "Tool is not exposed over MCP stdio", Some(json!({"name":name}))));
            }
            call_tool(instance, actor, token, name, &message).await
        }
        Some(other) => return Some(rpc_error(id, -32601, "Method not found", Some(json!({"method":other})))),
        None => return Some(rpc_error(id, -32600, "Invalid Request", None)),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => rpc_error(id, -32603, "Relay MCP transport error", Some(json!({"message":format!("{error:#}")}))),
    })
}

async fn tools(instance: Instance, actor: &Actor, token: Option<&str>) -> Result<Value> {
    let mut client = connect(instance).await?;
    let mut request = Request::new(actor.clone(), "bus.ops", json!({}));
    if let Some(token) = token { request = request.with_token(token); }
    let response = client.call(&request, |_| {}).await?;
    let result = response.into_result().map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;
    let listed = result["ops"].as_array().cloned().unwrap_or_default();
    // Advertise only what this session can really call. Listing the whole registry meant
    // ~115 of the tools were ops the role allowlist refuses at runtime, and the payload was
    // large enough that a harness could drop the lot — so the working integration reached
    // nobody at all (D104).
    let selection = selection();
    let style = Style::from_env();
    // A small model told only in a reading which accounts it must not record activity in tries
    // anyway: the tool itself names them.
    let filled = if style.readings && selected(selection.as_deref(), "money.invest.add") {
        let mut request = Request::new(actor.clone(), "money.lists", json!({}));
        if let Some(token) = token { request = request.with_token(token); }
        client.call(&request, |_| {}).await.ok().filter(|r| r.ok).and_then(|r| r.result).map(|lists| filled_by_wealthsimple(&lists)).unwrap_or_default()
    } else {
        Vec::new()
    };
    let tools = listed.into_iter().filter_map(|info| {
        if info["implemented"] != true { return None; }
        let name = info["name"].as_str()?;
        let entry = Registry::global().get(name)?;
        if !exposed(entry) { return None; }
        let admitted = match &selection {
            // An explicit RELAY_MCP_OPS is a deliberate override: it selects the set, and the
            // engine still refuses anything the role may not call.
            // Matched the way role allowlists are, so the two cannot drift apart (RA-590).
            Some(_) => selected(selection.as_deref(), name),
            None => info["call"] != "no",
        };
        admitted.then(|| {
            let tool = if style.docs { small_model_tool(entry) } else { op_tool(entry) };
            if style.readings { never_into(by_names(tool), &filled) } else { tool }
        })
    }).collect::<Vec<_>>();

    report_tool_count(&mut client, actor, token, tools.len()).await;
    Ok(json!({"tools":tools}))
}

/// A session that receives zero tools currently fails in silence — the integration looks
/// configured and simply is not there. Say so once, on the record.
async fn report_tool_count(client: &mut Client, actor: &Actor, token: Option<&str>, count: usize) {
    static REPORTED: OnceLock<()> = OnceLock::new();
    let first = REPORTED.set(()).is_ok();
    if !first && count > 0 {
        return;
    }
    if count == 0 {
        eprintln!("relay mcp: this session was offered 0 tools — check its role allowlist or RELAY_MCP_OPS");
    }
    let Some(session) = actor.session_name() else { return };
    let mut request = Request::new(actor.clone(), "session.report", json!({
        "session": session,
        "kind": "notification",
        "data": {"notification_type": "relay_mcp_tools", "tools": count},
    }));
    if let Some(token) = token { request = request.with_token(token); }
    let _ = client.call(&request, |_| {}).await;
}

/// `RELAY_MCP_OPS`: the op patterns this server was started for, when it was given a set.
fn selection() -> Option<Vec<String>> {
    std::env::var("RELAY_MCP_OPS").ok().map(|raw| {
        raw.split(',').map(str::trim).filter(|part| !part.is_empty()).map(str::to_string).collect()
    })
}

/// Whether `name` may be called here. A server given a set answers only that set, whatever a
/// client asks for: a thread's agent runs as the person, and its set is all it may change.
fn selected(selection: Option<&[String]>, name: &str) -> bool {
    selection.is_none_or(|patterns| patterns.iter().any(|pattern| relay_core::guardrail::op_matches(pattern, name)))
}

async fn call_tool(instance: Instance, actor: &Actor, token: Option<&str>, name: &str, message: &Value) -> Result<Value> {
    if !selected(selection().as_deref(), name) {
        return Ok(tool_error(
            json!({"kind":"refused","code":"bus.allowlist","message":format!("{name} is not one of the tools this server was started with")}),
            None,
        ));
    }
    let arguments = message.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Ok(tool_error(
            json!({"kind":"invalid","code":"bus.schema","message":"tool arguments must be an object"}),
            None,
        ));
    }
    let mut arguments = arguments;
    let mut client = connect(instance).await?;
    // Money goes to a model as decimals and comes back from it as decimals (`units`): the
    // bus's own integers stay between the engine and people's apps.
    let schema = relay_bus::schema::render_op(name).unwrap_or(Value::Null);
    unstring(&mut arguments, &schema["payload"]);
    // design.apply's own arguments here: a picture of what it changed, and a refused list sent
    // again with only its corrections (`design_calls`).
    let apply = name == relay_bus::ops::design::Apply::NAME;
    let render = match apply.then(|| crate::design_calls::prepare(&mut arguments)).transpose() {
        Ok(render) => render.flatten(),
        Err(why) => return Ok(tool_error(json!({"kind":"invalid","code":"bus.schema","message":why}), None)),
    };
    let (in_units, out_units) = (units::has_units(&schema["payload"]), units::has_units(&schema["result"]));
    let ledger = if in_units || out_units { ledger_digits(&mut client, actor, token).await } else { 2 };
    let style = Style::from_env();
    if style.readings && names_in(name, &arguments) {
        // A small model names the account and category as the person did; the door finds their
        // ids, or sends the write back with the names it may use. A proposed batch of edits
        // names them the same way, entry by entry.
        let lists = read(&mut client, actor, token, "money.lists", json!({})).await.unwrap_or_else(|| json!({}));
        let resolved = if name == "thread.propose" {
            arguments.pointer_mut("/action/money.tx.update").and_then(Value::as_array_mut)
                .map_or(Ok(()), |items| items.iter_mut().try_for_each(|item| resolve_names(item, &lists)))
        } else {
            resolve_names(&mut arguments, &lists)
        };
        if let Err(why) = resolved {
            let text = format!("Not done: {why}. Call {name} again with one of those names.");
            return Ok(json!({"content":[{"type":"text","text":text}],"isError":true}));
        }
        LISTS_READ.store(true, Ordering::Relaxed);
    }
    if in_units {
        if let Err(why) = units::to_engine(&mut arguments, &schema["payload"], ledger) {
            return Ok(refusal(json!({"kind":"invalid","code":"bus.schema","message":why}), None, &style));
        }
    }
    if let Some(why) = style.readings.then(|| held(name, &arguments, LISTS_READ.load(Ordering::Relaxed))).flatten() {
        // A small model guesses the ids it has not read and leaves out a category the schema
        // calls optional: such a write is sent back unmade with the accounts and categories, to
        // be made again with them.
        let lists = read(&mut client, actor, token, "money.lists", json!({})).await.unwrap_or_else(|| json!({}));
        if let Some(text) = reading(&mut client, actor, token, "money.lists", &json!({}), &lists).await {
            LISTS_READ.store(true, Ordering::Relaxed);
            let text = format!("Not done yet: {why}. The accounts and categories are read here for you: pick the ones the \
                person named, then call {name} again with their names.\n\n{text}");
            return Ok(json!({"content":[{"type":"text","text":text}],"isError":true}));
        }
    }
    let mut request = Request::new(actor.clone(), name, arguments);
    if let Some(token) = token { request = request.with_token(token); }
    let response = client.call(&request, |_| {}).await?;
    let mail = response.mail.clone();
    if response.ok && name == "money.lists" {
        LISTS_READ.store(true, Ordering::Relaxed);
    }
    if response.ok && style.readings && relay_core::readings::OPS.contains(&name) {
        // A small model is given the reading, worked out and in words, not the JSON.
        let result = response.result.clone().unwrap_or_else(|| json!({}));
        let payload = request.payload.clone();
        if let Some(text) = reading(&mut client, actor, token, name, &payload, &result).await {
            return Ok(reading_success(text, mail.as_ref()));
        }
    }
    if response.ok {
        let mut value = response.result.unwrap_or_else(|| json!({}));
        if out_units {
            units::to_agent(&mut value, &schema["result"], ledger);
        }
        if name == "bus.schema" {
            value = units::agent_schema(&value);
        }
        if name == relay_bus::ops::design::Render::NAME {
            return Ok(render_success(value, mail.as_ref()));
        }
        if apply {
            crate::design_calls::applied(value["design_id"].as_i64());
            if let Some(options) = render {
                return Ok(apply_and_look(&mut client, actor, token, value, options, mail).await);
            }
        }
        if name == relay_bus::ops::picture::Thumb::NAME {
            return Ok(thumb_success(value, mail.as_ref()));
        }
        if name == relay_bus::ops::video::Still::NAME {
            return Ok(still_success(value, mail.as_ref()));
        }
        Ok(tool_success(value, mail.as_ref())?)
    } else {
        let mut error = serde_json::to_value(response.error)?;
        if apply {
            crate::design_calls::refused(&request.payload, &mut error);
        }
        Ok(refusal(error, mail.as_ref(), &style))
    }
}

/// design.apply's answer with design.render's of what it changed after it, as one tool result:
/// the apply's JSON, then the render's and its picture. A render that cannot be made (no desktop
/// app, say) is said under the apply's answer: the edits stand either way.
async fn apply_and_look(client: &mut Client, actor: &Actor, token: Option<&str>, applied: Value, options: serde_json::Map<String, Value>, mail: Option<MailHint>) -> Value {
    let mut request = Request::new(actor.clone(), relay_bus::ops::design::Render::NAME, crate::design_calls::render_payload(options, &applied));
    if let Some(token) = token { request = request.with_token(token); }
    let mut content = vec![json!({"type":"text","text":serde_json::to_string(&applied).unwrap_or_default()})];
    let mut mail = mail;
    match client.call(&request, |_| {}).await {
        Ok(response) if response.ok => {
            mail = response.mail.clone().or(mail);
            let shown = render_success(response.result.unwrap_or_else(|| json!({})), None);
            content.extend(shown["content"].as_array().cloned().unwrap_or_default());
        }
        Ok(response) => {
            mail = response.mail.clone().or(mail);
            let error = response.error.map(|e| format!("{}: {}{}", e.code, e.message, e.hint.map(|h| format!(" ({h})")).unwrap_or_default()));
            content.push(json!({"type":"text","text":format!("Applied; no picture: {}", error.unwrap_or_else(|| "design.render refused".into()))}));
        }
        Err(e) => content.push(json!({"type":"text","text":format!("Applied; no picture: {e}")})),
    }
    if let Some(notice) = mail_notice(mail.as_ref()) {
        content.push(notice);
    }
    json!({"content":content,"isError":false})
}

/// Arguments as models sometimes write them: an array or an object sent as the string of its
/// JSON (`"ops": "[{\"op\":\"add\",…}]"`). Such a string, in a field that takes an array or an
/// object and never a string, is replaced by what it parses to when that is of the type the field
/// takes: at the top level, and one level down (each object of an array, as design.apply's `node`
/// and `set`). The call then runs as the model meant it, where the engine would refuse it with an
/// error larger than the request (TM-6). Anything else is left for the engine to judge.
fn unstring(arguments: &mut Value, payload: &Value) {
    unstring_in(arguments, &[payload], payload, 1);
}

/// [`unstring`] of `value`, which `schemas` describe (any of them; references resolve in `root`),
/// going `depth` more levels into the fields it holds.
fn unstring_in(value: &mut Value, schemas: &[&Value], root: &Value, depth: u8) {
    match value {
        Value::Object(map) => {
            for (key, field) in map.iter_mut() {
                // Only a string that opens an array or an object can change here, and below the
                // last level nothing else is looked at, so a long list's ids and tags cost no lookup.
                let opens = field.as_str().is_some_and(|text| matches!(text.trim_start().chars().next(), Some('[' | '{')));
                if !(opens || (depth > 0 && (field.is_object() || field.is_array()))) {
                    continue;
                }
                let subs: Vec<&Value> = schemas.iter().flat_map(|schema| fields(schema, key, root, 0)).collect();
                if subs.is_empty() {
                    continue;
                }
                if let Value::String(text) = field {
                    match parsed_as(text, &subs, root) {
                        Some(parsed) => *field = parsed,
                        None => continue,
                    }
                }
                if depth > 0 {
                    unstring_in(field, &subs, root, depth - 1);
                }
            }
        }
        Value::Array(items) => {
            let subs: Vec<&Value> = schemas.iter().flat_map(|schema| items_of(schema, root, 0)).collect();
            if !subs.is_empty() {
                items.iter_mut().filter(|item| item.is_object()).for_each(|item| unstring_in(item, &subs, root, depth));
            }
        }
        _ => {}
    }
}

/// `text` as JSON, when it is an array or an object that one of `schemas` takes and none of them
/// takes a string.
fn parsed_as(text: &str, schemas: &[&Value], root: &Value) -> Option<Value> {
    let mut types = Vec::new();
    for schema in schemas {
        types_of(schema, root, 0, &mut types)?;
    }
    if types.contains(&"string") {
        return None;
    }
    serde_json::from_str::<Value>(text).ok()
        .filter(|parsed| (parsed.is_array() && types.contains(&"array")) || (parsed.is_object() && types.contains(&"object")))
}

/// How far references and `anyOf`/`oneOf`/`allOf` branches are followed: a recursive schema
/// stops there.
const SCHEMA_HOPS: u8 = 8;

/// `schema` with its local `$ref` (`#/$defs/…`) followed in `root`.
fn resolved<'a>(mut schema: &'a Value, root: &'a Value) -> &'a Value {
    for _ in 0..SCHEMA_HOPS {
        match schema.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix('#')).and_then(|at| root.pointer(at)) {
            Some(target) => schema = target,
            None => break,
        }
    }
    schema
}

fn branches_of(schema: &Value) -> impl Iterator<Item = &Value> {
    ["anyOf", "oneOf", "allOf"].into_iter().filter_map(|k| schema.get(k)?.as_array()).flatten()
}

/// The schemas `key` has in an object schema: its own, and each branch's.
fn fields<'a>(schema: &'a Value, key: &str, root: &'a Value, hops: u8) -> Vec<&'a Value> {
    let schema = resolved(schema, root);
    let mut found: Vec<&Value> = schema.get("properties").and_then(|p| p.get(key)).into_iter().collect();
    if hops < SCHEMA_HOPS {
        found.extend(branches_of(schema).flat_map(|branch| fields(branch, key, root, hops + 1)));
    }
    found
}

/// The schemas an array schema gives its items: its own, and each branch's.
fn items_of<'a>(schema: &'a Value, root: &'a Value, hops: u8) -> Vec<&'a Value> {
    let schema = resolved(schema, root);
    let mut found: Vec<&Value> = schema.get("items").into_iter().collect();
    if hops < SCHEMA_HOPS {
        found.extend(branches_of(schema).flat_map(|branch| items_of(branch, root, hops + 1)));
    }
    found
}

/// Adds the JSON types `schema` takes to `types`; `None` when it takes any value (no `type`
/// anywhere it can be read), so nothing may be read as something else.
fn types_of<'a>(schema: &'a Value, root: &'a Value, hops: u8, types: &mut Vec<&'a str>) -> Option<()> {
    let schema = resolved(schema, root);
    match schema.get("type") {
        Some(Value::String(one)) => types.push(one),
        Some(Value::Array(many)) => types.extend(many.iter().filter_map(Value::as_str)),
        _ => {
            let mut branches = branches_of(schema).peekable();
            if branches.peek().is_none() || hops >= SCHEMA_HOPS {
                return None;
            }
            for branch in branches {
                types_of(branch, root, hops + 1, types)?;
            }
        }
    }
    Some(())
}

/// Whether this server has given its model money.lists: this process serves one thread's agent.
static LISTS_READ: AtomicBool = AtomicBool::new(false);

/// Why a small model's write waits, if it does: it takes ids only money.lists gives (an entry
/// added, or one moved to another account or category) and the lists are not read yet; or it adds
/// an expense or income with no category, which Tally's own sheet never does.
fn held(op: &str, arguments: &Value, lists_read: bool) -> Option<&'static str> {
    let given = |k: &str| arguments.get(k).is_some_and(|v| !v.is_null());
    let takes_ids = match op {
        "money.tx.add" => true,
        "money.tx.update" => ["account_id", "category_id", "to_account_id"].into_iter().any(given),
        _ => false,
    };
    if op == "money.tx.add" && arguments["type"] != "TRANSFER" && !given("category_id") {
        Some("an expense or income needs its category")
    } else if takes_ids && !lists_read {
        Some("the account and category ids come from money.lists, not yet read")
    } else {
        None
    }
}

/// The accounts Wealthsimple's files fill, by name: activity recorded in one by hand would count
/// twice once the next export brings it, and the engine refuses it from an agent.
fn filled_by_wealthsimple(lists: &Value) -> Vec<String> {
    lists["accounts"].as_array().map(|accounts| accounts.iter()
        .filter(|a| a["archived"] != true && a["institution"].as_str().is_some_and(|i| i.trim().eq_ignore_ascii_case("wealthsimple")))
        .filter_map(|a| a["name"].as_str().map(str::to_string))
        .collect()).unwrap_or_default()
}

/// `money.invest.add` as a small model is shown it: the accounts it is never for, named.
fn never_into(mut tool: Value, filled: &[String]) -> Value {
    if tool["name"] != "money.invest.add" || filled.is_empty() {
        return tool;
    }
    let about = tool["description"].as_str().unwrap_or("").to_string();
    tool["description"] = json!(format!(
        "{about}. NEVER for {}: Wealthsimple's files fill {}, so the next export brings the activity and recording it here would count \
         it twice. For those, say the next export brings it.",
        filled.join(", "), if filled.len() == 1 { "it" } else { "them" }
    ));
    tool
}

/// The writes a small model makes by naming the account and category instead of their ids.
const NAMED: [&str; 2] = ["money.tx.add", "money.tx.update"];
/// Each name a small model gives, and the id field the door fills from it.
const NAMES: [(&str, &str); 3] = [("account", "account_id"), ("to_account", "to_account_id"), ("category", "category_id")];

/// Whether a small model's call names an account or a category the door must turn into an id.
fn names_in(op: &str, arguments: &Value) -> bool {
    let named = |item: &Value| NAMES.iter().any(|(key, _)| item.get(*key).is_some());
    if op == "thread.propose" {
        return arguments.pointer("/action/money.tx.update").and_then(Value::as_array).is_some_and(|items| items.iter().any(named));
    }
    NAMED.contains(&op) && named(arguments)
}

/// `money.tx.add` and `money.tx.update` as a small model is shown them: the account and the
/// category by name ("cash", "the Visa", "Dining"), not by id, which it picks wrong from a list;
/// and an entry's category required, null only for a transfer, since a field marked optional is
/// one it leaves out.
fn by_names(mut tool: Value) -> Value {
    let Some(op) = NAMED.iter().find(|op| tool["name"] == **op) else {
        return tool;
    };
    let adding = *op == "money.tx.add";
    let schema = &mut tool["inputSchema"];
    if let Some(props) = schema["properties"].as_object_mut() {
        for (_, id) in NAMES {
            props.remove(id);
        }
        props.insert("account".into(), json!({"type": if adding { json!("string") } else { json!(["string", "null"]) },
            "description": "The account by its name in money.lists, or as the person said it: \"cash\", \"the Visa\", \"chequing\"."}));
        props.insert("to_account".into(), json!({"type": ["string", "null"], "description": "Transfers only: the account the money goes to, by name."}));
        props.insert("category".into(), json!({"type": ["string", "null"],
            "description": if adding { "Required for an expense or income: the category by its name in money.lists, e.g. \"Dining\". null for a transfer." }
                           else { "To move the entry to another category: its name in money.lists, e.g. \"Dining\"." }}));
    }
    if let Some(required) = schema["required"].as_array_mut() {
        required.retain(|r| !NAMES.iter().any(|(_, id)| r == id));
        if adding {
            required.extend([json!("account"), json!("category")]);
        }
    }
    tool
}

/// A name as compared: lower case, accents and a leading "the", "my" or "ma" gone.
fn plain(name: &str) -> String {
    let folded: String = name.trim().to_lowercase().chars().map(|c| match c {
        'é' | 'è' | 'ê' | 'ë' => 'e', 'à' | 'â' | 'ä' => 'a', 'î' | 'ï' => 'i', 'ô' | 'ö' => 'o', 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c',
        c => c,
    }).collect();
    let mut rest = folded.as_str();
    for lead in ["the ", "my ", "ma ", "mon ", "la ", "le "] {
        rest = rest.strip_prefix(lead).unwrap_or(rest);
    }
    rest.trim().to_string()
}

/// The account type a word means when no account carries it in its name: "cash", "card" and so on.
fn account_type_of(word: &str) -> Option<&'static str> {
    let has = |words: &[&str]| words.iter().any(|w| word.split(|c: char| !c.is_alphanumeric()).any(|part| part == *w));
    if has(&["cash", "comptant", "argent"]) {
        Some("CASH")
    } else if has(&["visa", "mastercard", "amex", "card", "credit", "carte"]) {
        Some("CREDIT")
    } else if has(&["chequing", "checking", "debit", "cheque", "cheques", "bank"]) {
        Some("CHEQUING")
    } else if has(&["savings", "epargne"]) {
        Some("SAVINGS")
    } else {
        None
    }
}

/// The one row of `rows` that `wanted` names: its whole name, else the only name holding it, else
/// the only name it holds; for an account, else the only one of the type it says.
fn find_named<'a>(rows: &[&'a Value], wanted: &str, accounts: bool) -> Option<&'a Value> {
    let wanted = plain(wanted);
    let name = |row: &Value| plain(row["name"].as_str().unwrap_or(""));
    let only = |hits: Vec<&'a Value>| (hits.len() == 1).then(|| hits[0]);
    if wanted.is_empty() {
        return None;
    }
    rows.iter().copied().find(|r| name(r) == wanted)
        .or_else(|| only(rows.iter().copied().filter(|r| name(r).contains(&wanted)).collect()))
        .or_else(|| only(rows.iter().copied().filter(|r| !name(r).is_empty() && wanted.contains(&name(r))).collect()))
        .or_else(|| {
            let ty = accounts.then(|| account_type_of(&wanted)).flatten()?;
            only(rows.iter().copied().filter(|r| r["type"] == ty).collect())
        })
}

/// The ids of the account and category a small model named, written into `arguments` in place of
/// the names; or why not, with the names it may use.
fn resolve_names(arguments: &mut Value, lists: &Value) -> Result<(), String> {
    let live = |key: &str| -> Vec<&Value> {
        lists[key].as_array().map(|rows| rows.iter().filter(|r| r["archived"] != true).collect()).unwrap_or_default()
    };
    let accounts = live("accounts");
    let kind = arguments["type"].as_str().filter(|t| *t != "TRANSFER").map(str::to_string);
    let categories: Vec<&Value> = live("categories").into_iter().filter(|c| kind.as_ref().is_none_or(|k| c["kind"] == k.as_str())).collect();
    let Some(args) = arguments.as_object_mut() else { return Ok(()) };
    for (key, id) in NAMES {
        let Some(given) = args.remove(key) else { continue };
        let wanted = match &given {
            Value::Null => continue,
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        let (rows, what) = if key == "category" { (&categories, "category") } else { (&accounts, "account") };
        match find_named(rows, &wanted, key != "category") {
            Some(row) => {
                args.insert(id.into(), row["id"].clone());
            }
            None => {
                let names: Vec<&str> = rows.iter().filter_map(|r| r["name"].as_str()).collect();
                return Err(format!("no {what} is named \"{wanted}\"; the {what} names are: {}", names.join(", ")));
            }
        }
    }
    Ok(())
}

/// A refusal as a tool error: in words for a model given readings, else the bus's JSON.
fn refusal(error: Value, mail: Option<&MailHint>, style: &Style) -> Value {
    if !style.readings {
        return tool_error(error, mail);
    }
    let mut text = format!("Not done ({}): {}", error["code"].as_str().unwrap_or("refused"), error["message"].as_str().unwrap_or("refused").trim_end_matches('.'));
    text.push('.');
    if let Some(hint) = error["hint"].as_str() {
        text.push_str(&format!(" Hint: {hint}."));
    }
    let mut content = vec![json!({"type":"text","text":text})];
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    json!({"content":content,"isError":true})
}

/// How this server answers the model it serves (`RELAY_MCP_STYLE`, set by the engine for a
/// thread on a model on this PC: `threads::LocalStyle`).
#[derive(Default)]
struct Style {
    /// A short introduction in place of the bus's.
    lean: bool,
    /// Reads answered as readings (`relay_core::readings`) instead of JSON.
    readings: bool,
    /// Tools described in plain words with an example each.
    docs: bool,
    /// The Dev space's assistant's server, which a lean introduction describes as Relay's.
    assistant: bool,
}

impl Style {
    fn from_env() -> Style {
        let spec = std::env::var("RELAY_MCP_STYLE").unwrap_or_default();
        let has = |part: &str| spec.split(',').any(|p| p.trim() == part);
        Style { lean: has("lean"), readings: has("readings"), docs: has("docs"), assistant: has("assistant") }
    }
}

/// One op's result, or `None` when it is refused or the engine does not answer.
async fn read(client: &mut Client, actor: &Actor, token: Option<&str>, op: &str, payload: Value) -> Option<Value> {
    let mut request = Request::new(actor.clone(), op, payload);
    if let Some(token) = token { request = request.with_token(token); }
    client.call(&request, |_| {}).await.ok().filter(|r| r.ok).and_then(|r| r.result)
}

/// `result` of `op` as its reading, the reads it folds in read first.
async fn reading(client: &mut Client, actor: &Actor, token: Option<&str>, op: &str, payload: &Value, result: &Value) -> Option<String> {
    let mut extras = Vec::new();
    for (extra, with) in relay_core::readings::extras(op, result) {
        extras.push(read(client, actor, token, extra, with).await);
    }
    let currency = read(client, actor, token, "money.lists", json!({})).await
        .and_then(|lists| lists["currency"].as_str().map(str::to_string))
        .unwrap_or_else(|| "CAD".into());
    let ctx = relay_core::readings::Ctx { today: jiff::Zoned::now().date(), currency };
    relay_core::readings::render(op, payload, result, &extras, &ctx)
}

/// A reading as a tool's answer: plain text, with no JSON beside it for a client to prefer.
fn reading_success(text: String, mail: Option<&MailHint>) -> Value {
    let mut content = vec![json!({"type":"text","text":text})];
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    json!({"content":content,"isError":false})
}

/// The ledger currency's decimal places, for a reading that does not name its currency.
async fn ledger_digits(client: &mut Client, actor: &Actor, token: Option<&str>) -> u32 {
    let mut request = Request::new(actor.clone(), "money.lists", json!({}));
    if let Some(token) = token { request = request.with_token(token); }
    match client.call(&request, |_| {}).await.ok().and_then(|r| r.result) {
        Some(lists) => lists["currency"].as_str().map_or(2, units::digits),
        None => 2,
    }
}

fn exposed(entry: &OpEntry) -> bool {
    entry.meta.stream.is_none()
        && !matches!(entry.name, "bus.subscribe" | "bus.unsubscribe")
}

fn op_tool(entry: &OpEntry) -> Value {
    let schema = relay_bus::schema::render_op(entry.name).expect("registry op has schema");
    // Result schemas are two thirds of this document and are only needed *after* choosing an
    // op. They stay one `bus.schema {op}` call away rather than in every session's context.
    let mut tool = json!({
        "name":entry.name,
        "title":entry.name,
        "description":entry.meta.summary,
        "inputSchema":units::agent_schema(&schema["payload"]),
        "annotations":{
            "readOnlyHint":entry.meta.kind == OpKind::Query,
            "destructiveHint":entry.meta.kind == OpKind::Mutation,
            "openWorldHint":false
        }
    });
    if entry.name == relay_bus::ops::design::Apply::NAME {
        crate::design_calls::extend_tool(&mut tool);
    }
    tool
}

/// A tool as a small model is shown it: its description in plain words with an example call, and
/// without the fields it is better without (`relay_core::readings`).
fn small_model_tool(entry: &OpEntry) -> Value {
    let mut tool = op_tool(entry);
    if let Some(about) = relay_core::readings::description(entry.name, jiff::Zoned::now().date()) {
        tool["description"] = Value::String(about);
    }
    for (op, field) in relay_core::readings::HIDDEN_FIELDS {
        if *op == entry.name {
            if let Some(props) = tool["inputSchema"]["properties"].as_object_mut() {
                props.remove(*field);
            }
        }
    }
    tool
}

fn mail_notice(mail: Option<&MailHint>) -> Option<Value> {
    mail.map(|mail| {
        json!({
            "type": "text",
            "text": format!(
                "Relay priority mail: {} unread. Finish the current atomic action, then call mailbox.list with unread_only true and acknowledge each message.",
                mail.priority
            )
        })
    })
}

/// An op's result: its JSON as text, compact (pretty printing put each value of a design's nodes
/// on its own line and took a large design past an agent's output cap), and the same value as
/// `structuredContent`, which the text mirrors as MCP asks.
fn tool_success(value: Value, mail: Option<&MailHint>) -> Result<Value> {
    let mut content = vec![json!({
        "type":"text",
        "text":serde_json::to_string(&value)?
    })];
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    Ok(json!({"content":content,"structuredContent":value,"isError":false}))
}

/// A design.render answer: the result as compact JSON, then the picture as an image block, so the
/// model sees it without reading a file. No `structuredContent`: Claude Code gives the model its
/// own JSON of that in place of this text, after the picture (render-SPEC-v2 §2.5); MCP asks for
/// it only of a tool with an `outputSchema`, which no op declares. A picture that cannot be
/// attached stays a path, and the text says where.
fn render_success(value: Value, mail: Option<&MailHint>) -> Value {
    picture_success(value, "/png/path", &RENDERS, mail)
}

/// A design.picture.thumb answer, as a render's: the compact JSON, then the thumbnail itself, so
/// an agent sees it without reading a file outside its write roots (pictures-SPEC-v2 §5.3).
fn thumb_success(value: Value, mail: Option<&MailHint>) -> Value {
    picture_success(value, "/path", &THUMBS, mail)
}

/// A video.still answer, as a render's: the compact JSON, then the frame or contact sheet itself,
/// so the agent sees what it made without reading a file (D167).
fn still_success(value: Value, mail: Option<&MailHint>) -> Value {
    picture_success(value, "/png", &STILLS, mail)
}

/// An answer that names a picture at `pointer`: its compact JSON, then the picture as an image
/// block when `shelf` holds it, else a line saying where it is; no `structuredContent`.
fn picture_success(value: Value, pointer: &str, shelf: &Shelf, mail: Option<&MailHint>) -> Value {
    let mut content = vec![json!({"type":"text","text":serde_json::to_string(&value).unwrap_or_default()})];
    if let Some(path) = value.pointer(pointer).and_then(Value::as_str) {
        content.push(image_block(Path::new(path), shelf).unwrap_or_else(|| {
            json!({"type":"text","text":format!("The picture could not be attached; it is at {path}.")})
        }));
    }
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    json!({"content":content,"isError":false})
}

/// Where a bus answer's picture may be read from to reach the model as an image: the folder the
/// engine writes it in, the names it gives there, and the size it holds it to. Nothing else is
/// read, whatever path a result names.
struct Shelf {
    /// The folder's own name, beside the store.
    folder: &'static str,
    /// Whether a file name is one the engine gives there.
    named: fn(&str) -> bool,
    /// Most bytes.
    max: u64,
}

/// design.render's pictures: `design-renders/<render id>.png`, at most 4 MiB (render-SPEC-v2 §5.6).
const RENDERS: Shelf = Shelf { folder: "design-renders", named: render_name, max: relay_core::picture::UPLOAD_MAX as u64 };

fn render_name(name: &str) -> bool {
    name.strip_suffix(".png").is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
}

/// video.still's pictures: `video-stills/<uuid>.png`, at most 4 MiB as a render's.
const STILLS: Shelf = Shelf { folder: "video-stills", named: render_name, max: relay_core::picture::UPLOAD_MAX as u64 };

/// design.picture.thumb's previews: `picture-cache/<openverse|pexels>-<key>.<jpg|png|webp|gif>`,
/// at most 2 MiB, the most the engine fetches for one (`Want::Thumb`).
const THUMBS: Shelf = Shelf { folder: "picture-cache", named: thumb_name, max: 2 << 20 };

/// A name the engine gives a kept thumbnail: its picture id's source and key, and a raster's
/// extension. The key is checked as the engine checks an id, so no other file there qualifies.
fn thumb_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    let Some((source, key)) = stem.split_once('-') else { return false };
    matches!(ext, "jpg" | "png" | "webp" | "gif") && relay_core::pictures::parse_id(&format!("{source}:{key}")).is_ok()
}

/// The picture at `path` as an MCP image block, its type read from its bytes; `None` unless the
/// path is absolute and plain (no `.` or `..`), names a file `shelf` holds, that file is a regular
/// one (not a symlink) of at most `shelf.max` bytes, and its bytes are a PNG, JPEG, GIF or WebP.
/// design.render's picture and a stock picture's thumbnail come this way, each from its shelf.
fn image_block(path: &Path, shelf: &Shelf) -> Option<Value> {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    use std::path::Component;
    let plain = path.is_absolute() && path.components().all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    let (folder, name) = (path.parent()?.file_name()?, path.file_name()?.to_str()?);
    if !plain || folder != shelf.folder || !(shelf.named)(name) {
        return None;
    }
    let seen = std::fs::symlink_metadata(path).ok().filter(|m| m.file_type().is_file() && m.len() <= shelf.max)?;
    let file = std::fs::File::open(path).ok()?;
    // The file read is the one looked at, not a link put in its place since.
    let opened = file.metadata().ok()?;
    if (opened.dev(), opened.ino()) != (seen.dev(), seen.ino()) {
        return None;
    }
    let mut bytes = Vec::with_capacity(seen.len() as usize);
    file.take(shelf.max + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > shelf.max {
        return None;
    }
    let picture = relay_core::picture::raster(&bytes)?;
    Some(image_content(&bytes, picture.media_type()))
}

/// `bytes` as MCP image content of `media_type`.
fn image_content(bytes: &[u8], media_type: &str) -> Value {
    use base64::Engine as _;
    json!({"type":"image","data":base64::engine::general_purpose::STANDARD.encode(bytes),"mimeType":media_type})
}

fn tool_error(error: Value, mail: Option<&MailHint>) -> Value {
    let text = serde_json::to_string(&error).unwrap_or_else(|_| "Relay tool error".to_string());
    let mut content = vec![json!({"type":"text","text":text})];
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    json!({"content":content,"isError":true})
}

pub(crate) fn rpc_error(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({"code":code,"message":message});
    if let Some(data) = data { error["data"] = data; }
    json!({"jsonrpc":"2.0","id":id,"error":error})
}

async fn connect(instance: Instance) -> Result<Client> {
    let path = instance.socket_path();
    Client::connect(&path).await.with_context(|| format!("no Relay engine at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_switched_in_this_project_changes_the_tools() {
        let event = |ev: &str, project: Option<i64>| {
            let mut event = relay_bus::Event::new(ev, "2026-10-22T00:00:00Z".into(), Actor::User, json!({"id": "video", "project_id": project, "enabled": true}));
            event.project_id = project;
            event
        };
        assert!(changes_tools(&event("plugin.changed", Some(1)), Some(1)));
        assert!(!changes_tools(&event("plugin.changed", Some(2)), Some(1)), "another project's plugins are not ours");
        assert!(changes_tools(&event("plugin.changed", Some(2)), None));
        assert!(!changes_tools(&event("task.changed", Some(1)), Some(1)));
    }

    #[tokio::test]
    async fn the_server_says_its_tools_can_change() {
        let message = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":FALLBACK_PROTOCOL}});
        let reply = handle(Instance::Test, &Actor::User, None, message).await.unwrap();
        assert_eq!(reply["result"]["capabilities"]["tools"]["listChanged"], true, "{reply}");
    }

    #[test]
    fn tool_is_the_bus_schema_and_streams_are_not_exposed() {
        let task = Registry::global().get("task.create").unwrap();
        let value = op_tool(task);
        assert_eq!(value["name"], "task.create");
        assert_eq!(value["inputSchema"]["type"], "object");
        assert!(value["inputSchema"]["required"].as_array().unwrap().iter().any(|item| item == "project_id"));
        assert_eq!(value["annotations"]["readOnlyHint"], false);
        assert!(value["outputSchema"].is_null(), "result schemas belong in bus.schema, not in every tool list");
        assert!(!exposed(Registry::global().get("session.attach").unwrap()));
        assert!(!exposed(Registry::global().get("device.mirror.start").unwrap()));
        assert!(exposed(Registry::global().get("session.brief").unwrap()));
        assert!(exposed(
            Registry::global().get("session.bootstrap").unwrap()
        ));
    }

    #[test]
    fn plugin_servers_share_one_json_rpc_side() {
        fn tools() -> Vec<Value> {
            vec![tool("echo", "Echo", json!({"x":{"type":"string"}}), &["x"], true)]
        }
        fn call(_name: &str, args: &Value) -> Result<Value> {
            match args["x"].as_str() {
                Some(x) => Ok(json!({"x": x})),
                None => anyhow::bail!("x is required"),
            }
        }
        let server = PluginServer { name: "test", title: "Test", instructions: "Test.", tools, call };
        let reply = |message: Value| plugin_reply(&server, &message).unwrap();
        let init = reply(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}));
        assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(init["result"]["serverInfo"]["name"], "test");
        let old = reply(json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2020-01-01"}}));
        assert_eq!(old["result"]["protocolVersion"], FALLBACK_PROTOCOL);
        assert_eq!(reply(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))["result"]["tools"][0]["name"], "echo");
        let ok = reply(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{"x":"hi"}}}));
        assert_eq!(ok["result"]["structuredContent"], json!({"x":"hi"}));
        let failed = reply(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"echo","arguments":{}}}));
        assert_eq!(failed["result"]["isError"], true);
        assert_eq!(reply(json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"nope"}}))["error"]["code"], -32602);
        assert_eq!(reply(json!({"jsonrpc":"2.0","id":7,"method":"nope"}))["error"]["code"], -32601);
        assert!(plugin_reply(&server, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    }

    #[test]
    fn selection_patterns_are_exact_or_one_namespace() {
        use relay_core::guardrail::op_matches;
        assert!(op_matches("mailbox.send", "mailbox.send"));
        assert!(op_matches("mailbox.*", "mailbox.send"));
        assert!(op_matches("*", "anything.at_all"));
        assert!(!op_matches("mailbox.*", "mailboxes.send"));
        assert!(!op_matches("task.get", "task.list"));
    }

    #[test]
    fn a_server_given_a_set_answers_only_that_set() {
        let set = vec!["money.tx.add".to_string(), "bus.*".to_string()];
        assert!(selected(Some(&set), "money.tx.add"));
        assert!(selected(Some(&set), "bus.schema"));
        assert!(!selected(Some(&set), "money.reset"));
        assert!(!selected(Some(&[]), "money.summary"));
        assert!(selected(None, "money.reset"), "no set: the engine's own checks decide");
    }

    #[test]
    fn a_small_model_sees_plain_descriptions_and_no_fields_it_would_misuse() {
        let tool = small_model_tool(Registry::global().get("money.tx.list").unwrap());
        assert!(tool["description"].as_str().unwrap().contains("period_offset: 0 this month, -1 last month"));
        let props = tool["inputSchema"]["properties"].as_object().unwrap();
        assert!(props.contains_key("period_offset") && !props.contains_key("today") && !props.contains_key("limit"));
        let tool = small_model_tool(Registry::global().get("money.tx.add").unwrap());
        assert_eq!(tool["inputSchema"]["properties"]["amount"]["type"], "string", "still decimals");
        for op in relay_core::threads::local_ops().into_iter().filter(|op| op.starts_with("money.") || op.starts_with("gym.")) {
            assert!(relay_core::readings::description(op, jiff::Zoned::now().date()).is_some(), "{op}");
        }
    }

    #[test]
    fn a_small_model_is_told_which_accounts_take_no_activity() {
        let lists = json!({"accounts": [
            {"id": 6, "name": "Demo TFSA", "type": "INVESTMENT", "institution": "Wealthsimple"},
            {"id": 7, "name": "Sample TFSA", "type": "INVESTMENT", "institution": ""},
            {"id": 8, "name": "Old WS", "type": "INVESTMENT", "institution": "wealthsimple", "archived": true},
        ]});
        assert_eq!(filled_by_wealthsimple(&lists), ["Demo TFSA"]);
        let tool = never_into(op_tool(Registry::global().get("money.invest.add").unwrap()), &filled_by_wealthsimple(&lists));
        assert!(tool["description"].as_str().unwrap().contains("NEVER for Demo TFSA"));
        let other = never_into(op_tool(Registry::global().get("money.tx.add").unwrap()), &["Demo TFSA".into()]);
        assert!(!other["description"].as_str().unwrap().contains("NEVER"));
    }

    #[test]
    fn a_small_model_names_the_account_and_category() {
        let lists = json!({
            "accounts": [
                {"id": 1, "name": "Sample chequing", "type": "CHEQUING"}, {"id": 2, "name": "Sample Visa", "type": "CREDIT"},
                {"id": 3, "name": "Sample savings", "type": "SAVINGS"}, {"id": 4, "name": "Sample cash", "type": "CASH"},
                {"id": 5, "name": "Old card", "type": "CREDIT", "archived": true},
            ],
            "categories": [
                {"id": 1, "name": "Groceries", "kind": "EXPENSE"}, {"id": 2, "name": "Dining", "kind": "EXPENSE"},
                {"id": 9, "name": "Salary", "kind": "INCOME"}, {"id": 10, "name": "Side income", "kind": "INCOME"},
            ],
        });
        let mut lunch = json!({"type": "EXPENSE", "amount": "18.50", "account": "cash", "category": "dining"});
        resolve_names(&mut lunch, &lists).unwrap();
        assert_eq!((&lunch["account_id"], &lunch["category_id"], lunch.get("account")), (&json!(4), &json!(2), None));
        for (said, id) in [("the Visa", 2), ("my credit card", 2), ("Sample chequing", 1), ("épargne", 3), ("debit", 1)] {
            let mut entry = json!({"type": "EXPENSE", "account": said, "category": "Groceries"});
            resolve_names(&mut entry, &lists).unwrap();
            assert_eq!(entry["account_id"], id, "{said}");
        }
        let mut transfer = json!({"type": "TRANSFER", "account": "chequing", "to_account": "savings", "category": null});
        resolve_names(&mut transfer, &lists).unwrap();
        assert_eq!((&transfer["account_id"], &transfer["to_account_id"], transfer.get("category_id")), (&json!(1), &json!(3), None));
        // A category of the other kind, or a name nothing holds, is sent back with the names.
        let why = resolve_names(&mut json!({"type": "EXPENSE", "account": "cash", "category": "Salary"}), &lists).unwrap_err();
        assert!(why.contains("Groceries, Dining") && !why.contains("Salary,"), "{why}");
        assert!(resolve_names(&mut json!({"type": "EXPENSE", "account": "Tangerine", "category": "Dining"}), &lists).unwrap_err().contains("Sample cash"));
        assert!(resolve_names(&mut json!({"type": "INCOME", "account": "chequing", "category": "income"}), &lists).is_ok(), "the only income name holding it");
        assert!(names_in("thread.propose", &json!({"action": {"money.tx.update": [{"id": 54, "category": "Entertainment"}]}})));
        assert!(!names_in("thread.propose", &json!({"action": {"money.tx.delete": [{"id": 54}]}})));
        assert!(names_in("money.tx.update", &json!({"id": 54, "category": "Dining"})) && !names_in("money.summary", &json!({"category": "x"})));
    }

    #[test]
    fn a_small_models_write_waits_for_the_ids_and_a_category() {
        let coffee = json!({"type": "EXPENSE", "amount": "4.50", "account_id": 2, "category_id": 4});
        assert!(held("money.tx.add", &coffee, false).is_some(), "ids not read yet");
        assert!(held("money.tx.add", &coffee, true).is_none());
        let uncategorized = json!({"type": "EXPENSE", "amount": "4.50", "account_id": 2});
        assert!(held("money.tx.add", &uncategorized, true).unwrap().contains("category"));
        assert!(held("money.tx.add", &json!({"type": "TRANSFER", "account_id": 1, "to_account_id": 3}), true).is_none());
        assert!(held("money.tx.update", &json!({"id": 7, "category_id": 3}), false).is_some());
        assert!(held("money.tx.update", &json!({"id": 7, "amount": "5"}), false).is_none(), "a new amount needs no id");
        assert!(held("money.tx.update", &json!({"id": 7, "category_id": null}), false).is_none());
        assert!(held("money.summary", &json!({}), false).is_none() && held("thread.propose", &json!({}), false).is_none());
        let tool = by_names(op_tool(Registry::global().get("money.tx.add").unwrap()));
        let required: Vec<&str> = tool["inputSchema"]["required"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(required.contains(&"account") && required.contains(&"category") && !required.contains(&"account_id"), "{required:?}");
        assert!(tool["inputSchema"]["properties"].get("category_id").is_none());
        let other = by_names(op_tool(Registry::global().get("money.tx.update").unwrap()));
        assert!(other["inputSchema"]["properties"].get("category").is_some() && !other["inputSchema"]["required"].as_array().unwrap().iter().any(|r| r == "category"));
    }

    #[test]
    fn a_reading_is_plain_text_and_a_refusal_is_words_for_a_model_given_readings() {
        let ok = reading_success("TALLY, the person's budget.".into(), None);
        assert_eq!(ok["content"][0]["text"], "TALLY, the person's budget.");
        assert!(ok.get("structuredContent").is_none(), "no JSON beside it for a client to prefer");
        let error = json!({"kind":"not_found","code":"money.not_found","message":"No entry 5","hint":"list entries with money.tx.list"});
        let plain = refusal(error.clone(), None, &Style { lean: true, readings: true, docs: false, assistant: false });
        assert_eq!(plain["isError"], true);
        assert_eq!(plain["content"][0]["text"], "Not done (money.not_found): No entry 5. Hint: list entries with money.tx.list.");
        let json = refusal(error, None, &Style::default());
        assert!(json["content"][0]["text"].as_str().unwrap().contains("\"code\":\"money.not_found\""));
    }

    #[test]
    fn tool_text_is_compact_json_that_mirrors_the_structured_content() {
        // Pretty printing put each value of a design's nodes on its own line, which took a
        // large design.get past an agent's output cap (TM-3).
        let value = json!({"design": {"id": 3, "name": "Home"}, "nodes": [{"id": "a", "kind": "rect", "x": 0}, {"id": "b", "kind": "text"}]});
        let success = tool_success(value.clone(), None).unwrap();
        let text = success["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains('\n') && !text.contains(": "), "{text}");
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), success["structuredContent"]);
        assert_eq!(success["structuredContent"], value, "the typed result is kept for clients that read it");
        let error = tool_error(json!({"kind":"conflict","code":"design.stale","details":{"conflicts":[{"id":"a"}]}}), None);
        let text = error["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains('\n'), "{text}");
        assert_eq!(serde_json::from_str::<Value>(text).unwrap()["details"]["conflicts"][0]["id"], "a");
        let plugin = tool_result(json!({"objects": [{"name": "Cube"}, {"name": "Light"}]}), false);
        let text = plugin["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains('\n'), "{text}");
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), plugin["structuredContent"]);
    }

    /// design.apply's arguments as a model writes them, its edits given as `ops`.
    fn apply_args(ops: Value) -> Value {
        json!({"design_id": 12, "base_rev": 4, "ops": ops})
    }

    fn apply_payload() -> Value {
        relay_bus::schema::render_op("design.apply").unwrap()["payload"].clone()
    }

    #[test]
    fn a_stringified_ops_array_is_applied_as_if_it_were_an_array() {
        let ops = json!([
            {"op": "add", "node": {"id": "card", "parent": "page-1", "kind": "rect", "x": 0, "y": 0, "w": 120, "h": 80, "fill": "#ffffff"}},
            {"op": "update", "id": "title", "set": {"text": "[not, an, array]", "font_size": 18}},
        ]);
        let mut arguments = apply_args(Value::String(ops.to_string()));
        unstring(&mut arguments, &apply_payload());
        assert_eq!(arguments, apply_args(ops.clone()), "the edits are read from the string, and a text that looks like JSON stays text");
        let read: relay_bus::ops::design::ApplyIn = serde_json::from_value(arguments).expect("the engine reads it as the array");
        let given: relay_bus::ops::design::ApplyIn = serde_json::from_value(apply_args(ops)).unwrap();
        assert_eq!(read, given);
        // What is not JSON of the type a field takes is left for the engine to refuse as it was.
        for ops in [json!("[{\"op\": \"add\""), json!("{\"op\": \"delete\", \"id\": \"card\"}"), json!("card")] {
            let mut arguments = apply_args(ops.clone());
            unstring(&mut arguments, &apply_payload());
            assert_eq!(arguments["ops"], ops);
        }
        // Only arrays and objects: a number sent as a string is not this door's to guess.
        let mut arguments = json!({"design_id": "12", "ops": []});
        unstring(&mut arguments, &apply_payload());
        assert_eq!(arguments["design_id"], "12");
    }

    #[test]
    fn a_stringified_node_or_set_inside_one_op_is_parsed_too() {
        let node = json!({"id": "card", "parent": "page-1", "kind": "rect", "x": 0, "y": 0, "w": 120, "h": 80});
        let set = json!({"fill": "#ff0000", "points": [[0, 0], [10, 10]]});
        let mut arguments = apply_args(json!([
            {"op": "add", "node": node.to_string()},
            {"op": "update", "id": "line", "set": set.to_string()},
            {"op": "delete", "id": "[\"old\"]"},
        ]));
        unstring(&mut arguments, &apply_payload());
        assert_eq!(arguments["ops"], json!([
            {"op": "add", "node": node},
            {"op": "update", "id": "line", "set": set},
            {"op": "delete", "id": "[\"old\"]"},
        ]), "an id is a string, whatever it looks like");
        // Both at once: the list a string, an edit's node a string inside it.
        let ops = json!([{"op": "add", "node": node.to_string()}]);
        let mut arguments = apply_args(Value::String(ops.to_string()));
        unstring(&mut arguments, &apply_payload());
        assert_eq!(arguments["ops"][0]["node"], node);
    }

    #[test]
    fn stringified_arguments_are_read_through_the_schemas_references() {
        // A payload schema whose fields name shared definitions instead of inlining them, as a
        // schema rendered with $defs is.
        let schema = json!({
            "$ref": "#/$defs/In",
            "$defs": {
                "In": {"type": "object", "properties": {
                    "ops": {"type": "array", "items": {"$ref": "#/$defs/Op"}},
                    "reply": {"anyOf": [{"$ref": "#/$defs/Reply"}, {"type": "null"}]},
                    "extra": {},
                }},
                "Op": {"oneOf": [
                    {"type": "object", "properties": {"op": {"const": "add", "type": "string"}, "node": {"$ref": "#/$defs/Node"}}},
                    {"type": "object", "properties": {"op": {"const": "put", "type": "string"}, "node": {"type": "string"}}},
                    {"type": "object", "properties": {"op": {"const": "update", "type": "string"}, "set": {"type": ["object", "null"]}}},
                ]},
                "Node": {"type": "object"},
                "Reply": {"type": "string", "enum": ["short", "full"]},
            },
        });
        let mut arguments = json!({"ops": "[{\"op\":\"update\",\"set\":\"{\\\"x\\\":1}\"}]", "reply": "[\"full\"]", "extra": "{\"a\":1}"});
        unstring(&mut arguments, &schema);
        assert_eq!(arguments["ops"], json!([{"op": "update", "set": {"x": 1}}]));
        assert_eq!(arguments["reply"], "[\"full\"]", "a field that takes a string keeps it");
        assert_eq!(arguments["extra"], "{\"a\":1}", "a field that takes anything keeps what it was given");
        // `node` is an object in one edit and a string in another: a string is taken, so kept.
        let mut arguments = json!({"ops": [{"op": "add", "node": "{\"id\":\"a\"}"}]});
        unstring(&mut arguments, &schema);
        assert_eq!(arguments["ops"][0]["node"], "{\"id\":\"a\"}");
    }

    /// A 1×1 PNG.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\
        \0\0\0\nIDATx\x9cc\0\x01\0\0\x05\0\x01\r\n-\xb4\0\0\0\0IEND\xaeB`\x82";

    /// The start of a 32×16 JPEG: as far as its frame header, which is what gives its type and size.
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x20, 0x03, 0x01, 0x22, 0x00];

    #[test]
    fn a_plugin_preview_takes_its_type_from_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            json!({"label": name.split('.').next().unwrap(), "path": path})
        };
        let images = [
            file("front.png", JPEG),
            file("side.jpg", PNG),
            // A header cut short: what its name says, as before.
            file("top.png", b"\x89PNG"),
            // What a model cannot be shown, whatever the name.
            file("plan.png", b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"/>"),
            file("notes.txt", b"hello"),
            file("depth.exr", b"v/1\x01\0\0\0\0"),
        ];
        let result = tool_result(json!({"_images": images}), false);
        let content = result["content"].as_array().unwrap();
        let types: Vec<&str> = content.iter().filter(|c| c["type"] == "image").filter_map(|c| c["mimeType"].as_str()).collect();
        assert_eq!(types, ["image/jpeg", "image/png", "image/png"]);
        assert_eq!(decoded(&content[2]), JPEG);
        assert_eq!(result["structuredContent"]["images"], json!({"shown": ["front", "side", "top"], "requested": 6, "limit": MAX_IMAGES}));
    }

    fn decoded(block: &Value) -> Vec<u8> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.decode(block["data"].as_str().unwrap()).unwrap()
    }

    /// A design.render answer whose picture is at `path`, or with none.
    fn render_answer(path: Option<&Path>) -> Value {
        let mut value = json!({
            "design_id": 12, "rev": 15, "page": "page-1", "area": {"x": -32, "y": -32, "w": 454, "h": 908}, "scale": 1.0,
            "issues": [{"code": "overflow", "ids": ["save", "card"], "message": "save extends 12 px past card's padding"}],
            "issue_count": 1, "warnings": [],
            "boxes": {"fields": ["id","kind","parent","x","y","w","h","rx","ry","rot"],
                      "rows": [["card","frame","page-1",0,0,390,844,0,0,0], ["save","rect","card",24,72,151.5,31,24,72,0]], "total": 2, "hidden": 0},
            "timings_ms": {"read": 6.1, "layout": 1.4, "paint": 7.9, "encode": 38.0},
        });
        if let Some(path) = path {
            value["png"] = json!({"path": path, "w": 1, "h": 1});
        }
        value
    }

    /// `<dir>/design-renders/<uuid>.png` holding `bytes`, as the engine writes a render.
    fn rendered(dir: &Path, bytes: &[u8]) -> PathBuf {
        let renders = dir.join("design-renders");
        std::fs::create_dir_all(&renders).unwrap();
        let path = renders.join(format!("{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_render_result_is_compact_text_then_the_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = rendered(dir.path(), PNG);
        let answer = render_answer(Some(&path));
        let result = render_success(answer.clone(), None);
        assert_eq!(result["isError"], false);
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.len(), 2, "{content:?}");
        let text = content[0]["text"].as_str().unwrap();
        assert!(content[0]["type"] == "text" && !text.contains('\n'), "{text}");
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), answer, "the whole answer, the path too, for a client that reads files");
        assert_eq!((&content[1]["type"], &content[1]["mimeType"]), (&json!("image"), &json!("image/png")));
        assert_eq!(decoded(&content[1]), PNG);
    }

    #[test]
    fn a_still_comes_as_compact_text_then_its_picture_and_only_from_its_shelf() {
        let dir = tempfile::tempdir().unwrap();
        let stills = dir.path().join("video-stills");
        std::fs::create_dir_all(&stills).unwrap();
        let path = stills.join(format!("{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&path, PNG).unwrap();
        let answer = json!({"png": path, "frames": [{"frame": 45, "at_ms": 1500.0, "label": "1.5s · f45"}], "issues": [], "version": 2});
        let result = still_success(answer.clone(), None);
        let content = result["content"].as_array().unwrap();
        assert_eq!(serde_json::from_str::<Value>(content[0]["text"].as_str().unwrap()).unwrap(), answer);
        assert_eq!((&content[1]["type"], &content[1]["mimeType"]), (&json!("image"), &json!("image/png")));
        assert!(result.get("structuredContent").is_none());
        // A render's folder is not a still's: the path stays a path.
        let elsewhere = rendered(dir.path(), PNG);
        let other = still_success(json!({"png": elsewhere}), None);
        assert_eq!(other["content"][1]["type"], "text");
    }

    #[test]
    fn a_render_result_has_no_structured_content() {
        let dir = tempfile::tempdir().unwrap();
        let with = render_success(render_answer(Some(&rendered(dir.path(), PNG))), None);
        assert!(with.get("structuredContent").is_none(), "Claude Code would give the model its own JSON of it in place of the text");
        // Boxes alone (`png: false`, or nothing visible): the text, and nothing after it.
        let without = render_success(render_answer(None), None);
        assert!(without.get("structuredContent").is_none());
        assert_eq!(without["content"].as_array().unwrap().len(), 1);
        assert_eq!(serde_json::from_str::<Value>(without["content"][0]["text"].as_str().unwrap()).unwrap(), render_answer(None));
    }

    #[test]
    fn a_symlink_a_foreign_folder_a_non_uuid_name_or_a_big_file_is_not_attached() {
        let dir = tempfile::tempdir().unwrap();
        let good = rendered(dir.path(), PNG);
        let renders = good.parent().unwrap().to_path_buf();
        let id = || format!("{}.png", uuid::Uuid::new_v4());
        let link = renders.join(id());
        std::os::unix::fs::symlink(&good, &link).unwrap();
        let foreign = dir.path().join("renders");
        std::fs::create_dir_all(&foreign).unwrap();
        let elsewhere = foreign.join(id());
        std::fs::write(&elsewhere, PNG).unwrap();
        let named = renders.join("shot.png");
        std::fs::write(&named, PNG).unwrap();
        let mut big_bytes = PNG.to_vec();
        big_bytes.resize(relay_core::picture::UPLOAD_MAX + 1, 0);
        let big = rendered(dir.path(), &big_bytes);
        let text = rendered(dir.path(), b"not a picture");
        let svg = rendered(dir.path(), b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"></svg>");
        let around = renders.join("..").join("design-renders").join(good.file_name().unwrap());
        let relative = Path::new("design-renders").join(good.file_name().unwrap());
        let folder = renders.join(id());
        std::fs::create_dir(&folder).unwrap();
        let missing = renders.join(id());
        for path in [&link, &elsewhere, &named, &big, &text, &svg, &around, &relative, &folder, &missing] {
            let result = render_success(render_answer(Some(path)), None);
            let content = result["content"].as_array().unwrap();
            assert!(content.iter().all(|c| c["type"] == "text"), "{} was attached", path.display());
            assert_eq!(content.len(), 2);
            assert_eq!(content[1]["text"], format!("The picture could not be attached; it is at {}.", path.display()));
        }
        // The same file, named as the engine names it, is.
        assert_eq!(render_success(render_answer(Some(&good)), None)["content"][1]["type"], "image");
        let mut fits = PNG.to_vec();
        fits.resize(relay_core::picture::UPLOAD_MAX, 0);
        assert_eq!(render_success(render_answer(Some(&rendered(dir.path(), &fits))), None)["content"][1]["type"], "image", "4 MiB is allowed");
    }

    #[test]
    fn mail_still_follows_a_render() {
        let dir = tempfile::tempdir().unwrap();
        let mail = MailHint { priority: 3 };
        let result = render_success(render_answer(Some(&rendered(dir.path(), PNG))), Some(&mail));
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.iter().map(|c| c["type"].as_str().unwrap()).collect::<Vec<_>>(), ["text", "image", "text"]);
        assert!(content[2]["text"].as_str().unwrap().contains("3 unread"));
        let without = render_success(render_answer(None), Some(&mail));
        assert!(without["content"][1]["text"].as_str().unwrap().contains("mailbox.list"));
    }

    const OPENVERSE_ID: &str = "48e4625b-0930-43db-990c-22858b1d633e";

    /// `<dir>/picture-cache/<name>` holding `bytes`, as the engine keeps a thumbnail.
    fn kept_thumb(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let cache = dir.join("picture-cache");
        std::fs::create_dir_all(&cache).unwrap();
        let path = cache.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn thumb_answer(path: &Path) -> Value {
        json!({"path": path, "w": 1, "h": 1})
    }

    /// pictures-SPEC-v2 §5.3: an agent sees a preview without reading the store's folder, which
    /// its sandbox may not reach.
    #[test]
    fn a_thumbnail_comes_as_compact_text_then_the_image() {
        let dir = tempfile::tempdir().unwrap();
        for name in [format!("openverse-{OPENVERSE_ID}.png"), "pexels-2014422.png".to_string()] {
            let path = kept_thumb(dir.path(), &name, PNG);
            let result = thumb_success(thumb_answer(&path), None);
            assert_eq!(result["isError"], false);
            assert!(result.get("structuredContent").is_none());
            let content = result["content"].as_array().unwrap();
            assert_eq!(serde_json::from_str::<Value>(content[0]["text"].as_str().unwrap()).unwrap(), thumb_answer(&path));
            assert_eq!((&content[1]["type"], &content[1]["mimeType"]), (&json!("image"), &json!("image/png")), "{name}");
            assert_eq!(decoded(&content[1]), PNG);
        }
        assert_eq!(THUMBS.max, relay_core::pictures::net::Want::Thumb.cap(), "the most the engine keeps");
    }

    #[test]
    fn a_thumbnail_the_engine_did_not_keep_stays_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let good = kept_thumb(dir.path(), &format!("openverse-{OPENVERSE_ID}.png"), PNG);
        let cache = good.parent().unwrap().to_path_buf();
        let link = cache.join("pexels-1.png");
        std::os::unix::fs::symlink(&good, &link).unwrap();
        let foreign = dir.path().join("design-assets");
        std::fs::create_dir_all(&foreign).unwrap();
        let elsewhere = foreign.join(format!("openverse-{OPENVERSE_ID}.png"));
        std::fs::write(&elsewhere, PNG).unwrap();
        let mut big_bytes = PNG.to_vec();
        big_bytes.resize(3 << 20, 0);
        let big = kept_thumb(dir.path(), "pexels-2.png", &big_bytes);
        let svg = kept_thumb(dir.path(), "pexels-3.png", b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"></svg>");
        let svg_named = kept_thumb(dir.path(), "pexels-4.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"></svg>");
        let render = kept_thumb(dir.path(), &format!("{}.png", uuid::Uuid::new_v4()), PNG);
        let odd_names: Vec<PathBuf> = ["flickr-1.png", "pexels-0123.png", "pexels-.png", "openverse-x.png", "pexels-5.bmp", "pexels-6", "photo.png"]
            .iter().map(|name| kept_thumb(dir.path(), name, PNG)).collect();
        let around = cache.join("..").join("picture-cache").join(good.file_name().unwrap());
        let relative = Path::new("picture-cache").join(good.file_name().unwrap());
        let mut paths = vec![link, elsewhere, big, svg, svg_named, render, around, relative, cache.join("pexels-7.png")];
        paths.extend(odd_names);
        for path in &paths {
            let content = thumb_success(thumb_answer(path), None)["content"].clone();
            let content = content.as_array().unwrap();
            assert!(content.iter().all(|c| c["type"] == "text"), "{} was attached", path.display());
            assert_eq!(content[1]["text"], format!("The picture could not be attached; it is at {}.", path.display()));
        }
        // A render's picture is not a thumbnail, nor a thumbnail a render's picture.
        assert_eq!(thumb_success(thumb_answer(&good), None)["content"][1]["type"], "image");
        assert_eq!(render_success(json!({"png": {"path": good}}), None)["content"][1]["type"], "text");
    }

    #[test]
    fn priority_mail_is_visible_without_changing_the_typed_result() {
        let mail = MailHint { priority: 2 };
        let success = tool_success(json!({"task_id": 7}), Some(&mail)).unwrap();
        assert_eq!(success["structuredContent"], json!({"task_id": 7}));
        assert_eq!(success["content"].as_array().unwrap().len(), 2);
        assert!(success["content"][1]["text"].as_str().unwrap().contains("2 unread"));

        let error = tool_error(json!({"code":"task.not_found"}), Some(&mail));
        assert_eq!(error["isError"], true);
        assert!(error["content"][1]["text"].as_str().unwrap().contains("mailbox.list"));
    }

    /// A writer the test can read back, standing in for stdout.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            lock(&self.0).extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Captured {
        fn output(&self) -> Output {
            Output(Arc::new(Mutex::new(Box::new(self.clone()))))
        }
        /// The ids answered, in the order the answers were written.
        fn ids(&self) -> Vec<Value> {
            String::from_utf8(lock(&self.0).clone()).unwrap().lines()
                .map(|line| serde_json::from_str::<Value>(line).expect("every answer is one whole JSON line")["id"].clone())
                .collect()
        }
    }

    fn call(id: u64, name: &str) -> String {
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":{}}}).to_string()
    }

    fn cancel(id: u64) -> String {
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id,"reason":"user"}}).to_string()
    }

    /// `slow` takes 300 ms, `fast` none; `serial_*` tools share one lane.
    fn plugin_handle(message: &Value) -> Option<Value> {
        let name = message.pointer("/params/name").and_then(Value::as_str).unwrap_or_default();
        if name.ends_with("slow") {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        if name == "serial_marker" {
            assert!(!std::mem::replace(&mut *lock(&SERIAL_BUSY), true), "two serial calls ran at once");
            std::thread::sleep(std::time::Duration::from_millis(50));
            *lock(&SERIAL_BUSY) = false;
        }
        Some(json!({"jsonrpc":"2.0","id":message["id"],"result":{}}))
    }
    static SERIAL_BUSY: Mutex<bool> = Mutex::new(false);

    fn plugin_lane(name: &str, _args: &Value) -> Lane {
        if name.starts_with("serial") { Lane::Serial } else { Lane::Parallel }
    }

    #[test]
    fn plugin_calls_run_side_by_side_and_cancelled_ones_go_unanswered() {
        let captured = Captured::default();
        let input = [
            json!({"jsonrpc":"2.0","id":0,"method":"ping"}).to_string(),
            call(1, "slow"),
            call(2, "fast"),
            "not json".to_string(),
            call(3, "slow"),
            cancel(3),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
            call(4, "serial_slow"),
            call(5, "serial_slow"),
            cancel(5),
        ].join("\n");
        let started = std::time::Instant::now();
        serve_sync_on(std::io::Cursor::new(input), captured.output(), plugin_handle, plugin_lane).unwrap();
        let ids = captured.ids();
        // The fast call is answered while the slow one is still running.
        assert!(ids.iter().position(|id| id == 2) < ids.iter().position(|id| id == 1), "{ids:?}");
        assert!(ids.contains(&Value::Null), "the parse error is answered");
        assert!(!ids.contains(&json!(3)), "a cancelled call gets no answer: {ids:?}");
        assert!(!ids.contains(&json!(5)), "a cancelled call waiting for its lane gets no answer: {ids:?}");
        assert_eq!(ids.len(), 5, "{ids:?}");
        // 1, 3 and 4 overlap; 5 never ran.
        assert!(started.elapsed() < std::time::Duration::from_millis(900), "{:?}", started.elapsed());
    }

    #[test]
    fn serial_plugin_calls_wait_their_turn() {
        let captured = Captured::default();
        let input = (1..=4).map(|id| call(id, "serial_marker")).chain([call(9, "fast")]).collect::<Vec<_>>().join("\n");
        serve_sync_on(std::io::Cursor::new(input), captured.output(), plugin_handle, plugin_lane).unwrap();
        assert_eq!(captured.ids().len(), 5);
    }

    #[tokio::test]
    async fn bus_calls_run_side_by_side_and_cancelling_aborts_the_call() {
        let captured = Captured::default();
        let (sender, lines) = mpsc::unbounded_channel();
        let aborted = Arc::new(Mutex::new(true));
        let flag = aborted.clone();
        let server = tokio::spawn(dispatch(lines, captured.output(), move |message: Value| {
            let flag = flag.clone();
            async move {
                let name = message.pointer("/params/name").and_then(Value::as_str).unwrap_or_default().to_string();
                if name == "bus.wait" {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    *lock(&flag) = false;
                }
                message.get("id").map(|id| json!({"jsonrpc":"2.0","id":id,"result":{}}))
            }
        }));
        for line in [call(1, "bus.wait"), call(2, "bus.ping"), cancel(1), call(3, "bus.ping")] {
            sender.send(Ok(line)).unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        drop(sender);
        server.await.unwrap().unwrap();
        assert_eq!(captured.ids(), [json!(2), json!(3)], "the wait neither blocks later calls nor answers once cancelled");
        assert!(*lock(&aborted), "the cancelled call was stopped, not left to finish");
    }

    #[test]
    fn plugin_code_meets_an_exec_gate_and_its_saves_meet_write_gates() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("art/hero.blend");
        let copy = root.join("art/hero_v2.blend");
        let gates = plugin_gate_payloads("calm-otter", Some(&root), "blender_python", Some(&file), &[&copy]);
        assert_eq!(gates, [
            json!({"session": "calm-otter", "kind": "exec", "command": "blender_python art/hero.blend"}),
            json!({"session": "calm-otter", "kind": "write", "path": copy, "diff": ""}),
        ]);
        assert_eq!(plugin_gate_payloads("calm-otter", Some(&root), "ue_python", None, &[]),
            [json!({"session": "calm-otter", "kind": "exec", "command": "ue_python"})]);
    }

    #[test]
    fn cancellations_name_the_request_they_cancel() {
        assert_eq!(cancelled(&serde_json::from_str(&cancel(7)).unwrap()), Some("7".to_string()));
        let by_string = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"7"}});
        assert_eq!(cancelled(&by_string), Some("\"7\"".to_string()), "string and number ids stay distinct");
        assert_eq!(cancelled(&serde_json::from_str(&call(7, "x")).unwrap()), None);
    }
}
