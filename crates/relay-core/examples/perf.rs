//! The performance baseline harness: `cargo run -p relay-core --example perf -- list|run|soak`.
//!
//! Builds a disposable fixture (a real git repository, a store on disk, fake providers, two live
//! PTY sessions, a populated board) and measures every bus op plus the paths that are not ops —
//! store open, engine construction, socket round trips, PTY throughput — with the same
//! instrumentation: wall time per iteration, process CPU time, allocation count and bytes, RSS.
//! Every case starts from the same fixture ([`Fixture::reset`]), and no case reaches a host
//! tool — a browser, `gh`, `adb`, the emulator — only the stand-ins [`isolate_host`] installs.
//!
//! Every measured region runs inside [`perf_measured`]. Under `valgrind --tool=callgrind
//! --instr-atstart=no`, `--callgrind` switches instrumentation on around exactly that call and
//! dumps one profile per scenario, so instructions are attributed to the work and not to the
//! fixture, on every thread the work touches. `--counters` counts the same instructions with the
//! CPU's own counters instead ([`Counters`]), where Valgrind cannot run, and `--marks` records
//! each region's bounds so a `perf record` sampling pass can keep only the samples inside them.
//! `scripts/perf/baseline.py` drives this binary and turns its JSON lines into the report under
//! `docs/perf/`. Nothing here is compiled into the engine; it is an example target, so it builds
//! and lints with `--all-targets` and is otherwise inert.

use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::socket::{Client, SocketServer};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------ counting allocator

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        LIVE_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        LIVE_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

// ------------------------------------------------------------------ process counters

fn cpu_time() -> Duration {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let user = Duration::new(usage.ru_utime.tv_sec as u64, (usage.ru_utime.tv_usec * 1000) as u32);
    let sys = Duration::new(usage.ru_stime.tv_sec as u64, (usage.ru_stime.tv_usec * 1000) as u32);
    user + sys
}

fn rss_kb() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    pages * 4096 / 1024
}

fn thread_count() -> u64 {
    std::fs::read_dir("/proc/self/task").map(|d| d.count() as u64).unwrap_or(0)
}

// ------------------------------------------------------------------ the measured region

/// What one iteration reports back: whether the call succeeded, and what it answered. An op's
/// result is carried out of the measured region whole and sized by [`Outcome::size`] after the
/// counters are read, so serializing it is not counted as the op's (that is
/// `path.response.serialize.*`).
pub struct Outcome {
    pub ok: bool,
    pub code: Option<String>,
    pub result: Option<Value>,
    pub bytes: usize,
    /// A "during" case's other op, and when the measured request was issued.
    pub overlap: Option<Overlap>,
}

impl Outcome {
    fn of(resp: Response) -> Outcome {
        Outcome { ok: resp.ok, code: resp.error.map(|e| e.code), result: resp.result, bytes: 0, overlap: None }
    }
    fn plain(bytes: usize) -> Outcome {
        Outcome { ok: true, code: None, result: None, bytes, overlap: None }
    }
    /// A path that can fail: ok only when the call was, with its error as the code.
    fn res<T, E: std::fmt::Display>(r: Result<T, E>, bytes: impl FnOnce(&T) -> usize) -> Outcome {
        match r {
            Ok(v) => Outcome::plain(bytes(&v)),
            Err(e) => Outcome { ok: false, code: Some(e.to_string()), result: None, bytes: 0, overlap: None },
        }
    }
    fn size(&self) -> usize {
        self.result.as_ref().map_or(self.bytes, |r| r.to_string().len())
    }
}

/// When a "during" case's other op ran, as its own thread saw it.
#[derive(Default)]
pub struct Span {
    start: std::sync::OnceLock<Instant>,
    end: std::sync::OnceLock<Instant>,
}

pub struct Overlap {
    other: Arc<Span>,
    issued: Instant,
}

impl Overlap {
    /// Whether the other op was in flight when the measured request was issued. Read after the
    /// measured region; an end not recorded yet means it is still running.
    fn in_flight(&self) -> bool {
        self.other.start.get().is_some_and(|s| *s <= self.issued) && self.other.end.get().is_none_or(|e| *e > self.issued)
    }
}

/// The one function every measurement runs inside. Never inlined, never mangled, so it is a
/// stable anchor in any profile (`--toggle-collect=perf_measured` also works).
#[no_mangle]
#[inline(never)]
pub fn perf_measured(work: Box<dyn FnOnce() -> Outcome>) -> Outcome {
    work()
}

// ------------------------------------------------------------------ fixture

type Work = Box<dyn FnOnce() -> Outcome>;
/// Runs before every iteration, uncounted, and hands back the work to measure.
type Prepare = Box<dyn Fn(&Arc<Fixture>) -> Work>;

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    origin: PathBuf,
    provider: PathBuf,
    skill_repo: PathBuf,
    store_path: PathBuf,
    engine: Arc<Engine>,
    rt: tokio::runtime::Runtime,
    socket_dir: PathBuf,
    _socket: SocketServer,
    client: Mutex<Client>,
    builder: String,
    reviewer: String,
    task_id: i64,
    parent_task: i64,
    child_task: i64,
    module_id: i64,
    note_id: i64,
    skill_id: i64,
    dirty_file: String,
    clean_worktree: String,
    head_sha: String,
    counter: AtomicUsize,
    scale: usize,
    /// Calls that undo what a measured region created — a session to close, a task to delete —
    /// drained, uncounted, before the next iteration and after the loop, so the store a case
    /// sees is the fixture and not the residue of every case before it.
    cleanup: Mutex<Vec<(Actor, &'static str, Value)>>,
    /// Threads a measured region started and must not wait for inside the measurement.
    joins: Mutex<Vec<std::thread::JoinHandle<()>>>,
    /// Engines a measured region built whose background work (recovery's deferred dirty scan)
    /// still holds a clone: waited for and dropped before the next iteration, so one
    /// iteration's `git status` workers do not run under the next one's counters.
    settle: Mutex<Vec<Arc<Engine>>>,
    /// The store and the checkouts as `populate` left them; every case starts from this.
    baseline: Baseline,
    /// One line per call the engine made to a host tool the fixture stands in for.
    host_log: PathBuf,
    /// The design rows' own engine and designs, built the first time one of them runs.
    designs: std::sync::OnceLock<Designs>,
}

/// What [`Fixture::reset`] puts back between cases, so a case measures the fixture and not the
/// rows, commits, branches and worktrees every case before it left behind — and so a number
/// at `--scale 10` is compared with one taken on the same store at scale 1, not on a store a
/// few hundred cases have used.
#[derive(Default)]
struct Baseline {
    store: PathBuf,
    refs: Vec<String>,
    origin_refs: Vec<String>,
    worktrees: Vec<String>,
    untracked: Vec<String>,
    readme: String,
}

/// Fixture git. It reads no global or system config: [`isolate_host`] has pointed
/// `GIT_CONFIG_GLOBAL` at the fixture's own file and set `GIT_CONFIG_NOSYSTEM` before the first
/// call, and every `git` this process or the engine forks inherits both.
fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn git_out(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().expect("git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn lines(repo: &Path, args: &[&str]) -> Vec<String> {
    git_out(repo, args).lines().map(str::to_string).filter(|l| !l.is_empty()).collect()
}

/// The host tools an op can reach. On a developer machine they are real: `xdg-open` opens a
/// browser tab per iteration, `gh auth login --web` starts a login, `gh api` spends the
/// person's token, `adb` starts a server. Each is replaced by a stand-in that logs its argv and
/// refuses, found first on PATH or through the settings the engine reads, so every case that
/// reaches one measures Relay's refusal path and nothing leaves the fixture.
const HOST_TOOLS: &[&str] = &["xdg-open", "gh", "adb", "emulator", "avdmanager", "sdkmanager", "keytool", "secret-tool", "apksigner", "jarsigner"];

/// Install the stand-ins and point the process at them. Runs before the engine, the runtime or
/// any other thread exists, which is what makes changing the environment sound.
fn isolate_host(root: &Path) -> PathBuf {
    let bin = root.join("host-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = root.join("host-tools.log");
    for tool in HOST_TOOLS {
        write_exec(&bin.join(tool), &format!(
            "#!/bin/sh\nprintf '%s\\n' \"{tool} $*\" >> '{}'\nexit 1\n",
            log.display()));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![bin];
    dirs.extend(std::env::split_paths(&path));
    std::env::set_var("PATH", std::env::join_paths(dirs).unwrap());
    // The Android SDK, the JDK and the person's provider configuration are found through these.
    for var in ["ANDROID_SDK_ROOT", "ANDROID_HOME", "JAVA_HOME"] {
        std::env::remove_var(var);
    }
    let home = root.join("host-home");
    for (var, dir) in [("ANDROID_AVD_HOME", "avd"), ("CLAUDE_CONFIG_DIR", ".claude"), ("CODEX_HOME", ".codex"), ("GH_CONFIG_DIR", "gh")] {
        std::fs::create_dir_all(home.join(dir)).unwrap();
        std::env::set_var(var, home.join(dir));
    }
    // git reads no global or system config: no credential helper, hooks path or signing key
    // of the person's runs against the fixture's repositories. No background `gc --auto`
    // either, which would pack the objects a clone of the fixture is copying.
    let gitconfig = home.join("gitconfig");
    std::fs::write(&gitconfig, "[user]\n\tname = perf\n\temail = perf@relay.test\n[credential]\n\thelper =\n[commit]\n\tgpgsign = false\n[gc]\n\tauto = 0\n[maintenance]\n\tauto = false\n").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &gitconfig);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    std::env::set_var("GIT_TERMINAL_PROMPT", "0");
    log
}

/// Calls to each stand-in host tool so far.
fn host_calls(log: &Path) -> std::collections::BTreeMap<String, u64> {
    let mut calls = std::collections::BTreeMap::new();
    for line in std::fs::read_to_string(log).unwrap_or_default().lines() {
        *calls.entry(line.split(' ').next().unwrap_or_default().to_string()).or_default() += 1;
    }
    calls
}

/// A repository shaped like a small application: `dirs × files` source files across nested
/// directories, three commits, one dirty file, one large file, a bare origin beside it.
fn build_repo(root: &Path, dirs: usize, files: usize) -> (PathBuf, PathBuf, String) {
    let repo = root.join("ws").join("app");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "perf@relay.test"]);
    git(&repo, &["config", "user.name", "perf"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    for d in 0..dirs {
        let dir = repo.join("src").join(format!("module_{d:02}")).join("inner");
        std::fs::create_dir_all(&dir).unwrap();
        for f in 0..files {
            let ext = ["rs", "ts", "kt", "md"][f % 4];
            let mut body = String::new();
            for line in 0..(20 + (f * 7) % 60) {
                body.push_str(&format!("pub fn item_{d}_{f}_{line}(x: u32) -> u32 {{ x + {line} }} // filler text line\n"));
            }
            std::fs::write(dir.join(format!("file_{f:03}.{ext}")), body).unwrap();
        }
    }
    std::fs::write(repo.join("README.md"), "# perf fixture\n\nDisposable.\n").unwrap();
    std::fs::write(repo.join("Cargo.toml"), "[package]\nname = \"fixture\"\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "fixture: initial tree"]);
    for round in 0..2 {
        std::fs::write(repo.join(format!("CHANGES-{round}.md")), format!("round {round}\n")).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", &format!("fixture: round {round}")]);
    }
    // The one-mebibyte file the editor bound is set at.
    let big: String = (0..16384).map(|i| format!("line {i:05} of the large fixture file padded to sixty-four bytes\n")).collect();
    std::fs::write(repo.join("large.txt"), &big).unwrap();
    git(&repo, &["add", "large.txt"]);
    git(&repo, &["commit", "-qm", "fixture: large file"]);
    let head = git_out(&repo, &["rev-parse", "HEAD"]);
    let origin = root.join("origin.git");
    git(root, &["clone", "-q", "--bare", repo.to_str().unwrap(), origin.to_str().unwrap()]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["fetch", "-q", "origin"]);
    git(&repo, &["branch", "--set-upstream-to=origin/main", "main"]);
    // One dirty tracked file, so status/diff/suggest_message have something to look at.
    std::fs::write(repo.join("README.md"), "# perf fixture\n\nDisposable. Edited after the last commit.\n").unwrap();
    (repo, origin, head)
}

const PROVIDER_SH: &str = r#"#!/bin/sh
case "${1:-}" in
  --version) echo 'perf-fixture 1.0'; exit 0;;
  auth|login) echo '{"loggedIn":true,"email":"perf@relay.test"}'; exit 0;;
esac
echo "hello-from-pty $RELAY_SESSION"
while IFS= read -r line; do
  case "$line" in
    burst*) n=${line#burst }; i=0
      while [ "$i" -lt "$n" ]; do
        echo "B$i xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
        i=$((i+1))
      done
      echo "BURST-END";;
    tick*) n=${line#tick }; i=0
      while [ "$i" -lt "$n" ]; do echo "tick $i"; sleep 0.1; i=$((i+1)); done;;
    *) echo "echo: $line";;
  esac
done
"#;

impl Fixture {
    fn build(scale: usize, keep: bool) -> Arc<Fixture> {
        let tmp = tempfile::Builder::new().prefix("relay-perf-").tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        if keep {
            eprintln!("fixture at {}", root.display());
        }
        let host_log = isolate_host(&root);
        let (repo, origin, head_sha) = build_repo(&root, 30, 50);
        let provider = root.join("provider.sh");
        write_exec(&provider, PROVIDER_SH);
        let skill_repo = root.join("skill-repo");
        std::fs::create_dir_all(&skill_repo).unwrap();
        std::fs::write(skill_repo.join("SKILL.md"), "---\nname: perf-skill\ndescription: fixture\n---\n\nA fixture skill body.\n").unwrap();
        git(&skill_repo, &["init", "-q", "-b", "main"]);
        git(&skill_repo, &["config", "user.email", "perf@relay.test"]);
        git(&skill_repo, &["config", "user.name", "perf"]);
        git(&skill_repo, &["add", "."]);
        git(&skill_repo, &["commit", "-qm", "skill"]);
        let store_path = root.join("store").join("store.db");
        let store = Store::open(&store_path, false).unwrap();
        let engine = Engine::new(Instance::Test, store);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let socket_dir = root.join("run");
        let socket = rt.block_on(SocketServer::start_in(engine.clone(), socket_dir.clone())).unwrap();
        let client = rt.block_on(Client::connect(&socket.path)).unwrap();

        let mut fx = Fixture {
            _tmp: tmp,
            root: root.clone(),
            repo: repo.clone(),
            origin,
            provider: provider.clone(),
            skill_repo,
            store_path,
            engine,
            rt,
            socket_dir,
            _socket: socket,
            client: Mutex::new(client),
            builder: String::new(),
            reviewer: String::new(),
            task_id: 0,
            parent_task: 0,
            child_task: 0,
            module_id: 0,
            note_id: 0,
            skill_id: 0,
            dirty_file: "README.md".into(),
            clean_worktree: String::new(),
            head_sha,
            counter: AtomicUsize::new(0),
            scale,
            cleanup: Mutex::new(Vec::new()),
            joins: Mutex::new(Vec::new()),
            settle: Mutex::new(Vec::new()),
            baseline: Baseline::default(),
            host_log,
            designs: std::sync::OnceLock::new(),
        };
        fx.populate();
        fx.baseline = fx.snapshot();
        // The temp dir must outlive `keep`; leaking it is the point.
        if keep {
            let path = std::mem::replace(&mut fx._tmp, tempfile::tempdir().unwrap());
            std::mem::forget(path);
        }
        Arc::new(fx)
    }

    fn call(&self, actor: Actor, op: &str, payload: Value) -> Response {
        self.engine.dispatch(Request::new(actor, op, payload), Door::InProcess)
    }
    fn user(&self, op: &str, payload: Value) -> Value {
        let r = self.call(Actor::User, op, payload);
        match r.into_result() {
            Ok(v) => v,
            Err(e) => panic!("fixture: {op} failed: {} {}", e.code, e.message),
        }
    }
    fn agent(&self) -> Actor {
        Actor::agent(self.builder.clone())
    }
    fn designs(&self) -> &Designs {
        self.designs.get_or_init(|| Designs::build(&self.root))
    }
    fn next(&self) -> usize {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }
    fn id(v: &Value) -> i64 {
        v["id"].as_i64().unwrap_or_else(|| panic!("no id in {v}"))
    }

    fn later(&self, actor: Actor, op: &'static str, payload: Value) {
        self.cleanup.lock().unwrap().push((actor, op, payload));
    }
    fn close_later(&self, session: String) {
        self.later(Actor::User, "session.close", json!({"session": session, "remove_worktree": true}));
    }
    fn join_later(&self, handle: std::thread::JoinHandle<()>) {
        self.joins.lock().unwrap().push(handle);
    }
    fn settle_later(&self, engine: Arc<Engine>) {
        self.settle.lock().unwrap().push(engine);
    }
    fn drain_cleanup(&self) {
        let joins: Vec<std::thread::JoinHandle<()>> = std::mem::take(&mut *self.joins.lock().unwrap());
        for handle in joins {
            let _ = handle.join();
        }
        let engines: Vec<Arc<Engine>> = std::mem::take(&mut *self.settle.lock().unwrap());
        for engine in engines {
            let deadline = Instant::now() + Duration::from_secs(60);
            while Arc::strong_count(&engine) > 1 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if Arc::strong_count(&engine) > 1 {
                eprint!("(an engine's background work outlived 60 s) ");
            }
        }
        let pending: Vec<(Actor, &'static str, Value)> = std::mem::take(&mut *self.cleanup.lock().unwrap());
        for (actor, op, payload) in pending {
            let _ = self.call(actor, op, payload);
        }
        if let Some(designs) = self.designs.get() {
            designs.drain();
        }
    }
    fn snapshot(&self) -> Baseline {
        let store = self.root.join("baseline.db");
        self.engine.store.lock().backup(rusqlite::MAIN_DB, &store, None).unwrap();
        Baseline {
            store,
            refs: lines(&self.repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
            origin_refs: lines(&self.origin, &["for-each-ref", "--format=%(refname) %(objectname)"]),
            worktrees: self.worktrees(),
            untracked: lines(&self.repo, &["ls-files", "--others", "--exclude-standard", "--directory"]),
            readme: std::fs::read_to_string(self.repo.join("README.md")).unwrap(),
        }
    }

    fn worktrees(&self) -> Vec<String> {
        lines(&self.repo, &["worktree", "list", "--porcelain"]).into_iter()
            .filter_map(|l| l.strip_prefix("worktree ").map(str::to_string)).collect()
    }

    /// Put the store and the repositories back to [`Baseline`], uncounted, between cases. The
    /// live PTYs are the fixture's own two sessions, whose rows the snapshot holds unchanged;
    /// everything a case opened has been closed by its cleanup before this runs.
    fn reset(&self) {
        self.drain_cleanup();
        if let Some(designs) = self.designs.get() {
            designs.reset();
        }
        let base = &self.baseline;
        self.engine.store.lock().restore(rusqlite::MAIN_DB, &base.store, None::<fn(rusqlite::backup::Progress)>).unwrap();
        // Worktrees a case left (a kept session checkout, an integration) before the branches
        // they hold, then every ref back to where it was.
        for path in self.worktrees().into_iter().filter(|w| !base.worktrees.contains(w)) {
            let _ = Command::new("git").arg("-C").arg(&self.repo).args(["worktree", "remove", "--force", &path]).output();
            let _ = std::fs::remove_dir_all(&path);
        }
        let _ = Command::new("git").arg("-C").arg(&self.repo).args(["worktree", "prune"]).output();
        let _ = Command::new("git").arg("-C").arg(&self.clean_worktree).args(["checkout", "-q", "-f", "relay/perf-clean"]).output();
        for (repo, refs) in [(&self.repo, &base.refs), (&self.origin, &base.origin_refs)] {
            for line in lines(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]) {
                let name = line.split(' ').next().unwrap_or_default();
                if !refs.iter().any(|r| r.split(' ').next() == Some(name)) {
                    let _ = Command::new("git").arg("-C").arg(repo).args(["update-ref", "-d", name]).output();
                }
            }
            for line in refs {
                if let Some((name, sha)) = line.split_once(' ') {
                    let _ = Command::new("git").arg("-C").arg(repo).args(["update-ref", name, sha]).output();
                }
            }
        }
        git(&self.repo, &["reset", "-q", "--hard"]);
        for path in lines(&self.repo, &["ls-files", "--others", "--exclude-standard", "--directory"]) {
            if !base.untracked.contains(&path) {
                let path = self.repo.join(&path);
                let _ = std::fs::remove_dir_all(&path).or_else(|_| std::fs::remove_file(&path));
            }
        }
        std::fs::write(self.repo.join("README.md"), &base.readme).unwrap();
    }

    /// Live row counts, so a number can be read against the store it was measured on.
    fn rows(&self) -> Value {
        let conn = self.engine.store.lock();
        let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(-1) };
        json!({
            "tasks": count("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL"),
            "sessions_open": count("SELECT COUNT(*) FROM sessions WHERE state!='closed'"),
            "audit": count("SELECT COUNT(*) FROM audit"),
            "messages": count("SELECT COUNT(*) FROM messages"),
            "notes": count("SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL"),
            "holds_open": count("SELECT COUNT(*) FROM holds WHERE state='open'"),
            "sessions_closed": count("SELECT COUNT(*) FROM sessions WHERE state='closed'"),
            "notifications": count("SELECT COUNT(*) FROM notifications"),
        })
    }

    fn wait_scrollback(&self, session: &str, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let text = self.user("session.scrollback", json!({"session": session, "lines": 5}));
            if text["text"].as_str().unwrap_or("").contains(needle) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {needle:?} in {session}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn populate(&mut self) {
        let ws = self.user("workspace.create", json!({"path": self.root.join("ws")}));
        let project = self.user("project.add", json!({"workspace_id": Fixture::id(&ws), "path": self.repo}));
        assert_eq!(Fixture::id(&project), 1);
        for name in ["claude", "codex"] {
            self.user("settings.set", json!({"path": format!("providers.{name}.path"), "value": self.provider}));
        }
        self.user("provider.refresh", json!({}));
        // The Android tools by setting, so neither PATH nor a real SDK under ~/Android is
        // consulted: adb, the emulator and avdmanager are the stand-ins from `isolate_host`.
        let bin = self.root.join("host-bin");
        let sdk = self.root.join("host-home").join("android-sdk");
        std::fs::create_dir_all(&sdk).unwrap();
        for (path, value) in [("device.adb_path", bin.join("adb")), ("device.emulator_path", bin.join("emulator")),
                              ("device.avdmanager_path", bin.join("avdmanager")), ("device.sdk_path", sdk)] {
            self.user("settings.set", json!({"path": path, "value": value}));
        }
        self.user("guardrail.config.set", json!({"project_id": 1, "patch": {"protected_paths": ["secret/*"]}}));
        for m in 0..5 {
            let module = self.user("module.create", json!({"project_id": 1, "name": format!("Module {m}"), "priority": "high"}));
            if m == 0 {
                self.module_id = Fixture::id(&module);
            }
        }
        let columns = ["backlog", "ready", "active", "in_review", "done"];
        for t in 0..(40 * self.scale) {
            let priority = ["low", "medium", "high", "urgent"][t % 4];
            let kind = ["task", "feature", "bug", "chore", "spike"][t % 5];
            let column = columns[t % 5];
            let task = self.user("task.create", json!({
                "project_id": 1, "title": format!("Task {t}: a title of ordinary length"),
                "body": "A body paragraph with enough words to look like a real task description, repeated once. A body paragraph with enough words to look like a real task description.",
                "column": column, "priority": priority,
                "module_id": self.module_id, "labels": [format!("area-{}", t % 7), "perf"],
                "type": kind,
            }));
            if t == 0 {
                self.task_id = Fixture::id(&task);
                self.user("task.changelog.write", json!({"task_id": self.task_id, "text": "changelog line"}));
            }
            if t == 1 {
                self.parent_task = Fixture::id(&task);
            }
            if t == 2 {
                self.child_task = Fixture::id(&task);
                self.user("task.parent.set", json!({"task_id": self.child_task, "parent_id": self.parent_task}));
            }
        }
        for n in 0..(10 * self.scale) {
            let note = self.user("notes.create", json!({"project_id": 1, "title": format!("Note {n}"), "body": "A note body.\n\nWith two paragraphs.", "pinned": n % 3 == 0}));
            if n == 0 {
                self.note_id = Fixture::id(&note);
            }
        }
        for s in 0..3 {
            let skill = self.user("skill.create", json!({"name": format!("perf-skill-{s}"), "body": "---\nname: x\n---\nbody"}));
            if s == 0 {
                self.skill_id = Fixture::id(&skill);
                self.user("skill.enable", json!({"skill_id": self.skill_id, "project_id": 1, "enabled": true}));
            }
        }
        self.user("ui.layout.save", json!({"project_id": 1, "name": "perf", "state": {"agent_layout": "grid", "panes": [1, 2, 3]}}));
        let clean = self.user("worktree.create", json!({"project_id": 1, "branch": "relay/perf-clean"}));
        self.clean_worktree = clean["path"].as_str().unwrap().to_string();
        let builder = self.user("session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}));
        self.builder = builder["name"].as_str().unwrap().to_string();
        let reviewer = self.user("session.create", json!({"project_id": 1, "provider": "codex", "role": "reviewer", "pair_with": self.builder}));
        self.reviewer = reviewer["name"].as_str().unwrap().to_string();
        self.user("task.dispatch", json!({"task_id": self.task_id, "session": self.builder, "start": false}));
        for name in [self.builder.clone(), self.reviewer.clone()] {
            self.user("session.spawn", json!({"session": name}));
        }
        for name in [self.builder.clone(), self.reviewer.clone()] {
            self.wait_scrollback(&name, "hello-from-pty");
        }
        for m in 0..(10 * self.scale) {
            self.user("mailbox.send", json!({"project_id": 1, "to": self.reviewer, "text": format!("message {m} about the review"), "priority": m % 4 == 0}));
        }
        self.call(self.agent(), "session.intent", json!({"session": self.builder, "text": "measuring"}));
        self.call(self.agent(), "session.claim", json!({"paths": ["src/module_00/inner/file_000.rs"]}));
        // One open hold and one refusal, so hold listing has a row and notifications exist.
        self.call(self.agent(), "guardrail.gate", json!({"session": self.builder, "kind": "write", "path": "secret/key", "new_text": "x"}));
        self.call(Actor::User, "guardrail.gate", json!({"session": self.builder, "kind": "write", "path": "secret/key", "new_text": "x"}));
        // Some audit history beyond what populate wrote: the board has been used.
        for i in 0..(50 * self.scale) {
            self.user("settings.set", json!({"path": "perf.warm", "value": i}));
        }
    }
}

// ------------------------------------------------------------------ cases

#[derive(Clone, Copy)]
enum Cost {
    Cheap,
    Mid,
    Heavy,
}

impl Cost {
    fn iters(self) -> usize {
        match self {
            Cost::Cheap => 200,
            Cost::Mid => 30,
            Cost::Heavy => 6,
        }
    }
}

struct Case {
    name: String,
    op: Option<String>,
    cost: Cost,
    prepare: Prepare,
}

fn dispatch(fx: &Arc<Fixture>, actor: Actor, op: &str, payload: Value) -> Work {
    let engine = fx.engine.clone();
    let req = Request::new(actor, op, payload);
    Box::new(move || Outcome::of(engine.dispatch(req, Door::InProcess)))
}

/// A fixed payload.
fn fixed(cases: &mut Vec<Case>, op: &str, actor: Actor, cost: Cost, payload: Value) {
    let op = op.to_string();
    let op2 = op.clone();
    cases.push(Case {
        name: format!("op.{op}"),
        op: Some(op),
        cost,
        prepare: Box::new(move |fx| dispatch(fx, actor.clone(), &op2, payload.clone())),
    });
}

/// A fixed payload under a variant name (`task.list.filtered` is still `task.list`).
fn fixed_as(cases: &mut Vec<Case>, name: &str, op: &str, actor: Actor, cost: Cost, payload: Value) {
    let op = op.to_string();
    cases.push(Case {
        name: format!("op.{name}"),
        op: Some(op.clone()),
        cost,
        prepare: Box::new(move |fx| dispatch(fx, actor.clone(), &op, payload.clone())),
    });
}

/// A mutation that creates a row: measured as itself, then undone outside the measured region
/// with `undo(result)`, so two hundred iterations do not leave two hundred rows behind.
fn created(cases: &mut Vec<Case>, name: &str, op: &'static str, actor: Actor, cost: Cost, payload: impl Fn(&Arc<Fixture>) -> Value + 'static, undo: fn(&Value) -> (&'static str, Value)) {
    cases.push(Case {
        name: format!("op.{name}"),
        op: Some(op.to_string()),
        cost,
        prepare: Box::new(move |fx| {
            let fx = fx.clone();
            let req = Request::new(actor.clone(), op, payload(&fx));
            Box::new(move || {
                let resp = fx.engine.dispatch(req, Door::InProcess);
                if let Some(result) = resp.result.as_ref().filter(|_| resp.ok) {
                    let (op, payload) = undo(result);
                    fx.later(Actor::User, op, payload);
                }
                Outcome::of(resp)
            })
        }),
    });
}

/// A payload built per iteration, from the fixture (fresh names, fresh rows).
fn per(cases: &mut Vec<Case>, name: &str, op: &str, actor: Actor, cost: Cost, payload: impl Fn(&Arc<Fixture>) -> Value + 'static) {
    let op = op.to_string();
    cases.push(Case {
        name: format!("op.{name}"),
        op: Some(op.clone()),
        cost,
        prepare: Box::new(move |fx| dispatch(fx, actor.clone(), &op, payload(fx))),
    });
}

/// Same, with the payload built as the builder agent.
fn per_agent(cases: &mut Vec<Case>, name: &str, op: &str, cost: Cost, payload: impl Fn(&Arc<Fixture>) -> Value + 'static) {
    let op = op.to_string();
    cases.push(Case {
        name: format!("op.{name}"),
        op: Some(op.clone()),
        cost,
        prepare: Box::new(move |fx| dispatch(fx, fx.agent(), &op, payload(fx))),
    });
}

fn path_case(cases: &mut Vec<Case>, name: &str, cost: Cost, prepare: impl Fn(&Arc<Fixture>) -> Work + 'static) {
    cases.push(Case { name: format!("path.{name}"), op: None, cost, prepare: Box::new(prepare) });
}

fn fresh_task(fx: &Fixture) -> i64 {
    let n = fx.next();
    let id = Fixture::id(&fx.user("task.create", json!({"project_id": 1, "title": format!("scratch {n}"), "body": "scratch", "column": "ready"})));
    fx.later(Actor::User, "task.delete", json!({"task_id": id}));
    id
}

/// A session the case will use up; it is closed, uncounted, before the next iteration, so the
/// fixture does not grow a pool of abandoned worktrees that later cases would pay for.
fn fresh_session(fx: &Fixture) -> String {
    let name = fx.user("session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}))["name"].as_str().unwrap().to_string();
    fx.close_later(name.clone());
    name
}

fn spawned_session(fx: &Fixture) -> String {
    let name = fresh_session(fx);
    fx.user("session.spawn", json!({"session": name}));
    fx.wait_scrollback(&name, "hello-from-pty");
    name
}

/// A user write to a protected path is held (an agent's is refused outright); the hold id
/// rides in the error's confirmation payload. The hold is rejected before the next iteration
/// unless the case itself resolves it.
fn last_hold(fx: &Fixture) -> i64 {
    let r = fx.call(Actor::User, "guardrail.gate", json!({"session": fx.builder, "kind": "write", "path": format!("secret/key-{}", fx.next()), "new_text": "x"}));
    let id = r.error.and_then(|e| e.confirm).and_then(|c| c.payload["hold_id"].as_i64()).unwrap_or(0);
    fx.later(Actor::User, "guardrail.reject", json!({"hold_id": id, "reason": "perf"}));
    id
}

/// A backup of the live store with the sessions' pids forgotten: crash recovery reaps any live
/// pid it finds in the store, and the fixture's own sessions must survive being measured.
fn store_copy(fx: &Fixture) -> PathBuf {
    let path = fx.engine.store.backup("perf").unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE sessions SET pid=NULL", []).unwrap();
    path
}

fn kib(n: usize) -> String {
    let line = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n";
    line.repeat(n * 1024 / line.len())
}

/// Start `other` on its own thread and return once it has begun and `lead` has passed: what a
/// "during" case issues its request beside. The span records when `other` actually ran.
fn beside(lead: Duration, other: impl FnOnce() + Send + 'static) -> (Arc<Span>, std::thread::JoinHandle<()>) {
    let span = Arc::new(Span::default());
    let ran = span.clone();
    let handle = std::thread::spawn(move || {
        let _ = ran.start.set(Instant::now());
        other();
        let _ = ran.end.set(Instant::now());
    });
    while span.start.get().is_none() {
        std::thread::yield_now();
    }
    std::thread::sleep(lead);
    (span, handle)
}

fn cases() -> Vec<Case> {
    use Actor::User as U;
    use Cost::*;
    let mut c = Vec::new();
    let one = |fx: &Arc<Fixture>| fx.next();

    // ---- bus / app
    fixed(&mut c, "bus.ping", U, Cheap, json!({}));
    fixed(&mut c, "bus.schema", U, Cheap, json!({"op": "task.create"}));
    per(&mut c, "bus.schema.whole", "bus.schema", U, Mid, |_| json!({}));
    fixed(&mut c, "bus.ops", U, Cheap, json!({}));
    per_agent(&mut c, "bus.whoami", "bus.whoami", Cheap, |_| json!({}));
    fixed(&mut c, "app.version", U, Cheap, json!({}));
    fixed(&mut c, "app.status", U, Cheap, json!({}));
    fixed(&mut c, "app.resources.get", U, Cheap, json!({}));
    per(&mut c, "app.resources.watch", "app.resources.watch", U, Mid, |fx| {
        fx.user("app.resources.watch", json!({"on": false}));
        json!({"on": true})
    });
    fixed(&mut c, "app.recovery.last", U, Cheap, json!({}));
    fixed(&mut c, "app.log.tail", U, Cheap, json!({}));
    fixed(&mut c, "app.backup.now", U, Heavy, json!({}));
    fixed(&mut c, "app.backup.list", U, Mid, json!({}));
    per(&mut c, "app.import.v3", "app.import.v3", U, Cheap, |fx| json!({"source": fx.root.join("no-such-v3"), "project_id": 1, "dry_run": true}));
    fixed(&mut c, "app.first_run.state", U, Cheap, json!({}));
    fixed(&mut c, "app.reconcile", U, Mid, json!({}));

    // ---- audit
    fixed(&mut c, "audit.list", U, Cheap, json!({"limit": 100}));
    per(&mut c, "audit.get", "audit.get", U, Cheap, |fx| {
        let rows = fx.user("audit.list", json!({"limit": 1}));
        json!({"audit_id": Fixture::id(&rows["rows"][0])})
    });
    per(&mut c, "audit.undo", "audit.undo", U, Mid, |fx| {
        fx.user("task.update", json!({"task_id": fx.task_id, "title": format!("undo me {}", fx.next())}));
        let rows = fx.user("audit.list", json!({"op_prefix": "task.update", "limit": 1}));
        json!({"audit_id": Fixture::id(&rows["rows"][0])})
    });

    // ---- workspace / project
    created(&mut c, "workspace.create", "workspace.create", U, Mid, move |fx| {
        let dir = fx.root.join(format!("ws-{}", one(fx)));
        std::fs::create_dir_all(&dir).unwrap();
        json!({"path": dir})
    }, |r| ("workspace.remove", json!({"workspace_id": Fixture::id(r)})));
    per(&mut c, "workspace.discover", "workspace.discover", U, Mid, |fx| json!({"path": fx.root.join("ws")}));
    fixed(&mut c, "workspace.list", U, Cheap, json!({}));
    fixed(&mut c, "workspace.update", U, Cheap, json!({"workspace_id": 1, "name": "perf"}));
    fixed(&mut c, "workspace.reorder", U, Cheap, json!({"orders": [{"workspace_id": 1, "order": 0}]}));
    per(&mut c, "workspace.remove", "workspace.remove", U, Mid, move |fx| {
        let dir = fx.root.join(format!("ws-rm-{}", one(fx)));
        std::fs::create_dir_all(&dir).unwrap();
        json!({"workspace_id": Fixture::id(&fx.user("workspace.create", json!({"path": dir})))})
    });
    let small_repo = move |fx: &Arc<Fixture>| -> PathBuf {
        let dir = fx.root.join("ws").join(format!("small-{}", one(fx)));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.email", "perf@relay.test"]);
        git(&dir, &["config", "user.name", "perf"]);
        std::fs::write(dir.join("README.md"), "x\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-qm", "init"]);
        dir
    };
    created(&mut c, "project.add", "project.add", U, Mid, move |fx| json!({"workspace_id": 1, "path": small_repo(fx)}), |r| ("project.remove", json!({"project_id": Fixture::id(r)})));
    created(&mut c, "project.clone", "project.clone", U, Heavy, move |fx| json!({"workspace_id": 1, "url": fx.origin, "dest": format!("clone-{}", one(fx))}), |r| ("project.remove", json!({"project_id": r["project"]["id"].as_i64().or_else(|| r["id"].as_i64()).unwrap_or(0)})));
    fixed(&mut c, "project.list", U, Cheap, json!({}));
    fixed(&mut c, "project.get", U, Cheap, json!({"project_id": 1}));
    fixed(&mut c, "project.update", U, Cheap, json!({"project_id": 1, "name": "app"}));
    fixed(&mut c, "project.reorder", U, Cheap, json!({"orders": [{"project_id": 1, "order": 0}]}));
    per(&mut c, "project.remove", "project.remove", U, Mid, move |fx| {
        json!({"project_id": Fixture::id(&fx.user("project.add", json!({"workspace_id": 1, "path": small_repo(fx)})))})
    });
    fixed(&mut c, "project.stats", U, Cheap, json!({"project_id": 1}));

    // ---- task
    created(&mut c, "task.create", "task.create", U, Cheap, move |fx| json!({"project_id": 1, "title": format!("created {}", one(fx)), "body": "body text", "column": "ready", "labels": ["perf"]}), |r| ("task.delete", json!({"task_id": Fixture::id(r)})));
    per(&mut c, "task.get", "task.get", U, Cheap, |fx| json!({"task_id": fx.task_id}));
    per(&mut c, "task.activity", "task.activity", U, Cheap, |fx| json!({"task_id": fx.task_id}));
    fixed(&mut c, "task.list", U, Cheap, json!({"project_id": 1}));
    fixed_as(&mut c, "task.list.filtered", "task.list", U, Cheap, json!({"project_id": 1, "column": "ready", "label": "perf", "sort": "priority"}));
    per(&mut c, "task.update", "task.update", U, Cheap, move |fx| json!({"task_id": fx.task_id, "body": format!("updated body {}", one(fx))}));
    per(&mut c, "task.move", "task.move", U, Cheap, move |fx| json!({"task_id": fx.task_id, "column": if one(fx) % 2 == 0 { "active" } else { "ready" }, "position": 0}));
    per(&mut c, "task.delete", "task.delete", U, Cheap, |fx| json!({"task_id": fresh_task(fx)}));
    per(&mut c, "task.restore", "task.restore", U, Cheap, |fx| {
        let id = fresh_task(fx);
        fx.user("task.delete", json!({"task_id": id}));
        json!({"task_id": id})
    });
    per(&mut c, "task.link_commit", "task.link_commit", U, Cheap, |fx| json!({"task_id": fresh_task(fx), "sha": fx.head_sha, "branch": "main"}));
    per(&mut c, "task.changelog.write", "task.changelog.write", U, Cheap, |fx| json!({"task_id": fresh_task(fx), "text": "did a thing"}));
    per(&mut c, "task.attach", "task.attach", U, Cheap, move |fx| json!({"task_id": fresh_task(fx), "name": format!("a-{}.txt", one(fx)), "mime": "text/plain", "bytes_b64": "aGVsbG8gd29ybGQK".repeat(256)}));
    per(&mut c, "task.detach", "task.detach", U, Cheap, move |fx| {
        let task = fresh_task(fx);
        let a = fx.user("task.attach", json!({"task_id": task, "name": format!("d-{}.txt", one(fx)), "mime": "text/plain", "bytes_b64": "aGVsbG8K"}));
        json!({"task_id": task, "attachment_id": a["id"].as_i64().or_else(|| a["attachment"]["id"].as_i64()).unwrap_or(0)})
    });
    per(&mut c, "task.parent.set", "task.parent.set", U, Cheap, |fx| json!({"task_id": fresh_task(fx), "parent_id": fresh_task(fx)}));
    per(&mut c, "task.children", "task.children", U, Cheap, |fx| json!({"task_id": fx.parent_task, "recursive": true}));
    per(&mut c, "task.label.add", "task.label.add", U, Cheap, move |fx| json!({"task_id": fresh_task(fx), "label": format!("l{}", one(fx) % 5)}));
    per(&mut c, "task.label.remove", "task.label.remove", U, Cheap, move |fx| {
        let task = fresh_task(fx);
        let label = format!("r{}", one(fx) % 5);
        fx.user("task.label.add", json!({"task_id": task, "label": label}));
        json!({"task_id": task, "label": label})
    });
    fixed(&mut c, "task.label.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "task.relate", "task.relate", U, Cheap, |fx| json!({"task_id": fresh_task(fx), "relation": "blocked_by", "other_id": fresh_task(fx)}));
    per(&mut c, "task.unrelate", "task.unrelate", U, Cheap, |fx| {
        let (a, b) = (fresh_task(fx), fresh_task(fx));
        fx.user("task.relate", json!({"task_id": a, "relation": "blocked_by", "other_id": b}));
        json!({"task_id": a, "relation": "blocked_by", "other_id": b})
    });
    per(&mut c, "task.dispatch", "task.dispatch", U, Mid, |fx| json!({"task_id": fresh_task(fx), "session": fx.reviewer, "start": false}));
    per(&mut c, "task.approve", "task.approve", U, Cheap, |fx| json!({"task_id": fx.task_id, "sha": fx.head_sha}));
    per(&mut c, "task.copy_text", "task.copy_text", U, Cheap, |fx| json!({"task_id": fx.task_id}));

    // ---- module
    created(&mut c, "module.create", "module.create", U, Cheap, move |fx| json!({"project_id": 1, "name": format!("m{}", one(fx))}), |r| ("module.delete", json!({"module_id": Fixture::id(r)})));
    per(&mut c, "module.get", "module.get", U, Cheap, |fx| json!({"module_id": fx.module_id}));
    fixed(&mut c, "module.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "module.update", "module.update", U, Cheap, |fx| json!({"module_id": fx.module_id, "name": "Module 0"}));
    let fresh_module = move |fx: &Arc<Fixture>| {
        let id = Fixture::id(&fx.user("module.create", json!({"project_id": 1, "name": format!("fm{}", one(fx))})));
        fx.later(U, "module.delete", json!({"module_id": id}));
        id
    };
    per(&mut c, "module.complete", "module.complete", U, Cheap, move |fx| json!({"module_id": fresh_module(fx)}));
    per(&mut c, "module.reopen", "module.reopen", U, Cheap, move |fx| {
        let id = fresh_module(fx);
        fx.user("module.complete", json!({"module_id": id}));
        json!({"module_id": id})
    });
    per(&mut c, "module.delete", "module.delete", U, Cheap, move |fx| json!({"module_id": fresh_module(fx)}));
    per(&mut c, "module.restore", "module.restore", U, Cheap, move |fx| {
        let id = fresh_module(fx);
        fx.user("module.delete", json!({"module_id": id}));
        json!({"module_id": id})
    });
    fixed(&mut c, "module.stats", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "module.changelog.draft", "module.changelog.draft", U, Cheap, |fx| json!({"module_id": fx.module_id}));

    // ---- notes
    fixed(&mut c, "notes.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "notes.get", "notes.get", U, Cheap, |fx| json!({"note_id": fx.note_id}));
    created(&mut c, "notes.create", "notes.create", U, Cheap, move |fx| json!({"project_id": 1, "title": format!("n{}", one(fx)), "body": "note body"}), |r| ("notes.delete", json!({"note_id": Fixture::id(r)})));
    let fresh_note = move |fx: &Arc<Fixture>| {
        let id = Fixture::id(&fx.user("notes.create", json!({"project_id": 1, "title": format!("fn{}", one(fx)), "body": "b"})));
        fx.later(U, "notes.delete", json!({"note_id": id}));
        id
    };
    per(&mut c, "notes.update", "notes.update", U, Cheap, move |fx| json!({"note_id": fx.note_id, "body": format!("edited {}", one(fx))}));
    per(&mut c, "notes.append", "notes.append", U, Cheap, move |fx| json!({"note_id": fresh_note(fx), "text": "appended line"}));
    per(&mut c, "notes.pin", "notes.pin", U, Cheap, move |fx| json!({"note_id": fx.note_id, "pinned": one(fx) % 2 == 0}));
    per(&mut c, "notes.delete", "notes.delete", U, Cheap, move |fx| json!({"note_id": fresh_note(fx)}));
    per(&mut c, "notes.restore", "notes.restore", U, Cheap, move |fx| {
        let id = fresh_note(fx);
        fx.user("notes.delete", json!({"note_id": id}));
        json!({"note_id": id})
    });
    fixed(&mut c, "notes.standing", U, Cheap, json!({"project_id": 1}));

    // ---- mailbox
    // Messages have no delete: thirty iterations, and the mailbox they land in is listed below.
    per(&mut c, "mailbox.send", "mailbox.send", U, Mid, |fx| json!({"project_id": 1, "to": fx.reviewer, "text": "a message about the review"}));
    per(&mut c, "mailbox.send.priority", "mailbox.send", U, Mid, |fx| json!({"project_id": 1, "to": fx.reviewer, "text": "urgent", "priority": true}));
    per(&mut c, "mailbox.list", "mailbox.list", U, Cheap, |fx| json!({"project_id": 1, "session": fx.reviewer}));
    fixed(&mut c, "mailbox.outbox", U, Cheap, json!({"project_id": 1}));
    c.push(Case {
        name: "op.mailbox.ack".into(),
        op: Some("mailbox.ack".into()),
        cost: Mid,
        prepare: Box::new(|fx| {
            let sent = fx.user("mailbox.send", json!({"project_id": 1, "to": fx.reviewer, "text": "ack me"}));
            let id = Fixture::id(&sent["message"]);
            dispatch(fx, Actor::agent(fx.reviewer.clone()), "mailbox.ack", json!({"message_id": id}))
        }),
    });

    // ---- session
    c.push(Case {
        name: "op.session.create".into(),
        op: Some("session.create".into()),
        cost: Mid,
        prepare: Box::new(|fx| {
            let fx = fx.clone();
            Box::new(move || {
                let resp = fx.call(Actor::User, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}));
                if let Some(name) = resp.result.as_ref().and_then(|r| r["name"].as_str()) {
                    fx.close_later(name.to_string());
                }
                Outcome::of(resp)
            })
        }),
    });
    per(&mut c, "session.spawn", "session.spawn", U, Heavy, |fx| json!({"session": fresh_session(fx)}));
    per(&mut c, "session.resume", "session.resume", U, Mid, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.clear_restorable", "session.clear_restorable", U, Cheap, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.park", "session.park", U, Heavy, |fx| json!({"session": spawned_session(fx)}));
    per(&mut c, "session.wake", "session.wake", U, Heavy, |fx| {
        let name = spawned_session(fx);
        fx.user("session.park", json!({"session": name}));
        json!({"session": name})
    });
    per(&mut c, "session.close", "session.close", U, Heavy, |fx| json!({"session": spawned_session(fx)}));
    per(&mut c, "session.close.created", "session.close", U, Mid, |fx| json!({"session": fresh_session(fx)}));
    // What the shell sends: close the pane, keep the checkout.
    per(&mut c, "session.close.keep_worktree", "session.close", U, Heavy, |fx| json!({"session": spawned_session(fx), "remove_worktree": false}));
    per_agent(&mut c, "session.done", "session.done", Mid, |fx| {
        fx.user("task.dispatch", json!({"task_id": fresh_task(fx), "session": fx.builder, "start": false}));
        json!({"session": fx.builder, "summary": "done", "status": "completed"})
    });
    per_agent(&mut c, "session.intent", "session.intent", Cheap, |fx| json!({"session": fx.builder, "text": "working on the thing"}));
    per_agent(&mut c, "session.claim", "session.claim", Cheap, move |fx| {
        let path = format!("src/claim-{}.rs", one(fx));
        fx.later(fx.agent(), "session.release", json!({"paths": [path]}));
        json!({"paths": [path]})
    });
    per_agent(&mut c, "session.release", "session.release", Cheap, move |fx| {
        let path = format!("src/rel-{}.rs", one(fx));
        fx.call(fx.agent(), "session.claim", json!({"paths": [path]}));
        json!({"paths": [path]})
    });
    per(&mut c, "session.get", "session.get", U, Cheap, |fx| json!({"session": fx.builder}));
    fixed(&mut c, "session.list", U, Cheap, json!({"project_id": 1}));
    fixed_as(&mut c, "session.list.all", "session.list", U, Cheap, json!({"include_closed": true}));
    per(&mut c, "session.peers", "session.peers", U, Cheap, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.brief", "session.brief", U, Cheap, |fx| json!({"session": fx.builder}));
    per_agent(&mut c, "session.bootstrap", "session.bootstrap", Cheap, |_| json!({}));
    per(&mut c, "session.update", "session.update", U, Mid, |fx| json!({"session": fresh_session(fx), "effort": "high"}));
    per_agent(&mut c, "session.report", "session.report", Cheap, |fx| json!({"session": fx.builder, "kind": "tool_use", "data": {"tool": "Read", "path": "src/a.rs"}}));
    per(&mut c, "session.attach", "session.attach", U, Cheap, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.detach", "session.detach", U, Cheap, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.input", "session.input", U, Cheap, |fx| json!({"session": fx.builder, "data": "k"}));
    per(&mut c, "session.input.paste_4k", "session.input", U, Cheap, |fx| json!({"session": fx.builder, "data": kib(4)}));
    per(&mut c, "session.resize", "session.resize", U, Cheap, move |fx| json!({"session": fx.builder, "cols": 100 + (one(fx) % 2) as u32, "rows": 40}));
    per(&mut c, "session.scrollback", "session.scrollback", U, Cheap, |fx| json!({"session": fx.builder}));
    per(&mut c, "session.scrollback.tail", "session.scrollback", U, Cheap, |fx| json!({"session": fx.builder, "lines": 20}));
    fixed(&mut c, "session.restorable", U, Cheap, json!({}));
    per(&mut c, "session.discard_restorable", "session.discard_restorable", U, Cheap, |fx| json!({"session": fx.builder}));

    // ---- overlap
    fixed(&mut c, "overlap.list", U, Cheap, json!({"project_id": 1}));
    per_agent(&mut c, "overlap.flag", "overlap.flag", Heavy, move |fx| json!({"project_id": 1, "path": format!("src/ov-{}.rs", one(fx) % 3)}));
    per_agent(&mut c, "overlap.ack", "overlap.ack", Heavy, move |fx| {
        let flagged = fx.call(fx.agent(), "overlap.flag", json!({"project_id": 1, "path": format!("src/ack-{}.rs", one(fx))})).into_result().unwrap_or_default();
        json!({"overlap_id": flagged["id"].as_i64().unwrap_or(0)})
    });
    fixed(&mut c, "overlap.scan", U, Heavy, json!({"project_id": 1}));

    // ---- guardrail
    per_agent(&mut c, "guardrail.gate", "guardrail.gate", Cheap, |fx| json!({"session": fx.builder, "kind": "exec", "command": "cargo test"}));
    // A refusal leaves a notification and a hold leaves a row; neither can be deleted, so
    // these run thirty times, and a hold is rejected before the next iteration.
    per_agent(&mut c, "guardrail.gate.refused", "guardrail.gate", Mid, move |fx| json!({"session": fx.builder, "kind": "write", "path": format!("secret/k-{}", one(fx)), "new_text": "x"}));
    c.push(Case {
        name: "op.guardrail.gate.hold".into(),
        op: Some("guardrail.gate".into()),
        cost: Mid,
        prepare: Box::new(move |fx| {
            let fx = fx.clone();
            let req = Request::new(U, "guardrail.gate", json!({"session": fx.builder, "kind": "write", "path": format!("secret/h-{}", one(&fx)), "new_text": "x"}));
            Box::new(move || {
                let resp = fx.engine.dispatch(req, Door::InProcess);
                if let Some(id) = resp.error.as_ref().and_then(|e| e.confirm.as_ref()).and_then(|c| c.payload["hold_id"].as_i64()) {
                    fx.later(U, "guardrail.reject", json!({"hold_id": id, "reason": "perf"}));
                }
                Outcome::of(resp)
            })
        }),
    });
    fixed(&mut c, "guardrail.holds.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "guardrail.hold.get", "guardrail.hold.get", U, Mid, |fx| json!({"hold_id": last_hold(fx)}));
    per(&mut c, "guardrail.confirm", "guardrail.confirm", U, Mid, |fx| json!({"hold_id": last_hold(fx)}));
    per(&mut c, "guardrail.reject", "guardrail.reject", U, Mid, |fx| json!({"hold_id": last_hold(fx), "reason": "no"}));
    fixed(&mut c, "guardrail.config.get", U, Cheap, json!({"project_id": 1}));
    fixed(&mut c, "guardrail.config.set", U, Cheap, json!({"project_id": 1, "patch": {"protected_paths": ["secret/*"]}}));
    fixed(&mut c, "guardrail.check", U, Cheap, json!({"project_id": 1, "kind": "exec", "command": "git push --force origin main"}));
    fixed(&mut c, "guardrail.explain", U, Cheap, json!({"project_id": 1, "paths": ["src/a.rs", "secret/x"], "lines": 400, "commands": ["rm -rf build", "git push"]}));

    // ---- worktree
    fixed(&mut c, "worktree.list", U, Mid, json!({"project_id": 1}));
    fixed_as(&mut c, "worktree.list.dirty", "worktree.list", U, Mid, json!({"project_id": 1, "include_dirty": true}));
    created(&mut c, "worktree.create", "worktree.create", U, Heavy, move |fx| json!({"project_id": 1, "branch": format!("relay/perf-{}", one(fx))}), |r| ("worktree.remove", json!({"project_id": 1, "path": r["path"]})));
    per(&mut c, "worktree.remove", "worktree.remove", U, Heavy, move |fx| {
        let w = fx.user("worktree.create", json!({"project_id": 1, "branch": format!("relay/rm-{}", one(fx))}));
        json!({"project_id": 1, "path": w["path"]})
    });
    fixed(&mut c, "worktree.disk", U, Mid, json!({"project_id": 1}));

    // ---- git
    fixed(&mut c, "git.status", U, Mid, json!({"project_id": 1}));
    fixed(&mut c, "git.diff", U, Mid, json!({"project_id": 1}));
    fixed_as(&mut c, "git.diff.staged", "git.diff", U, Mid, json!({"project_id": 1, "staged": true}));
    per(&mut c, "git.diff.file", "git.diff.file", U, Mid, |fx| json!({"project_id": 1, "path": fx.dirty_file}));
    fixed(&mut c, "git.log", U, Mid, json!({"project_id": 1, "limit": 50}));
    per(&mut c, "git.show", "git.show", U, Mid, |fx| json!({"project_id": 1, "sha": fx.head_sha}));
    fixed(&mut c, "git.branches", U, Mid, json!({"project_id": 1}));
    created(&mut c, "git.branch.create", "git.branch.create", U, Mid, move |fx| json!({"project_id": 1, "name": format!("perf/b-{}", one(fx)), "checkout": false}), |r| ("git.branch.delete", json!({"project_id": 1, "name": r["name"].as_str().or_else(|| r["branch"].as_str()).unwrap_or("")})));
    per(&mut c, "git.branch.switch", "git.branch.switch", U, Mid, move |fx| {
        let name = format!("perf/sw-{}", one(fx));
        fx.user("git.branch.create", json!({"project_id": 1, "worktree": fx.clean_worktree, "name": name, "checkout": false}));
        json!({"project_id": 1, "worktree": fx.clean_worktree, "name": name})
    });
    per(&mut c, "git.branch.delete", "git.branch.delete", U, Mid, move |fx| {
        let name = format!("perf/del-{}", one(fx));
        fx.user("git.branch.create", json!({"project_id": 1, "name": name, "checkout": false}));
        json!({"project_id": 1, "name": name})
    });
    per(&mut c, "git.stage", "git.stage", U, Mid, |fx| {
        fx.call(U, "git.unstage", json!({"project_id": 1, "paths": [fx.dirty_file]}));
        json!({"project_id": 1, "paths": [fx.dirty_file]})
    });
    per(&mut c, "git.unstage", "git.unstage", U, Mid, |fx| {
        fx.call(U, "git.stage", json!({"project_id": 1, "paths": [fx.dirty_file]}));
        json!({"project_id": 1, "paths": [fx.dirty_file]})
    });
    per(&mut c, "git.commit", "git.commit", U, Mid, move |fx| {
        std::fs::write(fx.repo.join(format!("commit-{}.txt", one(fx))), "x\n").unwrap();
        git(&fx.repo, &["add", "-A", "--", "commit-*.txt"]);
        json!({"project_id": 1, "message": "perf: commit"})
    });
    fixed(&mut c, "git.fetch", U, Mid, json!({"project_id": 1}));
    per(&mut c, "git.push", "git.push", U, Mid, move |fx| {
        std::fs::write(fx.repo.join(format!("push-{}.txt", one(fx))), "x\n").unwrap();
        git(&fx.repo, &["add", "-A", "--", "push-*.txt"]);
        git(&fx.repo, &["commit", "-qm", "perf: push"]);
        json!({"project_id": 1})
    });
    // These reach gh: the refusing stand-in.
    fixed(&mut c, "git.pr.list", U, Mid, json!({"project_id": 1}));
    fixed(&mut c, "git.pr.open", U, Mid, json!({"project_id": 1, "title": "perf"}));
    fixed(&mut c, "git.branch.clean_merged", U, Mid, json!({"project_id": 1, "dry_run": true}));
    fixed(&mut c, "git.suggest_message", U, Mid, json!({"project_id": 1}));

    // ---- integration
    // Two distinct session branches: the builder's and a fresh one.
    per(&mut c, "integration.request", "integration.request", U, Heavy, |fx| json!({"project_id": 1, "sessions": [fx.builder, fresh_session(fx)], "build": false}));
    let requested = |fx: &Arc<Fixture>| -> i64 {
        let r = fx.call(U, "integration.request", json!({"project_id": 1, "sessions": [fx.builder, fresh_session(fx)], "build": false}));
        r.result.as_ref().and_then(|v| v["id"].as_i64()).unwrap_or(1)
    };
    let once = std::sync::OnceLock::new();
    per(&mut c, "integration.get", "integration.get", U, Cheap, move |fx| json!({"integration_id": once.get_or_init(|| requested(fx))}));
    fixed(&mut c, "integration.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "integration.discard", "integration.discard", U, Heavy, move |fx| json!({"integration_id": requested(fx)}));

    // ---- file
    fixed(&mut c, "file.tree", U, Mid, json!({"project_id": 1, "depth": 1}));
    fixed_as(&mut c, "file.tree.deep", "file.tree", U, Mid, json!({"project_id": 1, "depth": 10, "git_badges": true}));
    fixed(&mut c, "file.read", U, Cheap, json!({"project_id": 1, "path": "README.md"}));
    fixed_as(&mut c, "file.read.1mib", "file.read", U, Mid, json!({"project_id": 1, "path": "large.txt"}));
    fixed_as(&mut c, "file.write.4k", "file.write", U, Mid, json!({"project_id": 1, "path": "scratch-4k.txt", "text": kib(4)}));
    fixed_as(&mut c, "file.write.1mib", "file.write", U, Mid, json!({"project_id": 1, "path": "scratch-1m.txt", "text": kib(1024)}));
    // Scratch files live in one directory the tree and search cases skip over cheaply, and
    // every one is removed from disk, not trashed, before the next iteration.
    let scratch = move |fx: &Arc<Fixture>, prefix: &str| -> String {
        let _ = std::fs::create_dir_all(fx.repo.join("scratch"));
        format!("scratch/{prefix}-{}.txt", one(fx))
    };
    let unlink = |fx: &Fixture, rel: &str| { let _ = std::fs::remove_file(fx.repo.join(rel)); };
    per(&mut c, "file.create", "file.create", U, Cheap, move |fx| {
        let _ = std::fs::remove_dir_all(fx.repo.join("scratch"));
        json!({"project_id": 1, "path": scratch(fx, "new"), "kind": "file", "text": "x"})
    });
    let made = move |fx: &Arc<Fixture>, prefix: &str| -> String {
        let path = scratch(fx, prefix);
        fx.user("file.create", json!({"project_id": 1, "path": path, "kind": "file", "text": "x"}));
        path
    };
    per(&mut c, "file.rename", "file.rename", U, Cheap, move |fx| {
        let path = made(fx, "ren");
        let new_name = format!("renamed-{}.txt", one(fx));
        unlink(fx, &format!("scratch/{new_name}"));
        json!({"project_id": 1, "path": path, "new_name": new_name})
    });
    per(&mut c, "file.move", "file.move", U, Cheap, move |fx| {
        let _ = std::fs::create_dir_all(fx.repo.join("scratch/moved"));
        json!({"project_id": 1, "path": made(fx, "mv"), "into": "scratch/moved"})
    });
    per(&mut c, "file.delete", "file.delete", U, Cheap, move |fx| json!({"project_id": 1, "path": made(fx, "del")}));
    per(&mut c, "file.restore", "file.restore", U, Cheap, move |fx| {
        let d = fx.user("file.delete", json!({"project_id": 1, "path": made(fx, "rest")}));
        json!({"project_id": 1, "trash_id": d["trash_id"].as_i64().unwrap_or(0)})
    });
    per(&mut c, "file.restore_head", "file.restore_head", U, Mid, |fx| {
        std::fs::write(fx.repo.join("CHANGES-0.md"), "dirty\n").unwrap();
        json!({"project_id": 1, "path": "CHANGES-0.md"})
    });
    per(&mut c, "file.import", "file.import", U, Cheap, move |fx| {
        let src = fx.root.join(format!("import-{}.txt", one(fx)));
        std::fs::write(&src, "imported\n").unwrap();
        let _ = std::fs::create_dir_all(fx.repo.join("imported"));
        json!({"project_id": 1, "into": "imported", "sources": [src]})
    });
    fixed(&mut c, "file.search", U, Mid, json!({"project_id": 1, "query": "item_03_", "limit": 100}));
    fixed_as(&mut c, "file.search.regex", "file.search", U, Mid, json!({"project_id": 1, "query": "fn item_\\d+_1_", "regex": true, "limit": 100}));

    // ---- device / avd: adb, the emulator and avdmanager are refusing stand-ins (`isolate_host`),
    // so these measure the refusal path and never reach a real device, server or AVD.
    fixed(&mut c, "device.list", U, Mid, json!({}));
    fixed(&mut c, "device.watch", U, Mid, json!({"on": true}));
    fixed(&mut c, "device.mirror.start", U, Mid, json!({"device": "emulator-5554"}));
    fixed(&mut c, "device.mirror.stop", U, Cheap, json!({"mirror_id": 1}));
    fixed(&mut c, "device.mirror.input", U, Cheap, json!({"mirror_id": 1, "event": {"type": "tap", "x": 1, "y": 1}}));
    fixed(&mut c, "device.run", U, Mid, json!({"project_id": 1, "device": "emulator-5554"}));
    fixed(&mut c, "device.build", U, Mid, json!({"project_id": 1}));
    fixed(&mut c, "device.signing.get", U, Cheap, json!({"project_id": 1}));
    fixed(&mut c, "device.signing.create", U, Mid, json!({"project_id": 1, "key_alias": "perf", "password": "perfperf"}));
    fixed(&mut c, "device.signing.set_enabled", U, Cheap, json!({"project_id": 1, "enabled": false}));
    fixed(&mut c, "device.run.stop", U, Cheap, json!({"run_id": 1}));
    fixed(&mut c, "device.run.list", U, Cheap, json!({"project_id": 1}));
    fixed(&mut c, "avd.list", U, Mid, json!({}));
    fixed(&mut c, "avd.catalog", U, Mid, json!({}));
    fixed(&mut c, "avd.create", U, Mid, json!({"name": "perf", "package": "system-images;android-34;google_apis;x86_64"}));
    fixed(&mut c, "avd.boot", U, Mid, json!({"name": "perf"}));
    fixed(&mut c, "avd.stop", U, Mid, json!({"name": "perf"}));

    // ---- provider / usage / skill / github / plugin
    fixed(&mut c, "provider.list", U, Cheap, json!({}));
    fixed(&mut c, "provider.refresh", U, Heavy, json!({}));
    fixed(&mut c, "provider.update", U, Mid, json!({"provider": "claude", "automatic": false}));
    fixed(&mut c, "usage.get", U, Mid, json!({}));
    per_agent(&mut c, "usage.report", "usage.report", Cheap, |fx| json!({"session": fx.builder, "provider": "claude", "payload": {"input_tokens": 1200, "output_tokens": 300, "cost_usd": 0.01}}));
    fixed(&mut c, "skill.list", U, Cheap, json!({"project_id": 1}));
    created(&mut c, "skill.create", "skill.create", U, Mid, move |fx| json!({"name": format!("sk-{}", one(fx)), "body": "---\nname: x\n---\nbody"}), |r| ("skill.delete", json!({"skill_id": Fixture::id(r)})));
    per(&mut c, "skill.update", "skill.update", U, Mid, move |fx| json!({"skill_id": fx.skill_id, "body": format!("---\nname: x\n---\nbody {}", one(fx))}));
    per(&mut c, "skill.delete", "skill.delete", U, Mid, move |fx| json!({"skill_id": Fixture::id(&fx.user("skill.create", json!({"name": format!("skd-{}", one(fx)), "body": "b"})))}));
    per(&mut c, "skill.enable", "skill.enable", U, Mid, move |fx| json!({"skill_id": fx.skill_id, "project_id": 1, "enabled": one(fx) % 2 == 0}));
    per(&mut c, "skill.install", "skill.install", U, Heavy, |fx| json!({"url": fx.skill_repo, "replace_skill_id": fx.user("skill.list", json!({})).get("skills").and_then(|s| s.as_array()).and_then(|s| s.iter().find(|s| s["name"] == "perf-skill")).map(Fixture::id)}));
    // gh is a refusing stand-in: the not-connected path, never a login, an API call or a token.
    fixed(&mut c, "github.status", U, Mid, json!({}));
    fixed(&mut c, "github.connect", U, Mid, json!({}));
    fixed(&mut c, "github.repo.list", U, Mid, json!({}));
    fixed(&mut c, "plugin.list", U, Cheap, json!({}));

    // ---- notify / settings / dashboard
    fixed(&mut c, "notify.list", U, Cheap, json!({}));
    per(&mut c, "notify.ack", "notify.ack", U, Cheap, |fx| {
        last_hold(fx);
        let list = fx.user("notify.list", json!({"unread_only": true, "limit": 1}));
        json!({"notification_id": list["notifications"].as_array().and_then(|n| n.first()).map(Fixture::id).unwrap_or(0)})
    });
    fixed(&mut c, "notify.ack_all", U, Cheap, json!({}));
    fixed(&mut c, "notify.settings.get", U, Cheap, json!({}));
    fixed(&mut c, "notify.settings.set", U, Cheap, json!({"patch": {"sound": false}}));
    fixed(&mut c, "settings.get", U, Cheap, json!({}));
    fixed_as(&mut c, "settings.get.path", "settings.get", U, Cheap, json!({"path": "providers.claude.path"}));
    per(&mut c, "settings.set", "settings.set", U, Cheap, move |fx| json!({"path": "perf.value", "value": one(fx)}));
    per(&mut c, "settings.reset", "settings.reset", U, Cheap, |fx| {
        fx.user("settings.set", json!({"path": "perf.reset", "value": 1}));
        json!({"path": "perf.reset"})
    });
    fixed(&mut c, "dashboard.get", U, Cheap, json!({}));

    // ---- ui / os (core answers ui.* from its in-memory shell model, client or not; os.* hands the
    // target to `xdg-open` after commit — the stand-in, which logs it)
    fixed(&mut c, "ui.state", U, Cheap, json!({}));
    fixed(&mut c, "ui.page.switch", U, Cheap, json!({"page": "board", "project_id": 1}));
    fixed(&mut c, "ui.pane.open", U, Cheap, json!({"kind": "notes"}));
    fixed(&mut c, "ui.pane.close", U, Cheap, json!({"pane": "p1"}));
    fixed(&mut c, "ui.pane.focus", U, Cheap, json!({"pane": "p1"}));
    fixed(&mut c, "ui.pane.move", U, Cheap, json!({"pane": "p1", "to": "p2", "edge": "left"}));
    fixed(&mut c, "ui.layout.list", U, Cheap, json!({"project_id": 1}));
    per(&mut c, "ui.layout.save", "ui.layout.save", U, Cheap, move |fx| json!({"project_id": 1, "name": "perf", "state": {"agent_layout": if one(fx) % 2 == 0 { "grid" } else { "focus" }, "panes": [1, 2, 3]}}));
    fixed(&mut c, "ui.layout.apply", U, Cheap, json!({"project_id": 1, "name": "perf"}));
    per(&mut c, "ui.layout.delete", "ui.layout.delete", U, Cheap, move |fx| {
        let name = format!("del-{}", one(fx));
        fx.user("ui.layout.save", json!({"project_id": 1, "name": name, "state": {}}));
        json!({"project_id": 1, "name": name})
    });
    fixed(&mut c, "ui.window.popout", U, Cheap, json!({"pane": "p1"}));
    fixed(&mut c, "ui.window.close", U, Cheap, json!({"window_id": "w1"}));
    fixed(&mut c, "ui.window.list", U, Cheap, json!({}));
    fixed(&mut c, "ui.toast", U, Cheap, json!({"text": "hi"}));
    per(&mut c, "os.reveal", "os.reveal", U, Cheap, |fx| json!({"path": fx.repo}));
    fixed(&mut c, "os.open_url", U, Cheap, json!({"url": "https://example.invalid/"}));

    // ---- paths that are not ops
    path_case(&mut c, "store.open_fresh", Mid, move |fx| {
        let path = fx.root.join(format!("fresh-{}", one(fx))).join("store.db");
        Box::new(move || {
            let store = Store::open(&path, false).unwrap();
            drop(store);
            Outcome::plain(0)
        })
    });
    path_case(&mut c, "store.open_existing", Mid, |fx| {
        let path = fx.store_path.clone();
        Box::new(move || {
            let store = Store::open(&path, false).unwrap();
            drop(store);
            Outcome::plain(0)
        })
    });
    path_case(&mut c, "engine.new", Mid, |_| {
        Box::new(|| {
            let e = Engine::new(Instance::Test, Store::open_memory().unwrap());
            drop(e);
            Outcome::plain(0)
        })
    });
    path_case(&mut c, "engine.startup", Mid, |fx| {
        // What `relay serve` does before it binds: open the populated store, build the engine,
        // run crash recovery, materialize skills into every checkout. On a backup of the store,
        // because recovery against the live one would reap the fixture's own sessions.
        // The deferred dirty scan outlives the region on a worker holding the engine; it is
        // waited for, and the engine dropped, before the next iteration (`settle_later`).
        let path = store_copy(fx);
        let fx = fx.clone();
        Box::new(move || {
            let store = Store::open(&path, false).unwrap();
            let engine = Engine::new(Instance::Test, store);
            let r = relay_core::recovery::run_with(&engine, relay_core::recovery::DirtyScan::Deferred);
            relay_core::skills::refresh_all(&engine);
            engine.shutdown();
            fx.settle_later(engine);
            Outcome::res(r, |_| 0)
        })
    });
    path_case(&mut c, "recovery.run", Mid, |fx| {
        let engine = Engine::new(Instance::Test, Store::open(&store_copy(fx), false).unwrap());
        let fx = fx.clone();
        Box::new(move || {
            let r = relay_core::recovery::run_with(&engine, relay_core::recovery::DirtyScan::Deferred);
            engine.shutdown();
            fx.settle_later(engine);
            Outcome::res(r, |_| 0)
        })
    });
    path_case(&mut c, "skills.refresh_all", Mid, |fx| {
        let engine = fx.engine.clone();
        Box::new(move || {
            relay_core::skills::refresh_all(&engine);
            Outcome::plain(0)
        })
    });
    path_case(&mut c, "request.parse_8k", Cheap, |_| {
        let line = serde_json::to_string(&Request::new(Actor::User, "notes.create", json!({"project_id": 1, "body": kib(8)}))).unwrap();
        let len = line.len();
        Box::new(move || Outcome::res(Engine::parse(&line).map_err(|r| r.error.map(|e| e.code).unwrap_or_default()), |_| len))
    });
    path_case(&mut c, "response.serialize.session_list", Cheap, |fx| {
        let resp = fx.call(Actor::User, "session.list", json!({"project_id": 1}));
        Box::new(move || {
            let ok = resp.ok;
            let out = Outcome::res(serde_json::to_string(&resp), String::len);
            // A failed list would measure serializing an error, not the list.
            Outcome { ok: ok && out.ok, ..out }
        })
    });
    let socket = |c: &mut Vec<Case>, name: &str, actor: Actor, op: &'static str, payload: Value| {
        path_case(c, name, Cheap, move |fx| {
            let fx = fx.clone();
            let req = Request::new(actor.clone(), op, payload.clone());
            Box::new(move || {
                let mut client = fx.client.lock().unwrap();
                let resp = fx.rt.block_on(client.call(&req, |_| {})).unwrap();
                Outcome::of(resp)
            })
        });
    };
    socket(&mut c, "socket.ping", Actor::User, "bus.ping", json!({}));
    socket(&mut c, "socket.bus.subscribe", Actor::User, "bus.subscribe", json!({"events": ["settings.changed"]}));
    socket(&mut c, "socket.bus.unsubscribe", Actor::User, "bus.unsubscribe", json!({}));
    socket(&mut c, "socket.session_list", Actor::User, "session.list", json!({"project_id": 1}));
    socket(&mut c, "socket.task_list", Actor::User, "task.list", json!({"project_id": 1}));
    path_case(&mut c, "socket.keystroke", Cheap, |fx| {
        let fx = fx.clone();
        let req = Request::new(Actor::User, "session.input", json!({"session": fx.builder, "data": "k"}));
        Box::new(move || {
            let mut client = fx.client.lock().unwrap();
            let resp = fx.rt.block_on(client.call(&req, |_| {})).unwrap();
            Outcome::of(resp)
        })
    });
    path_case(&mut c, "socket.connect", Mid, |fx| {
        let fx = fx.clone();
        Box::new(move || {
            let path = fx.socket_dir.join("test.sock");
            let mut client = fx.rt.block_on(Client::connect(&path)).unwrap();
            let resp = fx.rt.block_on(client.call(&Request::new(Actor::User, "bus.ping", json!({})), |_| {})).unwrap();
            Outcome::of(resp)
        })
    });
    path_case(&mut c, "event.roundtrip", Cheap, |fx| {
        // A second connection subscribed to settings.changed; the measured region is one
        // mutation and the wait for its event to come out of the other socket.
        let fx = fx.clone();
        let path = fx.socket_dir.join("test.sock");
        let mut sub = fx.rt.block_on(Client::connect(&path)).unwrap();
        let ok = fx.rt.block_on(sub.call(&Request::new(Actor::User, "bus.subscribe", json!({"events": ["settings.changed"]})), |_| {})).unwrap();
        let n = fx.next();
        Box::new(move || {
            if !ok.ok {
                return Outcome::of(ok);
            }
            let mut client = fx.client.lock().unwrap();
            let resp = fx.rt.block_on(client.call(&Request::new(Actor::User, "settings.set", json!({"path": "perf.event", "value": n})), |_| {})).unwrap();
            let got = fx.rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        match sub.next().await {
                            Ok(Some(relay_core::socket::Line::Event(e))) if e.ev == "settings.changed" => break true,
                            Ok(Some(_)) => continue,
                            _ => break false,
                        }
                    }
                })
                .await
                .unwrap_or(false)
            });
            let code = resp.error.map(|e| e.code).or_else(|| (!got).then(|| "no_event".to_string()));
            Outcome { ok: resp.ok && got, code, result: None, bytes: 0, overlap: None }
        })
    });
    path_case(&mut c, "pty.burst_1mib", Heavy, |fx| {
        // 10 000 lines of 100 bytes from the child, measured until the last one is readable
        // through the bus: PTY read, ring append, frame broadcast, scrollback tail.
        let fx = fx.clone();
        let name = spawned_session(&fx);
        Box::new(move || {
            fx.user("session.input", json!({"session": name, "data": "burst 10000\n"}));
            fx.wait_scrollback(&name, "BURST-END");
            fx.close_later(name);
            Outcome::plain(10_000 * 100)
        })
    });
    path_case(&mut c, "pty.attach_catchup", Mid, |fx| {
        // Attach against a ring that holds a full burst: what a reconnecting client copies.
        let fx = fx.clone();
        let name = spawned_session(&fx);
        fx.user("session.input", json!({"session": name, "data": "burst 10000\n"}));
        fx.wait_scrollback(&name, "BURST-END");
        let pty = fx.engine.pty_named(&name).map(|(_, p)| p).unwrap();
        Box::new(move || {
            let attached = pty.attach(None, None);
            let bytes = attached.catch_up.len();
            fx.close_later(name);
            Outcome::plain(bytes)
        })
    });
    path_case(&mut c, "pty.time_to_first_output", Heavy, |fx| {
        // From `session.spawn` to the provider's first line being readable: fork, PTY, the
        // shell's own startup, the reader thread, the ring. What a person waits for.
        let fx = fx.clone();
        let name = fresh_session(&fx);
        Box::new(move || {
            fx.user("session.spawn", json!({"session": name}));
            fx.wait_scrollback(&name, "hello-from-pty");
            fx.close_later(name);
            Outcome::plain(0)
        })
    });
    path_case(&mut c, "session.lifecycle", Heavy, |fx| {
        let fx = fx.clone();
        Box::new(move || {
            let name = spawned_session(&fx);
            fx.user("session.close", json!({"session": name}));
            Outcome::plain(0)
        })
    });
    // What the shell pays while something else is running: the other op starts on a second
    // thread, and the measured region is one request issued a millisecond after that thread
    // began. An op that holds the store mutex makes every locked request wait for all of it; a
    // keystroke never waits (D148), a session list or a scrollback tail does. Each row says in
    // how many iterations the other op was still in flight when the request went out
    // (`overlap`): one that is over by then measures an uncontended request, which is why
    // `task.list` and a `device.build` with no Gradle wrapper (both well under a millisecond)
    // are not here. The scan, search and status take a few milliseconds on a fast machine.
    let during = |c: &mut Vec<Case>, name: &str, other: &'static str, payload: Value, measured: &'static str, measured_payload: fn(&Fixture) -> Value| {
        path_case(c, name, Mid, move |fx| {
            let runner = fx.clone();
            let payload = payload.clone();
            let (span, running) = beside(Duration::from_millis(1), move || {
                runner.call(Actor::User, other, payload);
            });
            let fx = fx.clone();
            Box::new(move || {
                let issued = Instant::now();
                let mut out = Outcome::of(fx.call(Actor::User, measured, measured_payload(&fx)));
                out.overlap = Some(Overlap { other: span, issued });
                fx.join_later(running);
                out
            })
        });
    };
    let keystroke = |fx: &Fixture| json!({"session": fx.builder, "data": "k"});
    let tail = |fx: &Fixture| json!({"session": fx.builder, "lines": 20});
    let sessions = |_: &Fixture| json!({"project_id": 1});
    during(&mut c, "keystroke.during_overlap_scan", "overlap.scan", json!({"project_id": 1}), "session.input", keystroke);
    during(&mut c, "keystroke.during_file_search", "file.search", json!({"project_id": 1, "query": "nothing-matches-this", "limit": 10}), "session.input", keystroke);
    during(&mut c, "session_list.during_overlap_scan", "overlap.scan", json!({"project_id": 1}), "session.list", sessions);
    during(&mut c, "session_list.during_file_search", "file.search", json!({"project_id": 1, "query": "nothing-matches-this", "limit": 10}), "session.list", sessions);
    during(&mut c, "session_list.during_git_status", "git.status", json!({"project_id": 1}), "session.list", sessions);
    during(&mut c, "scrollback_tail.during_overlap_scan", "overlap.scan", json!({"project_id": 1}), "session.scrollback", tail);
    // The whole schema is ~4.5 ms of rendering; unlocked, a session list issued beside it does
    // not wait for it.
    during(&mut c, "session_list.during_bus_schema", "bus.schema", json!({}), "session.list", sessions);
    // Opening or closing one agent must not stall the panes of every other one. Half a
    // millisecond in: closing a session that keeps its checkout takes about 1.5 ms.
    let during_session = |c: &mut Vec<Case>, name: &str, other: &'static str, setup: fn(&Fixture) -> Value| {
        path_case(c, name, Heavy, move |fx| {
            let runner = fx.clone();
            let payload = setup(fx);
            let (span, running) = beside(Duration::from_micros(500), move || {
                runner.call(Actor::User, other, payload);
            });
            let fx = fx.clone();
            Box::new(move || {
                let issued = Instant::now();
                let mut out = Outcome::of(fx.call(Actor::User, "session.list", json!({"project_id": 1})));
                out.overlap = Some(Overlap { other: span, issued });
                fx.join_later(running);
                out
            })
        });
    };
    during_session(&mut c, "session_list.during_session_spawn", "session.spawn", |fx| json!({"session": fresh_session(fx)}));
    during_session(&mut c, "session_list.during_session_close", "session.close", |fx| json!({"session": spawned_session(fx)}));
    during_session(&mut c, "session_list.during_session_close_keep", "session.close", |fx| json!({"session": spawned_session(fx), "remove_worktree": false}));
    // `project.remove` backs the whole store up first. The fixture's store is 0.6 MB, over in a
    // millisecond, so 64 MB of ballast stands in for a real store's audit history (260 MB on the
    // 2026-10-14 machine): copied under the lock, that stalled every request for the whole copy.
    path_case(&mut c, "session_list.during_project_remove", Heavy, move |fx| {
        {
            let conn = fx.engine.store.lock();
            conn.execute_batch("CREATE TABLE IF NOT EXISTS perf_ballast(b BLOB)").unwrap();
            let rows: i64 = conn.query_row("SELECT COUNT(*) FROM perf_ballast", [], |r| r.get(0)).unwrap();
            if rows == 0 {
                conn.execute_batch("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1024) INSERT INTO perf_ballast SELECT randomblob(65536) FROM n").unwrap();
            }
        }
        let doomed = Fixture::id(&fx.user("project.add", json!({"workspace_id": 1, "path": small_repo(fx)})));
        let runner = fx.clone();
        let (span, running) = beside(Duration::from_micros(500), move || {
            runner.call(Actor::User, "project.remove", json!({"project_id": doomed}));
        });
        let fx = fx.clone();
        Box::new(move || {
            let issued = Instant::now();
            let mut out = Outcome::of(fx.call(Actor::User, "session.list", json!({"project_id": 1})));
            out.overlap = Some(Overlap { other: span, issued });
            fx.join_later(running);
            out
        })
    });
    path_case(&mut c, "keystroke.during_busy_thread", Mid, |fx| {
        // The control: a thread that only burns CPU for 30 ms, no engine involved.
        let fx = fx.clone();
        let (span, busy) = beside(Duration::from_millis(5), || {
            let t0 = Instant::now();
            let mut x = 0u64;
            while t0.elapsed() < Duration::from_millis(30) {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            std::hint::black_box(x);
        });
        Box::new(move || {
            let issued = Instant::now();
            let mut out = Outcome::of(fx.call(Actor::User, "session.input", json!({"session": fx.builder, "data": "k"})));
            out.overlap = Some(Overlap { other: span, issued });
            fx.join_later(busy);
            out
        })
    });
    path_case(&mut c, "worktree.dir_size", Mid, |fx| {
        // The source tree alone (1 500 files), not the pool of worktrees earlier cases left.
        let repo = fx.repo.join("src");
        Box::new(move || Outcome::plain(relay_core::worktree::dir_size(&repo) as usize))
    });
    path_case(&mut c, "worktree.status_files", Mid, |fx| {
        let repo = fx.repo.clone();
        Box::new(move || Outcome::res(relay_core::worktree::status_files(&repo), Vec::len))
    });
    path_case(&mut c, "guardrail.config_load", Cheap, |fx| {
        let engine = fx.engine.clone();
        Box::new(move || {
            let conn = engine.store.lock();
            Outcome::res(relay_core::guardrail::config(&conn, Some(1)).map_err(|e| e.code), |_| 0)
        })
    });
    design_cases(&mut c);
    c
}

// ------------------------------------------------------------------ design rows

/// The design rows' fixture (W44, B58, B85, TM-80): an engine of its own, so the designs' rows
/// (some 19,000 nodes) never sit in the store the other rows measure, with one project and these
/// designs, put back to how they were built before each case as [`Fixture::reset`] does:
///
/// - `500`, `2000`, `5000`: a page and frames of 49 children each (one in three a text), to
///   that many nodes; frame `f0`'s children are pinned to its right and bottom edges.
/// - `1721`, `4903`: `scripts/design-smoke.py perf`'s design (phone artboards in auto layout, a
///   title and ten rows of a text and a toggle each), with 40 and 114 artboards: the sizes the
///   2026-10-16 reading and teal-moose's TM-80 measured.
/// - `1721c`, `4903c`: those, with a chart that fills the first artboard's width at its top: a list
///   on the page lays the page out to size it (`charts.rs`, `fit_charts`).
/// - `siblings`: a page and 4,998 top-level nodes, one in five an empty frame (B58).
struct Designs {
    engine: Arc<Engine>,
    snapshot: PathBuf,
    ids: std::collections::BTreeMap<&'static str, i64>,
    /// `design.apply` payloads that put a measured edit back, sent uncounted before the next
    /// iteration.
    cleanup: Mutex<Vec<Value>>,
    /// The scenes the client rows hold between iterations, by name, with the rev each matches.
    scenes: Mutex<std::collections::HashMap<String, (relay_client::design::Scene, i64)>>,
}

const PAGE: &str = "page-1";

/// Frame `f{k}` of a grid design and its children, as adds.
fn grid_frame(k: usize, children: usize, pinned: bool) -> Vec<Value> {
    let f = format!("f{k}");
    let mut ops = vec![json!({"op": "add", "node": {"id": f, "parent": PAGE, "kind": "frame", "x": k % 100 * 1300, "y": k / 100 * 1000, "w": 1200, "h": 900}})];
    ops.extend((0..children).map(|i| {
        let mut node = json!({"id": format!("{f}-{i}"), "parent": f, "kind": if i % 3 == 0 { "text" } else { "rect" },
            "x": i * 3, "y": i * 2, "w": 40, "h": 20, "fill": "#336699"});
        if i % 3 == 0 {
            node["text"] = json!("Label");
        }
        if pinned {
            node["pin_x"] = json!("right");
            node["pin_y"] = json!("bottom");
        }
        json!({"op": "add", "node": node})
    }));
    ops
}

/// A grid design of `n` nodes, its page included.
fn grid(n: usize) -> Vec<Value> {
    let mut ops = Vec::new();
    let mut k = 0;
    while 1 + ops.len() < n {
        let children = (n - 1 - ops.len() - 1).min(49);
        ops.extend(grid_frame(k, children, k == 0));
        k += 1;
    }
    ops
}

/// `design-smoke.py perf`'s design with `boards` artboards: 1 + 43 × `boards` nodes.
fn smoke(boards: usize) -> Vec<Value> {
    let add = |node: Value| json!({"op": "add", "node": node});
    let mut ops = Vec::new();
    for a in 0..boards {
        let ab = format!("a{a}");
        ops.push(add(json!({"id": ab, "parent": PAGE, "kind": "frame", "name": format!("Screen {}", a + 1), "x": (a % 8) * 470, "y": (a / 8) * 924,
            "w": 390, "h": 844, "fill": "#F4F2EE", "layout": "column", "gap": 12, "padding": [48, 20]})));
        ops.push(add(json!({"id": format!("{ab}-t"), "parent": ab, "kind": "text", "text": format!("Screen {}", a + 1), "font_size": 28, "font_weight": 700})));
        ops.push(add(json!({"id": format!("{ab}-l"), "parent": ab, "kind": "frame", "fill": "#FFFFFF", "radius": 14, "layout": "column", "padding": [4, 14],
            "w_sizing": "fill", "h_sizing": "hug"})));
        for r in 0..10 {
            let row = format!("{ab}-r{r}");
            ops.push(add(json!({"id": row, "parent": format!("{ab}-l"), "kind": "frame", "fill": "#00000000", "layout": "row", "items": "center",
                "justify": "space_between", "padding": [12, 0], "w_sizing": "fill", "h_sizing": "hug"})));
            ops.push(add(json!({"id": format!("{row}-x"), "parent": row, "kind": "text", "text": format!("Setting number {}", r + 1), "font_size": 15})));
            ops.push(add(json!({"id": format!("{row}-s"), "parent": row, "kind": "frame", "w": 46, "h": 28, "radius": 14, "fill": "#2EC469",
                "layout": "row", "padding": [3], "justify": "end"})));
            ops.push(add(json!({"id": format!("{row}-k"), "parent": format!("{row}-s"), "kind": "ellipse", "w": 22, "h": 22, "fill": "#FFFFFF"})));
        }
    }
    ops
}

/// [`smoke`] with a bar chart that fills artboard `a0`'s width, first in it.
fn smoke_chart(boards: usize) -> Vec<Value> {
    let mut ops = smoke(boards);
    ops.insert(1, json!({"op": "add", "node": {"id": "a0-chart", "parent": "a0", "kind": "frame", "w": 300, "w_sizing": "fill", "h": 180,
        "chart": {"kind": "bar", "stacked": true, "labels": ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
            "series": [{"name": "Run", "values": [20, 0, 15, 30, 0, 45, 10]}, {"name": "Ride", "values": [0, 30, 10, 0, 25, 0, 60]}]}}}));
    ops
}

fn siblings() -> Vec<Value> {
    (0..4_998).map(|i| json!({"op": "add", "node": {
        "id": format!("s{i}"), "parent": PAGE, "kind": if i % 5 == 0 { "frame" } else { "rect" }, "x": i, "w": 40, "h": 20, "fill": "#336699",
    }})).collect()
}

/// The undo of an applied list, as the canvas sends it: its inverse, lenient, with what it
/// expects to find.
fn undo_of(design: i64, out: &Value) -> Value {
    json!({"design_id": design, "ops": out["inverse"], "lenient": true, "expect": out["expect"]})
}

/// What a client holds after reading `design.get`'s answer off the socket: the engine's bytes,
/// parsed, then decoded (`ui.call` and `decode` in the native client).
fn over_the_wire(result: &Value) -> relay_bus::ops::design::GetOut {
    let bytes = serde_json::to_vec(result).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    serde_json::from_value(value).unwrap()
}

impl Designs {
    fn build(root: &Path) -> Designs {
        let repo = root.join("design-ws").join("designs");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("README.md"), "# designs\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "designs"]);
        let store = Store::open(&root.join("design-store").join("store.db"), false).unwrap();
        let engine = Engine::new(Instance::Test, store);
        let mut d = Designs { engine, snapshot: root.join("design-baseline.db"), ids: Default::default(), cleanup: Mutex::new(Vec::new()), scenes: Mutex::new(Default::default()) };
        let ws = d.call("workspace.create", json!({"path": root.join("design-ws")}));
        d.call("project.add", json!({"workspace_id": Fixture::id(&ws), "path": repo}));
        for (name, ops) in [("500", grid(500)), ("2000", grid(2_000)), ("5000", grid(5_000)), ("1721", smoke(40)), ("4903", smoke(114)), ("1721c", smoke_chart(40)), ("4903c", smoke_chart(114)), ("siblings", siblings())] {
            let made = d.call("design.create", json!({"project_id": 1, "title": format!("perf {name}")}));
            let id = made["design"]["id"].as_i64().unwrap();
            assert_eq!(made["nodes"][0]["id"], PAGE);
            for chunk in ops.chunks(relay_bus::design::MAX_OPS) {
                d.apply(id, chunk.to_vec());
            }
            d.ids.insert(name, id);
        }
        d.engine.store.lock().backup(rusqlite::MAIN_DB, &d.snapshot, None).unwrap();
        d
    }

    fn call(&self, op: &str, payload: Value) -> Value {
        match self.engine.dispatch(Request::new(Actor::User, op, payload), Door::InProcess).into_result() {
            Ok(v) => v,
            Err(e) => panic!("design fixture: {op} failed: {} {}", e.code, e.message),
        }
    }

    fn apply(&self, design: i64, ops: Vec<Value>) -> Value {
        self.call("design.apply", json!({"design_id": design, "ops": ops}))
    }

    fn id(&self, name: &str) -> i64 {
        self.ids[name]
    }

    fn rev(&self, design: i64) -> i64 {
        self.call("design.get", json!({"design_id": design, "pages_only": true}))["design"]["rev"].as_i64().unwrap()
    }

    /// Put back what a measured edit did, before the next iteration.
    fn later(&self, payload: Value) {
        self.cleanup.lock().unwrap().push(payload);
    }

    fn drain(&self) {
        let pending: Vec<Value> = std::mem::take(&mut *self.cleanup.lock().unwrap());
        for payload in pending {
            self.call("design.apply", payload);
        }
    }

    fn reset(&self) {
        self.drain();
        self.engine.store.lock().restore(rusqlite::MAIN_DB, &self.snapshot, None::<fn(rusqlite::backup::Progress)>).unwrap();
        self.scenes.lock().unwrap().clear();
    }

    /// The scene `key` holds of design `name`, read whole when it has none or the engine's
    /// copy is not the one it matches (the store was put back since).
    fn scene_in_step(&self, key: &str, name: &str) {
        let design = self.id(name);
        let rev = self.rev(design);
        let mut scenes = self.scenes.lock().unwrap();
        if scenes.get(key).is_none_or(|(_, at)| *at != rev) {
            let got = over_the_wire(&self.call("design.get", json!({"design_id": design})));
            let styles = relay_bus::design::Styles { colors: got.colors, styles: got.styles };
            scenes.insert(key.to_string(), (relay_client::design::Scene::new(got.nodes).with_styles(styles), got.design.rev));
        }
    }
}

/// A `design.apply` measured as the person's canvas sends it (the user, a full answer), with
/// `prepare` run uncounted first: it may change the design, and answers the payload. `undo`
/// puts the measured edit back afterwards, uncounted.
fn design_apply(cases: &mut Vec<Case>, name: &str, cost: Cost, undo: bool, prepare: impl Fn(&Fixture, &Designs) -> Value + 'static) {
    cases.push(Case {
        name: format!("op.design.{name}"),
        op: Some("design.apply".into()),
        cost,
        prepare: Box::new(move |fx| {
            let d = fx.designs();
            let payload = prepare(fx, d);
            let design = payload["design_id"].as_i64().unwrap();
            let engine = d.engine.clone();
            let fx = fx.clone();
            Box::new(move || {
                let resp = engine.dispatch(Request::new(Actor::User, "design.apply", payload), Door::InProcess);
                if undo {
                    if let Some(out) = resp.result.as_ref().filter(|_| resp.ok) {
                        fx.designs().later(undo_of(design, out));
                    }
                }
                Outcome::of(resp)
            })
        }),
    });
}

fn design_cases(c: &mut Vec<Case>) {
    use Cost::*;
    for (n, name) in [(500usize, "500"), (2_000, "2000"), (5_000, "5000")] {
        let frames = n.div_ceil(50);
        design_apply(c, &format!("apply.update.{name}"), Mid, false, move |fx, d| {
            let fill = if fx.next() % 2 == 0 { "#FF5C5C" } else { "#2EC469" };
            json!({"design_id": d.id(name), "ops": [{"op": "update", "id": format!("f{}-7", frames / 2), "set": {"fill": fill}}]})
        });
        // The design holds `n` nodes once the node is in: it is taken out first, uncounted.
        design_apply(c, &format!("apply.add.{name}"), Mid, false, move |_, d| {
            d.apply(d.id(name), vec![json!({"op": "delete", "id": "f1-48"})]);
            json!({"design_id": d.id(name), "ops": [grid_frame(1, 49, false)[49].clone()]})
        });
        design_apply(c, &format!("apply.move.{name}"), Mid, true, move |_, d| {
            json!({"design_id": d.id(name), "ops": [{"op": "move", "id": "f1-10", "parent": "f2", "index": 5}]})
        });
        design_apply(c, &format!("apply.delete_subtree.{name}"), Mid, true, move |_, d| {
            json!({"design_id": d.id(name), "ops": [{"op": "delete", "id": "f2"}]})
        });
        // 20 frames of 49 children. Under the cap, new ones, deleted afterwards; at it, the
        // design's last 20 whole frames, taken out first, uncounted, and put back by the batch.
        let room = n + 1_000 <= relay_bus::design::MAX_NODES;
        let last = (n - 1) / 50;
        let batch = move |d: &Designs| -> Vec<Value> {
            if room {
                (0..20).flat_map(|k| grid_frame(1_000 + k, 49, false)).collect()
            } else {
                let ops: Vec<Value> = (last - 20..last).map(|k| json!({"op": "delete", "id": format!("f{k}")})).collect();
                d.apply(d.id(name), ops);
                (last - 20..last).flat_map(|k| grid_frame(k, 49, false)).collect()
            }
        };
        design_apply(c, &format!("apply.batch_1000.{name}"), Heavy, room, move |_, d| json!({"design_id": d.id(name), "ops": batch(d)}));
        design_apply(c, &format!("apply.undo_1000_adds.{name}"), Heavy, false, move |_, d| {
            let out = d.apply(d.id(name), batch(d));
            if !room {
                d.later(json!({"design_id": d.id(name), "ops": (last - 20..last).flat_map(|k| grid_frame(k, 49, false)).collect::<Vec<_>>()}));
            }
            undo_of(d.id(name), &out)
        });
        // engine-pins: a frame resized, its 49 children pinned to its right and bottom edges
        // (each follows: 49 more updates), and the same resize of a frame whose children are not.
        for (row, frame) in [("apply.resize", "f1"), ("apply.resize_pinned", "f0")] {
            design_apply(c, &format!("{row}.{name}"), Mid, false, move |fx, d| {
                let w = if fx.next() % 2 == 0 { 1260 } else { 1200 };
                json!({"design_id": d.id(name), "ops": [{"op": "update", "id": frame, "set": {"w": w, "h": w - 300}}]})
            });
        }
        c.push(Case {
            name: format!("op.design.get.{name}"),
            op: Some("design.get".into()),
            cost: Mid,
            prepare: Box::new(move |fx| {
                let d = fx.designs();
                let req = Request::new(Actor::User, "design.get", json!({"design_id": d.id(name)}));
                let engine = d.engine.clone();
                Box::new(move || Outcome::of(engine.dispatch(req, Door::InProcess)))
            }),
        });
    }

    // A text edited on a page with and without a chart that fills: in another artboard (`a3`),
    // which lays nothing out, and in the chart's own (`a0`, `beside_chart`), which lays that
    // artboard out to size the chart (`fit_charts`). A design without one never runs it.
    for name in ["1721", "1721c", "4903", "4903c"] {
        for (row, text) in [("update_text", "a3-r4-x"), ("update_text_beside_chart", "a0-r4-x")] {
            design_apply(c, &format!("apply.{row}.{name}"), Mid, false, move |fx, d| {
                json!({"design_id": d.id(name), "ops": [{"op": "update", "id": text, "set": {"text": format!("Setting {}", fx.next())}}]})
            });
        }
    }

    // B58: 1,999 of 4,998 siblings into a new group (2,000 edits, the most a list takes) and out
    // again where they were, and every top-level node deleted and undone.
    let group = |_: &Designs| -> Vec<Value> {
        let mut ops = vec![json!({"op": "add", "node": {"id": "g", "parent": PAGE, "kind": "group"}, "index": 1_000})];
        ops.extend((1_000..2_999).map(|i| json!({"op": "move", "id": format!("s{i}"), "parent": "g"})));
        ops
    };
    let ungroup = || -> Vec<Value> {
        let mut ops: Vec<Value> = (1_000..2_999).map(|i| json!({"op": "move", "id": format!("s{i}"), "parent": PAGE, "index": i})).collect();
        ops.push(json!({"op": "delete", "id": "g"}));
        ops
    };
    design_apply(c, "apply.group_1999_of_4998", Heavy, true, move |_, d| json!({"design_id": d.id("siblings"), "ops": group(d)}));
    // Each goes back to its own place, and the group is deleted: the design is as it was.
    design_apply(c, "apply.ungroup_1999_of_4998", Heavy, false, move |_, d| {
        d.apply(d.id("siblings"), group(d));
        json!({"design_id": d.id("siblings"), "ops": ungroup()})
    });
    let everything = || -> Vec<Value> { (0..4_998).map(|i| json!({"op": "delete", "id": format!("s{i}")})).collect() };
    design_apply(c, "apply.delete_all_4998", Heavy, true, move |_, d| json!({"design_id": d.id("siblings"), "ops": everything(), "lenient": true}));
    design_apply(c, "apply.undo_delete_all_4998", Heavy, false, move |_, d| {
        let out = d.call("design.apply", json!({"design_id": d.id("siblings"), "ops": everything(), "lenient": true}));
        undo_of(d.id("siblings"), &out)
    });

    // B85 and TM-80: what the client does with a design of 1,721 and of 4,903 nodes. A commit
    // (one edit of the toggle's fill: fit, expectations, and the scene's all-or-none apply, which
    // clones the scene); a reread after another window's edit of one text, of what changed since
    // (design.get since_rev, the wire, decoding, the scene cloned and patched) and whole (the
    // read, the wire, decoding, Scene::new). The native client's diff of the old scene and the
    // new one is not here: it lives in relay-native, which needs GTK.
    for name in ["1721", "4903"] {
        path_case(c, &format!("design.commit.{name}"), Mid, move |fx| {
            let d = fx.designs();
            let key = format!("commit {name}");
            if !d.scenes.lock().unwrap().contains_key(&key) {
                d.scene_in_step(&key, name);
            }
            let fill = if fx.next() % 2 == 0 { "#FF5C5C" } else { "#2EC469" };
            let set = json!({"fill": fill}).as_object().unwrap().clone();
            let fx = fx.clone();
            Box::new(move || {
                let d = fx.designs();
                let mut scenes = d.scenes.lock().unwrap();
                let scene = &mut scenes.get_mut(&key).unwrap().0;
                let mut ops = vec![relay_bus::design::NodeOp::Update { id: "a3-r4-s".into(), set }];
                scene.fit(&mut ops);
                let expect = relay_client::design::edit::expect_for(scene, &ops);
                match scene.apply_effective(ops) {
                    Ok(kept) => Outcome::plain(kept.len() + expect.len()),
                    Err(e) => Outcome { ok: false, code: Some(e), result: None, bytes: 0, overlap: None },
                }
            })
        });
        path_case(c, &format!("design.reread_delta.{name}"), Mid, move |fx| {
            let d = fx.designs();
            let key = format!("reread {name}");
            d.scene_in_step(&key, name);
            let design = d.id(name);
            d.apply(design, vec![json!({"op": "update", "id": "a3-r4-x", "set": {"text": format!("Changed {}", fx.next())}})]);
            let fx = fx.clone();
            Box::new(move || {
                let d = fx.designs();
                let mut scenes = d.scenes.lock().unwrap();
                let (held, since) = scenes.get_mut(&key).unwrap();
                let resp = d.engine.dispatch(Request::new(Actor::User, "design.get", json!({"design_id": design, "since_rev": *since})), Door::InProcess);
                let got = over_the_wire(resp.result.as_ref().unwrap());
                let mut gone = got.removed.unwrap_or_default();
                gone.extend(got.left);
                let mut scene = held.clone();
                scene.set_styles(relay_bus::design::Styles { colors: got.colors, styles: got.styles });
                let patched = scene.patch(got.nodes.into_iter().zip(got.indexes.unwrap_or_default()).collect(), &gone);
                *since = got.design.rev;
                *held = scene;
                Outcome::res(patched, |_| held.len())
            })
        });
        path_case(c, &format!("design.reread_whole.{name}"), Mid, move |fx| {
            let d = fx.designs();
            let design = d.id(name);
            d.apply(design, vec![json!({"op": "update", "id": "a3-r4-x", "set": {"text": format!("Changed {}", fx.next())}})]);
            let engine = d.engine.clone();
            Box::new(move || {
                let resp = engine.dispatch(Request::new(Actor::User, "design.get", json!({"design_id": design})), Door::InProcess);
                let got = over_the_wire(resp.result.as_ref().unwrap());
                let styles = relay_bus::design::Styles { colors: got.colors, styles: got.styles };
                let scene = relay_client::design::Scene::new(got.nodes).with_styles(styles);
                Outcome::plain(scene.len())
            })
        });
    }
}

// ------------------------------------------------------------------ SQL statements

/// `--sql`: every statement the stores run inside a measured region, by its text, with how many
/// times it ran and SQLite's own time for it (`sqlite3_trace_v2`, `SQLITE_TRACE_PROFILE`). The
/// count is deterministic; the time says which statement to look at (perf-sqlite §8).
static SQL_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SQL_SEEN: Mutex<Option<std::collections::HashMap<String, (u64, u64)>>> = Mutex::new(None);

unsafe extern "C" fn sql_profiled(_mask: std::ffi::c_uint, _ctx: *mut std::ffi::c_void, stmt: *mut std::ffi::c_void, ns: *mut std::ffi::c_void) -> std::ffi::c_int {
    if !SQL_ON.load(Ordering::Relaxed) {
        return 0;
    }
    let text = unsafe { rusqlite::ffi::sqlite3_sql(stmt.cast()) };
    if text.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().split_whitespace().collect::<Vec<_>>().join(" ");
    let ns = unsafe { *(ns as *const i64) } as u64;
    let mut seen = SQL_SEEN.lock().unwrap();
    let entry = seen.get_or_insert_with(Default::default).entry(text).or_default();
    entry.0 += 1;
    entry.1 += ns;
    0
}

/// Report every statement `engine`'s store runs while [`SQL_ON`] is set.
fn sql_trace(engine: &Engine) {
    let conn = engine.store.lock();
    unsafe { rusqlite::ffi::sqlite3_trace_v2(conn.handle(), rusqlite::ffi::SQLITE_TRACE_PROFILE, Some(sql_profiled), std::ptr::null_mut()) };
}

/// The statements counted since the last call, most time first, per iteration.
fn sql_summary(iters: usize) -> Value {
    let seen = SQL_SEEN.lock().unwrap().take().unwrap_or_default();
    let mut rows: Vec<(String, (u64, u64))> = seen.into_iter().collect();
    rows.sort_by_key(|(_, (_, ns))| std::cmp::Reverse(*ns));
    let n = iters.max(1) as u64;
    let total: u64 = rows.iter().map(|(_, (count, _))| count).sum();
    json!({
        "statements_per_iter": total / n,
        "top": rows.iter().take(12).map(|(text, (count, ns))| json!({"sql": text, "per_iter": count / n, "ns_per_iter": ns / n})).collect::<Vec<_>>(),
    })
}

// ------------------------------------------------------------------ measuring

#[derive(Default)]
struct Stats {
    wall_ns: Vec<u64>,
    cpu_ns: u64,
    allocs: u64,
    alloc_bytes: u64,
    ok: usize,
    codes: std::collections::BTreeMap<String, usize>,
    bytes: usize,
    /// "during" cases: iterations whose request went out while the other op was in flight.
    overlap: Option<usize>,
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

fn callgrind(args: &[&str]) {
    let pid = std::process::id().to_string();
    let _ = Command::new("callgrind_control").args(args).arg(&pid).output();
}

struct Strace {
    child: std::process::Child,
    file: PathBuf,
}

/// Paths the harness `access()`es right before and right after each measured region, so the
/// strace pass counts what happened between them and not the iteration's setup, its cleanup or
/// the git processes those fork.
const STRACE_BEGIN: &std::ffi::CStr = c"/relay-perf-mark/begin";
const STRACE_END: &std::ffi::CStr = c"/relay-perf-mark/end";

fn strace_mark(path: &std::ffi::CStr) {
    unsafe { libc::access(path.as_ptr(), libc::F_OK) };
}

/// Every thread of this process has a tracer.
fn all_traced() -> bool {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else { return false };
    tasks.flatten().all(|t| {
        std::fs::read_to_string(t.path().join("status")).unwrap_or_default().lines()
            .find_map(|l| l.strip_prefix("TracerPid:"))
            .is_some_and(|pid| pid.trim() != "0")
    })
}

fn strace_start(name: &str) -> Option<Strace> {
    let file = std::env::temp_dir().join(format!("relay-perf-strace-{}-{name}.txt", std::process::id()));
    // strace is this process's child, and under Yama ptrace_scope 1 a process may only trace its
    // descendants: let any process attach until `strace_stop`. EINVAL without Yama, which needs
    // nothing; scope 2 and 3 still refuse, and strace exits.
    unsafe { libc::prctl(libc::PR_SET_PTRACER, libc::PR_SET_PTRACER_ANY, 0, 0, 0) };
    let spawned = Command::new("strace")
        .args(["-f", "-o"])
        .arg(&file)
        .args(["-p", &std::process::id().to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    // Wait until it holds every thread before the work starts, or has given up.
    let deadline = Instant::now() + Duration::from_secs(10);
    if let Ok(mut child) = spawned {
        loop {
            if all_traced() {
                return Some(Strace { child, file });
            }
            if !matches!(child.try_wait(), Ok(None)) || Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    eprint!("(strace did not attach) ");
    unsafe { libc::prctl(libc::PR_SET_PTRACER, 0, 0, 0, 0) };
    let _ = std::fs::remove_file(&file);
    None
}

/// The syscalls between each begin and end mark, every thread, and how many marked regions the
/// trace holds. Lines are `PID name(args) = ret`; a call another thread interrupted is split
/// into `name(args <unfinished ...>` and `<... name resumed>`, counted once, at its start.
fn strace_count(text: &str) -> (std::collections::BTreeMap<String, u64>, usize) {
    let (begin, end) = (STRACE_BEGIN.to_str().unwrap(), STRACE_END.to_str().unwrap());
    let mut calls = std::collections::BTreeMap::new();
    let (mut inside, mut regions) = (false, 0usize);
    for line in text.lines() {
        if line.contains(begin) {
            inside = true;
            continue;
        }
        if line.contains(end) {
            regions += inside as usize;
            inside = false;
            continue;
        }
        if !inside {
            continue;
        }
        let rest = line.trim_start().trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
        let name = rest.split('(').next().unwrap_or_default();
        if !name.is_empty() && rest.len() > name.len() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            *calls.entry(name.to_string()).or_default() += 1;
        }
    }
    (calls, regions)
}

/// Null, not a count, when strace died during the loop or its trace does not hold one marked
/// region per iteration: a missing count must not read as zero syscalls.
fn strace_stop(mut s: Strace, iters: usize) -> Value {
    let alive = matches!(s.child.try_wait(), Ok(None));
    unsafe { libc::kill(s.child.id() as i32, libc::SIGINT) };
    let _ = s.child.wait();
    unsafe { libc::prctl(libc::PR_SET_PTRACER, 0, 0, 0, 0) };
    let text = std::fs::read_to_string(&s.file).unwrap_or_default();
    let _ = std::fs::remove_file(&s.file);
    let (calls, regions) = strace_count(&text);
    if !alive || regions != iters {
        eprint!("(strace: {} of {iters} regions{}) ", regions, if alive { "" } else { ", exited early" });
        return Value::Null;
    }
    let mut top: Vec<(u64, String)> = calls.into_iter().map(|(name, n)| (n, name)).collect();
    top.sort_by_key(|t| std::cmp::Reverse(t.0));
    let total: u64 = top.iter().map(|t| t.0).sum();
    json!({"total": total, "regions": regions, "top": top.iter().take(8).map(|(n, s)| json!({"syscall": s, "calls": n})).collect::<Vec<_>>()})
}

// ------------------------------------------------------------------ hardware counters

/// `struct perf_event_attr` up to `config1` (`PERF_ATTR_SIZE_VER0`, 64 bytes): every field this
/// harness sets. The kernel reads the rest as zero.
#[repr(C)]
#[derive(Default)]
struct PerfEventAttr {
    kind: u32,
    size: u32,
    config: u64,
    sample_period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup_events: u32,
    bp_type: u32,
    config1: u64,
}

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
const PERF_FORMAT_TOTAL_TIME_ENABLED: u64 = 1;
const PERF_FORMAT_TOTAL_TIME_RUNNING: u64 = 2;
const ATTR_DISABLED: u64 = 1;
const ATTR_INHERIT: u64 = 1 << 1;
const ATTR_EXCLUDE_USER: u64 = 1 << 4;
const ATTR_EXCLUDE_KERNEL: u64 = 1 << 5;
const ATTR_EXCLUDE_HV: u64 = 1 << 6;
/// `inherit` to new threads only, not to child processes (Linux 5.13).
const ATTR_INHERIT_THREAD: u64 = 1 << 35;
const PERF_FLAG_FD_CLOEXEC: libc::c_ulong = 8;

/// Instructions retired, counted by the CPU, on every thread of this process, switched on around
/// exactly the measured call. What callgrind's Ir was, without the emulator: Valgrind cannot run
/// where the loader itself uses AVX-512, and this costs nothing while the counters are off. Two
/// more counts beside it: the instructions of child processes the call started (a `git` the
/// handler forked; callgrind never saw those), and this process's instructions in the kernel.
///
/// One counter per thread (a counter follows one task), opened disabled with `inherit`, so a
/// thread or process started later is counted on the counter of the thread that started it. The
/// main thread owns them all, and `PR_TASK_PERF_EVENTS_ENABLE`/`_DISABLE` switch every counter it
/// owns in one call, so the switch is two syscalls however many threads there are.
struct Counters {
    /// User-space instructions of this process's threads.
    user: Vec<std::os::fd::OwnedFd>,
    /// The same, plus every child process: the children are the difference.
    with_children: Vec<std::os::fd::OwnedFd>,
    /// Empty when `perf_event_paranoid` is 2 or more: kernel instructions need 1 or less.
    kernel: Vec<std::os::fd::OwnedFd>,
    last: [u64; 3],
    multiplexed: bool,
}

fn perf_event_open(tid: i32, exclude: u64) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let attr = PerfEventAttr {
        kind: PERF_TYPE_HARDWARE,
        size: std::mem::size_of::<PerfEventAttr>() as u32,
        config: PERF_COUNT_HW_INSTRUCTIONS,
        read_format: PERF_FORMAT_TOTAL_TIME_ENABLED | PERF_FORMAT_TOTAL_TIME_RUNNING,
        flags: ATTR_DISABLED | ATTR_INHERIT | ATTR_EXCLUDE_HV | exclude,
        ..Default::default()
    };
    let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, &attr as *const PerfEventAttr, tid, -1, -1, PERF_FLAG_FD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd as i32) })
}

impl Counters {
    fn open() -> Result<Counters, String> {
        let tids: Vec<i32> = std::fs::read_dir("/proc/self/task")
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|t| t.file_name().to_str()?.parse().ok())
            .collect();
        let mut c = Counters { user: Vec::new(), with_children: Vec::new(), kernel: Vec::new(), last: [0; 3], multiplexed: false };
        let mut kernel = true;
        for tid in tids {
            match perf_event_open(tid, ATTR_EXCLUDE_KERNEL | ATTR_INHERIT_THREAD) {
                Ok(fd) => c.user.push(fd),
                // A thread that ended since the listing.
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => continue,
                Err(e) => return Err(format!("perf_event_open: {e} (perf_event_paranoid is {})", std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid").unwrap_or_default().trim())),
            }
            c.with_children.push(perf_event_open(tid, ATTR_EXCLUDE_KERNEL).map_err(|e| format!("perf_event_open: {e}"))?);
            if kernel {
                match perf_event_open(tid, ATTR_EXCLUDE_USER | ATTR_INHERIT_THREAD) {
                    Ok(fd) => c.kernel.push(fd),
                    Err(_) => {
                        kernel = false;
                        c.kernel.clear();
                    }
                }
            }
        }
        Ok(c)
    }

    fn sum(fds: &[std::os::fd::OwnedFd], multiplexed: &mut bool) -> u64 {
        use std::os::fd::AsRawFd;
        let mut total = 0;
        for fd in fds {
            let mut v = [0u64; 3];
            let n = unsafe { libc::read(fd.as_raw_fd(), v.as_mut_ptr().cast(), std::mem::size_of_val(&v)) };
            if n == std::mem::size_of_val(&v) as isize {
                total += v[0];
                // The PMU had more counters asked of it than it holds and took turns: the count
                // is an estimate. Flagged, not scaled.
                *multiplexed |= v[2] < v[1];
            }
        }
        total
    }

    fn on() {
        unsafe { libc::prctl(libc::PR_TASK_PERF_EVENTS_ENABLE, 0, 0, 0, 0) };
    }

    fn off() {
        unsafe { libc::prctl(libc::PR_TASK_PERF_EVENTS_DISABLE, 0, 0, 0, 0) };
    }

    /// This process's user instructions, its children's, and its kernel instructions, since the
    /// last call. Read only while the counters are off.
    fn delta(&mut self) -> [u64; 3] {
        let mut m = self.multiplexed;
        let now = [Self::sum(&self.user, &mut m), Self::sum(&self.with_children, &mut m), Self::sum(&self.kernel, &mut m)];
        self.multiplexed = m;
        let d = [now[0] - self.last[0], (now[1] - self.last[1]).saturating_sub(now[0] - self.last[0]), now[2] - self.last[2]];
        self.last = now;
        d
    }
}

fn counter_summary(mut per_iter: Vec<u64>) -> Value {
    per_iter.sort_unstable();
    let n = per_iter.len().max(1) as u64;
    json!({"min": per_iter.first().copied().unwrap_or(0), "p50": percentile(&per_iter, 0.5), "mean": per_iter.iter().sum::<u64>() / n, "max": per_iter.last().copied().unwrap_or(0)})
}

fn run_case(fx: &Arc<Fixture>, case: &Case, iters: usize, warmup: usize, opts: &Opts) -> Value {
    for _ in 0..warmup {
        fx.drain_cleanup();
        let work = (case.prepare)(fx);
        let _ = work();
    }
    fx.drain_cleanup();
    if opts.sql {
        sql_trace(&fx.engine);
        if let Some(designs) = fx.designs.get() {
            sql_trace(&designs.engine);
        }
        SQL_SEEN.lock().unwrap().take();
    }
    let mut stats = Stats::default();
    let rss_before = rss_kb();
    let threads_before = thread_count();
    if opts.callgrind {
        callgrind(&["-z"]);
    }
    let strace = if opts.strace { strace_start(&case.name) } else { None };
    let mut counters = if opts.counters {
        Counters::open().map_err(|e| eprint!("(counters: {e}) ")).ok()
    } else {
        None
    };
    let mut counted: [Vec<u64>; 3] = Default::default();
    let mut marks = opts.marks.as_ref().map(|_| Vec::with_capacity(iters));
    // The last pid handed out before the loop: a process with a higher one was started by the
    // case, and a lower one (a provider stand-in, a straggler from the case before) was not.
    let last_pid = std::fs::read_to_string("/proc/sys/kernel/ns_last_pid").ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
    perf_ctl(&opts.perf_ctl, "enable");
    let started = Instant::now();
    for _ in 0..iters {
        fx.drain_cleanup();
        let work = (case.prepare)(fx);
        if opts.callgrind {
            callgrind(&["-i", "on"]);
        }
        let allocs0 = ALLOCS.load(Ordering::Relaxed);
        let bytes0 = ALLOC_BYTES.load(Ordering::Relaxed);
        let cpu0 = cpu_time();
        if strace.is_some() {
            strace_mark(STRACE_BEGIN);
        }
        if counters.is_some() {
            Counters::on();
        }
        SQL_ON.store(opts.sql, Ordering::Relaxed);
        let m0 = monotonic_ns();
        let t0 = Instant::now();
        let out = perf_measured(work);
        let wall = t0.elapsed();
        let m1 = monotonic_ns();
        SQL_ON.store(false, Ordering::Relaxed);
        if counters.is_some() {
            Counters::off();
        }
        if let Some(c) = &mut counters {
            for (v, d) in counted.iter_mut().zip(c.delta()) {
                v.push(d);
            }
        }
        if let Some(marks) = &mut marks {
            marks.push((m0, m1));
        }
        if strace.is_some() {
            strace_mark(STRACE_END);
        }
        let cpu = cpu_time().saturating_sub(cpu0);
        if opts.callgrind {
            callgrind(&["-i", "off"]);
        }
        stats.wall_ns.push(wall.as_nanos() as u64);
        stats.cpu_ns += cpu.as_nanos() as u64;
        stats.allocs += ALLOCS.load(Ordering::Relaxed) - allocs0;
        stats.alloc_bytes += ALLOC_BYTES.load(Ordering::Relaxed) - bytes0;
        if out.ok {
            stats.ok += 1;
        }
        stats.bytes = stats.bytes.max(out.size());
        if let Some(overlap) = &out.overlap {
            *stats.overlap.get_or_insert(0) += overlap.in_flight() as usize;
        }
        if let Some(code) = out.code {
            *stats.codes.entry(code).or_default() += 1;
        }
    }
    let elapsed = started.elapsed();
    perf_ctl(&opts.perf_ctl, "disable");
    let strace = strace.map(|s| strace_stop(s, iters));
    let sql = opts.sql.then(|| sql_summary(iters));
    let counters = counters.map(|c| {
        let [user, children, kernel] = counted;
        json!({
            "instructions": counter_summary(user),
            "child_instructions": counter_summary(children),
            "kernel_instructions": if c.kernel.is_empty() { Value::Null } else { counter_summary(kernel) },
            "threads": c.user.len(),
            "multiplexed": c.multiplexed,
        })
    });
    if let (Some(path), Some(marks)) = (&opts.marks, marks) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        for (a, b) in marks {
            writeln!(f, "{} {a} {b} {} {last_pid}", case.name, std::process::id()).unwrap();
        }
    }
    if opts.callgrind {
        callgrind(&[&format!("--dump={}", case.name)]);
    }
    fx.drain_cleanup();
    let rss_after = rss_kb();
    let rows = fx.rows();
    let mut sorted = stats.wall_ns.clone();
    sorted.sort_unstable();
    let n = iters.max(1) as u64;
    let mean = stats.wall_ns.iter().sum::<u64>() / n;
    json!({
        "name": case.name,
        "op": case.op,
        "iters": iters,
        "ok": stats.ok,
        "codes": stats.codes,
        "result_bytes": stats.bytes,
        "overlap": stats.overlap,
        "wall_ns": {"min": sorted.first().copied().unwrap_or(0), "p50": percentile(&sorted, 0.5), "p95": percentile(&sorted, 0.95), "max": sorted.last().copied().unwrap_or(0), "mean": mean},
        "cpu_ns_per_iter": stats.cpu_ns / n,
        "allocs_per_iter": stats.allocs / n,
        "alloc_bytes_per_iter": stats.alloc_bytes / n,
        "rss_kb_before": rss_before,
        "rss_kb_after": rss_after,
        "threads_before": threads_before,
        "threads_after": thread_count(),
        "loop_wall_ms": elapsed.as_millis() as u64,
        "rows": rows,
        "strace": strace,
        "counters": counters,
        "sql": sql,
    })
}

// ------------------------------------------------------------------ soak

/// A mixed workload the shell might produce over an afternoon, with live heap bytes and RSS
/// sampled every tenth: growth that does not level off is a leak.
fn soak(fx: &Arc<Fixture>, ops: usize) -> Value {
    let mut samples = Vec::new();
    let mut n = 0usize;
    let checkpoint = (ops / 10).max(1);
    let record = |n: usize, samples: &mut Vec<Value>| {
        samples.push(json!({
            "ops": n,
            "live_heap_kb": LIVE_BYTES.load(Ordering::Relaxed) / 1024,
            "rss_kb": rss_kb(),
            "store_kb": std::fs::metadata(&fx.store_path).map(|m| m.len() / 1024).unwrap_or(0),
            "threads": thread_count(),
        }));
    };
    record(0, &mut samples);
    while n < ops {
        let i = fx.next();
        fx.user("session.list", json!({"project_id": 1}));
        fx.user("task.list", json!({"project_id": 1}));
        fx.user("session.input", json!({"session": fx.builder, "data": "k"}));
        fx.user("session.scrollback", json!({"session": fx.builder, "lines": 40}));
        fx.user("task.update", json!({"task_id": fx.task_id, "body": format!("soak {i}")}));
        fx.user("notes.list", json!({"project_id": 1}));
        fx.user("settings.set", json!({"path": "perf.soak", "value": i}));
        fx.user("mailbox.list", json!({"project_id": 1, "session": fx.reviewer}));
        fx.user("app.status", json!({}));
        fx.user("git.status", json!({"project_id": 1}));
        n += 10;
        if n % checkpoint < 10 {
            record(n, &mut samples);
        }
    }
    json!({"name": "soak", "ops": n, "samples": samples})
}

// ------------------------------------------------------------------ main

struct Opts {
    callgrind: bool,
    strace: bool,
    counters: bool,
    sql: bool,
    /// Where to append each measured region's CLOCK_MONOTONIC bounds, `name start_ns end_ns pid
    /// last_pid` per line: `perf record -k CLOCK_MONOTONIC` stamps its samples on the same clock,
    /// so `scripts/perf/baseline.py` keeps only the samples taken inside a region, tells this
    /// process's samples from its children's by the pid, and drops processes older than the case.
    marks: Option<PathBuf>,
    /// `perf record --control fifo:CTL,ACK -D -1`'s two fifos: recording is switched on for each
    /// scenario's measured loop only, so the fixture build and the warmup leave no samples.
    perf_ctl: Option<(PathBuf, PathBuf)>,
}

/// Tell a `perf record` started with `--control fifo:CTL,ACK` to `enable` or `disable`, and wait
/// for its `ack`, so the samples start and stop where the loop does.
fn perf_ctl(ctl: &Option<(PathBuf, PathBuf)>, cmd: &str) {
    use std::io::{Read, Write};
    let Some((ctl, ack)) = ctl else { return };
    let sent = std::fs::OpenOptions::new().write(true).open(ctl).and_then(|mut f| f.write_all(cmd.as_bytes()));
    if sent.is_ok() {
        let mut buf = [0u8; 8];
        let _ = std::fs::File::open(ack).and_then(|mut f| f.read(&mut buf));
    }
}

fn usage() -> ! {
    eprintln!("usage: perf list\n       perf run [FILTER...] [--iters N] [--warmup N] [--iters-div N] [--scale N] [--out FILE] [--callgrind] [--strace] [--counters] [--sql] [--marks FILE] [--perf-ctl CTL,ACK] [--iters-file FILE] [--keep]\n       perf soak [--ops N] [--scale N] [--out FILE]");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    let mut filters = Vec::new();
    let mut iters: Option<usize> = None;
    let mut iters_div = 1usize;
    let mut warmup = 2usize;
    let mut scale = 1usize;
    let mut out: Option<PathBuf> = None;
    let mut ops = 20_000usize;
    let mut keep = false;
    let mut iters_for = std::collections::HashMap::new();
    let mut opts = Opts { callgrind: false, strace: false, counters: false, sql: false, marks: None, perf_ctl: None };
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--iters" => iters = it.next().and_then(|v| v.parse().ok()),
            "--iters-div" => iters_div = it.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1),
            "--warmup" => warmup = it.next().and_then(|v| v.parse().ok()).unwrap_or(2),
            "--scale" => scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1),
            "--ops" => ops = it.next().and_then(|v| v.parse().ok()).unwrap_or(20_000),
            "--out" => out = it.next().map(PathBuf::from),
            "--callgrind" => opts.callgrind = true,
            "--strace" => opts.strace = true,
            "--counters" => opts.counters = true,
            "--sql" => opts.sql = true,
            "--marks" => opts.marks = it.next().map(PathBuf::from),
            "--perf-ctl" => opts.perf_ctl = it.next().and_then(|v| v.split_once(',')).map(|(c, a)| (PathBuf::from(c), PathBuf::from(a))),
            "--keep" => keep = true,
            // `name iters` per line: a count per scenario, for a sampling pass that wants about
            // the same number of samples from a 5 µs op as from a 50 ms one.
            "--iters-file" => {
                let text = it.next().map(std::fs::read_to_string).unwrap_or_else(|| usage()).unwrap();
                iters_for = text.lines().filter_map(|l| l.split_once(' ')).filter_map(|(n, i)| Some((n.to_string(), i.trim().parse::<usize>().ok()?))).collect();
            }
            other if other.starts_with("--") => usage(),
            other => filters.push(other.to_string()),
        }
    }
    let all = cases();
    match cmd.as_str() {
        "list" => {
            for case in &all {
                println!("{}", case.name);
            }
        }
        "run" => {
            // A filter is a scenario name, or a prefix when it ends in `*`: `op.git.diff` is one
            // case, `op.git.diff*` is three.
            let selected: Vec<&Case> = all
                .iter()
                .filter(|c| filters.is_empty() || filters.iter().any(|f| c.name == *f || (f.ends_with('*') && c.name.starts_with(f.trim_end_matches('*')))))
                .collect();
            if selected.is_empty() {
                eprintln!("no scenario matches {filters:?}");
                std::process::exit(1);
            }
            let fx = Fixture::build(scale, keep);
            let mut sink: Box<dyn std::io::Write> = match &out {
                Some(path) => Box::new(std::fs::File::create(path).unwrap()),
                None => Box::new(std::io::stdout()),
            };
            let suffix = if scale > 1 { format!("@x{scale}") } else { String::new() };
            for case in selected {
                let n = iters_for.get(&case.name).copied().or(iters).unwrap_or(case.cost.iters()) / iters_div;
                let n = n.max(1);
                eprint!("{:<40} ", case.name);
                fx.reset();
                let mut row = run_case(&fx, case, n, warmup.min(n), &opts);
                row["name"] = json!(format!("{}{suffix}", case.name));
                row["scale"] = json!(scale);
                let overlap = row["overlap"].as_u64().map(|k| format!("  {k}/{n} beside the other op")).unwrap_or_default();
                eprintln!("{:>10.1} µs  {} ok  {}{overlap}", row["wall_ns"]["p50"].as_u64().unwrap_or(0) as f64 / 1000.0, row["ok"], row["codes"]);
                writeln!(sink, "{row}").unwrap();
            }
            sink.flush().unwrap();
            // Some of these land after the case that caused them (os.* spawn after commit), so
            // they are counted for the run and not per case.
            eprintln!("host tool calls answered by the fixture's stand-ins: {:?}", host_calls(&fx.host_log));
            fx.engine.shutdown();
        }
        "soak" => {
            let fx = Fixture::build(scale, keep);
            let row = soak(&fx, ops);
            match &out {
                Some(path) => std::fs::write(path, format!("{row}\n")).unwrap(),
                None => println!("{row}"),
            }
            fx.engine.shutdown();
        }
        _ => usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::Counters;

    // Hardware counters are often absent in a VM (CI runners included) or refused by
    // `perf_event_paranoid`; there is nothing to check without them.
    #[test]
    fn counters_count_only_while_on_and_on_every_thread() {
        let Ok(mut c) = Counters::open() else { return };
        let spin = |n: u64| (0..n).fold(0u64, |a, i| std::hint::black_box(a ^ i));
        spin(1_000_000);
        assert_eq!(c.delta()[0], 0, "counted while off");
        Counters::on();
        spin(1_000_000);
        // A thread started while the counters are on is counted on the starting thread's counter.
        std::thread::spawn(move || spin(1_000_000)).join().unwrap();
        Counters::off();
        let [user, children, _] = c.delta();
        assert!((2_000_000..20_000_000).contains(&user), "{user} instructions for two million-step loops");
        assert!(children < 100_000, "{children} instructions in child processes, and none was started");
        spin(1_000_000);
        assert_eq!(c.delta()[0], 0, "counted after off");
    }

    // `cargo test -p relay-core --example perf`: the strace pass cannot run where strace is not
    // installed, so its parser is checked against a trace of the shape `strace -f -o` writes.
    #[test]
    fn strace_counts_only_between_marks() {
        let trace = r#"4101  openat(AT_FDCWD, "/tmp/x/.git/index", O_RDONLY) = 3
4101  clone3({flags=CLONE_VM|CLONE_VFORK, exit_signal=SIGCHLD, stack=0x7f, stack_size=0x9000}, 88) = 4200
4200  execve("/usr/bin/git", ["git", "status"], 0x7ffd /* 40 vars */) = 0
4101  access("/relay-perf-mark/begin", F_OK) = -1 ENOENT (No such file or directory)
4101  getrandom("\x01\x02", 16, 0) = 16
4102  futex(0x55d0, FUTEX_WAIT_PRIVATE, 0, NULL <unfinished ...>
4101  pwrite64(5, "SQLite", 4096, 0) = 4096
4102  <... futex resumed>) = 0
4102  --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED, si_pid=4200, si_uid=1000, si_status=0} ---
4200  +++ exited with 0 +++
4101  access("/relay-perf-mark/end", F_OK) = -1 ENOENT (No such file or directory)
4101  unlink("/tmp/x/scratch") = 0
4101  access("/relay-perf-mark/begin", F_OK <unfinished ...>
4102  epoll_wait(4,  <unfinished ...>
4101  <... access resumed>) = -1 ENOENT (No such file or directory)
4101  pwrite64(5, "SQLite", 4096, 0) = 4096
4101  access("/relay-perf-mark/end", F_OK) = -1 ENOENT (No such file or directory)
4101  close(3) = 0
"#;
        let (calls, regions) = super::strace_count(trace);
        assert_eq!(regions, 2);
        let calls: Vec<(&str, u64)> = calls.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(calls, [("epoll_wait", 1), ("futex", 1), ("getrandom", 1), ("pwrite64", 2)]);
    }
}
