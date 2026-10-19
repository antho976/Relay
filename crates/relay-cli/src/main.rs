//! `relay` — the CLI door (BUS.md §8). A thin client of the socket door: argument shaping,
//! defaults from the environment, exit codes. No logic of its own.

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use relay_bus::{Actor, Registry, Request, Response};
use relay_core::socket::Client;
use relay_core::Instance;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

mod mcp;
mod units;
mod unreal;
mod unreal_process;
mod blender;
mod blender_sandbox;
mod remote;
mod cloud;
mod write_targets;

#[derive(Parser)]
#[command(name = "relay", version, about = "RELAY v4 command bus CLI", long_about = None)]
struct Cli {
    /// Instance to talk to: stable | dev | test (env RELAY_INSTANCE)
    #[arg(long, global = true, env = "RELAY_INSTANCE", default_value = "stable")]
    instance: String,
    /// Actor override: user | test (agents come from RELAY_SESSION + RELAY_TOKEN)
    #[arg(long, global = true)]
    actor: Option<String>,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run one op: `relay cmd <op> [<json-payload>|-]` or `relay cmd '<json-envelope>'`
    Cmd { op: String, payload: Option<String> },
    /// Like cmd, but prints the result only, pretty
    Q { op: String, payload: Option<String> },
    /// Subscribe and print Event lines until killed
    Events {
        /// Filter: exact name, `prefix.*`, repeatable
        #[arg(long)]
        filter: Vec<String>,
    },
    /// Print bus.v1.json, or one op's payload+result schema
    Schema { op: Option<String> },
    /// List ops
    Ops {
        /// Only ops my actor may really call, all gating layers applied (needs a running engine)
        #[arg(long)]
        mine: bool,
        /// Substring filter on op name and summary
        #[arg(long)]
        grep: Option<String>,
    },
    /// Liveness check
    Ping,
    /// Attach to a session's PTY and print its output (decoded) until killed
    Attach { session: String },
    /// Run a headless engine (owns the socket for the instance)
    Serve {
        #[arg(long)]
        store: Option<PathBuf>,
        /// Also open the phone door (`relay remote serve`) in this process
        #[arg(long)]
        remote: bool,
        /// Address the phone door listens on, with --remote
        #[arg(long, default_value_t = format!("0.0.0.0:{}", relay_remote::DEFAULT_PORT))]
        remote_bind: String,
        /// Which ops a paired device may call, with --remote: `phone` (the PC's own door) or
        /// `cloud` (an always-on copy of the ledger and training history on a server, docs/CLOUD.md)
        #[arg(long, default_value = "phone")]
        remote_ops: String,
    },
    /// Serve callable bus ops as MCP tools over stdio
    Mcp,
    /// The Unreal Engine plugin's MCP server (stdio): project, build, log and live-editor tools
    UnrealMcp,
    /// The Blender plugin's MCP server (stdio): background Blender on the checkout's .blend files
    BlenderMcp,
    /// The phone door: pair a phone, serve it on the LAN or through a rendezvous you host
    Remote {
        #[command(subcommand)]
        remote: remote::RemoteCommand,
    },
    /// This PC's link to a cloud engine that Tally and Avex sync with while the PC is off (docs/CLOUD.md)
    Cloud {
        #[command(subcommand)]
        cloud: cloud::CloudCommand,
    },
    /// Internal provider hook adapters. Enforcement fails closed; lifecycle reports are best-effort.
    #[command(hide = true)]
    Hook {
        #[command(subcommand)]
        hook: HookCommand,
    },
}

#[derive(Subcommand)]
enum HookCommand {
    /// Translate Claude Code PreToolUse JSON on stdin into `guardrail.gate`.
    ClaudePreTool,
    /// Forward a Claude lifecycle hook payload to `session.report`.
    ClaudeReport { kind: String },
    /// Translate Codex PreToolUse JSON on stdin into `guardrail.gate`.
    CodexPreTool,
    /// Forward a Codex lifecycle hook payload to `session.report`.
    CodexReport { kind: String },
    /// Forward a Codex `notify` payload to `session.report`.
    CodexNotify { payload: String },
    /// Run the project's check before an agent's push (`guardrails.check_on_push`); refs on stdin.
    PrePush,
}

const EXIT_UNREACHABLE: u8 = 5;

/// An error with its own exit code (BUS.md §8.2). Any other error from `run` exits 5, "engine
/// unreachable"; a malformed payload or a missing project used to say the same, which sent
/// callers to restart an engine that was fine (RA-283).
#[derive(Debug)]
struct Exit(u8, String);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for Exit {}

/// A mistake in the call itself, found before the engine is asked: exit 1, as `invalid` is.
fn invalid(message: impl std::fmt::Display) -> anyhow::Error {
    anyhow::Error::new(Exit(1, message.to_string()))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    // A provider runs the tool when its PreToolUse hook exits with anything but 2, so a hook
    // that fails before it can ask (a bad --instance) blocks too.
    let gate = matches!(&cli.cmd, Command::Hook { hook: HookCommand::ClaudePreTool | HookCommand::CodexPreTool });
    match run(cli).await {
        Ok(code) => ExitCode::from(code),
        Err(e) if gate => {
            eprintln!("RELAY guardrail unavailable: {e:#}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("relay: {e:#}");
            ExitCode::from(e.downcast_ref::<Exit>().map_or(EXIT_UNREACHABLE, |exit| exit.0))
        }
    }
}

fn actor_from_env(override_: Option<&str>) -> Result<(Actor, Option<String>)> {
    if let Some(a) = override_ {
        return Ok((Actor::parse(a).map_err(invalid)?, std::env::var("RELAY_TOKEN").ok()));
    }
    if let Ok(name) = std::env::var("RELAY_SESSION") {
        let token = std::env::var("RELAY_TOKEN").ok();
        return Ok((Actor::Agent(name), token));
    }
    if let Ok(a) = std::env::var("RELAY_ACTOR") {
        return Ok((Actor::parse(&a).map_err(invalid)?, None));
    }
    Ok((Actor::User, None))
}

fn read_payload(arg: Option<String>) -> Result<Value> {
    match arg.as_deref() {
        None => Ok(json!({})),
        Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            serde_json::from_str(&s).map_err(|e| invalid(format!("payload on stdin is not JSON: {e}")))
        }
        Some(s) => serde_json::from_str(s).map_err(|e| invalid(format!("payload is not JSON: {e}"))),
    }
}

/// Does this op's payload schema require `project_id`?
fn requires_project(op: &str) -> bool {
    relay_bus::schema::render_op(op)
        .and_then(|s| s["payload"]["required"].as_array().map(|r| r.iter().any(|v| v == "project_id")))
        .unwrap_or(false)
}

/// Repo root of the cwd, by walking up to a `.git`.
fn cwd_repo_root() -> Option<PathBuf> {
    let mut p = std::env::current_dir().ok()?;
    loop {
        if p.join(".git").exists() {
            return std::fs::canonicalize(&p).ok();
        }
        if !p.pop() {
            return None;
        }
    }
}

async fn fill_project(client: &mut Client, actor: &Actor, token: &Option<String>, op: &str, payload: &mut Value) -> Result<()> {
    if !requires_project(op) || payload.get("project_id").is_some() {
        return Ok(());
    }
    if let Ok(p) = std::env::var("RELAY_PROJECT") {
        let id: i64 = p.parse().map_err(|e| invalid(format!("RELAY_PROJECT must be a project id: {e}")))?;
        payload["project_id"] = json!(id);
        return Ok(());
    }
    if let Some(root) = cwd_repo_root() {
        let mut req = Request::new(actor.clone(), "project.list", json!({}));
        if let Some(t) = token { req = req.with_token(t.clone()); }
        let resp = client.call(&req, |_| {}).await?;
        if let Ok(v) = resp.into_result() {
            let root_s = root.display().to_string();
            if let Some(p) = v["projects"].as_array().and_then(|ps| ps.iter().find(|p| p["path"] == root_s)) {
                payload["project_id"] = p["id"].clone();
                return Ok(());
            }
        }
    }
    Err(invalid(format!("{op} needs project_id: pass it in the payload, set RELAY_PROJECT, or run from inside a registered project (bus.project_required)")))
}

fn exit_for(resp: &Response) -> u8 {
    match &resp.error {
        None => 0,
        Some(e) => e.kind.exit_code() as u8,
    }
}

async fn connect(instance: Instance) -> Result<Client> {
    let path = instance.socket_path();
    Client::connect(&path).await.with_context(|| format!("no engine for instance {instance} at {}", path.display()))
}

async fn run(cli: Cli) -> Result<u8> {
    let instance = Instance::parse(&cli.instance).ok_or_else(|| invalid(format!("bad --instance {:?}", cli.instance)))?;
    match cli.cmd {
        Command::Serve { store, remote, remote_bind, remote_ops } => {
            let profile = relay_remote::wire::OpProfile::parse(&remote_ops)
                .ok_or_else(|| invalid(format!("bad --remote-ops {remote_ops:?}: phone or cloud")))?;
            // The engine outlives launcher worktrees. Git libraries and remote helpers
            // still consult process CWD even when given absolute repository paths.
            let store = store.map(std::path::absolute).transpose()?;
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
            std::env::set_current_dir(&home).context("entering the engine home directory")?;
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("RELAY_LOG").unwrap_or_else(|_| "info".into()))
                .init();
            let served = if remote {
                remote::serve_with_door(instance, store, &remote_bind, profile).await
            } else {
                relay_core::serve::serve(instance, store).await
            };
            match served {
                Ok(()) => Ok(0),
                Err(relay_core::socket::BindError::AlreadyRunning { instance, pid, socket }) => {
                    eprintln!("relay: engine already running for instance {instance} (pid {}) at {}", pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into()), socket.display());
                    Ok(EXIT_UNREACHABLE)
                }
                Err(relay_core::socket::BindError::Other(e)) => Err(e),
            }
        }
        Command::Mcp => {
            let (actor, token) = actor_from_env(cli.actor.as_deref())?;
            mcp::serve(instance, actor, token).await
        }
        Command::UnrealMcp => {
            mcp::use_instance(instance);
            unreal::serve()
        }
        Command::BlenderMcp => {
            mcp::use_instance(instance);
            blender::serve()
        }
        Command::Remote { remote } => remote::run(instance, remote).await,
        Command::Cloud { cloud } => cloud::run(instance, cloud).await,
        Command::Schema { op } => {
            match op {
                None => print!("{}", relay_bus::schema::render_pretty()),
                Some(op) => {
                    let s = relay_bus::schema::render_op(&op).ok_or_else(|| invalid(format!("unknown op {op:?}")))?;
                    println!("{}", serde_json::to_string_pretty(&s)?);
                }
            }
            Ok(0)
        }
        Command::Attach { session } => {
            use base64::Engine as _;
            let (actor, token) = actor_from_env(cli.actor.as_deref())?;
            let mut c = connect(instance).await?;
            let mut req = Request::new(actor, "session.attach", json!({ "session": session }));
            if let Some(t) = token { req = req.with_token(t); }
            let resp = c.call(&req, |_| {}).await?;
            if !resp.ok {
                println!("{}", serde_json::to_string(&resp)?);
                return Ok(exit_for(&resp));
            }
            use std::io::Write;
            let mut out = std::io::stdout();
            loop {
                match c.next().await? {
                    None => return Ok(0),
                    Some(relay_core::socket::Line::Frame(f)) => {
                        if let Some(b) = f.data.as_str().and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()) {
                            out.write_all(&b)?;
                            out.flush()?;
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        Command::Ping => {
            let mut c = connect(instance).await?;
            let req = Request::new(Actor::User, "bus.ping", json!({}));
            let resp = c.call(&req, |_| {}).await?;
            println!("{}", serde_json::to_string(&resp)?);
            Ok(exit_for(&resp))
        }
        Command::Ops { mine, grep } => {
            let (actor, token) = actor_from_env(cli.actor.as_deref())?;
            let ops: Vec<Value> = match connect(instance).await {
                Ok(mut c) => {
                    let mut req = Request::new(actor.clone(), "bus.ops", if mine { json!({}) } else { json!({"actor": "user"}) });
                    if let Some(t) = &token { req = req.with_token(t.clone()); }
                    let resp = c.call(&req, |_| {}).await?;
                    match resp.into_result() {
                        Ok(v) => v["ops"].as_array().cloned().unwrap_or_default(),
                        Err(e) => return Err(anyhow::Error::new(Exit(e.kind.exit_code() as u8, format!("bus.ops failed: {} {}", e.code, e.message)))),
                    }
                }
                Err(_) => {
                    if mine { anyhow::bail!("--mine needs a running engine"); }
                    Registry::global().entries().iter().map(|e| serde_json::to_value(relay_bus::registry::OpInfo::from_entry(e, false)).unwrap()).collect()
                }
            };
            let needle = grep.map(|g| g.to_ascii_lowercase());
            let matches = |o: &Value| match &needle {
                None => true,
                Some(needle) => {
                    let name = o["name"].as_str().unwrap_or("").to_ascii_lowercase();
                    let summary = o["summary"].as_str().unwrap_or("").to_ascii_lowercase();
                    name.contains(needle) || summary.contains(needle)
                }
            };
            // `--mine` used to answer with layer 1 only, which overstated a builder's write
            // surface tenfold. Every row now says whether it is really callable and why.
            println!("{:<32} {:<9} {:>3}  {:<5} {:<5} {:<34} summary", "op", "kind", "ph", "impl", "call", "why");
            let mut shown = 0;
            for o in ops.iter().filter(|o| matches(o)) {
                shown += 1;
                let call = match o["call"].as_str() {
                    Some("no") => "NO",
                    Some("self_only") => "SELF",
                    Some("yes") => "YES",
                    _ => "-",
                };
                println!(
                    "{:<32} {:<9} {:>3}  {:<5} {:<5} {:<34} {}",
                    o["name"].as_str().unwrap_or(""),
                    o["kind"].as_str().unwrap_or(""),
                    o["phase"],
                    if o["implemented"].as_bool().unwrap_or(false) { "yes" } else { "-" },
                    call,
                    o["why"].as_str().unwrap_or(""),
                    o["summary"].as_str().unwrap_or(""),
                );
            }
            if mine {
                let callable = ops.iter().filter(|o| matches(o) && o["call"] != "no").count();
                println!("\n{shown} listed, {callable} callable (SELF = own session only)");
            }
            Ok(0)
        }
        Command::Events { filter } => {
            // As whoever is asking: inside a session the socket refuses a `user` claim (D165).
            let (actor, token) = actor_from_env(cli.actor.as_deref())?;
            let mut c = connect(instance).await?;
            let mut req = Request::new(actor, "bus.subscribe", json!({ "events": filter }));
            if let Some(t) = token { req = req.with_token(t); }
            let resp = c.call(&req, |_| {}).await?;
            if !resp.ok {
                println!("{}", serde_json::to_string(&resp)?);
                return Ok(exit_for(&resp));
            }
            loop {
                match c.next().await? {
                    None => return Ok(0),
                    Some(relay_core::socket::Line::Event(e)) => println!("{}", serde_json::to_string(&e)?),
                    Some(relay_core::socket::Line::Response(r)) => println!("{}", serde_json::to_string(&r)?),
                    Some(relay_core::socket::Line::Frame(f)) => println!("{}", serde_json::to_string(&f)?),
                }
            }
        }
        Command::Hook { hook: HookCommand::ClaudePreTool } => {
            match claude_pre_tool(instance, cli.actor.as_deref()).await {
                Ok(code) => Ok(code),
                Err(error) => {
                    // Claude Code only treats exit 2 as blocking. Infrastructure and malformed
                    // hook input must therefore fail closed too, not use the CLI's normal 5.
                    eprintln!("RELAY guardrail unavailable: {error:#}");
                    Ok(2)
                }
            }
        }
        Command::Hook { hook: HookCommand::ClaudeReport { kind } } => {
            if let Err(error) = claude_report(instance, cli.actor.as_deref(), &kind).await {
                // Lifecycle reporting is observability, not enforcement. A down Relay must
                // never trap Claude in a Stop-hook retry loop.
                eprintln!("RELAY lifecycle report unavailable: {error:#}");
            }
            Ok(0)
        }
        Command::Hook { hook: HookCommand::CodexPreTool } => {
            match codex_pre_tool(instance, cli.actor.as_deref()).await {
                Ok(code) => Ok(code),
                Err(error) => {
                    eprintln!("RELAY guardrail unavailable: {error:#}");
                    Ok(2)
                }
            }
        }
        Command::Hook { hook: HookCommand::CodexReport { kind } } => {
            if let Err(error) = lifecycle_hook_report(instance, cli.actor.as_deref(), &kind, "Codex").await {
                eprintln!("RELAY lifecycle report unavailable: {error:#}");
            }
            Ok(0)
        }
        Command::Hook { hook: HookCommand::CodexNotify { payload } } => {
            if let Err(error) = codex_notify(instance, cli.actor.as_deref(), &payload).await {
                eprintln!("RELAY lifecycle report unavailable: {error:#}");
            }
            Ok(0)
        }
        Command::Hook { hook: HookCommand::PrePush } => pre_push(instance, cli.actor.as_deref()).await,
        Command::Cmd { op, payload } => do_cmd(instance, cli.actor.as_deref(), op, payload, false).await,
        Command::Q { op, payload } => do_cmd(instance, cli.actor.as_deref(), op, payload, true).await,
    }
}

async fn claude_pre_tool(instance: Instance, actor_override: Option<&str>) -> Result<u8> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let session = std::env::var("RELAY_SESSION").context("RELAY_SESSION is missing")?;
    let input: Value = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(error) => return Ok(cannot_check(&anyhow!(error).context("Claude hook input is not JSON"))),
    };
    match claude_payloads(&input, &session) {
        Ok(payloads) => gate(instance, actor_override, input["tool_name"].as_str().unwrap_or("?"), payloads).await,
        Err(error) => Ok(cannot_check(&error)),
    }
}

/// The tool call itself could not be turned into gates — malformed input, or an edit whose
/// result cannot be previewed. Blocked, like an outage, but said in its own words (RA-284):
/// "guardrail unavailable" sent agents looking for an engine fault that was not there.
fn cannot_check(error: &anyhow::Error) -> u8 {
    eprintln!("RELAY cannot check this tool call, so it is blocked: {error:#}");
    2
}

/// The gates a Claude Code PreToolUse call must pass, in order. Pure apart from reading the
/// file an Edit changes, so that the gate judges the text the edit will leave.
fn claude_payloads(input: &Value, session: &str) -> Result<Vec<Value>> {
    let tool = input["tool_name"]
        .as_str()
        .ok_or_else(|| anyhow!("Claude hook input has no tool_name"))?;
    let tool_input = input["tool_input"]
        .as_object()
        .ok_or_else(|| anyhow!("Claude hook input has no tool_input object"))?;
    let write = |path: &str, text: String| write_gate(session, Path::new(path), "new_text", json!(text));
    Ok(match tool {
        "Bash" => {
            let command = required_string(tool_input, "command")?;
            let mut payloads = vec![json!({
                "session": session,
                "kind": "exec",
                "command": command,
            })];
            payloads.extend(shell_write_gates(input, session, command));
            payloads
        }
        "Write" => vec![write(required_string(tool_input, "file_path")?, required_string(tool_input, "content")?.to_string())],
        "Edit" => {
            let file_path = required_string(tool_input, "file_path")?;
            let edit = (required_string(tool_input, "old_string")?, required_string(tool_input, "new_string")?,
                tool_input.get("replace_all").and_then(Value::as_bool).unwrap_or(false));
            vec![write(file_path, edited_text(file_path, &[edit])?)]
        }
        // Kept for Claude versions that still expose the former batched edit tool.
        "MultiEdit" => {
            let file_path = required_string(tool_input, "file_path")?;
            let edits = tool_input.get("edits").and_then(Value::as_array)
                .ok_or_else(|| anyhow!("MultiEdit input has no edits array"))?
                .iter()
                .map(|edit| {
                    let object = edit.as_object().ok_or_else(|| anyhow!("MultiEdit edit is not an object"))?;
                    Ok((required_string(object, "old_string")?, required_string(object, "new_string")?,
                        object.get("replace_all").and_then(Value::as_bool).unwrap_or(false)))
                })
                .collect::<Result<Vec<_>>>()?;
            vec![write(file_path, edited_text(file_path, &edits)?)]
        }
        // A notebook is JSON whose cells Claude rewrites by id; rebuilding it here would be a
        // second notebook editor. The path rules (protected paths, write roots) still meet it.
        "NotebookEdit" => vec![write_gate(session, Path::new(required_string(tool_input, "notebook_path")?), "diff", json!(""))],
        other => anyhow::bail!("unsupported Claude PreToolUse tool {other:?}"),
    })
}

/// The text `file_path` holds after Claude's Edit/MultiEdit `edits` (old, new, replace_all),
/// applied in order the way Claude applies them. An empty `old_string` on a file that does not
/// exist yet (or is empty) creates it. Anything the edit tool itself would refuse fails here
/// too, rather than letting the gate judge text the edit will never produce.
fn edited_text(file_path: &str, edits: &[(&str, &str, bool)]) -> Result<String> {
    anyhow::ensure!(!edits.is_empty(), "edit of {file_path} has no edits");
    let mut next = match std::fs::read_to_string(file_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("reading edit target {file_path}")),
    };
    for (old, new, replace_all) in edits {
        let creates = old.is_empty() && next.as_deref().is_none_or(str::is_empty);
        next = Some(match next {
            _ if creates => new.to_string(),
            None => anyhow::bail!("edit target {file_path} does not exist"),
            Some(_) if old.is_empty() => anyhow::bail!("an empty old_string only creates a new file; {file_path} already has content"),
            Some(text) if !text.contains(old) => anyhow::bail!("old_string does not occur in {file_path}; re-read the file"),
            Some(text) if *replace_all => text.replace(old, new),
            Some(text) => text.replacen(old, new, 1),
        });
    }
    next.ok_or_else(|| anyhow!("edit target {file_path} does not exist"))
}

/// How long a PreToolUse hook waits for the guardrail. Claude Code and Codex kill a hook after
/// 30 s and then run the tool unchecked, so the hook gives up first and blocks (D23).
const GATE_DEADLINE: Duration = Duration::from_secs(20);

/// Run every gate a tool call needs, in order, blocking (exit 2) at the first that refuses or
/// holds, or when the guardrail does not answer in time.
async fn gate(instance: Instance, actor_override: Option<&str>, tool: &str, payloads: Vec<Value>) -> Result<u8> {
    let (actor, token) = actor_from_env(actor_override)?;
    run_gates(connect(instance), actor, token, tool, payloads, GATE_DEADLINE).await
}

async fn run_gates(
    client: impl std::future::Future<Output = Result<Client>>,
    actor: Actor,
    token: Option<String>,
    tool: &str,
    payloads: Vec<Value>,
    deadline: Duration,
) -> Result<u8> {
    match first_refusal(client, actor, token, payloads, deadline).await? {
        None => Ok(0),
        Some(error) => {
            eprintln!("RELAY blocked {tool}: {}", refusal_text(error.as_ref()));
            Ok(2)
        }
    }
}

/// Send each gate in order and stop at the first that does not allow: `Some(error)` names it
/// (`Some(None)` when the guardrail failed without saying why). An unreachable or silent engine
/// is an `Err`, which every caller treats as a block.
async fn first_refusal(
    client: impl std::future::Future<Output = Result<Client>>,
    actor: Actor,
    token: Option<String>,
    payloads: Vec<Value>,
    deadline: Duration,
) -> Result<Option<Option<relay_bus::BusError>>> {
    let checks = async {
        let mut client = client.await?;
        for payload in payloads {
            let mut request = Request::new(actor.clone(), "guardrail.gate", payload);
            if let Some(token) = &token {
                request = request.with_token(token.clone());
            }
            let response = client.call(&request, |_| {}).await?;
            if !response.ok {
                return Ok(Some(response.error));
            }
        }
        Ok(None)
    };
    tokio::time::timeout(deadline, checks)
        .await
        .map_err(|_| anyhow!("the guardrail did not answer within {} s", deadline.as_secs_f32()))?
}

fn refusal_text(error: Option<&relay_bus::BusError>) -> String {
    match error {
        Some(error) => match error.hint.as_deref() {
            Some(hint) => format!("{} ({})\nRELAY hint: {hint}", error.message, error.code),
            None => format!("{} ({})", error.message, error.code),
        },
        None => "guardrail returned no result".to_string(),
    }
}

/// Write gates for the files a shell command visibly writes (write_targets). Overwrites and
/// deletions say how many lines go, so the destructive-write rule can weigh them; edits in
/// place and appends carry an empty diff, which still meets protected paths, write roots and
/// shape gates.
fn shell_write_gates(input: &Value, session: &str, command: &str) -> Vec<Value> {
    let Some(cwd) = input["cwd"].as_str().map(PathBuf::from).or_else(|| std::env::current_dir().ok()) else {
        return Vec::new();
    };
    write_targets::shell_writes(command, &cwd)
        .into_iter()
        .map(|write| {
            let diff = match write.effect {
                write_targets::Effect::Overwrite | write_targets::Effect::Delete => removed_lines(&write.path),
                write_targets::Effect::Append | write_targets::Effect::Modify => String::new(),
            };
            write_gate(session, &write.path, "diff", json!(diff))
        })
        .collect()
}

/// A diff that removes every line of `path`, for a write whose new content is unknown. Only the
/// '-' markers count, so the lines themselves are not copied; past 200 000 the count is capped,
/// which is already over any destructive-write limit.
fn removed_lines(path: &Path) -> String {
    let lines = match std::fs::metadata(path) {
        // Counted the way str::lines counts: a last line without a newline still counts.
        Ok(meta) if meta.is_file() && meta.len() <= 64 * 1024 * 1024 => std::fs::read(path)
            .map(|bytes| bytes.iter().filter(|b| **b == b'\n').count() + usize::from(bytes.last().is_some_and(|b| *b != b'\n')))
            .unwrap_or(0),
        _ => 0,
    };
    "-\n".repeat(lines.min(200_000))
}

async fn claude_report(instance: Instance, actor_override: Option<&str>, kind: &str) -> Result<()> {
    lifecycle_hook_report(instance, actor_override, kind, "Claude").await
}

async fn lifecycle_hook_report(instance: Instance, actor_override: Option<&str>, kind: &str, provider: &str) -> Result<()> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let data: Value = if raw.trim().is_empty() { json!({}) } else {
        serde_json::from_str(&raw).with_context(|| format!("{provider} lifecycle hook input is not JSON"))?
    };
    lifecycle_report(instance, actor_override, kind, data).await
}

async fn codex_pre_tool(instance: Instance, actor_override: Option<&str>) -> Result<u8> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let session = std::env::var("RELAY_SESSION").context("RELAY_SESSION is missing")?;
    let input: Value = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(error) => return Ok(cannot_check(&anyhow!(error).context("Codex hook input is not JSON"))),
    };
    match codex_payloads(&input, &session) {
        Ok(payloads) => gate(instance, actor_override, input["tool_name"].as_str().unwrap_or("?"), payloads).await,
        Err(error) => Ok(cannot_check(&error)),
    }
}

/// The gates a Codex PreToolUse call must pass, in order.
fn codex_payloads(input: &Value, session: &str) -> Result<Vec<Value>> {
    let tool = input["tool_name"].as_str().ok_or_else(|| anyhow!("Codex hook input has no tool_name"))?;
    let tool_input = input["tool_input"].as_object().ok_or_else(|| anyhow!("Codex hook input has no tool_input object"))?;
    let command = required_string(tool_input, "command")?;
    Ok(match tool {
        "Bash" => {
            let mut payloads = vec![json!({
                "session": session,
                "kind": "exec",
                "command": command,
            })];
            payloads.extend(shell_write_gates(input, session, command));
            payloads
        }
        // Codex edits files through apply_patch, whose `command` is the patch (Edit and Write
        // are matcher aliases for it). Each file meets the write gate, as Claude's Write and
        // Edit do; the patch text is not a command line to run exec rules on.
        "apply_patch" | "Edit" | "Write" => patch_write_gates(input, session, command)?,
        other => anyhow::bail!("unsupported Codex PreToolUse tool {other:?}"),
    })
}

/// One write gate per file an apply_patch touches. An added file carries its text and an
/// updated one its text after the hunks, so shape gates can validate it; a deleted file says
/// how many lines go. A moved file's source loses nothing, so only the path rules meet it.
fn patch_write_gates(input: &Value, session: &str, patch: &str) -> Result<Vec<Value>> {
    use write_targets::PatchOp;
    let cwd = input["cwd"].as_str().map(PathBuf::from).or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow!("cannot tell which directory the patch is relative to"))?;
    let mut payloads = Vec::new();
    for op in write_targets::parse_patch(patch)? {
        match op {
            PatchOp::Add { path, text } => payloads.push(write_gate(session, &cwd.join(path), "new_text", json!(text))),
            PatchOp::Delete { path } => {
                let path = cwd.join(path);
                payloads.push(write_gate(session, &path, "diff", json!(removed_lines(&path))));
            }
            PatchOp::Update { path, move_to, chunks } => {
                let source = cwd.join(path);
                let (field, value) = match std::fs::read_to_string(&source).ok().and_then(|old| write_targets::apply_chunks(&old, &chunks)) {
                    Some(next) => ("new_text", json!(next)),
                    // apply_patch would refuse this hunk as well. The diff still gives the
                    // destructive rule its counts; a shape-gated file is held for want of text.
                    None => ("diff", json!(write_targets::chunk_diff(&chunks))),
                };
                match move_to {
                    Some(to) => {
                        payloads.push(write_gate(session, &source, "diff", json!("")));
                        payloads.push(write_gate(session, &cwd.join(to), field, value));
                    }
                    None => payloads.push(write_gate(session, &source, field, value)),
                }
            }
        }
    }
    Ok(payloads)
}

fn write_gate(session: &str, path: &Path, field: &str, value: Value) -> Value {
    let mut payload = json!({
        "session": session,
        "kind": "write",
        "path": gate_path(path),
    });
    payload[field] = value;
    payload
}

async fn codex_notify(instance: Instance, actor_override: Option<&str>, raw: &str) -> Result<()> {
    let data: Value = serde_json::from_str(raw).context("Codex notification payload is not JSON")?;
    if data.get("type").and_then(Value::as_str) != Some("agent-turn-complete") {
        return Ok(());
    }
    lifecycle_report(instance, actor_override, "stop", data).await
}

async fn lifecycle_report(instance: Instance, actor_override: Option<&str>, kind: &str, data: Value) -> Result<()> {
    let session = std::env::var("RELAY_SESSION").context("RELAY_SESSION is missing")?;
    let (actor, token) = actor_from_env(actor_override)?;
    let mut client = connect(instance).await?;
    let mut request = Request::new(actor, "session.report", json!({
        "session": session,
        "kind": kind,
        "data": report_data(data),
    }));
    if let Some(token) = token { request = request.with_token(token); }
    let response = client.call(&request, |_| {}).await?;
    response.into_result().map(|_| ()).map_err(|error| anyhow!("{}: {}", error.code, error.message))
}

/// The hook payload as `session.report` gets it. A PostToolUse payload carries the tool's whole
/// output in `tool_response` — a file read, a build log — which the report never reads, yet it
/// crossed the socket and was hashed into the audit on every tool call (RA-401).
fn report_data(mut data: Value) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.remove("tool_response");
    }
    data
}

fn required_string<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("hook input field {key:?} is missing or not a string"))
}

/// The path `guardrail.gate` should judge: the one the tool names. The gate relativizes it
/// against the session's own worktree and judges it where it lands, through symlinks, so this
/// side keeps no second idea of the worktree (RA-589). Only a `..` is resolved here, through the
/// file system, since the gate refuses one in an absolute path; one the file system cannot
/// resolve yet is left for the gate to refuse.
fn gate_path(path: &Path) -> String {
    let dotted = path.is_absolute() && path.components().any(|part| part == std::path::Component::ParentDir);
    if dotted { resolve_existing(path) } else { path.to_path_buf() }.display().to_string()
}

/// `path` with its longest existing prefix resolved through the file system (symlinks and
/// `..` included) and the part that does not exist yet appended as written.
fn resolve_existing(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(existing) {
            return rest.iter().rev().fold(real, |acc: PathBuf, part: &&std::ffi::OsStr| acc.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// How much of a failed check's output the refusal shows: the end, where the failure is.
const CHECK_TAIL_LINES: usize = 80;

/// `guardrails.check_on_push` (check.rs): run the project's check in the pushing worktree and
/// refuse the push when it fails. Its output is shown only on failure, so a passing push costs
/// the agent one line. Anything that stops the check from being found — no engine, a checkout
/// that is not a Relay project, a project with nothing to run — lets the push through with a
/// note: this is a check the agent is meant to pass, not a fence, and GitHub's required checks
/// are the fence.
async fn pre_push(instance: Instance, actor_override: Option<&str>) -> Result<u8> {
    let mut refs = String::new();
    std::io::stdin().read_to_string(&mut refs)?;
    if !relay_core::check::pushes_commits(&refs) {
        return Ok(0);
    }
    let worktree = std::env::current_dir()?;
    let command = match push_check(instance, actor_override, &worktree).await {
        Ok(Some(command)) => command,
        Ok(None) => return Ok(0),
        Err(error) => {
            eprintln!("relay: the push was not checked: {error:#}");
            return Ok(0);
        }
    };
    let started = std::time::Instant::now();
    let ran = relay_core::check::check_command(&command, &worktree)
        .output()
        .with_context(|| format!("running `{command}`"))?;
    let seconds = started.elapsed().as_secs();
    if ran.status.success() {
        eprintln!("relay: `{command}` passed in {seconds}s; pushing.");
        return Ok(0);
    }
    let output = String::from_utf8_lossy(&ran.stdout);
    let lines: Vec<&str> = output.lines().collect();
    let tail = &lines[lines.len().saturating_sub(CHECK_TAIL_LINES)..];
    if tail.len() < lines.len() {
        eprintln!("[… {} earlier lines]", lines.len() - tail.len());
    }
    for line in tail {
        eprintln!("{line}");
    }
    let status = ran.status.code().map_or_else(|| "killed by a signal".to_string(), |code| format!("exit {code}"));
    eprintln!(
        "relay: push refused: `{command}` failed ({status}, {seconds}s). Fix what it reports and push again; \
         do not skip the hook. A person can change the check (the project's build command) or turn it off \
         (guardrails.check_on_push)."
    );
    Ok(1)
}

/// The command [`pre_push`] runs for `worktree`, or `None` when the project has the check off
/// or nothing to run.
async fn push_check(instance: Instance, actor_override: Option<&str>, worktree: &Path) -> Result<Option<String>> {
    // A session worktree's project is the repository its git directory belongs to.
    let common = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(worktree)
        .output()
        .context("asking git for the repository")?;
    if !common.status.success() {
        bail!("git rev-parse failed: {}", String::from_utf8_lossy(&common.stderr).trim());
    }
    let common = PathBuf::from(String::from_utf8_lossy(&common.stdout).trim());
    let repo = common.parent().map(Path::to_path_buf).context("the git directory has no parent")?;
    let repo = std::fs::canonicalize(&repo).unwrap_or(repo);

    let (actor, token) = actor_from_env(actor_override)?;
    let mut client = connect(instance).await?;
    let mut ask = async |op: &str, payload: Value| -> Result<Value> {
        let mut request = Request::new(actor.clone(), op, payload);
        if let Some(token) = &token { request = request.with_token(token.clone()); }
        client.call(&request, |_| {}).await?.into_result().map_err(|error| anyhow!("{op}: {}: {}", error.code, error.message))
    };
    let projects = ask("project.list", json!({})).await?;
    let repo_s = repo.display().to_string();
    let Some(project) = projects["projects"].as_array().and_then(|all| all.iter().find(|p| p["path"] == repo_s.as_str())).cloned() else {
        return Ok(None);
    };
    let config = ask("guardrail.config.get", json!({"project_id": project["id"]})).await?;
    if config["check_on_push"] == false {
        return Ok(None);
    }
    let command = project["build_cmd"].as_str().map(str::trim).filter(|command| !command.is_empty()).map(str::to_owned)
        .or_else(|| relay_core::check::detect(worktree));
    if command.is_none() {
        eprintln!("relay: the push was not checked: project {} has no build command and none could be detected", project["name"]);
    }
    Ok(command)
}

async fn do_cmd(instance: Instance, actor_override: Option<&str>, op: String, payload: Option<String>, pretty: bool) -> Result<u8> {
    let (actor, token) = actor_from_env(actor_override)?;
    // The call is checked before the engine is looked for, so a malformed one exits 1 whether
    // or not an engine is running.
    let envelope = op.trim_start().starts_with('{');
    let mut req = if envelope {
        // full envelope
        let mut v: Value = serde_json::from_str(&op).map_err(|e| invalid(format!("envelope is not JSON: {e}")))?;
        if v.get("v").is_none() { v["v"] = json!(relay_bus::ENVELOPE_V); }
        if v.get("id").is_none() { v["id"] = json!(uuid::Uuid::new_v4()); }
        if v.get("actor").is_none() { v["actor"] = json!(actor.to_string()); }
        if v.get("token").is_none() { if let Some(t) = &token { v["token"] = json!(t); } }
        serde_json::from_value::<Request>(v).map_err(|e| invalid(format!("bad envelope: {e}")))?
    } else {
        let p = read_payload(payload)?;
        if !p.is_object() { return Err(invalid("payload must be a JSON object")); }
        let mut r = Request::new(actor.clone(), op, p);
        if let Some(t) = &token { r = r.with_token(t.clone()); }
        r
    };
    let mut c = connect(instance).await?;
    if !envelope {
        fill_project(&mut c, &actor, &token, &req.op, &mut req.payload).await?;
    }
    let resp = c.call(&req, |_| {}).await?;
    if pretty {
        match &resp.error {
            None => println!("{}", serde_json::to_string_pretty(resp.result.as_ref().unwrap_or(&Value::Null))?),
            // A typed refusal printed on stdout beside real results reads as a result — every
            // caller then has to special-case an error that arrived looking like success.
            Some(e) => eprintln!("{}", serde_json::to_string_pretty(e)?),
        }
    } else {
        println!("{}", serde_json::to_string(&resp)?);
    }
    if pretty {
        if let Some(mail) = &resp.mail {
            eprintln!(
                "RELAY priority mail: {} unread. Finish the current atomic action, then run mailbox.list with unread_only true and acknowledge each message.",
                mail.priority
            );
        }
    }
    Ok(exit_for(&resp))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RA-401: the tool's output is not forwarded; what session.report reads is.
    #[test]
    fn a_lifecycle_report_drops_the_tool_response() {
        let hook = json!({
            "session_id": "abc", "hook_event_name": "PostToolUse", "tool_name": "Bash",
            "tool_input": {"command": "adb install app.apk"}, "tool_response": {"stdout": "x".repeat(100_000)},
        });
        let sent = report_data(hook);
        assert!(sent.get("tool_response").is_none());
        assert_eq!(sent["session_id"], "abc");
        assert_eq!(sent["tool_input"]["command"], "adb install app.apk");
        assert_eq!(report_data(json!({"message": "hi"})), json!({"message": "hi"}));
        assert_eq!(report_data(json!(null)), json!(null));
    }

    #[tokio::test]
    async fn a_guardrail_that_never_answers_blocks_before_the_provider_gives_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("engine.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        // Accepts and then says nothing, like an engine whose gate waits behind the store lock.
        let stalled = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
            drop(stream);
        });
        let started = std::time::Instant::now();
        let payloads = vec![json!({"session": "calm-otter", "kind": "exec", "command": "true"})];
        let error = run_gates(Client::connect(&path), Actor::Agent("calm-otter".into()), None, "Bash", payloads, Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("did not answer"), "{error:#}");
        assert!(started.elapsed() < Duration::from_secs(5));
        stalled.abort();
    }

    #[test]
    fn shell_and_patch_writes_become_write_gates() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("data.ts"), "a\nb\nc").unwrap();
        std::fs::write(root.join("lib.rs"), "fn a() {}\nfn b() {}\n").unwrap();
        let input = json!({"cwd": root});

        let gates = shell_write_gates(&input, "calm-otter", ": > data.ts && echo x >> notes.md");
        assert_eq!(gates, [
            json!({"session": "calm-otter", "kind": "write", "path": root.join("data.ts"), "diff": "-\n-\n-\n"}),
            json!({"session": "calm-otter", "kind": "write", "path": root.join("notes.md"), "diff": ""}),
        ]);

        let patch = "*** Begin Patch\n*** Update File: lib.rs\n@@\n-fn b() {}\n+fn c() {}\n*** Add File: new.rs\n+x\n*** Delete File: data.ts\n*** End Patch";
        let gates = patch_write_gates(&input, "calm-otter", patch).unwrap();
        assert_eq!(gates[0]["new_text"], "fn a() {}\nfn c() {}\n");
        assert_eq!(gates[1], json!({"session": "calm-otter", "kind": "write", "path": root.join("new.rs"), "new_text": "x\n"}));
        assert_eq!(gates[2]["diff"], "-\n-\n-\n");
        assert!(gates.iter().all(|g| g["kind"] == "write"));
        assert!(patch_write_gates(&input, "calm-otter", "not a patch").is_err(), "an unreadable patch blocks");
    }

    /// A checkout with `src/lib.rs` holding "a b a\n", and a hook input builder for it.
    fn checkout() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "a b a\n").unwrap();
        (dir, root)
    }

    fn hook_input(root: &Path, tool: &str, tool_input: Value) -> Value {
        json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": tool_input, "cwd": root})
    }

    #[test]
    fn claude_tool_calls_become_the_gates_they_must_pass() {
        let (_dir, root) = checkout();
        let lib = root.join("src/lib.rs").display().to_string();
        let gates = |tool: &str, tool_input: Value| claude_payloads(&hook_input(&root, tool, tool_input), "calm-otter");

        let bash = gates("Bash", json!({"command": "cargo test && echo done > out.txt", "description": "test"})).unwrap();
        assert_eq!(bash[0], json!({"session": "calm-otter", "kind": "exec", "command": "cargo test && echo done > out.txt"}));
        assert_eq!(bash[1], json!({"session": "calm-otter", "kind": "write", "path": root.join("out.txt"), "diff": ""}));
        assert_eq!(bash.len(), 2);

        // Paths go to the gate as the tool names them; it relativizes them against the session's
        // worktree (RA-589).
        let write = gates("Write", json!({"file_path": root.join("docs/new.md"), "content": "hello\n"})).unwrap();
        assert_eq!(write, [json!({"session": "calm-otter", "kind": "write", "path": root.join("docs/new.md"), "new_text": "hello\n"})]);

        let edit = gates("Edit", json!({"file_path": lib, "old_string": "a", "new_string": "x"})).unwrap();
        assert_eq!(edit, [json!({"session": "calm-otter", "kind": "write", "path": lib, "new_text": "x b a\n"})]);
        let all = gates("Edit", json!({"file_path": lib, "old_string": "a", "new_string": "x", "replace_all": true})).unwrap();
        assert_eq!(all[0]["new_text"], "x b x\n");

        // Applied in order: the second edit only matches what the first one wrote.
        let multi = gates("MultiEdit", json!({"file_path": lib, "edits": [
            {"old_string": "a", "new_string": "c", "replace_all": true},
            {"old_string": "c b", "new_string": "d"},
        ]})).unwrap();
        assert_eq!(multi, [json!({"session": "calm-otter", "kind": "write", "path": lib, "new_text": "d c\n"})]);

        let notebook = gates("NotebookEdit", json!({"notebook_path": root.join("nb/a.ipynb"), "new_source": "print(1)", "edit_mode": "replace"})).unwrap();
        assert_eq!(notebook, [json!({"session": "calm-otter", "kind": "write", "path": root.join("nb/a.ipynb"), "diff": ""})]);

        // The file was never changed: these are previews only.
        assert_eq!(std::fs::read_to_string(&lib).unwrap(), "a b a\n");
    }

    #[test]
    fn edits_that_cannot_be_previewed_fail_closed() {
        let (_dir, root) = checkout();
        let lib = root.join("src/lib.rs").display().to_string();
        let gates = |tool: &str, tool_input: Value| claude_payloads(&hook_input(&root, tool, tool_input), "calm-otter");
        let error = |tool: &str, tool_input: Value| format!("{:#}", gates(tool, tool_input).unwrap_err());

        assert!(error("Edit", json!({"file_path": lib, "old_string": "zzz", "new_string": "x"})).contains("does not occur"));
        assert!(error("MultiEdit", json!({"file_path": lib, "edits": [
            {"old_string": "a", "new_string": "c"}, {"old_string": "missing", "new_string": "d"},
        ]})).contains("does not occur"));
        assert!(error("Edit", json!({"file_path": root.join("gone.rs"), "old_string": "a", "new_string": "x"})).contains("does not exist"));
        assert!(error("Edit", json!({"file_path": lib, "old_string": "", "new_string": "x"})).contains("already has content"));
        assert!(error("MultiEdit", json!({"file_path": lib})).contains("edits array"));
        assert!(error("MultiEdit", json!({"file_path": lib, "edits": []})).contains("no edits"));
        assert!(error("MultiEdit", json!({"file_path": lib, "edits": ["a"]})).contains("not an object"));
        assert!(error("Edit", json!({"file_path": lib, "old_string": "a"})).contains("new_string"));

        // An empty old_string on a file that is not there yet is how Edit creates one.
        let created = gates("Edit", json!({"file_path": root.join("src/new.rs"), "old_string": "", "new_string": "fn n() {}\n"})).unwrap();
        assert_eq!(created[0]["new_text"], "fn n() {}\n");
    }

    #[test]
    fn malformed_hook_input_fails_closed() {
        for input in [
            json!({}),
            json!({"tool_name": "Bash"}),
            json!({"tool_name": "Bash", "tool_input": "ls"}),
            json!({"tool_name": "Bash", "tool_input": {"command": ["ls"]}}),
            json!({"tool_name": 7, "tool_input": {"command": "ls"}}),
            json!({"tool_name": "Write", "tool_input": {"file_path": "/x"}}),
            json!({"tool_name": "Read", "tool_input": {"file_path": "/x"}}),
        ] {
            assert!(claude_payloads(&input, "calm-otter").is_err(), "Claude {input}");
            assert!(codex_payloads(&input, "calm-otter").is_err(), "Codex {input}");
        }
        assert_eq!(cannot_check(&anyhow!("no tool_name")), 2, "a hook that cannot check blocks");
    }

    #[test]
    fn codex_tool_calls_become_the_gates_they_must_pass() {
        let (_dir, root) = checkout();
        let gates = |tool: &str, tool_input: Value| codex_payloads(&hook_input(&root, tool, tool_input), "calm-otter");
        let lib = root.join("src/lib.rs");

        let bash = gates("Bash", json!({"command": "rm -f src/lib.rs"})).unwrap();
        assert_eq!(bash[0], json!({"session": "calm-otter", "kind": "exec", "command": "rm -f src/lib.rs"}));
        assert_eq!(bash[1], json!({"session": "calm-otter", "kind": "write", "path": lib, "diff": "-\n"}));

        // apply_patch (and its Edit/Write aliases) are writes, never exec gates on the patch text.
        let patch = "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-a b a\n+a b c\n*** End Patch";
        for tool in ["apply_patch", "Edit", "Write"] {
            let gates = gates(tool, json!({"command": patch})).unwrap();
            assert_eq!(gates, [json!({"session": "calm-otter", "kind": "write", "path": lib, "new_text": "a b c\n"})], "{tool}");
        }
        assert!(gates("Read", json!({"command": "x"})).is_err());
    }

    /// Where a path lands (symlinks, the worktree under another name) is the gate's to judge;
    /// only a `..` is settled here, because the gate refuses one in an absolute path.
    #[test]
    fn hook_paths_go_to_the_gate_as_named_with_dot_dot_resolved() {
        let (_dir, root) = checkout();
        let path = |p: &Path| gate_path(p);
        assert_eq!(path(&root.join("src/lib.rs")), root.join("src/lib.rs").display().to_string());
        assert_eq!(path(Path::new("src/lib.rs")), "src/lib.rs", "a relative path is already worktree-relative");
        std::os::unix::fs::symlink(root.join("src"), root.join("alias")).unwrap();
        assert_eq!(path(&root.join("alias/lib.rs")), root.join("alias/lib.rs").display().to_string(), "the gate follows links itself");

        // `..` the file system can resolve is resolved, out of the worktree as well as within it.
        assert_eq!(path(&root.join("src/../top.rs")), root.join("top.rs").display().to_string());
        assert_eq!(path(&root.join("../escape.txt")), root.parent().unwrap().join("escape.txt").display().to_string());
        // One it cannot is left for the gate, which refuses it.
        assert_eq!(path(&root.join("missing/../x.rs")), root.join("missing/../x.rs").display().to_string());
    }
}
