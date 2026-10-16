//! MCP stdio door (BUS.md §6.4). This is transport and argument shaping only: every tool
//! call is still a normal request through the socket door and the engine pipeline.

use crate::units;
use anyhow::{Context, Result};
use relay_bus::registry::{OpEntry, OpKind, Registry};
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
    dispatch(stdin_lines(), Output::stdout(), move |message| {
        let (actor, token) = (actor.clone(), token.clone());
        async move { handle(instance, &actor, token.as_deref(), message).await }
    })
    .await
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

/// A plugin call's result as MCP content. `_images` (label and PNG path) become image content,
/// up to [`MAX_IMAGES`]; `_cleanup` names a folder of them to delete once they are read.
pub(crate) fn tool_result(mut value: Value, is_error: bool) -> Value {
    let images = value.as_object_mut().and_then(|o| o.remove("_images")).and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let cleanup = value.as_object_mut().and_then(|o| o.remove("_cleanup"));
    let mut content = Vec::new();
    let mut shown = Vec::new();
    for image in images.iter().take(MAX_IMAGES) {
        let Some(path) = image["path"].as_str() else { continue };
        if let Ok(bytes) = std::fs::read(path) {
            use base64::Engine as _;
            content.push(json!({"type":"text","text":format!("Image: {}", image["label"].as_str().unwrap_or(""))}));
            content.push(json!({"type":"image","data":base64::engine::general_purpose::STANDARD.encode(bytes),"mimeType":"image/png"}));
            shown.push(image["label"].clone());
        }
    }
    if !images.is_empty() {
        value["images"] = json!({"shown": shown, "requested": images.len(), "limit": MAX_IMAGES});
    }
    if let Some(dir) = cleanup.as_ref().and_then(Value::as_str).filter(|dir| relay_scratch(Path::new(dir))) {
        let _ = std::fs::remove_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();
    content.insert(0, json!({"type":"text","text":text}));
    let mut result = json!({"content":content,"isError":is_error});
    if !is_error {
        result["structuredContent"] = value;
    }
    result
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
            let instructions = if Style::from_env().lean {
                "The person's own data: Tally, their budget (money.*), and Avex, their gym log (gym.*)."
            } else {
                "Relay's typed command bus. Tool names are bus op names; typed refusals are returned as tool errors. The tool list is filtered to what this session may actually call, all three gating layers applied — if an op is missing, your role cannot call it. Result shapes are not inlined here: `bus.schema {op}` returns one op's full payload and result schema. `session.bootstrap` returns your peers, your callable ops and your guardrails."
            };
            Ok(json!({
                "protocolVersion": protocol(&message),
                "capabilities": {"tools":{"listChanged":false}},
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
        Ok(tool_success(value, mail.as_ref())?)
    } else {
        Ok(refusal(serde_json::to_value(response.error)?, mail.as_ref(), &style))
    }
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
}

impl Style {
    fn from_env() -> Style {
        let spec = std::env::var("RELAY_MCP_STYLE").unwrap_or_default();
        let has = |part: &str| spec.split(',').any(|p| p.trim() == part);
        Style { lean: has("lean"), readings: has("readings"), docs: has("docs") }
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
    json!({
        "name":entry.name,
        "title":entry.name,
        "description":entry.meta.summary,
        "inputSchema":units::agent_schema(&schema["payload"]),
        "annotations":{
            "readOnlyHint":entry.meta.kind == OpKind::Query,
            "destructiveHint":entry.meta.kind == OpKind::Mutation,
            "openWorldHint":false
        }
    })
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

fn tool_success(value: Value, mail: Option<&MailHint>) -> Result<Value> {
    let mut content = vec![json!({
        "type":"text",
        "text":serde_json::to_string_pretty(&value)?
    })];
    if let Some(notice) = mail_notice(mail) {
        content.push(notice);
    }
    Ok(json!({"content":content,"structuredContent":value,"isError":false}))
}

fn tool_error(error: Value, mail: Option<&MailHint>) -> Value {
    let text = serde_json::to_string_pretty(&error).unwrap_or_else(|_| "Relay tool error".to_string());
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
        let plain = refusal(error.clone(), None, &Style { lean: true, readings: true, docs: false });
        assert_eq!(plain["isError"], true);
        assert_eq!(plain["content"][0]["text"], "Not done (money.not_found): No entry 5. Hint: list entries with money.tx.list.");
        let json = refusal(error, None, &Style::default());
        assert!(json["content"][0]["text"].as_str().unwrap().contains("\"code\": \"money.not_found\""));
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
