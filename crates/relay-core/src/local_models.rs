//! Local models for Threads (docs/THREADS.md, "Local models").
//!
//! A thread may run on a model on this PC instead of one of Anthropic's. Its agent is still
//! Claude Code, pointed at a llama.cpp server (`llama-server`) that answers Anthropic's Messages
//! API. Relay starts the server when a local model is picked or a thread on one gets a message,
//! and stops it when another model is picked or no thread has used it for
//! `threads.local.idle_minutes`, so the model holds the graphics card only while it is wanted.
//! One runs at a time: starting another stops the first, and the agents talking to it.
//!
//! Loading a model takes seconds, so nothing here runs inside a handler. The server is started
//! from a thread of its own that waits on it, with the parent-death signal set, so it never
//! outlives Relay.

use crate::engine::Engine;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, Instant};

/// A model Relay can run here.
pub struct Model {
    /// The thread's model id, which Claude Code sends to the server as it is. A provider ref:
    /// letters, digits, `-` and `_`.
    pub id: &'static str,
    pub name: &'static str,
    /// Its GGUF file in the models folder.
    pub file: &'static str,
    /// The chat template the server renders it with, in place of the one in the file.
    template: &'static str,
}

/// The local models a thread may pick, each offered once its file is in the models folder
/// (`threads.local.dir`, `~/models` when unset).
pub const MODELS: &[Model] = &[
    Model { id: "qwen35-9b", name: "Qwen3.5 9B", file: "Qwen3.5-9B-Q5_K_M.gguf", template: QWEN35 },
    Model { id: "qwen35-4b", name: "Qwen3.5 4B", file: "Qwen3.5-4B-Q8_0.gguf", template: QWEN35 },
];

/// Qwen3.5's own template refuses a system message after the first, and Claude Code sends its
/// environment as one; this copy renders it as a system turn where it stands.
const QWEN35: &str = include_str!("local_models/qwen3.5.jinja");

/// Minutes unused before the server stops when `threads.local.idle_minutes` is unset; 0 keeps it.
pub const IDLE_MINUTES: u64 = 15;
/// The context of the server's one conversation. Claude Code's prompt and Relay's tools are a
/// few thousand tokens of it; the rest is the thread.
const CONTEXT: u32 = 32_768;
/// The longest a model may take to load before its start counts as failed.
const LOAD_WITHIN: Duration = Duration::from_secs(180);

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

struct Server {
    model: &'static Model,
    pid: u32,
    port: u16,
    /// Which start this is, so the waiter of a server that was replaced cannot clear the new one.
    serial: u64,
    ready: bool,
    last_used: Instant,
}

/// The engine's local model server, when one runs.
#[derive(Default)]
pub struct Local {
    server: Mutex<Option<Server>>,
    /// Held while a server loads, so two starts cannot run two.
    starting: Mutex<()>,
    next_serial: AtomicU64,
    watcher: OnceLock<()>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn setting(engine: &Engine, path: &str) -> Option<Value> {
    let conn = engine.store.lock();
    let raw: Option<String> = conn.prepare_cached("SELECT value FROM settings WHERE path=?1").ok()?
        .query_row([path], |row| row.get(0)).optional().ok().flatten();
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// The folder the models' files are read from.
pub fn dir(engine: &Engine) -> PathBuf {
    setting(engine, "threads.local.dir").and_then(|v| v.as_str().map(PathBuf::from)).unwrap_or_else(|| home().join("models"))
}

/// `llama-server`: `threads.local.server`, else on PATH, else in `~/.local/bin`, where a
/// desktop launch's PATH often does not reach.
pub fn server_bin(engine: &Engine) -> Result<PathBuf, String> {
    if let Some(path) = setting(engine, "threads.local.server").and_then(|v| v.as_str().map(PathBuf::from)) {
        return if path.is_file() { Ok(path) } else { Err(format!("threads.local.server = {} does not exist", path.display())) };
    }
    which::which("llama-server").ok()
        .or_else(|| Some(home().join(".local/bin/llama-server")).filter(|p| p.is_file()))
        .ok_or_else(|| "llama-server is not installed: build llama.cpp, or set threads.local.server".to_string())
}

/// Minutes a server may sit unused before it stops; 0 never.
pub fn idle_minutes(engine: &Engine) -> u64 {
    setting(engine, "threads.local.idle_minutes").and_then(|v| v.as_u64()).unwrap_or(IDLE_MINUTES)
}

/// The model the server runs, and whether it has loaded.
pub fn status(engine: &Engine) -> Option<(&'static Model, bool)> {
    lock(&engine.local.server).as_ref().map(|s| (s.model, s.ready))
}

/// A thread on a local model just got a message: the idle clock starts over.
pub fn touch(engine: &Engine) {
    if let Some(server) = lock(&engine.local.server).as_mut() {
        server.last_used = Instant::now();
    }
}

fn emit(engine: &Engine, model: &Model, state: &str, error: Option<&str>) {
    engine.emit_system("thread.local", json!({"model": model.id, "name": model.name, "state": state, "error": error}));
}

/// A model was picked in the Threads picker: a local one starts loading now, so the first
/// message does not wait for it, and any other stops the server.
pub fn pick(engine: &Arc<Engine>, model: Option<&str>) {
    match model.and_then(find) {
        Some(model) => {
            let engine = engine.clone();
            let _ = std::thread::Builder::new().name("local-model-pick".into()).spawn(move || {
                let _ = ensure(&engine, model);
            });
        }
        None => stop(engine),
    }
}

/// Stop the server, if one runs, and the agents talking to it.
pub fn stop(engine: &Engine) {
    let Some(server) = lock(&engine.local.server).take() else { return };
    kill(server.pid);
    crate::threads::stop_local(engine);
    emit(engine, server.model, "stopped", None);
}

/// The port of a server running `model`, started and loaded first when it is not. Blocks while
/// the model loads. A failure is announced (`thread.local`) and returned; a start cut short by
/// [`stop`] or by another model's start is only returned.
pub fn ensure(engine: &Arc<Engine>, model: &'static Model) -> Result<u16, String> {
    // Another model loading would hold `starting` until it finished: stop it first.
    if status(engine).is_some_and(|(running, _)| running.id != model.id) {
        stop(engine);
    }
    let _starting = lock(&engine.local.starting);
    if let Some(server) = lock(&engine.local.server).as_mut().filter(|s| s.model.id == model.id && s.ready) {
        server.last_used = Instant::now();
        return Ok(server.port);
    }
    stop(engine);
    let failed = |why: String| {
        emit(engine, model, "failed", Some(&why));
        Err(why)
    };
    let file = dir(engine).join(model.file);
    if !file.is_file() {
        return failed(format!("{} is not in {}", model.file, dir(engine).display()));
    }
    let bin = match server_bin(engine) {
        Ok(bin) => bin,
        Err(why) => return failed(why),
    };
    let port = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).and_then(|l| l.local_addr()) {
        Ok(addr) => addr.port(),
        Err(e) => return failed(format!("No free port for the model server: {e}")),
    };
    let log = log_path(engine);
    let template = log.with_file_name("local-model.jinja");
    if let Err(e) = std::fs::write(&template, model.template) {
        return failed(format!("{}: {e}", template.display()));
    }
    let serial = engine.local.next_serial.fetch_add(1, Ordering::Relaxed) + 1;
    emit(engine, model, "loading", None);

    let mut cmd = Command::new(&bin);
    cmd.arg("-m").arg(&file)
        .args(["--alias", model.id, "--host", "127.0.0.1", "--port", &port.to_string()])
        // One conversation with the whole context; thinking off, which is most of a small
        // model's wait. The layers on the card are fitted to its free memory.
        .args(["-c", &CONTEXT.to_string(), "-np", "1", "--jinja", "--reasoning", "off", "--no-webui"])
        .arg("--chat-template-file").arg(&template)
        .stdin(Stdio::null());
    match std::fs::File::create(&log).and_then(|f| Ok((f.try_clone()?, f))) {
        Ok((out, err)) => cmd.stdout(out).stderr(err),
        Err(_) => cmd.stdout(Stdio::null()).stderr(Stdio::null()),
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        // SAFETY: prctl is async-signal-safe and touches nothing of the parent's.
        unsafe {
            cmd.pre_exec(|| {
                #[cfg(target_os = "linux")]
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }
    // The thread that starts the server waits on it: the parent-death signal follows the thread
    // that forked, so the server dies with this one, which ends only when the server does or
    // Relay does.
    let (tx, rx) = mpsc::channel();
    let weak: Weak<Engine> = Arc::downgrade(engine);
    let waiter = std::thread::Builder::new().name("local-model".into()).spawn(move || {
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = tx.send(Err(format!("Could not start llama-server: {e}")));
                return;
            }
        };
        let _ = tx.send(Ok(child.id()));
        let _ = child.wait();
        if let Some(engine) = weak.upgrade() {
            exited(&engine, serial);
        }
    });
    if let Err(e) = waiter {
        return failed(format!("Could not start llama-server: {e}"));
    }
    let pid = match rx.recv() {
        Ok(Ok(pid)) => pid,
        Ok(Err(why)) => return failed(why),
        Err(_) => return failed("Could not start llama-server".into()),
    };
    *lock(&engine.local.server) = Some(Server { model, pid, port, serial, ready: false, last_used: Instant::now() });
    start_watcher(engine);

    let deadline = Instant::now() + LOAD_WITHIN;
    loop {
        if healthy(port) {
            break;
        }
        if !lock(&engine.local.server).as_ref().is_some_and(|s| s.serial == serial) {
            return Err(format!("{} stopped before it loaded", model.name));
        }
        if Instant::now() >= deadline {
            stop(engine);
            return failed(format!("{} did not load within {} seconds", model.name, LOAD_WITHIN.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    match lock(&engine.local.server).as_mut().filter(|s| s.serial == serial) {
        Some(server) => {
            server.ready = true;
            server.last_used = Instant::now();
        }
        None => return Err(format!("{} stopped before it loaded", model.name)),
    }
    emit(engine, model, "ready", None);
    Ok(port)
}

/// The environment that points a thread's Claude Code at the server on `port`. Every model
/// Claude Code would reach for, its background one included, is the local one.
pub fn agent_env(port: u16, model: &Model) -> Vec<(&'static str, String)> {
    let mut env = vec![
        ("ANTHROPIC_BASE_URL", format!("http://127.0.0.1:{port}")),
        ("ANTHROPIC_AUTH_TOKEN", "relay-local".to_string()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_string()),
    ];
    for var in ["ANTHROPIC_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"] {
        env.push((var, model.id.to_string()));
    }
    env
}

fn log_path(engine: &Engine) -> PathBuf {
    let store = engine.store.path();
    if store == std::path::Path::new(":memory:") {
        std::env::temp_dir().join(format!("relay-local-model-{}.log", std::process::id()))
    } else {
        store.with_file_name("local-model.log")
    }
}

/// The last line of the server's log that says something, for a failure's message.
fn log_tail(engine: &Engine) -> Option<String> {
    let text = std::fs::read_to_string(log_path(engine)).ok()?;
    let line = text.lines().rev().find(|l| l.contains(" E ") || l.contains("error")).or_else(|| text.lines().rev().find(|l| !l.trim().is_empty()))?;
    Some(line.trim().chars().take(300).collect())
}

/// The server's process ended. Unless [`stop`] ended it, it died: say so.
fn exited(engine: &Engine, serial: u64) {
    let died = {
        let mut server = lock(&engine.local.server);
        if server.as_ref().is_some_and(|s| s.serial == serial) { server.take() } else { None }
    };
    let Some(server) = died else { return };
    crate::threads::stop_local(engine);
    let why = log_tail(engine).map_or_else(|| format!("{} stopped", server.model.name), |line| format!("{} stopped: {line}", server.model.name));
    emit(engine, server.model, "failed", Some(&why));
}

/// GET /health answers 200 once the model has loaded, 503 while it loads.
fn healthy(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { return false };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).is_ok() && head.starts_with(b"HTTP/1.1 200")
}

#[cfg(unix)]
fn kill(pid: u32) {
    // The server leads its own process group.
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        unsafe {
            libc::killpg(pid, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn kill(_pid: u32) {}

/// Stop a server unused for `threads.local.idle_minutes`; one watcher per engine, gone with it.
/// A reply in progress counts as use.
fn start_watcher(engine: &Arc<Engine>) {
    if engine.local.watcher.set(()).is_err() {
        return;
    }
    let weak: Weak<Engine> = Arc::downgrade(engine);
    let _ = std::thread::Builder::new().name("local-model-idle".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let Some(engine) = weak.upgrade() else { return };
        let minutes = idle_minutes(&engine);
        if minutes == 0 || lock(&engine.local.server).as_ref().is_none_or(|s| !s.ready) {
            continue;
        }
        if crate::threads::local_busy(&engine) {
            touch(&engine);
            continue;
        }
        let idle = lock(&engine.local.server).as_ref().is_some_and(|s| s.last_used.elapsed() >= Duration::from_secs(minutes * 60));
        if idle {
            stop(&engine);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ids_are_model_refs_a_thread_can_store() {
        for model in MODELS {
            assert!(crate::providers::is_provider_ref(model.id), "{}", model.id);
            assert!(model.file.ends_with(".gguf"), "{}", model.file);
        }
    }

    #[test]
    fn the_agent_is_pointed_at_the_server_for_every_model_it_would_use() {
        let env = agent_env(4321, &MODELS[0]);
        let get = |k: &str| env.iter().find(|(var, _)| *var == k).map(|(_, v)| v.as_str());
        assert_eq!(get("ANTHROPIC_BASE_URL"), Some("http://127.0.0.1:4321"));
        assert_eq!(get("ANTHROPIC_SMALL_FAST_MODEL"), Some(MODELS[0].id));
        assert_eq!(get("ANTHROPIC_DEFAULT_HAIKU_MODEL"), Some(MODELS[0].id));
        assert!(get("ANTHROPIC_AUTH_TOKEN").is_some());
    }

    #[test]
    fn a_later_system_message_is_a_turn_not_an_error() {
        assert!(!QWEN35.contains("System message must be at the beginning"));
        assert!(QWEN35.contains("{%- if not loop.first %}\n            {{- '<|im_start|>system\\n' + content + '<|im_end|>\\n' }}"));
    }

    #[test]
    fn nothing_answering_is_not_healthy() {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap().port();
        assert!(!healthy(port));
    }
}
