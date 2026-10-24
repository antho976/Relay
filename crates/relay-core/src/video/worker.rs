//! The video worker (D167): one Node.js process per instance, `videos/.relay/worker.mjs`, that
//! bundles, typechecks, draws stills and renders. It holds a bundle per video and one headless
//! browser between requests, which is what makes a still after an edit take a fraction of a
//! second instead of the many seconds a cold start costs.
//!
//! It speaks JSON lines: a request `{"id", "cmd", ...}` on its stdin, an answer `{"id", "ok",
//! "result" | "error"}` on its stdout in whatever order requests finish, and a render's progress
//! as `{"event": "progress", "job", ...}`. Callers block on their own answer with a deadline,
//! never holding the store lock (the video handlers are unlocked or staged). It starts on the
//! first request that needs it, dies with Relay (parent-death signal, as `local_models`), and
//! stops itself after ten idle minutes, the browser's memory with it.

use crate::engine::Engine;
use relay_bus::error::BusError;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::time::{Duration, Instant};

/// A worker left idle this long stops.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// Lines of its stderr kept to explain a death.
const TAIL: usize = 20;
/// How long `worker.log` grows before it starts again.
const LOG_MOST: u64 = 2 * 1024 * 1024;

type Reply = Result<Value, BusError>;
type Progress = Arc<dyn Fn(&Value) + Send + Sync>;

/// The live worker, if any.
#[derive(Default)]
pub struct Slot {
    live: Mutex<Option<Arc<Worker>>>,
    starting: Mutex<()>,
    serial: AtomicU64,
}

pub struct Worker {
    serial: u64,
    pid: u32,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Reply>>>,
    progress: Mutex<HashMap<String, Progress>>,
    next: AtomicU64,
    last_used: Mutex<Instant>,
    gone: AtomicBool,
    tail: Mutex<VecDeque<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The refusal a worker's `{code, message}` stands for.
pub fn refusal(code: &str, message: &str) -> BusError {
    match code {
        "video.composition" => BusError::invalid(code, message).with_hint("video.check lists the compositions; name one of their ids"),
        "video.entry" | "video.format" => BusError::invalid(code, message),
        "video.browser" => BusError::unavailable(code, message).with_hint("try again; if it keeps failing, video.kit.install sets the browser up again"),
        _ => BusError::unavailable(if code.starts_with("video.") { code } else { "video.worker" }, message),
    }
}

impl Worker {
    fn died(&self) -> BusError {
        let tail: Vec<String> = lock(&self.tail).iter().cloned().collect();
        BusError::unavailable("video.worker", "the video worker stopped")
            .with_details(json!({"stderr": tail}))
            .with_hint("try again: the next request starts it anew; videos/.relay/worker.log has its log")
    }

    fn request(&self, cmd: &str, args: Value, within: Duration) -> Reply {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let mut line = Map::new();
        line.insert("id".into(), id.into());
        line.insert("cmd".into(), cmd.into());
        if let Value::Object(args) = args {
            line.extend(args);
        }
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(id, tx);
        *lock(&self.last_used) = Instant::now();
        let mut text = serde_json::to_string(&line).unwrap_or_default();
        text.push('\n');
        let sent = match lock(&self.stdin).as_mut() {
            Some(stdin) => stdin.write_all(text.as_bytes()).and_then(|_| stdin.flush()).is_ok(),
            None => false,
        };
        if !sent {
            lock(&self.pending).remove(&id);
            return Err(self.died());
        }
        let reply = match rx.recv_timeout(within) {
            Ok(reply) => reply,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&id);
                Err(BusError::unavailable("video.timeout", format!("the video worker did not answer {cmd} within {} s", within.as_secs()))
                    .with_hint("a first bundle of a large video can be slow: try again"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(self.died()),
        };
        *lock(&self.last_used) = Instant::now();
        reply
    }

    fn close(&self) {
        lock(&self.stdin).take();
    }
}

/// Ask the worker `cmd` with `args` (an object) and wait for its answer up to `within`; starts the
/// worker when none runs, which needs the kit set up.
pub fn call(engine: &Engine, cmd: &str, args: Value, within: Duration) -> Reply {
    ensure(engine)?.request(cmd, args, within)
}

/// Have `f` hear the progress events of render job `job` until [`unwatch`]; starts the worker
/// when none runs, so the job's first events have someone to hear them.
pub fn watch(engine: &Engine, job: &str, f: impl Fn(&Value) + Send + Sync + 'static) -> Result<(), BusError> {
    let worker = ensure(engine)?;
    lock(&worker.progress).insert(job.to_string(), Arc::new(f));
    Ok(())
}

/// Have a running worker forget the video in `dir` (its bundle and TypeScript service, which
/// otherwise stay until the worker's idle stop); no worker is started for it.
pub fn forget(engine: &Engine, dir: &str) {
    let Some(worker) = lock(&engine.video.worker.live).clone().filter(|w| !w.gone.load(Ordering::SeqCst)) else { return };
    let dir = dir.to_string();
    let _ = std::thread::Builder::new().name("video-worker-forget".into()).spawn(move || {
        let _ = worker.request("forget", json!({"dir": dir}), Duration::from_secs(10));
    });
}

pub fn unwatch(engine: &Engine, job: &str) {
    if let Some(worker) = lock(&engine.video.worker.live).clone() {
        lock(&worker.progress).remove(job);
    }
}

/// Stop the worker, if one runs: its stdin closes, which it takes as the end, and after a grace
/// its process group is killed in case it did not.
pub fn stop(engine: &Engine) {
    let Some(worker) = lock(&engine.video.worker.live).take() else { return };
    reap(worker);
}

/// Close `worker`'s stdin, which it takes as the end, and kill its process group after a grace
/// in case it does not go (its browser wedged, a close that never settles).
fn reap(worker: Arc<Worker>) {
    worker.close();
    let pid = worker.pid;
    let _ = std::thread::Builder::new().name("video-worker-stop".into()).spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !worker.gone.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if !worker.gone.load(Ordering::SeqCst) {
            // Remotion starts its browser detached, in a group of its own, which the worker's
            // SIGKILL would not reach (it closes the browser itself only on a gentle end): the
            // groups its children lead go too, read before it dies.
            #[cfg(target_os = "linux")]
            let groups = child_groups(pid);
            #[cfg(unix)]
            // SAFETY: a signal to the process group the worker leads; nothing else is touched.
            unsafe {
                libc::killpg(pid as i32, libc::SIGKILL);
            }
            #[cfg(target_os = "linux")]
            for group in groups {
                // SAFETY: a signal to a group one of the worker's own children leads.
                unsafe {
                    libc::killpg(group, libc::SIGKILL);
                }
            }
        }
    });
}

/// The process groups `pid`'s children lead apart from its own (a detached browser's), from
/// `/proc`: each child whose group is itself.
#[cfg(target_os = "linux")]
fn child_groups(pid: u32) -> Vec<i32> {
    let mut groups = Vec::new();
    for task in std::fs::read_dir(format!("/proc/{pid}/task")).into_iter().flatten().flatten() {
        let children = std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
        for child in children.split_whitespace().filter_map(|c| c.parse::<i32>().ok()) {
            // SAFETY: getpgid reads a process's group; it touches nothing.
            let group = unsafe { libc::getpgid(child) };
            if group == child && group != pid as i32 && !groups.contains(&group) {
                groups.push(group);
            }
        }
    }
    groups
}

fn ensure(engine: &Engine) -> Result<Arc<Worker>, BusError> {
    let slot = &engine.video.worker;
    // While the kit is set up again its packages are being replaced: no worker runs on them.
    if engine.video.kit.installing.load(Ordering::SeqCst) {
        super::kit::ready(engine)?;
    }
    // One whose stdin is closed (put away as idle) takes no more requests, gone or not yet.
    let usable = |w: &Arc<Worker>| !w.gone.load(Ordering::SeqCst) && lock(&w.stdin).is_some();
    if let Some(worker) = lock(&slot.live).as_ref().filter(|w| usable(w)) {
        return Ok(worker.clone());
    }
    let _starting = lock(&slot.starting);
    if let Some(worker) = lock(&slot.live).as_ref().filter(|w| usable(w)) {
        return Ok(worker.clone());
    }
    // A worker closed but still there is let go of (killed after its grace), and a new one started.
    let closed = lock(&slot.live).take().filter(|w| !w.gone.load(Ordering::SeqCst));
    if let Some(worker) = closed {
        reap(worker);
    }
    super::kit::ready(engine)?;
    let root = engine.store.videos_dir();
    let program = match engine.video.program() {
        Some(program) => program,
        None => {
            super::kit::sync(&root).map_err(|e| BusError::unavailable("video.worker", format!("could not write the worker: {e}")))?;
            let node = which::which("node").map_err(|_| BusError::unavailable("video.kit.no_node", "node is not installed"))?;
            vec![node.display().to_string(), root.join(".relay/worker.mjs").display().to_string()]
        }
    };
    let worker = spawn(engine, &root, &program)?;
    *lock(&slot.live) = Some(worker.clone());
    Ok(worker)
}

fn spawn(engine: &Engine, root: &std::path::Path, program: &[String]) -> Result<Arc<Worker>, BusError> {
    let fail = |e: String| BusError::unavailable("video.worker", format!("could not start the video worker: {e}"));
    std::fs::create_dir_all(root.join(".relay")).map_err(|e| fail(e.to_string()))?;
    let mut cmd = Command::new(&program[0]);
    cmd.args(&program[1..]).current_dir(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
    // The thread that forks waits on the child: the parent-death signal follows the thread that
    // forked, so the worker dies with this one, which ends only when the worker does or Relay does.
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new().name("video-worker".into()).spawn(move || {
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        let pipes = (child.id(), child.stdin.take(), child.stdout.take(), child.stderr.take());
        let _ = tx.send(Ok(pipes));
        let _ = child.wait();
    }).map_err(|e| fail(e.to_string()))?;
    let (pid, stdin, stdout, stderr) = rx.recv().map_err(|_| fail("its thread ended".into()))?.map_err(fail)?;
    let (Some(stdout), Some(stderr)) = (stdout, stderr) else { return Err(fail("no pipes".into())) };
    let serial = engine.video.worker.serial.fetch_add(1, Ordering::SeqCst) + 1;
    let worker = Arc::new(Worker {
        serial,
        pid,
        stdin: Mutex::new(stdin),
        pending: Mutex::default(),
        progress: Mutex::default(),
        next: AtomicU64::new(1),
        last_used: Mutex::new(Instant::now()),
        gone: AtomicBool::new(false),
        tail: Mutex::default(),
    });

    // Every worker's lines, each with its pid (one engine's may outlive another's in a test run),
    // and started afresh whenever the log passes 2 MiB, at a start or while it runs. The page's
    // probe and schema lines (one a frame of every scan, up to 72 KB each) reach the worker on
    // their own and are left out.
    let log_path = root.join(".relay/worker.log");
    let for_log = worker.clone();
    let _ = std::thread::Builder::new().name("video-worker-log".into()).spawn(move || {
        let open = |fresh: bool| std::fs::OpenOptions::new().create(true).append(!fresh).write(true).truncate(fresh).open(&log_path).ok();
        let mut written = std::fs::metadata(&log_path).map_or(0, |m| m.len());
        let mut log = open(written > LOG_MOST);
        if written > LOG_MOST {
            written = 0;
        }
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.contains("__relay_probe__") || line.contains("__relay_schema__") {
                continue;
            }
            if written > LOG_MOST {
                log = open(true);
                written = 0;
            }
            if let Some(log) = log.as_mut() {
                let _ = writeln!(log, "[{pid}] {line}");
                written += line.len() as u64 + 16;
            }
            let mut tail = lock(&for_log.tail);
            if tail.len() == TAIL {
                tail.pop_front();
            }
            tail.push_back(line.chars().take(400).collect());
        }
    });

    let weak: Weak<Engine> = engine.arc().map(|e| Arc::downgrade(&e)).unwrap_or_default();
    let reader = worker.clone();
    std::thread::Builder::new().name("video-worker-read".into()).spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                tracing::debug!(line = %line, "video worker wrote a line that is not JSON");
                continue;
            };
            if message.get("event").is_some() {
                let job = message.get("job").and_then(Value::as_str).unwrap_or_default();
                let f = lock(&reader.progress).get(job).cloned();
                if let Some(f) = f {
                    f(&message);
                }
                continue;
            }
            let Some(id) = message.get("id").and_then(Value::as_u64) else { continue };
            let Some(tx) = lock(&reader.pending).remove(&id) else { continue };
            let reply = if message.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            } else {
                let error = message.get("error").cloned().unwrap_or_default();
                Err(refusal(error["code"].as_str().unwrap_or("video.worker"), error["message"].as_str().unwrap_or("the video worker failed")))
            };
            let _ = tx.send(reply);
        }
        // Gone: every caller still waiting hears why, and the slot is cleared for the next one.
        reader.gone.store(true, Ordering::SeqCst);
        let waiting: Vec<_> = lock(&reader.pending).drain().collect();
        for (_, tx) in waiting {
            let _ = tx.send(Err(reader.died()));
        }
        if let Some(engine) = weak.upgrade() {
            let mut live = lock(&engine.video.worker.live);
            if live.as_ref().is_some_and(|w| w.serial == reader.serial) {
                *live = None;
            }
        }
    }).map_err(|e| fail(e.to_string()))?;

    let idle = Arc::downgrade(&worker);
    let _ = std::thread::Builder::new().name("video-worker-idle".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let Some(worker) = idle.upgrade() else { return };
        if worker.gone.load(Ordering::SeqCst) {
            return;
        }
        let busy = !lock(&worker.pending).is_empty() || !lock(&worker.progress).is_empty();
        if !busy && lock(&worker.last_used).elapsed() > IDLE {
            reap(worker);
            return;
        }
    });
    Ok(worker)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// A child that leads a group of its own (as Remotion's detached browser does) is found;
    /// one in its parent's group is not.
    #[test]
    fn a_detached_childs_group_is_found() {
        use std::os::unix::process::CommandExt;
        let mut parent = Command::new("sh").args(["-c", "setsid sleep 30 & sleep 30 & wait"]).process_group(0).spawn().unwrap();
        let pid = parent.id();
        let mut groups = Vec::new();
        for _ in 0..50 {
            groups = child_groups(pid);
            if !groups.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(groups.len(), 1, "{groups:?}");
        // SAFETY: the test's own children.
        unsafe {
            libc::killpg(groups[0], libc::SIGKILL);
            libc::killpg(pid as i32, libc::SIGKILL);
        }
        let _ = parent.wait();
    }
}
